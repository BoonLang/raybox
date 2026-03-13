//! WebSocket server for control protocol
//!
//! Provides a WebSocket server that relays commands to the demo app
//! and responses back to connected clients.

use super::protocol::{
    Command, ErrorCode, Event, EventMessage, Request, Response, ResponseMessage, DEFAULT_WS_PORT,
    PROTOCOL_VERSION,
};
use super::state::{new_shared_state, SharedControlState};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_tungstenite::{accept_async, tungstenite::Message};

/// WebSocket server for demo control
pub struct WsServer {
    state: SharedControlState,
    event_tx: broadcast::Sender<EventMessage>,
    command_waker: Option<Arc<dyn Fn() + Send + Sync>>,
    router: Arc<RouterState>,
}

impl WsServer {
    /// Create a new WebSocket server
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(16);
        Self {
            state: new_shared_state(),
            event_tx,
            command_waker: None,
            router: Arc::new(RouterState::default()),
        }
    }

    /// Create a new WebSocket server that wakes the app when commands arrive.
    pub fn with_command_waker(command_waker: Arc<dyn Fn() + Send + Sync>) -> Self {
        let mut server = Self::new();
        server.command_waker = Some(command_waker);
        server
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

        {
            let state = Arc::clone(&self.state);
            let router = Arc::clone(&self.router);
            tokio::spawn(async move {
                dispatch_app_responses(state, router).await;
            });
        }

        while let Ok((stream, peer_addr)) = listener.accept().await {
            log::info!("New WebSocket connection from {}", peer_addr);
            let state = Arc::clone(&self.state);
            let event_rx = self.event_tx.subscribe();
            let command_waker = self.command_waker.clone();
            let router = Arc::clone(&self.router);

            tokio::spawn(async move {
                if let Err(e) =
                    handle_connection(stream, state, event_rx, command_waker, router).await
                {
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
    command_waker: Option<Arc<dyn Fn() + Send + Sync>>,
    router: Arc<RouterState>,
) -> anyhow::Result<()> {
    let ws_stream = accept_async(stream).await?;
    let (mut write, mut read) = ws_stream.split();
    let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel::<Message>();
    let connection_id = router.register_connection(outbound_tx.clone()).await;

    update_connected_flag(&state, &router).await;

    loop {
        tokio::select! {
            Some(message) = outbound_rx.recv() => {
                if write.send(message).await.is_err() {
                    break;
                }
            }
            Ok(event) = event_rx.recv() => {
                let json = serde_json::to_string(&event).unwrap_or_default();
                if write.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
            maybe_msg = read.next() => {
                let Some(msg) = maybe_msg else {
                    break;
                };
                match msg {
                    Ok(Message::Text(text)) => {
                        log::debug!(
                            "Received text message on connection {}: {}",
                            connection_id,
                            text
                        );
                        if is_app_hello_message(&text) {
                            log::info!("Registered Web control app on connection {}", connection_id);
                            router.register_app(connection_id).await;
                            continue;
                        }

                        if router.is_app_connection(connection_id).await {
                            match serde_json::from_str::<ResponseMessage>(&text) {
                                Ok(response) => {
                                    log::debug!(
                                        "Routing app response {} from connection {}",
                                        response.id,
                                        connection_id
                                    );
                                    router.route_response(response).await;
                                    continue;
                                }
                                Err(error) => {
                                    log::warn!(
                                        "Failed to parse app response on connection {}: {}",
                                        connection_id,
                                        error
                                    );
                                }
                            }
                        }

                        if let Some(response) = process_message(
                            &text,
                            connection_id,
                            &state,
                            &router,
                            command_waker.as_deref(),
                        )
                        .await
                        {
                            let json = serde_json::to_string(&response).unwrap_or_default();
                            if write.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Ok(Message::Close(_)) => break,
                    Ok(Message::Binary(data)) => {
                        log::warn!(
                            "Ignoring unexpected binary message on connection {} ({} bytes)",
                            connection_id,
                            data.len()
                        );
                    }
                    Ok(Message::Ping(data)) => {
                        log::debug!("Received ping: {:?}", data);
                    }
                    Err(e) => {
                        log::error!("WebSocket receive error: {}", e);
                        break;
                    }
                    other => {
                        log::warn!(
                            "Ignoring unexpected websocket message on connection {}: {:?}",
                            connection_id,
                            other
                        );
                    }
                }
            }
        }
    }

    // Clean up
    router.unregister_connection(connection_id).await;
    update_connected_flag(&state, &router).await;

    Ok(())
}

/// Process a single message, returning a response only for immediate commands.
/// Demo-bound commands are queued in state and the demo pushes the response.
async fn process_message(
    text: &str,
    requester_id: u64,
    state: &SharedControlState,
    router: &Arc<RouterState>,
    command_waker: Option<&(dyn Fn() + Send + Sync)>,
) -> Option<ResponseMessage> {
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
                    if let Some(app_sender) = router.app_sender().await {
                        router.remember_route(req.id, requester_id).await;
                        log::info!(
                            "Forwarding request {} to web app from connection {}",
                            req.id,
                            requester_id
                        );
                        if app_sender
                            .send(Message::Text(text.to_string().into()))
                            .is_err()
                        {
                            router.forget_route(req.id).await;
                            return Some(ResponseMessage::error(
                                req.id,
                                ErrorCode::NotConnected,
                                "Web app connection is not available".to_string(),
                            ));
                        }
                    } else if command_waker.is_some() {
                        router.remember_route(req.id, requester_id).await;
                        log::info!(
                            "Queueing native request {} from connection {}",
                            req.id,
                            requester_id
                        );
                        let queued = if let Ok(mut s) = state.write() {
                            s.push_command(req.id, req.command);
                            true
                        } else {
                            false
                        };
                        if !queued {
                            router.forget_route(req.id).await;
                            return Some(ResponseMessage::error(
                                req.id,
                                ErrorCode::Internal,
                                "Failed to access control state".to_string(),
                            ));
                        }
                    } else {
                        log::warn!(
                            "Rejecting request {} from connection {}: no demo app connected",
                            req.id,
                            requester_id
                        );
                        return Some(ResponseMessage::error(
                            req.id,
                            ErrorCode::NotConnected,
                            "No demo app is connected".to_string(),
                        ));
                    }
                    if let Some(wake) = command_waker {
                        wake();
                    }
                    None
                }
            }
        }
        Err(e) => {
            log::warn!("Failed to parse incoming request payload: {}", e);
            Some(ResponseMessage::error(
                0,
                ErrorCode::InvalidCommand,
                format!("Failed to parse request: {}", e),
            ))
        }
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

#[derive(Default)]
struct RouterState {
    next_connection_id: AtomicU64,
    connections: Mutex<HashMap<u64, mpsc::UnboundedSender<Message>>>,
    pending_routes: Mutex<HashMap<u64, u64>>,
    app_connection_id: Mutex<Option<u64>>,
}

impl RouterState {
    async fn register_connection(&self, sender: mpsc::UnboundedSender<Message>) -> u64 {
        let connection_id = self.next_connection_id.fetch_add(1, Ordering::Relaxed) + 1;
        self.connections.lock().await.insert(connection_id, sender);
        connection_id
    }

    async fn unregister_connection(&self, connection_id: u64) {
        self.connections.lock().await.remove(&connection_id);
        let was_app = {
            let mut app = self.app_connection_id.lock().await;
            if *app == Some(connection_id) {
                *app = None;
                true
            } else {
                false
            }
        };
        if was_app {
            self.fail_all_pending_routes(
                ErrorCode::NotConnected,
                "Web app connection closed".to_string(),
            )
            .await;
        }
        self.pending_routes
            .lock()
            .await
            .retain(|_, requester_id| *requester_id != connection_id);
    }

    async fn register_app(&self, connection_id: u64) {
        *self.app_connection_id.lock().await = Some(connection_id);
    }

    async fn app_sender(&self) -> Option<mpsc::UnboundedSender<Message>> {
        let app_connection_id = *self.app_connection_id.lock().await;
        let connections = self.connections.lock().await;
        app_connection_id.and_then(|id| connections.get(&id).cloned())
    }

    async fn is_app_connection(&self, connection_id: u64) -> bool {
        *self.app_connection_id.lock().await == Some(connection_id)
    }

    async fn remember_route(&self, request_id: u64, requester_id: u64) {
        self.pending_routes
            .lock()
            .await
            .insert(request_id, requester_id);
    }

    async fn forget_route(&self, request_id: u64) {
        self.pending_routes.lock().await.remove(&request_id);
    }

    async fn route_response(&self, response: ResponseMessage) {
        let requester_id = self.pending_routes.lock().await.remove(&response.id);
        let Some(requester_id) = requester_id else {
            return;
        };
        let sender = self.connections.lock().await.get(&requester_id).cloned();
        if let Some(sender) = sender {
            let json = serde_json::to_string(&response).unwrap_or_default();
            let _ = sender.send(Message::Text(json.into()));
        } else {
            log::warn!(
                "Dropping response {} because requester connection {} no longer exists",
                response.id,
                requester_id
            );
        }
    }

    async fn has_connected_peer(&self) -> bool {
        !self.connections.lock().await.is_empty()
    }

    async fn fail_all_pending_routes(&self, code: ErrorCode, message: String) {
        let pending = std::mem::take(&mut *self.pending_routes.lock().await);
        let connections = self.connections.lock().await;
        for (request_id, requester_id) in pending {
            if let Some(sender) = connections.get(&requester_id) {
                let response = ResponseMessage::error(request_id, code, message.clone());
                let json = serde_json::to_string(&response).unwrap_or_default();
                let _ = sender.send(Message::Text(json.into()));
            }
        }
    }
}

async fn dispatch_app_responses(state: SharedControlState, router: Arc<RouterState>) {
    let response_notify = state
        .read()
        .ok()
        .map(|guard| guard.response_notify())
        .unwrap_or_else(|| Arc::new(tokio::sync::Notify::new()));

    loop {
        let response = {
            let mut guard = state.write().ok();
            guard.as_mut().and_then(|state| state.pop_response())
        };

        if let Some(response) = response {
            router.route_response(response).await;
            continue;
        }

        response_notify.notified().await;
    }
}

async fn update_connected_flag(state: &SharedControlState, router: &RouterState) {
    let connected = router.has_connected_peer().await;
    if let Ok(mut s) = state.write() {
        s.set_connected(connected);
    }
}

fn is_app_hello_message(text: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return false;
    };
    matches!(
        (
            value.get("type").and_then(Value::as_str),
            value.get("role").and_then(Value::as_str)
        ),
        (Some("appHello"), Some("webApp"))
    )
}
