//! Repeatable RA8 rule-copy cost and unchanged seeded match digest measurement.
use bevy_app::App;
use sim::*;
use std::{hint::black_box, path::PathBuf, time::Instant};

fn main() {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let data = assets.join("data");
    let definitions = load_ruleset(&data).expect("rules");
    let start = Instant::now();
    for _ in 0..20_000 {
        black_box(definitions.clone());
    }
    println!("20000 immutable definition copies: {:?}", start.elapsed());
    let skirmish = load_skirmish(data.join("skirmish.ron"), &definitions).expect("skirmish");
    let ai = load_ai(data.join("ai.ron"), &definitions).expect("ai");
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 8 });
    app.world_mut().insert_resource(
        load_map(assets.join(map_asset_path(&skirmish.maps[0].id)))
            .expect("map")
            .grid,
    );
    app.world_mut().insert_resource(definitions);
    setup_match(
        app.world_mut(),
        &skirmish,
        &ai,
        0,
        &[
            PlayerSetup {
                player: PlayerId(0),
                faction: 0,
                start: 0,
                controller: Controller::Ai { difficulty: 1 },
            },
            PlayerSetup {
                player: PlayerId(1),
                faction: 5,
                start: 1,
                controller: Controller::Ai { difficulty: 1 },
            },
        ],
    )
    .expect("match");
    let start = Instant::now();
    let mut digest = 0u64;
    for _ in 0..5000 {
        app.world_mut().run_schedule(SimSchedule);
        digest = digest.rotate_left(7) ^ state_hash(app.world_mut());
    }
    println!(
        "5000 seeded ticks incl hash: {:?}; digest {digest:016x}",
        start.elapsed()
    );
}
