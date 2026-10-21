//! RA9: actual shipped yard starts for every supported map/player count (START-01/R56-38).
//! Setup and bounded public movement only; this is not an all-to-Won or art/Prop integration gate.
use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{Building, CellPos, Command, CommandQueue, Controller, Definitions, Grid, IssuedCommand,
    MatchOptions, Mobile, Owner, PlayerId, PlayerSetup, Players, SimEvent, SimId, SimPlugin,
    SimSchedule, Unit, enlarged_map, find_path, load_ai, load_map, load_ruleset, load_skirmish,
    map_asset_path, setup_match_configured};
use std::{collections::BTreeSet, path::PathBuf};

#[test]
fn ra9_shipped_yard_starts_for_all_maps_two_to_ten_players_have_legal_occupancy_and_public_exits() {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let data = assets.join("data");
    let definitions = load_ruleset(&data).expect("shipped definitions");
    let skirmish = load_skirmish(data.join("skirmish.ron"), &definitions).expect("skirmish");
    let ai = load_ai(data.join("ai.ron"), &definitions).expect("AI");
    let mut scenarios = 0;
    for (map, spec) in skirmish.maps.iter().enumerate() {
        let file = load_map(assets.join(map_asset_path(&spec.id))).expect("shipped map");
        for grow in [false, true] {
            let most = if grow { sim::MAX_MATCH_PLAYERS } else { spec.starts.len().min(sim::MAX_MATCH_PLAYERS) };
            for count in sim::MIN_MATCH_PLAYERS..=most {
                let prepared = enlarged_map(&file.grid, &file.looks, &spec.starts,
                    if grow { count } else { sim::MIN_MATCH_PLAYERS }).expect("actual map growth");
                let mut starts = skirmish.clone();
                starts.maps[map].starts = prepared.starts;
                let players: Vec<_> = (0..count).map(|slot| PlayerSetup {
                    player: PlayerId(slot as u8), faction: slot % skirmish.factions.len(),
                    start: slot, controller: Controller::Human,
                }).collect();
                let mut app = App::new();
                app.add_plugins(SimPlugin { seed: 19 });
                app.world_mut().insert_resource(prepared.grid);
                app.world_mut().resource_mut::<Grid>().mark_world_features(&prepared.looks);
                app.world_mut().insert_resource(definitions.clone());
                let result = setup_match_configured(app.world_mut(), &starts, &ai, map, &players,
                    &MatchOptions { mcv_start: false, unit_count: Some(10), ..MatchOptions::default() });
                assert!(result.is_ok(), "{} grow{grow} players{count}: {result:?}", spec.id);
                let yards: Vec<_> = {
                    let world = app.world_mut();
                    world.query::<(&Owner, &Building)>().iter(world)
                        .map(|(owner, building)| (owner.0, building.footprint)).collect()
                };
                assert_eq!(yards.len(), count);
                for setup in &players {
                    let own: Vec<_> = yards.iter().filter(|(owner, _)| *owner == setup.player).collect();
                    assert_eq!(own.len(), 1);
                    assert_eq!(own[0].1.origin, starts.maps[map].starts[setup.start]);
                }
                assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
                assert!(sim::stray_claims(app.world_mut()).is_empty());
                let mut units: Vec<_> = {
                    let world = app.world_mut();
                    world.query::<(&SimId, &Owner, &Unit, &Mobile)>().iter(world)
                        .map(|(id, owner, unit, mobile)| (*id, owner.0, unit.kind, mobile.cell)).collect()
                };
                units.sort_by_key(|(id, ..)| *id);
                assert!(units.len() >= count);
                for (_, _, _, cell) in &units {
                    assert!(!yards.iter().any(|(_, footprint)| footprint.contains(*cell)));
                }
                // One real starting unit per player must leave through ordinary command validation,
                // pathfinding and registered movement, rather than a direct relocation/test-only exit.
                let mut selected = BTreeSet::new();
                let mut probes = Vec::new();
                let mut claimed = BTreeSet::new();
                for (id, owner, kind, from) in units {
                    if selected.contains(&owner) { continue; }
                    let grid = app.world().resource::<Grid>();
                    let view = grid.navigation_for(definitions.domain(kind));
                    let destination = (-4i32..=4).flat_map(|dy| (-4i32..=4).map(move |dx| (dx, dy)))
                        .filter(|(dx, dy)| dx.abs().max(dy.abs()) >= 2)
                        .map(|(dx, dy)| CellPos::new(from.x + dx, from.y + dy))
                        .find(|to| !claimed.contains(to) && view.is_passable(*to)
                            && view.has_room(*to, definitions.on_foot(kind))
                            && !yards.iter().any(|(_, footprint)| footprint.contains(*to))
                            && find_path(&view, from, *to, |cell| yards.iter().any(|(_, footprint)| footprint.contains(cell))).is_some());
                    let Some(destination) = destination else { continue; };
                    selected.insert(owner); claimed.insert(destination);
                    probes.push((id, owner, from, destination));
                }
                assert_eq!(selected.len(), count, "{} grow{grow} players{count}", spec.id);
                {
                    let mut queue = app.world_mut().resource_mut::<CommandQueue>();
                    for &(id, player, _, target) in &probes { queue.push(IssuedCommand {
                        player, command: Command::Move { units: vec![id], target },
                    }); }
                }
                let mut completed = BTreeSet::new();
                for _ in 0..256 {
                    app.world_mut().run_schedule(SimSchedule);
                    for event in app.world().resource::<Messages<SimEvent>>().iter_current_update_messages() {
                        assert!(!matches!(event, SimEvent::CommandRejected { .. }), "{event:?}");
                        if let SimEvent::MoveCompleted { unit, .. } = event { completed.insert(*unit); }
                    }
                    assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
                    assert!(sim::stray_claims(app.world_mut()).is_empty());
                    assert!(app.world().resource::<Players>().iter().all(|(_, state)| state.credits() >= 0));
                    if probes.iter().all(|(id, ..)| completed.contains(id)) { break; }
                }
                for (id, _, from, target) in probes {
                    assert_ne!(from, target);
                    assert!(completed.contains(&id), "{} grow{grow} players{count}: {id:?} {from:?}->{target:?}", spec.id);
                }
                scenarios += 1;
            }
        }
    }
    assert!(scenarios >= skirmish.maps.len() * 9);
    eprintln!("RA9_YARD_START_SUCCESS maps={} scenarios={scenarios} occupancy/public exits checked", skirmish.maps.len());
}
