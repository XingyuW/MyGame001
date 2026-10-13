//! RA4: veterancy (docs/RA2_PARITY_PLAN.md sections 4.3 to 4.5; docs/DECISIONS.md R47), on the
//! RA4 fixture (`tests/fixtures/data/ra4`; invariant I10), through `CommandQueue` and
//! `SimSchedule`.
//!
//! The fixture's rules are the ini's: a veteran at 3 times its own cost, elite at 6 times;
//! firepower x 1.1, armour x 1.5, reload x 0.6, speed x 1.2. The tank (400 credits, a cannon of
//! 40 every 25 ticks, speed 32) is a veteran with stronger, firepower, rof, sight and faster and
//! elite with self_heal and three of them again; the heavy (650) has firepower as a veteran and
//! stronger at elite rank. An engineer costs 300, a truck 100, a power plant 300.

mod ra4_common;

use bevy_app::App;
use ra4_common::*;
use sim::{
    CellPos, Command, ELITE, Experience, Multipliers, PlayerId, Players, SimEvent, SimId, VETERAN,
    load_match_definitions,
};

const P2: PlayerId = PlayerId(2);

/// Orders `attacker` to attack `victim`, whose hit points are set to 1, and runs until the
/// victim is gone; the events of those ticks.
fn kill(app: &mut App, attacker: SimId, victim: SimId) -> Vec<SimEvent> {
    set_hp(app, victim, 1);
    let mut events = tick(app, vec![attack(P0, attacker, victim)]);
    for _ in 0..300 {
        if !alive(app, victim) {
            return events;
        }
        events.extend(tick(app, Vec::new()));
    }
    panic!("{victim:?} was not destroyed");
}

fn ranks_gained(events: &[SimEvent]) -> Vec<(SimId, u8)> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::RankGained { unit, rank, .. } => Some((*unit, *rank)),
            _ => None,
        })
        .collect()
}

/// Sets the experience of the unit `id` to `value` credits (a setup step: the unit has its
/// experience component after its first tick).
fn set_experience(app: &mut App, id: SimId, value: i64) {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &mut Experience)>();
    for (other, mut experience) in query.iter_mut(world) {
        if *other == id {
            experience.value = value;
        }
    }
}

/// A world with a tank of P0 at (10, 10) and `count` engineers of P1 east of it, in its sight
/// and its range, after one tick.
fn tank_and_engineers(count: i32) -> (App, SimId, Vec<SimId>) {
    let mut app = world();
    let tank = spawn(&mut app, P0, "tank", 10, 10);
    let engineers = (0..count)
        .map(|n| spawn(&mut app, P1, "engineer", 13 + n / 5, 8 + n % 5))
        .collect();
    tick(&mut app, Vec::new());
    (app, tank, engineers)
}

// --- Positive: experience and ranks ---------------------------------------------------------

#[test]
fn a_unit_is_a_veteran_at_three_times_its_cost_and_elite_at_six_times() {
    let (mut app, tank, engineers) = tank_and_engineers(8);
    assert_eq!(experience(&mut app, tank), Some(Experience::default()));
    let mut gained = Vec::new();
    for (n, engineer) in engineers.iter().enumerate() {
        let events = kill(&mut app, tank, *engineer);
        gained.extend(ranks_gained(&events));
        let found = experience(&mut app, tank).expect("experience");
        let killed = n as i64 + 1;
        assert_eq!(found.value, 300 * killed, "after {killed} engineers");
        // 400 x 3 = 1200 after the fourth, 400 x 6 = 2400 after the eighth.
        let rank = match killed {
            ..4 => 0,
            4..8 => VETERAN,
            _ => ELITE,
        };
        assert_eq!(found.rank, rank, "after {killed} engineers");
    }
    assert_eq!(gained, [(tank, VETERAN), (tank, ELITE)]);
}

#[test]
fn the_thresholds_are_inclusive_and_one_credit_less_is_not_enough() {
    // (rank and experience before the kill of an engineer worth 300, rank after it): a tank
    // costs 400, so the thresholds are 1200 and 2400.
    for (from, before, rank) in [
        (0, 899, 0),
        (0, 900, VETERAN),
        (VETERAN, 2099, VETERAN),
        (VETERAN, 2100, ELITE),
    ] {
        let (mut app, tank, engineers) = tank_and_engineers(1);
        if from > 0 {
            assert!(sim::promote(app.world_mut(), tank, from));
        }
        set_experience(&mut app, tank, before);
        let events = kill(&mut app, tank, engineers[0]);
        let found = experience(&mut app, tank).expect("experience");
        assert_eq!(found.value, before + 300);
        assert_eq!(found.rank, rank, "from {before}");
        assert_eq!(
            ranks_gained(&events),
            if rank > from {
                vec![(tank, rank)]
            } else {
                Vec::new()
            },
            "from {before}"
        );
    }
    // A unit that skips a rank is reported once, at the rank it reaches.
    let (mut app, tank, engineers) = tank_and_engineers(1);
    set_experience(&mut app, tank, 2100);
    let events = kill(&mut app, tank, engineers[0]);
    assert_eq!(ranks_gained(&events), [(tank, ELITE)]);
}

#[test]
fn a_destroyed_building_gives_its_cost() {
    let mut app = world();
    let tank = spawn(&mut app, P0, "tank", 10, 10);
    let plant = place(&mut app, P1, "allied_power", 13, 10);
    tick(&mut app, Vec::new());
    kill(&mut app, tank, plant);
    assert_eq!(
        experience(&mut app, tank).map(|found| found.value),
        Some(300)
    );
}

#[test]
fn splash_counts_for_the_unit_that_fired_and_only_enemies_count() {
    // P0 and P1 are allies; P2 is the enemy of both.
    let mut app = world();
    {
        let mut players = app.world_mut().resource_mut::<Players>();
        players.add(P2, 0).expect("a third player");
        assert!(players.set_team(P0, 1) && players.set_team(P1, 1));
    }
    let mortar = spawn(&mut app, P0, "mortar", 10, 10);
    // The blast (2 cells, 25 percent at its edge, 80 against flesh) lands on the truck.
    let truck = spawn(&mut app, P2, "truck", 15, 10);
    let enemy = spawn(&mut app, P2, "engineer", 16, 10);
    let ally = spawn(&mut app, P1, "engineer", 15, 11);
    let own = spawn(&mut app, P0, "engineer", 15, 9);
    tick(&mut app, Vec::new());
    for id in [enemy, ally, own] {
        set_hp(&mut app, id, 1);
    }
    tick(&mut app, vec![attack(P0, mortar, truck)]);
    run_until(&mut app, 200, |app, _| !alive(app, enemy));
    assert!(!alive(&mut app, ally) && !alive(&mut app, own));
    // Only the enemy's engineer counts: 300, not 900.
    assert_eq!(
        experience(&mut app, mortar).map(|found| found.value),
        Some(300)
    );
}

#[test]
fn crushing_counts_for_the_crusher() {
    let mut app = world();
    let roller = spawn(&mut app, P0, "roller", 10, 10);
    for _ in 0..3 {
        spawn(&mut app, P1, "recruit", 13, 10);
    }
    tick(&mut app, Vec::new());
    let mut events = tick(&mut app, vec![move_to(P0, &[roller], 16, 10)]);
    events.extend(ticks(&mut app, 80));
    let crushed = events
        .iter()
        .filter(|event| matches!(event, SimEvent::UnitCrushed { .. }))
        .count();
    assert_eq!(crushed, 3);
    // Three recruits of 100 are three times the roller's cost of 100: a veteran.
    let found = experience(&mut app, roller).expect("experience");
    assert_eq!((found.value, found.rank), (300, VETERAN));
    assert_eq!(ranks_gained(&events), [(roller, VETERAN)]);
}

// --- Positive: what a rank grants -------------------------------------------------------------

#[test]
fn a_rank_grants_each_ability_of_its_lists_once() {
    let mut app = world();
    let tank = spawn(&mut app, P0, "tank", 10, 10);
    let heavy = spawn(&mut app, P0, "heavy", 12, 10);
    let raider = spawn(&mut app, P0, "raider", 14, 10);
    tick(&mut app, Vec::new());
    let ranked = |app: &mut App, id| experience(app, id).expect("experience").ranked;
    assert_eq!(ranked(&mut app, tank), Multipliers::NONE);
    assert!(sim::promote(app.world_mut(), tank, VETERAN));
    assert!(sim::promote(app.world_mut(), heavy, VETERAN));
    assert!(sim::promote(app.world_mut(), raider, VETERAN));
    let veteran = Multipliers {
        firepower_pm: 1100,
        armour_pm: 1500,
        reload_pm: 600,
        speed_pm: 1200,
    };
    assert_eq!(ranked(&mut app, tank), veteran);
    assert!(!experience(&mut app, tank).expect("experience").self_heal);
    assert_eq!(
        ranked(&mut app, heavy),
        Multipliers {
            firepower_pm: 1100,
            ..Multipliers::NONE
        }
    );
    // A type without abilities has the rank and nothing else.
    assert_eq!(ranked(&mut app, raider), Multipliers::NONE);
    assert_eq!(rank(&mut app, raider), VETERAN);
    // Elite: the tank's list names stronger, firepower and rof again, which changes nothing,
    // and adds self healing; the heavy gains stronger.
    assert!(sim::promote(app.world_mut(), tank, ELITE));
    assert!(sim::promote(app.world_mut(), heavy, ELITE));
    assert_eq!(ranked(&mut app, tank), veteran);
    assert!(experience(&mut app, tank).expect("experience").self_heal);
    assert_eq!(
        ranked(&mut app, heavy),
        Multipliers {
            firepower_pm: 1100,
            armour_pm: 1500,
            ..Multipliers::NONE
        }
    );
    // The experience is what the rank needs: 6 x 400 and 6 x 650.
    assert_eq!(experience(&mut app, tank).expect("experience").value, 2400);
    assert_eq!(experience(&mut app, heavy).expect("experience").value, 3900);
    // An elite unit carries its elite weapons: the autocannon in the place of the cannon,
    // and for the heavy beside the sam it keeps.
    let autocannon = weapon_kind(&app, "autocannon");
    assert_eq!(weapon(&mut app, tank).kind, autocannon);
    assert_eq!(weapon(&mut app, heavy).kind, autocannon);
    assert_eq!(
        weapon(&mut app, heavy).secondary,
        Some(weapon_kind(&app, "sam"))
    );
}

/// The damage of the first shot of `attacker` at `target`, and the ticks between its first two
/// shots.
fn first_shot_and_reload(app: &mut App, attacker: SimId, target: SimId) -> (i32, u32) {
    let before = hp(app, target).expect("target");
    let mut events = tick(app, vec![attack(P0, attacker, target)]);
    let mut fired_at = Vec::new();
    let mut damage = None;
    for n in 0..120 {
        if !shots(&events, attacker).is_empty() {
            fired_at.push(n);
            if damage.is_none() {
                damage = Some(before - hp(app, target).expect("target"));
            }
            if fired_at.len() == 2 {
                break;
            }
        }
        events = tick(app, Vec::new());
    }
    (damage.expect("a shot"), (fired_at[1] - fired_at[0]) as u32)
}

#[test]
fn firepower_and_rate_of_fire_are_measured_on_the_shots() {
    let scenario = |rank: u8| {
        let mut app = world();
        let tank = spawn(&mut app, P0, "tank", 10, 10);
        let truck = spawn(&mut app, P1, "truck", 13, 10);
        tick(&mut app, Vec::new());
        if rank > 0 {
            assert!(sim::promote(app.world_mut(), tank, rank));
        }
        first_shot_and_reload(&mut app, tank, truck)
    };
    // The cannon: 40 against plate at 100 percent, every 25 ticks.
    assert_eq!(scenario(0), (40, 25));
    // A veteran: 40 x 1.1 = 44, and 25 x 0.6 = 15 ticks.
    assert_eq!(scenario(VETERAN), (44, 15));
}

#[test]
fn an_elite_unit_fires_its_elite_weapon_with_its_multipliers() {
    let mut app = world();
    let tank = spawn(&mut app, P0, "tank", 10, 10);
    let truck = spawn(&mut app, P1, "truck", 13, 10);
    tick(&mut app, Vec::new());
    assert!(sim::promote(app.world_mut(), tank, ELITE));
    let autocannon = weapon_kind(&app, "autocannon");
    let mut events = tick(&mut app, vec![attack(P0, tank, truck)]);
    events.extend(ticks(&mut app, 40));
    let fired = shots(&events, tank);
    // A burst of three shots of 20 x 1.1 = 22, 5 ticks apart, then 30 x 0.6 = 18 ticks.
    assert!(fired.len() >= 4, "{fired:?}");
    assert!(fired.iter().all(|&(_, weapon)| weapon == autocannon));
    let times: Vec<usize> = {
        let mut app = world();
        let tank = spawn(&mut app, P0, "tank", 10, 10);
        let truck = spawn(&mut app, P1, "truck", 13, 10);
        tick(&mut app, Vec::new());
        assert!(sim::promote(app.world_mut(), tank, ELITE));
        let mut times = Vec::new();
        let mut events = tick(&mut app, vec![attack(P0, tank, truck)]);
        for n in 0..40 {
            if !shots(&events, tank).is_empty() {
                times.push(n);
            }
            events = tick(&mut app, Vec::new());
        }
        assert_eq!(hp(&mut app, truck), Some(300 - 22 * times.len() as i32));
        times
    };
    assert_eq!(&times[..4], [0, 5, 10, 28], "{times:?}");
}

#[test]
fn stronger_armour_divides_the_damage_taken() {
    let scenario = |rank: u8| {
        let mut app = world();
        let attacker = spawn(&mut app, P0, "tank", 10, 10);
        let target = spawn(&mut app, P1, "tank", 13, 10);
        tick(&mut app, Vec::new());
        if rank > 0 {
            assert!(sim::promote(app.world_mut(), target, rank));
        }
        first_shot_and_reload(&mut app, attacker, target).0
    };
    assert_eq!(scenario(0), 40);
    // 40 / 1.5 = 26.67, rounded down.
    assert_eq!(scenario(VETERAN), 26);
    assert_eq!(scenario(ELITE), 26, "stronger is not applied twice");
}

#[test]
fn a_hit_of_one_point_on_stronger_armour_deals_nothing() {
    // The plan's equations: the minimum of 1 is applied before the armour multiplier.
    assert_eq!(sim::impact_damage(10, 1000, 1, 0, 100, 0, 1000), 1);
    assert_eq!(sim::impact_damage(10, 1000, 1, 0, 100, 0, 1500), 0);
    assert_eq!(sim::impact_damage(40, 1100, 100, 0, 100, 0, 1500), 29);
    // With neutral multipliers it is the damage of RA3, at every distance of a spread.
    for distance in [0, 100, 256, 511, 512, 513] {
        assert_eq!(
            sim::impact_damage(80, 1000, 50, 512, 25, distance, 1000),
            sim::splash_damage(80, 50, 512, 25, distance),
            "at {distance}"
        );
    }
    // Firepower is inside the one division: 15 x 1.1 x 0.25 = 4.125, not floor(16.5) x 0.25.
    assert_eq!(sim::impact_damage(15, 1100, 25, 0, 100, 0, 1000), 4);
    assert_eq!(
        sim::impact_damage(i32::MAX, u32::MAX, i32::MAX, 0, 100, 0, 1),
        i32::MAX
    );
    assert_eq!(sim::impact_damage(40, 1100, 0, 0, 100, 0, 1000), 0);
}

#[test]
fn faster_is_measured_on_the_way_and_comes_before_the_slowing_of_damage() {
    let run = |rank: u8, damaged: bool| {
        let mut app = world();
        let tank = spawn(&mut app, P0, "tank", 5, 10);
        tick(&mut app, Vec::new());
        if rank > 0 {
            assert!(sim::promote(app.world_mut(), tank, rank));
        }
        if damaged {
            set_hp(&mut app, tank, 150);
            tick(&mut app, Vec::new());
        }
        tick(&mut app, vec![move_to(P0, &[tank], 25, 10)]);
        let (_, ran) = run_until(&mut app, 400, |app, _| {
            mobile(app, tank).cell == CellPos::new(25, 10) && mobile(app, tank).step.is_none()
        });
        (mobile(&mut app, tank).speed, ran)
    };
    let (speed, plain) = run(0, false);
    assert_eq!(speed, 32);
    // 32 x 1.2 = 38.4, rounded down.
    let (speed, veteran) = run(VETERAN, false);
    assert_eq!(speed, 38);
    assert!(veteran < plain, "{veteran} against {plain}");
    // 20 cells of 256 subcells are 5120: 160 ticks at 32 and 135 at 38 (134.7, what is left of
    // a tick's way carries into the next step), the first of them the tick of the command.
    assert_eq!((plain, veteran), (159, 134));
    // A yellow veteran: 38 - 38 / 4 = 29.
    assert_eq!(run(VETERAN, true).0, 29);
    assert_eq!(run(0, true).0, 24);
}

#[test]
fn sight_is_not_changed_by_a_rank() {
    let mut app = world();
    let tank = spawn(&mut app, P0, "tank", 10, 10);
    tick(&mut app, Vec::new());
    let sight = |app: &mut App| {
        let world = app.world_mut();
        let mut query = world.query::<(&SimId, &sim::Sight)>();
        query
            .iter(world)
            .find(|(id, _)| **id == tank)
            .map(|(_, sight)| sight.0)
    };
    let before = sight(&mut app);
    assert!(sim::promote(app.world_mut(), tank, ELITE));
    ticks(&mut app, 3);
    assert_eq!(sight(&mut app), before);
}

#[test]
fn self_healing_restores_a_point_every_repair_interval_up_to_full_health() {
    let mut app = world();
    let elite = spawn(&mut app, P0, "tank", 10, 10);
    let plain = spawn(&mut app, P0, "tank", 12, 10);
    let seer = spawn(&mut app, P0, "seer", 14, 10);
    tick(&mut app, Vec::new());
    assert!(sim::promote(app.world_mut(), elite, ELITE));
    for id in [elite, plain] {
        set_hp(&mut app, id, 297);
    }
    set_hp(&mut app, seer, 98);
    // The fixture's repair interval is 5 ticks: one point each on ticks 5, 10, 15, ...
    ticks(&mut app, 4);
    assert_eq!(hp(&mut app, elite), Some(297));
    tick(&mut app, Vec::new());
    assert_eq!(hp(&mut app, elite), Some(298));
    assert_eq!(hp(&mut app, seer), Some(99), "the seer heals by its type");
    ticks(&mut app, 30);
    assert_eq!(hp(&mut app, elite), Some(300), "up to full health");
    assert_eq!(hp(&mut app, seer), Some(100));
    assert_eq!(hp(&mut app, plain), Some(297), "no ability, no healing");
}

#[test]
fn a_bonus_multiplies_with_the_rank_and_is_set_through_the_api() {
    let mut app = world();
    let tank = spawn(&mut app, P0, "tank", 10, 10);
    let truck = spawn(&mut app, P1, "truck", 13, 10);
    tick(&mut app, Vec::new());
    assert!(sim::promote(app.world_mut(), tank, VETERAN));
    assert!(sim::set_bonus(
        app.world_mut(),
        tank,
        Multipliers {
            firepower_pm: 2000,
            armour_pm: 1500,
            speed_pm: 1200,
            ..Multipliers::NONE
        }
    ));
    let found = experience(&mut app, tank).expect("experience");
    assert_eq!(found.firepower_pm(), 2200);
    assert_eq!(found.armour_pm(), 2250);
    assert_eq!(found.speed_pm(), 1440);
    assert_eq!(found.reload_pm(), 600);
    // 40 x 1.1 x 2.0 = 88.
    assert_eq!(first_shot_and_reload(&mut app, tank, truck).0, 88);
    assert_eq!(mobile(&mut app, tank).speed, 46, "32 x 1.44 = 46.08");
}

#[test]
fn a_passenger_keeps_its_rank_and_its_elite_weapon() {
    let mut app = world();
    let tank = spawn(&mut app, P0, "tank", 10, 10);
    let hovercraft = spawn(&mut app, P0, "hovercraft", 11, 10);
    tick(&mut app, Vec::new());
    assert!(sim::promote(app.world_mut(), tank, ELITE));
    let before = experience(&mut app, tank).expect("experience");
    tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: tank,
                target: hovercraft,
            },
        )],
    );
    assert!(!alive(&mut app, tank), "aboard");
    tick(
        &mut app,
        vec![issue(P0, Command::Deploy { unit: hovercraft })],
    );
    assert_eq!(experience(&mut app, tank), Some(before));
    assert_eq!(weapon(&mut app, tank).kind, weapon_kind(&app, "autocannon"));
}

// --- Negative --------------------------------------------------------------------------------

#[test]
fn an_untrainable_unit_and_a_building_gain_nothing() {
    let mut app = world();
    let conscript = spawn(&mut app, P0, "conscript", 10, 10);
    // The bunker needs no power to fire.
    let turret = place(&mut app, P0, "bunker", 10, 14);
    let near_conscript = spawn(&mut app, P1, "engineer", 12, 10);
    let near_turret = spawn(&mut app, P1, "engineer", 12, 14);
    tick(&mut app, Vec::new());
    let events = kill(&mut app, conscript, near_conscript);
    assert_eq!(experience(&mut app, conscript), None);
    assert_eq!(rank(&mut app, conscript), 0);
    set_hp(&mut app, near_turret, 1);
    let (more, _) = run_until(&mut app, 100, |app, _| !alive(app, near_turret));
    assert_eq!(experience(&mut app, turret), None);
    assert!(ranks_gained(&events).is_empty() && ranks_gained(&more).is_empty());
    // The API refuses them too, and a unit that does not exist.
    assert!(!sim::promote(app.world_mut(), conscript, VETERAN));
    assert!(!sim::promote(app.world_mut(), turret, VETERAN));
    assert!(!sim::promote(app.world_mut(), SimId(9999), VETERAN));
    assert!(!sim::set_bonus(
        app.world_mut(),
        conscript,
        Multipliers::NONE
    ));
}

#[test]
fn a_promotion_never_lowers_a_rank_and_stops_at_the_cap() {
    let mut app = world();
    let tank = spawn(&mut app, P0, "tank", 10, 10);
    tick(&mut app, Vec::new());
    assert!(!sim::promote(app.world_mut(), tank, 0));
    assert!(
        sim::promote(app.world_mut(), tank, 9),
        "9 is capped at elite"
    );
    assert_eq!(rank(&mut app, tank), ELITE);
    assert!(!sim::promote(app.world_mut(), tank, VETERAN));
    assert!(!sim::promote(app.world_mut(), tank, ELITE));
    assert_eq!(rank(&mut app, tank), ELITE);
}

#[test]
fn without_a_rules_file_nobody_has_experience() {
    let definitions = load_match_definitions(
        fixture("units.ron"),
        fixture("economy.ron"),
        fixture("combat.ron"),
    )
    .and_then(|definitions| definitions.with_arsenal(fixture("arsenal.ron")));
    // The fixture's arsenal names countries, which need rules: the legacy way is an arsenal
    // without them, so the world below has no arsenal at all.
    assert!(definitions.is_err());
    let definitions = load_match_definitions(
        fixture("units.ron"),
        fixture("economy.ron"),
        fixture("combat.ron"),
    )
    .expect("the fixture");
    let mut app = App::new();
    app.add_plugins(sim::SimPlugin { seed: 1 });
    let cell = sim::Cell {
        passable: true,
        water: false,
        buildable: true,
        ore: 0,
    };
    app.world_mut()
        .insert_resource(sim::Grid::from_cells(40, 40, vec![cell; 1600]).expect("a grid"));
    for player in [P0, P1] {
        app.world_mut()
            .resource_mut::<Players>()
            .add(player, 0)
            .expect("a player");
    }
    app.world_mut().insert_resource(definitions);
    let tank = spawn(&mut app, P0, "tank", 10, 10);
    let engineer = spawn(&mut app, P1, "engineer", 13, 10);
    tick(&mut app, Vec::new());
    let events = kill(&mut app, tank, engineer);
    assert_eq!(experience(&mut app, tank), None);
    assert!(ranks_gained(&events).is_empty());
    assert!(!sim::promote(app.world_mut(), tank, VETERAN));
}

// --- Integration and regression ------------------------------------------------------------------

/// A battle in which units are promoted: ten tanks of each player with engineers behind them.
fn battle(seed: u64) -> App {
    let mut app = world_of(definitions(), seed);
    for n in 0..10 {
        spawn(&mut app, P0, "tank", 12, 5 + 2 * n);
        spawn(&mut app, P1, "tank", 20, 5 + 2 * n);
        spawn(&mut app, P1, "engineer", 22, 5 + 2 * n);
        spawn(&mut app, P1, "engineer", 23, 5 + 2 * n);
    }
    app
}

#[test]
fn a_long_battle_promotes_units_and_runs_the_same_twice() {
    let run = |seed: u64| {
        let mut app = battle(seed);
        let own = units_of(&mut app, P0, "tank");
        tick(&mut app, Vec::new());
        let mut hashes = vec![hash(&mut app)];
        let mut gained = Vec::new();
        let mut events = tick(
            &mut app,
            vec![issue(
                P0,
                Command::AttackMove {
                    units: own,
                    target: CellPos::new(24, 14),
                },
            )],
        );
        for _ in 0..1500 {
            gained.extend(ranks_gained(&events));
            hashes.push(hash(&mut app));
            assert_cells(&mut app, "the battle");
            events = tick(&mut app, Vec::new());
        }
        (hashes, gained)
    };
    let (first, gained) = run(3);
    let (second, again) = run(3);
    assert_eq!(first, second, "the same seed, the same state at every tick");
    assert_eq!(gained, again);
    assert!(
        gained.iter().any(|&(_, rank)| rank == VETERAN),
        "no unit was promoted in the battle: {gained:?}"
    );
}

#[test]
fn the_state_hash_covers_the_experience() {
    let scenario = |edit: fn(&mut Experience)| {
        let mut app = world();
        let tank = spawn(&mut app, P0, "tank", 10, 10);
        tick(&mut app, Vec::new());
        let world = app.world_mut();
        let mut query = world.query::<(&SimId, &mut Experience)>();
        for (id, mut experience) in query.iter_mut(world) {
            if *id == tank {
                edit(&mut experience);
            }
        }
        hash(&mut app)
    };
    let base = scenario(|_| {});
    assert_eq!(base, scenario(|_| {}));
    for (name, edit) in [
        ("value", (|e| e.value = 1) as fn(&mut Experience)),
        ("rank", |e| e.rank = 1),
        ("self heal", |e| e.self_heal = true),
        ("ranked", |e| e.ranked.reload_pm = 600),
        ("bonus", |e| e.bonus.armour_pm = 1500),
    ] {
        assert_ne!(scenario(edit), base, "{name} is not hashed");
    }
    // An untrainable unit has no experience, and the hash of its world has no such part.
    let mut plain = world();
    spawn(&mut plain, P0, "conscript", 10, 10);
    tick(&mut plain, Vec::new());
    let mut query = plain.world_mut().query::<&Experience>();
    assert_eq!(query.iter(plain.world()).count(), 0);
}

// --- Integration: an elite unit in a long battle (advisory review 2, tests P1-3) --------------------

/// A tank of P0 earns both ranks by its own kills and then fights on for more than 1,500 ticks:
/// four damaged heavy tanks of P1 that stand switched off within its reach (each worth 650
/// credits; elite at 2,400), then, one after the other, three tanks of P1 that are ordered to
/// advance on it and fight, with quiet stretches between them. Returns the state hash of every tick, the
/// ranks gained, the weapons the tank fired after it became elite, its hit points at the end
/// of each duel and after the quiet stretch that follows it, and whether it lives.
#[allow(clippy::type_complexity)]
fn elite_battle(
    seed: u64,
) -> (
    Vec<u64>,
    Vec<(SimId, u8)>,
    Vec<String>,
    Vec<(i32, i32)>,
    bool,
) {
    let mut app = world_of(definitions(), seed);
    let tank = spawn(&mut app, P0, "tank", 10, 10);
    for n in 0..4 {
        let victim = spawn(&mut app, P1, "heavy", 14, 8 + n);
        app.world_mut()
            .resource_mut::<sim::SpecialState>()
            .disabled
            .insert(victim, u64::MAX);
        set_hp(&mut app, victim, 30);
    }
    let mut hashes = Vec::new();
    let mut gained = Vec::new();
    let mut elite_shots = Vec::new();
    let mut step = |app: &mut App, commands: Vec<sim::IssuedCommand>| {
        // Elite before the tick: the shot that makes it elite is still its cannon's.
        let elite = rank(app, tank) == ELITE;
        let events = tick(app, commands);
        hashes.push(hash(app));
        assert_cells(app, "the battle of the elite tank");
        gained.extend(ranks_gained(&events));
        if elite {
            let definitions = defs(app);
            elite_shots.extend(
                shots(&events, tank)
                    .into_iter()
                    .map(|(_, weapon)| definitions.weapon(weapon).expect("weapon").id.clone()),
            );
        }
        events
    };
    // The four kills, on its own (it fires at what is in its reach).
    for _ in 0..150 {
        step(&mut app, Vec::new());
    }
    assert_eq!(units_of(&mut app, P1, "heavy"), []);
    // Three duels and the quiet after each.
    let mut healed = Vec::new();
    for n in 0..3 {
        let enemy = spawn(&mut app, P1, "tank", 20, 6 + 4 * n);
        step(&mut app, Vec::new());
        let advance = issue(
            P1,
            Command::AttackMove {
                units: vec![enemy],
                target: CellPos::new(11, 10),
            },
        );
        let events = step(&mut app, vec![advance]);
        assert!(rejections(&events).is_empty(), "{events:?}");
        let mut ticks = 0;
        while alive(&mut app, enemy) && alive(&mut app, tank) {
            step(&mut app, Vec::new());
            ticks += 1;
            assert!(ticks < 600, "duel {n} does not end");
        }
        if !alive(&mut app, tank) {
            break;
        }
        let after_duel = hp(&mut app, tank).expect("the tank");
        for _ in 0..300 {
            step(&mut app, Vec::new());
        }
        healed.push((after_duel, hp(&mut app, tank).expect("the tank")));
    }
    let lives = alive(&mut app, tank);
    (hashes, gained, elite_shots, healed, lives)
}

#[test]
fn a_unit_that_became_elite_by_its_kills_fights_on_with_all_that_its_rank_gives() {
    let (hashes, gained, elite_shots, healed, lives) = elite_battle(5);
    // Both ranks by its own kills (the third heavy tank makes 1,950 credits, the fourth
    // 2,600: veteran at 1,200, elite at 2,400), and nobody else is promoted.
    assert_eq!(gained.len(), 2, "{gained:?}");
    assert_eq!(
        gained.iter().map(|&(_, rank)| rank).collect::<Vec<_>>(),
        [VETERAN, ELITE]
    );
    assert_eq!(gained[0].0, gained[1].0);
    assert!(hashes.len() > 1_500, "{} ticks", hashes.len());
    // Elite, it fires its elite weapon and nothing else, in volleys of three.
    assert!(elite_shots.len() >= 9, "{elite_shots:?}");
    assert!(elite_shots.iter().all(|weapon| weapon == "autocannon"));
    // It wins its three duels (a tank of its own type and cost each: 300 hit points and a
    // cannon of 40, of which stronger armour lets 26 through), and in the quiet after each it
    // heals itself: a hit point every 5 ticks, 60 in 300 ticks, up to its 300.
    assert!(lives);
    assert_eq!(healed.len(), 3);
    for &(after_duel, after_quiet) in &healed {
        assert!(after_duel < 300, "it was hit: {healed:?}");
        assert_eq!(after_quiet, (after_duel + 60).min(300), "{healed:?}");
    }
    // The same battle twice: the same state at every tick.
    let again = elite_battle(5);
    assert_eq!(hashes, again.0);
    assert_eq!(gained, again.1);
    assert_eq!(healed, again.3);
}

// --- Regression: advisory review 2, sim, suspected item 1 (docs/DECISIONS.md R56) -------------------

#[test]
fn a_crusher_gains_nothing_for_driving_over_what_was_destroyed_already() {
    // A unit whose hit points are gone stays on the grid until `Cleanup`. A crusher that drove
    // over it in the `Movement` stage of that tick was recorded as its killer, so the victim's
    // cost was gained for a unit it had not destroyed (and twice, by two units, when a missile
    // had destroyed it). Here a nuclear strike, which has no killer, takes the recruit's last
    // hit points in the tick in which the roller drives into its cell.
    let scene = |strike: bool| {
        let mut app = world();
        let roller = spawn(&mut app, P0, "roller", 10, 10);
        let recruit = spawn(&mut app, P1, "recruit", 11, 10);
        app.world_mut()
            .resource_mut::<sim::SpecialState>()
            .disabled
            .insert(recruit, u64::MAX);
        let silo = place(&mut app, P0, "silo", 2, 2);
        ticks(&mut app, 25);
        set_hp(&mut app, recruit, 20);
        let mut commands = vec![move_to(P0, &[roller], 13, 10)];
        if strike {
            commands.insert(
                0,
                issue(
                    P0,
                    Command::Superweapon {
                        building: silo,
                        target: CellPos::new(11, 13),
                        units: Vec::new(),
                    },
                ),
            );
        }
        let mut events = tick(&mut app, commands);
        assert!(rejections(&events).is_empty(), "{events:?}");
        let crushed_at_once = events
            .iter()
            .any(|event| matches!(event, SimEvent::UnitCrushed { unit, .. } if *unit == recruit));
        events.extend(ticks(&mut app, 40));
        assert!(!alive(&mut app, recruit));
        (
            crushed_at_once,
            experience(&mut app, roller).expect("experience").value,
        )
    };
    // Control: the roller alone crushes it in the tick of its order and gains its 100.
    assert_eq!(scene(false), (true, 100));
    // Struck first in the same tick, it is driven over all the same, for nothing.
    assert_eq!(scene(true), (true, 0));
}
