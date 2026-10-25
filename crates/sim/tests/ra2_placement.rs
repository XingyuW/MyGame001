//! RA2: where a building may be placed (docs/RA2_PARITY_PLAN.md sections 4.2 and 5, RA2 item 7;
//! docs/DECISIONS.md R25).
//!
//! Driven through `CommandQueue` and `SimSchedule` on the RA2 fixture (`tests/fixtures/data/ra2`;
//! invariant I10). A building may be placed with at most its own `adjacent` free cells between
//! it and a base building of its owner: 2 for the buildings of the fixture, 4 for its defences,
//! 8 for its wall; defences and walls are no base buildings (`base_normal`).

mod ra2_common;

use std::path::PathBuf;

use bevy_app::App;
use ra2_common::*;
use sim::{
    CellPos, Command, PlacementError, ProductionItem, RejectReason, extends_base,
    load_match_definitions, placement_reach, state_hash,
};

/// Queues the building `id` for P0 and waits until it is ready.
fn ready(app: &mut App, id: &str) {
    let item = building(app, id);
    let time = defs(app).production(item).expect("terms").build_time;
    assert_eq!(rejections(&tick(app, vec![queue(P0, item)])), NONE, "{id}");
    ticks(app, time);
    assert_eq!(lane_of(app, P0, item)[0].progress, time, "{id} is ready");
}

/// `PlaceBuilding` of `id` at (`x`, `y`) for P0, and its rejections.
fn place_ready(app: &mut App, id: &str, x: i32, y: i32) -> Vec<RejectReason> {
    let command = issue(
        P0,
        Command::PlaceBuilding {
            building: kind(app, id),
            origin: CellPos::new(x, y),
        },
    );
    rejections(&tick(app, vec![command]))
}

fn too_far(distance: Option<u32>, max: u32) -> Vec<RejectReason> {
    vec![RejectReason::Placement(PlacementError::TooFar {
        distance,
        max,
    })]
}

/// P0 with its yard on (10, 10) to (12, 12) and plenty of credits.
fn base() -> App {
    let mut app = world(100_000);
    place(&mut app, P0, "allied_yard", 10, 10);
    // A rifleman who sees the cells beyond the yard's sight.
    spawn(&mut app, P0, "rifleman", 21, 13);
    tick(&mut app, Vec::new());
    app
}

// --- Positive and boundary ----------------------------------------------------------------------

#[test]
fn a_building_is_placed_within_its_own_adjacent_cells_of_a_base_building() {
    // (id, free cells allowed, the first column that is too far, the last that is allowed):
    // the yard ends in column 12, so a building in column 12 + n + 1 leaves n cells free.
    for (id, adjacent, far, last) in [
        ("allied_power", 2, 16, 15),
        ("bunker", 4, 18, 17),
        ("wall", 8, 22, 21),
    ] {
        let mut app = base();
        ready(&mut app, id);
        let reach = adjacent + 1;
        assert_eq!(placement_reach(&defs(&app), kind(&app, id)), reach, "{id}");
        // One cell beyond its reach: rejected, and nothing changes.
        let before = (credits(&app, P0), lane_of(&app, P0, building(&app, id)));
        assert_eq!(
            place_ready(&mut app, id, far, 10),
            too_far(Some(reach + 1), reach),
            "{id}"
        );
        assert_eq!(
            before,
            (credits(&app, P0), lane_of(&app, P0, building(&app, id)))
        );
        assert!(!owned_kinds(&mut app, P0).contains(&kind(&app, id)));
        // At its reach: placed.
        assert_eq!(place_ready(&mut app, id, last, 10), NONE, "{id}");
        assert!(owned_kinds(&mut app, P0).contains(&kind(&app, id)));
        assert!(lane_of(&app, P0, building(&app, id)).is_empty());
    }
}

#[test]
fn a_building_may_touch_a_base_building() {
    // No free cell between them: the smallest distance there is.
    let mut app = base();
    ready(&mut app, "allied_power");
    assert_eq!(place_ready(&mut app, "allied_power", 13, 10), NONE);
}

#[test]
fn a_rejected_placement_changes_no_state() {
    let run = |command: bool| {
        let mut app = base();
        ready(&mut app, "allied_power");
        if command {
            assert_eq!(
                place_ready(&mut app, "allied_power", 16, 10),
                too_far(Some(4), 3)
            );
        } else {
            tick(&mut app, Vec::new());
        }
        state_hash(app.world_mut())
    };
    assert_eq!(run(true), run(false));
}

// --- Negative -----------------------------------------------------------------------------------

#[test]
fn defences_and_walls_extend_no_base() {
    let mut app = base();
    // A bunker and a wall beside the yard, as scenario setup.
    place(&mut app, P0, "bunker", 16, 10);
    place(&mut app, P0, "wall", 16, 12);
    ready(&mut app, "allied_power");
    for id in ["bunker", "wall"] {
        assert!(!extends_base(&defs(&app), kind(&app, id)), "{id}");
    }
    assert!(extends_base(&defs(&app), kind(&app, "allied_yard")));
    // Two cells from the bunker, but six from the yard, the nearest base building.
    assert_eq!(
        place_ready(&mut app, "allied_power", 18, 10),
        too_far(Some(6), 3)
    );
    // A depot is a base building: beside it the power building is placed, far from the yard.
    place(&mut app, P0, "depot", 16, 14);
    tick(&mut app, Vec::new());
    assert_eq!(place_ready(&mut app, "allied_power", 20, 14), NONE);
}

#[test]
fn a_player_with_defences_only_has_no_place_to_build() {
    let mut app = world(100_000);
    let yard = place(&mut app, P0, "allied_yard", 10, 10);
    place(&mut app, P0, "bunker", 14, 10);
    tick(&mut app, Vec::new());
    ready(&mut app, "allied_power");
    let events = tick(
        &mut app,
        vec![issue(P0, Command::SellBuilding { building: yard })],
    );
    assert_eq!(rejections(&events), NONE);
    assert_eq!(
        place_ready(&mut app, "allied_power", 15, 10),
        too_far(None, 3)
    );
    assert_eq!(
        PlacementError::TooFar {
            distance: None,
            max: 3
        }
        .to_string(),
        "the player owns no building, and a building must lie within 3 cells of one"
    );
}

// --- Regression: a world without a rules file ------------------------------------------------------

#[test]
fn without_a_rules_file_every_building_extends_the_base_by_the_economy_files_distance() {
    // The legacy ruleset (the shipped data before RA1): an arsenal, no rules. Its traits of
    // schema version 3 are not read.
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data/legacy");
    let definitions = load_match_definitions(
        dir.join("units.ron"),
        dir.join("economy.ron"),
        dir.join("combat.ron"),
    )
    .and_then(|definitions| definitions.with_arsenal(dir.join("arsenal.ron")))
    .unwrap_or_else(|error| panic!("{error}"));
    assert!(definitions.rules().is_none());
    let distance = definitions.placement_distance();
    for (index, def) in definitions.buildings().iter().enumerate() {
        let kind = sim::BuildingKind(index as u16);
        assert!(extends_base(&definitions, kind), "{}", def.id);
        let shipyard = definitions
            .metadata(ProductionItem::Building(kind))
            .is_some_and(|entry| entry.role == sim::Role::Shipyard);
        assert_eq!(
            placement_reach(&definitions, kind),
            if shipyard {
                distance.max(sim::SHIPYARD_PLACEMENT_DISTANCE)
            } else {
                distance
            },
            "{}",
            def.id
        );
    }
}
