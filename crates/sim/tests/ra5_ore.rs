//! RA5: ore and gems in bails, the pace of harvesting, growth and spread, the purifier's and
//! the spy's rule values, and the refund of a refinery, on the RA5 fixture through
//! `CommandQueue` and `SimSchedule` (docs/RA2_PARITY_PLAN.md sections 4.10 and 5, RA5;
//! docs/DECISIONS.md R58, R59). The fixture's miners carry 20 and 40 bails and take one after
//! every 4 ticks of work; a bail of ore is worth 25 and a bail of gems 50; every 40 ticks every
//! cell of ore below 12 bails grows and every cell above 6 spreads.

mod ra5_common;

use bevy_app::App;
use ra5_common::*;
use sim::{
    CellPos, Command, Grid, Harvester, MatchOptions, MatchRules, MatchState, PlacementError,
    SimEvent, SimId, SimRng, state_hash,
};

/// The deliveries among `events`: bails, of them gems, and credits.
fn deliveries(events: &[SimEvent]) -> Vec<(SimId, u32, u32, i64)> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::OreDelivered {
                harvester,
                ore,
                gems,
                credits,
                ..
            } => Some((*harvester, *ore, *gems, *credits)),
            _ => None,
        })
        .collect()
}

/// Runs until the first delivery, at most `limit` ticks.
fn first_delivery(app: &mut App, limit: u32) -> (SimId, u32, u32, i64) {
    for _ in 0..limit {
        let events = tick(app, Vec::new());
        if let Some(delivery) = deliveries(&events).first() {
            return *delivery;
        }
    }
    panic!("no delivery within {limit} ticks");
}

/// The fixture's world with ore that neither grows nor spreads, for the tests of harvesting.
fn still_world(name: &str) -> App {
    world_of(
        definitions_with(
            name,
            &[
                ("ore_growth_pm: 1000,", "ore_growth_pm: 0,"),
                ("ore_spread_pm: 1000,", "ore_spread_pm: 0,"),
            ],
        ),
        7,
    )
}

/// A world with a refinery of `owner` at (10, 10) and a miner of type `miner` beside a row of
/// cells at y = 14, each with 12 bails, of gems where `gem` says so for the cell's index. The
/// ore stands still.
fn mine(
    owner: sim::PlayerId,
    refinery: &str,
    miner: &str,
    cells: i32,
    gem: impl Fn(i32) -> bool,
) -> (App, SimId) {
    let mut app = still_world(&format!("mine_{miner}_{cells}_{}", gem(0)));
    place(&mut app, owner, refinery, 10, 10);
    for index in 0..cells {
        if gem(index) {
            gems(&mut app, 10 + index, 14, 12);
        } else {
            ore(&mut app, 10 + index, 14, 12);
        }
    }
    let unit = spawn(&mut app, owner, miner, 10, 13);
    (app, unit)
}

// --- Positive -------------------------------------------------------------------------------

#[test]
fn a_full_miner_is_paid_25_a_bail_of_ore_and_50_a_bail_of_gems() {
    // The Allied miner carries 20 bails, the Soviet one 40 (the reference's Chrono Miner and
    // War Miner): 500 and 1000 credits of ore, 1000 and 2000 of gems.
    for (owner, refinery, miner, bails) in [
        (P0, "allied_refinery", "miner_a", 20),
        (P1, "soviet_refinery", "miner_s", 40),
    ] {
        for (all_gems, value) in [(false, 25), (true, 50)] {
            let (mut app, unit) = mine(owner, refinery, miner, 4, |_| all_gems);
            let before = credits(&app, owner);
            let (who, ore, gem_bails, paid) = first_delivery(&mut app, 2_000);
            assert_eq!(who, unit);
            assert_eq!(ore, bails, "{miner}: a full load");
            assert_eq!(gem_bails, if all_gems { bails } else { 0 });
            assert_eq!(paid, i64::from(bails) * value, "{miner}");
            assert_eq!(credits(&app, owner) - before, paid);
            // The load is gone, and what was taken is what is missing from the field.
            let left = harvester(&mut app, unit);
            assert_eq!((left.load, left.gems), (0, 0));
            let remaining: u32 = field(&app).iter().map(|(_, ore)| ore).sum();
            assert_eq!(remaining, 4 * 12 - bails);
        }
    }
}

#[test]
fn a_mixed_load_is_paid_by_what_each_bail_is() {
    // Ore in the first cell, gems in the second: the miner empties the first (12 bails) and
    // takes the other 8 from the gems.
    let (mut app, unit) = mine(P0, "allied_refinery", "miner_a", 2, |index| index == 1);
    let before = credits(&app, P0);
    let (who, ore, gem_bails, paid) = first_delivery(&mut app, 2_000);
    assert_eq!((who, ore), (unit, 20));
    assert_eq!(gem_bails, 8);
    assert_eq!(paid, 12 * 25 + 8 * 50);
    assert_eq!(credits(&app, P0) - before, 700);
    assert_eq!(field(&app), [(CellPos::new(11, 14), 4)]);
    assert!(app.world().resource::<Grid>().is_gems(CellPos::new(11, 14)));
}

#[test]
fn a_miner_takes_one_bail_after_every_four_ticks_of_work() {
    let mut app = still_world("pace");
    ore(&mut app, 10, 14, 12);
    let unit = spawn(&mut app, P0, "miner_a", 10, 14);
    for ran in 1..=48_u32 {
        tick(&mut app, Vec::new());
        let state = harvester(&mut app, unit);
        assert_eq!(state.load, ran / 4, "after {ran} ticks");
        assert_eq!(state.work, ran % 4, "after {ran} ticks");
        assert_eq!(bails(&app, 10, 14), 12 - ran / 4, "after {ran} ticks");
    }
    // The cell is empty after 48 ticks, exactly; without a refinery the miner stops with its
    // load.
    assert!(field(&app).is_empty());
    ticks(&mut app, 8);
    let state = harvester(&mut app, unit);
    assert_eq!((state.load, state.gems, state.work), (12, 0, 0));
}

#[test]
fn a_move_order_resets_the_work_on_a_cell() {
    // Three ticks of work, then the miner is ordered away and back: the bail takes four more.
    let mut app = world();
    ore(&mut app, 10, 14, 12);
    let unit = spawn(&mut app, P0, "miner_a", 10, 14);
    ticks(&mut app, 3);
    assert_eq!(harvester(&mut app, unit).work, 3);
    tick(&mut app, vec![move_to(P0, unit, 10, 15)]);
    assert_eq!(harvester(&mut app, unit).work, 0);
    assert_eq!(bails(&app, 10, 14), 12, "no bail for three ticks of work");
}

#[test]
fn ore_grows_by_a_bail_on_schedule_up_to_the_densest() {
    let mut app = world();
    for (x, amount) in [(5, 1), (6, 6), (7, 11), (8, 12)] {
        // Rock around each cell, so that nothing spreads.
        for dy in -1..=1 {
            for dx in -1..=1 {
                terrain(
                    &mut app,
                    x * 3 + dx,
                    20 + dy,
                    sim::Cell {
                        passable: false,
                        buildable: false,
                        ..OPEN
                    },
                );
            }
        }
        ore(&mut app, x * 3, 20, amount);
    }
    let amounts = |app: &App| [5, 6, 7, 8].map(|x| bails(app, x * 3, 20));
    // The pass is in the tick with index 40: nothing before it.
    ticks(&mut app, 40);
    assert_eq!(amounts(&app), [1, 6, 11, 12]);
    tick(&mut app, Vec::new());
    assert_eq!(amounts(&app), [2, 7, 12, 12]);
    // The next in the tick with index 80; the densest cells stay as they are.
    ticks(&mut app, 39);
    assert_eq!(amounts(&app), [2, 7, 12, 12]);
    tick(&mut app, Vec::new());
    assert_eq!(amounts(&app), [3, 8, 12, 12]);
    ticks(&mut app, 400);
    assert_eq!(amounts(&app), [12, 12, 12, 12]);
    assert_eq!(field(&app).len(), 4, "nothing spread onto rock");
}

#[test]
fn ore_above_the_spread_density_puts_a_bail_on_one_free_cell_beside_it() {
    // Growth off, so that only spreading changes the field.
    let definitions = definitions_with("spread", &[("ore_growth_pm: 1000,", "ore_growth_pm: 0,")]);
    let mut app = world_of(definitions, 3);
    ore(&mut app, 20, 20, 7);
    ticks(&mut app, 41);
    let cells = field(&app);
    assert_eq!(cells.len(), 2, "{cells:?}");
    let (new, amount) = *cells
        .iter()
        .find(|(cell, _)| *cell != CellPos::new(20, 20))
        .expect("a new cell");
    assert_eq!(amount, 1);
    assert!(
        (new.x - 20).abs() <= 1 && (new.y - 20).abs() <= 1,
        "{new:?}"
    );
    assert_eq!(bails(&app, 20, 20), 7, "the spreading cell loses nothing");
    assert!(!app.world().resource::<Grid>().is_gems(new));
    // The new cell holds one bail: it does not spread. The old one spreads again at 80, 120,
    // ..., each time onto another free cell, until the eight around it hold ore.
    ticks(&mut app, 40 * 12);
    let cells = field(&app);
    assert_eq!(cells.len(), 9, "{cells:?}");
    assert!(
        cells
            .iter()
            .all(|(cell, _)| (cell.x - 20).abs() <= 1 && (cell.y - 20).abs() <= 1)
    );
    assert!(
        cells
            .iter()
            .all(|(cell, amount)| *amount == if *cell == CellPos::new(20, 20) { 7 } else { 1 })
    );
}

#[test]
fn ore_that_spread_onto_free_ground_is_harvested_and_paid_for() {
    // Integration: a dense cell that no unit can enter (rock on every side but one corner,
    // and no unit steps diagonally between two blocked cells) spreads onto the one free cell
    // at that corner, again and again; a miner sent there whenever ore lies on it is paid for
    // ore that the map did not hold where a miner could reach it.
    let mut app = world();
    place(&mut app, P0, "allied_refinery", 10, 10);
    let rock = sim::Cell {
        passable: false,
        buildable: false,
        ..OPEN
    };
    for (dx, dy) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1)] {
        terrain(&mut app, 20 + dx, 20 + dy, rock);
    }
    ore(&mut app, 20, 20, 12);
    let unit = spawn(&mut app, P0, "miner_a", 22, 22);
    assert!(field(&app).len() == 1 && bails(&app, 21, 21) == 0);
    let mut paid = Vec::new();
    for _ in 0..3_000 {
        let idle = harvester(&mut app, unit).state == sim::HarvestState::Idle;
        let commands = if idle && bails(&app, 21, 21) > 0 {
            vec![issue(
                P0,
                Command::Harvest {
                    units: vec![unit],
                    target: CellPos::new(21, 21),
                },
            )]
        } else {
            Vec::new()
        };
        let events = tick(&mut app, commands);
        assert!(rejections(&events).is_empty(), "{events:?}");
        paid.extend(deliveries(&events));
        assert!(credits(&app, P0) >= 5_000);
    }
    assert!(paid.len() >= 3, "{paid:?}");
    assert!(paid.iter().all(|(who, ..)| *who == unit));
    let income: i64 = paid.iter().map(|(.., credits)| credits).sum();
    assert_eq!(credits(&app, P0), 5_000 + income);
    let delivered: u32 = paid.iter().map(|(_, ore, ..)| ore).sum();
    assert_eq!(income, i64::from(delivered) * 25);
    // The dense cell lost nothing, and nothing but the corner ever held ore.
    assert_eq!(bails(&app, 20, 20), 12);
    assert!(
        field(&app)
            .iter()
            .all(|(cell, _)| [CellPos::new(20, 20), CellPos::new(21, 21)].contains(cell))
    );
}

// --- Negative -------------------------------------------------------------------------------

#[test]
fn gems_neither_grow_nor_spread() {
    let mut app = world();
    gems(&mut app, 20, 20, 7);
    gems(&mut app, 25, 25, 12);
    ticks(&mut app, 500);
    assert_eq!(
        field(&app),
        [(CellPos::new(20, 20), 7), (CellPos::new(25, 25), 12)]
    );
}

#[test]
fn ore_stands_still_when_the_match_rule_is_off_or_the_chance_is_none() {
    let dense = |app: &mut App| {
        ore(app, 20, 20, 7);
        ore(app, 30, 30, 3);
    };
    // The rule of the match.
    let mut app = world();
    dense(&mut app);
    app.world_mut()
        .resource_mut::<MatchState>()
        .set_rules(MatchRules {
            ore_grows: false,
            ..MatchRules::default()
        });
    ticks(&mut app, 500);
    assert_eq!(
        field(&app),
        [(CellPos::new(20, 20), 7), (CellPos::new(30, 30), 3)]
    );
    // A chance of nothing, for growth and for spread; and no random number is drawn for it:
    // the generator of a world with ore is where that of a world without ore is.
    let none = [
        ("ore_growth_pm: 1000,", "ore_growth_pm: 0,"),
        ("ore_spread_pm: 1000,", "ore_spread_pm: 0,"),
    ];
    let mut app = world_of(definitions_with("still", &none), 7);
    dense(&mut app);
    ticks(&mut app, 500);
    assert_eq!(
        field(&app),
        [(CellPos::new(20, 20), 7), (CellPos::new(30, 30), 3)]
    );
    let mut bare = world_of(definitions_with("still_bare", &none), 7);
    ticks(&mut bare, 500);
    let next = |app: &mut App| app.world_mut().resource_mut::<SimRng>().next_u64();
    assert_eq!(next(&mut app), next(&mut bare));
    // With the chances of the fixture the generator has moved on (the test can fail).
    let mut growing = world();
    dense(&mut growing);
    ticks(&mut growing, 500);
    let mut still = world();
    ticks(&mut still, 500);
    assert_ne!(next(&mut growing), next(&mut still));
}

#[test]
fn ore_spreads_only_onto_free_building_ground_or_mined_ground() {
    let definitions = definitions_with("targets", &[("ore_growth_pm: 1000,", "ore_growth_pm: 0,")]);
    // The cells around (20, 20): rock, water, a road (passable, no building ground), a
    // building, ore, gems, and two free cells of which one is mined ground.
    let mut app = world_of(definitions, 5);
    let rock = sim::Cell {
        passable: false,
        buildable: false,
        ..OPEN
    };
    let water = sim::Cell {
        passable: false,
        water: true,
        buildable: false,
        ..OPEN
    };
    let road = sim::Cell {
        buildable: false,
        ..OPEN
    };
    terrain(&mut app, 19, 19, rock);
    terrain(&mut app, 20, 19, water);
    terrain(&mut app, 21, 19, road);
    ore(&mut app, 19, 20, 3);
    gems(&mut app, 21, 20, 3);
    place(&mut app, P0, "turret", 19, 21);
    // (20, 21): ore ground that a miner empties. (21, 21): building ground.
    ore(&mut app, 20, 21, 1);
    ore(&mut app, 20, 20, 7);
    let grid = |app: &App| app.world().resource::<Grid>().clone();
    for (x, y, takes) in [
        (19, 19, false),
        (20, 19, false),
        (21, 19, false),
        (19, 20, false),
        (21, 20, false),
        (19, 21, false),
        (20, 21, false),
        (21, 21, true),
        (20, 20, false),
        (-1, 0, false),
        (40, 40, false),
    ] {
        assert_eq!(
            grid(&app).takes_ore(CellPos::new(x, y)),
            takes,
            "({x}, {y})"
        );
    }
    // The miner empties (20, 21) in four ticks (and stays, without a refinery).
    let miner = spawn(&mut app, P0, "miner_a", 20, 21);
    ticks(&mut app, 4);
    assert_eq!(bails(&app, 20, 21), 0);
    assert_eq!(harvester(&mut app, miner).load, 1);
    tick(
        &mut app,
        vec![issue(P0, Command::Stop { units: vec![miner] })],
    );
    assert!(grid(&app).takes_ore(CellPos::new(20, 21)), "mined ground");
    // Two passes (ticks 40 and 80): the two free cells get a bail each, and no other cell.
    ticks(&mut app, 80);
    assert_eq!(bails(&app, 20, 21), 1);
    assert_eq!(bails(&app, 21, 21), 1);
    ticks(&mut app, 400);
    let cells = field(&app);
    assert_eq!(
        cells,
        [
            (CellPos::new(19, 20), 3),
            (CellPos::new(20, 20), 7),
            (CellPos::new(21, 20), 3),
            (CellPos::new(20, 21), 1),
            (CellPos::new(21, 21), 1),
        ]
    );
    // Planted ore is ore, also where gems lay before; and the ground is mined ground no more.
    assert!(!grid(&app).is_gems(CellPos::new(20, 21)));
    assert!(!grid(&app).takes_ore(CellPos::new(20, 21)));
}

#[test]
fn nothing_is_built_on_ore_that_spread_onto_building_ground() {
    let mut app = world();
    // Building ground with a bail of ore on it, as ore that spread leaves it.
    terrain(&mut app, 21, 21, sim::Cell { ore: 1, ..OPEN });
    let refused = sim::spawn_building(app.world_mut(), P0, "allied_power", CellPos::new(20, 20));
    assert!(
        matches!(
            refused,
            Err(sim::SpawnBuildingError::Placement(PlacementError::NotBuildable(cell)))
                if cell == CellPos::new(21, 21)
        ),
        "{refused:?}"
    );
    // Through a command: the base unit does not deploy onto it either, and nothing changes.
    let mcv = spawn(&mut app, P0, "allied_mcv", 20, 20);
    let before = state_hash(app.world_mut());
    let events = tick(&mut app, vec![issue(P0, Command::Deploy { unit: mcv })]);
    assert_eq!(rejections(&events).len(), 1, "{events:?}");
    assert!(units_of(&mut app, P0).iter().any(|(id, _)| *id == mcv));
    assert_ne!(before, 0);
    // Once a miner has taken the bail, the ground is building ground again.
    let miner = spawn(&mut app, P0, "miner_a", 21, 21);
    ticks(&mut app, 4);
    assert_eq!(bails(&app, 21, 21), 0);
    tick(&mut app, vec![move_to(P0, miner, 30, 30)]);
    ticks(&mut app, 200);
    let events = tick(&mut app, vec![issue(P0, Command::Deploy { unit: mcv })]);
    assert!(rejections(&events).is_empty(), "{events:?}");
    assert!(units_of(&mut app, P0).iter().all(|(id, _)| *id != mcv));
}

// --- Boundary -------------------------------------------------------------------------------

#[test]
fn a_miner_with_room_for_one_bail_takes_one_and_a_cell_is_emptied_exactly() {
    let (mut app, unit) = mine(P0, "allied_refinery", "miner_a", 1, |_| false);
    // Nineteen bails aboard, three of them gems (scenario setup).
    {
        let world = app.world_mut();
        let mut query = world.query::<(&SimId, &mut Harvester)>();
        let (_, mut state) = query
            .iter_mut(world)
            .find(|(id, _)| **id == unit)
            .expect("the miner");
        state.load = 19;
        state.gems = 3;
    }
    let (_, ore_bails, gem_bails, paid) = first_delivery(&mut app, 2_000);
    assert_eq!((ore_bails, gem_bails), (20, 3));
    assert_eq!(paid, 17 * 25 + 3 * 50);
    assert_eq!(bails(&app, 10, 14), 11);

    // A removal larger than what the cell holds empties it and takes no more: five bails at a
    // time from a cell of three (gems, which do not grow).
    let economy = variant(
        "rate_5",
        "economy.ron",
        &[(
            r#"(unit: "miner_a", capacity: 20, harvest_rate: 1, harvest_interval: 4),"#,
            r#"(unit: "miner_a", capacity: 20, harvest_rate: 5, harvest_interval: 4),"#,
        )],
    );
    let definitions = load(economy, fixture("rules.ron")).unwrap_or_else(|error| panic!("{error}"));
    let mut app = world_of(definitions, 7);
    gems(&mut app, 10, 14, 3);
    let unit = spawn(&mut app, P0, "miner_a", 10, 14);
    ticks(&mut app, 4);
    let state = harvester(&mut app, unit);
    assert_eq!((state.load, state.gems), (3, 3));
    assert_eq!(bails(&app, 10, 14), 0);
    // An emptied cell of gems is a cell of gems no longer.
    assert!(!app.world().resource::<Grid>().is_gems(CellPos::new(10, 14)));
    assert_eq!(app.world().resource::<Grid>().gem_cells(), 0);
}

#[test]
fn ore_spreads_from_seven_bails_and_not_from_six() {
    let definitions = definitions_with(
        "threshold",
        &[("ore_growth_pm: 1000,", "ore_growth_pm: 0,")],
    );
    let mut app = world_of(definitions, 9);
    ore(&mut app, 10, 10, 6);
    ore(&mut app, 30, 30, 7);
    ticks(&mut app, 41);
    let cells = field(&app);
    assert_eq!(cells.len(), 3, "{cells:?}");
    assert!(
        cells
            .iter()
            .all(|(cell, _)| *cell == CellPos::new(10, 10) || (cell.x - 30).abs() <= 1),
        "{cells:?}"
    );
    // A cell with no free cell around it draws nothing and spreads nowhere: the corner of
    // the map, walled in by ore.
    let definitions = definitions_with("walled", &[("ore_growth_pm: 1000,", "ore_growth_pm: 0,")]);
    let mut app = world_of(definitions, 9);
    ore(&mut app, 0, 0, 12);
    ore(&mut app, 1, 0, 1);
    ore(&mut app, 0, 1, 1);
    ore(&mut app, 1, 1, 1);
    let mut bare = world_of(
        definitions_with(
            "walled_bare",
            &[("ore_growth_pm: 1000,", "ore_growth_pm: 0,")],
        ),
        9,
    );
    ticks(&mut app, 200);
    ticks(&mut bare, 200);
    assert_eq!(field(&app).len(), 4);
    let next = |app: &mut App| app.world_mut().resource_mut::<SimRng>().next_u64();
    assert_eq!(next(&mut app), next(&mut bare));
}

#[test]
fn the_ore_amounts_of_a_map_become_bails_at_match_setup() {
    // The fixture map's ore is 100 a cell: 4 bails of ore. Variants of single cells: an
    // amount of 12 rounds down to no bail and is one all the same; 13 rounds up to one; 37
    // and 38 are either side of one and a half; 300 is the densest; more is no more.
    let data = data();
    let mut grid = fixture_map();
    let ore_cells: Vec<CellPos> = (0..40)
        .flat_map(|y| (0..40).map(move |x| CellPos::new(x, y)))
        .filter(|&cell| grid.cell(cell).is_some_and(|terrain| terrain.ore > 0))
        .collect();
    assert!(ore_cells.len() >= 12, "{}", ore_cells.len());
    let amounts = [1, 12, 13, 37, 38, 300, 301, 100_000];
    for (cell, amount) in ore_cells.iter().zip(amounts) {
        let terrain = *grid.cell(*cell).expect("a cell");
        grid.set_terrain(
            *cell,
            sim::Cell {
                ore: amount,
                ..terrain
            },
        );
    }
    // Gems: an amount of 100 is 2 bails of 50, of 25 one (the half rounds up), of 700 twelve.
    let gem_cells = &ore_cells[8..11];
    for (cell, amount) in gem_cells.iter().zip([100, 25, 700]) {
        let terrain = *grid.cell(*cell).expect("a cell");
        grid.set_terrain(
            *cell,
            sim::Cell {
                ore: amount,
                ..terrain
            },
        );
        assert!(grid.set_gems(*cell, true));
    }
    let players = [human(P0, ALPHA, 0), human(P1, GAMMA, 1)];
    let app = match_on(&data, grid, &players, &MatchOptions::default(), 1);
    let grid = app.world().resource::<Grid>();
    let bails_of = |cell: &CellPos| grid.cell(*cell).expect("a cell").ore;
    assert_eq!(
        ore_cells[..8].iter().map(bails_of).collect::<Vec<_>>(),
        [1, 1, 1, 1, 2, 12, 12, 12]
    );
    assert_eq!(
        gem_cells.iter().map(bails_of).collect::<Vec<_>>(),
        [2, 1, 12]
    );
    assert!(gem_cells.iter().all(|cell| grid.is_gems(*cell)));
    assert!(ore_cells[11..].iter().all(|cell| bails_of(cell) == 4));
    assert!(ore_cells[..8].iter().all(|cell| !grid.is_gems(*cell)));
    assert_eq!(grid.gem_cells(), 3);
}

// --- The rule values of the purifier and the spy, and the refund ----------------------------

#[test]
fn every_powered_purifier_adds_the_rules_share_to_a_delivery() {
    // The fixture's vault is an ore purifier. With the rules' 25 percent a load of 500 pays
    // 125 more; with 40 percent 200; two purifiers twice that; an enemy's nothing.
    for (name, percent, vaults, enemy_vault, bonus) in [
        ("purifier_25", 25, 0, true, 0),
        ("purifier_25_1", 25, 1, false, 125),
        ("purifier_40_1", 40, 1, false, 200),
        ("purifier_40_2", 40, 2, true, 400),
        ("purifier_0_2", 0, 2, false, 0),
    ] {
        let definitions = definitions_with(
            name,
            &[(
                "purifier_bonus_percent: 25,",
                &format!("purifier_bonus_percent: {percent},"),
            )],
        );
        let mut app = world_of(definitions, 7);
        place(&mut app, P0, "allied_refinery", 10, 10);
        place(&mut app, P0, "allied_power", 20, 4);
        for index in 0..vaults {
            place(&mut app, P0, "vault", 20 + 3 * index, 8);
        }
        if enemy_vault {
            place(&mut app, P1, "vault", 30, 30);
        }
        ore(&mut app, 10, 14, 12);
        ore(&mut app, 11, 14, 12);
        spawn(&mut app, P0, "miner_a", 10, 13);
        let before = credits(&app, P0);
        let (_, _, _, paid) = first_delivery(&mut app, 2_000);
        assert_eq!(paid, 500, "{name}: the event names the value of the load");
        assert_eq!(credits(&app, P0) - before, 500 + bonus, "{name}");
        assert_eq!(credits(&app, P1), 5_000, "{name}");
    }
}

#[test]
fn a_spy_in_a_refinery_takes_the_rules_share_of_the_owners_credits() {
    // The fixture's militia is a spy. P0 has 5000 credits: 50 percent are 2500, 30 percent
    // 1500, 0 percent nothing, and 100 percent everything.
    for (name, percent, stolen) in [
        ("spy_50", 50, 2_500),
        ("spy_30", 30, 1_500),
        ("spy_0", 0, 0),
        ("spy_100", 100, 5_000),
    ] {
        let definitions = definitions_with(
            name,
            &[(
                "spy_money_steal_percent: 50,",
                &format!("spy_money_steal_percent: {percent},"),
            )],
        );
        let mut app = world_of(definitions, 7);
        let refinery = place(&mut app, P0, "allied_refinery", 10, 10);
        let spy = spawn(&mut app, P1, "militia", 9, 10);
        tick(&mut app, Vec::new());
        let events = tick(
            &mut app,
            vec![issue(
                P1,
                Command::Interact {
                    unit: spy,
                    target: refinery,
                },
            )],
        );
        assert!(rejections(&events).is_empty(), "{name}: {events:?}");
        ticks(&mut app, 40);
        assert_eq!(credits(&app, P0), 5_000 - stolen, "{name}");
        assert_eq!(credits(&app, P1), 5_000 + stolen, "{name}");
    }
}

#[test]
fn a_sold_refinery_returns_its_fixed_refund_and_its_survivors_follow_the_rule() {
    // The Allied refinery (cost 600) returns 90 instead of the rule's 40 percent, 240; the
    // Soviet one, which is not listed, returns 240.
    let mut app = world();
    let allied = place(&mut app, P0, "allied_refinery", 10, 10);
    let soviet = place(&mut app, P1, "soviet_refinery", 25, 25);
    tick(&mut app, Vec::new());
    let before = (credits(&app, P0), credits(&app, P1));
    let events = tick(
        &mut app,
        vec![
            issue(P0, Command::SellBuilding { building: allied }),
            issue(P1, Command::SellBuilding { building: soviet }),
        ],
    );
    assert!(rejections(&events).is_empty(), "{events:?}");
    let refunds: Vec<(SimId, i64)> = events
        .iter()
        .filter_map(|event| match event {
            SimEvent::BuildingSold {
                building, refund, ..
            } => Some((*building, *refund)),
            _ => None,
        })
        .collect();
    assert_eq!(refunds, [(allied, 90), (soviet, 240)]);
    assert_eq!(credits(&app, P0) - before.0, 90);
    assert_eq!(credits(&app, P1) - before.1, 240);
    // Survivors: `max(1, ceil(240 / divisor))` by the rule's value for both (divisors 500 and
    // 250), not by the 90 credits of the sale: one and one. The fixed refund of 90 with the
    // Allied divisor would also be one, so the Soviet side is the control and the shipped
    // data's test (`ra5_shipped.rs`) tells the two apart.
    let survivors: Vec<usize> = events
        .iter()
        .filter_map(|event| match event {
            SimEvent::CrewEscaped { units, .. } => Some(units.len()),
            _ => None,
        })
        .collect();
    let crewed = defs(&app)
        .metadata(sim::ProductionItem::Building(
            defs(&app).building_kind("allied_refinery").expect("kind"),
        ))
        .is_some_and(|entry| entry.crewed);
    if crewed {
        assert_eq!(survivors, [1, 1]);
    } else {
        assert!(survivors.is_empty());
    }
}

// --- Determinism ----------------------------------------------------------------------------

#[test]
fn growth_and_spread_are_deterministic_and_follow_the_seed() {
    // The chances of the reference (60 per mille) on a field of 400 cells, six passes.
    let chances = [
        ("ore_growth_pm: 1000,", "ore_growth_pm: 60,"),
        ("ore_spread_pm: 1000,", "ore_spread_pm: 60,"),
    ];
    let run = |name: &str, seed: u64| {
        let mut app = world_of(definitions_with(name, &chances), seed);
        for y in 10..30 {
            for x in 10..30 {
                ore(&mut app, x, y, 7);
            }
        }
        let mut hashes = Vec::new();
        for _ in 0..6 {
            ticks(&mut app, 40);
            hashes.push(state_hash(app.world_mut()));
        }
        let cells = field(&app);
        (hashes, cells)
    };
    let (first, cells) = run("seeded_a", 21);
    let (again, same) = run("seeded_b", 21);
    assert_eq!(first, again);
    assert_eq!(cells, same);
    let (other, others) = run("seeded_c", 22);
    assert_ne!(first, other);
    assert_ne!(cells, others);
    // What grew: some cells, and far from all (6 passes at 6 percent: about a third of the
    // 400 cells gain a bail; the borders spread a little).
    let grown = cells
        .iter()
        .filter(|(cell, amount)| {
            (10..30).contains(&cell.x) && (10..30).contains(&cell.y) && *amount > 7
        })
        .count();
    assert!((60..=220).contains(&grown), "{grown}");
    let spread = cells.len() - 400;
    assert!((1..=40).contains(&spread), "{spread}");
    assert!(cells.iter().all(|(_, amount)| (1..=12).contains(amount)));
}

#[test]
fn the_state_hash_covers_the_ore_the_gems_the_mined_ground_and_a_miners_work() {
    let base = |app: &mut App| state_hash(app.world_mut());
    let mut plain = world();
    ore(&mut plain, 20, 20, 5);
    let mut same = world();
    ore(&mut same, 20, 20, 5);
    assert_eq!(base(&mut plain), base(&mut same));
    // Gems instead of ore.
    let mut gem = world();
    gems(&mut gem, 20, 20, 5);
    assert_ne!(base(&mut plain), base(&mut gem));
    // Another amount.
    let mut more = world();
    ore(&mut more, 20, 20, 6);
    assert_ne!(base(&mut plain), base(&mut more));
    // Mined ground: two worlds whose cell (20, 21) is empty, in one of them emptied by a
    // miner. The miner's own state is made equal first (it stands idle with one bail).
    let mined = |harvest: bool| {
        let mut app = world();
        ore(&mut app, 20, 21, 1);
        let miner = spawn(&mut app, P0, "miner_a", 20, 21);
        if !harvest {
            tick(
                &mut app,
                vec![issue(P0, Command::Stop { units: vec![miner] })],
            );
        }
        ticks(&mut app, 5);
        tick(
            &mut app,
            vec![issue(P0, Command::Stop { units: vec![miner] })],
        );
        (app, miner)
    };
    let (mut emptied, miner) = mined(true);
    assert_eq!(bails(&emptied, 20, 21), 0);
    assert_eq!(harvester(&mut emptied, miner).load, 1);
    let before = base(&mut emptied);
    // The same world with the cell made empty by scenario setup: no mined ground.
    let terrain_now = *emptied
        .world()
        .resource::<Grid>()
        .cell(CellPos::new(20, 21))
        .expect("a cell");
    emptied
        .world_mut()
        .resource_mut::<Grid>()
        .set_terrain(CellPos::new(20, 21), terrain_now);
    assert_ne!(before, base(&mut emptied), "mined ground is hashed");
    // A miner's ticks of work.
    let (mut idle, _) = mined(false);
    let (mut other, _) = mined(false);
    assert_eq!(base(&mut idle), base(&mut other));
    let mut working = world();
    ore(&mut working, 20, 21, 12);
    spawn(&mut working, P0, "miner_a", 20, 21);
    let mut twin = world();
    ore(&mut twin, 20, 21, 12);
    spawn(&mut twin, P0, "miner_a", 20, 21);
    ticks(&mut working, 2);
    ticks(&mut twin, 2);
    assert_eq!(base(&mut working), base(&mut twin));
    {
        let world = twin.world_mut();
        let mut query = world.query::<&mut Harvester>();
        for mut state in query.iter_mut(world) {
            state.work += 1;
        }
    }
    assert_ne!(base(&mut working), base(&mut twin));
    {
        let world = twin.world_mut();
        let mut query = world.query::<&mut Harvester>();
        for mut state in query.iter_mut(world) {
            state.work -= 1;
            state.gems += 1;
        }
    }
    assert_ne!(base(&mut working), base(&mut twin));
}
