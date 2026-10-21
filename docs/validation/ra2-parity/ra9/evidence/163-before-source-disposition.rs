//! UNVALIDATED/UNRUN static-unused proposal guard. Reading never grants source coverage.
//! Integrate only after root final source approval; no generic whitelist/default/Own escape.
use super::{
    Atom, Audit, AuditResult, Definitions, Inputs, Portion, SourceKey, Unused, Witness, WitnessId,
};
use std::collections::BTreeSet;
pub(super) struct Approval<'a> {
    pub(super) decision_id: &'a str,
    pub(super) scoped_evidence_sha256: &'a str,
}
struct Spec {
    file: &'static str,
    section: &'static str,
    key: &'static str,
    exact: &'static str,
    family: u16,
    game_id: &'static str,
    guards: Vec<(
        &'static str,
        &'static str,
        &'static str,
        Option<&'static str>,
    )>,
    bindings: Vec<(&'static str, &'static str)>,
}
fn specifications() -> Vec<Spec> {
    vec![
        Spec {
            file: "art",
            section: "CAAIRP",
            key: "Height",
            exact: "6",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "CAAIRP", "Image", None),
                ("art", "CAAIRP", "Foundation", Some("3x3")),
            ],
            bindings: vec![("airport", "CAAIRP")],
        },
        Spec {
            file: "art",
            section: "CABHUT",
            key: "Height",
            exact: "1",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "CABHUT", "Image", None),
                ("art", "CABHUT", "Foundation", Some("1x1")),
            ],
            bindings: vec![("bridge_hut", "CABHUT")],
        },
        Spec {
            file: "art",
            section: "CAEUR1",
            key: "Height",
            exact: "3",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "CAEUR1", "Image", None),
                ("art", "CAEUR1", "Foundation", Some("2x2")),
            ],
            bindings: vec![("civilian_house", "CAEUR1")],
        },
        Spec {
            file: "art",
            section: "CAHOSP",
            key: "Height",
            exact: "7",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "CATHOSP", "Image", Some("CAHOSP")),
                ("art", "CAHOSP", "Foundation", Some("6x4")),
            ],
            bindings: vec![("hospital", "CATHOSP")],
        },
        Spec {
            file: "art",
            section: "CAOILD",
            key: "Height",
            exact: "5",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "CAOILD", "Image", None),
                ("art", "CAOILD", "Foundation", Some("2x2")),
            ],
            bindings: vec![("oil_derrick", "CAOILD")],
        },
        Spec {
            file: "art",
            section: "CAOUTP",
            key: "Height",
            exact: "6",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "CAOUTP", "Image", None),
                ("art", "CAOUTP", "Foundation", Some("4x3")),
            ],
            bindings: vec![("tech_outpost", "CAOUTP")],
        },
        Spec {
            file: "art",
            section: "GAAIRC",
            key: "Height",
            exact: "7",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "GAAIRC", "Image", None),
                ("art", "GAAIRC", "Foundation", Some("3x2")),
                ("rules", "AMRADR", "Image", Some("GAAIRC")),
                ("art", "GAAIRC", "Foundation", Some("3x2")),
            ],
            bindings: vec![
                ("airforce_command", "GAAIRC"),
                ("american_airforce_command", "AMRADR"),
            ],
        },
        Spec {
            file: "art",
            section: "GACNST",
            key: "Height",
            exact: "4",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "GACNST", "Image", None),
                ("art", "GACNST", "Foundation", Some("4x4")),
            ],
            bindings: vec![("construction_yard", "GACNST")],
        },
        Spec {
            file: "art",
            section: "GACSPH",
            key: "Height",
            exact: "3",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "GACSPH", "Image", None),
                ("art", "GACSPH", "Foundation", Some("4x3")),
            ],
            bindings: vec![("chronosphere", "GACSPH")],
        },
        Spec {
            file: "art",
            section: "GADEPT",
            key: "Height",
            exact: "3",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "GADEPT", "Image", None),
                ("art", "GADEPT", "Foundation", Some("3x3")),
            ],
            bindings: vec![("service_depot", "GADEPT")],
        },
        Spec {
            file: "art",
            section: "GAGAP",
            key: "Height",
            exact: "6",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "GAGAP", "Image", None),
                ("art", "GAGAP", "Foundation", Some("1x1")),
            ],
            bindings: vec![("gap_generator", "GAGAP")],
        },
        Spec {
            file: "art",
            section: "GAOREP",
            key: "Height",
            exact: "4",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "GAOREP", "Image", None),
                ("art", "GAOREP", "Foundation", Some("3x3")),
            ],
            bindings: vec![("ore_purifier", "GAOREP")],
        },
        Spec {
            file: "art",
            section: "GAPILE",
            key: "Height",
            exact: "4",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "GAPILE", "Image", None),
                ("art", "GAPILE", "Foundation", Some("3x2")),
            ],
            bindings: vec![("barracks", "GAPILE")],
        },
        Spec {
            file: "art",
            section: "GAPILL",
            key: "Height",
            exact: "1",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "GAPILL", "Image", None),
                ("art", "GAPILL", "Foundation", Some("1x1")),
            ],
            bindings: vec![("pillbox", "GAPILL")],
        },
        Spec {
            file: "art",
            section: "GAPOWR",
            key: "Height",
            exact: "4",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "GAPOWR", "Image", None),
                ("art", "GAPOWR", "Foundation", Some("2x2")),
            ],
            bindings: vec![("power_plant", "GAPOWR")],
        },
        Spec {
            file: "art",
            section: "GAPRIS",
            key: "Height",
            exact: "6",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "ATESLA", "Image", Some("GAPRIS")),
                ("art", "GAPRIS", "Foundation", Some("1x1")),
            ],
            bindings: vec![("prism_tower", "ATESLA")],
        },
        Spec {
            file: "art",
            section: "GAREFN",
            key: "Height",
            exact: "4",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "GAREFN", "Image", None),
                ("art", "GAREFN", "Foundation", Some("4x3")),
            ],
            bindings: vec![("refinery", "GAREFN")],
        },
        Spec {
            file: "art",
            section: "GASPST",
            key: "Height",
            exact: "5",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "GASPYSAT", "Image", Some("GASPST")),
                ("art", "GASPST", "Foundation", Some("2x2")),
            ],
            bindings: vec![("spy_satellite", "GASPYSAT")],
        },
        Spec {
            file: "art",
            section: "GATECH",
            key: "Height",
            exact: "12",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "GATECH", "Image", None),
                ("art", "GATECH", "Foundation", Some("3x2")),
            ],
            bindings: vec![("tech_center", "GATECH")],
        },
        Spec {
            file: "art",
            section: "GAWEAP",
            key: "Height",
            exact: "4",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "GAWEAP", "Image", None),
                ("art", "GAWEAP", "Foundation", Some("5x3")),
            ],
            bindings: vec![("war_factory", "GAWEAP")],
        },
        Spec {
            file: "art",
            section: "GAWETH",
            key: "Height",
            exact: "5",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "GAWEAT", "Image", Some("GAWETH")),
                ("art", "GAWETH", "Foundation", Some("3x3")),
            ],
            bindings: vec![("weather_control", "GAWEAT")],
        },
        Spec {
            file: "art",
            section: "GAYARD",
            key: "Height",
            exact: "10",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "GAYARD", "Image", None),
                ("art", "GAYARD", "Foundation", Some("4x4")),
            ],
            bindings: vec![("naval_shipyard", "GAYARD")],
        },
        Spec {
            file: "art",
            section: "GTGCAN",
            key: "Height",
            exact: "3",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "GTGCAN", "Image", None),
                ("art", "GTGCAN", "Foundation", Some("2x2")),
            ],
            bindings: vec![("grand_cannon", "GTGCAN")],
        },
        Spec {
            file: "art",
            section: "NACLON",
            key: "Height",
            exact: "6",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "NACLON", "Image", None),
                ("art", "NACLON", "Foundation", Some("2x2")),
            ],
            bindings: vec![("cloning_vats", "NACLON")],
        },
        Spec {
            file: "art",
            section: "NACNST",
            key: "Height",
            exact: "6",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "NACNST", "Image", None),
                ("art", "NACNST", "Foundation", Some("4x4")),
            ],
            bindings: vec![("soviet_construction_yard", "NACNST")],
        },
        Spec {
            file: "art",
            section: "NADEPT",
            key: "Height",
            exact: "6",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "NADEPT", "Image", None),
                ("art", "NADEPT", "Foundation", Some("4x3")),
            ],
            bindings: vec![("soviet_service_depot", "NADEPT")],
        },
        Spec {
            file: "art",
            section: "NAFLAK",
            key: "Height",
            exact: "4",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "NAFLAK", "Image", None),
                ("art", "NAFLAK", "Foundation", Some("1x1")),
            ],
            bindings: vec![("flak_cannon", "NAFLAK")],
        },
        Spec {
            file: "art",
            section: "NAHAND",
            key: "Height",
            exact: "9",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "NAHAND", "Image", None),
                ("art", "NAHAND", "Foundation", Some("2x2")),
            ],
            bindings: vec![("soviet_barracks", "NAHAND")],
        },
        Spec {
            file: "art",
            section: "NAIRON",
            key: "Height",
            exact: "6",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "NAIRON", "Image", Some("NAIRON")),
                ("art", "NAIRON", "Foundation", Some("3x3")),
            ],
            bindings: vec![("iron_curtain", "NAIRON")],
        },
        Spec {
            file: "art",
            section: "NALASR",
            key: "Height",
            exact: "2",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "NALASR", "Image", None),
                ("art", "NALASR", "Foundation", Some("1x1")),
            ],
            bindings: vec![("sentry_gun", "NALASR")],
        },
        Spec {
            file: "art",
            section: "NAMISL",
            key: "Height",
            exact: "8",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "NAMISL", "Image", None),
                ("art", "NAMISL", "Foundation", Some("3x3")),
            ],
            bindings: vec![("nuclear_silo", "NAMISL")],
        },
        Spec {
            file: "art",
            section: "NANRCT",
            key: "Height",
            exact: "4",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "NANRCT", "Image", None),
                ("art", "NANRCT", "Foundation", Some("4x4")),
            ],
            bindings: vec![("nuclear_reactor", "NANRCT")],
        },
        Spec {
            file: "art",
            section: "NAPOWR",
            key: "Height",
            exact: "3",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "NAPOWR", "Image", None),
                ("art", "NAPOWR", "Foundation", Some("3x2")),
            ],
            bindings: vec![("tesla_reactor", "NAPOWR")],
        },
        Spec {
            file: "art",
            section: "NAPSIS",
            key: "Height",
            exact: "8",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "NAPSIS", "Image", None),
                ("art", "NAPSIS", "Foundation", Some("2x2")),
            ],
            bindings: vec![("psychic_sensor", "NAPSIS")],
        },
        Spec {
            file: "art",
            section: "NARADR",
            key: "Height",
            exact: "6",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "NARADR", "Image", None),
                ("art", "NARADR", "Foundation", Some("2x2")),
            ],
            bindings: vec![("radar_tower", "NARADR")],
        },
        Spec {
            file: "art",
            section: "NAREFN",
            key: "Height",
            exact: "6",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "NAREFN", "Image", None),
                ("art", "NAREFN", "Foundation", Some("4x3")),
            ],
            bindings: vec![("soviet_refinery", "NAREFN")],
        },
        Spec {
            file: "art",
            section: "NASAM",
            key: "Height",
            exact: "3",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "NASAM", "Image", None),
                ("art", "NASAM", "Foundation", Some("1x1")),
            ],
            bindings: vec![("patriot", "NASAM")],
        },
        Spec {
            file: "art",
            section: "NATECH",
            key: "Height",
            exact: "8",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "NATECH", "Image", None),
                ("art", "NATECH", "Foundation", Some("3x3")),
            ],
            bindings: vec![("soviet_battle_lab", "NATECH")],
        },
        Spec {
            file: "art",
            section: "NATSLA",
            key: "Height",
            exact: "5",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "TESLA", "Image", Some("NATSLA")),
                ("art", "NATSLA", "Foundation", Some("1x1")),
            ],
            bindings: vec![("tesla_coil", "TESLA")],
        },
        Spec {
            file: "art",
            section: "NAWEAP",
            key: "Height",
            exact: "6",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "NAWEAP", "Image", None),
                ("art", "NAWEAP", "Foundation", Some("5x3")),
            ],
            bindings: vec![("soviet_war_factory", "NAWEAP")],
        },
        Spec {
            file: "art",
            section: "NAYARD",
            key: "Height",
            exact: "10",
            family: 0,
            game_id: "",
            guards: vec![
                ("rules", "NAYARD", "Image", None),
                ("art", "NAYARD", "Foundation", Some("4x4")),
            ],
            bindings: vec![("soviet_naval_shipyard", "NAYARD")],
        },
        Spec {
            file: "rules",
            section: "20mm",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "gun_20mm",
            guards: vec![
                ("rules", "20mm", "Projectile", Some("Invisible3")),
                ("rules", "Invisible3", "Inviso", Some("yes")),
                ("rules", "20mm", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "20mmE",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "gun_20mm_e",
            guards: vec![
                ("rules", "20mmE", "Projectile", Some("Invisible3")),
                ("rules", "Invisible3", "Inviso", Some("yes")),
                ("rules", "20mmE", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "20mmRapid",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "gun_20mm_rapid",
            guards: vec![
                ("rules", "20mmRapid", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "20mmRapid", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "AWP",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "awp",
            guards: vec![
                ("rules", "AWP", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "AWP", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "AWPE",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "awp_e",
            guards: vec![
                ("rules", "AWPE", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "AWPE", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "AlliedOccupyW",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "allied_occupy",
            guards: vec![
                (
                    "rules",
                    "AlliedOccupyW",
                    "Projectile",
                    Some("InvisibleHigh"),
                ),
                ("rules", "InvisibleHigh", "Inviso", Some("yes")),
                ("rules", "AlliedOccupyW", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "AssaultBolt",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "assault_bolt",
            guards: vec![
                ("rules", "AssaultBolt", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "AssaultBolt", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "BadTeeth",
            key: "Speed",
            exact: "30",
            family: 2,
            game_id: "good_teeth",
            guards: vec![
                ("rules", "BadTeeth", "Projectile", Some("DOGJUMP")),
                ("rules", "DOGJUMP", "Inviso", None),
                ("rules", "BadTeeth", "LimboLaunch", Some("yes")),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "BlackHawkCannon",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "black_hawk_cannon",
            guards: vec![
                ("rules", "BlackHawkCannon", "Projectile", Some("QuadShell")),
                ("rules", "QuadShell", "Inviso", Some("yes")),
                ("rules", "BlackHawkCannon", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "BlackHawkCannonE",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "black_hawk_cannon_e",
            guards: vec![
                ("rules", "BlackHawkCannonE", "Projectile", Some("QuadShell")),
                ("rules", "QuadShell", "Inviso", Some("yes")),
                ("rules", "BlackHawkCannonE", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "CRElectricBolt",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "cr_electric_bolt",
            guards: vec![
                (
                    "rules",
                    "CRElectricBolt",
                    "Projectile",
                    Some("InvisibleLow"),
                ),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "CRElectricBolt", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "CRM60",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "crm60",
            guards: vec![
                ("rules", "CRM60", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "CRM60", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "CRMP5",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "crmp5",
            guards: vec![
                ("rules", "CRMP5", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "CRMP5", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "CRMindControl",
            key: "Speed",
            exact: "1",
            family: 2,
            game_id: "cr_mind_control",
            guards: vec![
                ("rules", "CRMindControl", "Projectile", Some("Psychic")),
                ("rules", "Psychic", "Inviso", Some("yes")),
                ("rules", "CRMindControl", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "CRNeutronRifle",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "cr_neutron_rifle",
            guards: vec![
                (
                    "rules",
                    "CRNeutronRifle",
                    "Projectile",
                    Some("InvisibleLow"),
                ),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "CRNeutronRifle", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "CRNuke",
            key: "Speed",
            exact: "35",
            family: 2,
            game_id: "cr_nuke",
            guards: vec![
                ("rules", "CRNuke", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "CRNuke", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "CRRadBeamWeapon",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "cr_rad_beam_weapon",
            guards: vec![
                (
                    "rules",
                    "CRRadBeamWeapon",
                    "Projectile",
                    Some("InvisibleLow"),
                ),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "CRRadBeamWeapon", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "ChronoMP5",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "chrono_mp5",
            guards: vec![
                ("rules", "ChronoMP5", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "ChronoMP5", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "ChronoMP5E",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "chrono_mp5_e",
            guards: vec![
                ("rules", "ChronoMP5E", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "ChronoMP5E", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "CoilBolt",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "coil_bolt",
            guards: vec![
                ("rules", "CoilBolt", "Projectile", Some("Invisible")),
                ("rules", "Invisible", "Inviso", Some("yes")),
                ("rules", "CoilBolt", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "Comet",
            key: "Speed",
            exact: "40",
            family: 2,
            game_id: "comet",
            guards: vec![
                ("rules", "Comet", "Projectile", Some("LargeCometP")),
                ("rules", "LargeCometP", "Inviso", Some("yes")),
                ("rules", "Comet", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "CowShot",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "cow_shot",
            guards: vec![
                ("rules", "CowShot", "Projectile", Some("InvisibleHigh")),
                ("rules", "InvisibleHigh", "Inviso", Some("yes")),
                ("rules", "CowShot", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "DefuseKit",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "defuse_kit",
            guards: vec![
                ("rules", "DefuseKit", "Projectile", Some("InvisibleAll")),
                ("rules", "InvisibleAll", "Inviso", Some("yes")),
                ("rules", "DefuseKit", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "Demobomb",
            key: "Speed",
            exact: "35",
            family: 2,
            game_id: "demobomb",
            guards: vec![
                ("rules", "Demobomb", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "Demobomb", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "DoublePistols",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "double_pistols",
            guards: vec![
                ("rules", "DoublePistols", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "DoublePistols", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "DoublePistolsE",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "double_pistols_e",
            guards: vec![
                (
                    "rules",
                    "DoublePistolsE",
                    "Projectile",
                    Some("InvisibleLow"),
                ),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "DoublePistolsE", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "DredLauncher",
            key: "Speed",
            exact: "15",
            family: 2,
            game_id: "dred_launcher",
            guards: vec![
                ("rules", "DredLauncher", "Projectile", Some("InvisibleHigh")),
                ("rules", "InvisibleHigh", "Inviso", Some("yes")),
                ("rules", "DredLauncher", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "DroneJump",
            key: "Speed",
            exact: "30",
            family: 2,
            game_id: "drone_jump",
            guards: vec![
                ("rules", "DroneJump", "Projectile", Some("JUMP")),
                ("rules", "JUMP", "Inviso", None),
                ("rules", "DroneJump", "LimboLaunch", Some("yes")),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "ElectricBolt",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "electric_bolt",
            guards: vec![
                ("rules", "ElectricBolt", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "ElectricBolt", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "ElectricBoltE",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "electric_bolt_e",
            guards: vec![
                (
                    "rules",
                    "ElectricBoltE",
                    "Projectile",
                    Some("Electricbounce"),
                ),
                ("rules", "Electricbounce", "Inviso", Some("yes")),
                ("rules", "ElectricBoltE", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "FakeC4",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "fakec4",
            guards: vec![
                ("rules", "FakeC4", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "FakeC4", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "FlakGuyAAGun",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "flak_guy_aa_gun",
            guards: vec![
                ("rules", "FlakGuyAAGun", "Projectile", Some("FlakProj")),
                ("rules", "FlakProj", "Inviso", Some("yes")),
                ("rules", "FlakGuyAAGun", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "FlakGuyAAGunE",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "flak_guy_aa_gun_e",
            guards: vec![
                ("rules", "FlakGuyAAGunE", "Projectile", Some("FlakProj")),
                ("rules", "FlakProj", "Inviso", Some("yes")),
                ("rules", "FlakGuyAAGunE", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "FlakTrackAAGun",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "flak_track_aa_gun",
            guards: vec![
                ("rules", "FlakTrackAAGun", "Projectile", Some("FlakProj")),
                ("rules", "FlakProj", "Inviso", Some("yes")),
                ("rules", "FlakTrackAAGun", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "FlakTrackAAGunE",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "flak_track_aa_gun_e",
            guards: vec![
                ("rules", "FlakTrackAAGunE", "Projectile", Some("FlakProj")),
                ("rules", "FlakProj", "Inviso", Some("yes")),
                ("rules", "FlakTrackAAGunE", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "FlakWeapon",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "flak_weapon",
            guards: vec![
                ("rules", "FlakWeapon", "Projectile", Some("FlakProj")),
                ("rules", "FlakProj", "Inviso", Some("yes")),
                ("rules", "FlakWeapon", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "FlakWeaponE",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "flak_weapon_e",
            guards: vec![
                ("rules", "FlakWeaponE", "Projectile", Some("FlakProj")),
                ("rules", "FlakProj", "Inviso", Some("yes")),
                ("rules", "FlakWeaponE", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "GoodTeeth",
            key: "Speed",
            exact: "30",
            family: 2,
            game_id: "good_teeth",
            guards: vec![
                ("rules", "GoodTeeth", "Projectile", Some("ADOGJUMP")),
                ("rules", "ADOGJUMP", "Inviso", None),
                ("rules", "GoodTeeth", "LimboLaunch", Some("yes")),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "HornetLauncher",
            key: "Speed",
            exact: "10",
            family: 2,
            game_id: "hornet_launcher",
            guards: vec![
                ("rules", "HornetLauncher", "Projectile", Some("Invisible")),
                ("rules", "Invisible", "Inviso", Some("yes")),
                ("rules", "HornetLauncher", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "JUMPJET",
            key: "Speed",
            exact: "9",
            family: 224,
            game_id: "rocketeer",
            guards: vec![
                (
                    "rules",
                    "JUMPJET",
                    "Locomotor",
                    Some("{92612C46-F71F-11d1-AC9F-006008055BB5}"),
                ),
                ("rules", "JUMPJET", "JumpjetSpeed", Some("30")),
            ],
            bindings: vec![("rocketeer", "JUMPJET")],
        },
        Spec {
            file: "rules",
            section: "M1Carbine",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "m1_carbine",
            guards: vec![
                ("rules", "M1Carbine", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "M1Carbine", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "M1CarbineE",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "m1_carbine_e",
            guards: vec![
                ("rules", "M1CarbineE", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "M1CarbineE", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "M60",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "m60",
            guards: vec![
                ("rules", "M60", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "M60", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "M60E",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "m60_e",
            guards: vec![
                ("rules", "M60E", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "M60E", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "MakeupKit",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "makeup_kit",
            guards: vec![
                ("rules", "MakeupKit", "Projectile", Some("InvisibleAll")),
                ("rules", "InvisibleAll", "Inviso", Some("yes")),
                ("rules", "MakeupKit", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "MindControl",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "mind_control",
            guards: vec![
                ("rules", "MindControl", "Projectile", Some("PsychicControl")),
                ("rules", "PsychicControl", "Inviso", Some("yes")),
                ("rules", "MindControl", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "MindControlE",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "mind_control_e",
            guards: vec![
                (
                    "rules",
                    "MindControlE",
                    "Projectile",
                    Some("PsychicControl"),
                ),
                ("rules", "PsychicControl", "Inviso", Some("yes")),
                ("rules", "MindControlE", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "MirageGun",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "mirage_gun",
            guards: vec![
                ("rules", "MirageGun", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "MirageGun", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "MirageGunE",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "mirage_gun_e",
            guards: vec![
                ("rules", "MirageGunE", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "MirageGunE", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "NeutronRifle",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "neutron_rifle",
            guards: vec![
                ("rules", "NeutronRifle", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "NeutronRifle", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "NeutronRifleE",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "neutron_rifle_e",
            guards: vec![
                ("rules", "NeutronRifleE", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "NeutronRifleE", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "OPCoilBolt",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "op_coil_bolt",
            guards: vec![
                ("rules", "OPCoilBolt", "Projectile", Some("InvisibleHigh")),
                ("rules", "InvisibleHigh", "Inviso", Some("yes")),
                ("rules", "OPCoilBolt", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "Para",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "para",
            guards: vec![
                ("rules", "Para", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "Para", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "ParaE",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "para_e",
            guards: vec![
                ("rules", "ParaE", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "ParaE", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "PrismShot",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "prism_shot",
            guards: vec![
                ("rules", "PrismShot", "Projectile", Some("InvisibleHigh")),
                ("rules", "InvisibleHigh", "Inviso", Some("yes")),
                ("rules", "PrismShot", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "PrismSupport",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "prism_support",
            guards: vec![
                ("rules", "PrismSupport", "Projectile", Some("InvisibleHigh")),
                ("rules", "InvisibleHigh", "Inviso", Some("yes")),
                ("rules", "PrismSupport", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "PsiWave",
            key: "Speed",
            exact: "1",
            family: 2,
            game_id: "psi_wave",
            guards: vec![
                ("rules", "PsiWave", "Projectile", Some("Psychic")),
                ("rules", "Psychic", "Inviso", Some("yes")),
                ("rules", "PsiWave", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "RadBeamWeapon",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "rad_beam_weapon",
            guards: vec![
                ("rules", "RadBeamWeapon", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "RadBeamWeapon", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "RadBeamWeaponE",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "rad_beam_weapon_e",
            guards: vec![
                (
                    "rules",
                    "RadBeamWeaponE",
                    "Projectile",
                    Some("InvisibleLow"),
                ),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "RadBeamWeaponE", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "RadEruptionWeapon",
            key: "Speed",
            exact: "1",
            family: 2,
            game_id: "rad_eruption_weapon",
            guards: vec![
                (
                    "rules",
                    "RadEruptionWeapon",
                    "Projectile",
                    Some("InvisibleLow"),
                ),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "RadEruptionWeapon", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "RepairBullet",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "repair_bullet",
            guards: vec![
                ("rules", "RepairBullet", "Projectile", Some("Invisible")),
                ("rules", "Invisible", "Inviso", Some("yes")),
                ("rules", "RepairBullet", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "SHAD",
            key: "Speed",
            exact: "14",
            family: 224,
            game_id: "nighthawk",
            guards: vec![
                (
                    "rules",
                    "SHAD",
                    "Locomotor",
                    Some("{92612C46-F71F-11d1-AC9F-006008055BB5}"),
                ),
                ("rules", "SHAD", "JumpjetSpeed", Some("30")),
            ],
            bindings: vec![("nighthawk", "SHAD")],
        },
        Spec {
            file: "rules",
            section: "SovietOccupyW",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "soviet_occupy",
            guards: vec![
                (
                    "rules",
                    "SovietOccupyW",
                    "Projectile",
                    Some("InvisibleHigh"),
                ),
                ("rules", "InvisibleHigh", "Inviso", Some("yes")),
                ("rules", "SovietOccupyW", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "SquidGrab",
            key: "Speed",
            exact: "30",
            family: 2,
            game_id: "squid_grab",
            guards: vec![
                ("rules", "SquidGrab", "Projectile", Some("SQDJUMP")),
                ("rules", "SQDJUMP", "Inviso", Some("yes")),
                ("rules", "SquidGrab", "LimboLaunch", Some("yes")),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "SquidGrabE",
            key: "Speed",
            exact: "30",
            family: 2,
            game_id: "squid_grab_e",
            guards: vec![
                ("rules", "SquidGrabE", "Projectile", Some("SQDJUMP")),
                ("rules", "SQDJUMP", "Inviso", Some("yes")),
                ("rules", "SquidGrabE", "LimboLaunch", Some("yes")),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "SquidPunch",
            key: "Speed",
            exact: "30",
            family: 2,
            game_id: "squid_punch",
            guards: vec![
                ("rules", "SquidPunch", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "SquidPunch", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "SquidPunchE",
            key: "Speed",
            exact: "30",
            family: 2,
            game_id: "squid_punch_e",
            guards: vec![
                ("rules", "SquidPunchE", "Projectile", Some("InvisibleAll")),
                ("rules", "InvisibleAll", "Inviso", Some("yes")),
                ("rules", "SquidPunchE", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "SuperComet",
            key: "Speed",
            exact: "10",
            family: 2,
            game_id: "super_comet",
            guards: vec![
                ("rules", "SuperComet", "Projectile", Some("SuperCometP")),
                ("rules", "SuperCometP", "Inviso", Some("yes")),
                ("rules", "SuperComet", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "SuperMindControl",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "super_mind_control",
            guards: vec![
                (
                    "rules",
                    "SuperMindControl",
                    "Projectile",
                    Some("PsychicControl"),
                ),
                ("rules", "PsychicControl", "Inviso", Some("yes")),
                ("rules", "SuperMindControl", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "TankBolt",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "tank_bolt",
            guards: vec![
                ("rules", "TankBolt", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "TankBolt", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "TankBoltE",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "tank_bolt_e",
            guards: vec![
                ("rules", "TankBoltE", "Projectile", Some("Electricbounce")),
                ("rules", "Electricbounce", "Inviso", Some("yes")),
                ("rules", "TankBoltE", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "V3Launcher",
            key: "Speed",
            exact: "10",
            family: 2,
            game_id: "v3_launcher",
            guards: vec![
                ("rules", "V3Launcher", "Projectile", Some("InvisibleHigh")),
                ("rules", "InvisibleHigh", "Inviso", Some("yes")),
                ("rules", "V3Launcher", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "Vulcan",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "vulcan",
            guards: vec![
                ("rules", "Vulcan", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "Vulcan", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "Vulcan2",
            key: "Speed",
            exact: "100",
            family: 2,
            game_id: "vulcan2",
            guards: vec![
                ("rules", "Vulcan2", "Projectile", Some("InvisibleLow")),
                ("rules", "InvisibleLow", "Inviso", Some("yes")),
                ("rules", "Vulcan2", "LimboLaunch", None),
            ],
            bindings: vec![],
        },
        Spec {
            file: "rules",
            section: "ZEP",
            key: "Speed",
            exact: "5",
            family: 224,
            game_id: "kirov",
            guards: vec![
                (
                    "rules",
                    "ZEP",
                    "Locomotor",
                    Some("{92612C46-F71F-11d1-AC9F-006008055BB5}"),
                ),
                ("rules", "ZEP", "JumpjetSpeed", Some("5")),
            ],
            bindings: vec![("kirov", "ZEP")],
        },
    ]
}
fn compared(a: &Audit, consumer: &str, field: &str) -> AuditResult<()> {
    let id = a.physical(WitnessId::new(consumer, field));
    if !a.required.contains(&id) {
        return Err(format!(
            "static disposition lacks independent physical demand {id:?}"
        ));
    }
    match a.witnesses.get(&id) {
        Some(Witness::Compared { dependencies, .. }) => {
            let expected = a
                .required_dependencies
                .iter()
                .filter(|(w, _)| w == &id)
                .map(|(_, k)| k.clone())
                .collect::<BTreeSet<_>>();
            if &expected != dependencies {
                return Err(format!(
                    "static disposition comparison dependencies differ {id:?}"
                ));
            }
            Ok(())
        }
        _ => Err(format!(
            "static disposition needs successful actual comparison, not alias/Own/read {id:?}"
        )),
    }
}
pub(super) fn require(
    a: &Audit,
    input: &Inputs,
    d: &Definitions,
    row: &Unused,
    approval: Option<&Approval<'_>>,
) -> AuditResult<()> {
    let approval = approval.ok_or(
        "PENDING recorded root static-disposition approval and actual final-source scope refresh",
    )?;
    if approval.decision_id.trim().is_empty()
        || approval.scoped_evidence_sha256
            != include_str!("ra9-approved-static-evidence-sha256.txt").trim()
    {
        return Err("static disposition lacks approved exact code/evidence scope".into());
    }
    // The scope-gate proposal must independently verify actual current source bytes before
    // compiling/running. A caller-supplied decision string is not itself evidence of that gate.
    let s = specifications()
        .into_iter()
        .find(|s| {
            s.file == row.atom.key.file
                && s.section == row.atom.key.section
                && s.key == row.atom.key.key
        })
        .ok_or_else(|| format!("source atom has no approved static policy {:?}", row.atom))?;
    if row.atom.portion != Portion::Whole || input.reference.shapes.contains_key(&row.atom.key) {
        return Err("static scalar disposition cannot mask tuple/components".into());
    }
    let mut canonical = BTreeSet::new();
    for k in input.reference.rows.keys() {
        let id = (
            k.file.to_ascii_lowercase(),
            k.section.to_ascii_lowercase(),
            k.key.to_ascii_lowercase(),
        );
        if !canonical.insert(id) {
            return Err(format!("stale/case-alias source duplication {k:?}"));
        }
    }
    if row.exact_value != s.exact
        || input.reference.rows.get(&row.atom.key).map(String::as_str) != Some(s.exact)
    {
        return Err(format!(
            "static source value/presence changed {:?}",
            row.atom
        ));
    }
    if a.required_dependencies
        .iter()
        .any(|(_, atom)| atom.key == row.atom.key)
    {
        return Err(format!("demanded source cannot be unused {:?}", row.atom));
    }
    if a.witnesses.values().any(|w|matches!(w,Witness::Compared{dependencies,..} if dependencies.iter().any(|atom|atom.key==row.atom.key))){return Err(format!("successful source cannot be unused {:?}",row.atom));}
    for (f, section, k, expected) in &s.guards {
        let source = SourceKey::new(f, section, k);
        let actual = input.reference.rows.get(&source).map(String::as_str);
        if actual != *expected {
            return Err(format!(
                "inactive premise changed {source:?}: {actual:?} != {expected:?}"
            ));
        }
    }
    match s.family {
        0 => {
            let actual = input
                .mappings
                .iter()
                .filter(|m| m.building)
                .filter(|m| {
                    input
                        .reference
                        .rows
                        .get(&SourceKey::new("rules", &m.section, "Image"))
                        .map_or(m.section.as_str(), String::as_str)
                        == s.section
                })
                .map(|m| (m.game_id.as_str(), m.section.as_str()))
                .collect::<BTreeSet<_>>();
            if actual != s.bindings.iter().copied().collect::<BTreeSet<_>>() {
                return Err("resolved art Image consumer set changed".into());
            }
            for (id, section) in &s.bindings {
                let k = d
                    .building_kind(id)
                    .ok_or_else(|| format!("missing actual building {id}"))?;
                d.building(k).ok_or("missing actual building definition")?;
                for axis in ["width", "height"] {
                    compared(
                        a,
                        &format!("{id}@rules:{section}"),
                        &format!("economy.buildings.{axis}"),
                    )?;
                }
            }
        }
        2 => {
            let found = input
                .weapons
                .iter()
                .filter(|(_, section)| section == s.section)
                .map(|(id, _)| id.as_str())
                .collect::<Vec<_>>();
            if found != vec![s.game_id] {
                return Err("instant source mapping/twin alias changed".into());
            }
            let k = d
                .weapon_kind(s.game_id)
                .ok_or("missing actual instant weapon")?;
            let w = d
                .weapon(k)
                .ok_or("missing actual instant weapon definition")?;
            if w.projectile_speed != 0 {
                return Err("actual instant projectile speed changed".into());
            }
            compared(
                a,
                &format!("weapon.{}@rules:{}", s.game_id, s.section),
                "combat.weapons.projectile_speed",
            )?;
        }
        224 => {
            let found = input
                .mappings
                .iter()
                .filter(|m| !m.building && m.section == s.section)
                .map(|m| m.game_id.as_str())
                .collect::<Vec<_>>();
            if found != vec![s.game_id] {
                return Err("jumpjet source mapping changed".into());
            }
            let k = d.unit_kind(s.game_id).ok_or("missing actual jumpjet")?;
            let u = d.unit(k).ok_or("missing actual jumpjet definition")?;
            if u.speed == 0 {
                return Err("actual jumpjet speed is zero".into());
            }
            compared(
                a,
                &format!("{}@rules:{}", s.game_id, s.section),
                "units.units.speed",
            )?;
        }
        _ => return Err("unknown static branch policy".into()),
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn final_approval(row: &Unused) -> AuditResult<Approval<'static>> {
        let scope =
            super::super::scope_inputs::ObservedScope::read(&super::super::super::reference_dir())?;
        let accepted = scope.accepted_source(row)?;
        let expected = include_str!("ra9-approved-static-evidence-sha256.txt").trim();
        if scope.static_evidence_sha256() != expected {
            return Err("actual final reviewed static scope changed".into());
        }
        Ok(Approval {
            decision_id: accepted.recorded_decision,
            scoped_evidence_sha256: expected,
        })
    }
    fn native_adapter_control() -> AuditResult<(Inputs, Definitions, Audit, Unused)> {
        super::super::audit_at(
            &super::super::super::data_dir(),
            &super::super::super::reference_dir(),
        )?;
        let input = Inputs::load(
            &super::super::super::data_dir(),
            &super::super::super::reference_dir(),
        )?;
        let d = sim::load_ruleset(&input.data_dir).map_err(|e| e.to_string())?;
        let mut a = super::super::load_audit(&input, &d)?;
        super::super::combat::weapon(
            &mut a,
            &input.reference,
            &d,
            "m60",
            "M60",
            &super::super::index(&input.warheads),
        )?;
        let s = specifications()
            .into_iter()
            .find(|s| s.section == "M60" && s.key == "Speed")
            .ok_or("missing exact M60 policy")?;
        let row = Unused {
            atom: Atom {
                key: SourceKey::new(s.file, s.section, s.key),
                portion: Portion::Whole,
            },
            exact_value: s.exact.into(),
            reason: "Exact instant inactive branch test".into(),
            code_evidence: "Actual loaded M60 adapter first succeeds".into(),
        };
        require(&a, &input, &d, &row, Some(&final_approval(&row)?))?;
        Ok((input, d, a, row))
    }
    #[test]
    fn ra9_static_speed_rejects_changed_real_instant_selector() -> AuditResult<()> {
        let (mut input, d, a, row) = native_adapter_control()?;
        input.reference.rows.insert(
            SourceKey::new("rules", "InvisibleLow", "Inviso"),
            "no".into(),
        );
        assert!(require(&a, &input, &d, &row, Some(&final_approval(&row)?)).is_err());
        Ok(())
    }
    #[test]
    fn ra9_static_speed_rejects_case_alias_after_actual_adapter_control() -> AuditResult<()> {
        let (mut input, d, a, row) = native_adapter_control()?;
        input.reference.rows.insert(
            SourceKey::new("rules", "M60", "speed"),
            row.exact_value.clone(),
        );
        assert!(require(&a, &input, &d, &row, Some(&final_approval(&row)?)).is_err());
        Ok(())
    }
    #[test]
    fn ra9_static_speed_rejects_demanded_actual_source_overlap() -> AuditResult<()> {
        let (input, d, mut a, row) = native_adapter_control()?;
        let id = a.physical(WitnessId::new(
            "weapon.m60@rules:M60",
            "combat.weapons.projectile_speed",
        ));
        a.required_dependencies.insert((id, row.atom.clone()));
        assert!(require(&a, &input, &d, &row, Some(&final_approval(&row)?)).is_err());
        Ok(())
    }
}
