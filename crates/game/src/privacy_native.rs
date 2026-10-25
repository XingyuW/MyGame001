//! Shared test-only setup for actual scheduled client privacy controls. Image handles and
//! cursor geometry are controlled inputs; actors, concealment, ownership and damage are native.
use bevy::ecs::message::Messages;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use sim::{CellPos, Command, IssuedCommand, PlayerId, SimEvent, SimId};

pub(crate) const HUMAN: PlayerId = PlayerId(0);
pub(crate) const ENEMY: PlayerId = PlayerId(1);

pub(crate) fn entity(host: &crate::sim_host::SimHost, id: SimId) -> Entity {
    host.world()
        .iter_entities()
        .find(|e| e.get::<SimId>() == Some(&id))
        .expect("native actor exists")
        .id()
}

pub(crate) fn cell(host: &crate::sim_host::SimHost, id: SimId) -> CellPos {
    host.world()
        .get::<sim::Mobile>(entity(host, id))
        .expect("mobile")
        .cell
}

pub(crate) fn issue(player: PlayerId, command: Command) -> IssuedCommand {
    IssuedCommand { player, command }
}

pub(crate) fn step(app: &mut App, commands: Vec<IssuedCommand>) -> Vec<SimEvent> {
    let mut host = app
        .world_mut()
        .remove_resource::<crate::sim_host::SimHost>()
        .expect("host");
    let events = crate::sim_host::tests::privacy_step(&mut host, commands);
    crate::sim_host::tests::assert_native_cells_for_test(&mut host);
    app.world_mut().insert_resource(host);
    app.world_mut()
        .resource_mut::<Messages<SimEvent>>()
        .write_batch(events.clone());
    events
}

pub(crate) fn move_to(app: &mut App, player: PlayerId, unit: SimId, target: CellPos) {
    let events = step(
        app,
        vec![issue(
            player,
            Command::Move {
                units: vec![unit],
                target,
            },
        )],
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
    for _ in 0..2400 {
        let host = app.world().resource::<crate::sim_host::SimHost>();
        let mobile = host
            .world()
            .get::<sim::Mobile>(entity(host, unit))
            .expect("moving actor");
        if mobile.cell == target && mobile.step.is_none() {
            return;
        }
        step(app, Vec::new());
    }
    panic!("native move did not arrive at {target:?}");
}

pub(crate) fn hidden(app: &App, id: SimId) -> bool {
    app.world()
        .resource::<crate::sim_host::SimHost>()
        .world()
        .resource::<sim::SpecialState>()
        .hidden
        .contains(&(HUMAN, id))
}

pub(crate) fn app(host: crate::sim_host::SimHost) -> App {
    use crate::{
        construction::InterfaceMode,
        map::{HoveredCell, MapTerrain, MapView},
        selection::{DragBox, OrderFeedback, OrderPreview, SelectedBuilding, Selection},
        sim_host::CommandSink,
        units::{Altitudes, UnitSprites, UnitViews},
    };
    let grid = host.grid().expect("grid").clone();
    let mut sprites = UnitSprites::default();
    for i in 0..host.definitions().expect("definitions").units().len() {
        let kind = sim::UnitKind(i as u16);
        sprites.0.insert(kind, Handle::default());
        sprites.1.insert(kind, Handle::default());
    }
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(host)
        .insert_resource(MapView {
            width: grid.width(),
            height: grid.height(),
            tile: crate::projection::TileSize::new(64, 32),
        })
        .insert_resource(MapTerrain(grid))
        .insert_resource(sprites)
        .init_resource::<Assets<Image>>()
        .init_resource::<Time<Fixed>>()
        .init_resource::<crate::game_speed::GameClock>()
        .init_resource::<CommandSink>()
        .init_resource::<InterfaceMode>()
        .init_resource::<Selection>()
        .init_resource::<SelectedBuilding>()
        .init_resource::<DragBox>()
        .init_resource::<OrderFeedback>()
        .init_resource::<OrderPreview>()
        .init_resource::<HoveredCell>()
        .init_resource::<UnitViews>()
        .init_resource::<Altitudes>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_message::<SimEvent>();
    let mut window = Window {
        resolution: (1280, 800).into(),
        focused: true,
        ..default()
    };
    window.set_cursor_position(Some(Vec2::new(640.0, 400.0)));
    app.world_mut().spawn((window, PrimaryWindow));
    app.world_mut().spawn((
        Camera {
            computed: bevy::camera::ComputedCameraValues {
                clip_from_view: Mat4::orthographic_rh(-640.0, 640.0, -400.0, 400.0, 0.0, 1000.0),
                target_info: Some(bevy::camera::RenderTargetInfo {
                    physical_size: UVec2::new(1280, 800),
                    scale_factor: 1.0,
                }),
                ..default()
            },
            ..default()
        },
        GlobalTransform::default(),
    ));
    app
}

/// Only cursor geometry is controlled. View creation and placement are the production systems.
pub(crate) fn hover_unit(app: &mut App, id: SimId) {
    crate::units::privacy_place_views(app);
    let view = app.world().resource::<crate::units::UnitViews>().0[&id];
    let at = app
        .world()
        .get::<Transform>(view)
        .expect("placed view")
        .translation;
    let world = app.world_mut();
    let mut camera = world.query_filtered::<&mut GlobalTransform, With<Camera>>();
    *camera.single_mut(world).expect("camera") = GlobalTransform::from_translation(at);
    let cell = cell(world.resource::<crate::sim_host::SimHost>(), id);
    world.resource_mut::<crate::map::HoveredCell>().0 = Some(cell);
}

pub(crate) fn hover_cell(app: &mut App, cell: CellPos) {
    let tile = app.world().resource::<crate::map::MapView>().tile;
    let at = crate::projection::cell_centre(cell, tile);
    let world = app.world_mut();
    let mut camera = world.query_filtered::<&mut GlobalTransform, With<Camera>>();
    *camera.single_mut(world).expect("camera") = GlobalTransform::from_translation(at.extend(0.0));
    world.resource_mut::<crate::map::HoveredCell>().0 = Some(cell);
}

pub(crate) fn gizmos(app: &mut App) {
    use bevy::gizmos::{AppGizmoBuilder, GizmoAsset, config::DefaultGizmoConfigGroup};
    app.init_resource::<Assets<GizmoAsset>>()
        .init_gizmo_group::<DefaultGizmoConfigGroup>();
}
