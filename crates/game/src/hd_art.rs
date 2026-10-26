//! High resolution presentation assets. Their world sizes are independent of texture resolution.
//! Legacy sprite contracts remain available to fixtures; the shipped roster uses authored IDs.
use bevy::prelude::*;
use sim::{Direction, Footprint};
use std::path::Path;

pub const SCALE: u32 = 4;
pub const UNIT_SIZE: f32 = 96.0;
pub const UNIT_GROUND_OFFSET: f32 = 22.0;
pub const BUILDING_RISE: f32 = 40.0;

pub fn building_size(width: u8, height: u8) -> Vec2 {
    let cells = f32::from(width) + f32::from(height);
    Vec2::new(cells * 32.0 + 32.0, cells * 16.0 + 96.0)
}
pub fn building_position(footprint: Footprint, tile: crate::projection::TileSize) -> Vec2 {
    crate::buildings::sprite_position(footprint, tile)
        + Vec2::Y * (BUILDING_RISE - crate::buildings::WALL_PX as f32 / 2.0)
}
pub fn facing_rect(facing: Direction) -> Rect {
    let size = UNIT_SIZE * SCALE as f32;
    let left = f32::from(facing.index()) * size;
    Rect::new(left, 0.0, left + size, size)
}

/// World size of a frame of a walk cycle (`sprites/hd/walk`): the standing frame less its empty
/// margin, so that eight frames of eight facings stay a moderate texture.
pub const WALK_SIZE: f32 = 64.0;
/// Frames in a walk cycle, one row of eight facings each.
pub const WALK_FRAMES: u32 = 8;
/// How far below the centre of a walk frame the ground point is (22 in the standing frame).
pub const WALK_GROUND_OFFSET: f32 = 14.0;
/// Cells a figure covers in one cycle of its walk, and of its run, so that its feet stay where
/// they are planted instead of sliding (scripts/hd_infantry.py: two steps of the gait).
pub const WALK_CYCLE_CELLS: f32 = 0.69;
pub const RUN_CYCLE_CELLS: f32 = 0.87;
/// The unit speed (`units.ron`) from which a figure runs instead of walking; the art's
/// `RUNNERS` are the infantry at or above it.
pub const RUN_SPEED: u16 = 20;

/// The frame of a walk cycle for `phase` (in cycles; any real number).
pub fn walk_frame(phase: f32) -> u32 {
    (phase.rem_euclid(1.0) * WALK_FRAMES as f32) as u32 % WALK_FRAMES
}

/// The part of a walk sheet that shows `frame` of the cycle in `facing`.
pub fn walk_rect(facing: Direction, frame: u32) -> Rect {
    let size = WALK_SIZE * SCALE as f32;
    let left = f32::from(facing.index()) * size;
    let top = (frame % WALK_FRAMES) as f32 * size;
    Rect::new(left, top, left + size, top + size)
}

/// Where the walk frame hangs on a unit's view: its ground point is the standing frame's.
pub fn walk_anchor() -> Vec2 {
    Vec2::new(0.0, (UNIT_GROUND_OFFSET - WALK_GROUND_OFFSET) / WALK_SIZE)
}
/// The part of `rect` (texture pixels) of an RGBA8 `image` that holds the miniature itself:
/// the bounds of its solid pixels (the soft baked shadow is mostly translucent and left out),
/// with a small margin. Production cameos show this crop, so a tank fills its card instead of
/// sitting small in the middle of a mostly empty frame. Returns `rect` when nothing is solid or
/// the pixels are not available.
pub fn content_rect(image: &Image, rect: Rect) -> Rect {
    const SOLID: u8 = 160;
    let Some(data) = image.data.as_ref() else {
        return rect;
    };
    let width = image.width() as usize;
    let height = image.height() as usize;
    if data.len() < width * height * 4 {
        return rect;
    }
    let (x0, y0) = (rect.min.x.max(0.0) as usize, rect.min.y.max(0.0) as usize);
    let (x1, y1) = (
        (rect.max.x as usize).min(width),
        (rect.max.y as usize).min(height),
    );
    let (mut lo, mut hi) = (UVec2::splat(u32::MAX), UVec2::ZERO);
    for y in y0..y1 {
        let row = &data[(y * width + x0) * 4..(y * width + x1) * 4];
        for (i, pixel) in row.as_chunks::<4>().0.iter().enumerate() {
            if pixel[3] >= SOLID {
                let p = UVec2::new((x0 + i) as u32, y as u32);
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
    }
    if lo.x > hi.x {
        return rect;
    }
    let margin = 3.0 * SCALE as f32;
    Rect::new(
        (lo.x as f32 - margin).max(rect.min.x),
        (lo.y as f32 - margin).max(rect.min.y),
        (hi.x as f32 + 1.0 + margin).min(rect.max.x),
        (hi.y as f32 + 1.0 + margin).min(rect.max.y),
    )
}

/// The size of a cameo showing `rect` that fits `max` while keeping its proportions.
pub fn fit(rect: Rect, max: Vec2) -> Vec2 {
    let size = rect.size().max(Vec2::ONE);
    size * (max.x / size.x).min(max.y / size.y)
}

pub fn checked(root: &Path, category: &str, id: &str, size: Vec2) -> Result<Vec<u8>, String> {
    let path = root
        .join("sprites/hd")
        .join(category)
        .join(format!("{id}.png"));
    crate::tileset_image::check_png_file(
        &path,
        (size.x * SCALE as f32) as u32,
        (size.y * SCALE as f32) as u32,
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    /// A walk cycle runs through its eight frames once per cycle, in either direction of time,
    /// and every frame of every facing is its own 256-texel cell of the sheet.
    #[test]
    fn a_stride_picks_its_frame_of_the_walk_sheet() {
        assert_eq!(walk_frame(0.0), 0);
        assert_eq!(walk_frame(0.13), 1);
        assert_eq!(walk_frame(0.99), 7);
        assert_eq!(walk_frame(1.0), 0);
        assert_eq!(walk_frame(7.5), 4);
        assert_eq!(walk_frame(-0.01), 7);
        let mut cells = std::collections::BTreeSet::new();
        for facing in Direction::ALL {
            for frame in 0..WALK_FRAMES {
                let rect = walk_rect(facing, frame);
                assert_eq!(rect.size(), Vec2::splat(WALK_SIZE * SCALE as f32));
                assert!(rect.max.x <= WALK_SIZE * 8.0 * SCALE as f32);
                assert!(rect.max.y <= WALK_SIZE * WALK_FRAMES as f32 * SCALE as f32);
                assert!(cells.insert((rect.min.x as u32, rect.min.y as u32)));
            }
        }
        // The walk frame is the standing frame cropped, so its ground point is the same point
        // of the view: the anchor lowers the frame's centre by the difference of the offsets.
        let centre = -walk_anchor().y * WALK_SIZE;
        assert_eq!(centre - WALK_GROUND_OFFSET, -UNIT_GROUND_OFFSET);
    }

    /// Every unit that goes on foot has a walk cycle, in which the figure really moves: no two
    /// consecutive frames are the same picture, and the owner's paint shows in every frame.
    #[test]
    fn every_foot_soldier_has_a_walk_cycle() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let (loaded, _) = crate::units::load_match_data(&root).expect("match data");
        let size = Vec2::new(WALK_SIZE * 8.0, WALK_SIZE * WALK_FRAMES as f32);
        let mut walkers = 0;
        for def in loaded.definitions.units() {
            let kind = loaded.definitions.unit_kind(&def.id).expect("kind");
            let item = sim::ProductionItem::Unit(kind);
            let on_foot = loaded.definitions.category(item) == sim::Category::Infantry
                && loaded.definitions.domain(kind) != sim::Domain::Air;
            // The walk cycle is that of the item whose art the type shows (RA1, arsenal `art`).
            let art = loaded.definitions.art(item);
            let path = root.join("sprites/hd/walk").join(format!("{art}.png"));
            assert_eq!(path.is_file(), on_foot, "{}", def.id);
            if !on_foot {
                continue;
            }
            walkers += 1;
            let bytes = checked(&root, "walk", art, size).expect(&def.id);
            let image = image::load_from_memory(&bytes).expect(&def.id).to_rgba8();
            let frame = (WALK_SIZE * SCALE as f32) as u32;
            for facing in 0..8 {
                let cell = |pose: u32| {
                    image::imageops::crop_imm(&image, facing * frame, pose * frame, frame, frame)
                        .to_image()
                };
                for pose in 0..WALK_FRAMES {
                    let (this, next) = (cell(pose), cell((pose + 1) % WALK_FRAMES));
                    let moved = this
                        .pixels()
                        .zip(next.pixels())
                        .filter(|(a, b)| a != b)
                        .count();
                    assert!(
                        moved > 200,
                        "{} facing {facing} frame {pose}: {moved}",
                        def.id
                    );
                    let painted = this
                        .pixels()
                        .filter(|p| {
                            p[3] > 0
                                && p[2] > p[0].saturating_add(60)
                                && p[1] > p[0].saturating_add(35)
                        })
                        .count();
                    assert!(
                        painted >= 32,
                        "{} facing {facing} frame {pose}: {painted} texels of team paint",
                        def.id
                    );
                }
            }
        }
        assert!(walkers >= 15, "{walkers}");
    }

    #[test]
    fn every_roster_identity_has_distinct_valid_hd_art() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let (loaded, _) = crate::units::load_match_data(&root).expect("match data");
        let mut art = std::collections::BTreeSet::new();
        let definitions = &loaded.definitions;
        for def in definitions.buildings() {
            let kind = definitions.building_kind(&def.id).expect("kind");
            let shown = definitions.art(sim::ProductionItem::Building(kind));
            let bytes = checked(
                &root,
                "buildings",
                shown,
                building_size(def.width, def.height),
            )
            .expect(&def.id);
            // A type that names another's art (RA1: the Soviet twins of the Allied production
            // buildings and the American Airforce Command, until they have art of their own,
            // docs/DECISIONS.md R11) shows that building's artwork; every other type has its own.
            if shown == def.id {
                assert!(art.insert(bytes), "shared building artwork: {}", def.id);
            } else {
                let twin = definitions.building_kind(shown).expect(shown);
                assert_eq!(
                    definitions.art(sim::ProductionItem::Building(twin)),
                    shown,
                    "{}",
                    def.id
                );
            }
        }
        art.clear();
        for def in definitions.units() {
            let kind = definitions.unit_kind(&def.id).expect("kind");
            let bytes = checked(
                &root,
                "units",
                definitions.art(sim::ProductionItem::Unit(kind)),
                Vec2::new(UNIT_SIZE * 8.0, UNIT_SIZE),
            )
            .expect(&def.id);
            // Legacy aliases may share their equivalent silhouette; expanded categories must not
            // collapse aircraft, ships, infantry and tanks onto one generic sprite.
            art.insert(bytes);
        }
        assert!(
            art.len() >= 40,
            "the arsenal must contain at least forty distinct unit sheets"
        );
    }
    #[test]
    fn cameos_crop_to_the_solid_miniature() {
        use bevy::asset::RenderAssetUsages;
        use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
        // A 16 x 8 sheet of two 8 x 8 frames; the second holds a solid 2 x 3 block and a faint
        // shadow pixel that the crop ignores.
        let mut data = vec![0u8; 16 * 8 * 4];
        let mut put = |x: usize, y: usize, a: u8| data[(y * 16 + x) * 4 + 3] = a;
        for (x, y) in [(10, 3), (11, 3), (10, 4), (11, 4), (10, 5), (11, 5)] {
            put(x, y, 255);
        }
        put(14, 7, 60);
        let image = Image::new(
            Extent3d {
                width: 16,
                height: 8,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::all(),
        );
        let frame = Rect::new(8.0, 0.0, 16.0, 8.0);
        let crop = content_rect(&image, frame);
        // The margin is clamped to the frame.
        assert_eq!(crop, Rect::new(8.0, 0.0, 16.0, 8.0));
        let empty = content_rect(&image, Rect::new(0.0, 0.0, 8.0, 8.0));
        assert_eq!(empty, Rect::new(0.0, 0.0, 8.0, 8.0));
        let big = Image::new_fill(
            Extent3d {
                width: 400,
                height: 100,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0, 0, 0, 0],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::all(),
        );
        let mut big = big;
        if let Some(data) = big.data.as_mut() {
            for y in 40..60 {
                for x in 150..170 {
                    data[(y * 400 + x) * 4 + 3] = 255;
                }
            }
        }
        let crop = content_rect(&big, Rect::new(100.0, 0.0, 200.0, 100.0));
        assert_eq!(crop, Rect::new(138.0, 28.0, 182.0, 72.0));
        assert_eq!(
            fit(Rect::new(0.0, 0.0, 40.0, 20.0), Vec2::new(60.0, 50.0)),
            Vec2::new(60.0, 30.0)
        );
    }
    #[test]
    fn hd_ground_anchors_match_simulation_footprints() {
        for (w, h) in [(1, 1), (2, 2), (3, 3), (2, 3)] {
            let size = building_size(w, h);
            // The generator's ground centre is y=88+(w+h)*8 from image top.
            assert_eq!(88.0 + f32::from(w + h) * 8.0 - size.y / 2.0, BUILDING_RISE);
        }
        assert_eq!(70.0 - UNIT_SIZE / 2.0, UNIT_GROUND_OFFSET);
        assert_eq!(
            facing_rect(Direction::ALL[7]).max.x,
            UNIT_SIZE * 8.0 * SCALE as f32
        );
    }
}
