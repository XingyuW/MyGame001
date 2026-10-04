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
mod map_subset;
mod match_setup;
mod movement;
mod pathfinding;
mod rng;
mod schedule;
mod skirmish_defs;
mod unit;
mod visibility;

pub use ai::{
    AI_UNIT_QUEUE, AiController, AiPlayers, GATHER_DISTANCE, GATHER_RADIUS, HARVEST_RETRY_TICKS,
    KnownBuilding, MAX_TARGET_FAILURES, WaveTarget, Waypoint,
};
pub use ai_defs::{
    AI_SCHEMA_VERSION, AiDefinitions, Difficulty, MAX_AI_ARMY, MAX_ARMY_ENTRIES, MAX_BUILD_ORDER,
    MAX_DEFENCE_RADIUS, MAX_DIFFICULTIES, MAX_HARVESTERS_PER_REFINERY, MAX_THINK_INTERVAL,
    check_ai_factions, load_ai,
};
pub use building::{
    Building, Footprint, PlacementError, Producer, SpawnBuildingError, spawn_building,
};

pub use combat::{
    Armour, AttackOrder, Health, MAX_CHASES, Projectile, Projectiles, Sight, Weapon,
    applied_damage, in_range,
};
pub use combat_defs::{
    ArmourClass, COMBAT_SCHEMA_VERSION, CombatTerms, MAX_ARMOUR_CLASSES, MAX_DAMAGE,
    MAX_HIT_POINTS, MAX_PROJECTILE_SPEED, MAX_RELOAD, MAX_SIGHT, MAX_VERSUS_PERCENT, MAX_WARHEADS,
    MAX_WEAPON_RANGE, MAX_WEAPONS, MAX_WEAPONS_PER_TYPE, MIN_PROJECTILE_SPEED, MIN_WEAPON_RANGE,
    WarheadDef, WarheadKind, WeaponDef, WeaponKind,
};
pub use command::{
    Command, CommandQueue, IssuedCommand, MAX_UNITS_PER_COMMAND, PlayerId, RejectReason,
};
pub use data::{DataError, DataErrorKind, MAX_DATA_FILE_BYTES, MAX_STRING_BYTES};
pub use definitions::{
    Definitions, MAX_NAME_LEN, MAX_UNIT_SPEED, MAX_UNIT_TYPES, UNITS_SCHEMA_VERSION, UnitDef,
    UnitKind, load_definitions, load_match_definitions, load_unit_definitions,
};
pub use economy::{MAX_QUEUE_LENGTH, PlayerSetupError, PlayerState, Players, QueueItem};
pub use economy_commands::check_placement;
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
    Cell, CellPos, CheckedFile, Grid, LoadedMap, MAP_SCHEMA_VERSION, MAX_MAP_FILE_BYTES,
    MAX_MAP_SIDE, MAX_TILESET_IMAGE_SIDE_PX, MAX_TILESET_TILES, MapError, MapErrorKind,
    SUBCELLS_PER_CELL, SubPos, TERRAIN_LAYER, TILE_HEIGHT_PX, TILE_WIDTH_PX, TilesetImage,
    load_map, load_map_with_files,
};
pub use match_setup::{
    Controller, MatchOutcome, MatchSetupError, MatchStart, MatchState, PlayerSetup, StartError,
    plan_starting_units, setup_match,
};
pub use movement::{MAX_PATHS_WITHOUT_PROGRESS, MAX_STALLED_TICKS, REPATH_AFTER_BLOCKED_TICKS};
pub use pathfinding::{
    DIAGONAL_COST, ORTHOGONAL_COST, PATH_REQUESTS_PER_TICK, Path, destination_cells, find_path,
    nearest_reachable, octile_distance,
};
pub use rng::SimRng;
pub use schedule::{SimPlugin, SimSchedule, SimSet, SimTick, TICKS_PER_SECOND};
pub use skirmish_defs::{
    FactionDef, MATCH_PLAYERS, MAX_FACTIONS, MAX_GROUP_UNITS, MAX_MAPS, MAX_START_OFFSET,
    MAX_STARTING_BUILDINGS, MAX_STARTING_CREDITS, MAX_STARTING_GROUPS, MapDef,
    SKIRMISH_SCHEMA_VERSION, SkirmishDefinitions, StartProblem, StartingBuilding, StartingGroup,
    check_map_starts, load_skirmish, map_asset_path, start_problem,
};
pub use unit::{
    DIAGONAL_STEP, Direction, Facing, Mobile, MoveOrder, ORTHOGONAL_STEP, Orders, Owner, PathState,
    Position, SpawnError, Step, Unit, spawn_unit,
};
pub use visibility::Vision;
