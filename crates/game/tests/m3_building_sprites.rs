//! M3 placeholder building sprites: the committed sprites are the generator's output, and they
//! pass the PNG subset the renderer is restricted to (docs/DECISIONS.md D16, D20, D27) at the
//! size their footprint implies.

#[path = "../examples/gen_building_sprites.rs"]
#[allow(dead_code)]
mod gen_building_sprites;

#[path = "../src/tileset_image.rs"]
#[allow(dead_code)]
mod tileset_image;

use gen_building_sprites::{SPRITES, WALL_PX, image_size, sprite_pixels};
use tileset_image::{TilesetImageErrorKind, check_png_file};

#[test]
fn committed_building_sprites_match_generator_output() {
    let hint = "run `cargo run -p game --example gen_building_sprites`";
    for sprite in &SPRITES {
        let path = gen_building_sprites::output_dir().join(format!("{}.png", sprite.name));
        let committed = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(
            committed == gen_building_sprites::sprite_png(sprite),
            "{} differs; {hint}",
            path.display()
        );
    }
}

#[test]
fn building_sprites_pass_the_png_subset_at_their_footprints_size() {
    for sprite in &SPRITES {
        let path = gen_building_sprites::output_dir().join(format!("{}.png", sprite.name));
        let (width, height) = image_size(sprite.width, sprite.height);
        let bytes = check_png_file(&path, width, height).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(bytes, std::fs::read(&path).expect("sprite"));
        let error = check_png_file(&path, width, height + 1).expect_err("size");
        assert!(matches!(
            error.kind,
            TilesetImageErrorKind::SizeMismatch { .. }
        ));
    }
}

#[test]
fn a_sprite_covers_its_footprint_and_the_walls_above_it() {
    // The footprint's diamond fills the image's lower part: its four corners are on the
    // image's edges, and the roof (the diamond lifted by WALL_PX) reaches the top edge.
    for sprite in &SPRITES {
        let (width, height, pixels) = sprite_pixels(sprite);
        let opaque = |x: u32, y: u32| pixels[((y * width + x) * 4 + 3) as usize] == 255;
        let left_corner_y = u32::from(sprite.height) * 16 + WALL_PX;
        let right_corner_y = u32::from(sprite.width) * 16 + WALL_PX;
        assert!(opaque(0, left_corner_y - 1), "{}: left corner", sprite.name);
        assert!(
            opaque(width - 1, right_corner_y - 1),
            "{}: right corner",
            sprite.name
        );
        let bottom_x = u32::from(sprite.width) * 32;
        assert!(
            opaque(bottom_x, height - 1),
            "{}: bottom corner",
            sprite.name
        );
        let top_x = u32::from(sprite.height) * 32;
        assert!(opaque(top_x, 0), "{}: roof's top corner", sprite.name);
        // The corners of the image are transparent.
        for (x, y) in [
            (0, 0),
            (width - 1, 0),
            (0, height - 1),
            (width - 1, height - 1),
        ] {
            assert!(!opaque(x, y), "{}: ({x}, {y})", sprite.name);
        }
    }
}
