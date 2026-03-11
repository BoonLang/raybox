//! CLI control tool for raybox
//!
//! Simple command-line interface to control running raybox demos.

use raybox::control::{BlockingWsClient, Command, Response};
use raybox::demo_core::DemoId;
use std::env;
use std::fs;
use std::thread;
use std::time::Duration;

fn print_usage() {
    eprintln!("Usage: raybox-ctl [--timeout-ms <ms>] <command> [args]");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  status                  Get current demo status");
    eprintln!("  switch <id>             Switch to demo (0-11)");
    eprintln!("  screenshot [--output <path>] [--crop WxH]  Take screenshot");
    eprintln!("  capture-demo <id> [--theme <name>] [--dark] [--reset-camera] [--output <path>] [--crop WxH] [--settle-ms <ms>]");
    eprintln!("                          Switch, optionally theme/reset, then take screenshot on one connection");
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
