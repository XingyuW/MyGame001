//! Exact seed-zero frozen RA8 gate failures, with the original R29 limits and rejection rules.
use bevy::ecs::message::Messages;
use bevy::ecs::schedule::IntoScheduleConfigs;
use sim::{
    CommandQueue, Controller, MatchOutcome, MatchState, PlayerId, PlayerSetup, RejectReason,
    SimEvent, SimId, SimPlugin, SimSchedule,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(bevy::prelude::Resource, Default)]
struct EconomyTrace {
    before: BTreeMap<SimId, i32>,
    counters: BTreeMap<String, u64>,
    buildings: BTreeMap<SimId, String>,
    units: BTreeMap<SimId, String>,
    last_air_hold: BTreeMap<PlayerId, (Option<SimId>, u64)>,
}

fn trace_before_economy(
    mut trace: bevy::prelude::ResMut<EconomyTrace>,
    buildings: bevy::prelude::Query<(&SimId, &sim::Building, &sim::Health)>,
) {
    trace.before = buildings
        .iter()
        .map(|(id, _, hp)| (*id, hp.hit_points))
        .collect();
}

fn trace_after_economy(
    mut trace: bevy::prelude::ResMut<EconomyTrace>,
    definitions: bevy::prelude::Res<sim::Definitions>,
    buildings: bevy::prelude::Query<(&SimId, &sim::Owner, &sim::Building, &sim::Health)>,
) {
    for (id, owner, building, hp) in &buildings {
        let before = trace.before.get(id).copied().unwrap_or(hp.hit_points);
        if hp.hit_points > before {
            let key = format!(
                "P{} repaired_hp {}",
                owner.0.0,
                definitions.building(building.kind).unwrap().id
            );
            *trace.counters.entry(key).or_default() += (hp.hit_points - before) as u64;
        }
    }
}

fn trace_economy_events(world: &mut bevy::ecs::world::World, events: &[SimEvent], at: u64) {
    let Some(mut trace) = world.remove_resource::<EconomyTrace>() else {
        return;
    };
    let definitions = world.resource::<sim::Definitions>().clone();
    if std::env::var_os("RA8_DEMO_LATE_TRACE").is_some()
        && ((59980..=60020).contains(&at) || (71980..=72000).contains(&at))
    {
        eprintln!(
            "demo late tick{at} queue{:?}",
            world.resource::<CommandQueue>()
        );
        for event in events {
            if matches!(event, SimEvent::MoveFailed { .. }) {
                eprintln!("demo late tick{at} {event:?}");
            }
        }
    }
    if std::env::var_os("RA8_LATE_MISSION_TRACE").is_some()
        && ((275980..=276020).contains(&at) || (287980..=288000).contains(&at))
    {
        eprintln!(
            "mission tick{at} queue{:?}",
            world.resource::<CommandQueue>()
        );
        for event in events {
            if matches!(
                event,
                SimEvent::MoveFailed { .. }
                    | SimEvent::AttackEnded { .. }
                    | SimEvent::WeaponFired { .. }
            ) {
                eprintln!("mission tick{at} {event:?}");
            }
        }
    }
    let trace_from = std::env::var("RA8_TRACE_FROM")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(180000);
    let service = std::env::var_os("RA8_SERVICE_TRACE").is_some() && at >= trace_from;
    for (id, owner, unit, mobile) in world
        .query::<(&SimId, &sim::Owner, &sim::Unit, &sim::Mobile)>()
        .iter(world)
    {
        if service && !trace.units.contains_key(id) {
            eprintln!(
                "service birth tick{at} {id:?} {owner:?} {} cell{:?}",
                definitions.unit(unit.kind).unwrap().id,
                mobile.cell
            );
        }
        trace
            .units
            .insert(*id, definitions.unit(unit.kind).unwrap().id.clone());
    }
    for (id, building) in world.query::<(&SimId, &sim::Building)>().iter(world) {
        trace
            .buildings
            .insert(*id, definitions.building(building.kind).unwrap().id.clone());
    }
    let coordination = std::env::var_os("RA8_AIR_COORD_TRACE").is_some();
    if coordination {
        for event in events {
            let relevant = match event {
                SimEvent::BuildingPlaced { kind, .. }
                | SimEvent::BuildingDestroyed { kind, .. } => {
                    definitions.building_combat(*kind).is_some_and(|terms| {
                        terms
                            .weapons(false)
                            .into_iter()
                            .flatten()
                            .filter_map(|k| definitions.weapon(k))
                            .any(|w| w.anti_air && !w.anti_ground)
                    })
                }
                SimEvent::WeaponFired {
                    attacker, target, ..
                } => {
                    trace
                        .units
                        .get(attacker)
                        .and_then(|id| definitions.unit_kind(id))
                        .and_then(|k| definitions.metadata(sim::ProductionItem::Unit(k)))
                        .is_some_and(|m| m.airport_bound)
                        && trace.buildings.contains_key(target)
                }
                _ => false,
            };
            if relevant {
                eprintln!("coord tick{at} {event:?}");
            }
        }
    }
    for event in events {
        let (key, amount) = match event {
            SimEvent::OreDelivered {
                player, credits, ..
            } => (format!("P{} ore_credits", player.0), *credits as u64),
            SimEvent::ProductionComplete { player, item } => {
                (format!("P{} produced {item:?}", player.0), 1)
            }
            SimEvent::BuildingPlaced { player, kind, .. } => (
                format!(
                    "P{} placed {}",
                    player.0,
                    definitions.building(*kind).unwrap().id
                ),
                1,
            ),
            SimEvent::ProductionCancelled {
                player,
                item,
                refund,
            } => (
                format!("P{} cancelled {item:?} refund{refund}", player.0),
                1,
            ),
            SimEvent::StrategicActivated { player, role, .. } => {
                (format!("P{} strategic {role:?}", player.0), 1)
            }
            SimEvent::BuildingSold { player, refund, .. } => {
                (format!("P{} sales_refund", player.0), *refund as u64)
            }
            SimEvent::UnitDestroyed { player, unit } => (
                format!(
                    "P{} lost {}",
                    player.0,
                    trace.units.get(unit).map_or("?", String::as_str)
                ),
                1,
            ),
            SimEvent::BuildingDestroyed {
                player, building, ..
            } => (
                format!(
                    "P{} lost_building {}",
                    player.0,
                    trace.buildings.get(building).map_or("?", String::as_str)
                ),
                1,
            ),
            SimEvent::WeaponFired {
                attacker, target, ..
            } => {
                let attacker = trace
                    .units
                    .get(attacker)
                    .or_else(|| trace.buildings.get(attacker))
                    .map_or("?", String::as_str);
                let target = trace
                    .units
                    .get(target)
                    .or_else(|| trace.buildings.get(target))
                    .map_or("?", String::as_str);
                (format!("shots {attacker}->{target}"), 1)
            }
            SimEvent::AttackEnded {
                unit,
                target,
                reason,
            } => (format!("ended {unit:?}->{target:?} {reason:?}"), 1),
            _ => continue,
        };
        *trace.counters.entry(key).or_default() += amount;
    }
    let pending: Vec<_> = world.resource::<CommandQueue>().iter().cloned().collect();
    if service {
        for event in events {
            if matches!(
                event,
                SimEvent::UnitDestroyed { .. }
                    | SimEvent::ProductionComplete { .. }
                    | SimEvent::BuildingSold { .. }
                    | SimEvent::UnitSpawned { .. }
                    | SimEvent::CrewEscaped { .. }
                    | SimEvent::StrategicActivated { .. }
                    | SimEvent::Impact { .. }
                    | SimEvent::DamageStateChanged { .. }
            ) {
                eprintln!("service tick{at} {event:?}");
            }
            if let SimEvent::WeaponFired {
                attacker, target, ..
            } = event
            {
                let target_kind = trace
                    .units
                    .get(target)
                    .or_else(|| trace.buildings.get(target))
                    .map(String::as_str);
                let attacker_kind = trace
                    .units
                    .get(attacker)
                    .or_else(|| trace.buildings.get(attacker))
                    .map(String::as_str);
                eprintln!(
                    "service shot tick{at} {attacker:?} {attacker_kind:?} -> {target:?} {target_kind:?}"
                );
            }
        }
        if at.is_multiple_of(3000) {
            for (id, owner, unit, mobile, orders, weapon) in world
                .query::<(
                    &SimId,
                    &sim::Owner,
                    &sim::Unit,
                    &sim::Mobile,
                    &sim::Orders,
                    Option<&sim::Weapon>,
                )>()
                .iter(world)
            {
                if weapon.is_some() {
                    eprintln!(
                        "service tick{at} P{} {id:?} {} cell{:?} move{:?} weapon{weapon:?} dock{:?} wave{}",
                        owner.0.0,
                        definitions.unit(unit.kind).unwrap().id,
                        mobile.cell,
                        orders
                            .current
                            .as_ref()
                            .map(|m| (m.destination, m.best, &m.state)),
                        world.resource::<sim::SpecialState>().docked.get(id),
                        world
                            .resource::<sim::AiPlayers>()
                            .controller(owner.0)
                            .is_some_and(|c| c.wave().contains(id))
                    );
                }
            }
        }
    }
    for issued in &pending {
        if service {
            eprintln!("service decision tick{at} {issued:?}");
        }
        if coordination && let sim::Command::Move { units, target } = &issued.command {
            let controller = world.resource::<sim::AiPlayers>().controller(issued.player);
            if controller.is_some_and(|c| c.gather() == *target) {
                let job = controller.and_then(|c| match c.target() {
                    Some(sim::WaveTarget::Building(id)) => Some(id),
                    _ => None,
                });
                let previous = trace.last_air_hold.get(&issued.player);
                if previous
                    .is_none_or(|(last, when)| *last != job || at.saturating_sub(*when) >= 6000)
                {
                    let mut aircraft = Vec::new();
                    for (id, unit, mobile) in world
                        .query::<(&SimId, &sim::Unit, &sim::Mobile)>()
                        .iter(world)
                    {
                        if units.contains(id)
                            && let Some(meta) =
                                definitions.metadata(sim::ProductionItem::Unit(unit.kind))
                            && meta.airport_bound
                        {
                            aircraft.push((
                                *id,
                                mobile.cell,
                                world
                                    .resource::<sim::SpecialState>()
                                    .ammo_of(*id, meta.ammo),
                            ));
                        }
                    }
                    if !aircraft.is_empty() {
                        eprintln!(
                            "coord tick{at} hold P{} job{job:?} target{target:?} aircraft{aircraft:?}",
                            issued.player.0
                        );
                        trace.last_air_hold.insert(issued.player, (job, at));
                    }
                }
            }
        }
        if let sim::Command::Attack { units, target } = &issued.command {
            let target_kind = trace.buildings.get(target).map_or("?", String::as_str);
            let key = format!(
                "P{} queued_attack {target:?} {target_kind}",
                issued.player.0
            );
            *trace.counters.entry(key).or_default() += 1;
            let key = format!("P{} queued_attack_actors {target:?}", issued.player.0);
            *trace.counters.entry(key).or_default() += units.len() as u64;
        }
    }
    if at % 60000 == 59999 {
        eprintln!("economy window {}..{} {:?}", at - 59999, at, trace.counters);
        trace.counters.clear();
    }
    world.insert_resource(trace);
}

fn run_case(map_id: &str, countries: [&str; 2], difficulties: [&str; 2]) {
    run_case_to(map_id, countries, difficulties, None);
}

fn run_case_to(
    map_id: &str,
    countries: [&str; 2],
    difficulties: [&str; 2],
    stricter_limit: Option<u64>,
) {
    let assets = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let definitions = sim::load_ruleset(assets.join("data")).unwrap();
    let skirmish = sim::load_skirmish(assets.join("data/skirmish.ron"), &definitions).unwrap();
    let ai = sim::load_ai(assets.join("data/ai.ron"), &definitions).unwrap();
    let map = skirmish.maps.iter().position(|m| m.id == map_id).unwrap();
    let grid = sim::load_map(assets.join(sim::map_asset_path(map_id)))
        .unwrap()
        .grid;
    let limit = 72000 * u64::from(grid.width() + grid.height()).max(128) / 128;
    let limit = stricter_limit.map_or(limit, |stricter| stricter.min(limit));
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
    if (map_id == "thousand_lakes" && countries == ["france", "germany"])
        || std::env::var_os("RA8_ECONOMY_TRACE").is_some()
    {
        app.init_resource::<EconomyTrace>();
        app.add_systems(
            SimSchedule,
            trace_before_economy
                .in_set(sim::SimSet::Economy)
                .before(sim::SimCoreSet(sim::SimSet::Economy)),
        );
        app.add_systems(
            SimSchedule,
            trace_after_economy
                .in_set(sim::SimSet::Economy)
                .after(sim::SimCoreSet(sim::SimSet::Economy)),
        );
    }
    if std::env::var_os("RA8_GATE7_SERVICE_TRACE").is_some()
        || (map_id == "steel_harbor" && countries == ["america", "cuba"])
        || (map_id == "thousand_lakes" && countries == ["great_britain", "america"])
    {
        app.add_systems(
            SimSchedule,
            (|world: &mut bevy::ecs::world::World| trace_lifecycle(world, "decision"))
                .in_set(sim::SimSet::Ai)
                .after(sim::SimCoreSet(sim::SimSet::Ai)),
        );
        app.add_systems(
            SimSchedule,
            (|world: &mut bevy::ecs::world::World| trace_lifecycle(world, "application"))
                .in_set(sim::SimSet::Commands)
                .before(sim::SimCoreSet(sim::SimSet::Commands)),
        );
    }
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
        trace_economy_events(world, &events, at);
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
                    if !explained {
                        dump(world);
                    }
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
    if let Some(trace) = app.world().get_resource::<EconomyTrace>() {
        eprintln!("economy final partial window {:?}", trace.counters);
    }
    assert!(
        matches!(outcome, Some(MatchOutcome::Won { .. })),
        "{name}: {outcome:?} after{limit}"
    );
}

fn trace_lifecycle(world: &mut bevy::ecs::world::World, stage: &str) {
    let tick = world.resource::<sim::SimTick>().0;
    if std::env::var_os("RA8_GATE7_SERVICE_TRACE").is_some() && (21275..=21285).contains(&tick) {
        eprintln!("service lifecycle {stage} tick{tick}");
        dump(world);
    }
    if !(28239..=28242).contains(&tick) && !(28920..=28923).contains(&tick) {
        return;
    }
    eprintln!(
        "lifecycle {stage} tick{tick} queued {:?}",
        world.resource::<CommandQueue>()
    );
    let definitions = world.resource::<sim::Definitions>().clone();
    for (id, owner, building) in world
        .query::<(&SimId, &sim::Owner, &sim::Building)>()
        .iter(world)
    {
        if id.0 == 62 {
            let player = world.resource::<sim::Players>().get(owner.0).unwrap();
            eprintln!(
                "strategic62 {owner:?} {} target23,23 visible{} fog{} lowpower{}",
                definitions.building(building.kind).unwrap().id,
                player.vision().is_visible(sim::CellPos::new(23, 23)),
                player.vision().uses_fog(),
                player.low_power()
            );
        }
    }
    let mut ships = world.query::<(&SimId, &sim::Owner, &sim::Unit, &sim::Mobile, &sim::Orders)>();
    let special = world.resource::<sim::SpecialState>();
    eprintln!("strategic storm {:?}", special.strategic.storm);
    let bridges = world.resource::<sim::Bridges>();
    let hut = SimId(2684378722);
    if let Some(bridge) = bridges
        .huts
        .get(&hut)
        .and_then(|id| bridges.bridges.get(id))
    {
        eprintln!(
            "bridge{hut:?} cells{:?} collapsed{}",
            bridge.cells, bridge.collapsed
        );
        for (id, owner, unit, mobile, orders) in ships.iter(world) {
            if definitions.domain(unit.kind) == sim::Domain::Naval
                && bridge
                    .cells
                    .iter()
                    .any(|c| c.x.abs_diff(mobile.cell.x) <= 2 && c.y.abs_diff(mobile.cell.y) <= 2)
            {
                let seen = [0, 1].map(|p| {
                    world
                        .resource::<sim::Players>()
                        .get(PlayerId(p))
                        .is_some_and(|s| s.vision().sees_unit(mobile))
                        && !special.hidden.contains(&(PlayerId(p), *id))
                });
                eprintln!(
                    "ship{id:?} {owner:?} {} domain{:?} {mobile:?} orders{orders:?} seen{seen:?}",
                    definitions.unit(unit.kind).unwrap().id,
                    definitions.domain(unit.kind)
                );
            }
        }
    }
}

fn dump(world: &mut bevy::ecs::world::World) {
    let definitions = world.resource::<sim::Definitions>().clone();
    eprintln!("snapshot tick {:?}", world.resource::<sim::SimTick>());
    eprintln!("queued {:?}", world.resource::<CommandQueue>());
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
fn ra8_gate_demo_germany_great_britain_brutal_seed_zero_finishes() {
    run_case_to(
        "demo",
        ["germany", "great_britain"],
        ["brutal", "brutal"],
        Some(72000),
    );
}

#[test]
fn ra8_gate_demo_france_cuba_brutal_seed_zero_finishes() {
    run_case("demo", ["france", "cuba"], ["brutal", "brutal"]);
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

#[test]
fn ra8_gate_steel_harbor_america_cuba_bridge_seed_zero_finishes() {
    run_case("steel_harbor", ["america", "cuba"], ["brutal", "brutal"]);
}

#[test]
fn ra8_gate_thousand_lakes_great_britain_america_seed_zero_finishes() {
    run_case(
        "thousand_lakes",
        ["great_britain", "america"],
        ["easy", "easy"],
    );
}

#[test]
fn ra8_gate_thousand_lakes_france_germany_seed_zero_finishes() {
    run_case(
        "thousand_lakes",
        ["france", "germany"],
        ["brutal", "brutal"],
    );
}

#[test]
fn ra8_gate_liberty_iraq_america_brutal_seed_zero_finishes() {
    run_case("liberty_bay", ["iraq", "america"], ["brutal", "brutal"]);
}

#[test]
fn ra8_gate_oasis_germany_great_britain_brutal_seed_zero_finishes() {
    run_case(
        "oasis_gulf",
        ["germany", "great_britain"],
        ["brutal", "brutal"],
    );
}

#[test]
fn ra8_gate_steel_france_germany_medium_seed_zero_finishes() {
    run_case("steel_harbor", ["france", "germany"], ["medium", "medium"]);
}

#[test]
fn ra8_gate_frozen_cuba_germany_brutal_seed_zero_finishes() {
    run_case("frozen_sound", ["cuba", "germany"], ["brutal", "brutal"]);
}

#[test]
fn ra8_gate_black_water_libya_america_brutal_seed_zero_finishes() {
    run_case("black_water", ["libya", "america"], ["brutal", "brutal"]);
}

#[test]
fn ra8_gate_black_water_korea_france_brutal_seed_zero_finishes() {
    run_case("black_water", ["korea", "france"], ["brutal", "brutal"]);
}

#[test]
fn ra8_gate_wheel_germany_great_britain_brutal_seed_zero_finishes() {
    run_case(
        "wheel_of_worlds",
        ["germany", "great_britain"],
        ["brutal", "brutal"],
    );
}

#[test]
fn ra8_gate7_riverlands_iraq_cuba_easy_seed_zero_finishes() {
    run_case("riverlands", ["iraq", "cuba"], ["easy", "easy"]);
}

#[test]
fn ra8_gate7_jungle_cuba_russia_easy_seed_zero_finishes() {
    run_case("jungle_delta", ["cuba", "russia"], ["easy", "easy"]);
}

#[test]
fn ra8_gate7_pacific_cuba_great_britain_brutal_seed_zero_finishes() {
    run_case(
        "pacific_atoll",
        ["cuba", "great_britain"],
        ["brutal", "brutal"],
    );
}

#[test]
fn ra8_gate7_lakes_russia_medium_libya_brutal_seed_zero_finishes() {
    run_case("thousand_lakes", ["russia", "libya"], ["medium", "brutal"]);
}
