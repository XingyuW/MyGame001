//! Crates (RA5, docs/RA2_PARITY_PLAN.md section 4.11; docs/DECISIONS.md R60).
//!
//! Only with a rules file. A crate lies in a cell and belongs to nobody; it blocks nothing, and
//! no unit fires at it. The ground unit, ship or soldier that stands in its cell takes it (an
//! aircraft does not).
//!
//! **Where crates come from** (the `Economy` stage, [`appear`]): in a match with players whose
//! rule `crates` is on, the rules' `crates.minimum` crates appear in the first tick, and every
//! crate that is taken is replaced after a time drawn between half and twice `crates.regen`,
//! both inclusive, as long as fewer than `crates.maximum` lie on the map. A crate that nobody
//! takes lies for a time drawn from the same range and is then replaced at once by one
//! somewhere else, as is a crate a building was placed on. A crate appears in a cell drawn
//! from the whole map that is land a ground unit may enter, or water, and holds nothing: no
//! building, no unit, no ore and no crate. If [`CRATE_PLACE_ATTEMPTS`] draws find none, the
//! crate is tried again [`CRATE_RETRY_TICKS`] later. A world without a match gets no crates by
//! itself; a scenario puts them down with [`Crates::place`], and those lie until they are
//! taken.
//!
//! **What a crate holds** is drawn when it is taken (the `Movement` stage, [`collect`]), from
//! the rules' `powerups` by their shares:
//!
//! * `money`: the powerup's value plus a draw from 0 to [`CRATE_MONEY_SPREAD`] credits;
//! * `unit`: one unit of a type drawn from the rules' `crate_units`, in the nearest cell with
//!   room within [`CRATE_UNIT_RADIUS`] cells of the crate; money on water, without a type to
//!   draw, or without room;
//! * `veteran`: every unit of the collector's owner within `crates.radius` of the crate rises
//!   by the powerup's value in rank;
//! * `armour`, `firepower`, `speed`: every such unit that has none yet gets the powerup's
//!   value as the multiplier of that kind ([`Experience::bonus`](crate::Experience));
//! * `heal_base`: every unit and building of the collector's owner regains all its hit points;
//! * `reveal`: the collector's owner has explored the whole map.
//!
//! A collector that cannot gain from `veteran`, `armour`, `firepower` or `speed` (it is not
//! trainable, is elite already, or has the multiplier already) gets money instead. With the
//! rules' `crates.free_mcv`, a collector whose owner has no building and no base unit and more
//! than [`FREE_MCV_CREDITS`] credits gets the base unit of its side instead of a drawn content
//! (money on water).
//!
//! **Randomness** comes only from [`SimRng`], in this order for each crate taken, the crates of
//! a tick in the order of their cells: the content (unless the free base unit applies), then
//! what the content draws (the type of the unit; the credits of money, also of money given
//! instead), then the time until the crate is replaced. A crate that appears draws its cell,
//! two draws an attempt, and then the time it lies.

use std::collections::BTreeMap;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Res, ResMut};
use bevy_ecs::world::World;

use crate::building::{Building, Footprint};
use crate::combat::Health;
use crate::command::PlayerId;
use crate::definitions::{Definitions, UnitKind};
use crate::economy::Players;
use crate::event::SimEvent;
use crate::hash::Fnv1a64;
use crate::id::SimId;
use crate::map::{CellPos, Grid, SUBCELLS_PER_CELL};
use crate::match_setup::MatchState;
use crate::rng::SimRng;
use crate::rules::Rules;
use crate::schedule::SimTick;
use crate::unit::{Mobile, Owner, Unit};
use crate::veterancy::{ELITE, Experience};

/// The most credits a crate of money adds to its powerup's value: a draw from 0 to this, both
/// inclusive. The ini has no key for it (docs/RA2_PARITY_PLAN.md appendix D, topic 10).
pub const CRATE_MONEY_SPREAD: u32 = 900;

/// A collector's owner gets its side's base unit from a crate only with more credits than this
/// (the rule `free_mcv`; hard coded in the original).
pub const FREE_MCV_CREDITS: i64 = 1_500;

/// Cells drawn for a crate that is due before it is put off.
pub const CRATE_PLACE_ATTEMPTS: u32 = 64;

/// Ticks a crate that found no cell waits before it is tried again.
pub const CRATE_RETRY_TICKS: u64 = 20;

/// A crate's unit appears at most this many cells (Chebyshev) from the crate.
pub const CRATE_UNIT_RADIUS: u8 = 3;

/// What a crate held for its collector.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CrateContent {
    Money,
    Unit,
    Veteran,
    HealBase,
    Armour,
    Firepower,
    Speed,
    Reveal,
}

impl CrateContent {
    /// Every content, in the order of [`CrateContent::id`]'s list.
    pub const ALL: [CrateContent; 8] = [
        CrateContent::Money,
        CrateContent::Unit,
        CrateContent::Veteran,
        CrateContent::HealBase,
        CrateContent::Armour,
        CrateContent::Firepower,
        CrateContent::Speed,
        CrateContent::Reveal,
    ];

    /// The content's name in the rules file's `powerups`.
    pub fn id(self) -> &'static str {
        match self {
            CrateContent::Money => "money",
            CrateContent::Unit => "unit",
            CrateContent::Veteran => "veteran",
            CrateContent::HealBase => "heal_base",
            CrateContent::Armour => "armour",
            CrateContent::Firepower => "firepower",
            CrateContent::Speed => "speed",
            CrateContent::Reveal => "reveal",
        }
    }

    /// The content named `id` in a rules file.
    pub fn parse(id: &str) -> Option<CrateContent> {
        Self::ALL.into_iter().find(|content| content.id() == id)
    }
}

/// The crates of the match.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct Crates {
    /// The cells a crate lies in, each with the tick from which the crate is gone if nobody
    /// took it (`u64::MAX`: it lies until it is taken).
    cells: BTreeMap<CellPos, u64>,
    /// The ticks at which a crate is due to appear, ascending.
    due: Vec<u64>,
    /// Whether the first crates of the match have been asked for.
    started: bool,
}

impl Crates {
    /// The cells a crate lies in, in ascending order (by x, then y).
    pub fn cells(&self) -> impl Iterator<Item = CellPos> + '_ {
        self.cells.keys().copied()
    }

    /// How many crates lie on the map.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Whether a crate lies in `cell`.
    pub fn contains(&self, cell: CellPos) -> bool {
        self.cells.contains_key(&cell)
    }

    /// The tick from which the crate in `cell` is gone if nobody took it: `None` without a
    /// crate there, `Some(u64::MAX)` for one that lies until it is taken.
    pub fn expires(&self, cell: CellPos) -> Option<u64> {
        self.cells.get(&cell).copied()
    }

    /// The ticks at which a crate is due to appear, ascending.
    pub fn due(&self) -> &[u64] {
        &self.due
    }

    /// Puts a crate into `cell` if one may lie there ([`crate_fits`]); it lies until it is
    /// taken. Scenario setup only, like [`Grid::set_terrain`]; in a match crates appear by
    /// themselves.
    pub fn place(&mut self, grid: &Grid, cell: CellPos) -> bool {
        crate_fits(grid, self, cell) && self.cells.insert(cell, u64::MAX).is_none()
    }

    /// Nothing for a world that never had a crate, so that it hashes as before RA5.
    pub(crate) fn hash_into(&self, hasher: &mut Fnv1a64) {
        if *self == Self::default() {
            return;
        }
        hasher.write(b"crates");
        hasher.write(&[u8::from(self.started)]);
        hasher.write_u64(self.cells.len() as u64);
        for (cell, expires) in &self.cells {
            hasher.write(&cell.x.to_le_bytes());
            hasher.write(&cell.y.to_le_bytes());
            hasher.write_u64(*expires);
        }
        hasher.write_u64(self.due.len() as u64);
        for tick in &self.due {
            hasher.write_u64(*tick);
        }
    }

    /// Asks for a crate at `tick`, keeping the list ascending.
    fn ask(&mut self, tick: u64) {
        let at = self.due.partition_point(|&due| due <= tick);
        self.due.insert(at, tick);
    }
}

/// Whether a crate may lie in `cell`: land a ground unit may enter, or water, that holds no
/// building, no unit, no ore and no crate.
pub fn crate_fits(grid: &Grid, crates: &Crates, cell: CellPos) -> bool {
    grid.cell(cell)
        .is_some_and(|terrain| (terrain.passable || terrain.water) && terrain.ore == 0)
        && grid.building(cell).is_none()
        && grid.occupant_for(cell, crate::Domain::Land).is_none()
        && !crates.contains(cell)
}

/// Lets the crates that are due appear (see the module documentation).
pub(crate) fn appear(
    definitions: Res<Definitions>,
    state: Res<MatchState>,
    grid: Res<Grid>,
    tick: Res<SimTick>,
    mut crates: ResMut<Crates>,
    mut rng: ResMut<SimRng>,
    mut events: ResMut<Messages<SimEvent>>,
) {
    let Some(rules) = definitions.rules().map(|rules| rules.crates) else {
        return;
    };
    if !state.rules().crates || state.players().is_empty() {
        return;
    }
    if !crates.started {
        crates.started = true;
        for _ in 0..rules.minimum.max(0) {
            crates.ask(tick.0);
        }
    }
    // A crate whose time is up, or on whose cell a building was placed, is gone and replaced
    // at once.
    let gone: Vec<CellPos> = crates
        .cells
        .iter()
        .filter(|&(&cell, &expires)| expires <= tick.0 || grid.building(cell).is_some())
        .map(|(&cell, _)| cell)
        .collect();
    for cell in gone {
        crates.cells.remove(&cell);
        crates.ask(tick.0);
        events.write(SimEvent::CrateGone { cell });
    }
    let maximum = usize::try_from(rules.maximum.max(0)).unwrap_or(0);
    let (width, height) = (grid.width(), grid.height());
    // Between half and twice the rules' time, both inclusive: one draw.
    let regen = u64::try_from(rules.regen.max(1)).unwrap_or(1);
    let (low, high) = (regen / 2, regen.saturating_mul(2));
    while crates.due.first().is_some_and(|&due| due <= tick.0) {
        if crates.cells.len() >= maximum || width == 0 || height == 0 {
            crates.due.remove(0);
            continue;
        }
        let mut found = None;
        for _ in 0..CRATE_PLACE_ATTEMPTS {
            // A side is at most `MAX_MAP_SIDE`, so both remainders are coordinates.
            let x = i32::try_from(rng.next_u32() % width).unwrap_or(0);
            let y = i32::try_from(rng.next_u32() % height).unwrap_or(0);
            let cell = CellPos::new(x, y);
            if crate_fits(&grid, &crates, cell) {
                found = Some(cell);
                break;
            }
        }
        crates.due.remove(0);
        match found {
            Some(cell) => {
                let lies = (low + u64::from(rng.next_u32()) % (high - low + 1)).max(1);
                crates.cells.insert(cell, tick.0.saturating_add(lies));
                events.write(SimEvent::CrateAppeared { cell });
            }
            None => {
                // No free cell among the draws: later, and nothing else in this tick.
                crates.ask(tick.0.saturating_add(CRATE_RETRY_TICKS));
                break;
            }
        }
    }
}

/// A unit on the map with hit points left.
#[derive(Clone, Copy)]
struct Soldier {
    entity: Entity,
    id: SimId,
    owner: PlayerId,
    kind: UnitKind,
    cell: CellPos,
}

/// The units on the map with hit points left, by id.
fn soldiers(world: &mut World) -> BTreeMap<SimId, Soldier> {
    world
        .query::<(Entity, &SimId, &Owner, &Unit, &Mobile, &Health)>()
        .iter(world)
        .filter(|(.., health)| health.hit_points > 0)
        .map(|(entity, id, owner, unit, mobile, _)| {
            (
                *id,
                Soldier {
                    entity,
                    id: *id,
                    owner: owner.0,
                    kind: unit.kind,
                    cell: mobile.cell,
                },
            )
        })
        .collect()
}

/// Lets the units standing on crates take them (see the module documentation).
pub(crate) fn collect(world: &mut World) {
    if world.resource::<Crates>().is_empty() {
        return;
    }
    let definitions = world.resource::<Definitions>().clone();
    let Some(rules) = definitions.rules() else {
        return;
    };
    // The crates with a unit in their cell, before any unit is looked up.
    let held: Vec<(CellPos, Vec<SimId>)> = {
        let grid = world.resource::<Grid>();
        world
            .resource::<Crates>()
            .cells()
            .filter_map(|cell| {
                let mut holders: Vec<SimId> = grid
                    .holders_for(cell, crate::Domain::Land)
                    .into_iter()
                    .flatten()
                    .collect();
                holders.sort_unstable();
                (!holders.is_empty()).then_some((cell, holders))
            })
            .collect()
    };
    if held.is_empty() {
        return;
    }
    let tick = world.resource::<SimTick>().0;
    for (cell, holders) in held {
        // The units of the tick's earlier crates may have changed: look them up anew.
        let units = soldiers(world);
        // The unit with the lowest id that stands in the cell (one that only reserved it has
        // not arrived yet), of a player of the match.
        let Some(collector) = holders
            .iter()
            .filter_map(|id| units.get(id))
            .find(|unit| unit.cell == cell && world.resource::<Players>().get(unit.owner).is_some())
            .copied()
        else {
            continue;
        };
        world.resource_mut::<Crates>().cells.remove(&cell);
        let (content, credits, unit) = open(world, &definitions, rules, &units, collector, cell);
        // The crate's replacement: between half and twice the rules' time, both inclusive.
        let regen = u64::try_from(rules.crates.regen.max(1)).unwrap_or(1);
        let (low, high) = (regen / 2, regen.saturating_mul(2));
        let wait = low + u64::from(world.resource_mut::<SimRng>().next_u32()) % (high - low + 1);
        world
            .resource_mut::<Crates>()
            .ask(tick.saturating_add(wait.max(1)));
        world
            .resource_mut::<Messages<SimEvent>>()
            .write(SimEvent::CrateTaken {
                player: collector.owner,
                unit: collector.id,
                cell,
                content,
                credits,
                spawned: unit,
            });
    }
}

/// Gives `collector` the content of the crate in `cell`: what it was in the end, the credits
/// it paid, and the unit it held.
fn open(
    world: &mut World,
    definitions: &Definitions,
    rules: &Rules,
    units: &BTreeMap<SimId, Soldier>,
    collector: Soldier,
    cell: CellPos,
) -> (CrateContent, i64, Option<SimId>) {
    let water = world
        .resource::<Grid>()
        .cell(cell)
        .is_some_and(|terrain| terrain.water);
    let value = |content: CrateContent| {
        rules
            .powerups
            .iter()
            .find(|powerup| powerup.id == content.id())
            .map_or(0, |powerup| powerup.value.max(0))
    };
    // Money: the powerup's value and the draw.
    let money = |world: &mut World| {
        let spread = world.resource_mut::<SimRng>().next_u32() % (CRATE_MONEY_SPREAD + 1);
        let credits = i64::from(value(CrateContent::Money)).saturating_add(i64::from(spread));
        if let Some(player) = world.resource_mut::<Players>().get_mut(collector.owner) {
            player.credits = player.credits.saturating_add(credits);
        }
        (CrateContent::Money, credits, None)
    };
    let unit = |world: &mut World, kind: UnitKind| {
        let id = definitions.unit(kind)?.id.clone();
        let mut cells = vec![cell];
        for radius in 1..=CRATE_UNIT_RADIUS {
            let reach = i32::from(radius) - 1;
            cells.extend(
                Footprint {
                    origin: CellPos::new(
                        cell.x.saturating_sub(reach),
                        cell.y.saturating_sub(reach),
                    ),
                    width: 2 * radius - 1,
                    height: 2 * radius - 1,
                }
                .ring(),
            );
        }
        cells
            .into_iter()
            .find_map(|at| crate::spawn_unit(world, collector.owner, &id, at).ok())
    };

    // The free base unit comes before any draw.
    if rules.crates.free_mcv == 1 && stranded(world, rules, units, collector.owner) {
        let base_unit = world
            .resource::<Players>()
            .get(collector.owner)
            .and_then(|player| player.country())
            .and_then(|country| rules.country(country))
            .and_then(|country| rules.side(country.side))
            .map(|side| side.base_unit);
        if let Some(base_unit) = base_unit {
            if !water && let Some(new) = unit(world, base_unit) {
                return (CrateContent::Unit, 0, Some(new));
            }
            return money(world);
        }
    }

    let total: u32 = rules
        .powerups
        .iter()
        .filter(|powerup| CrateContent::parse(&powerup.id).is_some())
        .map(|powerup| u32::try_from(powerup.share.max(0)).unwrap_or(0))
        .sum();
    if total == 0 {
        // A rules file without contents: a crate of money.
        return money(world);
    }
    let mut draw = world.resource_mut::<SimRng>().next_u32() % total;
    let mut content = CrateContent::Money;
    for powerup in &rules.powerups {
        let Some(listed) = CrateContent::parse(&powerup.id) else {
            continue;
        };
        let share = u32::try_from(powerup.share.max(0)).unwrap_or(0);
        if draw < share {
            content = listed;
            break;
        }
        draw -= share;
    }

    // The units a crate of the collector's owner reaches: within the rules' radius of the
    // crate, measured between the centres of the cells.
    let radius = i64::from(rules.crates.radius.max(0));
    let reached: Vec<Soldier> = units
        .values()
        .filter(|unit| unit.owner == collector.owner)
        .filter(|unit| {
            let side = i64::from(SUBCELLS_PER_CELL);
            let dx = (i64::from(unit.cell.x) - i64::from(cell.x)).saturating_mul(side);
            let dy = (i64::from(unit.cell.y) - i64::from(cell.y)).saturating_mul(side);
            dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy))
                <= radius.saturating_mul(radius)
        })
        .copied()
        .collect();
    let experience = |world: &World, unit: &Soldier| {
        definitions.trainable(unit.kind).then(|| {
            world
                .get::<Experience>(unit.entity)
                .copied()
                .unwrap_or_default()
        })
    };
    match content {
        CrateContent::Money => money(world),
        CrateContent::Unit => {
            if water || rules.crate_units.is_empty() {
                return money(world);
            }
            let draw = world.resource_mut::<SimRng>().next_u32();
            // At most `MAX_CRATE_UNITS` types, so the remainder is an index.
            let pick = usize::try_from(draw % (rules.crate_units.len() as u32)).unwrap_or(0);
            match rules
                .crate_units
                .get(pick)
                .and_then(|&kind| unit(world, kind))
            {
                Some(new) => (CrateContent::Unit, 0, Some(new)),
                None => money(world),
            }
        }
        CrateContent::Veteran => {
            let cap =
                u8::try_from(rules.veterancy.veteran_cap.clamp(0, i32::from(ELITE))).unwrap_or(0);
            if experience(world, &collector).is_none_or(|own| own.rank >= cap) {
                return money(world);
            }
            let ranks = u8::try_from(value(content).clamp(0, i32::from(ELITE))).unwrap_or(0);
            for unit in &reached {
                if let Some(own) = experience(world, unit) {
                    crate::veterancy::promote(world, unit.id, own.rank.saturating_add(ranks));
                }
            }
            (content, 0, None)
        }
        CrateContent::Armour | CrateContent::Firepower | CrateContent::Speed => {
            let get = |bonus: crate::Multipliers| match content {
                CrateContent::Armour => bonus.armour_pm,
                CrateContent::Firepower => bonus.firepower_pm,
                _ => bonus.speed_pm,
            };
            let has = |own: Experience| get(own.bonus) != 1000;
            if experience(world, &collector).is_none_or(has) {
                return money(world);
            }
            let multiplier = u32::try_from(value(content).max(1)).unwrap_or(1000);
            for unit in &reached {
                if let Some(own) = experience(world, unit).filter(|own| !has(*own)) {
                    let mut bonus = own.bonus;
                    match content {
                        CrateContent::Armour => bonus.armour_pm = multiplier,
                        CrateContent::Firepower => bonus.firepower_pm = multiplier,
                        _ => bonus.speed_pm = multiplier,
                    }
                    crate::veterancy::set_bonus(world, unit.id, bonus);
                }
            }
            (content, 0, None)
        }
        CrateContent::HealBase => {
            for unit in units.values().filter(|unit| unit.owner == collector.owner) {
                let whole = definitions
                    .unit_combat(unit.kind)
                    .map_or(0, |terms| terms.hit_points);
                if let Some(mut health) = world.get_mut::<Health>(unit.entity)
                    && health.hit_points > 0
                {
                    health.hit_points = health.hit_points.max(whole);
                }
            }
            let mut buildings = world.query::<(&Owner, &Building, &mut Health)>();
            for (owner, building, mut health) in buildings.iter_mut(world) {
                let whole = definitions
                    .building_combat(building.kind)
                    .map_or(0, |terms| terms.hit_points);
                if owner.0 == collector.owner && health.hit_points > 0 {
                    health.hit_points = health.hit_points.max(whole);
                }
            }
            (content, 0, None)
        }
        CrateContent::Reveal => {
            let (width, height) = {
                let grid = world.resource::<Grid>();
                (grid.width(), grid.height())
            };
            if let Some(player) = world.resource_mut::<Players>().get_mut(collector.owner) {
                player.vision.explore_all(width, height);
            }
            (content, 0, None)
        }
    }
}

/// Whether `owner` has no building and no base unit with hit points left, and more than
/// [`FREE_MCV_CREDITS`] credits: the player the rule `free_mcv` gives a base unit.
fn stranded(
    world: &mut World,
    rules: &Rules,
    units: &BTreeMap<SimId, Soldier>,
    owner: PlayerId,
) -> bool {
    let rich = world
        .resource::<Players>()
        .get(owner)
        .is_some_and(|player| player.credits() > FREE_MCV_CREDITS);
    rich && !units.values().any(|unit| {
        unit.owner == owner
            && (rules.is_base_unit(unit.kind)
                || world
                    .resource::<crate::SpecialState>()
                    .cargo_descendants(unit.id)
                    .iter()
                    .any(|passenger| passenger.hp > 0 && rules.is_base_unit(passenger.kind)))
    }) && !world
        .query::<(&Owner, &Building, &Health)>()
        .iter(world)
        .any(|(of, _, health)| of.0 == owner && health.hit_points > 0)
}
