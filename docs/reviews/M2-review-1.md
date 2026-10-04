# M2 review 1: Units and movement

## Scope and base

* **Reviewer:** an independent review session. It did not implement or remediate this code and edited no repository file.
* **Branch:** `milestone/m2` at `a8644e5`. The working tree was clean before and after the review.
* **Base:** tag `m1-reviewed` (`39d688c`). The cumulative diff is `git diff m1-reviewed..milestone/m2`: 34 files, +6248/−45. It contains commits `d690d4c` (sim), `7ac87d1` (game), and `a8644e5` (documentation).
* **Read in full:** PLAN.md (including section 12), docs/STATUS.md, and docs/DECISIONS.md.
* **Code read in full:** `crates/sim/src/{command,data,definitions,event,movement,pathfinding,unit}.rs`, the M2 changes to `crates/sim/src/{map,hash,lib,schedule}.rs` and `Cargo.toml`, `crates/game/src/{units,selection}.rs`, the M2 changes to `crates/game/src/{smoke,tileset_image,main,map}.rs`, `crates/sim/tests/m2_movement.rs`, and the structure and mutation test of `crates/sim/tests/m2_definitions.rs`.
* **Third-party code that consumes external data, read at the points the M2 subset depends on:**
  * `ron` 0.12.2: `parse.rs` whitespace set, `skip_ws`, `skip_comment`, integer parsing with `checked_add`/`checked_sub` accumulation, base prefixes, and type suffixes.
  * The PNG path (`image`/`png` through `check_png_file`) was reviewed in M1. It is exercised here with new hostile sprite sheets.
* **Probes:** every probe ran outside the repository, under `…/scratchpad/m2-review-1-probe/`, with a `perl alarm` timeout.
  * Asset-root copies selected with `BEVY_ASSET_ROOT` were run against `target/{debug,release}/game --smoke-test 150`.
  * A scratch crate, `simprobe`, depends on `crates/sim` by path. It was built and run in both profiles.
  * No probe was killed by the memory watchdog. Every peak footprint was at most 0.16 GB.

## Validation commands (PLAN.md section 9 and section 12 item 5)

All six commands ran from the repository root, in the foreground, in this order. A display was available.

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 170 passed, 0 failed, 0 ignored (see breakdown below). The known M0 linker warning appeared. |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | `26 units spawned`; `smoke test: 600 frames, 198 simulation ticks, map created: true, units spawned: 26, units moved: 18 in 198 ticks after the order` |
| `cargo run -p game -- --smoke-test 600` | 0 | `smoke test: 600 frames, 203 simulation ticks, map created: true, units spawned: 26, units moved: 18 in 203 ticks after the order` |

Test breakdown for `cargo test --workspace`:

* `game` unit 39.
* `m0_spike` 6, `m1_demo_map` 3, `m1_tileset_image` 6, `m2_unit_sprites` 5.
* `sim` unit 3.
* `m0_bootstrap` 12, `m1_map` 32, `m1_map_subset` 24, `m2_definitions` 14, `m2_movement` 26.
* doc-tests 0.

Additionally, `cargo test --release -p sim --test m2_movement --test m2_definitions` exited 0 (26 and 14 passed).

The implementer's validation table in STATUS is confirmed: the same counts and the same exit codes.

## Verification summary

### Tests removed, skipped, or weakened

None. The diff removes no `#[test]`, `assert`, or test function, and adds no `#[ignore]` or `should_panic`. No M0 or M1 test file is modified. PLAN.md, `rust-toolchain.toml`, the root and `game` manifests, and `.cargo/config.toml` are unchanged.

### Scope

All changes are within M2 scope (units from RON, spawning, selection, Move and Stop, A* with a budget, reservation, group destinations, repathing, facings, and placeholder sprites). There are two exceptions, both documented:

* The image reader now rejects non-regular files (STATUS deviation 8).
* `S` is used as a stop hotkey (D20).

### Required M2 tests (PLAN.md section 7)

All required tests are present and pass:

* path cost optimal on open and obstructed fixtures;
* unreachable target;
* move outside the map;
* own cell;
* newly placed obstacle;
* I6 with 50 units for 2,000 ticks;
* determinism.

Independent confirmation by the reviewer's own probes:

| Check | Probe | Result |
|-------|-------|--------|
| A* optimality | `simprobe astar 200`: 200 random 64×64 maps with 0–49 % rock and random `blocked` sets, against an independent Dijkstra | 106,762 paths compared, no mismatch |
| Determinism (I3) and I12 | `simprobe hash`: 60 units, 3 seeds, 4,000 ticks of random orders | identical per-tick hash digests in debug and release |

### Sections 8 categories

* **Positive, negative, boundary, integration:** present in `m2_movement` and `m2_definitions`.
* **End to end:** both smoke runs. The smoke test now orders a move through `CommandQueue`.
* **Regression:** M1 review 3 P2-1 (FIFO) received a test for images (`a_named_pipe_is_rejected_without_blocking`).
* **Gaps:** see P1-1.

### Section 5 units and equations

* `SubPos` centre is `c·256 + 128`, and the cell is found by `div_euclid`.
* Speed is `u16` subcells per tick. Exact per-tick coverage is tested (`units_cover_exactly_their_speed_per_tick`).
* Diagonal step length is 362 (256·√2 rounded).
* Equation 5:
  * step costs are 10 and 14;
  * the octile heuristic `10(dx+dy) − 6·min` is used;
  * a diagonal step is forbidden only when both orthogonal neighbours are blocked. This matches the plan's wording and is tested both ways.
* Equation 7 is used unchanged for drawing.
* No floating point is used in `crates/sim/src` apart from the M1 debt P2-B (`map_subset.rs:524`).

### Invariants

| Invariant | Result |
|-----------|--------|
| I1 | `cargo tree -p sim -e normal` lists no forbidden crate. The only new dependencies are `ron` and `serde`. |
| I2 | Holds; see section 5 above. |
| I3, I4 | Holds. Collections are `BTreeMap`/`BTreeSet`, iteration is sorted by `SimId`, and ambiguity detection is set to Error. Hashes are identical across profiles. |
| I5 | Holds. The game writes the sim world only at setup (`setup_units`, as D11) and otherwise only pushes into `CommandQueue`. |
| I6 | Held on every tick in all reviewer probes (`check_i6` in livelock, repro, and budget runs). |
| I7 | Not applicable. |
| I8 | Holds for every hostile data file and command tried; see the probe tables below. |
| I9 | Exact pins, and `Cargo.lock` changed only in `sim`'s dependency list. |
| I10 | Tests use `tests/fixtures/data/units.ron`. |
| I11 | The RON subset was checked against `ron`'s source (whitespace, comments, integers, suffixes, and attributes all agree). Sprite sheets reuse the PNG subset. |
| I12 | The arithmetic in the new sim code is saturating, bounded, or checked. |

### Hostile inputs, both profiles

Each case below exited 1 with a typed error naming the file, the field, and the reason. There was no panic, and peak memory stayed at or below 0.11 GB.

| Input | Error (release; debug identical) |
|-------|----------------------------------|
| `#![enable(implicit_some)]` prefix | `units.ron: file: line 1, column 1: expected a value or punctuation of the supported subset, found '#'` |
| 64 KiB + 1 file; 1 GiB sparse file | `file: the file is larger than 65536 bytes` |
| FIFO, symlink to `/dev/zero`, directory | `file: cannot read the file: not a regular file` (no hang) |
| 30,000-deep `[` nesting | `units[0]: line 1, column 29: expected a struct "(", found '['` |
| `sprite: "../../maps/demo/demo_tiles"` | `units[1].sprite: value … is not supported, expected 1 to 32 characters …` |
| duplicate id, speed 0, speed 129, unknown field `hp`, missing `speed` | precise field and reason in each case |
| byte order mark, vertical tab, block comment, empty file, CR-only line endings, schema version 2, missing file | precise reason in each case |
| speed `-9223372036854775808` / `9223372036854775808` | range error / `expected a 64-bit signed integer` |
| `scout.png`: missing, 512×32 image, 300 MiB `iCCP` bomb, 16-bit greyscale, FIFO, `/dev/zero` symlink | `units[1].sprite: sprite image …/scout.png: …` with the specific reason |

These cases exited 0, and were correctly accepted:

* a 400 MB `scout.png.meta` (never read);
* a sheet with 200 MiB of trailing zeros inside `IDAT` (peak 0.11 GB);
* two unit types sharing one sprite;
* 64 unit types with 62 extra sheets (peak 0.15 GB).

A units file without `scout` exited 1 with `starting unit "scout" at (14, 34): unit type "scout" is not defined`.

## Findings

### P0

None.

### P1-1: Move orders can livelock forever; the guarantee "no order lasts forever" is false, and the tests do not cover the case

**Files and lines:**

* `crates/sim/src/movement.rs:32-35`: the doc of `MAX_STALLED_TICKS` reads "every order ends".
* `crates/sim/src/movement.rs:116-129`: the stall counter resets on any advance.
* `crates/sim/src/pathfinding.rs:310-322`: the wait-cycle step aside.
* `docs/DECISIONS.md:276` (D19 item 6): "… ends its order with `MoveFailed { Blocked }`, so no order lasts forever".
* Test gaps:
  * `crates/sim/tests/m2_movement.rs:966-983` covers a two-cell room only.
  * `m2_movement.rs:1198-1246` covers only 3 seeds.

**Minimal deterministic reproduction** (`simprobe repro 0`, debug and release identical):

* Setup: `open_16x16` fixture with a sealed vertical room of three cells (7, 3), (7, 4), (7, 5). Every other cell of the 3×5 block around it is rock.
* Units: walker A at (7, 3) and walker B at (7, 5).
* Orders: issued in one tick with the repository's own `order_swap` sequence. A gets a dummy order, B is ordered to (7, 3), and A to (7, 5).
* Result after 20,000 ticks (16.7 minutes of game time), with no further command:

  ```
  variant 0: unit 0 at (7, 3), order Some((CellPos { x: 7, y: 5 }, 1702, 0)), ended None
  variant 0: unit 1 at (7, 5), order Some((CellPos { x: 7, y: 3 }, 1702, 0)), ended None
  ```

  Both orders are still live with 1,702 repaths each, and no `MoveCompleted` or `MoveFailed` was ever written.

**Mechanism** (from a trace, `simprobe trace 62`):

1. The units form a wait-for cycle.
2. The yielder steps aside into the free middle or end cell.
3. The other unit advances one cell and blocks again.
4. Roles alternate every 8 to 16 ticks.

Each unit advances at least once per cycle, so `stalled_ticks` returns to 0 before it reaches 200, and `MoveFailed { Blocked }` never fires.

**Gameplay reach:** the same livelock arises without a constructed room.

* In `simprobe livelock 300` (300 random 16×16 maps with 0–34 % rock, 5–64 units, random group orders for 1,500 ticks, then no orders), 2 of 300 scenarios never settled within 20,000 further ticks.
  * Seed 62: units 16 and 43 have 1,674 and 2,225 repaths.
  * Seed 154: units 28 and 38.
* In seed 62, the dead end is formed by rock and by idle units at its openings, which is an ordinary situation on a map with a rock ridge and a lake.
* I6 held on every tick, so this is not an overlap. It is a non-terminating order that keeps consuming the path budget forever.

**Why P1:**

* D19's documented guarantee and the module documentation are contradicted.
* The tests meant to establish "every order ends" are inadequate for the specified behaviour (PLAN.md section 10, P1).
* The acceptance criterion itself is stated for the fixture scenarios, where it holds.

**Expected behaviour:**

* Every move order ends within a bounded number of ticks. For example:
  * stall detection measures progress toward the destination (the best remaining path cost or octile distance has not improved for `MAX_STALLED_TICKS`) rather than any movement; or
  * the number of step-asides or repaths per order is capped and ends the order with a typed `MoveFailed` reason.
* A regression test reproduces the three-cell sealed room (and a dead-end corridor closed by idle units). It fails against the current code and passes after the fix.
* The random-order test runs enough seeds to cover such cases.

### P1-2: Match setup silently spawns fewer starting units than configured

**Files and lines:**

* `crates/game/src/units.rs:249-272` (`setup_units`): the `Vec` returned by `destination_cells(grid, CellPos::new(x, y), count, …)` at line 253 is used without checking that it holds `count` cells. An anchor outside the map yields an empty `Vec`.
* `crates/game/src/smoke.rs:134`: the smoke test fails only when `spawned == 0`.
* D20 item 4 states that the game spawns exactly 12 + 6 units for player 0 and 5 + 3 for player 1.

**Evidence:** the demo map was replaced in a scratch asset root by the M2 fixture `arena_32x32.tmx`, a valid map inside the subset, with a matching 128×32 tileset image.

* Smoke runs:
  * Release and debug `--smoke-test 150` both log `12 units spawned` and `units spawned: 12, units moved: 11|12`, and **exit 0**.
  * 14 of 26 configured units were dropped: all 8 opponent units and the 6 scouts, whose anchors (50, 30), (52, 36), and (14, 34) lie outside the 32×32 map. No warning or error was logged.
* Normal (non-smoke) run in release: it logs `12 units spawned` and keeps running (still alive at the 8 s alarm, exit 142), with no warning.
* 16×16 fixture as the demo map: `0 units spawned`. The normal game then runs with no units and no error. Only the smoke test fails, and only because the count is 0.

By contrast, `spawn_unit` errors in the same function do log `error!` and exit 1.

This is a silent fallback (PLAN.md section 10: P1): configured match content is dropped without an error.

**Expected behaviour:**

* `setup_units` treats fewer free cells than `count`, including an anchor outside the map, as a typed setup error.
* The error names the owner, unit type, anchor, and the number of cells found versus required.
* The game logs it and exits with code 1, as it does for `SpawnError`.
* A test covers an anchor outside the map and an anchor with too few reachable free cells.
* The smoke test requires the configured total, not merely a count above 0.

### P2-1: `MoveCompleted` is reported for retargeted orders, indistinguishable from a real arrival

**Files and lines:**

* `crates/sim/src/movement.rs:161-165` writes the event.
* `crates/sim/src/pathfinding.rs:338-345` retargets the order.
* `crates/sim/src/event.rs:25` defines the event.
* D19 item 7.

**Evidence:**

* `simprobe repro 1` (sealed three-cell room, A at (7, 3) ordered to (7, 4), B at (7, 4) ordered to (7, 3)):
  * B ends at (7, 5), farther from its target than where it started.
  * B receives `MoveCompleted { unit: SimId(1) }` at tick 24.
* The repository's own test `a_destination_held_by_an_idle_unit_is_replaced_by_the_nearest_reachable_cell` asserts `MoveCompleted` for a unit that stopped short of its destination.

The behaviour is documented, so it is not silent. However, the AI (M6) and the interface (M5) cannot tell arrival from abandonment.

**Expected behaviour:** a distinct event or field (for example `MoveCompleted { unit, retargeted: bool }` or `MoveFailed { reason: DestinationTaken }`), with a test.

### P2-2: Large group orders, which the command limit allows, make most units abandon the order with `Blocked` without moving

**Files and lines:**

* `crates/sim/src/command.rs:22`: `MAX_UNITS_PER_COMMAND = 1024`.
* `crates/sim/src/movement.rs:116-129`: the stall rule counts ticks spent in a crowd and waiting for a path.
* `crates/sim/src/pathfinding.rs:31`: the budget of 16 requests per tick.

**Evidence:** `simprobe budget N` on an open 256×256 map, with a √N-wide block of walkers ordered across the map, run in release.

| Units | Result |
|-------|--------|
| 50 | 50 completed |
| 200 | 199 completed, 1 `Blocked` |
| 400 | 52 units ended with `MoveFailed { Blocked }` without ever moving |
| 1,024 (one valid command) | 831 units ended with `Blocked` without ever moving; 193 completed |
| 4,000 (four commands) | 3,950 units ended with `Blocked` without moving |

Some of the 4,000-unit failures come from requests not served within 200 ticks (more than 3,200 queued), so `Blocked` is also reported for units that never received a path.

The M2 acceptance (50 units) is met. The 200-unit benchmark of M7 is close to the threshold.

**Expected behaviour:** before M7, either:

* stall counting excludes ticks spent queued for the pathfinder or waiting behind units that advance; or
* the limits are reduced and documented.

The `Blocked` reason should be accurate.

### P2-3: A right-click order can use the previous frame's hovered cell

**Files and lines:**

* `crates/game/src/selection.rs:36-40`: `issue_orders` is ordered only after `TransformSystems::Propagate` and `CameraUpdateSystems`.
* `crates/game/src/map.rs:49-54`: `update_hovered_cell` is in the same schedule with the same constraints.

The two systems are not ordered relative to each other. When `issue_orders` runs first, a right click uses the cell hovered in the previous frame, which can be a neighbouring cell or `None` at the map edge.

**Expected behaviour:** `issue_orders.after(update_hovered_cell)`, or the target computed from the cursor in the same system.

### P2-4: D19 item 3 states a validation order the code does not follow

**Files and lines:** `docs/DECISIONS.md:273`; `crates/sim/src/command.rs:166-200`.

* D19 says "no unit twice" is checked before "every unit exists" and "belongs to the player".
* `validate` instead checks duplicate, existence, and ownership per id in one pass. For `[unknown X, Y, Y]` it returns `UnknownUnit(X)`, not `DuplicateUnit(Y)`.
* No state is affected.

**Expected behaviour:** the documentation describes the per-id order, or the code checks in the documented order.

## Requires user verification

These items only the user can verify. They are not findings (PLAN.md section 12 item 2), do not affect the decision, and block only `m2-approved`. Run `cargo run --release -p game` and pan left of the rock ridge.

1. **Unit rendering:**
   * 18 blue-tinted player units around cells (10, 28) and (14, 34): 12 light tanks with turret and barrel, and 6 smaller wedge-shaped scouts.
   * 8 red-tinted opponent units around (50, 30) and (52, 36).
   * Each unit stands centred on its tile's diamond, and units lower on the screen are drawn in front.
2. **Facings:** a moving unit faces its direction of travel in all eight directions, and keeps the facing when it stops.
3. **Smooth movement:** no jumps or jitter at any zoom.
4. **Click selection:** a left click on an own unit draws a green diamond under it. A click on empty ground clears the selection. Opponent units cannot be selected.
5. **Box selection:** a left drag draws a green rectangle and selects the own units inside it on release.
6. **Additive selection:** Shift adds to the selection.
7. **Orders:**
   * A right click sends the selection to distinct cells around the clicked cell, through the ridge gap and around the lake.
   * A click on rock or water uses the nearest cells around it.
   * `S` stops the selection at the next cell centre.
8. **M1 items** (still pending for `m1-approved`): map rendering, picking, panning, zoom, and bounds, as listed in docs/STATUS.md.

## Recorded debt that must be carried in docs/STATUS.md

* **New from this review:** P2-1, P2-2, P2-3, and P2-4 above. P1-1 and P1-2 must be fixed, not recorded.
* **Implementer's M2 known debt, confirmed by this review:**
  1. Holes in group arrival (deviation 2).
  2. `MessageReader` sees only the last two ticks, so the smoke test's rejection check can miss an event if three or more ticks run in one frame. The movement check after 40 ticks partly compensates.
  3. Selection, order, and drawing systems are tested only through pure functions.
  4. Path searches allocate arrays of the map's size. The reviewer measured a maximum tick of 10.8 ms for 50 units and 24 ms for 200 units on a 256×256 map in release. The mean is far lower, and the budget is to be checked in M7.
  5. `Grid::set_terrain` and `spawn_unit` are public (I5 is a discipline until M5).
  6. No automated screenshot.
* **Carried from M1 and unchanged by M2:**
  * review 3: P2-1 for the map and `.tsx` reader in `sim` (FIFO), P2-2, and the remaining items of P2-3;
  * review 1: P2-3 and P2-5;
  * review 2: P2-B;
  * round 1: item 4;
  * round 2: items 5 and 6;
  * M0: items 1, 2 (the `Skipped event Destroyed` warning was seen again in this review's smoke runs), 3, 5, 7, 8, and 9.
* **Resolved by M2 and confirmed:**
  * M1 review 3 P2-1 for tileset images and sprite sheets (FIFO, `/dev/zero`, and directory rejected without blocking, in both profiles);
  * M1 review 3 P2-3 item 4 (D6 amended).

## Decision rationale

All six validation commands pass. No test was removed or weakened, and the required M2 tests exist and pass. Determinism, A* optimality, I6, and the RON and PNG subsets held under the reviewer's probes.

However, there are two P1 findings:

* P1-1: move orders can livelock forever, contradicting the documented guarantee, and the tests do not cover the case.
* P1-2: a silent fallback in match setup.

PLAN.md section 10 makes NO-GO mandatory for any unresolved P1.

Decision: NO-GO
