use anyhow::{Context, Result};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::process::Command;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

pub fn run(directory: &str, command: &str) -> Result<()> {
    log::info!("Watching {} and running: {}", directory, command);

    println!("✓ Watching directory: {}", directory);
    println!("  Command: {}", command);
    println!("  Press Ctrl+C to stop");
    println!();

    let path = Path::new(directory);
    if !path.exists() {
        anyhow::bail!("Directory does not exist: {}", directory);
    }

    // Channel for receiving file system events
    let (tx, rx) = channel();

    // Create watcher
    let mut watcher =
        RecommendedWatcher::new(tx, Config::default()).context("Failed to create file watcher")?;

    // Watch the directory recursively
    watcher
        .watch(path, RecursiveMode::Recursive)
        .context(format!("Failed to watch directory: {}", directory))?;

    println!("Watching for changes...");
    println!();

    // Track last execution time to debounce rapid events
    let mut last_execution = Instant::now();
    let debounce_duration = Duration::from_millis(500);

    // Run command once initially
    run_command(command)?;

    // Watch for events
    loop {
        match rx.recv() {
            Ok(Ok(event)) => {
                if should_trigger(&event) {
                    // Debounce: only run if enough time has passed
                    let now = Instant::now();
                    if now.duration_since(last_execution) >= debounce_duration {
                        println!("\n🔄 File changed, running command...\n");
                        run_command(command)?;
                        last_execution = now;
                    }
                }
            }
            Ok(Err(e)) => {
                log::error!("Watch error: {}", e);
            }
            Err(e) => {
                anyhow::bail!("Channel receive error: {}", e);
            }
        }
    }
}

fn should_trigger(event: &Event) -> bool {
    // Only trigger on modify, create, or remove events
    matches!(
        event.kind,
        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
    )
}

fn run_command(command: &str) -> Result<()> {
    let start = Instant::now();

    // Split command into program and args
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        anyhow::bail!("Empty command");
    }

    let program = parts[0];
    let args = &parts[1..];

    // Execute command
    let output = Command::new(program)
        .args(args)
        .output()
        .context(format!("Failed to execute: {}", command))?;

    let duration = start.elapsed();

    // Print stdout
    if !output.stdout.is_empty() {
        print!("{}", String::from_utf8_lossy(&output.stdout));
    }

    // Print stderr
    if !output.stderr.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
    }

    if output.status.success() {
        println!("\n✓ Command completed in {:.2}s", duration.as_secs_f64());
    } else {
        println!(
            "\n✗ Command failed with exit code: {:?}",
            output.status.code()
        );
    }

    println!("\nWatching for changes...");

    Ok(())
}
