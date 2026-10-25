//! Remediation fixtures retain genuine loaded Rules and drive all effects through the schedule.
#![allow(dead_code)]
#[path = "../ra9_soundness_common/mod.rs"]
mod native;
pub use native::*;
use sim::{CellPos, Command, Definitions, SimEvent, SimId, SpecialState, Weapon};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

pub fn disarm(app: &mut bevy_app::App, id: SimId) {
    let e = entity(app, id).expect("positive-HP passive fixture actor");
    app.world_mut().entity_mut(e).remove::<Weapon>();
}

pub fn variant(file: &str, edits: &[(&str, &str)]) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let mut text = std::fs::read_to_string(data_dir().join(file)).expect("shipped fixture source");
    for &(from, to) in edits {
        assert_eq!(
            text.matches(from).count(),
            1,
            "single-field fixture: {from}"
        );
        text = text.replacen(from, to, 1);
    }
    let dir = PathBuf::from(std::env::var_os("HOME").expect("home"))
        .join("Library/Caches/Codex/reviews/MyGame001/ra2-final-review-20261021/runtime-fixtures");
    std::fs::create_dir_all(&dir).expect("approved review fixtures directory");
    let path = dir.join(format!(
        "{}-{}-{file}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, text).expect("owned fixture");
    path
}

pub fn combat_variant(edits: &[(&str, &str)]) -> Definitions {
    shipped()
        .with_combat(variant("combat.ron", edits))
        .expect("Rules-present combat fixture")
}

pub fn arsenal_variant(edits: &[(&str, &str)]) -> Definitions {
    shipped()
        .with_arsenal(variant("arsenal.ron", edits))
        .expect("Rules-present arsenal fixture")
}

pub fn only_powerup(content: &str) -> Definitions {
    let text = std::fs::read_to_string(data_dir().join("rules.ron")).expect("rules");
    let mut edited = text.clone();
    for line in text
        .lines()
        .filter(|line| line.trim_start().starts_with("(id:") && line.contains("share:"))
    {
        let id = line.split('"').nth(1).expect("powerup id");
        let after = line.split("share:").nth(1).expect("share");
        let old = after.split(',').next().expect("integer share");
        let changed = line.replacen(
            &format!("share:{old}"),
            &format!("share: {}", u8::from(id == content)),
            1,
        );
        edited = edited.replacen(line, &changed, 1);
    }
    let path = variant("rules.ron", &[(text.as_str(), edited.as_str())]);
    let d = shipped()
        .with_rules(path)
        .expect("controlled crate shares")
        .with_arsenal(data_dir().join("arsenal.ron"))
        .expect("original arsenal");
    let enabled: Vec<_> = d
        .rules()
        .unwrap()
        .powerups
        .iter()
        .filter(|p| p.share > 0)
        .map(|p| p.id.as_str())
        .collect();
    assert_eq!(enabled, [content]);
    d
}

fn target_visible(app: &mut bevy_app::App, viewer: sim::PlayerId, target: SimId) -> bool {
    let e = entity(app, target).expect("native target body");
    let vision = app
        .world()
        .resource::<sim::Players>()
        .get(viewer)
        .unwrap()
        .vision();
    if let Some(mobile) = app.world().get::<sim::Mobile>(e) {
        vision.is_visible(mobile.cell)
    } else {
        vision.sees_footprint(app.world().get::<sim::Building>(e).unwrap().footprint)
    }
}

pub fn start_beam(app: &mut bevy_app::App, caster: SimId, target: SimId) -> Vec<SimEvent> {
    // Timed work fixtures install an earlier real observer. Untimed fixtures establish
    // the new caster's native sight while passive, before issuing the actual attack.
    if !target_visible(app, P1, target) {
        let e = entity(app, caster).unwrap();
        let original = *app
            .world()
            .get::<Weapon>(e)
            .expect("loaded native caster weapon");
        disarm(app, caster);
        tick(app, vec![]);
        app.world_mut().entity_mut(e).insert(original);
    }
    assert!(
        target_visible(app, P1, target),
        "native sight must precede public Temporal order"
    );
    let mut events = tick(
        app,
        vec![issue(
            P1,
            Command::Attack {
                units: vec![caster],
                target,
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    for _ in 0..30 {
        if app
            .world()
            .resource::<SpecialState>()
            .erasures
            .contains_key(&target)
        {
            assert!(hp(app, target) > 0);
            let e = &app.world().resource::<SpecialState>().erasures[&target];
            assert!(
                e.beams.contains_key(&caster),
                "real native caster must own channel"
            );
            return events;
        }
        events = tick(app, vec![]);
    }
    panic!("native Temporal channel never started");
}

pub fn release_beam(app: &mut bevy_app::App, caster: SimId, target: SimId) {
    let events = tick(
        app,
        vec![issue(
            P1,
            Command::Move {
                units: vec![caster],
                target: CellPos::new(45, 3),
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(
        !app.world()
            .resource::<SpecialState>()
            .erasures
            .contains_key(&target),
        "actual channel release"
    );
}

pub fn frozen(app: &mut bevy_app::App, target: SimId) {
    assert!(hp(app, target) > 0, "Temporal target is alive");
    assert!(
        app.world()
            .resource::<SpecialState>()
            .erasures
            .contains_key(&target)
    );
    cells(app);
}

pub fn credits(app: &bevy_app::App) -> i64 {
    app.world()
        .resource::<sim::Players>()
        .get(P0)
        .unwrap()
        .credits()
}

pub fn crate_at(app: &mut bevy_app::App, at: CellPos) {
    let grid = app.world().resource::<sim::Grid>().clone();
    assert!(
        app.world_mut()
            .resource_mut::<sim::Crates>()
            .place(&grid, at)
    );
}

pub fn collect_crate(
    app: &mut bevy_app::App,
    collector: SimId,
    at: CellPos,
) -> (sim::CrateContent, Option<SimId>) {
    let mut events = tick(
        app,
        vec![issue(
            P0,
            Command::Move {
                units: vec![collector],
                target: at,
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    for _ in 0..400 {
        if let Some(result) = events.iter().find_map(|event| match event {
            SimEvent::CrateTaken {
                unit,
                content,
                spawned,
                ..
            } if *unit == collector => Some((*content, *spawned)),
            _ => None,
        }) {
            return result;
        }
        events = tick(app, vec![]);
    }
    panic!("public collector never took crate");
}
