//! RA4 on the shipped data (`assets/data`), through `CommandQueue` and `SimSchedule`: the
//! veterancy of a Grizzly, the crews of the base units and the survivors of buildings, the four
//! docks of an Airforce Command and the Harrier's one volley, the V3 rocket and the
//! Dreadnought's missiles, the carrier's Hornets, the destroyer's Osprey, and the submerged
//! Typhoon (docs/RA2_PARITY_PLAN.md section 5, RA4; docs/DECISIONS.md R46 to R52). The numbers
//! in the comments are those of the reference (`tests/reference/ra2_reference.tsv`), converted
//! as the plan's section 2 says.

use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    Ability, Cell, CellPos, Command, CommandQueue, Definitions, ELITE, Experience, Grid, Health,
    IssuedCommand, Mobile, Multipliers, PlayerId, Players, ProductionItem, RejectReason, SimEvent,
    SimId, SimPlugin, SimSchedule, Slot, SpecialState, SubPos, VETERAN, load_ruleset,
    spawn_building, spawn_unit,
};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

fn shipped() -> Definitions {
    load_ruleset(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data"))
        .unwrap_or_else(|error| panic!("{error}"))
}

/// A world of 48 x 48 cells with the shipped data and the players P0 (Great Britain, Allied)
/// and P1 (Russia, Soviet), who are enemies; the cells from column `water_from` on are water.
fn world_seeded(water_from: i32, seed: u64) -> App {
    let definitions = shipped();
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed });
    let cells = (0..48 * 48)
        .map(|index| {
            let water = index % 48 >= water_from;
            Cell {
                passable: !water,
                water,
                buildable: !water,
                ore: 0,
            }
        })
        .collect();
    app.world_mut()
        .insert_resource(Grid::from_cells(48, 48, cells).expect("a grid"));
    for (player, country) in [(P0, "great_britain"), (P1, "russia")] {
        let id = definitions
            .rules()
            .and_then(|rules| rules.country_id(country))
            .unwrap_or_else(|| panic!("country {country}"));
        let mut players = app.world_mut().resource_mut::<Players>();
        players.add(player, 10_000).expect("a new player");
        assert!(players.set_country(player, Some(id)));
    }
    app.world_mut().insert_resource(definitions);
    app
}

fn world(water_from: i32) -> App {
    world_seeded(water_from, 11)
}

fn defs(app: &App) -> Definitions {
    app.world().resource::<Definitions>().clone()
}

/// A unit or building of type `id` of `owner` at (`x`, `y`), as scenario setup.
fn put(app: &mut App, owner: PlayerId, id: &str, x: i32, y: i32) -> SimId {
    if defs(app).unit_kind(id).is_some() {
        spawn_unit(app.world_mut(), owner, id, CellPos::new(x, y))
            .unwrap_or_else(|error| panic!("{id}: {error}"))
    } else {
        spawn_building(app.world_mut(), owner, id, CellPos::new(x, y))
            .unwrap_or_else(|error| panic!("{id}: {error}"))
    }
}

fn tick(app: &mut App, commands: Vec<IssuedCommand>) -> Vec<SimEvent> {
    {
        let mut queue = app.world_mut().resource_mut::<CommandQueue>();
        for command in commands {
            queue.push(command);
        }
    }
    app.world_mut().run_schedule(SimSchedule);
    app.world()
        .resource::<Messages<SimEvent>>()
        .iter_current_update_messages()
        .cloned()
        .collect()
}

fn ticks(app: &mut App, count: u32) -> Vec<SimEvent> {
    (0..count).flat_map(|_| tick(app, Vec::new())).collect()
}

/// Runs ticks until `done` holds, at most `limit`; the events, and the ticks run.
fn run_until(
    app: &mut App,
    limit: u32,
    mut done: impl FnMut(&mut App, &[SimEvent]) -> bool,
) -> (Vec<SimEvent>, u32) {
    let mut all = Vec::new();
    for ran in 1..=limit {
        let events = tick(app, Vec::new());
        let stop = done(app, &events);
        all.extend(events);
        if stop {
            return (all, ran);
        }
    }
    panic!("the condition did not hold within {limit} ticks");
}

fn hp(app: &mut App, id: SimId) -> Option<i32> {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &Health)>();
    query
        .iter(world)
        .find(|(other, _)| **other == id)
        .map(|(_, health)| health.hit_points)
}

fn set_hp(app: &mut App, id: SimId, hit_points: i32) {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &mut Health)>();
    for (other, mut health) in query.iter_mut(world) {
        if *other == id {
            health.hit_points = hit_points;
        }
    }
}

fn mobile(app: &mut App, id: SimId) -> Mobile {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &Mobile)>();
    *query
        .iter(world)
        .find(|(other, _)| **other == id)
        .map(|(_, mobile)| mobile)
        .unwrap_or_else(|| panic!("{id:?} is not a unit"))
}

fn experience(app: &mut App, id: SimId) -> Option<Experience> {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &Experience)>();
    query
        .iter(world)
        .find(|(other, _)| **other == id)
        .map(|(_, experience)| *experience)
}

/// The type id of the unit `id`.
fn kind_of(app: &mut App, id: SimId) -> String {
    let definitions = defs(app);
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &sim::Unit)>();
    let kind = query
        .iter(world)
        .find(|(other, _)| **other == id)
        .map(|(_, unit)| unit.kind)
        .unwrap_or_else(|| panic!("{id:?} is no unit"));
    definitions.unit(kind).expect("a type").id.clone()
}

/// The units of `owner` of the type `kind`, in id order.
fn units_of(app: &mut App, owner: PlayerId, kind: &str) -> Vec<SimId> {
    let wanted = defs(app)
        .unit_kind(kind)
        .unwrap_or_else(|| panic!("{kind}"));
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &sim::Owner, &sim::Unit)>();
    let mut found: Vec<SimId> = query
        .iter(world)
        .filter(|(_, by, unit)| by.0 == owner && unit.kind == wanted)
        .map(|(id, ..)| *id)
        .collect();
    found.sort_unstable();
    found
}

/// Keeps `id` from firing and moving (scenario setup: a target that only takes hits).
fn silence(app: &mut App, id: SimId) {
    app.world_mut()
        .resource_mut::<SpecialState>()
        .disabled
        .insert(id, u64::MAX);
}

fn attack(player: PlayerId, unit: SimId, target: SimId) -> IssuedCommand {
    IssuedCommand {
        player,
        command: Command::Attack {
            units: vec![unit],
            target,
        },
    }
}

fn state(app: &App) -> &SpecialState {
    app.world().resource::<SpecialState>()
}

fn hidden(app: &App, player: PlayerId, unit: SimId) -> bool {
    state(app).hidden.contains(&(player, unit))
}

fn rejections(events: &[SimEvent]) -> Vec<RejectReason> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::CommandRejected { reason, .. } => Some(reason.clone()),
            _ => None,
        })
        .collect()
}

fn launched(events: &[SimEvent]) -> Vec<SimId> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::UnitLaunched { unit, .. } => Some(*unit),
            _ => None,
        })
        .collect()
}

fn detonated(events: &[SimEvent]) -> Vec<(SimId, SubPos)> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::MissileDetonated { unit, at, .. } => Some((*unit, *at)),
            _ => None,
        })
        .collect()
}

fn destroyed(events: &[SimEvent]) -> Vec<SimId> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::UnitDestroyed { unit, .. } => Some(*unit),
            _ => None,
        })
        .collect()
}

fn shots_of(events: &[SimEvent], attacker: SimId) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::WeaponFired {
                attacker: by,
                weapon,
                ..
            } if *by == attacker => Some(weapon.0),
            _ => None,
        })
        .map(|kind| kind.to_string())
        .collect()
}

fn weapon_id(app: &mut App, id: SimId) -> String {
    let definitions = defs(app);
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &sim::Weapon)>();
    let kind = query
        .iter(world)
        .find(|(other, _)| **other == id)
        .map(|(_, weapon)| weapon.kind)
        .unwrap_or_else(|| panic!("{id:?} has no weapon"));
    definitions.weapon(kind).expect("weapon").id.clone()
}

/// Sets the hit points of `victim` to 1, orders `attacker` to attack it, and runs until it is
/// gone; the events.
fn kill(app: &mut App, attacker: SimId, victim: SimId) -> Vec<SimEvent> {
    set_hp(app, victim, 1);
    let mut events = tick(app, vec![attack(P0, attacker, victim)]);
    let (more, _) = run_until(app, 400, |app, _| hp(app, victim).is_none());
    events.extend(more);
    events
}

// --- Veterancy ----------------------------------------------------------------------------------

#[test]
fn a_grizzly_is_a_veteran_after_three_times_its_cost_and_elite_after_six() {
    // [MTNK] Cost=700, [General] VeteranRatio=3.0: 2100 and 4200. [ENGINEER] Cost=500,
    // [E2] Cost=100.
    let mut app = world(48);
    let grizzly = put(&mut app, P0, "grizzly", 10, 10);
    let mut victims = Vec::new();
    for round in 0..2 {
        for n in 0..4 {
            victims.push((put(&mut app, P1, "engineer", 13 + round, 8 + n), 500));
        }
        let conscript = put(&mut app, P1, "conscript", 13 + round, 13);
        silence(&mut app, conscript);
        victims.push((conscript, 100));
    }
    tick(&mut app, Vec::new());
    let mut total = 0;
    let mut gained = Vec::new();
    for (victim, cost) in victims {
        let events = kill(&mut app, grizzly, victim);
        total += cost;
        gained.extend(events.iter().filter_map(|event| match event {
            SimEvent::RankGained { unit, rank, .. } if *unit == grizzly => Some((total, *rank)),
            _ => None,
        }));
        let found = experience(&mut app, grizzly).expect("experience");
        assert_eq!(found.value, total);
        assert_eq!(
            found.rank,
            match total {
                ..2100 => 0,
                2100..4200 => VETERAN,
                _ => ELITE,
            },
            "at {total} credits"
        );
    }
    // Promoted exactly at the thresholds, which are inclusive.
    assert_eq!(gained, [(2100, VETERAN), (4200, ELITE)]);
    // [MTNK] VeteranAbilities=STRONGER,FIREPOWER,SIGHT,FASTER;
    // EliteAbilities=SELF_HEAL,STRONGER,FIREPOWER,ROF; [General] VeteranCombat=1.1,
    // VeteranArmor=1.5, VeteranSpeed=1.2, VeteranROF=0.6.
    let found = experience(&mut app, grizzly).expect("experience");
    assert_eq!(
        found.ranked,
        Multipliers {
            firepower_pm: 1100,
            armour_pm: 1500,
            reload_pm: 600,
            speed_pm: 1200,
        }
    );
    assert!(found.self_heal);
    // [MTNK] ElitePrimary=105mmE.
    assert_eq!(weapon_id(&mut app, grizzly), "gun_105mm_e");
    // Its speed: [MTNK] Speed=7 is 13 subcells a tick, x 1.2 = 15.6.
    assert_eq!(mobile(&mut app, grizzly).speed, 15);
}

#[test]
fn a_veteran_grizzly_hits_harder_and_is_hit_less() {
    // [105mm] Damage=65 through [AP]: heavy 100 %.
    let shot = |attacker_rank: u8, target_rank: u8| {
        let mut app = world(48);
        let attacker = put(&mut app, P0, "grizzly", 10, 10);
        let target = put(&mut app, P1, "grizzly", 13, 10);
        silence(&mut app, target);
        tick(&mut app, Vec::new());
        if attacker_rank > 0 {
            assert!(sim::promote(app.world_mut(), attacker, attacker_rank));
        }
        if target_rank > 0 {
            assert!(sim::promote(app.world_mut(), target, target_rank));
        }
        tick(&mut app, vec![attack(P0, attacker, target)]);
        run_until(&mut app, 100, |app, _| hp(app, target) != Some(300));
        300 - hp(&mut app, target).expect("the target")
    };
    assert_eq!(shot(0, 0), 65);
    assert_eq!(shot(VETERAN, 0), 71, "65 x 1.1 = 71.5");
    assert_eq!(shot(0, VETERAN), 43, "65 / 1.5 = 43.3");
    assert_eq!(shot(VETERAN, VETERAN), 47, "71 / 1.5 = 47.3");
}

#[test]
fn the_shipped_abilities_are_those_of_the_reference() {
    let definitions = shipped();
    let entry = |id: &str| {
        let kind = definitions.unit_kind(id).unwrap_or_else(|| panic!("{id}"));
        definitions
            .metadata(ProductionItem::Unit(kind))
            .unwrap_or_else(|| panic!("{id}"))
            .clone()
    };
    use Ability::*;
    // Infantry and ships have ROF as veterans; most vehicles and aircraft do not.
    assert_eq!(
        entry("gi").veteran_abilities,
        [Stronger, Firepower, Rof, Sight, Faster]
    );
    assert_eq!(
        entry("grizzly").veteran_abilities,
        [Stronger, Firepower, Sight, Faster]
    );
    assert_eq!(
        entry("grizzly").elite_abilities,
        [SelfHeal, Stronger, Firepower, Rof]
    );
    // The Harrier never heals itself; the Hornet has two abilities at both ranks.
    assert_eq!(entry("harrier").elite_abilities, [Stronger, Firepower, Rof]);
    assert_eq!(entry("hornet").veteran_abilities, [Stronger, Firepower]);
    assert!(entry("tanya").veteran_abilities.contains(&Scatter));
    // [ENGINEER], the dogs, the base units, the miners' Chrono Miner, the spawned missiles:
    // Trainable=no.
    for id in [
        "engineer",
        "attack_dog",
        "spy",
        "crazy_ivan",
        "terrorist",
        "terror_drone",
        "demolition_truck",
        "chrono_miner",
        "mcv",
        "soviet_mcv",
        "amphibious_transport",
        "soviet_amphibious_transport",
        "v3_rocket",
        "dred_missile",
    ] {
        assert!(!entry(id).trainable, "{id}");
        assert!(!definitions.trainable(definitions.unit_kind(id).expect(id)));
    }
    for id in ["gi", "grizzly", "harrier", "war_miner", "typhoon", "hornet"] {
        assert!(entry(id).trainable, "{id}");
    }
    // SelfHealing=yes: the Desolator, the Apocalypse, the squid, both miners, the Kirov.
    let healing: Vec<String> = definitions
        .units()
        .iter()
        .filter(|unit| entry(&unit.id).self_healing)
        .map(|unit| unit.id.clone())
        .collect();
    assert_eq!(
        healing,
        [
            "desolator",
            "chrono_miner",
            "war_miner",
            "apocalypse",
            "kirov",
            "giant_squid"
        ]
    );
    // The four launchers: [V3Launcher], [DredLauncher], [HornetLauncher] and [ASWLauncher]
    // have Spawner=yes.
    let mut launchers: Vec<String> = definitions
        .launcher_weapons()
        .into_iter()
        .map(|kind| definitions.weapon(kind).expect("weapon").id.clone())
        .collect();
    launchers.sort();
    assert_eq!(
        launchers,
        [
            "asw_launcher",
            "dred_launcher",
            "hornet_launcher",
            "v3_launcher"
        ]
    );
    // No building is trainable.
    for (item, entry) in &definitions.arsenal().0 {
        if matches!(item, ProductionItem::Building(_)) {
            assert!(!entry.trainable, "{}", entry.label);
        }
    }
}

#[test]
fn an_apocalypse_heals_itself_at_the_repair_rate() {
    // [APOC] SelfHealing=yes; [General] RepairRate=.016 minutes is 19 ticks.
    let mut app = world(48);
    let apocalypse = put(&mut app, P0, "apocalypse", 10, 10);
    let grizzly = put(&mut app, P0, "grizzly", 14, 10);
    tick(&mut app, Vec::new());
    let full = hp(&mut app, apocalypse).expect("hit points");
    set_hp(&mut app, apocalypse, full - 10);
    set_hp(&mut app, grizzly, 290);
    ticks(&mut app, 19 * 10 + 19);
    assert_eq!(
        hp(&mut app, apocalypse),
        Some(full),
        "ten points in 190 ticks"
    );
    assert_eq!(hp(&mut app, grizzly), Some(290));
    set_hp(&mut app, apocalypse, full - 10);
    // Up to and with the next tick whose number is a multiple of 19, then three intervals.
    let tick_now = app.world().resource::<sim::SimTick>().0;
    let to_next = (19 - (tick_now % 19) as u32) % 19 + 1;
    ticks(&mut app, to_next + 19 * 3);
    assert_eq!(hp(&mut app, apocalypse), Some(full - 10 + 4));
}

// --- Crews and survivors -------------------------------------------------------------------------

fn escaped(events: &[SimEvent]) -> Vec<SimId> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::CrewEscaped { units, .. } => Some(units.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn only_the_base_units_have_a_crew_and_it_is_of_their_owners_side() {
    // [AMCV] and [SMCV] Crewed=yes; [General] CrewEscape=50%, AlliedCrew=E1, SovietCrew=E2.
    let wrecks = |owner: PlayerId, unit: &str| -> Vec<Option<String>> {
        (0..120)
            .map(|seed| {
                let mut app = world_seeded(48, seed);
                // An aircraft that needs a dock has one (it is lost in its first tick without,
                // R49, and the row would show nothing): it stands on an Airforce Command.
                let definitions = defs(&app);
                let bound = definitions
                    .unit_kind(unit)
                    .and_then(|kind| definitions.metadata(ProductionItem::Unit(kind)))
                    .is_some_and(|entry| entry.airport_bound);
                if bound {
                    put(&mut app, owner, "airforce_command", 10, 10);
                }
                let id = put(&mut app, owner, unit, 10, 10);
                tick(&mut app, Vec::new());
                assert!(
                    hp(&mut app, id).is_some(),
                    "{unit} stands before it is destroyed"
                );
                set_hp(&mut app, id, 0);
                let events = tick(&mut app, Vec::new());
                assert!(destroyed(&events).contains(&id), "{unit} is destroyed");
                let crew = escaped(&events);
                assert!(crew.len() <= 1);
                crew.first().map(|&unit| kind_of(&mut app, unit))
            })
            .collect()
    };
    for (owner, unit, crew) in [
        (P0, "mcv", "gi"),
        (P1, "soviet_mcv", "conscript"),
        (P1, "mcv", "conscript"),
    ] {
        let found = wrecks(owner, unit);
        let escapes = found.iter().flatten().count();
        assert!(found.iter().flatten().all(|kind| kind == crew), "{unit}");
        // 50 percent of 120: within four standard deviations (5.5 each) of 60.
        assert!((38..=82).contains(&escapes), "{unit}: {escapes} of 120");
    }
    // A tank has no crew ([MTNK] has no Crewed key), and the pilots of the Harrier, the Black
    // Eagle and the NightHawk do not get out ([ORCA], [BEAG] and [SHAD] Crewed=yes: aircraft
    // release nobody).
    for unit in ["grizzly", "rhino", "harrier", "black_eagle", "nighthawk"] {
        assert_eq!(wrecks(P0, unit).iter().flatten().count(), 0, "{unit}");
    }
}

#[test]
fn the_survivors_of_shipped_buildings_follow_the_refund_and_the_divisors() {
    // [General] RefundPercent=50%, AlliedSurvivorDivisor=500, SovietSurvivorDivisor=250.
    for (owner, building, crew, count) in [
        // [GACNST] Cost=3000: 1500 / 500 = 3. [NACNST]: 1500 / 250 = 6.
        (P0, "construction_yard", "gi", 3),
        (P1, "soviet_construction_yard", "conscript", 6),
        // [GAPILE] Cost=500: 250 / 500 rounds up to 1. [NAHAND]: 250 / 250 = 1.
        (P0, "barracks", "gi", 1),
        (P1, "soviet_barracks", "conscript", 1),
        // [GAWEAP] Cost=2000: 1000 / 500 = 2. [NAWEAP]: 1000 / 250 = 4.
        (P0, "war_factory", "gi", 2),
        (P1, "soviet_war_factory", "conscript", 4),
        // Quotients that are not whole are rounded up (assumption of R48). [NAPOWR] Cost=600:
        // 300 / 250 = 1.2, 2 survivors. [GAOREP] Cost=2500: 1250 / 500 = 2.5, 3 survivors.
        (P1, "tesla_reactor", "conscript", 2),
        (P0, "ore_purifier", "gi", 3),
    ] {
        let mut app = world(48);
        let id = put(&mut app, owner, building, 10, 10);
        tick(&mut app, Vec::new());
        // Sold: the refund is paid and the survivors stand where the building stood.
        let before = app
            .world()
            .resource::<Players>()
            .get(owner)
            .expect("owner")
            .credits();
        let events = tick(
            &mut app,
            vec![IssuedCommand {
                player: owner,
                command: Command::SellBuilding { building: id },
            }],
        );
        let units = escaped(&events);
        let soldiers = units
            .iter()
            .filter(|&&unit| kind_of(&mut app, unit) == crew)
            .count();
        assert_eq!(soldiers, count, "{building}");
        assert!(units.len() - soldiers <= usize::from(building.contains("construction")));
        assert!(
            app.world()
                .resource::<Players>()
                .get(owner)
                .expect("owner")
                .credits()
                > before
        );
        // Soldiers on foot stand three to a cell: the first three in the building's first.
        if count >= 3 {
            for &unit in &units[..3] {
                assert_eq!(mobile(&mut app, unit).cell, CellPos::new(10, 10));
            }
        }
        assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
    }
    // Defences, shipyards and the Gap Generator are not crewed.
    for building in ["pillbox", "prism_tower", "gap_generator", "fortress_walls"] {
        let mut app = world(48);
        let id = put(&mut app, P0, building, 10, 10);
        tick(&mut app, Vec::new());
        set_hp(&mut app, id, 0);
        assert!(
            escaped(&tick(&mut app, Vec::new())).is_empty(),
            "{building}"
        );
    }
}

// --- Aircraft: ammunition and docks ---------------------------------------------------------------

/// An Airforce Command of P0 at (10, 10) with power, and `count` Harriers on its docks.
fn airfield(count: usize) -> (App, SimId, Vec<SimId>) {
    let mut app = world(48);
    put(&mut app, P0, "power_plant", 2, 2);
    let command = put(&mut app, P0, "airforce_command", 10, 10);
    let harriers: Vec<SimId> = (0..count)
        .map(|n| put(&mut app, P0, "harrier", 10 + n as i32, 14))
        .collect();
    run_until(&mut app, 200, |app, _| {
        harriers.iter().all(|id| {
            state(app).docks.contains_key(id)
                && mobile(app, *id).step.is_none()
                && (10..13).contains(&mobile(app, *id).cell.y)
        })
    });
    (app, command, harriers)
}

#[test]
fn a_fifth_harrier_is_refused_and_built_after_a_second_airforce_command_stands() {
    // [GAAIRC] NumberOfDocks=4; [ORCA] AirportBound=yes.
    let (mut app, _, harriers) = airfield(4);
    let cells: Vec<CellPos> = harriers
        .iter()
        .map(|&id| mobile(&mut app, id).cell)
        .collect();
    assert_eq!(
        cells,
        [
            CellPos::new(10, 10),
            CellPos::new(12, 10),
            CellPos::new(10, 12),
            CellPos::new(12, 12)
        ],
        "on the four corners of the building"
    );
    let harrier = ProductionItem::Unit(defs(&app).unit_kind("harrier").expect("harrier"));
    let queue = || IssuedCommand {
        player: P0,
        command: Command::QueueProduction { item: harrier },
    };
    assert_eq!(
        rejections(&tick(&mut app, vec![queue()])),
        [RejectReason::NoFreeDock(harrier)]
    );
    put(&mut app, P0, "airforce_command", 20, 10);
    assert!(rejections(&tick(&mut app, vec![queue()])).is_empty());
    // [ORCA] Cost=1200: 1008 ticks of build time.
    run_until(&mut app, 1300, |app, _| {
        units_of(app, P0, "harrier").len() == 5
    });
    let fifth = *units_of(&mut app, P0, "harrier").last().expect("the fifth");
    run_until(&mut app, 200, |app, _| {
        mobile(app, fifth).cell == CellPos::new(20, 10) && mobile(app, fifth).step.is_none()
    });
    // Without any Airforce Command nothing builds a Harrier, and nothing is queued.
    let mut app = world(48);
    put(&mut app, P0, "war_factory", 10, 10);
    tick(&mut app, Vec::new());
    // The reason names what is missing: its prerequisite, the Airforce Command ([ORCA]
    // Prerequisite=RADAR, of which the Airforce Command is the first member).
    let definitions = defs(&app);
    let harrier = ProductionItem::Unit(definitions.unit_kind("harrier").expect("harrier"));
    assert_eq!(
        rejections(&tick(&mut app, vec![queue()])),
        [RejectReason::MissingPrerequisite {
            item: harrier,
            prerequisite: definitions
                .building_kind("airforce_command")
                .expect("airforce command"),
        }]
    );
}

#[test]
fn a_harrier_fires_one_volley_and_reloads_it_on_its_dock() {
    // [ORCA] Ammo=1; [Maverick] Damage=150, Burst=2 through [ORCAAP]: heavy 100 %;
    // [General] ReloadRate=.3 minutes is 360 ticks.
    let (mut app, _, harriers) = airfield(1);
    let harrier = harriers[0];
    let target = put(&mut app, P1, "grizzly", 15, 10);
    silence(&mut app, target);
    set_hp(&mut app, target, 10_000);
    // The target is in its reach from the dock (5 of 6 cells): the order is obeyed there.
    let mut events = tick(&mut app, Vec::new());
    events.extend(tick(&mut app, vec![attack(P0, harrier, target)]));
    let (more, _) = run_until(&mut app, 200, |app, _| {
        state(app).ammo.get(&harrier) == Some(&0)
    });
    events.extend(more);
    let (more, _) = run_until(&mut app, 200, |app, _| {
        mobile(app, harrier).cell == CellPos::new(10, 10) && mobile(app, harrier).step.is_none()
    });
    events.extend(more);
    events.extend(ticks(&mut app, 30));
    assert_eq!(shots_of(&events, harrier).len(), 2, "one volley of two");
    assert_eq!(hp(&mut app, target), Some(10_000 - 300));
    // It reloads its one volley in 360 ticks on the dock. The target is taken away first
    // (scenario setup): a round is spent with the first shot of a volley
    // (docs/DECISIONS.md R56), and the reloaded Harrier would fire at it in the very tick in
    // which it has its round again.
    set_hp(&mut app, target, 0);
    tick(&mut app, Vec::new());
    let waited = state(&app).reloading[&harrier];
    ticks(&mut app, 359 - waited);
    assert_eq!(state(&app).ammo.get(&harrier), Some(&0));
    tick(&mut app, Vec::new());
    assert_eq!(state(&app).ammo.get(&harrier), None);
    // The NightHawk and the Kirov need no dock and have no ammunition.
    let definitions = defs(&app);
    for id in ["nighthawk", "kirov", "rocketeer"] {
        let entry = definitions
            .metadata(ProductionItem::Unit(definitions.unit_kind(id).expect(id)))
            .expect(id);
        assert!(!entry.airport_bound && entry.ammo == 0, "{id}");
    }
}

#[test]
fn harriers_are_lost_with_their_last_airforce_command() {
    let (mut app, command, harriers) = airfield(2);
    set_hp(&mut app, command, 0);
    let events = ticks(&mut app, 4);
    for harrier in harriers {
        assert!(destroyed(&events).contains(&harrier));
    }
}

// --- Spawned missiles and aircraft ----------------------------------------------------------------

/// A V3 launcher of P0 at (10, 10), an unarmed spotter at (17, 10), and a silent Grizzly of P1
/// with ample hit points at (20, 10), ten cells away ([V3Launcher] Range=18, MinimumRange=5).
fn v3_and_target() -> (App, SimId, SimId) {
    let mut app = world(48);
    let launcher = put(&mut app, P0, "v3_launcher", 10, 10);
    put(&mut app, P0, "engineer", 17, 10);
    let target = put(&mut app, P1, "grizzly", 20, 10);
    silence(&mut app, target);
    set_hp(&mut app, target, 10_000);
    (app, launcher, target)
}

#[test]
fn a_v3_rocket_is_a_unit_that_flies_to_its_target_and_deals_the_rockets_damage() {
    // The launcher fires at what comes into its reach by itself, as every armed unit does.
    let (mut app, launcher, target) = v3_and_target();
    let (events, _) = run_until(&mut app, 10, |_, events| !launched(events).is_empty());
    assert_eq!(shots_of(&events, launcher).len(), 1);
    let rocket = launched(&events)[0];
    assert_eq!(kind_of(&mut app, rocket), "v3_rocket");
    // [V3ROCKET] Strength=50, Armor=special_2, Speed=15 (29 subcells a tick).
    assert_eq!(hp(&mut app, rocket), Some(50));
    assert_eq!(mobile(&mut app, rocket).speed, 29);
    let (events, flight) = run_until(&mut app, 200, |_, events| !detonated(events).is_empty());
    // Ten cells at 29 subcells a tick: 89 ticks.
    assert!((88..=92).contains(&flight), "{flight}");
    assert_eq!(
        detonated(&events),
        [(rocket, SubPos::centre_of(CellPos::new(20, 10)))]
    );
    // [General] V3RocketDamage=200 through [V3WH]: heavy 70 %.
    assert_eq!(hp(&mut app, target), Some(10_000 - 140));
    // The next rocket: [V3] SpawnRegenRate=400 frames is 533 ticks after this one went off.
    let (later, wait) = run_until(&mut app, 600, |_, events| !launched(events).is_empty());
    assert!((533..=536).contains(&wait), "{wait}");
    assert_eq!(launched(&later).len(), 1);
    // An elite launcher: [General] V3RocketEliteDamage=400 through [V3EWH], heavy 70 %, with
    // the firepower of its rank ([V3] VeteranAbilities has FIREPOWER): 400 x 1.1 x 0.7 = 308.
    let (mut app, launcher, target) = v3_and_target();
    assert!(sim::promote(app.world_mut(), launcher, ELITE));
    run_until(&mut app, 200, |_, events| !detonated(events).is_empty());
    assert_eq!(hp(&mut app, target), Some(10_000 - 308));
}

#[test]
fn a_v3_rocket_is_shot_down_by_a_patriot_and_deals_nothing() {
    let (mut app, launcher, target) = v3_and_target();
    // A Patriot of P1 with power beside the rocket's way: [RedEye2] Damage=75 through [SAMWH]
    // against special_2 at 100 % destroys the rocket (50 hit points) with one missile.
    put(&mut app, P1, "tesla_reactor", 30, 2);
    put(&mut app, P1, "patriot", 16, 13);
    let events = ticks(&mut app, 150);
    assert_eq!(launched(&events).len(), 1);
    let rocket = launched(&events)[0];
    assert!(destroyed(&events).contains(&rocket));
    assert!(detonated(&events).is_empty());
    assert_eq!(hp(&mut app, target), Some(10_000), "it deals nothing");
    // The shot down rocket is worth [V3ROCKET] Cost=50 to nobody here (a building gains
    // no experience), and the launcher gained nothing.
    assert_eq!(
        experience(&mut app, launcher).map(|found| found.value),
        Some(0)
    );
}

#[test]
fn a_dreadnought_launches_two_missiles_with_their_damage() {
    // Water from column 24: the Dreadnought at (36, 10), the target on land 14 cells west
    // ([DredLauncher] Range=25, MinimumRange=8, Burst=2).
    let mut app = world(24);
    let dreadnought = put(&mut app, P0, "dreadnought", 36, 10);
    put(&mut app, P0, "engineer", 19, 10);
    let target = put(&mut app, P1, "grizzly", 22, 10);
    silence(&mut app, target);
    set_hp(&mut app, target, 10_000);
    tick(&mut app, Vec::new());
    let mut events = tick(&mut app, vec![attack(P0, dreadnought, target)]);
    let mut launches = Vec::new();
    let mut ends = 0;
    for n in 0..160 {
        for unit in launched(&events) {
            assert_eq!(kind_of(&mut app, unit), "dred_missile");
            // [DMISL] Speed=20 (38 subcells a tick), Strength=50.
            assert_eq!(mobile(&mut app, unit).speed, 38);
            launches.push(n);
        }
        ends += detonated(&events).len();
        events = tick(&mut app, Vec::new());
    }
    // Two missiles, 27 ticks apart; each [General] DMislDamage=300 through [DMISLWH]:
    // heavy 80 %.
    assert_eq!(launches.len(), 2, "{launches:?}");
    assert_eq!(launches[1] - launches[0], 27);
    assert_eq!(ends, 2);
    assert_eq!(hp(&mut app, target), Some(10_000 - 2 * 240));
    // What they destroy counts for the Dreadnought ([MTNK] Cost=700).
    set_hp(&mut app, target, 100);
    run_until(&mut app, 400, |app, _| hp(app, target).is_none());
    assert_eq!(
        experience(&mut app, dreadnought).map(|found| found.value),
        Some(700)
    );
}

#[test]
fn a_carrier_sends_its_three_hornets_and_regains_a_lost_one() {
    // Water from column 24: the carrier at (34, 10), the target on land ([HornetLauncher]
    // Range=25).
    let mut app = world(24);
    let carrier = put(&mut app, P0, "aircraft_carrier", 34, 10);
    put(&mut app, P0, "engineer", 19, 10);
    let target = put(&mut app, P1, "grizzly", 22, 10);
    silence(&mut app, target);
    set_hp(&mut app, target, 10_000);
    tick(&mut app, Vec::new());
    let mut events = tick(&mut app, vec![attack(P0, carrier, target)]);
    events.extend(ticks(&mut app, 110));
    let hornets = launched(&events);
    // [CARRIER] SpawnsNumber=3: three Hornets ([HORNET] Strength=75), and no fourth.
    assert_eq!(hornets.len(), 3);
    for &hornet in &hornets {
        assert_eq!(kind_of(&mut app, hornet), "hornet");
    }
    let (more, _) = run_until(&mut app, 400, |app, _| {
        hp(app, target) == Some(10_000 - 3 * 40)
    });
    events.extend(more);
    // [HornetBomb] Damage=40 through [ORCAAP]: heavy 100 %; one bomb each ([HORNET] Ammo=1).
    for &hornet in &hornets {
        assert_eq!(shots_of(&events, hornet).len(), 1);
    }
    // The last is lost on its way back: its place is filled after [CARRIER]
    // SpawnRegenRate=600 frames, 800 ticks; the others are aboard and ready after
    // SpawnReloadRate=150, 200 ticks.
    let lost = hornets[2];
    assert!(hp(&mut app, lost).is_some());
    set_hp(&mut app, lost, 0);
    set_hp(&mut app, target, 0);
    ticks(&mut app, 3);
    let slots = |app: &App| state(app).bays[&carrier].slots.clone();
    assert!(slots(&app).iter().any(|slot| matches!(slot, Slot::Lost(_))));
    ticks(&mut app, 400);
    let now = slots(&app);
    assert_eq!(
        now.iter().filter(|slot| **slot == Slot::Ready).count(),
        2,
        "{now:?}"
    );
    assert!(
        now.iter().any(|slot| matches!(slot, Slot::Lost(_))),
        "{now:?}"
    );
    ticks(&mut app, 410);
    assert_eq!(slots(&app), [Slot::Ready; 3]);
}

// --- Submerged units -------------------------------------------------------------------------------

#[test]
fn a_typhoon_is_unseen_at_five_cells_from_a_grizzly_and_seen_by_a_destroyer() {
    // Water from column 12: a Grizzly of P0 ([MTNK] Sight=8) on the shore at (10, 10) and a
    // Typhoon of P1 ([SUB] Cloakable=yes) at (15, 10).
    let mut app = world(12);
    put(&mut app, P0, "grizzly", 10, 10);
    let typhoon = put(&mut app, P1, "typhoon", 15, 10);
    silence(&mut app, typhoon);
    ticks(&mut app, 2);
    let sees = |app: &App, x: i32, y: i32| {
        app.world()
            .resource::<Players>()
            .get(P0)
            .expect("P0")
            .vision()
            .is_visible(CellPos::new(x, y))
    };
    assert!(sees(&app, 15, 10) && hidden(&app, P0, typhoon));
    // A Destroyer of P0: [DEST] Sensors=yes, SensorsSight=8. At 8 cells it detects, at 9 not.
    let destroyer = put(&mut app, P0, "destroyer", 24, 10);
    silence(&mut app, destroyer);
    ticks(&mut app, 2);
    assert!(hidden(&app, P0, typhoon), "nine cells away");
    set_hp(&mut app, destroyer, 0);
    ticks(&mut app, 2);
    let destroyer = put(&mut app, P0, "destroyer", 23, 10);
    silence(&mut app, destroyer);
    ticks(&mut app, 2);
    assert!(!hidden(&app, P0, typhoon), "eight cells away");
    // The Dolphin and the squid are cloakable too, and the three have sensors of 4 cells.
    let definitions = defs(&app);
    let entry = |id: &str| {
        definitions
            .metadata(ProductionItem::Unit(definitions.unit_kind(id).expect(id)))
            .expect(id)
            .clone()
    };
    for id in ["typhoon", "dolphin", "giant_squid"] {
        assert!(entry(id).cloakable && entry(id).sensors_sight == 4, "{id}");
    }
    assert!(!entry("destroyer").cloakable && entry("destroyer").sensors_sight == 8);
    let cloakable = definitions
        .arsenal()
        .0
        .values()
        .filter(|entry| entry.cloakable)
        .count();
    assert_eq!(cloakable, 3);
}

#[test]
fn a_destroyer_sends_its_osprey_against_a_typhoon_it_detects() {
    let mut app = world(0);
    let destroyer = put(&mut app, P0, "destroyer", 10, 10);
    let typhoon = put(&mut app, P1, "typhoon", 16, 10);
    silence(&mut app, typhoon);
    set_hp(&mut app, typhoon, 10_000);
    let (events, _) = run_until(&mut app, 300, |app, _| hp(app, typhoon) != Some(10_000));
    // The destroyer's gun does not reach under water: its only shot is the launcher's, and
    // the hit is the Osprey's: [ASWBomb] Damage=50 through [APSplash], heavy 100 %.
    let osprey = launched(&events)[0];
    assert_eq!(kind_of(&mut app, osprey), "osprey");
    assert_eq!(shots_of(&events, destroyer).len(), 1);
    assert_eq!(shots_of(&events, osprey).len(), 1);
    assert_eq!(hp(&mut app, typhoon), Some(10_000 - 50));
    // It flies back and is taken aboard.
    let (events, _) = run_until(&mut app, 300, |_, events| {
        events
            .iter()
            .any(|event| matches!(event, SimEvent::UnitRecovered { .. }))
    });
    assert!(destroyed(&events).is_empty());
    // Beyond the sensors the Typhoon is no target: nothing is launched.
    let mut app = world(0);
    let destroyer = put(&mut app, P0, "destroyer", 10, 10);
    let typhoon = put(&mut app, P1, "typhoon", 19, 10);
    silence(&mut app, typhoon);
    let events = ticks(&mut app, 200);
    assert!(hidden(&app, P0, typhoon));
    assert!(shots_of(&events, destroyer).is_empty() && launched(&events).is_empty());
}

#[test]
fn a_typhoon_that_fires_is_seen_for_the_cloak_delay() {
    // [General] CloakDelay=.02 minutes is 24 ticks; [SubTorpedo] ROF=120 is 160 ticks.
    let mut app = world(0);
    let transport = put(&mut app, P0, "amphibious_transport", 10, 10);
    set_hp(&mut app, transport, 100_000);
    let typhoon = put(&mut app, P1, "typhoon", 13, 10);
    let (_, _) = run_until(&mut app, 100, |_, events| {
        !shots_of(events, typhoon).is_empty()
    });
    assert!(!hidden(&app, P0, typhoon), "in the tick of its shot");
    ticks(&mut app, 23);
    assert!(!hidden(&app, P0, typhoon), "23 ticks later");
    tick(&mut app, Vec::new());
    assert!(hidden(&app, P0, typhoon), "24 ticks later");
}

// --- Regression: advisory review 2, sim, suspected item 4 (docs/DECISIONS.md R56) ------------------

#[test]
fn the_chronosphere_does_not_move_a_launched_rocket() {
    // A launched unit takes no command (R50 item 7), but the unit list of a Chronosphere did
    // not ask: the V3 rocket is a vehicle of the air domain, so its owner could put a rocket
    // in flight down on any cell it sees, where it went off.
    let (mut app, _, target) = v3_and_target();
    for column in 0..4 {
        put(&mut app, P0, "power_plant", 30 + 3 * column, 30);
    }
    let sphere = put(&mut app, P0, "chronosphere", 30, 34);
    let own = put(&mut app, P0, "grizzly", 12, 14);
    ticks(&mut app, 2);
    let definitions = defs(&app);
    let kind = definitions
        .building_kind("chronosphere")
        .expect("chronosphere");
    let charge = definitions
        .metadata(ProductionItem::Building(kind))
        .expect("chronosphere")
        .cooldown;
    app.world_mut()
        .resource_mut::<SpecialState>()
        .charge
        .insert(sphere, charge);
    let (events, _) = run_until(&mut app, 10, |_, events| !launched(events).is_empty());
    let rocket = launched(&events)[0];
    let send = |units: Vec<SimId>| IssuedCommand {
        player: P0,
        command: Command::Superweapon {
            building: sphere,
            target: CellPos::new(16, 14),
            units,
        },
    };
    let events = tick(&mut app, vec![send(vec![rocket])]);
    assert_eq!(rejections(&events), [RejectReason::NotCommandable(rocket)]);
    // The rocket flies on and goes off at its target; the Chronosphere is still charged, and
    // moves the player's own tank.
    let (events, _) = run_until(&mut app, 200, |_, events| !detonated(events).is_empty());
    assert_eq!(
        detonated(&events),
        [(rocket, SubPos::centre_of(CellPos::new(20, 10)))]
    );
    assert_eq!(hp(&mut app, target), Some(10_000 - 140));
    let events = tick(
        &mut app,
        vec![IssuedCommand {
            player: P0,
            command: Command::Chronoshift {
                building: sphere,
                source: CellPos::new(12, 14),
                target: CellPos::new(16, 14),
            },
        }],
    );
    assert!(rejections(&events).is_empty(), "{events:?}");
    assert_ne!(mobile(&mut app, own).cell, CellPos::new(12, 14));
}
