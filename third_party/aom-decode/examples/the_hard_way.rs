use aom_decode::avif::Avif;
use aom_decode::chroma::*;
use aom_decode::color;
use aom_decode::Config;
use aom_decode::RowsIters;
use rayon::prelude::*;
use rgb::ComponentMap;
use std::path::{Path, PathBuf};
use yuv::YUV;

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    args.into_par_iter().for_each(|path| {
        let path = Path::new(&path);
        match do_file(path) {
            Ok(path) => println!("{}", path.display()),
            Err(e) => eprintln!("{}: {}", path.display(), e),
        };
    });
}

fn do_file(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let file = std::fs::read(path)?;
    let out_path = Path::new(path.file_name().unwrap()).with_extension("example.png");
    let mut d = Avif::decode(&file, &Config {
        threads: num_cpus::get(),
    })?;

    let img = d.raw_color_data()?;
    let range = img.range();
    match img.rows_iter()? {
        RowsIters::YuvPlanes8 {y,u,v,chroma_sampling} => {
            let mc = img.matrix_coefficients().unwrap_or(color::MatrixCoefficients::BT709);
            let conv = yuv::convert::RGBConvert::<u8>::new(range, mc)?;
            let width = y.width();
            let height = y.height();
            let mut out = Vec::with_capacity(width * height);
            let mut tmp1;
            let mut tmp2;
            let mut tmp3;
            let px_iter: &mut dyn Iterator<Item=YUV<u8>> = match chroma_sampling {
                color::ChromaSampling::Cs444 => {
                    tmp1 = yuv_444(y, u, v);
                    &mut tmp1
                },
                color::ChromaSampling::Cs420 => {
                    tmp2 = yuv_420(y, u, v);
                    &mut tmp2
                },
                color::ChromaSampling::Cs422 => {
                    tmp3 = yuv_422(y, u, v);
                    &mut tmp3
                },
                color::ChromaSampling::Monochrome => unreachable!(),
            };
            out.extend(px_iter.map(|px| conv.to_rgb(px)));
            lodepng::encode24_file(&out_path, &out, width, height)?;
        },
        RowsIters::Mono8(y) => {
            let mc = img.matrix_coefficients().unwrap_or(color::MatrixCoefficients::Identity);
            let conv = yuv::convert::RGBConvert::<u8>::new(range, mc)?;

            let width = y.width();
            let height = y.height();
            let mut out = Vec::with_capacity(width * height);
            out.extend(y.flat_map(|row| {
                row.iter().copied().map(|y| {
                    conv.to_luma(y)
                })
            }));
            lodepng::encode_file(&out_path, &out, width, height, lodepng::ColorType::GREY, 8)?;
        },
        RowsIters::Mono16(y, depth) => {
            let mc = img.matrix_coefficients().unwrap_or(color::MatrixCoefficients::Identity);
            let conv = yuv::convert::RGBConvert::<u16>::new(range, mc, depth)?;
            let width = y.width();
            let height = y.height();
            let mut out = Vec::with_capacity(width * height);
            out.extend(y.flat_map(|row| {
                row.iter().copied().map(|y| {
                    let y = u16::from_ne_bytes(y);
                    (conv.to_luma(y)>>8) as u8
                })
            }));
            lodepng::encode_file(&out_path, &out, width, height, lodepng::ColorType::GREY, 8)?;
        },
        RowsIters::YuvPlanes16 {y,u,v,chroma_sampling, depth} => {
            let mc = img.matrix_coefficients().unwrap_or(color::MatrixCoefficients::BT709);
            let conv = yuv::convert::RGBConvert::<u16>::new(range, mc, depth)?;
            let width = y.width();
            let height = y.height();
            let mut out = Vec::with_capacity(width * height);
            let mut tmp1;
            let mut tmp2;
            let mut tmp3;
            let px_iter: &mut dyn Iterator<Item=YUV<[u8; 2]>> = match chroma_sampling {
                color::ChromaSampling::Cs444 => {
                    tmp1 = yuv_444(y, u, v);
                    &mut tmp1
                },
                color::ChromaSampling::Cs420 => {
                    tmp2 = yuv_420(y, u, v);
                    &mut tmp2
                },
                color::ChromaSampling::Cs422 => {
                    tmp3 = yuv_422(y, u, v);
                    &mut tmp3
                },
                color::ChromaSampling::Monochrome => unreachable!(),
            };
            out.extend(px_iter.map(|px| conv.to_rgb(YUV{
                y: u16::from_ne_bytes(px.y),
                u: u16::from_ne_bytes(px.u),
                v: u16::from_ne_bytes(px.v),
            }).map(|c| (c>>8) as u8)));
            lodepng::encode24_file(&out_path, &out, width, height)?;
        },
    };
    Ok(out_path)
}
