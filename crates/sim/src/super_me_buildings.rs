//! The game's own Super Me buildings (docs/DECISIONS.md R218, R220): building types, with
//! weapons of their own, that are no part of the Red Alert 2 ruleset and that only a human
//! player with Super Me may build. They are read from their own file after the arsenal
//! ([`Definitions::with_super_me_buildings`]), so the six files the RA2 data parity audit compares
//! with the reference stay exactly as generated.
//!
//! Super Me file, schema version 2:
//!
//! ```ron
//! (
//!     schema_version: 2,
//!     weapons: [
//!         (id: "futa_beam", damage: 300, range: 4096, reload: 30, warhead: "prism_warhead",
//!          anti_air: 1, anti_ground: 1),
//!     ],
//!     buildings: [
//!         (id: "futa", template: "pillbox", label: "Futa", description: "...",
//!          sprite: "power_plant", width: 2, height: 2, cost: 5000, build_time: 1500,
//!          producer: "construction_yard", hit_points: 8000, armour: "concrete", sight: 16,
//!          weapon: "futa_beam", build_radius: 6, unlimited_power: 1, anywhere: 1),
//!     ],
//! )
//! ```
//!
//! Every field is required. A weapon is instant (no projectile, a beam), accurate and fires one
//! shot per volley; `range` is in subcells and `reload` in ticks, `warhead` names a warhead of the
//! combat file, and at least one of `anti_air` and `anti_ground` is 1. A weapon may also be one of
//! the combat file.
//!
//! A building's `template` names a building type of the arsenal whose entry it copies (category,
//! lane, sale and repair, crew and the other traits) and whose combat terms it copies except hit
//! points, armour, sight and its weapon, which it fires at the ground and the air alike as its
//! flags allow. The copy is offered to no side or country (only Super Me opens it), has no
//! prerequisites but its producer and needs no power; no engineer can capture it, no spy enter it,
//! no C4 or Ivan bomb be planted on it, so that no other player can take or cheaply remove what it
//! grants.
//!
//! What it grants its owner, while one stands and its owner has Super Me: `unlimited_power` (1:
//! the owner is never low on power, whatever its buildings and a spy's outage) and `build_radius`
//! (cells, as [`Footprint::distance`](crate::Footprint::distance) measures them, 0 for none:
//! buildings may be placed within that reach of it, as of a base building). `anywhere` (1: it may
//! itself be placed on any free ground of the map, seen or not, however far from its owner's
//! buildings).

use std::path::Path;

use serde::Deserialize;

use crate::building::Footprint;
use crate::combat_defs::{
    CombatTerms, MAX_DAMAGE, MAX_RELOAD, MAX_WEAPON_RANGE, MIN_WEAPON_RANGE, WeaponDef,
};
use crate::data::{self, DataError, DataErrorKind, MAX_DATA_FILE_BYTES, Shape};
use crate::definitions::{Definitions, check_name};
use crate::economy_defs::{
    BuildingDef, BuildingKind, MAX_BUILD_TIME, MAX_BUILDING_TYPES, MAX_COST, MAX_FOOTPRINT_SIDE,
    Production, ProductionItem,
};

/// The schema version of the Super Me file this crate reads.
pub const SUPER_ME_SCHEMA_VERSION: i64 = 2;

/// Largest number of building types, and of weapons, in a Super Me file.
pub const MAX_SUPER_ME_ENTRIES: usize = 4;

/// Largest build radius of a Super Me building, in cells.
pub const MAX_BUILD_RADIUS: u32 = 16;

/// Largest hit points of a Super Me building.
pub const MAX_SUPER_ME_HIT_POINTS: i32 = 20_000;

/// Largest sight of a Super Me building, in cells.
pub const MAX_SUPER_ME_SIGHT: u32 = 16;

/// What a Super Me building type grants and where it may stand (module documentation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SuperMeBuilding {
    pub kind: BuildingKind,
    /// Cells within which its owner may place buildings; 0: none.
    pub build_radius: u32,
    /// While one stands, its owner is never low on power.
    pub unlimited_power: bool,
    /// It may be placed on any free ground, however far from its owner's buildings.
    pub anywhere: bool,
}

static WEAPON_SHAPE: Shape = Shape::Struct(&[
    ("id", Shape::Str),
    ("damage", Shape::Int),
    ("range", Shape::Int),
    ("reload", Shape::Int),
    ("warhead", Shape::Str),
    ("anti_air", Shape::Int),
    ("anti_ground", Shape::Int),
]);

static BUILDING_SHAPE: Shape = Shape::Struct(&[
    ("id", Shape::Str),
    ("template", Shape::Str),
    ("label", Shape::Str),
    ("description", Shape::Str),
    ("sprite", Shape::Str),
    ("width", Shape::Int),
    ("height", Shape::Int),
    ("cost", Shape::Int),
    ("build_time", Shape::Int),
    ("producer", Shape::Str),
    ("hit_points", Shape::Int),
    ("armour", Shape::Str),
    ("sight", Shape::Int),
    ("weapon", Shape::Str),
    ("build_radius", Shape::Int),
    ("unlimited_power", Shape::Int),
    ("anywhere", Shape::Int),
]);

static FILE_SHAPE: Shape = Shape::Struct(&[
    (
        "schema_version",
        Shape::SchemaVersion(SUPER_ME_SCHEMA_VERSION),
    ),
    (
        "weapons",
        Shape::List {
            entry: &WEAPON_SHAPE,
            max: MAX_SUPER_ME_ENTRIES,
        },
    ),
    (
        "buildings",
        Shape::List {
            entry: &BUILDING_SHAPE,
            max: MAX_SUPER_ME_ENTRIES,
        },
    ),
]);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[allow(dead_code)]
    schema_version: i64,
    weapons: Vec<WeaponEntry>,
    buildings: Vec<BuildingEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WeaponEntry {
    id: String,
    damage: i64,
    range: i64,
    reload: i64,
    warhead: String,
    anti_air: i64,
    anti_ground: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildingEntry {
    id: String,
    template: String,
    label: String,
    description: String,
    sprite: String,
    width: i64,
    height: i64,
    cost: i64,
    build_time: i64,
    producer: String,
    hit_points: i64,
    armour: String,
    sight: i64,
    weapon: String,
    build_radius: i64,
    unlimited_power: i64,
    anywhere: i64,
}

impl Definitions {
    /// These definitions together with the Super Me file at `path` (module documentation), whose
    /// templates, producers, warheads, armour classes and weapons are resolved against them. Load
    /// it last ([`load_game_ruleset`](crate::load_game_ruleset) does): it needs the combat file and
    /// the arsenal, and appends to the weapons, the buildings, their combat terms and the arsenal,
    /// which loading one of them again would replace.
    pub fn with_super_me_buildings(mut self, path: impl AsRef<Path>) -> Result<Self, DataError> {
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
        let invalid = |field: String, found: String, expected: String| {
            error(field, DataErrorKind::InvalidValue { found, expected })
        };
        let unknown = |field: String, kind: &str, id: &str| {
            error(
                field,
                DataErrorKind::UnknownReference {
                    kind: kind.to_owned(),
                    id: id.to_owned(),
                },
            )
        };
        let flag = |field: String, value: i64| match value {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(invalid(field, value.to_string(), "0 or 1".into())),
        };

        for (index, entry) in file.weapons.into_iter().enumerate() {
            let field = |name: &str| format!("weapons[{index}].{name}");
            check_name(&entry.id).map_err(|kind| error(field("id"), kind))?;
            if self.weapon_kind(&entry.id).is_some() {
                return Err(error(field("id"), DataErrorKind::DuplicateId(entry.id)));
            }
            let damage = i32::try_from(entry.damage)
                .ok()
                .filter(|damage| (1..=MAX_DAMAGE).contains(damage))
                .ok_or_else(|| {
                    invalid(
                        field("damage"),
                        entry.damage.to_string(),
                        format!("1 to {MAX_DAMAGE}"),
                    )
                })?;
            let range = u32::try_from(entry.range)
                .ok()
                .filter(|range| (MIN_WEAPON_RANGE..=MAX_WEAPON_RANGE).contains(range))
                .ok_or_else(|| {
                    invalid(
                        field("range"),
                        entry.range.to_string(),
                        format!("{MIN_WEAPON_RANGE} to {MAX_WEAPON_RANGE} subcells"),
                    )
                })?;
            let reload = u32::try_from(entry.reload)
                .ok()
                .filter(|reload| (1..=MAX_RELOAD).contains(reload))
                .ok_or_else(|| {
                    invalid(
                        field("reload"),
                        entry.reload.to_string(),
                        format!("1 to {MAX_RELOAD} ticks"),
                    )
                })?;
            let warhead = self
                .warhead_kind(&entry.warhead)
                .ok_or_else(|| unknown(field("warhead"), "warhead", &entry.warhead))?;
            let anti_air = flag(field("anti_air"), entry.anti_air)?;
            let anti_ground = flag(field("anti_ground"), entry.anti_ground)?;
            if !anti_air && !anti_ground {
                return Err(invalid(
                    field("anti_ground"),
                    "0 with anti_air 0".into(),
                    "1 where anti_air is 0".into(),
                ));
            }
            self.push_weapon(WeaponDef {
                id: entry.id,
                damage,
                range,
                min_range: 0,
                reload,
                burst: 1,
                warhead,
                projectile_speed: 0,
                anti_air,
                anti_ground,
                scatter: 0,
                rad_level: 0,
                area_fire: false,
                naval_only: false,
                sonic: false,
                repair: false,
                suicide: false,
            });
        }

        for (index, entry) in file.buildings.into_iter().enumerate() {
            let field = |name: &str| format!("buildings[{index}].{name}");
            check_name(&entry.id).map_err(|kind| error(field("id"), kind))?;
            check_name(&entry.sprite).map_err(|kind| error(field("sprite"), kind))?;
            if self.building_kind(&entry.id).is_some() {
                return Err(error(field("id"), DataErrorKind::DuplicateId(entry.id)));
            }
            if self.buildings().len() >= MAX_BUILDING_TYPES {
                return Err(invalid(
                    field("id"),
                    entry.id,
                    format!("at most {MAX_BUILDING_TYPES} building types in all"),
                ));
            }
            let template = self
                .building_kind(&entry.template)
                .ok_or_else(|| unknown(field("template"), "building", &entry.template))?;
            let (Some(template_entry), Some(template_combat)) = (
                self.metadata(ProductionItem::Building(template)).cloned(),
                self.building_combat(template),
            ) else {
                return Err(invalid(
                    field("template"),
                    entry.template,
                    "a building type with an arsenal entry and combat terms".into(),
                ));
            };
            let side = |name: &str, value: i64| {
                u8::try_from(value)
                    .ok()
                    .filter(|side| (1..=MAX_FOOTPRINT_SIDE).contains(side))
                    .ok_or_else(|| {
                        invalid(
                            field(name),
                            value.to_string(),
                            format!("1 to {MAX_FOOTPRINT_SIDE} cells"),
                        )
                    })
            };
            let (width, height) = (side("width", entry.width)?, side("height", entry.height)?);
            if !(0..=MAX_COST).contains(&entry.cost) {
                return Err(invalid(
                    field("cost"),
                    entry.cost.to_string(),
                    format!("0 to {MAX_COST} credits"),
                ));
            }
            let build_time = u32::try_from(entry.build_time)
                .ok()
                .filter(|time| (1..=MAX_BUILD_TIME).contains(time))
                .ok_or_else(|| {
                    invalid(
                        field("build_time"),
                        entry.build_time.to_string(),
                        format!("1 to {MAX_BUILD_TIME} ticks"),
                    )
                })?;
            let producer = self
                .building_kind(&entry.producer)
                .ok_or_else(|| unknown(field("producer"), "building", &entry.producer))?;
            let hit_points = i32::try_from(entry.hit_points)
                .ok()
                .filter(|hp| (1..=MAX_SUPER_ME_HIT_POINTS).contains(hp))
                .ok_or_else(|| {
                    invalid(
                        field("hit_points"),
                        entry.hit_points.to_string(),
                        format!("1 to {MAX_SUPER_ME_HIT_POINTS}"),
                    )
                })?;
            let armour = self
                .armour_class(&entry.armour)
                .ok_or_else(|| unknown(field("armour"), "armour class", &entry.armour))?;
            let sight = u32::try_from(entry.sight)
                .ok()
                .filter(|sight| *sight <= MAX_SUPER_ME_SIGHT)
                .ok_or_else(|| {
                    invalid(
                        field("sight"),
                        entry.sight.to_string(),
                        format!("0 to {MAX_SUPER_ME_SIGHT} cells"),
                    )
                })?;
            let weapon = self
                .weapon_kind(&entry.weapon)
                .ok_or_else(|| unknown(field("weapon"), "weapon", &entry.weapon))?;
            let build_radius = u32::try_from(entry.build_radius)
                .ok()
                .filter(|radius| *radius <= MAX_BUILD_RADIUS)
                .ok_or_else(|| {
                    invalid(
                        field("build_radius"),
                        entry.build_radius.to_string(),
                        format!("0 to {MAX_BUILD_RADIUS} cells"),
                    )
                })?;
            let unlimited_power = flag(field("unlimited_power"), entry.unlimited_power)?;
            let anywhere = flag(field("anywhere"), entry.anywhere)?;

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
            arsenal.build_limit = 0;
            arsenal.powered = false;
            arsenal.capturable = false;
            arsenal.needs_engineer = false;
            arsenal.spyable = false;
            arsenal.can_c4 = false;
            arsenal.bombable = false;
            arsenal.immune_psionics = true;
            let combat = CombatTerms {
                hit_points,
                armour,
                sight,
                weapon: Some(weapon),
                secondary: None,
                elite: [Some(weapon), None],
                ..template_combat
            };
            let kind = self.push_building(
                BuildingDef {
                    id: entry.id,
                    sprite: entry.sprite,
                    width,
                    height,
                    production: Production {
                        cost: entry.cost,
                        build_time,
                        producer,
                        prerequisites: Vec::new(),
                    },
                    power_produced: 0,
                    power_consumed: 0,
                    sell_value: entry.cost / 2,
                    free_units: Vec::new(),
                    credits_per_ore: None,
                    credits_per_gem: None,
                    fixed_refund: None,
                },
                combat,
                arsenal,
            );
            std::sync::Arc::make_mut(&mut self.super_me_buildings).push(SuperMeBuilding {
                kind,
                build_radius,
                unlimited_power,
                anywhere,
            });
        }
        Ok(self)
    }

    /// The Super Me buildings of the definitions, in the order of their file.
    pub fn super_me_buildings(&self) -> &[SuperMeBuilding] {
        &self.super_me_buildings
    }

    /// What building type `kind` grants if it is a Super Me building.
    pub fn super_me_building(&self, kind: BuildingKind) -> Option<&SuperMeBuilding> {
        self.super_me_buildings
            .iter()
            .find(|building| building.kind == kind)
    }

    /// Whether `item` is a Super Me building, which only a player with Super Me may build.
    pub fn is_super_me_item(&self, item: ProductionItem) -> bool {
        matches!(item, ProductionItem::Building(kind) if self.super_me_building(kind).is_some())
    }
}

/// Whether `footprint` lies within the reach of one of `anchors` (the footprints of a player's
/// Super Me buildings with their build radius, [`PlayerState::build_anchors`]).
///
/// [`PlayerState::build_anchors`]: crate::PlayerState::build_anchors
pub(crate) fn near_anchor(footprint: Footprint, anchors: &[(Footprint, u32)]) -> bool {
    anchors
        .iter()
        .any(|(anchor, radius)| footprint.distance(anchor) <= *radius)
}
