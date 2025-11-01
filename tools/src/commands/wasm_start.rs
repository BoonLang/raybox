use anyhow::{Context, Result};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::thread;

use crate::commands::wasm_build;

pub fn run(release: bool, open_browser: bool, port: u16) -> Result<()> {
    println!("Starting WASM development server...");
    println!();

    // Initial build
    println!("=== Initial Build ===");
    wasm_build::run(release)?;
    println!();

    // Generate initial build ID
    let build_id = Arc::new(Mutex::new(generate_build_id()));
    write_build_id(&build_id.lock().unwrap())?;

    // Start file watcher
    let build_id_clone = Arc::clone(&build_id);
    let watcher_thread = thread::spawn(move || {
        if let Err(e) = run_watcher(release, build_id_clone) {
            eprintln!("Watcher error: {}", e);
        }
    });

    println!("=== Development Server ===");
    println!("  URL: http://localhost:{}", port);
    println!("  Watching: renderer/src/");
    println!("  Press Ctrl+C to stop");
    println!();

    // Start HTTP server in background thread
    let server_port = port;
    let server_build_id = Arc::clone(&build_id);
    let server_thread = thread::spawn(move || {
        if let Err(e) = start_server(server_port, server_build_id) {
            eprintln!("Server error: {}", e);
        }
    });

    // Give server time to start
    thread::sleep(Duration::from_millis(500));

    // Open browser AFTER server starts
    if open_browser {
        let url = format!("http://localhost:{}", port);
        println!("Opening browser: {}", url);

        // Use Chrome explicitly on Linux to avoid opening Firefox
        #[cfg(target_os = "linux")]
        {
            if let Err(e) = std::process::Command::new("google-chrome")
                .arg(&url)
                .arg("--new-window")
                .arg("--remote-debugging-port=9222")
                .arg("--enable-unsafe-webgpu")
                .arg("--enable-webgpu-developer-features")
                .spawn()
            {
                eprintln!("Failed to open Chrome: {}", e);
                eprintln!("Falling back to default browser...");
                if let Err(e) = open::that(&url) {
                    eprintln!("Failed to open browser: {}", e);
                }
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            if let Err(e) = open::that(&url) {
                eprintln!("Failed to open browser: {}", e);
            }
        }
    }

    // Wait for both threads
    // Note: They both run forever, so this never returns (Ctrl+C kills)
    let _ = server_thread.join();
    watcher_thread.join().unwrap();

    Ok(())
}

fn run_watcher(release: bool, build_id: Arc<Mutex<String>>) -> Result<()> {
    let (tx, rx) = channel();

    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        match res {
            Ok(event) => {
                if let EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) = event.kind {
                    // Watch Rust files, WGSL shaders, and Cargo.toml
                    if event.paths.iter().any(|p| {
                        let ext = p.extension().and_then(|e| e.to_str());
                        ext == Some("rs") || ext == Some("wgsl") || p.ends_with("Cargo.toml")
                    }) {
                        let _ = tx.send(());
                    }
                }
            }
            Err(e) => eprintln!("Watch error: {:?}", e),
        }
    })?;

    // Watch renderer/src directory (includes .rs and .wgsl files)
    watcher.watch(Path::new("renderer/src"), RecursiveMode::Recursive)?;
    watcher.watch(Path::new("renderer/Cargo.toml"), RecursiveMode::NonRecursive)?;

    let mut last_rebuild = SystemTime::now();

    println!("👀 Watching for file changes...");
    println!();

    loop {
        // Wait for file change with debouncing
        if rx.recv_timeout(Duration::from_millis(100)).is_ok() {
            let now = SystemTime::now();
            let elapsed = now.duration_since(last_rebuild).unwrap_or(Duration::from_secs(0));

            // Debounce: wait 300ms after last change
            if elapsed < Duration::from_millis(300) {
                continue;
            }

            // Drain any pending events
            while rx.try_recv().is_ok() {}

            last_rebuild = now;

            println!("📝 File change detected, rebuilding...");
            println!();

            match wasm_build::run(release) {
                Ok(_) => {
                    let new_id = generate_build_id();
                    *build_id.lock().unwrap() = new_id.clone();
                    write_build_id(&new_id)?;
                    println!();
                    println!("✅ Build complete! Browser will reload...");
                    println!();
                    println!("👀 Watching for file changes...");
                    println!();
                }
                Err(e) => {
                    eprintln!();
                    eprintln!("❌ Build failed:");
                    eprintln!("{:#}", e);
                    eprintln!();
                    eprintln!("Waiting for fixes...");
                    eprintln!();
                }
            }
        }
    }
}

fn generate_build_id() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .to_string()
}

fn write_build_id(id: &str) -> Result<()> {
    std::fs::create_dir_all("web/_api")?;
    std::fs::write("web/_api/build_id", id)?;
    Ok(())
}

fn start_server(port: u16, build_id: Arc<Mutex<String>>) -> Result<()> {
    use axum::{
        routing::get,
        Router,
    };
    use tower_http::services::ServeDir;
    use tokio::net::TcpListener;

    let app = Router::new()
        // Build ID endpoint for reload detection
        .route("/_api/build_id", get(move || {
            let id = build_id.lock().unwrap().clone();
            async move { id }
        }))
        // Serve reference directory (for layout JSON)
        .nest_service("/reference", ServeDir::new("reference"))
        // Serve static files from web directory
        .nest_service("/", ServeDir::new("web"));

    tokio::runtime::Runtime::new()?
        .block_on(async {
            let listener = TcpListener::bind(("127.0.0.1", port)).await?;
            axum::serve(listener, app).await
        })
        .context("Failed to start HTTP server")?;

    Ok(())
}
