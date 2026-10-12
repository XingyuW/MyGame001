//! Specialist controls use the same command sink, selection and map hit testing as production.
use crate::{
    construction::InterfaceMode,
    map::HoveredCell,
    selection::Selection,
    sim_host::{CommandSink, SimHost},
    units::HUMAN,
};
use bevy::prelude::*;
use sim::{Building, Command, Owner, ProductionItem, SimId, SpecialState};
pub struct SpecialControlsPlugin;
#[derive(Component)]
struct StrategicPanel;
#[derive(Component)]
struct StrategicButton(SimId);
#[derive(Component)]
struct StrategicText(SimId);
#[derive(Component)]
struct ActionHint;
impl Plugin for SpecialControlsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StrategicEffects>()
            .add_systems(Startup, spawn.after(crate::skirmish::load_catalogue))
            .add_systems(
                Update,
                (panel, feedback_and_effects)
                    .chain()
                    .after(crate::skirmish::update_screens),
            )
            .add_systems(
                PostUpdate,
                input
                    .after(crate::construction::update_placement_ghost)
                    .after(crate::map::update_hovered_cell)
                    .before(crate::hover_info::show_hover_label),
            );
    }
}
fn spawn(mut commands: Commands) {
    commands.spawn((
        StrategicPanel,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            bottom: Val::Px(12.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        },
    ));
    commands.spawn((
        ActionHint,
        Text::new("D: deploy / unload    E: interact"),
        TextFont::from_font_size(10.0),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            bottom: Val::Px(12.0),
            ..default()
        },
    ));
}
fn panel(
    mut commands: Commands,
    host: Res<SimHost>,
    panels: Query<Entity, With<StrategicPanel>>,
    buttons: Query<(Entity, &StrategicButton)>,
    mut texts: Query<(&StrategicText, &mut Text)>,
    mut hints: Query<&mut Visibility, With<ActionHint>>,
) {
    for mut v in &mut hints {
        *v = if host.running() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    let Some(defs) = host.definitions() else {
        return;
    };
    let world = host.world();
    let state = world.get_resource::<SpecialState>();
    let mut owned = Vec::new();
    for e in world.iter_entities() {
        if e.get::<Owner>().is_some_and(|o| o.0 == HUMAN)
            && let (Some(id), Some(b)) = (e.get::<SimId>(), e.get::<Building>())
            && defs
                .metadata(ProductionItem::Building(b.kind))
                .is_some_and(|m| m.role.superweapon())
        {
            owned.push((*id, b.kind));
        }
    }
    owned.sort_unstable();
    for (e, b) in &buttons {
        if !owned.iter().any(|(id, _)| *id == b.0) {
            commands.entity(e).despawn();
        }
    }
    for (id, _) in &owned {
        if !buttons.iter().any(|(_, b)| b.0 == *id)
            && let Ok(panel) = panels.single()
        {
            commands.entity(panel).with_children(|p| {
                p.spawn((
                    StrategicButton(*id),
                    Button,
                    BackgroundColor(Color::srgb(0.14, 0.24, 0.25)),
                    Node {
                        width: Val::Px(290.0),
                        height: Val::Px(28.0),
                        padding: UiRect::all(Val::Px(4.0)),
                        ..default()
                    },
                ))
                .with_child((
                    StrategicText(*id),
                    Text::new("Charging"),
                    TextFont::from_font_size(10.0),
                ));
            });
        }
    }
    for (t, mut text) in &mut texts {
        if let Some((_, k)) = owned.iter().find(|(id, _)| *id == t.0)
            && let Some(m) = defs.metadata(ProductionItem::Building(*k))
        {
            let charge = state.and_then(|s| s.charge.get(&t.0)).copied().unwrap_or(0);
            let low = host
                .players()
                .and_then(|p| p.get(HUMAN))
                .is_some_and(|p| p.low_power());
            text.0 = format!(
                "{} · {}",
                m.label,
                if low {
                    "LOW POWER".into()
                } else if charge >= m.cooldown {
                    "READY — target".into()
                } else {
                    format!("{}s", m.cooldown.saturating_sub(charge).div_ceil(20))
                }
            );
        }
    }
}
pub fn deploy_commands(selection: &Selection) -> Vec<Command> {
    selection
        .0
        .iter()
        .map(|&unit| Command::Deploy { unit })
        .collect()
}
pub fn interact_commands(selection: &Selection, target: SimId) -> Vec<Command> {
    selection
        .0
        .iter()
        .filter(|&&id| id != target)
        .map(|&unit| Command::Interact { unit, target })
        .collect()
}
pub fn strategic_command(building: SimId, target: sim::CellPos, selection: &Selection) -> Command {
    Command::Superweapon {
        building,
        target,
        units: selection.0.iter().copied().collect(),
    }
}
#[allow(clippy::too_many_arguments)]
fn input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    hovered: Res<HoveredCell>,
    selection: Res<Selection>,
    host: Res<SimHost>,
    mut sink: ResMut<CommandSink>,
    mut mode: ResMut<InterfaceMode>,
    buttons: Query<(&StrategicButton, &Interaction), Changed<Interaction>>,
) {
    if !host.running() || sink.paused() {
        return;
    }
    for (b, i) in &buttons {
        if *i == Interaction::Pressed {
            *mode = InterfaceMode::Strategic(b.0);
            return;
        }
    }
    if keys.just_pressed(KeyCode::KeyD) {
        sink.issue(deploy_commands(&selection));
    }
    if keys.just_pressed(KeyCode::KeyE) {
        *mode = InterfaceMode::Interact;
    }
    if keys.just_pressed(KeyCode::Escape) || mouse.just_pressed(MouseButton::Right) {
        if matches!(*mode, InterfaceMode::Interact | InterfaceMode::Strategic(_)) {
            *mode = InterfaceMode::Normal;
        }
        return;
    }
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(cell) = hovered.0 else {
        return;
    };
    let issued = match *mode {
        InterfaceMode::Strategic(building) => {
            sink.issue([strategic_command(building, cell, &selection)])
        }
        InterfaceMode::Interact => {
            let target = host.grid().and_then(|g| {
                g.occupant(cell)
                    .or_else(|| g.occupant_for(cell, sim::Domain::Air))
                    .or_else(|| g.building(cell))
            });
            target.map_or(0, |t| sink.issue(interact_commands(&selection, t)))
        }
        _ => 0,
    };
    if issued > 0 {
        *mode = InterfaceMode::Normal;
    }
}

#[derive(Resource, Default)]
struct StrategicEffects(Vec<(sim::CellPos, sim::Role, u32, f32)>);
#[allow(clippy::too_many_arguments)]
fn feedback_and_effects(
    mut events: MessageReader<sim::SimEvent>,
    host: Res<SimHost>,
    time: Res<Time>,
    mut effects: ResMut<StrategicEffects>,
    mut hint: Query<&mut Text, With<ActionHint>>,
    mut remaining: Local<f32>,
    view: Option<Res<crate::map::MapView>>,
    mut gizmos: Gizmos,
) {
    *remaining = (*remaining - time.delta_secs()).max(0.0);
    for event in events.read() {
        match event {
            sim::SimEvent::CommandRejected { player, reason, .. } if *player == HUMAN => {
                if let Ok(mut t) = hint.single_mut() {
                    t.0 = reason.to_string();
                    *remaining = 5.0;
                }
            }
            sim::SimEvent::StrategicActivated {
                player,
                target,
                role,
                radius,
                ..
            } if *player == HUMAN
                || host
                    .players()
                    .and_then(|p| p.get(HUMAN))
                    .is_some_and(|p| p.vision().is_visible(*target)) =>
            {
                effects.0.push((*target, *role, *radius, 3.0))
            }
            _ => {}
        }
    }
    if *remaining == 0.0
        && let Ok(mut t) = hint.single_mut()
    {
        t.0 = "D: deploy / unload    E: interact".into();
    }
    effects.0.retain_mut(|(_, _, _, t)| {
        *t -= time.delta_secs();
        *t > 0.0
    });
    let Some(view) = view else {
        return;
    };
    for &(cell, role, radius, t) in &effects.0 {
        let center =
            crate::projection::to_world(cell.x as f32 + 0.5, cell.y as f32 + 0.5, view.tile);
        let color = match role {
            sim::Role::Nuke => Color::srgba(1.0, 0.5, 0.1, t / 3.0),
            sim::Role::Storm => Color::srgba(0.65, 0.8, 1.0, t / 3.0),
            sim::Role::Curtain => Color::srgba(1.0, 0.15, 0.15, t / 3.0),
            _ => Color::srgba(0.1, 1.0, 1.0, t / 3.0),
        };
        let extent = radius as f32 * view.tile.width * 0.5 * (1.0 - t / 4.0);
        gizmos.ellipse_2d(
            Isometry2d::from_translation(center),
            Vec2::new(extent, extent * 0.5),
            color,
        );
        if role == sim::Role::Storm {
            for x in [-0.7, 0.0, 0.7] {
                let p = center + Vec2::new(extent * x, 0.0);
                gizmos.linestrip_2d(
                    [
                        p + Vec2::new(0.0, 140.0),
                        p + Vec2::new(-20.0, 80.0),
                        p + Vec2::new(10.0, 65.0),
                        p,
                    ],
                    color,
                );
            }
        }
    }
    if let Some(s) = host.world().get_resource::<SpecialState>() {
        for zone in &s.zones {
            if host
                .players()
                .and_then(|p| p.get(HUMAN))
                .is_some_and(|p| p.vision().is_visible(zone.cell))
            {
                let c = crate::projection::to_world(
                    zone.cell.x as f32 + 0.5,
                    zone.cell.y as f32 + 0.5,
                    view.tile,
                );
                gizmos.ellipse_2d(
                    Isometry2d::from_translation(c),
                    Vec2::new(zone.radius as f32 * 32.0, zone.radius as f32 * 16.0),
                    Color::srgba(0.5, 1.0, 0.2, 0.6),
                );
            }
        }
    }
}
