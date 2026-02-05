//! Development server with hot-reload
//!
//! Watches source files and shaders, rebuilds automatically,
//! and notifies connected demo apps to reload.

use raybox::control::{
    broadcast_event, Event, WsServer, DEFAULT_WS_PORT,
};
use raybox::hot_reload::{BuildMode, Builder, FileChange, FileWatcher, WatcherConfig};
use std::env;
use std::process::{Child, Command as ProcessCommand, Stdio};
use tokio::sync::broadcast;

struct DevServer {
    builder: Builder,
    watcher: FileWatcher,
    event_tx: broadcast::Sender<raybox::control::EventMessage>,
    build_mode: BuildMode,
    demo_process: Option<Child>,
}

impl DevServer {
    fn new(build_mode: BuildMode, event_tx: broadcast::Sender<raybox::control::EventMessage>) -> anyhow::Result<Self> {
        let builder = Builder::new(".");
        let watcher = FileWatcher::new(WatcherConfig::default())?;

        Ok(Self {
            builder,
            watcher,
            event_tx,
            build_mode,
            demo_process: None,
        })
    }

    fn start_demo(&mut self) -> anyhow::Result<()> {
        log::info!("Starting demo process...");

        let child = match self.build_mode {
            BuildMode::Native => {
                ProcessCommand::new("cargo")
                    .args(["run", "--bin", "demos", "--features", "windowed,control", "--", "--control"])
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .spawn()?
            }
            BuildMode::Web => {
                // For web mode, start the miniserve server
                ProcessCommand::new("miniserve")
                    .args([".", "--port", "8000", "--index", "index.html"])
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .spawn()?
            }
        };

        self.demo_process = Some(child);
        Ok(())
    }

    fn stop_demo(&mut self) {
        if let Some(mut child) = self.demo_process.take() {
            log::info!("Stopping demo process...");
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn restart_demo(&mut self) -> anyhow::Result<()> {
        self.stop_demo();
        self.start_demo()
    }

    fn handle_changes(&mut self, changes: Vec<FileChange>) -> anyhow::Result<()> {
        if changes.is_empty() {
            return Ok(());
        }

        log::info!("Detected {} file change(s)", changes.len());
        for change in &changes {
            match change {
                FileChange::RustSource(path) => log::info!("  Rust: {}", path),
                FileChange::Shader(path) => log::info!("  Shader: {}", path),
                FileChange::Unknown(path) => log::info!("  Unknown: {}", path),
            }
        }

        // Broadcast build started event
        broadcast_event(&self.event_tx, Event::BuildStarted);

        let has_rust = FileWatcher::has_rust_changes(&changes);
        let has_shader = FileWatcher::has_shader_changes(&changes);

        let result = if has_rust {
            // Full rebuild for Rust changes
            log::info!("Rebuilding (Rust changes detected)...");
            match self.build_mode {
                BuildMode::Native => self.builder.build(BuildMode::Native),
                BuildMode::Web => self.builder.build_web_full(),
            }
        } else if has_shader {
            // Shader-only rebuild
            log::info!("Recompiling shaders...");
            self.builder.compile_shaders()
        } else {
            return Ok(());
        };

        if result.success {
            log::info!("Build succeeded in {:.2}s", result.duration.as_secs_f32());

            // Broadcast success
            broadcast_event(&self.event_tx, Event::BuildCompleted {
                success: true,
                error: None,
            });

            // Restart demo for Rust changes
            if has_rust {
                match self.build_mode {
                    BuildMode::Native => {
                        self.restart_demo()?;
                    }
                    BuildMode::Web => {
                        // For web, broadcast reload event so browser reloads WASM
                        log::info!("Broadcasting WASM reload to web clients...");
                        broadcast_event(&self.event_tx, Event::WasmReload);
                    }
                }
            }

            // For shader changes, broadcast reload event
            if has_shader {
                for shader in FileWatcher::changed_shaders(&changes) {
                    broadcast_event(&self.event_tx, Event::ShaderReloaded {
                        shader_name: shader,
                    });
                }
                // Also broadcast WASM reload for web mode shader changes
                if self.build_mode == BuildMode::Web {
                    broadcast_event(&self.event_tx, Event::WasmReload);
                }
            }
        } else {
            log::error!("Build failed:\n{}", result.stderr);

            // Broadcast failure
            broadcast_event(&self.event_tx, Event::BuildCompleted {
                success: false,
                error: Some(result.stderr),
            });
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info")
    ).init();

    let args: Vec<String> = env::args().collect();
    let build_mode = if args.contains(&"--web".to_string()) {
        BuildMode::Web
    } else {
        BuildMode::Native
    };

    log::info!("Starting raybox development server (mode: {:?})", build_mode);

    // Create WebSocket server for event broadcasting
    let ws_server = WsServer::new();
    let event_tx = ws_server.event_sender();

    // Only start WsServer in web mode — native demo starts its own control server on port 9300
    match build_mode {
        BuildMode::Web => {
            let port = DEFAULT_WS_PORT;
            tokio::spawn(async move {
                if let Err(e) = ws_server.run(port).await {
                    log::error!("WebSocket server error: {}", e);
                }
            });
        }
        BuildMode::Native => {
            drop(ws_server);
        }
    }

    // Create dev server
    let mut dev_server = DevServer::new(build_mode, event_tx)?;

    // Initial build
    log::info!("Performing initial build...");
    let result = match build_mode {
        BuildMode::Native => dev_server.builder.build(BuildMode::Native),
        BuildMode::Web => dev_server.builder.build_web_full(),
    };

    if !result.success {
        log::error!("Initial build failed:\n{}", result.stderr);
        return Ok(());
    }
    log::info!("Initial build succeeded in {:.2}s", result.duration.as_secs_f32());

    // Start demo process
    dev_server.start_demo()?;

    log::info!("Watching for file changes...");
    log::info!("Press Ctrl+C to stop");

    // Main loop
    loop {
        // Poll for file changes
        let changes = dev_server.watcher.poll();
        if !changes.is_empty() {
            if let Err(e) = dev_server.handle_changes(changes) {
                log::error!("Error handling changes: {}", e);
            }
        }

        // Check if demo process died
        if let Some(ref mut child) = dev_server.demo_process {
            if let Ok(Some(status)) = child.try_wait() {
                log::warn!("Demo process exited with status: {}", status);
                dev_server.demo_process = None;
            }
        }

        // Small sleep to avoid busy loop
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}
