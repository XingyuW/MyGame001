//! Exact seed-zero frozen RA8 gate failures, with the original R29 limits and rejection rules.
use bevy::ecs::message::Messages;
use sim::{
    CommandQueue, Controller, MatchOutcome, MatchState, PlayerId, PlayerSetup, RejectReason,
    SimEvent, SimId, SimPlugin, SimSchedule,
};
use std::collections::{BTreeMap, BTreeSet};

fn run_case(map_id: &str, countries: [&str; 2], difficulties: [&str; 2]) {
    let assets = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let definitions = sim::load_ruleset(assets.join("data")).unwrap();
    let skirmish = sim::load_skirmish(assets.join("data/skirmish.ron"), &definitions).unwrap();
    let ai = sim::load_ai(assets.join("data/ai.ron"), &definitions).unwrap();
    let map = skirmish.maps.iter().position(|m| m.id == map_id).unwrap();
    let grid = sim::load_map(assets.join(sim::map_asset_path(map_id)))
        .unwrap()
        .grid;
    let limit = 72000 * u64::from(grid.width() + grid.height()).max(128) / 128;
    let name = format!("{map_id} seed0 {countries:?} {difficulties:?}");
    let mut app = bevy::app::App::new();
    app.add_plugins(SimPlugin { seed: 0 });
    app.world_mut().insert_resource(grid);
    app.world_mut().insert_resource(definitions);
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
    for at in 0_u64..limit {
        let world = app.world_mut();
        world.run_schedule(SimSchedule);
        if at % 60000 == 59999 {
            dump(world);
        }
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
                        RejectReason::Placement(sim::PlacementError::Occupied { unit, .. }) => {
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
    if outcome.is_none() {
        dump(app.world_mut());
    }
    assert!(
        matches!(outcome, Some(MatchOutcome::Won { .. })),
        "{name}: {outcome:?} after{limit}"
    );
}

fn dump(world: &mut bevy::ecs::world::World) {
    let definitions = world.resource::<sim::Definitions>().clone();
    eprintln!("snapshot tick {:?}", world.resource::<sim::SimTick>());
    for (player, state) in world.resource::<sim::Players>().iter() {
        eprintln!(
            "player {player:?} credits {} queues {:?}",
            state.credits(),
            state.queues().collect::<Vec<_>>()
        );
    }
    let controllers = format!("{:?}", world.resource::<sim::AiPlayers>());
    let mut rest = controllers.as_str();
    while let Some(start) = rest.find("waypoints: [") {
        eprintln!("{}waypoints: <omitted>", &rest[..start]);
        let Some(end) = rest[start..].find("known:") else {
            break;
        };
        rest = &rest[start + end..];
    }
    eprintln!("{rest}");
    for (id, owner, unit, mobile, health, orders, weapon, harvester) in world
        .query::<(
            &SimId,
            &sim::Owner,
            &sim::Unit,
            &sim::Mobile,
            &sim::Health,
            &sim::Orders,
            Option<&sim::Weapon>,
            Option<&sim::Harvester>,
        )>()
        .iter(world)
    {
        let movement = orders
            .current
            .as_ref()
            .map(|order| (order.destination, order.path.len(), order.state));
        eprintln!(
            "unit {id:?} {owner:?} {} {mobile:?} {health:?} move{movement:?} {weapon:?} {harvester:?}",
            definitions.unit(unit.kind).unwrap().id
        );
    }
    for (id, owner, building, health) in world
        .query::<(&SimId, &sim::Owner, &sim::Building, &sim::Health)>()
        .iter(world)
    {
        if owner.0 != sim::NEUTRAL {
            eprintln!(
                "building {id:?} {owner:?} {} {:?} {health:?}",
                definitions.building(building.kind).unwrap().id,
                building.footprint.origin
            );
        }
    }
    eprintln!("special {:?}", world.resource::<sim::SpecialState>());
}
#[test]
fn ra8_gate_liberty_seed_zero_finishes() {
    run_case("liberty_bay", ["iraq", "korea"], ["medium", "medium"]);
}

#[test]
fn ra8_gate_red_canyon_seed_zero_finishes() {
    run_case("red_canyon", ["france", "russia"], ["medium", "easy"]);
}

#[test]
fn ra8_gate_steel_harbor_seed_zero_finishes() {
    run_case("steel_harbor", ["cuba", "america"], ["brutal", "brutal"]);
}

#[test]
fn ra8_gate_continental_stall_seed_zero_finishes() {
    run_case("continental_divide", ["france", "cuba"], ["easy", "medium"]);
}

#[test]
fn ra8_gate_ember_seed_zero_finishes() {
    run_case("ember_caldera", ["libya", "korea"], ["brutal", "medium"]);
}

#[test]
fn ra8_gate_wheel_seed_zero_finishes() {
    run_case("wheel_of_worlds", ["cuba", "france"], ["brutal", "brutal"]);
}

#[test]
fn ra8_gate_continental_bridge_seed_zero_finishes() {
    run_case(
        "continental_divide",
        ["iraq", "great_britain"],
        ["brutal", "brutal"],
    );
}

#[test]
fn ra8_gate_glacier_bridge_seed_zero_finishes() {
    run_case("glacier_pass", ["korea", "russia"], ["brutal", "brutal"]);
}

#[test]
fn ra8_gate_harvest_bridge_seed_zero_finishes() {
    run_case("harvest_plains", ["america", "korea"], ["brutal", "brutal"]);
}
