pub type Result<T, E = Error> = std::result::Result<T, E>;
pub use yuv::color;

mod error;
pub use error::Error;

mod aom;
pub use aom::Config;
pub use aom::Decoder;
pub use aom::FrameTempRef;
pub use aom::RowsIters;
pub use aom::RowsIter;

/// Helper functions for undoing chroma subsampling
pub mod chroma;

#[cfg(feature = "avif")]
pub mod avif;
