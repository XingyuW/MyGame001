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
//! A shot sounds like the weapon that fired it (`crate::weapon_fx::WeaponClass`): a rifle cracks,
//! a machine gun rattles off a burst, a tank gun booms and rolls away, a rocket roars off, the
//! prism rings like struck glass as it discharges, and a Tesla weapon snaps and buzzes at twice
//! the mains frequency, as an electric arc does. A shell that the player sees land is heard
//! landing ([`Sfx::Impact`]). A soldier driven over by a vehicle is heard as a dull crunch
//! ([`Sfx::Crush`], RA3) instead of the bang of a destruction. A promotion of one of the human
//! player's units pings ([`Sfx::Promotion`], RA4).
//!
//! At most [`MAX_ACTIVE_EFFECTS`] effects play at once, the same effect at most once per
//! [`MIN_REPEAT_SECS`], and every effect entity is removed when its sound has ended, also when
//! the machine has no audio device (Bevy then never starts the sound), so effects cannot
//! accumulate.

use std::collections::BTreeMap;
use std::f32::consts::{PI, TAU};
use std::sync::Arc;
use std::time::Duration;

use bevy::audio::{AddAudioSource, ChannelCount, Sample, SampleRate, Source, Volume};
use bevy::prelude::*;
use sim::{SimEvent, SimId, WeaponKind};

use crate::buildings::{BuildingView, BuildingViews};
use crate::sim_host::{CommandSink, SimHost};
use crate::units::{HUMAN, UnitView, UnitViews};
use crate::weapon_fx::{ImpactSounds, WeaponClass};

/// Sample rate of every sound.
pub const SAMPLE_RATE_HZ: u32 = 22_050;

const RATE: SampleRate = SampleRate::MIN.saturating_add(SAMPLE_RATE_HZ - 1);

/// Most effects that play at once.
pub const MAX_ACTIVE_EFFECTS: usize = 14;

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
            .add_systems(Startup, setup_audio.after(crate::skirmish::load_catalogue))
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
    /// A rifle or pistol fired where the human player sees it.
    Shot,
    /// A machine gun fired a burst.
    Burst,
    /// A sniper's rifle fired: one heavy round and its echo.
    Sniper,
    /// A tank or naval gun fired.
    Cannon,
    /// A howitzer fired.
    Artillery,
    /// A rocket or missile left its launcher.
    Rocket,
    /// A grenade was lobbed.
    Grenade,
    /// An anti-aircraft gun fired.
    Flak,
    /// A prism weapon discharged its beam of light.
    Prism,
    /// A Tesla weapon struck with lightning.
    Tesla,
    /// A radiation projector fired.
    Radiation,
    /// A psychic took control.
    Psychic,
    /// A chrono weapon fired.
    Chrono,
    /// A dolphin's sonic pulse.
    Sonic,
    /// A bomb fell from an aircraft.
    BombFall,
    /// Teeth or claws.
    Bite,
    /// A detonator was armed.
    Trigger,
    /// A shell, rocket or bomb landed where the human player sees it.
    Impact,
    /// A vehicle drove over a soldier (RA3).
    Crush,
    /// A unit of the human player rose in rank (RA4).
    Promotion,
    /// A unit or building the human player was shown was destroyed.
    Explosion,
}

impl Sfx {
    pub const ALL: [Sfx; 27] = [
        Sfx::Acknowledge,
        Sfx::Error,
        Sfx::Ready,
        Sfx::Placed,
        Sfx::Sold,
        Sfx::Credits,
        Sfx::Shot,
        Sfx::Burst,
        Sfx::Sniper,
        Sfx::Cannon,
        Sfx::Artillery,
        Sfx::Rocket,
        Sfx::Grenade,
        Sfx::Flak,
        Sfx::Prism,
        Sfx::Tesla,
        Sfx::Radiation,
        Sfx::Psychic,
        Sfx::Chrono,
        Sfx::Sonic,
        Sfx::BombFall,
        Sfx::Bite,
        Sfx::Trigger,
        Sfx::Impact,
        Sfx::Crush,
        Sfx::Promotion,
        Sfx::Explosion,
    ];

    /// The sounds of weapons firing and of what they fire landing.
    #[cfg(test)]
    pub const WEAPONS: [Sfx; 18] = [
        Sfx::Shot,
        Sfx::Burst,
        Sfx::Sniper,
        Sfx::Cannon,
        Sfx::Artillery,
        Sfx::Rocket,
        Sfx::Grenade,
        Sfx::Flak,
        Sfx::Prism,
        Sfx::Tesla,
        Sfx::Radiation,
        Sfx::Psychic,
        Sfx::Chrono,
        Sfx::Sonic,
        Sfx::BombFall,
        Sfx::Bite,
        Sfx::Trigger,
        Sfx::Impact,
    ];

    /// The sound of a weapon of `class` firing.
    pub fn of(class: WeaponClass) -> Sfx {
        match class {
            WeaponClass::Rifle => Sfx::Shot,
            WeaponClass::MachineGun => Sfx::Burst,
            WeaponClass::Sniper => Sfx::Sniper,
            WeaponClass::Cannon => Sfx::Cannon,
            WeaponClass::Artillery => Sfx::Artillery,
            WeaponClass::Rocket | WeaponClass::Torpedo => Sfx::Rocket,
            WeaponClass::Grenade => Sfx::Grenade,
            WeaponClass::Flak => Sfx::Flak,
            WeaponClass::Prism => Sfx::Prism,
            WeaponClass::Tesla => Sfx::Tesla,
            WeaponClass::Radiation => Sfx::Radiation,
            WeaponClass::Psychic => Sfx::Psychic,
            WeaponClass::Chrono => Sfx::Chrono,
            WeaponClass::Sonic => Sfx::Sonic,
            WeaponClass::Bomb => Sfx::BombFall,
            WeaponClass::Melee => Sfx::Bite,
            WeaponClass::Trigger => Sfx::Trigger,
        }
    }

    /// How loud the sound is at its peak, against the others: a rifle is not as loud as a tank
    /// gun, and what repeats often is kept below what happens once.
    fn peak(self) -> f32 {
        match self {
            Sfx::Cannon | Sfx::Artillery | Sfx::Explosion => 0.9,
            Sfx::Impact | Sfx::Tesla | Sfx::Rocket | Sfx::Sniper => 0.8,
            Sfx::Prism | Sfx::Flak => 0.72,
            Sfx::Shot | Sfx::Burst | Sfx::Grenade | Sfx::BombFall => 0.62,
            Sfx::Radiation | Sfx::Psychic | Sfx::Chrono | Sfx::Sonic | Sfx::Bite => 0.55,
            Sfx::Crush => 0.6,
            Sfx::Promotion => 0.5,
            Sfx::Trigger => 0.4,
            _ => 0.8,
        }
    }
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

/// A noise seed for a sound starting at sample `start` (never zero, which xorshift cannot leave).
fn seed(start: usize, salt: u32) -> u32 {
    ((start as u32).wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B)) | 1
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
        Wave::Sine => (t * TAU).sin(),
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

/// The sample rate as a float.
const SR: f32 = SAMPLE_RATE_HZ as f32;

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
        phase += frequency / SR;
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

// --- Building blocks --------------------------------------------------------------------------

/// A band-limited sawtooth (PolyBLEP), free of the aliasing of a naive ramp.
#[derive(Clone, Copy)]
struct Saw {
    phase: f32,
}

impl Saw {
    fn new(phase: f32) -> Self {
        Self { phase }
    }

    fn next(&mut self, frequency: f32) -> f32 {
        let dt = (frequency / SR).min(0.5);
        let t = self.phase;
        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        let blep = if t < dt {
            let x = t / dt;
            x + x - x * x - 1.0
        } else if t > 1.0 - dt {
            let x = (t - 1.0) / dt;
            x * x + x + x + 1.0
        } else {
            0.0
        };
        2.0 * t - 1.0 - blep
    }
}

/// A sine oscillator whose frequency may change every sample.
#[derive(Clone, Copy, Default)]
struct Sine {
    phase: f32,
}

impl Sine {
    fn next(&mut self, frequency: f32) -> f32 {
        let out = (self.phase * TAU).sin();
        self.phase = (self.phase + frequency / SR).fract();
        out
    }
}

/// A state-variable filter (trapezoidal, stable under fast cutoff changes).
#[derive(Clone, Copy, Default)]
struct Svf {
    ic1: f32,
    ic2: f32,
}

/// The outputs of one [`Svf`] step.
struct Bands {
    low: f32,
    band: f32,
    high: f32,
}

impl Svf {
    /// Filters `x` at `cutoff` Hz with resonance `q` (0.5: none, 0.7: flat, higher: peaked).
    fn process(&mut self, x: f32, cutoff: f32, q: f32) -> Bands {
        let g = (PI * cutoff.clamp(20.0, SR * 0.45) / SR).tan();
        let k = 1.0 / q;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = x - self.ic2;
        let v1 = a1 * self.ic1 + a2 * v3;
        let v2 = self.ic2 + a2 * self.ic1 + a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        Bands {
            low: v2,
            band: v1,
            high: x - k * v1 - v2,
        }
    }
}

/// An envelope that rises linearly over `attack` seconds, holds until `gate`, and falls
/// linearly to zero over `release`.
fn gate_env(t: f32, attack: f32, gate: f32, release: f32) -> f32 {
    if t < attack {
        t / attack
    } else if t < gate {
        1.0
    } else {
        (1.0 - (t - gate) / release).max(0.0)
    }
}

/// 1 until `fade` seconds before `length`, then falling linearly to 0 at `length`, so that a
/// decaying sound always ends in silence.
fn fade_out(t: f32, length: f32, fade: f32) -> f32 {
    ((length - t) / fade).clamp(0.0, 1.0)
}

/// A 0.3 ms rise, so that even a sharp transient starts from silence.
fn onset(t: f32) -> f32 {
    (t / 0.0003).min(1.0)
}

/// Removes offset and sub-sonic rumble below about `cutoff` Hz (a one-pole high-pass).
fn high_pass(buffer: &mut [f32], cutoff: f32) {
    let coefficient = (-TAU * cutoff / SR).exp();
    let (mut previous_in, mut previous_out) = (0.0f32, 0.0f32);
    for sample in buffer {
        let out = coefficient * (previous_out + *sample - previous_in);
        previous_in = *sample;
        previous_out = out;
        *sample = out;
    }
}

/// Fills `buffer` from its start with `voice(t)`, `t` in seconds.
fn render(buffer: &mut [f32], mut voice: impl FnMut(f32) -> f32) {
    for (i, slot) in buffer.iter_mut().enumerate() {
        *slot += voice(i as f32 / SR);
    }
}

// --- Effects ----------------------------------------------------------------------------------

/// The samples of `sfx`.
pub fn synthesize(sfx: Sfx) -> Vec<Sample> {
    let seconds = match sfx {
        Sfx::Acknowledge => 0.06,
        Sfx::Error => 0.25,
        Sfx::Ready => 0.45,
        Sfx::Placed => 0.3,
        Sfx::Sold => 0.35,
        Sfx::Credits => 0.2,
        Sfx::Shot => SHOT_SECS,
        Sfx::Burst => 0.55,
        Sfx::Sniper => 0.95,
        Sfx::Cannon => CANNON_SECS,
        Sfx::Artillery => 1.4,
        Sfx::Rocket => 0.95,
        Sfx::Grenade => 0.3,
        Sfx::Flak => 0.6,
        Sfx::Prism => PRISM_SECS,
        Sfx::Tesla => TESLA_SECS,
        Sfx::Radiation => 0.65,
        Sfx::Psychic => 0.9,
        Sfx::Chrono => 0.7,
        Sfx::Sonic => 0.5,
        Sfx::BombFall => 0.6,
        Sfx::Bite => 0.34,
        Sfx::Trigger => 0.25,
        Sfx::Impact => 0.8,
        Sfx::Crush => CRUSH_SECS,
        Sfx::Promotion => 0.42,
        Sfx::Explosion => EXPLOSION_SECS,
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
        Sfx::Shot => shot(&mut buffer),
        Sfx::Burst => burst(&mut buffer),
        Sfx::Sniper => sniper(&mut buffer),
        Sfx::Cannon => big_gun(&mut buffer, 1.0, 13),
        Sfx::Artillery => big_gun(&mut buffer, 1.35, 29),
        Sfx::Rocket => rocket(&mut buffer),
        Sfx::Grenade => grenade(&mut buffer),
        Sfx::Flak => flak(&mut buffer),
        Sfx::Prism => prism(&mut buffer),
        Sfx::Tesla => tesla(&mut buffer),
        Sfx::Radiation => radiation(&mut buffer),
        Sfx::Psychic => psychic(&mut buffer),
        Sfx::Chrono => chrono(&mut buffer),
        Sfx::Sonic => sonic(&mut buffer),
        Sfx::BombFall => bomb_fall(&mut buffer),
        Sfx::Bite => bite(&mut buffer),
        Sfx::Trigger => trigger(&mut buffer),
        Sfx::Impact => impact(&mut buffer),
        Sfx::Crush => crush(&mut buffer),
        // Two bright notes a fourth apart, the second ringing on: a ping, unlike the three
        // rising notes of a finished production.
        Sfx::Promotion => {
            for (index, frequency) in [1174.66, 1567.98].into_iter().enumerate() {
                let start = samples_for(0.09 * index as f32);
                add_note(
                    &mut buffer,
                    start,
                    0.14 + 0.18 * index as f32,
                    Wave::Sine,
                    frequency,
                    frequency,
                    0.45,
                    true,
                );
            }
        }
        Sfx::Explosion => explosion(&mut buffer),
    }
    let length = buffer.len() as f32 / SR;
    for (i, sample) in buffer.iter_mut().enumerate() {
        *sample *= fade_out(i as f32 / SR, length, 0.02);
    }
    normalise(buffer, sfx.peak())
}

const SHOT_SECS: f32 = 0.42;
const CANNON_SECS: f32 = 1.1;
const PRISM_SECS: f32 = 0.95;
const TESLA_SECS: f32 = 0.78;
const EXPLOSION_SECS: f32 = 1.4;

/// Adds to `buffer` what it sounds like coming back from the surroundings: a copy dulled below
/// `cutoff` Hz, once for every tap of (delay in seconds, gain). A gun fired in the open is
/// heard as its crack and then its echo off the ground and whatever stands around.
fn reflect(buffer: &mut [f32], taps: &[(f32, f32)], cutoff: f32) {
    let mut dull = Svf::default();
    let wet: Vec<f32> = buffer
        .iter()
        .map(|sample| dull.process(*sample, cutoff, 0.6).low)
        .collect();
    for &(delay, gain) in taps {
        let shift = samples_for(delay);
        for (i, sample) in wet.iter().enumerate() {
            let Some(slot) = buffer.get_mut(i + shift) else {
                break;
            };
            *slot += sample * gain;
        }
    }
}

/// Adds one round of small-arms fire to `buffer` from `start` seconds. A gunshot is not a tone:
/// it is the muzzle blast, a broadband crack lasting a few milliseconds, with the thump of the
/// weapon behind it. `body` scales the weapon: below 1 a carbine, above 1 a heavy rifle.
fn gunshot(buffer: &mut [f32], start: f32, body: f32, seed: u32) {
    let mut noise = Noise(seed.max(1));
    let (mut crack, mut chest, mut room) = (Svf::default(), Svf::default(), Svf::default());
    let mut thump = Sine::default();
    let first = samples_for(start);
    for (i, slot) in buffer.iter_mut().enumerate().skip(first) {
        let t = (i - first) as f32 / SR;
        if t > 0.5 * body {
            break;
        }
        let n = noise.next();
        let click = n * (-t / 0.0007).exp() * 0.7;
        let snap =
            crack.process(n, 2_700.0 / body.sqrt(), 0.7).band * (-t / (0.013 * body)).exp() * 2.6;
        let bark = chest.process(n, 680.0 / body, 1.1).band * (-t / (0.038 * body)).exp() * 1.1;
        let kick = thump.next(95.0 / body + 120.0 * (-t / 0.011).exp())
            * (-t / (0.034 * body)).exp()
            * 0.5
            * body;
        let air = room.process(n, 1_500.0, 0.7).low * (-t / (0.085 * body)).exp() * 0.5;
        *slot += (1.5 * (click + snap + bark + kick + air)).tanh() * onset(t);
    }
}

/// A rifle shot in the open.
fn shot(buffer: &mut [f32]) {
    gunshot(buffer, 0.0, 1.0, 11);
    reflect(
        buffer,
        &[(0.075, 0.22), (0.16, 0.12), (0.26, 0.06)],
        1_400.0,
    );
    high_pass(buffer, 40.0);
}

/// A machine gun: four rounds at about 800 a minute, no two quite alike.
fn burst(buffer: &mut [f32]) {
    for (round, body) in [0.86, 0.8, 0.9, 0.83].into_iter().enumerate() {
        gunshot(buffer, 0.076 * round as f32, body, 31 + 7 * round as u32);
    }
    reflect(buffer, &[(0.09, 0.2), (0.2, 0.1)], 1_300.0);
    high_pass(buffer, 45.0);
}

/// A heavy rifle at long range: the round, then its echo rolling back for most of a second.
fn sniper(buffer: &mut [f32]) {
    gunshot(buffer, 0.0, 1.7, 53);
    reflect(
        buffer,
        &[
            (0.11, 0.34),
            (0.23, 0.24),
            (0.38, 0.16),
            (0.55, 0.1),
            (0.72, 0.05),
        ],
        900.0,
    );
    high_pass(buffer, 35.0);
}

/// A large-calibre gun: the crack of the muzzle, a boom that falls below hearing, the blast
/// darkening as it spreads, and its echo rolling back. `calibre` scales it (1: a tank gun).
fn big_gun(buffer: &mut [f32], calibre: f32, seed: u32) {
    let mut noise = Noise(seed);
    let mut low_noise = Noise(seed + 2);
    let (mut crack, mut blast, mut rumble) = (Svf::default(), Svf::default(), Svf::default());
    let mut boom = Sine::default();
    render(buffer, |t| {
        let n = noise.next();
        let click = n * (-t / 0.001).exp() * 0.9;
        let snap =
            crack.process(n, 1_900.0 / calibre, 0.7).band * (-t / (0.03 * calibre)).exp() * 2.9;
        let thud = boom.next((42.0 + 110.0 * (-t / 0.045).exp()) / calibre)
            * (1.0 - (-t / 0.003).exp())
            * (-t / (0.24 * calibre)).exp()
            * 1.05;
        // The blast is what carries on small speakers: it starts as bright as the crack and
        // darkens to a rumble in a quarter of a second.
        let darkening = 230.0 + 3_000.0 * (-t / (0.08 * calibre)).exp();
        let burst = blast.process(n, darkening, 0.8).low * (-t / (0.2 * calibre)).exp() * 2.3;
        let roll = rumble.process(low_noise.next(), 160.0, 0.7).low
            * (1.0 - (-t / 0.05).exp())
            * (-t / (0.4 * calibre)).exp()
            * 0.55;
        (1.6 * (click + snap + thud + burst + roll)).tanh() * onset(t)
    });
    reflect(buffer, &[(0.14, 0.26), (0.29, 0.16), (0.47, 0.09)], 500.0);
    high_pass(buffer, 28.0);
}

/// A rocket leaving its launcher: the motor lights with a bang and roars away, its pitch
/// falling as it goes.
fn rocket(buffer: &mut [f32]) {
    let mut noise = Noise(61);
    let (mut bang, mut roar, mut hiss) = (Svf::default(), Svf::default(), Svf::default());
    let mut thump = Sine::default();
    render(buffer, |t| {
        let n = noise.next();
        let light = bang.process(n, 1_500.0, 0.8).band * (-t / 0.03).exp() * 1.8
            + thump.next(80.0 + 70.0 * (-t / 0.03).exp()) * (-t / 0.09).exp() * 0.7;
        // The motor's note rises as it lights and then falls away with distance.
        let pitch = 520.0 + 1_500.0 * (1.0 - (-t / 0.05).exp()) * (-t / 0.4).exp();
        let burn = (1.0 - (-t / 0.03).exp()) * (-t / 0.3).exp();
        let flutter = 1.0 + 0.3 * (TAU * 41.0 * t).sin();
        let engine = roar.process(n, pitch, 1.3).band * burn * flutter * 2.2;
        let jet = hiss.process(n, 4_200.0, 0.7).high * burn * burn * 0.35;
        (1.4 * (light + engine + jet)).tanh() * onset(t)
    });
    high_pass(buffer, 40.0);
}

/// A grenade launcher: a hollow thump.
fn grenade(buffer: &mut [f32]) {
    let mut noise = Noise(67);
    let mut puff = Svf::default();
    let mut tube = Sine::default();
    render(buffer, |t| {
        let n = noise.next();
        let pop = tube.next(95.0 + 210.0 * (-t / 0.02).exp()) * (-t / 0.06).exp() * 0.9;
        let gas = puff.process(n, 1_500.0, 0.8).low * (-t / 0.04).exp() * 2.0;
        (1.3 * (pop + gas + n * (-t / 0.001).exp() * 0.3)).tanh() * onset(t)
    });
    high_pass(buffer, 40.0);
}

/// An automatic anti-aircraft gun: three heavy rounds, with the ring of the breech.
fn flak(buffer: &mut [f32]) {
    for round in 0..3 {
        gunshot(buffer, 0.12 * round as f32, 1.45, 71 + 5 * round);
    }
    let mut ring = Sine::default();
    for (i, slot) in buffer.iter_mut().enumerate() {
        let t = (i as f32 / SR) % 0.12;
        *slot += ring.next(1_870.0) * (-t / 0.03).exp() * 0.1;
    }
    reflect(buffer, &[(0.13, 0.2), (0.27, 0.1)], 1_000.0);
    high_pass(buffer, 40.0);
}

/// A prism weapon. Light makes no sound, so this is what makes it: the capacitors charge with a
/// rising whine, then the crystal rings as the beam leaves it, a chord of pure high partials
/// that are not quite harmonic, as a struck glass or bell is, shimmering as they beat against
/// each other, over the sizzle of heated air.
fn prism(buffer: &mut [f32]) {
    const CHARGE: f32 = 0.13;
    // The partials of the crystal (Hz), each with a companion a few hertz away: the beating
    // between the pairs is the shimmer.
    const PARTIALS: [(f32, f32); 5] = [
        (1_760.0, 1.0),
        (2_637.0, 0.8),
        (3_520.0, 0.62),
        (4_434.0, 0.42),
        (5_588.0, 0.3),
    ];
    let mut noise = Noise(83);
    let mut sizzle = Svf::default();
    let mut whine = Sine::default();
    let mut zap = Sine::default();
    let mut thud = Sine::default();
    let mut ring = [Sine::default(); 10];
    render(buffer, |t| {
        let n = noise.next();
        if t < CHARGE {
            let rise = t / CHARGE;
            let mut sum = whine.next(500.0 + 2_300.0 * rise * rise) * rise * 0.3;
            for (i, (partial, level)) in PARTIALS.iter().enumerate().take(3) {
                sum +=
                    ring[2 * i].next(partial * (0.92 + 0.08 * rise)) * rise * rise * level * 0.16;
            }
            return sum * onset(t);
        }
        let u = t - CHARGE;
        // The discharge itself: a tone that drops two octaves in a tenth of a second.
        let drop = zap.next(1_100.0 + 3_900.0 * (-u / 0.06).exp()) * (-u / 0.13).exp() * 0.7;
        let power = thud.next(98.0) * (-u / 0.1).exp() * 0.45;
        let mut glass = 0.0;
        for (i, (partial, level)) in PARTIALS.iter().enumerate() {
            let decay = (-u / (0.3 - 0.04 * i as f32)).exp();
            glass += (ring[2 * i].next(*partial)
                + ring[2 * i + 1].next(partial + 5.0 + 3.0 * i as f32))
                * level
                * decay
                * 0.2;
        }
        let tremble = 1.0 + 0.35 * (TAU * 23.0 * u).sin() * (-u / 0.25).exp();
        let air = sizzle.process(n, 5_200.0, 0.8).high * (-u / 0.08).exp() * 0.4;
        (1.15 * (drop + power + glass * tremble + air)).tanh()
    });
    reflect(buffer, &[(0.09, 0.16), (0.19, 0.08)], 5_000.0);
    high_pass(buffer, 60.0);
}

/// A Tesla weapon. An electric arc is a crack as the air breaks down, then a harsh buzz: the
/// arc re-strikes on every half cycle of its supply, so its hiss is chopped at twice the supply's
/// frequency, over the hum of the transformer, and it spits as the channel wanders.
fn tesla(buffer: &mut [f32]) {
    const SUPPLY: f32 = 55.0;
    const HOLD: f32 = 0.36;
    let mut noise = Noise(97);
    let mut gate_noise = Noise(101);
    let (mut snap, mut hiss_low, mut hiss_high, mut nasal, mut spit) = (
        Svf::default(),
        Svf::default(),
        Svf::default(),
        Svf::default(),
        Svf::default(),
    );
    let mut hum = [Saw::new(0.0), Saw::new(0.37)];
    let (mut gate, mut target, mut until) = (1.0f32, 1.0f32, 0.0f32);
    render(buffer, |t| {
        let n = noise.next();
        let breakdown = n * (-t / 0.0006).exp() * 1.1
            + snap.process(n, 3_300.0, 0.7).band * (-t / 0.011).exp() * 2.2;
        // The arc gutters: its strength jumps to a new level every few milliseconds.
        if t >= until {
            until = t + 0.004 + 0.012 * gate_noise.next().abs();
            target = 0.3 + 0.7 * gate_noise.next().abs();
        }
        gate += (target - gate) * 0.02;
        let strike = (TAU * SUPPLY * t).sin().abs().powi(3);
        let hiss =
            hiss_low.process(n, 3_400.0, 0.9).band + hiss_high.process(n, 6_300.0, 1.2).band * 0.7;
        let transformer = hum[0].next(2.0 * SUPPLY) * 0.6 + hum[1].next(4.0 * SUPPLY + 1.3) * 0.4;
        let buzz = transformer * 0.35 + nasal.process(transformer, 950.0, 0.9).band * 0.6;
        let pop = if gate_noise.next().abs() > 0.9965 {
            gate_noise.next() * 7.0
        } else {
            0.0
        };
        let spitting = spit.process(pop, 2_600.0, 3.0).band;
        let hold = if t < HOLD {
            1.0
        } else {
            (-(t - HOLD) / 0.1).exp()
        };
        (1.8 * (breakdown
            + hold * gate * (buzz * (1.0 + 1.6 * strike) + hiss * strike * 1.1)
            + spitting * hold.sqrt()))
        .tanh()
            * onset(t)
    });
    high_pass(buffer, 50.0);
}

/// A radiation projector: a hiss, a low drone, and the clicks of a counter going wild.
fn radiation(buffer: &mut [f32]) {
    let mut noise = Noise(103);
    let mut clicks = Noise(107);
    let (mut hiss, mut tick) = (Svf::default(), Svf::default());
    let mut drone = Sine::default();
    render(buffer, |t| {
        let n = noise.next();
        let level = (t / 0.04).min(1.0) * (-t / 0.3).exp();
        let air = hiss.process(n, 5_000.0, 0.8).band * level * 0.5;
        let hum = drone.next(72.0 + 4.0 * (TAU * 9.0 * t).sin()) * level * 0.5;
        let pulse = if clicks.next().abs() > 1.0 - 0.02 * level {
            clicks.next() * 5.0
        } else {
            0.0
        };
        (1.3 * (air + hum + tick.process(pulse, 3_100.0, 4.0).band)).tanh()
    });
    high_pass(buffer, 45.0);
}

/// A psychic's grip: two voices a few hertz apart, swelling and sliding upwards.
fn psychic(buffer: &mut [f32]) {
    const LENGTH: f32 = 0.9;
    let mut noise = Noise(109);
    let mut breath = Svf::default();
    let mut voices = [Sine::default(); 3];
    render(buffer, |t| {
        let swell = (PI * t / LENGTH).sin().max(0.0).powf(0.7);
        let glide = 1.0 + 0.07 * t / LENGTH + 0.012 * (TAU * 5.5 * t).sin();
        let tone = voices[0].next(392.0 * glide)
            + voices[1].next(399.0 * glide)
            + voices[2].next(784.0 * glide) * 0.35;
        let air = breath
            .process(noise.next(), 1_900.0 + 900.0 * (TAU * 3.0 * t).sin(), 3.0)
            .band
            * 0.25;
        (tone * 0.4 + air) * swell
    });
    reflect(buffer, &[(0.11, 0.25), (0.23, 0.14)], 2_500.0);
}

/// A chrono weapon: air rushing in, rising to a snap.
fn chrono(buffer: &mut [f32]) {
    const SNAP: f32 = 0.52;
    let mut noise = Noise(113);
    let (mut rush, mut crack) = (Svf::default(), Svf::default());
    let mut tones = [Sine::default(); 2];
    render(buffer, |t| {
        let n = noise.next();
        if t < SNAP {
            let rise = t / SNAP;
            let wind = rush.process(n, 380.0 * 16.0f32.powf(rise), 2.2).band * rise * rise * 1.4;
            let whine = (tones[0].next(660.0 * (1.0 + rise)) + tones[1].next(993.0 * (1.0 + rise)))
                * rise.powi(3)
                * 0.25;
            wind + whine
        } else {
            let u = t - SNAP;
            n * (-u / 0.001).exp() * 0.8
                + crack.process(n, 2_400.0, 0.8).band * (-u / 0.03).exp() * 1.6
        }
    });
    reflect(buffer, &[(0.08, 0.2)], 3_000.0);
    high_pass(buffer, 60.0);
}

/// A dolphin's pulse: two rising chirps and the ping coming back.
fn sonic(buffer: &mut [f32]) {
    let mut voice = Sine::default();
    render(buffer, |t| {
        let chirp = |start: f32, length: f32| {
            let u = (t - start) / length;
            if (0.0..1.0).contains(&u) {
                Some((1_700.0 + 2_900.0 * u * u, (PI * u).sin()))
            } else {
                None
            }
        };
        match chirp(0.0, 0.11).or(chirp(0.14, 0.11)) {
            Some((pitch, level)) => voice.next(pitch) * level,
            None if t >= 0.3 => voice.next(3_300.0) * (-(t - 0.3) / 0.05).exp() * 0.5,
            None => 0.0,
        }
    });
    reflect(buffer, &[(0.06, 0.25)], 4_000.0);
}

/// A bomb on its way down: the whistle that falls in pitch and grows as it nears.
fn bomb_fall(buffer: &mut [f32]) {
    const LENGTH: f32 = 0.6;
    let mut noise = Noise(127);
    let mut wind = Svf::default();
    let mut whistle = Sine::default();
    render(buffer, |t| {
        let fall = t / LENGTH;
        let pitch = 1_950.0 - 1_150.0 * fall.powf(1.2);
        let near = fall.powf(1.5);
        let tone = whistle.next(pitch * (1.0 + 0.006 * (TAU * 13.0 * t).sin())) * near * 0.7;
        tone + wind.process(noise.next(), pitch, 6.0).band * near * 0.6
    });
}

/// A dog: two short barks out of a growl.
fn bite(buffer: &mut [f32]) {
    let mut noise = Noise(131);
    let (mut throat, mut mouth, mut rasp) = (Svf::default(), Svf::default(), Svf::default());
    let mut cords = Saw::new(0.0);
    render(buffer, |t| {
        let u = t % 0.15;
        let level = (u / 0.008).min(1.0) * (-u / 0.045).exp();
        let voice = cords.next(125.0 + 70.0 * (-u / 0.03).exp());
        let n = noise.next();
        let shaped =
            throat.process(voice, 720.0, 3.0).band + mouth.process(voice, 1_350.0, 4.0).band * 0.7;
        (1.6 * (shaped + rasp.process(n, 2_300.0, 1.0).band * 0.5) * level).tanh()
    });
    high_pass(buffer, 80.0);
}

/// A detonator armed: two beeps.
fn trigger(buffer: &mut [f32]) {
    add_note(buffer, 0, 0.05, Wave::Square, 2_400.0, 2_400.0, 0.5, false);
    add_note(
        buffer,
        samples_for(0.1),
        0.08,
        Wave::Square,
        3_200.0,
        3_200.0,
        0.5,
        false,
    );
}

/// A shell landing: shorter and sharper than a vehicle blowing up.
fn impact(buffer: &mut [f32]) {
    let mut noise = Noise(137);
    let mut debris_noise = Noise(139);
    let (mut crack, mut rumble, mut debris) = (Svf::default(), Svf::default(), Svf::default());
    let mut boom = Sine::default();
    render(buffer, |t| {
        let n = noise.next();
        let snap = crack.process(n, 1_600.0, 0.7).band * (-t / 0.02).exp() * 1.6;
        let thud = boom.next(44.0 + 85.0 * (-t / 0.06).exp())
            * (1.0 - (-t / 0.003).exp())
            * (-t / 0.2).exp();
        let dirt = rumble
            .process(n, 220.0 + 2_600.0 * (-t / 0.09).exp(), 0.9)
            .low
            * (-t / 0.22).exp()
            * 1.5;
        let chance = if t > 0.06 {
            0.005 * (-(t - 0.06) / 0.2).exp()
        } else {
            0.0
        };
        let pebble = if (debris_noise.next() + 1.0) * 0.5 < chance {
            debris_noise.next() * 5.0
        } else {
            0.0
        };
        (1.4 * (snap + thud + dirt + debris.process(pebble, 2_800.0, 2.0).band * 0.8)).tanh()
            * onset(t)
    });
    high_pass(buffer, 30.0);
}

const CRUSH_SECS: f32 = 0.4;

/// A vehicle driving over a soldier: a dull thud under the tracks and a short dry crunch, with
/// no crack and no boom.
fn crush(buffer: &mut [f32]) {
    let mut noise = Noise(149);
    let mut grit_noise = Noise(151);
    let (mut body, mut grit) = (Svf::default(), Svf::default());
    let mut thump = Sine::default();
    render(buffer, |t| {
        let thud = thump.next(58.0 + 40.0 * (-t / 0.03).exp())
            * (1.0 - (-t / 0.004).exp())
            * (-t / 0.09).exp();
        let squash = body.process(noise.next(), 320.0, 1.2).low * (-t / 0.07).exp() * 1.2;
        // The crunch: a handful of dry clicks in the first fifth of a second.
        let chance = 0.03 * (-t / 0.08).exp();
        let click = if (grit_noise.next() + 1.0) * 0.5 < chance {
            grit_noise.next() * 4.0
        } else {
            0.0
        };
        (1.3 * (thud + squash + grit.process(click, 1_900.0, 2.5).band * 0.7)).tanh() * onset(t)
    });
    high_pass(buffer, 35.0);
}

/// A destruction: a crack, a sub-bass boom, a rumble whose brightness falls as it decays, and
/// crackling debris.
fn explosion(buffer: &mut [f32]) {
    let mut noise = Noise(17);
    let mut debris_noise = Noise(19);
    let mut crack = Svf::default();
    let mut rumble = Svf::default();
    let mut debris = Svf::default();
    let mut boom = Sine::default();
    render(buffer, |t| {
        let n = noise.next();
        let crack = crack.process(n, 1_500.0, 0.7).band * (-t / 0.025).exp() * 1.6;
        let rise = 1.0 - (-t / 0.004).exp();
        let low = boom.next(40.0 + 70.0 * (-t / 0.08).exp()) * rise * (-t / 0.3).exp() * 0.7;
        let cutoff = 200.0 + 3_000.0 * (-t / 0.15).exp();
        let rumble = rumble.process(n, cutoff, 0.9).low * (-t / 0.4).exp() * 1.6;
        // Sparse impulses, thinning out, ringing a band-pass: falling debris.
        let chance = if t > 0.08 {
            0.004 * (-(t - 0.08) / 0.35).exp()
        } else {
            0.0
        };
        let roll = debris_noise.next();
        let impulse = if (roll + 1.0) * 0.5 < chance {
            debris_noise.next() * 6.0
        } else {
            0.0
        };
        let debris = debris.process(impulse, 2_500.0, 2.0).band * 0.9;
        (1.3 * (crack + low + rumble + debris)).tanh()
            * onset(t)
            * fade_out(t, EXPLOSION_SECS, 0.05)
    });
    high_pass(buffer, 30.0);
}

// --- Music ------------------------------------------------------------------------------------

/// Samples per sixteenth note of the music: 2,940 at 22,050 Hz, 112.5 beats per minute (a
/// marching tempo), a whole number so that every note starts on an exact sample.
const STEP: usize = 2_940;

/// Samples per bar of the music (sixteen sixteenths).
const BAR: usize = 16 * STEP;

/// Bars of the music loop: the march-in (4), the riff (8), the charge with the lead guitar (8),
/// the breakdown (4), and the riff again at full strength (8).
const MUSIC_BARS: usize = 32;

/// Bars rendered past the end of the loop; their ringing tails are added to its start, so that
/// the loop repeats without a seam.
const MUSIC_TAIL_BARS: usize = 2;

/// MIDI note of D2, the open low string of a guitar tuned down to D; the bass plays an octave
/// below.
const D2: f32 = 38.0;

/// A guitar riff of one bar: (sixteenth, semitones above D, length in sixteenths, palm-muted).
type Riff = [(u8, i8, u8, bool)];

/// The main riff: chugs on the open string under a climb through the minor third and the fourth
/// to the flat fifth.
const RIFF_MARCH: &Riff = &[
    (0, 0, 2, false),
    (2, 0, 1, true),
    (3, 0, 1, true),
    (4, 3, 2, false),
    (6, 0, 1, true),
    (7, 0, 1, true),
    (8, 5, 2, false),
    (10, 0, 1, true),
    (11, 0, 1, true),
    (12, 6, 2, false),
    (14, 5, 1, false),
    (15, 3, 1, false),
];

/// Its answer: the same start, then the flat second held over a run of chugs.
const RIFF_ANSWER: &Riff = &[
    (0, 0, 2, false),
    (2, 0, 1, true),
    (3, 0, 1, true),
    (4, 3, 2, false),
    (6, 0, 1, true),
    (7, 0, 1, true),
    (8, 1, 3, false),
    (11, 0, 1, true),
    (12, 0, 1, true),
    (13, 0, 1, true),
    (14, -2, 2, false),
];

/// The rhythm guitar of the charge over the chord `root`: accents three, three and two eighths
/// apart with chugs between them.
const RIFF_CHARGE: &Riff = &[
    (0, 0, 2, false),
    (2, 0, 1, true),
    (3, 0, 1, true),
    (4, 0, 2, true),
    (6, 0, 2, false),
    (8, 0, 1, true),
    (9, 0, 1, true),
    (10, 0, 2, true),
    (12, 0, 2, false),
    (14, 0, 1, true),
    (15, 0, 1, true),
];

/// The chord root of each bar of the charge, in semitones from D: D, D, B flat, C, D, D, F, G.
const CHARGE_ROOTS: [i8; 8] = [0, 0, -4, -2, 0, 0, 3, 5];

/// The lead guitar of the charge, a bar per row: (sixteenth, MIDI note, length in sixteenths).
const LEAD: [&[(u8, u8, u8)]; 8] = [
    &[(0, 69, 6), (6, 72, 2), (8, 74, 8)],
    &[(0, 72, 4), (4, 69, 4), (8, 65, 4), (12, 69, 4)],
    &[(0, 70, 8), (8, 74, 4), (12, 77, 4)],
    &[(0, 76, 6), (6, 72, 2), (8, 67, 8)],
    &[(0, 69, 6), (6, 72, 2), (8, 74, 8)],
    &[(0, 77, 6), (6, 76, 2), (8, 74, 4), (12, 72, 4)],
    &[(0, 81, 8), (8, 77, 4), (12, 72, 4)],
    &[(0, 79, 4), (4, 74, 4), (8, 73, 8)],
];

/// The sequencer line of the breakdown and the last riff: sixteenths over D minor, as semitones
/// above D4.
const ARPEGGIO: [i8; 16] = [0, 7, 12, 7, 3, 7, 12, 15, 0, 7, 12, 7, 5, 7, 10, 12];

/// The sixteenths of the parade-ground snare cadence, with their velocities.
const CADENCE: [(u8, f32); 9] = [
    (0, 0.8),
    (3, 0.3),
    (4, 0.7),
    (6, 0.35),
    (8, 0.8),
    (10, 0.35),
    (11, 0.3),
    (12, 0.75),
    (14, 0.45),
];

fn midi(note: f32) -> f32 {
    440.0 * ((note - 69.0) / 12.0).exp2()
}

/// The sample at which sixteenth `step` of `bar` starts.
fn at(bar: usize, step: f32) -> usize {
    bar * BAR + (step * STEP as f32).round() as usize
}

/// Seconds of `n` sixteenths.
fn steps(n: f32) -> f32 {
    n * STEP as f32 / SR
}

/// The music's buses: the dry mix, and a send feeding the echo and the reverb.
struct Mix {
    dry: Vec<f32>,
    send: Vec<f32>,
}

impl Mix {
    /// Adds `voice(t)` (`t` in seconds) for `seconds` from sample `start`, sending `send` of it
    /// to the echo and reverb.
    fn voice(&mut self, start: usize, seconds: f32, send: f32, mut voice: impl FnMut(f32) -> f32) {
        let end = (start + samples_for(seconds)).min(self.dry.len());
        let Some(dry) = self.dry.get_mut(start..end) else {
            return;
        };
        let wet = &mut self.send[start..end];
        for (i, (dry, wet)) in dry.iter_mut().zip(wet).enumerate() {
            let sample = voice(i as f32 / SR);
            *dry += sample;
            *wet += sample * send;
        }
    }

    /// A kick drum: a sine falling from about 170 to 48 Hz under a hard beater click, saturated.
    fn kick(&mut self, start: usize, velocity: f32) {
        const SECS: f32 = 0.4;
        let mut body = Sine::default();
        let mut noise = Noise(seed(start, 1));
        let mut click = Svf::default();
        self.voice(start, SECS, 0.0, |t| {
            let thump = body.next(52.0 + 125.0 * (-t / 0.024).exp()) * (-t / 0.085).exp();
            let snap = click.process(noise.next(), 3_500.0, 0.7).low
                * (t / 0.0005).min(1.0)
                * (-t / 0.005).exp()
                * 0.9;
            (2.2 * (thump * 0.8 + snap)).tanh() * 0.62 * velocity * fade_out(t, SECS, 0.03)
        });
    }

    /// A big snare drum: a tuned body, a band of noise for the wires, and a longer burst sent to
    /// the room for the gated-hall crack of an industrial kit.
    fn snare(&mut self, start: usize, velocity: f32) {
        const SECS: f32 = 0.36;
        let mut body = Sine::default();
        let mut overtone = Sine::default();
        let mut noise = Noise(seed(start, 2));
        let mut wires = Svf::default();
        self.voice(start, SECS, 0.3, |t| {
            let tone = (body.next(165.0 + 50.0 * (-t / 0.01).exp()) + 0.5 * overtone.next(318.0))
                * (-t / 0.05).exp();
            let bands = wires.process(noise.next(), 2_600.0, 0.6);
            // Held, then cut: the gate.
            let gate = if t < 0.16 {
                1.0
            } else {
                (-(t - 0.16) / 0.03).exp()
            };
            let rattle = (bands.high * 0.55 + bands.band * 0.9) * (-t / 0.2).exp() * gate;
            let attack = (t / 0.0015).min(1.0);
            (1.7 * (tone * 0.6 + rattle)).tanh() * 0.8 * velocity * attack * fade_out(t, SECS, 0.03)
        });
    }

    /// A hi-hat, closed or open.
    fn hat(&mut self, start: usize, velocity: f32, open: bool) {
        let (secs, decay) = if open { (0.3, 0.08) } else { (0.08, 0.018) };
        let mut noise = Noise(seed(start, 3));
        let mut filter = Svf::default();
        self.voice(start, secs, 0.08, |t| {
            let bands = filter.process(noise.next(), 7_000.0, 1.2);
            (bands.high * 0.5 + bands.band)
                * 0.13
                * velocity
                * (-t / decay).exp()
                * fade_out(t, secs, 0.01)
        });
    }

    /// A crash cymbal.
    fn crash(&mut self, start: usize, velocity: f32) {
        const SECS: f32 = 2.0;
        let mut noise = Noise(seed(start, 4));
        let mut filter = Svf::default();
        self.voice(start, SECS, 0.35, |t| {
            let bands = filter.process(noise.next(), 5_000.0, 0.5);
            let attack = (t / 0.002).min(1.0);
            (bands.band + 0.3 * bands.high)
                * 0.2
                * velocity
                * attack
                * (-t / 0.55).exp()
                * fade_out(t, SECS, 0.1)
        });
    }

    /// A floor tom at `frequency` Hz.
    fn tom(&mut self, start: usize, frequency: f32, velocity: f32) {
        const SECS: f32 = 0.45;
        let mut body = Sine::default();
        let mut noise = Noise(seed(start, 5));
        let mut click = Svf::default();
        self.voice(start, SECS, 0.2, |t| {
            let tone = body.next(frequency * (1.0 + 0.5 * (-t / 0.03).exp())) * (-t / 0.16).exp();
            let snap = click.process(noise.next(), 3_000.0, 0.7).low * (-t / 0.005).exp() * 0.3;
            (1.4 * (tone + snap)).tanh() * 0.55 * velocity * fade_out(t, SECS, 0.03)
        });
    }

    /// The impact under a section's first beat: a long sub-bass drop and a low rumble.
    fn boom(&mut self, start: usize) {
        const SECS: f32 = 2.2;
        let mut body = Sine::default();
        let mut noise = Noise(seed(start, 6));
        let mut filter = Svf::default();
        self.voice(start, SECS, 0.2, |t| {
            let attack = (t / 0.003).min(1.0);
            let tone = body.next(32.0 + 34.0 * (-t / 0.15).exp()) * (-t / 0.7).exp();
            let rumble = filter.process(noise.next(), 300.0, 0.7).low * (-t / 0.4).exp() * 0.8;
            (tone + rumble) * 0.35 * attack * fade_out(t, SECS, 0.2)
        });
    }

    /// A column on the march: one footfall of many boots on gravel. The feet land a few
    /// milliseconds apart, a low thud under the crunch.
    fn boots(&mut self, start: usize, velocity: f32) {
        const SECS: f32 = 0.26;
        const FEET: [(f32, f32); 4] = [(0.0, 1.0), (0.011, 0.75), (0.024, 0.6), (0.041, 0.35)];
        let mut noise = Noise(seed(start, 8));
        let mut crunch = Svf::default();
        let mut thud = Sine::default();
        self.voice(start, SECS, 0.22, |t| {
            let bands = crunch.process(noise.next(), 1_700.0, 0.9);
            let mut gravel = 0.0;
            let mut weight = 0.0;
            for (delay, gain) in FEET {
                let since = t - delay;
                if since >= 0.0 {
                    let rise = (since / 0.002).min(1.0);
                    gravel += gain * rise * (-since / 0.034).exp();
                    weight += gain * rise * (-since / 0.05).exp();
                }
            }
            let body = (bands.low * 1.1 + bands.band * 1.3 + bands.high * 0.25) * gravel;
            let low = thud.next(80.0 + 40.0 * (-t / 0.02).exp()) * weight * 0.22;
            (1.5 * (body + low)).tanh() * 0.6 * velocity * fade_out(t, SECS, 0.03)
        });
    }

    /// A struck girder: inharmonic partials with a hard attack, the anvil of an industrial kit.
    fn clank(&mut self, start: usize, velocity: f32) {
        const SECS: f32 = 0.5;
        const PARTIALS: [(f32, f32, f32); 5] = [
            (587.0, 1.0, 0.16),
            (845.0, 0.8, 0.12),
            (1_234.0, 0.7, 0.09),
            (1_702.0, 0.5, 0.06),
            (2_489.0, 0.35, 0.04),
        ];
        let mut sines = [Sine::default(); 5];
        let mut noise = Noise(seed(start, 9));
        self.voice(start, SECS, 0.45, |t| {
            let ring: f32 = sines
                .iter_mut()
                .zip(PARTIALS)
                .map(|(sine, (frequency, gain, decay))| {
                    sine.next(frequency) * gain * (-t / decay).exp()
                })
                .sum();
            let strike = noise.next() * (-t / 0.002).exp() * 0.8;
            (ring * 0.3 + strike) * 0.2 * velocity * onset(t) * fade_out(t, SECS, 0.05)
        });
    }

    /// A shouted "huh!" from the ranks: a few rough voices falling in pitch through the formants
    /// of an open vowel, with breath.
    fn shout(&mut self, start: usize, velocity: f32) {
        const SECS: f32 = 0.42;
        const VOICES: [f32; 4] = [98.0, 110.0, 124.0, 147.0];
        let mut saws = [0.0, 0.27, 0.55, 0.81].map(Saw::new);
        let mut noise = Noise(seed(start, 10));
        let mut formants = [Svf::default(); 3];
        self.voice(start, SECS, 0.5, |t| {
            let fall = (1.0 + 0.22 * (-t / 0.035).exp()) * (1.0 - 0.25 * t / SECS);
            let throat: f32 = saws
                .iter_mut()
                .zip(VOICES)
                .map(|(saw, pitch)| saw.next(pitch * fall))
                .sum::<f32>()
                / 4.0
                + noise.next() * 0.35;
            let vowel = formants[0].process(throat, 720.0, 5.0).band
                + formants[1].process(throat, 1_150.0, 6.0).band * 0.8
                + formants[2].process(throat, 2_500.0, 5.0).band * 0.3;
            let level = (t / 0.012).min(1.0) * (-t / 0.13).exp();
            (2.0 * vowel * level).tanh() * 0.34 * velocity * fade_out(t, SECS, 0.05)
        });
    }

    /// An air-raid siren in the distance, winding up and down once over `seconds`.
    fn siren(&mut self, start: usize, seconds: f32, velocity: f32) {
        let mut saws = [Saw::new(0.0), Saw::new(0.4)];
        let mut filter = Svf::default();
        self.voice(start, seconds, 0.7, |t| {
            let wind = 0.5 - 0.5 * (TAU * t / seconds).cos();
            let pitch = 330.0 + 330.0 * wind;
            let raw = (saws[0].next(pitch) + saws[1].next(pitch * 1.012)) * 0.5;
            let tone = filter.process(raw, pitch * 2.2, 2.5).band;
            let swell = (t / 0.8).min(1.0) * fade_out(t, seconds, 1.0);
            tone * 0.13 * velocity * swell * (0.35 + 0.65 * wind)
        });
    }

    /// A bass note: two detuned saws through a resonant low-pass whose cutoff starts `bright`
    /// Hz higher and falls, overdriven, over a clean sine at the fundamental.
    fn bass(&mut self, start: usize, gate: f32, note: f32, velocity: f32, bright: f32) {
        const RELEASE: f32 = 0.015;
        let frequency = midi(note);
        let mut saws = [Saw::new(0.0), Saw::new(0.5)];
        let mut sub = Sine::default();
        let mut filter = Svf::default();
        self.voice(start, gate + RELEASE, 0.0, |t| {
            let raw = (saws[0].next(frequency * 0.996) + saws[1].next(frequency * 1.004)) * 0.5;
            let cutoff = 160.0 + frequency * 3.0 + bright * (-t / 0.09).exp();
            let grit = (3.2 * filter.process(raw, cutoff, 2.8).low).tanh();
            let env = gate_env(t, 0.003, gate, RELEASE);
            (grit * 0.8 + sub.next(frequency * 2.0) * 0.18) * 0.3 * velocity * env
        });
    }

    /// A high-gain guitar on MIDI `root`: a power chord (root, fifth, and octave when it rings),
    /// tightened before the distortion and voiced through a speaker afterwards. Palm-muted, it
    /// is the short dark chug; open, it rings for the whole gate.
    fn guitar(&mut self, start: usize, gate: f32, root: f32, velocity: f32, palm: bool) {
        let release = if palm { 0.018 } else { 0.05 };
        let base = midi(root);
        let pitches = [
            base * 0.996,
            base * 1.004,
            base * 1.4983 * 0.997,
            base * 1.4983 * 1.003,
            base * 2.0 * 0.998,
            base * 2.0 * 1.002,
        ];
        let strings = if palm { 4 } else { 6 };
        let mut saws = [0.0, 0.31, 0.57, 0.13, 0.79, 0.44].map(Saw::new);
        let mut tight = Svf::default();
        let mut pre = Svf::default();
        let mut speaker = Svf::default();
        let mut presence = Svf::default();
        let send = if palm { 0.04 } else { 0.16 };
        self.voice(start, gate + release, send, |t| {
            let raw = saws
                .iter_mut()
                .zip(pitches)
                .take(strings)
                .map(|(saw, pitch)| saw.next(pitch))
                .sum::<f32>()
                / strings as f32;
            let (cutoff, level) = if palm {
                (520.0 + 2_400.0 * (-t / 0.022).exp(), (-t / 0.085).exp())
            } else {
                (
                    2_000.0 + 1_600.0 * (-t / 0.12).exp(),
                    0.8 + 0.2 * (-t / 0.35).exp(),
                )
            };
            // No mud into the amplifier, two stages of gain, then the speaker's roll-off with a
            // bite around 2 kHz.
            let string = pre
                .process(tight.process(raw, 150.0, 0.7).high, cutoff, 0.9)
                .low;
            let first = (9.0 * string).tanh();
            let driven = (2.5 * (first + 0.18 * first * first)).tanh();
            let cabinet = speaker.process(driven, 4_200.0, 0.9).low;
            let bite = presence.process(driven, 2_100.0, 1.6).band;
            (cabinet + 0.5 * bite) * 0.56 * velocity * level * gate_env(t, 0.002, gate, release)
        });
    }

    /// A lead guitar note that slides up into its pitch and takes on vibrato while it is held.
    fn lead(&mut self, start: usize, gate: f32, note: f32, velocity: f32) {
        const RELEASE: f32 = 0.1;
        let frequency = midi(note);
        let mut saws = [Saw::new(0.0), Saw::new(0.5)];
        let mut lfo = Sine::default();
        let mut pre = Svf::default();
        let mut tone = Svf::default();
        self.voice(start, gate + RELEASE, 0.5, |t| {
            let slide = 1.0 - 0.06 * (-t / 0.035).exp();
            let depth = 0.009 * ((t - 0.18) / 0.3).clamp(0.0, 1.0);
            let f = frequency * slide * (1.0 + depth * lfo.next(5.8));
            let raw = (saws[0].next(f) + saws[1].next(f * 1.006)) * 0.5;
            let driven = (7.0 * pre.process(raw, 2_600.0, 1.0).low).tanh();
            // A slow sweep of the tone, like a half-open wah.
            let voiced = tone
                .process(driven, 1_500.0 + 900.0 * (-t / 0.25).exp(), 2.2)
                .low;
            voiced * 0.24 * velocity * gate_env(t, 0.008, gate, RELEASE)
        });
    }

    /// A sequencer note: a saw through a filter that snaps shut.
    fn pluck(&mut self, start: usize, note: f32, velocity: f32) {
        const SECS: f32 = 0.16;
        let frequency = midi(note);
        let mut saw = Saw::new(0.0);
        let mut filter = Svf::default();
        self.voice(start, SECS, 0.55, |t| {
            let cutoff = frequency * (1.2 + 5.0 * (-t / 0.03).exp());
            filter.process(saw.next(frequency), cutoff, 3.0).low
                * 0.1
                * velocity
                * onset(t)
                * (-t / 0.07).exp()
                * fade_out(t, SECS, 0.02)
        });
    }

    /// An orchestra hit: every octave of `root` and its fifth at once, bright, then gone.
    fn stab(&mut self, start: usize, root: f32, velocity: f32) {
        const SECS: f32 = 0.55;
        let pitches: Vec<f32> = [0.0, 7.0, 12.0, 19.0, 24.0, 36.0]
            .iter()
            .flat_map(|&interval| [midi(root + interval) * 0.995, midi(root + interval) * 1.005])
            .collect();
        let mut saws: Vec<Saw> = (0..pitches.len())
            .map(|i| Saw::new((i as f32 * 0.37).fract()))
            .collect();
        let mut noise = Noise(seed(start, 11));
        let mut filter = Svf::default();
        let scale = 0.5 * velocity / pitches.len() as f32;
        self.voice(start, SECS, 0.5, |t| {
            let raw: f32 = saws
                .iter_mut()
                .zip(&pitches)
                .map(|(saw, &pitch)| saw.next(pitch))
                .sum();
            let hit = noise.next() * (-t / 0.01).exp() * 2.0;
            let cutoff = 700.0 + 4_500.0 * (-t / 0.09).exp();
            filter.process(raw + hit, cutoff, 0.9).low
                * scale
                * onset(t)
                * (-t / 0.17).exp()
                * fade_out(t, SECS, 0.05)
        });
    }

    /// A dark pad of MIDI `notes` (two detuned saws each) with slow attack and release.
    fn pad(&mut self, start: usize, gate: f32, notes: &[f32], velocity: f32) {
        const RELEASE: f32 = 0.8;
        let pitches: Vec<f32> = notes
            .iter()
            .flat_map(|&note| [midi(note) * 0.996, midi(note) * 1.004])
            .collect();
        let mut saws: Vec<Saw> = (0..pitches.len())
            .map(|i| Saw::new((i as f32 * 0.37).fract()))
            .collect();
        let mut filter = Svf::default();
        let scale = 0.16 * velocity / pitches.len() as f32;
        self.voice(start, gate + RELEASE, 0.5, |t| {
            let raw: f32 = saws
                .iter_mut()
                .zip(&pitches)
                .map(|(saw, &pitch)| saw.next(pitch))
                .sum();
            filter.process(raw, 800.0, 0.7).low * scale * gate_env(t, 0.6, gate, RELEASE)
        });
    }

    /// A rising sweep of filtered noise over `seconds`, ending in silence.
    fn riser(&mut self, start: usize, seconds: f32, velocity: f32) {
        let mut noise = Noise(seed(start, 7));
        let mut filter = Svf::default();
        self.voice(start, seconds, 0.3, |t| {
            let progress = t / seconds;
            let cutoff = 300.0 * 20.0f32.powf(progress);
            let band = filter.process(noise.next(), cutoff, 1.5).band;
            band * 0.15 * velocity * progress * progress * fade_out(t, seconds, 0.02)
        });
    }

    /// The guitar riff `riff` in `bar` over the chord `root` (semitones from D), with the bass
    /// an octave below it.
    fn riff(&mut self, bar: usize, root: f32, riff: &Riff, velocity: f32) {
        for &(step, interval, length, palm) in riff {
            let start = at(bar, f32::from(step));
            let note = D2 + root + f32::from(interval);
            let accent = if step == 0 { 1.0 } else { 0.88 };
            let held = steps(f32::from(length));
            let gate = if palm { held * 0.8 } else { held * 0.96 };
            self.guitar(start, gate, note, velocity * accent, palm);
            self.bass(
                start,
                held * 0.85,
                note - 12.0,
                accent,
                if palm { 500.0 } else { 1_300.0 },
            );
        }
    }

    /// The boots of the column on the four beats of `bar`, the left foot heavier.
    fn march(&mut self, bar: usize, velocity: f32) {
        for beat in 0..4 {
            let weight = if beat % 2 == 0 { 1.0 } else { 0.8 };
            self.boots(at(bar, beat as f32 * 4.0), velocity * weight);
        }
    }

    /// The snare cadence of the parade ground in `bar`, at `velocity`.
    fn cadence(&mut self, bar: usize, velocity: f32) {
        for (step, accent) in CADENCE {
            self.snare(at(bar, f32::from(step)), velocity * accent);
        }
    }

    /// A snare roll on the sixteenths `from` to `to` (exclusive) of `bar`, rising from `soft`
    /// to `loud`.
    fn roll(&mut self, bar: usize, from: u8, to: u8, soft: f32, loud: f32) {
        let count = f32::from(to - from).max(1.0);
        for step in from..to {
            let progress = f32::from(step - from) / count;
            self.snare(at(bar, f32::from(step)), soft + (loud - soft) * progress);
        }
    }

    /// The sequencer line in `bar`.
    fn arpeggio(&mut self, bar: usize, velocity: f32) {
        for (step, interval) in ARPEGGIO.into_iter().enumerate() {
            let accent = if step % 4 == 0 { 1.0 } else { 0.7 };
            self.pluck(
                at(bar, step as f32),
                62.0 + f32::from(interval),
                velocity * accent,
            );
        }
    }

    /// The march-in (`k` from 0 to 3): boots alone under a siren, then the snare cadence, a
    /// shout, and a roll into the riff.
    fn march_in(&mut self, bar: usize, k: usize) {
        self.march(bar, 1.0);
        if k == 0 {
            self.boom(at(bar, 0.0));
            self.siren(at(bar, 0.0), steps(62.0), 1.0);
            self.pad(at(bar, 0.0), steps(56.0), &[38.0, 45.0, 50.0], 0.8);
        }
        if k >= 1 {
            self.kick(at(bar, 0.0), 0.8);
            self.kick(at(bar, 8.0), 0.7);
        }
        match k {
            2 => self.cadence(bar, 0.7),
            3 => {
                for (step, accent) in &CADENCE[..7] {
                    self.snare(at(bar, f32::from(*step)), 0.8 * accent);
                }
                self.roll(bar, 12, 16, 0.5, 1.0);
                self.riser(at(bar, 0.0), steps(16.0), 0.8);
            }
            _ => {}
        }
        if k % 2 == 1 {
            self.shout(at(bar, 12.0), 1.0);
        }
        // The bass creeps in under the last two bars.
        if k >= 2 {
            for eighth in 0..8 {
                let bright = 150.0 + 90.0 * (eighth + 8 * (k - 2)) as f32;
                self.bass(
                    at(bar, eighth as f32 * 2.0),
                    steps(1.6),
                    D2 - 12.0,
                    0.8,
                    bright,
                );
            }
        }
    }

    /// A bar of the riff (`k` from 0 to 7) at `intensity` 0 (the first time) or 1 (the last):
    /// the guitars and bass over a half-time stomp, girders from its second half, and at full
    /// intensity the sequencer and a double kick.
    fn riff_section(&mut self, bar: usize, k: usize, intensity: usize) {
        let riff = if k.is_multiple_of(2) {
            RIFF_MARCH
        } else {
            RIFF_ANSWER
        };
        self.riff(bar, 0.0, riff, 1.0);
        self.march(bar, 0.45);
        if k.is_multiple_of(4) {
            self.crash(at(bar, 0.0), 0.95);
            self.stab(at(bar, 0.0), D2 + 12.0, 0.9);
            if k == 0 {
                self.boom(at(bar, 0.0));
            }
            if intensity > 0 {
                self.shout(at(bar, 0.0), 0.9);
            }
        }
        // Kick with the chugs, snare on two and four.
        let mut kicks = vec![0.0, 2.0, 3.0, 8.0, 10.0, 11.0];
        if intensity > 0 && k >= 4 {
            kicks.extend([6.0, 7.0, 14.0, 15.0]);
        } else if k % 2 == 1 {
            kicks.push(14.0);
        }
        for step in kicks {
            self.kick(at(bar, step), if step % 8.0 == 0.0 { 1.0 } else { 0.8 });
        }
        for eighth in 0..8 {
            let velocity = if eighth % 2 == 1 { 0.6 } else { 0.35 };
            let open = eighth == 7 && k.is_multiple_of(2);
            self.hat(at(bar, eighth as f32 * 2.0), velocity, open);
        }
        if k >= 4 {
            for step in [2.0, 6.0, 10.0, 14.0] {
                self.clank(at(bar, step), if step == 6.0 { 1.0 } else { 0.6 });
            }
        }
        if intensity > 0 {
            self.arpeggio(bar, 0.8);
        }
        self.snare(at(bar, 4.0), 1.0);
        match k {
            3 => self.roll(bar, 12, 16, 0.5, 0.95),
            7 => {
                self.snare(at(bar, 8.0), 0.9);
                for (step, frequency) in [(10.0, 140.0), (11.0, 140.0), (12.0, 112.0)] {
                    self.tom(at(bar, step), frequency, 0.9);
                }
                self.tom(at(bar, 13.0), 90.0, 0.95);
                self.roll(bar, 14, 16, 0.8, 1.0);
            }
            _ => {
                self.snare(at(bar, 12.0), 1.0);
                self.snare(at(bar, 15.0), 0.25);
            }
        }
    }

    /// The charge (`k` from 0 to 7): the lead guitar over driving chords and a full beat.
    fn charge(&mut self, bar: usize, k: usize) {
        let root = f32::from(CHARGE_ROOTS[k]);
        self.riff(bar, root, RIFF_CHARGE, 0.9);
        for &(step, note, length) in LEAD[k] {
            let gate = steps(f32::from(length)) * 0.94;
            self.lead(at(bar, f32::from(step)), gate, f32::from(note), 1.0);
        }
        if k.is_multiple_of(4) {
            self.crash(at(bar, 0.0), 1.0);
            self.shout(at(bar, 0.0), 0.85);
        }
        if k.is_multiple_of(2) {
            self.stab(at(bar, 0.0), D2 + 12.0 + root, 0.7);
        }
        for step in [0.0, 3.0, 6.0, 8.0, 11.0, 14.0] {
            self.kick(at(bar, step), if step % 8.0 == 0.0 { 1.0 } else { 0.85 });
        }
        for step in 0..16 {
            let velocity = [0.5, 0.2, 0.35, 0.2][step % 4];
            self.hat(at(bar, step as f32), velocity, false);
        }
        self.snare(at(bar, 4.0), 1.0);
        if k == 7 {
            // The dominant: a run of kicks and toms into the breakdown.
            for step in 8..16 {
                self.kick(at(bar, step as f32), 0.85);
            }
            for (step, frequency) in [(8.0, 140.0), (10.0, 118.0), (12.0, 96.0)] {
                self.tom(at(bar, step), frequency, 0.9);
            }
            self.roll(bar, 13, 16, 0.7, 1.0);
        } else {
            self.snare(at(bar, 12.0), 1.0);
            self.snare(at(bar, 11.0), 0.3);
        }
    }

    /// The breakdown (`k` from 0 to 3): the column marches on under the bass and the
    /// sequencer, the siren returns, and the cadence builds back into the riff.
    fn breakdown(&mut self, bar: usize, k: usize) {
        self.march(bar, 0.9);
        self.arpeggio(bar, 0.6 + 0.15 * k as f32);
        if k == 0 {
            self.boom(at(bar, 0.0));
            self.crash(at(bar, 0.0), 0.8);
            self.siren(at(bar, 0.0), steps(60.0), 0.8);
            self.pad(at(bar, 0.0), steps(56.0), &[50.0, 57.0, 62.0, 65.0], 1.0);
        }
        let riff = if k.is_multiple_of(2) {
            RIFF_MARCH
        } else {
            RIFF_ANSWER
        };
        for &(step, interval, length, palm) in riff {
            let bright = if palm { 700.0 } else { 1_800.0 } + 250.0 * k as f32;
            self.bass(
                at(bar, f32::from(step)),
                steps(f32::from(length)) * 0.85,
                D2 - 12.0 + f32::from(interval),
                0.95,
                bright,
            );
        }
        self.kick(at(bar, 0.0), 0.9);
        self.kick(at(bar, 8.0), 0.8);
        for step in [4.0, 12.0] {
            self.clank(at(bar, step), 0.9);
        }
        match k {
            0 => {}
            1 => self.shout(at(bar, 12.0), 0.9),
            2 => self.cadence(bar, 0.75),
            _ => {
                for (step, accent) in &CADENCE[..5] {
                    self.snare(at(bar, f32::from(*step)), 0.85 * accent);
                }
                self.roll(bar, 8, 16, 0.35, 1.0);
                self.riser(at(bar, 0.0), steps(16.0), 1.0);
                for step in 8..16 {
                    self.kick(at(bar, step as f32), 0.5 + 0.05 * (step - 8) as f32);
                }
                self.shout(at(bar, 12.0), 1.0);
            }
        }
    }
}

/// A damped feedback echo of `input` every `delay` samples.
fn echo(input: &[f32], delay: usize, feedback: f32) -> Vec<f32> {
    let mut out = vec![0.0f32; input.len()];
    let mut damped = 0.0f32;
    for n in delay..input.len() {
        let fed = input[n - delay] + feedback * out[n - delay];
        damped += (fed - damped) * 0.6;
        out[n] = damped;
    }
    out
}

/// A small Schroeder reverb: four damped combs in parallel, then two all-passes.
fn reverb(input: &[f32]) -> Vec<f32> {
    let mut out = vec![0.0f32; input.len()];
    for length in [557, 593, 641, 677] {
        let mut line = vec![0.0f32; length];
        let mut position = 0;
        let mut damped = 0.0f32;
        for (out, &x) in out.iter_mut().zip(input) {
            let y = line[position];
            damped = y * 0.7 + damped * 0.3;
            line[position] = x + damped * 0.82;
            position = (position + 1) % length;
            *out += y * 0.25;
        }
    }
    for length in [277, 211] {
        let mut line = vec![0.0f32; length];
        let mut position = 0;
        for sample in &mut out {
            let delayed = line[position];
            line[position] = *sample + delayed * 0.5;
            *sample = delayed - *sample;
            position = (position + 1) % length;
        }
    }
    out
}

/// The music: an original industrial-metal march, a loop of [`MUSIC_BARS`] bars in D minor at
/// 112.5 beats per minute. A column marches in under an air-raid siren; down-tuned high-gain
/// guitars and an overdriven bass play a riff built on the flat fifth over a half-time stomp
/// with struck girders and shouts from the ranks; a lead guitar carries the charge; the
/// breakdown leaves the boots, the bass and a sequencer, and the riff returns at full strength.
/// Everything is synthesised here; soft-clipped and normalised. Its tails wrap around to its
/// start, so it loops without a seam.
pub fn music() -> Vec<Sample> {
    let length = MUSIC_BARS * BAR;
    let mut mix = Mix {
        dry: vec![0.0; length + MUSIC_TAIL_BARS * BAR],
        send: vec![0.0; length + MUSIC_TAIL_BARS * BAR],
    };
    for bar in 0..MUSIC_BARS {
        match bar {
            0..=3 => mix.march_in(bar, bar),
            4..=11 => mix.riff_section(bar, bar - 4, 0),
            12..=19 => mix.charge(bar, bar - 12),
            20..=23 => mix.breakdown(bar, bar - 20),
            _ => mix.riff_section(bar, bar - 24, 1),
        }
    }
    // A dotted-eighth echo, and a reverb of the send and the echo.
    let echoes = echo(&mix.send, 3 * STEP, 0.3);
    let room: Vec<f32> = mix
        .send
        .iter()
        .zip(&echoes)
        .map(|(send, echo)| send + 0.5 * echo)
        .collect();
    let room = reverb(&room);
    let mut master: Vec<f32> = mix
        .dry
        .iter()
        .zip(&echoes)
        .zip(&room)
        .map(|((dry, echo), room)| dry + 0.25 * echo + 0.24 * room)
        .collect();
    high_pass(&mut master, 45.0);
    // Fade the last of the tail, then fold the tail onto the start.
    let total = master.len() as f32;
    for (i, sample) in master.iter_mut().enumerate().skip(length) {
        *sample *= fade_out(i as f32, total, BAR as f32);
    }
    let (body, tail) = master.split_at_mut(length);
    for (sample, tail) in body.iter_mut().zip(tail.iter()) {
        *sample += tail;
    }
    master.truncate(length);
    let mean = master.iter().sum::<f32>() / length as f32;
    let loudest = master
        .iter()
        .fold(0.0f32, |max, sample| max.max((sample - mean).abs()));
    let drive = if loudest > 0.0 { 1.6 / loudest } else { 1.0 };
    for sample in &mut master {
        *sample = ((*sample - mean) * drive).tanh();
    }
    normalise(master, MUSIC_PEAK)
}

/// The loudest sample of the music: below the effects' 0.8, because the dense mix is louder for
/// its peak than a single effect is.
const MUSIC_PEAK: f32 = 0.5;

// --- Volume ---------------------------------------------------------------------------------

/// A volume channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Channel {
    Master,
    Music,
    Effects,
}

/// The volume settings, in steps of 0 to [`VOLUME_STEPS`]; stored between sessions by
/// `crate::options` since M7.
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
    /// What kind of weapon it is.
    pub class: &'a dyn Fn(WeaponKind) -> WeaponClass,
    /// Whether the human player sees the cell now (scenery has no view of its own).
    pub cell_visible: &'a dyn Fn(sim::CellPos) -> bool,
    /// Whether the unit was crushed in this batch of events (RA3): its crushing is heard, not
    /// the bang of its destruction.
    pub crushed: &'a dyn Fn(SimId) -> bool,
}

/// The effect a simulation event plays for the human player, if any: see the module
/// documentation.
pub fn event_sound(event: &SimEvent, context: &SoundContext) -> Option<Sfx> {
    match event {
        SimEvent::StrategicActivated { player, building, .. } => (*player==HUMAN || (context.seen)(*building)).then_some(Sfx::Explosion),
        SimEvent::CommandRejected { player, .. } => (*player == HUMAN).then_some(Sfx::Error),
        SimEvent::ProductionComplete { player, .. } => (*player == HUMAN).then_some(Sfx::Ready),
        SimEvent::BuildingPlaced { player, .. } => (*player == HUMAN).then_some(Sfx::Placed),
        SimEvent::BuildingSold { player, .. } => (*player == HUMAN).then_some(Sfx::Sold),
        SimEvent::BuildingUndeployed { player, .. } => (*player == HUMAN).then_some(Sfx::Sold),
        SimEvent::OreDelivered { player, .. } => (*player == HUMAN).then_some(Sfx::Credits),
        // RA5: the player's own crate, whatever it held.
        SimEvent::CrateTaken { player, .. } => (*player == HUMAN).then_some(Sfx::Credits),
        SimEvent::WeaponFired {
            attacker,
            target,
            weapon,
        } => ((context.seen)(*attacker) || (context.seen)(*target))
            .then(|| Sfx::of((context.class)(*weapon))),
        // A soldier under a vehicle: heard by its owner, by the owner of the vehicle, and
        // by who was shown the soldier.
        SimEvent::UnitCrushed {
            player, unit, by, ..
        } => (*player == HUMAN || *by == HUMAN || (context.shown)(*unit)).then_some(Sfx::Crush),
        // RA4: the player hears its own units' promotions, wherever they are.
        SimEvent::RankGained { player, .. } => (*player == HUMAN).then_some(Sfx::Promotion),
        SimEvent::UnitDestroyed { player, unit } => (!(context.crushed)(*unit)
            && (*player == HUMAN || (context.shown)(*unit)))
        .then_some(Sfx::Explosion),
        SimEvent::BuildingDestroyed {
            player, building, ..
        } => (*player == HUMAN || (context.shown)(*building)).then_some(Sfx::Explosion),
        SimEvent::PropDestroyed { cell, .. } => {
            (context.cell_visible)(*cell).then_some(Sfx::Explosion)
        }
        SimEvent::MoveCompleted { .. }
        | SimEvent::MoveFailed { .. }
        | SimEvent::ProductionCancelled { .. }
        | SimEvent::UnitSpawned { .. }
        | SimEvent::HarvesterIdle { .. }
        | SimEvent::ProjectileExpired { .. }
        | SimEvent::AttackEnded { .. }
        | SimEvent::Impact { .. }
        | SimEvent::DamageStateChanged { .. }
        // RA4: a launch is heard as its launcher's shot, a missile's end as its impact.
        | SimEvent::CrewEscaped { .. }
        | SimEvent::CrateAppeared { .. }
        | SimEvent::CrateGone { .. }
        | SimEvent::UnitLaunched { .. }
        | SimEvent::UnitRecovered { .. }
        | SimEvent::MissileDetonated { .. }
        // The end screen announces the outcome (M6).
        | SimEvent::PlayerDefeated { .. }
        | SimEvent::MatchEnded { .. } => None,
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

pub(crate) fn setup_audio(
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
pub(crate) fn play_effects(
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
    mut impacts: ResMut<ImpactSounds>,
) {
    let Some(bank) = bank else {
        events.clear();
        impacts.0 = 0;
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
    let class = |weapon: WeaponKind| {
        definitions.map_or(WeaponClass::Rifle, |definitions| {
            crate::weapon_fx::class_of(definitions, weapon)
        })
    };
    let vision = crate::fog::human_vision(host.players());
    let cell_visible = |cell: sim::CellPos| vision.is_some_and(|vision| vision.is_visible(cell));
    // A crushing and the destruction it causes come in the same tick, so in the same batch.
    let crushed_units: std::collections::BTreeSet<SimId> = events
        .iter()
        .filter_map(|event| match event {
            SimEvent::UnitCrushed { unit, .. } => Some(*unit),
            _ => None,
        })
        .collect();
    let crushed = |id: SimId| crushed_units.contains(&id);
    let context = SoundContext {
        seen: &seen,
        shown: &shown,
        class: &class,
        cell_visible: &cell_visible,
        crushed: &crushed,
    };
    wanted.extend(
        events
            .into_iter()
            .filter_map(|event| event_sound(event, &context)),
    );
    // Shells the player saw land (`crate::weapon_fx` counts them as they burst).
    if std::mem::take(&mut impacts.0) > 0 {
        wanted.push(Sfx::Impact);
    }
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
        check_sound("music", &music, 80.0);
        assert!(music.len() >= samples_for(60.0));
        assert_eq!(music.len(), MUSIC_BARS * BAR);
        assert_eq!(music, self::music());
    }

    /// For listening outside the game: with `MYGAME_DUMP_MUSIC=<file.wav>` set, writes the
    /// music loop as a 16-bit mono WAV file. Without it, only checks the header arithmetic.
    #[test]
    fn the_music_can_be_written_as_a_wav_file() {
        let wav = |samples: &[Sample]| {
            let data = (samples.len() * 2) as u32;
            let mut bytes = Vec::with_capacity(44 + samples.len() * 2);
            bytes.extend(b"RIFF");
            bytes.extend((36 + data).to_le_bytes());
            bytes.extend(b"WAVEfmt ");
            bytes.extend(16u32.to_le_bytes());
            bytes.extend(1u16.to_le_bytes());
            bytes.extend(1u16.to_le_bytes());
            bytes.extend(SAMPLE_RATE_HZ.to_le_bytes());
            bytes.extend((SAMPLE_RATE_HZ * 2).to_le_bytes());
            bytes.extend(2u16.to_le_bytes());
            bytes.extend(16u16.to_le_bytes());
            bytes.extend(b"data");
            bytes.extend(data.to_le_bytes());
            for sample in samples {
                bytes.extend(((sample.clamp(-1.0, 1.0) * 32_767.0) as i16).to_le_bytes());
            }
            bytes
        };
        assert_eq!(wav(&[0.0, 1.0]).len(), 48);
        if let Ok(path) = std::env::var("MYGAME_DUMP_MUSIC") {
            std::fs::write(&path, wav(&music())).unwrap_or_else(|e| panic!("{path}: {e}"));
        }
        // Likewise every effect, into the directory `MYGAME_DUMP_SFX`.
        if let Ok(directory) = std::env::var("MYGAME_DUMP_SFX") {
            for sfx in Sfx::ALL {
                let path = format!("{directory}/{sfx:?}.wav");
                std::fs::write(&path, wav(&synthesize(sfx)))
                    .unwrap_or_else(|e| panic!("{path}: {e}"));
            }
        }
    }

    /// The share of a sound's energy above `hertz`, by a one-pole high-pass: enough to tell a
    /// bright sound from a dull one.
    fn brightness(samples: &[Sample], hertz: f32) -> f32 {
        let mut high = samples.to_vec();
        high_pass(&mut high, hertz);
        let energy = |s: &[Sample]| s.iter().map(|x| x * x).sum::<Sample>();
        energy(&high) / energy(samples)
    }

    /// How strongly a sound's loudness pulses at `hertz`: the correlation of its rectified
    /// signal with that frequency, against its mean level.
    fn pulsing(samples: &[Sample], hertz: f32) -> f32 {
        let (mut re, mut im, mut sum) = (0.0f32, 0.0f32, 0.0f32);
        for (i, sample) in samples.iter().enumerate() {
            let phase = TAU * hertz * i as f32 / SR;
            re += sample.abs() * phase.cos();
            im += sample.abs() * phase.sin();
            sum += sample.abs();
        }
        (re * re + im * im).sqrt() / sum
    }

    /// Each weapon sounds like what it is: a gun is a short burst of noise, a tank gun is mostly
    /// below a few hundred hertz and outlasts a rifle, the prism is the brightest of all (a
    /// ringing chord, not a bang), and the Tesla arc is chopped at twice its supply frequency.
    #[test]
    fn weapons_sound_like_what_they_are() {
        let sound = |sfx| synthesize(sfx);
        let (shot, cannon, prism, tesla) = (
            sound(Sfx::Shot),
            sound(Sfx::Cannon),
            sound(Sfx::Prism),
            sound(Sfx::Tesla),
        );
        // No two weapons share a sound.
        for (i, a) in Sfx::WEAPONS.iter().enumerate() {
            for b in &Sfx::WEAPONS[i + 1..] {
                assert_ne!(sound(*a), sound(*b), "{a:?} and {b:?}");
            }
        }
        // A cannon is darker and longer than a rifle shot.
        assert!(brightness(&cannon, 400.0) < brightness(&shot, 400.0));
        assert!(
            brightness(&cannon, 400.0) < 0.67,
            "{}",
            brightness(&cannon, 400.0)
        );
        assert!(cannon.len() > 2 * shot.len());
        // A rifle shot is over, but for its echo, in a twentieth of a second.
        let early = shot[..samples_for(0.05)]
            .iter()
            .map(|x| x * x)
            .sum::<Sample>();
        let all = shot.iter().map(|x| x * x).sum::<Sample>();
        assert!(early / all > 0.5, "{}", early / all);
        // Light: most of the prism is above a kilohertz, more than of any gun.
        assert!(
            brightness(&prism, 1_000.0) > 0.5,
            "{}",
            brightness(&prism, 1_000.0)
        );
        assert!(brightness(&prism, 1_000.0) > brightness(&shot, 1_000.0));
        // Electricity: the arc strikes 110 times a second; nothing else does.
        assert!(pulsing(&tesla, 110.0) > 0.12, "{}", pulsing(&tesla, 110.0));
        for other in [&shot, &cannon, &prism] {
            assert!(pulsing(other, 110.0) < pulsing(&tesla, 110.0) / 2.0);
        }
    }

    /// RA3: a soldier under a vehicle is a short dull crunch, not a bang: shorter and darker
    /// than a destruction, unlike a shell landing, and it ends in silence.
    #[test]
    fn a_crushing_is_a_short_dull_crunch() {
        let (crush, explosion, impact) = (
            synthesize(Sfx::Crush),
            synthesize(Sfx::Explosion),
            synthesize(Sfx::Impact),
        );
        assert!(2 * crush.len() <= explosion.len());
        assert_ne!(crush, impact[..crush.len().min(impact.len())].to_vec());
        assert!(
            brightness(&crush, 1_000.0) < brightness(&impact, 1_000.0),
            "{} and {}",
            brightness(&crush, 1_000.0),
            brightness(&impact, 1_000.0)
        );
        assert!(crush.last().is_some_and(|last| last.abs() < 0.01));
        // Most of it is over in a tenth of a second.
        let early = crush[..samples_for(0.1)]
            .iter()
            .map(|x| x * x)
            .sum::<Sample>();
        let all = crush.iter().map(|x| x * x).sum::<Sample>();
        assert!(early / all > 0.6, "{}", early / all);
    }

    #[test]
    fn weapon_sounds_end_in_silence_and_the_music_loops_without_a_click() {
        for sfx in Sfx::WEAPONS.into_iter().chain([Sfx::Explosion]) {
            let samples = synthesize(sfx);
            let last = samples.last().copied().unwrap_or_default();
            assert!(last.abs() < 0.01, "{sfx:?} ends at {last}");
        }
        let music = music();
        let mean = music.iter().sum::<Sample>() / music.len() as Sample;
        assert!(mean.abs() < 0.005, "music is offset by {mean}");
        // The jump from the last sample back to the first is an ordinary step of the music, no
        // larger than 99 % of the steps inside the loop: no click where the loop repeats.
        let seam = (music[0] - music[music.len() - 1]).abs();
        let mut steps: Vec<Sample> = music
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .collect();
        steps.sort_by(Sample::total_cmp);
        let typical = steps[steps.len() * 99 / 100];
        assert!(
            seam <= typical,
            "seam step {seam}, 99th percentile {typical}"
        );
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
        let class = |weapon: WeaponKind| match weapon.0 {
            0 => WeaponClass::Rifle,
            1 => WeaponClass::Cannon,
            2 => WeaponClass::Prism,
            _ => WeaponClass::Tesla,
        };
        let cell_visible = |cell: sim::CellPos| cell.x == 0;
        let crushed_id = SimId(3);
        let crushed = |id: SimId| id == crushed_id;
        let context = SoundContext {
            seen: &seen,
            shown: &shown,
            class: &class,
            cell_visible: &cell_visible,
            crushed: &crushed,
        };
        let sound = |event: SimEvent| event_sound(&event, &context);
        // RA4. A promotion pings for the human player's own unit only, seen or not.
        let promotion = |player, unit| SimEvent::RankGained {
            player,
            unit,
            rank: 1,
        };
        assert_eq!(sound(promotion(HUMAN, unseen_id)), Some(Sfx::Promotion));
        assert_eq!(sound(promotion(enemy, seen_id)), None);
        // A launch, a recovery, a missile's end and a crew have no sound of their own.
        assert_eq!(
            sound(SimEvent::UnitLaunched {
                player: HUMAN,
                parent: seen_id,
                unit: SimId(8)
            }),
            None
        );
        assert_eq!(
            sound(SimEvent::CrewEscaped {
                player: HUMAN,
                from: seen_id,
                at: sim::CellPos::new(0, 0),
                units: vec![SimId(8)]
            }),
            None
        );
        // RA3. A crushing is heard by the soldier's owner, by the owner of the vehicle, and
        // where the soldier was shown; and the destruction that follows it is silent.
        let crushing = |player, unit, by| SimEvent::UnitCrushed {
            player,
            unit,
            crusher: SimId(9),
            by,
            at: sim::SubPos::new(0, 0),
        };
        assert_eq!(sound(crushing(HUMAN, unseen_id, enemy)), Some(Sfx::Crush));
        assert_eq!(sound(crushing(enemy, unseen_id, HUMAN)), Some(Sfx::Crush));
        assert_eq!(
            sound(crushing(enemy, seen_id, PlayerId(2))),
            Some(Sfx::Crush)
        );
        assert_eq!(sound(crushing(enemy, unseen_id, PlayerId(2))), None);
        assert_eq!(
            sound(SimEvent::UnitDestroyed {
                player: HUMAN,
                unit: crushed_id
            }),
            None
        );
        // Scenery falls with a bang where the player is looking, silently elsewhere.
        let razed = |x| SimEvent::PropDestroyed {
            prop: SimId(sim::PROP_ID_BASE),
            cell: sim::CellPos::new(x, 0),
            kind: sim::PropKind::House,
        };
        assert_eq!(sound(razed(0)), Some(Sfx::Explosion));
        assert_eq!(sound(razed(1)), None);
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
                    gems: 0,
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
        assert_eq!(sound(fired(seen_id, seen_id, 2)), Some(Sfx::Prism));
        assert_eq!(sound(fired(seen_id, seen_id, 3)), Some(Sfx::Tesla));
        assert_eq!(sound(fired(unseen_id, unseen_id, 3)), None);
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
