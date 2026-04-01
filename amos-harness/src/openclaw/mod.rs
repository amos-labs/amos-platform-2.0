//! OpenClaw agent control plane
//!
//! Manages autonomous AI agents that register with AMOS and can be managed like
//! employees. Each agent has its own workspace, memory, tools, and capabilities.
//! They communicate with AMOS via a real WebSocket connection to the OpenClaw gateway.

use amos_core::{AmosError, AppConfig, Result};
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// Agent configuration as stored in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub agent_id: i32,
    pub name: String,
    pub display_name: String,
    pub role: String,
    pub capabilities: Vec<String>,
    pub system_prompt: Option<String>,
    pub model: String,
    // Autoresearch extensions
    pub provider_type: Option<String>,
    pub api_base: Option<String>,
    pub max_concurrent_tasks: Option<i32>,
    pub always_on: Option<bool>,
    pub cost_tier: Option<String>,
    pub task_specializations: Option<JsonValue>,
}

/// Agent status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Registered,
    Active,
    Working,
    Idle,
    Stopped,
    Error,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Registered => write!(f, "registered"),
            AgentStatus::Active => write!(f, "active"),
            AgentStatus::Working => write!(f, "working"),
            AgentStatus::Idle => write!(f, "idle"),
            AgentStatus::Stopped => write!(f, "stopped"),
            AgentStatus::Error => write!(f, "error"),
        }
    }
}

/// OpenClaw protocol frame types
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum OpenClawFrame {
    #[serde(rename = "req")]
    Request {
        id: String,
        method: String,
        params: JsonValue,
    },
    #[serde(rename = "res")]
    Response {
        id: String,
        ok: bool,
        payload: Option<JsonValue>,
        error: Option<OpenClawError>,
    },
    #[serde(rename = "event")]
    Event { event: String, payload: JsonValue },
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenClawError {
    code: String,
    message: String,
}

/// OpenClaw connection manager
struct OpenClawConnection {
    gateway_url: String,
    write_tx: mpsc::UnboundedSender<Message>,
    pending_requests: Arc<DashMap<String, oneshot::Sender<JsonValue>>>,
    connected: Arc<RwLock<bool>>,
    protocol_ready: Arc<RwLock<bool>>,
}

impl OpenClawConnection {
    /// Create a new OpenClaw connection
    async fn new(gateway_url: String) -> Result<Self> {
        let pending_requests = Arc::new(DashMap::new());
        let connected = Arc::new(RwLock::new(false));
        let protocol_ready = Arc::new(RwLock::new(false));

        let (write_tx, write_rx) = mpsc::unbounded_channel();

        let conn = Self {
            gateway_url: gateway_url.clone(),
            write_tx,
            pending_requests: pending_requests.clone(),
            connected: connected.clone(),
            protocol_ready: protocol_ready.clone(),
        };

        // Spawn connection task
        tokio::spawn(Self::connection_task(
            gateway_url,
            write_rx,
            pending_requests,
            connected,
            protocol_ready,
        ));

        Ok(conn)
    }

    /// Main connection task that manages WebSocket lifecycle.
    ///
    /// Uses exponential backoff (5s -> 10s -> 20s -> ... -> 5min cap)
    /// so logs don't flood when the gateway is down.
    async fn connection_task(
        gateway_url: String,
        write_rx: mpsc::UnboundedReceiver<Message>,
        pending_requests: Arc<DashMap<String, oneshot::Sender<JsonValue>>>,
        connected: Arc<RwLock<bool>>,
        protocol_ready: Arc<RwLock<bool>>,
    ) {
        let mut write_rx = write_rx;
        let mut backoff_secs: u64 = 5;
        const MAX_BACKOFF_SECS: u64 = 300; // 5 minutes

        loop {
            debug!("Connecting to OpenClaw gateway at {}", gateway_url);

            match connect_async(&gateway_url).await {
                Ok((ws_stream, _)) => {
                    info!("Connected to OpenClaw gateway");
                    *connected.write().await = true;
                    backoff_secs = 5; // reset backoff on successful connection

                    let (mut write, mut read) = ws_stream.split();

                    // Process messages in this task
                    loop {
                        tokio::select! {
                            // Handle incoming messages
                            msg_result = read.next() => {
                                match msg_result {
                                    Some(Ok(Message::Text(text))) => {
                                        debug!("Received: {}", text);

                                        match serde_json::from_str::<OpenClawFrame>(&text) {
                                            Ok(frame) => {
                                                Self::handle_frame(frame, &pending_requests, &protocol_ready).await;
                                            }
                                            Err(e) => {
                                                error!("Failed to parse frame: {}", e);
                                            }
                                        }
                                    }
                                    Some(Ok(Message::Close(_))) => {
                                        info!("WebSocket closed by server");
                                        break;
                                    }
                                    Some(Ok(Message::Ping(_))) => {
                                        debug!("Received ping, pong will be sent automatically");
                                    }
                                    Some(Ok(_)) => {} // Ignore other message types
                                    Some(Err(e)) => {
                                        error!("WebSocket error: {}", e);
                                        break;
                                    }
                                    None => {
                                        debug!("WebSocket stream ended");
                                        break;
                                    }
                                }
                            }
                            // Handle outgoing messages
                            Some(msg) = write_rx.recv() => {
                                if let Err(e) = write.send(msg).await {
                                    error!("Failed to send message: {}", e);
                                    break;
                                }
                            }
                        }
                    }

                    *connected.write().await = false;
                    *protocol_ready.write().await = false;

                    // Clear pending requests
                    pending_requests.clear();
                }
                Err(e) => {
                    // Only log at debug level after first failure to avoid log spam
                    if backoff_secs <= 5 {
                        warn!("Failed to connect to OpenClaw gateway: {}", e);
                    } else {
                        debug!(
                            "OpenClaw gateway still unavailable (retry in {}s)",
                            backoff_secs
                        );
                    }
                    *connected.write().await = false;
                    *protocol_ready.write().await = false;
                }
            }

            // Exponential backoff before reconnecting
            tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)).await;
            backoff_secs = (backoff_secs * 2).min(MAX_BACKOFF_SECS);
        }
    }

    /// Handle incoming frame
    async fn handle_frame(
        frame: OpenClawFrame,
        pending_requests: &Arc<DashMap<String, oneshot::Sender<JsonValue>>>,
        _protocol_ready: &Arc<RwLock<bool>>,
    ) {
        match frame {
            OpenClawFrame::Response {
                id,
                ok,
                payload,
                error,
            } => {
                if let Some((_, tx)) = pending_requests.remove(&id) {
                    if ok {
                        if let Some(payload) = payload {
                            let _ = tx.send(payload);
                        } else {
                            let _ = tx.send(serde_json::json!({}));
                        }
                    } else {
                        error!("Request {} failed: {:?}", id, error);
                        // Send empty response on error
                        let _ = tx.send(serde_json::json!({}));
                    }
                }
            }
            OpenClawFrame::Event { event, payload } => match event.as_str() {
                "connect.challenge" => {
                    info!("Received connect.challenge: {:?}", payload);
                }
                "agent.task.completed" => {
                    info!("Agent task completed: {:?}", payload);
                }
                "agent.status" => {
                    debug!("Agent status update: {:?}", payload);
                }
                "tick" => {
                    debug!("Heartbeat tick");
                }
                _ => {
                    debug!("Unknown event: {} {:?}", event, payload);
                }
            },
            OpenClawFrame::Request { .. } => {
                warn!("Received unexpected request frame from server");
            }
        }
    }

    /// Perform connection handshake
    async fn handshake(&self) -> Result<()> {
        // Wait for connection
        let mut retries = 0;
        while !*self.connected.read().await {
            if retries > 10 {
                return Err(AmosError::Internal(
                    "Failed to connect to OpenClaw gateway".to_string(),
                ));
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            retries += 1;
        }

        // Wait a bit for connect.challenge event
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Send connect request
        let connect_params = serde_json::json!({
            "minProtocol": 1,
            "maxProtocol": 1,
            "client": {
                "id": format!("amos-harness-{}", Uuid::new_v4()),
                "displayName": "AMOS Harness",
                "version": "1.0.0",
                "platform": "linux",
                "mode": "cli"
            },
            "role": "operator",
            "scopes": ["operator.admin"],
            "auth": {
                "token": serde_json::Value::Null
            },
            "commands": [],
            "caps": []
        });

        let response = self.send_request("connect", connect_params).await?;

        // Check if handshake was successful
        if response.get("protocol").is_some() {
            *self.protocol_ready.write().await = true;
            info!("OpenClaw handshake successful");
            Ok(())
        } else {
            Err(AmosError::Internal("OpenClaw handshake failed".to_string()))
        }
    }

    /// Send a request and wait for response
    async fn send_request(&self, method: &str, params: JsonValue) -> Result<JsonValue> {
        if !*self.connected.read().await {
            return Err(AmosError::Internal(
                "Not connected to OpenClaw gateway".to_string(),
            ));
        }

        let id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();

        self.pending_requests.insert(id.clone(), tx);

        let frame = OpenClawFrame::Request {
            id: id.clone(),
            method: method.to_string(),
            params,
        };

        let message = serde_json::to_string(&frame)
            .map_err(|e| AmosError::Internal(format!("Failed to serialize request: {}", e)))?;

        self.write_tx
            .send(Message::Text(message.into()))
            .map_err(|e| AmosError::Internal(format!("Failed to send message: {}", e)))?;

        // Wait for response with timeout
        match tokio::time::timeout(tokio::time::Duration::from_secs(30), rx).await {
            Ok(Ok(payload)) => Ok(payload),
            Ok(Err(_)) => Err(AmosError::Internal("Response channel closed".to_string())),
            Err(_) => {
                self.pending_requests.remove(&id);
                Err(AmosError::Internal("Request timeout".to_string()))
            }
        }
    }

    /// Register an agent with the OpenClaw gateway
    async fn register_agent(&self, config: &AgentConfig) -> Result<String> {
        let params = serde_json::json!({
            "name": config.name,
            "displayName": config.display_name,
            "model": config.model,
            "role": config.role,
            "systemPrompt": config.system_prompt,
            "capabilities": config.capabilities,
        });

        let response = self.send_request("agents.create", params).await?;

        if let Some(agent_id) = response.get("agentId").and_then(|v| v.as_str()) {
            Ok(agent_id.to_string())
        } else {
            Err(AmosError::Internal(
                "Failed to get agent ID from response".to_string(),
            ))
        }
    }

    /// Assign a task to an agent via the gateway
    async fn assign_task(
        &self,
        gateway_agent_id: &str,
        title: &str,
        description: &str,
        context: &JsonValue,
    ) -> Result<JsonValue> {
        let params = serde_json::json!({
            "agentId": gateway_agent_id,
            "task": {
                "title": title,
                "description": description,
                "context": context
            }
        });

        self.send_request("agents.assignTask", params).await
    }

    /// List agents known to the gateway
    async fn list_gateway_agents(&self) -> Result<JsonValue> {
        self.send_request("agents.list", serde_json::json!({}))
            .await
    }

    /// Stop an agent on the gateway
    async fn stop_gateway_agent(&self, gateway_agent_id: &str) -> Result<()> {
        let params = serde_json::json!({
            "agentId": gateway_agent_id
        });
        self.send_request("agents.stop", params).await?;
        Ok(())
    }
}

/// Agent manager handles lifecycle and communication with OpenClaw agents
pub struct AgentManager {
    db_pool: PgPool,
    config: Arc<AppConfig>,
    active_agents: Arc<RwLock<HashMap<i32, AgentStatus>>>,
    openclaw_conn: Arc<RwLock<Option<Arc<OpenClawConnection>>>>,
    /// Maps local agent_id to OpenClaw gateway agent_id
    gateway_agent_ids: Arc<RwLock<HashMap<i32, String>>>,
}

impl AgentManager {
    /// Create a new agent manager.
    ///
    /// Only attempts OpenClaw connection if `OPENCLAW_GATEWAY_URL` is set.
    /// Without it the gateway is assumed absent and no background reconnect
    /// task is spawned.
    pub async fn new(db_pool: PgPool, config: Arc<AppConfig>) -> Result<Self> {
        let manager = Self {
            db_pool,
            config,
            active_agents: Arc::new(RwLock::new(HashMap::new())),
            openclaw_conn: Arc::new(RwLock::new(None)),
            gateway_agent_ids: Arc::new(RwLock::new(HashMap::new())),
        };

        // Only connect if an explicit gateway URL is configured
        if std::env::var("OPENCLAW_GATEWAY_URL").is_ok() {
            if let Err(e) = manager.ensure_openclaw_connection().await {
                warn!("OpenClaw gateway not available: {}", e);
            }
        } else {
            info!("OPENCLAW_GATEWAY_URL not set — OpenClaw gateway disabled");
        }

        Ok(manager)
    }

    /// Ensure OpenClaw connection is established
    async fn ensure_openclaw_connection(&self) -> Result<()> {
        let mut conn_guard = self.openclaw_conn.write().await;

        if conn_guard.is_none() {
            let gateway_url = std::env::var("OPENCLAW_GATEWAY_URL")
                .unwrap_or_else(|_| "ws://127.0.0.1:18789".to_string());

            let conn = OpenClawConnection::new(gateway_url).await?;

            // Perform handshake
            conn.handshake().await?;

            *conn_guard = Some(Arc::new(conn));
        }

        Ok(())
    }

    /// Get OpenClaw connection (if available)
    async fn get_openclaw_connection(&self) -> Result<Arc<OpenClawConnection>> {
        let conn_guard = self.openclaw_conn.read().await;
        match &*conn_guard {
            Some(conn) => Ok(conn.clone()),
            None => {
                drop(conn_guard);
                self.ensure_openclaw_connection().await?;
                let conn_guard = self.openclaw_conn.read().await;
                conn_guard.as_ref().cloned().ok_or_else(|| {
                    AmosError::Internal("Failed to establish connection".to_string())
                })
            }
        }
    }

    /// Register a new agent
    pub async fn register_agent(
        &self,
        name: String,
        display_name: String,
        role: String,
        capabilities: Vec<String>,
        system_prompt: Option<String>,
        model: Option<String>,
    ) -> Result<AgentConfig> {
        let model = model.unwrap_or_else(|| "claude-3-5-sonnet".to_string());

        let capabilities_json = serde_json::to_value(&capabilities)
            .map_err(|e| AmosError::Internal(format!("Failed to serialize capabilities: {}", e)))?;

        let row = sqlx::query(
            r#"
            INSERT INTO openclaw_agents (name, display_name, role, capabilities, system_prompt, model, status, trust_level)
            VALUES ($1, $2, $3, $4, $5, $6, 'registered', 0)
            RETURNING id
            "#,
        )
        .bind(&name)
        .bind(&display_name)
        .bind(&role)
        .bind(&capabilities_json)
        .bind(&system_prompt)
        .bind(&model)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to register agent: {}", e)))?;

        let agent_id: i32 = row.get(0);

        Ok(AgentConfig {
            agent_id,
            name,
            display_name,
            role,
            capabilities,
            system_prompt,
            model,
            provider_type: None,
            api_base: None,
            max_concurrent_tasks: None,
            always_on: None,
            cost_tier: None,
            task_specializations: None,
        })
    }

    /// Activate an agent (connect to OpenClaw gateway)
    pub async fn activate_agent(&self, agent_id: i32) -> Result<()> {
        let config = self.get_agent_config(agent_id).await?;

        // Update status to active
        self.update_agent_status(agent_id, AgentStatus::Active)
            .await?;

        // If gateway is available, register on the gateway
        if let Ok(conn) = self.get_openclaw_connection().await {
            match conn.register_agent(&config).await {
                Ok(gateway_id) => {
                    info!(
                        "Registered agent {} on OpenClaw gateway as {}",
                        agent_id, gateway_id
                    );
                    self.gateway_agent_ids
                        .write()
                        .await
                        .insert(agent_id, gateway_id);
                }
                Err(e) => {
                    warn!(
                        "Could not register agent on gateway (will work locally): {}",
                        e
                    );
                }
            }
        }

        self.active_agents
            .write()
            .await
            .insert(agent_id, AgentStatus::Active);
        info!("Agent {} ({}) activated", config.display_name, agent_id);

        Ok(())
    }

    /// Stop an agent
    pub async fn stop_agent(&self, agent_id: i32) -> Result<()> {
        // Stop on gateway if connected
        if let Some(gateway_id) = self.gateway_agent_ids.write().await.remove(&agent_id) {
            if let Ok(conn) = self.get_openclaw_connection().await {
                let _ = conn.stop_gateway_agent(&gateway_id).await;
            }
        }

        // Update database
        self.update_agent_status(agent_id, AgentStatus::Stopped)
            .await?;

        // Remove from active agents
        self.active_agents.write().await.remove(&agent_id);

        info!("Agent {} stopped", agent_id);
        Ok(())
    }

    /// Get agent status
    pub async fn get_status(&self, agent_id: i32) -> Result<AgentStatus> {
        let agents = self.active_agents.read().await;
        Ok(agents
            .get(&agent_id)
            .cloned()
            .unwrap_or(AgentStatus::Stopped))
    }

    /// List all agents
    pub async fn list_agents(&self) -> Result<Vec<AgentConfig>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, display_name, role, capabilities, system_prompt, model
            FROM openclaw_agents
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to list agents: {}", e)))?;

        let mut agents = Vec::new();
        for row in rows {
            let agent_id: i32 = row.get(0);
            let name: String = row.get(1);
            let display_name: String = row.get(2);
            let role: String = row.get(3);
            let capabilities_json: JsonValue = row.get(4);
            let system_prompt: Option<String> = row.get(5);
            let model: String = row.get(6);

            let capabilities: Vec<String> =
                serde_json::from_value(capabilities_json).map_err(|e| {
                    AmosError::Internal(format!("Failed to deserialize capabilities: {}", e))
                })?;

            agents.push(AgentConfig {
                agent_id,
                name,
                display_name,
                role,
                capabilities,
                system_prompt,
                model,
                provider_type: None,
                api_base: None,
                max_concurrent_tasks: None,
                always_on: None,
                cost_tier: None,
                task_specializations: None,
            });
        }

        Ok(agents)
    }

    /// Update agent configuration
    pub async fn update_agent(
        &self,
        agent_id: i32,
        updates: AgentConfigUpdate,
    ) -> Result<AgentConfig> {
        let mut query_parts = Vec::new();
        let mut bind_values: Vec<JsonValue> = Vec::new();

        if let Some(capabilities) = &updates.capabilities {
            query_parts.push(format!("capabilities = ${}", query_parts.len() + 1));
            bind_values.push(
                serde_json::to_value(capabilities)
                    .map_err(|e| AmosError::Internal(format!("Failed to serialize: {}", e)))?,
            );
        }

        if let Some(system_prompt) = &updates.system_prompt {
            query_parts.push(format!("system_prompt = ${}", query_parts.len() + 1));
            bind_values.push(
                serde_json::to_value(system_prompt)
                    .map_err(|e| AmosError::Internal(format!("Failed to serialize: {}", e)))?,
            );
        }

        if let Some(model) = &updates.model {
            query_parts.push(format!("model = ${}", query_parts.len() + 1));
            bind_values.push(
                serde_json::to_value(model)
                    .map_err(|e| AmosError::Internal(format!("Failed to serialize: {}", e)))?,
            );
        }

        if let Some(role) = &updates.role {
            query_parts.push(format!("role = ${}", query_parts.len() + 1));
            bind_values.push(
                serde_json::to_value(role)
                    .map_err(|e| AmosError::Internal(format!("Failed to serialize: {}", e)))?,
            );
        }

        if query_parts.is_empty() {
            return Err(AmosError::Validation("No updates provided".to_string()));
        }

        let query = format!(
            "UPDATE openclaw_agents SET {} WHERE id = ${} RETURNING id, name, display_name, role, capabilities, system_prompt, model",
            query_parts.join(", "),
            query_parts.len() + 1
        );

        let mut query_builder = sqlx::query(&query);
        for value in bind_values {
            query_builder = query_builder.bind(value);
        }
        query_builder = query_builder.bind(agent_id);

        let row = query_builder
            .fetch_one(&self.db_pool)
            .await
            .map_err(|e| AmosError::Internal(format!("Failed to update agent: {}", e)))?;

        let agent_id: i32 = row.get(0);
        let name: String = row.get(1);
        let display_name: String = row.get(2);
        let role: String = row.get(3);
        let capabilities_json: JsonValue = row.get(4);
        let system_prompt: Option<String> = row.get(5);
        let model: String = row.get(6);

        let capabilities: Vec<String> = serde_json::from_value(capabilities_json)
            .map_err(|e| AmosError::Internal(format!("Failed to deserialize: {}", e)))?;

        Ok(AgentConfig {
            agent_id,
            name,
            display_name,
            role,
            capabilities,
            system_prompt,
            model,
            provider_type: None,
            api_base: None,
            max_concurrent_tasks: None,
            always_on: None,
            cost_tier: None,
            task_specializations: None,
        })
    }

    /// Get agent configuration by ID
    async fn get_agent_config(&self, agent_id: i32) -> Result<AgentConfig> {
        let row = sqlx::query(
            r#"
            SELECT id, name, display_name, role, capabilities, system_prompt, model
            FROM openclaw_agents
            WHERE id = $1
            "#,
        )
        .bind(agent_id)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| AmosError::NotFound {
            entity: "Agent".to_string(),
            id: e.to_string(),
        })?;

        let agent_id: i32 = row.get(0);
        let name: String = row.get(1);
        let display_name: String = row.get(2);
        let role: String = row.get(3);
        let capabilities_json: JsonValue = row.get(4);
        let system_prompt: Option<String> = row.get(5);
        let model: String = row.get(6);

        let capabilities: Vec<String> = serde_json::from_value(capabilities_json)
            .map_err(|e| AmosError::Internal(format!("Failed to deserialize: {}", e)))?;

        Ok(AgentConfig {
            agent_id,
            name,
            display_name,
            role,
            capabilities,
            system_prompt,
            model,
            provider_type: None,
            api_base: None,
            max_concurrent_tasks: None,
            always_on: None,
            cost_tier: None,
            task_specializations: None,
        })
    }

    /// Update agent status in database
    async fn update_agent_status(&self, agent_id: i32, status: AgentStatus) -> Result<()> {
        sqlx::query("UPDATE openclaw_agents SET status = $1 WHERE id = $2")
            .bind(status.to_string())
            .bind(agent_id)
            .execute(&self.db_pool)
            .await
            .map_err(|e| AmosError::Internal(format!("Failed to update agent status: {}", e)))?;

        Ok(())
    }
}

/// Agent configuration update
#[derive(Debug, Default)]
pub struct AgentConfigUpdate {
    pub role: Option<String>,
    pub capabilities: Option<Vec<String>>,
    pub system_prompt: Option<String>,
    pub model: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_status_display() {
        assert_eq!(AgentStatus::Registered.to_string(), "registered");
        assert_eq!(AgentStatus::Active.to_string(), "active");
        assert_eq!(AgentStatus::Working.to_string(), "working");
        assert_eq!(AgentStatus::Idle.to_string(), "idle");
        assert_eq!(AgentStatus::Stopped.to_string(), "stopped");
        assert_eq!(AgentStatus::Error.to_string(), "error");
    }

    #[test]
    fn agent_status_serde_roundtrip() {
        let status = AgentStatus::Working;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"working\"");
        let deserialized: AgentStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, status);
    }

    #[test]
    fn agent_config_serde_roundtrip() {
        let config = AgentConfig {
            agent_id: 42,
            name: "research-bot".to_string(),
            display_name: "Research Agent".to_string(),
            role: "Performs deep research on topics".to_string(),
            capabilities: vec!["web_search".to_string(), "shell".to_string()],
            system_prompt: Some("You are a research agent.".to_string()),
            model: "claude-3-5-sonnet".to_string(),
            provider_type: None,
            api_base: None,
            max_concurrent_tasks: None,
            always_on: None,
            cost_tier: None,
            task_specializations: None,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AgentConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.agent_id, 42);
        assert_eq!(deserialized.name, "research-bot");
        assert_eq!(deserialized.display_name, "Research Agent");
        assert_eq!(deserialized.role, "Performs deep research on topics");
        assert_eq!(deserialized.capabilities.len(), 2);
        assert_eq!(deserialized.capabilities[0], "web_search");
        assert_eq!(deserialized.model, "claude-3-5-sonnet");
    }

    #[test]
    fn agent_config_minimal_serde() {
        let config = AgentConfig {
            agent_id: 1,
            name: "worker".to_string(),
            display_name: "Worker".to_string(),
            role: "General tasks".to_string(),
            capabilities: vec![],
            system_prompt: None,
            model: "gpt-4o".to_string(),
            provider_type: None,
            api_base: None,
            max_concurrent_tasks: None,
            always_on: None,
            cost_tier: None,
            task_specializations: None,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AgentConfig = serde_json::from_str(&json).unwrap();
        assert!(deserialized.system_prompt.is_none());
        assert!(deserialized.capabilities.is_empty());
    }

    #[test]
    fn agent_status_equality() {
        assert_eq!(AgentStatus::Active, AgentStatus::Active);
        assert_ne!(AgentStatus::Active, AgentStatus::Stopped);
    }

    #[test]
    fn agent_config_update_default() {
        let update = AgentConfigUpdate::default();
        assert!(update.role.is_none());
        assert!(update.capabilities.is_none());
        assert!(update.system_prompt.is_none());
        assert!(update.model.is_none());
    }
}
