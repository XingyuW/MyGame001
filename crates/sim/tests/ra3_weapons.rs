//! RA3: weapons (docs/RA2_PARITY_PLAN.md section 4.4 and the damage states of section 4.1;
//! docs/DECISIONS.md R33, R34): the choice between the primary and the secondary weapon by the
//! target's layer, place and armour, the targets a type never attacks, burst, minimum range, the
//! air range bonus, scatter, the damage states and the slower damaged vehicle. The tests run on
//! the RA3 fixture (`tests/fixtures/data/ra3`, invariant I10) through `CommandQueue` and
//! `SimSchedule`.

mod ra3_common;

use bevy_app::App;
use ra3_common::*;
use sim::{
    AttackEnd, CellPos, Command, Condition, DamageState, Projectiles, SimEvent, SimId, SimRng,
    SpecialState, TargetTraits, damage_state, state_hash, weapon_against,
};

fn rejected(events: &[SimEvent]) -> bool {
    events
        .iter()
        .any(|event| matches!(event, SimEvent::CommandRejected { .. }))
}

fn ended(events: &[SimEvent], unit: SimId) -> Option<AttackEnd> {
    events.iter().find_map(|event| match event {
        SimEvent::AttackEnded {
            unit: who, reason, ..
        } if *who == unit => Some(*reason),
        _ => None,
    })
}

/// Keeps `unit` from firing (scenario setup: a target that must not shoot back).
fn silence(app: &mut App, unit: SimId) {
    app.world_mut()
        .resource_mut::<SpecialState>()
        .disabled
        .insert(unit, u64::MAX);
}

// --- The choice of the weapon -------------------------------------------------------------------------

#[test]
fn the_secondary_weapon_engages_what_the_primary_cannot() {
    // The heavy tank's big cannon engages the ground only, its missile aircraft only.
    let mut app = world();
    let heavy = spawn(&mut app, P0, "heavy", 10, 10);
    let aircraft = spawn(&mut app, P1, "gunship", 13, 10);
    silence(&mut app, aircraft);
    let events = ticks(&mut app, 2);
    assert_eq!(
        shots(&events, heavy),
        [(aircraft, weapon_kind(&app, "sam"))]
    );
    let mut app = world();
    let heavy = spawn(&mut app, P0, "heavy", 10, 10);
    let tank = spawn(&mut app, P1, "allied_mcv", 13, 10);
    let events = ticks(&mut app, 2);
    assert_eq!(
        shots(&events, heavy),
        [(tank, weapon_kind(&app, "big_cannon"))]
    );
    assert_eq!(hp(&mut app, tank), Some(600 - 60));
    // With both in range it fires at the nearer one with the weapon for it, and one timer
    // serves both weapons.
    let mut app = world();
    let heavy = spawn(&mut app, P0, "heavy", 10, 10);
    let aircraft = spawn(&mut app, P1, "gunship", 14, 10);
    silence(&mut app, aircraft);
    let tank = spawn(&mut app, P1, "allied_mcv", 12, 10);
    let events = ticks(&mut app, 25);
    let fired = shots(&events, heavy);
    assert_eq!(fired.len(), 1, "the big cannon reloads for 30 ticks");
    assert_eq!(fired[0], (tank, weapon_kind(&app, "big_cannon")));
}

#[test]
fn a_weapon_for_the_ground_never_fires_at_an_aircraft_and_the_reverse() {
    // A tank (cannon: ground only) and an aircraft in its range: no shot, and an order ends at
    // once without one.
    let mut app = world();
    let tank = spawn(&mut app, P0, "tank", 10, 10);
    let aircraft = spawn(&mut app, P1, "gunship", 12, 10);
    silence(&mut app, aircraft);
    let events = ticks(&mut app, 30);
    assert!(shots(&events, tank).is_empty());
    assert_eq!(weapon(&mut app, tank).target, None);
    let mut events = tick(&mut app, vec![attack(P0, tank, aircraft)]);
    events.extend(ticks(&mut app, 30));
    assert!(
        !rejected(&events),
        "the order is valid; it just cannot be carried out"
    );
    assert_eq!(ended(&events, tank), Some(AttackEnd::NoDamage));
    assert!(shots(&events, tank).is_empty());
    assert_eq!(
        mobile(&mut app, tank).cell,
        CellPos::new(10, 10),
        "it did not move"
    );
    assert_eq!(hp(&mut app, aircraft), Some(200));
    // The sentry (a missile for aircraft only) and a vehicle beside it: no shot; an aircraft:
    // shots.
    let mut app = world();
    let sentry = place(&mut app, P0, "sentry", 10, 10);
    spawn(&mut app, P1, "allied_mcv", 12, 10);
    assert!(shots(&ticks(&mut app, 30), sentry).is_empty());
    let aircraft = spawn(&mut app, P1, "gunship", 12, 12);
    silence(&mut app, aircraft);
    let events = ticks(&mut app, 3);
    assert_eq!(
        shots(&events, sentry),
        [(aircraft, weapon_kind(&app, "sam"))]
    );
}

#[test]
fn a_target_at_one_percent_is_attacked_on_order_only() {
    // The sniper's needle deals 1 % to plate. With a rules file the sniper never engages a
    // vehicle on its own, but an order makes it fire, shot after shot.
    let mut app = world();
    let sniper = spawn(&mut app, P0, "sniper", 10, 10);
    let vehicle = spawn(&mut app, P1, "allied_mcv", 14, 10);
    let events = ticks(&mut app, 60);
    assert!(shots(&events, sniper).is_empty());
    let mut events = tick(&mut app, vec![attack(P0, sniper, vehicle)]);
    events.extend(ticks(&mut app, 90));
    assert_eq!(shots(&events, sniper).len(), 3, "a shot every 40 ticks");
    assert_eq!(ended(&events, sniper), None, "the order goes on");
    // 60 at 1 %: 0.6, raised to 1 (equation 2).
    assert_eq!(hp(&mut app, vehicle), Some(600 - 3));
    // A soldier is engaged at once: 60 at 200 %.
    let mut app = world();
    let sniper = spawn(&mut app, P0, "sniper", 10, 10);
    let soldier = spawn(&mut app, P1, "engineer", 14, 10);
    let events = ticks(&mut app, 2);
    assert_eq!(shots(&events, sniper).len(), 1);
    assert_eq!(hp(&mut app, soldier), None, "120 against 60 hit points");
}

#[test]
fn targets_on_and_under_water_follow_the_types_targeting() {
    // A coast: land up to x = 19, water from x = 20.
    let coast = |app: &mut App| flood(app, (20, 0), (39, 39));
    let none: [(SimId, sim::WeaponKind); 0] = [];
    // The boat's cannon engages a boat; against a submarine the boat uses its depth charge
    // (underwater targets with the secondary weapon).
    let mut app = world();
    coast(&mut app);
    let boat = spawn(&mut app, P0, "boat", 22, 10);
    let enemy_boat = spawn(&mut app, P1, "boat", 25, 10);
    silence(&mut app, enemy_boat);
    let events = ticks(&mut app, 2);
    assert_eq!(
        shots(&events, boat),
        [(enemy_boat, weapon_kind(&app, "cannon"))]
    );
    let mut app = world();
    coast(&mut app);
    let boat = spawn(&mut app, P0, "boat", 22, 10);
    let sub = spawn(&mut app, P1, "sub", 25, 10);
    silence(&mut app, sub);
    let events = ticks(&mut app, 2);
    assert_eq!(
        shots(&events, boat),
        [(sub, weapon_kind(&app, "depth_charge"))]
    );
    assert_eq!(hp(&mut app, sub), Some(300 - 40));

    // A tank on the shore engages the boat, and never the submarine: no shot on its own, and
    // an order ends at once.
    let mut app = world();
    coast(&mut app);
    let tank = spawn(&mut app, P0, "tank", 19, 10);
    let sub = spawn(&mut app, P1, "sub", 21, 10);
    silence(&mut app, sub);
    assert_eq!(shots(&ticks(&mut app, 30), tank), none);
    let mut events = tick(&mut app, vec![attack(P0, tank, sub)]);
    events.extend(ticks(&mut app, 5));
    assert_eq!(ended(&events, tank), Some(AttackEnd::NoDamage));
    assert_eq!(shots(&events, tank), none);
    let enemy_boat = spawn(&mut app, P1, "boat", 21, 12);
    silence(&mut app, enemy_boat);
    let events = ticks(&mut app, 3);
    assert_eq!(
        shots(&events, tank),
        [(enemy_boat, weapon_kind(&app, "cannon"))]
    );

    // The submarine attacks nothing on land, and the boat beside it at once.
    let mut app = world();
    coast(&mut app);
    let sub = spawn(&mut app, P0, "sub", 20, 10);
    let on_land = spawn(&mut app, P1, "hovercraft", 18, 10);
    assert_eq!(shots(&ticks(&mut app, 30), sub), none);
    let mut events = tick(&mut app, vec![attack(P0, sub, on_land)]);
    events.extend(ticks(&mut app, 5));
    assert_eq!(ended(&events, sub), Some(AttackEnd::NoDamage));
    // The same amphibious hull afloat is a target on water: the torpedo is fired.
    let afloat = spawn(&mut app, P1, "hovercraft", 22, 12);
    let events = ticks(&mut app, 3);
    assert_eq!(shots(&events, sub), [(afloat, weapon_kind(&app, "torp"))]);

    // The sniper attacks nothing on water at all, whatever the armour.
    let mut app = world();
    coast(&mut app);
    let sniper = spawn(&mut app, P0, "sniper", 19, 10);
    let enemy_boat = spawn(&mut app, P1, "boat", 21, 10);
    silence(&mut app, enemy_boat);
    assert_eq!(shots(&ticks(&mut app, 60), sniper), none);
    let mut events = tick(&mut app, vec![attack(P0, sniper, enemy_boat)]);
    events.extend(ticks(&mut app, 5));
    assert_eq!(ended(&events, sniper), Some(AttackEnd::NoDamage));

    // The squid uses its second weapon against organic targets and its first against others.
    let mut app = world();
    coast(&mut app);
    let squid = spawn(&mut app, P0, "squid", 22, 10);
    let other_squid = spawn(&mut app, P1, "squid", 25, 10);
    silence(&mut app, other_squid);
    let events = ticks(&mut app, 2);
    assert_eq!(
        shots(&events, squid),
        [(other_squid, weapon_kind(&app, "depth_charge"))]
    );
    let mut app = world();
    coast(&mut app);
    let squid = spawn(&mut app, P0, "squid", 22, 10);
    let sub = spawn(&mut app, P1, "sub", 25, 10);
    silence(&mut app, sub);
    let events = ticks(&mut app, 2);
    assert_eq!(shots(&events, squid), [(sub, weapon_kind(&app, "torp"))]);
}

#[test]
fn the_rule_of_the_choice_covers_every_targeting_code() {
    // The pure rule (`weapon_against`), for a type with the cannon and the depth charge.
    let definitions = definitions();
    let boat = definitions
        .unit_combat(definitions.unit_kind("boat").expect("boat"))
        .expect("terms");
    let (cannon, charge) = (
        definitions.weapon_kind("cannon").expect("cannon"),
        definitions
            .weapon_kind("depth_charge")
            .expect("depth_charge"),
    );
    let hull = definitions.armour_class("hull").expect("hull");
    let flesh = definitions.armour_class("flesh").expect("flesh");
    let against = |naval: u8, land: u8, traits: TargetTraits| {
        let mut terms = boat;
        terms.naval_targeting = naval;
        terms.land_targeting = land;
        weapon_against(&definitions, &terms, terms.weapons(false), &traits)
    };
    let ship = TargetTraits {
        armour: hull,
        on_water: true,
        ..TargetTraits::default()
    };
    let submarine = TargetTraits {
        underwater: true,
        ..ship
    };
    let creature = TargetTraits {
        organic: true,
        ..submarine
    };
    let (first, second) = (Some((cannon, 100)), Some((charge, 100)));
    // (code, a ship, a submarine, an organic submarine)
    for (code, on_ship, on_submarine, on_creature) in [
        (0, first, None, None),
        (1, first, second, second),
        (2, None, first, first),
        (3, first, first, second),
        (4, second, second, second),
        (5, first, first, first),
        (6, None, None, None),
    ] {
        assert_eq!(against(code, 0, ship), on_ship, "code {code}, a ship");
        assert_eq!(
            against(code, 0, submarine),
            on_submarine,
            "code {code}, a submarine"
        );
        assert_eq!(
            against(code, 0, creature),
            on_creature,
            "code {code}, a creature"
        );
    }
    // On land: 0 both weapons in order, 1 none, 2 the secondary only. The depth charge does
    // nothing to flesh: a shot that does nothing is chosen only where no other weapon may fire.
    let soldier = TargetTraits {
        armour: flesh,
        ..TargetTraits::default()
    };
    assert_eq!(against(0, 0, soldier), Some((cannon, 50)));
    assert_eq!(against(0, 1, soldier), None);
    assert_eq!(against(0, 2, soldier), Some((charge, 0)));
    // An aircraft: neither weapon engages the air, whatever the codes.
    let aircraft = TargetTraits {
        airborne: true,
        ..soldier
    };
    assert_eq!(against(5, 0, aircraft), None);
    // A wall is damaged only by a warhead for walls: the cannon's shell is one, the torpedo of
    // the depth charge is not, and against concrete it is harmless anyway.
    let concrete = definitions.armour_class("concrete").expect("concrete");
    let wall = TargetTraits {
        armour: concrete,
        wall: true,
        ..TargetTraits::default()
    };
    assert_eq!(against(0, 0, wall), Some((cannon, 100)));
    assert_eq!(against(0, 2, wall), Some((charge, 0)));
}

// --- Burst, minimum range, air range bonus ------------------------------------------------------------

/// Makes `unit` elite as RA4 will: its weapons are those of its type at elite rank.
fn promote(app: &mut App, unit: SimId, kind: &str) {
    let definitions = defs(app);
    let terms = definitions
        .unit_combat(definitions.unit_kind(kind).expect("kind"))
        .expect("terms");
    let [primary, secondary] = terms.weapons(true);
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &mut sim::Weapon)>();
    for (id, mut weapon) in query.iter_mut(world) {
        if *id == unit {
            weapon.kind = primary.expect("an elite primary weapon");
            weapon.secondary = secondary;
        }
    }
}

/// The ticks (counted from the first call) in which `unit` fired during the next `count` ticks.
fn fire_ticks(app: &mut App, unit: SimId, count: u32) -> Vec<u32> {
    (0..count)
        .filter(|_| !shots(&tick(app, Vec::new()), unit).is_empty())
        .collect()
}

#[test]
fn a_burst_fires_its_shots_five_ticks_apart_and_then_reloads() {
    // The elite heavy tank's autocannon: bursts of 3, a reload of 30 ticks.
    let mut app = world();
    let heavy = spawn(&mut app, P0, "heavy", 10, 10);
    promote(&mut app, heavy, "heavy");
    let target = spawn(&mut app, P1, "allied_mcv", 13, 10);
    let fired = fire_ticks(&mut app, heavy, 90);
    assert_eq!(fired, [1, 6, 11, 41, 46, 51, 81, 86]);
    assert_eq!(sim::BURST_GAP, 5);
    // 8 shots of 20 against plate.
    assert_eq!(hp(&mut app, target), Some(600 - 8 * 20));
    assert_eq!(
        weapon(&mut app, heavy).burst_left,
        1,
        "one shot of the third volley is left"
    );
    // Its missile is still its weapon against aircraft at elite rank.
    let aircraft = spawn(&mut app, P1, "gunship", 12, 12);
    silence(&mut app, aircraft);
    assert_eq!(
        weapon(&mut app, heavy).secondary,
        Some(weapon_kind(&app, "sam"))
    );
}

#[test]
fn a_volley_whose_next_shot_cannot_be_fired_ends_and_the_weapon_reloads() {
    // The target dies with the first shot of the volley: no second shot is due anywhere, the
    // volley ends when its next shot would have been fired, and the reload starts then.
    let mut app = world();
    let heavy = spawn(&mut app, P0, "heavy", 10, 10);
    promote(&mut app, heavy, "heavy");
    let target = spawn(&mut app, P1, "allied_mcv", 13, 10);
    set_hp(&mut app, target, 20);
    assert_eq!(fire_ticks(&mut app, heavy, 4), [1]);
    assert_eq!(hp(&mut app, target), None);
    assert_eq!(weapon(&mut app, heavy).burst_left, 2);
    ticks(&mut app, 3);
    let after = weapon(&mut app, heavy);
    assert_eq!((after.burst_left, after.cooldown), (0, 30));
    // A new target is fired at when the reload is over (in the 30th tick from here), with a
    // whole volley.
    let next = spawn(&mut app, P1, "allied_mcv", 13, 10);
    let fired = fire_ticks(&mut app, heavy, 45);
    assert_eq!(fired, [29, 34, 39]);
    assert_eq!(hp(&mut app, next), Some(600 - 60));
}

#[test]
fn a_weapon_does_not_fire_inside_its_minimum_range() {
    // The mortar: a minimum range of 2 cells (512 subcells), a range of 6.
    let mut app = world();
    let mortar = spawn(&mut app, P0, "mortar", 10, 10);
    let close = spawn(&mut app, P1, "allied_mcv", 11, 10);
    assert!(shots(&ticks(&mut app, 60), mortar).is_empty());
    assert_eq!(weapon(&mut app, mortar).target, None);
    // Exactly at the minimum range it fires.
    let at_limit = spawn(&mut app, P1, "allied_mcv", 10, 12);
    let events = ticks(&mut app, 3);
    assert_eq!(
        shots(&events, mortar).first().map(|shot| shot.0),
        Some(at_limit)
    );
    // Ordered to attack the vehicle beside it, it moves away until it may fire, and fires.
    let mut app = world();
    let mortar = spawn(&mut app, P0, "mortar", 10, 10);
    let close = {
        let _ = close;
        spawn(&mut app, P1, "allied_mcv", 11, 10)
    };
    ticks(&mut app, 2);
    tick(&mut app, vec![attack(P0, mortar, close)]);
    let mut fired_from = None;
    for _ in 0..200 {
        let events = tick(&mut app, Vec::new());
        if !shots(&events, mortar).is_empty() {
            fired_from = Some(position(&mut app, mortar));
            break;
        }
    }
    let from = fired_from.expect("the mortar fired");
    let target = position(&mut app, close);
    let (dx, dy) = (i64::from(from.x - target.x), i64::from(from.y - target.y));
    assert!(!sim::inside_min_range(dx, dy, 512), "from {from:?}");
    assert!(sim::in_range(dx, dy, 1536), "from {from:?}");
    assert_ne!(mobile(&mut app, mortar).cell, CellPos::new(10, 10));
    // The boundary of the rule itself.
    assert!(sim::inside_min_range(511, 0, 512));
    assert!(!sim::inside_min_range(512, 0, 512));
    assert!(!sim::inside_min_range(0, 0, 0), "no minimum range");
}

#[test]
fn the_air_range_bonus_lengthens_only_the_shot_at_an_aircraft() {
    // The flak car: flak burst against aircraft (6 cells and a bonus of 2), flak shell against
    // the ground (5 cells, no bonus). It sees 8 cells.
    let mut app = world();
    let car = spawn(&mut app, P0, "flak_car", 10, 10);
    let aircraft = spawn(&mut app, P1, "gunship", 18, 10);
    silence(&mut app, aircraft);
    let events = ticks(&mut app, 2);
    assert_eq!(
        shots(&events, car),
        [(aircraft, weapon_kind(&app, "flak_burst"))]
    );
    // A vehicle 6 cells away is beyond the shell's 5 cells: the bonus is not for the ground.
    let mut app = world();
    let car = spawn(&mut app, P0, "flak_car", 10, 10);
    spawn(&mut app, P1, "allied_mcv", 16, 10);
    assert!(shots(&ticks(&mut app, 30), car).is_empty());
    let near = spawn(&mut app, P1, "allied_mcv", 15, 10);
    let events = ticks(&mut app, 3);
    assert_eq!(
        shots(&events, car).first(),
        Some(&(near, weapon_kind(&app, "flak_shell")))
    );
}

// --- Scatter ----------------------------------------------------------------------------------------

/// The impacts of 200 ticks of a flak car of P0 at (10, 10) firing at an aircraft `cells`
/// cells east of it, in a world with the seed `seed`, and the aircraft's hit points afterwards.
fn flak_run(seed: u64, cells: i32) -> (Vec<sim::SubPos>, i32) {
    let mut app = world_of(definitions(), seed);
    spawn(&mut app, P0, "flak_car", 10, 10);
    let aircraft = spawn(&mut app, P1, "gunship", 10 + cells, 10);
    silence(&mut app, aircraft);
    set_hp(&mut app, aircraft, 100_000);
    let events = ticks(&mut app, 200);
    let points = impacts(&events)
        .into_iter()
        .map(|(at, air)| {
            assert!(air, "flak against an aircraft bursts in the air");
            at
        })
        .collect();
    (points, hp(&mut app, aircraft).expect("the aircraft"))
}

#[test]
fn an_inaccurate_weapon_lands_within_its_scatter_of_the_target() {
    // The flak burst scatters up to 512 subcells at its full range of 1536, and in proportion
    // nearer: at 6 cells up to 512, at 3 cells up to 256.
    for (cells, radius) in [(6, 512), (3, 256)] {
        let (points, left) = flak_run(7, cells);
        assert_eq!(points.len(), 10, "a shot every 20 ticks");
        let target = sim::SubPos::centre_of(CellPos::new(10 + cells, 10));
        let mut farthest = 0;
        for at in &points {
            let (dx, dy) = (i64::from(at.x - target.x), i64::from(at.y - target.y));
            assert!(sim::in_range(dx, dy, radius), "{at:?} at {cells} cells");
            farthest = farthest.max(dx * dx + dy * dy);
        }
        assert!(points.iter().any(|at| *at != points[0]), "the shots differ");
        assert!(farthest > i64::from(radius / 4).pow(2), "they spread out");
        // The aircraft takes the flak's damage by its distance from each burst: at most 30
        // (100 % against plate) a shot, and nothing from a burst more than a cell away.
        let lost = 100_000 - left;
        assert!((0..=300).contains(&lost), "{lost}");
        let expected: i32 = points
            .iter()
            .map(|at| {
                let (dx, dy) = (i64::from(at.x - target.x), i64::from(at.y - target.y));
                let distance = ((dx * dx + dy * dy) as u64).isqrt();
                sim::splash_damage(30, 100, 256, 50, distance)
            })
            .sum();
        assert_eq!(lost, expected);
    }
    // The same seed gives the same shots, another seed others (the draws come from `SimRng`).
    assert_eq!(flak_run(7, 6), flak_run(7, 6));
    assert_ne!(flak_run(7, 6).0, flak_run(8, 6).0);
}

#[test]
fn a_scattered_shell_lands_where_it_was_aimed_and_only_inaccurate_weapons_draw() {
    // The flak shell flies (128 subcells a tick) and scatters up to 256 at its range of 1280.
    let mut app = world();
    let car = spawn(&mut app, P0, "flak_car", 10, 10);
    let vehicle = spawn(&mut app, P1, "allied_mcv", 15, 10);
    let rng_before = app.world().resource::<SimRng>().clone().next_u64();
    let events = ticks(&mut app, 2);
    assert_eq!(shots(&events, car).len(), 1);
    let shell = *app
        .world()
        .resource::<Projectiles>()
        .iter()
        .next()
        .expect("a shell in flight");
    assert!(shell.scattered && !shell.air);
    assert_eq!(shell.flight, 10);
    let target = sim::SubPos::centre_of(CellPos::new(15, 10));
    let (dx, dy) = (
        i64::from(shell.aim.x - target.x),
        i64::from(shell.aim.y - target.y),
    );
    assert!(sim::in_range(dx, dy, 256), "{:?}", shell.aim);
    assert_ne!(
        app.world().resource::<SimRng>().clone().next_u64(),
        rng_before,
        "the shot drew from the generator"
    );
    let events = ticks(&mut app, 10);
    assert_eq!(impacts(&events), [(shell.aim, false)]);
    let distance = ((dx * dx + dy * dy) as u64).isqrt();
    assert_eq!(
        hp(&mut app, vehicle),
        Some(600 - sim::splash_damage(30, 100, 256, 50, distance))
    );
    // Accurate weapons draw nothing: a fight of tanks leaves the generator where it was.
    let mut app = world();
    spawn(&mut app, P0, "tank", 10, 10);
    spawn(&mut app, P1, "tank", 13, 10);
    let rng_before = app.world().resource::<SimRng>().clone().next_u64();
    let events = ticks(&mut app, 60);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SimEvent::WeaponFired { .. }))
    );
    assert_eq!(
        app.world().resource::<SimRng>().clone().next_u64(),
        rng_before
    );
    // The pure rule at its limits: the largest scatter stays within its radius.
    let mut rng = SimRng::new(3);
    for _ in 0..500 {
        let at = sim::scattered(&mut rng, target, 512, 1536, 1536);
        let (dx, dy) = (i64::from(at.x - target.x), i64::from(at.y - target.y));
        assert!(sim::in_range(dx, dy, 512), "{at:?}");
    }
    assert_eq!(
        sim::scattered(&mut rng, target, 512, 0, 1536),
        target,
        "point blank"
    );
}

// --- Damage states ------------------------------------------------------------------------------------

fn condition(app: &mut App, id: SimId) -> DamageState {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &Condition)>();
    query
        .iter(world)
        .find(|(other, _)| **other == id)
        .map(|(_, condition)| condition.0)
        .expect("a condition")
}

#[test]
fn damage_states_change_exactly_at_the_rules_shares_and_are_reported() {
    // The fixture's rules: yellow at or below 50 %, red at or below 25 %.
    assert_eq!(damage_state(201, 400, 50, 25), DamageState::Green);
    assert_eq!(damage_state(200, 400, 50, 25), DamageState::Yellow);
    assert_eq!(damage_state(101, 400, 50, 25), DamageState::Yellow);
    assert_eq!(damage_state(100, 400, 50, 25), DamageState::Red);
    assert_eq!(damage_state(1, 400, 50, 25), DamageState::Red);
    assert_eq!(
        damage_state(i32::MAX, i32::MAX, 100, 0),
        DamageState::Yellow
    );
    let mut app = world();
    let heavy = spawn(&mut app, P0, "heavy", 10, 10);
    let power = place(&mut app, P0, "allied_power", 20, 20);
    assert!(
        ticks(&mut app, 2)
            .iter()
            .all(|e| !matches!(e, SimEvent::DamageStateChanged { .. }))
    );
    assert_eq!(condition(&mut app, heavy), DamageState::Green);
    let changes = |events: &[SimEvent]| -> Vec<(SimId, DamageState)> {
        events
            .iter()
            .filter_map(|event| match event {
                SimEvent::DamageStateChanged { player, id, state } => {
                    assert_eq!(*player, P0);
                    Some((*id, *state))
                }
                _ => None,
            })
            .collect()
    };
    for (hit_points, state, reported) in [
        (201, DamageState::Green, false),
        (200, DamageState::Yellow, true),
        (101, DamageState::Yellow, false),
        (100, DamageState::Red, true),
        (150, DamageState::Yellow, true),
        (400, DamageState::Green, true),
    ] {
        set_hp(&mut app, heavy, hit_points);
        let events = tick(&mut app, Vec::new());
        assert_eq!(condition(&mut app, heavy), state, "at {hit_points}");
        let expected = if reported {
            vec![(heavy, state)]
        } else {
            Vec::new()
        };
        assert_eq!(changes(&events), expected, "at {hit_points}");
    }
    // Buildings have states too.
    set_hp(&mut app, power, 100);
    assert_eq!(
        changes(&tick(&mut app, Vec::new())),
        [(power, DamageState::Red)]
    );
    // The state is part of the hash.
    let mut other = world();
    let mut same = world();
    for app in [&mut other, &mut same] {
        spawn(app, P0, "heavy", 10, 10);
        ticks(app, 2);
    }
    assert_eq!(state_hash(other.world_mut()), state_hash(same.world_mut()));
    let world = other.world_mut();
    let mut query = world.query::<&mut Condition>();
    for mut condition in query.iter_mut(world) {
        condition.0 = DamageState::Red;
    }
    assert_ne!(state_hash(other.world_mut()), state_hash(same.world_mut()));
}

/// The subcells `unit` covers in 10 ticks of a move along a row, after 5 ticks under way.
fn pace(app: &mut App, unit: SimId, to: CellPos) -> i32 {
    tick(
        app,
        vec![issue(
            P0,
            Command::Move {
                units: vec![unit],
                target: to,
            },
        )],
    );
    ticks(app, 5);
    let from = position(app, unit);
    ticks(app, 10);
    let after = position(app, unit);
    assert_eq!(after.y, from.y, "a straight row");
    after.x - from.x
}

#[test]
fn a_damaged_vehicle_moves_at_three_quarters_of_its_speed() {
    // The tank's speed is 32 subcells a tick; yellow or red it is 32 - 32 / 4 = 24.
    for (hit_points, speed) in [(300, 32), (151, 32), (150, 24), (75, 24)] {
        let mut app = world();
        let tank = spawn(&mut app, P0, "tank", 5, 10);
        set_hp(&mut app, tank, hit_points);
        ticks(&mut app, 2);
        assert_eq!(
            pace(&mut app, tank, CellPos::new(30, 10)),
            10 * speed,
            "at {hit_points}"
        );
    }
    // Infantry and aircraft are not slowed.
    for (kind, speed) in [("rifleman", 16), ("gunship", 32)] {
        let mut app = world();
        let unit = spawn(&mut app, P0, kind, 5, 10);
        set_hp(&mut app, unit, 1);
        ticks(&mut app, 2);
        assert_eq!(condition(&mut app, unit), DamageState::Red);
        assert_eq!(
            pace(&mut app, unit, CellPos::new(30, 10)),
            10 * speed,
            "{kind}"
        );
    }
}

// --- Determinism --------------------------------------------------------------------------------------

/// Twenty against twenty of mixed types (vehicles, infantry, mortars, flak cars and aircraft)
/// fought for `ticks` ticks; the hash after every tick, and how many impacts were reported.
fn battle(seed: u64, count: u32) -> (Vec<u64>, usize, usize) {
    let mut app = world_of(definitions(), seed);
    let kinds = ["tank", "heavy", "flak_car", "mortar", "rifleman", "gunship"];
    for i in 0..20 {
        let kind = kinds[i % kinds.len()];
        let (column, row) = ((i / 10) as i32, (i % 10) as i32);
        spawn(&mut app, P0, kind, 12 + column, 10 + 2 * row);
        spawn(&mut app, P1, kind, 19 - column, 10 + 2 * row);
    }
    let mut hashes = Vec::new();
    let (mut bursts, mut lost) = (0, 0);
    for _ in 0..count {
        let events = tick(&mut app, Vec::new());
        bursts += impacts(&events).len();
        lost += events
            .iter()
            .filter(|event| matches!(event, SimEvent::UnitDestroyed { .. }))
            .count();
        hashes.push(state_hash(app.world_mut()));
    }
    (hashes, bursts, lost)
}

#[test]
fn a_battle_of_mixed_types_is_deterministic_and_another_seed_is_another_battle() {
    let first = battle(21, 300);
    assert_eq!(first, battle(21, 300));
    assert!(first.1 > 0, "inaccurate weapons were fired");
    assert!(first.2 > 0, "units were destroyed");
    // Scatter is drawn from the seed: another seed scatters the flak elsewhere.
    assert_ne!(first.0, battle(22, 300).0);
}

#[test]
fn the_hash_covers_the_state_of_a_volley() {
    let mut a = world();
    let mut b = world();
    let mut units = Vec::new();
    for app in [&mut a, &mut b] {
        let car = spawn(app, P0, "flak_car", 10, 10);
        spawn(app, P1, "allied_mcv", 15, 10);
        ticks(app, 2);
        units.push(car);
    }
    assert_eq!(state_hash(a.world_mut()), state_hash(b.world_mut()));
    // A volley under way, and the weapon of the last shot.
    let world = a.world_mut();
    let mut query = world.query::<(&SimId, &mut sim::Weapon)>();
    for (id, mut weapon) in query.iter_mut(world) {
        if *id == units[0] {
            weapon.burst_left = 2;
        }
    }
    assert_ne!(state_hash(a.world_mut()), state_hash(b.world_mut()));
    let world = a.world_mut();
    let mut query = world.query::<(&SimId, &mut sim::Weapon)>();
    for (id, mut weapon) in query.iter_mut(world) {
        if *id == units[0] {
            weapon.burst_left = 0;
            weapon.volley = weapon.secondary.expect("a secondary weapon");
        }
    }
    assert_ne!(state_hash(a.world_mut()), state_hash(b.world_mut()));
}

// --- The computer player's army -----------------------------------------------------------------------

#[test]
fn an_army_must_attack_the_ground_and_damage_every_building_with_a_weapon_it_uses() {
    use sim::{DataErrorKind, load_ai};
    // The fixture's plans load: the tank's and the heavy tank's shells damage every building.
    load_ai(fixture("ai.ron"), &definitions()).unwrap_or_else(|error| panic!("{error}"));
    // A heavy tank with its missile alone attacks aircraft only: it is no army unit.
    let air_only = definitions_with(
        "heavy_air_only",
        &[(
            "weapons: [\"big_cannon\", \"sam\"], elite_weapons: [\"autocannon\", \"sam\"]",
            "weapons: [\"sam\"], elite_weapons: [\"sam\"]",
        )],
    );
    let error = load_ai(fixture("ai.ron"), &air_only).expect_err("an army of air defence");
    assert_eq!(error.field, "plans[1].army[0]", "{error}");
    match &error.kind {
        DataErrorKind::InvalidValue { found, expected } => {
            assert!(
                found.contains("heavy") && found.contains("aircraft only"),
                "{found}"
            );
            assert!(expected.contains("ground"), "{expected}");
        }
        other => panic!("{other:?}"),
    }
    // If no shell damages walls, no unit of either army can destroy the wall: a match of
    // computer players could not end.
    let no_wall_breaker = definitions_with(
        "no_wall_breaker",
        &[(
            "(id: \"shell\", cell_spread: 0, percent_at_max: 100, wall: 1,",
            "(id: \"shell\", cell_spread: 0, percent_at_max: 100, wall: 0,",
        )],
    );
    let error = load_ai(fixture("ai.ron"), &no_wall_breaker).expect_err("the wall stands");
    assert_eq!(error.field, "plans[0].army", "{error}");
    assert!(
        matches!(&error.kind, DataErrorKind::MissingEntry { id, .. } if id == "wall"),
        "{error}"
    );
    // A percentage of 1 is not enough with a rules file: a unit engages no target on its own
    // that it damages by 1 percent or less.
    let one_percent = definitions_with(
        "one_percent",
        &[
            (
                "(id: \"shell\", cell_spread: 0, percent_at_max: 100, wall: 1, wood: 1, versus: [(armour: \"flesh\", percent: 50), (armour: \"plate\", percent: 100), (armour: \"concrete\", percent: 100),",
                "(id: \"shell\", cell_spread: 0, percent_at_max: 100, wall: 1, wood: 1, versus: [(armour: \"flesh\", percent: 50), (armour: \"plate\", percent: 100), (armour: \"concrete\", percent: 1),",
            ),
            (
                "(id: \"bullet\", cell_spread: 0, percent_at_max: 100, wall: 0, wood: 0, versus: [(armour: \"flesh\", percent: 100), (armour: \"plate\", percent: 20), (armour: \"concrete\", percent: 10),",
                "(id: \"bullet\", cell_spread: 0, percent_at_max: 100, wall: 0, wood: 0, versus: [(armour: \"flesh\", percent: 100), (armour: \"plate\", percent: 20), (armour: \"concrete\", percent: 1),",
            ),
        ],
    );
    let error = load_ai(fixture("ai.ron"), &one_percent).expect_err("one percent");
    assert!(
        matches!(&error.kind, DataErrorKind::MissingEntry { kind, .. } if kind.contains("able to damage building")),
        "{error}"
    );
}
