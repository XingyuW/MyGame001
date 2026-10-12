//! M6 tests: the computer opponent (PLAN.md section 7, M6; invariants I3, I5, I6, I7;
//! docs/DECISIONS.md D42, D43).
//!
//! The plan's required tests: on a fixture map the AI completes its build order within a fixed tick
//! budget; the AI issues zero rejected commands in the standard scenario; an AI with no credits and
//! no harvester neither panics nor floods the queue; end to end AI against AI matches on every
//! shipped map finish with a winner within a fixed tick limit, without panic, and with equal hashes
//! across two runs. Further tests cover rebuilding, defence, and what the AI may know (fog of war).
//!
//! Every scenario is set up with `setup_match` on fixture data (the fixture units, economy, combat,
//! skirmish, and AI files; the 40x40 skirmish fixture map, or a shipped map with the start
//! positions of the production skirmish file) and then runs only through `SimPlugin` and
//! `CommandQueue`: the computer players push their commands into the queue, and the tests' own
//! players do the same. The computer players' commands of a tick are read from the queue after
//! the tick (`CommandQueue::iter`). The match length is bounded by [`MATCH_TICK_LIMIT`] (on the
//! large shipped maps by proportionally more, [`tick_limit`]), and the armies by the AI file's
//! `max_army`, so every test is bounded in time and memory.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    AiDefinitions, AiPlayers, Building, CellPos, Command, CommandQueue, Controller, Definitions,
    Grid, HARVEST_RETRY_TICKS, IssuedCommand, MatchOutcome, MatchState, Mobile, Owner, PlayerId,
    PlayerSetup, Players, RejectReason, SimEvent, SimId, SimPlugin, SimSchedule,
    SkirmishDefinitions, Unit, load_ai, load_map, load_match_definitions, load_skirmish,
    map_asset_path, setup_match, spawn_unit, state_hash,
};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

/// Ticks within which a match must end: 30 minutes of game time.
const MATCH_TICK_LIMIT: u64 = 36_000;

/// Ticks within which a computer player completes the fixture build order (six buildings, the
/// longest built in 40 ticks) at every fixture difficulty.
const BUILD_ORDER_BUDGET: u64 = 1_000;

/// Most commands a computer player pushes in one turn: a placement, a building, a harvester and
/// an army unit per producer type (the fixture has two), rally points for its producers (at most
/// one per building it owns), harvest, defence or gathering, and the wave.
const MAX_COMMANDS_PER_TURN: usize = 16;

fn fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(relative)
}

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

#[derive(Clone)]
struct Data {
    definitions: Definitions,
    skirmish: SkirmishDefinitions,
    ai: AiDefinitions,
}

fn data() -> Data {
    let definitions = load_match_definitions(
        fixture("data/units_combat.ron"),
        fixture("data/economy_combat.ron"),
        fixture("data/combat.ron"),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let skirmish = load_skirmish(fixture("data/skirmish.ron"), &definitions)
        .unwrap_or_else(|error| panic!("{error}"));
    let ai =
        load_ai(fixture("data/ai.ron"), &definitions).unwrap_or_else(|error| panic!("{error}"));
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

fn human(player: PlayerId, faction: usize, start: usize) -> PlayerSetup {
    PlayerSetup {
        player,
        faction,
        start,
        controller: Controller::Human,
    }
}

/// A match on `grid` set up for `players` on map `map` of `data.skirmish`.
fn match_on(data: &Data, grid: Grid, map: usize, players: &[PlayerSetup]) -> App {
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 6 });
    app.world_mut().insert_resource(grid);
    app.world_mut().insert_resource(data.definitions.clone());
    setup_match(app.world_mut(), &data.skirmish, &data.ai, map, players)
        .unwrap_or_else(|error| panic!("{error}"));
    app
}

fn fixture_grid() -> Grid {
    load_map(fixture("maps/skirmish_40x40.tmx"))
        .unwrap_or_else(|error| panic!("{error}"))
        .grid
}

fn fixture_match(data: &Data, players: &[PlayerSetup]) -> App {
    match_on(data, fixture_grid(), 0, players)
}

/// One tick with `commands`; returns its events and the commands the computer players pushed.
fn tick(app: &mut App, commands: Vec<IssuedCommand>) -> (Vec<SimEvent>, Vec<IssuedCommand>) {
    {
        let mut queue = app.world_mut().resource_mut::<CommandQueue>();
        for command in commands {
            queue.push(command);
        }
    }
    app.world_mut().run_schedule(SimSchedule);
    let events = app
        .world()
        .resource::<Messages<SimEvent>>()
        .iter_current_update_messages()
        .cloned()
        .collect();
    let pushed = app
        .world()
        .resource::<CommandQueue>()
        .iter()
        .cloned()
        .collect();
    (events, pushed)
}

fn outcome(app: &App) -> Option<MatchOutcome> {
    app.world().resource::<MatchState>().outcome()
}

fn owned_kinds(app: &mut App, player: PlayerId) -> BTreeMap<sim::BuildingKind, usize> {
    let mut query = app.world_mut().query::<(&Owner, &Building)>();
    let mut counts = BTreeMap::new();
    for (owner, building) in query.iter(app.world()) {
        if owner.0 == player {
            *counts.entry(building.kind).or_insert(0) += 1;
        }
    }
    counts
}

/// Whether `player` owns every building of the build order as often as the build order lists it.
fn build_order_complete(app: &mut App, ai: &AiDefinitions, player: PlayerId) -> bool {
    let owned = owned_kinds(app, player);
    ai.build_order.iter().all(|kind| {
        let listed = ai.build_order.iter().filter(|other| *other == kind).count();
        owned.get(kind).copied().unwrap_or(0) >= listed
    })
}

/// Asserts I6 and I7 for the current state: one unit per cell and no unit on a building's cell,
/// and credits never negative.
fn assert_invariants(app: &mut App, at: u64) {
    let mut units = app.world_mut().query::<(&SimId, &Mobile)>();
    let mut held = BTreeSet::new();
    let grid = app.world().resource::<Grid>().clone();
    for (id, mobile) in units.iter(app.world()) {
        for cell in [Some(mobile.cell), mobile.step.map(|step| step.to)]
            .into_iter()
            .flatten()
        {
            assert!(
                held.insert(cell),
                "tick {at}: cell {cell:?} held twice (I6)"
            );
            assert_eq!(
                grid.building(cell),
                None,
                "tick {at}: unit {id:?} on a building (I6)"
            );
        }
    }
    for (player, state) in app.world().resource::<Players>().iter() {
        assert!(
            state.credits() >= 0,
            "tick {at}: player {player:?} credits (I7)"
        );
    }
}

// --- Build order ----------------------------------------------------------------------------

#[test]
fn the_ai_completes_its_build_order_within_the_tick_budget() {
    let data = data();
    for difficulty in 0..data.ai.difficulties.len() {
        for (faction, start) in [(0, 0), (1, 1)] {
            let other = 1 - start;
            let mut app = fixture_match(
                &data,
                &[
                    computer(P1, faction, start, difficulty),
                    human(P0, 1 - faction, other),
                ],
            );
            let mut placed: Vec<sim::BuildingKind> = Vec::new();
            let mut done_at = None;
            for at in 0..BUILD_ORDER_BUDGET {
                let (events, pushed) = tick(&mut app, Vec::new());
                assert!(
                    pushed.len() <= MAX_COMMANDS_PER_TURN,
                    "tick {at}: {pushed:?}"
                );
                for event in events {
                    match event {
                        SimEvent::BuildingPlaced { player, kind, .. } if player == P1 => {
                            placed.push(kind);
                        }
                        SimEvent::CommandRejected { player, reason, .. } if player == P1 => {
                            panic!("tick {at}: rejected {reason}")
                        }
                        _ => {}
                    }
                }
                assert_invariants(&mut app, at);
                if build_order_complete(&mut app, &data.ai, P1) {
                    done_at = Some(at);
                    break;
                }
            }
            let done_at = done_at.unwrap_or_else(|| {
                panic!(
                    "difficulty {difficulty}, start {start}: build order incomplete after \
                     {BUILD_ORDER_BUDGET} ticks; placed {placed:?}"
                )
            });
            // Every building of the build order that the faction does not start with was placed
            // during the match (the power building may come earlier than listed when power runs
            // short).
            let mut expected: Vec<sim::BuildingKind> = Vec::new();
            for kind in data.ai.build_order.iter().collect::<BTreeSet<_>>() {
                let listed = data
                    .ai
                    .build_order
                    .iter()
                    .filter(|other| *other == kind)
                    .count();
                let started = data.skirmish.factions[faction]
                    .buildings
                    .iter()
                    .filter(|building| building.kind == *kind)
                    .count();
                expected.extend(std::iter::repeat_n(*kind, listed.saturating_sub(started)));
            }
            let mut sorted = placed.clone();
            sorted.sort_unstable();
            assert_eq!(sorted, expected, "start {start}: placed {placed:?}");
            assert!(done_at < BUILD_ORDER_BUDGET);
        }
    }
}

// --- Standard scenario ----------------------------------------------------------------------

/// What a match run observed.
struct Run {
    outcome: Option<MatchOutcome>,
    ticks: u64,
    /// `state_hash` after every tick.
    hashes: Vec<u64>,
    /// Every computer player's rejected commands: tick, player, command, reason, and for a
    /// placement rejected as `Occupied`, whether the unit in the way belongs to another player.
    rejections: Vec<(u64, PlayerId, Command, RejectReason, bool)>,
    /// The ids of the units and buildings each player lost, per tick.
    losses: BTreeMap<u64, BTreeMap<PlayerId, BTreeSet<SimId>>>,
    /// Commands each computer player pushed.
    pushed: BTreeMap<PlayerId, usize>,
    /// The commands each computer player pushed, with the tick.
    commands: BTreeMap<PlayerId, Vec<(u64, Command)>>,
}

/// Runs `app` until the match ends or [`MATCH_TICK_LIMIT`] ticks have run, checking the invariants
/// and the bound on the commands of a turn on every tick.
fn run_match(app: App) -> Run {
    run_match_within(app, MATCH_TICK_LIMIT)
}

/// The tick limit of a match on `grid`: [`MATCH_TICK_LIMIT`] on maps up to
/// `sim::ARMY_SCALE_SPAN` cells wide and high together, and proportionally longer on larger
/// ones, whose armies have farther to go (as `crates/game/tests/m6_production_matches.rs`).
fn tick_limit(grid: &Grid) -> u64 {
    let span = u64::from(grid.width() + grid.height()).max(u64::from(sim::ARMY_SCALE_SPAN));
    MATCH_TICK_LIMIT * span / u64::from(sim::ARMY_SCALE_SPAN)
}

/// [`run_match`] with a tick limit of `limit`.
fn run_match_within(mut app: App, limit: u64) -> Run {
    let computers: BTreeSet<PlayerId> = app.world().resource::<AiPlayers>().players().collect();
    let mut run = Run {
        outcome: None,
        ticks: 0,
        hashes: Vec::new(),
        rejections: Vec::new(),
        losses: BTreeMap::new(),
        pushed: BTreeMap::new(),
        commands: BTreeMap::new(),
    };
    for at in 0..limit {
        let (events, pushed) = tick(&mut app, Vec::new());
        assert!(
            pushed.len() <= 2 * MAX_COMMANDS_PER_TURN,
            "tick {at}: {pushed:?}"
        );
        for issued in &pushed {
            assert!(computers.contains(&issued.player), "{issued:?}");
            *run.pushed.entry(issued.player).or_insert(0) += 1;
            run.commands
                .entry(issued.player)
                .or_default()
                .push((at, issued.command.clone()));
        }
        for event in events {
            match event {
                SimEvent::CommandRejected {
                    player,
                    command,
                    reason,
                } => {
                    assert!(computers.contains(&player), "{player:?}");
                    let foreign = match &reason {
                        RejectReason::Placement(sim::PlacementError::Occupied { unit, .. }) => {
                            let mut owners = app.world_mut().query::<(&SimId, &Owner)>();
                            owners
                                .iter(app.world())
                                .any(|(id, owner)| id == unit && owner.0 != player)
                        }
                        _ => false,
                    };
                    run.rejections.push((at, player, command, reason, foreign));
                }
                SimEvent::UnitDestroyed { player, unit } => {
                    run.losses
                        .entry(at)
                        .or_default()
                        .entry(player)
                        .or_default()
                        .insert(unit);
                }
                SimEvent::BuildingDestroyed {
                    player, building, ..
                } => {
                    run.losses
                        .entry(at)
                        .or_default()
                        .entry(player)
                        .or_default()
                        .insert(building);
                }
                _ => {}
            }
        }
        if at % 10 == 0 {
            assert_invariants(&mut app, at);
        }
        run.hashes.push(state_hash(app.world_mut()));
        run.ticks = at + 1;
        if let Some(ended) = outcome(&app) {
            run.outcome = Some(ended);
            break;
        }
    }
    run
}

/// Asserts that every rejected command of a computer player is explained by what happened in the
/// tick between its decision and the command's validation (docs/DECISIONS.md D43): the unit or
/// building it names was destroyed then; for a production or placement command, one of its
/// buildings or units was; or a placement was entered by another player's unit that it could not
/// see next to the footprint.
fn assert_rejections_explained(run: &Run) {
    for (at, player, command, reason, foreign) in &run.rejections {
        let lost = at
            .checked_sub(1)
            .and_then(|before| run.losses.get(&before))
            .and_then(|losses| losses.get(player));
        let explained = match reason {
            RejectReason::UnknownUnit(id) | RejectReason::UnknownBuilding(id) => {
                lost.is_some_and(|lost| lost.contains(id))
            }
            RejectReason::MissingPrerequisite { .. }
            | RejectReason::NoProducer { .. }
            | RejectReason::Placement(sim::PlacementError::NotVisible(_))
            | RejectReason::Placement(sim::PlacementError::TooFar { .. }) => {
                lost.is_some_and(|lost| !lost.is_empty())
            }
            RejectReason::Placement(sim::PlacementError::Occupied { .. }) => *foreign,
            _ => false,
        };
        assert!(
            explained,
            "tick {at}: player {player:?}: {command:?} rejected ({reason}) without a loss"
        );
    }
}

#[test]
fn the_ai_issues_no_rejected_command_in_the_standard_scenario() {
    // The standard scenario (docs/DECISIONS.md D43): a computer player against a player who gives
    // no order, on the fixture map with the fixture data, until the match ends. The passive
    // player's units and turret still fire at what comes into range.
    let data = data();
    for difficulty in 0..data.ai.difficulties.len() {
        let app = fixture_match(&data, &[human(P0, 0, 0), computer(P1, 1, 1, difficulty)]);
        let run = run_match(app);
        assert_eq!(
            run.rejections,
            [],
            "difficulty {difficulty}: rejected commands"
        );
        assert!(
            matches!(run.outcome, Some(MatchOutcome::Won { winner: P1, .. })),
            "difficulty {difficulty}: {:?} after {} ticks",
            run.outcome,
            run.ticks
        );
        assert!(run.pushed.get(&P1).copied().unwrap_or(0) > 0);
    }
}

// --- No credits, no harvester ---------------------------------------------------------------

#[test]
fn an_ai_with_no_credits_and_no_harvester_neither_panics_nor_floods_the_queue() {
    let mut data = data();
    data.skirmish.starting_credits = 0;
    for faction in &mut data.skirmish.factions {
        faction.units.clear();
        faction.buildings.truncate(1);
    }
    // The most frequent turn: every tick.
    for difficulty in &mut data.ai.difficulties {
        difficulty.think_interval = 1;
    }
    let mut app = fixture_match(&data, &[computer(P0, 0, 0, 1), computer(P1, 1, 1, 0)]);
    let mut pushed_total = 0;
    for at in 0..3_000 {
        let (events, pushed) = tick(&mut app, Vec::new());
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, SimEvent::CommandRejected { .. })),
            "tick {at}: {events:?}"
        );
        // At most one command per player in any tick, and only the first building.
        assert!(pushed.len() <= 2, "tick {at}: {pushed:?}");
        pushed_total += pushed.len();
        let players = app.world().resource::<Players>();
        for (_, state) in players.iter() {
            assert_eq!(state.credits(), 0);
            let queued: usize = state.queues().map(|(_, items)| items.len()).sum();
            assert!(queued <= 1, "tick {at}: {queued} items queued");
        }
    }
    // Each queued its first building once, and nothing else.
    assert_eq!(pushed_total, 2);
    assert_eq!(app.world().resource::<CommandQueue>().len(), 0);
    assert_eq!(outcome(&app), None);
}

#[test]
fn idle_harvesters_without_ore_are_sent_to_work_at_most_once_per_retry_interval() {
    // Credits, a refinery, and harvesters, but no ore anywhere: the harvesters stop with
    // `HarvesterIdle { NoOre }`, and the computer player orders them back to work only once every
    // `HARVEST_RETRY_TICKS`, so neither commands nor ore searches accumulate.
    let mut data = data();
    let refinery = data
        .definitions
        .building_kind("refinery")
        .expect("refinery");
    let harvester = data.definitions.unit_kind("harvester").expect("harvester");
    data.skirmish.factions[0]
        .buildings
        .push(sim::StartingBuilding {
            kind: refinery,
            offset: (0, 5),
        });
    data.skirmish.factions[0].units.push(sim::StartingGroup {
        kind: harvester,
        offset: (4, 4),
        count: 2,
    });
    for difficulty in &mut data.ai.difficulties {
        difficulty.think_interval = 1;
    }
    let mut grid = fixture_grid();
    for y in 0..40 {
        for x in 0..40 {
            let cell = CellPos::new(x, y);
            if let Some(&terrain) = grid.cell(cell)
                && terrain.ore > 0
            {
                grid.set_terrain(cell, sim::Cell { ore: 0, ..terrain });
            }
        }
    }
    let mut app = match_on(&data, grid, 0, &[computer(P0, 0, 0, 1), human(P1, 1, 1)]);
    let mut harvest_orders = Vec::new();
    let ticks = 2_000;
    for at in 0..ticks {
        let (_, pushed) = tick(&mut app, Vec::new());
        for issued in pushed {
            if let Command::Harvest { units, .. } = issued.command {
                assert_eq!(units.len(), 2, "both harvesters");
                harvest_orders.push(at);
            }
        }
    }
    assert!(
        !harvest_orders.is_empty(),
        "the harvesters were sent to work"
    );
    for pair in harvest_orders.windows(2) {
        assert!(
            pair[1] - pair[0] >= HARVEST_RETRY_TICKS,
            "{harvest_orders:?}"
        );
    }
    assert!(harvest_orders.len() as u64 <= ticks / HARVEST_RETRY_TICKS + 1);
}

#[test]
fn a_broke_ai_without_army_sells_a_building_it_does_not_need_to_fund_units() {
    // No credits, no units, no harvesters: the computer player can neither fight nor pay for a
    // fighter. It sells the building with the highest sell value that its army does not need (not
    // the barracks, which produces riflemen, and not the yard, which produces buildings, while
    // anything else is left), does not rebuild it while broke, and buys an army unit with the
    // refund.
    let mut data = data();
    data.skirmish.starting_credits = 0;
    let building = |id| data.definitions.building_kind(id).expect("building");
    let (yard, power, barracks) = (building("yard"), building("power"), building("barracks"));
    for faction in &mut data.skirmish.factions {
        faction.units.clear();
        faction.buildings = vec![
            sim::StartingBuilding {
                kind: yard,
                offset: (0, 0),
            },
            sim::StartingBuilding {
                kind: power,
                offset: (4, 0),
            },
            sim::StartingBuilding {
                kind: barracks,
                offset: (0, 4),
            },
        ];
    }
    let mut app = fixture_match(&data, &[computer(P0, 0, 0, 1), human(P1, 1, 1)]);
    let mut sold = Vec::new();
    let mut armed_at = None;
    for at in 0..1_000 {
        let (events, pushed) = tick(&mut app, Vec::new());
        for event in &events {
            if let SimEvent::BuildingSold {
                player, building, ..
            } = event
            {
                assert_eq!(*player, P0);
                sold.push(*building);
            }
            assert!(
                !matches!(event, SimEvent::CommandRejected { .. }),
                "tick {at}: {event:?}"
            );
        }
        for issued in pushed {
            assert!(
                !matches!(
                    issued.command,
                    Command::QueueProduction {
                        item: sim::ProductionItem::Building(_)
                    }
                ) || !sold.is_empty() && armed_at.is_some(),
                "tick {at}: no building is queued while broke: {issued:?}"
            );
        }
        let mut units = app.world_mut().query::<(&Owner, &sim::Weapon)>();
        if armed_at.is_none() && units.iter(app.world()).any(|(owner, _)| owner.0 == P0) {
            armed_at = Some(at);
        }
        if armed_at.is_some() {
            break;
        }
    }
    let owned = owned_kinds(&mut app, P0);
    assert_eq!(sold.len(), 1, "one sale funds a rifleman: {sold:?}");
    assert_eq!(owned.get(&power), None, "the power building was sold");
    assert_eq!(owned.get(&yard), Some(&1));
    assert_eq!(owned.get(&barracks), Some(&1));
    assert!(
        armed_at.is_some(),
        "an army unit was bought with the refund"
    );
}

// --- Rebuilding and defence -----------------------------------------------------------------

#[test]
fn the_ai_rebuilds_a_building_it_lost() {
    // The computer player completes its build order; then the test's player destroys one of its
    // power buildings with an explicit attack (all through commands), and the computer player
    // places a new one.
    let mut data = data();
    data.skirmish.factions[0].units = vec![sim::StartingGroup {
        kind: data.definitions.unit_kind("tank").expect("tank"),
        offset: (4, 1),
        count: 12,
    }];
    // Waves start late, so that the test's tanks meet the computer player's units only at home.
    for difficulty in &mut data.ai.difficulties {
        difficulty.first_wave = 20_000;
    }
    let mut app = fixture_match(&data, &[human(P0, 0, 0), computer(P1, 1, 1, 1)]);
    let power = data.ai.power;
    for _ in 0..BUILD_ORDER_BUDGET {
        tick(&mut app, Vec::new());
        if build_order_complete(&mut app, &data.ai, P1) {
            break;
        }
    }
    assert!(build_order_complete(&mut app, &data.ai, P1));
    let target = {
        let mut query = app.world_mut().query::<(&SimId, &Owner, &Building)>();
        let mut powers: Vec<(SimId, sim::Footprint)> = query
            .iter(app.world())
            .filter(|(_, owner, building)| owner.0 == P1 && building.kind == power)
            .map(|(id, _, building)| (*id, building.footprint))
            .collect();
        powers.sort_unstable_by_key(|(id, _)| *id);
        powers[0]
    };
    let tanks: Vec<SimId> = {
        let mut query = app.world_mut().query::<(&SimId, &Owner, &Unit)>();
        let mut found: Vec<SimId> = query
            .iter(app.world())
            .filter(|(_, owner, _)| owner.0 == P0)
            .map(|(id, ..)| *id)
            .collect();
        found.sort_unstable();
        found
    };
    // Move next to the power building, then attack it as soon as it is visible.
    let near = CellPos::new(target.1.origin.x - 1, target.1.origin.y);
    tick(
        &mut app,
        vec![IssuedCommand {
            player: P0,
            command: Command::Move {
                units: tanks.clone(),
                target: near,
            },
        }],
    );
    let mut attacked = false;
    let mut destroyed_at = None;
    let mut rebuilt_at = None;
    for at in 0..6_000_u64 {
        let mut commands = Vec::new();
        let visible = app
            .world()
            .resource::<Players>()
            .get(P0)
            .is_some_and(|state| state.vision().sees_footprint(target.1));
        let alive: Vec<SimId> = {
            let mut query = app.world_mut().query::<&SimId>();
            let all: BTreeSet<SimId> = query.iter(app.world()).copied().collect();
            tanks
                .iter()
                .copied()
                .filter(|id| all.contains(id))
                .collect()
        };
        if !attacked && visible && destroyed_at.is_none() && !alive.is_empty() {
            commands.push(IssuedCommand {
                player: P0,
                command: Command::Attack {
                    units: alive.clone(),
                    target: target.0,
                },
            });
            attacked = true;
        }
        let (events, _) = tick(&mut app, commands);
        for event in events {
            match event {
                SimEvent::BuildingDestroyed { building, .. } if building == target.0 => {
                    destroyed_at = Some(at);
                }
                SimEvent::BuildingPlaced { player, kind, .. }
                    if player == P1 && kind == power && destroyed_at.is_some() =>
                {
                    rebuilt_at = Some(at);
                }
                _ => {}
            }
        }
        if destroyed_at == Some(at) {
            // Withdraw: a plain move does not fire, so nothing else of the base is attacked.
            let alive: Vec<SimId> = {
                let mut query = app.world_mut().query::<&SimId>();
                let all: BTreeSet<SimId> = query.iter(app.world()).copied().collect();
                tanks
                    .iter()
                    .copied()
                    .filter(|id| all.contains(id))
                    .collect()
            };
            if !alive.is_empty() {
                tick(
                    &mut app,
                    vec![IssuedCommand {
                        player: P0,
                        command: Command::Move {
                            units: alive,
                            target: CellPos::new(5, 12),
                        },
                    }],
                );
            }
        }
        if rebuilt_at.is_some() {
            break;
        }
    }
    let destroyed_at = destroyed_at.expect("the power building was destroyed");
    let rebuilt_at = rebuilt_at.expect("the computer player placed a new power building");
    assert!(rebuilt_at > destroyed_at);
    assert!(owned_kinds(&mut app, P1).get(&power).copied().unwrap_or(0) >= 2);
}

#[test]
fn the_ai_attacks_a_visible_enemy_near_its_base_with_its_army_at_home() {
    let data = data();
    let mut app = fixture_match(&data, &[human(P0, 0, 0), computer(P1, 1, 1, 1)]);
    // The computer player's riflemen gather first.
    for _ in 0..200 {
        tick(&mut app, Vec::new());
    }
    let radius = data.ai.difficulties[1].defence_radius;
    let tanks: Vec<SimId> = {
        let tank = data.definitions.unit_kind("tank").expect("tank");
        let mut query = app.world_mut().query::<(&SimId, &Owner, &Unit)>();
        let mut found: Vec<SimId> = query
            .iter(app.world())
            .filter(|(_, owner, unit)| owner.0 == P0 && unit.kind == tank)
            .map(|(id, ..)| *id)
            .collect();
        found.sort_unstable();
        found
    };
    tick(
        &mut app,
        vec![IssuedCommand {
            player: P0,
            command: Command::Move {
                units: tanks.clone(),
                target: CellPos::new(28, 26),
            },
        }],
    );
    let mut defended = None;
    for at in 0..3_000 {
        let (_, pushed) = tick(&mut app, Vec::new());
        // The computer player's view of the tanks this tick (as of the vision it used).
        for issued in pushed {
            if let Command::AttackMove { target, .. } = issued.command {
                let mut query = app.world_mut().query::<(&SimId, &Mobile)>();
                let near_tank = query.iter(app.world()).any(|(id, mobile)| {
                    tanks.contains(id)
                        && (mobile.cell.x - target.x).abs() <= 1
                        && (mobile.cell.y - target.y).abs() <= 1
                });
                if near_tank {
                    defended = Some(at);
                }
            }
        }
        if defended.is_some() {
            break;
        }
    }
    assert!(
        defended.is_some(),
        "no attack on the tanks (radius {radius})"
    );
}

// --- Fog of war -----------------------------------------------------------------------------

#[test]
fn the_ai_ignores_what_its_player_does_not_see() {
    // Two matches that differ only in an unarmed enemy unit standing near the computer player's
    // base, within its defence radius, but where it does not see it, and an enemy building in the
    // shroud. As long as the computer player sees neither, its commands are identical in both
    // matches, and it does not remember the building.
    let data = data();
    let setup = [human(P0, 0, 0), computer(P1, 1, 1, 1)];
    let mut plain = fixture_match(&data, &setup);
    let mut hidden = fixture_match(&data, &setup);
    // The plain match allocates the two ids the hidden one gives the crawler and the building, so
    // that both give their later units and buildings the same ids (scenario setup).
    for _ in 0..2 {
        plain
            .world_mut()
            .resource_mut::<sim::SimIdAllocator>()
            .allocate()
            .expect("id");
    }
    let crawler_cell = CellPos::new(39, 25);
    let crawler = spawn_unit(hidden.world_mut(), P0, "crawler", crawler_cell).expect("crawler");
    let beacon =
        sim::spawn_building(hidden.world_mut(), P0, "beacon", CellPos::new(39, 2)).expect("beacon");
    let controller_start = hidden
        .world()
        .resource::<AiPlayers>()
        .controller(P1)
        .expect("controller")
        .start();
    let radius = data.ai.difficulties[1].defence_radius as i32;
    assert!(
        (crawler_cell.x - (controller_start.x + 2)).abs() <= radius
            && (crawler_cell.y - controller_start.y).abs() <= radius,
        "the crawler stands within the defence radius of the yard"
    );
    let mut compared = 0;
    for at in 0..2_000 {
        let seen = |app: &App| {
            let vision = app
                .world()
                .resource::<Players>()
                .get(P1)
                .expect("player")
                .vision()
                .clone();
            vision.is_visible(crawler_cell) || vision.is_visible(CellPos::new(39, 2))
        };
        // The vision the computer player decides with in the next tick.
        if seen(&hidden) {
            break;
        }
        let (_, plain_pushed) = tick(&mut plain, Vec::new());
        let (_, hidden_pushed) = tick(&mut hidden, Vec::new());
        assert_eq!(plain_pushed, hidden_pushed, "tick {at}");
        let known = hidden
            .world()
            .resource::<AiPlayers>()
            .controller(P1)
            .expect("controller")
            .known_buildings()
            .clone();
        assert!(!known.contains_key(&beacon), "tick {at}");
        compared += 1;
    }
    assert!(compared >= 500, "only {compared} ticks compared");
    // The crawler is still where it was: nothing moved it.
    let mut query = hidden.world_mut().query::<(&SimId, &Mobile)>();
    assert!(
        query
            .iter(hidden.world())
            .any(|(id, mobile)| *id == crawler && mobile.cell == crawler_cell)
    );
}

// --- End to end -----------------------------------------------------------------------------

/// The matches of the end-to-end tests: computer players of every pairing of the fixture
/// difficulties, with the fixture factions at both starts.
fn pairings(data: &Data) -> Vec<[PlayerSetup; 2]> {
    let count = data.ai.difficulties.len();
    let mut found = Vec::new();
    for first in 0..count {
        for second in 0..count {
            found.push([computer(P0, 0, 0, first), computer(P1, 1, 1, second)]);
        }
    }
    found
}

/// Runs the match twice and checks the end-to-end requirements: a winner within the tick limit,
/// equal hashes on every tick of both runs, and every rejected command explained.
fn assert_ends_with_a_winner_deterministically(name: &str, make: impl Fn() -> App) {
    assert_ends_within(name, MATCH_TICK_LIMIT, make);
}

/// [`assert_ends_with_a_winner_deterministically`] with a tick limit of `limit`.
fn assert_ends_within(name: &str, limit: u64, make: impl Fn() -> App) {
    let first = run_match_within(make(), limit);
    let second = run_match_within(make(), limit);
    assert!(
        matches!(first.outcome, Some(MatchOutcome::Won { .. })),
        "{name}: {:?} after {} ticks",
        first.outcome,
        first.ticks
    );
    assert_eq!(first.outcome, second.outcome, "{name}");
    assert_eq!(first.ticks, second.ticks, "{name}");
    assert_eq!(first.hashes, second.hashes, "{name}: hashes differ");
    assert_rejections_explained(&first);
}

#[test]
fn ai_against_ai_matches_on_the_fixture_map_end_with_a_winner() {
    let data = data();
    for (index, players) in pairings(&data).into_iter().enumerate() {
        assert_ends_with_a_winner_deterministically(&format!("fixture pairing {index}"), || {
            fixture_match(&data, &players)
        });
    }
}

#[test]
fn ai_against_ai_matches_on_every_shipped_map_end_with_a_winner() {
    // The shipped maps are those of the production skirmish file, with its start positions; the
    // factions, units, buildings, and AI are fixture data (invariant I10).
    let production = load_match_definitions(
        assets().join("data/units.ron"),
        assets().join("data/economy.ron"),
        assets().join("data/combat.ron"),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let shipped = load_skirmish(assets().join("data/skirmish.ron"), &production)
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(!shipped.maps.is_empty());
    let mut data = data();
    data.skirmish.maps = shipped.maps.clone();
    // The matches are independent of each other, so they are played on all cores; a failed
    // assertion of any of them fails the test when the workers are joined.
    let mut jobs = Vec::new();
    for (map, def) in shipped.maps.iter().enumerate() {
        let grid = load_map(assets().join(map_asset_path(&def.id)))
            .unwrap_or_else(|error| panic!("{error}"))
            .grid;
        for (index, players) in pairings(&data).into_iter().enumerate() {
            let name = format!("map {}, pairing {index}", def.id);
            jobs.push((name, map, grid.clone(), players));
        }
    }
    let (data, jobs) = (&data, &jobs);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let played = std::sync::atomic::AtomicUsize::new(0);
    let workers = std::thread::available_parallelism().map_or(4, usize::from);
    std::thread::scope(|scope| {
        for _ in 0..workers.min(jobs.len()) {
            scope.spawn(|| {
                while let Some((name, map, grid, players)) =
                    jobs.get(next.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
                {
                    assert_ends_within(name, tick_limit(grid), || {
                        match_on(data, grid.clone(), *map, players)
                    });
                    played.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            });
        }
    });
    assert_eq!(played.into_inner(), jobs.len());
}

#[test]
fn a_run_with_another_difficulty_takes_another_course() {
    // The hashes of the end-to-end test are not trivially equal: a different difficulty changes
    // them from the first tick (it is part of the state, so this holds by construction), and the
    // matches differ in their course: the computer player whose difficulty differs issues other
    // commands, and the match ends in another tick. (Until M6 review 1 P2-3 the test was named for
    // a different seed too and asserted only the first hash; the seed is not claimed, because the
    // computer players use no randomness, implementation debt item 2.)
    let data = data();
    let one = run_match(fixture_match(
        &data,
        &[computer(P0, 0, 0, 0), computer(P1, 1, 1, 1)],
    ));
    let other = run_match(fixture_match(
        &data,
        &[computer(P0, 0, 0, 1), computer(P1, 1, 1, 1)],
    ));
    assert_ne!(one.hashes[0], other.hashes[0]);
    assert_ne!(one.ticks, 0);
    let pushed_one: usize = one.pushed.values().sum();
    assert!(pushed_one > 0);
    // The course differs.
    assert!(
        one.commands
            .get(&P0)
            .is_some_and(|commands| !commands.is_empty())
    );
    assert_ne!(one.commands.get(&P0), other.commands.get(&P0));
    assert_ne!(one.ticks, other.ticks);
}
