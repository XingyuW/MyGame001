// Included in selection::tests. All hidden changes use public commands and the native schedule.
use bevy::ecs::system::RunSystemOnce;
#[derive(Debug, PartialEq)]
struct PrivacyOrderObservation {
    preview: OrderPreview,
    cursor: String,
    label: String,
    display: Display,
    markers: Vec<Vec3>,
    commands: Vec<Command>,
    accepted: u64,
    mode: InterfaceMode,
    acknowledgements: u64,
    feedback: Option<CellPos>,
}

fn privacy_order_app(host: SimHost, looks: Vec<u8>) -> App {
    let mut app = crate::privacy_native::app(host);
    app.insert_resource(crate::map::MapLooks(looks))
        .init_resource::<ControlGroups>()
        .init_resource::<crate::audio::SfxState>()
        .init_resource::<crate::audio::VolumeSettings>()
        .init_resource::<crate::weapon_fx::ImpactSounds>()
        .init_resource::<crate::buildings::BuildingViews>()
        .insert_resource(crate::audio::SoundBank {
            effects: [(
                crate::audio::Sfx::Acknowledge,
                (Handle::default(), std::time::Duration::from_millis(50)),
            )]
            .into_iter()
            .collect(),
        });
    crate::privacy_native::gizmos(&mut app);
    crate::battlefield::privacy_prepare_order_map(&mut app);
    app.world_mut().run_system_once(spawn_intent_label).unwrap();
    app
}

fn privacy_order_observation(
    app: &mut App,
    actor: SimId,
    cell: CellPos,
    key: Option<KeyCode>,
) -> PrivacyOrderObservation {
    use bevy::window::CursorIcon;
    // Reset only client input/output state, never the simulation, visibility or observed map.
    let world = app.world_mut();
    let effects: Vec<_> = world
        .query_filtered::<Entity, With<crate::audio::SoundEffect>>()
        .iter(world)
        .collect();
    for entity in effects {
        world.despawn(entity);
    }
    app.insert_resource(CommandSink::default())
        .insert_resource(OrderFeedback::default())
        .insert_resource(crate::audio::SfxState::default());
    app.world_mut()
        .resource_mut::<bevy::ecs::message::Messages<sim::SimEvent>>()
        .clear();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .reset_all();
    app.world_mut().resource_mut::<Selection>().0 = [actor].into_iter().collect();
    crate::units::privacy_place_views(app);
    crate::privacy_native::hover_cell(app, cell);
    if let Some(key) = key {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
    }
    crate::battlefield::privacy_reconcile_order_map(app);
    app.world_mut().run_system_once(preview_order).unwrap();
    app.world_mut().run_system_once(show_intent_cursor).unwrap();
    let preview = *app.world().resource::<OrderPreview>();
    let markers = privacy_marker_points(app);
    let world = app.world_mut();
    let cursor = format!(
        "{:?}",
        world
            .query_filtered::<&CursorIcon, With<PrimaryWindow>>()
            .single(world)
            .unwrap()
    );
    let (label, display) = world
        .query_filtered::<(&Text, &Node), With<IntentLabel>>()
        .single(world)
        .map(|(text, node)| (text.0.clone(), node.display))
        .unwrap();
    if key == Some(KeyCode::KeyF) {
        crate::ra7_controls::run_extended_keys_for_test(app.world_mut());
    } else {
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.world_mut().run_system_once(issue_orders).unwrap();
    }
    app.world_mut()
        .run_system_once(crate::audio::play_effects)
        .unwrap();
    let result = PrivacyOrderObservation {
        preview,
        cursor,
        label,
        display,
        markers,
        commands: crate::sim_host::tests::take_commands(app),
        accepted: app.world().resource::<CommandSink>().accepted(),
        mode: *app.world().resource::<InterfaceMode>(),
        acknowledgements: app.world().resource::<crate::audio::SfxState>().started,
        feedback: app.world().resource::<OrderFeedback>().cell,
    };
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .reset_all();
    result
}

fn privacy_order_suite(app: &mut App, actor: SimId, cell: CellPos) -> Vec<PrivacyOrderObservation> {
    [None, Some(KeyCode::ControlLeft), Some(KeyCode::KeyF)]
        .into_iter()
        .map(|key| privacy_order_observation(app, actor, cell, key))
        .collect()
}

fn privacy_order_visible(app: &App, cell: CellPos) -> bool {
    app.world()
        .resource::<SimHost>()
        .players()
        .unwrap()
        .get(HUMAN)
        .unwrap()
        .vision()
        .is_visible(cell)
}

fn privacy_order_scenery_scene() -> (App, SimId, SimId, SimId, SimId, SimId, CellPos) {
    use crate::privacy_native as privacy;
    let cell = CellPos::new(28, 20);
    let mut plan = crate::sim_host::tests::privacy_plan(true);
    plan.looks[(cell.y * 48 + cell.x) as usize] = crate::battlefield::looks::look(
        crate::battlefield::looks::Ground::Grass,
        crate::battlefield::looks::Feature::House,
    );
    plan.grid.set_terrain(
        cell,
        sim::Cell {
            passable: false,
            water: false,
            buildable: false,
            ore: 0,
        },
    );
    let mut ids = None;
    let host = crate::sim_host::tests::privacy_host(&plan, |world| {
        let actor = sim::spawn_unit(world, HUMAN, "grizzly", CellPos::new(10, 20)).unwrap();
        let scout = sim::spawn_unit(world, HUMAN, "grizzly", CellPos::new(26, 20)).unwrap();
        let scout_entity = world
            .query::<(Entity, &SimId)>()
            .iter(world)
            .find(|(_, id)| **id == scout)
            .unwrap()
            .0;
        // A harmless durable observer survives the visible garrison-positive approach.
        world.entity_mut(scout_entity).remove::<Weapon>();
        let tank = sim::spawn_unit(world, privacy::ENEMY, "grizzly", CellPos::new(34, 24)).unwrap();
        let soldier = sim::spawn_unit(world, privacy::ENEMY, "gi", CellPos::new(34, 20)).unwrap();
        ids = Some((
            actor,
            scout,
            tank,
            soldier,
            world.resource::<sim::Props>().id_at(cell).unwrap(),
        ));
    });
    let (actor, scout, tank, soldier, prop) = ids.unwrap();
    let mut app = privacy_order_app(host, plan.looks);
    privacy::step(&mut app, vec![]);
    (app, actor, scout, tank, soldier, prop, cell)
}

#[test]
fn ra9_privacy_native_scenery_orders_ignore_unseen_damage_and_missing_prop() {
    use crate::privacy_native as privacy;
    let (mut app, actor, scout, tank, _, prop, cell) = privacy_order_scenery_scene();
    let visible = privacy_order_suite(&mut app, actor, cell);
    assert_eq!(visible[0].preview.intent, OrderIntent::Attack);
    assert_eq!(visible[0].preview.target, Some(prop));
    assert!(
        matches!(visible[0].commands.as_slice(), [Command::Attack { target, .. }] if *target == prop)
    );
    assert!(
        matches!(visible[1].commands.as_slice(), [Command::ForceFire { target: sim::ForceTarget::Entity(id), .. }] if *id == prop)
    );
    assert!(
        matches!(visible[2].commands.as_slice(), [Command::Follow { target, .. }] if *target == prop)
    );
    privacy::move_to(&mut app, HUMAN, scout, CellPos::new(12, 20));
    assert!(!privacy_order_visible(&app, cell));
    assert!(
        app.world()
            .resource::<SimHost>()
            .players()
            .unwrap()
            .get(HUMAN)
            .unwrap()
            .vision()
            .is_explored(cell)
    );
    let intact = privacy_order_suite(&mut app, actor, cell);
    privacy::step(
        &mut app,
        vec![privacy::issue(
            privacy::ENEMY,
            Command::Attack {
                units: vec![tank],
                target: prop,
            },
        )],
    );
    for _ in 0..1200 {
        if app
            .world()
            .resource::<SimHost>()
            .props()
            .unwrap()
            .get(prop)
            .is_some_and(|(_, p)| p.hit_points < p.kind.hit_points())
        {
            break;
        }
        privacy::step(&mut app, vec![]);
    }
    assert!(
        app.world()
            .resource::<SimHost>()
            .props()
            .unwrap()
            .get(prop)
            .is_some_and(|(_, p)| p.hit_points < p.kind.hit_points())
    );
    let damaged = privacy_order_suite(&mut app, actor, cell);
    let mut destroyed = false;
    for _ in 0..2400 {
        let events = privacy::step(&mut app, vec![]);
        destroyed |= events
            .iter()
            .any(|e| matches!(e, sim::SimEvent::PropDestroyed { cell: at, .. } if *at == cell));
        if destroyed {
            break;
        }
    }
    assert!(
        destroyed,
        "native public attack must really destroy the observed House"
    );
    assert!(
        app.world()
            .resource::<SimHost>()
            .props()
            .unwrap()
            .id_at(cell)
            .is_none()
    );
    assert!(!privacy_order_visible(&app, cell));
    let missing = privacy_order_suite(&mut app, actor, cell);
    privacy::move_to(&mut app, privacy::ENEMY, tank, CellPos::new(40, 24));
    privacy::move_to(&mut app, HUMAN, scout, CellPos::new(26, 20));
    assert!(privacy_order_visible(&app, cell));
    let observed = privacy_order_observation(&mut app, actor, cell, None);
    assert_eq!(observed.preview.intent, OrderIntent::Move);
    assert!(
        matches!(observed.commands.as_slice(), [Command::Move { target, .. }] if *target == cell)
    );
    assert_eq!((observed.accepted, observed.acknowledgements), (1, 1));
    assert_eq!(
        intact, damaged,
        "unseen damage must not alter any public order observable"
    );
    assert_eq!(
        intact, missing,
        "present and genuinely destroyed/missing live Props have equal observer knowledge"
    );
    assert_eq!(missing[0].preview.intent, OrderIntent::Invalid);
    assert!(missing[0].commands.is_empty());
    assert!(
        matches!(missing[1].commands.as_slice(), [Command::ForceFire { target: sim::ForceTarget::Cell(at), .. }] if *at == cell)
    );
    assert!(missing[2].commands.is_empty());
}

#[test]
fn ra9_privacy_native_scenery_orders_ignore_unseen_occupation_until_reobserved() {
    use crate::privacy_native as privacy;
    let (mut app, actor, scout, _, soldier, prop, cell) = privacy_order_scenery_scene();
    privacy::move_to(&mut app, HUMAN, scout, CellPos::new(12, 20));
    let empty = privacy_order_suite(&mut app, actor, cell);
    privacy::move_to(&mut app, privacy::ENEMY, soldier, CellPos::new(30, 20));
    let events = privacy::step(
        &mut app,
        vec![privacy::issue(
            privacy::ENEMY,
            Command::Interact {
                unit: soldier,
                target: prop,
            },
        )],
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, sim::SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
    for _ in 0..400 {
        if app
            .world()
            .resource::<SimHost>()
            .world()
            .resource::<sim::SpecialState>()
            .garrisons
            .contains_key(&prop)
        {
            break;
        }
        privacy::step(&mut app, vec![]);
    }
    assert!(
        app.world()
            .resource::<SimHost>()
            .world()
            .resource::<sim::SpecialState>()
            .garrisons
            .contains_key(&prop)
    );
    assert!(!privacy_order_visible(&app, cell));
    let occupied = privacy_order_suite(&mut app, actor, cell);
    privacy::move_to(&mut app, HUMAN, scout, CellPos::new(24, 20));
    assert!(privacy_order_visible(&app, cell));
    let seen = privacy_order_observation(&mut app, actor, cell, None);
    assert_eq!(seen.preview.intent, OrderIntent::Attack);
    assert_eq!(seen.preview.target, Some(prop));
    assert!(
        matches!(seen.commands.as_slice(), [Command::Attack { target, .. }] if *target == prop)
    );
    assert_eq!(
        empty, occupied,
        "actual unseen occupation must not alter preview, marker, modifier, Follow or Sink/audio"
    );
}

#[test]
fn ra9_privacy_native_bridge_orders_retain_seen_terrain_through_hidden_collapse_and_repair() {
    use crate::privacy_native as privacy;
    let cell = CellPos::new(28, 20);
    let mut plan = crate::sim_host::tests::privacy_plan(true);
    plan.looks[(cell.y * 48 + cell.x) as usize] = crate::battlefield::looks::look(
        crate::battlefield::looks::Ground::Water,
        crate::battlefield::looks::Feature::Bridge,
    );
    plan.grid.set_terrain(
        cell,
        sim::Cell {
            passable: true,
            water: false,
            buildable: false,
            ore: 0,
        },
    );
    // Actual Naval actor, weapon removed only at initial fixture setup to isolate terrain.
    // Stock amphibious transports are Amphibious and cannot establish the inverse contrast.
    for y in 29..=31 {
        for x in 9..=11 {
            plan.grid.set_terrain(
                CellPos::new(x, y),
                sim::Cell {
                    passable: false,
                    water: true,
                    buildable: false,
                    ore: 0,
                },
            );
        }
    }
    let mut ids = None;
    let host = crate::sim_host::tests::privacy_host(&plan, |world| {
        let land = sim::spawn_unit(world, HUMAN, "mcv", CellPos::new(10, 20)).unwrap();
        let ship = sim::spawn_unit(world, HUMAN, "aegis_cruiser", CellPos::new(10, 30)).unwrap();
        let ship_entity = world
            .query::<(Entity, &SimId)>()
            .iter(world)
            .find(|(_, id)| **id == ship)
            .unwrap()
            .0;
        world.entity_mut(ship_entity).remove::<Weapon>();
        let scout = sim::spawn_unit(world, HUMAN, "engineer", CellPos::new(26, 20)).unwrap();
        let gun = sim::spawn_unit(world, privacy::ENEMY, "grizzly", CellPos::new(34, 24)).unwrap();
        let engineer =
            sim::spawn_unit(world, privacy::ENEMY, "engineer", CellPos::new(34, 18)).unwrap();
        let bridge = world.resource::<sim::Props>().id_at(cell).unwrap();
        let hut = *world.resource::<sim::Bridges>().bridges[&bridge]
            .huts
            .iter()
            .next()
            .unwrap();
        ids = Some((land, ship, scout, gun, engineer, bridge, hut));
    });
    let (land, ship, scout, gun, engineer, bridge, hut) = ids.unwrap();
    assert_eq!(host.grid().unwrap().domain_of(ship), sim::Domain::Naval);
    assert!(!own_units_with::<Weapon>(&host).contains(&land));
    assert!(!own_units_with::<Weapon>(&host).contains(&ship));
    let mut app = privacy_order_app(host, plan.looks);
    privacy::step(&mut app, vec![]);
    assert!(privacy_order_visible(&app, cell));
    let original_land = privacy_order_observation(&mut app, land, cell, None);
    let original_ship = privacy_order_observation(&mut app, ship, cell, None);
    assert_eq!(original_land.preview.intent, OrderIntent::Move);
    assert_eq!(original_ship.preview.intent, OrderIntent::Invalid);
    privacy::move_to(&mut app, HUMAN, scout, CellPos::new(12, 20));
    let hidden_land = privacy_order_observation(&mut app, land, cell, None);
    let hidden_ship = privacy_order_observation(&mut app, ship, cell, None);
    privacy::step(
        &mut app,
        vec![privacy::issue(
            privacy::ENEMY,
            Command::Attack {
                units: vec![gun],
                target: bridge,
            },
        )],
    );
    for _ in 0..4000 {
        if app
            .world()
            .resource::<SimHost>()
            .world()
            .resource::<sim::Bridges>()
            .bridges[&bridge]
            .collapsed
        {
            break;
        }
        privacy::step(&mut app, vec![]);
    }
    assert!(
        app.world()
            .resource::<SimHost>()
            .world()
            .resource::<sim::Bridges>()
            .bridges[&bridge]
            .collapsed
    );
    assert!(!privacy_order_visible(&app, cell));
    assert!(
        !app.world()
            .resource::<SimHost>()
            .grid()
            .unwrap()
            .cell(cell)
            .unwrap()
            .passable
    );
    let collapsed_land = privacy_order_observation(&mut app, land, cell, None);
    let collapsed_ship = privacy_order_observation(&mut app, ship, cell, None);
    privacy::move_to(&mut app, privacy::ENEMY, gun, CellPos::new(40, 24));
    privacy::move_to(&mut app, HUMAN, scout, CellPos::new(26, 20));
    assert!(privacy_order_visible(&app, cell));
    let seen_land = privacy_order_observation(&mut app, land, cell, None);
    let seen_ship = privacy_order_observation(&mut app, ship, cell, None);
    assert_eq!(seen_land.preview.intent, OrderIntent::Invalid);
    assert!(seen_land.commands.is_empty());
    assert_eq!(seen_ship.preview.intent, OrderIntent::Move);
    assert!(
        matches!(seen_ship.commands.as_slice(), [Command::Move { target, .. }] if *target == cell)
    );
    assert_eq!((seen_ship.accepted, seen_ship.acknowledgements), (1, 1));
    privacy::move_to(&mut app, HUMAN, scout, CellPos::new(12, 20));
    let before_repair_land = privacy_order_observation(&mut app, land, cell, None);
    let before_repair_ship = privacy_order_observation(&mut app, ship, cell, None);
    let hut_cell = {
        let host = app.world().resource::<SimHost>();
        host.world()
            .get::<Building>(privacy::entity(host, hut))
            .unwrap()
            .footprint
            .origin
    };
    privacy::move_to(
        &mut app,
        privacy::ENEMY,
        engineer,
        CellPos::new(hut_cell.x, hut_cell.y + 2),
    );
    let events = privacy::step(
        &mut app,
        vec![privacy::issue(
            privacy::ENEMY,
            Command::Interact {
                unit: engineer,
                target: hut,
            },
        )],
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, sim::SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
    for _ in 0..2400 {
        if !app
            .world()
            .resource::<SimHost>()
            .world()
            .resource::<sim::Bridges>()
            .bridges[&bridge]
            .collapsed
        {
            break;
        }
        privacy::step(&mut app, vec![]);
    }
    assert!(
        !app.world()
            .resource::<SimHost>()
            .world()
            .resource::<sim::Bridges>()
            .bridges[&bridge]
            .collapsed,
        "public enemy Engineer must really repair"
    );
    assert!(!privacy_order_visible(&app, cell));
    let repaired_land = privacy_order_observation(&mut app, land, cell, None);
    let repaired_ship = privacy_order_observation(&mut app, ship, cell, None);
    privacy::move_to(&mut app, HUMAN, scout, CellPos::new(26, 20));
    assert!(privacy_order_visible(&app, cell));
    assert_eq!(
        original_land,
        privacy_order_observation(&mut app, land, cell, None),
        "reobserved repair restores real land Move admission"
    );
    assert_eq!(
        original_ship,
        privacy_order_observation(&mut app, ship, cell, None),
        "reobserved repair restores Naval rejection"
    );
    assert_eq!(
        hidden_land, collapsed_land,
        "unseen native collapse must not change actual land cursor/marker/Move Sink"
    );
    assert_eq!(
        hidden_ship, collapsed_ship,
        "unseen native collapse must not change actual Naval cursor/marker/Move Sink"
    );
    assert_eq!(
        before_repair_land, repaired_land,
        "unseen native repair retains observed collapse for land"
    );
    assert_eq!(
        before_repair_ship, repaired_ship,
        "unseen native repair retains observed collapse for Naval"
    );
}
