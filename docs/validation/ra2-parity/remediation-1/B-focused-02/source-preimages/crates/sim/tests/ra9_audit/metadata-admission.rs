//! Historical preparation note: UNVALIDATED/UNRUN metadata-only implementation non-use proposal. No production patch.
//! Only real source-code absence/model approval admits these exact147 rows; never RA2 semantic
//! non-use, constant native witnesses, Own coverage, default/read/alias credit or a reason wildcard.
use super::{
    Audit, AuditResult, Definitions, Inputs, Portion, SourceKey, Unused, Witness, WitnessId,
};
use std::collections::BTreeSet;
pub(super) struct Approval<'a> {
    pub(super) recorded_decision: &'a str,
    pub(super) source_census_sha256: &'a str,
}
struct Spec {
    section: &'static str,
    key: &'static str,
    exact: &'static str,
    policy: &'static str,
    bindings: Vec<(&'static str, &'static str, bool)>,
    weapon: Option<&'static str>,
    warhead: Option<&'static str>,
    guards: Vec<(
        &'static str,
        &'static str,
        &'static str,
        Option<&'static str>,
    )>,
    incoming: Vec<(&'static str, &'static str, &'static str)>,
}
fn specifications() -> Vec<Spec> {
    vec![
        Spec {
            section: "ADOG",
            key: "Points",
            exact: "10",
            policy: "points-model",
            bindings: vec![("attack_dog", "ADOG", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "ADOG", "Cost", Some("200"))],
            incoming: vec![],
        },
        Spec {
            section: "AEGIS",
            key: "Points",
            exact: "35",
            policy: "points-model",
            bindings: vec![("aegis_cruiser", "AEGIS", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "AEGIS", "Cost", Some("1200"))],
            incoming: vec![],
        },
        Spec {
            section: "AMCV",
            key: "Points",
            exact: "60",
            policy: "points-model",
            bindings: vec![("mcv", "AMCV", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "AMCV", "Cost", Some("3000"))],
            incoming: vec![],
        },
        Spec {
            section: "AMRADR",
            key: "Points",
            exact: "70",
            policy: "points-model",
            bindings: vec![("american_airforce_command", "AMRADR", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "AMRADR", "Cost", Some("1000"))],
            incoming: vec![],
        },
        Spec {
            section: "APOC",
            key: "Points",
            exact: "60",
            policy: "points-model",
            bindings: vec![("apocalypse", "APOC", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "APOC", "Cost", Some("1750"))],
            incoming: vec![],
        },
        Spec {
            section: "ASW",
            key: "Points",
            exact: "10",
            policy: "points-model",
            bindings: vec![("osprey", "ASW", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "ASW", "Cost", Some("50"))],
            incoming: vec![],
        },
        Spec {
            section: "ATESLA",
            key: "Points",
            exact: "30",
            policy: "points-model",
            bindings: vec![("prism_tower", "ATESLA", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "ATESLA", "Cost", Some("1500"))],
            incoming: vec![],
        },
        Spec {
            section: "BEAG",
            key: "Points",
            exact: "20",
            policy: "points-model",
            bindings: vec![("black_eagle", "BEAG", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "BEAG", "Cost", Some("1200"))],
            incoming: vec![],
        },
        Spec {
            section: "CAAIRP",
            key: "Points",
            exact: "5",
            policy: "points-model",
            bindings: vec![("airport", "CAAIRP", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "CAAIRP", "Cost", None)],
            incoming: vec![],
        },
        Spec {
            section: "CACITY01",
            key: "Points",
            exact: "5",
            policy: "points-model",
            bindings: vec![("civilian_tower", "CACITY01", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "CACITY01", "Cost", None)],
            incoming: vec![],
        },
        Spec {
            section: "CAEUR1",
            key: "Points",
            exact: "5",
            policy: "points-model",
            bindings: vec![("civilian_house", "CAEUR1", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "CAEUR1", "Cost", None)],
            incoming: vec![],
        },
        Spec {
            section: "CAOILD",
            key: "Points",
            exact: "5",
            policy: "points-model",
            bindings: vec![("oil_derrick", "CAOILD", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "CAOILD", "Cost", None)],
            incoming: vec![],
        },
        Spec {
            section: "CAOUTP",
            key: "Points",
            exact: "5",
            policy: "points-model",
            bindings: vec![("tech_outpost", "CAOUTP", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "CAOUTP", "Cost", None)],
            incoming: vec![],
        },
        Spec {
            section: "CARRIER",
            key: "Points",
            exact: "55",
            policy: "points-model",
            bindings: vec![("aircraft_carrier", "CARRIER", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "CARRIER", "Cost", Some("2000"))],
            incoming: vec![],
        },
        Spec {
            section: "CATHOSP",
            key: "Points",
            exact: "5",
            policy: "points-model",
            bindings: vec![("hospital", "CATHOSP", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "CATHOSP", "Cost", None)],
            incoming: vec![],
        },
        Spec {
            section: "CAWA2A",
            key: "Points",
            exact: "5",
            policy: "points-model",
            bindings: vec![("civilian_city", "CAWA2A", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "CAWA2A", "Cost", None)],
            incoming: vec![],
        },
        Spec {
            section: "CCOMAND",
            key: "Points",
            exact: "50",
            policy: "points-model",
            bindings: vec![("chrono_commando", "CCOMAND", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "CCOMAND", "Cost", Some("2000"))],
            incoming: vec![],
        },
        Spec {
            section: "CIVAN",
            key: "Points",
            exact: "50",
            policy: "points-model",
            bindings: vec![("chrono_ivan", "CIVAN", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "CIVAN", "Cost", Some("1000"))],
            incoming: vec![],
        },
        Spec {
            section: "CLEG",
            key: "Points",
            exact: "15",
            policy: "points-model",
            bindings: vec![("chrono_legionnaire", "CLEG", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "CLEG", "Cost", Some("1500"))],
            incoming: vec![],
        },
        Spec {
            section: "CMIN",
            key: "Points",
            exact: "55",
            policy: "points-model",
            bindings: vec![("chrono_miner", "CMIN", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "CMIN", "Cost", Some("1400"))],
            incoming: vec![],
        },
        Spec {
            section: "CRNeutronRifle",
            key: "IsRadBeam",
            exact: "yes",
            policy: "rad-beam",
            bindings: vec![],
            weapon: Some("cr_neutron_rifle"),
            warhead: None,
            guards: vec![
                ("rules", "CRNeutronRifle", "Warhead", Some("ChronoBeam")),
                ("rules", "CRNeutronRifle", "RadLevel", None),
                ("rules", "ChronoBeam", "Temporal", Some("yes")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "CRRadBeamWeapon",
            key: "IsRadBeam",
            exact: "yes",
            policy: "rad-beam",
            bindings: vec![],
            weapon: Some("cr_rad_beam_weapon"),
            warhead: None,
            guards: vec![
                (
                    "rules",
                    "CRRadBeamWeapon",
                    "Warhead",
                    Some("RadBeamWarhead"),
                ),
                ("rules", "CRRadBeamWeapon", "RadLevel", None),
                ("rules", "RadBeamWarhead", "Temporal", None),
            ],
            incoming: vec![],
        },
        Spec {
            section: "CombatDamage",
            key: "IvanIconFlickerRate",
            exact: "8",
            policy: "ivan-icon",
            bindings: vec![],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "CombatDamage", "IvanTimedDelay", Some("450"))],
            incoming: vec![],
        },
        Spec {
            section: "Comet",
            key: "IsHouseColor",
            exact: "true",
            policy: "house-color",
            bindings: vec![],
            weapon: Some("comet"),
            warhead: None,
            guards: vec![
                ("rules", "Comet", "IsLaser", Some("true")),
                ("rules", "Comet", "Projectile", Some("LargeCometP")),
                ("rules", "Comet", "Warhead", Some("CometWH")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "Comet",
            key: "IsLaser",
            exact: "true",
            policy: "nonbuilding-laser",
            bindings: vec![],
            weapon: Some("comet"),
            warhead: None,
            guards: vec![
                ("rules", "Comet", "IsLaser", Some("true")),
                ("rules", "Comet", "Projectile", Some("LargeCometP")),
                ("rules", "Comet", "Warhead", Some("CometWH")),
            ],
            incoming: vec![("SREF", "Weapon1", "Comet"), ("VehicleTypes", "37", "SREF")],
        },
        Spec {
            section: "CometFragment",
            key: "IsHouseColor",
            exact: "true",
            policy: "house-color",
            bindings: vec![],
            weapon: None,
            warhead: None,
            guards: vec![
                ("rules", "CometFragment", "IsLaser", Some("true")),
                ("rules", "CometFragment", "Projectile", Some("SmallCometP")),
                ("rules", "CometFragment", "Warhead", Some("CometWH")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "CometFragment",
            key: "IsLaser",
            exact: "true",
            policy: "nonbuilding-laser",
            bindings: vec![],
            weapon: None,
            warhead: None,
            guards: vec![
                ("rules", "CometFragment", "IsLaser", Some("true")),
                ("rules", "CometFragment", "Projectile", Some("SmallCometP")),
                ("rules", "CometFragment", "Warhead", Some("CometWH")),
            ],
            incoming: vec![
                ("Comet", "Projectile", "LargeCometP"),
                ("LargeCometP", "ShrapnelWeapon", "CometFragment"),
                ("SREF", "EliteWeapon1", "SuperComet"),
                ("SREF", "Weapon1", "Comet"),
                ("SuperComet", "Projectile", "SuperCometP"),
                ("SuperCometFragment", "Projectile", "SuperSmallCometP"),
                ("SuperCometP", "ShrapnelWeapon", "SuperCometFragment"),
                ("SuperSmallCometP", "ShrapnelWeapon", "CometFragment"),
                ("VehicleTypes", "37", "SREF"),
            ],
        },
        Spec {
            section: "CowShot",
            key: "IsLaser",
            exact: "true",
            policy: "nonbuilding-laser",
            bindings: vec![],
            weapon: Some("cow_shot"),
            warhead: None,
            guards: vec![
                ("rules", "CowShot", "IsLaser", Some("true")),
                ("rules", "CowShot", "Projectile", Some("InvisibleHigh")),
                ("rules", "CowShot", "Warhead", Some("PrismWarhead")),
            ],
            incoming: vec![
                ("FV", "EliteWeapon13", "CowShot"),
                ("FV", "Weapon13", "CowShot"),
                ("VehicleTypes", "41", "FV"),
            ],
        },
        Spec {
            section: "DESO",
            key: "Points",
            exact: "30",
            policy: "points-model",
            bindings: vec![("desolator", "DESO", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "DESO", "Cost", Some("600"))],
            incoming: vec![],
        },
        Spec {
            section: "DEST",
            key: "Points",
            exact: "30",
            policy: "points-model",
            bindings: vec![("destroyer", "DEST", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "DEST", "Cost", Some("1000"))],
            incoming: vec![],
        },
        Spec {
            section: "DLPH",
            key: "Points",
            exact: "15",
            policy: "points-model",
            bindings: vec![("dolphin", "DLPH", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "DLPH", "Cost", Some("500"))],
            incoming: vec![],
        },
        Spec {
            section: "DMISL",
            key: "Points",
            exact: "20",
            policy: "points-model",
            bindings: vec![("dred_missile", "DMISL", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "DMISL", "Cost", Some("50"))],
            incoming: vec![],
        },
        Spec {
            section: "DOG",
            key: "Points",
            exact: "10",
            policy: "points-model",
            bindings: vec![("attack_dog", "DOG", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "DOG", "Cost", Some("200"))],
            incoming: vec![],
        },
        Spec {
            section: "DRED",
            key: "Points",
            exact: "55",
            policy: "points-model",
            bindings: vec![("dreadnought", "DRED", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "DRED", "Cost", Some("2000"))],
            incoming: vec![],
        },
        Spec {
            section: "DRON",
            key: "Points",
            exact: "20",
            policy: "points-model",
            bindings: vec![("terror_drone", "DRON", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "DRON", "Cost", Some("500"))],
            incoming: vec![],
        },
        Spec {
            section: "DTRUCK",
            key: "Points",
            exact: "40",
            policy: "points-model",
            bindings: vec![("demolition_truck", "DTRUCK", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "DTRUCK", "Cost", Some("1500"))],
            incoming: vec![],
        },
        Spec {
            section: "E1",
            key: "Points",
            exact: "10",
            policy: "points-model",
            bindings: vec![("gi", "E1", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "E1", "Cost", Some("200"))],
            incoming: vec![],
        },
        Spec {
            section: "E2",
            key: "Points",
            exact: "5",
            policy: "points-model",
            bindings: vec![("conscript", "E2", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "E2", "Cost", Some("100"))],
            incoming: vec![],
        },
        Spec {
            section: "ENGINEER",
            key: "Points",
            exact: "5",
            policy: "points-model",
            bindings: vec![("engineer", "ENGINEER", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "ENGINEER", "Cost", Some("500"))],
            incoming: vec![],
        },
        Spec {
            section: "FLAKT",
            key: "Points",
            exact: "5",
            policy: "points-model",
            bindings: vec![("flak_trooper", "FLAKT", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "FLAKT", "Cost", Some("300"))],
            incoming: vec![],
        },
        Spec {
            section: "FV",
            key: "BrainBlastTurretIndex",
            exact: "3",
            policy: "ifv-turret-art",
            bindings: vec![("ifv", "FV", false)],
            weapon: None,
            warhead: None,
            guards: vec![
                ("rules", "FV", "Gunner", Some("yes")),
                ("rules", "FV", "TurretCount", Some("4")),
                ("rules", "FV", "WeaponCount", Some("13")),
                ("rules", "FV", "BrainBlastTurretWeapon", Some("8")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "FV",
            key: "ChronoTurretIndex",
            exact: "3",
            policy: "ifv-turret-art",
            bindings: vec![("ifv", "FV", false)],
            weapon: None,
            warhead: None,
            guards: vec![
                ("rules", "FV", "Gunner", Some("yes")),
                ("rules", "FV", "TurretCount", Some("4")),
                ("rules", "FV", "WeaponCount", Some("13")),
                ("rules", "FV", "ChronoTurretWeapon", Some("10")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "FV",
            key: "CowTurretIndex",
            exact: "3",
            policy: "ifv-turret-art",
            bindings: vec![("ifv", "FV", false)],
            weapon: None,
            warhead: None,
            guards: vec![
                ("rules", "FV", "Gunner", Some("yes")),
                ("rules", "FV", "TurretCount", Some("4")),
                ("rules", "FV", "WeaponCount", Some("13")),
                ("rules", "FV", "CowTurretWeapon", Some("12")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "FV",
            key: "ExplodeTurretIndex",
            exact: "3",
            policy: "ifv-turret-art",
            bindings: vec![("ifv", "FV", false)],
            weapon: None,
            warhead: None,
            guards: vec![
                ("rules", "FV", "Gunner", Some("yes")),
                ("rules", "FV", "TurretCount", Some("4")),
                ("rules", "FV", "WeaponCount", Some("13")),
                ("rules", "FV", "ExplodeTurretWeapon", Some("7")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "FV",
            key: "FlakTurretIndex",
            exact: "1",
            policy: "ifv-turret-art",
            bindings: vec![("ifv", "FV", false)],
            weapon: None,
            warhead: None,
            guards: vec![
                ("rules", "FV", "Gunner", Some("yes")),
                ("rules", "FV", "TurretCount", Some("4")),
                ("rules", "FV", "WeaponCount", Some("13")),
                ("rules", "FV", "FlakTurretWeapon", Some("3")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "FV",
            key: "MachineGunTurretIndex",
            exact: "1",
            policy: "ifv-turret-art",
            bindings: vec![("ifv", "FV", false)],
            weapon: None,
            warhead: None,
            guards: vec![
                ("rules", "FV", "Gunner", Some("yes")),
                ("rules", "FV", "TurretCount", Some("4")),
                ("rules", "FV", "WeaponCount", Some("13")),
                ("rules", "FV", "MachineGunTurretWeapon", Some("2")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "FV",
            key: "NormalTurretIndex",
            exact: "0",
            policy: "ifv-turret-art",
            bindings: vec![("ifv", "FV", false)],
            weapon: None,
            warhead: None,
            guards: vec![
                ("rules", "FV", "Gunner", Some("yes")),
                ("rules", "FV", "TurretCount", Some("4")),
                ("rules", "FV", "WeaponCount", Some("13")),
                ("rules", "FV", "NormalTurretWeapon", Some("0")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "FV",
            key: "PistolTurretIndex",
            exact: "1",
            policy: "ifv-turret-art",
            bindings: vec![("ifv", "FV", false)],
            weapon: None,
            warhead: None,
            guards: vec![
                ("rules", "FV", "Gunner", Some("yes")),
                ("rules", "FV", "TurretCount", Some("4")),
                ("rules", "FV", "WeaponCount", Some("13")),
                ("rules", "FV", "PistolTurretWeapon", Some("4")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "FV",
            key: "Points",
            exact: "20",
            policy: "points-model",
            bindings: vec![("ifv", "FV", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "FV", "Cost", Some("600"))],
            incoming: vec![],
        },
        Spec {
            section: "FV",
            key: "RadCannonTurretIndex",
            exact: "3",
            policy: "ifv-turret-art",
            bindings: vec![("ifv", "FV", false)],
            weapon: None,
            warhead: None,
            guards: vec![
                ("rules", "FV", "Gunner", Some("yes")),
                ("rules", "FV", "TurretCount", Some("4")),
                ("rules", "FV", "WeaponCount", Some("13")),
                ("rules", "FV", "RadCannonTurretWeapon", Some("9")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "FV",
            key: "RepairTurretIndex",
            exact: "2",
            policy: "ifv-turret-art",
            bindings: vec![("ifv", "FV", false)],
            weapon: None,
            warhead: None,
            guards: vec![
                ("rules", "FV", "Gunner", Some("yes")),
                ("rules", "FV", "TurretCount", Some("4")),
                ("rules", "FV", "WeaponCount", Some("13")),
                ("rules", "FV", "RepairTurretWeapon", Some("1")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "FV",
            key: "ShockTurretIndex",
            exact: "3",
            policy: "ifv-turret-art",
            bindings: vec![("ifv", "FV", false)],
            weapon: None,
            warhead: None,
            guards: vec![
                ("rules", "FV", "Gunner", Some("yes")),
                ("rules", "FV", "TurretCount", Some("4")),
                ("rules", "FV", "WeaponCount", Some("13")),
                ("rules", "FV", "ShockTurretWeapon", Some("6")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "FV",
            key: "SniperTurretIndex",
            exact: "1",
            policy: "ifv-turret-art",
            bindings: vec![("ifv", "FV", false)],
            weapon: None,
            warhead: None,
            guards: vec![
                ("rules", "FV", "Gunner", Some("yes")),
                ("rules", "FV", "TurretCount", Some("4")),
                ("rules", "FV", "WeaponCount", Some("13")),
                ("rules", "FV", "SniperTurretWeapon", Some("5")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "FV",
            key: "TerroristExplodeTurretIndex",
            exact: "3",
            policy: "ifv-turret-art",
            bindings: vec![("ifv", "FV", false)],
            weapon: None,
            warhead: None,
            guards: vec![
                ("rules", "FV", "Gunner", Some("yes")),
                ("rules", "FV", "TurretCount", Some("4")),
                ("rules", "FV", "WeaponCount", Some("13")),
                ("rules", "FV", "TerroristExplodeTurretWeapon", Some("11")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "GAAIRC",
            key: "Points",
            exact: "70",
            policy: "points-model",
            bindings: vec![("airforce_command", "GAAIRC", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "GAAIRC", "Cost", Some("1000"))],
            incoming: vec![],
        },
        Spec {
            section: "GACNST",
            key: "Points",
            exact: "80",
            policy: "points-model",
            bindings: vec![("construction_yard", "GACNST", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "GACNST", "Cost", Some("3000"))],
            incoming: vec![],
        },
        Spec {
            section: "GACSPH",
            key: "ChargedAnimTime",
            exact: "1",
            policy: "sw-animation",
            bindings: vec![("chronosphere", "GACSPH", true)],
            weapon: None,
            warhead: None,
            guards: vec![
                (
                    "rules",
                    "GACSPH",
                    "SuperWeapon",
                    Some("ChronoSphereSpecial"),
                ),
                ("rules", "ChronoSphereSpecial", "RechargeTime", Some("7")),
                ("rules", "ChronoSphereSpecial", "IsPowered", Some("true")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "GACSPH",
            key: "Points",
            exact: "30",
            policy: "points-model",
            bindings: vec![("chronosphere", "GACSPH", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "GACSPH", "Cost", Some("2500"))],
            incoming: vec![],
        },
        Spec {
            section: "GADEPT",
            key: "Points",
            exact: "80",
            policy: "points-model",
            bindings: vec![("service_depot", "GADEPT", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "GADEPT", "Cost", Some("800"))],
            incoming: vec![],
        },
        Spec {
            section: "GAGAP",
            key: "Points",
            exact: "30",
            policy: "points-model",
            bindings: vec![("gap_generator", "GAGAP", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "GAGAP", "Cost", Some("1000"))],
            incoming: vec![],
        },
        Spec {
            section: "GAOREP",
            key: "Points",
            exact: "30",
            policy: "points-model",
            bindings: vec![("ore_purifier", "GAOREP", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "GAOREP", "Cost", Some("2500"))],
            incoming: vec![],
        },
        Spec {
            section: "GAPILE",
            key: "Points",
            exact: "30",
            policy: "points-model",
            bindings: vec![("barracks", "GAPILE", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "GAPILE", "Cost", Some("500"))],
            incoming: vec![],
        },
        Spec {
            section: "GAPILL",
            key: "Points",
            exact: "30",
            policy: "points-model",
            bindings: vec![("pillbox", "GAPILL", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "GAPILL", "Cost", Some("500"))],
            incoming: vec![],
        },
        Spec {
            section: "GAPOWR",
            key: "Points",
            exact: "40",
            policy: "points-model",
            bindings: vec![("power_plant", "GAPOWR", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "GAPOWR", "Cost", Some("800"))],
            incoming: vec![],
        },
        Spec {
            section: "GAREFN",
            key: "Points",
            exact: "80",
            policy: "points-model",
            bindings: vec![("refinery", "GAREFN", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "GAREFN", "Cost", Some("2000"))],
            incoming: vec![],
        },
        Spec {
            section: "GASPYSAT",
            key: "Points",
            exact: "0",
            policy: "points-model",
            bindings: vec![("spy_satellite", "GASPYSAT", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "GASPYSAT", "Cost", Some("1500"))],
            incoming: vec![],
        },
        Spec {
            section: "GATECH",
            key: "Points",
            exact: "85",
            policy: "points-model",
            bindings: vec![("tech_center", "GATECH", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "GATECH", "Cost", Some("2000"))],
            incoming: vec![],
        },
        Spec {
            section: "GAWALL",
            key: "Points",
            exact: "5",
            policy: "points-model",
            bindings: vec![("fortress_walls", "GAWALL", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "GAWALL", "Cost", Some("100"))],
            incoming: vec![],
        },
        Spec {
            section: "GAWEAP",
            key: "Points",
            exact: "80",
            policy: "points-model",
            bindings: vec![("war_factory", "GAWEAP", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "GAWEAP", "Cost", Some("2000"))],
            incoming: vec![],
        },
        Spec {
            section: "GAWEAT",
            key: "ChargedAnimTime",
            exact: "1",
            policy: "sw-animation",
            bindings: vec![("weather_control", "GAWEAT", true)],
            weapon: None,
            warhead: None,
            guards: vec![
                (
                    "rules",
                    "GAWEAT",
                    "SuperWeapon",
                    Some("LightningStormSpecial"),
                ),
                ("rules", "LightningStormSpecial", "RechargeTime", Some("10")),
                ("rules", "LightningStormSpecial", "IsPowered", Some("true")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "GAWEAT",
            key: "Points",
            exact: "30",
            policy: "points-model",
            bindings: vec![("weather_control", "GAWEAT", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "GAWEAT", "Cost", Some("5000"))],
            incoming: vec![],
        },
        Spec {
            section: "GAYARD",
            key: "Points",
            exact: "55",
            policy: "points-model",
            bindings: vec![("naval_shipyard", "GAYARD", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "GAYARD", "Cost", Some("1000"))],
            incoming: vec![],
        },
        Spec {
            section: "GTGCAN",
            key: "Points",
            exact: "30",
            policy: "points-model",
            bindings: vec![("grand_cannon", "GTGCAN", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "GTGCAN", "Cost", Some("2000"))],
            incoming: vec![],
        },
        Spec {
            section: "General",
            key: "DefaultMirageDisguises",
            exact: "TREE01,TREE02,TREE03,TREE04",
            policy: "mirage-art",
            bindings: vec![],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "MGTK", "DisguiseWhenStill", Some("yes"))],
            incoming: vec![],
        },
        Spec {
            section: "HARV",
            key: "Points",
            exact: "55",
            policy: "points-model",
            bindings: vec![("war_miner", "HARV", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "HARV", "Cost", Some("1400"))],
            incoming: vec![],
        },
        Spec {
            section: "HARVWH",
            key: "Bullets",
            exact: "yes",
            policy: "ifv-name",
            bindings: vec![],
            weapon: None,
            warhead: Some("harvwh"),
            guards: vec![],
            incoming: vec![],
        },
        Spec {
            section: "HORNET",
            key: "Points",
            exact: "20",
            policy: "points-model",
            bindings: vec![("hornet", "HORNET", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "HORNET", "Cost", Some("50"))],
            incoming: vec![],
        },
        Spec {
            section: "HTK",
            key: "Points",
            exact: "20",
            policy: "points-model",
            bindings: vec![("flak_track", "HTK", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "HTK", "Cost", Some("500"))],
            incoming: vec![],
        },
        Spec {
            section: "HTNK",
            key: "Points",
            exact: "25",
            policy: "points-model",
            bindings: vec![("rhino", "HTNK", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "HTNK", "Cost", Some("900"))],
            incoming: vec![],
        },
        Spec {
            section: "HYD",
            key: "Points",
            exact: "20",
            policy: "points-model",
            bindings: vec![("sea_scorpion", "HYD", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "HYD", "Cost", Some("600"))],
            incoming: vec![],
        },
        Spec {
            section: "HollowPoint",
            key: "Bullets",
            exact: "yes",
            policy: "ifv-name",
            bindings: vec![],
            weapon: None,
            warhead: Some("hollow_point"),
            guards: vec![],
            incoming: vec![],
        },
        Spec {
            section: "HollowPoint3",
            key: "Bullets",
            exact: "yes",
            policy: "ifv-name",
            bindings: vec![],
            weapon: None,
            warhead: Some("hollow_point3"),
            guards: vec![],
            incoming: vec![],
        },
        Spec {
            section: "HollowPointNoBuilding",
            key: "Bullets",
            exact: "yes",
            policy: "ifv-name",
            bindings: vec![],
            weapon: None,
            warhead: Some("hollow_point_no_building"),
            guards: vec![],
            incoming: vec![],
        },
        Spec {
            section: "IVAN",
            key: "Points",
            exact: "30",
            policy: "points-model",
            bindings: vec![("crazy_ivan", "IVAN", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "IVAN", "Cost", Some("600"))],
            incoming: vec![],
        },
        Spec {
            section: "JUMPJET",
            key: "Points",
            exact: "15",
            policy: "points-model",
            bindings: vec![("rocketeer", "JUMPJET", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "JUMPJET", "Cost", Some("600"))],
            incoming: vec![],
        },
        Spec {
            section: "LCRF",
            key: "Points",
            exact: "15",
            policy: "points-model",
            bindings: vec![("amphibious_transport", "LCRF", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "LCRF", "Cost", Some("900"))],
            incoming: vec![],
        },
        Spec {
            section: "MGTK",
            key: "Points",
            exact: "25",
            policy: "points-model",
            bindings: vec![("mirage_tank", "MGTK", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "MGTK", "Cost", Some("1000"))],
            incoming: vec![],
        },
        Spec {
            section: "MTNK",
            key: "Points",
            exact: "25",
            policy: "points-model",
            bindings: vec![("grizzly", "MTNK", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "MTNK", "Cost", Some("700"))],
            incoming: vec![],
        },
        Spec {
            section: "NACLON",
            key: "Points",
            exact: "30",
            policy: "points-model",
            bindings: vec![("cloning_vats", "NACLON", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "NACLON", "Cost", Some("2500"))],
            incoming: vec![],
        },
        Spec {
            section: "NACNST",
            key: "Points",
            exact: "80",
            policy: "points-model",
            bindings: vec![("soviet_construction_yard", "NACNST", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "NACNST", "Cost", Some("3000"))],
            incoming: vec![],
        },
        Spec {
            section: "NADEPT",
            key: "Points",
            exact: "80",
            policy: "points-model",
            bindings: vec![("soviet_service_depot", "NADEPT", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "NADEPT", "Cost", Some("800"))],
            incoming: vec![],
        },
        Spec {
            section: "NAFLAK",
            key: "Points",
            exact: "30",
            policy: "points-model",
            bindings: vec![("flak_cannon", "NAFLAK", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "NAFLAK", "Cost", Some("1000"))],
            incoming: vec![],
        },
        Spec {
            section: "NAHAND",
            key: "Points",
            exact: "30",
            policy: "points-model",
            bindings: vec![("soviet_barracks", "NAHAND", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "NAHAND", "Cost", Some("500"))],
            incoming: vec![],
        },
        Spec {
            section: "NAIRON",
            key: "ChargedAnimTime",
            exact: "1",
            policy: "sw-animation",
            bindings: vec![("iron_curtain", "NAIRON", true)],
            weapon: None,
            warhead: None,
            guards: vec![
                ("rules", "NAIRON", "SuperWeapon", Some("IronCurtainSpecial")),
                ("rules", "IronCurtainSpecial", "RechargeTime", Some("5")),
                ("rules", "IronCurtainSpecial", "IsPowered", Some("true")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "NAIRON",
            key: "Points",
            exact: "30",
            policy: "points-model",
            bindings: vec![("iron_curtain", "NAIRON", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "NAIRON", "Cost", Some("2500"))],
            incoming: vec![],
        },
        Spec {
            section: "NALASR",
            key: "Points",
            exact: "30",
            policy: "points-model",
            bindings: vec![("sentry_gun", "NALASR", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "NALASR", "Cost", Some("500"))],
            incoming: vec![],
        },
        Spec {
            section: "NAMISL",
            key: "ChargedAnimTime",
            exact: "1",
            policy: "sw-animation",
            bindings: vec![("nuclear_silo", "NAMISL", true)],
            weapon: None,
            warhead: None,
            guards: vec![
                ("rules", "NAMISL", "SuperWeapon", Some("MultiSpecial")),
                ("rules", "MultiSpecial", "RechargeTime", Some("10")),
                ("rules", "MultiSpecial", "IsPowered", Some("true")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "NAMISL",
            key: "Points",
            exact: "30",
            policy: "points-model",
            bindings: vec![("nuclear_silo", "NAMISL", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "NAMISL", "Cost", Some("5000"))],
            incoming: vec![],
        },
        Spec {
            section: "NANRCT",
            key: "Points",
            exact: "30",
            policy: "points-model",
            bindings: vec![("nuclear_reactor", "NANRCT", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "NANRCT", "Cost", Some("1000"))],
            incoming: vec![],
        },
        Spec {
            section: "NAPOWR",
            key: "Points",
            exact: "40",
            policy: "points-model",
            bindings: vec![("tesla_reactor", "NAPOWR", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "NAPOWR", "Cost", Some("600"))],
            incoming: vec![],
        },
        Spec {
            section: "NAPSIS",
            key: "Points",
            exact: "50",
            policy: "points-model",
            bindings: vec![("psychic_sensor", "NAPSIS", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "NAPSIS", "Cost", Some("1000"))],
            incoming: vec![],
        },
        Spec {
            section: "NARADR",
            key: "Points",
            exact: "60",
            policy: "points-model",
            bindings: vec![("radar_tower", "NARADR", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "NARADR", "Cost", Some("1000"))],
            incoming: vec![],
        },
        Spec {
            section: "NAREFN",
            key: "Points",
            exact: "80",
            policy: "points-model",
            bindings: vec![("soviet_refinery", "NAREFN", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "NAREFN", "Cost", Some("2000"))],
            incoming: vec![],
        },
        Spec {
            section: "NASAM",
            key: "Points",
            exact: "30",
            policy: "points-model",
            bindings: vec![("patriot", "NASAM", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "NASAM", "Cost", Some("1000"))],
            incoming: vec![],
        },
        Spec {
            section: "NATECH",
            key: "Points",
            exact: "85",
            policy: "points-model",
            bindings: vec![("soviet_battle_lab", "NATECH", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "NATECH", "Cost", Some("2000"))],
            incoming: vec![],
        },
        Spec {
            section: "NAWALL",
            key: "Points",
            exact: "5",
            policy: "points-model",
            bindings: vec![("soviet_walls", "NAWALL", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "NAWALL", "Cost", Some("100"))],
            incoming: vec![],
        },
        Spec {
            section: "NAWEAP",
            key: "Points",
            exact: "80",
            policy: "points-model",
            bindings: vec![("soviet_war_factory", "NAWEAP", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "NAWEAP", "Cost", Some("2000"))],
            incoming: vec![],
        },
        Spec {
            section: "NAYARD",
            key: "Points",
            exact: "55",
            policy: "points-model",
            bindings: vec![("soviet_naval_shipyard", "NAYARD", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "NAYARD", "Cost", Some("1000"))],
            incoming: vec![],
        },
        Spec {
            section: "NeutronRifle",
            key: "IsRadBeam",
            exact: "yes",
            policy: "rad-beam",
            bindings: vec![],
            weapon: Some("neutron_rifle"),
            warhead: None,
            guards: vec![
                ("rules", "NeutronRifle", "Warhead", Some("ChronoBeam")),
                ("rules", "NeutronRifle", "RadLevel", None),
                ("rules", "ChronoBeam", "Temporal", Some("yes")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "NeutronRifleE",
            key: "IsRadBeam",
            exact: "yes",
            policy: "rad-beam",
            bindings: vec![],
            weapon: Some("neutron_rifle_e"),
            warhead: None,
            guards: vec![
                ("rules", "NeutronRifleE", "Warhead", Some("ChronoBeam")),
                ("rules", "NeutronRifleE", "RadLevel", None),
                ("rules", "ChronoBeam", "Temporal", Some("yes")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "ORCA",
            key: "Points",
            exact: "20",
            policy: "points-model",
            bindings: vec![("harrier", "ORCA", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "ORCA", "Cost", Some("1200"))],
            incoming: vec![],
        },
        Spec {
            section: "PDPLANE",
            key: "Points",
            exact: "30",
            policy: "points-model",
            bindings: vec![("paradrop_plane", "PDPLANE", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "PDPLANE", "Cost", None)],
            incoming: vec![],
        },
        Spec {
            section: "PTROOP",
            key: "Points",
            exact: "50",
            policy: "points-model",
            bindings: vec![("psi_commando", "PTROOP", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "PTROOP", "Cost", Some("1000"))],
            incoming: vec![],
        },
        Spec {
            section: "PrismShot",
            key: "IsHouseColor",
            exact: "true",
            policy: "house-color",
            bindings: vec![],
            weapon: Some("prism_shot"),
            warhead: None,
            guards: vec![
                ("rules", "PrismShot", "IsLaser", Some("true")),
                ("rules", "PrismShot", "Projectile", Some("InvisibleHigh")),
                ("rules", "PrismShot", "Warhead", Some("PrismWarhead")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "PrismSupport",
            key: "IsHouseColor",
            exact: "true",
            policy: "house-color",
            bindings: vec![],
            weapon: Some("prism_support"),
            warhead: None,
            guards: vec![
                ("rules", "PrismSupport", "IsLaser", Some("true")),
                ("rules", "PrismSupport", "Projectile", Some("InvisibleHigh")),
                ("rules", "PrismSupport", "Warhead", Some("DummyWarhead")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "RadBeamWeapon",
            key: "IsRadBeam",
            exact: "yes",
            policy: "rad-beam",
            bindings: vec![],
            weapon: Some("rad_beam_weapon"),
            warhead: None,
            guards: vec![
                ("rules", "RadBeamWeapon", "Warhead", Some("RadBeamWarhead")),
                ("rules", "RadBeamWeapon", "RadLevel", None),
                ("rules", "RadBeamWarhead", "Temporal", None),
            ],
            incoming: vec![],
        },
        Spec {
            section: "RadBeamWeaponE",
            key: "IsRadBeam",
            exact: "yes",
            policy: "rad-beam",
            bindings: vec![],
            weapon: Some("rad_beam_weapon_e"),
            warhead: None,
            guards: vec![
                ("rules", "RadBeamWeaponE", "Warhead", Some("RadBeamWarhead")),
                ("rules", "RadBeamWeaponE", "RadLevel", None),
                ("rules", "RadBeamWarhead", "Temporal", None),
            ],
            incoming: vec![],
        },
        Spec {
            section: "RadEruptionWeapon",
            key: "IsRadEruption",
            exact: "no",
            policy: "rad-umbrella",
            bindings: vec![],
            weapon: Some("rad_eruption_weapon"),
            warhead: None,
            guards: vec![
                ("rules", "RadEruptionWeapon", "IsRadBeam", None),
                ("rules", "RadEruptionWeapon", "RadLevel", Some("500")),
                (
                    "rules",
                    "RadEruptionWeapon",
                    "Warhead",
                    Some("RadEruptionWarhead"),
                ),
            ],
            incoming: vec![],
        },
        Spec {
            section: "Radiation",
            key: "RadLightFactor",
            exact: "0.1",
            policy: "rad-light",
            bindings: vec![],
            weapon: None,
            warhead: None,
            guards: vec![],
            incoming: vec![],
        },
        Spec {
            section: "Radiation",
            key: "RadTintFactor",
            exact: "1.0",
            policy: "rad-tint",
            bindings: vec![],
            weapon: None,
            warhead: None,
            guards: vec![],
            incoming: vec![],
        },
        Spec {
            section: "SA",
            key: "Bullets",
            exact: "yes",
            policy: "ifv-name",
            bindings: vec![],
            weapon: None,
            warhead: Some("sa"),
            guards: vec![],
            incoming: vec![],
        },
        Spec {
            section: "SAPC",
            key: "Points",
            exact: "25",
            policy: "points-model",
            bindings: vec![("soviet_amphibious_transport", "SAPC", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "SAPC", "Cost", Some("900"))],
            incoming: vec![],
        },
        Spec {
            section: "SENGINEER",
            key: "Points",
            exact: "5",
            policy: "points-model",
            bindings: vec![("engineer", "SENGINEER", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "SENGINEER", "Cost", Some("500"))],
            incoming: vec![],
        },
        Spec {
            section: "SHAD",
            key: "Points",
            exact: "15",
            policy: "points-model",
            bindings: vec![("nighthawk", "SHAD", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "SHAD", "Cost", Some("1000"))],
            incoming: vec![],
        },
        Spec {
            section: "SHK",
            key: "Points",
            exact: "5",
            policy: "points-model",
            bindings: vec![("tesla_trooper", "SHK", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "SHK", "Cost", Some("500"))],
            incoming: vec![],
        },
        Spec {
            section: "SMCV",
            key: "Points",
            exact: "60",
            policy: "points-model",
            bindings: vec![("soviet_mcv", "SMCV", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "SMCV", "Cost", Some("3000"))],
            incoming: vec![],
        },
        Spec {
            section: "SNIPE",
            key: "Points",
            exact: "10",
            policy: "points-model",
            bindings: vec![("sniper", "SNIPE", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "SNIPE", "Cost", Some("600"))],
            incoming: vec![],
        },
        Spec {
            section: "SPY",
            key: "Points",
            exact: "5",
            policy: "points-model",
            bindings: vec![("spy", "SPY", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "SPY", "Cost", Some("1000"))],
            incoming: vec![],
        },
        Spec {
            section: "SQD",
            key: "Points",
            exact: "20",
            policy: "points-model",
            bindings: vec![("giant_squid", "SQD", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "SQD", "Cost", Some("1000"))],
            incoming: vec![],
        },
        Spec {
            section: "SREF",
            key: "Points",
            exact: "50",
            policy: "points-model",
            bindings: vec![("prism_tank", "SREF", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "SREF", "Cost", Some("1200"))],
            incoming: vec![],
        },
        Spec {
            section: "SSA",
            key: "Bullets",
            exact: "yes",
            policy: "ifv-name",
            bindings: vec![],
            weapon: None,
            warhead: Some("ssa"),
            guards: vec![],
            incoming: vec![],
        },
        Spec {
            section: "SSAB",
            key: "Bullets",
            exact: "yes",
            policy: "ifv-name",
            bindings: vec![],
            weapon: None,
            warhead: Some("occupy"),
            guards: vec![],
            incoming: vec![],
        },
        Spec {
            section: "SUB",
            key: "Points",
            exact: "30",
            policy: "points-model",
            bindings: vec![("typhoon", "SUB", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "SUB", "Cost", Some("1000"))],
            incoming: vec![],
        },
        Spec {
            section: "SuperComet",
            key: "IsHouseColor",
            exact: "true",
            policy: "house-color",
            bindings: vec![],
            weapon: Some("super_comet"),
            warhead: None,
            guards: vec![
                ("rules", "SuperComet", "IsLaser", Some("true")),
                ("rules", "SuperComet", "Projectile", Some("SuperCometP")),
                ("rules", "SuperComet", "Warhead", Some("CometWH")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "SuperComet",
            key: "IsLaser",
            exact: "true",
            policy: "nonbuilding-laser",
            bindings: vec![],
            weapon: Some("super_comet"),
            warhead: None,
            guards: vec![
                ("rules", "SuperComet", "IsLaser", Some("true")),
                ("rules", "SuperComet", "Projectile", Some("SuperCometP")),
                ("rules", "SuperComet", "Warhead", Some("CometWH")),
            ],
            incoming: vec![
                ("SREF", "EliteWeapon1", "SuperComet"),
                ("VehicleTypes", "37", "SREF"),
            ],
        },
        Spec {
            section: "SuperCometFragment",
            key: "IsHouseColor",
            exact: "true",
            policy: "house-color",
            bindings: vec![],
            weapon: None,
            warhead: None,
            guards: vec![
                ("rules", "SuperCometFragment", "IsLaser", Some("true")),
                (
                    "rules",
                    "SuperCometFragment",
                    "Projectile",
                    Some("SuperSmallCometP"),
                ),
                ("rules", "SuperCometFragment", "Warhead", Some("CometWH")),
            ],
            incoming: vec![],
        },
        Spec {
            section: "SuperCometFragment",
            key: "IsLaser",
            exact: "true",
            policy: "nonbuilding-laser",
            bindings: vec![],
            weapon: None,
            warhead: None,
            guards: vec![
                ("rules", "SuperCometFragment", "IsLaser", Some("true")),
                (
                    "rules",
                    "SuperCometFragment",
                    "Projectile",
                    Some("SuperSmallCometP"),
                ),
                ("rules", "SuperCometFragment", "Warhead", Some("CometWH")),
            ],
            incoming: vec![
                ("SREF", "EliteWeapon1", "SuperComet"),
                ("SuperComet", "Projectile", "SuperCometP"),
                ("SuperCometP", "ShrapnelWeapon", "SuperCometFragment"),
                ("VehicleTypes", "37", "SREF"),
            ],
        },
        Spec {
            section: "TANY",
            key: "Points",
            exact: "50",
            policy: "points-model",
            bindings: vec![("tanya", "TANY", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "TANY", "Cost", Some("1000"))],
            incoming: vec![],
        },
        Spec {
            section: "TERROR",
            key: "Points",
            exact: "5",
            policy: "points-model",
            bindings: vec![("terrorist", "TERROR", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "TERROR", "Cost", Some("200"))],
            incoming: vec![],
        },
        Spec {
            section: "TESLA",
            key: "Points",
            exact: "30",
            policy: "points-model",
            bindings: vec![("tesla_coil", "TESLA", true)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "TESLA", "Cost", Some("1500"))],
            incoming: vec![],
        },
        Spec {
            section: "TNKD",
            key: "Points",
            exact: "25",
            policy: "points-model",
            bindings: vec![("tank_destroyer", "TNKD", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "TNKD", "Cost", Some("900"))],
            incoming: vec![],
        },
        Spec {
            section: "TTNK",
            key: "Points",
            exact: "25",
            policy: "points-model",
            bindings: vec![("tesla_tank", "TTNK", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "TTNK", "Cost", Some("1200"))],
            incoming: vec![],
        },
        Spec {
            section: "V3",
            key: "Points",
            exact: "40",
            policy: "points-model",
            bindings: vec![("v3_launcher", "V3", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "V3", "Cost", Some("800"))],
            incoming: vec![],
        },
        Spec {
            section: "V3ROCKET",
            key: "Points",
            exact: "20",
            policy: "points-model",
            bindings: vec![("v3_rocket", "V3ROCKET", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "V3ROCKET", "Cost", Some("50"))],
            incoming: vec![],
        },
        Spec {
            section: "YURI",
            key: "Points",
            exact: "5",
            policy: "points-model",
            bindings: vec![("yuri", "YURI", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "YURI", "Cost", Some("1200"))],
            incoming: vec![],
        },
        Spec {
            section: "YURIPR",
            key: "Points",
            exact: "50",
            policy: "points-model",
            bindings: vec![("yuri_prime", "YURIPR", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "YURIPR", "Cost", Some("2000"))],
            incoming: vec![],
        },
        Spec {
            section: "ZEP",
            key: "Points",
            exact: "100",
            policy: "points-model",
            bindings: vec![("kirov", "ZEP", false)],
            weapon: None,
            warhead: None,
            guards: vec![("rules", "ZEP", "Cost", Some("2000"))],
            incoming: vec![],
        },
    ]
}
fn compared(a: &Audit, consumer: &str, field: &str) -> AuditResult<()> {
    let id = a.physical(WitnessId::new(consumer, field));
    if !a.required.contains(&id) {
        return Err(format!(
            "metadata policy lacks actual independent demand {id:?}"
        ));
    }
    match a.witnesses.get(&id) {
        Some(Witness::Compared { dependencies, .. }) => {
            let expected = a
                .required_dependencies
                .iter()
                .filter(|(w, _)| w == &id)
                .map(|(_, atom)| atom.clone())
                .collect::<BTreeSet<_>>();
            if &expected != dependencies {
                return Err(format!(
                    "metadata actual comparison dependency mismatch {id:?}"
                ));
            }
            Ok(())
        }
        _ => Err(format!(
            "metadata policy needs successful actual comparison, not Own/alias/read {id:?}"
        )),
    }
}
fn mapped_weapon_controls(a: &Audit, input: &Inputs, d: &Definitions, s: &Spec) -> AuditResult<()> {
    let found = input
        .weapons
        .iter()
        .filter(|(_, section)| section == s.section)
        .map(|(id, _)| id.as_str())
        .collect::<Vec<_>>();
    if found != s.weapon.into_iter().collect::<Vec<_>>() {
        return Err("metadata source weapon mapping/helper-absence changed".into());
    }
    if let Some(id) = s.weapon {
        let k = d.weapon_kind(id).ok_or("metadata actual weapon missing")?;
        d.weapon(k)
            .ok_or("metadata actual weapon definition missing")?;
        for field in ["damage", "range", "reload", "warhead", "projectile_speed"] {
            compared(
                a,
                &format!("weapon.{id}@rules:{}", s.section),
                &format!("combat.weapons.{field}"),
            )?;
        }
    }
    Ok(())
}
fn nonbuilding_laser(input: &Inputs, d: &Definitions, s: &Spec) -> AuditResult<()> {
    let mut seen = BTreeSet::from([s.section.to_owned()]);
    let mut todo = vec![s.section.to_owned()];
    let mut edges = BTreeSet::new();
    while let Some(target) = todo.pop() {
        for (k, v) in &input.reference.rows {
            if k.file == "rules" && v.split(',').any(|part| part == target) {
                edges.insert((k.section.clone(), k.key.clone(), v.clone()));
                if seen.insert(k.section.clone()) {
                    todo.push(k.section.clone());
                }
            }
        }
    }
    let expected = s
        .incoming
        .iter()
        .map(|(s, k, v)| (s.to_string(), k.to_string(), v.to_string()))
        .collect::<BTreeSet<_>>();
    if edges != expected {
        return Err("nonbuilding IsLaser incoming source graph changed".into());
    }
    if input
        .mappings
        .iter()
        .any(|m| m.building && seen.contains(&m.section))
    {
        return Err("IsLaser now has a native building source consumer; Primary-data behavior needs real disposition".into());
    }
    if let Some(id) = s.weapon {
        let weapon = d.weapon_kind(id).ok_or("actual laser missing")?;
        for (index, _) in d.buildings().iter().enumerate() {
            let kind = sim::BuildingKind(u16::try_from(index).map_err(|e| e.to_string())?);
            if let Some(t) = d.building_combat(kind)
                && (t.weapon == Some(weapon)
                    || t.secondary == Some(weapon)
                    || t.elite.contains(&Some(weapon)))
            {
                return Err("IsLaser has an actual loaded building consumer".into());
            }
        }
    }
    Ok(())
}
pub(super) fn require(
    a: &Audit,
    input: &Inputs,
    d: &Definitions,
    row: &Unused,
    approval: Option<&Approval<'_>>,
) -> AuditResult<()> {
    let approval = approval.ok_or(
        "PENDING root recorded metadata/model approval and live whole-source census byte gate",
    )?;
    if approval.recorded_decision.trim().is_empty()
        || approval.source_census_sha256
            != include_str!("ra9-approved-model-census-sha256.txt").trim()
    {
        return Err("metadata approval has no recorded exact source/model scope".into());
    }
    // This argument is a required approval record, not self-verifying proof. Root must first
    // independently verify live source bytes against the pinned census; refresh on source change.
    let s = specifications()
        .into_iter()
        .find(|s| row.atom.key == SourceKey::new("rules", s.section, s.key))
        .ok_or("source atom has no exact metadata disposition")?;
    if row.atom.portion != Portion::Whole || input.reference.shapes.contains_key(&row.atom.key) {
        return Err("metadata Whole policy cannot hide tuple/components".into());
    }
    if row.exact_value != s.exact
        || input.reference.rows.get(&row.atom.key).map(String::as_str) != Some(s.exact)
    {
        return Err("metadata exact source value/presence changed".into());
    }
    let mut unique = BTreeSet::new();
    for k in input.reference.rows.keys() {
        if !unique.insert((
            k.file.to_ascii_lowercase(),
            k.section.to_ascii_lowercase(),
            k.key.to_ascii_lowercase(),
        )) {
            return Err("metadata case-alias source duplication".into());
        }
    }
    if a.required_dependencies.iter().any(|(_,atom)|atom.key==row.atom.key)||a.witnesses.values().any(|w|matches!(w,Witness::Compared{dependencies,..} if dependencies.iter().any(|atom|atom.key==row.atom.key))){return Err("demanded/successful source cannot also be metadata-unused".into());}
    for (f, section, key, value) in &s.guards {
        if input
            .reference
            .rows
            .get(&SourceKey::new(f, section, key))
            .map(String::as_str)
            != *value
        {
            return Err(format!("metadata active premise changed {section}.{key}"));
        }
    }
    match s.policy {
        "points-model" => {
            let bindings = input
                .mappings
                .iter()
                .filter(|m| m.section == s.section)
                .map(|m| (m.game_id.as_str(), m.section.as_str(), m.building))
                .collect::<BTreeSet<_>>();
            if bindings != s.bindings.iter().copied().collect() {
                return Err("Points exact mapped consumer set changed".into());
            }
            for (id, section, building) in &s.bindings {
                let item = if *building {
                    d.building_kind(id).map(sim::ProductionItem::Building)
                } else {
                    d.unit_kind(id).map(sim::ProductionItem::Unit)
                }
                .ok_or("actual Points item missing")?;
                d.production(item)
                    .ok_or("actual Cost-bearing production terms missing")?;
                compared(
                    a,
                    &format!("{id}@rules:{section}"),
                    "economy.production.cost",
                )?;
            }
        }
        "house-color" | "rad-beam" | "rad-umbrella" => mapped_weapon_controls(a, input, d, &s)?,
        "nonbuilding-laser" => {
            mapped_weapon_controls(a, input, d, &s)?;
            nonbuilding_laser(input, d, &s)?;
        }
        "ifv-name" => {
            let found = input
                .warheads
                .iter()
                .filter(|(_, section)| section == s.section)
                .map(|(id, _)| id.as_str())
                .collect::<Vec<_>>();
            if found != s.warhead.into_iter().collect::<Vec<_>>() {
                return Err("Bullets source warhead mapping changed".into());
            }
            if let Some(id) = s.warhead {
                let k = d.warhead_kind(id).ok_or("actual Bullets warhead missing")?;
                d.warhead(k).ok_or("actual Bullets warhead absent")?;
                for field in ["versus", "wall", "wood"] {
                    compared(
                        a,
                        &format!("warhead.{id}@rules:{}", s.section),
                        &format!("combat.warheads.{field}"),
                    )?;
                }
            }
        }
        "ifv-turret-art" => {
            let k = d.unit_kind("ifv").ok_or("actual IFV missing")?;
            let m = d
                .metadata(sim::ProductionItem::Unit(k))
                .ok_or("actual IFV metadata missing")?;
            if !m.gunner || m.ifv_weapons.len() != 13 {
                return Err("actual thirteen-mode Gunner premise changed".into());
            }
            let index = s.exact.parse::<u32>().map_err(|e| e.to_string())?;
            if index >= 4 {
                return Err("native four-turret art index outside exact shipped boundary".into());
            }
            super::array_plans::run(input, d)?;
        }
        "sw-animation" => {
            for (id, section, building) in &s.bindings {
                if !building {
                    return Err("superweapon animation is not a building".into());
                }
                let k = d.building_kind(id).ok_or("actual SW building missing")?;
                let m = d
                    .metadata(sim::ProductionItem::Building(k))
                    .ok_or("actual SW metadata missing")?;
                if !m.role.major_superweapon() || m.cooldown == 0 {
                    return Err("actual charged ability premise changed".into());
                }
                for field in ["cooldown", "is_powered"] {
                    compared(
                        a,
                        &format!("{id}@rules:{section}"),
                        &format!("arsenal.{field}"),
                    )?;
                }
            }
        }
        "rad-light" | "rad-tint" => {
            for field in [
                "application_delay",
                "level_delay",
                "level_factor_pm",
                "level_max",
                "duration_multiple_pm",
            ] {
                compared(a, "rules", &format!("rules.radiation.{field}"))?;
            }
        }
        "ivan-icon" => compared(a, "rules", "rules.combat.ivan_delay")?,
        "mirage-art" => {
            if s.exact.split(',').count() != 4
                || s.exact.split(',').collect::<BTreeSet<_>>().len() != 4
            {
                return Err("exact four native Mirage art identities changed".into());
            }
            let k = d.unit_kind("mirage_tank").ok_or("actual Mirage missing")?;
            if !d
                .metadata(sim::ProductionItem::Unit(k))
                .is_some_and(|m| m.disguise_when_still)
            {
                return Err("actual still-disguise gameplay premise changed".into());
            }
            compared(a, "mirage_tank@rules:MGTK", "arsenal.disguise_when_still")?;
        }
        _ => return Err("unknown metadata policy".into()),
    }
    Ok(())
}

pub(super) fn contains(key: &SourceKey) -> bool {
    key.file == "rules"
        && specifications()
            .iter()
            .any(|s| s.section == key.section && s.key == key.key)
}
// Keep the exact typed source-membership API checked; this grants no coverage.
const _: fn(&SourceKey) -> bool = contains;

#[cfg(test)]
mod tests {
    use super::*;
    fn classified(section: &str, key: &str) -> AuditResult<Unused> {
        super::super::source_routes::load_typed(
            &super::super::super::reference_dir().join("ra9_source_dispositions.tsv"),
        )?
        .into_iter()
        .map(|r| r.row)
        .find(|r| r.atom.key == SourceKey::new("rules", section, key))
        .ok_or_else(|| "actual accepted metadata source row missing".into())
    }
    fn approval(row: &Unused) -> AuditResult<Approval<'static>> {
        let _scope =
            super::super::scope_inputs::ObservedScope::read(&super::super::super::reference_dir())?;
        let accepted = super::super::policy_admission::source_acceptance(&row.atom.key)?
            .ok_or("accepted metadata record missing")?;
        Ok(Approval {
            recorded_decision: accepted.recorded_decision,
            source_census_sha256: include_str!("ra9-approved-model-census-sha256.txt").trim(),
        })
    }
    fn point_control() -> AuditResult<(Inputs, Definitions, Audit, Unused)> {
        let references = super::super::super::reference_dir();
        super::super::audit_at(&super::super::super::data_dir(), &references)?;
        let input = Inputs::load(&super::super::super::data_dir(), &references)?;
        let d = sim::load_ruleset(&input.data_dir).map_err(|e| e.to_string())?;
        let mut a = super::super::load_audit(&input, &d)?;
        let skirmish = sim::load_skirmish(input.data_dir.join("skirmish.ron"), &d)
            .map_err(|e| e.to_string())?;
        let errors = super::super::run_adapters(&mut a, &input, &d, &skirmish);
        assert!(errors.is_empty(), "{errors:?}");
        let row = classified("ZEP", "Points")?;
        require(&a, &input, &d, &row, Some(&approval(&row)?))?;
        Ok((input, d, a, row))
    }
    #[test]
    fn ra9_metadata_nonfirst_points_value_change_is_not_silent_coverage() -> AuditResult<()> {
        let (mut input, d, a, row) = point_control()?;
        input
            .reference
            .rows
            .insert(SourceKey::new("rules", "ZEP", "Points"), "101".into());
        assert!(require(&a, &input, &d, &row, Some(&approval(&row)?)).is_err());
        Ok(())
    }
    #[test]
    fn ra9_metadata_points_does_not_mask_nonfirst_actual_cost_mutation() -> AuditResult<()> {
        let _control = point_control()?;
        let copy = super::super::array_negatives::DataCopy::replace_row(
            "economy.ron",
            "(unit: \"kirov\",",
            "cost: 2000",
            "cost: 2001",
        )?;
        let (input, d) = copy.loaded()?;
        let mut a = super::super::load_audit(&input, &d)?;
        let failure =
            super::super::actual::core_item(&mut a, &input.reference, &d, "kirov", "ZEP", false)
                .err()
                .ok_or("actual changed Cost escaped source comparison")?;
        if !failure.contains("cost") {
            return Err(format!("wrong failure for actual Cost mutation: {failure}"));
        }
        Ok(())
    }
    #[test]
    fn ra9_metadata_house_color_stops_if_native_laser_premise_changes() -> AuditResult<()> {
        let (mut input, d, a, _) = point_control()?;
        let row = classified("Comet", "IsHouseColor")?;
        require(&a, &input, &d, &row, Some(&approval(&row)?))?;
        input
            .reference
            .rows
            .insert(SourceKey::new("rules", "Comet", "IsLaser"), "false".into());
        assert!(require(&a, &input, &d, &row, Some(&approval(&row)?)).is_err());
        Ok(())
    }
    #[test]
    fn ra9_metadata_laser_stops_when_a_building_source_consumer_is_added() -> AuditResult<()> {
        let (mut input, d, a, _) = point_control()?;
        let row = classified("Comet", "IsLaser")?;
        require(&a, &input, &d, &row, Some(&approval(&row)?))?;
        input.reference.rows.insert(
            SourceKey::new("rules", "ATESLA", "Secondary"),
            "Comet".into(),
        );
        assert!(require(&a, &input, &d, &row, Some(&approval(&row)?)).is_err());
        Ok(())
    }
}
