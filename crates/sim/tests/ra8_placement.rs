//! Shared planning geometry cannot bypass the actual paid production command.
mod ra7_common;
use ra7_common::*;
use sim::{Command, RejectReason, SimEvent, state_hash};

#[test]
fn ra8_place_without_a_completed_item_rejects_without_state_mutation() {
    let mut actual = world();
    let mut control = world();
    for app in [&mut actual, &mut control] {
        put(app, P0, "construction_yard", 10, 10);
        tick(app, vec![]);
    }
    let building = defs(&actual).building_kind("power_plant").unwrap();
    let events = tick(
        &mut actual,
        vec![issue(
            P0,
            Command::PlaceBuilding {
                building,
                origin: sim::CellPos::new(7, 10),
            },
        )],
    );
    tick(&mut control, vec![]);
    assert!(events.iter().any(|event| matches!(event,
        SimEvent::CommandRejected { reason: RejectReason::NotReady(kind), .. } if *kind == building)));
    assert_eq!(
        state_hash(actual.world_mut()),
        state_hash(control.world_mut())
    );
}
