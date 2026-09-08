//! ML Bridge - Async Rust <-> Python IPC for ML Backend
//!
//! This module handles bidirectional asynchronous communication with the Python NLP processor
//! via tokio-managed stdin/stdout pipes, enabling agentic RAG and real-time text analysis.
//! Phase 4: Production ML Integration with Local LLM Orchestration

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::process::{Stdio, Command, Child, ChildStdin, ChildStdout};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::time::Duration;
use tokio::time::timeout;
use log::{info, warn, error, debug};
use std::path::PathBuf;
use std::collections::HashMap;

/// Configuration for the ML Bridge
#[derive(Debug, Clone)]
pub struct MlBridgeConfig {
    pub python_path: String,
    pub script_path: String,
    pub working_dir: Option<PathBuf>,
    pub timeout_secs: u64,
    pub max_retries: u32,
}

impl Default for MlBridgeConfig {
    fn default() -> Self {
        Self {
            python_path: "python3".to_string(),
            script_path: "ml-backend/nlp_processor_v2.py".to_string(),
            working_dir: None,
            timeout_secs: 30,
            max_retries: 3,
        }
    }
}

/// Request types sent to the Python backend
#[derive(Debug, Serialize, Clone)]
#[serde(tag = "action")]
pub enum MlRequest {
    #[serde(rename = "analyze_sentiment")]
    Sentiment { text: String },

    #[serde(rename = "extract_entities")]
    Entities { text: String },

    #[serde(rename = "summarize")]
    Summarize { text: String, max_length: Option<usize> },

    #[serde(rename = "extract_keywords")]
    Keywords { text: String, top_k: Option<usize> },

    #[serde(rename = "generate_embedding")]
    Embedding { text: String },

    #[serde(rename = "store_document")]
    StoreDocument { id: String, text: String, meta Option<serde_json::Value> },

    #[serde(rename = "query_rag")]
    RagQuery { query: String, top_k: Option<usize> },

    #[serde(rename = "generate_response")]
    GenerateResponse {
        prompt: String,
        context: Option<String>,
        system_prompt: Option<String>,
        model: Option<String>, // "ollama" or "gemini"
    },

    #[serde(rename = "heartbeat")]
    Heartbeat,
}

/// Response types received from the Python backend
#[derive(Debug, Deserialize, Clone)]
pub struct MlResponse {
    pub status: String, // "success" or "error"
    pub  Option<serde_json::Value>,
    pub message: Option<String>,
}

/// Document for RAG storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagDocument {
    pub id: String,
    pub content: String,
    pub meta HashMap<String, String>,
    pub timestamp: u64,
}

/// The ML Bridge structure managing the Python subprocess
pub struct MlBridge {
    config: MlBridgeConfig,
    process: Arc<Mutex<Option<Child>>>,
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    stdout: Arc<Mutex<Option<BufReader<ChildStdout>>>>,
}

impl MlBridge {
    pub fn new(config: MlBridgeConfig) -> Self {
        Self {
            config,
            process: Arc::new(Mutex::new(None)),
            stdin: Arc::new(Mutex::new(None)),
            stdout: Arc::new(Mutex::new(None)),
        }
    }

    /// Initialize the Python subprocess
    pub async fn start(&self) -> Result<(), String> {
        let script_path = if let Some(ref work_dir) = self.config.working_dir {
            work_dir.join(&self.config.script_path)
        } else {
            PathBuf::from(&self.config.script_path)
        };

        let mut cmd = Command::new(&self.config.python_path);
        cmd.arg(&script_path)
           .stdin(Stdio::piped())
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());

        if let Some(ref work_dir) = self.config.working_dir {
            cmd.current_dir(work_dir);
        }

        let mut child = cmd.spawn()
            .map_err(|e| format!("Failed to spawn Python process: {}", e))?;

        let stdin = child.stdin.take().ok_or("Failed to open stdin")?;
        let stdout = child.stdout.take().ok_or("Failed to open stdout")?;

        // Spawn a task to log stderr asynchronously
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    warn!(target: "ml_bridge", "Python STDERR: {}", line);
                }
            });
        }

        let mut proc_lock = self.process.lock().await;
        *proc_lock = Some(child);
        drop(proc_lock);

        let mut stdin_lock = self.stdin.lock().await;
        *stdin_lock = Some(stdin);
        drop(stdin_lock);

        let mut stdout_lock = self.stdout.lock().await;
        *stdout_lock = Some(BufReader::new(stdout));
        drop(stdout_lock);

        info!("ML Bridge Python subprocess started successfully");

        // Verify heartbeat
        match self.send_request(MlRequest::Heartbeat).await {
            Ok(_) => {
                info!("ML Bridge heartbeat verified");
                Ok(())
            },
            Err(e) => {
                error!("ML Bridge heartbeat failed: {}", e);
                Err(e)
            }
        }
    }

    /// Send a request and wait for a response with timeout
    pub async fn send_request(&self, request: MlRequest) -> Result<MlResponse, String> {
        let timeout_dur = Duration::from_secs(self.config.timeout_secs);

        match timeout(timeout_dur, self._send_request_inner(request)).await {
            Ok(result) => result,
            Err(_) => Err(format!("Request timed out after {}s", self.config.timeout_secs)),
        }
    }

    async fn _send_request_inner(&self, request: MlRequest) -> Result<MlResponse, String> {
        // Ensure process is running
        if self.process.lock().await.is_none() {
            self.start().await?;
        }

        let json_req = serde_json::to_string(&request)
            .map_err(|e| format!("Serialization error: {}", e))?;

        // Write to stdin
        {
            let mut stdin_lock = self.stdin.lock().await;
            let stdin_ref = stdin_lock.as_mut().ok_or("Stdin not available")?;

            stdin_ref.write_all(json_req.as_bytes()).await
                .map_err(|e| format!("Write error: {}", e))?;
            stdin_ref.write_all(b"\n").await
                .map_err(|e| format!("Newline write error: {}", e))?;
            stdin_ref.flush().await
                .map_err(|e| format!("Flush error: {}", e))?;
        }

        // Read from stdout
        {
            let mut stdout_lock = self.stdout.lock().await;
            let stdout_ref = stdout_lock.as_mut().ok_or("Stdout not available")?;

            let mut line = String::new();
            stdout_ref.read_line(&mut line).await
                .map_err(|e| format!("Read error: {}", e))?;

            if line.trim().is_empty() {
                return Err("Empty response from Python backend".to_string());
            }

            let response: MlResponse = serde_json::from_str(&line)
                .map_err(|e| format!("Deserialization error: {}. Raw: {}", e, line))?;

            if response.status == "error" {
                return Err(response.message.unwrap_or_else(|| "Unknown Python error".to_string()));
            }

            Ok(response)
        }
    }

    /// High-level API methods for specific ML tasks

    pub async fn analyze_sentiment(&self, text: String) -> Result<serde_json::Value, String> {
        let resp = self.send_request(MlRequest::Sentiment { text }).await?;
        Ok(resp.data.unwrap_or(json!(null)))
    }

    pub async fn extract_entities(&self, text: String) -> Result<serde_json::Value, String> {
        let resp = self.send_request(MlRequest::Entities { text }).await?;
        Ok(resp.data.unwrap_or(json!([])))
    }

    pub async fn summarize(&self, text: String, max_length: Option<usize>) -> Result<String, String> {
        let resp = self.send_request(MlRequest::Summarize { text, max_length }).await?;
        match resp.data {
            Some(v) => v.as_str().map(|s| s.to_string()).ok_or("Invalid summary format".to_string()),
            None => Err("No summary returned".to_string())
        }
    }

    pub async fn extract_keywords(&self, text: String, top_k: Option<usize>) -> Result<Vec<String>, String> {
        let resp = self.send_request(MlRequest::Keywords { text, top_k }).await?;
        match resp.data {
            Some(v) => serde_json::from_value(v).map_err(|e| e.to_string()),
            None => Ok(vec![])
        }
    }

    pub async fn generate_embedding(&self, text: String) -> Result<Vec<f32>, String> {
        let resp = self.send_request(MlRequest::Embedding { text }).await?;
        match resp.data {
            Some(v) => serde_json::from_value(v).map_err(|e| e.to_string()),
            None => Err("No embedding returned".to_string())
        }
    }

    pub async fn store_document(&self, id: String, text: String, meta Option<serde_json::Value>) -> Result<bool, String> {
        let resp = self.send_request(MlRequest::StoreDocument { id, text, metadata }).await?;
        Ok(resp.status == "success")
    }

    pub async fn query_rag(&self, query: String, top_k: usize) -> Result<serde_json::Value, String> {
        let resp = self.send_request(MlRequest::RagQuery { query, top_k: Some(top_k) }).await?;
        Ok(resp.data.unwrap_or(json!([])))
    }

    pub async fn generate_response(
        &self,
        prompt: String,
        context: Option<String>,
        system_prompt: Option<String>,
        model: Option<String>
    ) -> Result<String, String> {
        let resp = self.send_request(MlRequest::GenerateResponse {
            prompt,
            context,
            system_prompt,
            model
        }).await?;

        match resp.data {
            Some(v) => v.as_str().map(|s| s.to_string()).ok_or("Invalid response format".to_string()),
            None => Err("No response generated".to_string())
        }
    }

    /// Health check
    pub async fn is_healthy(&self) -> bool {
        self.send_request(MlRequest::Heartbeat).await.is_ok()
    }

    /// Attempt to restart the Python process if it died
    pub async fn restart_if_needed(&self) -> Result<(), String> {
        if !self.is_healthy().await {
            warn!("ML Bridge unhealthy, attempting restart...");
            // Kill existing process if any
            {
                let mut proc_lock = self.process.lock().await;
                if let Some(mut child) = proc_lock.take() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
            // Clear stdin/stdout locks
            {
                *self.stdin.lock().await = None;
                *self.stdout.lock().await = None;
            }
            self.start().await?;
            info!("ML Bridge restarted successfully");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bridge_initialization() {
        // Note: This test requires python3 and the script to exist
        let config = MlBridgeConfig::default();
        let bridge = MlBridge::new(config);

        // In a real CI env, you'd mock this or skip if python isn't ready
        // let result = bridge.start().await;
        // assert!(result.is_ok());
    }

    #[test]
    fn test_request_serialization() {
        let req = MlRequest::Sentiment { text: "Hello world".to_string() };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("analyze_sentiment"));
        assert!(json.contains("Hello world"));
    }

    #[test]
    fn test_response_deserialization() {
        let json = r#"{
            "status": "success",
            "data": {"label": "positive", "score": 0.95},
            "message": null
        }"#;

        let response: MlResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.status, "success");
        assert!(response.data.is_some());
    }

    #[test]
    fn test_rag_document() {
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), "web".to_string());

        let doc = RagDocument {
            id: "doc-1".to_string(),
            content: "Test content".to_string(),
            metadata,
            timestamp: 1234567890,
        };

        assert_eq!(doc.id, "doc-1");
        assert_eq!(doc.content, "Test content");
    }
}
