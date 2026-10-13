# Advisory review, slice `sim`: the simulation code of RA1 to RA3

Reviewed: the snapshot of commit `46e694d` (`~/Library/Caches/MyGame001/advisory-review/ra3/`),
`crates/sim/src/` as changed by `21225b0..46e694d`. Advisory only: it gates and approves nothing.
All paths and line numbers below are in the snapshot.

Counts: **P0 1, P1 1, P2 5**, suspected and not confirmed 2.

Probes (throwaway, left in the snapshot, nothing else touched):
`crates/sim/tests/probe_sim_undeploy.rs`, `crates/sim/tests/probe_sim_leak.rs`. Run with
`cargo test --release -j 4 -p sim --test probe_sim_undeploy -- --nocapture` (two of its tests
fail on purpose: they assert the expected behaviour).

## Findings

### P0-1. A yard or base unit destroyed by a command of the tick lives on through a later `Undeploy` or `Deploy` of the same tick (and a destroyed building can still be sold)

* **Where:** `crates/sim/src/special.rs:161` (`actors`: no test of hit points), `:418`
  (`share_of` clamps the result to at least 1), `:431` to `:507` (`Deploy`), `:534` to `:619`
  (`Undeploy`); `crates/sim/src/economy_commands.rs:541` to `:560` (`SellBuilding`: the
  `BuildingIndex` knows no hit points).
* **Mechanism:** a `Superweapon` command applies its damage in the `Commands` stage
  (`special::area` to `combat::detonate`, `combat.rs:1466`), and the dead are removed only in
  `Cleanup`. Until then an entity with hit points at or below 0 is an ordinary actor for every
  later command of the tick. `Undeploy` and `Deploy` (with a rules file) then build the new
  unit or yard with `share_of(hp <= 0, ..)`, which is 1, so `remove_dead` no longer finds it.
  The share rule is RA1's (decision R12), so this is the programme's own code path; without a
  rules file `Deploy` gives a whole yard, as since M9.
* **Evidence (probe, RA3 fixture, through `CommandQueue` and `SimSchedule`):** a Soviet yard at
  20 hit points under a nuclear strike of the other player.
  * Control, strike alone: `BuildingDestroyed` in the tick.
  * Strike, then `Undeploy` of the victim in the same tick: no rejection; after the tick the id
    is a unit with 1 hit point and is still alive three ticks later.
  * Base unit at 20 hit points, strike, then `Deploy`: the id is a yard with 1 hit point.
  * Strike, then `SellBuilding`: `BuildingSold { refund: 800 }`, credits 5000 to 5800, no
    `BuildingDestroyed`.
  (The low hit points were set with the fixture helper `set_hp` to stand for earlier damage;
  the lethal damage and the commands went through the production path.)
* **Reach:** one tick wide, and today only the nuclear strike deals damage in the `Commands`
  stage; the computer player never undeploys. It stays a wrong result of the simulation that a
  player can trigger (the victim's `Undeploy`, `Deploy` or sale in the tick of the impact), and
  RA6 adds more command driven damage.
* **Expected:** an entity whose hit points are at or below 0 is gone for commands: `Undeploy`,
  `Deploy`, `SellBuilding` (and `Interact`, `RepairBuilding`) are rejected as for a missing id,
  and the entity is removed in `Cleanup`.
* **Minimal fix:** leave entities with `hit_points <= 0` out of `special::actors` when it is
  built for `apply` (or return `UnknownUnit`/`UnknownBuilding` for them in `own` and in the
  `Undeploy` lookup), and leave them out of `EconomyParams::building_index`. Regression tests:
  the three probe cases.

### P1-1. `Deploy`, `Interact` and `Superweapon` tell an unseen enemy unit or building from a missing id

* **Where:** `crates/sim/src/special.rs:406` (`own`), used at `:432` (`Deploy`), `:621`
  (`Interact`, which since RA2 is also the Service Depot order, decision R27), `:655`
  (`Superweapon { building }`), `:694` (the units of a Chronosphere).
* **Evidence (probe `probe_sim_leak.rs`):** player 0 sees neither a tank of player 1 at
  (36, 36) nor its yard at (30, 30). For both ids `Deploy`, `Interact { unit }` and
  `Superweapon { building }` answer `NotOwned(id)`; for the id 9999 they answer
  `UnknownUnit(9999)`. `Move`, `Undeploy` and `SellBuilding` answer the same for both
  (`UnknownUnit`, `UnknownBuilding`), as decisions D30 and D33 require.
* **Why it matters:** ids are handed out in order, so a player can ask for any id whether an
  enemy unit or building it has never seen is alive. PLAN.md section 5 and plan section 3.4
  ask that a rejection reveals nothing the player does not see. The code of `own` is M9's, but
  R27 item 1 states the behaviour for the new depot order as "`NotOwned`, as for every unit
  command", and that is not what the unit commands do under fog (`command.rs:621` to `:631`).
* **Expected:** another player's unit or building that the issuing player does not see (or
  that is in `SpecialState::hidden` for it) is reported as `UnknownUnit` or `UnknownBuilding`.
* **Minimal fix:** give `own` the player's vision: when the owner differs and the definitions
  have a combat file, return `UnknownUnit(id)` unless `Body::of(..).seen_by(vision)` and the id
  is not hidden for the player; correct the sentence of R27; one negative test per command.

### P2-1. The results of `Grid::claim` are dropped where a unit is put into a cell

`crates/sim/src/special.rs:301` (teleport), `:587` (undeploy), `:961` (unload) and
`crates/sim/src/unit.rs:361` (`spawn_unit`) call `claim` and ignore `false`. Each call is
guarded by a check made just before (`room_for`, `has_room`, `room_in`, `occupant_for`) that is
written separately from the rule inside `claim_spot`; if the two ever disagree, a unit exists
that holds no cell and invariant I6' is broken without a sign. Only `Spawner::leave`
(`economy.rs:787`) handles the refusal. Suggested: handle `false` as `leave` does (unregister
and give up), or at least `debug_assert!` the result.

### P2-2. The tail of the grid's hash has no lengths

`crates/sim/src/map.rs:640` to `:648`: the aircraft of the air layer (index and id, 12 bytes
each) and then the ids of the units on foot (4 bytes each) are written without a count, so
three units on foot and one aircraft give the same bytes at this place. The states differ
elsewhere in the hash (the units themselves), so no collision is reachable today; every other
variable length part of `state_hash` writes its length first. Suggested: write both counts
(changes the hash of worlds with units on foot only if the count of the air layer is written
conditionally; no test holds an expected hash).

### P2-3. A unit on foot that changes its spot on a diagonal step is for some ticks in a cell it does not hold

`crates/sim/src/unit.rs:170` (`Mobile::position`): the straight line from spot 0 of (0, 0) at
(64, 64) to spot 2 of (1, 1) at (368, 464) is in cell (0, 1) from about 48 to 63 percent of the
step. `combat::blast_hits` (`combat.rs:754` to `:812`) finds units through the cells they hold
and, for a warhead without a spread and no direct hit, tests `position.cell() == home`. With
the spreads of the shipped data I found no geometry in which such a soldier is missed (worked
by hand for 77, 128, 256 and 384 subcells); an inaccurate weapon whose warhead has no spread,
which the format admits, would miss it. Decision R6 item 5 mentions that two soldiers may be at
one point for a tick but not this. Suggested: say so in R6, or scan one cell more around the
box.

### P2-4. `Deploy` looks at the footprint before it asks whether the player sees it

`crates/sim/src/special.rs:463` to `:470`: `check_footprint` (which names an occupied or
unbuildable cell) runs before "Explore the deployment footprint first", the opposite order of
`check_placement` (`economy_commands.rs:383` to `:389`), whose comment gives the reason. M9's
code, but since RA1 every match starts with this command. Normally unreachable, because the
footprint lies inside the base unit's own sight; a Gap Generator (`Vision::conceal`) can make
it reachable. Suggested: swap the two checks.

### P2-5. A volley is kept while its unit is disabled or rearming

`crates/sim/src/combat.rs:1281`: the `continue` for a disabled or rearming unit comes before
the rule "a shot of a volley that cannot be fired when it is due ends the volley" (`:1333`), so
a unit switched off in the middle of a burst keeps `burst_left` and fires the rest when it
works again, however long that takes. Harmless and deterministic (the state is hashed), but
not what the module documentation says. Suggested: apply the `due` rule before the `continue`.

Also inherited and unchanged: `special.rs:1684` `.unwrap()` in `launch_fighters` (the entity of
an id that `spawn_unit` has just returned; cannot fail), and `command.rs:854`, `:872`
`.expect("registered command system")`. Judged not reachable from data or commands.

## Suspected, not confirmed

1. **Embarking in the tick of death.** By the same root cause as P0-1, `Interact` of a unit
   with hit points at or below 0 into a transport stores a `Passenger` with those hit points
   (`special.rs:897` to `:914`); it is unloaded with them and dies then. Read, not probed.
2. **Depot and low power.** `repair_docked` (`special.rs:1272`) stops for a spy's outage of the
   building but does not ask `PlayerState::powers`; whether Red Alert 2 stops a `Powered=yes`
   Service Depot under low power is not in the plan or R27. Only the user or the rule research
   can decide; not a finding.

## Checked and found correct

* **Hashing (I3).** Walked `PlayerState` (queues with `fraction`, lane, `radar`, `country`,
  `repairing`, options byte, team, speed bonus, roster), `SpecialState` (all thirteen maps,
  `docked` included), `MatchState` (the four rules, defeated, outcome), `Grid` (spots, units on
  foot, domains, air layer), `Weapon` (`secondary`, `burst_left`, `volley`), `Projectile`
  (`aim`, `scattered`, `air`), `Condition`, `MoveOrder::crush`, `AiController` (every field),
  the arsenal's entry (every trait of schemas 2 to 4) and the rules in `Definitions`. Nothing
  missing.
* **Determinism.** Iteration that changes state is over `BTreeMap`/`BTreeSet` or ids sorted
  first (`combat`, `move_units`, `advance_production`, `repair_buildings`, `repair_docked`,
  `update_conditions` sorts its events); randomness only from `SimRng` (scatter: two draws per
  shot in attacker id order; random countries; starting unit choice); no floating point, no
  clock in the reviewed files.
* **I6'.** Every caller of `claim`, `claim_spot`, `register`, `unregister`, `release`,
  `release_around`, `cover`, `uncover` read: spawn, production exit, unload, teleport
  (Chrono Legionnaire, Chrono Miner, Chronosphere), deploy, undeploy, embark, capture, carrier
  flights, crush, `remove_dead`. `claim_spot` itself refuses a covered cell, a fourth soldier
  and a mix of vehicle and infantry, so no caller can create one. The arsenal loader admits
  `crushable` only on infantry and `crusher` only on surface vehicles. `process_commands`
  flushes deferred entity changes before every specialist command, so `SellBuilding` and
  `Undeploy` of one building in one tick cannot both succeed. `ra3_cells`, `ra3_crush` pass.
* **I7.** Every write of `credits`: `advance_item` (paid units bounded by `credits / step`,
  last unit only if `credits >= last`), the legacy branch, `repair_buildings`, `repair_docked`
  (both test `credits >= cost` first), cancel (refunds `charged`), sell, spy theft (half of a
  non negative amount), purifier, refining. The sum charged for an item is its cost at every
  rate. No path below zero.
* **I8, I12.** No `unwrap`, `expect`, `panic!` outside tests beyond the three above; indexing
  in `match_setup` is behind the checks of faction, start and difficulty; divisors from the
  rules file are range checked in `rules.rs` and guarded with `max(1)`; damage sums in `i64`,
  splash in `i128`, the factory rate in `u128` (at most 1000^10).
* **Equations.** 4.2: `factory_rate_pm` (cap 10), `power_rate_pm` (1000 at output equal to
  drain, clamp 500 to 800), the product and Super Me, queue capacity 1 for a building lane and
  `max_queued_objects + 1` for units, two lanes, build limit as owned plus queued. 4.3:
  `splash_damage` equals the plan's formula, 0 beyond the spread, minimum 1 with `Verses > 0`,
  `max_damage` per hit, one hit per footprint cell, the impact cell only without a spread.
  4.4: weapon choice by layer and armour, the 1 percent floor for automatic acquisition with a
  rules file, minimum range, air range bonus, burst gap and reload, scatter radius. 4.6: refund,
  repair step and cost, interval, unpayable step skipped, `sellable`/`repairable`. 4.9: the
  budget gives 5 and 4, and 4 and 7. Plan 3.3 and R10: the order of the reject reasons, groups,
  factory side, country. Win conditions: short and long game, cargo counted, after `Cleanup`.
  Undeploy: R12 as written (apart from P0-1).
* **Command validation.** `Undeploy` and the new reject reasons (`NoFactoryOfSide`,
  `CountryRequired`, `CountryForbidden`, `DisabledByOptions`, `BuildLimitReached`,
  `NotSellable`, `NotRepairable`) are decided before any state change and only from the
  player's own state or after the ownership and fog check (apart from P1-1).
* **Worlds without rules or arsenal.** Every RA2 and RA3 rule read is behind
  `definitions.rules()` (rate, lanes, queue length, refund, repair, radar, `powers`, the floor,
  `max_damage`, damage states and the slowed vehicle, depot, win condition) or behind a trait
  that no older fixture has (`infantry`, `crusher`); `special::advance` and `resolve` return at
  once without an arsenal; `special::blast` (M9) is still what an unlisted effect uses.
* **Tests run in the snapshot:** `ra2_production`, `ra2_repair`, `ra2_power`, `ra2_placement`,
  `ra3_cells`, `ra3_crush`, `ra3_damage`, `ra3_weapons`: all pass (104 tests).

## Not checked

* `ai.rs` and `ai_defs.rs` beyond the controller's hash and the fact that it acts through
  commands; `skirmish_defs.rs`, `combat_defs.rs` and `rules.rs` loaders beyond the ranges of the
  values used as divisors and limits; `harvest.rs`, `props.rs`, `visibility.rs`.
* The long matches (`m6_ai`, soak, duels) were not run, as instructed; the `game` crate was not
  built or read.
* The measurement of decision R6 item 9 (the hash traces) was not repeated.
* Whether the data values match the reference (another slice).
