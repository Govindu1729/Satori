//! ML Bridge - Rust <-> Python IPC for ML Backend
//!
//! This module handles bidirectional communication with the Python NLP processor
//! via stdin/stdout pipes, enabling agentic RAG and real-time text analysis.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Configuration for the ML Bridge
#[derive(Debug, Clone)]
pub struct MlBridgeConfig {
    pub python_path: PathBuf,
    pub script_path: PathBuf,
    pub working_dir: Option<PathBuf>,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub heartbeat_interval_secs: u64,
}

impl Default for MlBridgeConfig {
    fn default() -> Self {
        Self {
            python_path: PathBuf::from("python3"),
            script_path: PathBuf::from("ml-backend/nlp_processor.py"),
            working_dir: None,
            timeout_secs: 30,
            max_retries: 3,
            heartbeat_interval_secs: 10,
        }
    }
}

/// Request types sent to Python ML backend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum MlRequest {
    #[serde(rename = "sentiment")]
    SentimentAnalysis { text: String },

    #[serde(rename = "entities")]
    EntityExtraction { text: String },

    #[serde(rename = "summary")]
    Summarization {
        text: String,
        max_length: Option<usize>
    },

    #[serde(rename = "keywords")]
    KeywordExtraction {
        text: String,
        top_k: Option<usize>
    },

    #[serde(rename = "embed")]
    GenerateEmbedding { text: String },

    #[serde(rename = "rag_query")]
    RagQuery {
        query: String,
        top_k: Option<usize>,
        context: Option<Vec<String>>
    },

    #[serde(rename = "rag_store")]
    RagStore {
        documents: Vec<RagDocument>
    },

    #[serde(rename = "heartbeat")]
    Heartbeat,

    #[serde(rename = "shutdown")]
    Shutdown,
}

/// Document for RAG storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagDocument {
    pub id: String,
    pub content: String,
    pub metadata: HashMap<String, String>,
    pub timestamp: u64,
}

/// Response from Python ML backend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MlResponse {
    pub success: bool,
    pub action: String,
    pub data: Value,
    pub error: Option<String>,
    pub latency_ms: Option<u64>,
}

/// ML Bridge handle for communicating with Python subprocess
pub struct MlBridge {
    config: MlBridgeConfig,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    process: Option<Child>,
    response_tx: Sender<MlResponse>,
    response_rx: Arc<Mutex<Receiver<MlResponse>>>,
    is_running: Arc<Mutex<bool>>,
}

impl MlBridge {
    /// Create a new ML Bridge instance
    pub fn new(config: MlBridgeConfig) -> Self {
        let (tx, rx) = channel();
        Self {
            config,
            stdin: None,
            stdout: None,
            process: None,
            response_tx: tx,
            response_rx: Arc::new(Mutex::new(rx)),
            is_running: Arc::new(Mutex::new(false)),
        }
    }

    /// Initialize the Python subprocess
    pub fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("[ML Bridge] Starting Python ML backend...");

        let working_dir = self.config.working_dir.clone()
            .unwrap_or_else(|| std::env::current_dir()?);

        // Resolve script path relative to working directory
        let script_path = if self.config.script_path.is_absolute() {
            self.config.script_path.clone()
        } else {
            working_dir.join(&self.config.script_path)
        };

        if !script_path.exists() {
            return Err(format!(
                "ML script not found at: {}",
                script_path.display()
            ).into());
        }

        let mut child = Command::new(&self.config.python_path)
            .arg(&script_path)
            .current_dir(&working_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let stdin = child.stdin.take()
            .ok_or("Failed to open stdin")?;
        let stdout = child.stdout.take()
            .ok_or("Failed to open stdout")?;

        self.stdin = Some(stdin);
        self.stdout = Some(BufReader::new(stdout));
        self.process = Some(child);

        *self.is_running.lock().unwrap() = true;

        // Start response reader thread
        let response_tx = self.response_tx.clone();
        let is_running = self.is_running.clone();
        let stdout_reader = self.stdout.take().unwrap();

        thread::spawn(move || {
            let mut reader = stdout_reader;
            while *is_running.lock().unwrap() {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => {
                        // EOF reached
                        break;
                    }
                    Ok(_) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }

                        // Parse JSON response
                        match serde_json::from_str::<MlResponse>(trimmed) {
                            Ok(response) => {
                                if response_tx.send(response).is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                eprintln!("[ML Bridge] Failed to parse response: {}", e);
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        println!("[ML Bridge] Python ML backend initialized successfully");
        Ok(())
    }

    /// Send a request and wait for response with timeout
    pub fn send_request(&mut self, request: MlRequest) -> Result<MlResponse, Box<dyn std::error::Error>> {
        if !*self.is_running.lock().unwrap() {
            self.initialize()?;
        }

        let stdin = self.stdin.as_mut()
            .ok_or("ML Bridge not initialized")?;

        // Serialize request
        let request_json = serde_json::to_string(&request)?;
        writeln!(stdin, "{}", request_json)?;
        stdin.flush()?;

        // Wait for response with timeout
        let timeout = Duration::from_secs(self.config.timeout_secs);
        let start = std::time::Instant::now();

        loop {
            if start.elapsed() > timeout {
                return Err("Request timeout".into());
            }

            match self.response_rx.lock().unwrap().try_recv() {
                Ok(response) => {
                    if response.action == serde_json::to_value(&request).unwrap_or_default().get("action").and_then(|v| v.as_str()).unwrap_or("") {
                        return Ok(response);
                    }
                }
                Err(TryRecvError::Empty) => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(TryRecvError::Disconnected) => {
                    return Err("Response channel disconnected".into());
                }
            }
        }
    }

    /// Perform sentiment analysis
    pub fn analyze_sentiment(&mut self, text: &str) -> Result<Value, Box<dyn std::error::Error>> {
        let request = MlRequest::SentimentAnalysis {
            text: text.to_string(),
        };

        let response = self.send_request(request)?;

        if response.success {
            Ok(response.data)
        } else {
            Err(response.error.unwrap_or("Unknown error".into()).into())
        }
    }

    /// Extract entities from text
    pub fn extract_entities(&mut self, text: &str) -> Result<Value, Box<dyn std::error::Error>> {
        let request = MlRequest::EntityExtraction {
            text: text.to_string(),
        };

        let response = self.send_request(request)?;

        if response.success {
            Ok(response.data)
        } else {
            Err(response.error.unwrap_or("Unknown error".into()).into())
        }
    }

    /// Summarize text
    pub fn summarize(&mut self, text: &str, max_length: Option<usize>) -> Result<Value, Box<dyn std::error::Error>> {
        let request = MlRequest::Summarization {
            text: text.to_string(),
            max_length,
        };

        let response = self.send_request(request)?;

        if response.success {
            Ok(response.data)
        } else {
            Err(response.error.unwrap_or("Unknown error".into()).into())
        }
    }

    /// Extract keywords from text
    pub fn extract_keywords(&mut self, text: &str, top_k: Option<usize>) -> Result<Value, Box<dyn std::error::Error>> {
        let request = MlRequest::KeywordExtraction {
            text: text.to_string(),
            top_k,
        };

        let response = self.send_request(request)?;

        if response.success {
            Ok(response.data)
        } else {
            Err(response.error.unwrap_or("Unknown error".into()).into())
        }
    }

    /// Generate embedding vector
    pub fn generate_embedding(&mut self, text: &str) -> Result<Value, Box<dyn std::error::Error>> {
        let request = MlRequest::GenerateEmbedding {
            text: text.to_string(),
        };

        let response = self.send_request(request)?;

        if response.success {
            Ok(response.data)
        } else {
            Err(response.error.unwrap_or("Unknown error".into()).into())
        }
    }

    /// Query RAG system
    pub fn rag_query(&mut self, query: &str, top_k: Option<usize>, context: Option<Vec<String>>) -> Result<Value, Box<dyn std::error::Error>> {
        let request = MlRequest::RagQuery {
            query: query.to_string(),
            top_k,
            context,
        };

        let response = self.send_request(request)?;

        if response.success {
            Ok(response.data)
        } else {
            Err(response.error.unwrap_or("Unknown error".into()).into())
        }
    }

    /// Store documents in RAG system
    pub fn rag_store(&mut self, documents: Vec<RagDocument>) -> Result<Value, Box<dyn std::error::Error>> {
        let request = MlRequest::RagStore { documents };

        let response = self.send_request(request)?;

        if response.success {
            Ok(response.data)
        } else {
            Err(response.error.unwrap_or("Unknown error".into()).into())
        }
    }

    /// Check if ML bridge is running
    pub fn is_running(&self) -> bool {
        *self.is_running.lock().unwrap()
    }

    /// Send heartbeat to check health
    pub fn heartbeat(&mut self) -> bool {
        let request = MlRequest::Heartbeat;
        self.send_request(request).is_ok()
    }

    /// Shutdown the Python subprocess gracefully
    pub fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("[ML Bridge] Shutting down Python ML backend...");

        *self.is_running.lock().unwrap() = false;

        // Send shutdown signal
        if let Some(stdin) = self.stdin.as_mut() {
            let shutdown_request = MlRequest::Shutdown;
            let _ = writeln!(stdin, "{}", serde_json::to_string(&shutdown_request)?);
            let _ = stdin.flush();
        }

        // Wait briefly for graceful shutdown
        thread::sleep(Duration::from_millis(500));

        // Terminate process
        if let Some(mut process) = self.process.take() {
            let _ = process.kill();
            let _ = process.wait();
        }

        println!("[ML Bridge] Python ML backend shut down");
        Ok(())
    }
}

impl Drop for MlBridge {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

/// Convenience function for one-off ML requests
pub fn quick_ml_request(action: &str, text: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let mut config = MlBridgeConfig::default();
    config.timeout_secs = 10;

    let mut bridge = MlBridge::new(config);
    bridge.initialize()?;

    let request = match action {
        "sentiment" => MlRequest::SentimentAnalysis { text: text.to_string() },
        "entities" => MlRequest::EntityExtraction { text: text.to_string() },
        "summary" => MlRequest::Summarization { text: text.to_string(), max_length: None },
        "keywords" => MlRequest::KeywordExtraction { text: text.to_string(), top_k: None },
        "embed" => MlRequest::GenerateEmbedding { text: text.to_string() },
        _ => return Err(format!("Unknown action: {}", action).into()),
    };

    let response = bridge.send_request(request)?;

    if response.success {
        Ok(response.data)
    } else {
        Err(response.error.unwrap_or("Unknown error".into()).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = MlBridgeConfig::default();
        assert_eq!(config.timeout_secs, 30);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_request_serialization() {
        let request = MlRequest::SentimentAnalysis {
            text: "Hello world".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("sentiment"));
        assert!(json.contains("Hello world"));
    }

    #[test]
    fn test_response_deserialization() {
        let json = r#"{
            "success": true,
            "action": "sentiment",
            "data": {"label": "positive", "score": 0.95},
            "error": null,
            "latency_ms": 120
        }"#;

        let response: MlResponse = serde_json::from_str(json).unwrap();
        assert!(response.success);
        assert_eq!(response.action, "sentiment");
        assert_eq!(response.latency_ms, Some(120));
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
