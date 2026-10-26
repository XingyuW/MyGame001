//! RA2: the computer player under the rules of RA2 (docs/RA2_PARITY_PLAN.md milestone RA2;
//! docs/DECISIONS.md R28): it builds one building at a time in each lane of its yard, uses both
//! lanes, places by the reach of each building, and its matches end.
//!
//! End to end on the RA2 fixture (`tests/fixtures/data/ra2`; invariant I10) and the fixture map,
//! through `setup_match_configured`, `CommandQueue` and `SimSchedule`. The Allied plan of the
//! fixture lists a defence (the bunker) among its buildings. The matches of the shipped data are
//! played by `crates/game/tests/m6_production_matches.rs` and the `m14_duels` example.

mod ra2_common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use bevy_app::App;
use ra2_common::*;
use sim::{
    AiDefinitions, Controller, Definitions, Lane, MatchOptions, MatchOutcome, MatchState, Owner,
    PlacementError, PlayerId, PlayerSetup, Players, ProductionItem, RejectReason, SimEvent, SimId,
    SimPlugin, SkirmishDefinitions, load_ai, load_map, load_skirmish, setup_match_configured,
    state_hash,
};

const ALPHA: usize = 0;
const BETA: usize = 1;
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
    /// Whether P0 had a building in production in the main lane and in the lane of the
    /// defences in one tick.
    both_lanes: bool,
}

/// Runs a match of two computer players until it ends or `limit` ticks passed, checking in
/// every tick that no credits are negative (I7) and that a lane of buildings holds one item.
fn play(data: &Data, players: &[PlayerSetup], limit: u64) -> Played {
    let mut app = start(data, players);
    let mut unexplained = Vec::new();
    let mut lost_before: BTreeMap<PlayerId, Vec<SimId>> = BTreeMap::new();
    let (mut ticks, mut both_lanes) = (0, false);
    while ticks < limit && app.world().resource::<MatchState>().outcome().is_none() {
        let events = tick(&mut app, Vec::new());
        for (player, state) in app.world().resource::<Players>().iter() {
            assert!(state.credits() >= 0, "player {} at tick {ticks}", player.0);
            let mut lanes = Vec::new();
            for (key, items) in state.queues() {
                if matches!(items[0].item, ProductionItem::Building(_)) {
                    assert_eq!(items.len(), 1, "{key:?} at tick {ticks}");
                    lanes.push(key.lane);
                }
            }
            both_lanes |= player == P0 && lanes == [Lane::Main, Lane::Defences];
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
                // In particular never a full lane or a build limit.
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
        both_lanes,
    }
}

#[test]
fn computer_players_finish_their_matches_under_the_rules_of_ra2() {
    // Every pairing of the two sides and of the two difficulties ends with a winner, without
    // a rejected command that the fighting does not explain, and the same way when it is
    // played again. An Allied player builds its bunkers beside its other buildings.
    let data = data();
    for (first, second) in [(ALPHA, GAMMA), (GAMMA, BETA), (ALPHA, BETA), (GAMMA, GAMMA)] {
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
            assert_eq!(
                played.both_lanes,
                first != GAMMA,
                "{name}: the Allied plan has a defence, the Soviet one has none"
            );
            assert_eq!(play(&data, &players, 72_000), played, "{name}");
        }
    }
}

#[test]
fn a_computer_player_places_its_buildings_within_their_reach_of_a_base_building() {
    // After its build order every building of an Allied computer player stands within the
    // reach of its type from a base building that is not itself.
    let data = data();
    let players = [computer(P0, ALPHA, 0, 1), computer(P1, GAMMA, 1, 0)];
    let mut app = start(&data, &players);
    let mut placed = 0;
    for _ in 0..3_000 {
        let events = tick(&mut app, Vec::new());
        for event in events {
            let SimEvent::BuildingPlaced {
                player,
                building,
                kind,
            } = event
            else {
                continue;
            };
            if player != P0 {
                continue;
            }
            let mut query = app.world_mut().query::<(&SimId, &Owner, &sim::Building)>();
            let own: Vec<(SimId, sim::BuildingKind, sim::Footprint)> = query
                .iter(app.world())
                .filter(|(_, owner, _)| owner.0 == P0)
                .map(|(id, _, building)| (*id, building.kind, building.footprint))
                .collect();
            let Some(&(_, _, footprint)) = own.iter().find(|(id, ..)| *id == building) else {
                continue;
            };
            let nearest = own
                .iter()
                .filter(|(id, other, _)| {
                    *id != building && sim::extends_base(&data.definitions, *other)
                })
                .map(|(_, _, other)| footprint.distance(other))
                .min();
            // The deployed yard is the first building, with nothing to be near to.
            if let Some(nearest) = nearest {
                assert!(
                    nearest <= sim::placement_reach(&data.definitions, kind),
                    "{kind:?}: {nearest} cells from the base"
                );
                placed += 1;
            }
        }
    }
    assert!(placed >= 5, "{placed} buildings placed");
}

// --- Super Me (a match option, so tested through the match setup) -----------------------------------

#[test]
fn super_me_multiplies_the_rate_and_pays_one_credit() {
    // A human player with Super Me and its yard: a power building of 300 credits and 20 ticks
    // advances `SUPER_ME_PRODUCTION_FACTOR` (30) units a tick, so it is ready in the first tick,
    // for 1 credit charged with the last unit. With three yards: 46.86 units a tick, and still
    // 1 credit.
    let data = data();
    for (yards, fraction) in [(1, 0), (3, 860)] {
        let mut app = App::new();
        app.add_plugins(SimPlugin { seed: 7 });
        let map = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/maps/skirmish_40x40.tmx");
        let grid = load_map(map).unwrap_or_else(|error| panic!("{error}")).grid;
        app.world_mut().insert_resource(grid);
        app.world_mut().insert_resource(data.definitions.clone());
        let players = [
            PlayerSetup {
                player: P0,
                faction: ALPHA,
                start: 0,
                controller: Controller::Human,
            },
            PlayerSetup {
                player: P1,
                faction: GAMMA,
                start: 1,
                controller: Controller::Human,
            },
        ];
        let options = MatchOptions {
            super_players: vec![P0],
            mcv_start: false,
            ..MatchOptions::default()
        };
        setup_match_configured(
            app.world_mut(),
            &data.skirmish,
            &data.ai,
            0,
            &players,
            &options,
        )
        .unwrap_or_else(|error| panic!("{error}"));
        for _ in 1..yards {
            // Further yards on the first free buildable ground of the fixture map.
            let spot = (0..40)
                .flat_map(|y| (0..40).map(move |x| sim::CellPos::new(x, y)))
                .find(|&cell| {
                    sim::spawn_building(app.world_mut(), P0, "allied_yard", cell).is_ok()
                });
            assert!(spot.is_some(), "ground for a yard");
        }
        let before = credits(&app, P0);
        let power = building(&app, "allied_power");
        assert_eq!(rejections(&tick(&mut app, vec![queue(P0, power)])), NONE);
        let head = lane_of(&app, P0, power)[0];
        assert_eq!(
            (head.progress, head.fraction, head.charged),
            (20, fraction, 1),
            "{yards} yards"
        );
        assert_eq!(credits(&app, P0), before - 1);
    }
}
