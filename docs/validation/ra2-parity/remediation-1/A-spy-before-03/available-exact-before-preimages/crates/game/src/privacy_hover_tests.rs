// Included in hover_info::tests so the controls call the actual private production handler.
use crate::privacy_native as privacy;
use bevy::ecs::system::RunSystemOnce;

fn privacy_label(app: &mut App, id: SimId) -> (String, Display, Color) {
    privacy::hover_unit(app, id);
    app.world_mut()
        .run_system_once(show_hover_label)
        .expect("actual hover handler");
    let world = app.world_mut();
    let mut label = world.query_filtered::<(&Text, &Node, &TextColor), With<HoverLabel>>();
    let (text, node, color) = label.single(world).expect("hover label");
    (text.0.clone(), node.display, color.0)
}

#[test]
fn ra9_privacy_spy_hover_uses_native_disguise_then_real_detection() {
    let mut ids = None;
    let plan = crate::sim_host::tests::privacy_plan(true);
    let host = crate::sim_host::tests::privacy_host(&plan, |world| {
        let spy = sim::spawn_unit(world, privacy::ENEMY, "spy", sim::CellPos::new(21, 20)).unwrap();
        let gi = sim::spawn_unit(world, HUMAN, "gi", sim::CellPos::new(22, 24)).unwrap();
        let gi_entity = world
            .query::<(Entity, &SimId)>()
            .iter(world)
            .find(|(_, id)| **id == gi)
            .unwrap()
            .0;
        world.entity_mut(gi_entity).remove::<sim::Weapon>();
        sim::spawn_unit(world, HUMAN, "engineer", sim::CellPos::new(22, 20)).unwrap();
        let dog = sim::spawn_unit(world, HUMAN, "attack_dog", sim::CellPos::new(10, 20)).unwrap();
        for id in [spy, gi] {
            let entity = world
                .query::<(Entity, &SimId)>()
                .iter(world)
                .find(|(_, other)| **other == id)
                .unwrap()
                .0;
            let kind = world.get::<sim::Unit>(entity).unwrap().kind;
            let max = world
                .resource::<sim::Definitions>()
                .unit_combat(kind)
                .unwrap()
                .hit_points;
            assert_eq!(max % 5, 0, "loaded stock HP permits exact80% setup");
            let hp = max * 4 / 5;
            assert!(hp > 0);
            world.get_mut::<sim::Health>(entity).unwrap().hit_points = hp;
        }
        ids = Some((spy, gi, dog));
    });
    let (spy, gi, dog) = ids.unwrap();
    let mut app = privacy::app(host);
    app.world_mut().run_system_once(spawn_hover_label).unwrap();
    privacy::step(&mut app, Vec::new());
    let host = app.world().resource::<SimHost>();
    let spy_entity = privacy::entity(host, spy);
    let spy_kind = host.world().get::<sim::Unit>(spy_entity).unwrap().kind;
    assert!(
        !host
            .definitions()
            .unwrap()
            .metadata(ProductionItem::Unit(spy_kind))
            .unwrap()
            .trainable,
        "stock Spy has no legitimate veteran/elite training premise"
    );
    assert!(
        host.world()
            .get::<sim::Experience>(spy_entity)
            .is_none_or(|xp| xp.rank == 0 && xp.value == 0),
        "stock Spy starts with no rank/XP"
    );
    assert!(
        privacy::hidden(&app, spy),
        "real reconnaissance must conceal the spy"
    );
    let kind = app
        .world()
        .resource::<SimHost>()
        .definitions()
        .unwrap()
        .unit_kind("gi")
        .unwrap();
    assert_eq!(
        app.world()
            .resource::<SimHost>()
            .world()
            .resource::<sim::SpecialState>()
            .disguise_for(HUMAN, spy),
        Some(kind)
    );
    let normal = privacy_label(&mut app, gi);
    let disguised = privacy_label(&mut app, spy);
    let view = app.world().resource::<crate::units::UnitViews>().0[&spy];
    assert_ne!(
        *app.world().get::<Visibility>(view).unwrap(),
        Visibility::Hidden,
        "the actual renderer deliberately shows apparent friendly infantry"
    );
    privacy::move_to(&mut app, HUMAN, dog, sim::CellPos::new(18, 20));
    assert!(
        !privacy::hidden(&app, spy),
        "actual dog movement/detection reveals true identity"
    );
    let detected = privacy_label(&mut app, spy);
    eprintln!(
        "native Spy hover: normal={normal:?}, disguised={disguised:?}, detected={detected:?}"
    );
    assert_eq!(normal.1, Display::Flex);
    assert!(normal.0.contains("80%"), "{normal:?}");
    assert_eq!(detected.1, Display::Flex);
    assert!(
        detected.0.contains("Spy")
            && detected.0.contains("Red Player")
            && detected.0.contains("Flak armour")
            && detected.0.contains("80%")
            && !detected.0.contains("Elite")
            && !detected.0.contains("Veteran"),
        "{detected:?}"
    );
    assert_ne!(
        detected.2, normal.2,
        "detected enemy keeps ordinary owner colour"
    );
    assert_eq!(
        disguised, normal,
        "undiscovered friendly GI appearance must not expose real Spy type/owner/armour/rank"
    );
}
