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
//! Targets (docs/DECISIONS.md D29):
//!
//! * A unit with an `Attack` order fires only at its target. Out of range, it moves to the
//!   nearest free cell from whose centre the target is in range; it gives up after
//!   [`MAX_CHASES`] such moves without firing in between, or when the target no longer exists or
//!   is no longer visible to its owner ([`SimEvent::AttackEnded`]). In range, it stops.
//! * A unit with an `AttackMove` order moves to its destination; while a visible enemy is in
//!   range it stops and fires at the nearest one, then moves on. The order ends when the move
//!   ends (the move's own event reports how).
//! * A unit without any order, and a building with a weapon, fire at the nearest visible enemy in
//!   range without moving. A unit with a plain move order does not fire. A building with a
//!   weapon does not fire while its owner has low power (PLAN.md section 5, equation 3).
//!
//! Distances (equation 1) are measured in subcells between the nearest points of the two
//! entities: a unit is its fine position, a building the rectangle of subcells its footprint
//! covers. "Visible" means visible to the attacker's owner as of the last `Visibility` stage
//! (`crate::visibility`); a player who is not a player of the match sees nothing.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Commands, Query, Res, ResMut};

use crate::building::{Building, Footprint};
use crate::combat_defs::{ArmourClass, CombatTerms, WeaponDef, WeaponKind};
use crate::command::PlayerId;
use crate::definitions::Definitions;
use crate::economy::Players;
use crate::event::{AttackEnd, SimEvent};
use crate::hash::Fnv1a64;
use crate::id::SimId;
use crate::map::{CellPos, Grid, SUBCELLS_PER_CELL, SubPos};
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

/// The weapon of a unit or building, and what it does with it.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Weapon {
    pub kind: WeaponKind,
    /// Ticks until the weapon can fire again (0: loaded).
    pub cooldown: u32,
    /// The entity it aims at, if any.
    pub target: Option<SimId>,
    /// The unit's combat order; always `None` for a building.
    pub order: Option<AttackOrder>,
}

impl Weapon {
    /// Ends the combat order and forgets the target (`Move`, `Stop`, `Harvest`).
    pub(crate) fn clear(&mut self) {
        self.order = None;
        self.target = None;
    }

    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        hasher.write(&self.kind.0.to_le_bytes());
        hasher.write_u32(self.cooldown);
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
    /// Ticks of flight in all, at least 1.
    pub flight: u32,
    /// Ticks of flight left.
    pub remaining: u32,
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
            hasher.write_u32(projectile.flight);
            hasher.write_u32(projectile.remaining);
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

/// Equation 1 of PLAN.md section 5: whether a target at offset (`dx`, `dy`) subcells is within
/// `range` subcells, `dx² + dy² ≤ range²`. Computed in `i128`, exact for every input.
pub fn in_range(dx: i64, dy: i64, range: u32) -> bool {
    let (dx, dy, range) = (i128::from(dx), i128::from(dy), i128::from(range));
    dx * dx + dy * dy <= range * range
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
        }
    }
}

/// The components of a unit or building with the combat terms `terms`.
pub(crate) fn combat_components(terms: CombatTerms) -> ((Health, Armour, Sight), Option<Weapon>) {
    (
        (
            Health {
                hit_points: terms.hit_points,
            },
            Armour(terms.armour),
            Sight(terms.sight),
        ),
        terms.weapon.map(|kind| Weapon {
            kind,
            cooldown: 0,
            target: None,
            order: None,
        }),
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
struct Target {
    entity: Entity,
    owner: PlayerId,
    extent: Extent,
    body: Body,
    armour: ArmourClass,
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
    ),
>;

/// Everything an attacker needs to know in the tick.
struct Field<'a> {
    targets: &'a BTreeMap<SimId, Target>,
    players: &'a Players,
    definitions: &'a Definitions,
    grid: &'a Grid,
    tick: u64,
}

/// What the tick's shots add up to.
#[derive(Default)]
struct Shots {
    /// Damage per target, summed in `i64` (invariant I12).
    damage: BTreeMap<SimId, i64>,
    fired: Vec<Projectile>,
    events: Vec<SimEvent>,
    /// Cells chosen this tick as destinations of units moving toward their targets, so that
    /// units attacking the same target spread over different cells.
    chase_cells: BTreeSet<CellPos>,
}

impl Field<'_> {
    fn vision(&self, player: PlayerId) -> Option<&Vision> {
        self.players.get(player).map(|state| state.vision())
    }

    /// `target` if it is an enemy of `owner` that `owner` sees.
    fn visible_enemy(&self, owner: PlayerId, target: SimId) -> Option<&Target> {
        let found = self.targets.get(&target)?;
        let vision = self.vision(owner)?;
        (found.owner != owner && found.body.seen_by(vision)).then_some(found)
    }

    /// The visible enemy of `owner` nearest to `from` within `range` (ties by id).
    fn nearest_enemy_in_range(&self, owner: PlayerId, from: &Extent, range: u32) -> Option<SimId> {
        let vision = self.vision(owner)?;
        self.targets
            .iter()
            .filter(|(_, target)| target.owner != owner && target.body.seen_by(vision))
            .filter_map(|(&id, target)| {
                let (dx, dy) = from.gap(&target.extent);
                in_range(dx, dy, range).then(|| {
                    (
                        i128::from(dx) * i128::from(dx) + i128::from(dy) * i128::from(dy),
                        id,
                    )
                })
            })
            .min()
            .map(|(_, id)| id)
    }

    /// Fires `weapon` at `target`.
    fn fire(
        &self,
        shots: &mut Shots,
        attacker: (SimId, PlayerId, &Extent),
        weapon: (WeaponKind, &WeaponDef),
        target_id: SimId,
        target: &Target,
    ) {
        let (id, owner, from) = attacker;
        let (kind, def) = weapon;
        shots.events.push(SimEvent::WeaponFired {
            attacker: id,
            target: target_id,
            weapon: kind,
        });
        if def.projectile_speed == 0 {
            let versus = self
                .definitions
                .versus(def.warhead, target.armour)
                .unwrap_or(0);
            let total = shots.damage.entry(target_id).or_default();
            *total = total.saturating_add(i64::from(applied_damage(def.damage, versus)));
            return;
        }
        let (dx, dy) = from.gap(&target.extent);
        let distance = (dx.unsigned_abs().saturating_mul(dx.unsigned_abs()))
            .saturating_add(dy.unsigned_abs().saturating_mul(dy.unsigned_abs()))
            .isqrt();
        let flight = u32::try_from(distance.div_ceil(u64::from(def.projectile_speed)))
            .unwrap_or(u32::MAX)
            .max(1);
        shots.fired.push(Projectile {
            owner,
            attacker: id,
            target: target_id,
            weapon: kind,
            origin: from.centre(),
            flight,
            remaining: flight,
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
    mut targets: TargetQuery,
    mut attackers: AttackerQuery,
) {
    if attackers.is_empty() && projectiles.is_empty() {
        return;
    }
    let index: BTreeMap<SimId, Target> = targets
        .iter()
        .filter_map(
            |(entity, id, owner, _, armour, position, mobile, building)| {
                let extent = match (position, building) {
                    (Some(position), _) => Extent::point(position.0),
                    (None, Some(building)) => Extent::footprint(building.footprint),
                    (None, None) => return None,
                };
                Some((
                    *id,
                    Target {
                        entity,
                        owner: owner.0,
                        extent,
                        body: Body::of(mobile, building)?,
                        armour: armour.0,
                    },
                ))
            },
        )
        .collect();
    let field = Field {
        targets: &index,
        players: &players,
        definitions: &definitions,
        grid: &grid,
        tick: tick.0,
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
        let hit = index.get(&projectile.target).and_then(|target| {
            let def = definitions.weapon(projectile.weapon)?;
            let versus = definitions.versus(def.warhead, target.armour)?;
            Some(applied_damage(def.damage, versus))
        });
        match hit {
            Some(damage) => {
                let total = shots.damage.entry(projectile.target).or_default();
                *total = total.saturating_add(i64::from(damage));
            }
            None => shots.events.push(SimEvent::ProjectileExpired {
                attacker: projectile.attacker,
                target: projectile.target,
                weapon: projectile.weapon,
            }),
        }
    }

    // 2. Armed units and buildings, in id order.
    let mut order: Vec<(SimId, Entity)> = Vec::new();
    for (id, ..) in attackers.iter() {
        if let Some(target) = index.get(id) {
            order.push((*id, target.entity));
        }
    }
    order.sort_unstable();
    for (id, entity) in order {
        let Ok((_, owner, mut weapon, position, mobile, orders, building)) =
            attackers.get_mut(entity)
        else {
            continue;
        };
        weapon.cooldown = weapon.cooldown.saturating_sub(1);
        // Every weapon kind comes from the definitions, which do not change during a match.
        let Some(def) = definitions.weapon(weapon.kind) else {
            continue;
        };
        let from = match (position, building) {
            (Some(position), _) => Extent::point(position.0),
            (None, Some(building)) => Extent::footprint(building.footprint),
            (None, None) => continue,
        };
        let owner = owner.0;
        match (mobile, orders) {
            (Some(mobile), Some(mut orders)) => act_unit(
                &field,
                &mut shots,
                (id, owner, &from, mobile),
                (&mut weapon, def),
                &mut orders,
            ),
            _ => {
                // A building: a defensive structure, which does not fire under low power.
                let low_power = players.get(owner).is_some_and(|state| state.low_power());
                weapon.target = if low_power {
                    None
                } else {
                    keep_or_acquire(&field, owner, &from, weapon.target, def.range)
                };
                fire_if_ready(&field, &mut shots, (id, owner, &from), &mut weapon, def);
            }
        }
    }

    // 3. Damage, in target id order.
    for (target, damage) in shots.damage {
        let Some(entity) = index.get(&target).map(|target| target.entity) else {
            continue;
        };
        if let Ok((_, _, _, mut health, ..)) = targets.get_mut(entity) {
            let left = i64::from(health.hit_points).saturating_sub(damage);
            health.hit_points = i32::try_from(left.max(i64::from(i32::MIN))).unwrap_or(i32::MIN);
        }
    }
    in_flight.extend(shots.fired);
    projectiles.list = in_flight;
    for event in shots.events {
        events.write(event);
    }
}

/// The current target if it is still a visible enemy in range, else the nearest visible enemy in
/// range.
fn keep_or_acquire(
    field: &Field<'_>,
    owner: PlayerId,
    from: &Extent,
    current: Option<SimId>,
    range: u32,
) -> Option<SimId> {
    current
        .filter(|&target| {
            field.visible_enemy(owner, target).is_some_and(|found| {
                let (dx, dy) = from.gap(&found.extent);
                in_range(dx, dy, range)
            })
        })
        .or_else(|| field.nearest_enemy_in_range(owner, from, range))
}

/// Fires at the weapon's target if it is loaded and the target is a visible enemy in range;
/// returns whether it fired.
fn fire_if_ready(
    field: &Field<'_>,
    shots: &mut Shots,
    attacker: (SimId, PlayerId, &Extent),
    weapon: &mut Weapon,
    def: &WeaponDef,
) -> bool {
    let (_, owner, from) = attacker;
    let Some(target_id) = weapon.target else {
        return false;
    };
    let Some(target) = field.visible_enemy(owner, target_id) else {
        return false;
    };
    let (dx, dy) = from.gap(&target.extent);
    if weapon.cooldown > 0 || !in_range(dx, dy, def.range) {
        return false;
    }
    weapon.cooldown = def.reload;
    field.fire(shots, attacker, (weapon.kind, def), target_id, target);
    true
}

/// One armed unit's turn: its combat order, or idle fire.
fn act_unit(
    field: &Field<'_>,
    shots: &mut Shots,
    (id, owner, from, mobile): (SimId, PlayerId, &Extent, &Mobile),
    (weapon, def): (&mut Weapon, &WeaponDef),
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
            let Some(found) = field.targets.get(&target) else {
                orders.current = None;
                return end(weapon, shots, AttackEnd::TargetGone);
            };
            if field.visible_enemy(owner, target).is_none() {
                orders.current = None;
                return end(weapon, shots, AttackEnd::TargetHidden);
            }
            weapon.target = Some(target);
            let (dx, dy) = from.gap(&found.extent);
            if in_range(dx, dy, def.range) {
                // Stop at the next cell centre and fight.
                orders.current = None;
                if fire_if_ready(field, shots, (id, owner, from), weapon, def) {
                    weapon.order = Some(AttackOrder::Attack { target, chases: 0 });
                }
                return;
            }
            let reachable = |cell: CellPos| {
                let (dx, dy) = Extent::point(SubPos::centre_of(cell)).gap(&found.extent);
                in_range(dx, dy, def.range)
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
            let chase = nearest_cell(field.grid, mobile.next_centre(), |cell| {
                field.grid.occupant(cell).is_none_or(|holder| holder == id)
                    && !shots.chase_cells.contains(&cell)
                    && reachable(cell)
            });
            match chase {
                Some(cell) => {
                    shots.chase_cells.insert(cell);
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
            weapon.target = keep_or_acquire(field, owner, from, weapon.target, def.range);
            if weapon.target.is_some() {
                // Stop at the next cell centre and fight; the move resumes afterwards.
                orders.current = None;
                weapon.order = Some(AttackOrder::AttackMove {
                    destination,
                    moving: false,
                });
                fire_if_ready(field, shots, (id, owner, from), weapon, def);
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
            orders.current = Some(MoveOrder::new(destination, field.tick));
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
            weapon.target = keep_or_acquire(field, owner, from, weapon.target, def.range);
            fire_if_ready(field, shots, (id, owner, from), weapon, def);
        }
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
    ),
>;

/// Removes every unit and building whose hit points are ≤ 0, in id order, frees its cells, and
/// reports it (the `Cleanup` stage).
pub(crate) fn remove_dead(
    mut commands: Commands,
    mut grid: ResMut<Grid>,
    mut events: ResMut<Messages<SimEvent>>,
    entities: DeadQuery,
) {
    let mut dead: Vec<_> = entities
        .iter()
        .filter(|(_, _, _, health, ..)| health.hit_points <= 0)
        .collect();
    dead.sort_unstable_by_key(|(_, id, ..)| **id);
    for (entity, &id, owner, _, mobile, building) in dead {
        if let Some(mobile) = mobile {
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
