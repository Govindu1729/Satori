// FIXED: Use correct import
use screen_capture_kit::ScreenCaptureEngine;  // Not ScreenCaptureManager

// FIXED: AppState with correct types
struct AppState {
    mcp_server: mcp_server::MCPServer,
    screen_capture: Box<dyn screen_capture_kit::ScreenCaptureEngine>,  // Trait object
    webdriver: webdriver_bidi::FirefoxWebDriver,  // Use concrete type
    ml_bridge: ml_bridge::MlBridge,
    screen_capture_allowed: bool,
}

impl AppState {
    fn new() -> Self {
        let ml_config = ml_bridge::MlBridgeConfig::default();
        let ml_bridge = ml_bridge::MlBridge::new(ml_config);

        // FIXED: Remove initialize() call - handle gracefully
        // The bridge will initialize on first use

        Self {
            mcp_server: mcp_server::MCPServer::new(),
            screen_capture: screen_capture_kit::create_capture_engine(),  // Use factory
            webdriver: webdriver_bidi::FirefoxWebDriver::new(),
            ml_bridge,
            screen_capture_allowed: false,
        }
    }
}

// FIXED: Variable shadowing pattern
async fn handle_message(
    state: Arc<Mutex<AppState>>,
    message: ipc::NativeMessage,
) -> Result<ipc::NativeMessage> {
    // FIXED: Don't shadow - use guard pattern
    let state_guard = state.lock().await;

    // For methods that need to drop the lock temporarily:
    // FIXED: Use drop() and re-acquire with new guard
    drop(state_guard);  // Release lock
    let mut state_guard = state.lock().await;  // Re-acquire
    // ... use state_guard ...

    // For all other occurrences:
    // Instead of: let state = state.lock().await;
    // Use: let state_guard = state.lock().await;
    // And: let mut state_guard = state.lock().await;
}
