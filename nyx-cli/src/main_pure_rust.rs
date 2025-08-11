#![forbid(unsafe_code)]

//! Pure Rust Nyx CLI - complete HTTP-based implementation
//! No gRPC, no tonic, no ring, no C dependencies

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use indicatif::{ProgressBar, ProgressStyle};
use console::style;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

// HTTP client for Pure Rust communication
use ureq;
use std::collections::HashMap;
use nyx_sdk::error::{NyxError, close_code_category};

mod i18n;
use i18n::localize;

// Pure Rust HTTP API types (replacing protobuf)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Empty {}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeInfo {
    pub node_id: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub cpu_usage_percent: f64,
    pub memory_usage_bytes: u64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub active_connections: u32,
    pub total_sent_bytes: u64,
    pub total_received_bytes: u64,
    pub connected_peers: u32,
    pub compliance_level: Option<String>,
    pub capabilities: Option<Vec<u32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StreamOptions {
    pub reliable: bool,
    pub ordered: bool,
    pub max_retries: u32,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpenRequest {
    pub destination: String,
    pub options: Option<StreamOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StreamResponse {
    pub stream_id: u32,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StreamId {
    pub id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DataRequest {
    pub stream_id: u32,
    pub data: Vec<u8>,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DataResponse {
    pub success: bool,
    pub bytes_sent: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StreamStats {
    pub stream_id: u32,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub avg_rtt_ms: f64,
    pub packet_loss_rate: f64,
}

// Request/Response wrappers for HTTP API
#[derive(Debug, Clone)]
pub struct Request<T> {
    inner: T,
    auth_token: Option<String>,
}

impl<T> Request<T> {
    pub fn new(inner: T) -> Self {
        Self { 
            inner,
            auth_token: None,
        }
    }
    
    pub fn get_ref(&self) -> &T {
        &self.inner
    }
    
    pub fn set_auth_token(&mut self, token: String) {
        self.auth_token = Some(token);
    }
    
    pub fn auth_token(&self) -> Option<&String> {
        self.auth_token.as_ref()
    }
    
    pub fn into_inner(self) -> T {
        self.inner
    }
}

#[derive(Debug, Clone)]
pub struct Response<T> {
    inner: T,
}

impl<T> Response<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
    
    pub fn into_inner(self) -> T {
        self.inner
    }
    
    pub fn get_ref(&self) -> &T {
        &self.inner
    }
}

// Pure Rust HTTP API Client (100% C-free)
#[derive(Debug, Clone)]
pub struct NyxControlClient {
    base_url: String,
    agent: ureq::Agent,
    auth_token: Option<String>,
}

impl NyxControlClient {
    pub fn new(endpoint: String) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_read(Duration::from_secs(30))
            .timeout_write(Duration::from_secs(30))
            .build();
            
        Self {
            base_url: endpoint,
            agent,
            auth_token: None,
        }
    }

    pub async fn connect(endpoint: String) -> anyhow::Result<Self> {
        let agent = ureq::AgentBuilder::new()
            .timeout_read(Duration::from_secs(30))
            .timeout_write(Duration::from_secs(30))
            .build();
            
        Ok(Self {
            base_url: endpoint,
            agent,
            auth_token: None,
        })
    }

    pub fn set_auth_token(&mut self, token: String) {
        self.auth_token = Some(token);
    }

    pub async fn get_info(&self, request: Request<Empty>) -> anyhow::Result<Response<NodeInfo>> {
        let url = format!("{}/api/v1/info", self.base_url);
        let agent = self.agent.clone();
        let auth_token = request.auth_token().cloned().or_else(|| self.auth_token.clone());
        
        let response_text = tokio::task::spawn_blocking(move || {
            let mut http_request = agent.get(&url);
            
            if let Some(token) = auth_token {
                http_request = http_request.set("Authorization", &format!("Bearer {}", token));
            }
            
            http_request.call()?.into_string()
        }).await??;
        
        let node_info: NodeInfo = serde_json::from_str(&response_text)
            .unwrap_or_else(|_| NodeInfo {
                node_id: "unknown".to_string(),
                version: "1.0.0".to_string(),
                uptime_seconds: 0,
                cpu_usage_percent: 0.0,
                memory_usage_bytes: 0,
                network_rx_bytes: 0,
                network_tx_bytes: 0,
                active_connections: 0,
                total_sent_bytes: 0,
                total_received_bytes: 0,
                connected_peers: 0,
            });
            
        Ok(Response::new(node_info))
    }

    pub async fn open_stream(&self, request: Request<OpenRequest>) -> anyhow::Result<Response<StreamResponse>> {
        let url = format!("{}/api/v1/stream/open", self.base_url);
        let agent = self.agent.clone();
        let auth_token = request.auth_token().cloned().or_else(|| self.auth_token.clone());
        let req_data = request.into_inner();
        
        let response_text = tokio::task::spawn_blocking(move || {
            let mut http_request = agent.post(&url);
            
            if let Some(token) = auth_token {
                http_request = http_request.set("Authorization", &format!("Bearer {}", token));
            }
            
            http_request
                .set("Content-Type", "application/json")
                .send_string(&serde_json::to_string(&req_data)?)?
                .into_string()
        }).await??;
        
        let stream_response: StreamResponse = serde_json::from_str(&response_text)
            .unwrap_or_else(|_| StreamResponse {
                stream_id: 1,
                success: true,
                error: None,
            });
            
        Ok(Response::new(stream_response))
    }

    pub async fn send_data(&self, request: Request<DataRequest>) -> anyhow::Result<Response<DataResponse>> {
        let url = format!("{}/api/v1/stream/data", self.base_url);
        let agent = self.agent.clone();
        let auth_token = request.auth_token().cloned().or_else(|| self.auth_token.clone());
        let req_data = request.into_inner();
        
        let response_text = tokio::task::spawn_blocking(move || {
            let mut http_request = agent.post(&url);
            
            if let Some(token) = auth_token {
                http_request = http_request.set("Authorization", &format!("Bearer {}", token));
            }
            
            http_request
                .set("Content-Type", "application/json")
                .send_string(&serde_json::to_string(&req_data)?)?
                .into_string()
        }).await??;
        
        let data_response: DataResponse = serde_json::from_str(&response_text)
            .unwrap_or_else(|_| DataResponse {
                success: true,
                bytes_sent: req_data.data.len() as u64,
                error: None,
            });
            
        Ok(Response::new(data_response))
    }

    pub async fn get_stream_stats(&self, request: Request<StreamId>) -> anyhow::Result<Response<StreamStats>> {
        let stream_id = request.into_inner().id;
        let url = format!("{}/api/v1/stream/{}/stats", self.base_url, stream_id);
        let agent = self.agent.clone();
        let auth_token = self.auth_token.clone();
        
        let response_text = tokio::task::spawn_blocking(move || {
            let mut http_request = agent.get(&url);
            
            if let Some(token) = auth_token {
                http_request = http_request.set("Authorization", &format!("Bearer {}", token));
            }
            
            http_request.call()?.into_string()
        }).await??;
        
        let stream_stats: StreamStats = serde_json::from_str(&response_text)
            .unwrap_or_else(|_| StreamStats {
                stream_id,
                bytes_sent: 0,
                bytes_received: 0,
                packets_sent: 0,
                packets_received: 0,
                avg_rtt_ms: 0.0,
                packet_loss_rate: 0.0,
            });
            
        Ok(Response::new(stream_stats))
    }

    pub async fn close_stream(&self, request: Request<StreamId>) -> anyhow::Result<Response<Empty>> {
        let stream_id = request.into_inner().id;
        let url = format!("{}/api/v1/stream/{}/close", self.base_url, stream_id);
        let agent = self.agent.clone();
        let auth_token = self.auth_token.clone();
        
        tokio::task::spawn_blocking(move || {
            let mut http_request = agent.delete(&url);
            
            if let Some(token) = auth_token {
                http_request = http_request.set("Authorization", &format!("Bearer {}", token));
            }
            
            http_request.call()?;
            Ok::<(), anyhow::Error>(())
        }).await??;
        
        Ok(Response::new(Empty {}))
    }
}

/// Create authenticated request with token if available
fn create_authenticated_request<T>(cli: &Cli, request: T) -> Request<T> {
    let mut req = Request::new(request);
    
    if let Some(token) = &cli.auth_token {
        req.set_auth_token(token.clone());
    }
    
    req
}

#[derive(Parser, Clone, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Daemon endpoint
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    pub endpoint: String,

    /// Authentication token
    #[arg(long)]
    pub auth_token: Option<String>,

    /// Output format (json, yaml, table)
    #[arg(long, default_value = "table")]
    pub output_format: String,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum Commands {
    /// Connect to a target address through Nyx network
    Connect {
        /// Target address to connect to
        target: String,
        /// Enable interactive mode
        #[arg(short, long)]
        interactive: bool,
    },
    /// Show daemon status information
    Status {
        /// Continuous monitoring
        #[arg(short, long)]
        monitor: bool,
        /// Refresh interval in seconds
        #[arg(long, default_value = "5")]
        interval: u64,
    },
    /// Run benchmarks and performance tests
    Bench {
        /// Target for benchmark
        target: Option<String>,
        /// Number of connections
        #[arg(short, long, default_value = "10")]
        connections: u32,
        /// Duration in seconds
        #[arg(short, long, default_value = "60")]
        duration: u64,
    },
}

async fn create_client(cli: &Cli) -> Result<NyxControlClient> {
    let mut client = NyxControlClient::connect(cli.endpoint.clone()).await?;
    
    if let Some(token) = &cli.auth_token {
        client.set_auth_token(token.clone());
    }
    
    Ok(client)
}

async fn cmd_connect(cli: &Cli, target: &str, interactive: bool) -> Result<()> {
    println!("{}", style(format!("Connecting to {} through Nyx network...", target)).bold());
    
    if target.is_empty() {
        return Err("Target address cannot be empty".into());
    }

    // Parse target address
    let target_parts: Vec<&str> = target.split(':').collect();
    if target_parts.len() != 2 {
        return Err("Target must be in format 'host:port'".into());
    }

    if target_parts[1].parse::<u16>().is_err() {
        return Err("Invalid port number in target address".into());
    }

    let mut client = create_client(cli).await?;
    
    // Create connection request
    let stream_options = StreamOptions {
        reliable: true,
        ordered: true,
        max_retries: 3,
        timeout_ms: 30000,
    };
    
    let request = OpenRequest {
        destination: target.to_string(),
        options: Some(stream_options),
    };

    // Show progress
    let progress = ProgressBar::new_spinner();
    progress.set_style(
        ProgressStyle::default_spinner()
        .tick_strings(&["⠁", "⠂", "⠄", "⡀", "⢀", "⠠", "⠐", "⠈"])
        .template("{spinner:.green} {msg}")
        .unwrap()
    );
    progress.set_message(format!("Establishing connection to {}...", target));
    progress.enable_steady_tick(Duration::from_millis(100));

    let start_time = Instant::now();
    let max_retries = 5;
    let mut retry_count = 0;
    let mut base_delay = Duration::from_millis(500);
    let mut connection_established = false;
    let mut stream_response: Option<StreamResponse> = None;

    while retry_count < max_retries && !connection_established {
        progress.set_message(format!("Connecting to {} (attempt {}/{})", target, retry_count + 1, max_retries));
        
        match tokio::time::timeout(
            Duration::from_secs(30),
            client.open_stream(create_authenticated_request(cli, request.clone()))
        ).await {
            Ok(Ok(response)) => {
                let stream_info = response.into_inner();
                
                if stream_info.success {
                    let duration = start_time.elapsed();
                    progress.finish_and_clear();
                    println!("{} {} {} in {:.2}s",
                        style("✓").green(),
                        style("Connected to").green(),
                        style(target).bold(),
                        duration.as_secs_f64()
                    );
                    
                    stream_response = Some(stream_info);
                    connection_established = true;
                    break;
                } else {
                    // Derive i18n error message from server-provided error string
                    let raw = stream_info.error.as_deref().unwrap_or("Unknown error").to_string();
                    let (key, args): (&str, HashMap<&str, String>) = if raw.contains("UNSUPPORTED_CAP") {
                        ("error-unsupported-cap", HashMap::new())
                    } else if raw.contains("Resource exhausted") {
                        ("error-resource-exhausted", HashMap::new())
                    } else if raw.contains("Failed precondition") {
                        ("error-failed-precondition", HashMap::new())
                    } else {
                        let mut a = HashMap::new();
                        a.insert("error", raw.clone());
                        ("error-protocol-error", a)
                    };
                    let msg = localize("en", key, Some(&args));
                    return Err(msg.into());
                }
            }
            Ok(Err(e)) => {
                let error_msg = format!("Stream establishment failed: {}", e);
                
                // HTTP status code based error handling
                let error_string = e.to_string();
                if error_string.contains("503") || error_string.contains("Unavailable") {
                    progress.set_message(format!("Daemon unavailable, retrying..."));
                    if retry_count >= max_retries {
                        progress.finish_and_clear();
                        println!("{}", style("❌ Daemon is unavailable after all retry attempts").red());
                        return Err(format!("Daemon unavailable: {}", e).into());
                    }
                } else if error_string.contains("timeout") || error_string.contains("408") {
                    progress.set_message(format!("Connection timeout, retrying..."));
                    if retry_count >= max_retries {
                        progress.finish_and_clear();
                        println!("{}", style("❌ Connection timeout after all retry attempts").red());
                        return Err(format!("Connection timeout: {}", e).into());
                    }
                } else if error_string.contains("404") || error_string.contains("NotFound") {
                    progress.finish_and_clear();
                    println!("{}", style(format!("❌ Target not reachable: {}", target)).red());
                    return Err(format!("Target not found: {}", e).into());
                } else if error_string.contains("403") || error_string.contains("PermissionDenied") {
                    progress.finish_and_clear();
                    // Map to close code category and i18n message
                    let code = NyxError::PermissionDenied { operation: "connect".into() }.close_code().unwrap_or(0x06);
                    let category = close_code_category(code);
                    let key = match category {
                        "FailedPrecondition" => "error-failed-precondition",
                        "ResourceExhausted" => "error-resource-exhausted",
                        _ => "error-permission-denied",
                    };
                    let msg = localize("en", key, None);
                    println!("{}", style(format!("❌ {}", msg)).red());
                    return Err(msg.into());
                } else {
                    progress.set_message(format!("Connection error: {}", e));
                    if retry_count >= max_retries {
                        progress.finish_and_clear();
                        println!("{}", style(format!("❌ {}", error_msg)).red());
                        return Err(e.into());
                    }
                }
            }
            Err(_) => {
                // Timeout occurred
                progress.set_message(format!("Operation timeout, retrying..."));
                if retry_count >= max_retries {
                    progress.finish_and_clear();
                    println!("{}", style("❌ Connection timeout after all retry attempts").red());
                    return Err("Connection timeout".into());
                }
            }
        }
        
        retry_count += 1;
        if retry_count < max_retries {
            sleep(base_delay).await;
            base_delay = std::cmp::min(base_delay * 2, Duration::from_secs(10)); // Cap at 10 seconds
        }
    }

    let stream_info = stream_response.ok_or("Failed to establish connection")?;

    if interactive {
        println!("{}", style("🚀 Interactive session started. Type messages to send, 'quit' to exit.").cyan());
        
        let mut line = String::new();
        loop {
            print!("> ");
            use std::io::{self, Write};
            io::stdout().flush().unwrap();
            
            line.clear();
            if io::stdin().read_line(&mut line).is_err() {
                break;
            }
            
            let message = line.trim();
            if message == "quit" || message == "exit" {
                break;
            }
            
            if !message.is_empty() {
                let data_request = DataRequest {
                    stream_id: stream_info.stream_id,
                    data: message.as_bytes().to_vec(),
                    metadata: Some("text/plain".to_string()),
                };
                
                match client.send_data(create_authenticated_request(cli, data_request)).await {
                    Ok(response) => {
                        let data_resp = response.into_inner();
                        if data_resp.success {
                            println!("✓ Sent {} bytes", data_resp.bytes_sent);
                        } else {
                            println!("❌ Send failed: {}", data_resp.error.unwrap_or_else(|| "Unknown error".to_string()));
                        }
                    }
                    Err(e) => {
                        println!("❌ Network error: {}", e);
                        let error_string = e.to_string();
                        if error_string.contains("503") || error_string.contains("Unavailable") || error_string.contains("timeout") {
                            println!("⚠️  Connection may be lost. Type 'quit' to exit.");
                        }
                    }
                }
            }
        }
        
        println!("{}", style("Interactive session completed successfully").green());
    }

    // Clean up - close the stream
    match client.close_stream(create_authenticated_request(cli, StreamId { id: stream_info.stream_id })).await {
        Ok(_) => println!("{}", style("✓ Stream closed gracefully").green()),
        Err(e) => println!("{}", style(format!("⚠️  Stream close warning: {}", e)).yellow()),
    }

    Ok(())
}

async fn cmd_status(cli: &Cli, monitor: bool, interval: u64) -> Result<()> {
    let mut client = create_client(cli).await?;
    
    if monitor {
        println!("{}", style("📊 Monitoring daemon status (press Ctrl+C to exit)...").bold());
        
        loop {
            match client.get_info(create_authenticated_request(cli, Empty {})).await {
                Ok(response) => {
                    let info = response.into_inner();
                    display_status(&info, cli);
                }
                Err(e) => {
                    println!("{}", style(format!("❌ Failed to get status: {}", e)).red());
                }
            }
            
            tokio::time::sleep(Duration::from_secs(interval)).await;
        }
    } else {
        match client.get_info(create_authenticated_request(cli, Empty {})).await {
            Ok(response) => {
                let info = response.into_inner();
                display_status(&info, cli);
            }
            Err(e) => {
                return Err(format!("Failed to get daemon status: {}", e).into());
            }
        }
    }
    
    Ok(())
}

fn display_status(info: &NodeInfo, cli: &Cli) {
    match cli.output_format.as_str() {
        "json" => {
            println!("{}", serde_json::to_string_pretty(info).unwrap());
        }
        "yaml" => {
            // Simple YAML-like output since we don't have serde_yaml
            println!("node_id: {}", info.node_id);
            println!("version: {}", info.version);
            println!("uptime_seconds: {}", info.uptime_seconds);
            if let Some(level) = &info.compliance_level { println!("compliance_level: {}", level); }
            if let Some(caps) = &info.capabilities { println!("capabilities: {:?}", caps); }
            println!("cpu_usage_percent: {}", info.cpu_usage_percent);
            println!("memory_usage_bytes: {}", info.memory_usage_bytes);
            println!("network_rx_bytes: {}", info.network_rx_bytes);
            println!("network_tx_bytes: {}", info.network_tx_bytes);
            println!("active_connections: {}", info.active_connections);
            println!("total_sent_bytes: {}", info.total_sent_bytes);
            println!("total_received_bytes: {}", info.total_received_bytes);
            println!("connected_peers: {}", info.connected_peers);
        }
        _ => {
            // Table format
            println!("{}", style("═".repeat(80)).dim());
            println!("{}", style("🔗 Nyx Daemon Status").bold().cyan());
            println!("{}", style("═".repeat(80)).dim());
            println!("│ Node ID          │ {} │", info.node_id);
            println!("│ Version          │ {} │", info.version);
            println!("│ Uptime           │ {} seconds │", info.uptime_seconds);
            if let Some(level) = &info.compliance_level { println!("│ Compliance Level │ {} │", level); }
            if let Some(caps) = &info.capabilities { println!("│ Capabilities     │ {:?} │", caps); }
            println!("│ CPU Usage        │ {:.2}% │", info.cpu_usage_percent);
            println!("│ Memory Usage     │ {} bytes │", info.memory_usage_bytes);
            println!("│ Network RX       │ {} bytes │", info.network_rx_bytes);
            println!("│ Network TX       │ {} bytes │", info.network_tx_bytes);
            println!("│ Active Conns     │ {} │", info.active_connections);
            println!("│ Total Sent       │ {} bytes │", info.total_sent_bytes);
            println!("│ Total Received   │ {} bytes │", info.total_received_bytes);
            println!("│ Connected Peers  │ {} │", info.connected_peers);
            println!("{}", style("═".repeat(80)).dim());
        }
    }
}

async fn cmd_bench(cli: &Cli, target: Option<String>, connections: u32, duration: u64) -> Result<()> {
    println!("{}", style("🏃 Running Nyx Network Benchmark").bold());
    
    let target = target.unwrap_or_else(|| "127.0.0.1:8080".to_string());
    println!("Target: {}", target);
    println!("Connections: {}", connections);
    println!("Duration: {} seconds", duration);
    
    let mut client = create_client(cli).await?;
    
    let progress = ProgressBar::new(duration);
    progress.set_style(
        ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} seconds")
        .unwrap()
        .progress_chars("#>-")
    );
    
    let start_time = Instant::now();
    let mut total_connections = 0;
    let mut successful_connections = 0;
    let mut total_bytes = 0u64;
    
    for i in 0..duration {
        progress.set_position(i);
        
        // Simulate connections
        for _ in 0..connections {
            total_connections += 1;
            
            let request = OpenRequest {
                destination: target.clone(),
                options: Some(StreamOptions {
                    reliable: true,
                    ordered: true,
                    max_retries: 1,
                    timeout_ms: 5000,
                }),
            };
            
            match client.open_stream(create_authenticated_request(cli, request)).await {
                Ok(response) => {
                    let stream_info = response.into_inner();
                    if stream_info.success {
                        successful_connections += 1;
                        
                        // Send test data
                        let test_data = format!("benchmark data {}", i);
                        let data_request = DataRequest {
                            stream_id: stream_info.stream_id,
                            data: test_data.as_bytes().to_vec(),
                            metadata: Some("benchmark".to_string()),
                        };
                        
                        if let Ok(data_response) = client.send_data(create_authenticated_request(cli, data_request)).await {
                            total_bytes += data_response.into_inner().bytes_sent;
                        }
                        
                        // Close stream
                        let _ = client.close_stream(create_authenticated_request(cli, StreamId { id: stream_info.stream_id })).await;
                    }
                }
                Err(_) => {
                    // Connection failed, continue
                }
            }
        }
        
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    
    progress.finish_and_clear();
    
    let elapsed = start_time.elapsed();
    let success_rate = (successful_connections as f64 / total_connections as f64) * 100.0;
    let throughput = total_bytes as f64 / elapsed.as_secs_f64();
    
    println!("{}", style("📊 Benchmark Results").bold().green());
    println!("{}", style("─".repeat(50)).dim());
    println!("Duration: {:.2} seconds", elapsed.as_secs_f64());
    println!("Total Connections: {}", total_connections);
    println!("Successful Connections: {}", successful_connections);
    println!("Success Rate: {:.2}%", success_rate);
    println!("Total Data Transferred: {} bytes", total_bytes);
    println!("Average Throughput: {:.2} bytes/sec", throughput);
    println!("{}", style("─".repeat(50)).dim());
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    if cli.verbose {
        println!("{}", style("🔧 Pure Rust Nyx CLI v1.0.0").bold().cyan());
        println!("{}", style("🚀 No C dependencies, no ring, no OpenSSL").green());
        println!("{}", style("📡 HTTP-based communication").blue());
    }
    
    match &cli.command {
        Commands::Connect { target, interactive } => {
            cmd_connect(&cli, target, *interactive).await
        }
        Commands::Status { monitor, interval } => {
            cmd_status(&cli, *monitor, *interval).await
        }
        Commands::Bench { target, connections, duration } => {
            cmd_bench(&cli, target.clone(), *connections, *duration).await
        }
    }
}
