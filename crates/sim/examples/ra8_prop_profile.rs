//! Repeatable default ten-player Frostline digest and measured tick cost for R98.
use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::*;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

fn main() {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let data = assets.join("data");
    let definitions = load_ruleset(&data).expect("rules");
    let mut skirmish = load_skirmish(data.join("skirmish.ron"), &definitions).expect("skirmish");
    let ai = load_ai(data.join("ai.ron"), &definitions).expect("ai");
    let map = skirmish
        .maps
        .iter()
        .position(|m| m.id == "frostline")
        .expect("Frostline");
    let file = load_map(assets.join(map_asset_path("frostline"))).expect("map");
    let enlarged = enlarged_map(
        &file.grid,
        &file.looks,
        &skirmish.maps[map].starts,
        MAX_MATCH_PLAYERS,
    )
    .expect("enlarged map");
    skirmish.maps[map].starts.clone_from(&enlarged.starts);
    let players: Vec<_> = (0..MAX_MATCH_PLAYERS)
        .map(|k| PlayerSetup {
            player: PlayerId(k as u8),
            faction: k % skirmish.factions.len(),
            start: k,
            controller: Controller::Ai {
                difficulty: ai.difficulties.len() / 2,
            },
        })
        .collect();
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 13 });
    app.world_mut().insert_resource(enlarged.grid);
    app.world_mut().insert_resource(definitions);
    setup_match(app.world_mut(), &skirmish, &ai, map, &players).expect("match");
    let (mut state_digest, mut event_digest) = (0u64, 0xcbf29ce484222325u64);
    let mut event_count = 0u64;
    let mut measured = Duration::ZERO;
    let started = Instant::now();
    for _ in 0..5000 {
        let tick = Instant::now();
        app.world_mut().run_schedule(SimSchedule);
        measured += tick.elapsed();
        state_digest = state_digest.rotate_left(7) ^ state_hash(app.world_mut());
        for event in app
            .world()
            .resource::<Messages<SimEvent>>()
            .iter_current_update_messages()
        {
            event_count += 1;
            for byte in format!("{event:?}\n").bytes() {
                event_digest = (event_digest ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
    }
    println!(
        "Frostline seed13 default ten players, 5000 ticks; props{}; measured {measured:?}; mean {:?}; elapsed {:?}; state {state_digest:016x}; events {event_count}/{event_digest:016x}",
        app.world().resource::<Props>().len(),
        measured / 5000,
        started.elapsed()
    );
}
