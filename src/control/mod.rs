//! Control server module for MCP and CLI communication
//!
//! Provides WebSocket-based control protocol for:
//! - Switching demos
//! - Camera control
//! - Screenshots
//! - Status queries
//! - Hot-reload triggering

pub mod protocol;
pub mod state;
pub mod ws_client;
pub mod ws_server;

pub use protocol::{
    Command, ErrorCode, Event, EventMessage, Request, Response, ResponseMessage,
    DEFAULT_WS_PORT, PROTOCOL_VERSION,
};
pub use state::{AppStatus, ControlState, PendingCommand, SharedControlState, new_shared_state};
pub use ws_client::{BlockingWsClient, WsClient};
pub use ws_server::{WsServer, broadcast_event, run_standalone};
