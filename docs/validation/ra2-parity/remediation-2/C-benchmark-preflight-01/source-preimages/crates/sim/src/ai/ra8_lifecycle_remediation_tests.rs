//! Public recursive boarding/capture/loss through the actual AI memory and attempt consumers.
use super::ra8_attrition_tests::{app, building, issue, step, unit};
use super::*;
use bevy_app::App;
use bevy_ecs::entity::Entity;

fn entity(app: &mut App, id: SimId) -> Option<Entity> {
    let world = app.world_mut();
    world
        .query::<(Entity, &SimId)>()
        .iter(world)
        .find(|(_, found)| **found == id)
        .map(|(e, _)| e)
}
fn controller(app: &App) -> &AiController {
    app.world()
        .resource::<AiPlayers>()
        .controller(PlayerId(0))
        .unwrap()
}
fn owner(app: &mut App, id: SimId) -> Option<PlayerId> {
    entity(app, id).and_then(|e| app.world().get::<Owner>(e).map(|o| o.0))
}
fn observe(app: &mut App, controllers: AiPlayers) {
    let c = controllers.controller(PlayerId(0)).unwrap();
    while !due(PlayerId(0), c, app.world().resource::<SimTick>().0) {
        step(app);
    }
    app.world_mut().insert_resource(controllers);
    step(app);
}
fn board(app: &mut App, passenger: SimId, host: SimId) {
    issue(
        app,
        0,
        Command::Interact {
            unit: passenger,
            target: host,
        },
    );
    for _ in 0..200 {
        step(app);
        if app
            .world()
            .resource::<crate::SpecialState>()
            .cargo
            .get(&host)
            .is_some_and(|p| p.iter().any(|p| p.id == passenger))
        {
            return;
        }
    }
    panic!("public boarding never completed for {passenger:?} into {host:?}");
}

fn admitted_infantry() -> (App, SimId, SimId) {
    let mut app = app();
    let soldier = unit(&mut app, 0, "gi", CellPos::new(14, 14));
    let target = building(&mut app, 1, "power_plant", CellPos::new(30, 8));
    for _ in 0..5000 {
        step(&mut app);
        if controller(&app)
            .building_jobs
            .get(&target)
            .is_some_and(|j| j.actors.contains_key(&soldier))
        {
            assert!(controller(&app).observed_units.contains_key(&soldier));
            return (app, soldier, target);
        }
    }
    panic!("actual AI never admitted this infantry's attack");
}

fn nested_infantry() -> (App, AiPlayers, SimId, SimId, SimId, SimId) {
    let (mut app, soldier, target) = admitted_infantry();
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<AiPlayers>());
    *app.world_mut().resource_mut::<CommandQueue>() = CommandQueue::default();
    let e = entity(&mut app, soldier).unwrap();
    let at = app.world().get::<Mobile>(e).unwrap().cell;
    let inner = unit(&mut app, 0, "flak_track", CellPos::new(at.x - 1, at.y));
    let outer = unit(
        &mut app,
        0,
        "amphibious_transport",
        CellPos::new(at.x, at.y + 1),
    );
    board(&mut app, soldier, inner);
    let engineer = unit(&mut app, 0, "engineer", CellPos::new(at.x + 1, at.y));
    board(&mut app, engineer, inner);
    board(&mut app, inner, outer);
    assert!(entity(&mut app, soldier).is_none());
    assert!(entity(&mut app, inner).is_none());
    assert_eq!(owner(&mut app, outer), Some(PlayerId(0)));
    (app, controllers, soldier, inner, outer, target)
}

#[test]
fn ra9_recursive_owned_cargo_keeps_real_ai_memory_and_attack_attempt_then_unloads_same_ids() {
    let (mut app, controllers, soldier, inner, outer, target) = nested_infantry();
    let before = controllers.controller(PlayerId(0)).unwrap().building_jobs[&target].clone();
    observe(&mut app, controllers);
    let c = controller(&app);
    eprintln!(
        "native nested owned cargo: tracked={}, detector={}, outstanding={}, failures={}, prior_failures={}",
        c.observed_units.contains_key(&soldier),
        c.unseen_loss,
        c.building_jobs[&target].actors.contains_key(&soldier),
        c.building_jobs[&target].failures,
        before.failures
    );
    assert!(
        c.observed_units.contains_key(&soldier),
        "living owned grandchild must remain in actual memory"
    );
    assert!(
        !c.unseen_loss,
        "real nested embark must not request a detector as a fictitious casualty"
    );
    for p in app
        .world()
        .resource::<crate::SpecialState>()
        .cargo_descendants(outer)
    {
        if app
            .world()
            .resource::<Definitions>()
            .metadata(crate::ProductionItem::Unit(p.kind))
            .is_some_and(|m| matches!(m.role, Role::Engineer | Role::Spy | Role::Mcv))
        {
            assert!(
                !c.observed_units.contains_key(&p.id),
                "recursive census preserves existing noncombat-role exclusions"
            );
        }
    }
    let job = &c.building_jobs[&target];
    assert!(
        job.actors.contains_key(&soldier),
        "actual admitted attacker must remain outstanding while nested"
    );
    assert_eq!(
        job.failures, before.failures,
        "nested boarding must not finance a new failed attempt"
    );
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<AiPlayers>());
    *app.world_mut().resource_mut::<CommandQueue>() = CommandQueue::default();
    issue(&mut app, 0, Command::Deploy { unit: outer });
    step(&mut app);
    assert_eq!(owner(&mut app, inner), Some(PlayerId(0)));
    issue(&mut app, 0, Command::Deploy { unit: inner });
    step(&mut app);
    assert_eq!(owner(&mut app, soldier), Some(PlayerId(0)));
    observe(&mut app, controllers);
    assert!(controller(&app).observed_units.contains_key(&soldier));
    assert_eq!(
        controller(&app).building_jobs[&target].failures,
        before.failures,
        "ordinary unloaded order replacement is not a casualty"
    );
}

#[test]
fn ra9_recursive_ai_census_releases_real_captured_and_destroyed_roots() {
    for capture in [true, false] {
        let (mut app, controllers, soldier, inner, outer, target) = nested_infantry();
        let e = entity(&mut app, outer).unwrap();
        let at = app.world().get::<Mobile>(e).unwrap().cell;
        let enemy = unit(
            &mut app,
            1,
            if capture { "yuri" } else { "apocalypse" },
            CellPos::new(at.x + 4, at.y),
        );
        step(&mut app);
        issue(
            &mut app,
            1,
            Command::Attack {
                units: vec![enemy],
                target: outer,
            },
        );
        for _ in 0..2000 {
            step(&mut app);
            if owner(&mut app, outer) != Some(PlayerId(0)) {
                break;
            }
        }
        if capture {
            assert_eq!(
                owner(&mut app, outer),
                Some(PlayerId(1)),
                "real Controller shot must change root ownership"
            );
            assert!(
                app.world()
                    .resource::<crate::SpecialState>()
                    .cargo_descendants(outer)
                    .iter()
                    .any(|p| p.id == soldier)
            );
        } else {
            assert!(
                entity(&mut app, outer).is_none(),
                "actual public weapon damage must destroy root"
            );
            for id in [outer, inner, soldier] {
                assert!(
                    app.world()
                        .resource::<crate::SpecialState>()
                        .cargo_descendants(outer)
                        .iter()
                        .all(|p| p.id != id)
                );
            }
        }
        issue(&mut app, 1, Command::Stop { units: vec![enemy] });
        step(&mut app);
        observe(&mut app, controllers);
        let c = controller(&app);
        assert!(
            !c.observed_units.contains_key(&soldier),
            "captured/dead roots cannot preserve an owned grandchild"
        );
        assert!(!c.building_jobs[&target].actors.contains_key(&soldier));
        assert_eq!(
            c.building_jobs[&target].failures, 1,
            "real root loss/capture must complete exactly one admitted attempt"
        );
    }
}

fn financed_cargo_cohort(nested: bool, lost: bool) {
    let (mut app, first, target) = admitted_infantry();
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<AiPlayers>());
    *app.world_mut().resource_mut::<CommandQueue>() = CommandQueue::default();
    assert_eq!(
        controllers
            .controller(PlayerId(0))
            .unwrap()
            .difficulty
            .wave_size,
        16,
        "use shipped Brutal cohort, no retune"
    );
    building(&mut app, 0, "barracks", CellPos::new(6, 14));
    let first_entity = entity(&mut app, first).unwrap();
    let at = app.world().get::<Mobile>(first_entity).unwrap().cell;
    let inner = unit(&mut app, 0, "flak_track", CellPos::new(at.x - 1, at.y));
    let outer = unit(
        &mut app,
        0,
        "amphibious_transport",
        CellPos::new(at.x, at.y + 1),
    );
    let mut payload = vec![first];
    for _ in 0..2 {
        payload.push(unit(&mut app, 0, "gi", at));
    }
    for _ in 0..2 {
        payload.push(unit(&mut app, 0, "gi", CellPos::new(at.x + 1, at.y)));
    }
    for passenger in &payload {
        board(&mut app, *passenger, if nested { inner } else { outer });
    }
    board(&mut app, inner, outer);
    let mut field = Vec::new();
    for (x, y) in [(26, 8), (26, 9), (26, 10)] {
        for _ in 0..3 {
            field.push(unit(&mut app, 0, "gi", CellPos::new(x, y)));
        }
    }
    field.push(unit(&mut app, 0, "gi", CellPos::new(27, 10)));
    issue(
        &mut app,
        0,
        Command::Attack {
            units: field.clone(),
            target,
        },
    );
    step(&mut app);
    assert!(
        field.iter().any(|id| {
            let e = entity(&mut app, *id).unwrap();
            app.world()
                .get::<Weapon>(e)
                .is_some_and(|w| w.target == Some(target))
                && app
                    .world()
                    .get::<Mobile>(e)
                    .is_some_and(|m| m.step.is_none())
        }),
        "funding control requires actual stationary combat"
    );
    if lost {
        let enemy = unit(&mut app, 1, "apocalypse", CellPos::new(at.x + 4, at.y + 1));
        step(&mut app);
        issue(
            &mut app,
            1,
            Command::Attack {
                units: vec![enemy],
                target: outer,
            },
        );
        for _ in 0..2000 {
            step(&mut app);
            if entity(&mut app, outer).is_none() {
                break;
            }
        }
        assert!(entity(&mut app, outer).is_none());
        assert!(
            app.world()
                .resource::<crate::SpecialState>()
                .cargo_descendants(outer)
                .is_empty()
        );
        issue(&mut app, 1, Command::Stop { units: vec![enemy] });
        step(&mut app);
    }
    assert_eq!(
        app.world()
            .resource::<Players>()
            .get(PlayerId(0))
            .unwrap()
            .credits(),
        0,
        "no income or fixture grant"
    );
    observe(&mut app, controllers);
    let sales: Vec<_> = app
        .world()
        .resource::<CommandQueue>()
        .iter()
        .filter(|i| i.player == PlayerId(0))
        .filter_map(|i| match i.command {
            Command::SellBuilding { building } => Some(building),
            _ => None,
        })
        .collect();
    assert_eq!(
        !sales.is_empty(),
        lost,
        "only real loss below shipped cohort size may release reinforcement capital; nested={nested}"
    );
    let events = step(&mut app);
    assert_eq!(
        events.iter().any(|e| matches!(
            e,
            crate::SimEvent::BuildingSold {
                player: PlayerId(0),
                ..
            }
        )),
        lost,
        "queued financial decision must reach actual public sale"
    );
}

#[test]
fn ra9_nested_owned_cohort_direct_financing_control() {
    financed_cargo_cohort(false, false);
}

#[test]
fn ra9_nested_owned_cohort_nested_financing_preserves_real_asset() {
    financed_cargo_cohort(true, false);
}

#[test]
fn ra9_nested_owned_cohort_real_root_loss_releases_reinforcement_finance() {
    financed_cargo_cohort(true, true);
}
