//! Weapons on screen: what a shot looks like, by the kind of weapon that fired it.
//!
//! | Weapon | Drawn as |
//! |---|---|
//! | rifles, machine guns | a muzzle flash, tracers that cross to the target, dust where they strike |
//! | cannon, artillery | a muzzle blast with smoke and a dust ring, the shell in flight, an explosion where it lands |
//! | rockets, missiles | the rocket on its flame with a smoke trail, an explosion where it lands |
//! | prism | a beam of light: a white core in a blue halo with the fringes of a spectrum, a glare at the emitter and where it strikes |
//! | tesla | a lightning bolt that forks and jumps to a new path many times a second |
//! | flak, radiation, psychic, chrono, sonic | bursts in the air, a green ray, travelling rings |
//!
//! Everything here is presentation: it reads the simulation's events and positions and never
//! writes them (invariant I5), and nothing is shown of a shot whose end the human player does not
//! see (`crate::fog::shot_view`; docs/DECISIONS.md D31 item 3, D33). Effects age in game time, so
//! they stand still while the match is paused.

use std::collections::BTreeMap;
use std::f32::consts::{PI, TAU};

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use sim::{Building, CellPos, Position, SUBCELLS_PER_CELL, SimEvent, SimId, Unit, WeaponKind};

use crate::battlefield::{BattlefieldVisual, Materials};
use crate::fog::{ShotView, human_vision, shot_view};
use crate::map::MapView;
use crate::projection::{pick_cell, to_world};
use crate::sim_host::{CommandSink, SimHost};
use crate::units::{Altitudes, GROUND_OFFSET_PX};

/// Effect sprites alive at once at most; a shot that would exceed it is drawn in part or not at
/// all, so a large battle cannot stall the frame.
const MAX_FX: usize = 1_800;

/// Seconds a lightning bolt lasts, and between two of its paths.
const BOLT_SECS: f32 = 0.36;
const BOLT_JUMP_SECS: f32 = 0.045;

/// Above particles (3.5) and everything on the ground or in the air.
const Z_GLOW: f32 = 3.6;
const Z_CORE: f32 = 3.7;

pub struct WeaponFxPlugin;

impl Plugin for WeaponFxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ImpactSounds>()
            .add_systems(Startup, make_art)
            .add_systems(Update, (fire, fly_shells, crackle, animate).chain());
    }
}

/// What kind of weapon a shot came from: decides how it looks and sounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WeaponClass {
    Rifle,
    MachineGun,
    Sniper,
    Cannon,
    Artillery,
    Rocket,
    Grenade,
    Flak,
    Prism,
    Tesla,
    Radiation,
    Psychic,
    Chrono,
    Sonic,
    Bomb,
    Torpedo,
    Melee,
    Trigger,
}

/// The class of the weapon `id` (`assets/data/combat.ron`); `instant` is whether it has no
/// projectile. A weapon this does not know is a rifle if instant and a cannon otherwise.
pub fn classify(id: &str, instant: bool) -> WeaponClass {
    use WeaponClass::*;
    match id {
        "prism_beam" => Prism,
        "tesla_weapon" => Tesla,
        "radiation_weapon" => Radiation,
        "control_beam" => Psychic,
        "chrono_beam" => Chrono,
        "sonic" => Sonic,
        "sniper_rifle" => Sniper,
        "scout_gun" | "raider_gun" => MachineGun,
        "flak_weapon" => Flak,
        "grenade" => Grenade,
        "air_bomb" | "eagle_bomb" | "kirov_bomb" => Bomb,
        "dog_bite" | "drone_claw" | "squid_grip" => Melee,
        "bomb_trigger" | "suicide_trigger" => Trigger,
        "artillery_shell" | "grand_shell" => Artillery,
        "torpedo" => Torpedo,
        "mirage_gun" => Cannon,
        _ if id.contains("rocket") || id.contains("missile") || id.contains("strike") => Rocket,
        _ if id.contains("cannon") || id.contains("shell") => Cannon,
        _ if instant => Rifle,
        _ => Cannon,
    }
}

/// The class of `weapon` in the match's definitions.
pub fn class_of(definitions: &sim::Definitions, weapon: WeaponKind) -> WeaponClass {
    definitions
        .weapon(weapon)
        .map_or(WeaponClass::Rifle, |def| {
            classify(&def.id, def.projectile_speed == 0)
        })
}

/// Impacts the human player saw this frame, for `crate::audio` to give a sound.
#[derive(Resource, Debug, Default)]
pub struct ImpactSounds(pub u32);

/// The textures of the effects, drawn once at startup.
#[derive(Resource)]
struct FxArt {
    /// A line with a soft edge: opaque along its middle, clear at its sides.
    beam: Handle<Image>,
    /// The same, fading towards its tail: tracers, shells, flame.
    streak: Handle<Image>,
}

fn soft_line(taper: bool) -> Image {
    const W: u32 = 32;
    const H: u32 = 32;
    let mut data = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        let across = (y as f32 + 0.5) / H as f32 * 2.0 - 1.0;
        for x in 0..W {
            let along = (x as f32 + 0.5) / W as f32;
            let lengthwise = if taper { along * along } else { 1.0 };
            let alpha = (-across * across * 5.0).exp() * (1.0 - across * across) * lengthwise;
            data.extend_from_slice(&[255, 255, 255, (alpha.clamp(0.0, 1.0) * 255.0) as u8]);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    image
}

fn make_art(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.insert_resource(FxArt {
        beam: images.add(soft_line(false)),
        streak: images.add(soft_line(true)),
    });
}

/// One sprite of an effect.
#[derive(Component)]
struct Fx {
    /// Seconds since it appeared; negative while it waits to (the later shots of a burst).
    age: f32,
    life: f32,
    velocity: Vec2,
    size: Vec2,
    growth: Vec2,
    color: [f32; 4],
    /// How much of its width it loses as it dies (a beam narrows to nothing).
    taper: f32,
    /// How much its brightness wavers.
    flicker: f32,
    /// Stays bright and fades late (light), instead of fading from the start (smoke).
    hold: bool,
    /// Shown while the human player sees both.
    cells: [CellPos; 2],
}

/// A shell, rocket or bomb between its weapon and its target.
#[derive(Component)]
struct Shell {
    class: WeaponClass,
    target: SimId,
    /// Ground point and height above it where it was fired, and the same for where it is going
    /// (kept in case the target is gone before it lands).
    from: (Vec2, f32),
    to: (Vec2, f32),
    age: f32,
    flight: f32,
    /// The height of its arc at mid-flight.
    arc: f32,
    trail: f32,
}

/// A lightning bolt: it draws itself anew along a different path every few hundredths of a
/// second, which is what makes an electric arc look alive.
#[derive(Component)]
struct Bolt {
    a: Vec2,
    b: Vec2,
    cells: [CellPos; 2],
    left: f32,
    next: f32,
    seed: u32,
}

/// A small deterministic generator for the shapes of effects (xorshift32).
struct Dice(u32);

impl Dice {
    fn new(seed: u32) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9) | 1)
    }

    /// From -1 to 1.
    fn roll(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// From 0 to 1.
    fn unit(&mut self) -> f32 {
        self.roll() * 0.5 + 0.5
    }
}

/// The path of a lightning bolt from `a` to `b`: a random walk across the straight line that
/// returns to it at both ends. Always starts at `a` and ends at `b`.
pub fn bolt_path(a: Vec2, b: Vec2, seed: u32) -> Vec<Vec2> {
    let span = b - a;
    let length = span.length();
    let steps = ((length / 9.0).round() as usize).clamp(3, 28);
    let normal = span.perp().normalize_or_zero();
    let reach = (length * 0.06).clamp(2.5, 9.0);
    let mut dice = Dice::new(seed);
    let mut offset = 0.0f32;
    let mut points = Vec::with_capacity(steps + 1);
    points.push(a);
    for i in 1..steps {
        let t = i as f32 / steps as f32;
        // A wandering channel: each kink starts from most of the last one's offset, so the bolt
        // drifts to one side and back instead of zigzagging evenly about its line.
        offset = offset * 0.72 + dice.roll() * reach;
        // Jitter along the bolt as well, so that the kinks are not evenly spaced.
        let along = t + dice.roll() * 0.25 / steps as f32;
        points.push(a + span * along + normal * offset * (PI * t).sin().sqrt());
    }
    points.push(b);
    points
}

/// What draws the sprites of one frame's effects.
struct Painter<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    art: &'a FxArt,
    mat: &'a Materials,
    left: usize,
}

/// A sprite to draw: see [`Fx`].
struct Stroke {
    delay: f32,
    life: f32,
    velocity: Vec2,
    growth: Vec2,
    taper: f32,
    flicker: f32,
    hold: bool,
    z: f32,
}

impl Default for Stroke {
    fn default() -> Self {
        Self {
            delay: 0.0,
            life: 0.3,
            velocity: Vec2::ZERO,
            growth: Vec2::ZERO,
            taper: 0.0,
            flicker: 0.0,
            hold: false,
            z: Z_GLOW,
        }
    }
}

impl Painter<'_, '_, '_> {
    #[allow(clippy::too_many_arguments)]
    fn sprite(
        &mut self,
        image: Handle<Image>,
        at: Vec2,
        angle: f32,
        size: Vec2,
        color: [f32; 4],
        cells: [CellPos; 2],
        stroke: Stroke,
    ) {
        if self.left == 0 {
            return;
        }
        self.left -= 1;
        self.commands.spawn((
            BattlefieldVisual,
            Fx {
                age: -stroke.delay,
                life: stroke.life,
                velocity: stroke.velocity,
                size,
                growth: stroke.growth,
                color,
                taper: stroke.taper,
                flicker: stroke.flicker,
                hold: stroke.hold,
                cells,
            },
            Sprite {
                image,
                custom_size: Some(size),
                color: Color::srgba(color[0], color[1], color[2], color[3]),
                ..default()
            },
            Transform {
                translation: at.extend(stroke.z),
                rotation: Quat::from_rotation_z(angle),
                ..default()
            },
            if stroke.delay > 0.0 {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            },
        ));
    }

    /// A soft-edged line from `a` to `b`.
    fn line(
        &mut self,
        a: Vec2,
        b: Vec2,
        width: f32,
        color: [f32; 4],
        cells: [CellPos; 2],
        stroke: Stroke,
    ) {
        let span = b - a;
        let length = span.length();
        if length < 0.5 {
            return;
        }
        self.sprite(
            self.art.beam.clone(),
            (a + b) * 0.5,
            span.y.atan2(span.x),
            Vec2::new(length, width),
            color,
            cells,
            stroke,
        );
    }

    /// A round glow.
    fn glow(&mut self, at: Vec2, size: f32, color: [f32; 4], cells: [CellPos; 2], stroke: Stroke) {
        self.sprite(
            self.mat.glow.clone(),
            at,
            0.0,
            Vec2::splat(size),
            color,
            cells,
            stroke,
        );
    }

    /// A puff of smoke or dust.
    fn smoke(&mut self, at: Vec2, size: f32, color: [f32; 4], cells: [CellPos; 2], stroke: Stroke) {
        self.sprite(
            self.mat.smoke.clone(),
            at,
            0.0,
            Vec2::splat(size),
            color,
            cells,
            stroke,
        );
    }

    /// A ring lying on the ground (half as tall as wide, as the map is seen).
    fn ground_ring(&mut self, at: Vec2, size: f32, spread: f32, color: [f32; 4], cell: CellPos) {
        self.sprite(
            self.mat.ring.clone(),
            at,
            0.0,
            Vec2::new(size, size * 0.5),
            color,
            [cell; 2],
            Stroke {
                life: 0.4,
                growth: Vec2::new(spread, spread * 0.5),
                ..default()
            },
        );
    }

    /// Glowing fragments thrown out from `at`.
    fn sparks(
        &mut self,
        at: Vec2,
        count: usize,
        speed: f32,
        color: [f32; 4],
        cell: CellPos,
        seed: u32,
    ) {
        let mut dice = Dice::new(seed);
        for _ in 0..count {
            let angle = dice.unit() * TAU;
            let pace = speed * (0.4 + 0.6 * dice.unit());
            self.sprite(
                Handle::default(),
                at,
                angle,
                Vec2::new(3.2, 1.3),
                color,
                [cell; 2],
                Stroke {
                    life: 0.25 + 0.3 * dice.unit(),
                    // Flattened: they fly out over the ground more than up the screen.
                    velocity: Vec2::new(angle.cos(), angle.sin() * 0.6 + 0.25) * pace,
                    z: Z_CORE,
                    ..default()
                },
            );
        }
    }

    /// The flash of a gun at `at`, pointing along `dir`.
    fn muzzle_flash(&mut self, at: Vec2, dir: Vec2, scale: f32, cell: CellPos) {
        self.glow(
            at,
            11.0 * scale,
            [1.0, 0.9, 0.62, 1.0],
            [cell; 2],
            Stroke {
                life: 0.07,
                hold: true,
                z: Z_CORE,
                ..default()
            },
        );
        self.sprite(
            self.art.streak.clone(),
            at + dir * 5.0 * scale,
            (-dir.y).atan2(-dir.x),
            Vec2::new(12.0 * scale, 4.5 * scale),
            [1.0, 0.82, 0.4, 0.95],
            [cell; 2],
            Stroke {
                life: 0.06,
                hold: true,
                z: Z_CORE,
                ..default()
            },
        );
    }

    /// The blast of a cannon: a white flash, a tongue of flame, rolling smoke, and the dust
    /// the shock lifts off the ground under the muzzle.
    fn muzzle_blast(&mut self, at: Vec2, dir: Vec2, scale: f32, cell: CellPos, seed: u32) {
        let cells = [cell; 2];
        self.glow(
            at,
            30.0 * scale,
            [1.0, 0.96, 0.8, 1.0],
            cells,
            Stroke {
                life: 0.09,
                hold: true,
                z: Z_CORE,
                ..default()
            },
        );
        self.glow(
            at + dir * 4.0,
            16.0 * scale,
            [1.0, 0.55, 0.14, 0.9],
            cells,
            Stroke {
                life: 0.2,
                growth: Vec2::splat(70.0 * scale),
                velocity: dir * 30.0,
                ..default()
            },
        );
        self.sprite(
            self.art.streak.clone(),
            at + dir * 13.0 * scale,
            (-dir.y).atan2(-dir.x),
            Vec2::new(30.0 * scale, 9.0 * scale),
            [1.0, 0.8, 0.42, 0.95],
            cells,
            Stroke {
                life: 0.11,
                hold: true,
                z: Z_CORE,
                ..default()
            },
        );
        let mut dice = Dice::new(seed);
        for i in 0..4 {
            self.smoke(
                at + dir * (6.0 + 5.0 * i as f32),
                9.0 * scale,
                [0.62, 0.6, 0.56, 0.5],
                cells,
                Stroke {
                    delay: 0.03 * i as f32,
                    life: 0.9 + 0.5 * dice.unit(),
                    velocity: dir * (26.0 - 5.0 * i as f32) + Vec2::new(dice.roll() * 5.0, 7.0),
                    growth: Vec2::splat(24.0 * scale),
                    ..default()
                },
            );
        }
        self.ground_ring(
            at - Vec2::Y * (GROUND_OFFSET_PX + 2.0),
            12.0 * scale,
            110.0 * scale,
            [0.66, 0.58, 0.44, 0.45],
            cell,
        );
    }

    /// An explosion on the ground at `at`: flash, fireball, smoke that rises, fragments, and the
    /// shock running out over the ground.
    fn explosion(&mut self, at: Vec2, size: f32, cell: CellPos, seed: u32) {
        let cells = [cell; 2];
        let mut dice = Dice::new(seed);
        self.glow(
            at,
            size * 1.9,
            [1.0, 0.97, 0.85, 1.0],
            cells,
            Stroke {
                life: 0.1,
                hold: true,
                z: Z_CORE,
                ..default()
            },
        );
        for i in 0..3 {
            let offset = Vec2::new(dice.roll(), dice.roll() * 0.6) * size * 0.25;
            self.glow(
                at + offset,
                size * (0.7 + 0.2 * i as f32),
                [1.0, 0.58 - 0.12 * i as f32, 0.12, 0.92],
                cells,
                Stroke {
                    delay: 0.02 * i as f32,
                    life: 0.3 + 0.08 * i as f32,
                    velocity: Vec2::Y * (8.0 + 6.0 * i as f32),
                    growth: Vec2::splat(size * 1.6),
                    ..default()
                },
            );
        }
        for i in 0..4 {
            let offset = Vec2::new(dice.roll(), dice.roll() * 0.5) * size * 0.3;
            self.smoke(
                at + offset,
                size * 0.6,
                [0.16, 0.15, 0.15, 0.62],
                cells,
                Stroke {
                    delay: 0.05 + 0.04 * i as f32,
                    life: 1.3 + 0.6 * dice.unit(),
                    velocity: Vec2::new(dice.roll() * 6.0 + 4.0, 13.0 + 6.0 * dice.unit()),
                    growth: Vec2::splat(size * 0.9),
                    ..default()
                },
            );
        }
        self.sparks(
            at,
            (size / 3.0) as usize + 4,
            size * 4.5,
            [1.0, 0.8, 0.35, 1.0],
            cell,
            seed ^ 0x51ED,
        );
        self.ground_ring(
            at - Vec2::Y * 4.0,
            size * 0.6,
            size * 6.0,
            [0.85, 0.74, 0.52, 0.55],
            cell,
        );
    }

    /// A beam of light from `a` to `b`: see the module documentation.
    fn prism_beam(&mut self, a: Vec2, b: Vec2, cells: [CellPos; 2], seed: u32) {
        const LIFE: f32 = 0.42;
        let normal = (b - a).perp().normalize_or_zero();
        let light = |life: f32, z: f32, taper: f32, flicker: f32| Stroke {
            life,
            taper,
            flicker,
            hold: true,
            z,
            ..default()
        };
        self.line(
            a,
            b,
            30.0,
            [0.35, 0.62, 1.0, 0.2],
            cells,
            light(LIFE, Z_GLOW, 0.3, 0.18),
        );
        // Light split by a prism leaves its colours side by side: red and gold on one edge of
        // the beam, green and violet on the other.
        for (offset, color) in [
            (3.2, [1.0, 0.32, 0.25, 0.3]),
            (1.9, [1.0, 0.86, 0.3, 0.3]),
            (-1.9, [0.35, 1.0, 0.55, 0.27]),
            (-3.2, [0.62, 0.36, 1.0, 0.34]),
        ] {
            let shift = normal * offset;
            self.line(
                a + shift,
                b + shift,
                3.2,
                color,
                cells,
                light(LIFE * 0.9, Z_GLOW + 0.02, 0.5, 0.1),
            );
        }
        self.line(
            a,
            b,
            9.0,
            [0.76, 0.92, 1.0, 0.62],
            cells,
            light(LIFE, Z_GLOW + 0.04, 0.5, 0.12),
        );
        self.line(
            a,
            b,
            3.4,
            [1.0, 1.0, 1.0, 1.0],
            cells,
            light(LIFE, Z_CORE, 0.75, 0.06),
        );
        // The glare of the emitter: a bloom and a four-pointed glint.
        self.glow(
            a,
            34.0,
            [0.85, 0.95, 1.0, 0.95],
            [cells[0]; 2],
            light(LIFE, Z_CORE, 0.0, 0.1),
        );
        for arm in [Vec2::X, Vec2::Y] {
            self.line(
                a - arm * 24.0,
                a + arm * 24.0,
                2.6,
                [1.0, 1.0, 1.0, 0.85],
                [cells[0]; 2],
                light(0.28, Z_CORE, 1.0, 0.0),
            );
        }
        // Where it strikes: glare, a burst of incandescent fragments, heat left behind.
        let hit = [cells[1]; 2];
        self.glow(
            b,
            46.0,
            [0.8, 0.94, 1.0, 0.95],
            hit,
            light(LIFE + 0.1, Z_CORE, 0.0, 0.15),
        );
        self.glow(
            b,
            20.0,
            [1.0, 0.62, 0.2, 0.7],
            hit,
            Stroke {
                delay: LIFE * 0.6,
                life: 0.9,
                growth: Vec2::splat(14.0),
                ..default()
            },
        );
        self.sparks(b, 10, 90.0, [0.86, 0.96, 1.0, 1.0], cells[1], seed);
        self.ground_ring(
            b - Vec2::Y * 6.0,
            10.0,
            150.0,
            [0.7, 0.9, 1.0, 0.6],
            cells[1],
        );
        self.smoke(
            b,
            9.0,
            [0.3, 0.3, 0.32, 0.45],
            hit,
            Stroke {
                delay: LIFE * 0.5,
                life: 1.4,
                velocity: Vec2::new(3.0, 13.0),
                growth: Vec2::splat(18.0),
                ..default()
            },
        );
    }

    /// One path of a lightning bolt, with two forks.
    fn lightning(&mut self, a: Vec2, b: Vec2, cells: [CellPos; 2], seed: u32) {
        let arc = |z: f32| Stroke {
            life: BOLT_JUMP_SECS * 1.5,
            hold: true,
            z,
            ..default()
        };
        let path = bolt_path(a, b, seed);
        for pair in path.windows(2) {
            self.line(
                pair[0],
                pair[1],
                13.0,
                [0.3, 0.5, 1.0, 0.34],
                cells,
                arc(Z_GLOW),
            );
            self.line(
                pair[0],
                pair[1],
                2.7,
                [0.88, 0.94, 1.0, 1.0],
                cells,
                arc(Z_CORE),
            );
        }
        // Forks leave the main channel and die out in the air.
        let mut dice = Dice::new(seed ^ 0xA53C);
        let along = (b - a).normalize_or_zero();
        for _ in 0..2 {
            let root =
                path[1 + (dice.unit() * (path.len() - 2) as f32) as usize % (path.len() - 1)];
            let turn = (0.5 + 0.5 * dice.unit()) * dice.roll().signum();
            let heading = Vec2::from_angle(turn).rotate(along);
            let tip = root + heading * (14.0 + 22.0 * dice.unit());
            for pair in bolt_path(root, tip, seed ^ dice.0).windows(2) {
                self.line(
                    pair[0],
                    pair[1],
                    7.0,
                    [0.3, 0.5, 1.0, 0.24],
                    cells,
                    arc(Z_GLOW),
                );
                self.line(
                    pair[0],
                    pair[1],
                    1.5,
                    [0.85, 0.92, 1.0, 0.85],
                    cells,
                    arc(Z_CORE),
                );
            }
        }
        self.glow(a, 18.0, [0.7, 0.84, 1.0, 0.8], [cells[0]; 2], arc(Z_CORE));
        self.glow(b, 30.0, [0.72, 0.86, 1.0, 0.85], [cells[1]; 2], arc(Z_CORE));
        self.sparks(b, 2, 110.0, [0.8, 0.9, 1.0, 1.0], cells[1], seed ^ 0x77);
    }
}

/// Where a unit, building or piece of scenery is on screen.
#[derive(Clone, Copy)]
struct Spot {
    /// Its point on the ground, in world pixels.
    ground: Vec2,
    /// How high above that its body (or, for a building, its weapon) is drawn.
    lift: f32,
    cell: CellPos,
    /// How far from its centre its weapon ends.
    reach: f32,
}

impl Spot {
    fn body(&self) -> Vec2 {
        self.ground + Vec2::Y * self.lift
    }
}

/// The spots of everything in the simulation that can fire or be fired at.
fn spots(host: &SimHost, map: &MapView, altitudes: &Altitudes) -> BTreeMap<SimId, Spot> {
    let Some(mut positions) =
        host.world()
            .try_query::<(&SimId, Option<&Position>, Option<&Unit>, Option<&Building>)>()
    else {
        return BTreeMap::new();
    };
    let scale = SUBCELLS_PER_CELL as f32;
    let definitions = host.definitions();
    positions
        .iter(host.world())
        .filter_map(|(id, position, unit, building)| {
            let spot = match (position, building) {
                (Some(position), _) => {
                    let infantry = unit.zip(definitions).is_some_and(|(unit, definitions)| {
                        definitions.category(sim::ProductionItem::Unit(unit.kind))
                            == sim::Category::Infantry
                    });
                    Spot {
                        ground: to_world(
                            position.0.x as f32 / scale,
                            position.0.y as f32 / scale,
                            map.tile,
                        ),
                        lift: GROUND_OFFSET_PX + altitudes.of(*id),
                        cell: position.0.cell(),
                        reach: if infantry { 6.0 } else { 15.0 },
                    }
                }
                (None, Some(building)) => {
                    let footprint = building.footprint;
                    let centre = (
                        footprint.origin.x as f32 + f32::from(footprint.width) * 0.5,
                        footprint.origin.y as f32 + f32::from(footprint.height) * 0.5,
                    );
                    Spot {
                        ground: to_world(centre.0, centre.1, map.tile),
                        // A defence's weapon is at the top of it.
                        lift: 30.0,
                        cell: CellPos::new(centre.0 as i32, centre.1 as i32),
                        reach: 4.0,
                    }
                }
                (None, None) => return None,
            };
            Some((*id, spot))
        })
        .collect()
}

/// Scenery (`sim::Props`) is not an entity: its spot is its cell's centre.
fn spot_of(
    spots: &BTreeMap<SimId, Spot>,
    host: &SimHost,
    map: &MapView,
    id: SimId,
) -> Option<Spot> {
    spots.get(&id).copied().or_else(|| {
        let (cell, _) = host.props()?.get(id)?;
        Some(Spot {
            ground: crate::projection::cell_centre(cell, map.tile),
            lift: GROUND_OFFSET_PX,
            cell,
            reach: 0.0,
        })
    })
}

/// Draws the shots fired in the last ticks.
#[allow(clippy::too_many_arguments)]
fn fire(
    mut commands: Commands,
    mut events: MessageReader<SimEvent>,
    host: Res<SimHost>,
    sink: Res<CommandSink>,
    map: Option<Res<MapView>>,
    art: Option<Res<FxArt>>,
    mat: Option<Res<Materials>>,
    altitudes: Res<Altitudes>,
    live: Query<(), With<Fx>>,
) {
    let (Some(map), Some(art), Some(mat), Some(vision), Some(definitions)) = (
        map,
        art,
        mat,
        human_vision(host.players()),
        host.definitions(),
    ) else {
        events.clear();
        return;
    };
    if sink.paused() {
        events.clear();
        return;
    }
    let shots: Vec<(SimId, SimId, WeaponKind)> = events
        .read()
        .filter_map(|event| match event {
            SimEvent::WeaponFired {
                attacker,
                target,
                weapon,
            } => Some((*attacker, *target, *weapon)),
            _ => None,
        })
        .collect();
    if shots.is_empty() {
        return;
    }
    let spots = spots(&host, &map, &altitudes);
    let mut painter = Painter {
        commands: &mut commands,
        art: &art,
        mat: &mat,
        left: MAX_FX.saturating_sub(live.iter().count()),
    };
    for (attacker, target, weapon) in shots {
        let (Some(from), Some(to)) = (
            spots.get(&attacker).copied(),
            spot_of(&spots, &host, &map, target),
        ) else {
            continue;
        };
        let Some(def) = definitions.weapon(weapon) else {
            continue;
        };
        let class = classify(&def.id, def.projectile_speed == 0);
        let view = shot_view(vision.is_visible(from.cell), vision.is_visible(to.cell));
        if view == ShotView::Hidden {
            continue;
        }
        let seed = attacker.0 ^ (host.tick() as u32).wrapping_mul(0x9E37) ^ target.0;
        let dir = (to.body() - from.body()).normalize_or_zero();
        let muzzle = from.body() + dir * from.reach;
        let hit = to.body();
        let cells = [from.cell, to.cell];
        let (at_muzzle, at_target, across) = (
            view != ShotView::AtTarget,
            view != ShotView::AtAttacker,
            view == ShotView::Line,
        );
        if def.projectile_speed > 0 || class == WeaponClass::Bomb {
            if at_muzzle {
                match class {
                    WeaponClass::Cannon => painter.muzzle_blast(muzzle, dir, 1.0, from.cell, seed),
                    WeaponClass::Artillery => {
                        painter.muzzle_blast(muzzle, dir, 1.35, from.cell, seed)
                    }
                    WeaponClass::Rocket | WeaponClass::Torpedo => {
                        // Backblast: the exhaust goes out behind the launcher.
                        painter.glow(
                            muzzle,
                            16.0,
                            [1.0, 0.8, 0.5, 0.95],
                            [from.cell; 2],
                            Stroke {
                                life: 0.12,
                                hold: true,
                                z: Z_CORE,
                                ..default()
                            },
                        );
                        for i in 0..3 {
                            painter.smoke(
                                muzzle - dir * (4.0 + 5.0 * i as f32),
                                8.0,
                                [0.8, 0.78, 0.74, 0.5],
                                [from.cell; 2],
                                Stroke {
                                    delay: 0.02 * i as f32,
                                    life: 0.9,
                                    velocity: -dir * 22.0 + Vec2::Y * 6.0,
                                    growth: Vec2::splat(22.0),
                                    ..default()
                                },
                            );
                        }
                    }
                    _ => {}
                }
            }
            // The flight a projectile has left, from the simulation; a bomb simply falls.
            let ticks = host
                .projectiles()
                .into_iter()
                .flat_map(|projectiles| projectiles.iter())
                .filter(|p| p.attacker == attacker && p.target == target && p.weapon == weapon)
                .map(|p| p.remaining)
                .max();
            let flight = match ticks {
                Some(ticks) => ticks as f32 / sim::TICKS_PER_SECOND as f32,
                None if class == WeaponClass::Bomb => 0.45,
                None => 0.05,
            };
            let distance = from.ground.distance(to.ground);
            let arc = match class {
                WeaponClass::Artillery | WeaponClass::Grenade => (distance * 0.3).min(110.0),
                WeaponClass::Rocket if def.range >= 3_000 => (distance * 0.35).min(150.0),
                WeaponClass::Rocket => (distance * 0.06).min(18.0),
                _ => 0.0,
            };
            painter.commands.spawn((
                BattlefieldVisual,
                Shell {
                    class,
                    target,
                    from: (
                        from.ground,
                        from.lift + if class == WeaponClass::Bomb { 0.0 } else { 2.0 },
                    ),
                    to: (to.ground, to.lift),
                    age: 0.0,
                    flight: flight.max(0.05),
                    arc,
                    trail: 0.0,
                },
                Sprite {
                    image: art.streak.clone(),
                    custom_size: Some(Vec2::ZERO),
                    ..default()
                },
                Transform::from_translation(muzzle.extend(Z_CORE)),
                Visibility::Hidden,
            ));
            continue;
        }
        match class {
            WeaponClass::Rifle | WeaponClass::MachineGun | WeaponClass::Sniper => {
                let (rounds, gap, width) = match class {
                    WeaponClass::MachineGun => (3, 0.07, 1.7),
                    WeaponClass::Sniper => (1, 0.0, 2.2),
                    _ => (1, 0.0, 1.6),
                };
                let mut dice = Dice::new(seed);
                for round in 0..rounds {
                    let delay = gap * round as f32;
                    let stray = Vec2::new(dice.roll(), dice.roll()) * 3.0;
                    if at_muzzle {
                        let scale = if class == WeaponClass::Sniper {
                            1.5
                        } else {
                            1.0
                        };
                        if round == 0 {
                            painter.muzzle_flash(muzzle, dir, scale, from.cell);
                        } else {
                            painter.glow(
                                muzzle,
                                9.0,
                                [1.0, 0.9, 0.62, 1.0],
                                [from.cell; 2],
                                Stroke {
                                    delay,
                                    life: 0.06,
                                    hold: true,
                                    z: Z_CORE,
                                    ..default()
                                },
                            );
                        }
                    }
                    let span = hit + stray - muzzle;
                    let distance = span.length().max(1.0);
                    // A tracer is seen as a short streak crossing the gap, not as a line.
                    const PACE: f32 = 1_300.0;
                    if across {
                        let length = (distance * 0.45).min(26.0);
                        painter.sprite(
                            art.streak.clone(),
                            muzzle + span / distance * length * 0.5,
                            span.y.atan2(span.x),
                            Vec2::new(length, width),
                            [1.0, 0.93, 0.62, 0.95],
                            cells,
                            Stroke {
                                delay,
                                life: ((distance - length) / PACE).max(0.03),
                                velocity: span / distance * PACE,
                                hold: true,
                                z: Z_CORE,
                                ..default()
                            },
                        );
                    }
                    if at_target {
                        let landed = delay + distance / PACE;
                        painter.smoke(
                            hit + stray - Vec2::Y * 4.0,
                            5.0,
                            [0.62, 0.55, 0.42, 0.55],
                            [to.cell; 2],
                            Stroke {
                                delay: landed,
                                life: 0.45,
                                velocity: Vec2::Y * 9.0,
                                growth: Vec2::splat(20.0),
                                ..default()
                            },
                        );
                        painter.glow(
                            hit + stray,
                            5.0,
                            [1.0, 0.9, 0.6, 0.9],
                            [to.cell; 2],
                            Stroke {
                                delay: landed,
                                life: 0.07,
                                hold: true,
                                z: Z_CORE,
                                ..default()
                            },
                        );
                    }
                }
            }
            WeaponClass::Cannon => {
                // A gun without a shell in flight (the mirage tank's): it strikes at once.
                if at_muzzle {
                    painter.muzzle_blast(muzzle, dir, 1.0, from.cell, seed);
                }
                if at_target {
                    painter.explosion(hit - Vec2::Y * 4.0, 16.0, to.cell, seed);
                }
            }
            WeaponClass::Prism => match view {
                ShotView::Line => painter.prism_beam(muzzle + Vec2::Y * 6.0, hit, cells, seed),
                ShotView::AtAttacker => painter.glow(
                    muzzle + Vec2::Y * 6.0,
                    34.0,
                    [0.85, 0.95, 1.0, 0.95],
                    [from.cell; 2],
                    Stroke {
                        life: 0.4,
                        hold: true,
                        z: Z_CORE,
                        ..default()
                    },
                ),
                _ => {
                    painter.glow(
                        hit,
                        46.0,
                        [0.8, 0.94, 1.0, 0.95],
                        [to.cell; 2],
                        Stroke {
                            life: 0.5,
                            hold: true,
                            z: Z_CORE,
                            ..default()
                        },
                    );
                    painter.sparks(hit, 10, 90.0, [0.86, 0.96, 1.0, 1.0], to.cell, seed);
                }
            },
            WeaponClass::Tesla => match view {
                ShotView::Line => {
                    painter.commands.spawn((
                        BattlefieldVisual,
                        Bolt {
                            // From the discharge electrode, above the body of whatever carries it.
                            a: from.body() + dir * (from.reach * 0.4) + Vec2::Y * 9.0,
                            b: hit,
                            cells,
                            left: BOLT_SECS,
                            next: 0.0,
                            seed,
                        },
                    ));
                }
                ShotView::AtAttacker => painter.glow(
                    muzzle,
                    22.0,
                    [0.7, 0.84, 1.0, 0.9],
                    [from.cell; 2],
                    Stroke {
                        life: BOLT_SECS,
                        flicker: 0.6,
                        hold: true,
                        z: Z_CORE,
                        ..default()
                    },
                ),
                _ => {
                    painter.glow(
                        hit,
                        30.0,
                        [0.72, 0.86, 1.0, 0.9],
                        [to.cell; 2],
                        Stroke {
                            life: BOLT_SECS,
                            flicker: 0.6,
                            hold: true,
                            z: Z_CORE,
                            ..default()
                        },
                    );
                    painter.sparks(hit, 8, 110.0, [0.8, 0.9, 1.0, 1.0], to.cell, seed);
                }
            },
            WeaponClass::Flak => {
                if at_muzzle {
                    painter.muzzle_flash(muzzle, dir, 1.3, from.cell);
                }
                if at_target {
                    let mut dice = Dice::new(seed);
                    for burst in 0..3 {
                        let at = hit + Vec2::new(dice.roll(), dice.roll()) * 11.0;
                        let delay = 0.08 * burst as f32;
                        painter.glow(
                            at,
                            13.0,
                            [1.0, 0.8, 0.42, 1.0],
                            [to.cell; 2],
                            Stroke {
                                delay,
                                life: 0.09,
                                hold: true,
                                z: Z_CORE,
                                ..default()
                            },
                        );
                        painter.smoke(
                            at,
                            8.0,
                            [0.13, 0.13, 0.14, 0.75],
                            [to.cell; 2],
                            Stroke {
                                delay,
                                life: 1.0,
                                velocity: Vec2::new(4.0, 3.0),
                                growth: Vec2::splat(17.0),
                                ..default()
                            },
                        );
                    }
                }
            }
            WeaponClass::Radiation => {
                if across {
                    for (width, color, flicker) in [
                        (22.0, [0.42, 1.0, 0.25, 0.24], 0.35),
                        (5.0, [0.78, 1.0, 0.5, 0.8], 0.2),
                    ] {
                        painter.line(
                            muzzle,
                            hit,
                            width,
                            color,
                            cells,
                            Stroke {
                                life: 0.5,
                                taper: 0.5,
                                flicker,
                                hold: true,
                                ..default()
                            },
                        );
                    }
                }
                if at_target {
                    painter.glow(
                        hit,
                        34.0,
                        [0.5, 1.0, 0.3, 0.55],
                        [to.cell; 2],
                        Stroke {
                            life: 0.7,
                            flicker: 0.3,
                            ..default()
                        },
                    );
                    for i in 0..3 {
                        painter.smoke(
                            hit - Vec2::Y * 6.0,
                            10.0,
                            [0.36, 0.8, 0.22, 0.34],
                            [to.cell; 2],
                            Stroke {
                                delay: 0.1 * i as f32,
                                life: 1.6,
                                velocity: Vec2::new(5.0 - 5.0 * i as f32, 8.0),
                                growth: Vec2::splat(20.0),
                                ..default()
                            },
                        );
                    }
                }
            }
            WeaponClass::Psychic | WeaponClass::Sonic | WeaponClass::Chrono => {
                let color = match class {
                    WeaponClass::Psychic => [0.78, 0.42, 1.0, 0.75],
                    WeaponClass::Sonic => [0.7, 0.95, 1.0, 0.6],
                    _ => [0.86, 0.96, 1.0, 0.8],
                };
                if across {
                    // Rings that leave the weapon one after another and widen on their way.
                    let span = hit - muzzle;
                    let angle = span.y.atan2(span.x);
                    for i in 0..6 {
                        let t = (i as f32 + 0.5) / 6.0;
                        painter.sprite(
                            mat.ring.clone(),
                            muzzle + span * t,
                            angle,
                            Vec2::new(5.0, 9.0 + 14.0 * t),
                            color,
                            cells,
                            Stroke {
                                delay: 0.05 * i as f32,
                                life: 0.34,
                                growth: Vec2::new(10.0, 44.0),
                                ..default()
                            },
                        );
                    }
                    if class != WeaponClass::Sonic {
                        painter.line(
                            muzzle,
                            hit,
                            5.0,
                            [color[0], color[1], color[2], 0.3],
                            cells,
                            Stroke {
                                life: 0.4,
                                taper: 0.6,
                                flicker: 0.4,
                                hold: true,
                                ..default()
                            },
                        );
                    }
                }
                if at_target {
                    painter.glow(
                        hit,
                        28.0,
                        color,
                        [to.cell; 2],
                        Stroke {
                            delay: if across { 0.25 } else { 0.0 },
                            life: 0.6,
                            flicker: 0.3,
                            ..default()
                        },
                    );
                    if class == WeaponClass::Chrono {
                        // Time closing on the target: rings that shrink onto it.
                        for i in 0..3 {
                            painter.sprite(
                                mat.ring.clone(),
                                hit,
                                0.0,
                                Vec2::new(46.0, 30.0),
                                color,
                                [to.cell; 2],
                                Stroke {
                                    delay: 0.12 * i as f32,
                                    life: 0.4,
                                    growth: Vec2::new(-95.0, -62.0),
                                    hold: true,
                                    ..default()
                                },
                            );
                        }
                    }
                }
            }
            WeaponClass::Melee => {
                if at_target {
                    let mut dice = Dice::new(seed);
                    for _ in 0..3 {
                        let slash = Vec2::from_angle(dice.unit() * PI) * 7.0;
                        let at = hit + Vec2::new(dice.roll(), dice.roll()) * 3.0;
                        painter.line(
                            at - slash,
                            at + slash,
                            2.0,
                            [1.0, 0.96, 0.9, 0.9],
                            [to.cell; 2],
                            Stroke {
                                life: 0.13,
                                taper: 1.0,
                                hold: true,
                                z: Z_CORE,
                                ..default()
                            },
                        );
                    }
                }
            }
            WeaponClass::Trigger => {
                if at_muzzle {
                    painter.glow(
                        from.body(),
                        12.0,
                        [1.0, 0.25, 0.2, 0.95],
                        [from.cell; 2],
                        Stroke {
                            life: 0.3,
                            flicker: 0.8,
                            hold: true,
                            z: Z_CORE,
                            ..default()
                        },
                    );
                }
            }
            // These always have a projectile (or fall), handled above.
            WeaponClass::Artillery
            | WeaponClass::Rocket
            | WeaponClass::Grenade
            | WeaponClass::Bomb
            | WeaponClass::Torpedo => {}
        }
    }
}

/// Moves the shells in flight, leaves their trails, and bursts them where they land.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn fly_shells(
    mut commands: Commands,
    speed: Res<crate::game_speed::GameClock>,
    sink: Res<CommandSink>,
    host: Res<SimHost>,
    map: Option<Res<MapView>>,
    art: Option<Res<FxArt>>,
    mat: Option<Res<Materials>>,
    altitudes: Res<Altitudes>,
    mut sounds: ResMut<ImpactSounds>,
    live: Query<(), With<Fx>>,
    mut shells: Query<(
        Entity,
        &mut Shell,
        &mut Sprite,
        &mut Transform,
        &mut Visibility,
    )>,
) {
    let (Some(map), Some(art), Some(mat)) = (map, art, mat) else {
        return;
    };
    if shells.is_empty() {
        return;
    }
    if !host.running() {
        for (entity, ..) in &shells {
            commands.entity(entity).despawn();
        }
        return;
    }
    let dt = if sink.paused() { 0.0 } else { speed.delta() };
    let vision = human_vision(host.players());
    let seen = |cell: Option<CellPos>| {
        cell.zip(vision)
            .is_some_and(|(cell, vision)| vision.is_visible(cell))
    };
    let spots = spots(&host, &map, &altitudes);
    let mut painter = Painter {
        commands: &mut commands,
        art: &art,
        mat: &mat,
        left: MAX_FX.saturating_sub(live.iter().count()),
    };
    let mut landed = Vec::new();
    for (entity, mut shell, mut sprite, mut transform, mut visibility) in &mut shells {
        if let Some(spot) = spot_of(&spots, &host, &map, shell.target) {
            shell.to = (spot.ground, spot.lift);
        }
        shell.age += dt;
        let t = (shell.age / shell.flight).clamp(0.0, 1.0);
        let ground = shell.from.0.lerp(shell.to.0, t);
        let cell = pick_cell(ground, map.tile, map.width, map.height);
        let height =
            shell.from.1 + (shell.to.1 - shell.from.1) * t + shell.arc * 4.0 * t * (1.0 - t);
        let at = ground + Vec2::Y * height;
        if t >= 1.0 {
            if seen(cell)
                && let Some(cell) = cell
            {
                let seed = (entity.to_bits() as u32) ^ 0x0B57;
                match shell.class {
                    WeaponClass::Artillery | WeaponClass::Bomb => {
                        painter.explosion(at - Vec2::Y * 6.0, 30.0, cell, seed)
                    }
                    WeaponClass::Grenade => painter.explosion(at - Vec2::Y * 6.0, 13.0, cell, seed),
                    WeaponClass::Torpedo => {
                        // A column of water.
                        for i in 0..5 {
                            painter.smoke(
                                at,
                                10.0,
                                [0.9, 0.95, 1.0, 0.7],
                                [cell; 2],
                                Stroke {
                                    delay: 0.03 * i as f32,
                                    life: 0.8,
                                    velocity: Vec2::new(
                                        4.0 * (i as f32 - 2.0),
                                        34.0 - 5.0 * i as f32,
                                    ),
                                    growth: Vec2::splat(22.0),
                                    ..default()
                                },
                            );
                        }
                        painter.ground_ring(
                            at - Vec2::Y * 8.0,
                            12.0,
                            130.0,
                            [0.9, 0.96, 1.0, 0.6],
                            cell,
                        );
                    }
                    _ => painter.explosion(at - Vec2::Y * 6.0, 19.0, cell, seed),
                }
                sounds.0 = sounds.0.saturating_add(1);
            }
            landed.push(entity);
            continue;
        }
        // It points along its path, which bends with its arc.
        let slope = shell.to.1 - shell.from.1 + shell.arc * 4.0 * (1.0 - 2.0 * t);
        let heading = shell.to.0 - shell.from.0 + Vec2::Y * slope;
        transform.translation = at.extend(Z_CORE);
        transform.rotation = Quat::from_rotation_z(heading.y.atan2(heading.x));
        let (size, color) = match shell.class {
            WeaponClass::Rocket => (Vec2::new(17.0, 5.5), Color::srgba(1.0, 0.82, 0.5, 1.0)),
            WeaponClass::Torpedo => (Vec2::new(12.0, 3.5), Color::srgba(0.9, 0.96, 1.0, 0.8)),
            WeaponClass::Artillery => (Vec2::new(11.0, 3.6), Color::srgba(1.0, 0.86, 0.6, 1.0)),
            WeaponClass::Grenade => (Vec2::new(5.0, 4.0), Color::srgba(0.2, 0.22, 0.18, 1.0)),
            WeaponClass::Bomb => (Vec2::new(9.0, 5.0), Color::srgba(0.16, 0.17, 0.17, 1.0)),
            _ => (Vec2::new(16.0, 3.2), Color::srgba(1.0, 0.92, 0.66, 1.0)),
        };
        sprite.custom_size = Some(size);
        sprite.color = color;
        let shown = seen(cell);
        *visibility = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        shell.trail -= dt;
        if shown
            && shell.trail <= 0.0
            && let Some(cell) = cell
        {
            match shell.class {
                WeaponClass::Rocket => {
                    shell.trail = 0.022;
                    painter.smoke(
                        at,
                        5.0,
                        [0.86, 0.84, 0.8, 0.5],
                        [cell; 2],
                        Stroke {
                            life: 0.7,
                            velocity: Vec2::Y * 4.0,
                            growth: Vec2::splat(15.0),
                            ..default()
                        },
                    );
                    painter.glow(
                        at,
                        9.0,
                        [1.0, 0.7, 0.3, 0.9],
                        [cell; 2],
                        Stroke {
                            life: 0.08,
                            z: Z_CORE,
                            ..default()
                        },
                    );
                }
                WeaponClass::Torpedo => {
                    shell.trail = 0.05;
                    painter.smoke(
                        at - Vec2::Y * 8.0,
                        5.0,
                        [0.9, 0.96, 1.0, 0.45],
                        [cell; 2],
                        Stroke {
                            life: 0.9,
                            growth: Vec2::splat(9.0),
                            ..default()
                        },
                    );
                }
                _ => shell.trail = f32::MAX,
            }
        }
    }
    for entity in landed {
        commands.entity(entity).despawn();
    }
}

/// Redraws every lightning bolt along a new path.
#[allow(clippy::too_many_arguments)]
fn crackle(
    mut commands: Commands,
    speed: Res<crate::game_speed::GameClock>,
    sink: Res<CommandSink>,
    host: Res<SimHost>,
    art: Option<Res<FxArt>>,
    mat: Option<Res<Materials>>,
    live: Query<(), With<Fx>>,
    mut bolts: Query<(Entity, &mut Bolt)>,
) {
    let (Some(art), Some(mat)) = (art, mat) else {
        return;
    };
    if bolts.is_empty() {
        return;
    }
    let dt = if sink.paused() { 0.0 } else { speed.delta() };
    let mut ended = Vec::new();
    let mut painter = Painter {
        commands: &mut commands,
        art: &art,
        mat: &mat,
        left: MAX_FX.saturating_sub(live.iter().count()),
    };
    for (entity, mut bolt) in &mut bolts {
        bolt.left -= dt;
        bolt.next -= dt;
        if bolt.left <= 0.0 || !host.running() {
            ended.push(entity);
            continue;
        }
        if bolt.next <= 0.0 {
            bolt.next = BOLT_JUMP_SECS;
            bolt.seed = bolt
                .seed
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            painter.lightning(bolt.a, bolt.b, bolt.cells, bolt.seed);
        }
    }
    for entity in ended {
        commands.entity(entity).despawn();
    }
}

/// Ages every effect sprite, moves and fades it, and hides it where the human player does not
/// see.
fn animate(
    mut commands: Commands,
    speed: Res<crate::game_speed::GameClock>,
    sink: Res<CommandSink>,
    host: Res<SimHost>,
    mut sprites: Query<(
        Entity,
        &mut Fx,
        &mut Sprite,
        &mut Transform,
        &mut Visibility,
    )>,
) {
    let dt = if sink.paused() { 0.0 } else { speed.delta() };
    let vision = human_vision(host.players());
    for (entity, mut fx, mut sprite, mut transform, mut visibility) in &mut sprites {
        fx.age += dt;
        if fx.age >= fx.life || !host.running() {
            commands.entity(entity).despawn();
            continue;
        }
        let seen = fx.age >= 0.0
            && vision.is_some_and(|vision| fx.cells.iter().all(|cell| vision.is_visible(*cell)));
        let wanted = if seen {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
        if fx.age < 0.0 {
            continue;
        }
        let t = fx.age / fx.life;
        transform.translation += (fx.velocity * dt).extend(0.0);
        let size = (fx.size + fx.growth * fx.age).max(Vec2::ZERO);
        sprite.custom_size = Some(Vec2::new(size.x, size.y * (1.0 - fx.taper * t * t)));
        let fade = if fx.hold {
            1.0 - t * t
        } else {
            (1.0 - t).powf(1.3)
        };
        let waver = 1.0
            - fx.flicker * (0.5 + 0.5 * (fx.age * 150.0 + (entity.to_bits() as u32) as f32).sin());
        sprite.color = Color::srgba(
            fx.color[0],
            fx.color[1],
            fx.color[2],
            (fx.color[3] * fade * waver).clamp(0.0, 1.0),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_weapon_has_the_class_its_name_promises() {
        use WeaponClass::*;
        for (id, instant, class) in [
            ("rifle", true, Rifle),
            ("carbine", true, Rifle),
            ("air_rifle", true, Rifle),
            ("commando_gun", true, Rifle),
            ("scout_gun", true, MachineGun),
            ("raider_gun", true, MachineGun),
            ("sniper_rifle", true, Sniper),
            ("tank_cannon", false, Cannon),
            ("heavy_cannon", false, Cannon),
            ("apocalypse_gun", false, Cannon),
            ("destroyer_gun", false, Cannon),
            ("naval_gun", false, Cannon),
            ("turret_gun", false, Cannon),
            ("mirage_gun", true, Cannon),
            ("artillery_shell", false, Artillery),
            ("grand_shell", false, Artillery),
            ("infantry_rocket", false, Rocket),
            ("rocket_salvo", false, Rocket),
            ("tower_rocket", false, Rocket),
            ("v3_missile", false, Rocket),
            ("aegis_missile", false, Rocket),
            ("dread_missile", false, Rocket),
            ("carrier_strike", false, Rocket),
            ("torpedo", false, Torpedo),
            ("grenade", false, Grenade),
            ("flak_weapon", true, Flak),
            ("prism_beam", true, Prism),
            ("tesla_weapon", true, Tesla),
            ("radiation_weapon", true, Radiation),
            ("control_beam", true, Psychic),
            ("chrono_beam", true, Chrono),
            ("sonic", true, Sonic),
            ("kirov_bomb", true, Bomb),
            ("dog_bite", true, Melee),
            ("suicide_trigger", true, Trigger),
        ] {
            assert_eq!(classify(id, instant), class, "{id}");
        }
        // Weapons of other data files fall back on whether they have a projectile.
        assert_eq!(classify("pea_shooter", true), Rifle);
        assert_eq!(classify("pea_shooter", false), Cannon);
    }

    /// Every weapon of the shipped combat file is classified from its own terms, and the two
    /// the request names look and sound like nothing else.
    #[test]
    fn the_shipped_combat_file_classifies_light_and_electricity() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let (loaded, _) = crate::units::load_match_data(&root).expect("match data");
        let class = |id: &str| {
            let kind = loaded.definitions.weapon_kind(id).expect(id);
            class_of(&loaded.definitions, kind)
        };
        assert_eq!(class("prism_beam"), WeaponClass::Prism);
        assert_eq!(class("tesla_weapon"), WeaponClass::Tesla);
        assert_eq!(class("tank_cannon"), WeaponClass::Cannon);
        assert_eq!(class("rifle"), WeaponClass::Rifle);
        for weapon in loaded.definitions.weapons() {
            let class = classify(&weapon.id, weapon.projectile_speed == 0);
            // A weapon with a projectile is never drawn as an instant ray, and the reverse.
            let ray = matches!(
                class,
                WeaponClass::Prism | WeaponClass::Tesla | WeaponClass::Radiation
            );
            assert!(!(ray && weapon.projectile_speed > 0), "{}", weapon.id);
        }
    }

    #[test]
    fn a_bolt_joins_its_ends_and_strays_between_them() {
        let (a, b) = (Vec2::new(10.0, 20.0), Vec2::new(210.0, -40.0));
        let path = bolt_path(a, b, 7);
        assert_eq!(path.first(), Some(&a));
        assert_eq!(path.last(), Some(&b));
        assert!(path.len() >= 4);
        let normal = (b - a).perp().normalize();
        let stray = path
            .iter()
            .map(|p| (*p - a).dot(normal).abs())
            .fold(0.0f32, f32::max);
        assert!(stray > 2.0, "a bolt is not a straight line: {stray}");
        assert!(stray < 40.0, "a bolt stays near its line: {stray}");
        // Another seed, another path: the bolt jumps.
        assert_ne!(bolt_path(a, b, 8), path);
        // A very short bolt still has a middle.
        assert!(bolt_path(a, a + Vec2::X * 3.0, 1).len() >= 4);
    }
}
