//! Native messaging host main entry point
//!
//! Implements the WebExtensions Native Messaging protocol:
//! - Spawns as subprocess from browser extension
//! - Communicates via stdin/stdout with 32-bit length-prefixed JSON
//! - Routes messages to appropriate subsystems (MCP, ScreenCapture, WebDriver)

mod ipc;
mod mcp_server;
mod screen_capture_kit;
mod webdriver_bidi;
mod ml_bridge;

use anyhow::{Context, Result};
use std::io::{self, Read, Write};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, error, warn, debug};

/// Application state shared across subsystems
struct AppState {
    /// MCP server instance
    mcp_server: mcp_server::MCPServer,
    /// Screen capture engine (macOS ScreenCaptureKit)
    screen_capture: screen_capture_kit::ScreenCaptureManager,
    /// WebDriver client for DOM automation
    webdriver: webdriver_bidi::WebDriverSession,
    /// ML Bridge for Python NLP backend
    ml_bridge: ml_bridge::MlBridge,
    /// Whether screen capture permission has been granted
    screen_capture_allowed: bool,
}

impl AppState {
    fn new() -> Self {
        let ml_config = ml_bridge::MlBridgeConfig::default();
        let mut ml_bridge = ml_bridge::MlBridge::new(ml_config);

        // Try to initialize ML bridge, but don't fail if it's not available
        if let Err(e) = ml_bridge.initialize() {
            warn!("ML Bridge initialization failed (will retry on first use): {}", e);
        }

        Self {
            mcp_server: mcp_server::MCPServer::new(),
            screen_capture: screen_capture_kit::ScreenCaptureManager::new(),
            webdriver: webdriver_bidi::WebDriverSession::new(),
            ml_bridge,
            screen_capture_allowed: false,
        }
    }
}

/// Main message handler
async fn handle_message(
    state: Arc<Mutex<AppState>>,
    message: ipc::NativeMessage,
) -> Result<ipc::NativeMessage> {
    let state = state.lock().await;

    match message.method.as_str() {
        // Basic ping/pong for connection testing
        "ping" => Ok(ipc::NativeMessage::response(
            message.id,
            serde_json::json!({ "pong": true }),
        )),

        // Initialize screen capture subsystem
        "init_screen_capture" => {
            drop(state); // Release lock before initialization
            let mut state_locked = state.lock().await;

            // Check permission first
            let has_permission = screen_capture_kit::ScreenCaptureManager::check_permission().await;
            state_locked.screen_capture_allowed = has_permission;

            Ok(ipc::NativeMessage::response(
                message.id,
                serde_json::json!({
                    "initialized": true,
                    "has_permission": has_permission,
                    "platform": if cfg!(target_os = "macos") { "macos" }
                               else if cfg!(target_os = "windows") { "windows" }
                               else { "linux" },
                }),
            ))
        }

        // Check screen capture permission status
        "check_screen_capture_permission" => {
            let has_permission = screen_capture_kit::ScreenCaptureManager::check_permission().await;
            Ok(ipc::NativeMessage::response(
                message.id,
                serde_json::json!({ "has_permission": has_permission }),
            ))
        }

        // Request screen capture permission (shows platform-specific instructions)
        "request_screen_capture_permission" => {
            let granted = screen_capture_kit::ScreenCaptureManager::request_permission().await;
            Ok(ipc::NativeMessage::response(
                message.id,
                serde_json::json!({ "granted": granted }),
            ))
        }

        // Enumerate available capture sources
        "enumerate_capture_sources" => {
            let mut manager = screen_capture_kit::ScreenCaptureManager::new();
            match manager.enumerate_sources().await {
                Ok(sources) => Ok(ipc::NativeMessage::response(
                    message.id,
                    serde_json::to_value(sources)?,
                )),
                Err(e) => Ok(ipc::NativeMessage::error_response(
                    message.id,
                    -32001,
                    e.to_string(),
                )),
            }
        }

        // Find windows for a specific application (Zen Browser)
        "find_application_windows" => {
            let params = message.params.as_object()
                .ok_or_else(|| anyhow::anyhow!("Invalid params"))?;

            let app_name = params.get("app_name")
                .and_then(|v| v.as_str())
                .unwrap_or("Zen Browser");

            let manager = screen_capture_kit::ScreenCaptureManager::new();
            let windows = manager.find_application_windows(app_name).await;

            Ok(ipc::NativeMessage::response(
                message.id,
                serde_json::to_value(windows)?,
            ))
        }

        // Find Zen browser windows specifically
        "find_zen_windows" => {
            let manager = screen_capture_kit::ScreenCaptureManager::new();
            let windows = manager.find_zen_windows().await;

            Ok(ipc::NativeMessage::response(
                message.id,
                serde_json::to_value(windows)?,
            ))
        }

        // Configure stream for capturing
        "configure_stream" => {
            let params = message.params.as_object()
                .ok_or_else(|| anyhow::anyhow!("Invalid params"))?;

            let window_id = params.get("window_id")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| anyhow::anyhow!("Missing window_id"))? as u32;

            let width = params.get("width")
                .and_then(|v| v.as_u64())
                .unwrap_or(1920) as u32;

            let height = params.get("height")
                .and_then(|v| v.as_u64())
                .unwrap_or(1080) as u32;

            drop(state);
            let mut state_locked = state.lock().await;

            match state_locked.screen_capture.configure_stream(window_id, width, height).await {
                Ok(_) => Ok(ipc::NativeMessage::response(
                    message.id,
                    serde_json::json!({ "configured": true }),
                )),
                Err(e) => Ok(ipc::NativeMessage::error_response(
                    message.id,
                    -32002,
                    e.to_string(),
                )),
            }
        }

        // Capture a single frame
        "capture_frame" => {
            let state_locked = state.lock().await;

            match state_locked.screen_capture.capture_frame().await {
                Ok(frame) => Ok(ipc::NativeMessage::response(
                    message.id,
                    serde_json::to_value(frame)?,
                )),
                Err(e) => Ok(ipc::NativeMessage::error_response(
                    message.id,
                    -32004,
                    e.to_string(),
                )),
            }
        }

        // Capture frame as JPEG
        "capture_frame_jpeg" => {
            let state_locked = state.lock().await;

            match state_locked.screen_capture.capture_frame_jpeg().await {
                Ok(jpeg_data) => {
                    use base64::{Engine as _, engine::general_purpose};
                    let base64_data = general_purpose::STANDARD.encode(&jpeg_data);

                    Ok(ipc::NativeMessage::response(
                        message.id,
                        serde_json::json!({
                            "format": "jpeg",
                            "size": jpeg_data.len(),
                            "data": base64_data
                        }),
                    ))
                },
                Err(e) => Ok(ipc::NativeMessage::error_response(
                    message.id,
                    -32004,
                    e.to_string(),
                )),
            }
        }

        // Stop screen capture
        "stop_capture" => {
            Ok(ipc::NativeMessage::response(
                message.id,
                serde_json::json!({ "stopped": true }),
            ))
        }

        // List available MCP tools
        "list_mcp_tools" => {
            let tools = state.mcp_server.list_tools();
            Ok(ipc::NativeMessage::response(
                message.id,
                serde_json::to_value(tools)?,
            ))
        }

        // Call an MCP tool
        "call_mcp_tool" => {
            let params = message.params.as_object()
                .ok_or_else(|| anyhow::anyhow!("Invalid params"))?;

            let tool_name = params.get("tool_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing tool_name"))?;

            let arguments = params.get("arguments")
                .and_then(|v| v.as_object())
                .cloned()
                .unwrap_or(serde_json::Map::new());

            // Convert arguments to HashMap<String, Value>
            let args_map: std::collections::HashMap<String, serde_json::Value> =
                arguments.into_iter().collect();

            let tool_call = mcp_server::MCPToolCall {
                name: tool_name.to_string(),
                arguments: args_map,
            };

            // Execute the tool with empty DOM and network state for now
            let dom_state = serde_json::json!({});
            let network_log = serde_json::json!({});

            match state.mcp_server.execute_tool(&tool_call, &dom_state, &network_log).await {
                Ok(response) => Ok(ipc::NativeMessage::response(
                    message.id,
                    serde_json::to_value(response)?,
                )),
                Err(e) => Ok(ipc::NativeMessage::error_response(
                    message.id,
                    -32602,
                    e.to_string(),
                )),
            }
        }

        // Connect to browser via WebDriver BiDi
        "connect_webdriver" => {
            let params = message.params.as_object()
                .ok_or_else(|| anyhow::anyhow!("Invalid params"))?;

            let port = params.get("port")
                .and_then(|v| v.as_u64())
                .unwrap_or(2828) as u16;

            drop(state);
            let mut state_locked = state.lock().await;

            state_locked.webdriver = webdriver_bidi::WebDriverSession::with_port(port);

            match state_locked.webdriver.connect().await {
                Ok(_) => Ok(ipc::NativeMessage::response(
                    message.id,
                    serde_json::json!({ "connected": true, "port": port }),
                )),
                Err(e) => Ok(ipc::NativeMessage::error_response(
                    message.id,
                    -32010,
                    e.to_string(),
                )),
            }
        }

        // Take DOM snapshot via WebDriver
        "take_dom_snapshot" => {
            let state_locked = state.lock().await;

            if !state_locked.webdriver.is_connected() {
                return Ok(ipc::NativeMessage::error_response(
                    message.id,
                    -32011,
                    "WebDriver not connected. Call connect_webdriver first.".to_string(),
                ));
            }

            match state_locked.webdriver.take_snapshot().await {
                Ok(snapshot) => Ok(ipc::NativeMessage::response(
                    message.id,
                    snapshot,
                )),
                Err(e) => Ok(ipc::NativeMessage::error_response(
                    message.id,
                    -32012,
                    e.to_string(),
                )),
            }
        }

        // Click element by UID via WebDriver
        "webdriver_click" => {
            let params = message.params.as_object()
                .ok_or_else(|| anyhow::anyhow!("Invalid params"))?;

            let uid = params.get("uid")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing uid"))?;

            let state_locked = state.lock().await;

            if !state_locked.webdriver.is_connected() {
                return Ok(ipc::NativeMessage::error_response(
                    message.id,
                    -32011,
                    "WebDriver not connected".to_string(),
                ));
            }

            match state_locked.webdriver.click_by_uid(uid).await {
                Ok(_) => Ok(ipc::NativeMessage::response(
                    message.id,
                    serde_json::json!({ "clicked": true, "uid": uid }),
                )),
                Err(e) => Ok(ipc::NativeMessage::error_response(
                    message.id,
                    -32012,
                    e.to_string(),
                )),
            }
        }

        // Execute script via WebDriver
        "webdriver_execute_script" => {
            let params = message.params.as_object()
                .ok_or_else(|| anyhow::anyhow!("Invalid params"))?;

            let script = params.get("script")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing script"))?;

            let await_promise = params.get("await_promise")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let state_locked = state.lock().await;

            if !state_locked.webdriver.is_connected() {
                return Ok(ipc::NativeMessage::error_response(
                    message.id,
                    -32011,
                    "WebDriver not connected".to_string(),
                ));
            }

            match state_locked.webdriver.execute_script(script, await_promise).await {
                Ok(result) => Ok(ipc::NativeMessage::response(
                    message.id,
                    result,
                )),
                Err(e) => Ok(ipc::NativeMessage::error_response(
                    message.id,
                    -32012,
                    e.to_string(),
                )),
            }
        }

        // Get version info
        "get_version" => {
            Ok(ipc::NativeMessage::response(
                message.id,
                serde_json::json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "rust_version": rustc_version_runtime::version(),
                    "target": std::env::consts::OS,
                }),
            ))
        }

        // ML Bridge: Sentiment Analysis
        "ml_sentiment_analysis" => {
            let params = message.params.as_object()
                .ok_or_else(|| anyhow::anyhow!("Invalid params"))?;

            let text = params.get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing text"))?;

            let mut state_locked = state.lock().await;

            match state_locked.ml_bridge.analyze_sentiment(text) {
                Ok(result) => Ok(ipc::NativeMessage::response(
                    message.id,
                    result,
                )),
                Err(e) => Ok(ipc::NativeMessage::error_response(
                    message.id,
                    -32020,
                    e.to_string(),
                )),
            }
        }

        // ML Bridge: Entity Extraction
        "ml_entity_extraction" => {
            let params = message.params.as_object()
                .ok_or_else(|| anyhow::anyhow!("Invalid params"))?;

            let text = params.get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing text"))?;

            let mut state_locked = state.lock().await;

            match state_locked.ml_bridge.extract_entities(text) {
                Ok(result) => Ok(ipc::NativeMessage::response(
                    message.id,
                    result,
                )),
                Err(e) => Ok(ipc::NativeMessage::error_response(
                    message.id,
                    -32021,
                    e.to_string(),
                )),
            }
        }

        // ML Bridge: Summarization
        "ml_summarize" => {
            let params = message.params.as_object()
                .ok_or_else(|| anyhow::anyhow!("Invalid params"))?;

            let text = params.get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing text"))?;

            let max_length = params.get("max_length")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);

            let mut state_locked = state.lock().await;

            match state_locked.ml_bridge.summarize(text, max_length) {
                Ok(result) => Ok(ipc::NativeMessage::response(
                    message.id,
                    result,
                )),
                Err(e) => Ok(ipc::NativeMessage::error_response(
                    message.id,
                    -32022,
                    e.to_string(),
                )),
            }
        }

        // ML Bridge: Keyword Extraction
        "ml_extract_keywords" => {
            let params = message.params.as_object()
                .ok_or_else(|| anyhow::anyhow!("Invalid params"))?;

            let text = params.get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing text"))?;

            let top_k = params.get("top_k")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);

            let mut state_locked = state.lock().await;

            match state_locked.ml_bridge.extract_keywords(text, top_k) {
                Ok(result) => Ok(ipc::NativeMessage::response(
                    message.id,
                    result,
                )),
                Err(e) => Ok(ipc::NativeMessage::error_response(
                    message.id,
                    -32023,
                    e.to_string(),
                )),
            }
        }

        // ML Bridge: Generate Embedding
        "ml_generate_embedding" => {
            let params = message.params.as_object()
                .ok_or_else(|| anyhow::anyhow!("Invalid params"))?;

            let text = params.get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing text"))?;

            let mut state_locked = state.lock().await;

            match state_locked.ml_bridge.generate_embedding(text) {
                Ok(result) => Ok(ipc::NativeMessage::response(
                    message.id,
                    result,
                )),
                Err(e) => Ok(ipc::NativeMessage::error_response(
                    message.id,
                    -32024,
                    e.to_string(),
                )),
            }
        }

        // ML Bridge: RAG Query
        "ml_rag_query" => {
            let params = message.params.as_object()
                .ok_or_else(|| anyhow::anyhow!("Invalid params"))?;

            let query = params.get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing query"))?;

            let top_k = params.get("top_k")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);

            let context = params.get("context")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(String::from).collect());

            let mut state_locked = state.lock().await;

            match state_locked.ml_bridge.rag_query(query, top_k, context) {
                Ok(result) => Ok(ipc::NativeMessage::response(
                    message.id,
                    result,
                )),
                Err(e) => Ok(ipc::NativeMessage::error_response(
                    message.id,
                    -32025,
                    e.to_string(),
                )),
            }
        }

        // ML Bridge: RAG Store Documents
        "ml_rag_store" => {
            let params = message.params.as_object()
                .ok_or_else(|| anyhow::anyhow!("Invalid params"))?;

            let documents = params.get("documents")
                .and_then(|v| v.as_array())
                .ok_or_else(|| anyhow::anyhow!("Missing documents"))?;

            let rag_docs: Result<Vec<ml_bridge::RagDocument>, _> = documents.iter().map(|doc| {
                let id = doc.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let content = doc.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let metadata = doc.get("metadata")
                    .and_then(|v| v.as_object())
                    .map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect())
                    .unwrap_or_default();
                let timestamp = doc.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);

                Ok(ml_bridge::RagDocument {
                    id,
                    content,
                    metadata,
                    timestamp,
                })
            }).collect();

            match rag_docs {
                Ok(docs) => {
                    let mut state_locked = state.lock().await;

                    match state_locked.ml_bridge.rag_store(docs) {
                        Ok(result) => Ok(ipc::NativeMessage::response(
                            message.id,
                            result,
                        )),
                        Err(e) => Ok(ipc::NativeMessage::error_response(
                            message.id,
                            -32026,
                            e.to_string(),
                        )),
                    }
                },
                Err(e) => Ok(ipc::NativeMessage::error_response(
                    message.id,
                    -32027,
                    e.to_string(),
                )),
            }
        }

        // ML Bridge: Health Check
        "ml_health_check" => {
            let mut state_locked = state.lock().await;
            let is_healthy = state_locked.ml_bridge.is_running() && state_locked.ml_bridge.heartbeat();

            Ok(ipc::NativeMessage::response(
                message.id,
                serde_json::json!({
                    "healthy": is_healthy,
                    "running": state_locked.ml_bridge.is_running(),
                }),
            ))
        }

        // Unknown method
        _ => Ok(ipc::NativeMessage::error_response(
            message.id,
            -32601,
            format!("Method not found: {}", message.method),
        )),
    }
}

/// Main message loop
async fn run_message_loop() -> Result<()> {
    info!("Zen Agentic Native Host starting...");
    info!("Platform: {}", if cfg!(target_os = "macos") { "macOS" }
                          else if cfg!(target_os = "windows") { "Windows" }
                          else { "Linux/Other" });

    let state = Arc::new(Mutex::new(AppState::new()));
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let mut stdin_reader = stdin;
    let mut stdout_writer = stdout;

    info!("Waiting for messages from browser extension...");

    loop {
        match ipc::read_message_async(&mut stdin_reader).await {
            Ok(Some(message)) => {
                debug!("Received message: {} - {}", message.id, message.method);

                match handle_message(Arc::clone(&state), message).await {
                    Ok(response) => {
                        if let Err(e) = ipc::write_message_async(&mut stdout_writer, &response).await {
                            error!("Failed to write response: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Failed to handle message: {}", e);
                        let error_response = ipc::NativeMessage::error_response(
                            "unknown",
                            -32603,
                            e.to_string(),
                        );
                        let _ = ipc::write_message_async(&mut stdout_writer, &error_response).await;
                    }
                }
            }
            Ok(None) => {
                info!("EOF received from browser, shutting down...");
                break;
            }
            Err(e) => {
                error!("Failed to read message: {}", e);
                break;
            }
        }
    }

    info!("Native host shutting down...");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("zen_agentic_native=debug".parse().unwrap()),
        )
        .with_writer(std::io::stderr)
        .init();

    info!("=== Zen Agentic Native Host ===");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    if let Err(e) = run_message_loop().await {
        error!("Fatal error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}

// Add runtime version dependency info
mod rustc_version_runtime {
    pub fn version() -> String {
        format!("{}", ::rustc_version_runtime::version())
    }
}
