//! Voices: an announcer for the player's base, radio replies from the player's units, and the
//! enemy command's taunts (feel sprint 3, docs/DECISIONS.md).
//!
//! The lines are recordings written for this game and spoken by the Apache-2.0 Kokoro-82M speech
//! model (`scripts/generate_voices.py`, `assets/LICENSES.md`). They are compiled into the binary
//! with `include_bytes!`, so the game reads no audio file at run time, and decoded by
//! [`parse_wav`], which accepts exactly the files the generator writes (mono 16-bit PCM WAV at
//! [`SAMPLE_RATE_HZ`], a 44-byte header, at most [`MAX_VOICE_SECS`]); no third-party decoder
//! is involved (as D38 item 2). They play through [`Pcm`] like every other sound.
//!
//! One voice speaks at a time. Announcements and taunts wait their turn (at most
//! [`MAX_QUEUED`] of them); a unit's reply is dropped while one of those speaks and cuts off an
//! earlier reply. Everything the announcer says is about the human player's own side, or public
//! (a superweapon built or launched, which the effects already make heard), so a voice reveals
//! nothing the fog hides (D31).

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::Duration;

use bevy::audio::{Sample, Volume};
use bevy::prelude::*;
use sim::{Command, Domain, ProductionItem, SimEvent, SimId};

use crate::audio::{Channel, Pcm, SAMPLE_RATE_HZ, VolumeSettings};
use crate::buildings::{BuildingView, BuildingViews};
use crate::selection::Selection;
use crate::sim_host::{CommandSink, SimHost};
use crate::units::{HUMAN, UnitView, UnitViews};

/// Longest voice line, in seconds.
pub const MAX_VOICE_SECS: f32 = 4.0;
/// Announcements and taunts that may wait for the voice playing.
pub const MAX_QUEUED: usize = 3;
/// Real seconds between two replies of the player's units at least.
pub const REPLY_GAP_SECS: f64 = 0.5;

macro_rules! voice_files {
    ($($name:literal),* $(,)?) => {
        /// Every voice line: its name and its WAV file.
        pub const FILES: &[(&str, &[u8])] = &[$(
            ($name, include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"), "/../../assets/audio/voices/", $name, ".wav"
            ))),
        )*];
    };
}

voice_files!(
    "announcer_match_start",
    "announcer_structure_ready",
    "announcer_unit_ready",
    "announcer_low_power",
    "announcer_no_credits",
    "announcer_base_attacked",
    "announcer_forces_attacked",
    "announcer_structure_lost",
    "announcer_promotion",
    "announcer_superweapon_ready",
    "announcer_enemy_superweapon",
    "announcer_enemy_launch",
    "announcer_victory",
    "announcer_defeat",
    "infantry_select_1",
    "infantry_select_2",
    "infantry_select_3",
    "infantry_move_1",
    "infantry_move_2",
    "infantry_move_3",
    "infantry_attack_1",
    "infantry_attack_2",
    "infantry_attack_3",
    "vehicle_select_1",
    "vehicle_select_2",
    "vehicle_select_3",
    "vehicle_move_1",
    "vehicle_move_2",
    "vehicle_move_3",
    "vehicle_attack_1",
    "vehicle_attack_2",
    "vehicle_attack_3",
    "craft_select_1",
    "craft_select_2",
    "craft_select_3",
    "craft_move_1",
    "craft_move_2",
    "craft_move_3",
    "craft_attack_1",
    "craft_attack_2",
    "craft_attack_3",
    "taunt_building_1",
    "taunt_building_2",
    "taunt_building_3",
    "taunt_building_4",
    "taunt_building_5",
    "taunt_building_6",
    "taunt_loss_1",
    "taunt_loss_2",
    "taunt_loss_3",
    "taunt_loss_4",
    "taunt_loss_5",
);

/// Why a WAV file is not one of the voice files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WavError {
    /// Shorter than its header or than the lengths it declares.
    Truncated,
    /// Not the `RIFF`/`WAVE`/`fmt `/`data` layout of the generator.
    Layout,
    /// Not mono 16-bit PCM at the game's sample rate.
    Format,
    /// Longer than [`MAX_VOICE_SECS`].
    TooLong,
}

/// The samples of a voice file: see the module documentation for the format it accepts.
pub fn parse_wav(bytes: &[u8]) -> Result<Vec<Sample>, WavError> {
    const HEADER: usize = 44;
    let u16_at = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
    let u32_at =
        |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    if bytes.len() < HEADER {
        return Err(WavError::Truncated);
    }
    if &bytes[0..4] != b"RIFF"
        || &bytes[8..12] != b"WAVE"
        || &bytes[12..16] != b"fmt "
        || u32_at(16) != 16
        || &bytes[36..40] != b"data"
    {
        return Err(WavError::Layout);
    }
    let rate = SAMPLE_RATE_HZ;
    if u16_at(20) != 1
        || u16_at(22) != 1
        || u32_at(24) != rate
        || u32_at(28) != rate * 2
        || u16_at(32) != 2
        || u16_at(34) != 16
    {
        return Err(WavError::Format);
    }
    let data = u32_at(40) as usize;
    if u32_at(4) as usize != bytes.len() - 8
        || data != bytes.len() - HEADER
        || !data.is_multiple_of(2)
    {
        return Err(WavError::Truncated);
    }
    if data / 2 > (MAX_VOICE_SECS * rate as f32) as usize {
        return Err(WavError::TooLong);
    }
    Ok(bytes[HEADER..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| Sample::from(i16::from_le_bytes([pair[0], pair[1]])) / 32_768.0)
        .collect())
}

// --- What is said ---------------------------------------------------------------------------

/// What the announcer says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Announcement {
    MatchStart,
    StructureReady,
    UnitReady,
    LowPower,
    NoCredits,
    BaseAttacked,
    ForcesAttacked,
    StructureLost,
    Promotion,
    SuperweaponReady,
    EnemySuperweapon,
    EnemyLaunch,
    Victory,
    Defeat,
}

impl Announcement {
    #[cfg(test)]
    pub const ALL: [Self; 14] = [
        Self::MatchStart,
        Self::StructureReady,
        Self::UnitReady,
        Self::LowPower,
        Self::NoCredits,
        Self::BaseAttacked,
        Self::ForcesAttacked,
        Self::StructureLost,
        Self::Promotion,
        Self::SuperweaponReady,
        Self::EnemySuperweapon,
        Self::EnemyLaunch,
        Self::Victory,
        Self::Defeat,
    ];

    pub fn file(self) -> &'static str {
        match self {
            Self::MatchStart => "announcer_match_start",
            Self::StructureReady => "announcer_structure_ready",
            Self::UnitReady => "announcer_unit_ready",
            Self::LowPower => "announcer_low_power",
            Self::NoCredits => "announcer_no_credits",
            Self::BaseAttacked => "announcer_base_attacked",
            Self::ForcesAttacked => "announcer_forces_attacked",
            Self::StructureLost => "announcer_structure_lost",
            Self::Promotion => "announcer_promotion",
            Self::SuperweaponReady => "announcer_superweapon_ready",
            Self::EnemySuperweapon => "announcer_enemy_superweapon",
            Self::EnemyLaunch => "announcer_enemy_launch",
            Self::Victory => "announcer_victory",
            Self::Defeat => "announcer_defeat",
        }
    }

    /// Real seconds before the same announcement may be made again.
    pub fn cooldown(self) -> f64 {
        match self {
            Self::BaseAttacked => 30.0,
            Self::ForcesAttacked => 45.0,
            Self::LowPower => 20.0,
            Self::NoCredits => 30.0,
            Self::Promotion => 15.0,
            Self::StructureLost => 8.0,
            Self::UnitReady => 4.0,
            _ => 2.0,
        }
    }
}

/// Who answers an order: the unit's radio voice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Crew {
    Infantry,
    Vehicle,
    /// Aircraft and ships.
    Craft,
}

/// What a unit answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Reply {
    Select,
    Move,
    Attack,
}

/// Lines of each crew and reply.
pub const REPLY_TAKES: u8 = 3;

/// The file of take `take` (counted from 0, wrapping) of `crew`'s `reply`.
pub fn reply_file(crew: Crew, reply: Reply, take: u8) -> String {
    let crew = match crew {
        Crew::Infantry => "infantry",
        Crew::Vehicle => "vehicle",
        Crew::Craft => "craft",
    };
    let reply = match reply {
        Reply::Select => "select",
        Reply::Move => "move",
        Reply::Attack => "attack",
    };
    format!("{crew}_{reply}_{}", take % REPLY_TAKES + 1)
}

/// The crew of a unit of `kind`.
pub fn crew_of(definitions: &sim::Definitions, kind: sim::UnitKind) -> Crew {
    match definitions.domain(kind) {
        Domain::Air | Domain::Naval => Crew::Craft,
        _ if definitions.on_foot(kind) => Crew::Infantry,
        _ => Crew::Vehicle,
    }
}

/// The reply an accepted order earns, and the unit that gives it; `None` for an order units do
/// not answer.
pub fn order_reply(command: &Command) -> Option<(Reply, SimId)> {
    let (reply, units) = match command {
        Command::Attack { units, .. }
        | Command::AttackMove { units, .. }
        | Command::ForceFire { units, .. } => (Reply::Attack, units),
        Command::Move { units, .. }
        | Command::ForceMove { units, .. }
        | Command::QueueMove { units, .. }
        | Command::Follow { units, .. }
        | Command::Guard { units }
        | Command::Scatter { units }
        | Command::Harvest { units, .. } => (Reply::Move, units),
        _ => return None,
    };
    units.first().map(|&unit| (reply, unit))
}

/// The file of an enemy taunt (`crate::taunts`), by its text.
pub fn taunt_file(line: &str) -> Option<String> {
    let find = |lines: &[&str]| lines.iter().position(|l| *l == line);
    find(&crate::taunts::BUILDING_LINES)
        .map(|i| format!("taunt_building_{}", i + 1))
        .or_else(|| find(&crate::taunts::LOSS_LINES).map(|i| format!("taunt_loss_{}", i + 1)))
}

/// The announcement a simulation event makes, given who owns its target (for shots).
pub fn event_announcement(
    event: &SimEvent,
    target_owner: impl Fn(SimId) -> Option<(sim::PlayerId, bool)>,
) -> Option<Announcement> {
    match event {
        SimEvent::ProductionComplete { player, item } if *player == HUMAN => Some(match item {
            ProductionItem::Building(_) => Announcement::StructureReady,
            ProductionItem::Unit(_) => Announcement::UnitReady,
        }),
        SimEvent::BuildingDestroyed { player, .. } if *player == HUMAN => {
            Some(Announcement::StructureLost)
        }
        SimEvent::RankGained { player, .. } if *player == HUMAN => Some(Announcement::Promotion),
        SimEvent::SuperweaponReady { player, .. } if *player == HUMAN => {
            Some(Announcement::SuperweaponReady)
        }
        SimEvent::SuperweaponBuilt { player, .. } if *player != HUMAN => {
            Some(Announcement::EnemySuperweapon)
        }
        SimEvent::SuperweaponLaunched { player, .. } if *player != HUMAN => {
            Some(Announcement::EnemyLaunch)
        }
        SimEvent::MatchEnded {
            winner: Some(winner),
        } => Some(if *winner == HUMAN {
            Announcement::Victory
        } else {
            Announcement::Defeat
        }),
        SimEvent::PlayerDefeated { player } if *player == HUMAN => Some(Announcement::Defeat),
        SimEvent::WeaponFired {
            attacker, target, ..
        } => match (target_owner(*target), target_owner(*attacker)) {
            (Some((HUMAN, building)), attacker) if attacker.is_none_or(|(p, _)| p != HUMAN) => {
                Some(if building {
                    Announcement::BaseAttacked
                } else {
                    Announcement::ForcesAttacked
                })
            }
            _ => None,
        },
        _ => None,
    }
}

// --- Playing --------------------------------------------------------------------------------

/// The decoded voice lines.
#[derive(Resource, Debug, Default)]
pub struct VoiceBank {
    clips: BTreeMap<String, (Handle<Pcm>, Duration)>,
}

/// Lines other modules ask to be said with priority (the enemy's taunts), by file name.
#[derive(Resource, Debug, Default)]
pub struct VoiceRequests(pub Vec<String>);

/// A voice line playing; removed at `expires` (real seconds) at the latest.
#[derive(Component, Debug)]
pub struct VoiceClip {
    expires: f64,
}

/// Who speaks, what waits, and what was answered.
#[derive(Resource, Debug, Default)]
pub struct VoiceState {
    /// The line playing: its entity, when it ends, and whether it is an announcement or taunt.
    speaking: Option<(Entity, f64, bool)>,
    queue: VecDeque<String>,
    last: BTreeMap<Announcement, f64>,
    takes: BTreeMap<(Crew, Reply), u8>,
    last_reply: Option<f64>,
    accepted: u64,
    selection: BTreeSet<SimId>,
    tick: u64,
    started: bool,
    ended: bool,
    low_power: bool,
    broke: bool,
    /// Lines started so far (for tests).
    pub said: u64,
}

impl VoiceState {
    /// Queues `announcement` at real time `now` unless it was made within its cooldown.
    pub fn announce(&mut self, announcement: Announcement, now: f64) -> bool {
        if self
            .last
            .get(&announcement)
            .is_some_and(|&last| now - last < announcement.cooldown())
        {
            return false;
        }
        self.last.insert(announcement, now);
        self.urge(announcement.file().to_owned());
        true
    }

    /// Queues a line that waits for its turn; the oldest waiting one is dropped when full.
    fn urge(&mut self, file: String) {
        if self.queue.len() >= MAX_QUEUED {
            self.queue.pop_front();
        }
        self.queue.push_back(file);
    }

    /// The next take of `crew`'s `reply`, if a reply may be given at `now`.
    pub fn reply(&mut self, crew: Crew, reply: Reply, now: f64) -> Option<String> {
        if self.last_reply.is_some_and(|t| now - t < REPLY_GAP_SECS) {
            return None;
        }
        self.last_reply = Some(now);
        let take = self.takes.entry((crew, reply)).or_insert(0);
        let file = reply_file(crew, reply, *take);
        *take = take.wrapping_add(1) % REPLY_TAKES;
        Some(file)
    }
}

pub struct VoicesPlugin;

impl Plugin for VoicesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VoiceState>()
            .init_resource::<VoiceRequests>()
            .init_resource::<VoiceBank>()
            .add_systems(Startup, load_voices)
            // After every input and simulation system of the frame, so it sees this frame's
            // selection, orders and events.
            .add_systems(Last, speak);
    }
}

fn load_voices(mut bank: ResMut<VoiceBank>, mut sounds: ResMut<Assets<Pcm>>) {
    for (name, bytes) in FILES {
        match parse_wav(bytes) {
            Ok(samples) => {
                let pcm = Pcm::new(samples);
                let duration = pcm.duration();
                bank.clips
                    .insert((*name).to_owned(), (sounds.add(pcm), duration));
            }
            Err(error) => error!("voice line {name}: {error:?}"),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn speak(
    mut commands: Commands,
    mut events: MessageReader<SimEvent>,
    host: Res<SimHost>,
    sink: Res<CommandSink>,
    selection: Res<Selection>,
    bank: Res<VoiceBank>,
    volume: Res<VolumeSettings>,
    time: Res<Time<Real>>,
    mut state: ResMut<VoiceState>,
    mut requests: ResMut<VoiceRequests>,
    unit_views: Res<UnitViews>,
    building_views: Res<BuildingViews>,
    unit_view: Query<&UnitView>,
    building_view: Query<&BuildingView>,
    clips: Query<(Entity, &VoiceClip)>,
) {
    let now = time.elapsed_secs_f64();
    // Lines that ended (also without an audio device, when Bevy never plays them).
    for (entity, clip) in &clips {
        if now >= clip.expires {
            commands.entity(entity).despawn();
            if state.speaking.is_some_and(|(e, ..)| e == entity) {
                state.speaking = None;
            }
        }
    }
    if state.speaking.is_some_and(|(_, ends, _)| now >= ends) {
        state.speaking = None;
    }
    // A new match: forget the last one.
    let tick = host.tick();
    if tick < state.tick {
        let said = state.said;
        let accepted = state.accepted;
        *state = VoiceState {
            said,
            accepted,
            ..default()
        };
    }
    state.tick = tick;
    let definitions = host.definitions();
    let world = host.world();
    let owner = |id: SimId| -> Option<(sim::PlayerId, bool)> {
        if let Some(view) = unit_views.0.get(&id).and_then(|&e| unit_view.get(e).ok()) {
            return Some((view.owner, false));
        }
        let view = building_views
            .0
            .get(&id)
            .and_then(|&e| building_view.get(e).ok())?;
        world
            .get::<sim::Owner>(view.building)
            .map(|owner| (owner.0, true))
    };
    for event in events.read() {
        if let Some(announcement) = event_announcement(event, owner) {
            if matches!(announcement, Announcement::Victory | Announcement::Defeat) {
                if state.ended {
                    continue;
                }
                state.ended = true;
            }
            state.announce(announcement, now);
        }
    }
    if host.running() {
        if !state.started && tick > 0 {
            state.started = true;
            state.announce(Announcement::MatchStart, now);
        }
        if let Some(player) = host.players().and_then(|players| players.get(HUMAN)) {
            let low = player.low_power();
            if low && !state.low_power {
                state.announce(Announcement::LowPower, now);
            }
            state.low_power = low;
            let broke = player.credits() <= 0;
            if broke && !state.broke && state.started {
                state.announce(Announcement::NoCredits, now);
            }
            state.broke = broke;
        }
    }
    for file in requests.0.drain(..) {
        state.urge(file);
    }
    // The player's units answer: a new order, else a new selection.
    let crew = |id: SimId| {
        let view = unit_views.0.get(&id).and_then(|&e| unit_view.get(e).ok())?;
        let unit = world.get::<sim::Unit>(view.unit)?;
        Some(crew_of(definitions?, unit.kind))
    };
    let mut reply = None;
    if sink.accepted() > state.accepted
        && let Some((kind, unit)) = sink.last_accepted().and_then(order_reply)
        && let Some(crew) = crew(unit)
    {
        reply = Some((crew, kind));
    }
    state.accepted = sink.accepted();
    if selection.is_changed() {
        let fresh = selection
            .0
            .iter()
            .find(|id| !state.selection.contains(id))
            .copied();
        state.selection = selection.0.clone();
        if reply.is_none()
            && let Some(crew) = fresh.and_then(crew)
        {
            reply = Some((crew, Reply::Select));
        }
    }
    let gain = volume.gain(Channel::Effects);
    let play = |commands: &mut Commands, state: &mut VoiceState, file: &str, priority: bool| {
        let Some((handle, duration)) = bank.clips.get(file) else {
            return;
        };
        let ends = now + duration.as_secs_f64();
        let entity = commands
            .spawn((
                AudioPlayer(handle.clone()),
                PlaybackSettings::DESPAWN.with_volume(Volume::Linear(gain)),
                VoiceClip {
                    expires: ends + 1.0,
                },
            ))
            .id();
        state.speaking = Some((entity, ends, priority));
        state.said = state.said.saturating_add(1);
    };
    if let Some((crew, kind)) = reply
        && !state.speaking.is_some_and(|(.., priority)| priority)
        && let Some(file) = state.reply(crew, kind, now)
    {
        if let Some((entity, ..)) = state.speaking.take() {
            commands.entity(entity).despawn();
        }
        if gain > 0.0 {
            play(&mut commands, &mut state, &file, false);
        }
    }
    if state.speaking.is_none()
        && let Some(file) = state.queue.pop_front()
        && gain > 0.0
    {
        play(&mut commands, &mut state, &file, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_voice_file_is_in_the_accepted_format() {
        assert_eq!(FILES.len(), 52);
        let names: BTreeSet<&str> = FILES.iter().map(|(name, _)| *name).collect();
        assert_eq!(names.len(), FILES.len(), "no line twice");
        for (name, bytes) in FILES {
            let samples = parse_wav(bytes).unwrap_or_else(|e| panic!("{name}: {e:?}"));
            assert!(
                samples.len() > SAMPLE_RATE_HZ as usize / 4,
                "{name} is too short"
            );
            let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(peak > 0.2 && peak <= 1.0, "{name}: peak {peak}");
        }
    }

    #[test]
    fn every_line_the_game_names_has_a_file() {
        let names: BTreeSet<&str> = FILES.iter().map(|(name, _)| *name).collect();
        for announcement in Announcement::ALL {
            assert!(names.contains(announcement.file()), "{announcement:?}");
        }
        for crew in [Crew::Infantry, Crew::Vehicle, Crew::Craft] {
            for reply in [Reply::Select, Reply::Move, Reply::Attack] {
                for take in 0..REPLY_TAKES {
                    let file = reply_file(crew, reply, take);
                    assert!(names.contains(file.as_str()), "{file}");
                }
            }
        }
        for line in crate::taunts::BUILDING_LINES
            .iter()
            .chain(&crate::taunts::LOSS_LINES)
        {
            let file = taunt_file(line).expect("taunt has a voice");
            assert!(names.contains(file.as_str()), "{file}");
        }
    }

    #[test]
    fn malformed_files_are_refused() {
        let good = FILES[0].1;
        assert!(parse_wav(good).is_ok());
        assert_eq!(parse_wav(&good[..40]), Err(WavError::Truncated));
        assert_eq!(parse_wav(&good[..good.len() - 2]), Err(WavError::Truncated));
        let mut stereo = good.to_vec();
        stereo[22] = 2;
        assert_eq!(parse_wav(&stereo), Err(WavError::Format));
        let mut rate = good.to_vec();
        rate[24..28].copy_from_slice(&44_100u32.to_le_bytes());
        assert_eq!(parse_wav(&rate), Err(WavError::Format));
        let mut list = good.to_vec();
        list[36..40].copy_from_slice(b"LIST");
        assert_eq!(parse_wav(&list), Err(WavError::Layout));
        let mut long = good[..44].to_vec();
        let samples = (MAX_VOICE_SECS as usize + 1) * SAMPLE_RATE_HZ as usize;
        long.resize(44 + samples * 2, 0);
        let data = (samples * 2) as u32;
        long[4..8].copy_from_slice(&(data + 36).to_le_bytes());
        long[40..44].copy_from_slice(&data.to_le_bytes());
        assert_eq!(parse_wav(&long), Err(WavError::TooLong));
    }

    #[test]
    fn announcements_respect_their_cooldowns() {
        let mut state = VoiceState::default();
        assert!(state.announce(Announcement::BaseAttacked, 0.0));
        assert!(!state.announce(Announcement::BaseAttacked, 10.0));
        assert!(state.announce(Announcement::StructureLost, 10.0));
        assert!(state.announce(Announcement::BaseAttacked, 31.0));
        // The queue keeps the newest lines.
        state.announce(Announcement::Victory, 40.0);
        assert_eq!(state.queue.len(), MAX_QUEUED);
        assert_eq!(
            state.queue.back().map(String::as_str),
            Some("announcer_victory")
        );
    }

    #[test]
    fn replies_rotate_and_are_spaced() {
        let mut state = VoiceState::default();
        let a = state.reply(Crew::Vehicle, Reply::Move, 0.0).unwrap();
        assert!(state.reply(Crew::Vehicle, Reply::Move, 0.2).is_none());
        let b = state.reply(Crew::Vehicle, Reply::Move, 1.0).unwrap();
        assert_ne!(a, b);
        assert_eq!(a, "vehicle_move_1");
    }

    #[test]
    fn the_announcer_speaks_only_of_the_players_side_or_public_news() {
        let unknown = |_| None;
        let ours = |id: SimId| {
            Some((
                if id == SimId(1) {
                    HUMAN
                } else {
                    sim::PlayerId(2)
                },
                true,
            ))
        };
        let enemy = sim::PlayerId(2);
        let item = ProductionItem::Unit(sim::UnitKind(0));
        assert_eq!(
            event_announcement(
                &SimEvent::ProductionComplete {
                    player: enemy,
                    item
                },
                unknown
            ),
            None
        );
        assert_eq!(
            event_announcement(
                &SimEvent::ProductionComplete {
                    player: HUMAN,
                    item
                },
                unknown
            ),
            Some(Announcement::UnitReady)
        );
        let shot = |attacker, target| SimEvent::WeaponFired {
            attacker: SimId(attacker),
            target: SimId(target),
            weapon: sim::WeaponKind(0),
        };
        assert_eq!(
            event_announcement(&shot(2, 1), ours),
            Some(Announcement::BaseAttacked)
        );
        // Our own shots, and shots between others, say nothing.
        assert_eq!(event_announcement(&shot(1, 2), ours), None);
        assert_eq!(event_announcement(&shot(3, 2), ours), None);
        assert_eq!(
            event_announcement(
                &SimEvent::MatchEnded {
                    winner: Some(HUMAN)
                },
                unknown
            ),
            Some(Announcement::Victory)
        );
        assert_eq!(
            event_announcement(&SimEvent::MatchEnded { winner: None }, unknown),
            None
        );
    }
}
