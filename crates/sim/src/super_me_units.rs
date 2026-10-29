//! The game's own Super Me units (docs/DECISIONS.md R218): unit types that are no part of the
//! Red Alert 2 ruleset and that only a human player with Super Me may build. They are read from
//! their own file after the arsenal ([`Definitions::with_super_me_units`]), so the six files the
//! RA2 data parity audit compares with the reference stay exactly as generated.
//!
//! Super Me units file, schema version 1:
//!
//! ```ron
//! (
//!     schema_version: 1,
//!     units: [
//!         (id: "futa", template: "apocalypse", label: "Futa", description: "...", speed: 13,
//!          sprite: "futa", cost: 2000, build_time: 600, producer: "construction_yard",
//!          hit_points: 1200, armour: "heavy", sight: 10,
//!          weapons: ["gun_120mmx", "mammoth_tusk"], elite_weapons: ["gun_120mmx_e", "mammoth_tusk"],
//!          build_radius: 6, unlimited_power: 1, placed: 1, hover: 1),
//!     ],
//! )
//! ```
//!
//! Every field is required. `template` names a unit type of the arsenal whose entry the new type
//! copies (domain, role, veterancy, crushing and the other traits) and whose combat terms it
//! copies except hit points, armour, sight and weapons; the copy is offered to no side or country
//! (only Super Me opens it), has no prerequisites but its producer, and cannot be mind
//! controlled, so that no other player can take over what it grants. `weapons` and
//! `elite_weapons` name weapons of the combat file, primary first, one or two each.
//!
//! What the type grants its owner, while a unit of it stands and its owner has Super Me:
//! `unlimited_power` (1: the owner is never low on power, whatever its buildings and a spy's
//! outage) and `build_radius` (cells, as [`Footprint::distance`](crate::Footprint) measures them,
//! 0 for none: buildings may be placed within that reach of the unit, as of a base building).
//! `placed` (1: when complete the unit waits in its queue like a building and is put on any free
//! cell of the map by `PlaceUnit`, instead of leaving a factory) and `hover` (1: the game draws
//! it floating above the ground; it moves as any unit of its domain).

use std::path::Path;

use serde::Deserialize;

use crate::combat_defs::CombatTerms;
use crate::data::{self, DataError, DataErrorKind, MAX_DATA_FILE_BYTES, Shape};
use crate::definitions::{
    Definitions, MAX_UNIT_SPEED, MAX_UNIT_TYPES, UnitDef, UnitKind, check_name,
};
use crate::economy_defs::{MAX_BUILD_TIME, MAX_COST, Production, ProductionItem};

/// The schema version of the Super Me units file this crate reads.
pub const SUPER_ME_UNITS_SCHEMA_VERSION: i64 = 1;

/// Largest number of unit types in a Super Me units file.
pub const MAX_SUPER_ME_UNITS: usize = 4;

/// Largest build radius of a Super Me unit, in cells.
pub const MAX_BUILD_RADIUS: u32 = 16;

/// Largest hit points of a Super Me unit.
pub const MAX_SUPER_ME_HIT_POINTS: i32 = 10_000;

/// Largest sight of a Super Me unit, in cells.
pub const MAX_SUPER_ME_SIGHT: u32 = 16;

/// What a Super Me unit type grants and how it is placed (module documentation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SuperMeUnit {
    pub kind: UnitKind,
    /// Cells within which its owner may place buildings; 0: none.
    pub build_radius: u32,
    /// While one stands, its owner is never low on power.
    pub unlimited_power: bool,
    /// When complete it waits for `PlaceUnit` instead of leaving a factory.
    pub placed: bool,
    /// Drawn floating above the ground (presentation only).
    pub hover: bool,
}

static ENTRY_SHAPE: Shape = Shape::Struct(&[
    ("id", Shape::Str),
    ("template", Shape::Str),
    ("label", Shape::Str),
    ("description", Shape::Str),
    ("speed", Shape::Int),
    ("sprite", Shape::Str),
    ("cost", Shape::Int),
    ("build_time", Shape::Int),
    ("producer", Shape::Str),
    ("hit_points", Shape::Int),
    ("armour", Shape::Str),
    ("sight", Shape::Int),
    (
        "weapons",
        Shape::List {
            entry: &Shape::Str,
            max: 2,
        },
    ),
    (
        "elite_weapons",
        Shape::List {
            entry: &Shape::Str,
            max: 2,
        },
    ),
    ("build_radius", Shape::Int),
    ("unlimited_power", Shape::Int),
    ("placed", Shape::Int),
    ("hover", Shape::Int),
]);

static FILE_SHAPE: Shape = Shape::Struct(&[
    (
        "schema_version",
        Shape::SchemaVersion(SUPER_ME_UNITS_SCHEMA_VERSION),
    ),
    (
        "units",
        Shape::List {
            entry: &ENTRY_SHAPE,
            max: MAX_SUPER_ME_UNITS,
        },
    ),
]);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[allow(dead_code)]
    schema_version: i64,
    units: Vec<Entry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: String,
    template: String,
    label: String,
    description: String,
    speed: i64,
    sprite: String,
    cost: i64,
    build_time: i64,
    producer: String,
    hit_points: i64,
    armour: String,
    sight: i64,
    weapons: Vec<String>,
    elite_weapons: Vec<String>,
    build_radius: i64,
    unlimited_power: i64,
    placed: i64,
    hover: i64,
}

impl Definitions {
    /// These definitions together with the Super Me units file at `path` (module
    /// documentation), whose templates, producers, armour classes and weapons are resolved
    /// against them. Load it last ([`load_ruleset`](crate::load_ruleset) does): it needs the
    /// combat file and the arsenal, and each new type is appended to the units, the economy, the
    /// combat terms and the arsenal, which loading one of them again would replace.
    pub fn with_super_me_units(mut self, path: impl AsRef<Path>) -> Result<Self, DataError> {
        let path = path.as_ref();
        let error = |field: String, kind| DataError {
            file: path.to_owned(),
            field,
            kind,
        };
        let bytes = data::read_bounded(path, MAX_DATA_FILE_BYTES)
            .map_err(|kind| error("file".into(), kind))?;
        let text = data::check(path, &bytes, &FILE_SHAPE)?;
        let file: File = data::parse(path, text)?;
        for (index, entry) in file.units.into_iter().enumerate() {
            let field = |name: &str| format!("units[{index}].{name}");
            let invalid = |name: &str, found: String, expected: &str| {
                error(
                    field(name),
                    DataErrorKind::InvalidValue {
                        found,
                        expected: expected.to_owned(),
                    },
                )
            };
            let unknown = |name: &str, kind: &str, id: &str| {
                error(
                    field(name),
                    DataErrorKind::UnknownReference {
                        kind: kind.to_owned(),
                        id: id.to_owned(),
                    },
                )
            };
            check_name(&entry.id).map_err(|kind| error(field("id"), kind))?;
            check_name(&entry.sprite).map_err(|kind| error(field("sprite"), kind))?;
            if self.unit_kind(&entry.id).is_some() {
                return Err(error(field("id"), DataErrorKind::DuplicateId(entry.id)));
            }
            if self.units().len() >= MAX_UNIT_TYPES {
                return Err(invalid(
                    "id",
                    entry.id,
                    &format!("at most {MAX_UNIT_TYPES} unit types in all"),
                ));
            }
            let template = self
                .unit_kind(&entry.template)
                .ok_or_else(|| unknown("template", "unit", &entry.template))?;
            let (Some(template_entry), Some(template_combat)) = (
                self.metadata(ProductionItem::Unit(template)).cloned(),
                self.unit_combat(template),
            ) else {
                return Err(invalid(
                    "template",
                    entry.template,
                    "a unit type with an arsenal entry and combat terms",
                ));
            };
            let speed = u16::try_from(entry.speed)
                .ok()
                .filter(|speed| (1..=MAX_UNIT_SPEED).contains(speed))
                .ok_or_else(|| {
                    invalid(
                        "speed",
                        entry.speed.to_string(),
                        &format!("1 to {MAX_UNIT_SPEED} subcells per tick"),
                    )
                })?;
            if !(0..=MAX_COST).contains(&entry.cost) {
                return Err(invalid(
                    "cost",
                    entry.cost.to_string(),
                    &format!("0 to {MAX_COST} credits"),
                ));
            }
            let build_time = u32::try_from(entry.build_time)
                .ok()
                .filter(|time| (1..=MAX_BUILD_TIME).contains(time))
                .ok_or_else(|| {
                    invalid(
                        "build_time",
                        entry.build_time.to_string(),
                        &format!("1 to {MAX_BUILD_TIME} ticks"),
                    )
                })?;
            let producer = self
                .building_kind(&entry.producer)
                .ok_or_else(|| unknown("producer", "building", &entry.producer))?;
            let hit_points = i32::try_from(entry.hit_points)
                .ok()
                .filter(|hp| (1..=MAX_SUPER_ME_HIT_POINTS).contains(hp))
                .ok_or_else(|| {
                    invalid(
                        "hit_points",
                        entry.hit_points.to_string(),
                        &format!("1 to {MAX_SUPER_ME_HIT_POINTS}"),
                    )
                })?;
            let armour = self
                .armour_class(&entry.armour)
                .ok_or_else(|| unknown("armour", "armour class", &entry.armour))?;
            let sight = u32::try_from(entry.sight)
                .ok()
                .filter(|sight| *sight <= MAX_SUPER_ME_SIGHT)
                .ok_or_else(|| {
                    invalid(
                        "sight",
                        entry.sight.to_string(),
                        &format!("0 to {MAX_SUPER_ME_SIGHT} cells"),
                    )
                })?;
            let weapons = |name: &str, ids: &[String]| {
                if ids.is_empty() {
                    return Err(invalid(name, "an empty list".into(), "one or two weapons"));
                }
                let mut kinds = [None; 2];
                for (slot, id) in ids.iter().enumerate() {
                    kinds[slot] = Some(
                        self.weapon_kind(id)
                            .ok_or_else(|| unknown(name, "weapon", id))?,
                    );
                }
                Ok(kinds)
            };
            let [weapon, secondary] = weapons("weapons", &entry.weapons)?;
            let elite = weapons("elite_weapons", &entry.elite_weapons)?;
            let build_radius = u32::try_from(entry.build_radius)
                .ok()
                .filter(|radius| *radius <= MAX_BUILD_RADIUS)
                .ok_or_else(|| {
                    invalid(
                        "build_radius",
                        entry.build_radius.to_string(),
                        &format!("0 to {MAX_BUILD_RADIUS} cells"),
                    )
                })?;
            let flag = |name: &str, value: i64| match value {
                0 => Ok(false),
                1 => Ok(true),
                _ => Err(invalid(name, value.to_string(), "0 or 1")),
            };
            let unlimited_power = flag("unlimited_power", entry.unlimited_power)?;
            let placed = flag("placed", entry.placed)?;
            let hover = flag("hover", entry.hover)?;

            let mut arsenal = template_entry;
            arsenal.label = entry.label;
            arsenal.description = entry.description;
            arsenal.art = entry.id.clone();
            arsenal.side = None;
            arsenal.owner_sides = Vec::new();
            arsenal.required_countries = Vec::new();
            arsenal.forbidden_countries = Vec::new();
            arsenal.tech_level = 1;
            arsenal.prerequisite_groups = Vec::new();
            arsenal.immune_psionics = true;
            let combat = CombatTerms {
                hit_points,
                armour,
                sight,
                weapon,
                secondary,
                elite,
                ..template_combat
            };
            let kind = self.push_unit(
                UnitDef {
                    id: entry.id,
                    speed,
                    sprite: entry.sprite,
                },
                Production {
                    cost: entry.cost,
                    build_time,
                    producer,
                    prerequisites: Vec::new(),
                },
                combat,
                arsenal,
            );
            std::sync::Arc::make_mut(&mut self.super_me_units).push(SuperMeUnit {
                kind,
                build_radius,
                unlimited_power,
                placed,
                hover,
            });
        }
        Ok(self)
    }

    /// The Super Me units of the definitions, in the order of their file.
    pub fn super_me_units(&self) -> &[SuperMeUnit] {
        &self.super_me_units
    }

    /// What unit type `kind` grants if it is a Super Me unit.
    pub fn super_me_unit(&self, kind: UnitKind) -> Option<&SuperMeUnit> {
        self.super_me_units.iter().find(|unit| unit.kind == kind)
    }

    /// Whether `item` is a Super Me unit, which only a player with Super Me may build.
    pub fn is_super_me_item(&self, item: ProductionItem) -> bool {
        matches!(item, ProductionItem::Unit(kind) if self.super_me_unit(kind).is_some())
    }

    /// Whether a complete `item` waits in its queue to be placed: a building, or a `placed`
    /// Super Me unit.
    pub fn placed_on_completion(&self, item: ProductionItem) -> bool {
        match item {
            ProductionItem::Building(_) => true,
            ProductionItem::Unit(kind) => self.super_me_unit(kind).is_some_and(|unit| unit.placed),
        }
    }
}
