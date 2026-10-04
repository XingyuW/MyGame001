//! M0 spike: render an isometric diamond Tiled map through `bevy_ecs_tiled`.

use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;

pub const SPIKE_MAP: &str = "maps/spike/spike.tmx";

pub struct SpikePlugin;

impl Plugin for SpikePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpikeStatus>()
            .add_systems(Startup, spawn_spike)
            .add_systems(Update, on_map_created);
    }
}

/// What the spike has observed so far; read by the smoke test.
#[derive(Resource, Debug, Default)]
pub struct SpikeStatus {
    pub map_created: bool,
}

fn spawn_spike(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2d);
    commands.spawn((
        TiledMap(asset_server.load(SPIKE_MAP)),
        TilemapAnchor::Center,
    ));
}

/// Records map creation and logs the custom tile properties the map exposes (assumption A2).
fn on_map_created(
    mut created: MessageReader<TiledEvent<MapCreated>>,
    maps: Res<Assets<TiledMapAsset>>,
    mut status: ResMut<SpikeStatus>,
) {
    for event in created.read() {
        status.map_created = true;
        let Some(map) = event.get_map(&maps) else {
            warn!("spike map created but its asset is not available");
            continue;
        };
        info!(
            "spike map created: {:?} {}x{} cells, tile {}x{} px",
            map.orientation, map.width, map.height, map.tile_width, map.tile_height
        );
        for tileset in map.tilesets() {
            // `tiles()` iterates a hash map; sort so the log is reproducible.
            let mut tiles: Vec<_> = tileset.tiles().collect();
            tiles.sort_by_key(|(id, _)| *id);
            for (id, tile) in tiles {
                let mut properties: Vec<_> = tile.properties.iter().collect();
                properties.sort_by(|a, b| a.0.cmp(b.0));
                info!("tileset {:?} tile {id}: {properties:?}", tileset.name);
            }
        }
    }
}
