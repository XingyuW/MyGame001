//! Active radiation sites only. Linear radial attenuation, stepped decay and global damage cadence.
use super::*;

#[derive(Clone, Debug)]
pub struct Site {
    pub cell: CellPos,
    pub spread: u32,
    pub level: u32,
    pub born: u64,
    pub duration: u64,
    pub source: Option<SimId>,
}
impl Site {
    pub fn level_at(&self, tick: u64, delay: u32) -> u32 {
        self.cell_level(tick, delay, self.cell)
    }
    /// Appendix D13: attenuate once, then truncate the loss of each decay step.
    pub fn cell_level(&self, tick: u64, delay: u32, cell: CellPos) -> u32 {
        let elapsed = tick.saturating_sub(self.born);
        if elapsed >= self.duration || self.spread == 0 {
            return 0;
        }
        let dx = i128::from(cell.x) - i128::from(self.cell.x);
        let dy = i128::from(cell.y) - i128::from(self.cell.y);
        let distance = (((dx * dx + dy * dy) as u128 * 65536).isqrt()) as u64;
        let radius = u64::from(self.spread);
        if distance >= radius {
            return 0;
        }
        let initial = u64::from(self.level) * (radius - distance) / radius;
        let steps = (self.duration / u64::from(delay.max(1))).max(1);
        let taken = elapsed / u64::from(delay.max(1));
        initial.saturating_sub((initial / steps).saturating_mul(taken)) as u32
    }
}
pub fn level(world: &World, cell: CellPos) -> u32 {
    let Some(rules) = world.resource::<Definitions>().rules() else {
        return 0;
    };
    let tick = world.resource::<SimTick>().0;
    world
        .resource::<SpecialState>()
        .radiation_sites
        .iter()
        .fold(0u32, |sum, site| {
            sum.saturating_add(site.cell_level(tick, rules.radiation.level_delay as u32, cell))
                .min(rules.radiation.level_max as u32)
        })
}
/// Shared with RA6-B payloads/death weapons. Spread is truncated to whole reference cells.
pub(crate) fn add(
    world: &mut World,
    cell: CellPos,
    spread: u32,
    amount: u32,
    source: Option<SimId>,
) {
    let Some(rules) = world.resource::<Definitions>().rules().map(|r| r.radiation) else {
        return;
    };
    if amount == 0 || spread < 256 {
        return;
    }
    let tick = world.resource::<SimTick>().0;
    let mut state = world.resource_mut::<SpecialState>();
    let spread = spread / 256 * 256;
    let existing = state
        .radiation_sites
        .iter()
        .position(|s| s.cell == cell && s.spread == spread);
    let old = existing.map_or(0, |i| {
        state.radiation_sites[i].level_at(tick, rules.level_delay as u32)
    });
    let level = old.saturating_add(amount).min(rules.level_max as u32);
    // RadDurationMultiple is a frame multiplier, then 15 frames / 20 ticks.
    let duration = (u64::from(level) * rules.duration_multiple_pm as u64 * 4 + 1500) / 3000;
    let site = Site {
        cell,
        spread,
        level,
        born: tick,
        duration,
        source,
    };
    if let Some(i) = existing {
        state.radiation_sites[i] = site;
    } else {
        state.radiation_sites.push(site);
    }
}
pub(crate) fn advance(world: &mut World) {
    if world.resource::<SpecialState>().radiation_sites.is_empty() {
        return;
    }
    let tick = world.resource::<SimTick>().0;
    let defs = world.resource::<Definitions>().clone();
    let Some(rules) = defs.rules() else {
        return;
    };
    world
        .resource_mut::<SpecialState>()
        .radiation_sites
        .retain(|s| s.level_at(tick, rules.radiation.level_delay as u32) > 0);
    if !tick.is_multiple_of(rules.radiation.application_delay.max(1) as u64) {
        return;
    }
    let Some(effect) = defs.effect(Effect::Radiation) else {
        return;
    };
    let index = living(world);
    let hits: Vec<_> = index
        .values()
        .filter(|a| {
            a.foot.is_none()
                && defs
                    .metadata(a.item)
                    .is_some_and(|m| m.domain != Domain::Air && !m.immune_radiation)
        })
        .filter_map(|a| {
            let level = level(world, a.cell);
            let damage = (u64::from(level) * rules.radiation.level_factor_pm as u64 / 1000) as i32;
            let source = world
                .resource::<SpecialState>()
                .radiation_sites
                .iter()
                .find(|s| near(s.cell, a.cell, s.spread / 256))
                .and_then(|s| s.source);
            (damage > 0).then_some((a.id, position_of(world, a), damage, source))
        })
        .collect();
    for (id, point, damage, source) in hits {
        crate::combat::detonate_one(
            world,
            Blast {
                point,
                air: false,
                warhead: effect.warhead,
                damage,
                direct: Some(id),
                firepower_pm: 1000,
                source,
            },
            id,
        );
    }
}
