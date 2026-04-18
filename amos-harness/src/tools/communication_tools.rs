//! Communication tools — email, WhatsApp, Discord.
//!
//! These wrap the transport clients (SES, Twilio, Discord webhooks) that the
//! harness owns, so the agent can send messages without learning each API.

use super::{Tool, ToolCategory, ToolResult};
use crate::ses::{EmailMessage, SesClient};
use amos_core::Result;
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use std::sync::Arc;

/// Send an email via AWS SES.
pub struct SendEmailTool {
    email_client: Option<Arc<SesClient>>,
}

impl SendEmailTool {
    pub fn new(email_client: Option<Arc<SesClient>>) -> Self {
        Self { email_client }
    }
}

#[async_trait]
impl Tool for SendEmailTool {
    fn name(&self) -> &str {
        "send_email"
    }

    fn description(&self) -> &str {
        "Send an email via AWS SES. Use this for any email-based communication: \
         customer notifications, marketing blasts, transactional messages, receipts. \
         Supports plain-text and HTML bodies, CC/BCC, and reply-to. For mass email, \
         query a collection for recipients then call this tool in a loop or via an \
         automation with channel='email'. Requires the harness to be configured with \
         AMOS__EMAIL__FROM_ADDRESS — if email is disabled, the tool returns an error."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "to": {
                    "description": "Recipient email address(es). String for single, array for multiple.",
                    "oneOf": [
                        { "type": "string" },
                        { "type": "array", "items": { "type": "string" } }
                    ]
                },
                "subject": {
                    "type": "string",
                    "description": "Email subject line"
                },
                "text": {
                    "type": "string",
                    "description": "Plain-text body. At least one of `text` or `html` required."
                },
                "html": {
                    "type": "string",
                    "description": "HTML body. At least one of `text` or `html` required."
                },
                "cc": {
                    "description": "Optional CC recipient(s)",
                    "oneOf": [
                        { "type": "string" },
                        { "type": "array", "items": { "type": "string" } }
                    ]
                },
                "bcc": {
                    "description": "Optional BCC recipient(s)",
                    "oneOf": [
                        { "type": "string" },
                        { "type": "array", "items": { "type": "string" } }
                    ]
                },
                "from": {
                    "type": "string",
                    "description": "Optional override of the default From address. Must be SES-verified."
                },
                "reply_to": {
                    "type": "string",
                    "description": "Optional Reply-To address"
                }
            },
            "required": ["to", "subject"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Integration
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let client = match &self.email_client {
            Some(c) => c,
            None => {
                return Ok(ToolResult::error(
                    "Email is not configured on this harness. Set AMOS__EMAIL__FROM_ADDRESS \
                     to enable SES delivery."
                        .to_string(),
                ));
            }
        };

        let to = parse_address_list(params.get("to"));
        if to.is_empty() {
            return Ok(ToolResult::error(
                "`to` is required and must be a non-empty string or array of strings".to_string(),
            ));
        }
        let cc = parse_address_list(params.get("cc"));
        let bcc = parse_address_list(params.get("bcc"));

        let subject = match params.get("subject").and_then(|v| v.as_str()) {
            Some(s) if !s.trim().is_empty() => s.to_string(),
            _ => return Ok(ToolResult::error("`subject` is required".to_string())),
        };

        let text = params
            .get("text")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let html = params
            .get("html")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        if text.is_none() && html.is_none() {
            return Ok(ToolResult::error(
                "At least one of `text` or `html` body is required".to_string(),
            ));
        }

        let from = params
            .get("from")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let reply_to = params
            .get("reply_to")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let msg = EmailMessage {
            to: to.clone(),
            cc,
            bcc,
            subject: subject.clone(),
            text,
            html,
            from,
            reply_to,
        };

        match client.send(msg).await {
            Ok(result) => Ok(ToolResult::success(json!({
                "sent": true,
                "message_id": result.message_id,
                "to": to,
                "subject": subject,
            }))),
            Err(e) => Ok(ToolResult::error(format!("Email send failed: {}", e))),
        }
    }
}

/// Parse a JSON value into a list of email addresses.
/// Accepts either a single string or an array of strings.
fn parse_address_list(value: Option<&JsonValue>) -> Vec<String> {
    match value {
        Some(JsonValue::String(s)) if !s.trim().is_empty() => vec![s.trim().to_string()],
        Some(JsonValue::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_metadata() {
        let tool = SendEmailTool::new(None);
        assert_eq!(tool.name(), "send_email");
        assert_eq!(tool.category(), ToolCategory::Integration);
        let schema = tool.parameters_schema();
        assert!(schema["properties"]["to"].is_object());
        assert!(schema["properties"]["subject"].is_object());
        let required = schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 2);
    }

    #[tokio::test]
    async fn returns_error_when_email_disabled() {
        let tool = SendEmailTool::new(None);
        let result = tool
            .execute(json!({
                "to": "a@b.com",
                "subject": "hi",
                "text": "body"
            }))
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("not configured"));
    }

    #[test]
    fn parse_address_list_single_string() {
        let v = json!("a@b.com");
        assert_eq!(parse_address_list(Some(&v)), vec!["a@b.com".to_string()]);
    }

    #[test]
    fn parse_address_list_array() {
        let v = json!(["a@b.com", "c@d.com"]);
        assert_eq!(parse_address_list(Some(&v)).len(), 2);
    }

    #[test]
    fn parse_address_list_empty_string_returns_empty() {
        let v = json!("");
        assert!(parse_address_list(Some(&v)).is_empty());
    }

    #[test]
    fn parse_address_list_missing_returns_empty() {
        assert!(parse_address_list(None).is_empty());
    }

    #[test]
    fn parse_address_list_filters_empty_strings_in_array() {
        let v = json!(["a@b.com", "", "  ", "c@d.com"]);
        assert_eq!(parse_address_list(Some(&v)).len(), 2);
    }
}
