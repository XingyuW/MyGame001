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
//! it can be sold (`Wall`, `Unsellable`) and the `IsPowered` of its superweapon. RA3 adds, for
//! every unit and building, its armour type, the weapons it carries (primary and secondary, and
//! those at elite rank), `AirRangeBonus`, `LandTargeting`, `NavalTargeting`, `Underwater`,
//! `Organic` and `Wall`; and, with the third table `ra2_combat.tsv` (game id of a weapon or
//! warhead to its ini section), every weapon (damage, range, minimum range, reload, burst,
//! warhead, projectile speed, the layers its projectile engages, scatter), every warhead
//! (`Verses`, `CellSpread`, `PercentAtMax`, `Wall`, `Wood`) and the area effects. Later
//! milestones extend it.
//!
//! Since the remediation of advisory review 1 (docs/DECISIONS.md R56) it also compares the
//! free unit and the `Refinery` flag of a building, the producer of every item, the factory
//! group, side, domain, category and capacity of the arsenal, power as produced and consumed
//! separately, and the refund with the section's `Soylent`; the game's own damage of the two
//! effects without a reference value needs a row; and every field of an item's entry in the
//! shipped files must be audited or be listed as the game's own (`ITEM_FIELDS`).

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
    /// The side the mapping gives the item (`allied`, `soviet`, `none`).
    side: String,
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
            side: row[3].clone(),
            note: row[4].clone(),
        }
    })
    .collect()
}

/// The game ids of the weapons and of the warheads by ini section (`ra2_combat.tsv`), and the
/// rows of each kind in the order of the table: (game id, section).
struct CombatMapping {
    weapon_id: BTreeMap<String, String>,
    warhead_id: BTreeMap<String, String>,
    weapons: Vec<(String, String)>,
    warheads: Vec<(String, String)>,
}

fn combat_mapping() -> CombatMapping {
    let mut mapping = CombatMapping {
        weapon_id: BTreeMap::new(),
        warhead_id: BTreeMap::new(),
        weapons: Vec::new(),
        warheads: Vec::new(),
    };
    for row in table("ra2_combat.tsv", &["game_id", "kind", "section"]) {
        let (ids, rows) = match row[1].as_str() {
            "weapon" => (&mut mapping.weapon_id, &mut mapping.weapons),
            "warhead" => (&mut mapping.warhead_id, &mut mapping.warheads),
            other => panic!("ra2_combat.tsv: unknown kind {other:?}"),
        };
        assert!(
            ids.insert(row[2].clone(), row[0].clone()).is_none(),
            "ra2_combat.tsv: [{}] is mapped twice",
            row[2]
        );
        rows.push((row[0].clone(), row[2].clone()));
    }
    mapping
}

/// The prefixes of the game ids under which `ra2_differences.tsv` lists weapons, warheads and
/// area effects (a weapon and a unit may share an id).
const COMBAT_PREFIXES: [&str; 3] = ["weapon.", "warhead.", "effect."];

/// The notes of mapped sections whose items enter the game in a later milestone.
const LATER: [&str; 1] = ["support"];

/// The intentional differences, and which of them the audit met.
struct Differences {
    rows: Vec<(String, String, String, String)>,
    used: BTreeSet<usize>,
    problems: Vec<String>,
    /// The fields the audit compared (of any game id), for the completeness guard.
    checked: BTreeSet<String>,
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
            checked: BTreeSet::new(),
        }
    }

    /// Compares the shipped value `game` of `field` of `game_id` with `expected`, the conversion
    /// of the reference value `reference` (as the ini writes it).
    fn check(&mut self, game_id: &str, field: &str, game: &str, expected: &str, reference: &str) {
        self.checked.insert(field.to_owned());
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
/// `infantry`: whether `[InfantryTypes]` lists the section (the one test of "infantry on foot"
/// of the pipeline: the generator, the arsenal's `infantry` and this audit all use the list).
fn speed(section: &BTreeMap<String, String>, infantry: bool) -> (i128, String) {
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
        if infantry {
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

/// Whether `[InfantryTypes]` of the reference lists `section`.
fn in_infantry_list(reference: &Reference, section: &str) -> bool {
    reference
        .section("InfantryTypes")
        .values()
        .any(|listed| listed == section)
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
    // The test of the weapons and warheads meets their differences.
    let foreign: Vec<usize> = differences
        .rows
        .iter()
        .enumerate()
        .filter(|(_, (id, ..))| COMBAT_PREFIXES.iter().any(|prefix| id.starts_with(prefix)))
        .map(|(index, _)| index)
        .collect();
    differences.used.extend(foreign);
    let combat_ids = combat_mapping();
    let armour_types = definitions.armour_classes();
    let weapon_name = |kind: Option<sim::WeaponKind>| {
        kind.and_then(|kind| definitions.weapon(kind))
            .map(|weapon| weapon.id.clone())
    };

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
            let text = value(key).unwrap_or_else(|| match key {
                "Cost" | "Sight" => "0",
                _ => required(key),
            });
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

        // RA3. The armour type, and the weapons of the two slots below and at elite rank: the
        // ini's `Primary` (or `Weapon1`) and `Secondary`, and `ElitePrimary` (or
        // `EliteWeapon1`) and `EliteSecondary`, each in the place of the weapon of its slot. A
        // weapon of the ini that the game does not carry reads `?Name` and needs a row.
        let armour = value("Armor").unwrap_or("none");
        differences.check(
            id,
            "armour",
            &armour_types[usize::from(combat.armour.0)],
            &armour.to_ascii_lowercase(),
            armour,
        );
        let slot = |keys: &[&str]| {
            keys.iter()
                .filter_map(|key| value(key))
                .find(|name| *name != "none")
        };
        let primary = slot(&["Primary", "Weapon1"]);
        let secondary = slot(&["Secondary"]);
        let elite = [
            slot(&["ElitePrimary", "EliteWeapon1"]).or(primary),
            slot(&["EliteSecondary"]).or(secondary),
        ];
        let listed = |slots: [Option<&str>; 2]| {
            let expected: Vec<String> = slots
                .iter()
                .flatten()
                .map(|name| {
                    combat_ids
                        .weapon_id
                        .get(*name)
                        .cloned()
                        .unwrap_or_else(|| format!("?{name}"))
                })
                .collect();
            let reference: Vec<&str> = slots.iter().flatten().copied().collect();
            (expected.join(","), reference.join(","))
        };
        let carried = |slots: [Option<sim::WeaponKind>; 2]| {
            slots
                .into_iter()
                .filter_map(weapon_name)
                .collect::<Vec<_>>()
                .join(",")
        };
        let (expected, source) = listed([primary, secondary]);
        differences.check(
            id,
            "weapons",
            &carried(combat.weapons(false)),
            &expected,
            &source,
        );
        let has_elite = ["ElitePrimary", "EliteWeapon1", "EliteSecondary"]
            .iter()
            .any(|key| value(key).is_some());
        if has_elite {
            let (expected, source) = listed(elite);
            differences.check(
                id,
                "elite_weapons",
                &carried(combat.weapons(true)),
                &expected,
                &source,
            );
        } else {
            assert_eq!(
                combat.weapons(true),
                combat.weapons(false),
                "{id}: without elite weapons in the ini a type keeps its weapons at elite rank"
            );
        }
        for (field, game, key, conversion) in [
            (
                "air_range_bonus",
                i128::from(combat.air_range_bonus),
                "AirRangeBonus",
                Conversion::Cells,
            ),
            (
                "land_targeting",
                i128::from(combat.land_targeting),
                "LandTargeting",
                Conversion::Int,
            ),
            (
                "naval_targeting",
                i128::from(combat.naval_targeting),
                "NavalTargeting",
                Conversion::Int,
            ),
        ] {
            let text = value(key).unwrap_or("0");
            differences.check(
                id,
                field,
                &game.to_string(),
                &convert(conversion, text).to_string(),
                text,
            );
        }
        let traits: &[(&str, bool, &str)] = if row.building {
            &[("wall", combat.wall, "Wall")]
        } else {
            &[
                ("underwater", combat.underwater, "Underwater"),
                ("organic", combat.organic, "Organic"),
            ]
        };
        for &(field, game, key) in traits {
            let text = value(key).unwrap_or("no");
            differences.check(
                id,
                field,
                &i128::from(game).to_string(),
                &convert(Conversion::Flag, text).to_string(),
                text,
            );
        }
        assert!(
            if row.building {
                !combat.underwater && !combat.organic
            } else {
                !combat.wall
            },
            "{id}: a trait of the other kind"
        );

        match item {
            ProductionItem::Unit(kind) => {
                let (expected, source) = speed(section, in_infantry_list(&reference, &row.section));
                let game = definitions.unit(kind).expect("unit").speed;
                differences.check(
                    id,
                    "speed",
                    &game.to_string(),
                    &expected.to_string(),
                    &source,
                );
                // RA5: `Harvester=yes` is a harvester of the economy file, whose capacity is
                // the section's `Storage` in bails.
                let harvester = definitions.harvester(kind);
                let text = value("Harvester").unwrap_or("no");
                differences.check(
                    id,
                    "harvester",
                    &u8::from(harvester.is_some()).to_string(),
                    &convert(Conversion::Flag, text).to_string(),
                    text,
                );
                if let Some(terms) = harvester {
                    let storage = required("Storage");
                    differences.check(
                        id,
                        "storage",
                        &terms.capacity.to_string(),
                        &convert(Conversion::Int, storage).to_string(),
                        storage,
                    );
                }
            }
            ProductionItem::Building(kind) => {
                let def = definitions.building(kind).expect("building");
                // `Power` above 0 is produced, below 0 consumed; a building without the key has
                // neither.
                let power = value("Power").unwrap_or("0");
                let net = convert(Conversion::Int, power);
                for (field, game, expected) in [
                    ("power_produced", def.power_produced, net.max(0)),
                    ("power_consumed", def.power_consumed, (-net).max(0)),
                ] {
                    differences.check(id, field, &game.to_string(), &expected.to_string(), power);
                }
                // The economy file's `sell_value` is what the refund rule gives: the cost times
                // `RefundPercent`. What a sale returns (`Definitions::refund`, RA5) is the
                // section's `Soylent` if it has one that is not 0 (plan appendix D, topic 9;
                // the economy file's `refunds`), else that value.
                let cost = value("Cost").unwrap_or("0");
                let by_rule = (convert(Conversion::Int, cost) * refund).div_euclid(100);
                differences.check(
                    id,
                    "sell_value",
                    &def.sell_value.to_string(),
                    &by_rule.to_string(),
                    cost,
                );
                let soylent = value("Soylent").filter(|text| convert(Conversion::Int, text) != 0);
                differences.check(
                    id,
                    "refund",
                    &definitions.refund(kind).to_string(),
                    &soylent
                        .map_or(by_rule, |text| convert(Conversion::Int, text))
                        .to_string(),
                    soylent.unwrap_or(cost),
                );
                // A type is in `refunds` exactly if its section has a `Soylent` that is not 0.
                assert_eq!(
                    def.fixed_refund.is_some(),
                    soylent.is_some(),
                    "{id}: the economy file's refunds and [{}] Soylent",
                    row.section
                );
                // `FreeUnit`: the unit that leaves the building when it is placed.
                let free = value("FreeUnit");
                let game: Vec<String> = def
                    .free_units
                    .iter()
                    .map(|&kind| definitions.unit(kind).expect("free unit").id.clone())
                    .collect();
                differences.check(
                    id,
                    "free_units",
                    &game.join(","),
                    free.map_or("", |section| {
                        game_id_of
                            .get(section)
                            .unwrap_or_else(|| panic!("[{}] gives unmapped {section}", row.section))
                    }),
                    free.unwrap_or(""),
                );
                // `Refinery=yes`: the building takes ore (the economy file's `refineries`).
                let (refinery, text) = {
                    let text = value("Refinery").unwrap_or("no");
                    (convert(Conversion::Flag, text), text)
                };
                differences.check(
                    id,
                    "refinery",
                    &u8::from(def.credits_per_ore.is_some()).to_string(),
                    &refinery.to_string(),
                    text,
                );
                // RA5: a refinery pays the value of a bail of ore and of gems.
                for (field, game, section) in [
                    ("credits_per_ore", def.credits_per_ore, "Riparius"),
                    ("credits_per_gem", def.credits_per_gem, "Cruentus"),
                ] {
                    if let Some(game) = game {
                        let text = reference.value(section, "Value");
                        differences.check(
                            id,
                            field,
                            &game.to_string(),
                            &convert(Conversion::Int, text).to_string(),
                            text,
                        );
                    }
                }
            }
        }

        // The factory group of a building: the ini's `Factory`, with `Naval=yes` a shipyard;
        // a unit is none.
        let group_of = |section: &BTreeMap<String, String>| {
            let naval = section.get("Naval").map(String::as_str) == Some("yes");
            match (section.get("Factory").map(String::as_str), naval) {
                (None, _) => "none",
                (Some("BuildingType"), _) => "buildings",
                (Some("InfantryType"), _) => "infantry",
                (Some("UnitType"), false) => "vehicles",
                (Some("UnitType"), true) => "naval",
                (Some("AircraftType"), _) => "aircraft",
                (Some(other), _) => panic!("unknown Factory={other}"),
            }
        };
        let factory = value("Factory").unwrap_or("");
        differences.check(
            id,
            "factory",
            match entry.factory {
                None => "none",
                Some(sim::arsenal::FactoryGroup::Buildings) => "buildings",
                Some(sim::arsenal::FactoryGroup::Infantry) => "infantry",
                Some(sim::arsenal::FactoryGroup::Vehicles) => "vehicles",
                Some(sim::arsenal::FactoryGroup::Naval) => "naval",
                Some(sim::arsenal::FactoryGroup::Aircraft) => "aircraft",
            },
            if row.building {
                group_of(section)
            } else {
                "none"
            },
            factory,
        );
        // The side: that of the mapping (which `scripts/ra2_reference.py` checks against the
        // owners of the ini), or none for an item that stands for sections of both sides.
        let sides: BTreeSet<&str> = mapping
            .iter()
            .filter(|other| other.game_id == row.game_id)
            .map(|other| other.side.as_str())
            .collect();
        let side = if sides.len() == 1 {
            row.side.as_str()
        } else {
            "none"
        };
        differences.check(
            id,
            "side",
            entry.side.map_or("none", |side| side.id()),
            side,
            &row.side,
        );
        // The producer: a building is built by the Construction Yard, a unit by the factory of
        // its type list (`[InfantryTypes]`: the barracks; else a shipyard with `Naval=yes`, an
        // airfield for the types of `[General] PadAircraft`, a War Factory for the others), of
        // the item's side where that side has such a factory (an item of no side: Allied).
        let wanted = if row.building {
            "buildings"
        } else if in_infantry_list(&reference, &row.section) {
            "infantry"
        } else if value("Naval") == Some("yes") {
            "naval"
        } else if list(reference.get("General", "PadAircraft")).contains(&row.section.as_str()) {
            "aircraft"
        } else {
            "vehicles"
        };
        let factories: Vec<&Mapped> = mapping
            .iter()
            .filter(|other| {
                other.building
                    && !LATER.contains(&other.note.as_str())
                    && group_of(reference.section(&other.section)) == wanted
            })
            .collect();
        let own_side = if side == "none" { "allied" } else { side };
        let of_side: Vec<&str> = factories
            .iter()
            .filter(|other| other.side == own_side)
            .map(|other| other.game_id.as_str())
            .collect();
        let acceptable: Vec<&str> = if of_side.is_empty() {
            factories
                .iter()
                .map(|other| other.game_id.as_str())
                .collect()
        } else {
            of_side
        };
        let producer = definitions
            .building(terms.producer)
            .expect("producer")
            .id
            .as_str();
        differences.check(
            id,
            "producer",
            producer,
            &if acceptable.contains(&producer) {
                producer.to_owned()
            } else {
                acceptable.join("|")
            },
            wanted,
        );
        // The domain: by `MovementZone` for a unit (`Fly`: air; `Water`: naval; the amphibious
        // zones: amphibious; else land); a building is on land, also a shipyard.
        let zone = value("MovementZone").unwrap_or("");
        differences.check(
            id,
            "domain",
            match entry.domain {
                sim::Domain::Land => "land",
                sim::Domain::Air => "air",
                sim::Domain::Naval => "naval",
                sim::Domain::Amphibious => "amphibious",
            },
            match zone {
                _ if row.building => "land",
                "Fly" => "air",
                "Water" => "naval",
                "Amphibious" | "AmphibiousDestroyer" | "AmphibiousCrusher" => "amphibious",
                _ => "land",
            },
            zone,
        );
        // The capacity: the `Passengers` of a unit; nothing else carries anything.
        let passengers = value("Passengers").filter(|_| !row.building).unwrap_or("0");
        differences.check(
            id,
            "capacity",
            &entry.capacity.to_string(),
            &convert(Conversion::Int, passengers).to_string(),
            passengers,
        );

        // The category: a building is a defence (`BuildCat=Combat`, the arsenal's `armory`) or
        // a building; a unit is infantry by its type list, else a vehicle.
        let build_cat = value("BuildCat").unwrap_or("");
        differences.check(
            id,
            "category",
            match entry.category {
                Category::Buildings => "buildings",
                Category::Armory => "armory",
                Category::Infantry => "infantry",
                Category::Vehicles => "vehicles",
            },
            match (row.building, build_cat) {
                (true, "Combat") => "armory",
                (true, _) => "buildings",
                (false, _) if in_infantry_list(&reference, &row.section) => "infantry",
                (false, _) => "vehicles",
            },
            build_cat,
        );

        // RA2. The build time: the cost times `BuildSpeed` game minutes per 1000 credits, in
        // ticks, at least 1, and for a wall times `WallBuildSpeedCoefficient` (plan section 4.2).
        let flag = |key: &str, default: &'static str| {
            let text = value(key).unwrap_or(default);
            (convert(Conversion::Flag, text), text)
        };
        // RA5: world-object traits, independently converted from reference keys/defaults.
        for (field, game, key) in [
            ("needs_engineer", entry.needs_engineer, "NeedsEngineer"),
            ("capturable", entry.capturable, "Capturable"),
            ("insignificant", entry.insignificant, "Insignificant"),
            ("immune", entry.immune, "Immune"),
            ("hospital", entry.hospital, "Hospital"),
            (
                "bridge_repair_hut",
                entry.bridge_repair_hut,
                "BridgeRepairHut",
            ),
            ("occupier", entry.occupier, "Occupier"),
            ("can_be_occupied", entry.can_be_occupied, "CanBeOccupied"),
        ] {
            let (expected, text) = flag(key, "no");
            differences.check(
                id,
                field,
                &u8::from(game).to_string(),
                &expected.to_string(),
                text,
            );
        }
        for (field, game, key) in [
            ("max_occupants", entry.max_occupants, "MaxNumberOccupants"),
            ("vehicle_docks", entry.vehicle_docks, "NumberOfDocks"),
            ("cash_startup", entry.cash_startup, "ProduceCashStartup"),
            ("cash_amount", entry.cash_amount, "ProduceCashAmount"),
            ("cash_delay", entry.cash_delay, "ProduceCashDelay"),
        ] {
            let text = value(key).unwrap_or("0");
            let expected = if field == "vehicle_docks" && !entry.unit_repair {
                0
            } else if field == "cash_delay" && text != "0" {
                convert(Conversion::Frames, text)
            } else {
                convert(Conversion::Int, text)
            };
            differences.check(id, field, &game.to_string(), &expected.to_string(), text);
        }
        let (wall, _) = flag("Wall", "no");
        let cost = value("Cost").unwrap_or("0");
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
            let adjacent = value("Adjacent").unwrap_or("2");
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

        // RA3. The traits of a unit (schema version 4): whether `[InfantryTypes]` lists it,
        // `Crusher` (no without the key), `Crushable` (without the key yes for infantry and no
        // for every other unit) and `Size` (1 without the key). A building has none of them.
        if row.building {
            assert!(
                !entry.infantry && !entry.crusher && !entry.crushable && entry.size == 0,
                "{id}: a building has no trait of a unit"
            );
        } else {
            let listed = reference
                .section("InfantryTypes")
                .iter()
                .find(|(_, section)| **section == row.section);
            let infantry = listed.is_some();
            differences.check(
                id,
                "infantry",
                &u8::from(entry.infantry).to_string(),
                &u8::from(infantry).to_string(),
                &listed.map_or_else(String::new, |(key, section)| {
                    format!("[InfantryTypes] {key}={section}")
                }),
            );
            let (crusher, text) = flag("Crusher", "no");
            differences.check(
                id,
                "crusher",
                &u8::from(entry.crusher).to_string(),
                &crusher.to_string(),
                text,
            );
            let (crushable, text) = flag("Crushable", if infantry { "yes" } else { "no" });
            differences.check(
                id,
                "crushable",
                &u8::from(entry.crushable).to_string(),
                &crushable.to_string(),
                text,
            );
            let size = value("Size").unwrap_or("1");
            differences.check(
                id,
                "size",
                &entry.size.to_string(),
                &convert(Conversion::Int, size).to_string(),
                size,
            );
        }

        // RA4. The traits of schema version 5 (docs/DECISIONS.md R46): `Crewed` (no without
        // the key) and `SensorsSight` of a type with `Sensors=yes` for every item; for a unit
        // `Trainable` (yes without the key), the tokens of `VeteranAbilities` and
        // `EliteAbilities`, `SelfHealing`, `Ammo` (unlimited without the key: 0),
        // `AirportBound`, `Cloakable`, the spawned type with its number and its two rates in
        // ticks (0 frames stay 0), the weapon with `Spawner=yes`, and whether the type is
        // `[General] V3RocketType` or `DMislType`; for a building `NumberOfDocks` if a
        // mapped `AirportBound` aircraft names it as its `Dock`.
        {
            let (crewed, text) = flag("Crewed", "no");
            differences.check(
                id,
                "crewed",
                &u8::from(entry.crewed).to_string(),
                &crewed.to_string(),
                text,
            );
            let (sensors, _) = flag("Sensors", "no");
            let sight = value("SensorsSight").unwrap_or("0");
            differences.check(
                id,
                "sensors_sight",
                &entry.sensors_sight.to_string(),
                &(sensors * convert(Conversion::Int, sight)).to_string(),
                sight,
            );
            let users: Vec<&Mapped> = mapping
                .iter()
                .filter(|other| {
                    !other.building
                        && !LATER.contains(&other.note.as_str())
                        && reference
                            .get(&other.section, "AirportBound")
                            .is_some_and(|text| convert(Conversion::Flag, text) == 1)
                        && list(reference.get(&other.section, "Dock"))
                            .contains(&row.section.as_str())
                })
                .collect();
            let docks = value("NumberOfDocks").unwrap_or("0");
            let expected = if row.building && !users.is_empty() {
                convert(Conversion::Int, docks)
            } else {
                0
            };
            differences.check(
                id,
                "docks",
                &entry.docks.to_string(),
                &expected.to_string(),
                docks,
            );
        }
        if row.building {
            assert!(
                !entry.trainable
                    && entry.veteran_abilities.is_empty()
                    && entry.elite_abilities.is_empty()
                    && !entry.self_healing
                    && entry.ammo == 0
                    && !entry.airport_bound
                    && !entry.cloakable
                    && entry.spawns.is_none()
                    && entry.launcher.is_none()
                    && entry.missile.is_none(),
                "{id}: a building has no trait of a unit"
            );
        } else {
            for (field, game, key, default) in [
                ("trainable", entry.trainable, "Trainable", "yes"),
                ("self_healing", entry.self_healing, "SelfHealing", "no"),
                ("airport_bound", entry.airport_bound, "AirportBound", "no"),
                ("cloakable", entry.cloakable, "Cloakable", "no"),
            ] {
                let (expected, text) = flag(key, default);
                differences.check(
                    id,
                    field,
                    &u8::from(game).to_string(),
                    &expected.to_string(),
                    text,
                );
            }
            for (field, game, key) in [
                (
                    "veteran_abilities",
                    &entry.veteran_abilities,
                    "VeteranAbilities",
                ),
                ("elite_abilities", &entry.elite_abilities, "EliteAbilities"),
            ] {
                let tokens = value(key).unwrap_or("");
                let expected = list(Some(tokens))
                    .into_iter()
                    .map(str::to_ascii_lowercase)
                    .collect();
                let game = game.iter().map(|ability| ability.id().to_owned()).collect();
                differences.check(id, field, &joined(game), &joined(expected), tokens);
            }
            let ammo = value("Ammo").unwrap_or("-1");
            differences.check(
                id,
                "ammo",
                &entry.ammo.to_string(),
                &convert(Conversion::Int, ammo).max(0).to_string(),
                ammo,
            );
            let spawns = value("Spawns");
            differences.check(
                id,
                "spawns",
                &entry
                    .spawns
                    .and_then(|kind| definitions.unit(kind))
                    .map_or("none".to_owned(), |unit| unit.id.clone()),
                spawns.map_or("none", |section| {
                    game_id_of
                        .get(section)
                        .unwrap_or_else(|| panic!("[{}] spawns unmapped {section}", row.section))
                }),
                spawns.unwrap_or(""),
            );
            let ticks = |key: &str| {
                let text = value(key).filter(|_| spawns.is_some()).unwrap_or("0");
                let frames = decimal(text);
                (
                    if frames.0 == 0 {
                        0
                    } else {
                        convert(Conversion::Frames, text)
                    },
                    text,
                )
            };
            let number = value("SpawnsNumber")
                .filter(|_| spawns.is_some())
                .unwrap_or("0");
            for (field, game, (expected, text)) in [
                (
                    "spawns_number",
                    entry.spawns_number,
                    (convert(Conversion::Int, number), number),
                ),
                ("spawn_regen", entry.spawn_regen, ticks("SpawnRegenRate")),
                ("spawn_reload", entry.spawn_reload, ticks("SpawnReloadRate")),
            ] {
                differences.check(id, field, &game.to_string(), &expected.to_string(), text);
            }
            let spawner = |key: &str| {
                value(key).is_some_and(|weapon| {
                    reference
                        .0
                        .get(weapon)
                        .and_then(|section| section.get("Spawner"))
                        .is_some_and(|text| convert(Conversion::Flag, text) == 1)
                })
            };
            let expected = match (spawns.is_some(), spawner("Primary"), spawner("Secondary")) {
                (true, _, true) => "secondary",
                (true, true, false) => "primary",
                _ => "none",
            };
            differences.check(
                id,
                "launcher",
                match entry.launcher {
                    None => "none",
                    Some(sim::Launcher::Primary) => "primary",
                    Some(sim::Launcher::Secondary) => "secondary",
                },
                expected,
                value("Spawns").unwrap_or(""),
            );
            let named = |key: &str| reference.value("General", key) == row.section;
            let expected = match (named("V3RocketType"), named("DMislType")) {
                (true, _) => "v3",
                (_, true) => "dmisl",
                _ => "none",
            };
            differences.check(
                id,
                "missile",
                match entry.missile {
                    None => "none",
                    Some(sim::Missile::V3) => "v3",
                    Some(sim::Missile::DMisl) => "dmisl",
                },
                expected,
                &row.section,
            );
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

    // Every field of every item's entries in the shipped files is audited above or is on the
    // list of the game's own fields, so that a field added later cannot go unaudited unseen
    // (advisory review 1, data P1-2).
    for &(file, list, fields) in ITEM_FIELDS {
        let shipped = entry_fields(file, list);
        let listed: BTreeSet<&str> = fields.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            listed.len(),
            fields.len(),
            "{file} {list}: a field listed twice"
        );
        assert_eq!(
            shipped.iter().map(String::as_str).collect::<BTreeSet<_>>(),
            listed,
            "{file} {list}: the fields of the shipped entries and those the audit lists"
        );
        for (name, kind) in fields {
            match kind {
                Field::Key => {}
                Field::Own(reason) => assert!(!reason.is_empty(), "{file} {list}.{name}"),
                Field::Audited(audited) => assert!(
                    differences.checked.contains(*audited),
                    "{file} {list}.{name}: the audit never compared {audited:?}"
                ),
            }
        }
    }
    differences.finish();
}

/// What the audit does with a field of an item's entry in a shipped data file.
enum Field {
    /// The id that names the item.
    Key,
    /// Compared with the reference, under this name in `ra2_differences.tsv`.
    Audited(&'static str),
    /// The game's own value, with the reason; no reference value corresponds to it (yet).
    Own(&'static str),
}

/// A list of a shipped data file: the file, the list, and each field of its entries with what
/// the audit does with it.
type ListFields = (&'static str, &'static str, &'static [(&'static str, Field)]);

/// Every field of the entries of units and buildings in the shipped data files: file, list,
/// and for each field what the audit does with it. The audit fails on a field of a shipped
/// entry that is not here, on one here that no entry has, and on an audited one it did not
/// compare.
const ITEM_FIELDS: &[ListFields] = &[
    (
        "units.ron",
        "units",
        &[
            ("id", Field::Key),
            ("speed", Field::Audited("speed")),
            ("sprite", Field::Own("art")),
        ],
    ),
    (
        "economy.ron",
        "buildings",
        &[
            ("id", Field::Key),
            ("sprite", Field::Own("art")),
            (
                "width",
                Field::Own("footprints are the game's until a later milestone (R9)"),
            ),
            (
                "height",
                Field::Own("footprints are the game's until a later milestone (R9)"),
            ),
            ("cost", Field::Audited("cost")),
            ("build_time", Field::Audited("build_time")),
            ("producer", Field::Audited("producer")),
            ("prerequisites", Field::Audited("prerequisites")),
            ("power_produced", Field::Audited("power_produced")),
            ("power_consumed", Field::Audited("power_consumed")),
            ("sell_value", Field::Audited("sell_value")),
            ("free_units", Field::Audited("free_units")),
        ],
    ),
    (
        "economy.ron",
        "units",
        &[
            ("unit", Field::Key),
            ("cost", Field::Audited("cost")),
            ("build_time", Field::Audited("build_time")),
            ("producer", Field::Audited("producer")),
            ("prerequisites", Field::Audited("prerequisites")),
        ],
    ),
    (
        "economy.ron",
        "harvesters",
        &[
            ("unit", Field::Key),
            ("capacity", Field::Audited("storage")),
            (
                "harvest_rate",
                Field::Own("one bail at a time; the ini has no key for the pace of harvesting"),
            ),
            (
                "harvest_interval",
                Field::Own("19 ticks a bail; the ini has no key for the pace of harvesting"),
            ),
        ],
    ),
    (
        "economy.ron",
        "refineries",
        &[
            ("building", Field::Audited("refinery")),
            ("credits_per_ore", Field::Audited("credits_per_ore")),
            ("credits_per_gem", Field::Audited("credits_per_gem")),
        ],
    ),
    (
        "economy.ron",
        "refunds",
        &[
            ("building", Field::Key),
            ("credits", Field::Audited("refund")),
        ],
    ),
    (
        "combat.ron",
        "units",
        &[
            ("unit", Field::Key),
            ("hit_points", Field::Audited("strength")),
            ("armour", Field::Audited("armour")),
            ("sight", Field::Audited("sight")),
            ("weapons", Field::Audited("weapons")),
            ("elite_weapons", Field::Audited("elite_weapons")),
            ("air_range_bonus", Field::Audited("air_range_bonus")),
            ("land_targeting", Field::Audited("land_targeting")),
            ("naval_targeting", Field::Audited("naval_targeting")),
            ("underwater", Field::Audited("underwater")),
            ("organic", Field::Audited("organic")),
        ],
    ),
    (
        "combat.ron",
        "buildings",
        &[
            ("building", Field::Key),
            ("hit_points", Field::Audited("strength")),
            ("armour", Field::Audited("armour")),
            ("sight", Field::Audited("sight")),
            ("weapons", Field::Audited("weapons")),
            ("elite_weapons", Field::Audited("elite_weapons")),
            ("air_range_bonus", Field::Audited("air_range_bonus")),
            ("land_targeting", Field::Audited("land_targeting")),
            ("naval_targeting", Field::Audited("naval_targeting")),
            ("wall", Field::Audited("wall")),
        ],
    ),
    (
        "arsenal.ron",
        "entries",
        &[
            ("id", Field::Key),
            ("label", Field::Own("the game's text")),
            ("description", Field::Own("the game's text")),
            ("category", Field::Audited("category")),
            ("domain", Field::Audited("domain")),
            (
                "role",
                Field::Own("the specialist mechanism of M9, replaced by RA6"),
            ),
            (
                "cooldown",
                Field::Own("a term of the role (M9); RA6 ports the recharge times"),
            ),
            ("radius", Field::Own("a term of the role (M9)")),
            ("strength", Field::Own("a term of the role (M9)")),
            ("capacity", Field::Audited("capacity")),
            ("art", Field::Own("art")),
            ("side", Field::Audited("side")),
            ("factory", Field::Audited("factory")),
            ("owner_sides", Field::Audited("owner_sides")),
            ("required_countries", Field::Audited("required_countries")),
            ("forbidden_countries", Field::Audited("forbidden_countries")),
            ("tech_level", Field::Audited("tech_level")),
            ("prerequisite_groups", Field::Audited("prerequisites")),
            ("build_limit", Field::Audited("build_limit")),
            ("adjacent", Field::Audited("adjacent")),
            ("base_normal", Field::Audited("base_normal")),
            ("radar", Field::Audited("radar")),
            ("sellable", Field::Audited("sellable")),
            ("repairable", Field::Audited("repairable")),
            ("powered", Field::Audited("powered")),
            ("is_powered", Field::Audited("is_powered")),
            ("unit_repair", Field::Audited("unit_repair")),
            ("crusher", Field::Audited("crusher")),
            ("crushable", Field::Audited("crushable")),
            ("size", Field::Audited("size")),
            ("infantry", Field::Audited("infantry")),
            ("trainable", Field::Audited("trainable")),
            ("veteran_abilities", Field::Audited("veteran_abilities")),
            ("elite_abilities", Field::Audited("elite_abilities")),
            ("crewed", Field::Audited("crewed")),
            ("self_healing", Field::Audited("self_healing")),
            ("ammo", Field::Audited("ammo")),
            ("airport_bound", Field::Audited("airport_bound")),
            ("docks", Field::Audited("docks")),
            ("spawns", Field::Audited("spawns")),
            ("spawns_number", Field::Audited("spawns_number")),
            ("spawn_regen", Field::Audited("spawn_regen")),
            ("spawn_reload", Field::Audited("spawn_reload")),
            ("launcher", Field::Audited("launcher")),
            ("missile", Field::Audited("missile")),
            ("cloakable", Field::Audited("cloakable")),
            ("sensors_sight", Field::Audited("sensors_sight")),
            ("needs_engineer", Field::Audited("needs_engineer")),
            ("capturable", Field::Audited("capturable")),
            ("insignificant", Field::Audited("insignificant")),
            ("immune", Field::Audited("immune")),
            ("hospital", Field::Audited("hospital")),
            ("bridge_repair_hut", Field::Audited("bridge_repair_hut")),
            ("occupier", Field::Audited("occupier")),
            ("can_be_occupied", Field::Audited("can_be_occupied")),
            ("max_occupants", Field::Audited("max_occupants")),
            ("vehicle_docks", Field::Audited("vehicle_docks")),
            ("cash_startup", Field::Audited("cash_startup")),
            ("cash_amount", Field::Audited("cash_amount")),
            ("cash_delay", Field::Audited("cash_delay")),
        ],
    ),
];

/// The names of the fields of the entries of the list `list` (from the line that ends with
/// `list: [` to the line that closes it, one entry a line, as the generator writes them) of the shipped file `file`: every entry must
/// have the same fields. Read from the text, so that it sees what the file holds and not what
/// the loader keeps.
fn entry_fields(file: &str, list: &str) -> Vec<String> {
    let path = data_dir().join(file);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut lines = text
        .lines()
        .skip_while(|line| !line.ends_with(&format!("{list}: [")))
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with(']'))
        .peekable();
    assert!(lines.peek().is_some(), "{file}: no list {list}");
    let mut fields: Option<Vec<String>> = None;
    for line in lines {
        let line = line.trim_start();
        assert!(
            line.starts_with('('),
            "{file} {list}: not an entry: {line:?}"
        );
        let mut found = Vec::new();
        let mut depth = 0;
        let mut word = String::new();
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '"' => {
                    // A string: to its closing quote.
                    while let Some(inner) = chars.next() {
                        match inner {
                            '\\' => {
                                chars.next();
                            }
                            '"' => break,
                            _ => {}
                        }
                    }
                    word.clear();
                }
                '/' if chars.peek() == Some(&'/') => break,
                '(' | '[' => {
                    depth += 1;
                    word.clear();
                }
                ')' | ']' => {
                    depth -= 1;
                    word.clear();
                }
                ':' => {
                    if depth == 1 && !word.is_empty() {
                        found.push(std::mem::take(&mut word));
                    }
                    word.clear();
                }
                c if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' => word.push(c),
                _ => word.clear(),
            }
        }
        match &fields {
            None => fields = Some(found),
            Some(first) => assert_eq!(&found, first, "{file} {list}: {line:?}"),
        }
    }
    fields.unwrap_or_default()
}

// --- The audit of the weapons, the warheads and the area effects (RA3) -----------------------------

/// The armour types in the order of a warhead's `Verses` (docs/RA2_PARITY_PLAN.md appendix D).
const ARMOURS: [&str; 11] = [
    "none",
    "flak",
    "plate",
    "light",
    "medium",
    "heavy",
    "wood",
    "steel",
    "concrete",
    "special_1",
    "special_2",
];

/// Leptons per frame as subcells per tick: `round(leptons x 3 / 4)`, at least 1.
fn leptons_per_tick(leptons: i128) -> i128 {
    ((2 * leptons * 3 + 4).div_euclid(8)).max(1)
}

/// The largest integer whose square is at most `value`.
fn floor_sqrt(value: i128) -> i128 {
    let mut root = 0_i128;
    while (root + 1) * (root + 1) <= value {
        root += 1;
    }
    root
}

#[test]
fn every_shipped_weapon_warhead_and_effect_equals_its_reference_or_is_a_listed_difference() {
    let reference = Reference::load();
    let definitions = shipped();
    let combat = combat_mapping();
    let mut differences = Differences::load();
    // The other tests of this file meet the differences of units, buildings and rules.
    let foreign: Vec<usize> = differences
        .rows
        .iter()
        .enumerate()
        .filter(|(_, (id, ..))| !COMBAT_PREFIXES.iter().any(|prefix| id.starts_with(prefix)))
        .map(|(index, _)| index)
        .collect();
    differences.used.extend(foreign);
    assert_eq!(definitions.armour_classes(), ARMOURS);
    let flag = |section: &str, key: &str, default: &'static str| {
        let text = reference.get(section, key).unwrap_or(default);
        (convert(Conversion::Flag, text), text.to_owned())
    };

    // Warheads: `Verses` in the order of the armour types, `CellSpread` in subcells,
    // `PercentAtMax` as percent (1 without the key), and the flags `Wall` and `Wood`.
    let mut mapped: BTreeSet<&str> = BTreeSet::new();
    for (game_id, section) in &combat.warheads {
        mapped.insert(game_id.as_str());
        let id = format!("warhead.{game_id}");
        let warhead = definitions
            .warhead_kind(game_id)
            .and_then(|kind| definitions.warhead(kind))
            .unwrap_or_else(|| panic!("{game_id} is mapped but not shipped"));
        let verses = reference.value(section, "Verses");
        let expected: Vec<String> = verses
            .split(',')
            .map(|text| convert(Conversion::Int, text.trim_end_matches('%')).to_string())
            .collect();
        assert_eq!(expected.len(), ARMOURS.len(), "[{section}] Verses");
        let game: Vec<String> = warhead.versus.iter().map(i32::to_string).collect();
        differences.check(&id, "versus", &game.join(","), &expected.join(","), verses);
        let spread = reference.get(section, "CellSpread").unwrap_or("0");
        differences.check(
            &id,
            "cell_spread",
            &warhead.cell_spread.to_string(),
            &convert(Conversion::Cells, spread).to_string(),
            spread,
        );
        let at_max = reference.get(section, "PercentAtMax").unwrap_or("1");
        differences.check(
            &id,
            "percent_at_max",
            &warhead.percent_at_max.to_string(),
            &convert(Conversion::Percent, at_max).to_string(),
            at_max,
        );
        for (field, game, key) in [
            ("wall", warhead.wall, "Wall"),
            ("wood", warhead.wood, "Wood"),
        ] {
            let (expected, text) = flag(section, key, "no");
            differences.check(
                &id,
                field,
                &i128::from(game).to_string(),
                &expected.to_string(),
                &text,
            );
        }
    }
    for warhead in definitions.warheads() {
        assert!(
            mapped.contains(warhead.id.as_str()),
            "the warhead {} is shipped but not mapped to an ini section",
            warhead.id
        );
    }

    // Weapons (docs/RA2_PARITY_PLAN.md sections 2 and 4.4).
    let gravity = convert(Conversion::Int, reference.value("AudioVisual", "Gravity"));
    let ballistic_scatter = convert(
        Conversion::Cells,
        reference.value("CombatDamage", "BallisticScatter"),
    );
    let mut mapped: BTreeSet<&str> = BTreeSet::new();
    for (game_id, section) in &combat.weapons {
        mapped.insert(game_id.as_str());
        let id = format!("weapon.{game_id}");
        let weapon = definitions
            .weapon_kind(game_id)
            .and_then(|kind| definitions.weapon(kind))
            .unwrap_or_else(|| panic!("{game_id} is mapped but not shipped"));
        let mut number = |field: &str, game: i128, key: &str, default: Option<&'static str>, c| {
            let text = match (reference.get(section, key), default) {
                (Some(text), _) => text,
                (None, Some(default)) => default,
                (None, None) => panic!("the reference has no [{section}] {key}"),
            };
            differences.check(
                &id,
                field,
                &game.to_string(),
                &convert(c, text).to_string(),
                text,
            );
        };
        number(
            "damage",
            i128::from(weapon.damage),
            "Damage",
            None,
            Conversion::Int,
        );
        number(
            "range",
            i128::from(weapon.range),
            "Range",
            None,
            Conversion::Cells,
        );
        number(
            "min_range",
            i128::from(weapon.min_range),
            "MinimumRange",
            Some("0"),
            Conversion::Cells,
        );
        number(
            "reload",
            i128::from(weapon.reload),
            "ROF",
            None,
            Conversion::Frames,
        );
        number(
            "burst",
            i128::from(weapon.burst),
            "Burst",
            Some("1"),
            Conversion::Int,
        );
        // The warhead: the game id of the ini's, or `?Name` for one the game does not have.
        let warhead = reference.value(section, "Warhead");
        differences.check(
            &id,
            "warhead",
            &definitions
                .warhead(weapon.warhead)
                .map_or(String::new(), |warhead| warhead.id.clone()),
            &combat
                .warhead_id
                .get(warhead)
                .cloned()
                .unwrap_or_else(|| format!("?{warhead}")),
            warhead,
        );
        // The projectile: an invisible one, and the leap of a unit that is its own projectile
        // (`LimboLaunch`), arrive at once; an arcing shell flies at
        // floor(sqrt(1.2 x Gravity x range)) leptons per frame, whatever the weapon's `Speed`
        // (an assumption, docs/DECISIONS.md R34); every other one at the weapon's `Speed`,
        // read as a unit's (`min(Speed x 256 div 100, 255)` leptons per frame).
        let projectile = reference.value(section, "Projectile");
        let (inviso, _) = flag(projectile, "Inviso", "no");
        let (limbo, _) = flag(section, "LimboLaunch", "no");
        let (arcing, _) = flag(projectile, "Arcing", "no");
        let (expected, source) = if inviso == 1 || limbo == 1 {
            (0, format!("{projectile}:instant"))
        } else if arcing == 1 {
            let range = reference.value(section, "Range");
            let reach = convert(Conversion::Cells, range);
            (
                leptons_per_tick(floor_sqrt((reach * gravity * 6).div_euclid(5))),
                format!("{projectile}:arcing:{range}"),
            )
        } else {
            let speed = reference.value(section, "Speed");
            let leptons = (convert(Conversion::Int, speed) * 256)
                .div_euclid(100)
                .min(255);
            (leptons_per_tick(leptons), format!("{projectile}:{speed}"))
        };
        differences.check(
            &id,
            "projectile_speed",
            &weapon.projectile_speed.to_string(),
            &expected.to_string(),
            &source,
        );
        for (field, game, key, default) in [
            ("anti_air", weapon.anti_air, "AA", "no"),
            ("anti_ground", weapon.anti_ground, "AG", "yes"),
        ] {
            let (expected, text) = flag(projectile, key, default);
            differences.check(
                &id,
                field,
                &i128::from(game).to_string(),
                &expected.to_string(),
                &text,
            );
        }
        // Scatter (section 4.4): an `Inaccurate` projectile lands up to `BallisticScatter`
        // from its target at full range, an invisible one (the flak in the air) up to twice
        // that.
        let (inaccurate, text) = flag(projectile, "Inaccurate", "no");
        differences.check(
            &id,
            "scatter",
            &weapon.scatter.to_string(),
            &(inaccurate * ballistic_scatter * (1 + inviso)).to_string(),
            &text,
        );
    }
    for weapon in definitions.weapons() {
        assert!(
            mapped.contains(weapon.id.as_str()),
            "the weapon {} is shipped but not mapped to an ini section",
            weapon.id
        );
    }

    // The area effects: damage and warhead of the ini where it names them. The charge of a
    // commando and the pulse of a radiation zone are mechanics of M9 with a damage of the
    // game's own until RA6; their warheads are the ini's.
    type Key = (&'static str, &'static str);
    let effects: [(sim::Effect, Option<Key>, Key); 9] = [
        (
            sim::Effect::Nuke,
            Some(("NukePayload", "Damage")),
            ("NukePayload", "Warhead"),
        ),
        (
            sim::Effect::Lightning,
            Some(("General", "LightningDamage")),
            ("General", "LightningWarhead"),
        ),
        (
            sim::Effect::IvanBomb,
            Some(("CombatDamage", "IvanDamage")),
            ("CombatDamage", "IvanWarhead"),
        ),
        (sim::Effect::C4, None, ("CombatDamage", "C4Warhead")),
        (
            sim::Effect::Radiation,
            None,
            ("Radiation", "RadSiteWarhead"),
        ),
        // RA4: the spawned missiles, and each of an elite launcher.
        (
            sim::Effect::V3Rocket,
            Some(("General", "V3RocketDamage")),
            ("CombatDamage", "V3Warhead"),
        ),
        (
            sim::Effect::V3RocketElite,
            Some(("General", "V3RocketEliteDamage")),
            ("CombatDamage", "V3EliteWarhead"),
        ),
        (
            sim::Effect::DMisl,
            Some(("General", "DMislDamage")),
            ("CombatDamage", "DMislWarhead"),
        ),
        (
            sim::Effect::DMislElite,
            Some(("General", "DMislEliteDamage")),
            ("CombatDamage", "DMislEliteWarhead"),
        ),
    ];
    assert_eq!(effects.map(|(effect, ..)| effect), sim::Effect::ALL);
    for (effect, damage, (section, key)) in effects {
        let id = format!("effect.{}", effect.id());
        let game = definitions
            .effect(effect)
            .unwrap_or_else(|| panic!("the effect {} is not shipped", effect.id()));
        match damage {
            Some((section, key)) => {
                let text = reference.value(section, key);
                differences.check(
                    &id,
                    "damage",
                    &game.damage.to_string(),
                    &convert(Conversion::Int, text).to_string(),
                    text,
                );
            }
            // The game's own value (docs/DECISIONS.md R35): it is a difference, and needs its
            // row, which names the value; the reference has none ("-").
            None => differences.check(&id, "damage", &game.damage.to_string(), "-", "-"),
        }
        let warhead = reference.value(section, key);
        differences.check(
            &id,
            "warhead",
            &definitions
                .warhead(game.warhead)
                .map_or(String::new(), |warhead| warhead.id.clone()),
            &combat
                .warhead_id
                .get(warhead)
                .cloned()
                .unwrap_or_else(|| format!("?{warhead}")),
            warhead,
        );
    }
    differences.finish();
}

// --- The audit of rules.ron -------------------------------------------------------------------------

/// The scalar values of `rules.ron`: group, field, ini section, ini key, conversion.
/// The scalar rules the ini has no key for (RA5, docs/RA2_PARITY_PLAN.md section 4.10 and
/// appendix D, topic 8: assumptions from the overlay count and the ancestor engine): group,
/// field, and the game's own value.
const OWN_RULES: &[(&str, &str, i32)] = &[
    ("economy", "ore_max_bails", 12),
    ("economy", "ore_spread_above", 6),
];

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

    // Every scalar field has exactly one row of the table, so that none goes unaudited; a
    // field the ini has no key for is the game's own and has the value recorded here.
    for (group, fields) in scalar_groups(rules) {
        for (field, game) in fields {
            if let Some((.., own)) = OWN_RULES
                .iter()
                .find(|(g, f, _)| *g == group && *f == field)
            {
                assert_eq!(
                    game, *own,
                    "rules.ron {group}.{field}: the game's own value"
                );
                continue;
            }
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
        RULES.len() + OWN_RULES.len(),
        scalar_groups(rules)
            .iter()
            .map(|(_, fields)| fields.len())
            .sum::<usize>(),
        "the audit table names a field the rules file does not have"
    );

    // RA5: the units of a crate are the mapped units with `CrateGoodie=yes`.
    let goodies: BTreeSet<String> = mapping()
        .into_iter()
        .filter(|row| !row.building)
        .filter(|row| {
            reference
                .get(&row.section, "CrateGoodie")
                .is_some_and(|text| convert(Conversion::Flag, text) == 1)
        })
        .map(|row| row.game_id)
        .collect();
    let listed: BTreeSet<String> = rules
        .crate_units
        .iter()
        .map(|&kind| definitions.unit(kind).expect("a crate's unit").id.clone())
        .collect();
    assert_eq!(listed, goodies, "rules.ron crate_units and CrateGoodie=yes");
    assert_eq!(listed.len(), rules.crate_units.len());
    assert!(!listed.is_empty());

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
    // by all of its countries, other than the base units; infantry by `[InfantryTypes]`.
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
                if in_infantry_list(&reference, &row.section) {
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
        speed(&unit(&[("Speed", "4"), ("Locomotor", foot)]), true).0,
        11,
        "infantry on foot (a section of `[InfantryTypes]`): the factor 3/2"
    );
    assert_eq!(
        speed(&unit(&[("Speed", "7"), ("Locomotor", foot)]), false).0,
        13
    );
    assert_eq!(
        speed(&unit(&[("Speed", "14"), ("Locomotor", foot)]), false).0,
        26
    );
    // A jumpjet moves by `JumpjetSpeed`; a speed above the engine's 255 leptons is capped.
    let jumpjet = "{92612C46-F71F-11d1-AC9F-006008055BB5}";
    assert_eq!(
        speed(
            &unit(&[
                ("Speed", "9"),
                ("JumpjetSpeed", "30"),
                ("Locomotor", jumpjet)
            ]),
            true
        )
        .0,
        23,
        "a jumpjet, also one of the infantry list (the Rocketeer): no factor"
    );
    assert_eq!(
        speed(&unit(&[("Speed", "200"), ("Locomotor", foot)]), false).0,
        191
    );
}
