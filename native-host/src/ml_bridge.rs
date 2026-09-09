//! ML Bridge - Async Rust <-> Python IPC for ML Backend

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::process::Stdio;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use std::time::Duration;
use tokio::time::timeout;
use std::path::PathBuf;
use std::collections::HashMap;
use log::{info, warn, error, debug};

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
            script_path: "ml-backend/nlp_processor.py".to_string(),
            working_dir: None,
            timeout_secs: 30,
            max_retries: 3,
        }
    }
}

/// StoreDocument - fixed colon
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoreDocument {
    pub id: String,
    pub text: String,
    pub meta: Option<serde_json::Value>,
}

/// Request types sent to the Python backend - fixed derives
#[derive(Debug, Serialize, Deserialize, Clone)]
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
    StoreDocument {
        id: String,
        text: String,
        meta: Option<serde_json::Value>,
    },

    #[serde(rename = "query_rag")]
    RagQuery { query: String, top_k: Option<usize> },

    #[serde(rename = "generate_response")]
    GenerateResponse {
        prompt: String,
        context: Option<String>,
        system_prompt: Option<String>,
        model: Option<String>,
    },

    #[serde(rename = "heartbeat")]
    Heartbeat,
}

/// Response types - fixed with field name and derives
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MlResponse {
    pub status: String,
    pub data: Option<serde_json::Value>,
    pub message: Option<String>,
}

/// Document for RAG storage - fixed colon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagDocument {
    pub id: String,
    pub content: String,
    pub meta: HashMap<String, String>,
    pub timestamp: u64,
}

/// The ML Bridge structure
pub struct MlBridge {
    config: MlBridgeConfig,
    process: Arc<Mutex<Option<Child>>>,
    stdin: Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    stdout: Arc<Mutex<Option<BufReader<tokio::process::ChildStdout>>>>,
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

        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr);
                let mut line = String::new();
                while let Ok(n) = reader.read_line(&mut line).await {
                    if n == 0 { break; }
                    warn!(target: "ml_bridge", "Python STDERR: {}", line.trim());
                    line.clear();
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
        if self.process.lock().await.is_none() {
            self.start().await?;
        }

        let json_req = serde_json::to_string(&request)
            .map_err(|e| format!("Serialization error: {}", e))?;

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

    /// High-level API methods
    pub async fn analyze_sentiment(&self, text: &str) -> Result<serde_json::Value, String> {
        let resp = self.send_request(MlRequest::Sentiment { text: text.to_string() }).await?;
        Ok(resp.data.unwrap_or(json!(null)))
    }

    pub async fn extract_entities(&self, text: &str) -> Result<serde_json::Value, String> {
        let resp = self.send_request(MlRequest::Entities { text: text.to_string() }).await?;
        Ok(resp.data.unwrap_or(json!([])))
    }

    pub async fn summarize(&self, text: &str, max_length: Option<usize>) -> Result<serde_json::Value, String> {
        let resp = self.send_request(MlRequest::Summarize {
            text: text.to_string(),
            max_length,
        }).await?;
        Ok(resp.data.unwrap_or(json!(null)))
    }

    pub async fn extract_keywords(&self, text: &str, top_k: Option<usize>) -> Result<Vec<String>, String> {
        let resp = self.send_request(MlRequest::Keywords {
            text: text.to_string(),
            top_k,
        }).await?;
        match resp.data {
            Some(v) => serde_json::from_value(v).map_err(|e| e.to_string()),
            None => Ok(vec![]),
        }
    }

    pub async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>, String> {
        let resp = self.send_request(MlRequest::Embedding { text: text.to_string() }).await?;
        match resp.data {
            Some(v) => serde_json::from_value(v).map_err(|e| e.to_string()),
            None => Err("No embedding returned".to_string()),
        }
    }

    pub async fn store_document(&self, id: String, text: String, meta: Option<serde_json::Value>) -> Result<bool, String> {
        let resp = self.send_request(MlRequest::StoreDocument { id, text, meta }).await?;
        Ok(resp.status == "success")
    }

    pub async fn query_rag(&self, query: &str, top_k: usize) -> Result<serde_json::Value, String> {
        let resp = self.send_request(MlRequest::RagQuery {
            query: query.to_string(),
            top_k: Some(top_k),
        }).await?;
        Ok(resp.data.unwrap_or(json!([])))
    }

    pub async fn generate_response(
        &self,
        prompt: &str,
        context: Option<String>,
        system_prompt: Option<String>,
        model: Option<String>,
    ) -> Result<String, String> {
        let resp = self.send_request(MlRequest::GenerateResponse {
            prompt: prompt.to_string(),
            context,
            system_prompt,
            model,
        }).await?;

        match resp.data {
            Some(v) => v.as_str().map(|s| s.to_string()).ok_or("Invalid response format".to_string()),
            None => Err("No response generated".to_string()),
        }
    }

    pub async fn rag_store(&self, documents: Vec<RagDocument>) -> Result<serde_json::Value, String> {
        let resp = self.send_request(MlRequest::StoreDocument {
            id: "batch".to_string(),
            text: serde_json::to_string(&documents).unwrap_or_default(),
            meta: Some(json!({ "batch": true })),
        }).await?;
        Ok(resp.data.unwrap_or(json!({ "stored": documents.len() })))
    }

    pub async fn is_healthy(&self) -> bool {
        self.send_request(MlRequest::Heartbeat).await.is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            meta: metadata,
            timestamp: 1234567890,
        };

        assert_eq!(doc.id, "doc-1");
        assert_eq!(doc.content, "Test content");
    }
}
