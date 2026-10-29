//! Futa, the game's own Super Me building (docs/DECISIONS.md R218, R220), on the shipped data
//! (`assets/data`, loaded with `load_game_ruleset`): only a Super Me player may build it, it may
//! be placed on any free ground of the map, its beam of light strikes units on the ground and in
//! the air far away, it is hard to destroy and cannot be captured, bombed or entered, and while it
//! stands its owner has unlimited power and may place buildings near it. `load_ruleset` alone
//! keeps the Red Alert 2 ruleset without it.
//!
//! Driven through `CommandQueue` and `SimSchedule`.

use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    Building, BuildingKind, CellPos, Command, CommandQueue, Controller, Definitions, Grid, Health,
    IssuedCommand, MatchOptions, Owner, PlacementError, PlayerId, PlayerSetup, Players,
    ProductionItem, RejectReason, SimEvent, SimId, SimPlugin, SimSchedule, check_buildable,
    check_placement, load_ai, load_game_ruleset, load_map, load_ruleset, load_skirmish,
    map_asset_path, setup_match_configured, spawn_building, spawn_unit,
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

fn futa_kind(app: &App) -> BuildingKind {
    app.world()
        .resource::<Definitions>()
        .building_kind("futa")
        .expect("futa")
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

/// The origin of the valid site for a ready `building` farthest from the human player's yard
/// (Chebyshev), with what the human player's state allows now.
fn far_site(app: &mut App, building: BuildingKind) -> CellPos {
    let start = yard(app);
    let grid = app.world().resource::<Grid>().clone();
    let definitions = app.world().resource::<Definitions>().clone();
    let state = human(app).clone();
    let mut best: Option<(i32, CellPos)> = None;
    for y in 0..grid.height() as i32 {
        for x in 0..grid.width() as i32 {
            let origin = CellPos::new(x, y);
            let distance = (x - start.x).abs().max((y - start.y).abs());
            if best.is_none_or(|(d, _)| distance > d)
                && check_placement(
                    &grid,
                    &definitions,
                    &state,
                    std::iter::empty(),
                    building,
                    origin,
                )
                .is_ok()
            {
                best = Some((distance, origin));
            }
        }
    }
    best.expect("a site").1
}

/// Produces Futa and places it on the free site farthest from the base; returns its id and
/// origin.
fn place_futa(app: &mut App) -> (SimId, CellPos) {
    let kind = futa_kind(app);
    produce(app, ProductionItem::Building(kind));
    let origin = far_site(app, kind);
    let events = tick(
        app,
        vec![Command::PlaceBuilding {
            building: kind,
            origin,
        }],
    );
    let id = events
        .iter()
        .find_map(|e| match e {
            SimEvent::BuildingPlaced {
                player,
                building,
                kind: k,
            } if *player == HUMAN && *k == kind => Some(*building),
            _ => None,
        })
        .unwrap_or_else(|| panic!("Futa was not placed: {events:?}"));
    (id, origin)
}

fn destroy(app: &mut App, id: SimId) {
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
    tick(app, Vec::new());
    tick(app, Vec::new());
}

#[test]
fn futa_is_the_games_own_building_and_load_ruleset_stays_the_ra2_ruleset() {
    let ra2 = load_ruleset(data()).unwrap_or_else(|error| panic!("{error}"));
    assert!(ra2.building_kind("futa").is_none());
    assert!(ra2.weapon_kind("futa_beam").is_none());
    assert!(ra2.super_me_buildings().is_empty());

    let game = load_game_ruleset(data()).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(game.units(), ra2.units(), "no unit is added");
    assert_eq!(game.buildings().len(), ra2.buildings().len() + 1);
    assert_eq!(&game.buildings()[..ra2.buildings().len()], ra2.buildings());
    let kind = game.building_kind("futa").expect("futa");
    let grants = *game.super_me_building(kind).expect("a Super Me building");
    assert!(grants.unlimited_power && grants.anywhere);
    assert_eq!(grants.build_radius, 6);
    let item = ProductionItem::Building(kind);
    let entry = game.metadata(item).expect("arsenal entry");
    assert_eq!(entry.label, "Futa");
    assert_eq!(entry.art, "futa");
    assert_eq!(entry.category, sim::Category::Armory, "a defence");
    assert!(entry.side.is_none() && entry.owner_sides.is_empty());
    assert!(!entry.powered, "it fires whatever the power");
    // No one takes it or removes it cheaply.
    assert!(!entry.capturable && !entry.spyable && !entry.can_c4 && !entry.bombable);

    // Its one weapon is a beam of the Prism Tank's light that engages ground and air, much
    // further and harder than a Prism Tower.
    let terms = game.building_combat(kind).expect("combat terms");
    let beam = game.weapon(terms.weapon.expect("a weapon")).unwrap();
    assert_eq!(beam.id, "futa_beam");
    assert!(beam.anti_air && beam.anti_ground);
    assert_eq!(beam.projectile_speed, 0, "light, not a shell");
    assert_eq!(game.warhead(beam.warhead).unwrap().id, "prism_warhead");
    let tower = game.building_kind("prism_tower").unwrap();
    let tower_terms = game.building_combat(tower).unwrap();
    let tower_beam = game.weapon(tower_terms.weapon.unwrap()).unwrap();
    assert!(beam.range >= 2 * tower_beam.range);
    assert!(beam.damage * tower_beam.reload as i32 > 5 * tower_beam.damage * beam.reload as i32);
    // Very hard to destroy: far more hit points than any building of the ruleset.
    let strongest = (0..ra2.buildings().len())
        .filter_map(|i| ra2.building_combat(BuildingKind(i as u16)))
        .map(|t| t.hit_points)
        .max()
        .unwrap();
    assert!(
        terms.hit_points >= 3 * strongest,
        "{} vs {strongest}",
        terms.hit_points
    );
}

#[test]
fn only_a_super_me_player_may_queue_futa() {
    for super_me in [false, true] {
        let mut app = game(super_me);
        tick(&mut app, Vec::new());
        let item = ProductionItem::Building(futa_kind(&app));
        let owned = {
            let mut query = app.world_mut().query::<(&Owner, &Building)>();
            query
                .iter(app.world())
                .map(|(_, building)| building.kind)
                .collect::<Vec<_>>()
        };
        let world = app.world();
        let definitions = world.resource::<Definitions>();
        let state = world.resource::<Players>().get(HUMAN).unwrap();
        let outcome = check_buildable(definitions, state, true, &owned, item);
        if super_me {
            assert_eq!(outcome, Ok(()));
        } else {
            assert_eq!(outcome, Err(RejectReason::NotInRoster(item)));
        }
        // The computer player never may.
        let ai = world.resource::<Players>().get(AI).unwrap();
        assert_eq!(
            check_buildable(definitions, ai, true, &owned, item),
            Err(RejectReason::NotInRoster(item))
        );
    }
}

#[test]
fn futa_is_placed_anywhere_on_the_map_and_there_is_no_limit() {
    let mut app = game(true);
    tick(&mut app, Vec::new());
    let kind = futa_kind(&app);
    let start = yard(&mut app);
    let (_, first) = place_futa(&mut app);
    // Far beyond the reach of any base building, and on ground the player had not seen.
    let distance = (first.x - start.x).abs().max((first.y - start.y).abs());
    assert!(distance > 20, "{distance} cells from the yard");
    // A second one, and its footprint must still be free.
    produce(&mut app, ProductionItem::Building(kind));
    let events = tick(
        &mut app,
        vec![Command::PlaceBuilding {
            building: kind,
            origin: first,
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
    let second = far_site(&mut app, kind);
    let events = tick(
        &mut app,
        vec![Command::PlaceBuilding {
            building: kind,
            origin: second,
        }],
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SimEvent::BuildingPlaced { kind: k, .. } if *k == kind)),
        "{events:?}"
    );
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

    let (id, _) = place_futa(&mut app);
    tick(&mut app, Vec::new());
    let state = human(&app);
    assert!(state.unlimited_power());
    assert!(!state.low_power());
    assert!(state.power_produced() >= state.power_consumed());

    // Once it is destroyed the power is gone.
    destroy(&mut app, id);
    assert!(!human(&app).unlimited_power());
    assert!(human(&app).low_power());
}

#[test]
fn buildings_may_be_placed_near_futa_far_from_the_base() {
    let mut app = game(true);
    tick(&mut app, Vec::new());
    let (id, origin) = place_futa(&mut app);
    // Its sight shows the ground around it before anything is placed there.
    for _ in 0..3 {
        tick(&mut app, Vec::new());
    }
    let anchors = human(&app).build_anchors().to_vec();
    assert_eq!(anchors.len(), 1);
    assert_eq!((anchors[0].0.origin, anchors[0].1), (origin, 6));

    let definitions = app.world().resource::<Definitions>().clone();
    let plant = definitions.building_kind("power_plant").unwrap();
    produce(&mut app, ProductionItem::Building(plant));
    let near = |app: &App| {
        (-8..=9)
            .flat_map(|dy| (-8..=9).map(move |dx| (dx, dy)))
            .map(|(dx, dy)| CellPos::new(origin.x + dx, origin.y + dy))
            .map(|site| {
                let grid = app.world().resource::<Grid>();
                let state = app.world().resource::<Players>().get(HUMAN).unwrap();
                (
                    site,
                    check_placement(grid, &definitions, state, std::iter::empty(), plant, site),
                )
            })
            .collect::<Vec<_>>()
    };
    let site = near(&app)
        .into_iter()
        .find_map(|(site, check)| check.is_ok().then_some(site))
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
    destroy(&mut app, id);
    produce(&mut app, ProductionItem::Building(plant));
    assert!(human(&app).build_anchors().is_empty());
    let refused = near(&app).into_iter().any(|(_, check)| {
        matches!(
            check,
            Err(RejectReason::Placement(PlacementError::TooFar { .. }))
        )
    });
    assert!(refused);
}

#[test]
fn futas_light_strikes_units_on_the_ground_and_in_the_air_far_away() {
    let mut app = game(true);
    tick(&mut app, Vec::new());
    let (id, origin) = place_futa(&mut app);
    // Twelve cells out: beyond the reach of a Prism Tower (8) and of the targets' own weapons.
    let centre = CellPos::new(origin.x + 1, origin.y + 1);
    let spawn = |app: &mut App, unit: &str, reach: i32, skip: usize| {
        (-reach..=reach)
            .flat_map(|dy| (-reach..=reach).map(move |dx| (dx, dy)))
            .filter(|(dx, dy)| dx.abs().max(dy.abs()) == reach)
            .skip(skip)
            .find_map(|(dx, dy)| {
                let cell = CellPos::new(centre.x + dx, centre.y + dy);
                spawn_unit(app.world_mut(), AI, unit, cell).ok()
            })
            .unwrap_or_else(|| panic!("room for a {unit}"))
    };
    let tank = spawn(&mut app, "rhino", 12, 0);
    let kirov = spawn(&mut app, "kirov", 11, 40);
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
                assert_eq!(definitions.weapon(weapon).unwrap().id, "futa_beam");
                ground |= target == tank;
                air |= target == kirov;
            }
        }
        if ground && air {
            return;
        }
    }
    panic!("Futa fired at the ground: {ground}, at the air: {air}");
}
