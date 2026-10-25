//! RA6 required-field, flag bounds, numeric bounds and hash coverage.
use sim::{DataErrorKind, Definitions};
use std::path::PathBuf;
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data")
}
fn base() -> Definitions {
    sim::load_ruleset(root()).unwrap()
}
fn variant(file: &str, from: &str, to: &str) -> PathBuf {
    let text = std::fs::read_to_string(root().join(file)).unwrap();
    assert!(text.contains(from), "{file}: {from}");
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ra6_definitions");
    std::fs::create_dir_all(&dir).unwrap();
    let mut h = sim::Fnv1a64::new();
    h.write(from.as_bytes());
    h.write(to.as_bytes());
    let path = dir.join(format!("{}-{:016x}-{file}", std::process::id(), h.finish()));
    std::fs::write(&path, text.replacen(from, to, 1)).unwrap();
    path
}
fn load(file: &str, path: PathBuf) -> Result<Definitions, sim::DataError> {
    if file == "arsenal.ron" {
        base().with_arsenal(path)
    } else if file == "rules.ron" {
        base().with_rules(path)
    } else {
        base().with_combat(path)
    }
}
fn hash(defs: Definitions) -> u64 {
    let mut app = bevy_app::App::new();
    app.add_plugins(sim::SimPlugin { seed: 1 });
    app.world_mut().insert_resource(defs);
    sim::state_hash(app.world_mut())
}
#[test]
fn ra6_each_new_flag_is_required_bounded_and_hashed() {
    let original = hash(base());
    for (file, fields) in [
        (
            "arsenal.ron",
            vec![
                "deployer",
                "deploy_fire",
                "c4",
                "can_c4",
                "bombable",
                "spyable",
                "detect_disguise",
                "immune_psionics",
                "immune_radiation",
                "teleporter",
                "stolen_allied",
                "stolen_soviet",
                "overpowerable",
                "explodes",
                "disguise_when_still",
                "strategic",
                "show_timer",
            ],
        ),
        (
            "combat.ron",
            vec![
                "temporal",
                "mind_control",
                "psychic_damage",
                "radiation",
                "parasite",
                "ivan_bomb",
                "bomb_disarm",
                "electric_assault",
                "makes_disguise",
                "area_fire",
                "naval_only",
                "suicide",
                "sonic",
                "culling",
            ],
        ),
    ] {
        let text = std::fs::read_to_string(root().join(file)).unwrap();
        for field in fields {
            let (prefix, suffix) = if field == "suicide" {
                (", ", "")
            } else {
                ("", ",")
            };
            let zero = format!("{prefix}{field}: 0{suffix}");
            let one = format!("{prefix}{field}: 1{suffix}");
            let (from, to) = if text.contains(&zero) {
                (&zero, &one)
            } else {
                (&one, &zero)
            };
            assert!(
                matches!(
                    load(file, variant(file, from, "")).unwrap_err().kind,
                    DataErrorKind::MissingField
                ),
                "{field}"
            );
            for n in [-1, 2] {
                assert!(
                    matches!(
                        load(
                            file,
                            variant(file, from, &format!("{prefix}{field}: {n}{suffix}"))
                        )
                        .unwrap_err()
                        .kind,
                        DataErrorKind::InvalidValue { .. }
                    ),
                    "{field}"
                );
            }
            let changed = load(file, variant(file, from, to)).unwrap();
            assert_ne!(hash(changed), original, "{field}");
        }
    }
}
#[test]
fn ra6_area_fire_one_cell_minimum_is_narrow_and_new_numbers_are_bounded() {
    for (file, field, max) in [
        ("arsenal.ron", "undeploy_delay", 1_000_000),
        ("combat.ron", "rad_level", 10_000),
        ("combat.ron", "paralyzes", 32767),
        ("arsenal.ron", "ifv_mode", 12),
    ] {
        let from = format!("{field}: 0");
        assert!(load(file, variant(file, &from, &format!("{field}: {max}"))).is_ok());
        for n in [-1, max + 1] {
            assert!(matches!(
                load(file, variant(file, &from, &format!("{field}: {n}")))
                    .unwrap_err()
                    .kind,
                DataErrorKind::InvalidValue { .. }
            ));
        }
    }
    let prefix = "id: \"psi_wave\", damage: 250, range: 256";
    assert!(load("combat.ron", variant("combat.ron", prefix, prefix)).is_ok());
    assert!(
        load(
            "combat.ron",
            variant(
                "combat.ron",
                prefix,
                "id: \"psi_wave\", damage: 250, range: 255"
            )
        )
        .is_err()
    );
    let text = std::fs::read_to_string(root().join("combat.ron")).unwrap();
    let line = text.lines().find(|l| l.contains(prefix)).unwrap();
    assert!(
        load(
            "combat.ron",
            variant(
                "combat.ron",
                line,
                &line.replace("area_fire: 1", "area_fire: 0")
            )
        )
        .is_err()
    );
}

#[test]
fn ra6_repair_negative_damage_is_narrow_and_gunner_requires_all_thirteen_modes() {
    let combat = std::fs::read_to_string(root().join("combat.ron")).unwrap();
    let repair = combat
        .lines()
        .find(|l| l.contains("id: \"repair_bullet\""))
        .unwrap();
    assert!(repair.contains("damage: -50"));
    assert!(
        load(
            "combat.ron",
            variant(
                "combat.ron",
                repair,
                &repair.replace("repair: 1", "repair: 0")
            )
        )
        .is_err()
    );
    for damage in [-1, -50, i32::MIN] {
        let ordinary = combat
            .lines()
            .find(|l| l.contains("id: \"prism_shot\""))
            .unwrap();
        assert!(
            load(
                "combat.ron",
                variant(
                    "combat.ron",
                    ordinary,
                    &ordinary.replace("damage: 120", &format!("damage: {damage}"))
                )
            )
            .is_err()
        );
    }
    let arsenal = std::fs::read_to_string(root().join("arsenal.ron")).unwrap();
    let ifv = arsenal.lines().find(|l| l.contains("id: \"ifv\"")).unwrap();
    assert!(
        load(
            "arsenal.ron",
            variant("arsenal.ron", ifv, &ifv.replace("gunner: 1", "gunner: 0"))
        )
        .is_err()
    );
    let start = ifv.find("ifv_weapons: [").unwrap();
    let end = start + ifv[start..].find(']').unwrap();
    let empty = format!("{}ifv_weapons: []{}", &ifv[..start], &ifv[end + 1..]);
    assert!(load("arsenal.ron", variant("arsenal.ron", ifv, &empty)).is_err());
    let disabled = load(
        "arsenal.ron",
        variant("arsenal.ron", ifv, &empty.replace("gunner: 1", "gunner: 0")),
    )
    .unwrap();
    assert_ne!(hash(disabled), hash(base()));
    assert!(
        load(
            "arsenal.ron",
            variant(
                "arsenal.ron",
                ifv,
                &ifv.replace("\"cow_shot\"", "\"missing\"")
            )
        )
        .is_err()
    );
}
#[test]
fn ra6_strategic_timing_fields_are_required_bounded_and_hashed() {
    for (field, value) in [("nuclear_flight", 200), ("chrono_freeze", 80)] {
        let old = format!("{field}: {value},");
        assert!(matches!(
            load("rules.ron", variant("rules.ron", &old, ""))
                .unwrap_err()
                .kind,
            DataErrorKind::MissingField
        ));
        for value in [0, 72_001] {
            assert!(
                load(
                    "rules.ron",
                    variant("rules.ron", &old, &format!("{field}: {value},"))
                )
                .is_err()
            );
        }
        for value in [1, 72_000] {
            let d = load(
                "rules.ron",
                variant("rules.ron", &old, &format!("{field}: {value},")),
            )
            .unwrap();
            assert_ne!(hash(d), hash(base()));
        }
    }
}
