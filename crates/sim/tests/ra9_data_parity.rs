//! The data parity audit (docs/RA2_PARITY_PLAN.md section 3.1 and milestone RA9; docs/DECISIONS.md
//! R9): every shipped value that is ported from Red Alert 2 equals the conversion of its
//! reference value, or is listed in `tests/reference/ra2_differences.tsv` with a reason.
//!
//! The audit reads the committed tables of `tests/reference/` (`ra2_reference.tsv`: file,
//! section, key, value; `ra2_mapping.tsv`: game id to ini section) and the shipped data files as
//! the game loads them, and recomputes every conversion of the plan's sections 2 and 4.1 itself,
//! in integers, independently of `scripts/generate_ra2_data.py`, which writes the data. It fails
//!
//! * on a shipped value that differs from its converted reference value and has no row in
//!   `ra2_differences.tsv` (game id, field, game value, reference value, reason);
//! * on a row of `ra2_differences.tsv` that no longer applies (stale);
//! * on a mapped section or a key the reference lacks, and on a shipped item that is not mapped.
//!
//! RA1 audits, for every mapped unit and building: cost, strength, sight, speed, prerequisites,
//! power, sell value, owner sides, required and forbidden countries, and tech level; and every
//! value of `rules.ron` and the ported values of `skirmish.ron`. RA2 adds the build time of
//! every item (the formula of the plan's section 4.2 from its cost, times the wall coefficient
//! for a wall), and for every building its production lane (`BuildCat`), build limit, `Adjacent`
//! distance and the flags `BaseNormal`, `Radar`, `Powered`, `UnitRepair`, `Repairable`, whether
//! it can be sold (`Wall`, `Unsellable`) and the `IsPowered` of its superweapon. Later
//! milestones extend it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use sim::{
    Category, Definitions, MatchOptions, ProductionItem, Rules, Side, SkirmishDefinitions,
    load_match_definitions, load_skirmish,
};

// --- The tables ---------------------------------------------------------------------------------

fn reference_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/reference")
}

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data")
}

/// The rows of a tab separated table after its header, which must be `header`.
fn table(name: &str, header: &[&str]) -> Vec<Vec<String>> {
    let path = reference_dir().join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut lines = text.lines();
    assert_eq!(
        lines
            .next()
            .map(|line| line.split('\t').collect::<Vec<_>>()),
        Some(header.to_vec()),
        "{name}: header"
    );
    lines
        .map(|line| {
            let row: Vec<String> = line.split('\t').map(str::to_owned).collect();
            assert_eq!(row.len(), header.len(), "{name}: {line:?}");
            row
        })
        .collect()
}

/// `rules.ini` of the reference: section, then key, then value.
struct Reference(BTreeMap<String, BTreeMap<String, String>>);

impl Reference {
    fn load() -> Self {
        let mut sections: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        for row in table("ra2_reference.tsv", &["file", "section", "key", "value"]) {
            if row[0] == "rules" {
                sections
                    .entry(row[1].clone())
                    .or_default()
                    .insert(row[2].clone(), row[3].clone());
            }
        }
        Self(sections)
    }

    fn section(&self, section: &str) -> &BTreeMap<String, String> {
        self.0
            .get(section)
            .unwrap_or_else(|| panic!("the reference has no section [{section}]"))
    }

    fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.section(section).get(key).map(String::as_str)
    }

    fn value(&self, section: &str, key: &str) -> &str {
        self.get(section, key)
            .unwrap_or_else(|| panic!("the reference has no [{section}] {key}"))
    }
}

/// A row of the mapping: a game id and one ini section it stands for.
struct Mapped {
    game_id: String,
    building: bool,
    section: String,
    note: String,
}

fn mapping() -> Vec<Mapped> {
    table(
        "ra2_mapping.tsv",
        &["game_id", "kind", "section", "side", "note"],
    )
    .into_iter()
    .map(|row| {
        assert!(row[1] == "unit" || row[1] == "building", "{row:?}");
        Mapped {
            game_id: row[0].clone(),
            building: row[1] == "building",
            section: row[2].clone(),
            note: row[4].clone(),
        }
    })
    .collect()
}

/// The notes of mapped sections whose items enter the game in a later milestone.
const LATER: [&str; 3] = ["spawned", "tech_building", "support"];

/// The intentional differences, and which of them the audit met.
struct Differences {
    rows: Vec<(String, String, String, String)>,
    used: BTreeSet<usize>,
    problems: Vec<String>,
}

impl Differences {
    fn load() -> Self {
        let rows = table(
            "ra2_differences.tsv",
            &[
                "game_id",
                "field",
                "game_value",
                "reference_value",
                "reason",
            ],
        )
        .into_iter()
        .map(|row| {
            assert!(
                !row[4].trim().is_empty(),
                "a difference needs a reason: {row:?}"
            );
            (
                row[0].clone(),
                row[1].clone(),
                row[2].clone(),
                row[3].clone(),
            )
        })
        .collect();
        Self {
            rows,
            used: BTreeSet::new(),
            problems: Vec::new(),
        }
    }

    /// Compares the shipped value `game` of `field` of `game_id` with `expected`, the conversion
    /// of the reference value `reference` (as the ini writes it).
    fn check(&mut self, game_id: &str, field: &str, game: &str, expected: &str, reference: &str) {
        let listed = self.rows.iter().position(|(id, name, value, from)| {
            id == game_id && name == field && value == game && from == reference
        });
        match (game == expected, listed) {
            (true, None) => {}
            (true, Some(index)) => self.problems.push(format!(
                "stale difference (the values agree): {:?}",
                self.rows[index]
            )),
            (false, Some(index)) => {
                self.used.insert(index);
            }
            (false, None) => self.problems.push(format!(
                "{game_id}: {field} is {game:?}, the reference {reference:?} converts to \
                 {expected:?}, and ra2_differences.tsv does not list it"
            )),
        }
    }

    /// Fails the test with every problem found, including rows that were never met.
    fn finish(mut self) {
        for (index, row) in self.rows.iter().enumerate() {
            if !self.used.contains(&index) {
                self.problems
                    .push(format!("stale difference (never met): {row:?}"));
            }
        }
        assert!(
            self.problems.is_empty(),
            "{} problems:\n{}",
            self.problems.len(),
            self.problems.join("\n")
        );
    }
}

// --- The conversions (docs/RA2_PARITY_PLAN.md sections 2 and 4.1), in integers --------------------

/// A decimal of the ini (`.7`, `1.5`, `50%` without its sign, `-3`) as numerator and
/// denominator.
fn decimal(text: &str) -> (i128, i128) {
    let text = text.trim_end_matches('%');
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    assert!(
        (whole.is_empty() || whole.bytes().all(|b| b.is_ascii_digit()))
            && fraction.bytes().all(|b| b.is_ascii_digit())
            && !(whole.is_empty() && fraction.is_empty()),
        "not a decimal: {text:?}"
    );
    let denominator = 10_i128.pow(fraction.len() as u32);
    let numerator =
        whole.parse::<i128>().unwrap_or(0) * denominator + fraction.parse::<i128>().unwrap_or(0);
    (if negative { -numerator } else { numerator }, denominator)
}

/// `value x factor / divisor`, rounded half away from zero (for the positive values used:
/// `floor(x + 1/2)`).
fn scaled(text: &str, factor: i128, divisor: i128) -> i128 {
    let (numerator, denominator) = decimal(text);
    let (a, b) = (numerator * factor, denominator * divisor);
    (2 * a + b).div_euclid(2 * b)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Conversion {
    /// The number itself.
    Int,
    /// `50%`, or a ratio such as `.25`, as percent.
    Percent,
    /// A ratio as permille.
    Permille,
    /// A percentage such as `150%` as permille.
    PercentAsPermille,
    /// Game minutes of 900 frames as ticks.
    Minutes,
    /// Frames (15 per second) as ticks (20 per second), at least 1.
    Frames,
    /// Cells as subcells.
    Cells,
    /// `yes`, `true` as 1; `no`, `false` as 0.
    Flag,
}

fn convert(conversion: Conversion, text: &str) -> i128 {
    match conversion {
        Conversion::Int => scaled(text, 1, 1),
        Conversion::Percent if text.ends_with('%') => scaled(text, 1, 1),
        Conversion::Percent => scaled(text, 100, 1),
        Conversion::Permille => scaled(text, 1000, 1),
        Conversion::PercentAsPermille => scaled(text, 10, 1),
        Conversion::Minutes => scaled(text, 1200, 1),
        Conversion::Frames => scaled(text, 4, 3).max(1),
        Conversion::Cells => scaled(text, 256, 1),
        Conversion::Flag => match text.to_ascii_lowercase().as_str() {
            "yes" | "true" => 1,
            "no" | "false" => 0,
            other => panic!("not a flag: {other:?}"),
        },
    }
}

/// The class id prefix of the jumpjet locomotor.
const JUMPJET: &str = "{92612C46-";

/// Subcells per tick of a unit section (section 4.1), and the reference value it comes from.
fn speed(section: &BTreeMap<String, String>) -> (i128, String) {
    let key = |name: &str| {
        section
            .get(name)
            .unwrap_or_else(|| panic!("a unit section without {name}"))
    };
    let (leptons, source) = if key("Locomotor").starts_with(JUMPJET) {
        (
            scaled(key("JumpjetSpeed"), 1, 1),
            key("JumpjetSpeed").clone(),
        )
    } else {
        let mut leptons = (scaled(key("Speed"), 1, 1) * 256).div_euclid(100).min(255);
        if section.get("Category").map(String::as_str) == Some("Soldier") {
            leptons = (leptons * 3).div_euclid(2);
        }
        (leptons, key("Speed").clone())
    };
    // round(leptons x 3 / 4), at least 1.
    (((2 * leptons * 3 + 4).div_euclid(8)).max(1), source)
}

// --- The shipped data -----------------------------------------------------------------------------

fn shipped() -> Definitions {
    let dir = data_dir();
    load_match_definitions(
        dir.join("units.ron"),
        dir.join("economy.ron"),
        dir.join("combat.ron"),
    )
    .and_then(|definitions| definitions.with_rules(dir.join("rules.ron")))
    .and_then(|definitions| definitions.with_arsenal(dir.join("arsenal.ron")))
    .unwrap_or_else(|error| panic!("{error}"))
}

fn shipped_skirmish(definitions: &Definitions) -> SkirmishDefinitions {
    load_skirmish(data_dir().join("skirmish.ron"), definitions)
        .unwrap_or_else(|error| panic!("{error}"))
}

/// The houses of the ini in the order of `[Countries]`, with the game's country ids.
const HOUSES: [(&str, &str); 9] = [
    ("Americans", "america"),
    ("Alliance", "korea"),
    ("French", "france"),
    ("Germans", "germany"),
    ("British", "great_britain"),
    ("Africans", "libya"),
    ("Arabs", "iraq"),
    ("Confederation", "cuba"),
    ("Russians", "russia"),
];

/// The ini's side names with the game's sides.
const SIDES: [(&str, Side); 2] = [("GDI", Side::Allied), ("Nod", Side::Soviet)];

/// The prerequisite group tokens of the ini (upper case) with their `[General]` key and the
/// game's group id.
const GROUPS: [(&str, &str, &str); 6] = [
    ("POWER", "PrerequisitePower", "power"),
    ("BARRACKS", "PrerequisiteBarracks", "barracks"),
    ("FACTORY", "PrerequisiteFactory", "factory"),
    ("RADAR", "PrerequisiteRadar", "radar"),
    ("TECH", "PrerequisiteTech", "tech"),
    ("PROC", "PrerequisiteProc", "proc"),
];

fn country_of(house: &str) -> &'static str {
    HOUSES
        .iter()
        .find(|(name, _)| *name == house)
        .unwrap_or_else(|| panic!("house {house}"))
        .1
}

/// The side of a house by `[Sides]`.
fn side_of(reference: &Reference, house: &str) -> Option<Side> {
    SIDES.iter().find_map(|(ini, side)| {
        reference
            .value("Sides", ini)
            .split(',')
            .any(|member| member == house)
            .then_some(*side)
    })
}

fn list(value: Option<&str>) -> Vec<&str> {
    value
        .unwrap_or("")
        .split(',')
        .filter(|entry| !entry.is_empty())
        .collect()
}

fn joined(mut entries: Vec<String>) -> String {
    entries.sort();
    entries.dedup();
    entries.join(",")
}

// --- The audit of units and buildings -------------------------------------------------------------

#[test]
fn every_shipped_unit_and_building_equals_its_reference_or_is_a_listed_difference() {
    let reference = Reference::load();
    let mapping = mapping();
    let definitions = shipped();
    let rules = definitions.rules().expect("shipped rules");
    let mut differences = Differences::load();

    // The game id of every mapped section that is in the game by now.
    let game_id_of: BTreeMap<&str, &str> = mapping
        .iter()
        .filter(|row| !LATER.contains(&row.note.as_str()))
        .map(|row| (row.section.as_str(), row.game_id.as_str()))
        .collect();
    let refund = convert(
        Conversion::Percent,
        reference.value("General", "RefundPercent"),
    );
    // RA2: ticks of build time per 1000 credits, and the factor of a wall in permille.
    let build_speed = convert(
        Conversion::Minutes,
        reference.value("General", "BuildSpeed"),
    );
    let wall_factor = convert(
        Conversion::Permille,
        reference.value("General", "WallBuildSpeedCoefficient"),
    );

    for row in &mapping {
        let section = reference.section(&row.section);
        let item = if row.building {
            definitions
                .building_kind(&row.game_id)
                .map(ProductionItem::Building)
        } else {
            definitions
                .unit_kind(&row.game_id)
                .map(ProductionItem::Unit)
        };
        // An item of a later milestone is not in the data yet, and says so in the differences.
        let present = if item.is_some() { "yes" } else { "no" };
        if LATER.contains(&row.note.as_str()) {
            differences.check(&row.game_id, "present", present, "yes", "yes");
            continue;
        }
        let Some(item) = item else {
            differences.check(&row.game_id, "present", present, "yes", "yes");
            continue;
        };
        let id = row.game_id.as_str();
        let value = |key: &str| section.get(key).map(String::as_str);
        let required = |key: &str| {
            value(key).unwrap_or_else(|| panic!("the reference has no [{}] {key}", row.section))
        };
        let terms = definitions
            .production(item)
            .unwrap_or_else(|| panic!("{id} has no production terms"));
        let entry = definitions
            .metadata(item)
            .unwrap_or_else(|| panic!("{id} has no arsenal entry"));
        let mut number = |field: &str, game: i128, conversion: Conversion, key: &str| {
            let text = required(key);
            differences.check(
                id,
                field,
                &game.to_string(),
                &convert(conversion, text).to_string(),
                text,
            );
        };

        number("cost", i128::from(terms.cost), Conversion::Int, "Cost");
        let combat = match item {
            ProductionItem::Building(kind) => definitions.building_combat(kind),
            ProductionItem::Unit(kind) => definitions.unit_combat(kind),
        }
        .unwrap_or_else(|| panic!("{id} has no combat terms"));
        number(
            "strength",
            i128::from(combat.hit_points),
            Conversion::Int,
            "Strength",
        );
        number("sight", i128::from(combat.sight), Conversion::Int, "Sight");
        number(
            "tech_level",
            i128::from(entry.tech_level),
            Conversion::Int,
            "TechLevel",
        );

        match item {
            ProductionItem::Unit(kind) => {
                let (expected, source) = speed(section);
                let game = definitions.unit(kind).expect("unit").speed;
                differences.check(
                    id,
                    "speed",
                    &game.to_string(),
                    &expected.to_string(),
                    &source,
                );
            }
            ProductionItem::Building(kind) => {
                let def = definitions.building(kind).expect("building");
                // `Power` above 0 is produced, below 0 consumed; a building without the key has
                // neither.
                let power = value("Power").unwrap_or("0");
                differences.check(
                    id,
                    "power",
                    &(i128::from(def.power_produced) - i128::from(def.power_consumed)).to_string(),
                    &convert(Conversion::Int, power).to_string(),
                    power,
                );
                let cost = required("Cost");
                differences.check(
                    id,
                    "sell_value",
                    &def.sell_value.to_string(),
                    &(convert(Conversion::Int, cost) * refund)
                        .div_euclid(100)
                        .to_string(),
                    cost,
                );
            }
        }

        // RA2. The build time: the cost times `BuildSpeed` game minutes per 1000 credits, in
        // ticks, at least 1, and for a wall times `WallBuildSpeedCoefficient` (plan section 4.2).
        let flag = |key: &str, default: &'static str| {
            let text = value(key).unwrap_or(default);
            (convert(Conversion::Flag, text), text)
        };
        let (wall, _) = flag("Wall", "no");
        let cost = required("Cost");
        let mut expected =
            ((2 * convert(Conversion::Int, cost) * build_speed + 1000).div_euclid(2000)).max(1);
        if row.building && wall == 1 {
            expected = ((2 * expected * wall_factor + 1000).div_euclid(2000)).max(1);
        }
        differences.check(
            id,
            "build_time",
            &terms.build_time.to_string(),
            &expected.to_string(),
            cost,
        );
        // The build limit of every item; the other traits of schema version 3 are a building's,
        // and 0 on every unit.
        let limit = value("BuildLimit").unwrap_or("0");
        differences.check(
            id,
            "build_limit",
            &entry.build_limit.to_string(),
            &convert(Conversion::Int, limit).to_string(),
            limit,
        );
        let traits = [
            ("base_normal", entry.base_normal),
            ("radar", entry.radar),
            ("sellable", entry.sellable),
            ("repairable", entry.repairable),
            ("powered", entry.powered),
            ("is_powered", entry.is_powered),
            ("unit_repair", entry.unit_repair),
        ];
        if row.building {
            // The lane: the defences are `BuildCat=Combat`, and the arsenal's category `armory`.
            let build_cat = value("BuildCat").unwrap_or("");
            differences.check(
                id,
                "lane",
                if entry.category == Category::Armory {
                    "defences"
                } else {
                    "buildings"
                },
                if build_cat == "Combat" {
                    "defences"
                } else {
                    "buildings"
                },
                build_cat,
            );
            let adjacent = required("Adjacent");
            differences.check(
                id,
                "adjacent",
                &entry.adjacent.to_string(),
                &convert(Conversion::Int, adjacent).to_string(),
                adjacent,
            );
            let (unsellable, unsellable_text) = flag("Unsellable", "no");
            // The `IsPowered` of the superweapon the building gives, if it gives one.
            let is_powered = value("SuperWeapon").map_or((0, ""), |special| {
                let text = reference.value(special, "IsPowered");
                (convert(Conversion::Flag, text), text)
            });
            let expected = [
                flag("BaseNormal", "yes"),
                flag("Radar", "no"),
                (
                    i128::from(wall == 0 && unsellable == 0),
                    if wall == 1 { "Wall" } else { unsellable_text },
                ),
                flag("Repairable", "yes"),
                flag("Powered", "no"),
                is_powered,
                flag("UnitRepair", "no"),
            ];
            for ((field, game), (expected, text)) in traits.into_iter().zip(expected) {
                differences.check(
                    id,
                    field,
                    &u8::from(game).to_string(),
                    &expected.to_string(),
                    text,
                );
            }
        } else {
            assert_eq!(entry.adjacent, 0, "{id}: a unit has no adjacent distance");
            for (field, game) in traits {
                assert!(!game, "{id}: a unit has no {field}");
            }
        }

        // Prerequisites: the buildings of the economy file and the groups of the arsenal against
        // the tokens of `Prerequisite`, a group token being one of `[General] Prerequisite...`.
        let game: Vec<String> = terms
            .prerequisites
            .iter()
            .map(|&kind| definitions.building(kind).expect("prerequisite").id.clone())
            .chain(
                entry
                    .prerequisite_groups
                    .iter()
                    .map(|&group| format!("group:{}", rules.group(group).expect("group").id)),
            )
            .collect();
        let tokens = value("Prerequisite").unwrap_or("");
        let expected: Vec<String> = list(Some(tokens))
            .into_iter()
            .map(|token| {
                match GROUPS
                    .iter()
                    .find(|(group, _, _)| token.eq_ignore_ascii_case(group))
                {
                    Some((_, _, group)) => format!("group:{group}"),
                    None => game_id_of
                        .get(token)
                        .unwrap_or_else(|| panic!("[{}] needs unmapped {token}", row.section))
                        .to_string(),
                }
            })
            .collect();
        differences.check(
            id,
            "prerequisites",
            &joined(game),
            &joined(expected),
            tokens,
        );

        // Owner sides: the sides of the houses of `Owner`.
        let owner = value("Owner").unwrap_or("");
        let expected: Vec<String> = list(Some(owner))
            .into_iter()
            .filter_map(|house| side_of(&reference, house))
            .map(|side| side.id().to_owned())
            .collect();
        let game = entry
            .owner_sides
            .iter()
            .map(|side| side.id().to_owned())
            .collect();
        differences.check(id, "owner_sides", &joined(game), &joined(expected), owner);

        for (field, key, countries) in [
            (
                "required_countries",
                "RequiredHouses",
                &entry.required_countries,
            ),
            (
                "forbidden_countries",
                "ForbiddenHouses",
                &entry.forbidden_countries,
            ),
        ] {
            let houses = value(key).unwrap_or("");
            let expected = list(Some(houses))
                .into_iter()
                .map(|house| country_of(house).to_owned())
                .collect();
            let game = countries
                .iter()
                .map(|&country| rules.country(country).expect("country").id.clone())
                .collect();
            differences.check(id, field, &joined(game), &joined(expected), houses);
        }
    }

    // Every shipped unit and building is mapped, so nothing escapes the audit.
    let mapped: BTreeSet<&str> = mapping.iter().map(|row| row.game_id.as_str()).collect();
    for id in definitions
        .units()
        .iter()
        .map(|unit| unit.id.as_str())
        .chain(
            definitions
                .buildings()
                .iter()
                .map(|building| building.id.as_str()),
        )
    {
        assert!(
            mapped.contains(id),
            "{id} is shipped but not mapped to an ini section"
        );
    }
    differences.finish();
}

// --- The audit of rules.ron -------------------------------------------------------------------------

/// The scalar values of `rules.ron`: group, field, ini section, ini key, conversion.
const RULES: &[(&str, &str, &str, &str, Conversion)] = {
    use Conversion::*;
    &[
        (
            "production",
            "build_ticks_per_1000",
            "General",
            "BuildSpeed",
            Minutes,
        ),
        (
            "production",
            "multiple_factory_pm",
            "General",
            "MultipleFactory",
            Permille,
        ),
        (
            "production",
            "wall_build_time_pm",
            "General",
            "WallBuildSpeedCoefficient",
            Permille,
        ),
        (
            "production",
            "low_power_penalty_pm",
            "General",
            "LowPowerPenaltyModifier",
            Permille,
        ),
        (
            "production",
            "min_low_power_speed_pm",
            "General",
            "MinLowPowerProductionSpeed",
            Permille,
        ),
        (
            "production",
            "max_low_power_speed_pm",
            "General",
            "MaxLowPowerProductionSpeed",
            Permille,
        ),
        (
            "production",
            "max_queued_objects",
            "General",
            "MaximumQueuedObjects",
            Int,
        ),
        (
            "economy",
            "refund_percent",
            "General",
            "RefundPercent",
            Percent,
        ),
        (
            "economy",
            "repair_percent",
            "General",
            "RepairPercent",
            Percent,
        ),
        ("economy", "repair_step", "General", "RepairStep", Int),
        (
            "economy",
            "repair_interval",
            "General",
            "RepairRate",
            Minutes,
        ),
        (
            "economy",
            "unit_repair_interval",
            "General",
            "URepairRate",
            Minutes,
        ),
        (
            "economy",
            "infantry_repair_step",
            "General",
            "IRepairStep",
            Int,
        ),
        (
            "economy",
            "infantry_repair_interval",
            "General",
            "IRepairRate",
            Minutes,
        ),
        (
            "economy",
            "reload_interval",
            "General",
            "ReloadRate",
            Minutes,
        ),
        (
            "economy",
            "purifier_bonus_percent",
            "General",
            "PurifierBonus",
            Percent,
        ),
        ("economy", "ore_value", "Riparius", "Value", Int),
        ("economy", "gem_value", "Cruentus", "Value", Int),
        (
            "economy",
            "ore_growth_interval",
            "Riparius",
            "Growth",
            Frames,
        ),
        (
            "economy",
            "ore_growth_pm",
            "Riparius",
            "GrowthPercentage",
            Permille,
        ),
        (
            "economy",
            "ore_spread_interval",
            "Riparius",
            "Spread",
            Frames,
        ),
        (
            "economy",
            "ore_spread_pm",
            "Riparius",
            "SpreadPercentage",
            Permille,
        ),
        (
            "economy",
            "harvesters_per_refinery",
            "General",
            "HarvestersPerRefinery",
            Int,
        ),
        (
            "veterancy",
            "veteran_ratio_pm",
            "General",
            "VeteranRatio",
            Permille,
        ),
        ("veterancy", "veteran_cap", "General", "VeteranCap", Int),
        (
            "veterancy",
            "combat_pm",
            "General",
            "VeteranCombat",
            Permille,
        ),
        ("veterancy", "speed_pm", "General", "VeteranSpeed", Permille),
        ("veterancy", "sight_pm", "General", "VeteranSight", Permille),
        ("veterancy", "armor_pm", "General", "VeteranArmor", Permille),
        ("veterancy", "rof_pm", "General", "VeteranROF", Permille),
        (
            "veterancy",
            "initial_veteran",
            "General",
            "InitialVeteran",
            Flag,
        ),
        (
            "crew",
            "crew_escape_percent",
            "General",
            "CrewEscape",
            Percent,
        ),
        (
            "crew",
            "survivor_rate_pm",
            "General",
            "SurvivorRate",
            Permille,
        ),
        ("combat", "max_damage", "CombatDamage", "MaxDamage", Int),
        ("combat", "min_damage", "CombatDamage", "MinDamage", Int),
        (
            "combat",
            "condition_yellow_percent",
            "AudioVisual",
            "ConditionYellow",
            Percent,
        ),
        (
            "combat",
            "condition_red_percent",
            "AudioVisual",
            "ConditionRed",
            Percent,
        ),
        (
            "combat",
            "ballistic_scatter",
            "CombatDamage",
            "BallisticScatter",
            Cells,
        ),
        (
            "combat",
            "homing_scatter",
            "CombatDamage",
            "HomingScatter",
            Cells,
        ),
        ("combat", "c4_delay", "CombatDamage", "C4Delay", Minutes),
        ("combat", "ivan_damage", "CombatDamage", "IvanDamage", Int),
        (
            "combat",
            "ivan_delay",
            "CombatDamage",
            "IvanTimedDelay",
            Frames,
        ),
        (
            "combat",
            "v3_rocket_damage",
            "General",
            "V3RocketDamage",
            Int,
        ),
        (
            "combat",
            "v3_rocket_elite_damage",
            "General",
            "V3RocketEliteDamage",
            Int,
        ),
        ("combat", "dmisl_damage", "General", "DMislDamage", Int),
        (
            "combat",
            "dmisl_elite_damage",
            "General",
            "DMislEliteDamage",
            Int,
        ),
        (
            "combat",
            "bridge_strength",
            "CombatDamage",
            "BridgeStrength",
            Int,
        ),
        ("combat", "cloak_delay", "General", "CloakDelay", Minutes),
        (
            "combat",
            "chrono_distance_factor",
            "General",
            "ChronoDistanceFactor",
            Int,
        ),
        (
            "combat",
            "chrono_minimum_delay",
            "General",
            "ChronoMinimumDelay",
            Frames,
        ),
        (
            "combat",
            "spy_power_blackout",
            "General",
            "SpyPowerBlackout",
            Frames,
        ),
        (
            "combat",
            "spy_money_steal_percent",
            "General",
            "SpyMoneyStealPercent",
            Percent,
        ),
        ("combat", "tree_strength", "General", "TreeStrength", Int),
        (
            "radiation",
            "application_delay",
            "Radiation",
            "RadApplicationDelay",
            Frames,
        ),
        (
            "radiation",
            "level_delay",
            "Radiation",
            "RadLevelDelay",
            Frames,
        ),
        (
            "radiation",
            "level_factor_pm",
            "Radiation",
            "RadLevelFactor",
            Permille,
        ),
        ("radiation", "level_max", "Radiation", "RadLevelMax", Int),
        (
            "radiation",
            "duration_multiple_pm",
            "Radiation",
            "RadDurationMultiple",
            Permille,
        ),
        (
            "superweapons",
            "nuclear_recharge",
            "MultiSpecial",
            "RechargeTime",
            Minutes,
        ),
        (
            "superweapons",
            "lightning_recharge",
            "LightningStormSpecial",
            "RechargeTime",
            Minutes,
        ),
        (
            "superweapons",
            "iron_curtain_recharge",
            "IronCurtainSpecial",
            "RechargeTime",
            Minutes,
        ),
        (
            "superweapons",
            "chronosphere_recharge",
            "ChronoSphereSpecial",
            "RechargeTime",
            Minutes,
        ),
        (
            "superweapons",
            "paradrop_recharge",
            "ParaDropSpecial",
            "RechargeTime",
            Minutes,
        ),
        (
            "superweapons",
            "american_paradrop_recharge",
            "AmericanParaDropSpecial",
            "RechargeTime",
            Minutes,
        ),
        (
            "superweapons",
            "iron_curtain_duration",
            "CombatDamage",
            "IronCurtainDuration",
            Frames,
        ),
        (
            "superweapons",
            "lightning_deferment",
            "General",
            "LightningDeferment",
            Frames,
        ),
        (
            "superweapons",
            "lightning_duration",
            "General",
            "LightningStormDuration",
            Frames,
        ),
        (
            "superweapons",
            "lightning_damage",
            "General",
            "LightningDamage",
            Int,
        ),
        (
            "superweapons",
            "lightning_hit_delay",
            "General",
            "LightningHitDelay",
            Frames,
        ),
        (
            "superweapons",
            "lightning_scatter_delay",
            "General",
            "LightningScatterDelay",
            Frames,
        ),
        (
            "superweapons",
            "lightning_cell_spread",
            "General",
            "LightningCellSpread",
            Int,
        ),
        (
            "superweapons",
            "lightning_separation",
            "General",
            "LightningSeparation",
            Int,
        ),
        (
            "superweapons",
            "american_paradrop_count",
            "General",
            "AmerParaDropNum",
            Int,
        ),
        (
            "superweapons",
            "paradrop_radius",
            "General",
            "ParadropRadius",
            Int,
        ),
        (
            "prism",
            "support_delay",
            "General",
            "PrismSupportDelay",
            Frames,
        ),
        (
            "prism",
            "support_duration",
            "General",
            "PrismSupportDuration",
            Frames,
        ),
        ("prism", "support_max", "General", "PrismSupportMax", Int),
        (
            "prism",
            "support_modifier_pm",
            "General",
            "PrismSupportModifier",
            PercentAsPermille,
        ),
        (
            "prism",
            "support_height",
            "General",
            "PrismSupportHeight",
            Int,
        ),
        ("crates", "maximum", "CrateRules", "CrateMaximum", Int),
        ("crates", "minimum", "CrateRules", "CrateMinimum", Int),
        ("crates", "radius", "CrateRules", "CrateRadius", Cells),
        ("crates", "regen", "CrateRules", "CrateRegen", Minutes),
        ("crates", "free_mcv", "CrateRules", "FreeMCV", Flag),
        (
            "dialog",
            "min_credits",
            "MultiplayerDialogSettings",
            "MinMoney",
            Int,
        ),
        (
            "dialog",
            "credits",
            "MultiplayerDialogSettings",
            "Money",
            Int,
        ),
        (
            "dialog",
            "max_credits",
            "MultiplayerDialogSettings",
            "MaxMoney",
            Int,
        ),
        (
            "dialog",
            "credits_step",
            "MultiplayerDialogSettings",
            "MoneyIncrement",
            Int,
        ),
        (
            "dialog",
            "min_unit_count",
            "MultiplayerDialogSettings",
            "MinUnitCount",
            Int,
        ),
        (
            "dialog",
            "unit_count",
            "MultiplayerDialogSettings",
            "UnitCount",
            Int,
        ),
        (
            "dialog",
            "max_unit_count",
            "MultiplayerDialogSettings",
            "MaxUnitCount",
            Int,
        ),
        (
            "dialog",
            "tech_level",
            "MultiplayerDialogSettings",
            "TechLevel",
            Int,
        ),
        (
            "dialog",
            "bases",
            "MultiplayerDialogSettings",
            "Bases",
            Flag,
        ),
        (
            "dialog",
            "short_game",
            "MultiplayerDialogSettings",
            "ShortGame",
            Flag,
        ),
        (
            "dialog",
            "crates",
            "MultiplayerDialogSettings",
            "Crates",
            Flag,
        ),
        (
            "dialog",
            "mcv_redeploys",
            "MultiplayerDialogSettings",
            "MCVRedeploys",
            Flag,
        ),
        (
            "dialog",
            "bridge_destruction",
            "MultiplayerDialogSettings",
            "BridgeDestruction",
            Flag,
        ),
        (
            "dialog",
            "fog_of_war",
            "MultiplayerDialogSettings",
            "FogOfWar",
            Flag,
        ),
        (
            "dialog",
            "shroud",
            "MultiplayerDialogSettings",
            "Shroud",
            Flag,
        ),
        ("dialog", "max_players", "Maximums", "Players", Int),
    ]
};

/// The scalar groups of the shipped rules by name, each with its fields in file order.
fn scalar_groups(rules: &Rules) -> Vec<(&'static str, Vec<(&'static str, i32)>)> {
    vec![
        ("production", rules.production.fields()),
        ("economy", rules.economy.fields()),
        ("veterancy", rules.veterancy.fields()),
        ("crew", rules.crew.fields()),
        ("combat", rules.combat.fields()),
        ("radiation", rules.radiation.fields()),
        ("superweapons", rules.superweapons.fields()),
        ("prism", rules.prism.fields()),
        ("crates", rules.crates.fields()),
        ("dialog", rules.dialog.fields()),
    ]
}

#[test]
fn every_value_of_the_shipped_rules_equals_its_reference_or_is_a_listed_difference() {
    let reference = Reference::load();
    let definitions = shipped();
    let rules = definitions.rules().expect("shipped rules");
    let mut differences = Differences::load();
    // The other test of this file meets the differences of units and buildings.
    let foreign: Vec<usize> = differences
        .rows
        .iter()
        .enumerate()
        .filter(|(_, (id, ..))| id != "rules")
        .map(|(index, _)| index)
        .collect();
    differences.used.extend(foreign);

    // Every scalar field has exactly one row of the table, so that none goes unaudited.
    for (group, fields) in scalar_groups(rules) {
        for (field, game) in fields {
            let rows: Vec<_> = RULES
                .iter()
                .filter(|(g, f, ..)| *g == group && *f == field)
                .collect();
            assert_eq!(
                rows.len(),
                1,
                "rules.ron {group}.{field} is not audited exactly once"
            );
            let (_, _, section, key, conversion) = rows[0];
            let text = reference.value(section, key);
            differences.check(
                "rules",
                &format!("{group}.{field}"),
                &game.to_string(),
                &convert(*conversion, text).to_string(),
                text,
            );
        }
    }
    assert_eq!(
        RULES.len(),
        scalar_groups(rules)
            .iter()
            .map(|(_, fields)| fields.len())
            .sum::<usize>(),
        "the audit table names a field the rules file does not have"
    );

    // Powerups: the crate contents with a share above 0, with their argument.
    let powerups = [
        ("money", "Money", Some(Conversion::Int)),
        ("unit", "Unit", None),
        ("veteran", "Veteran", Some(Conversion::Int)),
        ("heal_base", "HealBase", None),
        ("armour", "Armor", Some(Conversion::Permille)),
        ("firepower", "Firepower", Some(Conversion::Permille)),
        ("speed", "Speed", Some(Conversion::Permille)),
        ("reveal", "Reveal", None),
    ];
    let with_share: BTreeSet<&str> = reference
        .section("Powerups")
        .iter()
        .filter(|(_, value)| !value.starts_with("0,"))
        .map(|(key, _)| key.as_str())
        .collect();
    assert_eq!(
        with_share,
        powerups
            .iter()
            .map(|(_, key, _)| *key)
            .collect::<BTreeSet<_>>(),
        "the powerups with a share"
    );
    assert_eq!(rules.powerups.len(), powerups.len());
    for (id, key, argument) in powerups {
        let text = reference.value("Powerups", key);
        let parts: Vec<&str> = text.split(',').collect();
        let game = rules
            .powerups
            .iter()
            .find(|powerup| powerup.id == id)
            .unwrap_or_else(|| panic!("powerup {id}"));
        let value = argument.map_or(0, |conversion| convert(conversion, parts[3]));
        differences.check(
            "rules",
            &format!("powerups.{id}"),
            &format!("{},{}", game.share, game.value),
            &format!("{},{value}", convert(Conversion::Int, parts[0])),
            text,
        );
    }

    // Difficulties: the ini's sections are named from the human's point of view (plan section
    // 4.15), and the lists of [General] are ordered hard, medium, easy.
    let difficulties = [
        ("easy", "Difficult", 2),
        ("medium", "Normal", 1),
        ("brutal", "Easy", 0),
    ];
    assert_eq!(rules.difficulties.len(), difficulties.len());
    for (id, section, index) in difficulties {
        let game = rules
            .difficulties
            .iter()
            .find(|difficulty| difficulty.id == id)
            .unwrap_or_else(|| panic!("difficulty {id}"));
        let entry = |key: &str| {
            reference
                .value("General", key)
                .split(',')
                .nth(index)
                .expect(key)
        };
        for (field, game, text, conversion) in [
            (
                "rof_pm",
                game.rof_pm,
                reference.value(section, "ROF"),
                Conversion::Permille,
            ),
            (
                "build_time_pm",
                game.build_time_pm,
                reference.value(section, "BuildTime"),
                Conversion::Permille,
            ),
            (
                "credits_bonus_percent",
                game.credits_bonus_percent,
                entry("MultiplayerAICM"),
                Conversion::Int,
            ),
            (
                "virtual_purifiers",
                game.virtual_purifiers,
                entry("AIVirtualPurifiers"),
                Conversion::Int,
            ),
            (
                "team_delay",
                game.team_delay,
                entry("TeamDelays"),
                Conversion::Frames,
            ),
            (
                "allied_base_defences",
                game.allied_base_defences,
                entry("AlliedBaseDefenseCounts"),
                Conversion::Int,
            ),
            (
                "soviet_base_defences",
                game.soviet_base_defences,
                entry("SovietBaseDefenseCounts"),
                Conversion::Int,
            ),
        ] {
            differences.check(
                "rules",
                &format!("difficulty.{id}.{field}"),
                &game.to_string(),
                &convert(conversion, text).to_string(),
                text,
            );
        }
    }

    // Prerequisite groups, sides and countries name game ids, resolved through the mapping.
    let mapping = mapping();
    let game_id = |section: &str| {
        mapping
            .iter()
            .find(|row| row.section == section)
            .unwrap_or_else(|| panic!("[{section}] is not mapped"))
            .game_id
            .clone()
    };
    let building = |kind| definitions.building(kind).expect("building").id.clone();
    let unit = |kind| definitions.unit(kind).expect("unit").id.clone();
    assert_eq!(rules.prerequisite_groups.len(), GROUPS.len());
    for (_, key, id) in GROUPS {
        let text = reference.value("General", key);
        let group = rules
            .group_id(id)
            .and_then(|group| rules.group(group))
            .unwrap_or_else(|| panic!("group {id}"));
        differences.check(
            "rules",
            &format!("prerequisite_groups.{id}"),
            &joined(group.buildings.iter().map(|&kind| building(kind)).collect()),
            &joined(list(Some(text)).into_iter().map(game_id).collect()),
            text,
        );
    }
    let base_units = list(reference.get("General", "BaseUnit"));
    assert_eq!(rules.sides.len(), 2);
    for (index, (side, crew, divisor, disguise, infantry, count)) in [
        (
            Side::Allied,
            "AlliedCrew",
            "AlliedSurvivorDivisor",
            "AlliedDisguise",
            "AllyParaDropInf",
            "AllyParaDropNum",
        ),
        (
            Side::Soviet,
            "SovietCrew",
            "SovietSurvivorDivisor",
            "SovietDisguise",
            "SovParaDropInf",
            "SovParaDropNum",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let game = rules.side(side).unwrap_or_else(|| panic!("side {side:?}"));
        let name = side.id();
        let general = |key: &str| reference.value("General", key);
        let base = base_units[index];
        for (field, game, expected, text) in [
            ("base_unit", unit(game.base_unit), game_id(base), base),
            (
                "construction_yard",
                building(game.construction_yard),
                game_id(reference.value(base, "DeploysInto")),
                reference.value(base, "DeploysInto"),
            ),
            (
                "crew",
                unit(game.crew),
                game_id(general(crew)),
                general(crew),
            ),
            (
                "survivor_divisor",
                game.survivor_divisor.to_string(),
                convert(Conversion::Int, general(divisor)).to_string(),
                general(divisor),
            ),
            (
                "disguise",
                unit(game.disguise),
                game_id(general(disguise)),
                general(disguise),
            ),
            (
                "paradrop_infantry",
                unit(game.paradrop_infantry),
                game_id(general(infantry)),
                general(infantry),
            ),
            (
                "paradrop_count",
                game.paradrop_count.to_string(),
                convert(Conversion::Int, general(count)).to_string(),
                general(count),
            ),
        ] {
            differences.check(
                "rules",
                &format!("sides.{name}.{field}"),
                &game,
                &expected,
                text,
            );
        }
        // The yard undeploys into the base unit it comes from.
        assert_eq!(
            reference.value(reference.value(base, "DeploysInto"), "UndeploysInto"),
            base
        );
    }
    // The nine playable countries in the order of [Countries], each with the side of its house.
    assert_eq!(rules.countries.len(), HOUSES.len());
    for (index, (house, country)) in HOUSES.into_iter().enumerate() {
        assert_eq!(reference.value("Countries", &index.to_string()), house);
        let game = &rules.countries[index];
        let expected = SIDES
            .iter()
            .find(|(ini, _)| *ini == reference.value(house, "Side"))
            .unwrap_or_else(|| panic!("side of {house}"))
            .1;
        differences.check(
            "rules",
            &format!("countries.{index}"),
            &format!("{},{}", game.id, game.side.id()),
            &format!("{country},{}", expected.id()),
            house,
        );
        assert_eq!(
            side_of(&reference, house),
            Some(expected),
            "[Sides] and [{house}] agree"
        );
    }
    differences.finish();
}

// --- The audit of skirmish.ron and of the default options -----------------------------------------

#[test]
fn the_shipped_skirmish_file_and_the_default_options_are_those_of_the_lobby() {
    let reference = Reference::load();
    let mapping = mapping();
    let definitions = shipped();
    let rules = definitions.rules().expect("shipped rules");
    let skirmish = shipped_skirmish(&definitions);
    let dialog = |key: &str| reference.value("MultiplayerDialogSettings", key);

    assert_eq!(
        i128::from(skirmish.starting_credits),
        convert(Conversion::Int, dialog("Money"))
    );
    // The factions are the nine countries of the rules, in their order.
    assert_eq!(
        skirmish
            .factions
            .iter()
            .map(|faction| (
                faction.id.as_str(),
                faction.side,
                faction.country.map(|c| c.0)
            ))
            .collect::<Vec<_>>(),
        rules
            .countries
            .iter()
            .enumerate()
            .map(|(index, country)| (country.id.as_str(), Some(country.side), Some(index as u8)))
            .collect::<Vec<_>>()
    );
    // The starting unit types (plan section 4.9): the mapped units in the game without
    // `AllowedToStartInMultiplayer=no`, of a tech level of the lobby, built by one side only and
    // by all of its countries, other than the base units; infantry by `Category=Soldier`.
    let top = convert(Conversion::Int, dialog("TechLevel"));
    let base_units = list(reference.get("General", "BaseUnit"));
    for (side, name) in [(Side::Allied, "GDI"), (Side::Soviet, "Nod")] {
        let houses = list(reference.get("Sides", name));
        let mut vehicles = BTreeSet::new();
        let mut infantry = BTreeSet::new();
        for row in &mapping {
            if row.building || LATER.contains(&row.note.as_str()) || row.note == "twin" {
                continue;
            }
            let section = reference.section(&row.section);
            let get = |key: &str| section.get(key).map(String::as_str);
            let owners = list(get("Owner"));
            let tech = convert(Conversion::Int, get("TechLevel").expect("TechLevel"));
            let eligible = get("AllowedToStartInMultiplayer") != Some("no")
                && (0..=top).contains(&tech)
                && !owners.is_empty()
                && owners.iter().all(|owner| houses.contains(owner))
                && get("RequiredHouses").is_none()
                && get("ForbiddenHouses").is_none()
                && !get("RequiresStolenAlliedTech").is_some()
                && !get("RequiresStolenSovietTech").is_some()
                && !base_units.contains(&row.section.as_str());
            if eligible {
                if get("Category") == Some("Soldier") {
                    infantry.insert(row.game_id.clone());
                } else {
                    vehicles.insert(row.game_id.clone());
                }
            }
        }
        let start = skirmish
            .side(side)
            .unwrap_or_else(|| panic!("side {side:?}"));
        let ids = |kinds: &[sim::UnitKind]| -> BTreeSet<String> {
            kinds
                .iter()
                .map(|&kind| definitions.unit(kind).expect("unit").id.clone())
                .collect()
        };
        assert_eq!(
            ids(&start.vehicles),
            vehicles,
            "starting vehicles of {side:?}"
        );
        assert_eq!(
            ids(&start.infantry),
            infantry,
            "starting infantry of {side:?}"
        );
    }
    // 4.9 with the vanilla values: the mean cost is 475, and a unit count of 10 gives 5 Grizzly
    // tanks and 4 GIs, or 4 Rhino tanks and 7 Conscripts.
    assert_eq!(skirmish.average_starting_cost(&definitions), 475);

    // The defaults of the match options are the lobby's.
    let options = MatchOptions::default();
    let flag = |key: &str| convert(Conversion::Flag, dialog(key)) == 1;
    assert_eq!(options.mcv_start, flag("Bases"));
    assert_eq!(options.short_game, flag("ShortGame"));
    assert_eq!(options.crates, flag("Crates"));
    assert_eq!(options.mcv_repacks, flag("MCVRedeploys"));
    assert!(options.superweapons && !options.free_for_all);
    assert_eq!((options.starting_credits, options.unit_count), (None, None));
    assert_eq!(
        i128::from(rules.dialog.unit_count),
        convert(Conversion::Int, dialog("UnitCount"))
    );
}

// --- The audit's own arithmetic ---------------------------------------------------------------------

#[test]
fn the_conversions_round_half_away_from_zero_in_integers() {
    // Section 2 of the plan, with the values the ini holds.
    assert_eq!(convert(Conversion::Minutes, ".7"), 840);
    assert_eq!(convert(Conversion::Minutes, ".016"), 19);
    assert_eq!(convert(Conversion::Minutes, ".001"), 1);
    assert_eq!(convert(Conversion::Minutes, "10"), 12_000);
    assert_eq!(convert(Conversion::Frames, "750"), 1000);
    assert_eq!(convert(Conversion::Frames, "250"), 333);
    assert_eq!(convert(Conversion::Frames, "10"), 13);
    assert_eq!(convert(Conversion::Frames, "5"), 7);
    assert_eq!(convert(Conversion::Frames, "0"), 1);
    assert_eq!(convert(Conversion::Frames, "2200"), 2933);
    assert_eq!(convert(Conversion::Cells, "3.0"), 768);
    assert_eq!(convert(Conversion::Cells, "1.5"), 384);
    assert_eq!(convert(Conversion::Permille, "0.8"), 800);
    assert_eq!(convert(Conversion::Permille, ".06"), 60);
    assert_eq!(convert(Conversion::Permille, "1"), 1000);
    assert_eq!(convert(Conversion::PercentAsPermille, "150%"), 1500);
    assert_eq!(convert(Conversion::Percent, "50%"), 50);
    assert_eq!(convert(Conversion::Percent, ".25"), 25);
    assert_eq!(convert(Conversion::Int, "-1"), -1);
    assert_eq!(
        (
            convert(Conversion::Flag, "yes"),
            convert(Conversion::Flag, "no")
        ),
        (1, 0)
    );
    // Section 4.1: the GI 11, the Grizzly 13, the Harrier 26 subcells per tick.
    let unit = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    };
    let foot = "{4A582744-9839-11d1-B709-00A024DDAFD1}";
    assert_eq!(
        speed(&unit(&[
            ("Speed", "4"),
            ("Category", "Soldier"),
            ("Locomotor", foot)
        ]))
        .0,
        11
    );
    assert_eq!(
        speed(&unit(&[
            ("Speed", "7"),
            ("Category", "AFV"),
            ("Locomotor", foot)
        ]))
        .0,
        13
    );
    assert_eq!(
        speed(&unit(&[
            ("Speed", "14"),
            ("Category", "AirPower"),
            ("Locomotor", foot)
        ]))
        .0,
        26
    );
    // A jumpjet moves by `JumpjetSpeed`; a speed above the engine's 255 leptons is capped.
    let jumpjet = "{92612C46-F71F-11d1-AC9F-006008055BB5}";
    assert_eq!(
        speed(&unit(&[
            ("Speed", "9"),
            ("JumpjetSpeed", "30"),
            ("Locomotor", jumpjet)
        ]))
        .0,
        23
    );
    assert_eq!(
        speed(&unit(&[("Speed", "200"), ("Locomotor", foot)])).0,
        191
    );
}
