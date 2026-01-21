//! Vector font loading and Bézier curve extraction
//!
//! Parses TTF fonts and extracts glyph outlines as quadratic Bézier curves
//! for GPU-based exact SDF computation.

use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use ttf_parser::{Face, OutlineBuilder};

/// A quadratic Bézier curve with bounding box for early rejection
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct BezierCurve {
    /// Control points: [x0, y0, x1, y1, x2, y2, 0, 0]
    /// For quadratic: p0 = start, p1 = control, p2 = end
    pub points: [f32; 8],
    /// Bounding box [min_x, min_y, max_x, max_y]
    pub bbox: [f32; 4],
    /// Curve flags (reserved for future use)
    pub flags: u32,
    /// Padding for 16-byte alignment
    pub _padding: [u32; 3],
}

impl BezierCurve {
    /// Create a quadratic Bézier curve
    pub fn quadratic(p0: (f32, f32), p1: (f32, f32), p2: (f32, f32)) -> Self {
        let min_x = p0.0.min(p1.0).min(p2.0);
        let min_y = p0.1.min(p1.1).min(p2.1);
        let max_x = p0.0.max(p1.0).max(p2.0);
        let max_y = p0.1.max(p1.1).max(p2.1);

        Self {
            points: [p0.0, p0.1, p1.0, p1.1, p2.0, p2.1, 0.0, 0.0],
            bbox: [min_x, min_y, max_x, max_y],
            flags: 0,
            _padding: [0; 3],
        }
    }

    /// Create a line segment as a degenerate quadratic (control point at midpoint)
    pub fn line(p0: (f32, f32), p1: (f32, f32)) -> Self {
        let mid = ((p0.0 + p1.0) * 0.5, (p0.1 + p1.1) * 0.5);
        Self::quadratic(p0, mid, p1)
    }

    /// Check if this curve's bounding box intersects with an AABB
    pub fn intersects_aabb(&self, aabb: &[f32; 4]) -> bool {
        self.bbox[0] <= aabb[2]
            && self.bbox[2] >= aabb[0]
            && self.bbox[1] <= aabb[3]
            && self.bbox[3] >= aabb[1]
    }

    /// Get the start point
    pub fn p0(&self) -> (f32, f32) {
        (self.points[0], self.points[1])
    }

    /// Get the control point
    pub fn p1(&self) -> (f32, f32) {
        (self.points[2], self.points[3])
    }

    /// Get the end point
    pub fn p2(&self) -> (f32, f32) {
        (self.points[4], self.points[5])
    }
}

/// Metrics for a single glyph
#[derive(Debug, Clone)]
pub struct VectorGlyphMetrics {
    /// Unicode codepoint
    pub codepoint: u32,
    /// Advance width in em units
    pub advance: f32,
    /// Glyph bounding box [left, bottom, right, top] in em units
    pub bounds: [f32; 4],
    /// Index of first curve in the global curve array
    pub curve_start: u32,
    /// Number of curves for this glyph
    pub curve_count: u32,
}

/// A parsed vector font with extracted Bézier curves
#[derive(Debug)]
pub struct VectorFont {
    /// All curves from all glyphs, densely packed
    pub curves: Vec<BezierCurve>,
    /// Per-glyph metrics, indexed by codepoint
    pub glyphs: HashMap<u32, VectorGlyphMetrics>,
    /// Font em size
    pub units_per_em: f32,
    /// Ascender height in em units
    pub ascender: f32,
    /// Descender depth in em units (negative)
    pub descender: f32,
    /// Line height in em units
    pub line_height: f32,
}

impl VectorFont {
    /// Parse a TTF font from bytes
    pub fn from_ttf(data: &[u8]) -> Result<Self, &'static str> {
        let face = Face::parse(data, 0).map_err(|_| "Failed to parse TTF")?;

        let units_per_em = face.units_per_em() as f32;
        let ascender = face.ascender() as f32 / units_per_em;
        let descender = face.descender() as f32 / units_per_em;
        let line_height = face.height() as f32 / units_per_em;

        let mut font = VectorFont {
            curves: Vec::new(),
            glyphs: HashMap::new(),
            units_per_em,
            ascender,
            descender,
            line_height,
        };

        // Extract curves for ASCII printable range
        for codepoint in 32u32..=126 {
            if let Some(ch) = char::from_u32(codepoint) {
                if let Some(glyph_id) = face.glyph_index(ch) {
                    let advance = face
                        .glyph_hor_advance(glyph_id)
                        .map(|a| a as f32 / units_per_em)
                        .unwrap_or(0.5);

                    let curve_start = font.curves.len() as u32;
                    let mut builder = CurveBuilder::new(units_per_em);

                    // Get glyph outline
                    let bounds = if let Some(rect) = face.outline_glyph(glyph_id, &mut builder) {
                        [
                            rect.x_min as f32 / units_per_em,
                            rect.y_min as f32 / units_per_em,
                            rect.x_max as f32 / units_per_em,
                            rect.y_max as f32 / units_per_em,
                        ]
                    } else {
                        // Space or empty glyph
                        [0.0, 0.0, advance, 1.0]
                    };

                    let curves = builder.finish();
                    let curve_count = curves.len() as u32;
                    font.curves.extend(curves);

                    font.glyphs.insert(
                        codepoint,
                        VectorGlyphMetrics {
                            codepoint,
                            advance,
                            bounds,
                            curve_start,
                            curve_count,
                        },
                    );
                }
            }
        }

        Ok(font)
    }

    /// Get glyph metrics for a character
    pub fn get_glyph(&self, ch: char) -> Option<&VectorGlyphMetrics> {
        self.glyphs.get(&(ch as u32))
    }

    /// Get all curves for a glyph
    pub fn get_glyph_curves(&self, glyph: &VectorGlyphMetrics) -> &[BezierCurve] {
        let start = glyph.curve_start as usize;
        let end = start + glyph.curve_count as usize;
        &self.curves[start..end]
    }
}

/// Builder that collects Bézier curves from font outlines
struct CurveBuilder {
    curves: Vec<BezierCurve>,
    current_pos: (f32, f32),
    start_pos: (f32, f32),
    scale: f32,
}

impl CurveBuilder {
    fn new(units_per_em: f32) -> Self {
        Self {
            curves: Vec::new(),
            current_pos: (0.0, 0.0),
            start_pos: (0.0, 0.0),
            scale: 1.0 / units_per_em,
        }
    }

    fn finish(self) -> Vec<BezierCurve> {
        self.curves
    }
}

impl OutlineBuilder for CurveBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        let pos = (x * self.scale, y * self.scale);
        self.current_pos = pos;
        self.start_pos = pos;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let end = (x * self.scale, y * self.scale);
        self.curves.push(BezierCurve::line(self.current_pos, end));
        self.current_pos = end;
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let ctrl = (x1 * self.scale, y1 * self.scale);
        let end = (x * self.scale, y * self.scale);
        self.curves
            .push(BezierCurve::quadratic(self.current_pos, ctrl, end));
        self.current_pos = end;
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        // Convert cubic to quadratics using subdivision
        // Simple approximation: split cubic into two quadratics
        let p0 = self.current_pos;
        let p1 = (x1 * self.scale, y1 * self.scale);
        let p2 = (x2 * self.scale, y2 * self.scale);
        let p3 = (x * self.scale, y * self.scale);

        // de Casteljau subdivision at t=0.5
        let q0 = p0;
        let q1 = ((p0.0 + p1.0) * 0.5, (p0.1 + p1.1) * 0.5);
        let q2 = (
            (p0.0 + 2.0 * p1.0 + p2.0) * 0.25,
            (p0.1 + 2.0 * p1.1 + p2.1) * 0.25,
        );
        let q3 = (
            (p0.0 + 3.0 * p1.0 + 3.0 * p2.0 + p3.0) * 0.125,
            (p0.1 + 3.0 * p1.1 + 3.0 * p2.1 + p3.1) * 0.125,
        );
        let q4 = (
            (p1.0 + 2.0 * p2.0 + p3.0) * 0.25,
            (p1.1 + 2.0 * p2.1 + p3.1) * 0.25,
        );
        let q5 = ((p2.0 + p3.0) * 0.5, (p2.1 + p3.1) * 0.5);
        let q6 = p3;

        // First half: approximate cubic [q0, q1, q2, q3] as quadratic
        // Use midpoint approximation: control point = (3*q1 + 3*q2 - q0 - q3) / 4
        let ctrl1 = (
            (3.0 * q1.0 + 3.0 * q2.0 - q0.0 - q3.0) * 0.25,
            (3.0 * q1.1 + 3.0 * q2.1 - q0.1 - q3.1) * 0.25,
        );
        self.curves.push(BezierCurve::quadratic(q0, ctrl1, q3));

        // Second half: approximate cubic [q3, q4, q5, q6] as quadratic
        let ctrl2 = (
            (3.0 * q4.0 + 3.0 * q5.0 - q3.0 - q6.0) * 0.25,
            (3.0 * q4.1 + 3.0 * q5.1 - q3.1 - q6.1) * 0.25,
        );
        self.curves.push(BezierCurve::quadratic(q3, ctrl2, q6));

        self.current_pos = p3;
    }

    fn close(&mut self) {
        if self.current_pos != self.start_pos {
            self.curves
                .push(BezierCurve::line(self.current_pos, self.start_pos));
        }
        self.current_pos = self.start_pos;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bezier_curve_creation() {
        let curve = BezierCurve::quadratic((0.0, 0.0), (0.5, 1.0), (1.0, 0.0));
        assert_eq!(curve.p0(), (0.0, 0.0));
        assert_eq!(curve.p1(), (0.5, 1.0));
        assert_eq!(curve.p2(), (1.0, 0.0));
        assert_eq!(curve.bbox, [0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn test_aabb_intersection() {
        let curve = BezierCurve::quadratic((0.0, 0.0), (0.5, 0.5), (1.0, 0.0));
        assert!(curve.intersects_aabb(&[0.0, 0.0, 0.5, 0.5]));
        assert!(curve.intersects_aabb(&[0.5, 0.0, 1.0, 0.5]));
        assert!(!curve.intersects_aabb(&[2.0, 2.0, 3.0, 3.0]));
    }
}
