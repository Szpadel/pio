use aom_decode::avif::Avif;

use rgb::alt::GRAY8;

use crate::common::{
    exif_orientation, orient_image, CompressResult, FastCompressResult, Image, ReadResult,
};

pub fn read(buffer: &[u8]) -> ReadResult {
    let mut d = Avif::decode(buffer, &aom_decode::Config { threads: 1 })
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
        aom_decode::avif::Image::RGB16(_) => Err("16bit not supported")?,
        aom_decode::avif::Image::RGBA16(_) => Err("16bit not supported")?,
        aom_decode::avif::Image::Gray8(img) => Image::from_gray(
            img.pixels().map(|p| GRAY8::new(p)).collect(),
            img.width(),
            img.height(),
        ),
        aom_decode::avif::Image::Gray16(_) => Err("16bit not supported")?,
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
    let config = ravif::Config {
        quality: quality as f32,
        alpha_quality: if has_alpha { 100.0 } else { 0.0 },
        color_space: ravif::ColorSpace::YCbCr,
        premultiplied_alpha: false,
        speed: if fast { 10 } else { 1 },
        threads: 1,
    };

    let img = ravif::Img::new(image.data.clone(), image.width, image.height);
    let img = ravif::cleared_alpha(img);
    let result = ravif::encode_rgba(img.as_ref(), &config)
        .map_err(|err| format!("Failed to compress image: {}", err))?;
    Ok(result.0)
}

pub fn compress_fast(image: &Image, quality: u8) -> FastCompressResult {
    let result = compress_base(image, quality, true)?;
    Ok(result)
}

pub fn compress(image: &Image, quality: u8) -> CompressResult {
    let result = compress_base(image, quality, false)?;
    Ok((read(&result)?, result))
}
