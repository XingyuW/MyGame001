# M6 review 2: Skirmish AI and game flow

* **Reviewer:** an independent review session. It did not implement or remediate any of this code, and it changed no repository file. All scratch work and build output outside `target/` is under `…/scratchpad/m6-review-2-probe/`.
* **Date:** 2026-10-04.
* **Branch and commit:** `milestone/m6` at `834efc0`, with a clean working tree before and after the review.
* **Base:** tag `m5-reviewed`. The cumulative diff is `git diff m5-reviewed..milestone/m6`: 34 files, +9,586 and −465 lines, in commits `3279e84`, `30ea562`, `7c44890` (implementation), `bdbaa5e` (review 1), `85b310f` (remediation code), and `834efc0` (remediation report). This round looked hardest at the remediation diff `bdbaa5e..85b310f` (12 files, +1,274 and −86), and re-checked the rest of the cumulative diff for regressions.
* **Documents read:** PLAN.md in full, including section 12; docs/STATUS.md (the status table, the M5 review decision with its carried debt, the M6 implementation report, and the M6 remediation round 1 report); docs/DECISIONS.md (the M6 sections A1 follow-up and D40 to D44, the M6 notes, and D45 to D47); docs/reviews/M6-review-1.md.
* **Probes:**
  * `simprobe2`, a crate outside the repository that depends on `crates/sim` by path, with its own target directory and a copy of `Cargo.lock`. It plays matches through `SimPlugin`, `setup_match`, and `CommandQueue`. It can override start positions and terrain, and it classifies every rejection with the rule of `m6_ai::assert_rejections_explained`.
  * Scratch asset roots selected with `BEVY_ASSET_ROOT` for the real `game` binaries, in both profiles. Each probe ran with a `perl alarm` timeout and `/usr/bin/time -l`.
  * A `git archive` copy of `HEAD` with one check disabled, used only to confirm that a regression test fails without the fix.

## 1. Validation commands (PLAN.md section 9 with section 12 item 5)

The reviewer ran every command from the repository root, in the foreground, one at a time, on macOS 27.0.1 (Apple M5) with Rust 1.98.1. A display and an audio device were present.

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | No diff. |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | No warnings. |
| `cargo test --workspace` | 0 | 394 passed, 0 failed, 0 ignored; 153 s wall time; peak memory footprint 0.13 GB. Breakdown below the table. Only the known M0 linker warning appeared. |
| `cargo build --release -p game` | 0 | Up to date. |
| `cargo run --release -p game -- --smoke-test 600` | 0 | Summary below the table; peak 0.41 GB. |
| `cargo run -p game -- --smoke-test 600` | 0 | Summary below the table; peak 0.40 GB. |

**Test breakdown:**

* `game`: 114 unit tests, plus integration tests `m0_spike` 6, `m1_demo_map` 3, `m1_tileset_image` 6, `m2_unit_sprites` 5, `m3_building_sprites` 3, and `m6_production_matches` 1 (52.4 s).
* `sim`: 11 unit tests, plus integration tests `m0_bootstrap` 12, `m1_map` 32, `m1_map_subset` 24, `m2_definitions` 14, `m2_movement` 37, `m3_definitions` 11, `m3_economy` 21, `m3_harvest` 10, `m4_combat` 32, `m4_definitions` 12, `m5_placement_preview` 2, `m6_ai` 11, `m6_definitions` 14, and `m6_match` 13.
* These counts match the remediation report.

**Release smoke run:**

* Three matches started: `match on map demo (seed 6)`, then `(seed 7)`, then `(seed 6)`.
* The final line reported 600 frames and 168 ticks, `menu buttons: Some(10)`, `matches started: 3`, `flow: Last`, `views: 16 of 16 units and 2 of 2 buildings`, and `computer player's commands: 2 (0 rejected)`.

**Debug smoke run:** 600 frames and 180 ticks, with the same M6 fields, including `(0 rejected)`.

**Additional runs by the reviewer:**

* `cargo test -p sim --release --test m6_ai --test m6_match --test m6_definitions`: 11, 13, and 14 tests pass.
* `cargo test -p sim [--release] --test m4_combat twenty_against_twenty -- --nocapture`: in both profiles the battle ends at tick 665, with survivors 6 to 0, 451 shots, 34 units destroyed, and final hash `c9401425d786dac6`. This is the same as in review 1.
* `simprobe2`, shipped data, normal against normal: the per-tick digest is `09f9b60baf788477` in release and in debug. This is the value review 1 measured before the remediation, so the claim that the gathering points, courses, and hashes of valid matches are unchanged holds.

## 2. Status of the findings of review 1

| Finding | Status | Evidence |
|---------|--------|----------|
| **P1-1:** start positions are not checked against their map, and the AI falls back to a gathering point off the map | **Resolved** | See below. |
| **P2-1:** the AI file does not ensure that the army or harvester can be produced | **Resolved** | See below. |
| **P2-2:** `uninstall_match` resets (silently) before `close` | **Resolved** | See below. |
| **P2-3:** `a_run_with_another_seed_or_difficulty_diverges` tests neither a seed nor a course | **Resolved** | See below. |
| **P2-4:** the smoke check counts pushed, not accepted, computer-player commands | **Resolved** | See below. |

**P1-1: resolved.**

* `sim::start_problem` and `check_map_starts` (`skirmish_defs.rs`) define the rule. `setup_match` checks every start position before any change (`match_setup.rs:418-430`, `InvalidStart`). The game's `check_catalogue` (`skirmish.rs`) checks every map's starts, then trial-sets up every menu match.
* The `unwrap_or(start)` fallback is gone. `gathering_point` (`ai.rs:323-359`) returns `None`, which becomes `MatchSetupError::NoGatheringPoint`.
* Real binaries, both profiles, all exit 1 with a typed error naming the file and field:
  * review 1's root `p1_start_outside`: `maps[0].starts[1]: value (70, 42), which is outside the 64 x 64 cells of map "demo" …`;
  * a start on water `(40, 20)`: `… impassable terrain …`;
  * a start at `(64, 42)` and a human start at `(15, 99)`.
* A start on sand `(38, 15)`, which is passable but not buildable, exits 1 at startup with `the match of faction "vanguard" against faction "vanguard" cannot be set up: starting building "construction_yard" of player 1 at (38, 15): cell (38, 15) is not buildable`. This also confirms that implementation debt item 7 is resolved for catalogue data.
* The regression test fails without the fix. In a `git archive` copy of `HEAD` with the start check of `setup_match` disabled (`start_problem(..).filter(|_| false)`), `m6_match::a_start_position_that_is_not_a_passable_cell_of_the_map_is_a_setup_error` fails (panic at `m6_match.rs:472`), and the other 12 `m6_match` tests pass.

**P2-1: resolved.**

* `obtainable` and `unobtainable_producer` (`ai_defs.rs`) add the checks; `check_ai_factions` runs in the catalogue and `UnobtainableProducer` in `setup_match`. D42 item 4 is amended.
* Review 1's root `p2_army_unbuildable` now exits 1 in both profiles: `ai.ron: harvester: value "harvester", whose producer "war_factory" is not listed in the build order …`.
* New hostile probes in both profiles all exit 1 with the field named:
  * no barracks: `army[2] … "scout", whose producer "barracks" …`;
  * no refinery: `build_order[2] "war_factory", whose prerequisite "refinery" …`;
  * the factory first: `build_order[0]`;
  * a faction without a yard: `ai.ron: build_order[0]: value "power_plant", whose producer "construction_yard" faction "vanguard" …`;
  * a turret as the power building: `power: … produces no power`.
* Every building type has production terms (`definitions.rs:112-114`, `economy.ron`), so the `if let Some(producer)` in `unobtainable_producer` skips nothing.

**P2-2: resolved.**

* `CommandSink::discard` is the only path that drops pending commands; it logs each one and counts it. `reset` and `close` both use it, and `uninstall_match` closes before it unpauses (`sim_host.rs:213-218`, `315-345`).
* A grep of `pending` in `sim_host.rs` finds no other mutation besides `push`, `take`, and `drain` in `discard`.
* The new test `commands_pending_when_a_match_is_replaced_are_discarded_with_a_log_line` passes.
* For a related path that D47's title also covers, see new finding P2-2.

**P2-3: resolved.**

* The test was renamed to `a_run_with_another_difficulty_takes_another_course`. It keeps the first-hash assertion and adds assertions that player 0's command stream and the match length differ.
* The name lists confirm a rename, not a removal (section 5).

**P2-4: resolved.**

* `watch_events` counts the computer player's `CommandRejected` events in every match, and `smoke_failures` fails on any of them (`smoke.rs:394-407`, `881-888`).
* Both smoke runs report `(0 rejected)`, and the unit test covers 9 of 10 and 1 of 2.

**Implementer claims checked:**

* "Four-neighbour connectivity connects the same cells as the eight-neighbour steps": this holds for terrain. `pathfinding::neighbours` (`pathfinding.rs:59-81`) refuses a diagonal step only when both orthogonal neighbours are not enterable. It does not hold once starting buildings stand on the map; see P2-1 below.
* "1,024 trial setups at most; 64 take a few ms in release": confirmed. Measurements are in section 6.

## 3. New findings

### P0

None.

### P1

None.

### P2

#### P2-1: The gathering point is searched over terrain only, through the computer player's own starting buildings, so it can be a cell none of its units can reach. The code comment and D45 item 3 claim otherwise.

* **File and line:**
  * `crates/sim/src/ai.rs:333` defines `terrain` as `grid.cell(cell).is_some_and(|t| t.passable)`, and line 351 uses it as the only traversal test of the flood fill in `gathering_point` (`ai.rs:323-359`). Cells covered by buildings are traversed; they are only excluded as candidates (`grid.building(cell).is_none()`).
  * `crates/sim/src/match_setup.rs:455` places the starting buildings before `AiController::new` is called (`match_setup.rs:513`). Those buildings are therefore on the grid when the gathering point is chosen. Units cannot enter them: `Grid::is_passable` is false under a building, and `pathfinding::neighbours` uses it as the enterable test.
  * `ai.rs:321-322` says "Its units can walk there unless buildings placed later block the way". `docs/DECISIONS.md:673` (D45 item 3) says the four-neighbour fill "connects the same cells as the eight-neighbour steps of equation 5".
  * `m6_match::every_gathering_point_and_waypoint_of_a_computer_player_is_a_passable_cell_of_the_map` (`m6_match.rs:583`) checks passability, not reachability from the computer player's units.
* **Evidence:**
  * **Map:** `simprobe2` on the demo map with the shipped data, and the terrain around the computer player's start changed (`rock:45,36,63,50; grass:45,42,51,44; grass:52,42,54,44; grass:55,38,62,48; grass:52,45,62,48`). The yard (52..54, 42..44) is the only link between a corridor that leads west to the rest of the map and an eastern pocket in which all 8 of its starting units stand.
  * **Checks pass:** setup succeeds. Every start is a passable cell and every check of D45 and D46 passes.
  * **The gathering point:** it is `(44, 39)`, west of the yard. No unit of the computer player can reach it.
  * **Human player who gives no orders against normal, 36,000 ticks:**
    * outcome `None`;
    * the computer player issues 1,990 commands, mostly one gathering order per turn;
    * 17,874 `MoveFailed { reason: Unreachable }` events;
    * 0 rejections.
  * **Normal against normal:** it ends at tick 13,979, with 5,984 unreachable moves.
  * **Debug and release:** the first 12,000 ticks give an identical digest (`89a4372ecd92da4d`).
  * **Real binaries:** the same map as a `.tmx` in scratch root `plugged` passes the catalogue and the smoke test (`--smoke-test 600`, exit 0 in both profiles) with `computer player's commands: 10 (0 rejected)`. The usual count is 2; the extra commands are the futile gathering orders, and no check notices them.
* **Why P2 and not P1:**
  * No command is rejected and no invariant is violated.
  * The data is a contrived map whose own starting buildings partition the start's region. In this case the computer player's army is walled in whatever its gathering point is.
  * No shipped or fixture map has this shape, and the termination debt is already recorded.
  * The defect is that the rule and its documentation promise reachability for the computer player's units, which the code does not deliver.
* **Expected behaviour:**
  * Flood-fill with the units' rule: passable terrain and no building. Start from the free cells around the starting buildings (or from the cells of the computer player's starting units). If no gathering point can be reached that way, return `NoGatheringPoint`.
  * Alternatively, correct the comment and D45 item 3 and record the limitation as debt.
  * In either case, add a test with a start whose starting buildings separate its units from the point toward the centre.

#### P2-2: D47's title says that nothing the computer player issues is lost unnoticed, but its commands queued in the last tick of a match, or pending when a running match is replaced, are dropped without a log line

* **File and line:**
  * `crates/sim/src/schedule.rs:61-68`: the `Ai` stage runs after `Commands` and before `WinCondition`. A computer player's commands therefore wait in `CommandQueue` until the next tick.
  * `crates/game/src/sim_host.rs:349-372`: `run_tick` stops ticking once an outcome is set.
  * `crates/game/src/sim_host.rs:214`: `uninstall_match` replaces `SimHost`, and so its `CommandQueue`.
  * `docs/DECISIONS.md:687`: the title of D47 is "Nothing a player or the computer player issues is lost or rejected unnoticed". Its item 1 covers only `CommandSink`.
* **Evidence:**
  * `simprobe2` printed the queue after the last tick. After 1 tick it held `[(0, "QueueProduction …"), (0, "AttackMove { units: [SimId(7), …")]`, and after 2 ticks the corresponding commands of player 1.
  * Commands pushed in a tick that ends the match, or in the last tick before the smoke flow or a restart replaces a running match, are never validated. They produce neither a `CommandRejected` event nor a log line.
  * The effect is benign, because the commands belong to a match that is over or abandoned. The class is the same as review 1 P2-2, on the computer player's side.
* **Expected behaviour:** narrow D47's title to the interface's sink, or log the commands left in the simulation's `CommandQueue` when a match ends or is uninstalled.

## 4. Section 5 equations and units, section 6 and 12 invariants, section 8 categories

* **Equations.** The remediation changes no equation of section 5.
  * `gathering_point` orders candidates by `octile_distance` (equation 5's heuristic), then row, then column. It is used only to choose a cell.
  * The new checks are integer comparisons on cells and kinds.
  * Ticks stay `u64` and credits `i64`.
* **Boundaries.**
  * `m6_definitions::start_positions_are_checked_against_their_map` tests (40, 5), (−1, 0), (255, 255), and the map's corners.
  * The reviewer probed (64, 42), (256, 42), (−1, 42), (15, 99), (54, 56), water, and sand on the real binaries.
* **I1:** confirmed. `cargo tree -p sim -e normal` lists no rendering, windowing, input, audio, image, or png crate.
* **I2:** confirmed. The diff adds no float to `sim`. The only float in `sim` remains the pre-existing property check at `map_subset.rs:524`.
* **I3:** confirmed.
  * The digests are equal in debug and release for shipped normal against normal (`09f9b60baf788477`), hard against easy with swapped starts (`ccf2a7776bb691f5`), and the P2-1 map (`89a4372ecd92da4d`).
  * The tests assert equal per-tick hashes.
  * The hash layout is unchanged since review 1 (the 20 against 20 hash is identical).
* **I4:** confirmed. The new code uses `BTreeMap`, `BTreeSet`, and `Vec` only, with no clock in `sim`. The flood fill's choice is ordered (distance, y, x), so the order in which cells leave the stack does not matter.
* **I5:** confirmed. `run_ai` writes only `AiPlayers` and `CommandQueue`. The catalogue's trial setups run in dropped simulations (`sim_host::check_match`). Match setup as setup is pending the user's acknowledgement (user verification item 12).
* **I6 and I7:** checked by the M6 tests (every tick of the build-order test; every 10 ticks of every end-to-end match).
* **I8:** confirmed.
  * In the new code, every index is bounded by a check: `reached[index(start)?]` follows a `contains` check; `skirmish.factions[setup.faction]` and `ai.difficulties[difficulty]` follow the range checks at `match_setup.rs:375` and `394`; `% army.len()` relies on a non-empty army.
  * 60 hostile-input runs (30 roots in both profiles) and 6 more roots all gave typed errors or a clean exit. None panicked, hung, or was killed (section 6).
* **I9:** confirmed. `git diff m5-reviewed..milestone/m6 -- Cargo.toml Cargo.lock crates/*/Cargo.toml rust-toolchain.toml` is empty.
* **I10:** confirmed. No fixture or shipped data value changed in the remediation; `ai.ron` gained comment lines only. The plan's tests use fixture data.
* **I11:** confirmed.
  * Both new files go through `data::read_bounded`, then `data::check` (the RON subset, driven by the schema), and only then `ron::Options::default().with_recursion_limit(16).from_str` (`data.rs:617-628`).
  * In `ron-0.12.2/src/parse.rs:1051-1079`, `skip_ws` treats whitespace and `//` comments as the checker does; block comments, attributes, and escapes are refused by the checker before `ron` sees them.
  * Maps go through `sim::load_map_with_files` and `check_tileset_image`, and only the checked bytes are registered (`game/src/map.rs:102-138`). `CheckedAssetReader::read` (`checked_assets.rs:51-57`) serves only those bytes, so `bevy_ecs_tiled` 0.13.4 and `tiled` 0.16.0 consume only checked bytes.
  * The remediation does not touch `map.rs`, `map_subset.rs`, or `data.rs`.
* **I12:** confirmed. The new arithmetic is saturating (`saturating_add` and `saturating_mul` in `gathering_point`) or bounded by the map side. Identical digests in debug and release.
* **Section 8 categories:**
  * **Positive:** present.
  * **Negative:** present (start off the map or on rock, unobtainable producer, no gathering point, the AI file against the factions, with typed errors and state unchanged where documented).
  * **Boundary:** present.
  * **Regression:** a test exists for each finding of review 1. The P1-1 test was confirmed to fail without the fix.
  * **Integration:** present (`m6_ai` and `m6_match` through `SimPlugin` and `CommandQueue`).
  * **End to end:** present (AI against AI on every shipped map, twice each with equal hashes; `m6_production_matches`; both smoke runs).
* **D43 item 5 (rejections are explained).**
  * The reviewer played 27 further matches on the demo map with 9 other start pairs: (5, 5) and (58, 58); (58, 5) and (5, 58); (10, 57) and (55, 20); (30, 3) and (30, 58); (15, 37) and (27, 37); (52, 42) and (55, 55); (40, 5) and (8, 30); the shipped pair swapped; and others. Each pair ran normal against normal, a passive player against normal, and hard against easy.
  * Every match that could be set up ended with a winner.
  * There was 1 rejection in all, `building 38 does not exist`, which the loss of that building in the tick before explains.
  * Pairs that cannot be set up return typed errors.

## 5. Tests removed, skipped, or weakened, and scope

* **Test names.** The sorted `#[test]` function names were compared at `m5-reviewed`, `bdbaa5e`, and `milestone/m6`.
  * No name was removed since `m5-reviewed`.
  * Since `bdbaa5e`, only `a_run_with_another_seed_or_difficulty_diverges` disappears, renamed to `a_run_with_another_difficulty_takes_another_course` with stronger assertions. Eleven tests are added.
  * There is no `#[ignore]`. The one `should_panic` (`m0_bootstrap.rs:192`) predates M6.
* **Stimulus changes, reviewed:**
  * `m6_definitions::limits_are_accepted_at_both_ends` still loads a 32-entry build order. The entries now include the producers that D46 requires.
  * `sim_host::tests::match_setup_installs_the_loaded_match_or_ends_the_game_with_an_error` additionally asserts the new start-position error and keeps the "starting building" assertion with starts on the small map.
  * Both keep or strengthen their assertions.
* **Scope.** Every remediation change concerns a review 1 finding or its class (start positions, cross-file references, AI fallbacks, discarded and rejected commands). No dependency, manifest, schema version, fixture, map, or sprite changed. `PLAN.md` is unchanged.

## 6. Adversarial runs of the real binaries (debug and release)

Each run used a scratch copy of `assets/` selected with `BEVY_ASSET_ROOT`, ran `--smoke-test 100` (600 where noted), and had a 60 to 600 s alarm. Every run gave the same result in both profiles. None was killed by the watchdog, panicked, or hung.

* **Exit 0:**
  * the unchanged copy (peak 0.40 to 0.41 GB);
  * `start_corner_ok`, with the computer player at (54, 56);
  * `plugged` (600 frames; P2-1).
* **Exit 1, with a typed error naming the file and field, or the map and factions; peak 0.137 to 0.139 GB:**
  * **Review 1 roots:** `p1_start_outside` and `p2_army_unbuildable`.
  * **Start positions:** on water; on sand; starts (64, 42), (256, 42), (−1, 42), (15, 99); a duplicated start.
  * **Skirmish file contents:** a yard overlapping the other yard; a group off the map; a faction without a yard; the `spike` map (its `schema_version` property is missing); the map id `../demo`.
  * **Skirmish file form:** a missing file; a named pipe; a symlink to `/dev/zero`; a directory; 70 KB; a byte order mark; `#![enable(implicit_some)]`; an unknown field; schema version 2; credits of 10^20; 50 levels of nesting.
  * **AI file:** a named pipe; 70 KB; `think_interval` 0; `max_army` 201; an army of harvesters; a power building without power (two cases); no barracks; no refinery; a prerequisite listed later; the factory first.
* **Catalogue size (recorded debt item 4):**
  * **`big16x8` (16 maps of 256 × 256, 8 factions), 1,024 trial setups:**
    * release: 0.78 s, exit 0, peak 0.55 GB;
    * debug: 29.5 s, exit 0, peak 0.55 GB.
  * **`heavy1x8` (8 factions of 16 groups × 64 units, so 2,048 units per setup):**
    * release: 64 trial setups in under 0.1 s;
    * debug: 3.6 s (about 56 ms per setup).
  * **`heavy16x8`, release:** 1,024 such setups and the 16 map loads finish in 1.8 s.
  * The window stays open without content while the catalogue is checked. This is bounded and recorded; it is not a finding.

## 7. Requires user verification (not findings; they block only `m6-approved`)

Run `cargo run --release -p game` with sound on. These are the 13 M6 items of the implementation report; the remediation changes nothing on screen.

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

The earlier pending items (16 for M5, 9 for M4, 10 for M3, 7 for M2, and 5 for M1) remain, with the M6 starting setup noted in the implementation report.

## 8. Recorded debt that must be carried in docs/STATUS.md

**New in this review (P2):**

* **P2-1:** the gathering point's flood fill traverses the computer player's own starting buildings. On a map where they separate its units from the point toward the centre, the gathering point is unreachable for them: one futile gathering order per turn, and 17,874 unreachable moves in 36,000 ticks in the probe.
  * The comment at `ai.rs:321-322` and D45 item 3 claim that the units can reach it.
  * The fix is to flood-fill with the units' rule, or correct the documentation and add a test.
* **P2-2:** D47's title overclaims. Commands of the computer player that wait in `CommandQueue` when a match ends, or when a running match is replaced, are dropped without a log line. Narrow the title or log them.

**From the M6 remediation round 1 report (known debt), confirmed by this review:**

1. Termination is measured, not proven. A computer player that loses a producer (or its yard, which no build order lists) and cannot pay to rebuild it can stall. P2-1 above is a further data-shaped case.
2. Start positions require passable terrain (D45), and the yard must stand on buildable terrain (checked by the trial setup). The sand probe confirms that this case is a typed error.
3. The power building may have no prerequisites (D46).
4. The catalogue check's startup cost grows with maps × factions².
   * Measured worst case: 1,024 setups on 256 × 256 maps take 0.8 s in release and 29.5 s in debug.
   * With 2,048 starting units per setup, they take 1.8 s in release, and the reviewer estimates about 1 minute in debug.
   * The window shows nothing meanwhile.
5. Implementation debt item 7 is resolved for catalogue data; `begin_match` keeps its exit-1 path as a second line.
6. Carried from the M6 review 1 decision: implementation debt items 2 (no randomness), 3 (a ready building waits for a visible spot), 4 (`AttackMove` only), 5 (balance), 6 (ids in the menu; the end screen does not block map clicks), 8 (the terminal log writes the computer player's events), 9 (test time: `m6_production_matches` 52 s, `m6_ai` 40 s, workspace 153 s), and 10 (hotkeys work in the menu).

**Resolved by M6 and confirmed:**

* Review 1 P1-1 and P2-1 to P2-4.
* M0 debt item 3 (the constant seed).
* M3 review 1 P2-4, and D20 item 4, D27 item 3, and D31 item 5 (starting setup in game code).
* M1 review 3 P2-3, first item.
* M3 implementation debt item 2, for computer players only.
* M6 implementation debt item 7, for catalogue data.

**Carried unchanged from the M5 review decision:** every item of its "Recorded debt to carry", namely M5 review 1 P2-1 to P2-5 and M5 implementation debt items 1 to 8, the accepted deviations, the OFL note for M7 packaging, and everything carried from M4, M3, M2, M1, and M0. M6 did not make any of them worse:

* M5 P2-2 (the minimap for W + H > 128) still applies. The M6 catalogue accepts such maps (the reviewer's 256 × 256 roots load and play), but none is shipped. It must be resolved before M7 ships such a map.
* M5 P2-4 (the `m4_combat` temporary-file race) did not occur in this review's debug and release runs.

The linker warning (M0 item 1) appeared again.

## 9. Decision

All six validation commands pass. Every finding of review 1 is resolved and confirmed on the real binaries, and the P1-1 regression test was shown to fail without its fix. No test was removed, skipped, or weakened, and every change lies within M6.

The invariants of sections 6 and 12 hold, and the section 5 equations are unchanged. Debug and release results are identical. Every probe of the hostile and adversarial inputs ended with a typed error or a clean run.

No P0 or P1 was found. The two new findings are P2 documentation and robustness debt, recorded above for docs/STATUS.md. The acceptance items that only the user can verify are listed in section 7; under PLAN.md section 12 item 2 they do not affect this decision.

Decision: GO WITH DOCUMENTED NON-BLOCKING DEBT
