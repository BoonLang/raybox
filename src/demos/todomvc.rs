//! Demo 7: TodoMVC SDF Rendering
//!
//! Renders the classic TodoMVC UI using SDF primitives (rectangles, circles,
//! lines, shadows) and exact Bézier text SDF computation from vector font data.

use super::{Demo, DemoContext, DemoId, DemoType, KEYBINDINGS_2D};
use crate::camera::FlyCamera;
use crate::input::CameraConfig;
use crate::shader_bindings::sdf_todomvc;
use crate::text::{VectorFont, VectorFontAtlas, build_char_grid};
use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use std::any::Any;
use wgpu::util::DeviceExt;

// No X offset — virtual viewport matches metadata 700x700
const X_OFFSET: f32 = 0.0;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Uniforms {
    screen_size: [f32; 2],
    offset: [f32; 2],
    text_params: [f32; 4], // charCount, scale, rotation, uiPrimCount
    char_grid_params: [f32; 4],
    char_grid_bounds: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct GpuGridCell {
    curve_start_and_count: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct GpuBezierCurve {
    points01: [f32; 4],
    points2bbox: [f32; 4],
    bbox_flags: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct GpuGlyphData {
    bounds: [f32; 4],
    grid_info: [u32; 4],
    curve_info: [u32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct GpuCharInstanceEx {
    pos_and_char: [f32; 4], // x, y, scale, glyphIndex
    color_flags: [f32; 4],  // r, g, b, flags
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct GpuUiPrimitive {
    pos_size: [f32; 4],
    color: [f32; 4],
    params: [f32; 4],  // cornerRadius, borderWidth, blur, type
    extra: [f32; 4],
}

// UI primitive type constants (matching shader UiPrimitive type field)
#[allow(dead_code)]
const PRIM_FILLED_RECT: f32 = 0.0;
#[allow(dead_code)]
const PRIM_STROKED_RECT: f32 = 1.0;
#[allow(dead_code)]
const PRIM_FILLED_CIRCLE: f32 = 2.0;
#[allow(dead_code)]
const PRIM_STROKED_CIRCLE: f32 = 3.0;
#[allow(dead_code)]
const PRIM_LINE: f32 = 4.0;
#[allow(dead_code)]
const PRIM_BOX_SHADOW: f32 = 5.0;
#[allow(dead_code)]
const PRIM_CHECKMARK_V: f32 = 6.0;

/// Color helper: 0-255 sRGB → float4 (kept in sRGB space for compositing;
/// shader converts final result to linear before framebuffer write)
fn rgb(r: u8, g: u8, b: u8) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

fn rgba(r: u8, g: u8, b: u8, a: f32) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a]
}

/// 0-255 sRGB → float3 (for text colors, kept in sRGB space)
fn rgb3(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

/// Flip Y from metadata Y-down to shader Y-up
fn fy(y: f32) -> f32 { SCREEN_H - y }

/// Make a rect in Y-up coords from metadata Y-down: [x, fy(y+h), w, h]
fn rect_yu(x: f32, y: f32, w: f32, h: f32) -> [f32; 4] {
    [x, fy(y + h), w, h]
}

/// Make a horizontal line in Y-up coords: [x1, fy(y), x2, fy(y)]
fn hline_yu(x1: f32, y: f32, x2: f32) -> [f32; 4] {
    [x1, fy(y), x2, fy(y)]
}

const SCREEN_H: f32 = 700.0;

/// Codepoint offset for italic font glyphs (merged into same atlas)
const ITALIC_CODEPOINT_OFFSET: u32 = 0x10000;

/// Build UI primitives (back-to-front order) from hardcoded metadata values.
/// All metadata Y coordinates are flipped from Y-down to shader Y-up.
fn build_ui_primitives() -> Vec<GpuUiPrimitive> {
    let mut prims = Vec::new();

    // Card coordinates from metadata (Y-down)
    let card_x = 75.0 + X_OFFSET;
    let card_y = 130.0; // metadata Y-down
    let card_w = 550.0;
    let card_h = 344.2;

    // --- Box shadows (rendered first, behind everything) ---

    // Shadow 2 (larger, softer): rgba(0,0,0,0.1) 0px 25px 50px
    // CSS positive Y offset = down → negate for Y-up
    prims.push(GpuUiPrimitive {
        pos_size: rect_yu(card_x, card_y, card_w, card_h),
        color: rgba(0, 0, 0, 0.1),
        params: [0.0, 0.0, 50.0, PRIM_BOX_SHADOW],
        extra: [0.0, -25.0, 0.0, 0.0],
    });

    // Shadow 1 (smaller, sharper): rgba(0,0,0,0.2) 0px 2px 4px
    prims.push(GpuUiPrimitive {
        pos_size: rect_yu(card_x, card_y, card_w, card_h),
        color: rgba(0, 0, 0, 0.2),
        params: [0.0, 0.0, 4.0, PRIM_BOX_SHADOW],
        extra: [0.0, -2.0, 0.0, 0.0],
    });

    // --- Stacked card decorative rects (below card in Y-down = below in Y-up) ---
    let stack2_y = card_y + card_h + 8.0;
    prims.push(GpuUiPrimitive {
        pos_size: rect_yu(card_x + 4.0, stack2_y, card_w - 8.0, 4.0),
        color: rgb(246, 246, 246),
        params: [0.0, 0.0, 0.0, PRIM_FILLED_RECT],
        extra: [0.0; 4],
    });
    prims.push(GpuUiPrimitive {
        pos_size: rect_yu(card_x + 4.0, stack2_y, card_w - 8.0, 4.0),
        color: rgba(0, 0, 0, 0.2),
        params: [0.0, 0.0, 2.0, PRIM_BOX_SHADOW],
        extra: [0.0, -1.0, 0.0, 0.0],
    });

    let stack1_y = card_y + card_h + 4.0;
    prims.push(GpuUiPrimitive {
        pos_size: rect_yu(card_x + 2.0, stack1_y, card_w - 4.0, 4.0),
        color: rgb(246, 246, 246),
        params: [0.0, 0.0, 0.0, PRIM_FILLED_RECT],
        extra: [0.0; 4],
    });
    prims.push(GpuUiPrimitive {
        pos_size: rect_yu(card_x + 2.0, stack1_y, card_w - 4.0, 4.0),
        color: rgba(0, 0, 0, 0.2),
        params: [0.0, 0.0, 2.0, PRIM_BOX_SHADOW],
        extra: [0.0, -1.0, 0.0, 0.0],
    });

    // --- Main card fill (white) ---
    prims.push(GpuUiPrimitive {
        pos_size: rect_yu(card_x, card_y, card_w, card_h),
        color: rgb(255, 255, 255),
        params: [0.0, 0.0, 0.0, PRIM_FILLED_RECT],
        extra: [0.0; 4],
    });

    // --- Input area inset shadow (at top of card in Y-down) ---
    prims.push(GpuUiPrimitive {
        pos_size: rect_yu(card_x, card_y, card_w, 2.0),
        color: rgba(0, 0, 0, 0.03),
        params: [0.0, 0.0, 0.0, PRIM_FILLED_RECT],
        extra: [0.0; 4],
    });

    // --- Separator lines between todo items ---
    let sep_color = rgba(237, 237, 237, 1.0); // #ededed
    for &y in &[255.4_f32, 315.0, 374.6] {
        prims.push(GpuUiPrimitive {
            pos_size: hline_yu(card_x, y, card_x + card_w),
            color: sep_color,
            params: [0.0, 0.8, 0.0, PRIM_LINE],
            extra: [0.0; 4],
        });
    }

    // --- Footer top border ---
    prims.push(GpuUiPrimitive {
        pos_size: hline_yu(card_x, 433.4, card_x + card_w),
        color: rgba(230, 230, 230, 1.0), // #e6e6e6
        params: [0.0, 0.8, 0.0, PRIM_LINE],
        extra: [0.0; 4],
    });

    // --- Border between input and todo list ---
    prims.push(GpuUiPrimitive {
        pos_size: hline_yu(card_x, 195.8, card_x + card_w),
        color: sep_color,
        params: [0.0, 0.8, 0.0, PRIM_LINE],
        extra: [0.0; 4],
    });

    // --- Toggle-all chevron (❯ rotated 90° = ∨ pointing down) ---
    // Label area: x=75, y=130.8, 45x65. Chevron center ~(97.5, 163)
    {
        let chev_cx = card_x + 22.5;
        let chev_cy_down = 130.8 + 32.5; // center in Y-down
        let half_w = 7.0;
        let half_h = 4.0;
        // ∨ shape: left-top → bottom-center, bottom-center → right-top (in Y-down)
        let left = (chev_cx - half_w, chev_cy_down - half_h);
        let bottom = (chev_cx, chev_cy_down + half_h);
        let right = (chev_cx + half_w, chev_cy_down - half_h);
        prims.push(GpuUiPrimitive {
            pos_size: [left.0, fy(left.1), bottom.0, fy(bottom.1)],
            color: rgb(0x94, 0x94, 0x94), // #949494
            params: [0.0, 1.0, 0.0, PRIM_CHECKMARK_V],
            extra: [right.0, fy(right.1), 0.0, 0.0],
        });
    }

    // --- Focus border around input area ---
    // Thin reddish border visible in reference when input is focused
    prims.push(GpuUiPrimitive {
        pos_size: rect_yu(card_x, card_y, card_w, 65.8),
        color: rgba(0xCE, 0x46, 0x46, 0.6), // #ce4646 semi-transparent
        params: [0.0, 0.8, 0.0, PRIM_STROKED_RECT],
        extra: [0.0; 4],
    });

    // --- Checkbox circles ---
    // Center at card_x + 26, item midpoint Y, radius ~14
    // Reference order top→bottom: Read doc, Finish TodoMVC (checked), Walk the dog, Buy groceries
    struct CheckboxInfo { y_center: f32, checked: bool }
    let checkboxes = [
        CheckboxInfo { y_center: 195.8 + 29.8, checked: false },  // Read documentation
        CheckboxInfo { y_center: 255.4 + 29.8, checked: true },   // Finish TodoMVC renderer
        CheckboxInfo { y_center: 315.0 + 29.8, checked: false },  // Walk the dog
        CheckboxInfo { y_center: 374.6 + 29.4, checked: false },  // Buy groceries
    ];

    for cb in &checkboxes {
        let cx = card_x + 26.0;
        let cy = fy(cb.y_center); // flip to Y-up
        let r = 14.0;
        let stroke_w = 1.2;

        if cb.checked {
            // Green circle: stroke #59A193
            prims.push(GpuUiPrimitive {
                pos_size: [cx, cy, r, 0.0],
                color: rgb(0x59, 0xA1, 0x93),
                params: [0.0, stroke_w, 0.0, PRIM_STROKED_CIRCLE],
                extra: [0.0; 4],
            });
            // Checkmark V: SVG path M72 25 L42 71 L27 56
            // SVG Y is Y-down, negate Y offset from center for Y-up
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
                params: [0.0, 1.2, 0.0, PRIM_CHECKMARK_V],
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

    // --- Strikethrough on completed item ("Finish TodoMVC renderer") ---
    // Now at position 2, Y=271.2 (reference order)
    // Text bounds: x=135.0, y=271.2, w=273.0, h=26.4 (metadata Y-down)
    let strike_y = fy(271.2 + 13.2); // middle of text height, flipped to Y-up
    prims.push(GpuUiPrimitive {
        pos_size: [135.0 + X_OFFSET, strike_y, 135.0 + 273.0 + X_OFFSET, strike_y],
        color: rgb(0x94, 0x94, 0x94),
        params: [0.0, 0.8, 0.0, PRIM_LINE],
        extra: [0.0; 4],
    });

    // --- "All" filter button border (selected state) ---
    // Button bounds: x=255.6, y=441.2, w=32.3, h=24.4
    prims.push(GpuUiPrimitive {
        pos_size: rect_yu(255.6 + X_OFFSET, 441.2, 32.3, 24.4),
        color: rgba(0xCE, 0x46, 0x46, 1.0), // #ce4646
        params: [3.0, 0.8, 0.0, PRIM_STROKED_RECT],
        extra: [0.0; 4],
    });

    prims
}

/// Build text layout from hardcoded TodoMVC content
fn build_text_layout(atlas: &VectorFontAtlas) -> Vec<GpuCharInstanceEx> {
    let mut instances = Vec::new();

    /// Emit characters for a text run, returning the final X position.
    /// codepoint_offset: 0 for regular, ITALIC_CODEPOINT_OFFSET for italic
    fn emit_text_with_offset(
        instances: &mut Vec<GpuCharInstanceEx>,
        text: &str,
        x: f32,
        baseline_y: f32,
        font_size: f32,
        color: [f32; 3],
        atlas: &VectorFontAtlas,
        codepoint_offset: u32,
    ) -> f32 {
        let mut cx = x;
        for ch in text.chars() {
            if ch == ' ' {
                // Space advance: try offset version first, then regular
                let adv = atlas.glyphs.get(&(' ' as u32 + codepoint_offset))
                    .or_else(|| atlas.glyphs.get(&(' ' as u32)))
                    .map(|e| e.advance)
                    .unwrap_or(0.25);
                cx += adv * font_size;
                continue;
            }
            let codepoint = ch as u32 + codepoint_offset;
            if let Some(idx) = atlas.glyph_list
                .iter()
                .position(|(cp, _)| *cp == codepoint)
            {
                let entry = &atlas.glyph_list[idx].1;
                instances.push(GpuCharInstanceEx {
                    pos_and_char: [cx, baseline_y, font_size, idx as f32],
                    color_flags: [color[0], color[1], color[2], 0.0],
                });
                cx += entry.advance * font_size;
            }
        }
        cx
    }

    fn emit_text(
        instances: &mut Vec<GpuCharInstanceEx>,
        text: &str, x: f32, baseline_y: f32, font_size: f32,
        color: [f32; 3], atlas: &VectorFontAtlas,
    ) -> f32 {
        emit_text_with_offset(instances, text, x, baseline_y, font_size, color, atlas, 0)
    }

    // Color constants (converted from sRGB CSS values to linear for sRGB framebuffer)
    let heading_color = rgb3(184, 63, 69); // #b83f45
    let active_text = rgb3(72, 72, 72); // #484848
    let completed_text = rgb3(148, 148, 148); // #949494
    let body_text = rgb3(17, 17, 17); // #111
    let info_text_light = rgb3(191, 191, 191); // #bfbfbf

    // --- "todos" heading ---
    // Text bounds from metadata: x=252.1, y=8.4, w=195.7, h=89.6
    // Font size 80px, centered
    // baseline = y + ascent (but metadata says text top at y=8.4 with font-size 80px)
    // For Liberation Sans at 80px: ascent ~= 72px from top of em
    // Use metadata text bounds top y=8.4, ascent in em ~0.9, so baseline ~ 8.4 + 72 = 80.4
    // But we need Y in our coordinate system. Metadata Y is from top.
    // Our shader converts: font coords have Y-up from baseline.
    // We'll place characters at (x, screen_height - baseline_y) since shader flips Y.
    // Actually the shader uses Y-down coordinates directly (pixelPos.y = metadata y).
    // So baseline_y in screen coords = text_top_y + ascent_in_pixels
    // For Liberation Sans, ascent/unitsPerEm ratio ~ 0.9 (ascender is ~900 of 1000 units)
    // Baseline Y = 8.4 + 0.9 * 80 = 80.4 (metadata coords, Y-down)
    // But wait — our text rendering expects baseline Y in a Y-up system because the glyph
    // coordinates have Y-up. The shader then reads pixelPos (Y-down) and does
    // localPos = (pixelPos - charPos) / charScale where charPos.y is what we set.
    // For Y-down pixel coords, if we set charPos.y = baseline_y (Y-down),
    // then localPos.y = (pixelY - baseline_y) / scale.
    // Glyph coords have Y-up, so descenders go below baseline (negative y) and
    // ascenders go above (positive y). But in Y-down screen space, above = smaller y.
    // So localPos.y would be negative for pixels above baseline, which maps correctly
    // to the Y-up glyph system IF we negate: localPos.y = (baseline_y - pixelY) / scale.
    // But our shader does localPos = (pixelPos - charPos) / charScale with no negation.
    // This means we need to invert the glyph rendering... OR we can use the same
    // convention as text2d where charPos.y is in the font's coordinate system.
    // text2d uses start_y = height - margin - font_size with Y going UP from 0 at bottom.
    // But in our shader, pixelPos for text2d is also bottom-up (1.0-uv.y)*screenSize.
    //
    // For TodoMVC shader, we changed it: pixelPos.y = (1 - uv.y) * screenSize.y for Y-down.
    // Wait, let me re-read the shader... Actually:
    //   pixelPos = float2(input.uv.x * screenSize.x, (1.0 - input.uv.y) * screenSize.y)
    // With uv.y going from 1 (bottom of screen) to -1 (top), but the UV mapping
    // has uvs = {(0,1), (2,1), (0,-1)} so at the top of screen uv.y=0 (interpolated),
    // middle uv.y=0.5, bottom uv.y=1.0.
    // So (1 - uv.y) at top = 1.0, at bottom = 0.0 → pixelPos.y at top = screenSize.y,
    // at bottom = 0.0. This is Y-UP (bottom = 0, top = screenHeight).
    //
    // For Y-down metadata coordinates, we need to flip: metadata_y = screenHeight - pixelPos.y.
    // OR we place things in Y-up coordinates.
    //
    // Since the glyph coordinate system is already Y-up and the text2d shader uses Y-up pixel
    // coordinates, let's keep the same Y-up convention:
    //   charPos.y = screen_height - metadata_baseline_y
    //
    // metadata_baseline_y = text_top_y + ascent_fraction * font_size
    // For Liberation Sans: ascent ~= 0.905 * unitsPerEm (from OS/2 table, typically 905/1000)

    let screen_h = 700.0_f32;

    // Ascent ratio for Liberation Sans (ascender / unitsPerEm ≈ 0.905)
    let ascent_ratio = 0.905;

    // Helper: convert metadata Y-down to our Y-up coordinate for baseline placement
    let baseline_y = |text_top_y: f32, font_size: f32| -> f32 {
        let metadata_baseline = text_top_y + ascent_ratio * font_size;
        screen_h - metadata_baseline
    };

    // --- "todos" heading: 80px, #b83f45 ---
    // Text bounds: x=252.1, y=8.4
    emit_text(&mut instances, "todos", 252.1 + X_OFFSET, baseline_y(8.4, 80.0), 80.0, heading_color, atlas);

    // --- Placeholder text: "What needs to be done?" (italic) ---
    // rgba(0,0,0,0.4) on white bg → premultiplied gray, converted to linear
    let placeholder_gray = rgb3(153, 153, 153); // ~rgba(0,0,0,0.4) on white
    // Input padding-left=60, text starts at x=75+60=135, y=130+16 (padding-top=16)
    emit_text_with_offset(&mut instances, "What needs to be done?", 135.0 + X_OFFSET, baseline_y(146.0, 24.0), 24.0, placeholder_gray, atlas, ITALIC_CODEPOINT_OFFSET);

    // --- Todo items: 24px ---
    // Reference order (top→bottom): Read documentation, Finish TodoMVC renderer, Walk the dog, Buy groceries
    // Item 1: "Read documentation" - Y=211.6
    emit_text(&mut instances, "Read documentation", 135.0 + X_OFFSET, baseline_y(211.6, 24.0), 24.0, active_text, atlas);

    // Item 2: "Finish TodoMVC renderer" - Y=271.2 (completed, gray)
    emit_text(&mut instances, "Finish TodoMVC renderer", 135.0 + X_OFFSET, baseline_y(271.2, 24.0), 24.0, completed_text, atlas);

    // Item 3: "Walk the dog" - Y=330.8
    emit_text(&mut instances, "Walk the dog", 135.0 + X_OFFSET, baseline_y(330.8, 24.0), 24.0, active_text, atlas);

    // Item 4: "Buy groceries" - Y=390.4
    emit_text(&mut instances, "Buy groceries", 135.0 + X_OFFSET, baseline_y(390.4, 24.0), 24.0, active_text, atlas);

    // --- Footer: "3 items left" ---
    // "3" at x=90, y=445, 15px, #111
    // " items left" at x=98.3, y=445
    let fx = emit_text(&mut instances, "3", 90.0 + X_OFFSET, baseline_y(445.0, 15.0), 15.0, body_text, atlas);
    emit_text(&mut instances, " items left", fx, baseline_y(445.0, 15.0), 15.0, body_text, atlas);

    // --- Filter buttons: "All", "Active", "Completed" ---
    // "All" at x=263.4, y=445, 15px
    emit_text(&mut instances, "All", 263.4 + X_OFFSET, baseline_y(445.0, 15.0), 15.0, body_text, atlas);
    // "Active" at x=301.6, y=445
    emit_text(&mut instances, "Active", 301.6 + X_OFFSET, baseline_y(445.0, 15.0), 15.0, body_text, atlas);
    // "Completed" at x=364.1, y=445
    emit_text(&mut instances, "Completed", 364.1 + X_OFFSET, baseline_y(445.0, 15.0), 15.0, body_text, atlas);

    // --- "Clear completed" button ---
    // At x=500.8, y=445, 15px
    emit_text(&mut instances, "Clear completed", 500.8 + X_OFFSET, baseline_y(445.0, 15.0), 15.0, body_text, atlas);

    // --- Info footer (3 lines at ~11px, #bfbfbf) ---
    // Line 1: "Double-click to edit a todo" at x=286.7, y=538.4
    emit_text(&mut instances, "Double-click to edit a todo", 286.7 + X_OFFSET, baseline_y(538.4, 11.0), 11.0, info_text_light, atlas);
    // Line 2: "Created by Martin Kavík" at x=283.0, y=560.4
    emit_text(&mut instances, "Created by Martin Kav\u{00ED}k", 283.0 + X_OFFSET, baseline_y(560.4, 11.0), 11.0, info_text_light, atlas);
    // Line 3: "Part of TodoMVC" at x=308.1, y=582.4
    emit_text(&mut instances, "Part of TodoMVC", 308.1 + X_OFFSET, baseline_y(582.4, 11.0), 11.0, info_text_light, atlas);

    instances
}

pub struct TodoMvcDemo {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    char_count: u32,
    ui_prim_count: u32,
    char_grid_params: [f32; 4],
    char_grid_bounds: [f32; 4],
    width: u32,
    height: u32,
    // 2D controls state
    pub offset: [f32; 2],
    pub scale: f32,
    pub rotation: f32,
}

impl TodoMvcDemo {
    pub fn new(ctx: &DemoContext) -> Result<Self> {
        // Load Liberation Sans Regular + Italic (italic glyphs at codepoint offset 0x10000)
        let font_data = std::fs::read("assets/fonts/LiberationSans-Regular.ttf")
            .context("Failed to load Liberation Sans Regular font")?;
        let mut font = VectorFont::from_ttf(&font_data).map_err(|e| anyhow::anyhow!(e))?;

        let italic_data = std::fs::read("assets/fonts/LiberationSans-Italic.ttf")
            .context("Failed to load Liberation Sans Italic font")?;
        font.merge_from_ttf(&italic_data, ITALIC_CODEPOINT_OFFSET)
            .map_err(|e| anyhow::anyhow!(e))?;

        let atlas = VectorFontAtlas::from_font(&font, 32);

        // Build UI primitives
        let ui_primitives = build_ui_primitives();
        let ui_prim_count = ui_primitives.len() as u32;

        // Build text layout
        let char_instances = build_text_layout(&atlas);
        let char_count = char_instances.len() as u32;

        // Build character spatial grid — extract first 4 floats from each CharInstanceEx
        let instance_data: Vec<[f32; 4]> = char_instances.iter().map(|c| c.pos_and_char).collect();
        let char_grid = build_char_grid(&instance_data, &atlas, [64, 48]);

        let char_grid_params = [
            char_grid.dims[0] as f32,
            char_grid.dims[1] as f32,
            char_grid.cell_size[0],
            char_grid.cell_size[1],
        ];
        let char_grid_bounds = char_grid.bounds;

        // Prepare GPU data (same as text2d)
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

        // Create buffers
        let uniforms = Uniforms {
            screen_size: [ctx.width as f32, ctx.height as f32],
            offset: [0.0, 0.0],
            text_params: [char_count as f32, 1.0, 0.0, ui_prim_count as f32],
            char_grid_params,
            char_grid_bounds,
        };

        let uniform_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("TodoMVC Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

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
            usage: wgpu::BufferUsages::STORAGE,
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
            label: Some("UI Primitives Buffer"),
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

        // Create bind group layout (9 bindings: uniform + 7 storage from text2d + UI primitives)
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
            label: Some("TodoMVC Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                storage_entry(1),
                storage_entry(2),
                storage_entry(3),
                storage_entry(4),
                storage_entry(5),
                storage_entry(6),
                storage_entry(7),
                storage_entry(8),
            ],
        });

        let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TodoMVC Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: grid_cells_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: curve_indices_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: curves_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: glyph_data_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: char_instances_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: char_grid_cells_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: char_grid_indices_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: ui_primitives_buffer.as_entire_binding(),
                },
            ],
        });

        // Create pipeline
        let shader_module = sdf_todomvc::create_shader_module_embed_source(ctx.device);

        let pipeline_layout = ctx.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("TodoMVC Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = ctx.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("TodoMVC Pipeline"),
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
            bind_group,
            char_count,
            ui_prim_count,
            char_grid_params,
            char_grid_bounds,
            width: ctx.width,
            height: ctx.height,
            offset: [0.0, 0.0],
            scale: 1.0,
            rotation: 0.0,
        })
    }

    fn update_uniforms(&self, queue: &wgpu::Queue) {
        let uniforms = Uniforms {
            screen_size: [self.width as f32, self.height as f32],
            offset: self.offset,
            text_params: [self.char_count as f32, self.scale, self.rotation, self.ui_prim_count as f32],
            char_grid_params: self.char_grid_params,
            char_grid_bounds: self.char_grid_bounds,
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    pub fn reset_rotation(&mut self) {
        self.rotation = 0.0;
    }

    pub fn reset_all(&mut self) {
        self.offset = [0.0, 0.0];
        self.scale = 1.0;
        self.rotation = 0.0;
    }
}

impl Demo for TodoMvcDemo {
    fn name(&self) -> &'static str {
        "TodoMVC"
    }

    fn id(&self) -> DemoId {
        DemoId::TodoMvc
    }

    fn demo_type(&self) -> DemoType {
        DemoType::Scene2D
    }

    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        KEYBINDINGS_2D
    }

    fn camera_config(&self) -> CameraConfig {
        CameraConfig::default()
    }

    fn update(&mut self, _dt: f32, _camera: &mut FlyCamera) {
        // 2D controls are handled by the runner
    }

    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, queue: &wgpu::Queue, _time: f32) {
        self.update_uniforms(queue);
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
