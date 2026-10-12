//! Specialist controls use the same command sink, selection and map hit testing as production.
//!
//! Since RA1 a selected Construction Yard can be packed up into the base unit of its side
//! (`Command::Undeploy`) while the match has "MCV repacks" on: with `D`, or with the Repack
//! button that then stands over the superweapon buttons ([`repack_command`]). With the option
//! off there is neither the button nor the command.
use crate::{
    construction::InterfaceMode,
    map::HoveredCell,
    selection::{SelectedBuilding, Selection},
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
/// The button that packs up the selected Construction Yard; it exists only while one is
/// selected and the match has "MCV repacks" on.
#[derive(Component)]
struct RepackButton;

/// What the hint line says while nothing else is to be said.
const HINT: &str = "D: deploy / unload    E: interact";
/// The hint while a Construction Yard that can be packed up is selected.
const REPACK_HINT: &str = "D: repack the Construction Yard into its MCV    E: interact";
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
            // Above the hint line, which stands in the corner.
            bottom: Val::Px(30.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        },
    ));
    commands.spawn((
        ActionHint,
        Text::new(HINT),
        TextFont::from_font_size(10.0),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            bottom: Val::Px(12.0),
            ..default()
        },
    ));
}
#[allow(clippy::too_many_arguments)]
fn panel(
    mut commands: Commands,
    host: Res<SimHost>,
    selection: Res<Selection>,
    selected: Res<SelectedBuilding>,
    panels: Query<Entity, With<StrategicPanel>>,
    buttons: Query<(Entity, &StrategicButton)>,
    repack: Query<Entity, With<RepackButton>>,
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
    // The Repack button: there exactly while D would pack up the selected Construction Yard.
    let offered = host.running() && repack_target(&host, &selection, &selected).is_some();
    match (offered, repack.iter().next()) {
        (true, None) => {
            if let Ok(panel) = panels.single() {
                commands.entity(panel).with_children(|p| {
                    p.spawn((
                        RepackButton,
                        Button,
                        BackgroundColor(Color::srgb(0.30, 0.24, 0.10)),
                        Node {
                            width: Val::Px(290.0),
                            height: Val::Px(28.0),
                            padding: UiRect::all(Val::Px(4.0)),
                            ..default()
                        },
                    ))
                    .with_child((
                        Text::new("Repack the Construction Yard (D)"),
                        TextFont::from_font_size(10.0),
                    ));
                });
            }
        }
        (false, Some(button)) => commands.entity(button).despawn(),
        _ => {}
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
            // The charge waits for power: without a rules file always, with one for a
            // superweapon that `is_powered` (RA2).
            let low = (defs.rules().is_none() || m.is_powered)
                && host
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
/// `Undeploy` for the player's Construction Yard `building` (RA1, "MCV repacks"): it becomes the
/// base unit of its side again.
pub fn undeploy_command(building: SimId) -> Command {
    Command::Undeploy { building }
}
/// What `D` and the Repack button do with the selected building `selected`: pack it up
/// ([`undeploy_command`]) if it is one of the player's Construction Yards (`yards`), no unit is
/// selected (`units`: `D` deploys those instead) and the match has "MCV repacks" on (`repacks`).
/// Otherwise nothing: with the option off the control does not exist.
pub fn repack_command(
    selected: Option<SimId>,
    yards: &std::collections::BTreeSet<SimId>,
    units: usize,
    repacks: bool,
) -> Option<Command> {
    selected
        .filter(|building| repacks && units == 0 && yards.contains(building))
        .map(undeploy_command)
}
/// The Construction Yard that [`repack_command`] would pack up in the match of `host`, if any.
fn repack_target(
    host: &SimHost,
    selection: &Selection,
    selected: &SelectedBuilding,
) -> Option<Command> {
    repack_command(
        selected.0,
        &crate::selection::own_yards(host),
        selection.0.len(),
        host.match_state()
            .is_some_and(|state| state.rules().mcv_repacks),
    )
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
    selected: Res<SelectedBuilding>,
    host: Res<SimHost>,
    mut sink: ResMut<CommandSink>,
    mut mode: ResMut<InterfaceMode>,
    buttons: Query<(&StrategicButton, &Interaction), Changed<Interaction>>,
    repack: Query<&Interaction, (With<RepackButton>, Changed<Interaction>)>,
) {
    if !host.running() || sink.paused() {
        return;
    }
    if repack.iter().any(|i| *i == Interaction::Pressed) {
        sink.issue(repack_target(&host, &selection, &selected));
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
        // With no unit selected, D packs up the selected Construction Yard (RA1, while the
        // match has "MCV repacks" on).
        sink.issue(repack_target(&host, &selection, &selected));
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
    selection: Res<Selection>,
    selected: Res<SelectedBuilding>,
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
        let wanted = if repack_target(&host, &selection, &selected).is_some() {
            REPACK_HINT
        } else {
            HINT
        };
        if t.0 != wanted {
            t.0 = wanted.into();
        }
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::skirmish::MenuSelection;

    /// The human player's base units and buildings in `host`, as `(units, buildings)`.
    fn base(host: &SimHost) -> (Vec<SimId>, Vec<SimId>) {
        let rules = host.definitions().and_then(sim::Definitions::rules);
        let world = host.world();
        let mut units = world
            .try_query::<(&SimId, &Owner, &sim::Unit)>()
            .expect("registered");
        let mut buildings = world
            .try_query::<(&SimId, &Owner, &Building)>()
            .expect("registered");
        (
            units
                .iter(world)
                .filter(|(_, owner, unit)| {
                    owner.0 == HUMAN && rules.is_some_and(|rules| rules.is_base_unit(unit.kind))
                })
                .map(|(id, ..)| *id)
                .collect(),
            buildings
                .iter(world)
                .filter(|(_, owner, _)| owner.0 == HUMAN)
                .map(|(id, ..)| *id)
                .collect(),
        )
    }

    #[test]
    fn a_selected_construction_yard_is_repacked_only_with_the_option_on() {
        // RA1 item 2: D or the Repack button pack up the selected Construction Yard while the
        // match has "MCV repacks" on; with it off there is no such control.
        let yard = SimId(7);
        let yards = BTreeSet::from([yard]);
        assert_eq!(
            repack_command(Some(yard), &yards, 0, true),
            Some(Command::Undeploy { building: yard })
        );
        assert_eq!(
            repack_command(Some(yard), &yards, 0, false),
            None,
            "option off"
        );
        assert_eq!(
            repack_command(Some(yard), &yards, 1, true),
            None,
            "D deploys the units"
        );
        assert_eq!(
            repack_command(Some(SimId(8)), &yards, 0, true),
            None,
            "a factory"
        );
        assert_eq!(
            repack_command(None, &yards, 0, true),
            None,
            "nothing selected"
        );
        assert_eq!(repack_command(Some(yard), &BTreeSet::new(), 0, true), None);

        // Through the simulation: the match as the menu starts it, with a fixed opponent.
        let catalogue = crate::skirmish::tests::production_catalogue();
        let selection = MenuSelection::versus(&catalogue);
        let (mut host, _) = crate::sim_host::tests::match_host(&catalogue, selection, 0);
        let none = Selection::default();
        let rejected = |events: &[sim::SimEvent]| {
            events
                .iter()
                .any(|event| matches!(event, sim::SimEvent::CommandRejected { .. }))
        };
        // The player starts with its MCV: nothing to repack, whatever is "selected".
        let (units, buildings) = base(&host);
        assert_eq!((units.len(), buildings.len()), (1, 0));
        assert!(crate::selection::own_yards(&host).is_empty());
        assert_eq!(
            repack_target(&host, &none, &SelectedBuilding(Some(units[0]))),
            None
        );
        // D deploys the selected MCV (the first tick gives the player its sight).
        crate::sim_host::tests::run(&mut host, Vec::new());
        let events = crate::sim_host::tests::run(
            &mut host,
            deploy_commands(&Selection(units.iter().copied().collect())),
        );
        assert!(!rejected(&events), "{events:?}");
        let (units, buildings) = base(&host);
        assert_eq!((units.len(), buildings.len()), (0, 1));
        let yard = buildings[0];
        assert_eq!(crate::selection::own_yards(&host), BTreeSet::from([yard]));
        // The yard selected: the control packs it up; not while units are selected, and not
        // for a building that is not selected.
        let selected = SelectedBuilding(Some(yard));
        assert_eq!(
            repack_target(&host, &Selection(BTreeSet::from([SimId(1)])), &selected),
            None
        );
        assert_eq!(repack_target(&host, &none, &SelectedBuilding(None)), None);
        let command = repack_target(&host, &none, &selected).expect("a repack");
        assert_eq!(command, undeploy_command(yard));
        let events = crate::sim_host::tests::run(&mut host, vec![command]);
        assert!(!rejected(&events), "{events:?}");
        assert!(
            events
                .iter()
                .any(|event| matches!(event, sim::SimEvent::BuildingUndeployed { .. })),
            "{events:?}"
        );
        let (units, buildings) = base(&host);
        assert_eq!((units.len(), buildings.len()), (1, 0), "the MCV is back");
        // And it deploys again.
        let events = crate::sim_host::tests::run(
            &mut host,
            deploy_commands(&Selection(units.iter().copied().collect())),
        );
        assert!(!rejected(&events), "{events:?}");
        assert_eq!(base(&host).1.len(), 1);

        // With "MCV repacks" off in the menu, the yard the player starts with (MCV start off)
        // offers no repack.
        let fixed = MenuSelection {
            mcv_repacks: false,
            mcv_start: false,
            ..selection
        };
        let (host, _) = crate::sim_host::tests::match_host(&catalogue, fixed, 0);
        let yards = crate::selection::own_yards(&host);
        assert_eq!(yards.len(), 1);
        let selected = SelectedBuilding(yards.first().copied());
        assert_eq!(repack_target(&host, &none, &selected), None);
        // The same match with the option on offers it.
        let (host, _) = crate::sim_host::tests::match_host(
            &catalogue,
            MenuSelection {
                mcv_repacks: true,
                ..fixed
            },
            0,
        );
        let selected = SelectedBuilding(crate::selection::own_yards(&host).first().copied());
        assert!(repack_target(&host, &none, &selected).is_some());
    }
}
