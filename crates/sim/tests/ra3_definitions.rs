//! RA3: the arsenal file of schema version 4 and what the definitions derive from it
//! (docs/RA2_PARITY_PLAN.md section 5, RA3 items 6 and 8; docs/DECISIONS.md R6).
//!
//! The traits of a unit that movement reads (`crusher`, `crushable`, `size`, `infantry`):
//! loaded from the RA3 fixture (`tests/fixtures/data/ra3`; invariant I10), rejected outside
//! their ranges, on a building, and on a unit of a kind that cannot have them, and hashed. That
//! every field is required, that unknown fields are rejected and that a file of another schema
//! version is rejected as such is tested with the other files in `ra1_definitions.rs`.

mod ra3_common;

use std::path::PathBuf;

use ra3_common::*;
use sim::{
    ARSENAL_SCHEMA_VERSION, DataErrorKind, Definitions, Domain, MAX_UNIT_SIZE, ProductionItem,
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
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ra3_definitions");
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

// --- Positive -----------------------------------------------------------------------------------

#[test]
fn the_fixture_loads_with_the_traits_of_schema_version_4() {
    assert_eq!(ARSENAL_SCHEMA_VERSION, 5);
    let definitions = definitions();
    let kind = |id: &str| definitions.unit_kind(id).unwrap_or_else(|| panic!("{id}"));
    let entry = |id: &str| {
        definitions
            .metadata(ProductionItem::Unit(kind(id)))
            .unwrap_or_else(|| panic!("{id}"))
            .clone()
    };
    // (id, infantry, on foot, crushable, crusher, size)
    for (id, infantry, on_foot, crushable, crusher, size) in [
        ("recruit", true, true, true, false, 1),
        ("trooper", true, true, true, false, 1),
        ("hazmat", true, true, false, false, 1),
        ("jumper", true, false, false, false, 1),
        ("truck", false, false, false, false, 3),
        ("roller", false, false, false, true, 3),
        ("brute", false, false, false, true, 6),
        // The types of RA3-A have none of the traits: one unit to a cell, as before.
        ("rifleman", false, false, false, false, 0),
        ("engineer", false, false, false, false, 0),
        ("tank", false, false, false, false, 0),
        ("gunship", false, false, false, false, 0),
    ] {
        let found = entry(id);
        assert_eq!(found.infantry, infantry, "{id}: infantry");
        assert_eq!(definitions.on_foot(kind(id)), on_foot, "{id}: on foot");
        assert_eq!(
            definitions.crushable(kind(id)),
            crushable,
            "{id}: crushable"
        );
        assert_eq!(definitions.crusher(kind(id)), crusher, "{id}: crusher");
        assert_eq!(found.crusher, crusher, "{id}: crusher");
        assert_eq!(found.size, size, "{id}: size");
    }
    // The jumper is crushable by its data and flies: it is never on foot, so never crushed.
    assert!(entry("jumper").crushable && entry("jumper").domain == Domain::Air);
    assert!(definitions.has_crushers());
    // No building has a trait of a unit.
    for (item, entry) in &definitions.arsenal().0 {
        if matches!(item, ProductionItem::Building(_)) {
            assert!(
                !entry.infantry && !entry.crusher && !entry.crushable && entry.size == 0,
                "{}",
                entry.label
            );
        }
    }
}

#[test]
fn the_traits_load_at_the_ends_of_their_ranges() {
    assert_eq!(MAX_UNIT_SIZE, 255);
    for (name, id, from, to, size) in [
        ("size_none", "truck", "size: 3", "size: 0", 0),
        ("size_most", "truck", "size: 3", "size: 255", MAX_UNIT_SIZE),
    ] {
        let (path, _) = variant(name, id, &[(from, to)]);
        let loaded = context()
            .with_arsenal(&path)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let kind = loaded.unit_kind(id).expect(id);
        assert_eq!(
            loaded.metadata(ProductionItem::Unit(kind)).expect(id).size,
            size
        );
    }
    // A crusher on water and an amphibious one are surface vehicles (the Soviet transport).
    for (name, id) in [
        ("crusher_boat", "boat"),
        ("crusher_hovercraft", "hovercraft"),
    ] {
        let (path, _) = variant(name, id, &[("crusher: 0", "crusher: 1")]);
        let loaded = context()
            .with_arsenal(&path)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(loaded.crusher(loaded.unit_kind(id).expect(id)), "{name}");
    }
    // Infantry of the older kind in the fixture may be given the trait: the rifleman on foot.
    let (path, _) = variant(
        "rifleman_on_foot",
        "rifleman",
        &[
            ("infantry: 0", "infantry: 1"),
            ("crushable: 0", "crushable: 1"),
        ],
    );
    let loaded = context().with_arsenal(&path).expect("loads");
    let rifleman = loaded.unit_kind("rifleman").expect("rifleman");
    assert!(loaded.on_foot(rifleman) && loaded.crushable(rifleman));
}

// --- Negative -----------------------------------------------------------------------------------

#[test]
fn traits_of_a_unit_outside_their_ranges_are_rejected() {
    let cases = [
        ("crusher", "roller", "crusher: 1", "2"),
        ("crusher", "roller", "crusher: 1", "-1"),
        ("crushable", "recruit", "crushable: 1", "2"),
        ("crushable", "recruit", "crushable: 1", "-1"),
        ("size", "truck", "size: 3", "256"),
        ("size", "truck", "size: 3", "-1"),
        ("infantry", "recruit", "infantry: 1", "2"),
        ("infantry", "truck", "infantry: 0", "-1"),
    ];
    for (index, (field, id, from, value)) in cases.into_iter().enumerate() {
        let to = from.replace(|c: char| c.is_ascii_digit(), "");
        let to = format!("{to}{value}");
        assert_rejected(&format!("range_{index}"), id, &[(from, &to)], field, value);
    }
}

#[test]
fn a_trait_of_a_unit_on_a_building_is_rejected() {
    for (field, from, to) in [
        ("crusher", "crusher: 0", "crusher: 1"),
        ("crushable", "crushable: 0", "crushable: 1"),
        ("size", "size: 0", "size: 1"),
        ("infantry", "infantry: 0", "infantry: 1"),
    ] {
        assert_rejected(
            &format!("building_{field}"),
            "depot",
            &[(from, to)],
            field,
            "a unit trait on a building type",
        );
    }
}

#[test]
fn a_trait_on_a_unit_that_cannot_have_it_is_rejected() {
    // Infantry is of the category infantry.
    assert_rejected(
        "infantry_vehicle",
        "truck",
        &[("infantry: 0", "infantry: 1")],
        "infantry",
        "the infantry trait outside the category infantry",
    );
    // Only infantry is crushable.
    for id in ["truck", "rifleman"] {
        assert_rejected(
            &format!("crushable_{id}"),
            id,
            &[("crushable: 0", "crushable: 1")],
            "crushable",
            "a crushable unit that is not infantry",
        );
    }
    // A crusher is a vehicle, and not an aircraft.
    for id in ["recruit", "rifleman", "gunship"] {
        assert_rejected(
            &format!("crusher_{id}"),
            id,
            &[("crusher: 0", "crusher: 1")],
            "crusher",
            "a crusher that is not a vehicle on the surface",
        );
    }
}

// --- Regression: the traits are part of the hashed definitions ------------------------------------

#[test]
fn every_trait_of_a_unit_changes_the_state_hash() {
    let hash = |definitions: Definitions| {
        let mut app = world_of(definitions, 0);
        sim::state_hash(app.world_mut())
    };
    let plain = hash(definitions());
    assert_eq!(plain, hash(definitions()));
    for (index, (id, edits)) in [
        ("truck", vec![("crusher: 0", "crusher: 1")]),
        ("recruit", vec![("crushable: 1", "crushable: 0")]),
        ("truck", vec![("size: 3", "size: 4")]),
        // The jumper without the trait may not be crushable either.
        (
            "jumper",
            vec![
                ("infantry: 1", "infantry: 0"),
                ("crushable: 1", "crushable: 0"),
            ],
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let (path, _) = variant(&format!("hash_{index}"), id, &edits);
        let changed = context().with_arsenal(&path).expect("loads");
        assert_ne!(plain, hash(changed), "{id}: {edits:?}");
    }
    // One trait alone: the hazmat is infantry that is not crushable; as a plain unit it differs
    // in nothing but the trait.
    let (path, _) = variant("hash_infantry", "hazmat", &[("infantry: 1", "infantry: 0")]);
    let changed = context().with_arsenal(&path).expect("loads");
    assert_ne!(plain, hash(changed));
}
