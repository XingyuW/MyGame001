//! Opt-in renderer capture for art review. MYGAME_VISUAL_REVIEW names an existing output
//! directory. This is separate from smoke tests and ordinary play; screenshots include a
//! labelled developed-base setup, installed through the same pre-tick match setup as skirmish.
//! Never changes a running simulation. The production catalogue is restored after setup.
//!
//! Since RA1 the factions are countries, which start with the base unit of their side. A scene
//! adds its buildings and units to the starting lists of the human player's country, which match
//! setup places besides what the country starts with, and starts with the Construction Yards
//! instead of the base units ([`begin_scene`]), so that the base stands at the start position
//! from the first frame; the starting units of both players stand around their yards.
use crate::skirmish::{Catalogue, MenuSelection, begin_match, begin_match_with};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use std::path::PathBuf;

#[derive(Component)]
struct ReviewLabel;

#[derive(Resource)]
struct Review {
    directory: PathBuf,
    frame: u32,
    last_step: f64,
}

/// Art review only: terrain is drawn as if seen, so whole maps can be inspected. Presentation
/// only; units, buildings and the simulation's vision are unchanged.
#[derive(Resource)]
pub struct RevealTerrain;

/// MYGAME_MAP_REVIEW: a tour of every map from the fourth on (the large maps), revealing the
/// terrain and capturing the map's centre and the first start at two zoom levels.
/// MYGAME_MAP_REVIEW_ENEMIES gives the toured matches that many computer players (1 to 9), to
/// review the maps as they are enlarged for more players, and MYGAME_MAP_REVIEW_START numbers the
/// start position the "base" capture shows (the made ones come last); the frame times of each
/// map are logged, with all of its terrain drawn.
pub fn enable_maps(app: &mut App, directory: PathBuf) {
    app.insert_resource(Review {
        directory,
        frame: 0,
        last_step: 0.0,
    })
    .insert_resource(RevealTerrain)
    .init_resource::<FrameTimes>()
    .add_systems(Update, record_frame_time)
    .add_systems(Last, tour_maps);
}

/// The frames drawn since a map of the tour was started: how many, and their total and longest
/// time in seconds.
#[derive(Resource, Default)]
struct FrameTimes {
    frames: u32,
    total: f32,
    longest: f32,
}

fn record_frame_time(time: Res<Time<Real>>, mut times: ResMut<FrameTimes>) {
    let frame = time.delta_secs();
    times.frames += 1;
    times.total += frame;
    times.longest = times.longest.max(frame);
}

fn tour_maps(world: &mut World) {
    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    if now - world.resource::<Review>().last_step < 1.0 / 30.0 {
        return;
    }
    let (frame, dir) = {
        let mut review = world.resource_mut::<Review>();
        review.last_step = now;
        review.frame += 1;
        (review.frame, review.directory.clone())
    };
    let count = world.resource::<Catalogue>().maps.len();
    // Each map: start the match, then four shots 25 frames apart.
    const STEP: u32 = 25;
    const PER_MAP: u32 = STEP * 5;
    // MYGAME_MAP_REVIEW_FROM names the first map of the tour (to review only the newest maps).
    let first = std::env::var("MYGAME_MAP_REVIEW_FROM")
        .ok()
        .and_then(|id| {
            world
                .resource::<Catalogue>()
                .maps
                .iter()
                .position(|map| map.id == id)
        })
        .unwrap_or(3)
        .min(count.saturating_sub(1));
    let index = first + ((frame.saturating_sub(10)) / PER_MAP) as usize;
    if frame < 10 {
        return;
    }
    if index >= count {
        world.write_message(AppExit::Success);
        return;
    }
    let phase = (frame - 10) % PER_MAP;
    let id = world.resource::<Catalogue>().maps[index].id.clone();
    if phase == 0 {
        let mut choice = MenuSelection::initial(world.resource::<Catalogue>());
        choice.map = index;
        choice.enemies = std::env::var("MYGAME_MAP_REVIEW_ENEMIES")
            .ok()
            .and_then(|enemies| enemies.parse::<usize>().ok())
            .map_or(0, |enemies| {
                enemies.clamp(1, sim::MAX_MATCH_PLAYERS - 1) - 1
            });
        begin_match(world, choice, 8);
        return;
    }
    // The frames of the match's first step build its terrain; the rest draw it.
    if phase == STEP {
        *world.resource_mut::<FrameTimes>() = FrameTimes::default();
    }
    if phase == PER_MAP - 1 {
        let times = world.resource::<FrameTimes>();
        let view = world.get_resource::<crate::map::MapView>();
        info!(
            "map review {id}: {} cells, {} frames, mean {:.2} ms, longest {:.2} ms",
            view.map_or(0, |view| view.width * view.height),
            times.frames,
            1000.0 * times.total / times.frames.max(1) as f32,
            1000.0 * times.longest
        );
    }
    // Move the camera at the start of a step and capture halfway through it: a screenshot shows
    // the frame rendered after the request, so capturing with the move would show the old view.
    let (shot, offset) = (phase / STEP, phase % STEP);
    if offset != 0 && offset != STEP / 2 {
        return;
    }
    let Some(view) = world.get_resource::<crate::map::MapView>().copied() else {
        return;
    };
    // A start on the map of the match, which may be enlarged: the human player's, or the one
    // MYGAME_MAP_REVIEW_START numbers (from 0; the starts made for an enlarged map come last).
    let shown = std::env::var("MYGAME_MAP_REVIEW_START")
        .ok()
        .and_then(|start| start.parse::<usize>().ok())
        .unwrap_or(0);
    let Some(start) = world
        .get_resource::<crate::map::MapStarts>()
        .and_then(|starts| starts.0.get(shown).or(starts.0.first()).copied())
    else {
        return;
    };
    let (target, scale, name) = match shot {
        1 => (
            sim::CellPos::new(view.width as i32 / 2, view.height as i32 / 2),
            1.0,
            "centre",
        ),
        2 => (
            sim::CellPos::new(view.width as i32 / 2, view.height as i32 / 2),
            3.0,
            "overview",
        ),
        3 => (start, 1.0, "base"),
        _ => (
            sim::CellPos::new(view.width as i32 / 4, view.height as i32 * 3 / 4),
            1.0,
            "flank",
        ),
    };
    if offset == STEP / 2 {
        world
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(dir.join(format!("map-{id}-{name}.png"))));
        return;
    }
    let centre = crate::projection::cell_centre(target, view.tile);
    let mut cameras = world.query_filtered::<(&mut Transform, &mut Projection), With<Camera2d>>();
    for (mut transform, mut projection) in cameras.iter_mut(world) {
        transform.translation = centre.extend(transform.translation.z);
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scale = scale;
        }
    }
}

/// Starts the match of a review scene: as [`begin_match`] with the seed of the review, but with
/// the players' Construction Yards in place of their base units.
fn begin_scene(world: &mut World, choice: MenuSelection) {
    begin_match_with(world, choice, 8, SCENE_MCV_START);
}

/// The lobby's "start with an MCV" for the review's scenes: off.
const SCENE_MCV_START: bool = false;

/// MYGAME_FEATURE_REVIEW: a short review of the interface around a developed base with every
/// aircraft type beside it: aircraft at their flying height over their shadows, a selected war
/// factory with its rally point, and the notices of defeated players in their team colours.
pub fn enable_features(app: &mut App, directory: PathBuf) {
    app.insert_resource(Review {
        directory,
        frame: 0,
        last_step: 0.0,
    })
    .add_systems(Last, capture_features);
}

/// The human player's base of [`base_review`] with an air base, a few tanks, and aircraft over
/// the open ground beside it, against two computer players.
fn feature_review(original: &Catalogue) -> (Catalogue, MenuSelection) {
    let (mut review, mut choice) = base_review(original);
    choice.enemies = 1;
    let faction = &mut review.skirmish.factions[choice.faction];
    if let Some(kind) = review.definitions.building_kind("airforce_command") {
        faction.buildings.push(sim::StartingBuilding {
            kind,
            offset: (10, -3),
        });
    }
    faction.units.clear();
    for (id, offset, count) in [
        ("grizzly", (13, 6), 3),
        ("harrier", (14, -5), 2),
        ("black_eagle", (15, 1), 2),
        ("kirov", (10, 3), 1),
        ("nighthawk", (6, -5), 1),
        ("rocketeer", (8, -7), 3),
    ] {
        let kind = review.definitions.unit_kind(id).expect("review unit");
        faction.roster.insert(sim::ProductionItem::Unit(kind));
        faction.units.push(sim::StartingGroup {
            kind,
            offset,
            count,
        });
    }
    (review, choice)
}

fn capture_features(world: &mut World) {
    use crate::units::HUMAN;
    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    if now - world.resource::<Review>().last_step < 1.0 / 60.0 {
        return;
    }
    let (frame, dir) = {
        let mut review = world.resource_mut::<Review>();
        review.last_step = now;
        review.frame += 1;
        (review.frame, review.directory.clone())
    };
    let look_at = |world: &mut World, offset: (i32, i32), scale: f32| {
        let Some(view) = world.get_resource::<crate::map::MapView>().copied() else {
            return;
        };
        let Some(start) = world
            .get_resource::<crate::map::MapStarts>()
            .and_then(|starts| starts.0.first().copied())
        else {
            return;
        };
        let target = sim::CellPos::new(start.x + offset.0, start.y + offset.1);
        let centre = crate::projection::cell_centre(target, view.tile);
        let mut cameras =
            world.query_filtered::<(&mut Transform, &mut Projection), With<Camera2d>>();
        for (mut transform, mut projection) in cameras.iter_mut(world) {
            transform.translation = centre.extend(transform.translation.z);
            if let Projection::Orthographic(ortho) = &mut *projection {
                ortho.scale = scale;
            }
        }
    };
    let shot = |world: &mut World, name: &str| {
        world
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(dir.join(name)));
    };
    match frame {
        10 => {
            let original = world.resource::<Catalogue>().clone();
            let (review, choice) = feature_review(&original);
            world.insert_resource(review);
            world.insert_resource(choice);
            begin_scene(world, choice);
            world.insert_resource(original);
        }
        // The war factory is selected and given a rally point, as a right click would.
        20 => {
            let host = world.resource::<crate::sim_host::SimHost>();
            let factory = host
                .definitions()
                .and_then(|definitions| definitions.building_kind("war_factory"))
                .and_then(|kind| {
                    let mut buildings = host
                        .world()
                        .try_query::<(&sim::SimId, &sim::Owner, &sim::Building)>()?;
                    buildings
                        .iter(host.world())
                        .find(|(_, owner, building)| owner.0 == HUMAN && building.kind == kind)
                        .map(|(id, _, building)| (*id, building.footprint.origin))
                });
            if let Some((building, origin)) = factory {
                let cell = sim::CellPos::new(origin.x + 12, origin.y - 1);
                world
                    .resource_mut::<crate::sim_host::CommandSink>()
                    .issue([sim::Command::SetRallyPoint { building, cell }]);
                world.resource_mut::<crate::selection::SelectedBuilding>().0 = Some(building);
            }
            look_at(world, (9, 1), 1.0);
        }
        130 => shot(world, "features-rally-and-flight.png"),
        // Two players fall (review only: the notices are shown, the match is not changed), and
        // the aircraft are selected.
        140 => {
            for player in [1, 2] {
                world.write_message(sim::SimEvent::PlayerDefeated {
                    player: sim::PlayerId(player),
                });
            }
            world.write_message(sim::SimEvent::PlayerDefeated { player: HUMAN });
            let host = world.resource::<crate::sim_host::SimHost>();
            let aircraft: std::collections::BTreeSet<sim::SimId> = host
                .world()
                .try_query::<(&sim::SimId, &sim::Owner, &sim::Unit)>()
                .map(|mut units| {
                    units
                        .iter(host.world())
                        .filter(|(id, owner, _)| {
                            owner.0 == HUMAN
                                && host
                                    .grid()
                                    .is_some_and(|grid| grid.domain_of(**id) == sim::Domain::Air)
                        })
                        .map(|(id, ..)| *id)
                        .collect()
                })
                .unwrap_or_default();
            world.resource_mut::<crate::selection::Selection>().0 = aircraft;
        }
        170 => shot(world, "features-defeat-notices.png"),
        180 => look_at(world, (13, 0), 0.5),
        210 => shot(world, "features-flight-close.png"),
        230 => {
            world.write_message(AppExit::Success);
        }
        _ => {}
    }
}

/// MYGAME_FX_REVIEW: weapons at work beside a developed base. Prism tanks, Tesla tanks and
/// troopers, battle tanks, GIs and a V3 launcher engage an armoured column, then the
/// infantry walks back past the cloning vats, which are selected and given a rally point.
pub fn enable_fx(app: &mut App, directory: PathBuf) {
    app.insert_resource(Review {
        directory,
        frame: 0,
        last_step: 0.0,
    })
    .add_systems(Last, capture_fx);
}

/// Where the armoured column of [`fx_review`] stands, from the human player's start.
const FX_TARGET: (i32, i32) = (20, 5);

fn fx_review(original: &Catalogue) -> (Catalogue, MenuSelection) {
    let (mut review, choice) = base_review(original);
    let starts = review.skirmish.maps[choice.map].starts.clone();
    let (home, away) = (starts[0], starts[1]);
    let faction = &mut review.skirmish.factions[choice.faction];
    if let Some(kind) = review.definitions.building_kind("cloning_vats") {
        // The first clear two-by-two plot beside the base.
        let grid = &review.maps[choice.map].map.grid;
        let clear = |offset: &(i32, i32)| {
            (0..2).all(|dy| {
                (0..2).all(|dx| {
                    let cell = sim::CellPos::new(home.x + offset.0 + dx, home.y + offset.1 + dy);
                    grid.cell(cell).is_some_and(|cell| cell.buildable)
                })
            })
        };
        let plots = [
            (8, 5),
            (8, -5),
            (9, -4),
            (-7, -3),
            (-7, 3),
            (4, -6),
            (-6, 7),
            (5, 11),
        ];
        if let Some(offset) = plots.into_iter().find(clear) {
            faction.roster.insert(sim::ProductionItem::Building(kind));
            faction
                .buildings
                .push(sim::StartingBuilding { kind, offset });
        }
    }
    faction.units.clear();
    for (id, offset, count) in [
        ("prism_tank", (12, 2), 2),
        ("tesla_tank", (13, 5), 2),
        ("grizzly", (13, 8), 2),
        ("gi", (12, -1), 4),
        ("tesla_trooper", (14, 10), 2),
        ("v3_launcher", (10, 6), 1),
        ("conscript", (11, 11), 3),
        ("attack_dog", (11, 0), 1),
    ] {
        let kind = review.definitions.unit_kind(id).expect("review unit");
        faction.roster.insert(sim::ProductionItem::Unit(kind));
        faction.units.push(sim::StartingGroup {
            kind,
            offset,
            count,
        });
    }
    // The column stands where the review looks, wherever its owner's base is.
    let shift = (home.x - away.x, home.y - away.y);
    let enemy = &mut review.skirmish.factions[choice.opponent];
    enemy.units.clear();
    for (id, offset, count) in [("rhino", (0, -2), 4), ("mcv", (1, 3), 2)] {
        enemy.units.push(sim::StartingGroup {
            kind: review.definitions.unit_kind(id).expect("enemy"),
            offset: (
                shift.0 + FX_TARGET.0 + offset.0,
                shift.1 + FX_TARGET.1 + offset.1,
            ),
            count,
        });
    }
    (review, choice)
}

fn capture_fx(world: &mut World) {
    use crate::units::HUMAN;
    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    if now - world.resource::<Review>().last_step < 1.0 / 60.0 {
        return;
    }
    let (frame, dir) = {
        let mut review = world.resource_mut::<Review>();
        review.last_step = now;
        review.frame += 1;
        (review.frame, review.directory.clone())
    };
    let start = |world: &World| {
        world
            .get_resource::<crate::map::MapStarts>()
            .and_then(|starts| starts.0.first().copied())
    };
    let look_at = |world: &mut World, offset: (i32, i32), scale: f32| {
        let (Some(view), Some(start)) = (
            world.get_resource::<crate::map::MapView>().copied(),
            start(world),
        ) else {
            return;
        };
        let target = sim::CellPos::new(start.x + offset.0, start.y + offset.1);
        let centre = crate::projection::cell_centre(target, view.tile);
        let mut cameras =
            world.query_filtered::<(&mut Transform, &mut Projection), With<Camera2d>>();
        for (mut transform, mut projection) in cameras.iter_mut(world) {
            transform.translation = centre.extend(transform.translation.z);
            if let Projection::Orthographic(ortho) = &mut *projection {
                ortho.scale = scale;
            }
        }
    };
    // The human player's units of the types `ids` (all armed ones for an empty list).
    let own = |world: &World, ids: &[&str]| -> Vec<sim::SimId> {
        let host = world.resource::<crate::sim_host::SimHost>();
        let Some(definitions) = host.definitions() else {
            return Vec::new();
        };
        let Some(mut units) = host
            .world()
            .try_query::<(&sim::SimId, &sim::Owner, &sim::Unit)>()
        else {
            return Vec::new();
        };
        units
            .iter(host.world())
            .filter(|(_, owner, unit)| {
                owner.0 == HUMAN
                    && definitions
                        .unit(unit.kind)
                        .is_some_and(|def| ids.is_empty() || ids.contains(&def.id.as_str()))
            })
            .map(|(id, ..)| *id)
            .collect()
    };
    let shot = |world: &mut World, name: String| {
        world
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(dir.join(name)));
    };
    match frame {
        10 => {
            let original = world.resource::<Catalogue>().clone();
            let (review, choice) = fx_review(&original);
            world.insert_resource(review);
            world.insert_resource(choice);
            begin_scene(world, choice);
            world.insert_resource(original);
        }
        30 => {
            look_at(world, (FX_TARGET.0, FX_TARGET.1), 0.62);
            if let Some(start) = start(world) {
                let units = own(world, &[]);
                let target = sim::CellPos::new(start.x + FX_TARGET.0, start.y + FX_TARGET.1);
                world
                    .resource_mut::<crate::sim_host::CommandSink>()
                    .issue([sim::Command::AttackMove { units, target }]);
            }
        }
        // The battle, every sixth of a second: beams and bolts last a third of one.
        60..=330 if frame % 10 == 0 => {
            shot(world, format!("fx-battle-{:02}.png", (frame - 60) / 10))
        }
        // The infantry walks home; the cloning vats are selected and given a rally point.
        340 => {
            look_at(world, (10, 4), 0.5);
            if let Some(start) = start(world) {
                let units = own(world, &["gi", "conscript", "tesla_trooper", "attack_dog"]);
                let target = sim::CellPos::new(start.x + 4, start.y + 6);
                let host = world.resource::<crate::sim_host::SimHost>();
                let vats = host
                    .definitions()
                    .and_then(|definitions| definitions.building_kind("cloning_vats"))
                    .and_then(|kind| {
                        let mut buildings =
                            host.world()
                                .try_query::<(&sim::SimId, &sim::Owner, &sim::Building)>()?;
                        buildings
                            .iter(host.world())
                            .find(|(_, owner, building)| owner.0 == HUMAN && building.kind == kind)
                            .map(|(id, ..)| *id)
                    });
                let mut commands = vec![sim::Command::Move { units, target }];
                if let Some(building) = vats {
                    commands.push(sim::Command::SetRallyPoint {
                        building,
                        cell: sim::CellPos::new(start.x + 12, start.y + 8),
                    });
                }
                world
                    .resource_mut::<crate::sim_host::CommandSink>()
                    .issue(commands);
                world.resource_mut::<crate::selection::SelectedBuilding>().0 = vats;
            }
        }
        380..=470 if frame % 15 == 5 => {
            shot(world, format!("fx-walk-{:02}.png", (frame - 380) / 15))
        }
        490 => {
            world.write_message(AppExit::Success);
        }
        _ => {}
    }
}

pub fn enable(app: &mut App, directory: PathBuf) {
    app.insert_resource(Review {
        directory,
        frame: 0,
        last_step: 0.0,
    })
    .add_systems(Last, capture);
}
fn capture(world: &mut World) {
    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    if now - world.resource::<Review>().last_step < 1.0 / 60.0 {
        return;
    }
    world.resource_mut::<Review>().last_step = now;
    let (frame, dir) = {
        let mut review = world.resource_mut::<Review>();
        review.frame += 1;
        (review.frame, review.directory.clone())
    };
    let filename = match frame {
        30 => Some("m8-menu.png"),
        120 => Some("m8-match.png"),
        210 => Some("m8-base-review.png"),
        300 => Some("m8-crossroads-meridian.png"),
        390 => Some("m8-riverlands-vanguard.png"),
        450 => Some("m9-armory.png"),
        510 => Some("m9-infantry.png"),
        570 => Some("m9-vehicles.png"),
        690 => Some("hd-technology.png"),
        840 => Some("hd-combat.png"),
        900 => Some("followup-menu-players.png"),
        990 => Some("followup-four-enemies-speed.png"),
        1080 => Some("distinct-roster.png"),
        _ => None,
    };
    if let Some(name) = filename {
        world
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(dir.join(name)));
    }
    if frame == 60 {
        let choice = *world.resource::<MenuSelection>();
        begin_match(world, choice, 8);
    }
    if frame == 150 {
        let original = world.resource::<Catalogue>().clone();
        let (review, choice) = base_review(&original);
        world.insert_resource(review);
        begin_scene(world, choice);
        world.insert_resource(original);
        world.spawn((
            ReviewLabel,
            Text::new("ART REVIEW / DEVELOPED BASE SETUP"),
            TextFont::from_font_size(13.0),
            TextColor(Color::srgb(0.90, 0.80, 0.48)),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(20.0),
                top: Val::Px(20.0),
                ..default()
            },
            GlobalZIndex(5),
        ));
    }
    if frame == 240 || frame == 330 {
        // The menu's first choice (RA1: America against the first Soviet country), and on the
        // second map the two the other way round.
        let initial = MenuSelection::initial(world.resource::<Catalogue>());
        let mut choice = initial;
        choice.map = if frame == 240 { 1 } else { 2 };
        if frame == 240 {
            (choice.faction, choice.opponent) = (initial.opponent, initial.faction);
        }
        begin_match(world, choice, 8);
        // Remove the setup label with the old match; these are ordinary shipped starts.
        let labels: Vec<Entity> = world
            .query_filtered::<Entity, With<ReviewLabel>>()
            .iter(world)
            .collect();
        for entity in labels {
            world.despawn(entity);
        }
    }
    if frame == 420 {
        world.insert_resource(crate::sidebar::Tab::Armory);
    }
    if frame == 480 {
        world.insert_resource(crate::sidebar::Tab::Infantry);
    }
    if frame == 540 {
        world.insert_resource(crate::sidebar::Tab::Vehicles);
    }
    if frame == 600 {
        let original = world.resource::<Catalogue>().clone();
        let (review, choice) = technology_review(&original);
        world.insert_resource(review);
        begin_scene(world, choice);
        world.insert_resource(original);
        world.insert_resource(crate::sidebar::Tab::Vehicles);
        world.insert_resource(crate::sidebar::BuildPage(4));
        world.spawn((
            ReviewLabel,
            Text::new("ART REVIEW / TECHNOLOGY & SILHOUETTES"),
            TextFont::from_font_size(13.0),
            TextColor(Color::srgb(0.9, 0.8, 0.48)),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(20.0),
                top: Val::Px(20.0),
                ..default()
            },
        ));
    }
    if frame == 720 {
        let original = world.resource::<Catalogue>().clone();
        let (review, choice) = combat_review(&original);
        world.insert_resource(review);
        begin_scene(world, choice);
        world.insert_resource(original);
        let mut labels = world.query_filtered::<&mut Text, With<ReviewLabel>>();
        for mut label in labels.iter_mut(world) {
            **label = "ART REVIEW / LIVE COMBAT SETUP".into();
        }
    }
    // The multi-enemy menu (four computer players, each with its colour in the player list,
    // "unlock all" chosen), then that match running at 3x speed.
    if frame == 870 {
        crate::skirmish::end_match(world);
        let labels: Vec<Entity> = world
            .query_filtered::<Entity, With<ReviewLabel>>()
            .iter(world)
            .collect();
        for entity in labels {
            world.despawn(entity);
        }
        let mut choice = MenuSelection::initial(world.resource::<Catalogue>());
        choice.enemies = 3;
        choice.unlock_all = true;
        world.insert_resource(choice);
    }
    if frame == 930 {
        let choice = *world.resource::<MenuSelection>();
        begin_match(world, choice, 8);
        let mut speed = world.resource_mut::<crate::game_speed::GameSpeed>();
        speed.step(true);
        speed.step(true);
        speed.step(true);
        world.insert_resource(crate::sidebar::Tab::Infantry);
    }
    // Every ground vehicle and infantry type side by side, to check that each reads distinctly.
    if frame == 1020 {
        let original = world.resource::<Catalogue>().clone();
        let (review, choice) = roster_review(&original);
        world.insert_resource(review);
        begin_scene(world, choice);
        world.insert_resource(original);
        world.spawn((
            ReviewLabel,
            Text::new("ART REVIEW / ROSTER LINE-UP"),
            TextFont::from_font_size(13.0),
            TextColor(Color::srgb(0.9, 0.8, 0.48)),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(20.0),
                top: Val::Px(20.0),
                ..default()
            },
        ));
    }
    if frame == 1110 {
        world.write_message(AppExit::Success);
    }
}

/// Vehicles in two rows, then infantry in two, each type once, beside the developed base. (The
/// ten types of the armies before RA1, which the shipped data no longer defines, have left the
/// line-up: scout, raider, light tank, artillery, heavy tank, rocket launcher, harvester,
/// rifleman, rocket soldier and grenadier.)
pub(crate) const ROSTER_LINEUP: [&str; 28] = [
    "ifv",
    "flak_track",
    "grizzly",
    "rhino",
    "tank_destroyer",
    "mirage_tank",
    "prism_tank",
    "tesla_tank",
    "apocalypse",
    "v3_launcher",
    "demolition_truck",
    "mcv",
    "war_miner",
    "chrono_miner",
    "gi",
    "conscript",
    "engineer",
    "rocketeer",
    "spy",
    "tanya",
    "chrono_legionnaire",
    "sniper",
    "flak_trooper",
    "tesla_trooper",
    "crazy_ivan",
    "yuri",
    "desolator",
    "terrorist",
];

/// How many entries of [`ROSTER_LINEUP`] are vehicles, which come first.
const LINEUP_VEHICLES: usize = 14;

fn roster_review(original: &Catalogue) -> (Catalogue, MenuSelection) {
    let (mut review, choice) = base_review(original);
    let faction = &mut review.skirmish.factions[choice.faction];
    faction.units.clear();
    for (i, id) in ROSTER_LINEUP.iter().enumerate() {
        let kind = review.definitions.unit_kind(id).expect("roster unit");
        let (row, column) = if i < LINEUP_VEHICLES {
            (i / 7, i % 7)
        } else {
            (
                LINEUP_VEHICLES / 7 + (i - LINEUP_VEHICLES) / 9,
                (i - LINEUP_VEHICLES) % 9,
            )
        };
        faction.roster.insert(sim::ProductionItem::Unit(kind));
        faction.units.push(sim::StartingGroup {
            kind,
            offset: (5 + 2 * column as i32, 5 + 2 * row as i32),
            count: 1,
        });
    }
    (review, choice)
}

fn base_review(original: &Catalogue) -> (Catalogue, MenuSelection) {
    let mut review = original.clone();
    // The human player's country against the first of the other side (RA1), whose start the
    // scene leaves as it is. The yard of the base is the one the scene starts with
    // ([`begin_scene`]), at the start position.
    let choice = MenuSelection::initial(&review);
    for (id, offset) in [
        ("power_plant", (4, -2)),
        ("refinery", (-3, 3)),
        ("barracks", (4, 3)),
        ("war_factory", (0, 7)),
        ("tech_center", (-3, -1)),
        ("pillbox", (8, 0)),
    ] {
        if let Some(kind) = review.definitions.building_kind(id) {
            review.skirmish.factions[choice.faction]
                .buildings
                .push(sim::StartingBuilding { kind, offset });
        }
    }
    (review, choice)
}

fn technology_review(original: &Catalogue) -> (Catalogue, MenuSelection) {
    let (mut review, choice) = base_review(original);
    for (old, new) in [
        ("power_plant", "tesla_reactor"),
        ("tech_center", "nuclear_reactor"),
        ("barracks", "chronosphere"),
        ("pillbox", "tesla_coil"),
    ] {
        let old = review
            .definitions
            .building_kind(old)
            .expect("base building");
        let new = review.definitions.building_kind(new).expect("HD building");
        let faction = &mut review.skirmish.factions[choice.faction];
        faction.roster.insert(sim::ProductionItem::Building(new));
        for building in &mut faction.buildings {
            if building.kind == old {
                building.kind = new;
            }
        }
    }
    let faction = &mut review.skirmish.factions[choice.faction];
    faction.units.clear();
    for (id, offset, count) in [
        ("apocalypse", (4, 1), 2),
        ("kirov", (1, 4), 1),
        ("rocketeer", (4, 4), 2),
        ("prism_tank", (6, 4), 2),
    ] {
        let kind = review.definitions.unit_kind(id).expect("HD unit");
        faction.roster.insert(sim::ProductionItem::Unit(kind));
        faction.units.push(sim::StartingGroup {
            kind,
            offset,
            count,
        });
    }
    (review, choice)
}

fn combat_review(original: &Catalogue) -> (Catalogue, MenuSelection) {
    let (mut review, choice) = technology_review(original);
    let enemy = &mut review.skirmish.factions[choice.opponent];
    enemy.units.clear();
    for (id, offset, count) in [("rhino", (-30, 0), 3), ("conscript", (-31, -2), 4)] {
        enemy.units.push(sim::StartingGroup {
            kind: review.definitions.unit_kind(id).expect("enemy"),
            offset,
            count,
        });
    }
    (review, choice)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_weapons_scene_is_a_valid_match() {
        let original = crate::skirmish::tests::production_catalogue();
        let (review, choice) = fx_review(&original);
        let (plan, _) = crate::skirmish::match_plan(&review, choice, 8).expect("plan");
        // As `begin_scene` starts it.
        let plan = crate::sim_host::MatchPlan {
            mcv_start: SCENE_MCV_START,
            ..plan
        };
        let checked = crate::sim_host::check_match(&plan);
        assert!(checked.is_ok(), "{:?}", checked.err());
    }

    #[test]
    fn technology_scene_uses_valid_real_match_setup() {
        let original = crate::skirmish::tests::production_catalogue();
        let (review, choice) = combat_review(&original);
        let plan = crate::sim_host::MatchPlan {
            super_me: false,
            unlock_all: false,
            unit_speed_percent: 100,
            mcv_start: SCENE_MCV_START,
            grid: review.maps[choice.map].map.grid.clone(),
            looks: review.maps[choice.map].map.looks.clone(),
            definitions: review.definitions,
            skirmish: review.skirmish,
            ai: review.ai,
            map: choice.map,
            players: choice.players().to_vec(),
            seed: 8,
        };
        assert!(crate::sim_host::check_match(&plan).is_ok());
    }
    #[test]
    fn roster_lineup_is_a_valid_match_with_every_type_once() {
        let original = crate::skirmish::tests::production_catalogue();
        let (review, choice) = roster_review(&original);
        let plan = crate::sim_host::MatchPlan {
            super_me: false,
            unlock_all: false,
            unit_speed_percent: 100,
            mcv_start: SCENE_MCV_START,
            grid: review.maps[choice.map].map.grid.clone(),
            looks: review.maps[choice.map].map.looks.clone(),
            definitions: review.definitions,
            skirmish: review.skirmish,
            ai: review.ai,
            map: choice.map,
            players: choice.players().to_vec(),
            seed: 8,
        };
        let start = crate::sim_host::check_match(&plan).expect("valid line-up");
        assert!(start.units >= ROSTER_LINEUP.len(), "{}", start.units);
        // Exactly the line-up besides the starting units of the two countries (RA1).
        let (starting, yards) =
            crate::skirmish::starting_totals(&original, choice, SCENE_MCV_START);
        assert_eq!(start.units, ROSTER_LINEUP.len() + starting);
        assert_eq!(start.buildings, 6 + yards);
    }
    #[test]
    fn developed_base_is_valid_and_does_not_extend_the_opponents_start() {
        let original = crate::skirmish::tests::production_catalogue();
        let (review, choice) = base_review(&original);
        assert_ne!(choice.faction, choice.opponent);
        assert_eq!(
            original.skirmish.factions[choice.opponent],
            review.skirmish.factions[choice.opponent]
        );
        let plan = crate::sim_host::MatchPlan {
            super_me: false,
            unlock_all: false,
            unit_speed_percent: 100,
            mcv_start: SCENE_MCV_START,
            grid: review.maps[choice.map].map.grid.clone(),
            looks: review.maps[choice.map].map.looks.clone(),
            definitions: review.definitions,
            skirmish: review.skirmish,
            ai: review.ai,
            map: choice.map,
            players: choice.players().to_vec(),
            seed: 8,
        };
        let start = crate::sim_host::check_match(&plan).expect("valid developed base");
        // The six buildings of the scene and the yard of either player, which a scene starts
        // with (`begin_scene`); the shipped countries have no starting building of their own.
        assert_eq!(start.buildings, 8);
        assert_eq!(
            original.skirmish.factions[choice.faction].buildings.len(),
            0
        );
    }
}
