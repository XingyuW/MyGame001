//! Futa, the game's own Super Me unit (docs/DECISIONS.md R218), on the shipped data
//! (`assets/data`, loaded with `load_game_ruleset`): only a Super Me player may build it, a
//! finished one waits in its queue and is placed on any free cell of the map, it fires at units
//! on the ground and in the air, and while it stands its owner has unlimited power and may place
//! buildings near it. `load_ruleset` alone keeps the Red Alert 2 ruleset without it.
//!
//! Driven through `CommandQueue` and `SimSchedule`.

use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    Building, CellPos, Command, CommandQueue, Controller, Definitions, Grid, Health, IssuedCommand,
    MatchOptions, Mobile, Owner, PlacementError, PlayerId, PlayerSetup, Players, ProductionItem,
    RejectReason, SimEvent, SimId, SimPlugin, SimSchedule, Unit, check_buildable, load_ai,
    load_game_ruleset, load_map, load_ruleset, load_skirmish, map_asset_path,
    setup_match_configured, spawn_building, spawn_unit,
};

const HUMAN: PlayerId = PlayerId(0);
const AI: PlayerId = PlayerId(1);

fn data() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data")
}

/// A match on the demo map: the American human player, with or without Super Me, against a
/// Russian computer player; each starts with its Construction Yard.
fn game(super_me: bool) -> App {
    let definitions = load_game_ruleset(data()).unwrap_or_else(|error| panic!("{error}"));
    let skirmish =
        load_skirmish(data().join("skirmish.ron"), &definitions).unwrap_or_else(|e| panic!("{e}"));
    let ai = load_ai(data().join("ai.ron"), &definitions).unwrap_or_else(|e| panic!("{e}"));
    let map = skirmish
        .maps
        .iter()
        .position(|map| map.id == "demo")
        .expect("demo map");
    let grid = load_map(data().join("..").join(map_asset_path("demo")))
        .unwrap_or_else(|e| panic!("{e}"))
        .grid;
    let faction = |id: &str| {
        skirmish
            .factions
            .iter()
            .position(|faction| faction.id == id)
            .unwrap_or_else(|| panic!("faction {id}"))
    };
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 5 });
    app.world_mut().insert_resource(grid);
    app.world_mut().insert_resource(definitions);
    let players = [
        PlayerSetup {
            player: HUMAN,
            faction: faction("america"),
            start: 0,
            controller: Controller::Human,
        },
        PlayerSetup {
            player: AI,
            faction: faction("russia"),
            start: 1,
            controller: Controller::Ai { difficulty: 0 },
        },
    ];
    let options = MatchOptions {
        super_players: if super_me { vec![HUMAN] } else { Vec::new() },
        mcv_start: false,
        ..MatchOptions::default()
    };
    setup_match_configured(app.world_mut(), &skirmish, &ai, map, &players, &options)
        .unwrap_or_else(|e| panic!("{e:?}"));
    app
}

fn tick(app: &mut App, commands: Vec<Command>) -> Vec<SimEvent> {
    {
        let mut queue = app.world_mut().resource_mut::<CommandQueue>();
        for command in commands {
            queue.push(IssuedCommand {
                player: HUMAN,
                command,
            });
        }
    }
    app.world_mut().run_schedule(SimSchedule);
    app.world()
        .resource::<Messages<SimEvent>>()
        .iter_current_update_messages()
        .cloned()
        .collect()
}

fn futa(app: &App) -> ProductionItem {
    ProductionItem::Unit(
        app.world()
            .resource::<Definitions>()
            .unit_kind("futa")
            .expect("futa"),
    )
}

fn human(app: &App) -> &sim::PlayerState {
    app.world().resource::<Players>().get(HUMAN).expect("human")
}

fn yard(app: &mut App) -> CellPos {
    let mut query = app.world_mut().query::<(&Owner, &Building)>();
    query
        .iter(app.world())
        .find(|(owner, _)| owner.0 == HUMAN)
        .map(|(_, building)| building.footprint.origin)
        .expect("the human player's yard")
}

/// Queues `item` and ticks until it is complete.
fn produce(app: &mut App, item: ProductionItem) {
    let events = tick(app, vec![Command::QueueProduction { item }]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
    for _ in 0..4000 {
        if tick(app, Vec::new()).iter().any(|e| {
            matches!(e, SimEvent::ProductionComplete { player, item: done }
                if *player == HUMAN && *done == item)
        }) {
            return;
        }
    }
    panic!("{item:?} was not completed");
}

/// The free land cell farthest from the human player's yard (Chebyshev), so that no rule of
/// distance from its base could allow it.
fn far_cell(app: &mut App) -> CellPos {
    let start = yard(app);
    let grid = app.world().resource::<Grid>();
    let mut best: Option<(i32, CellPos)> = None;
    for y in 2..grid.height() as i32 - 2 {
        for x in 2..grid.width() as i32 - 2 {
            let cell = CellPos::new(x, y);
            let open = (-1..=1).all(|dy| {
                (-1..=1).all(|dx| {
                    let near = CellPos::new(x + dx, y + dy);
                    grid.passable_for(near, sim::Domain::Land)
                        && grid.occupant_for(near, sim::Domain::Land).is_none()
                })
            });
            let distance = (x - start.x).abs().max((y - start.y).abs());
            if open && best.is_none_or(|(d, _)| distance > d) {
                best = Some((distance, cell));
            }
        }
    }
    best.expect("a free cell").1
}

/// Produces a Futa and places it on `cell`; returns its id.
fn place_futa(app: &mut App, cell: CellPos) -> SimId {
    let item = futa(app);
    let ProductionItem::Unit(kind) = item else {
        unreachable!()
    };
    produce(app, item);
    let events = tick(app, vec![Command::PlaceUnit { unit: kind, cell }]);
    events
        .iter()
        .find_map(|e| match e {
            SimEvent::UnitPlaced {
                player,
                unit,
                kind: k,
            } if *player == HUMAN && *k == kind => Some(*unit),
            _ => None,
        })
        .unwrap_or_else(|| panic!("Futa was not placed: {events:?}"))
}

fn units_of(app: &mut App, item: ProductionItem) -> Vec<(SimId, CellPos)> {
    let ProductionItem::Unit(kind) = item else {
        unreachable!()
    };
    let mut query = app.world_mut().query::<(&SimId, &Owner, &Unit, &Mobile)>();
    query
        .iter(app.world())
        .filter(|(_, owner, unit, _)| owner.0 == HUMAN && unit.kind == kind)
        .map(|(id, _, _, mobile)| (*id, mobile.cell))
        .collect()
}

#[test]
fn futa_is_the_games_own_unit_and_load_ruleset_stays_the_ra2_ruleset() {
    let ra2 = load_ruleset(data()).unwrap_or_else(|error| panic!("{error}"));
    assert!(ra2.unit_kind("futa").is_none());
    assert!(ra2.super_me_units().is_empty());

    let game = load_game_ruleset(data()).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(game.units().len(), ra2.units().len() + 1);
    let kind = game.unit_kind("futa").expect("futa");
    let grants = *game.super_me_unit(kind).expect("a Super Me unit");
    assert!(grants.unlimited_power && grants.placed && grants.hover);
    assert_eq!(grants.build_radius, 6);
    let item = ProductionItem::Unit(kind);
    let entry = game.metadata(item).expect("arsenal entry");
    assert_eq!(entry.label, "Futa");
    assert_eq!(entry.art, "futa");
    assert!(entry.immune_psionics, "no one may mind control it");
    assert!(entry.owner_sides.is_empty() && entry.side.is_none());
    assert_eq!(game.lane(item), sim::Lane::Placed);
    // One weapon for the ground and one for the air.
    let terms = game.unit_combat(kind).expect("combat terms");
    let [ground, air] = terms
        .weapons(false)
        .map(|w| game.weapon(w.expect("weapon")).unwrap());
    assert!(ground.anti_ground && !ground.anti_air, "{}", ground.id);
    assert!(air.anti_air, "{}", air.id);
    // The RA2 part of the definitions is the same in both.
    for (index, unit) in ra2.units().iter().enumerate() {
        assert_eq!(&game.units()[index], unit);
    }
}

#[test]
fn only_a_super_me_player_may_queue_futa() {
    for super_me in [false, true] {
        let mut app = game(super_me);
        tick(&mut app, Vec::new());
        let item = futa(&app);
        let owned = {
            let mut query = app.world_mut().query::<(&Owner, &Building)>();
            query
                .iter(app.world())
                .filter(|(owner, _)| owner.0 == HUMAN)
                .map(|(_, building)| building.kind)
                .collect::<Vec<_>>()
        };
        let world = app.world();
        let state = world.resource::<Players>().get(HUMAN).unwrap();
        let outcome = check_buildable(world.resource::<Definitions>(), state, true, &owned, item);
        if super_me {
            assert_eq!(outcome, Ok(()));
        } else {
            assert_eq!(outcome, Err(RejectReason::NotInRoster(item)));
        }
        // The computer player never may.
        let ai = world.resource::<Players>().get(AI).unwrap();
        assert_eq!(
            check_buildable(world.resource::<Definitions>(), ai, true, &owned, item),
            Err(RejectReason::NotInRoster(item))
        );
    }
}

#[test]
fn a_finished_futa_waits_and_is_placed_anywhere_on_the_map() {
    let mut app = game(true);
    tick(&mut app, Vec::new());
    let item = futa(&app);
    let ProductionItem::Unit(kind) = item else {
        unreachable!()
    };
    produce(&mut app, item);
    for _ in 0..40 {
        tick(&mut app, Vec::new());
    }
    assert!(units_of(&mut app, item).is_empty(), "it left no factory");
    let key = app
        .world()
        .resource::<Definitions>()
        .queue_of(item)
        .unwrap();
    assert_eq!(human(&app).lane(key).len(), 1, "it waits in its queue");

    // A cell under a building is refused.
    let start = yard(&mut app);
    let events = tick(
        &mut app,
        vec![Command::PlaceUnit {
            unit: kind,
            cell: start,
        }],
    );
    assert!(
        events.iter().any(|e| matches!(
            e,
            SimEvent::CommandRejected {
                reason: RejectReason::Placement(PlacementError::Overlap { .. }),
                ..
            }
        )),
        "{events:?}"
    );

    let cell = far_cell(&mut app);
    let events = tick(&mut app, vec![Command::PlaceUnit { unit: kind, cell }]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SimEvent::UnitPlaced { kind: k, .. } if *k == kind)),
        "{events:?}"
    );
    assert_eq!(
        units_of(&mut app, item)
            .into_iter()
            .map(|(_, c)| c)
            .collect::<Vec<_>>(),
        [cell]
    );
    assert!(human(&app).lane(key).is_empty());

    // Nothing is ready any more.
    let events = tick(&mut app, vec![Command::PlaceUnit { unit: kind, cell }]);
    assert!(events.iter().any(|e| matches!(e,
        SimEvent::CommandRejected { reason: RejectReason::UnitNotReady(k), .. } if *k == kind)));

    // There is no limit: a second one is built and placed as well.
    let second = CellPos::new(cell.x + 2, cell.y);
    let second = if app
        .world()
        .resource::<Grid>()
        .passable_for(second, sim::Domain::Land)
    {
        second
    } else {
        CellPos::new(cell.x - 2, cell.y)
    };
    place_futa(&mut app, second);
    assert_eq!(units_of(&mut app, item).len(), 2);
}

#[test]
fn futa_gives_its_owner_unlimited_power_while_it_stands() {
    let mut app = game(true);
    // Power consumers and no power plant: low power.
    for id in ["war_factory", "barracks", "tech_center"] {
        let start = yard(&mut app);
        let placed = (4..30).any(|r| {
            spawn_building(
                app.world_mut(),
                HUMAN,
                id,
                CellPos::new(start.x + r, start.y),
            )
            .is_ok()
                || spawn_building(
                    app.world_mut(),
                    HUMAN,
                    id,
                    CellPos::new(start.x, start.y + r),
                )
                .is_ok()
        });
        assert!(placed, "no ground for {id}");
    }
    tick(&mut app, Vec::new());
    assert!(human(&app).low_power());
    assert!(!human(&app).unlimited_power());

    let cell = far_cell(&mut app);
    let id = place_futa(&mut app, cell);
    tick(&mut app, Vec::new());
    let state = human(&app);
    assert!(state.unlimited_power());
    assert!(!state.low_power());
    assert!(state.power_produced() >= state.power_consumed());

    // Once it is destroyed the power is gone.
    let entity = {
        let mut query = app
            .world_mut()
            .query::<(bevy_ecs::entity::Entity, &SimId)>();
        query
            .iter(app.world())
            .find(|(_, sim_id)| **sim_id == id)
            .map(|(entity, _)| entity)
            .expect("Futa")
    };
    app.world_mut()
        .get_mut::<Health>(entity)
        .unwrap()
        .hit_points = 0;
    tick(&mut app, Vec::new());
    tick(&mut app, Vec::new());
    assert!(!human(&app).unlimited_power());
    assert!(human(&app).low_power());
}

#[test]
fn buildings_may_be_placed_near_futa_far_from_the_base() {
    let mut app = game(true);
    tick(&mut app, Vec::new());
    let cell = far_cell(&mut app);
    place_futa(&mut app, cell);
    // Its sight shows the ground around it before anything is placed there.
    for _ in 0..3 {
        tick(&mut app, Vec::new());
    }
    assert_eq!(human(&app).build_anchors(), &[(cell, 6)]);

    let definitions = app.world().resource::<Definitions>().clone();
    let plant = definitions.building_kind("power_plant").unwrap();
    produce(&mut app, ProductionItem::Building(plant));
    // The first 2x2 site within the reach of Futa that is free.
    let grid = app.world().resource::<Grid>().clone();
    let state = human(&app).clone();
    let site = (-6..=4)
        .flat_map(|dy| (-6..=4).map(move |dx| (dx, dy)))
        .map(|(dx, dy)| CellPos::new(cell.x + dx, cell.y + dy))
        .find(|&origin| {
            sim::check_placement(
                &grid,
                &definitions,
                &state,
                std::iter::empty(),
                plant,
                origin,
            )
            .is_ok()
        })
        .expect("a site near Futa");
    let events = tick(
        &mut app,
        vec![Command::PlaceBuilding {
            building: plant,
            origin: site,
        }],
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SimEvent::BuildingPlaced { kind, .. } if *kind == plant)),
        "{events:?}"
    );

    // Once Futa is destroyed the ground around it is too far from every building again.
    let futa_kind = definitions.unit_kind("futa").unwrap();
    let entity = {
        let mut query = app
            .world_mut()
            .query::<(bevy_ecs::entity::Entity, &Unit, &Owner)>();
        query
            .iter(app.world())
            .find(|(_, unit, owner)| unit.kind == futa_kind && owner.0 == HUMAN)
            .map(|(entity, _, _)| entity)
            .expect("Futa")
    };
    app.world_mut()
        .get_mut::<Health>(entity)
        .unwrap()
        .hit_points = 0;
    tick(&mut app, Vec::new());
    produce(&mut app, ProductionItem::Building(plant));
    tick(&mut app, Vec::new());
    assert!(human(&app).build_anchors().is_empty());
    let grid = app.world().resource::<Grid>().clone();
    let state = human(&app).clone();
    let refused = (-6..=4)
        .flat_map(|dy| (-6..=4).map(move |dx| (dx, dy)))
        .map(|(dx, dy)| CellPos::new(cell.x + dx, cell.y + dy))
        .filter_map(|origin| {
            sim::check_placement(
                &grid,
                &definitions,
                &state,
                std::iter::empty(),
                plant,
                origin,
            )
            .err()
        })
        .any(|reason| {
            matches!(
                reason,
                RejectReason::Placement(PlacementError::TooFar { .. })
            )
        });
    assert!(refused);
}

#[test]
fn futa_fires_at_units_on_the_ground_and_in_the_air() {
    let mut app = game(true);
    tick(&mut app, Vec::new());
    let cell = far_cell(&mut app);
    let id = place_futa(&mut app, cell);
    let tank = spawn_unit(
        app.world_mut(),
        AI,
        "rhino",
        CellPos::new(cell.x + 3, cell.y),
    )
    .or_else(|_| {
        spawn_unit(
            app.world_mut(),
            AI,
            "rhino",
            CellPos::new(cell.x - 3, cell.y),
        )
    })
    .expect("room for a tank");
    let kirov = spawn_unit(
        app.world_mut(),
        AI,
        "kirov",
        CellPos::new(cell.x, cell.y + 3),
    )
    .or_else(|_| {
        spawn_unit(
            app.world_mut(),
            AI,
            "kirov",
            CellPos::new(cell.x, cell.y - 3),
        )
    })
    .expect("room for an airship");
    let definitions = app.world().resource::<Definitions>().clone();
    let (mut ground, mut air) = (false, false);
    for _ in 0..400 {
        for event in tick(&mut app, Vec::new()) {
            if let SimEvent::WeaponFired {
                attacker,
                target,
                weapon,
            } = event
                && attacker == id
            {
                let weapon = definitions.weapon(weapon).unwrap();
                if target == tank {
                    assert!(weapon.anti_ground, "{}", weapon.id);
                    ground = true;
                }
                if target == kirov {
                    assert!(weapon.anti_air, "{}", weapon.id);
                    air = true;
                }
            }
        }
        if ground && air {
            return;
        }
    }
    panic!("Futa fired at the ground: {ground}, at the air: {air}");
}
