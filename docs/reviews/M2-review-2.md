# M2 review 2: Units and movement

## Scope and base

* **Reviewer:** an independent review session. It did not implement or remediate this code and edited no repository file. The working tree was clean before and after the review.
* **Branch:** `milestone/m2` at `2adf66f`.
* **Base:** tag `m1-reviewed` (commit `39d688c`). The cumulative diff is `git diff m1-reviewed..milestone/m2`: 35 files, +7453/−46. It contains:
  * `d690d4c` (sim) and `7ac87d1` (game): the M2 implementation;
  * `a8644e5` and `6d9c332`: the implementation report and review 1;
  * `6af1b4d` (code) and `2adf66f` (documentation): remediation round 1. The remediation alone is `git diff 6d9c332..milestone/m2`: 12 files, +933/−76.
* **Read in full:** PLAN.md (including section 12), docs/STATUS.md, docs/DECISIONS.md, and docs/reviews/M2-review-1.md.
* **Code read in full:**
  * sim: `crates/sim/src/{movement,pathfinding,unit,event,command,data,definitions}.rs`, and the M2 changes to `map.rs`, `hash.rs`, `schedule.rs`, and `lib.rs`;
  * game: `crates/game/src/{units,smoke,selection}.rs`, and the M2 changes to `main.rs`, `map.rs`, and `tileset_image.rs`;
  * tests: all changes to `crates/sim/tests/m2_movement.rs` since review 1, and the test helpers (`scenario`, `tick`, `assert_i6`, `order_swap`).
* **Third-party code that consumes external data:** the M2 RON subset (`data.rs`) was rechecked against `ron` 0.12.2's documented reading rules: whitespace, `//` comments, raw identifiers, integers, attributes, and named structs. Each rule was then exercised through the real binaries (table below). The PNG path for sprite sheets reuses the M1 subset (`check_png_file`), which was exercised again with hostile sheets.
* **Probes:** all ran outside the repository, under `…/scratchpad/m2-review-2-probe/`, with a `perl alarm` timeout:
  * `probe`: a scratch crate that depends on `crates/sim` by path. It was built and run in debug and release.
  * `probe_base`: the same crate built against a `git archive` of the reviewed code `6d9c332`.
  * 38 scratch asset roots, selected with `BEVY_ASSET_ROOT`, run against `target/{debug,release}/game`.

  No probe was killed by the memory watchdog. Every peak footprint of the game binary was at most 0.11 GB.

## Validation commands (PLAN.md section 9 and section 12 item 5)

All six commands ran from the repository root, in the foreground, in this order, at `2adf66f`. A display was available.

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings (rerun after forcing a re-lint of both crates) |
| `cargo test --workspace` | 0 | 181 passed, 0 failed, 0 ignored. `m2_movement` took 27.96 s. Peak footprint 0.13 GB. The known M0 linker warning appeared. |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | `26 units spawned`; `smoke test: 600 frames, 198 simulation ticks, map created: true, units spawned: 26, units moved: 18 in 198 ticks after the order` |
| `cargo run -p game -- --smoke-test 600` | 0 | `smoke test: 600 frames, 203 simulation ticks, map created: true, units spawned: 26, units moved: 18 in 203 ticks after the order` |

Test breakdown for `cargo test --workspace`:

* `game` unit 44.
* `m0_spike` 6, `m1_demo_map` 3, `m1_tileset_image` 6, `m2_unit_sprites` 5.
* `sim` unit 3.
* `m0_bootstrap` 12, `m1_map` 32, `m1_map_subset` 24, `m2_definitions` 14, `m2_movement` 32.
* doc-tests 0.

Additionally, `cargo test --release -p sim --test m2_movement --test m2_definitions` exited 0 (32 and 14 passed). The validation table of the remediation report is confirmed: same counts, same exit codes, and the same smoke log lines. The occasional `Skipped event Destroyed` warning (M0 debt item 2) appeared in both smoke runs.

## Status of the findings of review 1

| Finding | Status | Evidence from this review |
|---------|--------|---------------------------|
| P1-1: move orders can livelock forever | **Resolved** | Reviewer 1's sealed room (`probe room`, the repository's `order_swap` order sequence), release: <ul><li>reviewed code `6d9c332`: both orders still live after 3,000 ticks;</li><li>`2adf66f`: `MoveFailed { Blocked }` at tick 776 and `MoveCompleted { retargeted: true }` at tick 792.</li></ul>The repository's random-map generator over seeds 0–999 (walker and runner; release, 22 s): 0 unsettled within 20,000 ticks, slowest settle 1,033 ticks, 249,011 completions, 398 `Blocked`, I6 held on every tick. This matches the remediation report. With mixed speeds (24, 128, and 3; 300 seeds) and with speed 1 plus 32 (100 seeds), every order also ended (slowest 4,503 and 875 ticks). The three regression tests pass in debug and release. A side effect of the new stall rule is new finding P0-1. |
| P1-2: match setup silently spawns fewer starting units | **Resolved** | Debug and release binaries: <ul><li>the arena fixture as the demo map exits 1 with `starting units of player 0: 6 "scout" units are placed around cell (14, 34), but only 0 free cells are reachable there (the cell is outside the map or walled in)`;</li><li>the 16×16 fixture exits 1 for the 12 light tanks;</li><li>the normal (non-smoke) run on the arena root exits 1 in both profiles;</li><li>a units file without `scout` exits 1 with `starting unit "scout" at (14, 34): unit type "scout" is not defined`.</li></ul>The smoke test requires `starting_unit_total()` = 26 (`smoke.rs:169`). |
| P2-1: `MoveCompleted` for retargeted orders | **Resolved** | `SimEvent::MoveCompleted { unit, retargeted }` (`event.rs:28`). The sealed-room probe reports `retargeted: true` for the unit that stopped short. Test: `a_completion_says_whether_the_unit_stopped_short_of_its_destination`. |
| P2-2: large group orders end with `Blocked` without moving | **Partly resolved; the remainder is recorded as debt** | Ticks spent waiting for the pathfinder no longer count (`movement.rs:146-150`). Test: `ticks_spent_waiting_for_the_pathfinder_do_not_count_as_stalled`. The remainder (crowds of about 1,000 units, and the 16-requests-per-tick throughput) is STATUS remediation debt item 1. |
| P2-3: a right click can use the previous frame's hovered cell | **Resolved** | `selection.rs:35-44` adds `.after(crate::map::update_hovered_cell)`. `game_systems_that_share_state_in_a_frame_are_ordered` passes. |
| P2-4: D19 item 3 states a validation order the code does not follow | **Resolved** | D19 item 3 now describes the per-id order of `command.rs:182-195`. `validation_checks_each_named_unit_in_turn` passes. |

## Verification summary

### Tests removed, skipped, or weakened

None.

* Per-file `#[test]` counts never fall between `m1-reviewed` and `2adf66f`.
* The diff removes no `assert` and adds no `#[ignore]` or `should_panic`.
* The remediation changed existing `MoveCompleted` uses in `m2_movement.rs` in only two ways:
  * seven exact matches now also require `retargeted: false`, and one requires `retargeted: true` (stricter);
  * three `match` arms use `MoveCompleted { unit, .. }`, which keeps their meaning.
* PLAN.md, `rust-toolchain.toml`, the workspace and `game` manifests, and `.cargo/config.toml` are unchanged.

### Scope

The remediation touches only M2 files: sim movement, pathfinding, unit, and event; game units, smoke, selection, and map; the M2 tests; and the documentation. The deviations already recorded for M2 still apply. None is new.

### Required M2 tests and acceptance (PLAN.md section 7)

All required tests are present and pass in debug and release:

* path cost optimal on open and obstructed fixtures;
* unreachable target;
* move outside the map;
* own cell;
* newly placed obstacle;
* I6 with 50 units for 2,000 ticks;
* determinism.

The acceptance scenario `fifty_units_reach_a_clicked_destination_without_overlap_or_deadlock` passes unchanged.

The required tests and the acceptance scenario run only the fixture speeds 32 and 100. No movement test runs a unit at the boundary speeds 1 or 128 (see P0-1).

### Section 8 categories

* **Positive, negative, integration:** present.
* **Regression:** each finding of review 1 has a test. The P1-2 tests name new functions, so the pre-fix evidence is the binary behaviour, which this review reproduced.
* **End to end:** both smoke runs.
* **Boundary:** the data loader tests speed 1 and 128, but movement is never tested at those speeds. The gap hides P0-1.

### Section 5 units and equations

* `SubPos` centre is `c·256 + 128`, found by `div_euclid`.
* Speed is `u16` subcells per tick. With speed 2, 3, and 32, arrival times equal path length ÷ speed: for example, 5 orthogonal cells take 640, 427, and 40 ticks.
* Step lengths are 256 and 362.
* Equation 5 is unchanged: costs 10 and 14, octile heuristic, diagonal rule.
* Equation 7 is used for drawing. No floating point is used in `crates/sim/src` apart from the recorded M1 debt P2-B (`map_subset.rs:524`).

### Invariants

| Invariant | Result |
|-----------|--------|
| I1 | `cargo tree -p sim -e normal` lists `bevy_app`, `bevy_ecs`, `quick-xml`, `rand_chacha`, `ron`, `serde`, and `tiled`, and no forbidden crate. |
| I2 | Holds (see section 5 above). |
| I3, I4 | Per-tick `state_hash` digest of `probe hash` is `7a56fad63db7ef59` in both debug and release. It covers 60 units of five speeds, 3 seeds, 4,000 ticks, and random `Move` and `Stop` orders. The new order fields are hashed (`unit.rs:355-365`). |
| I5 | Holds. The game writes the sim world only in match setup (`setup_units`) and otherwise pushes into `CommandQueue`. |
| I6 | Held on every tick of every probe: 1,400 random scenarios and the corridor and room probes. |
| I8 | Holds for every hostile data file, sprite sheet, and command tried (tables below). No `unwrap`, `expect`, or `panic!` is reachable in non-test M2 code. |
| I9 | Exact pins. `Cargo.lock` changed only in `sim`'s dependency list. |
| I10 | Tests use fixture data. |
| I11 | The RON subset and the PNG subset hold (tables below). |
| I12 | M2 arithmetic is saturating, checked, or bounded. Debug and release digests are identical. |

### Hostile inputs through the real binaries (both profiles, identical results)

Each case below exited 1 with a typed error naming the file, the field, and the reason, with no panic and a peak footprint of at most 0.11 GB.

| Input (`assets/data/units.ron` unless stated) | Error |
|------------------------------------------------|-------|
| `#![enable(implicit_some)]` prefix | `file: line 1, column 1: expected a value or punctuation of the supported subset, found '#'` |
| 70 KB file; 1 GiB sparse file; 60,000 nested `[` beyond 64 KiB | `file: the file is larger than 65536 bytes` |
| 60,000 nested `[` within 64 KiB | `units[0]: line 1, column 29: expected a struct "(", found '['` |
| FIFO; symlink to `/dev/zero`; directory | `file: cannot read the file: not a regular file` (no hang) |
| `ö` in a string | `file: line 16, column 20: … found byte 0xc3` |
| empty file; `units: []`; 65 entries; 65-byte id | precise reason in each case |
| `../../maps/demo/demo_tiles` as sprite; duplicate id; unknown field `hp`; missing `speed` | precise field and reason in each case |
| speed `-0`; `9223372036854775808`; `40.0` | `value 0 is not supported …`; `… expected a 64-bit signed integer`; `"40" followed by '.'` |
| `r#speed`; `/* x */`; CR-only line endings; `schema_version: 2`; named struct `UnitsFile(` | precise reason in each case |
| `scout.png`: missing; FIFO; `/dev/zero` symlink; 300 MiB `iCCP` bomb; 512×32 image | `units[1].sprite: sprite image …/scout.png: …` with the specific reason (`chunk "iCCP": chunk is not supported`, `the image is 512x32 px, but a sprite sheet is 512x64 px`, …) |

These cases exited 0 and were correctly accepted:

* CRLF line endings;
* a malformed `scout.png.meta` (never read);
* two unit types sharing one sprite;
* speed 128 for both types.

Speed 1 for both types also exited 0, but every ordered unit ended with `Blocked` (P0-1).

## Findings

### P0-1: A unit of speed 1, the documented minimum, fails every move longer than one cell with `MoveFailed { Blocked }` on an empty map. This is a regression introduced by remediation round 1.

**Files and lines:**

* `crates/sim/src/movement.rs:143-159`: `stalled_ticks` increments on every tick in which no step was started and the path state is `Following`, including every tick spent in the middle of a step along a path to the destination. When it reaches `MAX_STALLED_TICKS` (line 55, 200 ticks) and the unit stands at a cell centre, the order ends with `MoveFailure::Blocked`.
* A step lasts `ORTHOGONAL_STEP / speed` = 256 ticks or `DIAGONAL_STEP / speed` = 362 ticks at speed 1 (`unit.rs:20, 25`). This is longer than the 200-tick limit, so the counter passes the limit during an unobstructed step.
* At speed 1 the budget is spent exactly at the cell centre. `at_centre` then returns `Wait` (`movement.rs:229-231`), the unit stands at a centre, and the order ends.
* Speed 1 is valid data: `definitions.rs:176` accepts 1 to `MAX_UNIT_SPEED`, and D18 states "`speed` is 1 to `MAX_UNIT_SPEED` = 128".
* `event.rs:42-45` defines `Blocked` as "Other units kept the way blocked". No other unit exists in the reproduction.
* DECISIONS D21 item 1 (docs/DECISIONS.md:296-299) states the end rule as "ticks without a step along a path that leads to the destination" and, in the bound, that "a step takes at most 362 ticks". It does not reconcile the 362-tick step with the 200-tick limit. Remediation class 1 in STATUS claims that "every way an order can stay alive was enumerated", but time spent inside a step was not.
* The reviewed code `6d9c332` reset the counter on any advance, so the defect is new in `6af1b4d`.

**Evidence:**

* `probe speed`: one unit alone on the open 16×16 fixture at (1, 1), with no other unit and no rock in the way. Debug and release output is identical (diffed).

  | Speed | Order | Result at `2adf66f` |
  |-------|-------|---------------------|
  | 1 | (+1, 0) | `MoveCompleted { retargeted: false }` at tick 256 |
  | 1 | (+2, 0), (+5, 0) | `MoveFailed { reason: Blocked }` at tick 256, unit at (2, 1) |
  | 1 | (+3, +3), (+10, +7) | `MoveFailed { reason: Blocked }` at tick 362, unit at (2, 2) |
  | 2, 3, 32 | all six orders | `MoveCompleted` at the expected times (for example, speed 2 to (+10, +7) at tick 1,651) |

* The same probe against the reviewed code (`probe_base`, `git archive 6d9c332`) completes every speed-1 order: (+2, 0) at tick 512, (+5, 0) at 1,280, (+3, +3) at 1,086, (+10, +7) at 3,302.
* **Production path:** a scratch asset root whose `units.ron` sets both unit types to speed 1 (a file the loader accepts) was run with `--smoke-test 1300` in debug and release.
  * Every one of the 18 ordered player units logged `unit N stopped: Blocked` (18 lines in each profile).
  * The smoke test still exited 0 (`units moved: 11 in 431 ticks after the order`), because it checks only that some unit moved.
* **Test gap:** the fixture unit types have speeds 32 and 100 (`crates/sim/tests/fixtures/data/units.ron`). `m2_definitions` loads speeds 1 and 128, but no movement test runs a unit at either boundary speed. This is the PLAN.md section 8 boundary category ("zero and maximum quantities").

**Severity:** P0 under PLAN.md section 10 ("incorrect simulation result"). A unit with valid data, on an open map, without any obstruction, has its order cancelled and is reported as blocked by other units. The shipped `assets/data/units.ron` uses speeds 24 and 40, so the demo match is not affected. The simulation's behaviour on accepted data is still wrong, and the defect is a regression of the remediation. The same mechanism also makes any unit that follows a speed-1 unit in a one-cell corridor give up while the leader advances, because the follower waits 256 ticks for each cell.

**Expected behaviour:**

* An order never ends with `Blocked` while its unit is in the middle of a step along a path to its destination, and a unit that advances unobstructed always completes. For example:
  * do not count ticks during which a step along a path to the destination is in progress; or
  * reset the counter at the end of such a step as well as at its start; or
  * make `MAX_STALLED_TICKS` exceed the longest step (362 ticks at speed 1) and re-derive the D21 bound.
* Regression tests through `SimPlugin` and `CommandQueue`:
  * a speed-1 unit crosses an open map orthogonally and diagonally and receives `MoveCompleted { retargeted: false }`;
  * movement at the boundary speeds 1 and `MAX_UNIT_SPEED` = 128 is covered, for example by adding fixture unit types with these speeds.

  The tests must fail against `2adf66f` and pass after the fix.
* D21 item 1, the `MAX_STALLED_TICKS` and `MoveOrder::stalled_ticks` documentation, and the remediation report's "every way an order can stay alive was enumerated" are corrected to match.

### P1

None.

### P2

None new. The P2 items carried as debt are listed below.

## Requires user verification

These items are not findings (PLAN.md section 12 item 2). They do not affect the decision and block only `m2-approved` (and `m1-approved`). Run `cargo run --release -p game` and pan to the left of the rock ridge.

1. **Unit rendering:**
   * 18 blue-tinted player units around cells (10, 28) and (14, 34): 12 light tanks with turret and barrel, and 6 smaller wedge-shaped scouts.
   * 8 red-tinted opponent units around (50, 30) and (52, 36).
   * Each unit stands centred on its tile, and units lower on the screen are drawn in front.
2. **Facings:** a moving unit faces its direction of travel in all eight directions, and keeps its facing when it stops.
3. **Smooth movement:** units glide between cells without jumps or jitter at any zoom.
4. **Click selection:** a left click on an own unit draws a green diamond under it. A click on empty ground clears the selection. Opponent units cannot be selected.
5. **Box selection:** a left drag draws a green rectangle and selects the own units inside it on release.
6. **Additive selection:** Shift adds clicks and boxes to the selection.
7. **Orders:**
   * A right click sends the selection to distinct cells around the clicked cell, through the ridge gap and around the lake, without overlap or getting stuck.
   * A click on rock or water uses the nearest cells around it.
   * `S` stops the selection at the next cell centre.
8. **M1 items** (still pending for `m1-approved`): map rendering, picking, panning, zoom, and bounds, as listed in docs/STATUS.md.

## Recorded debt that must be carried in docs/STATUS.md

* **From M2 review 1, still open:**
  * P2-2 remainder: crowds of about 1,000 units in one command can still end with `Blocked` without moving, and path throughput is 16 requests per tick. This is remediation round 1 debt item 1, to be measured against the M7 benchmark.
* **From the M2 remediation report, confirmed by this review:**
  * Debt item 2: orders that cannot get closer end after about 800 ticks.
  * A unit following a slower unit through a one-cell corridor waits behind an advancing unit, and that wait counts as stalled. This is part of remediation debt item 1. It becomes visible only at very low speeds, which P0-1 must address.
* **M2 implementation debt items 1 to 6, as confirmed by review 1:**
  1. Holes in group arrival.
  2. The two-tick `MessageReader` window for the event log and the smoke test's rejection check.
  3. Selection, order, and drawing systems are tested through pure functions only.
  4. Path searches allocate arrays of the map's size.
  5. `Grid::set_terrain` and `spawn_unit` are public.
  6. No automated screenshot.
* **Carried from M1 and M0, unchanged by M2:**
  * M1 review 3: P2-1 for the map and `.tsx` reader in `sim` (FIFO), P2-2, and the remaining items of P2-3;
  * M1 review 1: P2-3 and P2-5;
  * M1 review 2: P2-B;
  * M1 round 1: item 4;
  * M1 round 2: items 5 and 6;
  * M0: items 1, 2 (the `Skipped event Destroyed` warning was seen again in this review's smoke runs), 3, 5, 7, 8, and 9.
* **Resolved and confirmed:**
  * M2 review 1 P1-1, P1-2, P2-1, P2-3, and P2-4;
  * the pathfinder-queue part of P2-2;
  * M1 review 3 P2-1 for tileset images and sprite sheets (FIFO, `/dev/zero`, and directory rejected without blocking, in both profiles).

## Decision rationale

All six validation commands pass. No test was removed or weakened, and the required M2 tests and the acceptance scenario pass. Both P1 findings of review 1 are resolved, as confirmed by this review's own probes in both profiles, and so are P2-1, P2-3, and P2-4.

However, the remediation introduced an incorrect simulation result for valid data: a unit of the documented minimum speed is reported `Blocked` and stops on an empty map (P0-1). No boundary-speed movement test exists to catch it. PLAN.md section 10 makes NO-GO mandatory for any unresolved P0.

Decision: NO-GO
