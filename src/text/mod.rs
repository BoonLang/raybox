//! Text rendering module
//!
//! Provides high-quality text rendering using:
//! - MSDF (Multi-channel Signed Distance Fields) atlas-based rendering
//! - Vector SDF (exact Bézier curve distance) rendering

pub mod atlas;
pub mod renderer;
pub mod vector_font;
pub mod glyph_atlas;
pub mod vector_renderer;

pub use atlas::{MsdfAtlas, GlyphMetrics};
pub use renderer::{TextRenderer, GlyphInstance};
pub use vector_font::{VectorFont, BezierCurve, VectorGlyphMetrics};
pub use glyph_atlas::{VectorFontAtlas, GlyphAtlasEntry, GridCell};
pub use vector_renderer::{VectorTextRenderer, VectorGlyphInstance};
