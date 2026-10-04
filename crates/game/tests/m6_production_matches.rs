//! M6: computer players against each other with the shipped data on every shipped map (PLAN.md
//! section 7, M6; docs/DECISIONS.md D43).
//!
//! `crates/sim/tests/m6_ai.rs` runs the plan's end-to-end test on every shipped map with fixture
//! data (invariant I10), and checks equal hashes across two runs. This test runs the shipped
//! factions, units, buildings, and AI file, as a match started from the menu sets them up, once
//! for every pairing of the shipped difficulties: every match must end with a winner within
//! [`MATCH_TICK_LIMIT`] ticks, without panic, and with no rejected command of a computer player
//! that a loss in the tick before does not explain. It checks a property of the shipped game
//! (that its computer players finish their matches), so a change of the balance in M7 that breaks
//! it is meant to fail it. Every match is bounded in time by the tick limit and in memory by the AI
//! file's `max_army`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use bevy::ecs::message::Messages;
use bevy::ecs::world::World;
use sim::{
    CommandQueue, Controller, MatchOutcome, MatchState, PlayerId, PlayerSetup, RejectReason,
    SimEvent, SimId, SimPlugin, SimSchedule, load_ai, load_map, load_match_definitions,
    load_skirmish, map_asset_path, setup_match,
};

/// 30 minutes of game time.
const MATCH_TICK_LIMIT: u64 = 36_000;

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

#[test]
fn computer_players_finish_matches_with_the_shipped_data_on_every_shipped_map() {
    let data = assets().join("data");
    let definitions = load_match_definitions(
        data.join("units.ron"),
        data.join("economy.ron"),
        data.join("combat.ron"),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let skirmish =
        load_skirmish(data.join("skirmish.ron"), &definitions).unwrap_or_else(|e| panic!("{e}"));
    let ai = load_ai(data.join("ai.ron"), &definitions).unwrap_or_else(|e| panic!("{e}"));
    for (map, def) in skirmish.maps.iter().enumerate() {
        let grid = load_map(assets().join(map_asset_path(&def.id)))
            .unwrap_or_else(|error| panic!("{error}"))
            .grid;
        for first in 0..ai.difficulties.len() {
            for second in 0..ai.difficulties.len() {
                let name = format!(
                    "{}: {} against {}",
                    def.id, ai.difficulties[first].id, ai.difficulties[second].id
                );
                let mut app = bevy::app::App::new();
                app.add_plugins(SimPlugin { seed: 0 });
                app.world_mut().insert_resource(grid.clone());
                app.world_mut().insert_resource(definitions.clone());
                let players = [
                    PlayerSetup {
                        player: PlayerId(0),
                        faction: 0,
                        start: 0,
                        controller: Controller::Ai { difficulty: first },
                    },
                    PlayerSetup {
                        player: PlayerId(1),
                        faction: 0,
                        start: 1,
                        controller: Controller::Ai { difficulty: second },
                    },
                ];
                setup_match(app.world_mut(), &skirmish, &ai, map, &players)
                    .unwrap_or_else(|error| panic!("{name}: {error}"));
                let outcome = run(app.world_mut(), &name);
                assert!(
                    matches!(outcome, Some(MatchOutcome::Won { .. })),
                    "{name}: {outcome:?}"
                );
            }
        }
    }
}

/// Runs the match until it ends or the tick limit is reached; returns its outcome. Every rejected
/// command must name a unit or building its player lost in the tick before, or be a production or
/// placement command of a player who lost a unit or building then (docs/DECISIONS.md D43).
fn run(world: &mut World, name: &str) -> Option<MatchOutcome> {
    let mut losses: BTreeMap<PlayerId, BTreeSet<SimId>> = BTreeMap::new();
    for at in 0..MATCH_TICK_LIMIT {
        world.run_schedule(SimSchedule);
        let events: Vec<SimEvent> = world
            .resource::<Messages<SimEvent>>()
            .iter_current_update_messages()
            .cloned()
            .collect();
        let before = std::mem::take(&mut losses);
        for event in events {
            match event {
                SimEvent::CommandRejected {
                    player,
                    command,
                    reason,
                } => {
                    let lost = before.get(&player);
                    let explained = match reason {
                        RejectReason::UnknownUnit(id) | RejectReason::UnknownBuilding(id) => {
                            lost.is_some_and(|lost| lost.contains(&id))
                        }
                        RejectReason::MissingPrerequisite { .. }
                        | RejectReason::NoProducer { .. }
                        | RejectReason::Placement(_) => lost.is_some_and(|lost| !lost.is_empty()),
                        _ => false,
                    };
                    assert!(
                        explained,
                        "{name}: tick {at}: {command:?} rejected ({reason}) without a loss"
                    );
                }
                SimEvent::UnitDestroyed { player, unit } => {
                    losses.entry(player).or_default().insert(unit);
                }
                SimEvent::BuildingDestroyed {
                    player, building, ..
                } => {
                    losses.entry(player).or_default().insert(building);
                }
                _ => {}
            }
        }
        // The computer players push a bounded number of commands per turn.
        assert!(
            world.resource::<CommandQueue>().len() <= 32,
            "{name}: tick {at}: {} commands",
            world.resource::<CommandQueue>().len()
        );
        if let Some(outcome) = world.resource::<MatchState>().outcome() {
            return Some(outcome);
        }
    }
    None
}
