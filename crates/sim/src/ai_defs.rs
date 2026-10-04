//! The computer opponent's definitions loaded from RON: its build order, the building it builds
//! for power, its harvester, its army composition, and the parameters of each difficulty
//! (PLAN.md section 7, M6: "a data defined build order ... difficulty set by data parameters";
//! docs/DECISIONS.md D42).
//!
//! AI file, schema version 1:
//!
//! ```ron
//! (
//!     schema_version: 1,
//!     build_order: ["power_plant", "refinery", "barracks"],
//!     power: "power_plant",
//!     harvester: "harvester",
//!     army: ["light_tank", "light_tank", "scout"],
//!     difficulties: [
//!         (
//!             id: "normal",
//!             think_interval: 20,
//!             harvesters_per_refinery: 2,
//!             max_army: 20,
//!             wave_size: 6,
//!             first_wave: 3600,
//!             wave_patience: 2400,
//!             defence_radius: 10,
//!         ),
//!     ],
//! )
//! ```
//!
//! Every field is required, so there are no silent defaults. The file is cross validated against
//! the match definitions, which must include a combat file: every building of the build order is
//! defined, and its prerequisites are listed before it; the power building produces power and has
//! no prerequisites, so that it can be queued whenever power is short; the harvester is a
//! harvester type with production terms; every army unit type has production terms and a weapon;
//! the producer and the prerequisites of the harvester and of every army unit type are listed in
//! the build order, so that a computer player can obtain them and rebuild them; and for the armour
//! class of every building type some army unit's weapon deals damage (equation 2 above 0), so that
//! the army can destroy every building and a match between computer players can end.
//!
//! The producers of the buildings a computer player builds are usually starting buildings (a
//! construction yard), which the skirmish file defines, so they are checked against the factions
//! with [`check_ai_factions`] when both files are loaded, and again by match setup for every
//! computer player (M6 review 1 P2-1).
//!
//! Factions (M7, docs/DECISIONS.md D49): a computer player builds the entries of `build_order`
//! and `army` that the roster of its faction offers, in the file's order, and skips the others
//! ([`AiDefinitions::plan_for`]); `power` and `harvester` must be offered by its faction. This is
//! the specified meaning of the lists, not a fallback: [`check_ai_factions`] rejects an entry that
//! no faction offers, and checks the plan of every faction with every rule above, so that the
//! plan a computer player follows is complete for each faction it may play.

use std::path::Path;

use serde::Deserialize;

use crate::data::{self, DataError, DataErrorKind, MAX_DATA_FILE_BYTES, Shape};
use crate::definitions::{Definitions, UnitKind, check_name};
use crate::economy_defs::{BuildingKind, MAX_BUILD_TIME, Problem, ProductionItem, invalid, ranged};
use crate::hash::Fnv1a64;
use crate::skirmish_defs::{FactionDef, SkirmishDefinitions};

/// The schema version of the AI file this crate reads.
pub const AI_SCHEMA_VERSION: i64 = 1;

/// Longest build order.
pub const MAX_BUILD_ORDER: usize = 32;

/// Largest number of entries of the army composition.
pub const MAX_ARMY_ENTRIES: usize = 16;

/// Largest number of difficulties.
pub const MAX_DIFFICULTIES: usize = 8;

/// Longest interval between two decisions of a computer player, in ticks (one minute).
pub const MAX_THINK_INTERVAL: u32 = 1200;

/// Largest army a computer player builds. It bounds the units one command names (far below
/// `MAX_UNITS_PER_COMMAND`) and, with the starting units, the memory a computer player's units
/// use.
pub const MAX_AI_ARMY: u32 = 200;

/// Largest number of harvesters a computer player keeps per refinery.
pub const MAX_HARVESTERS_PER_REFINERY: u32 = 8;

/// Largest radius around its buildings in which a computer player defends, in cells.
pub const MAX_DEFENCE_RADIUS: u32 = 32;

/// A difficulty: how fast and how strongly a computer player acts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Difficulty {
    pub id: String,
    /// Ticks between two decisions, from 1 to [`MAX_THINK_INTERVAL`].
    pub think_interval: u32,
    /// Harvesters kept per refinery, from 0 to [`MAX_HARVESTERS_PER_REFINERY`].
    pub harvesters_per_refinery: u32,
    /// Armed units it builds up to, from 1 to [`MAX_AI_ARMY`].
    pub max_army: u32,
    /// Armed units at home that start an attack wave, from 1 to `max_army`.
    pub wave_size: u32,
    /// The tick from which attack waves may start, from 0 to `MAX_BUILD_TIME` (one hour).
    pub first_wave: u32,
    /// Ticks after `first_wave` or the last wave after which the armed units at home attack even
    /// if they are fewer than `wave_size`, from 1 to `MAX_BUILD_TIME`, so that an army that cannot
    /// grow (no credits left) still attacks.
    pub wave_patience: u32,
    /// Visible enemies this close to one of its buildings (Chebyshev distance in cells, 0 to
    /// [`MAX_DEFENCE_RADIUS`]) are attacked by its army at home.
    pub defence_radius: u32,
}

/// The AI file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiDefinitions {
    /// Building types in the order they are built; a type listed n times is kept at n buildings.
    pub build_order: Vec<BuildingKind>,
    /// The building type built when the power would not suffice.
    pub power: BuildingKind,
    /// The harvester unit type.
    pub harvester: UnitKind,
    /// Armed unit types, produced in this order, cyclically.
    pub army: Vec<UnitKind>,
    pub difficulties: Vec<Difficulty>,
}

/// The plan a computer player of one faction follows: the entries of the AI file that the roster
/// of its faction offers (M7, docs/DECISIONS.md D49).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiPlan {
    /// The buildings of `build_order` the faction offers, in the file's order.
    pub build_order: Vec<BuildingKind>,
    pub power: BuildingKind,
    pub harvester: UnitKind,
    /// The unit types of `army` the faction offers, in the file's order.
    pub army: Vec<UnitKind>,
}

impl AiPlan {
    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        hasher.write_u64(self.build_order.len() as u64);
        for kind in &self.build_order {
            hasher.write(&kind.0.to_le_bytes());
        }
        hasher.write(&self.power.0.to_le_bytes());
        hasher.write(&self.harvester.0.to_le_bytes());
        hasher.write_u64(self.army.len() as u64);
        for kind in &self.army {
            hasher.write(&kind.0.to_le_bytes());
        }
    }
}

impl AiDefinitions {
    /// The plan of a computer player of `faction`: the entries of `build_order` and `army` the
    /// faction's roster offers, in the file's order, with the power building and the harvester.
    /// [`check_ai_factions`] (and match setup) check that this plan is complete for every faction
    /// a computer player may play.
    pub fn plan_for(&self, faction: &FactionDef) -> AiPlan {
        AiPlan {
            build_order: self
                .build_order
                .iter()
                .copied()
                .filter(|&kind| faction.offers(ProductionItem::Building(kind)))
                .collect(),
            power: self.power,
            harvester: self.harvester,
            army: self
                .army
                .iter()
                .copied()
                .filter(|&kind| faction.offers(ProductionItem::Unit(kind)))
                .collect(),
        }
    }

    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        hasher.write_u64(self.build_order.len() as u64);
        for kind in &self.build_order {
            hasher.write(&kind.0.to_le_bytes());
        }
        hasher.write(&self.power.0.to_le_bytes());
        hasher.write(&self.harvester.0.to_le_bytes());
        hasher.write_u64(self.army.len() as u64);
        for kind in &self.army {
            hasher.write(&kind.0.to_le_bytes());
        }
        hasher.write_u64(self.difficulties.len() as u64);
        for difficulty in &self.difficulties {
            difficulty.hash_into(hasher);
        }
    }
}

impl Difficulty {
    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        hasher.write_u64(self.id.len() as u64);
        hasher.write(self.id.as_bytes());
        for value in [
            self.think_interval,
            self.harvesters_per_refinery,
            self.max_army,
            self.wave_size,
            self.first_wave,
            self.wave_patience,
            self.defence_radius,
        ] {
            hasher.write_u32(value);
        }
    }
}

// --- Schema ---------------------------------------------------------------------------------

static DIFFICULTY_SHAPE: Shape = Shape::Struct(&[
    ("id", Shape::Str),
    ("think_interval", Shape::Int),
    ("harvesters_per_refinery", Shape::Int),
    ("max_army", Shape::Int),
    ("wave_size", Shape::Int),
    ("first_wave", Shape::Int),
    ("wave_patience", Shape::Int),
    ("defence_radius", Shape::Int),
]);

static AI_FILE_SHAPE: Shape = Shape::Struct(&[
    ("schema_version", Shape::SchemaVersion(AI_SCHEMA_VERSION)),
    (
        "build_order",
        Shape::List {
            entry: &Shape::Str,
            max: MAX_BUILD_ORDER,
        },
    ),
    ("power", Shape::Str),
    ("harvester", Shape::Str),
    (
        "army",
        Shape::List {
            entry: &Shape::Str,
            max: MAX_ARMY_ENTRIES,
        },
    ),
    (
        "difficulties",
        Shape::List {
            entry: &DIFFICULTY_SHAPE,
            max: MAX_DIFFICULTIES,
        },
    ),
]);

/// The AI file as `ron` reads it. Integers are read as `i64`, the widest type the checker admits,
/// so that range errors are reported with their field.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AiFile {
    schema_version: i64,
    build_order: Vec<String>,
    power: String,
    harvester: String,
    army: Vec<String>,
    difficulties: Vec<DifficultyEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DifficultyEntry {
    id: String,
    think_interval: i64,
    harvesters_per_refinery: i64,
    max_army: i64,
    wave_size: i64,
    first_wave: i64,
    wave_patience: i64,
    defence_radius: i64,
}

/// Loads and validates the AI file at `path` against `definitions` (the units, economy, and
/// combat files of the match). Every problem is a [`DataError`] naming the file, the field, and
/// the reason.
pub fn load_ai(
    path: impl AsRef<Path>,
    definitions: &Definitions,
) -> Result<AiDefinitions, DataError> {
    let path = path.as_ref();
    let bytes = data::read_bounded(path, MAX_DATA_FILE_BYTES).map_err(|kind| DataError {
        file: path.to_owned(),
        field: "file".into(),
        kind,
    })?;
    let text = data::check(path, &bytes, &AI_FILE_SHAPE)?;
    let file: AiFile = data::parse(path, text)?;
    validate(file, definitions).map_err(|(field, kind)| DataError {
        file: path.to_owned(),
        field,
        kind,
    })
}

fn building(definitions: &Definitions, field: String, id: &str) -> Result<BuildingKind, Problem> {
    definitions.building_kind(id).ok_or((
        field,
        DataErrorKind::UnknownReference {
            kind: "building".into(),
            id: id.to_owned(),
        },
    ))
}

fn unit(definitions: &Definitions, field: String, id: &str) -> Result<UnitKind, Problem> {
    definitions.unit_kind(id).ok_or((
        field,
        DataErrorKind::UnknownReference {
            kind: "unit".into(),
            id: id.to_owned(),
        },
    ))
}

fn validate(file: AiFile, definitions: &Definitions) -> Result<AiDefinitions, Problem> {
    if file.schema_version != AI_SCHEMA_VERSION {
        return Err((
            "schema_version".into(),
            DataErrorKind::UnsupportedSchemaVersion {
                found: file.schema_version,
                expected: AI_SCHEMA_VERSION,
            },
        ));
    }
    if !definitions.has_combat() {
        // The army is checked against the weapons; without a combat file there are none.
        return Err(invalid(
            "file".into(),
            "definitions without a combat file",
            "the definitions of a match, with a combat file",
        ));
    }
    if file.build_order.is_empty() {
        return Err(invalid(
            "build_order".into(),
            "an empty list",
            format!("1 to {MAX_BUILD_ORDER} building types"),
        ));
    }
    let build_order = file
        .build_order
        .iter()
        .enumerate()
        .map(|(index, id)| building(definitions, format!("build_order[{index}]"), id))
        .collect::<Result<Vec<_>, _>>()?;
    let power = building(definitions, "power".into(), &file.power)?;
    let harvester = unit(definitions, "harvester".into(), &file.harvester)?;
    if file.army.is_empty() {
        return Err(invalid(
            "army".into(),
            "an empty list",
            format!("1 to {MAX_ARMY_ENTRIES} unit types"),
        ));
    }
    let mut army = Vec::with_capacity(file.army.len());
    for (index, id) in file.army.iter().enumerate() {
        army.push(unit(definitions, format!("army[{index}]"), id)?);
    }
    let build_entries: Vec<(usize, BuildingKind)> =
        build_order.iter().copied().enumerate().collect();
    let army_entries: Vec<(usize, UnitKind)> = army.iter().copied().enumerate().collect();
    check_plan(
        definitions,
        &build_entries,
        power,
        harvester,
        &army_entries,
        None,
    )?;
    if file.difficulties.is_empty() {
        return Err(invalid(
            "difficulties".into(),
            "an empty list",
            format!("1 to {MAX_DIFFICULTIES} difficulties"),
        ));
    }
    let mut difficulties: Vec<Difficulty> = Vec::with_capacity(file.difficulties.len());
    for (index, entry) in file.difficulties.into_iter().enumerate() {
        let field = |name: &str| format!("difficulties[{index}].{name}");
        check_name(&entry.id).map_err(|kind| (field("id"), kind))?;
        if difficulties.iter().any(|known| known.id == entry.id) {
            return Err((field("id"), DataErrorKind::DuplicateId(entry.id)));
        }
        let max_army = ranged(&field("max_army"), entry.max_army, 1, MAX_AI_ARMY, " units")?;
        difficulties.push(Difficulty {
            think_interval: ranged(
                &field("think_interval"),
                entry.think_interval,
                1,
                MAX_THINK_INTERVAL,
                " ticks",
            )?,
            harvesters_per_refinery: ranged(
                &field("harvesters_per_refinery"),
                entry.harvesters_per_refinery,
                0,
                MAX_HARVESTERS_PER_REFINERY,
                " harvesters",
            )?,
            max_army,
            wave_size: ranged(
                &field("wave_size"),
                entry.wave_size,
                1,
                max_army,
                " units (at most max_army)",
            )?,
            first_wave: ranged(
                &field("first_wave"),
                entry.first_wave,
                0,
                MAX_BUILD_TIME,
                " ticks",
            )?,
            wave_patience: ranged(
                &field("wave_patience"),
                entry.wave_patience,
                1,
                MAX_BUILD_TIME,
                " ticks",
            )?,
            defence_radius: ranged(
                &field("defence_radius"),
                entry.defence_radius,
                0,
                MAX_DEFENCE_RADIUS,
                " cells",
            )?,
            id: entry.id,
        });
    }
    Ok(AiDefinitions {
        build_order,
        power,
        harvester,
        army,
        difficulties,
    })
}

/// The name of building type `kind`, or an empty string if it is not defined.
fn building_name(definitions: &Definitions, kind: BuildingKind) -> String {
    definitions
        .building(kind)
        .map_or_else(String::new, |def| def.id.clone())
}

/// The name of unit type `kind`, or an empty string if it is not defined.
fn unit_name(definitions: &Definitions, kind: UnitKind) -> String {
    definitions
        .unit(kind)
        .map_or_else(String::new, |def| def.id.clone())
}

/// The rules of a plan (docs/DECISIONS.md D42, D46): every prerequisite of a building of the build
/// order is listed before it; the power building produces power and has no prerequisites; the
/// harvester is a harvester type with production terms whose producer and prerequisites the
/// build order lists; the army is not empty, and each of its unit types is armed, has production
/// terms, and has its producer and prerequisites in the build order; and for the armour class of
/// every building type some army unit's weapon deals damage. The entries carry their index in the
/// AI file, so that an error names the file's field. `faction` is `None` for the file's own lists
/// and names the faction for a faction's plan (M7), whose name the error then gives.
fn check_plan(
    definitions: &Definitions,
    build_order: &[(usize, BuildingKind)],
    power: BuildingKind,
    harvester: UnitKind,
    army: &[(usize, UnitKind)],
    faction: Option<&str>,
) -> Result<(), Problem> {
    let of = |found: String| match faction {
        None => found,
        Some(faction) => format!("{found} in the plan of faction {faction:?}"),
    };
    let kinds: Vec<BuildingKind> = build_order.iter().map(|&(_, kind)| kind).collect();
    if build_order.is_empty() {
        return Err(invalid(
            "build_order".into(),
            of("no building type".into()),
            "a build order with at least one building type the faction offers",
        ));
    }
    // A prerequisite of a building of the build order comes earlier in it, so that the build
    // order never waits for a building it would build only later.
    for (position, &(index, kind)) in build_order.iter().enumerate() {
        let prerequisites = definitions
            .production(ProductionItem::Building(kind))
            .map_or(&[][..], |terms| terms.prerequisites.as_slice());
        if let Some(missing) = prerequisites
            .iter()
            .find(|prerequisite| !kinds[..position].contains(prerequisite))
        {
            return Err(invalid(
                format!("build_order[{index}]"),
                of(format!(
                    "{:?}, whose prerequisite {:?} is not listed before it",
                    building_name(definitions, kind),
                    building_name(definitions, *missing)
                )),
                "a building type whose prerequisites are listed before it",
            ));
        }
    }
    let power_name = building_name(definitions, power);
    if definitions
        .building(power)
        .is_none_or(|def| def.power_produced <= 0)
    {
        return Err(invalid(
            "power".into(),
            of(format!("{power_name:?}, which produces no power")),
            "a building type that produces power",
        ));
    }
    // The power building is queued whenever the next building would leave the player short of
    // power, from the first turn on, so nothing but its producer may be needed to queue it.
    if definitions
        .production(ProductionItem::Building(power))
        .is_some_and(|terms| !terms.prerequisites.is_empty())
    {
        return Err(invalid(
            "power".into(),
            of(format!("{power_name:?}, which has prerequisites")),
            "a building type that produces power and has no prerequisites",
        ));
    }
    let harvester_name = unit_name(definitions, harvester);
    if definitions.harvester(harvester).is_none()
        || definitions
            .production(ProductionItem::Unit(harvester))
            .is_none()
    {
        return Err(invalid(
            "harvester".into(),
            of(format!("{harvester_name:?}")),
            "a harvester unit type with production terms",
        ));
    }
    obtainable(definitions, &kinds, "harvester", &harvester_name, harvester)
        .map_err(|(field, kind)| (field, in_plan(kind, faction)))?;
    if army.is_empty() {
        return Err(invalid(
            "army".into(),
            of("no unit type".into()),
            "an army with at least one unit type the faction offers",
        ));
    }
    for &(index, kind) in army {
        let field = format!("army[{index}]");
        let id = unit_name(definitions, kind);
        let armed = definitions
            .unit_combat(kind)
            .is_some_and(|terms| terms.weapon.is_some());
        if !armed || definitions.production(ProductionItem::Unit(kind)).is_none() {
            return Err(invalid(
                field,
                of(format!("{id:?}")),
                "an armed unit type with production terms",
            ));
        }
        obtainable(definitions, &kinds, &field, &id, kind)
            .map_err(|(field, kind)| (field, in_plan(kind, faction)))?;
    }
    // The army must be able to damage every building type, or a match between computer players
    // could not end (docs/DECISIONS.md D42).
    for (index, def) in definitions.buildings().iter().enumerate() {
        let Some(terms) = u16::try_from(index)
            .ok()
            .and_then(|index| definitions.building_combat(BuildingKind(index)))
        else {
            continue;
        };
        let damaged = army.iter().any(|&(_, kind)| {
            definitions
                .unit_combat(kind)
                .and_then(|unit| unit.weapon)
                .and_then(|weapon| definitions.weapon(weapon))
                .and_then(|weapon| definitions.versus(weapon.warhead, terms.armour))
                .is_some_and(|percent| percent > 0)
        });
        if !damaged {
            return Err((
                "army".into(),
                DataErrorKind::MissingEntry {
                    kind: match faction {
                        None => "unit able to damage building".into(),
                        Some(faction) => {
                            format!("unit of faction {faction:?} able to damage building")
                        }
                    },
                    id: def.id.clone(),
                },
            ));
        }
    }
    Ok(())
}

/// `kind` with the faction named in its found value, for a faction's plan.
fn in_plan(kind: DataErrorKind, faction: Option<&str>) -> DataErrorKind {
    match (kind, faction) {
        (DataErrorKind::InvalidValue { found, expected }, Some(faction)) => {
            DataErrorKind::InvalidValue {
                found: format!("{found} in the plan of faction {faction:?}"),
                expected,
            }
        }
        (kind, _) => kind,
    }
}

/// Checks that the build order lists the producer and every prerequisite of unit type `kind`
/// (named `id`, at `field`), so that a computer player can obtain them, and rebuild them when they
/// are lost; otherwise it could never produce the unit (M6 review 1 P2-1: without its producer,
/// a computer player builds no army, never attacks, and the match does not end).
fn obtainable(
    definitions: &Definitions,
    build_order: &[BuildingKind],
    field: &str,
    id: &str,
    kind: UnitKind,
) -> Result<(), Problem> {
    let Some(terms) = definitions.production(ProductionItem::Unit(kind)) else {
        return Err(invalid(
            field.to_owned(),
            format!("{id:?}"),
            "a unit type with production terms",
        ));
    };
    let missing = std::iter::once(("producer", terms.producer))
        .chain(
            terms
                .prerequisites
                .iter()
                .map(|&prerequisite| ("prerequisite", prerequisite)),
        )
        .find(|(_, building)| !build_order.contains(building));
    match missing {
        None => Ok(()),
        Some((role, building)) => Err(invalid(
            field.to_owned(),
            format!(
                "{id:?}, whose {role} {:?} is not listed in the build order",
                definitions
                    .building(building)
                    .map_or_else(String::new, |def| def.id.clone())
            ),
            "a unit type whose producer and prerequisites are listed in the build order",
        )),
    }
}

/// The first building the AI file has a computer player of `faction` build whose producer that
/// player would never own, as `(field of the AI file, building, producer)`: a building of the
/// faction's plan ([`AiDefinitions::plan_for`]) whose producer is neither a starting building of
/// the faction nor listed before it in the plan's build order, or the power building (queued
/// whenever power is short, from the first turn on) whose producer is not a starting building of
/// the faction. Without its producer the build order would wait for that building forever (M6
/// review 1 P2-1).
pub(crate) fn unobtainable_producer(
    ai: &AiDefinitions,
    definitions: &Definitions,
    faction: &FactionDef,
) -> Option<(String, BuildingKind, BuildingKind)> {
    let starts_with = |kind: BuildingKind| {
        faction
            .buildings
            .iter()
            .any(|building| building.kind == kind)
    };
    let producer = |kind: BuildingKind| {
        definitions
            .production(ProductionItem::Building(kind))
            .map(|terms| terms.producer)
    };
    let plan: Vec<(usize, BuildingKind)> = faction_build_order(ai, faction);
    for (position, &(index, kind)) in plan.iter().enumerate() {
        if let Some(producer) = producer(kind)
            && !starts_with(producer)
            && !plan[..position]
                .iter()
                .any(|&(_, earlier)| earlier == producer)
        {
            return Some((format!("build_order[{index}]"), kind, producer));
        }
    }
    match producer(ai.power) {
        Some(producer) if !starts_with(producer) => Some(("power".into(), ai.power, producer)),
        _ => None,
    }
}

/// The entries of the build order that `faction` offers, with their index in the AI file.
fn faction_build_order(ai: &AiDefinitions, faction: &FactionDef) -> Vec<(usize, BuildingKind)> {
    ai.build_order
        .iter()
        .copied()
        .enumerate()
        .filter(|&(_, kind)| faction.offers(ProductionItem::Building(kind)))
        .collect()
}

/// The entries of the army that `faction` offers, with their index in the AI file.
fn faction_army(ai: &AiDefinitions, faction: &FactionDef) -> Vec<(usize, UnitKind)> {
    ai.army
        .iter()
        .copied()
        .enumerate()
        .filter(|&(_, kind)| faction.offers(ProductionItem::Unit(kind)))
        .collect()
}

/// Why the plan of a computer player of `faction` is incomplete (M7, docs/DECISIONS.md D49), as
/// the field of the AI file and the reason: the faction does not offer the power building or the
/// harvester, or its plan breaks a rule of the AI file (`check_plan`), for example because the
/// faction offers no army unit type of the file that can damage some building type.
pub(crate) fn faction_plan_problem(
    ai: &AiDefinitions,
    definitions: &Definitions,
    faction: &FactionDef,
) -> Result<(), Problem> {
    if !faction.offers(ProductionItem::Building(ai.power)) {
        return Err(invalid(
            "power".into(),
            format!(
                "{:?}, which faction {:?} does not offer",
                building_name(definitions, ai.power),
                faction.id
            ),
            "a building type the roster of every faction offers",
        ));
    }
    if !faction.offers(ProductionItem::Unit(ai.harvester)) {
        return Err(invalid(
            "harvester".into(),
            format!(
                "{:?}, which faction {:?} does not offer",
                unit_name(definitions, ai.harvester),
                faction.id
            ),
            "a unit type the roster of every faction offers",
        ));
    }
    check_plan(
        definitions,
        &faction_build_order(ai, faction),
        ai.power,
        ai.harvester,
        &faction_army(ai, faction),
        Some(&faction.id),
    )
}

/// Checks the AI file at `path` against every faction of the skirmish file (cross validation of
/// the two files), so that a computer player can play every faction:
///
/// 1. (M6 review 1 P2-1) for every faction, the producer of every building of its plan is a
///    starting building of the faction or is listed before that building in the plan, and the
///    producer of the power building is a starting building of the faction;
/// 2. (M7) every entry of the build order and of the army is offered by some faction, so that no
///    entry is dead;
/// 3. (M7) for every faction, its roster offers the power building and the harvester, and its
///    plan ([`AiDefinitions::plan_for`]) keeps every rule of the AI file.
///
/// The error names the AI file, the field, the reason, and the faction.
pub fn check_ai_factions(
    path: impl AsRef<Path>,
    ai: &AiDefinitions,
    skirmish: &SkirmishDefinitions,
    definitions: &Definitions,
) -> Result<(), DataError> {
    let error = |(field, kind): Problem| DataError {
        file: path.as_ref().to_owned(),
        field,
        kind,
    };
    for faction in &skirmish.factions {
        if let Some((field, building, producer)) = unobtainable_producer(ai, definitions, faction) {
            return Err(error(invalid(
                field,
                format!(
                    "{:?}, whose producer {:?} faction {:?} of the skirmish file does not start \
                     with",
                    building_name(definitions, building),
                    building_name(definitions, producer),
                    faction.id
                ),
                "a building type whose producer every faction starts with, or (in the build \
                 order) one listed before it",
            )));
        }
    }
    let offered =
        |item: ProductionItem| skirmish.factions.iter().any(|faction| faction.offers(item));
    for (index, &kind) in ai.build_order.iter().enumerate() {
        if !offered(ProductionItem::Building(kind)) {
            return Err(error(invalid(
                format!("build_order[{index}]"),
                format!(
                    "{:?}, which no faction of the skirmish file offers",
                    building_name(definitions, kind)
                ),
                "a building type the roster of some faction offers",
            )));
        }
    }
    for (index, &kind) in ai.army.iter().enumerate() {
        if !offered(ProductionItem::Unit(kind)) {
            return Err(error(invalid(
                format!("army[{index}]"),
                format!(
                    "{:?}, which no faction of the skirmish file offers",
                    unit_name(definitions, kind)
                ),
                "a unit type the roster of some faction offers",
            )));
        }
    }
    for faction in &skirmish.factions {
        faction_plan_problem(ai, definitions, faction).map_err(error)?;
    }
    Ok(())
}
