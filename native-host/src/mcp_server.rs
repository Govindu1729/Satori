//! Model Context Protocol (MCP) Server Implementation
//! Provides standardized tools for AI agents to interact with the browser

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

/// MCP Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    #[serde(default)]
    pub read_only_hint: bool,
}

/// MCP Resource definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPResource {
    pub uri: String,
    pub name: String,
    pub description: String,
    pub mime_type: String,
}

/// MCP Tool Call Request
#[derive(Debug, Clone, Deserialize)]
pub struct MCPToolCall {
    pub name: String,
    pub arguments: HashMap<String, Value>,
}

/// MCP Tool Call Response
#[derive(Debug, Clone, Serialize)]
pub struct MCPToolResponse {
    pub content: Vec<MCPContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// MCP Content types
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum MCPContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
    #[serde(rename = "resource")]
    Resource { uri: String, mime_type: String, data: Value },
}

/// MCP Server state
pub struct MCPServer {
    tools: HashMap<String, MCPTool>,
    resources: HashMap<String, MCPResource>,
}

impl MCPServer {
    /// Create a new MCP server with all tools registered
    pub fn new() -> Self {
        let mut server = Self {
            tools: HashMap::new(),
            resources: HashMap::new(),
        };

        // Register all MCP tools
        server.register_tools();
        server
    }

    /// Register all available MCP tools
    fn register_tools(&mut self) {
        // Tool 1: take_snapshot
        self.tools.insert(
            "take_snapshot".to_string(),
            MCPTool {
                name: "take_snapshot".to_string(),
                description: "Capture the current DOM state with bounding boxes and UIDs for all interactive elements. Returns a structured representation of the page suitable for AI analysis.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "include_images": {
                            "type": "boolean",
                            "description": "Whether to include base64-encoded screenshots of visible elements",
                            "default": false
                        },
                        "max_depth": {
                            "type": "integer",
                            "description": "Maximum DOM depth to traverse",
                            "default": 10
                        }
                    }
                }),
                read_only_hint: true,
            },
        );

        // Tool 2: click_by_uid
        self.tools.insert(
            "click_by_uid".to_string(),
            MCPTool {
                name: "click_by_uid".to_string(),
                description: "Simulate a native click on an element by its unique identifier (UID). Bypasses CSS overlays and works even on complex layouts.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "uid": {
                            "type": "string",
                            "description": "The unique identifier of the element to click"
                        },
                        "button": {
                            "type": "string",
                            "enum": ["left", "middle", "right"],
                            "default": "left",
                            "description": "Mouse button to simulate"
                        }
                    },
                    "required": ["uid"]
                }),
                read_only_hint: false,
            },
        );

        // Tool 3: evaluate_script
        self.tools.insert(
            "evaluate_script".to_string(),
            MCPTool {
                name: "evaluate_script".to_string(),
                description: "Execute arbitrary JavaScript in the context of the active browser tab. Requires explicit user confirmation due to security implications.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "script": {
                            "type": "string",
                            "description": "JavaScript code to execute"
                        },
                        "await_promise": {
                            "type": "boolean",
                            "default": false,
                            "description": "Whether to await Promise resolution before returning"
                        }
                    },
                    "required": ["script"]
                }),
                read_only_hint: false,
            },
        );

        // Tool 4: list_network_requests
        self.tools.insert(
            "list_network_requests".to_string(),
            MCPTool {
                name: "list_network_requests".to_string(),
                description: "List all network requests (XHR/Fetch) made by the current page. Useful for analyzing API traffic and responses.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "filter_url": {
                            "type": "string",
                            "description": "Filter requests by URL substring"
                        },
                        "filter_method": {
                            "type": "string",
                            "enum": ["GET", "POST", "PUT", "DELETE", "PATCH"],
                            "description": "Filter requests by HTTP method"
                        },
                        "include_body": {
                            "type": "boolean",
                            "default": false,
                            "description": "Include request/response bodies (may be large)"
                        }
                    }
                }),
                read_only_hint: true,
            },
        );

        // Tool 5: get_screen_capture
        self.tools.insert(
            "get_screen_capture".to_string(),
            MCPTool {
                name: "get_screen_capture".to_string(),
                description: "Capture the current browser viewport as an image using OS-level screen capture. Provides exact visual context as seen by the user.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "format": {
                            "type": "string",
                            "enum": ["jpeg", "webp", "png"],
                            "default": "jpeg",
                            "description": "Image format for the capture"
                        },
                        "quality": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 100,
                            "default": 85,
                            "description": "Image quality (for lossy formats)"
                        },
                        "target_window": {
                            "type": "string",
                            "description": "Specific window title to capture (defaults to active Zen window)"
                        }
                    }
                }),
                read_only_hint: true,
            },
        );

        // Tool 6: extract_page_content
        self.tools.insert(
            "extract_page_content".to_string(),
            MCPTool {
                name: "extract_page_content".to_string(),
                description: "Extract and analyze the main text content of the current page. Performs NLP processing including sentiment analysis, entity extraction, and summarization.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "include_nlp": {
                            "type": "boolean",
                            "default": true,
                            "description": "Whether to perform NLP analysis (sentiment, entities, summary)"
                        },
                        "max_length": {
                            "type": "integer",
                            "default": 5000,
                            "description": "Maximum characters to extract"
                        },
                        "selectors": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "CSS selectors to target specific content areas"
                        }
                    }
                }),
                read_only_hint: true,
            },
        );

        // Register resources
        self.resources.insert(
            "browser://current-tab".to_string(),
            MCPResource {
                uri: "browser://current-tab".to_string(),
                name: "Current Tab".to_string(),
                description: "The currently active browser tab".to_string(),
                mime_type: "text/html".to_string(),
            },
        );
    }

    /// Get list of all registered tools
    pub fn list_tools(&self) -> Vec<MCPTool> {
        self.tools.values().cloned().collect()
    }

    /// Get list of all registered resources
    pub fn list_resources(&self) -> Vec<MCPResource> {
        self.resources.values().cloned().collect()
    }

    /// Execute a tool call
    pub async fn execute_tool(
        &self,
        tool_call: &MCPToolCall,
        dom_state: &Value,
        network_log: &Value,
    ) -> Result<MCPToolResponse, String> {
        match tool_call.name.as_str() {
            "take_snapshot" => self.execute_take_snapshot(&tool_call.arguments, dom_state),
            "click_by_uid" => self.execute_click_by_uid(&tool_call.arguments),
            "evaluate_script" => self.execute_evaluate_script(&tool_call.arguments),
            "list_network_requests" => self.execute_list_network_requests(&tool_call.arguments, network_log),
            "get_screen_capture" => self.execute_get_screen_capture(&tool_call.arguments).await,
            "extract_page_content" => self.execute_extract_page_content(&tool_call.arguments, dom_state).await,
            _ => Err(format!("Unknown tool: {}", tool_call.name)),
        }
    }

    /// Execute take_snapshot tool
    fn execute_take_snapshot(
        &self,
        args: &HashMap<String, Value>,
        dom_state: &Value,
    ) -> Result<MCPToolResponse, String> {
        let include_images = args.get("include_images")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let max_depth = args.get("max_depth")
            .and_then(|v| v.as_i64())
            .unwrap_or(10) as usize;

        // Extract DOM snapshot from the provided state
        let snapshot = json!({
            "url": dom_state.get("url").unwrap_or(&Value::Null),
            "title": dom_state.get("title").unwrap_or(&Value::Null),
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "elements": self.extract_interactive_elements(dom_state, max_depth, include_images),
            "metadata": {
                "total_elements": dom_state.get("element_count").unwrap_or(&Value::Null),
                "depth": max_depth,
                "images_included": include_images
            }
        });

        Ok(MCPToolResponse {
            content: vec![MCPContent::Text {
                text: serde_json::to_string_pretty(&snapshot).unwrap_or_default()
            }],
            error: None,
        })
    }

    /// Extract interactive elements from DOM
    fn extract_interactive_elements(
        &self,
        dom_state: &Value,
        max_depth: usize,
        include_images: bool,
    ) -> Value {
        // In actual implementation, traverse the DOM AST and extract:
        // - Buttons, links, inputs, selects, textareas
        // - Calculate bounding boxes
        // - Assign UIDs
        // - Optionally capture element screenshots

        dom_state.get("elements").cloned().unwrap_or_else(|| json!([]))
    }

    /// Execute click_by_uid tool
    fn execute_click_by_uid(
        &self,
        args: &HashMap<String, Value>,
    ) -> Result<MCPToolResponse, String> {
        let uid = args.get("uid")
            .and_then(|v| v.as_str())
            .ok_or("Missing required argument: uid")?;

        let button = args.get("button")
            .and_then(|v| v.as_str())
            .unwrap_or("left");

        // In actual implementation, send click command to WebDriver BiDi
        let result = json!({
            "action": "click",
            "uid": uid,
            "button": button,
            "success": true,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        Ok(MCPToolResponse {
            content: vec![MCPContent::Text {
                text: serde_json::to_string_pretty(&result).unwrap_or_default()
            }],
            error: None,
        })
    }

    /// Execute evaluate_script tool
    fn execute_evaluate_script(
        &self,
        args: &HashMap<String, Value>,
    ) -> Result<MCPToolResponse, String> {
        let script = args.get("script")
            .and_then(|v| v.as_str())
            .ok_or("Missing required argument: script")?;

        let await_promise = args.get("await_promise")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // SECURITY NOTE: This tool requires human-in-the-loop confirmation
        // The MCP client should prompt the user before executing

        let result = json!({
            "action": "evaluate_script",
            "script_preview": if script.len() > 100 { &script[..100] } else { script },
            "await_promise": await_promise,
            "warning": "Script execution requires user confirmation",
            "status": "pending_user_approval"
        });

        Ok(MCPToolResponse {
            content: vec![MCPContent::Text {
                text: serde_json::to_string_pretty(&result).unwrap_or_default()
            }],
            error: None,
        })
    }

    /// Execute list_network_requests tool
    fn execute_list_network_requests(
        &self,
        args: &HashMap<String, Value>,
        network_log: &Value,
    ) -> Result<MCPToolResponse, String> {
        let filter_url = args.get("filter_url").and_then(|v| v.as_str());
        let filter_method = args.get("filter_method").and_then(|v| v.as_str());
        let include_body = args.get("include_body").and_then(|v| v.as_bool()).unwrap_or(false);

        // Filter network log based on arguments
        let requests = network_log.get("requests")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let filtered: Vec<Value> = requests
            .into_iter()
            .filter(|req| {
                if let Some(url_filter) = filter_url {
                    if let Some(url) = req.get("url").and_then(|v| v.as_str()) {
                        if !url.contains(url_filter) {
                            return false;
                        }
                    }
                }
                if let Some(method_filter) = filter_method {
                    if let Some(method) = req.get("method").and_then(|v| v.as_str()) {
                        if method != method_filter {
                            return false;
                        }
                    }
                }
                true
            })
            .map(|mut req| {
                if !include_body {
                    if let Some(obj) = req.as_object_mut() {
                        obj.remove("request_body");
                        obj.remove("response_body");
                    }
                }
                req
            })
            .collect();

        let result = json!({
            "total_requests": filtered.len(),
            "filters_applied": {
                "url": filter_url,
                "method": filter_method,
                "include_body": include_body
            },
            "requests": filtered
        });

        Ok(MCPToolResponse {
            content: vec![MCPContent::Text {
                text: serde_json::to_string_pretty(&result).unwrap_or_default()
            }],
            error: None,
        })
    }

    /// Execute get_screen_capture tool
    async fn execute_get_screen_capture(
        &self,
        args: &HashMap<String, Value>,
    ) -> Result<MCPToolResponse, String> {
        let format = args.get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("jpeg");

        let quality = args.get("quality")
            .and_then(|v| v.as_i64())
            .unwrap_or(85) as u8;

        let target_window = args.get("target_window").and_then(|v| v.as_str());

        // In actual implementation, call ScreenCaptureKit
        // For now, return a placeholder response
        let result = json!({
            "action": "screen_capture",
            "format": format,
            "quality": quality,
            "target_window": target_window,
            "status": "capture_initiated",
            "note": "Screen capture will be implemented with ScreenCaptureKit in Phase 2"
        });

        Ok(MCPToolResponse {
            content: vec![MCPContent::Text {
                text: serde_json::to_string_pretty(&result).unwrap_or_default()
            }],
            error: None,
        })
    }

    /// Execute extract_page_content tool
    async fn execute_extract_page_content(
        &self,
        args: &HashMap<String, Value>,
        dom_state: &Value,
    ) -> Result<MCPToolResponse, String> {
        let include_nlp = args.get("include_nlp")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let max_length = args.get("max_length")
            .and_then(|v| v.as_i64())
            .unwrap_or(5000) as usize;

        let selectors = args.get("selectors")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_default();

        // Extract text content from DOM
        let text_content = dom_state.get("text_content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .chars()
            .take(max_length)
            .collect::<String>();

        let mut result_data = json!({
            "url": dom_state.get("url").unwrap_or(&Value::Null),
            "title": dom_state.get("title").unwrap_or(&Value::Null),
            "content_length": text_content.len(),
            "content": text_content,
            "selectors_used": selectors,
        });

        // Add NLP analysis if requested
        if include_nlp {
            // In actual implementation, call Python NLP processor
            // For now, return placeholder analysis
            result_data["nlp_analysis"] = json!({
                "sentiment": "neutral",
                "entities": [],
                "summary": "NLP analysis will be performed by ml-backend/nlp_processor.py",
                "keywords": []
            });
        }

        Ok(MCPToolResponse {
            content: vec![MCPContent::Text {
                text: serde_json::to_string_pretty(&result_data).unwrap_or_default()
            }],
            error: None,
        })
    }

    /// Generate MCP server manifest for Claude Desktop
    pub fn generate_manifest(&self) -> Value {
        json!({
            "mcp_version": "1.0",
            "server_name": "zen-agentic-mcp",
            "description": "MCP server for Zen Browser agentic AI integration",
            "tools": self.list_tools().iter().map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.input_schema,
                    "read_only_hint": t.read_only_hint
                })
            }).collect::<Vec<_>>(),
            "resources": self.list_resources().iter().map(|r| {
                json!({
                    "uri": r.uri,
                    "name": r.name,
                    "description": r.description,
                    "mime_type": r.mime_type
                })
            }).collect::<Vec<_>>()
        })
    }
}

impl Default for MCPServer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_server_initialization() {
        let server = MCPServer::new();
        assert_eq!(server.list_tools().len(), 6);
        assert_eq!(server.list_resources().len(), 1);
    }

    #[test]
    fn test_tool_registration() {
        let server = MCPServer::new();
        let tools = server.list_tools();

        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(tool_names.contains(&"take_snapshot"));
        assert!(tool_names.contains(&"click_by_uid"));
        assert!(tool_names.contains(&"evaluate_script"));
        assert!(tool_names.contains(&"list_network_requests"));
        assert!(tool_names.contains(&"get_screen_capture"));
        assert!(tool_names.contains(&"extract_page_content"));
    }

    #[tokio::test]
    async fn test_take_snapshot() {
        let server = MCPServer::new();
        let dom_state = json!({
            "url": "https://example.com",
            "title": "Example Domain",
            "elements": [
                {"tag": "button", "uid": "btn1", "text": "Click me"}
            ],
            "element_count": 42
        });

        let call = MCPToolCall {
            name: "take_snapshot".to_string(),
            arguments: HashMap::from([
                ("include_images".to_string(), json!(false)),
                ("max_depth".to_string(), json!(5)),
            ]),
        };

        let response = server.execute_tool(&call, &dom_state, &json!({})).await.unwrap();
        assert!(response.error.is_none());
        assert!(!response.content.is_empty());
    }

    #[tokio::test]
    async fn test_click_by_uid() {
        let server = MCPServer::new();

        let call = MCPToolCall {
            name: "click_by_uid".to_string(),
            arguments: HashMap::from([
                ("uid".to_string(), json!("btn123")),
                ("button".to_string(), json!("left")),
            ]),
        };

        let response = server.execute_tool(&call, &json!({}), &json!({})).await.unwrap();
        assert!(response.error.is_none());
    }

    #[tokio::test]
    async fn test_list_network_requests() {
        let server = MCPServer::new();
        let network_log = json!({
            "requests": [
                {"url": "https://api.example.com/data", "method": "GET", "status": 200},
                {"url": "https://api.example.com/submit", "method": "POST", "status": 201}
            ]
        });

        let call = MCPToolCall {
            name: "list_network_requests".to_string(),
            arguments: HashMap::from([
                ("filter_method".to_string(), json!("GET")),
                ("include_body".to_string(), json!(false)),
            ]),
        };

        let response = server.execute_tool(&call, &json!({}), &network_log).await.unwrap();
        assert!(response.error.is_none());
    }

    #[test]
    fn test_manifest_generation() {
        let server = MCPServer::new();
        let manifest = server.generate_manifest();

        assert_eq!(manifest["server_name"], "zen-agentic-mcp");
        assert!(manifest["tools"].is_array());
        assert!(manifest["resources"].is_array());
    }
}
