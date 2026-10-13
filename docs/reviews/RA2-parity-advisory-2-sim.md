# Advisory review 2, slice `sim`: the simulation code of RA4

Scope: `crates/sim/src/` as changed by `46e694d..6a41bc3` (`veterancy.rs`, `flight.rs`, `crew.rs`,
`combat.rs`, `special.rs`, `movement.rs`, `economy.rs`, `economy_commands.rs`, `command.rs`,
`ai.rs`, `hash.rs`, `event.rs`, `schedule.rs`), read in the snapshot of `6a41bc3`. Advisory: it
gates nothing. All line numbers are those of the snapshot.

Probes (throwaway, in the snapshot only): `crates/sim/tests/probe_sim_a.rs` (RA4 fixture) and
`crates/sim/tests/probe_sim_b.rs` (shipped data). Run with

```
cd ~/Library/Caches/MyGame001/advisory-review/ra4 && CARGO_TARGET_DIR=~/Library/Caches/MyGame001/advisory-review/target \
  cargo test --release -j 4 -p sim --test probe_sim_a -- --nocapture --test-threads=1
```

The probes print what they observe and assert nothing, so they pass; the observations are quoted
below. The five RA4 test files `ra4_veterancy`, `ra4_crew`, `ra4_aircraft`, `ra4_spawns` and
`ra4_cloak` pass in the snapshot (20, 10, 14, 14, 8).

Counts: **P0 1, P1 4, P2 6**, suspected and not confirmed 4.

## Findings

### P0-1. Ammunition is spent only by the last shot of a volley at a unit or building: shots at scenery are free, and so is a volley that is cut short

* **Where:** `crates/sim/src/special.rs:1491` to `:1498` (`resolve` calls `flight::fired` only
  after `index.get(&attacker)` **and** `index.get(&target)` both succeed; scenery is not in the
  index of actors), and `crates/sim/src/flight.rs:715` (`fired` takes a round only when
  `burst_left == 0` after the shot), with `crates/sim/src/combat.rs:1479` (a volley whose next
  shot is not fired ends without a shot, so without a `WeaponFired`).
* **Evidence, scenery (probe `probe_b_shots_at_scenery_spend_no_ammunition`):** a striker of the
  fixture (2 volleys of 2 shots of 20) is ordered onto a tower (scenery, 1000 hit points). In
  400 ticks it fires **50 shots**, the tower is destroyed, and `SpecialState::ammo` never has an
  entry for it ("ammo None", full). It never returns to its dock.
* **Evidence, cut volley (probe `probe_a_interrupted_volley_spends_no_ammunition`):** the same
  striker is ordered onto six trucks with 5 hit points, one after the other. Each dies with the
  first shot of the burst; six shots, six kills, ammunition full after each.
* **On the shipped data** the Harrier's `maverick` and the Black Eagle's `maverick2` have a
  burst of 2 (elite Harrier: 4) and `Ammo` 1, and every map has scenery. The scenery case needs
  nothing else. The cut volley needs the target to die within the 5 ticks between the two
  missiles and nothing else in range for the second (probe `probe_sim_b`: at one cell the first
  Maverick lands after 3 ticks; with another enemy in range the idle Harrier fires the second
  missile at that one and the round is spent, so the case is narrower there than on the fixture).
* **Same root cause, read and not probed:** a cloakable unit that fires at scenery is not put
  into `SpecialState::surfaced` (`flight.rs:685` is behind the same call), so it stays hidden
  while it fires.
* **Expected (R49 item 1, plan 4.8):** an aircraft with `ammo` n fires n volleys, whatever it
  fires at, and a volley that was begun counts.
* **Minimal fix:** in `resolve`, look up the attacker alone before `flight::fired` and require
  the target only for the role handling below it; take the round with the **first** shot of a
  volley (in `fired`: when the weapon's `burst_left` is `burst - 1` of the fired weapon, or
  simply when `weapon.volley` began), and let `combat.rs:1417` skip an aircraft only when
  `ammo == 0 && weapon.burst_left == 0` so that the volley is finished. Tests: an aircraft
  ordered onto scenery fires `ammo` volleys; a target that dies with the first shot costs a
  round; a submerged unit that fires at scenery is seen for 24 ticks.

### P1-1. A launcher ordered onto scenery fires blanks for ever

* **Where:** `crates/sim/src/special.rs:1491` (no `fired`, so the bay never gets a target),
  `crates/sim/src/flight.rs:570` (`bays` looks the target up in the actors and drops it),
  `crates/sim/src/combat.rs:1207` (the launcher's shot is no impact).
* **Evidence (probe `probe_e_a_launcher_ordered_onto_scenery`):** `Attack` of a launcher on a
  tower is accepted. In 600 ticks: 11 `WeaponFired` of the launcher, 0 `UnitLaunched`, no
  `AttackEnded`, the order still `Attack { .. }`, the tower at 1000 hit points, no bay.
* **Why it matters:** a silent no-op that never ends (the interface draws a launch every 200
  ticks, `Definitions::launcher_weapons`); the shipped `v3_launcher` and `dred_launcher` have
  warheads for wood and walls, so the order is accepted there too. RA5 adds bridges and tech
  buildings as targets of the same kind.
* **Expected:** the rocket flies to the scenery's cell and goes off there (a missile's aim is a
  point), or the order is refused or ends with `AttackEnd::NoDamage`.
* **Minimal fix:** in `bays` and `launch`, take the aim of a target that is scenery from
  `Props::get` (its cell); a carried aircraft may be given the same `Attack` order. Otherwise
  end the order in `act_unit` when the chosen weapon is a launcher and the target is scenery.

### P1-2. A spawner that boards a transport and leaves it has all its missiles again at once

* **Where:** `crates/sim/src/flight.rs:537` to `:541` (`bays` removes the bay of a spawner that
  is not among the actors), `crates/sim/src/special.rs:33` (`Passenger` keeps the id and the
  experience, not the bay) and `:88` (`launch_ready` is true without a bay).
* **Evidence (probe `probe_d_boarding_a_transport_refills_the_bay`):** a launcher launches at
  tick 2, boards the hovercraft beside it, is unloaded, and launches again at **tick 8**; the
  fixture replaces a rocket 200 ticks after it is gone. Two rockets of one launcher are in the
  air.
* **Why it matters:** the shipped V3 Launcher (533 ticks) and the Amphibious Transport; the
  number of launched units of one spawner is bounded by `spawns_number` only while its bay
  lives. Read, not probed: `validate_interaction` (`special.rs:893` to `:910`) lets an
  amphibious transport take any unit that is no aircraft, so the same holds for a spawner with
  aircraft out, which are destroyed when it boards and are back at once when it leaves. The
  weapon's own cooldown is reset the same way since M9 (`unload` builds a new `Weapon`); R47
  item 7 carried the experience across and nothing else.
* **Expected (R50 item 2):** a place is filled `spawn_regen` ticks after its unit is lost.
* **Minimal fix:** keep the bay with the passenger (`Passenger::bay`, hashed like
  `experience`) and put it back in `unload`, with its `Out` places turned into `Lost(regen)`
  for an aircraft and left as they are for a missile in flight; or refuse to embark a unit
  whose bay has a place that is not `Ready`. Test: launch, board, leave, no launch before the
  time to replace.

### P1-3. `Attack` tells a hidden unit anywhere on the map from an id that does not exist

* **Where:** `crates/sim/src/command.rs:377` to `:383`: an `Attack` whose target is in
  `SpecialState::hidden` for the issuing player is answered with `UnknownTarget` **before**
  `validate_player`, `validate` and `validate_combat` run.
* **Evidence (probe `probe_c_rejections_on_a_hidden_unit`):** player 0, a submerged sub of
  player 1 five cells from its tank ("near") and one at (35, 35) under the shroud ("far"), and
  the id 99999:

  | Command of player 0 | near sub | far sub | no such id |
  |---|---|---|---|
  | `Attack { units: [], target }` | `UnknownTarget` | `UnknownTarget` | `NoUnits` |
  | `Attack { units: [engineer], target }` | `UnknownTarget` | `UnknownTarget` | `NotArmed` |
  | `Attack { units: [tank], target }` | `UnknownTarget` | `UnknownTarget` | `UnknownTarget` |

* **Why it matters:** ids are handed out in order, so with an empty unit list a player learns
  for every id whether it is a cloaked (or disguised, or mirage) enemy unit that it does not
  detect, wherever it is. The line is M9's; RA4 makes every Typhoon, Dolphin and Giant Squid a
  member of the set (R51 item 1 relies on "rejected as on an unknown target").
* **Expected (D30, plan 3.4):** the same answers as for an id that does not exist.
* **Minimal fix:** drop the special case and let `validate_combat` decide: add
  `&& !special.hidden.contains(&(issued.player, target))` to its `seen_by` arm
  (`command.rs:746`). Negative test: the three rows above answer alike.

### P1-4. `Move`, `Stop`, `Harvest`, `Attack` and `AttackMove` that name a submerged enemy unit in a visible cell answer `NotOwned`

* **Where:** `crates/sim/src/command.rs:645` to `:652` (`validate`): "hidden" there is the fog
  alone; `SpecialState::hidden` is not asked.
* **Evidence (probe `probe_c`):** `Move { units: [near sub] }` and `Stop` answer
  `NotOwned(SimId(2))`; for the far sub and for the id 99999 they answer `UnknownUnit`. The sub
  was in `hidden` for player 0 in that tick.
* **Why it matters:** the brief of the milestone asks that no command rejection leaks a hidden
  unit; a player who sees the water learns that an enemy unit is in it. (`Deploy` and
  `Interact { unit }` answer `NotOwned` for every enemy unit, seen or not: that is finding P1-1
  of advisory review 1 and is not repeated here; its fix should ask the hidden set too.)
* **Expected:** `UnknownUnit`, as for a unit under the fog.
* **Minimal fix:** pass `&special.hidden` to `validate` and treat `(issued.player, id)` in it as
  hidden. Negative test with the fixture's sub.

### P2-1. In the tick in which a building with docks is sold or lost, an aircraft more than the docks can be queued and paid for

`crates/sim/src/economy_commands.rs:313`: the docks are `max(docks of the buildings in owned,
PlayerState::docks())`, and the second is as of the last `Economy` stage. Probe
`probe_l2_queueing_in_the_tick_a_dock_is_sold`: two airfields (4 docks), `SellBuilding` of one
and five `QueueProduction` in one tick: four are accepted with 2 docks left. After 3000 ticks
two strikers exist and the third is complete and fully paid (`progress` 10, `charged` 300) and
waits at the head of the queue for ever, with the fourth behind it. By reading, the same window
is open for one tick after an airfield is destroyed in combat. The comment says the maximum is
for a caller that lists every type once (the sidebar). Suggested: let `QueueProduction` pass the
real list and use it alone; give the sidebar's caller a flag or the per building list.

### P2-2. An aircraft whose dock cell is held by another aircraft never lands, never reloads, and asks for a path every few ticks

`crates/sim/src/flight.rs:342` to `:348`. Probe `probe_f_an_aircraft_above_the_dock`: two
gunships of the same player stand above the two docks of an airfield; the striker comes to rest
one cell away, has its dock, and in 300 ticks completes 31 moves (`MoveCompleted`) without ever
standing on it. Invariant I6' holds on every tick. Any aircraft that needs no dock (a
NightHawk, a Kirov, an enemy's) does this. Suggested: count "landed" when the aircraft stands
within one cell of its dock cell and the cell is held, or do not hand out a destination that an
idle aircraft holds; at least do not issue a new order while the last one ended beside the dock.

### P2-3. An order that ends because the ammunition is spent has no event

`crates/sim/src/flight.rs:320` to `:324` and `:722` to `:724` call `clear_orders` and write
nothing; every other end of an `Attack` writes `SimEvent::AttackEnded` (`combat.rs:1791`). The
interface and the computer player cannot tell "out of ammunition" from "still attacking".
Suggested: a reason `AttackEnd::NoAmmunition`.

### P2-4. The turn of the last stand can never come for some think intervals the data admits

`crates/sim/src/ai.rs:1353` with `:807`: a player thinks in the ticks `t = id (mod interval)`
and may sell when `(t / 256) % 16 == id % 16`. For an interval of 512 only even turns are ever
met, for 1024 only every fourth (`MAX_THINK_INTERVAL` is 1200), so a player with an odd id never
sells. By arithmetic, not run; the shipped intervals (10, 20, 40) are not affected. Suggested:
remember that the turn was missed (sell at the first think at or after the turn began), or
reject intervals that do not meet every turn.

### P2-5. `rank_of` and `experience_for` disagree for a type that costs nothing

`crates/sim/src/veterancy.rs:166` to `:185`: `experience_for(rank, 0, ratio)` is 0, and
`rank_of(0, 0, ratio, cap)` is 0 (`cost.max(1)`), so the documented property "a unit with
exactly this much has the rank" fails, and such a unit is elite after its first kill worth 6
credits. No trainable type of the shipped data costs 0 (checked); `promote` and `inherit` would
give such a unit a rank whose experience `award` then reads as rank 0 (it never lowers a rank,
so nothing breaks). Suggested: one definition of the threshold, used by both.

### P2-6. Decision text against code

* R50 item 4 says a launched aircraft "attacks only what its spawner fires at"; that holds
  because an aircraft with a move order does not fire (`combat.rs:1926`), not by a rule of
  `flight.rs`. Worth a sentence, since a change to idle fire would break it silently.
* R53 item 2 says the last stand sells "one a turn": it is one building per think inside a turn
  of 256 ticks, so with the shipped think intervals a whole base of defences goes in one turn;
  only the survivors of a crewed building interrupt it (they are units that fight).
* The module documentation of `veterancy.rs` (`:27` to `:32`) says speed is `max(1, ..)`;
  `scaled_speed` also caps at `MAX_UNIT_SPEED` (R47 item 5 says so).

## Suspected, not confirmed

1. **Two killers for one victim.** A unit whose hit points a missile brings to 0 in the
   `Economy` stage (`flight.rs:449`, `detonate`) stays on the grid until `Cleanup`; if a crusher
   drives over it in `Movement` of the same tick, `movement.rs:327` records a second kill, and
   launcher and crusher both gain its cost. Read, not probed. (That such a unit still moves and
   fires in that tick is the older behaviour of every area effect of `special.rs`.)
2. **The last stand of a player whose only obstacle is a building it cannot place.** A player
   with no unit that fights, no harvester at work and a finished building that
   `place_ready_building` cannot place stands still by the picture of `last_stand` (credits,
   counts, progress) and sells its base after 1,200 ticks plus its turn, although it has
   credits. Whether a match reaches this I could not run (no AI matches allowed). The other
   ways I looked for a wrongful sale came to nothing: a player with credits and a factory queues
   units (progress changes), an armed idle War Miner counts as a unit that fights
   (`View::fights`), the computer player fires no superweapon, and a player that only has
   defences cannot win by itself.
3. **A missile or aircraft bought with ALL UNLOCKED.** `check_buildable` refuses tech level -1
   whatever the option, so the four spawned types cannot be queued; read only at
   `economy_commands.rs:259`, the game's roster not checked.

4. **A launched unit is moved by the Chronosphere.** `Superweapon { units }` of a Chronosphere
   (`special.rs:779` to `:820`) takes the player's units of category `vehicles`, and the four
   launched types are `vehicles` of the air domain in the shipped arsenal (so are the
   fixture's); it does not ask `commandable`, and after the teleport it calls `clear_orders`.
   A missile without an order goes off where it stands (`flight.rs:413` to `:424`), so a rocket
   in flight could be put down on any visible cell, against R50 item 7 ("a launched unit takes
   no command"). Read, not probed: the fixture has no Chronosphere. Suggested: `commandable`
   for each unit of the list, with a negative test.

## Checked and found correct

* **Hashing and determinism.** Every new piece of state is in `state_hash` in a fixed order:
  `Experience` (value, rank, self heal, both multiplier sets) per unit in id order;
  `Passenger::experience`; `Projectile::firepower_pm`; `SpecialState::{ammo, reloading, docks,
  bays, spawned, missiles, surfaced}` (BTreeMaps, each under a tag); `PlayerState::{aircraft,
  docks}`; the computer player's `stalled` and `last_stand`; `Kills` and `Wrecks` (emptied in
  `Cleanup`, also without a rules file). Iteration that changes state is over BTreeMaps or
  sorted ids (`enlist`, `award` in the order of the kills, `escape` in the order of the
  records, `docks`, `spawned_units`, `bays`). No floating point, no `HashMap`; `SimRng` only in
  `crew::chance` (one draw per crewed vehicle that stood still, one per construction yard, no
  draw for an uncrewed type, a moving vehicle, an aircraft or an owner without a side).
* **I8, I12.** No `unwrap`, `expect` or panic in the new non test code; `bay.slots[place]`
  follows `position`; `pending -= 1` is guarded; the damage, rank and reload arithmetic is in
  `i128`/`u64` with saturating conversions; `world.entity_mut` is only used on entities found
  in the same call.
* **Plan 4.5.** Thresholds inclusive in exact integers, cap from the rules, one event per
  promotion at the rank reached (also 0 to 2 in one kill), abilities by `contains` (once);
  nothing for own units, allies (`Players::hostile`), buildings as killers, untrainable types,
  scenery; credit for splash (`Blast::source`), crushing, a missile (the launcher directly),
  a launched aircraft (`spawned`, and `Projectiles::reassign` once it is gone). No double
  credit between `combat` and `detonate` (`before > 0`).
* **Plan 4.3 order.** `impact_damage`: firepower inside the one division with falloff and
  `Verses`, minimum 1, then the armour divisor, then `max_damage` in the caller; equal to
  `splash_damage` for multipliers of 1000. Reload scaled where a volley ends (three places),
  not the burst gap; speed scaled before the slowing; self heal 1 point on multiples of the
  repair interval, only above 0 and below the type's hit points, before the missiles of the
  tick go off.
* **Plan 4.7.** Who is crewed, the counts (`survivors`), the side of the owner's country, room
  through `spawn_unit` (I6'), the loop bounded by the room; sold buildings are recorded once
  and are not in `remove_dead`; deploy and undeploy make no wreck. Refunds unchanged (I7).
* **Plan 4.8 and 4.6.** Docks never shared (`free` reads the map as it fills), kept only while
  aircraft and building have one owner; reload counted only while landed; an aircraft without
  any dock is lost only when idle; production waits (`continue` skips one queue only). Bay
  timers; launch gap 27; a missile shot down deals nothing and is replaced; a missile outlives
  its launcher and its entries are removed when it goes off; aircraft die with their spawner
  one tick later and their entries go the tick after; flight is bounded by the stall rules of
  `move_units`. Launched units take no `Move`, `Stop`, `Attack`, `AttackMove`, `Harvest`,
  `Deploy`, `Interact` (but see suspected item 4 for the Chronosphere).
* **Submerged units.** Detection `dx^2 + dy^2 <= s^2` to the nearest cell of a building, per
  player; `fired` runs in the `Combat` stage of the shot, so the unit is seen in 24 visibility
  passes; no weapon acquires a hidden unit (`visible_enemy`, `nearest_enemy_in_range`,
  `keep_or_acquire`); `Interact` answers `UnknownTarget`; `special.rs` has no target choice of
  its own; the computer player's `enemy_units` leaves hidden units out and its own units leave
  launched ones out.
* **The last stand.** Reached only with no unit that fights and no harvester at work and an
  unchanged picture for 1,200 ticks; ends when either changes; bounded by the number of
  sellable buildings; the turns of ten player ids never overlap.

## Not checked

* Any match of computer players (`ra4_ai.rs`, `m6_ai`, the duels, the soak, the benchmark) and
  the unit tests inside `sim` (`--lib`): not run, as instructed. The figures of R53 item 4 and 6
  are therefore unverified.
* The `game` crate (what it does with the new events and with hidden units), the data files and
  the generator, the arsenal's validation (`arsenal.rs`), and the adequacy of the RA4 tests
  beyond running five files: other slices.
* Shipped data behaviour except the Harrier's volley (probe `probe_sim_b`); the Dreadnought,
  the carrier and the Destroyer were read, not run.
* Whether Red Alert 2 itself charges a round for a volley that is cut short, or resets a
  spawner in a transport: judged against R49 and R50, not against the original.
