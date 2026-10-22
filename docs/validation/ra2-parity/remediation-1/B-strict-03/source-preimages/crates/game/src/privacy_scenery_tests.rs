// Included in battlefield::tests. Live public damage, actual sprite/map/hover observers.
fn privacy_scenery_picture(
    app: &mut App,
    cell: CellPos,
) -> (usize, u8, sim::Cell, String, Display) {
    app.world_mut()
        .run_system_once(raze_scenery)
        .expect("actual scenery reconciliation");
    let world = app.world_mut();
    let mut query = world.query::<&Ground>();
    let tops = query
        .iter(world)
        .filter(|g| g.cell == cell && g.scenery)
        .count();
    let map = world.resource::<MapView>();
    let look = world.resource::<crate::map::MapLooks>().0
        [(cell.y as u32 * map.width + cell.x as u32) as usize];
    let terrain = *world.resource::<MapTerrain>().0.cell(cell).unwrap();
    let (text, shown) = crate::hover_info::privacy_scenery_label(app, cell);
    (tops, look, terrain, text, shown)
}

#[test]
fn ra9_privacy_fogged_scenery_keeps_observed_hp_and_destruction_until_reobserved() {
    use crate::privacy_native as privacy;
    let cell = CellPos::new(28, 20);
    let mut plan = crate::sim_host::tests::privacy_plan(true);
    plan.looks[(cell.y * 48 + cell.x) as usize] = looks::look(Soil::Grass, Feature::House);
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
        let scout = sim::spawn_unit(world, HUMAN, "engineer", CellPos::new(26, 20)).unwrap();
        let tank = sim::spawn_unit(world, privacy::ENEMY, "grizzly", CellPos::new(34, 24)).unwrap();
        ids = Some((
            scout,
            tank,
            world.resource::<sim::Props>().id_at(cell).unwrap(),
        ));
    });
    let (scout, tank, prop) = ids.unwrap();
    let mut app = privacy::app(host);
    privacy_materials(&mut app);
    app.insert_resource(crate::map::MapLooks(plan.looks.clone()));
    app.world_mut().run_system_once(terrain).unwrap();
    privacy::step(&mut app, Vec::new());
    let original = privacy_scenery_picture(&mut app, cell);
    assert_eq!(original.0, 1);
    assert_eq!(original.4, Display::Flex);
    assert!(
        original.3.contains("100%"),
        "actually observed original health"
    );
    privacy::move_to(&mut app, HUMAN, scout, CellPos::new(12, 20));
    let host = app.world().resource::<SimHost>();
    let vision = host.players().unwrap().get(HUMAN).unwrap().vision();
    assert!(
        vision.is_explored(cell) && !vision.is_visible(cell),
        "genuine optional fog"
    );
    let events = privacy::step(
        &mut app,
        vec![privacy::issue(
            privacy::ENEMY,
            Command::Attack {
                units: vec![tank],
                target: prop,
            },
        )],
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
    let mut damaged = false;
    for _ in 0..1200 {
        privacy::step(&mut app, Vec::new());
        let host = app.world().resource::<SimHost>();
        if host
            .props()
            .unwrap()
            .get(prop)
            .is_some_and(|(_, p)| p.hit_points < p.kind.hit_points())
        {
            damaged = true;
            break;
        }
    }
    assert!(
        damaged,
        "public enemy attack must damage the actual Prop first"
    );
    let unseen_damage = privacy_scenery_picture(&mut app, cell);
    let mut destroyed = false;
    for _ in 0..2400 {
        let events = privacy::step(&mut app, Vec::new());
        destroyed |= events.iter().any(|e| {
            matches!(e,
            SimEvent::PropDestroyed { cell: destroyed_cell, .. } if *destroyed_cell == cell)
        });
        if destroyed {
            break;
        }
    }
    assert!(
        destroyed,
        "public attack must actually destroy the Prop through Cleanup"
    );
    let unseen_destruction = privacy_scenery_picture(&mut app, cell);
    // Several actual ticks before one picture read exercise the retained frame-event batch.
    privacy::step(
        &mut app,
        vec![privacy::issue(
            privacy::ENEMY,
            Command::Move {
                units: vec![tank],
                target: CellPos::new(40, 20),
            },
        )],
    );
    for _ in 0..100 {
        privacy::step(&mut app, Vec::new());
    }
    privacy::move_to(&mut app, HUMAN, scout, CellPos::new(26, 20));
    let observed_destruction = privacy_scenery_picture(&mut app, cell);
    eprintln!(
        "native scenery observations: original={original:?}, unseen_damage={unseen_damage:?}, unseen_destruction={unseen_destruction:?}, reobserved={observed_destruction:?}"
    );
    assert_eq!(
        observed_destruction.0, 0,
        "actual top disappears after re-observation"
    );
    assert_eq!(
        observed_destruction.1 & 0xf0,
        0,
        "actual minimap look reconciles"
    );
    assert!(
        observed_destruction.2.passable,
        "actual interface terrain reconciles"
    );
    assert_eq!(
        observed_destruction.4,
        Display::None,
        "hover knows destruction after re-observation"
    );
    assert_eq!(
        unseen_damage, original,
        "fogged hover must retain last observed HP"
    );
    assert_eq!(
        unseen_destruction, original,
        "unseen destruction must not change top/map/hover"
    );
}

#[test]
fn ra9_privacy_scenery_observation_resets_on_uninstall_and_match_replacement() {
    use crate::privacy_native as privacy;
    let cell = CellPos::new(28, 20);
    let mut plan = crate::sim_host::tests::privacy_plan(true);
    plan.looks[(cell.y * 48 + cell.x) as usize] = looks::look(Soil::Grass, Feature::House);
    plan.grid.set_terrain(
        cell,
        sim::Cell {
            passable: false,
            water: false,
            buildable: false,
            ore: 0,
        },
    );
    let host = crate::sim_host::tests::privacy_host(&plan, |world| {
        sim::spawn_unit(world, HUMAN, "engineer", CellPos::new(26, 20)).unwrap();
    });
    let mut app = privacy::app(host);
    privacy::step(&mut app, Vec::new());
    assert_eq!(
        crate::hover_info::privacy_scenery_label(&mut app, cell).1,
        Display::Flex
    );
    let mut invalid = plan.clone();
    invalid.players[0].start = usize::MAX;
    assert!(crate::sim_host::install_match(app.world_mut(), &invalid).is_err());
    assert_eq!(
        crate::hover_info::privacy_scenery_label(&mut app, cell).1,
        Display::Flex,
        "failed install preserves the existing match and observed information"
    );
    crate::sim_host::uninstall_match(app.world_mut());
    assert_eq!(
        crate::hover_info::privacy_scenery_label(&mut app, cell).1,
        Display::None
    );
    crate::sim_host::install_match(app.world_mut(), &plan).unwrap();
    privacy::step(&mut app, Vec::new());
    assert_eq!(
        crate::hover_info::privacy_scenery_label(&mut app, cell).1,
        Display::None,
        "new match starts with genuinely unseen scenery, never an old observation"
    );
}

#[test]
fn ra9_privacy_fogged_hover_does_not_consult_unseen_garrison_presence() {
    use crate::privacy_native as privacy;
    let cell = CellPos::new(28, 20);
    let mut plan = crate::sim_host::tests::privacy_plan(true);
    plan.looks[(cell.y * 48 + cell.x) as usize] = looks::look(Soil::Grass, Feature::House);
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
        let scout = sim::spawn_unit(world, HUMAN, "grizzly", CellPos::new(26, 20)).unwrap();
        let soldier = sim::spawn_unit(world, privacy::ENEMY, "gi", CellPos::new(34, 20)).unwrap();
        let entity = world
            .query::<(Entity, &SimId)>()
            .iter(world)
            .find(|(_, id)| **id == scout)
            .unwrap()
            .0;
        // Initial observer is harmless; real garrison entry and fog are left intact.
        world.entity_mut(entity).remove::<sim::Weapon>();
        ids = Some((
            scout,
            soldier,
            world.resource::<sim::Props>().id_at(cell).unwrap(),
        ));
    });
    let (scout, soldier, prop) = ids.unwrap();
    let mut app = privacy::app(host);
    privacy::step(&mut app, Vec::new());
    let initial = crate::hover_info::privacy_scenery_label(&mut app, cell);
    assert_eq!(initial.1, Display::Flex);
    assert!(!initial.0.contains("occupants"));
    privacy::move_to(&mut app, HUMAN, scout, CellPos::new(12, 20));
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
            .any(|e| matches!(e, SimEvent::CommandRejected { .. })),
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
        privacy::step(&mut app, Vec::new());
    }
    assert!(
        app.world()
            .resource::<SimHost>()
            .world()
            .resource::<sim::SpecialState>()
            .garrisons
            .contains_key(&prop),
        "entry must be a real native garrison"
    );
    let host = app.world().resource::<SimHost>();
    assert!(
        !host
            .players()
            .unwrap()
            .get(HUMAN)
            .unwrap()
            .vision()
            .is_visible(cell)
    );
    let fogged = crate::hover_info::privacy_scenery_label(&mut app, cell);
    privacy::move_to(&mut app, HUMAN, scout, CellPos::new(24, 20));
    let observed = crate::hover_info::privacy_scenery_label(&mut app, cell);
    assert_eq!(observed.1, Display::Flex);
    assert!(
        observed.0.contains("1 occupants") && observed.0.contains("Enemy"),
        "{observed:?}"
    );
    assert_eq!(
        fogged, initial,
        "unseen garrison presence must not change whether the fogged scenery label exists"
    );
}
