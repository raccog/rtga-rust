//! # rtga-rust
//!
//! `rtga-rust` is a toy library for interfacing with TGA images.

#[cfg(test)]
mod tests;

use std::convert::TryInto;
use std::fs::File;
use std::io::Error as IOError;
use std::io::{Read, Write};
use std::path::Path;

use TgaColor::*;
use TgaError::*;
use TgaImageType::*;

/// The size of a TGA header in bytes.
pub const HEADER_SIZE: usize = 18;

/// The color formats used in a TGA image.
#[derive(Clone, Copy)]
pub enum TgaColor {
    Greyscale([u8; 1]),
    RGB16([u8; 2]),
    RGB24([u8; 3]),
    RGBA([u8; 4])
}

impl TgaColor {
    /// Extracts a slice containing the color data.
    /// 
    /// The length of the slice will be the same as the color's byte depth.
    pub fn as_slice(&self) -> &[u8] {
        match self {
            Greyscale(s) => &s[..],
            RGB16(s) => &s[..],
            RGB24(s) => &s[..],
            RGBA(s) => &s[..],
        }
    }

    /// Returns the color's full bit depth.
    pub fn bit_depth(&self) -> u8 {
        self.byte_depth() * 8
    }

    /// Returns the color's full byte depth.
    pub fn byte_depth(&self) -> u8 {
        match self {
            Greyscale(_) => 1,
            RGB16(_) => 2,
            RGB24(_) => 3,
            RGBA(_) => 4
        }
    }
}

/// An interface for editing a TGA image file.
/// 
/// Image data is saved in memory when editing and can be read from or written to a file. Provides functions for editing individual pixels. 
#[derive(Clone)]
pub struct TgaImage {
    pub header: TgaHeader,
    state: TgaImageState,
    id: Box<[u8]>,
    color_map: Box<[u8]>,
    data: Box<[u8]>,
}

/// The possible types of a TGA image.
#[derive(Clone, Copy)]
pub enum TgaImageType {
    NoImage = 0,
    ColorMappedImage = 1,
    TrueColorImage = 2,
    BlackAndWhiteImage = 3,
    RleColorMappedImage = 9,
    RleTrueColorImage = 10,
    RleBlackAndWhiteImage = 11,
}

impl TgaImageType {
    /// Tries to convert `val` to one of the possible `TgaImageType`s.
    /// 
    /// # Errors
    /// If `val` is not a valid image type, then returns `InvalidImageType` error.
    pub fn from_u8(val: u8) -> Result<TgaImageType, TgaError> {
        match val {
            0 => Ok(NoImage),
            1 => Ok(ColorMappedImage),
            2 => Ok(TrueColorImage),
            3 => Ok(BlackAndWhiteImage),
            9 => Ok(RleColorMappedImage),
            10 => Ok(RleTrueColorImage),
            11 => Ok(RleBlackAndWhiteImage),
            _ => Err(InvalidImageType)
        }
    }

    /// Returns true if `color` is in a valid format for the image type.
    pub fn valid_color(&self, color: TgaColor) -> bool {
        match self {
            NoImage => false,
            ColorMappedImage | TrueColorImage |
            RleColorMappedImage |
            RleTrueColorImage => match color {
                Greyscale(_) => false,
                _ => true
            },
            BlackAndWhiteImage | RleBlackAndWhiteImage => match color {
                Greyscale(_) => true,
                _ => false
            }
        }
    }

    /// Returns true if `bit_depth` is a valid bit depth for the image type.
    pub fn valid_depth(&self, bit_depth: u8) -> bool {
        match self {
            NoImage => bit_depth == 0,
            ColorMappedImage | TrueColorImage |
            RleColorMappedImage |
            RleTrueColorImage => match bit_depth {
                16 | 24 | 32 => true,
                _ => false
            }
            BlackAndWhiteImage | RleBlackAndWhiteImage => bit_depth == 8
        }
    }
}

/// The current state of a TGA image in memory.
#[derive(Copy, Clone)]
pub enum TgaImageState {
    Uncompressed,
    ColorMapped,
    Rle,
}

/// The header for a TGA image file.
#[derive(Clone, Copy)]
pub struct TgaHeader {
    pub id_size: u8,
    pub has_color_map: bool,
    pub image_type: TgaImageType,
    pub color_map_first_index: u16,
    pub color_map_size: u16,
    pub color_map_bit_depth: u8,
    pub x_origin: u16,
    pub y_origin: u16,
    pub width: u16,
    pub height: u16,
    pub image_bit_depth: u8,
    pub descriptor: u8,
}

impl TgaHeader {
    /// Tries to create a `TgaHeader` from the data in `buf`.
    /// 
    /// # Errors
    /// TODO: Change expect() calls to `TgaError`s
    pub fn from_buf(buf: [u8; HEADER_SIZE]) -> Result<TgaHeader, TgaError> {
        Ok(TgaHeader {
            id_size: buf[0],
            has_color_map: buf[1] != 0,
            image_type: TgaImageType::from_u8(buf[2])?,
            color_map_first_index: u16::from_le_bytes(buf[3..5].try_into().expect("bad slice")),
            color_map_size: u16::from_le_bytes(buf[5..7].try_into().expect("bad slice")),
            color_map_bit_depth: buf[7],
            x_origin: u16::from_le_bytes(buf[8..10].try_into().expect("bad slice")),
            y_origin: u16::from_le_bytes(buf[10..12].try_into().expect("bad slice")),
            width: u16::from_le_bytes(buf[12..14].try_into().expect("bad slice")),
            height: u16::from_le_bytes(buf[14..16].try_into().expect("bad slice")),
            image_bit_depth: buf[16],
            descriptor: buf[17]
        })
    }

    /// Returns the size of the TGA image in bytes.
    /// 
    /// Includes the header, color map, id, and pixel data.
    pub fn file_size(&self) -> usize {
        HEADER_SIZE as usize + self.id_size as usize + self.color_map_size as usize + self.image_size()
    }

    /// Returns the size of the TGA image pixel data in bytes.
    pub fn image_size(&self) -> usize {
        image_size(self.width, self.height, self.image_bit_depth)
    }

    /// Returns the header as a byte array.
    pub fn to_buf(&self) -> [u8; HEADER_SIZE] {
        [
            self.id_size,
            if self.has_color_map { 1 } else { 0 },
            self.image_type as u8,
            self.color_map_first_index as u8 & 0xff,
            (self.color_map_first_index >> 8) as u8,
            self.color_map_size as u8 & 0xff,
            (self.color_map_size >> 8) as u8,
            self.color_map_bit_depth,
            self.x_origin as u8 & 0xff,
            (self.x_origin >> 8) as u8,
            self.y_origin as u8 & 0xff,
            (self.y_origin >> 8) as u8,
            self.width as u8 & 0xff,
            (self.width >> 8) as u8,
            self.height as u8 & 0xff,
            (self.height >> 8) as u8,
            self.image_bit_depth,
            self.descriptor
        ]
    }
}

/// An error resulting from one of this library's functions.
#[derive(Debug)]
pub enum TgaError {
    InvalidPixelDepth,
    InvalidImageType,
    InvalidSize,
    InvalidCoordinate,
    InvalidColor,
    FileOpen(IOError),
    FileRead(IOError),
    FileWrite(IOError),
}

impl TgaImage {
    /// Tries to create a new color with black pixels.
    /// 
    /// # Errors
    /// If `bit_depth` is invalid for `image_type`, returns `InvalidPixelDepth` error.
    pub fn new(image_type: TgaImageType, width: u16, height: u16, bit_depth: u8) -> Result<TgaImage, TgaError> {
        // Ensure the pixel depth is valid
        if !image_type.valid_depth(bit_depth) {
            return Err(InvalidPixelDepth);
        }

        // Create header
        let header = TgaHeader {
            id_size: 0,
            has_color_map: false,
            image_type,
            color_map_first_index: 0,
            color_map_size: 0,
            color_map_bit_depth: 0,
            x_origin: 0,
            y_origin: 0,
            width,
            height,
            image_bit_depth: bit_depth,
            descriptor: 0
        };

        Ok(TgaImage {
            header,
            state: TgaImageState::Uncompressed,
            id: vec![].into_boxed_slice(),
            color_map: vec![].into_boxed_slice(),
            data: vec![0; image_size(width, height, bit_depth)].into_boxed_slice()
        })
    }

    /// Tries to read a TGA image from a file.
    /// 
    /// # Errors
    /// If the file could not be opened, returns `FileOpen` error.
    /// 
    /// If the file could not be read, returns `FileRead` error.
    /// 
    /// If the file is not large enough to contain a TGA header, returns `InvalidSize` error.
    /// 
    /// If the file is not large enough to contain the TGA image size read from the header, returns `InvalidSize` error.
    /// 
    /// If the bit depth is invalid for the image type, returns `InvalidPixelDepth` error.
    pub fn from_file<P: AsRef<Path>>(&self, filename: P) -> Result<TgaImage, TgaError> {
        // Open file and read into buffer
        let mut file = File::open(filename).map_err(|e| {FileOpen(e)})?;
        let mut buf = vec![];
        let size = file.read_to_end(&mut buf).map_err(|e| {FileRead(e)})?;
        if size < HEADER_SIZE {
            return Err(InvalidSize);
        }

        // Copy header from file
        let header_buf: [u8; HEADER_SIZE] = buf[0..HEADER_SIZE].try_into().map_err(|_| {InvalidSize})?;
        let header = TgaHeader::from_buf(header_buf)?;

        // Ensure file size is large enough to contain all data specified in the header
        if size < header.file_size() {
            return Err(InvalidSize);
        }

        // Ensure the pixel depth is valid
        if !header.image_type.valid_depth(header.image_bit_depth) {
            return Err(InvalidPixelDepth);
        }

        // Read image id, color map, and image data
        let mut idx = HEADER_SIZE;
        let id = buf[idx..idx + header.id_size as usize].to_vec().into_boxed_slice();
        idx += header.id_size as usize;
        let color_map = buf[idx..idx + header.color_map_size as usize].to_vec().into_boxed_slice();
        idx += header.color_map_size as usize;
        let data = buf[idx..idx + image_size(header.width, header.height, header.image_bit_depth)].to_vec().into_boxed_slice();

        Ok(TgaImage {
            header,
            state: TgaImageState::Uncompressed,
            id,
            color_map,
            data
        })
    }
    
    pub fn set_pixel(&mut self, x: u16, y: u16, color: TgaColor) -> Result<(), TgaError> {
        // Ensure that the pixel coordinate is valid for this image
        if self.header.width <= x || self.header.height <= y {
            return Err(InvalidCoordinate);
        }

        // Ensure the color is valid for this image
        if !self.header.image_type.valid_color(color) {
            return Err(InvalidColor);
        }

        // Ensure the color's pixel depth is valid for this image
        let bit_depth = color.bit_depth();
        if !self.header.image_type.valid_depth(bit_depth) || bit_depth != self.header.image_bit_depth {
            return Err(InvalidPixelDepth);
        }

        // Set pixel to color
        let byte_depth = (bit_depth / 8) as u16;
        let start = (x + y * self.header.width) * byte_depth;
        let end = start + byte_depth;
        let start = start as usize;
        let end = end as usize;
        self.data[start..end].copy_from_slice(color.as_slice());

        Ok(())
    }

    pub fn to_file<P: AsRef<Path>>(&self, filename: P) -> Result<(), TgaError> {
        // Allocate buffer to write
        let mut buf = vec![0; self.header.file_size()].into_boxed_slice();

        // Copy header and all data to buffer
        let id_size = self.header.id_size as usize;
        let color_map_size = self.header.color_map_size as usize;
        let image_size = self.header.image_size();
        buf[0..HEADER_SIZE].copy_from_slice(&self.header.to_buf());
        let mut idx = HEADER_SIZE;
        buf[idx..idx + id_size].copy_from_slice(&self.id);
        idx += id_size;
        buf[idx..idx + color_map_size].copy_from_slice(&self.color_map);
        idx += color_map_size;
        buf[idx..idx + image_size].copy_from_slice(&self.data);

        // Create file and write buffer
        let mut file = File::create(filename).map_err(|e| {FileOpen(e)})?;
        file.write_all(&buf).map_err(|e| {FileWrite(e)})?;

        Ok(())
    }
}

fn image_size(width: u16, height: u16, bit_depth: u8) -> usize {
    return width as usize * height as usize * (bit_depth as usize / 8)
}
