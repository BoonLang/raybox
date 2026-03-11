//! Text rendering module
//!
//! Provides high-quality text rendering using Vector SDF (exact Bézier curve distance).

pub mod char_grid;
pub mod glyph_atlas;
pub mod vector_font;

#[allow(unused_imports)]
pub use char_grid::{
    build_char_grid, build_fixed_char_grid, fixed_char_grid_cells_for_instance, CharGrid,
    CharGridCell, FixedCharGridSpec,
};
#[allow(unused_imports)]
pub use glyph_atlas::{GlyphAtlasEntry, GridCell, VectorFontAtlas};
#[allow(unused_imports)]
pub use vector_font::{BezierCurve, VectorFont, VectorGlyphMetrics};
