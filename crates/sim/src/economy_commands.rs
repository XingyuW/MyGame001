//! Validation and application of the economy commands: `QueueProduction`, `CancelProduction`,
//! `PlaceBuilding`, `SellBuilding`, and `SetRallyPoint` (PLAN.md section 5; docs/DECISIONS.md
//! D25).
//!
//! Every check runs before any state changes, so a rejected command changes no state. Each
//! command is checked in this order, and the first failure is the reason:
//!
//! * all: the issuing player is a player of the match (`UnknownPlayer`);
//! * `QueueProduction` ([`check_buildable`], then the queue): the item is defined and producible
//!   and, with an arsenal entry, its tech level lies between 0 and that of the rules
//!   (`UnknownItem`); the roster of the player's faction offers it (`NotInRoster`, M7; a player
//!   without a roster may produce every item); it is not a superweapon building while the match
//!   has superweapons switched off (`DisabledByOptions`, RA1); the player owns a building of
//!   each prerequisite type and a member of each prerequisite group (`MissingPrerequisite`, in
//!   the order the economy file lists them, then the groups of the arsenal, each named by its
//!   first member), a factory of the item's kind (`NoProducer`), and one of them of a side that
//!   builds the item (`NoFactoryOfSide`, RA1); the player's country is among the required ones
//!   if there are any (`CountryRequired`) and is not forbidden (`CountryForbidden`); with a
//!   rules file the player owns and has queued fewer buildings of the type than its
//!   `build_limit`, if it has one (`BuildLimitReached`, RA2); and the queue is not full
//!   (`QueueFull`: [`Definitions::queue_capacity`]). This is the buildability rule of Red Alert 2
//!   (docs/RA2_PARITY_PLAN.md section 3.3, docs/DECISIONS.md R10): the first three tests look
//!   at the player's buildings, so a captured factory opens the other side's items, and the
//!   country test keeps national specials national. For a unit, a Super Me player
//!   ([`PlayerState::super_me`]) skips the roster, stolen technology, side and country tests, and
//!   its buildings also stand in for their counterparts of the other side as prerequisites of a
//!   unit, and of the other side's factory for a kind of unit its side has no factory for
//!   ([`super_me_counterparts`], [`super_me_owned`], docs/DECISIONS.md R213);
//! * `CancelProduction`: the item is defined and producible, and queued (`NotQueued`);
//! * `PlaceBuilding`: the building type is defined, a complete building of that type is at the
//!   head of its queue (`NotReady`), the footprint lies inside the map (`Placement(OutsideMap)`),
//!   under fog of war (M4) the player sees every cell of it (`Placement(NotVisible)`, so that
//!   the checks that follow name nothing the player does not see), the footprint is valid
//!   (`Placement`: buildable, no building, no unit), and it lies within the placement distance
//!   of a building of the player (`Placement(TooFar)`): the economy file's distance, or with a
//!   rules file (RA2) the building's own reach from a base building ([`placement_reach`],
//!   [`extends_base`]);
//!   with Super Me, a Super Me unit with a build radius is an anchor as well, within its own
//!   radius (docs/DECISIONS.md R218);
//! * `PlaceUnit` ([`check_unit_placement`], R218): a complete placed Super Me unit of the type is
//!   at the head of its queue (`UnitNotReady`), and the cell lies inside the map (`OutsideMap`)
//!   and is free for the unit (`Placement`), anywhere on the map;
//! * `SellBuilding`: the building exists (`UnknownBuilding`) and belongs to the player
//!   (`NotOwned`); under fog of war (M4), another player's building the player does not see is
//!   reported as `UnknownBuilding`; with a rules file its type is `sellable` (`NotSellable`,
//!   RA2: walls are not). The refund is [`Definitions::refund`];
//! * `RepairBuilding`: the building exists and belongs to the player (as for `SellBuilding`),
//!   and with a rules file a repair starts only on a `repairable` type (`NotRepairable`, RA2);
//! * `SetRallyPoint`: the building exists and belongs to the player (as for `SellBuilding`),
//!   its type produces units (`NotAProducer`), and the cell is inside the map (`OutsideMap`).
//!
//! A queue is kept per factory group ([`Definitions::queue_key`]): all buildings of one group
//! (the Allied and the Soviet War Factory) share it, and a building without a group has the
//! queue of its own type, as in M3. With a rules file the Construction Yards have two queues,
//! the lanes of buildings and of defences ([`Definitions::queue_of`], RA2).

use std::collections::BTreeMap;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::{Commands, Query, Res, ResMut, SystemParam};

use crate::building::{
    Building, Footprint, PlacementError, Producer, building_bundle, check_building_footprint,
    check_distance, check_visible,
};
use crate::combat::Body;
use crate::combat::combat_components;
use crate::command::{Command, IssuedCommand, PlayerId, RejectReason};
use crate::definitions::{Definitions, UnitKind};
use crate::economy::{PlayerState, Players, QueueItem, QueueKey};
use crate::economy_defs::{BuildingKind, ProductionItem};
use crate::event::SimEvent;
use crate::id::{SimId, SimIdAllocator};
use crate::map::{CellPos, Grid};
use crate::unit::{Owner, unit_bundle};
use crate::visibility::Vision;

/// The system parameters the economy commands need besides the grid and the event buffer.
#[derive(SystemParam)]
pub(crate) struct EconomyParams<'w, 's> {
    commands: Commands<'w, 's>,
    players: ResMut<'w, Players>,
    definitions: Res<'w, Definitions>,
    ids: ResMut<'w, SimIdAllocator>,
    props: Res<'w, crate::Props>,
    match_state: Res<'w, crate::MatchState>,
    wrecks: ResMut<'w, crate::crew::Wrecks>,
    buildings: Query<'w, 's, (Entity, &'static SimId, &'static Owner, &'static Building)>,
    healths: Query<'w, 's, &'static crate::combat::Health>,
}

/// A building as the commands of one tick see it.
#[derive(Clone, Copy)]
struct Indexed {
    entity: Entity,
    owner: PlayerId,
    kind: BuildingKind,
    footprint: Footprint,
}

/// The buildings, kept up to date as the commands of a tick place and sell them (the entity
/// changes themselves are deferred to the end of the stage).
pub(crate) struct BuildingIndex {
    by_id: BTreeMap<SimId, Indexed>,
}

impl BuildingIndex {
    /// The types of the buildings `player` owns, one entry per building.
    fn kinds_of(&self, player: PlayerId) -> Vec<BuildingKind> {
        self.by_id
            .values()
            .filter(|building| building.owner == player)
            .map(|building| building.kind)
            .collect()
    }

    /// The types and footprints of the buildings `player` owns.
    fn anchors_of(&self, player: PlayerId) -> impl Iterator<Item = (BuildingKind, Footprint)> + '_ {
        self.by_id
            .values()
            .filter(move |building| building.owner == player)
            .map(|building| (building.kind, building.footprint))
    }

    /// The owner, type, and footprint of building `id`, if it exists as the commands of this tick
    /// see it.
    pub(crate) fn get(&self, id: SimId) -> Option<(PlayerId, BuildingKind, Footprint)> {
        self.by_id
            .get(&id)
            .map(|building| (building.owner, building.kind, building.footprint))
    }

    /// The player's building `id`. Under fog of war (`fog` is the player's vision), another
    /// player's building that the player does not see is reported as unknown, like one that does
    /// not exist, so that commands reveal nothing about unseen enemies (docs/DECISIONS.md D30).
    fn own(
        &self,
        player: PlayerId,
        id: SimId,
        fog: Option<&Vision>,
    ) -> Result<Indexed, RejectReason> {
        let building = *self
            .by_id
            .get(&id)
            .ok_or(RejectReason::UnknownBuilding(id))?;
        if building.owner != player {
            if fog.is_some_and(|vision| !Body::Building(building.footprint).seen_by(vision)) {
                return Err(RejectReason::UnknownBuilding(id));
            }
            return Err(RejectReason::NotOwned(id));
        }
        Ok(building)
    }
}

impl EconomyParams<'_, '_> {
    pub(crate) fn definitions(&self) -> &Definitions {
        &self.definitions
    }

    pub(crate) fn players(&self) -> &Players {
        &self.players
    }

    pub(crate) fn props(&self) -> &crate::Props {
        &self.props
    }

    /// Whether `entity` has hit points left, or has none to lose (a world without a combat
    /// file). A unit or building that an earlier command of the tick destroyed (a nuclear strike
    /// lands in the `Commands` stage) is removed only in `Cleanup`; the commands after it
    /// neither act on it nor for it (docs/DECISIONS.md R56).
    pub(crate) fn alive(&self, entity: Entity) -> bool {
        !self
            .healths
            .get(entity)
            .is_ok_and(|health| health.hit_points <= 0)
    }

    /// The buildings that stand: one without hit points left is none for a command.
    pub(crate) fn building_index(&self) -> BuildingIndex {
        BuildingIndex {
            by_id: self
                .buildings
                .iter()
                .filter(|(entity, ..)| self.alive(*entity))
                .map(|(entity, id, owner, building)| {
                    (
                        *id,
                        Indexed {
                            entity,
                            owner: owner.0,
                            kind: building.kind,
                            footprint: building.footprint,
                        },
                    )
                })
                .collect(),
        }
    }
}

/// The player's vision while fog of war is active, which it is when the definitions include a
/// combat file (only then do units and buildings have sight); `None` without one (the M2 and M3
/// fixtures), in which commands are validated as in M3.
pub(crate) fn fog<'a>(definitions: &Definitions, state: &'a PlayerState) -> Option<&'a Vision> {
    definitions.has_combat().then_some(state.vision())
}

/// Whether a building of type `building` produces `item`: it is a factory of the kind of the
/// item's producer ([`Definitions::same_factory`]), and its side is one of the item's
/// `owner_sides` if the arsenal lists any (docs/RA2_PARITY_PLAN.md section 3.3, tests 2 and 3;
/// assumption RA-A3: a factory's side is that of its building type).
pub fn produces(definitions: &Definitions, building: BuildingKind, item: ProductionItem) -> bool {
    let Some(terms) = definitions.production(item) else {
        return false;
    };
    if !definitions.same_factory(building, terms.producer) {
        return false;
    }
    match definitions.metadata(item) {
        Some(entry) if !entry.owner_sides.is_empty() => definitions
            .metadata(ProductionItem::Building(building))
            .and_then(|factory| factory.side)
            .is_some_and(|side| entry.owner_sides.contains(&side)),
        _ => true,
    }
}

/// Whether a building of type `building` produces `item` for a player with or without Super Me:
/// [`produces`], and for a Super Me player every unit a factory of its kind produces, whatever
/// the side of either (docs/DECISIONS.md R213).
pub fn produces_for(
    definitions: &Definitions,
    super_me: bool,
    building: BuildingKind,
    item: ProductionItem,
) -> bool {
    produces(definitions, building, item)
        || super_me
            && matches!(item, ProductionItem::Unit(_))
            && definitions
                .production(item)
                .is_some_and(|terms| definitions.same_factory(building, terms.producer))
}

/// Whether buildings of the types `a` and `b` do the same job for the two sides, so that a Super
/// Me player's building of one type stands in for the other as a unit's prerequisite: they are
/// factories of one group (the barracks, the war factories, the shipyards), members of one
/// prerequisite group of the rules (radar, battle lab, refinery, power), or buildings of the
/// same role other than plain combat (the service depots).
fn counterparts(definitions: &Definitions, a: BuildingKind, b: BuildingKind) -> bool {
    if a == b || definitions.same_factory(a, b) {
        return true;
    }
    let grouped = definitions.rules().is_some_and(|rules| {
        rules
            .prerequisite_groups
            .iter()
            .any(|group| group.buildings.contains(&a) && group.buildings.contains(&b))
    });
    let (role_a, role_b) = (
        definitions.metadata(ProductionItem::Building(a)),
        definitions.metadata(ProductionItem::Building(b)),
    );
    grouped
        || role_a.zip(role_b).is_some_and(|(x, y)| {
            x.role == y.role && x.role != crate::arsenal::Role::Combat && x.side != y.side
        })
}

/// The building types that buildings of the types `owned` count as when a Super Me player's unit
/// is checked for its prerequisites: the owned types and every counterpart of them
/// ([`counterparts`]). An Allied War Factory and Battle Lab so open the Apocalypse Tank, whose
/// prerequisites are the Soviet ones (docs/DECISIONS.md R213).
pub fn super_me_owned(definitions: &Definitions, owned: &[BuildingKind]) -> Vec<BuildingKind> {
    (0..definitions.buildings().len())
        .filter_map(|index| u16::try_from(index).ok())
        .map(BuildingKind)
        .filter(|&kind| {
            owned
                .iter()
                .any(|&have| counterparts(definitions, have, kind))
        })
        .collect()
}

/// Whether `item` is checked against [`super_me_owned`] for a player of `country` with or without
/// Super Me: with Super Me every unit, and the other side's factory of a kind of unit for which
/// the player's side has no factory (the Airforce Command for a Soviet player, which alone
/// builds and docks the Allied jets). Other buildings keep the prerequisites of the player's
/// side (docs/DECISIONS.md R213).
pub fn super_me_counterparts(
    definitions: &Definitions,
    super_me: bool,
    country: Option<crate::rules::CountryId>,
    item: ProductionItem,
) -> bool {
    if !super_me {
        return false;
    }
    let kind = match item {
        ProductionItem::Unit(_) => return true,
        ProductionItem::Building(kind) => kind,
    };
    let (Some(group), Some(side)) = (
        definitions.factory_group(kind),
        definitions
            .rules()
            .zip(country)
            .and_then(|(rules, country)| rules.country(country))
            .map(|country| country.side),
    ) else {
        return false;
    };
    !(0..definitions.buildings().len())
        .filter_map(|index| u16::try_from(index).ok())
        .map(BuildingKind)
        .any(|other| {
            definitions.factory_group(other) == Some(group)
                && definitions
                    .metadata(ProductionItem::Building(other))
                    .is_some_and(|entry| entry.side == Some(side))
        })
}

/// [`missing_prerequisite`] for a player of `country` with or without Super Me: an item of
/// [`super_me_counterparts`] is checked against [`super_me_owned`].
pub fn missing_prerequisite_for(
    definitions: &Definitions,
    super_me: bool,
    country: Option<crate::rules::CountryId>,
    owned: &[BuildingKind],
    item: ProductionItem,
) -> Option<BuildingKind> {
    if super_me_counterparts(definitions, super_me, country, item) {
        missing_prerequisite(definitions, &super_me_owned(definitions, owned), item)
    } else {
        missing_prerequisite(definitions, owned, item)
    }
}

/// The first prerequisite of `item` that a player owning buildings of the types `owned` lacks:
/// a building type of the economy file's list, or the first member of a prerequisite group of
/// the arsenal of which the player owns no member. Without a rules file the basic power plants
/// stand in for each other, as since M9 ([`Definitions::prerequisite_met`]); with one, power is
/// a prerequisite group like any other.
pub fn missing_prerequisite(
    definitions: &Definitions,
    owned: &[BuildingKind],
    item: ProductionItem,
) -> Option<BuildingKind> {
    let terms = definitions.production(item)?;
    let rules = definitions.rules();
    let met = |required: BuildingKind| match rules {
        Some(_) => owned.contains(&required),
        None => definitions.prerequisite_met(required, owned.iter().copied()),
    };
    if let Some(&missing) = terms.prerequisites.iter().find(|&&kind| !met(kind)) {
        return Some(missing);
    }
    let (entry, rules) = (definitions.metadata(item)?, rules?);
    entry
        .prerequisite_groups
        .iter()
        .filter_map(|&group| rules.group(group))
        .find(|group| !group.buildings.iter().any(|kind| owned.contains(kind)))
        .and_then(|group| group.buildings.first().copied())
}

/// The checks of `QueueProduction` except the length of the queue, in the order of the module
/// documentation: whether the player with `state`, who owns buildings of the types `owned`, may
/// queue `item` in a match with superweapons switched on or off.
///
/// The command's validation calls exactly this function, so the interface and the computer
/// player can know the outcome of a `QueueProduction` before it is issued.
pub fn check_buildable(
    definitions: &Definitions,
    state: &PlayerState,
    superweapons: bool,
    owned: &[BuildingKind],
    item: ProductionItem,
) -> Result<(), RejectReason> {
    let terms = definitions
        .production(item)
        .ok_or(RejectReason::UnknownItem(item))?;
    let entry = definitions.metadata(item);
    // Super Me opens every unit of both sides and all countries (docs/DECISIONS.md R213).
    let any_unit = state.super_me() && matches!(item, ProductionItem::Unit(_));
    if let Some(entry) = entry {
        let highest = definitions
            .rules()
            .map_or(crate::rules::MAX_TECH_LEVEL, |rules| {
                rules.dialog.tech_level
            });
        if !(0..=highest).contains(&entry.tech_level) {
            return Err(RejectReason::UnknownItem(item));
        }
    }
    if !any_unit
        && entry.is_some_and(|e| {
            (e.stolen_allied && state.infiltration & 4 == 0)
                || (e.stolen_soviet && state.infiltration & 8 == 0)
        })
    {
        return Err(RejectReason::AbilityUnavailable(
            "Infiltrate the required Battle Lab first",
        ));
    }
    // The game's own Super Me units are for a Super Me player alone (docs/DECISIONS.md R218).
    if !state.offers(item) || (!state.super_me() && definitions.is_super_me_item(item)) {
        return Err(RejectReason::NotInRoster(item));
    }
    if !superweapons
        && matches!(item, ProductionItem::Building(_))
        && entry.is_some_and(|entry| entry.role.major_superweapon())
    {
        return Err(RejectReason::DisabledByOptions);
    }
    if !state.unlock_all()
        && let Some(prerequisite) =
            missing_prerequisite_for(definitions, state.super_me(), state.country(), owned, item)
    {
        return Err(RejectReason::MissingPrerequisite { item, prerequisite });
    }
    if !owned
        .iter()
        .any(|&kind| definitions.same_factory(kind, terms.producer))
    {
        return Err(RejectReason::NoProducer {
            item,
            producer: terms.producer,
        });
    }
    if !owned
        .iter()
        .any(|&kind| produces_for(definitions, state.super_me(), kind, item))
    {
        return Err(RejectReason::NoFactoryOfSide(item));
    }
    if let Some(entry) = entry {
        let country = state.country();
        if !any_unit
            && !entry.required_countries.is_empty()
            && !country.is_some_and(|country| entry.required_countries.contains(&country))
        {
            return Err(RejectReason::CountryRequired(item));
        }
        if !any_unit && country.is_some_and(|country| entry.forbidden_countries.contains(&country))
        {
            return Err(RejectReason::CountryForbidden(item));
        }
        // RA4: an aircraft that needs a dock can be queued only while the docks of the
        // player's buildings outnumber such aircraft it owns and has queued. The docks are
        // those of the buildings in `owned`, or as many as the player had at the last
        // `Economy` stage if that is more: a caller that lists every type once (the
        // sidebar) still counts the docks of two buildings of a type.
        if crate::flight::needs_dock(definitions, item) {
            let taken = crate::flight::dock_commitments(definitions, state);
            let docks =
                crate::flight::docks_of(definitions, owned.iter().copied()).max(state.docks());
            if taken >= docks as usize {
                return Err(RejectReason::NoFreeDock(item));
            }
        }
        // RA2, the ini's `BuildLimit`: the buildings of the type the player owns and those in
        // its queues together.
        if definitions.rules().is_some() && entry.build_limit > 0 {
            let have = match item {
                ProductionItem::Building(kind) => {
                    owned.iter().filter(|&&other| other == kind).count()
                }
                ProductionItem::Unit(kind) => {
                    state.limited_units.get(&kind).copied().unwrap_or(0) as usize
                }
            } + state
                .queues()
                .flat_map(|(_, items)| items.iter())
                .filter(|queued| queued.item == item)
                .count();
            if have >= entry.build_limit as usize {
                return Err(RejectReason::BuildLimitReached {
                    item,
                    limit: entry.build_limit,
                });
            }
        }
    }
    Ok(())
}

/// How far a new building of type `building` may lie from a building of its owner, as
/// [`Footprint::distance`] measures it (1: touching). Without a rules file the economy file's
/// `placement_distance`, and for a shipyard at least [`SHIPYARD_PLACEMENT_DISTANCE`]. With one
/// (RA2, docs/RA2_PARITY_PLAN.md section 4.2) the building's own `adjacent` free cells, so one
/// more; a building without an arsenal entry keeps the economy file's distance.
pub fn placement_reach(definitions: &Definitions, building: BuildingKind) -> u32 {
    let entry = definitions.metadata(ProductionItem::Building(building));
    match (definitions.rules(), entry) {
        (Some(_), Some(entry)) => entry.adjacent.saturating_add(1),
        (None, Some(entry)) if entry.role == crate::Role::Shipyard => definitions
            .placement_distance()
            .max(SHIPYARD_PLACEMENT_DISTANCE),
        _ => definitions.placement_distance(),
    }
}

/// Whether a new building may be placed beside a building of type `kind` of its owner: always
/// without a rules file; with one (RA2) unless the arsenal says the type is no base building
/// (`base_normal`, the ini's `BaseNormal=no` of defences, walls and shipyards).
pub fn extends_base(definitions: &Definitions, kind: BuildingKind) -> bool {
    definitions.rules().is_none()
        || definitions
            .metadata(ProductionItem::Building(kind))
            .is_none_or(|entry| entry.base_normal)
}

/// The checks of `PlaceBuilding` (docs/DECISIONS.md D24, D33), in the order of the module
/// documentation: the building type is defined (`UnknownItem`), a complete building of that type
/// is at the head of its queue in `state` (`NotReady`), the footprint lies inside the map, under
/// fog of war `state`'s player sees every cell of it, the footprint is valid, and it lies within
/// its reach ([`placement_reach`]) of one of `owned` (the types and footprints of the player's
/// buildings) beside which a building may be placed ([`extends_base`]). Returns the footprint
/// the building would cover.
///
/// The command's validation calls exactly this function, so the interface's placement preview
/// (M5) can show the outcome of a `PlaceBuilding` before it is issued. It reads only what the
/// command's own rejection would reveal: under fog of war an unseen cell is reported as
/// `NotVisible` before anything in the footprint is looked at.
pub fn check_placement(
    grid: &Grid,
    definitions: &Definitions,
    state: &PlayerState,
    owned: impl Iterator<Item = (BuildingKind, Footprint)>,
    building: BuildingKind,
    origin: CellPos,
) -> Result<Footprint, RejectReason> {
    let item = ProductionItem::Building(building);
    let def = definitions
        .building(building)
        .ok_or(RejectReason::UnknownItem(item))?;
    let ready = definitions
        .queue_of(item)
        .and_then(|key| state.lane(key).first())
        .is_some_and(|head| head.item == item && head.progress >= def.production.build_time);
    if !ready {
        return Err(RejectReason::NotReady(building));
    }
    check_placement_site(grid, definitions, state, owned, building, origin)
}

/// The shared site rules without requiring a completed queue item. AI navy planning asks
/// whether a legal coastal site exists before committing its production lane to a shipyard.
pub(crate) fn check_placement_site(
    grid: &Grid,
    definitions: &Definitions,
    state: &PlayerState,
    owned: impl Iterator<Item = (BuildingKind, Footprint)>,
    building: BuildingKind,
    origin: CellPos,
) -> Result<Footprint, RejectReason> {
    let item = ProductionItem::Building(building);
    let def = definitions
        .building(building)
        .ok_or(RejectReason::UnknownItem(item))?;
    let footprint = Footprint {
        origin,
        width: def.width,
        height: def.height,
    };
    if let Some(vision) = fog(definitions, state) {
        check_visible(grid, footprint, vision).map_err(RejectReason::Placement)?;
    }
    let shipyard = definitions
        .metadata(item)
        .is_some_and(|e| e.role == crate::Role::Shipyard);
    check_building_footprint(grid, footprint, shipyard).map_err(RejectReason::Placement)?;
    let anchors = owned
        .filter(|&(kind, _)| extends_base(definitions, kind))
        .map(|(_, footprint)| footprint);
    let base = check_distance(footprint, anchors, placement_reach(definitions, building));
    // A Super Me unit with a build radius is an anchor too, with its own reach (R218).
    let near_super_me_unit = state.build_anchors().iter().any(|&(cell, radius)| {
        footprint.distance(&Footprint {
            origin: cell,
            width: 1,
            height: 1,
        }) <= radius
    });
    if !near_super_me_unit {
        base.map_err(RejectReason::Placement)?;
    }
    Ok(footprint)
}

/// The checks of `PlaceUnit` (docs/DECISIONS.md R218): a complete unit of type `unit` that is
/// placed on completion ([`Definitions::placed_on_completion`]) is at the head of its queue in
/// `state` (`UnitNotReady`), `cell` lies inside the map (`OutsideMap`), the unit could stand on
/// it (`Placement(NotBuildable)`), no building covers it (`Placement(Overlap)`) and no unit holds
/// it (`Placement(Occupied)`). Unlike a building, the cell may be anywhere on the map, seen or
/// not.
pub fn check_unit_placement(
    grid: &Grid,
    definitions: &Definitions,
    state: &PlayerState,
    unit: UnitKind,
    cell: CellPos,
) -> Result<(), RejectReason> {
    let item = ProductionItem::Unit(unit);
    let ready = definitions.placed_on_completion(item)
        && definitions
            .production(item)
            .zip(definitions.queue_of(item))
            .and_then(|(terms, key)| {
                state
                    .lane(key)
                    .first()
                    .map(|head| head.item == item && head.progress >= terms.build_time)
            })
            .unwrap_or(false);
    if !ready {
        return Err(RejectReason::UnitNotReady(unit));
    }
    if !grid.contains(cell) {
        return Err(RejectReason::OutsideMap(cell));
    }
    let domain = definitions.domain(unit);
    if !grid.passable_for(cell, domain) {
        if let Some(building) = grid.building(cell) {
            return Err(RejectReason::Placement(PlacementError::Overlap {
                cell,
                building,
            }));
        }
        return Err(RejectReason::Placement(PlacementError::NotBuildable(cell)));
    }
    if let Some(unit) = grid.occupant_for(cell, domain) {
        return Err(RejectReason::Placement(PlacementError::Occupied {
            cell,
            unit,
        }));
    }
    Ok(())
}

/// How far (cells, Chebyshev distance between nearest cells) a naval shipyard may lie from one of
/// its owner's buildings in a world without a rules file. (With one every building has its own
/// `adjacent` distance, [`placement_reach`]; the shipped shipyards keep this reach there by
/// their data, an intentional difference from Red Alert 2, docs/DECISIONS.md R25.) Ordinary
/// structures use the economy file's `placement_distance`, but a
/// shipyard needs a 3x3 open-water site touching shore, and the shipped maps' lakes and rivers lie
/// 18-28 cells from the human player's start; with the ordinary reach naval production was only
/// reachable by chaining buildings across the map. The site must still be visible (scouted) and
/// valid water.
pub const SHIPYARD_PLACEMENT_DISTANCE: u32 = 32;

/// Validates and applies an economy command. Unit commands are handled by `command.rs`.
pub(crate) fn apply(
    issued: &IssuedCommand,
    params: &mut EconomyParams,
    index: &mut BuildingIndex,
    grid: &mut Grid,
    events: &mut Messages<SimEvent>,
    special: &crate::SpecialState,
) -> Result<(), RejectReason> {
    let player = issued.player;
    let definitions = &*params.definitions;
    let state = params
        .players
        .get_mut(player)
        .ok_or(RejectReason::UnknownPlayer(player))?;
    match &issued.command {
        &Command::QueueProduction { item } => {
            check_buildable(
                definitions,
                state,
                params.match_state.rules().superweapons,
                &index.kinds_of(player),
                item,
            )?;
            // RA4: `check_buildable` counts the docks of `owned` or, if that is more, those the
            // player had at the last `Economy` stage, for callers that list every type once.
            // The command knows every building that stands now, and counts those alone: a
            // dock sold or lost earlier in this tick takes no aircraft.
            if crate::flight::needs_dock(definitions, item) {
                let taken = crate::flight::dock_commitments(definitions, state);
                let docks =
                    crate::flight::docks_of(definitions, index.kinds_of(player).into_iter());
                if taken >= docks as usize {
                    return Err(RejectReason::NoFreeDock(item));
                }
            }
            // `check_buildable` found the production terms.
            let Some(terms) = definitions.production(item) else {
                return Err(RejectReason::UnknownItem(item));
            };
            let key = QueueKey {
                producer: definitions.queue_key(terms.producer),
                lane: definitions.lane(item),
            };
            let max = definitions.queue_capacity(item);
            if state.lane(key).len() >= max {
                return Err(RejectReason::QueueFull {
                    producer: terms.producer,
                    max,
                });
            }
            state
                .queues
                .entry(key)
                .or_default()
                .push(QueueItem::new(item));
        }
        &Command::CancelProduction { item } => {
            let key = definitions
                .queue_of(item)
                .ok_or(RejectReason::UnknownItem(item))?;
            let queue = state
                .queues
                .get_mut(&key)
                .ok_or(RejectReason::NotQueued(item))?;
            let position = queue
                .iter()
                .rposition(|queued| queued.item == item)
                .ok_or(RejectReason::NotQueued(item))?;
            let removed = queue.remove(position);
            if queue.is_empty() {
                state.queues.remove(&key);
            }
            state.credits = state.credits.saturating_add(removed.charged);
            events.write(SimEvent::ProductionCancelled {
                player,
                item,
                refund: removed.charged,
            });
        }
        &Command::PlaceBuilding { building, origin } => {
            let footprint = check_placement(
                grid,
                definitions,
                state,
                index.anchors_of(player),
                building,
                origin,
            )?;
            // `check_placement` found the definition.
            let Some(def) = definitions.building(building) else {
                return Err(RejectReason::UnknownItem(ProductionItem::Building(
                    building,
                )));
            };
            let producer = QueueKey {
                producer: definitions.queue_key(def.production.producer),
                lane: definitions.lane(ProductionItem::Building(building)),
            };
            let id = params.ids.allocate().ok_or(RejectReason::IdsExhausted)?;

            if let Some(queue) = state.queues.get_mut(&producer) {
                queue.remove(0);
                if queue.is_empty() {
                    state.queues.remove(&producer);
                }
            }
            for cell in footprint.cells() {
                grid.cover(cell, id);
            }
            let (id, owner, component, rally) = building_bundle(
                definitions,
                id,
                player,
                building,
                footprint,
                def.free_units.clone(),
            );
            let mut entity = params.commands.spawn((id, owner, component));
            if let Some(rally) = rally {
                entity.insert(rally);
            }
            if let Some(terms) = definitions.building_combat(building) {
                let (components, weapon) = combat_components(terms);
                entity.insert(components);
                if let Some(weapon) = weapon {
                    entity.insert(weapon);
                }
            }
            let entity = entity.id();
            index.by_id.insert(
                id,
                Indexed {
                    entity,
                    owner: player,
                    kind: building,
                    footprint,
                },
            );
            events.write(SimEvent::BuildingPlaced {
                player,
                building: id,
                kind: building,
            });
        }
        &Command::PlaceUnit { unit, cell } => {
            check_unit_placement(grid, definitions, state, unit, cell)?;
            let item = ProductionItem::Unit(unit);
            // `check_unit_placement` found the queue and its head.
            let Some(key) = definitions.queue_of(item) else {
                return Err(RejectReason::UnitNotReady(unit));
            };
            let speed = definitions.unit(unit).map_or(1, |def| def.speed);
            let domain = definitions.domain(unit);
            let on_foot = definitions.on_foot(unit);
            let id = params.ids.allocate().ok_or(RejectReason::IdsExhausted)?;
            grid.register(id, domain, on_foot);
            if !grid.claim(cell, id) {
                grid.unregister(id);
                return Err(RejectReason::Placement(PlacementError::NotBuildable(cell)));
            }
            if let Some(queue) = state.queues.get_mut(&key) {
                queue.remove(0);
                if queue.is_empty() {
                    state.queues.remove(&key);
                }
            }
            let position = grid.standing_point(cell, id);
            let mut entity = params
                .commands
                .spawn(unit_bundle(id, player, unit, speed, cell, position));
            if let Some(terms) = definitions.unit_combat(unit) {
                let (components, weapon) = combat_components(terms);
                entity.insert(components);
                if let Some(weapon) = weapon {
                    entity.insert(weapon);
                }
            }
            events.write(SimEvent::UnitPlaced {
                player,
                unit: id,
                kind: unit,
            });
        }
        &Command::SellBuilding { building } => {
            let target = index.own(player, building, fog(definitions, state))?;
            if special.erasures.contains_key(&building) {
                return Err(RejectReason::AbilityUnavailable("Target is being erased"));
            }
            let entry = definitions.metadata(ProductionItem::Building(target.kind));
            if definitions.rules().is_some() && entry.is_some_and(|entry| !entry.sellable) {
                return Err(RejectReason::NotSellable(building));
            }
            let refund = definitions.refund(target.kind);
            for cell in target.footprint.cells() {
                grid.uncover(cell, building);
            }
            params.commands.entity(target.entity).despawn();
            index.by_id.remove(&building);
            state.repairing.remove(&building);
            state.credits = state.credits.saturating_add(refund);
            // RA4: the survivors of a sold building leave it at the end of the tick.
            params.wrecks.0.push(crate::crew::Wreck {
                owner: player,
                id: building,
                what: crate::crew::Wrecked::Building {
                    kind: target.kind,
                    footprint: target.footprint,
                },
            });
            events.write(SimEvent::BuildingSold {
                player,
                building,
                refund,
            });
        }
        &Command::RepairBuilding { building } => {
            let target = index.own(player, building, fog(definitions, state))?;
            if !state.repairing.remove(&building) {
                let entry = definitions.metadata(ProductionItem::Building(target.kind));
                if definitions.rules().is_some() && entry.is_some_and(|entry| !entry.repairable) {
                    return Err(RejectReason::NotRepairable(building));
                }
                state.repairing.insert(building);
            }
        }
        &Command::SetRallyPoint { building, cell } => {
            let target = index.own(player, building, fog(definitions, state))?;
            if !definitions.produces_units(target.kind) {
                return Err(RejectReason::NotAProducer(building));
            }
            if !grid.contains(cell) {
                return Err(RejectReason::OutsideMap(cell));
            }
            params
                .commands
                .entity(target.entity)
                .insert(Producer { rally: Some(cell) });
        }
        Command::Deploy { .. }
        | Command::Undeploy { .. }
        | Command::Interact { .. }
        | Command::Chronoshift { .. }
        | Command::Superweapon { .. }
        | Command::Guard { .. }
        | Command::Scatter { .. }
        | Command::Follow { .. }
        | Command::ForceFire { .. }
        | Command::ForceMove { .. }
        | Command::QueueMove { .. }
        | Command::Move { .. }
        | Command::Stop { .. }
        | Command::Harvest { .. }
        | Command::Attack { .. }
        | Command::AttackMove { .. } => {}
    }
    Ok(())
}

#[cfg(test)]
mod ra8_site_tests {
    use super::*;

    #[test]
    fn ra8_legal_site_can_be_inspected_without_consuming_or_bypassing_a_ready_item() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data");
        let definitions = crate::load_ruleset(path).expect("shipped rules");
        let grid = Grid::from_cells(
            32,
            32,
            vec![
                crate::Cell {
                    passable: true,
                    water: false,
                    buildable: true,
                    ore: 0,
                };
                32 * 32
            ],
        )
        .expect("grid");
        let mut state = PlayerState::default();
        state
            .vision
            .reveal_for_test(&grid, crate::CellPos::new(10, 10), 20);
        let yard = definitions
            .building_kind("construction_yard")
            .expect("yard");
        let power = definitions.building_kind("power_plant").expect("power");
        let def = definitions.building(yard).expect("yard definition");
        let anchor = (
            yard,
            Footprint {
                origin: crate::CellPos::new(10, 10),
                width: def.width,
                height: def.height,
            },
        );
        let origin = crate::CellPos::new(7, 10);
        let before = state.clone();
        assert!(
            check_placement_site(
                &grid,
                &definitions,
                &state,
                [anchor].into_iter(),
                power,
                origin
            )
            .is_ok()
        );
        assert_eq!(
            check_placement(
                &grid,
                &definitions,
                &state,
                [anchor].into_iter(),
                power,
                origin
            ),
            Err(RejectReason::NotReady(power))
        );
        assert_eq!(state, before);
        let item = ProductionItem::Building(power);
        let mut ready = crate::QueueItem::new(item);
        ready.progress = definitions
            .building(power)
            .expect("power definition")
            .production
            .build_time;
        state
            .queues
            .insert(definitions.queue_of(item).expect("lane"), vec![ready]);
        assert!(
            check_placement(
                &grid,
                &definitions,
                &state,
                [anchor].into_iter(),
                power,
                origin
            )
            .is_ok()
        );
    }
}
