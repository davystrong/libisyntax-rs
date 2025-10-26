use image::{RgbImage, RgbaImage};
use std::{
    ffi::{CStr, CString, c_uint},
    os::unix::ffi::OsStrExt,
    path::Path,
    slice, sync,
};
use thiserror::Error;

mod libisyntax {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]

    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

#[derive(Error, Debug)]
pub enum ISyntaxError {
    #[error("unknown fatal error")]
    Fatal,

    #[error("invalid arguments")]
    InvalidArgument,

    #[error("unexpected null pointer")]
    NullPointer,

    #[error("invalid string")]
    StringError,

    #[error("invalid image data")]
    ImageDecodeError,

    #[error("index out of range")]
    IndexOutOfRange,
}

pub type Result<T> = std::result::Result<T, ISyntaxError>;

impl From<libisyntax::isyntax_error_t> for Result<()> {
    fn from(value: libisyntax::isyntax_error_t) -> Self {
        match value.0 {
            0 => Ok(()),
            2 => Err(ISyntaxError::InvalidArgument),
            _ => Err(ISyntaxError::Fatal),
        }
    }
}

static INIT: sync::Once = sync::Once::new();

pub struct ISyntax {
    isyntax: *mut libisyntax::isyntax_t,
    wsi_image: *const libisyntax::isyntax_image_t,
    cache: *mut libisyntax::isyntax_cache_t,
    levels: Vec<ISyntaxLevel>,
}

impl ISyntax {
    pub fn open<P: AsRef<Path>>(
        path: P,
        // init_allocators: bool, // Not supported
        // read_barcode_only: bool, // TODO: Make this standalone
    ) -> Result<Self> {
        INIT.call_once(|| unsafe {
            let result: Result<()> = libisyntax::libisyntax_init().into();
            result.expect("failed to inistialise libisyntax");
        });

        let init_allocators = false;
        let read_barcode_only = false;
        let flags = libisyntax::libisyntax_open_flags_t_LIBISYNTAX_OPEN_FLAG_INIT_ALLOCATORS
            * (init_allocators as c_uint)
            + libisyntax::libisyntax_open_flags_t_LIBISYNTAX_OPEN_FLAG_READ_BARCODE_ONLY
                * (read_barcode_only as c_uint);

        let mut isyntax: *mut libisyntax::isyntax_t = std::ptr::null_mut();

        unsafe {
            let path = CString::new(path.as_ref().as_os_str().as_bytes())
                .map_err(|_| ISyntaxError::StringError)?;
            let result: Result<()> =
                libisyntax::libisyntax_open(path.as_ptr(), flags, &raw mut isyntax).into();
            result?;
        };

        if isyntax.is_null() {
            return Err(ISyntaxError::NullPointer);
        }

        let wsi_image = unsafe { libisyntax::libisyntax_get_wsi_image(isyntax) };

        if wsi_image.is_null() {
            return Err(ISyntaxError::NullPointer);
        }

        let level_count = unsafe { libisyntax::libisyntax_image_get_level_count(wsi_image) };

        // TODO: Make this lazy?
        let mut cache: *mut libisyntax::isyntax_cache_t = std::ptr::null_mut();

        unsafe {
            //TODO: Make cache_size configurable
            let result: Result<_> =
                libisyntax::libisyntax_cache_create(std::ptr::null(), 2000, &raw mut cache).into();
            result?;

            if cache.is_null() {
                return Err(ISyntaxError::NullPointer);
            }

            let result: Result<_> = libisyntax::libisyntax_cache_inject(cache, isyntax).into();
            result?;
        }

        let tile_width = unsafe { libisyntax::libisyntax_get_tile_width(isyntax) };
        let tile_height = unsafe { libisyntax::libisyntax_get_tile_height(isyntax) };

        let levels: Result<Vec<_>> = (0..level_count)
            .map(|i| unsafe {
                let level = libisyntax::libisyntax_image_get_level(wsi_image, i);

                if level.is_null() {
                    return Err(ISyntaxError::NullPointer);
                }

                Ok(ISyntaxLevel {
                    isyntax,
                    level,
                    cache,
                    index: i,
                    tile_width,
                    tile_height,
                })
            })
            .collect();

        Ok(Self {
            isyntax,
            wsi_image,
            cache,
            levels: levels?,
        })
    }

    pub fn tile_width(&self) -> i32 {
        unsafe { libisyntax::libisyntax_get_tile_width(self.isyntax) }
    }

    pub fn tile_height(&self) -> i32 {
        unsafe { libisyntax::libisyntax_get_tile_height(self.isyntax) }
    }

    pub fn read_label_image(&self) -> Result<RgbImage> {
        let jpeg_slice = unsafe {
            // libisyntax::libisyntax_get_label_image(self.inner);
            let mut jpeg_buffer: *mut u8 = std::ptr::null_mut();
            let mut jpeg_size: u32 = 0;
            let result: Result<()> = libisyntax::libisyntax_read_label_image_jpeg(
                self.isyntax,
                &raw mut jpeg_buffer,
                &raw mut jpeg_size,
            )
            .into();
            result?;

            if jpeg_buffer.is_null() || jpeg_size == 0 {
                return Err(ISyntaxError::NullPointer);
            }

            slice::from_raw_parts(jpeg_buffer, jpeg_size as usize)
        };

        let img = image::load_from_memory_with_format(jpeg_slice, image::ImageFormat::Jpeg)
            .map_err(|_| ISyntaxError::ImageDecodeError)?;

        Ok(img.into_rgb8())
    }

    pub fn read_macro_image(&self) -> Result<RgbImage> {
        let jpeg_slice = unsafe {
            let mut jpeg_buffer: *mut u8 = std::ptr::null_mut();
            let mut jpeg_size: u32 = 0;
            let result: Result<()> = libisyntax::libisyntax_read_macro_image_jpeg(
                self.isyntax,
                &raw mut jpeg_buffer,
                &raw mut jpeg_size,
            )
            .into();
            result?;

            if jpeg_buffer.is_null() || jpeg_size == 0 {
                return Err(ISyntaxError::NullPointer);
            }

            slice::from_raw_parts(jpeg_buffer, jpeg_size as usize)
        };

        let img = image::load_from_memory_with_format(jpeg_slice, image::ImageFormat::Jpeg)
            .map_err(|_| ISyntaxError::ImageDecodeError)?;

        Ok(img.into_rgb8())
    }

    pub fn barcode(&self) -> Result<&str> {
        let barcode = unsafe {
            let ptr = libisyntax::libisyntax_get_barcode(self.isyntax);
            if ptr.is_null() {
                return Err(ISyntaxError::NullPointer);
            }
            CStr::from_ptr(ptr)
        };
        barcode.to_str().map_err(|_| ISyntaxError::StringError)
    }

    pub fn level_count(&self) -> i32 {
        self.levels.len() as i32
    }

    pub fn offset_x(&self) -> i32 {
        unsafe { libisyntax::libisyntax_image_get_offset_x(self.wsi_image) }
    }

    pub fn offset_y(&self) -> i32 {
        unsafe { libisyntax::libisyntax_image_get_offset_y(self.wsi_image) }
    }

    pub fn level(&self, index: i32) -> Result<&ISyntaxLevel> {
        self.levels
            .get(index as usize)
            .ok_or(ISyntaxError::IndexOutOfRange)
    }
}

impl Drop for ISyntax {
    fn drop(&mut self) {
        unsafe { libisyntax::libisyntax_cache_destroy(self.cache) };
        unsafe { libisyntax::libisyntax_close(self.isyntax) };
    }
}

// This should only ever be given out as a reference since it contains a pointer to the isyntax_t
pub struct ISyntaxLevel {
    isyntax: *mut libisyntax::isyntax_t,
    level: *const libisyntax::isyntax_level_t,
    cache: *mut libisyntax::isyntax_cache_t,
    index: i32,
    tile_width: i32,
    tile_height: i32,
}

impl ISyntaxLevel {
    pub fn scale(&self) -> i32 {
        unsafe { libisyntax::libisyntax_level_get_scale(self.level) }
    }

    pub fn width_in_tiles(&self) -> i32 {
        unsafe { libisyntax::libisyntax_level_get_width_in_tiles(self.level) }
    }

    pub fn height_in_tiles(&self) -> i32 {
        unsafe { libisyntax::libisyntax_level_get_height_in_tiles(self.level) }
    }

    pub fn width(&self) -> i32 {
        unsafe { libisyntax::libisyntax_level_get_width(self.level) }
    }

    pub fn height(&self) -> i32 {
        unsafe { libisyntax::libisyntax_level_get_height(self.level) }
    }

    pub fn mpp_x(&self) -> f32 {
        unsafe { libisyntax::libisyntax_level_get_mpp_x(self.level) }
    }

    pub fn mpp_y(&self) -> f32 {
        unsafe { libisyntax::libisyntax_level_get_mpp_y(self.level) }
    }

    pub fn read_tile(&self, tile_x: i64, tile_y: i64) -> Result<RgbaImage> {
        let mut buffer: Vec<u8> = vec![0; (self.tile_width * self.tile_height * 4) as usize];

        self.read_tile_buf(tile_x, tile_y, &mut buffer)?;
        RgbaImage::from_vec(self.tile_width as u32, self.tile_height as u32, buffer)
            .ok_or(ISyntaxError::ImageDecodeError)
    }

    pub fn read_tile_buf(&self, tile_x: i64, tile_y: i64, buf: &mut Vec<u8>) -> Result<()> {
        unsafe {
            let result: Result<_> = libisyntax::libisyntax_tile_read(
                self.isyntax,
                self.cache,
                self.index,
                tile_x,
                tile_y,
                buf.as_mut_ptr() as *mut u32,
                libisyntax::isyntax_pixel_format_t_LIBISYNTAX_PIXEL_FORMAT_RGBA as i32,
            )
            .into();
            result?;
        }
        Ok(())
    }

    pub fn read_region(
        &self,
        tile_x: i64,
        tile_y: i64,
        width: i64,
        height: i64,
    ) -> Result<RgbaImage> {
        let mut buffer: Vec<u8> = vec![0; (width * height * 4) as usize];

        self.read_region_buf(tile_x, tile_y, width, height, &mut buffer)?;
        RgbaImage::from_vec(width as u32, height as u32, buffer)
            .ok_or(ISyntaxError::ImageDecodeError)
    }

    pub fn read_region_buf(
        &self,
        tile_x: i64,
        tile_y: i64,
        width: i64,
        height: i64,
        buf: &mut Vec<u8>,
    ) -> Result<()> {
        unsafe {
            let result: Result<_> = libisyntax::libisyntax_read_region(
                self.isyntax,
                self.cache,
                self.index,
                tile_x,
                tile_y,
                width,
                height,
                buf.as_mut_ptr() as *mut u32,
                libisyntax::isyntax_pixel_format_t_LIBISYNTAX_PIXEL_FORMAT_RGBA as i32,
            )
            .into();
            result?;
        }
        Ok(())
    }
}
