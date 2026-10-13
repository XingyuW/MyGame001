# Advisory review at RA3, slice `tests`: test integrity, acceptance criteria, game client

Reviewed tree: the snapshot of commit `46e694d` (`~/Library/Caches/MyGame001/advisory-review/ra3/`),
range `21225b0..46e694d`. Paths below are relative to the snapshot. Advisory only: nothing here
gates or approves anything.

Counts: **P0: 0. P1: 5. P2: 11. Suspected, not confirmed: 4.**

How the evidence was obtained:

* Read: the diffs of every existing test file, the examples, the decisions R4 to R45, the STATUS
  entries RA0 to RA3, the validation logs, the README sections on rules.
* Scripts over `git show`: the set of `#[test]` functions per file before and after, the assertion
  count of every test whose body changed, the legacy fixture against the data shipped at `21225b0`,
  the test names of the baseline log against the RA3 log, the match lengths of the five duel logs,
  the finished matches of the five soak logs.
* Run in the snapshot (sim crate only): `ra1_buildability`, `ra1_match`, `ra2_production`,
  `ra2_power`, `ra2_placement`, `ra2_repair`, `ra3_damage`, `ra3_weapons`, `ra3_crush`, `ra3_cells`
  (135 tests, all pass), and two probe files I added, `crates/sim/tests/probe_tests_i7.rs` and
  `crates/sim/tests/probe_tests_catalogue.rs` (left in the snapshot as evidence; they pass).
* The game crate was reviewed by reading only (a helper agent read it; I re-read the lines behind
  findings 1, 12, 13, 14 and 15 myself).

## P1

### 1. The smoke check "the minimap follows the radar" cannot fail, and the older minimap check now passes on the blank screen

* Where: `crates/game/src/smoke.rs:1797-1798`, `crates/game/src/smoke.rs:2010`,
  `crates/game/src/minimap.rs:507-547`.
* Evidence: the flag is
  `minimap.drawn_tick != Some(tick) || minimap.offline != crate::minimap::radar_online(&host)`.
  `draw_minimap` sets `state.offline = !radar_online(&host)` and `drawn_tick = Some(tick)` in the
  same tick from the same host, so the second operand is always true. The check that the minimap
  has lit pixels (`smoke.rs:2010`) is satisfied by the offline diamond, whose colour is not black:
  the smoke logs of RA1 show `minimap pixels lit: 2064` (a fogged map), those of RA2, RA3-A and
  RA3 show `minimap pixels lit: 262144` in both builds, because the player never owns a radar
  building in a smoke run. No test calls `draw_minimap`; the minimap tests cover `radar_online`,
  `offline_text` and `offline_pixels` only. So since RA2 no automated run draws the map on the
  minimap at all, and none can notice a map drawn without radar or a blank screen with it.
* Claims affected: R30 item 6 ("the smoke test fails if the drawing of a tick and the player's
  radar disagree"), STATUS RA2 row 6 ("the minimap drawn 509 times and following the radar"), RA2
  acceptance "the minimap follows radar". R31 does not list the lit-pixel check as weakened.
* Expected: a check that looks at what was drawn, in both states.
* Minimal fix: in the smoke outcome compare the image data with `offline_pixels(layout)` (equal
  when offline, different when online); add a game test that runs `draw_minimap` on a host with a
  radar building and power, then under low power; let one smoke match own a radar building.

### 2. The soak covers far less of a match since RA2, and no record says so

* Where: `crates/sim/examples/m7_soak.rs:61` (`SOAK_TICKS = 36_000`), `docs/DECISIONS.md:2221`.
* Evidence (counted from `docs/validation/ra2-parity/*/09-soak.log`, the 252 shipped
  configurations):

  | Run | Configurations in which no match ended | Matches ended |
  |---|---|---|
  | RA0 baseline | 91 | 317 |
  | RA1 | 60 | 339 |
  | RA2 | 110 | 243 |
  | RA3-A | 194 | 71 |
  | RA3 | 177 | 95 |

  The end to end limit was doubled for the pace of RA2 (R29) and matches grew again in RA3
  (R38), but the soak still plays 36,000 ticks. In 177 of 252 shipped configurations it now never
  reaches the end of a match, so its checks of I6', I7, the rejection rule, the state limits and
  the determinism of the late game, the end of a match and the restart run on about a quarter of
  what they ran on at RA1.
* R29 only says the soak "passes as it is"; R8, R31, R40, R45 and the STATUS debt lists do not
  mention the loss.
* Expected: the soak's length follows the pace, or the loss is a recorded decision.
* Minimal fix: rescale `SOAK_TICKS` by the pacing factor (72,000), or keep the wall time by
  playing each configuration once to its end and twice only for the hash; otherwise record it as
  debt with these numbers.

### 3. The RA1 regression test for the sweep near the enemy's start lost its failing condition when the limit was doubled

* Where: `crates/game/tests/m6_production_matches.rs:337-349` and `:40`.
* Evidence: the test's only assertion is that Crossroads, Libya against Iraq, both easy, ends
  within `tick_limit`. Its comment gives the defect's signature: "needed 39,983 ticks, more than
  the limit of 36,000". The limit is 72,000 since RA2. In the duel logs the fixed code needs
  41,022 ticks for this pairing at RA2 (`ra2/10-duels.log`), so the defect's extra 13,000 ticks
  of wandering would end near 55,000, inside the new limit. R29 and R31 list the new limit for
  "all four tests" and do not say that this one no longer guards what it names.
* Not measured: I may not run the game crate, so I did not revert the fix to see the test pass.
* Expected: a regression test that fails without the fix.
* Minimal fix: assert the behaviour, not the length: a unit test of `choose_target` (the first
  waypoint of a sweep is the one nearest the enemy's start), as R44 did for its two fixes.

### 4. The doubled `MATCH_TICK_LIMIT` rests on measurements of other matches; the test's own margin is unknown

* Where: `crates/game/tests/m6_production_matches.rs:40`, `:83`, `:131`;
  `crates/sim/examples/m14_duels.rs:181`; `docs/DECISIONS.md:2234`.
* Evidence: R29 measures "`m14_duels -- full`, the same 756 matches as the test", and the test's
  comment says the duels "play the same matches". The test seeds `SimPlugin { seed: 0 }`, the
  duels `seed: 14`; since RA1 the generator decides courses, and R44 item 4 says so ("another
  seed and so along other courses"). I recomputed R29's and R44's tables from the logs and they
  agree with the logs, but they describe the duels. The test prints no lengths (output of a
  passing test is captured), so how near its own 756 matches come to the limit was never
  measured. The duels have one match at 0.96 of the limit at RA3 (69,221 of 72,000), disclosed
  as debt; the baseline before the programme already had one at 0.97 of the old limit.
* Also: the raised limit is listed in R31 under the heading "none deleted, ignored or weakened".
* Expected: the limit's justification and its margin come from the test's matches.
* Minimal fix: let the test write the length of each match over its limit to a file under the
  target directory (or run the duels with seed 0 once) and record median, 90th percentile and
  maximum; correct R29 and the comment at line 83; say in R31 that one limit was raised.

### 5. The mechanics tests moved to the legacy fixture have no counterpart on the shipped ruleset

* Where: `crates/sim/tests/m9_arsenal.rs` (24 tests), `crates/sim/tests/m11_repair.rs` (4),
  `crates/game/tests/m10_unlocks_and_enemies.rs` (5); R15.
* Evidence: these tests kept their assertions (checked: no body changed apart from the path and
  one helper) but now assert a ruleset that is not shipped and has no rules file.
  `crates/sim/src/special.rs` branches on `defs.rules()` in 13 places, `economy_commands.rs` in 8,
  `combat.rs`, `economy.rs`, `ai.rs` and `match_setup.rs` in 4 each: the shipped game runs the
  other branch. On the shipped data nothing replaces
  `every_item_executes_queue_cost_completion_and_placement_or_spawn` or game
  `every_human_production_item_has_a_reachable_unlock_path` (the shipped tests build about 20 of
  87 items through commands), and no shipped test issues a load or unload command, clones
  infantry, rearms an aircraft at its airfield, launches from a carrier, mind controls, steals
  credits with a spy, plants a bomb, or builds a shipyard and a ship through commands
  (`grep` over `ra1_shipped.rs`, `ra2_shipped.rs`, `ra3_shipped.rs`, `ra2_ai.rs` and
  `crates/game/tests/`). Some of these are covered on the fixtures with rules (`ra2`, `ra3`).
* Probe (behaviour is fine today): `probe_tests_catalogue.rs` builds every item of every
  country's own tree from a yard through `QueueProduction` and `PlaceBuilding` on a map with
  water: 37 or 38 items per country, all nine countries, exact cost, no rejection, 0 problems.
* Expected: R15's "regression: the migrated legacy tests" keeps the old rules tested; the shipped
  rules need the same tests where they differ.
* Minimal fix: add the catalogue test on the shipped data (the probe can be adapted), and a
  shipped test per specialist mechanic that RA4 to RA6 will not rewrite anyway.

## P2

### 6. A "captured" building is set up directly in two tests

`crates/sim/tests/ra1_shipped.rs:967` gives the Allied player a Soviet Barracks with
`spawn_building` (its comment says so) and `crates/sim/tests/ra1_buildability.rs:603` uses `place`;
the plan's items are "a captured Soviet Barracks" and "also after capture", and `Command::Interact`
exists. The fixture test at `ra1_buildability.rs:314` does capture with an engineer. Fix: capture
with the shipped Engineer in the shipped test.

### 7. "Property test over costs and rates" is a fixed loop over one cost

`crates/sim/tests/ra2_production.rs:532` varies 1 to 4 factories and the power, with one item
(400 credits over 10 units, which divides evenly). The plan asks for a property test over costs
and rates. Probe `probe_tests_i7.rs`: all 29 shipped unit types a full base can queue, 1 to 4
factories, with and without power, with exactly the cost (produced, 0 credits left) and one
credit less (never produced, never negative): 232 runs, no problem. Fix: adopt the probe.

### 8. The low power test has no positive control

`crates/sim/tests/ra2_shipped.rs:517`: `assert!(!fired(prism))` would also hold if the Prism Tower
could not fire at that target at all. Probe: with two Power Plants the tower fires in the same
scenario, so the assertion means what it says today. Fix: add the powered run to the test.

### 9. Silent fallbacks in the validation examples

`crates/sim/examples/m14_duels.rs:131` (`rotating(..).unwrap_or(0)`: country 0 instead of an
error) and `crates/sim/examples/m7_soak.rs` (`let (Some(first), Some(second)) = .. else { continue }`:
a configuration silently skipped). Both are unreachable while `pairing_groups` filters empty
groups, and neither example checks its number of jobs against an expected count. Fix: `expect`.

### 10. Stale comments in `m6_production_matches.rs`

Line 83 ("plays the same matches", see finding 4) and line 339 ("the limit of 36,000").

### 11. A test name promises more than it asserts

`crates/sim/tests/ra2_production.rs:611` `a_player_id_outside_the_match_changes_nothing` asserts
the rejection only, not the state hash.

### 12. The smoke flow does not go through the player's input, and compares less than R20 and the README say

`crates/game/src/smoke.rs:1033` (`begin_match` queued directly), `:1120` (`*selection = choice`),
`:1196` (`while speed.step(true) {}`). `skirmish::screen_input`, `sidebar::sidebar_input` and
`special_controls::input` have no automated driver; nothing constructs an `Interaction::Pressed`.
R20 says "the menu's pure functions", which is accurate; `README.md:536` says "by the menu's
buttons ... every row of the lobby changed", and match (a) leaves Game speed, MCV start and MCV
repacks at their defaults. `match_problems` compares neither the game speed nor the difficulty,
and the 4x check at `:1273` is satisfied by line 1196 whatever `begin_match` did with the Game
speed row. The simulation is still changed only through `CommandSink` (D34 holds). Fix: correct
the README sentence; add speed and difficulty to `match_problems`; one test that presses a
`MenuButton` and a `SidebarButton` through the systems.

### 13. Card cost labels are those of the first match of a session

`crates/game/src/sidebar.rs:1068-1070` fills the item list only while it is empty; the label
with `state.production_cost(p.cost)` is written once (`:1136-1139`); the sidebar is spawned at
`Startup` and nothing in `sidebar.rs` despawns the cards. After a match with Super Me every later
match of the session shows `$1`, and the reverse. The code is older than the programme; the
lobby of RA1 makes switching Super Me between matches an ordinary thing to do. Fix: refresh the
label in `update_sidebar`, or clear the list when a match ends.

### 14. The headless end to end matches play without scenery

`sim::Props::from_looks` is installed only by `crates/game/src/sim_host.rs:167` (and two sim
tests). `m6_production_matches.rs` (header: "as a match started from the menu sets them up"),
`m14_duels`, `m7_soak` and `m13_ten_players` insert none, so since RA3 (R35: trees and houses
take damage through the armour table) their matches differ from the game's. Whether outcomes
change was not traced. Fix: install the props in `setup_match_configured`, or say so in the
test's header.

### 15. Decision text against code

R43 item 2 (`docs/DECISIONS.md:2784`): "a box selects every soldier whose ground point lies in
it"; `crates/game/src/selection.rs:580` adds `GROUND_OFFSET_PX` (the body centre), as the README
says.

### 16. The shared snapshot was changed while I worked (process, not the product)

At 23:36 a load of `assets/data/combat.ron` in the snapshot returned an empty file ("expected a
struct, found the end of the file"); the file's change time is 23:36:35 and its content and
modification time are those of the commit again. Another reviewer rewrote and restored it. My
probes read a private copy of the data of `46e694d` after that (`PROBE_DATA`). Any result another
slice obtained from the snapshot around that minute may be affected.

## Suspected, not confirmed

* **S1.** `check_catalogue` (`crates/game/src/skirmish.rs:199`, `:261-268`) tries the menu's
  matches with MCV start on only; with MCV start off a yard that cannot be placed ends the game
  with code 1 (`:1588-1594`). `m7_maps.rs` asserts a buildable 5 x 5 at every filed start; no test
  sweeps yard starts over all maps and player counts with map growth.
* **S2.** `sidebar::factories_of` (`crates/game/src/sidebar.rs:580-596`) counts every building of
  the factory group, the simulation counts working exits (`crates/sim/src/economy.rs:895-937`);
  the hint's time may be wrong while a factory is disabled.
* **S3.** `offline_text` (`crates/game/src/minimap.rs:398-402`) says "low power" for every owner
  of a radar building without radar, also during a spy's outage.
* **S4.** Finding 3's size: whether the unfixed sweep would still exceed 72,000 ticks.

## Checked and found correct

* **No test deleted, ignored or skipped.** 524 `#[test]` functions at `21225b0`, 782 at `46e694d`
  by my count (the logs say 529 and 787); the only names that left are the two renamed ones of
  R40. No `#[ignore]`, no `cfg` on a test, one `should_panic` that predates the range. Every test
  name of the baseline log is in the RA3 log as `ok` except those two.
* **No assertion lost.** Of the tests whose bodies changed (about 75), none has fewer assertions
  than before. `m13_grown_maps.rs` went from `placed >= 2` to `>= 3` (stricter).
* **Legacy fixture.** `fixtures/data/legacy/` equals the data shipped at `21225b0` apart from
  the added schema fields with neutral values (`economy.ron` and `units.ron` identical; all 72
  arsenal entries keep their first ten fields; combat keeps every number). The shared fixtures
  `ai.ron`, `combat.ron`, `skirmish.ron` changed by schema only.
* **Decision records.** R15, R21, R31, R40 and R45 list every changed test I found; R40's change
  of `m4_combat.rs` (limits 30 cells and 16 subcells a tick, 480 ticks) keeps the assertions.
  R29's and R44's tables match the logs (recomputed: 0.15/0.34/0.86, 0.22/0.49/1.49 with 5 over,
  0.11/0.24/0.75, 0.19/0.36/0.76, 0.16/0.33/0.96; 34.6 and 30.3 million ticks).
* **STATUS against the logs.** Test counts 529, 610, 626, 690, 735, 787, all exit codes, the
  benchmark, soak and duel numbers and the smoke lines of RA1 to RA3 agree with the logs. The
  seed difference, the match at 0.96 and the first failed run of RA3 are disclosed.
* **Acceptance criteria and "Tests" items of RA1 to RA3.** Each has a test through
  `CommandQueue` and `SimSchedule` with the data it names, except as listed in findings 1 and 5
  to 8. Direct state writes in the new tests are scenario setup (`spawn_unit`, `spawn_building`,
  hit points, credits).
* **README, rules (about 30 values sampled against the shipped data).** Build times, two
  factories, queue of 29, build limits, radar buildings, placement reaches 2, 4, 8 and 31, base
  buildings, walls, refund, repair, the armour of 20 types, seven splash radii, bursts, minimum
  ranges, the IFV's air bonus, crushers and what cannot be crushed: all as written.
* **Game client (by reading).** Every lobby row reaches `MatchOptions` or `PlayerSetup`; map
  growth is off by default (R4) and R17 matches the code; the sidebar lists and judges cards with
  the simulation's own `check_buildable`, `produces`, `queue_of`; the radar is read from
  `PlayerState::radar`; picking, box selection, hover and health bars use view positions with no
  cell to entity map; no mid-match write to the simulation outside `sim_host.rs`; 14 README
  claims on controls and the lobby match the code.

## Could not check

* Anything that needs the game crate built or run, the long AI matches, the soak and the duels
  (not allowed in this review): the test's own match lengths (finding 4), the size of finding 3,
  whether finding 14 changes outcomes.
* The reference tables against the ini, and the rules of Red Alert 2 as such (other slices).
* `MenuLayout` and `measure_menu` internals; the diffs of `weapon_fx`, `audio` and `notices`
  beyond their assertions.
* Everything visual, audible or a matter of feel.
