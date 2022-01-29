/*
This file is part of jpegxl-rs.

jpegxl-rs is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

jpegxl-rs is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with jpegxl-rs.  If not, see <https://www.gnu.org/licenses/>.
*/

//! Decoder and encoder errors

#[allow(clippy::wildcard_imports)]
use jpegxl_sys::*;
use thiserror::Error;

/// Errors derived from [`JxlDecoderStatus`]
#[derive(Error, Debug)]
pub enum DecodeError {
    /// Unable to read more data
    #[error(transparent)]
    InputError(#[from] std::io::Error),
    /// Cannot create a decoder
    #[error("Cannot create a decoder")]
    CannotCreateDecoder,
    /// Unknown Error
    // TODO: underlying library is working on a way to retrieve error message
    #[error("Decoder error: {0}")]
    GenericError(&'static str),
    /// Invalid file format
    #[error("The file does not contain a valid codestream or container")]
    InvalidFileFormat,
    /// Cannot reconstruct JPEG codestream
    #[error("Cannot reconstruct JPEG codestream from the file")]
    CannotReconstruct,
    /// Unknown status
    #[error("Unknown status: `{0:?}`")]
    UnknownStatus(JxlDecoderStatus),
}

/// Errors derived from [`JxlEncoderStatus`]
#[derive(Error, Debug)]
pub enum EncodeError {
    /// Cannot create an encoder
    #[error("Cannot create an encoder")]
    CannotCreateEncoder,
    /// Unknown Error
    // TODO: underlying library is working on a way to retrieve error message
    #[error("Encoder error: {0}")]
    GenericError(&'static str),
    /// Not Supported
    #[error("Encoder does not support it (yet)")]
    NotSupported,
    /// Unknown status
    #[error("Unknown status: `{0:?}`")]
    UnknownStatus(JxlEncoderStatus),
}

/// Error mapping from underlying C const to [`JxlDecoderStatus` enum
pub(crate) fn check_dec_status(
    status: JxlDecoderStatus,
    msg: &'static str,
) -> Result<(), DecodeError> {
    match status {
        JxlDecoderStatus::Success => Ok(()),
        JxlDecoderStatus::Error => Err(DecodeError::GenericError(msg)),
        _ => Err(DecodeError::UnknownStatus(status)),
    }
}

/// Error mapping from underlying C const to [`JxlEncoderStatus`] enum
pub(crate) fn check_enc_status(
    status: JxlEncoderStatus,
    msg: &'static str,
) -> Result<(), EncodeError> {
    match status {
        JxlEncoderStatus::Success => Ok(()),
        JxlEncoderStatus::Error => Err(EncodeError::GenericError(msg)),
        JxlEncoderStatus::NotSupported => Err(EncodeError::NotSupported),
        JxlEncoderStatus::NeedMoreOutput => Err(EncodeError::UnknownStatus(status)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_invalid_data() {
        let sample = Vec::new();

        let decoder = crate::decoder_builder()
            .build()
            .expect("Failed to create decoder");
        assert!(matches!(
            decoder.decode_to::<u8>(&sample),
            Err(DecodeError::UnknownStatus(JxlDecoderStatus::NeedMoreInput))
        ));
    }

    #[test]
    fn encode_invalid_data() {
        let mut encoder = crate::encoder_builder()
            .has_alpha(true)
            .build()
            .expect("Failed to create encoder");

        assert!(matches!(
            encoder.encode::<u8, u8>(&[], 0, 0),
            Err(EncodeError::GenericError("Set basic info"))
        ));

        assert!(matches!(
            encoder.encode::<f32, f32>(&[1.0, 1.0, 1.0, 0.5], 1, 1),
            Err(EncodeError::NotSupported)
        ));
    }

    #[test]
    fn mapping() {
        assert!(matches!(
            check_dec_status(JxlDecoderStatus::Error, "Testing"),
            Err(DecodeError::GenericError("Testing"))
        ));

        assert!(matches!(
            check_dec_status(JxlDecoderStatus::NeedMoreInput, "Testing"),
            Err(DecodeError::UnknownStatus(JxlDecoderStatus::NeedMoreInput))
        ));

        assert!(matches!(
            check_enc_status(JxlEncoderStatus::NeedMoreOutput, "Testing"),
            Err(EncodeError::UnknownStatus(JxlEncoderStatus::NeedMoreOutput))
        ));
    }
}
