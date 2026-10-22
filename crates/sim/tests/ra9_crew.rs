//! Historical preparation note: UNVALIDATED / PENDING. Crew evidence draft on shipped rules and commands.
//! Historical preparation note: Proposed integration path crates/sim/tests/ra9_crew.rs.
mod ra9_soundness_common;
use ra9_soundness_common::*;
use sim::{Cell, CellPos, Command, Grid, Owner, SimEvent, SimId, Unit};

fn escaped(events: &[SimEvent], source: SimId) -> Vec<SimId> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::CrewEscaped { from, units, .. } if *from == source => Some(units),
            _ => None,
        })
        .flatten()
        .copied()
        .collect()
}

#[test]
fn ra9_sold_shipped_yard_uses_all_three_last_surface_spots_without_overlap() {
    let mut app = world();
    let yard = put(&mut app, P1, "soviet_construction_yard", 10, 10);
    steps(&mut app, 1);
    // Setup leaves only one passable foundation cell after the real sale. Shipped Conscripts
    // hold three subcell spots, so the six nominal survivors cannot all escape.
    for y in 10..13 {
        for x in 10..13 {
            if (x, y) != (12, 12) {
                app.world_mut().resource_mut::<Grid>().set_terrain(
                    CellPos::new(x, y),
                    Cell {
                        passable: false,
                        water: true,
                        buildable: false,
                        ore: 0,
                    },
                );
            }
        }
    }
    let events = tick(
        &mut app,
        vec![issue(P1, Command::SellBuilding { building: yard })],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    let crew = escaped(&events, yard);
    assert_eq!(
        crew.len(),
        3,
        "the last free cell has exactly three infantry spots"
    );
    let mut points = std::collections::BTreeSet::new();
    for id in crew {
        assert_eq!(cell(&mut app, id), CellPos::new(12, 12));
        let e = entity(&mut app, id).unwrap();
        let position = app.world().get::<sim::Position>(e).unwrap().0;
        points.insert((position.x, position.y));
        assert_eq!(app.world().get::<Owner>(e).unwrap().0, P1);
    }
    assert_eq!(points.len(), 3);
    cells(&mut app);
}

#[test]
fn ra9_genuine_engineer_capture_then_sale_uses_the_capturers_side_for_crew() {
    let mut app = world();
    let yard = put(&mut app, P1, "soviet_construction_yard", 10, 10);
    let engineer = put(&mut app, P0, "engineer", 9, 10);
    steps(&mut app, 2);
    let events = interact(&mut app, engineer, yard);
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(
        entity(&mut app, engineer).is_none(),
        "Engineer must actually enter and be consumed"
    );
    assert_eq!(owner(&mut app, yard), Some(P0));
    let events = tick(
        &mut app,
        vec![issue(P0, Command::SellBuilding { building: yard })],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    let crew = escaped(&events, yard);
    assert!(!crew.is_empty());
    let gi = defs(&app).unit_kind("gi").unwrap();
    let conscript = defs(&app).unit_kind("conscript").unwrap();
    let mut soldiers = 0;
    for id in crew {
        let e = entity(&mut app, id).unwrap();
        let kind = app.world().get::<Unit>(e).unwrap().kind;
        assert_ne!(
            kind, conscript,
            "captured Soviet foundation cannot choose Soviet crew"
        );
        if kind == gi {
            soldiers += 1;
        }
        assert_eq!(app.world().get::<Owner>(e).unwrap().0, P0);
    }
    assert_eq!(soldiers, 3, "1500 source-rule refund / Allied divisor500");
    cells(&mut app);
}
