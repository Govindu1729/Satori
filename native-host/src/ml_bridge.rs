//! ML Bridge - Async Rust <-> Python IPC for ML Backend

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::process::{Stdio, Command};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Child;  // Use tokio::process instead of std::process
use std::time::Duration;
use tokio::time::timeout;
use std::path::PathBuf;
use std::collections::HashMap;

// Add log dependency
use log::{info, warn, error, debug};

// ... (rest of imports remain same) ...

/// StoreDocument with correct syntax - FIXED
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoreDocument {
    pub id: String,
    pub text: String,
    pub meta: Option<serde_json::Value>,  // Added colon
}

/// Request types sent to the Python backend - FIXED with derives
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
        meta: Option<serde_json::Value>  // Fixed: added colon
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

/// Response types - FIXED with field name and derives
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MlResponse {
    pub status: String, // "success" or "error"
    pub data: Option<serde_json::Value>,  // Added field name "data"
    pub message: Option<String>,
}

/// Document for RAG storage - FIXED with colon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagDocument {
    pub id: String,
    pub content: String,
    pub meta: HashMap<String, String>,  // Added colon
    pub timestamp: u64,
}

/// The ML Bridge structure - FIXED async I/O
pub struct MlBridge {
    config: MlBridgeConfig,
    process: Arc<Mutex<Option<Child>>>,  // Use tokio::process::Child
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

    /// Initialize the Python subprocess - FIXED with tokio::process
    pub async fn start(&self) -> Result<(), String> {
        let script_path = if let Some(ref work_dir) = self.config.working_dir {
            work_dir.join(&self.config.script_path)
        } else {
            PathBuf::from(&self.config.script_path)
        };

        let mut cmd = tokio::process::Command::new(&self.config.python_path);
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

        // Spawn a task to log stderr asynchronously - FIXED
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

    // ... (rest of methods remain similar but with async fixes) ...

    /// FIXED: Store document with correct parameter syntax
    pub async fn store_document(&self, id: String, text: String, meta: Option<serde_json::Value>) -> Result<bool, String> {
        let resp = self.send_request(MlRequest::StoreDocument { id, text, meta }).await?;
        Ok(resp.status == "success")
    }

    // ... (other methods remain the same) ...
}
