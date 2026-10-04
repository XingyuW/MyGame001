# M7 review 1: Content, balance, performance, and packaging

Independent review under PLAN.md sections 10 and 12. The reviewer did not implement or remediate this code and changed no repository file. Build output went to `target/`. All probes and scratch copies are under `…/scratchpad/m7-review-1-probe/`.

## Scope and base

* **Branch:** `milestone/m7` at `8cdd42e` ("Record the M7 implementation report and decisions D48 to D57"). The working tree was clean before and after the review.
* **Base:** tag `m6-reviewed` (`8f24652`), the most recent reviewed or approved tag.
* **Diff:** `git diff m6-reviewed..milestone/m7`: 53 files, +5,923 / −316, in 5 commits (`51fab5d`, `c8e031e`, `53d0202`, `b1e11a2`, `8cdd42e`).
* **Read in full:** PLAN.md (sections 1 to 12), docs/STATUS.md (status table, the M6 decision, and the whole M7 implementation report; earlier sections for the carried debt), and docs/DECISIONS.md (D1 to D57, U1 and U2).
* **Code read:**
  * the whole `sim` diff: `ai.rs`, `ai_defs.rs`, `command.rs`, `economy.rs`, `economy_commands.rs`, `lib.rs`, `match_setup.rs`, `skirmish_defs.rs`;
  * `crates/sim/examples/m7_benchmark.rs` and `m7_soak.rs`;
  * `crates/game/src/options.rs` (whole file);
  * the diffs of `smoke.rs`, `sidebar.rs`, `minimap.rs`, `skirmish.rs`, `camera.rs`, `audio.rs`, and `main.rs`;
  * the generator diff of `gen_unit_sprites.rs`;
  * every changed test and fixture, plus `m7_factions.rs` and `m7_maps.rs`;
  * `scripts/package_macos.sh`, `README.md`, `assets/LICENSES.md`, and every shipped data file.
* **Environment:** macOS (Darwin 27.0.0), Apple Silicon, Rust 1.98.1 (pinned), with a display and an audio device.

## Validation command results (PLAN.md section 9, section 12 item 5, and the M7 commands)

Every command was run from the repository root, in the foreground, one after the other, on `8cdd42e`.

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings (re-checked `sim` and `game`) |
| `cargo test --workspace` | 0 | 414 passed, 0 failed, 0 ignored. `game`: 122 unit, `m0_spike` 6, `m1_demo_map` 3, `m1_tileset_image` 6, `m2_unit_sprites` 5, `m3_building_sprites` 3, `m6_production_matches` 1 (119.3 s, 108 matches), `m7_maps` 5. `sim`: 11 unit, then 12, 32, 24, 14, 37, 11, 21, 10, 32, 12, 2, `m6_ai` 11 (39.1 s), 14, 13, `m7_factions` 7. 212.6 s wall clock; peak footprint 0.13 GB. Only the known M0 linker warning appeared. |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | three matches on `demo` (`vanguard against vanguard`, twice, then `meridian against vanguard`); 168 ticks; `menu buttons: Some(11)`, `items outside the roster shown: 0`, `options buttons: 11`, `settings saved: true`, `computer player's commands: 2 (0 rejected)`; peak footprint 0.17 GB. The known `Skipped event Destroyed` warning appeared. |
| `cargo run -p game -- --smoke-test 600` | 0 | the same checks pass; 180 ticks; peak footprint 0.17 GB |
| `cargo run --release -p sim --example m7_benchmark` | 0 | Fixture: mean 0.078 ms, longest 0.721 ms, busy mean 0.248 ms over 494 ticks, alive `[200, 147, 34, 28, 28]`, 2,617 shots, 172 destroyed. Shipped: mean 0.158 ms, longest 0.864 ms, busy mean 0.364 ms over 602 ticks, 1,192 shots, 145 destroyed. Peak heap 0.6 MiB. `m7 benchmark: passed`. |
| `cargo run --release -p sim --example m7_soak` | 0 | 44 configurations, each played twice for 36,000 ticks; 3,168,000 ticks in 150.5 s; peak heap 1.6 MiB; `m7 soak: passed` |
| `sh scripts/package_macos.sh` | 0 | `target/package/MyGame001.app` (116 MB) and `MyGame001-macos.zip` (30.5 MB). The packaged executable's smoke test passed from a temporary directory without `BEVY_ASSET_ROOT` or `CARGO_MANIFEST_DIR`. `Contents/MacOS/assets/licenses/FiraMono-OFL.txt` is present. |

### Do the M7 commands measure what section 7 requires, and do they fail when they should?

**Benchmark (`m7_benchmark`):**
* It spawns 200 armed units, 100 per player, in two scenarios (fixture data and shipped data). They attack-move into each other through `CommandQueue`.
* It times 2,000 ticks one by one and reports the mean and the longest tick.
* It also checks the mean over the busy part of the battle (ticks that start with at least 150 units alive).
* The budget is recorded in DECISIONS D55: mean ≤ 5 ms and longest tick ≤ 50 ms, release build.
* **Fails when it should (verified):** a copy built outside the repository with `MEAN_BUDGET = 1 ns` exits 1 and names both scenarios.
* **Limitation:** see P2-4 for how the "moving" part is measured.

**Soak (`m7_soak`):**
* It plays 36,000 ticks of computer players against each other for every shipped map × faction pairing × difficulty, and for the fixture configurations.
* When a match ends early, the next match of the same configuration starts.
* On every tick it checks I6, I7, that every rejection is explained by a loss, and that the state stays within bounds.
* It compares the hashes of two runs on every tick (I3), and a 1 GiB allocator budget bounds memory.
* **Fails when it should (verified):** a copy with `MAX_UNITS = 1` exits 1, with 968 failure lines naming the configuration, the tick, and the counts.

**Package script:** it exits non-zero if the build, the packaging, the smoke test, or `ditto` fails (`set -eu`, with the smoke test run in a subshell). The issues found are in P2-2.

## Verification summary

### Acceptance criteria and required tests of M7

| Item | Verdict and evidence |
|------|----------------------|
| Second faction | **Met.** `meridian` beside `vanguard`, with different rosters (skirmish schema 2). `NotInRoster` validation in `economy_commands.rs:219`. Per-faction AI plans (D49). `m7_factions` (7 tests). The smoke test's last match plays as the Meridian. |
| Full roster | **Met as defined in D50:** 11 unit types and 8 building types (PLAN.md gives no number). Every item is in some roster (production-data test), and every sprite is named by a definition. |
| Three maps | **Met.** `demo`, `crossroads`, `riverlands`. `m7_maps` (5 tests); every map is played in `m6_production_matches`, `m6_ai`, and the soak. **Reviewer probe:** with `crossroads` or `riverlands` listed first in a scratch skirmish file, `--smoke-test 600` passes in both profiles (3 matches each, `settings saved: true`). |
| Final or licensed art and audio, recorded in `assets/LICENSES.md` | **Recorded.** Every file under `assets/` is in `LICENSES.md` (checked by the production-data test). The only third-party file is the OFL text, whose SHA-256 (`587dd954…ecbe53`) is identical to `bevy_feathers-0.19.1/src/assets/fonts/FiraMono-LICENSE`. Bevy's `CREDITS.md` names FiraMono under OFL 1.1. The sprites are procedural (inspected). Whether they count as "final" is user verification. |
| Balance pass | **Met.** D51. **Reviewer probe:** an independent program built outside the repository on the same `Cargo.lock` played all 108 pairings with the shipped data and reproduced every D51 number: 108 of 108 end with a winner; longest 29,984 ticks; mean end ticks demo 12,973, crossroads 12,448, riverlands 9,959; faction wins 26 and 28; 9 and 9 at equal difficulty; the harder difficulty won 57 of 72; mirror seats 9 and 9. |
| Options | **Met.** D52. Options screen, settings file, and own bounded parser. **Reviewer probes:** see "Hostile inputs" below; one write-side defect is P2-1. |
| Release builds | **Met.** Release binary and package. The package's smoke test passes. Bundle signature and architecture: P2-2. |
| README with controls | **Present.** Spot-checked against the data: sell refund is half the cost and the placement distance is 4. On-screen accuracy is user verification. |
| Test: every production data file loads and passes cross validation | **Met.** `skirmish::tests::every_production_data_file_loads_passes_cross_validation_and_is_licensed`. |
| Test: benchmark with 200 moving and fighting units against the DECISIONS budget | **Met.** See above, and P2-4. |
| Test: soak of 36,000 ticks of AI against AI | **Met.** |
| Test: the release build passes the smoke test | **Met.** Both `cargo run --release` and the packaged executable. |
| Acceptance: version 1.0 (section 1 as amended by section 12) | **Met in every automatable part:** a macOS executable (release binary and `.app`); skirmish on 3 maps; 2 factions; an AI opponent at 3 difficulties; the win and lose condition of D41 with the end screen of D44 (M6). Playing complete matches from the menu is user verification. |

### Test categories of section 8

* **Positive:** roster loading, plans, the options round trip, maps.
* **Negative:**
  * roster entries that are unknown, unproducible, duplicated, too many, or missing;
  * `NotInRoster` with an unchanged state hash;
  * eight AI plan cases;
  * 19 malformed settings cases;
  * the reviewer's hostile probes.
* **Boundary:**
  * an empty roster;
  * settings at exactly 4,096 bytes;
  * every option at both ends;
  * minimap sizes 1 × 1 to 256 × 256 across W + H = 128 and 129;
  * the roster limit tested at 73 entries, not exactly 65 (P2-6).
* **Regression:** M5 review 1 P2-2 (minimap test) and P2-4 (one file per call).
* **Integration:** `m7_factions` through `setup_match`, `SimPlugin`, and `CommandQueue`.
* **End to end:** 108 production matches, the soak, the benchmark, and the smoke runs. The smoke test covers only the first map (P2-5).

### No test removed, skipped, or weakened

* Test function names went from 395 at `m6-reviewed` to 415. Every earlier name is still present (`comm -23` is empty), and no `#[ignore]` was added.
* The changed tests are:
  * `m6_definitions` (the schema-version case now expects version 2, and a version-1 case was added);
  * `m6_production_matches` (27 → 108 matches, plus an assertion on the count);
  * `m4_combat::extreme_scenario` (only the file name changed);
  * `loading_rejects_a_start_position_outside_its_map` (the edit strings changed to match the M7 data; the assertions are unchanged);
  * `first_queueable` (now checked for every faction, and stronger).
* None of these is weakened.

### Invariants (sections 6 and 12)

| ID | Evidence |
|----|----------|
| I1 | `cargo tree -p sim -e normal` has no rendering, windowing, input, or audio crate. `Cargo.lock` and the crate manifests are unchanged (`git diff` is empty). |
| I2 | No new `f32`/`f64` in `crates/sim/src` (the only hit is the M1 `map_subset.rs:524` parse check). The examples use `f64` for printing only. |
| I3 | Rosters and `AiPlan` are hashed (`economy.rs` hash of the roster after the vision; `ai.rs` `hash_into`). The soak checks equal hashes on every tick of two runs for 44 configurations. |
| I4 | Rosters are `BTreeSet`s, and plans are filtered in file order. `the_game_systems_share_no_state_without_an_order` now covers `OptionsPlugin` and the `Last` schedule. |
| I5 | The options touch only game resources. Rosters are set by match setup; during a match, state changes only through `CommandQueue`. |
| I6, I7 | Checked on every tick of the soak (3.17 M ticks). |
| I8, I11 | The new skirmish fields go through the RON subset checker. Probes (below) gave typed errors in both profiles. The settings file uses the game's own parser, with no third-party hand-off. |
| I9 | No dependency change. `[profile.dev.package.sim] opt-level = 3` is a build setting. |
| I10 | The plan tests and one benchmark scenario use fixtures. Production tests check properties of the shipped game. |
| I12 | No new overflow-prone simulation arithmetic (`len() as u64` only for hashing). `cargo build -v -p sim` in the dev profile passes `-C opt-level=3 -C debug-assertions=on`, so overflow checks stay on. |

### Equations and units of section 5

M7 changes no equation, unit, or arithmetic path of section 5. Equation 3 is unchanged: the roster check comes before any charge, and a rejected command changes nothing (state hash compared in `m7_factions`). Equations 1, 2, and 4 to 7 are unchanged, and their M2 to M6 tests pass.

### Hostile inputs in the real binaries (both profiles; each run under `perl alarm`; nothing was killed by the memory watchdog)

**Skirmish data, through scratch asset roots (`BEVY_ASSET_ROOT`):**

| Input | Result (debug and release) |
|-------|----------------------------|
| 71-entry roster | exit 1: `factions[0].roster_units: more than 64 entries` |
| 5,000-deep `[[[…]]]` nesting inside a roster | exit 1: `roster_units[0]: line 34, column 17: expected a string, found '['` |
| 65-byte roster string | exit 1: `value of 65 bytes is not supported, expected a string of at most 64 bytes` |
| Faction without `power_plant` | exit 1, from `ai.ron`: `power: … which faction "meridian" does not offer` |
| Faction with empty `roster_units` | exit 1, from `ai.ron`: `army[0]: … which no faction of the skirmish file offers` |
| Duplicate roster entry | exit 1: `DuplicateId` |
| Schema version 1 file | exit 1: `schema version 1 is not supported, expected 2` |
| 100 KB file | exit 1: `file: the file is larger than 65536 bytes` |
| 256 × 256 map listed first | `--smoke-test 600` exit 0; minimap lit 1,892 / 2,116 pixels. This confirms M5 review 1 P2-2 is resolved in the real binary. |

**Settings file, normal runs with `HOME` pointing at scratch directories:**

* **Inputs:** a named pipe, 10 MB, a symlink to `/dev/zero`, a directory, mode 000, random bytes, `master_volume = 99`, a symlink loop, the parent path being a file, a valid file, and no `HOME`.
* **Result:** each case logs a typed warning naming the file, line, key, and reason (or "HOME is not set") and keeps running in both profiles. The runs were ended by the alarm (exit 142). The hostile files were left unchanged.

**Settings file, smoke-mode write path:** the directory `$TMPDIR/mygame001-smoke-<pid>` was prepared and the game was `exec`'d with the same PID.

* An invalid `settings.txt` is replaced, with a log line, and the run passes.
* `settings.txt` as a directory, or a read-only directory: `cannot write` is logged, and the smoke test fails with exit 1 and a message. This is correct.
* A named pipe or a symlink at `settings.txt.tmp`: **P2-1**.

## Findings

No P0 and no P1 findings.

### P2-1: The settings file's write path follows a symlink and blocks on a named pipe at `settings.txt.tmp`; the read path rejects both

* **File and line:**
  * `crates/game/src/options.rs:353-360` (`write_settings`): `std::fs::write(&temporary, …)` on the fixed name `settings.txt.tmp`, then `rename`.
  * The read side (`options.rs:312-331`) rejects non-regular files before opening, as D52 item 2 states. It also leaves a check-then-open window between `metadata` and `File::open`.
* **Evidence (debug and release):**
  * **Named pipe at `…/mygame001-smoke-<pid>/settings.txt.tmp`:**
    * The last log line is "no settings file … the defaults are used".
    * The process then blocks in `save_settings` (schedule `Last`) until the 40 s alarm kills it (exit 142).
    * There is no error and no smoke result.
    * In a normal run the same file at `~/Library/Application Support/MyGame001/settings.txt.tmp` would freeze the game the first time the player changes an option.
  * **Symlink at `settings.txt.tmp` pointing to a scratch `victim.txt`:**
    * The run passes (exit 0) and logs "settings written to …".
    * `victim.txt` now holds the settings text, so the file outside the settings directory was overwritten.
    * `rename` then replaces `settings.txt` with the symlink itself.
* **Why P2:**
  * Only a hostile file in the user's own Application Support directory triggers it. A leftover regular `.tmp` file from a crash is simply overwritten.
  * No privilege boundary is crossed.
  * It is the same class and severity as M1 review 3 P2-1 (a named pipe at the map path hangs at startup).
* **Problem with the documentation:** D52 item 3 ("No silent fallback or loss") and the module documentation describe only the read side.
* **Expected behaviour:**
  * Create the temporary file with `OpenOptions::new().write(true).create_new(true)` (O_EXCL, which does not follow a final symlink and never opens an existing pipe).
  * Use a unique name, or remove a stale entry first after checking with `symlink_metadata` that it is a regular file.
  * Report a failure as the existing typed write error.
  * Add a test with a named pipe and a symlink at the temporary path.

### P2-2: The release package's bundle signature does not verify, the executable is arm64 only, and the README's advice for other Macs is unverified

* **File and line:**
  * `scripts/package_macos.sh:27` copies the linker-signed binary into a bundle and never re-signs it.
  * `README.md:58`: "on another Mac, open it with a right click and **Open** the first time".
  * `docs/DECISIONS.md` D54 item 3 says Gatekeeper "asks on first open on another Mac".
* **Evidence:**
  * `codesign -dv target/package/MyGame001.app` reports `flags=0x20002(adhoc,linker-signed)`, `Info.plist=not bound`, `Sealed Resources=none`.
  * `codesign --verify` and `spctl --assess` both report "code has no resources but signature indicates they must be present".
  * `file` reports `Mach-O 64-bit executable arm64`.
  * A scratch copy re-signed with `codesign --force --deep -s -` verifies ("valid on disk", "satisfies its Designated Requirement").
* **Why P2:**
  * The version 1.0 target is the user's Mac (U1, PLAN.md section 12 item 3). A locally built bundle carries no quarantine attribute and opens.
  * D54 item 3 and user-verification items 9 and 10 already leave signing and distribution to the user.
  * The defect is that the package as built has an invalid bundle signature and an unverified claim about other Macs.
* **Expected behaviour:**
  * Ad-hoc sign the assembled bundle in the script and have the script fail unless `codesign --verify` passes.
  * State that the package is arm64 only, or build a universal binary.
  * Verify the README instruction on a second Mac, or reword it.

### P2-3: STATUS lists M6 implementation debt item 9 (test time) as resolved ("reduced"), but test time grew

* **File and line:** `docs/STATUS.md:2435` ("M6 implementation debt item 9, test time, reduced (D53)"); D53.
* **Evidence:**
  * M6 debt item 9 recorded `m6_production_matches` 52 s, `m6_ai` 40 s, and workspace 153 s (`STATUS.md:2273`).
  * On M7: `m6_production_matches` 119.3 s, `m6_ai` 39.1 s, and `cargo test --workspace` 212.6 s (this review) or 189 s (implementer).
  * The time per match fell, but the debt (the time the suite takes) is larger.
* **Expected behaviour:** carry item 9 as open with the M7 numbers, or say "time per match reduced; total test time 189–213 s".

### P2-4: The benchmark does not show that 200 units moved, and its headline mean is mostly post-battle ticks

* **File and line:** `crates/sim/examples/m7_benchmark.rs:271-274` (`moved` counts only units that survive the window) and `:431` (the pass criterion is `moved > 0`).
* **Evidence:**
  * The fixture scenario reports "28 units moved" out of 200.
  * Units alive are `[200, 147, 34, 28, 28]`, so about 1,000 of the 2,000 measured ticks run with at most 34 units.
  * The reported mean (0.078 ms) is about a third of the busy-part mean (0.248 ms).
  * The plan asks for "200 moving and fighting units".
* **Why P2:**
  * The busy-part mean is also held to the budget, and it stays far inside it (0.25 and 0.36 ms against 5 ms, a margin of more than 13×).
  * The scenarios force movement: the anchors are about 23 cells apart and every weapon range is at most 16 cells.
  * The budget decision therefore stands; only the evidence is weaker than its description.
* **Expected behaviour:**
  * Count units that ever changed position, including destroyed ones, and require that most of the 200 did.
  * Either end the measured window when the battle ends, or label the headline mean as the window mean.

### P2-5: The automated end-to-end runs of the real binary start matches only on the first map

* **File and line:** `crates/game/src/smoke.rs:188` onward (`play_flow`): every match of the flow uses `MenuSelection`'s map 0 (`demo`). `MOVE_TARGET` (`smoke.rs:69`) is a demo cell.
* **Evidence:**
  * `crossroads` and `riverlands` reach `bevy_ecs_tiled` and the renderer only in manual play. The catalogue checks their files at startup, and `m7_maps` and the simulation tests cover them without rendering.
  * The reviewer reordered the maps in scratch roots, and `--smoke-test 600` passed on each new map in both profiles. The code path works, but section 9's commands do not cover it.
* **Expected behaviour:** let the smoke flow's three matches use three different maps (for example, Restart on the next map and the last match on the third), with a move target valid on each.

### P2-6: The roster-limit boundary is tested at 73 entries, not at the limit

* **File and line:** `crates/sim/tests/m7_factions.rs:254-265`. The case appends 64 entries to a 9-entry list.
* **Evidence:** the report lists a "64-entry limit" boundary test, but no case loads exactly 64 or rejects exactly 65. A 65-entry list of repeated names reaches `TooManyEntries` before uniqueness, so the exact case needs no extra definitions.
* **Expected behaviour:** add the exact 65-entry rejection, or correct the report's wording.

### Observation, not a finding

* The AI file keeps schema version 1 although the meaning of its lists changed: entries are now filtered by roster, and dead entries are rejected.
* D49 item 3 records and justifies this. The fields are unchanged, and the shipped and fixture files were updated and checked.

## Requires user verification (not findings; blocks only `m7-approved`, PLAN.md section 12 item 2)

1. The 12 M7 items of the implementation report (STATUS "Requires user verification"):
   * art and audio as final;
   * the second faction's sidebar;
   * new units in play;
   * the maps on screen (minimap, fords);
   * options persisting across restarts;
   * balance feel;
   * version 1.0 acceptance: lose to and defeat the computer player on each map with either faction, started from the menu;
   * the `.app` opening by double click;
   * signing and notarisation;
   * release notices and version 1.0.0;
   * the README controls;
   * the settings file location.
2. **Third-party crate notices:** the licence notices of the Rust crates compiled into the executable (MIT, Apache-2.0, and others) are not shipped with the package (D54 item 3). Decide whether they must ship before any distribution.
3. **Distribution to other Macs:** the package is arm64 only, and its bundle signature does not verify as built (P2-2). Decide whether other Macs (including Intel Macs) are targets, and check the README advice there if so.
4. **Naming (PLAN.md section 3 item 7):**
   * Several unit and building names are generic military terms that also appear in Electronic Arts' Command & Conquer series: construction yard, war factory, tech center, refinery, harvester, conscript, grenadier, rocket soldier, gun turret.
   * No EA asset or trademark logo is present, and every sprite is procedural.
   * Confirm that these generic names are acceptable.
5. The earlier pending items (13 for M6, 16 for M5, 9 for M4, 10 for M3, 7 for M2, 5 for M1). With M7 data the starting setup differs from what they describe (STATUS M7 report, last paragraph).

## Recorded debt that must be carried in docs/STATUS.md

**New in this review (P2):**

* **P2-1:** the settings write path follows a symlink and blocks on a named pipe at `settings.txt.tmp` (`options.rs:353-360`). Fix: create the temporary file with `create_new` and a unique name, and add a test.
* **P2-2:**
  * The package's bundle signature does not verify (`codesign --verify`: "code has no resources but signature indicates they must be present").
  * The executable is arm64 only.
  * The README's right-click advice for other Macs is unverified.
  * Fix: ad-hoc sign and verify in the script, and document the architecture.
* **P2-3:** M6 implementation debt item 9 (test time) is not resolved. The workspace now takes 189–213 s (M6: 153 s), and `m6_production_matches` 119 s. Carry it as open.
* **P2-4:** the benchmark's `moved` counts only survivors, its pass criterion needs one moving unit, and its headline mean is diluted by post-battle ticks. The busy-part mean (0.25 / 0.36 ms) is the meaningful figure.
* **P2-5:** the smoke flow starts matches only on the first map. The two new maps reach the renderer only in manual play (the reviewer's reordered-root probes pass).
* **P2-6:** the roster limit is tested at 73 entries, not exactly 65. The report's "64-entry limit" boundary claim overstates the test.

**New in M7, confirmed by this review (from the implementation report's known debt):**

1. The package is not signed or notarised, and the crate licence notices are not shipped (D54 item 3). See also P2-2 and user verification items 2 and 3.
2. The package is 116 MB; the executable is not stripped.
3. The art is procedural; its acceptance as final art is the user's (D50 item 3).
4. Balance is measured only between computer players (D51 item 5). Reproduced exactly by the reviewer's probe. Termination is still measured, not proven.
5. Dock contention (M3 review 1 P2-2) remains (D57 item 4).
6. The end rule's speed dependence in jams (M2 review 3 P2-1) remains (D57 item 5).
7. The benchmark does not measure the rendering side (M5 implementation debt 6). The minimap image of the largest map is 1,024 × 512. A 256 × 256 smoke run passed in both profiles.
8. The menu shows ids, not display names (M6 debt item 6).
9. The crate version stays 0.1.0.
10. `scripts/package_macos.sh` needs a display.

**Resolved by M7 and confirmed:**

* M0 debt item 7 and D1's packaging note: the package smoke test passes without the cargo variables.
* The OFL note of the M5 review: the OFL text is byte-identical to the pinned source and ships in the package.
* M5 review 1 P2-2: minimap scale of at least 2, a test, and a 256 × 256 map in the real binary.
* M5 review 1 P2-4: one file per call.
* M3 review 1 P2-3: `later_commands_of_a_tick_see_the_buildings_placed_before_them` exists, and the citation is corrected.
* D38 item 5: the options are stored.
* M6 implementation debt item 5: the balance pass (D51, reproduced).
* The measurements deferred to "the M7 benchmark" (M2 round 2 item 3, M3 debt 4, M4 debt 1).

**Not resolved, despite the report:** M6 implementation debt item 9 (P2-3).

**Still open from earlier milestones (unchanged by M7, as listed in the M7 report):**

* M6 review 2 P2-1 and P2-2.
* M6 remediation debt items 1 to 4.
* M6 implementation debt items 2, 3, 4, 6, 8, 9 (see P2-3), and 10.
* M5 review 1 P2-1, P2-3, and P2-5, and M5 implementation debt items 1 to 8.
* M4 review 2 P2-1, and the M4 remediation and implementation debt.
* M3 review 1 P2-1 and P2-2, and M3 implementation debt items 1 to 3, 6, and 9.
* M2 review 3 P2-1 and P2-2, M2 remediation round 2 items 1 to 7, and the M2 implementation debt.
* M1 review 3 P2-1 (named-pipe hang of the map and `.tsx` reader; P2-1 above is the same class on the settings write side), P2-2, and the rest of P2-3; review 1 P2-3 and P2-5; review 2 P2-B; round 1 item 4; round 2 items 5 and 6.
* M0 items 1, 2, 5, 8, and 9. The linker warning (M0 item 1) and the `Skipped event Destroyed` warning (M0 item 2) were seen again in this review.

## Decision

* There are no P0 or P1 findings, and every validation command (section 9, section 12 item 5, and the three M7 commands) exits with code 0.
* Every automatable acceptance criterion of M7 and of version 1.0 has evidence.
* There is no unresolved numerical or data-integrity risk and no compatibility break: the skirmish schema was incremented, with the data and fixture updated in the same change.
* The six P2 items above are non-blocking and must be recorded in docs/STATUS.md.

Decision: GO WITH DOCUMENTED NON-BLOCKING DEBT
