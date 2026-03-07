use crate::text::{VectorFont, VectorFontAtlas};
use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};

pub const VIRTUAL_WIDTH: f32 = 700.0;
pub const VIRTUAL_HEIGHT: f32 = 700.0;
pub const X_OFFSET: f32 = 0.0;
pub const SCREEN_H: f32 = VIRTUAL_HEIGHT;

pub const PIXEL_TO_WORLD: f32 = 1.0 / 100.0;
pub const PIXEL_CENTER_X: f32 = 350.0;
pub const PIXEL_CENTER_Z: f32 = 302.0;

pub const ITALIC_CODEPOINT_OFFSET: u32 = 0x10000;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuGridCell {
    pub curve_start_and_count: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuBezierCurve {
    pub points01: [f32; 4],
    pub points2bbox: [f32; 4],
    pub bbox_flags: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuGlyphData {
    pub bounds: [f32; 4],
    pub grid_info: [u32; 4],
    pub curve_info: [u32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuCharInstanceEx {
    pub pos_and_char: [f32; 4], // x, y, scale, glyphIndex in pixel-space layout coordinates
    pub color_flags: [f32; 4],  // r, g, b, role flags
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuUiPrimitive {
    pub pos_size: [f32; 4],
    pub color: [f32; 4],
    pub params: [f32; 4],
    pub extra: [f32; 4],
}

pub const PRIM_FILLED_RECT: f32 = 0.0;
pub const PRIM_STROKED_RECT: f32 = 1.0;
pub const PRIM_FILLED_CIRCLE: f32 = 2.0;
pub const PRIM_STROKED_CIRCLE: f32 = 3.0;
pub const PRIM_LINE: f32 = 4.0;
pub const PRIM_BOX_SHADOW: f32 = 5.0;
pub const PRIM_CHECKMARK_V: f32 = 6.0;
pub const CLASSIC_DECAL_PRIM_START: u32 = 7;

pub const ROLE_ACTIVE: f32 = 0.0;
pub const ROLE_COMPLETED: f32 = 1.0;
pub const ROLE_HEADING: f32 = 2.0;
pub const ROLE_PLACEHOLDER: f32 = 3.0;
pub const ROLE_BODY: f32 = 4.0;
pub const ROLE_INFO: f32 = 5.0;

#[derive(Clone, Copy)]
pub struct TextColors {
    pub heading: [f32; 3],
    pub active: [f32; 3],
    pub completed: [f32; 3],
    pub placeholder: [f32; 3],
    pub body: [f32; 3],
    pub info: [f32; 3],
}

pub fn rgb(r: u8, g: u8, b: u8) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a]
}

pub fn rgb3(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

pub fn px_to_world_x(px: f32) -> f32 {
    (px - PIXEL_CENTER_X) * PIXEL_TO_WORLD
}

pub fn px_to_world_z(py: f32) -> f32 {
    -(py - PIXEL_CENTER_Z) * PIXEL_TO_WORLD
}

pub fn load_todomvc_font_atlas() -> Result<VectorFontAtlas> {
    let font_data = std::fs::read("assets/fonts/LiberationSans-Regular.ttf")
        .context("Failed to load Liberation Sans Regular font")?;
    let mut font = VectorFont::from_ttf(&font_data).map_err(|e| anyhow::anyhow!(e))?;

    let italic_data = std::fs::read("assets/fonts/LiberationSans-Italic.ttf")
        .context("Failed to load Liberation Sans Italic font")?;
    font.merge_from_ttf(&italic_data, ITALIC_CODEPOINT_OFFSET)
        .map_err(|e| anyhow::anyhow!(e))?;

    Ok(VectorFontAtlas::from_font(&font, 32))
}

fn fy(y: f32) -> f32 {
    SCREEN_H - y
}

fn rect_yu(x: f32, y: f32, w: f32, h: f32) -> [f32; 4] {
    [x, fy(y + h), w, h]
}

fn hline_yu(x1: f32, y: f32, x2: f32) -> [f32; 4] {
    [x1, fy(y), x2, fy(y)]
}

pub fn build_ui_primitives() -> Vec<GpuUiPrimitive> {
    let mut prims = Vec::new();

    let card_x = 75.0 + X_OFFSET;
    let card_y = 130.0;
    let card_w = 550.0;
    let card_h = 344.2;

    prims.push(GpuUiPrimitive {
        pos_size: rect_yu(card_x, card_y, card_w, card_h),
        color: rgba(0, 0, 0, 0.1),
        params: [0.0, 0.0, 50.0, PRIM_BOX_SHADOW],
        extra: [0.0, -25.0, 0.0, 0.0],
    });

    let stack2_y = card_y + card_h;
    prims.push(GpuUiPrimitive {
        pos_size: rect_yu(card_x + 4.0, stack2_y, card_w - 8.0, 8.0),
        color: rgba(0, 0, 0, 0.2),
        params: [0.0, 0.0, 1.0, PRIM_BOX_SHADOW],
        extra: [0.0, -1.0, 0.0, 0.0],
    });
    prims.push(GpuUiPrimitive {
        pos_size: rect_yu(card_x + 4.0, stack2_y, card_w - 8.0, 8.0),
        color: rgb(252, 252, 252),
        params: [0.0, 0.0, 0.0, PRIM_FILLED_RECT],
        extra: [0.0; 4],
    });

    let stack1_y = card_y + card_h;
    prims.push(GpuUiPrimitive {
        pos_size: rect_yu(card_x + 2.0, stack1_y, card_w - 4.0, 4.0),
        color: rgba(0, 0, 0, 0.2),
        params: [0.0, 0.0, 1.0, PRIM_BOX_SHADOW],
        extra: [0.0, -1.0, 0.0, 0.0],
    });
    prims.push(GpuUiPrimitive {
        pos_size: rect_yu(card_x + 2.0, stack1_y, card_w - 4.0, 4.0),
        color: rgb(252, 252, 252),
        params: [0.0, 0.0, 0.0, PRIM_FILLED_RECT],
        extra: [0.0; 4],
    });

    prims.push(GpuUiPrimitive {
        pos_size: rect_yu(card_x, card_y, card_w, card_h),
        color: rgba(0, 0, 0, 0.2),
        params: [0.0, 0.0, 1.0, PRIM_BOX_SHADOW],
        extra: [0.0, -1.0, 0.0, 0.0],
    });

    prims.push(GpuUiPrimitive {
        pos_size: rect_yu(card_x, card_y, card_w, card_h),
        color: rgb(255, 255, 255),
        params: [0.0, 0.0, 0.0, PRIM_FILLED_RECT],
        extra: [0.0; 4],
    });

    prims.push(GpuUiPrimitive {
        pos_size: rect_yu(card_x, card_y, card_w, 2.0),
        color: rgba(0, 0, 0, 0.03),
        params: [0.0, 0.0, 0.0, PRIM_FILLED_RECT],
        extra: [0.0; 4],
    });

    let sep_color = rgba(237, 237, 237, 1.0);
    for &y in &[255.4_f32, 315.0, 374.6] {
        prims.push(GpuUiPrimitive {
            pos_size: hline_yu(card_x, y, card_x + card_w),
            color: sep_color,
            params: [0.0, 0.8, 0.0, PRIM_LINE],
            extra: [0.0; 4],
        });
    }

    prims.push(GpuUiPrimitive {
        pos_size: hline_yu(card_x, 433.4, card_x + card_w),
        color: rgba(230, 230, 230, 1.0),
        params: [0.0, 0.8, 0.0, PRIM_LINE],
        extra: [0.0; 4],
    });

    prims.push(GpuUiPrimitive {
        pos_size: hline_yu(card_x, 195.8, card_x + card_w),
        color: sep_color,
        params: [0.0, 0.8, 0.0, PRIM_LINE],
        extra: [0.0; 4],
    });

    {
        let chev_cx = card_x + 26.0;
        let chev_cy_down = 130.8 + 32.5;
        let half_w = 10.0;
        let half_h = 4.5;
        let left = (chev_cx - half_w, chev_cy_down - half_h);
        let bottom = (chev_cx, chev_cy_down + half_h);
        let right = (chev_cx + half_w, chev_cy_down - half_h);
        prims.push(GpuUiPrimitive {
            pos_size: [left.0, fy(left.1), bottom.0, fy(bottom.1)],
            color: rgb(0x94, 0x94, 0x94),
            params: [0.0, 4.0, 0.0, PRIM_CHECKMARK_V],
            extra: [right.0, fy(right.1), 0.0, 0.0],
        });
    }

    prims.push(GpuUiPrimitive {
        pos_size: rect_yu(card_x, card_y, card_w, 65.8),
        color: rgba(0xCE, 0x46, 0x46, 0.6),
        params: [0.0, 0.8, 0.0, PRIM_STROKED_RECT],
        extra: [0.0; 4],
    });

    struct CheckboxInfo {
        y_center: f32,
        checked: bool,
    }
    let checkboxes = [
        CheckboxInfo {
            y_center: 195.8 + 29.8,
            checked: false,
        },
        CheckboxInfo {
            y_center: 255.4 + 29.8,
            checked: true,
        },
        CheckboxInfo {
            y_center: 315.0 + 29.8,
            checked: false,
        },
        CheckboxInfo {
            y_center: 374.6 + 29.4,
            checked: false,
        },
    ];

    for cb in &checkboxes {
        let cx = card_x + 26.0;
        let cy = fy(cb.y_center);
        let r = 17.0;
        let stroke_w = 1.2;

        if cb.checked {
            prims.push(GpuUiPrimitive {
                pos_size: [cx, cy, r, 0.0],
                color: rgb(0x59, 0xA1, 0x93),
                params: [0.0, stroke_w, 0.0, PRIM_STROKED_CIRCLE],
                extra: [0.0; 4],
            });
            let s = r / 50.0;
            let map = |sx: f32, sy: f32| -> (f32, f32) {
                (cx + (sx - 50.0) * s, cy - (sy - 50.0) * s)
            };
            let (ax, ay) = map(27.0, 56.0);
            let (bx, by) = map(42.0, 71.0);
            let (cx2, cy2) = map(72.0, 25.0);
            prims.push(GpuUiPrimitive {
                pos_size: [ax, ay, bx, by],
                color: rgb(0x3E, 0xA3, 0x90),
                params: [0.0, 2.0, 0.0, PRIM_CHECKMARK_V],
                extra: [cx2, cy2, 0.0, 0.0],
            });
        } else {
            prims.push(GpuUiPrimitive {
                pos_size: [cx, cy, r, 0.0],
                color: rgb(0x94, 0x94, 0x94),
                params: [0.0, stroke_w, 0.0, PRIM_STROKED_CIRCLE],
                extra: [0.0; 4],
            });
        }
    }

    let strike_y = fy(271.2 + 13.2);
    prims.push(GpuUiPrimitive {
        pos_size: [135.0 + X_OFFSET, strike_y, 135.0 + 273.0 + X_OFFSET, strike_y],
        color: rgb(0x94, 0x94, 0x94),
        params: [0.0, 2.0, 0.0, PRIM_LINE],
        extra: [0.0; 4],
    });

    prims.push(GpuUiPrimitive {
        pos_size: rect_yu(255.6 + X_OFFSET, 441.2, 32.3, 24.4),
        color: rgba(0xCE, 0x46, 0x46, 1.0),
        params: [3.0, 0.8, 0.0, PRIM_STROKED_RECT],
        extra: [0.0; 4],
    });

    prims
}

pub fn classic_text_colors() -> TextColors {
    TextColors {
        heading: rgb3(184, 63, 69),
        active: rgb3(72, 72, 72),
        completed: rgb3(148, 148, 148),
        placeholder: rgb3(153, 153, 153),
        body: rgb3(17, 17, 17),
        info: rgb3(77, 77, 77),
    }
}

pub fn build_text_layout_pixels(
    atlas: &VectorFontAtlas,
    colors: &TextColors,
) -> Vec<GpuCharInstanceEx> {
    let mut instances = Vec::new();
    let ascent_ratio = 0.905;

    let baseline_y = |text_top_y: f32, font_size: f32| -> f32 {
        let metadata_baseline = text_top_y + ascent_ratio * font_size;
        SCREEN_H - metadata_baseline
    };

    let emit_text = |instances: &mut Vec<GpuCharInstanceEx>,
                     text: &str,
                     x: f32,
                     baseline_y: f32,
                     font_size: f32,
                     color: [f32; 3],
                     flags: f32,
                     atlas: &VectorFontAtlas,
                     codepoint_offset: u32| -> f32 {
        let mut cx = x;
        for ch in text.chars() {
            if ch == ' ' {
                let adv = atlas
                    .glyphs
                    .get(&(' ' as u32 + codepoint_offset))
                    .or_else(|| atlas.glyphs.get(&(' ' as u32)))
                    .map(|e| e.advance)
                    .unwrap_or(0.25);
                cx += adv * font_size;
                continue;
            }
            let codepoint = ch as u32 + codepoint_offset;
            if let Some(idx) = atlas.glyph_list.iter().position(|(cp, _)| *cp == codepoint) {
                let entry = &atlas.glyph_list[idx].1;
                instances.push(GpuCharInstanceEx {
                    pos_and_char: [cx, baseline_y, font_size, idx as f32],
                    color_flags: [color[0], color[1], color[2], flags],
                });
                cx += entry.advance * font_size;
            }
        }
        cx
    };

    emit_text(
        &mut instances,
        "todos",
        252.1 + X_OFFSET,
        baseline_y(8.4, 80.0),
        80.0,
        colors.heading,
        ROLE_HEADING,
        atlas,
        0,
    );

    emit_text(
        &mut instances,
        "What needs to be done?",
        135.0 + X_OFFSET,
        baseline_y(148.4, 24.0),
        24.0,
        colors.placeholder,
        ROLE_PLACEHOLDER,
        atlas,
        ITALIC_CODEPOINT_OFFSET,
    );

    emit_text(
        &mut instances,
        "Read documentation",
        135.0 + X_OFFSET,
        baseline_y(211.6, 24.0),
        24.0,
        colors.active,
        ROLE_ACTIVE,
        atlas,
        0,
    );
    emit_text(
        &mut instances,
        "Finish TodoMVC renderer",
        135.0 + X_OFFSET,
        baseline_y(271.2, 24.0),
        24.0,
        colors.completed,
        ROLE_COMPLETED,
        atlas,
        0,
    );
    emit_text(
        &mut instances,
        "Walk the dog",
        135.0 + X_OFFSET,
        baseline_y(330.8, 24.0),
        24.0,
        colors.active,
        ROLE_ACTIVE,
        atlas,
        0,
    );
    emit_text(
        &mut instances,
        "Buy groceries",
        135.0 + X_OFFSET,
        baseline_y(390.4, 24.0),
        24.0,
        colors.active,
        ROLE_ACTIVE,
        atlas,
        0,
    );

    let footer_x = emit_text(
        &mut instances,
        "3",
        90.0 + X_OFFSET,
        baseline_y(445.0, 15.0),
        15.0,
        colors.body,
        ROLE_BODY,
        atlas,
        0,
    );
    emit_text(
        &mut instances,
        " items left",
        footer_x,
        baseline_y(445.0, 15.0),
        15.0,
        colors.body,
        ROLE_BODY,
        atlas,
        0,
    );

    emit_text(
        &mut instances,
        "All",
        263.4 + X_OFFSET,
        baseline_y(445.0, 15.0),
        15.0,
        colors.body,
        ROLE_BODY,
        atlas,
        0,
    );
    emit_text(
        &mut instances,
        "Active",
        301.6 + X_OFFSET,
        baseline_y(445.0, 15.0),
        15.0,
        colors.body,
        ROLE_BODY,
        atlas,
        0,
    );
    emit_text(
        &mut instances,
        "Completed",
        364.1 + X_OFFSET,
        baseline_y(445.0, 15.0),
        15.0,
        colors.body,
        ROLE_BODY,
        atlas,
        0,
    );
    emit_text(
        &mut instances,
        "Clear completed",
        500.8 + X_OFFSET,
        baseline_y(445.0, 15.0),
        15.0,
        colors.body,
        ROLE_BODY,
        atlas,
        0,
    );

    emit_text(
        &mut instances,
        "Double-click to edit a todo",
        286.7 + X_OFFSET,
        baseline_y(538.4, 11.0),
        11.0,
        colors.info,
        ROLE_INFO,
        atlas,
        0,
    );
    emit_text(
        &mut instances,
        "Created by Martin Kav\u{00ED}k",
        291.5 + X_OFFSET,
        baseline_y(560.4, 11.0),
        11.0,
        colors.info,
        ROLE_INFO,
        atlas,
        0,
    );
    emit_text(
        &mut instances,
        "Part of TodoMVC",
        308.1 + X_OFFSET,
        baseline_y(582.4, 11.0),
        11.0,
        colors.info,
        ROLE_INFO,
        atlas,
        0,
    );

    instances
}
