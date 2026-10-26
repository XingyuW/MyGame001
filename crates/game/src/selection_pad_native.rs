// Included inside selection::tests: real SimHost receipt and ordinary input handlers.
fn sp_unit(host: &SimHost, id: SimId) -> (Entity, sim::SubPos) {
    let mut query = host.world().try_query::<(Entity, &SimId, &sim::Position)>().unwrap();
    query.iter(host.world()).find(|(_, other, _)| **other == id)
        .map(|(entity, _, pos)| (entity, pos.0)).expect("native unit")
}
fn sp_mission_host() -> (SimHost, SimId, SimId) {
    let mut normal = None;
    let mut airport = None;
    let mut host = crate::sim_host::tests::fixture_host(|world| {
        let definitions = sim::load_ruleset(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data")).unwrap();
        let open = sim::Cell { passable: true, water: false, buildable: true, ore: 0 };
        world.insert_resource(sim::Grid::from_cells(48, 48, vec![open; 48 * 48]).unwrap());
        for (player, country) in [(HUMAN, "great_britain"), (sim::PlayerId(1), "russia")] {
            let country = definitions.rules().unwrap().country_id(country).unwrap();
            world.resource_mut::<sim::Players>().add(player, 10_000).unwrap();
            assert!(world.resource_mut::<sim::Players>().set_country(player, Some(country)));
        }
        world.insert_resource(definitions);
        airport = Some(sim::spawn_building(world, HUMAN, "american_airforce_command", CellPos::new(8, 8)).unwrap());
        sim::spawn_building(world, HUMAN, "nuclear_reactor", CellPos::new(2, 2)).unwrap();
        normal = Some(sim::spawn_unit(world, HUMAN, "engineer", CellPos::new(24, 24)).unwrap());
    });
    let airport = airport.unwrap();
    let definitions = host.world().resource::<sim::Definitions>();
    let kind = definitions.building_kind("american_airforce_command").unwrap();
    let cooldown = definitions.metadata(sim::ProductionItem::Building(kind)).unwrap().cooldown;
    assert!(cooldown > 0 && cooldown <= 10_000, "bounded source cooldown");
    // Actual native charging, including the strategic first-observation reset.
    for _ in 0..cooldown + 2 {
        if host.world().resource::<sim::SpecialState>().charge.get(&airport).copied().unwrap_or(0) >= cooldown { break; }
        crate::sim_host::tests::run(&mut host, vec![]);
    }
    assert_eq!(host.world().resource::<sim::SpecialState>().charge.get(&airport), Some(&cooldown));
    let events = crate::sim_host::tests::run(&mut host, vec![Command::Superweapon {
        building: airport, target: CellPos::new(24, 24), units: vec![],
    }]);
    assert!(!events.iter().any(|e| matches!(e, sim::SimEvent::CommandRejected { .. })), "{events:?}");
    let special = host.world().resource::<sim::SpecialState>();
    assert_eq!(special.strategic.flights.len(), 1, "real launch consumer");
    let plane = *special.strategic.flights.keys().next().unwrap();
    assert!(!special.spawned.contains_key(&plane), "exercises the distinct flight path");
    let (entity, _) = sp_unit(&host, plane);
    let kind = host.world().resource::<sim::Definitions>().unit_kind("paradrop_plane").unwrap();
    assert_eq!(host.world().get::<sim::Unit>(entity).unwrap().kind, kind);
    assert_eq!(host.world().get::<sim::Health>(entity).unwrap().hit_points, 400);
    (host, plane, normal.unwrap())
}
fn sp_mission_input() -> (App, Entity, SimId, SimId) {
    let (host, plane, normal) = sp_mission_host();
    let plane_data = sp_unit(&host, plane);
    let normal_data = sp_unit(&host, normal);
    let (mut app, window, _) = input_app_on(host);
    let old: Vec<_> = app.world_mut().query_filtered::<Entity, With<UnitView>>().iter(app.world()).collect();
    for entity in old { app.world_mut().despawn(entity); }
    for (id, (unit, pos), x) in [(plane, plane_data, 0.0), (normal, normal_data, 200.0)] {
        app.world_mut().spawn((UnitView {
            unit, id, owner: HUMAN, previous: pos, current: pos, altitude: 0.0, drawn: None, stride: 0.0, still: 0.0,
        }, Transform::from_xyz(x, 0.0, 10.0), Visibility::Visible));
    }
    (app, window, plane, normal)
}

#[test]
fn ra9_strategic_plane_is_excluded_by_real_click_shift_and_box() {
    let (mut app, window, plane, normal) = sp_mission_input();
    cursor(&mut app, window, Some(Vec2::new(640.0, 400.0)));
    click(&mut app, MouseButton::Left);
    assert!(app.world().resource::<Selection>().0.is_empty());
    click(&mut app, MouseButton::Left);
    assert!(app.world().resource::<Selection>().0.is_empty(), "second click cannot type-select it");
    cursor(&mut app, window, Some(Vec2::new(840.0, 400.0)));
    click(&mut app, MouseButton::Left);
    assert_eq!(app.world().resource::<Selection>().0, BTreeSet::from([normal]));
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ShiftLeft);
    cursor(&mut app, window, Some(Vec2::new(640.0, 400.0)));
    click(&mut app, MouseButton::Left);
    assert_eq!(app.world().resource::<Selection>().0, BTreeSet::from([normal]));
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().reset_all();
    cursor(&mut app, window, Some(Vec2::new(600.0, 350.0)));
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
    app.update();
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().clear();
    cursor(&mut app, window, Some(Vec2::new(880.0, 450.0)));
    app.update();
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Left);
    app.update();
    assert_eq!(app.world().resource::<Selection>().0, BTreeSet::from([normal]));
    assert!(!app.world().resource::<Selection>().0.contains(&plane));
}
#[test]
fn ra9_mission_selection_boundary_agrees_with_real_command_rejection() {
    let (mut host, plane, _) = sp_mission_host();
    let command = Command::Move { units: vec![plane], target: CellPos::new(20, 20) };
    let events = crate::sim_host::tests::run(&mut host, vec![command.clone()]);
    assert!(events.iter().any(|event| matches!(event, sim::SimEvent::CommandRejected {
        player, command: found, reason: sim::RejectReason::NotCommandable(id),
    } if *player == HUMAN && *id == plane && found == &command)), "{events:?}");
    assert!(host.world().resource::<sim::SpecialState>().strategic.flights.contains_key(&plane));
}
#[test]
fn ra9_own_bridge_hut_characterizes_narrower_selected_building_policy() {
    let mut hut = None;
    let host = crate::sim_host::tests::fixture_host(|world| {
        let definitions = sim::load_ruleset(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data")).unwrap();
        world.insert_resource(definitions);
        let open = sim::Cell { passable: true, water: false, buildable: true, ore: 0 };
        world.insert_resource(sim::Grid::from_cells(32, 32, vec![open; 1024]).unwrap());
        world.resource_mut::<sim::Players>().add(HUMAN, 10_000).unwrap();
        hut = Some(sim::spawn_building(world, HUMAN, "bridge_hut", CellPos::new(14, 14)).unwrap());
        sim::spawn_unit(world, HUMAN, "engineer", CellPos::new(20, 20)).unwrap();
    });
    let hut = hut.unwrap();
    assert_eq!(own_building(&host, Some(CellPos::new(14, 14))), Some(hut));
    assert!(!own_producers(&host).contains(&hut));
    assert!(!own_yards(&host).contains(&hut));
    assert!(!own_garrisons(&host).contains(&hut));
    let (mut app, window, _) = input_app_on(host);
    app.add_systems(Update, prune_selected_building.after(select_units));
    app.world_mut().resource_mut::<HoveredCell>().0 = Some(CellPos::new(14, 14));
    cursor(&mut app, window, Some(Vec2::new(300.0, 200.0)));
    click(&mut app, MouseButton::Left);
    assert!(app.world().resource::<Selection>().0.is_empty());
    assert_eq!(app.world().resource::<SelectedBuilding>().0, None);
    app.world_mut().resource_mut::<SelectedBuilding>().0 = Some(hut);
    app.update();
    assert_eq!(app.world().resource::<SelectedBuilding>().0, None, "real pruning uses the same contract");
}
