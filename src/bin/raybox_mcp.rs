//! MCP (Model Context Protocol) server for raybox
//!
//! Provides an MCP-compatible interface for controlling raybox demos
//! from AI assistants like Claude.

use raybox::control::{BlockingWsClient, Command, Response};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use std::thread;
use std::time::{Duration, Instant};

/// MCP request structure
#[derive(Debug, Deserialize)]
struct McpRequest {
    jsonrpc: String,
    id: Option<serde_json::Value>,
    method: String,
    params: Option<serde_json::Value>,
}

/// MCP response structure
#[derive(Debug, Serialize)]
struct McpResponse {
    jsonrpc: String,
    id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<McpError>,
}

/// MCP error structure
#[derive(Debug, Serialize)]
struct McpError {
    code: i32,
    message: String,
}

/// MCP server state
struct McpServer {
    client: Option<BlockingWsClient>,
}

impl McpServer {
    fn new() -> Self {
        Self { client: None }
    }

    fn ensure_connected(&mut self) -> Result<(), String> {
        if self.client.is_none() {
            let mut client = BlockingWsClient::new()
                .map_err(|e| format!("Failed to create client: {}", e))?;
            client.connect_local()
                .map_err(|e| format!("Failed to connect: {}", e))?;
            self.client = Some(client);
        }
        Ok(())
    }

    fn send_command(&self, command: Command) -> Result<raybox::control::ResponseMessage, String> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| "Not connected".to_string())?;
        client
            .send_command_with_timeout(command, Duration::from_secs(30))
            .map_err(|e| format!("Command failed: {}", e))
    }

    fn wait_for_demo(&self, demo_id: u8) -> Result<(), String> {
        let started = Instant::now();
        loop {
            let response = self.send_command(Command::GetStatus)?;
            match response.response {
                Response::Status { current_demo, .. } if current_demo == demo_id => return Ok(()),
                Response::Status { .. } => {}
                _ => {}
            }

            if started.elapsed() >= Duration::from_secs(30) {
                return Err(format!("Timed out waiting for demo {}", demo_id));
            }

            thread::sleep(Duration::from_millis(100));
        }
    }

    fn handle_request(&mut self, request: McpRequest) -> McpResponse {
        let response = match request.method.as_str() {
            "initialize" => self.handle_initialize(&request),
            "tools/list" => self.handle_tools_list(&request),
            "tools/call" => self.handle_tools_call(&request),
            "shutdown" => self.handle_shutdown(&request),
            _ => McpResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(McpError {
                    code: -32601,
                    message: format!("Method not found: {}", request.method),
                }),
            },
        };
        response
    }

    fn handle_initialize(&self, request: &McpRequest) -> McpResponse {
        McpResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id.clone(),
            result: Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "raybox-mcp",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })),
            error: None,
        }
    }

    fn handle_tools_list(&self, request: &McpRequest) -> McpResponse {
        McpResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id.clone(),
            result: Some(serde_json::json!({
                "tools": [
                    {
                        "name": "switch_demo",
                        "description": "Switch to a specific demo (0-8). Demos: 0=Empty, 1=Objects, 2=Spheres, 3=Towers, 4=2DText, 5=Clay, 6=TextShadow, 7=TodoMVC, 8=TodoMVC3D",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "id": {
                                    "type": "integer",
                                    "description": "Demo ID (0-8)",
                                    "minimum": 0,
                                    "maximum": 8
                                }
                            },
                            "required": ["id"]
                        }
                    },
                    {
                        "name": "set_camera",
                        "description": "Set camera position and/or orientation",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "position": {
                                    "type": "array",
                                    "items": { "type": "number" },
                                    "minItems": 3,
                                    "maxItems": 3,
                                    "description": "Camera position [x, y, z]"
                                },
                                "yaw": {
                                    "type": "number",
                                    "description": "Camera yaw in degrees"
                                },
                                "pitch": {
                                    "type": "number",
                                    "description": "Camera pitch in degrees"
                                }
                            }
                        }
                    },
                    {
                        "name": "screenshot",
                        "description": "Capture a screenshot and return as base64 PNG. Use center_crop to crop to a centered region.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "center_crop_width": { "type": "integer", "description": "Width of centered crop region (e.g. 700)" },
                                "center_crop_height": { "type": "integer", "description": "Height of centered crop region (e.g. 700)" }
                            }
                        }
                    },
                    {
                        "name": "get_status",
                        "description": "Get current demo status (name, camera, FPS, etc.)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    },
                    {
                        "name": "reload_shaders",
                        "description": "Trigger hot-reload of shaders",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    },
                    {
                        "name": "set_theme",
                        "description": "Set theme for TodoMVC 3D demo. Themes: classic2d, professional, neobrutalism, glassmorphism, neumorphism",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "theme": {
                                    "type": "string",
                                    "description": "Theme name (classic2d, professional, neobrutalism, glassmorphism, neumorphism)"
                                },
                                "dark_mode": {
                                    "type": "boolean",
                                    "description": "Enable dark mode"
                                }
                            },
                            "required": ["theme"]
                        }
                    },
                    {
                        "name": "capture_demo_screenshot",
                        "description": "Switch to a demo, optionally set a TodoMVC 3D theme and reset the camera, then capture a screenshot on one stable connection.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "id": {
                                    "type": "integer",
                                    "description": "Demo ID (0-8)",
                                    "minimum": 0,
                                    "maximum": 8
                                },
                                "theme": {
                                    "type": "string",
                                    "description": "Optional TodoMVC 3D theme"
                                },
                                "dark_mode": {
                                    "type": "boolean",
                                    "description": "Optional dark mode flag"
                                },
                                "reset_camera": {
                                    "type": "boolean",
                                    "description": "Press T before capturing"
                                },
                                "center_crop_width": { "type": "integer", "description": "Width of centered crop region" },
                                "center_crop_height": { "type": "integer", "description": "Height of centered crop region" }
                            },
                            "required": ["id"]
                        }
                    }
                ]
            })),
            error: None,
        }
    }

    fn handle_tools_call(&mut self, request: &McpRequest) -> McpResponse {
        let params = match &request.params {
            Some(p) => p,
            None => {
                return McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id.clone(),
                    result: None,
                    error: Some(McpError {
                        code: -32602,
                        message: "Missing params".to_string(),
                    }),
                };
            }
        };

        let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let arguments = params.get("arguments").cloned().unwrap_or(serde_json::json!({}));

        // Ensure connected
        if let Err(e) = self.ensure_connected() {
            return McpResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.clone(),
                result: None,
                error: Some(McpError {
                    code: -32000,
                    message: e,
                }),
            };
        }

        if tool_name == "capture_demo_screenshot" {
            let demo_id = arguments.get("id").and_then(|v| v.as_u64()).unwrap_or(1) as u8;
            let center_crop = match (
                arguments.get("center_crop_width").and_then(|v| v.as_u64()),
                arguments.get("center_crop_height").and_then(|v| v.as_u64()),
            ) {
                (Some(w), Some(h)) => Some([w as u32, h as u32]),
                _ => None,
            };

            if let Err(e) = self.send_command(Command::SwitchDemo { id: demo_id }) {
                return McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id.clone(),
                    result: None,
                    error: Some(McpError { code: -32000, message: e }),
                };
            }

            if let Err(e) = self.wait_for_demo(demo_id) {
                return McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id.clone(),
                    result: None,
                    error: Some(McpError { code: -32000, message: e }),
                };
            }

            if let Some(theme) = arguments.get("theme").and_then(|v| v.as_str()) {
                if let Err(e) = self.send_command(Command::SetTheme {
                    theme: theme.to_string(),
                    dark_mode: arguments.get("dark_mode").and_then(|v| v.as_bool()),
                }) {
                    return McpResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id.clone(),
                        result: None,
                        error: Some(McpError { code: -32000, message: e }),
                    };
                }
            }

            if arguments
                .get("reset_camera")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                if let Err(e) = self.send_command(Command::PressKey {
                    key: "T".to_string(),
                }) {
                    return McpResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id.clone(),
                        result: None,
                        error: Some(McpError { code: -32000, message: e }),
                    };
                }
            }

            return match self.send_command(Command::Screenshot { center_crop }) {
                Ok(response) => {
                    let content = match response.response {
                        Response::Screenshot { base64, .. } => serde_json::json!({
                            "content": [{
                                "type": "image",
                                "data": base64,
                                "mimeType": "image/png"
                            }]
                        }),
                        Response::Error { message, .. } => serde_json::json!({
                            "content": [{ "type": "text", "text": format!("Error: {}", message) }],
                            "isError": true
                        }),
                        other => serde_json::json!({
                            "content": [{ "type": "text", "text": format!("Unexpected response: {:?}", other) }],
                            "isError": true
                        }),
                    };

                    McpResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id.clone(),
                        result: Some(content),
                        error: None,
                    }
                }
                Err(e) => McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id.clone(),
                    result: None,
                    error: Some(McpError { code: -32000, message: e }),
                },
            };
        }

        let command = match tool_name {
            "switch_demo" => {
                let id = arguments.get("id").and_then(|v| v.as_u64()).unwrap_or(1) as u8;
                Command::SwitchDemo { id }
            }
            "set_camera" => {
                let position = arguments.get("position").and_then(|v| {
                    v.as_array().map(|arr| {
                        let x = arr.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        let y = arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        let z = arr.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        [x, y, z]
                    })
                });
                let yaw = arguments.get("yaw").and_then(|v| v.as_f64()).map(|v| (v as f32).to_radians());
                let pitch = arguments.get("pitch").and_then(|v| v.as_f64()).map(|v| (v as f32).to_radians());
                Command::SetCamera { position, yaw, pitch, roll: None }
            }
            "screenshot" => {
                let center_crop = match (
                    arguments.get("center_crop_width").and_then(|v| v.as_u64()),
                    arguments.get("center_crop_height").and_then(|v| v.as_u64()),
                ) {
                    (Some(w), Some(h)) => Some([w as u32, h as u32]),
                    _ => None,
                };
                Command::Screenshot { center_crop }
            }
            "get_status" => Command::GetStatus,
            "reload_shaders" => Command::ReloadShaders,
            "set_theme" => {
                let theme = arguments.get("theme").and_then(|v| v.as_str()).unwrap_or("professional").to_string();
                let dark_mode = arguments.get("dark_mode").and_then(|v| v.as_bool());
                Command::SetTheme { theme, dark_mode }
            }
            _ => {
                return McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id.clone(),
                    result: None,
                    error: Some(McpError {
                        code: -32602,
                        message: format!("Unknown tool: {}", tool_name),
                    }),
                };
            }
        };

        // Send command to demo app
        match self.send_command(command) {
            Ok(response) => {
                let content = match response.response {
                    Response::Success { data } => {
                        serde_json::json!({
                            "content": [{ "type": "text", "text": data.map(|d| d.to_string()).unwrap_or("OK".to_string()) }]
                        })
                    }
                    Response::Status { current_demo, demo_name, camera_position, fps, .. } => {
                        serde_json::json!({
                            "content": [{
                                "type": "text",
                                "text": format!("Demo: {} ({})\nCamera: [{:.2}, {:.2}, {:.2}]\nFPS: {:.1}",
                                    demo_name, current_demo,
                                    camera_position[0], camera_position[1], camera_position[2],
                                    fps
                                )
                            }]
                        })
                    }
                    Response::Screenshot { base64, .. } => {
                        serde_json::json!({
                            "content": [{
                                "type": "image",
                                "data": base64,
                                "mimeType": "image/png"
                            }]
                        })
                    }
                    Response::Error { message, .. } => {
                        serde_json::json!({
                            "content": [{ "type": "text", "text": format!("Error: {}", message) }],
                            "isError": true
                        })
                    }
                    Response::Pong => {
                        serde_json::json!({
                            "content": [{ "type": "text", "text": "Pong" }]
                        })
                    }
                };

                McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id.clone(),
                    result: Some(content),
                    error: None,
                }
            }
            Err(e) => McpResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.clone(),
                result: None,
                error: Some(McpError {
                    code: -32000,
                    message: e,
                }),
            },
        }
    }

    fn handle_shutdown(&self, request: &McpRequest) -> McpResponse {
        McpResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id.clone(),
            result: Some(serde_json::json!({})),
            error: None,
        }
    }
}

fn main() {
    env_logger::init();

    let mut server = McpServer::new();
    let stdin = io::stdin();
    let stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let request: McpRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Failed to parse request: {}", e);
                continue;
            }
        };

        let response = server.handle_request(request);
        let response_json = serde_json::to_string(&response).unwrap();

        let mut stdout = stdout.lock();
        writeln!(stdout, "{}", response_json).unwrap();
        stdout.flush().unwrap();
    }
}
