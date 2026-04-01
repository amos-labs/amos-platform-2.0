//! MCP (Model Context Protocol) client.
//!
//! Connects to external MCP servers and registers their tools into the agent's
//! tool set. Supports stdio and HTTP transports.
//!
//! ## Configuration
//!
//! MCP servers are configured in `.amos/settings.json`:
//! ```json
//! {
//!   "mcpServers": {
//!     "my-server": {
//!       "type": "stdio",
//!       "command": "uvx",
//!       "args": ["my-mcp-server"],
//!       "env": {"API_KEY": "..."}
//!     },
//!     "remote-server": {
//!       "type": "http",
//!       "url": "https://mcp.example.com/v1"
//!     }
//!   }
//! }
//! ```
//!
//! Tool names are namespaced: `mcp__<server>__<tool>`

use amos_core::settings::McpServerEntry;
use amos_core::types::ToolDefinition;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tracing::{debug, info, warn};

/// An MCP tool discovered from a server.
#[derive(Debug, Clone)]
pub struct McpTool {
    /// Full namespaced name: mcp__<server>__<tool>
    pub full_name: String,
    /// Original tool name from the server.
    pub original_name: String,
    /// Server name this tool belongs to.
    pub server_name: String,
    /// Tool definition for the LLM.
    pub definition: ToolDefinition,
}

/// A connected MCP server (stdio transport).
pub struct McpStdioConnection {
    pub server_name: String,
    child: Child,
    request_id: u64,
}

/// MCP JSON-RPC request.
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
}

/// MCP JSON-RPC response.
#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: Option<u64>,
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    #[allow(dead_code)]
    code: i64,
    message: String,
}

/// MCP tool listing response.
#[derive(Debug, Deserialize)]
struct ToolsListResult {
    tools: Vec<McpToolDef>,
}

#[derive(Debug, Deserialize)]
struct McpToolDef {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default, rename = "inputSchema")]
    input_schema: Option<serde_json::Value>,
}

/// Normalize a name for use in tool identifiers (alphanumeric, _, -).
fn normalize_name(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Connect to an MCP stdio server and discover its tools.
pub async fn connect_stdio(
    server_name: &str,
    entry: &McpServerEntry,
) -> Result<(McpStdioConnection, Vec<McpTool>), String> {
    let command = entry
        .command
        .as_ref()
        .ok_or("stdio MCP server requires 'command' field")?;

    info!(server = server_name, command = %command, "Connecting to MCP stdio server");

    let mut cmd = Command::new(command);
    cmd.args(&entry.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for (key, val) in &entry.env {
        cmd.env(key, val);
    }

    let child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn MCP server '{server_name}': {e}"))?;

    let mut conn = McpStdioConnection {
        server_name: server_name.to_string(),
        child,
        request_id: 0,
    };

    // Initialize the connection
    let init_result = conn
        .send_request(
            "initialize",
            Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "amos-agent",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })),
        )
        .await?;

    debug!(server = server_name, result = ?init_result, "MCP initialized");

    // Send initialized notification
    conn.send_notification("notifications/initialized", None)
        .await?;

    // List tools
    let tools_result = conn.send_request("tools/list", None).await?;

    let tools = if let Some(result) = tools_result {
        let tools_list: ToolsListResult =
            serde_json::from_value(result).map_err(|e| format!("Failed to parse tools: {e}"))?;

        let normalized_server = normalize_name(server_name);

        tools_list
            .tools
            .into_iter()
            .map(|t| {
                let normalized_tool = normalize_name(&t.name);
                let full_name = format!("mcp__{normalized_server}__{normalized_tool}");
                McpTool {
                    full_name: full_name.clone(),
                    original_name: t.name.clone(),
                    server_name: server_name.to_string(),
                    definition: ToolDefinition {
                        name: full_name,
                        description: t.description.unwrap_or_else(|| {
                            format!("MCP tool '{}' from server '{}'", t.name, server_name)
                        }),
                        input_schema: t
                            .input_schema
                            .unwrap_or_else(|| json!({"type": "object", "properties": {}})),
                        requires_confirmation: false,
                        permission_level: amos_core::permissions::PermissionLevel::WorkspaceWrite,
                    },
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    info!(
        server = server_name,
        tool_count = tools.len(),
        "MCP server connected"
    );

    Ok((conn, tools))
}

impl McpStdioConnection {
    /// Send a JSON-RPC request and wait for the response.
    async fn send_request(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<Option<serde_json::Value>, String> {
        self.request_id += 1;
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: self.request_id,
            method: method.to_string(),
            params,
        };

        let mut request_json =
            serde_json::to_string(&request).map_err(|e| format!("Serialize error: {e}"))?;
        request_json.push('\n');

        let stdin = self
            .child
            .stdin
            .as_mut()
            .ok_or("MCP server stdin not available")?;
        stdin
            .write_all(request_json.as_bytes())
            .await
            .map_err(|e| format!("Failed to write to MCP server: {e}"))?;
        stdin
            .flush()
            .await
            .map_err(|e| format!("Failed to flush MCP server stdin: {e}"))?;

        // Read response
        let stdout = self
            .child
            .stdout
            .as_mut()
            .ok_or("MCP server stdout not available")?;
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();

        // Read with timeout
        let timeout = std::time::Duration::from_secs(30);
        tokio::time::timeout(timeout, reader.read_line(&mut line))
            .await
            .map_err(|_| "MCP server response timed out".to_string())?
            .map_err(|e| format!("Failed to read MCP response: {e}"))?;

        let response: JsonRpcResponse =
            serde_json::from_str(&line).map_err(|e| format!("Invalid MCP response: {e}"))?;

        if let Some(error) = response.error {
            return Err(format!("MCP error: {}", error.message));
        }

        Ok(response.result)
    }

    /// Send a JSON-RPC notification (no response expected).
    async fn send_notification(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<(), String> {
        #[derive(Serialize)]
        struct Notification {
            jsonrpc: String,
            method: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            params: Option<serde_json::Value>,
        }

        let notification = Notification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        };

        let mut json =
            serde_json::to_string(&notification).map_err(|e| format!("Serialize error: {e}"))?;
        json.push('\n');

        let stdin = self
            .child
            .stdin
            .as_mut()
            .ok_or("MCP server stdin not available")?;
        stdin
            .write_all(json.as_bytes())
            .await
            .map_err(|e| format!("Failed to write notification: {e}"))?;
        stdin
            .flush()
            .await
            .map_err(|e| format!("Failed to flush: {e}"))?;

        Ok(())
    }

    /// Execute a tool call on the MCP server.
    pub async fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<String, String> {
        let result = self
            .send_request(
                "tools/call",
                Some(json!({
                    "name": tool_name,
                    "arguments": arguments,
                })),
            )
            .await?;

        match result {
            Some(val) => {
                // Extract text content from the result
                if let Some(content) = val.get("content").and_then(|c| c.as_array()) {
                    let texts: Vec<&str> = content
                        .iter()
                        .filter_map(|block| {
                            if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                                block.get("text").and_then(|t| t.as_str())
                            } else {
                                None
                            }
                        })
                        .collect();
                    Ok(texts.join("\n"))
                } else {
                    Ok(serde_json::to_string(&val).unwrap_or_default())
                }
            }
            None => Ok("Tool returned no result".to_string()),
        }
    }

    /// Gracefully shut down the MCP server.
    pub async fn shutdown(&mut self) {
        let _ = self
            .send_notification("notifications/cancelled", None)
            .await;
        let _ = self.child.kill().await;
    }
}

impl Drop for McpStdioConnection {
    fn drop(&mut self) {
        // Best-effort kill on drop
        let _ = self.child.start_kill();
    }
}

/// Manager for all MCP connections.
pub struct McpManager {
    /// Active stdio connections.
    pub connections: HashMap<String, McpStdioConnection>,
    /// All discovered MCP tools (server_name → tools).
    pub tools: Vec<McpTool>,
}

impl McpManager {
    /// Create a new manager and connect to all configured MCP servers.
    pub async fn new(servers: &HashMap<String, McpServerEntry>) -> Self {
        let mut connections = HashMap::new();
        let mut all_tools = Vec::new();

        for (name, entry) in servers {
            match entry.transport.as_str() {
                "stdio" => match connect_stdio(name, entry).await {
                    Ok((conn, tools)) => {
                        all_tools.extend(tools);
                        connections.insert(name.clone(), conn);
                    }
                    Err(e) => {
                        warn!(server = name, error = %e, "Failed to connect MCP server");
                    }
                },
                "http" | "sse" => {
                    // HTTP transport: tools are called via HTTP, no persistent connection
                    info!(
                        server = name,
                        "HTTP MCP servers not yet supported (skipping)"
                    );
                }
                other => {
                    warn!(
                        server = name,
                        transport = other,
                        "Unknown MCP transport (skipping)"
                    );
                }
            }
        }

        Self {
            connections,
            tools: all_tools,
        }
    }

    /// Get tool definitions for the LLM.
    pub fn tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tools.iter().map(|t| t.definition.clone()).collect()
    }

    /// Execute an MCP tool by its full namespaced name.
    pub async fn execute_tool(
        &mut self,
        full_name: &str,
        input: serde_json::Value,
    ) -> Result<String, String> {
        // Find which server and original tool name
        let tool = self
            .tools
            .iter()
            .find(|t| t.full_name == full_name)
            .ok_or_else(|| format!("MCP tool not found: {full_name}"))?
            .clone();

        let conn = self
            .connections
            .get_mut(&tool.server_name)
            .ok_or_else(|| format!("MCP server not connected: {}", tool.server_name))?;

        conn.call_tool(&tool.original_name, input).await
    }

    /// Shut down all MCP connections.
    pub async fn shutdown(&mut self) {
        for (name, conn) in &mut self.connections {
            debug!(server = name, "Shutting down MCP connection");
            conn.shutdown().await;
        }
        self.connections.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_name() {
        assert_eq!(normalize_name("my-server"), "my-server");
        assert_eq!(normalize_name("my server"), "my_server");
        assert_eq!(normalize_name("test.tool"), "test_tool");
        assert_eq!(normalize_name("abc_123"), "abc_123");
    }

    #[test]
    fn test_tool_namespacing() {
        let server = "my-server";
        let tool = "get_data";
        let full = format!("mcp__{}__{}", normalize_name(server), normalize_name(tool));
        assert_eq!(full, "mcp__my-server__get_data");
    }
}
