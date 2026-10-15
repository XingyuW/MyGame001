//! Immutable combat definitions loaded from RON (PLAN.md sections 5 and 7, M4; docs/DECISIONS.md
//! D28; schema version 2: docs/RA2_PARITY_PLAN.md sections 4.3 and 4.4, docs/DECISIONS.md R32;
//! schema version 3: section 4.8, docs/DECISIONS.md R46).
//!
//! Combat file, schema version 3:
//!
//! ```ron
//! (
//!     schema_version: 3,
//!     armour_classes: ["light", "heavy", "structure"],
//!     warheads: [
//!         (id: "ap", cell_spread: 77, percent_at_max: 50, wall: 1, wood: 1, versus: [
//!             (armour: "light", percent: 50),
//!             (armour: "heavy", percent: 100),
//!             (armour: "structure", percent: 60),
//!         ]),
//!     ],
//!     weapons: [
//!         (id: "cannon", damage: 40, range: 1280, min_range: 0, reload: 30, burst: 1,
//!          warhead: "ap", projectile_speed: 160, anti_air: 0, anti_ground: 1, scatter: 0),
//!     ],
//!     units: [
//!         (unit: "light_tank", hit_points: 300, armour: "heavy", sight: 6,
//!          weapons: ["cannon"], elite_weapons: ["cannon"], air_range_bonus: 0,
//!          land_targeting: 0, naval_targeting: 0, underwater: 0, organic: 0),
//!     ],
//!     buildings: [
//!         (building: "construction_yard", hit_points: 1500, armour: "structure", sight: 5,
//!          weapons: [], elite_weapons: [], air_range_bonus: 0, land_targeting: 0,
//!          naval_targeting: 0, wall: 0),
//!     ],
//!     effects: [
//!         (id: "nuke", damage: 600, warhead: "ap"),
//!     ],
//! )
//! ```
//!
//! Every field is required, so there are no silent defaults. Names follow the rules of unit ids
//! (`definitions::check_name`). Every reference (an armour class, a warhead, a weapon, a unit type
//! of the units file, a building type of the economy file) must be defined; every warhead lists
//! every armour class exactly once; and every unit type and every building type has exactly one
//! entry, so no unit or building is silently left without hit points, armour, or sight. Ranges are
//! checked on the parsed values; the limits below keep every quantity the simulation derives from
//! them inside its integer types (invariant I12) and bound the number of projectiles in flight.
//!
//! Since schema version 2 (RA3):
//!
//! * a warhead has `cell_spread` (the radius in subcells around its impact point within which it
//!   damages; 0: only what it hits), `percent_at_max` (the percentage of its damage dealt at
//!   that radius, falling linearly from 100 at the impact point), and as 1 or 0 `wall` (it
//!   damages buildings that are walls) and `wood` (it damages trees);
//! * a weapon has `min_range` (subcells; it does not fire at a target nearer than this),
//!   `burst` (shots of one volley, [`BURST_GAP`] ticks apart, before it reloads), `anti_air` and
//!   `anti_ground` (the layers its projectile may engage: aircraft, and everything on the ground
//!   and at sea), and `scatter` (0 for an accurate weapon; otherwise the largest distance in
//!   subcells, at the weapon's full range, between its target and the point its shot lands on);
//! * a type lists up to two `weapons` (primary, secondary) and the `elite_weapons` that take
//!   their places at elite rank, and has `air_range_bonus` (subcells added to the range of an
//!   anti air weapon against aircraft), `land_targeting` (0: it attacks targets on land; 1: it
//!   does not; 2: with its secondary weapon only) and `naval_targeting` (how it treats targets
//!   on water: 0 none that are underwater; 1 underwater ones with its secondary; 2 only
//!   underwater ones; 3 organic ones with its secondary; 4 all with its secondary; 5 all like
//!   targets on land; 6 none);
//! * a unit type is `underwater` and `organic` or not (1 or 0), and a building type is a `wall`
//!   or not;
//! * `effects` names the base damage and the warhead of area effects that are no weapon of a
//!   type ([`Effect`]): each at most once. An effect that is not listed keeps the rule it had
//!   before RA3 (damage that falls off with the distance in cells and ignores armour).
//!
//! Since schema version 3 (RA4) `effects` may also name the impact of the two spawned missiles
//! and of each when its launcher is elite (`v3_rocket`, `v3_rocket_elite`, `dmisl`,
//! `dmisl_elite`); nothing else changed. A missile whose effect is not listed deals nothing.
//!
//! Values that leave every rule of M4 as it was (the fixtures of M2 to M9): no spread and 100
//! percent at the edge, `wall` and `wood` 1, no minimum range, a burst of 1, both layers, no
//! scatter, the elite weapons equal to the weapons, no bonus, `land_targeting` 0,
//! `naval_targeting` 5, 0 for `underwater`, `organic` and `wall`, and no effects.

use std::path::Path;

use serde::Deserialize;

use crate::data::{self, DataError, DataErrorKind, MAX_DATA_FILE_BYTES, Shape};
use crate::definitions::{MAX_UNIT_TYPES, check_name};
use crate::economy_defs::MAX_BUILDING_TYPES;
use crate::hash::Fnv1a64;

/// The schema version of the combat file this crate reads.
pub const COMBAT_SCHEMA_VERSION: i64 = 5;

/// Largest number of armour classes.
pub const MAX_ARMOUR_CLASSES: usize = 16;

/// Largest number of warheads.
pub const MAX_WARHEADS: usize = 64;

/// Largest number of weapons.
pub const MAX_WEAPONS: usize = 128;

/// Largest number of weapons of one unit or building type: a primary and a secondary one, and as
/// many elite weapons.
pub const MAX_WEAPONS_PER_TYPE: usize = 2;

/// Largest hit points of a unit or building type.
pub const MAX_HIT_POINTS: i32 = 1_000_000;

/// Largest base damage of a weapon. With [`MAX_VERSUS_PERCENT`] the damage of one shot is at most
/// 10^6, so `base × versus` fits an `i32` and many shots on one target in one tick are summed in
/// `i64` (equation 2).
pub const MAX_DAMAGE: i32 = 100_000;

/// Largest percentage a warhead deals against an armour class.
pub const MAX_VERSUS_PERCENT: i32 = 1_000;

/// Smallest weapon range in subcells: 1.5 cells. A unit standing at a cell centre then reaches a
/// target at the centre of any neighbouring cell (362 subcells away diagonally) and the footprint
/// of a building whose ring it stands in, so every weapon can hit something its unit can walk up
/// to.
pub const MIN_WEAPON_RANGE: u32 = 384;

/// Largest weapon range in subcells: 30 cells.
pub const MAX_WEAPON_RANGE: u32 = 7680;

/// Longest reload time of a weapon in ticks (one minute).
pub const MAX_RELOAD: u32 = 1200;

/// Smallest speed of a travelling projectile in subcells per tick. A shot is fired only at a
/// target within range (and the largest air range bonus), so a projectile flies at most
/// `(MAX_WEAPON_RANGE + MAX_AIR_RANGE_BONUS) / MIN_PROJECTILE_SPEED` = 608 ticks, which bounds
/// the projectiles a weapon has in flight (docs/DECISIONS.md D28, R32).
pub const MIN_PROJECTILE_SPEED: u32 = 16;

/// Largest speed of a travelling projectile in subcells per tick.
pub const MAX_PROJECTILE_SPEED: u32 = 4096;

/// Largest sight radius in cells.
pub const MAX_SIGHT: u32 = 16;

/// Largest spread of a warhead in subcells: 11 cells, the largest the original engine supports.
pub const MAX_CELL_SPREAD: u32 = 2816;

/// Largest number of shots in one volley of a weapon.
pub const MAX_BURST: u32 = 8;

/// Ticks between two shots of a volley (docs/RA2_PARITY_PLAN.md section 4.4; an assumption: the
/// original waits 3 to 5 frames at random).
pub const BURST_GAP: u32 = 5;

/// Largest scatter of a weapon in subcells: 8 cells.
pub const MAX_SCATTER: u32 = 2048;

/// Largest air range bonus of a type in subcells: 8 cells.
pub const MAX_AIR_RANGE_BONUS: u32 = 2048;

/// Largest value of `land_targeting`.
pub const MAX_LAND_TARGETING: u8 = 2;

/// Largest value of `naval_targeting`.
pub const MAX_NAVAL_TARGETING: u8 = 6;

/// An area effect of `crate::special` that is no weapon of a unit or building type. With an entry
/// in the combat file it is an impact of that entry's warhead and base damage
/// (docs/RA2_PARITY_PLAN.md section 4.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Effect {
    /// The strike of the nuclear missile.
    Nuke,
    /// A bolt of the lightning storm.
    Lightning,
    /// The bomb a demolition specialist plants.
    IvanBomb,
    /// The charge a commando sets on a building.
    C4,
    /// A pulse of a radiation zone.
    Radiation,
    /// The impact of a V3 rocket (RA4: a spawned missile, `crate::flight`).
    V3Rocket,
    /// The impact of the V3 rocket of an elite launcher.
    V3RocketElite,
    /// The impact of a Dreadnought's missile.
    DMisl,
    /// The impact of the missile of an elite Dreadnought.
    DMislElite,
}

impl Effect {
    /// Every effect, in the order of their hashes.
    pub const ALL: [Effect; 9] = [
        Effect::Nuke,
        Effect::Lightning,
        Effect::IvanBomb,
        Effect::C4,
        Effect::Radiation,
        Effect::V3Rocket,
        Effect::V3RocketElite,
        Effect::DMisl,
        Effect::DMislElite,
    ];

    /// The id of the effect in the combat file.
    pub fn id(self) -> &'static str {
        match self {
            Effect::Nuke => "nuke",
            Effect::Lightning => "lightning",
            Effect::IvanBomb => "ivan_bomb",
            Effect::C4 => "c4",
            Effect::Radiation => "radiation",
            Effect::V3Rocket => "v3_rocket",
            Effect::V3RocketElite => "v3_rocket_elite",
            Effect::DMisl => "dmisl",
            Effect::DMislElite => "dmisl_elite",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// The impact of an [`Effect`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EffectDef {
    /// Base damage, from 1 to [`MAX_DAMAGE`].
    pub damage: i32,
    pub warhead: WarheadKind,
}

/// Index of an armour class in [`Definitions`](crate::Definitions).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArmourClass(pub u16);

/// Index of a warhead in [`Definitions`](crate::Definitions).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WarheadKind(pub u16);

/// Index of a weapon in [`Definitions`](crate::Definitions).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WeaponKind(pub u16);

/// A warhead: the percentage of a weapon's base damage it deals against each armour class
/// (PLAN.md section 5, equation 2), and how far around its impact point it damages
/// (docs/RA2_PARITY_PLAN.md section 4.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WarheadDef {
    pub id: String,
    /// Indexed by [`ArmourClass`]; one entry per armour class, each from 0 to
    /// [`MAX_VERSUS_PERCENT`].
    pub versus: Vec<i32>,
    /// The radius in subcells around the impact point within which it damages, from 0 (only
    /// what it hits) to [`MAX_CELL_SPREAD`].
    pub cell_spread: u32,
    /// The percentage of the damage dealt at `cell_spread` from the impact point, from 0 to 100.
    pub percent_at_max: u32,
    /// It damages buildings that are walls.
    pub wall: bool,
    /// It damages trees.
    pub wood: bool,
    pub temporal: bool,
    pub mind_control: bool,
    pub psychic_damage: bool,
    pub radiation: bool,
    pub parasite: bool,
    pub ivan_bomb: bool,
    pub bomb_disarm: bool,
    pub electric_assault: bool,
    pub paralyzes: u32,
    pub culling: bool,
    pub makes_disguise: bool,
}

/// A weapon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponDef {
    pub id: String,
    /// Base damage, from 1 to [`MAX_DAMAGE`].
    pub damage: i32,
    /// Range in subcells, from [`MIN_WEAPON_RANGE`] to [`MAX_WEAPON_RANGE`] (equation 1).
    pub range: u32,
    /// It does not fire at a target nearer than this many subcells; below `range`.
    pub min_range: u32,
    /// Ticks between the last shot of a volley and the next volley, from 1 to [`MAX_RELOAD`].
    pub reload: u32,
    /// Shots of one volley, [`BURST_GAP`] ticks apart, from 1 to [`MAX_BURST`].
    pub burst: u32,
    pub warhead: WarheadKind,
    /// 0 for an instant weapon, whose damage is dealt in the tick it fires; otherwise the speed of
    /// its projectile in subcells per tick, from [`MIN_PROJECTILE_SPEED`] to
    /// [`MAX_PROJECTILE_SPEED`].
    pub projectile_speed: u32,
    /// Its projectile may engage aircraft.
    pub anti_air: bool,
    /// Its projectile may engage what is on the ground or at sea. At least one of the two holds.
    pub anti_ground: bool,
    /// 0 for an accurate weapon. Otherwise its shot lands on a point up to
    /// `scatter × distance / range` subcells from its target, and damages by the spread of its
    /// warhead alone; at most [`MAX_SCATTER`].
    pub scatter: u32,
    pub rad_level: u32,
    pub area_fire: bool,
    pub naval_only: bool,
    pub sonic: bool,
    pub repair: bool,
    pub suicide: bool,
}

/// The combat terms of a unit or building type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CombatTerms {
    /// From 1 to [`MAX_HIT_POINTS`].
    pub hit_points: i32,
    pub armour: ArmourClass,
    /// Sight radius in cells, from 0 (its own cells only) to [`MAX_SIGHT`] (equation 6).
    pub sight: u32,
    /// The primary weapon. A type without one has no weapon at all.
    pub weapon: Option<WeaponKind>,
    /// The secondary weapon.
    pub secondary: Option<WeaponKind>,
    /// The weapons in the places of the primary and the secondary one at elite rank.
    pub elite: [Option<WeaponKind>; 2],
    /// Subcells added to the range of an anti air weapon against aircraft, at most
    /// [`MAX_AIR_RANGE_BONUS`].
    pub air_range_bonus: u32,
    /// 0: it attacks targets on land; 1: it does not; 2: with its secondary weapon only.
    pub land_targeting: u8,
    /// How it treats targets on water: 0 none that are underwater; 1 underwater ones with its
    /// secondary weapon; 2 only underwater ones; 3 organic ones with its secondary weapon; 4 all
    /// with its secondary weapon; 5 all like targets on land; 6 none.
    pub naval_targeting: u8,
    /// A unit type that moves under water. Always false for a building type.
    pub underwater: bool,
    /// A unit type that is a living thing. Always false for a building type.
    pub organic: bool,
    /// A building type that is a wall: only warheads with `wall` damage it. Always false for a
    /// unit type.
    pub wall: bool,
}

impl CombatTerms {
    /// The weapons in the places of the primary and the secondary one, at elite rank or below.
    pub fn weapons(&self, elite: bool) -> [Option<WeaponKind>; 2] {
        if elite {
            self.elite
        } else {
            [self.weapon, self.secondary]
        }
    }
}

/// The combat part of [`Definitions`](crate::Definitions). Empty until match setup installs a
/// loaded file; with it, every unit and building type has [`CombatTerms`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CombatDefinitions {
    pub(crate) armour_classes: Vec<String>,
    pub(crate) warheads: Vec<WarheadDef>,
    pub(crate) weapons: Vec<WeaponDef>,
    /// Indexed by [`UnitKind`](crate::UnitKind).
    pub(crate) units: Vec<Option<CombatTerms>>,
    /// Indexed by [`BuildingKind`](crate::BuildingKind).
    pub(crate) buildings: Vec<Option<CombatTerms>>,
    /// Indexed as [`Effect::ALL`].
    pub(crate) effects: [Option<EffectDef>; 9],
}

impl CombatDefinitions {
    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        fn text(hasher: &mut Fnv1a64, text: &str) {
            hasher.write_u64(text.len() as u64);
            hasher.write(text.as_bytes());
        }
        fn terms(hasher: &mut Fnv1a64, entry: &Option<CombatTerms>) {
            match entry {
                None => hasher.write(&[0]),
                Some(terms) => {
                    hasher.write(&[1]);
                    hasher.write(&terms.hit_points.to_le_bytes());
                    hasher.write(&terms.armour.0.to_le_bytes());
                    hasher.write_u32(terms.sight);
                    for weapon in [
                        terms.weapon,
                        terms.secondary,
                        terms.elite[0],
                        terms.elite[1],
                    ] {
                        match weapon {
                            None => hasher.write(&[0]),
                            Some(weapon) => {
                                hasher.write(&[1]);
                                hasher.write(&weapon.0.to_le_bytes());
                            }
                        }
                    }
                    hasher.write_u32(terms.air_range_bonus);
                    hasher.write(&[
                        terms.land_targeting,
                        terms.naval_targeting,
                        u8::from(terms.underwater),
                        u8::from(terms.organic),
                        u8::from(terms.wall),
                    ]);
                }
            }
        }
        hasher.write_u64(self.armour_classes.len() as u64);
        for name in &self.armour_classes {
            text(hasher, name);
        }
        hasher.write_u64(self.warheads.len() as u64);
        for warhead in &self.warheads {
            text(hasher, &warhead.id);
            hasher.write_u32(warhead.paralyzes);
            hasher.write(&[
                u8::from(warhead.temporal),
                u8::from(warhead.mind_control),
                u8::from(warhead.psychic_damage),
                u8::from(warhead.radiation),
                u8::from(warhead.parasite),
                u8::from(warhead.ivan_bomb),
                u8::from(warhead.bomb_disarm),
                u8::from(warhead.electric_assault),
                u8::from(warhead.culling),
                u8::from(warhead.makes_disguise),
            ]);
            hasher.write_u64(warhead.versus.len() as u64);
            for percent in &warhead.versus {
                hasher.write(&percent.to_le_bytes());
            }
            hasher.write_u32(warhead.cell_spread);
            hasher.write_u32(warhead.percent_at_max);
            hasher.write(&[u8::from(warhead.wall), u8::from(warhead.wood)]);
        }
        hasher.write_u64(self.weapons.len() as u64);
        for weapon in &self.weapons {
            text(hasher, &weapon.id);
            hasher.write(&[u8::from(weapon.sonic)]);
            hasher.write(&[u8::from(weapon.repair)]);
            hasher.write(&weapon.damage.to_le_bytes());
            hasher.write_u32(weapon.range);
            hasher.write_u32(weapon.min_range);
            hasher.write_u32(weapon.reload);
            hasher.write_u32(weapon.burst);
            hasher.write(&weapon.warhead.0.to_le_bytes());
            hasher.write_u32(weapon.projectile_speed);
            hasher.write(&[u8::from(weapon.anti_air), u8::from(weapon.anti_ground)]);
            hasher.write_u32(weapon.scatter);
            hasher.write_u32(weapon.rad_level);
            hasher.write(&[
                u8::from(weapon.area_fire),
                u8::from(weapon.naval_only),
                u8::from(weapon.suicide),
            ]);
        }
        for list in [&self.units, &self.buildings] {
            hasher.write_u64(list.len() as u64);
            for entry in list {
                terms(hasher, entry);
            }
        }
        for effect in &self.effects {
            match effect {
                None => hasher.write(&[0]),
                Some(effect) => {
                    hasher.write(&[1]);
                    hasher.write(&effect.damage.to_le_bytes());
                    hasher.write(&effect.warhead.0.to_le_bytes());
                }
            }
        }
    }
}

// --- Schema ---------------------------------------------------------------------------------

static VERSUS_SHAPE: Shape = Shape::Struct(&[("armour", Shape::Str), ("percent", Shape::Int)]);

static WARHEAD_SHAPE: Shape = Shape::Struct(&[
    ("id", Shape::Str),
    ("cell_spread", Shape::Int),
    ("percent_at_max", Shape::Int),
    ("wall", Shape::Int),
    ("wood", Shape::Int),
    ("temporal", Shape::Int),
    ("mind_control", Shape::Int),
    ("psychic_damage", Shape::Int),
    ("radiation", Shape::Int),
    ("parasite", Shape::Int),
    ("ivan_bomb", Shape::Int),
    ("bomb_disarm", Shape::Int),
    ("electric_assault", Shape::Int),
    ("paralyzes", Shape::Int),
    ("culling", Shape::Int),
    ("makes_disguise", Shape::Int),
    (
        "versus",
        Shape::List {
            entry: &VERSUS_SHAPE,
            max: MAX_ARMOUR_CLASSES,
        },
    ),
]);

static WEAPON_SHAPE: Shape = Shape::Struct(&[
    ("id", Shape::Str),
    ("damage", Shape::Int),
    ("range", Shape::Int),
    ("min_range", Shape::Int),
    ("reload", Shape::Int),
    ("burst", Shape::Int),
    ("warhead", Shape::Str),
    ("projectile_speed", Shape::Int),
    ("anti_air", Shape::Int),
    ("anti_ground", Shape::Int),
    ("scatter", Shape::Int),
    ("rad_level", Shape::Int),
    ("area_fire", Shape::Int),
    ("naval_only", Shape::Int),
    ("sonic", Shape::Int),
    ("repair", Shape::Int),
    ("suicide", Shape::Int),
]);

const WEAPON_LIST_SHAPE: Shape = Shape::List {
    entry: &Shape::Str,
    max: MAX_WEAPONS_PER_TYPE,
};

static UNIT_TERMS_SHAPE: Shape = Shape::Struct(&[
    ("unit", Shape::Str),
    ("hit_points", Shape::Int),
    ("armour", Shape::Str),
    ("sight", Shape::Int),
    ("weapons", WEAPON_LIST_SHAPE),
    ("elite_weapons", WEAPON_LIST_SHAPE),
    ("air_range_bonus", Shape::Int),
    ("land_targeting", Shape::Int),
    ("naval_targeting", Shape::Int),
    ("underwater", Shape::Int),
    ("organic", Shape::Int),
]);

static BUILDING_TERMS_SHAPE: Shape = Shape::Struct(&[
    ("building", Shape::Str),
    ("hit_points", Shape::Int),
    ("armour", Shape::Str),
    ("sight", Shape::Int),
    ("weapons", WEAPON_LIST_SHAPE),
    ("elite_weapons", WEAPON_LIST_SHAPE),
    ("air_range_bonus", Shape::Int),
    ("land_targeting", Shape::Int),
    ("naval_targeting", Shape::Int),
    ("wall", Shape::Int),
]);

static EFFECT_SHAPE: Shape = Shape::Struct(&[
    ("id", Shape::Str),
    ("damage", Shape::Int),
    ("warhead", Shape::Str),
]);

static COMBAT_FILE_SHAPE: Shape = Shape::Struct(&[
    (
        "schema_version",
        Shape::SchemaVersion(COMBAT_SCHEMA_VERSION),
    ),
    (
        "armour_classes",
        Shape::List {
            entry: &Shape::Str,
            max: MAX_ARMOUR_CLASSES,
        },
    ),
    (
        "warheads",
        Shape::List {
            entry: &WARHEAD_SHAPE,
            max: MAX_WARHEADS,
        },
    ),
    (
        "weapons",
        Shape::List {
            entry: &WEAPON_SHAPE,
            max: MAX_WEAPONS,
        },
    ),
    (
        "units",
        Shape::List {
            entry: &UNIT_TERMS_SHAPE,
            max: MAX_UNIT_TYPES,
        },
    ),
    (
        "buildings",
        Shape::List {
            entry: &BUILDING_TERMS_SHAPE,
            max: MAX_BUILDING_TYPES,
        },
    ),
    (
        "effects",
        Shape::List {
            entry: &EFFECT_SHAPE,
            max: Effect::ALL.len(),
        },
    ),
]);

/// The combat file as `ron` reads it. Integers are read as `i64`, the widest type the checker
/// admits, so that range errors are reported by [`validate`] with their field.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CombatFile {
    schema_version: i64,
    armour_classes: Vec<String>,
    warheads: Vec<WarheadEntry>,
    weapons: Vec<WeaponEntry>,
    units: Vec<UnitTermsEntry>,
    buildings: Vec<BuildingTermsEntry>,
    effects: Vec<EffectEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EffectEntry {
    id: String,
    damage: i64,
    warhead: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WarheadEntry {
    id: String,
    cell_spread: i64,
    percent_at_max: i64,
    wall: i64,
    wood: i64,
    temporal: i64,
    mind_control: i64,
    psychic_damage: i64,
    radiation: i64,
    parasite: i64,
    ivan_bomb: i64,
    bomb_disarm: i64,
    electric_assault: i64,
    paralyzes: i64,
    culling: i64,
    makes_disguise: i64,

    versus: Vec<VersusEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VersusEntry {
    armour: String,
    percent: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WeaponEntry {
    id: String,
    damage: i64,
    range: i64,
    min_range: i64,
    reload: i64,
    burst: i64,
    warhead: String,
    projectile_speed: i64,
    anti_air: i64,
    anti_ground: i64,
    scatter: i64,
    rad_level: i64,
    area_fire: i64,
    naval_only: i64,
    sonic: i64,
    repair: i64,
    suicide: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnitTermsEntry {
    unit: String,
    hit_points: i64,
    armour: String,
    sight: i64,
    weapons: Vec<String>,
    elite_weapons: Vec<String>,
    air_range_bonus: i64,
    land_targeting: i64,
    naval_targeting: i64,
    underwater: i64,
    organic: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildingTermsEntry {
    building: String,
    hit_points: i64,
    armour: String,
    sight: i64,
    weapons: Vec<String>,
    elite_weapons: Vec<String>,
    air_range_bonus: i64,
    land_targeting: i64,
    naval_targeting: i64,
    wall: i64,
}

/// Loads and validates the combat file at `path` against the unit types `units` and the building
/// types `buildings` (their ids, in order).
pub(crate) fn load(
    path: &Path,
    units: &[String],
    buildings: &[String],
) -> Result<CombatDefinitions, DataError> {
    let bytes = data::read_bounded(path, MAX_DATA_FILE_BYTES).map_err(|kind| DataError {
        file: path.to_owned(),
        field: "file".into(),
        kind,
    })?;
    let text = data::check(path, &bytes, &COMBAT_FILE_SHAPE)?;
    let file: CombatFile = data::parse(path, text)?;
    validate(file, units, buildings).map_err(|(field, kind)| DataError {
        file: path.to_owned(),
        field,
        kind,
    })
}

/// A validation problem: the field and the reason.
type Problem = (String, DataErrorKind);

fn invalid(field: String, found: impl ToString, expected: impl Into<String>) -> Problem {
    (
        field,
        DataErrorKind::InvalidValue {
            found: found.to_string(),
            expected: expected.into(),
        },
    )
}

/// `value` as `T` if it lies in `min..=max`.
fn ranged<T>(field: &str, value: i64, min: T, max: T, unit: &str) -> Result<T, Problem>
where
    T: Copy + TryFrom<i64> + Into<i64> + std::fmt::Display,
{
    T::try_from(value)
        .ok()
        .filter(|&v| (min.into()..=max.into()).contains(&v.into()))
        .ok_or_else(|| invalid(field.to_owned(), value, format!("{min} to {max}{unit}")))
}

/// The index of `id` in `known` as a `u16`, or an unknown reference of `kind` at `field`.
fn reference(field: String, kind: &str, known: &[String], id: &str) -> Result<u16, Problem> {
    known
        .iter()
        .position(|candidate| candidate == id)
        .and_then(|index| u16::try_from(index).ok())
        .ok_or_else(|| {
            (
                field,
                DataErrorKind::UnknownReference {
                    kind: kind.to_owned(),
                    id: id.to_owned(),
                },
            )
        })
}

/// Checks the names in `names` (the `id` fields of a list at `list`) and that they are unique.
fn unique_names<'a>(
    list: &str,
    names: impl Iterator<Item = (String, &'a str)>,
) -> Result<Vec<String>, Problem> {
    let mut seen: Vec<String> = Vec::new();
    for (field, name) in names {
        check_name(name).map_err(|kind| (field.clone(), kind))?;
        if seen.iter().any(|known| known == name) {
            return Err((field, DataErrorKind::DuplicateId(name.to_owned())));
        }
        seen.push(name.to_owned());
    }
    if seen.is_empty() && list == "armour_classes" {
        return Err(invalid(
            list.to_owned(),
            "an empty list",
            format!("1 to {MAX_ARMOUR_CLASSES} armour classes"),
        ));
    }
    Ok(seen)
}

/// `value` as a flag: 1 or 0.
fn flag(field: &str, value: i64) -> Result<bool, Problem> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(invalid(field.to_owned(), value, "0 or 1")),
    }
}

/// The fields of an entry of `units` or `buildings` that both have.
struct TermsEntry<'a> {
    hit_points: i64,
    armour: &'a str,
    sight: i64,
    weapons: &'a [String],
    elite_weapons: &'a [String],
    air_range_bonus: i64,
    land_targeting: i64,
    naval_targeting: i64,
    /// `underwater`, `organic` and `wall`, each with its field name; a unit entry has the first
    /// two and a building entry the third.
    underwater: Option<i64>,
    organic: Option<i64>,
    wall: Option<i64>,
}

/// Validates one entry of `units` or `buildings`.
fn terms(
    field: &str,
    entry: &TermsEntry<'_>,
    armour_classes: &[String],
    weapon_ids: &[String],
) -> Result<CombatTerms, Problem> {
    let at = |name: &str| format!("{field}.{name}");
    let hit_points = ranged(&at("hit_points"), entry.hit_points, 1, MAX_HIT_POINTS, "")?;
    let armour = ArmourClass(reference(
        at("armour"),
        "armour class",
        armour_classes,
        entry.armour,
    )?);
    let sight = ranged(&at("sight"), entry.sight, 0, MAX_SIGHT, " cells")?;
    // The schema admits at most `MAX_WEAPONS_PER_TYPE` = 2 entries per list.
    let slots = |list: &str, ids: &[String]| -> Result<[Option<WeaponKind>; 2], Problem> {
        let mut slots = [None; 2];
        for (index, (slot, id)) in slots.iter_mut().zip(ids).enumerate() {
            *slot = Some(WeaponKind(reference(
                format!("{field}.{list}[{index}]"),
                "weapon",
                weapon_ids,
                id,
            )?));
        }
        Ok(slots)
    };
    let [weapon, secondary] = slots("weapons", entry.weapons)?;
    let elite = slots("elite_weapons", entry.elite_weapons)?;
    if weapon.is_none() && elite[0].is_some() {
        return Err(invalid(
            at("elite_weapons"),
            format!("{} weapons", entry.elite_weapons.len()),
            "no elite weapon for a type without a weapon",
        ));
    }
    if weapon.is_some() && elite[0].is_none() {
        return Err(invalid(
            at("elite_weapons"),
            "an empty list",
            "the weapons of an armed type at elite rank",
        ));
    }
    let air_range_bonus = ranged(
        &at("air_range_bonus"),
        entry.air_range_bonus,
        0,
        MAX_AIR_RANGE_BONUS,
        " subcells",
    )?;
    let land_targeting = ranged(
        &at("land_targeting"),
        entry.land_targeting,
        0,
        MAX_LAND_TARGETING,
        "",
    )?;
    let naval_targeting = ranged(
        &at("naval_targeting"),
        entry.naval_targeting,
        0,
        MAX_NAVAL_TARGETING,
        "",
    )?;
    let optional = |name: &str, value: Option<i64>| match value {
        Some(value) => flag(&at(name), value),
        None => Ok(false),
    };
    Ok(CombatTerms {
        hit_points,
        armour,
        sight,
        weapon,
        secondary,
        elite,
        air_range_bonus,
        land_targeting,
        naval_targeting,
        underwater: optional("underwater", entry.underwater)?,
        organic: optional("organic", entry.organic)?,
        wall: optional("wall", entry.wall)?,
    })
}

fn validate(
    file: CombatFile,
    units: &[String],
    buildings: &[String],
) -> Result<CombatDefinitions, Problem> {
    if file.schema_version != COMBAT_SCHEMA_VERSION {
        return Err((
            "schema_version".into(),
            DataErrorKind::UnsupportedSchemaVersion {
                found: file.schema_version,
                expected: COMBAT_SCHEMA_VERSION,
            },
        ));
    }
    let armour_classes = unique_names(
        "armour_classes",
        file.armour_classes
            .iter()
            .enumerate()
            .map(|(index, name)| (format!("armour_classes[{index}]"), name.as_str())),
    )?;
    let warhead_ids = unique_names(
        "warheads",
        file.warheads
            .iter()
            .enumerate()
            .map(|(index, entry)| (format!("warheads[{index}].id"), entry.id.as_str())),
    )?;
    let weapon_ids = unique_names(
        "weapons",
        file.weapons
            .iter()
            .enumerate()
            .map(|(index, entry)| (format!("weapons[{index}].id"), entry.id.as_str())),
    )?;

    let mut warheads = Vec::with_capacity(file.warheads.len());
    for (index, entry) in file.warheads.iter().enumerate() {
        let field = format!("warheads[{index}].versus");
        let mut versus: Vec<Option<i32>> = vec![None; armour_classes.len()];
        for (position, pair) in entry.versus.iter().enumerate() {
            let at = |name: &str| format!("{field}[{position}].{name}");
            let class = usize::from(reference(
                at("armour"),
                "armour class",
                &armour_classes,
                &pair.armour,
            )?);
            let percent = ranged(&at("percent"), pair.percent, 0, MAX_VERSUS_PERCENT, " %")?;
            let slot = &mut versus[class];
            if slot.is_some() {
                return Err((
                    at("armour"),
                    DataErrorKind::DuplicateId(pair.armour.clone()),
                ));
            }
            *slot = Some(percent);
        }
        // Every armour class exactly once: no silent default percentage.
        let versus = versus
            .into_iter()
            .zip(&armour_classes)
            .map(|(percent, class)| {
                percent.ok_or_else(|| {
                    (
                        field.clone(),
                        DataErrorKind::MissingEntry {
                            kind: "armour class".into(),
                            id: class.clone(),
                        },
                    )
                })
            })
            .collect::<Result<Vec<i32>, Problem>>()?;
        let at = |name: &str| format!("warheads[{index}].{name}");
        warheads.push(WarheadDef {
            id: entry.id.clone(),
            versus,
            cell_spread: ranged(
                &at("cell_spread"),
                entry.cell_spread,
                0,
                MAX_CELL_SPREAD,
                " subcells",
            )?,
            percent_at_max: ranged(&at("percent_at_max"), entry.percent_at_max, 0, 100u32, " %")?,
            wall: flag(&at("wall"), entry.wall)?,
            wood: flag(&at("wood"), entry.wood)?,
            temporal: flag(&at("temporal"), entry.temporal)?,
            mind_control: flag(&at("mind_control"), entry.mind_control)?,
            psychic_damage: flag(&at("psychic_damage"), entry.psychic_damage)?,
            radiation: flag(&at("radiation"), entry.radiation)?,
            parasite: flag(&at("parasite"), entry.parasite)?,
            ivan_bomb: flag(&at("ivan_bomb"), entry.ivan_bomb)?,
            bomb_disarm: flag(&at("bomb_disarm"), entry.bomb_disarm)?,
            electric_assault: flag(&at("electric_assault"), entry.electric_assault)?,
            paralyzes: ranged(&at("paralyzes"), entry.paralyzes, 0, 32767u32, "")?,
            culling: flag(&at("culling"), entry.culling)?,
            makes_disguise: flag(&at("makes_disguise"), entry.makes_disguise)?,
        });
    }

    let mut weapons = Vec::with_capacity(file.weapons.len());
    for (index, entry) in file.weapons.iter().enumerate() {
        let field = format!("weapons[{index}]");
        let at = |name: &str| format!("{field}.{name}");
        let damage = ranged(
            &at("damage"),
            entry.damage,
            if entry.repair == 1 { -MAX_DAMAGE } else { 1 },
            if entry.repair == 1 { -1 } else { MAX_DAMAGE },
            "",
        )?;
        let range = ranged(
            &at("range"),
            entry.range,
            if entry.area_fire == 1 || entry.suicide == 1 {
                256
            } else {
                MIN_WEAPON_RANGE
            },
            MAX_WEAPON_RANGE,
            " subcells",
        )?;
        let min_range = ranged(&at("min_range"), entry.min_range, 0, MAX_WEAPON_RANGE, "")
            .ok()
            .filter(|&min| min < range)
            .ok_or_else(|| {
                invalid(
                    at("min_range"),
                    entry.min_range,
                    format!("0 to {} subcells, below the range", range - 1),
                )
            })?;
        let reload = ranged(&at("reload"), entry.reload, 1, MAX_RELOAD, " ticks")?;
        let burst = ranged(&at("burst"), entry.burst, 1, MAX_BURST, " shots")?;
        let warhead = WarheadKind(reference(
            at("warhead"),
            "warhead",
            &warhead_ids,
            &entry.warhead,
        )?);
        let projectile_speed = if entry.projectile_speed == 0 {
            0
        } else {
            ranged(
                &at("projectile_speed"),
                entry.projectile_speed,
                MIN_PROJECTILE_SPEED,
                MAX_PROJECTILE_SPEED,
                " subcells per tick, or 0 for an instant weapon",
            )?
        };
        let anti_air = flag(&at("anti_air"), entry.anti_air)?;
        let anti_ground = flag(&at("anti_ground"), entry.anti_ground)?;
        if !anti_air && !anti_ground {
            return Err(invalid(
                at("anti_ground"),
                entry.anti_ground,
                "1 for a weapon whose anti_air is 0: a weapon engages aircraft, the ground, or both",
            ));
        }
        let scatter = ranged(&at("scatter"), entry.scatter, 0, MAX_SCATTER, " subcells")?;
        weapons.push(WeaponDef {
            id: entry.id.clone(),
            damage,
            range,
            min_range,
            reload,
            burst,
            warhead,
            projectile_speed,
            anti_air,
            anti_ground,
            scatter,
            rad_level: ranged(&at("rad_level"), entry.rad_level, 0, 10000u32, "")?,
            area_fire: flag(&at("area_fire"), entry.area_fire)?,
            naval_only: flag(&at("naval_only"), entry.naval_only)?,
            sonic: flag(&at("sonic"), entry.sonic)?,
            repair: flag(&at("repair"), entry.repair)?,
            suicide: flag(&at("suicide"), entry.suicide)?,
        });
    }

    let mut unit_terms: Vec<Option<CombatTerms>> = vec![None; units.len()];
    for (index, entry) in file.units.iter().enumerate() {
        let field = format!("units[{index}]");
        let kind = usize::from(reference(
            format!("{field}.unit"),
            "unit",
            units,
            &entry.unit,
        )?);
        if unit_terms[kind].is_some() {
            return Err((
                format!("{field}.unit"),
                DataErrorKind::DuplicateId(entry.unit.clone()),
            ));
        }
        unit_terms[kind] = Some(terms(
            &field,
            &TermsEntry {
                hit_points: entry.hit_points,
                armour: &entry.armour,
                sight: entry.sight,
                weapons: &entry.weapons,
                elite_weapons: &entry.elite_weapons,
                air_range_bonus: entry.air_range_bonus,
                land_targeting: entry.land_targeting,
                naval_targeting: entry.naval_targeting,
                underwater: Some(entry.underwater),
                organic: Some(entry.organic),
                wall: None,
            },
            &armour_classes,
            &weapon_ids,
        )?);
    }
    let mut building_terms: Vec<Option<CombatTerms>> = vec![None; buildings.len()];
    for (index, entry) in file.buildings.iter().enumerate() {
        let field = format!("buildings[{index}]");
        let kind = usize::from(reference(
            format!("{field}.building"),
            "building",
            buildings,
            &entry.building,
        )?);
        if building_terms[kind].is_some() {
            return Err((
                format!("{field}.building"),
                DataErrorKind::DuplicateId(entry.building.clone()),
            ));
        }
        building_terms[kind] = Some(terms(
            &field,
            &TermsEntry {
                hit_points: entry.hit_points,
                armour: &entry.armour,
                sight: entry.sight,
                weapons: &entry.weapons,
                elite_weapons: &entry.elite_weapons,
                air_range_bonus: entry.air_range_bonus,
                land_targeting: entry.land_targeting,
                naval_targeting: entry.naval_targeting,
                underwater: None,
                organic: None,
                wall: Some(entry.wall),
            },
            &armour_classes,
            &weapon_ids,
        )?);
    }
    // Every unit and building type has combat terms (cross validation, PLAN.md section 5).
    for (list, kind, ids, entries) in [
        ("units", "unit", units, &unit_terms),
        ("buildings", "building", buildings, &building_terms),
    ] {
        if let Some((id, _)) = ids.iter().zip(entries).find(|(_, terms)| terms.is_none()) {
            return Err((
                list.to_owned(),
                DataErrorKind::MissingEntry {
                    kind: kind.to_owned(),
                    id: id.clone(),
                },
            ));
        }
    }

    let mut effects = [None; 9];
    for (index, entry) in file.effects.iter().enumerate() {
        let at = |name: &str| format!("effects[{index}].{name}");
        let Some(effect) = Effect::ALL
            .into_iter()
            .find(|effect| effect.id() == entry.id)
        else {
            let known: Vec<&str> = Effect::ALL.iter().map(|effect| effect.id()).collect();
            return Err(invalid(
                at("id"),
                format!("{:?}", entry.id),
                format!("one of {}", known.join(", ")),
            ));
        };
        let slot = &mut effects[effect.index()];
        if slot.is_some() {
            return Err((at("id"), DataErrorKind::DuplicateId(entry.id.clone())));
        }
        *slot = Some(EffectDef {
            damage: ranged(&at("damage"), entry.damage, 1, MAX_DAMAGE, "")?,
            warhead: WarheadKind(reference(
                at("warhead"),
                "warhead",
                &warhead_ids,
                &entry.warhead,
            )?),
        });
    }

    Ok(CombatDefinitions {
        armour_classes,
        warheads,
        weapons,
        units: unit_terms,
        buildings: building_terms,
        effects,
    })
}
