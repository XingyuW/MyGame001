//! Engine-only intent lifecycle regression fixtures; public commands use the same schedule.
use super::*;
use crate::{Cell, Controller, Health, MatchOptions, PlayerSetup, SimPlugin, SimSchedule};
use bevy_app::App;
use bevy_ecs::world::World;

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
        strategic_valid: false,
        service_valid: false,
    }
}

#[test]
fn ra8_service_healing_certificate_keeps_public_intrinsic_capacity_and_fifo_errors_strict() {
    use bevy_ecs::schedule::IntoScheduleConfigs;
    for mode in [
        "internal",
        "public",
        "already_full",
        "already_occupied",
        "wrong_domain",
        "wrong_target",
        "earlier_occupant",
        "later_occupant",
        "later_owner",
        "later_spawned",
    ] {
        let mut app = app();
        let depot = crate::spawn_building(
            app.world_mut(),
            PlayerId(0),
            if mode == "wrong_target" {
                "soviet_barracks"
            } else {
                "soviet_service_depot"
            },
            CellPos::new(2, 20),
        )
        .unwrap();
        let unit_kind = if mode == "wrong_domain" {
            "kirov"
        } else {
            "war_miner"
        };
        let miner = unit(&mut app, 0, unit_kind, 5, 20);
        let other = unit(&mut app, 0, "rhino", 4, 21);
        let miner_entity = entity(&mut app, miner);
        let other_entity = entity(&mut app, other);
        app.world_mut()
            .get_mut::<Health>(other_entity)
            .unwrap()
            .hit_points = 10;
        let maximum = app
            .world()
            .resource::<Definitions>()
            .unit_combat(
                app.world()
                    .resource::<Definitions>()
                    .unit_kind(unit_kind)
                    .unwrap(),
            )
            .unwrap()
            .hit_points;
        app.world_mut()
            .get_mut::<Health>(miner_entity)
            .unwrap()
            .hit_points = maximum - i32::from(!["already_full", "wrong_domain"].contains(&mode));
        if mode == "already_occupied" {
            issue(
                &mut app,
                Command::Interact {
                    unit: other,
                    target: depot,
                },
                None,
            );
            tick(&mut app);
            assert_eq!(
                app.world()
                    .resource::<crate::SpecialState>()
                    .docked
                    .get(&other),
                Some(&depot)
            );
        }
        let repair_interval = app
            .world()
            .resource::<Definitions>()
            .rules()
            .unwrap()
            .economy
            .repair_interval
            .unsigned_abs();
        app.world_mut().resource_mut::<SimTick>().0 = u64::from(repair_interval);
        let decision_tick = app.world().resource::<SimTick>().0;
        app.add_systems(
            SimSchedule,
            (move |world: &mut World| {
                if world.resource::<SimTick>().0 != decision_tick {
                    return;
                }
                let service = IssuedCommand {
                    player: PlayerId(0),
                    command: Command::Interact {
                        unit: miner,
                        target: depot,
                    },
                };
                let before = crate::state_hash(world);
                let valid = crate::special::service_intent_valid(world, &service, true);
                assert_eq!(
                    before,
                    crate::state_hash(world),
                    "{mode}: certification must be read-only"
                );
                assert_eq!(
                    valid,
                    ![
                        "already_full",
                        "already_occupied",
                        "wrong_domain",
                        "wrong_target"
                    ]
                    .contains(&mode),
                    "{mode}"
                );
                let occupant = IssuedCommand {
                    player: PlayerId(0),
                    command: Command::Interact {
                        unit: other,
                        target: depot,
                    },
                };
                if mode == "earlier_occupant" {
                    world.resource_mut::<CommandQueue>().push(occupant.clone());
                }
                if mode == "public" {
                    world.resource_mut::<CommandQueue>().push(service);
                } else {
                    world
                        .resource_mut::<CommandQueue>()
                        .push_ai(service, AiIntent::default());
                }
                if mode == "later_occupant" {
                    world.resource_mut::<CommandQueue>().push(occupant);
                }
                certify_ai_strategic_intents(world);
            })
            .in_set(crate::SimSet::Ai)
            .after(crate::SimCoreSet(crate::SimSet::Ai)),
        );
        app.add_systems(
            SimSchedule,
            (move |world: &mut World| {
                if world.resource::<SimTick>().0 != decision_tick {
                    return;
                }
                if mode == "later_owner" {
                    let e = world
                        .query::<(Entity, &SimId)>()
                        .iter(world)
                        .find(|(_, id)| **id == depot)
                        .unwrap()
                        .0;
                    world.get_mut::<Owner>(e).unwrap().0 = PlayerId(1);
                }
                if mode == "later_spawned" {
                    world
                        .resource_mut::<crate::SpecialState>()
                        .spawned
                        .insert(miner, SimId(999999));
                }
            })
            .in_set(crate::SimSet::Visibility)
            .after(crate::SimCoreSet(crate::SimSet::Visibility)),
        );
        tick(&mut app);
        assert_eq!(
            app.world().get::<Health>(miner_entity).unwrap().hit_points,
            maximum,
            "{mode}: ordinary source heal"
        );
        let certified = app
            .world()
            .resource::<CommandQueue>()
            .pending
            .iter()
            .find(|p| matches!(p.issued.command, Command::Interact {unit,..} if unit==miner))
            .and_then(|p| p.ai_intent.as_ref())
            .is_some_and(|w| w.service_valid);
        assert_eq!(
            certified,
            ![
                "public",
                "already_full",
                "already_occupied",
                "wrong_domain",
                "wrong_target"
            ]
            .contains(&mode),
            "{mode}"
        );
        if certified {
            let before = crate::state_hash(app.world_mut());
            let pending = app.world_mut().resource_mut::<CommandQueue>().into_inner();
            let w = pending
                .pending
                .iter_mut()
                .find(|p| matches!(p.issued.command, Command::Interact {unit,..} if unit==miner))
                .unwrap()
                .ai_intent
                .as_mut()
                .unwrap();
            w.service_valid = false;
            assert_ne!(
                before,
                crate::state_hash(app.world_mut()),
                "service certificate must be value-hashed"
            );
            app.world_mut()
                .resource_mut::<CommandQueue>()
                .pending
                .iter_mut()
                .find(|p| matches!(p.issued.command, Command::Interact {unit,..} if unit==miner))
                .unwrap()
                .ai_intent
                .as_mut()
                .unwrap()
                .service_valid = true;
        }
        let events = tick(&mut app);
        let rejection = events.iter().find_map(|event| match event {
            SimEvent::CommandRejected {
                command: Command::Interact { unit, .. },
                reason,
                ..
            } if *unit == miner => Some(reason),
            _ => None,
        });
        let cancelled = ["internal", "later_occupant"].contains(&mode);
        assert_eq!(rejection.is_none(), cancelled, "{mode}: {events:?}");
        if let Some(reason) = rejection {
            let expected = match mode {
                "wrong_domain" => {
                    RejectReason::AbilityUnavailable("This unit is not repaired here")
                }
                "wrong_target" => RejectReason::AbilityUnavailable("Choose a friendly transport"),
                "later_owner" => {
                    RejectReason::AbilityUnavailable("Choose your own repair building")
                }
                "later_spawned" => RejectReason::NotCommandable(miner),
                _ => RejectReason::AbilityUnavailable("This unit is not damaged"),
            };
            assert_eq!(*reason, expected, "{mode}: original public error order");
        }
        assert!(
            !app.world()
                .resource::<crate::SpecialState>()
                .docked
                .contains_key(&miner),
            "{mode}"
        );
        assert!(
            !app.world()
                .resource::<crate::SpecialState>()
                .interactions
                .contains_key(&miner),
            "{mode}"
        );
        if ["earlier_occupant", "later_occupant", "already_occupied"].contains(&mode) {
            assert_eq!(
                app.world()
                    .resource::<crate::SpecialState>()
                    .docked
                    .get(&other),
                Some(&depot),
                "{mode}: ordinary FIFO occupant must still dock"
            );
        }
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

fn bridge_job_app() -> (App, SimId, SimId, SimId) {
    bridge_job_app_from(20)
}
fn bridge_job_app_from(x: i32) -> (App, SimId, SimId, SimId) {
    let (app, bridge, hut, engineer, _) = bridge_job_with_controllers(x);
    (app, bridge, hut, engineer)
}
fn bridge_job_with_controllers(x: i32) -> (App, SimId, SimId, SimId, crate::AiPlayers) {
    let mut app = app();
    let controllers = std::mem::take(&mut *app.world_mut().resource_mut::<crate::AiPlayers>());
    let mut looks = vec![0; 48 * 48];
    {
        let mut grid = app.world_mut().resource_mut::<Grid>();
        for y in 0..48 {
            for x in 22..=24 {
                grid.set_terrain(
                    CellPos::new(x, y),
                    Cell {
                        passable: false,
                        water: true,
                        buildable: false,
                        ore: 0,
                    },
                );
            }
        }
        for x in 22..=24 {
            grid.set_terrain(
                CellPos::new(x, 24),
                Cell {
                    passable: true,
                    water: false,
                    buildable: false,
                    ore: 0,
                },
            );
            looks[24 * 48 + x as usize] = 8 << 4;
        }
        for (x, y) in [(21, 22), (25, 22)] {
            looks[y * 48 + x] = 15 << 4;
        }
        grid.mark_world_features(&looks);
    }
    crate::install_world_objects(app.world_mut(), true);
    let bridge = *app
        .world()
        .resource::<crate::Bridges>()
        .bridges
        .keys()
        .next()
        .unwrap();
    let hut = app
        .world()
        .resource::<Grid>()
        .building(CellPos::new(21, 22))
        .unwrap();
    app.world_mut()
        .resource_mut::<crate::Props>()
        .hurt(CellPos::new(23, 24), i64::MAX);
    tick(&mut app); // Real scheduled collapse updates both terrain and bridge state.
    assert!(app.world().resource::<crate::Bridges>().bridges[&bridge].collapsed);
    let engineer = unit(&mut app, 0, "engineer", x, 22);
    tick(&mut app);
    (app, bridge, hut, engineer, controllers)
}

fn bridge_goal(app: &mut App, unit: SimId, hut: SimId) {
    app.world_mut()
        .resource_mut::<CommandQueue>()
        .push_bridge_goal(PlayerId(0), unit, hut, AiIntent::default());
}

#[test]
fn ra8_private_bridge_goal_planning_and_pending_hash_ignore_unseen_naval_blockers() {
    let mut decisions = Vec::new();
    for hidden_ship in [false, true] {
        let (mut app, _, hut, engineer, controllers) = bridge_job_with_controllers(20);
        let ship = hidden_ship.then(|| unit(&mut app, 1, "typhoon", 23, 24));
        tick(&mut app);
        if let Some(ship) = ship {
            assert!(
                app.world()
                    .resource::<crate::SpecialState>()
                    .hidden
                    .contains(&(PlayerId(0), ship))
            );
        }
        while !app.world().resource::<SimTick>().0.is_multiple_of(40) {
            tick(&mut app);
        }
        app.world_mut().insert_resource(controllers);
        tick(&mut app); // Actual shipped controller; no fabricated visibility/hidden state.
        let queue = app.world().resource::<CommandQueue>();
        assert!(
            queue
                .pending
                .iter()
                .any(|p| p.action == PendingAction::ConditionalBridgeRepair
                    && p.issued.command
                        == Command::Interact {
                            unit: engineer,
                            target: hut
                        })
        );
        let mut h = crate::Fnv1a64::new();
        queue.hash_into(&mut h);
        decisions.push((queue.iter().cloned().collect::<Vec<_>>(), h.finish()));
    }
    assert_eq!(
        decisions[0], decisions[1],
        "unknown physical occupancy cannot change controller output, goal tagging, or witnesses"
    );
}

#[test]
fn ra8_private_bridge_goal_waits_for_hidden_occupancy_then_consumes_engineer_only_after_clear() {
    for reserved in [false, true] {
        let (mut app, bridge, hut, engineer) = bridge_job_app();
        let ship = unit(&mut app, 1, "typhoon", 23, if reserved { 23 } else { 24 });
        tick(&mut app);
        if reserved {
            app.world_mut()
                .resource_mut::<CommandQueue>()
                .push(IssuedCommand {
                    player: PlayerId(1),
                    command: Command::Move {
                        units: vec![ship],
                        target: CellPos::new(23, 24),
                    },
                });
            tick(&mut app);
            let ship_entity = entity(&mut app, ship);
            assert_eq!(
                app.world()
                    .get::<Mobile>(ship_entity)
                    .unwrap()
                    .step
                    .unwrap()
                    .to,
                CellPos::new(23, 24)
            );
        }
        assert!(
            app.world()
                .resource::<crate::SpecialState>()
                .hidden
                .contains(&(PlayerId(0), ship))
        );
        issue(
            &mut app,
            Command::Interact {
                unit: engineer,
                target: hut,
            },
            None,
        );
        assert!(tick(&mut app).iter().any(|e| matches!(
            e,
            SimEvent::CommandRejected {
                reason: RejectReason::AbilityUnavailable(
                    "Ships must clear the collapsed bridge before repair"
                ),
                ..
            }
        )));
        bridge_goal(&mut app, engineer, hut);
        assert!(
            !tick(&mut app)
                .iter()
                .any(|e| matches!(e, SimEvent::CommandRejected { .. }))
        );
        assert!(app.world().resource::<crate::Bridges>().bridges[&bridge].collapsed);
        let engineer_entity = entity(&mut app, engineer);
        assert!(
            app.world()
                .get::<crate::Health>(engineer_entity)
                .unwrap()
                .hit_points
                > 0
        );
        app.world_mut()
            .resource_mut::<CommandQueue>()
            .push(IssuedCommand {
                player: PlayerId(1),
                command: Command::Move {
                    units: vec![ship],
                    target: CellPos::new(23, 20),
                },
            });
        for _ in 0..300 {
            tick(&mut app);
        }
        bridge_goal(&mut app, engineer, hut);
        assert!(
            tick(&mut app)
                .iter()
                .any(|e| matches!(e, SimEvent::BridgeChanged { intact: true, .. }))
        );
        assert!(!app.world().resource::<crate::Bridges>().bridges[&bridge].collapsed);
        assert!(app.world().get::<Unit>(engineer_entity).is_none());
    }
}

#[test]
fn ra8_private_bridge_goal_preserves_fifo_and_rechecks_arrival_occupancy() {
    for stop_after in [false, true] {
        let (mut app, bridge, hut, engineer) = bridge_job_app_from(18);
        let e = entity(&mut app, engineer);
        // A far Engineer starts a genuine Interact travel job on a clear deck.
        if !stop_after {
            issue(
                &mut app,
                Command::Stop {
                    units: vec![engineer],
                },
                None,
            );
        }
        bridge_goal(&mut app, engineer, hut);
        if stop_after {
            issue(
                &mut app,
                Command::Stop {
                    units: vec![engineer],
                },
                None,
            );
        }
        let events = tick(&mut app);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, SimEvent::CommandRejected { .. })),
            "{events:?}"
        );
        assert_eq!(
            app.world()
                .resource::<crate::SpecialState>()
                .interactions
                .get(&engineer)
                == Some(&hut),
            !stop_after
        );
        if stop_after {
            continue;
        }
        let ship = unit(&mut app, 1, "typhoon", 23, 24);
        for _ in 0..400 {
            tick(&mut app);
        }
        assert!(app.world().resource::<crate::Bridges>().bridges[&bridge].collapsed);
        assert!(app.world().get::<crate::Health>(e).unwrap().hit_points > 0);
        app.world_mut()
            .resource_mut::<CommandQueue>()
            .push(IssuedCommand {
                player: PlayerId(1),
                command: Command::Move {
                    units: vec![ship],
                    target: CellPos::new(23, 20),
                },
            });
        for _ in 0..300 {
            tick(&mut app);
        }
        bridge_goal(&mut app, engineer, hut);
        let mut repaired = false;
        for _ in 0..400 {
            repaired |= tick(&mut app)
                .iter()
                .any(|event| matches!(event, SimEvent::BridgeChanged { intact: true, .. }));
        }
        assert!(repaired);
        assert!(app.world().get::<Unit>(e).is_none());
    }
}

#[test]
fn ra8_private_bridge_goal_authorizes_before_readiness_and_hashes_its_distinct_action() {
    for blocked in [false, true] {
        for fault in [
            "missing_actor",
            "dead_actor",
            "enemy_actor",
            "hidden_actor",
            "wrong_type",
            "missing_hut",
            "dead_hut",
            "hidden_hut",
            "wrong_target",
        ] {
            let (mut app, _, hut, engineer) = bridge_job_app();
            if blocked {
                unit(&mut app, 1, "typhoon", 23, 24);
                tick(&mut app);
            }
            let (actor, target) = match fault {
                "missing_actor" => (SimId(999999), hut),
                "dead_actor" => {
                    let e = entity(&mut app, engineer);
                    app.world_mut()
                        .get_mut::<crate::Health>(e)
                        .unwrap()
                        .hit_points = 0;
                    (engineer, hut)
                }
                "enemy_actor" => (unit(&mut app, 1, "engineer", 19, 22), hut),
                "hidden_actor" => {
                    let id = unit(&mut app, 1, "spy", 19, 22);
                    tick(&mut app);
                    (id, hut)
                }
                "wrong_type" => (unit(&mut app, 0, "rhino", 19, 22), hut),
                "missing_hut" => (engineer, SimId(999998)),
                "dead_hut" => {
                    let e = entity(&mut app, hut);
                    app.world_mut()
                        .get_mut::<crate::Health>(e)
                        .unwrap()
                        .hit_points = 0;
                    (engineer, hut)
                }
                "hidden_hut" => {
                    let e = entity(&mut app, hut);
                    app.world_mut().get_mut::<Owner>(e).unwrap().0 = PlayerId(1);
                    app.world_mut()
                        .resource_mut::<crate::Players>()
                        .get_mut(PlayerId(0))
                        .unwrap()
                        .vision
                        .reshroud(CellPos::new(21, 22), 20);
                    (engineer, hut)
                }
                _ => (engineer, engineer),
            };
            let issued = IssuedCommand {
                player: PlayerId(0),
                command: Command::Interact {
                    unit: actor,
                    target,
                },
            };
            let reason =
                crate::special::bridge_goal_ready(app.world_mut(), &issued).expect_err(fault);
            match fault {
                "missing_actor" | "dead_actor" | "hidden_actor" => {
                    assert_eq!(reason, RejectReason::UnknownUnit(actor))
                }
                "enemy_actor" => assert_eq!(reason, RejectReason::NotOwned(actor)),
                "missing_hut" | "dead_hut" | "hidden_hut" => {
                    assert_eq!(reason, RejectReason::UnknownTarget(target))
                }
                _ => assert_eq!(
                    reason,
                    RejectReason::AbilityUnavailable("Choose an Engineer and bridge repair hut")
                ),
            }
            bridge_goal(&mut app, actor, target);
            assert!(
                tick(&mut app).iter().any(
                    |e| matches!(e,SimEvent::CommandRejected {reason:found,..} if *found==reason)
                ),
                "{fault}"
            );
        }
    }
    let (mut app, _, hut, engineer) = bridge_job_app();
    let issued = IssuedCommand {
        player: PlayerId(0),
        command: Command::Interact {
            unit: engineer,
            target: hut,
        },
    };
    app.world_mut()
        .resource_mut::<CommandQueue>()
        .push_ai(issued.clone(), AiIntent::default());
    let ordinary = crate::state_hash(app.world_mut());
    *app.world_mut().resource_mut::<CommandQueue>() = CommandQueue::default();
    bridge_goal(&mut app, engineer, hut);
    assert_ne!(ordinary, crate::state_hash(app.world_mut()));
}

#[test]
fn ra8_strategic_erasure_cancels_only_a_certified_later_actor_change() {
    use bevy_ecs::schedule::IntoScheduleConfigs;
    let mut outcomes = Vec::new();
    for mode in [
        "internal",
        "public",
        "already_erased",
        "became_unready",
        "became_spawned",
        "became_wrong_type",
        "became_arguments",
        "wrong_type",
        "wrong_form",
        "outside",
        "unit_arguments",
    ] {
        let mut app = app();
        app.world_mut().insert_resource(crate::AiPlayers::default());
        crate::spawn_building(
            app.world_mut(),
            PlayerId(0),
            "nuclear_reactor",
            CellPos::new(2, 2),
        )
        .unwrap();
        crate::spawn_building(
            app.world_mut(),
            PlayerId(0),
            "spy_satellite",
            CellPos::new(14, 2),
        )
        .unwrap();
        let ability = match mode {
            "wrong_type" => "barracks",
            "wrong_form" => "chronosphere",
            _ => "nuclear_silo",
        };
        let source =
            crate::spawn_building(app.world_mut(), PlayerId(0), ability, CellPos::new(2, 20))
                .unwrap();
        tick(&mut app); // Ordinary source announcement before isolated readiness.
        let cooldown = app
            .world()
            .resource::<Definitions>()
            .metadata(ProductionItem::Building(
                app.world()
                    .resource::<Definitions>()
                    .building_kind(ability)
                    .unwrap(),
            ))
            .unwrap()
            .cooldown;
        app.world_mut()
            .resource_mut::<crate::SpecialState>()
            .charge
            .insert(source, cooldown);
        let decision_tick = app.world().resource::<SimTick>().0;
        app.add_systems(
            SimSchedule,
            (move |world: &mut bevy_ecs::world::World| {
                if world.resource::<SimTick>().0 != decision_tick {
                    return;
                }
                if mode == "already_erased" {
                    world.resource_mut::<crate::SpecialState>().erasures.insert(
                        source,
                        crate::special::infantry::Erasure {
                            remaining: 6000,
                            beams: BTreeMap::new(),
                        },
                    );
                }
                let issued = IssuedCommand {
                    player: PlayerId(0),
                    command: Command::Superweapon {
                        building: source,
                        target: if mode == "outside" {
                            CellPos::new(-1, -1)
                        } else {
                            CellPos::new(28, 28)
                        },
                        units: if mode == "unit_arguments" {
                            vec![SimId(999999)]
                        } else {
                            Vec::new()
                        },
                    },
                };
                if mode == "public" {
                    world.resource_mut::<CommandQueue>().push(issued);
                } else {
                    world
                        .resource_mut::<CommandQueue>()
                        .push_ai(issued, AiIntent::default());
                }
                certify_ai_strategic_intents(world);
            })
            .in_set(crate::SimSet::Ai)
            .after(crate::SimCoreSet(crate::SimSet::Ai)),
        );
        app.add_systems(
            SimSchedule,
            (move |world: &mut bevy_ecs::world::World| {
                if world.resource::<SimTick>().0 != decision_tick {
                    return;
                }
                // Isolate the same late actor-state boundary that an actual Combat beam
                // creates, after the decision's complete read-only certification.
                world.resource_mut::<crate::SpecialState>().erasures.insert(
                    source,
                    crate::special::infantry::Erasure {
                        remaining: 6000,
                        beams: BTreeMap::new(),
                    },
                );
                if mode == "became_unready" {
                    world
                        .resource_mut::<crate::SpecialState>()
                        .charge
                        .insert(source, 0);
                }
                if mode == "became_spawned" {
                    world
                        .resource_mut::<crate::SpecialState>()
                        .spawned
                        .insert(source, SimId(999999));
                }
                if mode == "became_wrong_type" {
                    let kind = world
                        .resource::<Definitions>()
                        .building_kind("barracks")
                        .unwrap();
                    let entity = world
                        .query::<(Entity, &SimId)>()
                        .iter(world)
                        .find(|(_, id)| **id == source)
                        .unwrap()
                        .0;
                    world.get_mut::<crate::Building>(entity).unwrap().kind = kind;
                }
                if mode == "became_arguments" {
                    for pending in &mut world.resource_mut::<CommandQueue>().pending {
                        if let Command::Superweapon { units, .. } = &mut pending.issued.command {
                            units.push(SimId(999999));
                        }
                    }
                }
            })
            .in_set(crate::SimSet::Visibility)
            .after(crate::SimCoreSet(crate::SimSet::Visibility)),
        );
        tick(&mut app);
        let certified = app
            .world()
            .resource::<CommandQueue>()
            .pending
            .iter()
            .find_map(|p| p.ai_intent.as_ref().map(|w| w.strategic_valid))
            .unwrap_or(false);
        assert_eq!(
            certified,
            [
                "internal",
                "became_unready",
                "became_spawned",
                "became_wrong_type",
                "became_arguments"
            ]
            .contains(&mode),
            "{mode}: full source guard certification"
        );
        let events = tick(&mut app);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, SimEvent::StrategicActivated { .. })),
            "{mode}"
        );
        let rejection = events.iter().find_map(|e| match e {
            SimEvent::CommandRejected { reason, .. } => Some(reason),
            _ => None,
        });
        if let Some(reason) = rejection {
            assert_eq!(
                *reason,
                RejectReason::AbilityUnavailable(
                    if ["wrong_type", "became_wrong_type"].contains(&mode) {
                        "No strategic ability"
                    } else {
                        "Target is being erased"
                    }
                ),
                "{mode}: original public error order/string"
            );
        }
        let rejected = rejection.is_some();
        eprintln!("erasure lifecycle {mode}: certified={certified} rejected={rejected} {events:?}");
        outcomes.push((mode, rejected));
    }
    for (mode, rejected) in outcomes {
        assert_eq!(rejected, mode != "internal", "{mode}");
    }
}

#[test]
fn ra8_strategic_visible_cell_loss_cancels_only_fully_valid_internal_intents() {
    use bevy_ecs::schedule::IntoScheduleConfigs;
    for mode in [
        "internal",
        "public",
        "already_unready",
        "became_unready",
        "wrong_type",
        "wrong_chronosphere",
        "outside",
        "unit_arguments",
    ] {
        let mut app = app();
        app.world_mut().insert_resource(crate::AiPlayers::default());
        crate::spawn_building(
            app.world_mut(),
            PlayerId(0),
            "nuclear_reactor",
            CellPos::new(2, 2),
        )
        .unwrap();
        crate::spawn_building(
            app.world_mut(),
            PlayerId(0),
            "spy_satellite",
            CellPos::new(14, 2),
        )
        .unwrap();
        crate::spawn_building(
            app.world_mut(),
            PlayerId(1),
            "nuclear_reactor",
            CellPos::new(32, 2),
        )
        .unwrap();
        let ability = if mode == "wrong_chronosphere" {
            "chronosphere"
        } else if mode == "wrong_type" {
            "barracks"
        } else {
            "nuclear_silo"
        };
        let source =
            crate::spawn_building(app.world_mut(), PlayerId(0), ability, CellPos::new(2, 20))
                .unwrap();
        tick(&mut app); // Real announcement initializes the timer before fixture readiness.
        let cooldown = app
            .world()
            .resource::<Definitions>()
            .metadata(ProductionItem::Building(
                app.world()
                    .resource::<Definitions>()
                    .building_kind(ability)
                    .unwrap(),
            ))
            .unwrap()
            .cooldown;
        if mode != "already_unready" {
            app.world_mut()
                .resource_mut::<crate::SpecialState>()
                .charge
                .insert(source, cooldown);
        }
        let target = if mode == "outside" {
            CellPos::new(-1, -1)
        } else {
            CellPos::new(28, 28)
        };
        assert!(
            app.world()
                .resource::<crate::Players>()
                .get(PlayerId(0))
                .unwrap()
                .vision()
                .is_visible(CellPos::new(28, 28))
        );
        let decision_tick = app.world().resource::<SimTick>().0;
        app.add_systems(
            SimSchedule,
            (move |world: &mut bevy_ecs::world::World| {
                if world.resource::<SimTick>().0 != decision_tick {
                    return;
                }
                let command = Command::Superweapon {
                    building: source,
                    target,
                    units: if mode == "unit_arguments" {
                        vec![SimId(999999)]
                    } else {
                        Vec::new()
                    },
                };
                let issued = IssuedCommand {
                    player: PlayerId(0),
                    command,
                };
                if mode == "public" {
                    world.resource_mut::<CommandQueue>().push(issued);
                } else {
                    world
                        .resource_mut::<CommandQueue>()
                        .push_ai(issued, AiIntent::default());
                }
                certify_ai_strategic_intents(world);
                crate::spawn_building(world, PlayerId(1), "gap_generator", CellPos::new(30, 30))
                    .unwrap();
                if mode == "became_unready" {
                    world
                        .resource_mut::<crate::SpecialState>()
                        .charge
                        .insert(source, 0);
                }
            })
            .in_set(crate::SimSet::Ai)
            .after(crate::SimCoreSet(crate::SimSet::Ai)),
        );
        let events = tick(&mut app);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, SimEvent::StrategicActivated { .. }))
        );
        assert!(
            !app.world()
                .resource::<crate::Players>()
                .get(PlayerId(0))
                .unwrap()
                .vision()
                .is_visible(CellPos::new(28, 28)),
            "{mode}: real Gap must remove satellite-only sight"
        );
        let certified = app
            .world()
            .resource::<CommandQueue>()
            .pending
            .iter()
            .find_map(|p| p.ai_intent.as_ref().map(|w| w.strategic_valid))
            .unwrap_or(false);
        assert_eq!(
            certified,
            ["internal", "became_unready"].contains(&mode),
            "{mode}"
        );
        let events = tick(&mut app);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, SimEvent::StrategicActivated { .. })),
            "{mode}"
        );
        assert_eq!(
            events
                .iter()
                .any(|e| matches!(e, SimEvent::CommandRejected { .. })),
            mode != "internal",
            "{mode}: {events:?}"
        );
    }
}

#[test]
fn ra8_strategic_visibility_certificate_is_value_hashed_and_never_certifies_hidden_cells() {
    let mut app = app();
    let source = crate::spawn_building(
        app.world_mut(),
        PlayerId(0),
        "nuclear_silo",
        CellPos::new(2, 20),
    )
    .unwrap();
    issue(
        &mut app,
        Command::Superweapon {
            building: source,
            target: CellPos::new(40, 2),
            units: Vec::new(),
        },
        Some(AiIntent::default()),
    );
    certify_ai_strategic_intents(app.world_mut());
    assert!(
        !app.world().resource::<CommandQueue>().pending[0]
            .ai_intent
            .as_ref()
            .unwrap()
            .strategic_valid
    );
    let before = crate::state_hash(app.world_mut());
    app.world_mut().resource_mut::<CommandQueue>().pending[0]
        .ai_intent
        .as_mut()
        .unwrap()
        .strategic_valid = true;
    assert_ne!(before, crate::state_hash(app.world_mut()));
}
