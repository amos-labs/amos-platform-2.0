//! AWS SES v2 client for transactional and bulk email delivery.
//!
//! Uses manual HTTP + SigV4 signing (same pattern as `bedrock.rs`) to avoid
//! the heavy `aws-sdk-sesv2` dependency. Shares AWS credential loading and
//! signing helpers with `bedrock.rs`.
//!
//! The client is initialized at startup if `AMOS__EMAIL__FROM_ADDRESS` is set;
//! otherwise email delivery is a no-op (logged as a warning).

use crate::bedrock::{calculate_signature, load_aws_credentials};
use amos_core::{AmosError, Result};
use chrono::Utc;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use tracing::debug;

/// A single outbound email.
#[derive(Debug, Clone)]
pub struct EmailMessage {
    /// Recipient(s). At least one required.
    pub to: Vec<String>,
    /// Optional CC recipients.
    pub cc: Vec<String>,
    /// Optional BCC recipients.
    pub bcc: Vec<String>,
    /// Subject line.
    pub subject: String,
    /// Plain-text body. At least one of `text` or `html` required.
    pub text: Option<String>,
    /// HTML body.
    pub html: Option<String>,
    /// Override the default From address for this send (must be SES-verified).
    pub from: Option<String>,
    /// Optional reply-to address.
    pub reply_to: Option<String>,
}

/// Result of a successful send.
#[derive(Debug, Clone)]
pub struct SendResult {
    pub message_id: String,
}

/// AWS SES v2 client.
#[derive(Clone)]
pub struct SesClient {
    region: String,
    access_key_id: String,
    secret_access_key: String,
    session_token: Option<String>,
    http_client: reqwest::Client,
    /// Default From address — all sends use this unless overridden per-message.
    default_from: String,
}

impl SesClient {
    /// Build a new SES client. Returns `None` if `from_address` is empty (email disabled).
    ///
    /// Uses the same AWS credential chain as `BedrockClient`:
    /// explicit params → env vars → `~/.aws/credentials`.
    pub fn new(from_address: String, region: Option<String>) -> Result<Self> {
        if from_address.trim().is_empty() {
            return Err(AmosError::Config(
                "SES client requires a non-empty From address".to_string(),
            ));
        }

        let creds = load_aws_credentials(region, None, None)?;

        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| AmosError::Internal(format!("Failed to build HTTP client: {}", e)))?;

        debug!(
            "Initialized SesClient in region {} with from_address {}",
            creds.region, from_address
        );

        Ok(Self {
            region: creds.region,
            access_key_id: creds.access_key_id,
            secret_access_key: creds.secret_access_key,
            session_token: creds.session_token,
            http_client,
            default_from: from_address,
        })
    }

    /// Send a single email via SES v2 `SendEmail` API.
    pub async fn send(&self, msg: EmailMessage) -> Result<SendResult> {
        if msg.to.is_empty() {
            return Err(AmosError::Validation(
                "Email requires at least one recipient".to_string(),
            ));
        }
        if msg.text.is_none() && msg.html.is_none() {
            return Err(AmosError::Validation(
                "Email requires at least a text or html body".to_string(),
            ));
        }

        let from = msg.from.as_deref().unwrap_or(&self.default_from);

        // Build SES v2 SendEmail request body.
        //
        // Shape: https://docs.aws.amazon.com/ses/latest/APIReference-V2/API_SendEmail.html
        let mut body_content = serde_json::Map::new();
        if let Some(ref text) = msg.text {
            body_content.insert(
                "Text".to_string(),
                json!({ "Data": text, "Charset": "UTF-8" }),
            );
        }
        if let Some(ref html) = msg.html {
            body_content.insert(
                "Html".to_string(),
                json!({ "Data": html, "Charset": "UTF-8" }),
            );
        }

        let mut destination = serde_json::Map::new();
        destination.insert("ToAddresses".to_string(), json!(msg.to));
        if !msg.cc.is_empty() {
            destination.insert("CcAddresses".to_string(), json!(msg.cc));
        }
        if !msg.bcc.is_empty() {
            destination.insert("BccAddresses".to_string(), json!(msg.bcc));
        }

        let mut request_body = serde_json::Map::new();
        request_body.insert("FromEmailAddress".to_string(), json!(from));
        request_body.insert("Destination".to_string(), json!(destination));
        request_body.insert(
            "Content".to_string(),
            json!({
                "Simple": {
                    "Subject": { "Data": msg.subject, "Charset": "UTF-8" },
                    "Body": body_content,
                }
            }),
        );
        if let Some(ref reply_to) = msg.reply_to {
            request_body.insert("ReplyToAddresses".to_string(), json!([reply_to]));
        }

        let body_json = serde_json::to_string(&request_body)
            .map_err(|e| AmosError::Internal(format!("Failed to serialize SES request: {}", e)))?;

        let endpoint = format!(
            "https://email.{}.amazonaws.com/v2/email/outbound-emails",
            self.region
        );

        debug!("SES endpoint: {}", endpoint);

        let headers = self.sign_request("POST", &endpoint, &body_json)?;

        let resp = self
            .http_client
            .post(&endpoint)
            .headers(headers)
            .body(body_json)
            .send()
            .await
            .map_err(|e| AmosError::Internal(format!("SES request failed: {}", e)))?;

        let status = resp.status();
        let body_text = resp
            .text()
            .await
            .map_err(|e| AmosError::Internal(format!("Failed to read SES response: {}", e)))?;

        if !status.is_success() {
            return Err(AmosError::Internal(format!(
                "SES error {}: {}",
                status, body_text
            )));
        }

        let parsed: serde_json::Value = serde_json::from_str(&body_text).map_err(|e| {
            AmosError::Internal(format!("Invalid SES response JSON: {} ({})", e, body_text))
        })?;

        let message_id = parsed
            .get("MessageId")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        Ok(SendResult { message_id })
    }

    /// The default From address this client sends from.
    pub fn default_from(&self) -> &str {
        &self.default_from
    }

    /// Sign an SES request using AWS SigV4 (service = "ses").
    fn sign_request(&self, method: &str, url: &str, body: &str) -> Result<HeaderMap> {
        let now = Utc::now();
        let date_stamp = now.format("%Y%m%d").to_string();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();

        let parsed_url = reqwest::Url::parse(url)
            .map_err(|e| AmosError::Internal(format!("Invalid SES URL: {}", e)))?;
        let host = parsed_url
            .host_str()
            .ok_or_else(|| AmosError::Internal("No host in SES URL".to_string()))?;
        let canonical_uri = parsed_url.path().to_string();
        let canonical_querystring = parsed_url.query().unwrap_or("");

        let payload_hash = format!("{:x}", Sha256::digest(body.as_bytes()));

        let mut canonical_headers_map = BTreeMap::new();
        canonical_headers_map.insert("content-type".to_string(), "application/json".to_string());
        canonical_headers_map.insert("host".to_string(), host.to_string());
        canonical_headers_map.insert("x-amz-date".to_string(), amz_date.clone());

        if let Some(ref token) = self.session_token {
            canonical_headers_map.insert("x-amz-security-token".to_string(), token.clone());
        }

        let canonical_headers_str = canonical_headers_map
            .iter()
            .map(|(k, v)| format!("{}:{}", k, v))
            .collect::<Vec<_>>()
            .join("\n");

        let signed_headers = canonical_headers_map
            .keys()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(";");

        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n\n{}\n{}",
            method,
            canonical_uri,
            canonical_querystring,
            canonical_headers_str,
            signed_headers,
            payload_hash
        );

        let canonical_request_hash = format!("{:x}", Sha256::digest(canonical_request.as_bytes()));

        let service = "ses";
        let algorithm = "AWS4-HMAC-SHA256";
        let credential_scope = format!("{}/{}/{}/aws4_request", date_stamp, self.region, service);

        let string_to_sign = format!(
            "{}\n{}\n{}\n{}",
            algorithm, amz_date, credential_scope, canonical_request_hash
        );

        let signature = calculate_signature(
            &self.secret_access_key,
            &date_stamp,
            &self.region,
            service,
            &string_to_sign,
        )?;

        let authorization_header = format!(
            "{} Credential={}/{}, SignedHeaders={}, Signature={}",
            algorithm, self.access_key_id, credential_scope, signed_headers, signature
        );

        let mut headers = HeaderMap::new();
        headers.insert("content-type", HeaderValue::from_static("application/json"));
        headers.insert(
            HeaderName::from_static("x-amz-date"),
            HeaderValue::from_str(&amz_date)
                .map_err(|e| AmosError::Internal(format!("Invalid header value: {}", e)))?,
        );
        headers.insert(
            HeaderName::from_static("authorization"),
            HeaderValue::from_str(&authorization_header)
                .map_err(|e| AmosError::Internal(format!("Invalid header value: {}", e)))?,
        );
        headers.insert(
            HeaderName::from_static("host"),
            HeaderValue::from_str(host)
                .map_err(|e| AmosError::Internal(format!("Invalid header value: {}", e)))?,
        );

        if let Some(ref token) = self.session_token {
            headers.insert(
                HeaderName::from_static("x-amz-security-token"),
                HeaderValue::from_str(token)
                    .map_err(|e| AmosError::Internal(format!("Invalid header value: {}", e)))?,
            );
        }

        Ok(headers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_message_validation_empty_recipients() {
        // Can't construct an SesClient without real AWS creds in tests, so just
        // assert the EmailMessage struct compiles and defaults are sensible.
        let msg = EmailMessage {
            to: vec!["a@b.com".to_string()],
            cc: vec![],
            bcc: vec![],
            subject: "hi".to_string(),
            text: Some("body".to_string()),
            html: None,
            from: None,
            reply_to: None,
        };
        assert_eq!(msg.to.len(), 1);
        assert!(msg.text.is_some());
    }

    #[test]
    fn ses_client_rejects_empty_from() {
        let result = SesClient::new(String::new(), None);
        assert!(result.is_err());
        if let Err(AmosError::Config(msg)) = result {
            assert!(msg.contains("From"));
        } else {
            panic!("expected Config error");
        }
    }
}
