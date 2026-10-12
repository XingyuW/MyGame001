//! Global rules loaded from RON: the values of the Red Alert 2 ruleset that belong to no single
//! unit or building (docs/RA2_PARITY_PLAN.md sections 3.2 and 4; docs/DECISIONS.md R9, R10).
//!
//! Rules file, schema version 1 (an extension file, like the arsenal: a scenario may omit it, and
//! then every rule that reads it keeps its behaviour of M2 to M9):
//!
//! ```ron
//! (
//!     schema_version: 1,
//!     production: (build_ticks_per_1000: 840, multiple_factory_pm: 800, ...),
//!     economy: (refund_percent: 50, ...),
//!     veterancy: (...), crew: (...), combat: (...), radiation: (...), superweapons: (...),
//!     prism: (...), crates: (...),
//!     powerups: [(id: "money", share: 20, value: 2000)],
//!     dialog: (min_credits: 5000, credits: 10000, ...),
//!     difficulty: [(id: "easy", rof_pm: 1200, ...)],
//!     prerequisite_groups: [(id: "power", buildings: ["power_plant", "tesla_reactor"])],
//!     sides: [(id: "allied", base_unit: "mcv", construction_yard: "construction_yard", ...)],
//!     countries: [(id: "america", side: "allied")],
//! )
//! ```
//!
//! Every field is required, so there are no silent defaults. All values are in game units
//! (ticks, subcells, permille, percent; docs/RA2_PARITY_PLAN.md section 2); yes/no keys are 1 or
//! 0. The scalar groups are checked field by field against the ranges below; ids follow the rules
//! of unit ids (`definitions::check_name`) and are unique within their list; every building and
//! unit named (the members of a prerequisite group, a side's base unit, construction yard, crew,
//! disguise and paradrop infantry) is defined by the units and economy files; `sides` lists
//! `allied` and `soviet` at most once each, no two sides share a base unit or a construction
//! yard, and every country names a listed side. A load error
//! names the file, the field, and the reason.
//!
//! RA1 consumes `sides` (the base unit and the construction yard it deploys into), `countries`,
//! `prerequisite_groups`, and of `dialog` the tech level and the ranges of the starting credits
//! and the unit count; the other groups are read by later milestones.

use std::path::Path;

use serde::Deserialize;

use crate::data::{self, DataError, DataErrorKind, MAX_DATA_FILE_BYTES, Shape};
use crate::definitions::{Definitions, UnitKind, check_name};
use crate::economy_defs::{BuildingKind, Problem, invalid, ranged, unknown};
use crate::hash::Fnv1a64;

/// The schema version of the rules file this crate reads.
pub const RULES_SCHEMA_VERSION: i64 = 1;

/// Largest number of prerequisite groups.
pub const MAX_PREREQUISITE_GROUPS: usize = 16;

/// Largest number of buildings in one prerequisite group.
pub const MAX_GROUP_BUILDINGS: usize = 16;

/// Largest number of countries.
pub const MAX_COUNTRIES: usize = 16;

/// Largest number of powerups (crate contents).
pub const MAX_POWERUPS: usize = 32;

/// Largest number of difficulty entries.
pub const MAX_RULES_DIFFICULTIES: usize = 8;

/// Largest tech level (`[MultiplayerDialogSettings] TechLevel`).
pub const MAX_TECH_LEVEL: i32 = 10;

/// A side: the two armies of Red Alert 2. An item or a faction without a side (the fixture
/// scenarios of M2 to M9) has `None` wherever a side is optional.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    Allied,
    Soviet,
}

impl Side {
    /// Both sides, in the order of [`Side::index`].
    pub const ALL: [Side; 2] = [Side::Allied, Side::Soviet];

    /// The side's name in the data files.
    pub fn id(self) -> &'static str {
        match self {
            Side::Allied => "allied",
            Side::Soviet => "soviet",
        }
    }

    /// The side named `id` in a data file, or `None` for any other text (`"none"` included).
    pub fn parse(id: &str) -> Option<Side> {
        match id {
            "allied" => Some(Side::Allied),
            "soviet" => Some(Side::Soviet),
            _ => None,
        }
    }

    /// 0 for the Allied and 1 for the Soviet side.
    pub fn index(self) -> usize {
        match self {
            Side::Allied => 0,
            Side::Soviet => 1,
        }
    }
}

/// Index of a country in [`Rules::countries`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CountryId(pub u8);

/// Index of a prerequisite group in [`Rules::prerequisite_groups`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GroupId(pub u8);

/// A group of scalar rules: the struct, its schema, its range checks and its hash from one list
/// of fields `name: min => max`.
macro_rules! scalar_group {
    (
        $(#[$meta:meta])*
        $name:ident, $entry:ident, $fields:ident {
            $($(#[$field_meta:meta])* $field:ident: $min:expr => $max:expr,)*
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
        pub struct $name {
            $($(#[$field_meta])* pub $field: i32,)*
        }

        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct $entry {
            $($field: i64,)*
        }

        static $fields: &[(&str, Shape)] = &[$((stringify!($field), Shape::Int),)*];

        impl $name {
            /// The fields in the order of the rules file, with their values.
            pub fn fields(&self) -> Vec<(&'static str, i32)> {
                vec![$((stringify!($field), self.$field),)*]
            }

            fn validate(group: &str, entry: &$entry) -> Result<Self, Problem> {
                Ok(Self {
                    $($field: ranged(
                        &format!("{group}.{}", stringify!($field)),
                        entry.$field,
                        $min,
                        $max,
                        "",
                    )?,)*
                })
            }

            fn hash_into(&self, hasher: &mut Fnv1a64) {
                $(hasher.write(&self.$field.to_le_bytes());)*
            }
        }
    };
}

/// Longest duration in ticks a rule may hold (one hour).
const MAX_TICKS: i32 = 72_000;
/// Largest permille value (ten times).
const MAX_PM: i32 = 10_000;
/// Largest distance in subcells (64 cells).
const MAX_SUBCELLS: i32 = 16_384;
/// Largest credit, damage or hit point amount a rule may hold.
const MAX_AMOUNT: i32 = 1_000_000;

scalar_group! {
    /// Production (docs/RA2_PARITY_PLAN.md section 4.2).
    ProductionRules, ProductionEntry, PRODUCTION_FIELDS {
        /// Ticks of build time per 1000 credits of cost (`BuildSpeed` in game minutes).
        build_ticks_per_1000: 1 => MAX_TICKS,
        /// Build time factor of each further factory (`MultipleFactory`).
        multiple_factory_pm: 1 => 1000,
        /// Build time factor of walls (`WallBuildSpeedCoefficient`).
        wall_build_time_pm: 1 => MAX_PM,
        low_power_penalty_pm: 0 => MAX_PM,
        min_low_power_speed_pm: 0 => 1000,
        max_low_power_speed_pm: 0 => 1000,
        /// Items a unit queue holds behind the one in production (`MaximumQueuedObjects`).
        max_queued_objects: 0 => 255,
    }
}

scalar_group! {
    /// Selling, repair, reloading and ore (sections 4.6 and 4.10).
    EconomyRules, EconomyEntry, ECONOMY_FIELDS {
        refund_percent: 0 => 100,
        repair_percent: 0 => 100,
        /// Hit points per repair step of a building.
        repair_step: 1 => MAX_AMOUNT,
        /// Ticks between two repair steps of a building.
        repair_interval: 1 => MAX_TICKS,
        /// Ticks between two repair steps of a vehicle on a depot.
        unit_repair_interval: 1 => MAX_TICKS,
        infantry_repair_step: 1 => MAX_AMOUNT,
        infantry_repair_interval: 1 => MAX_TICKS,
        /// Ticks per round an aircraft reloads on its dock.
        reload_interval: 1 => MAX_TICKS,
        purifier_bonus_percent: 0 => 1000,
        /// Credits per bail of ore and of gems.
        ore_value: 0 => MAX_AMOUNT,
        gem_value: 0 => MAX_AMOUNT,
        ore_growth_interval: 1 => MAX_TICKS,
        ore_growth_pm: 0 => 1000,
        ore_spread_interval: 1 => MAX_TICKS,
        ore_spread_pm: 0 => 1000,
        harvesters_per_refinery: 0 => 8,
    }
}

scalar_group! {
    /// Veterancy (section 4.5).
    VeterancyRules, VeterancyEntry, VETERANCY_FIELDS {
        veteran_ratio_pm: 1 => 100_000,
        veteran_cap: 0 => 2,
        combat_pm: 0 => MAX_PM,
        speed_pm: 0 => MAX_PM,
        sight_pm: 0 => MAX_PM,
        armor_pm: 1 => MAX_PM,
        rof_pm: 1 => MAX_PM,
        /// 1 if units start as veterans (`InitialVeteran`).
        initial_veteran: 0 => 1,
    }
}

scalar_group! {
    /// Crews and survivors (section 4.7).
    CrewRules, CrewEntry, CREW_FIELDS {
        crew_escape_percent: 0 => 100,
        survivor_rate_pm: 0 => 1000,
    }
}

scalar_group! {
    /// Damage and the values of weapons the ini keeps in `[General]` and `[CombatDamage]`
    /// (sections 4.3, 4.8 and 4.13).
    CombatRules, CombatEntry, COMBAT_FIELDS {
        max_damage: 1 => MAX_AMOUNT,
        min_damage: 0 => MAX_AMOUNT,
        condition_yellow_percent: 0 => 100,
        condition_red_percent: 0 => 100,
        /// Subcells.
        ballistic_scatter: 0 => MAX_SUBCELLS,
        homing_scatter: 0 => MAX_SUBCELLS,
        /// Ticks between a C4 charge and its explosion.
        c4_delay: 1 => MAX_TICKS,
        ivan_damage: 0 => MAX_AMOUNT,
        ivan_delay: 1 => MAX_TICKS,
        v3_rocket_damage: 0 => MAX_AMOUNT,
        v3_rocket_elite_damage: 0 => MAX_AMOUNT,
        dmisl_damage: 0 => MAX_AMOUNT,
        dmisl_elite_damage: 0 => MAX_AMOUNT,
        bridge_strength: 1 => MAX_AMOUNT,
        /// Ticks a submerged unit stays visible after firing.
        cloak_delay: 1 => MAX_TICKS,
        /// Leptons of distance per frame of delay after a chrono move.
        chrono_distance_factor: 1 => MAX_SUBCELLS,
        /// Shortest delay after a chrono move, in ticks.
        chrono_minimum_delay: 0 => MAX_TICKS,
        /// Ticks an infiltrated power plant is off.
        spy_power_blackout: 0 => MAX_TICKS,
        spy_money_steal_percent: 0 => 100,
        /// Hit points per level of a tree (`TreeStrength`).
        tree_strength: 1 => MAX_AMOUNT,
    }
}

scalar_group! {
    /// Radiation (section 4.13).
    RadiationRules, RadiationEntry, RADIATION_FIELDS {
        /// Ticks between two applications of radiation damage.
        application_delay: 1 => MAX_TICKS,
        /// Ticks between two steps of decay.
        level_delay: 1 => MAX_TICKS,
        level_factor_pm: 0 => MAX_PM,
        level_max: 0 => MAX_AMOUNT,
        duration_multiple_pm: 0 => MAX_PM,
    }
}

scalar_group! {
    /// Superweapons and support powers (section 4.13).
    SuperweaponRules, SuperweaponEntry, SUPERWEAPON_FIELDS {
        nuclear_recharge: 1 => MAX_TICKS,
        lightning_recharge: 1 => MAX_TICKS,
        iron_curtain_recharge: 1 => MAX_TICKS,
        chronosphere_recharge: 1 => MAX_TICKS,
        paradrop_recharge: 1 => MAX_TICKS,
        american_paradrop_recharge: 1 => MAX_TICKS,
        iron_curtain_duration: 1 => MAX_TICKS,
        lightning_deferment: 0 => MAX_TICKS,
        lightning_duration: 1 => MAX_TICKS,
        lightning_damage: 0 => MAX_AMOUNT,
        lightning_hit_delay: 1 => MAX_TICKS,
        lightning_scatter_delay: 1 => MAX_TICKS,
        /// Cells.
        lightning_cell_spread: 0 => 64,
        lightning_separation: 0 => 64,
        american_paradrop_count: 0 => 64,
        /// Subcells.
        paradrop_radius: 0 => MAX_SUBCELLS,
    }
}

scalar_group! {
    /// The Prism Tower support chain (section 4.13).
    PrismRules, PrismEntry, PRISM_FIELDS {
        support_delay: 1 => MAX_TICKS,
        support_duration: 1 => MAX_TICKS,
        support_max: 0 => 64,
        support_modifier_pm: 0 => MAX_PM,
        /// Leptons.
        support_height: 0 => MAX_SUBCELLS,
    }
}

scalar_group! {
    /// Crates (section 4.11).
    CrateRules, CrateEntry, CRATE_FIELDS {
        maximum: 0 => 255,
        minimum: 0 => 255,
        /// Subcells.
        radius: 0 => MAX_SUBCELLS,
        /// Ticks.
        regen: 1 => MAX_TICKS,
        free_mcv: 0 => 1,
    }
}

scalar_group! {
    /// The lobby settings (`[MultiplayerDialogSettings]`, section 3.5).
    DialogRules, DialogEntry, DIALOG_FIELDS {
        min_credits: 0 => MAX_AMOUNT,
        credits: 0 => MAX_AMOUNT,
        max_credits: 0 => MAX_AMOUNT,
        credits_step: 1 => MAX_AMOUNT,
        min_unit_count: 0 => 64,
        unit_count: 0 => 64,
        max_unit_count: 0 => 64,
        tech_level: 0 => MAX_TECH_LEVEL,
        bases: 0 => 1,
        short_game: 0 => 1,
        crates: 0 => 1,
        mcv_redeploys: 0 => 1,
        bridge_destruction: 0 => 1,
        fog_of_war: 0 => 1,
        shroud: 0 => 1,
        max_players: 2 => 16,
    }
}

/// A crate's content: its share of all contents and its argument (credits, a permille
/// multiplier, or ranks, by kind).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Powerup {
    pub id: String,
    pub share: i32,
    pub value: i32,
}

/// The multipliers and counts of a computer player's difficulty (section 4.15).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DifficultyRules {
    pub id: String,
    /// Reload time factor.
    pub rof_pm: i32,
    pub build_time_pm: i32,
    /// Starting credits added, in percent of the match's starting credits.
    pub credits_bonus_percent: i32,
    pub virtual_purifiers: i32,
    /// Ticks between two attack waves.
    pub team_delay: i32,
    pub allied_base_defences: i32,
    pub soviet_base_defences: i32,
}

/// A prerequisite group: owning any one of its buildings satisfies it (section 3.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrerequisiteGroup {
    pub id: String,
    /// At least one building type, each once.
    pub buildings: Vec<BuildingKind>,
}

/// What belongs to a side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SideRules {
    pub side: Side,
    /// The unit a player of the side starts with and that counts as its base in a short game
    /// (`[General] BaseUnit`).
    pub base_unit: UnitKind,
    /// The building the base unit deploys into and that undeploys into it.
    pub construction_yard: BuildingKind,
    /// The infantry that leaves a destroyed vehicle or building of the side.
    pub crew: UnitKind,
    pub survivor_divisor: i32,
    /// The look a spy of another side takes by default.
    pub disguise: UnitKind,
    pub paradrop_infantry: UnitKind,
    pub paradrop_count: i32,
}

/// A country (a house of the ini): the nine of the skirmish menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Country {
    pub id: String,
    pub side: Side,
}

/// The rules file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rules {
    pub production: ProductionRules,
    pub economy: EconomyRules,
    pub veterancy: VeterancyRules,
    pub crew: CrewRules,
    pub combat: CombatRules,
    pub radiation: RadiationRules,
    pub superweapons: SuperweaponRules,
    pub prism: PrismRules,
    pub crates: CrateRules,
    pub powerups: Vec<Powerup>,
    pub dialog: DialogRules,
    pub difficulties: Vec<DifficultyRules>,
    pub prerequisite_groups: Vec<PrerequisiteGroup>,
    /// At most one entry per side.
    pub sides: Vec<SideRules>,
    pub countries: Vec<Country>,
}

impl Rules {
    /// The rules of `side`, if the file lists it.
    pub fn side(&self, side: Side) -> Option<&SideRules> {
        self.sides.iter().find(|rules| rules.side == side)
    }

    /// The country with this id.
    pub fn country_id(&self, id: &str) -> Option<CountryId> {
        self.countries
            .iter()
            .position(|country| country.id == id)
            .and_then(|index| u8::try_from(index).ok())
            .map(CountryId)
    }

    pub fn country(&self, id: CountryId) -> Option<&Country> {
        self.countries.get(usize::from(id.0))
    }

    /// The prerequisite group with this id.
    pub fn group_id(&self, id: &str) -> Option<GroupId> {
        self.prerequisite_groups
            .iter()
            .position(|group| group.id == id)
            .and_then(|index| u8::try_from(index).ok())
            .map(GroupId)
    }

    pub fn group(&self, id: GroupId) -> Option<&PrerequisiteGroup> {
        self.prerequisite_groups.get(usize::from(id.0))
    }

    /// The side whose base unit is `unit`.
    pub fn side_of_base_unit(&self, unit: UnitKind) -> Option<&SideRules> {
        self.sides.iter().find(|side| side.base_unit == unit)
    }

    /// The side whose construction yard is `building`.
    pub fn side_of_construction_yard(&self, building: BuildingKind) -> Option<&SideRules> {
        self.sides
            .iter()
            .find(|side| side.construction_yard == building)
    }

    /// Whether `unit` is the base unit of some side.
    pub fn is_base_unit(&self, unit: UnitKind) -> bool {
        self.side_of_base_unit(unit).is_some()
    }

    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        self.production.hash_into(hasher);
        self.economy.hash_into(hasher);
        self.veterancy.hash_into(hasher);
        self.crew.hash_into(hasher);
        self.combat.hash_into(hasher);
        self.radiation.hash_into(hasher);
        self.superweapons.hash_into(hasher);
        self.prism.hash_into(hasher);
        self.crates.hash_into(hasher);
        let text = |hasher: &mut Fnv1a64, text: &str| {
            hasher.write_u64(text.len() as u64);
            hasher.write(text.as_bytes());
        };
        hasher.write_u64(self.powerups.len() as u64);
        for powerup in &self.powerups {
            text(hasher, &powerup.id);
            hasher.write(&powerup.share.to_le_bytes());
            hasher.write(&powerup.value.to_le_bytes());
        }
        self.dialog.hash_into(hasher);
        hasher.write_u64(self.difficulties.len() as u64);
        for difficulty in &self.difficulties {
            text(hasher, &difficulty.id);
            for value in [
                difficulty.rof_pm,
                difficulty.build_time_pm,
                difficulty.credits_bonus_percent,
                difficulty.virtual_purifiers,
                difficulty.team_delay,
                difficulty.allied_base_defences,
                difficulty.soviet_base_defences,
            ] {
                hasher.write(&value.to_le_bytes());
            }
        }
        hasher.write_u64(self.prerequisite_groups.len() as u64);
        for group in &self.prerequisite_groups {
            text(hasher, &group.id);
            hasher.write_u64(group.buildings.len() as u64);
            for building in &group.buildings {
                hasher.write(&building.0.to_le_bytes());
            }
        }
        hasher.write_u64(self.sides.len() as u64);
        for side in &self.sides {
            hasher.write(&[side.side.index() as u8]);
            for unit in [
                side.base_unit,
                side.crew,
                side.disguise,
                side.paradrop_infantry,
            ] {
                hasher.write(&unit.0.to_le_bytes());
            }
            hasher.write(&side.construction_yard.0.to_le_bytes());
            hasher.write(&side.survivor_divisor.to_le_bytes());
            hasher.write(&side.paradrop_count.to_le_bytes());
        }
        hasher.write_u64(self.countries.len() as u64);
        for country in &self.countries {
            text(hasher, &country.id);
            hasher.write(&[country.side.index() as u8]);
        }
    }
}

// --- Schema ---------------------------------------------------------------------------------

static POWERUP_SHAPE: Shape = Shape::Struct(&[
    ("id", Shape::Str),
    ("share", Shape::Int),
    ("value", Shape::Int),
]);

static DIFFICULTY_SHAPE: Shape = Shape::Struct(&[
    ("id", Shape::Str),
    ("rof_pm", Shape::Int),
    ("build_time_pm", Shape::Int),
    ("credits_bonus_percent", Shape::Int),
    ("virtual_purifiers", Shape::Int),
    ("team_delay", Shape::Int),
    ("allied_base_defences", Shape::Int),
    ("soviet_base_defences", Shape::Int),
]);

static GROUP_SHAPE: Shape = Shape::Struct(&[
    ("id", Shape::Str),
    (
        "buildings",
        Shape::List {
            entry: &Shape::Str,
            max: MAX_GROUP_BUILDINGS,
        },
    ),
]);

static SIDE_SHAPE: Shape = Shape::Struct(&[
    ("id", Shape::Str),
    ("base_unit", Shape::Str),
    ("construction_yard", Shape::Str),
    ("crew", Shape::Str),
    ("survivor_divisor", Shape::Int),
    ("disguise", Shape::Str),
    ("paradrop_infantry", Shape::Str),
    ("paradrop_count", Shape::Int),
]);

static COUNTRY_SHAPE: Shape = Shape::Struct(&[("id", Shape::Str), ("side", Shape::Str)]);

static RULES_FILE_SHAPE: Shape = Shape::Struct(&[
    ("schema_version", Shape::SchemaVersion(RULES_SCHEMA_VERSION)),
    ("production", Shape::Struct(PRODUCTION_FIELDS)),
    ("economy", Shape::Struct(ECONOMY_FIELDS)),
    ("veterancy", Shape::Struct(VETERANCY_FIELDS)),
    ("crew", Shape::Struct(CREW_FIELDS)),
    ("combat", Shape::Struct(COMBAT_FIELDS)),
    ("radiation", Shape::Struct(RADIATION_FIELDS)),
    ("superweapons", Shape::Struct(SUPERWEAPON_FIELDS)),
    ("prism", Shape::Struct(PRISM_FIELDS)),
    ("crates", Shape::Struct(CRATE_FIELDS)),
    (
        "powerups",
        Shape::List {
            entry: &POWERUP_SHAPE,
            max: MAX_POWERUPS,
        },
    ),
    ("dialog", Shape::Struct(DIALOG_FIELDS)),
    (
        "difficulty",
        Shape::List {
            entry: &DIFFICULTY_SHAPE,
            max: MAX_RULES_DIFFICULTIES,
        },
    ),
    (
        "prerequisite_groups",
        Shape::List {
            entry: &GROUP_SHAPE,
            max: MAX_PREREQUISITE_GROUPS,
        },
    ),
    (
        "sides",
        Shape::List {
            entry: &SIDE_SHAPE,
            max: 2,
        },
    ),
    (
        "countries",
        Shape::List {
            entry: &COUNTRY_SHAPE,
            max: MAX_COUNTRIES,
        },
    ),
]);

/// The rules file as `ron` reads it. Integers are read as `i64`, the widest type the checker
/// admits, so that range errors are reported with their field.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RulesFile {
    schema_version: i64,
    production: ProductionEntry,
    economy: EconomyEntry,
    veterancy: VeterancyEntry,
    crew: CrewEntry,
    combat: CombatEntry,
    radiation: RadiationEntry,
    superweapons: SuperweaponEntry,
    prism: PrismEntry,
    crates: CrateEntry,
    powerups: Vec<PowerupEntry>,
    dialog: DialogEntry,
    difficulty: Vec<DifficultyEntry>,
    prerequisite_groups: Vec<GroupEntry>,
    sides: Vec<SideEntry>,
    countries: Vec<CountryEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PowerupEntry {
    id: String,
    share: i64,
    value: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DifficultyEntry {
    id: String,
    rof_pm: i64,
    build_time_pm: i64,
    credits_bonus_percent: i64,
    virtual_purifiers: i64,
    team_delay: i64,
    allied_base_defences: i64,
    soviet_base_defences: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupEntry {
    id: String,
    buildings: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SideEntry {
    id: String,
    base_unit: String,
    construction_yard: String,
    crew: String,
    survivor_divisor: i64,
    disguise: String,
    paradrop_infantry: String,
    paradrop_count: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CountryEntry {
    id: String,
    side: String,
}

/// Loads and validates the rules file at `path` against `definitions` (the units and economy
/// files), whose building and unit types it names.
pub(crate) fn load(path: &Path, definitions: &Definitions) -> Result<Rules, DataError> {
    let bytes = data::read_bounded(path, MAX_DATA_FILE_BYTES).map_err(|kind| DataError {
        file: path.to_owned(),
        field: "file".into(),
        kind,
    })?;
    let text = data::check(path, &bytes, &RULES_FILE_SHAPE)?;
    let file: RulesFile = data::parse(path, text)?;
    validate(file, definitions).map_err(|(field, kind)| DataError {
        file: path.to_owned(),
        field,
        kind,
    })
}

fn validate(file: RulesFile, definitions: &Definitions) -> Result<Rules, Problem> {
    if file.schema_version != RULES_SCHEMA_VERSION {
        return Err((
            "schema_version".into(),
            DataErrorKind::UnsupportedSchemaVersion {
                found: file.schema_version,
                expected: RULES_SCHEMA_VERSION,
            },
        ));
    }
    let production = ProductionRules::validate("production", &file.production)?;
    if production.min_low_power_speed_pm > production.max_low_power_speed_pm {
        return Err(invalid(
            "production.min_low_power_speed_pm".into(),
            production.min_low_power_speed_pm,
            "at most max_low_power_speed_pm",
        ));
    }
    let combat = CombatRules::validate("combat", &file.combat)?;
    if combat.condition_red_percent > combat.condition_yellow_percent {
        return Err(invalid(
            "combat.condition_red_percent".into(),
            combat.condition_red_percent,
            "at most condition_yellow_percent",
        ));
    }
    let crates = CrateRules::validate("crates", &file.crates)?;
    if crates.minimum > crates.maximum {
        return Err(invalid(
            "crates.minimum".into(),
            crates.minimum,
            "at most crates.maximum",
        ));
    }
    let dialog = DialogRules::validate("dialog", &file.dialog)?;
    for (field, value, low, high) in [
        (
            "dialog.credits",
            dialog.credits,
            dialog.min_credits,
            dialog.max_credits,
        ),
        (
            "dialog.unit_count",
            dialog.unit_count,
            dialog.min_unit_count,
            dialog.max_unit_count,
        ),
    ] {
        if low > high || !(low..=high).contains(&value) {
            return Err(invalid(
                field.into(),
                value,
                format!("the default within its range, {low} to {high}"),
            ));
        }
    }
    if (dialog.credits - dialog.min_credits) % dialog.credits_step != 0
        || (dialog.max_credits - dialog.min_credits) % dialog.credits_step != 0
    {
        return Err(invalid(
            "dialog.credits_step".into(),
            dialog.credits_step,
            "a step that divides the range of the credits and reaches their default",
        ));
    }

    let mut powerups: Vec<Powerup> = Vec::with_capacity(file.powerups.len());
    for (index, entry) in file.powerups.into_iter().enumerate() {
        let field = |name: &str| format!("powerups[{index}].{name}");
        check_name(&entry.id).map_err(|kind| (field("id"), kind))?;
        if powerups.iter().any(|known| known.id == entry.id) {
            return Err((field("id"), DataErrorKind::DuplicateId(entry.id)));
        }
        powerups.push(Powerup {
            share: ranged(&field("share"), entry.share, 0, 1000, "")?,
            value: ranged(&field("value"), entry.value, 0, MAX_AMOUNT, "")?,
            id: entry.id,
        });
    }

    let mut difficulties: Vec<DifficultyRules> = Vec::with_capacity(file.difficulty.len());
    for (index, entry) in file.difficulty.into_iter().enumerate() {
        let field = |name: &str| format!("difficulty[{index}].{name}");
        check_name(&entry.id).map_err(|kind| (field("id"), kind))?;
        if difficulties.iter().any(|known| known.id == entry.id) {
            return Err((field("id"), DataErrorKind::DuplicateId(entry.id)));
        }
        difficulties.push(DifficultyRules {
            rof_pm: ranged(&field("rof_pm"), entry.rof_pm, 1, MAX_PM, "")?,
            build_time_pm: ranged(&field("build_time_pm"), entry.build_time_pm, 1, MAX_PM, "")?,
            credits_bonus_percent: ranged(
                &field("credits_bonus_percent"),
                entry.credits_bonus_percent,
                0,
                10_000,
                "",
            )?,
            virtual_purifiers: ranged(
                &field("virtual_purifiers"),
                entry.virtual_purifiers,
                0,
                64,
                "",
            )?,
            team_delay: ranged(&field("team_delay"), entry.team_delay, 1, MAX_TICKS, "")?,
            allied_base_defences: ranged(
                &field("allied_base_defences"),
                entry.allied_base_defences,
                0,
                255,
                "",
            )?,
            soviet_base_defences: ranged(
                &field("soviet_base_defences"),
                entry.soviet_base_defences,
                0,
                255,
                "",
            )?,
            id: entry.id,
        });
    }

    let building = |field: String, id: &str| {
        definitions
            .building_kind(id)
            .ok_or_else(|| unknown(field, "building", id))
    };
    let unit = |field: String, id: &str| {
        definitions
            .unit_kind(id)
            .ok_or_else(|| unknown(field, "unit", id))
    };

    let mut prerequisite_groups: Vec<PrerequisiteGroup> =
        Vec::with_capacity(file.prerequisite_groups.len());
    for (index, entry) in file.prerequisite_groups.into_iter().enumerate() {
        let field = format!("prerequisite_groups[{index}]");
        check_name(&entry.id).map_err(|kind| (format!("{field}.id"), kind))?;
        if prerequisite_groups.iter().any(|known| known.id == entry.id) {
            return Err((format!("{field}.id"), DataErrorKind::DuplicateId(entry.id)));
        }
        if entry.buildings.is_empty() {
            return Err(invalid(
                format!("{field}.buildings"),
                "an empty list",
                format!("1 to {MAX_GROUP_BUILDINGS} building types"),
            ));
        }
        let mut buildings = Vec::with_capacity(entry.buildings.len());
        for (position, id) in entry.buildings.iter().enumerate() {
            let at = format!("{field}.buildings[{position}]");
            let kind = building(at.clone(), id)?;
            if buildings.contains(&kind) {
                return Err((at, DataErrorKind::DuplicateId(id.clone())));
            }
            buildings.push(kind);
        }
        prerequisite_groups.push(PrerequisiteGroup {
            id: entry.id,
            buildings,
        });
    }

    let mut sides: Vec<SideRules> = Vec::with_capacity(file.sides.len());
    for (index, entry) in file.sides.into_iter().enumerate() {
        let field = |name: &str| format!("sides[{index}].{name}");
        let side = Side::parse(&entry.id).ok_or_else(|| {
            invalid(
                field("id"),
                format!("{:?}", entry.id),
                "\"allied\" or \"soviet\"",
            )
        })?;
        if sides.iter().any(|known| known.side == side) {
            return Err((field("id"), DataErrorKind::DuplicateId(entry.id)));
        }
        let base_unit = unit(field("base_unit"), &entry.base_unit)?;
        let construction_yard = building(field("construction_yard"), &entry.construction_yard)?;
        // Deploying and undeploying map a base unit to its yard and back, so neither may
        // belong to two sides.
        if sides.iter().any(|known| known.base_unit == base_unit) {
            return Err(invalid(
                field("base_unit"),
                format!("{:?}", entry.base_unit),
                "a base unit no other side has",
            ));
        }
        if sides
            .iter()
            .any(|known| known.construction_yard == construction_yard)
        {
            return Err(invalid(
                field("construction_yard"),
                format!("{:?}", entry.construction_yard),
                "a construction yard no other side has",
            ));
        }
        sides.push(SideRules {
            side,
            base_unit,
            construction_yard,
            crew: unit(field("crew"), &entry.crew)?,
            survivor_divisor: ranged(
                &field("survivor_divisor"),
                entry.survivor_divisor,
                1,
                MAX_AMOUNT,
                "",
            )?,
            disguise: unit(field("disguise"), &entry.disguise)?,
            paradrop_infantry: unit(field("paradrop_infantry"), &entry.paradrop_infantry)?,
            paradrop_count: ranged(&field("paradrop_count"), entry.paradrop_count, 0, 64, "")?,
        });
    }

    let mut countries: Vec<Country> = Vec::with_capacity(file.countries.len());
    for (index, entry) in file.countries.into_iter().enumerate() {
        let field = |name: &str| format!("countries[{index}].{name}");
        check_name(&entry.id).map_err(|kind| (field("id"), kind))?;
        if countries.iter().any(|known| known.id == entry.id) {
            return Err((field("id"), DataErrorKind::DuplicateId(entry.id)));
        }
        let side = Side::parse(&entry.side)
            .filter(|side| sides.iter().any(|known| known.side == *side))
            .ok_or_else(|| {
                invalid(
                    field("side"),
                    format!("{:?}", entry.side),
                    "a side listed in sides",
                )
            })?;
        countries.push(Country { id: entry.id, side });
    }

    Ok(Rules {
        production,
        economy: EconomyRules::validate("economy", &file.economy)?,
        veterancy: VeterancyRules::validate("veterancy", &file.veterancy)?,
        crew: CrewRules::validate("crew", &file.crew)?,
        combat,
        radiation: RadiationRules::validate("radiation", &file.radiation)?,
        superweapons: SuperweaponRules::validate("superweapons", &file.superweapons)?,
        prism: PrismRules::validate("prism", &file.prism)?,
        crates,
        powerups,
        dialog,
        difficulties,
        prerequisite_groups,
        sides,
        countries,
    })
}
