//! Material-preserving team paint. Only the authored blue paint range is recoloured; concrete,
//! armour, glass, hazard markings and shadows retain their lighting. Inputs have passed I11.
use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;

pub fn team_pixel(pixel: &mut [u8]) {
    if pixel[2] > pixel[0].saturating_add(60) && pixel[1] > pixel[0].saturating_add(35) {
        let light = i16::from(pixel[0]) - 42;
        pixel[0] = (195 + light).clamp(0, 255) as u8;
        pixel[1] = (66 + light).clamp(0, 255) as u8;
        pixel[2] = (48 + light).clamp(0, 255) as u8;
    }
}

/// Decode only bytes already validated by check_png_file. Keep CPU pixels for icons and review.
pub fn sprite_pair(
    bytes: &[u8],
    images: &mut Assets<Image>,
) -> Result<(Handle<Image>, Handle<Image>), String> {
    let image = Image::from_buffer(
        bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::nearest(),
        RenderAssetUsages::default(),
    )
    .map_err(|e| e.to_string())?;
    let mut enemy = image.clone();
    if let Some(pixels) = enemy.data.as_mut() {
        for pixel in pixels.as_chunks_mut::<4>().0 {
            team_pixel(pixel);
        }
    }
    Ok((images.add(image), images.add(enemy)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn team_paint_preserves_materials_alpha_and_shading() {
        for mut pixel in [
            [126, 139, 142, 255],
            [166, 219, 201, 255],
            [140, 140, 140, 255],
            [20, 28, 22, 90],
            [66, 111, 125, 255],
        ] {
            let before = pixel;
            team_pixel(&mut pixel);
            assert_eq!(pixel, before);
        }
        let mut blue = [42, 133, 202, 255];
        team_pixel(&mut blue);
        assert_eq!(blue, [195, 66, 48, 255]);
        let mut shadow = [24, 115, 184, 190];
        team_pixel(&mut shadow);
        assert_eq!(shadow, [177, 48, 30, 190]);
    }
}
