use windows_capture::frame::{Frame, Error};
use windows_capture::settings::ColorFormat;
use image::{DynamicImage, GenericImageView, GrayImage, ImageBuffer, Luma, Rgba};


pub fn image_from_frame(frame: &mut Frame) -> Result<DynamicImage, Error> {
    let data = frame.buffer()?.as_nopadding_buffer()?.to_vec();
    let width = frame.width();
    let height = frame.height();
    match frame.color_format() {
        ColorFormat::Bgra8 => {
            // BGRA -> RGBA
            let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);
            for chunk in data.chunks_exact(4) {
                rgba_data.extend_from_slice(&[chunk[2], chunk[1], chunk[0], 255]); // R,G,B,A=255
            }
            let img = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, rgba_data)
                .ok_or(Error::InvalidSize)?;
            Ok(DynamicImage::ImageRgba8(img))
        }
        ColorFormat::Rgba8 => {
            // RGBA -> RGBA
            let mut rgba_data = Vec::with_capacity((width * height * 3) as usize);
            for chunk in data.chunks_exact(4) {
                rgba_data.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]); // R,G,B,A=255
            }
            let img = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, rgba_data)
                .ok_or(Error::InvalidSize)?;
            Ok(DynamicImage::ImageRgba8(img))
        }
        _ => Err(Error::UnsupportedFormat)
    }
}

pub fn bitmap(src: &DynamicImage, x: u32, y: u32, w: u32, h:u32, threshold: u8) -> GrayImage {
    let mut out = GrayImage::new(w, h);
    for dy in 0..h {
        for dx in 0..w {
            let p = src.get_pixel(x + dx, y + dy);
            let r = p[0] as u16;
            let g = p[1] as u16;
            let b = p[2] as u16;

            let lum = ((r * 77 + g * 150 + b * 29) >> 8) as u8;
            let v = if lum >= threshold { 255 } else { 0 };
            out.put_pixel(dx, dy, Luma([v]));
        }
    }
    return out;
}

pub fn bitline(src: &DynamicImage, x: u32, y: u32, w: u32, threshold: u8) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(w as usize);
    for dx in 0..w {
        let p = src.get_pixel(x + dx, y);
        let r = p[0] as u16;
        let g = p[1] as u16;
        let b = p[2] as u16;

        let lum = ((r * 77 + g * 150 + b * 29) >> 8) as u8;
        let v = if lum >= threshold { 255 } else { 0 };
        out.push(v);
    }
    return out;
}
