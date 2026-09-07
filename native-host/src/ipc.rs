//! IPC module for 32-bit length-prefixed JSON messaging
//!
//! Implements the WebExtensions Native Messaging protocol:
//! - 4-byte little-endian length prefix
//! - UTF-8 encoded JSON payload
//! - Maximum message size: 1MB

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

/// Maximum message size as per WebExtensions standard (1 MB)
pub const MAX_MESSAGE_SIZE: u32 = 1024 * 1024;

/// Native message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeMessage {
    pub id: String,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<MessageError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageError {
    pub code: i32,
    pub message: String,
}

impl NativeMessage {
    /// Create a request message
    pub fn request(id: impl Into<String>, method: impl Into<String>, params: serde_json::Value) -> Self {
        Self {
            id: id.into(),
            method: method.into(),
            params,
            result: None,
            error: None,
        }
    }

    /// Create a response message
    pub fn response(id: impl Into<String>, result: serde_json::Value) -> Self {
        Self {
            id: id.into(),
            method: String::new(),
            params: serde_json::Value::Null,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response
    pub fn error_response(id: impl Into<String>, code: i32, message: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            method: String::new(),
            params: serde_json::Value::Null,
            result: None,
            error: Some(MessageError {
                code,
                message: message.into(),
            }),
        }
    }

    /// Serialize message to bytes with length prefix
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let json = serde_json::to_string(self)
            .context("Failed to serialize message to JSON")?;
        let payload = json.as_bytes();

        if payload.len() > MAX_MESSAGE_SIZE as usize {
            anyhow::bail!("Message size {} exceeds maximum {}", payload.len(), MAX_MESSAGE_SIZE);
        }

        let mut buffer = Vec::with_capacity(4 + payload.len());

        // Write 32-bit little-endian length prefix
        let len = payload.len() as u32;
        buffer.extend_from_slice(&len.to_le_bytes());
        buffer.extend_from_slice(payload);

        Ok(buffer)
    }

    /// Deserialize message from bytes (without length prefix)
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let json = std::str::from_utf8(data)
            .context("Invalid UTF-8 in message payload")?;
        let message: NativeMessage = serde_json::from_str(json)
            .context("Failed to deserialize JSON message")?;
        Ok(message)
    }
}

/// Read a single native message from stdin (blocking)
pub fn read_message_blocking<R: Read>(reader: &mut R) -> Result<Option<NativeMessage>> {
    // Read 4-byte length prefix
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf) {
        Ok(_) => {},
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e).context("Failed to read message length"),
    }

    let message_len = u32::from_le_bytes(len_buf);

    if message_len > MAX_MESSAGE_SIZE {
        anyhow::bail!("Message length {} exceeds maximum {}", message_len, MAX_MESSAGE_SIZE);
    }

    // Read payload
    let mut payload = vec![0u8; message_len as usize];
    reader.read_exact(&mut payload)
        .context("Failed to read message payload")?;

    NativeMessage::from_bytes(&payload).map(Some)
}

/// Write a native message to stdout (blocking)
pub fn write_message_blocking<W: Write>(writer: &mut W, message: &NativeMessage) -> Result<()> {
    let bytes = message.to_bytes()?;
    writer.write_all(&bytes)
        .context("Failed to write message to stdout")?;
    writer.flush().context("Failed to flush stdout")?;
    Ok(())
}

/// Async version: Read a single native message from stdin
pub async fn read_message_async<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<Option<NativeMessage>> {
    // Read 4-byte length prefix
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf).await {
        Ok(_) => {},
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e).context("Failed to read message length"),
    }

    let message_len = u32::from_le_bytes(len_buf);

    if message_len > MAX_MESSAGE_SIZE {
        anyhow::bail!("Message length {} exceeds maximum {}", message_len, MAX_MESSAGE_SIZE);
    }

    // Read payload
    let mut payload = vec![0u8; message_len as usize];
    reader.read_exact(&mut payload).await
        .context("Failed to read message payload")?;

    NativeMessage::from_bytes(&payload).map(Some)
}

/// Async version: Write a native message to stdout
pub async fn write_message_async<W: AsyncWriteExt + Unpin>(writer: &mut W, message: &NativeMessage) -> Result<()> {
    let bytes = message.to_bytes()?;
    writer.write_all(&bytes).await
        .context("Failed to write message to stdout")?;
    writer.flush().await.context("Failed to flush stdout")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let msg = NativeMessage::request("test-1", "ping", serde_json::json!({}));
        let bytes = msg.to_bytes().unwrap();

        // First 4 bytes should be length
        assert_eq!(bytes.len(), 4 + msg.to_bytes().unwrap().len() - 4);

        // Deserialize back
        let payload = &bytes[4..];
        let decoded = NativeMessage::from_bytes(payload).unwrap();
        assert_eq!(decoded.id, "test-1");
        assert_eq!(decoded.method, "ping");
    }

    #[test]
    fn test_large_message_rejection() {
        let large_msg = NativeMessage::request(
            "test-large",
            "dom_snapshot",
            serde_json::json!({"html": "x".repeat((MAX_MESSAGE_SIZE + 100) as usize)})
        );
        assert!(large_msg.to_bytes().is_err());
    }
}
