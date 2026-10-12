//! M6: computer players against each other with the shipped data on every shipped map (PLAN.md
//! section 7, M6; docs/DECISIONS.md D43).
//!
//! `crates/sim/tests/m6_ai.rs` runs the plan's end-to-end test on every shipped map with fixture
//! data (invariant I10), and checks equal hashes across two runs. This test runs the shipped
//! countries, units, buildings, and AI file, as a match started from the menu sets them up (each
//! player with the base unit of its side and its starting units), once for every pairing of the
//! shipped difficulties and of the two sides; the country that plays a side rotates with the map
//! and the pairing, so that every country is played on several maps (RA1, docs/DECISIONS.md R5:
//! every pairing of the nine countries would be 15,309 matches instead of 756). Every match must
//! end with a winner within [`MATCH_TICK_LIMIT`] ticks
//! (scaled up on large maps, [`tick_limit`]), without panic, and with no rejected command of a
//! computer player that a loss in the tick before, or another player's unit stepping into a
//! footprint, does not explain. It checks a property of the shipped game
//! (that its computer players finish their matches), so a change of the balance in M7 that breaks
//! it is meant to fail it. Every match is bounded in time by the tick limit and in memory by the AI
//! file's `max_army`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use bevy::ecs::message::Messages;
use bevy::ecs::world::World;
use sim::{
    CommandQueue, Controller, MatchOutcome, MatchState, PlayerId, PlayerSetup, RejectReason,
    SimEvent, SimId, SimPlugin, SimSchedule, SkirmishDefinitions, load_ai, load_map, load_ruleset,
    load_skirmish, map_asset_path, setup_match,
};

/// 30 minutes of game time, on maps up to `sim::ARMY_SCALE_SPAN` cells wide and high together;
/// larger maps get proportionally longer, as their armies travel up to twice as far
/// ([`tick_limit`]).
const MATCH_TICK_LIMIT: u64 = 36_000;

/// The tick limit of a match on `grid`.
fn tick_limit(grid: &sim::Grid) -> u64 {
    let span = u64::from(grid.width() + grid.height()).max(u64::from(sim::ARMY_SCALE_SPAN));
    MATCH_TICK_LIMIT * span / u64::from(sim::ARMY_SCALE_SPAN)
}

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

#[test]
fn computer_players_finish_matches_with_the_shipped_data_on_every_shipped_map() {
    let data = assets().join("data");
    let definitions = load_ruleset(&data).unwrap_or_else(|error| panic!("{error}"));
    let skirmish =
        load_skirmish(data.join("skirmish.ron"), &definitions).unwrap_or_else(|e| panic!("{e}"));
    let ai = load_ai(data.join("ai.ron"), &definitions).unwrap_or_else(|e| panic!("{e}"));
    sim::check_ai_factions(data.join("ai.ron"), &ai, &skirmish, &definitions)
        .unwrap_or_else(|e| panic!("{e}"));
    // RA1 ships two sides of five and four countries (before: the two factions of M7).
    let groups = skirmish.pairing_groups();
    assert_eq!(groups.len(), 2, "two sides");
    assert_eq!(
        groups.iter().map(Vec::len).sum::<usize>(),
        skirmish.factions.len()
    );
    let factions: Vec<(usize, usize)> = (0..groups.len())
        .flat_map(|first| (0..groups.len()).map(move |second| (first, second)))
        .collect();
    let difficulties = ai.difficulties.len();
    // The matches are independent of each other, so they are played on all cores; a failed
    // assertion of any of them fails the test when the workers are joined.
    let mut jobs = Vec::new();
    for (map, def) in skirmish.maps.iter().enumerate() {
        let grid = load_map(assets().join(map_asset_path(&def.id)))
            .unwrap_or_else(|error| panic!("{error}"))
            .grid;
        for &(first_side, second_side) in &factions {
            for first in 0..difficulties {
                for second in 0..difficulties {
                    // The number of the pairing among all pairings of sides and difficulties
                    // (`m14_duels -- full` counts them the same way and plays the same matches).
                    let pairing = ((first_side * groups.len() + second_side) * difficulties
                        + first)
                        * difficulties
                        + second;
                    let country = |side: usize, slot: usize| {
                        SkirmishDefinitions::rotating(&groups[side], map + pairing + slot)
                            .expect("a country of the side")
                    };
                    jobs.push((
                        map,
                        grid.clone(),
                        [
                            (country(first_side, 0), first),
                            (country(second_side, 1), second),
                        ],
                    ));
                }
            }
        }
    }
    // Every country is played, as the first and as the second player.
    for slot in 0..2 {
        let played: BTreeSet<usize> = jobs.iter().map(|(_, _, sides)| sides[slot].0).collect();
        assert_eq!(played.len(), skirmish.factions.len(), "slot {slot}");
    }
    assert_eq!(
        jobs.len(),
        skirmish.maps.len() * factions.len() * ai.difficulties.len() * ai.difficulties.len()
    );
    let next = std::sync::atomic::AtomicUsize::new(0);
    let played = std::sync::atomic::AtomicUsize::new(0);
    let workers = std::thread::available_parallelism().map_or(4, usize::from);
    std::thread::scope(|scope| {
        for _ in 0..workers.min(jobs.len()) {
            scope.spawn(|| {
                while let Some((map, grid, sides)) =
                    jobs.get(next.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
                {
                    let name = format!(
                        "{}: {} ({}) against {} ({})",
                        skirmish.maps[*map].id,
                        skirmish.factions[sides[0].0].id,
                        ai.difficulties[sides[0].1].id,
                        skirmish.factions[sides[1].0].id,
                        ai.difficulties[sides[1].1].id
                    );
                    let mut app = bevy::app::App::new();
                    app.add_plugins(SimPlugin { seed: 0 });
                    app.world_mut().insert_resource(grid.clone());
                    app.world_mut().insert_resource(definitions.clone());
                    let players = [0, 1].map(|k| PlayerSetup {
                        player: PlayerId(k as u8),
                        faction: sides[k].0,
                        start: k,
                        controller: Controller::Ai {
                            difficulty: sides[k].1,
                        },
                    });
                    setup_match(app.world_mut(), &skirmish, &ai, *map, &players)
                        .unwrap_or_else(|error| panic!("{name}: {error}"));
                    let outcome = run(app.world_mut(), &name, tick_limit(grid));
                    assert!(
                        matches!(outcome, Some(MatchOutcome::Won { .. })),
                        "{name}: {outcome:?}"
                    );
                    played.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            });
        }
    });
    assert_eq!(played.into_inner(), jobs.len());
}

/// Runs the match until it ends or the tick limit is reached; returns its outcome. Every rejected
/// command must name a unit or building its player lost in the tick before, or be a production or
/// placement command of a player who lost a unit or building then (docs/DECISIONS.md D43).
fn run(world: &mut World, name: &str, limit: u64) -> Option<MatchOutcome> {
    let mut losses: BTreeMap<PlayerId, BTreeSet<SimId>> = BTreeMap::new();
    for at in 0..limit {
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
                        // A unit of another player that the placing player could not see
                        // beside the footprint stepped into it (docs/DECISIONS.md D43, and
                        // `find_placement` in `crates/sim/src/ai.rs`): on the large maps the
                        // armies arrive at a base that is still building, which the small ones
                        // never showed in these matches.
                        RejectReason::Placement(sim::PlacementError::Occupied { unit, .. }) => {
                            let mut owners = world.query::<(&SimId, &sim::Owner)>();
                            lost.is_some_and(|lost| !lost.is_empty())
                                || owners
                                    .iter(world)
                                    .any(|(id, owner)| *id == unit && owner.0 != player)
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
    eprintln!(
        "{name} stalled: players {:?}",
        world
            .resource::<sim::Players>()
            .iter()
            .map(|(p, s)| (p, s.credits()))
            .collect::<Vec<_>>()
    );
    for (id, o, b) in world
        .query::<(&SimId, &sim::Owner, &sim::Building)>()
        .iter(world)
    {
        eprintln!(
            "building {id:?} {o:?} {:?} {:?}",
            b.kind, b.footprint.origin
        );
    }
    for (id, o, m, h) in world
        .query::<(&SimId, &sim::Owner, &sim::Mobile, Option<&sim::Harvester>)>()
        .iter(world)
    {
        eprintln!("unit {id:?} {o:?} {:?} {h:?}", m.cell);
    }
    None
}

#[test]
fn crossroads_armies_finish_after_new_terrain_changes() {
    let data = assets().join("data");
    let d = load_ruleset(&data).unwrap();
    let sk = load_skirmish(data.join("skirmish.ron"), &d).unwrap();
    let ai = load_ai(data.join("ai.ron"), &d).unwrap();
    let mut app = bevy::app::App::new();
    app.add_plugins(SimPlugin { seed: 0 });
    app.world_mut().insert_resource(
        load_map(assets().join("maps/crossroads/crossroads.tmx"))
            .unwrap()
            .grid,
    );
    app.world_mut().insert_resource(d);
    // The second faction against itself, both easy: before RA1 the Meridian, now the first
    // country of the second (the Soviet) side.
    let faction = sk.pairing_groups()[1][0];
    let players = [
        PlayerSetup {
            player: PlayerId(0),
            faction,
            start: 0,
            controller: Controller::Ai { difficulty: 0 },
        },
        PlayerSetup {
            player: PlayerId(1),
            faction,
            start: 1,
            controller: Controller::Ai { difficulty: 0 },
        },
    ];
    setup_match(app.world_mut(), &sk, &ai, 1, &players).unwrap();
    let outcome = run(app.world_mut(), "crossroads regression", MATCH_TICK_LIMIT);
    assert!(matches!(outcome, Some(MatchOutcome::Won { .. })));
}
