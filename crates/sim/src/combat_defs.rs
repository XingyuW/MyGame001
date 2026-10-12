//! Immutable combat definitions loaded from RON (PLAN.md sections 5 and 7, M4; docs/DECISIONS.md
//! D28).
//!
//! Combat file, schema version 1:
//!
//! ```ron
//! (
//!     schema_version: 1,
//!     armour_classes: ["light", "heavy", "structure"],
//!     warheads: [
//!         (id: "ap", versus: [
//!             (armour: "light", percent: 50),
//!             (armour: "heavy", percent: 100),
//!             (armour: "structure", percent: 60),
//!         ]),
//!     ],
//!     weapons: [
//!         (id: "cannon", damage: 40, range: 1280, reload: 30, warhead: "ap",
//!          projectile_speed: 160),
//!     ],
//!     units: [
//!         (unit: "light_tank", hit_points: 300, armour: "heavy", sight: 6,
//!          weapons: ["cannon"]),
//!     ],
//!     buildings: [
//!         (building: "construction_yard", hit_points: 1500, armour: "structure", sight: 5,
//!          weapons: []),
//!     ],
//! )
//! ```
//!
//! Every field is required, so there are no silent defaults. Names follow the rules of unit ids
//! (`definitions::check_name`). Every reference (an armour class, a warhead, a weapon, a unit type
//! of the units file, a building type of the economy file) must be defined; every warhead lists
//! every armour class exactly once; and every unit type and every building type has exactly one
//! entry, so no unit or building is silently left without hit points, armour, or sight. Ranges are
//! checked on the parsed values; the limits below keep every quantity the simulation derives from
//! them inside its integer types (invariant I12) and bound the number of projectiles in flight.

use std::path::Path;

use serde::Deserialize;

use crate::data::{self, DataError, DataErrorKind, MAX_DATA_FILE_BYTES, Shape};
use crate::definitions::{MAX_UNIT_TYPES, check_name};
use crate::economy_defs::MAX_BUILDING_TYPES;
use crate::hash::Fnv1a64;

/// The schema version of the combat file this crate reads.
pub const COMBAT_SCHEMA_VERSION: i64 = 1;

/// Largest number of armour classes.
pub const MAX_ARMOUR_CLASSES: usize = 16;

/// Largest number of warheads.
pub const MAX_WARHEADS: usize = 32;

/// Largest number of weapons.
pub const MAX_WEAPONS: usize = 64;

/// Largest number of weapons of one unit or building type.
pub const MAX_WEAPONS_PER_TYPE: usize = 1;

/// Largest hit points of a unit or building type.
pub const MAX_HIT_POINTS: i32 = 1_000_000;

/// Largest base damage of a weapon. With [`MAX_VERSUS_PERCENT`] the damage of one shot is at most
/// 10^6, so `base × versus` fits an `i32` and many shots on one target in one tick are summed in
/// `i64` (equation 2).
pub const MAX_DAMAGE: i32 = 100_000;

/// Largest percentage a warhead deals against an armour class.
pub const MAX_VERSUS_PERCENT: i32 = 1_000;

/// Smallest weapon range in subcells: 1.5 cells. A unit standing at a cell centre then reaches a
/// target at the centre of any neighbouring cell (362 subcells away diagonally) and the footprint
/// of a building whose ring it stands in, so every weapon can hit something its unit can walk up
/// to.
pub const MIN_WEAPON_RANGE: u32 = 384;

/// Largest weapon range in subcells: 16 cells.
pub const MAX_WEAPON_RANGE: u32 = 4096;

/// Longest reload time of a weapon in ticks (one minute).
pub const MAX_RELOAD: u32 = 1200;

/// Smallest speed of a travelling projectile in subcells per tick. A shot is fired only at a
/// target within range, so a projectile flies at most `MAX_WEAPON_RANGE / MIN_PROJECTILE_SPEED`
/// = 32 ticks, and a weapon has at most 32 projectiles in flight (docs/DECISIONS.md D28).
pub const MIN_PROJECTILE_SPEED: u32 = 128;

/// Largest speed of a travelling projectile in subcells per tick.
pub const MAX_PROJECTILE_SPEED: u32 = 4096;

/// Largest sight radius in cells.
pub const MAX_SIGHT: u32 = 16;

/// Index of an armour class in [`Definitions`](crate::Definitions).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArmourClass(pub u16);

/// Index of a warhead in [`Definitions`](crate::Definitions).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WarheadKind(pub u16);

/// Index of a weapon in [`Definitions`](crate::Definitions).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WeaponKind(pub u16);

/// A warhead: the percentage of a weapon's base damage it deals against each armour class
/// (PLAN.md section 5, equation 2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WarheadDef {
    pub id: String,
    /// Indexed by [`ArmourClass`]; one entry per armour class, each from 0 to
    /// [`MAX_VERSUS_PERCENT`].
    pub versus: Vec<i32>,
}

/// A weapon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponDef {
    pub id: String,
    /// Base damage, from 1 to [`MAX_DAMAGE`].
    pub damage: i32,
    /// Range in subcells, from [`MIN_WEAPON_RANGE`] to [`MAX_WEAPON_RANGE`] (equation 1).
    pub range: u32,
    /// Ticks between two shots, from 1 to [`MAX_RELOAD`].
    pub reload: u32,
    pub warhead: WarheadKind,
    /// 0 for an instant weapon, whose damage is dealt in the tick it fires; otherwise the speed of
    /// its projectile in subcells per tick, from [`MIN_PROJECTILE_SPEED`] to
    /// [`MAX_PROJECTILE_SPEED`].
    pub projectile_speed: u32,
}

/// The combat terms of a unit or building type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CombatTerms {
    /// From 1 to [`MAX_HIT_POINTS`].
    pub hit_points: i32,
    pub armour: ArmourClass,
    /// Sight radius in cells, from 0 (its own cells only) to [`MAX_SIGHT`] (equation 6).
    pub sight: u32,
    /// At most one weapon ([`MAX_WEAPONS_PER_TYPE`]).
    pub weapon: Option<WeaponKind>,
}

/// The combat part of [`Definitions`](crate::Definitions). Empty until match setup installs a
/// loaded file; with it, every unit and building type has [`CombatTerms`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CombatDefinitions {
    pub(crate) armour_classes: Vec<String>,
    pub(crate) warheads: Vec<WarheadDef>,
    pub(crate) weapons: Vec<WeaponDef>,
    /// Indexed by [`UnitKind`](crate::UnitKind).
    pub(crate) units: Vec<Option<CombatTerms>>,
    /// Indexed by [`BuildingKind`](crate::BuildingKind).
    pub(crate) buildings: Vec<Option<CombatTerms>>,
}

impl CombatDefinitions {
    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        fn text(hasher: &mut Fnv1a64, text: &str) {
            hasher.write_u64(text.len() as u64);
            hasher.write(text.as_bytes());
        }
        fn terms(hasher: &mut Fnv1a64, entry: &Option<CombatTerms>) {
            match entry {
                None => hasher.write(&[0]),
                Some(terms) => {
                    hasher.write(&[1]);
                    hasher.write(&terms.hit_points.to_le_bytes());
                    hasher.write(&terms.armour.0.to_le_bytes());
                    hasher.write_u32(terms.sight);
                    match terms.weapon {
                        None => hasher.write(&[0]),
                        Some(weapon) => {
                            hasher.write(&[1]);
                            hasher.write(&weapon.0.to_le_bytes());
                        }
                    }
                }
            }
        }
        hasher.write_u64(self.armour_classes.len() as u64);
        for name in &self.armour_classes {
            text(hasher, name);
        }
        hasher.write_u64(self.warheads.len() as u64);
        for warhead in &self.warheads {
            text(hasher, &warhead.id);
            hasher.write_u64(warhead.versus.len() as u64);
            for percent in &warhead.versus {
                hasher.write(&percent.to_le_bytes());
            }
        }
        hasher.write_u64(self.weapons.len() as u64);
        for weapon in &self.weapons {
            text(hasher, &weapon.id);
            hasher.write(&weapon.damage.to_le_bytes());
            hasher.write_u32(weapon.range);
            hasher.write_u32(weapon.reload);
            hasher.write(&weapon.warhead.0.to_le_bytes());
            hasher.write_u32(weapon.projectile_speed);
        }
        for list in [&self.units, &self.buildings] {
            hasher.write_u64(list.len() as u64);
            for entry in list {
                terms(hasher, entry);
            }
        }
    }
}

// --- Schema ---------------------------------------------------------------------------------

static VERSUS_SHAPE: Shape = Shape::Struct(&[("armour", Shape::Str), ("percent", Shape::Int)]);

static WARHEAD_SHAPE: Shape = Shape::Struct(&[
    ("id", Shape::Str),
    (
        "versus",
        Shape::List {
            entry: &VERSUS_SHAPE,
            max: MAX_ARMOUR_CLASSES,
        },
    ),
]);

static WEAPON_SHAPE: Shape = Shape::Struct(&[
    ("id", Shape::Str),
    ("damage", Shape::Int),
    ("range", Shape::Int),
    ("reload", Shape::Int),
    ("warhead", Shape::Str),
    ("projectile_speed", Shape::Int),
]);

const WEAPON_LIST_SHAPE: Shape = Shape::List {
    entry: &Shape::Str,
    max: MAX_WEAPONS_PER_TYPE,
};

static UNIT_TERMS_SHAPE: Shape = Shape::Struct(&[
    ("unit", Shape::Str),
    ("hit_points", Shape::Int),
    ("armour", Shape::Str),
    ("sight", Shape::Int),
    ("weapons", WEAPON_LIST_SHAPE),
]);

static BUILDING_TERMS_SHAPE: Shape = Shape::Struct(&[
    ("building", Shape::Str),
    ("hit_points", Shape::Int),
    ("armour", Shape::Str),
    ("sight", Shape::Int),
    ("weapons", WEAPON_LIST_SHAPE),
]);

static COMBAT_FILE_SHAPE: Shape = Shape::Struct(&[
    (
        "schema_version",
        Shape::SchemaVersion(COMBAT_SCHEMA_VERSION),
    ),
    (
        "armour_classes",
        Shape::List {
            entry: &Shape::Str,
            max: MAX_ARMOUR_CLASSES,
        },
    ),
    (
        "warheads",
        Shape::List {
            entry: &WARHEAD_SHAPE,
            max: MAX_WARHEADS,
        },
    ),
    (
        "weapons",
        Shape::List {
            entry: &WEAPON_SHAPE,
            max: MAX_WEAPONS,
        },
    ),
    (
        "units",
        Shape::List {
            entry: &UNIT_TERMS_SHAPE,
            max: MAX_UNIT_TYPES,
        },
    ),
    (
        "buildings",
        Shape::List {
            entry: &BUILDING_TERMS_SHAPE,
            max: MAX_BUILDING_TYPES,
        },
    ),
]);

/// The combat file as `ron` reads it. Integers are read as `i64`, the widest type the checker
/// admits, so that range errors are reported by [`validate`] with their field.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CombatFile {
    schema_version: i64,
    armour_classes: Vec<String>,
    warheads: Vec<WarheadEntry>,
    weapons: Vec<WeaponEntry>,
    units: Vec<UnitTermsEntry>,
    buildings: Vec<BuildingTermsEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WarheadEntry {
    id: String,
    versus: Vec<VersusEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VersusEntry {
    armour: String,
    percent: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WeaponEntry {
    id: String,
    damage: i64,
    range: i64,
    reload: i64,
    warhead: String,
    projectile_speed: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnitTermsEntry {
    unit: String,
    hit_points: i64,
    armour: String,
    sight: i64,
    weapons: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildingTermsEntry {
    building: String,
    hit_points: i64,
    armour: String,
    sight: i64,
    weapons: Vec<String>,
}

/// Loads and validates the combat file at `path` against the unit types `units` and the building
/// types `buildings` (their ids, in order).
pub(crate) fn load(
    path: &Path,
    units: &[String],
    buildings: &[String],
) -> Result<CombatDefinitions, DataError> {
    let bytes = data::read_bounded(path, MAX_DATA_FILE_BYTES).map_err(|kind| DataError {
        file: path.to_owned(),
        field: "file".into(),
        kind,
    })?;
    let text = data::check(path, &bytes, &COMBAT_FILE_SHAPE)?;
    let file: CombatFile = data::parse(path, text)?;
    validate(file, units, buildings).map_err(|(field, kind)| DataError {
        file: path.to_owned(),
        field,
        kind,
    })
}

/// A validation problem: the field and the reason.
type Problem = (String, DataErrorKind);

fn invalid(field: String, found: impl ToString, expected: impl Into<String>) -> Problem {
    (
        field,
        DataErrorKind::InvalidValue {
            found: found.to_string(),
            expected: expected.into(),
        },
    )
}

/// `value` as `T` if it lies in `min..=max`.
fn ranged<T>(field: &str, value: i64, min: T, max: T, unit: &str) -> Result<T, Problem>
where
    T: Copy + TryFrom<i64> + Into<i64> + std::fmt::Display,
{
    T::try_from(value)
        .ok()
        .filter(|&v| (min.into()..=max.into()).contains(&v.into()))
        .ok_or_else(|| invalid(field.to_owned(), value, format!("{min} to {max}{unit}")))
}

/// The index of `id` in `known` as a `u16`, or an unknown reference of `kind` at `field`.
fn reference(field: String, kind: &str, known: &[String], id: &str) -> Result<u16, Problem> {
    known
        .iter()
        .position(|candidate| candidate == id)
        .and_then(|index| u16::try_from(index).ok())
        .ok_or_else(|| {
            (
                field,
                DataErrorKind::UnknownReference {
                    kind: kind.to_owned(),
                    id: id.to_owned(),
                },
            )
        })
}

/// Checks the names in `names` (the `id` fields of a list at `list`) and that they are unique.
fn unique_names<'a>(
    list: &str,
    names: impl Iterator<Item = (String, &'a str)>,
) -> Result<Vec<String>, Problem> {
    let mut seen: Vec<String> = Vec::new();
    for (field, name) in names {
        check_name(name).map_err(|kind| (field.clone(), kind))?;
        if seen.iter().any(|known| known == name) {
            return Err((field, DataErrorKind::DuplicateId(name.to_owned())));
        }
        seen.push(name.to_owned());
    }
    if seen.is_empty() && list == "armour_classes" {
        return Err(invalid(
            list.to_owned(),
            "an empty list",
            format!("1 to {MAX_ARMOUR_CLASSES} armour classes"),
        ));
    }
    Ok(seen)
}

/// Validates one entry of `units` or `buildings`.
fn terms(
    field: &str,
    hit_points: i64,
    armour: &str,
    sight: i64,
    weapons: &[String],
    armour_classes: &[String],
    weapon_ids: &[String],
) -> Result<CombatTerms, Problem> {
    let at = |name: &str| format!("{field}.{name}");
    let hit_points = ranged(&at("hit_points"), hit_points, 1, MAX_HIT_POINTS, "")?;
    let armour = ArmourClass(reference(
        at("armour"),
        "armour class",
        armour_classes,
        armour,
    )?);
    let sight = ranged(&at("sight"), sight, 0, MAX_SIGHT, " cells")?;
    // The schema admits at most `MAX_WEAPONS_PER_TYPE` = 1 entry.
    let weapon = match weapons.first() {
        None => None,
        Some(id) => Some(WeaponKind(reference(
            format!("{field}.weapons[0]"),
            "weapon",
            weapon_ids,
            id,
        )?)),
    };
    Ok(CombatTerms {
        hit_points,
        armour,
        sight,
        weapon,
    })
}

fn validate(
    file: CombatFile,
    units: &[String],
    buildings: &[String],
) -> Result<CombatDefinitions, Problem> {
    if file.schema_version != COMBAT_SCHEMA_VERSION {
        return Err((
            "schema_version".into(),
            DataErrorKind::UnsupportedSchemaVersion {
                found: file.schema_version,
                expected: COMBAT_SCHEMA_VERSION,
            },
        ));
    }
    let armour_classes = unique_names(
        "armour_classes",
        file.armour_classes
            .iter()
            .enumerate()
            .map(|(index, name)| (format!("armour_classes[{index}]"), name.as_str())),
    )?;
    let warhead_ids = unique_names(
        "warheads",
        file.warheads
            .iter()
            .enumerate()
            .map(|(index, entry)| (format!("warheads[{index}].id"), entry.id.as_str())),
    )?;
    let weapon_ids = unique_names(
        "weapons",
        file.weapons
            .iter()
            .enumerate()
            .map(|(index, entry)| (format!("weapons[{index}].id"), entry.id.as_str())),
    )?;

    let mut warheads = Vec::with_capacity(file.warheads.len());
    for (index, entry) in file.warheads.iter().enumerate() {
        let field = format!("warheads[{index}].versus");
        let mut versus: Vec<Option<i32>> = vec![None; armour_classes.len()];
        for (position, pair) in entry.versus.iter().enumerate() {
            let at = |name: &str| format!("{field}[{position}].{name}");
            let class = usize::from(reference(
                at("armour"),
                "armour class",
                &armour_classes,
                &pair.armour,
            )?);
            let percent = ranged(&at("percent"), pair.percent, 0, MAX_VERSUS_PERCENT, " %")?;
            let slot = &mut versus[class];
            if slot.is_some() {
                return Err((
                    at("armour"),
                    DataErrorKind::DuplicateId(pair.armour.clone()),
                ));
            }
            *slot = Some(percent);
        }
        // Every armour class exactly once: no silent default percentage.
        let versus = versus
            .into_iter()
            .zip(&armour_classes)
            .map(|(percent, class)| {
                percent.ok_or_else(|| {
                    (
                        field.clone(),
                        DataErrorKind::MissingEntry {
                            kind: "armour class".into(),
                            id: class.clone(),
                        },
                    )
                })
            })
            .collect::<Result<Vec<i32>, Problem>>()?;
        warheads.push(WarheadDef {
            id: entry.id.clone(),
            versus,
        });
    }

    let mut weapons = Vec::with_capacity(file.weapons.len());
    for (index, entry) in file.weapons.iter().enumerate() {
        let field = format!("weapons[{index}]");
        let at = |name: &str| format!("{field}.{name}");
        let damage = ranged(&at("damage"), entry.damage, 1, MAX_DAMAGE, "")?;
        let range = ranged(
            &at("range"),
            entry.range,
            MIN_WEAPON_RANGE,
            MAX_WEAPON_RANGE,
            " subcells",
        )?;
        let reload = ranged(&at("reload"), entry.reload, 1, MAX_RELOAD, " ticks")?;
        let warhead = WarheadKind(reference(
            at("warhead"),
            "warhead",
            &warhead_ids,
            &entry.warhead,
        )?);
        let projectile_speed = if entry.projectile_speed == 0 {
            0
        } else {
            ranged(
                &at("projectile_speed"),
                entry.projectile_speed,
                MIN_PROJECTILE_SPEED,
                MAX_PROJECTILE_SPEED,
                " subcells per tick, or 0 for an instant weapon",
            )?
        };
        weapons.push(WeaponDef {
            id: entry.id.clone(),
            damage,
            range,
            reload,
            warhead,
            projectile_speed,
        });
    }

    let mut unit_terms: Vec<Option<CombatTerms>> = vec![None; units.len()];
    for (index, entry) in file.units.iter().enumerate() {
        let field = format!("units[{index}]");
        let kind = usize::from(reference(
            format!("{field}.unit"),
            "unit",
            units,
            &entry.unit,
        )?);
        if unit_terms[kind].is_some() {
            return Err((
                format!("{field}.unit"),
                DataErrorKind::DuplicateId(entry.unit.clone()),
            ));
        }
        unit_terms[kind] = Some(terms(
            &field,
            entry.hit_points,
            &entry.armour,
            entry.sight,
            &entry.weapons,
            &armour_classes,
            &weapon_ids,
        )?);
    }
    let mut building_terms: Vec<Option<CombatTerms>> = vec![None; buildings.len()];
    for (index, entry) in file.buildings.iter().enumerate() {
        let field = format!("buildings[{index}]");
        let kind = usize::from(reference(
            format!("{field}.building"),
            "building",
            buildings,
            &entry.building,
        )?);
        if building_terms[kind].is_some() {
            return Err((
                format!("{field}.building"),
                DataErrorKind::DuplicateId(entry.building.clone()),
            ));
        }
        building_terms[kind] = Some(terms(
            &field,
            entry.hit_points,
            &entry.armour,
            entry.sight,
            &entry.weapons,
            &armour_classes,
            &weapon_ids,
        )?);
    }
    // Every unit and building type has combat terms (cross validation, PLAN.md section 5).
    for (list, kind, ids, entries) in [
        ("units", "unit", units, &unit_terms),
        ("buildings", "building", buildings, &building_terms),
    ] {
        if let Some((id, _)) = ids.iter().zip(entries).find(|(_, terms)| terms.is_none()) {
            return Err((
                list.to_owned(),
                DataErrorKind::MissingEntry {
                    kind: kind.to_owned(),
                    id: id.clone(),
                },
            ));
        }
    }

    Ok(CombatDefinitions {
        armour_classes,
        warheads,
        weapons,
        units: unit_terms,
        buildings: building_terms,
    })
}
