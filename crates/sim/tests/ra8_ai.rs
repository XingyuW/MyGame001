//! RA8 shipped controller acceptance, through match setup, commands and SimSchedule.
use std::path::PathBuf;

use bevy_app::App;
use sim::{
    Cell, CellPos, Command, CommandQueue, Controller, Grid, MatchOptions, PlayerId, PlayerSetup,
    SimPlugin, SimSchedule, SimTick, load_ai, load_ruleset, load_skirmish, setup_match_configured,
    spawn_unit,
};

fn data() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data")
}

fn start(country: &str, difficulty: &str) -> App {
    let definitions = load_ruleset(data()).unwrap();
    let mut skirmish = load_skirmish(data().join("skirmish.ron"), &definitions).unwrap();
    skirmish.maps[0].starts = vec![CellPos::new(8, 8), CellPos::new(38, 38)];
    let ai = load_ai(data().join("ai.ron"), &definitions).unwrap();
    let difficulty = ai
        .difficulties
        .iter()
        .position(|d| d.id == difficulty)
        .unwrap();
    let setups = [
        PlayerSetup {
            player: PlayerId(0),
            faction: skirmish
                .factions
                .iter()
                .position(|f| f.id == country)
                .unwrap(),
            start: 0,
            controller: Controller::Ai { difficulty },
        },
        PlayerSetup {
            player: PlayerId(1),
            faction: skirmish
                .factions
                .iter()
                .position(|f| f.id == "russia")
                .unwrap(),
            start: 1,
            controller: Controller::Human,
        },
    ];
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 8 });
    app.world_mut().insert_resource(
        Grid::from_cells(
            48,
            48,
            vec![
                Cell {
                    passable: true,
                    buildable: true,
                    water: false,
                    ore: 0
                };
                48 * 48
            ],
        )
        .unwrap(),
    );
    app.world_mut().insert_resource(definitions);
    setup_match_configured(
        app.world_mut(),
        &skirmish,
        &ai,
        0,
        &setups,
        &MatchOptions {
            mcv_start: false,
            crates: false,
            ..MatchOptions::default()
        },
    )
    .unwrap();
    app
}

#[test]
fn ra8_raw_spy_spawn_with_empty_queue_is_concealed_before_actual_ai_thinks() {
    let mut app = start("great_britain", "easy");
    app.world_mut().run_schedule(SimSchedule);
    // Drain first-turn ordinary decisions, then start precisely on the next P0 think.
    app.world_mut().run_schedule(SimSchedule);
    app.world_mut().insert_resource(CommandQueue::default());
    app.world_mut().resource_mut::<SimTick>().0 = 40;
    for (owner, mut orders, weapon) in app
        .world_mut()
        .query::<(&sim::Owner, &mut sim::Orders, Option<&mut sim::Weapon>)>()
        .iter_mut(app.world_mut())
    {
        if owner.0 == PlayerId(0) {
            orders.current = None;
            if let Some(mut weapon) = weapon {
                weapon.order = None;
            }
        }
    }
    let target = CellPos::new(12, 10);
    let spy = spawn_unit(app.world_mut(), PlayerId(1), "spy", target).unwrap();
    assert!(
        !app.world()
            .resource::<sim::SpecialState>()
            .hidden
            .contains(&(PlayerId(0), spy))
    );
    assert!(app.world().resource::<CommandQueue>().is_empty());
    assert!(
        app.world()
            .resource::<sim::Players>()
            .get(PlayerId(0))
            .unwrap()
            .vision()
            .is_visible(target)
    );
    app.world_mut().run_schedule(SimSchedule);
    let commands: Vec<_> = app.world().resource::<CommandQueue>().iter().collect();
    assert!(!commands.iter().any(|issued| matches!(issued.command, Command::AttackMove { target: found, .. } if found == target)), "concealed Spy affected actual controller decisions: {commands:?}");
    assert!(
        app.world()
            .resource::<sim::SpecialState>()
            .hidden
            .contains(&(PlayerId(0), spy))
    );
}
