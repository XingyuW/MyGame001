//! RA4: aircraft ammunition and docks (docs/RA2_PARITY_PLAN.md sections 4.6 and 4.8;
//! docs/DECISIONS.md R49), on the RA4 fixture (`tests/fixtures/data/ra4`; invariant I10),
//! through `CommandQueue` and `SimSchedule`.
//!
//! In the fixture the striker (300 credits, speed 48) has two volleys of its pod (a burst of 2
//! shots of 20, 4 cells) and needs a dock; the airfield (2 x 2 cells) is its factory and has
//! two docks; a round is reloaded in 40 ticks. The gunship needs no dock and has no ammunition.

mod ra4_common;

use bevy_app::App;
use ra4_common::*;
use sim::{
    CellPos, Command, PlayerId, Players, ProductionItem, RejectReason, SimEvent, SimId,
    SpecialState, dock_cell,
};

fn striker_item(app: &App) -> ProductionItem {
    ProductionItem::Unit(defs(app).unit_kind("striker").expect("striker"))
}

fn queue(app: &App, player: PlayerId) -> sim::IssuedCommand {
    issue(
        player,
        Command::QueueProduction {
            item: striker_item(app),
        },
    )
}

fn state(app: &App) -> &SpecialState {
    app.world().resource::<SpecialState>()
}

/// The volleys the aircraft `id` has left: `None` while it has all of them.
fn ammo(app: &App, id: SimId) -> Option<u32> {
    state(app).ammo.get(&id).copied()
}

fn destroyed(events: &[SimEvent]) -> Vec<SimId> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::UnitDestroyed { unit, .. } => Some(*unit),
            _ => None,
        })
        .collect()
}

/// Whether the aircraft `id` stands on its dock.
fn landed(app: &mut App, id: SimId) -> bool {
    let Some(&(building, dock)) = state(app).docks.get(&id) else {
        return false;
    };
    let footprint = {
        let world = app.world_mut();
        let mut query = world.query::<(&SimId, &sim::Building)>();
        query
            .iter(world)
            .find(|(other, _)| **other == building)
            .map(|(_, building)| building.footprint)
            .expect("the building of the dock")
    };
    let mobile = mobile(app, id);
    mobile.cell == dock_cell(footprint, dock) && mobile.step.is_none()
}

/// A world with an airfield of P0 at (10, 10) and `count` strikers that have landed on it.
fn airfield_with(count: usize) -> (App, SimId, Vec<SimId>) {
    let mut app = world();
    let airfield = place(&mut app, P0, "airfield", 10, 10);
    let strikers: Vec<SimId> = (0..count)
        .map(|n| spawn(&mut app, P0, "striker", 10 + n as i32, 13))
        .collect();
    run_until(&mut app, 100, |app, _| {
        strikers.iter().all(|&id| landed(app, id))
    });
    (app, airfield, strikers)
}

// --- Production needs a free dock -------------------------------------------------------------

#[test]
fn an_aircraft_is_queued_only_while_a_dock_is_free() {
    let mut app = world();
    place(&mut app, P0, "airfield", 10, 10);
    tick(&mut app, Vec::new());
    let item = striker_item(&app);
    // Two docks: two strikers may be queued, the third is refused without any change.
    let first = queue(&app, P0);
    let second = queue(&app, P0);
    let third = queue(&app, P0);
    let before = app
        .world()
        .resource::<Players>()
        .get(P0)
        .expect("P0")
        .credits();
    let events = tick(&mut app, vec![first, second, third]);
    assert_eq!(rejections(&events), [RejectReason::NoFreeDock(item)]);
    let lane = |app: &App| {
        let definitions = defs(app);
        let key = definitions.queue_of(item).expect("a queue");
        app.world()
            .resource::<Players>()
            .get(P0)
            .expect("P0")
            .lane(key)
            .len()
    };
    assert_eq!(lane(&app), 2);
    assert!(
        before
            >= app
                .world()
                .resource::<Players>()
                .get(P0)
                .expect("P0")
                .credits()
    );
    // Both are built and land on the two docks.
    run_until(&mut app, 400, |app, _| {
        let strikers = units_of(app, P0, "striker");
        strikers.len() == 2 && strikers.iter().all(|&id| landed(app, id))
    });
    let strikers = units_of(&mut app, P0, "striker");
    assert_eq!(
        app.world()
            .resource::<Players>()
            .get(P0)
            .expect("P0")
            .docked_aircraft(),
        2
    );
    let docks: Vec<(SimId, u8)> = strikers.iter().map(|id| state(&app).docks[id]).collect();
    assert_ne!(docks[0], docks[1], "each has a dock of its own");
    assert_eq!(
        strikers
            .iter()
            .map(|&id| mobile(&mut app, id).cell)
            .collect::<Vec<_>>(),
        [CellPos::new(10, 10), CellPos::new(11, 10)],
        "above the first two cells of the airfield"
    );
    assert_cells(&mut app, "on the docks");
    // Owned aircraft count like queued ones: a third is still refused.
    let again = queue(&app, P0);
    assert_eq!(
        rejections(&tick(&mut app, vec![again])),
        [RejectReason::NoFreeDock(item)]
    );
    // With a second airfield it is built.
    place(&mut app, P0, "airfield", 16, 10);
    let again = queue(&app, P0);
    assert!(rejections(&tick(&mut app, vec![again])).is_empty());
    run_until(&mut app, 400, |app, _| {
        units_of(app, P0, "striker").len() == 3
    });
}

#[test]
fn one_owned_and_one_queued_aircraft_take_both_docks() {
    let (mut app, _, _) = airfield_with(1);
    let item = striker_item(&app);
    let (first, second) = (queue(&app, P0), queue(&app, P0));
    let events = tick(&mut app, vec![first, second]);
    assert_eq!(rejections(&events), [RejectReason::NoFreeDock(item)]);
}

#[test]
fn without_a_building_with_docks_no_such_aircraft_is_queued() {
    let mut app = world();
    // The factory of vehicles is no airfield: no producer, and no dock either.
    place(&mut app, P0, "allied_factory", 10, 10);
    tick(&mut app, Vec::new());
    let command = queue(&app, P0);
    let events = tick(&mut app, vec![command]);
    assert!(
        matches!(rejections(&events)[..], [RejectReason::NoProducer { .. }]),
        "{events:?}"
    );
    // The gunship needs neither: it is queued at the factory.
    let gunship = ProductionItem::Unit(defs(&app).unit_kind("gunship").expect("gunship"));
    let events = tick(
        &mut app,
        vec![issue(P0, Command::QueueProduction { item: gunship })],
    );
    assert!(rejections(&events).is_empty());
}

#[test]
fn an_aircraft_that_is_complete_waits_for_a_dock_that_was_lost_meanwhile() {
    let mut app = world();
    place(&mut app, P0, "airfield", 10, 10);
    let second = place(&mut app, P0, "airfield", 16, 10);
    tick(&mut app, Vec::new());
    let commands = (0..4).map(|_| queue(&app, P0)).collect();
    assert!(rejections(&tick(&mut app, commands)).is_empty());
    run_until(&mut app, 400, |app, _| {
        units_of(app, P0, "striker").len() == 2
    });
    // Two docks go with the second airfield: the third striker is complete and stays inside.
    set_hp(&mut app, second, 0);
    ticks(&mut app, 200);
    assert_eq!(units_of(&mut app, P0, "striker").len(), 2);
    // It leaves when there is a dock again (the survivor of the airfield stands where it
    // was, so the new one is built beside).
    place(&mut app, P0, "airfield", 22, 10);
    run_until(&mut app, 400, |app, _| {
        units_of(app, P0, "striker").len() == 4
    });
}

// --- Ammunition, the way home and reloading --------------------------------------------------

#[test]
fn an_aircraft_fires_its_volleys_returns_and_reloads_a_round_at_a_time() {
    let (mut app, _, strikers) = airfield_with(1);
    let striker = strikers[0];
    // An unarmed spotter keeps the target in sight.
    spawn(&mut app, P0, "engineer", 15, 10);
    let truck = spawn(&mut app, P1, "truck", 18, 10);
    tick(&mut app, Vec::new());
    assert_eq!(ammo(&app, striker), None, "full");
    let mut events = tick(&mut app, vec![attack(P0, striker, truck)]);
    // The first volley: two shots, and one volley less. The round is spent when the volley
    // begins, with its first shot (docs/DECISIONS.md R56; before, with its last).
    let (more, _) = run_until(&mut app, 200, |app, _| ammo(app, striker) == Some(1));
    events.extend(more);
    assert_eq!(shots(&events, striker).len(), 1);
    let volley_over = |app: &mut App, left: u32| {
        ammo(app, striker) == Some(left) && weapon(app, striker).burst_left == 0
    };
    let (more, _) = run_until(&mut app, 200, |app, _| volley_over(app, 1));
    events.extend(more);
    assert_eq!(shots(&events, striker).len(), 2);
    // The second: it has none left, its attack is over and it flies home.
    let (more, _) = run_until(&mut app, 200, |app, _| volley_over(app, 0));
    events.extend(more);
    assert_eq!(shots(&events, striker).len(), 4);
    assert_eq!(hp(&mut app, truck), Some(300 - 4 * 20));
    let (more, _) = run_until(&mut app, 200, |app, _| landed(app, striker));
    events.extend(more);
    assert_eq!(
        shots(&events, striker).len(),
        4,
        "no shot without ammunition"
    );
    assert_eq!(weapon(&mut app, striker).order, None);
    // On its dock: a round after 40 ticks, the second after 40 more.
    ticks(&mut app, 39);
    assert_eq!(ammo(&app, striker), Some(0), "after 39 ticks on the dock");
    tick(&mut app, Vec::new());
    assert_eq!(ammo(&app, striker), Some(1), "after 40");
    ticks(&mut app, 39);
    assert_eq!(ammo(&app, striker), Some(1));
    tick(&mut app, Vec::new());
    assert_eq!(ammo(&app, striker), None, "full again");
    assert!(state(&app).reloading.is_empty());
}

#[test]
fn an_aircraft_without_ammunition_does_not_fire_and_its_order_ends() {
    let (mut app, _, strikers) = airfield_with(1);
    let striker = strikers[0];
    app.world_mut()
        .resource_mut::<SpecialState>()
        .ammo
        .insert(striker, 0);
    let truck = spawn(&mut app, P1, "truck", 13, 10);
    tick(&mut app, Vec::new());
    let mut events = tick(&mut app, vec![attack(P0, striker, truck)]);
    events.extend(ticks(&mut app, 30));
    assert!(shots(&events, striker).is_empty());
    assert_eq!(weapon(&mut app, striker).order, None);
    assert_eq!(hp(&mut app, truck), Some(300));
    // With one round it fires one volley, also on its own from its dock (the 20 ticks are
    // too short for the dock to reload another).
    {
        let mut state = app.world_mut().resource_mut::<SpecialState>();
        state.ammo.insert(striker, 1);
        state.reloading.clear();
    }
    let events = ticks(&mut app, 20);
    assert_eq!(shots(&events, striker).len(), 2);
    assert_eq!(ammo(&app, striker), Some(0));
}

#[test]
fn only_an_aircraft_on_its_dock_reloads_and_leaving_starts_the_round_anew() {
    let (mut app, _, strikers) = airfield_with(1);
    let striker = strikers[0];
    app.world_mut()
        .resource_mut::<SpecialState>()
        .ammo
        .insert(striker, 0);
    ticks(&mut app, 30);
    assert_eq!(state(&app).reloading.get(&striker), Some(&30));
    // Ordered away after 30 of the 40 ticks: the round is not kept.
    tick(&mut app, vec![move_to(P0, &[striker], 20, 20)]);
    assert!(state(&app).reloading.is_empty());
    let (_, away) = run_until(&mut app, 200, |app, _| {
        mobile(app, striker).cell == CellPos::new(20, 20)
    });
    assert!(away > 5);
    assert_eq!(ammo(&app, striker), Some(0), "nothing reloads in the air");
    // With nothing to do it flies home by itself and starts the round from the beginning.
    run_until(&mut app, 200, |app, _| landed(app, striker));
    assert_eq!(ammo(&app, striker), Some(0));
    ticks(&mut app, 39);
    assert_eq!(ammo(&app, striker), Some(0));
    ticks(&mut app, 2);
    assert_eq!(ammo(&app, striker), Some(1));
}

// --- The loss of the dock ---------------------------------------------------------------------

#[test]
fn an_aircraft_whose_dock_is_lost_takes_a_free_one() {
    let (mut app, first, strikers) = airfield_with(2);
    let second = place(&mut app, P0, "airfield", 20, 20);
    tick(&mut app, Vec::new());
    assert!(strikers.iter().all(|id| state(&app).docks[id].0 == first));
    let events = tick(
        &mut app,
        vec![issue(P0, Command::SellBuilding { building: first })],
    );
    assert!(destroyed(&events).is_empty());
    let (events, _) = run_until(&mut app, 200, |app, _| {
        strikers.iter().all(|&id| landed(app, id))
    });
    assert!(destroyed(&events).is_empty());
    assert!(strikers.iter().all(|id| state(&app).docks[id].0 == second));
    assert_eq!(
        strikers
            .iter()
            .map(|&id| mobile(&mut app, id).cell)
            .collect::<Vec<_>>(),
        [CellPos::new(20, 20), CellPos::new(21, 20)]
    );
    assert_cells(&mut app, "on the other airfield");
}

#[test]
fn an_aircraft_with_no_dock_left_is_lost() {
    // Three strikers on two airfields; one airfield is destroyed: two keep or find a dock,
    // the third has none and is lost.
    let mut app = world();
    let first = place(&mut app, P0, "airfield", 10, 10);
    place(&mut app, P0, "airfield", 20, 20);
    let strikers: Vec<SimId> = (0..3)
        .map(|n| spawn(&mut app, P0, "striker", 10 + n, 13))
        .collect();
    run_until(&mut app, 200, |app, _| {
        strikers.iter().all(|&id| landed(app, id))
    });
    let on_first: Vec<SimId> = strikers
        .iter()
        .copied()
        .filter(|id| state(&app).docks[id].0 == first)
        .collect();
    assert_eq!(on_first.len(), 2);
    set_hp(&mut app, first, 0);
    let events = ticks(&mut app, 5);
    // The one with the lower id takes the last free dock; the other is destroyed.
    assert_eq!(destroyed(&events), [on_first[1]]);
    assert!(alive(&mut app, on_first[0]));
    run_until(&mut app, 200, |app, _| landed(app, on_first[0]));
    assert_eq!(state(&app).docks.len(), 2);
    assert_eq!(
        app.world()
            .resource::<Players>()
            .get(P0)
            .expect("P0")
            .docked_aircraft(),
        2
    );
}

#[test]
fn an_aircraft_without_any_dock_flies_on_while_it_has_an_order_and_is_lost_after_it() {
    let (mut app, airfield, strikers) = airfield_with(1);
    let striker = strikers[0];
    tick(&mut app, vec![move_to(P0, &[striker], 25, 25)]);
    ticks(&mut app, 5);
    set_hp(&mut app, airfield, 0);
    let (events, _) = run_until(&mut app, 200, |app, _| {
        mobile(app, striker).cell == CellPos::new(25, 25) && mobile(app, striker).step.is_none()
    });
    assert!(destroyed(&events).is_empty(), "it flies on under its order");
    let events = ticks(&mut app, 3);
    assert_eq!(destroyed(&events), [striker], "and is lost when it ends");
    assert!(state(&app).docks.is_empty() && state(&app).ammo.is_empty());
}

#[test]
fn the_dock_of_another_player_is_no_dock() {
    let mut app = world();
    place(&mut app, P1, "airfield", 10, 10);
    let striker = spawn(&mut app, P0, "striker", 10, 13);
    let events = ticks(&mut app, 3);
    assert_eq!(destroyed(&events), [striker]);
}

// --- Aircraft that need no dock -----------------------------------------------------------------

#[test]
fn an_aircraft_without_the_traits_needs_no_dock_and_never_runs_out() {
    let mut app = world();
    let gunship = spawn(&mut app, P0, "gunship", 10, 10);
    let truck = spawn(&mut app, P1, "truck", 13, 10);
    tick(&mut app, Vec::new());
    let mut events = tick(&mut app, vec![attack(P0, gunship, truck)]);
    events.extend(ticks(&mut app, 200));
    assert!(alive(&mut app, gunship));
    // Its rockets: 30 every 20 ticks until the truck (300 hit points) is gone.
    assert_eq!(shots(&events, gunship).len(), 10);
    assert!(!alive(&mut app, truck));
    assert!(state(&app).ammo.is_empty() && state(&app).docks.is_empty());
    assert_eq!(mobile(&mut app, gunship).cell, CellPos::new(10, 10));
}

// --- Integration and regression ------------------------------------------------------------------

#[test]
fn sorties_keep_the_cells_in_order_and_run_the_same_twice() {
    let run = || {
        let mut app = world();
        place(&mut app, P0, "airfield", 10, 10);
        place(&mut app, P0, "airfield", 10, 14);
        let strikers: Vec<SimId> = (0..4)
            .map(|n| spawn(&mut app, P0, "striker", 14, 10 + n))
            .collect();
        spawn(&mut app, P0, "engineer", 20, 12);
        let target = spawn(&mut app, P1, "allied_mcv", 23, 12);
        let mut hashes = Vec::new();
        let mut fired = 0;
        for n in 0..700 {
            // A new attack whenever all are home and loaded.
            let commands = if n % 175 == 60 && alive(&mut app, target) {
                strikers.iter().map(|&id| attack(P0, id, target)).collect()
            } else {
                Vec::new()
            };
            let events = tick(&mut app, commands);
            fired += strikers
                .iter()
                .map(|&id| shots(&events, id).len())
                .sum::<usize>();
            assert_cells(&mut app, "the sorties");
            hashes.push(hash(&mut app));
        }
        (hashes, fired)
    };
    let (first, fired) = run();
    let (second, again) = run();
    assert_eq!(first, second);
    assert_eq!(fired, again);
    // Four strikers with two volleys of two shots each sortie.
    assert!(fired >= 16, "{fired}");
}

#[test]
fn ammunition_reloading_and_docks_are_hashed() {
    let scenario = |edit: fn(&mut SpecialState, SimId, SimId)| {
        let (mut app, airfield, strikers) = airfield_with(1);
        edit(
            &mut app.world_mut().resource_mut::<SpecialState>(),
            strikers[0],
            airfield,
        );
        hash(&mut app)
    };
    let base = scenario(|_, _, _| {});
    assert_eq!(base, scenario(|_, _, _| {}));
    for (name, edit) in [
        (
            "ammunition",
            (|state, striker, _| {
                state.ammo.insert(striker, 1);
            }) as fn(&mut SpecialState, SimId, SimId),
        ),
        ("reloading", |state, striker, _| {
            state.reloading.insert(striker, 7);
        }),
        ("the dock", |state, striker, airfield| {
            state.docks.insert(striker, (airfield, 1));
        }),
        ("no dock", |state, striker, _| {
            state.docks.remove(&striker);
        }),
    ] {
        assert_ne!(scenario(edit), base, "{name} is not hashed");
    }
}

// --- Regression: advisory review 2, sim P0-1 (docs/DECISIONS.md R56) ---------------------------------

#[test]
fn an_aircraft_spends_its_rounds_on_scenery_as_on_everything_else() {
    // The striker has 2 volleys of 2 shots of 20. Ordered onto a tower (scenery, no actor of
    // the simulation) it spent nothing: it fired 50 shots in 400 ticks and never flew home.
    let (mut app, _, strikers) = airfield_with(1);
    let striker = strikers[0];
    let scenery = tower(&mut app, 18, 10, "plate");
    // An unarmed spotter beside the tower, so that the player has seen its cell.
    spawn(&mut app, P0, "engineer", 17, 11);
    ticks(&mut app, 2);
    let mut events = tick(&mut app, vec![attack(P0, striker, scenery)]);
    assert!(rejections(&events).is_empty(), "{events:?}");
    let (more, _) = run_until(&mut app, 400, |app, _| {
        ammo(app, striker) == Some(0) && landed(app, striker)
    });
    events.extend(more);
    events.extend(ticks(&mut app, 100));
    assert_eq!(shots(&events, striker).len(), 4, "two volleys of two");
    assert_eq!(scenery_hp(&app, 18, 10), Some(1000 - 4 * 20));
    assert!(landed(&mut app, striker));
}

#[test]
fn a_volley_that_is_cut_short_costs_its_round() {
    // Each truck is destroyed by the first shot of a volley, and nothing else is in reach for
    // the second: the volley ends there, and it has cost a round all the same (before: six
    // trucks, six kills, the ammunition still full).
    let (mut app, _, strikers) = airfield_with(1);
    let striker = strikers[0];
    spawn(&mut app, P0, "engineer", 15, 10);
    let mut kills = 0;
    for (n, left) in [(0, Some(1)), (1, Some(0))] {
        let victim = spawn(&mut app, P1, "truck", 18, 8 + 2 * n);
        app.world_mut()
            .resource_mut::<SpecialState>()
            .disabled
            .insert(victim, u64::MAX);
        set_hp(&mut app, victim, 5);
        // A tick, so that the spotter has seen it.
        tick(&mut app, Vec::new());
        let mut events = tick(&mut app, vec![attack(P0, striker, victim)]);
        assert!(rejections(&events).is_empty(), "{events:?}");
        let (more, _) = run_until(&mut app, 200, |app, _| !alive(app, victim));
        events.extend(more);
        events.extend(ticks(&mut app, 12));
        assert_eq!(shots(&events, striker).len(), 1, "victim {n}");
        assert_eq!(ammo(&app, striker), left, "victim {n}");
        assert_eq!(
            weapon(&mut app, striker).burst_left,
            0,
            "the volley is over"
        );
        kills += 1;
    }
    assert_eq!(kills, 2);
    // Out of ammunition it fires at nothing more, and flies home to reload.
    let victim = spawn(&mut app, P1, "truck", 18, 14);
    app.world_mut()
        .resource_mut::<SpecialState>()
        .disabled
        .insert(victim, u64::MAX);
    tick(&mut app, Vec::new());
    let mut events = tick(&mut app, vec![attack(P0, striker, victim)]);
    assert!(rejections(&events).is_empty(), "{events:?}");
    let (more, _) = run_until(&mut app, 200, |app, _| landed(app, striker));
    events.extend(more);
    events.extend(ticks(&mut app, 50));
    assert!(shots(&events, striker).is_empty());
    assert_eq!(hp(&mut app, victim), Some(300));
}

// --- Regression: advisory review 2, tests P1-2 (the loss of a dock by capture) ----------------------

/// An airfield of P0 at (10, 10) with a striker on it and, if `second`, another airfield of P0
/// at (20, 20); an engineer of P1 beside the first captures it through the command queue.
/// Returns the world after the tick of the capture, the two airfields, the striker, and the
/// units destroyed in that tick.
fn captured(second: bool) -> (App, SimId, Option<SimId>, SimId, Vec<SimId>) {
    let (mut app, first, strikers) = airfield_with(1);
    let other = second.then(|| place(&mut app, P0, "airfield", 20, 20));
    let engineer = spawn(&mut app, P1, "engineer", 12, 10);
    tick(&mut app, Vec::new());
    assert_eq!(state(&app).docks[&strikers[0]].0, first);
    let events = tick(
        &mut app,
        vec![issue(
            P1,
            Command::Interact {
                unit: engineer,
                target: first,
            },
        )],
    );
    assert!(rejections(&events).is_empty(), "{events:?}");
    let owner = {
        let world = app.world_mut();
        let mut query = world.query::<(&SimId, &sim::Owner)>();
        query
            .iter(world)
            .find(|(id, _)| **id == first)
            .map(|(_, owner)| owner.0)
    };
    assert_eq!(owner, Some(P1), "captured");
    let lost: Vec<SimId> = destroyed(&events)
        .into_iter()
        .filter(|id| *id != engineer)
        .collect();
    (app, first, other, strikers[0], lost)
}

fn docks_of(app: &App, player: PlayerId) -> (u32, u32) {
    let state = app
        .world()
        .resource::<Players>()
        .get(player)
        .expect("player");
    (state.docks(), state.docked_aircraft())
}

#[test]
fn an_aircraft_whose_airfield_is_captured_takes_a_dock_of_another_or_is_lost() {
    // Decision R49 item 4: a dock is lost with its building when that is destroyed, sold or
    // captured. No test captured one, and an aircraft that kept the dock of a building in
    // another player's hands passed every test (mutation M22 of the review).
    // With another airfield of its owner: it takes a dock there and flies over.
    let (mut app, first, other, striker, lost) = captured(true);
    assert!(lost.is_empty());
    let other = other.expect("the second airfield");
    let (events, _) = run_until(&mut app, 300, |app, _| landed(app, striker));
    assert!(destroyed(&events).is_empty());
    assert_eq!(state(&app).docks[&striker].0, other);
    assert_ne!(state(&app).docks[&striker].0, first);
    assert_eq!(mobile(&mut app, striker).cell, CellPos::new(20, 20));
    // The docks follow the buildings: two each, and the aircraft is its owner's.
    assert_eq!(docks_of(&app, P0), (2, 1));
    assert_eq!(docks_of(&app, P1), (2, 0));
    // Without one it has no dock left and, idle, is lost in the tick of the capture.
    let (mut app, _, _, striker, lost) = captured(false);
    assert_eq!(lost, [striker]);
    assert!(!alive(&mut app, striker));
    assert!(!state(&app).docks.contains_key(&striker));
    tick(&mut app, Vec::new());
    assert_eq!(docks_of(&app, P0), (0, 0));
    assert_eq!(docks_of(&app, P1), (2, 0));
}

// --- Regression: advisory review 2, sim P2-1 --------------------------------------------------------

#[test]
fn a_dock_sold_earlier_in_the_tick_takes_no_aircraft() {
    // The check counted the docks the player had at the last `Economy` stage if that was
    // more than those of its buildings: with two airfields (4 docks), the sale of one and five
    // orders in one tick, four aircraft were accepted for the 2 docks left, and the third was
    // built, paid for and waited at the head of the queue for ever.
    let mut app = world();
    let first = place(&mut app, P0, "airfield", 10, 10);
    place(&mut app, P0, "airfield", 20, 20);
    place(&mut app, P0, "allied_yard", 2, 2);
    place(&mut app, P0, "allied_factory", 6, 2);
    ticks(&mut app, 2);
    let mut commands = vec![issue(P0, Command::SellBuilding { building: first })];
    commands.extend((0..5).map(|_| queue(&app, P0)));
    let events = tick(&mut app, commands);
    assert_eq!(
        rejections(&events),
        vec![RejectReason::NoFreeDock(striker_item(&app)); 3]
    );
    // Control: without the sale four are accepted and the fifth is refused.
    let mut app = world();
    place(&mut app, P0, "airfield", 10, 10);
    place(&mut app, P0, "airfield", 20, 20);
    place(&mut app, P0, "allied_yard", 2, 2);
    place(&mut app, P0, "allied_factory", 6, 2);
    ticks(&mut app, 2);
    let orders = (0..5).map(|_| queue(&app, P0)).collect();
    let events = tick(&mut app, orders);
    assert_eq!(
        rejections(&events),
        [RejectReason::NoFreeDock(striker_item(&app))]
    );
}
