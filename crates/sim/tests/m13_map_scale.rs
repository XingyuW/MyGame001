//! Matches with more players than a map file has start positions (docs/DECISIONS.md, "Ten
//! players on maps that grow with them"), on fixture data (invariant I10).
//!
//! `src/map_scale.rs` tests the enlargement itself on made-up terrain. Here a match is set up on
//! the enlarged 40 x 40 skirmish fixture map, which lists two start positions, with `setup_match`
//! exactly as the game does it (the enlarged grid installed, the skirmish definitions given the
//! enlarged start positions), and runs only through `SimPlugin`: the computer players at the made
//! start positions build and harvest like those at the file's, and the match is deterministic.

use std::collections::BTreeMap;
use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    AiPlayers, Building, CellPos, Controller, MatchSetupError, MatchState, Owner, PlayerId,
    PlayerSetup, SimEvent, SimPlugin, SimSchedule, enlarged_map, enlarged_size, load_ai, load_map,
    load_match_definitions, load_skirmish, setup_match, state_hash,
};

fn fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(relative)
}

/// What a match of `players` computer players on the enlarged fixture map came to after `ticks`
/// ticks: the buildings each placed, the credits each brought in, and the state hash.
fn play(players: usize, ticks: u64) -> (BTreeMap<PlayerId, usize>, BTreeMap<PlayerId, i64>, u64) {
    let definitions = load_match_definitions(
        fixture("data/units_combat.ron"),
        fixture("data/economy_combat.ron"),
        fixture("data/combat.ron"),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let mut skirmish = load_skirmish(fixture("data/skirmish.ron"), &definitions)
        .unwrap_or_else(|error| panic!("{error}"));
    let ai =
        load_ai(fixture("data/ai.ron"), &definitions).unwrap_or_else(|error| panic!("{error}"));
    let file = load_map(fixture("maps/skirmish_40x40.tmx")).unwrap_or_else(|e| panic!("{e}"));
    let listed = skirmish.maps[0].starts.clone();
    assert_eq!(listed.len(), 2);

    let enlarged = enlarged_map(&file.grid, &file.looks, &listed, players)
        .unwrap_or_else(|error| panic!("{players} players: {error}"));
    assert_eq!(
        (enlarged.grid.width(), enlarged.grid.height()),
        enlarged_size(40, 40, players)
    );
    assert_eq!(enlarged.starts.len(), players);
    let setups: Vec<PlayerSetup> = (0..players)
        .map(|k| PlayerSetup {
            player: PlayerId(k as u8),
            faction: k % skirmish.factions.len(),
            start: k,
            controller: Controller::Ai { difficulty: 0 },
        })
        .collect();

    // On the map file itself the match has too many players for its two start positions.
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 5 });
    app.world_mut().insert_resource(file.grid.clone());
    app.world_mut().insert_resource(definitions.clone());
    assert_eq!(
        matches!(
            setup_match(app.world_mut(), &skirmish, &ai, 0, &setups),
            Err(MatchSetupError::Players(_))
        ),
        players > listed.len()
    );

    // On the enlarged map, with its start positions, it sets up.
    skirmish.maps[0].starts.clone_from(&enlarged.starts);
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 5 });
    app.world_mut().insert_resource(enlarged.grid.clone());
    app.world_mut().insert_resource(definitions);
    let start = setup_match(app.world_mut(), &skirmish, &ai, 0, &setups)
        .unwrap_or_else(|error| panic!("{players} players: {error}"));
    assert!(start.buildings >= players);
    let world = app.world_mut();
    assert_eq!(world.resource::<AiPlayers>().players().count(), players);
    assert_eq!(world.resource::<MatchState>().players().len(), players);
    // Every player's yard stands at its start position.
    let yards: Vec<(PlayerId, CellPos)> = world
        .query::<(&Owner, &Building)>()
        .iter(world)
        .map(|(owner, building)| (owner.0, building.footprint.origin))
        .collect();
    for (k, start) in enlarged.starts.iter().enumerate() {
        assert!(
            yards.contains(&(PlayerId(k as u8), *start)),
            "{players} players: player {k} at {start:?}: {yards:?}"
        );
    }

    let (mut placed, mut earned) = (BTreeMap::new(), BTreeMap::new());
    for _ in 0..ticks {
        world.run_schedule(SimSchedule);
        for event in world
            .resource::<Messages<SimEvent>>()
            .iter_current_update_messages()
        {
            match event {
                SimEvent::BuildingPlaced { player, .. } => {
                    *placed.entry(*player).or_insert(0) += 1;
                }
                SimEvent::OreDelivered {
                    player, credits, ..
                } => *earned.entry(*player).or_insert(0) += credits,
                _ => {}
            }
        }
    }
    (placed, earned, state_hash(world))
}

#[test]
fn computer_players_at_made_start_positions_build_and_harvest() {
    // Two to five players on the 40 x 40 fixture map with two start positions: up to three made
    // ones on a map of up to 56 x 56 cells.
    for players in 2..=5 {
        let (placed, earned, hash) = play(players, 1_500);
        for k in 0..players {
            let id = PlayerId(k as u8);
            assert!(
                placed.get(&id).copied().unwrap_or(0) >= 2,
                "{players} players: player {k} placed {placed:?}"
            );
            assert!(
                earned.get(&id).copied().unwrap_or(0) > 0,
                "{players} players: player {k} earned {earned:?}"
            );
        }
        // The same match again: the same state (invariant I3).
        assert_eq!(play(players, 1_500).2, hash, "{players} players");
    }
}
