# M2 review 3: Units and movement

## Scope and base

* **Reviewer:** an independent review session. It did not implement or remediate this code, and it edited no repository file. The working tree was clean before and after the review (`git status --short` is empty).
* **Branch:** `milestone/m2` at `1a165c8`.
* **Base:** tag `m1-reviewed` (commit `39d688c`). The cumulative diff is `git diff m1-reviewed..milestone/m2`: 37 files, +8432/−46, in these commits:
  * the M2 implementation: `d690d4c` (sim), `7ac87d1` (game), `a8644e5` (report);
  * review 1, `6d9c332`;
  * remediation round 1: `6af1b4d` (code), `2adf66f` (report);
  * review 2, `4fa123b`;
  * remediation round 2: `8b24946` (code), `1a165c8` (report).

  Remediation round 2 alone (`git diff 4fa123b..milestone/m2`) changes 8 files, +780/−53. Its code changes are in `crates/sim/src/{movement,pathfinding,unit,event}.rs`, plus one new fixture and 5 new tests in `crates/sim/tests/m2_movement.rs`.
* **Read in full:**
  * PLAN.md, including section 12;
  * docs/STATUS.md and docs/DECISIONS.md;
  * docs/reviews/M2-review-1.md and M2-review-2.md.
* **Code read in full:**
  * sim: `crates/sim/src/{movement,pathfinding,unit,event,command,data,definitions}.rs`, and the M2 changes to `map.rs`, `hash.rs`, `schedule.rs`, `lib.rs`, and `Cargo.toml`;
  * game: the non-test code of `crates/game/src/{units,selection,smoke}.rs`, and the M2 changes to `main.rs`, `map.rs`, and `tileset_image.rs`;
  * tests: every test added in round 2, and the helpers and acceptance tests of `m2_movement.rs`.
* **Third-party code that consumes external data:** I read `ron` 0.12.2 `parse.rs` at the points the D18 subset depends on:
  * `is_whitespace_char`, which also accepts VT, FF, U+0085, U+200E/F, and U+2028/9;
  * `skip_ws` and `skip_comment` (line comments run to `\n`; block comments nest);
  * the integer accumulation with `checked_add`/`checked_sub`;
  * the unclosed-line-comment handling, which applies only to raw values.

  The checker's subset is consistent with all of these: printable ASCII only, `//` comments only, decimal `i64`. The PNG path (`check_png_file`, the M1 subset) was exercised again with hostile sprite sheets.
* **Probes:** all ran outside the repository, under `…/scratchpad/m2-review-3-probe/`, each under a `perl alarm` timeout.
  * `probe` is a scratch crate that depends on `crates/sim` by path. It was built in debug and release.
  * `probe_mod` is the same crate built against a `git archive` of `1a165c8` in which only `MAX_STALLED_TICKS` and `MAX_PATHS_WITHOUT_PROGRESS` are raised to 60,000. It is used only to tell whether an order ended by those limits would have ended anyway.
  * `base4fa` is a `git archive` of the reviewed commit `4fa123b`, with the round-2 test file and fixture copied in.
  * 69 scratch asset roots, selected with `BEVY_ASSET_ROOT`, were run against `target/{release,debug}/game`.

  The memory watchdog killed no probe. The largest peak footprint of the game binary was 0.15 GB, and of a probe 0.06 GB.

  Note: the validation logs were written to the scratchpad root (`build.log`, `clippy.log`, `test.log`, `smoke_rel.log`, `smoke_dbg.log`) before the probe directory was created, and may have replaced files of the same name there.

## Validation commands (PLAN.md section 9 and section 12 item 5)

All six commands ran from the repository root, in the foreground, in this order, at `1a165c8`. A display was available.

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings. Both crates were re-checked after a `touch`. |
| `cargo test --workspace` | 0 | 186 passed, 0 failed, 0 ignored. `m2_movement` took 31.5 s. Peak footprint 0.13 GB. The known M0 linker warning appeared. |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | `26 units spawned`; `smoke test: 600 frames, 198 simulation ticks, map created: true, units spawned: 26, units moved: 18 in 198 ticks after the order` |
| `cargo run -p game -- --smoke-test 600` | 0 | `smoke test: 600 frames, 203 simulation ticks, map created: true, units spawned: 26, units moved: 18 in 203 ticks after the order` |

Test breakdown:

* `game`: unit 44, `m0_spike` 6, `m1_demo_map` 3, `m1_tileset_image` 6, `m2_unit_sprites` 5;
* `sim`: unit 3, `m0_bootstrap` 12, `m1_map` 32, `m1_map_subset` 24, `m2_definitions` 14, `m2_movement` 37;
* doc-tests 0.

Additionally, `cargo test --release -p sim -p game` exited 0 with the same 186 tests (`m2_movement` took 4.3 s). The remediation report's validation table is confirmed: the same counts, exit codes, and smoke log lines. Both smoke runs logged the known `Skipped event Destroyed` warning (M0 debt item 2).

## Status of earlier findings

| Finding | Status | Evidence from this review |
|---------|--------|---------------------------|
| Review 2 P0-1: a unit of speed 1 fails every move longer than one cell with `Blocked` | **Resolved** | See the detail after this table. |
| Review 1 P1-1: move orders can livelock forever | **Resolved (still holds)** | Every probe scenario of this review ended: 200 random maps × 6 speed mixes, the crossing, group, convoy, and demo scenarios, and random orders with mixed speeds. Livelocks still end through `MAX_PATHS_WITHOUT_PROGRESS`, slowly at speed 1 (recorded debt). The three D21 regression tests and the boundary-speed termination test pass in debug and release. |
| Review 1 P1-2: match setup silently spawns fewer starting units | **Resolved (still holds)** | `noscout` root, both profiles: exit 1 with `starting unit "scout" at (14, 34): unit type "scout" is not defined`. `plan_starting_units` is unchanged since review 2. |
| Review 1 P2-1: `MoveCompleted` for retargeted orders | **Resolved (still holds)** | `retargeted` is reported. The probes counted 1,558–1,712 retargeted completions per 200 scenarios. |
| Review 1 P2-2: large group orders end `Blocked` | **Partly resolved; remainder carried as debt** | Waits for the pathfinder and waits behind units that are moving on are no longer stalled. The remainder (path throughput, crowds) is round-2 debt item 1. A related speed-dependent residue is new finding P2-1. |
| Review 1 P2-3, P2-4 | **Resolved (still hold)** | Code unchanged since review 2. `game_systems_that_share_state_in_a_frame_are_ordered` and `validation_checks_each_named_unit_in_turn` pass. |

**Detail for review 2 P0-1:**

* **Lone unit, every speed.** This review ran all 128 admitted speeds (data files `speeds_a.ron` and `speeds_b.ron`), each with 7 orders: orthogonal, diagonal, mixed, single-cell, and own cell. Every one of the 896 orders completed with `MoveCompleted { retargeted: false }` exactly at `ceil(length / speed)`, with `stalled_ticks` 0 on every tick. The implementer checked 9 speeds.
* **Production path.** Scratch roots with both unit types at speed 1, at speed 128, and at speeds 1 and 128 were run with `--smoke-test 1300` in debug and release. All exited 0 with 0 `unit N stopped` lines (review 2 saw 18).
* **Regression tests.** Against a `git archive` of `4fa123b` with the new test file and fixture copied in, exactly the four behavioural tests fail and the other 33 pass. The failures are:
  * `a_unit_of_either_boundary_speed_crosses_an_open_map_and_arrives_on_time`;
  * `orders_end_at_both_boundary_speeds_when_units_keep_giving_way`;
  * `units_queued_behind_a_unit_of_the_slowest_speed_follow_it_to_their_destinations`;
  * `fifty_units_of_the_boundary_speeds_reach_a_clicked_destination` (`unexpected MoveFailed { unit: SimId(7), reason: Blocked }`).

  All four pass at `1a165c8` in debug and release. This confirms the report's "fails before" claim.
* **Documentation.** D21 item 1, D19 item 6, `MAX_STALLED_TICKS`, `MoveOrder::stalled_ticks`, and the round 1 report's enumeration claim are corrected. D22's general claim of speed independence is too strong (P2-1).

## Verification summary

### Tests removed, skipped, or weakened

None.

* Per-file `#[test]` counts never fall between `m1-reviewed`, `4fa123b`, and `1a165c8`. `m2_movement` goes from 32 to 37 tests, and every other file is unchanged.
* The only removed lines in `m2_movement.rs` since review 2 are the `use sim::{…}` list, which was re-wrapped to add `MAX_UNIT_SPEED`.
* No `#[ignore]` or `should_panic` was added.
* The existing fixture `units.ron` is unchanged.
* PLAN.md, `rust-toolchain.toml`, the workspace and `game` manifests, `.cargo/`, and every M0 and M1 test file are unchanged.

### Scope

Round 2 changes only the rules that end a move order and the hash of `PathState::Requested`, together with tests, a fixture, and documentation. Every change is within M2.

### Required M2 tests and acceptance (PLAN.md section 7)

All required tests are present and pass in debug and release:

* path cost is optimal on open and obstructed maps;
* an unreachable target ends the order with an event;
* a move outside the map is rejected;
* a unit ordered to its own cell stays;
* a newly placed obstacle triggers a repath;
* I6 holds with 50 units for 2,000 ticks;
* determinism.

**Acceptance, independently probed** (`probe group`): 50 units, the four fixture scenarios of the acceptance test, at nine speed mixes:

* uniform speeds 1, 2, 3, 5, 7, 16, and 128;
* mixed speeds 1 and 128;
* mixed speeds 3, 100, 1, and 128.

All 36 runs settled with 50 completions, no `Blocked`, and no I6 violation. They took from 151 ticks (speed 128) to 26,274 ticks (speed 1).

**The required I6 crossing scenario, run to completion** (`probe cross`, 4 crossings each): uniform speeds 1, 2, 3, 7, 24, 32, and 128, and the mixes 1+128, 1+32, 3+100, and 2+127. Every run gave 200 of 200 completions, 0 `Blocked`, and 0 I6 violations.

### Section 8 categories

* **Positive, negative, integration:** present.
* **Boundary:** movement is now tested at speeds 1 and 128, which closes review 2's gap.
* **Regression:** P0-1 has four tests that fail before the fix and pass after it.
* **End to end:** both smoke runs, plus this review's binary runs.

### Section 5 units and equations

* `SubPos` centre is `c·256 + 128`, found by `div_euclid`.
* Speed is `u16` subcells per tick. Arrival is exact for all 128 speeds.
* Step lengths are 256 and 362.
* Equation 5 is unchanged (`find_path` is untouched since review 2): costs 10 and 14, octile heuristic, and diagonal rule. `path_cost_is_optimal_on_open_and_obstructed_fixture_maps` passes.
* Equation 7 is used only in `game`.
* No floating point is used in `crates/sim/src` apart from the recorded M1 debt P2-B (`map_subset.rs:524`).

### Invariants

| Invariant | Result |
|-----------|--------|
| I1 | `cargo tree -p sim -e normal`: `bevy_app`, `bevy_ecs`, `quick-xml`, `rand_chacha`, `ron`, `serde`, `tiled`. No forbidden crate. |
| I2 | Holds (section 5 above). |
| I3, I4 | Per-tick `state_hash` digest of `probe hash` is `03ac03ccb4964878` in both debug and release: 3 seeds on the wall map, 60 units of speeds 1, 128, 32, 3, and 100, two players, 4,000 ticks of random `Move` and `Stop` with targets including outside the map, then 2,000 quiet ticks. The new `activity` map is a `BTreeMap` updated in `SimId` order. `queued` is hashed (`unit.rs:354-361`). `crates/sim/src` uses no `HashMap`, `HashSet`, or clock. |
| I5 | Holds. The game writes the sim world only at match setup and otherwise pushes into `CommandQueue`. |
| I6 | Held on every tick of every probe: position inside the closed squares of the held cells, no cell held twice, and grid and units consistent. |
| I8 | Holds for every hostile input below, in both profiles. No `unwrap`, `expect`, or `panic!` is reachable in non-test M2 code (the matches in `units.rs` and `selection.rs` are in `#[cfg(test)]` modules). |
| I9 | Exact pins. `Cargo.lock` changed only in `sim`'s dependency list. |
| I10 | Tests use fixtures (`units.ron`, `units_boundary_speeds.ron`). |
| I11 | The RON subset is consistent with `ron`'s reading rules (above). The PNG subset holds for sprite sheets (below). |
| I12 | Round-2 arithmetic is `saturating_add` (`movement.rs:238`, `pathfinding.rs:371`). `moving_on` does no arithmetic and terminates through its visited set. Debug and release digests are identical. |

### Every order ends (D21, D22), independently probed

* **Random maps, one wave:** `probe wave`, the repository's map generator style, 16×16 with 0–29 % rock, 10–59 units, three group orders at tick 0, run until no order is left.

  | Speeds | Unsettled | Slowest settle (ticks) | `Blocked` |
  |--------|-----------|------------------------|-----------|
  | 128 | 0 | 686 | 1 |
  | 32 | 0 | 956 | 6 |
  | 7 | 0 | 4,838 | 8 |
  | 2 | 0 | 11,433 | 13 |
  | 1 | 0 | 27,142 | 30 |
  | 1 and 128 | 0 | 17,361 | 8 |

  There were about 6,540–6,570 completions per speed set, and no I6 violation.
* **Demo map:** the real starting layout of the game, with the 18 player units ordered to 4 targets, at production speeds (24 and 40), 1, 128, 1 and 128, and 2 and 3. All 20 runs ended with 18 completions and 0 `Blocked`.
* **Random orders:** the remaining orders of the hash scenario end within 7,095 ticks.

### Hostile inputs through the real binaries (debug and release, identical results)

Each case exited 1 with a typed error naming the file, the field, and the reason. There was no panic, no hang, and a peak footprint of 0.11 GB.

| Input (`assets/data/units.ron` unless stated) | Error |
|------------------------------------------------|-------|
| `#![enable(implicit_some)]` prefix; block comment; named struct `UnitsFile(`; `Some(40)`; `r#speed` | `file: line 1, column 1: … found '#'`; `… (block comments are not supported)`; `… found identifier "UnitsFile"`; `units[1].speed: … found identifier "Some"`; `units[1].r: unknown field` |
| 1 GiB sparse file; 72 KB file | `file: the file is larger than 65536 bytes` |
| FIFO; symlink to `/dev/zero`; directory | `file: cannot read the file: not a regular file` (no hang) |
| symlink loop; mode 000 | `Too many levels of symbolic links`; `Permission denied` |
| 65,000 × `(`; 60,000 × `[` | `file: line 1, column 2: expected a field name, found '('`; `units[0]: line 1, column 29: expected a struct "(", found '['` |
| NUL byte; `Ü`; BOM; vertical tab | `expected printable ASCII, tabs, and line breaks, found byte 0x…` with line and column |
| speed `99999999999999999999`, `-9223372036854775808`, `-0`, `0`, `129`, `24.0`, `0x18` | `expected a 64-bit signed integer`; range errors naming `1 to 128 subcells per tick`; `expected a decimal integer without leading zeros` |
| empty file; `units: []`; 65 entries; 65-byte id; `\u{6f}` escape | precise reason in each case |
| duplicate id; duplicate field; unknown field `hp`; missing `speed`; `schema_version: 2`; trailing `()`; CR-only line endings | precise field and reason in each case. CR-only endings make the first `//` comment run to the end of the file, as in `ron`. |
| sprite `../../maps/demo/demo_tiles`, `Scout`, `nosuch` | `units[1].sprite: value … is not supported …`; `… sprite image …/nosuch.png: file: cannot be read: No such file or directory` |
| `scout.png`: missing, empty, FIFO, directory, 1 GiB sparse, 300 MiB `iCCP` bomb, 16-bit grey, palette, interlaced, `acTL`, `tRNS`, 512×32, short `IDAT`, bytes after `IEND`, damaged `IEND` | `units[1].sprite: sprite image …/scout.png: …` with the specific reason (for example `chunk "iCCP": chunk is not supported`, `chunk "IHDR" bit depth: value 16 is not supported, expected 8`, `image data: cannot be decoded: … not have enough data for image`) |

These cases exited 0 and were correctly accepted:

* CRLF line endings, tabs, and a final `//` comment without a newline;
* a 64,800-byte file of comments;
* 64 unit types with 62 extra sheets (peak 0.15 GB);
* an RGB8 sheet;
* a malformed `scout.png.meta` (never read);
* a sheet whose `IDAT` inflates 200 MiB beyond the image (bounded decode, peak 0.11 GB, as accepted by earlier reviews).

The M1 debt "a FIFO at the map path hangs at startup" (M1 review 3 P2-1) is unchanged: the release binary was killed by the 15 s alarm (exit 142). M2 neither touches nor depends on the map reader.

## Findings

### P0

None.

### P1

None.

### P2-1: The rule that ends an order still depends on unit speed in jams, because a unit waiting behind a slow unit in a step alternates its path every 8 ticks onto a way that counts as stalled. D22's claim that "the rules that end a move order do not depend on unit speed" is too strong.

**Files and lines:**

* `crates/sim/src/pathfinding.rs:216-221` (`is_waiting`): a unit counts as waiting only while `blocked_ticks > 0` or it is `Requested`. In the tick after its own path is served, a unit that is still stuck is not avoided by other units' path requests.
* `crates/sim/src/pathfinding.rs:370-372`: a served path that differs from the current one always counts toward `MAX_PATHS_WITHOUT_PROGRESS`, including when `queued`.
* `crates/sim/src/movement.rs:345-364`: on the alternative path, the wait is for a unit in a wait-for cycle, so it returns `AtCentre::Blocked` and counts as stalled.
* `crates/sim/src/movement.rs:14-16`: the module documentation says the end rule "never [measures] elapsed time whose length depends on a unit's speed".
* `docs/DECISIONS.md:310` and `:314` (D22 title and item 1): "the rules that end an order do not depend on unit speed"; "Every rule that ends an order or counts toward its end was checked against that range".

**Evidence:**

* **Trend across speeds.** `probe wave` (above), seeds 0–199, normal code: `Blocked` is 1, 6, 8, 13, and 30 at uniform speeds 128, 32, 7, 2, and 1. Of these, 0, 5, 7, 12, and 25 were ended by `MAX_STALLED_TICKS`. The implementer's own D22 item 8 data shows the same trend: 124 at speed 1 alone and 24 at speed 128 alone.
* **Which of those orders were premature.** `probe_mod` is the same code with both limits raised to 60,000.
  * Speed 32: still exactly 6 `Blocked`, each after at least 60,000 stalled ticks, and completions unchanged (6,564). These are permanent deadlocks, so the limits ended nothing at speed 32 that would have resolved.
  * Speed 1: 0 `Blocked`, and completions rise from 6,540 to 6,564. Three seeds (0, 80, and 147, the path-limit cases) never settle; these are livelocks that `MAX_PATHS_WITHOUT_PROGRESS` rightly ends.

  So at speed 1, about 25 orders per 200 scenarios are ended by the stall limit in jams that resolve, while at speed 32 none is.
* **Minimal trace** (`probe trace mix s1 81 6`, deterministic). In seed 81, all units have speed 1, and units 6, 7, 9, and 41 end with `MoveFailed { Blocked }` at tick 1116. At speeds 2, 7, 32, and 128, the same seed has no `Blocked`.

  Unit 6 stands at (6, 9) with destination (13, 8). Every 8 ticks its served path alternates between two ways:

  | Path via | Held by | What that unit does | Effect on unit 6 |
  |----------|---------|---------------------|------------------|
  | (7, 8) | unit 39 | moving on (from tick 1037 it is in a 256-tick step out of (7, 8)) | not stalled, but each new path counts |
  | (7, 9) | unit 41 | wants (6, 9): a head-on cycle | about 7 stalled ticks per interval |

  `stalled_ticks` climbs from 49 (tick 821) to 160 (tick 1036) and then to 193 (tick 1109), while unit 39 is stepping out of (7, 8). The order ends at tick 1116, 177 ticks before (7, 8) is free.

  With the limits raised (`probe_mod`), unit 6 completes at its own destination (`retargeted: false`) at tick 4879, and every order of seed 81 ends with no `Blocked`.
* **Speed dependence.** At speed 32 the step of unit 39 lasts 8 ticks, so unit 6 flips at most once per wait. At speed 1 it flips about 32 times per orthogonal step, gaining about 112 stalled ticks and 32 counted paths per step of the unit ahead. This is the class of review 2 P0-1 (counts that grow with the step time), reached through re-planning instead of through the unit's own step.

**Severity rationale:** P2. Every stalled tick counted is, at that moment, a wait for a unit that is not moving on, as D22 item 2 specifies. Every order still ends, I6 holds, and the event is typed. The effect is limited to jams at the lowest speeds (about 0.4 % of orders at speed 1 in this corpus, and none at the production speeds 24 and 40). The precedent is review 1 P2-2, premature `Blocked` inside crowds, which was rated P2.

**Expected behaviour:**

* Record this residue in docs/STATUS.md as debt, and correct D22's title and item 1, and `movement.rs:14-16`, so that they state the residual speed dependence and its measurement.
* When the end rules are next revised (at the latest with the M7 benchmark), a wait behind a unit that is moving on should not turn into stalled ticks or counted paths by re-planning onto a way through a wait-for cycle. One option is to make the "waiting" set seen by the pathfinder stable across the tick in which a path is served. Another is not to count a path that returns to a way already followed in the same wait.
* Add a regression test at speed 1, for example the seed-81 jam, showing that orders that resolve without the limits are not ended as `Blocked`.

### P2-2: A box selection released outside the window silently becomes a click at the drag's start point

**Files and lines:** `crates/game/src/selection.rs:162`: `let end = window.cursor_position().unwrap_or(start);`.

**Evidence:**

* `bevy_window` 0.19.1 `Window::cursor_position` / `physical_cursor_position` (`window.rs:620-632`) documents "Returns `None` if the cursor is outside the window area".
* `bevy_winit` 0.19.1 `state.rs:304-306` sets the position to `None` on `CursorLeft`.
* A left drag released outside the window therefore yields `start.distance(end) = 0 < DRAG_THRESHOLD_PX`. It is handled as a click at the drag's start point: it selects the unit there, or clears the selection, and the box is discarded. Nothing is logged.
* No test covers it: the pure functions are tested, and the system wiring is M2 debt item 3. Whether macOS reports `CursorLeft` during a drag was not observed; that part is listed under "requires user verification".

**Expected behaviour:** the system keeps the last cursor position seen during the drag, or clamps it to the window, and uses it as the box's end corner. A test of the selection decision covers a release with no cursor position.

## Requires user verification

These items are not findings (PLAN.md section 12 item 2). They do not affect the decision, and they block only `m2-approved` and `m1-approved`. Run `cargo run --release -p game` and pan to the left of the rock ridge.

1. **Unit rendering:**
   * 18 blue-tinted player units around cells (10, 28) and (14, 34): 12 light tanks with turret and barrel, and 6 smaller wedge-shaped scouts;
   * 8 red-tinted opponent units around (50, 30) and (52, 36);
   * each unit is centred on its tile, and units lower on the screen are drawn in front.
2. **Facings:** a moving unit faces its direction of travel in all eight directions, and keeps its facing when it stops.
3. **Smooth movement:** units glide between cells without jumps or jitter at any zoom.
4. **Click selection:**
   * a left click on an own unit draws a green diamond under it;
   * a click on empty ground clears the selection;
   * opponent units cannot be selected.
5. **Box selection:** a left drag draws a green rectangle and selects the own units inside it on release. Also check what happens when the drag is released outside the window (see P2-2).
6. **Additive selection:** Shift adds clicks and boxes to the selection.
7. **Orders:**
   * a right click sends the selection to distinct cells around the clicked cell, through the ridge gap and around the lake, without overlap or getting stuck;
   * a click on rock or water uses the nearest cells around it;
   * `S` stops the selection at the next cell centre.
8. **M1 items** (still pending for `m1-approved`): map rendering, picking, panning, zoom, and bounds, as listed in docs/STATUS.md.

## Recorded debt that must be carried in docs/STATUS.md

* **New in this review:**
  * P2-1: the residual speed dependence of the stall and path counts in jams, and the overstated D22 claim. The measurements and the expected behaviour are above.
  * P2-2: box selection released outside the window.
* **From the M2 remediation round 2 report, confirmed by this review:**
  1. Round 1 debt item 1, narrowed: path throughput is 16 requests per tick, and a waiting unit asks every 8 ticks. A crowd of 4,000 units in single file on a 256×256 map, measured by this review: mean tick 11.6 ms, worst 73 ms. To be measured against the M7 benchmark.
  2. Time to end a livelock: about 800 ticks at speed 32, and 17,000–18,000 ticks at speed 1. This review saw a speed-1 livelock end only after 22,143 ticks (seed 0, unit 38).
  3. Cost of the chain walk (3.25 → 3.34 ms mean for 1,024 units, per the report).
  4. No overtaking.
  5. Waiting under continuous orders.
  6. The smoke test passes even if every ordered unit ends `Blocked`.
  7. The M2 implementation debt items 1–6:
     1. holes in group arrival;
     2. the two-tick `MessageReader` window for the event log and the smoke test's rejection check;
     3. selection, order, and drawing systems are tested through pure functions only;
     4. path searches allocate arrays of the map's size;
     5. `Grid::set_terrain` and `spawn_unit` are public;
     6. no automated screenshot.
* **From M2 review 1:** P2-2 remainder, as narrowed in round-2 debt item 1.
* **Carried from M1 and M0, unchanged by M2:**
  * M1 review 3: P2-1 for the map and `.tsx` reader in `sim` (FIFO hang, re-confirmed above), P2-2, and the remaining items of P2-3;
  * M1 review 1: P2-3 and P2-5;
  * M1 review 2: P2-B;
  * M1 round 1: item 4;
  * M1 round 2: items 5 and 6;
  * M0: items 1, 2 (the `Skipped event Destroyed` warning was seen again in this review's smoke runs), 3, 5, 7, 8, and 9.
* **Resolved and confirmed:**
  * M2 review 2 P0-1;
  * M2 review 1 P1-1, P1-2, P2-1, P2-3, and P2-4, and the pathfinder-queue part of P2-2;
  * the "followers of a slower unit count their wait as stalled" part of round 1 debt item 1;
  * M1 review 3 P2-1 for tileset images, sprite sheets, and the units file (FIFO, `/dev/zero`, directory, symlink loop: all rejected without blocking, in both profiles).

## Decision rationale

* All six validation commands pass, and so does the release test run.
* No test was removed, skipped, or weakened. Every change is within M2.
* All required M2 tests and the acceptance scenario pass. This review's own probes confirmed the acceptance criterion at nine speed mixes, from speed 1 to 128.
* Review 2's P0-1 is resolved:
  * all 128 speeds arrive exactly on time on an open map;
  * the four regression tests fail against `4fa123b` and pass now;
  * the production binaries at speeds 1 and 128 report no stopped unit.
* Every invariant held in every probe. Debug and release hashes are identical. Every hostile data file and sprite sheet was rejected with a typed error in both profiles, and every order ended.
* The two new findings are P2:
  * P2-1 is a residue of the speed-dependence class in jams at the lowest speeds. It is per specification, bounded, and limited in reach, and it has the precedent of review 1 P2-2.
  * P2-2 is a UI edge case.

  Both must be recorded in docs/STATUS.md with the carried debt above.

Decision: GO WITH DOCUMENTED NON-BLOCKING DEBT
