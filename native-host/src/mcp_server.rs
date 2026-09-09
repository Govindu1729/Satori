//! Model Context Protocol (MCP) Server Implementation
//! Provides standardized tools for AI agents to interact with the browser

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use chrono::Utc;

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
    pub fn new() -> Self {
        let mut server = Self {
            tools: HashMap::new(),
            resources: HashMap::new(),
        };
        server.register_tools();
        server
    }

    fn register_tools(&mut self) {
        self.tools.insert(
            "take_snapshot".to_string(),
            MCPTool {
                name: "take_snapshot".to_string(),
                description: "Capture the current DOM state with bounding boxes and UIDs for all interactive elements.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "include_images": {
                            "type": "boolean",
                            "description": "Whether to include base64-encoded screenshots",
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

        self.tools.insert(
            "click_by_uid".to_string(),
            MCPTool {
                name: "click_by_uid".to_string(),
                description: "Simulate a native click on an element by its unique identifier (UID).".to_string(),
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
                            "default": "left"
                        }
                    },
                    "required": ["uid"]
                }),
                read_only_hint: false,
            },
        );

        self.tools.insert(
            "evaluate_script".to_string(),
            MCPTool {
                name: "evaluate_script".to_string(),
                description: "Execute arbitrary JavaScript in the context of the active browser tab.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "script": {
                            "type": "string",
                            "description": "JavaScript code to execute"
                        },
                        "await_promise": {
                            "type": "boolean",
                            "default": false
                        }
                    },
                    "required": ["script"]
                }),
                read_only_hint: false,
            },
        );

        self.tools.insert(
            "list_network_requests".to_string(),
            MCPTool {
                name: "list_network_requests".to_string(),
                description: "List all network requests (XHR/Fetch) made by the current page.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "filter_url": {
                            "type": "string",
                            "description": "Filter requests by URL substring"
                        },
                        "filter_method": {
                            "type": "string",
                            "enum": ["GET", "POST", "PUT", "DELETE", "PATCH"]
                        },
                        "include_body": {
                            "type": "boolean",
                            "default": false
                        }
                    }
                }),
                read_only_hint: true,
            },
        );

        self.tools.insert(
            "get_screen_capture".to_string(),
            MCPTool {
                name: "get_screen_capture".to_string(),
                description: "Capture the current browser viewport as an image using OS-level screen capture.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "format": {
                            "type": "string",
                            "enum": ["jpeg", "webp", "png"],
                            "default": "jpeg"
                        },
                        "quality": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 100,
                            "default": 85
                        }
                    }
                }),
                read_only_hint: true,
            },
        );

        self.tools.insert(
            "extract_page_content".to_string(),
            MCPTool {
                name: "extract_page_content".to_string(),
                description: "Extract and analyze the main text content of the current page.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "include_nlp": {
                            "type": "boolean",
                            "default": true
                        },
                        "max_length": {
                            "type": "integer",
                            "default": 5000
                        }
                    }
                }),
                read_only_hint: true,
            },
        );

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

    pub fn list_tools(&self) -> Vec<MCPTool> {
        self.tools.values().cloned().collect()
    }

    pub fn list_resources(&self) -> Vec<MCPResource> {
        self.resources.values().cloned().collect()
    }

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

    fn execute_take_snapshot(
        &self,
        args: &HashMap<String, Value>,
        dom_state: &Value,
    ) -> Result<MCPToolResponse, String> {
        let include_images = args.get("include_images")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let _max_depth = args.get("max_depth")
            .and_then(|v| v.as_i64())
            .unwrap_or(10) as usize;

        let snapshot = json!({
            "url": dom_state.get("url").unwrap_or(&Value::Null),
            "title": dom_state.get("title").unwrap_or(&Value::Null),
            "timestamp": Utc::now().to_rfc3339(),
            "elements": dom_state.get("elements").unwrap_or(&Value::Null),
            "metadata": {
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

        let result = json!({
            "action": "click",
            "uid": uid,
            "button": button,
            "success": true,
            "timestamp": Utc::now().to_rfc3339()
        });

        Ok(MCPToolResponse {
            content: vec![MCPContent::Text {
                text: serde_json::to_string_pretty(&result).unwrap_or_default()
            }],
            error: None,
        })
    }

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

        let result = json!({
            "action": "evaluate_script",
            "script_preview": if script.len() > 100 { &script[..100] } else { script },
            "await_promise": await_promise,
            "status": "pending_user_approval"
        });

        Ok(MCPToolResponse {
            content: vec![MCPContent::Text {
                text: serde_json::to_string_pretty(&result).unwrap_or_default()
            }],
            error: None,
        })
    }

    fn execute_list_network_requests(
        &self,
        args: &HashMap<String, Value>,
        network_log: &Value,
    ) -> Result<MCPToolResponse, String> {
        let filter_url = args.get("filter_url").and_then(|v| v.as_str());
        let filter_method = args.get("filter_method").and_then(|v| v.as_str());
        let include_body = args.get("include_body").and_then(|v| v.as_bool()).unwrap_or(false);

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

    async fn execute_get_screen_capture(
        &self,
        args: &HashMap<String, Value>,
    ) -> Result<MCPToolResponse, String> {
        let _format = args.get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("jpeg");

        let _quality = args.get("quality")
            .and_then(|v| v.as_i64())
            .unwrap_or(85) as u8;

        let result = json!({
            "action": "screen_capture",
            "status": "capture_available",
            "note": "Use the capture_frame method via native messaging"
        });

        Ok(MCPToolResponse {
            content: vec![MCPContent::Text {
                text: serde_json::to_string_pretty(&result).unwrap_or_default()
            }],
            error: None,
        })
    }

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
        });

        if include_nlp {
            result_data["nlp_analysis"] = json!({
                "sentiment": "neutral",
                "entities": [],
                "summary": "NLP analysis available via ML Bridge",
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
            "elements": []
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

    #[test]
    fn test_manifest_generation() {
        let server = MCPServer::new();
        let manifest = server.generate_manifest();
        assert_eq!(manifest["server_name"], "zen-agentic-mcp");
        assert!(manifest["tools"].is_array());
        assert!(manifest["resources"].is_array());
    }
}
