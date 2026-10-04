//! M7 skirmish maps (PLAN.md section 7, M7: "three maps"; docs/DECISIONS.md D50): the committed
//! files are the generator's output, the maps pass the `sim` loader with the terrain the generator
//! describes, the start positions of the shipped skirmish file are the generator's, the maps are
//! point-symmetric, and each start has a base, ore, and a way to the other start.

#[path = "../examples/gen_maps.rs"]
#[allow(dead_code)]
mod gen_maps;

#[path = "../src/tileset_image.rs"]
#[allow(dead_code)]
mod tileset_image;

use std::collections::VecDeque;
use std::path::PathBuf;

use gen_maps::{MAPS, MapSpec, ORE, TILES};
use sim::{CellPos, Grid, LoadedMap, load_map, load_match_definitions, load_skirmish};

fn load(spec: &MapSpec) -> LoadedMap {
    let path = spec.output_dir().join(format!("{}.tmx", spec.id));
    load_map(&path).unwrap_or_else(|error| panic!("{error}"))
}

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

#[test]
fn committed_map_files_match_generator_output() {
    let hint = "run `cargo run -p game --example gen_maps`";
    for spec in &MAPS {
        let dir = spec.output_dir();
        let tmx = std::fs::read_to_string(dir.join(format!("{}.tmx", spec.id)))
            .unwrap_or_else(|error| panic!("{}: {error}", spec.id));
        assert!(
            tmx == gen_maps::map_tmx(spec),
            "{}.tmx differs; {hint}",
            spec.id
        );
        let png = std::fs::read(dir.join(spec.tileset_name())).expect("tileset is committed");
        assert!(
            png == gen_maps::tileset_png(),
            "{} differs; {hint}",
            spec.tileset_name()
        );
    }
}

#[test]
fn the_maps_load_with_the_generators_terrain_and_their_tilesets_pass_the_png_subset() {
    for spec in &MAPS {
        let map = load(spec);
        assert_eq!(
            (map.grid.width(), map.grid.height()),
            (spec.width, spec.height),
            "{}",
            spec.id
        );
        for y in 0..spec.height {
            for x in 0..spec.width {
                let kind = &TILES[spec.tile_at(x, y) as usize];
                let cell = map
                    .grid
                    .cell(CellPos::new(x as i32, y as i32))
                    .expect("cell inside the map");
                assert_eq!(
                    (cell.passable, cell.buildable, i64::from(cell.ore)),
                    (kind.passable, kind.buildable, i64::from(kind.ore)),
                    "{} cell ({x}, {y}), {}",
                    spec.id,
                    kind.name
                );
            }
        }
        // The tileset image passes the PNG subset the renderer is restricted to (D16).
        let path = spec.output_dir().join(format!("{}.tmx", spec.id));
        tileset_image::check_tileset_image(&path, &map.tileset_image)
            .unwrap_or_else(|error| panic!("{error}"));
        // The minimap draws every cell (M5 review 1 P2-2 concerned W + H > 128).
        assert!(spec.width + spec.height <= 256);
    }
}

#[test]
fn the_maps_are_point_symmetric_so_both_starts_are_equal() {
    for spec in &MAPS {
        for y in 0..spec.height {
            for x in 0..spec.width {
                assert_eq!(
                    spec.tile_at(x, y),
                    spec.tile_at(spec.width - 1 - x, spec.height - 1 - y),
                    "{} ({x}, {y})",
                    spec.id
                );
            }
        }
        // The second start is the first's mirror for a 3 x 3 yard (its footprint's origin).
        let (x, y) = spec.starts[0];
        assert_eq!(
            spec.starts[1],
            (spec.width - 3 - x, spec.height - 3 - y),
            "{}",
            spec.id
        );
    }
}

/// The passable cells reachable from `start` over the terrain (four-neighbour steps, which connect
/// the same cells as the pathfinder's eight-neighbour steps, D45).
fn reachable(grid: &Grid, start: CellPos) -> Vec<CellPos> {
    let index = |cell: CellPos| (cell.y as u32 * grid.width() + cell.x as u32) as usize;
    let mut seen = vec![false; (grid.width() * grid.height()) as usize];
    let mut queue = VecDeque::from([start]);
    seen[index(start)] = true;
    let mut cells = Vec::new();
    while let Some(cell) = queue.pop_front() {
        cells.push(cell);
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let next = CellPos::new(cell.x + dx, cell.y + dy);
            if grid.cell(next).is_some_and(|terrain| terrain.passable) && !seen[index(next)] {
                seen[index(next)] = true;
                queue.push_back(next);
            }
        }
    }
    cells
}

#[test]
fn each_start_has_a_base_site_ore_nearby_and_a_way_to_the_other_start() {
    for spec in &MAPS {
        let map = load(spec);
        let starts = spec.starts.map(|(x, y)| CellPos::new(x as i32, y as i32));
        for start in starts {
            // Buildable ground for the 3 x 3 yard and the cells around it.
            for y in start.y - 1..=start.y + 3 {
                for x in start.x - 1..=start.x + 3 {
                    let cell = map.grid.cell(CellPos::new(x, y)).expect("on the map");
                    assert!(cell.buildable, "{} ({x}, {y}) near {start:?}", spec.id);
                }
            }
            // An ore field within 10 cells.
            let near_ore = (start.y - 10..=start.y + 10)
                .flat_map(|y| (start.x - 10..=start.x + 10).map(move |x| CellPos::new(x, y)))
                .filter(|&cell| map.grid.cell(cell).is_some_and(|terrain| terrain.ore > 0))
                .count();
            assert!(
                near_ore >= 9,
                "{}: {near_ore} ore cells near {start:?}",
                spec.id
            );
        }
        let from_first = reachable(&map.grid, starts[0]);
        assert!(from_first.contains(&starts[1]), "{}", spec.id);
        // Every passable cell is reachable: no part of the map is cut off.
        let passable = (0..spec.height)
            .flat_map(|y| (0..spec.width).map(move |x| (x, y)))
            .filter(|&(x, y)| TILES[spec.tile_at(x, y) as usize].passable)
            .count();
        assert_eq!(from_first.len(), passable, "{}", spec.id);
        // The map has ore.
        assert!(
            (0..spec.height)
                .flat_map(|y| (0..spec.width).map(move |x| (x, y)))
                .any(|(x, y)| spec.tile_at(x, y) == ORE)
        );
    }
}

#[test]
fn the_shipped_skirmish_file_lists_every_generated_map_with_its_starts() {
    let data = assets().join("data");
    let definitions = load_match_definitions(
        data.join("units.ron"),
        data.join("economy.ron"),
        data.join("combat.ron"),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let skirmish = load_skirmish(data.join("skirmish.ron"), &definitions)
        .unwrap_or_else(|error| panic!("{error}"));
    // The demo map of M1 and the two maps of M7: three maps (PLAN.md section 1).
    assert!(skirmish.maps.len() >= 3);
    for spec in &MAPS {
        let def = skirmish
            .maps
            .iter()
            .find(|map| map.id == spec.id)
            .unwrap_or_else(|| panic!("{} is not in the skirmish file", spec.id));
        assert_eq!(
            def.starts,
            spec.starts.map(|(x, y)| CellPos::new(x as i32, y as i32)),
            "{}",
            spec.id
        );
    }
}
