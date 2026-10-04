//! M5 test: the placement preview of the interface (PLAN.md section 7, M5: "placement preview
//! with validity colouring"; docs/DECISIONS.md D35).
//!
//! The game colours the footprint at the cursor with `sim::check_placement`, which the
//! validation of `PlaceBuilding` calls. These tests check, through `SimPlugin` and
//! `CommandQueue` with fixture data, that for every origin of a region around a base the preview
//! predicts exactly the outcome of the command issued in the next tick: accepted, or rejected
//! with the same reason. The region covers valid footprints, footprints beyond the placement
//! distance, over the map's edge, over sand, over the player's own building and unit, over a
//! visible enemy unit, and over cells the player does not see (fog of war, D33).

use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    Building, BuildingKind, CellPos, Command, CommandQueue, Definitions, Footprint, IssuedCommand,
    PlayerId, Players, ProductionItem, RejectReason, SimEvent, SimPlugin, SimSchedule,
    check_placement, load_map, load_match_definitions, spawn_building, spawn_unit, state_hash,
};

const HUMAN: PlayerId = PlayerId(0);
const ENEMY: PlayerId = PlayerId(1);

fn fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(relative)
}

fn definitions() -> Definitions {
    load_match_definitions(
        fixture("data/units_combat.ron"),
        fixture("data/economy_combat.ron"),
        fixture("data/combat.ron"),
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

fn issue(app: &mut App, command: Command) {
    app.world_mut()
        .resource_mut::<CommandQueue>()
        .push(IssuedCommand {
            player: HUMAN,
            command,
        });
}

fn tick(app: &mut App) -> Vec<SimEvent> {
    app.world_mut().run_schedule(SimSchedule);
    app.world()
        .resource::<Messages<SimEvent>>()
        .iter_current_update_messages()
        .cloned()
        .collect()
}

/// The human player's yard at (10, 10) (sight 4), a crawler of its own at (17, 9) (sight 2, so
/// that the sand at x 18 is seen), and a rifleman of its own at (5, 5) (sight 5, so that cells
/// beyond the placement distance of 3 are seen); a visible enemy crawler at (14, 12) and an
/// unseen enemy beacon at (17, 15), on the economy fixture map (sand at x 18, y 0..=12). No
/// armed unit has an enemy in range. With `ready`, a power plant (2 × 2) is ready for placement.
fn scenario(ready: bool) -> App {
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 0 });
    let grid = load_map(fixture("maps/economy_32x32.tmx"))
        .unwrap_or_else(|error| panic!("{error}"))
        .grid;
    app.world_mut().insert_resource(grid);
    app.world_mut().insert_resource(definitions());
    for player in [HUMAN, ENEMY] {
        app.world_mut()
            .resource_mut::<Players>()
            .add(player, 1_000)
            .unwrap_or_else(|error| panic!("{error}"));
    }
    let world = app.world_mut();
    spawn_building(world, HUMAN, "yard", CellPos::new(10, 10)).expect("yard");
    spawn_unit(world, HUMAN, "crawler", CellPos::new(17, 9)).expect("own crawler");
    spawn_unit(world, HUMAN, "rifleman", CellPos::new(5, 5)).expect("rifleman");
    spawn_unit(world, ENEMY, "crawler", CellPos::new(14, 12)).expect("crawler");
    spawn_building(world, ENEMY, "beacon", CellPos::new(17, 15)).expect("beacon");
    if ready {
        issue(
            &mut app,
            Command::QueueProduction {
                item: ProductionItem::Building(power()),
            },
        );
    }
    // Long enough for the power plant (10 ticks) and for the vision of the first tick.
    for _ in 0..12 {
        let events = tick(&mut app);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, SimEvent::CommandRejected { .. })),
            "{events:?}"
        );
    }
    app
}

fn power() -> BuildingKind {
    definitions().building_kind("power").expect("power")
}

/// The preview of a power plant at `origin` for the human player, as the game computes it.
fn preview(app: &App, origin: CellPos) -> Result<Footprint, RejectReason> {
    let world = app.world();
    let state = world
        .resource::<Players>()
        .get(HUMAN)
        .expect("human player");
    let owned: Vec<Footprint> = world
        .iter_entities()
        .filter(|entity| {
            entity
                .get::<sim::Owner>()
                .is_some_and(|owner| owner.0 == HUMAN)
        })
        .filter_map(|entity| entity.get::<Building>().map(|building| building.footprint))
        .collect();
    check_placement(
        world.resource::<sim::Grid>(),
        world.resource::<Definitions>(),
        state,
        owned.into_iter(),
        power(),
        origin,
    )
}

/// The outcome of `PlaceBuilding` at `origin` in the next tick: `Ok` if the building was placed,
/// else the rejection.
fn place(app: &mut App, origin: CellPos) -> Result<(), RejectReason> {
    issue(
        app,
        Command::PlaceBuilding {
            building: power(),
            origin,
        },
    );
    let events = tick(app);
    if let Some(reason) = events.iter().find_map(|event| match event {
        SimEvent::CommandRejected { reason, .. } => Some(reason.clone()),
        _ => None,
    }) {
        return Err(reason);
    }
    assert!(
        events
            .iter()
            .any(|event| matches!(event, SimEvent::BuildingPlaced { player: HUMAN, .. })),
        "{events:?}"
    );
    Ok(())
}

#[test]
fn the_placement_preview_predicts_the_outcome_of_place_building_at_every_origin() {
    let mut origins: Vec<CellPos> = (4..=19)
        .flat_map(|y| (4..=19).map(move |x| CellPos::new(x, y)))
        .collect();
    // Over the map's edges and at the extremes of the coordinates a command can carry.
    origins.extend([
        CellPos::new(-1, 5),
        CellPos::new(5, -1),
        CellPos::new(31, 10),
        CellPos::new(10, 31),
        CellPos::new(i32::MIN, i32::MIN),
        CellPos::new(i32::MAX, i32::MAX),
    ]);
    let mut seen_reasons = std::collections::BTreeSet::new();
    let mut accepted = 0;
    for origin in origins {
        let mut app = scenario(true);
        let before = state_hash(app.world_mut());
        let predicted = preview(&app, origin);
        // Computing the preview changes nothing.
        assert_eq!(state_hash(app.world_mut()), before);
        let actual = place(&mut app, origin);
        match (&predicted, &actual) {
            (Ok(footprint), Ok(())) => {
                assert_eq!(footprint.origin, origin);
                accepted += 1;
            }
            (Err(expected), Err(found)) => {
                assert_eq!(expected, found, "{origin:?}");
                seen_reasons.insert(
                    format!("{:?}", std::mem::discriminant(expected))
                        + &match expected {
                            RejectReason::Placement(error) => {
                                format!("{:?}", std::mem::discriminant(error))
                            }
                            _ => String::new(),
                        },
                );
            }
            _ => panic!("{origin:?}: preview {predicted:?}, command {actual:?}"),
        }
    }
    // The region holds valid origins and every kind of placement rejection: outside the map,
    // not visible, not buildable (sand), overlap (the own yard), occupied (the own walker and
    // the visible crawler), and too far.
    assert!(accepted > 0);
    assert_eq!(seen_reasons.len(), 6, "{seen_reasons:?}");
}

#[test]
fn the_preview_of_a_building_that_is_not_ready_is_not_ready() {
    let mut app = scenario(false);
    let origin = CellPos::new(13, 11);
    assert_eq!(preview(&app, origin), Err(RejectReason::NotReady(power())));
    assert_eq!(
        place(&mut app, origin),
        Err(RejectReason::NotReady(power()))
    );
}
