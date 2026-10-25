//! RA8 immutable sharing preserves value semantics and independent consuming loaders.
use sim::{Definitions, load_ruleset};
use std::path::PathBuf;

fn variant(name: &str, file: &str, needle: &str, replacement: &str) -> PathBuf {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/data")
        .join(file);
    let text = std::fs::read_to_string(source).unwrap();
    assert!(text.contains(needle), "missing mutation anchor {needle}");
    let dir = PathBuf::from(std::env::var("CARGO_TARGET_DIR").expect("configured cargo target"))
        .join("tmp/ra8_definitions");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}-{file}"));
    std::fs::write(&path, text.replacen(needle, replacement, 1)).unwrap();
    path
}

#[test]
fn ra8_country_support_defence_and_house_policy_fields_are_required_bounded_and_cross_validated() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data");
    let defs = load_ruleset(&root).unwrap();
    for (name, needle, replacement) in [
        ("policy-missing", "rules_difficulty: 1,", ""),
        (
            "policy-low",
            "rules_difficulty: 1,",
            "rules_difficulty: -1,",
        ),
        (
            "policy-high",
            "rules_difficulty: 1,",
            "rules_difficulty: 2,",
        ),
        ("country-missing", "country: \"\",", ""),
        (
            "country-wrong-side",
            "country: \"\",",
            "country: \"russia\",",
        ),
        ("defence-missing", "defences: [],", ""),
        (
            "defence-lane",
            "defences: [],",
            "defences: [\"power_plant\"],",
        ),
        ("support-missing", "support: [],", ""),
        (
            "support-prerequisite",
            "support: [],",
            "support: [\"aircraft_carrier\"],",
        ),
        ("house-unknown", "id: \"easy\",", "id: \"unknown\","),
        ("reserves-required", "tier_reserves: [0, 0, 0],", ""),
        (
            "reserves-short",
            "tier_reserves: [0, 0, 0],",
            "tier_reserves: [0,0],",
        ),
        (
            "reserves-long",
            "tier_reserves: [0, 0, 0],",
            "tier_reserves: [0,0,0,0],",
        ),
        (
            "reserves-negative",
            "tier_reserves: [0, 0, 0],",
            "tier_reserves: [-1,0,0],",
        ),
        (
            "reserves-bound",
            "tier_reserves: [0, 0, 0],",
            "tier_reserves: [201,0,0],",
        ),
        ("tier-required", "advanced_tech_level: 6,", ""),
        (
            "tier-low",
            "advanced_tech_level: 6,",
            "advanced_tech_level: -1,",
        ),
        (
            "tier-high",
            "advanced_tech_level: 6,",
            "advanced_tech_level: 11,",
        ),
    ] {
        let path = variant(name, "ai.ron", needle, replacement);
        assert!(sim::load_ai(path, &defs).is_err(), "{name}");
    }
    let policy = variant(
        "policy-neutral",
        "ai.ron",
        "rules_difficulty: 1,",
        "rules_difficulty: 0,",
    );
    assert!(!sim::load_ai(policy, &defs).unwrap().rules_difficulty);
}

#[test]
fn ra8_strategic_target_weights_are_required_exactly_twelve_and_bounded() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data");
    let defs = load_ruleset(&root).unwrap();
    let needle = "target_weights: [10, 100, 60, 1, 1, 1, 1, 1, 35, 40, 20, 40],";
    for (name, replacement) in [
        ("missing", ""),
        ("short", "target_weights: [1, 2],"),
        ("long", "target_weights: [1,1,1,1,1,1,1,1,1,1,1,1,1],"),
        ("negative", "target_weights: [-1,1,1,1,1,1,1,1,1,1,1,1],"),
        (
            "above-bound",
            "target_weights: [10001,1,1,1,1,1,1,1,1,1,1,1],",
        ),
    ] {
        assert!(
            defs.clone()
                .with_rules(variant(name, "rules.ron", needle, replacement))
                .is_err(),
            "{name}"
        );
    }
    let positive = defs
        .clone()
        .with_rules(variant(
            "bounds",
            "rules.ron",
            needle,
            "target_weights: [0,10000,1,1,1,1,1,1,1,1,1,1],",
        ))
        .unwrap();
    assert_eq!(
        positive.rules().unwrap().difficulties[0].target_weights[0..2],
        [0, 10000]
    );
    assert_ne!(positive, defs);
}

#[test]
fn ra8_every_tactical_policy_field_is_required_and_bounded() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data");
    let defs = load_ruleset(root).unwrap();
    for (field, value, max) in [
        ("garrison_quota", 3, 200),
        ("garrison_radius", 12, 32),
        ("depot_radius", 16, 32),
        ("naval_rescan", 600, 1200),
        ("naval_detector_quota", 1, 200),
        ("artillery_min_range", 8, 32),
        ("artillery_standoff", 3, 32),
        ("paradrop_offset", 4, 32),
        ("chrono_min_offset", 3, 32),
        ("chrono_max_offset", 8, 32),
    ] {
        let needle = format!("{field}: {value}");
        let missing_needle = if field == "chrono_max_offset" {
            format!(", {needle}")
        } else {
            format!("{needle},")
        };
        assert!(
            sim::load_ai(
                variant(
                    &format!("policy-{field}-missing"),
                    "ai.ron",
                    &missing_needle,
                    ""
                ),
                &defs
            )
            .is_err(),
            "{field} missing"
        );
        for (label, replacement) in [
            ("negative", format!("{field}: -1")),
            ("above", format!("{field}: {}", max + 1)),
        ] {
            let path = variant(
                &format!("policy-{field}-{label}"),
                "ai.ron",
                &needle,
                &replacement,
            );
            assert!(sim::load_ai(path, &defs).is_err(), "{field} {label}");
        }
    }
    assert!(
        sim::load_ai(
            variant(
                "policy-chrono-order",
                "ai.ron",
                "chrono_min_offset: 3",
                "chrono_min_offset: 9"
            ),
            &defs
        )
        .is_err()
    );
    assert!(
        sim::load_ai(
            variant(
                "policy-scan-zero",
                "ai.ron",
                "naval_rescan: 600",
                "naval_rescan: 0"
            ),
            &defs
        )
        .is_err()
    );
}

#[test]
fn ra8_clones_share_read_only_tables_but_loading_a_variant_replaces_only_that_clone() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let original = load_ruleset(root.join("../../assets/data")).unwrap();
    let copy = original.clone();
    assert_eq!(original, copy);
    assert!(std::ptr::eq(original.units(), copy.units()));
    assert!(std::ptr::eq(original.arsenal(), copy.arsenal()));
    assert!(std::ptr::eq(original.weapons(), copy.weapons()));
    let variant = copy.with_economy(root.join("tests/fixtures/data/legacy/economy.ron"));
    // That economy names legacy units, so its typed rejection cannot alter the original.
    assert!(variant.is_err());
    assert_eq!(
        original,
        load_ruleset(root.join("../../assets/data")).unwrap()
    );
    let unit_file = root.join("../../assets/data/units.ron");
    let units = sim::load_unit_definitions(unit_file).unwrap();
    let empty_combat = Definitions::default();
    assert_ne!(units, empty_combat);
    let loaded = units
        .clone()
        .with_economy(root.join("../../assets/data/economy.ron"))
        .unwrap();
    assert_eq!(units.buildings().len(), 0);
    assert!(!loaded.buildings().is_empty());
}
