//! UNVALIDATED native Ai/Visibility schedule body. Proposed ai/ra9_keys_tests.rs + cfg(test) mod.
//! Cadence2 is an explicit scenario fixture; no stock-cadence or all-AIs-see claim.
use super::*;
use bevy_app::App;
#[test]
fn ra9_public_reveal_obeys_actual_fog_gap_and_ai_turn_alignment() {
    for fog in [false, true] {
        for gap in [false, true] {
            let data =
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data");
            let definitions = crate::load_ruleset(&data).expect("shipped");
            let mut skirmish =
                crate::load_skirmish(data.join("skirmish.ron"), &definitions).expect("skirmish");
            skirmish.maps[0].starts = vec![
                CellPos::new(8, 8),
                CellPos::new(38, 38),
                CellPos::new(38, 8),
            ];
            skirmish.starting_credits = 0;
            let mut ai = crate::load_ai(data.join("ai.ron"), &definitions).expect("ai");
            let difficulty = ai
                .difficulties
                .iter()
                .position(|d| d.id == "brutal")
                .expect("difficulty");
            ai.difficulties[difficulty].think_interval = 2;
            let setups = ["germany", "great_britain", "russia"]
                .into_iter()
                .enumerate()
                .map(|(i, country)| crate::PlayerSetup {
                    player: PlayerId(i as u8),
                    faction: skirmish
                        .factions
                        .iter()
                        .position(|f| f.id == country)
                        .expect("faction"),
                    start: i,
                    controller: if i < 2 {
                        crate::Controller::Ai { difficulty }
                    } else {
                        crate::Controller::Human
                    },
                })
                .collect::<Vec<_>>();
            let mut app = App::new();
            app.add_plugins(crate::SimPlugin { seed: 11 });
            app.world_mut().insert_resource(
                Grid::from_cells(
                    48,
                    48,
                    vec![
                        crate::Cell {
                            passable: true,
                            water: false,
                            buildable: true,
                            ore: 0,
                        };
                        48 * 48
                    ],
                )
                .expect("grid"),
            );
            app.world_mut().insert_resource(definitions);
            crate::setup_match_configured(
                app.world_mut(),
                &skirmish,
                &ai,
                0,
                &setups,
                &crate::MatchOptions {
                    mcv_start: false,
                    crates: false,
                    unit_count: Some(0),
                    short_game: false,
                    fog_of_war: fog,
                    ..Default::default()
                },
            )
            .expect("match");
            crate::spawn_unit(app.world_mut(), PlayerId(0), "engineer", CellPos::new(7, 8))
                .expect("actor");
            crate::spawn_unit(
                app.world_mut(),
                PlayerId(1),
                "engineer",
                CellPos::new(37, 38),
            )
            .expect("actor");
            let host = crate::spawn_building(
                app.world_mut(),
                PlayerId(2),
                "chronosphere",
                CellPos::new(24, 12),
            )
            .expect("host");
            let unrelated = crate::spawn_building(
                app.world_mut(),
                PlayerId(2),
                "power_plant",
                CellPos::new(30, 2),
            )
            .expect("unrelated");
            let spy = crate::spawn_unit(app.world_mut(), PlayerId(2), "spy", CellPos::new(25, 15))
                .expect("spy");
            if gap {
                crate::spawn_building(
                    app.world_mut(),
                    PlayerId(2),
                    "gap_generator",
                    CellPos::new(24, 16),
                )
                .expect("gap");
                crate::spawn_building(
                    app.world_mut(),
                    PlayerId(2),
                    "nuclear_reactor",
                    CellPos::new(3, 40),
                )
                .expect("power");
            }
            app.world_mut().run_schedule(crate::SimSchedule); // tick0 Ai BEFORE first reveal.
            for p in [PlayerId(0), PlayerId(1)] {
                assert!(
                    !app.world()
                        .resource::<AiPlayers>()
                        .controller(p)
                        .expect("ai")
                        .known_buildings()
                        .contains_key(&host)
                );
                assert!(
                    app.world()
                        .resource::<crate::SpecialState>()
                        .hidden
                        .contains(&(p, spy))
                );
            }
            app.world_mut().run_schedule(crate::SimSchedule); // tick1 observer1 due.
            assert_eq!(
                app.world()
                    .resource::<AiPlayers>()
                    .controller(PlayerId(1))
                    .expect("ai")
                    .known_buildings()
                    .contains_key(&host),
                !gap
            );
            app.world_mut().run_schedule(crate::SimSchedule); // tick2 observer0 due, sight expired with fog.
            assert_eq!(
                app.world()
                    .resource::<AiPlayers>()
                    .controller(PlayerId(0))
                    .expect("ai")
                    .known_buildings()
                    .contains_key(&host),
                !gap && !fog
            );
            for p in [PlayerId(0), PlayerId(1)] {
                assert!(
                    !app.world()
                        .resource::<AiPlayers>()
                        .controller(p)
                        .expect("ai")
                        .known_buildings()
                        .contains_key(&unrelated)
                );
            }
        }
    }
}
