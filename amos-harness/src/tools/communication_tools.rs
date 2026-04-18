//! Communication tools — email (SES), WhatsApp (Twilio), Discord (webhooks).
//!
//! These wrap the transport clients that the harness owns, so the agent can
//! send messages without learning each API. All three are single-provider
//! for now; BYOK (customer-supplied credentials) comes later via the
//! credential vault.

use super::{Tool, ToolCategory, ToolResult};
use crate::ses::{EmailMessage, SesClient};
use amos_core::{AppConfig, Result};
use async_trait::async_trait;
use secrecy::ExposeSecret;
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

// ─── WhatsApp (Twilio) ──────────────────────────────────────────────────

/// Send a WhatsApp message via the Twilio Messaging API.
///
/// Requires `AMOS__TWILIO__ACCOUNT_SID`, `AMOS__TWILIO__AUTH_TOKEN`, and
/// `AMOS__TWILIO__FROM_NUMBER` to be configured.
pub struct SendWhatsappTool {
    config: Arc<AppConfig>,
    http_client: reqwest::Client,
}

impl SendWhatsappTool {
    pub fn new(config: Arc<AppConfig>) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            config,
            http_client,
        }
    }
}

#[async_trait]
impl Tool for SendWhatsappTool {
    fn name(&self) -> &str {
        "send_whatsapp"
    }

    fn description(&self) -> &str {
        "Send a WhatsApp message via Twilio. Requires the harness to be configured \
         with Twilio credentials (AMOS__TWILIO__ACCOUNT_SID, AUTH_TOKEN, FROM_NUMBER). \
         Use this to reach customers or yourself on WhatsApp from an automation or \
         interactively from chat. The recipient must have opted in to receive messages \
         from your Twilio WhatsApp number."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "to": {
                    "type": "string",
                    "description": "Recipient phone number in E.164 format (e.g. '+15551234567'). \
                                    The 'whatsapp:' prefix is added automatically."
                },
                "body": {
                    "type": "string",
                    "description": "Message body (UTF-8, up to 1600 characters)"
                },
                "from": {
                    "type": "string",
                    "description": "Optional override of the default From number. \
                                    Use 'whatsapp:+E.164' format."
                }
            },
            "required": ["to", "body"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Integration
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let cfg = &self.config.twilio;
        let account_sid = match &cfg.account_sid {
            Some(s) if !s.trim().is_empty() => s,
            _ => {
                return Ok(ToolResult::error(
                    "Twilio not configured: set AMOS__TWILIO__ACCOUNT_SID".to_string(),
                ))
            }
        };
        let auth_token = match &cfg.auth_token {
            Some(t) if !t.expose_secret().trim().is_empty() => t.expose_secret().to_string(),
            _ => {
                return Ok(ToolResult::error(
                    "Twilio not configured: set AMOS__TWILIO__AUTH_TOKEN".to_string(),
                ))
            }
        };

        let from = params
            .get("from")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| cfg.from_number.clone())
            .ok_or_else(|| {
                amos_core::AmosError::Config(
                    "Twilio not configured: set AMOS__TWILIO__FROM_NUMBER or pass 'from'"
                        .to_string(),
                )
            })?;
        let from = if from.starts_with("whatsapp:") {
            from
        } else {
            format!("whatsapp:{}", from)
        };

        let to_raw = match params.get("to").and_then(|v| v.as_str()) {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => return Ok(ToolResult::error("`to` is required".to_string())),
        };
        let to = if to_raw.starts_with("whatsapp:") {
            to_raw
        } else {
            format!("whatsapp:{}", to_raw)
        };

        let body = match params.get("body").and_then(|v| v.as_str()) {
            Some(s) if !s.trim().is_empty() => s.to_string(),
            _ => return Ok(ToolResult::error("`body` is required".to_string())),
        };

        let url = format!(
            "https://api.twilio.com/2010-04-01/Accounts/{}/Messages.json",
            account_sid
        );

        let form = [
            ("From", from.as_str()),
            ("To", to.as_str()),
            ("Body", body.as_str()),
        ];

        let resp = self
            .http_client
            .post(&url)
            .basic_auth(account_sid, Some(auth_token))
            .form(&form)
            .send()
            .await;

        match resp {
            Ok(r) => {
                let status = r.status();
                let body_text = r.text().await.unwrap_or_default();
                if !status.is_success() {
                    return Ok(ToolResult::error(format!(
                        "Twilio HTTP {}: {}",
                        status, body_text
                    )));
                }
                let parsed: serde_json::Value = serde_json::from_str(&body_text)
                    .unwrap_or_else(|_| serde_json::json!({ "raw": body_text }));
                let sid = parsed
                    .get("sid")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                Ok(ToolResult::success(json!({
                    "sent": true,
                    "sid": sid,
                    "to": to,
                    "from": from,
                })))
            }
            Err(e) => Ok(ToolResult::error(format!("Twilio request failed: {}", e))),
        }
    }
}

// ─── Discord (Webhook) ──────────────────────────────────────────────────

/// Post a message to a Discord channel via webhook URL.
///
/// The webhook URL can come from `AMOS__DISCORD__DEFAULT_WEBHOOK_URL` or be
/// passed per call. No authentication beyond the URL secret.
pub struct SendDiscordTool {
    config: Arc<AppConfig>,
    http_client: reqwest::Client,
}

impl SendDiscordTool {
    pub fn new(config: Arc<AppConfig>) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            config,
            http_client,
        }
    }
}

#[async_trait]
impl Tool for SendDiscordTool {
    fn name(&self) -> &str {
        "send_discord"
    }

    fn description(&self) -> &str {
        "Post a message to a Discord channel via webhook URL. Get a webhook URL from \
         Discord channel settings → Integrations → Webhooks → New Webhook. Either set \
         AMOS__DISCORD__DEFAULT_WEBHOOK_URL or pass `webhook_url` per call. Supports \
         plain text plus optional username/avatar override and embeds."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "Message text (up to 2000 characters)"
                },
                "webhook_url": {
                    "type": "string",
                    "description": "Discord webhook URL. Optional if AMOS__DISCORD__DEFAULT_WEBHOOK_URL is set."
                },
                "username": {
                    "type": "string",
                    "description": "Optional display name override"
                },
                "avatar_url": {
                    "type": "string",
                    "description": "Optional avatar URL override"
                },
                "embeds": {
                    "type": "array",
                    "description": "Optional Discord embed objects (see Discord API docs)",
                    "items": { "type": "object" }
                }
            },
            "required": ["content"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Integration
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let url = params
            .get("webhook_url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| self.config.discord.default_webhook_url.clone());

        let url = match url {
            Some(u) if !u.trim().is_empty() => u,
            _ => {
                return Ok(ToolResult::error(
                    "No Discord webhook URL available. Set AMOS__DISCORD__DEFAULT_WEBHOOK_URL \
                     or pass `webhook_url` in the tool params."
                        .to_string(),
                ));
            }
        };

        let content = match params.get("content").and_then(|v| v.as_str()) {
            Some(s) if !s.trim().is_empty() => s.to_string(),
            _ => return Ok(ToolResult::error("`content` is required".to_string())),
        };

        let mut body = serde_json::Map::new();
        body.insert("content".to_string(), json!(content));
        if let Some(u) = params.get("username").and_then(|v| v.as_str()) {
            body.insert("username".to_string(), json!(u));
        }
        if let Some(a) = params.get("avatar_url").and_then(|v| v.as_str()) {
            body.insert("avatar_url".to_string(), json!(a));
        }
        if let Some(e) = params.get("embeds").cloned() {
            body.insert("embeds".to_string(), e);
        }

        let resp = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await;

        match resp {
            Ok(r) => {
                let status = r.status();
                if !status.is_success() {
                    let body_text = r.text().await.unwrap_or_default();
                    return Ok(ToolResult::error(format!(
                        "Discord HTTP {}: {}",
                        status, body_text
                    )));
                }
                Ok(ToolResult::success(json!({
                    "sent": true,
                    "status": status.as_u16(),
                })))
            }
            Err(e) => Ok(ToolResult::error(format!("Discord request failed: {}", e))),
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
