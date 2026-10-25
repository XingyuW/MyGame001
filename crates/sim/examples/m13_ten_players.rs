//! Ten computer players on enlarged maps: how long a tick takes (docs/DECISIONS.md, "Ten players
//! on maps that grow with them").
//!
//! Run from the repository root with
//!
//! ```text
//! cargo run --release -p sim --example m13_ten_players
//! ```
//!
//! For each of a few shipped maps, as a match of ten players plays it (`sim::enlarged_map`: the
//! large maps are 256 x 256 cells as filed, the largest a map may be, and get the start positions
//! their files lack; the small ones are doubled), ten computer players of the shipped data
//! play all against all for [`MEASURED_TICKS`] ticks (20 minutes of game time) or until the match
//! ends. Every tick is timed with the clock (the harness reads the clock; the simulation never
//! does, I4), and the run reports the mean and the longest tick, the mean of the busiest
//! thousand ticks in a row, and how many units and buildings there were at most. It fails (exit
//! code 1) if a map's mean tick time exceeds [`MEAN_BUDGET`], the budget of the M7 benchmark
//! (docs/DECISIONS.md D55), or if the match did not get going (fewer than ten players placed a
//! building).
//!
//! Optional arguments: `to-the-end` plays the matches to their end instead (up to [`END_LIMIT`]
//! ticks) and reports the tick they ended in; `all` measures every map of the skirmish file
//! instead of the four of [`MAPS`], and map ids measure those maps: `-- to-the-end all`,
//! `-- to-the-end frostline`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    Building, Controller, MAX_MATCH_PLAYERS, MatchOutcome, MatchState, PlayerId, PlayerSetup,
    SimEvent, SimPlugin, SimSchedule, Unit, enlarged_map, load_ai, load_map, load_ruleset,
    load_skirmish, map_asset_path, setup_match,
};

/// The budget for the mean tick time, as in the M7 benchmark.
const MEAN_BUDGET: Duration = Duration::from_millis(5);

/// One tick period at 20 ticks per second: a longer tick makes the game fall behind for a moment.
const TICK_PERIOD: Duration = Duration::from_millis(50);

/// Ticks measured per map (20 minutes of game time).
const MEASURED_TICKS: u64 = 24_000;

/// Ticks a match played to its end may take before the run gives up on it.
const END_LIMIT: u64 = 400_000;

/// The window, in ticks, whose busiest mean is reported.
const WINDOW: usize = 1_000;

/// The maps measured: the city map, the one with the longest land routes, an ocean map (all
/// 256 x 256), and the smallest map (112 x 112 for ten players).
const MAPS: [&str; 4] = [
    "liberty_bay",
    "continental_divide",
    "pacific_atoll",
    "crossroads",
];

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn has_winner(ended: Option<(u64, MatchOutcome)>) -> bool {
    matches!(ended, Some((_, MatchOutcome::Won { .. })))
}

fn trace(world: &mut bevy_ecs::world::World, definitions: &sim::Definitions, map: &str, tick: u64) {
    let units: Vec<_> = world
        .query::<(
            &sim::SimId,
            &sim::Owner,
            &sim::Unit,
            &sim::Mobile,
            &sim::Orders,
            &sim::Health,
            Option<&sim::Weapon>,
        )>()
        .iter(world)
        .map(|(id, owner, unit, mobile, orders, health, weapon)| {
            (
                *id,
                owner.0,
                unit.kind,
                mobile.cell,
                mobile.step.is_some(),
                health.hit_points,
                format!("{:?}", orders.current),
                format!("{weapon:?}"),
                weapon.and_then(|w| w.target),
            )
        })
        .collect();
    let buildings: Vec<_> = world
        .query::<(&sim::SimId, &sim::Owner, &sim::Building, &sim::Health)>()
        .iter(world)
        .map(|(id, owner, building, hp)| {
            (
                *id,
                owner.0,
                building.kind,
                building.footprint.origin,
                hp.hit_points,
            )
        })
        .collect();
    let harvesters: Vec<_> = world
        .query::<(&sim::SimId, &sim::Owner, &sim::Harvester)>()
        .iter(world)
        .map(|(id, owner, harvest)| (*id, owner.0, format!("{harvest:?}")))
        .collect();
    let special = world.resource::<sim::SpecialState>();
    for target in units
        .iter()
        .filter_map(|unit| unit.8)
        .collect::<BTreeSet<_>>()
    {
        let unit = units.iter().find(|unit| unit.0 == target);
        let building = buildings.iter().find(|building| building.0 == target);
        let garrison = special.garrisons.get(&target);
        let prop = world.resource::<sim::Props>().get(target);
        eprintln!(
            "{map} trace tick{tick} acquired{target:?}: unit{unit:?}, building{building:?}, garrison{garrison:?}, prop{prop:?}"
        );
    }
    for (player, state) in world.resource::<sim::Players>().iter() {
        if world.resource::<MatchState>().defeated(player).is_some() {
            continue;
        }
        let controller = world.resource::<sim::AiPlayers>().controller(player);
        if tick == 120_000 {
            let vision = state.vision();
            let visible_units: Vec<_> = units
                .iter()
                .filter(|u| world.resource::<sim::Players>().hostile(player, u.1))
                .map(|u| {
                    (
                        u.0,
                        u.1,
                        definitions.unit(u.2).map(|d| d.id.as_str()),
                        u.3,
                        vision.is_visible(u.3),
                        special.hidden.contains(&(player, u.0)),
                        harvesters.iter().any(|h| h.0 == u.0),
                        &u.7,
                    )
                })
                .collect();
            let origin_visibility: Vec<_> = buildings
                .iter()
                .filter(|b| world.resource::<sim::Players>().hostile(player, b.1))
                .map(|b| (b.0, vision.is_visible(b.3)))
                .collect();
            eprintln!(
                "{map} focus guard tick{tick} player{player:?} fog{} origins{origin_visibility:?} visible_hostiles{visible_units:?}",
                vision.uses_fog()
            );
        }
        let own_buildings: Vec<_> = buildings
            .iter()
            .filter(|b| b.1 == player)
            .map(|b| {
                (
                    b.0,
                    definitions.building(b.2).map(|d| d.id.as_str()),
                    b.3,
                    b.4,
                )
            })
            .collect();
        let queues: Vec<_> = state.queues().collect();
        eprintln!(
            "{map} trace tick{tick} player{}: credits{}, repairing{:?}, queues{queues:?}, buildings{own_buildings:?}, wave{:?}, target{:?}, sea{:?}",
            player.0,
            state.credits(),
            state.repairing(),
            controller.map(|c| c.wave()),
            controller.and_then(|c| c.target()),
            controller.and_then(|c| c.naval_target())
        );
        if tick >= 120_000
            && let Some(controller) = controller
        {
            let debug = format!("{controller:?}");
            let tail = debug.split_once(", known: ").map(|(_, tail)| tail);
            let objective = match controller.target() {
                Some(sim::WaveTarget::Waypoint(index)) => controller.waypoints().get(index),
                _ => None,
            };
            let observations: Vec<_> = controller
                .known_buildings()
                .iter()
                .map(|(id, known)| (*id, state.vision().sees_footprint(known.footprint)))
                .collect();
            let charges: Vec<_> = buildings
                .iter()
                .filter(|building| building.1 == player)
                .filter_map(|building| {
                    special
                        .charge
                        .get(&building.0)
                        .map(|charge| (building.0, *charge))
                })
                .collect();
            let own_harvesters: Vec<_> = harvesters.iter().filter(|h| h.1 == player).collect();
            eprintln!(
                "{map} detail tick{tick} player{} power{}/{} low{} gather{:?} objective{objective:?} observations{observations:?} charges{charges:?} harvesters{own_harvesters:?} known_and_controller{tail:?}",
                player.0,
                state.power_produced(),
                state.power_consumed(),
                state.low_power(),
                controller.gather()
            );
        }
        for unit in units.iter().filter(|u| u.1 == player) {
            let commandable = !special.spawned.contains_key(&unit.0)
                && !special.strategic.flights.contains_key(&unit.0)
                && !special.erasures.contains_key(&unit.0)
                && !special.deployed.contains_key(&unit.0)
                && !special.interactions.contains_key(&unit.0)
                && !special.docked.contains_key(&unit.0)
                && !special.rearming.contains_key(&unit.0)
                && !special.disabled.contains_key(&unit.0);
            eprintln!(
                "  {:?} {:?} at{:?} stepping{} hp{} commandable{commandable} order{} attack{}",
                unit.0,
                definitions.unit(unit.2).map(|d| d.id.as_str()),
                unit.3,
                unit.4,
                unit.5,
                unit.6,
                unit.7
            );
        }
    }
}

#[test]
fn ra8_ten_player_completion_requires_an_explicit_win() {
    assert!(!has_winner(None));
    assert!(!has_winner(Some((1, MatchOutcome::Draw { tick: 1 }))));
    assert!(has_winner(Some((
        1,
        MatchOutcome::Won {
            winner: PlayerId(0),
            tick: 1
        }
    ))));
    assert_eq!(END_LIMIT, 400_000);
}

fn main() -> ExitCode {
    let to_the_end = std::env::args().any(|argument| argument == "to-the-end");
    let tracing = std::env::args().any(|argument| argument == "trace");
    let data = repository().join("assets/data");
    let loaded = load_ruleset(&data).and_then(|definitions| {
        let skirmish = load_skirmish(data.join("skirmish.ron"), &definitions)?;
        let ai = load_ai(data.join("ai.ron"), &definitions)?;
        Ok((definitions, skirmish, ai))
    });
    let (definitions, skirmish, ai) = match loaded {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let mut failed = false;
    let named: Vec<String> = std::env::args()
        .skip(1)
        .filter(|argument| skirmish.maps.iter().any(|def| def.id == *argument))
        .collect();
    let ids: Vec<String> = if !named.is_empty() {
        named
    } else if std::env::args().any(|argument| argument == "all") {
        skirmish.maps.iter().map(|def| def.id.clone()).collect()
    } else {
        MAPS.iter().map(|id| (*id).to_owned()).collect()
    };
    for id in &ids {
        let id = id.as_str();
        let Some(map) = skirmish.maps.iter().position(|def| def.id == id) else {
            eprintln!("{id}: not a map of the skirmish file");
            return ExitCode::FAILURE;
        };
        let file = match load_map(repository().join("assets").join(map_asset_path(id))) {
            Ok(file) => file,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::FAILURE;
            }
        };
        let started = Instant::now();
        let enlarged = match enlarged_map(
            &file.grid,
            &file.looks,
            &skirmish.maps[map].starts,
            MAX_MATCH_PLAYERS,
        ) {
            Ok(enlarged) => enlarged,
            Err(error) => {
                eprintln!("{id}: {error}");
                return ExitCode::FAILURE;
            }
        };
        let enlarging = started.elapsed();
        let mut starts = skirmish.clone();
        starts.maps[map].starts.clone_from(&enlarged.starts);
        let players: Vec<PlayerSetup> = (0..MAX_MATCH_PLAYERS)
            .map(|k| PlayerSetup {
                player: PlayerId(k as u8),
                // The nine countries in turn (docs/DECISIONS.md R5): five Allied, four Soviet,
                // and the first again.
                faction: k % skirmish.factions.len(),
                start: k,
                // The middle difficulty, the menu's first choice.
                controller: Controller::Ai {
                    difficulty: ai.difficulties.len() / 2,
                },
            })
            .collect();
        let mut app = App::new();
        app.add_plugins(SimPlugin { seed: 13 });
        app.world_mut().insert_resource(enlarged.grid.clone());
        app.world_mut().insert_resource(definitions.clone());
        let started = Instant::now();
        if let Err(error) = setup_match(app.world_mut(), &starts, &ai, map, &players) {
            eprintln!("{id}: {error}");
            return ExitCode::FAILURE;
        }
        let setting_up = started.elapsed();
        let world = app.world_mut();
        let limit = if to_the_end {
            END_LIMIT
        } else {
            MEASURED_TICKS
        };
        let mut times: Vec<Duration> = Vec::new();
        let (mut most_units, mut most_buildings) = (0, 0);
        let mut builders: BTreeSet<PlayerId> = BTreeSet::new();
        let mut ended = None;
        let playing = Instant::now();
        // Observational trace counters never enter the World or command queue.
        let mut trace_window: BTreeMap<String, u64> = BTreeMap::new();
        for tick in 0..limit {
            let trace_before = (tracing && tick >= 120_000).then(|| {
                let credits: BTreeMap<_, _> = world
                    .resource::<sim::Players>()
                    .iter()
                    .map(|(id, state)| (id, state.credits()))
                    .collect();
                let hp: BTreeMap<_, _> = world
                    .query::<(&sim::SimId, &sim::Health)>()
                    .iter(world)
                    .map(|(id, hp)| (*id, hp.hit_points))
                    .collect();
                (credits, hp)
            });
            let started = Instant::now();
            world.run_schedule(SimSchedule);
            times.push(started.elapsed());
            for event in world
                .resource::<Messages<SimEvent>>()
                .iter_current_update_messages()
            {
                if let SimEvent::BuildingPlaced { player, .. } = event {
                    builders.insert(*player);
                }
                if tracing
                    && matches!(
                        event,
                        SimEvent::WeaponFired { .. }
                            | SimEvent::OreDelivered { .. }
                            | SimEvent::StrategicActivated { .. }
                            | SimEvent::BuildingDestroyed { .. }
                            | SimEvent::UnitDestroyed { .. }
                            | SimEvent::ProductionComplete { .. }
                            | SimEvent::ProductionCancelled { .. }
                            | SimEvent::UnitSpawned { .. }
                            | SimEvent::BuildingPlaced { .. }
                            | SimEvent::BuildingSold { .. }
                            | SimEvent::MoveFailed { .. }
                            | SimEvent::CommandRejected { .. }
                    )
                {
                    let count = trace_window.entry(format!("{event:?}")).or_default();
                    *count = count.saturating_add(1);
                }
            }
            if let Some((credits, hp)) = trace_before {
                for (actor, owner, current) in world
                    .query::<(&sim::SimId, &sim::Owner, &sim::Health)>()
                    .iter(world)
                {
                    if let Some(&previous) = hp.get(actor)
                        && current.hit_points > previous
                    {
                        let delta =
                            world
                                .resource::<sim::Players>()
                                .get(owner.0)
                                .and_then(|state| {
                                    credits.get(&owner.0).map(|before| state.credits() - before)
                                });
                        let count = trace_window.entry(format!("NativeHpIncrease {{ actor: {actor:?}, owner: {:?}, hp_gain: {}, simultaneous_credit_delta: {delta:?} }}", owner.0, current.hit_points - previous)).or_default();
                        *count = count.saturating_add(1);
                    }
                }
            }
            if tracing && (tick.is_multiple_of(12_000) || tick + 1 == limit) {
                trace(world, &definitions, id, tick);
                eprintln!("{id} native event window ending tick{tick}: {trace_window:?}");
                trace_window.clear();
            }
            if tick % 100 == 0 {
                most_units = most_units.max(world.query::<&Unit>().iter(world).count());
                most_buildings = most_buildings.max(world.query::<&Building>().iter(world).count());
            }
            if let Some(outcome) = world.resource::<MatchState>().outcome() {
                ended = Some((tick, outcome));
                break;
            }
        }
        let total: Duration = times.iter().sum();
        let elapsed = playing.elapsed();
        let mean = total / u32::try_from(times.len().max(1)).unwrap_or(u32::MAX);
        let longest = times.iter().max().copied().unwrap_or_default();
        let late = times.iter().filter(|&&time| time > TICK_PERIOD).count();
        let busiest = times
            .windows(WINDOW.min(times.len()).max(1))
            .map(|window| window.iter().sum::<Duration>() / window.len() as u32)
            .max()
            .unwrap_or_default();
        let mut sorted = times.clone();
        sorted.sort_unstable();
        let centile = sorted[(sorted.len() * 99 / 100).min(sorted.len() - 1)];
        println!(
            "{id}: {} x {} cells for {MAX_MATCH_PLAYERS} players (file {} x {}), enlarged in \
             {enlarging:.1?}, set up in {setting_up:.1?}",
            enlarged.grid.width(),
            enlarged.grid.height(),
            file.grid.width(),
            file.grid.height()
        );
        println!(
            "  {} ticks: mean {mean:.3?}, 99th centile {centile:.3?}, longest {longest:.3?}, \
             busiest {WINDOW} ticks mean {busiest:.3?}, {late} ticks over {TICK_PERIOD:?}; measured total {total:.3?}, elapsed {elapsed:.3?}",
            times.len()
        );
        println!(
            "  at most {most_units} units and {most_buildings} buildings; {} players built; {}",
            builders.len(),
            match ended {
                Some((tick, outcome)) => format!("ended in tick {tick}: {outcome:?}"),
                None => {
                    let state = world.resource::<MatchState>();
                    let left = state
                        .players()
                        .iter()
                        .filter(|&&player| state.defeated(player).is_none())
                        .count();
                    format!("{left} players left after {} ticks", times.len())
                }
            }
        );
        if mean > MEAN_BUDGET {
            eprintln!("{id}: the mean tick time {mean:?} exceeds the budget of {MEAN_BUDGET:?}");
            failed = true;
        }
        if builders.len() < MAX_MATCH_PLAYERS {
            eprintln!(
                "{id}: only {} of {MAX_MATCH_PLAYERS} players built",
                builders.len()
            );
            failed = true;
        }
        if to_the_end && !has_winner(ended) {
            eprintln!("{id}: the match had no winner within {END_LIMIT} ticks");
            failed = true;
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
