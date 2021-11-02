use crate::{TgaError, TgaImage, TgaImageType};

#[test]
fn write_blank() -> Result<(), TgaError> {
    // Create blank image
    let image = TgaImage::new(TgaImageType::TrueColorImage, 1900, 1080, 24)?;
    
    // Write image to file
    image.to_file("test.tga")?;
    
    Ok(())
}

