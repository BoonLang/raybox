//! Hot-reload module for development
//!
//! Provides file watching, automatic rebuilding, and state preservation
//! for rapid iteration during development.

pub mod builder;
pub mod shader_loader;
pub mod state;
pub mod watcher;

pub use builder::{BuildMode, BuildResult, Builder};
pub use shader_loader::{HotReloadable, ShaderCompileResult, ShaderLoader};
pub use state::{OverlayModeState, ReloadableState};
pub use watcher::{FileChange, FileWatcher, WatcherConfig};
