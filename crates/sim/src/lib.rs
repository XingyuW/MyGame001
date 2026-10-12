//! Game rules for the isometric RTS.
//!
//! All simulation state lives in a Bevy [`World`](bevy_ecs::world::World) and is advanced only by
//! running [`SimSchedule`], one run per tick. Input sources (interface, AI, tests) change state
//! only by pushing [`IssuedCommand`] values into the [`CommandQueue`].
//!
//! This crate uses no floating point types (invariant I2) and depends on no rendering,
//! windowing, audio, or input crate (invariant I1).

mod ai;
mod ai_defs;
mod building;
mod combat;
mod combat_defs;
mod command;
mod data;
mod definitions;
mod economy;
mod economy_commands;
mod economy_defs;
mod event;
mod harvest;
mod hash;
mod id;
mod map;
mod map_scale;
mod map_subset;
mod match_setup;
mod movement;
mod pathfinding;
mod props;
mod rng;
mod rules;
mod schedule;
mod skirmish_defs;
mod unit;
mod visibility;

pub use ai::{
    AI_UNIT_QUEUE, ARMY_SCALE_SPAN, AiController, AiPlayers, GATHER_DISTANCE, GATHER_RADIUS,
    HARVEST_RETRY_TICKS, KnownBuilding, MAX_TARGET_FAILURES, WaveTarget, Waypoint, scaled_for_map,
};
pub use ai_defs::{
    AI_SCHEMA_VERSION, AiDefinitions, AiPlan, Difficulty, MAX_AI_ARMY, MAX_ARMY_ENTRIES,
    MAX_BUILD_ORDER, MAX_DEFENCE_RADIUS, MAX_DIFFICULTIES, MAX_HARVESTERS_PER_REFINERY, MAX_PLANS,
    MAX_THINK_INTERVAL, SidePlan, check_ai_factions, load_ai,
};
pub use building::{
    Building, Footprint, PlacementError, Producer, SpawnBuildingError, spawn_building,
};

pub use combat::{
    Armour, AttackOrder, Condition, DamageState, Health, MAX_CHASES, Projectile, Projectiles,
    Sight, TargetTraits, Weapon, applied_damage, damage_state, in_range, inside_min_range,
    scattered, splash_damage, versus_against, weapon_against,
};
pub use combat_defs::{
    ArmourClass, BURST_GAP, COMBAT_SCHEMA_VERSION, CombatTerms, Effect, EffectDef,
    MAX_AIR_RANGE_BONUS, MAX_ARMOUR_CLASSES, MAX_BURST, MAX_CELL_SPREAD, MAX_DAMAGE,
    MAX_HIT_POINTS, MAX_LAND_TARGETING, MAX_NAVAL_TARGETING, MAX_PROJECTILE_SPEED, MAX_RELOAD,
    MAX_SCATTER, MAX_SIGHT, MAX_VERSUS_PERCENT, MAX_WARHEADS, MAX_WEAPON_RANGE, MAX_WEAPONS,
    MAX_WEAPONS_PER_TYPE, MIN_PROJECTILE_SPEED, MIN_WEAPON_RANGE, WarheadDef, WarheadKind,
    WeaponDef, WeaponKind,
};
pub use command::{
    Command, CommandQueue, IssuedCommand, MAX_UNITS_PER_COMMAND, PlayerId, RejectReason,
};
pub use data::{DataError, DataErrorKind, MAX_DATA_FILE_BYTES, MAX_STRING_BYTES};
pub use definitions::{
    Definitions, MAX_NAME_LEN, MAX_UNIT_SPEED, MAX_UNIT_TYPES, UNITS_SCHEMA_VERSION, UnitDef,
    UnitKind, load_definitions, load_match_definitions, load_ruleset, load_unit_definitions,
};
pub use economy::{
    Lane, MAX_COUNTED_FACTORIES, MAX_PRODUCTION_RATE_PM, MAX_QUEUE_LENGTH, PlayerSetupError,
    PlayerState, Players, QueueItem, QueueKey, REPAIR_COST_DIVISOR, REPAIR_INTERVAL, REPAIR_STEPS,
    factory_rate_pm, power_rate_pm, production_rate_pm, repair_cost, repair_step_cost,
};
pub use economy_commands::{
    SHIPYARD_PLACEMENT_DISTANCE, check_buildable, check_placement, extends_base,
    missing_prerequisite, placement_reach, produces,
};
pub use economy_defs::{
    BuildingDef, BuildingKind, ECONOMY_SCHEMA_VERSION, HarvesterDef, MAX_BUILD_TIME,
    MAX_BUILDING_TYPES, MAX_COST, MAX_CREDITS_PER_ORE, MAX_FOOTPRINT_SIDE, MAX_FREE_UNITS,
    MAX_ORE_CAPACITY, MAX_PLACEMENT_DISTANCE, MAX_POWER, MAX_PREREQUISITES, Production,
    ProductionItem,
};
pub use event::{AttackEnd, HarvestIdle, MoveFailure, SimEvent};
pub use harvest::{HarvestState, Harvester, MAX_HARVEST_ATTEMPTS};
pub use hash::{Fnv1a64, state_hash};
pub use id::{SimId, SimIdAllocator};
pub use map::{
    CELL_ROOM, Cell, CellPos, CheckedFile, Grid, LoadedMap, MAP_SCHEMA_VERSION, MAX_MAP_FILE_BYTES,
    MAX_MAP_SIDE, MAX_TILESET_IMAGE_SIDE_PX, MAX_TILESET_TILES, MapError, MapErrorKind,
    SPOT_OFFSETS, SPOTS_PER_CELL, SUBCELLS_PER_CELL, SubPos, TERRAIN_LAYER, TILE_HEIGHT_PX,
    TILE_WIDTH_PX, TilesetImage, load_map, load_map_with_files,
};
pub use map_scale::{
    APRON_RADIUS, CORE_RADIUS, DEFAULT_ORE, EDGE_MARGIN, EnlargedMap, LOOK_FEATURE_BRIDGE,
    LOOK_FEATURE_ORE, LOOK_GROUND_AUTO, LOOK_GROUND_MASK, LOOK_GROUND_WATER, MAX_SITE_ATTEMPTS,
    MIN_ORE_CELLS, MIN_START_DISTANCE, MapScaleError, ORE_DISTANCE, ORE_RADIUS, PAD_CORNER,
    PAD_RADIUS, SCALE_DIVISOR, YARD_CENTRE_OFFSET, axis_map, enlarged_map, enlarged_size,
};
pub use match_setup::{
    AI_TEAM, Controller, MAX_UNIT_SPEED_PERCENT, MatchOptions, MatchOutcome, MatchRules,
    MatchSetupError, MatchStart, MatchState, PlayerSetup, STARTING_UNIT_DISTANCE, StartError,
    plan_starting_units, setup_match, setup_match_configured, setup_match_with_options,
    setup_match_with_super_me, starting_units,
};
pub use movement::{MAX_PATHS_WITHOUT_PROGRESS, MAX_STALLED_TICKS, REPATH_AFTER_BLOCKED_TICKS};
pub use pathfinding::{
    DIAGONAL_COST, ORTHOGONAL_COST, PATH_REQUESTS_PER_TICK, Path, destination_cells, find_path,
    nearest_reachable, octile_distance,
};
pub use props::{PROP_ID_BASE, Prop, PropKind, Props};
pub use rng::SimRng;
pub use rules::{
    CombatRules, Country, CountryId, CrateRules, CrewRules, DialogRules, DifficultyRules,
    EconomyRules, GroupId, MAX_COUNTRIES, MAX_GROUP_BUILDINGS, MAX_POWERUPS,
    MAX_PREREQUISITE_GROUPS, MAX_RULES_DIFFICULTIES, MAX_TECH_LEVEL, Powerup, PrerequisiteGroup,
    PrismRules, ProductionRules, RULES_SCHEMA_VERSION, RadiationRules, Rules, Side, SideRules,
    SuperweaponRules, VeterancyRules,
};
pub use schedule::{SimCoreSet, SimPlugin, SimSchedule, SimSet, SimTick, TICKS_PER_SECOND};
pub use skirmish_defs::{
    FactionDef, MATCH_PLAYERS, MAX_FACTIONS, MAX_GROUP_UNITS, MAX_LABEL_LEN, MAX_MAPS,
    MAX_MATCH_PLAYERS, MAX_ROSTER_ENTRIES, MAX_START_OFFSET, MAX_STARTING_BUILDINGS,
    MAX_STARTING_CREDITS, MAX_STARTING_GROUPS, MAX_STARTING_TYPES, MIN_MATCH_PLAYERS, MapDef,
    SKIRMISH_SCHEMA_VERSION, SideStart, SkirmishDefinitions, StartProblem, StartingBuilding,
    StartingGroup, check_map_starts, country_tree, load_skirmish, map_asset_path, start_problem,
};
pub use unit::{
    DIAGONAL_STEP, Direction, Facing, Mobile, MoveOrder, ORTHOGONAL_STEP, Orders, Owner, PathState,
    Position, SpawnError, Step, Unit, cell_invariant_violations, spawn_unit, stray_claims,
};
pub use visibility::Vision;

pub mod arsenal;
pub use arsenal::{
    ARSENAL_SCHEMA_VERSION, Ability, Arsenal, ArsenalEntry, Category, Domain, FactoryGroup,
    Launcher, MAX_AMMO, MAX_BUILD_LIMIT, MAX_DOCKS, MAX_SPAWN_TICKS, MAX_SPAWNS, MAX_UNIT_SIZE,
    Missile, Role,
};

pub mod special;
pub use special::SpecialState;
