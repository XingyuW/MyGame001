//! RA3: the single damage path (docs/RA2_PARITY_PLAN.md section 4.3; docs/DECISIONS.md R33, R35).
//!
//! Every shot, bomb and superweapon ends in an impact of a warhead at a point: its target takes
//! the damage of equation 2, and with a `cell_spread` everything of the impact's layer within it
//! takes a share that falls linearly to `percent_at_max` at the edge, through the armour table;
//! a building takes one hit per cell of its footprint within the spread. The tests run on the
//! RA3 fixture (`tests/fixtures/data/ra3`, invariant I10) through `CommandQueue` and
//! `SimSchedule`.

mod ra3_common;

use ra3_common::*;
use sim::{AttackEnd, CellPos, Command, Grid, Props, SimEvent, SimId, SpecialState, splash_damage};

/// The damage of the fixture's blast (spread 512, 25 percent at the edge) from a base damage
/// against a percentage, at a distance.
fn blast(base: i32, versus: i32, distance: u64) -> i32 {
    splash_damage(base, versus, 512, 25, distance)
}

// --- The equation ---------------------------------------------------------------------------------

#[test]
fn the_damage_of_an_impact_falls_linearly_to_the_percentage_at_the_edge() {
    // 80 through the blast against flesh (100 %): 80 at the point, 50 half way (62.5 %), 20 at
    // the edge (25 %), nothing one subcell beyond.
    assert_eq!(blast(80, 100, 0), 80);
    assert_eq!(blast(80, 100, 256), 50);
    assert_eq!(blast(80, 100, 512), 20);
    assert_eq!(blast(80, 100, 513), 0);
    // Rounded down after the armour: 80 x (51200 - 75 x 362) x 50 / 5120000 = 18.78.
    assert_eq!(blast(80, 50, 362), 18);
    // At least 1 while the warhead damages the armour at all, and 0 if it does not.
    assert_eq!(blast(1, 1, 512), 1);
    assert_eq!(blast(100, 0, 0), 0);
    assert_eq!(blast(100, -5, 0), 0);
    // 100 percent at the edge: no falloff. 0 percent: down to the minimum of 1.
    assert_eq!(splash_damage(40, 100, 256, 100, 256), 40);
    assert_eq!(splash_damage(40, 100, 256, 0, 256), 1);
    assert_eq!(splash_damage(40, 100, 256, 0, 128), 20);
    // Without a spread it is equation 2, whatever the distance.
    assert_eq!(splash_damage(25, 30, 0, 100, 0), 7);
    assert_eq!(splash_damage(25, 30, 0, 100, 9_999), 7);
    assert_eq!(
        splash_damage(25, 30, 0, 100, 0),
        sim::applied_damage(25, 30)
    );
    // The limits of the format and beyond do not overflow (invariant I12).
    assert_eq!(
        splash_damage(
            sim::MAX_DAMAGE,
            sim::MAX_VERSUS_PERCENT,
            sim::MAX_CELL_SPREAD,
            100,
            0
        ),
        sim::MAX_DAMAGE * 10
    );
    assert_eq!(
        splash_damage(i32::MAX, i32::MAX, u32::MAX, 100, 0),
        i32::MAX
    );
    assert_eq!(splash_damage(i32::MAX, 1, u32::MAX, 0, u64::MAX), 0);
}

// --- Splash -----------------------------------------------------------------------------------------

/// A mortar of P0 at (10, 10) and around its target at (15, 10) vehicles of both players at 1
/// cell, diagonally, at 2 cells and at 3 cells. Returns the mortar, the target, and the others
/// with their distance in subcells.
fn mortar_scene(app: &mut bevy_app::App) -> (SimId, SimId, Vec<(SimId, u64)>) {
    let mortar = spawn(app, P0, "mortar", 10, 10);
    let target = spawn(app, P1, "allied_mcv", 15, 10);
    let around = vec![
        (spawn(app, P1, "allied_mcv", 16, 10), 256),
        // The mortar's own side is hit like anyone.
        (spawn(app, P0, "allied_mcv", 15, 11), 256),
        (spawn(app, P1, "allied_mcv", 16, 11), 362),
        (spawn(app, P1, "allied_mcv", 17, 10), 512),
        (spawn(app, P1, "allied_mcv", 15, 13), 768),
    ];
    (mortar, target, around)
}

#[test]
fn a_shell_with_a_spread_damages_everything_within_it_by_its_distance() {
    let mut app = world();
    let (mortar, target, around) = mortar_scene(&mut app);
    // The mortar sees its targets after the first tick and fires in the second; its bomb flies
    // 5 cells at 128 subcells a tick: 10 ticks.
    let events = ticks(&mut app, 2);
    assert_eq!(shots(&events, mortar).len(), 1);
    assert_eq!(shots(&events, mortar)[0].0, target);
    ticks(&mut app, 9);
    assert_eq!(
        hp(&mut app, target),
        Some(600),
        "the bomb is still in flight"
    );
    ticks(&mut app, 1);
    // 80 against plate (50 %): 40 on the target, and by the distance on the others.
    assert_eq!(hp(&mut app, target), Some(600 - 40));
    for (id, distance) in around {
        let expected = blast(80, 50, distance);
        assert_eq!(hp(&mut app, id), Some(600 - expected), "at {distance}");
    }
    assert_eq!(
        [256, 362, 512, 768].map(|distance| blast(80, 50, distance)),
        [25, 18, 10, 0]
    );
}

#[test]
fn the_spread_ends_exactly_at_its_radius() {
    // With a spread of 511 subcells the vehicle 2 cells (512) from the impact is out of it.
    let mut app = world_with(
        "spread_511",
        &[(
            "(id: \"blast\", cell_spread: 512,",
            "(id: \"blast\", cell_spread: 511,",
        )],
    );
    let (_, target, around) = mortar_scene(&mut app);
    ticks(&mut app, 12);
    assert_eq!(hp(&mut app, target), Some(560));
    let at = |distance: u64| {
        around
            .iter()
            .find(|(_, d)| *d == distance)
            .expect("there")
            .0
    };
    assert_eq!(
        hp(&mut app, at(512)),
        Some(600),
        "one subcell beyond the spread"
    );
    assert_eq!(
        hp(&mut app, at(362)),
        Some(600 - splash_damage(80, 50, 511, 25, 362))
    );
}

#[test]
fn a_building_takes_one_hit_for_every_cell_of_its_footprint_within_the_spread() {
    // A power building of 2 x 2 cells at (15, 10): the bomb lands on its nearest cell, and
    // the three others are 1 cell, 1 cell and a diagonal from there. Concrete: 100 %.
    let mut app = world();
    let mortar = spawn(&mut app, P0, "mortar", 10, 10);
    let power = place(&mut app, P1, "soviet_power", 15, 10);
    let events = ticks(&mut app, 2);
    assert_eq!(
        shots(&events, mortar),
        [(power, weapon_kind(&app, "mortar_bomb"))]
    );
    // 1152 subcells to the footprint: 9 ticks of flight.
    ticks(&mut app, 8);
    assert_eq!(hp(&mut app, power), Some(400));
    ticks(&mut app, 1);
    let expected = blast(80, 100, 0) + 2 * blast(80, 100, 256) + blast(80, 100, 362);
    assert_eq!(expected, 80 + 50 + 50 + 37);
    assert_eq!(hp(&mut app, power), Some(400 - expected));
    // A warhead without a spread hits it once: the tank's shell deals 40.
    let mut app = world();
    spawn(&mut app, P0, "tank", 10, 10);
    let power = place(&mut app, P1, "soviet_power", 13, 10);
    ticks(&mut app, 2);
    assert_eq!(hp(&mut app, power), Some(360));
}

#[test]
fn an_impact_in_the_air_spares_the_ground_and_one_on_the_ground_the_air() {
    // The heavy tank's missile (the flak warhead: 1 cell of spread, 50 % at the edge) hits an
    // aircraft: the aircraft one cell beside it is caught, the vehicle under it is not.
    let mut app = world();
    let heavy = spawn(&mut app, P0, "heavy", 10, 10);
    let gunship = spawn(&mut app, P1, "gunship", 13, 10);
    let beside = spawn(&mut app, P1, "gunship", 14, 10);
    let below = spawn(&mut app, P1, "allied_mcv", 13, 10);
    let events = ticks(&mut app, 2);
    assert_eq!(shots(&events, heavy), [(gunship, weapon_kind(&app, "sam"))]);
    assert_eq!(hp(&mut app, gunship), Some(200 - 30));
    assert_eq!(hp(&mut app, beside), Some(200 - 15));
    assert_eq!(hp(&mut app, below), Some(600));
    // A bomb on the ground under an aircraft: the mortar's blast leaves the aircraft alone.
    let mut app = world();
    spawn(&mut app, P0, "mortar", 10, 10);
    let target = spawn(&mut app, P1, "allied_mcv", 15, 10);
    let above = spawn(&mut app, P0, "gunship", 15, 10);
    // The gunship of the mortar's owner would fire too: it is kept from it.
    app.world_mut()
        .resource_mut::<SpecialState>()
        .disabled
        .insert(above, u64::MAX);
    ticks(&mut app, 12);
    assert_eq!(hp(&mut app, target), Some(560));
    assert_eq!(hp(&mut app, above), Some(200));
}

// --- Walls and trees ----------------------------------------------------------------------------------

#[test]
fn only_a_warhead_for_walls_damages_a_wall() {
    // The rifle's bullet does not damage walls: a rifleman beside one does not fire at it, and
    // an order ends with its one shot, which does nothing.
    let mut app = world();
    let rifleman = spawn(&mut app, P0, "rifleman", 10, 10);
    let wall = place(&mut app, P1, "wall", 12, 10);
    let events = ticks(&mut app, 20);
    assert!(shots(&events, rifleman).is_empty());
    let mut events = tick(&mut app, vec![attack(P0, rifleman, wall)]);
    events.extend(ticks(&mut app, 20));
    assert_eq!(shots(&events, rifleman).len(), 1);
    assert!(events.contains(&SimEvent::AttackEnded {
        unit: rifleman,
        target: wall,
        reason: AttackEnd::NoDamage,
    }));
    assert_eq!(hp(&mut app, wall), Some(200));
    // The same bullet damages another building of the same armour (10 % of 10: 1).
    let vault = place(&mut app, P1, "vault", 12, 13);
    let events = ticks(&mut app, 3);
    assert_eq!(
        shots(&events, rifleman).first().map(|shot| shot.0),
        Some(vault)
    );
    assert!(hp(&mut app, vault).is_some_and(|left| left < 500));
    // The tank's shell does damage walls: 40 against concrete.
    let mut app = world();
    let tank = spawn(&mut app, P0, "tank", 10, 10);
    let wall = place(&mut app, P1, "wall", 12, 10);
    let events = ticks(&mut app, 2);
    assert_eq!(shots(&events, tank).len(), 1);
    assert_eq!(hp(&mut app, wall), Some(160));
}

#[test]
fn only_a_warhead_for_wood_damages_trees() {
    let mut app = world();
    let (trees, house) = (CellPos::new(13, 10), CellPos::new(13, 12));
    let rock = sim::Cell {
        passable: false,
        water: false,
        buildable: false,
        ore: 0,
    };
    let mut looks = vec![0u8; 1600];
    for (cell, look) in [(trees, 4u8 << 4), (house, 5 << 4)] {
        app.world_mut()
            .resource_mut::<Grid>()
            .set_terrain(cell, rock);
        looks[cell.y as usize * 40 + cell.x as usize] = look;
    }
    let armour = defs(&app).armour_class("concrete").expect("concrete");
    let props = Props::from_looks(app.world().resource::<Grid>(), &looks, armour);
    let (trees_id, house_id) = (
        props.id_at(trees).expect("trees"),
        props.id_at(house).expect("house"),
    );
    app.world_mut().insert_resource(props);
    let rifleman = spawn(&mut app, P0, "rifleman", 10, 10);
    let tank = spawn(&mut app, P0, "tank", 10, 12);
    ticks(&mut app, 2);
    let prop_hp = |app: &bevy_app::App, cell| {
        app.world()
            .resource::<Props>()
            .at(cell)
            .map(|prop| prop.hit_points)
    };
    // The bullet is no warhead for wood: the order on the trees ends with one useless shot.
    let mut events = tick(&mut app, vec![attack(P0, rifleman, trees_id)]);
    events.extend(ticks(&mut app, 15));
    assert_eq!(shots(&events, rifleman).len(), 1);
    assert!(events.contains(&SimEvent::AttackEnded {
        unit: rifleman,
        target: trees_id,
        reason: AttackEnd::NoDamage,
    }));
    assert_eq!(prop_hp(&app, trees), Some(60));
    // A house is no tree: the bullet deals its 10 % against the scenery's armour.
    tick(&mut app, vec![attack(P0, rifleman, house_id)]);
    ticks(&mut app, 12);
    assert!(prop_hp(&app, house).is_some_and(|left| left < 300));
    // The shell is a warhead for wood: 40 a shot fells the trees with the second.
    tick(&mut app, vec![attack(P0, tank, trees_id)]);
    ticks(&mut app, 3);
    assert_eq!(prop_hp(&app, trees), Some(20));
    let events = ticks(&mut app, 30);
    assert!(
        events.iter().any(
            |event| matches!(event, SimEvent::PropDestroyed { prop, .. } if *prop == trees_id)
        )
    );
}

// --- The area effects of the specialists and superweapons -------------------------------------------

/// Fires the nuclear strike of a silo of P0 at (20, 20), which an engineer of P0 five cells
/// away sees, after `setup` has put the victims there. Returns the events of the tick.
fn strike(app: &mut bevy_app::App) -> Vec<SimEvent> {
    let silo = place(app, P0, "silo", 2, 2);
    spawn(app, P0, "engineer", 15, 20);
    // The silo charges for 20 ticks.
    ticks(app, 25);
    let events = tick(
        app,
        vec![issue(
            P0,
            Command::Superweapon {
                building: silo,
                target: CellPos::new(20, 20),
                units: Vec::new(),
            },
        )],
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
    events
}

#[test]
fn a_nuclear_strike_goes_through_its_warhead_and_the_armour_table() {
    // The megaton: 300, a spread of 4 cells, 10 % at the edge; flesh 100 %, plate 40 %,
    // concrete 10 %.
    let megaton = |versus: i32, distance: u64| splash_damage(300, versus, 1024, 10, distance);
    let mut app = world();
    let soldier = spawn(&mut app, P1, "conscript", 20, 20);
    let tank = spawn(&mut app, P1, "tank", 21, 20);
    let far_soldier = spawn(&mut app, P1, "engineer", 21, 21);
    let heavy = spawn(&mut app, P1, "heavy", 20, 24);
    let beyond = spawn(&mut app, P1, "engineer", 20, 25);
    let aircraft = spawn(&mut app, P1, "gunship", 20, 20);
    let power = place(&mut app, P1, "soviet_power", 22, 22);
    let events = strike(&mut app);
    // A soldier at the point is gone; a tank one cell away loses less than a soldier a
    // diagonal away would: its armour takes 40 %.
    ticks(&mut app, 1);
    assert_eq!(hp(&mut app, soldier), None);
    assert_eq!(megaton(40, 256), 93);
    assert_eq!(hp(&mut app, tank), Some(300 - 93));
    assert_eq!(megaton(100, 362), 204);
    assert_eq!(hp(&mut app, far_soldier), None, "60 hit points against 204");
    // Exactly at the edge: 10 % of 300 at 40 %. One cell farther: nothing.
    assert_eq!(hp(&mut app, heavy), Some(400 - 12));
    assert_eq!(hp(&mut app, beyond), Some(60));
    // The strike is on the ground.
    assert_eq!(hp(&mut app, aircraft), Some(200));
    // The building: three of its four cells are within the spread, at 724, 923 and 923.
    assert_eq!(
        [megaton(10, 724), megaton(10, 923), megaton(10, 1086)],
        [10, 5, 0]
    );
    assert_eq!(hp(&mut app, power), Some(400 - 10 - 5 - 5));
    // The impact is reported for the interface, where it happened.
    assert_eq!(
        impacts(&events),
        [(sim::SubPos::centre_of(CellPos::new(20, 20)), false)]
    );
}

#[test]
fn an_impact_deals_at_most_the_largest_damage_of_the_rules() {
    // A strike of 20000 against flesh at the point is cut to the rules' 10000.
    let mut app = world_with(
        "huge_nuke",
        &[(
            "(id: \"nuke\", damage: 300,",
            "(id: \"nuke\", damage: 20000,",
        )],
    );
    let giant = spawn(&mut app, P1, "conscript", 20, 20);
    set_hp(&mut app, giant, 50_000);
    strike(&mut app);
    assert_eq!(hp(&mut app, giant), Some(40_000));
}

#[test]
fn a_suicide_unit_blows_up_with_its_own_weapon_where_it_stands() {
    // The martyr's bomb: 100 through the blast, at the martyr's own position.
    let mut app = world();
    let martyr = spawn(&mut app, P1, "martyr", 10, 10);
    let vehicle = spawn(&mut app, P0, "allied_mcv", 11, 10);
    let engineer = spawn(&mut app, P0, "engineer", 10, 12);
    let comrade = spawn(&mut app, P1, "engineer", 9, 10);
    let beyond = spawn(&mut app, P0, "engineer", 10, 13);
    let events = ticks(&mut app, 2);
    assert_eq!(shots(&events, martyr).len(), 1);
    assert!(events.contains(&SimEvent::UnitDestroyed {
        player: P1,
        unit: martyr
    }));
    assert_eq!(impacts(&events).len(), 1);
    assert_eq!(hp(&mut app, vehicle), Some(600 - blast(100, 50, 256)));
    assert_eq!(hp(&mut app, engineer), Some(60 - blast(100, 100, 512)));
    assert_eq!(
        hp(&mut app, comrade),
        None,
        "62 against 60 hit points, friend or not"
    );
    assert_eq!(hp(&mut app, beyond), Some(60));
    assert_eq!([blast(100, 50, 256), blast(100, 100, 512)], [31, 25]);
}

#[test]
fn a_planted_bomb_goes_off_after_the_rules_delay_through_its_warhead() {
    // The sapper plants its bomb with its first shot, which deals nothing itself; after the
    // rules' 600 ticks the bomb is an impact of the effect (120 through the blast) on its
    // carrier.
    let mut app = world();
    let sapper = spawn(&mut app, P1, "sapper", 10, 10);
    let vehicle = spawn(&mut app, P0, "allied_mcv", 11, 10);
    let events = ticks(&mut app, 2);
    assert_eq!(shots(&events, sapper).len(), 1);
    let planted = app.world().resource::<sim::SimTick>().0 - 1;
    assert_eq!(
        app.world()
            .resource::<SpecialState>()
            .bombs
            .get(&vehicle)
            .map(|bomb| bomb.0),
        Some(planted + 600)
    );
    ticks(&mut app, 598);
    assert_eq!(
        hp(&mut app, vehicle),
        Some(600),
        "no shot of the sapper deals damage"
    );
    assert_eq!(hp(&mut app, sapper), Some(100));
    let events = ticks(&mut app, 2);
    assert_eq!(impacts(&events).len(), 1);
    assert_eq!(hp(&mut app, vehicle), Some(600 - blast(120, 50, 0)));
    // The sapper stands one cell from its own bomb.
    assert_eq!(hp(&mut app, sapper), Some(100 - blast(120, 100, 256)));
}

#[test]
fn a_commandos_charge_and_a_radiation_zone_go_through_their_warheads() {
    // The commando's rifle deals 1 to concrete (10 % of 10) and its charge 150 through the
    // shell warhead: 151 a shot.
    let mut app = world();
    let commando = spawn(&mut app, P0, "commando", 10, 10);
    let power = place(&mut app, P1, "soviet_power", 12, 10);
    let events = ticks(&mut app, 2);
    assert_eq!(shots(&events, commando).len(), 1);
    assert_eq!(hp(&mut app, power), Some(400 - 151));
    // Against a unit the charge does not go off.
    let mut app = world();
    spawn(&mut app, P0, "commando", 10, 10);
    let vehicle = spawn(&mut app, P1, "allied_mcv", 12, 10);
    ticks(&mut app, 2);
    assert_eq!(hp(&mut app, vehicle), Some(600 - 2));

    // The seer's zone (2 cells, 60 ticks: three pulses) deals 15 through the fallout warhead:
    // all of it to flesh, a tenth to plate, nothing to a building, to an aircraft or to a seer.
    let mut app = world();
    let seer = spawn(&mut app, P0, "seer", 10, 10);
    let soldier = spawn(&mut app, P1, "engineer", 11, 10);
    let vehicle = spawn(&mut app, P1, "allied_mcv", 10, 12);
    let own = spawn(&mut app, P0, "engineer", 9, 10);
    let aircraft = spawn(&mut app, P1, "gunship", 10, 11);
    app.world_mut()
        .resource_mut::<SpecialState>()
        .disabled
        .insert(aircraft, u64::MAX);
    let outside = spawn(&mut app, P1, "engineer", 13, 10);
    let power = place(&mut app, P1, "soviet_power", 8, 11);
    ticks(&mut app, 2);
    let events = tick(&mut app, vec![issue(P0, Command::Deploy { unit: seer })]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::CommandRejected { .. }))
    );
    let events = ticks(&mut app, 70);
    assert!(
        impacts(&events).is_empty(),
        "a pulse is no event of its own"
    );
    assert_eq!(hp(&mut app, soldier), Some(60 - 45));
    assert_eq!(hp(&mut app, own), Some(60 - 45), "friend or foe");
    assert_eq!(hp(&mut app, vehicle), Some(600 - 3));
    assert_eq!(hp(&mut app, seer), Some(100));
    assert_eq!(hp(&mut app, aircraft), Some(200));
    assert_eq!(hp(&mut app, outside), Some(60));
    assert_eq!(hp(&mut app, power), Some(400));
}

#[test]
fn the_bolts_of_a_storm_go_through_their_warhead() {
    // The storm device strikes its target cell with a bolt every 20 ticks for 200 ticks: ten
    // impacts of 100 through the blast. Plate takes 50 %.
    let mut app = world();
    place(&mut app, P0, "allied_power", 2, 6);
    let device = place(&mut app, P0, "storm_device", 2, 2);
    spawn(&mut app, P0, "engineer", 15, 20);
    let vehicle = spawn(&mut app, P1, "allied_mcv", 20, 20);
    let near = spawn(&mut app, P1, "allied_mcv", 22, 20);
    ticks(&mut app, 2);
    app.world_mut()
        .resource_mut::<SpecialState>()
        .charge
        .insert(device, 600);
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Superweapon {
                building: device,
                target: CellPos::new(20, 20),
                units: Vec::new(),
            },
        )],
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::CommandRejected { .. }))
    );
    let events = ticks(&mut app, 220);
    assert_eq!(impacts(&events).len(), 10);
    assert_eq!(hp(&mut app, vehicle), Some(600 - 10 * blast(100, 50, 0)));
    assert_eq!(hp(&mut app, near), Some(600 - 10 * blast(100, 50, 512)));
}

#[test]
fn an_effect_the_combat_file_does_not_list_keeps_the_rule_of_m9() {
    // Without the effect the strike is M9's: the silo's strength (20) at the target, less with
    // every cell, nothing beyond its radius (2 cells), whatever the armour.
    let mut app = world_with(
        "no_nuke",
        &[(
            "        (id: \"nuke\", damage: 300, warhead: \"megaton\"),\n",
            "",
        )],
    );
    let soldier = spawn(&mut app, P1, "conscript", 20, 20);
    let tank = spawn(&mut app, P1, "tank", 21, 20);
    let beyond = spawn(&mut app, P1, "tank", 23, 20);
    let events = strike(&mut app);
    assert!(impacts(&events).is_empty());
    assert_eq!(hp(&mut app, soldier), Some(90 - 20));
    assert_eq!(hp(&mut app, tank), Some(300 - 20 * 2 / 3));
    assert_eq!(hp(&mut app, beyond), Some(300));
}
