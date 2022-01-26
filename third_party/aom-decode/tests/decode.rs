use aom_decode::*;

#[test]
fn decode_test() {
    let file = include_bytes!("test.avif");
    let avif = avif_parse::read_avif(&mut &file[..]).unwrap();
    let mut d = Decoder::new(&Config {
        threads: 1,
    }).unwrap();
    let _ = d.decode_frame(&avif.primary_item).unwrap();
    let _ = d.decode_frame(avif.alpha_item.as_deref().unwrap()).unwrap();
}
