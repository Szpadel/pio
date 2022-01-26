use avif_parse::AvifData;
use crate::chroma::yuv_420;
use crate::chroma::yuv_422;
use crate::chroma::yuv_444;
use crate::color::{MatrixCoefficients, ChromaSampling};
use crate::Config;
use crate::Decoder;
use crate::Error;
use crate::FrameTempRef;
use crate::Result;
use crate::RowsIter;
use crate::RowsIters;
use imgref::ImgVec;
use yuv::convert::RGBConvert;
use yuv::RGB;
use yuv::RGBA;
use yuv::YUV;
use yuv::color::{Range, Depth};

pub struct Avif {
    decoder: Decoder,
    avif: AvifData,
}

pub enum Image {
    RGB8(ImgVec<RGB<u8>>),
    RGBA8(ImgVec<RGBA<u8>>),
    RGB16(ImgVec<RGB<u16>>),
    RGBA16(ImgVec<RGBA<u16>>),
    Gray8(ImgVec<u8>),
    Gray16(ImgVec<u16>),
}

impl Avif {
    pub fn decode(data: &[u8], config: &Config) -> Result<Self> {
        let avif = avif_parse::read_avif(&mut &data[..])?;
        let decoder = Decoder::new(config)?;
        Ok(Self {
            decoder,
            avif,
        })
    }

    pub fn convert(&mut self) -> Result<Image> {
        // aom decoder recycles buffers, so can't have both color and alpha without copying,
        // therefore conversion will put placeholders and then update alpha
        let has_alpha = self.avif.alpha_item.is_some();
        let color = self.raw_color_data()?;
        let range = color.range();
        let mut img = match color.rows_iter()? {
            RowsIters::YuvPlanes8 {y,u,v,chroma_sampling} => {
                yuv_to_rgb8(&color, range, y, chroma_sampling, u, v, has_alpha)?
            },
            RowsIters::Mono8(y) => {
                yuv_to_gray8(&color, range, y, has_alpha)?
            },
            RowsIters::Mono16(y, depth) => {
                yuv_to_gray16(&color, range, depth, y, has_alpha)?
            },
            RowsIters::YuvPlanes16 {y,u,v,chroma_sampling, depth} => {
                yuv_to_rgb16(&color, range, depth, y, chroma_sampling, u, v, has_alpha)?
            },
        };
        drop(color);
        if let Some(alpha) = self.raw_alpha_data()? {
            let range = alpha.range();
            let mc = alpha.matrix_coefficients().unwrap_or(MatrixCoefficients::Identity);
            match alpha.rows_iter()? {
                RowsIters::YuvPlanes8 {y, ..} | RowsIters::Mono8(y) => {
                    let conv = RGBConvert::<u8>::new(range, mc)?;
                    add_alpha8(&mut img, y, conv)?;
                },
                RowsIters::YuvPlanes16 {y, depth, ..} | RowsIters::Mono16(y, depth) => {
                    let conv = RGBConvert::<u16>::new(range, mc, depth)?;
                    add_alpha16(&mut img, y, conv)?;
                },
            };
        } else {
            assert!(!has_alpha);
        }
        Ok(img)
    }

    pub fn raw_color_data(&mut self) -> Result<FrameTempRef<'_>> {
        Ok(self.decoder.decode_frame(&self.avif.primary_item)?)
    }

    pub fn raw_alpha_data(&mut self) -> Result<Option<FrameTempRef<'_>>> {
        Ok(if let Some(alpha) = &self.avif.alpha_item {
            Some(self.decoder.decode_frame(alpha)?)
        } else {
            None
        })
    }
}

fn add_alpha16(img: &mut Image, y: RowsIter<[u8; 2]>, conv: RGBConvert<u16>) -> Result<()> {
    if let RGBConvert::Matrix(_) = conv {
        return Err(Error::Unsupported("alpha image has color info"));
    }
    match img {
        Image::RGBA8(img) => {
            for (y_row, img_row) in y.zip(img.rows_mut()) {
                if y_row.len() != img_row.len() {
                    return Err(Error::Unsupported("invalid alpha size"));
                }
                for (y, px) in y_row.iter().copied().zip(img_row) {
                    let y = u16::from_ne_bytes(y);
                    px.a = (conv.to_luma(y) >> 8) as u8;
                }
            }
        },
        Image::RGBA16(img) => {
            for (y_row, img_row) in y.zip(img.rows_mut()) {
                if y_row.len() != img_row.len() {
                    return Err(Error::Unsupported("invalid alpha size"));
                }
                for (y, px) in y_row.iter().copied().zip(img_row) {
                    let y = u16::from_ne_bytes(y);
                    px.a = conv.to_luma(y);
                }
            }
        },
        _ => return Err(Error::Unsupported("internal error")),
    };
    Ok(())
}

fn add_alpha8(img: &mut Image, y: RowsIter<u8>, conv: RGBConvert) -> Result<()> {
    if let RGBConvert::Matrix(_) = conv {
        return Err(Error::Unsupported("alpha image has color info"));
    }
    match img {
        Image::RGBA8(img) => {
            for (y_row, img_row) in y.zip(img.rows_mut()) {
                if y_row.len() != img_row.len() {
                    return Err(Error::Unsupported("invalid alpha size"));
                }
                for (y, px) in y_row.iter().copied().zip(img_row) {
                    px.a = conv.to_luma(y);
                }
            }
        },
        Image::RGBA16(img) => {
            for (y_row, img_row) in y.zip(img.rows_mut()) {
                if y_row.len() != img_row.len() {
                    return Err(Error::Unsupported("invalid alpha size"));
                }
                for (y, px) in y_row.iter().copied().zip(img_row) {
                    let g = conv.to_luma(y) as u16;
                    px.a = (g << 8) | g;
                }
            }
        },
        _ => return Err(Error::Unsupported("internal error")),
    };
    Ok(())
}

fn yuv_to_rgb16(color: &FrameTempRef, range: Range, depth: Depth, y: RowsIter<[u8; 2]>, chroma_sampling: ChromaSampling, u: RowsIter<[u8; 2]>, v: RowsIter<[u8; 2]>, has_alpha: bool) -> Result<Image, Error> {
    let mc = color.matrix_coefficients().unwrap_or(MatrixCoefficients::BT709);
    let conv = RGBConvert::<u16>::new(range, mc, depth)?;
    let width = y.width();
    let height = y.height();
    let mut tmp1;
    let mut tmp2;
    let mut tmp3;
    let px_iter: &mut dyn Iterator<Item=YUV<[u8; 2]>> = match chroma_sampling {
        ChromaSampling::Cs444 => {
            tmp1 = yuv_444(y, u, v);
            &mut tmp1
        },
        ChromaSampling::Cs420 => {
            tmp2 = yuv_420(y, u, v);
            &mut tmp2
        },
        ChromaSampling::Cs422 => {
            tmp3 = yuv_422(y, u, v);
            &mut tmp3
        },
        ChromaSampling::Monochrome => unreachable!(),
    };
    if has_alpha {
        let mut out = Vec::with_capacity(width * height);
        out.extend(px_iter.map(|px| conv.to_rgb(YUV{
            y: u16::from_ne_bytes(px.y),
            u: u16::from_ne_bytes(px.u),
            v: u16::from_ne_bytes(px.v),
        }).alpha(0)));
        Ok(Image::RGBA16(ImgVec::new(out, width, height)))
    } else {
        let mut out = Vec::with_capacity(width * height);
        out.extend(px_iter.map(|px| conv.to_rgb(YUV{
            y: u16::from_ne_bytes(px.y),
            u: u16::from_ne_bytes(px.u),
            v: u16::from_ne_bytes(px.v),
        })));
        Ok(Image::RGB16(ImgVec::new(out, width, height)))
    }
}

fn yuv_to_gray16(color: &FrameTempRef, range: Range, depth: Depth, y: RowsIter<[u8; 2]>, has_alpha: bool) -> Result<Image, Error> {
    let mc = color.matrix_coefficients().unwrap_or(MatrixCoefficients::Identity);
    let conv = RGBConvert::<u16>::new(range, mc, depth)?;
    let width = y.width();
    let height = y.height();
    if has_alpha {
        let mut out = Vec::with_capacity(width * height);
        out.extend(y.flat_map(|row| {
            row.iter().copied().map(|y| {
                let g = conv.to_luma(u16::from_ne_bytes(y));
                RGBA::new(g, g, g, 0)
            })
        }));
        Ok(Image::RGBA16(ImgVec::new(out, width, height)))
    } else {
        let mut out = Vec::with_capacity(width * height);
        out.extend(y.flat_map(|row| {
            row.iter().copied().map(|y| {
                conv.to_luma(u16::from_ne_bytes(y))
            })
        }));
        Ok(Image::Gray16(ImgVec::new(out, width, height)))
    }
}

fn yuv_to_gray8(color: &FrameTempRef, range: Range, y: RowsIter<u8>, has_alpha: bool) -> Result<Image, Error> {
    let mc = color.matrix_coefficients().unwrap_or(MatrixCoefficients::Identity);
    let conv = RGBConvert::<u8>::new(range, mc)?;
    let width = y.width();
    let height = y.height();
    if has_alpha {
        let mut out = Vec::with_capacity(width * height);
        out.extend(y.flat_map(|row| {
            row.iter().copied().map(|y| {
                conv.to_rgb(YUV{y,u:128,v:128}).alpha(0)
            })
        }));
        Ok(Image::RGBA8(ImgVec::new(out, width, height)))
    } else {
        let mut out = Vec::with_capacity(width * height);
        out.extend(y.flat_map(|row| {
            row.iter().copied().map(|y| {
                conv.to_rgb(YUV{y,u:128,v:128}).g
            })
        }));
        Ok(Image::Gray8(ImgVec::new(out, width, height)))
    }
}

fn yuv_to_rgb8(color: &FrameTempRef, range: Range, y: RowsIter<u8>, chroma_sampling: ChromaSampling, u: RowsIter<u8>, v: RowsIter<u8>, has_alpha: bool) -> Result<Image, Error> {
    let mc = color.matrix_coefficients().unwrap_or(MatrixCoefficients::BT709);
    let conv = RGBConvert::<u8>::new(range, mc)?;
    let width = y.width();
    let height = y.height();
    let mut tmp1;
    let mut tmp2;
    let mut tmp3;
    let px_iter: &mut dyn Iterator<Item=YUV<u8>> = match chroma_sampling {
        ChromaSampling::Cs444 => {
            tmp1 = yuv_444(y, u, v);
            &mut tmp1
        },
        ChromaSampling::Cs420 => {
            tmp2 = yuv_420(y, u, v);
            &mut tmp2
        },
        ChromaSampling::Cs422 => {
            tmp3 = yuv_422(y, u, v);
            &mut tmp3
        },
        ChromaSampling::Monochrome => unreachable!(),
    };
    if has_alpha {
        let mut out = Vec::with_capacity(width * height);
        out.extend(px_iter.map(|px| conv.to_rgb(px).alpha(0)));
        Ok(Image::RGBA8(ImgVec::new(out, width, height)))
    } else {
        let mut out = Vec::with_capacity(width * height);
        out.extend(px_iter.map(|px| conv.to_rgb(px)));
        Ok(Image::RGB8(ImgVec::new(out, width, height)))
    }
}

