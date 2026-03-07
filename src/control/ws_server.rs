//! WebSocket server for control protocol
//!
//! Provides a WebSocket server that relays commands to the demo app
//! and responses back to connected clients.

use super::protocol::{
    Command, ErrorCode, Event, EventMessage, Request, Response, ResponseMessage,
    DEFAULT_WS_PORT, PROTOCOL_VERSION,
};
use super::state::{SharedControlState, new_shared_state};
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::{accept_async, tungstenite::Message};

/// WebSocket server for demo control
pub struct WsServer {
    state: SharedControlState,
    event_tx: broadcast::Sender<EventMessage>,
}

impl WsServer {
    /// Create a new WebSocket server
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(16);
        Self {
            state: new_shared_state(),
            event_tx,
        }
    }

    /// Get a clone of the shared state for the demo app
    pub fn state(&self) -> SharedControlState {
        Arc::clone(&self.state)
    }

    /// Get an event sender for broadcasting events
    pub fn event_sender(&self) -> broadcast::Sender<EventMessage> {
        self.event_tx.clone()
    }

    /// Start the WebSocket server
    pub async fn run(self, port: u16) -> anyhow::Result<()> {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let listener = TcpListener::bind(&addr).await?;
        log::info!("WebSocket server listening on ws://{}", addr);

        while let Ok((stream, peer_addr)) = listener.accept().await {
            log::info!("New WebSocket connection from {}", peer_addr);
            let state = Arc::clone(&self.state);
            let event_rx = self.event_tx.subscribe();

            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, state, event_rx).await {
                    log::error!("WebSocket error for {}: {}", peer_addr, e);
                }
                log::info!("WebSocket connection closed for {}", peer_addr);
            });
        }

        Ok(())
    }
}

impl Default for WsServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle a single WebSocket connection
async fn handle_connection(
    stream: TcpStream,
    state: SharedControlState,
    mut event_rx: broadcast::Receiver<EventMessage>,
) -> anyhow::Result<()> {
    let ws_stream = accept_async(stream).await?;
    let (mut write, mut read) = ws_stream.split();

    // Mark as connected
    if let Ok(mut s) = state.write() {
        s.set_connected(true);
    }

    // Spawn a task to send events and responses
    let state_clone = Arc::clone(&state);
    let send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                // Check for events to broadcast
                Ok(event) = event_rx.recv() => {
                    let json = serde_json::to_string(&event).unwrap_or_default();
                    if write.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
                // Check for pending responses
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)) => {
                    let response = {
                        let mut s = state_clone.write().ok();
                        s.as_mut().and_then(|s| s.pop_response())
                    };
                    if let Some(resp) = response {
                        let json = serde_json::to_string(&resp).unwrap_or_default();
                        if write.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });

    // Process incoming messages
    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Some(response) = process_message(&text, &state) {
                    if let Ok(mut s) = state.write() {
                        s.push_response(response);
                    }
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(Message::Ping(data)) => {
                // Pong is handled automatically by tungstenite
                log::debug!("Received ping: {:?}", data);
            }
            Err(e) => {
                log::error!("WebSocket receive error: {}", e);
                break;
            }
            _ => {}
        }
    }

    // Clean up
    send_task.abort();
    if let Ok(mut s) = state.write() {
        s.clear_queues();
        s.set_connected(false);
    }

    Ok(())
}

/// Process a single message, returning a response only for immediate commands.
/// Demo-bound commands are queued in state and the demo pushes the response.
fn process_message(text: &str, state: &SharedControlState) -> Option<ResponseMessage> {
    // Parse the request
    let request: Result<Request, _> = serde_json::from_str(text);

    match request {
        Ok(req) => {
            // Check protocol version
            if req.version != PROTOCOL_VERSION {
                return Some(ResponseMessage::error(
                    req.id,
                    ErrorCode::VersionMismatch,
                    format!(
                        "Protocol version mismatch: expected {}, got {}",
                        PROTOCOL_VERSION, req.version
                    ),
                ));
            }

            // Handle immediate responses
            match &req.command {
                Command::Ping => Some(ResponseMessage::new(req.id, Response::Pong)),
                _ => {
                    // Queue command for demo app to process
                    // Demo will push the response when it handles the command
                    if let Ok(mut s) = state.write() {
                        s.push_command(req.id, req.command);
                    }
                    None
                }
            }
        }
        Err(e) => Some(ResponseMessage::error(
            0,
            ErrorCode::InvalidCommand,
            format!("Failed to parse request: {}", e),
        )),
    }
}

/// Start a standalone WebSocket server (for testing or separate process mode)
pub async fn run_standalone(port: Option<u16>) -> anyhow::Result<()> {
    let server = WsServer::new();
    let port = port.unwrap_or(DEFAULT_WS_PORT);
    server.run(port).await
}

/// Broadcast an event to all connected clients
pub fn broadcast_event(tx: &broadcast::Sender<EventMessage>, event: Event) {
    let _ = tx.send(EventMessage::new(event));
}
