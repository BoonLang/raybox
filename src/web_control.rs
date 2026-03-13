//! Web WebSocket client for control protocol
//!
//! Allows the web version to be controlled remotely via WebSocket.

#![allow(dead_code)]

use serde_json::{json, Value};
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// Command received from control server
#[derive(Debug, Clone)]
pub enum WebCommand {
    SwitchDemo(u8),
    SetCamera {
        position: Option<[f32; 3]>,
        yaw: Option<f32>,
        pitch: Option<f32>,
        roll: Option<f32>,
    },
    SetTheme {
        theme: String,
        dark_mode: Option<bool>,
    },
    SetListItem {
        index: u32,
        completed: Option<bool>,
        label: Option<String>,
        toggle: bool,
    },
    SetListFilter {
        filter: String,
    },
    SetListScroll {
        offset_y: f32,
    },
    SetNamedScroll {
        name: String,
        offset_y: f32,
    },
    Screenshot {
        center_crop: Option<[u32; 2]>,
    },
    GetStatus,
    ToggleOverlay(String),
    PressKey(String),
    Ping,
    /// Trigger WASM hot-reload (handled by JavaScript, not Rust)
    Reload,
}

/// Response to send back to control server
#[derive(Debug, Clone)]
pub struct WebResponse {
    pub id: u64,
    pub data: String, // JSON string
}

/// Web control state (shared via Rc<RefCell>)
pub struct WebControlState {
    pending_commands: VecDeque<(u64, WebCommand)>,
    pending_responses: VecDeque<WebResponse>,
    connected: bool,
    last_received_message: Option<String>,
    last_sent_message: Option<String>,
    /// Flag indicating a reload was requested (JavaScript should handle this)
    reload_requested: bool,
}

impl WebControlState {
    pub fn new() -> Self {
        Self {
            pending_commands: VecDeque::new(),
            pending_responses: VecDeque::new(),
            connected: false,
            last_received_message: None,
            last_sent_message: None,
            reload_requested: false,
        }
    }

    pub fn push_command(&mut self, id: u64, command: WebCommand) {
        self.pending_commands.push_back((id, command));
    }

    pub fn pop_command(&mut self) -> Option<(u64, WebCommand)> {
        self.pending_commands.pop_front()
    }

    pub fn push_response(&mut self, response: WebResponse) {
        self.pending_responses.push_back(response);
    }

    pub fn pop_response(&mut self) -> Option<WebResponse> {
        self.pending_responses.pop_front()
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn set_connected(&mut self, connected: bool) {
        self.connected = connected;
    }

    pub fn set_last_received_message(&mut self, message: String) {
        self.last_received_message = Some(message);
    }

    pub fn set_last_sent_message(&mut self, message: String) {
        self.last_sent_message = Some(message);
    }

    pub fn pending_command_count(&self) -> usize {
        self.pending_commands.len()
    }

    pub fn pending_response_count(&self) -> usize {
        self.pending_responses.len()
    }

    pub fn last_received_message(&self) -> Option<&str> {
        self.last_received_message.as_deref()
    }

    pub fn last_sent_message(&self) -> Option<&str> {
        self.last_sent_message.as_deref()
    }

    pub fn request_reload(&mut self) {
        self.reload_requested = true;
    }

    pub fn take_reload_request(&mut self) -> bool {
        let requested = self.reload_requested;
        self.reload_requested = false;
        requested
    }
}

pub type SharedWebControlState = Rc<RefCell<WebControlState>>;

pub fn new_shared_state() -> SharedWebControlState {
    Rc::new(RefCell::new(WebControlState::new()))
}

/// WebSocket client for web control
pub struct WebWsClient {
    socket: web_sys::WebSocket,
    state: SharedWebControlState,
    hello_sent: Rc<Cell<bool>>,
}

impl WebWsClient {
    /// Create a new WebSocket client and connect to the control server
    pub fn connect(url: &str, state: SharedWebControlState) -> Result<Self, JsValue> {
        let socket = web_sys::WebSocket::new(url)?;
        socket.set_binary_type(web_sys::BinaryType::Arraybuffer);
        let hello_sent = Rc::new(Cell::new(false));

        // Set up event handlers
        let state_clone = state.clone();
        let socket_clone = socket.clone();
        let hello_sent_clone = hello_sent.clone();
        let onopen_callback = Closure::wrap(Box::new(move |_: web_sys::Event| {
            log::info!("WebSocket connected to control server");
            state_clone.borrow_mut().set_connected(true);
            if socket_clone
                .send_with_str(r#"{"type":"appHello","role":"webApp"}"#)
                .is_ok()
            {
                hello_sent_clone.set(true);
            }
        }) as Box<dyn FnMut(_)>);
        socket.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
        onopen_callback.forget();

        let state_clone = state.clone();
        let hello_sent_clone = hello_sent.clone();
        let onclose_callback = Closure::wrap(Box::new(move |_: web_sys::CloseEvent| {
            log::info!("WebSocket disconnected from control server");
            state_clone.borrow_mut().set_connected(false);
            hello_sent_clone.set(false);
        }) as Box<dyn FnMut(_)>);
        socket.set_onclose(Some(onclose_callback.as_ref().unchecked_ref()));
        onclose_callback.forget();

        let onerror_callback = Closure::wrap(Box::new(move |e: web_sys::ErrorEvent| {
            log::error!("WebSocket error: {:?}", e.message());
        }) as Box<dyn FnMut(_)>);
        socket.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
        onerror_callback.forget();

        let state_clone = state.clone();
        let onmessage_callback = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
            if let Some(text) = e.data().as_string() {
                state_clone
                    .borrow_mut()
                    .set_last_received_message(text.clone());
                // Parse the incoming JSON message
                if let Some((id, cmd)) = parse_command(&text) {
                    state_clone.borrow_mut().push_command(id, cmd);
                }
            }
        }) as Box<dyn FnMut(_)>);
        socket.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        onmessage_callback.forget();

        Ok(Self {
            socket,
            state,
            hello_sent,
        })
    }

    /// Connect to localhost with default port
    pub fn connect_local(state: SharedWebControlState) -> Result<Self, JsValue> {
        Self::connect("ws://127.0.0.1:9300", state)
    }

    /// Send a response back to the server
    pub fn send_response(&self, response: &WebResponse) -> Result<(), JsValue> {
        self.socket.send_with_str(&response.data)
    }

    /// Poll for pending responses and send them
    pub fn flush_responses(&self) {
        self.send_hello_if_needed();
        loop {
            let response = { self.state.borrow_mut().pop_response() };
            let Some(response) = response else {
                break;
            };
            if let Err(e) = self.send_response(&response) {
                log::error!("Failed to send response: {:?}", e);
            } else {
                self.state
                    .borrow_mut()
                    .set_last_sent_message(response.data.clone());
            }
        }
    }

    fn send_hello_if_needed(&self) {
        if self.hello_sent.get() {
            return;
        }
        if self.socket.ready_state() != web_sys::WebSocket::OPEN {
            return;
        }
        if self
            .socket
            .send_with_str(r#"{"type":"appHello","role":"webApp"}"#)
            .is_ok()
        {
            self.hello_sent.set(true);
        }
    }
}

/// Parse a JSON command message
fn parse_command(json: &str) -> Option<(u64, WebCommand)> {
    // Simple JSON parsing without serde (to avoid adding more deps for wasm)
    // Expected format: {"id": N, "version": 1, "command": {"type": "...", ...}}
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let id = value.get("id")?.as_u64()?;
    let command = value.get("command")?;
    let raw_cmd_type = command.get("type")?.as_str()?;
    let cmd_type = match raw_cmd_type {
        "setTodoItem" => "setListItem",
        "setTodoFilter" => "setListFilter",
        "setTodoScroll" => "setListScroll",
        other => other,
    };

    let cmd = match cmd_type {
        "switchDemo" => {
            let demo_id = command.get("id")?.as_u64()? as u8;
            WebCommand::SwitchDemo(demo_id)
        }
        "setCamera" => {
            let position = command.get("position").and_then(|p| {
                let arr = p.as_array()?;
                if arr.len() == 3 {
                    Some([
                        arr[0].as_f64()? as f32,
                        arr[1].as_f64()? as f32,
                        arr[2].as_f64()? as f32,
                    ])
                } else {
                    None
                }
            });
            let yaw = command
                .get("yaw")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32);
            let pitch = command
                .get("pitch")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32);
            let roll = command
                .get("roll")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32);
            WebCommand::SetCamera {
                position,
                yaw,
                pitch,
                roll,
            }
        }
        "setListItem" => {
            let index = command.get("index")?.as_u64()? as u32;
            let completed = command.get("completed").and_then(|v| v.as_bool());
            let label = command
                .get("label")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let toggle = command
                .get("toggle")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            WebCommand::SetListItem {
                index,
                completed,
                label,
                toggle,
            }
        }
        "setTheme" => {
            let theme = command.get("theme")?.as_str()?.to_string();
            let dark_mode = command.get("darkMode").and_then(|v| v.as_bool());
            WebCommand::SetTheme { theme, dark_mode }
        }
        "setListFilter" => {
            let filter = command.get("filter")?.as_str()?.to_string();
            WebCommand::SetListFilter { filter }
        }
        "setListScroll" => {
            let offset_y = command.get("offsetY")?.as_f64()? as f32;
            WebCommand::SetListScroll { offset_y }
        }
        "setNamedScroll" => {
            let name = command.get("name")?.as_str()?.to_string();
            let offset_y = command.get("offsetY")?.as_f64()? as f32;
            WebCommand::SetNamedScroll { name, offset_y }
        }
        "screenshot" => {
            let center_crop = command.get("centerCrop").and_then(|crop| {
                let arr = crop.as_array()?;
                if arr.len() != 2 {
                    return None;
                }
                Some([arr[0].as_u64()? as u32, arr[1].as_u64()? as u32])
            });
            WebCommand::Screenshot { center_crop }
        }
        "getStatus" => WebCommand::GetStatus,
        "toggleOverlay" => {
            let mode = command.get("mode")?.as_str()?.to_string();
            WebCommand::ToggleOverlay(mode)
        }
        "pressKey" => {
            let key = command.get("key")?.as_str()?.to_string();
            WebCommand::PressKey(key)
        }
        "ping" => WebCommand::Ping,
        "reload" | "reloadWasm" => WebCommand::Reload,
        _ => return None,
    };

    Some((id, cmd))
}

/// Create a JSON response
pub fn create_response(id: u64, response_type: &str, data: Option<&str>) -> WebResponse {
    let response = match response_type {
        "pong" => json!({ "type": "pong" }),
        "success" => success_body(data.and_then(parse_json_value)),
        other => {
            let mut response = serde_json::Map::new();
            response.insert("type".to_string(), Value::String(other.to_string()));
            if let Some(extra) = data
                .and_then(parse_json_value)
                .and_then(|value| value.as_object().cloned())
            {
                response.extend(extra);
            }
            Value::Object(response)
        }
    };
    serialize_response(id, response)
}

/// Create a success response
pub fn success_response(id: u64, data: Option<&str>) -> WebResponse {
    serialize_response(id, success_body(data.and_then(parse_json_value)))
}

/// Create an error response
pub fn error_response(id: u64, code: &str, message: &str) -> WebResponse {
    serialize_response(
        id,
        json!({
            "type": "error",
            "code": normalize_error_code(code),
            "message": message,
        }),
    )
}

/// Create a status response
pub fn status_response(
    id: u64,
    current_demo: u8,
    demo_name: &str,
    demo_family: &str,
    camera_pos: [f32; 3],
    camera_yaw: f32,
    camera_pitch: f32,
    camera_roll: f32,
    fps: f32,
    overlay_mode: &str,
    show_keybindings: bool,
) -> WebResponse {
    let camera_pos = [
        sanitize_f32(camera_pos[0]),
        sanitize_f32(camera_pos[1]),
        sanitize_f32(camera_pos[2]),
    ];
    serialize_response(
        id,
        json!({
            "type": "status",
            "current_demo": current_demo,
            "demo_name": demo_name,
            "demo_family": demo_family,
            "camera_position": camera_pos,
            "camera_yaw": sanitize_f32(camera_yaw),
            "camera_pitch": sanitize_f32(camera_pitch),
            "camera_roll": sanitize_f32(camera_roll),
            "fps": sanitize_f32(fps),
            "overlay_mode": overlay_mode,
            "show_keybindings": show_keybindings,
        }),
    )
}

/// Create a pong response
pub fn pong_response(id: u64) -> WebResponse {
    serialize_response(id, json!({ "type": "pong" }))
}

/// Create a screenshot response (base64 encoded PNG)
pub fn screenshot_response(id: u64, base64_data: &str, width: u32, height: u32) -> WebResponse {
    serialize_response(
        id,
        json!({
            "type": "screenshot",
            "base64": base64_data,
            "width": width,
            "height": height,
        }),
    )
}

fn serialize_response(id: u64, response: Value) -> WebResponse {
    let data = serde_json::to_string(&json!({
        "id": id,
        "response": response,
    }))
    .unwrap_or_else(|_| {
        format!(
            r#"{{"id":{},"response":{{"type":"error","code":"internal","message":"response serialization failed"}}}}"#,
            id
        )
    });
    WebResponse { id, data }
}

fn parse_json_value(raw: &str) -> Option<Value> {
    serde_json::from_str(raw).ok()
}

fn success_body(data: Option<Value>) -> Value {
    match data {
        Some(data) => json!({
            "type": "success",
            "data": data,
        }),
        None => json!({
            "type": "success",
        }),
    }
}

fn normalize_error_code(raw: &str) -> &'static str {
    match raw {
        "InvalidCommand" | "invalidCommand" => "invalidCommand",
        "InvalidDemoId" | "invalidDemoId" => "invalidDemoId",
        "NotConnected" | "notConnected" => "notConnected",
        "ScreenshotFailed" | "screenshotFailed" => "screenshotFailed",
        "VersionMismatch" | "versionMismatch" => "versionMismatch",
        "InvalidTheme" | "invalidTheme" => "invalidTheme",
        _ => "internal",
    }
}

fn sanitize_f32(value: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}
