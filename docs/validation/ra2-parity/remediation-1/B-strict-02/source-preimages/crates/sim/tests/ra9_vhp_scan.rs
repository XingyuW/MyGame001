// Historical preparation note: UNVALIDATED / UNCOMPILED / UNRUN: native current-API Patriot baseline and control bodies.
// Proposed integration file crates/sim/tests/ra9_vhp_scan.rs, only after root releases RA8.
mod ra7_common;
use bevy_app::App;
use ra7_common::*;
use sim::{Command, Definitions, Health, Projectiles, SimEvent, SimId, Weapon};
fn hp(app: &mut App, id: SimId) -> Option<i32> {
    let e = entity(app, id)?;
    app.world().get::<Health>(e).map(|h| h.hit_points)
}
fn no_rejections(events: &[SimEvent]) {
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
}
// One balance-independent slow-flight variant, used only to expose real repair/source-death
// while shots remain in flight. Stock cadence/damage/range/HP are otherwise unchanged.
fn slow_flight_definitions(tag: usize) -> Definitions {
    let mut text = std::fs::read_to_string(data_dir().join("combat.ron")).expect("combat");
    let mut count = 0;
    text = text
        .lines()
        .map(|line| {
            if line.contains(r#"(id: "red_eye2","#) {
                count += 1;
                assert_eq!(line.matches("projectile_speed: 191").count(), 1);
                line.replace("projectile_speed: 191", "projectile_speed: 19")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(count, 1);
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::path::PathBuf::from(std::env::var_os("TMPDIR").expect("configured TMPDIR"))
        .join(format!(
            "ra9-vhp-slow-flight-{}-{tag}-{}.ron",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
    {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .expect("exclusive per-invocation parser fixture");
        file.write_all(text.as_bytes())
            .expect("short-lived parser file");
    }
    let result = sim::load_match_definitions(
        data_dir().join("units.ron"),
        data_dir().join("economy.ron"),
        &path,
    )
    .and_then(|d| d.with_rules(data_dir().join("rules.ron")))
    .and_then(|d| d.with_arsenal(data_dir().join("arsenal.ron")));
    std::fs::remove_file(path).expect("remove before load-result assertion");
    result.unwrap_or_else(|e| panic!("{e}"))
}
struct Salvo {
    app: App,
    patriots: Vec<SimId>,
    a: SimId,
    b: SimId,
    tanya: Option<SimId>,
}
fn scenario(n: usize, slow: bool, plant_source_death: bool) -> Salvo {
    let mut app = world();
    if slow {
        app.world_mut().insert_resource(slow_flight_definitions(n));
    }
    put(&mut app, P0, "nuclear_reactor", 2, 2);
    let patriots = [(8, 8), (8, 11), (8, 14)]
        .into_iter()
        .take(n)
        .map(|(x, y)| put(&mut app, P0, "patriot", x, y))
        .collect::<Vec<_>>();
    let a = put(&mut app, P1, "kirov", 18, 11);
    let b = put(&mut app, P1, "kirov", 20, 11);
    // Legal before-first-tick scenario HP. Native SelfHealing raises149→150 at tick0.
    let e = entity(&mut app, a).expect("target");
    app.world_mut()
        .get_mut::<Health>(e)
        .expect("health")
        .hit_points = 149;
    for id in [a, b] {
        let e = entity(&mut app, id).expect("target");
        app.world_mut().entity_mut(e).remove::<Weapon>();
    }
    put(&mut app, P0, "engineer", 19, 14); // Real Sight4 sees both aircraft, no forged visibility.
    let tanya = plant_source_death.then(|| put(&mut app, P1, "tanya", 7, 8));
    let events = tick(&mut app, vec![]);
    no_rejections(&events);
    assert_eq!(hp(&mut app, a), Some(150));
    for id in [a, b] {
        let e = entity(&mut app, id).expect("target");
        let cell = app.world().get::<sim::Mobile>(e).expect("mobile").cell;
        assert!(
            app.world()
                .resource::<sim::Players>()
                .get(P0)
                .expect("player")
                .vision()
                .is_visible(cell)
        );
        assert!(
            !app.world()
                .resource::<sim::SpecialState>()
                .hidden
                .contains(&(P0, id))
        );
    }
    Salvo {
        app,
        patriots,
        a,
        b,
        tanya,
    }
}
fn estimate(p: &sim::Projectile, d: &Definitions) -> i32 {
    assert!(!p.scattered, "only accurate launches count in this witness");
    let w = d.weapon(p.weapon).expect("weapon");
    let h = d.warhead(w.warhead).expect("warhead");
    let t = d
        .unit_combat(d.unit_kind("kirov").expect("kind"))
        .expect("terms");
    let versus = sim::versus_against(
        h,
        &sim::TargetTraits {
            armour: t.armour,
            airborne: true,
            ..Default::default()
        },
    );
    sim::impact_damage(
        w.damage,
        p.firepower_pm,
        versus,
        h.cell_spread,
        h.percent_at_max,
        0,
        1000,
    )
}
fn first_salvo(s: &mut Salvo) -> Vec<(SimId, SimId)> {
    let events = tick(&mut s.app, vec![]);
    no_rejections(&events);
    let d = defs(&s.app);
    let weapon = d.weapon_kind("red_eye2").expect("weapon");
    let shots = events
        .iter()
        .filter_map(|e| match e {
            SimEvent::WeaponFired {
                attacker,
                target,
                weapon: w,
            } if s.patriots.contains(attacker) && *w == weapon => Some((*attacker, *target)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        shots.len(),
        s.patriots.len(),
        "native Patriot launches required: {events:?}"
    );
    let pending = s
        .app
        .world()
        .resource::<Projectiles>()
        .iter()
        .filter(|p| s.patriots.contains(&p.attacker) && p.target == s.a)
        .collect::<Vec<_>>();
    assert!(
        pending.len() >= s.patriots.len().min(2),
        "first two accurate missiles must exist in flight"
    );
    assert!(
        pending
            .iter()
            .all(|p| p.remaining > 0 && p.weapon == weapon)
    );
    if s.patriots.len() >= 2 {
        let estimated = pending
            .iter()
            .take(2)
            .map(|p| i64::from(estimate(p, &d)))
            .sum::<i64>();
        assert_eq!(
            estimated, 150,
            "native selected RedEye2/SAMWH/armour/firepower estimate"
        );
        assert!(i64::from(hp(&mut s.app, s.a).expect("alive")) <= estimated);
    }
    shots
}

#[test]
fn ra9_vhp_before_three_native_patriots_oversaturate_the_nearest_aircraft() {
    let mut s = scenario(3, false, false);
    let shots = first_salvo(&mut s);
    assert_eq!(
        shots,
        s.patriots.iter().map(|id| (*id, s.a)).collect::<Vec<_>>()
    );
    assert_eq!(
        hp(&mut s.app, s.a),
        Some(150),
        "no impact yet: this is pending damage"
    );
    assert!(hp(&mut s.app, s.b).is_some());
}
#[test]
fn ra9_vhp_current_target_death_releases_native_patriots_to_the_other_real_aircraft() {
    let mut s = scenario(3, false, false);
    first_salvo(&mut s);
    let mut gone = false;
    let mut selected_b = false;
    for _ in 0..160 {
        let events = tick(&mut s.app, vec![]);
        no_rejections(&events);
        gone |= entity(&mut s.app, s.a).is_none();
        selected_b |= events.iter().any(|e| {
            matches!(e,SimEvent::WeaponFired {attacker,target,..}
            if s.patriots.contains(attacker)&&*target==s.b)
        });
        if gone && selected_b {
            break;
        }
    }
    assert!(gone, "real projectile impacts must destroy first target");
    assert!(
        selected_b,
        "native later acquisition must select living reachable B"
    );
}
#[test]
fn ra9_vhp_real_self_repair_occurs_while_slow_fixture_missiles_still_fly() {
    let mut s = scenario(2, true, false);
    first_salvo(&mut s);
    let before = hp(&mut s.app, s.a).expect("target");
    let mut healed = false;
    for _ in 0..22 {
        tick(&mut s.app, vec![]);
        if hp(&mut s.app, s.a).is_some_and(|hp| hp > before) {
            assert!(
                s.app
                    .world()
                    .resource::<Projectiles>()
                    .iter()
                    .filter(|p| s.patriots.contains(&p.attacker)
                        && p.target == s.a
                        && p.remaining > 0)
                    .count()
                    >= 2
            );
            healed = true;
            break;
        }
    }
    assert!(
        healed,
        "genuine Economy SelfHealing, not injected HP or forged repair history"
    );
    // This establishes the real repair premise. It cannot prove a nonexistent VHP reset.
}
#[test]
fn ra9_vhp_native_source_death_does_not_cancel_its_existing_delayed_missile() {
    let mut s = scenario(1, true, true);
    first_salvo(&mut s);
    let source = s.patriots[0];
    let events = tick(
        &mut s.app,
        vec![issue(
            P1,
            Command::Interact {
                unit: s.tanya.expect("Tanya"),
                target: source,
            },
        )],
    );
    no_rejections(&events);
    assert!(
        s.app
            .world()
            .resource::<sim::SpecialState>()
            .charges
            .get(&source)
            .is_some_and(|c| c.c4)
    );
    let mut died_in_flight = false;
    for _ in 0..50 {
        tick(&mut s.app, vec![]);
        let gone = entity(&mut s.app, source).is_none();
        let pending = s
            .app
            .world()
            .resource::<Projectiles>()
            .iter()
            .any(|p| p.attacker == source && p.target == s.a && p.remaining > 0);
        if gone && pending {
            died_in_flight = true;
            break;
        }
    }
    assert!(
        died_in_flight,
        "real C4 death/Cleanup before native projectile impact"
    );
    let at_death = hp(&mut s.app, s.a).expect("target");
    let mut damaged = false;
    for _ in 0..180 {
        tick(&mut s.app, vec![]);
        if hp(&mut s.app, s.a).is_some_and(|hp| hp < at_death) {
            damaged = true;
            break;
        }
    }
    assert!(
        damaged,
        "one dead source's genuine missile must still cause native damage"
    );
}
