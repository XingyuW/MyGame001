// Included in selection::tests; inspect real GizmoAsset output from draw_selection.
fn privacy_marker_points(app: &mut App) -> Vec<Vec3> {
    use bevy::ecs::system::RunSystemOnce;
    use bevy::gizmos::{
        AppGizmoBuilder, GizmoAsset, GizmoHandles, config::DefaultGizmoConfigGroup,
    };
    if !app.world().contains_resource::<Assets<GizmoAsset>>() {
        app.init_resource::<Assets<GizmoAsset>>()
            .init_gizmo_group::<DefaultGizmoConfigGroup>();
    }
    app.world_mut()
        .run_system_once(draw_selection)
        .expect("actual marker drawer");
    app.world_mut().run_schedule(Last);
    let handles = app.world().resource::<GizmoHandles>();
    let handle = handles
        .handles()
        .get(&std::any::TypeId::of::<DefaultGizmoConfigGroup>())
        .and_then(Option::as_ref);
    handle
        .and_then(|handle| app.world().resource::<Assets<GizmoAsset>>().get(handle))
        .map_or_else(Vec::new, |asset| {
            asset
                .buffer()
                .strip_positions
                .iter()
                .chain(&asset.buffer().list_positions)
                .copied()
                .filter(|p| p.is_finite())
                .collect()
        })
}

#[test]
fn ra9_privacy_native_mind_control_prunes_groups_and_independently_guards_markers() {
    use crate::privacy_native as privacy;
    use bevy::ecs::system::RunSystemOnce;
    let plan = crate::sim_host::tests::privacy_plan(true);
    let mut ids = None;
    let host = crate::sim_host::tests::privacy_host(&plan, |world| {
        let victim = sim::spawn_unit(world, HUMAN, "engineer", CellPos::new(20, 20)).unwrap();
        let yuri = sim::spawn_unit(world, privacy::ENEMY, "yuri", CellPos::new(24, 20)).unwrap();
        let sniper = sim::spawn_unit(world, HUMAN, "sniper", CellPos::new(5, 20)).unwrap();
        ids = Some((victim, yuri, sniper));
    });
    let (victim, yuri, sniper) = ids.unwrap();
    let mut app = privacy::app(host);
    app.init_resource::<ControlGroups>();
    crate::units::privacy_place_views(&mut app);
    app.world_mut().resource_mut::<Selection>().0.insert(victim);
    for digit in [KeyCode::Digit1, KeyCode::Digit9] {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::ControlLeft);
        keys.press(digit);
        app.world_mut().run_system_once(control_groups).unwrap();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
    }
    assert_eq!(
        privacy_marker_points(&mut app).len(),
        5,
        "real own selection diamond"
    );
    for _ in 0..20 {
        privacy::step(&mut app, Vec::new());
        let host = app.world().resource::<SimHost>();
        if host
            .world()
            .get::<Owner>(privacy::entity(host, victim))
            .unwrap()
            .0
            == privacy::ENEMY
        {
            break;
        }
    }
    let host = app.world().resource::<SimHost>();
    assert_eq!(
        host.world()
            .get::<Owner>(privacy::entity(host, victim))
            .unwrap()
            .0,
        privacy::ENEMY,
        "real Yuri must take ownership before checking the client"
    );
    assert_eq!(
        host.world().resource::<sim::SpecialState>().controlled[&victim].0,
        yuri
    );
    crate::units::privacy_place_views(&mut app);
    app.world_mut().run_system_once(prune_selection).unwrap();
    let pruned_selection = app.world().resource::<Selection>().0.clone();
    let pruned_groups = app.world().resource::<ControlGroups>().clone();
    // A stale caller must not defeat the independent render guard even while the enemy is seen.
    app.world_mut().resource_mut::<Selection>().0.insert(victim);
    let visible_enemy_markers = privacy_marker_points(&mut app);
    privacy::move_to(&mut app, privacy::ENEMY, victim, CellPos::new(35, 20));
    crate::units::privacy_place_views(&mut app);
    let view = app.world().resource::<UnitViews>().0[&victim];
    assert_eq!(
        *app.world().get::<Visibility>(view).unwrap(),
        Visibility::Hidden
    );
    let hidden_enemy_markers = privacy_marker_points(&mut app);
    app.world_mut().resource_mut::<Selection>().0.clear();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Digit1);
    app.world_mut().run_system_once(control_groups).unwrap();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    let recalled = app.world().resource::<Selection>().0.clone();
    // Controller death is native combat, rather than a manually restored Owner component.
    privacy::move_to(&mut app, HUMAN, sniper, CellPos::new(16, 20));
    for _ in 0..400 {
        privacy::step(&mut app, Vec::new());
        let host = app.world().resource::<SimHost>();
        if !host
            .world()
            .resource::<sim::SpecialState>()
            .controlled
            .contains_key(&victim)
        {
            break;
        }
    }
    let host = app.world().resource::<SimHost>();
    assert_eq!(
        host.world()
            .get::<Owner>(privacy::entity(host, victim))
            .unwrap()
            .0,
        HUMAN,
        "real controller death must restore the victim"
    );
    app.world_mut().resource_mut::<Selection>().0.clear();
    privacy::hover_unit(&mut app, victim);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.world_mut().run_system_once(select_units).unwrap();
    {
        let mut buttons = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        buttons.clear();
        buttons.release(MouseButton::Left);
    }
    app.world_mut().run_system_once(select_units).unwrap();
    assert!(
        app.world().resource::<Selection>().0.contains(&victim),
        "restored unit is freshly selectable"
    );
    eprintln!("native capture observations: selection={pruned_selection:?}, groups={pruned_groups:?}, recalled={recalled:?}, visible_enemy_markers={visible_enemy_markers:?}, hidden_enemy_markers={hidden_enemy_markers:?}");
    assert!(
        pruned_selection.is_empty(),
        "ownership loss must prune current selection"
    );
    assert!(
        pruned_groups.0.iter().all(|g| !g.contains(&victim)),
        "ownership loss must prune every stored group"
    );
    assert!(
        recalled.is_empty(),
        "old group recall cannot restore tracking"
    );
    assert!(
        visible_enemy_markers.is_empty(),
        "current enemy ownership independently prohibits markers"
    );
    assert!(
        hidden_enemy_markers.is_empty(),
        "a stale selected hidden view must not draw live-position markers"
    );
}
