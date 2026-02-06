//! Text rendering module
//!
//! Provides high-quality text rendering using Vector SDF (exact Bézier curve distance).

pub mod vector_font;
pub mod glyph_atlas;
pub mod char_grid;

#[allow(unused_imports)]
pub use vector_font::{VectorFont, BezierCurve, VectorGlyphMetrics};
#[allow(unused_imports)]
pub use glyph_atlas::{VectorFontAtlas, GlyphAtlasEntry, GridCell};
#[allow(unused_imports)]
pub use char_grid::{build_char_grid, CharGrid, CharGridCell};
