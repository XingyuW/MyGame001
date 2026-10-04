# M3 review 1: Economy and construction

Independent review. The reviewer did not write or remediate any of this code and edited no repository file.

## Scope and base

* **Repository:** `/Users/xingyuwang/GameProjects/MyGame001`, branch `milestone/m3`, HEAD `9aab887`, with a clean working tree.
* **Base:** tag `m2-reviewed`, the most recent reviewed or approved tag (PLAN.md section 12 item 1).
* **Diff reviewed:** `git diff m2-reviewed..milestone/m3`. It spans 41 files, with +8,113 and −89 lines, in three commits:
  * `0f9507d`: sim
  * `f460221`: game
  * `9aab887`: docs
* **Documents read:**
  * PLAN.md in full, including section 12;
  * docs/STATUS.md in full, including the "M3 implementation report";
  * docs/DECISIONS.md in full, including A1 follow-up and D23 to D27.
* **Code read in full:**
  * `crates/sim/src/{economy,economy_commands,economy_defs,building,harvest}.rs`;
  * the M3 diffs of `command.rs`, `event.rs`, `map.rs`, `unit.rs`, `hash.rs`, `definitions.rs`, `data.rs`, `pathfinding.rs`, `schedule.rs`, and `lib.rs`;
  * `crates/game/src/{buildings,construction}.rs`;
  * the M3 diffs of `units.rs`, `selection.rs`, `smoke.rs`, `main.rs`, and `examples/gen_unit_sprites.rs`;
  * `crates/sim/tests/m3_economy.rs` and `m3_harvest.rs` in full;
  * the main tests of `m3_definitions.rs`;
  * `assets/data/economy.ron`, the M3 fixtures, and `assets/LICENSES.md`.
* **Implementer's report:** treated as a set of claims and checked against the code and against my own runs.
* **Probe locations:** every probe ran outside the repository, under `…/scratchpad/m3-review-1-probe/`:
  * a probe crate, `probe/`, with a path dependency on the repository's `sim`;
  * 44 scratch asset roots, under `roots/`;
  * an archive of `m2-reviewed`, in `base_m2/`.
* **Probe conditions:** every probe ran in the foreground under a `perl alarm` timeout. The watchdog killed nothing, and no probe exited 137.

## Validation commands (PLAN.md section 9, with section 12 item 5)

I ran these from the repository root, in this order, in the foreground, on HEAD `9aab887`. A display was available.

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 248 passed, 0 failed, 0 ignored (breakdown below). 49.6 s wall time; the whole run peaked at 0.13 GB. Only the known M0 linker warning appeared. |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | `2 buildings spawned`, `26 units spawned`; `smoke test: 600 frames, 198 simulation ticks, map created: true, units spawned: 26, units moved: 18 in 198 ticks after the order, buildings spawned: 2, production charged 396 credits in 198 ticks` |
| `cargo run -p game -- --smoke-test 600` | 0 | `… 203 simulation ticks … buildings spawned: 2, production charged 406 credits in 203 ticks` |

Test breakdown for `cargo test --workspace`:

* `game` unit tests: 58.
* `game` integration tests: `m0_spike` 6, `m1_demo_map` 3, `m1_tileset_image` 6, `m2_unit_sprites` 5, `m3_building_sprites` 3.
* `sim` unit tests: 6.
* `sim` integration tests: `m0_bootstrap` 12, `m1_map` 32, `m1_map_subset` 24, `m2_definitions` 14, `m2_movement` 37, `m3_definitions` 11, `m3_economy` 21, `m3_harvest` 10.
* Doc tests: 0.

These counts match the implementer's report.

* **Charge per tick in the smoke runs:** the smoke test queues the construction yard (3000 credits over 1200 ticks). Equation 3 then charges ⌊3000/1200⌋ = 2 credits per tick, which matches 396 credits in 198 ticks.
* **Shutdown warning:** the M0 debt item 2 warning (`Skipped event Destroyed`) appeared in both smoke runs.

Additional runs:

* **M3 tests in release.** `cargo test --release -p sim --test m3_economy --test m3_harvest --test m3_definitions` passed: 21, 10, and 11 tests.
* **M2 movement tests, base against head.** Release, single-threaded, three runs each: 2.54–2.55 s at `m2-reviewed` and 2.64–2.77 s at HEAD.
  * The cost of the new `Economy` stage and the building check in `is_passable` is about 5 %.
  * The report's 31.5 → 36.8 s is a debug-build figure and is not a meaningful regression.

## Verification of the milestone (PLAN.md section 7, M3)

| Item | Verdict | Evidence |
|------|---------|----------|
| Scope: players, credits, power | Met | `Players` and `PlayerState`. `update_power` is the first system of `Economy`. Tests: `power_follows_the_buildings_placed_and_sold`, and the boundary case in `low_power_halves_progress` (consumption equal to production is not low power). |
| Scope: construction yard; building and unit production queues | Met | One queue per producing type; only the head progresses. Tests: `every_item_of_the_fixture_is_charged_exactly_its_cost`, `production_needs_a_producer_and_continues_with_another`, `invalid_queue_commands_are_rejected_without_changing_state`. |
| Scope: placement rules | Met | `check_footprint`, then `check_distance`, in the order D24 item 3 documents (code read). Tests are listed below. |
| Scope: ore, harvester cycle, refinery | Met | `harvest.rs` (D26). `harvesters_share_the_field_and_every_unit_of_ore_is_paid_once` asserts ore conservation and the credit ledger on every tick. |
| Scope: rally points, selling | Met | `a_produced_unit_leaves_its_building_and_moves_to_the_rally_point`, `rally_point_commands_are_validated`, `selling_refunds_the_sell_value_and_frees_the_cells`, and both same-tick tests |
| Test: I7 for completion | Met | `a_completed_item_is_charged_exactly_its_cost` covers 10×9 + 15, build time 1, cost below the build time, and cost 0. Also `every_item_of_the_fixture_…`, and the unit test of `charge_for_tick` at the limits. |
| Test: I7 pause on insufficient credits | Met | `progress_pauses_while_credits_are_insufficient`: a pause at 9 of 10, 0 credits, and credits exactly equal to the charge |
| Test: I7 cancel refund | Met | `cancelling_refunds_exactly_what_was_charged`: an item in progress, one not started, a complete one, and one not queued (rejected, with the hash equal to a control run) |
| Test: low power halves progress | Met | Progress happens only on even ticks: 19 or 20 ticks against 10, still charged exactly the cost |
| Test: placement rejections (overlap, off map, non-buildable terrain, excessive distance) | Met | `placement_rejections_change_no_state` covers 12 cases, including `i32` extremes, and compares the state hash with a control run over 10 ticks. Also `placement_is_rejected_on_cells_held_by_units`, `placement_needs_a_ready_building_and_an_owned_building_nearby`, and an accepted case at exactly the distance. |
| Test: a harvester with no reachable ore idles | Met | `a_harvester_with_no_reachable_ore_idles`: harvesters sealed in rock and on open ground with the ore sealed; 500 quiet ticks; re-idle after a `Harvest` command |
| Test: a refinery destroyed during the return trip leads to a retarget or idle state | Met, through selling | Two tests: the harvester turns to the other refinery and delivers there, or idles keeping its load. Destruction arrives in M4; the logic checks only whether the refinery still exists. |
| Test: an ore cell depletes exactly to zero | Met | `an_ore_cell_depletes_exactly_to_zero`: per-tick loads 3…18, 20, then 2; the cell ends at exactly 0 |
| Test: integration scenario | Met | `a_base_is_built_and_funded_from_ore_through_commands_alone`: the starting credits pay exactly for the first two buildings, so the barracks is paid with ore. Income is positive after it, the ledger balances, and the hash trace repeats. |
| Acceptance: a base can be built and funded from ore through commands alone | Met | The test above uses fixture data. The reviewer's probe (`probe base 8000`) uses the **production** data and the demo map, through commands only. The power plant was placed at tick 200, the refinery at 606, and the barracks at 847. The first delivery came at tick 723. Income was 13,300 by tick 8000, and credits never fell below 2,584. On-screen behaviour requires user verification. |

Section 8 test categories:

* **Positive, negative, boundary, and integration:** covered as listed above.
* **End to end:** both smoke runs. The smoke test now queues production through `CommandQueue` in the real binary and requires that it is charged.
* **Regression:** not applicable in round 1.
* **Test integrity:** no test was removed, skipped, or weakened. I checked every removed line in the test and example diffs:
  * `smoke.rs` changed one failure message, and the test that covers it counts failures, so it is unaffected;
  * `gen_unit_sprites.rs` keeps the tank and scout geometry, which `m2_unit_sprites` confirms byte for byte.

## Invariants and equations

| Item | Verdict | Evidence |
|------|---------|----------|
| I1 | Holds | No manifest or lock change. `cargo tree -p sim -e normal` lists no forbidden crate. |
| I2 | Holds | `grep` finds no `f32` or `f64` in the new `sim` sources. |
| I3 | Holds | Repeated traces in the tests. The reviewer's fuzz probe gave identical per-tick hash digests in debug and release builds: `b0f68b534ac2a0cc` for 40 seeds × 3000 ticks on the fixture, and `1cbb14a8a6f9dfb4` for 6 seeds × 4000 ticks on the demo map with production data. |
| I4 | Holds | Players, queues, refineries, and buildings use `BTreeMap` or are sorted by `SimId`. The only unordered sum, of power, cannot saturate. There is no `HashMap`, `Instant`, or wall-clock use in `sim`. |
| I5 | Holds | The game writes `Players` and buildings only during match setup before the first tick (`setup_economy`). The construction keys and right-click harvest push only to `CommandQueue` (grep: no `ResMut` of simulation state in `game`). |
| I6 | Holds | Covered cells are impassable (`Grid::is_passable`), `claim` refuses them, and placement requires that no unit holds a cell. Every M3 test checks I6 after every tick. The reviewer's fuzz checked I6 and the grid's building layer after every tick of 300 seeds × 3000 ticks (fixture) and 40 seeds × 6000 ticks (production data). No violation was found. |
| I7 | Holds | The reviewer's fuzz checked these properties after every tick:<br>• credits ≥ 0;<br>• every queue item's `charged` equals equation 3's sum over its progress;<br>• a full ledger: Δ(credits + charged) = sales + deliveries − costs of placed buildings − costs of produced units, with free units told apart from produced ones.<br>It also checked that ore removed from the map equals the change in load plus the ore delivered. No mismatch was found in any run. |
| I8 and I11 | Hold | The economy file passes the D18 RON subset before `ron`. Building and harvester sprites pass the PNG subset before the renderer. 44 hostile roots were run with both binaries (below). |
| I9 | Holds | No dependency change |
| I10 | Holds for `sim`, with one recorded deviation in `game` | The tests use fixture data only. Starting credits are a game constant (P2-4). |
| I12 | Holds | Footprints are computed in `i64`, and credits, payments, and power use saturating operations. The plain `-=` and `+=` operations (charge, progress, attempts) cannot overflow given their guards. Debug and release digests are identical. |
| Equation 3 | Correct | `charge_for_tick` charges `C/T` per tick and `C/T + C mod T` on the last tick, which is the only reading under which the total equals C. There is no charge when credits are insufficient, and progress happens only on even ticks under low power. |
| Equation 4 | Correct | `take_ore` removes `min(max, ore)`, with `max = min(rate, room)`. A refinery pays `load × credits_per_ore`, saturating. |
| Units of section 5 | Correct | Ticks are `u64`, credits `i64`, and power `i32`, and build times are in ticks. |

## Hostile inputs and runtime behaviour

Both the debug and release `game` binaries ran `--smoke-test 120` against 44 scratch roots, selected with `BEVY_ASSET_ROOT` and run under a 60 s alarm. Every hostile root exited with code 1 and a typed error naming the file, the field, and the reason. No root panicked, hung, or used more than 0.12 GB peak memory.

Economy file faults:

* The file is missing, a named pipe, a symlink to `/dev/zero`, a directory, or a 1 GiB sparse file.
* It contains 100,000 opening brackets, or 30,841 nested brackets or parentheses inside a list within 64 KiB.
* It starts with `#![enable(implicit_some)]` or a byte order mark, contains non-ASCII text, or has trailing content.
* An integer is a float, beyond `i64`, or equal to `i64::MIN`.
* A field is unknown, missing, or duplicated, or the schema version is 2.
* `harvest_rate` exceeds the capacity, `sell_value` exceeds the cost, or `free_units` has 9 entries.
* A free unit or producer is unknown.
* The prerequisites form a cycle, including one of length 1.
* A sprite name is `../../data/units`.

Sprite and setup faults:

* The yard is resized to 8×8 (wrong sprite size).
* A building sprite is a named pipe, a `/dev/zero` symlink, missing, or of the wrong size. The harvester sprite is missing.
* The units file lacks the harvester that the economy file references.
* The starting building type is renamed (`starting building "construction_yard" of player 0 at (15, 37): building type "construction_yard" is not defined`).

The same faults in a normal, non-smoke run also exit 1 in both profiles, so none of them is a silent fallback.

Valid variants exit 0 in both profiles: CRLF line endings, tabs, comments, extreme valid values (capacity and rate 100,000, 1,000 credits per ore, distance 32), 8 free units, no refinery, low power, and placement distance 1.

The third-party consumers of external data in M3 are the same as in M2: `ron` and `serde` for the economy file (D18 subset, recursion limit 16 against a schema depth of 5) and the PNG decoder for sprites (D16 subset). No new third-party consumer was added.

Simulation fuzz (`probe fuzz`) ran random commands of all nine variants with hostile values:

* `i32` extremes, unknown, foreign, and unit ids given to building commands, unknown kinds, and players 2 and 3;
* same-tick place, sell, and rally combinations;
* every rejection reason was produced.

It ran 300 seeds × 3000 ticks on the fixture (27 s, release, 3.9 MB peak) and 40 seeds × 6000 ticks on the demo map with production data. There was no panic and no invariant failure.

## Findings

No P0 and no P1 findings.

### P2-1: A building's pending free unit is silently lost when the building is sold before the unit leaves

* **Files and lines:**
  * `crates/sim/src/economy_commands.rs:253-271` (`SellBuilding` despawns the building with its `pending_units`);
  * `crates/sim/src/economy.rs:389-400` (pending units leave only from buildings that still exist).
* **Evidence:** `probe pending`, release, fixture data.
  * A refinery was placed while all 14 passable cells of its ring were held by walkers. The placement tick emitted only `BuildingPlaced`, and the refinery kept `pending [UnitKind(2)]`.
  * It was then sold. The sale tick emitted only `BuildingSold { refund: 100 }`.
  * 200 more ticks produced no event, there were 0 harvesters on the map, and the credits were 900. Nothing reports that the harvester was lost.
* **Context:** the implementer recorded this behaviour as known debt item 1. It needs every ring cell held at placement time, and the state stays consistent (the hash covers `pending_units`). It is therefore not a P1 silent fallback of an error path; it is an undocumented-in-events loss of game state.
* **Expected behaviour:** either the free units are refunded or delivered, or `BuildingSold` (or a separate event) reports the units that were lost. A test should pin the chosen behaviour.

### P2-2: Dock cells are not claimed, so crowded refineries make harvesters stop as `Blocked` with ore that is never delivered

* **Files and lines:**
  * `crates/sim/src/harvest.rs:210-232` (`nearest_dock` picks the nearest free ring cell with no claim, unlike ore cells, which `claimed_ore` reserves);
  * `crates/sim/src/harvest.rs:194-205` (every set-out counts toward `MAX_HARVEST_ATTEMPTS`).
* **Evidence:** `probe crowd N 20000`, release. The setup: the production data, one refinery at (18, 39), N harvesters next to the ore field around (22, 44), which holds 35,500 ore.

  | N | Stopped `Blocked` | Ore carried by stopped harvesters, never delivered | Mean / worst tick |
  |---|-------------------|-----------------------------------------------------|-------------------|
  | 1 to 12 | 0 | 0 (except N = 2, still working at the end) | ≤ 1.7 µs / 0.73 ms |
  | 15 | 1 | 100 | 2.0 µs / 0.41 ms |
  | 20 | 6 | 2,700 | 3.2 µs / 0.49 ms |
  | 60 | 46 | 24,500 | 14.7 µs / 4.2 ms |

  Under known debt item 2, a stopped harvester never resumes without a `Harvest` command.
* **Severity rationale:**
  * The event is typed (`HarvesterIdle { Blocked }`).
  * Every order ends.
  * The effect starts only at about 15 harvesters per refinery, far above normal play.
  * The behaviour matches the documented bounded-attempts rule (D26 item 6).
* **Expected behaviour:** record the measurement in docs/STATUS.md. When the harvester cycle is next revised (at the latest with the M7 benchmark), claim dock cells as ore cells are claimed, or do not count a set-out that only waits for a dock. Add a test with more harvesters than ring cells.

### P2-3: DECISIONS cites a test that does not exist

* **File and line:** `docs/DECISIONS.md:358` (A1 follow-up of M3) cites `m3_economy::two_placements_in_one_tick_see_each_other`.
* **Evidence:** `grep -rn two_placements_in_one_tick_see_each_other crates docs` finds only that line. The test that observes the effect is `later_commands_of_a_tick_see_the_buildings_placed_before_them` (`crates/sim/tests/m3_economy.rs:995`).
* **Expected behaviour:** the decision cites the existing test.

### P2-4: A balance value lives in game code

* **File and line:** `crates/game/src/buildings.rs:41` (`STARTING_CREDITS = 5_000`, with the starting yard positions next to it).
* **Evidence:** invariant I10 says that balance values live only in data files. The implementer records this as deviation 5 and D27 item 3 ("match setup in game code until M6"), following the precedent of M2's starting units (D20 item 4). `sim` and its tests are unaffected.
* **Expected behaviour:** carry this as debt, and move starting credits (and starting buildings and units) to data with the match setup of M6.

## Requires user verification

These are not findings and do not affect the decision (PLAN.md section 12 item 2). They block only `m3-approved`, and the M2 and M1 items also block `m2-approved` and `m1-approved`.

Run `cargo run --release -p game` and check the ten M3 items listed in docs/STATUS.md under the M3 implementation report:

1. **Building sprites:** each sprite sits on its footprint (blue for the human's yard at (15..17, 37..39), red for the opponent's at (52..54, 42..44)), and units walk around buildings.
2. **Draw order:** units are drawn correctly against buildings (debt item 6).
3. **Window title:** it reads `MyGame001 - credits 5000 - power 0/20`, shows `(low)` under low power, and updates.
4. **Production:** `2` queues a power plant. Credits fall by 1 per tick for 10 s and by 101 on the last tick. Shift+`2` cancels with a refund.
5. **Placement:** a yellow footprint outline follows the cursor, and `B` places within 4 cells. After a power plant the title shows power 0/120.
6. **Harvesting:** with a refinery placed near the ore, its harvester harvests for about 5 s, returns, and adds 700 credits per delivery.
7. **Unit production and rally points:** `4` and `5` build a barracks and a war factory, and `6`, `7`, and `8` build units, which leave their buildings. `R` sets a rally point.
8. **Selling:** `Delete` or `Backspace` sells and refunds half the cost, and the cells become walkable.
9. **Harvest order:** a right click on ore with a harvester selected makes it harvest there, and other selected units move there.
10. **Low power:** at 130 consumed against 120 produced, the title shows `(low)` and production takes about twice as long.

The seven M2 items and the five M1 items listed in docs/STATUS.md remain pending.

## Recorded debt that must be carried in docs/STATUS.md

**New in this review:**

* **P2-1:** pending free units are lost on sale without an event.
* **P2-2:** dock contention. With 15 or more harvesters per refinery, harvesters stop as `Blocked` and keep their ore (measurements above).
* **P2-3:** the test cited in DECISIONS.md:358 does not exist.
* **P2-4:** `STARTING_CREDITS` is a balance value in game code until M6.

**From the M3 implementation report (known debt), confirmed by this review:**

1. Units wait to leave while every ring cell is held, and a free unit of a building sold before it leaves is lost (now P2-1).
2. A stopped harvester (`NoOre`, `NoRefinery`, `Blocked`) does not resume by itself. Its consequence under crowding is P2-2.
3. `Harvest` chooses the ore cell nearest to the target without checking that the harvester can reach it, so the move then fails with `Unreachable`.
4. Ore and dock searches each run a full Dijkstra, and production rebuilds the building list every tick. To be measured against the M7 benchmark. This review measured a mean tick of 15 µs and a worst tick of 4.2 ms with 60 harvesters on the demo map. The M2 movement tests are about 5 % slower in release.
5. The interim controls give no validity feedback before a placement.
6. Building draw order uses the footprint's centre.
7. `Players::add` and `spawn_building` are public setup APIs.
8. The window title is the only display of credits and power until M5.
9. The earlier debt the report lists as unchanged.

**Carried from M2, unchanged by M3:**

* M2 review 3:
  * P2-1: the speed dependence of the end rule in jams;
  * P2-2: a box selection released outside the window.
* M2 remediation round 2, items 1 to 7:
  1. path throughput;
  2. the time to end a livelock;
  3. the cost of the chain walk;
  4. no overtaking;
  5. waiting under continuous orders;
  6. the weak smoke move check;
  7. M2 implementation debt items 1 to 6, including the two-tick `MessageReader` window, which now also applies to the smoke test's production rejection check.
* M2 review 1: the remainder of P2-2.

**Carried from M1 and M0, unchanged by M3:**

* M1 review 3:
  * P2-1, the FIFO hang of the map and `.tsx` reader in `sim` (the data and sprite readers were confirmed in this review to reject FIFOs);
  * P2-2;
  * the remaining items of P2-3.
* M1 review 1: P2-3 and P2-5.
* M1 review 2: P2-B.
* M1 round 1: item 4.
* M1 round 2: items 5 and 6. Item 6, registering every asset in `CheckedAssets`, was followed for the building and harvester sprites.
* M0: items 1, 2, 3, 5, 7, 8, and 9. The `Skipped event Destroyed` warning of item 2 appeared again in this review's smoke runs.

## Decision

* No P0 or P1 findings.
* All six validation commands exit 0.
* Every M3 acceptance criterion and required test is met with evidence.
* No invariant is violated.
* There are no unresolved numerical or data-integrity risks, and no broken compatibility: the units schema is unchanged, and the new economy file has schema 1.
* The four P2 items above must be recorded in docs/STATUS.md.

Decision: GO WITH DOCUMENTED NON-BLOCKING DEBT
