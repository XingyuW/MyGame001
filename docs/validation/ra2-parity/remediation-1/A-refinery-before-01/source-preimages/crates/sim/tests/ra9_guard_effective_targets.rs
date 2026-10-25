//! Guard automatic acquisition uses effective weapons before distance/ID ranking.
mod ra9_runtime_common;
use ra9_runtime_common::*;
use sim::{Cell, CellPos, Command, Grid, SimEvent, SimId, WeaponKind};

fn require_shot(app: &mut bevy_app::App, guard: SimId, target: SimId, weapon: Option<WeaponKind>) {
    let before = hp(app, target);
    let mut events = tick(app, vec![issue(P0, Command::Guard { units: vec![guard] })]);
    assert!(rejected(&events).is_empty(), "{events:?}");
    let mut fired = false;
    for _ in 0..240 {
        fired |= events.iter().any(|event| matches!(event, SimEvent::WeaponFired { attacker, target: found, weapon: chosen } if *attacker == guard && *found == target && weapon.is_none_or(|w| w == *chosen)));
        if fired && (entity(app, target).is_none() || hp(app, target) < before) {
            cells(app);
            return;
        }
        events = tick(app, vec![]);
    }
    // Before recording starvation, prove the same living source and legal target can
    // really fire/damage through the public explicit path. A failed positive is setup,
    // and cannot be credited as the intended automatic-acquisition failure.
    let mut events = tick(
        app,
        vec![issue(
            P0,
            Command::Attack {
                units: vec![guard],
                target,
            },
        )],
    );
    assert!(
        rejected(&events).is_empty(),
        "SETUP: legal explicit control rejected: {events:?}"
    );
    let mut explicit = false;
    for _ in 0..400 {
        explicit |= events.iter().any(|event| matches!(event, SimEvent::WeaponFired { attacker, target: found, weapon: chosen } if *attacker == guard && *found == target && weapon.is_none_or(|w| w == *chosen)));
        if explicit && (entity(app, target).is_none() || hp(app, target) < before) {
            panic!(
                "INTENDED: Guard starved legal target for240ticks, then the same source's public explicit Attack actually fired and damaged it; guard_fired={fired}"
            );
        }
        events = tick(app, vec![]);
    }
    panic!("SETUP: legal explicit positive produced no native shot/damage; explicit={explicit}");
}

fn warhead_edit(id: &str, armour: &str, percent: i32) -> sim::Definitions {
    let text = std::fs::read_to_string(data_dir().join("combat.ron")).unwrap();
    let row = text
        .lines()
        .find(|line| line.contains(&format!("(id: \"{id}\",")))
        .unwrap();
    let needle = format!("(armour: \"{armour}\", percent: ");
    let start = row.find(&needle).unwrap() + needle.len();
    let end = start + row[start..].find(')').unwrap();
    let mut changed = row.to_owned();
    changed.replace_range(start..end, &percent.to_string());
    shipped()
        .with_combat(variant("combat.ron", &[(row, &changed)]))
        .unwrap()
}

#[test]
fn ra9_guard_skips_zero_verses_nearer_target() {
    verses_case(0);
}
#[test]
fn ra9_guard_skips_one_verses_automatic_floor_nearer_target() {
    verses_case(1);
}
fn verses_case(percent: i32) {
    let mut app = world();
    app.world_mut()
        .insert_resource(warhead_edit("ap", "none", percent));
    let guard = put(&mut app, P0, "grizzly", 10, 10);
    let near = put(&mut app, P1, "gi", 11, 10);
    let legal = put(&mut app, P1, "rhino", 13, 10);
    disarm(&mut app, near);
    disarm(&mut app, legal);
    require_shot(&mut app, guard, legal, None);
}

#[test]
fn ra9_guard_aa_only_ship_skips_nearer_ground_for_air() {
    let mut app = world();
    let mut terrain = vec![
        Cell {
            passable: true,
            water: true,
            buildable: false,
            ore: 0
        };
        48 * 48
    ];
    terrain[10 * 48 + 11] = OPEN;
    app.world_mut()
        .insert_resource(Grid::from_cells(48, 48, terrain).unwrap());
    let guard = put(&mut app, P0, "aegis_cruiser", 10, 10);
    let near = put(&mut app, P1, "rhino", 11, 10);
    let legal = put(&mut app, P1, "rocketeer", 13, 10);
    disarm(&mut app, near);
    disarm(&mut app, legal);
    require_shot(&mut app, guard, legal, None);
}

#[test]
fn ra9_guard_uses_elite_secondary_after_primary_target_is_ineligible() {
    let mut app = world();
    app.world_mut()
        .insert_resource(warhead_edit("flak_twh", "flak", 0));
    let guard = put(&mut app, P0, "flak_trooper", 10, 10);
    assert!(sim::promote(app.world_mut(), guard, sim::ELITE));
    let near = put(&mut app, P1, "conscript", 11, 10);
    let legal = put(&mut app, P1, "rocketeer", 13, 10);
    disarm(&mut app, near);
    disarm(&mut app, legal);
    let weapon = defs(&app).weapon_kind("flak_guy_aa_gun_e").unwrap();
    require_shot(&mut app, guard, legal, Some(weapon));
}

#[test]
fn ra9_guard_preserves_loaded_gunner_area_fire_selector() {
    let mut app = world();
    // Durable passive victim survives incidental old-source pulses aimed at the zero-
    // Verses competitor, so the explicit legality positive remains a real living target.
    app.world_mut().insert_resource(combat_variant(&[(
        "(unit: \"conscript\", hit_points: 125,",
        "(unit: \"conscript\", hit_points: 10000,",
    )]));
    let guard = put(&mut app, P0, "ifv", 10, 10);
    let passenger = put(&mut app, P0, "yuri", 9, 10);
    assert!(rejected(&interact(&mut app, passenger, guard)).is_empty());
    assert_eq!(cargo(&app, guard), [passenger]);
    let near = put(&mut app, P1, "rhino", 11, 10);
    let legal = put(&mut app, P1, "conscript", 10, 11);
    disarm(&mut app, near);
    disarm(&mut app, legal);
    let weapon = defs(&app).weapon_kind("cr_mind_control").unwrap();
    require_shot(&mut app, guard, legal, Some(weapon));
}

#[test]
fn ra9_guard_skips_minimum_range_target_without_reachable_firing_cell() {
    let mut app = world();
    let mut terrain = vec![
        Cell {
            passable: false,
            water: false,
            buildable: false,
            ore: 0
        };
        48 * 48
    ];
    for y in 9..=11 {
        for x in 9..=11 {
            terrain[y * 48 + x] = OPEN;
        }
    }
    terrain[10 * 48 + 12] = OPEN;
    terrain[10 * 48 + 16] = OPEN;
    app.world_mut()
        .insert_resource(Grid::from_cells(48, 48, terrain).unwrap());
    let guard = put(&mut app, P0, "v3_launcher", 10, 10);
    let near = put(&mut app, P1, "rhino", 12, 10);
    let legal = put(&mut app, P1, "rhino", 16, 10);
    disarm(&mut app, near);
    disarm(&mut app, legal);
    let d = defs(&app);
    let w = d.weapon(d.weapon_kind("v3_launcher").unwrap()).unwrap();
    assert_eq!(w.min_range, 5 * 256);
    for y in 9..=11 {
        for x in 9..=12 {
            // Every passable cell connected to source is closer than minimum range to nearer
            // threat. The farther target is in native range from the current source cell.
            if app
                .world()
                .resource::<Grid>()
                .cell(CellPos::new(x, y))
                .unwrap()
                .passable
            {
                assert!(((x - 12_i32).pow(2) + (y - 10_i32).pow(2)) < 25);
            }
        }
    }
    require_shot(&mut app, guard, legal, None);
}
