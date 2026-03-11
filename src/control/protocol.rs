//! Control protocol for MCP and CLI communication
//!
//! Defines the message format for controlling demo apps via WebSocket.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Protocol version for compatibility checking
pub const PROTOCOL_VERSION: u32 = 1;

/// Default WebSocket server port
pub const DEFAULT_WS_PORT: u16 = 9300;

/// Request message from client to server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// Unique request ID for correlating responses
    pub id: u64,
    /// Protocol version
    pub version: u32,
    /// The command to execute
    pub command: Command,
}

impl Request {
    pub fn new(id: u64, command: Command) -> Self {
        Self {
            id,
            version: PROTOCOL_VERSION,
            command,
        }
    }
}

/// Commands that can be sent to the demo app
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Command {
    /// Switch to a specific demo (0-6)
    SwitchDemo { id: u8 },

    /// Set camera position and/or orientation
    SetCamera {
        position: Option<[f32; 3]>,
        yaw: Option<f32>,
        pitch: Option<f32>,
        roll: Option<f32>,
    },

    /// Take a screenshot and return as base64
    /// Optional center_crop: [width, height] to crop a centered region
    Screenshot {
        #[serde(default)]
        center_crop: Option<[u32; 2]>,
    },

    /// Get current status (demo, camera, FPS, etc.)
    GetStatus,

    /// Toggle overlay mode
    ToggleOverlay { mode: String },

    /// Simulate a key press
    PressKey { key: String },

    /// Reload shaders (for hot-reload)
    ReloadShaders,

    /// Set a named theme on demos that support theme switching
    SetTheme {
        theme: String,
        #[serde(default)]
        dark_mode: Option<bool>,
    },

    /// Mutate an item in a list-style retained scene
    #[serde(alias = "setTodoItem")]
    SetListItem {
        index: u32,
        #[serde(default)]
        completed: Option<bool>,
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        toggle: bool,
    },

    /// Select the visible list filter
    #[serde(alias = "setTodoFilter")]
    SetListFilter { filter: String },

    /// Set list scroll offset in logical pixels
    #[serde(alias = "setTodoScroll")]
    SetListScroll { offset_y: f32 },

    /// Set a named retained scroll root offset in logical pixels
    SetNamedScroll { name: String, offset_y: f32 },

    /// Ping for connection testing
    Ping,
}

/// Response message from server to client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMessage {
    /// Matches the request ID
    pub id: u64,
    /// The response data
    pub response: Response,
}

impl ResponseMessage {
    pub fn new(id: u64, response: Response) -> Self {
        Self { id, response }
    }

    pub fn success(id: u64, data: Option<Value>) -> Self {
        Self::new(id, Response::Success { data })
    }

    pub fn error(id: u64, code: ErrorCode, message: String) -> Self {
        Self::new(id, Response::Error { code, message })
    }
}

/// Response types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Response {
    /// Generic success response
    Success { data: Option<Value> },

    /// Current status
    Status {
        current_demo: u8,
        demo_name: String,
        demo_family: String,
        camera_position: [f32; 3],
        camera_yaw: f32,
        camera_pitch: f32,
        camera_roll: f32,
        fps: f32,
        overlay_mode: String,
        show_keybindings: bool,
    },

    /// Screenshot data
    Screenshot {
        base64: String,
        width: u32,
        height: u32,
    },

    /// Error response
    Error { code: ErrorCode, message: String },

    /// Pong response to Ping
    Pong,
}

/// Error codes for protocol errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    /// Invalid or unknown command
    InvalidCommand,
    /// Invalid demo ID
    InvalidDemoId,
    /// Not connected to a demo app
    NotConnected,
    /// Screenshot capture failed
    ScreenshotFailed,
    /// Protocol version mismatch
    VersionMismatch,
    /// Invalid theme name
    InvalidTheme,
    /// Internal error
    Internal,
}

/// Event notifications sent from server to clients (not in response to a request)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event {
    /// Demo was switched
    DemoChanged { id: u8, name: String },

    /// Shader was reloaded
    ShaderReloaded { shader_name: String },

    /// App is about to shut down
    Shutdown,

    /// Build started (for hot-reload)
    BuildStarted,

    /// Build completed (for hot-reload)
    BuildCompleted {
        success: bool,
        error: Option<String>,
    },

    /// WASM module should be reloaded (web hot-reload)
    WasmReload,
}

/// Wrapper for events with type discrimination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMessage {
    pub event: Event,
}

impl EventMessage {
    pub fn new(event: Event) -> Self {
        Self { event }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization() {
        let req = Request::new(1, Command::SwitchDemo { id: 3 });
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("switchDemo"));
        assert!(json.contains("\"id\":3"));
    }

    #[test]
    fn test_response_serialization() {
        let resp = ResponseMessage::success(1, None);
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("success"));
    }

    #[test]
    fn test_todo_command_deserialization_alias() {
        let json = r#"{
            "id": 2,
            "version": 1,
            "command": {
                "type": "setTodoItem",
                "index": 1,
                "completed": true,
                "toggle": false
            }
        }"#;
        let req: Request = serde_json::from_str(json).unwrap();
        match req.command {
            Command::SetListItem {
                index,
                completed,
                label,
                toggle,
            } => {
                assert_eq!(index, 1);
                assert_eq!(completed, Some(true));
                assert_eq!(label, None);
                assert!(!toggle);
            }
            other => panic!("expected SetListItem alias, got {other:?}"),
        }
    }

    #[test]
    fn test_named_scroll_command_serialization() {
        let req = Request::new(
            3,
            Command::SetNamedScroll {
                name: "feed_scroll".to_string(),
                offset_y: 48.0,
            },
        );
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("setNamedScroll"));
        assert!(json.contains("\"name\":\"feed_scroll\""));
        assert!(json.contains("\"offsetY\":48.0"));
    }

    #[test]
    fn test_list_command_serialization() {
        let req = Request::new(
            4,
            Command::SetListItem {
                index: 1,
                completed: Some(false),
                label: Some("alpha".to_string()),
                toggle: false,
            },
        );
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("setListItem"));
        assert!(json.contains("\"index\":1"));
        assert!(json.contains("\"completed\":false"));
        assert!(json.contains("\"label\":\"alpha\""));
    }
}
