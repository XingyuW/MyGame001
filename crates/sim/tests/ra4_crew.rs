//! RA4: crews and survivors (docs/RA2_PARITY_PLAN.md section 4.7; docs/DECISIONS.md R48), on
//! the RA4 fixture (`tests/fixtures/data/ra4`; invariant I10), through `CommandQueue` and
//! `SimSchedule`.
//!
//! In the fixture the two base units, the yards, the barracks, the factories and the airfield
//! are crewed. P0 is Allied (crew: the rifleman, divisor 500) and P1 Soviet (the conscript,
//! divisor 250); the rules refund 40 percent and let a crew escape with 50 percent.

mod ra4_common;

use bevy_app::App;
use ra4_common::*;
use sim::{
    Cell, CellPos, Command, Definitions, ENGINEER_ESCAPE_PERCENT, Grid, PlayerId, Players,
    SimEvent, SimId, SimRng,
};

/// The crews that left in `events`: (owner, what they left, where, the new units).
fn escaped(events: &[SimEvent]) -> Vec<(PlayerId, SimId, CellPos, Vec<SimId>)> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::CrewEscaped {
                player,
                from,
                at,
                units,
            } => Some((*player, *from, *at, units.clone())),
            _ => None,
        })
        .collect()
}

/// The type id of the unit `id`.
fn kind_of(app: &mut App, id: SimId) -> String {
    let definitions = defs(app);
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &sim::Unit)>();
    let kind = query
        .iter(world)
        .find(|(other, _)| **other == id)
        .map(|(_, unit)| unit.kind)
        .unwrap_or_else(|| panic!("{id:?} is no unit"));
    definitions.unit(kind).expect("a type").id.clone()
}

/// Destroys `id` (its hit points are set to 0, as a blow would leave them) and runs the tick
/// in which it is removed; the events of that tick.
fn destroy(app: &mut App, id: SimId) -> Vec<SimEvent> {
    set_hp(app, id, 0);
    tick(app, Vec::new())
}

/// The next number the world's generator would draw, without drawing it.
fn next_draw(app: &App) -> u32 {
    app.world().resource::<SimRng>().clone().next_u32()
}

// --- Vehicles ----------------------------------------------------------------------------------

/// A base unit of P0 standing at (10, 10), destroyed in a world with `seed`: the crew.
fn wrecked_base_unit(definitions: &Definitions, seed: u64) -> (App, Vec<SimId>) {
    let mut app = world_of(definitions.clone(), seed);
    let mcv = spawn(&mut app, P0, "allied_mcv", 10, 10);
    tick(&mut app, Vec::new());
    let events = destroy(&mut app, mcv);
    let crews = escaped(&events);
    assert!(crews.len() <= 1);
    let units = crews.into_iter().flat_map(|(.., units)| units).collect();
    (app, units)
}

#[test]
fn the_crew_of_a_destroyed_vehicle_escapes_at_the_documented_rate() {
    let definitions = definitions();
    let mut escapes = 0;
    for seed in 0..400 {
        let (mut app, units) = wrecked_base_unit(&definitions, seed);
        assert!(units.len() <= 1, "one crew member at most");
        if let Some(&unit) = units.first() {
            escapes += 1;
            // The crew is of the owner's side, stands where the vehicle stood, and is whole.
            assert_eq!(kind_of(&mut app, unit), "rifleman");
            assert_eq!(mobile(&mut app, unit).cell, CellPos::new(10, 10));
            assert_eq!(hp(&mut app, unit), Some(100));
            assert_cells(&mut app, "after the escape");
        }
    }
    // 50 percent of 400: within four standard deviations (10 each) of 200.
    assert!((160..=240).contains(&escapes), "{escapes} of 400");
    // The same seeds give the same crews.
    let again = (0..400)
        .filter(|&seed| !wrecked_base_unit(&definitions, seed).1.is_empty())
        .count();
    assert_eq!(again, escapes);
}

#[test]
fn the_crew_is_of_the_side_of_the_owners_country() {
    let definitions = definitions();
    let mut seen = 0;
    for seed in 0..40 {
        let mut app = world_of(definitions.clone(), seed);
        // P1 is Soviet: also from an Allied base unit its crew is a conscript.
        let mcv = spawn(&mut app, P1, "allied_mcv", 10, 10);
        tick(&mut app, Vec::new());
        for (player, from, at, units) in escaped(&destroy(&mut app, mcv)) {
            assert_eq!((player, from, at), (P1, mcv, CellPos::new(10, 10)));
            assert_eq!(kind_of(&mut app, units[0]), "conscript");
            seen += 1;
        }
    }
    assert!(seen > 5, "{seen}");
}

#[test]
fn nobody_leaves_a_vehicle_that_was_moving_or_is_not_crewed() {
    let definitions = definitions();
    for seed in 0..60 {
        // In a step when it is destroyed.
        let mut app = world_of(definitions.clone(), seed);
        let mcv = spawn(&mut app, P0, "allied_mcv", 10, 10);
        tick(&mut app, Vec::new());
        tick(&mut app, vec![move_to(P0, &[mcv], 20, 10)]);
        ticks(&mut app, 4);
        assert!(mobile(&mut app, mcv).step.is_some(), "in a step");
        let draw = next_draw(&app);
        assert!(escaped(&destroy(&mut app, mcv)).is_empty(), "seed {seed}");
        assert_eq!(next_draw(&app), draw, "no draw for a vehicle in a step");
        // A tank has no crew, and draws nothing.
        let mut app = world_of(definitions.clone(), seed);
        let tank = spawn(&mut app, P0, "tank", 10, 10);
        tick(&mut app, Vec::new());
        let draw = next_draw(&app);
        assert!(escaped(&destroy(&mut app, tank)).is_empty());
        assert_eq!(next_draw(&app), draw, "no draw for an uncrewed vehicle");
    }
}

#[test]
fn a_crew_member_takes_the_nearest_cell_with_room_or_does_not_escape() {
    let definitions = definitions();
    let mut beside = 0;
    for seed in 0..40 {
        // The wreck's cell is taken at once by... nothing can stand there in the tick, so the
        // cell itself is closed: the base unit stands on a bridge of one cell in water.
        let mut app = world_of(definitions.clone(), seed);
        let mcv = spawn(&mut app, P0, "allied_mcv", 10, 10);
        tick(&mut app, Vec::new());
        set_hp(&mut app, mcv, 0);
        // The ground under it gives way as it is destroyed.
        app.world_mut().resource_mut::<Grid>().set_terrain(
            CellPos::new(10, 10),
            Cell {
                passable: false,
                water: true,
                buildable: false,
                ore: 0,
            },
        );
        let events = tick(&mut app, Vec::new());
        for (.., units) in escaped(&events) {
            let cell = mobile(&mut app, units[0]).cell;
            assert_ne!(cell, CellPos::new(10, 10));
            assert!(
                cell.x.abs_diff(10) <= 1 && cell.y.abs_diff(10) <= 1,
                "{cell:?}"
            );
            beside += 1;
        }
        assert_cells(&mut app, "beside the wreck");
    }
    assert!(beside > 5, "{beside}");
    // With no room at all, nobody escapes, whatever is drawn.
    for seed in 0..40 {
        let mut app = world_of(definitions.clone(), seed);
        flood(&mut app, (8, 8), (12, 12));
        app.world_mut().resource_mut::<Grid>().set_terrain(
            CellPos::new(10, 10),
            Cell {
                passable: true,
                water: false,
                buildable: true,
                ore: 0,
            },
        );
        let mcv = spawn(&mut app, P0, "allied_mcv", 10, 10);
        tick(&mut app, Vec::new());
        set_hp(&mut app, mcv, 0);
        flood(&mut app, (10, 10), (10, 10));
        assert!(escaped(&tick(&mut app, Vec::new())).is_empty());
    }
}

// --- Buildings -------------------------------------------------------------------------------

#[test]
fn the_survivors_of_a_destroyed_building_are_its_refund_over_the_divisor() {
    // (owner, building, crew type, survivors): the refund is 40 percent of the cost.
    for (owner, building, crew, count) in [
        // 2000 x 0.4 = 800: 800 / 500 = 1.6, and 800 / 250 = 3.2, rounded up.
        (P0, "allied_yard", "rifleman", 2),
        (P1, "soviet_yard", "conscript", 4),
        // 500 x 0.4 = 200: at least one.
        (P0, "allied_factory", "rifleman", 1),
        (P1, "soviet_factory", "conscript", 1),
        // 200 x 0.4 = 80.
        (P1, "soviet_barracks", "conscript", 1),
        // An Allied building of a Soviet owner: the owner's crew and divisor.
        (P1, "allied_yard", "conscript", 4),
    ] {
        let mut app = world();
        let id = place(&mut app, owner, building, 10, 10);
        tick(&mut app, Vec::new());
        let events = destroy(&mut app, id);
        let crews = escaped(&events);
        assert_eq!(crews.len(), 1, "{building}");
        let (player, from, at, units) = crews[0].clone();
        assert_eq!((player, from, at), (owner, id, CellPos::new(10, 10)));
        let soldiers: Vec<SimId> = units
            .iter()
            .copied()
            .filter(|&unit| kind_of(&mut app, unit) == crew)
            .collect();
        assert_eq!(soldiers.len(), count, "{building} of {owner:?}");
        // What is not a soldier is the engineer of a construction yard.
        assert!(units.len() - soldiers.len() <= usize::from(building.ends_with("yard")));
        // They stand on the cells the building covered, one to a cell.
        let footprint = |cell: CellPos| (10..13).contains(&cell.x) && (10..13).contains(&cell.y);
        for unit in units {
            assert!(footprint(mobile(&mut app, unit).cell));
        }
        assert_cells(&mut app, building);
    }
}

#[test]
fn a_sold_building_releases_its_survivors_too() {
    let mut app = world();
    let yard = place(&mut app, P1, "soviet_yard", 10, 10);
    tick(&mut app, Vec::new());
    let events = tick(
        &mut app,
        vec![issue(P1, Command::SellBuilding { building: yard })],
    );
    assert!(events.iter().any(|event| matches!(
        event,
        SimEvent::BuildingSold {
            building,
            refund: 800,
            ..
        } if *building == yard
    )));
    let crews = escaped(&events);
    assert_eq!(crews.len(), 1);
    let conscripts = crews[0]
        .3
        .iter()
        .filter(|&&unit| kind_of(&mut app, unit) == "conscript")
        .count();
    assert_eq!(conscripts, 4);
    assert_cells(&mut app, "after the sale");
    // The survivors are units like any other: they obey a command.
    let unit = crews[0].3[0];
    let events = tick(&mut app, vec![move_to(P1, &[unit], 20, 20)]);
    assert!(rejections(&events).is_empty());
}

#[test]
fn an_uncrewed_building_and_an_owner_without_a_country_release_nobody() {
    let mut app = world();
    let turret = place(&mut app, P0, "turret", 10, 10);
    let power = place(&mut app, P0, "allied_power", 14, 10);
    tick(&mut app, Vec::new());
    let draw = next_draw(&app);
    assert!(escaped(&destroy(&mut app, turret)).is_empty());
    let events = tick(
        &mut app,
        vec![issue(P0, Command::SellBuilding { building: power })],
    );
    assert!(escaped(&events).is_empty());
    assert_eq!(next_draw(&app), draw, "nothing is drawn for them");
    // A player without a country has no side, and so no crew.
    let stateless = PlayerId(2);
    app.world_mut()
        .resource_mut::<Players>()
        .add(stateless, 0)
        .expect("a third player");
    let factory = place(&mut app, stateless, "allied_factory", 20, 20);
    tick(&mut app, Vec::new());
    assert!(escaped(&destroy(&mut app, factory)).is_empty());
}

#[test]
fn survivors_are_as_many_as_the_footprint_has_room_for() {
    let definitions = definitions();
    for seed in 0..20 {
        let mut app = world_of(definitions.clone(), seed);
        let yard = place(&mut app, P1, "soviet_yard", 10, 10);
        tick(&mut app, Vec::new());
        set_hp(&mut app, yard, 0);
        // All but three cells under the yard are no ground to stand on when it falls: of the
        // four survivors and perhaps an engineer, three find room.
        flood(&mut app, (10, 10), (12, 11));
        let events = tick(&mut app, Vec::new());
        let crews = escaped(&events);
        assert_eq!(crews.len(), 1);
        assert_eq!(crews[0].3.len(), 3, "seed {seed}");
        for &unit in &crews[0].3 {
            assert_eq!(mobile(&mut app, unit).cell.y, 12);
        }
        assert_cells(&mut app, "the last free cells");
    }
    // With no cell at all nobody is released, and no event is written.
    let mut app = world();
    let yard = place(&mut app, P1, "soviet_yard", 10, 10);
    tick(&mut app, Vec::new());
    set_hp(&mut app, yard, 0);
    flood(&mut app, (10, 10), (12, 12));
    assert!(escaped(&tick(&mut app, Vec::new())).is_empty());
}

#[test]
fn an_engineer_leaves_a_construction_yard_at_its_rate_and_no_other_building() {
    let definitions = definitions();
    let engineers = |building: &str| {
        (0..400)
            .filter(|&seed| {
                let mut app = world_of(definitions.clone(), seed);
                let id = place(&mut app, P0, building, 10, 10);
                tick(&mut app, Vec::new());
                let events = destroy(&mut app, id);
                escaped(&events)
                    .into_iter()
                    .flat_map(|(.., units)| units)
                    .any(|unit| kind_of(&mut app, unit) == "engineer")
            })
            .count()
    };
    let from_yards = engineers("allied_yard");
    // 25 percent of 400: within four standard deviations (8.7 each) of 100.
    assert_eq!(ENGINEER_ESCAPE_PERCENT, 25);
    assert!((65..=135).contains(&from_yards), "{from_yards} of 400");
    assert_eq!(engineers("allied_factory"), 0);
}

// --- Integration -------------------------------------------------------------------------------

#[test]
fn a_base_unit_destroyed_in_battle_releases_its_crew_and_the_world_runs_the_same_twice() {
    let run = |seed: u64| {
        let mut app = world_of(definitions(), seed);
        let mcv = spawn(&mut app, P0, "allied_mcv", 10, 10);
        let yard = place(&mut app, P0, "allied_yard", 8, 14);
        let tanks: Vec<SimId> = (0..3)
            .map(|n| spawn(&mut app, P1, "tank", 14, 9 + n))
            .collect();
        tick(&mut app, Vec::new());
        let mut events = tick(
            &mut app,
            tanks.iter().map(|&tank| attack(P1, tank, mcv)).collect(),
        );
        let mut hashes = Vec::new();
        for n in 0..600 {
            if n == 300 {
                events.extend(tick(
                    &mut app,
                    tanks.iter().map(|&tank| attack(P1, tank, yard)).collect(),
                ));
            } else {
                events.extend(tick(&mut app, Vec::new()));
            }
            hashes.push(hash(&mut app));
            assert_cells(&mut app, "the battle");
        }
        assert!(!alive(&mut app, mcv) && !alive(&mut app, yard));
        (hashes, escaped(&events))
    };
    let (first, crews) = run(5);
    let (second, again) = run(5);
    assert_eq!(first, second);
    assert_eq!(crews, again);
    // The yard always releases its two riflemen.
    assert!(
        crews.iter().any(|(.., units)| units.len() >= 2),
        "{crews:?}"
    );
}
