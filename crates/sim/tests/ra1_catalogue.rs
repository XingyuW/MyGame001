//! The shipped catalogue through commands (advisory review 1, tests P1-5; docs/DECISIONS.md R56):
//! the counterpart, on the shipped data (`assets/data`) and so on the rules of Red Alert 2, of
//! the legacy fixture's `m9_arsenal.rs::every_item_executes_queue_cost_completion_and_placement_or_spawn`.
//! For each of the nine countries, from a Construction Yard on a map with a shore, every item
//! of the country's own tree is queued (`QueueProduction` answers as `check_buildable` says),
//! paid for to the credit, completed, and placed (`PlaceBuilding` at a site `check_placement`
//! accepts) or spawned, through `CommandQueue` and `SimSchedule`; then every building the
//! player owns is sold for its refund, or refused as not sellable (the walls). The test was
//! written by the review as a probe and found nothing wrong; it is kept so that it stays so.
use std::collections::BTreeSet;
use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    Building, BuildingKind, Cell, CellPos, Command, CommandQueue, Definitions, Grid, IssuedCommand,
    Owner, PlayerId, Players, ProductionItem, RejectReason, SimEvent, SimPlugin, SimSchedule, Unit,
    check_buildable, check_placement, country_tree, load_ruleset, spawn_building, spawn_unit,
};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);
const W: i32 = 96;
const H: i32 = 72;
const SHORE: i32 = 56;

fn shipped() -> Definitions {
    load_ruleset(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data"))
        .unwrap_or_else(|error| panic!("{error}"))
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

fn owned(app: &mut App) -> Vec<(BuildingKind, sim::Footprint)> {
    let mut query = app.world_mut().query::<(&Owner, &Building)>();
    query
        .iter(app.world())
        .filter(|(owner, _)| owner.0 == P0)
        .map(|(_, building)| (building.kind, building.footprint))
        .collect()
}

fn units_of(app: &mut App, kind: sim::UnitKind) -> usize {
    let mut query = app.world_mut().query::<(&Owner, &Unit)>();
    query
        .iter(app.world())
        .filter(|(owner, unit)| owner.0 == P0 && unit.kind == kind)
        .count()
}

fn name(definitions: &Definitions, item: ProductionItem) -> String {
    match item {
        ProductionItem::Building(kind) => definitions.building(kind).expect("kind").id.clone(),
        ProductionItem::Unit(kind) => definitions.unit(kind).expect("kind").id.clone(),
    }
}

fn site(app: &mut App, kind: BuildingKind) -> Option<CellPos> {
    let footprints = owned(app);
    let world = app.world();
    let (grid, definitions) = (world.resource::<Grid>(), world.resource::<Definitions>());
    let state = world.resource::<Players>().get(P0).expect("player");
    let ok = |origin: CellPos| {
        check_placement(
            grid,
            definitions,
            state,
            footprints.iter().copied(),
            kind,
            origin,
        )
        .is_ok()
    };
    // Sites on a lattice first, so that factories keep free cells around them.
    let lattice = (0..H)
        .flat_map(|y| (0..W).map(move |x| CellPos::new(x, y)))
        .filter(|cell| cell.x % 6 == 2 && cell.y % 6 == 2)
        .find(|&cell| ok(cell));
    lattice.or_else(|| {
        (0..H)
            .flat_map(|y| (0..W).map(move |x| CellPos::new(x, y)))
            .find(|&cell| ok(cell))
    })
}

/// Builds the tree of `country`; the problems found, and the number of items of the tree.
fn country(country: &str) -> (Vec<String>, usize) {
    let definitions = shipped();
    let rules = definitions.rules().expect("rules").clone();
    let id = rules.country_id(country).expect("country");
    let soviet = ["libya", "iraq", "cuba", "russia"].contains(&country);
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 5 });
    let cells = (0..W * H)
        .map(|index| {
            let water = index % W >= SHORE;
            Cell {
                passable: !water,
                water,
                buildable: !water,
                ore: 0,
            }
        })
        .collect();
    app.world_mut()
        .insert_resource(Grid::from_cells(W as u32, H as u32, cells).expect("a grid"));
    {
        let mut players = app.world_mut().resource_mut::<Players>();
        players.add(P0, 10_000_000).expect("a new player");
        assert!(players.set_country(P0, Some(id)));
        players.add(P1, 0).expect("a new player");
        let other = rules
            .country_id(if soviet { "america" } else { "russia" })
            .expect("country");
        assert!(players.set_country(P1, Some(other)));
    }
    app.world_mut().insert_resource(definitions.clone());
    let (yard, power) = if soviet {
        ("soviet_construction_yard", "tesla_reactor")
    } else {
        ("construction_yard", "power_plant")
    };
    spawn_building(app.world_mut(), P0, yard, CellPos::new(26, 26)).expect("yard");
    for index in 0..14 {
        spawn_building(app.world_mut(), P0, power, CellPos::new(2 + 4 * index, 66)).expect("power");
    }
    // Eyes along the shore and across the land, so that every site is visible.
    for y in (2..H).step_by(8) {
        for x in [6, 18, 30, 42, 54] {
            let _ = spawn_unit(app.world_mut(), P0, "attack_dog", CellPos::new(x, y));
        }
    }
    tick(&mut app, Vec::new());

    let tree: BTreeSet<ProductionItem> = country_tree(&definitions, id);
    let mut done: BTreeSet<ProductionItem> = BTreeSet::new();
    let mut problems = Vec::new();
    let mut failed: BTreeSet<ProductionItem> = BTreeSet::new();
    loop {
        let mut progress = false;
        for &item in &tree {
            if done.contains(&item) || failed.contains(&item) {
                continue;
            }
            let kinds: Vec<BuildingKind> = owned(&mut app).into_iter().map(|(k, _)| k).collect();
            let state = app
                .world()
                .resource::<Players>()
                .get(P0)
                .expect("P0")
                .clone();
            if check_buildable(&definitions, &state, true, &kinds, item).is_err() {
                continue;
            }
            let label = name(&definitions, item);
            let terms = definitions.production(item).expect("terms").clone();
            let before = credits(&app);
            let before_units = match item {
                ProductionItem::Unit(kind) => units_of(&mut app, kind),
                ProductionItem::Building(_) => 0,
            };
            let events = tick(
                &mut app,
                vec![IssuedCommand {
                    player: P0,
                    command: Command::QueueProduction { item },
                }],
            );
            let rejected = rejections(&events);
            if !rejected.is_empty() {
                problems.push(format!(
                    "{country}: {label}: check_buildable is Ok but the queue command is \
                     rejected: {rejected:?}"
                ));
                failed.insert(item);
                continue;
            }
            let limit = terms.build_time * 2 + 400;
            let mut complete = false;
            for _ in 0..limit {
                let finished = match item {
                    ProductionItem::Unit(kind) => units_of(&mut app, kind) > before_units,
                    ProductionItem::Building(_) => {
                        let key = definitions.queue_of(item).expect("queue");
                        let state = app.world().resource::<Players>().get(P0).expect("P0");
                        state
                            .lane(key)
                            .first()
                            .is_some_and(|head| head.progress >= terms.build_time)
                    }
                };
                if finished {
                    complete = true;
                    break;
                }
                let events = tick(&mut app, Vec::new());
                let rejected = rejections(&events);
                if !rejected.is_empty() {
                    problems.push(format!(
                        "{country}: {label}: rejected while built: {rejected:?}"
                    ));
                }
            }
            if !complete {
                problems.push(format!(
                    "{country}: {label}: not complete after {limit} ticks (build time {}, cost {}, \
                     charged {})",
                    terms.build_time,
                    terms.cost,
                    before - credits(&app)
                ));
                failed.insert(item);
                // Leave the lane: cancel.
                tick(
                    &mut app,
                    vec![IssuedCommand {
                        player: P0,
                        command: Command::CancelProduction { item },
                    }],
                );
                continue;
            }
            let charged = before - credits(&app);
            if charged != terms.cost {
                problems.push(format!(
                    "{country}: {label}: charged {charged}, cost {}",
                    terms.cost
                ));
            }
            if let ProductionItem::Building(kind) = item {
                match site(&mut app, kind) {
                    None => {
                        problems.push(format!("{country}: {label}: no site to place it"));
                        failed.insert(item);
                        tick(
                            &mut app,
                            vec![IssuedCommand {
                                player: P0,
                                command: Command::CancelProduction { item },
                            }],
                        );
                        continue;
                    }
                    Some(origin) => {
                        let events = tick(
                            &mut app,
                            vec![IssuedCommand {
                                player: P0,
                                command: Command::PlaceBuilding {
                                    building: kind,
                                    origin,
                                },
                            }],
                        );
                        let rejected = rejections(&events);
                        if !rejected.is_empty() {
                            problems.push(format!(
                                "{country}: {label}: check_placement is Ok at {origin:?} but \
                                 the command is rejected: {rejected:?}"
                            ));
                            failed.insert(item);
                            continue;
                        }
                    }
                }
            }
            done.insert(item);
            progress = true;
        }
        if !progress {
            break;
        }
    }
    for &item in &tree {
        if !done.contains(&item) && !failed.contains(&item) {
            let kinds: Vec<BuildingKind> = owned(&mut app).into_iter().map(|(k, _)| k).collect();
            let state = app
                .world()
                .resource::<Players>()
                .get(P0)
                .expect("P0")
                .clone();
            problems.push(format!(
                "{country}: {} of its tree never became buildable: {:?}",
                name(&definitions, item),
                check_buildable(&definitions, &state, true, &kinds, item)
            ));
        }
    }
    if done.len() != tree.len() {
        problems.push(format!(
            "{country}: {} of the {} items of its tree were built",
            done.len(),
            tree.len()
        ));
    }
    // The sale of every building it owns now: the refund of its type to the credit (with the
    // shipped rules half its cost, and 300 for a refinery), and a wall is not sold.
    let rules = definitions.rules().expect("rules");
    let mut buildings: Vec<(sim::SimId, BuildingKind)> = {
        let mut query = app.world_mut().query::<(&sim::SimId, &Owner, &Building)>();
        query
            .iter(app.world())
            .filter(|(_, owner, _)| owner.0 == P0)
            .map(|(id, _, building)| (*id, building.kind))
            .collect()
    };
    buildings.sort();
    let mut sold = BTreeSet::new();
    for (id, kind) in buildings {
        let item = ProductionItem::Building(kind);
        let label = name(&definitions, item);
        let cost = definitions.production(item).expect("terms").cost;
        let sellable = definitions
            .metadata(item)
            .is_none_or(|entry| entry.sellable);
        let before = credits(&app);
        let events = tick(
            &mut app,
            vec![IssuedCommand {
                player: P0,
                command: Command::SellBuilding { building: id },
            }],
        );
        let refunds: Vec<i64> = events
            .iter()
            .filter_map(|event| match event {
                SimEvent::BuildingSold {
                    building, refund, ..
                } if *building == id => Some(*refund),
                _ => None,
            })
            .collect();
        // RA5 (docs/DECISIONS.md R58): the two refineries return the ini's `Soylent`, 300,
        // and every other type the cost times the refund percent (the Ore Purifier's
        // `Soylent` is half its cost).
        let expected = match label.as_str() {
            "refinery" | "soviet_refinery" => 300,
            _ => cost * i64::from(rules.economy.refund_percent) / 100,
        };
        let fine = if sellable {
            refunds == [expected]
                && credits(&app) - before == expected
                && rejections(&events).is_empty()
        } else {
            refunds.is_empty()
                && credits(&app) == before
                && rejections(&events) == [RejectReason::NotSellable(id)]
        };
        if !fine {
            problems.push(format!(
                "{country}: {label}: sold for {refunds:?} (cost {cost}, sellable {sellable}), \
                 credits {before} to {}, rejected {:?}",
                credits(&app),
                rejections(&events)
            ));
        }
        sold.insert(kind);
    }
    if sold.len() < 18 {
        problems.push(format!(
            "{country}: only {} building types were sold",
            sold.len()
        ));
    }
    (problems, tree.len())
}

#[test]
fn every_item_of_every_countrys_tree_is_built_through_commands_and_every_building_sold() {
    let handles: Vec<_> = [
        "america",
        "korea",
        "france",
        "germany",
        "great_britain",
        "libya",
        "iraq",
        "cuba",
        "russia",
    ]
    .into_iter()
    .map(|id| std::thread::spawn(move || country(id)))
    .collect();
    let mut problems = Vec::new();
    let mut sizes = Vec::new();
    for handle in handles {
        let (found, size) = handle.join().expect("a country");
        problems.extend(found);
        sizes.push(size);
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
    // The trees are those of the shipped arsenal: 37 or 38 items a country at RA4 (the two
    // sides' buildings, defences, infantry, vehicles, ships and aircraft, with the country's
    // own special), so the loop above cannot have passed over nearly nothing.
    assert!(sizes.iter().all(|&size| size >= 37), "{sizes:?}");
}
