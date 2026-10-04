//! Hotkeys and control groups (PLAN.md section 7, M5; docs/DECISIONS.md D35).
//!
//! Every key the interface reacts to, besides the arrow keys of the camera (D10), is listed in
//! [`hotkey`], so that no key has two meanings. The systems that act on keys ask this function
//! and handle the actions that concern them. Control groups are pure functions over the
//! selection, applied by `crate::selection`.
//!
//! | Key | Action |
//! |-----|--------|
//! | `S` | stop the selection |
//! | `A` | attack-move the selection to the cell under the cursor |
//! | `B` | placement mode for the ready building |
//! | `X` | sell mode |
//! | `R` | rally point of the player's unit-producing buildings at the cell under the cursor |
//! | `Delete`, `Backspace` | sell the player's building under the cursor |
//! | `Escape` | leave the current mode |
//! | `P`, `Pause` | pause or resume |
//! | `Tab` | next production tab |
//! | `0` to `9` | select control group n |
//! | Shift + `0` to `9` | add control group n to the selection |
//! | Ctrl or Cmd + `0` to `9` | make the selection control group n |
//! | `-`, `=` | master volume down, up |
//! | `M` | mute or unmute |

use std::collections::BTreeSet;

use bevy::prelude::*;
use sim::SimId;

/// Number of control groups (keys `0` to `9`).
pub const CONTROL_GROUPS: usize = 10;

/// What a key press asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hotkey {
    Stop,
    AttackMove,
    PlaceReady,
    SellMode,
    RallyAtCursor,
    SellAtCursor,
    Cancel,
    Pause,
    NextTab,
    SelectGroup(u8),
    AddGroup(u8),
    AssignGroup(u8),
    VolumeDown,
    VolumeUp,
    Mute,
}

/// The digit of a digit key, `0` to `9`.
fn digit(key: KeyCode) -> Option<u8> {
    Some(match key {
        KeyCode::Digit0 => 0,
        KeyCode::Digit1 => 1,
        KeyCode::Digit2 => 2,
        KeyCode::Digit3 => 3,
        KeyCode::Digit4 => 4,
        KeyCode::Digit5 => 5,
        KeyCode::Digit6 => 6,
        KeyCode::Digit7 => 7,
        KeyCode::Digit8 => 8,
        KeyCode::Digit9 => 9,
        _ => return None,
    })
}

/// The action of `key` pressed with Ctrl or Cmd held (`command`) and with Shift held (`shift`).
pub fn hotkey(key: KeyCode, command: bool, shift: bool) -> Option<Hotkey> {
    if let Some(group) = digit(key) {
        return Some(if command {
            Hotkey::AssignGroup(group)
        } else if shift {
            Hotkey::AddGroup(group)
        } else {
            Hotkey::SelectGroup(group)
        });
    }
    if command {
        // Ctrl or Cmd with a letter is left to the operating system (Cmd+Q quits on macOS).
        return None;
    }
    Some(match key {
        KeyCode::KeyS => Hotkey::Stop,
        KeyCode::KeyA => Hotkey::AttackMove,
        KeyCode::KeyB => Hotkey::PlaceReady,
        KeyCode::KeyX => Hotkey::SellMode,
        KeyCode::KeyR => Hotkey::RallyAtCursor,
        KeyCode::Delete | KeyCode::Backspace => Hotkey::SellAtCursor,
        KeyCode::Escape => Hotkey::Cancel,
        KeyCode::KeyP | KeyCode::Pause => Hotkey::Pause,
        KeyCode::Tab => Hotkey::NextTab,
        KeyCode::Minus => Hotkey::VolumeDown,
        KeyCode::Equal => Hotkey::VolumeUp,
        KeyCode::KeyM => Hotkey::Mute,
        _ => return None,
    })
}

/// The hotkeys pressed in this frame.
pub fn pressed_hotkeys(keys: &ButtonInput<KeyCode>) -> Vec<Hotkey> {
    let command = keys.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]);
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    keys.get_just_pressed()
        .filter_map(|&key| hotkey(key, command, shift))
        .collect()
}

/// The player's control groups.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct ControlGroups(pub [BTreeSet<SimId>; CONTROL_GROUPS]);

impl ControlGroups {
    /// The selection after `action` (a control group key), and the groups after it: assigning
    /// replaces group n with the selection (an empty selection empties the group); selecting
    /// replaces the selection with group n; adding unites them. Other actions change nothing.
    pub fn apply(&mut self, action: Hotkey, selection: &BTreeSet<SimId>) -> BTreeSet<SimId> {
        let group = |n: u8| usize::from(n).min(CONTROL_GROUPS - 1);
        match action {
            Hotkey::AssignGroup(n) => {
                self.0[group(n)] = selection.clone();
                selection.clone()
            }
            Hotkey::SelectGroup(n) => self.0[group(n)].clone(),
            Hotkey::AddGroup(n) => selection.union(&self.0[group(n)]).copied().collect(),
            _ => selection.clone(),
        }
    }

    /// Removes units that no longer exist (`alive` says whether a unit does).
    pub fn prune(&mut self, alive: impl Fn(SimId) -> bool) {
        for group in &mut self.0 {
            group.retain(|&id| alive(id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(values: &[u32]) -> BTreeSet<SimId> {
        values.iter().map(|&id| SimId(id)).collect()
    }

    #[test]
    fn every_listed_key_has_one_action() {
        assert_eq!(hotkey(KeyCode::KeyS, false, false), Some(Hotkey::Stop));
        assert_eq!(
            hotkey(KeyCode::KeyA, false, false),
            Some(Hotkey::AttackMove)
        );
        assert_eq!(
            hotkey(KeyCode::KeyB, false, false),
            Some(Hotkey::PlaceReady)
        );
        assert_eq!(hotkey(KeyCode::KeyX, false, false), Some(Hotkey::SellMode));
        assert_eq!(
            hotkey(KeyCode::KeyR, false, false),
            Some(Hotkey::RallyAtCursor)
        );
        for key in [KeyCode::Delete, KeyCode::Backspace] {
            assert_eq!(hotkey(key, false, false), Some(Hotkey::SellAtCursor));
        }
        assert_eq!(hotkey(KeyCode::Escape, false, false), Some(Hotkey::Cancel));
        for key in [KeyCode::KeyP, KeyCode::Pause] {
            assert_eq!(hotkey(key, false, false), Some(Hotkey::Pause));
        }
        assert_eq!(hotkey(KeyCode::Tab, false, false), Some(Hotkey::NextTab));
        assert_eq!(
            hotkey(KeyCode::Minus, false, false),
            Some(Hotkey::VolumeDown)
        );
        assert_eq!(hotkey(KeyCode::Equal, false, false), Some(Hotkey::VolumeUp));
        assert_eq!(hotkey(KeyCode::KeyM, false, false), Some(Hotkey::Mute));
        // Shift does not change letter keys; the camera's arrow keys and unlisted keys are not
        // hotkeys; Ctrl or Cmd with a letter is left to the system.
        assert_eq!(hotkey(KeyCode::KeyS, false, true), Some(Hotkey::Stop));
        for key in [
            KeyCode::ArrowLeft,
            KeyCode::ArrowUp,
            KeyCode::KeyQ,
            KeyCode::KeyW,
            KeyCode::Space,
        ] {
            assert_eq!(hotkey(key, false, false), None, "{key:?}");
        }
        assert_eq!(hotkey(KeyCode::KeyS, true, false), None);
        assert_eq!(hotkey(KeyCode::KeyQ, true, false), None);
    }

    #[test]
    fn digit_keys_select_add_and_assign_control_groups() {
        assert_eq!(
            hotkey(KeyCode::Digit0, false, false),
            Some(Hotkey::SelectGroup(0))
        );
        assert_eq!(
            hotkey(KeyCode::Digit9, false, true),
            Some(Hotkey::AddGroup(9))
        );
        assert_eq!(
            hotkey(KeyCode::Digit5, true, false),
            Some(Hotkey::AssignGroup(5))
        );
        // Ctrl or Cmd wins over Shift.
        assert_eq!(
            hotkey(KeyCode::Digit5, true, true),
            Some(Hotkey::AssignGroup(5))
        );
    }

    #[test]
    fn control_groups_assign_select_add_and_forget_dead_units() {
        let mut groups = ControlGroups::default();
        let selection = ids(&[1, 2]);
        assert_eq!(groups.apply(Hotkey::AssignGroup(1), &selection), selection);
        assert_eq!(groups.0[1], selection);
        // Selecting a group replaces the selection; an empty group empties it.
        assert_eq!(groups.apply(Hotkey::SelectGroup(1), &ids(&[7])), selection);
        assert_eq!(groups.apply(Hotkey::SelectGroup(0), &ids(&[7])), ids(&[]));
        // Adding unites the group with the selection.
        assert_eq!(
            groups.apply(Hotkey::AddGroup(1), &ids(&[2, 7])),
            ids(&[1, 2, 7])
        );
        // Assigning an empty selection empties the group; groups 0 and 9 are the boundaries.
        assert_eq!(groups.apply(Hotkey::AssignGroup(9), &ids(&[3])), ids(&[3]));
        assert_eq!(groups.apply(Hotkey::AssignGroup(1), &ids(&[])), ids(&[]));
        assert!(groups.0[1].is_empty());
        assert_eq!(groups.0[9], ids(&[3]));
        // Other actions change nothing.
        assert_eq!(groups.apply(Hotkey::Stop, &ids(&[4])), ids(&[4]));
        // Destroyed units leave every group.
        groups.apply(Hotkey::AssignGroup(2), &ids(&[3, 4]));
        groups.prune(|id| id != SimId(3));
        assert_eq!(groups.0[9], ids(&[]));
        assert_eq!(groups.0[2], ids(&[4]));
    }
}
