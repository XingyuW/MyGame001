// Included in weapon_fx::tests. Initial damaged HP is scenario setup; Condition, detection,
// reconcealment and actual smoulder output are production paths.
#[test]
fn ra9_privacy_damage_smoke_stops_new_emission_after_native_reconcealment() {
    use crate::privacy_native as privacy;
    use crate::units::HUMAN;
    use bevy::ecs::system::RunSystemOnce;
    use sim::Command;
    let mut leaks = Vec::new();
    for kind in ["typhoon", "mirage_tank"] {
        let mut plan = crate::sim_host::tests::privacy_plan(true);
        if kind == "typhoon" {
            for y in 18..27 {
                for x in 2..41 {
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
        }
        let mut ids = None;
        let host = crate::sim_host::tests::privacy_host(&plan, |world| {
            let target =
                sim::spawn_unit(world, privacy::ENEMY, kind, CellPos::new(26, 20)).unwrap();
            // Controlled initial damage isolates emission privacy from the damage equation.
            // No Condition, hidden/disguise relation or output particle is seeded.
            let target_entity = world
                .query::<(Entity, &SimId)>()
                .iter(world)
                .find(|(_, id)| **id == target)
                .unwrap()
                .0;
            let target_kind = world.get::<sim::Unit>(target_entity).unwrap().kind;
            let max = world
                .resource::<sim::Definitions>()
                .unit_combat(target_kind)
                .unwrap()
                .hit_points;
            world
                .get_mut::<sim::Health>(target_entity)
                .unwrap()
                .hit_points = max / 3;
            world.entity_mut(target_entity).remove::<sim::Weapon>();
            let source = sim::spawn_unit(
                world,
                HUMAN,
                if kind == "typhoon" {
                    "destroyer"
                } else {
                    "engineer"
                },
                CellPos::new(22, 20),
            )
            .unwrap();
            let detector = if kind == "typhoon" {
                source
            } else {
                sim::spawn_unit(world, HUMAN, "attack_dog", CellPos::new(23, 20)).unwrap()
            };
            sim::spawn_unit(world, HUMAN, "rocketeer", CellPos::new(26, 18)).unwrap();
            let entities: Vec<_> = world
                .query::<(Entity, &sim::Owner)>()
                .iter(world)
                .filter(|(_, owner)| owner.0 == HUMAN)
                .map(|(e, _)| e)
                .collect();
            for entity in entities {
                world.entity_mut(entity).remove::<sim::Weapon>();
            }
            ids = Some((target, source, detector));
        });
        let (target, source, detector) = ids.unwrap();
        let mut app = privacy::app(host);
        crate::battlefield::privacy_materials(&mut app);
        app.insert_resource(FxArt {
            beam: Handle::default(),
            streak: Handle::default(),
        });
        app.insert_resource(crate::game_speed::GameClock::privacy_frame(1.0));
        privacy::step(&mut app, Vec::new());
        assert!(
            !privacy::hidden(&app, target),
            "{kind}: actual detector positive"
        );
        {
            let host = app.world().resource::<SimHost>();
            let entity = privacy::entity(host, target);
            assert!(
                damage_smoke(host.world().get::<sim::Condition>(entity).unwrap().0).is_some(),
                "{kind}: native damage reached a smoking condition"
            );
            assert!(host.world().get::<sim::Health>(entity).unwrap().hit_points > 0);
        }
        app.world_mut()
            .run_system_once(smoulder)
            .expect("actual visible smoulder");
        let old_puffs: Vec<_> = {
            let world = app.world_mut();
            world
                .query_filtered::<(Entity, &Transform), With<Fx>>()
                .iter(world)
                .map(|(e, t)| (e, *t))
                .collect()
        };
        assert!(
            !old_puffs.is_empty(),
            "{kind}: visible damaged actor must actually emit"
        );
        privacy::step(
            &mut app,
            vec![privacy::issue(
                HUMAN,
                Command::Stop {
                    units: vec![source],
                },
            )],
        );
        privacy::move_to(&mut app, HUMAN, detector, CellPos::new(12, 20));
        assert!(
            privacy::hidden(&app, target),
            "{kind}: scheduled detector loss must reconceal target"
        );
        let host = app.world().resource::<SimHost>();
        let target_cell = privacy::cell(host, target);
        assert!(
            host.players()
                .unwrap()
                .get(HUMAN)
                .unwrap()
                .vision()
                .is_visible(target_cell),
            "{kind}: terrain stays visible; actor concealment is the sole cause"
        );
        // Old puffs are fixed historical observations, not a moving view of the actor.
        for (entity, transform) in &old_puffs {
            assert_eq!(app.world().get::<Transform>(*entity), Some(transform));
        }
        // Give this real system a new local cadence state; count only newly emitted particles.
        // This also ensures cadence cannot make a broken concealment gate accidentally pass.
        let before = app
            .world_mut()
            .query_filtered::<Entity, With<Fx>>()
            .iter(app.world())
            .count();
        app.world_mut()
            .run_system_once(smoulder)
            .expect("actual concealed smoulder");
        let after = app
            .world_mut()
            .query_filtered::<Entity, With<Fx>>()
            .iter(app.world())
            .count();
        leaks.push((kind, after.saturating_sub(before)));
        privacy::move_to(&mut app, HUMAN, detector, CellPos::new(23, 20));
        assert!(
            !privacy::hidden(&app, target),
            "{kind}: real detector return restores emission permission"
        );
        let before = app
            .world_mut()
            .query_filtered::<Entity, With<Fx>>()
            .iter(app.world())
            .count();
        app.world_mut().run_system_once(smoulder).unwrap();
        let after = app
            .world_mut()
            .query_filtered::<Entity, With<Fx>>()
            .iter(app.world())
            .count();
        assert!(
            after > before,
            "{kind}: detected damaged positive still emits actual Fx"
        );
    }
    assert!(
        leaks.iter().all(|(_, count)| *count == 0),
        "new actor-position smoke leaked through real concealment: {leaks:?}"
    );
}
