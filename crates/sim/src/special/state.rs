//! Specialist state mechanics, moved unchanged from special.rs.
use super::*;

#[derive(Clone, Debug)]
pub struct Passenger {
    pub id: SimId,
    pub kind: UnitKind,
    pub hp: i32,
    pub harvester: Option<crate::Harvester>,
    /// The passenger's experience (RA4): it keeps its rank aboard.
    pub experience: Option<crate::Experience>,
    /// The bay of a passenger that spawns units (RA4), as it went aboard: what it had launched
    /// counts as lost, and the timers of its places stand still until it is unloaded.
    pub bay: Option<crate::flight::Bay>,
}
#[derive(Clone, Debug)]
pub struct Zone {
    pub role: Role,
    pub cell: CellPos,
    pub radius: u32,
    pub damage: i32,
    pub remaining: u32,
}
#[derive(Resource, Clone, Debug, Default)]
pub struct SpecialState {
    pub strategic: super::superweapons::StrategicState,
    pub parasites: BTreeMap<SimId, super::vehicles::Parasite>,
    pub deployed: BTreeMap<SimId, u64>,
    pub mirage_trees: BTreeMap<SimId, u8>,
    pub disguises: BTreeMap<SimId, UnitKind>,
    pub erasures: BTreeMap<SimId, super::infantry::Erasure>,
    pub charges: BTreeMap<SimId, super::infantry::Charge>,
    pub radiation_sites: Vec<super::radiation::Site>,
    pub coil_charge: BTreeMap<SimId, u32>,
    pub death_detonated: BTreeSet<SimId>,
    pub eruption_next: BTreeMap<SimId, u64>,
    pub temporal_removed: BTreeSet<SimId>,
    pub charge: BTreeMap<SimId, u32>,
    pub protected: BTreeMap<SimId, u64>,
    pub disabled: BTreeMap<SimId, u64>,
    pub cargo: BTreeMap<SimId, Vec<Passenger>>,
    pub garrisons: BTreeMap<SimId, crate::Garrison>,
    pub controlled: BTreeMap<SimId, (SimId, PlayerId)>,
    pub bombs: BTreeMap<SimId, (u64, i32, u32)>,
    pub zones: Vec<Zone>,
    pub rearming: BTreeMap<SimId, u32>,
    pub carrier_flights: BTreeMap<SimId, SimId>,
    pub interactions: BTreeMap<SimId, SimId>,
    /// DEFUSE-only jobs: actor -> (host, actor owner at selection).
    pub auto_defuses: BTreeMap<SimId, (SimId, PlayerId)>,
    /// Accepted replacements suppress re-selection for the current tick only.
    pub auto_defuse_cancelled: BTreeMap<SimId, u64>,
    /// Separate from strategic timer announcements, including generic fixture buildings.
    pub revealed_owners: BTreeMap<SimId, PlayerId>,
    pub hidden: BTreeSet<(PlayerId, SimId)>,
    pub outages: BTreeMap<PlayerId, u64>,
    /// Vehicles docked at a building that repairs units, by vehicle (RA2: `unit_repair`, the
    /// Service Depot). A docked vehicle stands beside the building and is repaired in steps
    /// for credits until it is whole, is ordered elsewhere, or the building is lost.
    pub docked: BTreeMap<SimId, SimId>,
    /// RA4 (`crate::flight`): the volleys an aircraft with ammunition has left, for every one
    /// that has fewer than its type's `ammo` (0: it must reload before it fires again).
    pub ammo: BTreeMap<SimId, u32>,
    /// The ticks an aircraft has stood on its dock toward its next ammunition or paid health step.
    pub reloading: BTreeMap<SimId, u32>,
    /// The dock of an airport bound aircraft: the building, and which of its docks.
    pub docks: BTreeMap<SimId, (SimId, u8)>,
    /// The units a spawner carries and what it launched them at, by spawner.
    pub bays: BTreeMap<SimId, crate::flight::Bay>,
    /// The unit that launched a missile or aircraft, by the launched unit.
    pub spawned: BTreeMap<SimId, SimId>,
    /// The flight of a launched missile.
    pub missiles: BTreeMap<SimId, crate::flight::MissileFlight>,
    /// The tick until which a cloakable unit that fired is seen like any other unit.
    pub surfaced: BTreeMap<SimId, u64>,
}
impl SpecialState {
    /// Every nested passenger below `root`, once, in stable id order. Root ownership applies
    /// to its off-map descendants. The visited set also bounds malformed cyclic state.
    pub(crate) fn cargo_descendants(&self, root: SimId) -> Vec<&Passenger> {
        let mut visited = BTreeSet::from([root]);
        let mut pending = BTreeSet::from([root]);
        let mut passengers = Vec::new();
        while let Some(host) = pending.pop_first() {
            if let Some(children) = self.cargo.get(&host) {
                for passenger in children {
                    if visited.insert(passenger.id) {
                        pending.insert(passenger.id);
                        passengers.push(passenger);
                    }
                }
            }
        }
        passengers.sort_unstable_by_key(|passenger| passenger.id);
        passengers
    }
    /// The visible infantry appearance of an undetected spy, without exposing its true kind.
    pub fn disguise_for(&self, viewer: PlayerId, id: SimId) -> Option<UnitKind> {
        self.hidden
            .contains(&(viewer, id))
            .then(|| self.disguises.get(&id).copied())
            .flatten()
    }

    pub fn invulnerable(&self, id: SimId, tick: u64) -> bool {
        self.erasures.contains_key(&id)
            || self.protected.get(&id).is_some_and(|&until| until > tick)
    }
    /// Whether the spawner `id`, which carries `number` units, has one ready to launch (RA4).
    pub fn launch_ready(&self, id: SimId, number: u32) -> bool {
        match self.bays.get(&id) {
            None => number > 0,
            Some(bay) => bay.ready(),
        }
    }
    /// The volleys the aircraft `id` of a type with `ammo` volleys has left (RA4).
    pub fn ammo_of(&self, id: SimId, ammo: u32) -> u32 {
        self.ammo.get(&id).copied().unwrap_or(ammo).min(ammo)
    }
    pub(crate) fn hash_into(&self, h: &mut Fnv1a64) {
        super::infantry::hash(self, h);
        self.strategic.hash(h);
        h.write_u64(self.mirage_trees.len() as u64);
        for (id, v) in &self.mirage_trees {
            h.write_u32(id.0);
            h.write(&[*v]);
        }
        h.write_u64(self.parasites.len() as u64);
        for (host, p) in &self.parasites {
            h.write_u32(host.0);
            h.write_u32(p.passenger.id.0);
            h.write(&[u8::from(p.escape.is_some())]);
            if let Some(c) = p.escape {
                h.write(&c.x.to_le_bytes());
                h.write(&c.y.to_le_bytes());
            }
            h.write(&p.passenger.kind.0.to_le_bytes());
            h.write(&p.passenger.hp.to_le_bytes());
            h.write(&[p.owner.0, u8::from(p.squid)]);
            h.write(&p.weapon.0.to_le_bytes());
            h.write_u64(p.due);
            h.write(&[u8::from(p.passenger.experience.is_some())]);
            if let Some(xp) = p.passenger.experience {
                xp.hash_into(h);
            }
        }

        // Stable ordered data, including off-map passengers and pending effects.
        for map in [&self.protected, &self.disabled] {
            h.write_u64(map.len() as u64);
            for (id, t) in map {
                h.write_u32(id.0);
                h.write_u64(*t);
            }
        }
        for map in [&self.charge, &self.rearming] {
            h.write_u64(map.len() as u64);
            for (id, t) in map {
                h.write_u32(id.0);
                h.write_u32(*t);
            }
        }
        h.write_u64(self.carrier_flights.len() as u64);
        for (fighter, carrier) in &self.carrier_flights {
            h.write_u32(fighter.0);
            h.write_u32(carrier.0);
        }
        if !self.garrisons.is_empty() {
            h.write(b"garrisons");
            h.write_u64(self.garrisons.len() as u64);
            for (id, g) in &self.garrisons {
                h.write_u32(id.0);
                h.write(&[g.owner.0]);
                h.write(&g.cell.x.to_le_bytes());
                h.write(&g.cell.y.to_le_bytes());
                h.write_u32(g.cooldown);
                h.write(&[u8::from(g.evicting)]);
            }
        }
        h.write_u64(self.cargo.len() as u64);
        for (id, ps) in &self.cargo {
            h.write_u32(id.0);
            h.write_u64(ps.len() as u64);
            for p in ps {
                h.write_u32(p.id.0);
                h.write(&p.kind.0.to_le_bytes());
                h.write(&p.hp.to_le_bytes());
                if let Some(worker) = &p.harvester {
                    h.write(&[1]);
                    worker.hash_into(h);
                } else {
                    h.write(&[0]);
                }
                if let Some(experience) = &p.experience {
                    h.write(b"xp");
                    experience.hash_into(h);
                }
                if let Some(bay) = &p.bay {
                    h.write(b"by");
                    bay.hash_into(h);
                }
            }
        }
        h.write_u64(self.controlled.len() as u64);
        for (id, (c, o)) in &self.controlled {
            h.write_u32(id.0);
            h.write_u32(c.0);
            h.write(&[o.0]);
        }
        h.write_u64(self.bombs.len() as u64);
        for (id, (t, d, r)) in &self.bombs {
            h.write_u32(id.0);
            h.write_u64(*t);
            h.write(&d.to_le_bytes());
            h.write_u32(*r);
        }
        h.write_u64(self.zones.len() as u64);
        for z in &self.zones {
            h.write(&[z.role as u8]);
            h.write(&z.cell.x.to_le_bytes());
            h.write(&z.cell.y.to_le_bytes());
            h.write_u32(z.radius);
            h.write(&z.damage.to_le_bytes());
            h.write_u32(z.remaining);
        }
        h.write_u64(self.interactions.len() as u64);
        for (id, t) in &self.interactions {
            h.write_u32(id.0);
            h.write_u32(t.0);
        }
        if !self.auto_defuses.is_empty() || !self.auto_defuse_cancelled.is_empty() {
            h.write(b"auto-defuse");
            h.write_u64(self.auto_defuses.len() as u64);
            for (actor, (host, owner)) in &self.auto_defuses {
                h.write_u32(actor.0);
                h.write_u32(host.0);
                h.write(&[owner.0]);
            }
            h.write_u64(self.auto_defuse_cancelled.len() as u64);
            for (actor, tick) in &self.auto_defuse_cancelled {
                h.write_u32(actor.0);
                h.write_u64(*tick);
            }
        }
        if !self.revealed_owners.is_empty() {
            h.write(b"public-building-reveals");
            h.write_u64(self.revealed_owners.len() as u64);
            for (id, owner) in &self.revealed_owners {
                h.write_u32(id.0);
                h.write(&[owner.0]);
            }
        }
        h.write_u64(self.hidden.len() as u64);
        for (p, id) in &self.hidden {
            h.write(&[p.0]);
            h.write_u32(id.0);
        }
        h.write_u64(self.outages.len() as u64);
        for (p, t) in &self.outages {
            h.write(&[p.0]);
            h.write_u64(*t);
        }
        h.write_u64(self.docked.len() as u64);
        for (unit, building) in &self.docked {
            h.write_u32(unit.0);
            h.write_u32(building.0);
        }
        // RA4: each part is written only if it holds something, under its own tag, so that a
        // world without aircraft docks, spawners and cloaking hashes as before.
        for (tag, map) in [(b"am", &self.ammo), (b"rl", &self.reloading)] {
            if !map.is_empty() {
                h.write(tag);
                h.write_u64(map.len() as u64);
                for (id, value) in map {
                    h.write_u32(id.0);
                    h.write_u32(*value);
                }
            }
        }
        if !self.docks.is_empty() {
            h.write(b"dk");
            h.write_u64(self.docks.len() as u64);
            for (unit, (building, dock)) in &self.docks {
                h.write_u32(unit.0);
                h.write_u32(building.0);
                h.write(&[*dock]);
            }
        }
        if !self.bays.is_empty() {
            h.write(b"by");
            h.write_u64(self.bays.len() as u64);
            for (unit, bay) in &self.bays {
                h.write_u32(unit.0);
                bay.hash_into(h);
            }
        }
        if !self.spawned.is_empty() {
            h.write(b"sp");
            h.write_u64(self.spawned.len() as u64);
            for (unit, parent) in &self.spawned {
                h.write_u32(unit.0);
                h.write_u32(parent.0);
            }
        }
        if !self.missiles.is_empty() {
            h.write(b"ms");
            h.write_u64(self.missiles.len() as u64);
            for (unit, flight) in &self.missiles {
                h.write_u32(unit.0);
                flight.hash_into(h);
            }
        }
        if !self.surfaced.is_empty() {
            h.write(b"sf");
            h.write_u64(self.surfaced.len() as u64);
            for (unit, until) in &self.surfaced {
                h.write_u32(unit.0);
                h.write_u64(*until);
            }
        }
    }
}
