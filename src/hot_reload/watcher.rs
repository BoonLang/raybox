//! File watcher for hot-reload
//!
//! Watches source files and shaders for changes.

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

/// File change event
#[derive(Debug, Clone)]
pub enum FileChange {
    /// Rust source file changed
    RustSource(String),
    /// Shader file changed
    Shader(String),
    /// Unknown file type
    Unknown(String),
}

impl FileChange {
    fn from_path(path: &Path) -> Option<Self> {
        let path_str = path.to_string_lossy().to_string();

        if let Some(ext) = path.extension() {
            match ext.to_str() {
                Some("rs") => Some(FileChange::RustSource(path_str)),
                Some("slang") | Some("wgsl") => Some(FileChange::Shader(path_str)),
                _ => None,
            }
        } else {
            None
        }
    }
}

/// File watcher configuration
pub struct WatcherConfig {
    /// Paths to watch
    pub watch_paths: Vec<String>,
    /// Debounce duration (to avoid multiple events for single save)
    pub debounce: Duration,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            watch_paths: vec![
                "src".to_string(),
                "shaders".to_string(),
            ],
            debounce: Duration::from_millis(500),
        }
    }
}

/// File watcher for hot-reload
pub struct FileWatcher {
    _watcher: RecommendedWatcher,
    receiver: Receiver<Result<Event, notify::Error>>,
    last_event_time: std::time::Instant,
    debounce: Duration,
}

impl FileWatcher {
    /// Create a new file watcher
    pub fn new(config: WatcherConfig) -> anyhow::Result<Self> {
        let (tx, rx) = channel();

        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default().with_poll_interval(Duration::from_millis(100)),
        )?;

        // Watch configured paths
        for path in &config.watch_paths {
            let path = Path::new(path);
            if path.exists() {
                watcher.watch(path, RecursiveMode::Recursive)?;
                log::info!("Watching: {}", path.display());
            } else {
                log::warn!("Watch path does not exist: {}", path.display());
            }
        }

        Ok(Self {
            _watcher: watcher,
            receiver: rx,
            last_event_time: std::time::Instant::now(),
            debounce: config.debounce,
        })
    }

    /// Poll for file changes (non-blocking)
    pub fn poll(&mut self) -> Vec<FileChange> {
        let mut changes = Vec::new();

        // Collect all pending events
        while let Ok(result) = self.receiver.try_recv() {
            if let Ok(event) = result {
                // Only handle modify and create events
                if !matches!(
                    event.kind,
                    notify::EventKind::Modify(_) | notify::EventKind::Create(_)
                ) {
                    continue;
                }

                for path in event.paths {
                    if let Some(change) = FileChange::from_path(&path) {
                        changes.push(change);
                    }
                }
            }
        }

        // Debounce: only return changes if enough time has passed
        if !changes.is_empty() {
            let now = std::time::Instant::now();
            if now.duration_since(self.last_event_time) >= self.debounce {
                self.last_event_time = now;
                // Deduplicate changes
                let mut unique_changes = Vec::new();
                for change in changes {
                    let path = match &change {
                        FileChange::RustSource(p) => p,
                        FileChange::Shader(p) => p,
                        FileChange::Unknown(p) => p,
                    };
                    if !unique_changes.iter().any(|c| {
                        match c {
                            FileChange::RustSource(p2) | FileChange::Shader(p2) | FileChange::Unknown(p2) => path == p2,
                        }
                    }) {
                        unique_changes.push(change);
                    }
                }
                return unique_changes;
            }
        }

        Vec::new()
    }

    /// Check if any Rust source files changed
    pub fn has_rust_changes(changes: &[FileChange]) -> bool {
        changes.iter().any(|c| matches!(c, FileChange::RustSource(_)))
    }

    /// Check if any shader files changed
    pub fn has_shader_changes(changes: &[FileChange]) -> bool {
        changes.iter().any(|c| matches!(c, FileChange::Shader(_)))
    }

    /// Get the shader names that changed
    pub fn changed_shaders(changes: &[FileChange]) -> Vec<String> {
        changes
            .iter()
            .filter_map(|c| {
                if let FileChange::Shader(path) = c {
                    Path::new(path)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect()
    }
}
