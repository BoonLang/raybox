//! MSDF text rendering module
//!
//! Provides high-quality text rendering using Multi-channel Signed Distance Fields.

pub mod atlas;
pub mod renderer;

pub use atlas::{MsdfAtlas, GlyphMetrics};
pub use renderer::{TextRenderer, GlyphInstance};
