//! Match notices: short lines in the top left corner of the map that report what happened
//! elsewhere on it. A player's defeat (`SimEvent::PlayerDefeated`): "Red Player (Iraq) has been
//! defeated", written in that player's team colour and with the country it plays (RA1), so that
//! in a match against several computer players the human player learns which of them is out
//! without having to watch its base fall. And, when a match starts, the country the human player
//! plays ([`announce`], from `crate::skirmish::begin_match`), which is news after a RANDOM
//! choice.
//!
//! Since RA4 also what happens to the human player's own units away from the camera: a unit that
//! rose in rank ("Grizzly Battle Tank promoted to veteran", [`promotion_text`]) and the crew or
//! the survivors that left a destroyed vehicle or a destroyed or sold building
//! ([`crew_text`]), both in the player's colour.
//!
//! A notice stays for [`NOTICE_SECONDS`] of real time (also while the match is paused, when no
//! new one can arrive) and fades out over its last second. The newest is at the bottom of the
//! column; when a match ends, its notices go with it (`crate::skirmish::end_match`).

use bevy::prelude::*;
use sim::{PlayerId, SimEvent};

use crate::presentation::{TEAM_PALETTE, TeamColors};
use crate::units::HUMAN;

/// Seconds a notice is shown.
pub const NOTICE_SECONDS: f32 = 9.0;

/// Seconds over which a notice fades out at the end of its time.
pub const FADE_SECONDS: f32 = 1.2;

/// More notices than this at once push the oldest out (a match has at most ten players, nine of
/// whom can be reported defeated while it runs).
pub const MAX_NOTICES: usize = 6;

pub struct NoticesPlugin;

impl Plugin for NoticesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            spawn_notice_column.after(crate::skirmish::load_catalogue),
        )
        .add_systems(
            Update,
            (announce_defeats, fade_notices)
                .chain()
                .after(crate::skirmish::update_screens),
        );
    }
}

/// The column the notices are listed in.
#[derive(Component, Debug)]
pub struct NoticeColumn;

/// A notice: the seconds it has left and its colour at full strength.
#[derive(Component, Debug)]
pub struct Notice {
    pub left: f32,
    pub colour: Color,
}

/// The line that reports the defeat of `player`, whose team colour is called `colour` and who
/// plays `country` (`None` when the match does not say: "Red Player has been defeated").
pub fn defeat_text(player: PlayerId, colour: &str, country: Option<&str>) -> String {
    if player == HUMAN {
        "You have been defeated".to_owned()
    } else {
        format!(
            "{} has been defeated",
            crate::skirmish::player_name(colour, country)
        )
    }
}

/// The line that reports that a unit of the human player called `label` rose to `rank` (RA4).
pub fn promotion_text(label: &str, rank: u8) -> String {
    let rank = crate::hover_info::rank_name(rank)
        .unwrap_or("Veteran")
        .to_lowercase();
    format!("{label} promoted to {rank}")
}

/// The line that reports that `count` crew members or survivors of the human player left a
/// wreck (RA4).
pub fn crew_text(count: usize) -> String {
    match count {
        1 => "A survivor escaped".to_owned(),
        count => format!("{count} survivors escaped"),
    }
}

/// The line that reports what a crate held for the human player (RA5): `credits` for money.
pub fn crate_text(content: sim::CrateContent, credits: i64) -> String {
    match content {
        sim::CrateContent::Money => format!("Crate: {credits} credits"),
        sim::CrateContent::Unit => "Crate: a free unit".to_owned(),
        sim::CrateContent::Veteran => "Crate: units promoted".to_owned(),
        sim::CrateContent::HealBase => "Crate: everything repaired".to_owned(),
        sim::CrateContent::Armour => "Crate: armour upgraded".to_owned(),
        sim::CrateContent::Firepower => "Crate: firepower upgraded".to_owned(),
        sim::CrateContent::Speed => "Crate: speed upgraded".to_owned(),
        sim::CrateContent::Reveal => "Crate: map revealed".to_owned(),
    }
}

/// How opaque a notice with `left` seconds to go is: full, then fading to nothing.
pub fn notice_alpha(left: f32) -> f32 {
    (left / FADE_SECONDS).clamp(0.0, 1.0)
}

fn spawn_notice_column(mut commands: Commands) {
    commands.spawn((
        NoticeColumn,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            top: Val::Px(12.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexStart,
            row_gap: Val::Px(6.0),
            ..default()
        },
        GlobalZIndex(4),
        bevy::ui::FocusPolicy::Pass,
        Pickable::IGNORE,
    ));
}

/// The background of a notice at opacity `alpha`.
fn panel_colour(alpha: f32) -> Color {
    Color::srgba(0.03, 0.04, 0.05, 0.84 * alpha)
}

/// A notice with `text` in `colour`.
fn notice(text: String, colour: Color) -> impl Bundle {
    (
        Notice {
            left: NOTICE_SECONDS,
            colour,
        },
        Node {
            padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
            border: UiRect::left(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(panel_colour(1.0)),
        BorderColor::all(colour),
        Text::new(text),
        TextFont::from_font_size(17.0),
        TextColor(colour),
        bevy::ui::FocusPolicy::Pass,
        Pickable::IGNORE,
    )
}

/// Adds the notice `text` in `colour` to the column, if the game has one (the tests of the game
/// flow run without the interface).
pub fn announce(world: &mut World, text: String, colour: Color) {
    let Some(column) = world
        .query_filtered::<Entity, With<NoticeColumn>>()
        .iter(world)
        .next()
    else {
        return;
    };
    world.entity_mut(column).with_child(notice(text, colour));
}

/// The label of the type of the unit `unit`, if it still exists.
fn unit_label(host: &crate::sim_host::SimHost, unit: sim::SimId) -> Option<String> {
    let definitions = host.definitions()?;
    let mut units = host.world().try_query::<(&sim::SimId, &sim::Unit)>()?;
    let kind = units
        .iter(host.world())
        .find(|(id, _)| **id == unit)
        .map(|(_, unit)| unit.kind)?;
    definitions
        .metadata(sim::ProductionItem::Unit(kind))
        .map(|entry| entry.label.clone())
}

/// Adds a notice for every player defeated in the ticks of this frame, and for every promotion
/// and escaped crew of the human player (RA4).
fn announce_defeats(
    mut commands: Commands,
    mut events: MessageReader<SimEvent>,
    teams: Option<Res<TeamColors>>,
    played: Option<Res<crate::skirmish::MatchPlayers>>,
    host: Option<Res<crate::sim_host::SimHost>>,
    column: Single<Entity, With<NoticeColumn>>,
    notices: Query<(Entity, &Notice)>,
) {
    let mut shown = notices.iter().count();
    for event in events.read() {
        let (player, text) = match event {
            SimEvent::PlayerDefeated { player } => (*player, None),
            SimEvent::RankGained { player, unit, rank } if *player == HUMAN => {
                let label = host
                    .as_deref()
                    .and_then(|host| unit_label(host, *unit))
                    .unwrap_or_else(|| "Unit".to_owned());
                (*player, Some(promotion_text(&label, *rank)))
            }
            SimEvent::CrewEscaped { player, units, .. } if *player == HUMAN => {
                (*player, Some(crew_text(units.len())))
            }
            SimEvent::CrateTaken {
                player,
                content,
                credits,
                ..
            } if *player == HUMAN => (*player, Some(crate_text(*content, *credits))),
            _ => continue,
        };
        let player = &player;
        let teams = teams.as_deref().copied().unwrap_or_default();
        let (name, [r, g, b]) = TEAM_PALETTE[teams.index_of(*player)];
        let colour = Color::srgb_u8(r, g, b);
        // The oldest makes room: it has the least time left.
        if shown >= MAX_NOTICES
            && let Some((oldest, _)) = notices.iter().min_by(|a, b| a.1.left.total_cmp(&b.1.left))
        {
            commands.entity(oldest).despawn();
            shown -= 1;
        }
        shown += 1;
        let text = text.unwrap_or_else(|| {
            let country = played.as_deref().and_then(|played| played.label(*player));
            defeat_text(*player, name, country)
        });
        commands.entity(*column).with_child(notice(text, colour));
    }
}

/// Counts every notice down, fades it out, and removes it when its time is up.
fn fade_notices(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut notices: Query<(
        Entity,
        &mut Notice,
        &mut TextColor,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
) {
    for (entity, mut notice, mut text, mut background, mut border) in &mut notices {
        notice.left -= time.delta_secs();
        if notice.left <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let alpha = notice_alpha(notice.left);
        if alpha < 1.0 {
            let colour = notice.colour.with_alpha(alpha);
            text.0 = colour;
            background.0 = panel_colour(alpha);
            *border = BorderColor::all(colour);
        }
    }
}

/// Removes every notice: they belong to the match that ends.
pub fn clear(world: &mut World) {
    let notices: Vec<Entity> = world
        .query_filtered::<Entity, With<Notice>>()
        .iter(world)
        .collect();
    for notice in notices {
        world.despawn(notice);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RA4: the lines of a promotion and of an escaped crew.
    #[test]
    fn a_promotion_and_a_crew_are_reported_in_plain_words() {
        assert_eq!(
            promotion_text("Grizzly Battle Tank", sim::VETERAN),
            "Grizzly Battle Tank promoted to veteran"
        );
        assert_eq!(promotion_text("GI", sim::ELITE), "GI promoted to elite");
        assert_eq!(crew_text(1), "A survivor escaped");
        assert_eq!(crew_text(6), "6 survivors escaped");
        // RA5: every content of a crate has a line of its own, and money names its credits.
        assert_eq!(
            crate_text(sim::CrateContent::Money, 2450),
            "Crate: 2450 credits"
        );
        let lines: std::collections::BTreeSet<String> = sim::CrateContent::ALL
            .into_iter()
            .map(|content| crate_text(content, 0))
            .collect();
        assert_eq!(lines.len(), sim::CrateContent::ALL.len());
    }

    #[test]
    fn a_defeat_is_reported_in_the_colour_name_of_the_defeated_player() {
        assert_eq!(
            defeat_text(PlayerId(1), "Red", None),
            "Red Player has been defeated"
        );
        assert_eq!(
            defeat_text(PlayerId(4), "Purple", None),
            "Purple Player has been defeated"
        );
        assert_eq!(defeat_text(HUMAN, "Blue", None), "You have been defeated");
        // RA1: with the country the player plays.
        assert_eq!(
            defeat_text(PlayerId(1), "Blue", Some("France")),
            "Blue Player (France) has been defeated"
        );
        assert_eq!(
            defeat_text(HUMAN, "Blue", Some("America")),
            "You have been defeated"
        );
    }

    #[test]
    fn a_notice_is_solid_until_its_last_moments() {
        assert_eq!(notice_alpha(NOTICE_SECONDS), 1.0);
        assert_eq!(notice_alpha(FADE_SECONDS), 1.0);
        assert_eq!(notice_alpha(FADE_SECONDS / 2.0), 0.5);
        assert_eq!(notice_alpha(0.0), 0.0);
        assert_eq!(notice_alpha(-1.0), 0.0);
    }

    /// The whole path through Bevy: a `PlayerDefeated` message becomes a line in the column, in
    /// the defeated player's colour, and is gone once its time is up.
    #[test]
    fn a_defeated_player_is_announced_in_its_team_colour_and_the_notice_expires() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<SimEvent>()
            // The human player chose Green (3): computer player 2 is Cyan (5).
            .insert_resource(TeamColors(3))
            .add_systems(Startup, spawn_notice_column)
            .add_systems(Update, (announce_defeats, fade_notices).chain());
        app.update();
        app.world_mut().write_message(SimEvent::PlayerDefeated {
            player: PlayerId(2),
        });
        app.world_mut().write_message(SimEvent::UnitDestroyed {
            player: PlayerId(2),
            unit: sim::SimId(7),
        });
        app.update();
        let world = app.world_mut();
        let lines: Vec<(String, Color, Entity)> = world
            .query::<(&Text, &TextColor, &ChildOf, &Notice)>()
            .iter(world)
            .map(|(text, colour, parent, _)| (text.0.clone(), colour.0, parent.parent()))
            .collect();
        let column = world
            .query_filtered::<Entity, With<NoticeColumn>>()
            .single(world)
            .expect("one column");
        assert_eq!(
            lines,
            [(
                "Cyan Player has been defeated".to_owned(),
                Color::srgb_u8(45, 185, 191),
                column
            )]
        );
        // Its time runs out.
        let mut notices = world.query::<&mut Notice>();
        for mut notice in notices.iter_mut(world) {
            notice.left = 0.0;
        }
        app.update();
        let world = app.world_mut();
        assert_eq!(world.query::<&Notice>().iter(world).count(), 0);
    }

    /// RA4, the whole path through Bevy: a promotion and an escaped crew of the human player
    /// become lines in the player's colour; those of other players do not.
    #[test]
    fn only_the_human_players_promotions_and_crews_are_announced() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<SimEvent>()
            .insert_resource(TeamColors(3))
            .add_systems(Startup, spawn_notice_column)
            .add_systems(Update, announce_defeats);
        app.update();
        let enemy = PlayerId(1);
        for player in [HUMAN, enemy] {
            app.world_mut().write_message(SimEvent::RankGained {
                player,
                unit: sim::SimId(3),
                rank: sim::ELITE,
            });
            app.world_mut().write_message(SimEvent::CrewEscaped {
                player,
                from: sim::SimId(4),
                at: sim::CellPos::new(1, 1),
                units: vec![sim::SimId(5), sim::SimId(6)],
            });
            // RA5: a crate, announced to the player who took it alone.
            app.world_mut().write_message(SimEvent::CrateTaken {
                player,
                unit: sim::SimId(3),
                cell: sim::CellPos::new(2, 2),
                content: sim::CrateContent::Money,
                credits: 2100,
                spawned: None,
            });
        }
        app.update();
        let world = app.world_mut();
        let lines: Vec<(String, Color)> = world
            .query::<(&Text, &TextColor, &Notice)>()
            .iter(world)
            .map(|(text, colour, _)| (text.0.clone(), colour.0))
            .collect();
        // Without a simulation to name the unit's type it is a "Unit".
        let texts: Vec<&str> = lines.iter().map(|(text, _)| text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "Unit promoted to elite",
                "2 survivors escaped",
                "Crate: 2100 credits"
            ]
        );
        let (_, [r, g, b]) = TEAM_PALETTE[TeamColors(3).index_of(HUMAN)];
        assert!(
            lines
                .iter()
                .all(|(_, colour)| *colour == Color::srgb_u8(r, g, b))
        );
    }

    #[test]
    fn the_oldest_notice_makes_room_and_a_match_end_clears_them() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<SimEvent>()
            .add_systems(Startup, spawn_notice_column)
            .add_systems(Update, announce_defeats);
        app.update();
        for player in 1..=7 {
            app.world_mut().write_message(SimEvent::PlayerDefeated {
                player: PlayerId(player),
            });
            app.update();
            // Age them apart, as frames would.
            let world = app.world_mut();
            let mut notices = world.query::<&mut Notice>();
            for mut notice in notices.iter_mut(world) {
                notice.left -= 0.5;
            }
        }
        let world = app.world_mut();
        let mut texts: Vec<String> = world
            .query_filtered::<&Text, With<Notice>>()
            .iter(world)
            .map(|text| text.0.clone())
            .collect();
        texts.sort();
        assert_eq!(texts.len(), MAX_NOTICES);
        assert!(
            !texts.contains(&"Red Player has been defeated".to_owned()),
            "the first is gone"
        );
        clear(world);
        assert_eq!(world.query::<&Notice>().iter(world).count(), 0);
    }

    /// RA1: the notice names the country the defeated player plays, and the start of a match
    /// is announced the same way.
    #[test]
    fn notices_name_the_country_a_player_plays() {
        let entry = |player: u8, label: &str| crate::skirmish::MatchPlayer {
            player: PlayerId(player),
            faction: usize::from(player),
            country: format!("{label} (Soviet)"),
            label: label.to_owned(),
            side: Some(sim::Side::Soviet),
            difficulty: (player != 0).then(|| "Medium".to_owned()),
        };
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<SimEvent>()
            .insert_resource(TeamColors(0))
            .insert_resource(crate::skirmish::MatchPlayers(vec![
                entry(0, "Cuba"),
                entry(1, "Iraq"),
            ]))
            .add_systems(Startup, spawn_notice_column)
            .add_systems(Update, announce_defeats);
        // Without a column (the tests of the game flow) an announcement is dropped.
        announce(
            app.world_mut(),
            "nobody reads this".to_owned(),
            Color::WHITE,
        );
        app.update();
        let blue = Color::srgb_u8(42, 133, 202);
        announce(
            app.world_mut(),
            crate::skirmish::start_text("Cuba (Soviet)"),
            blue,
        );
        for player in [1, 2] {
            app.world_mut().write_message(SimEvent::PlayerDefeated {
                player: PlayerId(player),
            });
        }
        app.update();
        let world = app.world_mut();
        let lines: Vec<(String, Color)> = world
            .query::<(&Text, &TextColor, &Notice)>()
            .iter(world)
            .map(|(text, colour, _)| (text.0.clone(), colour.0))
            .collect();
        assert_eq!(
            lines,
            [
                ("You play Cuba (Soviet)".to_owned(), blue),
                (
                    "Red Player (Iraq) has been defeated".to_owned(),
                    Color::srgb_u8(195, 66, 48)
                ),
                // A player the match does not list is named by its colour alone.
                (
                    "Gold Player has been defeated".to_owned(),
                    Color::srgb_u8(212, 175, 52)
                ),
            ]
        );
    }
}
