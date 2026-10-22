// Included in special_controls::tests; input/feedback and audio execute their real bodies.
fn privacy_interact_observation(
    kind: &str,
    present: bool,
    known: bool,
) -> (Vec<Command>, InterfaceMode, u64, String, u64) {
    use crate::privacy_native as privacy;
    use bevy::ecs::system::RunSystemOnce;
    let mut plan = crate::sim_host::tests::privacy_plan(true);
    let cell = if known || kind == "spy" {
        sim::CellPos::new(12, 20)
    } else {
        sim::CellPos::new(30, 20)
    };
    if kind == "scenery" && present {
        let index = (cell.y * 48 + cell.x) as usize;
        plan.looks[index] = 5 << 4; // House, installed through the real world-object path.
        plan.grid.set_terrain(
            cell,
            sim::Cell {
                passable: false,
                water: false,
                buildable: false,
                ore: 0,
            },
        );
    }
    let mut actor = None;
    let mut target = None;
    let host = crate::sim_host::tests::privacy_host(&plan, |world| {
        actor = Some(sim::spawn_unit(world, HUMAN, "engineer", sim::CellPos::new(10, 20)).unwrap());
        if present {
            target = if kind == "building" {
                Some(sim::spawn_building(world, privacy::ENEMY, "pillbox", cell).unwrap())
            } else if kind == "scenery" {
                world.resource::<sim::Props>().id_at(cell)
            } else {
                Some(
                    sim::spawn_unit(
                        world,
                        if known { HUMAN } else { privacy::ENEMY },
                        kind,
                        cell,
                    )
                    .unwrap(),
                )
            };
        }
    });
    let mut app = privacy::app(host);
    privacy::gizmos(&mut app);
    app.init_resource::<StrategicEffects>()
        .init_resource::<crate::audio::SfxState>()
        .init_resource::<crate::audio::VolumeSettings>()
        .init_resource::<crate::weapon_fx::ImpactSounds>()
        .init_resource::<crate::buildings::BuildingViews>()
        .insert_resource(crate::audio::SoundBank {
            effects: [
                (
                    crate::audio::Sfx::Acknowledge,
                    (Handle::default(), std::time::Duration::from_millis(50)),
                ),
                (
                    crate::audio::Sfx::Error,
                    (Handle::default(), std::time::Duration::from_millis(50)),
                ),
            ]
            .into_iter()
            .collect(),
        });
    let hint = app.world_mut().spawn((ActionHint, Text::new(HINT))).id();
    privacy::step(&mut app, Vec::new());
    if kind == "spy" && !known {
        if present {
            assert!(
                privacy::hidden(&app, target.unwrap()),
                "actual disguise, not a seeded hidden ID"
            );
        }
        assert!(
            app.world()
                .resource::<SimHost>()
                .players()
                .unwrap()
                .get(HUMAN)
                .unwrap()
                .vision()
                .is_visible(cell),
            "special concealment in genuinely visible terrain"
        );
    } else if !known {
        assert!(
            !app.world()
                .resource::<SimHost>()
                .players()
                .unwrap()
                .get(HUMAN)
                .unwrap()
                .vision()
                .is_visible(cell),
            "unseen target fixture"
        );
    }
    app.world_mut()
        .resource_mut::<Selection>()
        .0
        .insert(actor.unwrap());
    app.world_mut().resource_mut::<HoveredCell>().0 = Some(cell);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyI);
    app.world_mut()
        .run_system_once(input)
        .expect("I key handler");
    assert_eq!(
        *app.world().resource::<InterfaceMode>(),
        InterfaceMode::Interact
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.world_mut()
        .run_system_once(input)
        .expect("actual left-click I handler");
    let mode = *app.world().resource::<InterfaceMode>();
    let accepted = app.world().resource::<CommandSink>().accepted();
    app.world_mut()
        .run_system_once(crate::audio::play_effects)
        .expect("actual acknowledgement handler");
    let sounds = app.world().resource::<crate::audio::SfxState>().started;
    let commands = crate::sim_host::tests::take_commands(&mut app);
    privacy::step(
        &mut app,
        commands
            .iter()
            .cloned()
            .map(|command| privacy::issue(HUMAN, command))
            .collect(),
    );
    app.world_mut()
        .run_system_once(feedback_and_effects)
        .expect("actual rejection hint handler");
    let text = app.world().get::<Text>(hint).unwrap().0.clone();
    (commands, mode, accepted, text, sounds)
}

#[test]
fn ra9_privacy_interact_hidden_and_missing_have_equal_sink_mode_audio_and_text() {
    for kind in ["spy", "engineer", "rocketeer", "building", "scenery"] {
        let hidden = privacy_interact_observation(kind, true, false);
        let missing = privacy_interact_observation(kind, false, false);
        assert_eq!(
            hidden, missing,
            "I-mode leaked hidden {kind} occupancy before/through native validation"
        );
        assert_eq!(hidden.0, []);
        assert_eq!(hidden.1, InterfaceMode::Interact);
        assert_eq!((hidden.2, hidden.4), (0, 0));
    }
    let known = privacy_interact_observation("amphibious_transport", true, true);
    assert!(matches!(known.0.as_slice(), [Command::Interact { .. }]));
    assert_eq!(known.1, InterfaceMode::Normal);
    assert_eq!(
        (known.2, known.4),
        (1, 1),
        "known target still acknowledges normally"
    );
    let scenery = privacy_interact_observation("scenery", true, true);
    assert!(
        matches!(scenery.0.as_slice(), [Command::Interact { .. }]),
        "visible scenery remains a legitimate context"
    );
}
