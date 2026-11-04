//! Enemy taunts: now and then the opposing command gloats over the radio, as a notice in the
//! column of `crate::notices`. It reacts only to what the human player already knows: its own
//! buildings destroyed, and a run of its own units lost. It never names a hidden enemy, a place
//! or a number, so it reveals nothing the player could not see (docs/DECISIONS.md D31).
//!
//! At most one taunt per [`TAUNT_COOLDOWN_SECS`] of real time; the lines of each kind are used in
//! turn, so the same one is not repeated back to back.

use bevy::prelude::*;
use sim::SimEvent;
use std::collections::VecDeque;

use crate::notices::{MAX_NOTICES, Notice, NoticeColumn, notice};
use crate::units::HUMAN;

/// Real seconds between two taunts at least.
pub const TAUNT_COOLDOWN_SECS: f64 = 25.0;
/// Units the player must lose within [`LOSS_WINDOW_SECS`] to be taunted for it.
pub const LOSS_RUN: usize = 5;
pub const LOSS_WINDOW_SECS: f64 = 20.0;

/// Said when one of the player's buildings is destroyed.
pub const BUILDING_LINES: [&str; 6] = [
    "Enemy command: Your base is crumbling, Commander. Surrender while you can.",
    "Enemy command: Another building falls. How predictable.",
    "Enemy command: Our engineers will enjoy the scrap.",
    "Enemy command: Was that meant to be a defence?",
    "Enemy command: We will plant our flag on your ruins.",
    "Enemy command: Your factories make excellent target practice.",
];

/// Said when the player loses a run of units.
pub const LOSS_LINES: [&str; 5] = [
    "Enemy command: Your soldiers fight bravely. It does not help them.",
    "Enemy command: Send more. We have plenty of ammunition.",
    "Enemy command: Is this the best your academy can produce?",
    "Enemy command: Your army melts like snow in spring.",
    "Enemy command: Retreat now and we may let you live.",
];

/// The colour of the enemy's radio voice.
const TAUNT_COLOUR: Color = Color::srgb(0.95, 0.42, 0.32);

pub struct TauntsPlugin;

impl Plugin for TauntsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Taunts>()
            .add_systems(Update, taunt.after(crate::skirmish::update_screens));
    }
}

/// What was said and when.
#[derive(Resource, Debug, Default)]
pub struct Taunts {
    last: Option<f64>,
    losses: VecDeque<f64>,
    building_line: usize,
    loss_line: usize,
}

/// What prompts a taunt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prompt {
    BuildingLost,
    UnitLost,
}

impl Taunts {
    /// Records `prompt` at real time `now` and returns the line to say, if any.
    pub fn hear(&mut self, prompt: Prompt, now: f64) -> Option<&'static str> {
        let line = match prompt {
            Prompt::BuildingLost => {
                let line = BUILDING_LINES[self.building_line % BUILDING_LINES.len()];
                Some((line, true))
            }
            Prompt::UnitLost => {
                self.losses.push_back(now);
                while self
                    .losses
                    .front()
                    .is_some_and(|&t| now - t > LOSS_WINDOW_SECS)
                {
                    self.losses.pop_front();
                }
                (self.losses.len() >= LOSS_RUN)
                    .then(|| (LOSS_LINES[self.loss_line % LOSS_LINES.len()], false))
            }
        }?;
        if self
            .last
            .is_some_and(|last| now - last < TAUNT_COOLDOWN_SECS)
        {
            return None;
        }
        self.last = Some(now);
        if line.1 {
            self.building_line += 1;
        } else {
            self.loss_line += 1;
            self.losses.clear();
        }
        Some(line.0)
    }
}

fn taunt(
    mut commands: Commands,
    mut events: MessageReader<SimEvent>,
    mut taunts: ResMut<Taunts>,
    time: Res<Time<Real>>,
    column: Option<Single<Entity, With<NoticeColumn>>>,
    notices: Query<(Entity, &Notice)>,
    mut voices: ResMut<crate::voices::VoiceRequests>,
) {
    let now = time.elapsed_secs_f64();
    let mut said = None;
    for event in events.read() {
        let prompt = match event {
            SimEvent::BuildingDestroyed { player, .. } if *player == HUMAN => Prompt::BuildingLost,
            SimEvent::UnitDestroyed { player, .. } if *player == HUMAN => Prompt::UnitLost,
            _ => continue,
        };
        if let Some(line) = taunts.hear(prompt, now) {
            said = Some(line);
        }
    }
    let Some(line) = said else {
        return;
    };
    // The enemy says it over the radio too.
    voices.0.extend(crate::voices::taunt_file(line));
    let Some(column) = column else {
        return;
    };
    if notices.iter().count() >= MAX_NOTICES
        && let Some((oldest, _)) = notices.iter().min_by(|a, b| a.1.left.total_cmp(&b.1.left))
    {
        commands.entity(oldest).despawn();
    }
    commands
        .entity(*column)
        .with_child(notice(line.to_owned(), TAUNT_COLOUR));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lost_building_is_taunted_at_most_once_per_cooldown_and_lines_rotate() {
        let mut taunts = Taunts::default();
        assert_eq!(
            taunts.hear(Prompt::BuildingLost, 0.0),
            Some(BUILDING_LINES[0])
        );
        assert_eq!(taunts.hear(Prompt::BuildingLost, 5.0), None);
        assert_eq!(
            taunts.hear(Prompt::BuildingLost, TAUNT_COOLDOWN_SECS + 1.0),
            Some(BUILDING_LINES[1])
        );
    }

    #[test]
    fn only_a_run_of_unit_losses_is_taunted() {
        let mut taunts = Taunts::default();
        // Losses spread out further than the window never add up to a run.
        for i in 0..LOSS_RUN * 2 {
            assert_eq!(
                taunts.hear(Prompt::UnitLost, i as f64 * (LOSS_WINDOW_SECS + 1.0)),
                None
            );
        }
        let start = 1_000.0;
        for i in 0..LOSS_RUN - 1 {
            assert_eq!(taunts.hear(Prompt::UnitLost, start + i as f64), None);
        }
        assert_eq!(
            taunts.hear(Prompt::UnitLost, start + LOSS_RUN as f64),
            Some(LOSS_LINES[0])
        );
    }

    #[test]
    fn every_line_speaks_for_the_enemy() {
        for line in BUILDING_LINES.iter().chain(&LOSS_LINES) {
            assert!(line.starts_with("Enemy command: "), "{line}");
            assert!(line.len() < 80, "{line} is too long for the notice column");
        }
    }
}
