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
//!
//! One difference from a match of the game: the scenery (`sim::Props`, the trees, houses and
//! towers of a map's look, which take damage since RA3) is installed by the game's host from
//! the map's presentation data and is not part of these matches, nor of `m14_duels`, `m7_soak`
//! and `m13_ten_players` (advisory review 1, tests P2-14; docs/DECISIONS.md R56, debt).

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use bevy::ecs::message::Messages;
use bevy::ecs::world::World;
use sim::{
    CommandQueue, Controller, MatchOutcome, MatchState, PlayerId, PlayerSetup, RejectReason,
    SimEvent, SimId, SimPlugin, SimSchedule, SkirmishDefinitions, load_ai, load_map, load_ruleset,
    load_skirmish, map_asset_path, setup_match,
};

/// 60 minutes of game time, on maps up to `sim::ARMY_SCALE_SPAN` cells wide and high together;
/// larger maps get proportionally longer, as their armies travel up to twice as far
/// ([`tick_limit`]).
///
/// Before RA2 it was 36,000. RA2 gives every item the build time of Red Alert 2,
/// `cost x 21 / 25` ticks, where a vehicle took `cost / 3`: 2.52 times as long. The plans of
/// the computer players build two War Factories, which the multiple factory rule speeds up by
/// 1 / 0.8, so their armies, mostly vehicles, are built 2.52 x 0.8 = 2.016 times as slowly.
/// The limit is rescaled by that pacing factor, 2 (docs/DECISIONS.md R29, with the lengths of
/// the 756 pairings as `m14_duels -- full` plays them, with another seed, before and after; the
/// lengths of this test's own matches are in R56, and every run writes them to
/// `m6_production_matches_lengths.tsv` in the test's temporary directory).
const MATCH_TICK_LIMIT: u64 = 72_000;

/// Ticks between two checks of invariant I6' in a match ([`run`]).
const CELL_CHECK_INTERVAL: u64 = 16;

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
                    // (`m14_duels -- full` counts them the same way and plays the same pairings,
                    // with another seed and so along other courses: docs/DECISIONS.md R56).
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
    // The length of every match with its limit, written to a file when all are played, so
    // that the margin of this test's own matches (seed 0) is measured by every run of it
    // (docs/DECISIONS.md R56; before, only the duels' lengths were known, with another seed).
    let lengths = std::sync::Mutex::new(Vec::new());
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
                    let limit = tick_limit(grid);
                    let outcome = run(app.world_mut(), &name, limit);
                    assert!(
                        matches!(outcome, Some(MatchOutcome::Won { .. })),
                        "{name}: {outcome:?}"
                    );
                    let ticks = app.world().resource::<sim::SimTick>().0;
                    lengths
                        .lock()
                        .expect("the lengths")
                        .push((name, ticks, limit));
                    played.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            });
        }
    });
    assert_eq!(played.into_inner(), jobs.len());
    let mut lengths = lengths.into_inner().expect("the lengths");
    lengths.sort();
    let mut table = String::from("match\tticks\tlimit\n");
    for (name, ticks, limit) in &lengths {
        table.push_str(&format!("{name}\t{ticks}\t{limit}\n"));
    }
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("m6_production_matches_lengths.tsv");
    std::fs::write(&path, table).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}

/// Runs the match until it ends or the tick limit is reached; returns its outcome. Every rejected
/// command must name a unit or building its player lost in the tick before, or be a production or
/// placement command of a player who lost a unit or building then (docs/DECISIONS.md D43).
fn run(world: &mut World, name: &str, limit: u64) -> Option<MatchOutcome> {
    let mut losses: BTreeMap<PlayerId, BTreeSet<SimId>> = BTreeMap::new();
    let trace_ra5 = std::env::var_os("RA5_TRACE").is_some() && name.contains("coral_archipelago");
    for at in 0..limit {
        world.run_schedule(SimSchedule);
        if trace_ra5 && at % 10000 == 0 {
            let controllers = format!("{:?}", world.resource::<sim::AiPlayers>());
            let mut compact = String::new();
            let mut rest = controllers.as_str();
            while let Some(start) = rest.find("waypoints: [") {
                compact.push_str(&rest[..start]);
                let Some(end) = rest[start..].find("known:") else {
                    break;
                };
                rest = &rest[start + end..];
            }
            compact.push_str(rest);
            eprintln!("RA5 tick {at} controllers {compact}");
            let bridges = world.resource::<sim::Bridges>();
            for (id, b) in &bridges.bridges {
                if b.collapsed {
                    eprintln!(
                        "RA5 collapsed {id:?} cell_count {} huts {:?}",
                        b.cells.len(),
                        b.huts
                    );
                }
            }
            let d = world.resource::<sim::Definitions>().clone();
            let mut kinds: BTreeMap<(PlayerId, String), (usize, i64, i64, usize)> = BTreeMap::new();
            for (o, u, m, orders) in world
                .query::<(&sim::Owner, &sim::Unit, &sim::Mobile, Option<&sim::Orders>)>()
                .iter(world)
            {
                let key = (
                    o.0,
                    d.unit(u.kind).map_or("?", |d| d.id.as_str()).to_owned(),
                );
                let value = kinds.entry(key).or_default();
                value.0 += 1;
                value.1 += i64::from(m.cell.x);
                value.2 += i64::from(m.cell.y);
                value.3 += usize::from(orders.is_none_or(|o| o.current.is_none()));
            }
            eprintln!("RA5 types(count,sum_x,sum_y,idle) {kinds:?}");
        }
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
        // The invariants of the soak, to the end of the match (docs/DECISIONS.md R56: since
        // RA2 the soak's 36,000 ticks end before most matches on the shipped maps do, so the
        // late game is checked here, where every match is played out). I7 on every tick;
        // I6', from the units and from the grid, on every sixteenth tick and on the last.
        for (player, state) in world.resource::<sim::Players>().iter() {
            assert!(
                state.credits() >= 0,
                "{name}: tick {at}: player {} has {} credits",
                player.0,
                state.credits()
            );
        }
        let outcome = world.resource::<MatchState>().outcome();
        if at % CELL_CHECK_INTERVAL == 0 || outcome.is_some() {
            let mut problems = sim::cell_invariant_violations(world);
            problems.extend(sim::stray_claims(world));
            assert!(problems.is_empty(), "{name}: tick {at}: {problems:?}");
        }
        if let Some(outcome) = outcome {
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

/// One match of two computer players on a shipped map: the countries `countries` at the
/// difficulties `difficulties` of the AI file, played to its end within the tick limit.
fn duel(map: &str, countries: [&str; 2], difficulties: [&str; 2]) -> Option<MatchOutcome> {
    duel_with_length(map, countries, difficulties).0
}

/// [`duel`], and the ticks the match took.
fn duel_with_length(
    map: &str,
    countries: [&str; 2],
    difficulties: [&str; 2],
) -> (Option<MatchOutcome>, u64) {
    let data = assets().join("data");
    let d = load_ruleset(&data).unwrap();
    let sk = load_skirmish(data.join("skirmish.ron"), &d).unwrap();
    let ai = load_ai(data.join("ai.ron"), &d).unwrap();
    let index = sk.maps.iter().position(|def| def.id == map).unwrap();
    let grid = load_map(assets().join(map_asset_path(map))).unwrap().grid;
    let limit = tick_limit(&grid);
    let mut app = bevy::app::App::new();
    app.add_plugins(SimPlugin { seed: 0 });
    app.world_mut().insert_resource(grid);
    app.world_mut().insert_resource(d);
    let players = [0, 1].map(|k| PlayerSetup {
        player: PlayerId(k as u8),
        faction: sk
            .factions
            .iter()
            .position(|faction| faction.id == countries[k])
            .unwrap(),
        start: k,
        controller: Controller::Ai {
            difficulty: ai
                .difficulties
                .iter()
                .position(|difficulty| difficulty.id == difficulties[k])
                .unwrap(),
        },
    });
    setup_match(app.world_mut(), &sk, &ai, index, &players).unwrap();
    let name = format!("{map}: {countries:?} at {difficulties:?}");
    let outcome = run(app.world_mut(), &name, limit);
    let ticks = app.world().resource::<sim::SimTick>().0;
    eprintln!("{name}: {ticks} ticks of {limit}");
    (outcome, ticks)
}

#[test]
fn a_soviet_player_left_with_an_armed_miner_still_finishes_the_match() {
    // Regression (RA1, found by `m14_duels -- full`): on riverlands France (brutal) and Cuba
    // (easy) wore each other down until neither had an army, a producer of one, or income, and
    // Cuba was left with a War Miner that had nothing to mine. The match never ended (300,000
    // ticks). An armed harvester whose cycle has stopped fights when its player has no other
    // armed unit (`crates/sim/src/ai.rs`, `View::fights`; docs/DECISIONS.md R14).
    let outcome = duel("riverlands", ["france", "cuba"], ["brutal", "easy"]);
    assert!(
        matches!(outcome, Some(MatchOutcome::Won { .. })),
        "{outcome:?}"
    );
}

#[test]
fn the_last_wave_looks_for_the_last_buildings_near_the_enemys_start_first() {
    // Regression (RA1, found by `m14_duels -- full`): on crossroads Libya and Iraq (both easy)
    // needed 39,983 ticks, more than the limit of then, 36,000: for the last 13,000 the
    // winner's wave, formed at its own base, swept the waypoints nearest to itself for three
    // buildings that stood beside the loser's start. With the slower units of RA1
    // (docs/DECISIONS.md R7) that wandering no longer fitted the limit, so a sweep starts near
    // the enemy's start on every map (`crates/sim/src/ai.rs`, `choose_target`; R14).
    //
    // Since RA2 the limit is 72,000 (R29), and this match no longer fails by it without the
    // rule (measured at RA4 with this test's seed: 45,345 ticks with the rule, 53,431 without;
    // docs/DECISIONS.md R56). What fails without the rule is the unit test
    // `sim::ai::tests::a_sweep_starts_at_the_waypoint_nearest_to_an_enemy_start`; this test
    // still shows that the match of the report ends.
    let outcome = duel("crossroads", ["libya", "iraq"], ["easy", "easy"]);
    assert!(
        matches!(outcome, Some(MatchOutcome::Won { .. })),
        "{outcome:?}"
    );
}

#[test]
fn ra5_coral_archipelago_brutal_libya_germany_seed_zero_finishes() {
    let outcome = duel(
        "coral_archipelago",
        ["libya", "germany"],
        ["brutal", "brutal"],
    );
    assert!(matches!(outcome, Some(MatchOutcome::Won { .. })));
}
