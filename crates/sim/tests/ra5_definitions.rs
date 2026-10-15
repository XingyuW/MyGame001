//! RA5: the economy file of schema version 2 (bails, gems, the pace of harvesting, fixed
//! refunds) and the rules file of schema version 2 (the densest ore, the density from which
//! ore spreads, the units of a crate, the contents a crate may hold), on the RA5 fixture
//! (docs/RA2_PARITY_PLAN.md section 5, RA5; docs/DECISIONS.md R58). Negative cases are variants
//! of the fixture's text, as in `m3_definitions.rs` and `ra1_definitions.rs`.

mod ra5_common;

use ra5_common::{definitions, fixture, load, variant};
use sim::{
    BuildingKind, DataErrorKind, Definitions, MAX_CRATE_UNITS, MAX_CREDITS_PER_ORE,
    MAX_HARVEST_INTERVAL, UnitKind,
};

fn building(definitions: &Definitions, id: &str) -> BuildingKind {
    definitions
        .building_kind(id)
        .unwrap_or_else(|| panic!("{id}"))
}

fn unit(definitions: &Definitions, id: &str) -> UnitKind {
    definitions.unit_kind(id).unwrap_or_else(|| panic!("{id}"))
}

/// The error of the fixture with `edits` to its economy file: the field and the kind.
fn economy_error(name: &str, edits: &[(&str, &str)]) -> (String, DataErrorKind) {
    let error = load(variant(name, "economy.ron", edits), fixture("rules.ron"))
        .expect_err(&format!("{name} must be rejected"));
    (error.field, error.kind)
}

/// The error of the fixture with `edits` to its rules file.
fn rules_error(name: &str, edits: &[(&str, &str)]) -> (String, DataErrorKind) {
    let error = load(fixture("economy.ron"), variant(name, "rules.ron", edits))
        .expect_err(&format!("{name} must be rejected"));
    (error.field, error.kind)
}

fn invalid(found: &str, expected: &str) -> DataErrorKind {
    DataErrorKind::InvalidValue {
        found: found.to_owned(),
        expected: expected.to_owned(),
    }
}

const MINER_A: &str = r#"(unit: "miner_a", capacity: 20, harvest_rate: 1, harvest_interval: 4),"#;
const REFINERY_A: &str =
    r#"(building: "allied_refinery", credits_per_ore: 25, credits_per_gem: 50),"#;
const REFUND: &str = r#"(building: "allied_refinery", credits: 90),"#;

// --- Positive -------------------------------------------------------------------------------

#[test]
fn the_fixture_loads_with_bails_gems_the_pace_of_harvesting_and_a_fixed_refund() {
    let definitions = definitions();
    let miner = definitions
        .harvester(unit(&definitions, "miner_a"))
        .expect("a harvester");
    assert_eq!(
        (miner.capacity, miner.harvest_rate, miner.harvest_interval),
        (20, 1, 4)
    );
    let miner = definitions
        .harvester(unit(&definitions, "miner_s"))
        .expect("a harvester");
    assert_eq!(
        (miner.capacity, miner.harvest_rate, miner.harvest_interval),
        (40, 1, 4)
    );
    for id in ["allied_refinery", "soviet_refinery"] {
        let def = definitions
            .building(building(&definitions, id))
            .expect("a building");
        assert_eq!(
            (def.credits_per_ore, def.credits_per_gem),
            (Some(25), Some(50)),
            "{id}"
        );
    }
    // A building that is no refinery pays for neither.
    let power = definitions
        .building(building(&definitions, "allied_power"))
        .expect("a building");
    assert_eq!((power.credits_per_ore, power.credits_per_gem), (None, None));

    // The fixed refund replaces what the rule gives (40 percent of 600), for the listed type
    // alone; the rule's value stays known, for the survivors.
    let allied = building(&definitions, "allied_refinery");
    let soviet = building(&definitions, "soviet_refinery");
    assert_eq!(
        definitions
            .building(allied)
            .expect("a building")
            .fixed_refund,
        Some(90)
    );
    assert_eq!(definitions.refund(allied), 90);
    assert_eq!(definitions.rule_refund(allied), 240);
    assert_eq!(
        definitions
            .building(soviet)
            .expect("a building")
            .fixed_refund,
        None
    );
    assert_eq!(definitions.refund(soviet), 240);
    assert_eq!(definitions.rule_refund(soviet), 240);
}

#[test]
fn the_rules_fixture_loads_with_the_ore_limits_and_the_units_of_a_crate() {
    let definitions = definitions();
    let rules = definitions.rules().expect("rules");
    assert_eq!(
        (rules.economy.ore_max_bails, rules.economy.ore_spread_above),
        (12, 6)
    );
    assert_eq!(
        rules.crate_units,
        ["tank", "heavy", "miner_a"].map(|id| unit(&definitions, id))
    );
    let ids: Vec<&str> = rules
        .powerups
        .iter()
        .map(|powerup| powerup.id.as_str())
        .collect();
    assert_eq!(
        ids,
        sim::CrateContent::ALL.map(sim::CrateContent::id),
        "the fixture lists every content a crate may hold"
    );
    // Every content parses back to itself, and nothing else is a content.
    for content in sim::CrateContent::ALL {
        assert_eq!(sim::CrateContent::parse(content.id()), Some(content));
    }
    assert_eq!(sim::CrateContent::parse("gas"), None);
    assert_eq!(sim::CrateContent::parse(""), None);
}

#[test]
fn definitions_with_other_values_hash_differently() {
    // Invariant I3: every new value of the two files is part of the state hash.
    let hash = |definitions: Definitions| {
        let mut app = ra5_common::world_of(definitions, 1);
        sim::state_hash(app.world_mut())
    };
    let base = hash(definitions());
    assert_eq!(base, hash(definitions()));
    let economy = |name: &str, from: &str, to: &str| {
        hash(
            load(
                variant(name, "economy.ron", &[(from, to)]),
                fixture("rules.ron"),
            )
            .unwrap_or_else(|error| panic!("{error}")),
        )
    };
    let rules =
        |name: &str, from: &str, to: &str| hash(ra5_common::definitions_with(name, &[(from, to)]));
    let others = [
        economy(
            "hash_interval",
            MINER_A,
            r#"(unit: "miner_a", capacity: 20, harvest_rate: 1, harvest_interval: 5),"#,
        ),
        economy(
            "hash_gem",
            REFINERY_A,
            r#"(building: "allied_refinery", credits_per_ore: 25, credits_per_gem: 51),"#,
        ),
        economy(
            "hash_refund",
            REFUND,
            r#"(building: "allied_refinery", credits: 91),"#,
        ),
        economy("hash_no_refund", REFUND, ""),
        rules("hash_max", "ore_max_bails: 12,", "ore_max_bails: 13,"),
        rules(
            "hash_spread",
            "ore_spread_above: 6,",
            "ore_spread_above: 5,",
        ),
        rules(
            "hash_units",
            r#"crate_units: ["tank", "heavy", "miner_a"],"#,
            r#"crate_units: ["tank", "heavy"],"#,
        ),
    ];
    for (index, other) in others.iter().enumerate() {
        assert_ne!(*other, base, "variant {index}");
    }
    let distinct: std::collections::BTreeSet<u64> = others.into_iter().collect();
    assert_eq!(distinct.len(), 7);
}

// --- Negative -------------------------------------------------------------------------------

#[test]
fn the_new_fields_of_the_economy_file_are_required() {
    let cases = [
        (
            MINER_A,
            r#"(unit: "miner_a", capacity: 20, harvest_rate: 1),"#,
            "harvesters[0].harvest_interval",
        ),
        (
            REFINERY_A,
            r#"(building: "allied_refinery", credits_per_ore: 25),"#,
            "refineries[0].credits_per_gem",
        ),
        (
            REFUND,
            r#"(building: "allied_refinery"),"#,
            "refunds[0].credits",
        ),
        (REFUND, "(credits: 90),", "refunds[0].building"),
    ];
    for (index, (from, to, field)) in cases.into_iter().enumerate() {
        assert_eq!(
            economy_error(&format!("missing_{index}"), &[(from, to)]),
            (field.to_owned(), DataErrorKind::MissingField)
        );
    }
    // The list itself.
    let (field, kind) = economy_error(
        "missing_refunds",
        &[(
            "    refunds: [\n        (building: \"allied_refinery\", credits: 90),\n    ],\n",
            "",
        )],
    );
    assert_eq!(
        (field.as_str(), kind),
        ("refunds", DataErrorKind::MissingField)
    );
}

#[test]
fn the_new_values_of_the_economy_file_are_checked() {
    let interval = |value: i64| {
        economy_error(
            &format!("interval_{value}"),
            &[(
                MINER_A,
                &format!(
                    r#"(unit: "miner_a", capacity: 20, harvest_rate: 1, harvest_interval: {value}),"#
                ),
            )],
        )
    };
    let range = format!("1 to {MAX_HARVEST_INTERVAL} ticks");
    for value in [0, -1, i64::from(MAX_HARVEST_INTERVAL) + 1] {
        assert_eq!(
            interval(value),
            (
                "harvesters[0].harvest_interval".to_owned(),
                invalid(&value.to_string(), &range)
            )
        );
    }
    let gem = |value: i64| {
        economy_error(
            &format!("gem_{value}"),
            &[(
                REFINERY_A,
                &format!(
                    r#"(building: "allied_refinery", credits_per_ore: 25, credits_per_gem: {value}),"#
                ),
            )],
        )
    };
    let range = format!("1 to {MAX_CREDITS_PER_ORE} credits per bail");
    for value in [0, MAX_CREDITS_PER_ORE + 1] {
        assert_eq!(
            gem(value),
            (
                "refineries[0].credits_per_gem".to_owned(),
                invalid(&value.to_string(), &range)
            )
        );
    }
    // A refund: of a building of the file, once, from nothing to the cost (600).
    let refund = |name: &str, to: &str| economy_error(name, &[(REFUND, to)]);
    assert_eq!(
        refund("refund_unknown", r#"(building: "palace", credits: 90),"#),
        (
            "refunds[0].building".to_owned(),
            DataErrorKind::UnknownReference {
                kind: "building".to_owned(),
                id: "palace".to_owned()
            }
        )
    );
    assert_eq!(
        refund(
            "refund_twice",
            r#"(building: "allied_refinery", credits: 90), (building: "allied_refinery", credits: 80),"#
        ),
        (
            "refunds[1].building".to_owned(),
            DataErrorKind::DuplicateId("allied_refinery".to_owned())
        )
    );
    for value in [-1, 601] {
        assert_eq!(
            refund(
                &format!("refund_{value}"),
                &format!(r#"(building: "allied_refinery", credits: {value}),"#)
            ),
            (
                "refunds[0].credits".to_owned(),
                invalid(&value.to_string(), "0 to 600 credits (at most the cost)")
            )
        );
    }
    // An unknown field of an entry.
    assert_eq!(
        refund(
            "refund_field",
            r#"(building: "allied_refinery", credits: 90, percent: 1),"#
        ),
        ("refunds[0].percent".to_owned(), DataErrorKind::UnknownField)
    );
}

#[test]
fn files_of_schema_version_1_are_rejected() {
    assert_eq!(
        economy_error(
            "economy_v1",
            &[("schema_version: 2,", "schema_version: 1,")]
        ),
        (
            "schema_version".to_owned(),
            DataErrorKind::UnsupportedSchemaVersion {
                found: 1,
                expected: 2
            }
        )
    );
    assert_eq!(
        rules_error("rules_v1", &[("schema_version: 3,", "schema_version: 1,")]),
        (
            "schema_version".to_owned(),
            DataErrorKind::UnsupportedSchemaVersion {
                found: 1,
                expected: 3
            }
        )
    );
}

#[test]
fn the_new_fields_and_values_of_the_rules_file_are_checked() {
    assert_eq!(
        rules_error("no_max", &[("        ore_max_bails: 12,\n", "")]),
        (
            "economy.ore_max_bails".to_owned(),
            DataErrorKind::MissingField
        )
    );
    assert_eq!(
        rules_error("no_spread", &[("        ore_spread_above: 6,\n", "")]),
        (
            "economy.ore_spread_above".to_owned(),
            DataErrorKind::MissingField
        )
    );
    assert_eq!(
        rules_error("max_0", &[("ore_max_bails: 12,", "ore_max_bails: 0,")]),
        (
            "economy.ore_max_bails".to_owned(),
            invalid("0", "1 to 1000000")
        )
    );
    assert_eq!(
        rules_error(
            "spread_low",
            &[("ore_spread_above: 6,", "ore_spread_above: -1,")]
        ),
        (
            "economy.ore_spread_above".to_owned(),
            invalid("-1", "0 to 1000000")
        )
    );
    let units = r#"crate_units: ["tank", "heavy", "miner_a"],"#;
    assert_eq!(
        rules_error("no_units", &[(&format!("    {units}\n"), "")]),
        ("crate_units".to_owned(), DataErrorKind::MissingField)
    );
    assert_eq!(
        rules_error(
            "unknown_unit",
            &[(units, r#"crate_units: ["tank", "zeppelin"],"#)]
        ),
        (
            "crate_units[1]".to_owned(),
            DataErrorKind::UnknownReference {
                kind: "unit".to_owned(),
                id: "zeppelin".to_owned()
            }
        )
    );
    // A building is no unit.
    assert_eq!(
        rules_error("a_building", &[(units, r#"crate_units: ["allied_yard"],"#)]),
        (
            "crate_units[0]".to_owned(),
            DataErrorKind::UnknownReference {
                kind: "unit".to_owned(),
                id: "allied_yard".to_owned()
            }
        )
    );
    assert_eq!(
        rules_error(
            "unit_twice",
            &[(units, r#"crate_units: ["tank", "heavy", "tank"],"#)]
        ),
        (
            "crate_units[2]".to_owned(),
            DataErrorKind::DuplicateId("tank".to_owned())
        )
    );
    // A powerup that is no content of a crate.
    assert_eq!(
        rules_error(
            "unknown_powerup",
            &[(
                r#"(id: "reveal", share: 10, value: 0),"#,
                r#"(id: "gas", share: 10, value: 0),"#
            )]
        ),
        (
            "powerups[7].id".to_owned(),
            invalid(
                "\"gas\"",
                "money, unit, veteran, heal_base, armour, firepower, speed or reveal"
            )
        )
    );
}

// --- Boundary -------------------------------------------------------------------------------

#[test]
fn the_limits_of_the_new_values_are_accepted() {
    let economy = |name: &str, from: &str, to: &str| {
        load(
            variant(name, "economy.ron", &[(from, to)]),
            fixture("rules.ron"),
        )
        .unwrap_or_else(|error| panic!("{name}: {error}"))
    };
    for interval in [1, MAX_HARVEST_INTERVAL] {
        let definitions = economy(
            &format!("limit_interval_{interval}"),
            MINER_A,
            &format!(
                r#"(unit: "miner_a", capacity: 20, harvest_rate: 1, harvest_interval: {interval}),"#
            ),
        );
        assert_eq!(
            definitions
                .harvester(unit(&definitions, "miner_a"))
                .expect("a harvester")
                .harvest_interval,
            interval
        );
    }
    for value in [1, MAX_CREDITS_PER_ORE] {
        let definitions = economy(
            &format!("limit_gem_{value}"),
            REFINERY_A,
            &format!(
                r#"(building: "allied_refinery", credits_per_ore: 25, credits_per_gem: {value}),"#
            ),
        );
        let kind = building(&definitions, "allied_refinery");
        assert_eq!(
            definitions
                .building(kind)
                .expect("a building")
                .credits_per_gem,
            Some(value)
        );
    }
    // A refund of nothing and of the whole cost; and no refunds at all.
    for value in [0, 600] {
        let definitions = economy(
            &format!("limit_refund_{value}"),
            REFUND,
            &format!(r#"(building: "allied_refinery", credits: {value}),"#),
        );
        assert_eq!(
            definitions.refund(building(&definitions, "allied_refinery")),
            value
        );
    }
    let definitions = economy("limit_no_refund", REFUND, "");
    assert_eq!(
        definitions.refund(building(&definitions, "allied_refinery")),
        240
    );
}

#[test]
fn a_crate_holds_from_no_unit_type_to_the_most_the_schema_allows() {
    let all: Vec<String> = definitions()
        .units()
        .iter()
        .map(|unit| format!("{:?}", unit.id))
        .collect();
    assert!(all.len() > MAX_CRATE_UNITS, "the fixture has enough types");
    let units = r#"crate_units: ["tank", "heavy", "miner_a"],"#;
    let with = |name: &str, count: usize| {
        variant(
            name,
            "rules.ron",
            &[(
                units,
                &format!("crate_units: [{}],", all[..count].join(", ")),
            )],
        )
    };
    let none =
        load(fixture("economy.ron"), with("units_0", 0)).unwrap_or_else(|error| panic!("{error}"));
    assert!(none.rules().expect("rules").crate_units.is_empty());
    let most = load(fixture("economy.ron"), with("units_most", MAX_CRATE_UNITS))
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        most.rules().expect("rules").crate_units.len(),
        MAX_CRATE_UNITS
    );
    let error = load(
        fixture("economy.ron"),
        with("units_too_many", MAX_CRATE_UNITS + 1),
    )
    .expect_err("one type too many");
    assert_eq!(
        (error.field.as_str(), error.kind),
        (
            "crate_units",
            DataErrorKind::TooManyEntries {
                max: MAX_CRATE_UNITS
            }
        )
    );
}
