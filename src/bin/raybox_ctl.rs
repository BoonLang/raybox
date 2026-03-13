//! CLI control tool for raybox
//!
//! Simple command-line interface to control running raybox demos.

use raybox::browser_launch::{
    build_launch_url, default_control_ready_timeout, spawn_chromium, stop_browser,
    wait_for_control_ready, BrowserLaunchConfig,
};
use raybox::control::{run_standalone, BlockingWsClient, Command, Response};
use raybox::demo_core::DemoId;
use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command as ProcessCommand, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn print_usage() {
    eprintln!("Usage: raybox-ctl [--timeout-ms <ms>] <command> [args]");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  status                  Get current demo status");
    eprintln!("  switch <id>             Switch to demo (0-11)");
    eprintln!("  screenshot [--output <path>] [--crop WxH]  Take screenshot");
    eprintln!("  capture-demo <id> [--theme <name>] [--dark] [--reset-camera] [--output <path>] [--crop WxH] [--settle-ms <ms>]");
    eprintln!("                          Switch, optionally theme/reset, then take screenshot on one connection");
    eprintln!("  web-open [--url <url>] [--demo <id>] [--control] [--hotreload] [--headless]");
    eprintln!("                          Launch Chromium with the supported Raybox WebGPU flags");
    eprintln!(
        "  web-smoke [--url <url>] [--demo <id>] [--output <path>] [--crop WxH] [--headless]"
    );
    eprintln!(
        "                          Launch Chromium, wait for control, then capture a screenshot"
    );
    eprintln!("  camera <x> <y> <z>      Set camera position");
    eprintln!("  pressKey <key>          Simulate key press (e.g. T, R)");
    eprintln!("  theme <name> [--dark]   Set theme (classic2d, professional, neobrutalism, glassmorphism, neumorphism)");
    eprintln!("  list-toggle <index>     Toggle an item in a list-style retained scene");
    eprintln!("  list-complete <index> <on|off>  Set a list item completion state");
    eprintln!("  list-label <index> <text...>    Set a list item label");
    eprintln!("  list-filter <name>      Set a list filter (all, active, completed)");
    eprintln!("  list-scroll <offset-y>  Set a list scroll offset");
    eprintln!("  todo-toggle <index>     Compatibility alias for list-toggle");
    eprintln!("  todo-complete <index> <on|off>  Compatibility alias for list-complete");
    eprintln!("  todo-label <index> <text...>    Compatibility alias for list-label");
    eprintln!("  todo-filter <name>      Compatibility alias for list-filter");
    eprintln!("  todo-scroll <offset-y>  Compatibility alias for list-scroll");
    eprintln!("  scroll <name> <offset-y>  Set a named retained scroll root offset");
    eprintln!("  reload                  Reload shaders");
    eprintln!("  ping                    Test connection");
    eprintln!();
    eprintln!("Global options:");
    eprintln!("  --timeout-ms <ms>       Command timeout in milliseconds (default: 30000)");
    eprintln!("  --chrome-bin <path>     Chromium/Chrome binary override for web-open/web-smoke");
    eprintln!("  --browser-log <path>    Write Chromium stdout/stderr to this log file");
    eprintln!("  --chrome-arg <arg>      Extra Chromium argument (repeatable)");
    eprintln!("  --app                   Launch visible Chromium in app-window mode");
    eprintln!("  --no-app                Launch a normal Chromium window with browser chrome");
    eprintln!(
        "  --debug-port <port>     Remote debugging port for web-open/web-smoke (default: 9222)"
    );
    eprintln!(
        "  --user-data-dir <path>  Browser profile directory (default: isolated temp profile)"
    );
    eprintln!("  --use-default-profile   Launch against the browser's default profile");
    eprintln!("  --no-compat             Disable the Raybox Linux/WebGPU compatibility flag pack");
    eprintln!(
        "  --wait-for-control-ms <ms>  Override control readiness wait time for web-open/web-smoke"
    );
    eprintln!();
    eprintln!("Demo IDs:");
    eprintln!("  0 = Empty");
    eprintln!("  1 = Objects");
    eprintln!("  2 = Spheres");
    eprintln!("  3 = Towers");
    eprintln!("  4 = 2D Text");
    eprintln!("  5 = Clay Tablet");
    eprintln!("  6 = Text Shadow");
    eprintln!("  7 = TodoMVC");
    eprintln!("  8 = TodoMVC 3D");
    eprintln!("  9 = Retained UI");
    eprintln!("  10 = Retained UI Physical");
    eprintln!("  11 = Text Physical");
}

fn parse_flag_value(args: &[String], names: &[&str]) -> Option<String> {
    args.iter()
        .position(|a| names.iter().any(|name| a == name))
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|arg| arg == name)
}

fn local_control_server_available() -> bool {
    match BlockingWsClient::new() {
        Ok(mut client) => client.connect_local().is_ok(),
        Err(_) => false,
    }
}

fn wait_for_control_server_socket(timeout: Duration) -> anyhow::Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if local_control_server_available() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    anyhow::bail!("timed out waiting for the local control server to start");
}

fn spawn_control_server_process() -> anyhow::Result<Child> {
    let exe = env::current_exe()?;
    let mut command = ProcessCommand::new(exe);
    command
        .arg("control-server")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    command.process_group(0);
    let child = command.spawn()?;
    Ok(child)
}

fn ensure_control_server_process(timeout: Duration) -> anyhow::Result<Option<Child>> {
    if local_control_server_available() {
        return Ok(None);
    }

    let mut child = spawn_control_server_process()?;
    if let Err(error) = wait_for_control_server_socket(timeout) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }

    Ok(Some(child))
}

fn stop_control_server_process(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn parse_repeated_flag_values(args: &[String], name: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == name {
            if let Some(value) = args.get(i + 1) {
                values.push(value.clone());
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    values
}

fn parse_center_crop(args: &[String]) -> Option<[u32; 2]> {
    parse_flag_value(args, &["--crop"]).and_then(|s| {
        let parts: Vec<&str> = s.split('x').collect();
        if parts.len() != 2 {
            return None;
        }
        Some([parts[0].parse::<u32>().ok()?, parts[1].parse::<u32>().ok()?])
    })
}

fn parse_timeout_ms(args: &[String]) -> u64 {
    parse_flag_value(args, &["--timeout-ms"])
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(30_000)
}

fn command_index(args: &[String]) -> Option<usize> {
    let mut i = 1usize;
    while i < args.len() {
        match args[i].as_str() {
            "--timeout-ms" => i += 2,
            _ if args[i].starts_with("--") => i += 1,
            _ => return Some(i),
        }
    }
    None
}

fn screenshot_output_path(args: &[String]) -> String {
    parse_flag_value(args, &["--output", "-o"]).unwrap_or_else(|| "screenshot.png".to_string())
}

fn parse_demo_flag(args: &[String]) -> Option<u8> {
    parse_flag_value(args, &["--demo"]).and_then(|value| match value.parse::<u8>() {
        Ok(id) if DemoId::from_u8(id).is_some() => Some(id),
        _ => None,
    })
}

fn parse_wait_for_control_ms(args: &[String], control: bool) -> u64 {
    parse_flag_value(args, &["--wait-for-control-ms"])
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_else(|| {
            if control {
                default_control_ready_timeout().as_millis() as u64
            } else {
                0
            }
        })
}

fn parse_browser_launch_config(args: &[String], default_control: bool) -> BrowserLaunchConfig {
    let base_url =
        parse_flag_value(args, &["--url"]).unwrap_or_else(|| "http://127.0.0.1:8000".to_string());
    let control = default_control || has_flag(args, "--control");
    let hotreload = has_flag(args, "--hotreload");
    let demo = parse_demo_flag(args);
    let url = build_launch_url(&base_url, demo, control, hotreload).unwrap_or(base_url);

    BrowserLaunchConfig {
        url,
        chrome_bin: parse_flag_value(args, &["--chrome-bin"]).map(PathBuf::from),
        log_path: parse_flag_value(args, &["--browser-log"]).map(PathBuf::from),
        debug_port: parse_flag_value(args, &["--debug-port"])
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(raybox::browser_launch::DEFAULT_DEBUG_PORT),
        headless: has_flag(args, "--headless"),
        app_mode: !has_flag(args, "--headless") && has_flag(args, "--app"),
        compat: !has_flag(args, "--no-compat"),
        use_default_profile: has_flag(args, "--use-default-profile"),
        user_data_dir: parse_flag_value(args, &["--user-data-dir"]).map(PathBuf::from),
        extra_args: parse_repeated_flag_values(args, "--chrome-arg"),
    }
}

fn send_command(
    client: &BlockingWsClient,
    command: Command,
    timeout_ms: u64,
) -> raybox::control::ResponseMessage {
    match client.send_command_with_timeout(command, Duration::from_millis(timeout_ms)) {
        Ok(response) => response,
        Err(e) => {
            eprintln!("Command failed: {}", e);
            std::process::exit(1);
        }
    }
}

fn print_response(response: raybox::control::ResponseMessage, screenshot_output_path: &str) {
    match response.response {
        Response::Success { data } => {
            if let Some(d) = data {
                println!("{}", serde_json::to_string_pretty(&d).unwrap());
            } else {
                println!("OK");
            }
        }
        Response::Status {
            current_demo,
            demo_name,
            demo_family,
            camera_position,
            camera_yaw,
            camera_pitch,
            camera_roll,
            fps,
            overlay_mode,
            show_keybindings,
        } => {
            println!("Demo: {} ({})", demo_name, current_demo);
            println!("Family: {}", demo_family);
            println!(
                "Camera Position: [{:.2}, {:.2}, {:.2}]",
                camera_position[0], camera_position[1], camera_position[2]
            );
            println!(
                "Camera Angles: yaw={:.1}°, pitch={:.1}°, roll={:.1}°",
                camera_yaw.to_degrees(),
                camera_pitch.to_degrees(),
                camera_roll.to_degrees()
            );
            println!("FPS: {:.1}", fps);
            println!("Overlay: {}", overlay_mode);
            println!("Show Keybindings: {}", show_keybindings);
        }
        Response::Screenshot {
            base64,
            width,
            height,
        } => match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &base64) {
            Ok(data) => {
                if let Err(e) = fs::write(screenshot_output_path, &data) {
                    eprintln!("Failed to write screenshot: {}", e);
                    std::process::exit(1);
                }
                println!(
                    "Screenshot saved to {} ({}x{})",
                    screenshot_output_path, width, height
                );
            }
            Err(e) => {
                eprintln!("Failed to decode screenshot: {}", e);
                std::process::exit(1);
            }
        },
        Response::Error { code, message } => {
            eprintln!("Error ({:?}): {}", code, message);
            std::process::exit(1);
        }
        Response::Pong => {
            println!("Pong!");
        }
    }
}

fn wait_for_demo(client: &BlockingWsClient, demo_id: u8, timeout_ms: u64) {
    let started = std::time::Instant::now();
    loop {
        let response = send_command(client, Command::GetStatus, timeout_ms);
        match response.response {
            Response::Status { current_demo, .. } if current_demo == demo_id => return,
            Response::Status { .. } => {}
            _ => {}
        }

        if started.elapsed() >= Duration::from_millis(timeout_ms) {
            eprintln!("Timed out waiting for demo {}", demo_id);
            std::process::exit(1);
        }

        thread::sleep(Duration::from_millis(100));
    }
}

fn handle_capture_demo(
    client: &BlockingWsClient,
    args: &[String],
    timeout_ms: u64,
    command_idx: usize,
) {
    if args.len() <= command_idx + 1 {
        eprintln!(
            "Usage: raybox-ctl capture-demo <id> [--theme <name>] [--dark] [--reset-camera] [--output <path>] [--crop WxH] [--settle-ms <ms>]"
        );
        std::process::exit(1);
    }

    let demo_id: u8 = match args[command_idx + 1].parse() {
        Ok(id) if DemoId::from_u8(id).is_some() => id,
        _ => {
            eprintln!("Invalid demo ID. Must be 0-11.");
            std::process::exit(1);
        }
    };

    let settle_ms = parse_flag_value(args, &["--settle-ms"])
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(150);
    let output_path = screenshot_output_path(args);
    let center_crop = parse_center_crop(args);

    print_response(
        send_command(client, Command::SwitchDemo { id: demo_id }, timeout_ms),
        &output_path,
    );
    wait_for_demo(client, demo_id, timeout_ms);

    if let Some(theme) = parse_flag_value(args, &["--theme"]) {
        let dark_mode = if has_flag(args, "--dark") {
            Some(true)
        } else {
            None
        };
        print_response(
            send_command(client, Command::SetTheme { theme, dark_mode }, timeout_ms),
            &output_path,
        );
    }

    if has_flag(args, "--reset-camera") {
        print_response(
            send_command(
                client,
                Command::PressKey {
                    key: "T".to_string(),
                },
                timeout_ms,
            ),
            &output_path,
        );
    }

    if settle_ms > 0 {
        thread::sleep(Duration::from_millis(settle_ms));
    }

    print_response(
        send_command(client, Command::Screenshot { center_crop }, timeout_ms),
        &output_path,
    );
}

fn handle_web_open(args: &[String]) {
    let control = has_flag(args, "--control");
    let wait_ms = parse_wait_for_control_ms(args, control);
    let config = parse_browser_launch_config(args, control);
    let mut control_server = if control {
        match ensure_control_server_process(Duration::from_secs(5)) {
            Ok(server) => server,
            Err(error) => {
                eprintln!("Failed to start local control server: {error:#}");
                std::process::exit(1);
            }
        }
    } else {
        None
    };

    let mut launch = match spawn_chromium(&config) {
        Ok(launch) => launch,
        Err(error) => {
            eprintln!("Failed to launch Chromium: {error:#}");
            std::process::exit(1);
        }
    };

    println!("Launched Chromium: {}", launch.chrome_bin.display());
    println!("URL: {}", launch.url);
    println!("Debug Port: {}", launch.debug_port);
    if let Some(log_path) = &launch.log_path {
        println!("Browser Log: {}", log_path.display());
    }
    if let Some(profile) = &launch.owned_profile_dir {
        println!("Profile: {}", profile.display());
    }
    if control_server.is_some() {
        println!("Started local control server on ws://127.0.0.1:9300");
    }

    if wait_ms > 0 {
        match wait_for_control_ready(Duration::from_millis(wait_ms)) {
            Ok(response) => {
                println!("Control ready.");
                print_response(response, &screenshot_output_path(args));
            }
            Err(error) => {
                eprintln!("Browser launched, but the web app never became ready: {error:#}");
                stop_browser(&mut launch);
                if let Some(server) = control_server.as_mut() {
                    stop_control_server_process(server);
                }
                std::process::exit(1);
            }
        }
    }
}

fn handle_web_smoke(args: &[String]) {
    let mut config = parse_browser_launch_config(args, true);
    if !config.url.contains("control=1") {
        config.url = build_launch_url(&config.url, None, true, false).unwrap_or(config.url);
    }

    let wait_ms = parse_wait_for_control_ms(args, true).max(1);
    let output_path = screenshot_output_path(args);
    let center_crop = parse_center_crop(args);
    let mut control_server = match ensure_control_server_process(Duration::from_secs(5)) {
        Ok(server) => server,
        Err(error) => {
            eprintln!("Failed to start local control server: {error:#}");
            std::process::exit(1);
        }
    };

    let mut launch = match spawn_chromium(&config) {
        Ok(launch) => launch,
        Err(error) => {
            eprintln!("Failed to launch Chromium: {error:#}");
            std::process::exit(1);
        }
    };

    let smoke_result = (|| -> anyhow::Result<()> {
        let response = wait_for_control_ready(Duration::from_millis(wait_ms))?;
        print_response(response, &output_path);

        let mut client = BlockingWsClient::new()?;
        client.connect_local()?;
        let response = send_command(&client, Command::Screenshot { center_crop }, wait_ms);
        print_response(response, &output_path);
        Ok(())
    })();

    stop_browser(&mut launch);
    if let Some(server) = control_server.as_mut() {
        stop_control_server_process(server);
    }

    if let Err(error) = smoke_result {
        eprintln!("Web smoke failed: {error:#}");
        std::process::exit(1);
    }
}

fn handle_control_server() {
    let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    if let Err(error) = runtime.block_on(run_standalone(None)) {
        eprintln!("Control server failed: {error:#}");
        std::process::exit(1);
    }
}

fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    let timeout_ms = parse_timeout_ms(&args);
    let command_idx = match command_index(&args) {
        Some(idx) => idx,
        None => {
            print_usage();
            std::process::exit(1);
        }
    };
    let command = args[command_idx].clone();

    if matches!(command.as_str(), "help" | "--help" | "-h") {
        print_usage();
        std::process::exit(0);
    }

    if command == "web-open" {
        handle_web_open(&args);
        return;
    }

    if command == "web-smoke" {
        handle_web_smoke(&args);
        return;
    }

    if command == "control-server" {
        handle_control_server();
        return;
    }

    let mut client = match BlockingWsClient::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = client.connect_local() {
        eprintln!(
            "Failed to connect to raybox (is it running with --control?): {}",
            e
        );
        std::process::exit(1);
    }

    if command == "capture-demo" {
        handle_capture_demo(&client, &args, timeout_ms, command_idx);
        return;
    }

    let cmd = match command.as_str() {
        "status" => Command::GetStatus,
        "switch" => {
            if args.len() <= command_idx + 1 {
                eprintln!("Usage: raybox-ctl switch <id>");
                std::process::exit(1);
            }
            let id: u8 = match args[command_idx + 1].parse() {
                Ok(id) if DemoId::from_u8(id).is_some() => id,
                _ => {
                    eprintln!("Invalid demo ID. Must be 0-11.");
                    std::process::exit(1);
                }
            };
            Command::SwitchDemo { id }
        }
        "screenshot" => Command::Screenshot {
            center_crop: parse_center_crop(&args),
        },
        "camera" => {
            if args.len() <= command_idx + 3 {
                eprintln!("Usage: raybox-ctl camera <x> <y> <z>");
                std::process::exit(1);
            }
            let x: f32 = args[command_idx + 1].parse().unwrap_or(0.0);
            let y: f32 = args[command_idx + 2].parse().unwrap_or(0.0);
            let z: f32 = args[command_idx + 3].parse().unwrap_or(4.0);
            Command::SetCamera {
                position: Some([x, y, z]),
                yaw: None,
                pitch: None,
                roll: None,
            }
        }
        "pressKey" => {
            if args.len() <= command_idx + 1 {
                eprintln!("Usage: raybox-ctl pressKey <key>");
                std::process::exit(1);
            }
            Command::PressKey {
                key: args[command_idx + 1].clone(),
            }
        }
        "theme" => {
            if args.len() <= command_idx + 1 {
                eprintln!("Usage: raybox-ctl theme <name> [--dark]");
                std::process::exit(1);
            }
            let dark_mode = if has_flag(&args, "--dark") {
                Some(true)
            } else {
                None
            };
            Command::SetTheme {
                theme: args[command_idx + 1].clone(),
                dark_mode,
            }
        }
        "todo-toggle" | "list-toggle" => {
            if args.len() <= command_idx + 1 {
                eprintln!("Usage: raybox-ctl {} <index>", command);
                std::process::exit(1);
            }
            let index: u32 = match args[command_idx + 1].parse() {
                Ok(index) => index,
                Err(_) => {
                    eprintln!("Invalid list item index.");
                    std::process::exit(1);
                }
            };
            Command::SetListItem {
                index,
                completed: None,
                label: None,
                toggle: true,
            }
        }
        "todo-complete" | "list-complete" => {
            if args.len() <= command_idx + 2 {
                eprintln!("Usage: raybox-ctl {} <index> <on|off>", command);
                std::process::exit(1);
            }
            let index: u32 = match args[command_idx + 1].parse() {
                Ok(index) => index,
                Err(_) => {
                    eprintln!("Invalid list item index.");
                    std::process::exit(1);
                }
            };
            let completed = match args[command_idx + 2].to_ascii_lowercase().as_str() {
                "on" | "true" | "1" | "yes" => true,
                "off" | "false" | "0" | "no" => false,
                _ => {
                    eprintln!("Completion must be on or off.");
                    std::process::exit(1);
                }
            };
            Command::SetListItem {
                index,
                completed: Some(completed),
                label: None,
                toggle: false,
            }
        }
        "todo-label" | "list-label" => {
            if args.len() <= command_idx + 2 {
                eprintln!("Usage: raybox-ctl {} <index> <text...>", command);
                std::process::exit(1);
            }
            let index: u32 = match args[command_idx + 1].parse() {
                Ok(index) => index,
                Err(_) => {
                    eprintln!("Invalid list item index.");
                    std::process::exit(1);
                }
            };
            let label = args[command_idx + 2..].join(" ");
            Command::SetListItem {
                index,
                completed: None,
                label: Some(label),
                toggle: false,
            }
        }
        "todo-filter" | "list-filter" => {
            if args.len() <= command_idx + 1 {
                eprintln!("Usage: raybox-ctl {} <all|active|completed>", command);
                std::process::exit(1);
            }
            Command::SetListFilter {
                filter: args[command_idx + 1].clone(),
            }
        }
        "todo-scroll" | "list-scroll" => {
            if args.len() <= command_idx + 1 {
                eprintln!("Usage: raybox-ctl {} <offset-y>", command);
                std::process::exit(1);
            }
            let offset_y: f32 = match args[command_idx + 1].parse() {
                Ok(offset_y) => offset_y,
                Err(_) => {
                    eprintln!("Invalid scroll offset.");
                    std::process::exit(1);
                }
            };
            Command::SetListScroll { offset_y }
        }
        "scroll" => {
            if args.len() <= command_idx + 2 {
                eprintln!("Usage: raybox-ctl scroll <name> <offset-y>");
                std::process::exit(1);
            }
            let name = args[command_idx + 1].clone();
            let offset_y: f32 = match args[command_idx + 2].parse() {
                Ok(offset_y) => offset_y,
                Err(_) => {
                    eprintln!("Invalid scroll offset.");
                    std::process::exit(1);
                }
            };
            Command::SetNamedScroll { name, offset_y }
        }
        "reload" => Command::ReloadShaders,
        "ping" => Command::Ping,
        _ => {
            eprintln!("Unknown command: {}", command);
            print_usage();
            std::process::exit(1);
        }
    };

    let output_path = screenshot_output_path(&args);
    let response = send_command(&client, cmd, timeout_ms);
    print_response(response, &output_path);
}
