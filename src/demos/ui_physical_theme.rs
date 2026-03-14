use crate::retained::text::TextColors;
use crate::ui_physical_shader_bindings;
use std::cell::Cell;

pub type ThemeUniforms = ui_physical_shader_bindings::ThemeUniforms_std140_0;

#[derive(Copy, Clone, Debug)]
struct ThemeSpec {
    material_colors: [[f32; 4]; 16],
    material_props: [[f32; 4]; 16],
    geometry_params: [f32; 4],
    ambient_color: [f32; 4],
    extra_params: [f32; 4],
}

impl Default for ThemeSpec {
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

impl ThemeSpec {
    fn into_uniforms(self) -> ThemeUniforms {
        ThemeUniforms::new(
            self.material_colors,
            self.material_props,
            self.geometry_params,
            self.ambient_color,
            self.extra_params,
        )
    }
}

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

    pub fn next(self) -> Self {
        match self {
            Self::Classic2D => Self::Professional,
            Self::Professional => Self::Neobrutalism,
            Self::Neobrutalism => Self::Glassmorphism,
            Self::Glassmorphism => Self::Neumorphism,
            Self::Neumorphism => Self::Classic2D,
        }
    }
}

pub const PHYSICAL_THEME_OPTIONS: &[&str] = &[
    "classic2d",
    "professional",
    "neobrutalism",
    "glassmorphism",
    "neumorphism",
];

fn rgb3(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

fn classic_text_colors() -> TextColors {
    TextColors {
        heading: rgb3(184, 63, 69),
        active: rgb3(72, 72, 72),
        completed: rgb3(148, 148, 148),
        placeholder: rgb3(153, 153, 153),
        body: rgb3(17, 17, 17),
        info: rgb3(77, 77, 77),
    }
}

fn tune_generic_ui_physical_text_color(color: [f32; 3]) -> [f32; 3] {
    let luminance = 0.2126 * color[0] + 0.7152 * color[1] + 0.0722 * color[2];
    let target = if luminance > 0.5 {
        [0.07, 0.07, 0.08]
    } else {
        [0.94, 0.95, 0.97]
    };
    let strength = if luminance > 0.5 { 0.22 } else { 0.12 };
    [
        color[0] * (1.0 - strength) + target[0] * strength,
        color[1] * (1.0 - strength) + target[1] * strength,
        color[2] * (1.0 - strength) + target[2] * strength,
    ]
}

pub fn tune_generic_ui_physical_text_colors(colors: TextColors) -> TextColors {
    TextColors {
        heading: tune_generic_ui_physical_text_color(colors.heading),
        active: tune_generic_ui_physical_text_color(colors.active),
        completed: tune_generic_ui_physical_text_color(colors.completed),
        placeholder: tune_generic_ui_physical_text_color(colors.placeholder),
        body: tune_generic_ui_physical_text_color(colors.body),
        info: tune_generic_ui_physical_text_color(colors.info),
    }
}

pub fn canonical_dark_mode(theme_id: ThemeId, dark_mode: bool) -> bool {
    if theme_id.supports_dark_mode() {
        dark_mode
    } else {
        false
    }
}

pub fn build_theme(theme_id: ThemeId, dark_mode: bool) -> ThemeUniforms {
    let dark_mode = canonical_dark_mode(theme_id, dark_mode);
    let mut t = ThemeSpec::default();
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
            t.material_colors[0] = [0.92, 0.90, 0.87, 1.0];
            t.material_colors[1] = [0.35, 0.33, 0.30, 1.0];
            t.material_props[1] = [0.15, 0.0, 0.0, 0.0];
            t.material_colors[2] = [0.98, 0.97, 0.95, 1.0];
            t.material_props[2] = [0.35, 0.0, 0.0, 0.0];
            t.material_colors[3] = [0.96, 0.95, 0.93, 1.0];
            t.material_props[3] = [0.25, 0.0, 0.0, 0.0];
            t.material_colors[4] = [0.58, 0.58, 0.58, 1.0];
            t.material_props[4] = [0.4, 0.1, 0.0, 0.0];
            t.material_colors[5] = [0.88, 0.88, 0.88, 1.0];
            t.material_props[5] = [0.1, 0.0, 0.0, 0.0];
            t.material_colors[6] = [0.96, 0.95, 0.93, 1.0];
            t.material_props[6] = [0.3, 0.0, 0.0, 0.0];
            t.material_props[7] = [0.65, 0.0, 0.0, 0.0];
            t.geometry_params = [0.015, 0.10, 0.005, 0.02];
            t.ambient_color = [0.18, 0.16, 0.14, 1.0];
            t.extra_params[0] = 0.20;
            t.extra_params[1] = 0.0;
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
            t.material_colors[0] = [0.88, 0.89, 0.93, 1.0];
            t.material_colors[1] = [0.84, 0.86, 0.90, 1.0];
            t.material_props[1] = [0.3, 0.0, 0.0, 0.0];
            t.material_colors[2] = [0.96, 0.97, 0.99, 0.22];
            t.material_props[2] = [0.75, 0.0, 0.0, 0.0];
            t.material_colors[3] = [0.95, 0.96, 0.98, 0.18];
            t.material_props[3] = [0.65, 0.0, 0.0, 0.0];
            t.material_colors[4] = [0.45, 0.50, 0.65, 1.0];
            t.material_props[4] = [0.5, 0.1, 0.0, 0.0];
            t.material_colors[5] = [0.82, 0.84, 0.90, 0.15];
            t.material_props[5] = [0.3, 0.0, 0.0, 0.0];
            t.material_colors[6] = [0.92, 0.93, 0.97, 0.20];
            t.material_props[6] = [0.6, 0.0, 0.0, 0.0];
            t.material_props[7] = [0.55, 0.0, 0.0, 0.0];
            t.geometry_params = [0.060, 0.10, 0.003, 0.015];
            t.ambient_color = [0.38, 0.40, 0.48, 1.0];
            t.extra_params[0] = 0.08;
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
            t.extra_params[0] = 0.08;
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
            t.extra_params[1] = 1.0;
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
            t.extra_params[1] = 1.0;
        }
        (ThemeId::Classic2D, true) => unreachable!("Classic2D is light-only"),
    }

    t.into_uniforms()
}

pub fn theme_light(theme_id: ThemeId, dark_mode: bool) -> [f32; 4] {
    let dark_mode = canonical_dark_mode(theme_id, dark_mode);
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

pub fn text_colors(theme_id: ThemeId, dark_mode: bool) -> TextColors {
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
            completed: rgb3(140, 145, 160),
            placeholder: rgb3(145, 150, 165),
            body: rgb3(190, 195, 205),
            info: rgb3(160, 165, 180),
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

pub struct UiPhysicalThemeState {
    current_theme: ThemeId,
    dark_mode: bool,
    theme_dirty: Cell<bool>,
}

impl UiPhysicalThemeState {
    pub fn new(current_theme: ThemeId, dark_mode: bool) -> Self {
        Self {
            current_theme,
            dark_mode: canonical_dark_mode(current_theme, dark_mode),
            theme_dirty: Cell::new(false),
        }
    }

    pub fn current_theme(&self) -> ThemeId {
        self.current_theme
    }

    pub fn dark_mode(&self) -> bool {
        self.dark_mode
    }

    pub fn light_dir_intensity(&self) -> [f32; 4] {
        theme_light(self.current_theme, self.dark_mode)
    }

    pub fn text_colors(&self) -> TextColors {
        text_colors(self.current_theme, self.dark_mode)
    }

    pub fn theme_uniforms(&self) -> ThemeUniforms {
        build_theme(self.current_theme, self.dark_mode)
    }

    pub fn is_dirty(&self) -> bool {
        self.theme_dirty.get()
    }

    pub fn clear_dirty(&self) {
        self.theme_dirty.set(false);
    }

    fn mark_visual_change(&self) {
        self.theme_dirty.set(true);
    }

    pub fn cycle_theme(&mut self) {
        self.current_theme = self.current_theme.next();
        self.dark_mode = canonical_dark_mode(self.current_theme, self.dark_mode);
        self.mark_visual_change();
    }

    pub fn toggle_dark_mode(&mut self) {
        self.dark_mode = canonical_dark_mode(self.current_theme, !self.dark_mode);
        self.mark_visual_change();
    }

    pub fn set_theme(&mut self, theme_id: ThemeId) {
        if self.current_theme != theme_id {
            self.current_theme = theme_id;
            self.dark_mode = canonical_dark_mode(self.current_theme, self.dark_mode);
            self.mark_visual_change();
        }
    }

    pub fn set_dark_mode(&mut self, dark: bool) {
        let dark_mode = canonical_dark_mode(self.current_theme, dark);
        if self.dark_mode != dark_mode {
            self.dark_mode = dark_mode;
            self.mark_visual_change();
        }
    }

    pub fn set_named_theme(
        &mut self,
        theme: &str,
        dark_mode: Option<bool>,
    ) -> Option<(&'static str, bool)> {
        let theme_id = ThemeId::from_str(theme)?;
        self.set_theme(theme_id);
        if let Some(dm) = dark_mode {
            self.set_dark_mode(dm);
        }
        Some((theme_id.name(), self.dark_mode))
    }
}
