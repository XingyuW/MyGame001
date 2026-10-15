//! RA2: the arsenal file of schema version 3 and what the definitions derive from it
//! (docs/RA2_PARITY_PLAN.md section 5, RA2 item 4; docs/DECISIONS.md R22).
//!
//! The traits of a building that production, power, placement, selling and repair read
//! (`build_limit`, `adjacent`, `base_normal`, `radar`, `sellable`, `repairable`, `powered`,
//! `is_powered`, `unit_repair`): loaded from the RA2 fixture (`tests/fixtures/data/ra2`;
//! invariant I10), rejected outside their ranges and on a unit, and hashed. That every field is
//! required, that unknown fields are rejected and that a file of schema version 2 is rejected
//! as such is tested with the other files in `ra1_definitions.rs`.

mod ra2_common;

use std::path::PathBuf;

use ra2_common::*;
use sim::{
    ARSENAL_SCHEMA_VERSION, Category, DataErrorKind, Definitions, Lane, MAX_BUILD_LIMIT,
    MAX_PLACEMENT_DISTANCE, MAX_QUEUE_LENGTH, ProductionItem, QueueKey, extends_base,
    load_match_definitions, placement_reach,
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

/// The arsenal fixture with `from` replaced by `to` in the line of the entry `id`, written
/// under `name`; and the index of that entry.
fn variant(name: &str, id: &str, from: &str, to: &str) -> (PathBuf, usize) {
    let text = std::fs::read_to_string(fixture("arsenal.ron")).expect("the fixture");
    let marker = format!("(id: {id:?},");
    let mut index = None;
    let mut entries = 0;
    let mut lines = Vec::new();
    for line in text.lines() {
        let mut line = line.to_owned();
        if line.trim_start().starts_with("(id: ") {
            if line.contains(&marker) {
                assert_eq!(line.matches(from).count(), 1, "{name}: {from:?} in {id}");
                line = line.replace(from, to);
                index = Some(entries);
            }
            entries += 1;
        }
        lines.push(line);
    }
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ra2_definitions");
    std::fs::create_dir_all(&dir).expect("temporary directory");
    let path = dir.join(format!("{name}.ron"));
    std::fs::write(&path, lines.join("\n") + "\n").expect("temporary file");
    (
        path,
        index.unwrap_or_else(|| panic!("{name}: no entry {id}")),
    )
}

fn building(definitions: &Definitions, id: &str) -> sim::BuildingKind {
    definitions
        .building_kind(id)
        .unwrap_or_else(|| panic!("building {id}"))
}

// --- Positive -----------------------------------------------------------------------------------

#[test]
fn the_fixture_loads_with_the_traits_of_schema_version_3() {
    // The file is at schema version 8 since RA6, which keeps the traits of version 3.
    assert_eq!(ARSENAL_SCHEMA_VERSION, 8);
    let definitions = definitions();
    let entry = |id: &str| {
        definitions
            .metadata(ProductionItem::Building(building(&definitions, id)))
            .unwrap_or_else(|| panic!("{id} has an entry"))
    };
    // (build limit, adjacent, base normal, radar, sellable, repairable, powered, is powered,
    // unit repair)
    let traits = |id: &str| {
        let e = entry(id);
        (
            e.build_limit,
            e.adjacent,
            [
                e.base_normal,
                e.radar,
                e.sellable,
                e.repairable,
                e.powered,
                e.is_powered,
                e.unit_repair,
            ],
        )
    };
    let (y, n) = (true, false);
    assert_eq!(traits("allied_yard"), (0, 2, [y, n, y, y, n, n, n]));
    assert_eq!(traits("allied_radar"), (0, 2, [y, y, y, y, y, n, n]));
    assert_eq!(traits("turret"), (0, 4, [n, n, y, y, y, n, n]));
    assert_eq!(traits("sentry"), (0, 4, [n, n, y, y, y, n, n]));
    assert_eq!(traits("bunker"), (0, 4, [n, n, y, y, n, n, n]));
    assert_eq!(traits("wall"), (0, 8, [n, n, n, n, n, n, n]));
    assert_eq!(traits("depot"), (0, 2, [y, n, y, y, n, n, y]));
    assert_eq!(traits("vault"), (2, 2, [y, n, y, y, n, n, n]));
    assert_eq!(traits("storm_device"), (1, 2, [y, n, y, y, y, y, n]));
    // A unit has none of them.
    let tank = definitions
        .metadata(ProductionItem::Unit(
            definitions.unit_kind("tank").expect("tank"),
        ))
        .expect("entry");
    assert_eq!((tank.build_limit, tank.adjacent), (0, 0));
    assert!(!tank.base_normal && !tank.sellable && !tank.repairable && !tank.radar);
}

#[test]
fn lanes_queues_refunds_and_reach_follow_the_traits_with_a_rules_file() {
    let definitions = definitions();
    let item = |id: &str| ProductionItem::Building(building(&definitions, id));
    let yard = definitions.queue_key(building(&definitions, "allied_yard"));
    // The defences (category armory) are a lane of their own; both yards share both lanes.
    for id in ["bunker", "sentry", "turret", "wall", "storm_device"] {
        assert_eq!(
            definitions.metadata(item(id)).expect("entry").category,
            Category::Armory
        );
        assert_eq!(definitions.lane(item(id)), Lane::Defences, "{id}");
        assert_eq!(
            definitions.queue_of(item(id)),
            Some(QueueKey {
                producer: yard,
                lane: Lane::Defences
            })
        );
    }
    for id in [
        "allied_power",
        "soviet_power",
        "depot",
        "vault",
        "soviet_factory",
    ] {
        assert_eq!(definitions.lane(item(id)), Lane::Main, "{id}");
        assert_eq!(
            definitions.queue_of(item(id)),
            Some(QueueKey {
                producer: yard,
                lane: Lane::Main
            })
        );
        // A lane of buildings holds one item.
        assert_eq!(definitions.queue_capacity(item(id)), 1);
    }
    // A unit is in the main lane of its factory group, which holds 1 + 3 items by the rules.
    let tank = ProductionItem::Unit(definitions.unit_kind("tank").expect("tank"));
    let factory = definitions.queue_key(building(&definitions, "allied_factory"));
    assert_eq!(
        definitions.queue_of(tank),
        Some(QueueKey {
            producer: factory,
            lane: Lane::Main
        })
    );
    assert_eq!(definitions.queue_capacity(tank), 4);
    // 40 percent of the cost, where the economy file's sell value is half of it.
    assert_eq!(
        definitions.refund(building(&definitions, "allied_factory")),
        200
    );
    assert_eq!(definitions.refund(building(&definitions, "wall")), 40);
    // The reach is one more than the free cells; defences and walls are no base buildings.
    for (id, reach, base) in [
        ("allied_power", 3, true),
        ("bunker", 5, false),
        ("wall", 9, false),
        ("depot", 3, true),
    ] {
        assert_eq!(
            placement_reach(&definitions, building(&definitions, id)),
            reach
        );
        assert_eq!(extends_base(&definitions, building(&definitions, id)), base);
    }
}

#[test]
fn without_a_rules_file_the_traits_are_not_read() {
    // The legacy ruleset: an arsenal of schema version 3 and no rules.
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data/legacy");
    let definitions = load_match_definitions(
        dir.join("units.ron"),
        dir.join("economy.ron"),
        dir.join("combat.ron"),
    )
    .and_then(|definitions| definitions.with_arsenal(dir.join("arsenal.ron")))
    .unwrap_or_else(|error| panic!("{error}"));
    let pillbox = building(&definitions, "pillbox");
    let item = ProductionItem::Building(pillbox);
    assert_eq!(
        definitions.metadata(item).expect("entry").category,
        Category::Armory
    );
    // One queue per producer, of the length of M3; the economy file's sell value.
    assert_eq!(definitions.lane(item), Lane::Main);
    assert_eq!(definitions.queue_capacity(item), MAX_QUEUE_LENGTH);
    let def = definitions.building(pillbox).expect("pillbox");
    assert_eq!(definitions.refund(pillbox), def.sell_value);
}

// --- Boundary -----------------------------------------------------------------------------------

#[test]
fn the_limits_of_the_traits_are_accepted() {
    assert_eq!((MAX_BUILD_LIMIT, MAX_PLACEMENT_DISTANCE), (255, 32));
    for (name, id, from, to) in [
        ("limit_max", "vault", "build_limit: 2", "build_limit: 255"),
        ("limit_none", "vault", "build_limit: 2", "build_limit: 0"),
        ("adjacent_max", "wall", "adjacent: 8", "adjacent: 32"),
        ("adjacent_none", "wall", "adjacent: 8", "adjacent: 0"),
    ] {
        let (path, _) = variant(name, id, from, to);
        let loaded = context()
            .with_arsenal(&path)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let entry = loaded
            .metadata(ProductionItem::Building(building(&loaded, id)))
            .expect("entry");
        let value: u32 = to
            .rsplit(' ')
            .next()
            .expect("value")
            .parse()
            .expect("number");
        assert!(
            entry.build_limit == value || entry.adjacent == value,
            "{name}"
        );
    }
    // Touching: no free cell allowed gives a reach of 1.
    let (path, _) = variant("adjacent_touching", "wall", "adjacent: 8", "adjacent: 0");
    let loaded = context().with_arsenal(&path).expect("loads");
    assert_eq!(placement_reach(&loaded, building(&loaded, "wall")), 1);
}

// --- Negative -----------------------------------------------------------------------------------

#[test]
fn traits_outside_their_ranges_are_rejected() {
    let cases = [
        ("build_limit", "vault", "build_limit: 2", "256"),
        ("build_limit", "vault", "build_limit: 2", "-1"),
        ("adjacent", "wall", "adjacent: 8", "33"),
        ("adjacent", "wall", "adjacent: 8", "-1"),
        ("base_normal", "depot", "base_normal: 1", "2"),
        ("base_normal", "depot", "base_normal: 1", "-1"),
        ("radar", "allied_radar", "radar: 1", "2"),
        ("sellable", "wall", "sellable: 0", "2"),
        ("repairable", "wall", "repairable: 0", "-1"),
        ("powered", "sentry", ", powered: 1", "2"),
        ("is_powered", "storm_device", "is_powered: 1", "2"),
        ("unit_repair", "depot", "unit_repair: 1", "2"),
    ];
    for (index, (field, id, from, value)) in cases.into_iter().enumerate() {
        let to = from.replace(|c: char| c.is_ascii_digit(), "");
        let to = format!("{to}{value}");
        let (path, entry) = variant(&format!("range_{index}"), id, from, &to);
        let error = context()
            .with_arsenal(&path)
            .expect_err("the file must be rejected");
        assert_eq!(error.file, path);
        assert_eq!(error.field, format!("entries[{entry}].{field}"), "{error}");
        match &error.kind {
            DataErrorKind::InvalidValue { found, .. } => assert_eq!(found, value, "{error}"),
            other => panic!("expected an invalid value, found {other:?}"),
        }
    }
}

#[test]
fn building_only_traits_on_units_are_rejected_but_unit_build_limits_are_supported() {
    let cases = [
        ("build_limit", "build_limit: 0", "build_limit: 1"),
        ("adjacent", "adjacent: 0", "adjacent: 2"),
        ("base_normal", "base_normal: 0", "base_normal: 1"),
        ("radar", "radar: 0", "radar: 1"),
        ("sellable", "sellable: 0", "sellable: 1"),
        ("repairable", "repairable: 0", "repairable: 1"),
        ("powered", ", powered: 0", ", powered: 1"),
        ("is_powered", "is_powered: 0", "is_powered: 1"),
        ("unit_repair", "unit_repair: 0", "unit_repair: 1"),
    ];
    for (field, from, to) in cases {
        let (path, entry) = variant(&format!("unit_{field}"), "tank", from, to);
        if field == "build_limit" {
            let loaded = context().with_arsenal(&path).expect("RA6 unit limit");
            let k = loaded.unit_kind("tank").unwrap();
            assert_eq!(
                loaded
                    .metadata(ProductionItem::Unit(k))
                    .unwrap()
                    .build_limit,
                1
            );
            continue;
        }

        let error = context()
            .with_arsenal(&path)
            .expect_err("the file must be rejected");
        assert_eq!(error.field, format!("entries[{entry}].{field}"), "{error}");
        assert_eq!(
            error.kind,
            DataErrorKind::InvalidValue {
                found: "a building trait on a unit type".into(),
                expected: "a supported arsenal value".into()
            }
        );
    }
}

// --- Regression: the traits are part of the hashed definitions ------------------------------------

#[test]
fn every_trait_changes_the_state_hash() {
    let hash = |definitions: Definitions| {
        let mut app = world(0);
        app.world_mut().insert_resource(definitions);
        sim::state_hash(app.world_mut())
    };
    let plain = hash(definitions());
    assert_eq!(plain, hash(definitions()));
    for (index, (id, from, to)) in [
        ("vault", "build_limit: 2", "build_limit: 3"),
        ("wall", "adjacent: 8", "adjacent: 7"),
        ("depot", "base_normal: 1", "base_normal: 0"),
        ("depot", "radar: 0", "radar: 1"),
        ("depot", "sellable: 1", "sellable: 0"),
        ("depot", "repairable: 1", "repairable: 0"),
        ("depot", ", powered: 0", ", powered: 1"),
        ("depot", "is_powered: 0", "is_powered: 1"),
        ("depot", "unit_repair: 1", "unit_repair: 0"),
    ]
    .into_iter()
    .enumerate()
    {
        let (path, _) = variant(&format!("hash_{index}"), id, from, to);
        let changed = context().with_arsenal(&path).expect("loads");
        assert_ne!(plain, hash(changed), "{id}: {to}");
    }
}
