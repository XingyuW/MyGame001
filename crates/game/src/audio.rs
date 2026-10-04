//! Sound effects, music, and volume settings (PLAN.md section 7, M5; docs/DECISIONS.md D38).
//!
//! Every sound is synthesised by the code below when the game starts: no audio file is read,
//! and no audio decoder runs. The samples reach Bevy's audio output as a custom audio source
//! ([`Pcm`], a `bevy::audio::Decodable` that hands out the samples as they are), so Bevy's
//! `AudioSource` and its decoders, whose `decoder()` unwraps the decoder's result, are never
//! used. Bevy's `.ogg` loader stays registered by the `audio` feature, but the asset reader of
//! `crate::checked_assets` serves no audio file, so it never runs either.
//!
//! What is heard follows what is seen (as on screen, D31, D33): the effects of the human player's
//! own production, placement, sales, deliveries, and rejected commands play; shots play only
//! when the human player sees the attacker or the target; a destruction plays only for the human
//! player's own units and buildings and for enemies whose view was shown. Events of the opponent's
//! economy never play.
//!
//! At most [`MAX_ACTIVE_EFFECTS`] effects play at once, the same effect at most once per
//! [`MIN_REPEAT_SECS`], and every effect entity is removed when its sound has ended, also when
//! the machine has no audio device (Bevy then never starts the sound), so effects cannot
//! accumulate.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use bevy::audio::{AddAudioSource, ChannelCount, Sample, SampleRate, Source, Volume};
use bevy::prelude::*;
use sim::{SimEvent, SimId, WeaponKind};

use crate::buildings::{BuildingView, BuildingViews};
use crate::sim_host::{CommandSink, SimHost};
use crate::units::{HUMAN, UnitView, UnitViews};

/// Sample rate of every sound.
pub const SAMPLE_RATE_HZ: u32 = 22_050;

const RATE: SampleRate = SampleRate::MIN.saturating_add(SAMPLE_RATE_HZ - 1);

/// Most effects that play at once.
pub const MAX_ACTIVE_EFFECTS: usize = 8;

/// Shortest time between two starts of the same effect, in seconds.
pub const MIN_REPEAT_SECS: f64 = 0.08;

/// Volume levels run from 0 (silent) to this (full).
pub const VOLUME_STEPS: u8 = 10;

pub struct GameAudioPlugin;

impl Plugin for GameAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<Pcm>()
            .init_resource::<VolumeSettings>()
            .init_resource::<SfxState>()
            .add_systems(Startup, setup_audio.after(crate::sim_host::start_match))
            .add_systems(
                Update,
                (play_effects, expire_effects, apply_music_volume)
                    .chain()
                    // The views' registries, which tell whether a destroyed enemy was shown.
                    .after(crate::units::spawn_unit_views)
                    .after(crate::buildings::spawn_building_views),
            );
    }
}

// --- Samples as an audio source -------------------------------------------------------------

/// Mono samples at [`SAMPLE_RATE_HZ`], synthesised by this module.
#[derive(Asset, TypePath, Clone, Debug)]
pub struct Pcm {
    samples: Arc<[Sample]>,
}

impl Pcm {
    /// A sound of `samples`; an empty list becomes one silent sample, so that a looping sound
    /// always has something to repeat.
    pub fn new(mut samples: Vec<Sample>) -> Self {
        if samples.is_empty() {
            samples.push(0.0);
        }
        Self {
            samples: samples.into(),
        }
    }

    pub fn duration(&self) -> Duration {
        Duration::from_secs_f64(self.samples.len() as f64 / f64::from(SAMPLE_RATE_HZ))
    }
}

/// Hands out the samples of a [`Pcm`] in order.
pub struct PcmDecoder {
    samples: Arc<[Sample]>,
    position: usize,
    duration: Duration,
}

impl Iterator for PcmDecoder {
    type Item = Sample;

    fn next(&mut self) -> Option<Sample> {
        let sample = self.samples.get(self.position).copied()?;
        self.position += 1;
        Some(sample)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = self.samples.len().saturating_sub(self.position);
        (left, Some(left))
    }
}

impl Source for PcmDecoder {
    fn current_span_len(&self) -> Option<usize> {
        // One span of the whole sound; `Some(0)` once it is exhausted (rodio's contract).
        if self.position >= self.samples.len() {
            Some(0)
        } else {
            Some(self.samples.len())
        }
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::MIN
    }

    fn sample_rate(&self) -> SampleRate {
        RATE
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(self.duration)
    }
}

impl Decodable for Pcm {
    type Decoder = PcmDecoder;

    fn decoder(&self) -> PcmDecoder {
        PcmDecoder {
            samples: self.samples.clone(),
            position: 0,
            duration: self.duration(),
        }
    }
}

// --- Synthesis ------------------------------------------------------------------------------

/// The sound effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sfx {
    /// A command of the human player was accepted.
    Acknowledge,
    /// A command of the human player was refused or rejected.
    Error,
    /// Production of the human player completed.
    Ready,
    /// The human player placed a building.
    Placed,
    /// The human player sold a building.
    Sold,
    /// A harvester of the human player delivered ore.
    Credits,
    /// An instant weapon fired where the human player sees it.
    Shot,
    /// A projectile weapon fired where the human player sees it.
    Cannon,
    /// A unit or building the human player was shown was destroyed.
    Explosion,
}

impl Sfx {
    pub const ALL: [Sfx; 9] = [
        Sfx::Acknowledge,
        Sfx::Error,
        Sfx::Ready,
        Sfx::Placed,
        Sfx::Sold,
        Sfx::Credits,
        Sfx::Shot,
        Sfx::Cannon,
        Sfx::Explosion,
    ];
}

/// A deterministic noise source (xorshift32), so that every run synthesises the same sounds.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

#[derive(Clone, Copy)]
enum Wave {
    Sine,
    Square,
    Triangle,
}

fn wave(kind: Wave, phase: f32) -> f32 {
    let t = phase.fract();
    match kind {
        Wave::Sine => (t * std::f32::consts::TAU).sin(),
        Wave::Square => {
            if t < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        Wave::Triangle => 1.0 - 4.0 * (t - 0.5).abs(),
    }
}

fn samples_for(seconds: f32) -> usize {
    (seconds * SAMPLE_RATE_HZ as f32).round() as usize
}

/// Adds a note to `buffer` from sample `start`: `wave` gliding from `from` to `to` Hz over
/// `seconds`, at `gain`, with a 5 ms attack and a linear (or, with `decay`, exponential) fade.
#[allow(clippy::too_many_arguments)]
fn add_note(
    buffer: &mut [f32],
    start: usize,
    seconds: f32,
    kind: Wave,
    from: f32,
    to: f32,
    gain: f32,
    decay: bool,
) {
    let length = samples_for(seconds).max(1);
    let attack = samples_for(0.005).max(1);
    let mut phase = 0.0f32;
    for i in 0..length {
        let Some(slot) = buffer.get_mut(start + i) else {
            break;
        };
        let progress = i as f32 / length as f32;
        let frequency = from + (to - from) * progress;
        phase += frequency / SAMPLE_RATE_HZ as f32;
        let fade = if decay {
            (-5.0 * progress).exp() * (1.0 - progress)
        } else {
            1.0 - progress
        };
        let rise = (i as f32 / attack as f32).min(1.0);
        *slot += wave(kind, phase) * gain * fade * rise;
    }
}

/// Adds noise to `buffer` from `start` for `seconds`, low-passed by `smoothing` (0: none, close
/// to 1: dull), fading out.
fn add_noise(buffer: &mut [f32], start: usize, seconds: f32, gain: f32, smoothing: f32, seed: u32) {
    let length = samples_for(seconds).max(1);
    let mut noise = Noise(seed.max(1));
    let mut previous = 0.0f32;
    for i in 0..length {
        let Some(slot) = buffer.get_mut(start + i) else {
            break;
        };
        let progress = i as f32 / length as f32;
        previous = previous * smoothing + noise.next() * (1.0 - smoothing);
        *slot += previous * gain * (1.0 - progress) * (1.0 - progress);
    }
}

/// Scales `buffer` so that its loudest sample is `peak` (silence stays silent).
fn normalise(mut buffer: Vec<f32>, peak: f32) -> Vec<Sample> {
    let loudest = buffer
        .iter()
        .fold(0.0f32, |max, sample| max.max(sample.abs()));
    if loudest > 0.0 {
        for sample in &mut buffer {
            *sample *= peak / loudest;
        }
    }
    // `Sample` is `f32` unless rodio's `64bit` feature makes it `f64`.
    #[allow(clippy::useless_conversion)]
    buffer.into_iter().map(Sample::from).collect()
}

/// The samples of `sfx`.
pub fn synthesize(sfx: Sfx) -> Vec<Sample> {
    let seconds = match sfx {
        Sfx::Acknowledge => 0.06,
        Sfx::Error => 0.25,
        Sfx::Ready => 0.45,
        Sfx::Placed => 0.3,
        Sfx::Sold => 0.35,
        Sfx::Credits => 0.2,
        Sfx::Shot => 0.12,
        Sfx::Cannon => 0.3,
        Sfx::Explosion => 0.9,
    };
    let mut buffer = vec![0.0f32; samples_for(seconds)];
    match sfx {
        Sfx::Acknowledge => {
            add_note(&mut buffer, 0, 0.06, Wave::Sine, 1320.0, 1320.0, 0.6, false);
        }
        Sfx::Error => {
            add_note(&mut buffer, 0, 0.11, Wave::Square, 180.0, 170.0, 0.4, false);
            add_note(
                &mut buffer,
                samples_for(0.13),
                0.11,
                Wave::Square,
                150.0,
                140.0,
                0.4,
                false,
            );
        }
        Sfx::Ready => {
            for (index, frequency) in [523.25, 659.25, 783.99].into_iter().enumerate() {
                let start = samples_for(0.12 * index as f32);
                add_note(
                    &mut buffer,
                    start,
                    0.2,
                    Wave::Triangle,
                    frequency,
                    frequency,
                    0.5,
                    true,
                );
            }
        }
        Sfx::Placed => {
            add_note(&mut buffer, 0, 0.3, Wave::Sine, 140.0, 60.0, 0.9, true);
            add_noise(&mut buffer, 0, 0.12, 0.3, 0.8, 7);
        }
        Sfx::Sold => {
            add_note(
                &mut buffer,
                0,
                0.15,
                Wave::Triangle,
                880.0,
                880.0,
                0.5,
                true,
            );
            add_note(
                &mut buffer,
                samples_for(0.1),
                0.25,
                Wave::Triangle,
                1174.66,
                1174.66,
                0.5,
                true,
            );
        }
        Sfx::Credits => {
            add_note(&mut buffer, 0, 0.2, Wave::Sine, 1567.98, 1567.98, 0.4, true);
        }
        Sfx::Shot => {
            add_noise(&mut buffer, 0, 0.12, 0.9, 0.2, 11);
        }
        Sfx::Cannon => {
            add_noise(&mut buffer, 0, 0.25, 0.7, 0.6, 13);
            add_note(&mut buffer, 0, 0.3, Wave::Sine, 110.0, 45.0, 0.8, true);
        }
        Sfx::Explosion => {
            add_noise(&mut buffer, 0, 0.9, 1.0, 0.85, 17);
            add_note(&mut buffer, 0, 0.7, Wave::Sine, 80.0, 30.0, 0.9, true);
        }
    }
    normalise(buffer, 0.8)
}

/// Seconds per beat of the music (100 beats per minute).
const BEAT_SECS: f32 = 0.6;

/// Bars of the music loop, four beats each: two bars per chord of A minor, F, C, and G.
const MUSIC_BARS: usize = 8;

/// The music: a loop of [`MUSIC_BARS`] bars of bass, arpeggio, and drums.
pub fn music() -> Vec<Sample> {
    // Root of each chord (octave 2) and its three tones (octave 4).
    const CHORDS: [(f32, [f32; 3]); 4] = [
        (110.0, [440.0, 523.25, 659.25]),
        (87.31, [349.23, 440.0, 523.25]),
        (130.81, [523.25, 659.25, 783.99]),
        (98.0, [392.0, 493.88, 587.33]),
    ];
    let beats = MUSIC_BARS * 4;
    let mut buffer = vec![0.0f32; samples_for(beats as f32 * BEAT_SECS)];
    let eighth = BEAT_SECS / 2.0;
    let sixteenth = BEAT_SECS / 4.0;
    for bar in 0..MUSIC_BARS {
        let (root, tones) = CHORDS[(bar / 2) % CHORDS.len()];
        let bar_start = bar as f32 * 4.0 * BEAT_SECS;
        for step in 0..8 {
            let start = samples_for(bar_start + step as f32 * eighth);
            let octave = if step % 4 == 3 { 2.0 } else { 1.0 };
            add_note(
                &mut buffer,
                start,
                eighth * 0.9,
                Wave::Triangle,
                root * octave,
                root * octave,
                0.35,
                false,
            );
        }
        // The arpeggio enters in the second half of the loop.
        if bar >= MUSIC_BARS / 2 {
            for step in 0..16 {
                let tone = tones[[0, 1, 2, 1][step % 4]];
                let start = samples_for(bar_start + step as f32 * sixteenth);
                add_note(
                    &mut buffer,
                    start,
                    sixteenth * 0.8,
                    Wave::Square,
                    tone,
                    tone,
                    0.05,
                    true,
                );
            }
        }
        // A pad of the chord's tones over the whole bar.
        for tone in tones {
            add_note(
                &mut buffer,
                samples_for(bar_start),
                4.0 * BEAT_SECS * 0.98,
                Wave::Sine,
                tone / 2.0,
                tone / 2.0,
                0.05,
                false,
            );
        }
        for beat in 0..4 {
            let start = samples_for(bar_start + beat as f32 * BEAT_SECS);
            if beat % 2 == 0 {
                add_note(&mut buffer, start, 0.15, Wave::Sine, 120.0, 40.0, 0.6, true);
            }
            let off = samples_for(bar_start + beat as f32 * BEAT_SECS + eighth);
            add_noise(&mut buffer, off, 0.03, 0.12, 0.0, 23 + beat as u32);
        }
    }
    normalise(buffer, 0.7)
}

// --- Volume ---------------------------------------------------------------------------------

/// A volume channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Channel {
    Master,
    Music,
    Effects,
}

/// The volume settings of this session, in steps of 0 to [`VOLUME_STEPS`] (they are not stored
/// between sessions; options are M7's).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct VolumeSettings {
    pub master: u8,
    pub music: u8,
    pub effects: u8,
    pub muted: bool,
}

impl Default for VolumeSettings {
    fn default() -> Self {
        Self {
            master: 8,
            music: 5,
            effects: 7,
            muted: false,
        }
    }
}

impl VolumeSettings {
    pub fn level(&self, channel: Channel) -> u8 {
        match channel {
            Channel::Master => self.master,
            Channel::Music => self.music,
            Channel::Effects => self.effects,
        }
    }

    /// Raises (`up`) or lowers the channel by one step, within 0 to [`VOLUME_STEPS`].
    pub fn step(&mut self, channel: Channel, up: bool) {
        let level = match channel {
            Channel::Master => &mut self.master,
            Channel::Music => &mut self.music,
            Channel::Effects => &mut self.effects,
        };
        *level = if up {
            level.saturating_add(1).min(VOLUME_STEPS)
        } else {
            level.saturating_sub(1)
        };
    }

    /// The linear gain of `channel` (music or effects): master × channel, 0 when muted.
    pub fn gain(&self, channel: Channel) -> f32 {
        if self.muted {
            return 0.0;
        }
        let step = |level: u8| f32::from(level.min(VOLUME_STEPS)) / f32::from(VOLUME_STEPS);
        match channel {
            Channel::Master => step(self.master),
            channel => step(self.master) * step(self.level(channel)),
        }
    }
}

// --- What plays -----------------------------------------------------------------------------

/// What the human player sees, for [`event_sound`].
pub struct SoundContext<'a> {
    /// Whether the human player sees the unit or building now (its own always).
    pub seen: &'a dyn Fn(SimId) -> bool,
    /// Whether the view of the unit or building was shown to the human player (for entities
    /// removed in the tick: their views remain until the next drawing).
    pub shown: &'a dyn Fn(SimId) -> bool,
    /// Whether the weapon is instant.
    pub instant: &'a dyn Fn(WeaponKind) -> bool,
}

/// The effect a simulation event plays for the human player, if any: see the module
/// documentation.
pub fn event_sound(event: &SimEvent, context: &SoundContext) -> Option<Sfx> {
    match event {
        SimEvent::CommandRejected { player, .. } => (*player == HUMAN).then_some(Sfx::Error),
        SimEvent::ProductionComplete { player, .. } => (*player == HUMAN).then_some(Sfx::Ready),
        SimEvent::BuildingPlaced { player, .. } => (*player == HUMAN).then_some(Sfx::Placed),
        SimEvent::BuildingSold { player, .. } => (*player == HUMAN).then_some(Sfx::Sold),
        SimEvent::OreDelivered { player, .. } => (*player == HUMAN).then_some(Sfx::Credits),
        SimEvent::WeaponFired {
            attacker,
            target,
            weapon,
        } => ((context.seen)(*attacker) || (context.seen)(*target)).then(|| {
            if (context.instant)(*weapon) {
                Sfx::Shot
            } else {
                Sfx::Cannon
            }
        }),
        SimEvent::UnitDestroyed { player, unit } => {
            (*player == HUMAN || (context.shown)(*unit)).then_some(Sfx::Explosion)
        }
        SimEvent::BuildingDestroyed {
            player, building, ..
        } => (*player == HUMAN || (context.shown)(*building)).then_some(Sfx::Explosion),
        SimEvent::MoveCompleted { .. }
        | SimEvent::MoveFailed { .. }
        | SimEvent::ProductionCancelled { .. }
        | SimEvent::UnitSpawned { .. }
        | SimEvent::HarvesterIdle { .. }
        | SimEvent::ProjectileExpired { .. }
        | SimEvent::AttackEnded { .. } => None,
    }
}

/// Whether an effect last started at `last` (seconds) may start at `now` with `active` effects
/// playing.
pub fn may_play(last: Option<f64>, now: f64, active: usize) -> bool {
    active < MAX_ACTIVE_EFFECTS && last.is_none_or(|last| now - last >= MIN_REPEAT_SECS)
}

// --- Systems --------------------------------------------------------------------------------

/// The synthesised sounds.
#[derive(Resource, Debug)]
pub struct SoundBank {
    pub effects: BTreeMap<Sfx, (Handle<Pcm>, Duration)>,
}

/// The entity that plays the music.
#[derive(Component, Debug)]
pub struct MusicPlayer;

/// An effect being played; removed at `expires` (seconds) at the latest.
#[derive(Component, Debug)]
pub struct SoundEffect {
    pub expires: f64,
}

/// When each effect last started, and the command counters already answered with a sound.
#[derive(Resource, Debug, Default)]
pub struct SfxState {
    last: BTreeMap<Sfx, f64>,
    accepted: u64,
    refused: u64,
    /// Effects started so far (for the smoke test).
    pub started: u64,
}

fn setup_audio(
    mut commands: Commands,
    mut sounds: ResMut<Assets<Pcm>>,
    volume: Res<VolumeSettings>,
) {
    let mut effects = BTreeMap::new();
    for sfx in Sfx::ALL {
        let pcm = Pcm::new(synthesize(sfx));
        let duration = pcm.duration();
        effects.insert(sfx, (sounds.add(pcm), duration));
    }
    let music = sounds.add(Pcm::new(music()));
    commands.spawn((
        MusicPlayer,
        AudioPlayer(music),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(volume.gain(Channel::Music))),
    ));
    commands.insert_resource(SoundBank { effects });
}

/// The units and buildings the human player sees now: its own always, others while their cells
/// are visible (as `crate::units::unit_shown`, `crate::buildings::building_shown`).
fn seen_entities(host: &SimHost) -> BTreeMap<SimId, bool> {
    let Some(players) = host.players() else {
        return BTreeMap::new();
    };
    let Some(mut entities) = host.world().try_query::<(
        &SimId,
        &sim::Owner,
        Option<&sim::Mobile>,
        Option<&sim::Building>,
    )>() else {
        return BTreeMap::new();
    };
    entities
        .iter(host.world())
        .map(|(id, owner, mobile, building)| {
            let seen = match (mobile, building) {
                (Some(mobile), _) => crate::units::unit_shown(owner.0, mobile, players),
                (None, Some(building)) => {
                    crate::buildings::building_shown(owner.0, building.footprint, players)
                }
                (None, None) => false,
            };
            (*id, seen)
        })
        .collect()
}

/// The visibility of the unit and building views.
type ViewVisibility<'w, 's> =
    Query<'w, 's, &'static Visibility, Or<(With<UnitView>, With<BuildingView>)>>;

#[allow(clippy::too_many_arguments)]
fn play_effects(
    mut commands: Commands,
    mut events: MessageReader<SimEvent>,
    host: Res<SimHost>,
    sink: Res<CommandSink>,
    bank: Option<Res<SoundBank>>,
    volume: Res<VolumeSettings>,
    time: Res<Time<Real>>,
    mut state: ResMut<SfxState>,
    unit_views: Res<UnitViews>,
    building_views: Res<BuildingViews>,
    views: ViewVisibility,
    active: Query<(), With<SoundEffect>>,
) {
    let Some(bank) = bank else {
        events.clear();
        return;
    };
    let mut wanted = Vec::new();
    if sink.accepted() > state.accepted {
        wanted.push(Sfx::Acknowledge);
    }
    if sink.refused() > state.refused {
        wanted.push(Sfx::Error);
    }
    state.accepted = sink.accepted();
    state.refused = sink.refused();
    let events: Vec<&SimEvent> = events.read().collect();
    let definitions = host.definitions();
    // Who the human player sees is looked up only when a shot needs it, once per frame.
    let visible = if events
        .iter()
        .any(|event| matches!(event, SimEvent::WeaponFired { .. }))
    {
        seen_entities(&host)
    } else {
        BTreeMap::new()
    };
    let seen = |id: SimId| visible.get(&id).copied().unwrap_or(false);
    let shown = |id: SimId| {
        unit_views
            .0
            .get(&id)
            .or_else(|| building_views.0.get(&id))
            .and_then(|&view| views.get(view).ok())
            .is_some_and(|visibility| *visibility != Visibility::Hidden)
    };
    let instant = |weapon: WeaponKind| {
        definitions
            .and_then(|definitions| definitions.weapon(weapon))
            .is_some_and(|def| def.projectile_speed == 0)
    };
    let context = SoundContext {
        seen: &seen,
        shown: &shown,
        instant: &instant,
    };
    wanted.extend(
        events
            .into_iter()
            .filter_map(|event| event_sound(event, &context)),
    );
    let now = time.elapsed_secs_f64();
    let mut playing = active.iter().count();
    let gain = volume.gain(Channel::Effects);
    for sfx in wanted {
        if gain <= 0.0 || !may_play(state.last.get(&sfx).copied(), now, playing) {
            continue;
        }
        let Some((handle, duration)) = bank.effects.get(&sfx) else {
            continue;
        };
        commands.spawn((
            AudioPlayer(handle.clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(gain)),
            SoundEffect {
                expires: now + duration.as_secs_f64() + 1.0,
            },
        ));
        state.last.insert(sfx, now);
        state.started = state.started.saturating_add(1);
        playing += 1;
    }
}

/// Removes effects whose sound has ended (Bevy removes them itself only when an audio device
/// played them).
fn expire_effects(
    mut commands: Commands,
    time: Res<Time<Real>>,
    effects: Query<(Entity, &SoundEffect)>,
) {
    let now = time.elapsed_secs_f64();
    for (entity, effect) in &effects {
        if now >= effect.expires {
            commands.entity(entity).despawn();
        }
    }
}

/// Applies the music volume to the playing music when the settings change or the music starts.
fn apply_music_volume(
    volume: Res<VolumeSettings>,
    mut music: Query<&mut AudioSink, With<MusicPlayer>>,
) {
    for mut sink in &mut music {
        if volume.is_changed() || sink.is_added() {
            sink.set_volume(Volume::Linear(volume.gain(Channel::Music)));
        }
    }
}

#[cfg(test)]
mod tests {
    use sim::{
        AttackEnd, BuildingKind, Command, HarvestIdle, MoveFailure, PlayerId, ProductionItem,
        RejectReason, UnitKind,
    };

    use super::*;

    /// Longest effect, in seconds.
    const MAX_EFFECT_SECS: f32 = 1.5;

    fn check_sound(name: &str, samples: &[Sample], max_secs: f32) {
        assert!(!samples.is_empty(), "{name}");
        assert!(
            samples.len() <= samples_for(max_secs),
            "{name}: {} samples",
            samples.len()
        );
        assert!(
            samples.iter().all(|s| s.is_finite() && s.abs() <= 1.0),
            "{name}"
        );
        let peak = samples.iter().fold(0.0, |m: Sample, s| m.max(s.abs()));
        assert!(peak > 0.1, "{name} is silent");
    }

    #[test]
    fn every_sound_is_synthesised_within_bounds_and_the_same_on_every_run() {
        for sfx in Sfx::ALL {
            let samples = synthesize(sfx);
            check_sound(&format!("{sfx:?}"), &samples, MAX_EFFECT_SECS);
            assert_eq!(samples, synthesize(sfx), "{sfx:?} is deterministic");
        }
        let music = music();
        check_sound("music", &music, 30.0);
        assert!(music.len() >= samples_for(10.0));
        assert_eq!(
            music.len(),
            samples_for(MUSIC_BARS as f32 * 4.0 * BEAT_SECS)
        );
        assert_eq!(music, self::music());
    }

    #[test]
    fn the_decoder_hands_out_the_samples_unchanged() {
        let pcm = Pcm::new(vec![0.25, -0.5, 1.0]);
        let mut decoder = pcm.decoder();
        assert_eq!(decoder.channels().get(), 1);
        assert_eq!(decoder.sample_rate().get(), SAMPLE_RATE_HZ);
        assert_eq!(decoder.current_span_len(), Some(3));
        assert_eq!(
            decoder.total_duration(),
            Some(Duration::from_secs_f64(3.0 / f64::from(SAMPLE_RATE_HZ)))
        );
        assert_eq!(decoder.size_hint(), (3, Some(3)));
        assert_eq!(decoder.by_ref().collect::<Vec<_>>(), [0.25, -0.5, 1.0]);
        assert_eq!(decoder.current_span_len(), Some(0));
        assert_eq!(decoder.next(), None);
        // An empty sound becomes one silent sample, so that a loop has something to repeat.
        let empty = Pcm::new(Vec::new());
        assert_eq!(empty.samples.len(), 1);
        assert_eq!(empty.decoder().collect::<Vec<_>>(), [0.0]);
    }

    #[test]
    fn volume_steps_stay_within_their_range() {
        let mut volume = VolumeSettings {
            master: 10,
            music: 0,
            effects: 5,
            muted: false,
        };
        volume.step(Channel::Master, true);
        volume.step(Channel::Music, false);
        assert_eq!((volume.master, volume.music), (VOLUME_STEPS, 0));
        assert_eq!(volume.gain(Channel::Music), 0.0);
        assert_eq!(volume.gain(Channel::Effects), 0.5);
        assert_eq!(volume.gain(Channel::Master), 1.0);
        volume.step(Channel::Effects, true);
        assert_eq!(volume.level(Channel::Effects), 6);
        volume.step(Channel::Master, false);
        assert_eq!(volume.gain(Channel::Effects), 0.9 * 0.6);
        volume.muted = true;
        for channel in [Channel::Master, Channel::Music, Channel::Effects] {
            assert_eq!(volume.gain(channel), 0.0);
        }
        // A level beyond the range counts as full volume.
        let loud = VolumeSettings {
            master: 200,
            music: 200,
            effects: 0,
            muted: false,
        };
        assert_eq!(loud.gain(Channel::Music), 1.0);
    }

    #[test]
    fn events_play_only_what_the_human_player_may_know() {
        let enemy = PlayerId(1);
        let seen_id = SimId(1);
        let unseen_id = SimId(2);
        let seen = |id: SimId| id == seen_id;
        let shown = |id: SimId| id == seen_id;
        let instant = |weapon: WeaponKind| weapon.0 == 0;
        let context = SoundContext {
            seen: &seen,
            shown: &shown,
            instant: &instant,
        };
        let sound = |event: SimEvent| event_sound(&event, &context);
        let item = ProductionItem::Building(BuildingKind(0));
        for player in [HUMAN, enemy] {
            let own = |sfx| (player == HUMAN).then_some(sfx);
            assert_eq!(
                sound(SimEvent::CommandRejected {
                    player,
                    command: Command::Stop { units: vec![] },
                    reason: RejectReason::NoUnits
                }),
                own(Sfx::Error)
            );
            assert_eq!(
                sound(SimEvent::ProductionComplete { player, item }),
                own(Sfx::Ready)
            );
            assert_eq!(
                sound(SimEvent::BuildingPlaced {
                    player,
                    building: unseen_id,
                    kind: BuildingKind(0)
                }),
                own(Sfx::Placed)
            );
            assert_eq!(
                sound(SimEvent::BuildingSold {
                    player,
                    building: unseen_id,
                    refund: 1
                }),
                own(Sfx::Sold)
            );
            assert_eq!(
                sound(SimEvent::OreDelivered {
                    player,
                    harvester: unseen_id,
                    refinery: unseen_id,
                    ore: 1,
                    credits: 1
                }),
                own(Sfx::Credits)
            );
            // Destruction: own always; an enemy only if its view was shown.
            assert_eq!(
                sound(SimEvent::UnitDestroyed {
                    player,
                    unit: unseen_id
                }),
                own(Sfx::Explosion)
            );
            assert_eq!(
                sound(SimEvent::BuildingDestroyed {
                    player,
                    building: unseen_id,
                    kind: BuildingKind(0),
                    lost_units: vec![]
                }),
                own(Sfx::Explosion)
            );
            assert_eq!(
                sound(SimEvent::UnitDestroyed {
                    player,
                    unit: seen_id
                }),
                Some(Sfx::Explosion)
            );
        }
        // Shots: when the attacker or the target is seen, by weapon kind; never otherwise.
        let fired = |attacker, target, weapon| SimEvent::WeaponFired {
            attacker,
            target,
            weapon: WeaponKind(weapon),
        };
        assert_eq!(sound(fired(seen_id, unseen_id, 0)), Some(Sfx::Shot));
        assert_eq!(sound(fired(unseen_id, seen_id, 1)), Some(Sfx::Cannon));
        assert_eq!(sound(fired(unseen_id, unseen_id, 0)), None);
        // Events without a sound.
        for event in [
            SimEvent::MoveCompleted {
                unit: seen_id,
                retargeted: false,
            },
            SimEvent::MoveFailed {
                unit: seen_id,
                reason: MoveFailure::Blocked,
            },
            SimEvent::ProductionCancelled {
                player: HUMAN,
                item,
                refund: 0,
            },
            SimEvent::UnitSpawned {
                player: HUMAN,
                unit: seen_id,
                building: seen_id,
            },
            SimEvent::HarvesterIdle {
                unit: seen_id,
                reason: HarvestIdle::NoOre,
            },
            SimEvent::ProjectileExpired {
                attacker: seen_id,
                target: seen_id,
                weapon: WeaponKind(0),
            },
            SimEvent::AttackEnded {
                unit: seen_id,
                target: seen_id,
                reason: AttackEnd::TargetGone,
            },
        ] {
            assert_eq!(sound(event.clone()), None, "{event:?}");
        }
        let _ = UnitKind(0);
    }

    #[test]
    fn effects_are_limited_in_number_and_repetition() {
        assert!(may_play(None, 0.0, 0));
        assert!(may_play(None, 0.0, MAX_ACTIVE_EFFECTS - 1));
        assert!(!may_play(None, 0.0, MAX_ACTIVE_EFFECTS));
        assert!(!may_play(Some(1.0), 1.0 + MIN_REPEAT_SECS / 2.0, 0));
        assert!(may_play(Some(1.0), 1.0 + MIN_REPEAT_SECS, 0));
    }
}
