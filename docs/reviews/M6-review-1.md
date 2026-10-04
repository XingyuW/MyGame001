# M6 review 1: Skirmish AI and game flow

* **Reviewer:** an independent review session. It did not implement or remediate any of this code, and it changed no repository file.
* **Date:** 2026-10-04.
* **Branch and commit:** `milestone/m6` at `7c44890`, with a clean working tree before and after the review.
* **Base:** tag `m5-reviewed`. The diff reviewed is `git diff m5-reviewed..milestone/m6`: 33 files, +7,891 and −463 lines, in commits `3279e84`, `30ea562`, and `7c44890`.
* **Documents read:** PLAN.md in full, including section 12; docs/STATUS.md (the status table, the M5 review decision with its carried debt, and the M6 implementation report); docs/DECISIONS.md (D1 to D44, with the M6 A1 follow-up and the M6 notes).
* **Scratch work:** all probes ran outside the repository, under `…/scratchpad/m6-review-1-probe/`:
  * scratch asset roots selected with `BEVY_ASSET_ROOT`;
  * a probe crate `simprobe` that depends on the repository's `sim` crate by path, with its own target directory and a copy of `Cargo.lock`;
  * a `git archive` copy of the repository with one added logging system. This copy is used only for the entity-count probe and is not part of any finding.

## 1. Validation commands (PLAN.md section 9 with section 12 item 5)

The reviewer ran every command from the repository root, in the foreground, one at a time, on macOS 27.0.1 (Apple M5) with Rust 1.98.1. A display and an audio device were present.

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | No diff. |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | No warnings. |
| `cargo test --workspace` | 0 | 384 passed, 0 failed, 0 ignored; 153 s wall time; peak memory footprint 0.13 GB. Breakdown below the table. Only the known M0 linker warning appeared. |
| `cargo build --release -p game` | 0 | Up to date. |
| `cargo run --release -p game -- --smoke-test 600` | 0 | Summary below the table. |
| `cargo run -p game -- --smoke-test 600` | 0 | The same three matches as the release run; summary below the table. |

**Test breakdown:**

* `game`: 111 unit tests, plus integration tests `m0_spike` 6, `m1_demo_map` 3, `m1_tileset_image` 6, `m2_unit_sprites` 5, `m3_building_sprites` 3, and `m6_production_matches` 1 (52.3 s).
* `sim`: 11 unit tests, plus integration tests `m0_bootstrap` 12, `m1_map` 32, `m1_map_subset` 24, `m2_definitions` 14, `m2_movement` 37, `m3_definitions` 11, `m3_economy` 21, `m3_harvest` 10, `m4_combat` 32, `m4_definitions` 12, `m5_placement_preview` 2, `m6_ai` 11, `m6_definitions` 11, and `m6_match` 9.

**Release smoke run:**

* Three matches started: `match on map demo (seed 6)`, then `(seed 7)`, then `(seed 6)`.
* The final line reported 600 frames and 168 ticks, with 16 units, 2 buildings, `menu buttons: Some(10)`, `matches started: 3`, `flow: Last`, `views: 16 of 16 units and 2 of 2 buildings`, and `computer player's commands: 2`.

**Debug smoke run:** 600 frames and 180 ticks, with the same M6 fields.

**Additional runs by the reviewer:**

* `cargo test -p sim --release --test m6_ai --test m6_match --test m6_definitions`: all 31 tests pass in release.
* `cargo test -p sim [--release] --test m4_combat twenty_against_twenty -- --nocapture`: in both profiles the battle ends at tick 665, with survivors 6 to 0, 451 shots, and 34 units destroyed. This is the same course as in M4.
  * The final hash is now `c9401425d786dac6` in both profiles. In M4 it was `05af3b1ac550cf16`.
  * The change comes from the documented hash layout extension (D41 item 4: `MatchState` and `AiPlayers` are appended). It is not a behaviour change.

## 2. Scope and claims checked

### 2.1 Implementer claims named by the orchestrator

| Claim | Verdict | Evidence |
|-------|---------|----------|
| The AI issues commands only through `CommandQueue` (I5) | **Confirmed** | `crates/sim/src/ai.rs:567-577`: `run_ai` takes only `Res<…>` and read-only `Query<&…>` parameters, plus `ResMut<AiPlayers>` (its own state) and `ResMut<CommandQueue>`. Its only writes are its controller state and `queue.push(IssuedCommand { player, command })` (`ai.rs:647-651`). The commands are validated in the `Commands` stage of the next tick, as the human player's are. |
| The AI's knowledge of map terrain and start positions is a recorded decision, not a fog-of-war leak | **Confirmed** | See the analysis after this table. |
| Match setup writes the simulation directly before the first tick, and its acknowledgement is deferred to the user | **Agreed** | `sim::setup_match` (`match_setup.rs:287-448`) and `SimHost::install` (`sim_host.rs:141-153`) write before the first tick, as D11, D19 item 10, D24 item 7, and D34 item 4 did since M1. M1 review 1 P2-5 asked for the user's acknowledgement, which STATUS lists as user verification item 12. This is not a finding. |
| Starting values moved to data files (I10) | **Confirmed** | See the analysis after this table. |
| Shipping one faction and one map is a deviation that M7 covers | **Accepted** | See the analysis after this table. |

**What the AI knows.** D43 item 2 records it. Reading the code confirms that `think` sees only the `View` (`ai.rs:415-534`):

* enemy units and buildings filtered by `Vision::sees_unit` and `Vision::sees_footprint`;
* ore, occupants, and buildings read only after an `is_visible` check (`ai.rs:837-852`, `866-874`);
* `check_placement`, whose first check is visibility (D33).

The static terrain flags and the start positions come from match setup (`ai.rs:143-230`). The enemy's credits and queues are never read. `m6_ai::the_ai_ignores_what_its_player_does_not_see` checks this. The reviewer found no other read of hidden state.

**Starting values in data (I10).** `assets/data/skirmish.ron` holds the starting credits, buildings, and units. `game::{buildings,units}` no longer contain `STARTING_*`, and a grep of `crates/game/src` and `crates/sim/src` finds these values only inside `#[cfg(test)]` modules.

The AI's behavioural constants stay in code: `AI_UNIT_QUEUE`, `GATHER_DISTANCE`, `GATHER_RADIUS`, `HARVEST_RETRY_TICKS`, `MAX_TARGET_FAILURES`, `BUILDING_GAP`, and `PRODUCER_GAP`. They are documented in D43, and the difficulty is set by the seven data parameters (D42 item 3). The reviewer treats them as algorithm constants, not balance values.

**One faction and one map.** PLAN.md section 7 gives the second faction and three maps to M7. The selection mechanism is tested with two fixture factions and with the pure menu functions over several entries per row.

The reviewer also ran the real binaries in both profiles with a scratch catalogue of two maps (`second`, a copy of the demo map with swapped starts, listed first, then `demo`):

* both maps were checked at startup (`catalogue: 2 maps`);
* all three matches of the smoke flow started on `second`, with the human player at (52, 42);
* both runs exited 0.

### 2.2 Other runtime checks of the production path

**Ending a match removes everything of it.** A `git archive` copy of the repository, with one added logging system, counted `MapRoot`, `TiledMap`, and `TilePos` entities whenever the counts changed. It was built in release in the scratch directory and run through the smoke flow.

* Each match had 1 map root and 4,096 tiles.
* Ending a match left 0 map roots, 0 tiled maps, and 0 tiles.
* The last match kept its map after `bevy_ecs_tiled` logged `Map removed` for the previous asset id.

So the recursive despawn of D44 item 5 works, and the old asset's `Removed` event does not despawn the new map.

**A match that ends inside the smoke run.** The reviewer used a scratch root with the computer player's start at (21, 37) and `normal` set to think every tick, waves of 1 from tick 0, and patience 1. The release and debug binaries both:

* logged `the match has ended: Won { winner: PlayerId(1), tick: 564 }`, `player 0 is defeated`, and `the match is over: player 1 wins`;
* stopped ticking at 565 ticks;
* exited 0, with no panic.

**Shipped AI against AI, through `simprobe`.** All nine pairings of the shipped difficulties on the demo map end with a winner within 23,022 ticks (the longest pairing is normal against easy). Every difficulty against a passive player ends with the computer player winning within 7,611 ticks. There are 0 rejected commands in all twelve matches. Normal against normal gives the same per-tick hash digest (`09f9b60baf788477`) and final hash in debug and release, which confirms I3 and I12. These figures match the report's D43 item 9.

### 2.3 Hostile inputs on the real binaries (debug and release)

Each probe used a scratch copy of `assets/` selected with `BEVY_ASSET_ROOT`. Each binary ran with `--smoke-test 100` (600 where noted), a 60 s alarm, and `/usr/bin/time -l`.

All of the following exit with code 1, a typed error naming the file and the field, a peak footprint of 0.136 to 0.144 GB, and an identical message in both profiles:

* **`skirmish.ron`:**
  * missing file; a named pipe; a symlink to `/dev/zero`; a directory; 70 KB; a byte order mark; `#![enable(implicit_some)]`; a block comment; trailing content;
  * an unknown field; a missing field; schema version 2;
  * credits −1, 1,000,001, and 10^20; count 0 and 65; offset 33;
  * a duplicated start; a start at x = 256; a start at x = −1; three starts; one start;
  * no buildings; an unknown building; an unknown unit;
  * the map id `Demo`, the map id `../demo`, and a map without a file;
  * a duplicated map; a duplicated faction; 200 levels of nesting;
  * the two yards overlapping, the yard off the map, and a group off the map (the last three fail when the match starts).
* **A second map with a 0 × 0 `.tmx`:** the error names `skirmish.ron`, the map, the `.tmx`, and the field, and the run fails within 1 frame.
* **`ai.ron`:**
  * missing file; a named pipe; 70 KB; `#![enable(...)]`;
  * `think_interval` 0 and 1,201; `wave_size` above `max_army`; `max_army` 0 and 201; `harvesters_per_refinery` 9; `defence_radius` 33; `first_wave` 72,001; `wave_patience` 0;
  * an army of harvesters; an empty army; an unknown army unit; a power building that produces no power; a harvester that is not a harvester;
  * a prerequisite listed later in the build order; an empty build order; 33 build-order entries; a duplicated difficulty id; no difficulties.

No probe was killed, panicked, or hung. Two inputs that pass validation misbehave: one is P1-1 and the other is P2-1.

## 3. Findings

### P0

None.

### P1

#### P1-1: The start positions of the skirmish file are never checked against their map, and the computer player falls back to a gathering point off the map. Every gathering and rally command it then issues is rejected, and no test or smoke check notices.

* **File and line:**
  * `crates/sim/src/skirmish_defs.rs:369-383`: a start is checked only against `0..=255` (`MAX_MAP_SIDE − 1`) and against duplicates. The skirmish file names its map (`map_asset_path`), but the start is never compared with that map's size or terrain.
  * `crates/game/src/skirmish.rs:114-129`: `load` checks every map of the skirmish file at startup with `check_map`, so the grid is at hand, but it does not check the map's start positions against it.
  * `crates/game/src/skirmish.rs:770-772`: the property is asserted only in a unit test for the shipped data (`assert!(entry.map.grid.contains(start))`).
  * `crates/sim/src/match_setup.rs:364-381, 420-439`: `setup_match` accepts the start whenever the faction's offset buildings and groups fit, and gives it to `AiController::new`.
  * `crates/sim/src/ai.rs:165-172`: `gather = destination_cells(grid, toward, 1, |_| false).first().copied().unwrap_or(start)`. When `toward` lies off the map, `destination_cells` returns nothing (`pathfinding.rs:171`), and the gathering point silently becomes the off-map `start`.
  * `crates/sim/src/ai.rs:183-190`: the enemy's off-map start also becomes a priority waypoint of the attack wave.
* **Evidence:**
  * **Scratch root `roots/p1_start_outside`:** the demo map (64 × 64) with starts (27, 37) and (70, 42). The single faction has its yard at offset (−12, 0) and its groups at (−8, 1) and (−11, 4). Every building and unit therefore lies on the map, but the computer player's start (70, 42) does not.
  * **The game accepts the file:**
    * `target/release/game --smoke-test 600` and `target/debug/game --smoke-test 600` both exit **0**.
    * Each logs 11 times `WARN game::units: command of player 1 rejected: cell (70, 42) is outside the map: AttackMove { units: [SimId(10), …, SimId(17)], target: CellPos { x: 70, y: 42 } }`.
    * The smoke line reports `computer player's commands: 10`.
  * **The smoke check is too weak to notice:** it counts commands pushed, not accepted (`smoke.rs:863-868`), and the human player's rejections are the only ones that fail a run (`smoke.rs:388-398`).
  * **In the simulation (`simprobe`, normal against normal, 36,000-tick limit):**
    * **release:** 19,249 ticks; **2,048 rejected commands** (2,047 of player 1 and 1 of player 0, whose wave was sent to the enemy's off-map start); the first is at tick 2 and then one every 20 ticks (each turn).
    * **debug:** identical (digest `afc4872e90ba01e7`).
    * **passive player against normal:** 544 rejections in 4,726 ticks.
  * **The computer player's home army can never gather.** Idle units farther than `GATHER_RADIUS` from the gathering point are re-ordered to it every turn, and each order is rejected. This falsifies D43 item 5: "A computer player's command can be rejected only because of what happens in the tick between its decision and the command's validation."
  * **The data is malformed** (a start that is not a cell of its map), yet it is accepted without a typed error. PLAN.md section 5 says "Data files … are cross validated at load", and section 8 says "Negative: … malformed data are rejected with the specified error". The fallback in `ai.rs:172` turns the configuration error into wrong behaviour instead of an error, which is a silent fallback (section 10: P1).
  * `m6_definitions::limits_are_accepted_at_both_ends` even accepts `starts[1] = (255, 255)` for the 40 × 40 fixture map.
* **Expected behaviour:**
  * When the catalogue is loaded (the game) and in `setup_match` (sim), a start position that is not a cell of its map is rejected with a typed error. The error names `skirmish.ron`, `maps[i].starts[j]`, the cell, and the map size; a passable start cell should preferably be required as well.
  * The computer player never derives a gathering point or waypoint outside the map. If no gathering cell exists, that is a setup error, not a fallback to an invalid cell.
  * A regression test fails on the current code (for example, `setup_match` or `load` with a start outside the map).
  * The smoke run, or a sim test, should treat computer-player rejections that a loss does not explain as failures, as the end-to-end tests already do.
  * It would be natural for the same load-time pass to check that every faction fits at every start of every map, which is today's implementation debt item 7 (exit code 1 when such a match is chosen).

### P2

#### P2-1: The AI file's cross checks do not ensure that the army (or the harvester) can ever be produced. D42 item 4 claims more than the code does, and a valid-looking AI file stalls every match.

* **File and line:**
  * `crates/sim/src/ai_defs.rs:285-310`: only building prerequisites are checked against the build order.
  * `crates/sim/src/ai_defs.rs:341-356`: each army entry must be armed and have production terms. Its producer and prerequisites are not checked against the build order (or the factions' starting buildings).
  * The same holds for the harvester (`ai_defs.rs:322-333`).
  * `docs/DECISIONS.md` D42 item 4 says the cross checks "keep a computer player from getting stuck (rules that must hold for all values, not only the shipped ones)".
* **Evidence:** scratch root `roots/p2_army_unbuildable`: the shipped data with `war_factory` removed from `build_order`, `army: ["light_tank"]`, and a faction without starting units. `load_ai` accepts the file. Normal against normal then runs 36,000 ticks with **outcome `None`** and 0 rejections: neither player can ever produce an army unit or a harvester, so neither can attack, and `broke` never sells, because it requires the army's producer (`ai.rs:891-904`).
* **Mitigation:** D43 item 9 and implementation debt item 1 already record that termination is not proven, and the shipped data ends in every pairing.
* **Expected behaviour:** either add a cross check (every army unit type's and the harvester's producer and prerequisites are in `build_order`, or are starting buildings of every faction) or correct D42 item 4 to say that these cases are not covered. Then record the gap under the termination debt.

#### P2-2: `uninstall_match` clears pending commands silently before `close()` can log them

* **File and line:** `crates/game/src/sim_host.rs:204-209` calls `sink.reset()` and then `sink.close()`. `reset` (`sim_host.rs:308-313`) runs `pending.clear()` without a log line, so the `warn!("command not issued: the match is over: …")` in `close` (`sim_host.rs:316-321`) never sees a command on this path.
* **Evidence:** D44 item 4 says pending commands are discarded "with a log line, so nothing … is lost silently".
  * Through the interface the path is not reachable today. `end_match` runs only from the end screen, after `run_tick` already closed and drained the sink with log lines (`sim_host.rs:341-345`), or from the menu, when no match runs and no command can be built.
  * It is reachable when a running match is replaced, which the smoke flow does (`smoke.rs:196-220`), and which any later "abandon match" path would do.
* **Expected behaviour:** `close` (with its log) before `reset`, or `reset` logs what it drops.

#### P2-3: `a_run_with_another_seed_or_difficulty_diverges` tests neither a seed nor a different course

* **File and line:** `crates/sim/tests/m6_ai.rs:932-949`.
* **Evidence:**
  * Both runs use seed 6. They differ only in player 0's difficulty, and the test asserts only `one.hashes[0] != other.hashes[0]`.
  * That holds by construction, because `Difficulty::hash_into` (`ai_defs.rs:129-143`) puts the difficulty into `state_hash` before the first decision.
  * The name and comment ("the matches differ in their course") claim more.
* **Expected behaviour:** rename the test, and either assert a difference in the course (for example in the command stream or the outcome tick) or drop the seed claim.

#### P2-4: The smoke test's computer-player check passes when every command of the computer player is rejected

* **File and line:** `crates/game/src/smoke.rs:599-602` (`ai_commands = controller.commands_issued()`, which counts pushed commands) and `smoke.rs:863-868` (fails only if 0).
* **Evidence:** this is the P1-1 run: both profiles exit 0 although all but one of the computer player's commands in the last match were rejected (11 rejection warnings in the run).
* **Expected behaviour:** count accepted commands, or fail on computer-player rejections that a loss does not explain. In a 170-tick smoke match without contact, none can be explained.

## 4. Section 5 equations and units, section 6 and 12 invariants, section 8 categories

* **Equations.** M6 changes no equation of section 5. What the AI and the win condition compute agrees with them:
  * The AI's visibility forecast uses `dx² + dy² ≤ sight²` in cells from the footprint's nearest cell (`ai.rs:512-533`), which is equation 6 and D30 item 1.
  * Its remaining cost of a queued item is `cost − charged` (`ai.rs:913`), which is equation 3.
  * Defence uses the Chebyshev distance to footprints (documented). Targeting uses `octile_distance` only for ordering.
  * Ticks are `u64`, credits `i64`, and the AI's arithmetic is `i64` or saturating.
* **Boundaries.**
  * `m6_definitions::limits_are_accepted_at_both_ends` covers every data limit at both ends.
  * The build order is completed within 1,000 ticks; a turn every tick is tested; 0 credits is tested.
  * A draw in one tick and a defeat in the tick of the loss are tested (`m6_match`).
  * The early smoke exit at frame 599 of 600 is tested.
  * The missing boundary is the start position against the map size (P1-1).
* **I1:** confirmed. `cargo tree -p sim -e normal` shows no `wgpu`, `winit`, `render`, `window`, `input`, `audio`, `cpal`, `rodio`, `gilrs`, `image`, or `png` crate.
* **I2:** confirmed. There is no `f32` or `f64` in the new `sim` code. The only float in `sim` is the pre-existing property check in `map_subset.rs:524`.
* **I3:** confirmed. The tests assert equal per-tick hashes. The reviewer's debug and release digests are equal (shipped normal against normal, and the P1-1 scenario).
* **I4:** confirmed. Only `BTreeMap` and `BTreeSet` are used, and snapshots are sorted by `SimId`; ties are broken by id or cell. The ambiguity test includes `SkirmishPlugin`. The clock seed is read only in `game` (`skirmish.rs:418-427`).
* **I5:** confirmed for the AI and the interface (section 2.1). Match setup is setup, pending the user's acknowledgement (user verification item 12).
* **I6 and I7:** checked on every tick of the build-order test and every 10 ticks of every end-to-end match. Credits are never negative in the 0-credit test.
* **I8:** no panic, `unwrap`, or `expect` is reachable from the new data files (35 skirmish and 23 AI probes in both profiles, plus the seeded mutation tests). The cross validation is incomplete (P1-1, P2-1).
* **I9:** confirmed. `git diff m5-reviewed..milestone/m6 -- Cargo.toml Cargo.lock crates/*/Cargo.toml rust-toolchain.toml` is empty.
* **I10:** confirmed (section 2.1). The plan's tests use fixture data.
* **I11:** confirmed for both new files. They pass the RON subset checker before `ron` parses them (recursion limit 16; 6 schema levels). New maps go through `check_map` (D13 and D16) and the checked-bytes reader. `ron` 0.12.2 consumes only checked text. `bevy_ecs_tiled` 0.13.4 and `tiled` 0.16.0 consume only checked bytes (`map.rs:104-139`).
* **I12:** confirmed. `i64` and saturating arithmetic are used in `ai.rs` and `match_setup.rs`; the casts are bounded (`clamp(1, 16) as i32`, lengths written as `u64`). The probes and the 20 against 20 battle give identical results in debug and release.
* **Section 8 categories:**
  * **Positive:** present (`m6_ai`, `m6_match`, `skirmish` tests).
  * **Negative:** present, except for P1-1.
  * **Boundary:** present, except for P1-1.
  * **Regression:** the smoke early-exit hole has a test (`an_exit_before_the_last_frame_fails_the_run`).
  * **Integration:** present (`m6_ai` and `m6_match` through `SimPlugin` and `CommandQueue`).
  * **End to end:** present (AI against AI on the fixture map and on every shipped map, twice each with equal hashes; `m6_production_matches`; both smoke runs).

## 5. Tests removed, skipped, or weakened, and scope

* **No test was removed or skipped.** The test-name lists of every changed file at `m5-reviewed` and at `milestone/m6` differ only by additions: smoke +2, sim_host +2, skirmish +6, and the new test files. There is no `#[ignore]`. No existing `sim` test file changed.
* **Changed game tests.** The changes listed in the report, section "Tests changed", were checked. Each now reads the skirmish data instead of the removed constants and keeps or strengthens its assertions.
  * The one replaced assertion (the opponent starts with a gun turret → the gun turret is in the AI's build order) follows from documented deviation 4.
  * The M2 groups are kept as `DEMO_GROUPS` test input with the same values.
* **Scope.** Everything is within M6 (the AI, data, setup, win condition, menu, end screen, and restart). Two items lie outside the M6 list, and both are justified:
  * the smoke-test hardening (`fail_on_early_exit`), a regression fix of the validation harness;
  * the `misalignment` comment fix, which resolves recorded debt (M1 review 3 P2-3, first item).

## 6. Requires user verification (not findings; they block only `m6-approved`)

Run `cargo run --release -p game` with sound on. These are the 13 M6 items of the implementation report, verbatim in substance:

1. **Menu:** dark full-window menu, title, four rows (map `demo`, your faction `vanguard`, opponent `vanguard`, difficulty `normal`) with `<` and `>`, and Start and Quit. All text is legible, and the sidebar is neither visible nor clickable through the menu.
2. **Selection:** `<` and `>` cycle each row and wrap around (difficulty easy, normal, hard; one map and one faction).
3. **Start:** the demo map, with the camera on your base: yard at (15, 37), 5 light tanks and 3 scouts, credits 5000. The music keeps playing.
4. **The computer player builds:** around (52, 42) it builds a power plant, refinery, barracks, war factory, and later gun turrets, a second refinery, and a second power plant. Its harvesters collect ore, and its units gather toward the map's centre.
5. **It attacks and defends:**
   * waves from about 3 minutes on normal;
   * your units near its base are attacked by its units at home and by its turrets;
   * a building of its that you destroy is rebuilt.
6. **Difficulty:** on easy the first wave comes later (about 5 minutes) and is smaller; on hard it comes earlier (about 2 minutes) and is larger.
7. **Defeat:** losing all of your buildings stops the simulation and shows DEFEAT with Restart and Main menu; orders are refused with the error sound.
8. **Victory:** destroying all of its buildings shows VICTORY.
9. **Restart:** a new match with the same choices; nothing of the old match is left; credits are 5000 again.
10. **Main menu:** it returns to the menu, and Start with other choices uses them.
11. **Quit:** it closes the game.
12. **Acknowledgement (M1 review 1 P2-5):** match setup installs the grid and places the players, buildings, and units directly before the first tick, not through `CommandQueue` (D11, D40 item 5). During a match, the interface and the computer player change state only through `CommandQueue` (I5). Please confirm this reading of I5.
13. **Acceptance:** in complete matches started from the menu, you can lose to the computer player and you can defeat it.

The earlier pending items (16 for M5, 9 for M4, 10 for M3, 7 for M2, and 5 for M1) remain, with the M6 starting setup noted in the report: each player starts alike, and the opponent has no gun turret at the start.

## 7. Recorded debt that must be carried in docs/STATUS.md

**New in this review:**

* **P1-1 (blocking):** start positions are not validated against their map, and the AI falls back to an off-map gathering point. This must be fixed with a regression test, not carried.
* **P2-1:** the AI file does not check that the army and harvester producers and prerequisites are obtainable. D42 item 4 claims more than the code does. Probe: 36,000 ticks without an outcome.
* **P2-2:** `uninstall_match` resets (clears silently) before `close` (which logs). D44 item 4 claims a log line.
* **P2-3:** `a_run_with_another_seed_or_difficulty_diverges` tests neither a seed nor a different course.
* **P2-4:** the smoke check of the computer player counts pushed commands, not accepted ones.

**From the M6 implementation report (known debt), confirmed by this review:**

1. Termination of computer-player matches is measured, not proven. P2-1 shows a data-driven stall.
2. The computer player uses no randomness; the seed changes only `state_hash`.
3. A ready building waits while no valid spot is visible.
4. Waves use `AttackMove` only, and harvesters are not targeted on purpose.
5. Balance is uneven. The reviewer's measurements on the demo map:
   * easy beats normal from both starts;
   * normal beats hard from both starts;
   * hard beats easy from both starts.
6. The menu shows ids; the end screen does not block clicks on the map, which the closed sink refuses.
7. A match setup error ends the game with exit code 1 instead of returning to the menu. It should be removed together with the P1-1 load-time check.
8. The terminal event log also writes the computer player's events.
9. Test time: `m6_production_matches` takes 52 s, `m6_ai` 40 s, and the workspace 153 s.
10. Hotkeys work in the menu.

**Resolved by M6 and confirmed:**

* M0 debt item 3 (the constant seed; D44 item 3).
* M3 review 1 P2-4 and D20 item 4, D27 item 3, D31 item 5 (starting setup in game code; D40).
* M1 review 3 P2-3, first item (the `misalignment` comment).
* M3 implementation debt item 2, for computer players only.

**Carried unchanged from the M5 review decision:** every item of its "Recorded debt to carry", namely M5 review 1 P2-1 to P2-5 and M5 implementation debt items 1 to 8, the accepted deviations, the OFL note for M7 packaging, and everything carried from M4, M3, M2, M1, and M0. M6 did not make any of them worse:

* M5 P2-2 (the minimap for W + H > 128) still applies, because no such map is shipped.
* M5 P2-4 (the `m4_combat` temporary-file race) did not occur in this review's runs.

The linker warning (M0 item 1) appeared again.

## 8. Decision

One unresolved P1 (P1-1) makes NO-GO mandatory under PLAN.md section 10. P1-1 is accepted malformed data together with a silent fallback that produces an unending stream of rejected computer-player commands, and no test or smoke check detects it. All validation commands pass, no P0 was found, and the four P2 items would be acceptable as documented debt.

Decision: NO-GO
