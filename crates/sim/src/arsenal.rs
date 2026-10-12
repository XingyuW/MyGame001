//! Shared, checked production metadata and specialist rules. Legacy scenarios may omit this
//! extension; the playable arsenal explicitly supplies a record for every offered item.
use crate::data::{self, DataError, DataErrorKind, MAX_DATA_FILE_BYTES, Shape};
use crate::hash::Fnv1a64;
use crate::{Definitions, ProductionItem, UnitKind};
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path};

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
]);
static FILE: Shape = Shape::Struct(&[
    ("schema_version", Shape::SchemaVersion(1)),
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
        debug_assert_eq!(file.schema_version, 1);
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
                },
            );
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
