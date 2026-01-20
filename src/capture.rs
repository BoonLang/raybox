use crate::constants::{HEIGHT, TEXTURE_FORMAT, WIDTH};
use anyhow::{Context, Result};
use std::path::Path;

/// Bytes per row must be aligned to COPY_BYTES_PER_ROW_ALIGNMENT (256)
const BYTES_PER_PIXEL: u32 = 4; // RGBA8
const UNPADDED_BYTES_PER_ROW: u32 = WIDTH * BYTES_PER_PIXEL;
const ALIGN: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
const PADDED_BYTES_PER_ROW: u32 = (UNPADDED_BYTES_PER_ROW + ALIGN - 1) / ALIGN * ALIGN;

/// Capture a texture to a PNG file
pub async fn capture_texture_to_png(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    output_path: &Path,
) -> Result<()> {
    // Create staging buffer for reading back the texture
    let buffer_size = (PADDED_BYTES_PER_ROW * HEIGHT) as u64;
    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Staging Buffer"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // Copy texture to buffer
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Copy Encoder"),
    });

    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging_buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(PADDED_BYTES_PER_ROW),
                rows_per_image: Some(HEIGHT),
            },
        },
        wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(std::iter::once(encoder.finish()));

    // Map the buffer and read back data
    let buffer_slice = staging_buffer.slice(..);

    // Create a channel for async notification
    let (tx, rx) = std::sync::mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        tx.send(result).unwrap();
    });

    // Poll the device to ensure the mapping completes
    device.poll(wgpu::PollType::Wait)?;

    // Wait for the mapping to complete
    rx.recv()
        .context("Failed to receive buffer mapping result")?
        .context("Failed to map buffer")?;

    // Read the data
    let padded_data = buffer_slice.get_mapped_range();

    // Remove padding and convert to image format
    let mut image_data = Vec::with_capacity((UNPADDED_BYTES_PER_ROW * HEIGHT) as usize);
    for row in 0..HEIGHT {
        let start = (row * PADDED_BYTES_PER_ROW) as usize;
        let end = start + UNPADDED_BYTES_PER_ROW as usize;
        image_data.extend_from_slice(&padded_data[start..end]);
    }

    // Drop the mapping before unmapping
    drop(padded_data);
    staging_buffer.unmap();

    // Save as PNG
    let color_type = match TEXTURE_FORMAT {
        wgpu::TextureFormat::Rgba8UnormSrgb | wgpu::TextureFormat::Rgba8Unorm => {
            image::ColorType::Rgba8
        }
        _ => anyhow::bail!("Unsupported texture format for PNG export"),
    };

    // Ensure output directory exists
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create output directory")?;
    }

    image::save_buffer(output_path, &image_data, WIDTH, HEIGHT, color_type)
        .context("Failed to save PNG")?;

    Ok(())
}
