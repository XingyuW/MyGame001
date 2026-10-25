//! Ore that grows and spreads (RA5, docs/RA2_PARITY_PLAN.md section 4.10; docs/DECISIONS.md
//! R59).
//!
//! Only with a rules file, and only while the match's rule `ore_grows` is on. At the end of the
//! `Commands` stage (so that no ore spreads between the tick in which a player, or a computer
//! player in the `Ai` stage, decides where to build and the next tick's `Commands` stage, in
//! which the placement is checked: nothing is built on ore):
//!
//! * **Growth.** On every tick whose index is a positive multiple of the rules'
//!   `ore_growth_interval`, each cell of ore below `ore_max_bails` gains one bail with the
//!   probability `ore_growth_pm`.
//! * **Spread.** On every tick whose index is a positive multiple of `ore_spread_interval`, each
//!   cell of ore that holds more than `ore_spread_above` bails puts, with the probability
//!   `ore_spread_pm`, one bail on one of the cells around it that ore may spread onto
//!   ([`Grid::takes_ore`]), each of them equally likely.
//!
//! Gems neither grow nor spread. A pass visits the cells of ore in row-major order with the
//! bails they held when it began, so ore planted or grown by the pass is not visited by it;
//! for each cell the growth is drawn before the spread, and a cell that cannot grow, or has no
//! cell to spread onto, draws nothing for it. All randomness comes from [`SimRng`] in that
//! order (invariants I3 and I4). A probability of 0 draws nothing, so a rules file that turns
//! growth and spread off leaves the random sequence of its world as it was.

use bevy_ecs::system::{Res, ResMut};

use crate::definitions::Definitions;
use crate::map::{CellPos, Grid};
use crate::match_setup::MatchState;
use crate::rng::SimRng;
use crate::schedule::SimTick;

/// The eight cells around a cell, in the order a spreading cell counts them.
const AROUND: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// Whether an event of `pm` permille happens: one draw from `rng`.
fn chance(rng: &mut SimRng, pm: u32) -> bool {
    rng.next_u32() % 1000 < pm
}

/// Lets the ore grow and spread when a pass is due (see the module documentation).
pub(crate) fn grow(
    mut grid: ResMut<Grid>,
    definitions: Res<Definitions>,
    state: Res<MatchState>,
    tick: Res<SimTick>,
    mut rng: ResMut<SimRng>,
) {
    let Some(rules) = definitions.rules().map(|rules| rules.economy) else {
        return;
    };
    if !state.rules().ore_grows || tick.0 == 0 {
        return;
    }
    let positive = |value: i32| u32::try_from(value.max(0)).unwrap_or(0);
    let due = |interval: i32| tick.0.is_multiple_of(u64::from(positive(interval).max(1)));
    let growth_pm = positive(rules.ore_growth_pm);
    let spread_pm = positive(rules.ore_spread_pm);
    let grows = growth_pm > 0 && due(rules.ore_growth_interval);
    let spreads = spread_pm > 0 && due(rules.ore_spread_interval);
    if !grows && !spreads {
        return;
    }
    let max_bails = positive(rules.ore_max_bails).max(1);
    let spread_above = positive(rules.ore_spread_above);
    for (cell, bails) in grid.ore_cells() {
        if grows && bails < max_bails && chance(&mut rng, growth_pm) {
            grid.grow_ore(cell, max_bails);
        }
        if spreads && bails > spread_above {
            let free: Vec<CellPos> = AROUND
                .iter()
                .map(|&(dx, dy)| CellPos::new(cell.x.saturating_add(dx), cell.y.saturating_add(dy)))
                .filter(|&target| grid.takes_ore(target))
                .collect();
            if !free.is_empty() && chance(&mut rng, spread_pm) {
                // At most eight cells, so the remainder is an index.
                let pick = usize::try_from(rng.next_u32() % (free.len() as u32)).unwrap_or(0);
                if let Some(&target) = free.get(pick) {
                    grid.plant_ore(target);
                }
            }
        }
    }
}
