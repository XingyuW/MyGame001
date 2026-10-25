//! RA5: crates, on the RA5 fixture through `CommandQueue` and `SimSchedule`
//! (docs/RA2_PARITY_PLAN.md sections 4.11 and 5, RA5; docs/DECISIONS.md R60). The fixture's
//! rules: at most 3 crates, one at the start of a match, a taken one replaced after 50 to 200
//! ticks; a crate reaches 3 cells; money is 2000 and up to 900 more; the unit of a crate is a
//! tank, a heavy tank or an Allied miner; veteran one rank; armour x 1.5, firepower x 2,
//! speed x 1.2. A test that needs one content gives the others a share of 0
//! (`ra5_common::only`).

mod ra5_common;

use bevy_app::App;
use ra5_common::*;
use sim::{
    CRATE_MONEY_SPREAD, CellPos, CrateContent, Crates, ELITE, FREE_MCV_CREDITS, Grid, MatchOptions,
    MatchState, PlayerId, Players, SimEvent, SimId, VETERAN, state_hash,
};

/// A world without a match in which a crate holds only `content`, seeded with `seed`. Both
/// players own a power plant in a corner of the map, so that neither is a player without a
/// base, to whom a crate gives a base unit instead of its content.
fn world_seeded(content: &str, seed: u64) -> App {
    let mut app = world_of(
        definitions_with(&format!("only_{content}"), &only(content)),
        seed,
    );
    place(&mut app, P0, "allied_power", 0, 38);
    place(&mut app, P1, "soviet_power", 38, 38);
    app
}

fn world_with(content: &str) -> App {
    world_seeded(content, 7)
}

#[test]
fn concurrent_fixture_variants_keep_each_readers_requested_rules() {
    let written = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        for content in ["money", "veteran"] {
            let written = &written;
            scope.spawn(move || {
                let path = variant("parallel_contents", "rules.ron", &only(content));
                written.wait();
                let definitions = load(fixture("economy.ron"), path).expect("independent rules");
                let enabled: Vec<_> = definitions
                    .rules()
                    .unwrap()
                    .powerups
                    .iter()
                    .filter(|entry| entry.share > 0)
                    .map(|entry| entry.id.as_str())
                    .collect();
                assert_eq!(enabled, vec![content]);
            });
        }
    });
}

/// Sends `unit` of `player` to (`x`, `y`) and runs until a crate is taken, at most 400 ticks:
/// what the crate held, and the ticks it took.
fn fetch(
    app: &mut App,
    player: PlayerId,
    unit: SimId,
    x: i32,
    y: i32,
) -> ((SimId, CrateContent, i64, Option<SimId>), u32) {
    let mut events = tick(app, vec![move_to(player, unit, x, y)]);
    assert!(rejections(&events).is_empty(), "{events:?}");
    for ran in 1..=400 {
        if let Some(found) = taken(&events).first() {
            return (*found, ran);
        }
        events = tick(app, Vec::new());
    }
    panic!("no crate was taken");
}

// --- Positive: what a crate holds ------------------------------------------------------------

#[test]
fn a_crate_of_money_pays_its_value_and_a_draw() {
    let mut seen = std::collections::BTreeSet::new();
    for seed in 0..12 {
        let mut app = world_seeded("money", seed);
        let tank = spawn(&mut app, P0, "tank", 10, 10);
        crate_at(&mut app, 12, 10);
        let ((who, content, paid, new), _) = fetch(&mut app, P0, tank, 12, 10);
        assert_eq!((who, content, new), (tank, CrateContent::Money, None));
        assert!(
            (2_000..=2_000 + i64::from(CRATE_MONEY_SPREAD)).contains(&paid),
            "{paid}"
        );
        assert_eq!(credits(&app, P0), 5_000 + paid);
        assert_eq!(credits(&app, P1), 5_000);
        assert!(crates(&app).is_empty(), "the crate is gone");
        seen.insert(paid);
    }
    // The draw is a draw: twelve seeds do not pay the same.
    assert!(seen.len() > 6, "{seen:?}");
}

#[test]
fn a_crate_of_a_unit_gives_one_of_its_types_beside_the_crate() {
    let mut kinds = std::collections::BTreeSet::new();
    for seed in 0..12 {
        let mut app = world_seeded("unit", seed);
        let tank = spawn(&mut app, P0, "tank", 10, 10);
        crate_at(&mut app, 12, 10);
        let before = units_of(&mut app, P0);
        let ((who, content, paid, new), _) = fetch(&mut app, P0, tank, 12, 10);
        assert_eq!((who, content, paid), (tank, CrateContent::Unit, 0));
        let new = new.expect("a unit");
        let after = units_of(&mut app, P0);
        assert_eq!(after.len(), before.len() + 1);
        let (_, kind) = after
            .iter()
            .find(|(id, _)| *id == new)
            .expect("the new unit is P0's");
        assert!(
            ["tank", "heavy", "miner_a"].contains(&kind.as_str()),
            "{kind}"
        );
        let cell = cell_of(&mut app, new);
        assert!(
            (cell.x - 12).abs() <= 1 && (cell.y - 10).abs() <= 1,
            "{cell:?}"
        );
        assert_ne!(cell, CellPos::new(12, 10), "the collector holds the cell");
        assert_eq!(credits(&app, P0), 5_000);
        assert_cells(&mut app, "a crate's unit");
        kinds.insert(kind.clone());
    }
    assert!(kinds.len() >= 2, "{kinds:?}");
}

#[test]
fn a_veteran_crate_promotes_the_owners_units_within_three_cells() {
    let mut app = world_with("veteran");
    let collector = spawn(&mut app, P0, "tank", 10, 10);
    let near = spawn(&mut app, P0, "rifleman", 15, 10);
    let far = spawn(&mut app, P0, "rifleman", 15, 11);
    let veteran = spawn(&mut app, P0, "heavy", 12, 13);
    let untrainable = spawn(&mut app, P0, "miner_a", 12, 8);
    let enemy = spawn(&mut app, P1, "heavy", 13, 10);
    tick(&mut app, Vec::new());
    assert!(sim::promote(app.world_mut(), veteran, VETERAN));
    crate_at(&mut app, 12, 10);
    let ((who, content, paid, new), _) = fetch(&mut app, P0, collector, 12, 10);
    assert_eq!(
        (who, content, paid, new),
        (collector, CrateContent::Veteran, 0, None)
    );
    // (15, 10) is three cells from the crate, (15, 11) further; (12, 13) is three cells.
    assert_eq!(experience(&mut app, collector).rank, VETERAN);
    assert_eq!(experience(&mut app, near).rank, VETERAN);
    assert_eq!(experience(&mut app, far).rank, 0);
    assert_eq!(experience(&mut app, veteran).rank, ELITE);
    assert_eq!(experience(&mut app, untrainable).rank, 0);
    assert_eq!(experience(&mut app, enemy).rank, 0);
    assert_eq!(credits(&app, P0), 5_000);
}

#[test]
fn armour_firepower_and_speed_crates_set_the_multiplier_of_their_kind() {
    for (content, expected) in [
        ("armour", (1000, 1500, 1000)),
        ("firepower", (2000, 1000, 1000)),
        ("speed", (1000, 1000, 1200)),
    ] {
        let mut app = world_with(content);
        let collector = spawn(&mut app, P0, "tank", 10, 10);
        let near = spawn(&mut app, P0, "rifleman", 12, 13);
        let far = spawn(&mut app, P0, "rifleman", 13, 13);
        let enemy = spawn(&mut app, P1, "heavy", 13, 10);
        crate_at(&mut app, 12, 10);
        let ((_, got, paid, _), _) = fetch(&mut app, P0, collector, 12, 10);
        assert_eq!(got.id(), content);
        assert_eq!(paid, 0);
        let bonus = |app: &mut App, unit| {
            let bonus = experience(app, unit).bonus;
            (bonus.firepower_pm, bonus.armour_pm, bonus.speed_pm)
        };
        assert_eq!(bonus(&mut app, collector), expected, "{content}");
        assert_eq!(bonus(&mut app, near), expected, "{content}");
        assert_eq!(bonus(&mut app, far), (1000, 1000, 1000), "{content}");
        assert_eq!(bonus(&mut app, enemy), (1000, 1000, 1000), "{content}");
        assert_eq!(experience(&mut app, collector).bonus.reload_pm, 1000);
        // The multiplier acts: it is part of the product the simulation reads.
        let own = experience(&mut app, collector);
        assert_eq!(
            (own.firepower_pm(), own.armour_pm(), own.speed_pm()),
            expected
        );
    }
}

#[test]
fn a_heal_crate_makes_everything_of_its_collectors_owner_whole() {
    let mut app = world_with("heal_base");
    let collector = spawn(&mut app, P0, "tank", 10, 10);
    let soldier = spawn(&mut app, P0, "rifleman", 5, 30);
    let yard = place(&mut app, P0, "allied_yard", 20, 20);
    let enemy = spawn(&mut app, P1, "heavy", 30, 10);
    let enemy_yard = place(&mut app, P1, "soviet_yard", 30, 30);
    tick(&mut app, Vec::new());
    let whole: Vec<i32> = [collector, soldier, yard, enemy, enemy_yard]
        .map(|id| hit_points(&mut app, id))
        .to_vec();
    for id in [collector, soldier, yard, enemy, enemy_yard] {
        set_hit_points(&mut app, id, 7);
    }
    crate_at(&mut app, 12, 10);
    let ((_, content, paid, _), _) = fetch(&mut app, P0, collector, 12, 10);
    assert_eq!((content, paid), (CrateContent::HealBase, 0));
    assert_eq!(hit_points(&mut app, collector), whole[0]);
    assert_eq!(
        hit_points(&mut app, soldier),
        whole[1],
        "anywhere on the map"
    );
    assert_eq!(hit_points(&mut app, yard), whole[2]);
    assert_eq!(hit_points(&mut app, enemy), 7);
    assert_eq!(hit_points(&mut app, enemy_yard), 7);
}

#[test]
fn a_reveal_crate_explores_the_whole_map_for_its_collectors_owner() {
    let mut app = world_with("reveal");
    let collector = spawn(&mut app, P0, "tank", 10, 10);
    spawn(&mut app, P1, "heavy", 30, 30);
    tick(&mut app, Vec::new());
    let explored = |app: &App, player| {
        app.world()
            .resource::<Players>()
            .get(player)
            .expect("a player")
            .vision()
            .explored_cells()
    };
    assert!(explored(&app, P0) < 400);
    let enemy_before = explored(&app, P1);
    crate_at(&mut app, 12, 10);
    let ((_, content, _, _), _) = fetch(&mut app, P0, collector, 12, 10);
    assert_eq!(content, CrateContent::Reveal);
    assert_eq!(explored(&app, P0), 1_600);
    assert_eq!(explored(&app, P1), enemy_before);
    // Explored, not seen: what is visible is still what the units see.
    let vision = app.world().resource::<Players>().get(P0).expect("a player");
    assert!(vision.vision().visible_cells() < 400);
    assert!(!vision.vision().is_visible(CellPos::new(30, 30)));
}

// --- Money instead ---------------------------------------------------------------------------

#[test]
fn a_collector_that_cannot_gain_gets_money_instead() {
    let money = |paid: i64| (2_000..=2_900).contains(&paid);
    // An elite collector and a veteran crate; the unit beside it is not promoted either.
    let mut app = world_with("veteran");
    let collector = spawn(&mut app, P0, "tank", 10, 10);
    let near = spawn(&mut app, P0, "rifleman", 12, 11);
    tick(&mut app, Vec::new());
    assert!(sim::promote(app.world_mut(), collector, ELITE));
    crate_at(&mut app, 12, 10);
    let ((_, content, paid, _), _) = fetch(&mut app, P0, collector, 12, 10);
    assert_eq!(content, CrateContent::Money);
    assert!(money(paid), "{paid}");
    assert_eq!(credits(&app, P0), 5_000 + paid);
    assert_eq!(experience(&mut app, near).rank, 0);

    // A second armour crate: the collector has the multiplier already. A unit beside it that
    // had none keeps none, since the crate turned into money.
    let mut app = world_with("armour");
    let collector = spawn(&mut app, P0, "tank", 10, 10);
    crate_at(&mut app, 12, 10);
    crate_at(&mut app, 14, 10);
    let ((_, content, paid, _), _) = fetch(&mut app, P0, collector, 12, 10);
    assert_eq!((content, paid), (CrateContent::Armour, 0));
    let late = spawn(&mut app, P0, "rifleman", 14, 11);
    let ((_, content, paid, _), _) = fetch(&mut app, P0, collector, 14, 10);
    assert_eq!(content, CrateContent::Money);
    assert!(money(paid), "{paid}");
    assert_eq!(experience(&mut app, collector).bonus.armour_pm, 1500);
    assert_eq!(experience(&mut app, late).bonus.armour_pm, 1000);

    // A collector that is not trainable (the Allied miner), for each of the four.
    for content in ["veteran", "armour", "firepower", "speed"] {
        let mut app = world_with(content);
        let miner = spawn(&mut app, P0, "miner_a", 10, 10);
        let near = spawn(&mut app, P0, "tank", 12, 11);
        crate_at(&mut app, 12, 10);
        let ((who, got, paid, _), _) = fetch(&mut app, P0, miner, 12, 10);
        assert_eq!((who, got), (miner, CrateContent::Money), "{content}");
        assert!(money(paid), "{content}: {paid}");
        let untouched = experience(&mut app, near);
        assert_eq!(untouched.rank, 0, "{content}");
        assert_eq!(untouched.bonus, sim::Multipliers::NONE, "{content}");
    }
}

#[test]
fn a_crate_of_a_unit_is_money_on_water_without_room_and_without_types() {
    let water = sim::Cell {
        passable: false,
        water: true,
        buildable: false,
        ..OPEN
    };
    // On water: a boat takes it.
    let mut app = world_with("unit");
    for y in 0..40 {
        for x in 20..40 {
            terrain(&mut app, x, y, water);
        }
    }
    let boat = spawn(&mut app, P0, "boat", 25, 10);
    crate_at(&mut app, 27, 10);
    let before = units_of(&mut app, P0).len();
    let ((who, content, paid, new), _) = fetch(&mut app, P0, boat, 27, 10);
    assert_eq!((who, content, new), (boat, CrateContent::Money, None));
    assert!((2_000..=2_900).contains(&paid));
    assert_eq!(units_of(&mut app, P0).len(), before);

    // On land without room within three cells: the crate lies in the one cell that is not
    // rock, and the tank is put onto it (scenario setup), which is how it takes it.
    let mut app = world_with("unit");
    let rock = sim::Cell {
        passable: false,
        buildable: false,
        ..OPEN
    };
    for y in 0..36 {
        for x in 0..40 {
            if (x, y) != (12, 10) {
                terrain(&mut app, x, y, rock);
            }
        }
    }
    crate_at(&mut app, 12, 10);
    let tank = spawn(&mut app, P0, "tank", 12, 10);
    let before = units_of(&mut app, P0).len();
    let events = tick(&mut app, Vec::new());
    let found = taken(&events);
    assert_eq!(found.len(), 1, "{events:?}");
    let (who, content, paid, new) = found[0];
    assert_eq!((who, content, new), (tank, CrateContent::Money, None));
    assert!((2_000..=2_900).contains(&paid));
    assert_eq!(units_of(&mut app, P0).len(), before);
    assert_cells(&mut app, "no room for a crate's unit");
    // The same with one free cell at the edge of the reach, three cells away: the unit.
    let mut app = world_with("unit");
    for y in 0..36 {
        for x in 0..40 {
            if (x, y) != (12, 10) && (x, y) != (15, 13) {
                terrain(&mut app, x, y, rock);
            }
        }
    }
    crate_at(&mut app, 12, 10);
    spawn(&mut app, P0, "tank", 12, 10);
    let events = tick(&mut app, Vec::new());
    let (_, content, _, new) = taken(&events)[0];
    assert_eq!(content, CrateContent::Unit);
    assert_eq!(
        cell_of(&mut app, new.expect("a unit")),
        CellPos::new(15, 13)
    );

    // Without a type to draw.
    let mut edits = only("unit");
    edits.push((
        r#"crate_units: ["tank", "heavy", "miner_a"],"#,
        "crate_units: [],",
    ));
    let mut app = world_of(definitions_with("unit_no_types", &edits), 7);
    place(&mut app, P0, "allied_power", 0, 38);
    let tank = spawn(&mut app, P0, "tank", 10, 10);
    crate_at(&mut app, 12, 10);
    let ((_, content, _, new), _) = fetch(&mut app, P0, tank, 12, 10);
    assert_eq!((content, new), (CrateContent::Money, None));
}

// --- The free base unit ----------------------------------------------------------------------

/// A world in which P0 has `credits`, a rifleman at (10, 10) and a crate of armour at (12, 10).
fn stranded(name: &str, edits: &[(&'static str, &'static str)], credits: i64) -> (App, SimId) {
    let mut all = only("armour");
    all.extend_from_slice(edits);
    let mut app = world_with_credits(definitions_with(name, &all), 7, [credits, credits]);
    let soldier = spawn(&mut app, P0, "rifleman", 10, 10);
    crate_at(&mut app, 12, 10);
    (app, soldier)
}

#[test]
fn a_player_without_base_and_base_unit_gets_the_base_unit_of_its_side() {
    for (player, id) in [(P0, "allied_mcv"), (P1, "soviet_mcv")] {
        // Neither player owns anything but the soldier.
        let mut app = world_of(definitions_with("only_armour", &only("armour")), 7);
        let soldier = spawn(
            &mut app,
            player,
            if player == P0 {
                "rifleman"
            } else {
                "conscript"
            },
            10,
            10,
        );
        crate_at(&mut app, 12, 10);
        let ((who, content, paid, new), _) = fetch(&mut app, player, soldier, 12, 10);
        assert_eq!((who, content, paid), (soldier, CrateContent::Unit, 0));
        let new = new.expect("a base unit");
        assert!(
            units_of(&mut app, player)
                .iter()
                .any(|(unit, kind)| *unit == new && kind == id),
            "{id}"
        );
        // No armour: the base unit took the place of the content.
        assert_eq!(experience(&mut app, soldier).bonus, sim::Multipliers::NONE);
        assert_eq!(credits(&app, player), 5_000, "the base unit is free");
        assert_cells(&mut app, "a free base unit");
    }
}

#[test]
fn the_free_base_unit_needs_more_than_1500_credits_and_nothing_to_build_with() {
    let armour = |app: &mut App, soldier: SimId| {
        let ((_, content, _, new), _) = fetch(app, P0, soldier, 12, 10);
        (content, new.is_some())
    };
    // Exactly 1500 credits: the crate's own content. One more: the base unit.
    let (mut app, soldier) = stranded("poor", &[], FREE_MCV_CREDITS);
    assert_eq!(armour(&mut app, soldier), (CrateContent::Armour, false));
    let (mut app, soldier) = stranded("rich", &[], FREE_MCV_CREDITS + 1);
    assert_eq!(armour(&mut app, soldier), (CrateContent::Unit, true));
    // The rule off.
    let (mut app, soldier) = stranded("no_rule", &[("free_mcv: 1,", "free_mcv: 0,")], 5_000);
    assert_eq!(armour(&mut app, soldier), (CrateContent::Armour, false));
    // With a building.
    let (mut app, soldier) = stranded("housed", &[], 5_000);
    place(&mut app, P0, "allied_power", 30, 30);
    assert_eq!(armour(&mut app, soldier), (CrateContent::Armour, false));
    // With a base unit, of either side.
    for id in ["allied_mcv", "soviet_mcv"] {
        let (mut app, soldier) = stranded(&format!("mobile_{id}"), &[], 5_000);
        spawn(&mut app, P0, id, 30, 30);
        assert_eq!(armour(&mut app, soldier), (CrateContent::Armour, false));
    }
    // A building or base unit of the enemy does not count.
    let (mut app, soldier) = stranded("alone", &[], 5_000);
    place(&mut app, P1, "soviet_power", 30, 30);
    spawn(&mut app, P1, "soviet_mcv", 25, 25);
    assert_eq!(armour(&mut app, soldier), (CrateContent::Unit, true));
}

// --- Negative: who takes a crate, and where one may lie --------------------------------------

#[test]
fn an_aircraft_does_not_take_a_crate_and_a_unit_takes_it_when_it_arrives() {
    let mut app = world_with("money");
    crate_at(&mut app, 12, 10);
    let gunship = spawn(&mut app, P0, "gunship", 12, 10);
    let events = ticks(&mut app, 60);
    assert!(taken(&events).is_empty());
    assert_eq!(crates(&app), [CellPos::new(12, 10)]);
    assert_eq!(cell_of(&mut app, gunship), CellPos::new(12, 10));
    // A tank on its way: the crate lies there until the tank is in the cell.
    let tank = spawn(&mut app, P0, "tank", 8, 10);
    let mut events = tick(&mut app, vec![move_to(P0, tank, 14, 10)]);
    let mut ran = 0;
    while taken(&events).is_empty() {
        assert_ne!(cell_of(&mut app, tank), CellPos::new(12, 10));
        assert_eq!(crates(&app).len(), 1);
        events = tick(&mut app, Vec::new());
        ran += 1;
        assert!(ran < 200);
    }
    assert_eq!(cell_of(&mut app, tank), CellPos::new(12, 10));
    assert_eq!(taken(&events)[0].0, tank, "passing through is enough");
    assert!(crates(&app).is_empty());
}

#[test]
fn a_crate_lies_only_in_a_free_cell_of_land_or_water() {
    let mut app = world();
    terrain(
        &mut app,
        5,
        5,
        sim::Cell {
            passable: false,
            buildable: false,
            ..OPEN
        },
    );
    terrain(
        &mut app,
        6,
        5,
        sim::Cell {
            passable: false,
            water: true,
            buildable: false,
            ..OPEN
        },
    );
    ore(&mut app, 7, 5, 3);
    place(&mut app, P0, "turret", 8, 5);
    spawn(&mut app, P0, "tank", 9, 5);
    spawn(&mut app, P0, "rifleman", 10, 5);
    let grid = app.world().resource::<Grid>().clone();
    let mut put = |x: i32, y: i32| {
        app.world_mut()
            .resource_mut::<Crates>()
            .place(&grid, CellPos::new(x, y))
    };
    assert!(!put(5, 5), "rock");
    assert!(put(6, 5), "water");
    assert!(!put(6, 5), "a crate lies there already");
    assert!(!put(7, 5), "ore");
    assert!(!put(8, 5), "a building");
    assert!(!put(9, 5), "a vehicle");
    assert!(!put(10, 5), "a soldier");
    assert!(!put(-1, 0) && !put(40, 0) && !put(0, 40), "outside the map");
    assert!(put(11, 5));
    assert_eq!(crates(&app), [CellPos::new(6, 5), CellPos::new(11, 5)]);
}

// --- Crates of a match -----------------------------------------------------------------------

/// A match of two human players on the fixture map with the rules edits `rules`.
fn match_with(name: &str, rules: &[(&str, &str)], options: &MatchOptions, seed: u64) -> App {
    let data = data_with(name, rules);
    let players = [human(P0, ALPHA, 0), human(P1, GAMMA, 1)];
    match_on(&data, fixture_map(), &players, options, seed)
}

fn appeared(events: &[SimEvent]) -> Vec<CellPos> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::CrateAppeared { cell } => Some(*cell),
            _ => None,
        })
        .collect()
}

#[test]
fn a_match_starts_with_a_crate_that_lies_its_time_and_is_replaced_when_taken() {
    let gone = |events: &[SimEvent]| -> Vec<CellPos> {
        events
            .iter()
            .filter_map(|event| match event {
                SimEvent::CrateGone { cell } => Some(*cell),
                _ => None,
            })
            .collect()
    };
    let mut taken_once = 0;
    for seed in [1, 2, 3, 4, 5, 6, 7, 8] {
        let mut app = match_with("match", &only("money"), &MatchOptions::default(), seed);
        assert!(crates(&app).is_empty(), "none before the first tick");
        let events = tick(&mut app, Vec::new());
        let cells = appeared(&events);
        assert_eq!(cells.len(), 1, "seed {seed}");
        assert_eq!(crates(&app), cells);
        let first = cells[0];
        // The cell: land or water, with nothing on it.
        let grid = app.world().resource::<Grid>().clone();
        let terrain = grid.cell(first).expect("inside the map");
        assert!(terrain.passable || terrain.water, "{first:?}");
        assert_eq!(terrain.ore, 0);
        assert!(grid.building(first).is_none() && grid.occupant(first).is_none());
        // It lies 50 to 200 ticks (half to twice the rules' 100), counted from the tick it
        // appeared in (index 0); nothing happens before, and in the tick its time is up it is
        // gone and another appears.
        let expires = app
            .world()
            .resource::<Crates>()
            .expires(first)
            .expect("a crate");
        assert!((50..=200).contains(&expires), "seed {seed}: {expires}");
        for index in 1..expires {
            let events = tick(&mut app, Vec::new());
            assert!(appeared(&events).is_empty() && gone(&events).is_empty());
            assert_eq!(crates(&app), [first], "tick {index}");
        }
        let events = tick(&mut app, Vec::new());
        assert_eq!(gone(&events), [first], "seed {seed}");
        let second = appeared(&events);
        assert_eq!(second.len(), 1);
        assert_eq!(crates(&app), second);
        let cell = second[0];
        let now = expires + 1;
        // A unit put beside the new crate takes it (scenario setup), if a tank can stand
        // there; the replacement is due 50 to 200 ticks later and appears in exactly that tick.
        let grid = app.world().resource::<Grid>().clone();
        if !grid.is_passable(cell) {
            continue;
        }
        let free = [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .into_iter()
            .map(|(dx, dy)| CellPos::new(cell.x + dx, cell.y + dy))
            .find(|&beside| grid.is_passable(beside) && grid.occupant(beside).is_none());
        let Some(beside) = free else {
            continue;
        };
        let lies_until = app
            .world()
            .resource::<Crates>()
            .expires(cell)
            .expect("a crate");
        let tank = spawn(&mut app, P0, "tank", beside.x, beside.y);
        let (_, ran) = fetch(&mut app, P0, tank, cell.x, cell.y);
        let taken_in = now + u64::from(ran) - 1;
        assert!(
            taken_in < lies_until,
            "the tank is there within a few ticks"
        );
        let due = app.world().resource::<Crates>().due().to_vec();
        assert_eq!(due.len(), 1);
        let wait = due[0] - taken_in;
        assert!((50..=200).contains(&wait), "seed {seed}: {wait}");
        assert!(crates(&app).is_empty());
        for index in taken_in + 1..due[0] {
            let events = tick(&mut app, Vec::new());
            assert!(
                appeared(&events).is_empty(),
                "seed {seed}: early at {index}"
            );
            assert!(gone(&events).is_empty(), "a taken crate is not gone twice");
        }
        let events = tick(&mut app, Vec::new());
        assert_eq!(appeared(&events).len(), 1, "seed {seed}: due at {}", due[0]);
        assert_eq!(crates(&app), appeared(&events));
        assert!(app.world().resource::<Crates>().due().is_empty());
        taken_once += 1;
    }
    assert!(
        taken_once >= 3,
        "the second half of the test ran for {taken_once} seeds"
    );
}

#[test]
fn no_crate_appears_with_the_option_off_or_without_a_match() {
    let off = MatchOptions {
        crates: false,
        ..MatchOptions::default()
    };
    let mut app = match_with("crates_off", &[], &off, 1);
    assert!(!app.world().resource::<MatchState>().rules().crates);
    let events = ticks(&mut app, 600);
    assert!(appeared(&events).is_empty());
    assert!(crates(&app).is_empty());
    assert_eq!(*app.world().resource::<Crates>(), Crates::default());
    // The same match with the option on has one at a time (the test can fail).
    let mut app = match_with("crates_on", &[], &MatchOptions::default(), 1);
    assert!(!appeared(&ticks(&mut app, 600)).is_empty());
    assert_eq!(crates(&app).len(), 1);
    // A world without a match: the rule is on, and nothing appears.
    let mut app = world();
    spawn(&mut app, P0, "tank", 10, 10);
    assert!(app.world().resource::<MatchState>().rules().crates);
    assert!(appeared(&ticks(&mut app, 600)).is_empty());
    assert_eq!(*app.world().resource::<Crates>(), Crates::default());
    // A crate put down in a match with the option off is still a crate: the option decides
    // what appears, not what a unit may take. It lies until it is taken, and is not replaced.
    let mut app = match_with("crates_off_put", &only("money"), &off, 1);
    tick(&mut app, Vec::new());
    let (mcv, _) = units_of(&mut app, P0)
        .into_iter()
        .find(|(_, kind)| kind == "allied_mcv")
        .expect("the base unit");
    let at = cell_of(&mut app, mcv);
    crate_at(&mut app, at.x + 2, at.y);
    assert_eq!(
        app.world()
            .resource::<Crates>()
            .expires(CellPos::new(at.x + 2, at.y)),
        Some(u64::MAX)
    );
    ticks(&mut app, 500);
    assert_eq!(crates(&app), [CellPos::new(at.x + 2, at.y)]);
    let ((who, content, _, _), _) = fetch(&mut app, P0, mcv, at.x + 2, at.y);
    assert_eq!((who, content), (mcv, CrateContent::Money));
    assert!(appeared(&ticks(&mut app, 400)).is_empty());
}

#[test]
fn a_match_keeps_its_minimum_and_never_exceeds_its_maximum() {
    // Three at the start with a minimum of 3 (the maximum is 3), and three in every tick
    // after it: one whose time is up is replaced in the same tick.
    let mut app = match_with(
        "three",
        &[("minimum: 1,", "minimum: 3,")],
        &MatchOptions::default(),
        4,
    );
    let events = tick(&mut app, Vec::new());
    assert_eq!(appeared(&events).len(), 3);
    let mut moved = 0;
    for _ in 0..600 {
        moved += appeared(&tick(&mut app, Vec::new())).len();
        assert_eq!(crates(&app).len(), 3);
    }
    assert!(moved >= 6, "every crate lay at most 200 ticks: {moved}");
    // A maximum of 2 with a minimum of 2, and a third put down by hand is over the maximum:
    // when that one is taken, two are left, which is the maximum, so the replacement that
    // comes due for it is dropped, and there are never more than two again.
    let mut edits = only("money");
    edits.push(("minimum: 1,", "minimum: 2,"));
    edits.push(("maximum: 3,", "maximum: 2,"));
    let mut app = match_with("two", &edits, &MatchOptions::default(), 4);
    tick(&mut app, Vec::new());
    assert_eq!(crates(&app).len(), 2);
    let (mcv, _) = units_of(&mut app, P0)
        .into_iter()
        .find(|(_, kind)| kind == "allied_mcv")
        .expect("the base unit");
    let at = cell_of(&mut app, mcv);
    crate_at(&mut app, at.x + 2, at.y);
    assert_eq!(crates(&app).len(), 3);
    fetch(&mut app, P0, mcv, at.x + 2, at.y);
    assert_eq!(crates(&app).len(), 2);
    assert_eq!(app.world().resource::<Crates>().due().len(), 1);
    for _ in 0..400 {
        tick(&mut app, Vec::new());
        assert_eq!(crates(&app).len(), 2);
    }
    assert!(app.world().resource::<Crates>().due().is_empty());
    // No crates at all with a maximum of 0.
    let mut app = match_with(
        "zero",
        &[
            ("minimum: 1,", "minimum: 0,"),
            ("maximum: 3,", "maximum: 0,"),
        ],
        &MatchOptions::default(),
        4,
    );
    assert!(appeared(&ticks(&mut app, 300)).is_empty());
}

#[test]
fn a_crate_under_a_new_building_is_gone_and_replaced_at_once() {
    // No starting units, so that the cells beside the base unit are free.
    let options = MatchOptions {
        unit_count: Some(0),
        ..MatchOptions::default()
    };
    let mut app = match_with("built_over", &[], &options, 2);
    tick(&mut app, Vec::new());
    // A crate put down beside P0's base unit, and a building placed on it (scenario setup;
    // a placement by command does not look at crates either).
    let (mcv, _) = units_of(&mut app, P0)
        .into_iter()
        .find(|(_, kind)| kind == "allied_mcv")
        .expect("the base unit");
    let at = cell_of(&mut app, mcv);
    let first = crates(&app);
    crate_at(&mut app, at.x + 3, at.y);
    assert_eq!(crates(&app).len(), 2);
    place(&mut app, P0, "turret", at.x + 3, at.y);
    let events = tick(&mut app, Vec::new());
    assert!(
        events
            .iter()
            .any(|event| matches!(event, SimEvent::CrateGone { cell } if *cell == CellPos::new(at.x + 3, at.y)))
    );
    // The lost crate is replaced in the same tick, somewhere a crate may lie.
    let now = crates(&app);
    assert_eq!(now.len(), 2);
    assert!(!now.contains(&CellPos::new(at.x + 3, at.y)));
    assert!(first.iter().all(|cell| now.contains(cell)));
    let grid = app.world().resource::<Grid>();
    assert!(now.iter().all(|&cell| grid.building(cell).is_none()));
}

// --- Determinism and the state hash ----------------------------------------------------------

#[test]
fn the_state_hash_covers_the_crates() {
    let mut plain = world();
    let mut with = world();
    assert_eq!(state_hash(plain.world_mut()), state_hash(with.world_mut()));
    crate_at(&mut with, 12, 10);
    let one = state_hash(with.world_mut());
    assert_ne!(state_hash(plain.world_mut()), one);
    let mut other = world();
    crate_at(&mut other, 12, 11);
    assert_ne!(state_hash(other.world_mut()), one);
    // A crate that is due: two matches that differ only in when the replacement comes.
    let run = |seed: u64| {
        let mut app = world_of(definitions_with("hash_due", &only("armour")), seed);
        let tank = spawn(&mut app, P0, "tank", 10, 10);
        crate_at(&mut app, 12, 10);
        fetch(&mut app, P0, tank, 12, 10);
        let due = app.world().resource::<Crates>().due().to_vec();
        (state_hash(app.world_mut()), due)
    };
    let (first, due) = run(1);
    assert_eq!(run(1), (first, due.clone()));
    let different = (2..40)
        .map(run)
        .find(|(_, other)| *other != due)
        .expect("a seed with another time");
    assert_ne!(different.0, first);
}

#[test]
fn a_match_of_computer_players_with_crates_and_growing_ore_is_deterministic() {
    // Integration: two computer players on the fixture map for 4000 ticks with every content
    // at its share: no credits below zero (I7), the cells in order (I6'), never more crates
    // than the maximum, and two runs with the same seed agree in every hash.
    let run = |seed: u64| {
        let data = data();
        let players = [computer(P0, ALPHA, 0, 1), computer(P1, GAMMA, 1, 1)];
        let mut app = match_on(
            &data,
            fixture_map(),
            &players,
            &MatchOptions::default(),
            seed,
        );
        let mut hashes = Vec::new();
        let (mut took, mut came) = (Vec::new(), 0);
        for index in 0..4_000_u32 {
            let events = tick(&mut app, Vec::new());
            took.extend(taken(&events));
            came += appeared(&events).len();
            for (player, state) in app.world().resource::<Players>().iter() {
                assert!(state.credits() >= 0, "player {} at tick {index}", player.0);
            }
            assert!(crates(&app).len() <= 3);
            if index % 16 == 0 {
                assert_cells(&mut app, &format!("tick {index}"));
            }
            if index % 100 == 0 {
                hashes.push(state_hash(app.world_mut()));
            }
            if app.world().resource::<MatchState>().outcome().is_some() {
                break;
            }
        }
        hashes.push(state_hash(app.world_mut()));
        (hashes, took, came)
    };
    let (hashes, took, came) = run(11);
    let again = run(11);
    assert_eq!(again.0, hashes);
    assert_eq!(again.1, took);
    assert!(came >= 1);
    let other = run(12);
    assert_ne!(other.0, hashes);
}
