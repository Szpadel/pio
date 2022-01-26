# Rust wrapper for AOMedia AV1 decoder

It's a minimal safe wrapper that allows decoding individual AV1 frames. It's meant for decoding AVIF images.


## Usage

See [`examples/topng.rs`](examples/topng.rs) for the full code.

You'll need the [avif-parse](//lib.rs/avif-parse) crate to get AV1 data out of an AVIF file, and the [yuv](//lib.rs/yuv) crate to convert YUV pixels into RGB.

```rust
let avif = avif_parse::read_avif(file)?;

let mut d = Decoder::new(&Config {
    threads: num_cpus::get(),
})?;

let img = d.decode_frame(&avif.primary_item)?;
match img.rows_iter()? {
    RowsIters::YuvPlanes8 {y,u,v,chroma_sampling} => {
        match chroma_sampling {
            color::ChromaSampling::Cs444 => {
                yuv_444(y, u, v).map(|px| {
                    // here's your YUV pixel
                });
            },
        }
    },
}
```
