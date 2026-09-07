--- zen-agentic-extension/native-host/src/screencapture/mod.rs (原始)


+++ zen-agentic-extension/native-host/src/screencapture/mod.rs (修改后)
//! Screen capture module using macOS ScreenCaptureKit
//!
//! Provides zero-copy frame capture via IOSurface and Metal
//! Target: 60 FPS with minimal CPU overhead (~1.9% single core)

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Screen capture configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureConfig {
    /// Target window ID (0 for full screen)
    pub window_id: Option<u32>,
    /// Frame rate target (default 60)
    pub fps: u32,
    /// Output format
    pub format: ImageFormat,
    /// Quality level (0-100)
    pub quality: u8,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            window_id: None,
            fps: 60,
            format: ImageFormat::Jpeg,
            quality: 85,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ImageFormat {
    Jpeg,
    Png,
    WebP,
}

/// Captured frame data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedFrame {
    /// Timestamp in milliseconds since epoch
    pub timestamp: u64,
    /// Frame width in pixels
    pub width: u32,
    /// Frame height in pixels
    pub height: u32,
    /// Encoded image data (base64)
    pub data: String,
    /// Source window title (if available)
    pub window_title: Option<String>,
}

/// Screen capture engine trait
pub trait ScreenCaptureEngine: Send + Sync {
    /// Initialize the capture engine
    fn initialize(&mut self) -> Result<()>;

    /// Check if screen capture permission is granted
    fn has_permission(&self) -> bool;

    /// Request screen capture permission (platform-specific)
    fn request_permission(&self) -> Result<()>;

    /// Enumerate available capture sources (windows/displays)
    fn enumerate_sources(&self) -> Result<Vec<CaptureSource>>;

    /// Start capturing a specific source
    fn start_capture(&mut self, source_id: &str, config: CaptureConfig) -> Result<()>;

    /// Stop current capture
    fn stop_capture(&mut self) -> Result<()>;

    /// Capture a single frame (blocking)
    fn capture_frame(&mut self) -> Result<Option<CapturedFrame>>;
}

/// Available capture source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureSource {
    /// Unique identifier for this source
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Source type
    pub source_type: CaptureSourceType,
    /// Window dimensions (if applicable)
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CaptureSourceType {
    Display,
    Window,
}

/// macOS ScreenCaptureKit implementation
#[cfg(target_os = "macos")]
pub struct MacOSScreenCapture {
    config: Option<CaptureConfig>,
    is_capturing: bool,
    stream: Option<*mut std::ffi::c_void>, // Opaque pointer to SCStream
}

#[cfg(target_os = "macos")]
impl MacOSScreenCapture {
    pub fn new() -> Self {
        Self {
            config: None,
            is_capturing: false,
            stream: None,
        }
    }

    /// Find Zen Browser windows specifically
    pub fn find_zen_windows(&self) -> Result<Vec<CaptureSource>> {
        // Implementation will use SCShareableContent to enumerate windows
        // Filter by application name "Zen" or bundle identifier
        todo!("Implement window enumeration using ScreenCaptureKit")
    }
}

#[cfg(target_os = "macos")]
impl ScreenCaptureEngine for MacOSScreenCapture {
    fn initialize(&mut self) -> Result<()> {
        // Initialize ScreenCaptureKit
        // This is a placeholder - actual implementation requires Objective-C bindings
        Ok(())
    }

    fn has_permission(&self) -> bool {
        // Check TCC permissions for screen recording
        // Returns true if user has granted permission in System Settings
        todo!("Check kCGScreenCaptureAccess authorization status")
    }

    fn request_permission(&self) -> Result<()> {
        // On macOS, we can't programmatically request permission
        // User must manually grant in System Settings → Privacy & Security
        anyhow::bail!(
            "Screen capture permission must be granted manually:\n\
             1. Open System Settings\n\
             2. Go to Privacy & Security → Screen & System Audio Recording\n\
             3. Enable zen-agentic-native"
        )
    }

    fn enumerate_sources(&self) -> Result<Vec<CaptureSource>> {
        // Use SCShareableContent to get all shareable windows and displays
        todo!("Implement source enumeration")
    }

    fn start_capture(&mut self, source_id: &str, config: CaptureConfig) -> Result<()> {
        self.config = Some(config);
        // Create SCStream with SCStreamConfiguration
        // Set up IOSurface for zero-copy frame delivery
        todo!("Implement stream creation")
    }

    fn stop_capture(&mut self) -> Result<()> {
        self.is_capturing = false;
        self.stream = None;
        Ok(())
    }

    fn capture_frame(&mut self) -> Result<Option<CapturedFrame>> {
        if !self.is_capturing {
            return Ok(None);
        }
        // Wait for frame callback from ScreenCaptureKit
        // Encode to JPEG/PNG using hardware acceleration
        todo!("Implement frame capture")
    }
}

/// Windows Desktop Duplication implementation (placeholder)
#[cfg(target_os = "windows")]
pub struct WindowsScreenCapture {
    // DXGI factory and device
    // Output duplication interface
}

#[cfg(target_os = "windows")]
impl ScreenCaptureEngine for WindowsScreenCapture {
    fn initialize(&mut self) -> Result<()> {
        todo!("Initialize DXGI and Desktop Duplication API")
    }

    fn has_permission(&self) -> bool {
        // Windows doesn't have TCC-like restrictions
        true
    }

    fn request_permission(&self) -> Result<()> {
        Ok(())
    }

    fn enumerate_sources(&self) -> Result<Vec<CaptureSource>> {
        todo!("Enumerate displays using DXGI")
    }

    fn start_capture(&mut self, source_id: &str, config: CaptureConfig) -> Result<()> {
        todo!("Start desktop duplication")
    }

    fn stop_capture(&mut self) -> Result<()> {
        Ok(())
    }

    fn capture_frame(&mut self) -> Result<Option<CapturedFrame>> {
        todo!("Acquire next frame from GPU")
    }
}

/// Linux implementation (not yet supported)
#[cfg(target_os = "linux")]
pub struct LinuxScreenCapture;

#[cfg(target_os = "linux")]
impl ScreenCaptureEngine for LinuxScreenCapture {
    fn initialize(&mut self) -> Result<()> {
        anyhow::bail!("Linux screen capture not yet implemented")
    }

    fn has_permission(&self) -> bool {
        false
    }

    fn request_permission(&self) -> Result<()> {
        anyhow::bail!("Linux screen capture not yet implemented")
    }

    fn enumerate_sources(&self) -> Result<Vec<CaptureSource>> {
        anyhow::bail!("Linux screen capture not yet implemented")
    }

    fn start_capture(&mut self, _source_id: &str, _config: CaptureConfig) -> Result<()> {
        anyhow::bail!("Linux screen capture not yet implemented")
    }

    fn stop_capture(&mut self) -> Result<()> {
        Ok(())
    }

    fn capture_frame(&mut self) -> Result<Option<CapturedFrame>> {
        anyhow::bail!("Linux screen capture not yet implemented")
    }
}

/// Factory function to create platform-appropriate capture engine
pub fn create_capture_engine() -> Box<dyn ScreenCaptureEngine> {
    #[cfg(target_os = "macos")]
    {
        Box::new(MacOSScreenCapture::new())
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(WindowsScreenCapture { /* init */ })
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(LinuxScreenCapture)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        panic!("Unsupported platform for screen capture")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_config_default() {
        let config = CaptureConfig::default();
        assert_eq!(config.fps, 60);
        assert_eq!(config.quality, 85);
    }

    #[test]
    fn test_capture_source_serialization() {
        let source = CaptureSource {
            id: "window-123".to_string(),
            name: "Zen Browser".to_string(),
            source_type: CaptureSourceType::Window,
            width: Some(1920),
            height: Some(1080),
        };
        let json = serde_json::to_string(&source).unwrap();
        assert!(json.contains("Zen Browser"));
    }
}
