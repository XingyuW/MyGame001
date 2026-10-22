//! Historical preparation note: UNVALIDATED / PENDING. Proposed crates/sim/tests/ra9_veterancy.rs.
mod ra4_common;
use bevy_ecs::{resource::Resource, schedule::IntoScheduleConfigs, world::World};
use ra4_common::*;
use sim::{Command, Experience, ProductionItem, SimCoreSet, SimId, SimSchedule, SimSet, VETERAN};

#[derive(Resource, Default)]
struct PromoteOnce(Option<SimId>);
fn promote_once(world: &mut World) {
    if let Some(unit) = world.resource_mut::<PromoteOnce>().0.take() {
        assert!(sim::promote(world, unit, VETERAN));
    }
}

#[test]
fn ra9_zero_cost_explicit_scheduled_promotion_is_consistent_with_rank_threshold() {
    let original = std::fs::read_to_string(fixture("economy.ron")).unwrap();
    let anchor = "(unit: \"rifleman\", cost: 100,";
    assert_eq!(original.matches(anchor).count(), 1);
    let altered = original.replacen(anchor, "(unit: \"rifleman\", cost: 0,", 1);
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ra9-veterancy");
    std::fs::create_dir_all(&dir).unwrap();
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = dir.join(format!(
        "zero-cost-{}-{:?}-{}.ron",
        std::process::id(),
        std::thread::current().id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .and_then(|mut f| f.write_all(altered.as_bytes()))
        .unwrap();
    let definitions =
        sim::load_match_definitions(fixture("units.ron"), &path, fixture("combat.ron"))
            .and_then(|d| d.with_rules(fixture("rules.ron")))
            .and_then(|d| d.with_arsenal(fixture("arsenal.ron")))
            .unwrap();
    std::fs::remove_file(path).unwrap();
    let mut app = world_of(definitions, 7);
    app.init_resource::<PromoteOnce>();
    app.add_systems(
        SimSchedule,
        promote_once
            .in_set(SimSet::Ai)
            .after(SimCoreSet(SimSet::Ai)),
    );
    place(&mut app, P0, "allied_yard", 1, 1);
    place(&mut app, P0, "allied_power", 5, 1);
    place(&mut app, P0, "allied_barracks", 10, 10);
    let kind = defs(&app).unit_kind("rifleman").unwrap();
    let mut events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::QueueProduction {
                item: ProductionItem::Unit(kind),
            },
        )],
    );
    events.extend(ticks(&mut app, 20));
    let unit = events
        .iter()
        .find_map(|e| match e {
            sim::SimEvent::UnitSpawned { unit, .. } => Some(*unit),
            _ => None,
        })
        .expect("real zero-cost paid-queue completion");
    let world = app.world_mut();
    let e = world
        .query::<(bevy_ecs::entity::Entity, &SimId)>()
        .iter(world)
        .find(|(_, id)| **id == unit)
        .map(|(e, _)| e)
        .unwrap();
    assert_eq!(
        app.world().get::<Experience>(e).unwrap().rank,
        0,
        "new free unit starts green"
    );
    app.world_mut().resource_mut::<PromoteOnce>().0 = Some(unit);
    tick(&mut app, vec![]);
    let xp = *app.world().get::<Experience>(e).unwrap();
    let rules = defs(&app).rules().unwrap().veterancy;
    assert_eq!(xp.rank, VETERAN);
    assert_eq!(
        xp.value, 3,
        "normalized one-credit base times fixture VeteranRatio3"
    );
    assert_eq!(
        sim::rank_of(xp.value, 0, rules.veteran_ratio_pm, rules.veteran_cap),
        xp.rank
    );
    for cost in [0, 1, 333, i64::MAX] {
        for ratio in [0, 1, 3000, i32::MAX] {
            for rank in [0, VETERAN, sim::ELITE] {
                let needed = sim::experience_for(rank, cost, ratio);
                // Saturated thresholds beyond i64 are not representable promises.
                if needed < i64::MAX {
                    assert!(sim::rank_of(needed, cost, ratio, 2) >= rank);
                }
            }
        }
    }
    assert_cells(&mut app, "after real scheduled promotion");
}
