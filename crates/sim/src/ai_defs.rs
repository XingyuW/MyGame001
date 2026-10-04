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
//! defined; the power building produces power; the harvester is a harvester type with production
//! terms; every army unit type has production terms and a weapon, and for the armour class of
//! every building type some army unit's weapon deals damage (equation 2 above 0), so that the
//! army can destroy every building and a match between computer players can end.

use std::path::Path;

use serde::Deserialize;

use crate::data::{self, DataError, DataErrorKind, MAX_DATA_FILE_BYTES, Shape};
use crate::definitions::{Definitions, UnitKind, check_name};
use crate::economy_defs::{BuildingKind, MAX_BUILD_TIME, Problem, ProductionItem, invalid, ranged};
use crate::hash::Fnv1a64;

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

impl AiDefinitions {
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
    // A prerequisite of a building of the build order comes earlier in it, so that the build
    // order never waits for a building it would build only later.
    for (index, &kind) in build_order.iter().enumerate() {
        let prerequisites = definitions
            .production(ProductionItem::Building(kind))
            .map_or(&[][..], |terms| terms.prerequisites.as_slice());
        if let Some(missing) = prerequisites
            .iter()
            .find(|prerequisite| !build_order[..index].contains(prerequisite))
        {
            let name = |kind: BuildingKind| {
                definitions
                    .building(kind)
                    .map_or_else(String::new, |def| def.id.clone())
            };
            return Err(invalid(
                format!("build_order[{index}]"),
                format!(
                    "{:?}, whose prerequisite {:?} is not listed before it",
                    name(kind),
                    name(*missing)
                ),
                "a building type whose prerequisites are listed before it",
            ));
        }
    }
    let power = building(definitions, "power".into(), &file.power)?;
    if definitions
        .building(power)
        .is_none_or(|def| def.power_produced <= 0)
    {
        return Err(invalid(
            "power".into(),
            format!("{:?}, which produces no power", file.power),
            "a building type that produces power",
        ));
    }
    let harvester = unit(definitions, "harvester".into(), &file.harvester)?;
    if definitions.harvester(harvester).is_none()
        || definitions
            .production(ProductionItem::Unit(harvester))
            .is_none()
    {
        return Err(invalid(
            "harvester".into(),
            format!("{:?}", file.harvester),
            "a harvester unit type with production terms",
        ));
    }
    if file.army.is_empty() {
        return Err(invalid(
            "army".into(),
            "an empty list",
            format!("1 to {MAX_ARMY_ENTRIES} unit types"),
        ));
    }
    let mut army = Vec::with_capacity(file.army.len());
    for (index, id) in file.army.iter().enumerate() {
        let field = format!("army[{index}]");
        let kind = unit(definitions, field.clone(), id)?;
        let armed = definitions
            .unit_combat(kind)
            .is_some_and(|terms| terms.weapon.is_some());
        if !armed || definitions.production(ProductionItem::Unit(kind)).is_none() {
            return Err(invalid(
                field,
                format!("{id:?}"),
                "an armed unit type with production terms",
            ));
        }
        army.push(kind);
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
        let damaged = army.iter().any(|&kind| {
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
                    kind: "unit able to damage building".into(),
                    id: def.id.clone(),
                },
            ));
        }
    }
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
