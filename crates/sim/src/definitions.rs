//! Immutable unit definitions loaded from RON (PLAN.md section 5; docs/DECISIONS.md D18).
//!
//! Units file, schema version 1:
//!
//! ```ron
//! (
//!     schema_version: 1,
//!     units: [
//!         (id: "light_tank", speed: 24, sprite: "light_tank"),
//!     ],
//! )
//! ```
//!
//! Every field is required. `id` and `sprite` are names of 1 to 32 characters from `a`-`z`,
//! `0`-`9`, and `_`, starting with a letter; ids are unique. `speed` is in subcells per tick, from
//! 1 to [`MAX_UNIT_SPEED`]. `sprite` names a sprite of the game; `sim` checks only its form, and the
//! game checks that the sprite exists before the match starts (cross validation).

use std::path::Path;

use bevy_ecs::resource::Resource;
use serde::Deserialize;

use crate::combat_defs::{
    self, ArmourClass, CombatDefinitions, CombatTerms, WarheadDef, WarheadKind, WeaponDef,
    WeaponKind,
};
use crate::data::{self, DataError, DataErrorKind, MAX_DATA_FILE_BYTES, Shape};
use crate::economy_defs::{
    self, BuildingDef, BuildingKind, EconomyDefinitions, HarvesterDef, Production, ProductionItem,
};
use crate::hash::Fnv1a64;

/// The schema version of the units file this crate reads.
pub const UNITS_SCHEMA_VERSION: i64 = 1;

/// Largest number of unit types in a units file.
pub const MAX_UNIT_TYPES: usize = 64;

/// Largest unit speed in subcells per tick: half a cell per tick, 10 cells per second.
pub const MAX_UNIT_SPEED: u16 = 128;

/// Longest unit id or sprite name.
pub const MAX_NAME_LEN: usize = 32;

/// Index of a unit type in [`Definitions`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnitKind(pub u16);

/// A unit type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitDef {
    pub id: String,
    /// Subcells per tick; cells per second = speed × 20 / 256.
    pub speed: u16,
    /// Name of the unit's sprite in the game.
    pub sprite: String,
}

/// Immutable definitions of the current match (PLAN.md section 5).
///
/// [`SimPlugin`](crate::SimPlugin) starts with no definitions; match setup installs the loaded
/// ones before units are spawned.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct Definitions {
    units: Vec<UnitDef>,
    pub(crate) arsenal: crate::arsenal::Arsenal,
    /// Empty unless the definitions were loaded with an economy file ([`load_definitions`]).
    economy: EconomyDefinitions,
    /// Empty unless a combat file was loaded ([`Definitions::with_combat`]); then every unit and
    /// building type has combat terms.
    combat: CombatDefinitions,
    /// `None` unless a rules file was loaded ([`Definitions::with_rules`], RA1): the global
    /// values, sides and countries of the Red Alert 2 ruleset. Without it every rule that reads
    /// it keeps its behaviour of M2 to M9.
    rules: Option<crate::rules::Rules>,
}

impl Definitions {
    pub fn units(&self) -> &[UnitDef] {
        &self.units
    }

    pub fn unit(&self, kind: UnitKind) -> Option<&UnitDef> {
        self.units.get(usize::from(kind.0))
    }

    /// The kind of the unit type with this id.
    pub fn unit_kind(&self, id: &str) -> Option<UnitKind> {
        self.units
            .iter()
            .position(|unit| unit.id == id)
            .and_then(|index| u16::try_from(index).ok())
            .map(UnitKind)
    }

    /// The building types, indexed by [`BuildingKind`].
    pub fn buildings(&self) -> &[BuildingDef] {
        &self.economy.buildings
    }

    pub fn building(&self, kind: BuildingKind) -> Option<&BuildingDef> {
        self.economy.buildings.get(usize::from(kind.0))
    }

    /// The kind of the building type with this id.
    pub fn building_kind(&self, id: &str) -> Option<BuildingKind> {
        self.economy
            .buildings
            .iter()
            .position(|building| building.id == id)
            .and_then(|index| u16::try_from(index).ok())
            .map(BuildingKind)
    }

    /// The production terms of `item`, or `None` if it is not defined or cannot be produced.
    pub fn production(&self, item: ProductionItem) -> Option<&Production> {
        match item {
            ProductionItem::Building(kind) => self.building(kind).map(|def| &def.production),
            ProductionItem::Unit(kind) => self
                .economy
                .unit_production
                .get(usize::from(kind.0))
                .and_then(Option::as_ref),
        }
    }

    /// The harvesting terms of `kind`, or `None` if the unit type does not harvest.
    pub fn harvester(&self, kind: UnitKind) -> Option<HarvesterDef> {
        self.economy
            .harvesters
            .get(usize::from(kind.0))
            .copied()
            .flatten()
    }

    /// Whether units leave buildings of type `kind`, which therefore have a rally point: some
    /// unit type is produced by them (or, with factory groups, by a factory of their group:
    /// `Definitions::same_factory`), or they duplicate trained infantry (the arsenal's cloner,
    /// whose copies would otherwise all stand around it).
    pub fn produces_units(&self, kind: BuildingKind) -> bool {
        self.economy
            .unit_production
            .iter()
            .flatten()
            .any(|production| self.same_factory(kind, production.producer))
            || self
                .metadata(ProductionItem::Building(kind))
                .is_some_and(|entry| entry.role == crate::Role::Cloner)
    }

    /// Largest distance in cells between a new building and a building of its owner
    /// (PLAN.md section 7, M3; docs/DECISIONS.md D23).
    pub fn placement_distance(&self) -> u32 {
        self.economy.placement_distance
    }

    /// Whether a combat file was loaded, so that every unit and building type has combat terms.
    pub fn has_combat(&self) -> bool {
        !self.combat.armour_classes.is_empty()
    }

    /// The armour class names, indexed by [`ArmourClass`].
    pub fn armour_classes(&self) -> &[String] {
        &self.combat.armour_classes
    }

    /// The warheads, indexed by [`WarheadKind`].
    pub fn warheads(&self) -> &[WarheadDef] {
        &self.combat.warheads
    }

    pub fn warhead(&self, kind: WarheadKind) -> Option<&WarheadDef> {
        self.combat.warheads.get(usize::from(kind.0))
    }

    /// The weapons, indexed by [`WeaponKind`].
    pub fn weapons(&self) -> &[WeaponDef] {
        &self.combat.weapons
    }

    pub fn weapon(&self, kind: WeaponKind) -> Option<&WeaponDef> {
        self.combat.weapons.get(usize::from(kind.0))
    }

    /// The kind of the weapon with this id.
    pub fn weapon_kind(&self, id: &str) -> Option<WeaponKind> {
        self.combat
            .weapons
            .iter()
            .position(|weapon| weapon.id == id)
            .and_then(|index| u16::try_from(index).ok())
            .map(WeaponKind)
    }

    /// The kind of the armour class with this name.
    pub fn armour_class(&self, name: &str) -> Option<ArmourClass> {
        self.combat
            .armour_classes
            .iter()
            .position(|class| class == name)
            .and_then(|index| u16::try_from(index).ok())
            .map(ArmourClass)
    }

    /// The percentage `warhead` deals against `armour` (equation 2), or `None` if either is not
    /// defined.
    pub fn versus(&self, warhead: WarheadKind, armour: ArmourClass) -> Option<i32> {
        self.warhead(warhead)?
            .versus
            .get(usize::from(armour.0))
            .copied()
    }

    /// The combat terms of unit type `kind`, or `None` without a combat file.
    pub fn unit_combat(&self, kind: UnitKind) -> Option<CombatTerms> {
        self.combat
            .units
            .get(usize::from(kind.0))
            .copied()
            .flatten()
    }

    /// The combat terms of building type `kind`, or `None` without a combat file.
    pub fn building_combat(&self, kind: BuildingKind) -> Option<CombatTerms> {
        self.combat
            .buildings
            .get(usize::from(kind.0))
            .copied()
            .flatten()
    }

    /// The rules file, or `None` for definitions loaded without one.
    pub fn rules(&self) -> Option<&crate::rules::Rules> {
        self.rules.as_ref()
    }

    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        hasher.write_u64(self.units.len() as u64);
        for unit in &self.units {
            for text in [&unit.id, &unit.sprite] {
                hasher.write_u64(text.len() as u64);
                hasher.write(text.as_bytes());
            }
            hasher.write(&unit.speed.to_le_bytes());
        }
        self.arsenal.hash_into(hasher);
        self.economy.hash_into(hasher);
        self.combat.hash_into(hasher);
        match &self.rules {
            None => hasher.write(&[0]),
            Some(rules) => {
                hasher.write(&[1]);
                rules.hash_into(hasher);
            }
        }
    }
}

static UNIT_SHAPE: Shape = Shape::Struct(&[
    ("id", Shape::Str),
    ("speed", Shape::Int),
    ("sprite", Shape::Str),
]);

static UNITS_FILE_SHAPE: Shape = Shape::Struct(&[
    ("schema_version", Shape::SchemaVersion(UNITS_SCHEMA_VERSION)),
    (
        "units",
        Shape::List {
            entry: &UNIT_SHAPE,
            max: MAX_UNIT_TYPES,
        },
    ),
]);

/// The units file as `ron` reads it. Integers are read as `i64`, the widest type the checker
/// admits, so that range errors are reported by [`validate`] with their field.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnitsFile {
    schema_version: i64,
    units: Vec<UnitEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnitEntry {
    id: String,
    speed: i64,
    sprite: String,
}

/// Loads and validates the units file at `path`.
///
/// The file is read with a size bound, checked against the supported subset of RON and the
/// units schema (`data`, invariant I11), parsed by `ron` from exactly the checked text, and
/// validated; every problem is a [`DataError`] naming the file, the field, and the reason.
pub fn load_unit_definitions(path: impl AsRef<Path>) -> Result<Definitions, DataError> {
    let path = path.as_ref();
    let bytes = data::read_bounded(path, MAX_DATA_FILE_BYTES).map_err(|kind| DataError {
        file: path.to_owned(),
        field: "file".into(),
        kind,
    })?;
    let text = data::check(path, &bytes, &UNITS_FILE_SHAPE)?;
    let file: UnitsFile = data::parse(path, text)?;
    validate(path, file)
}

/// Loads the units file at `units_file` ([`load_unit_definitions`]) and the economy file at
/// `economy_file`, whose references to unit types are resolved against the units file
/// (docs/DECISIONS.md D23). Every problem is a [`DataError`] naming the file, the field, and the
/// reason.
pub fn load_definitions(
    units_file: impl AsRef<Path>,
    economy_file: impl AsRef<Path>,
) -> Result<Definitions, DataError> {
    load_unit_definitions(units_file)?.with_economy(economy_file)
}

impl Definitions {
    /// These unit definitions together with the economy file at `economy_file`, whose
    /// references to unit types are resolved against them ([`load_definitions`] in two steps,
    /// for a caller that has loaded the units file already).
    ///
    /// Combat definitions refer to the building types of the economy file, so loading an economy
    /// file removes any combat definitions loaded before; load the combat file afterwards
    /// ([`Definitions::with_combat`]). The same holds for the rules and the arsenal.
    pub fn with_economy(mut self, economy_file: impl AsRef<Path>) -> Result<Self, DataError> {
        let ids: Vec<String> = self.units.iter().map(|unit| unit.id.clone()).collect();
        self.economy = economy_defs::load(economy_file.as_ref(), &ids)?;
        self.combat = CombatDefinitions::default();
        self.rules = None;
        self.arsenal = crate::arsenal::Arsenal::default();
        Ok(self)
    }

    /// These definitions together with the rules file at `rules_file` (RA1, `crate::rules`),
    /// whose references to unit and building types are resolved against them. The arsenal names
    /// the countries and prerequisite groups of the rules, so loading a rules file removes an
    /// arsenal loaded before; load the arsenal afterwards ([`Definitions::with_arsenal`]). Every
    /// problem is a [`DataError`] naming the file, the field, and the reason.
    pub fn with_rules(mut self, rules_file: impl AsRef<Path>) -> Result<Self, DataError> {
        self.rules = Some(crate::rules::load(rules_file.as_ref(), &self)?);
        self.arsenal = crate::arsenal::Arsenal::default();
        Ok(self)
    }

    /// These definitions together with the combat file at `combat_file`, whose references to
    /// unit and building types are resolved against them, and which must give every unit type
    /// and every building type combat terms (docs/DECISIONS.md D28). Every problem is a
    /// [`DataError`] naming the file, the field, and the reason.
    pub fn with_combat(mut self, combat_file: impl AsRef<Path>) -> Result<Self, DataError> {
        let units: Vec<String> = self.units.iter().map(|unit| unit.id.clone()).collect();
        let buildings: Vec<String> = self
            .economy
            .buildings
            .iter()
            .map(|building| building.id.clone())
            .collect();
        self.combat = combat_defs::load(combat_file.as_ref(), &units, &buildings)?;
        Ok(self)
    }
}

/// Loads the units file, the economy file, and the combat file of a match ([`load_definitions`],
/// then [`Definitions::with_combat`]).
pub fn load_match_definitions(
    units_file: impl AsRef<Path>,
    economy_file: impl AsRef<Path>,
    combat_file: impl AsRef<Path>,
) -> Result<Definitions, DataError> {
    load_definitions(units_file, economy_file)?.with_combat(combat_file)
}

fn validate(path: &Path, file: UnitsFile) -> Result<Definitions, DataError> {
    let error = |field: String, kind| DataError {
        file: path.to_owned(),
        field,
        kind,
    };
    if file.schema_version != UNITS_SCHEMA_VERSION {
        return Err(error(
            "schema_version".into(),
            DataErrorKind::UnsupportedSchemaVersion {
                found: file.schema_version,
                expected: UNITS_SCHEMA_VERSION,
            },
        ));
    }
    if file.units.is_empty() {
        return Err(error(
            "units".into(),
            DataErrorKind::InvalidValue {
                found: "an empty list".into(),
                expected: format!("1 to {MAX_UNIT_TYPES} unit types"),
            },
        ));
    }
    let mut units: Vec<UnitDef> = Vec::with_capacity(file.units.len());
    for (index, entry) in file.units.into_iter().enumerate() {
        let field = |name: &str| format!("units[{index}].{name}");
        check_name(&entry.id).map_err(|kind| error(field("id"), kind))?;
        if units.iter().any(|unit| unit.id == entry.id) {
            return Err(error(field("id"), DataErrorKind::DuplicateId(entry.id)));
        }
        let speed = u16::try_from(entry.speed)
            .ok()
            .filter(|speed| (1..=MAX_UNIT_SPEED).contains(speed))
            .ok_or_else(|| {
                error(
                    field("speed"),
                    DataErrorKind::InvalidValue {
                        found: entry.speed.to_string(),
                        expected: format!("1 to {MAX_UNIT_SPEED} subcells per tick"),
                    },
                )
            })?;
        check_name(&entry.sprite).map_err(|kind| error(field("sprite"), kind))?;
        units.push(UnitDef {
            id: entry.id,
            speed,
            sprite: entry.sprite,
        });
    }
    Ok(Definitions {
        units,
        economy: EconomyDefinitions::default(),
        combat: CombatDefinitions::default(),
        arsenal: crate::arsenal::Arsenal::default(),
        rules: None,
    })
}

/// Unit ids and sprite names: 1 to [`MAX_NAME_LEN`] characters from `a`-`z`, `0`-`9`, and `_`,
/// starting with a letter. Sprite names become file names, so nothing else is allowed.
pub(crate) fn check_name(name: &str) -> Result<(), DataErrorKind> {
    let valid = (1..=MAX_NAME_LEN).contains(&name.len())
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    if valid {
        Ok(())
    } else {
        Err(DataErrorKind::InvalidValue {
            found: format!("{name:?}"),
            expected: format!(
                "1 to {MAX_NAME_LEN} characters from a-z, 0-9, and _, starting with a letter"
            ),
        })
    }
}
