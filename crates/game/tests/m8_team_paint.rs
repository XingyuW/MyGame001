//! M8 review 1 P1-1: ownership must survive every shipped unit facing.
#[path = "../examples/gen_unit_sprites.rs"]
#[allow(dead_code)]
mod gen_unit_sprites;
#[path = "../src/presentation.rs"]
mod presentation;
#[path = "../src/tileset_image.rs"]
#[allow(dead_code)]
mod tileset_image;

use bevy::prelude::*;
use gen_unit_sprites::{FACINGS, FRAME, SPRITES};

#[test]
fn every_shipped_unit_facing_has_visible_material_preserving_team_paint() {
    let mut failures = Vec::new();
    for kind in &SPRITES {
        let path = gen_unit_sprites::output_dir().join(format!("{}.png", kind.name));
        let bytes = tileset_image::check_png_file(&path, FRAME * FACINGS, FRAME)
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(bytes, gen_unit_sprites::sheet_png(kind), "{}", kind.name);
        let mut images = Assets::<Image>::default();
        let (friendly, enemy) = presentation::sprite_pair(&bytes, &mut images).expect("decode");
        let friendly = images.get(&friendly).unwrap().data.as_ref().unwrap();
        let enemy = images.get(&enemy).unwrap().data.as_ref().unwrap();
        assert_eq!(*friendly, gen_unit_sprites::sheet_pixels(kind).2);
        let mut coverage = [0; FACINGS as usize];
        for (index, (human, opponent)) in friendly
            .as_chunks::<4>()
            .0
            .iter()
            .zip(enemy.as_chunks::<4>().0)
            .enumerate()
        {
            assert_eq!(human[3], opponent[3], "{} alpha", kind.name);
            // Explicit authored paint palette, independent of the production predicate.
            // Every other material (including transparent pixels, shadows and front markers)
            // must be byte-identical, so restoring whole-sprite tint cannot satisfy this test.
            if human[..3] == [42, 133, 202] || human[..3] == [28, 119, 188] {
                assert_eq!(human[3], 255, "paint must be opaque");
                assert_ne!(human, opponent, "{} paint must change", kind.name);
                coverage[index % (FRAME * FACINGS) as usize / FRAME as usize] += 1;
            } else {
                assert_eq!(human, opponent, "{} neutral material", kind.name);
            }
        }
        println!("{} E/SE/S/SW/W/NW/N/NE: {coverage:?}", kind.name);
        for (frame, count) in coverage.into_iter().enumerate() {
            // Ten opaque pixels is the smallest established good scout panel. This rejects
            // the missing NW paint and the four-pixel infantry W/N slivers found in review.
            if count < 10 {
                failures.push(format!("{} frame {frame}: {count} < 10", kind.name));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
