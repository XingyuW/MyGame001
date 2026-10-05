//! Opt-in renderer capture for art review. MYGAME_VISUAL_REVIEW names an existing output
//! directory. This is separate from smoke tests and ordinary play; screenshots include a
//! labelled developed-base setup, installed through the same pre-tick match setup as skirmish.
//! Never changes a running simulation. The production catalogue is restored after setup.
use crate::skirmish::{Catalogue, MenuSelection, begin_match};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use std::path::PathBuf;

#[derive(Component)]
struct ReviewLabel;

#[derive(Resource)]
struct Review {
    directory: PathBuf,
    frame: u32,
}

pub fn enable(app: &mut App, directory: PathBuf) {
    app.insert_resource(Review {
        directory,
        frame: 0,
    })
    .add_systems(Last, capture);
}
fn capture(world: &mut World) {
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
        begin_match(world, choice, 8);
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
        let mut choice = MenuSelection::initial(world.resource::<Catalogue>());
        choice.map = if frame == 240 { 1 } else { 2 };
        choice.faction = if frame == 240 { 1 } else { 0 };
        choice.opponent = 1 - choice.faction;
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
        world.write_message(AppExit::Success);
    }
}

fn base_review(original: &Catalogue) -> (Catalogue, MenuSelection) {
    let mut review = original.clone();
    let mut choice = MenuSelection::initial(&review);
    choice.opponent = 1;
    for (id, offset) in [
        ("power_plant", (4, -2)),
        ("refinery", (-3, 3)),
        ("barracks", (4, 3)),
        ("war_factory", (0, 7)),
        ("tech_center", (-3, -1)),
        ("gun_turret", (8, 0)),
    ] {
        if let Some(kind) = review.definitions.building_kind(id) {
            review.skirmish.factions[choice.faction]
                .buildings
                .push(sim::StartingBuilding { kind, offset });
        }
    }
    (review, choice)
}

#[cfg(test)]
mod tests {
    use super::*;
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
            grid: review.maps[choice.map].map.grid.clone(),
            definitions: review.definitions,
            skirmish: review.skirmish,
            ai: review.ai,
            map: choice.map,
            players: choice.players().to_vec(),
            seed: 8,
        };
        let start = crate::sim_host::check_match(&plan).expect("valid developed base");
        assert_eq!(start.buildings, 8);
        assert_eq!(
            original.skirmish.factions[choice.faction].buildings.len(),
            1
        );
    }
}
