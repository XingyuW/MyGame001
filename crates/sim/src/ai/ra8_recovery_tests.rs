//! Finite-credit recovery uses ordinary production, cancellation, capture and placement.
use super::ra8_attrition_tests::{app, app_with_power, building, issue, step, unit};
use super::*;
use bevy_app::App;

fn starved(mode: &str) -> (App, AiPlayers, ProductionItem, ProductionItem, i64) {
    let mut app = app();
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<AiPlayers>());
    building(&mut app, 0, "barracks", CellPos::new(2, 14));
    building(&mut app, 0, "war_factory", CellPos::new(5, 14));
    // Captured radar satisfies the normal shared RADAR prerequisite without
    // introducing an unrelated paid aircraft lane into this tank probe.
    building(&mut app, 0, "radar_tower", CellPos::new(12, 4));
    unit(&mut app, 0, "grizzly", CellPos::new(14, 20));
    // Declared finite scenario budget. Every charged/refunded credit below then
    // changes through the normal source production/capture/command path.
    app.world_mut()
        .resource_mut::<Players>()
        .get_mut(PlayerId(0))
        .unwrap()
        .credits = if mode == "complete" { 2000 } else { 1600 };
    step(&mut app);
    let defs = app.world().resource::<Definitions>();
    let construction = ProductionItem::Building(
        defs.building_kind(if mode == "refinery" {
            "refinery"
        } else {
            "tech_center"
        })
        .unwrap(),
    );
    let tank = ProductionItem::Unit(defs.unit_kind("grizzly").unwrap());
    let duration = defs.production(construction).unwrap().build_time;
    issue(&mut app, 0, Command::QueueProduction { item: construction });
    let mut complete = false;
    for _ in 0..3000 {
        for event in step(&mut app) {
            complete |= matches!(event, crate::SimEvent::ProductionComplete { player: PlayerId(0), item } if item == construction);
        }
        if app
            .world()
            .resource::<Players>()
            .get(PlayerId(0))
            .unwrap()
            .credits()
            == 0
        {
            break;
        }
    }
    let head = *app
        .world()
        .resource::<Players>()
        .get(PlayerId(0))
        .unwrap()
        .queues()
        .flat_map(|(_, q)| q.iter())
        .find(|q| q.item == construction)
        .unwrap();
    assert!(head.charged > 0);
    assert_eq!(complete, mode == "complete");
    assert_eq!(head.progress >= duration, mode == "complete");
    issue(&mut app, 0, Command::QueueProduction { item: tank });
    for _ in 0..20 {
        step(&mut app);
    }
    let state = app.world().resource::<Players>().get(PlayerId(0)).unwrap();
    let blocked = state
        .queues()
        .flat_map(|(_, q)| q.iter())
        .find(|q| q.item == tank)
        .unwrap();
    assert_eq!(
        blocked.charged, 0,
        "normal combat production must really be starved"
    );
    assert_eq!(state.credits(), 0);
    if mode == "income" {
        let oil = building(
            &mut app,
            crate::NEUTRAL.0,
            "oil_derrick",
            CellPos::new(20, 28),
        );
        let engineer = unit(&mut app, 0, "engineer", CellPos::new(19, 28));
        step(&mut app);
        issue(
            &mut app,
            0,
            Command::Interact {
                unit: engineer,
                target: oil,
            },
        );
        for _ in 0..20 {
            step(&mut app);
        }
        assert!(
            app.world()
                .resource::<Players>()
                .get(PlayerId(0))
                .unwrap()
                .credits()
                > 0,
            "ordinary captured oil must actually fund recovery"
        );
    }
    (app, controllers, construction, tank, head.charged)
}

#[test]
fn ra8_no_income_reallocates_real_unfinished_tech_credits_to_starved_combat_without_requeue() {
    let (mut app, controllers, construction, tank, charged) = starved("starved");
    app.world_mut().insert_resource(controllers);
    let mut refunds = Vec::new();
    let mut paid_tank = false;
    let mut requeued = false;
    for _ in 0..2000 {
        for event in step(&mut app) {
            if let crate::SimEvent::ProductionCancelled {
                player: PlayerId(0),
                item,
                refund,
            } = event
                && item == construction
            {
                refunds.push(refund);
            }
            paid_tank |= matches!(event, crate::SimEvent::ProductionComplete { player: PlayerId(0), item } if item == tank);
        }
        requeued |= !refunds.is_empty() && app.world().resource::<CommandQueue>().iter().any(|issued|
            issued.player == PlayerId(0) && matches!(issued.command, Command::QueueProduction { item } if item == construction));
    }
    eprintln!(
        "finite construction charged{charged} refunds{refunds:?} paid_tank{paid_tank} requeued{requeued}"
    );
    eprintln!(
        "finite recovery final credits{} queues{:?}",
        app.world()
            .resource::<Players>()
            .get(PlayerId(0))
            .unwrap()
            .credits(),
        app.world()
            .resource::<Players>()
            .get(PlayerId(0))
            .unwrap()
            .queues()
            .collect::<Vec<_>>()
    );
    assert_eq!(
        refunds,
        vec![charged],
        "exact paid construction funds must be refunded once"
    );
    assert!(paid_tank, "ordinary paid combat production stayed starved");
    assert!(
        !requeued,
        "the same unaffordable construction reclaimed its refund"
    );
    assert!(
        app.world()
            .resource::<crate::MatchState>()
            .outcome()
            .is_none()
    );
}

#[test]
fn ra8_income_recovery_and_completed_construction_keep_their_normal_heads() {
    for mode in ["income", "refinery", "complete"] {
        let (mut app, controllers, construction, _, _) = starved(mode);
        app.world_mut().insert_resource(controllers);
        let mut placed = false;
        for _ in 0..200 {
            for event in step(&mut app) {
                assert!(
                    !matches!(event, crate::SimEvent::ProductionCancelled { player: PlayerId(0), item, .. } if item == construction),
                    "protected {mode} head was cancelled"
                );
                if let ProductionItem::Building(kind) = construction {
                    placed |= matches!(event, crate::SimEvent::BuildingPlaced { player: PlayerId(0), kind: found, .. } if kind == found);
                }
            }
        }
        if mode == "complete" {
            assert!(placed, "completed paid head must take normal placement");
        }
    }
}

#[test]
fn ra8_attainable_power_recovery_restores_real_production_despite_reserved_unit_tails() {
    let mut app = app_with_power(false);
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<AiPlayers>());
    building(&mut app, 0, "barracks", CellPos::new(2, 14));
    building(&mut app, 0, "war_factory", CellPos::new(5, 14));
    building(&mut app, 0, "radar_tower", CellPos::new(12, 4));
    unit(&mut app, 0, "grizzly", CellPos::new(14, 20));
    app.world_mut()
        .resource_mut::<Players>()
        .get_mut(PlayerId(0))
        .unwrap()
        .credits = 2500;
    step(&mut app);
    assert!(
        app.world()
            .resource::<Players>()
            .get(PlayerId(0))
            .unwrap()
            .low_power()
    );
    let defs = app.world().resource::<Definitions>();
    let tank = ProductionItem::Unit(defs.unit_kind("grizzly").unwrap());
    let infantry = ProductionItem::Unit(defs.unit_kind("gi").unwrap());
    let power = defs.building_kind("power_plant").unwrap();
    for item in [tank, tank, infantry, infantry] {
        issue(&mut app, 0, Command::QueueProduction { item });
    }
    step(&mut app);
    let progress = |app: &App| {
        app.world()
            .resource::<Players>()
            .get(PlayerId(0))
            .unwrap()
            .queues()
            .flat_map(|(_, q)| q.iter())
            .find(|q| q.item == tank)
            .unwrap()
            .progress
    };
    let before = progress(&app);
    for _ in 0..8 {
        step(&mut app);
    }
    let slow = progress(&app) - before;
    assert!(
        slow > 0,
        "source low-power production slows rather than absolutely stops"
    );
    app.world_mut().insert_resource(controllers);
    let mut placed = false;
    let mut fast_from = None;
    let mut fast = None;
    for _ in 0..3000 {
        for event in step(&mut app) {
            placed |= matches!(event, crate::SimEvent::BuildingPlaced { player: PlayerId(0), kind, .. } if kind == power);
        }
        if placed && fast_from.is_none() {
            assert!(
                !app.world()
                    .resource::<Players>()
                    .get(PlayerId(0))
                    .unwrap()
                    .low_power()
            );
            fast_from = Some((
                app.world().resource::<SimTick>().0,
                progress(&app),
                app.world()
                    .resource::<Players>()
                    .get(PlayerId(0))
                    .unwrap()
                    .credits(),
            ));
        }
        if let Some((tick, from, cash)) = fast_from
            && fast.is_none()
            && app.world().resource::<SimTick>().0 >= tick + 8
        {
            fast = Some((
                progress(&app).saturating_sub(from),
                cash - app
                    .world()
                    .resource::<Players>()
                    .get(PlayerId(0))
                    .unwrap()
                    .credits(),
            ));
        }
        if fast.is_some() {
            break;
        }
    }
    eprintln!(
        "power recovery placed{placed} slow8ticks{slow} fast8ticks_progress_and_paidcredits{fast:?}"
    );
    assert!(
        placed,
        "finite-cost power recovery was reserved out by unaffordable unit tails"
    );
    assert!(fast.is_some_and(|(fast, paid)| fast > slow && paid > 0));
}

struct UnitDebtFixture {
    app: App,
    controllers: AiPlayers,
    barracks: SimId,
    expensive: [ProductionItem; 2],
    cheaper: ProductionItem,
    heads: Vec<crate::QueueItem>,
}

fn unit_debt_fixture(mode: &str) -> UnitDebtFixture {
    let mut app = app();
    let mut controllers = std::mem::take(&mut *app.world_mut().resource_mut::<AiPlayers>());
    if mode == "detector" {
        // Declared remembered unseen-loss obligation; no living detector exists.
        controllers
            .controllers
            .get_mut(&PlayerId(0))
            .unwrap()
            .unseen_loss = true;
    }
    let barracks = building(&mut app, 0, "barracks", CellPos::new(2, 14));
    building(&mut app, 0, "war_factory", CellPos::new(5, 14));
    building(&mut app, 0, "radar_tower", CellPos::new(12, 4));
    building(&mut app, 0, "tech_center", CellPos::new(2, 20));
    if mode == "harvester" {
        building(&mut app, 0, "refinery", CellPos::new(18, 4));
    }
    if mode == "isolated" {
        for y in 0..48 {
            assert!(app.world_mut().resource_mut::<Grid>().set_terrain(
                CellPos::new(24, y),
                crate::Cell {
                    passable: false,
                    buildable: false,
                    water: false,
                    ore: 0
                }
            ));
        }
    }
    unit(&mut app, 0, "gi", CellPos::new(14, 20));
    app.world_mut()
        .resource_mut::<Players>()
        .get_mut(PlayerId(0))
        .unwrap()
        .credits = if mode == "investment" { 1600 } else { 350 };
    step(&mut app);
    let defs = app.world().resource::<Definitions>();
    let expensive = [
        if mode == "detector" {
            "attack_dog"
        } else {
            "tanya"
        },
        if mode == "harvester" {
            "chrono_miner"
        } else {
            "tank_destroyer"
        },
    ]
    .map(|name| ProductionItem::Unit(defs.unit_kind(name).unwrap()));
    let cheaper = ProductionItem::Unit(defs.unit_kind("gi").unwrap());
    for item in expensive {
        issue(&mut app, 0, Command::QueueProduction { item });
    }
    for _ in 0..4000 {
        step(&mut app);
        if app
            .world()
            .resource::<Players>()
            .get(PlayerId(0))
            .unwrap()
            .credits()
            == 0
        {
            break;
        }
    }
    let heads: Vec<_> = app
        .world()
        .resource::<Players>()
        .get(PlayerId(0))
        .unwrap()
        .queues()
        .flat_map(|(_, q)| q.iter())
        .copied()
        .collect();
    let charged = heads.iter().map(|h| h.charged).sum::<i64>();
    let cash = app
        .world()
        .resource::<Players>()
        .get(PlayerId(0))
        .unwrap()
        .credits();
    assert_eq!(
        charged.saturating_add(cash),
        if mode == "investment" { 1600 } else { 350 }
    );
    assert!(
        cash < heads
            .iter()
            .map(|h| app
                .world()
                .resource::<Definitions>()
                .production(h.item)
                .unwrap()
                .cost
                - h.charged)
            .min()
            .unwrap()
    );
    assert!(heads.iter().all(|head| {
        head.charged > 0
            && head.progress
                < app
                    .world()
                    .resource::<Definitions>()
                    .production(head.item)
                    .unwrap()
                    .build_time
    }));
    // Public cancellation removes the last matching object. These real unpaid
    // duplicates must be cancelled before the paid head, without inventing refunds.
    for item in [expensive[1], expensive[1], cheaper] {
        issue(&mut app, 0, Command::QueueProduction { item });
    }
    for _ in 0..20 {
        step(&mut app);
    }
    if mode == "income" {
        let oil = building(
            &mut app,
            crate::NEUTRAL.0,
            "oil_derrick",
            CellPos::new(20, 28),
        );
        let engineer = unit(&mut app, 0, "engineer", CellPos::new(19, 28));
        step(&mut app);
        issue(
            &mut app,
            0,
            Command::Interact {
                unit: engineer,
                target: oil,
            },
        );
        for _ in 0..20 {
            step(&mut app);
        }
        assert!(
            app.world()
                .resource::<Players>()
                .get(PlayerId(0))
                .unwrap()
                .credits()
                > 0
        );
    }
    UnitDebtFixture {
        app,
        controllers,
        barracks,
        expensive,
        cheaper,
        heads,
    }
}

#[test]
fn ra8_unfundable_duplicate_unit_heads_refund_exactly_and_produce_a_real_cheaper_finisher() {
    let UnitDebtFixture {
        mut app,
        controllers,
        barracks,
        expensive,
        cheaper,
        heads,
    } = unit_debt_fixture("plain");
    app.world_mut().insert_resource(controllers);
    let mut refunds = Vec::new();
    let mut recruits = BTreeSet::new();
    let mut shots = 0;
    let mut requeued = false;
    for _ in 0..5000 {
        for event in step(&mut app) {
            if let crate::SimEvent::ProductionCancelled {
                player: PlayerId(0),
                item,
                refund,
            } = event
            {
                refunds.push((item, refund));
            }
            if let crate::SimEvent::UnitSpawned {
                player: PlayerId(0),
                unit,
                building,
            } = event
                && building == barracks
            {
                let world = app.world_mut();
                let recruited = world
                    .query::<(&SimId, &crate::Unit)>()
                    .iter(world)
                    .any(|(id, u)| *id == unit && ProductionItem::Unit(u.kind) == cheaper);
                if recruited {
                    recruits.insert(unit);
                }
            }
            if let crate::SimEvent::WeaponFired {
                attacker, target, ..
            } = event
                && recruits.contains(&attacker)
            {
                let world = app.world_mut();
                shots += usize::from(
                    world
                        .query::<(&SimId, &crate::Building)>()
                        .iter(world)
                        .any(|(id, _)| *id == target),
                );
            }
        }
        requeued |= !refunds.is_empty() && app.world().resource::<CommandQueue>().iter().any(|issued|
            matches!(issued.command, Command::QueueProduction { item } if expensive.contains(&item)));
        if shots > 0 {
            break;
        }
    }
    eprintln!("unit recovery refunds{refunds:?} recruits{recruits:?} actual_building_shots{shots}");
    assert_eq!(refunds.iter().map(|(_, amount)| *amount).sum::<i64>(), 350);
    assert_eq!(
        refunds,
        vec![
            (cheaper, 0),
            (expensive[1], 0),
            (expensive[1], 0),
            (
                expensive[0],
                heads
                    .iter()
                    .find(|h| h.item == expensive[0])
                    .unwrap()
                    .charged
            ),
            (
                expensive[1],
                heads
                    .iter()
                    .find(|h| h.item == expensive[1])
                    .unwrap()
                    .charged
            )
        ]
    );
    assert!(!requeued, "the same unfundable unit reacquired its refund");
    assert!(
        refunds
            .iter()
            .filter(|(item, _)| *item == expensive[1])
            .map(|(_, amount)| *amount)
            .take(2)
            .all(|amount| amount == 0),
        "last-matching unpaid tails must precede the charged duplicate head"
    );
    assert!(
        !recruits.is_empty() && shots > 0,
        "fully refunded credits never produced a real cheaper finishing shot"
    );
}

#[test]
fn ra8_unit_reallocation_preserves_actual_income_harvesters_and_unreachable_alternatives() {
    for mode in ["income", "harvester", "isolated", "detector"] {
        let UnitDebtFixture {
            mut app,
            controllers,
            expensive,
            ..
        } = unit_debt_fixture(mode);
        app.world_mut().insert_resource(controllers);
        for _ in 0..200 {
            for event in step(&mut app) {
                assert!(
                    !matches!(event, crate::SimEvent::ProductionCancelled { player: PlayerId(0), item, .. } if expensive.contains(&item)),
                    "{mode} recovery was mistaken for disposable unit debt: {event:?}"
                );
            }
        }
    }
}

#[test]
fn ra8_completed_paid_unit_heads_and_duplicate_tails_keep_normal_exit_and_fifo() {
    let mut app = app();
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<AiPlayers>());
    building(&mut app, 0, "barracks", CellPos::new(2, 14));
    let factory = building(&mut app, 0, "war_factory", CellPos::new(5, 14));
    building(&mut app, 0, "radar_tower", CellPos::new(12, 4));
    building(&mut app, 0, "tech_center", CellPos::new(2, 20));
    let world = app.world_mut();
    let footprint = world
        .query::<(&SimId, &crate::Building)>()
        .iter(world)
        .find(|(id, _)| **id == factory)
        .unwrap()
        .1
        .footprint;
    // Land production searches beyond occupied ring cells. A declared water
    // moat leaves one actual land exit, held by an amphibious service vehicle
    // that can later leave through an ordinary public Move.
    let gate = CellPos::new(footprint.origin.x - 1, footprint.origin.y);
    for cell in footprint.ring() {
        if cell != gate {
            assert!(app.world_mut().resource_mut::<Grid>().set_terrain(
                cell,
                crate::Cell {
                    passable: false,
                    buildable: false,
                    water: true,
                    ore: 0,
                }
            ));
        }
    }
    for dy in -1..=1 {
        let cell = CellPos::new(gate.x - 1, gate.y + dy);
        assert!(app.world_mut().resource_mut::<Grid>().set_terrain(
            cell,
            crate::Cell {
                passable: false,
                buildable: false,
                water: true,
                ore: 0,
            }
        ));
    }
    let blocker = unit(&mut app, 0, "amphibious_transport", gate);
    let defs = app.world().resource::<Definitions>();
    let item = ProductionItem::Unit(defs.unit_kind("tank_destroyer").unwrap());
    let cost = defs.production(item).unwrap().cost;
    let duration = defs.production(item).unwrap().build_time;
    app.world_mut()
        .resource_mut::<Players>()
        .get_mut(PlayerId(0))
        .unwrap()
        .credits = cost;
    step(&mut app);
    issue(&mut app, 0, Command::QueueProduction { item });
    let mut completed = false;
    for _ in 0..3000 {
        for event in step(&mut app) {
            completed |= matches!(event, crate::SimEvent::ProductionComplete { player: PlayerId(0), item: found } if found == item);
        }
        if completed {
            break;
        }
    }
    assert!(completed);
    let state = app.world().resource::<Players>().get(PlayerId(0)).unwrap();
    let head = state
        .queues()
        .flat_map(|(_, q)| q.iter())
        .find(|q| q.item == item)
        .unwrap();
    assert_eq!((head.charged, head.progress), (cost, duration));
    issue(&mut app, 0, Command::QueueProduction { item });
    step(&mut app);
    app.world_mut().insert_resource(controllers);
    for _ in 0..20 {
        for event in step(&mut app) {
            assert!(
                !matches!(event, crate::SimEvent::ProductionCancelled { player: PlayerId(0), item: found, .. } if found == item)
            );
        }
    }
    issue(
        &mut app,
        0,
        Command::Move {
            units: vec![blocker],
            target: CellPos::new(gate.x - 1, gate.y),
        },
    );
    let mut exited = false;
    for _ in 0..200 {
        for event in step(&mut app) {
            assert!(
                !matches!(event, crate::SimEvent::ProductionCancelled { player: PlayerId(0), item: found, .. } if found == item)
            );
            exited |= matches!(event, crate::SimEvent::UnitSpawned { player: PlayerId(0), building, .. } if building == factory);
        }
    }
    assert!(
        exited,
        "normal movement must free the fully paid head's actual exit"
    );
}

#[test]
fn ra8_finite_funds_preserve_paid_head_and_finance_source_cost_finisher_after_unpaid_tails() {
    let UnitDebtFixture {
        mut app,
        controllers,
        expensive,
        heads,
        ..
    } = unit_debt_fixture("investment");
    let grizzly = app
        .world()
        .resource::<Definitions>()
        .unit_kind("grizzly")
        .unwrap();
    app.world_mut().insert_resource(controllers);
    let mut refunds = Vec::new();
    let mut recruited = BTreeSet::new();
    let mut shots = 0;
    for _ in 0..5000 {
        for event in step(&mut app) {
            match event {
                crate::SimEvent::ProductionCancelled {
                    player: PlayerId(0),
                    item,
                    refund,
                } => refunds.push((item, refund)),
                crate::SimEvent::UnitSpawned {
                    player: PlayerId(0),
                    unit: id,
                    ..
                } => {
                    let world = app.world_mut();
                    if world
                        .query::<(&SimId, &crate::Unit)>()
                        .iter(world)
                        .any(|(found, u)| *found == id && u.kind == grizzly)
                    {
                        recruited.insert(id);
                    }
                }
                crate::SimEvent::WeaponFired {
                    attacker, target, ..
                } if recruited.contains(&attacker) => {
                    let world = app.world_mut();
                    if world
                        .query::<(&SimId, &crate::Building)>()
                        .iter(world)
                        .any(|(id, _)| *id == target)
                    {
                        shots += 1;
                    }
                }
                _ => {}
            }
        }
        if shots > 0 {
            break;
        }
    }
    eprintln!("aggregate recovery refunds{refunds:?} grizzlies{recruited:?} building_shots{shots}");
    assert!(
        refunds
            .iter()
            .filter(|(item, _)| *item == expensive[1])
            .all(|(_, refund)| *refund == 0),
        "paid near-complete armor head was discarded"
    );
    assert_eq!(
        refunds.iter().map(|(_, refund)| *refund).sum::<i64>(),
        heads
            .iter()
            .find(|h| h.item == expensive[0])
            .unwrap()
            .charged
    );
    assert_eq!(
        refunds
            .iter()
            .filter(|(item, _)| *item == expensive[1])
            .count(),
        2
    );
    assert!(
        !recruited.is_empty() && shots > 0,
        "preserved head plus source-cost finisher never reached a real building shot"
    );
}

#[test]
fn ra8_existing_main_building_obligation_cannot_be_reused_to_fund_the_defence_lane() {
    let mut app = app();
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<AiPlayers>());
    let barracks = building(&mut app, 0, "barracks", CellPos::new(2, 14));
    building(&mut app, 0, "war_factory", CellPos::new(5, 14));
    building(&mut app, 0, "radar_tower", CellPos::new(12, 4));
    unit(&mut app, 0, "grizzly", CellPos::new(14, 20));
    let defs = app.world().resource::<Definitions>();
    let main = ProductionItem::Building(defs.building_kind("tech_center").unwrap());
    let infantry = ProductionItem::Unit(defs.unit_kind("gi").unwrap());
    let defence = ProductionItem::Building(defs.building_kind("pillbox").unwrap());
    let main_cost = defs.production(main).unwrap().cost;
    let cash = main_cost
        + defs.production(infantry).unwrap().cost
        + defs.production(defence).unwrap().cost
        - 1;
    app.world_mut()
        .resource_mut::<Players>()
        .get_mut(PlayerId(0))
        .unwrap()
        .credits = cash;
    step(&mut app);
    issue(&mut app, 0, Command::QueueProduction { item: main });
    for _ in 0..3000 {
        step(&mut app);
        if app
            .world()
            .resource::<Players>()
            .get(PlayerId(0))
            .unwrap()
            .queues()
            .flat_map(|(_, q)| q.iter())
            .any(|q| q.item == main && q.charged >= main_cost / 3)
        {
            break;
        }
    }
    issue(&mut app, 0, Command::QueueProduction { item: infantry });
    step(&mut app);
    let state = app.world().resource::<Players>().get(PlayerId(0)).unwrap();
    assert!(
        state
            .queues()
            .flat_map(|(_, q)| q.iter())
            .any(|q| q.item == main && q.charged > 0 && q.charged < main_cost)
    );
    assert!(
        state
            .queues()
            .flat_map(|(_, q)| q.iter())
            .any(|q| q.item == infantry)
    );
    app.world_mut().insert_resource(controllers);
    let mut placed = false;
    let mut paid_infantry = false;
    for _ in 0..5000 {
        for event in step(&mut app) {
            assert!(
                !matches!(event, crate::SimEvent::ProductionCancelled { player: PlayerId(0), item, .. } if item == main),
                "attainable existing construction was cancelled instead of funded"
            );
            placed |= matches!(event, crate::SimEvent::BuildingPlaced { player: PlayerId(0), kind, .. } if ProductionItem::Building(kind) == main);
            paid_infantry |= matches!(event, crate::SimEvent::UnitSpawned { player: PlayerId(0), building, .. } if building == barracks);
        }
        let state = app.world().resource::<Players>().get(PlayerId(0)).unwrap();
        let defs = app.world().resource::<Definitions>();
        let existing = state
            .queues()
            .flat_map(|(_, q)| q.iter())
            .map(|q| defs.production(q.item).unwrap().cost - q.charged)
            .sum::<i64>();
        let pending: Vec<_> = app
            .world()
            .resource::<CommandQueue>()
            .iter()
            .filter(|p| p.player == PlayerId(0))
            .filter_map(|p| match p.command {
                Command::QueueProduction { item } => Some(item),
                _ => None,
            })
            .collect();
        let added = pending
            .iter()
            .map(|item| defs.production(*item).unwrap().cost)
            .sum::<i64>();
        assert!(
            existing.saturating_add(added) <= state.credits(),
            "finite existing{existing} + new{added} {pending:?} exceeds cash{}",
            state.credits()
        );
        if placed && paid_infantry {
            break;
        }
    }
    assert!(
        placed && paid_infantry,
        "existing main construction and ordinary paid infantry did not finish"
    );
}
