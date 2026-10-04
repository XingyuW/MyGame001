use bevy_ecs::resource::Resource;
use bevy_ecs::system::ResMut;

/// Identifies a player (human or AI).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerId(pub u8);

/// An order issued by a player.
///
/// M0 defines no variants: the type is uninhabited, so no command can be constructed yet. The
/// variants listed in PLAN.md section 5 (`Move`, `Stop`, ...) are added by the milestones that
/// implement them, together with their validation in [`process_commands`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {}

/// A [`Command`] together with the player who issued it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssuedCommand {
    pub player: PlayerId,
    pub command: Command,
}

/// Pending commands, consumed in push order at the start of the next tick.
///
/// This is the only way for the interface, the AI, and tests to change simulation state
/// (invariant I5).
#[derive(Resource, Debug, Default)]
pub struct CommandQueue {
    pending: Vec<IssuedCommand>,
}

impl CommandQueue {
    pub fn push(&mut self, issued: IssuedCommand) {
        self.pending.push(issued);
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

/// Drains the queue in push order, validating and applying each command.
#[expect(
    clippy::never_loop,
    reason = "`Command` has no variants in M0; remove once the first variant is added"
)]
pub(crate) fn process_commands(mut queue: ResMut<CommandQueue>) {
    for issued in queue.pending.drain(..) {
        match issued.command {}
    }
}
