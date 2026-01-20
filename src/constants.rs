pub const WIDTH: u32 = 800;
pub const HEIGHT: u32 = 600;

#[cfg(any(not(feature = "windowed"), target_arch = "wasm32"))]
pub const TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
