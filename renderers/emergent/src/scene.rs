//! Scene graph for emergent renderer
//!
//! Elements are described by:
//! - Position (center in 3D space)
//! - Size (half-extents)
//! - Material (color, roughness)
//! - Depth offset (move_closer / move_further)

use bytemuck::{Pod, Zeroable};

/// Maximum number of elements in a scene (GPU buffer limit)
pub const MAX_ELEMENTS: usize = 64;

/// Element shape type
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShapeType {
    Box = 0,
    RoundedBox = 1,
    Sphere = 2,
    Ring = 3,       // Hollow circle (for unchecked checkboxes)
    TodosText = 4,  // Procedural "todos" text
}

/// A single element in the scene
#[derive(Clone, Debug)]
pub struct Element {
    pub center: [f32; 3],
    pub half_extents: [f32; 3],
    pub color: [f32; 3],
    pub corner_radius: f32,
    pub shape_type: ShapeType,
    pub depth_offset: f32, // move_closer (positive) or move_further (negative)
}

impl Element {
    pub fn new_box(center: [f32; 3], half_extents: [f32; 3], color: [f32; 3], depth_offset: f32) -> Self {
        Self {
            center,
            half_extents,
            color,
            corner_radius: 0.0,
            shape_type: ShapeType::Box,
            depth_offset,
        }
    }

    pub fn new_rounded_box(
        center: [f32; 3],
        half_extents: [f32; 3],
        color: [f32; 3],
        corner_radius: f32,
        depth_offset: f32,
    ) -> Self {
        Self {
            center,
            half_extents,
            color,
            corner_radius,
            shape_type: ShapeType::RoundedBox,
            depth_offset,
        }
    }

    #[allow(dead_code)]
    pub fn new_sphere(center: [f32; 3], radius: f32, color: [f32; 3], depth_offset: f32) -> Self {
        Self {
            center,
            half_extents: [radius, radius, radius],
            color,
            corner_radius: radius,
            shape_type: ShapeType::Sphere,
            depth_offset,
        }
    }

    /// Create a ring (hollow circle) - good for unchecked checkboxes
    /// outer_radius: the outer edge of the ring
    /// thickness: the width of the ring stroke
    pub fn new_ring(
        center: [f32; 3],
        outer_radius: f32,
        thickness: f32,
        color: [f32; 3],
        depth_offset: f32,
    ) -> Self {
        Self {
            center,
            half_extents: [outer_radius, outer_radius, 0.01],
            color,
            corner_radius: thickness, // Store thickness in corner_radius for Ring shape
            shape_type: ShapeType::Ring,
            depth_offset,
        }
    }

    /// Create procedural "todos" text
    /// width: the width of the text bounding box (used to scale letters)
    pub fn new_todos_text(
        center: [f32; 3],
        width: f32,
        height: f32,
        color: [f32; 3],
        depth_offset: f32,
    ) -> Self {
        Self {
            center,
            half_extents: [width / 2.0, height / 2.0, 0.01],
            color,
            corner_radius: width, // Store full width for text scaling
            shape_type: ShapeType::TodosText,
            depth_offset,
        }
    }

    /// Convert to GPU-friendly format
    pub fn to_gpu(&self) -> ElementGpu {
        ElementGpu {
            center: [
                self.center[0],
                self.center[1],
                self.center[2] + self.depth_offset,
                0.0,
            ],
            half_extents: [
                self.half_extents[0],
                self.half_extents[1],
                self.half_extents[2],
                0.0,
            ],
            color: [self.color[0], self.color[1], self.color[2], 1.0],
            params: [
                self.corner_radius,
                self.shape_type as u32 as f32,
                0.0,
                0.0,
            ],
        }
    }
}

/// GPU-friendly element representation (aligned to 16 bytes)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ElementGpu {
    pub center: [f32; 4],       // xyz + padding
    pub half_extents: [f32; 4], // xyz + padding
    pub color: [f32; 4],        // rgb + alpha
    pub params: [f32; 4],       // corner_radius, shape_type, reserved, reserved
}

/// Scene containing all elements
pub struct Scene {
    pub elements: Vec<Element>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
        }
    }

    pub fn add_element(&mut self, element: Element) {
        if self.elements.len() < MAX_ELEMENTS {
            self.elements.push(element);
        }
    }

    /// Convert to GPU buffer data
    pub fn to_gpu_buffer(&self) -> Vec<ElementGpu> {
        self.elements.iter().map(|e| e.to_gpu()).collect()
    }

    pub fn element_count(&self) -> u32 {
        self.elements.len() as u32
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}
