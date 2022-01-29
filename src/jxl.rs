use jpegxl_rs::{
    decoder_builder,
    encode::{EncoderResult, EncoderSpeed, EncoderFrame},
    encoder_builder,
};
use rgb::RGBA;

use crate::common::{exif_orientation, orient_image, CompressResult, Image, ReadResult};

pub fn read(buffer: &[u8]) -> ReadResult {
    let decoder = decoder_builder().num_channels(4).build().unwrap();
    let image = decoder.decode_to::<u8>(&buffer).unwrap();
    // TODO: ICC profile
    // TODO: orientation
    // TODO: variable channels
    let data = match image.data {
        jpegxl_rs::decode::Data::U8(data) => data
            .chunks(4)
            .map(|p| RGBA::new(p[0], p[1], p[2], p[3]))
            .collect(),
        jpegxl_rs::decode::Data::U16(_) => todo!(),
        jpegxl_rs::decode::Data::U32(_) => todo!(),
        jpegxl_rs::decode::Data::F32(_) => todo!(),
    };
    Ok(Image::from_rgba(
        data,
        image.info.width as usize,
        image.info.height as usize,
    ))
}

pub fn compress(image: &Image, quality: u8, lossless: bool) -> CompressResult {
    let mut encoder = encoder_builder()
        .speed(EncoderSpeed::Tortoise)
        .has_alpha(true)
        .build()
        .unwrap();
    if lossless {
        encoder.lossless = true;
    } else {
        encoder.quality = 15.0 - (quality as f32 * 0.15);
        if encoder.quality < 0.1 {
            encoder.quality = 0.1;
        }
    }

    let frame = EncoderFrame::new(&image.as_bytes()).num_channels(4);

    let buffer: EncoderResult<u8> = encoder
        .encode_frame(&frame, image.width as u32, image.height as u32)
        .unwrap();

    Ok((read(&buffer.data).unwrap(), buffer.data))
}
