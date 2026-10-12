//! Desktop client: rendering, input, interface, and audio on top of the `sim` crate.
//!
//! The simulation runs in a world of its own inside `sim_host::SimHost`, which the rest of the
//! game can only read; `unsafe` code is forbidden so that this boundary cannot be bypassed
//! (docs/DECISIONS.md D34).

#![forbid(unsafe_code)]

mod audio;
mod battlefield;
mod buildings;
mod camera;
mod checked_assets;
mod cli;
mod construction;
mod fog;
mod game_speed;
mod hd_art;
mod health;
mod hotkeys;
mod hover_info;
mod map;
mod minimap;
mod notices;
mod options;
mod presentation;
mod projection;
mod selection;
mod sidebar;
mod sim_host;
mod skirmish;
mod smoke;
mod special_controls;
mod tileset_image;
mod units;
mod visual_review;
mod weapon_fx;

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
    app.insert_resource(ClearColor(Color::BLACK));
    app.add_plugins(
        DefaultPlugins
            .build()
            // The renderer reads only checked bytes (docs/DECISIONS.md, D16); the default asset
            // source must be replaced before `AssetPlugin` builds the sources.
            .add_before::<AssetPlugin>(checked_assets::CheckedAssetsPlugin)
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "MyGame001".into(),
                    resolution: (1280, 800).into(),
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
        battlefield::BattlefieldPlugin,
        map::MapPlugin,
        units::UnitsPlugin,
        buildings::BuildingsPlugin,
        selection::SelectionPlugin,
        construction::ConstructionPlugin,
        fog::FogPlugin,
        sidebar::SidebarPlugin,
        special_controls::SpecialControlsPlugin,
        minimap::MinimapPlugin,
        health::HealthBarsPlugin,
        audio::GameAudioPlugin,
        skirmish::SkirmishPlugin,
        game_speed::GameSpeedPlugin,
    ))
    .add_plugins((
        hover_info::HoverInfoPlugin,
        notices::NoticesPlugin,
        weapon_fx::WeaponFxPlugin,
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
    } else if options.smoke_test_frames.is_none()
        && let Some(directory) = std::env::var_os("MYGAME_MAP_REVIEW")
    {
        visual_review::enable_maps(&mut app, std::path::PathBuf::from(directory));
    } else if options.smoke_test_frames.is_none()
        && let Some(directory) = std::env::var_os("MYGAME_FEATURE_REVIEW")
    {
        visual_review::enable_features(&mut app, std::path::PathBuf::from(directory));
    } else if options.smoke_test_frames.is_none()
        && let Some(directory) = std::env::var_os("MYGAME_FX_REVIEW")
    {
        visual_review::enable_fx(&mut app, std::path::PathBuf::from(directory));
    }
    app.run()
}
