//! Skirmish definitions loaded from RON: the starting credits, the factions with their starting
//! buildings and units, and the maps with their start positions (PLAN.md section 7, M6;
//! docs/DECISIONS.md D40).
//!
//! Skirmish file, schema version 2 (version 1, M6, had no rosters):
//!
//! ```ron
//! (
//!     schema_version: 2,
//!     starting_credits: 5000,
//!     factions: [
//!         (
//!             id: "vanguard",
//!             roster_buildings: ["power_plant", "refinery", "barracks"],
//!             roster_units: ["light_tank", "harvester"],
//!             buildings: [(building: "construction_yard", x: 0, y: 0)],
//!             units: [(unit: "light_tank", x: 4, y: 1, count: 4)],
//!         ),
//!     ],
//!     maps: [(id: "demo", starts: [(x: 15, y: 37), (x: 52, y: 42)])],
//! )
//! ```
//!
//! Every field is required, so there are no silent defaults. Faction and map ids follow the rules
//! of unit ids (`definitions::check_name`); a map id `m` names the map file `maps/m/m.tmx` under
//! the asset directory ([`map_asset_path`]). The buildings and units of a faction are placed
//! relative to the start position of the player who plays it: a building's footprint origin is the
//! start plus its offset, and a group of units stands on the free cells nearest to the start plus
//! its offset. Every map has exactly [`MATCH_PLAYERS`] start positions.
//!
//! A faction's roster (M7, PLAN.md section 7: "the second faction, the full roster"; docs/
//! DECISIONS.md D48) lists the building and unit types a player of the faction may produce:
//! `QueueProduction` of anything else is rejected (`RejectReason::NotInRoster`). Every entry is a
//! defined type with production terms, listed once. Starting buildings and units and the free units
//! of a placed building (a refinery's harvester) need not be in the roster.
//!
//! The file names its maps but does not read them, so the checks against a map are separate: every
//! loader of a map checks the map's start positions against its grid with [`check_map_starts`]
//! (each start is a cell of the map with passable terrain; M6 review 1 P1-1), and
//! `crate::match_setup` checks them again, together with whether a faction's buildings and units
//! fit at a start, with a typed error.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::data::{self, DataError, DataErrorKind, MAX_DATA_FILE_BYTES, Shape};
use crate::definitions::{Definitions, UnitKind, check_name};
use crate::economy_defs::{
    BuildingKind, MAX_COST, Problem, ProductionItem, invalid, ranged, unknown,
};
use crate::map::{CellPos, Grid, MAX_MAP_SIDE};

/// The schema version of the skirmish file this crate reads (2 since M7: faction rosters).
pub const SKIRMISH_SCHEMA_VERSION: i64 = 2;

/// Players of a skirmish match, and start positions of every map.
pub const MATCH_PLAYERS: usize = 2;

/// Largest number of factions.
pub const MAX_FACTIONS: usize = 8;

/// Largest number of maps.
pub const MAX_MAPS: usize = 16;

/// Largest number of starting buildings of a faction.
pub const MAX_STARTING_BUILDINGS: usize = 16;

/// Largest number of groups of starting units of a faction.
pub const MAX_STARTING_GROUPS: usize = 16;

/// Largest number of units in one group of starting units.
pub const MAX_GROUP_UNITS: u32 = 64;

/// Largest number of building types and of unit types in a faction's roster.
pub const MAX_ROSTER_ENTRIES: usize = 64;

/// Largest offset of a starting building or group from the start position, along each axis.
pub const MAX_START_OFFSET: i32 = 32;

/// Largest starting credits.
pub const MAX_STARTING_CREDITS: i64 = MAX_COST;

/// A starting building of a faction: its type and the offset of its footprint's origin from the
/// start position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartingBuilding {
    pub kind: BuildingKind,
    pub offset: (i32, i32),
}

/// A group of starting units of a faction: `count` units of type `kind` on the free cells nearest
/// to the start position plus `offset`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartingGroup {
    pub kind: UnitKind,
    pub offset: (i32, i32),
    pub count: u32,
}

/// A faction: what a player who plays it starts with, and what it may produce.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FactionDef {
    pub id: String,
    /// The building and unit types a player of the faction may produce (each with production
    /// terms).
    pub roster: BTreeSet<ProductionItem>,
    /// At least one building, so that a player does not start defeated.
    pub buildings: Vec<StartingBuilding>,
    pub units: Vec<StartingGroup>,
}

impl FactionDef {
    /// Whether a player of the faction may produce `item`.
    pub fn offers(&self, item: ProductionItem) -> bool {
        self.roster.contains(&item)
    }
}

/// A map of the skirmish: its id and its start positions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapDef {
    pub id: String,
    pub starts: [CellPos; MATCH_PLAYERS],
}

/// The skirmish definitions (the skirmish file).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkirmishDefinitions {
    /// Credits of every player at the start, from 0 to [`MAX_STARTING_CREDITS`].
    pub starting_credits: i64,
    pub factions: Vec<FactionDef>,
    pub maps: Vec<MapDef>,
}

/// The path of the map file of map `id`, relative to the asset directory: `maps/<id>/<id>.tmx`.
pub fn map_asset_path(id: &str) -> PathBuf {
    Path::new("maps").join(id).join(format!("{id}.tmx"))
}

/// Why a start position cannot be used on its map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartProblem {
    /// The cell lies outside the map, which has `width` × `height` cells.
    OutsideMap { width: u32, height: u32 },
    /// The cell's terrain is not passable.
    Impassable,
}

impl fmt::Display for StartProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutsideMap { width, height } => {
                write!(f, "outside the {width} x {height} cells")
            }
            Self::Impassable => write!(f, "impassable terrain"),
        }
    }
}

/// Why `cell` cannot be a start position on the map of `grid`, or `None` if it can: a start
/// position is a cell of the map whose terrain is passable (its own buildings may cover it).
pub fn start_problem(grid: &Grid, cell: CellPos) -> Option<StartProblem> {
    match grid.cell(cell) {
        None => Some(StartProblem::OutsideMap {
            width: grid.width(),
            height: grid.height(),
        }),
        Some(terrain) if !terrain.passable => Some(StartProblem::Impassable),
        Some(_) => None,
    }
}

/// Checks the start positions of map `index` of the skirmish file at `path` against `grid`, the
/// grid of that map (cross validation of the skirmish file with its maps; M6 review 1 P1-1): every
/// start position is a cell of the map with passable terrain. The error names the file, the field
/// (`maps[i].starts[j]`), the cell, the map, and the reason.
pub fn check_map_starts(
    path: impl AsRef<Path>,
    skirmish: &SkirmishDefinitions,
    index: usize,
    grid: &Grid,
) -> Result<(), DataError> {
    let error = |field: String, kind: DataErrorKind| DataError {
        file: path.as_ref().to_owned(),
        field,
        kind,
    };
    let map = skirmish.maps.get(index).ok_or_else(|| {
        error(
            "maps".into(),
            DataErrorKind::MissingEntry {
                kind: "map".into(),
                id: index.to_string(),
            },
        )
    })?;
    for (position, &cell) in map.starts.iter().enumerate() {
        if let Some(problem) = start_problem(grid, cell) {
            let (field, kind) = invalid(
                format!("maps[{index}].starts[{position}]"),
                format!(
                    "({}, {}), which is {problem} of map {:?}",
                    cell.x, cell.y, map.id
                ),
                "a cell of the map with passable terrain",
            );
            return Err(error(field, kind));
        }
    }
    Ok(())
}

// --- Schema ---------------------------------------------------------------------------------

static BUILDING_SHAPE: Shape = Shape::Struct(&[
    ("building", Shape::Str),
    ("x", Shape::Int),
    ("y", Shape::Int),
]);

static GROUP_SHAPE: Shape = Shape::Struct(&[
    ("unit", Shape::Str),
    ("x", Shape::Int),
    ("y", Shape::Int),
    ("count", Shape::Int),
]);

static FACTION_SHAPE: Shape = Shape::Struct(&[
    ("id", Shape::Str),
    (
        "roster_buildings",
        Shape::List {
            entry: &Shape::Str,
            max: MAX_ROSTER_ENTRIES,
        },
    ),
    (
        "roster_units",
        Shape::List {
            entry: &Shape::Str,
            max: MAX_ROSTER_ENTRIES,
        },
    ),
    (
        "buildings",
        Shape::List {
            entry: &BUILDING_SHAPE,
            max: MAX_STARTING_BUILDINGS,
        },
    ),
    (
        "units",
        Shape::List {
            entry: &GROUP_SHAPE,
            max: MAX_STARTING_GROUPS,
        },
    ),
]);

static START_SHAPE: Shape = Shape::Struct(&[("x", Shape::Int), ("y", Shape::Int)]);

static MAP_SHAPE: Shape = Shape::Struct(&[
    ("id", Shape::Str),
    (
        "starts",
        Shape::List {
            entry: &START_SHAPE,
            max: MATCH_PLAYERS,
        },
    ),
]);

static SKIRMISH_FILE_SHAPE: Shape = Shape::Struct(&[
    (
        "schema_version",
        Shape::SchemaVersion(SKIRMISH_SCHEMA_VERSION),
    ),
    ("starting_credits", Shape::Int),
    (
        "factions",
        Shape::List {
            entry: &FACTION_SHAPE,
            max: MAX_FACTIONS,
        },
    ),
    (
        "maps",
        Shape::List {
            entry: &MAP_SHAPE,
            max: MAX_MAPS,
        },
    ),
]);

/// The skirmish file as `ron` reads it. Integers are read as `i64`, the widest type the checker
/// admits, so that range errors are reported with their field.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SkirmishFile {
    schema_version: i64,
    starting_credits: i64,
    factions: Vec<FactionEntry>,
    maps: Vec<MapEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FactionEntry {
    id: String,
    roster_buildings: Vec<String>,
    roster_units: Vec<String>,
    buildings: Vec<BuildingEntry>,
    units: Vec<GroupEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildingEntry {
    building: String,
    x: i64,
    y: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupEntry {
    unit: String,
    x: i64,
    y: i64,
    count: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapEntry {
    id: String,
    starts: Vec<StartEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StartEntry {
    x: i64,
    y: i64,
}

/// Loads and validates the skirmish file at `path` against `definitions`, whose building and
/// unit types the factions name. Every problem is a [`DataError`] naming the file, the field, and
/// the reason.
pub fn load_skirmish(
    path: impl AsRef<Path>,
    definitions: &Definitions,
) -> Result<SkirmishDefinitions, DataError> {
    let path = path.as_ref();
    let bytes = data::read_bounded(path, MAX_DATA_FILE_BYTES).map_err(|kind| DataError {
        file: path.to_owned(),
        field: "file".into(),
        kind,
    })?;
    let text = data::check(path, &bytes, &SKIRMISH_FILE_SHAPE)?;
    let file: SkirmishFile = data::parse(path, text)?;
    validate(file, definitions).map_err(|(field, kind)| DataError {
        file: path.to_owned(),
        field,
        kind,
    })
}

fn offset(field: &str, x: i64, y: i64) -> Result<(i32, i32), Problem> {
    Ok((
        ranged(
            &format!("{field}.x"),
            x,
            -MAX_START_OFFSET,
            MAX_START_OFFSET,
            " cells",
        )?,
        ranged(
            &format!("{field}.y"),
            y,
            -MAX_START_OFFSET,
            MAX_START_OFFSET,
            " cells",
        )?,
    ))
}

/// The roster of the faction at `field`: every entry names a defined type with production terms,
/// once.
fn roster(
    definitions: &Definitions,
    field: &str,
    buildings: &[String],
    units: &[String],
) -> Result<BTreeSet<ProductionItem>, Problem> {
    let mut roster = BTreeSet::new();
    for (position, id) in buildings.iter().enumerate() {
        let at = format!("{field}.roster_buildings[{position}]");
        let kind = definitions
            .building_kind(id)
            .ok_or_else(|| unknown(at.clone(), "building", id))?;
        let item = ProductionItem::Building(kind);
        if definitions.production(item).is_none() {
            return Err(invalid(
                at,
                format!("{id:?}"),
                "a building type with production terms",
            ));
        }
        if !roster.insert(item) {
            return Err((at, DataErrorKind::DuplicateId(id.clone())));
        }
    }
    for (position, id) in units.iter().enumerate() {
        let at = format!("{field}.roster_units[{position}]");
        let kind = definitions
            .unit_kind(id)
            .ok_or_else(|| unknown(at.clone(), "unit", id))?;
        let item = ProductionItem::Unit(kind);
        if definitions.production(item).is_none() {
            return Err(invalid(
                at,
                format!("{id:?}"),
                "a unit type with production terms",
            ));
        }
        if !roster.insert(item) {
            return Err((at, DataErrorKind::DuplicateId(id.clone())));
        }
    }
    Ok(roster)
}

fn validate(file: SkirmishFile, definitions: &Definitions) -> Result<SkirmishDefinitions, Problem> {
    if file.schema_version != SKIRMISH_SCHEMA_VERSION {
        return Err((
            "schema_version".into(),
            DataErrorKind::UnsupportedSchemaVersion {
                found: file.schema_version,
                expected: SKIRMISH_SCHEMA_VERSION,
            },
        ));
    }
    let starting_credits = ranged(
        "starting_credits",
        file.starting_credits,
        0,
        MAX_STARTING_CREDITS,
        " credits",
    )?;
    if file.factions.is_empty() {
        return Err(invalid(
            "factions".into(),
            "an empty list",
            format!("1 to {MAX_FACTIONS} factions"),
        ));
    }
    let mut factions: Vec<FactionDef> = Vec::with_capacity(file.factions.len());
    for (index, entry) in file.factions.into_iter().enumerate() {
        let field = format!("factions[{index}]");
        check_name(&entry.id).map_err(|kind| (format!("{field}.id"), kind))?;
        if factions.iter().any(|faction| faction.id == entry.id) {
            return Err((format!("{field}.id"), DataErrorKind::DuplicateId(entry.id)));
        }
        let roster = roster(
            definitions,
            &field,
            &entry.roster_buildings,
            &entry.roster_units,
        )?;
        if entry.buildings.is_empty() {
            // A player without buildings is defeated at once (docs/DECISIONS.md D41).
            return Err(invalid(
                format!("{field}.buildings"),
                "an empty list",
                format!("1 to {MAX_STARTING_BUILDINGS} starting buildings"),
            ));
        }
        let mut buildings = Vec::with_capacity(entry.buildings.len());
        for (position, building) in entry.buildings.iter().enumerate() {
            let at = format!("{field}.buildings[{position}]");
            let kind = definitions
                .building_kind(&building.building)
                .ok_or_else(|| unknown(format!("{at}.building"), "building", &building.building))?;
            buildings.push(StartingBuilding {
                kind,
                offset: offset(&at, building.x, building.y)?,
            });
        }
        let mut units = Vec::with_capacity(entry.units.len());
        for (position, group) in entry.units.iter().enumerate() {
            let at = format!("{field}.units[{position}]");
            let kind = definitions
                .unit_kind(&group.unit)
                .ok_or_else(|| unknown(format!("{at}.unit"), "unit", &group.unit))?;
            units.push(StartingGroup {
                kind,
                offset: offset(&at, group.x, group.y)?,
                count: ranged(
                    &format!("{at}.count"),
                    group.count,
                    1,
                    MAX_GROUP_UNITS,
                    " units",
                )?,
            });
        }
        factions.push(FactionDef {
            id: entry.id,
            roster,
            buildings,
            units,
        });
    }
    if file.maps.is_empty() {
        return Err(invalid(
            "maps".into(),
            "an empty list",
            format!("1 to {MAX_MAPS} maps"),
        ));
    }
    let largest = i64::from(MAX_MAP_SIDE) - 1;
    let mut maps: Vec<MapDef> = Vec::with_capacity(file.maps.len());
    for (index, entry) in file.maps.into_iter().enumerate() {
        let field = format!("maps[{index}]");
        check_name(&entry.id).map_err(|kind| (format!("{field}.id"), kind))?;
        if maps.iter().any(|map| map.id == entry.id) {
            return Err((format!("{field}.id"), DataErrorKind::DuplicateId(entry.id)));
        }
        if entry.starts.len() != MATCH_PLAYERS {
            return Err(invalid(
                format!("{field}.starts"),
                format!("{} start positions", entry.starts.len()),
                format!("exactly {MATCH_PLAYERS} start positions"),
            ));
        }
        let mut starts = [CellPos::new(0, 0); MATCH_PLAYERS];
        for (position, start) in entry.starts.iter().enumerate() {
            let at = format!("{field}.starts[{position}]");
            let x: i32 = ranged(&format!("{at}.x"), start.x, 0, largest as i32, " (a cell)")?;
            let y: i32 = ranged(&format!("{at}.y"), start.y, 0, largest as i32, " (a cell)")?;
            let cell = CellPos::new(x, y);
            if starts[..position].contains(&cell) {
                return Err(invalid(
                    at,
                    format!("({x}, {y}), the cell of an earlier start"),
                    "a cell no other start position of the map has",
                ));
            }
            starts[position] = cell;
        }
        maps.push(MapDef {
            id: entry.id,
            starts,
        });
    }
    Ok(SkirmishDefinitions {
        starting_credits,
        factions,
        maps,
    })
}
