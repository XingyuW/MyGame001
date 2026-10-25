//! Invariant I7 with the shipped costs and build times at every production rate (advisory
//! review 1, tests P2-7 and P2-8; docs/DECISIONS.md R56), on the shipped data (`assets/data`)
//! through `CommandQueue` and `SimSchedule`: every unit type a full base of either side can
//! queue, with one to four factories, with and without power, is built with exactly its cost in
//! hand (and leaves no credit) and is never built with one credit less (and the credits never
//! go below zero). `ra2_production.rs` tests the same property on the fixture with one cost;
//! this is the plan's "property test over costs and rates" (milestone RA2). And the positive
//! control of `ra2_shipped.rs::low_power_stops_the_powered_buildings_that_consume_power_and_no_others`:
//! in that test's scenario, with power, the Prism Tower fires. Both were written by the review
//! as probes and found nothing wrong; they are kept so that it stays so.
use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    Cell, CellPos, Command, CommandQueue, Definitions, Grid, IssuedCommand, PlayerId, Players,
    ProductionItem, RejectReason, SimEvent, SimId, SimPlugin, SimSchedule, UnitKind, load_ruleset,
    spawn_building, spawn_unit,
};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

fn shipped() -> Definitions {
    load_ruleset(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data"))
        .unwrap_or_else(|error| panic!("{error}"))
}

fn world(credits: i64, countries: [&str; 2]) -> App {
    let definitions = shipped();
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 11 });
    let cell = Cell {
        passable: true,
        water: false,
        buildable: true,
        ore: 0,
    };
    app.world_mut()
        .insert_resource(Grid::from_cells(96, 96, vec![cell; 96 * 96]).expect("a grid"));
    for (player, country) in [(P0, countries[0]), (P1, countries[1])] {
        let id = definitions
            .rules()
            .and_then(|rules| rules.country_id(country))
            .unwrap_or_else(|| panic!("country {country}"));
        let mut players = app.world_mut().resource_mut::<Players>();
        players.add(player, credits).expect("a new player");
        assert!(players.set_country(player, Some(id)));
    }
    app.world_mut().insert_resource(definitions);
    app
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

fn rejections(events: &[SimEvent]) -> Vec<RejectReason> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::CommandRejected { reason, .. } => Some(reason.clone()),
            _ => None,
        })
        .collect()
}

fn credits(app: &App) -> i64 {
    app.world()
        .resource::<Players>()
        .get(P0)
        .expect("P0")
        .credits()
}

fn place(app: &mut App, id: &str, x: i32, y: i32) -> SimId {
    spawn_building(app.world_mut(), P0, id, CellPos::new(x, y))
        .unwrap_or_else(|error| panic!("{id}: {error}"))
}

/// A base of `side` with `factories` barracks and war factories, with or without power.
fn base(credits: i64, soviet: bool, factories: usize, powered: bool) -> App {
    let countries = if soviet {
        ["russia", "america"]
    } else {
        ["america", "russia"]
    };
    let mut app = world(credits, countries);
    let (yard, power, barracks, factory, refinery, radar, lab) = if soviet {
        (
            "soviet_construction_yard",
            "tesla_reactor",
            "soviet_barracks",
            "soviet_war_factory",
            "soviet_refinery",
            "radar_tower",
            "soviet_battle_lab",
        )
    } else {
        (
            "construction_yard",
            "power_plant",
            "barracks",
            "war_factory",
            "refinery",
            "american_airforce_command",
            "tech_center",
        )
    };
    place(&mut app, yard, 2, 2);
    place(&mut app, refinery, 8, 2);
    place(&mut app, radar, 14, 2);
    place(&mut app, lab, 20, 2);
    if powered {
        for index in 0..12 {
            place(&mut app, power, 2 + 5 * index, 10);
        }
    }
    for index in 0..factories {
        place(&mut app, barracks, 2 + 5 * index as i32, 20);
        place(&mut app, factory, 2 + 7 * index as i32, 30);
    }
    tick(&mut app, Vec::new());
    app
}

#[test]
fn every_shipped_unit_is_charged_exactly_its_cost_at_every_rate_and_never_built_for_less() {
    let definitions = shipped();
    let mut tested = 0;
    let mut skipped = Vec::new();
    let mut problems = Vec::new();
    for soviet in [false, true] {
        for (index, def) in definitions.units().iter().enumerate() {
            let kind = UnitKind(index as u16);
            let item = ProductionItem::Unit(kind);
            let Some(terms) = definitions.production(item).cloned() else {
                continue;
            };
            let cost = terms.cost;
            for factories in 1..=4usize {
                for powered in [true, false] {
                    // Exactly the cost: the unit must leave and nothing must be left.
                    let mut app = base(cost, soviet, factories, powered);
                    let before_units = count_units(&mut app);
                    let events = tick(
                        &mut app,
                        vec![IssuedCommand {
                            player: P0,
                            command: Command::QueueProduction { item },
                        }],
                    );
                    if !rejections(&events).is_empty() {
                        if factories == 1 && powered {
                            skipped.push(format!("{} ({})", def.id, soviet));
                        }
                        continue;
                    }
                    tested += 1;
                    let mut done = false;
                    for _ in 0..(terms.build_time * 3 + 50) {
                        if credits(&app) < 0 {
                            problems.push(format!(
                                "{}: negative credits {} ({factories} factories, powered {powered})",
                                def.id,
                                credits(&app)
                            ));
                            break;
                        }
                        if count_units(&mut app) > before_units {
                            done = true;
                            break;
                        }
                        tick(&mut app, Vec::new());
                    }
                    if !done {
                        problems.push(format!(
                            "{}: cost {cost} time {}: not produced with exactly its cost \
                             ({factories} factories, powered {powered}), credits left {}",
                            def.id,
                            terms.build_time,
                            credits(&app)
                        ));
                    } else if credits(&app) != 0 {
                        problems.push(format!(
                            "{}: cost {cost}: {} credits left after production \
                             ({factories} factories, powered {powered})",
                            def.id,
                            credits(&app)
                        ));
                    }
                    // One credit short: never produced, never negative.
                    let mut app = base(cost - 1, soviet, factories, powered);
                    let before_units = count_units(&mut app);
                    tick(
                        &mut app,
                        vec![IssuedCommand {
                            player: P0,
                            command: Command::QueueProduction { item },
                        }],
                    );
                    for _ in 0..(terms.build_time * 2 + 50) {
                        tick(&mut app, Vec::new());
                        if credits(&app) < 0 {
                            problems.push(format!("{}: negative when short", def.id));
                            break;
                        }
                    }
                    if count_units(&mut app) > before_units {
                        problems.push(format!(
                            "{}: produced for {} credits instead of {cost} ({factories} \
                             factories, powered {powered})",
                            def.id,
                            cost - 1
                        ));
                    }
                }
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    assert!(tested > 100, "{tested}");
}

fn count_units(app: &mut App) -> usize {
    let mut query = app.world_mut().query::<(&sim::Owner, &sim::Unit)>();
    query.iter(app.world()).filter(|(o, _)| o.0 == P0).count()
}

fn fired(events: &[SimEvent], id: SimId) -> bool {
    events
        .iter()
        .any(|event| matches!(event, SimEvent::WeaponFired { attacker, .. } if *attacker == id))
}

#[test]
fn the_prism_tower_fires_with_power_in_the_scenario_of_the_low_power_test() {
    // The scenario of ra2_shipped.rs::low_power_stops_..., on its 48 x 48 map positions, with
    // power: the positive control that test lacks.
    let mut app = world(1_000, ["america", "russia"]);
    place(&mut app, "power_plant", 20, 20);
    place(&mut app, "power_plant", 24, 20);
    let pillbox = place(&mut app, "pillbox", 10, 10);
    let prism = place(&mut app, "prism_tower", 12, 10);
    spawn_unit(app.world_mut(), P1, "rhino", CellPos::new(11, 13)).expect("rhino");
    let mut events = Vec::new();
    for _ in 0..40 {
        events.extend(tick(&mut app, Vec::new()));
    }
    let state = app.world().resource::<Players>().get(P0).expect("P0");
    assert!(!state.low_power());
    assert!(fired(&events, pillbox), "pillbox");
    assert!(fired(&events, prism), "prism tower with power");
}
