//! Historical preparation note: UNVALIDATED / PENDING. Native nested individual BuildLimit rejection and death release.
//! The temporary metadata variant is test fixture data, never a canonical generated RON edit.
mod ra9_soundness_common;
use ra9_soundness_common::*;
use sim::{Cell, CellPos, Command, Grid, ProductionItem, RejectReason, SimEvent};

#[test]
fn ra9_individual_limit_counts_grandchild_then_releases_it_after_real_root_death() {
    let mut app = world();
    let source = std::fs::read_to_string(data_dir().join("arsenal.ron")).unwrap();
    let mut changed = 0;
    let altered = source
        .lines()
        .map(|line| {
            if line.contains("id: \"gi\"") {
                assert_eq!(line.matches("build_limit: 0").count(), 1);
                changed += 1;
                line.replacen("build_limit: 0", "build_limit: 1", 1)
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        changed, 1,
        "one explicit balance-independent fixture override"
    );
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ra9-nested-limit");
    std::fs::create_dir_all(&dir).unwrap();
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = dir.join(format!(
        "gi-{}-{:?}-{}.ron",
        std::process::id(),
        std::thread::current().id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .and_then(|mut f| f.write_all(altered.as_bytes()))
        .unwrap();
    let definitions = defs(&app).with_arsenal(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    app.world_mut().insert_resource(definitions);
    put(&mut app, P0, "construction_yard", 1, 1);
    put(&mut app, P0, "power_plant", 5, 1);
    put(&mut app, P0, "barracks", 1, 5);
    let outer = put(&mut app, P0, "amphibious_transport", 11, 10);
    let inner = put(&mut app, P0, "flak_track", 10, 10);
    let soldier = put(&mut app, P0, "gi", 9, 10);
    assert!(rejected(&interact(&mut app, soldier, inner)).is_empty());
    assert!(rejected(&interact(&mut app, inner, outer)).is_empty());
    steps(&mut app, 2);
    assert_eq!(cargo(&app, outer), vec![inner]);
    assert_eq!(cargo(&app, inner), vec![soldier]);
    assert!(
        entity(&mut app, soldier).is_none(),
        "limited unit is a genuine off-map grandchild"
    );
    let gi = ProductionItem::Unit(defs(&app).unit_kind("gi").unwrap());
    let events = tick(
        &mut app,
        vec![issue(P0, Command::QueueProduction { item: gi })],
    );
    assert!(
        events.iter().any(|e| matches!(e,SimEvent::CommandRejected {
        reason:RejectReason::BuildLimitReached {item,limit:1},..
    } if *item==gi)),
        "{events:?}"
    );

    put(&mut app, P0, "nuclear_reactor", 35, 35);
    let sphere = put(&mut app, P0, "chronosphere", 6, 6);
    put(&mut app, P0, "spy_satellite", 3, 8);
    let target = CellPos::new(30, 30);
    app.world_mut().resource_mut::<Grid>().set_terrain(
        target,
        Cell {
            passable: false,
            water: false,
            buildable: false,
            ore: 0,
        },
    );
    steps(&mut app, 2);
    charge(&mut app, sphere);
    let source = cell(&mut app, outer);
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Chronoshift {
                building: sphere,
                source,
                target,
            },
        )],
    );
    assert!(rejected(&events).is_empty(), "{events:?}");
    assert!(
        entity(&mut app, outer).is_none(),
        "native failed landing must kill root"
    );
    let events = tick(
        &mut app,
        vec![issue(P0, Command::QueueProduction { item: gi })],
    );
    assert!(
        rejected(&events).is_empty(),
        "native payload death must release the individual limit: {events:?}"
    );
    cells(&mut app);
}
