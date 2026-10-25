//! RA5: gems on the generated maps (docs/RA2_PARITY_PLAN.md section 5, RA5, item 8;
//! docs/DECISIONS.md R61). A gem tile is ore ground whose look has the feature `Gems`; the
//! simulation's loader marks exactly those cells as gems. Every large map has seams of gems in
//! the open country, mirrored like everything else and away from every base; the two small
//! generated maps (and the demo map, which has a generator of its own) have none, so that the
//! tests that play on them by coordinate keep their numbers.

#[path = "../examples/gen_maps.rs"]
#[allow(dead_code)]
mod gen_maps;

use gen_maps::looks::Feature;
use gen_maps::{MAPS, MapSpec, TILES};
use sim::{CellPos, load_map};

fn cells(spec: &MapSpec) -> impl Iterator<Item = (u32, u32)> + '_ {
    (0..spec.height).flat_map(move |y| (0..spec.width).map(move |x| (x, y)))
}

fn is_gem_tile(spec: &MapSpec, x: u32, y: u32) -> bool {
    TILES[spec.tile_at(x, y) as usize].look >> 4 == Feature::Gems as u8
}

#[test]
fn the_gem_tiles_are_ore_ground_of_the_densest_gems() {
    let gems: Vec<_> = TILES
        .iter()
        .filter(|tile| tile.look >> 4 == Feature::Gems as u8)
        .collect();
    assert_eq!(gems.len(), 5, "one for every ground ore lies on");
    for tile in gems {
        // 600 is twelve bails of 50 credits.
        assert_eq!(
            (tile.passable, tile.buildable, tile.water, tile.ore),
            (true, false, false, 600),
            "{}",
            tile.name
        );
    }
    assert_eq!(Feature::Gems as u8, sim::LOOK_FEATURE_GEMS);
}

#[test]
fn the_loader_marks_exactly_the_gem_tiles_as_gems() {
    for spec in &MAPS {
        let path = spec.output_dir().join(format!("{}.tmx", spec.id));
        let map = load_map(&path).unwrap_or_else(|error| panic!("{error}"));
        let mut gems = 0;
        for (x, y) in cells(spec) {
            let cell = CellPos::new(x as i32, y as i32);
            let gem = is_gem_tile(spec, x, y);
            assert_eq!(map.grid.is_gems(cell), gem, "{} ({x}, {y})", spec.id);
            if gem {
                assert_eq!(map.grid.cell(cell).expect("a cell").ore, 600);
                gems += 1;
            }
        }
        assert_eq!(map.grid.gem_cells(), gems, "{}", spec.id);
    }
}

#[test]
fn every_large_map_has_mirrored_seams_of_gems_away_from_the_bases() {
    for spec in &MAPS {
        let gems: Vec<(u32, u32)> = cells(spec)
            .filter(|&(x, y)| is_gem_tile(spec, x, y))
            .collect();
        let ore = cells(spec)
            .filter(|&(x, y)| TILES[spec.tile_at(x, y) as usize].ore > 0)
            .count();
        if ["crossroads", "riverlands"].contains(&spec.id) {
            assert!(gems.is_empty(), "{}", spec.id);
            continue;
        }
        // At least a pair of seams, and no more than a tenth of the ore cells of the map.
        assert!(gems.len() >= 34, "{}: {}", spec.id, gems.len());
        assert!(
            gems.len() * 10 <= ore,
            "{}: {} of {ore}",
            spec.id,
            gems.len()
        );
        for &(x, y) in &gems {
            assert!(
                is_gem_tile(spec, spec.width - 1 - x, spec.height - 1 - y),
                "{} ({x}, {y}) has no mirror",
                spec.id
            );
            // No gems within fifteen cells of a yard: the two fields beside a base are ore.
            for (sx, sy) in spec.starts() {
                let (dx, dy) = (
                    i64::from(x) - i64::from(sx + 1),
                    i64::from(y) - i64::from(sy + 1),
                );
                assert!(
                    dx * dx + dy * dy > 15 * 15,
                    "{}: gems at ({x}, {y}) beside the base at ({sx}, {sy})",
                    spec.id
                );
            }
        }
    }
}
