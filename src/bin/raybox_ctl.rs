//! CLI control tool for raybox
//!
//! Simple command-line interface to control running raybox demos.

use raybox::control::{
    Command, Response, BlockingWsClient, DEFAULT_WS_PORT,
};
use std::env;
use std::fs;

fn print_usage() {
    eprintln!("Usage: raybox-ctl <command> [args]");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  status                  Get current demo status");
    eprintln!("  switch <id>             Switch to demo (0-6)");
    eprintln!("  screenshot [--output <path>]  Take screenshot");
    eprintln!("  camera <x> <y> <z>      Set camera position");
    eprintln!("  reload                  Reload shaders");
    eprintln!("  ping                    Test connection");
    eprintln!();
    eprintln!("Demo IDs:");
    eprintln!("  0 = Empty");
    eprintln!("  1 = Objects");
    eprintln!("  2 = Spheres");
    eprintln!("  3 = Towers");
    eprintln!("  4 = 2D Text");
    eprintln!("  5 = Clay Tablet");
    eprintln!("  6 = Text Shadow");
}

fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    let command = &args[1];

    // Create and connect client
    let mut client = match BlockingWsClient::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = client.connect_local() {
        eprintln!("Failed to connect to raybox (is it running with --control?): {}", e);
        std::process::exit(1);
    }

    let cmd = match command.as_str() {
        "status" => Command::GetStatus,
        "switch" => {
            if args.len() < 3 {
                eprintln!("Usage: raybox-ctl switch <id>");
                std::process::exit(1);
            }
            let id: u8 = match args[2].parse() {
                Ok(id) if id <= 6 => id,
                _ => {
                    eprintln!("Invalid demo ID. Must be 0-6.");
                    std::process::exit(1);
                }
            };
            Command::SwitchDemo { id }
        }
        "screenshot" => {
            Command::Screenshot
        }
        "camera" => {
            if args.len() < 5 {
                eprintln!("Usage: raybox-ctl camera <x> <y> <z>");
                std::process::exit(1);
            }
            let x: f32 = args[2].parse().unwrap_or(0.0);
            let y: f32 = args[3].parse().unwrap_or(0.0);
            let z: f32 = args[4].parse().unwrap_or(4.0);
            Command::SetCamera {
                position: Some([x, y, z]),
                yaw: None,
                pitch: None,
                roll: None,
            }
        }
        "reload" => Command::ReloadShaders,
        "ping" => Command::Ping,
        "help" | "--help" | "-h" => {
            print_usage();
            std::process::exit(0);
        }
        _ => {
            eprintln!("Unknown command: {}", command);
            print_usage();
            std::process::exit(1);
        }
    };

    // Send command
    match client.send_command(cmd.clone()) {
        Ok(response) => {
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
                    camera_position,
                    camera_yaw,
                    camera_pitch,
                    camera_roll,
                    fps,
                    overlay_mode,
                    show_keybindings,
                } => {
                    println!("Demo: {} ({})", demo_name, current_demo);
                    println!("Camera Position: [{:.2}, {:.2}, {:.2}]",
                        camera_position[0], camera_position[1], camera_position[2]);
                    println!("Camera Angles: yaw={:.1}°, pitch={:.1}°, roll={:.1}°",
                        camera_yaw.to_degrees(), camera_pitch.to_degrees(), camera_roll.to_degrees());
                    println!("FPS: {:.1}", fps);
                    println!("Overlay: {}", overlay_mode);
                    println!("Show Keybindings: {}", show_keybindings);
                }
                Response::Screenshot { base64, width, height } => {
                    // Find output path from args
                    let output_path = args.iter()
                        .position(|a| a == "--output" || a == "-o")
                        .and_then(|i| args.get(i + 1))
                        .map(|s| s.as_str())
                        .unwrap_or("screenshot.png");

                    // Decode and save
                    match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &base64) {
                        Ok(data) => {
                            if let Err(e) = fs::write(output_path, &data) {
                                eprintln!("Failed to write screenshot: {}", e);
                                std::process::exit(1);
                            }
                            println!("Screenshot saved to {} ({}x{})", output_path, width, height);
                        }
                        Err(e) => {
                            eprintln!("Failed to decode screenshot: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                Response::Error { code, message } => {
                    eprintln!("Error ({:?}): {}", code, message);
                    std::process::exit(1);
                }
                Response::Pong => {
                    println!("Pong!");
                }
            }
        }
        Err(e) => {
            eprintln!("Command failed: {}", e);
            std::process::exit(1);
        }
    }
}
