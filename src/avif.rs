use aom_decode::avif::Avif;

use lodepng::RGB;
use rgb::{alt::GRAY8, RGBA};

use crate::common::{
    exif_orientation, orient_image, CompressResult, FastCompressResult, Image, ReadResult,
};

fn rgb16to8(p: RGB<u16>) -> RGB<u8> {
    RGB {
        r: (p.r / 256) as u8,
        g: (p.g / 256) as u8,
        b: (p.b / 256) as u8,
    }
}

fn rgba16to8(p: RGBA<u16>) -> RGBA<u8> {
    RGBA {
        r: (p.r / 256) as u8,
        g: (p.g / 256) as u8,
        b: (p.b / 256) as u8,
        a: (p.a / 256) as u8,
    }
}

pub fn read(buffer: &[u8]) -> ReadResult {
    let mut d = Avif::decode(
        buffer,
        &aom_decode::Config {
            threads: num_cpus::get(),
        },
    )
    .map_err(|err| format!("Failed to create decoder: {}", err))?;

    let image = match d
        .convert()
        .map_err(|err| format!("Failed to convert avif: {}", err))?
    {
        aom_decode::avif::Image::RGB8(img) => {
            Image::from_rgb(img.pixels().collect(), img.width(), img.height())
        }
        aom_decode::avif::Image::RGBA8(img) => {
            Image::from_rgba(img.pixels().collect(), img.width(), img.height())
        }
        aom_decode::avif::Image::RGB16(img) => Image::from_rgb(
            img.pixels().map(rgb16to8).collect(),
            img.width(),
            img.height(),
        ),
        aom_decode::avif::Image::RGBA16(img) => Image::from_rgba(
            img.pixels().map(rgba16to8).collect(),
            img.width(),
            img.height(),
        ),
        aom_decode::avif::Image::Gray8(img) => Image::from_gray(
            img.pixels().map(|p| GRAY8::new(p)).collect(),
            img.width(),
            img.height(),
        ),
        aom_decode::avif::Image::Gray16(img) => Image::from_gray(
            img.pixels().map(|p| GRAY8::new((p / 256) as u8)).collect(),
            img.width(),
            img.height(),
        ),
    };

    let orientation = exif::Reader::new()
        .read_from_container(&mut std::io::Cursor::new(buffer))
        .ok()
        .and_then(exif_orientation)
        .unwrap_or(1);

    Ok(orient_image(image, orientation))
}

fn compress_base(image: &Image, quality: u8, fast: bool) -> Result<Vec<u8>, String> {
    let has_alpha = image.has_alpha();

    let result = ravif::Encoder::new()
        .with_quality(quality as f32)
        .with_alpha_quality(if has_alpha { 100.0 } else { 1.0 })
        .with_internal_color_space(ravif::ColorSpace::YCbCr)
        .with_speed(if fast { 10 } else { 1 })
        .with_num_threads(Some(std::cmp::min(num_cpus::get(), 8)))
        .encode_rgba(ravif::Img::new(&image.data, image.width, image.height))
        .map_err(|err| format!("Failed to compress image: {}", err))?;

    Ok(result.avif_file)
}

pub fn compress_fast(image: &Image, quality: u8) -> FastCompressResult {
    let result = compress_base(image, quality, true)?;
    Ok(result)
}

pub fn compress(image: &Image, quality: u8) -> CompressResult {
    let result = compress_base(image, quality, false)?;
    Ok((read(&result)?, result))
}
