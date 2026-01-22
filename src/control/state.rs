//! Control state management
//!
//! Shared state between the WebSocket server and the demo app.

use super::protocol::{Command, Response, ResponseMessage};
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

/// Pending command with response channel info
#[derive(Debug)]
pub struct PendingCommand {
    pub id: u64,
    pub command: Command,
}

/// Control state shared between WebSocket server and demo app
#[derive(Debug, Default)]
pub struct ControlState {
    /// Pending commands to be processed by the demo app
    pending_commands: VecDeque<PendingCommand>,
    /// Responses from the demo app to be sent to clients
    pending_responses: VecDeque<ResponseMessage>,
    /// Whether a client is connected
    connected: bool,
}

impl ControlState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a command to be processed
    pub fn push_command(&mut self, id: u64, command: Command) {
        self.pending_commands.push_back(PendingCommand { id, command });
    }

    /// Take the next pending command (if any)
    pub fn pop_command(&mut self) -> Option<PendingCommand> {
        self.pending_commands.pop_front()
    }

    /// Check if there are pending commands
    pub fn has_commands(&self) -> bool {
        !self.pending_commands.is_empty()
    }

    /// Add a response to be sent
    pub fn push_response(&mut self, response: ResponseMessage) {
        self.pending_responses.push_back(response);
    }

    /// Take the next pending response (if any)
    pub fn pop_response(&mut self) -> Option<ResponseMessage> {
        self.pending_responses.pop_front()
    }

    /// Check if there are pending responses
    pub fn has_responses(&self) -> bool {
        !self.pending_responses.is_empty()
    }

    /// Set connection status
    pub fn set_connected(&mut self, connected: bool) {
        self.connected = connected;
    }

    /// Check if a client is connected
    pub fn is_connected(&self) -> bool {
        self.connected
    }
}

/// Thread-safe wrapper for ControlState
pub type SharedControlState = Arc<RwLock<ControlState>>;

/// Create a new shared control state
pub fn new_shared_state() -> SharedControlState {
    Arc::new(RwLock::new(ControlState::new()))
}

/// App status snapshot for GetStatus command
#[derive(Debug, Clone)]
pub struct AppStatus {
    pub current_demo: u8,
    pub demo_name: String,
    pub camera_position: [f32; 3],
    pub camera_yaw: f32,
    pub camera_pitch: f32,
    pub camera_roll: f32,
    pub fps: f32,
    pub overlay_mode: String,
    pub show_keybindings: bool,
}

impl AppStatus {
    pub fn to_response(&self) -> Response {
        Response::Status {
            current_demo: self.current_demo,
            demo_name: self.demo_name.clone(),
            camera_position: self.camera_position,
            camera_yaw: self.camera_yaw,
            camera_pitch: self.camera_pitch,
            camera_roll: self.camera_roll,
            fps: self.fps,
            overlay_mode: self.overlay_mode.clone(),
            show_keybindings: self.show_keybindings,
        }
    }
}
