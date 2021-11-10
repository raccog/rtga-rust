//! # rtga-rust
//!
//! `rtga-rust` is a library for interfacing with TGA images.

#[cfg(test)]
mod tests;

use std::convert::TryInto;
use std::fs::File;
use std::io::Error as IOError;
use std::io::{Read, Write};
use std::path::Path;

use TgaColor::*;
use TgaImageType::*;

pub const HEADER_SIZE: usize = 18;

#[derive(Clone, Copy)]
pub enum TgaColor {
    Greyscale([u8; 1]),
    RGB16([u8; 2]),
    RGB24([u8; 3]),
    RGBA([u8; 4])
}

impl TgaColor {
    pub fn pixel_depth(&self) -> u8 {
        match self {
            Greyscale(_) => 8,
            RGB16(_) => 16,
            RGB24(_) => 24,
            RGBA(_) => 32
        }
    }

    pub fn to_slice(&self) -> &[u8] {
        match self {
            Greyscale(s) => &s[..],
            RGB16(s) => &s[..],
            RGB24(s) => &s[..],
            RGBA(s) => &s[..],
        }
    }
}

#[derive(Clone)]
pub struct TgaImage {
    pub header: TgaHeader,
    state: TgaImageState,
    id: Box<[u8]>,
    color_map: Box<[u8]>,
    data: Box<[u8]>,
}

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
    pub fn from_u8(val: u8) -> Result<TgaImageType, TgaError> {
        match val {
            0 => Ok(NoImage),
            1 => Ok(ColorMappedImage),
            2 => Ok(TrueColorImage),
            3 => Ok(BlackAndWhiteImage),
            9 => Ok(RleColorMappedImage),
            10 => Ok(RleTrueColorImage),
            11 => Ok(RleBlackAndWhiteImage),
            _ => Err(TgaError::InvalidImageType)
        }
    }

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

    pub fn valid_depth(&self, pixel_depth: u8) -> bool {
        match self {
            NoImage => pixel_depth == 0,
            ColorMappedImage | TrueColorImage |
            RleColorMappedImage |
            RleTrueColorImage => match pixel_depth {
                16 | 24 | 32 => true,
                _ => false
            }
            BlackAndWhiteImage | RleBlackAndWhiteImage => pixel_depth == 8
        }
    }
}

#[derive(Copy, Clone)]
pub enum TgaImageState {
    Uncompressed,
    ColorMapped,
    Rle,
}

#[derive(Clone, Copy)]
pub struct TgaHeader {
    pub id_size: u8,
    pub has_color_map: bool,
    pub image_type: TgaImageType,
    pub color_map_first_index: u16,
    pub color_map_size: u16,
    pub color_map_pixel_depth: u8,
    pub x_origin: u16,
    pub y_origin: u16,
    pub width: u16,
    pub height: u16,
    pub image_pixel_depth: u8,
    pub descriptor: u8,
}

impl TgaHeader {
    pub fn from_buf(buf: [u8; HEADER_SIZE]) -> Result<TgaHeader, TgaError> {
        Ok(TgaHeader {
            id_size: buf[0],
            has_color_map: buf[1] != 0,
            image_type: TgaImageType::from_u8(buf[2])?,
            color_map_first_index: u16::from_le_bytes(buf[3..5].try_into().expect("bad slice")),
            color_map_size: u16::from_le_bytes(buf[5..7].try_into().expect("bad slice")),
            color_map_pixel_depth: buf[7],
            x_origin: u16::from_le_bytes(buf[8..10].try_into().expect("bad slice")),
            y_origin: u16::from_le_bytes(buf[10..12].try_into().expect("bad slice")),
            width: u16::from_le_bytes(buf[12..14].try_into().expect("bad slice")),
            height: u16::from_le_bytes(buf[14..16].try_into().expect("bad slice")),
            image_pixel_depth: buf[16],
            descriptor: buf[17]
        })
    }

    pub fn file_size(&self) -> usize {
        HEADER_SIZE as usize + self.id_size as usize + self.color_map_size as usize + self.image_size()
    }

    pub fn image_size(&self) -> usize {
        image_size(self.width, self.height, self.image_pixel_depth)
    }

    pub fn to_buf(&self) -> [u8; HEADER_SIZE] {
        [
            self.id_size,
            if self.has_color_map { 1 } else { 0 },
            self.image_type as u8,
            self.color_map_first_index as u8 & 0xff,
            (self.color_map_first_index >> 8) as u8,
            self.color_map_size as u8 & 0xff,
            (self.color_map_size >> 8) as u8,
            self.color_map_pixel_depth,
            self.x_origin as u8 & 0xff,
            (self.x_origin >> 8) as u8,
            self.y_origin as u8 & 0xff,
            (self.y_origin >> 8) as u8,
            self.width as u8 & 0xff,
            (self.width >> 8) as u8,
            self.height as u8 & 0xff,
            (self.height >> 8) as u8,
            self.image_pixel_depth,
            self.descriptor
        ]
    }
}

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
    pub fn new(image_type: TgaImageType, width: u16, height: u16, pixel_depth: u8) -> Result<TgaImage, TgaError> {
        // Ensure the pixel depth is valid
        if !image_type.valid_depth(pixel_depth) {
            return Err(TgaError::InvalidPixelDepth);
        }

        // Create header
        let header = TgaHeader {
            id_size: 0,
            has_color_map: false,
            image_type,
            color_map_first_index: 0,
            color_map_size: 0,
            color_map_pixel_depth: 0,
            x_origin: 0,
            y_origin: 0,
            width,
            height,
            image_pixel_depth: pixel_depth,
            descriptor: 0
        };

        Ok(TgaImage {
            header,
            state: TgaImageState::Uncompressed,
            id: vec![].into_boxed_slice(),
            color_map: vec![].into_boxed_slice(),
            data: vec![0; image_size(width, height, pixel_depth)].into_boxed_slice()
        })
    }

    pub fn from_file<P: AsRef<Path>>(&self, filename: P) -> Result<TgaImage, TgaError> {
        // Open file and read into buffer
        let mut file = File::open(filename).map_err(|e| {TgaError::FileOpen(e)})?;
        let mut buf = vec![];
        let size = file.read_to_end(&mut buf).map_err(|e| {TgaError::FileRead(e)})?;
        if size < HEADER_SIZE {
            return Err(TgaError::InvalidSize);
        }

        // Copy header from file
        let header_buf: [u8; HEADER_SIZE] = buf[0..HEADER_SIZE].try_into().map_err(|_| {TgaError::InvalidSize})?;
        let header = TgaHeader::from_buf(header_buf)?;

        // Ensure file size is large enough to contain all data specified in the header
        if size < header.file_size() {
            return Err(TgaError::InvalidSize);
        }

        // Ensure the pixel depth is valid
        if !header.image_type.valid_depth(header.image_pixel_depth) {
            return Err(TgaError::InvalidPixelDepth);
        }

        // Read image id, color map, and image data
        let mut idx = HEADER_SIZE;
        let id = buf[idx..idx + header.id_size as usize].to_vec().into_boxed_slice();
        idx += header.id_size as usize;
        let color_map = buf[idx..idx + header.color_map_size as usize].to_vec().into_boxed_slice();
        idx += header.color_map_size as usize;
        let data = buf[idx..idx + image_size(header.width, header.height, header.image_pixel_depth)].to_vec().into_boxed_slice();

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
            return Err(TgaError::InvalidCoordinate);
        }

        // Ensure the color is valid for this image
        if !self.header.image_type.valid_color(color) {
            return Err(TgaError::InvalidColor);
        }

        // Ensure the color's pixel depth is valid for this image
        let pixel_depth = color.pixel_depth();
        if !self.header.image_type.valid_depth(pixel_depth) || pixel_depth != self.header.image_pixel_depth {
            return Err(TgaError::InvalidPixelDepth);
        }

        // Set pixel to color
        let pixel_depth_bytes = (pixel_depth / 8) as u16;
        let start = (x + y * self.header.width) * pixel_depth_bytes;
        let end = start + pixel_depth_bytes;
        let start = start as usize;
        let end = end as usize;
        self.data[start..end].copy_from_slice(color.to_slice());

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
        let mut file = File::create(filename).map_err(|e| {TgaError::FileOpen(e)})?;
        file.write_all(&buf).map_err(|e| {TgaError::FileWrite(e)})?;

        Ok(())
    }
}

fn image_size(width: u16, height: u16, pixel_depth: u8) -> usize {
    return width as usize * height as usize * (pixel_depth as usize / 8)
}
