pub const WIDTH: u32 = 800;
pub const HEIGHT: u32 = 600;

#[cfg(not(feature = "windowed"))]
pub const TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
