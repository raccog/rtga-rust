use crate::{TgaColor, TgaError, TgaImage, TgaImageType};

#[test]
fn write_blank() -> Result<(), TgaError> {
    // Create blank image
    let image = TgaImage::new(TgaImageType::TrueColorImage, 1920, 1080, 24)?;
    
    // Write image to file
    image.to_file("test0.tga")?;

    // Create blank image
    let mut image = TgaImage::new(TgaImageType::TrueColorImage, 25, 25, 24)?;

    // Set first pixel to red
    image.set_pixel(0, 0, TgaColor::RGB24([0, 0, 255]))?;
    
    // Write image to file
    image.to_file("test1.tga")?;
    
    Ok(())
}

