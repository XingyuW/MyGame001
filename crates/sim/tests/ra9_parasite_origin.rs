//! Historical preparation note: UNCOMPILED / UNRUN. Install only after schema10 + integrated370471e + identity delta.
//! Native current-model fixture, not stock Drone-on-Drone parity: common fixed HP2000/armour
//! light gives stock Parasite a positive versus. The two arms differ ONLY in TypeImmune0/1.
//! This proves ordinary periodic damage immunity, not refusal of parasite attachment.
mod ra7_common;
use bevy_app::App;
use ra7_common::*;
use sim::{
    Command, Definitions, ForceTarget, Grid, Health, Owner, ProductionItem, SimEvent, SimId,
    SimTick, SpecialState, Unit,
};

fn rejected(events: &[SimEvent]) {
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
}
fn fixture(immune: bool) -> Definitions {
    let original = std::fs::read_to_string(data_dir().join("combat.ron")).expect("combat");
    let mut changed = 0;
    let common = original
        .lines()
        .map(|line| {
            if line.contains(r#"unit: "terror_drone""#) {
                changed += 1;
                assert_eq!(
                    line.matches("hit_points: 100").count(),
                    1,
                    "fixed independent HP premise"
                );
                assert_eq!(
                    line.matches(r#"armour: "special_1""#).count(),
                    1,
                    "stock zero-versus premise"
                );
                line.replace("hit_points: 100", "hit_points: 2000")
                    .replace(r#"armour: "special_1""#, r#"armour: "light""#)
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(changed, 1);
    let original =
        std::fs::read_to_string(data_dir().join("arsenal.ron")).expect("schema10 arsenal");
    let mut changed = 0;
    let on = original
        .lines()
        .map(|line| {
            if line.contains(r#"id: "terror_drone""#) {
                changed += 1;
                assert_eq!(
                    line.matches("type_immune: 0").count(),
                    1,
                    "real schema10 field required"
                );
                line.replace("type_immune: 0", "type_immune: 1")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(changed, 1);
    let off = original
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>()
        .join("\n");
    let changed_lines = off
        .lines()
        .zip(on.lines())
        .filter(|(a, b)| a != b)
        .collect::<Vec<_>>();
    assert_eq!(off.lines().count(), on.lines().count());
    assert_eq!(changed_lines.len(), 1);
    assert_eq!(
        changed_lines[0]
            .0
            .replace("type_immune: 0", "type_immune: 1"),
        changed_lines[0].1,
        "exact one-field TypeImmune difference between paired fixtures"
    );
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let suffix = format!(
        "{}-{:?}-{}-{}",
        std::process::id(),
        std::thread::current().id(),
        u8::from(immune),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let cp = std::env::temp_dir().join(format!("ra9-parasite-origin-{suffix}.combat.ron"));
    let ap = std::env::temp_dir().join(format!("ra9-parasite-origin-{suffix}.arsenal.ron"));
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&cp)
        .and_then(|mut f| f.write_all(common.as_bytes()))
        .expect("short-lived independent parser fixture");
    let arsenal = if immune { on } else { off };
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&ap)
        .and_then(|mut f| f.write_all(arsenal.as_bytes()))
        .expect("short-lived arsenal variant");
    let result = shipped().with_combat(&cp).and_then(|d| d.with_arsenal(&ap));
    std::fs::remove_file(&cp).expect("remove combat before result assertions");
    std::fs::remove_file(&ap).expect("remove arsenal before result assertions");
    let d = result.unwrap_or_else(|e| panic!("{e}"));
    let k = d.unit_kind("terror_drone").expect("native kind");
    let t = d.unit_combat(k).expect("combat terms");
    assert_eq!(t.hit_points, 2000);
    assert_eq!(
        t.armour,
        d.armour_class("light").expect("positive-versus armour")
    );
    let m = d.metadata(ProductionItem::Unit(k)).expect("metadata");
    assert_eq!(m.type_immune, immune);
    assert!(!m.self_healing);
    assert!(!m.infantry);
    let w = d.weapon_kind("drone_jump").expect("selected real weapon");
    assert_eq!(t.weapon, Some(w));
    let w = d.weapon(w).expect("weapon");
    assert_eq!((w.damage, w.reload, w.projectile_speed), (50, 80, 0));
    let h = d.warhead(w.warhead).expect("warhead");
    assert!(h.parasite);
    assert!(!h.culling);
    let versus = sim::versus_against(
        h,
        &sim::TargetTraits {
            armour: t.armour,
            ..Default::default()
        },
    );
    assert_eq!(
        versus, 100,
        "stock Parasite can actually damage this common fixed fixture"
    );
    d
}
fn hp(app: &mut App, id: SimId) -> i32 {
    let e = entity(app, id).expect("live host");
    app.world().get::<Health>(e).expect("health").hit_points
}
fn remains_attached(app: &mut App, source: SimId, host: SimId, k: sim::UnitKind) {
    assert!(
        entity(app, source).is_none(),
        "source genuinely despawned/off-map"
    );
    assert!(
        !app.world()
            .resource::<Grid>()
            .holders(sim::CellPos::new(8, 8))
            .into_iter()
            .flatten()
            .any(|id| id == source),
        "source released its old grid cell"
    );
    let e = entity(app, host).expect("live host");
    assert_eq!(app.world().get::<Owner>(e).expect("owner").0, P0);
    assert_eq!(app.world().get::<Unit>(e).expect("type").kind, k);
    let p = app
        .world()
        .resource::<SpecialState>()
        .parasites
        .get(&host)
        .expect("native attachment");
    assert_eq!(p.passenger.id, source);
    assert_eq!(p.passenger.kind, k);
    assert_eq!(p.owner, P0);
    assert!(!p.squid);
    assert!(p.escape.is_none());
}
fn ordinary_periodic_damage(immune: bool) {
    let mut app = world();
    app.world_mut().insert_resource(fixture(immune));
    let k = defs(&app).unit_kind("terror_drone").expect("kind");
    let source = put(&mut app, P0, "terror_drone", 8, 8);
    let host = put(&mut app, P0, "terror_drone", 9, 8);
    assert_eq!(hp(&mut app, source), 2000);
    assert_eq!(hp(&mut app, host), 2000);
    // Normal first scheduled visibility/idle pass, with no enemies or fabricated knowledge.
    rejected(&tick(&mut app, vec![]));
    let fired_at = app.world().resource::<SimTick>().0;
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::ForceFire {
                units: vec![source],
                target: ForceTarget::Entity(host),
            },
        )],
    );
    rejected(&events);
    let weapon = defs(&app).weapon_kind("drone_jump").expect("weapon");
    assert!(
        events.iter().any(|e| matches!(e,SimEvent::WeaponFired {
        attacker,target,weapon:w } if *attacker==source&&*target==host&&*w==weapon)),
        "public own-target ForceFire must really fire: {events:?}"
    );
    remains_attached(&mut app, source, host, k);
    assert_eq!(
        hp(&mut app, host),
        2000,
        "parasite attachment is not an ordinary impact"
    );
    let due = app.world().resource::<SpecialState>().parasites[&host].due;
    assert_eq!(due, fired_at.checked_add(80).expect("fixture tick bound"));
    let mut expected = 2000;
    for _ in 0..2 {
        let due = app.world().resource::<SpecialState>().parasites[&host].due;
        let now = app.world().resource::<SimTick>().0;
        assert!(due >= now && due - now <= 80, "bounded native cadence");
        for _ in now..due {
            rejected(&tick(&mut app, vec![]));
            remains_attached(&mut app, source, host, k);
            assert_eq!(hp(&mut app, host), expected, "not due yet");
        }
        assert_eq!(app.world().resource::<SimTick>().0, due);
        let events = tick(&mut app, vec![]);
        rejected(&events);
        // detonate_one(report=false) emits no ordinary Impact receipt; HP and actual due
        // advance are intentional receipts. No direct detonate/Parasite/HP/owner injection.
        if !immune {
            expected -= 50;
        }
        assert_eq!(
            hp(&mut app, host),
            expected,
            "due periodic native damage must honor saved off-map source identity"
        );
        remains_attached(&mut app, source, host, k);
        assert_eq!(
            app.world().resource::<SpecialState>().parasites[&host].due,
            due.checked_add(80).expect("fixture bound")
        );
    }
}
#[test]
fn ra9_parasite_origin_same_kind_owner_false_takes_real_periodic_damage() {
    ordinary_periodic_damage(false);
}
#[test]
fn ra9_parasite_origin_same_kind_owner_type_immune_blocks_only_periodic_damage() {
    ordinary_periodic_damage(true);
}
