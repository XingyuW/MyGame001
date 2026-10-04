//! Game rules for the isometric RTS.
//!
//! All simulation state lives in a Bevy [`World`](bevy_ecs::world::World) and is advanced only by
//! running [`SimSchedule`], one run per tick. Input sources (interface, AI, tests) change state
//! only by pushing [`IssuedCommand`] values into the [`CommandQueue`].
//!
//! This crate uses no floating point types (invariant I2) and depends on no rendering,
//! windowing, audio, or input crate (invariant I1).

mod building;
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
mod movement;
mod pathfinding;
mod rng;
mod schedule;
mod unit;

pub use building::{
    Building, Footprint, PlacementError, Producer, SpawnBuildingError, spawn_building,
};

pub use command::{
    Command, CommandQueue, IssuedCommand, MAX_UNITS_PER_COMMAND, PlayerId, RejectReason,
};
pub use data::{DataError, DataErrorKind, MAX_DATA_FILE_BYTES, MAX_STRING_BYTES};
pub use definitions::{
    Definitions, MAX_NAME_LEN, MAX_UNIT_SPEED, MAX_UNIT_TYPES, UNITS_SCHEMA_VERSION, UnitDef,
    UnitKind, load_definitions, load_unit_definitions,
};
pub use economy::{MAX_QUEUE_LENGTH, PlayerSetupError, PlayerState, Players, QueueItem};
pub use economy_defs::{
    BuildingDef, BuildingKind, ECONOMY_SCHEMA_VERSION, HarvesterDef, MAX_BUILD_TIME,
    MAX_BUILDING_TYPES, MAX_COST, MAX_CREDITS_PER_ORE, MAX_FOOTPRINT_SIDE, MAX_FREE_UNITS,
    MAX_ORE_CAPACITY, MAX_PLACEMENT_DISTANCE, MAX_POWER, MAX_PREREQUISITES, Production,
    ProductionItem,
};
pub use event::{HarvestIdle, MoveFailure, SimEvent};
pub use harvest::{HarvestState, Harvester, MAX_HARVEST_ATTEMPTS};
pub use hash::{Fnv1a64, state_hash};
pub use id::{SimId, SimIdAllocator};
pub use map::{
    Cell, CellPos, CheckedFile, Grid, LoadedMap, MAP_SCHEMA_VERSION, MAX_MAP_FILE_BYTES,
    MAX_MAP_SIDE, MAX_TILESET_IMAGE_SIDE_PX, MAX_TILESET_TILES, MapError, MapErrorKind,
    SUBCELLS_PER_CELL, SubPos, TERRAIN_LAYER, TILE_HEIGHT_PX, TILE_WIDTH_PX, TilesetImage,
    load_map, load_map_with_files,
};
pub use movement::{MAX_PATHS_WITHOUT_PROGRESS, MAX_STALLED_TICKS, REPATH_AFTER_BLOCKED_TICKS};
pub use pathfinding::{
    DIAGONAL_COST, ORTHOGONAL_COST, PATH_REQUESTS_PER_TICK, Path, destination_cells, find_path,
    nearest_reachable, octile_distance,
};
pub use rng::SimRng;
pub use schedule::{SimPlugin, SimSchedule, SimSet, SimTick, TICKS_PER_SECOND};
pub use unit::{
    DIAGONAL_STEP, Direction, Facing, Mobile, MoveOrder, ORTHOGONAL_STEP, Orders, Owner, PathState,
    Position, SpawnError, Step, Unit, spawn_unit,
};
