//! RA5 neutral tech buildings, tested on shipped rules through ordinary commands.
use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    Building, Cell, CellPos, Command, CommandQueue, Definitions, Domain, Grid, IssuedCommand,
    Mobile, Owner, PlayerId, Players, ProductionItem, SimEvent, SimId, SimPlugin, SimSchedule,
    Unit, load_ruleset, spawn_building, spawn_unit,
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

fn entity(app: &mut App, id: SimId) -> Option<bevy_ecs::entity::Entity> {
    let world = app.world_mut();
    world
        .query::<(bevy_ecs::entity::Entity, &SimId)>()
        .iter(world)
        .find(|(_, found)| **found == id)
        .map(|(entity, _)| entity)
}
fn owner(app: &mut App, id: SimId) -> Option<PlayerId> {
    let entity = entity(app, id)?;
    app.world().get::<Owner>(entity).map(|owner| owner.0)
}
fn hp(app: &mut App, id: SimId) -> i32 {
    let entity = entity(app, id).expect("object exists");
    app.world()
        .get::<sim::Health>(entity)
        .expect("health")
        .hit_points
}

#[test]
fn ra5_engineer_captures_each_tech_and_is_consumed_but_the_hut_is_not_capturable() {
    for name in ["oil_derrick", "hospital", "airport", "tech_outpost"] {
        let mut app = world();
        let tech = put(&mut app, sim::NEUTRAL, name, 10, 10);
        let engineer = put(&mut app, P0, "engineer", 9, 10);
        tick(&mut app, vec![]);
        let events = tick(
            &mut app,
            vec![issue(
                P0,
                Command::Interact {
                    unit: engineer,
                    target: tech,
                },
            )],
        );
        assert!(rejected(&events).is_empty(), "{name}: {events:?}");
        assert_eq!(owner(&mut app, tech), Some(P0));
        assert!(entity(&mut app, engineer).is_none());
        let events = tick(
            &mut app,
            vec![issue(P0, Command::SellBuilding { building: tech })],
        );
        assert_eq!(rejected(&events).len(), 1, "{name} is unsellable");
    }
    let mut app = world();
    let hut = put(&mut app, sim::NEUTRAL, "bridge_hut", 10, 10);
    let engineer = put(&mut app, P0, "engineer", 9, 10);
    tick(&mut app, vec![]);
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: engineer,
                target: hut,
            },
        )],
    );
    assert_eq!(rejected(&events).len(), 1);
    assert_eq!(owner(&mut app, hut), Some(sim::NEUTRAL));
    assert!(entity(&mut app, engineer).is_some());
}

#[test]
fn ra5_derrick_pays_neutral_capture_startup_then_exactly_every_133_ticks() {
    let mut app = world();
    let derrick = put(&mut app, sim::NEUTRAL, "oil_derrick", 10, 10);
    let engineer = put(&mut app, P0, "engineer", 9, 10);
    tick(&mut app, vec![]);
    tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: engineer,
                target: derrick,
            },
        )],
    );
    assert_eq!(credits(&app, P0), 11_000);
    for _ in 0..132 {
        tick(&mut app, vec![]);
    }
    assert_eq!(credits(&app, P0), 11_000);
    tick(&mut app, vec![]);
    assert_eq!(credits(&app, P0), 11_020);
}

#[test]
fn ra5_neutral_is_not_hostile_or_a_competing_player_and_outpost_stays_inert() {
    let mut app = world();
    assert!(!app.world().resource::<Players>().hostile(P0, sim::NEUTRAL));
    assert!(
        app.world_mut()
            .resource_mut::<Players>()
            .add(sim::NEUTRAL, 0)
            .is_err()
    );
    let outpost = put(&mut app, sim::NEUTRAL, "tech_outpost", 10, 10);
    let tank = put(&mut app, P0, "grizzly", 12, 10);
    for _ in 0..150 {
        tick(&mut app, vec![]);
    }
    assert_eq!(hp(&mut app, outpost), 2000);
    assert_eq!(hp(&mut app, tank), 300);
    let metadata = defs(&app)
        .metadata(ProductionItem::Building(
            defs(&app).building_kind("tech_outpost").expect("outpost"),
        ))
        .expect("metadata")
        .clone();
    assert_eq!((metadata.vehicle_docks, metadata.docks), (1, 0));
}

#[test]
fn ra5_hospital_heals_entered_infantry_20_per_tick_and_returns_the_same_unit() {
    let mut app = world();
    let hospital = put(&mut app, P0, "hospital", 10, 10);
    let infantry = put(&mut app, P0, "gi", 9, 10);
    let e = entity(&mut app, infantry).expect("GI");
    app.world_mut()
        .entity_mut(e)
        .insert(sim::Health { hit_points: 5 });
    tick(&mut app, vec![]);
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: infantry,
                target: hospital,
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(entity(&mut app, infantry).is_none());
    assert_eq!(
        app.world().resource::<sim::SpecialState>().cargo[&hospital][0].hp,
        25
    );
    for _ in 0..5 {
        tick(&mut app, vec![]);
    }
    assert_eq!(hp(&mut app, infantry), 125);
    assert_eq!(owner(&mut app, infantry), Some(P0));
}

fn house(app: &mut App, x: i32, y: i32) -> SimId {
    let cell = CellPos::new(x, y);
    app.world_mut().resource_mut::<Grid>().set_terrain(
        cell,
        Cell {
            passable: false,
            buildable: false,
            ..OPEN
        },
    );
    let mut looks = vec![0; 48 * 48];
    looks[y as usize * 48 + x as usize] = 5 << 4;
    let props = sim::Props::from_looks(
        app.world().resource::<Grid>(),
        &looks,
        defs(app).armour_class("wood").expect("wood"),
    );
    let id = props.id_at(cell).expect("house");
    app.world_mut().insert_resource(props);
    id
}

#[test]
fn ra5_house_accepts_three_occupiers_refuses_tank_and_fourth_then_deploy_evicts() {
    let mut app = world();
    let house = house(&mut app, 10, 10);
    let units: Vec<_> = (0..3).map(|_| put(&mut app, P0, "gi", 9, 10)).collect();
    let extra = put(&mut app, P0, "gi", 9, 11);
    let tank = put(&mut app, P0, "grizzly", 10, 11);
    tick(&mut app, vec![]);
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: tank,
                target: house,
            },
        )],
    );
    assert_eq!(rejected(&events).len(), 1);
    let events = tick(
        &mut app,
        units
            .iter()
            .map(|&unit| {
                issue(
                    P0,
                    Command::Interact {
                        unit,
                        target: house,
                    },
                )
            })
            .collect(),
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert_eq!(
        app.world().resource::<sim::SpecialState>().cargo[&house].len(),
        3
    );
    for &unit in &units {
        assert!(entity(&mut app, unit).is_none());
    }
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: extra,
                target: house,
            },
        )],
    );
    assert_eq!(rejected(&events).len(), 1);
    let events = tick(&mut app, vec![issue(P0, Command::Deploy { unit: house })]);
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .garrisons
            .contains_key(&house)
    );
    for unit in units {
        assert_eq!(owner(&mut app, unit), Some(P0));
    }
    assert!(app.world().resource::<sim::Props>().get(house).is_some());
}

#[test]
fn ra5_two_gis_fire_occupation_weapon_every_ten_ticks_from_the_house() {
    let mut app = world();
    let house = house(&mut app, 10, 10);
    let a = put(&mut app, P0, "gi", 9, 10);
    let b = put(&mut app, P0, "gi", 9, 10);
    let enemy = put(&mut app, P1, "hospital", 14, 10);
    tick(&mut app, vec![]);
    let mut shots = Vec::new();
    for tick_index in 0..31 {
        let commands = if tick_index == 0 {
            vec![a, b]
                .into_iter()
                .map(|unit| {
                    issue(
                        P0,
                        Command::Interact {
                            unit,
                            target: house,
                        },
                    )
                })
                .collect()
        } else {
            vec![]
        };
        let events = tick(&mut app, commands);
        assert!(rejected(&events).is_empty());
        for event in events {
            if let SimEvent::WeaponFired {
                attacker,
                target,
                weapon,
            } = event
                && attacker == house
            {
                assert_eq!(target, enemy);
                assert_eq!(defs(&app).weapon(weapon).expect("weapon").damage, 30);
                shots.push(tick_index);
            }
        }
    }
    assert_eq!(shots, [0, 10, 20, 30]);
    assert!(hp(&mut app, enemy) < 800);
}

fn bridge_world(destructible: bool) -> (App, SimId, SimId) {
    let mut app = world();
    let mut looks = vec![0; 48 * 48];
    {
        let mut grid = app.world_mut().resource_mut::<Grid>();
        for y in 0..48 {
            for x in 22..=24 {
                grid.set_terrain(
                    CellPos::new(x, y),
                    Cell {
                        passable: false,
                        water: true,
                        buildable: false,
                        ore: 0,
                    },
                );
            }
        }
        for x in 22..=24 {
            grid.set_terrain(
                CellPos::new(x, 24),
                Cell {
                    buildable: false,
                    ..OPEN
                },
            );
            looks[24 * 48 + x as usize] = 8 << 4;
        }
        for (x, y) in [(21, 22), (25, 22)] {
            looks[y * 48 + x] = 15 << 4;
        }
        grid.mark_world_features(&looks);
    }
    sim::install_world_objects(app.world_mut(), destructible);
    let bridge = *app
        .world()
        .resource::<sim::Bridges>()
        .bridges
        .keys()
        .next()
        .expect("bridge");
    let hut = app
        .world()
        .resource::<Grid>()
        .building(CellPos::new(21, 22))
        .expect("west hut");
    (app, bridge, hut)
}

#[test]
fn ra5_bridge_collapses_as_one_run_and_consumes_engineer_to_restore_the_only_land_route() {
    let (mut app, bridge, hut) = bridge_world(true);
    assert_eq!(
        app.world().resource::<sim::Bridges>().bridges[&bridge]
            .cells
            .len(),
        3
    );
    let tank = put(&mut app, P0, "grizzly", 20, 24);
    let gun = put(&mut app, P0, "grizzly", 20, 25);
    tick(&mut app, vec![]);
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Attack {
                units: vec![tank, gun],
                target: bridge,
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    for _ in 0..3000 {
        if app.world().resource::<sim::Bridges>().bridges[&bridge].collapsed {
            break;
        }
        tick(&mut app, vec![]);
    }
    let tank_entity = entity(&mut app, tank).expect("tank");
    assert!(
        app.world().resource::<sim::Bridges>().bridges[&bridge].collapsed,
        "props={:?}, weapon={:?}",
        app.world().resource::<sim::Props>().get(bridge),
        app.world().get::<sim::Weapon>(tank_entity)
    );
    for x in 22..=24 {
        let cell = app
            .world()
            .resource::<Grid>()
            .cell(CellPos::new(x, 24))
            .expect("deck");
        assert!(cell.water && !cell.passable);
    }
    tick(
        &mut app,
        vec![issue(P0, Command::Stop { units: vec![tank] })],
    );
    let engineer = put(&mut app, P0, "engineer", 20, 22);
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: engineer,
                target: hut,
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(entity(&mut app, engineer).is_none());
    assert!(!app.world().resource::<sim::Bridges>().bridges[&bridge].collapsed);
    assert_eq!(
        app.world()
            .resource::<sim::Props>()
            .get(bridge)
            .expect("restored")
            .1
            .hit_points,
        1500
    );
    tick(
        &mut app,
        vec![issue(
            P0,
            Command::Move {
                units: vec![tank],
                target: CellPos::new(28, 24),
            },
        )],
    );
    for _ in 0..500 {
        tick(&mut app, vec![]);
    }
    let entity = entity(&mut app, tank).expect("tank");
    assert!(app.world().get::<Mobile>(entity).expect("mobile").cell.x > 24);
}

#[test]
fn ra5_bridge_destruction_off_and_immune_hut_ignore_explicit_fire() {
    let (mut app, bridge, hut) = bridge_world(false);
    let tank = put(&mut app, P0, "grizzly", 20, 24);
    tick(&mut app, vec![]);
    for target in [bridge, hut] {
        let events = tick(
            &mut app,
            vec![issue(
                P0,
                Command::Attack {
                    units: vec![tank],
                    target,
                },
            )],
        );
        assert!(rejected(&events).is_empty(), "{events:?}");
        for _ in 0..200 {
            tick(&mut app, vec![]);
        }
    }
    assert_eq!(
        app.world()
            .resource::<sim::Props>()
            .get(bridge)
            .expect("deck")
            .1
            .hit_points,
        1500
    );
    assert_eq!(hp(&mut app, hut), 2000);
    assert!(!app.world().resource::<sim::Bridges>().bridges[&bridge].collapsed);
}

#[test]
fn ra5_ai_repairs_the_only_land_route_with_a_consumed_engineer_and_wins() {
    only_land_route_ai(false);
}

#[test]
fn ra5_ai_remembers_a_seen_collapse_and_approaches_its_hut_after_losing_sight() {
    only_land_route_ai(true);
}

fn only_land_route_ai(lose_sight: bool) {
    let definitions = shipped();
    let mut skirmish =
        sim::load_skirmish(data_dir().join("skirmish.ron"), &definitions).expect("skirmish");
    skirmish.maps[0].starts = vec![CellPos::new(4, 20), CellPos::new(40, 20)];
    let ai = sim::load_ai(data_dir().join("ai.ron"), &definitions).expect("AI");
    let brutal = ai
        .difficulties
        .iter()
        .position(|d| d.id == "brutal")
        .expect("brutal");
    let players: Vec<_> = ["great_britain", "russia"]
        .iter()
        .enumerate()
        .map(|(i, country)| sim::PlayerSetup {
            player: PlayerId(i as u8),
            faction: skirmish
                .factions
                .iter()
                .position(|f| f.id == *country)
                .expect("country"),
            start: i,
            controller: sim::Controller::Ai { difficulty: brutal },
        })
        .collect();
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 23 });
    let mut grid = Grid::from_cells(48, 48, vec![OPEN; 48 * 48]).expect("grid");
    let mut looks = vec![0; 48 * 48];
    for y in 0..48 {
        for x in 22..=24 {
            grid.set_terrain(
                CellPos::new(x, y),
                Cell {
                    passable: false,
                    water: true,
                    buildable: false,
                    ore: 0,
                },
            );
        }
    }
    for x in 22..=24 {
        grid.set_terrain(
            CellPos::new(x, 24),
            Cell {
                buildable: false,
                ..OPEN
            },
        );
        looks[24 * 48 + x as usize] = 8 << 4;
    }
    for (x, y) in [(21, 22), (25, 22)] {
        looks[y * 48 + x] = 15 << 4;
    }
    for base_x in [10, 34] {
        for y in 17..=20 {
            for x in base_x..base_x + 4 {
                grid.set_terrain(
                    CellPos::new(x, y),
                    Cell {
                        ore: 2000,
                        buildable: false,
                        ..OPEN
                    },
                );
            }
        }
    }
    grid.mark_world_features(&looks);
    app.world_mut().insert_resource(grid);
    app.world_mut().insert_resource(definitions.clone());
    sim::setup_match_configured(
        app.world_mut(),
        &skirmish,
        &ai,
        0,
        &players,
        &sim::MatchOptions {
            mcv_start: false,
            unit_count: Some(0),
            ..Default::default()
        },
    )
    .expect("match");
    let bridge = *app
        .world()
        .resource::<sim::Bridges>()
        .bridges
        .keys()
        .next()
        .expect("bridge");
    let guns = vec![
        put(&mut app, P0, "grizzly", 20, 24),
        put(&mut app, P0, "grizzly", 20, 25),
    ];
    tick(&mut app, vec![]);
    let mut collapsed = false;
    for _ in 0..2400 {
        let events = tick(
            &mut app,
            vec![issue(
                P0,
                Command::Attack {
                    units: guns.clone(),
                    target: bridge,
                },
            )],
        );
        assert!(rejected(&events).is_empty(), "{events:?}");
        if app.world().resource::<sim::Bridges>().bridges[&bridge].collapsed {
            collapsed = true;
            break;
        }
    }
    assert!(
        collapsed,
        "the commanded attack must collapse the complete route before the first wave"
    );
    // Give the AI a normal observation of the collapse, then remove the scouts' sight.
    tick(&mut app, vec![]);
    tick(
        &mut app,
        vec![issue(
            P0,
            if lose_sight {
                Command::Move {
                    units: guns,
                    target: CellPos::new(8, 10),
                }
            } else {
                Command::Stop { units: guns }
            },
        )],
    );
    let mut lost_sight = false;
    let mut approached_from_memory = false;
    let mut engineers = std::collections::BTreeSet::new();
    let mut repaired = false;
    let mut consumed = false;
    let mut crossed = false;
    let limit = 72_000;
    while app.world().resource::<sim::SimTick>().0 < limit {
        let events = tick(&mut app, vec![]);
        assert!(
            rejected(&events).is_empty(),
            "AI command rejected: {events:?}"
        );
        let world = app.world_mut();
        let hut_visible = world
            .resource::<Players>()
            .get(P0)
            .expect("player")
            .vision()
            .is_visible(CellPos::new(21, 22));
        if !repaired && !hut_visible {
            lost_sight = true;
        }
        let mut query = world.query::<(&SimId, &Unit, &Owner, &Mobile)>();
        let live: Vec<_> = query
            .iter(world)
            .map(|(id, u, o, m)| (*id, u.kind, o.0, m.cell))
            .collect();
        for (id, kind, owner, cell) in &live {
            if definitions
                .metadata(ProductionItem::Unit(*kind))
                .is_some_and(|m| m.role == sim::Role::Engineer)
            {
                engineers.insert(*id);
                if *owner == P0 && lost_sight && !hut_visible {
                    approached_from_memory |= world.resource::<sim::CommandQueue>().iter().any(|issued| {
                        issued.player == P0 && matches!(&issued.command, Command::Move { units, target }
                            if units.contains(id) && (target.x - 21).abs().max((target.y - 22).abs()) == 1)
                    });
                }
            }
            assert_ne!(
                definitions.domain(*kind),
                Domain::Air,
                "this ground-army regression has no air bypass"
            );
            if repaired && ((*owner == P0 && cell.x > 24) || (*owner == P1 && cell.x < 22)) {
                crossed = true;
            }
        }
        if events
            .iter()
            .any(|event| matches!(event, SimEvent::BridgeChanged { intact: true, .. }))
        {
            repaired = true;
            consumed = engineers
                .iter()
                .any(|id| !live.iter().any(|(live, _, _, _)| live == id));
        }
        if let Some(outcome) = world.resource::<sim::MatchState>().outcome() {
            assert!(
                matches!(outcome, sim::MatchOutcome::Won { .. }),
                "{outcome:?}"
            );
            if lose_sight {
                assert!(
                    lost_sight && approached_from_memory,
                    "lost_sight={lost_sight}, approached_from_memory={approached_from_memory}"
                );
            }
            assert!(
                repaired && consumed && crossed,
                "repaired={repaired}, consumed={consumed}, crossed={crossed}, engineers={engineers:?}"
            );
            return;
        }
    }
    panic!(
        "no winner inside unchanged {limit}-tick limit; repaired={repaired}, consumed={consumed}, crossed={crossed}, engineers={engineers:?}"
    );
}

#[test]
fn ra5_garrison_red_health_evicts_and_refuses_reentry_then_remains_destructible() {
    let mut app = world();
    let house = house(&mut app, 10, 10);
    let gi = put(&mut app, P0, "gi", 9, 10);
    let tank = put(&mut app, P1, "grizzly", 14, 10);
    tick(&mut app, vec![]);
    tick(
        &mut app,
        vec![
            issue(
                P0,
                Command::Interact {
                    unit: gi,
                    target: house,
                },
            ),
            issue(
                P1,
                Command::Attack {
                    units: vec![tank],
                    target: house,
                },
            ),
        ],
    );
    let mut evicted = false;
    for _ in 0..1200 {
        let events = tick(&mut app, vec![]);
        assert!(rejected(&events).is_empty());
        if entity(&mut app, gi).is_some() {
            let remaining = app
                .world()
                .resource::<sim::Props>()
                .get(house)
                .expect("eviction before destruction")
                .1
                .hit_points;
            assert!((1..=75).contains(&remaining), "red threshold: {remaining}");
            let events = tick(
                &mut app,
                vec![issue(
                    P0,
                    Command::Interact {
                        unit: gi,
                        target: house,
                    },
                )],
            );
            assert_eq!(rejected(&events).len(), 1);
            evicted = true;
            break;
        }
    }
    assert!(evicted);
    for _ in 0..1200 {
        tick(&mut app, vec![]);
        if app.world().resource::<sim::Props>().get(house).is_none() {
            return;
        }
    }
    panic!("an emptied house must still be destructible");
}

#[test]
fn ra5_outpost_repairs_one_vehicle_without_becoming_an_aircraft_pad() {
    let mut app = world();
    let outpost = put(&mut app, P0, "tech_outpost", 10, 10);
    let first = put(&mut app, P0, "grizzly", 9, 10);
    let second = put(&mut app, P0, "grizzly", 9, 11);
    let plane = put(&mut app, P0, "harrier", 10, 9);
    let entity = entity(&mut app, first).expect("vehicle");
    app.world_mut()
        .get_mut::<sim::Health>(entity)
        .expect("hp")
        .hit_points = 10;
    tick(&mut app, vec![]);
    let before = credits(&app, P0);
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: first,
                target: outpost,
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    let events = tick(
        &mut app,
        vec![
            issue(
                P0,
                Command::Interact {
                    unit: second,
                    target: outpost,
                },
            ),
            issue(
                P0,
                Command::Interact {
                    unit: plane,
                    target: outpost,
                },
            ),
        ],
    );
    assert_eq!(rejected(&events).len(), 2);
    for _ in 0..100 {
        tick(&mut app, vec![]);
    }
    assert!(hp(&mut app, first) > 10);
    assert!(credits(&app, P0) < before, "vehicle repair is paid");
}

#[test]
fn ra5_enemy_capture_does_not_repeat_the_derricks_neutral_startup_grant() {
    let mut app = world();
    let derrick = put(&mut app, sim::NEUTRAL, "oil_derrick", 10, 10);
    let a = put(&mut app, P0, "engineer", 9, 10);
    let b = put(&mut app, P1, "engineer", 11, 10);
    tick(&mut app, vec![]);
    tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: a,
                target: derrick,
            },
        )],
    );
    let before = credits(&app, P1);
    let events = tick(
        &mut app,
        vec![issue(
            P1,
            Command::Interact {
                unit: b,
                target: derrick,
            },
        )],
    );
    assert!(rejected(&events).is_empty());
    assert_eq!(owner(&mut app, derrick), Some(P1));
    assert_eq!(credits(&app, P1), before);
    for _ in 0..133 {
        tick(&mut app, vec![]);
    }
    assert_eq!(credits(&app, P1), before + 20);
}

#[test]
fn ra5_neutral_and_captured_insignificant_buildings_never_keep_a_defeated_player_alive() {
    for short_game in [true, false] {
        let definitions = shipped();
        let mut skirmish =
            sim::load_skirmish(data_dir().join("skirmish.ron"), &definitions).expect("skirmish");
        skirmish.maps[0].starts = vec![CellPos::new(3, 3), CellPos::new(35, 35)];
        let ai = sim::load_ai(data_dir().join("ai.ron"), &definitions).expect("AI");
        let players = [P0, P1].map(|player| sim::PlayerSetup {
            player,
            faction: 0,
            start: usize::from(player.0),
            controller: sim::Controller::Human,
        });
        let mut app = App::new();
        app.add_plugins(SimPlugin { seed: 11 });
        app.world_mut()
            .insert_resource(Grid::from_cells(48, 48, vec![OPEN; 48 * 48]).expect("grid"));
        app.world_mut().insert_resource(definitions);
        sim::setup_match_configured(
            app.world_mut(),
            &skirmish,
            &ai,
            0,
            &players,
            &sim::MatchOptions {
                mcv_start: false,
                short_game,
                unit_count: Some(0),
                ..Default::default()
            },
        )
        .expect("match");
        let derrick = put(&mut app, sim::NEUTRAL, "oil_derrick", 20, 20);
        let engineer = put(&mut app, P1, "engineer", 19, 20);
        put(&mut app, sim::NEUTRAL, "hospital", 25, 25);
        tick(&mut app, vec![]);
        tick(
            &mut app,
            vec![issue(
                P1,
                Command::Interact {
                    unit: engineer,
                    target: derrick,
                },
            )],
        );
        assert_eq!(owner(&mut app, derrick), Some(P1));
        let world = app.world_mut();
        let buildings: Vec<_> = world
            .query::<(&SimId, &Owner, &Building)>()
            .iter(world)
            .filter(|(id, owner, _)| owner.0 == P1 && **id != derrick)
            .map(|(id, _, _)| *id)
            .collect();
        let events = tick(
            &mut app,
            buildings
                .into_iter()
                .map(|building| issue(P1, Command::SellBuilding { building }))
                .collect(),
        );
        assert!(rejected(&events).is_empty(), "{events:?}");
        // A sold yard legitimately releases surviving crew. In a long game those units must
        // be eliminated too; the captured derrick alone then cannot prevent defeat.
        if !short_game {
            for (x, y) in [(34, 34), (35, 34), (36, 34)] {
                put(&mut app, P0, "tanya", x, y);
            }
            for _ in 0..3000 {
                tick(&mut app, vec![]);
                if app
                    .world()
                    .resource::<sim::MatchState>()
                    .outcome()
                    .is_some()
                {
                    break;
                }
            }
        }
        let state = app.world().resource::<sim::MatchState>();
        assert!(
            matches!(
                state.outcome(),
                Some(sim::MatchOutcome::Won { winner: P0, .. })
            ),
            "short={short_game}: {:?}",
            state.outcome()
        );
        assert_eq!(state.defeated(sim::NEUTRAL), None);
        assert!(!state.players().contains(&sim::NEUTRAL));
    }
}

#[test]
fn ra5_scaled_adjacent_tech_anchors_keep_identity_without_duplicates_or_ghost_obstacles() {
    let mut grid = Grid::from_cells(48, 48, vec![OPEN; 48 * 48]).expect("grid");
    let mut looks = vec![0; 48 * 48];
    for x in [23, 24] {
        let cell = CellPos::new(x, 24);
        grid.set_terrain(
            cell,
            Cell {
                passable: false,
                buildable: false,
                ..OPEN
            },
        );
        looks[24 * 48 + x as usize] = 5 | (11 << 4);
    }
    grid.mark_world_features(&looks);
    let enlarged = sim::enlarged_map(
        &grid,
        &looks,
        &[CellPos::new(3, 3), CellPos::new(40, 40)],
        10,
    )
    .expect("grown");
    let anchors: Vec<_> = enlarged
        .grid
        .world_features()
        .filter(|(_, f)| *f == 11)
        .collect();
    assert_eq!(anchors.len(), 2, "two adjacent sources remain two objects");
    assert_eq!(
        enlarged
            .looks
            .iter()
            .filter(|look| **look >> 4 == 11)
            .count(),
        2
    );
    let mut app = world();
    app.world_mut().insert_resource(enlarged.grid);
    sim::install_world_objects(app.world_mut(), true);
    for (cell, _) in anchors {
        assert!(app.world().resource::<Grid>().building(cell).is_some());
    }
    let grid = app.world().resource::<Grid>();
    for y in 45..51 {
        for x in 44..52 {
            let cell = CellPos::new(x, y);
            if grid.building(cell).is_none() {
                assert!(
                    grid.cell(cell).expect("cell").passable,
                    "no ghost terrain {cell:?}"
                );
            }
        }
    }
}

#[test]
fn ra5_all_large_maps_have_symmetric_tech_and_real_objects_after_two_to_ten_player_scaling() {
    let definitions = shipped();
    let skirmish =
        sim::load_skirmish(data_dir().join("skirmish.ron"), &definitions).expect("skirmish");
    let assets = data_dir().parent().expect("assets").to_owned();
    let mut checked = 0;
    for spec in skirmish
        .maps
        .iter()
        .filter(|m| !["demo", "crossroads", "riverlands"].contains(&m.id.as_str()))
    {
        let loaded = sim::load_map(assets.join(sim::map_asset_path(&spec.id))).expect("map");
        for feature in 11..=14 {
            let cells: Vec<_> = loaded
                .grid
                .world_features()
                .filter(|(_, f)| *f == feature)
                .map(|(c, _)| c)
                .collect();
            assert_eq!(cells.len(), 2, "{} feature {feature}", spec.id);
            assert_eq!(cells[0].x + cells[1].x, loaded.grid.width() as i32 - 1);
            assert_eq!(cells[0].y + cells[1].y, loaded.grid.height() as i32 - 1);
        }
        for players in 2..=10 {
            let enlarged = sim::enlarged_map(&loaded.grid, &loaded.looks, &spec.starts, players)
                .expect("grown");
            let anchors: Vec<_> = enlarged
                .grid
                .world_features()
                .filter(|(_, f)| *f >= 11)
                .collect();
            let mut app = App::new();
            app.add_plugins(SimPlugin { seed: 11 });
            app.world_mut().insert_resource(definitions.clone());
            app.world_mut().insert_resource(enlarged.grid);
            sim::install_world_objects(app.world_mut(), true);
            for (cell, feature) in anchors {
                let id = app
                    .world()
                    .resource::<Grid>()
                    .building(cell)
                    .unwrap_or_else(|| {
                        panic!("{} {players}: feature {feature} at {cell:?}", spec.id)
                    });
                assert_eq!(owner(&mut app, id), Some(sim::NEUTRAL));
            }
            for bridge in app.world().resource::<sim::Bridges>().bridges.values() {
                assert!(
                    bridge.huts.len() >= 2,
                    "{} {players}: bridge needs both repair ends: {bridge:?}",
                    spec.id
                );
            }
        }
        checked += 1;
    }
    assert_eq!(checked, 18);
}

#[test]
fn ra5_city_and_tower_take_ten_conscripts_and_use_the_soviet_occupation_weapon() {
    for feature in [6, 7] {
        let mut app = world();
        let cell = CellPos::new(10, 10);
        app.world_mut().resource_mut::<Grid>().set_terrain(
            cell,
            Cell {
                passable: false,
                buildable: false,
                ..OPEN
            },
        );
        let mut looks = vec![0; 48 * 48];
        looks[10 * 48 + 10] = feature << 4;
        app.world_mut()
            .resource_mut::<Grid>()
            .mark_world_features(&looks);
        sim::install_world_objects(app.world_mut(), true);
        let host = app
            .world()
            .resource::<sim::Props>()
            .id_at(cell)
            .expect("civilian");
        for _ in 0..10 {
            let unit = put(&mut app, P1, "conscript", 9, 10);
            tick(&mut app, vec![]);
            let events = tick(
                &mut app,
                vec![issue(P1, Command::Interact { unit, target: host })],
            );
            assert!(rejected(&events).is_empty());
        }
        let extra = put(&mut app, P1, "conscript", 9, 10);
        let events = tick(
            &mut app,
            vec![issue(
                P1,
                Command::Interact {
                    unit: extra,
                    target: host,
                },
            )],
        );
        assert_eq!(rejected(&events).len(), 1);
        let target = put(&mut app, P0, "hospital", 14, 10);
        let mut fired = Vec::new();
        for t in 0..12 {
            for event in tick(&mut app, vec![]) {
                if let SimEvent::WeaponFired {
                    attacker,
                    target: found,
                    weapon,
                } = event
                    && attacker == host
                {
                    assert_eq!(found, target);
                    let weapon = defs(&app).weapon(weapon).cloned().expect("weapon");
                    assert_eq!((weapon.damage, weapon.reload), (20, 27));
                    fired.push(t);
                }
            }
        }
        assert!(fired.len() >= 5, "{fired:?}");
        assert!(
            fired.windows(2).all(|w| w[1] - w[0] == 2),
            "27 / 10 ticks: {fired:?}"
        );
    }
}

#[test]
fn ra5_garrison_owner_location_reload_and_eviction_each_change_the_state_hash() {
    let mut app = world();
    let host = house(&mut app, 10, 10);
    let gi = put(&mut app, P0, "gi", 9, 10);
    tick(&mut app, vec![]);
    tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: gi,
                target: host,
            },
        )],
    );
    let original = app.world().resource::<sim::SpecialState>().garrisons[&host].clone();
    let base = sim::state_hash(app.world_mut());
    for change in 0..4 {
        let mut changed = original.clone();
        match change {
            0 => changed.owner = P1,
            1 => changed.cell.x += 1,
            2 => changed.cooldown += 1,
            _ => changed.evicting = true,
        }
        app.world_mut()
            .resource_mut::<sim::SpecialState>()
            .garrisons
            .insert(host, changed);
        assert_ne!(sim::state_hash(app.world_mut()), base, "field {change}");
    }
    app.world_mut()
        .resource_mut::<sim::SpecialState>()
        .garrisons
        .insert(host, original);
    assert_eq!(sim::state_hash(app.world_mut()), base);
    let crew = app.world().resource::<sim::SpecialState>().cargo[&host].clone();
    app.world_mut()
        .resource_mut::<sim::SpecialState>()
        .cargo
        .get_mut(&host)
        .expect("crew")[0]
        .hp -= 1;
    assert_ne!(sim::state_hash(app.world_mut()), base);
    app.world_mut()
        .resource_mut::<sim::SpecialState>()
        .cargo
        .insert(host, crew);
    assert_eq!(sim::state_hash(app.world_mut()), base);
}

#[test]
fn ra5_seaward_pier_hut_adds_no_land_route_and_shore_engineer_repairs_after_collapse() {
    let loaded = sim::load_map(
        data_dir()
            .parent()
            .expect("assets")
            .join(sim::map_asset_path("liberty_bay")),
    )
    .expect("map");
    let mut app = world();
    app.world_mut().insert_resource(loaded.grid);
    sim::install_world_objects(app.world_mut(), true);
    let (bridge, run) = app
        .world()
        .resource::<sim::Bridges>()
        .bridges
        .iter()
        .find(|(_, b)| b.cells.contains(&CellPos::new(183, 28)))
        .map(|(id, b)| (*id, b.clone()))
        .expect("original one-bank pier");
    let mut shore = None;
    let mut seaward = 0;
    for hut in &run.huts {
        let e = entity(&mut app, *hut).expect("hut");
        let cell = app
            .world()
            .get::<Building>(e)
            .expect("building")
            .footprint
            .origin;
        let terrain = app.world().resource::<Grid>().cell(cell).expect("cell");
        if terrain.water {
            assert!(!terrain.passable, "a raised seaward hut must not make land");
            seaward += 1;
        } else {
            shore = Some((*hut, cell));
        }
    }
    assert!(seaward > 0);
    let (hut, cell) = shore.expect("shore hut");
    let standing = (cell.y - 1..=cell.y + 1)
        .flat_map(|y| (cell.x - 1..=cell.x + 1).map(move |x| CellPos::new(x, y)))
        .find(|c| app.world().resource::<Grid>().is_passable(*c) && !run.cells.contains(c))
        .expect("shore approach");
    let engineer = put(&mut app, P0, "engineer", standing.x, standing.y);
    let target_cell = CellPos::new(184, 38);
    let target = app
        .world()
        .resource::<sim::Props>()
        .id_at(target_cell)
        .expect("shore deck");
    let mut tanks = Vec::new();
    for _ in 0..2 {
        let grid = app.world().resource::<Grid>();
        let cell = (37..=42)
            .flat_map(|y| (180..=188).map(move |x| CellPos::new(x, y)))
            .find(|c| grid.has_room(*c, false) && !run.cells.contains(c))
            .expect("tank bank");
        tanks.push(put(&mut app, P0, "grizzly", cell.x, cell.y));
    }
    tick(&mut app, vec![]);
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Attack {
                units: tanks.clone(),
                target,
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    for _ in 0..3000 {
        tick(&mut app, vec![]);
        if app.world().resource::<sim::Bridges>().bridges[&bridge].collapsed {
            break;
        }
    }
    assert!(app.world().resource::<sim::Bridges>().bridges[&bridge].collapsed);
    let events = tick(
        &mut app,
        vec![
            issue(P0, Command::Stop { units: tanks }),
            issue(
                P0,
                Command::Interact {
                    unit: engineer,
                    target: hut,
                },
            ),
        ],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    for _ in 0..100 {
        if entity(&mut app, engineer).is_none() {
            break;
        }
        tick(&mut app, vec![]);
    }
    assert!(
        entity(&mut app, engineer).is_none(),
        "shore hut remains reachable after collapse"
    );
    assert!(!app.world().resource::<sim::Bridges>().bridges[&bridge].collapsed);
}

#[test]
fn ra5_one_area_blast_hits_a_connected_bridge_once_at_its_strongest_deck_cell() {
    let (mut app, bridge, _) = bridge_world(true);
    for x in [1, 4, 7, 10] {
        put(&mut app, P0, "tesla_reactor", x, 1);
    }
    let silo = put(&mut app, P0, "nuclear_silo", 1, 5);
    put(&mut app, P0, "engineer", 20, 24);
    tick(&mut app, vec![]);
    let kind = defs(&app).building_kind("nuclear_silo").expect("silo");
    let charge = defs(&app)
        .metadata(ProductionItem::Building(kind))
        .expect("metadata")
        .cooldown;
    app.world_mut()
        .resource_mut::<sim::SpecialState>()
        .charge
        .insert(silo, charge);
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Superweapon {
                building: silo,
                target: CellPos::new(23, 24),
                units: vec![],
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    // RA6 retains this exact bridge damage assertion after the 200-tick missile flight.
    for _ in 0..200 {
        tick(&mut app, vec![]);
    }
    // Shipped NukePayload 600, NUKE versus concrete 8%: 48 once, not once per deck.
    for cell in &app.world().resource::<sim::Bridges>().bridges[&bridge].cells {
        let props = app.world().resource::<sim::Props>();
        let id = props.id_at(*cell).expect("deck");
        assert_eq!(props.get(id).expect("standing").1.hit_points, 1452);
    }
}

#[test]
fn ra5_bridge_collapse_and_hospital_cargo_change_hash_and_replay_deterministically() {
    fn replay() -> Vec<u64> {
        let (mut app, bridge, _) = bridge_world(true);
        let derrick = put(&mut app, sim::NEUTRAL, "oil_derrick", 10, 10);
        let engineer = put(&mut app, P0, "engineer", 9, 10);
        let hospital = put(&mut app, P0, "hospital", 15, 10);
        let patient = put(&mut app, P0, "gi", 14, 10);
        let e = entity(&mut app, patient).expect("patient");
        app.world_mut()
            .entity_mut(e)
            .insert(sim::Health { hit_points: 5 });
        tick(&mut app, vec![]);
        tick(
            &mut app,
            vec![
                issue(
                    P0,
                    Command::Interact {
                        unit: engineer,
                        target: derrick,
                    },
                ),
                issue(
                    P0,
                    Command::Interact {
                        unit: patient,
                        target: hospital,
                    },
                ),
            ],
        );
        assert_eq!(owner(&mut app, derrick), Some(P0));
        let base = sim::state_hash(app.world_mut());
        app.world_mut()
            .resource_mut::<sim::SpecialState>()
            .cargo
            .get_mut(&hospital)
            .expect("patients")[0]
            .hp += 1;
        assert_ne!(sim::state_hash(app.world_mut()), base);
        app.world_mut()
            .resource_mut::<sim::SpecialState>()
            .cargo
            .get_mut(&hospital)
            .expect("patients")[0]
            .hp -= 1;
        app.world_mut()
            .resource_mut::<sim::Bridges>()
            .bridges
            .get_mut(&bridge)
            .expect("bridge")
            .collapsed = true;
        assert_ne!(sim::state_hash(app.world_mut()), base);
        app.world_mut()
            .resource_mut::<sim::Bridges>()
            .bridges
            .get_mut(&bridge)
            .expect("bridge")
            .collapsed = false;
        assert_eq!(sim::state_hash(app.world_mut()), base);
        let mut hashes = vec![base];
        for _ in 0..140 {
            tick(&mut app, vec![]);
            hashes.push(sim::state_hash(app.world_mut()));
        }
        assert_eq!(hp(&mut app, patient), 125);
        assert_eq!(credits(&app, P0), 11_020);
        hashes
    }
    let first = replay();
    assert_eq!(first, replay());
    let mut digest = sim::Fnv1a64::new();
    for hash in &first {
        digest.write_u64(*hash);
    }
    println!(
        "RA5 replay: {} checkpoints, digest {:016x}",
        first.len(),
        digest.finish()
    );
}

#[test]
fn ra5_enemy_garrisons_exchange_occupation_fire_but_hidden_garrisons_are_not_targets() {
    let mut app = world();
    let mut looks = vec![0; 48 * 48];
    for x in [10, 14] {
        let cell = CellPos::new(x, 10);
        app.world_mut().resource_mut::<Grid>().set_terrain(
            cell,
            Cell {
                passable: false,
                buildable: false,
                ..OPEN
            },
        );
        looks[10 * 48 + x as usize] = 5 << 4;
    }
    app.world_mut()
        .resource_mut::<Grid>()
        .mark_world_features(&looks);
    sim::install_world_objects(app.world_mut(), true);
    let houses: Vec<_> = [10, 14]
        .into_iter()
        .map(|x| {
            app.world()
                .resource::<sim::Props>()
                .id_at(CellPos::new(x, 10))
                .expect("house")
        })
        .collect();
    let a = put(&mut app, P0, "gi", 9, 10);
    let b = put(&mut app, P1, "conscript", 15, 10);
    tick(&mut app, vec![]);
    tick(
        &mut app,
        vec![
            issue(
                P0,
                Command::Interact {
                    unit: a,
                    target: houses[0],
                },
            ),
            issue(
                P1,
                Command::Interact {
                    unit: b,
                    target: houses[1],
                },
            ),
        ],
    );
    assert!(entity(&mut app, a).is_none());
    assert!(entity(&mut app, b).is_none());
    let mut fired = [false; 2];
    for _ in 0..30 {
        for event in tick(&mut app, vec![]) {
            if let SimEvent::WeaponFired {
                attacker, target, ..
            } = event
            {
                for i in 0..2 {
                    if attacker == houses[i] && target == houses[1 - i] {
                        fired[i] = true;
                    }
                }
            }
        }
    }
    assert_eq!(
        fired,
        [true, true],
        "both owners must target the enemy occupied prop"
    );
    for id in &houses {
        assert!(
            app.world()
                .resource::<sim::Props>()
                .get(*id)
                .expect("house")
                .1
                .hit_points
                < 300
        );
    }
    for _ in 0..25 {
        // Hidden is refreshed by visibility at tick end; seed the current tick's observation.
        app.world_mut()
            .resource_mut::<sim::SpecialState>()
            .hidden
            .insert((P0, houses[1]));
        assert!(!tick(&mut app,vec![]).iter().any(|event|matches!(event,SimEvent::WeaponFired{attacker,target,..} if *attacker==houses[0] && *target==houses[1])));
    }
}

#[test]
fn ra5_bridge_repair_waits_for_standing_and_entering_ships_without_harming_compatible_units() {
    for mode in 0..3 {
        let entering = mode != 0;
        let (mut app, bridge, hut) = bridge_world(true);
        let tank = put(&mut app, P0, "grizzly", 20, 24);
        tick(&mut app, vec![]);
        tick(
            &mut app,
            vec![issue(
                P0,
                Command::Attack {
                    units: vec![tank],
                    target: bridge,
                },
            )],
        );
        for _ in 0..4000 {
            if app.world().resource::<sim::Bridges>().bridges[&bridge].collapsed {
                break;
            }
            tick(&mut app, vec![]);
        }
        assert!(app.world().resource::<sim::Bridges>().bridges[&bridge].collapsed);
        tick(
            &mut app,
            vec![issue(P0, Command::Stop { units: vec![tank] })],
        );
        let ship = put(
            &mut app,
            P0,
            "destroyer",
            23,
            if entering { 23 } else { 24 },
        );
        let engineer = put(
            &mut app,
            P0,
            "engineer",
            if mode == 2 { 16 } else { 20 },
            22,
        );
        if mode == 2 {
            let events = tick(
                &mut app,
                vec![issue(
                    P0,
                    Command::Interact {
                        unit: engineer,
                        target: hut,
                    },
                )],
            );
            assert!(
                rejected(&events).is_empty(),
                "initially clear repair route: {events:?}"
            );
            assert!(
                app.world()
                    .resource::<sim::SpecialState>()
                    .interactions
                    .contains_key(&engineer)
            );
        }
        if entering {
            tick(
                &mut app,
                vec![issue(
                    P0,
                    Command::Move {
                        units: vec![ship],
                        target: CellPos::new(23, 24),
                    },
                )],
            );
            let e = entity(&mut app, ship).expect("ship");
            let mobile = app.world().get::<Mobile>(e).expect("mobile");
            assert_eq!(mobile.cell, CellPos::new(23, 23));
            assert_eq!(mobile.step.expect("reserved step").to, CellPos::new(23, 24));
        } else {
            tick(&mut app, vec![]);
        }
        if mode == 2 {
            for _ in 0..200 {
                tick(&mut app, vec![]);
            }
            assert!(
                entity(&mut app, engineer).is_some(),
                "arrival cannot consume the Engineer after a ship enters"
            );
            assert!(
                !app.world()
                    .resource::<sim::SpecialState>()
                    .interactions
                    .contains_key(&engineer)
            );
        }
        let ship_hp = hp(&mut app, ship);
        let events = tick(
            &mut app,
            vec![issue(
                P0,
                Command::Interact {
                    unit: engineer,
                    target: hut,
                },
            )],
        );
        assert_eq!(rejected(&events).len(), 1, "{events:?}");
        assert!(entity(&mut app, engineer).is_some());
        assert_eq!(hp(&mut app, ship), ship_hp);
        assert!(app.world().resource::<sim::Bridges>().bridges[&bridge].collapsed);
        assert_eq!(
            app.world()
                .resource::<Grid>()
                .occupant(CellPos::new(23, 24)),
            Some(ship)
        );
        tick(
            &mut app,
            vec![issue(
                P0,
                Command::Move {
                    units: vec![ship],
                    target: CellPos::new(23, 20),
                },
            )],
        );
        for _ in 0..300 {
            tick(&mut app, vec![]);
        }
        assert_ne!(
            app.world()
                .resource::<Grid>()
                .occupant(CellPos::new(23, 24)),
            Some(ship)
        );
        let amph = put(&mut app, P0, "amphibious_transport", 22, 24);
        let air = put(&mut app, P0, "rocketeer", 22, 24);
        tick(
            &mut app,
            vec![issue(
                P0,
                Command::Interact {
                    unit: engineer,
                    target: hut,
                },
            )],
        );
        assert!(entity(&mut app, engineer).is_none());
        assert!(!app.world().resource::<sim::Bridges>().bridges[&bridge].collapsed);
        for id in [ship, amph, air] {
            assert!(hp(&mut app, id) > 0);
        }
        let grid = app.world().resource::<Grid>();
        assert_eq!(grid.occupant(CellPos::new(22, 24)), Some(amph));
        assert_eq!(
            grid.occupant_for(CellPos::new(22, 24), Domain::Air),
            Some(air)
        );
    }
}
