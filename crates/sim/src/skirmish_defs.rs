//! Skirmish definitions loaded from RON: the starting credits, the factions with their starting
//! buildings and units, and the maps with their start positions (PLAN.md section 7, M6;
//! docs/DECISIONS.md D40).
//!
//! Skirmish file, schema version 3 (version 1, M6, had no rosters; version 2, M7, no sides):
//!
//! ```ron
//! (
//!     schema_version: 3,
//!     starting_credits: 5000,
//!     sides: [(side: "allied", starting_vehicles: ["grizzly"], starting_infantry: ["gi"])],
//!     factions: [
//!         (
//!             id: "vanguard",
//!             label: "Vanguard",
//!             side: "none",
//!             roster_buildings: ["power_plant", "refinery", "barracks"],
//!             roster_units: ["light_tank", "harvester"],
//!             buildings: [(building: "construction_yard", x: 0, y: 0)],
//!             units: [(unit: "light_tank", x: 4, y: 1, count: 4)],
//!         ),
//!         (
//!             id: "america", label: "America", side: "allied",
//!             roster_buildings: [], roster_units: [], buildings: [], units: [],
//!         ),
//!     ],
//!     maps: [(id: "demo", starts: [(x: 15, y: 37), (x: 52, y: 42)])],
//! )
//! ```
//!
//! Every field is required, so there are no silent defaults. Faction and map ids follow the rules
//! of unit ids (`definitions::check_name`); a map id `m` names the map file `maps/m/m.tmx` under
//! the asset directory ([`map_asset_path`]). Every map has from [`MIN_MATCH_PLAYERS`] to
//! [`MAX_MATCH_PLAYERS`] start positions, each a distinct cell; a match uses as many of them, in
//! order, as it has players. A match with more than two players is played on the map enlarged for
//! them (`crate::map_scale`), with its start positions moved accordingly and the missing ones
//! made, so a match may have more players than its map file lists start positions.
//!
//! A faction is one of two kinds (RA1, docs/RA2_PARITY_PLAN.md section 3.3; docs/DECISIONS.md
//! R10, R13):
//!
//! * **Without a side** (`side: "none"`, the factions of M6 and M7 and of the fixture scenarios):
//!   its buildings and units are placed relative to the start position of the player who plays
//!   it (a building's footprint origin is the start plus its offset, and a group of units stands
//!   on the free cells nearest to the start plus its offset), and its roster (M7, docs/
//!   DECISIONS.md D48) lists the building and unit types a player of the faction may produce:
//!   `QueueProduction` of anything else is rejected (`RejectReason::NotInRoster`). Every roster
//!   entry is a defined type with production terms, listed once. Starting buildings and units
//!   and the free units of a placed building (a refinery's harvester) need not be in the roster.
//! * **A country** (`side: "allied"` or `"soviet"`): its id is a country of the rules file with
//!   that side, and its four lists are empty. A player of a country starts with the base unit of
//!   its side (or the Construction Yard it deploys into) and the starting units the match
//!   options ask for, chosen from the side's entry in `sides`, and has no roster: what it may
//!   build is decided by the buildings it owns and its country (`economy_commands::
//!   check_buildable`). [`FactionDef::roster`] of a country holds its own tech tree instead
//!   ([`country_tree`]), for the callers that ask what a faction offers.
//!
//! `sides` lists, for a side of the rules file, the unit types a player's starting units are
//! drawn from (the ini's `AllowedToStartInMultiplayer`): each has production terms with a cost
//! above 0. Every side a country names has an entry.
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
use crate::economy::PlayerState;
use crate::economy_commands::check_buildable;
use crate::economy_defs::{
    BuildingKind, MAX_COST, Problem, ProductionItem, invalid, ranged, unknown,
};
use crate::map::{CellPos, Grid, MAX_MAP_SIDE};
use crate::rules::{CountryId, Side};

/// The schema version of the skirmish file this crate reads (2 since M7: faction rosters; 3 since
/// RA1: sides and countries).
pub const SKIRMISH_SCHEMA_VERSION: i64 = 3;

/// Fewest players of a skirmish match (and start positions of a map).
pub const MIN_MATCH_PLAYERS: usize = 2;

/// Most players of a skirmish match (and start positions of a map): one human and up to nine
/// independent computer players. A match with more players than its map file has start positions
/// is played on the map enlarged for them, which has the missing ones (`crate::map_scale`).
pub const MAX_MATCH_PLAYERS: usize = 10;

/// Players of the classic one-against-one match (the M6/M7 format, still the default).
pub const MATCH_PLAYERS: usize = 2;

/// Largest number of factions.
pub const MAX_FACTIONS: usize = 16;

/// Largest number of unit types in one of a side's lists of starting units.
pub const MAX_STARTING_TYPES: usize = 16;

/// Longest label of a faction, in bytes.
pub const MAX_LABEL_LEN: usize = 32;

/// Largest number of maps.
pub const MAX_MAPS: usize = 24;

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
    /// The name the game shows.
    pub label: String,
    /// `None` for a faction with a roster and starting lists; the side of a country.
    pub side: Option<Side>,
    /// The country of the rules file with the faction's id, for a faction with a side.
    pub country: Option<CountryId>,
    /// The building and unit types a player of the faction may produce (each with production
    /// terms): the roster of a faction without a side, which its players are held to; the own
    /// tech tree of a country ([`country_tree`]), whose players have no roster.
    pub roster: BTreeSet<ProductionItem>,
    /// For a faction without a side at least one building, so that a player does not start
    /// defeated; empty for a country.
    pub buildings: Vec<StartingBuilding>,
    pub units: Vec<StartingGroup>,
}

impl FactionDef {
    /// Whether a player of the faction may produce `item`: the roster offers it, or for a
    /// country its own tech tree holds it.
    pub fn offers(&self, item: ProductionItem) -> bool {
        self.roster.contains(&item)
    }

    /// Whether a player of the faction owns a building of type `kind` from the start: one of its
    /// starting buildings, or for a country the Construction Yard its base unit deploys into.
    pub fn starts_with(&self, definitions: &Definitions, kind: BuildingKind) -> bool {
        self.buildings.iter().any(|building| building.kind == kind)
            || self.side.is_some_and(|side| {
                definitions
                    .rules()
                    .and_then(|rules| rules.side(side))
                    .is_some_and(|rules| rules.construction_yard == kind)
            })
    }
}

/// The unit types the starting units of a side's players are drawn from (docs/
/// RA2_PARITY_PLAN.md section 4.9).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SideStart {
    pub side: Side,
    pub vehicles: Vec<UnitKind>,
    pub infantry: Vec<UnitKind>,
}

/// Everything a player of `country` can build on its own, starting from the Construction Yard of
/// its side: the buildings the buildability rule (`economy_commands::check_buildable`) admits
/// for a player who owns the yard and every building found so far, until none is added, and the
/// units it then admits. Items of the other side, which need a captured building, and the
/// specials of other countries are not in it; superweapons are, whatever a match's options say.
/// Empty without a rules file or for a country it does not define.
pub fn country_tree(definitions: &Definitions, country: CountryId) -> BTreeSet<ProductionItem> {
    let mut tree = BTreeSet::new();
    let Some(yard) = definitions.rules().and_then(|rules| {
        let side = rules.country(country)?.side;
        Some(rules.side(side)?.construction_yard)
    }) else {
        return tree;
    };
    let state = PlayerState {
        country: Some(country),
        ..PlayerState::default()
    };
    let mut owned = vec![yard];
    let buildings = definitions.buildings().len();
    loop {
        let before = owned.len();
        for index in 0..buildings {
            let Ok(index) = u16::try_from(index) else {
                break;
            };
            let kind = BuildingKind(index);
            if !owned.contains(&kind)
                && check_buildable(
                    definitions,
                    &state,
                    true,
                    &owned,
                    ProductionItem::Building(kind),
                )
                .is_ok()
            {
                owned.push(kind);
                tree.insert(ProductionItem::Building(kind));
            }
        }
        if owned.len() == before {
            break;
        }
    }
    for index in 0..definitions.units().len() {
        let Ok(index) = u16::try_from(index) else {
            break;
        };
        let item = ProductionItem::Unit(UnitKind(index));
        if check_buildable(definitions, &state, true, &owned, item).is_ok() {
            tree.insert(item);
        }
    }
    tree
}

/// A map of the skirmish: its id and its start positions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapDef {
    pub id: String,
    /// [`MIN_MATCH_PLAYERS`] to [`MAX_MATCH_PLAYERS`] distinct start positions.
    pub starts: Vec<CellPos>,
}

/// The skirmish definitions (the skirmish file).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkirmishDefinitions {
    /// Credits of every player at the start, from 0 to [`MAX_STARTING_CREDITS`]; a match's
    /// options may name another amount (`MatchOptions::starting_credits`).
    pub starting_credits: i64,
    /// At most one entry per side.
    pub sides: Vec<SideStart>,
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

impl SkirmishDefinitions {
    /// The starting unit types of `side`, if the file lists the side.
    pub fn side(&self, side: Side) -> Option<&SideStart> {
        self.sides.iter().find(|start| start.side == side)
    }

    /// The indices of the factions that are countries, in the file's order.
    pub fn countries(&self) -> Vec<usize> {
        (0..self.factions.len())
            .filter(|&index| self.factions[index].side.is_some())
            .collect()
    }

    /// The mean cost of the starting unit types of all sides together, rounded down (0 without
    /// any): the unit of the starting units' budget (docs/RA2_PARITY_PLAN.md section 4.9).
    pub fn average_starting_cost(&self, definitions: &Definitions) -> i64 {
        let costs: Vec<i64> = self
            .sides
            .iter()
            .flat_map(|side| side.vehicles.iter().chain(&side.infantry))
            .filter_map(|&kind| definitions.production(ProductionItem::Unit(kind)))
            .map(|terms| terms.cost)
            .collect();
        match i64::try_from(costs.len()) {
            Ok(count) if count > 0 => costs.iter().sum::<i64>() / count,
            _ => 0,
        }
    }
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

static SIDE_SHAPE: Shape = Shape::Struct(&[
    ("side", Shape::Str),
    (
        "starting_vehicles",
        Shape::List {
            entry: &Shape::Str,
            max: MAX_STARTING_TYPES,
        },
    ),
    (
        "starting_infantry",
        Shape::List {
            entry: &Shape::Str,
            max: MAX_STARTING_TYPES,
        },
    ),
]);

static FACTION_SHAPE: Shape = Shape::Struct(&[
    ("id", Shape::Str),
    ("label", Shape::Str),
    ("side", Shape::Str),
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
            max: MAX_MATCH_PLAYERS,
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
        "sides",
        Shape::List {
            entry: &SIDE_SHAPE,
            max: 2,
        },
    ),
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
    sides: Vec<SideEntry>,
    factions: Vec<FactionEntry>,
    maps: Vec<MapEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SideEntry {
    side: String,
    starting_vehicles: Vec<String>,
    starting_infantry: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FactionEntry {
    id: String,
    label: String,
    side: String,
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
    let mut sides: Vec<SideStart> = Vec::with_capacity(file.sides.len());
    for (index, entry) in file.sides.iter().enumerate() {
        let field = format!("sides[{index}]");
        // A side is one the rules file lists; without rules there are no sides.
        let side = Side::parse(&entry.side)
            .filter(|&side| {
                definitions
                    .rules()
                    .is_some_and(|rules| rules.side(side).is_some())
            })
            .ok_or_else(|| {
                invalid(
                    format!("{field}.side"),
                    format!("{:?}", entry.side),
                    "a side of the rules file",
                )
            })?;
        if sides.iter().any(|known| known.side == side) {
            return Err((
                format!("{field}.side"),
                DataErrorKind::DuplicateId(entry.side.clone()),
            ));
        }
        let types = |name: &str, ids: &[String]| -> Result<Vec<UnitKind>, Problem> {
            let mut kinds = Vec::with_capacity(ids.len());
            for (position, id) in ids.iter().enumerate() {
                let at = format!("{field}.{name}[{position}]");
                let kind = definitions
                    .unit_kind(id)
                    .ok_or_else(|| unknown(at.clone(), "unit", id))?;
                // The budget of the starting units is spent by cost, so a type without one
                // would never use it up.
                if definitions
                    .production(ProductionItem::Unit(kind))
                    .is_none_or(|terms| terms.cost <= 0)
                {
                    return Err(invalid(
                        at,
                        format!("{id:?}"),
                        "a unit type with production terms and a cost above 0",
                    ));
                }
                if kinds.contains(&kind) {
                    return Err((at, DataErrorKind::DuplicateId(id.clone())));
                }
                kinds.push(kind);
            }
            Ok(kinds)
        };
        sides.push(SideStart {
            side,
            vehicles: types("starting_vehicles", &entry.starting_vehicles)?,
            infantry: types("starting_infantry", &entry.starting_infantry)?,
        });
    }
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
        if !(1..=MAX_LABEL_LEN).contains(&entry.label.len()) {
            return Err(invalid(
                format!("{field}.label"),
                format!("{:?}", entry.label),
                format!("1 to {MAX_LABEL_LEN} characters"),
            ));
        }
        if entry.side != "none" {
            // A country: a country of the rules file of that side, with empty lists.
            let country = Side::parse(&entry.side)
                .filter(|&side| sides.iter().any(|known| known.side == side))
                .ok_or_else(|| {
                    invalid(
                        format!("{field}.side"),
                        format!("{:?}", entry.side),
                        "\"none\" or a side listed in sides",
                    )
                })
                .and_then(|side| {
                    definitions
                        .rules()
                        .and_then(|rules| {
                            let id = rules.country_id(&entry.id)?;
                            (rules.country(id)?.side == side).then_some((side, id))
                        })
                        .ok_or_else(|| {
                            invalid(
                                format!("{field}.id"),
                                format!("{:?}", entry.id),
                                format!("a country of the rules file of the side {:?}", entry.side),
                            )
                        })
                })?;
            for (name, length) in [
                ("roster_buildings", entry.roster_buildings.len()),
                ("roster_units", entry.roster_units.len()),
                ("buildings", entry.buildings.len()),
                ("units", entry.units.len()),
            ] {
                if length != 0 {
                    return Err(invalid(
                        format!("{field}.{name}"),
                        format!("{length} entries"),
                        "an empty list: a country starts with the base unit of its side and \
                         builds by the rules",
                    ));
                }
            }
            factions.push(FactionDef {
                id: entry.id,
                label: entry.label,
                side: Some(country.0),
                country: Some(country.1),
                roster: country_tree(definitions, country.1),
                buildings: Vec::new(),
                units: Vec::new(),
            });
            continue;
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
            label: entry.label,
            side: None,
            country: None,
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
        if !(MIN_MATCH_PLAYERS..=MAX_MATCH_PLAYERS).contains(&entry.starts.len()) {
            return Err(invalid(
                format!("{field}.starts"),
                format!("{} start positions", entry.starts.len()),
                format!("{MIN_MATCH_PLAYERS} to {MAX_MATCH_PLAYERS} start positions"),
            ));
        }
        let mut starts = vec![CellPos::new(0, 0); entry.starts.len()];
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
        sides,
        factions,
        maps,
    })
}
