//! RA3: the computer player after infantry sub cells and crushing (docs/DECISIONS.md R44).
//!
//! Three soldiers to a cell and crushing changed how battles end, and the 756 matches of the
//! shipped data showed a defect of the computer player that had been there since RA1: its last
//! armed unit, an armed harvester that fights when it has nothing to harvest, was sent back to
//! harvesting whenever it stood and fired. The regression test plays that situation on the RA2
//! fixture (`tests/fixtures/data/ra2`, whose Soviet miner is an armed harvester; invariant I10)
//! through `setup_match_configured`, `CommandQueue` and `SimSchedule`.

mod ra2_common;

use std::path::PathBuf;

use bevy_app::App;
use ra2_common::*;
use sim::{
    AiPlayers, Building, Cell, CellPos, Controller, HarvestState, Harvester, MatchOptions,
    MatchOutcome, MatchState, Owner, PlayerSetup, SimEvent, SimId, SimPlugin, Unit, load_ai,
    load_map, load_skirmish, setup_match_configured,
};

const ALPHA: usize = 0;
const GAMMA: usize = 2;

/// A match on the fixture map without ore: the Soviet computer player P0 (the second
/// difficulty) at the first start, a human player P1, who does nothing, at the second. Each
/// starts with its Construction Yard and no units.
fn start() -> App {
    let definitions = definitions();
    let skirmish = load_skirmish(fixture("skirmish.ron"), &definitions)
        .unwrap_or_else(|error| panic!("{error}"));
    let ai = load_ai(fixture("ai.ron"), &definitions).unwrap_or_else(|error| panic!("{error}"));
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 7 });
    let map =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/maps/skirmish_40x40.tmx");
    let mut grid = load_map(map).unwrap_or_else(|error| panic!("{error}")).grid;
    for y in 0..40 {
        for x in 0..40 {
            let at = CellPos::new(x, y);
            if let Some(&cell) = grid.cell(at)
                && cell.ore > 0
            {
                grid.set_terrain(at, Cell { ore: 0, ..cell });
            }
        }
    }
    app.world_mut().insert_resource(grid);
    app.world_mut().insert_resource(definitions);
    let players = [
        PlayerSetup {
            player: P0,
            faction: GAMMA,
            start: 0,
            controller: Controller::Ai { difficulty: 1 },
        },
        PlayerSetup {
            player: P1,
            faction: ALPHA,
            start: 1,
            controller: Controller::Human,
        },
    ];
    let options = MatchOptions {
        mcv_start: false,
        unit_count: Some(0),
        ..MatchOptions::default()
    };
    setup_match_configured(app.world_mut(), &skirmish, &ai, 0, &players, &options)
        .unwrap_or_else(|error| panic!("{error}"));
    app
}

fn buildings_of(app: &mut App, player: sim::PlayerId) -> Vec<SimId> {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &Owner, &Building)>();
    query
        .iter(world)
        .filter(|(_, owner, _)| owner.0 == player)
        .map(|(id, ..)| *id)
        .collect()
}

#[test]
fn an_armed_harvester_that_fights_as_a_last_resort_is_not_called_back_to_harvest() {
    // Regression (RA3, found by `m14_duels -- full` with the shipped data: Riverlands, Libya
    // against Great Britain, both brutal, 108,205 ticks). The computer player's last armed unit
    // was a War Miner with nothing to harvest. In the wave it has no move order whenever it
    // stands and fires; the computer player then ordered it to harvest, which ended its attack
    // and, since there was no ore, left it idle at the enemy's base, from where it was called
    // home to gather and wait for the next wave: a few shots for every wave's patience.
    let mut app = start();
    // The computer player is left with a refinery, a miner, and nothing that builds or
    // trains; the other player with a Construction Yard that takes 40 hits.
    let yard = buildings_of(&mut app, P0)[0];
    place(&mut app, P0, "soviet_refinery", 5, 10);
    spawn(&mut app, P0, "miner_s", 9, 10);
    set_hit_points(&mut app, yard, 0);
    let target = buildings_of(&mut app, P1)[0];
    set_hit_points(&mut app, target, 40);

    let mut joined = None;
    let mut shots = 0;
    let mut ended = None;
    for index in 0..6_000_u64 {
        let events = tick(&mut app, Vec::new());
        let miners: Vec<(SimId, HarvestState)> = {
            let world = app.world_mut();
            let mut query = world.query::<(&SimId, &Owner, &Unit, &Harvester)>();
            query
                .iter(world)
                .filter(|(_, owner, ..)| owner.0 == P0)
                .map(|(id, _, _, harvester)| (*id, harvester.state))
                .collect()
        };
        if index == 60 {
            assert_eq!(miners.len(), 1);
            assert_eq!(buildings_of(&mut app, P0).len(), 1, "the yard is gone");
        }
        let in_wave = |app: &App, id: SimId| {
            app.world()
                .resource::<AiPlayers>()
                .controller(P0)
                .expect("a controller")
                .wave()
                .contains(&id)
        };
        if let Some(&(miner, state)) = miners.first() {
            if joined.is_none() && in_wave(&app, miner) {
                joined = Some(index);
            }
            if joined.is_some() && app.world().resource::<MatchState>().outcome().is_none() {
                // Once it attacks, it attacks: it stays in the wave, and its cycle stays
                // stopped (a `Harvest` command would start it again).
                assert!(in_wave(&app, miner), "tick {index}: it left the wave");
                assert_eq!(state, HarvestState::Idle, "tick {index}");
            }
            shots += events
                .iter()
                .filter(
                    |event| matches!(event, SimEvent::WeaponFired { attacker, .. } if *attacker == miner),
                )
                .count();
        }
        if let Some(outcome) = app.world().resource::<MatchState>().outcome() {
            ended = Some((index, outcome));
            break;
        }
    }
    // The second difficulty of the fixture waits 600 + 800 ticks for its first wave.
    let joined = joined.expect("the miner joined a wave");
    assert!((1_400..1_500).contains(&joined), "joined in tick {joined}");
    let (tick, outcome) = ended.expect("the match ended");
    assert!(
        matches!(outcome, MatchOutcome::Won { winner, .. } if winner == P0),
        "{outcome:?}"
    );
    // One march to the other start, and 40 shots ten ticks apart: with the defect the miner
    // went home after at most 200 ticks of fire and came back a wave's patience later.
    assert_eq!(shots, 40);
    assert!(tick < joined + 1_000, "the match ended in tick {tick}");
}
