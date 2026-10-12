//! RA3 with the shipped data (`assets/data`): the armour types, warheads and weapons of Red
//! Alert 2 as the game fights with them (docs/RA2_PARITY_PLAN.md sections 4.3 and 4.4;
//! docs/DECISIONS.md R32 to R36). The data parity audit (`ra9_data_parity.rs`) compares every
//! value with the reference; these tests fire the weapons through `CommandQueue` and
//! `SimSchedule` and compare what they do with numbers worked out from the reference by hand.

use std::collections::BTreeMap;
use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    ArmourClass, Cell, CellPos, Command, CommandQueue, Condition, DamageState, Definitions, Grid,
    Health, IssuedCommand, PlayerId, Players, Position, ProductionItem, SimEvent, SimId, SimPlugin,
    SimSchedule, SpecialState, applied_damage, load_ai, load_ruleset, spawn_building, spawn_unit,
    splash_damage,
};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn shipped() -> Definitions {
    load_ruleset(assets().join("data")).unwrap_or_else(|error| panic!("{error}"))
}

/// A world of 48 x 48 open cells with the shipped data and the players P0 (America) and P1
/// (Russia), who are enemies; the cells from column `water_from` on are water.
fn world(water_from: i32) -> App {
    let definitions = shipped();
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 11 });
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
    for (player, country) in [(P0, "america"), (P1, "russia")] {
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

/// Keeps `id` from firing and moving (scenario setup: a target that only takes hits).
fn silence(app: &mut App, id: SimId) {
    app.world_mut()
        .resource_mut::<SpecialState>()
        .disabled
        .insert(id, u64::MAX);
}

/// Hit points no shot uses up, so that a target shows the damage of every hit.
const AMPLE: i32 = 100_000;

/// An attacker of P0 at (10, 10) and a silent target of P1 `cells` cells east of it with ample
/// hit points; the weapon of the attacker's first shot within 60 ticks and the damage of the
/// first hit on the target, or `None` if it does not fire.
fn first_shot(water_from: i32, attacker: &str, target: &str, cells: i32) -> Option<(String, i32)> {
    let mut app = world(water_from);
    // A defence needs power to fire (the Patriot and the Flak Cannon are `Powered`).
    if water_from >= 48 && defs(&app).building_kind(attacker).is_some() {
        put(&mut app, P0, "power_plant", 2, 2);
        put(&mut app, P0, "power_plant", 5, 2);
    }
    let attacker = put(&mut app, P0, attacker, 10, 10);
    let target = put(&mut app, P1, target, 10 + cells, 10);
    silence(&mut app, target);
    set_hp(&mut app, target, AMPLE);
    let mut weapon = None;
    for _ in 0..160 {
        let events = tick(&mut app, Vec::new());
        if weapon.is_none() {
            weapon = events.iter().find_map(|event| match event {
                SimEvent::WeaponFired {
                    attacker: by,
                    target: at,
                    weapon,
                } if *by == attacker => {
                    assert_eq!(*at, target);
                    Some(*weapon)
                }
                _ => None,
            });
        }
        let left = hp(&mut app, target).expect("the target");
        if left < AMPLE {
            let definitions = defs(&app);
            let id = definitions
                .weapon(weapon.expect("a shot before a hit"))?
                .id
                .clone();
            return Some((id, AMPLE - left));
        }
        if weapon.is_none() && app.world().resource::<sim::SimTick>().0 > 60 {
            return None;
        }
    }
    weapon.map(|weapon| (defs(&app).weapon(weapon).expect("weapon").id.clone(), 0))
}

// --- Every weapon against every armour type ---------------------------------------------------------

/// The rows of a table of `tests/reference` after its header.
fn table(name: &str) -> Vec<Vec<String>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/reference")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
        .lines()
        .skip(1)
        .map(|line| line.split('\t').map(str::to_owned).collect())
        .collect()
}

#[test]
fn every_shipped_weapon_deals_the_references_damage_to_every_armour_type() {
    // `Verses` of every warhead from the reference table, by the warhead's game id.
    let mut verses: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    let mut section_of: BTreeMap<String, String> = BTreeMap::new();
    for row in table("ra2_combat.tsv") {
        if row[1] == "warhead" {
            section_of.insert(row[2].clone(), row[0].clone());
        }
    }
    for row in table("ra2_reference.tsv") {
        if row[0] == "rules"
            && row[2] == "Verses"
            && let Some(game_id) = section_of.get(&row[1])
        {
            let percents = row[3]
                .split(',')
                .map(|text| text.trim_end_matches('%').parse().expect("a percentage"))
                .collect();
            verses.insert(game_id.clone(), percents);
        }
    }
    let definitions = shipped();
    assert_eq!(definitions.armour_classes().len(), 11);
    let mut pairs = 0;
    for weapon in definitions.weapons() {
        let warhead = definitions.warhead(weapon.warhead).expect("warhead");
        let percents = verses
            .get(&warhead.id)
            .unwrap_or_else(|| panic!("no Verses for {}", warhead.id));
        assert_eq!(percents.len(), 11);
        for (armour, &percent) in percents.iter().enumerate() {
            // Equation 2 from the reference: floor(Damage x Verses / 100), at least 1 unless
            // the percentage is 0.
            let expected = if percent == 0 {
                0
            } else {
                (i64::from(weapon.damage) * percent / 100).max(1)
            };
            let versus = definitions
                .versus(weapon.warhead, ArmourClass(armour as u16))
                .expect("versus");
            assert_eq!(
                i64::from(applied_damage(weapon.damage, versus)),
                expected,
                "{} against {}",
                weapon.id,
                definitions.armour_classes()[armour]
            );
            // A direct hit is at the impact point: the spread takes nothing off.
            assert_eq!(
                splash_damage(
                    weapon.damage,
                    versus,
                    warhead.cell_spread,
                    warhead.percent_at_max,
                    0
                ),
                applied_damage(weapon.damage, versus),
                "{}",
                weapon.id
            );
            pairs += 1;
        }
    }
    assert_eq!(pairs, definitions.weapons().len() * 11);
    assert!(pairs >= 900, "{pairs} pairs");
}

// --- Shots through the simulation -----------------------------------------------------------------

#[test]
fn shipped_weapons_deal_the_damage_worked_out_from_the_reference() {
    // (attacker, target, cells apart, the weapon fired, the damage of the first hit)
    let cases = [
        // [105mm] Damage=65 through [AP]: heavy 100 %, flak 25 %, wood 65 %, concrete 60 %.
        ("grizzly", "rhino", 4, "gun_105mm", 65),
        ("grizzly", "conscript", 4, "gun_105mm", 16),
        ("grizzly", "power_plant", 4, "gun_105mm", 42),
        ("grizzly", "fortress_walls", 3, "gun_105mm", 39),
        // [120mm] Damage=90 through [AP].
        ("rhino", "grizzly", 4, "gun_120mm", 90),
        // [M60] and [M1Carbine] Damage=15 through [SA]: flak 80 %, heavy 25 %.
        ("gi", "conscript", 3, "m60", 12),
        ("conscript", "grizzly", 3, "m1_carbine", 3),
        // [Vulcan2] Damage=50 through [SA].
        ("pillbox", "conscript", 4, "vulcan2", 40),
        // [ElectricBolt] Damage=50 through [Shock]: heavy 100 %.
        ("tesla_trooper", "grizzly", 2, "electric_bolt", 50),
        // [AWP] Damage=125 through [HollowPoint]: none 200 %.
        ("sniper", "gi", 8, "awp", 250),
        // [HoverMissile] Damage=25 through [HE]: heavy 35 % (8.75).
        ("ifv", "grizzly", 4, "hover_missile", 8),
        // [120mmx] Damage=100 through [ApocAP]; [MammothTusk] Damage=50 through [HE] against
        // the Kirov's light armour, 70 %.
        ("apocalypse", "grizzly", 4, "gun_120mmx", 100),
        ("apocalypse", "kirov", 5, "mammoth_tusk", 35),
        // [Comet] Damage=100 through [CometWH]: flak 100 %.
        ("prism_tank", "conscript", 7, "comet", 100),
        // [RedEye2] Damage=75 through [SAMWH]: light 100 %.
        ("patriot", "harrier", 8, "red_eye2", 75),
        // [Maverick] Damage=150 through [ORCAAP]: heavy 100 %.
        ("harrier", "grizzly", 4, "maverick", 150),
        // [20mm] Damage=25 through [SSA] against the Kirov's light armour, 60 %.
        ("rocketeer", "kirov", 4, "gun_20mm", 15),
    ];
    for (attacker, target, cells, weapon, damage) in cases {
        assert_eq!(
            first_shot(48, attacker, target, cells),
            Some((weapon.to_owned(), damage)),
            "{attacker} against {target}"
        );
    }
}

#[test]
fn shipped_types_fire_only_at_what_red_alert_2_lets_them() {
    // Weapons for the ground against aircraft and the reverse; the sniper's 1 % against a
    // tank (only on order); the GI's bullets against a wall; the dog's bite against a tank.
    for (attacker, target, cells) in [
        ("gi", "harrier", 3),
        ("grizzly", "rocketeer", 3),
        ("patriot", "grizzly", 5),
        ("flak_cannon", "grizzly", 4),
        ("sniper", "grizzly", 6),
        ("gi", "fortress_walls", 2),
        ("attack_dog", "grizzly", 1),
        ("yuri", "power_plant", 3),
    ] {
        assert_eq!(
            first_shot(48, attacker, target, cells),
            None,
            "{attacker} at {target}"
        );
    }
    // The types with a weapon for each layer use the one for the target's.
    for (attacker, target, cells, weapon) in [
        ("flak_track", "gi", 4, "flak_track_gun"),
        ("flak_track", "rocketeer", 5, "flak_track_aa_gun"),
        ("flak_trooper", "gi", 4, "flak_guy_gun"),
        ("flak_trooper", "harrier", 5, "flak_guy_aa_gun"),
        // The IFV's missile reaches aircraft 4 cells beyond its range of 6 (`AirRangeBonus`).
        ("ifv", "rocketeer", 8, "hover_missile"),
    ] {
        assert_eq!(
            first_shot(48, attacker, target, cells).map(|shot| shot.0),
            Some(weapon.to_owned()),
            "{attacker} at {target}"
        );
    }
    assert_eq!(
        first_shot(48, "ifv", "gi", 7),
        None,
        "the bonus is not for the ground"
    );
}

#[test]
fn shipped_ships_follow_their_naval_targeting() {
    // All at sea (water from column 0), the attacker at (10, 10).
    for (attacker, target, cells, expected) in [
        // [SubTorpedo] Damage=100 through [APSplash]: heavy 100 %.
        ("typhoon", "destroyer", 4, Some(("sub_torpedo", 100))),
        // The destroyer's gun cannot reach under water; its stand-in for the Osprey can:
        // [ASWBomb] Damage=50 through [APSplash].
        ("destroyer", "typhoon", 5, Some(("asw_launcher", 50))),
        // [155mm] Damage=60 through [ARTYHE]: heavy 60 %.
        ("destroyer", "sea_scorpion", 5, Some(("gun_155mm", 36))),
        // The sonic beam keeps M9's damage of 100 until RA6; [SonicWarhead]: heavy 80 %.
        ("dolphin", "typhoon", 3, Some(("sonic_zap", 80))),
        // The squid grips ships ([SquidGrab] Damage=15) and punches organic targets
        // ([SquidPunch] Damage=50 through [HE]: light 70 %).
        ("giant_squid", "destroyer", 1, Some(("squid_grab", 15))),
        ("giant_squid", "dolphin", 1, Some(("squid_punch", 35))),
        // The Aegis attacks aircraft only: [Medusa] Damage=100 through [SAMWH].
        ("aegis_cruiser", "destroyer", 5, None),
        ("aegis_cruiser", "harrier", 6, Some(("medusa", 100))),
        // The Sea Scorpion's guns do not reach under water.
        ("sea_scorpion", "typhoon", 4, None),
    ] {
        assert_eq!(
            first_shot(0, attacker, target, cells),
            expected.map(|(weapon, damage)| (weapon.to_owned(), damage)),
            "{attacker} at {target}"
        );
    }
    // From the shore (water from column 12): a tank engages a destroyer and never a
    // submarine, and a submarine engages nothing on land.
    assert_eq!(
        first_shot(12, "grizzly", "destroyer", 4),
        Some(("gun_105mm".to_owned(), 65))
    );
    assert_eq!(first_shot(12, "grizzly", "typhoon", 4), None);
    let mut app = world(12);
    let sub = put(&mut app, P0, "typhoon", 12, 10);
    put(&mut app, P1, "grizzly", 10, 10);
    let fired = (0..60).any(|_| {
        tick(&mut app, Vec::new()).iter().any(
            |event| matches!(event, SimEvent::WeaponFired { attacker, .. } if *attacker == sub),
        )
    });
    assert!(!fired, "a submarine attacks nothing on land");
}

// --- The nuclear strike, damage states, the computer player ---------------------------------------

/// A charged Nuclear Missile Silo of P0 with power, far from (`x`, `y`), struck there in the
/// presence of an engineer of P0 (unarmed) who sees the cell.
fn strike(app: &mut App, x: i32, y: i32) {
    let definitions = defs(app);
    for column in 0..4 {
        put(app, P0, "tesla_reactor", 1 + 3 * column, 1);
    }
    let silo = put(app, P0, "nuclear_silo", 2, 5);
    put(app, P0, "engineer", x - 3, y);
    tick(app, Vec::new());
    tick(app, Vec::new());
    let kind = definitions.building_kind("nuclear_silo").expect("silo");
    let charge = definitions
        .metadata(ProductionItem::Building(kind))
        .expect("silo")
        .cooldown;
    app.world_mut()
        .resource_mut::<SpecialState>()
        .charge
        .insert(silo, charge);
    let events = tick(
        app,
        vec![IssuedCommand {
            player: P0,
            command: Command::Superweapon {
                building: silo,
                target: CellPos::new(x, y),
                units: Vec::new(),
            },
        }],
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
}

#[test]
fn a_nuclear_strike_destroys_a_power_plant_and_not_a_construction_yard() {
    // [NukePayload] Damage=600 through [NUKE]: CellSpread=10, PercentAtMax=.02; concrete 8 %,
    // wood 60 %. A building takes one hit per cell of its footprint (plan section 4.3).
    let nuke = |versus: i32, distance: u64| splash_damage(600, versus, 2560, 2, distance);
    // The Construction Yard (3 x 3 cells, 1000 hit points, concrete) struck in its centre.
    let mut app = world(48);
    let yard = put(&mut app, P1, "construction_yard", 30, 30);
    strike(&mut app, 31, 31);
    let lost = nuke(8, 0) + 4 * nuke(8, 256) + 4 * nuke(8, 362);
    assert_eq!((nuke(8, 0), nuke(8, 256), nuke(8, 362)), (48, 43, 41));
    assert_eq!(hp(&mut app, yard), Some(1000 - lost));
    assert_eq!(lost, 384);
    // The Power Plant (2 x 2 cells, 750 hit points, wood) struck on one of its cells.
    let mut app = world(48);
    let plant = put(&mut app, P1, "power_plant", 30, 30);
    let tank = put(&mut app, P1, "grizzly", 30, 33);
    let soldier = put(&mut app, P1, "conscript", 31, 33);
    strike(&mut app, 30, 30);
    assert!(nuke(60, 0) + 2 * nuke(60, 256) + nuke(60, 362) > 750);
    assert_eq!(hp(&mut app, plant), None, "destroyed");
    // Three cells from the point a tank (heavy armour, 100 %) loses 600 x 70.6 % = 423 of its
    // 300 hit points, as a soldier does: at this distance nothing ordinary survives.
    assert_eq!(hp(&mut app, tank), None);
    assert_eq!(hp(&mut app, soldier), None);
}

#[test]
fn a_damaged_grizzly_is_yellow_then_red_and_drives_at_three_quarters_speed() {
    // [AudioVisual] ConditionYellow=50%, ConditionRed=25%; the Grizzly has 300 hit points and
    // moves 13 subcells a tick, damaged 13 - 13 / 4 = 10.
    let mut app = world(48);
    let tank = put(&mut app, P0, "grizzly", 5, 10);
    let condition = |app: &mut App| {
        let world = app.world_mut();
        let mut query = world.query::<(&SimId, &Condition)>();
        query
            .iter(world)
            .find(|(id, _)| **id == tank)
            .map(|(_, condition)| condition.0)
            .expect("condition")
    };
    let position = |app: &mut App| {
        let world = app.world_mut();
        let mut query = world.query::<(&SimId, &Position)>();
        query
            .iter(world)
            .find(|(id, _)| **id == tank)
            .map(|(_, position)| position.0.x)
            .expect("position")
    };
    for (hit_points, state) in [
        (151, DamageState::Green),
        (150, DamageState::Yellow),
        (76, DamageState::Yellow),
        (75, DamageState::Red),
    ] {
        set_hp(&mut app, tank, hit_points);
        tick(&mut app, Vec::new());
        assert_eq!(condition(&mut app), state, "at {hit_points}");
    }
    tick(
        &mut app,
        vec![IssuedCommand {
            player: P0,
            command: Command::Move {
                units: vec![tank],
                target: CellPos::new(40, 10),
            },
        }],
    );
    for _ in 0..5 {
        tick(&mut app, Vec::new());
    }
    let from = position(&mut app);
    for _ in 0..10 {
        tick(&mut app, Vec::new());
    }
    assert_eq!(position(&mut app) - from, 100, "red: 10 subcells a tick");
    set_hp(&mut app, tank, 300);
    tick(&mut app, Vec::new());
    tick(&mut app, Vec::new());
    let from = position(&mut app);
    for _ in 0..10 {
        tick(&mut app, Vec::new());
    }
    assert_eq!(
        position(&mut app) - from,
        130,
        "whole again: 13 subcells a tick"
    );
}

#[test]
fn the_armies_of_the_shipped_plans_can_destroy_every_shipped_building() {
    // The loader checks that the army of each side's plan damages every building type with a
    // weapon it uses against it (and no unit of an army attacks aircraft only).
    let definitions = shipped();
    let ai = load_ai(assets().join("data/ai.ron"), &definitions)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(ai.plans.len(), 2);
    // By hand for one pairing the check rests on: the Grizzly's shell ([AP], Wall=yes) against
    // the concrete of a wall, 60 %, and the GI's bullet ([SA], no Wall): nothing.
    let grizzly = definitions
        .unit_combat(definitions.unit_kind("grizzly").expect("grizzly"))
        .expect("terms");
    let gi = definitions
        .unit_combat(definitions.unit_kind("gi").expect("gi"))
        .expect("terms");
    let wall = definitions
        .building_combat(definitions.building_kind("fortress_walls").expect("wall"))
        .expect("terms");
    assert!(wall.wall);
    let target = sim::TargetTraits {
        armour: wall.armour,
        wall: true,
        ..sim::TargetTraits::default()
    };
    let percent = |terms: &sim::CombatTerms| {
        sim::weapon_against(&definitions, terms, terms.weapons(false), &target)
            .map(|(_, percent)| percent)
    };
    assert_eq!(percent(&grizzly), Some(60));
    assert_eq!(percent(&gi), Some(0));
}

// --- RA3-B: infantry sub cells and crushing with the shipped data (docs/DECISIONS.md R6, R41, R42) ---

/// The positions of the infantry spots of a cell.
fn spots(x: i32, y: i32) -> [sim::SubPos; 3] {
    [0, 1, 2].map(|spot| sim::SubPos::spot_of(CellPos::new(x, y), spot))
}

fn position_of(app: &mut App, id: SimId) -> sim::SubPos {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &Position)>();
    query
        .iter(world)
        .find(|(other, _)| **other == id)
        .map(|(_, position)| position.0)
        .unwrap_or_else(|| panic!("{id:?} is not a unit"))
}

fn assert_cells(app: &mut App, context: &str) {
    let problems = sim::cell_invariant_violations(app.world_mut());
    assert!(problems.is_empty(), "{context}: {problems:?}");
    let stray = sim::stray_claims(app.world_mut());
    assert!(stray.is_empty(), "{context}: {stray:?}");
}

#[test]
fn the_shipped_infantry_is_on_foot_and_the_shipped_crushers_are_those_of_red_alert_2() {
    let definitions = shipped();
    let mut on_foot = Vec::new();
    let mut flying_infantry = Vec::new();
    let mut not_crushable = Vec::new();
    let mut crushers = Vec::new();
    for (index, unit) in definitions.units().iter().enumerate() {
        let kind = sim::UnitKind(index as u16);
        let entry = definitions
            .metadata(ProductionItem::Unit(kind))
            .unwrap_or_else(|| panic!("{} has no arsenal entry", unit.id));
        // Every unit of the category infantry is of the type list, and no other unit is.
        assert_eq!(
            entry.infantry,
            entry.category == sim::Category::Infantry,
            "{}",
            unit.id
        );
        if definitions.on_foot(kind) {
            on_foot.push(unit.id.as_str());
            if !definitions.crushable(kind) {
                not_crushable.push(unit.id.as_str());
            }
        } else if entry.infantry {
            flying_infantry.push(unit.id.as_str());
        }
        if definitions.crusher(kind) {
            crushers.push(unit.id.as_str());
            assert_ne!(entry.domain, sim::Domain::Air, "{}", unit.id);
        }
        assert!(
            (1..=50).contains(&entry.size),
            "{}: size {}",
            unit.id,
            entry.size
        );
    }
    assert_eq!(on_foot.len(), 18, "{on_foot:?}");
    assert_eq!(flying_infantry, ["rocketeer"]);
    not_crushable.sort_unstable();
    assert_eq!(
        not_crushable,
        [
            "chrono_commando",
            "chrono_ivan",
            "chrono_legionnaire",
            "desolator",
            "psi_commando",
            "tesla_trooper",
            "yuri_prime"
        ]
    );
    crushers.sort_unstable();
    assert_eq!(
        crushers,
        [
            "apocalypse",
            "chrono_miner",
            "flak_track",
            "grizzly",
            "mcv",
            "mirage_tank",
            "prism_tank",
            "rhino",
            "soviet_amphibious_transport",
            "soviet_mcv",
            "tank_destroyer",
            "tesla_tank",
            "v3_launcher",
            "war_miner"
        ]
    );
    // [E1] Size=1, [MTNK] Size=3, [AMCV] Size=6, [CARRIER] Size=50.
    let size = |id: &str| {
        let kind = definitions.unit_kind(id).expect(id);
        definitions
            .metadata(ProductionItem::Unit(kind))
            .expect(id)
            .size
    };
    assert_eq!(
        [
            size("gi"),
            size("grizzly"),
            size("mcv"),
            size("aircraft_carrier")
        ],
        [1, 3, 6, 50]
    );
}

#[test]
fn three_gis_share_a_cell_and_a_fourth_soldier_or_a_tank_does_not_fit() {
    let mut app = world(48);
    let gis = [
        put(&mut app, P0, "gi", 10, 10),
        put(&mut app, P0, "gi", 10, 10),
        put(&mut app, P1, "conscript", 10, 10),
    ];
    for (soldier, at) in gis.into_iter().zip(spots(10, 10)) {
        assert_eq!(position_of(&mut app, soldier), at);
    }
    for kind in ["gi", "conscript", "attack_dog", "grizzly", "ifv"] {
        assert_eq!(
            spawn_unit(app.world_mut(), P0, kind, CellPos::new(10, 10)),
            Err(sim::SpawnError::Occupied {
                cell: CellPos::new(10, 10),
                by: gis[0]
            }),
            "{kind}"
        );
    }
    // The Rocketeer flies: the air above the cell is free, and its own.
    let rocketeer = put(&mut app, P0, "rocketeer", 10, 10);
    assert_eq!(
        position_of(&mut app, rocketeer),
        sim::SubPos::centre_of(CellPos::new(10, 10))
    );
    assert_cells(&mut app, "after the spawns");
}

/// A `mover` of P0 at (5, 20) is commanded to (15, 20) past `soldiers` of P1 that stand in
/// (10, 20) and are kept from firing; the ids of the crushed, in order, and whether the mover
/// arrived.
fn drive_through(mover: &str, soldiers: &[&str]) -> (Vec<SimId>, Vec<SimId>, bool) {
    let mut app = world(48);
    let standing: Vec<SimId> = soldiers
        .iter()
        .map(|kind| {
            let id = put(&mut app, P1, kind, 10, 20);
            silence(&mut app, id);
            id
        })
        .collect();
    let moving = put(&mut app, P0, mover, 5, 20);
    let mut commands = vec![IssuedCommand {
        player: P0,
        command: Command::Move {
            units: vec![moving],
            target: CellPos::new(15, 20),
        },
    }];
    let mut crushed = Vec::new();
    for index in 0..600 {
        let events = tick(&mut app, std::mem::take(&mut commands));
        assert_cells(
            &mut app,
            &format!("{mover} through {soldiers:?}, tick {index}"),
        );
        let mut arrived = false;
        for event in events {
            match event {
                SimEvent::UnitCrushed {
                    player,
                    unit,
                    crusher,
                    by,
                    ..
                } => {
                    assert_eq!((player, crusher, by), (P1, moving, P0));
                    crushed.push(unit);
                }
                SimEvent::MoveCompleted { unit, .. } if unit == moving => arrived = true,
                _ => {}
            }
        }
        if arrived {
            let at_goal = {
                let world = app.world_mut();
                let mut query = world.query::<(&SimId, &sim::Mobile)>();
                query
                    .iter(world)
                    .any(|(id, mobile)| *id == moving && mobile.cell == CellPos::new(15, 20))
            };
            return (standing, crushed, at_goal);
        }
    }
    panic!("{mover} through {soldiers:?}: the move did not end");
}

#[test]
fn a_tank_crushes_conscripts_and_gis_and_what_is_not_crushable_stays() {
    // [MTNK] Crusher=yes, [HTNK] Crusher=yes; [E1] Crushable=yes, [E2] in [InfantryTypes].
    for tank in ["grizzly", "rhino", "war_miner", "mcv"] {
        let (standing, crushed, arrived) = drive_through(tank, &["conscript", "gi", "engineer"]);
        assert_eq!(crushed, standing, "{tank}");
        assert!(arrived, "{tank}");
    }
    // [SHK], [DESO], [CLEG] Crushable=no: the tank goes round them, and round a cell that
    // holds one of them beside a Conscript.
    for soldiers in [
        &["tesla_trooper"][..],
        &["desolator"],
        &["chrono_legionnaire"],
        &["conscript", "desolator"],
    ] {
        let (_, crushed, arrived) = drive_through("grizzly", soldiers);
        assert!(crushed.is_empty(), "{soldiers:?}");
        assert!(arrived, "{soldiers:?}");
    }
    // [FV] Crusher=no, [DRON] and [DTRUCK] have no such key: they go round a Conscript.
    for vehicle in ["ifv", "terror_drone", "demolition_truck"] {
        let (_, crushed, arrived) = drive_through(vehicle, &["conscript"]);
        assert!(crushed.is_empty(), "{vehicle}");
        assert!(arrived, "{vehicle}");
    }
}
