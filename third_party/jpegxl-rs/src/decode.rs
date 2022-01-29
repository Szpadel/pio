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

//! Decoder of JPEG XL format

use std::{mem::MaybeUninit, ptr::null};

#[allow(clippy::wildcard_imports)]
use jpegxl_sys::*;

use crate::{
    common::{Endianness, PixelType},
    errors::{check_dec_status, DecodeError},
    memory::JxlMemoryManager,
    parallel::JxlParallelRunner,
};

/// Basic Information
pub type BasicInfo = JxlBasicInfo;

/// Result of decoding
pub struct DecoderResult {
    /// Information about returned result
    pub info: ResultInfo,
    /// Enum for various pixel types
    pub data: Data,
}

/// Wrapper type for different pixel types
pub enum Data {
    /// `u8`
    U8(Vec<u8>),
    /// `u16`
    U16(Vec<u16>),
    /// `u32`
    U32(Vec<u32>),
    /// `f32`
    F32(Vec<f32>),
}

/// Extra info of the result
pub struct ResultInfo {
    /// Width of the image
    pub width: u32,
    /// Height of the image
    pub height: u32,
    /// Orientation
    pub orientation: JxlOrientation,
    /// Number of color channels per pixel
    pub num_channels: u32,
    /// ICC color profile
    pub icc_profile: Vec<u8>,
}

/// JPEG XL Decoder
#[derive(Builder)]
#[builder(build_fn(skip))]
#[builder(setter(strip_option))]
pub struct JxlDecoder<'pr, 'mm> {
    /// Opaque pointer to the underlying decoder
    #[builder(setter(skip))]
    dec: *mut jpegxl_sys::JxlDecoder,

    /// Number of channels for returned result
    ///
    /// Default: 0 for automatic detection
    pub num_channels: u32,
    /// Endianness for returned result
    ///
    /// Default: native endian
    pub endianness: Endianness,
    /// Set pixel scanlines alignment for returned result
    ///
    /// Default: 0
    pub align: usize,

    /// Keep orientation or not
    ///
    /// Default: false, so the decoder rotates the image for you
    pub keep_orientation: bool,
    /// Set initial buffer for JPEG reconstruction.
    /// Larger one could be faster with fewer allocations
    ///
    /// Default: 512 KiB
    pub init_jpeg_buffer: usize,

    /// Parallel runner
    pub parallel_runner: Option<&'pr dyn JxlParallelRunner>,

    /// Store memory manager ref so it pins until the end of the decoder
    #[builder(setter(skip))]
    _memory_manager: Option<&'mm dyn JxlMemoryManager>,
}

impl<'pr, 'mm> JxlDecoderBuilder<'pr, 'mm> {
    fn _build(
        &self,
        memory_manager: Option<&'mm dyn JxlMemoryManager>,
    ) -> Result<JxlDecoder<'pr, 'mm>, DecodeError> {
        let dec = unsafe {
            memory_manager.map_or_else(
                || JxlDecoderCreate(null()),
                |mm| JxlDecoderCreate(&mm.manager()),
            )
        };

        if dec.is_null() {
            return Err(DecodeError::CannotCreateDecoder);
        }

        Ok(JxlDecoder {
            dec,
            num_channels: self.num_channels.unwrap_or(0),
            endianness: self.endianness.unwrap_or(Endianness::Native),
            align: self.align.unwrap_or(0),
            keep_orientation: self.keep_orientation.unwrap_or(false),
            init_jpeg_buffer: self.init_jpeg_buffer.unwrap_or(512 * 1024),
            parallel_runner: self.parallel_runner.flatten(),
            _memory_manager: memory_manager,
        })
    }

    /// Build a [`JxlDecoder`]
    ///
    /// # Errors
    /// Return [`DecodeError::CannotCreateDecoder`] if it fails to create the decoder.
    pub fn build(&self) -> Result<JxlDecoder<'pr, 'mm>, DecodeError> {
        Self::_build(self, None)
    }

    /// Build a [`JxlDecoder`] with custom memory manager
    ///
    /// # Errors
    /// Return [`DecodeError::CannotCreateDecoder`] if it fails to create the decoder.
    pub fn build_with(
        &self,
        mm: &'mm dyn JxlMemoryManager,
    ) -> Result<JxlDecoder<'pr, 'mm>, DecodeError> {
        Self::_build(self, Some(mm))
    }
}

impl<'pr, 'mm> JxlDecoder<'pr, 'mm> {
    #[allow(clippy::needless_pass_by_value)]
    fn decode_internal(
        &self,
        pixel_type: Option<JxlDataType>,
        data: &[u8],
        reconstruct_jpeg: bool,
    ) -> Result<(ResultInfo, Data), DecodeError> {
        let mut basic_info = MaybeUninit::uninit();
        let mut pixel_format = MaybeUninit::uninit();

        let mut result = Data::U8(vec![]);
        let mut icc_profile = vec![];
        let mut jpeg_buffer = vec![];

        let mut jpeg_reconstructed = false;

        if reconstruct_jpeg {
            jpeg_buffer.resize(self.init_jpeg_buffer, 0);
        }

        self.setup_decoder(reconstruct_jpeg)?;

        let next_in = data.as_ptr();
        let avail_in = std::mem::size_of_val(data) as _;

        check_dec_status(
            unsafe { JxlDecoderSetInput(self.dec, next_in, avail_in) },
            "Set input",
        )?;

        let mut status;
        loop {
            use JxlDecoderStatus::*;

            status = unsafe { JxlDecoderProcessInput(self.dec) };

            match status {
                Error => return Err(DecodeError::GenericError("Process input")),

                // Get the basic info
                BasicInfo => {
                    self.get_basic_info(
                        pixel_type,
                        basic_info.as_mut_ptr(),
                        pixel_format.as_mut_ptr(),
                    )?;
                }

                // Get color encoding
                ColorEncoding => {
                    icc_profile = self.get_icc_profile(unsafe { &*pixel_format.as_ptr() })?;
                }

                // Get JPEG reconstruction buffer
                JpegReconstruction => {
                    jpeg_reconstructed = true;

                    check_dec_status(
                        unsafe {
                            JxlDecoderSetJPEGBuffer(
                                self.dec,
                                jpeg_buffer.as_mut_ptr(),
                                jpeg_buffer.len(),
                            )
                        },
                        "In JPEG reconstruction event",
                    )?;
                }

                // JPEG buffer need more space
                JpegNeedMoreOutput => {
                    let need_to_write = unsafe { JxlDecoderReleaseJPEGBuffer(self.dec) };

                    let old_len = jpeg_buffer.len();
                    jpeg_buffer.resize(old_len + need_to_write, 0);
                    check_dec_status(
                        unsafe {
                            JxlDecoderSetJPEGBuffer(
                                self.dec,
                                jpeg_buffer.as_mut_ptr(),
                                jpeg_buffer.len(),
                            )
                        },
                        "In JPEG need more output event, set without releasing",
                    )?;
                }

                // Get the output buffer
                NeedImageOutBuffer => {
                    result = self.output(unsafe { &*pixel_format.as_ptr() })?;
                }

                FullImage => continue,
                Success => {
                    if reconstruct_jpeg {
                        if !jpeg_reconstructed {
                            return Err(DecodeError::CannotReconstruct);
                        }

                        let remaining = unsafe { JxlDecoderReleaseJPEGBuffer(self.dec) };

                        jpeg_buffer.truncate(jpeg_buffer.len() - remaining);
                        jpeg_buffer.shrink_to_fit();
                    }

                    unsafe { JxlDecoderReset(self.dec) };

                    let info = unsafe { basic_info.assume_init() };
                    return Ok((
                        ResultInfo {
                            width: info.xsize,
                            height: info.ysize,
                            orientation: info.orientation,
                            num_channels: unsafe { pixel_format.assume_init().num_channels },
                            icc_profile,
                        },
                        if reconstruct_jpeg {
                            Data::U8(jpeg_buffer)
                        } else {
                            result
                        },
                    ));
                }
                _ => return Err(DecodeError::UnknownStatus(status)),
            }
        }
    }

    fn setup_decoder(&self, reconstruct_jpeg: bool) -> Result<(), DecodeError> {
        if let Some(runner) = self.parallel_runner {
            check_dec_status(
                unsafe {
                    JxlDecoderSetParallelRunner(self.dec, runner.runner(), runner.as_opaque_ptr())
                },
                "Set parallel runner",
            )?;
        }

        let events = {
            use JxlDecoderStatus::*;

            let mut events = jxl_dec_events!(BasicInfo, ColorEncoding, FullImage);

            if reconstruct_jpeg {
                events |= JpegReconstruction as i32;
            }

            events
        };
        check_dec_status(
            unsafe { JxlDecoderSubscribeEvents(self.dec, events) },
            "Subscribe events",
        )?;

        check_dec_status(
            unsafe { JxlDecoderSetKeepOrientation(self.dec, self.keep_orientation) },
            "Set if keep orientation",
        )?;

        Ok(())
    }

    fn get_basic_info(
        &self,
        pixel_type: Option<JxlDataType>,
        basic_info: *mut JxlBasicInfo,
        pixel_format: *mut JxlPixelFormat,
    ) -> Result<(), DecodeError> {
        unsafe {
            check_dec_status(
                JxlDecoderGetBasicInfo(self.dec, basic_info),
                "Get basic info",
            )?;
        }

        let basic_info = unsafe { &*basic_info };
        let num_channels = if self.num_channels == 0 {
            basic_info.num_color_channels + (basic_info.alpha_bits != 0) as u32
        } else {
            self.num_channels
        };
        let data_type = pixel_type.unwrap_or_else(|| match basic_info.bits_per_sample {
            8 => JxlDataType::Uint8,
            16 => JxlDataType::Uint16,
            32 => {
                if basic_info.exponent_bits_per_sample == 0 {
                    JxlDataType::Uint32
                } else {
                    JxlDataType::Float
                }
            }
            _ => unreachable!(),
        });

        unsafe {
            *pixel_format = JxlPixelFormat {
                num_channels,
                data_type,
                endianness: self.endianness,
                align: self.align,
            };
        }

        Ok(())
    }

    fn get_icc_profile(&self, format: &JxlPixelFormat) -> Result<Vec<u8>, DecodeError> {
        let mut icc_size = 0;

        check_dec_status(
            unsafe {
                JxlDecoderGetICCProfileSize(
                    self.dec,
                    format,
                    JxlColorProfileTarget::Data,
                    &mut icc_size,
                )
            },
            "Get ICC profile size",
        )?;

        let mut icc_profile = vec![0; icc_size];

        check_dec_status(
            unsafe {
                JxlDecoderGetColorAsICCProfile(
                    self.dec,
                    format,
                    JxlColorProfileTarget::Data,
                    icc_profile.as_mut_ptr(),
                    icc_size,
                )
            },
            "Get ICC profile",
        )?;

        icc_profile.shrink_to_fit();

        Ok(icc_profile)
    }

    fn output(&self, pixel_format: &JxlPixelFormat) -> Result<Data, DecodeError> {
        unsafe fn buf<T: PixelType>(
            dec: *mut jpegxl_sys::JxlDecoder,
            f: &JxlPixelFormat,
            size: usize,
        ) -> Result<Vec<T>, DecodeError> {
            let mut buffer = vec![T::default(); size / std::mem::size_of::<T>()];
            check_dec_status(
                JxlDecoderSetImageOutBuffer(dec, f, buffer.as_mut_ptr().cast(), size),
                "Set output buffer",
            )?;

            buffer.shrink_to_fit();

            Ok(buffer)
        }

        let mut size = 0;
        check_dec_status(
            unsafe { JxlDecoderImageOutBufferSize(self.dec, pixel_format, &mut size) },
            "Get output buffer size",
        )?;

        Ok(unsafe {
            match pixel_format.data_type {
                JxlDataType::Float => Data::F32(buf(self.dec, pixel_format, size)?),
                JxlDataType::Uint8 => Data::U8(buf(self.dec, pixel_format, size)?),
                JxlDataType::Uint16 => Data::U16(buf(self.dec, pixel_format, size)?),
                _ => unimplemented!(), // TODO: Add other types
            }
        })
    }

    /// Decode a JPEG XL image
    ///
    /// Currently only support RGB(A)8/16/32 encoded static image. Other info are discarded.
    /// # Errors
    /// Return a [`DecodeError`] when internal decoder fails
    pub fn decode(&self, data: &[u8]) -> Result<DecoderResult, DecodeError> {
        let (info, data) = self.decode_internal(None, data, false)?;
        Ok(DecoderResult { info, data })
    }

    /// Decode a JPEG XL image to a given pixel type
    ///
    /// Currently only support RGB(A)8/16/32 encoded static image. Other info are discarded.
    /// # Errors
    /// Return a [`DecodeError`] when internal decoder fails
    pub fn decode_to<T: PixelType>(&self, data: &[u8]) -> Result<DecoderResult, DecodeError> {
        let (info, data) = self.decode_internal(Some(T::pixel_type()), data, false)?;
        Ok(DecoderResult { info, data })
    }

    /// Decode a JPEG XL image and reconstruct JPEG data
    ///
    /// Currently only support RGB(A)8/16/32 encoded static image. Other info are discarded.
    /// # Errors
    /// Return a [`DecodeError`] when internal decoder fails
    pub fn decode_jpeg(&self, data: &[u8]) -> Result<(ResultInfo, Vec<u8>), DecodeError> {
        if let (info, Data::U8(data)) = self.decode_internal(None, data, true)? {
            Ok((info, data))
        } else {
            Err(DecodeError::CannotReconstruct)
        }
    }
}

impl<'prl, 'mm> Drop for JxlDecoder<'prl, 'mm> {
    fn drop(&mut self) {
        unsafe { JxlDecoderDestroy(self.dec) };
    }
}

/// Return a [`JxlDecoderBuilder`] with default settings
#[must_use]
pub fn decoder_builder<'prl, 'mm>() -> JxlDecoderBuilder<'prl, 'mm> {
    JxlDecoderBuilder::default()
}
