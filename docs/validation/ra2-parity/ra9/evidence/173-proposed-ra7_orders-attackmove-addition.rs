
#[test]
fn ra9_attack_move_combat_at_destination_emits_only_one_completion_for_one_order() {
    fn scenario(enemy: bool) -> (App, SimId, Option<SimId>, Vec<SimEvent>) {
        let mut app = world();
        let tank = put(&mut app, P0, "grizzly", 12, 12);
        tick(&mut app, vec![]); // establish actual sight before the command
        let victim = enemy.then(|| {
            let id = put(&mut app, P1, "engineer", 14, 12);
            let e = entity(&mut app, id).expect("victim");
            app.world_mut().get_mut::<sim::Health>(e).expect("health").hit_points = 1;
            id
        });
        let events = send(&mut app, Command::AttackMove {
            units: vec![tank], target: CellPos::new(12, 12),
        });
        (app, tank, victim, events)
    }
    for enemy in [false, true] {
        let (mut app, tank, victim, mut events) = scenario(enemy);
        events.extend(run(&mut app, 160));
        assert!(rejected(&events).is_empty(), "{events:?}");
        assert_eq!(at(&mut app, tank), CellPos::new(12, 12));
        if let Some(victim) = victim {
            assert!(events.iter().any(|event| matches!(event,
                SimEvent::WeaponFired { attacker, target, .. } if *attacker == tank && *target == victim)));
            assert!(events.iter().any(|event| matches!(event,
                SimEvent::UnitDestroyed { unit, .. } if *unit == victim)));
        }
        assert_eq!(events.iter().filter(|event| matches!(event,
            SimEvent::MoveCompleted { unit, .. } if *unit == tank)).count(), 1,
            "one accepted order has one completion; enemy={enemy}: {events:?}");
    }
}
