//! # rtga-rust
//!
//! `rtga-rust` is a library for interfacing with TGA images.

#[cfg(test)]
mod tests;

struct TgaImage {
    header: TgaHeader,
    state: TgaImageState,
    id: Box<[u8]>,
    color_map: Box<[u8]>,
    data: Box<[u8]>,
}

enum TgaImageType {
    NoImage = 0,
    ColorMappedImage = 1,
    TrueColorImage = 2,
    BlackAndWhiteImage = 3,
    RleColorMappedImage = 9,
    RleTrueColorImage = 10,
    RleBlackAndWhiteImage = 11,
}

enum TgaImageState {
    Uncompressed,
    ColorMapped,
    Rle,
}

struct TgaHeader {
    id_length: u8,
    has_color_map: bool,
    image_type: TgaImageType,
    color_map_first_index: u16,
    color_map_length: u16,
    color_map_pixel_depth: u8,
    x_origin: u16,
    y_origin: u16,
    width: u16,
    height: u16,
    image_pixel_depth: u8,
    descriptor: u8,
}

enum TgaError {
    InvalidPixelDepth,
}

