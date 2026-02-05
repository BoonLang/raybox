//! Text rendering module
//!
//! Provides high-quality text rendering using Vector SDF (exact Bézier curve distance).

pub mod vector_font;
pub mod glyph_atlas;

#[allow(unused_imports)]
pub use vector_font::{VectorFont, BezierCurve, VectorGlyphMetrics};
#[allow(unused_imports)]
pub use glyph_atlas::{VectorFontAtlas, GlyphAtlasEntry, GridCell};
