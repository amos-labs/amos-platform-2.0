//! External HTTP metric adapter — fetches fitness metric values from
//! third-party APIs (trading platforms, analytics services, etc.)
//!
//! Configuration lives in `FitnessFunction.metric_endpoint` (the URL) and
//! `FitnessFunction.metric_config` (auth + response extraction):
//!
//! ```json
//! {
//!   "auth_header": "Authorization",
//!   "credential": "Bearer sk-...",
//!   "response_path": "$.data.sharpe_ratio"
//! }
//! ```

use crate::types::FitnessFunction;
use amos_core::{AmosError, Result};
use serde_json::Value as JsonValue;

/// Fetch a metric value from an external HTTP endpoint.
///
/// 1. Read the endpoint URL from `function.metric_endpoint`.
/// 2. Optionally attach an auth header from `function.metric_config`.
/// 3. Parse the JSON response and extract a float via `response_path`.
pub async fn fetch_external(
    client: &reqwest::Client,
    function: &FitnessFunction,
    _agent_id: i32,
) -> Result<f64> {
    let endpoint = function
        .metric_endpoint
        .as_deref()
        .ok_or_else(|| AmosError::Validation("External metric requires metric_endpoint".into()))?;

    let config = &function.metric_config;

    // Build the request, optionally adding an authentication header.
    let mut request = client.get(endpoint);

    if let (Some(header_name), Some(credential)) = (
        config["auth_header"].as_str(),
        config["credential"].as_str(),
    ) {
        request = request.header(header_name, credential);
    }

    let response = request.send().await.map_err(|e| {
        AmosError::Internal(format!("External metric request to {endpoint} failed: {e}"))
    })?;

    if !response.status().is_success() {
        return Err(AmosError::Internal(format!(
            "External metric endpoint returned HTTP {}",
            response.status()
        )));
    }

    let body: JsonValue = response.json().await.map_err(|e| {
        AmosError::Internal(format!(
            "Failed to parse external metric response as JSON: {e}"
        ))
    })?;

    // Extract the value using the configured JSON path.
    let response_path = config["response_path"].as_str().unwrap_or("$.value");

    extract_json_path(&body, response_path).ok_or_else(|| {
        AmosError::Internal(format!(
            "Could not extract numeric value at path '{response_path}' from response"
        ))
    })
}

/// Simple dot-path extraction from a JSON value.
///
/// Supports paths like:
/// - `"$.field.subfield"` (leading `$` and `.` are stripped)
/// - `"field.subfield"`
/// - `"value"` (single key)
///
/// Returns `Some(f64)` if the path resolves to a number (or a numeric string),
/// `None` otherwise. This is deliberately minimal — not a full JSONPath
/// implementation.
pub fn extract_json_path(value: &JsonValue, path: &str) -> Option<f64> {
    // Strip the optional leading `$.` or `$` prefix.
    let normalized = path
        .strip_prefix("$.")
        .or_else(|| path.strip_prefix("$"))
        .unwrap_or(path);

    let segments: Vec<&str> = normalized.split('.').filter(|s| !s.is_empty()).collect();

    let mut current = value;
    for segment in &segments {
        // Try object key access first.
        if let Some(next) = current.get(segment) {
            current = next;
            continue;
        }
        // Try array index access (e.g. "0", "1").
        if let Ok(idx) = segment.parse::<usize>() {
            if let Some(next) = current.get(idx) {
                current = next;
                continue;
            }
        }
        return None;
    }

    // Attempt to convert the terminal value to f64.
    if let Some(n) = current.as_f64() {
        return Some(n);
    }
    // Fall back to parsing a string representation.
    if let Some(s) = current.as_str() {
        return s.parse::<f64>().ok();
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_simple_key() {
        let data = json!({"sharpe_ratio": 1.42});
        assert_eq!(extract_json_path(&data, "$.sharpe_ratio"), Some(1.42));
    }

    #[test]
    fn test_extract_nested() {
        let data = json!({"data": {"metrics": {"sharpe": 2.1}}});
        assert_eq!(extract_json_path(&data, "$.data.metrics.sharpe"), Some(2.1));
    }

    #[test]
    fn test_extract_without_dollar_prefix() {
        let data = json!({"value": 0.85});
        assert_eq!(extract_json_path(&data, "value"), Some(0.85));
    }

    #[test]
    fn test_extract_string_number() {
        let data = json!({"rate": "2.72"});
        assert_eq!(extract_json_path(&data, "$.rate"), Some(2.72));
    }

    #[test]
    fn test_extract_array_index() {
        let data = json!({"results": [10.0, 20.0, 30.0]});
        assert_eq!(extract_json_path(&data, "$.results.1"), Some(20.0));
    }

    #[test]
    fn test_extract_missing_path() {
        let data = json!({"a": 1});
        assert_eq!(extract_json_path(&data, "$.b.c"), None);
    }

    #[test]
    fn test_extract_non_numeric() {
        let data = json!({"name": "hello"});
        assert_eq!(extract_json_path(&data, "$.name"), None);
    }
}
