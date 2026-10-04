//! Desktop client: rendering, input, interface, and audio on top of the `sim` crate.

mod buildings;
mod camera;
mod checked_assets;
mod cli;
mod construction;
mod fog;
mod map;
mod projection;
mod selection;
mod smoke;
mod tileset_image;
mod units;

use bevy::prelude::*;
use bevy_ecs_tiled::prelude::TiledPlugin;
use sim::{SimPlugin, SimSchedule, TICKS_PER_SECOND};

/// Seed for the simulation RNG until match setup (M6) chooses one.
const MATCH_SEED: u64 = 0;

fn main() -> AppExit {
    let options = match cli::parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("error: {error}\n{}", cli::USAGE);
            return AppExit::from_code(2);
        }
    };

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .build()
            // The renderer reads only checked bytes (docs/DECISIONS.md, D16); the default asset
            // source must be replaced before `AssetPlugin` builds the sources.
            .add_before::<AssetPlugin>(checked_assets::CheckedAssetsPlugin)
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "MyGame001".into(),
                    ..default()
                }),
                ..default()
            })
            .set(ImagePlugin::default_nearest()),
    )
    .add_plugins(TiledPlugin::default())
    .add_plugins(SimPlugin { seed: MATCH_SEED })
    .insert_resource(Time::<Fixed>::from_hz(f64::from(TICKS_PER_SECOND)))
    .add_systems(
        FixedUpdate,
        (units::record_previous_positions, run_sim_tick).chain(),
    )
    .add_plugins((
        camera::CameraPlugin,
        map::MapPlugin,
        units::UnitsPlugin,
        buildings::BuildingsPlugin,
        selection::SelectionPlugin,
        construction::ConstructionPlugin,
        fog::FogPlugin,
    ));

    if let Some(frames) = options.smoke_test_frames {
        app.add_plugins(smoke::SmokeTestPlugin { frames });
    }

    app.run()
}

/// Advances the simulation by exactly one tick per fixed timestep (20 Hz).
fn run_sim_tick(world: &mut World) {
    world.run_schedule(SimSchedule);
}
