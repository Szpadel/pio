use aom_decode::Decoder;
use aom_decode::Config;
use std::path::Path;

fn main() {
    for path in std::env::args_os().skip(1) {
        let path = Path::new(&path);
        print!("{}: ", path.display());
        match do_file(path) {
            Ok(()) => println!("ok"),
            Err(e) => println!("{}", e),
        };
    }
}

fn do_file(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::read(path)?;
    let avif = avif_parse::read_avif(&mut &file[..])?;
    let mut d = Decoder::new(&Config {
        threads: num_cpus::get(),
    })?;

    let img1 = d.decode_frame(&avif.primary_item)?;
    eprintln!("{:#?}", img1);
    Ok(())
}
