use anyhow::{bail, Context, Result};
use image::{codecs::png::PngEncoder, RgbaImage};
use screenshots::Screen;

use crate::config::CaptureMode;

#[derive(Debug, Clone)]
pub struct CaptureResult {
    pub width: u32,
    pub height: u32,
    pub png_bytes: Vec<u8>,
}

pub fn capture_screen(mode: CaptureMode) -> Result<CaptureResult> {
    let screen = match mode {
        CaptureMode::Primary | CaptureMode::ActiveMonitor => primary_screen()?,
        CaptureMode::Region => {
            bail!("region capture is not yet implemented in the Rust rewrite")
        }
    };

    let image = screen
        .capture()
        .context("failed to capture monitor frame buffer")?;
    let width = image.width();
    let height = image.height();

    // Convert BGRA to RGBA
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for pixel in image.rgba().chunks_exact(4) {
        let b = pixel[0];
        let g = pixel[1];
        let r = pixel[2];
        let a = pixel[3];
        rgba.extend_from_slice(&[r, g, b, a]);
    }

    let img = RgbaImage::from_raw(width, height, rgba)
        .context("failed to construct RGBA image from raw capture buffer")?;

    let mut png_bytes = Vec::new();
    {
        use image::ImageEncoder;
        let encoder = PngEncoder::new(&mut png_bytes);
        encoder
            .write_image(&img, width, height, image::ColorType::Rgba8.into())
            .context("failed to encode screenshot to PNG")?;
    }

    Ok(CaptureResult {
        width,
        height,
        png_bytes,
    })
}

fn primary_screen() -> Result<Screen> {
    let screens = Screen::all().context("failed to enumerate displays")?;
    screens.into_iter().next().context("no displays detected")
}
