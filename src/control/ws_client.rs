//! WebSocket client for control protocol
//!
//! Used by the demo app to connect to the control server.

use super::protocol::{Command, Request, ResponseMessage, DEFAULT_WS_PORT};
use futures_util::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::Duration;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Request ID counter
static REQUEST_ID: AtomicU64 = AtomicU64::new(1);

fn next_request_id() -> u64 {
    let counter = REQUEST_ID.fetch_add(1, Ordering::Relaxed) & 0xffff;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    now.wrapping_shl(16) ^ counter
}

/// WebSocket client for connecting to the control server
pub struct WsClient {
    write_tx: mpsc::Sender<Message>,
    pending_requests: Arc<RwLock<std::collections::HashMap<u64, oneshot::Sender<ResponseMessage>>>>,
}

impl WsClient {
    /// Connect to the control server
    pub async fn connect(host: &str, port: u16) -> anyhow::Result<Self> {
        let url = format!("ws://{}:{}", host, port);
        let (ws_stream, _) = connect_async(&url).await?;
        log::info!("Connected to control server at {}", url);

        let (write, read) = ws_stream.split();

        // Channel for sending messages
        let (write_tx, mut write_rx) = mpsc::channel::<Message>(32);

        // Pending requests waiting for responses
        let pending_requests = Arc::new(RwLock::new(
            std::collections::HashMap::<u64, oneshot::Sender<ResponseMessage>>::new(),
        ));

        // Spawn write task
        let _write_task = tokio::spawn(async move {
            let mut write = write;
            while let Some(msg) = write_rx.recv().await {
                if write.send(msg).await.is_err() {
                    break;
                }
            }
        });

        // Spawn read task
        let pending_clone = Arc::clone(&pending_requests);
        let _read_task = tokio::spawn(async move {
            let mut read = read;
            while let Some(Ok(msg)) = read.next().await {
                if let Message::Text(text) = msg {
                    if let Ok(response) = serde_json::from_str::<ResponseMessage>(&text) {
                        let mut pending = pending_clone.write().await;
                        if let Some(tx) = pending.remove(&response.id) {
                            let _ = tx.send(response);
                        }
                    }
                }
            }
        });

        Ok(Self {
            write_tx,
            pending_requests,
        })
    }

    /// Connect to localhost with default port
    pub async fn connect_local() -> anyhow::Result<Self> {
        Self::connect("127.0.0.1", DEFAULT_WS_PORT).await
    }

    /// Send a command and wait for response
    pub async fn send_command(&self, command: Command) -> anyhow::Result<ResponseMessage> {
        self.send_command_with_timeout(command, Duration::from_secs(10)).await
    }

    /// Send a command and wait for response with an explicit timeout
    pub async fn send_command_with_timeout(
        &self,
        command: Command,
        timeout: Duration,
    ) -> anyhow::Result<ResponseMessage> {
        let id = next_request_id();
        let request = Request::new(id, command);
        let json = serde_json::to_string(&request)?;

        // Create response channel
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending_requests.write().await;
            pending.insert(id, tx);
        }

        // Send request
        self.write_tx.send(Message::Text(json.into())).await?;

        // Wait for response with timeout
        let response = tokio::time::timeout(timeout, rx).await??;

        Ok(response)
    }

    /// Send a command without waiting for response
    pub async fn send_command_fire_and_forget(&self, command: Command) -> anyhow::Result<()> {
        let id = next_request_id();
        let request = Request::new(id, command);
        let json = serde_json::to_string(&request)?;
        self.write_tx.send(Message::Text(json.into())).await?;
        Ok(())
    }
}

/// Simple blocking client for CLI usage
pub struct BlockingWsClient {
    runtime: tokio::runtime::Runtime,
    client: Option<WsClient>,
}

impl BlockingWsClient {
    /// Create a new blocking client
    pub fn new() -> anyhow::Result<Self> {
        let runtime = tokio::runtime::Runtime::new()?;
        Ok(Self {
            runtime,
            client: None,
        })
    }

    /// Connect to the control server
    pub fn connect(&mut self, host: &str, port: u16) -> anyhow::Result<()> {
        let client = self.runtime.block_on(WsClient::connect(host, port))?;
        self.client = Some(client);
        Ok(())
    }

    /// Connect to localhost with default port
    pub fn connect_local(&mut self) -> anyhow::Result<()> {
        self.connect("127.0.0.1", DEFAULT_WS_PORT)
    }

    /// Send a command and wait for response
    pub fn send_command(&self, command: Command) -> anyhow::Result<ResponseMessage> {
        let client = self.client.as_ref().ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        self.runtime.block_on(client.send_command(command))
    }

    /// Send a command and wait for response with an explicit timeout
    pub fn send_command_with_timeout(
        &self,
        command: Command,
        timeout: Duration,
    ) -> anyhow::Result<ResponseMessage> {
        let client = self.client.as_ref().ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        self.runtime
            .block_on(client.send_command_with_timeout(command, timeout))
    }
}

impl Default for BlockingWsClient {
    fn default() -> Self {
        Self::new().expect("Failed to create tokio runtime")
    }
}
