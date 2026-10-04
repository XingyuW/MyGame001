//! Immutable economy definitions loaded from RON (PLAN.md sections 5 and 7, M3; docs/DECISIONS.md
//! D23).
//!
//! Economy file, schema version 1:
//!
//! ```ron
//! (
//!     schema_version: 1,
//!     placement_distance: 4,
//!     buildings: [
//!         (
//!             id: "power_plant", sprite: "power_plant", width: 2, height: 2,
//!             cost: 300, build_time: 200, producer: "construction_yard", prerequisites: [],
//!             power_produced: 100, power_consumed: 0, sell_value: 150, free_units: [],
//!         ),
//!     ],
//!     units: [
//!         (unit: "light_tank", cost: 700, build_time: 400, producer: "war_factory",
//!          prerequisites: []),
//!     ],
//!     harvesters: [(unit: "harvester", capacity: 700, harvest_rate: 7)],
//!     refineries: [(building: "refinery", credits_per_ore: 1)],
//! )
//! ```
//!
//! Every field is required, so there are no silent defaults. Building ids and sprite names follow
//! the rules of unit ids (`definitions::check_name`). Every reference (a producer, a prerequisite,
//! a free unit, a unit with production costs, a harvester, a refinery) must name a building of
//! this file or a unit of the units file, and the prerequisites of the buildings must be acyclic
//! (PLAN.md section 5). Ranges are checked on the parsed values; the limits below keep every
//! quantity the simulation derives from them inside its integer types (invariant I12).

use std::path::Path;

use serde::Deserialize;

use crate::data::{self, DataError, DataErrorKind, MAX_DATA_FILE_BYTES, Shape};
use crate::definitions::{MAX_UNIT_TYPES, UnitKind, check_name};
use crate::hash::Fnv1a64;

/// The schema version of the economy file this crate reads.
pub const ECONOMY_SCHEMA_VERSION: i64 = 1;

/// Largest number of building types in an economy file.
pub const MAX_BUILDING_TYPES: usize = 64;

/// Largest width and height of a building's footprint, in cells.
pub const MAX_FOOTPRINT_SIDE: u8 = 8;

/// Largest cost of a building or unit, in credits.
pub const MAX_COST: i64 = 1_000_000;

/// Longest build time of a building or unit, in ticks (one hour at 20 ticks per second).
pub const MAX_BUILD_TIME: u32 = 72_000;

/// Largest power a building produces or consumes. Every building covers at least one cell of a
/// map of at most 256 × 256 cells, so a player's total stays below 65,536 × 10,000, far below
/// `i32::MAX`.
pub const MAX_POWER: i32 = 10_000;

/// Largest number of prerequisites of one building or unit.
pub const MAX_PREREQUISITES: usize = 8;

/// Largest number of free units a building delivers when it is placed.
pub const MAX_FREE_UNITS: usize = 8;

/// Largest placement distance in cells.
pub const MAX_PLACEMENT_DISTANCE: u32 = 32;

/// Largest ore load of a harvester.
pub const MAX_ORE_CAPACITY: u32 = 100_000;

/// Largest number of credits a refinery pays per ore unit. A full load of the largest capacity is
/// worth at most 10^8 credits.
pub const MAX_CREDITS_PER_ORE: i64 = 1_000;

/// Index of a building type in [`Definitions`](crate::Definitions).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BuildingKind(pub u16);

/// Something a player can produce: a building (placed with `PlaceBuilding` when complete) or a
/// unit (which leaves a producing building when complete).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProductionItem {
    Building(BuildingKind),
    Unit(UnitKind),
}

/// The production terms of an item: equation 3 of PLAN.md section 5 charges `cost / build_time`
/// credits on each progressing tick and the remainder on the last.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Production {
    /// Credits, from 0 to [`MAX_COST`].
    pub cost: i64,
    /// Ticks of progress, from 1 to [`MAX_BUILD_TIME`].
    pub build_time: u32,
    /// The building type that produces the item; the player must own one for it to progress.
    pub producer: BuildingKind,
    /// Building types the player must own to queue the item.
    pub prerequisites: Vec<BuildingKind>,
}

/// A building type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildingDef {
    pub id: String,
    /// Name of the building's sprite in the game.
    pub sprite: String,
    /// Footprint width (along x) in cells, 1 to [`MAX_FOOTPRINT_SIDE`].
    pub width: u8,
    /// Footprint height (along y) in cells, 1 to [`MAX_FOOTPRINT_SIDE`].
    pub height: u8,
    pub production: Production,
    /// 0 to [`MAX_POWER`].
    pub power_produced: i32,
    /// 0 to [`MAX_POWER`].
    pub power_consumed: i32,
    /// Credits refunded when the building is sold, from 0 to its cost.
    pub sell_value: i64,
    /// Units that leave the building when it is placed (for example a refinery's harvester).
    pub free_units: Vec<UnitKind>,
    /// Credits per ore unit if the building is a refinery.
    pub credits_per_ore: Option<i64>,
}

/// The harvesting terms of a harvester unit type (PLAN.md section 5, equation 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HarvesterDef {
    /// Ore units the harvester carries when full, 1 to [`MAX_ORE_CAPACITY`].
    pub capacity: u32,
    /// Ore units it removes from its cell per tick, 1 to `capacity`.
    pub harvest_rate: u32,
}

/// The economy part of [`Definitions`](crate::Definitions). Empty until match setup installs a
/// loaded file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EconomyDefinitions {
    pub(crate) placement_distance: u32,
    pub(crate) buildings: Vec<BuildingDef>,
    /// Indexed by [`UnitKind`]; `None` for units that cannot be produced.
    pub(crate) unit_production: Vec<Option<Production>>,
    /// Indexed by [`UnitKind`]; `None` for units that do not harvest.
    pub(crate) harvesters: Vec<Option<HarvesterDef>>,
}

impl EconomyDefinitions {
    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        fn production(hasher: &mut Fnv1a64, production: &Production) {
            hasher.write(&production.cost.to_le_bytes());
            hasher.write_u32(production.build_time);
            hasher.write(&production.producer.0.to_le_bytes());
            hasher.write_u64(production.prerequisites.len() as u64);
            for kind in &production.prerequisites {
                hasher.write(&kind.0.to_le_bytes());
            }
        }
        hasher.write_u32(self.placement_distance);
        hasher.write_u64(self.buildings.len() as u64);
        for building in &self.buildings {
            for text in [&building.id, &building.sprite] {
                hasher.write_u64(text.len() as u64);
                hasher.write(text.as_bytes());
            }
            hasher.write(&[building.width, building.height]);
            production(hasher, &building.production);
            hasher.write(&building.power_produced.to_le_bytes());
            hasher.write(&building.power_consumed.to_le_bytes());
            hasher.write(&building.sell_value.to_le_bytes());
            hasher.write_u64(building.free_units.len() as u64);
            for kind in &building.free_units {
                hasher.write(&kind.0.to_le_bytes());
            }
            match building.credits_per_ore {
                None => hasher.write(&[0]),
                Some(rate) => {
                    hasher.write(&[1]);
                    hasher.write(&rate.to_le_bytes());
                }
            }
        }
        hasher.write_u64(self.unit_production.len() as u64);
        for entry in &self.unit_production {
            match entry {
                None => hasher.write(&[0]),
                Some(entry) => {
                    hasher.write(&[1]);
                    production(hasher, entry);
                }
            }
        }
        hasher.write_u64(self.harvesters.len() as u64);
        for entry in &self.harvesters {
            match entry {
                None => hasher.write(&[0]),
                Some(entry) => {
                    hasher.write(&[1]);
                    hasher.write_u32(entry.capacity);
                    hasher.write_u32(entry.harvest_rate);
                }
            }
        }
    }
}

// --- Schema ---------------------------------------------------------------------------------

static BUILDING_SHAPE: Shape = Shape::Struct(&[
    ("id", Shape::Str),
    ("sprite", Shape::Str),
    ("width", Shape::Int),
    ("height", Shape::Int),
    ("cost", Shape::Int),
    ("build_time", Shape::Int),
    ("producer", Shape::Str),
    (
        "prerequisites",
        Shape::List {
            entry: &Shape::Str,
            max: MAX_PREREQUISITES,
        },
    ),
    ("power_produced", Shape::Int),
    ("power_consumed", Shape::Int),
    ("sell_value", Shape::Int),
    (
        "free_units",
        Shape::List {
            entry: &Shape::Str,
            max: MAX_FREE_UNITS,
        },
    ),
]);

static UNIT_PRODUCTION_SHAPE: Shape = Shape::Struct(&[
    ("unit", Shape::Str),
    ("cost", Shape::Int),
    ("build_time", Shape::Int),
    ("producer", Shape::Str),
    (
        "prerequisites",
        Shape::List {
            entry: &Shape::Str,
            max: MAX_PREREQUISITES,
        },
    ),
]);

static HARVESTER_SHAPE: Shape = Shape::Struct(&[
    ("unit", Shape::Str),
    ("capacity", Shape::Int),
    ("harvest_rate", Shape::Int),
]);

static REFINERY_SHAPE: Shape =
    Shape::Struct(&[("building", Shape::Str), ("credits_per_ore", Shape::Int)]);

static ECONOMY_FILE_SHAPE: Shape = Shape::Struct(&[
    (
        "schema_version",
        Shape::SchemaVersion(ECONOMY_SCHEMA_VERSION),
    ),
    ("placement_distance", Shape::Int),
    (
        "buildings",
        Shape::List {
            entry: &BUILDING_SHAPE,
            max: MAX_BUILDING_TYPES,
        },
    ),
    (
        "units",
        Shape::List {
            entry: &UNIT_PRODUCTION_SHAPE,
            max: MAX_UNIT_TYPES,
        },
    ),
    (
        "harvesters",
        Shape::List {
            entry: &HARVESTER_SHAPE,
            max: MAX_UNIT_TYPES,
        },
    ),
    (
        "refineries",
        Shape::List {
            entry: &REFINERY_SHAPE,
            max: MAX_BUILDING_TYPES,
        },
    ),
]);

/// The economy file as `ron` reads it. Integers are read as `i64`, the widest type the checker
/// admits, so that range errors are reported by [`validate`] with their field.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EconomyFile {
    schema_version: i64,
    placement_distance: i64,
    buildings: Vec<BuildingEntry>,
    units: Vec<UnitProductionEntry>,
    harvesters: Vec<HarvesterEntry>,
    refineries: Vec<RefineryEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildingEntry {
    id: String,
    sprite: String,
    width: i64,
    height: i64,
    cost: i64,
    build_time: i64,
    producer: String,
    prerequisites: Vec<String>,
    power_produced: i64,
    power_consumed: i64,
    sell_value: i64,
    free_units: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnitProductionEntry {
    unit: String,
    cost: i64,
    build_time: i64,
    producer: String,
    prerequisites: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HarvesterEntry {
    unit: String,
    capacity: i64,
    harvest_rate: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RefineryEntry {
    building: String,
    credits_per_ore: i64,
}

/// Loads and validates the economy file at `path` against the unit types `units` (the ids of the
/// units file, in order).
pub(crate) fn load(path: &Path, units: &[String]) -> Result<EconomyDefinitions, DataError> {
    let bytes = data::read_bounded(path, MAX_DATA_FILE_BYTES).map_err(|kind| DataError {
        file: path.to_owned(),
        field: "file".into(),
        kind,
    })?;
    let text = data::check(path, &bytes, &ECONOMY_FILE_SHAPE)?;
    let file: EconomyFile = data::parse(path, text)?;
    validate(path, file, units)
}

/// A validation problem: the field and the reason.
pub(crate) type Problem = (String, DataErrorKind);

pub(crate) fn invalid(field: String, found: impl ToString, expected: impl Into<String>) -> Problem {
    (
        field,
        DataErrorKind::InvalidValue {
            found: found.to_string(),
            expected: expected.into(),
        },
    )
}

/// `value` as `T` if it lies in `min..=max`.
pub(crate) fn ranged<T>(field: &str, value: i64, min: T, max: T, unit: &str) -> Result<T, Problem>
where
    T: Copy + TryFrom<i64> + Into<i64> + std::fmt::Display,
{
    T::try_from(value)
        .ok()
        .filter(|&v| (min.into()..=max.into()).contains(&v.into()))
        .ok_or_else(|| invalid(field.to_owned(), value, format!("{min} to {max}{unit}")))
}

pub(crate) fn unknown(field: String, kind: &str, id: &str) -> Problem {
    (
        field,
        DataErrorKind::UnknownReference {
            kind: kind.to_owned(),
            id: id.to_owned(),
        },
    )
}

fn validate(
    path: &Path,
    file: EconomyFile,
    units: &[String],
) -> Result<EconomyDefinitions, DataError> {
    validate_parsed(file, units).map_err(|(field, kind)| DataError {
        file: path.to_owned(),
        field,
        kind,
    })
}

fn validate_parsed(file: EconomyFile, units: &[String]) -> Result<EconomyDefinitions, Problem> {
    if file.schema_version != ECONOMY_SCHEMA_VERSION {
        return Err((
            "schema_version".into(),
            DataErrorKind::UnsupportedSchemaVersion {
                found: file.schema_version,
                expected: ECONOMY_SCHEMA_VERSION,
            },
        ));
    }
    let placement_distance = ranged(
        "placement_distance",
        file.placement_distance,
        1,
        MAX_PLACEMENT_DISTANCE,
        " cells",
    )?;
    if file.buildings.is_empty() {
        return Err(invalid(
            "buildings".into(),
            "an empty list",
            format!("1 to {MAX_BUILDING_TYPES} building types"),
        ));
    }

    // Building ids first, so that references can be resolved in any order.
    let mut ids: Vec<&str> = Vec::with_capacity(file.buildings.len());
    for (index, entry) in file.buildings.iter().enumerate() {
        let field = format!("buildings[{index}].id");
        check_name(&entry.id).map_err(|kind| (field.clone(), kind))?;
        if ids.contains(&entry.id.as_str()) {
            return Err((field, DataErrorKind::DuplicateId(entry.id.clone())));
        }
        ids.push(&entry.id);
    }
    let building = |field: String, id: &str| -> Result<BuildingKind, Problem> {
        ids.iter()
            .position(|known| *known == id)
            .and_then(|index| u16::try_from(index).ok())
            .map(BuildingKind)
            .ok_or_else(|| unknown(field, "building", id))
    };
    let unit = |field: String, id: &str| -> Result<UnitKind, Problem> {
        units
            .iter()
            .position(|known| known == id)
            .and_then(|index| u16::try_from(index).ok())
            .map(UnitKind)
            .ok_or_else(|| unknown(field, "unit", id))
    };
    let prerequisites = |field: &str, names: &[String]| -> Result<Vec<BuildingKind>, Problem> {
        let mut kinds: Vec<BuildingKind> = Vec::with_capacity(names.len());
        for (index, name) in names.iter().enumerate() {
            let field = format!("{field}[{index}]");
            let kind = building(field.clone(), name)?;
            if kinds.contains(&kind) {
                return Err((field, DataErrorKind::DuplicateId(name.clone())));
            }
            kinds.push(kind);
        }
        Ok(kinds)
    };
    let production = |field: &str,
                      cost: i64,
                      build_time: i64,
                      producer: &str,
                      names: &[String]|
     -> Result<Production, Problem> {
        Ok(Production {
            cost: ranged(&format!("{field}.cost"), cost, 0, MAX_COST, " credits")?,
            build_time: ranged(
                &format!("{field}.build_time"),
                build_time,
                1,
                MAX_BUILD_TIME,
                " ticks",
            )?,
            producer: building(format!("{field}.producer"), producer)?,
            prerequisites: prerequisites(&format!("{field}.prerequisites"), names)?,
        })
    };

    let mut buildings: Vec<BuildingDef> = Vec::with_capacity(file.buildings.len());
    for (index, entry) in file.buildings.iter().enumerate() {
        let field = format!("buildings[{index}]");
        let at = |name: &str| format!("{field}.{name}");
        check_name(&entry.sprite).map_err(|kind| (at("sprite"), kind))?;
        let width = ranged(&at("width"), entry.width, 1, MAX_FOOTPRINT_SIDE, " cells")?;
        let height = ranged(&at("height"), entry.height, 1, MAX_FOOTPRINT_SIDE, " cells")?;
        let production = production(
            &field,
            entry.cost,
            entry.build_time,
            &entry.producer,
            &entry.prerequisites,
        )?;
        let power_produced = ranged(
            &at("power_produced"),
            entry.power_produced,
            0,
            MAX_POWER,
            "",
        )?;
        let power_consumed = ranged(
            &at("power_consumed"),
            entry.power_consumed,
            0,
            MAX_POWER,
            "",
        )?;
        let sell_value = ranged(
            &at("sell_value"),
            entry.sell_value,
            0,
            production.cost,
            " credits (at most the cost)",
        )?;
        let free_units = entry
            .free_units
            .iter()
            .enumerate()
            .map(|(unit_index, name)| unit(format!("{field}.free_units[{unit_index}]"), name))
            .collect::<Result<Vec<_>, _>>()?;
        buildings.push(BuildingDef {
            id: entry.id.clone(),
            sprite: entry.sprite.clone(),
            width,
            height,
            production,
            power_produced,
            power_consumed,
            sell_value,
            free_units,
            credits_per_ore: None,
        });
    }
    check_acyclic(&buildings)?;

    let mut unit_production: Vec<Option<Production>> = vec![None; units.len()];
    for (index, entry) in file.units.iter().enumerate() {
        let field = format!("units[{index}]");
        let kind = unit(format!("{field}.unit"), &entry.unit)?;
        let slot = &mut unit_production[usize::from(kind.0)];
        if slot.is_some() {
            return Err((
                format!("{field}.unit"),
                DataErrorKind::DuplicateId(entry.unit.clone()),
            ));
        }
        *slot = Some(production(
            &field,
            entry.cost,
            entry.build_time,
            &entry.producer,
            &entry.prerequisites,
        )?);
    }

    let mut harvesters: Vec<Option<HarvesterDef>> = vec![None; units.len()];
    for (index, entry) in file.harvesters.iter().enumerate() {
        let field = format!("harvesters[{index}]");
        let kind = unit(format!("{field}.unit"), &entry.unit)?;
        if harvesters[usize::from(kind.0)].is_some() {
            return Err((
                format!("{field}.unit"),
                DataErrorKind::DuplicateId(entry.unit.clone()),
            ));
        }
        let capacity = ranged(
            &format!("{field}.capacity"),
            entry.capacity,
            1,
            MAX_ORE_CAPACITY,
            " ore units",
        )?;
        let harvest_rate = ranged(
            &format!("{field}.harvest_rate"),
            entry.harvest_rate,
            1,
            capacity,
            " ore units per tick (at most the capacity)",
        )?;
        harvesters[usize::from(kind.0)] = Some(HarvesterDef {
            capacity,
            harvest_rate,
        });
    }

    for (index, entry) in file.refineries.iter().enumerate() {
        let field = format!("refineries[{index}]");
        let kind = building(format!("{field}.building"), &entry.building)?;
        let rate = ranged(
            &format!("{field}.credits_per_ore"),
            entry.credits_per_ore,
            1,
            MAX_CREDITS_PER_ORE,
            " credits per ore unit",
        )?;
        let slot = &mut buildings[usize::from(kind.0)].credits_per_ore;
        if slot.is_some() {
            return Err((
                format!("{field}.building"),
                DataErrorKind::DuplicateId(entry.building.clone()),
            ));
        }
        *slot = Some(rate);
    }

    Ok(EconomyDefinitions {
        placement_distance,
        buildings,
        unit_production,
        harvesters,
    })
}

/// The prerequisites of the buildings must be acyclic (PLAN.md section 5). A cycle is reported
/// at the prerequisite list of the building that closes it, naming the cycle.
fn check_acyclic(buildings: &[BuildingDef]) -> Result<(), Problem> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        New,
        Open,
        Done,
    }
    let mut marks = vec![Mark::New; buildings.len()];
    for start in 0..buildings.len() {
        if marks[start] != Mark::New {
            continue;
        }
        // Iterative depth-first search: the path, and the next prerequisite to visit of each
        // building on it. At most `MAX_BUILDING_TYPES` deep, without recursion.
        let mut path: Vec<(usize, usize)> = vec![(start, 0)];
        marks[start] = Mark::Open;
        while let Some(top) = path.last_mut() {
            let (node, next) = *top;
            let Some(prerequisite) = buildings[node].production.prerequisites.get(next) else {
                marks[node] = Mark::Done;
                path.pop();
                continue;
            };
            top.1 += 1;
            let target = usize::from(prerequisite.0);
            match marks[target] {
                Mark::Done => {}
                Mark::New => {
                    marks[target] = Mark::Open;
                    path.push((target, 0));
                }
                Mark::Open => {
                    let from = path
                        .iter()
                        .position(|&(member, _)| member == target)
                        .unwrap_or(0);
                    let mut cycle: Vec<String> = path[from..]
                        .iter()
                        .map(|&(member, _)| buildings[member].id.clone())
                        .collect();
                    cycle.push(buildings[target].id.clone());
                    return Err((
                        format!("buildings[{node}].prerequisites"),
                        DataErrorKind::PrerequisiteCycle(cycle),
                    ));
                }
            }
        }
    }
    Ok(())
}
