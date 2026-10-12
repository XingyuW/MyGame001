//! Shared, checked production metadata and specialist rules. Legacy scenarios may omit this
//! extension; the playable arsenal explicitly supplies a record for every offered item.
//!
//! Schema version 2 (RA1, docs/RA2_PARITY_PLAN.md section 3.3; docs/DECISIONS.md R10, R11) adds
//! to every entry what Red Alert 2 needs to decide who may build it, and whose art it shows:
//!
//! * `art`: the id of the item whose miniature and sprites the game draws for it (its own id, or
//!   that of its twin of the other side until it has art of its own);
//! * `side`: `"allied"`, `"soviet"` or `"none"`, the side of the type itself; for a factory it
//!   is the side the `Owner` rule looks at (assumption RA-A3);
//! * `factory`: for a building, the kind of factory it is (`"buildings"`, `"infantry"`,
//!   `"vehicles"`, `"naval"`, `"aircraft"`) or `"none"`; an item's producer in the economy file
//!   then means "a building of the producer's factory group";
//! * `owner_sides`: the sides whose factories build the item (the ini's `Owner`); empty for an
//!   item any factory of its group builds;
//! * `required_countries`, `forbidden_countries`: the ini's `RequiredHouses` and
//!   `ForbiddenHouses`, as country ids of the rules file;
//! * `tech_level`: the ini's `TechLevel`; an item below 0 or above the rules' tech level cannot
//!   be queued;
//! * `prerequisite_groups`: groups of the rules file of which the player must own one member
//!   (the ini's `POWER`, `RADAR`, ... tokens), besides the buildings the economy file lists.
//!
//! Countries and prerequisite groups are those of the rules file, so the rules are loaded first
//! ([`Definitions::with_rules`]); an entry that names one without rules is rejected. With rules,
//! the base unit of every side must have the role `mcv`, since that role is what deploys.
use crate::data::{self, DataError, DataErrorKind, MAX_DATA_FILE_BYTES, Shape};
use crate::hash::Fnv1a64;
use crate::rules::{
    CountryId, GroupId, MAX_COUNTRIES, MAX_PREREQUISITE_GROUPS, MAX_TECH_LEVEL, Side,
};
use crate::{BuildingKind, Definitions, ProductionItem, UnitKind};
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path};

/// The schema version of the arsenal file this crate reads (2 since RA1).
pub const ARSENAL_SCHEMA_VERSION: i64 = 2;

/// The kind of factory a building is (docs/RA2_PARITY_PLAN.md section 3.3): buildings of one
/// group produce the same items and share a queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FactoryGroup {
    Buildings,
    Infantry,
    Vehicles,
    Naval,
    Aircraft,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Domain {
    #[default]
    Land,
    Air,
    Naval,
    Amphibious,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Category {
    #[default]
    Buildings,
    Armory,
    Infantry,
    Vehicles,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Role {
    #[default]
    Combat,
    Engineer,
    Dog,
    Spy,
    Commando,
    Chrono,
    Flak,
    Tesla,
    Demolition,
    MindControl,
    Radiation,
    Suicide,
    ChronoMiner,
    Transport,
    Ifv,
    Drone,
    Mcv,
    Mirage,
    Splash,
    Strike,
    Carrier,
    Squid,
    Shipyard,
    Purifier,
    Repair,
    Cloner,
    Gap,
    Satellite,
    Sensor,
    Chronosphere,
    Storm,
    Curtain,
    Nuke,
}
impl Role {
    pub fn superweapon(self) -> bool {
        matches!(
            self,
            Self::Chronosphere | Self::Storm | Self::Curtain | Self::Nuke
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArsenalEntry {
    pub label: String,
    pub description: String,
    pub category: Category,
    pub domain: Domain,
    pub role: Role,
    pub cooldown: u32,
    pub radius: u32,
    pub strength: i32,
    pub capacity: usize,
    /// The id of the item whose art the game draws for this one.
    pub art: String,
    pub side: Option<Side>,
    /// For a building: the kind of factory it is.
    pub factory: Option<FactoryGroup>,
    /// The sides whose factories build the item; empty: any factory of its group.
    pub owner_sides: Vec<Side>,
    pub required_countries: Vec<CountryId>,
    pub forbidden_countries: Vec<CountryId>,
    /// -1 to [`MAX_TECH_LEVEL`].
    pub tech_level: i32,
    pub prerequisite_groups: Vec<GroupId>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Arsenal(pub BTreeMap<ProductionItem, ArsenalEntry>);
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    schema_version: i64,
    entries: Vec<Entry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: String,
    label: String,
    description: String,
    category: String,
    domain: String,
    role: String,
    cooldown: i64,
    radius: i64,
    strength: i64,
    capacity: i64,
    art: String,
    side: String,
    factory: String,
    owner_sides: Vec<String>,
    required_countries: Vec<String>,
    forbidden_countries: Vec<String>,
    tech_level: i64,
    prerequisite_groups: Vec<String>,
}
static ENTRY: Shape = Shape::Struct(&[
    ("id", Shape::Str),
    ("label", Shape::Str),
    ("description", Shape::Str),
    ("category", Shape::Str),
    ("domain", Shape::Str),
    ("role", Shape::Str),
    ("cooldown", Shape::Int),
    ("radius", Shape::Int),
    ("strength", Shape::Int),
    ("capacity", Shape::Int),
    ("art", Shape::Str),
    ("side", Shape::Str),
    ("factory", Shape::Str),
    (
        "owner_sides",
        Shape::List {
            entry: &Shape::Str,
            max: 2,
        },
    ),
    (
        "required_countries",
        Shape::List {
            entry: &Shape::Str,
            max: MAX_COUNTRIES,
        },
    ),
    (
        "forbidden_countries",
        Shape::List {
            entry: &Shape::Str,
            max: MAX_COUNTRIES,
        },
    ),
    ("tech_level", Shape::Int),
    (
        "prerequisite_groups",
        Shape::List {
            entry: &Shape::Str,
            max: MAX_PREREQUISITE_GROUPS,
        },
    ),
]);
static FILE: Shape = Shape::Struct(&[
    (
        "schema_version",
        Shape::SchemaVersion(ARSENAL_SCHEMA_VERSION),
    ),
    (
        "entries",
        Shape::List {
            entry: &ENTRY,
            max: 128,
        },
    ),
]);
impl Arsenal {
    pub(crate) fn load(path: &Path, defs: &Definitions) -> Result<Self, DataError> {
        let err = |field: String, kind| DataError {
            file: path.to_owned(),
            field,
            kind,
        };
        let bytes =
            data::read_bounded(path, MAX_DATA_FILE_BYTES).map_err(|e| err("file".into(), e))?;
        let file: File = data::parse(path, data::check(path, &bytes, &FILE)?)?;
        debug_assert_eq!(file.schema_version, ARSENAL_SCHEMA_VERSION);
        let mut result = BTreeMap::new();
        for (i, e) in file.entries.into_iter().enumerate() {
            let bad = |field: &str, value: &str| {
                err(
                    format!("entries[{i}].{field}"),
                    DataErrorKind::InvalidValue {
                        found: value.into(),
                        expected: "a supported arsenal value".into(),
                    },
                )
            };
            let category = match e.category.as_str() {
                "buildings" => Category::Buildings,
                "armory" => Category::Armory,
                "infantry" => Category::Infantry,
                "vehicles" => Category::Vehicles,
                _ => return Err(bad("category", &e.category)),
            };
            let item = if matches!(category, Category::Buildings | Category::Armory) {
                defs.building_kind(&e.id).map(ProductionItem::Building)
            } else {
                defs.unit_kind(&e.id).map(ProductionItem::Unit)
            }
            .ok_or_else(|| bad("id", &e.id))?;
            if defs.production(item).is_none() || result.contains_key(&item) {
                return Err(bad("id", &e.id));
            }
            let domain = match e.domain.as_str() {
                "land" => Domain::Land,
                "air" => Domain::Air,
                "naval" => Domain::Naval,
                "amphibious" => Domain::Amphibious,
                _ => return Err(bad("domain", &e.domain)),
            };
            let role = match e.role.as_str() {
                "combat" => Role::Combat,
                "engineer" => Role::Engineer,
                "dog" => Role::Dog,
                "spy" => Role::Spy,
                "commando" => Role::Commando,
                "chrono" => Role::Chrono,
                "flak" => Role::Flak,
                "tesla" => Role::Tesla,
                "demolition" => Role::Demolition,
                "mind_control" => Role::MindControl,
                "radiation" => Role::Radiation,
                "suicide" => Role::Suicide,
                "chrono_miner" => Role::ChronoMiner,
                "transport" => Role::Transport,
                "ifv" => Role::Ifv,
                "drone" => Role::Drone,
                "mcv" => Role::Mcv,
                "mirage" => Role::Mirage,
                "splash" => Role::Splash,
                "strike" => Role::Strike,
                "carrier" => Role::Carrier,
                "squid" => Role::Squid,
                "shipyard" => Role::Shipyard,
                "purifier" => Role::Purifier,
                "repair" => Role::Repair,
                "cloner" => Role::Cloner,
                "gap" => Role::Gap,
                "satellite" => Role::Satellite,
                "sensor" => Role::Sensor,
                "chronosphere" => Role::Chronosphere,
                "storm" => Role::Storm,
                "curtain" => Role::Curtain,
                "nuke" => Role::Nuke,
                _ => return Err(bad("role", &e.role)),
            };
            if (matches!(item, ProductionItem::Building(_)) && domain != Domain::Land)
                || (role.superweapon() && matches!(item, ProductionItem::Unit(_)))
            {
                return Err(bad("role", "role/domain incompatible with entity type"));
            }
            for (name, n, max) in [
                ("cooldown", e.cooldown, 72000),
                ("radius", e.radius, 32),
                ("strength", e.strength, 10000),
                ("capacity", e.capacity, 16),
            ] {
                if !(0..=max).contains(&n) {
                    return Err(bad(name, &n.to_string()));
                }
            }
            if role.superweapon() && (e.cooldown == 0 || e.radius == 0) {
                return Err(bad("cooldown", "zero superweapon timing/radius"));
            }
            crate::definitions::check_name(&e.art)
                .map_err(|kind| err(format!("entries[{i}].art"), kind))?;
            let side = match e.side.as_str() {
                "none" => None,
                other => Some(Side::parse(other).ok_or_else(|| bad("side", &e.side))?),
            };
            let factory = match e.factory.as_str() {
                "none" => None,
                "buildings" => Some(FactoryGroup::Buildings),
                "infantry" => Some(FactoryGroup::Infantry),
                "vehicles" => Some(FactoryGroup::Vehicles),
                "naval" => Some(FactoryGroup::Naval),
                "aircraft" => Some(FactoryGroup::Aircraft),
                _ => return Err(bad("factory", &e.factory)),
            };
            if factory.is_some() && matches!(item, ProductionItem::Unit(_)) {
                return Err(bad("factory", "a factory group on a unit type"));
            }
            let mut owner_sides = Vec::with_capacity(e.owner_sides.len());
            for (n, id) in e.owner_sides.iter().enumerate() {
                let at = format!("entries[{i}].owner_sides[{n}]");
                let side = Side::parse(id).ok_or_else(|| {
                    err(
                        at.clone(),
                        DataErrorKind::InvalidValue {
                            found: format!("{id:?}"),
                            expected: "\"allied\" or \"soviet\"".into(),
                        },
                    )
                })?;
                if owner_sides.contains(&side) {
                    return Err(err(at, DataErrorKind::DuplicateId(id.clone())));
                }
                owner_sides.push(side);
            }
            let countries = |name: &str, ids: &[String]| -> Result<Vec<CountryId>, DataError> {
                let mut found = Vec::with_capacity(ids.len());
                for (n, id) in ids.iter().enumerate() {
                    let at = format!("entries[{i}].{name}[{n}]");
                    let country = defs
                        .rules()
                        .and_then(|rules| rules.country_id(id))
                        .ok_or_else(|| {
                            err(
                                at.clone(),
                                DataErrorKind::UnknownReference {
                                    kind: "country".into(),
                                    id: id.clone(),
                                },
                            )
                        })?;
                    if found.contains(&country) {
                        return Err(err(at, DataErrorKind::DuplicateId(id.clone())));
                    }
                    found.push(country);
                }
                Ok(found)
            };
            let required_countries = countries("required_countries", &e.required_countries)?;
            let forbidden_countries = countries("forbidden_countries", &e.forbidden_countries)?;
            if let Some(n) = forbidden_countries
                .iter()
                .position(|country| required_countries.contains(country))
            {
                return Err(err(
                    format!("entries[{i}].forbidden_countries[{n}]"),
                    DataErrorKind::InvalidValue {
                        found: format!("{:?}", e.forbidden_countries[n]),
                        expected: "a country that is not also required".into(),
                    },
                ));
            }
            if !(-1..=i64::from(MAX_TECH_LEVEL)).contains(&e.tech_level) {
                return Err(bad("tech_level", &e.tech_level.to_string()));
            }
            let mut prerequisite_groups = Vec::with_capacity(e.prerequisite_groups.len());
            for (n, id) in e.prerequisite_groups.iter().enumerate() {
                let at = format!("entries[{i}].prerequisite_groups[{n}]");
                let group = defs
                    .rules()
                    .and_then(|rules| rules.group_id(id))
                    .ok_or_else(|| {
                        err(
                            at.clone(),
                            DataErrorKind::UnknownReference {
                                kind: "prerequisite group".into(),
                                id: id.clone(),
                            },
                        )
                    })?;
                if prerequisite_groups.contains(&group) {
                    return Err(err(at, DataErrorKind::DuplicateId(id.clone())));
                }
                prerequisite_groups.push(group);
            }
            result.insert(
                item,
                ArsenalEntry {
                    label: e.label,
                    description: e.description,
                    category,
                    domain,
                    role,
                    cooldown: e.cooldown as u32,
                    radius: e.radius as u32,
                    strength: e.strength as i32,
                    capacity: e.capacity as usize,
                    art: e.art,
                    side,
                    factory,
                    owner_sides,
                    required_countries,
                    forbidden_countries,
                    tech_level: e.tech_level as i32,
                    prerequisite_groups,
                },
            );
        }
        // The base unit of a side deploys into its construction yard through the role `mcv`
        // (`crate::special`), so the rules' base units must have it.
        for side in defs.rules().map_or(&[][..], |rules| rules.sides.as_slice()) {
            let base = ProductionItem::Unit(side.base_unit);
            if result
                .get(&base)
                .is_none_or(|entry| entry.role != Role::Mcv)
            {
                return Err(err(
                    "entries".into(),
                    DataErrorKind::MissingEntry {
                        kind: format!(
                            "base unit of side {:?} with the role \"mcv\"",
                            side.side.id()
                        ),
                        id: defs
                            .unit(side.base_unit)
                            .map_or_else(String::new, |unit| unit.id.clone()),
                    },
                ));
            }
        }
        Ok(Self(result))
    }
    pub(crate) fn hash_into(&self, h: &mut Fnv1a64) {
        h.write_u64(self.0.len() as u64);
        for (item, e) in &self.0 {
            let (tag, k) = match item {
                ProductionItem::Building(k) => (0, k.0),
                ProductionItem::Unit(k) => (1, k.0),
            };
            h.write(&[tag, e.category as u8, e.domain as u8, e.role as u8]);
            h.write(&k.to_le_bytes());
            for s in [&e.label, &e.description] {
                h.write_u64(s.len() as u64);
                h.write(s.as_bytes());
            }
            h.write_u32(e.cooldown);
            h.write_u32(e.radius);
            h.write(&e.strength.to_le_bytes());
            h.write_u64(e.capacity as u64);
            h.write_u64(e.art.len() as u64);
            h.write(e.art.as_bytes());
            h.write(&[
                e.side.map_or(u8::MAX, |side| side.index() as u8),
                e.factory.map_or(u8::MAX, |factory| factory as u8),
            ]);
            h.write_u64(e.owner_sides.len() as u64);
            for side in &e.owner_sides {
                h.write(&[side.index() as u8]);
            }
            for countries in [&e.required_countries, &e.forbidden_countries] {
                h.write_u64(countries.len() as u64);
                for country in countries {
                    h.write(&[country.0]);
                }
            }
            h.write(&e.tech_level.to_le_bytes());
            h.write_u64(e.prerequisite_groups.len() as u64);
            for group in &e.prerequisite_groups {
                h.write(&[group.0]);
            }
        }
    }
}
impl Definitions {
    pub fn with_arsenal(mut self, path: impl AsRef<Path>) -> Result<Self, DataError> {
        self.arsenal = Arsenal::load(path.as_ref(), &self)?;
        Ok(self)
    }
    pub fn prerequisite_met(
        &self,
        required: crate::BuildingKind,
        owned: impl Iterator<Item = crate::BuildingKind>,
    ) -> bool {
        // Basic generators are interchangeable (a Tesla Reactor satisfies "Power Plant" and vice
        // versa), but an advanced generator with prerequisites of its own (the Nuclear Reactor
        // behind the Battle Lab) is a real tech requirement, not satisfied by any power plant.
        let power = self.building(required).is_some_and(|b| {
            b.power_produced > 0
                && b.production.producer != required
                && b.production.prerequisites.is_empty()
        });
        owned.into_iter().any(|k| {
            k == required
                || (power
                    && self
                        .building(k)
                        .is_some_and(|b| b.power_produced > 0 && b.production.producer != k))
        })
    }
    pub fn arsenal(&self) -> &Arsenal {
        &self.arsenal
    }
    /// The kind of factory building type `kind` is, or `None` for a building without the trait.
    pub fn factory_group(&self, kind: BuildingKind) -> Option<FactoryGroup> {
        self.metadata(ProductionItem::Building(kind))
            .and_then(|entry| entry.factory)
    }
    /// Whether a building of type `owned` counts as the producer `producer` of an item: it is
    /// that type, or both are factories of one group (docs/RA2_PARITY_PLAN.md section 3.3). A
    /// building without a factory group is its own group, which is the rule of M3.
    pub fn same_factory(&self, owned: BuildingKind, producer: BuildingKind) -> bool {
        owned == producer
            || self
                .factory_group(producer)
                .is_some_and(|group| self.factory_group(owned) == Some(group))
    }
    /// The building type under which a player's queue of the items produced by `producer` is
    /// kept ([`PlayerState::queue`](crate::PlayerState::queue)): the first building type of the
    /// producer's factory group, so that all factories of a group share one queue, or `producer`
    /// itself without a group.
    pub fn queue_key(&self, producer: BuildingKind) -> BuildingKind {
        let Some(group) = self.factory_group(producer) else {
            return producer;
        };
        self.arsenal
            .0
            .iter()
            .find_map(|(item, entry)| match item {
                ProductionItem::Building(kind) if entry.factory == Some(group) => Some(*kind),
                _ => None,
            })
            .unwrap_or(producer)
    }
    /// The id of the item whose art the game draws for `item`: the arsenal's `art`, or the
    /// item's own id without an arsenal entry.
    pub fn art(&self, item: ProductionItem) -> &str {
        match self.metadata(item) {
            Some(entry) => &entry.art,
            None => match item {
                ProductionItem::Building(kind) => self.building(kind).map_or("", |def| &def.id),
                ProductionItem::Unit(kind) => self.unit(kind).map_or("", |def| &def.id),
            },
        }
    }
    pub fn metadata(&self, item: ProductionItem) -> Option<&ArsenalEntry> {
        self.arsenal.0.get(&item)
    }
    pub fn domain(&self, kind: UnitKind) -> Domain {
        self.metadata(ProductionItem::Unit(kind))
            .map_or(Domain::Land, |e| e.domain)
    }
    pub fn category(&self, item: ProductionItem) -> Category {
        self.metadata(item).map_or(
            match item {
                // Armed structures outside the arsenal (the factions' gun turret and rocket
                // tower) are defences, listed with the arsenal's defences.
                ProductionItem::Building(k)
                    if self.building_combat(k).is_some_and(|c| c.weapon.is_some()) =>
                {
                    Category::Armory
                }
                ProductionItem::Building(_) => Category::Buildings,
                ProductionItem::Unit(k) => {
                    if self
                        .production(ProductionItem::Unit(k))
                        .and_then(|p| self.building(p.producer))
                        .is_some_and(|b| b.id == "barracks")
                    {
                        Category::Infantry
                    } else {
                        Category::Vehicles
                    }
                }
            },
            |e| e.category,
        )
    }
}
