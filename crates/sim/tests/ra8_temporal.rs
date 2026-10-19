//! Native Temporal range retention on shipped data, through ordinary commands and SimSchedule.
use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    Cell, CellPos, Command, CommandQueue, Definitions, Grid, IssuedCommand, PlayerId, Players,
    SimEvent, SimId, SimPlugin, SimSchedule, load_ruleset, spawn_building, spawn_unit,
};
use std::path::PathBuf;
const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);
const OPEN: Cell = Cell {
    passable: true,
    water: false,
    buildable: true,
    ore: 0,
};
fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data")
}

fn shipped() -> Definitions {
    load_ruleset(data_dir()).unwrap_or_else(|error| panic!("{error}"))
}

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

fn hp(app: &mut App, id: SimId) -> i32 {
    let entity = entity(app, id).expect("object exists");
    app.world()
        .get::<sim::Health>(entity)
        .expect("health")
        .hit_points
}

fn cell(app: &mut App, id: SimId) -> CellPos {
    let e = entity(app, id).expect("unit");
    app.world().get::<sim::Mobile>(e).expect("mobile").cell
}

fn set_hp(app: &mut App, id: SimId, hp: i32) {
    let e = entity(app, id).expect("actor");
    app.world_mut()
        .get_mut::<sim::Health>(e)
        .expect("health")
        .hit_points = hp;
}

fn temporal_edge_fixture(
    source_cell: CellPos,
    target_kind: &str,
    target_cell: CellPos,
    elite: bool,
) -> (App, SimId, SimId) {
    let mut app = world();
    let source = put(
        &mut app,
        P0,
        "chrono_legionnaire",
        source_cell.x,
        source_cell.y,
    );
    if elite {
        assert!(sim::promote(app.world_mut(), source, sim::ELITE));
    }
    let target = put(&mut app, P1, target_kind, target_cell.x, target_cell.y);
    let e = entity(&mut app, source).unwrap();
    assert_eq!(
        app.world().get::<sim::Position>(e).unwrap().0,
        app.world()
            .resource::<Grid>()
            .standing_point(source_cell, source),
        "shipped Chrono must use its native infantry spot"
    );
    (app, source, target)
}

fn temporal_edge_begin(app: &mut App, source: SimId, target: SimId) -> (u64, u64) {
    let mut fired = false;
    for _ in 0..8 {
        let events = tick(app, vec![]);
        fired |= events.iter().any(|event| {
            matches!(event,
            SimEvent::WeaponFired { attacker, target: found, .. }
                if *attacker == source && *found == target)
        });
        if let Some(beam) = app
            .world()
            .resource::<sim::SpecialState>()
            .erasures
            .get(&target)
        {
            assert!(fired, "a retained beam requires an actual scheduled shot");
            assert_eq!(beam.beams.len(), 1);
            let damage = u64::from(*beam.beams.get(&source).expect("the actual source beam"));
            return (beam.remaining, damage * 3 / 4);
        }
    }
    panic!("native shot must retain its beam after Combat resolve: fired={fired}");
}

#[test]
fn ra8_temporal_footprint_edge_retains_during_reload_and_completes_erasure() {
    for elite in [false, true] {
        let (mut app, source, target) = temporal_edge_fixture(
            CellPos::new(29, 14),
            "power_plant",
            CellPos::new(34, 12),
            elite,
        );
        let initial = hp(&mut app, target);
        assert_eq!(initial, 750, "the untouched shipped target strength");
        let e = entity(&mut app, source).unwrap();
        let point = app.world().get::<sim::Position>(e).unwrap().0;
        let e = entity(&mut app, target).unwrap();
        let foot = app.world().get::<sim::Building>(e).unwrap().footprint;
        let low = sim::SubPos::new(foot.origin.x * 256, foot.origin.y * 256);
        let high = sim::SubPos::new(
            (foot.origin.x + i32::from(foot.width)) * 256 - 1,
            (foot.origin.y + i32::from(foot.height)) * 256 - 1,
        );
        let gap = (
            (i64::from(low.x) - i64::from(point.x))
                .max(i64::from(point.x) - i64::from(high.x))
                .max(0),
            (i64::from(low.y) - i64::from(point.y))
                .max(i64::from(point.y) - i64::from(high.y))
                .max(0),
        );
        let centre = sim::SubPos::centre_of(CellPos::new(34, 13));
        let old_gap = (
            i64::from(point.x) - i64::from(centre.x),
            i64::from(point.y) - i64::from(centre.y),
        );
        eprintln!(
            "native edge elite{elite} point{point:?} footprint{foot:?} gap{gap:?} old_gap{old_gap:?}"
        );
        assert!(sim::in_range(gap.0, gap.1, 1280));
        assert!(!sim::in_range(old_gap.0, old_gap.1, 1280));
        let (first, rate) = temporal_edge_begin(&mut app, source, target);
        assert_eq!(rate, if elite { 12 } else { 6 });
        let e = entity(&mut app, source).unwrap();
        assert!(app.world().get::<sim::Weapon>(e).unwrap().cooldown > 16);
        for _ in 0..16 {
            let events = tick(&mut app, vec![]);
            assert!(
                !events.iter().any(|event| matches!(event,
                SimEvent::WeaponFired { attacker, .. } if *attacker == source)),
                "progress must come from the existing beam while reloading"
            );
        }
        assert_eq!(
            app.world().resource::<sim::SpecialState>().erasures[&target].remaining,
            first - 16 * rate,
            "native channel must advance on every tick"
        );
        assert_eq!(
            hp(&mut app, target),
            initial,
            "Temporal does not chip ordinary health"
        );
        assert_eq!(cell(&mut app, source), CellPos::new(29, 14));
        let mut previous = first - 16 * rate;
        for _ in 0..1300 {
            tick(&mut app, vec![]);
            if entity(&mut app, target).is_none() {
                break;
            }
            let remaining = app.world().resource::<sim::SpecialState>().erasures[&target].remaining;
            assert_eq!(previous - remaining, rate);
            previous = remaining;
            assert_eq!(hp(&mut app, target), initial);
        }
        assert!(
            entity(&mut app, target).is_none(),
            "actual erasure must finish within shipped duration"
        );
        assert!(entity(&mut app, source).is_some());
    }
}

#[test]
fn ra8_temporal_native_inside_outside_and_unit_exact_range_controls() {
    // The first is the retained RA8 admission-control placement; the second is
    // genuinely outside Combat's building-footprint range and has no chase order.
    for (source_cell, expected) in [(CellPos::new(30, 15), true), (CellPos::new(28, 14), false)] {
        let (mut app, source, target) =
            temporal_edge_fixture(source_cell, "power_plant", CellPos::new(34, 12), false);
        if expected {
            let (remaining, rate) = temporal_edge_begin(&mut app, source, target);
            tick(&mut app, vec![]);
            assert_eq!(
                app.world().resource::<sim::SpecialState>().erasures[&target].remaining,
                remaining - rate
            );
        } else {
            for _ in 0..24 {
                let events = tick(&mut app, vec![]);
                assert!(!events.iter().any(|event| matches!(event,
                    SimEvent::WeaponFired { attacker, target: found, .. }
                        if *attacker == source && *found == target)));
                assert!(
                    !app.world()
                        .resource::<sim::SpecialState>()
                        .erasures
                        .contains_key(&target)
                );
            }
            assert_eq!(
                cell(&mut app, source),
                source_cell,
                "range control must not chase"
            );
            assert_eq!(hp(&mut app, target), 750);
        }
    }
    // Engineer has a native point extent and no weapon: 4, exactly 5, and 6 cells.
    // Both infantry stand at native spot 0, so their offsets cancel exactly.
    // Exact maximum remains inclusive; unit extents must not acquire a footprint.
    for (x, expected) in [(33, true), (34, true), (35, false)] {
        let (mut app, source, target) =
            temporal_edge_fixture(CellPos::new(29, 14), "engineer", CellPos::new(x, 14), false);
        let initial = hp(&mut app, target);
        let e = entity(&mut app, target).unwrap();
        assert_eq!(
            app.world().get::<sim::Position>(e).unwrap().0,
            sim::SubPos::spot_of(CellPos::new(x, 14), 0)
        );
        if expected {
            let (remaining, rate) = temporal_edge_begin(&mut app, source, target);
            tick(&mut app, vec![]);
            assert_eq!(
                app.world().resource::<sim::SpecialState>().erasures[&target].remaining,
                remaining - rate
            );
        } else {
            for _ in 0..24 {
                let events = tick(&mut app, vec![]);
                assert!(!events.iter().any(|event| matches!(event,
                    SimEvent::WeaponFired { attacker, .. } if *attacker == source)));
                assert!(
                    !app.world()
                        .resource::<sim::SpecialState>()
                        .erasures
                        .contains_key(&target)
                );
            }
        }
        assert_eq!(hp(&mut app, target), initial);
    }
}

#[test]
fn ra8_temporal_edge_releases_on_move_retarget_or_death() {
    for why in ["stop", "move", "retarget", "source-death", "target-death"] {
        let (mut app, source, target) = temporal_edge_fixture(
            CellPos::new(29, 14),
            "power_plant",
            CellPos::new(34, 12),
            false,
        );
        temporal_edge_begin(&mut app, source, target);
        let mut replacement = None;
        let commands = match why {
            "stop" => vec![issue(
                P0,
                Command::Stop {
                    units: vec![source],
                },
            )],
            "move" => vec![issue(
                P0,
                Command::Move {
                    units: vec![source],
                    target: CellPos::new(20, 20),
                },
            )],
            "retarget" => {
                let next = put(&mut app, P1, "power_plant", 31, 17);
                replacement = Some(next);
                vec![issue(
                    P0,
                    Command::Attack {
                        units: vec![source],
                        target: next,
                    },
                )]
            }
            "source-death" => {
                set_hp(&mut app, source, 0);
                vec![]
            }
            "target-death" => {
                set_hp(&mut app, target, 0);
                vec![]
            }
            _ => unreachable!(),
        };
        let events = tick(&mut app, commands);
        assert!(rejected(&events).is_empty(), "{why}: {events:?}");
        assert!(
            !app.world()
                .resource::<sim::SpecialState>()
                .erasures
                .contains_key(&target),
            "{why}"
        );
        if why == "target-death" {
            assert!(entity(&mut app, target).is_none());
        } else {
            assert_eq!(
                hp(&mut app, target),
                750,
                "{why}: release must leave health untouched"
            );
        }
        if why == "move" {
            assert_eq!(cell(&mut app, source), CellPos::new(20, 20));
            assert!(
                app.world()
                    .resource::<sim::SpecialState>()
                    .disabled
                    .contains_key(&source)
            );
        } else if why == "source-death" {
            assert!(entity(&mut app, source).is_none());
        } else if let Some(next) = replacement {
            let e = entity(&mut app, source).unwrap();
            assert_eq!(
                app.world().get::<sim::Weapon>(e).unwrap().target,
                Some(next)
            );
        }
    }
}

#[test]
fn ra8_temporal_edge_frozen_source_releases_its_existing_native_beam() {
    let (mut app, source, target) = temporal_edge_fixture(
        CellPos::new(29, 14),
        "power_plant",
        CellPos::new(34, 12),
        false,
    );
    temporal_edge_begin(&mut app, source, target);
    let freezer = put(&mut app, P1, "chrono_legionnaire", 27, 14);
    let mut native_freeze_shot = false;
    for _ in 0..8 {
        let events = tick(&mut app, vec![]);
        native_freeze_shot |= events.iter().any(|event| {
            matches!(event,
            SimEvent::WeaponFired { attacker, target: found, .. }
                if *attacker == freezer && *found == source)
        });
        if app
            .world()
            .resource::<sim::SpecialState>()
            .erasures
            .contains_key(&source)
        {
            break;
        }
    }
    assert!(native_freeze_shot);
    let special = app.world().resource::<sim::SpecialState>();
    assert!(special.erasures[&source].beams.contains_key(&freezer));
    assert!(!special.erasures.contains_key(&target));
    assert_eq!(hp(&mut app, target), 750);
}

#[test]
fn ra8_temporal_fixture_minimum_is_strict_at_the_native_point_boundary() {
    // Shipped neutron rifles have minimum 0. This isolated fixed-definition
    // control changes only the normal rifle's minimum to four cells before any tick
    // runs; it remains strictly below the unchanged shipped five-cell maximum.
    // The one short-lived text file uses TMPDIR; no copied dataset or environment.
    for (x, expected) in [(33, true), (32, false)] {
        let (mut app, source, target) =
            temporal_edge_fixture(CellPos::new(29, 14), "engineer", CellPos::new(x, 14), false);
        let original = std::fs::read_to_string(data_dir().join("combat.ron")).unwrap();
        let modified = original
            .lines()
            .map(|line| {
                if line.contains(r#"id: "neutron_rifle""#) {
                    line.replace("min_range: 0", "min_range: 1024")
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_ne!(original.trim_end(), modified);
        let path = PathBuf::from(std::env::var_os("TMPDIR").expect("TMPDIR")).join(format!(
            "ra8-temporal-min-{}-{:?}-{x}.ron",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::write(&path, modified).unwrap();
        let loaded = defs(&app).with_combat(&path);
        std::fs::remove_file(&path).unwrap();
        let loaded = loaded.unwrap();
        let rifle = loaded.weapon_kind("neutron_rifle").unwrap();
        assert_eq!(loaded.weapon(rifle).unwrap().min_range, 1024);
        app.world_mut().insert_resource(loaded);
        if expected {
            let (remaining, rate) = temporal_edge_begin(&mut app, source, target);
            tick(&mut app, vec![]);
            assert_eq!(
                app.world().resource::<sim::SpecialState>().erasures[&target].remaining,
                remaining - rate,
                "exactly minimum remains eligible during reload"
            );
        } else {
            for _ in 0..24 {
                let events = tick(&mut app, vec![]);
                assert!(!events.iter().any(|event| matches!(event,
                    SimEvent::WeaponFired { attacker, .. } if *attacker == source)));
                assert!(
                    !app.world()
                        .resource::<sim::SpecialState>()
                        .erasures
                        .contains_key(&target)
                );
            }
        }
    }
}

#[test]
fn ra8_temporal_native_concealment_releases_a_beam_after_the_detector_is_sold() {
    let (mut app, source, target) =
        temporal_edge_fixture(CellPos::new(29, 14), "spy", CellPos::new(34, 14), false);
    put(&mut app, P0, "power_plant", 24, 10);
    let detector = put(&mut app, P0, "psychic_sensor", 28, 10);
    temporal_edge_begin(&mut app, source, target);
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .hidden
            .contains(&(P0, target))
    );
    let events = tick(
        &mut app,
        vec![issue(P0, Command::SellBuilding { building: detector })],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .hidden
            .contains(&(P0, target)),
        "ordinary detector sale must actually conceal the native Spy"
    );
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .erasures
            .contains_key(&target)
    );
    assert_eq!(hp(&mut app, target), 100);
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Attack {
                units: vec![source],
                target,
            },
        )],
    );
    assert!(events.iter().any(|event| matches!(event,
        SimEvent::CommandRejected { reason: sim::RejectReason::UnknownTarget(id), .. } if *id == target)), "{events:?}");
    assert!(!events.iter().any(|event| matches!(event,
        SimEvent::WeaponFired { target: id, .. } if *id == target)));
}
