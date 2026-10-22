//! Historical preparation note: UNVALIDATED / PENDING. Native immediate-command fixture, never injected zero health.
mod ra4_common;
use ra4_common::*;
use sim::{
    Cell, CellPos, Command, Grid, HarvestState, Harvester, Mobile, Orders, Players, SimEvent,
    SimId, SpecialState,
};

// Independent fixture equation: the command blast reaches each covered building cell.
// Keep the loaded dimensions and damage terms explicit; do not call runtime impact_damage.
fn footprint_strike_damage(d: &sim::Definitions, name: &str) -> i32 {
    let k = d.building_kind(name).expect("fixture building");
    let b = d.building(k).expect("loaded footprint");
    let terms = d.building_combat(k).expect("loaded armour");
    let e = d.effect(sim::Effect::Nuke).expect("fixture strike");
    let wh = d.warhead(e.warhead).expect("fixture warhead");
    let versus = wh.versus[usize::from(terms.armour.0)];
    assert_eq!(
        (
            b.width,
            b.height,
            e.damage,
            wh.cell_spread,
            wh.percent_at_max,
            versus
        ),
        (2, 2, 300, 1024, 10, 10)
    );
    let mut total = 0_i64;
    for y in 0..u64::from(b.height) {
        for x in 0..u64::from(b.width) {
            let distance = ((x * 256).pow(2) + (y * 256).pow(2)).isqrt();
            if distance <= u64::from(wh.cell_spread) {
                total += (i64::from(e.damage)
                    * (100 * i64::from(wh.cell_spread)
                        - i64::from(100 - wh.percent_at_max) * i64::try_from(distance).unwrap())
                    * i64::from(versus)
                    / (100 * i64::from(wh.cell_spread) * 100))
                    .max(1);
            }
        }
    }
    assert_eq!(total, 96, "30+23+23+20 across the actual 2x2 footprint");
    i32::try_from(total).unwrap()
}

fn entity(app: &mut bevy_app::App, id: SimId) -> Option<bevy_ecs::entity::Entity> {
    let world = app.world_mut();
    world
        .query::<(bevy_ecs::entity::Entity, &SimId)>()
        .iter(world)
        .find(|(_, found)| **found == id)
        .map(|(entity, _)| entity)
}
fn armed_silo(app: &mut bevy_app::App) -> SimId {
    place(app, P1, "soviet_power", 32, 25);
    let silo = place(app, P1, "silo", 32, 30);
    // Unarmed Engineer supplies ordinary native current visibility, not an injected Vision.
    spawn(app, P1, "engineer", 24, 20);
    silo
}
fn strike(silo: SimId) -> sim::IssuedCommand {
    issue(
        P1,
        Command::Superweapon {
            building: silo,
            target: CellPos::new(20, 20),
            units: vec![],
        },
    )
}
fn ready_deposit() -> (bevy_app::App, SimId, SimId, SimId) {
    let mut app = world();
    place(&mut app, P0, "allied_yard", 1, 1);
    place(&mut app, P0, "allied_power", 5, 1);
    app.world_mut().resource_mut::<Grid>().set_terrain(
        CellPos::new(16, 20),
        Cell {
            passable: true,
            water: false,
            buildable: true,
            ore: 1000,
        },
    );
    let refinery = place(&mut app, P0, "allied_refinery", 20, 20);
    // Public spawn_building is controlled setup; free units belong to paid PlaceBuilding.
    // Spawn the exact loaded miner kind as controlled setup, then harvest/return natively.
    let miner = spawn(&mut app, P0, "miner_a", 19, 20);
    let d = defs(&app);
    assert_eq!(
        d.building(d.building_kind("allied_refinery").unwrap())
            .unwrap()
            .free_units,
        vec![d.unit_kind("miner_a").unwrap()]
    );
    assert!(entity(&mut app, miner).is_some());
    let silo = armed_silo(&mut app);
    ticks(&mut app, 25);
    for _ in 0..3000 {
        let world = app.world_mut();
        let found = world
            .query::<(&SimId, &Harvester, &Mobile, &Orders)>()
            .iter(world)
            .find_map(|(id, h, m, o)| match h.state {
                HarvestState::ToRefinery {
                    refinery: target,
                    dock,
                } if target == refinery
                    && h.load > 0
                    && m.cell == dock
                    && m.step.is_none()
                    && o.current.is_none() =>
                {
                    Some(*id)
                }
                _ => None,
            });
        if let Some(worker) = found {
            return (app, refinery, silo, worker);
        }
        tick(&mut app, vec![]);
    }
    panic!(
        "controlled miner never reached actual dock after Movement and before next Economy unload"
    );
}

#[test]
fn ra9_command_destroyed_refinery_cannot_pay_a_delivery_later_in_economy() {
    let (mut actual, refinery, silo, worker) = ready_deposit();
    let (mut control, _, _, control_worker) = ready_deposit();
    assert_eq!(worker, control_worker);
    set_hp(&mut actual, refinery, 1); // positive old injury; native command must kill it
    let e = entity(&mut actual, worker).unwrap();
    let load = actual.world().get::<Harvester>(e).unwrap().load;
    let money = actual
        .world()
        .resource::<Players>()
        .get(P0)
        .unwrap()
        .credits();
    let events = tick(&mut actual, vec![strike(silo)]);
    assert!(
        events
            .iter()
            .all(|e| !matches!(e, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e,SimEvent::BuildingDestroyed{building,..} if *building==refinery))
    );
    assert!(
        entity(&mut actual, worker).is_some(),
        "worker must survive the physical strike"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::OreDelivered { .. }))
    );
    assert_eq!(actual.world().get::<Harvester>(e).unwrap().load, load);
    assert_eq!(
        actual
            .world()
            .resource::<Players>()
            .get(P0)
            .unwrap()
            .credits(),
        money
    );
    let events = tick(&mut control, vec![]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SimEvent::OreDelivered { .. })),
        "living twin must actually deliver this tick, not merely remain an eligible refinery"
    );
    assert!(
        control
            .world()
            .resource::<Players>()
            .get(P0)
            .unwrap()
            .credits()
            > money
    );
    assert_cells(&mut actual, "native refinery death");
    assert_cells(&mut control, "native living delivery");
}

#[test]
fn ra9_native_command_killed_depot_cannot_heal_or_charge_but_one_hp_depot_can() {
    for dies in [true, false] {
        let mut app = world();
        place(&mut app, P0, "allied_yard", 1, 1);
        place(&mut app, P0, "allied_power", 5, 1);
        let depot = place(&mut app, P0, "depot", 20, 20);
        let unit = spawn(&mut app, P0, "tank", 19, 20);
        set_hp(&mut app, unit, 200); // actual living patient; never a zero-health corpse
        let silo = armed_silo(&mut app);
        ticks(&mut app, 25);
        let events = tick(
            &mut app,
            vec![issue(
                P0,
                Command::Interact {
                    unit,
                    target: depot,
                },
            )],
        );
        assert!(
            events
                .iter()
                .all(|e| !matches!(e, SimEvent::CommandRejected { .. })),
            "{events:?}"
        );
        assert_eq!(
            app.world().resource::<SpecialState>().docked.get(&unit),
            Some(&depot)
        );
        let interval = defs(&app).rules().unwrap().economy.unit_repair_interval as u64;
        while !app
            .world()
            .resource::<sim::SimTick>()
            .0
            .is_multiple_of(interval)
        {
            tick(&mut app, vec![]);
        }
        let before_hp = hp(&mut app, unit).unwrap();
        let money = app.world().resource::<Players>().get(P0).unwrap().credits();
        let damage = footprint_strike_damage(&defs(&app), "depot");
        set_hp(&mut app, depot, damage + i32::from(!dies));
        let events = tick(&mut app, vec![strike(silo)]);
        assert!(
            events
                .iter()
                .all(|e| !matches!(e, SimEvent::CommandRejected { .. })),
            "{events:?}"
        );
        assert_eq!(
            events
                .iter()
                .any(|e| matches!(e,SimEvent::BuildingDestroyed{building,..} if *building==depot)),
            dies
        );
        // Fixture Nuke300, plate40%, radius4cells/edge10%, one-cell distance: floor93.
        let dealt = 300_i64 * (100 * 1024 - 90 * 256) * 40 / (100 * 1024 * 100);
        assert_eq!(dealt, 93);
        assert_eq!(
            hp(&mut app, unit),
            Some(before_hp - 93 + if dies { 0 } else { 10 })
        );
        if dies {
            assert_eq!(
                app.world().resource::<Players>().get(P0).unwrap().credits(),
                money
            );
            assert!(
                !app.world()
                    .resource::<SpecialState>()
                    .docked
                    .contains_key(&unit)
            );
        } else {
            assert!(app.world().resource::<Players>().get(P0).unwrap().credits() < money);
        }
        assert_cells(&mut app, "native depot-host death/living boundary");
    }
}
