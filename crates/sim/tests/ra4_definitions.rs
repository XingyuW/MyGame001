//! RA4: the arsenal file of schema version 5, the combat file of schema version 3, and what the
//! definitions derive from them (docs/RA2_PARITY_PLAN.md section 5, RA4; docs/DECISIONS.md R46).
//!
//! The traits of veterancy, crews, aircraft, spawned units and submerged units: loaded from the
//! RA4 fixture (`tests/fixtures/data/ra4`; invariant I10), rejected outside their ranges, on an
//! item of a kind that cannot have them, and in combinations no rule can serve, and hashed. That
//! every field is required, that unknown fields are rejected and that a file of another schema
//! version is rejected as such is tested with the other files in `ra1_definitions.rs` and
//! `m4_definitions.rs`.

mod ra4_common;

use std::path::PathBuf;

use ra4_common::*;
use sim::{
    ARSENAL_SCHEMA_VERSION, Ability, COMBAT_SCHEMA_VERSION, DataErrorKind, Definitions, Effect,
    Launcher, MAX_AMMO, MAX_DOCKS, MAX_SIGHT, MAX_SPAWN_TICKS, MAX_SPAWNS, Missile, ProductionItem,
    load_match_definitions,
};

/// The definitions of the fixture up to the rules, onto which an arsenal is loaded.
fn context() -> Definitions {
    load_match_definitions(
        fixture("units.ron"),
        fixture("economy.ron"),
        fixture("combat.ron"),
    )
    .and_then(|definitions| definitions.with_rules(fixture("rules.ron")))
    .unwrap_or_else(|error| panic!("{error}"))
}

/// The arsenal fixture with each `(from, to)` replaced in the line of the entry `id`, written
/// under `name`; and the index of that entry.
fn variant(name: &str, id: &str, edits: &[(&str, &str)]) -> (PathBuf, usize) {
    let text = std::fs::read_to_string(fixture("arsenal.ron")).expect("the fixture");
    let marker = format!("(id: {id:?},");
    let mut index = None;
    let mut entries = 0;
    let mut lines = Vec::new();
    for line in text.lines() {
        let mut line = line.to_owned();
        if line.trim_start().starts_with("(id: ") {
            if line.contains(&marker) {
                for (from, to) in edits {
                    assert_eq!(line.matches(from).count(), 1, "{name}: {from:?} in {id}");
                    line = line.replace(from, to);
                }
                index = Some(entries);
            }
            entries += 1;
        }
        lines.push(line);
    }
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ra4_definitions");
    std::fs::create_dir_all(&dir).expect("temporary directory");
    let path = dir.join(format!("{name}.ron"));
    std::fs::write(&path, lines.join("\n") + "\n").expect("temporary file");
    (
        path,
        index.unwrap_or_else(|| panic!("{name}: no entry {id}")),
    )
}

/// The arsenal with the edits is rejected at `field` of the entry with the value `found`.
fn assert_rejected(name: &str, id: &str, edits: &[(&str, &str)], field: &str, found: &str) {
    let (path, entry) = variant(name, id, edits);
    let error = context()
        .with_arsenal(&path)
        .expect_err("the file must be rejected");
    assert_eq!(error.file, path, "{name}");
    assert_eq!(
        error.field,
        format!("entries[{entry}].{field}"),
        "{name}: {error}"
    );
    assert_eq!(
        error.kind,
        DataErrorKind::InvalidValue {
            found: found.into(),
            expected: "a supported arsenal value".into()
        },
        "{name}: {error}"
    );
}

fn accepted(name: &str, id: &str, edits: &[(&str, &str)]) -> Definitions {
    let (path, _) = variant(name, id, edits);
    context()
        .with_arsenal(&path)
        .unwrap_or_else(|error| panic!("{name}: {error}"))
}

// --- Positive -----------------------------------------------------------------------------------

#[test]
fn the_fixture_loads_with_the_traits_of_schema_version_5() {
    assert_eq!(ARSENAL_SCHEMA_VERSION, 5);
    assert_eq!(COMBAT_SCHEMA_VERSION, 3);
    let definitions = definitions();
    let kind = |id: &str| definitions.unit_kind(id).unwrap_or_else(|| panic!("{id}"));
    let unit = |id: &str| {
        definitions
            .metadata(ProductionItem::Unit(kind(id)))
            .unwrap_or_else(|| panic!("{id}"))
            .clone()
    };
    let building = |id: &str| {
        let kind = definitions
            .building_kind(id)
            .unwrap_or_else(|| panic!("{id}"));
        definitions
            .metadata(ProductionItem::Building(kind))
            .unwrap_or_else(|| panic!("{id}"))
            .clone()
    };
    use Ability::*;
    // Veterancy.
    let tank = unit("tank");
    assert!(tank.trainable && definitions.trainable(kind("tank")));
    assert_eq!(
        tank.veteran_abilities,
        [Stronger, Firepower, Rof, Sight, Faster]
    );
    assert_eq!(tank.elite_abilities, [SelfHeal, Stronger, Firepower, Rof]);
    assert!(!unit("conscript").trainable && !definitions.trainable(kind("conscript")));
    assert!(unit("raider").trainable && unit("raider").veteran_abilities.is_empty());
    // A rank has the abilities of its list and of the ranks below, each once.
    for (rank, ability, has) in [
        (0, Firepower, false),
        (1, Firepower, true),
        (2, Firepower, true),
        (1, SelfHeal, false),
        (2, SelfHeal, true),
        (2, Scatter, false),
    ] {
        assert_eq!(
            definitions.has_ability(kind("tank"), rank, ability),
            has,
            "tank at rank {rank}: {ability:?}"
        );
    }
    assert!(definitions.has_ability(kind("heavy"), 1, Firepower));
    assert!(!definitions.has_ability(kind("heavy"), 1, Stronger));
    assert!(definitions.has_ability(kind("heavy"), 2, Stronger));
    // Crews and self healing.
    for id in ["allied_mcv", "soviet_mcv"] {
        assert!(unit(id).crewed, "{id}");
    }
    for id in [
        "allied_yard",
        "soviet_barracks",
        "allied_factory",
        "airfield",
    ] {
        assert!(building(id).crewed, "{id}");
    }
    assert!(!building("turret").crewed && !unit("tank").crewed);
    assert!(unit("seer").self_healing && !unit("tank").self_healing);
    // Aircraft and docks.
    let striker = unit("striker");
    assert!(striker.airport_bound && striker.ammo == 2);
    assert!(!unit("gunship").airport_bound && unit("gunship").ammo == 0);
    assert_eq!(building("airfield").docks, 2);
    assert_eq!(building("allied_factory").docks, 0);
    // Spawners, what they spawn, and the missile.
    let launcher = unit("launcher");
    assert_eq!(launcher.spawns, Some(kind("rocket")));
    assert_eq!(
        (
            launcher.spawns_number,
            launcher.spawn_regen,
            launcher.spawn_reload
        ),
        (1, 200, 0)
    );
    assert_eq!(launcher.launcher, Some(Launcher::Primary));
    assert_eq!(unit("rocket").missile, Some(Missile::V3));
    assert_eq!(unit("rocket").ammo, 1);
    let carrier = unit("carrier");
    assert_eq!(carrier.spawns, Some(kind("drone")));
    assert_eq!(
        (
            carrier.spawns_number,
            carrier.spawn_regen,
            carrier.spawn_reload
        ),
        (3, 400, 100)
    );
    let boat = unit("boat");
    assert_eq!(boat.spawns, Some(kind("hunter")));
    assert_eq!(boat.launcher, Some(Launcher::Secondary));
    assert_eq!(
        definitions.spawned_kinds().into_iter().collect::<Vec<_>>(),
        {
            let mut kinds = vec![kind("rocket"), kind("drone"), kind("hunter")];
            kinds.sort();
            kinds
        }
    );
    assert!(unit("tank").spawns.is_none() && unit("tank").launcher.is_none());
    // The weapons that launch: the launcher's and the carrier's primary, the boat's secondary.
    let launchers: Vec<String> = definitions
        .launcher_weapons()
        .into_iter()
        .map(|kind| definitions.weapon(kind).expect("weapon").id.clone())
        .collect();
    assert_eq!(launchers, ["launch", "launch_wing", "launch_hunter"]);
    // Cloaking and sensors.
    assert!(unit("sub").cloakable && unit("squid").cloakable && !unit("boat").cloakable);
    assert_eq!(unit("boat").sensors_sight, 5);
    assert_eq!(unit("sub").sensors_sight, 2);
    assert_eq!(unit("tank").sensors_sight, 0);
    // The missile's effects of the combat file, and those of the elite launcher.
    let effect = |effect: Effect| {
        let found = definitions.effect(effect).expect("the effect");
        (
            found.damage,
            definitions
                .warhead(found.warhead)
                .expect("warhead")
                .id
                .clone(),
        )
    };
    assert_eq!(effect(Missile::V3.effect(false)), (150, "blast".to_owned()));
    assert_eq!(effect(Missile::V3.effect(true)), (300, "blast".to_owned()));
    assert_eq!(Missile::DMisl.effect(false), Effect::DMisl);
    assert_eq!(Missile::DMisl.effect(true), Effect::DMislElite);
    assert!(definitions.effect(Effect::DMisl).is_none());
    // Without a rules file no type is trainable and nothing is spawned.
    let bare = load_match_definitions(
        fixture("units.ron"),
        fixture("economy.ron"),
        fixture("combat.ron"),
    )
    .expect("the fixture");
    assert!(!bare.trainable(kind("tank")) && bare.spawned_kinds().is_empty());
    assert!(bare.launcher_weapons().is_empty());
}

// --- Negative and boundary --------------------------------------------------------------------

#[test]
fn flags_outside_0_and_1_and_numbers_outside_their_ranges_are_rejected() {
    for (field, id) in [
        ("trainable", "raider"),
        ("crewed", "allied_mcv"),
        ("self_healing", "seer"),
        ("airport_bound", "striker"),
        ("cloakable", "sub"),
    ] {
        for bad in ["2", "-1"] {
            assert_rejected(
                &format!("{field}_{bad}"),
                id,
                &[(&format!("{field}: 1"), &format!("{field}: {bad}"))],
                field,
                bad,
            );
        }
    }
    let over = |max: u32| (max + 1).to_string();
    for (field, id, from, bad) in [
        ("ammo", "striker", "ammo: 2", over(MAX_AMMO)),
        ("ammo", "striker", "ammo: 2", "-1".to_owned()),
        ("docks", "airfield", "docks: 2", over(MAX_DOCKS)),
        ("docks", "airfield", "docks: 2", "-1".to_owned()),
        (
            "spawns_number",
            "carrier",
            "spawns_number: 3",
            over(MAX_SPAWNS),
        ),
        (
            "spawn_regen",
            "carrier",
            "spawn_regen: 400",
            over(MAX_SPAWN_TICKS),
        ),
        (
            "spawn_reload",
            "carrier",
            "spawn_reload: 100",
            over(MAX_SPAWN_TICKS),
        ),
        (
            "spawn_reload",
            "carrier",
            "spawn_reload: 100",
            "-1".to_owned(),
        ),
        ("sensors_sight", "boat", "sensors_sight: 5", over(MAX_SIGHT)),
        ("sensors_sight", "boat", "sensors_sight: 5", "-1".to_owned()),
    ] {
        let to = format!("{field}: {bad}");
        assert_rejected(&format!("{field}_{bad}"), id, &[(from, &to)], field, &bad);
    }
    // The limits themselves load.
    let definitions = accepted(
        "limits",
        "carrier",
        &[
            ("spawns_number: 3", &format!("spawns_number: {MAX_SPAWNS}")),
            (
                "spawn_regen: 400",
                &format!("spawn_regen: {MAX_SPAWN_TICKS}"),
            ),
            (
                "spawn_reload: 100",
                &format!("spawn_reload: {MAX_SPAWN_TICKS}"),
            ),
            ("sensors_sight: 0", &format!("sensors_sight: {MAX_SIGHT}")),
        ],
    );
    let carrier = definitions
        .metadata(ProductionItem::Unit(
            definitions.unit_kind("carrier").expect("carrier"),
        ))
        .expect("entry");
    assert_eq!(carrier.spawns_number, MAX_SPAWNS);
    assert_eq!(carrier.spawn_regen, MAX_SPAWN_TICKS);
    assert_eq!(carrier.sensors_sight, MAX_SIGHT);
    accepted(
        "ammo_limit",
        "striker",
        &[("ammo: 2", &format!("ammo: {MAX_AMMO}"))],
    );
}

#[test]
fn abilities_must_be_known_and_listed_once() {
    let (path, entry) = variant(
        "ability_unknown",
        "heavy",
        &[(
            r#"veteran_abilities: ["firepower"]"#,
            r#"veteran_abilities: ["cloak"]"#,
        )],
    );
    let error = context().with_arsenal(&path).expect_err("rejected");
    assert_eq!(
        error.field,
        format!("entries[{entry}].veteran_abilities[0]")
    );
    assert!(
        matches!(&error.kind, DataErrorKind::InvalidValue { found, .. } if found == "\"cloak\""),
        "{error}"
    );
    let (path, entry) = variant(
        "ability_twice",
        "heavy",
        &[(
            r#"elite_abilities: ["stronger"]"#,
            r#"elite_abilities: ["stronger", "rof", "stronger"]"#,
        )],
    );
    let error = context().with_arsenal(&path).expect_err("rejected");
    assert_eq!(error.field, format!("entries[{entry}].elite_abilities[2]"));
    assert_eq!(error.kind, DataErrorKind::DuplicateId("stronger".into()));
    // The same ability in both lists is what the shipped data has: it applies once.
    accepted(
        "ability_in_both",
        "heavy",
        &[(
            r#"elite_abilities: ["stronger"]"#,
            r#"elite_abilities: ["stronger", "firepower"]"#,
        )],
    );
}

#[test]
fn a_trait_of_a_unit_on_a_building_and_docks_on_a_unit_are_rejected() {
    let on_a_building = "a unit trait on a building type";
    for (field, from, to) in [
        ("trainable", "trainable: 0", "trainable: 1"),
        (
            "veteran_abilities",
            "veteran_abilities: []",
            r#"veteran_abilities: ["rof"]"#,
        ),
        (
            "elite_abilities",
            "elite_abilities: []",
            r#"elite_abilities: ["rof"]"#,
        ),
        ("self_healing", "self_healing: 0", "self_healing: 1"),
        ("ammo", "ammo: 0", "ammo: 1"),
        ("cloakable", "cloakable: 0", "cloakable: 1"),
        ("spawns", r#"spawns: "none""#, r#"spawns: "drone""#),
        ("spawns", r#"launcher: "none""#, r#"launcher: "primary""#),
        ("spawns", r#"missile: "none""#, r#"missile: "v3""#),
    ] {
        assert_rejected(
            &format!("building_{field}_{}", to.len()),
            "allied_power",
            &[(from, to)],
            field,
            on_a_building,
        );
    }
    // A building may be crewed and may have sensors.
    accepted(
        "building_crewed",
        "allied_power",
        &[
            ("crewed: 0", "crewed: 1"),
            ("sensors_sight: 0", "sensors_sight: 4"),
        ],
    );
    assert_rejected(
        "unit_docks",
        "tank",
        &[("docks: 0", "docks: 1")],
        "docks",
        "a building trait on a unit type",
    );
    // An airfield of 2 x 2 cells has at most four docks.
    accepted("docks_4", "airfield", &[("docks: 2", "docks: 4")]);
    assert_rejected(
        "docks_5",
        "airfield",
        &[("docks: 2", "docks: 5")],
        "docks",
        "more docks than cells of the footprint",
    );
}

#[test]
fn an_airport_bound_unit_is_an_aircraft_with_ammunition() {
    let reason = "an airport bound unit that is no aircraft with ammunition";
    assert_rejected(
        "bound_without_ammo",
        "striker",
        &[("ammo: 2", "ammo: 0")],
        "airport_bound",
        reason,
    );
    assert_rejected(
        "bound_on_the_ground",
        "tank",
        &[
            ("airport_bound: 0", "airport_bound: 1"),
            ("ammo: 0", "ammo: 1"),
        ],
        "airport_bound",
        reason,
    );
}

#[test]
fn the_terms_of_a_spawner_must_fit_together() {
    // A spawned type must be defined, must not be the type itself, and comes with a number, a
    // time to replace it and a launcher.
    let (path, entry) = variant(
        "spawns_unknown",
        "carrier",
        &[(r#"spawns: "drone""#, r#"spawns: "kite""#)],
    );
    let error = context().with_arsenal(&path).expect_err("rejected");
    assert_eq!(error.field, format!("entries[{entry}].spawns"));
    assert_eq!(
        error.kind,
        DataErrorKind::UnknownReference {
            kind: "unit".into(),
            id: "kite".into()
        }
    );
    assert_rejected(
        "spawns_itself",
        "carrier",
        &[(r#"spawns: "drone""#, r#"spawns: "carrier""#)],
        "spawns",
        "a unit that spawns its own type",
    );
    assert_rejected(
        "spawns_nobody",
        "carrier",
        &[("spawns_number: 3", "spawns_number: 0")],
        "spawns_number",
        "a spawner of no units",
    );
    assert_rejected(
        "spawns_never_again",
        "carrier",
        &[("spawn_regen: 400", "spawn_regen: 0")],
        "spawn_regen",
        "a spawner that never replaces a unit",
    );
    assert_rejected(
        "spawns_without_launcher",
        "carrier",
        &[(r#"launcher: "primary""#, r#"launcher: "none""#)],
        "launcher",
        "a spawner without a launcher",
    );
    assert_rejected(
        "launcher_unknown",
        "carrier",
        &[(r#"launcher: "primary""#, r#"launcher: "tertiary""#)],
        "launcher",
        "tertiary",
    );
    // Without a spawned type there are no terms and no launcher.
    for (name, from, to, field, found) in [
        (
            "number",
            "spawns_number: 0",
            "spawns_number: 1",
            "spawns",
            "spawn terms without a spawned type",
        ),
        (
            "regen",
            "spawn_regen: 0",
            "spawn_regen: 1",
            "spawns",
            "spawn terms without a spawned type",
        ),
        (
            "reload",
            "spawn_reload: 0",
            "spawn_reload: 1",
            "spawns",
            "spawn terms without a spawned type",
        ),
        (
            "launcher",
            r#"launcher: "none""#,
            r#"launcher: "secondary""#,
            "launcher",
            "a launcher that spawns nothing",
        ),
    ] {
        assert_rejected(
            &format!("no_spawn_{name}"),
            "tank",
            &[(from, to)],
            field,
            found,
        );
    }
    // The spawned type is an aircraft of the arsenal that spawns nothing itself.
    for (name, to) in [("ground", "tank"), ("chain", "carrier")] {
        let edits = [(r#"spawns: "rocket""#, format!(r#"spawns: "{to}""#))];
        let edits: Vec<(&str, &str)> = edits.iter().map(|(a, b)| (*a, b.as_str())).collect();
        let (path, _) = variant(&format!("spawned_{name}"), "launcher", &edits);
        let error = context().with_arsenal(&path).expect_err("rejected");
        assert_eq!(error.field, "entries", "{name}: {error}");
        assert!(
            matches!(&error.kind, DataErrorKind::InvalidValue { expected, .. }
                if expected.contains("an aircraft of the arsenal")),
            "{name}: {error}"
        );
    }
    // A missile flies.
    assert_rejected(
        "missile_unknown",
        "rocket",
        &[(r#"missile: "v3""#, r#"missile: "v4""#)],
        "missile",
        "v4",
    );
    assert_rejected(
        "missile_on_the_ground",
        "tank",
        &[(r#"missile: "none""#, r#"missile: "dmisl""#)],
        "missile",
        "a missile that does not fly",
    );
    accepted(
        "missile_dmisl",
        "rocket",
        &[(r#"missile: "v3""#, r#"missile: "dmisl""#)],
    );
}

// --- Regression: hashing (invariant I3) ---------------------------------------------------------

#[test]
fn every_trait_of_schema_version_5_is_hashed() {
    let hash = |definitions: Definitions| {
        let mut app = world_of(definitions, 1);
        sim::state_hash(app.world_mut())
    };
    let base = hash(definitions());
    assert_eq!(base, hash(definitions()));
    for (name, id, from, to) in [
        ("trainable", "raider", "trainable: 1", "trainable: 0"),
        (
            "veteran",
            "heavy",
            r#"veteran_abilities: ["firepower"]"#,
            r#"veteran_abilities: ["rof"]"#,
        ),
        (
            "elite",
            "heavy",
            r#"elite_abilities: ["stronger"]"#,
            r#"elite_abilities: ["rof"]"#,
        ),
        ("crewed", "allied_mcv", "crewed: 1", "crewed: 0"),
        ("self_healing", "seer", "self_healing: 1", "self_healing: 0"),
        ("ammo", "striker", "ammo: 2", "ammo: 3"),
        ("docks", "airfield", "docks: 2", "docks: 3"),
        ("number", "carrier", "spawns_number: 3", "spawns_number: 2"),
        ("regen", "carrier", "spawn_regen: 400", "spawn_regen: 401"),
        ("reload", "carrier", "spawn_reload: 100", "spawn_reload: 99"),
        (
            "spawns",
            "carrier",
            r#"spawns: "drone""#,
            r#"spawns: "hunter""#,
        ),
        (
            "launcher",
            "carrier",
            r#"launcher: "primary""#,
            r#"launcher: "secondary""#,
        ),
        (
            "missile",
            "rocket",
            r#"missile: "v3""#,
            r#"missile: "dmisl""#,
        ),
        ("cloakable", "sub", "cloakable: 1", "cloakable: 0"),
        ("sensors", "boat", "sensors_sight: 5", "sensors_sight: 6"),
    ] {
        let changed = hash(accepted(&format!("hash_{name}"), id, &[(from, to)]));
        assert_ne!(changed, base, "{name} is not hashed");
    }
    // An airport bound aircraft keeps its docks and its ammunition; without the flag the
    // file is another.
    let free = accepted(
        "hash_bound",
        "striker",
        &[("airport_bound: 1", "airport_bound: 0")],
    );
    assert_ne!(hash(free), base, "airport_bound is not hashed");
}

/// The effects of the combat file's schema version 3 are checked like the others: each at most
/// once, with a defined warhead.
#[test]
fn the_effects_of_the_missiles_are_checked_like_every_effect() {
    let text = std::fs::read_to_string(fixture("combat.ron")).expect("fixture");
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ra4_definitions");
    std::fs::create_dir_all(&dir).expect("temporary directory");
    let load = |name: &str, from: &str, to: &str| {
        assert_eq!(text.matches(from).count(), 1, "{name}");
        let path = dir.join(format!("combat_{name}.ron"));
        std::fs::write(&path, text.replacen(from, to, 1)).expect("variant");
        load_match_definitions(fixture("units.ron"), fixture("economy.ron"), &path)
    };
    let line = r#"(id: "v3_rocket_elite", damage: 300, warhead: "blast"),"#;
    let both = load(
        "dmisl",
        line,
        r#"(id: "dmisl", damage: 200, warhead: "shell"), (id: "dmisl_elite", damage: 400, warhead: "megaton"),"#,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        both.effect(Effect::DMisl).map(|effect| effect.damage),
        Some(200)
    );
    assert_eq!(
        both.effect(Effect::DMislElite).map(|effect| effect.damage),
        Some(400)
    );
    assert!(both.effect(Effect::V3RocketElite).is_none());
    let error = load(
        "twice",
        line,
        r#"(id: "v3_rocket", damage: 300, warhead: "blast"),"#,
    )
    .expect_err("an effect listed twice");
    assert_eq!(error.field, "effects[6].id", "{error}");
    let error = load(
        "warhead",
        line,
        r#"(id: "v3_rocket_elite", damage: 300, warhead: "plasma"),"#,
    )
    .expect_err("an unknown warhead");
    assert_eq!(error.field, "effects[6].warhead", "{error}");
}
