//! RA5 regression: regrowing ore must not leave equal ground armies trading tiny waves
//! beyond the existing shipped-match limit (R64). Uses shipped data and SimSchedule.

use std::path::PathBuf;

use bevy_app::App;
use sim::{
    AiPlayers, Building, Controller, MatchOutcome, MatchState, Mobile, Owner, PlayerId,
    PlayerSetup, Players, SimPlugin, SimSchedule, Unit, load_ai, load_map, load_ruleset,
    load_skirmish, map_asset_path, setup_match,
};

#[test]
fn equal_brutal_armies_finish_thousand_lakes_with_regrowing_ore() {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let data = assets.join("data");
    let definitions = load_ruleset(&data).expect("shipped rules");
    let skirmish = load_skirmish(data.join("skirmish.ron"), &definitions).expect("skirmish");
    let ai = load_ai(data.join("ai.ron"), &definitions).expect("AI");
    let map = skirmish
        .maps
        .iter()
        .position(|m| m.id == "thousand_lakes")
        .expect("map");
    let grid = load_map(assets.join(map_asset_path(&skirmish.maps[map].id)))
        .expect("map")
        .grid;
    let limit = 72_000 * u64::from(grid.width() + grid.height()).max(128) / 128;
    let difficulty = ai
        .difficulties
        .iter()
        .position(|d| d.id == "brutal")
        .expect("brutal");
    let players: Vec<_> = ["france", "germany"]
        .iter()
        .enumerate()
        .map(|(i, country)| PlayerSetup {
            player: PlayerId(i as u8),
            faction: skirmish
                .factions
                .iter()
                .position(|f| f.id == *country)
                .expect("country"),
            start: i,
            controller: Controller::Ai { difficulty },
        })
        .collect();
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 0 });
    app.world_mut().insert_resource(grid);
    app.world_mut().insert_resource(definitions.clone());
    setup_match(app.world_mut(), &skirmish, &ai, map, &players).expect("match");
    let mut ended = None;
    for tick in 0..limit {
        let world = app.world_mut();
        world.run_schedule(SimSchedule);
        if tick % 12_000 == 0 {
            for player in [PlayerId(0), PlayerId(1)] {
                let mut query = world.query::<(&Owner, &Unit, &Mobile)>();
                let mut army = std::collections::BTreeMap::new();
                for (owner, unit, mobile) in query.iter(world).filter(|(o, _, _)| o.0 == player) {
                    let _ = owner;
                    let id = &definitions.unit(unit.kind).expect("unit").id;
                    army.entry(id.clone())
                        .or_insert_with(Vec::new)
                        .push(mobile.cell);
                }
                let mut query = world.query::<(&Owner, &Building)>();
                let buildings = query.iter(world).filter(|(o, _)| o.0 == player).count();
                let controller = world
                    .resource::<AiPlayers>()
                    .controller(player)
                    .expect("controller");
                eprintln!(
                    "tick {tick}, {player:?}: {buildings} buildings, {} credits, wave {}, target {:?}, army {army:?}",
                    world
                        .resource::<Players>()
                        .get(player)
                        .expect("player")
                        .credits(),
                    controller.wave().len(),
                    controller.target()
                );
            }
        }
        if let Some(outcome) = world.resource::<MatchState>().outcome() {
            ended = Some(outcome);
            eprintln!("ended at tick {tick}: {outcome:?}");
            break;
        }
    }
    assert!(
        matches!(ended, Some(MatchOutcome::Won { .. })),
        "no winner within {limit} ticks"
    );
}
