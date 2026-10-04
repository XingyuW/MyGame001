//! M1 demo map: the committed files are the generator's output, and the map passes the `sim`
//! loader with the terrain the generator describes.

#[path = "../examples/gen_demo_map.rs"]
#[allow(dead_code)]
mod gen_demo_map;

use gen_demo_map::{
    MARKER_ORIGIN, MARKER_X_END, MARKER_Y_END, ORE, ROCK, SIDE, TILES, WATER, tile_at,
};
use sim::{CellPos, LoadedMap, load_map};

fn load_demo() -> LoadedMap {
    let path = gen_demo_map::output_dir().join("demo.tmx");
    load_map(&path).unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn committed_demo_files_match_generator_output() {
    let dir = gen_demo_map::output_dir();
    let hint = "run `cargo run -p game --example gen_demo_map`";
    let tmx = std::fs::read_to_string(dir.join("demo.tmx")).expect("demo.tmx is committed");
    assert!(tmx == gen_demo_map::map_tmx(), "demo.tmx differs; {hint}");
    let png = std::fs::read(dir.join("demo_tiles.png")).expect("demo_tiles.png is committed");
    assert!(
        png == gen_demo_map::tileset_png(),
        "demo_tiles.png differs; {hint}"
    );
}

#[test]
fn demo_map_loads_as_a_64_by_64_grid_matching_the_generator() {
    let map = load_demo();
    assert_eq!((map.grid.width(), map.grid.height()), (SIDE, SIDE));
    assert_eq!((map.tile_width, map.tile_height), (64, 32));
    for y in 0..SIDE {
        for x in 0..SIDE {
            let kind = &TILES[tile_at(x, y) as usize];
            let cell = map
                .grid
                .cell(CellPos::new(x as i32, y as i32))
                .expect("cell inside the map");
            assert_eq!(
                (cell.passable, cell.buildable, i64::from(cell.ore)),
                (kind.passable, kind.buildable, i64::from(kind.ore)),
                "cell ({x}, {y}), {}",
                kind.name
            );
        }
    }
}

#[test]
fn demo_map_has_markers_and_every_terrain_kind() {
    let last = SIDE - 1;
    assert_eq!(tile_at(0, 0), MARKER_ORIGIN);
    assert_eq!(tile_at(last, 0), MARKER_X_END);
    assert_eq!(tile_at(0, last), MARKER_Y_END);
    for id in 0..TILES.len() as u32 {
        let count = (0..SIDE)
            .flat_map(|y| (0..SIDE).map(move |x| tile_at(x, y)))
            .filter(|&tile| tile == id)
            .count();
        assert!(
            count > 0,
            "tile {id} ({}) is unused",
            TILES[id as usize].name
        );
    }
    // The ridge has a passable gap, so both halves of the map stay connected.
    assert_eq!(tile_at(25, 25), ROCK);
    assert_ne!(tile_at(31, 31), ROCK);
    assert_eq!(tile_at(40, 22), WATER);
    assert_eq!(tile_at(12, 12), ORE);
}
