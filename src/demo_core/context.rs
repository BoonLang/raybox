//! Demo context for initialization
//!
//! Provides GPU resources needed during demo creation.

/// Context provided during demo creation
pub struct DemoContext<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub surface_format: wgpu::TextureFormat,
    pub width: u32,
    pub height: u32,
}
