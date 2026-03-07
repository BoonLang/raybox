//! Demo 8: TodoMVC 3D - Physical 3D rendering of TodoMVC UI
//!
//! A 3D card with extruded/carved text, PBR materials, and 4 switchable themes
//! (Professional, Neobrutalism, Glassmorphism, Neumorphism) with light/dark mode.
//!
//! Non-pixel-perfect AI-generated theme references (for visual direction only):
//! - assets/todomvc_3d/reference_professional.jpg
//! - assets/todomvc_3d/reference_neobrutalism.jpg
//! - assets/todomvc_3d/reference_glassmorphism.jpg
//! - assets/todomvc_3d/reference_neumorphism.jpg

use super::{
    Demo, DemoContext, DemoId, DemoType,
    todomvc_common::{
        GpuBezierCurve, GpuCharInstanceEx, GpuGlyphData, GpuGridCell, GpuUiPrimitive,
        CLASSIC_DECAL_PRIM_START, build_text_layout_pixels, build_ui_primitives, classic_text_colors,
        load_todomvc_font_atlas, rgb3, TextColors,
    },
};
use crate::camera::FlyCamera;
use crate::input::CameraConfig;
use crate::shader_bindings::sdf_todomvc_3d;
use crate::text::build_char_grid;
use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use std::any::Any;
use wgpu::util::DeviceExt;

// ---- GPU structs ----

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Uniforms {
    inv_view_proj: [[f32; 4]; 4],
    camera_pos_time: [f32; 4],
    light_dir_intensity: [f32; 4],
    render_params: [f32; 4],    // xy = resolution, z = textDepth, w = textScale
    text_params: [f32; 4],      // x = charCount, y = uiPrimCount, z = windowScaleFactor, w = classicDecalPrimStart
    char_grid_params: [f32; 4], // xy = gridDims, zw = cellSize
    char_grid_bounds: [f32; 4], // xy = gridMin, zw = gridMax
}

impl Default for Uniforms {
    fn default() -> Self {
        Self {
            inv_view_proj: [[0.0; 4]; 4],
            camera_pos_time: [0.0, 3.5, 3.5, 0.0],
            light_dir_intensity: [0.5, 0.8, 0.3, 1.5],
            render_params: [800.0, 600.0, 0.08, 1.0],
            text_params: [0.0; 4],
            char_grid_params: [0.0; 4],
            char_grid_bounds: [0.0; 4],
        }
    }
}

impl Uniforms {
    fn update_from_camera(&mut self, camera: &FlyCamera, width: u32, height: u32, time: f32) {
        let aspect = width as f32 / height as f32;
        self.inv_view_proj = camera.inv_view_projection_matrix(aspect).to_cols_array_2d();
        self.camera_pos_time = [
            camera.position().x,
            camera.position().y,
            camera.position().z,
            time,
        ];
        self.render_params[0] = width as f32;
        self.render_params[1] = height as f32;
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct ThemeUniforms {
    material_colors: [[f32; 4]; 16], // per materialId: rgb + alpha
    material_props: [[f32; 4]; 16],  // [gloss, metalness, glow, transparency]
    geometry_params: [f32; 4],       // edgeRadius, cardElevation, itemElevation, inputDepth
    ambient_color: [f32; 4],         // rgb + intensity
    extra_params: [f32; 4],          // textReliefDepth, textReliefMode, themeId, darkMode
}

impl Default for ThemeUniforms {
    fn default() -> Self {
        Self {
            material_colors: [[0.5, 0.5, 0.5, 1.0]; 16],
            material_props: [[0.3, 0.0, 0.0, 0.0]; 16],
            geometry_params: [0.015, 0.1, 0.005, 0.02],
            ambient_color: [0.15, 0.14, 0.13, 1.0],
            extra_params: [0.06, 0.0, 0.0, 0.0],
        }
    }
}

// ---- Theme definitions ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ThemeId {
    Classic2D = 0,
    Professional = 1,
    Neobrutalism = 2,
    Glassmorphism = 3,
    Neumorphism = 4,
}

impl ThemeId {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "classic2d" => Some(Self::Classic2D),
            "professional" => Some(Self::Professional),
            "neobrutalism" => Some(Self::Neobrutalism),
            "glassmorphism" => Some(Self::Glassmorphism),
            "neumorphism" => Some(Self::Neumorphism),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Classic2D => "Classic2D",
            Self::Professional => "Professional",
            Self::Neobrutalism => "Neobrutalism",
            Self::Glassmorphism => "Glassmorphism",
            Self::Neumorphism => "Neumorphism",
        }
    }

    fn supports_dark_mode(self) -> bool {
        !matches!(self, Self::Classic2D)
    }

    fn next(self) -> Self {
        match self {
            Self::Classic2D => Self::Professional,
            Self::Professional => Self::Neobrutalism,
            Self::Neobrutalism => Self::Glassmorphism,
            Self::Glassmorphism => Self::Neumorphism,
            Self::Neumorphism => Self::Classic2D,
        }
    }
}

fn canonical_dark_mode(theme_id: ThemeId, dark_mode: bool) -> bool {
    if theme_id.supports_dark_mode() {
        dark_mode
    } else {
        false
    }
}

/// Build ThemeUniforms for a given theme + dark_mode combination
fn build_theme(theme_id: ThemeId, dark_mode: bool) -> ThemeUniforms {
    let dark_mode = canonical_dark_mode(theme_id, dark_mode);
    let mut t = ThemeUniforms::default();
    t.extra_params[2] = theme_id as u8 as f32;
    t.extra_params[3] = if dark_mode { 1.0 } else { 0.0 };

    match (theme_id, dark_mode) {
        (ThemeId::Classic2D, false) => {
            t.material_colors[0] = [0.96, 0.96, 0.96, 1.0];
            t.material_colors[1] = [0.96, 0.96, 0.96, 1.0];
            t.material_props[1] = [0.0, 0.0, 0.0, 0.0];
            t.material_colors[2] = [1.0, 1.0, 1.0, 1.0];
            t.material_props[2] = [0.0, 0.0, 0.0, 0.0];
            t.material_colors[3] = [1.0, 1.0, 1.0, 1.0];
            t.material_props[3] = [0.0, 0.0, 0.0, 0.0];
            t.material_colors[4] = [0.58, 0.58, 0.58, 1.0];
            t.material_props[4] = [0.0, 0.0, 0.0, 0.0];
            t.material_colors[5] = [0.93, 0.93, 0.93, 1.0];
            t.material_props[5] = [0.0, 0.0, 0.0, 0.0];
            t.material_colors[6] = [0.99, 0.99, 0.99, 1.0];
            t.material_props[6] = [0.0, 0.0, 0.0, 0.0];
            t.material_props[7] = [0.0, 0.0, 0.0, 0.0];
            t.geometry_params = [0.0, 0.0, 0.0, 0.0];
            t.ambient_color = [1.0, 1.0, 1.0, 1.0];
            t.extra_params[0] = 0.0;
            t.extra_params[1] = 0.0;
        }
        (ThemeId::Professional, false) => {
            // Material 0: sky/background
            t.material_colors[0] = [0.92, 0.90, 0.87, 1.0];
            // Material 1: ground
            t.material_colors[1] = [0.35, 0.33, 0.30, 1.0];
            t.material_props[1] = [0.15, 0.0, 0.0, 0.0];
            // Material 2: main card (warm white)
            t.material_colors[2] = [0.98, 0.97, 0.95, 1.0];
            t.material_props[2] = [0.35, 0.0, 0.0, 0.0];
            // Material 3: todo items (slightly raised)
            t.material_colors[3] = [0.96, 0.95, 0.93, 1.0];
            t.material_props[3] = [0.25, 0.0, 0.0, 0.0];
            // Material 4: checkbox ring
            t.material_colors[4] = [0.58, 0.58, 0.58, 1.0];
            t.material_props[4] = [0.4, 0.1, 0.0, 0.0];
            // Material 5: separator lines
            t.material_colors[5] = [0.88, 0.88, 0.88, 1.0];
            t.material_props[5] = [0.1, 0.0, 0.0, 0.0];
            // Material 6: stacked cards
            t.material_colors[6] = [0.96, 0.95, 0.93, 1.0];
            t.material_props[6] = [0.3, 0.0, 0.0, 0.0];
            // Material 7: text (color comes from char instance)
            t.material_props[7] = [0.65, 0.0, 0.0, 0.0];
            // Geometry
            t.geometry_params = [0.015, 0.10, 0.005, 0.02];
            // Ambient
            t.ambient_color = [0.18, 0.16, 0.14, 1.0];
            // Text relief: raised
            t.extra_params[0] = 0.20; // textReliefDepth
            t.extra_params[1] = 0.0;  // raised
        }
        (ThemeId::Professional, true) => {
            t.material_colors[0] = [0.12, 0.12, 0.14, 1.0];
            t.material_colors[1] = [0.08, 0.08, 0.10, 1.0];
            t.material_props[1] = [0.15, 0.0, 0.0, 0.0];
            t.material_colors[2] = [0.18, 0.18, 0.20, 1.0];
            t.material_props[2] = [0.35, 0.0, 0.0, 0.0];
            t.material_colors[3] = [0.16, 0.16, 0.18, 1.0];
            t.material_props[3] = [0.25, 0.0, 0.0, 0.0];
            t.material_colors[4] = [0.45, 0.45, 0.48, 1.0];
            t.material_props[4] = [0.4, 0.1, 0.0, 0.0];
            t.material_colors[5] = [0.25, 0.25, 0.28, 1.0];
            t.material_props[5] = [0.1, 0.0, 0.0, 0.0];
            t.material_colors[6] = [0.15, 0.15, 0.17, 1.0];
            t.material_props[6] = [0.3, 0.0, 0.0, 0.0];
            t.material_props[7] = [0.65, 0.0, 0.0, 0.0];
            t.geometry_params = [0.015, 0.10, 0.005, 0.02];
            t.ambient_color = [0.06, 0.06, 0.08, 1.0];
            t.extra_params[0] = 0.20;
            t.extra_params[1] = 0.0;
        }
        (ThemeId::Neobrutalism, false) => {
            t.material_colors[0] = [0.95, 0.93, 0.88, 1.0];
            t.material_colors[1] = [0.20, 0.18, 0.15, 1.0];
            t.material_props[1] = [0.05, 0.0, 0.0, 0.0];
            t.material_colors[2] = [0.98, 0.97, 0.92, 1.0];
            t.material_props[2] = [0.08, 0.0, 0.0, 0.0];
            t.material_colors[3] = [0.97, 0.95, 0.88, 1.0];
            t.material_props[3] = [0.05, 0.0, 0.0, 0.0];
            t.material_colors[4] = [0.15, 0.15, 0.15, 1.0];
            t.material_props[4] = [0.1, 0.0, 0.0, 0.0];
            t.material_colors[5] = [0.15, 0.15, 0.15, 1.0];
            t.material_props[5] = [0.05, 0.0, 0.0, 0.0];
            t.material_colors[6] = [0.95, 0.93, 0.88, 1.0];
            t.material_props[6] = [0.08, 0.0, 0.0, 0.0];
            t.material_props[7] = [0.15, 0.0, 0.0, 0.0];
            t.geometry_params = [0.005, 0.12, 0.008, 0.025];
            t.ambient_color = [0.20, 0.18, 0.15, 1.0];
            t.extra_params[0] = 0.20;
            t.extra_params[1] = 0.0;
        }
        (ThemeId::Neobrutalism, true) => {
            t.material_colors[0] = [0.10, 0.10, 0.08, 1.0];
            t.material_colors[1] = [0.05, 0.05, 0.04, 1.0];
            t.material_props[1] = [0.05, 0.0, 0.0, 0.0];
            t.material_colors[2] = [0.15, 0.15, 0.12, 1.0];
            t.material_props[2] = [0.08, 0.0, 0.0, 0.0];
            t.material_colors[3] = [0.13, 0.13, 0.10, 1.0];
            t.material_props[3] = [0.05, 0.0, 0.0, 0.0];
            t.material_colors[4] = [0.70, 0.70, 0.65, 1.0];
            t.material_props[4] = [0.1, 0.0, 0.0, 0.0];
            t.material_colors[5] = [0.30, 0.30, 0.28, 1.0];
            t.material_props[5] = [0.05, 0.0, 0.0, 0.0];
            t.material_colors[6] = [0.12, 0.12, 0.10, 1.0];
            t.material_props[6] = [0.08, 0.0, 0.0, 0.0];
            t.material_props[7] = [0.15, 0.0, 0.0, 0.0];
            t.geometry_params = [0.005, 0.12, 0.008, 0.025];
            t.ambient_color = [0.06, 0.06, 0.05, 1.0];
            t.extra_params[0] = 0.20;
            t.extra_params[1] = 0.0;
        }
        (ThemeId::Glassmorphism, false) => {
            t.material_colors[0] = [0.88, 0.89, 0.93, 1.0]; // sky - lighter
            t.material_colors[1] = [0.84, 0.86, 0.90, 1.0]; // ground - lighter
            t.material_props[1] = [0.3, 0.0, 0.0, 0.0];
            t.material_colors[2] = [0.96, 0.97, 0.99, 0.22]; // card - more transparent
            t.material_props[2] = [0.75, 0.0, 0.0, 0.0];
            t.material_colors[3] = [0.95, 0.96, 0.98, 0.18]; // items - more transparent
            t.material_props[3] = [0.65, 0.0, 0.0, 0.0];
            t.material_colors[4] = [0.45, 0.50, 0.65, 1.0]; // checkbox
            t.material_props[4] = [0.5, 0.1, 0.0, 0.0];
            t.material_colors[5] = [0.82, 0.84, 0.90, 0.15]; // separators
            t.material_props[5] = [0.3, 0.0, 0.0, 0.0];
            t.material_colors[6] = [0.92, 0.93, 0.97, 0.20]; // stack cards
            t.material_props[6] = [0.6, 0.0, 0.0, 0.0];
            t.material_props[7] = [0.55, 0.0, 0.0, 0.0];
            t.geometry_params = [0.060, 0.10, 0.003, 0.015];
            t.ambient_color = [0.38, 0.40, 0.48, 1.0]; // brighter ambient
            t.extra_params[0] = 0.08; // heading glass pipe depth (body text is decal)
            t.extra_params[1] = 0.0;
        }
        (ThemeId::Glassmorphism, true) => {
            t.material_colors[0] = [0.06, 0.08, 0.14, 1.0];
            t.material_colors[1] = [0.05, 0.06, 0.10, 1.0];
            t.material_props[1] = [0.3, 0.0, 0.0, 0.0];
            t.material_colors[2] = [0.12, 0.14, 0.20, 0.55];
            t.material_props[2] = [0.75, 0.0, 0.0, 0.0];
            t.material_colors[3] = [0.13, 0.15, 0.20, 0.40];
            t.material_props[3] = [0.65, 0.0, 0.0, 0.0];
            t.material_colors[4] = [0.40, 0.45, 0.55, 1.0];
            t.material_props[4] = [0.5, 0.1, 0.0, 0.0];
            t.material_colors[5] = [0.20, 0.22, 0.28, 0.20];
            t.material_props[5] = [0.3, 0.0, 0.0, 0.0];
            t.material_colors[6] = [0.12, 0.14, 0.19, 0.25];
            t.material_props[6] = [0.6, 0.0, 0.0, 0.0];
            t.material_props[7] = [0.55, 0.0, 0.0, 0.0];
            t.geometry_params = [0.060, 0.10, 0.003, 0.015];
            t.ambient_color = [0.06, 0.07, 0.10, 1.0];
            t.extra_params[0] = 0.08; // heading glass pipe depth (body text is decal)
            t.extra_params[1] = 0.0;
        }
        (ThemeId::Neumorphism, false) => {
            t.material_colors[0] = [0.88, 0.88, 0.90, 1.0];
            t.material_colors[1] = [0.78, 0.78, 0.80, 1.0];
            t.material_props[1] = [0.2, 0.0, 0.0, 0.0];
            t.material_colors[2] = [0.88, 0.88, 0.90, 1.0];
            t.material_props[2] = [0.25, 0.0, 0.0, 0.0];
            t.material_colors[3] = [0.86, 0.86, 0.88, 1.0];
            t.material_props[3] = [0.2, 0.0, 0.0, 0.0];
            t.material_colors[4] = [0.60, 0.60, 0.62, 1.0];
            t.material_props[4] = [0.3, 0.0, 0.0, 0.0];
            t.material_colors[5] = [0.82, 0.82, 0.84, 1.0];
            t.material_props[5] = [0.15, 0.0, 0.0, 0.0];
            t.material_colors[6] = [0.86, 0.86, 0.88, 1.0];
            t.material_props[6] = [0.2, 0.0, 0.0, 0.0];
            t.material_props[7] = [0.3, 0.0, 0.0, 0.0];
            t.geometry_params = [0.020, 0.08, 0.003, 0.015];
            t.ambient_color = [0.22, 0.22, 0.24, 1.0];
            t.extra_params[0] = 0.20;
            t.extra_params[1] = 1.0; // carved
        }
        (ThemeId::Neumorphism, true) => {
            t.material_colors[0] = [0.18, 0.18, 0.20, 1.0];
            t.material_colors[1] = [0.12, 0.12, 0.14, 1.0];
            t.material_props[1] = [0.2, 0.0, 0.0, 0.0];
            t.material_colors[2] = [0.18, 0.18, 0.20, 1.0];
            t.material_props[2] = [0.25, 0.0, 0.0, 0.0];
            t.material_colors[3] = [0.16, 0.16, 0.18, 1.0];
            t.material_props[3] = [0.2, 0.0, 0.0, 0.0];
            t.material_colors[4] = [0.35, 0.35, 0.38, 1.0];
            t.material_props[4] = [0.3, 0.0, 0.0, 0.0];
            t.material_colors[5] = [0.22, 0.22, 0.24, 1.0];
            t.material_props[5] = [0.15, 0.0, 0.0, 0.0];
            t.material_colors[6] = [0.16, 0.16, 0.18, 1.0];
            t.material_props[6] = [0.2, 0.0, 0.0, 0.0];
            t.material_props[7] = [0.3, 0.0, 0.0, 0.0];
            t.geometry_params = [0.020, 0.08, 0.003, 0.015];
            t.ambient_color = [0.06, 0.06, 0.07, 1.0];
            t.extra_params[0] = 0.20;
            t.extra_params[1] = 1.0; // carved
        }
        (ThemeId::Classic2D, true) => unreachable!("Classic2D is light-only"),
    }

    t
}

/// Get per-theme light direction and intensity
/// Light comes from upper-left so shadows fall to the right (+X) and down (+Z)
/// Direction vector points TOWARD the light source (normalized in shader)
fn theme_light(theme_id: ThemeId, dark_mode: bool) -> [f32; 4] {
    let dark_mode = canonical_dark_mode(theme_id, dark_mode);
    // Light from upper-left: negative X (left), positive Y (up), negative Z (toward top of card)
    // This casts shadows to +X (right) and +Z (down)
    match (theme_id, dark_mode) {
        (ThemeId::Classic2D, false) => [-0.4, 0.8, -0.3, 1.0],
        (ThemeId::Professional, false) => [-0.4, 0.8, -0.3, 1.6],
        (ThemeId::Professional, true) => [-0.4, 0.8, -0.3, 1.2],
        (ThemeId::Neobrutalism, false) => [-0.5, 0.9, -0.2, 1.8],
        (ThemeId::Neobrutalism, true) => [-0.5, 0.9, -0.2, 1.3],
        (ThemeId::Glassmorphism, false) => [-0.3, 0.7, -0.3, 1.1],
        (ThemeId::Glassmorphism, true) => [-0.3, 0.7, -0.3, 0.85],
        (ThemeId::Neumorphism, false) => [-0.3, 0.6, -0.3, 1.2],
        (ThemeId::Neumorphism, true) => [-0.3, 0.6, -0.3, 0.9],
        (ThemeId::Classic2D, true) => unreachable!("Classic2D is light-only"),
    }
}

/// Get per-theme text colors
fn text_colors(theme_id: ThemeId, dark_mode: bool) -> TextColors {
    let dark_mode = canonical_dark_mode(theme_id, dark_mode);
    match (theme_id, dark_mode) {
        (ThemeId::Classic2D, false) => classic_text_colors(),
        (ThemeId::Professional, false) | (ThemeId::Neobrutalism, false) => TextColors {
            heading: rgb3(184, 63, 69),
            active: rgb3(72, 72, 72),
            completed: rgb3(148, 148, 148),
            placeholder: rgb3(153, 153, 153),
            body: rgb3(17, 17, 17),
            info: rgb3(77, 77, 77),
        },
        (ThemeId::Professional, true) | (ThemeId::Neobrutalism, true) => TextColors {
            heading: rgb3(220, 100, 105),
            active: rgb3(210, 210, 215),
            completed: rgb3(120, 120, 125),
            placeholder: rgb3(130, 130, 135),
            body: rgb3(200, 200, 205),
            info: rgb3(140, 140, 145),
        },
        (ThemeId::Glassmorphism, false) => TextColors {
            heading: rgb3(75, 100, 180),
            active: rgb3(40, 45, 60),
            completed: rgb3(130, 135, 150),
            placeholder: rgb3(130, 135, 155),
            body: rgb3(30, 35, 50),
            info: rgb3(100, 105, 120),
        },
        (ThemeId::Glassmorphism, true) => TextColors {
            heading: rgb3(140, 165, 220),
            active: rgb3(200, 205, 215),
            completed: rgb3(140, 145, 160), // brighter for contrast on dark glass
            placeholder: rgb3(145, 150, 165),
            body: rgb3(190, 195, 205),
            info: rgb3(160, 165, 180), // brighter for readability at small sizes
        },
        (ThemeId::Neumorphism, false) => TextColors {
            heading: rgb3(140, 80, 85),
            active: rgb3(60, 60, 62),
            completed: rgb3(140, 140, 142),
            placeholder: rgb3(145, 145, 148),
            body: rgb3(40, 40, 42),
            info: rgb3(110, 110, 112),
        },
        (ThemeId::Neumorphism, true) => TextColors {
            heading: rgb3(200, 110, 115),
            active: rgb3(190, 190, 195),
            completed: rgb3(100, 100, 105),
            placeholder: rgb3(110, 110, 115),
            body: rgb3(180, 180, 185),
            info: rgb3(120, 120, 125),
        },
        (ThemeId::Classic2D, true) => unreachable!("Classic2D is light-only"),
    }
}

// ---- Keybindings ----

const KEYBINDINGS_TODOMVC_3D: &[(&str, &str)] = &[
    ("WASD", "Move"),
    ("Mouse", "Look"),
    ("Space/Ctrl", "Up/Down"),
    ("Q/E", "Roll"),
    ("Scroll", "Speed"),
    ("R", "Reset roll"),
    ("T", "Reset camera"),
    ("Tab", "Capture mouse"),
    ("N", "Cycle theme"),
    ("M", "Toggle dark mode"),
];

// ---- Demo struct ----

pub struct TodoMvc3DDemo {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    theme_buffer: wgpu::Buffer,
    char_instances_buffer: wgpu::Buffer,
    char_instances: Vec<GpuCharInstanceEx>,
    bind_group: wgpu::BindGroup,
    char_count: u32,
    ui_prim_count: u32,
    char_grid_params: [f32; 4],
    char_grid_bounds: [f32; 4],
    pub current_theme: ThemeId,
    pub dark_mode: bool,
    width: u32,
    height: u32,
    scale_factor: f32,
}

impl TodoMvc3DDemo {
    pub fn new(ctx: &DemoContext) -> Result<Self> {
        let current_theme = ThemeId::Classic2D;
        let dark_mode = false;
        let colors = text_colors(current_theme, dark_mode);
        let atlas = load_todomvc_font_atlas()?;
        let ui_primitives = build_ui_primitives();
        let ui_prim_count = ui_primitives.len() as u32;
        let char_instances = build_text_layout_pixels(&atlas, &colors);
        let char_count = char_instances.len() as u32;

        let instance_data: Vec<[f32; 4]> = char_instances.iter().map(|c| c.pos_and_char).collect();
        let char_grid = build_char_grid(&instance_data, &atlas, [64, 48]);

        let char_grid_params = [
            char_grid.dims[0] as f32,
            char_grid.dims[1] as f32,
            char_grid.cell_size[0],
            char_grid.cell_size[1],
        ];
        let char_grid_bounds = char_grid.bounds;

        // Prepare GPU data
        let gpu_grid_cells: Vec<GpuGridCell> = atlas
            .grid_cells
            .iter()
            .map(|c| GpuGridCell {
                curve_start_and_count: (c.curve_start as u32)
                    | ((c.curve_count as u32) << 16)
                    | ((c.flags as u32) << 24),
            })
            .collect();

        let gpu_curve_indices: Vec<u32> = atlas.curve_indices.iter().map(|&i| i as u32).collect();

        let gpu_curves: Vec<GpuBezierCurve> = atlas
            .curves
            .iter()
            .map(|c| {
                let p0 = c.p0();
                let p1 = c.p1();
                let p2 = c.p2();
                GpuBezierCurve {
                    points01: [p0.0, p0.1, p1.0, p1.1],
                    points2bbox: [p2.0, p2.1, c.bbox[0], c.bbox[1]],
                    bbox_flags: [c.bbox[2], c.bbox[3], c.flags as f32, 0.0],
                }
            })
            .collect();

        let gpu_glyph_data: Vec<GpuGlyphData> = atlas
            .glyph_list
            .iter()
            .map(|(_, entry)| GpuGlyphData {
                bounds: entry.bounds,
                grid_info: [
                    entry.grid_offset,
                    entry.grid_size[0],
                    entry.grid_size[1],
                    0,
                ],
                curve_info: [
                    entry.curve_offset,
                    entry.curve_count,
                    0,
                    0,
                ],
            })
            .collect();

        // Create uniform buffers
        let mut uniforms = Uniforms::default();
        uniforms.text_params[0] = char_count as f32;
        uniforms.text_params[1] = ui_prim_count as f32;
        uniforms.text_params[2] = ctx.scale_factor;
        uniforms.text_params[3] = CLASSIC_DECAL_PRIM_START as f32;
        uniforms.char_grid_params = char_grid_params;
        uniforms.char_grid_bounds = char_grid_bounds;
        uniforms.light_dir_intensity = theme_light(current_theme, dark_mode);

        let uniform_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("TodoMVC 3D Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let theme_uniforms = build_theme(current_theme, dark_mode);
        let theme_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("TodoMVC 3D Theme Buffer"),
            contents: bytemuck::cast_slice(&[theme_uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create storage buffers
        let grid_cells_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Grid Cells Buffer"),
            contents: bytemuck::cast_slice(if gpu_grid_cells.is_empty() {
                &[GpuGridCell { curve_start_and_count: 0 }]
            } else {
                &gpu_grid_cells
            }),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let curve_indices_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Curve Indices Buffer"),
            contents: bytemuck::cast_slice(if gpu_curve_indices.is_empty() {
                &[0u32]
            } else {
                &gpu_curve_indices
            }),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let curves_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Curves Buffer"),
            contents: bytemuck::cast_slice(if gpu_curves.is_empty() {
                &[GpuBezierCurve {
                    points01: [0.0; 4],
                    points2bbox: [0.0; 4],
                    bbox_flags: [0.0; 4],
                }]
            } else {
                &gpu_curves
            }),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let glyph_data_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Glyph Data Buffer"),
            contents: bytemuck::cast_slice(if gpu_glyph_data.is_empty() {
                &[GpuGlyphData {
                    bounds: [0.0; 4],
                    grid_info: [0; 4],
                    curve_info: [0; 4],
                }]
            } else {
                &gpu_glyph_data
            }),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let char_instances_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Char Instances Buffer"),
            contents: bytemuck::cast_slice(if char_instances.is_empty() {
                &[GpuCharInstanceEx {
                    pos_and_char: [0.0; 4],
                    color_flags: [0.0; 4],
                }]
            } else {
                &char_instances
            }),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let char_grid_cells_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Char Grid Cells Buffer"),
            contents: bytemuck::cast_slice(&char_grid.cells),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let char_grid_indices_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Char Grid Indices Buffer"),
            contents: bytemuck::cast_slice(&char_grid.char_indices),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let ui_primitives_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("TodoMVC 3D UI Primitives Buffer"),
            contents: bytemuck::cast_slice(if ui_primitives.is_empty() {
                &[GpuUiPrimitive {
                    pos_size: [0.0; 4],
                    color: [0.0; 4],
                    params: [0.0; 4],
                    extra: [0.0; 4],
                }]
            } else {
                &ui_primitives
            }),
            usage: wgpu::BufferUsages::STORAGE,
        });

        // Bind group layout: 10 bindings (2 uniform + 8 storage)
        let uniform_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };

        let storage_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };

        let bind_group_layout = ctx.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TodoMVC 3D Bind Group Layout"),
            entries: &[
                uniform_entry(0),
                uniform_entry(1),
                storage_entry(2),
                storage_entry(3),
                storage_entry(4),
                storage_entry(5),
                storage_entry(6),
                storage_entry(7),
                storage_entry(8),
                storage_entry(9),
            ],
        });

        let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TodoMVC 3D Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: theme_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: grid_cells_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: curve_indices_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: curves_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: glyph_data_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: char_instances_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: char_grid_cells_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: char_grid_indices_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: ui_primitives_buffer.as_entire_binding(),
                },
            ],
        });

        // Pipeline
        let shader_module = sdf_todomvc_3d::create_shader_module_embed_source(ctx.device);

        let pipeline_layout = ctx.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("TodoMVC 3D Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = ctx.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("TodoMVC 3D Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: ctx.surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Ok(Self {
            pipeline,
            uniform_buffer,
            theme_buffer,
            char_instances_buffer,
            char_instances,
            bind_group,
            char_count,
            ui_prim_count,
            char_grid_params,
            char_grid_bounds,
            current_theme,
            dark_mode,
            width: ctx.width,
            height: ctx.height,
            scale_factor: ctx.scale_factor,
        })
    }

    pub fn update_uniforms(&self, queue: &wgpu::Queue, camera: &FlyCamera, time: f32) {
        let mut uniforms = Uniforms::default();
        uniforms.update_from_camera(camera, self.width, self.height, time);
        uniforms.text_params[0] = self.char_count as f32;
        uniforms.text_params[1] = self.ui_prim_count as f32;
        uniforms.text_params[2] = self.scale_factor;
        uniforms.text_params[3] = CLASSIC_DECAL_PRIM_START as f32;
        uniforms.char_grid_params = self.char_grid_params;
        uniforms.char_grid_bounds = self.char_grid_bounds;
        uniforms.light_dir_intensity = theme_light(self.current_theme, self.dark_mode);
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    fn update_theme_buffer(&self, queue: &wgpu::Queue) {
        let theme_uniforms = build_theme(self.current_theme, self.dark_mode);
        queue.write_buffer(&self.theme_buffer, 0, bytemuck::cast_slice(&[theme_uniforms]));

        // Update text colors based on current theme
        let colors = text_colors(self.current_theme, self.dark_mode);
        let updated: Vec<GpuCharInstanceEx> = self.char_instances.iter().map(|inst| {
            let flags = inst.color_flags[3];
            let color = if flags == 2.0 {
                colors.heading
            } else if flags == 1.0 {
                colors.completed
            } else if flags == 3.0 {
                colors.placeholder
            } else if flags == 4.0 {
                colors.body
            } else if flags == 5.0 {
                colors.info
            } else {
                colors.active
            };
            GpuCharInstanceEx {
                pos_and_char: inst.pos_and_char,
                color_flags: [color[0], color[1], color[2], flags],
            }
        }).collect();
        queue.write_buffer(&self.char_instances_buffer, 0, bytemuck::cast_slice(&updated));
    }

    pub fn cycle_theme(&mut self) {
        self.current_theme = self.current_theme.next();
        self.dark_mode = canonical_dark_mode(self.current_theme, self.dark_mode);
    }

    pub fn toggle_dark_mode(&mut self) {
        self.dark_mode = canonical_dark_mode(self.current_theme, !self.dark_mode);
    }

    pub fn set_theme(&mut self, theme_id: ThemeId) {
        self.current_theme = theme_id;
        self.dark_mode = canonical_dark_mode(self.current_theme, self.dark_mode);
    }

    pub fn set_dark_mode(&mut self, dark: bool) {
        self.dark_mode = canonical_dark_mode(self.current_theme, dark);
    }

    pub fn set_scale_factor(&mut self, scale_factor: f32) {
        self.scale_factor = scale_factor.max(0.5);
    }
}

impl Demo for TodoMvc3DDemo {
    fn name(&self) -> &'static str {
        "TodoMVC 3D"
    }

    fn id(&self) -> DemoId {
        DemoId::TodoMvc3D
    }

    fn demo_type(&self) -> DemoType {
        DemoType::Scene3D
    }

    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        KEYBINDINGS_TODOMVC_3D
    }

    fn camera_config(&self) -> CameraConfig {
        CameraConfig {
            initial_position: glam::Vec3::new(0.0, 9.5, 0.001),
            look_at_target: glam::Vec3::new(0.0, 0.0, 0.0),
        }
    }

    fn update(&mut self, _dt: f32, _camera: &mut FlyCamera) {
        // No per-frame updates needed
    }

    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, queue: &wgpu::Queue, _time: f32) {
        self.update_theme_buffer(queue);
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
