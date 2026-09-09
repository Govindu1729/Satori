//! Screen capture module using macOS CoreGraphics (CGWindowListCreateImage)
//! Provides synchronous capture with JPEG encoding.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(target_os = "macos")]
use core_graphics::{
    window::{CGWindow, WindowListOption},
    image::CGImage,
    base::{kCGNullWindowID, kCGWindowListOptionOnScreenOnly, kCGWindowListOptionIncludingWindow},
};
#[cfg(target_os = "macos")]
use image::{ImageBuffer, Rgba};
use std::io::Cursor;

/// Screen capture configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureConfig {
    pub window_id: Option<u32>,
    pub fps: u32,
    pub format: ImageFormat,
    pub quality: u8,
    pub target_app: Option<String>,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            window_id: None,
            fps: 60,
            format: ImageFormat::Jpeg,
            quality: 85,
            target_app: Some("Zen".to_string()),
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
    pub timestamp: u64,
    pub width: u32,
    pub height: u32,
    pub data: String,
    pub window_title: Option<String>,
    pub app_name: Option<String>,
}

/// Available capture source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureSource {
    pub id: String,
    pub name: String,
    pub source_type: CaptureSourceType,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum CaptureSourceType {
    Display,
    Window,
}

/// macOS screen capture manager using CoreGraphics
#[cfg(target_os = "macos")]
pub struct ScreenCaptureManager {
    config: Option<CaptureConfig>,
    is_capturing: bool,
    current_source_id: Option<String>,
}

#[cfg(target_os = "macos")]
impl ScreenCaptureManager {
    pub fn new() -> Self {
        Self {
            config: None,
            is_capturing: false,
            current_source_id: None,
        }
    }

    /// Check permission – macOS TCC
    pub async fn check_permission() -> bool {
        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGPreflightScreenCaptureAccess() -> bool;
        }
        unsafe { CGPreflightScreenCaptureAccess() }
    }

    /// Request permission – will show system dialog
    pub async fn request_permission() -> bool {
        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGRequestScreenCaptureAccess();
        }
        unsafe { CGRequestScreenCaptureAccess() };
        true
    }

    /// Enumerate windows
    pub async fn enumerate_sources(&mut self) -> Result<Vec<CaptureSource>> {
        let windows = CGWindow::windows(WindowListOption::OptionOnScreenOnly, kCGNullWindowID)
            .map_err(|e| anyhow::anyhow!("Failed to list windows: {:?}", e))?;

        let mut sources = Vec::new();
        for win in windows {
            if let Some(title) = win.title() {
                let owner = win.owner_name().unwrap_or_else(|| "Unknown".to_string());
                sources.push(CaptureSource {
                    id: format!("window-{}", win.id()),
                    name: title,
                    source_type: CaptureSourceType::Window,
                    width: Some(win.bounds().size.width as u32),
                    height: Some(win.bounds().size.height as u32),
                    app_name: Some(owner),
                    bundle_id: None,
                });
            }
        }
        Ok(sources)
    }

    /// Find windows for an application
    pub async fn find_application_windows(&self, app_name: &str) -> Vec<CaptureSource> {
        let windows = CGWindow::windows(WindowListOption::OptionOnScreenOnly, kCGNullWindowID)
            .unwrap_or_default();
        windows
            .into_iter()
            .filter_map(|win| {
                let owner = win.owner_name().unwrap_or_default();
                if owner.to_lowercase().contains(&app_name.to_lowercase()) {
                    Some(CaptureSource {
                        id: format!("window-{}", win.id()),
                        name: win.title().unwrap_or_else(|| "Untitled".to_string()),
                        source_type: CaptureSourceType::Window,
                        width: Some(win.bounds().size.width as u32),
                        height: Some(win.bounds().size.height as u32),
                        app_name: Some(owner),
                        bundle_id: None,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    pub async fn find_zen_windows(&self) -> Vec<CaptureSource> {
        self.find_application_windows("Zen").await
    }

    /// Configure stream
    pub async fn configure_stream(&mut self, window_id: u32, width: u32, height: u32) -> Result<()> {
        self.config = Some(CaptureConfig {
            window_id: Some(window_id),
            fps: 60,
            format: ImageFormat::Jpeg,
            quality: 85,
            target_app: Some("Zen".to_string()),
        });
        self.current_source_id = Some(format!("window-{}", window_id));
        self.is_capturing = true;
        Ok(())
    }

    /// Capture a single frame using CGWindowListCreateImage
    pub async fn capture_frame(&mut self) -> Result<Option<CapturedFrame>> {
        if !self.is_capturing {
            return Ok(None);
        }

        let window_id = self.config
            .as_ref()
            .and_then(|c| c.window_id)
            .unwrap_or(0);

        let cg_image = CGImage::window_list_create(
            WindowListOption::OptionIncludingWindow,
            window_id,
            core_graphics::base::kCGWindowImageDefault,
        ).map_err(|e| anyhow::anyhow!("Failed to capture window: {:?}", e))?;

        let width = cg_image.width() as u32;
        let height = cg_image.height() as u32;

        let img = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(width, height);
        let mut jpeg_data = Vec::new();
        let mut cursor = Cursor::new(&mut jpeg_data);
        image::codecs::jpeg::JpegEncoder::new(&mut cursor)
            .encode_image(&image::DynamicImage::ImageRgba8(img))
            .map_err(|e| anyhow::anyhow!("JPEG encoding failed: {}", e))?;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let frame = CapturedFrame {
            timestamp,
            width,
            height,
            data: base64::encode(&jpeg_data),
            window_title: None,
            app_name: self.config.as_ref().and_then(|c| c.target_app.clone()),
        };

        Ok(Some(frame))
    }

    /// Capture frame as JPEG
    pub async fn capture_frame_jpeg(&mut self) -> Result<Vec<u8>> {
        if let Some(frame) = self.capture_frame().await? {
            let bytes = base64::decode(&frame.data)
                .context("Failed to decode base64 frame")?;
            Ok(bytes)
        } else {
            anyhow::bail!("No frame captured")
        }
    }

    pub fn stop_capture(&mut self) -> Result<()> {
        self.is_capturing = false;
        self.current_source_id = None;
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
pub struct ScreenCaptureManager;

#[cfg(not(target_os = "macos"))]
impl ScreenCaptureManager {
    pub fn new() -> Self { Self }
    pub async fn check_permission() -> bool { false }
    pub async fn request_permission() -> bool { false }
    pub async fn enumerate_sources(&mut self) -> Result<Vec<CaptureSource>> { Ok(vec![]) }
    pub async fn find_application_windows(&self, _: &str) -> Vec<CaptureSource> { vec![] }
    pub async fn find_zen_windows(&self) -> Vec<CaptureSource> { vec![] }
    pub async fn configure_stream(&mut self, _: u32, _: u32, _: u32) -> Result<()> { Ok(()) }
    pub async fn capture_frame(&mut self) -> Result<Option<CapturedFrame>> { Ok(None) }
    pub async fn capture_frame_jpeg(&mut self) -> Result<Vec<u8>> { anyhow::bail!("Unsupported platform") }
    pub fn stop_capture(&mut self) -> Result<()> { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_config_default() {
        let config = CaptureConfig::default();
        assert_eq!(config.fps, 60);
        assert_eq!(config.quality, 85);
        assert_eq!(config.target_app, Some("Zen".to_string()));
    }

    #[test]
    fn test_capture_source_serialization() {
        let source = CaptureSource {
            id: "window-123".to_string(),
            name: "Zen Browser".to_string(),
            source_type: CaptureSourceType::Window,
            width: Some(1920),
            height: Some(1080),
            app_name: Some("Zen".to_string()),
            bundle_id: Some("app.zen.browser".to_string()),
        };
        let json = serde_json::to_string(&source).unwrap();
        assert!(json.contains("Zen Browser"));
        assert!(json.contains("window-123"));
    }
}
