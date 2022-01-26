use aom_decode::avif::Avif;
use aom_decode::avif::Image;
use aom_decode::Config;
use rayon::prelude::*;
use rgb::ComponentMap;
use std::path::{Path, PathBuf};

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

    match d.convert()? {
        Image::RGB8(img) => {
            let (out, width, height) = img.into_contiguous_buf();
            lodepng::encode24_file(&out_path, &out, width, height)
        },
        Image::RGBA8(img) => {
            let (out, width, height) = img.into_contiguous_buf();
            lodepng::encode32_file(&out_path, &out, width, height)
        },
        Image::Gray8(img) => {
            let (out, width, height) = img.into_contiguous_buf();
            lodepng::encode_file(&out_path, &out, width, height, lodepng::ColorType::GREY, 8)
        },
        // 16-bit PNG are huuuge, so save as 8-bit anyway.
        Image::RGB16(img) => {
            let mut out = Vec::new();
            for px in img.pixels() {
                out.push(px.map(|c| (c >> 8) as u8));
            }
            lodepng::encode24_file(&out_path, &out, img.width(), img.height())
        },
        Image::RGBA16(img) => {
            let mut out = Vec::new();
            for px in img.pixels() {
                out.push(px.map(|c| (c >> 8) as u8));
            }
            lodepng::encode32_file(&out_path, &out, img.width(), img.height())
        },
        Image::Gray16(img) => {
            let mut out = Vec::new();
            for px in img.pixels() {
                out.push((px >> 8) as u8);
            }
            lodepng::encode_file(&out_path, &out, img.width(), img.height(), lodepng::ColorType::GREY, 8)
        },
    }?;
    Ok(out_path)
}

