//! Current-source liveness at real unload/production events. The Rules-present RA5 fixture's
//! strategic=0 role Nuke is immediate in Commands; this is not the shipped delayed missile.
mod ra5_common;
use bevy_ecs::{resource::Resource, schedule::IntoScheduleConfigs, world::World};
use ra5_common::*;
use sim::{
    Building, CellPos, Command, Definitions, HarvestState, Health, Mobile, Orders, ProductionItem,
    SimCoreSet, SimEvent, SimId, SimSchedule, SimSet,
};

#[derive(Resource, Default)]
struct CommandBoundary {
    source: Option<SimId>,
    hp: Option<i32>,
}
fn record_commands(world: &mut World) {
    let source = world.resource::<CommandBoundary>().source;
    let hp = source.and_then(|source| {
        world
            .query::<(&SimId, &Health)>()
            .iter(world)
            .find_map(|(id, health)| (*id == source).then_some(health.hit_points))
    });
    world.resource_mut::<CommandBoundary>().hp = hp;
}
fn install_boundary(app: &mut bevy_app::App) {
    app.init_resource::<CommandBoundary>();
    app.add_systems(
        SimSchedule,
        record_commands
            .in_set(SimSet::Commands)
            .after(SimCoreSet(SimSet::Commands)),
    );
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
fn supports_world(d: Definitions) -> (bevy_app::App, SimId) {
    let mut app = world_of(d, 7);
    place(&mut app, P0, "allied_yard", 1, 1);
    place(&mut app, P0, "allied_power", 5, 1);
    place(&mut app, P1, "soviet_power", 32, 25);
    let silo = place(&mut app, P1, "silo", 32, 30);
    spawn(&mut app, P1, "engineer", 24, 20); // genuine current sight of strike footprint
    install_boundary(&mut app);
    (app, silo)
}
fn strike_damage(d: &Definitions) -> i32 {
    let k = d.building_kind("vault").unwrap();
    let b = d.building(k).unwrap();
    let e = d.effect(sim::Effect::Nuke).unwrap();
    let wh = d.warhead(e.warhead).unwrap();
    let versus = wh.versus[usize::from(d.building_combat(k).unwrap().armour.0)];
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
    // Independent full-footprint sum: origin 30, orthogonal cells 23+23, diagonal 20.
    let mut total = 0_i64;
    for y in 0..u64::from(b.height) {
        for x in 0..u64::from(b.width) {
            let distance = ((x * 256).pow(2) + (y * 256).pow(2)).isqrt();
            total += (i64::from(e.damage)
                * (100 * i64::from(wh.cell_spread)
                    - i64::from(100 - wh.percent_at_max) * i64::try_from(distance).unwrap())
                * i64::from(versus)
                / (100 * i64::from(wh.cell_spread) * 100))
                .max(1);
        }
    }
    assert_eq!(total, 96);
    i32::try_from(total).unwrap()
}
fn arm_source_boundary(app: &mut bevy_app::App, source: SimId, alive_after: bool) {
    let damage = strike_damage(&defs(app));
    set_hit_points(app, source, damage + i32::from(alive_after));
    assert!(
        hit_points(app, source) > 0,
        "positive injury; the command must cause any death"
    );
    app.world_mut().resource_mut::<CommandBoundary>().source = Some(source);
}
fn delivered(events: &[SimEvent], worker: SimId) -> i64 {
    let amounts: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            SimEvent::OreDelivered {
                harvester, credits, ..
            } if *harvester == worker => Some(*credits),
            _ => None,
        })
        .collect();
    assert_eq!(
        amounts,
        [500],
        "genuine base delivery, not an injected event"
    );
    amounts[0]
}
fn purifier_case(presence: Option<bool>) {
    let (mut app, silo) = supports_world(definitions());
    let purifier = presence.map(|_| place(&mut app, P0, "vault", 20, 20));
    let refinery = place(&mut app, P0, "allied_refinery", 7, 20);
    ore(&mut app, 6, 20, 1000);
    let worker = spawn(&mut app, P0, "miner_a", 6, 20);
    let mut ready = false;
    for _ in 0..3000 {
        let h = harvester(&mut app, worker);
        let m = component::<Mobile>(&mut app, worker).unwrap();
        let o = component::<Orders>(&mut app, worker).unwrap();
        if matches!(h.state, HarvestState::ToRefinery { refinery: r, dock } if r == refinery && dock == m.cell)
            && h.load == 20
            && m.step.is_none()
            && o.current.is_none()
        {
            ready = true;
            break;
        }
        tick(&mut app, vec![]);
    }
    assert!(
        ready,
        "real harvesting must reach next Economy's full unload"
    );
    assert!(app.world().resource::<sim::SpecialState>().charge[&silo] >= 20);
    if let Some(source) = purifier {
        arm_source_boundary(&mut app, source, presence == Some(true));
    }
    let before = credits(&app, P0);
    let events = tick(&mut app, vec![strike(silo)]);
    assert!(rejections(&events).is_empty(), "{events:?}");
    if let Some(source) = purifier {
        assert_eq!(
            app.world().resource::<CommandBoundary>().hp,
            Some(i32::from(presence == Some(true)))
        );
        if presence == Some(true) {
            assert_eq!(hit_points(&mut app, source), 1);
        } else {
            assert!(component::<Health>(&mut app, source).is_none());
            assert!(events.iter().any(
                |e| matches!(e,SimEvent::BuildingDestroyed{building,..} if *building == source)
            ));
        }
    }
    let base = delivered(&events, worker);
    assert_eq!(
        credits(&app, P0) - before,
        base + if presence == Some(true) { 125 } else { 0 },
        "only a living eligible support credits its exact bonus"
    );
    assert_eq!(harvester(&mut app, worker).load, 0);
    assert_cells(&mut app, "current purifier support");
}
#[test]
fn ra9_command_dead_purifier_cannot_credit_same_tick_delivery() {
    purifier_case(Some(false));
}
#[test]
fn ra9_command_one_hp_purifier_still_credits_exact_bonus() {
    purifier_case(Some(true));
}
#[test]
fn ra9_no_purifier_delivery_keeps_exact_base_credit() {
    purifier_case(None);
}

fn cloner_definitions() -> Definitions {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let text = std::fs::read_to_string(fixture("arsenal.ron")).unwrap();
    let line = text
        .lines()
        .find(|l| l.trim_start().starts_with("(id: \"vault\", "))
        .unwrap();
    assert_eq!(line.matches("role: \"purifier\"").count(), 1);
    assert_eq!(line.matches("capturable: 0").count(), 1);
    let changed = line
        .replace("role: \"purifier\"", "role: \"cloner\"")
        .replace("capturable: 0", "capturable: 1");
    let path = std::path::PathBuf::from(std::env::var_os("HOME").unwrap())
        .join("Library/Caches/Codex/reviews/MyGame001/ra2-final-review-20261021")
        .join(format!(
            "runtime2-cloner-{}-{}.ron",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
    std::fs::write(&path, text.replace(line, &changed)).unwrap();
    definitions()
        .with_arsenal(path)
        .expect("same RA5 fixture, vault support role and capture trait only")
}
fn pending(app: &mut bevy_app::App, id: SimId) -> Vec<sim::UnitKind> {
    component::<Building>(app, id).unwrap().pending_units
}
fn cloner_case(presence: Option<bool>) {
    let (mut app, silo) = supports_world(cloner_definitions());
    let older = presence.map(|_| place(&mut app, P0, "vault", 20, 20));
    let captured = presence.map(|_| {
        let vat = place(&mut app, P1, "vault", 10, 5);
        let engineer = spawn(&mut app, P0, "engineer", 9, 5);
        tick(&mut app, vec![]);
        let events = tick(
            &mut app,
            vec![issue(
                P0,
                Command::Interact {
                    unit: engineer,
                    target: vat,
                },
            )],
        );
        assert!(rejections(&events).is_empty(), "{events:?}");
        assert_eq!(component::<sim::Owner>(&mut app, vat).unwrap().0, P0);
        assert!(
            component::<Health>(&mut app, engineer).is_none(),
            "real second vat capture consumes actual Engineer"
        );
        vat
    });
    let barracks = place(&mut app, P0, "allied_barracks", 7, 12);
    ticks(&mut app, 25); // actual immediate-role fixture charge, no charge-map injection
    let d = defs(&app);
    let kind = d.unit_kind("rifleman").unwrap();
    let item = ProductionItem::Unit(kind);
    let key = d.queue_of(item).unwrap();
    let time = d.production(item).unwrap().build_time;
    let events = tick(&mut app, vec![issue(P0, Command::QueueProduction { item })]);
    assert!(rejections(&events).is_empty(), "{events:?}");
    for _ in 0..time {
        let head = app
            .world()
            .resource::<sim::Players>()
            .get(P0)
            .unwrap()
            .lane(key)[0];
        if head.progress + 1 == time {
            break;
        }
        assert!(head.progress + 1 < time);
        tick(&mut app, vec![]);
    }
    assert_eq!(
        app.world()
            .resource::<sim::Players>()
            .get(P0)
            .unwrap()
            .lane(key)[0]
            .progress
            + 1,
        time
    );
    if let Some(source) = older {
        arm_source_boundary(&mut app, source, presence == Some(true));
    }
    let events = tick(&mut app, vec![strike(silo)]);
    assert!(rejections(&events).is_empty(), "{events:?}");
    let births: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            SimEvent::UnitSpawned { building, unit, .. } if *building == barracks => Some(*unit),
            _ => None,
        })
        .collect();
    assert_eq!(
        births.len(),
        1,
        "genuine queued production must complete after command-stage strike"
    );
    if let (Some(older), Some(captured)) = (older, captured) {
        assert_eq!(
            app.world().resource::<CommandBoundary>().hp,
            Some(i32::from(presence == Some(true)))
        );
        if presence == Some(true) {
            assert!(pending(&mut app, captured).is_empty());
        } else {
            assert!(component::<Health>(&mut app, older).is_none());
            assert!(events.iter().any(
                |e| matches!(e,SimEvent::BuildingDestroyed{building,..} if *building == older)
            ));
        }
        let selected = if presence == Some(true) {
            older
        } else {
            captured
        };
        assert_eq!(
            pending(&mut app, selected),
            vec![kind],
            "a dead older vat must not shadow the actual captured living vat"
        );
        let events = tick(&mut app, vec![]);
        let copies: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                SimEvent::UnitSpawned { building, unit, .. } if *building == selected => {
                    Some(*unit)
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            copies.len(),
            1,
            "exactly one actual clone leaves the eligible source"
        );
        assert_ne!(copies[0], births[0]);
        assert!(pending(&mut app, selected).is_empty());
    } else {
        let events = tick(&mut app, vec![]);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, SimEvent::UnitSpawned { .. })),
            "no support means no extra copy"
        );
    }
    assert_cells(&mut app, "current Cloner support");
}
#[test]
fn ra9_command_dead_older_cloner_cannot_shadow_captured_living_vat() {
    cloner_case(Some(false));
}
#[test]
fn ra9_command_one_hp_older_cloner_still_produces_exactly_one_copy() {
    cloner_case(Some(true));
}
#[test]
fn ra9_no_cloner_production_has_no_extra_copy() {
    cloner_case(None);
}
