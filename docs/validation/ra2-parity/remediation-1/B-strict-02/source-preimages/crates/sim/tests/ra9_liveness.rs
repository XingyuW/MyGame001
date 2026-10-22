//! Historical preparation note: UNVALIDATED / PENDING. Native scheduled liveness regression draft.
//! Historical preparation note: Proposed integration path crates/sim/tests/ra9_liveness.rs.
mod ra4_common;
use ra4_common::*;
use sim::{CellPos, Command, Players, ProductionItem, SimEvent};

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

#[test]
fn ra9_real_command_strike_stops_dead_producer_progress_payment_and_spawn_at_exact_death_boundary()
{
    // Positive scenario injury brackets the independently summed footprint damage.
    for dies in [true, false] {
        let mut app = world();
        place(&mut app, P0, "allied_yard", 1, 1);
        place(&mut app, P0, "allied_power", 5, 1);
        let factory = place(&mut app, P0, "allied_barracks", 20, 20);
        place(&mut app, P1, "soviet_power", 32, 25);
        let silo = place(&mut app, P1, "silo", 32, 30);
        spawn(&mut app, P1, "engineer", 24, 20); // unarmed sight, no incidental return fire
        ticks(&mut app, 25);
        let definitions = defs(&app);
        let kind = definitions.unit_kind("recruit").unwrap();
        let producer = definitions.building_kind("allied_barracks").unwrap();
        let events = tick(
            &mut app,
            vec![issue(
                P0,
                Command::QueueProduction {
                    item: ProductionItem::Unit(kind),
                },
            )],
        );
        assert!(
            events
                .iter()
                .all(|e| !matches!(e, SimEvent::CommandRejected { .. })),
            "{events:?}"
        );
        let progress = app
            .world()
            .resource::<Players>()
            .get(P0)
            .unwrap()
            .queue(producer)[0]
            .progress;
        assert!(progress > 0, "real paid production must already be active");
        let damage = footprint_strike_damage(&definitions, "allied_barracks");
        set_hp(&mut app, factory, damage + i32::from(!dies));
        let before_credits = app.world().resource::<Players>().get(P0).unwrap().credits();
        let events = tick(
            &mut app,
            vec![issue(
                P1,
                Command::Superweapon {
                    building: silo,
                    target: CellPos::new(20, 20),
                    units: vec![],
                },
            )],
        );
        assert!(
            events
                .iter()
                .all(|e| !matches!(e, SimEvent::CommandRejected { .. })),
            "{events:?}"
        );
        let destroyed = events.iter().any(|e| {
            matches!(e,
            SimEvent::BuildingDestroyed { building, .. } if *building == factory)
        });
        assert_eq!(
            destroyed, dies,
            "the native strike must establish the boundary"
        );
        let after = app.world().resource::<Players>().get(P0).unwrap();
        if dies {
            assert_eq!(after.queue(producer)[0].progress, progress);
            assert_eq!(after.credits(), before_credits);
            assert!(!events.iter().any(|e| matches!(e,
                SimEvent::UnitSpawned { building, .. } if *building == factory)));
        } else {
            assert!(after.queue(producer)[0].progress > progress);
            assert!(after.credits() < before_credits);
        }
        assert_cells(&mut app, "after native command death/living control");
    }
}
