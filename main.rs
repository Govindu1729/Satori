//! Native messaging host main entry point
//!
//! Implements the WebExtensions Native Messaging protocol:
//! - Spawns as subprocess from browser extension
//! - Communicates via stdin/stdout with 32-bit length-prefixed JSON
//! - Routes messages to appropriate subsystems (MCP, ScreenCapture, WebDriver)

mod ipc;
mod mcp;
mod screencapture;
mod webdriver;

use anyhow::{Context, Result};
use std::io::{self, Read, Write};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, error, warn, debug};

/// Application state shared across subsystems
struct AppState {
    /// MCP server instance
    mcp_server: mcp::MCPServer,
    /// Screen capture engine (platform-specific)
    screen_capture: Box<dyn screencapture::ScreenCaptureEngine>,
    /// WebDriver client for DOM automation
    webdriver: webdriver::FirefoxWebDriver,
    /// Whether screen capture permission has been granted
    screen_capture_allowed: bool,
}

impl AppState {
    fn new() -> Self {
        Self {
            mcp_server: mcp::MCPServer::new(),
            screen_capture: screencapture::create_capture_engine(),
            webdriver: webdriver::FirefoxWebDriver::new(),
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
            state_locked.screen_capture.initialize()
                .context("Failed to initialize screen capture")?;

            let has_permission = state_locked.screen_capture.has_permission();
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
            let has_permission = state.screen_capture.has_permission();
            Ok(ipc::NativeMessage::response(
                message.id,
                serde_json::json!({ "has_permission": has_permission }),
            ))
        }

        // Request screen capture permission (shows platform-specific instructions)
        "request_screen_capture_permission" => {
            match state.screen_capture.request_permission() {
                Ok(_) => Ok(ipc::NativeMessage::response(
                    message.id,
                    serde_json::json!({ "granted": true }),
                )),
                Err(e) => Ok(ipc::NativeMessage::error_response(
                    message.id,
                    -32000,
                    e.to_string(),
                )),
            }
        }

        // Enumerate available capture sources
        "enumerate_capture_sources" => {
            match state.screen_capture.enumerate_sources() {
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

        // Start capturing a specific source
        "start_capture" => {
            let params = message.params.as_object()
                .ok_or_else(|| anyhow::anyhow!("Invalid params"))?;

            let source_id = params.get("source_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing source_id"))?;

            let config = screencapture::CaptureConfig::default();

            match state.screen_capture.start_capture(source_id, config) {
                Ok(_) => Ok(ipc::NativeMessage::response(
                    message.id,
                    serde_json::json!({ "started": true }),
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
            // Note: This is blocking - in production, use async stream
            todo!("Implement async frame streaming")
        }

        // Stop screen capture
        "stop_capture" => {
            match state.screen_capture.stop_capture() {
                Ok(_) => Ok(ipc::NativeMessage::response(
                    message.id,
                    serde_json::json!({ "stopped": true }),
                )),
                Err(e) => Ok(ipc::NativeMessage::error_response(
                    message.id,
                    -32003,
                    e.to_string(),
                )),
            }
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
                .cloned()
                .unwrap_or(serde_json::Value::Null);

            let request = state.mcp_server.create_tool_call(tool_name, arguments);

            // In production, this would be awaited properly
            // For now, return pending response
            Ok(ipc::NativeMessage::response(
                message.id,
                serde_json::json!({
                    "request_id": request.request_id,
                    "status": "pending",
                    "tool": tool_name,
                }),
            ))
        }

        // Connect to browser via WebDriver
        "connect_webdriver" => {
            let params = message.params.as_object()
                .ok_or_else(|| anyhow::anyhow!("Invalid params"))?;

            let port = params.get("port")
                .and_then(|v| v.as_u64())
                .unwrap_or(2828) as u16;

            let config = webdriver::WebDriverConfig {
                port,
                ..Default::default()
            };

            // Clone state arc to avoid holding lock during connect
            drop(state);
            let mut state_locked = state.lock().await;

            match state_locked.webdriver.connect(&config) {
                Ok(_) => Ok(ipc::NativeMessage::response(
                    message.id,
                    serde_json::json!({ "connected": true }),
                )),
                Err(e) => Ok(ipc::NativeMessage::error_response(
                    message.id,
                    -32010,
                    e.to_string(),
                )),
            }
        }

        // Take DOM snapshot
        "take_dom_snapshot" => {
            let max_depth = message.params.get("max_depth")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);

            // In production, implement actual snapshot
            Ok(ipc::NativeMessage::response(
                message.id,
                serde_json::json!({
                    "status": "not_implemented",
                    "message": "DOM snapshot via WebDriver not yet implemented",
                }),
            ))
        }

        // Get version info
        "get_version" => {
            Ok(ipc::NativeMessage::response(
                message.id,
                serde_json::json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "rust_version": rustc_version_runtime::version(),
                    "target": env!("TARGET"),
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
        format!("{}", rustc_version_runtime::version())
    }
}
