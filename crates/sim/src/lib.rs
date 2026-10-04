//! Game rules for the isometric RTS.
//!
//! All simulation state lives in a Bevy [`World`](bevy_ecs::world::World) and is advanced only by
//! running [`SimSchedule`], one run per tick. Input sources (interface, AI, tests) change state
//! only by pushing [`IssuedCommand`] values into the [`CommandQueue`].
//!
//! This crate uses no floating point types (invariant I2) and depends on no rendering,
//! windowing, audio, or input crate (invariant I1).

mod command;
mod hash;
mod id;
mod map;
mod map_subset;
mod rng;
mod schedule;

pub use command::{Command, CommandQueue, IssuedCommand, PlayerId};
pub use hash::{Fnv1a64, state_hash};
pub use id::{SimId, SimIdAllocator};
pub use map::{
    Cell, CellPos, CheckedFile, Grid, LoadedMap, MAP_SCHEMA_VERSION, MAX_MAP_FILE_BYTES,
    MAX_MAP_SIDE, MAX_TILESET_IMAGE_SIDE_PX, MAX_TILESET_TILES, MapError, MapErrorKind,
    TERRAIN_LAYER, TILE_HEIGHT_PX, TILE_WIDTH_PX, TilesetImage, load_map, load_map_with_files,
};
pub use rng::SimRng;
pub use schedule::{SimPlugin, SimSchedule, SimSet, SimTick, TICKS_PER_SECOND};
