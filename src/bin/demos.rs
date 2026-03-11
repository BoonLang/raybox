//! Unified demo application
//!
//! Run with: cargo run --bin demos --features windowed
//! With control server: cargo run --bin demos --features windowed,control -- --control
//!
//! Controls:
//! - 0-9/-/=: Switch between demos
//! - F: Toggle app stats overlay
//! - G: Toggle full system stats
//! - K: Toggle keybindings display
//! - Esc: Exit

use raybox::demos::{runner, DemoId};

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    let control_mode = args.contains(&"--control".to_string());

    // Check for RAYBOX_DEMO environment variable for initial demo
    let initial_demo = std::env::var("RAYBOX_DEMO")
        .ok()
        .and_then(|s| s.parse::<u8>().ok())
        .and_then(DemoId::from_u8)
        .unwrap_or(DemoId::Objects);

    println!(
        "Starting unified demos with Demo {}: {}",
        initial_demo as u8,
        initial_demo.name()
    );
    println!("Controls: 0-9/-/= switch demos, F/G stats, K keybindings, Esc exit");

    #[cfg(feature = "control")]
    if control_mode {
        println!("Control server enabled on ws://127.0.0.1:9300");
        return runner::run_with_control(initial_demo, None);
    }

    #[cfg(not(feature = "control"))]
    if control_mode {
        eprintln!("Warning: --control flag requires the 'control' feature to be enabled");
    }

    runner::run(initial_demo)
}
