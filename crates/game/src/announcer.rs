//! Original synthesized announcer motifs paired with plain text. Per-category eight-second
//! cooldown and one-second spacing bound bursts; all input is owner-scoped or explicitly public.
use crate::{
    audio::{Channel, Pcm, SAMPLE_RATE_HZ, SoundEffect, VolumeSettings},
    sim_host::SimHost,
    units::HUMAN,
};
use bevy::{audio::Volume, prelude::*};
use sim::{Command, ProductionItem, SimEvent};
use std::collections::{BTreeMap, VecDeque};
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Cue {
    UnitReady,
    ConstructionComplete,
    OnHold,
    CannotDeploy,
    BaseAttack,
    UnitLost,
    LowPower,
    InsufficientFunds,
    SuperweaponBuilt,
    SuperweaponReady,
    SuperweaponLaunch,
    RadarOnline,
    RadarOffline,
}
impl Cue {
    pub const ALL: [Self; 13] = [
        Self::UnitReady,
        Self::ConstructionComplete,
        Self::OnHold,
        Self::CannotDeploy,
        Self::BaseAttack,
        Self::UnitLost,
        Self::LowPower,
        Self::InsufficientFunds,
        Self::SuperweaponBuilt,
        Self::SuperweaponReady,
        Self::SuperweaponLaunch,
        Self::RadarOnline,
        Self::RadarOffline,
    ];
    pub fn text(self) -> &'static str {
        match self {
            Self::UnitReady => "Unit ready",
            Self::ConstructionComplete => "Construction complete",
            Self::OnHold => "Production on hold",
            Self::CannotDeploy => "Cannot deploy here",
            Self::BaseAttack => "Base under attack",
            Self::UnitLost => "Unit lost",
            Self::LowPower => "Low power",
            Self::InsufficientFunds => "Insufficient funds",
            Self::SuperweaponBuilt => "Superweapon constructed",
            Self::SuperweaponReady => "Superweapon ready",
            Self::SuperweaponLaunch => "Superweapon launched",
            Self::RadarOnline => "Radar online",
            Self::RadarOffline => "Radar offline",
        }
    }
}
pub fn event_cue(e: &SimEvent) -> Option<Cue> {
    match e {
        SimEvent::UnitSpawned { player, .. } | SimEvent::UnitPlaced { player, .. }
            if *player == HUMAN =>
        {
            Some(Cue::UnitReady)
        }
        SimEvent::ProductionComplete {
            player,
            item: ProductionItem::Building(_),
        } if *player == HUMAN => Some(Cue::ConstructionComplete),
        SimEvent::UnitDestroyed { player, .. } if *player == HUMAN => Some(Cue::UnitLost),
        SimEvent::BuildingDestroyed { player, .. } if *player == HUMAN => Some(Cue::BaseAttack),
        SimEvent::CommandRejected {
            player,
            command:
                Command::Deploy { .. } | Command::PlaceBuilding { .. } | Command::PlaceUnit { .. },
            ..
        } if *player == HUMAN => Some(Cue::CannotDeploy),
        SimEvent::SuperweaponBuilt { .. } => Some(Cue::SuperweaponBuilt),
        SimEvent::SuperweaponReady { player, .. } if *player == HUMAN => {
            Some(Cue::SuperweaponReady)
        }
        SimEvent::SuperweaponLaunched { .. } => Some(Cue::SuperweaponLaunch),
        _ => None,
    }
}
fn next_charge(cost: i64, time: u32, progress: u32, super_me: bool) -> i64 {
    let time = time.max(1);
    if super_me {
        i64::from(progress.saturating_add(1) == time)
    } else {
        cost / i64::from(time)
            + if progress.saturating_add(1) == time {
                cost % i64::from(time)
            } else {
                0
            }
    }
}

/// Deterministic three-note motif with an attack/release envelope. No recordings or speech assets.
pub fn synthesize(cue: Cue) -> Vec<f32> {
    let n = (SAMPLE_RATE_HZ as usize) * 3 / 5;
    let step = Cue::ALL.iter().position(|c| *c == cue).unwrap_or(0) as f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE_HZ as f32;
            let note = (t / 0.2).floor();
            let f = 330.0 + step * 23.0 + note * 70.0;
            let local = t % 0.2;
            let envelope = (local / 0.015).min(1.0) * ((0.2 - local) / 0.06).clamp(0.0, 1.0);
            (std::f32::consts::TAU * f * t).sin() * envelope * 0.3
        })
        .collect()
}
#[derive(Resource, Default)]
struct Announcer {
    last: BTreeMap<Cue, f64>,
    queue: VecDeque<Cue>,
    next: f64,
    low: bool,
    radar: Option<bool>,
    held: bool,
    hp: BTreeMap<sim::SimId, i32>,
    tick: u64,
}
impl Announcer {
    fn enqueue(&mut self, cue: Cue, now: f64) {
        if self.queue.len() < 8
            && !self.queue.contains(&cue)
            && self.last.get(&cue).is_none_or(|t| now - *t >= 8.0)
        {
            self.queue.push_back(cue);
        }
    }
    fn pop(&mut self, now: f64) -> Option<Cue> {
        if now < self.next {
            return None;
        }
        let c = self.queue.pop_front()?;
        self.next = now + 1.0;
        self.last.insert(c, now);
        Some(c)
    }
}
#[derive(Resource)]
struct Bank(BTreeMap<Cue, Handle<Pcm>>);
pub struct AnnouncerPlugin;
impl Plugin for AnnouncerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Announcer>()
            .add_systems(Startup, setup)
            .add_systems(Update, announce.after(crate::skirmish::update_screens));
    }
}
fn setup(mut commands: Commands, mut sounds: ResMut<Assets<Pcm>>) {
    commands.insert_resource(Bank(
        Cue::ALL
            .into_iter()
            .map(|c| (c, sounds.add(Pcm::new(synthesize(c)))))
            .collect(),
    ));
}
#[allow(clippy::too_many_arguments)]
fn announce(
    mut commands: Commands,
    host: Res<SimHost>,
    mut events: MessageReader<SimEvent>,
    time: Res<Time<Real>>,
    volume: Res<VolumeSettings>,
    bank: Option<Res<Bank>>,
    mut state: ResMut<Announcer>,
) {
    if !host.running() {
        *state = Announcer::default();
        events.clear();
        return;
    }
    let now = time.elapsed_secs_f64();
    if host.tick() < state.tick {
        *state = Announcer::default();
    }
    state.tick = host.tick();
    for e in events.read() {
        if let Some(c) = event_cue(e) {
            state.enqueue(c, now);
        }
    }
    if let Some(p) = host.players().and_then(|p| p.get(HUMAN)) {
        if p.low_power() && !state.low {
            state.enqueue(Cue::LowPower, now);
        }
        state.low = p.low_power();
        if state.radar.is_some_and(|r| r != p.radar()) {
            state.enqueue(
                if p.radar() {
                    Cue::RadarOnline
                } else {
                    Cue::RadarOffline
                },
                now,
            );
        }
        state.radar = Some(p.radar());
        let held = p.queues().any(|(_, q)| {
            q.first().is_some_and(|q| {
                host.definitions()
                    .and_then(|d| d.production(q.item))
                    .is_some_and(|d| {
                        q.progress < d.build_time
                            && p.credits()
                                < next_charge(d.cost, d.build_time, q.progress, p.super_me())
                    })
            })
        });
        if held && !state.held {
            state.enqueue(Cue::OnHold, now);
            state.enqueue(Cue::InsufficientFunds, now);
        }
        state.held = held;
    }
    let hp: BTreeMap<_, _> = host
        .world()
        .iter_entities()
        .filter_map(|e| {
            if e.get::<sim::Owner>()?.0 != HUMAN || e.get::<sim::Building>().is_none() {
                return None;
            }
            Some((*e.get::<sim::SimId>()?, e.get::<sim::Health>()?.hit_points))
        })
        .collect();
    if hp
        .iter()
        .any(|(id, h)| state.hp.get(id).is_some_and(|old| h < old))
    {
        state.enqueue(Cue::BaseAttack, now);
    }
    state.hp = hp;
    if let Some(c) = state.pop(now) {
        commands.queue(move |world: &mut World| {
            crate::notices::announce(world, c.text().into(), Color::srgb(1.0, 0.85, 0.3))
        });
        if let Some(bank) = bank
            && let Some(handle) = bank.0.get(&c)
            && volume.gain(Channel::Effects) > 0.0
        {
            commands.spawn((
                AudioPlayer(handle.clone()),
                PlaybackSettings::DESPAWN
                    .with_volume(Volume::Linear(volume.gain(Channel::Effects))),
                SoundEffect { expires: now + 1.6 },
            ));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_original_cue_is_bounded_distinct_and_has_text() {
        let mut all = Vec::new();
        for c in Cue::ALL {
            let s = synthesize(c);
            assert_eq!(s, synthesize(c));
            assert!(s.iter().all(|v| v.is_finite() && v.abs() <= 1.0));
            assert!(s.iter().any(|v| v.abs() > 0.1));
            assert!(!c.text().is_empty());
            assert!(!all.contains(&s));
            all.push(s);
        }
    }
    #[test]
    fn announcer_rate_limits_bursts_and_resets_categories() {
        let mut a = Announcer::default();
        a.enqueue(Cue::UnitLost, 0.0);
        a.enqueue(Cue::UnitLost, 0.0);
        a.enqueue(Cue::LowPower, 0.0);
        assert_eq!(a.pop(0.0), Some(Cue::UnitLost));
        assert_eq!(a.pop(0.5), None);
        assert_eq!(a.pop(1.0), Some(Cue::LowPower));
        a.enqueue(Cue::UnitLost, 2.0);
        assert!(a.queue.is_empty());
        a.enqueue(Cue::UnitLost, 8.0);
        assert_eq!(a.pop(8.0), Some(Cue::UnitLost));
    }
}

#[cfg(test)]
mod acceptance {
    use super::*;
    #[test]
    fn ra7_announcements_filter_private_events_and_keep_public_warnings() {
        assert_eq!(
            event_cue(&SimEvent::UnitDestroyed {
                player: sim::PlayerId(1),
                unit: sim::SimId(9)
            }),
            None
        );
        assert_eq!(
            event_cue(&SimEvent::SuperweaponReady {
                player: sim::PlayerId(1),
                role: sim::Role::Nuke
            }),
            None
        );
        assert_eq!(
            event_cue(&SimEvent::SuperweaponReady {
                player: HUMAN,
                role: sim::Role::Nuke
            }),
            Some(Cue::SuperweaponReady)
        );
        assert_eq!(
            event_cue(&SimEvent::SuperweaponBuilt {
                player: sim::PlayerId(1),
                role: sim::Role::Nuke
            }),
            Some(Cue::SuperweaponBuilt)
        );
        assert_eq!(
            event_cue(&SimEvent::SuperweaponLaunched {
                player: sim::PlayerId(1),
                role: sim::Role::Nuke
            }),
            Some(Cue::SuperweaponLaunch)
        );
        assert_eq!(next_charge(10, 3, 0, false), 3);
        assert_eq!(next_charge(10, 3, 2, false), 4);
        assert_eq!(next_charge(10, 3, 0, true), 0);
        assert_eq!(next_charge(10, 3, 2, true), 1);
    }
    #[test]
    fn ra7_simulation_event_reaches_the_actual_notice_column() {
        let mut app = App::new();
        app.insert_resource(crate::sim_host::tests::demo_host())
            .insert_resource(Time::<Real>::default())
            .init_resource::<VolumeSettings>()
            .init_resource::<Announcer>()
            .init_resource::<bevy::ecs::message::Messages<SimEvent>>()
            .add_systems(Update, announce);
        app.world_mut().spawn(crate::notices::NoticeColumn);
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<SimEvent>>()
            .write(SimEvent::UnitSpawned {
                player: HUMAN,
                unit: sim::SimId(99),
                building: sim::SimId(2),
            });
        app.update();
        assert!(
            app.world_mut()
                .query::<&Text>()
                .iter(app.world())
                .any(|t| t.0 == "Unit ready")
        );
    }
}
