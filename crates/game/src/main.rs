//! Desktop client: rendering, input, interface, and audio on top of the `sim` crate.
//!
//! The simulation runs in a world of its own inside `sim_host::SimHost`, which the rest of the
//! game can only read; `unsafe` code is forbidden so that this boundary cannot be bypassed
//! (docs/DECISIONS.md D34).

#![forbid(unsafe_code)]

mod audio;
mod buildings;
mod camera;
mod checked_assets;
mod cli;
mod construction;
mod fog;
mod health;
mod hotkeys;
mod map;
mod minimap;
mod options;
mod presentation;
mod projection;
mod selection;
mod sidebar;
mod sim_host;
mod skirmish;
mod smoke;
mod tileset_image;
mod units;
mod visual_review;

use bevy::prelude::*;
use bevy_ecs_tiled::prelude::TiledPlugin;
use sim::TICKS_PER_SECOND;

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
    .add_plugins(sim_host::SimHostPlugin)
    .insert_resource(Time::<Fixed>::from_hz(f64::from(TICKS_PER_SECOND)))
    .add_plugins((
        camera::CameraPlugin,
        map::MapPlugin,
        units::UnitsPlugin,
        buildings::BuildingsPlugin,
        selection::SelectionPlugin,
        construction::ConstructionPlugin,
        fog::FogPlugin,
        sidebar::SidebarPlugin,
        minimap::MinimapPlugin,
        health::HealthBarsPlugin,
        audio::GameAudioPlugin,
        skirmish::SkirmishPlugin,
    ))
    // A smoke run stores its settings in a file of its own, which it removes, so that it never
    // reads or changes the player's settings (docs/DECISIONS.md D52).
    .add_plugins(crate::options::OptionsPlugin {
        path: match options.smoke_test_frames {
            Some(_) => Some(smoke::settings_path()),
            None => crate::options::default_settings_path(),
        },
    });

    if let Some(frames) = options.smoke_test_frames {
        app.add_plugins(smoke::SmokeTestPlugin { frames });
    }

    if options.smoke_test_frames.is_none()
        && let Some(directory) = std::env::var_os("MYGAME_VISUAL_REVIEW")
    {
        visual_review::enable(&mut app, std::path::PathBuf::from(directory));
    }
    app.run()
}
