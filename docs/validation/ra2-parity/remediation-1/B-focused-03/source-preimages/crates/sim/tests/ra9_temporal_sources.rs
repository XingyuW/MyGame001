//! Real Temporal channels suspend active frozen sources and preserve release behavior.
mod ra9_runtime_common;
use ra9_runtime_common::*;
use sim::{
    Building, Cell, CellPos, Command, Grid, HarvestState, Harvester, Mobile, Orders, Players,
    ProductionItem, SimEvent, SimId, SpecialState,
};

#[test]
fn ra9_erased_building_sell_rejects_without_mutation_then_release() {
    erased_building_action(false);
}

#[test]
fn ra9_erased_building_repack_rejects_without_mutation_then_release() {
    erased_building_action(true);
}

fn erased_building_action(repack: bool) {
    let mut app = world();
    let yard = put(&mut app, P0, "construction_yard", 12, 12);
    let caster = put(&mut app, P1, "chrono_legionnaire", 10, 12);
    start_beam(&mut app, caster, yard);
    let e = entity(&mut app, yard).unwrap();
    let before = app.world().get::<Building>(e).unwrap().clone();
    let money = credits(&app);
    let command = if repack {
        Command::Undeploy { building: yard }
    } else {
        Command::SellBuilding { building: yard }
    };
    let events = tick(&mut app, vec![issue(P0, command.clone())]);
    assert!(
        events.iter().any(|event| matches!(
            event,
            SimEvent::CommandRejected {
                reason: sim::RejectReason::AbilityUnavailable("Target is being erased"),
                ..
            }
        )),
        "frozen source mutation must reject: {events:?}"
    );
    assert_eq!(credits(&app), money);
    let e = entity(&mut app, yard).expect("rejection retains original building");
    let after = app
        .world()
        .get::<Building>(e)
        .expect("rejection cannot repack");
    assert_eq!(after.kind, before.kind);
    assert_eq!(after.footprint, before.footprint);
    for cell in before.footprint.cells() {
        assert_eq!(app.world().resource::<Grid>().building(cell), Some(yard));
    }
    assert!(!events.iter().any(|event| matches!(
        event,
        SimEvent::BuildingSold { .. }
            | SimEvent::BuildingUndeployed { .. }
            | SimEvent::CrewEscaped { .. }
    )));
    frozen(&mut app, yard);
    release_beam(&mut app, caster, yard);
    let events = tick(&mut app, vec![issue(P0, command)]);
    assert!(
        rejected(&events).is_empty(),
        "same legal action after release: {events:?}"
    );
    if repack {
        let e = entity(&mut app, yard).unwrap();
        assert!(app.world().get::<sim::Unit>(e).is_some());
        assert!(app.world().get::<Building>(e).is_none());
    } else {
        assert!(entity(&mut app, yard).is_none());
        assert!(credits(&app) > money);
    }
    cells(&mut app);
}

fn ore(app: &mut bevy_app::App, at: CellPos) {
    app.world_mut()
        .resource_mut::<Grid>()
        .set_terrain(at, Cell { ore: 1000, ..OPEN });
}

fn miner_snapshot(app: &mut bevy_app::App, miner: SimId) -> (Harvester, CellPos, i64, u32) {
    let e = entity(app, miner).unwrap();
    let h = *app.world().get::<Harvester>(e).unwrap();
    let at = app.world().get::<Mobile>(e).unwrap().cell;
    (
        h,
        at,
        credits(app),
        app.world().resource::<Grid>().cell(at).unwrap().ore,
    )
}

#[test]
fn ra9_erased_miner_preserves_due_work_ore_load_and_resumes_after_release() {
    let mut app = world();
    put(&mut app, P1, "engineer", 9, 12); // real shared sight before timed beam boundary
    let at = CellPos::new(12, 12);
    ore(&mut app, at);
    let miner = put(&mut app, P0, "war_miner", 12, 12);
    disarm(&mut app, miner);
    let interval = defs(&app)
        .harvester(defs(&app).unit_kind("war_miner").unwrap())
        .unwrap()
        .harvest_interval;
    for _ in 0..200 {
        let e = entity(&mut app, miner).unwrap();
        if app.world().get::<Harvester>(e).unwrap().work + 2 == interval {
            break;
        }
        tick(&mut app, vec![]);
    }
    let caster = put(&mut app, P1, "chrono_legionnaire", 9, 12);
    start_beam(&mut app, caster, miner);
    let (before, held, money, left) = miner_snapshot(&mut app, miner);
    assert_eq!(
        before.work + 1,
        interval,
        "channel must span a real due boundary"
    );
    assert_eq!(before.load, 0);
    for _ in 0..interval + 2 {
        let events = tick(&mut app, vec![]);
        frozen(&mut app, miner);
        let (now, at, paid, ore) = miner_snapshot(&mut app, miner);
        assert_eq!(
            (now.work, now.load, now.gems, now.state, now.attempts),
            (
                before.work,
                before.load,
                before.gems,
                before.state,
                before.attempts
            ),
            "a real frozen worker cannot advance work"
        );
        assert_eq!((at, paid, ore), (held, money, left));
        assert!(
            !events.iter().any(
                |e| matches!(e, SimEvent::OreDelivered { harvester, .. } if *harvester == miner)
            )
        );
    }
    release_beam(&mut app, caster, miner);
    tick(&mut app, vec![]);
    let (after, _, _, remaining) = miner_snapshot(&mut app, miner);
    assert!(
        after.load > before.load,
        "released positive resumes real harvest"
    );
    assert!(remaining < left);
}

#[test]
fn ra9_erased_loaded_miner_cannot_pay_until_native_release() {
    let mut app = world();
    put(&mut app, P1, "engineer", 9, 12); // real shared sight before timed beam boundary
    let at = CellPos::new(13, 12);
    ore(&mut app, at);
    let refinery = put(&mut app, P0, "soviet_refinery", 14, 12);
    let miner = put(&mut app, P0, "war_miner", 13, 12);
    disarm(&mut app, miner);
    let d = defs(&app);
    let terms = d.harvester(d.unit_kind("war_miner").unwrap()).unwrap();
    let mut ready = false;
    for _ in 0..3000 {
        let e = entity(&mut app, miner).unwrap();
        let h = app.world().get::<Harvester>(e).unwrap();
        if h.load + terms.harvest_rate >= terms.capacity && h.work + 1 == terms.harvest_interval {
            ready = true;
            break;
        }
        tick(&mut app, vec![]);
    }
    assert!(ready, "native load must actually reach last-bail boundary");
    let caster = put(&mut app, P1, "chrono_legionnaire", 10, 12);
    start_beam(&mut app, caster, miner);
    let (before, held, money, _) = miner_snapshot(&mut app, miner);
    assert_eq!(before.load, terms.capacity);
    assert!(
        matches!(before.state, HarvestState::ToRefinery { refinery: target, dock } if target == refinery && dock == held)
    );
    let e = entity(&mut app, miner).unwrap();
    assert!(app.world().get::<Orders>(e).unwrap().current.is_none());
    let events = tick(&mut app, vec![]);
    frozen(&mut app, miner);
    let (now, at, paid, _) = miner_snapshot(&mut app, miner);
    assert_eq!(
        (now.load, now.gems, now.state, at, paid),
        (before.load, before.gems, before.state, held, money),
        "frozen adjacent load cannot pay"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::OreDelivered { harvester, .. } if *harvester == miner))
    );
    release_beam(&mut app, caster, miner);
    let events = tick(&mut app, vec![]);
    let expected = i64::from(before.load)
        * d.building(d.building_kind("soviet_refinery").unwrap())
            .unwrap()
            .credits_per_ore
            .unwrap();
    assert!(events.iter().any(|e| matches!(e, SimEvent::OreDelivered { harvester, credits, .. } if *harvester == miner && *credits == expected)));
    assert_eq!(credits(&app), money + expected);
    cells(&mut app);
}

#[test]
fn ra9_erased_chrono_miner_cannot_start_return_teleport() {
    let mut app = world();
    put(&mut app, P1, "engineer", 9, 12); // real shared sight before timed beam boundary
    let at = CellPos::new(12, 12);
    ore(&mut app, at);
    put(&mut app, P0, "refinery", 30, 30);
    let miner = put(&mut app, P0, "chrono_miner", 12, 12);
    let d = defs(&app);
    let terms = d.harvester(d.unit_kind("chrono_miner").unwrap()).unwrap();
    let mut ready = false;
    for _ in 0..3000 {
        let e = entity(&mut app, miner).unwrap();
        let h = app.world().get::<Harvester>(e).unwrap();
        if h.load + terms.harvest_rate >= terms.capacity && h.work + 2 == terms.harvest_interval {
            ready = true;
            break;
        }
        tick(&mut app, vec![]);
    }
    assert!(ready, "actual native last-bail precursor");
    let caster = put(&mut app, P1, "chrono_legionnaire", 9, 12);
    start_beam(&mut app, caster, miner);
    let (before, held, _, _) = miner_snapshot(&mut app, miner);
    assert_eq!(before.work + 1, terms.harvest_interval);
    assert!(before.load < terms.capacity);
    tick(&mut app, vec![]);
    assert_eq!(
        cell(&mut app, miner),
        held,
        "frozen worker must not harvest full then teleport out of beam"
    );
    frozen(&mut app, miner);
    release_beam(&mut app, caster, miner);
    let mut returned = false;
    for _ in 0..60 {
        tick(&mut app, vec![]);
        if cell(&mut app, miner) != held {
            returned = true;
            break;
        }
    }
    assert!(
        returned,
        "released Chrono Miner must perform its native return teleport"
    );
    cells(&mut app);
}

#[test]
fn ra9_erased_depot_host_cannot_charge_or_heal_patient_then_resumes() {
    let mut app = world();
    power(&mut app);
    let depot = put(&mut app, P0, "service_depot", 12, 12);
    let patient = put(&mut app, P0, "grizzly", 11, 12);
    disarm(&mut app, patient);
    set_hp(&mut app, patient, 50);
    let events = interact(&mut app, patient, depot);
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert_eq!(
        app.world().resource::<SpecialState>().docked.get(&patient),
        Some(&depot)
    );
    let caster = put(&mut app, P1, "chrono_legionnaire", 9, 14);
    start_beam(&mut app, caster, depot);
    let before = hp(&mut app, patient);
    let money = credits(&app);
    let interval = defs(&app)
        .rules()
        .unwrap()
        .economy
        .unit_repair_interval
        .unsigned_abs();
    for _ in 0..interval + 2 {
        tick(&mut app, vec![]);
        frozen(&mut app, depot);
        assert_eq!(
            hp(&mut app, patient),
            before,
            "active erased depot cannot heal"
        );
        assert_eq!(credits(&app), money, "erased depot cannot charge service");
        assert_eq!(
            app.world().resource::<SpecialState>().docked.get(&patient),
            Some(&depot)
        );
    }
    release_beam(&mut app, caster, depot);
    steps(&mut app, interval as usize + 1);
    assert!(
        hp(&mut app, patient) > before,
        "living released host must service"
    );
    assert!(credits(&app) < money);
    cells(&mut app);
}

#[test]
fn ra9_erased_strategic_source_preserves_charge_then_resumes() {
    let mut app = world();
    power(&mut app);
    let sphere = put(&mut app, P0, "chronosphere", 12, 12);
    let caster = put(&mut app, P1, "chrono_legionnaire", 9, 12);
    start_beam(&mut app, caster, sphere);
    let before = app.world().resource::<SpecialState>().charge[&sphere];
    assert!(before > 0);
    for _ in 0..25 {
        tick(&mut app, vec![]);
        frozen(&mut app, sphere);
        assert_eq!(
            app.world().resource::<SpecialState>().charge[&sphere],
            before,
            "frozen source cannot actively charge"
        );
    }
    release_beam(&mut app, caster, sphere);
    tick(&mut app, vec![]);
    assert!(app.world().resource::<SpecialState>().charge[&sphere] > before);
}

#[test]
fn ra9_erased_active_factory_cannot_complete_charge_or_spawn_then_resumes() {
    let mut app = world();
    put(&mut app, P1, "engineer", 9, 12); // real shared sight before timed beam boundary
    power(&mut app);
    put(&mut app, P0, "construction_yard", 2, 10);
    let factory = put(&mut app, P0, "barracks", 12, 12);
    let d = defs(&app);
    let item = ProductionItem::Unit(d.unit_kind("gi").unwrap());
    let key = d.queue_of(item).unwrap();
    let time = d.production(item).unwrap().build_time;
    let events = tick(&mut app, vec![issue(P0, Command::QueueProduction { item })]);
    assert!(rejected(&events).is_empty(), "{events:?}");
    for _ in 0..time {
        let head = &app.world().resource::<Players>().get(P0).unwrap().lane(key)[0];
        if head.progress + 3 >= time {
            break;
        }
        tick(&mut app, vec![]);
    }
    let caster = put(&mut app, P1, "chrono_legionnaire", 9, 12);
    start_beam(&mut app, caster, factory);
    let before = app.world().resource::<Players>().get(P0).unwrap().lane(key)[0];
    assert!(before.progress > 0 && before.progress < time);
    let money = credits(&app);
    for _ in 0..10 {
        let events = tick(&mut app, vec![]);
        frozen(&mut app, factory);
        assert_eq!(
            app.world()
                .resource::<Players>()
                .get(P0)
                .unwrap()
                .lane(key)
                .first(),
            Some(&before),
            "native active queue must pause before completion"
        );
        assert_eq!(credits(&app), money);
        assert!(
            !events.iter().any(
                |e| matches!(e, SimEvent::UnitSpawned { building, .. } if *building == factory)
            )
        );
    }
    release_beam(&mut app, caster, factory);
    let mut spawned = false;
    for _ in 0..10 {
        let events = tick(&mut app, vec![]);
        spawned |= events
            .iter()
            .any(|e| matches!(e, SimEvent::UnitSpawned { building, .. } if *building == factory));
    }
    assert!(
        spawned,
        "same live released factory must complete and spawn"
    );
    cells(&mut app);
}

#[test]
fn ra9_erased_already_returning_chrono_miner_cannot_teleport_to_newly_open_dock() {
    let mut app = world();
    put(&mut app, P1, "engineer", 9, 12); // real shared sight before timed beam boundary
    let field = CellPos::new(12, 12);
    ore(&mut app, field);
    let refinery = put(&mut app, P0, "refinery", 30, 30);
    let e = entity(&mut app, refinery).unwrap();
    let foot = app.world().get::<Building>(e).unwrap().footprint;
    let dock = CellPos::new(29, 30);
    assert_eq!(foot.distance_to_cell(dock), 1);
    // Exactly one passable ring cell, occupied by a genuine owned unit. The public
    // nearest-dock fallback produces a real ToRefinery job while teleport cannot claim it.
    for at in foot.ring() {
        if at != dock {
            app.world_mut().resource_mut::<Grid>().set_terrain(
                at,
                Cell {
                    passable: false,
                    water: false,
                    buildable: false,
                    ore: 0,
                },
            );
        }
    }
    let blocker = put(&mut app, P0, "chrono_legionnaire", dock.x, dock.y);
    disarm(&mut app, blocker);
    let miner = put(&mut app, P0, "chrono_miner", field.x, field.y);
    let d = defs(&app);
    let terms = d.harvester(d.unit_kind("chrono_miner").unwrap()).unwrap();
    let mut returning = false;
    for _ in 0..3000 {
        let e = entity(&mut app, miner).unwrap();
        let h = app.world().get::<Harvester>(e).unwrap();
        if matches!(h.state, HarvestState::ToRefinery { refinery: target, dock: actual } if target == refinery && actual == dock)
        {
            assert_eq!(
                h.load, terms.capacity,
                "load was acquired from actual native harvest"
            );
            returning = true;
            break;
        }
        tick(&mut app, vec![]);
    }
    assert!(
        returning,
        "blocked native dock must establish pre-existing return job"
    );
    assert_eq!(
        cell(&mut app, miner),
        field,
        "occupied dock prevented real teleport"
    );
    let caster = put(&mut app, P1, "chrono_legionnaire", 9, 12);
    start_beam(&mut app, caster, miner);
    let (before, held, money, _) = miner_snapshot(&mut app, miner);
    assert!(
        matches!(before.state, HarvestState::ToRefinery { refinery: target, dock: actual } if target == refinery && actual == dock)
    );
    assert_eq!(before.load, terms.capacity);
    assert!(!app.world().resource::<Grid>().room_for(dock, miner));
    // The blocker itself teleports through its ordinary public Move behavior. It frees
    // room later in Economy, after this tick's blocked Chrono Miner return attempt.
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Move {
                units: vec![blocker],
                target: CellPos::new(27, 30),
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert_ne!(
        cell(&mut app, blocker),
        dock,
        "public blocker movement actually frees dock"
    );
    assert!(app.world().resource::<Grid>().room_for(dock, miner));
    assert_eq!(cell(&mut app, miner), held);
    frozen(&mut app, miner);
    let events = tick(&mut app, vec![]);
    assert_eq!(
        cell(&mut app, miner),
        held,
        "an already-returning erased miner cannot teleport at a newly legal dock boundary"
    );
    frozen(&mut app, miner);
    let (now, _, paid, _) = miner_snapshot(&mut app, miner);
    assert_eq!(
        (now.load, now.gems, now.state, paid),
        (before.load, before.gems, before.state, money)
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::OreDelivered { harvester, .. } if *harvester == miner))
    );
    release_beam(&mut app, caster, miner);
    tick(&mut app, vec![]);
    assert_eq!(
        cell(&mut app, miner),
        dock,
        "same native return job teleports after release"
    );
    cells(&mut app);
}

#[test]
fn ra9_temporal_retained_passive_power_radar_prerequisite_and_external_repair_model() {
    // Bounded current model, not a recovered closed-engine claim: erasure suspends own
    // active work, while positive-HP owned buildings retain passive contributions and
    // can receive the owner's external RepairBuilding effect.
    let mut app = world();
    let power_source = put(&mut app, P0, "nuclear_reactor", 12, 12);
    put(&mut app, P0, "construction_yard", 3, 3);
    let radar = put(&mut app, P0, "airforce_command", 24, 12);
    set_hp(&mut app, radar, 500);
    tick(&mut app, vec![]);
    let baseline = app.world().resource::<Players>().get(P0).unwrap();
    let passive = (
        baseline.power_produced(),
        baseline.power_consumed(),
        baseline.radar(),
    );
    assert!(passive.0 > passive.1 && passive.2);
    let power_caster = put(&mut app, P1, "chrono_legionnaire", 9, 12);
    start_beam(&mut app, power_caster, power_source);
    let radar_caster = put(&mut app, P1, "chrono_legionnaire", 21, 12);
    start_beam(&mut app, radar_caster, radar);
    let before = hp(&mut app, radar);
    let money = credits(&app);
    let events = tick(
        &mut app,
        vec![issue(P0, Command::RepairBuilding { building: radar })],
    );
    assert!(
        rejected(&events).is_empty(),
        "external owner repair remains accepted: {events:?}"
    );
    assert!(
        app.world()
            .resource::<Players>()
            .get(P0)
            .unwrap()
            .repairing()
            .contains(&radar)
    );
    let interval = defs(&app)
        .rules()
        .unwrap()
        .economy
        .repair_interval
        .unsigned_abs();
    for _ in 0..interval + 2 {
        tick(&mut app, vec![]);
        frozen(&mut app, power_source);
        frozen(&mut app, radar);
        let state = app.world().resource::<Players>().get(P0).unwrap();
        assert_eq!(
            (
                state.power_produced(),
                state.power_consumed(),
                state.radar()
            ),
            passive,
            "retained passive contributions while both actual beams remain active"
        );
    }
    assert!(
        hp(&mut app, radar) > before,
        "native external repair restores HP under erasure in this bounded model"
    );
    assert!(
        credits(&app) < money,
        "native external repair charges its normal positive cost"
    );
    let events = tick(
        &mut app,
        vec![issue(P0, Command::RepairBuilding { building: radar })],
    );
    assert!(rejected(&events).is_empty());
    assert!(
        !app.world()
            .resource::<Players>()
            .get(P0)
            .unwrap()
            .repairing()
            .contains(&radar)
    );
    // An erased but living Airforce Command still satisfies its prerequisite/dock census
    // for accepting this order; its own active production is tested separately to pause.
    let d = defs(&app);
    let item = ProductionItem::Unit(d.unit_kind("harrier").unwrap());
    let airforce = d.building_kind("airforce_command").unwrap();
    assert!(
        d.metadata(item)
            .unwrap()
            .prerequisite_groups
            .iter()
            .any(|group| d
                .rules()
                .unwrap()
                .group(*group)
                .unwrap()
                .buildings
                .contains(&airforce)),
        "loaded RADAR group actually requires this frozen building"
    );
    let events = tick(&mut app, vec![issue(P0, Command::QueueProduction { item })]);
    assert!(
        rejected(&events).is_empty(),
        "retained frozen prerequisite must admit native order: {events:?}"
    );
    let key = d.queue_of(item).unwrap();
    assert!(
        app.world()
            .resource::<Players>()
            .get(P0)
            .unwrap()
            .lane(key)
            .iter()
            .any(|head| head.item == item)
    );
    frozen(&mut app, power_source);
    frozen(&mut app, radar);
}

#[test]
fn ra9_erased_refinery_host_preserves_live_loaded_waiter_until_release() {
    let mut app = world();
    put(&mut app, P1, "engineer", 12, 15); // native shared sight before timed channel
    let at = CellPos::new(13, 12);
    ore(&mut app, at);
    let refinery = put(&mut app, P0, "soviet_refinery", 14, 12);
    let miner = put(&mut app, P0, "war_miner", 13, 12);
    disarm(&mut app, miner);
    let d = defs(&app);
    let terms = d.harvester(d.unit_kind("war_miner").unwrap()).unwrap();
    let mut precursor = false;
    for _ in 0..3000 {
        let e = entity(&mut app, miner).unwrap();
        let h = app.world().get::<Harvester>(e).unwrap();
        if h.load + terms.harvest_rate >= terms.capacity && h.work + 2 == terms.harvest_interval {
            precursor = true;
            break;
        }
        tick(&mut app, vec![]);
    }
    assert!(precursor, "actual mined load reaches final-bail precursor");
    let caster = put(&mut app, P1, "chrono_legionnaire", 10, 14);
    start_beam(&mut app, caster, refinery);
    let mut waiting = false;
    for _ in 0..3 {
        let (h, held, _, _) = miner_snapshot(&mut app, miner);
        if h.load == terms.capacity
            && matches!(h.state,
            HarvestState::ToRefinery { refinery: target, dock } if target == refinery && dock == held)
        {
            waiting = true;
            break;
        }
        tick(&mut app, vec![]);
        frozen(&mut app, refinery);
    }
    assert!(
        waiting,
        "real full living worker must reach its adjacent native unloading job"
    );
    assert!(
        !app.world()
            .resource::<SpecialState>()
            .erasures
            .contains_key(&miner),
        "only refinery host is frozen"
    );
    let (before, held, money, _) = miner_snapshot(&mut app, miner);
    let e = entity(&mut app, miner).unwrap();
    assert!(app.world().get::<Orders>(e).unwrap().current.is_none());
    frozen(&mut app, refinery);
    let events = tick(&mut app, vec![]);
    frozen(&mut app, refinery);
    let (after, at, paid, _) = miner_snapshot(&mut app, miner);
    assert_eq!(
        (
            after.load,
            after.gems,
            after.work,
            after.attempts,
            after.state,
            at,
            paid
        ),
        (
            before.load,
            before.gems,
            before.work,
            before.attempts,
            before.state,
            held,
            money
        ),
        "an erased refinery cannot actively accept/pay; keep the existing waiting job rather than retarget it"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::OreDelivered { harvester, .. } if *harvester == miner))
    );
    release_beam(&mut app, caster, refinery);
    let events = tick(&mut app, vec![]);
    let refinery_terms = d
        .building(d.building_kind("soviet_refinery").unwrap())
        .unwrap();
    let expected = i64::from(before.load - before.gems) * refinery_terms.credits_per_ore.unwrap()
        + i64::from(before.gems)
            * refinery_terms
                .credits_per_gem
                .unwrap_or(refinery_terms.credits_per_ore.unwrap());
    assert!(
        events.iter().any(
            |e| matches!(e, SimEvent::OreDelivered { harvester, refinery: host, credits, .. }
        if *harvester == miner && *host == refinery && *credits == expected)
        ),
        "same native job must pay exactly after host release: {events:?}"
    );
    assert_eq!(credits(&app), money + expected);
    cells(&mut app);
}
