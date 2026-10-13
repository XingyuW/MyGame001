//! RA5 on the shipped data (`assets/data`), through `CommandQueue` and `SimSchedule`: the
//! miners' loads in bails and what a refinery pays for them, the free miner of each side's
//! refinery, the refund of a sold refinery, the purifier and the spy, the contents of a crate,
//! and the ore of the shipped maps (docs/RA2_PARITY_PLAN.md sections 4.10, 4.11 and 5, RA5;
//! docs/DECISIONS.md R58 to R60). The numbers in the comments are those of the reference
//! (`tests/reference/ra2_reference.tsv`), converted as the plan's section 2 says.

use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    Building, Cell, CellPos, Command, CommandQueue, CrateContent, Crates, Definitions, Domain,
    Grid, Harvester, IssuedCommand, Mobile, Owner, PlayerId, Players, ProductionItem, SimEvent,
    SimId, SimPlugin, SimSchedule, Unit, load_ruleset, spawn_building, spawn_unit,
};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data")
}

fn shipped() -> Definitions {
    load_ruleset(data_dir()).unwrap_or_else(|error| panic!("{error}"))
}

const OPEN: Cell = Cell {
    passable: true,
    water: false,
    buildable: true,
    ore: 0,
};

/// A world of 48 x 48 open cells with the shipped data and the players P0 (Great Britain,
/// Allied) and P1 (Russia, Soviet), who are enemies. It has no match, so no crate appears.
fn world() -> App {
    let definitions = shipped();
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 11 });
    app.world_mut()
        .insert_resource(Grid::from_cells(48, 48, vec![OPEN; 48 * 48]).expect("a grid"));
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

fn defs(app: &App) -> Definitions {
    app.world().resource::<Definitions>().clone()
}

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

fn issue(player: PlayerId, command: Command) -> IssuedCommand {
    IssuedCommand { player, command }
}

fn credits(app: &App, player: PlayerId) -> i64 {
    app.world()
        .resource::<Players>()
        .get(player)
        .expect("a player")
        .credits()
}

fn rejected(events: &[SimEvent]) -> Vec<&SimEvent> {
    events
        .iter()
        .filter(|event| matches!(event, SimEvent::CommandRejected { .. }))
        .collect()
}

/// Bails of ore (`gems`: of gems) in the cell (`x`, `y`): ore ground.
fn ore(app: &mut App, x: i32, y: i32, bails: u32, gems: bool) {
    let mut grid = app.world_mut().resource_mut::<Grid>();
    assert!(grid.set_terrain(
        CellPos::new(x, y),
        Cell {
            buildable: false,
            ore: bails,
            ..OPEN
        }
    ));
    assert!(grid.set_gems(CellPos::new(x, y), gems));
}

fn harvester(app: &mut App, id: SimId) -> Harvester {
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &Harvester)>();
    *query
        .iter(world)
        .find(|(found, _)| **found == id)
        .expect("a harvester")
        .1
}

/// The units of `owner`: id and type id.
fn units_of(app: &mut App, owner: PlayerId) -> Vec<(SimId, String)> {
    let definitions = defs(app);
    let world = app.world_mut();
    let mut query = world.query::<(&SimId, &Owner, &Unit)>();
    let mut units: Vec<(SimId, String)> = query
        .iter(world)
        .filter(|(_, of, _)| of.0 == owner)
        .map(|(id, _, unit)| (*id, definitions.unit(unit.kind).expect("type").id.clone()))
        .collect();
    units.sort();
    units
}

// --- Bails and gems --------------------------------------------------------------------------

#[test]
fn a_full_war_miner_pays_1000_for_ore_and_2000_for_gems_and_a_chrono_miner_half() {
    // [HARV] Storage=40, [CMIN] Storage=20; [Riparius] Value=25, [Cruentus] Value=50.
    for (owner, refinery, miner, bails) in [
        (P1, "soviet_refinery", "war_miner", 40_u32),
        (P0, "refinery", "chrono_miner", 20),
    ] {
        for (gems, value) in [(false, 25_i64), (true, 50)] {
            let mut app = world();
            put(&mut app, owner, refinery, 10, 10);
            for x in 10..14 {
                ore(&mut app, x, 15, 12, gems);
            }
            let unit = put(&mut app, owner, miner, 10, 14);
            let before = credits(&app, owner);
            // Well before the first pass of growth (tick 2933).
            let mut paid = None;
            for _ in 0..2_500 {
                let events = tick(&mut app, Vec::new());
                paid = events.iter().find_map(|event| match event {
                    SimEvent::OreDelivered {
                        harvester,
                        ore,
                        gems,
                        credits,
                        ..
                    } if *harvester == unit => Some((*ore, *gems, *credits)),
                    _ => None,
                });
                if paid.is_some() {
                    break;
                }
            }
            let (ore_bails, gem_bails, paid) = paid.unwrap_or_else(|| panic!("{miner}: no load"));
            assert_eq!(ore_bails, bails, "{miner}");
            assert_eq!(gem_bails, if gems { bails } else { 0 }, "{miner}");
            assert_eq!(paid, i64::from(bails) * value, "{miner}");
            assert_eq!(credits(&app, owner) - before, paid, "{miner}");
        }
    }
}

#[test]
fn a_shipped_miner_takes_a_bail_every_19_ticks() {
    // The game's own pace (the ini has no key for it): one bail after every 19 ticks of work.
    for (owner, miner) in [(P0, "chrono_miner"), (P1, "war_miner")] {
        let mut app = world();
        ore(&mut app, 20, 20, 12, false);
        let unit = put(&mut app, owner, miner, 20, 20);
        for ran in 1..=57_u32 {
            tick(&mut app, Vec::new());
            assert_eq!(
                harvester(&mut app, unit).load,
                ran / 19,
                "{miner} after {ran}"
            );
        }
        let grid = app.world().resource::<Grid>();
        assert_eq!(grid.cell(CellPos::new(20, 20)).expect("a cell").ore, 9);
    }
}

#[test]
fn the_value_of_a_bail_is_the_same_in_the_rules_and_at_every_refinery() {
    // The rules file's `ore_value` and `gem_value` are the reference's values, which the
    // generator also gives every refinery (and by which match setup divides a map's amounts).
    let definitions = shipped();
    let rules = definitions.rules().expect("rules").economy;
    assert_eq!((rules.ore_value, rules.gem_value), (25, 50));
    let refineries: Vec<_> = definitions
        .buildings()
        .iter()
        .filter(|def| def.credits_per_ore.is_some())
        .collect();
    assert_eq!(refineries.len(), 2);
    for def in refineries {
        assert_eq!(
            def.credits_per_ore,
            Some(i64::from(rules.ore_value)),
            "{}",
            def.id
        );
        assert_eq!(
            def.credits_per_gem,
            Some(i64::from(rules.gem_value)),
            "{}",
            def.id
        );
    }
    // [Riparius] Growth=2200 frames, GrowthPercentage=.06; twelve density stages.
    assert_eq!(
        (
            rules.ore_growth_interval,
            rules.ore_growth_pm,
            rules.ore_spread_interval,
            rules.ore_spread_pm,
            rules.ore_max_bails,
            rules.ore_spread_above
        ),
        (2933, 60, 2933, 60, 12, 6)
    );
}

// --- The free miner ---------------------------------------------------------------------------

#[test]
fn a_refinery_built_through_commands_gives_the_miner_of_its_side() {
    // [GAREFN] FreeUnit=CMIN, [NAREFN] FreeUnit=HARV.
    for (owner, yard, power, refinery, miner) in [
        (
            P0,
            "construction_yard",
            "power_plant",
            "refinery",
            "chrono_miner",
        ),
        (
            P1,
            "soviet_construction_yard",
            "tesla_reactor",
            "soviet_refinery",
            "war_miner",
        ),
    ] {
        let mut app = world();
        put(&mut app, owner, yard, 10, 10);
        put(&mut app, owner, power, 10, 15);
        tick(&mut app, Vec::new());
        assert!(units_of(&mut app, owner).is_empty());
        let kind = defs(&app).building_kind(refinery).expect("kind");
        let item = ProductionItem::Building(kind);
        let events = tick(
            &mut app,
            vec![issue(owner, Command::QueueProduction { item })],
        );
        assert!(rejected(&events).is_empty(), "{events:?}");
        let mut ready = false;
        for _ in 0..3_000 {
            let events = tick(&mut app, Vec::new());
            if events.iter().any(
                |event| matches!(event, SimEvent::ProductionComplete { item: done, .. } if *done == item),
            ) {
                ready = true;
                break;
            }
        }
        assert!(ready, "{refinery} was not built");
        let before = credits(&app, owner);
        let events = tick(
            &mut app,
            vec![issue(
                owner,
                Command::PlaceBuilding {
                    building: kind,
                    origin: CellPos::new(14, 10),
                },
            )],
        );
        assert!(rejected(&events).is_empty(), "{events:?}");
        // Its miner is there with the building, free of charge, and is a harvester at work.
        let units = units_of(&mut app, owner);
        assert_eq!(units.len(), 1, "{units:?}");
        assert_eq!(units[0].1, miner);
        assert_eq!(credits(&app, owner), before);
        assert_eq!(harvester(&mut app, units[0].0).load, 0);
        // [General] Cost of the refinery was charged exactly: 2000 of the 10000.
        assert_eq!(before, 8_000);
    }
}

// --- The refund of a refinery -------------------------------------------------------------------

#[test]
fn a_sold_refinery_returns_300_credits_and_as_many_survivors_as_before() {
    // [GAREFN] and [NAREFN] Soylent=300 instead of half of Cost=2000. The survivors follow the
    // refund rule's value (1000): 1000 / 500 = 2 for an Allied owner, 1000 / 250 = 4 for a
    // Soviet one (300 would give 1 and 2).
    for (owner, building, refund, crew, survivors) in [
        (P0, "refinery", 300, "gi", 2),
        (P1, "soviet_refinery", 300, "conscript", 4),
        // [GAOREP] Soylent=1250 is half its cost: 1250 / 500 rounds up to 3.
        (P0, "ore_purifier", 1250, "gi", 3),
        // No `Soylent`: half the cost. [GAPOWR] Cost=800.
        (P0, "power_plant", 400, "gi", 1),
    ] {
        let mut app = world();
        let id = put(&mut app, owner, building, 10, 10);
        tick(&mut app, Vec::new());
        // The free miner of a refinery is not part of the count below.
        let before = (credits(&app, owner), units_of(&mut app, owner).len());
        let events = tick(
            &mut app,
            vec![issue(owner, Command::SellBuilding { building: id })],
        );
        assert!(rejected(&events).is_empty(), "{events:?}");
        let sold: Vec<i64> = events
            .iter()
            .filter_map(|event| match event {
                SimEvent::BuildingSold { refund, .. } => Some(*refund),
                _ => None,
            })
            .collect();
        assert_eq!(sold, [refund], "{building}");
        assert_eq!(credits(&app, owner) - before.0, refund, "{building}");
        let units = units_of(&mut app, owner);
        let soldiers = units.iter().filter(|(_, kind)| kind == crew).count();
        assert_eq!(soldiers, survivors, "{building}: {units:?}");
        assert_eq!(units.len(), before.1 + survivors);
        let definitions = defs(&app);
        let kind = definitions.building_kind(building).expect("kind");
        assert_eq!(definitions.refund(kind), refund);
    }
}

// --- The purifier and the spy ---------------------------------------------------------------------

#[test]
fn the_ore_purifier_adds_a_quarter_and_the_spy_takes_half() {
    // [General] PurifierBonus=.25: a Chrono Miner's 500 credits of ore become 625.
    let mut app = world();
    put(&mut app, P0, "refinery", 10, 10);
    put(&mut app, P0, "power_plant", 20, 10);
    put(&mut app, P0, "power_plant", 23, 10);
    put(&mut app, P0, "ore_purifier", 20, 14);
    for x in 10..13 {
        ore(&mut app, x, 15, 12, false);
    }
    let miner = put(&mut app, P0, "chrono_miner", 10, 14);
    let before = credits(&app, P0);
    let mut delivered = None;
    for _ in 0..2_500 {
        let events = tick(&mut app, Vec::new());
        delivered = events.iter().find_map(|event| match event {
            SimEvent::OreDelivered {
                harvester, credits, ..
            } if *harvester == miner => Some(*credits),
            _ => None,
        });
        if delivered.is_some() {
            break;
        }
    }
    assert_eq!(delivered, Some(500));
    assert_eq!(credits(&app, P0) - before, 625);

    // [General] SpyMoneyStealPercent=.5: a spy in a refinery takes half the owner's credits.
    let mut app = world();
    let refinery = put(&mut app, P1, "soviet_refinery", 10, 10);
    let spy = put(&mut app, P0, "spy", 9, 10);
    tick(&mut app, Vec::new());
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: spy,
                target: refinery,
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    for _ in 0..60 {
        tick(&mut app, Vec::new());
    }
    assert_eq!(credits(&app, P1), 5_000);
    assert_eq!(credits(&app, P0), 15_000);
}

// --- Crates ---------------------------------------------------------------------------------------

#[test]
fn the_shipped_crates_hold_the_references_contents_and_land_vehicles() {
    let definitions = shipped();
    let rules = definitions.rules().expect("rules");
    // [Powerups]: the eight contents with a share, 110 shares in all.
    let shares: Vec<(&str, i32, i32)> = rules
        .powerups
        .iter()
        .map(|powerup| (powerup.id.as_str(), powerup.share, powerup.value))
        .collect();
    assert_eq!(
        shares,
        [
            ("money", 20, 2000),
            ("unit", 20, 0),
            ("veteran", 20, 1),
            ("heal_base", 10, 0),
            ("armour", 10, 1500),
            ("firepower", 10, 2000),
            ("speed", 10, 1200),
            ("reveal", 10, 0),
        ]
    );
    assert!(
        rules
            .powerups
            .iter()
            .all(|powerup| CrateContent::parse(&powerup.id).is_some())
    );
    // [CrateRules] CrateMaximum=255, CrateMinimum=1, CrateRadius=3.0, CrateRegen=3 minutes.
    assert_eq!(
        (
            rules.crates.maximum,
            rules.crates.minimum,
            rules.crates.radius,
            rules.crates.regen,
            rules.crates.free_mcv
        ),
        (255, 1, 768, 3600, 1)
    );
    // `CrateGoodie=yes`: ten vehicles, all of them drive on land, both base units among them.
    assert_eq!(rules.crate_units.len(), 10);
    for &kind in &rules.crate_units {
        let id = &definitions.unit(kind).expect("a unit").id;
        assert_eq!(definitions.domain(kind), Domain::Land, "{id}");
        assert!(
            definitions.production(ProductionItem::Unit(kind)).is_some(),
            "{id}"
        );
    }
    for side in &rules.sides {
        assert!(rules.crate_units.contains(&side.base_unit));
    }
}

#[test]
fn a_shipped_crate_gives_every_content_to_a_grizzly_over_many_seeds() {
    // End to end on the shipped rules: a Grizzly of a player with a base takes a crate; over
    // 200 seeds every content of the reference comes up, in about its share, and each does
    // what it says.
    let mut seen: std::collections::BTreeMap<CrateContent, u32> = std::collections::BTreeMap::new();
    for seed in 0..200_u64 {
        let definitions = shipped();
        let mut app = App::new();
        app.add_plugins(SimPlugin { seed });
        app.world_mut()
            .insert_resource(Grid::from_cells(48, 48, vec![OPEN; 48 * 48]).expect("a grid"));
        let country = definitions
            .rules()
            .and_then(|rules| rules.country_id("great_britain"));
        {
            let mut players = app.world_mut().resource_mut::<Players>();
            players.add(P0, 10_000).expect("a new player");
            assert!(players.set_country(P0, country));
        }
        app.world_mut().insert_resource(definitions);
        put(&mut app, P0, "power_plant", 40, 40);
        let tank = put(&mut app, P0, "grizzly", 10, 10);
        tick(&mut app, Vec::new());
        let grid = app.world().resource::<Grid>().clone();
        assert!(
            app.world_mut()
                .resource_mut::<Crates>()
                .place(&grid, CellPos::new(11, 10))
        );
        let mut found = None;
        let mut events = tick(
            &mut app,
            vec![issue(
                P0,
                Command::Move {
                    units: vec![tank],
                    target: CellPos::new(11, 10),
                },
            )],
        );
        for _ in 0..200 {
            found = events.iter().find_map(|event| match event {
                SimEvent::CrateTaken {
                    unit,
                    content,
                    credits,
                    spawned,
                    ..
                } => Some((*unit, *content, *credits, *spawned)),
                _ => None,
            });
            if found.is_some() {
                break;
            }
            events = tick(&mut app, Vec::new());
        }
        let (who, content, paid, new) = found.expect("the crate is taken");
        assert_eq!(who, tank);
        *seen.entry(content).or_default() += 1;
        match content {
            // [Powerups] Money=20,MONEY,yes,2000 and up to 900 more.
            CrateContent::Money => {
                assert!((2_000..=2_900).contains(&paid), "{paid}");
                assert_eq!(credits(&app, P0), 10_000 + paid);
            }
            CrateContent::Unit => {
                let new = new.expect("a unit");
                let units = units_of(&mut app, P0);
                let (_, kind) = units.iter().find(|(id, _)| *id == new).expect("P0's");
                let definitions = defs(&app);
                let kind = definitions.unit_kind(kind).expect("kind");
                assert!(
                    definitions
                        .rules()
                        .expect("rules")
                        .crate_units
                        .contains(&kind)
                );
            }
            _ => assert_eq!((paid, new), (0, None)),
        }
        if content != CrateContent::Money {
            assert_eq!(credits(&app, P0), 10_000);
        }
        let world = app.world_mut();
        let mut query = world.query::<(&SimId, &sim::Experience)>();
        let own = query
            .iter(world)
            .find(|(id, _)| **id == tank)
            .map(|(_, experience)| *experience)
            .expect("experience");
        // [Powerups] Veteran=...,1; Armor=...,1.5; Firepower=...,2.0; Speed=...,1.2.
        assert_eq!(own.rank, u8::from(content == CrateContent::Veteran));
        let expected = |of: CrateContent, value: u32| if content == of { value } else { 1000 };
        assert_eq!(own.bonus.armour_pm, expected(CrateContent::Armour, 1500));
        assert_eq!(
            own.bonus.firepower_pm,
            expected(CrateContent::Firepower, 2000)
        );
        assert_eq!(own.bonus.speed_pm, expected(CrateContent::Speed, 1200));
        assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
    }
    assert_eq!(seen.len(), 8, "{seen:?}");
    // 200 draws by the shares 20 : 20 : 20 : 10 : 10 : 10 : 10 : 10 of 110: about 36 and 18.
    for (content, count) in &seen {
        let (low, high) = match content {
            CrateContent::Money | CrateContent::Unit | CrateContent::Veteran => (18, 60),
            _ => (5, 36),
        };
        assert!((low..=high).contains(count), "{content:?}: {count} of 200");
    }
    assert_eq!(seen.values().sum::<u32>(), 200);
}

// --- The shipped maps -------------------------------------------------------------------------

#[test]
fn every_shipped_map_sets_up_with_ore_in_bails_and_a_crate() {
    let definitions = shipped();
    let skirmish = sim::load_skirmish(data_dir().join("skirmish.ron"), &definitions)
        .unwrap_or_else(|error| panic!("{error}"));
    let ai = sim::load_ai(data_dir().join("ai.ron"), &definitions)
        .unwrap_or_else(|error| panic!("{error}"));
    let countries = skirmish.countries();
    assert!(skirmish.maps.len() >= 21);
    for (index, map) in skirmish.maps.iter().enumerate() {
        let path = data_dir().join("..").join(sim::map_asset_path(&map.id));
        let loaded = sim::load_map(&path).unwrap_or_else(|error| panic!("{error}"));
        // The file's amounts, and which of its cells are gems (the look says so).
        let filed: Vec<(CellPos, u32, bool)> = (0..loaded.grid.height() as i32)
            .flat_map(|y| (0..loaded.grid.width() as i32).map(move |x| CellPos::new(x, y)))
            .filter_map(|cell| {
                let ore = loaded.grid.cell(cell)?.ore;
                (ore > 0).then(|| (cell, ore, loaded.grid.is_gems(cell)))
            })
            .collect();
        assert!(!filed.is_empty(), "{}", map.id);
        for (cell, _, gems) in &filed {
            let look = loaded.looks[(cell.y * loaded.grid.width() as i32 + cell.x) as usize];
            assert_eq!(
                *gems,
                look >> 4 == sim::LOOK_FEATURE_GEMS,
                "{}: {cell:?}",
                map.id
            );
        }
        let mut app = App::new();
        app.add_plugins(SimPlugin { seed: 3 });
        app.world_mut().insert_resource(loaded.grid.clone());
        app.world_mut().insert_resource(definitions.clone());
        let players = [0, 1].map(|player| sim::PlayerSetup {
            player: PlayerId(player),
            faction: countries[(index + usize::from(player)) % countries.len()],
            start: usize::from(player),
            controller: sim::Controller::Ai { difficulty: 1 },
        });
        sim::setup_match_configured(
            app.world_mut(),
            &skirmish,
            &ai,
            index,
            &players,
            &sim::MatchOptions::default(),
        )
        .unwrap_or_else(|error| panic!("{}: {error}", map.id));
        // In the match: the same cells, each with round(amount / value) bails, at most 12.
        let grid = app.world().resource::<Grid>().clone();
        for (cell, amount, gems) in &filed {
            let value = if *gems { 50 } else { 25 };
            let expected = ((2 * amount + value) / (2 * value)).clamp(1, 12);
            assert_eq!(
                grid.cell(*cell).expect("a cell").ore,
                expected,
                "{}",
                map.id
            );
            assert_eq!(grid.is_gems(*cell), *gems);
        }
        assert_eq!(
            grid.gem_cells(),
            filed.iter().filter(|(_, _, gems)| *gems).count()
        );
        // The first tick puts the match's crate down.
        let events = tick(&mut app, Vec::new());
        let crates: Vec<CellPos> = app.world().resource::<Crates>().cells().collect();
        assert_eq!(crates.len(), 1, "{}", map.id);
        assert!(
            events.iter().any(
                |event| matches!(event, SimEvent::CrateAppeared { cell } if *cell == crates[0])
            )
        );
        let terrain = grid.cell(crates[0]).expect("a cell");
        assert!(terrain.passable || terrain.water);
        assert_eq!(terrain.ore, 0);
        // Nobody's building or unit stands on it.
        let world = app.world_mut();
        let mut buildings = world.query::<&Building>();
        assert!(
            buildings
                .iter(world)
                .all(|building| building.footprint.distance_to_cell(crates[0]) > 0)
        );
        let mut units = world.query::<&Mobile>();
        assert!(units.iter(world).all(|mobile| mobile.cell != crates[0]));
    }
}
