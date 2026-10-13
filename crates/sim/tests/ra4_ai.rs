//! RA4: the computer player under the rules of RA4 (docs/RA2_PARITY_PLAN.md milestone RA4;
//! docs/DECISIONS.md R53): with units that are promoted and buildings that release survivors,
//! its matches end, it issues no command that is rejected for a reason of its own, and the
//! cells stay in order. A player that can neither fight nor earn makes its last stand: it sells
//! its buildings, so that a match of two players who have spent each other ends.
//!
//! End to end on the RA4 fixture (`tests/fixtures/data/ra4`; invariant I10), whose plans field
//! the trainable tank, heavy and rifleman and whose yards, barracks and factories are crewed,
//! on the fixture map, through `setup_match_configured`, `CommandQueue` and `SimSchedule`. The
//! matches of the shipped data are played by `crates/game/tests/m6_production_matches.rs` and
//! the `m14_duels` example. That a computer player does not see a hidden unit and does not
//! command a launched one is a unit test of `sim::ai`.

mod ra4_common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use bevy_app::App;
use ra4_common::*;
use sim::{
    AiDefinitions, Controller, Definitions, MatchOptions, MatchOutcome, MatchState, Owner,
    PlacementError, PlayerId, PlayerSetup, Players, RejectReason, SimEvent, SimId, SimPlugin,
    SkirmishDefinitions, load_ai, load_map, load_skirmish, setup_match_configured, state_hash,
};

const ALPHA: usize = 0;
const GAMMA: usize = 2;

struct Data {
    definitions: Definitions,
    skirmish: SkirmishDefinitions,
    ai: AiDefinitions,
}

fn data() -> Data {
    let definitions = definitions();
    let skirmish = load_skirmish(fixture("skirmish.ron"), &definitions)
        .unwrap_or_else(|error| panic!("{error}"));
    let ai = load_ai(fixture("ai.ron"), &definitions).unwrap_or_else(|error| panic!("{error}"));
    sim::check_ai_factions(fixture("ai.ron"), &ai, &skirmish, &definitions)
        .unwrap_or_else(|error| panic!("{error}"));
    Data {
        definitions,
        skirmish,
        ai,
    }
}

fn computer(player: PlayerId, faction: usize, start: usize, difficulty: usize) -> PlayerSetup {
    PlayerSetup {
        player,
        faction,
        start,
        controller: Controller::Ai { difficulty },
    }
}

fn start(data: &Data, players: &[PlayerSetup]) -> App {
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 7 });
    let map =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/maps/skirmish_40x40.tmx");
    let grid = load_map(map).unwrap_or_else(|error| panic!("{error}")).grid;
    app.world_mut().insert_resource(grid);
    app.world_mut().insert_resource(data.definitions.clone());
    setup_match_configured(
        app.world_mut(),
        &data.skirmish,
        &data.ai,
        0,
        players,
        &MatchOptions::default(),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    app
}

/// What a match of two computer players showed.
#[derive(Debug, PartialEq, Eq)]
struct Played {
    outcome: Option<MatchOutcome>,
    ticks: u64,
    hash: u64,
    /// Rejected commands that the losses of the tick before do not explain (D43).
    unexplained: Vec<SimEvent>,
    promotions: usize,
    survivors: usize,
}

/// Runs a match of two computer players until it ends or `limit` ticks passed, checking in
/// every tick that no credits are negative (I7) and every 16th tick that the cells are in
/// order (I6').
fn play(data: &Data, players: &[PlayerSetup], limit: u64) -> Played {
    let mut app = start(data, players);
    let mut unexplained = Vec::new();
    let mut lost_before: BTreeMap<PlayerId, Vec<SimId>> = BTreeMap::new();
    let (mut ticks, mut promotions, mut survivors) = (0, 0, 0);
    while ticks < limit && app.world().resource::<MatchState>().outcome().is_none() {
        let events = tick(&mut app, Vec::new());
        for (player, state) in app.world().resource::<Players>().iter() {
            assert!(state.credits() >= 0, "player {} at tick {ticks}", player.0);
        }
        if ticks % 16 == 0 {
            assert_cells(&mut app, &format!("tick {ticks}"));
        }
        let mut lost: BTreeMap<PlayerId, Vec<SimId>> = BTreeMap::new();
        for event in &events {
            match event {
                SimEvent::UnitDestroyed { player, unit } => {
                    lost.entry(*player).or_default().push(*unit);
                }
                SimEvent::BuildingDestroyed {
                    player, building, ..
                } => lost.entry(*player).or_default().push(*building),
                SimEvent::RankGained { .. } => promotions += 1,
                SimEvent::CrewEscaped { units, .. } => survivors += units.len(),
                _ => {}
            }
        }
        for event in events {
            let SimEvent::CommandRejected { player, reason, .. } = &event else {
                continue;
            };
            let losses = lost_before.get(player).map_or(&[][..], Vec::as_slice);
            let explained = match reason {
                RejectReason::UnknownUnit(id) | RejectReason::UnknownBuilding(id) => {
                    losses.contains(id)
                }
                RejectReason::MissingPrerequisite { .. }
                | RejectReason::NoProducer { .. }
                | RejectReason::NoFactoryOfSide(_)
                | RejectReason::Placement(PlacementError::NotVisible(_))
                | RejectReason::Placement(PlacementError::TooFar { .. }) => !losses.is_empty(),
                RejectReason::Placement(PlacementError::Occupied { unit, .. }) => {
                    let mut owners = app.world_mut().query::<(&SimId, &Owner)>();
                    owners
                        .iter(app.world())
                        .any(|(id, owner)| id == unit && owner.0 != *player)
                }
                // In particular never a unit that takes no commands or a missing dock.
                _ => false,
            };
            if !explained {
                unexplained.push(event);
            }
        }
        lost_before = lost;
        ticks += 1;
    }
    Played {
        outcome: app.world().resource::<MatchState>().outcome(),
        ticks,
        hash: state_hash(app.world_mut()),
        unexplained,
        promotions,
        survivors,
    }
}

/// Regression (RA4): with survivors that fight on, the two brutal players of the fixture spend
/// each other by tick 4,400: the Allied one is left with a power plant, two bunkers and two
/// idle miners, the Soviet one with a factory and no credits. Neither could ever change
/// anything, and before the last stand the match had no winner after 72,000 ticks. Now the
/// Soviet player, which has less to sell, is out first, and the match ends in a few thousand
/// ticks.
#[test]
fn two_players_who_have_spent_each_other_make_their_last_stand_and_the_match_ends() {
    let data = data();
    let players = [computer(P0, ALPHA, 0, 1), computer(P1, GAMMA, 1, 1)];
    let played = play(&data, &players, 72_000);
    assert!(
        matches!(played.outcome, Some(MatchOutcome::Won { .. })),
        "{:?} after {} ticks",
        played.outcome,
        played.ticks
    );
    assert!(played.ticks < 20_000, "{} ticks", played.ticks);
    assert!(played.unexplained.is_empty(), "{:?}", played.unexplained);
    // It waited out its patience first: the players had spent each other by tick 4,400.
    assert!(played.ticks > 4_400 + sim::LAST_STAND_PATIENCE);
}

#[test]
fn computer_players_finish_their_matches_with_veterans_and_survivors() {
    let data = data();
    let (mut promotions, mut survivors) = (0, 0);
    for (first, second) in [(ALPHA, GAMMA), (GAMMA, ALPHA), (GAMMA, GAMMA)] {
        for (easy, hard) in [(0, 1), (1, 0), (1, 1)] {
            let players = [computer(P0, first, 0, easy), computer(P1, second, 1, hard)];
            let name = format!("factions {first} and {second}, difficulties {easy} and {hard}");
            let played = play(&data, &players, 72_000);
            assert!(
                matches!(played.outcome, Some(MatchOutcome::Won { .. })),
                "{name}: {:?} after {} ticks",
                played.outcome,
                played.ticks
            );
            assert!(
                played.unexplained.is_empty(),
                "{name}: {:?}",
                played.unexplained
            );
            promotions += played.promotions;
            survivors += played.survivors;
            assert_eq!(play(&data, &players, 72_000), played, "{name}");
        }
    }
    // The rules of RA4 are at work in these matches: units rise in rank, and the buildings
    // that fall release their survivors.
    assert!(promotions > 0, "no unit was promoted in nine matches");
    assert!(survivors > 0, "no survivor left a building in nine matches");
}
