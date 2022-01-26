use core::num::NonZeroU32;
use quick_error::*;

quick_error! {
    #[derive(Debug)]
    #[non_exhaustive]
    pub enum Error {
        AOM(code: NonZeroU32, msg: Option<String>) {
            display("{} ({})", msg.as_deref().unwrap_or("libaom error"), code)
        }
        #[cfg(feature = "avif")]
        AVIF(err: avif_parse::Error) {
            display("{}", err)
            from()
        }
        #[cfg(feature = "avif")]
        YUV(err: yuv::Error) {
            display("{}", err)
            from()
        }
        Unsupported(msg: &'static str) {
            display("{}", msg)
        }
    }
}
