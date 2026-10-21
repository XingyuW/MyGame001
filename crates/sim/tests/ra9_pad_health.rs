//! RA9 proposed UnitReload health service, native CommandQueue/SimSchedule only.
//! Controlled initial HP/ammunition/disable/timer setup is not a claim that a command caused damage.
mod ra4_common;
use bevy_app::App;
use bevy_ecs::entity::Entity;
use ra4_common::*;
use sim::{
    Building, Cell, CellPos, Command, Grid, Owner, Players, ProductionItem, SimEvent, SimId,
    SimPlugin, SpecialState,
};
fn sp_entity(app: &mut App, id: SimId) -> Option<Entity> {
    let world = app.world_mut();
    world
        .query::<(Entity, &SimId)>()
        .iter(world)
        .find(|(_, found)| **found == id)
        .map(|(entity, _)| entity)
}
fn sp_credits(app: &App) -> i64 {
    app.world().resource::<Players>().get(P0).unwrap().credits()
}
fn sp_state(app: &App) -> &SpecialState {
    app.world().resource::<SpecialState>()
}
fn sp_case(
    shipped: bool,
    aircraft: &str,
    credits: i64,
    hp: i32,
    ammo: Option<u32>,
) -> (App, SimId, SimId) {
    sp_case_pad(shipped, aircraft, credits, hp, ammo, "airforce_command")
}
fn sp_case_pad(
    shipped: bool,
    aircraft: &str,
    credits: i64,
    hp: i32,
    ammo: Option<u32>,
    shipped_pad: &str,
) -> (App, SimId, SimId) {
    let definitions = if shipped {
        sim::load_ruleset(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data"),
        )
        .unwrap()
    } else {
        definitions()
    };
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 271 });
    let open = Cell {
        passable: true,
        water: false,
        buildable: true,
        ore: 0,
    };
    app.world_mut()
        .insert_resource(Grid::from_cells(40, 40, vec![open; 1600]).unwrap());
    let countries = if shipped {
        ["great_britain", "russia"]
    } else {
        ["alpha", "gamma"]
    };
    for (player, country) in [(P0, countries[0]), (P1, countries[1])] {
        let country = definitions.rules().unwrap().country_id(country).unwrap();
        app.world_mut()
            .resource_mut::<Players>()
            .add(player, if player == P0 { credits } else { 5000 })
            .unwrap();
        assert!(
            app.world_mut()
                .resource_mut::<Players>()
                .set_country(player, Some(country))
        );
    }
    app.world_mut().insert_resource(definitions);
    let pad = place(
        &mut app,
        P0,
        if shipped { shipped_pad } else { "airfield" },
        10,
        10,
    );
    let pad_entity = sp_entity(&mut app, pad).unwrap();
    let footprint = app.world().get::<Building>(pad_entity).unwrap().footprint;
    let cell = sim::dock_cell(footprint, 0);
    let plane = spawn(&mut app, P0, aircraft, cell.x, cell.y);
    set_hp(&mut app, plane, hp);
    if let Some(ammo) = ammo {
        app.world_mut()
            .resource_mut::<SpecialState>()
            .ammo
            .insert(plane, ammo);
    }
    assert_cells(&mut app, "initial legal dock cell");
    (app, pad, plane)
}
fn sp_no_reject(events: &[SimEvent]) {
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
}

#[test]
fn ra9_pad_ammo_finishes_before_fresh_paid_health_interval_and_clamps() {
    let (mut app, pad, plane) = sp_case(false, "striker", 5000, 85, Some(1));
    let definitions = defs(&app);
    let rules = &definitions.rules().unwrap().economy;
    assert_eq!(
        (
            rules.reload_interval,
            rules.repair_step,
            rules.repair_percent
        ),
        (40, 10, 20)
    );
    ticks(&mut app, 39);
    assert_eq!(hp(&mut app, plane), Some(85));
    assert_eq!(sp_state(&app).ammo.get(&plane), Some(&1));
    assert_eq!(sp_credits(&app), 5000);
    tick(&mut app, vec![]);
    assert_eq!(sp_state(&app).ammo.get(&plane), None);
    assert_eq!(sp_state(&app).docks.get(&plane), Some(&(pad, 0)));
    assert_eq!(
        hp(&mut app, plane),
        Some(85),
        "no simultaneous final ammo and heal"
    );
    ticks(&mut app, 39);
    assert_eq!(hp(&mut app, plane), Some(85));
    tick(&mut app, vec![]);
    assert_eq!(hp(&mut app, plane), Some(95));
    assert_eq!(sp_credits(&app), 4994, "300/(100/10)*20%=6");
    ticks(&mut app, 40);
    assert_eq!(hp(&mut app, plane), Some(100));
    assert_eq!(
        sp_credits(&app),
        4988,
        "clamped last step still costs one step"
    );
    ticks(&mut app, 80);
    assert_eq!(hp(&mut app, plane), Some(100));
    assert_eq!(sp_credits(&app), 4988);
    assert!(!sp_state(&app).reloading.contains_key(&plane));
    assert_cells(&mut app, "after paid reload service");
}
#[test]
fn ra9_pad_health_zero_and_exact_step_credit_boundary() {
    for (credits, expected_hp, expected_credits) in [(0, 80, 0), (5, 80, 5), (6, 90, 0)] {
        let (mut app, _, plane) = sp_case(false, "striker", credits, 80, None);
        ticks(&mut app, 40);
        assert_eq!(hp(&mut app, plane), Some(expected_hp));
        assert_eq!(sp_credits(&app), expected_credits);
        assert!(
            !sp_state(&app).reloading.contains_key(&plane),
            "paid or skipped step consumes interval"
        );
        ticks(&mut app, 40);
        assert_eq!(hp(&mut app, plane), Some(expected_hp));
        assert_eq!(sp_credits(&app), expected_credits);
    }
}
#[test]
fn ra9_pad_health_leaving_discards_elapsed_service_then_native_return_restarts() {
    let (mut app, _, plane) = sp_case(false, "striker", 5000, 80, None);
    ticks(&mut app, 39);
    assert_eq!(sp_state(&app).reloading.get(&plane), Some(&39));
    sp_no_reject(&tick(&mut app, vec![move_to(P0, &[plane], 20, 20)]));
    assert!(!sp_state(&app).reloading.contains_key(&plane));
    assert_eq!(hp(&mut app, plane), Some(80));
    let mut reached = false;
    let mut restarted = false;
    for _ in 0..400 {
        tick(&mut app, vec![]);
        assert_eq!(hp(&mut app, plane), Some(80));
        reached |= mobile(&mut app, plane).cell == CellPos::new(20, 20);
        if reached && sp_state(&app).reloading.get(&plane) == Some(&1) {
            restarted = true;
            break;
        }
    }
    assert!(
        reached && restarted,
        "actual out-and-back path and Economy landing observation"
    );
    ticks(&mut app, 38);
    assert_eq!(hp(&mut app, plane), Some(80));
    tick(&mut app, vec![]);
    assert_eq!(hp(&mut app, plane), Some(90));
    assert_eq!(sp_credits(&app), 4994);
    assert_cells(&mut app, "real move away and dock return");
}
#[test]
fn ra9_pad_health_waits_for_real_disabled_expiry() {
    let (mut app, pad, plane) = sp_case(false, "striker", 5000, 80, None);
    // Controlled initial condition; native Economy expires it. No command-disable claim.
    app.world_mut()
        .resource_mut::<SpecialState>()
        .disabled
        .insert(pad, 20);
    ticks(&mut app, 20); // Tick indices 0..19 are disabled.
    assert_eq!(hp(&mut app, plane), Some(80));
    assert_eq!(sp_credits(&app), 5000);
    assert!(!sp_state(&app).reloading.contains_key(&plane));
    tick(&mut app, vec![]); // Tick index 20 drops disabled and starts a fresh service interval.
    assert!(!sp_state(&app).disabled.contains_key(&pad));
    assert_eq!(sp_state(&app).reloading.get(&plane), Some(&1));
    ticks(&mut app, 38);
    assert_eq!(hp(&mut app, plane), Some(80));
    tick(&mut app, vec![]);
    assert_eq!(hp(&mut app, plane), Some(90));
    assert_eq!(sp_credits(&app), 4994);
}
#[test]
fn ra9_shipped_aircraft_use_committed_reload_and_paid_repair_values_even_when_radar_is_off() {
    for pad_id in ["airforce_command", "american_airforce_command"] {
        for (name, max, cost) in [("harrier", 150, 9), ("black_eagle", 200, 7)] {
            for with_power in [false, true] {
                let (mut app, pad, plane) =
                    sp_case_pad(true, name, 10_000, max - 10, Some(0), pad_id);
                if with_power {
                    place(&mut app, P0, "nuclear_reactor", 2, 2);
                }
                let definitions = defs(&app);
                let kind = definitions.unit_kind(name).unwrap();
                assert_eq!(
                    definitions
                        .production(ProductionItem::Unit(kind))
                        .unwrap()
                        .cost,
                    1200
                );
                assert_eq!(definitions.unit_combat(kind).unwrap().hit_points, max);
                assert_eq!(definitions.rules().unwrap().economy.reload_interval, 360);
                assert_eq!(definitions.rules().unwrap().economy.repair_step, 8);
                assert_eq!(definitions.rules().unwrap().economy.repair_percent, 15);
                ticks(&mut app, 360);
                assert_eq!(sp_state(&app).ammo.get(&plane), None);
                assert_eq!(hp(&mut app, plane), Some(max - 10));
                assert_eq!(sp_credits(&app), 10_000);
                let pad_entity = sp_entity(&mut app, pad).unwrap();
                let pad_kind = app.world().get::<Building>(pad_entity).unwrap().kind;
                let player = app.world().resource::<Players>().get(P0).unwrap();
                assert_eq!(player.low_power(), !with_power);
                assert_eq!(player.radar(), with_power);
                assert!(
                    player.powers(&definitions, pad_kind),
                    "stock Powered=no services in low power"
                );
                ticks(&mut app, 359);
                assert_eq!(hp(&mut app, plane), Some(max - 10));
                tick(&mut app, vec![]);
                assert_eq!(hp(&mut app, plane), Some(max - 2));
                assert_eq!(sp_credits(&app), 10_000 - cost);
                ticks(&mut app, 360);
                assert_eq!(hp(&mut app, plane), Some(max));
                assert_eq!(sp_credits(&app), 10_000 - 2 * cost);
                assert_cells(&mut app, "shipped paid health service");
            }
        }
    }
}

#[test]
fn ra9_pad_capture_in_same_economy_stage_does_not_heal_from_stale_actor_owner() {
    for capture in [false, true] {
        let (mut app, pad, plane) = sp_case(true, "harrier", 10_000, 140, None);
        // Both arms retain a remote live P0 pad. Losing the measured pad alone would
        // legitimately destroy an idle AirportBound aircraft and mask the service boundary.
        assert_eq!(app.world().resource::<sim::SimTick>().0, 0);
        let fallback = place(&mut app, P0, "airforce_command", 28, 28);
        // Allied houses suppress incidental aircraft auto-fire. Both arms use identical
        // geometry/teams and actual one-cell approach; only the pending capture intent differs.
        assert!(app.world_mut().resource_mut::<Players>().set_team(P0, 1));
        assert!(app.world_mut().resource_mut::<Players>().set_team(P1, 1));
        let engineer = spawn(&mut app, P1, "engineer", 14, 10);
        let engineer_entity = sp_entity(&mut app, engineer).unwrap();
        let pad_entity = sp_entity(&mut app, pad).unwrap();
        let footprint = app.world().get::<Building>(pad_entity).unwrap().footprint;
        assert_eq!(footprint.distance_to_cell(CellPos::new(14, 10)), 2);
        assert_eq!(footprint.distance_to_cell(CellPos::new(13, 10)), 1);
        assert_eq!(
            app.world().resource::<Players>().unit_speed_percent(P1),
            100
        );
        let speed = app
            .world()
            .get::<sim::Mobile>(engineer_entity)
            .unwrap()
            .speed;
        let travel = u32::from(sim::ORTHOGONAL_STEP).div_ceil(u32::from(speed));
        let interval = u32::try_from(defs(&app).rules().unwrap().economy.reload_interval).unwrap();
        assert_eq!(interval, 360);
        assert!(travel > 1 && travel + 2 < interval);
        // Pre-first-tick controlled clock only: one sight tick, real one-cell travel,
        // then the capture/due tick. No timer/Owner/Health injection while native ticks run.
        app.world_mut()
            .resource_mut::<SpecialState>()
            .reloading
            .insert(plane, interval.checked_sub(travel + 2).unwrap());
        sp_no_reject(&tick(&mut app, vec![])); // Actual scheduled P1 sight precedes Interact.
        assert_eq!(app.world().get::<Owner>(pad_entity).unwrap().0, P0);
        assert!(!sp_state(&app).interactions.contains_key(&engineer));
        for step in 0..travel {
            let commands = if step == 0 {
                vec![issue(
                    P1,
                    if capture {
                        Command::Interact {
                            unit: engineer,
                            target: pad,
                        }
                    } else {
                        Command::Move {
                            units: vec![engineer],
                            target: CellPos::new(13, 10),
                        }
                    },
                )]
            } else {
                vec![]
            };
            sp_no_reject(&tick(&mut app, commands));
            assert_eq!(
                sp_state(&app).interactions.get(&engineer).copied(),
                if capture { Some(pad) } else { None },
                "real queued approach, not adjacent Commands capture"
            );
            let pad_entity = sp_entity(&mut app, pad).unwrap();
            assert_eq!(app.world().get::<Owner>(pad_entity).unwrap().0, P0);
            assert_eq!(hp(&mut app, plane), Some(140));
            assert_eq!(sp_credits(&app), 10_000);
            assert_eq!(
                sp_state(&app).reloading.get(&plane).copied(),
                Some(interval - travel + step),
                "only real scheduled service advances the clock"
            );
            assert_cells(&mut app, "public pending Engineer approach");
        }
        let engineer_entity = sp_entity(&mut app, engineer).unwrap();
        let arrived = app.world().get::<sim::Mobile>(engineer_entity).unwrap();
        assert_eq!(arrived.cell, CellPos::new(13, 10));
        assert!(arrived.step.is_none());
        assert_eq!(sp_state(&app).reloading.get(&plane), Some(&359));
        assert_eq!(sp_state(&app).docks.get(&plane), Some(&(pad, 0)));
        // The already-pending interaction resolves AFTER special::advance creates its
        // Actor snapshot, and BEFORE flight::docks in this SAME Economy stage.
        assert_eq!(
            app.world().resource::<sim::SimTick>().0,
            u64::from(travel + 1)
        );
        let capture_events = tick(&mut app, vec![]);
        sp_no_reject(&capture_events);
        assert!(
            !capture_events.iter().any(
                |event| matches!(event, SimEvent::UnitDestroyed { unit, .. } if *unit == plane)
            )
        );
        assert_eq!(
            app.world().resource::<sim::SimTick>().0,
            u64::from(travel + 2)
        );
        assert_eq!(
            sp_entity(&mut app, engineer).is_none(),
            capture,
            "actual native capture consumes only its Engineer"
        );
        assert!(!sp_state(&app).interactions.contains_key(&engineer));
        let entity = sp_entity(&mut app, pad).unwrap();
        assert_eq!(
            app.world().get::<Owner>(entity).unwrap().0,
            if capture { P1 } else { P0 }
        );
        assert_eq!(hp(&mut app, plane), Some(if capture { 140 } else { 148 }));
        assert_eq!(sp_credits(&app), if capture { 10_000 } else { 9991 });
        if capture {
            assert!(!sp_state(&app).reloading.contains_key(&plane));
        }
        let plane_entity = sp_entity(&mut app, plane).expect("actual surviving aircraft");
        assert_eq!(app.world().get::<Owner>(plane_entity).unwrap().0, P0);
        let fallback_entity = sp_entity(&mut app, fallback).unwrap();
        assert_eq!(app.world().get::<Owner>(fallback_entity).unwrap().0, P0);
        assert!(hp(&mut app, fallback).unwrap() > 0);
        // This is intentionally the old dock on the capture tick: the indexed owner was
        // P0, while its ACTUAL current Owner above is P1. Service must re-read that mismatch.
        assert_eq!(sp_state(&app).docks.get(&plane), Some(&(pad, 0)));
        let events = tick(&mut app, vec![]);
        sp_no_reject(&events);
        assert!(
            !events.iter().any(
                |event| matches!(event, SimEvent::UnitDestroyed { unit, .. } if *unit == plane)
            )
        );
        assert_eq!(
            sp_state(&app).docks.get(&plane),
            Some(&(if capture { fallback } else { pad }, 0)),
            "fresh next-tick ownership rehomes only the captured-pad arm"
        );
        let plane_entity = sp_entity(&mut app, plane).expect("aircraft survives actual rehoming");
        assert_eq!(app.world().get::<Owner>(plane_entity).unwrap().0, P0);
        assert_cells(&mut app, "native pending capture and owned fallback pad");
        assert_eq!(hp(&mut app, plane), Some(if capture { 140 } else { 148 }));
        assert_eq!(sp_credits(&app), if capture { 10_000 } else { 9991 });
    }
}
#[test]
fn ra9_pad_sale_or_dead_aircraft_never_pays_or_revives() {
    let (mut app, pad, plane) = sp_case(false, "striker", 5000, 80, None);
    ticks(&mut app, 39);
    let entity = sp_entity(&mut app, pad).unwrap();
    let kind = app.world().get::<Building>(entity).unwrap().kind;
    let refund = defs(&app).refund(kind);
    sp_no_reject(&tick(
        &mut app,
        vec![issue(P0, Command::SellBuilding { building: pad })],
    ));
    assert_eq!(
        hp(&mut app, plane),
        None,
        "idle airport-bound aircraft loses its last pad"
    );
    assert_eq!(sp_credits(&app), 5000 + refund);
    assert!(!sp_state(&app).reloading.contains_key(&plane));
    let (mut app, _, dead) = sp_case(false, "striker", 5000, 0, None);
    app.world_mut()
        .resource_mut::<SpecialState>()
        .reloading
        .insert(dead, 39);
    tick(&mut app, vec![]);
    assert_eq!(
        hp(&mut app, dead),
        None,
        "controlled pre-Cleanup zero HP cannot revive"
    );
    assert_eq!(sp_credits(&app), 5000);
}
#[test]
fn ra9_pad_paid_service_replays_with_identical_native_hashes_and_cell_claims() {
    let (mut a, _, ua) = sp_case(false, "striker", 5000, 75, Some(0));
    let (mut b, _, ub) = sp_case(false, "striker", 5000, 75, Some(0));
    for n in 0..240 {
        sp_no_reject(&tick(&mut a, vec![]));
        sp_no_reject(&tick(&mut b, vec![]));
        assert_eq!(
            sim::state_hash(a.world_mut()),
            sim::state_hash(b.world_mut()),
            "hash tick {n}"
        );
        assert_cells(&mut a, "paid replay A");
        assert_cells(&mut b, "paid replay B");
    }
    assert_eq!(hp(&mut a, ua), Some(100));
    assert_eq!(hp(&mut b, ub), Some(100));
    assert_eq!(sp_credits(&a), 4982);
    assert_eq!(sp_credits(&a), sp_credits(&b));
}

#[test]
fn ra9_pad_health_has_healthy_nonairport_and_dead_pad_controls() {
    for (unit, start_hp) in [("striker", 100), ("gunship", 80)] {
        let (mut app, _, plane) = sp_case(false, unit, 5000, start_hp, None);
        ticks(&mut app, 120);
        assert_eq!(hp(&mut app, plane), Some(start_hp));
        assert_eq!(sp_credits(&app), 5000);
        assert!(!sp_state(&app).reloading.contains_key(&plane));
    }
    let (mut app, pad, plane) = sp_case(false, "striker", 5000, 80, None);
    set_hp(&mut app, pad, 0); // Controlled pre-Cleanup state; not a claimed destruction command.
    app.world_mut()
        .resource_mut::<SpecialState>()
        .reloading
        .insert(plane, 39);
    tick(&mut app, vec![]);
    assert_eq!(hp(&mut app, pad), None);
    assert_eq!(hp(&mut app, plane), None);
    assert_eq!(sp_credits(&app), 5000);
}

#[test]
fn ra9_pad_capture_also_prevents_due_ammo_service_from_stale_owner() {
    for capture in [false, true] {
        let (mut app, pad, plane) = sp_case(true, "harrier", 10_000, 140, Some(0));
        // Both arms retain a remote live P0 pad. Losing the measured pad alone would
        // legitimately destroy an idle AirportBound aircraft and mask the service boundary.
        assert_eq!(app.world().resource::<sim::SimTick>().0, 0);
        let fallback = place(&mut app, P0, "airforce_command", 28, 28);
        // Allied houses suppress incidental aircraft auto-fire. Both arms use identical
        // geometry/teams and actual one-cell approach; only the pending capture intent differs.
        assert!(app.world_mut().resource_mut::<Players>().set_team(P0, 1));
        assert!(app.world_mut().resource_mut::<Players>().set_team(P1, 1));
        let engineer = spawn(&mut app, P1, "engineer", 14, 10);
        let engineer_entity = sp_entity(&mut app, engineer).unwrap();
        let pad_entity = sp_entity(&mut app, pad).unwrap();
        let footprint = app.world().get::<Building>(pad_entity).unwrap().footprint;
        assert_eq!(footprint.distance_to_cell(CellPos::new(14, 10)), 2);
        assert_eq!(footprint.distance_to_cell(CellPos::new(13, 10)), 1);
        assert_eq!(
            app.world().resource::<Players>().unit_speed_percent(P1),
            100
        );
        let speed = app
            .world()
            .get::<sim::Mobile>(engineer_entity)
            .unwrap()
            .speed;
        let travel = u32::from(sim::ORTHOGONAL_STEP).div_ceil(u32::from(speed));
        let interval = u32::try_from(defs(&app).rules().unwrap().economy.reload_interval).unwrap();
        assert_eq!(interval, 360);
        assert!(travel > 1 && travel + 2 < interval);
        // Pre-first-tick controlled clock only: one sight tick, real one-cell travel,
        // then the capture/due tick. No timer/Owner/Health injection while native ticks run.
        app.world_mut()
            .resource_mut::<SpecialState>()
            .reloading
            .insert(plane, interval.checked_sub(travel + 2).unwrap());
        sp_no_reject(&tick(&mut app, vec![])); // Actual scheduled P1 sight precedes Interact.
        assert_eq!(app.world().get::<Owner>(pad_entity).unwrap().0, P0);
        assert!(!sp_state(&app).interactions.contains_key(&engineer));
        for step in 0..travel {
            let commands = if step == 0 {
                vec![issue(
                    P1,
                    if capture {
                        Command::Interact {
                            unit: engineer,
                            target: pad,
                        }
                    } else {
                        Command::Move {
                            units: vec![engineer],
                            target: CellPos::new(13, 10),
                        }
                    },
                )]
            } else {
                vec![]
            };
            sp_no_reject(&tick(&mut app, commands));
            assert_eq!(
                sp_state(&app).interactions.get(&engineer).copied(),
                if capture { Some(pad) } else { None },
                "real queued approach, not adjacent Commands capture"
            );
            let pad_entity = sp_entity(&mut app, pad).unwrap();
            assert_eq!(app.world().get::<Owner>(pad_entity).unwrap().0, P0);
            assert_eq!(hp(&mut app, plane), Some(140));
            assert_eq!(sp_credits(&app), 10_000);
            assert_eq!(
                sp_state(&app).reloading.get(&plane).copied(),
                Some(interval - travel + step),
                "only real scheduled service advances the clock"
            );
            assert_cells(&mut app, "public pending Engineer approach");
        }
        let engineer_entity = sp_entity(&mut app, engineer).unwrap();
        let arrived = app.world().get::<sim::Mobile>(engineer_entity).unwrap();
        assert_eq!(arrived.cell, CellPos::new(13, 10));
        assert!(arrived.step.is_none());
        assert_eq!(sp_state(&app).reloading.get(&plane), Some(&359));
        assert_eq!(sp_state(&app).docks.get(&plane), Some(&(pad, 0)));
        // The already-pending interaction resolves AFTER special::advance creates its
        // Actor snapshot, and BEFORE flight::docks in this SAME Economy stage.
        assert_eq!(
            app.world().resource::<sim::SimTick>().0,
            u64::from(travel + 1)
        );
        let capture_events = tick(&mut app, vec![]);
        sp_no_reject(&capture_events);
        assert!(
            !capture_events.iter().any(
                |event| matches!(event, SimEvent::UnitDestroyed { unit, .. } if *unit == plane)
            )
        );
        assert_eq!(
            app.world().resource::<sim::SimTick>().0,
            u64::from(travel + 2)
        );
        assert_eq!(
            sp_entity(&mut app, engineer).is_none(),
            capture,
            "actual native capture consumes only its Engineer"
        );
        assert!(!sp_state(&app).interactions.contains_key(&engineer));
        let entity = sp_entity(&mut app, pad).unwrap();
        assert_eq!(
            app.world().get::<Owner>(entity).unwrap().0,
            if capture { P1 } else { P0 }
        );
        assert_eq!(
            sp_state(&app).ammo.get(&plane).copied(),
            if capture { Some(0) } else { None }
        );
        assert_eq!(
            hp(&mut app, plane),
            Some(140),
            "no health on final ammo interval"
        );
        assert_eq!(sp_credits(&app), 10_000);
        assert!(!sp_state(&app).reloading.contains_key(&plane));
        let plane_entity = sp_entity(&mut app, plane).expect("actual surviving aircraft");
        assert_eq!(app.world().get::<Owner>(plane_entity).unwrap().0, P0);
        let fallback_entity = sp_entity(&mut app, fallback).unwrap();
        assert_eq!(app.world().get::<Owner>(fallback_entity).unwrap().0, P0);
        assert!(hp(&mut app, fallback).unwrap() > 0);
        // This is intentionally the old dock on the capture tick: the indexed owner was
        // P0, while its ACTUAL current Owner above is P1. Service must re-read that mismatch.
        assert_eq!(sp_state(&app).docks.get(&plane), Some(&(pad, 0)));
        let events = tick(&mut app, vec![]);
        sp_no_reject(&events);
        assert!(
            !events.iter().any(
                |event| matches!(event, SimEvent::UnitDestroyed { unit, .. } if *unit == plane)
            )
        );
        assert_eq!(
            sp_state(&app).docks.get(&plane),
            Some(&(if capture { fallback } else { pad }, 0)),
            "fresh next-tick ownership rehomes only the captured-pad arm"
        );
        let plane_entity = sp_entity(&mut app, plane).expect("aircraft survives actual rehoming");
        assert_eq!(app.world().get::<Owner>(plane_entity).unwrap().0, P0);
        assert_cells(&mut app, "native pending capture and owned fallback pad");
        assert_eq!(
            sp_state(&app).ammo.get(&plane).copied(),
            if capture { Some(0) } else { None }
        );
        assert_eq!(
            hp(&mut app, plane),
            Some(140),
            "no health on final ammo interval"
        );
        assert_eq!(sp_credits(&app), 10_000);
    }
}
