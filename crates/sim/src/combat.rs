//! Combat: weapons, warheads, armour, projectiles, target acquisition, the `Attack` and
//! `AttackMove` orders, and death and cleanup (PLAN.md sections 5 and 7, M4; docs/DECISIONS.md
//! D29).
//!
//! Every unit and building of a match with a combat file has [`Health`], [`Armour`], and
//! [`Sight`]; those whose type has a weapon also have a [`Weapon`]. In the `Combat` stage of each
//! tick, in this order:
//!
//! 1. Every projectile in flight comes one tick closer; one that arrives deals its damage to its
//!    target, or, if the target no longer exists, expires with
//!    [`SimEvent::ProjectileExpired`].
//! 2. Every armed unit and building, in ascending id order, counts down its reload, chooses its
//!    target, and fires if the target is in range (equation 1) and its weapon is loaded. An
//!    instant weapon (projectile speed 0) deals its damage in this tick; any other weapon fires a
//!    projectile that arrives after `ceil(distance / speed)` ticks (at least 1) and always hits
//!    its target if the target still exists then.
//! 3. The damage of the tick (equation 2) is subtracted from each target's hit points.
//!
//! All armed entities choose and fire from the state at the start of the stage, and damage is
//! applied only after all of them have fired, so the order in which they are processed does not
//! decide which of two units that shoot each other fires first. In the `Cleanup` stage every unit
//! and building whose hit points are ≤ 0 is removed and its cells are freed
//! ([`SimEvent::UnitDestroyed`], [`SimEvent::BuildingDestroyed`]).
//!
//! Since RA3 (docs/RA2_PARITY_PLAN.md sections 4.3 and 4.4, docs/DECISIONS.md R33 to R35):
//!
//! * **Weapons.** A unit or building carries a primary and perhaps a secondary weapon. Against
//!   a target it uses the first of the two that its type's `land_targeting` and
//!   `naval_targeting` allow for where the target is, whose projectile may engage the target's
//!   layer (`anti_air` for an aircraft, `anti_ground` for everything else), and whose warhead
//!   damages the target's armour ([`weapon_against`]). A target that none of them may engage is
//!   never acquired, and an `Attack` order on it ends with [`AttackEnd::NoDamage`].
//! * **Range.** A weapon fires at a target between its `min_range` and its `range`, the latter
//!   lengthened by the type's `air_range_bonus` for an anti air weapon against an aircraft. A
//!   unit ordered to attack a target inside the minimum range moves away to a cell from which
//!   it may fire.
//! * **Burst.** A weapon with a burst of n fires n shots [`BURST_GAP`] ticks apart and then
//!   reloads. A shot of a volley that cannot be fired when it is due ends the volley.
//! * **Impacts.** Every shot ends in an impact (`Blast`) of its warhead at a point: the
//!   target's position, or for an inaccurate weapon (`scatter`) a point drawn from
//!   [`SimRng`](crate::SimRng) around it. The impact damages its target and, if the warhead has
//!   a `cell_spread`, everything of the same layer (the air, or the ground and the sea) within
//!   that distance of the point, friend and foe alike, by [`splash_damage`]; a building takes
//!   one hit for every cell of its footprint whose centre lies within the spread. Bombs,
//!   superweapons and the other area effects of `crate::special` are impacts too
//!   (`detonate`), so no area damage ignores armour.
//! * **Damage states.** With a rules file every unit and building is green, yellow (at or
//!   below `condition_yellow_percent` of its hit points) or red ([`Condition`],
//!   [`SimEvent::DamageStateChanged`]).
//!
//! Targets (docs/DECISIONS.md D29):
//!
//! * A unit with an `Attack` order fires only at its target. Out of range, it moves to the
//!   nearest free cell from whose centre the target is in range (a unit on foot, RA3: the
//!   nearest cell with a spot left from each of whose three spots it is); it gives up after
//!   [`MAX_CHASES`] such moves without firing in between, or when the target no longer exists or
//!   is no longer visible to its owner ([`SimEvent::AttackEnded`]). In range, it stops. If its
//!   warhead deals no damage to the target's armour, the order ends with its first shot
//!   ([`AttackEnd::NoDamage`]).
//! * A unit with an `AttackMove` order moves to its destination; while a visible enemy it can
//!   damage is in range it stops and fires at the nearest one, then moves on. The order ends
//!   when the move ends (the move's own event reports how).
//! * A unit without any order, and a building with a weapon, fire at the nearest visible enemy in
//!   range that they can damage, without moving. A unit with a plain move order does not fire.
//!   A building with a weapon does not fire while its owner has low power (PLAN.md section 5,
//!   equation 3).
//!
//! No rule engages a target the weapon cannot damage (equation 2 gives 0) for longer than one
//! ordered shot, so no engagement, and no order waiting for one to end, lasts forever on a
//! target the unit cannot affect (docs/DECISIONS.md D32).
//!
//! Distances (equation 1) are measured in subcells between the nearest points of the two
//! entities: a unit is its fine position (for a unit on foot its spot in the cell, RA3,
//! docs/DECISIONS.md R6), a building the rectangle of subcells its footprint covers. "Visible" means visible to the attacker's owner as of the last `Visibility` stage
//! (`crate::visibility`); a player who is not a player of the match sees nothing.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Commands, Query, Res, ResMut};

use crate::building::{Building, Footprint};
use crate::combat_defs::{
    ArmourClass, BURST_GAP, CombatTerms, WarheadDef, WarheadKind, WeaponDef, WeaponKind,
};
use crate::command::PlayerId;
use crate::definitions::Definitions;
use crate::economy::Players;
use crate::event::{AttackEnd, SimEvent};
use crate::hash::Fnv1a64;
use crate::id::SimId;
use crate::map::{CellPos, Grid, SPOTS_PER_CELL, SUBCELLS_PER_CELL, SubPos};
use crate::pathfinding::nearest_cell;
use crate::schedule::SimTick;
use crate::unit::{Mobile, MoveOrder, Orders, Owner, Position};
use crate::visibility::Vision;

/// Moves toward its target that a unit with an `Attack` order may start without firing in
/// between before the order ends with [`AttackEnd::OutOfReach`]. Moves are counted, not ticks,
/// so the rule does not depend on unit speed (the class of M2 review 2 P0-1).
pub const MAX_CHASES: u8 = 8;

/// Hit points left. A unit or building whose hit points are ≤ 0 is removed in the `Cleanup`
/// stage of the tick.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Health {
    pub hit_points: i32,
}

/// How damaged a unit or building is (RA3; the ini's `ConditionYellow` and `ConditionRed`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DamageState {
    /// Above `condition_yellow_percent` of its hit points.
    #[default]
    Green,
    /// At or below `condition_yellow_percent`, above `condition_red_percent`.
    Yellow,
    /// At or below `condition_red_percent`.
    Red,
}

/// The damage state of a unit or building as of the last `Cleanup` stage. Always green in a
/// world without a rules file.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Condition(pub DamageState);

/// The damage state of `hit_points` of `full`: yellow at or below `yellow_percent` of `full`,
/// red at or below `red_percent`. Computed in `i64` (invariant I12).
pub fn damage_state(
    hit_points: i32,
    full: i32,
    yellow_percent: i32,
    red_percent: i32,
) -> DamageState {
    let scaled = i64::from(hit_points).saturating_mul(100);
    let at_or_below = |percent: i32| scaled <= i64::from(full).saturating_mul(i64::from(percent));
    if at_or_below(red_percent) {
        DamageState::Red
    } else if at_or_below(yellow_percent) {
        DamageState::Yellow
    } else {
        DamageState::Green
    }
}

/// The armour class of a unit or building (equation 2).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Armour(pub ArmourClass);

/// Sight radius in cells (equation 6).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sight(pub u32);

/// A combat order of a unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttackOrder {
    /// Attack `target` until it is destroyed or no longer visible. `chases` counts the moves
    /// toward it since the unit last fired.
    Attack { target: SimId, chases: u8 },
    /// Move to `destination`, fighting visible enemies in range on the way. `moving` is set while
    /// the move order toward the destination that this order gave is under way.
    AttackMove { destination: CellPos, moving: bool },
}

/// The weapons of a unit or building, and what it does with them.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Weapon {
    /// The primary weapon.
    pub kind: WeaponKind,
    /// The secondary weapon, if it has one.
    pub secondary: Option<WeaponKind>,
    /// Ticks until it can fire again (0: loaded). One timer serves both weapons.
    pub cooldown: u32,
    /// Shots of the volley under way that are still to be fired (0: none is under way).
    pub burst_left: u32,
    /// The weapon of the shot it fired last.
    pub volley: WeaponKind,
    /// The entity it aims at, if any.
    pub target: Option<SimId>,
    /// The unit's combat order; always `None` for a building.
    pub order: Option<AttackOrder>,
}

impl Weapon {
    pub(crate) fn new(kind: WeaponKind) -> Self {
        Self::with(kind, None)
    }

    pub(crate) fn with(kind: WeaponKind, secondary: Option<WeaponKind>) -> Self {
        Self {
            kind,
            secondary,
            cooldown: 0,
            burst_left: 0,
            volley: kind,
            target: None,
            order: None,
        }
    }
    /// Ends the combat order and forgets the target (`Move`, `Stop`, `Harvest`).
    pub(crate) fn clear(&mut self) {
        self.order = None;
        self.target = None;
    }

    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        hasher.write(&self.kind.0.to_le_bytes());
        match self.secondary {
            None => hasher.write(&[0]),
            Some(secondary) => {
                hasher.write(&[1]);
                hasher.write(&secondary.0.to_le_bytes());
            }
        }
        hasher.write_u32(self.cooldown);
        hasher.write_u32(self.burst_left);
        hasher.write(&self.volley.0.to_le_bytes());
        match self.target {
            None => hasher.write(&[0]),
            Some(target) => {
                hasher.write(&[1]);
                hasher.write_u32(target.0);
            }
        }
        match self.order {
            None => hasher.write(&[0]),
            Some(AttackOrder::Attack { target, chases }) => {
                hasher.write(&[1]);
                hasher.write_u32(target.0);
                hasher.write(&[chases]);
            }
            Some(AttackOrder::AttackMove {
                destination,
                moving,
            }) => {
                hasher.write(&[2]);
                hasher.write(&destination.x.to_le_bytes());
                hasher.write(&destination.y.to_le_bytes());
                hasher.write(&[u8::from(moving)]);
            }
        }
    }
}

/// A projectile in flight. It hits `target` when `remaining` reaches 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Projectile {
    pub owner: PlayerId,
    pub attacker: SimId,
    pub target: SimId,
    pub weapon: WeaponKind,
    /// Where it was fired from (for drawing).
    pub origin: SubPos,
    /// Where it was aimed when it was fired: the target's position then (for a building, the
    /// centre of the cell of its footprint nearest to the attacker), or for an inaccurate
    /// weapon the point drawn around it.
    pub aim: SubPos,
    /// Whether it lands on `aim` whatever its target does (an inaccurate weapon). Otherwise it
    /// hits its target where that then is.
    pub scattered: bool,
    /// Whether it was fired at an aircraft, so that its impact is in the air.
    pub air: bool,
    /// Ticks of flight in all, at least 1.
    pub flight: u32,
    /// Ticks of flight left.
    pub remaining: u32,
    /// The firepower multiplier of its attacker when it was fired, in permille (RA4).
    pub firepower_pm: u32,
}

/// The projectiles in flight, in the order they were fired (invariant I4).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct Projectiles {
    list: Vec<Projectile>,
}

impl Projectiles {
    pub fn iter(&self) -> impl Iterator<Item = &Projectile> {
        self.list.iter()
    }

    /// The projectiles of `from` in flight become those of `to` (RA4: what a launched aircraft
    /// fired counts for its spawner once the aircraft is gone).
    pub(crate) fn reassign(&mut self, from: SimId, to: SimId) {
        for projectile in &mut self.list {
            if projectile.attacker == from {
                projectile.attacker = to;
            }
        }
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        hasher.write_u64(self.list.len() as u64);
        for projectile in &self.list {
            hasher.write(&[projectile.owner.0]);
            hasher.write_u32(projectile.attacker.0);
            hasher.write_u32(projectile.target.0);
            hasher.write(&projectile.weapon.0.to_le_bytes());
            hasher.write(&projectile.origin.x.to_le_bytes());
            hasher.write(&projectile.origin.y.to_le_bytes());
            hasher.write(&projectile.aim.x.to_le_bytes());
            hasher.write(&projectile.aim.y.to_le_bytes());
            hasher.write(&[u8::from(projectile.scattered), u8::from(projectile.air)]);
            hasher.write_u32(projectile.flight);
            hasher.write_u32(projectile.remaining);
            // RA4: written only when it changes something, so that a world without veterancy
            // hashes as before.
            if projectile.firepower_pm != 1000 {
                hasher.write(b"fp");
                hasher.write_u32(projectile.firepower_pm);
            }
        }
    }
}

/// Equation 2 of PLAN.md section 5: the damage a shot of base damage `base` deals through a
/// warhead with percentage `versus` against the target's armour class:
/// `floor(base × versus / 100)`, at least 1 if `versus > 0`, and 0 if `versus = 0` (or below).
/// Computed in `i64` and clamped to `i32`, so no input overflows (invariant I12).
pub fn applied_damage(base: i32, versus: i32) -> i32 {
    if versus <= 0 {
        return 0;
    }
    let applied = (i64::from(base) * i64::from(versus)).div_euclid(100).max(1);
    i32::try_from(applied).unwrap_or(i32::MAX)
}

/// The damage of an impact on something `distance` subcells from its point
/// (docs/RA2_PARITY_PLAN.md section 4.3), for a shot of base damage `base` through a warhead
/// with percentage `versus` against the target's armour, spread `cell_spread` (subcells) and
/// `percent_at_max` percent at that distance:
///
/// ```text
/// floor(base × (100 R − (100 − percent_at_max) d) × versus / (100 R × 100))
/// ```
///
/// at least 1 if `versus > 0` (equation 2), 0 if `versus = 0`, and 0 beyond the spread. Without
/// a spread (`cell_spread` 0) it is equation 2 whatever the distance: the caller decides what
/// such an impact hits. Computed in `i128` and clamped to `i32`, so no input overflows
/// (invariant I12).
pub fn splash_damage(
    base: i32,
    versus: i32,
    cell_spread: u32,
    percent_at_max: u32,
    distance: u64,
) -> i32 {
    if versus <= 0 {
        return 0;
    }
    if cell_spread == 0 {
        return applied_damage(base, versus);
    }
    let spread = i128::from(cell_spread);
    let distance = i128::from(distance);
    if distance > spread {
        return 0;
    }
    let lost = i128::from(100u32.saturating_sub(percent_at_max));
    let share = 100 * spread - lost * distance;
    let applied = (i128::from(base) * share * i128::from(versus))
        .div_euclid(100 * spread * 100)
        .max(1);
    i32::try_from(applied).unwrap_or(i32::MAX)
}

/// The damage of a hit (docs/RA2_PARITY_PLAN.md section 4.3, with the multipliers of RA4): of
/// base damage `base` from an attacker whose firepower multiplier is `firepower_pm`, through a
/// warhead with percentage `versus` against the target's armour, spread `cell_spread`
/// (subcells) and `percent_at_max` percent at that distance, on something `distance` subcells
/// from the impact whose armour multiplier is `armour_pm`:
///
/// ```text
/// D2 = floor(base × firepower_pm × (100 R − (100 − percent_at_max) d) × versus
///            / (1000 × 100 R × 100)),   at least 1 if versus > 0
/// D3 = floor(D2 × 1000 / armour_pm)
/// ```
///
/// 0 if `versus = 0` and beyond the spread; without a spread (`cell_spread` 0) the share of the
/// distance is 1, as in [`splash_damage`], which this is for multipliers of 1000. The armour
/// multiplier is applied after the minimum of equation 2, as the plan's equations have it, so
/// a hit of 1 on a unit with stronger armour deals nothing. Computed in `i128` and clamped to
/// `i32` (invariant I12).
pub fn impact_damage(
    base: i32,
    firepower_pm: u32,
    versus: i32,
    cell_spread: u32,
    percent_at_max: u32,
    distance: u64,
    armour_pm: u32,
) -> i32 {
    if versus <= 0 {
        return 0;
    }
    let (share, whole) = if cell_spread == 0 {
        (1, 1)
    } else {
        let spread = i128::from(cell_spread);
        let distance = i128::from(distance);
        if distance > spread {
            return 0;
        }
        let lost = i128::from(100u32.saturating_sub(percent_at_max));
        (100 * spread - lost * distance, 100 * spread)
    };
    let dealt = (i128::from(base) * i128::from(firepower_pm) * share * i128::from(versus))
        .div_euclid(1000 * whole * 100)
        .max(1);
    let taken = dealt * 1000 / i128::from(armour_pm.max(1));
    i32::try_from(taken).unwrap_or(i32::MAX)
}

/// Equation 1 of PLAN.md section 5: whether a target at offset (`dx`, `dy`) subcells is within
/// `range` subcells, `dx² + dy² ≤ range²`. Computed in `i128`, exact for every input.
pub fn in_range(dx: i64, dy: i64, range: u32) -> bool {
    let (dx, dy, range) = (i128::from(dx), i128::from(dy), i128::from(range));
    dx * dx + dy * dy <= range * range
}

/// Whether a target at offset (`dx`, `dy`) subcells is nearer than `min_range` subcells, where
/// a weapon with that minimum range does not fire: `dx² + dy² < min_range²`.
pub fn inside_min_range(dx: i64, dy: i64, min_range: u32) -> bool {
    let (dx, dy, min) = (i128::from(dx), i128::from(dy), i128::from(min_range));
    dx * dx + dy * dy < min * min
}

/// The distance in subcells of the offset (`dx`, `dy`), rounded down.
fn distance(dx: i64, dy: i64) -> u64 {
    let (dx, dy) = (dx.unsigned_abs(), dy.unsigned_abs());
    dx.saturating_mul(dx)
        .saturating_add(dy.saturating_mul(dy))
        .isqrt()
}

/// What a weapon needs to know of a target to decide whether it may engage it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TargetTraits {
    pub armour: ArmourClass,
    /// An aircraft (a unit of the air layer).
    pub airborne: bool,
    /// On water: a ship, an amphibious unit over water, or a building standing in water.
    pub on_water: bool,
    /// A unit that moves under water.
    pub underwater: bool,
    /// A living thing.
    pub organic: bool,
    /// A building that is a wall: only warheads with `wall` damage it.
    pub wall: bool,
    /// Trees: only warheads with `wood` damage them.
    pub tree: bool,
}

/// The percentage `warhead` deals to a target with `traits`: its percentage against the armour,
/// or 0 for a wall or trees that it does not damage at all.
pub fn versus_against(warhead: &WarheadDef, traits: &TargetTraits) -> i32 {
    if (traits.wall && !warhead.wall) || (traits.tree && !warhead.wood) {
        return 0;
    }
    warhead
        .versus
        .get(usize::from(traits.armour.0))
        .copied()
        .unwrap_or(0)
}

/// The weapon that a type with the combat terms `terms`, carrying `weapons` (primary,
/// secondary), uses against a target with `traits`, and the percentage its warhead deals
/// (docs/RA2_PARITY_PLAN.md section 4.4):
///
/// 1. the weapons its `land_targeting` (for a target on land) or `naval_targeting` (for a
///    target on water) allows; neither applies to an aircraft;
/// 2. of these, those whose projectile may engage the target's layer (`anti_air` for an
///    aircraft, `anti_ground` otherwise);
/// 3. of these, the first whose warhead damages the target (a percentage above 0).
///
/// If step 3 leaves none, the first weapon of step 2 is returned with the percentage 0: a shot
/// that does nothing, which only an order makes a unit fire, once. `None`: no weapon may be
/// fired at the target at all.
pub fn weapon_against(
    definitions: &Definitions,
    terms: &CombatTerms,
    weapons: [Option<WeaponKind>; 2],
    traits: &TargetTraits,
) -> Option<(WeaponKind, i32)> {
    let both = [true, true];
    let secondary = [false, true];
    let none = [false, false];
    let allowed = if traits.airborne {
        both
    } else if traits.on_water {
        match terms.naval_targeting {
            0 if traits.underwater => none,
            1 if traits.underwater => secondary,
            2 if !traits.underwater => none,
            3 if traits.organic => secondary,
            4 => secondary,
            6 => none,
            _ => both,
        }
    } else {
        match terms.land_targeting {
            0 => both,
            1 => none,
            _ => secondary,
        }
    };
    let mut harmless = None;
    for (kind, allowed) in weapons.into_iter().zip(allowed) {
        let Some(kind) = kind.filter(|_| allowed) else {
            continue;
        };
        let Some(def) = definitions.weapon(kind) else {
            continue;
        };
        let layer = if traits.airborne {
            def.anti_air
        } else {
            def.anti_ground
        };
        if !layer {
            continue;
        }
        let versus = definitions
            .warhead(def.warhead)
            .map_or(0, |warhead| versus_against(warhead, traits));
        if versus > 0 {
            return Some((kind, versus));
        }
        harmless.get_or_insert((kind, 0));
    }
    harmless
}

/// The subcells an entity covers, inclusive, for distances (equation 1): a unit's fine position,
/// or the rectangle of a building's footprint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Extent {
    x0: i64,
    y0: i64,
    x1: i64,
    y1: i64,
}

impl Extent {
    pub(crate) fn point(position: SubPos) -> Self {
        let (x, y) = (i64::from(position.x), i64::from(position.y));
        Self {
            x0: x,
            y0: y,
            x1: x,
            y1: y,
        }
    }

    pub(crate) fn footprint(footprint: Footprint) -> Self {
        let cells = i64::from(SUBCELLS_PER_CELL);
        let (x, y) = (i64::from(footprint.origin.x), i64::from(footprint.origin.y));
        let (w, h) = (
            i64::from(footprint.width.max(1)),
            i64::from(footprint.height.max(1)),
        );
        Self {
            x0: x * cells,
            y0: y * cells,
            x1: (x + w) * cells - 1,
            y1: (y + h) * cells - 1,
        }
    }

    /// The offset between the nearest points of the two extents (0 along an axis on which they
    /// overlap).
    pub(crate) fn gap(&self, other: &Extent) -> (i64, i64) {
        let dx = (other.x0 - self.x1).max(self.x0 - other.x1).max(0);
        let dy = (other.y0 - self.y1).max(self.y0 - other.y1).max(0);
        (dx, dy)
    }

    /// The centre, for drawing (where a projectile starts).
    fn centre(&self) -> SubPos {
        let mid = |a: i64, b: i64| i32::try_from((a + b + 1).div_euclid(2)).unwrap_or(i32::MAX);
        SubPos::new(mid(self.x0, self.x1), mid(self.y0, self.y1))
    }

    /// Where a shot fired from `from` at this extent is aimed: a unit's position, or the centre
    /// of the cell of a footprint nearest to the centre of `from`.
    fn aim_from(&self, from: &Extent) -> SubPos {
        if self.x0 == self.x1 && self.y0 == self.y1 {
            return self.centre();
        }
        let cells = i64::from(SUBCELLS_PER_CELL);
        let centre = from.centre();
        let nearest = |at: i32, low: i64, high: i64| {
            let cell = i64::from(at)
                .div_euclid(cells)
                .clamp(low.div_euclid(cells), high.div_euclid(cells));
            i32::try_from(cell).unwrap_or(i32::MAX)
        };
        SubPos::centre_of(CellPos::new(
            nearest(centre.x, self.x0, self.x1),
            nearest(centre.y, self.y0, self.y1),
        ))
    }
}

/// What a target looks like to the units that see it.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Body {
    /// A unit: the cell it stands in and the cell it is stepping into, if any.
    Unit {
        cell: CellPos,
        next: Option<CellPos>,
    },
    Building(Footprint),
    /// Scenery (`crate::props`): the cell it stands in.
    Prop(CellPos),
}

impl Body {
    pub(crate) fn of(mobile: Option<&Mobile>, building: Option<&Building>) -> Option<Self> {
        match (mobile, building) {
            (Some(mobile), _) => Some(Body::Unit {
                cell: mobile.cell,
                next: mobile.step.map(|step| step.to),
            }),
            (None, Some(building)) => Some(Body::Building(building.footprint)),
            (None, None) => None,
        }
    }

    /// Whether `vision` shows any cell of the body (docs/DECISIONS.md D30).
    pub(crate) fn seen_by(&self, vision: &Vision) -> bool {
        match *self {
            Body::Unit { cell, next } => {
                vision.is_visible(cell) || next.is_some_and(|next| vision.is_visible(next))
            }
            Body::Building(footprint) => vision.sees_footprint(footprint),
            // Scenery does not move or hide: once explored, the player knows it is there.
            Body::Prop(cell) => vision.is_explored(cell),
        }
    }
}

/// The components of a unit or building with the combat terms `terms`.
pub(crate) fn combat_components(
    terms: CombatTerms,
) -> ((Health, Armour, Sight, Condition), Option<Weapon>) {
    (
        (
            Health {
                hit_points: terms.hit_points,
            },
            Armour(terms.armour),
            Sight(terms.sight),
            Condition::default(),
        ),
        terms.weapon.map(|kind| Weapon::with(kind, terms.secondary)),
    )
}

/// Appends a unit's or building's health, armour, and sight to the hash.
pub(crate) fn hash_combat(
    hasher: &mut Fnv1a64,
    (health, armour, sight): (&Health, &Armour, &Sight),
) {
    hasher.write(&health.hit_points.to_le_bytes());
    hasher.write(&armour.0.0.to_le_bytes());
    hasher.write_u32(sight.0);
}

/// A possible target, as seen at the start of the `Combat` stage.
#[derive(Clone, Copy)]
struct Target {
    /// `None` for scenery, which is not an entity.
    entity: Option<Entity>,
    /// `None` for scenery, which belongs to nobody.
    owner: Option<PlayerId>,
    extent: Extent,
    body: Body,
    traits: TargetTraits,
    /// The divisor of the damage it takes, in permille (RA4: 1000 without experience).
    armour_pm: u32,
    /// What it is, for the experience of whoever destroys it.
    item: Option<crate::ProductionItem>,
}

/// The traits of a unit or building as a target. `terms` are the combat terms of its type.
fn traits_of(
    grid: &Grid,
    id: SimId,
    armour: ArmourClass,
    body: &Body,
    terms: Option<&CombatTerms>,
) -> TargetTraits {
    let water = |cell: CellPos| grid.cell(cell).is_some_and(|cell| cell.water);
    let (airborne, on_water) = match *body {
        Body::Unit { cell, .. } => match grid.domain_of(id) {
            crate::Domain::Air => (true, false),
            crate::Domain::Naval => (false, true),
            crate::Domain::Amphibious => (false, water(cell)),
            crate::Domain::Land => (false, false),
        },
        Body::Building(footprint) => (false, water(footprint.origin)),
        Body::Prop(_) => (false, false),
    };
    TargetTraits {
        armour,
        airborne,
        on_water,
        underwater: terms.is_some_and(|terms| terms.underwater),
        organic: terms.is_some_and(|terms| terms.organic),
        wall: terms.is_some_and(|terms| terms.wall),
        tree: false,
    }
}

type TargetQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static SimId,
        &'static Owner,
        &'static mut Health,
        &'static Armour,
        Option<&'static Position>,
        Option<&'static Mobile>,
        Option<&'static Building>,
        Option<&'static crate::Unit>,
        Option<&'static crate::Experience>,
    ),
>;

type AttackerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static SimId,
        &'static Owner,
        &'static mut Weapon,
        Option<&'static Position>,
        Option<&'static Mobile>,
        Option<&'static mut Orders>,
        Option<&'static Building>,
        Option<&'static crate::Experience>,
    ),
>;

/// An impact of a warhead at a point (docs/RA2_PARITY_PLAN.md section 4.3).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Blast {
    pub(crate) point: SubPos,
    /// In the air: it damages aircraft only. Otherwise it damages what is on the ground and at
    /// sea.
    pub(crate) air: bool,
    pub(crate) warhead: WarheadKind,
    /// The base damage.
    pub(crate) damage: i32,
    /// The unit, building or scenery it hit: damaged as at the point itself, wherever its
    /// position is.
    pub(crate) direct: Option<SimId>,
    /// The firepower multiplier of whoever caused it, in permille (RA4).
    pub(crate) firepower_pm: u32,
    /// The unit or building that caused it: the killer of what it destroys.
    pub(crate) source: Option<SimId>,
}

/// One hit of an impact: what is hit, the damage, and who caused it.
type Hit = (SimId, i32, Option<SimId>);

/// What an impact needs to know of an entity it may hit: its position (of a unit), its traits,
/// and the divisor of the damage it takes in permille.
type Struck = (Option<SubPos>, TargetTraits, u32);

/// The damage of every hit of `blast`, by the id of what is hit, in the order the hits are
/// found; an entity may be hit more than once (a building, once per cell of its footprint
/// within the spread). `traits` gives the position (of a unit), the traits and the armour
/// multiplier of an entity.
///
/// Only the cells the spread reaches are looked at, through the grid's occupants (every unit
/// that holds a cell: a vehicle, or each of the infantry in its spots, by its own distance), so
/// the cost of an impact does not grow with the number of units of the match.
fn blast_hits(
    definitions: &Definitions,
    grid: &Grid,
    props: &crate::Props,
    traits: &dyn Fn(SimId) -> Option<Struck>,
    blast: &Blast,
    hits: &mut Vec<Hit>,
) {
    let Some(warhead) = definitions.warhead(blast.warhead) else {
        return;
    };
    let most = definitions
        .rules()
        .map_or(i32::MAX, |rules| rules.combat.max_damage);
    let spread = warhead.cell_spread;
    let mut hit = |id: SimId, traits: &TargetTraits, distance: u64, armour_pm: u32| {
        let damage = impact_damage(
            blast.damage,
            blast.firepower_pm,
            versus_against(warhead, traits),
            spread,
            warhead.percent_at_max,
            distance,
            armour_pm,
        )
        .min(most);
        if damage > 0 {
            hits.push((id, damage, blast.source));
        }
    };
    let prop_traits = |kind: crate::PropKind| TargetTraits {
        armour: props.armour(),
        tree: kind == crate::PropKind::Trees,
        ..TargetTraits::default()
    };
    if let Some(direct) = blast.direct {
        if let Some((_, found, armour_pm)) = traits(direct) {
            hit(direct, &found, 0, armour_pm);
        } else if let Some((_, prop)) = props.get(direct) {
            hit(direct, &prop_traits(prop.kind), 0, 1000);
        }
        if spread == 0 {
            return;
        }
    }
    // The cells whose centre, or a unit in which, may lie within the spread.
    let cells = i64::from(SUBCELLS_PER_CELL);
    let (px, py) = (i64::from(blast.point.x), i64::from(blast.point.y));
    let reach = i64::from(spread);
    let cell_of = |at: i64| i32::try_from(at.div_euclid(cells)).unwrap_or(i32::MAX);
    let (x0, x1) = (cell_of(px - reach), cell_of(px + reach));
    let (y0, y1) = (cell_of(py - reach), cell_of(py + reach));
    let home = blast.point.cell();
    // Whether something at `at` is hit, and how far it is: within the spread, or for a
    // warhead without one in the cell of the impact.
    let within = |at: SubPos| {
        let (dx, dy) = (i64::from(at.x) - px, i64::from(at.y) - py);
        if spread == 0 {
            (at.cell() == home).then_some(0)
        } else {
            in_range(dx, dy, spread).then(|| distance(dx, dy))
        }
    };
    let layer = if blast.air {
        crate::Domain::Air
    } else {
        crate::Domain::Land
    };
    let mut seen: Vec<SimId> = Vec::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            let cell = CellPos::new(x, y);
            let centre = SubPos::centre_of(cell);
            if !blast.air {
                // A building: one hit for every cell of its footprint that is reached. The
                // cell a direct hit struck has been counted.
                if let Some(building) = grid.building(cell)
                    && !(blast.direct == Some(building) && cell == home)
                    && let Some(distance) = within(centre)
                    && let Some((_, found, armour_pm)) = traits(building)
                {
                    hit(building, &found, distance, armour_pm);
                }
                if let Some(prop) = props.at(cell)
                    && let Some(id) = props.id_at(cell)
                    && blast.direct != Some(id)
                    && let Some(distance) = within(centre)
                {
                    hit(id, &prop_traits(prop.kind), distance, 1000);
                }
            }
            // A unit holds the cell it stands in and the one it is entering: once is enough.
            for unit in grid.holders_for(cell, layer).into_iter().flatten() {
                if blast.direct == Some(unit) || seen.contains(&unit) {
                    continue;
                }
                seen.push(unit);
                if let Some((Some(position), found, armour_pm)) = traits(unit)
                    && found.airborne == blast.air
                    && let Some(distance) = within(position)
                {
                    hit(unit, &found, distance, armour_pm);
                }
            }
        }
    }
}

/// Sixty-four directions as (cos, sin) × 1024, rounded toward zero so that none is longer than
/// 1024 (scatter, docs/RA2_PARITY_PLAN.md section 4.4).
const DIRECTIONS: [(i32, i32); 64] = [
    (1024, 0),
    (1019, 100),
    (1004, 199),
    (979, 297),
    (946, 391),
    (903, 482),
    (851, 568),
    (791, 649),
    (724, 724),
    (649, 791),
    (568, 851),
    (482, 903),
    (391, 946),
    (297, 979),
    (199, 1004),
    (100, 1019),
    (0, 1024),
    (-100, 1019),
    (-199, 1004),
    (-297, 979),
    (-391, 946),
    (-482, 903),
    (-568, 851),
    (-649, 791),
    (-724, 724),
    (-791, 649),
    (-851, 568),
    (-903, 482),
    (-946, 391),
    (-979, 297),
    (-1004, 199),
    (-1019, 100),
    (-1024, 0),
    (-1019, -100),
    (-1004, -199),
    (-979, -297),
    (-946, -391),
    (-903, -482),
    (-851, -568),
    (-791, -649),
    (-724, -724),
    (-649, -791),
    (-568, -851),
    (-482, -903),
    (-391, -946),
    (-297, -979),
    (-199, -1004),
    (-100, -1019),
    (0, -1024),
    (100, -1019),
    (199, -1004),
    (297, -979),
    (391, -946),
    (482, -903),
    (568, -851),
    (649, -791),
    (724, -724),
    (791, -649),
    (851, -568),
    (903, -482),
    (946, -391),
    (979, -297),
    (1004, -199),
    (1019, -100),
];

/// The point a shot of a weapon with `scatter` lands on when it is aimed at `aim` from
/// `distance` subcells away: in one of [`DIRECTIONS`], at a distance drawn uniformly from 0 to
/// `scatter × distance / range` (at most `scatter`). Two draws from `rng`.
pub fn scattered(
    rng: &mut crate::SimRng,
    aim: SubPos,
    scatter: u32,
    distance: u64,
    range: u32,
) -> SubPos {
    let radius =
        u64::from(scatter).saturating_mul(distance.min(u64::from(range))) / u64::from(range.max(1));
    let (cos, sin) = DIRECTIONS[(rng.next_u32() % 64) as usize];
    let length = i64::try_from(u64::from(rng.next_u32()) % (radius + 1)).unwrap_or(0);
    let shift = |at: i32, part: i32| {
        let moved = i64::from(at) + length * i64::from(part) / 1024;
        i32::try_from(moved).unwrap_or(at)
    };
    SubPos::new(shift(aim.x, cos), shift(aim.y, sin))
}

/// What an attacker fires with: the weapons it carries and the combat terms of its type.
#[derive(Clone, Copy)]
struct Arms {
    weapons: [Option<WeaponKind>; 2],
    terms: CombatTerms,
    /// The multipliers of the attacker's damage and reload, in permille (RA4).
    firepower_pm: u32,
    reload_pm: u32,
    /// The weapon with which the attacker launches the units it spawns, if it has one that is
    /// ready to launch (RA4, `crate::flight`): `Some(false)` while none is ready.
    launcher: Option<(WeaponKind, bool)>,
}

impl Arms {
    /// The ticks until the attacker may fire again after a volley of the weapon `def`.
    fn reload(&self, def: &WeaponDef) -> u32 {
        if self.reload_pm == 1000 {
            def.reload
        } else {
            crate::veterancy::reload_ticks(def.reload, self.reload_pm)
        }
    }
}

/// The weapon chosen against a target.
#[derive(Clone, Copy)]
struct Choice<'a> {
    kind: WeaponKind,
    def: &'a WeaponDef,
    /// The percentage its warhead deals to the target; 0: the shot does nothing.
    versus: i32,
    /// Its range against the target, with the air range bonus.
    reach: u32,
}

impl Choice<'_> {
    /// Whether a target at the offset (`dx`, `dy`) is where the weapon may fire at it.
    fn covers(&self, dx: i64, dy: i64) -> bool {
        in_range(dx, dy, self.reach) && !inside_min_range(dx, dy, self.def.min_range)
    }
}

/// Everything an attacker needs to know in the tick.
struct Field<'a> {
    targets: &'a BTreeMap<SimId, Target>,
    props: &'a crate::Props,
    players: &'a Players,
    definitions: &'a Definitions,
    grid: &'a Grid,
    tick: u64,
    special: &'a crate::SpecialState,
    roles: &'a BTreeMap<SimId, crate::Role>,
    /// The percentage a warhead must exceed for its weapon to engage a target on its own: 0,
    /// and with a rules file 1 (RA2: a target at 1 percent is attacked only on order).
    floor: i32,
    /// The units of a type with the arsenal's `crusher` trait: their attack moves may crush.
    crushers: &'a BTreeSet<SimId>,
}

/// What the tick's shots add up to.
#[derive(Default)]
struct Shots {
    /// The impacts of the tick, in the order they happen.
    blasts: Vec<Blast>,
    fired: Vec<Projectile>,
    events: Vec<SimEvent>,
    /// Cells chosen this tick as destinations of units moving toward their targets, each with
    /// the room that was promised in it (in spots, [`Grid::room_needed`]), so that units
    /// attacking the same target spread over different cells, the infantry on foot three to a
    /// cell.
    chase_cells: BTreeMap<CellPos, u8>,
}

impl Field<'_> {
    fn vision(&self, player: PlayerId) -> Option<&Vision> {
        self.players.get(player).map(|state| state.vision())
    }

    /// The unit, building, or scenery with the id `target`.
    fn target(&self, target: SimId) -> Option<Target> {
        if let Some(found) = self.targets.get(&target) {
            return Some(*found);
        }
        let (cell, prop) = self.props.get(target)?;
        Some(Target {
            entity: None,
            owner: None,
            extent: Extent::footprint(Footprint {
                origin: cell,
                width: 1,
                height: 1,
            }),
            body: Body::Prop(cell),
            traits: TargetTraits {
                armour: self.props.armour(),
                tree: prop.kind == crate::PropKind::Trees,
                ..TargetTraits::default()
            },
            armour_pm: 1000,
            item: None,
        })
    }

    /// `target` if `owner` sees it and may fire at it: an enemy (not its own or an ally's, see
    /// [`Players::hostile`]), or scenery, which only an `Attack` order ever names.
    fn visible_enemy(&self, owner: PlayerId, target: SimId) -> Option<Target> {
        let found = self.target(target)?;
        let vision = self.vision(owner)?;
        (found
            .owner
            .is_none_or(|other| self.players.hostile(owner, other))
            && found.body.seen_by(vision)
            && !self.special.hidden.contains(&(owner, target)))
        .then_some(found)
    }

    /// The weapon `arms` uses against a target with `traits` ([`weapon_against`]).
    fn choose(&self, arms: &Arms, traits: &TargetTraits) -> Option<Choice<'_>> {
        let (kind, versus) = weapon_against(self.definitions, &arms.terms, arms.weapons, traits)?;
        let def = self.definitions.weapon(kind)?;
        let bonus = if traits.airborne && def.anti_air {
            arms.terms.air_range_bonus
        } else {
            0
        };
        Some(Choice {
            kind,
            def,
            versus,
            reach: def.range.saturating_add(bonus),
        })
    }

    /// The weapon `arms` engages a target with `traits` with on its own: one that damages it by
    /// more than the floor.
    fn engages(&self, arms: &Arms, traits: &TargetTraits) -> Option<Choice<'_>> {
        self.choose(arms, traits)
            .filter(|choice| choice.versus > self.floor)
    }

    /// The farthest any weapon of `arms` reaches against any target.
    fn longest_reach(&self, arms: &Arms) -> u32 {
        arms.weapons
            .into_iter()
            .flatten()
            .filter_map(|kind| self.definitions.weapon(kind))
            .map(|def| def.range.saturating_add(arms.terms.air_range_bonus))
            .max()
            .unwrap_or(0)
    }

    /// The visible enemy of `owner` that `arms` engages on its own nearest to `from` within the
    /// range of the weapon used against it (ties by id).
    fn nearest_enemy_in_range(&self, owner: PlayerId, from: &Extent, arms: &Arms) -> Option<SimId> {
        let vision = self.vision(owner)?;
        let longest = self.longest_reach(arms);
        self.targets
            .iter()
            .filter_map(|(&id, target)| {
                let (dx, dy) = from.gap(&target.extent);
                // The cheapest test first: most targets are out of every weapon's range.
                if !in_range(dx, dy, longest)
                    || self.special.hidden.contains(&(owner, id))
                    || !target
                        .owner
                        .is_some_and(|other| self.players.hostile(owner, other))
                    || !target.body.seen_by(vision)
                {
                    return None;
                }
                self.engages(arms, &target.traits)
                    .filter(|choice| choice.covers(dx, dy))
                    .map(|_| {
                        (
                            i128::from(dx) * i128::from(dx) + i128::from(dy) * i128::from(dy),
                            id,
                        )
                    })
            })
            .min()
            .map(|(_, id)| id)
    }

    /// Fires the weapon of `choice` at `target`.
    fn fire(
        &self,
        shots: &mut Shots,
        rng: &mut crate::SimRng,
        attacker: (SimId, PlayerId, &Extent),
        arms: &Arms,
        choice: &Choice<'_>,
        (target_id, target): (SimId, &Target),
    ) {
        let target = *target;
        let (id, owner, from) = attacker;
        let def = choice.def;
        shots.events.push(SimEvent::WeaponFired {
            attacker: id,
            target: target_id,
            weapon: choice.kind,
        });
        // The shot of some specialists is not an impact of its own: `crate::special` acts on
        // the event (a carrier launches aircraft; with a rules file a mind controller takes
        // its target over, a bomber plants its bomb, and a suicide unit blows itself up).
        match self.roles.get(&id) {
            Some(crate::Role::Carrier) => return,
            Some(crate::Role::MindControl | crate::Role::Demolition | crate::Role::Suicide)
                if self.definitions.rules().is_some() =>
            {
                return;
            }
            _ => {}
        }
        // The shot of a launcher is no impact either: `crate::flight` launches a missile or
        // an aircraft at the target (RA4).
        if arms
            .launcher
            .is_some_and(|(launcher, _)| launcher == choice.kind)
        {
            return;
        }
        let (dx, dy) = from.gap(&target.extent);
        let distance = distance(dx, dy);
        let aim = target.extent.aim_from(from);
        let (aim, scattered) = if def.scatter > 0 {
            (scattered(rng, aim, def.scatter, distance, def.range), true)
        } else {
            (aim, false)
        };
        let air = target.traits.airborne;
        if def.projectile_speed == 0 {
            shots.blasts.push(Blast {
                point: aim,
                air,
                warhead: def.warhead,
                damage: def.damage,
                direct: (!scattered).then_some(target_id),
                firepower_pm: arms.firepower_pm,
                source: Some(id),
            });
            return;
        }
        let flight = u32::try_from(distance.div_ceil(u64::from(def.projectile_speed)))
            .unwrap_or(u32::MAX)
            .max(1);
        shots.fired.push(Projectile {
            owner,
            attacker: id,
            target: target_id,
            weapon: choice.kind,
            origin: from.centre(),
            aim,
            scattered,
            air,
            flight,
            remaining: flight,
            firepower_pm: arms.firepower_pm,
        });
    }
}

/// Moves projectiles, lets armed units and buildings choose targets and fire, and applies the
/// tick's damage (see the module documentation).
#[allow(clippy::too_many_arguments)]
pub(crate) fn combat(
    mut events: ResMut<Messages<SimEvent>>,
    mut projectiles: ResMut<Projectiles>,
    players: Res<Players>,
    definitions: Res<Definitions>,
    grid: Res<Grid>,
    tick: Res<SimTick>,
    mut rng: ResMut<crate::SimRng>,
    mut targets: TargetQuery,
    mut attackers: AttackerQuery,
    mut props: ResMut<crate::Props>,
    special: Res<crate::SpecialState>,
    mut kills: ResMut<crate::veterancy::Kills>,
) {
    if attackers.is_empty() && projectiles.is_empty() {
        return;
    }
    // RA4: the units that launch what they spawn, with the weapon slot that does it and the
    // number they carry.
    let mut launchers: BTreeMap<SimId, (crate::Launcher, u32)> = BTreeMap::new();
    let ruled = definitions.rules().is_some();
    let mut roles: BTreeMap<SimId, crate::Role> = BTreeMap::new();
    let mut terms: BTreeMap<SimId, CombatTerms> = BTreeMap::new();
    let mut crushers: BTreeSet<SimId> = BTreeSet::new();
    let index: BTreeMap<SimId, Target> = targets
        .iter()
        .filter_map(
            |(entity, id, owner, _, armour, position, mobile, building, unit, experience)| {
                let extent = match (position, building) {
                    (Some(position), _) => Extent::point(position.0),
                    (None, Some(building)) => Extent::footprint(building.footprint),
                    (None, None) => return None,
                };
                let body = Body::of(mobile, building)?;
                let item = unit
                    .map(|unit| crate::ProductionItem::Unit(unit.kind))
                    .or_else(|| {
                        building.map(|building| crate::ProductionItem::Building(building.kind))
                    });
                let of_type = match item {
                    Some(crate::ProductionItem::Unit(kind)) => definitions.unit_combat(kind),
                    Some(crate::ProductionItem::Building(kind)) => {
                        definitions.building_combat(kind)
                    }
                    None => None,
                };
                if let Some(role) = item.and_then(|item| definitions.metadata(item)) {
                    roles.insert(*id, role.role);
                    if role.crusher {
                        crushers.insert(*id);
                    }
                    if ruled
                        && role.spawns.is_some()
                        && let Some(launcher) = role.launcher
                    {
                        launchers.insert(*id, (launcher, role.spawns_number));
                    }
                }
                if let Some(of_type) = of_type {
                    terms.insert(*id, of_type);
                }
                Some((
                    *id,
                    Target {
                        entity: Some(entity),
                        owner: Some(owner.0),
                        extent,
                        body,
                        traits: traits_of(&grid, *id, armour.0, &body, of_type.as_ref()),
                        armour_pm: experience.map_or(1000, crate::Experience::armour_pm),
                        item,
                    },
                ))
            },
        )
        .collect();
    let field = Field {
        targets: &index,
        props: &props,
        players: &players,
        definitions: &definitions,
        grid: &grid,
        tick: tick.0,
        special: &special,
        roles: &roles,
        floor: i32::from(definitions.rules().is_some()),
        crushers: &crushers,
    };
    let mut shots = Shots::default();

    // 1. Projectiles in flight.
    let mut in_flight = Vec::with_capacity(projectiles.list.len());
    for mut projectile in std::mem::take(&mut projectiles.list) {
        projectile.remaining = projectile.remaining.saturating_sub(1);
        if projectile.remaining > 0 {
            in_flight.push(projectile);
            continue;
        }
        let Some(def) = definitions.weapon(projectile.weapon) else {
            continue;
        };
        let spread = definitions
            .warhead(def.warhead)
            .map_or(0, |warhead| warhead.cell_spread);
        let target = field.target(projectile.target);
        if target.is_none() {
            shots.events.push(SimEvent::ProjectileExpired {
                attacker: projectile.attacker,
                target: projectile.target,
                weapon: projectile.weapon,
            });
        }
        // An accurate projectile hits its target where that now is; without its target it
        // still bursts where it was aimed if its warhead has a spread. An inaccurate one lands
        // where it was aimed.
        let blast = match target {
            Some(target) if !projectile.scattered => Some(Blast {
                point: match target.body {
                    Body::Unit { .. } => target.extent.centre(),
                    Body::Building(_) | Body::Prop(_) => projectile.aim,
                },
                air: target.traits.airborne,
                warhead: def.warhead,
                damage: def.damage,
                direct: Some(projectile.target),
                firepower_pm: projectile.firepower_pm,
                source: Some(projectile.attacker),
            }),
            _ if projectile.scattered || spread > 0 => Some(Blast {
                point: projectile.aim,
                air: projectile.air,
                warhead: def.warhead,
                damage: def.damage,
                direct: None,
                firepower_pm: projectile.firepower_pm,
                source: Some(projectile.attacker),
            }),
            _ => None,
        };
        shots.blasts.extend(blast);
    }

    // 2. Armed units and buildings, in id order.
    let mut order: Vec<(SimId, Entity)> = Vec::new();
    for (id, ..) in attackers.iter() {
        if let Some(entity) = index.get(id).and_then(|target| target.entity) {
            order.push((*id, entity));
        }
    }
    order.sort_unstable();
    for (id, entity) in order {
        let Ok((_, owner, mut weapon, position, mobile, orders, building, experience)) =
            attackers.get_mut(entity)
        else {
            continue;
        };
        weapon.cooldown = weapon.cooldown.saturating_sub(1);
        // A unit that is switched off or rearming does not fire, nor does an aircraft that has
        // spent its ammunition until it has reloaded (RA4). A shot of a volley that is due
        // meanwhile is not fired, so the volley ends and the weapon reloads, as for every shot
        // that cannot be fired when it is due.
        if special.disabled.contains_key(&id)
            || special.rearming.contains_key(&id)
            || special.ammo.get(&id) == Some(&0)
        {
            if weapon.cooldown == 0 && weapon.burst_left > 0 {
                let reload_pm = experience.map_or(1000, crate::Experience::reload_pm);
                weapon.burst_left = 0;
                weapon.cooldown = definitions.weapon(weapon.volley).map_or(0, |def| {
                    if reload_pm == 1000 {
                        def.reload
                    } else {
                        crate::veterancy::reload_ticks(def.reload, reload_pm)
                    }
                });
            }
            continue;
        }
        // Every weapon kind comes from the definitions, which do not change during a match,
        // and every armed unit and building is of a type with combat terms.
        let Some(&of_type) = terms.get(&id) else {
            continue;
        };
        let launcher = launchers.get(&id).and_then(|&(slot, number)| {
            let kind = match slot {
                crate::Launcher::Primary => Some(weapon.kind),
                crate::Launcher::Secondary => weapon.secondary,
            }?;
            Some((kind, special.launch_ready(id, number)))
        });
        let arms = Arms {
            weapons: [Some(weapon.kind), weapon.secondary],
            terms: of_type,
            firepower_pm: experience.map_or(1000, crate::Experience::firepower_pm),
            reload_pm: experience.map_or(1000, crate::Experience::reload_pm),
            launcher,
        };
        let from = match (position, building) {
            (Some(position), _) => Extent::point(position.0),
            (None, Some(building)) => Extent::footprint(building.footprint),
            (None, None) => continue,
        };
        let owner = owner.0;
        // A shot of a volley is due now: if it is not fired, the volley ends.
        let due = weapon.cooldown == 0 && weapon.burst_left > 0;
        match (mobile, orders) {
            (Some(mobile), Some(mut orders)) => act_unit(
                &field,
                &mut shots,
                &mut rng,
                (id, owner, &from, mobile),
                (&mut weapon, &arms),
                &mut orders,
            ),
            _ => {
                // A building: a defensive structure, which does not fire while power does not
                // let it work (`PlayerState::powers`: without a rules file under low power;
                // with one, RA2, under low power if it is `powered` and consumes power).
                let low_power = players.get(owner).is_some_and(|state| match building {
                    Some(building) => !state.powers(&definitions, building.kind),
                    None => state.low_power(),
                });
                weapon.target = if low_power {
                    None
                } else {
                    keep_or_acquire(&field, owner, &from, weapon.target, &arms)
                };
                fire_at_target(
                    &field,
                    &mut shots,
                    &mut rng,
                    (id, owner, &from),
                    &mut weapon,
                    &arms,
                );
            }
        }
        if due && weapon.cooldown == 0 {
            weapon.burst_left = 0;
            weapon.cooldown = definitions
                .weapon(weapon.volley)
                .map_or(0, |def| arms.reload(def));
        }
    }

    // 3. Damage: every impact's hits, summed per target in `i64` (invariant I12), then applied
    // in target id order (scenery has the highest ids). The hit that brings a target's hit
    // points to 0, in the order of the hits, names its killer (RA4).
    let mut hits = Vec::new();
    {
        let traits = |id: SimId| {
            index.get(&id).map(|target| {
                let position =
                    matches!(target.body, Body::Unit { .. }).then(|| target.extent.centre());
                (position, target.traits, target.armour_pm)
            })
        };
        for blast in &shots.blasts {
            blast_hits(&definitions, &grid, &props, &traits, blast, &mut hits);
        }
    }
    let mut damage: BTreeMap<SimId, Vec<(i64, Option<SimId>)>> = BTreeMap::new();
    for (target, hit, source) in hits {
        damage
            .entry(target)
            .or_default()
            .push((i64::from(hit), source));
    }
    let mut prop_damage = Vec::new();
    for (target, hits) in damage {
        if special.invulnerable(target, tick.0) {
            continue;
        }
        let damage = hits
            .iter()
            .fold(0i64, |total, (hit, _)| total.saturating_add(*hit));
        let Some(found) = index.get(&target) else {
            if let Some((cell, _)) = props.get(target) {
                prop_damage.push((cell, damage));
            }
            continue;
        };
        let Some(entity) = found.entity else {
            continue;
        };
        if let Ok((_, _, _, mut health, ..)) = targets.get_mut(entity) {
            let before = i64::from(health.hit_points);
            let left = before.saturating_sub(damage);
            health.hit_points = i32::try_from(left.max(i64::from(i32::MIN))).unwrap_or(i32::MIN);
            if ruled && before > 0 && left <= 0 {
                let mut remaining = before;
                let killer = hits.iter().find_map(|&(hit, source)| {
                    remaining = remaining.saturating_sub(hit);
                    (remaining <= 0).then_some(source)
                });
                if let (Some(Some(killer)), Some(owner), Some(item)) =
                    (killer, found.owner, found.item)
                {
                    kills.record(killer, owner, crate::veterancy::worth(&definitions, item));
                }
            }
        }
    }
    in_flight.extend(shots.fired);
    projectiles.list = in_flight;
    let shot_events = shots.events;
    let area = shots.blasts;
    for (cell, damage) in prop_damage {
        props.hurt(cell, damage);
    }
    for event in shot_events {
        events.write(event);
    }
    // An impact that struck no target of a shot is reported on its own: the interface cannot
    // tell where it is from the shot's event.
    for blast in area.iter().filter(|blast| blast.direct.is_none()) {
        events.write(impact_event(blast));
    }
}

/// The event that reports `blast` to the interface.
fn impact_event(blast: &Blast) -> SimEvent {
    SimEvent::Impact {
        warhead: blast.warhead,
        at: blast.point,
        air: blast.air,
    }
}

/// An impact outside the `Combat` stage's own shots: a bomb, a superweapon, a suicide unit, a
/// radiation zone (`crate::special`). Its damage is applied at once, and if `report` it is
/// reported with [`SimEvent::Impact`].
pub(crate) fn detonate(world: &mut bevy_ecs::world::World, blast: Blast, report: bool) {
    let definitions = world.resource::<Definitions>().clone();
    let tick = world.resource::<SimTick>().0;
    // Per entity: where it is, its traits, its armour multiplier, its owner and what it is.
    type Found = (
        Entity,
        Option<SubPos>,
        TargetTraits,
        u32,
        PlayerId,
        Option<crate::ProductionItem>,
    );
    let mut found: BTreeMap<SimId, Found> = BTreeMap::new();
    {
        let mut query = world.query::<(
            Entity,
            &SimId,
            &Owner,
            &Armour,
            Option<&Position>,
            Option<&Mobile>,
            Option<&Building>,
            Option<&crate::Unit>,
            Option<&crate::Experience>,
        )>();
        let grid = world.resource::<Grid>();
        for (entity, id, owner, armour, position, mobile, building, unit, experience) in
            query.iter(world)
        {
            let Some(body) = Body::of(mobile, building) else {
                continue;
            };
            let of_type = match (unit, building) {
                (Some(unit), _) => definitions.unit_combat(unit.kind),
                (None, Some(building)) => definitions.building_combat(building.kind),
                (None, None) => None,
            };
            let item = match (unit, building) {
                (Some(unit), _) => Some(crate::ProductionItem::Unit(unit.kind)),
                (None, Some(building)) => Some(crate::ProductionItem::Building(building.kind)),
                (None, None) => None,
            };
            found.insert(
                *id,
                (
                    entity,
                    position.map(|position| position.0),
                    traits_of(grid, *id, armour.0, &body, of_type.as_ref()),
                    experience.map_or(1000, crate::Experience::armour_pm),
                    owner.0,
                    item,
                ),
            );
        }
    }
    let mut hits = Vec::new();
    {
        let traits = |id: SimId| {
            found
                .get(&id)
                .map(|&(_, position, traits, armour_pm, ..)| (position, traits, armour_pm))
        };
        blast_hits(
            &definitions,
            world.resource::<Grid>(),
            world.resource::<crate::Props>(),
            &traits,
            &blast,
            &mut hits,
        );
    }
    for (id, damage, source) in hits {
        if world
            .resource::<crate::SpecialState>()
            .invulnerable(id, tick)
        {
            continue;
        }
        match found.get(&id) {
            Some(&(entity, _, _, _, owner, item)) => {
                let mut killed = false;
                if let Some(mut health) = world.get_mut::<Health>(entity) {
                    let before = health.hit_points;
                    health.hit_points = before.saturating_sub(damage);
                    killed = before > 0 && health.hit_points <= 0;
                }
                // RA4: whoever caused the impact is the killer of what it destroys.
                if killed
                    && definitions.rules().is_some()
                    && let (Some(killer), Some(item)) = (source, item)
                {
                    let cost = crate::veterancy::worth(&definitions, item);
                    world
                        .resource_mut::<crate::veterancy::Kills>()
                        .record(killer, owner, cost);
                }
            }
            None => {
                let cell = world
                    .resource::<crate::Props>()
                    .get(id)
                    .map(|(cell, _)| cell);
                if let Some(cell) = cell {
                    world
                        .resource_mut::<crate::Props>()
                        .hurt(cell, i64::from(damage));
                }
            }
        }
    }
    if report {
        world
            .resource_mut::<Messages<SimEvent>>()
            .write(impact_event(&blast));
    }
}

/// The current target if it is still a visible enemy that `arms` engages on its own and may
/// fire at from `from`, else the nearest such enemy (automatic acquisition: idle and
/// attack-moving units, and defensive structures).
fn keep_or_acquire(
    field: &Field<'_>,
    owner: PlayerId,
    from: &Extent,
    current: Option<SimId>,
    arms: &Arms,
) -> Option<SimId> {
    current
        .filter(|&target| {
            field.visible_enemy(owner, target).is_some_and(|found| {
                let (dx, dy) = from.gap(&found.extent);
                field
                    .engages(arms, &found.traits)
                    .is_some_and(|choice| choice.covers(dx, dy))
            })
        })
        .or_else(|| field.nearest_enemy_in_range(owner, from, arms))
}

/// Fires at the weapon's target if it is a visible enemy that a weapon of `arms` may be fired
/// at; returns the weapon's choice if it fired.
fn fire_at_target<'a>(
    field: &'a Field<'_>,
    shots: &mut Shots,
    rng: &mut crate::SimRng,
    attacker: (SimId, PlayerId, &Extent),
    weapon: &mut Weapon,
    arms: &Arms,
) -> Option<Choice<'a>> {
    let (_, owner, _) = attacker;
    let target_id = weapon.target?;
    let target = field.visible_enemy(owner, target_id)?;
    let choice = field.choose(arms, &target.traits)?;
    fire_if_ready(
        field, shots, rng, attacker, weapon, arms, &choice, target_id, &target,
    )
    .then_some(choice)
}

/// Fires the weapon of `choice` at `target` if it is loaded and the target is where the weapon
/// may fire at it; returns whether it fired. A volley under way belongs to the weapon that
/// began it: if another weapon is called for, the volley ends and its weapon reloads.
#[allow(clippy::too_many_arguments)]
fn fire_if_ready(
    field: &Field<'_>,
    shots: &mut Shots,
    rng: &mut crate::SimRng,
    attacker: (SimId, PlayerId, &Extent),
    weapon: &mut Weapon,
    arms: &Arms,
    choice: &Choice<'_>,
    target_id: SimId,
    target: &Target,
) -> bool {
    let (_, _, from) = attacker;
    let (dx, dy) = from.gap(&target.extent);
    if weapon.cooldown > 0 || !choice.covers(dx, dy) {
        return false;
    }
    // RA4: a launcher waits until one of the units it spawns is ready.
    if arms.launcher == Some((choice.kind, false)) {
        return false;
    }
    if weapon.burst_left > 0 && weapon.volley != choice.kind {
        weapon.burst_left = 0;
        weapon.cooldown = field
            .definitions
            .weapon(weapon.volley)
            .map_or(1, |def| arms.reload(def));
        return false;
    }
    weapon.burst_left = if weapon.burst_left > 0 {
        weapon.burst_left - 1
    } else {
        choice.def.burst.saturating_sub(1)
    };
    weapon.volley = choice.kind;
    weapon.cooldown = if weapon.burst_left > 0 {
        BURST_GAP
    } else {
        arms.reload(choice.def)
    };
    field.fire(shots, rng, attacker, arms, choice, (target_id, target));
    true
}

/// One armed unit's turn: its combat order, or idle fire.
fn act_unit(
    field: &Field<'_>,
    shots: &mut Shots,
    rng: &mut crate::SimRng,
    (id, owner, from, mobile): (SimId, PlayerId, &Extent, &Mobile),
    (weapon, arms): (&mut Weapon, &Arms),
    orders: &mut Orders,
) {
    match weapon.order {
        Some(AttackOrder::Attack { target, chases }) => {
            let end = |weapon: &mut Weapon, shots: &mut Shots, reason| {
                weapon.clear();
                shots.events.push(SimEvent::AttackEnded {
                    unit: id,
                    target,
                    reason,
                });
            };
            let Some(found) = field.target(target) else {
                orders.current = None;
                return end(weapon, shots, AttackEnd::TargetGone);
            };
            let found = &found;
            if field.visible_enemy(owner, target).is_none() {
                orders.current = None;
                return end(weapon, shots, AttackEnd::TargetHidden);
            }
            weapon.target = Some(target);
            let Some(choice) = field.choose(arms, &found.traits) else {
                // No weapon of the unit may be fired at it at all.
                orders.current = None;
                return end(weapon, shots, AttackEnd::NoDamage);
            };
            let (dx, dy) = from.gap(&found.extent);
            if choice.covers(dx, dy) {
                // Stop at the next cell centre and fight.
                orders.current = None;
                if fire_if_ready(
                    field,
                    shots,
                    rng,
                    (id, owner, from),
                    weapon,
                    arms,
                    &choice,
                    target,
                    found,
                ) {
                    if choice.versus <= 0 {
                        // The shot, as ordered, did nothing; firing on would never end.
                        return end(weapon, shots, AttackEnd::NoDamage);
                    }
                    weapon.order = Some(AttackOrder::Attack { target, chases: 0 });
                }
                return;
            }
            // A unit approaches to the weapon's plain range (the air range bonus lengthens
            // only the shot), and keeps outside its minimum range. A unit on foot will stand
            // at one of the three spots of the cell, whichever is free when it arrives: the
            // target must be in range from each of them.
            let on_foot = field.grid.is_on_foot(id);
            let fires_from = |point: SubPos| {
                let (dx, dy) = Extent::point(point).gap(&found.extent);
                in_range(dx, dy, choice.def.range)
                    && !inside_min_range(dx, dy, choice.def.min_range)
            };
            let reachable = |cell: CellPos| {
                if on_foot {
                    (0..SPOTS_PER_CELL as u8).all(|spot| fires_from(SubPos::spot_of(cell, spot)))
                } else {
                    fires_from(SubPos::centre_of(cell))
                }
            };
            let heading_in_range = orders
                .current
                .as_ref()
                .is_some_and(|order| reachable(order.destination));
            if heading_in_range {
                return;
            }
            if chases >= MAX_CHASES {
                orders.current = None;
                return end(weapon, shots, AttackEnd::OutOfReach);
            }
            let view = field.grid.navigation_for(field.grid.domain_of(id));
            // The cell must have room for the unit beside the units that hold it and those
            // that chose it in this tick.
            let needed = Grid::room_needed(on_foot);
            let chase = nearest_cell(&view, mobile.next_centre(), |cell| {
                view.room(cell, on_foot, |holder| holder != id)
                    .saturating_sub(shots.chase_cells.get(&cell).copied().unwrap_or(0))
                    >= needed
                    && reachable(cell)
            });
            match chase {
                Some(cell) => {
                    let promised = shots.chase_cells.entry(cell).or_default();
                    *promised = promised.saturating_add(needed);
                    orders.current = Some(MoveOrder::new(cell, field.tick));
                    weapon.order = Some(AttackOrder::Attack {
                        target,
                        chases: chases.saturating_add(1),
                    });
                }
                None => {
                    orders.current = None;
                    end(weapon, shots, AttackEnd::OutOfReach);
                }
            }
        }
        Some(AttackOrder::AttackMove {
            destination,
            moving,
        }) => {
            weapon.target = keep_or_acquire(field, owner, from, weapon.target, arms);
            if weapon.target.is_some() {
                // Stop at the next cell centre and fight; the move resumes afterwards.
                orders.current = None;
                weapon.order = Some(AttackOrder::AttackMove {
                    destination,
                    moving: false,
                });
                fire_at_target(field, shots, rng, (id, owner, from), weapon, arms);
                return;
            }
            if orders.current.is_some() {
                return;
            }
            if moving {
                // The move to the destination ended; its event says how.
                weapon.clear();
                return;
            }
            // The move of an `AttackMove` command: a crusher may crush on its way.
            orders.current = Some(MoveOrder::commanded(
                destination,
                field.tick,
                field.crushers.contains(&id),
            ));
            weapon.order = Some(AttackOrder::AttackMove {
                destination,
                moving: true,
            });
        }
        None => {
            if orders.current.is_some() {
                // A plain move: no fire.
                weapon.target = None;
                return;
            }
            weapon.target = keep_or_acquire(field, owner, from, weapon.target, arms);
            fire_at_target(field, shots, rng, (id, owner, from), weapon, arms);
        }
    }
}

type ConditionQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static SimId,
        &'static Owner,
        &'static Health,
        &'static mut Condition,
        Option<&'static crate::Unit>,
        Option<&'static Building>,
    ),
>;

/// Sets the damage state of every unit and building from its hit points, in id order, and
/// reports every change (the `Cleanup` stage, with a rules file; RA3).
pub(crate) fn update_conditions(
    definitions: Res<Definitions>,
    mut events: ResMut<Messages<SimEvent>>,
    mut entities: ConditionQuery,
) {
    let Some(rules) = definitions.rules().map(|rules| rules.combat) else {
        return;
    };
    let mut changed = Vec::new();
    for (id, owner, health, mut condition, unit, building) in &mut entities {
        let full = match (unit, building) {
            (Some(unit), _) => definitions.unit_combat(unit.kind),
            (None, Some(building)) => definitions.building_combat(building.kind),
            (None, None) => None,
        }
        .map(|terms| terms.hit_points);
        let Some(full) = full else {
            continue;
        };
        if health.hit_points <= 0 {
            continue;
        }
        let state = damage_state(
            health.hit_points,
            full,
            rules.condition_yellow_percent,
            rules.condition_red_percent,
        );
        if condition.0 != state {
            condition.0 = state;
            changed.push((*id, owner.0, state));
        }
    }
    changed.sort_unstable();
    for (id, player, state) in changed {
        events.write(SimEvent::DamageStateChanged { player, id, state });
    }
}

type DeadQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static SimId,
        &'static Owner,
        &'static Health,
        Option<&'static Mobile>,
        Option<&'static Building>,
        Option<&'static crate::Unit>,
    ),
>;

/// Removes every unit and building whose hit points are ≤ 0, in id order, frees its cells, and
/// reports it (the `Cleanup` stage). Each is recorded as a wreck, whose crew `crate::crew` lets
/// out (RA4).
pub(crate) fn remove_dead(
    mut commands: Commands,
    mut grid: ResMut<Grid>,
    mut events: ResMut<Messages<SimEvent>>,
    mut wrecks: ResMut<crate::crew::Wrecks>,
    entities: DeadQuery,
) {
    let mut dead: Vec<_> = entities
        .iter()
        .filter(|(_, _, _, health, ..)| health.hit_points <= 0)
        .collect();
    dead.sort_unstable_by_key(|(_, id, ..)| **id);
    for (entity, &id, owner, _, mobile, building, unit) in dead {
        if let (Some(mobile), Some(unit)) = (mobile, unit) {
            wrecks.0.push(crate::crew::Wreck {
                owner: owner.0,
                id,
                what: crate::crew::Wrecked::Unit {
                    kind: unit.kind,
                    cell: mobile.cell,
                    moving: mobile.step.is_some(),
                },
            });
        }
        if let Some(mobile) = mobile {
            grid.unregister(id);
            grid.release(mobile.cell, id);
            if let Some(step) = mobile.step {
                grid.release(step.to, id);
            }
            events.write(SimEvent::UnitDestroyed {
                player: owner.0,
                unit: id,
            });
        }
        if let Some(building) = building {
            wrecks.0.push(crate::crew::Wreck {
                owner: owner.0,
                id,
                what: crate::crew::Wrecked::Building {
                    kind: building.kind,
                    footprint: building.footprint,
                },
            });
            for cell in building.footprint.cells() {
                grid.uncover(cell, id);
            }
            events.write(SimEvent::BuildingDestroyed {
                player: owner.0,
                building: id,
                kind: building.kind,
                lost_units: building.pending_units.clone(),
            });
        }
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat_defs::{MAX_DAMAGE, MAX_VERSUS_PERCENT, MAX_WEAPON_RANGE};

    /// Equation 2 at the limits the combat file admits.
    #[test]
    fn applied_damage_follows_equation_2_at_the_limits() {
        assert_eq!(applied_damage(100, 100), 100);
        assert_eq!(applied_damage(25, 30), 7, "floor of 7.5");
        assert_eq!(applied_damage(1, 1), 1, "at least 1");
        assert_eq!(applied_damage(99, 1), 1, "0.99 is raised to 1");
        assert_eq!(applied_damage(100, 1), 1);
        assert_eq!(applied_damage(MAX_DAMAGE, 0), 0, "versus 0 deals nothing");
        assert_eq!(applied_damage(1, 0), 0);
        assert_eq!(
            applied_damage(MAX_DAMAGE, MAX_VERSUS_PERCENT),
            MAX_DAMAGE * 10
        );
        // Values the data format does not admit still do not overflow (invariant I12).
        assert_eq!(applied_damage(i32::MAX, i32::MAX), i32::MAX);
        assert_eq!(applied_damage(i32::MAX, -5), 0);
    }

    /// Equation 1 at its boundary.
    #[test]
    fn in_range_includes_exactly_the_range() {
        assert!(in_range(768, 0, 768));
        assert!(!in_range(769, 0, 768));
        assert!(in_range(0, -768, 768));
        assert!(in_range(300, 400, 500), "3-4-5 exactly");
        assert!(!in_range(300, 401, 500));
        let max = i64::from(MAX_WEAPON_RANGE);
        assert!(in_range(max, 0, MAX_WEAPON_RANGE));
        assert!(!in_range(max + 1, 0, MAX_WEAPON_RANGE));
        assert!(!in_range(i64::MAX, i64::MIN, u32::MAX), "no overflow");
    }

    /// RA3: the hash of a projectile covers where it was aimed, whether it scatters, and its
    /// layer (invariant I3).
    #[test]
    fn the_hash_of_the_projectiles_covers_aim_scatter_and_layer() {
        let shell = Projectile {
            owner: PlayerId(0),
            attacker: SimId(1),
            target: SimId(2),
            weapon: WeaponKind(0),
            origin: SubPos::new(10, 20),
            aim: SubPos::new(300, 400),
            scattered: false,
            air: false,
            flight: 5,
            remaining: 3,
            firepower_pm: 1000,
        };
        let hash = |projectile: Projectile| {
            let mut hasher = Fnv1a64::new();
            Projectiles {
                list: vec![projectile],
            }
            .hash_into(&mut hasher);
            hasher.finish()
        };
        let base = hash(shell);
        assert_eq!(base, hash(shell));
        for changed in [
            Projectile {
                aim: SubPos::new(301, 400),
                ..shell
            },
            Projectile {
                aim: SubPos::new(300, 401),
                ..shell
            },
            Projectile {
                scattered: true,
                ..shell
            },
            Projectile { air: true, ..shell },
            Projectile {
                firepower_pm: 1100,
                ..shell
            },
        ] {
            assert_ne!(hash(changed), base, "{changed:?}");
        }
    }

    /// RA3: a shot at a building is aimed at the centre of the cell of its footprint nearest
    /// to the attacker, and a shot at a unit at the unit.
    #[test]
    fn a_shot_at_a_building_is_aimed_at_its_nearest_cell() {
        let building = Extent::footprint(Footprint {
            origin: CellPos::new(5, 4),
            width: 2,
            height: 3,
        });
        let from = |x, y| Extent::point(SubPos::centre_of(CellPos::new(x, y)));
        let aim = |x, y| building.aim_from(&from(x, y)).cell();
        assert_eq!(aim(2, 5), CellPos::new(5, 5), "from the west, in its rows");
        assert_eq!(aim(9, 9), CellPos::new(6, 6), "from the south east");
        assert_eq!(aim(5, 0), CellPos::new(5, 4), "from the north");
        assert_eq!(aim(6, 5), CellPos::new(6, 5), "from above it (an aircraft)");
        let unit = from(3, 3);
        assert_eq!(
            unit.aim_from(&from(0, 0)),
            SubPos::centre_of(CellPos::new(3, 3))
        );
    }

    #[test]
    fn extents_measure_between_the_nearest_points() {
        let unit = Extent::point(SubPos::centre_of(CellPos::new(2, 5)));
        let building = Extent::footprint(Footprint {
            origin: CellPos::new(5, 4),
            width: 2,
            height: 3,
        });
        // From x 640 to the footprint's left edge at 1280; y 1408 lies inside 1024..=1791.
        assert_eq!(unit.gap(&building), (640, 0));
        assert_eq!(building.gap(&unit), (640, 0));
        assert_eq!(building.gap(&building), (0, 0));
        assert_eq!(building.centre(), SubPos::new(1536, 1408));
    }
}
