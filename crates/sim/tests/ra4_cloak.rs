//! RA4: submerged units (docs/RA2_PARITY_PLAN.md section 4.8; docs/DECISIONS.md R51), on the RA4
//! fixture (`tests/fixtures/data/ra4`; invariant I10), through `CommandQueue` and `SimSchedule`.
//!
//! In the fixture the sub and the squid are cloakable and have sensors of 2 cells, the boat has
//! sensors of 5 cells, and the rules keep a unit that fired in sight for 24 ticks. A tank sees
//! 7 cells and has no sensors.

mod ra4_common;

use bevy_app::App;
use ra4_common::*;
use sim::{PlayerId, Players, RejectReason, SimId, SpecialState};

const P2: PlayerId = PlayerId(2);

fn hidden(app: &App, player: PlayerId, unit: SimId) -> bool {
    app.world()
        .resource::<SpecialState>()
        .hidden
        .contains(&(player, unit))
}

/// Land west of column 12 and water east of it; a tank of P0 on the shore at (10, 10) and a
/// sub of P1 at (15, 10), five cells from it, that neither fires nor moves.
fn shore() -> (App, SimId, SimId) {
    let mut app = world();
    flood(&mut app, (12, 0), (39, 39));
    let tank = spawn(&mut app, P0, "tank", 10, 10);
    let sub = spawn(&mut app, P1, "sub", 15, 10);
    silence(&mut app, sub);
    tick(&mut app, Vec::new());
    (app, tank, sub)
}

/// Keeps `id` from firing and moving (scenario setup).
fn silence(app: &mut App, id: SimId) {
    app.world_mut()
        .resource_mut::<SpecialState>()
        .disabled
        .insert(id, u64::MAX);
}

fn sees_cell(app: &App, player: PlayerId, x: i32, y: i32) -> bool {
    app.world()
        .resource::<Players>()
        .get(player)
        .expect("player")
        .vision()
        .is_visible(sim::CellPos::new(x, y))
}

// --- Positive and boundary -----------------------------------------------------------------------

#[test]
fn a_submerged_unit_is_unseen_without_sensors_and_seen_with_them_in_range() {
    let (mut app, _, sub) = shore();
    // The tank sees the water the sub is in, and not the sub.
    assert!(sees_cell(&app, P0, 15, 10));
    assert!(hidden(&app, P0, sub));
    assert!(!hidden(&app, P1, sub), "never from its owner");
    // A boat of P0 with sensors of 5 cells: at 5 cells it detects the sub, at 6 it does not.
    for (x, y, detected) in [
        (20, 10, true),
        (21, 10, false),
        (18, 14, true),
        (19, 14, false),
        (15, 15, true),
        (15, 16, false),
    ] {
        let boat = spawn(&mut app, P0, "boat", x, y);
        silence(&mut app, boat);
        tick(&mut app, Vec::new());
        assert_eq!(!hidden(&app, P0, sub), detected, "a boat at ({x}, {y})");
        set_hp(&mut app, boat, 0);
        tick(&mut app, Vec::new());
        assert!(hidden(&app, P0, sub), "the boat is gone");
    }
    // The squid is cloakable too.
    let squid = spawn(&mut app, P1, "squid", 14, 12);
    tick(&mut app, Vec::new());
    assert!(hidden(&app, P0, squid) && sees_cell(&app, P0, 14, 12));
}

#[test]
fn the_sensors_of_a_cloaked_unit_detect_too_and_those_of_a_building() {
    let mut app = world();
    flood(&mut app, (0, 0), (39, 39));
    // Two subs of enemies: each has sensors of 2 cells.
    let ours = spawn(&mut app, P0, "sub", 10, 10);
    let theirs = spawn(&mut app, P1, "sub", 12, 10);
    silence(&mut app, ours);
    silence(&mut app, theirs);
    tick(&mut app, Vec::new());
    assert!(!hidden(&app, P0, theirs) && !hidden(&app, P1, ours));
    let far = spawn(&mut app, P1, "sub", 13, 10);
    silence(&mut app, far);
    tick(&mut app, Vec::new());
    assert!(hidden(&app, P0, far), "three cells from our sub");
    // A building with sensors (here the bunker, given 4 cells) detects from its footprint.
    let mut app = world_of(
        definitions_with_arsenal(
            "bunker_sensors",
            "bunker",
            &[("sensors_sight: 0", "sensors_sight: 4")],
        ),
        1,
    );
    flood(&mut app, (12, 0), (39, 39));
    place(&mut app, P0, "bunker", 11, 10);
    let near = spawn(&mut app, P1, "sub", 15, 10);
    let beyond = spawn(&mut app, P1, "sub", 16, 10);
    silence(&mut app, near);
    silence(&mut app, beyond);
    tick(&mut app, Vec::new());
    assert!(!hidden(&app, P0, near) && hidden(&app, P0, beyond));
}

#[test]
fn a_unit_that_fired_is_seen_for_the_cloak_delay() {
    let mut app = world();
    flood(&mut app, (0, 0), (39, 39));
    // An unarmed hovercraft of P0 without sensors, three cells from a sub of P1.
    let hovercraft = spawn(&mut app, P0, "hovercraft", 10, 10);
    set_hp(&mut app, hovercraft, 100_000);
    let sub = spawn(&mut app, P1, "sub", 13, 10);
    // It fires its torpedo every 30 ticks; after each shot it is seen for 24 ticks.
    let mut since_shot = None;
    let mut pattern = Vec::new();
    for _ in 0..100 {
        let events = tick(&mut app, Vec::new());
        if !shots(&events, sub).is_empty() {
            since_shot = Some(0);
        }
        if let Some(ticks) = since_shot {
            pattern.push((ticks, hidden(&app, P0, sub)));
            since_shot = Some(ticks + 1);
        }
    }
    assert!(pattern.len() > 60);
    for (ticks, unseen) in pattern {
        assert_eq!(unseen, ticks >= 24, "{ticks} ticks after a shot");
    }
}

// --- Negative: what hidden means ------------------------------------------------------------------

#[test]
fn an_unseen_unit_is_no_target_of_a_command_or_of_a_weapon() {
    let mut app = world();
    flood(&mut app, (0, 0), (39, 39));
    // The boat's sensors reach 5 cells; its launcher 12.
    let boat = spawn(&mut app, P0, "boat", 10, 10);
    let sub = spawn(&mut app, P1, "sub", 16, 10);
    silence(&mut app, sub);
    tick(&mut app, Vec::new());
    assert!(hidden(&app, P0, sub) && sees_cell(&app, P0, 16, 10));
    // An attack command names a unit the player does not know of.
    let events = tick(&mut app, vec![attack(P0, boat, sub)]);
    assert_eq!(rejections(&events), [RejectReason::UnknownTarget(sub)]);
    // Left alone the boat does nothing.
    let events = ticks(&mut app, 100);
    assert!(shots(&events, boat).is_empty());
    assert_eq!(hp(&mut app, sub), Some(300));
    // One cell nearer the sub is detected, attacked on the boat's own, and may be named.
    let mut app = world();
    flood(&mut app, (0, 0), (39, 39));
    let boat = spawn(&mut app, P0, "boat", 10, 10);
    let sub = spawn(&mut app, P1, "sub", 15, 10);
    silence(&mut app, sub);
    tick(&mut app, Vec::new());
    let mut events = tick(&mut app, vec![attack(P0, boat, sub)]);
    assert!(rejections(&events).is_empty());
    events.extend(ticks(&mut app, 100));
    assert!(!shots(&events, boat).is_empty());
    assert!(hp(&mut app, sub).is_some_and(|left| left < 300));
}

#[test]
fn an_attack_ends_when_its_target_goes_out_of_the_sensors() {
    let mut app = world();
    flood(&mut app, (0, 0), (39, 39));
    let boat = spawn(&mut app, P0, "boat", 10, 10);
    silence(&mut app, boat);
    // A squid of P0 is ordered to attack a sub that the boat detects: the sub is in the reach
    // of the squid's torpedo (5 cells) and outside the squid's own sensors (2 cells).
    let squid = spawn(&mut app, P0, "squid", 12, 12);
    let sub = spawn(&mut app, P1, "sub", 14, 10);
    set_hp(&mut app, sub, 100_000);
    silence(&mut app, sub);
    tick(&mut app, Vec::new());
    assert!(!hidden(&app, P0, sub));
    let mut events = tick(&mut app, vec![attack(P0, squid, sub)]);
    assert!(rejections(&events).is_empty());
    events.extend(ticks(&mut app, 40));
    assert!(!shots(&events, squid).is_empty());
    assert_eq!(mobile(&mut app, squid).cell, sim::CellPos::new(12, 12));
    // The detector is lost: the sub is hidden again, and the attack ends as on any target
    // that is no longer seen.
    set_hp(&mut app, boat, 0);
    let events = ticks(&mut app, 3);
    assert!(hidden(&app, P0, sub));
    let ended = events.iter().any(|event| {
        matches!(
            event,
            sim::SimEvent::AttackEnded {
                unit,
                reason: sim::AttackEnd::TargetHidden,
                ..
            } if *unit == squid
        )
    });
    assert!(ended, "{events:?}");
    assert_eq!(weapon(&mut app, squid).order, None);
    assert!(shots(&ticks(&mut app, 60), squid).is_empty());
}

#[test]
fn allies_see_a_cloaked_unit_and_every_enemy_needs_sensors_of_its_own() {
    let (mut app, _, sub) = shore();
    {
        let mut players = app.world_mut().resource_mut::<Players>();
        players.add(P2, 0).expect("a third player");
        assert!(players.set_team(P1, 2) && players.set_team(P2, 2));
    }
    tick(&mut app, Vec::new());
    assert!(!hidden(&app, P2, sub), "an ally of its owner");
    assert!(hidden(&app, P0, sub));
    // A third party that is an enemy of both: its sensors serve only itself.
    let neutral = PlayerId(3);
    app.world_mut()
        .resource_mut::<Players>()
        .add(neutral, 0)
        .expect("a fourth player");
    let boat = spawn(&mut app, neutral, "boat", 17, 10);
    silence(&mut app, boat);
    tick(&mut app, Vec::new());
    assert!(!hidden(&app, neutral, sub));
    assert!(hidden(&app, P0, sub));
}

#[test]
fn a_type_without_the_trait_is_never_hidden() {
    let mut app = world();
    flood(&mut app, (12, 0), (39, 39));
    spawn(&mut app, P0, "tank", 10, 10);
    let boat = spawn(&mut app, P1, "boat", 15, 10);
    silence(&mut app, boat);
    ticks(&mut app, 3);
    assert!(!hidden(&app, P0, boat));
    assert!(app.world().resource::<SpecialState>().surfaced.is_empty());
}

// --- Regression: hashing ---------------------------------------------------------------------------

#[test]
fn what_keeps_a_unit_in_sight_is_hashed() {
    let scenario = |until: Option<u64>| {
        let (mut app, _, sub) = shore();
        if let Some(until) = until {
            app.world_mut()
                .resource_mut::<SpecialState>()
                .surfaced
                .insert(sub, until);
        }
        hash(&mut app)
    };
    let base = scenario(None);
    assert_eq!(base, scenario(None));
    assert_ne!(scenario(Some(50)), base);
    assert_ne!(scenario(Some(50)), scenario(Some(51)));
}
