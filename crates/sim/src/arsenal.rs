//! Shared, checked production metadata and specialist rules. Legacy scenarios may omit this
//! extension; the playable arsenal explicitly supplies a record for every offered item.
//!
//! Schema version 2 (RA1, docs/RA2_PARITY_PLAN.md section 3.3; docs/DECISIONS.md R10, R11) adds
//! to every entry what Red Alert 2 needs to decide who may build it, and whose art it shows:
//!
//! * `art`: the id of the item whose miniature and sprites the game draws for it (its own id, or
//!   that of its twin of the other side until it has art of its own): a defined item of the
//!   same kind, a building for a building and a unit for a unit;
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
//! Schema version 3 (RA2, docs/RA2_PARITY_PLAN.md sections 4.2 and 4.6; docs/DECISIONS.md R22)
//! adds the traits of a building that production, power, placement, selling and repair read. A
//! unit has 0 in every one of them:
//!
//! * `build_limit`: how many buildings of the type a player may own and have queued together
//!   (the ini's `BuildLimit`); 0: any number;
//! * `adjacent`: the free cells allowed between a new building of the type and a base building
//!   of its owner (the ini's `Adjacent`: 0 means touching), 0 to [`MAX_PLACEMENT_DISTANCE`];
//! * and as 1 or 0: `base_normal` (other buildings may be placed beside it, `BaseNormal`),
//!   `radar` (`Radar`), `sellable` (no `Wall`, no `Unsellable`), `repairable` (`Repairable`),
//!   `powered` (`Powered`: under low power it stops working if it consumes power),
//!   `is_powered` (the `IsPowered` of its superweapon: the charge waits under low power), and
//!   `unit_repair` (`UnitRepair`: a damaged vehicle sent to it is repaired for credits).
//!
//! The traits are read only by a world with a rules file; without one the rules of M3 to M9
//! apply unchanged (docs/RA2_PARITY_PLAN.md section 3.2). The category of a building is also its
//! production lane there: `"armory"` (the ini's `BuildCat=Combat`) is the defence lane of the
//! Construction Yard ([`Definitions::lane`]).
//!
//! Schema version 4 (RA3, docs/RA2_PARITY_PLAN.md section 5, RA3 items 6 and 8;
//! docs/DECISIONS.md R6) adds the traits of a unit that movement reads. A building has 0 in
//! every one of them:
//!
//! * `infantry` (1 or 0): the unit is of the ini's infantry type list. An infantry unit that does
//!   not fly is on foot ([`Definitions::on_foot`]): it holds one of the three spots of a cell
//!   instead of the cell (invariant I6'). Only an item of the category `"infantry"` may have it;
//! * `crushable` (1 or 0, the ini's `Crushable`): a crusher of an enemy may drive over the unit
//!   while it is on foot. Only an `infantry` unit may have it;
//! * `crusher` (1 or 0, the ini's `Crusher`): on a `Move` or `AttackMove` command the unit
//!   drives over hostile crushable infantry in its way, which die. Only a unit of the category
//!   `"vehicles"` that does not fly may have it;
//! * `size`: the room the unit takes in a transport (the ini's `Size`), 0 to [`MAX_UNIT_SIZE`].
//!   Nothing reads it yet; it is ported with the other three for the transports of RA4.
//!
//! Unlike the traits of schema version 3 these are read in every world, with or without a rules
//! file: a world whose arsenal gives no unit the `infantry` trait, like a world without an
//! arsenal, has one unit to a cell as before RA3.
//!
//! Schema version 5 (RA4, docs/RA2_PARITY_PLAN.md sections 4.5, 4.7 and 4.8; docs/DECISIONS.md
//! R46) adds the traits of veterancy, crews, aircraft, spawned units and submerged units:
//!
//! * `trainable` (1 or 0, the ini's `Trainable`): units of the type gain experience and ranks.
//!   `veteran_abilities` and `elite_abilities` list what a rank grants (`VeteranAbilities`,
//!   `EliteAbilities`): `"stronger"`, `"firepower"`, `"rof"`, `"sight"`, `"faster"`,
//!   `"self_heal"`, `"scatter"`, each at most once in a list. Units only;
//! * `crewed` (1 or 0, `Crewed`): a destroyed vehicle may release one crew member, a destroyed
//!   or sold building releases survivors;
//! * `self_healing` (1 or 0, `SelfHealing`): the unit regains hit points on its own;
//! * `ammo` (`Ammo`, 0 to [`MAX_AMMO`]; 0: unlimited): the volleys an aircraft fires before it
//!   must reload, and `airport_bound` (1 or 0, `AirportBound`): it needs a dock of a building
//!   with `docks` to be produced, lands there to reload, and is lost without one. Only an
//!   aircraft with `ammo` may be `airport_bound`;
//! * `docks` (`NumberOfDocks` of a building that an `AirportBound` aircraft names as its
//!   `Dock`, 0 to [`MAX_DOCKS`] and at most the cells of its footprint): the aircraft it holds;
//! * `spawns` (the id of a unit type or `"none"`, `Spawns`), `spawns_number`
//!   (`SpawnsNumber`, 1 to [`MAX_SPAWNS`]), `spawn_regen` and `spawn_reload` (ticks,
//!   `SpawnRegenRate` and `SpawnReloadRate`) and `launcher` (`"primary"`, `"secondary"` or
//!   `"none"`: which of the type's weapons has `Spawner=yes`): the unit launches units of the
//!   type `spawns`, which fly and may be shot down, with that weapon. The spawned type is an
//!   aircraft with an entry of its own that spawns nothing;
//! * `missile` (`"v3"`, `"dmisl"` or `"none"`: the type is the ini's `V3RocketType` or
//!   `DMislType`): a spawned unit of the type flies to its target and goes off there with the
//!   combat file's effect of that name, or its elite effect if its launcher is elite;
//! * `cloakable` (1 or 0, `Cloakable`): a player sees the unit only while one of its units or
//!   buildings with sensors is near enough, or shortly after the unit fired;
//! * `sensors_sight` (cells, `SensorsSight` of a type with `Sensors=yes`; 0: no sensors, at
//!   most [`MAX_SIGHT`](crate::MAX_SIGHT)).
//!
//! They are read only by a world with a rules file. A type without any of them, like every type
//! of the fixtures of M2 to RA3, is as before RA4.
//!
//! Countries and prerequisite groups are those of the rules file, so the rules are loaded first
//! ([`Definitions::with_rules`]); an entry that names one without rules is rejected. With rules,
//! the base unit of every side must have the role `mcv`, since that role is what deploys.
use crate::data::{self, DataError, DataErrorKind, MAX_DATA_FILE_BYTES, Shape};
use crate::economy_defs::MAX_PLACEMENT_DISTANCE;
use crate::hash::Fnv1a64;
use crate::rules::{
    CountryId, GroupId, MAX_COUNTRIES, MAX_PREREQUISITE_GROUPS, MAX_TECH_LEVEL, Side,
};
use crate::{BuildingKind, Definitions, ProductionItem, UnitKind};
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path};

/// The schema version of the arsenal file this crate reads (2 since RA1, 3 since RA2, 4 since
/// RA3, 5 since RA4).
pub const ARSENAL_SCHEMA_VERSION: i64 = 7;

/// Largest `build_limit` of an arsenal entry.
pub const MAX_BUILD_LIMIT: u32 = 255;

/// Largest `size` of an arsenal entry.
pub const MAX_UNIT_SIZE: u32 = 255;

/// Largest `ammo` of an arsenal entry.
pub const MAX_AMMO: u32 = 255;

/// Largest `docks` of an arsenal entry.
pub const MAX_DOCKS: u32 = 16;

/// Largest `spawns_number` of an arsenal entry.
pub const MAX_SPAWNS: u32 = 8;

/// Longest `spawn_regen` and `spawn_reload` of an arsenal entry, in ticks (one hour).
pub const MAX_SPAWN_TICKS: u32 = 72_000;

/// What a rank grants a unit (RA4, docs/RA2_PARITY_PLAN.md section 4.5): a token of the ini's
/// `VeteranAbilities` and `EliteAbilities`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ability {
    /// Incoming damage is divided by the rules' `armor_pm`.
    Stronger,
    /// Weapon damage is multiplied by the rules' `combat_pm`.
    Firepower,
    /// The reload of the weapons is multiplied by the rules' `rof_pm`.
    Rof,
    /// Sight times the rules' `sight_pm`: nothing with the ini's 0.0 (an assumption).
    Sight,
    /// Speed is multiplied by the rules' `speed_pm`.
    Faster,
    /// The unit regains hit points on its own.
    SelfHeal,
    /// The unit scatters from threats: not modelled.
    Scatter,
}

impl Ability {
    /// Every ability, in the order of their bits.
    pub const ALL: [Ability; 7] = [
        Ability::Stronger,
        Ability::Firepower,
        Ability::Rof,
        Ability::Sight,
        Ability::Faster,
        Ability::SelfHeal,
        Ability::Scatter,
    ];

    /// The id of the ability in the arsenal file.
    pub fn id(self) -> &'static str {
        match self {
            Ability::Stronger => "stronger",
            Ability::Firepower => "firepower",
            Ability::Rof => "rof",
            Ability::Sight => "sight",
            Ability::Faster => "faster",
            Ability::SelfHeal => "self_heal",
            Ability::Scatter => "scatter",
        }
    }

    fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|ability| ability.id() == id)
    }
}

/// Which weapon of a type launches the units it spawns (the one with the ini's `Spawner=yes`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Launcher {
    Primary,
    Secondary,
}

/// The missile a spawned unit type is (RA4, docs/RA2_PARITY_PLAN.md section 4.8): it flies to
/// its target and goes off there with an area effect of the combat file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Missile {
    /// The ini's `V3RocketType`.
    V3,
    /// The ini's `DMislType`.
    DMisl,
}

impl Missile {
    /// The effect the missile goes off with: of an elite launcher, or of any other.
    pub fn effect(self, elite: bool) -> crate::Effect {
        match (self, elite) {
            (Missile::V3, false) => crate::Effect::V3Rocket,
            (Missile::V3, true) => crate::Effect::V3RocketElite,
            (Missile::DMisl, false) => crate::Effect::DMisl,
            (Missile::DMisl, true) => crate::Effect::DMislElite,
        }
    }
}

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
    /// Buildings of the type a player may own and have queued together; 0: any number.
    pub build_limit: u32,
    /// Free cells allowed between a new building of the type and a base building of its owner.
    pub adjacent: u32,
    /// Other buildings of its owner may be placed beside it.
    pub base_normal: bool,
    /// A radar building: its owner has the minimap while it works.
    pub radar: bool,
    pub sellable: bool,
    pub repairable: bool,
    /// Under low power the building stops working if it consumes power.
    pub powered: bool,
    /// The charge of its superweapon waits while its owner is low on power.
    pub is_powered: bool,
    /// A damaged vehicle of its owner sent to it is repaired for credits.
    pub unit_repair: bool,
    /// The unit drives over hostile crushable infantry on a `Move` or `AttackMove` command.
    pub crusher: bool,
    /// A crusher of an enemy may drive over the unit while it is on foot.
    pub crushable: bool,
    /// The room the unit takes in a transport.
    pub size: u32,
    /// The unit is of the infantry type list: on foot unless it flies.
    pub infantry: bool,
    /// Units of the type gain experience and ranks (RA4).
    pub trainable: bool,
    /// What the veteran rank grants.
    pub veteran_abilities: Vec<Ability>,
    /// What the elite rank adds.
    pub elite_abilities: Vec<Ability>,
    /// A destroyed vehicle may release a crew member; a destroyed or sold building survivors.
    pub crewed: bool,
    /// The unit regains hit points on its own.
    pub self_healing: bool,
    /// The volleys an aircraft fires before it must reload; 0: unlimited.
    pub ammo: u32,
    /// The aircraft needs a dock: to be produced, to reload, and to exist at all.
    pub airport_bound: bool,
    /// The aircraft the building holds.
    pub docks: u32,
    /// The unit type the unit launches.
    pub spawns: Option<UnitKind>,
    /// How many units of the type `spawns` it has.
    pub spawns_number: u32,
    /// Ticks until a lost one is replaced.
    pub spawn_regen: u32,
    /// Ticks until a returned one is ready again.
    pub spawn_reload: u32,
    /// Which of the type's weapons launches them.
    pub launcher: Option<Launcher>,
    /// The missile a spawned unit of the type is.
    pub missile: Option<Missile>,
    /// A player sees the unit only with sensors near it or shortly after it fired.
    pub cloakable: bool,
    /// The radius in cells within which the type detects cloaked units; 0: no sensors.
    pub sensors_sight: u32,
    // RA5: neutral world objects, occupation and captured-building effects.
    pub needs_engineer: bool,
    pub capturable: bool,
    pub insignificant: bool,
    pub immune: bool,
    pub hospital: bool,
    pub bridge_repair_hut: bool,
    pub occupier: bool,
    pub can_be_occupied: bool,
    pub max_occupants: u32,
    pub vehicle_docks: u32,
    pub cash_startup: u32,
    pub cash_amount: u32,
    pub cash_delay: u32,
    pub deployer: bool,
    pub deploy_fire: bool,
    pub c4: bool,
    pub can_c4: bool,
    pub bombable: bool,
    pub spyable: bool,
    pub detect_disguise: bool,
    pub immune_psionics: bool,
    pub immune_radiation: bool,
    pub teleporter: bool,
    pub stolen_allied: bool,
    pub stolen_soviet: bool,
    pub overpowerable: bool,
    pub undeploy_delay: u32,
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
    build_limit: i64,
    adjacent: i64,
    base_normal: i64,
    radar: i64,
    sellable: i64,
    repairable: i64,
    powered: i64,
    is_powered: i64,
    unit_repair: i64,
    crusher: i64,
    crushable: i64,
    size: i64,
    infantry: i64,
    trainable: i64,
    veteran_abilities: Vec<String>,
    elite_abilities: Vec<String>,
    crewed: i64,
    self_healing: i64,
    ammo: i64,
    airport_bound: i64,
    docks: i64,
    spawns: String,
    spawns_number: i64,
    spawn_regen: i64,
    spawn_reload: i64,
    launcher: String,
    missile: String,
    cloakable: i64,
    sensors_sight: i64,
    needs_engineer: i64,
    capturable: i64,
    insignificant: i64,
    immune: i64,
    hospital: i64,
    bridge_repair_hut: i64,
    occupier: i64,
    can_be_occupied: i64,
    max_occupants: i64,
    vehicle_docks: i64,
    cash_startup: i64,
    cash_amount: i64,
    cash_delay: i64,
    deployer: i64,
    deploy_fire: i64,
    c4: i64,
    can_c4: i64,
    bombable: i64,
    spyable: i64,
    detect_disguise: i64,
    immune_psionics: i64,
    immune_radiation: i64,
    teleporter: i64,
    stolen_allied: i64,
    stolen_soviet: i64,
    overpowerable: i64,
    undeploy_delay: i64,
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
    ("build_limit", Shape::Int),
    ("adjacent", Shape::Int),
    ("base_normal", Shape::Int),
    ("radar", Shape::Int),
    ("sellable", Shape::Int),
    ("repairable", Shape::Int),
    ("powered", Shape::Int),
    ("is_powered", Shape::Int),
    ("unit_repair", Shape::Int),
    ("crusher", Shape::Int),
    ("crushable", Shape::Int),
    ("size", Shape::Int),
    ("infantry", Shape::Int),
    ("trainable", Shape::Int),
    (
        "veteran_abilities",
        Shape::List {
            entry: &Shape::Str,
            max: Ability::ALL.len(),
        },
    ),
    (
        "elite_abilities",
        Shape::List {
            entry: &Shape::Str,
            max: Ability::ALL.len(),
        },
    ),
    ("crewed", Shape::Int),
    ("self_healing", Shape::Int),
    ("ammo", Shape::Int),
    ("airport_bound", Shape::Int),
    ("docks", Shape::Int),
    ("spawns", Shape::Str),
    ("spawns_number", Shape::Int),
    ("spawn_regen", Shape::Int),
    ("spawn_reload", Shape::Int),
    ("launcher", Shape::Str),
    ("missile", Shape::Str),
    ("cloakable", Shape::Int),
    ("sensors_sight", Shape::Int),
    ("needs_engineer", Shape::Int),
    ("capturable", Shape::Int),
    ("insignificant", Shape::Int),
    ("immune", Shape::Int),
    ("hospital", Shape::Int),
    ("bridge_repair_hut", Shape::Int),
    ("occupier", Shape::Int),
    ("can_be_occupied", Shape::Int),
    ("max_occupants", Shape::Int),
    ("vehicle_docks", Shape::Int),
    ("cash_startup", Shape::Int),
    ("cash_amount", Shape::Int),
    ("cash_delay", Shape::Int),
    ("deployer", Shape::Int),
    ("deploy_fire", Shape::Int),
    ("c4", Shape::Int),
    ("can_c4", Shape::Int),
    ("bombable", Shape::Int),
    ("spyable", Shape::Int),
    ("detect_disguise", Shape::Int),
    ("immune_psionics", Shape::Int),
    ("immune_radiation", Shape::Int),
    ("teleporter", Shape::Int),
    ("stolen_allied", Shape::Int),
    ("stolen_soviet", Shape::Int),
    ("overpowerable", Shape::Int),
    ("undeploy_delay", Shape::Int),
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
            // The art is that of an item of the same kind: the game draws a building with a
            // building's sheet and a unit with a unit's.
            let (kind, known) = match item {
                ProductionItem::Building(_) => ("building", defs.building_kind(&e.art).is_some()),
                ProductionItem::Unit(_) => ("unit", defs.unit_kind(&e.art).is_some()),
            };
            if !known {
                return Err(err(
                    format!("entries[{i}].art"),
                    DataErrorKind::UnknownReference {
                        kind: kind.into(),
                        id: e.art.clone(),
                    },
                ));
            }
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
            // The traits of a building (schema version 3): numbers within their ranges, flags
            // 1 or 0, and all of them 0 on a unit.
            let building = matches!(item, ProductionItem::Building(_));
            let traits = [
                ("build_limit", e.build_limit, i64::from(MAX_BUILD_LIMIT)),
                ("adjacent", e.adjacent, i64::from(MAX_PLACEMENT_DISTANCE)),
                ("base_normal", e.base_normal, 1),
                ("radar", e.radar, 1),
                ("sellable", e.sellable, 1),
                ("repairable", e.repairable, 1),
                ("powered", e.powered, 1),
                ("is_powered", e.is_powered, 1),
                ("unit_repair", e.unit_repair, 1),
            ];
            for (name, value, max) in traits {
                if !(0..=max).contains(&value) {
                    return Err(bad(name, &value.to_string()));
                }
                if !building && value != 0 && name != "build_limit" {
                    return Err(bad(name, "a building trait on a unit type"));
                }
            }
            // The traits of a unit (schema version 4): flags 1 or 0, the size within its
            // range, all of them 0 on a building, and each only on the kind of unit that
            // movement can give it to.
            let traits = [
                ("crusher", e.crusher, 1),
                ("crushable", e.crushable, 1),
                ("size", e.size, i64::from(MAX_UNIT_SIZE)),
                ("infantry", e.infantry, 1),
            ];
            for (name, value, max) in traits {
                if !(0..=max).contains(&value) {
                    return Err(bad(name, &value.to_string()));
                }
                if building && value != 0 {
                    return Err(bad(name, "a unit trait on a building type"));
                }
            }
            if e.infantry == 1 && category != Category::Infantry {
                return Err(bad(
                    "infantry",
                    "the infantry trait outside the category infantry",
                ));
            }
            if e.crushable == 1 && e.infantry != 1 {
                return Err(bad("crushable", "a crushable unit that is not infantry"));
            }
            if e.crusher == 1 && (category != Category::Vehicles || domain == Domain::Air) {
                return Err(bad(
                    "crusher",
                    "a crusher that is not a vehicle on the surface",
                ));
            }
            // The traits of RA4 (schema version 5): flags 1 or 0, numbers within their ranges,
            // and each only on the kind of item that the rules can give it to.
            if !(0..=1_000_000).contains(&e.undeploy_delay) {
                return Err(bad("undeploy_delay", "outside 0..1000000"));
            }
            let flags = [
                ("deployer", e.deployer, true),
                ("deploy_fire", e.deploy_fire, true),
                ("c4", e.c4, true),
                ("can_c4", e.can_c4, true),
                ("bombable", e.bombable, true),
                ("spyable", e.spyable, true),
                ("detect_disguise", e.detect_disguise, true),
                ("immune_psionics", e.immune_psionics, true),
                ("immune_radiation", e.immune_radiation, true),
                ("teleporter", e.teleporter, true),
                ("stolen_allied", e.stolen_allied, true),
                ("stolen_soviet", e.stolen_soviet, true),
                ("overpowerable", e.overpowerable, true),
                ("needs_engineer", e.needs_engineer, true),
                ("capturable", e.capturable, true),
                ("insignificant", e.insignificant, true),
                ("immune", e.immune, true),
                ("hospital", e.hospital, true),
                ("bridge_repair_hut", e.bridge_repair_hut, true),
                ("occupier", e.occupier, false),
                ("can_be_occupied", e.can_be_occupied, true),
                ("trainable", e.trainable, false),
                ("crewed", e.crewed, true),
                ("self_healing", e.self_healing, false),
                ("airport_bound", e.airport_bound, false),
                ("cloakable", e.cloakable, false),
            ];
            for (name, value, on_buildings) in flags {
                if !(0..=1).contains(&value) {
                    return Err(bad(name, &value.to_string()));
                }
                if building && !on_buildings && value != 0 {
                    return Err(bad(name, "a unit trait on a building type"));
                }
            }
            let abilities = |name: &str, ids: &[String]| -> Result<Vec<Ability>, DataError> {
                let mut found = Vec::with_capacity(ids.len());
                for (n, id) in ids.iter().enumerate() {
                    let at = format!("entries[{i}].{name}[{n}]");
                    let ability = Ability::parse(id).ok_or_else(|| {
                        err(
                            at.clone(),
                            DataErrorKind::InvalidValue {
                                found: format!("{id:?}"),
                                expected: "a veteran ability".into(),
                            },
                        )
                    })?;
                    if found.contains(&ability) {
                        return Err(err(at, DataErrorKind::DuplicateId(id.clone())));
                    }
                    found.push(ability);
                }
                if building && !found.is_empty() {
                    return Err(bad(name, "a unit trait on a building type"));
                }
                Ok(found)
            };
            let veteran_abilities = abilities("veteran_abilities", &e.veteran_abilities)?;
            let elite_abilities = abilities("elite_abilities", &e.elite_abilities)?;
            let numbers = [
                ("max_occupants", e.max_occupants, 32, true),
                ("vehicle_docks", e.vehicle_docks, 32, true),
                ("cash_startup", e.cash_startup, 1000000, true),
                ("cash_amount", e.cash_amount, 1000000, true),
                ("cash_delay", e.cash_delay, 1000000, true),
                ("ammo", e.ammo, i64::from(MAX_AMMO), false),
                ("docks", e.docks, i64::from(MAX_DOCKS), true),
                (
                    "spawns_number",
                    e.spawns_number,
                    i64::from(MAX_SPAWNS),
                    false,
                ),
                (
                    "spawn_regen",
                    e.spawn_regen,
                    i64::from(MAX_SPAWN_TICKS),
                    false,
                ),
                (
                    "spawn_reload",
                    e.spawn_reload,
                    i64::from(MAX_SPAWN_TICKS),
                    false,
                ),
                (
                    "sensors_sight",
                    e.sensors_sight,
                    i64::from(crate::MAX_SIGHT),
                    true,
                ),
            ];
            for (name, value, max, on_buildings) in numbers {
                if !(0..=max).contains(&value) {
                    return Err(bad(name, &value.to_string()));
                }
                if building && !on_buildings && value != 0 {
                    return Err(bad(name, "a unit trait on a building type"));
                }
            }
            if !building && e.docks != 0 {
                return Err(bad("docks", "a building trait on a unit type"));
            }
            if let ProductionItem::Building(kind) = item
                && let Some(def) = defs.building(kind)
                && e.docks > i64::from(def.width) * i64::from(def.height)
            {
                return Err(bad("docks", "more docks than cells of the footprint"));
            }
            if e.airport_bound == 1 && (domain != Domain::Air || e.ammo == 0) {
                return Err(bad(
                    "airport_bound",
                    "an airport bound unit that is no aircraft with ammunition",
                ));
            }
            let spawns = match e.spawns.as_str() {
                "none" => None,
                id => Some(defs.unit_kind(id).ok_or_else(|| {
                    err(
                        format!("entries[{i}].spawns"),
                        DataErrorKind::UnknownReference {
                            kind: "unit".into(),
                            id: id.into(),
                        },
                    )
                })?),
            };
            let launcher = match e.launcher.as_str() {
                "none" => None,
                "primary" => Some(Launcher::Primary),
                "secondary" => Some(Launcher::Secondary),
                _ => return Err(bad("launcher", &e.launcher)),
            };
            let missile = match e.missile.as_str() {
                "none" => None,
                "v3" => Some(Missile::V3),
                "dmisl" => Some(Missile::DMisl),
                _ => return Err(bad("missile", &e.missile)),
            };
            if building && (spawns.is_some() || launcher.is_some() || missile.is_some()) {
                return Err(bad("spawns", "a unit trait on a building type"));
            }
            match spawns {
                Some(spawned) => {
                    if item == ProductionItem::Unit(spawned) {
                        return Err(bad("spawns", "a unit that spawns its own type"));
                    }
                    if e.spawns_number == 0 {
                        return Err(bad("spawns_number", "a spawner of no units"));
                    }
                    if e.spawn_regen == 0 {
                        return Err(bad("spawn_regen", "a spawner that never replaces a unit"));
                    }
                    if launcher.is_none() {
                        return Err(bad("launcher", "a spawner without a launcher"));
                    }
                }
                None => {
                    if e.spawns_number != 0 || e.spawn_regen != 0 || e.spawn_reload != 0 {
                        return Err(bad("spawns", "spawn terms without a spawned type"));
                    }
                    if launcher.is_some() {
                        return Err(bad("launcher", "a launcher that spawns nothing"));
                    }
                }
            }
            if missile.is_some() && domain != Domain::Air {
                return Err(bad("missile", "a missile that does not fly"));
            }
            // A missile goes off with an effect of the combat file, another one when its
            // launcher is elite: both must be listed, or it would silently deal nothing.
            if let Some(missile) = missile {
                for elite in [false, true] {
                    let effect = missile.effect(elite);
                    if defs.effect(effect).is_none() {
                        return Err(err(
                            format!("entries[{i}].missile"),
                            DataErrorKind::UnknownReference {
                                kind: "effect".into(),
                                id: effect.id().into(),
                            },
                        ));
                    }
                }
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
                    build_limit: e.build_limit as u32,
                    adjacent: e.adjacent as u32,
                    base_normal: e.base_normal == 1,
                    radar: e.radar == 1,
                    sellable: e.sellable == 1,
                    repairable: e.repairable == 1,
                    powered: e.powered == 1,
                    is_powered: e.is_powered == 1,
                    unit_repair: e.unit_repair == 1,
                    crusher: e.crusher == 1,
                    crushable: e.crushable == 1,
                    size: e.size as u32,
                    infantry: e.infantry == 1,
                    trainable: e.trainable == 1,
                    veteran_abilities,
                    elite_abilities,
                    crewed: e.crewed == 1,
                    self_healing: e.self_healing == 1,
                    ammo: e.ammo as u32,
                    airport_bound: e.airport_bound == 1,
                    docks: e.docks as u32,
                    spawns,
                    spawns_number: e.spawns_number as u32,
                    spawn_regen: e.spawn_regen as u32,
                    spawn_reload: e.spawn_reload as u32,
                    launcher,
                    missile,
                    cloakable: e.cloakable == 1,
                    sensors_sight: e.sensors_sight as u32,
                    needs_engineer: e.needs_engineer == 1,
                    capturable: e.capturable == 1,
                    insignificant: e.insignificant == 1,
                    immune: e.immune == 1,
                    hospital: e.hospital == 1,
                    bridge_repair_hut: e.bridge_repair_hut == 1,
                    occupier: e.occupier == 1,
                    can_be_occupied: e.can_be_occupied == 1,
                    max_occupants: e.max_occupants as u32,
                    vehicle_docks: e.vehicle_docks as u32,
                    cash_startup: e.cash_startup as u32,
                    cash_amount: e.cash_amount as u32,
                    cash_delay: e.cash_delay as u32,
                    deployer: e.deployer == 1,
                    deploy_fire: e.deploy_fire == 1,
                    c4: e.c4 == 1,
                    can_c4: e.can_c4 == 1,
                    bombable: e.bombable == 1,
                    spyable: e.spyable == 1,
                    detect_disguise: e.detect_disguise == 1,
                    immune_psionics: e.immune_psionics == 1,
                    immune_radiation: e.immune_radiation == 1,
                    teleporter: e.teleporter == 1,
                    stolen_allied: e.stolen_allied == 1,
                    stolen_soviet: e.stolen_soviet == 1,
                    overpowerable: e.overpowerable == 1,
                    undeploy_delay: e.undeploy_delay as u32,
                },
            );
        }
        // A spawned type is an aircraft with an entry of its own, and spawns nothing itself.
        for (item, entry) in &result {
            let Some(spawned) = entry.spawns else {
                continue;
            };
            let fits = result
                .get(&ProductionItem::Unit(spawned))
                .is_some_and(|other| other.domain == Domain::Air && other.spawns.is_none());
            if !fits {
                let id = match item {
                    ProductionItem::Unit(kind) => defs.unit(*kind).map(|def| def.id.clone()),
                    ProductionItem::Building(_) => None,
                };
                return Err(err(
                    "entries".into(),
                    DataErrorKind::InvalidValue {
                        found: format!(
                            "{:?} spawns {:?}",
                            id.unwrap_or_default(),
                            defs.unit(spawned)
                                .map_or_else(String::new, |def| def.id.clone())
                        ),
                        expected:
                            "a spawned type that is an aircraft of the arsenal and spawns nothing"
                                .into(),
                    },
                ));
            }
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
            h.write_u32(e.build_limit);
            h.write_u32(e.adjacent);
            h.write(&[
                u8::from(e.base_normal),
                u8::from(e.radar),
                u8::from(e.sellable),
                u8::from(e.repairable),
                u8::from(e.powered),
                u8::from(e.is_powered),
                u8::from(e.unit_repair),
            ]);
            h.write(&[
                u8::from(e.crusher),
                u8::from(e.crushable),
                u8::from(e.infantry),
            ]);
            h.write_u32(e.size);
            // RA4: the traits of schema version 5.
            let bits = |abilities: &[Ability]| {
                abilities
                    .iter()
                    .fold(0u8, |bits, &ability| bits | (1 << ability as u8))
            };
            h.write(&[
                u8::from(e.trainable),
                bits(&e.veteran_abilities),
                bits(&e.elite_abilities),
                u8::from(e.crewed),
                u8::from(e.self_healing),
                u8::from(e.airport_bound),
                e.launcher.map_or(u8::MAX, |launcher| launcher as u8),
                e.missile.map_or(u8::MAX, |missile| missile as u8),
                u8::from(e.cloakable),
                u8::from(e.deployer),
                u8::from(e.deploy_fire),
                u8::from(e.c4),
                u8::from(e.can_c4),
                u8::from(e.bombable),
                u8::from(e.spyable),
                u8::from(e.detect_disguise),
                u8::from(e.immune_psionics),
                u8::from(e.immune_radiation),
                u8::from(e.teleporter),
                u8::from(e.stolen_allied),
                u8::from(e.stolen_soviet),
                u8::from(e.overpowerable),
                u8::from(e.needs_engineer),
                u8::from(e.capturable),
                u8::from(e.insignificant),
                u8::from(e.immune),
                u8::from(e.hospital),
                u8::from(e.bridge_repair_hut),
                u8::from(e.occupier),
                u8::from(e.can_be_occupied),
            ]);
            h.write(&e.spawns.map_or(u16::MAX, |kind| kind.0).to_le_bytes());
            for value in [
                e.ammo,
                e.docks,
                e.spawns_number,
                e.spawn_regen,
                e.spawn_reload,
                e.sensors_sight,
                e.max_occupants,
                e.vehicle_docks,
                e.cash_startup,
                e.cash_amount,
                e.cash_delay,
                e.undeploy_delay,
            ] {
                h.write_u32(value);
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
    /// The lane in which `item` is produced (RA2): with a rules file a building of the
    /// arsenal's category `armory` (the ini's `BuildCat=Combat`) is in the defence lane; every
    /// other item, and every item of a world without a rules file, in the main lane.
    pub fn lane(&self, item: ProductionItem) -> crate::Lane {
        let defence = self.rules().is_some()
            && matches!(item, ProductionItem::Building(_))
            && self
                .metadata(item)
                .is_some_and(|entry| entry.category == Category::Armory);
        if defence {
            crate::Lane::Defences
        } else {
            crate::Lane::Main
        }
    }
    /// What a player's queue of `item` is kept under ([`Definitions::queue_key`] of its
    /// producer, and its [`Definitions::lane`]), or `None` for an item without production
    /// terms.
    pub fn queue_of(&self, item: ProductionItem) -> Option<crate::QueueKey> {
        self.production(item).map(|terms| crate::QueueKey {
            producer: self.queue_key(terms.producer),
            lane: self.lane(item),
        })
    }
    /// How many items the queue of `item` holds. Without a rules file
    /// [`MAX_QUEUE_LENGTH`](crate::MAX_QUEUE_LENGTH). With one (RA2): a lane of buildings holds
    /// one item, since Red Alert 2 does not queue buildings; a queue of units the one in
    /// production and `max_queued_objects` behind it.
    pub fn queue_capacity(&self, item: ProductionItem) -> usize {
        match (self.rules(), item) {
            (None, _) => crate::MAX_QUEUE_LENGTH,
            (Some(_), ProductionItem::Building(_)) => 1,
            (Some(rules), ProductionItem::Unit(_)) => {
                usize::try_from(rules.production.max_queued_objects)
                    .unwrap_or(0)
                    .saturating_add(1)
            }
        }
    }
    /// The credits a player gets for selling a building of type `kind`: the fixed refund of a
    /// type the economy file's `refunds` lists (RA5: the ini's `Soylent`), else what the refund
    /// rule gives ([`Definitions::rule_refund`]).
    pub fn refund(&self, kind: BuildingKind) -> i64 {
        self.building(kind)
            .and_then(|def| def.fixed_refund)
            .unwrap_or_else(|| self.rule_refund(kind))
    }
    /// What the refund rule gives for a building of type `kind`: with a rules file the cost
    /// times `refund_percent` (RA2, docs/RA2_PARITY_PLAN.md section 4.6), without one the
    /// economy file's `sell_value`. The survivors of a building follow this value, not a fixed
    /// refund (docs/DECISIONS.md R58).
    pub fn rule_refund(&self, kind: BuildingKind) -> i64 {
        let Some(def) = self.building(kind) else {
            return 0;
        };
        match self.rules() {
            Some(rules) => {
                def.production
                    .cost
                    .saturating_mul(i64::from(rules.economy.refund_percent))
                    / 100
            }
            None => def.sell_value,
        }
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
    /// Whether units of type `kind` are on foot (RA3, invariant I6'): infantry that does not
    /// fly, which holds one of the three spots of a cell instead of the cell. No unit of a
    /// world without an arsenal is.
    pub fn on_foot(&self, kind: UnitKind) -> bool {
        self.metadata(ProductionItem::Unit(kind))
            .is_some_and(|e| e.infantry && e.domain != Domain::Air)
    }
    /// Whether units of type `kind` drive over hostile crushable infantry when they are
    /// commanded to move (the arsenal's `crusher`).
    pub fn crusher(&self, kind: UnitKind) -> bool {
        self.metadata(ProductionItem::Unit(kind))
            .is_some_and(|e| e.crusher)
    }
    /// Whether a crusher of an enemy may drive over units of type `kind` (the arsenal's
    /// `crushable`, for a unit on foot).
    pub fn crushable(&self, kind: UnitKind) -> bool {
        self.on_foot(kind)
            && self
                .metadata(ProductionItem::Unit(kind))
                .is_some_and(|e| e.crushable)
    }
    /// Whether any unit type is a crusher: without one nothing is ever crushed.
    pub fn has_crushers(&self) -> bool {
        self.arsenal.0.values().any(|e| e.crusher)
    }
    /// Whether units of type `kind` gain experience and ranks (RA4: the arsenal's `trainable`,
    /// with a rules file).
    pub fn trainable(&self, kind: UnitKind) -> bool {
        self.rules().is_some()
            && self
                .metadata(ProductionItem::Unit(kind))
                .is_some_and(|e| e.trainable)
    }
    /// What a unit of type `kind` has of `ability` at `rank` (0 none, 1 veteran, 2 elite): a
    /// veteran has the type's `veteran_abilities`, an elite unit those and its
    /// `elite_abilities`, each once (docs/RA2_PARITY_PLAN.md section 4.5).
    pub fn has_ability(&self, kind: UnitKind, rank: u8, ability: Ability) -> bool {
        self.metadata(ProductionItem::Unit(kind)).is_some_and(|e| {
            (rank >= 1 && e.veteran_abilities.contains(&ability))
                || (rank >= 2 && e.elite_abilities.contains(&ability))
        })
    }
    /// The unit types that some type of the arsenal spawns (RA4, with a rules file): missiles
    /// and carried aircraft, which belong to the unit that launched them.
    pub fn spawned_kinds(&self) -> std::collections::BTreeSet<UnitKind> {
        if self.rules().is_none() {
            return std::collections::BTreeSet::new();
        }
        self.arsenal.0.values().filter_map(|e| e.spawns).collect()
    }
    /// The weapons with which the types of the arsenal launch what they spawn (RA4, with a
    /// rules file): the weapon in the `launcher` slot of every spawner, below and at elite
    /// rank. A shot of such a weapon launches a unit and is no shot at the target itself.
    pub fn launcher_weapons(&self) -> std::collections::BTreeSet<crate::WeaponKind> {
        if self.rules().is_none() {
            return std::collections::BTreeSet::new();
        }
        self.arsenal
            .0
            .iter()
            .filter_map(|(item, entry)| match (item, entry.spawns, entry.launcher) {
                (ProductionItem::Unit(kind), Some(_), Some(slot)) => {
                    Some((self.unit_combat(*kind)?, slot))
                }
                _ => None,
            })
            .flat_map(|(terms, slot)| {
                [false, true].map(|elite| terms.weapons(elite)[slot as usize])
            })
            .flatten()
            .collect()
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
