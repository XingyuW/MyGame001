//! Veterancy (RA4, docs/RA2_PARITY_PLAN.md section 4.5; docs/DECISIONS.md R47): experience,
//! the veteran and the elite rank, and the multipliers a unit carries.
//!
//! Only with a rules file, and only for units of a type with the arsenal's `trainable` trait:
//! each has an [`Experience`] (given to it in the `Economy` stage of the first tick it exists
//! in). Buildings and untrainable units have none and gain nothing.
//!
//! **Experience.** Whoever strikes the blow that destroys a unit or building of an enemy gains
//! the victim's cost (the economy file's, whatever was paid for it): the unit whose shot,
//! splash or crushing it was, and for a spawned missile or aircraft the unit that launched it.
//! Nothing is gained for a unit or building of its own player or of an ally, and nothing for
//! scenery, which has no cost. A unit is a veteran (rank 1) when `value × 1000 ≥ cost ×
//! veteran_ratio_pm` with its own type's cost, and elite (rank 2) at twice that, both inclusive;
//! `veteran_cap` is the highest rank.
//!
//! **What a rank grants.** A veteran has the abilities of its type's `veteran_abilities`, an
//! elite unit those and its type's `elite_abilities`; an ability applies once however often it
//! is listed. They set the unit's rank multipliers ([`Experience::rank`]: `firepower` from the
//! rules' `combat_pm`, `armour` from `armor_pm`, `reload` from `rof_pm`, `speed` from
//! `speed_pm`) and whether it heals itself. `sight` grants nothing with the rules' `sight_pm`
//! of 0 (an assumption), and `scatter` is not modelled. An elite unit fires its elite weapons.
//!
//! **The multipliers** are state of the unit, in permille, and two sets multiply: those of the
//! rank, and [`Experience::bonus`], which nothing sets yet (the crates of RA5). The simulation
//! reads only the products ([`Experience::firepower_pm`] and its like):
//!
//! * damage dealt: `base × firepower_pm` inside the damage equation
//!   ([`impact_damage`](crate::impact_damage));
//! * damage taken: the damage of a hit `× 1000 / armour_pm`, rounded down;
//! * reload: `max(1, reload × reload_pm / 1000)` ticks after a volley;
//! * speed: `max(1, speed × speed_pm / 1000)` subcells per tick, at most `MAX_UNIT_SPEED`
//!   ([`scaled_speed`]), before the slowing of a damaged vehicle.
//!
//! [`promote`] and [`set_bonus`] are how another rule gives a unit a rank or a multiplier.

use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::query::Without;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;

use crate::arsenal::Ability;
use crate::combat::Weapon;
use crate::command::PlayerId;
use crate::definitions::{Definitions, UnitKind};
use crate::economy::Players;
use crate::economy_defs::ProductionItem;
use crate::event::SimEvent;
use crate::hash::Fnv1a64;
use crate::id::SimId;
use crate::unit::{Owner, Unit};

/// The rank of an elite unit, the highest there is.
pub const ELITE: u8 = 2;

/// The rank of a veteran.
pub const VETERAN: u8 = 1;

/// Multipliers of a unit, in permille (1000: unchanged).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Multipliers {
    /// Of the damage its weapons deal.
    pub firepower_pm: u32,
    /// By which the damage it takes is divided.
    pub armour_pm: u32,
    /// Of the reload of its weapons.
    pub reload_pm: u32,
    /// Of its speed.
    pub speed_pm: u32,
}

impl Multipliers {
    /// The multipliers that change nothing.
    pub const NONE: Multipliers = Multipliers {
        firepower_pm: 1000,
        armour_pm: 1000,
        reload_pm: 1000,
        speed_pm: 1000,
    };

    fn fields(&self) -> [u32; 4] {
        [
            self.firepower_pm,
            self.armour_pm,
            self.reload_pm,
            self.speed_pm,
        ]
    }
}

impl Default for Multipliers {
    fn default() -> Self {
        Self::NONE
    }
}

/// The experience of a trainable unit, its rank, and its multipliers.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Experience {
    /// The summed cost of what the unit has destroyed, in credits.
    pub value: i64,
    /// 0, [`VETERAN`] or [`ELITE`].
    pub rank: u8,
    /// What the abilities of its rank multiply; set whenever the rank changes.
    pub ranked: Multipliers,
    /// What else multiplies: nothing by itself. Set through [`set_bonus`].
    pub bonus: Multipliers,
    /// Whether an ability of its rank makes it heal itself.
    pub self_heal: bool,
}

/// `a × b / 1000` in permille, rounded down, at least 1.
fn product(a: u32, b: u32) -> u32 {
    let product = u64::from(a) * u64::from(b) / 1000;
    u32::try_from(product).unwrap_or(u32::MAX).max(1)
}

impl Experience {
    /// The multiplier of the damage the unit's weapons deal.
    pub fn firepower_pm(&self) -> u32 {
        product(self.ranked.firepower_pm, self.bonus.firepower_pm)
    }

    /// The divisor of the damage the unit takes.
    pub fn armour_pm(&self) -> u32 {
        product(self.ranked.armour_pm, self.bonus.armour_pm)
    }

    /// The multiplier of the reload of the unit's weapons.
    pub fn reload_pm(&self) -> u32 {
        product(self.ranked.reload_pm, self.bonus.reload_pm)
    }

    /// The multiplier of the unit's speed.
    pub fn speed_pm(&self) -> u32 {
        product(self.ranked.speed_pm, self.bonus.speed_pm)
    }

    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        hasher.write(&self.value.to_le_bytes());
        hasher.write(&[self.rank, u8::from(self.self_heal)]);
        for value in self.ranked.fields().into_iter().chain(self.bonus.fields()) {
            hasher.write_u32(value);
        }
    }
}

/// The reload of `ticks` for a unit whose reload multiplier is `reload_pm`: rounded down, at
/// least 1 (docs/RA2_PARITY_PLAN.md section 4.4).
pub fn reload_ticks(ticks: u32, reload_pm: u32) -> u32 {
    let scaled = u64::from(ticks) * u64::from(reload_pm) / 1000;
    u32::try_from(scaled).unwrap_or(u32::MAX).max(1)
}

/// The speed of a unit of type speed `speed` whose speed multiplier is `speed_pm`: rounded
/// down, at least 1 and at most [`MAX_UNIT_SPEED`](crate::MAX_UNIT_SPEED).
pub fn scaled_speed(speed: u16, speed_pm: u32) -> u16 {
    let scaled = u64::from(speed) * u64::from(speed_pm) / 1000;
    u16::try_from(scaled.clamp(1, u64::from(crate::MAX_UNIT_SPEED))).unwrap_or(speed)
}

/// The experience a unit of a type that costs `cost` needs for `rank` with the rules'
/// `veteran_ratio_pm`: `rank × cost × ratio / 1000`, rounded up, so that a unit with exactly
/// this much has at least the requested rank. Zero cost/ratio normalize to one, as in
/// [`rank_of`], so explicit promotion's stored experience agrees with its computed threshold.
pub fn experience_for(rank: u8, cost: i64, veteran_ratio_pm: i32) -> i64 {
    let needed = i128::from(rank) * i128::from(cost.max(1)) * i128::from(veteran_ratio_pm.max(1));
    i64::try_from((needed + 999) / 1000).unwrap_or(i64::MAX)
}

/// The rank of a unit of a type that costs `cost` with the experience `value`
/// (docs/RA2_PARITY_PLAN.md section 4.5): 1 when `value × 1000 ≥ cost × veteran_ratio_pm`, 2
/// when it is at least twice that, and never above `cap`. Computed in `i128` (invariant I12).
pub fn rank_of(value: i64, cost: i64, veteran_ratio_pm: i32, cap: i32) -> u8 {
    let scaled = i128::from(value) * 1000;
    let step = i128::from(cost.max(1)) * i128::from(veteran_ratio_pm.max(1));
    let rank = if scaled >= 2 * step {
        ELITE
    } else if scaled >= step {
        VETERAN
    } else {
        0
    };
    rank.min(u8::try_from(cap.clamp(0, i32::from(ELITE))).unwrap_or(0))
}

/// A blow that destroyed a unit or building in this tick: who struck it, whose the victim was,
/// and what the victim costs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Kill {
    pub(crate) killer: SimId,
    pub(crate) victim_owner: PlayerId,
    pub(crate) cost: i64,
}

/// The kills of the tick, in the order they happened; empty between ticks ([`award`] takes
/// them in the `Cleanup` stage).
#[derive(Resource, Clone, Debug, Default)]
pub(crate) struct Kills(pub(crate) Vec<Kill>);

impl Kills {
    /// Records that `killer` destroyed something of `victim_owner` that costs `cost`.
    pub(crate) fn record(&mut self, killer: SimId, victim_owner: PlayerId, cost: i64) {
        self.0.push(Kill {
            killer,
            victim_owner,
            cost,
        });
    }

    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        if self.0.is_empty() {
            return;
        }
        hasher.write(b"kills");
        hasher.write_u64(self.0.len() as u64);
        for kill in &self.0 {
            hasher.write_u32(kill.killer.0);
            hasher.write(&[kill.victim_owner.0]);
            hasher.write(&kill.cost.to_le_bytes());
        }
    }
}

/// The cost of `item` as experience for whoever destroys it.
pub(crate) fn worth(definitions: &Definitions, item: ProductionItem) -> i64 {
    definitions.production(item).map_or(0, |terms| terms.cost)
}

/// Sets `experience` to `rank` of a unit of type `kind`: the rank multipliers and the self
/// healing of the abilities of that rank.
fn rank_terms(definitions: &Definitions, kind: UnitKind, rank: u8, experience: &mut Experience) {
    let Some(rules) = definitions.rules().map(|rules| rules.veterancy) else {
        return;
    };
    let pm = |ability: Ability, value: i32| {
        if definitions.has_ability(kind, rank, ability) {
            u32::try_from(value.max(1)).unwrap_or(1000)
        } else {
            1000
        }
    };
    experience.rank = rank;
    experience.ranked = Multipliers {
        firepower_pm: pm(Ability::Firepower, rules.combat_pm),
        armour_pm: pm(Ability::Stronger, rules.armor_pm),
        reload_pm: pm(Ability::Rof, rules.rof_pm),
        speed_pm: pm(Ability::Faster, rules.speed_pm),
    };
    experience.self_heal = definitions.has_ability(kind, rank, Ability::SelfHeal);
}

/// Gives the unit `entity` of type `kind` the rank `rank`: its multipliers, and at elite rank
/// the elite weapons of its type in the places of those it carries by its type (a weapon a
/// passenger gave it stays).
fn apply_rank(world: &mut World, entity: Entity, kind: UnitKind, rank: u8) {
    let definitions = world.resource::<Definitions>().clone();
    let mut experience = world.get::<Experience>(entity).copied().unwrap_or_default();
    rank_terms(&definitions, kind, rank, &mut experience);
    world.entity_mut(entity).insert(experience);
    if rank < ELITE {
        return;
    }
    let Some(terms) = definitions.unit_combat(kind) else {
        return;
    };
    let [primary, secondary] = terms.weapons(true);
    if let Some(mut weapon) = world.get_mut::<Weapon>(entity) {
        if terms.weapon == Some(weapon.kind)
            && let Some(primary) = primary
        {
            if weapon.volley == weapon.kind && weapon.burst_left == 0 {
                weapon.volley = primary;
            }
            weapon.kind = primary;
        }
        if weapon.secondary == terms.secondary && secondary.is_some() {
            weapon.secondary = secondary;
        }
    }
}

/// Gives the unit `entity` of type `kind`, which another unit launched, the rank `rank` of
/// that unit, without an event: its experience is what the rank needs. Nothing for an
/// untrainable type.
pub(crate) fn inherit(world: &mut World, entity: Entity, kind: UnitKind, rank: u8) {
    let definitions = world.resource::<Definitions>().clone();
    let Some(rules) = definitions.rules().map(|rules| rules.veterancy) else {
        return;
    };
    if rank == 0 || !definitions.trainable(kind) {
        return;
    }
    let cost = worth(&definitions, ProductionItem::Unit(kind));
    world.entity_mut(entity).insert(Experience {
        value: experience_for(rank, cost, rules.veteran_ratio_pm),
        ..Experience::default()
    });
    apply_rank(world, entity, kind, rank);
}

/// Gives the unit `entity` of type `kind` the weapons of the rank its [`Experience`] holds:
/// for a unit whose weapons were set anew (a passenger that leaves its transport).
pub(crate) fn rearm(world: &mut World, entity: Entity, kind: UnitKind) {
    if let Some(experience) = world.get::<Experience>(entity).copied() {
        apply_rank(world, entity, kind, experience.rank);
    }
}

/// The trainable unit with the id `unit`: its entity, owner and type.
fn trainee(world: &mut World, unit: SimId) -> Option<(Entity, PlayerId, UnitKind)> {
    let mut query = world.query::<(Entity, &SimId, &Owner, &Unit)>();
    let found = query
        .iter(world)
        .find(|(_, id, ..)| **id == unit)
        .map(|(entity, _, owner, unit)| (entity, owner.0, unit.kind))?;
    world
        .resource::<Definitions>()
        .trainable(found.2)
        .then_some(found)
}

/// Raises the unit `unit` to at least `rank` (1 veteran, 2 elite; at most the rules'
/// `veteran_cap`): its experience becomes what the rank needs, unless it has more, and it gets
/// what the rank grants, with [`SimEvent::RankGained`]. Returns whether the rank rose; `false`
/// for a unit that does not exist, is not trainable, or has the rank already.
///
/// For rules that promote a unit without a kill (the veteran crate of RA5, the veteran
/// production of RA6). It changes simulation state, so it may be called only from a system of
/// [`SimSchedule`](crate::SimSchedule) or from a scenario setup between ticks (invariant I5).
pub fn promote(world: &mut World, unit: SimId, rank: u8) -> bool {
    let Some((entity, owner, kind)) = trainee(world, unit) else {
        return false;
    };
    let definitions = world.resource::<Definitions>().clone();
    let Some(rules) = definitions.rules().map(|rules| rules.veterancy) else {
        return false;
    };
    let rank = rank.min(u8::try_from(rules.veteran_cap.clamp(0, i32::from(ELITE))).unwrap_or(0));
    let mut experience = world.get::<Experience>(entity).copied().unwrap_or_default();
    if rank <= experience.rank {
        return false;
    }
    let cost = worth(&definitions, ProductionItem::Unit(kind));
    experience.value = experience
        .value
        .max(experience_for(rank, cost, rules.veteran_ratio_pm));
    world.entity_mut(entity).insert(experience);
    apply_rank(world, entity, kind, rank);
    world
        .resource_mut::<Messages<SimEvent>>()
        .write(SimEvent::RankGained {
            player: owner,
            unit,
            rank,
        });
    true
}

/// Sets the bonus multipliers of the unit `unit` ([`Experience::bonus`]), which multiply with
/// those of its rank. Returns `false` for a unit that does not exist or is not trainable.
///
/// For rules that strengthen a unit otherwise than by rank (the armour, firepower and speed
/// crates of RA5). The same restriction as for [`promote`] applies (invariant I5).
pub fn set_bonus(world: &mut World, unit: SimId, bonus: Multipliers) -> bool {
    let Some((entity, ..)) = trainee(world, unit) else {
        return false;
    };
    let mut experience = world.get::<Experience>(entity).copied().unwrap_or_default();
    experience.bonus = bonus;
    world.entity_mut(entity).insert(experience);
    true
}

/// Gives every trainable unit that has none yet its [`Experience`] (the `Economy` stage; with
/// the rules' `initial_veteran` it starts as a veteran).
pub(crate) fn enlist(world: &mut World) {
    let Some(rules) = world
        .resource::<Definitions>()
        .rules()
        .map(|rules| rules.veterancy)
    else {
        return;
    };
    let mut query = world.query_filtered::<(Entity, &SimId, &Unit), Without<Experience>>();
    let mut recruits: Vec<(SimId, Entity)> = query
        .iter(world)
        .filter(|(_, _, unit)| world.resource::<Definitions>().trainable(unit.kind))
        .map(|(entity, id, _)| (*id, entity))
        .collect();
    recruits.sort_unstable();
    for (id, entity) in recruits {
        world.entity_mut(entity).insert(Experience::default());
        if rules.initial_veteran == 1 {
            promote(world, id, VETERAN);
        }
    }
}

/// Credits the kills of the tick, in the order they happened (the `Cleanup` stage, before the
/// dead are removed): the killer, or the unit that launched it, gains the victim's cost if the
/// victim belonged to an enemy, and rises in rank when it has enough.
pub(crate) fn award(world: &mut World) {
    let kills = std::mem::take(&mut world.resource_mut::<Kills>().0);
    if kills.is_empty() {
        return;
    }
    let definitions = world.resource::<Definitions>().clone();
    let Some(rules) = definitions.rules().map(|rules| rules.veterancy) else {
        return;
    };
    for kill in kills {
        // A spawned missile or aircraft kills for the unit that launched it.
        let killer = world
            .resource::<crate::SpecialState>()
            .spawned
            .get(&kill.killer)
            .copied()
            .unwrap_or(kill.killer);
        let Some((entity, owner, kind)) = trainee(world, killer) else {
            // A blocked parasite escape retains its identity and earned experience off-map.
            let pending = world
                .resource::<crate::SpecialState>()
                .parasites
                .iter()
                .find(|(_, p)| p.passenger.id == killer)
                .map(|(host, p)| (*host, p.owner, p.passenger.kind));
            if let Some((host, owner, kind)) = pending
                && definitions.trainable(kind)
                && world
                    .resource::<Players>()
                    .hostile(owner, kill.victim_owner)
            {
                let mut state = world.resource_mut::<crate::SpecialState>();
                if let Some(p) = state.parasites.get_mut(&host) {
                    let xp = p.passenger.experience.get_or_insert_default();
                    xp.value = xp.value.saturating_add(kill.cost.max(0));
                    let rank = rank_of(
                        xp.value,
                        worth(&definitions, ProductionItem::Unit(kind)),
                        rules.veteran_ratio_pm,
                        rules.veteran_cap,
                    );
                    rank_terms(&definitions, kind, rank, xp);
                }
            }
            continue;
        };
        if !world
            .resource::<Players>()
            .hostile(owner, kill.victim_owner)
        {
            continue;
        }
        let mut experience = world.get::<Experience>(entity).copied().unwrap_or_default();
        experience.value = experience.value.saturating_add(kill.cost.max(0));
        let cost = worth(&definitions, ProductionItem::Unit(kind));
        let rank = rank_of(
            experience.value,
            cost,
            rules.veteran_ratio_pm,
            rules.veteran_cap,
        );
        let before = experience.rank;
        world.entity_mut(entity).insert(experience);
        if rank > before {
            apply_rank(world, entity, kind, rank);
            world
                .resource_mut::<Messages<SimEvent>>()
                .write(SimEvent::RankGained {
                    player: owner,
                    unit: killer,
                    rank,
                });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Section 4.5: the thresholds are inclusive, and the cap holds.
    #[test]
    fn the_rank_follows_the_thresholds_inclusively() {
        // A type of 700 credits with the ratio 3.0: veteran at 2100, elite at 4200.
        assert_eq!(rank_of(0, 700, 3000, 2), 0);
        assert_eq!(rank_of(2099, 700, 3000, 2), 0);
        assert_eq!(rank_of(2100, 700, 3000, 2), VETERAN);
        assert_eq!(rank_of(4199, 700, 3000, 2), VETERAN);
        assert_eq!(rank_of(4200, 700, 3000, 2), ELITE);
        assert_eq!(rank_of(i64::MAX, 700, 3000, 2), ELITE);
        assert_eq!(rank_of(4200, 700, 3000, 1), VETERAN, "the cap");
        assert_eq!(rank_of(4200, 700, 3000, 0), 0);
        // A ratio that does not divide: 2.5 x 333 = 832.5, so 833 is the first veteran.
        assert_eq!(rank_of(832, 333, 2500, 2), 0);
        assert_eq!(rank_of(833, 333, 2500, 2), VETERAN);
        assert_eq!(experience_for(VETERAN, 333, 2500), 833);
        assert_eq!(experience_for(ELITE, 700, 3000), 4200);
        assert_eq!(
            rank_of(experience_for(ELITE, 333, 2500), 333, 2500, 2),
            ELITE
        );
        assert_eq!(
            rank_of(experience_for(ELITE, 333, 2500) - 1, 333, 2500, 2),
            VETERAN
        );
        assert_eq!(rank_of(i64::MAX, i64::MAX, i32::MAX, 2), 0, "no overflow");
    }

    #[test]
    fn multipliers_multiply_and_scale_reload_and_speed_rounded_down() {
        let mut experience = Experience::default();
        assert_eq!(experience.firepower_pm(), 1000);
        experience.ranked.firepower_pm = 1100;
        experience.bonus.firepower_pm = 2000;
        assert_eq!(experience.firepower_pm(), 2200);
        experience.ranked.armour_pm = 1500;
        experience.bonus.armour_pm = 1500;
        assert_eq!(experience.armour_pm(), 2250);
        assert_eq!(reload_ticks(80, 600), 48);
        assert_eq!(reload_ticks(27, 600), 16, "16.2 is rounded down");
        assert_eq!(reload_ticks(1, 600), 1, "at least 1");
        assert_eq!(reload_ticks(u32::MAX, 10_000), u32::MAX);
        assert_eq!(scaled_speed(11, 1200), 13, "13.2 is rounded down");
        assert_eq!(scaled_speed(13, 1200), 15);
        assert_eq!(scaled_speed(1, 500), 1, "at least 1");
        assert_eq!(scaled_speed(128, 1200), 128, "at most the largest speed");
    }

    #[test]
    fn the_hash_covers_every_field_of_the_experience() {
        let hash = |experience: Experience| {
            let mut hasher = Fnv1a64::new();
            experience.hash_into(&mut hasher);
            hasher.finish()
        };
        let base = Experience::default();
        let mut changed = vec![
            Experience { value: 1, ..base },
            Experience { rank: 1, ..base },
            Experience {
                self_heal: true,
                ..base
            },
        ];
        for index in 0..4 {
            for ranked in [true, false] {
                let mut other = base;
                let set = if ranked {
                    &mut other.ranked
                } else {
                    &mut other.bonus
                };
                match index {
                    0 => set.firepower_pm = 1001,
                    1 => set.armour_pm = 1001,
                    2 => set.reload_pm = 1001,
                    _ => set.speed_pm = 1001,
                }
                changed.push(other);
            }
        }
        let mut seen = std::collections::BTreeSet::from([hash(base)]);
        for other in changed {
            assert!(seen.insert(hash(other)), "{other:?}");
        }
    }
}
