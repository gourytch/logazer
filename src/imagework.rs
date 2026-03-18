use windows_capture::frame::{Frame, Error};
use windows_capture::settings::ColorFormat;
use image::{DynamicImage, ImageBuffer, Rgba};


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

