//! Engine-only intent lifecycle regression fixtures; public commands use the same schedule.
use super::*;
use crate::{Cell, Controller, MatchOptions, PlayerSetup, SimPlugin, SimSchedule};
use bevy_app::App;

fn app() -> App {
    let data = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data");
    let definitions = crate::load_ruleset(&data).unwrap();
    let mut skirmish = crate::load_skirmish(data.join("skirmish.ron"), &definitions).unwrap();
    skirmish.maps[0].starts = vec![CellPos::new(8, 8), CellPos::new(38, 38)];
    let ai = crate::load_ai(data.join("ai.ron"), &definitions).unwrap();
    let faction = skirmish
        .factions
        .iter()
        .position(|f| f.id == "russia")
        .unwrap();
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 103 });
    app.world_mut().insert_resource(
        Grid::from_cells(
            48,
            48,
            vec![
                Cell {
                    passable: true,
                    buildable: true,
                    water: false,
                    ore: 0
                };
                48 * 48
            ],
        )
        .unwrap(),
    );
    app.world_mut().insert_resource(definitions);
    let players = [0, 1].map(|p| PlayerSetup {
        player: PlayerId(p),
        faction,
        start: p as usize,
        controller: if p == 0 {
            Controller::Ai { difficulty: 0 }
        } else {
            Controller::Human
        },
    });
    crate::setup_match_configured(
        app.world_mut(),
        &skirmish,
        &ai,
        0,
        &players,
        &MatchOptions {
            mcv_start: false,
            crates: false,
            unit_count: Some(0),
            ..MatchOptions::default()
        },
    )
    .unwrap();
    app.world_mut().run_schedule(SimSchedule);
    // Keep the real AI owner, while isolating this boundary from its startup building
    // intent. Its next ordinary decision is tick40, beyond these command probes.
    *app.world_mut().resource_mut::<CommandQueue>() = CommandQueue::default();
    app
}

fn unit(app: &mut App, player: u8, kind: &str, x: i32, y: i32) -> SimId {
    crate::spawn_unit(app.world_mut(), PlayerId(player), kind, CellPos::new(x, y)).unwrap()
}
fn entity(app: &mut App, id: SimId) -> Entity {
    let world = app.world_mut();
    world
        .query::<(Entity, &SimId)>()
        .iter(world)
        .find(|(_, found)| **found == id)
        .unwrap()
        .0
}
fn tick(app: &mut App) -> Vec<SimEvent> {
    app.world_mut().run_schedule(SimSchedule);
    app.world()
        .resource::<Messages<SimEvent>>()
        .iter_current_update_messages()
        .cloned()
        .collect()
}
fn issue(app: &mut App, command: Command, witness: Option<AiIntent>) {
    let issued = IssuedCommand {
        player: PlayerId(0),
        command,
    };
    let mut queue = app.world_mut().resource_mut::<CommandQueue>();
    if let Some(witness) = witness {
        queue.push_ai(issued, witness);
    } else {
        queue.push(issued);
    }
}
fn witness(ids: &[SimId], target: Option<SimId>) -> AiIntent {
    AiIntent {
        owned: ids.iter().copied().collect(),
        armed: ids.iter().copied().collect(),
        target,
    }
}

#[test]
fn ra8_internal_mixed_and_fully_stale_selections_prune_only_witnessed_lifecycle_changes() {
    for internal in [false, true] {
        for changed_owner in [false, true] {
            for all in [false, true] {
                let mut app = app();
                let a = unit(&mut app, 0, "rhino", 20, 8);
                let b = unit(&mut app, 0, "rhino", 22, 8);
                tick(&mut app); // Establish the decision snapshot's normal visibility first.
                let proof = witness(&[a, b], None); // Both are live and owned at decision time.
                for id in if all { vec![a, b] } else { vec![a] } {
                    let e = entity(&mut app, id);
                    if changed_owner {
                        app.world_mut().get_mut::<Owner>(e).unwrap().0 = PlayerId(1);
                    } else {
                        app.world_mut()
                            .get_mut::<crate::Health>(e)
                            .unwrap()
                            .hit_points = 0;
                    }
                }
                issue(
                    &mut app,
                    Command::Move {
                        units: vec![a, b],
                        target: CellPos::new(27, 12),
                    },
                    internal.then_some(proof),
                );
                let events = tick(&mut app);
                let rejection = events.iter().find_map(|e| {
                    if let SimEvent::CommandRejected { reason, .. } = e {
                        Some(reason)
                    } else {
                        None
                    }
                });
                if internal {
                    assert!(rejection.is_none(), "{events:?}");
                } else if changed_owner {
                    assert_eq!(rejection, Some(&RejectReason::NotOwned(a)));
                } else {
                    assert_eq!(rejection, Some(&RejectReason::UnknownUnit(a)));
                }
                if !all {
                    let e = entity(&mut app, b);
                    assert_eq!(
                        app.world().get::<Orders>(e).unwrap().current.is_some(),
                        internal
                    );
                }
            }
        }
    }
}

#[test]
fn ra8_internal_targets_cancel_after_death_capture_or_real_detector_loss_but_public_stays_strict() {
    for internal in [false, true] {
        for cause in 0..3 {
            let mut app = app();
            let a = unit(&mut app, 0, "rhino", 20, 8);
            let target = unit(&mut app, 1, if cause == 2 { "spy" } else { "rhino" }, 24, 8);
            let dog = unit(&mut app, 0, "attack_dog", 23, 9);
            tick(&mut app);
            assert!(
                !app.world()
                    .resource::<crate::SpecialState>()
                    .hidden
                    .contains(&(PlayerId(0), target))
            );
            let proof = witness(&[a], Some(target));
            let e = entity(&mut app, if cause == 2 { dog } else { target });
            if cause == 1 {
                app.world_mut().get_mut::<Owner>(e).unwrap().0 = PlayerId(0);
            } else {
                app.world_mut()
                    .get_mut::<crate::Health>(e)
                    .unwrap()
                    .hit_points = 0;
            }
            issue(
                &mut app,
                Command::Attack {
                    units: vec![a],
                    target,
                },
                internal.then_some(proof),
            );
            let events = tick(&mut app);
            let rejection = events.iter().find_map(|e| {
                if let SimEvent::CommandRejected { reason, .. } = e {
                    Some(reason)
                } else {
                    None
                }
            });
            if internal {
                assert!(rejection.is_none(), "{events:?}");
            } else if cause == 1 {
                assert_eq!(rejection, Some(&RejectReason::OwnTarget(target)));
            } else {
                assert_eq!(rejection, Some(&RejectReason::UnknownTarget(target)));
            }
            if cause == 2 {
                assert!(
                    app.world()
                        .resource::<crate::SpecialState>()
                        .hidden
                        .contains(&(PlayerId(0), target))
                );
            }
        }
    }
}

#[test]
fn ra8_intrinsically_invalid_internal_intents_still_reject_with_original_public_reasons() {
    for case in 0..10 {
        let mut app = app();
        let a = unit(&mut app, 0, "rhino", 20, 8);
        let spy = unit(&mut app, 1, "spy", 24, 8);
        let foreign = unit(&mut app, 1, "rhino", 26, 8);
        tick(&mut app);
        let (command, reason) = match case {
            0 => (
                Command::Move {
                    units: vec![SimId(999999)],
                    target: CellPos::new(27, 12),
                },
                RejectReason::UnknownUnit(SimId(999999)),
            ),
            1 => (
                Command::Move {
                    units: vec![foreign],
                    target: CellPos::new(27, 12),
                },
                RejectReason::NotOwned(foreign),
            ),
            2 => (
                Command::Attack {
                    units: vec![a],
                    target: spy,
                },
                RejectReason::UnknownTarget(spy),
            ),
            3 => (
                Command::Attack {
                    units: vec![a],
                    target: SimId(999999),
                },
                RejectReason::UnknownTarget(SimId(999999)),
            ),
            4 => (
                Command::Move {
                    units: vec![],
                    target: CellPos::new(27, 12),
                },
                RejectReason::NoUnits,
            ),
            5 => (
                Command::Move {
                    units: vec![a],
                    target: CellPos::new(100, 12),
                },
                RejectReason::OutsideMap(CellPos::new(100, 12)),
            ),
            6 => (
                Command::Move {
                    units: vec![a, a],
                    target: CellPos::new(27, 12),
                },
                RejectReason::DuplicateUnit(a),
            ),
            7 => (
                Command::QueueProduction {
                    item: ProductionItem::Unit(crate::UnitKind(u16::MAX)),
                },
                RejectReason::UnknownItem(ProductionItem::Unit(crate::UnitKind(u16::MAX))),
            ),
            8 => {
                let k = app
                    .world()
                    .resource::<Definitions>()
                    .building_kind("sentry_gun")
                    .unwrap();
                (
                    Command::PlaceBuilding {
                        building: k,
                        origin: CellPos::new(12, 12),
                    },
                    RejectReason::NotReady(k),
                )
            }
            _ => (
                Command::Deploy { unit: a },
                RejectReason::AbilityUnavailable("This unit has no deploy action"),
            ),
        };
        // No fact is witnessed when the decision-time selection/target is already invalid.
        issue(&mut app, command, Some(AiIntent::default()));
        assert!(tick(&mut app).iter().any(
            |e| matches!(e, SimEvent::CommandRejected { reason: found, .. } if *found == reason)
        ));
    }
}

#[test]
fn ra8_pending_intent_origin_witness_payload_and_order_are_value_hashed() {
    let mut app = app();
    let a = unit(&mut app, 0, "rhino", 20, 8);
    let base = crate::state_hash(app.world_mut());
    let command = Command::Move {
        units: vec![a],
        target: CellPos::new(27, 12),
    };
    issue(&mut app, command.clone(), None);
    let public = crate::state_hash(app.world_mut());
    *app.world_mut().resource_mut::<CommandQueue>() = CommandQueue::default();
    issue(&mut app, command.clone(), Some(witness(&[a], None)));
    let internal = crate::state_hash(app.world_mut());
    assert_ne!(base, public);
    assert_ne!(public, internal);
    *app.world_mut().resource_mut::<CommandQueue>() = CommandQueue::default();
    issue(&mut app, command.clone(), Some(AiIntent::default()));
    assert_ne!(internal, crate::state_hash(app.world_mut()));
    let mut hashes = BTreeSet::from([internal]);
    for proof in [
        AiIntent::default(),
        AiIntent {
            owned: [a].into(),
            ..AiIntent::default()
        },
        AiIntent {
            armed: [a].into(),
            ..AiIntent::default()
        },
        witness(&[a], Some(a)),
    ] {
        *app.world_mut().resource_mut::<CommandQueue>() = CommandQueue::default();
        issue(&mut app, command.clone(), Some(proof));
        assert!(hashes.insert(crate::state_hash(app.world_mut())));
    }
    let mut order_hash = None;
    for reverse in [false, true] {
        *app.world_mut().resource_mut::<CommandQueue>() = CommandQueue::default();
        let stop = Command::Stop { units: vec![a] };
        for cmd in if reverse {
            vec![stop, command.clone()]
        } else {
            vec![command.clone(), stop]
        } {
            issue(&mut app, cmd, Some(witness(&[a], None)));
        }
        let h = crate::state_hash(app.world_mut());
        if let Some(previous) = order_hash {
            assert_ne!(previous, h);
        }
        order_hash = Some(h);
        assert_ne!(internal, h);
    }
}

#[test]
fn ra8_internal_and_public_commands_keep_fifo_across_ordinary_and_ability_boundaries() {
    for reverse in [false, true] {
        let mut app = app();
        let a = unit(&mut app, 0, "rhino", 20, 8);
        tick(&mut app);
        let move_command = Command::Move {
            units: vec![a],
            target: CellPos::new(27, 12),
        };
        let stop = Command::Stop { units: vec![a] };
        if reverse {
            issue(&mut app, stop, None);
            issue(&mut app, move_command, Some(witness(&[a], None)));
        } else {
            issue(&mut app, move_command, Some(witness(&[a], None)));
            issue(&mut app, stop, None);
        }
        let events = tick(&mut app);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, SimEvent::CommandRejected { .. }))
        );
        let e = entity(&mut app, a);
        assert_eq!(
            app.world().get::<Orders>(e).unwrap().current.is_some(),
            reverse
        );
    }
    let mut app = app();
    let gi = unit(&mut app, 0, "gi", 20, 8);
    tick(&mut app);
    issue(&mut app, Command::Deploy { unit: gi }, None);
    issue(
        &mut app,
        Command::Move {
            units: vec![gi],
            target: CellPos::new(27, 12),
        },
        Some(witness(&[gi], None)),
    );
    issue(
        &mut app,
        Command::Move {
            units: vec![gi],
            target: CellPos::new(27, 12),
        },
        None,
    );
    let events = tick(&mut app);
    let rejections: Vec<_> = events
        .iter()
        .filter_map(|e| {
            if let SimEvent::CommandRejected { reason, .. } = e {
                Some(reason)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(rejections, vec![&RejectReason::NotCommandable(gi)]);
}
