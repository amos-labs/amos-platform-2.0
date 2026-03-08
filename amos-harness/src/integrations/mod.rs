//! Integration connector framework
//!
//! Provides a pluggable system for connecting to third-party services.
//! Includes:
//! - Type definitions for all integration DB models
//! - Universal API executor for authenticated HTTP calls
//! - ETL pipeline for data synchronization

pub mod etl;
pub mod executor;
pub mod types;

use amos_core::{AmosError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;

/// Integration connector trait
#[async_trait]
pub trait Connector: Send + Sync {
    /// Get connector name
    fn name(&self) -> &str;

    /// Connect/authenticate with the service
    async fn connect(&self, config: &IntegrationConfig) -> Result<()>;

    /// Disconnect from the service
    async fn disconnect(&self) -> Result<()>;

    /// Execute an action on the integration
    async fn execute_action(&self, action: &str, params: JsonValue) -> Result<JsonValue>;

    /// List available actions
    fn list_actions(&self) -> Vec<ActionInfo>;

    /// Get connector type
    fn connector_type(&self) -> ConnectorType;
}

/// Connector type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ConnectorType {
    CRM,
    Email,
    Payment,
    Calendar,
    Storage,
    Communication,
    Analytics,
    Custom,
}

/// Integration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationConfig {
    pub connector_type: ConnectorType,
    pub credentials: JsonValue,
    pub endpoint: Option<String>,
    pub metadata: Option<JsonValue>,
}

/// Action information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionInfo {
    pub name: String,
    pub description: String,
    pub parameters_schema: JsonValue,
}

/// Connector registry
pub struct ConnectorRegistry {
    connectors: HashMap<String, Arc<dyn Connector>>,
}

impl ConnectorRegistry {
    /// Create a new connector registry
    pub fn new() -> Self {
        Self {
            connectors: HashMap::new(),
        }
    }

    /// Register a connector
    pub fn register(&mut self, connector: Arc<dyn Connector>) {
        self.connectors.insert(connector.name().to_string(), connector);
    }

    /// Get a connector by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn Connector>> {
        self.connectors.get(name).cloned()
    }

    /// List all registered connectors
    pub fn list(&self) -> Vec<String> {
        self.connectors.keys().cloned().collect()
    }

    /// Create a registry with built-in connectors
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();

        registry.register(Arc::new(CRMConnector));
        registry.register(Arc::new(EmailConnector));
        registry.register(Arc::new(PaymentConnector));
        registry.register(Arc::new(CalendarConnector));
        registry.register(Arc::new(StorageConnector));

        registry
    }
}

impl Default for ConnectorRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// Built-in connector stubs

/// CRM connector (Salesforce, HubSpot, etc.)
pub struct CRMConnector;

#[async_trait]
impl Connector for CRMConnector {
    fn name(&self) -> &str {
        "crm"
    }

    async fn connect(&self, _config: &IntegrationConfig) -> Result<()> {
        // TODO: Implement actual CRM connection
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    async fn execute_action(&self, action: &str, params: JsonValue) -> Result<JsonValue> {
        match action {
            "get_contact" => Ok(serde_json::json!({
                "id": "contact_123",
                "name": "John Doe",
                "email": "john@example.com"
            })),
            "create_lead" => Ok(serde_json::json!({
                "id": "lead_456",
                "status": "created"
            })),
            _ => Err(AmosError::NotFound { entity: "Action".to_string(), id: action.to_string() }),
        }
    }

    fn list_actions(&self) -> Vec<ActionInfo> {
        vec![
            ActionInfo {
                name: "get_contact".to_string(),
                description: "Get a contact by ID".to_string(),
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" }
                    }
                }),
            },
            ActionInfo {
                name: "create_lead".to_string(),
                description: "Create a new lead".to_string(),
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "email": { "type": "string" }
                    }
                }),
            },
        ]
    }

    fn connector_type(&self) -> ConnectorType {
        ConnectorType::CRM
    }
}

/// Email connector (Gmail, SendGrid, etc.)
pub struct EmailConnector;

#[async_trait]
impl Connector for EmailConnector {
    fn name(&self) -> &str {
        "email"
    }

    async fn connect(&self, _config: &IntegrationConfig) -> Result<()> {
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    async fn execute_action(&self, action: &str, _params: JsonValue) -> Result<JsonValue> {
        match action {
            "send_email" => Ok(serde_json::json!({
                "message_id": "msg_789",
                "status": "sent"
            })),
            _ => Err(AmosError::NotFound { entity: "Action".to_string(), id: action.to_string() }),
        }
    }

    fn list_actions(&self) -> Vec<ActionInfo> {
        vec![ActionInfo {
            name: "send_email".to_string(),
            description: "Send an email".to_string(),
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "to": { "type": "string" },
                    "subject": { "type": "string" },
                    "body": { "type": "string" }
                }
            }),
        }]
    }

    fn connector_type(&self) -> ConnectorType {
        ConnectorType::Email
    }
}

/// Payment connector (Stripe, PayPal, etc.)
pub struct PaymentConnector;

#[async_trait]
impl Connector for PaymentConnector {
    fn name(&self) -> &str {
        "payment"
    }

    async fn connect(&self, _config: &IntegrationConfig) -> Result<()> {
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    async fn execute_action(&self, action: &str, _params: JsonValue) -> Result<JsonValue> {
        match action {
            "create_payment" => Ok(serde_json::json!({
                "payment_id": "pay_123",
                "status": "succeeded"
            })),
            _ => Err(AmosError::NotFound { entity: "Action".to_string(), id: action.to_string() }),
        }
    }

    fn list_actions(&self) -> Vec<ActionInfo> {
        vec![ActionInfo {
            name: "create_payment".to_string(),
            description: "Create a payment".to_string(),
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "amount": { "type": "number" },
                    "currency": { "type": "string" }
                }
            }),
        }]
    }

    fn connector_type(&self) -> ConnectorType {
        ConnectorType::Payment
    }
}

/// Calendar connector (Google Calendar, Outlook, etc.)
pub struct CalendarConnector;

#[async_trait]
impl Connector for CalendarConnector {
    fn name(&self) -> &str {
        "calendar"
    }

    async fn connect(&self, _config: &IntegrationConfig) -> Result<()> {
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    async fn execute_action(&self, action: &str, _params: JsonValue) -> Result<JsonValue> {
        match action {
            "create_event" => Ok(serde_json::json!({
                "event_id": "evt_456",
                "status": "created"
            })),
            _ => Err(AmosError::NotFound { entity: "Action".to_string(), id: action.to_string() }),
        }
    }

    fn list_actions(&self) -> Vec<ActionInfo> {
        vec![ActionInfo {
            name: "create_event".to_string(),
            description: "Create a calendar event".to_string(),
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "start": { "type": "string" },
                    "end": { "type": "string" }
                }
            }),
        }]
    }

    fn connector_type(&self) -> ConnectorType {
        ConnectorType::Calendar
    }
}

/// Storage connector (S3, Dropbox, Google Drive, etc.)
pub struct StorageConnector;

#[async_trait]
impl Connector for StorageConnector {
    fn name(&self) -> &str {
        "storage"
    }

    async fn connect(&self, _config: &IntegrationConfig) -> Result<()> {
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    async fn execute_action(&self, action: &str, _params: JsonValue) -> Result<JsonValue> {
        match action {
            "upload_file" => Ok(serde_json::json!({
                "file_id": "file_789",
                "url": "https://storage.example.com/file_789"
            })),
            _ => Err(AmosError::NotFound { entity: "Action".to_string(), id: action.to_string() }),
        }
    }

    fn list_actions(&self) -> Vec<ActionInfo> {
        vec![ActionInfo {
            name: "upload_file".to_string(),
            description: "Upload a file".to_string(),
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_name": { "type": "string" },
                    "content": { "type": "string" }
                }
            }),
        }]
    }

    fn connector_type(&self) -> ConnectorType {
        ConnectorType::Storage
    }
}
