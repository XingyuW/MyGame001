//! Exact default-path admission/lifecycle failures from the first RA8 game preflight.
use bevy::ecs::message::Messages;
use sim::{
    CommandQueue, Controller, MatchOutcome, MatchState, PlayerId, PlayerSetup, RejectReason,
    SimEvent, SimId, SimPlugin, SimSchedule,
};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn ra8_demo_admission_and_target_lifecycle_regressions_finish_with_original_rejection_rules() {
    let assets = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let definitions = sim::load_ruleset(assets.join("data")).unwrap();
    let skirmish = sim::load_skirmish(assets.join("data/skirmish.ron"), &definitions).unwrap();
    let ai = sim::load_ai(assets.join("data/ai.ron"), &definitions).unwrap();
    let map = skirmish.maps.iter().position(|m| m.id == "demo").unwrap();
    for (countries, difficulties, trace_tick) in [
        (["germany", "great_britain"], ["medium", "easy"], Some(8900)),
        (
            ["germany", "great_britain"],
            ["brutal", "brutal"],
            Some(12861),
        ),
        (["france", "iraq"], ["medium", "easy"], Some(13481)),
        (["france", "cuba"], ["brutal", "brutal"], Some(9620)),
        (["russia", "libya"], ["easy", "easy"], Some(10241)),
        (["cuba", "germany"], ["medium", "medium"], Some(22881)),
        (["cuba", "russia"], ["medium", "easy"], Some(8241)),
        (["russia", "libya"], ["medium", "medium"], Some(11421)),
        (["cuba", "russia"], ["brutal", "medium"], Some(7521)),
        (["russia", "libya"], ["brutal", "brutal"], Some(5451)),
    ] {
        let name = format!("demo seed0 {countries:?} {difficulties:?}");
        let mut app = bevy::app::App::new();
        app.add_plugins(SimPlugin { seed: 0 });
        app.world_mut().insert_resource(
            sim::load_map(assets.join("maps/demo/demo.tmx"))
                .unwrap()
                .grid,
        );
        app.world_mut().insert_resource(definitions.clone());
        let players = [0, 1].map(|i| PlayerSetup {
            player: PlayerId(i as u8),
            start: i,
            faction: skirmish
                .factions
                .iter()
                .position(|f| f.id == countries[i])
                .unwrap(),
            controller: Controller::Ai {
                difficulty: ai
                    .difficulties
                    .iter()
                    .position(|d| d.id == difficulties[i])
                    .unwrap(),
            },
        });
        sim::setup_match(app.world_mut(), &skirmish, &ai, map, &players).unwrap();
        let mut losses: BTreeMap<PlayerId, BTreeSet<SimId>> = BTreeMap::new();
        let mut outcome = None;
        for at in 0_u64..72000 {
            let world = app.world_mut();
            if trace_tick.is_some_and(|t: u64| at.abs_diff(t) <= 1) {
                eprintln!(
                    "{name} before tick{at}: {:?}",
                    world.resource::<CommandQueue>()
                );
            }
            world.run_schedule(SimSchedule);
            let before = std::mem::take(&mut losses);
            let events: Vec<_> = world
                .resource::<Messages<SimEvent>>()
                .iter_current_update_messages()
                .cloned()
                .collect();
            for event in events {
                match event {
                    SimEvent::CommandRejected {
                        player,
                        command,
                        reason,
                    } => {
                        // Identical exemptions to the original m6 production sweep; no new
                        // rejection category is permitted by this focused regression.
                        let lost = before.get(&player);
                        let explained = match reason {
                            RejectReason::UnknownUnit(id) | RejectReason::UnknownBuilding(id) => {
                                lost.is_some_and(|l| l.contains(&id))
                            }
                            RejectReason::Placement(sim::PlacementError::Occupied {
                                unit, ..
                            }) => {
                                lost.is_some_and(|l| !l.is_empty())
                                    || world
                                        .query::<(&SimId, &sim::Owner)>()
                                        .iter(world)
                                        .any(|(id, owner)| *id == unit && owner.0 != player)
                            }
                            RejectReason::MissingPrerequisite { .. }
                            | RejectReason::NoProducer { .. }
                            | RejectReason::Placement(_) => lost.is_some_and(|l| !l.is_empty()),
                            _ => false,
                        };
                        assert!(
                            explained,
                            "{name} tick{at}: {command:?} rejected {reason:?}"
                        );
                    }
                    SimEvent::UnitDestroyed { player, unit } => {
                        losses.entry(player).or_default().insert(unit);
                    }
                    SimEvent::BuildingDestroyed {
                        player, building, ..
                    } => {
                        losses.entry(player).or_default().insert(building);
                    }
                    _ => {}
                }
            }
            assert!(
                world.resource::<CommandQueue>().len() <= 32,
                "{name} tick{at}"
            );
            assert!(
                world
                    .resource::<sim::Players>()
                    .iter()
                    .all(|(_, s)| s.credits() >= 0)
            );
            outcome = world.resource::<MatchState>().outcome();
            if at % 16 == 0 || outcome.is_some() {
                assert!(
                    sim::cell_invariant_violations(world).is_empty(),
                    "{name} tick{at}"
                );
                assert!(sim::stray_claims(world).is_empty(), "{name} tick{at}");
            }
            if outcome.is_some() {
                eprintln!("{name}: {outcome:?}");
                break;
            }
        }
        assert!(
            matches!(outcome, Some(MatchOutcome::Won { .. })),
            "{name}: {outcome:?} after72000"
        );
    }
}
