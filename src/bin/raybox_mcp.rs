//! MCP (Model Context Protocol) server for raybox
//!
//! Provides an MCP-compatible interface for controlling raybox demos
//! from AI assistants like Claude.

use raybox::browser_launch::{
    build_launch_url, default_control_ready_timeout, spawn_chromium, stop_browser,
    wait_for_control_ready, BrowserLaunch, BrowserLaunchConfig,
};
use raybox::control::{run_standalone, BlockingWsClient, Command, Response};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

/// MCP request structure
#[derive(Debug, Deserialize)]
struct McpRequest {
    #[allow(dead_code)]
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
    browser: Option<BrowserLaunch>,
    control_server_started: bool,
}

impl McpServer {
    fn new() -> Self {
        Self {
            client: None,
            browser: None,
            control_server_started: false,
        }
    }

    fn local_control_server_available() -> bool {
        match BlockingWsClient::new() {
            Ok(mut client) => client.connect_local().is_ok(),
            Err(_) => false,
        }
    }

    fn wait_for_control_server_socket(timeout: Duration) -> Result<(), String> {
        let started = Instant::now();
        while started.elapsed() < timeout {
            if Self::local_control_server_available() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(100));
        }
        Err("Timed out waiting for the local control server to start".to_string())
    }

    fn ensure_control_server(&mut self) -> Result<(), String> {
        if Self::local_control_server_available() {
            return Ok(());
        }

        if !self.control_server_started {
            self.control_server_started = true;
            thread::spawn(|| {
                let runtime =
                    tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
                if let Err(error) = runtime.block_on(run_standalone(None)) {
                    log::error!("Embedded control server failed: {error:#}");
                }
            });
        }

        Self::wait_for_control_server_socket(Duration::from_secs(5))
    }

    fn ensure_connected(&mut self) -> Result<(), String> {
        if self.client.is_none() {
            let mut client =
                BlockingWsClient::new().map_err(|e| format!("Failed to create client: {}", e))?;
            client
                .connect_local()
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

    fn stop_browser(&mut self) {
        if let Some(mut browser) = self.browser.take() {
            stop_browser(&mut browser);
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
                        "description": "Switch to a specific demo (0-11). Demos: 0=Empty, 1=Objects, 2=Spheres, 3=Towers, 4=2DText, 5=Clay, 6=TextShadow, 7=TodoMVC, 8=TodoMVC3D, 9=RetainedUI, 10=RetainedUiPhysical, 11=TextPhysical",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "id": {
                                    "type": "integer",
                                    "description": "Demo ID (0-11)",
                                    "minimum": 0,
                                    "maximum": 11
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
                        "description": "Set a named theme for demos that support theme switching. Themes: classic2d, professional, neobrutalism, glassmorphism, neumorphism",
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
                        "name": "set_list_item",
                        "description": "Update an item in a list-style retained scene.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "index": {
                                    "type": "integer",
                                    "description": "Zero-based item index"
                                },
                                "completed": {
                                    "type": "boolean",
                                    "description": "Optional completion state"
                                },
                                "toggle": {
                                    "type": "boolean",
                                    "description": "Toggle completion state instead of setting it explicitly"
                                },
                                "label": {
                                    "type": "string",
                                    "description": "Optional replacement label"
                                }
                            },
                            "required": ["index"]
                        }
                    },
                    {
                        "name": "set_list_filter",
                        "description": "Set the active filter for a list-style retained scene. Supported values: all, active, completed.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "filter": {
                                    "type": "string",
                                    "description": "List filter name (all, active, completed)"
                                }
                            },
                            "required": ["filter"]
                        }
                    },
                    {
                        "name": "set_list_scroll",
                        "description": "Set the scroll offset for a list-style retained scene.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "offset_y": {
                                    "type": "number",
                                    "description": "Vertical list scroll offset in pixels"
                                }
                            },
                            "required": ["offset_y"]
                        }
                    },
                    {
                        "name": "set_named_scroll",
                        "description": "Set the scroll offset for a named retained scroll root.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "name": {
                                    "type": "string",
                                    "description": "Named retained scroll root"
                                },
                                "offset_y": {
                                    "type": "number",
                                    "description": "Vertical scroll offset in pixels"
                                }
                            },
                            "required": ["name", "offset_y"]
                        }
                    },
                    {
                        "name": "capture_demo_screenshot",
                        "description": "Switch to a demo, optionally set a named theme and reset the camera, then capture a screenshot on one stable connection.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "id": {
                                    "type": "integer",
                                    "description": "Demo ID (0-11)",
                                    "minimum": 0,
                                    "maximum": 11
                                },
                                "theme": {
                                    "type": "string",
                                    "description": "Optional theme name for demos that support themes"
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
                    },
                    {
                        "name": "launch_web_browser",
                        "description": "Launch Chromium for the Raybox web app with the supported WebGPU flags and optionally wait for the control connection.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "Base web URL (default http://127.0.0.1:8000)" },
                                "demo": { "type": "integer", "description": "Optional demo ID appended as a query parameter" },
                                "control": { "type": "boolean", "description": "Enable ?control=1 and wait for control readiness (default true)" },
                                "hotreload": { "type": "boolean", "description": "Enable ?hotreload=1" },
                                "headless": { "type": "boolean", "description": "Launch headless Chromium" },
                                "app_mode": { "type": "boolean", "description": "Launch Chromium in app-window mode without normal browser chrome (default false)" },
                                "debug_port": { "type": "integer", "description": "Chromium remote-debugging port (default 9222)" },
                                "chrome_bin": { "type": "string", "description": "Explicit Chromium binary path" },
                                "chrome_args": {
                                    "type": "array",
                                    "items": { "type": "string" },
                                    "description": "Extra Chromium arguments"
                                },
                                "user_data_dir": { "type": "string", "description": "Explicit browser profile directory" },
                                "use_default_profile": { "type": "boolean", "description": "Use the default browser profile instead of an isolated temp profile" },
                                "compat": { "type": "boolean", "description": "Enable the Raybox Linux/WebGPU compatibility flag pack (default true)" },
                                "wait_for_control_ms": { "type": "integer", "description": "Control readiness timeout in milliseconds" }
                            }
                        }
                    },
                    {
                        "name": "close_web_browser",
                        "description": "Close the browser launched by this raybox-mcp instance.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
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
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        if tool_name == "launch_web_browser" {
            self.stop_browser();

            let control = arguments
                .get("control")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let hotreload = arguments
                .get("hotreload")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let demo = arguments
                .get("demo")
                .and_then(|v| v.as_u64())
                .map(|v| v as u8);
            let base_url = arguments
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("http://127.0.0.1:8000");
            let url = match build_launch_url(base_url, demo, control, hotreload) {
                Ok(url) => url,
                Err(error) => {
                    return McpResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id.clone(),
                        result: None,
                        error: Some(McpError {
                            code: -32602,
                            message: error.to_string(),
                        }),
                    }
                }
            };

            let config = BrowserLaunchConfig {
                url,
                chrome_bin: arguments
                    .get("chrome_bin")
                    .and_then(|v| v.as_str())
                    .map(PathBuf::from),
                debug_port: arguments
                    .get("debug_port")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u16)
                    .unwrap_or(raybox::browser_launch::DEFAULT_DEBUG_PORT),
                headless: arguments
                    .get("headless")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                app_mode: arguments
                    .get("app_mode")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                compat: arguments
                    .get("compat")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(cfg!(target_os = "linux")),
                use_default_profile: arguments
                    .get("use_default_profile")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                user_data_dir: arguments
                    .get("user_data_dir")
                    .and_then(|v| v.as_str())
                    .map(PathBuf::from),
                extra_args: arguments
                    .get("chrome_args")
                    .and_then(|v| v.as_array())
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|item| item.as_str().map(str::to_string))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default(),
            };

            let wait_for_control_ms = arguments
                .get("wait_for_control_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or_else(|| {
                    if control {
                        default_control_ready_timeout().as_millis() as u64
                    } else {
                        0
                    }
                });

            if control {
                if let Err(error) = self.ensure_control_server() {
                    return McpResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id.clone(),
                        result: None,
                        error: Some(McpError {
                            code: -32000,
                            message: error,
                        }),
                    };
                }
            }

            let launch = match spawn_chromium(&config) {
                Ok(launch) => launch,
                Err(error) => {
                    return McpResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id.clone(),
                        result: None,
                        error: Some(McpError {
                            code: -32000,
                            message: error.to_string(),
                        }),
                    }
                }
            };
            let summary = serde_json::json!({
                "chrome_bin": launch.chrome_bin.display().to_string(),
                "debug_port": launch.debug_port,
                "url": launch.url,
                "profile_dir": launch.owned_profile_dir.as_ref().map(|path| path.display().to_string()),
            });
            self.browser = Some(launch);

            if control && wait_for_control_ms > 0 {
                if let Err(error) =
                    wait_for_control_ready(Duration::from_millis(wait_for_control_ms))
                {
                    self.stop_browser();
                    return McpResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id.clone(),
                        result: None,
                        error: Some(McpError {
                            code: -32000,
                            message: format!(
                                "browser launched but raybox control never became ready: {}",
                                error
                            ),
                        }),
                    };
                }
                self.client = None;
                if let Err(error) = self.ensure_connected() {
                    return McpResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id.clone(),
                        result: None,
                        error: Some(McpError {
                            code: -32000,
                            message: error,
                        }),
                    };
                }
            }

            return McpResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.clone(),
                result: Some(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string_pretty(&summary).unwrap_or_else(|_| summary.to_string())
                    }]
                })),
                error: None,
            };
        }

        if tool_name == "close_web_browser" {
            self.stop_browser();
            self.client = None;
            return McpResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.clone(),
                result: Some(serde_json::json!({
                    "content": [{ "type": "text", "text": "Closed launched browser" }]
                })),
                error: None,
            };
        }

        // Ensure connected for app-control tools.
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
                    error: Some(McpError {
                        code: -32000,
                        message: e,
                    }),
                };
            }

            if let Err(e) = self.wait_for_demo(demo_id) {
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

            if let Some(theme) = arguments.get("theme").and_then(|v| v.as_str()) {
                if let Err(e) = self.send_command(Command::SetTheme {
                    theme: theme.to_string(),
                    dark_mode: arguments.get("dark_mode").and_then(|v| v.as_bool()),
                }) {
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
                        error: Some(McpError {
                            code: -32000,
                            message: e,
                        }),
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
                    error: Some(McpError {
                        code: -32000,
                        message: e,
                    }),
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
                let yaw = arguments
                    .get("yaw")
                    .and_then(|v| v.as_f64())
                    .map(|v| (v as f32).to_radians());
                let pitch = arguments
                    .get("pitch")
                    .and_then(|v| v.as_f64())
                    .map(|v| (v as f32).to_radians());
                Command::SetCamera {
                    position,
                    yaw,
                    pitch,
                    roll: None,
                }
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
                let theme = arguments
                    .get("theme")
                    .and_then(|v| v.as_str())
                    .unwrap_or("professional")
                    .to_string();
                let dark_mode = arguments.get("dark_mode").and_then(|v| v.as_bool());
                Command::SetTheme { theme, dark_mode }
            }
            "set_list_filter" => {
                let filter = arguments
                    .get("filter")
                    .and_then(|v| v.as_str())
                    .unwrap_or("all")
                    .to_string();
                Command::SetListFilter { filter }
            }
            "set_list_item" => {
                let index = arguments.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                let completed = arguments.get("completed").and_then(|v| v.as_bool());
                let toggle = arguments
                    .get("toggle")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let label = arguments
                    .get("label")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                Command::SetListItem {
                    index,
                    completed,
                    toggle,
                    label,
                }
            }
            "set_list_scroll" => {
                let offset_y = arguments
                    .get("offset_y")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as f32;
                Command::SetListScroll { offset_y }
            }
            "set_named_scroll" => {
                let name = arguments
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let offset_y = arguments
                    .get("offset_y")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as f32;
                Command::SetNamedScroll { name, offset_y }
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
                    Response::Status {
                        current_demo,
                        demo_name,
                        demo_family,
                        camera_position,
                        fps,
                        ..
                    } => {
                        serde_json::json!({
                            "content": [{
                                "type": "text",
                                "text": format!("Demo: {} ({})\nFamily: {}\nCamera: [{:.2}, {:.2}, {:.2}]\nFPS: {:.1}",
                                    demo_name, current_demo, demo_family,
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
