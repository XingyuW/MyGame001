//! FreeMCV is an owned live-root census, including actual embarked descendants.
mod ra9_runtime_common;
use ra9_runtime_common::*;
use sim::{Cell, CellPos, Command, CrateContent, Grid, Owner, SimId, SpecialState, Unit};

fn setup(nested: bool) -> (bevy_app::App, SimId, SimId, SimId, Option<SimId>) {
    let mut app = world();
    let mut d = only_powerup("reveal");
    if nested {
        // Stock MCV6 fits Landing Craft6/12, but stock Landing Craft16 fits no stock
        // transport. Only the outer-transport capacity boundary is widened for this
        // recursive fixture; no shipped claim is made for that nesting configuration.
        let text = std::fs::read_to_string(data_dir().join("arsenal.ron")).unwrap();
        let row = text
            .lines()
            .find(|r| r.contains("(id: \"amphibious_transport\","))
            .unwrap();
        let changed = row
            .replace("capacity: 12,", "capacity: 16,")
            .replace("size_limit: 6,", "size_limit: 16,");
        assert_ne!(row, changed);
        d = d
            .with_arsenal(variant("arsenal.ron", &[(row, changed.as_str())]))
            .unwrap();
    }
    app.world_mut().insert_resource(d);
    let root = put(&mut app, P0, "amphibious_transport", 20, 20);
    let mcv = put(&mut app, P0, "mcv", 19, 20);
    assert!(rejected(&interact(&mut app, mcv, root)).is_empty());
    assert!(entity(&mut app, mcv).is_none());
    assert_eq!(cargo(&app, root), [mcv]);
    let (root, inner) = if nested {
        let outer = put(&mut app, P0, "amphibious_transport", 21, 20);
        assert!(rejected(&interact(&mut app, root, outer)).is_empty());
        assert!(entity(&mut app, root).is_none());
        assert_eq!(cargo(&app, outer), [root]);
        assert_eq!(cargo(&app, root), [mcv]);
        (outer, Some(root))
    } else {
        (root, None)
    };
    let collector = put(&mut app, P0, "engineer", 10, 12);
    let at = CellPos::new(12, 12);
    crate_at(&mut app, at);
    assert!(credits(&app) > sim::FREE_MCV_CREDITS);
    assert!(
        app.world_mut()
            .query::<&sim::Building>()
            .iter(app.world())
            .next()
            .is_none(),
        "no base building premise"
    );
    (app, root, mcv, collector, inner)
}

fn ordinary_with_cargo(nested: bool) {
    let (mut app, root, mcv, collector, inner) = setup(nested);
    let taken = collect_crate(&mut app, collector, CellPos::new(12, 12));
    assert_eq!(
        taken,
        (CrateContent::Reveal, None),
        "a live owned cargo MCV prevents guaranteed FreeMCV"
    );
    let events = tick(&mut app, vec![issue(P0, Command::Deploy { unit: root })]);
    assert!(rejected(&events).is_empty(), "{events:?}");
    if let Some(inner) = inner {
        assert!(entity(&mut app, inner).is_some());
        let events = tick(&mut app, vec![issue(P0, Command::Deploy { unit: inner })]);
        assert!(rejected(&events).is_empty(), "{events:?}");
    }
    assert_eq!(
        owner(&mut app, mcv),
        Some(P0),
        "same original viable MCV must really unload"
    );
    assert!(hp(&mut app, mcv) > 0);
    cells(&mut app);
}

#[test]
fn ra9_free_mcv_counts_actual_direct_cargo_and_original_unload() {
    ordinary_with_cargo(false);
}
#[test]
fn ra9_free_mcv_counts_actual_nested_cargo_and_original_unload() {
    ordinary_with_cargo(true);
}

#[test]
fn ra9_free_mcv_uses_current_root_owner_after_native_control() {
    let (mut app, root, mcv, collector, _) = setup(false);
    put(&mut app, P1, "engineer", 23, 21);
    tick(&mut app, vec![]); // genuine shared P1 sight before the new Yuri order
    let yuri = put(&mut app, P1, "yuri", 23, 20);
    let events = tick(
        &mut app,
        vec![issue(
            P1,
            Command::Attack {
                units: vec![yuri],
                target: root,
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    for _ in 0..100 {
        if owner(&mut app, root) == Some(P1) {
            break;
        }
        tick(&mut app, vec![]);
    }
    assert_eq!(
        owner(&mut app, root),
        Some(P1),
        "real root ownership transfer"
    );
    assert_eq!(
        app.world().resource::<SpecialState>().controlled[&root].0,
        yuri
    );
    assert!(entity(&mut app, mcv).is_none());
    let (content, spawned) = collect_crate(&mut app, collector, CellPos::new(12, 12));
    assert_eq!(
        content,
        CrateContent::Unit,
        "enemy root cargo does not rescue original owner"
    );
    let new = spawned.expect("genuinely stranded P0 must get a base unit");
    let e = entity(&mut app, new).unwrap();
    let kind = app.world().get::<Unit>(e).unwrap().kind;
    assert!(defs(&app).rules().unwrap().is_base_unit(kind));
    assert_eq!(app.world().get::<Owner>(e).unwrap().0, P0);
    cells(&mut app);
}

#[test]
fn ra9_free_mcv_ignores_actual_destroyed_cargo_payload() {
    let (mut app, root, mcv, collector, _) = setup(false);
    // Enemy sphere does not enter P0's no-building census.
    put(&mut app, P1, "nuclear_reactor", 40, 40);
    let sphere = put(&mut app, P1, "chronosphere", 3, 3);
    put(&mut app, P1, "spy_satellite", 3, 8);
    let bad = CellPos::new(30, 30);
    app.world_mut().resource_mut::<Grid>().set_terrain(
        bad,
        Cell {
            passable: false,
            water: false,
            buildable: false,
            ore: 0,
        },
    );
    steps(&mut app, 2);
    charge(&mut app, sphere);
    let source = cell(&mut app, root);
    let events = tick(
        &mut app,
        vec![issue(
            P1,
            Command::Chronoshift {
                building: sphere,
                source,
                target: bad,
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(
        entity(&mut app, root).is_none(),
        "native invalid landing must kill carrier"
    );
    assert!(entity(&mut app, mcv).is_none());
    assert!(
        !app.world()
            .resource::<SpecialState>()
            .cargo
            .contains_key(&root)
    );
    let (content, spawned) = collect_crate(&mut app, collector, CellPos::new(12, 12));
    assert_eq!(content, CrateContent::Unit);
    assert!(
        spawned.is_some(),
        "actual payload loss restores stranded positive"
    );
    cells(&mut app);
}

#[test]
fn ra9_free_mcv_on_map_base_and_truly_stranded_neighbors() {
    for present in [true, false] {
        let mut app = world();
        app.world_mut().insert_resource(only_powerup("reveal"));
        if present {
            put(&mut app, P0, "mcv", 20, 20);
        }
        let collector = put(&mut app, P0, "engineer", 10, 12);
        crate_at(&mut app, CellPos::new(12, 12));
        let (content, spawned) = collect_crate(&mut app, collector, CellPos::new(12, 12));
        assert_eq!(
            content,
            if present {
                CrateContent::Reveal
            } else {
                CrateContent::Unit
            }
        );
        assert_eq!(spawned.is_some(), !present);
        cells(&mut app);
    }
}
