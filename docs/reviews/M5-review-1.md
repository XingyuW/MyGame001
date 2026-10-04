# M5 review 1: Interface and audio

## Scope and base

* **Repository:** `/Users/xingyuwang/GameProjects/MyGame001`, branch `milestone/m5` at `522f70b`. The working tree was clean before and after the review. No repository file was created, modified, staged, or committed.
* **Base:** tag `m4-reviewed` (`386d8af`). The cumulative diff is `git diff m4-reviewed..milestone/m5`: 21 files, +5,541 / −347.
  * `crates/sim`: `economy_commands.rs`, `lib.rs`, and the new `tests/m5_placement_preview.rs`.
  * `crates/game/src`: new `audio.rs`, `health.rs`, `hotkeys.rs`, `minimap.rs`, `sidebar.rs`, and `sim_host.rs`; modified `buildings.rs`, `camera.rs`, `construction.rs`, `fog.rs`, `main.rs`, `map.rs`, `selection.rs`, `smoke.rs`, and `units.rs`.
  * `assets/LICENSES.md`, `docs/DECISIONS.md`, and `docs/STATUS.md`.
* **Read in full:** PLAN.md (including section 12), docs/DECISIONS.md, the M4 review decision, and the M5 implementation report in docs/STATUS.md.
* **Code read:** every changed source file. Third-party code was read where M5 depends on it:
  * `bevy_ecs` 0.19.1: `World` accessors on `&self` and `EntityAllocator`.
  * `bevy_text` 0.19.1: the embedded default font.
  * the `bevy` 0.19.1 `CREDITS.md`.
  * `cargo tree` feature resolution for `parley`.
* **Reviewer:** an independent session that did not write or remediate this code.

## Validation command results (PLAN.md section 9, with section 12 item 5)

The commands were run from the repository root, in the foreground and in order, on macOS (Apple M5, Rust 1.98.1), with a display and an audio device.

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 342 passed, 0 failed, 0 ignored. `game`: 101 unit tests, plus 6 + 3 + 6 + 5 + 3 integration tests. `sim`: 11 unit tests, plus 12 + 32 + 24 + 14 + 37 + 11 + 21 + 10 + 32 + 12 + 2 integration tests. The known M0 linker warning appears. |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | See the release smoke output below. Peak footprint 0.145 GB. |
| `cargo run -p game -- --smoke-test 600` | 0 | See the debug smoke output below. Peak footprint 0.143 GB. |

Release smoke output: `smoke test: 600 frames, 188 simulation ticks, map created: true, units spawned: 26, units moved: 18 …, production charged 376 credits …, visible cells: 391, enemy views shown: 0, view mismatches: 0, sidebar items: 9 of 9, sidebar buttons outside the window: 0, sidebar credits: Some("Credits  4624"), minimap drawings: 189, minimap pixels lit: 2260, sounds: 9, music players: 1, pause: Done { ticks_while_paused: 0, refused: true }`. This is identical to the implementer's report.

Debug smoke output: 196 ticks, `Credits  4608`, minimap drawings 193, pause `Done { ticks_while_paused: 0, refused: true }`, all checks passed.

The known `Skipped event Destroyed` warning (M0 debt item 2) appeared in both smoke runs.

**Test inventory.**

* All 307 test functions present at `m4-reviewed` still exist (`comm -23` of the test names is empty). 35 tests were added.
* No `#[ignore]`, `#[should_panic]`, or `cfg(not …)` was added.
* No removed line in `crates/` contains an assertion or a test attribute.
* The changes to existing test modules are imports only (`construction::tests`).
* Manifests, `Cargo.lock`, the toolchain file, and `.cargo/config.toml` are unchanged.

**Additional runs by the reviewer.**

1. `cargo test --release -p sim --test m5_placement_preview --test m4_combat`, then `m4_combat` in release 15 more times:
   * 3 of 16 release runs failed (see P2-4).
   * `m4_combat` in debug 20 times: 0 failures.
2. Hostile and boundary probes. Each ran the real binaries `target/{debug,release}/game` directly against scratch asset roots under `…/scratchpad/m5-review-1-probe/roots/`, selected with `BEVY_ASSET_ROOT`. Each probe ran under `perl -e 'alarm N'` and was measured with `/usr/bin/time -l`. No probe was killed, and none exceeded 0.28 GB peak footprint. Results were the same in debug and release:

| Root | Change | Debug | Release |
|------|--------|-------|---------|
| `base` | unchanged copy, `--smoke-test 600` | exit 0, all M5 checks pass | exit 0, all M5 checks pass |
| `nocombat` | `data/combat.ron` deleted | exit 1, `data file …/combat.ron: file: cannot read the file: No such file or directory` | same |
| `garbagemap` | `demo.tmx` = `not a map` | exit 1, `map …/demo.tmx: file: not a valid Tiled map: text before the root element is not supported` | same |
| `smallmap` | valid 20×20 map (from the demo tiles) | exit 1, `ERROR game::sim_host: starting building "construction_yard" of player 0 at (15, 37): the 3x3 footprint at (15, 37) does not lie inside the map`; no panic in the sidebar, minimap, or audio systems after the failed setup | same |
| `emptyaudio` | 100 KB of random bytes at `assets/audio/music.ogg` | exit 0; the file is never read (D16 item 3, D38 item 2) | same |
| `bigmap` | valid 256×256 map (the demo map tiled 4×4), `--smoke-test 600` | exit 0, 283 ticks, peak 0.276 GB, minimap pixels lit 566 | exit 0, 189 ticks, peak 0.277 GB, minimap pixels lit 566 |

3. Minimap coverage probe (`minimap_cover.rs`). It is a verbatim copy of `MinimapLayout::{new, sum, size, cell_coordinates, pixel_cell}` with `Vec2` replaced by `(f32, f32)`. It counts the cells that at least one minimap pixel shows (see P2-2).

## Verification of the milestone (PLAN.md section 7, M5)

| Item | Verdict | Evidence |
|------|---------|----------|
| Sidebar with credits and power | Verified (automatic part) | `sidebar.rs` texts; `item_status_follows_the_players_buildings_and_queue` runs through `SimHost::step`: `Credits  4700` and `Power  0 / 20` after a power plant. In the smoke runs the credits text equals the player's credits. The on-screen result requires user verification. |
| Build tabs, queue progress | Verified | Covered by `badge_text`, `progress_percent`, and `item_status`. Tested at 50 % with `+1` queued and at `READY`, through the simulation. |
| Placement preview with validity colouring | Verified | `sim::check_placement` is the exact sequence that `PlaceBuilding` validation now calls; the refactor preserves order and reasons (diff read). `m5_placement_preview` covers 262 origins, including `i32` extremes and all six placement rejections, through `SimPlugin` and `CommandQueue`, and the preview leaves the hash unchanged. The game side uses `placement_preview` and `mode_click`. Colours require user verification. |
| Minimap with fog and camera rectangle | Verified with a defect (P2-2) | The layout, fog colours, shown-only markers, and camera frame are tested. Markers are never drawn on half of all cells for maps with W + H > 128 (P2-2); the shipped 64×64 demo map is fully covered. |
| Health bars | Verified (automatic part) | `bar_drawn` draws only over shown views; `health_fraction` is checked at its boundaries. |
| Control groups, hotkeys | Verified | One key table (`hotkeys::hotkey`); groups 0 and 9 at the boundaries; pruning of dead units. |
| Pause | Verified | `run_tick` does not tick while paused, and the sink refuses commands. The smoke runs show 0 ticks in 30 paused frames and a command refused. P2-3 concerns a mode side effect. |
| Sound effects, music, volume | Verified (automatic part) | Sounds are synthesised (finite, bounded, deterministic). `event_sound` is tested for every `SimEvent` variant. Effects are bounded; volume levels are tested at 0 and 10. Audible output requires user verification. |
| Test: interface→`Command` functions are pure and tested for every variant | Verified | `every_command_variant_is_built_by_a_pure_interface_function` uses a wildcard-free `match` over all ten variants, plus per-function tests. |
| Test: the interface cannot obtain mutable access to simulation state (type or module boundary) | Verified, with a documentation caveat (P2-5) | The simulation `World` is a private field of `SimHost`. Only `&World` is public. There is no public constructor or `&mut self` method, and `#![forbid(unsafe_code)]` applies. `the_game_world_holds_no_simulation_state` passes. No sim resource or component has interior mutability (grep for `Cell`, `RefCell`, `Mutex`, `RwLock`, `Atomic`, `OnceLock`: none). All 14 sim components are registered before the first query. |
| Test: smoke test passes with the full interface loaded | Verified | Both profiles pass, with every M5 plugin and the D6 M5 checks. |
| Acceptance: a full match can be played by a human without debug shortcuts | Requires user verification | The interim keys `1`–`9` are gone; every action is reachable from the sidebar and mouse. |
| Section 8 categories | Covered | Positive, negative, boundary, integration (`m5_placement_preview`, host tests through `SimSchedule`), and end-to-end (both smoke runs). The regression category does not apply because no review or play defect preceded this round. |
| Section 5 equations and units | Unchanged in `sim` | The only `sim` change is the extraction of `check_placement`, with no arithmetic change. Equation 7 is used by the minimap; its inverse and the axis-aligned frame are checked by `the_layout_fills_the_image_with_the_maps_bounding_rectangle`. |
| Invariants | Hold | See below. |

**Invariants.**

* **I1, I9:** manifests and lock file unchanged. `cargo tree -p sim -e normal` contains no forbidden crate.
* **I2:** no new float in `sim`; the only float is the recorded M1 debt at `map_subset.rs:524`.
* **I3, I4:**
  * `sim` behaviour is unchanged.
  * Game systems are ordered: `the_game_systems_share_no_state_without_an_order` covers Startup, FixedUpdate, Update, and PostUpdate.
  * The host runs exactly one `SimSchedule` per fixed step.
* **I5:** enforced by `SimHost` and `CommandSink` (D34). P2-1 is a wiring defect that issues an unintended order, but through `CommandSink`, not by a direct state write.
* **I6, I7:** unchanged in `sim`; all M2 to M4 tests pass.
* **I8:** the new game code has no `unwrap`, `expect`, or panic outside tests; control-group indexing is clamped. The hostile probes ended with typed errors and exit code 1.
* **I10:** tests use fixture data. The sidebar and smoke tests that read production data assert only labels and costs taken from the data or from its structure.
* **I11:** no new external input. Audio is synthesised and the font is embedded. A junk `.ogg` under `assets/audio` is never read (probe).
* **I12:** no new simulation arithmetic.

**Licences (section 11).**

* No new file under `assets/`.
* The FiraMono subset compiled into the binary by `bevy_text` 0.19.1 (`src/FiraMono-subset.ttf`, `DEFAULT_FONT_DATA`, feature `default_font`) is recorded in `assets/LICENSES.md` as OFL 1.1, with authors matching `bevy-0.19.1/CREDITS.md`.
* `parley` is built with `std` only, so no system font is discovered.
* The synthesised sounds are recorded.

**API and schema changes.** One new public function, `sim::check_placement`, is documented in D35 item 5. There is no data schema change and no `schema_version` change is needed.

## Findings

There are no P0 or P1 findings.

### P2-1: A right click on the sidebar or minimap can order an attack on an enemy drawn behind the sidebar

* **File and line:**
  * `crates/game/src/selection.rs:353-357`: `issue_orders` gates the right click only on `*mode == InterfaceMode::Normal`. It then computes `cursor_world(&window, …)` from the raw cursor position (`selection.rs:223-224`) and calls `enemy_at(point, &shown_enemy_units(&views), building)`.
  * There is no `over_sidebar` check. `over_sidebar` is applied only in `map.rs:222`, to the hovered cell; in `map.rs:246-249`, to the picking log; and in `selection.rs:269`, to the left press.
* **Evidence:**
  * The camera draws the map under the whole window, and the sidebar is a 0.96-alpha UI overlay. An enemy unit view standing behind the sidebar therefore keeps `Visibility::Inherited` and is in `shown_enemy_units`.
  * Suppose armed units are selected and the player right-clicks a sidebar item to cancel production (`sidebar.rs:610-616`) or right-clicks the minimap, and that point lies over such an enemy's body. Then `enemy_at` returns the enemy, and `attack_commands(&selection.0, &armed, target, None)` pushes `Command::Attack` into the sink (the acknowledgement sound plays) alongside the `CancelProduction`.
  * This contradicts D35 item 7 ("clicks on the sidebar never act on the map", `docs/DECISIONS.md:556`) and user verification item 8 ("with the cursor over the sidebar … clicks do not select or order", `docs/STATUS.md:1736`). No test covers right clicks over the sidebar.
* **Expected behaviour:** a right click whose cursor is over the sidebar issues no map order. Gate `issue_orders` (and `cursor_world`) with `over_sidebar`, or derive the enemy from a cursor point that is `None` over the sidebar. Add a test of that decision, for example a pure function that takes the over-sidebar flag.
* **Severity rationale:** P2. Simulation state, validation, and the invariants are unaffected: the command passes through `CommandSink` and is a valid, visible-target order. The effect is an unintended user-facing order, and an inaccurate claim in D35 and the report.

### P2-2: On maps with width + height > 128 the minimap never draws half of all cells, so units and buildings on them are not marked

* **File and line:**
  * `crates/game/src/minimap.rs:74`: `scale: (MINIMAP_TARGET_WIDTH / sum).max(1)`.
  * `minimap.rs` `pixel_cell` and `render`: one pixel shows exactly one cell.
  * `minimap.rs:237`: unit markers are drawn at `mobile.cell`.
  * `minimap.rs:514`: `every_pixel_shows_the_cell_under_it` asserts that every cell is drawn only `if layout.scale >= 4`.
* **Evidence:** the probe `minimap_cover.rs` (verbatim copy of the layout code) gave these counts:

  | Map | Scale | Image | Cells drawn |
  |-----|-------|-------|-------------|
  | 64×64 | 2 | 256×128 | 4,096 of 4,096 |
  | 32×32 | 4 | 256×128 | all |
  | 100×100 | 1 | 200×100 | 5,000 of 10,000 |
  | 128×128 | 1 | 256×128 | 8,192 of 16,384 |
  | 200×56 | 1 | 256×128 | 5,600 of 11,200 |
  | 256×256 | 1 | 512×256 | 32,768 of 65,536 |

  At scale 1 the first missing cells are (1, 0), (3, 0), (5, 0), (7, 0): every cell with odd x + y. A cell's diamond is 2 × 1 pixels there, and pixel centres fall only into the diamonds of one parity.
  * A unit standing on such a cell, own or a shown enemy, is never marked. A moving unit blinks on and off.
  * The real release binary on the 256×256 probe map lit 566 minimap pixels for 391 visible cells; scale 1 would give about 782 if every visible cell were drawn.
  * D8 accepts maps up to 256×256, and D36 item 1 states the layout for "the largest map" (512 × 256). D36 item 2 states that units and buildings are drawn. Neither DECISIONS nor STATUS records this limitation; the test's `scale >= 4` condition hides it.
* **Expected behaviour:** every cell of every accepted map size is shown by at least one pixel, or markers are drawn at their projected pixel position, independent of the cell sampling. For example, use `k = max(2, …)`, giving at most 1,024 × 512, or draw markers by projection. `every_pixel_shows_the_cell_under_it`, or a new test, asserts full coverage for 1×1 to 256×256, including W + H = 129 and 256×256.
* **Severity rationale:** P2, because the only shipped map (64×64) is fully covered, and simulation state and invariants are unaffected. It must be resolved before any map with W + H > 128 ships (M6 map selection, M7 maps).

### P2-3: A placement or rally click made while paused, or refused by a full sink, leaves the mode although nothing was issued

* **File and line:** `crates/game/src/construction.rs:381-391`. `mode_click` returns `InterfaceMode::Normal` for a valid placement or rally click. Line 386 assigns `next = after` before line 391 calls `sink.issue(commands)`, whose result is ignored. `CommandSink::push` refuses every command while paused (`sim_host.rs:222-225`, D39).
* **Evidence:**
  * While paused, a left click on a green footprint in placement mode, or on a cell in rally mode, ends the mode. The command is refused and logged with the error sound, but the player must re-enter the mode. The building stays ready, so no state is lost.
  * D39 says only that commands are refused while paused; it does not say that modes end.
  * No test covers this interaction, because `mode_click` is tested without the sink.
* **Expected behaviour:** the mode changes only if the sink accepted the command (use `sink.issue`'s count), or modes are kept while paused. A test of the decision should accompany the fix.
* **Severity rationale:** P2 (user experience; no data loss).

### P2-4: Pre-existing M4 test race, found in this review: four `m4_combat` tests write and read the same temporary file concurrently

* **File and line:** `crates/sim/tests/m4_combat.rs:452-471`. `extreme_scenario()` writes `CARGO_TARGET_TMPDIR/m4_combat/extreme_combat.ron` and reads it back. It is called by four tests (lines 499, 519, 544, and 572), which the test harness runs in parallel. The race was introduced in `9608923` (M4) and is unchanged by M5.
* **Evidence:**
  * `cargo test --release -p sim --test m4_combat` failed in 3 of 16 runs, each time in one of `the_longest_range_with_the_slowest_projectile_flies_32_ticks` or `the_shortest_range_reaches_every_neighbouring_cell_and_an_adjacent_building`, with `panicked at crates/sim/tests/m4_combat.rs:477:29: data file …/target/tmp/m4_combat/extreme_combat.ron: file: line 1, column 1: expected a struct "(", found the end of the file`. Another test had truncated the file while this one read it.
  * Debug: 0 of 20 runs failed, and the section 9 command `cargo test --workspace` passed here.
  * All writers write identical text, so the race can only cause spurious failures, never a false pass.
* **Expected behaviour:** each call writes its own file, for example named per test (as `combat_variant(name, …)` does) or with a unique suffix, so that `cargo test` is deterministic in both profiles.
* **Severity rationale:** P2. It concerns test reliability; no assertion is weakened and production behaviour is unaffected. It is reported because it is not in the recorded debt and can make the validation command fail spuriously.

### P2-5: D34 overstates what a `&World` withholds: the simulation world's entity allocator is reachable through `SimHost::world()`

* **File and line:**
  * `crates/game/src/sim_host.rs:92-94` (`pub fn world(&self) -> &World`).
  * `docs/DECISIONS.md:542` (D34 item 2: "For a `&World`, Bevy gives only shared resources and read-only query items").
  * `bevy_ecs-0.19.1/src/world/mod.rs:222`: `pub fn entity_allocator(&self) -> &EntityAllocator`.
  * `src/entity/mod.rs`: `pub fn alloc(&self) -> Entity`, `alloc_many(&self, count)`, and `build_remote_allocator(&self)`. The allocator "panics" when no ids remain.
* **Evidence:** any game system with `Res<SimHost>` can allocate, and so leak, entity ids of the simulation world through `host.world().entity_allocator()`, without `unsafe`. This changes no simulation component, resource, or `state_hash`: `sim` orders everything by `SimId`, never by `Entity` (grep). It is not simulation state under PLAN.md section 5. No game code does it (grep: no `entity_allocator` or `entities()` in `crates/game/src`).
* **Expected behaviour:**
  * Either correct D34 to state this residual: the world's entity allocator is concurrently allocatable through `&World`; it is not simulation state, and only deliberate misuse could exhaust it.
  * Or expose a read-only facade instead of `&World`: query helpers and the accessors already present.
* **Severity rationale:** P2 (documentation accuracy; the plan's boundary over simulation state holds).

## Requires user verification

These are not findings and do not affect the decision (PLAN.md section 12 item 2); they block only `m5-approved`. Run `cargo run --release -p game` with sound on and check the 16 M5 items of docs/STATUS.md, "M5 implementation report", "Requires user verification":

1. Sidebar layout and legibility.
2. Production from the sidebar (badges, progress bars, `READY`, cancel and refund, `locked`).
3. Units tab.
4. Placement preview and ghost (green and red, crossed-out cell, placement thud, right click or `Escape`).
5. Sell mode and `Delete`/`Backspace`.
6. Rally mode and `R`.
7. Minimap: shroud, fog, markers, white frame, click and drag.
8. Camera beside the sidebar. Note P2-1: a right click over the sidebar can order an attack on an enemy drawn behind it.
9. Health bars.
10. Control groups (Ctrl or Cmd + digit, digit, Shift + digit).
11. Hotkeys.
12. Pause.
13. Sound effects follow what is seen.
14. Music loop and volume controls.
15. A full match without debug keys (acceptance).
16. The window title is consistent with the sidebar.

The nine M4 items, the ten M3 items, the seven M2 items, and the five M1 items listed in docs/STATUS.md remain pending.

## Recorded debt that must be carried in docs/STATUS.md

**New in this review (P2):**

* **P2-1:** a right click over the sidebar or minimap can issue `Attack` on a shown enemy drawn behind the sidebar. D35 item 7 and user verification item 8 claim otherwise.
* **P2-2:** the minimap never draws half of all cells (odd x + y) on maps with W + H > 128, so markers on them are missing. The test asserts full coverage only for scale ≥ 4. This must be resolved before such maps ship.
* **P2-3:** placement and rally modes end on a click whose command the sink refused (paused or full).
* **P2-4:** pre-existing M4 test race on `target/tmp/m4_combat/extreme_combat.ron`. It caused spurious failures in 3 of 16 release runs and 0 of 20 debug runs.
* **P2-5:** D34 overstates `&World`: the simulation world's entity allocator is reachable. It is not simulation state.

**From the M5 implementation report (known debt), confirmed by this review:**

1. Interface wiring in the window is tested through pure functions, the conflict test, and the smoke runs, not by automated input tests. P2-1 and P2-3 are instances of this gap.
2. Fixed sidebar width (240 px), laid out for 1280 × 720. Lower buttons are clipped in small windows. The smoke check counts only displayed buttons, so the Units tab is not checked.
3. There is no building selection. Rally mode sets every producer's rally point.
4. A full queue (16) is not shown as unavailable.
5. At most 8 effects at once, and one start per effect per 80 ms. Effects volume applies to effects started afterwards.
6. `QueryState`s are built every frame, and the minimap is redrawn every tick. To be measured against the M7 benchmark. The 256×256 probe ran at 0.28 GB peak.
7. No "centre on control group" on a double press.
8. The terminal event log writes both players' events (M4 remediation debt item 2). M2 review 3 P2-2 (a box selection released outside the window).

**Deviations recorded in the report, accepted as documented:**

* The interface terrain is the map file's, so ore depletion is never shown, also in visible cells.
* Volume settings last for the session only.
* Pause refuses commands.
* Audio is synthesised.

**Note for M7 packaging:** OFL 1.1 requires the licence text to accompany redistributed copies of the embedded FiraMono font. The release package must include it.

**Resolved by M5 and confirmed in this review:**

* M2 implementation debt item 2 (the two-tick event window): events are copied per tick into the game's per-frame buffer.
* M2 implementation debt item 5 and M3 implementation debt item 7, as far as the interface is concerned (D34).
* M3 implementation debt items 5 and 8.
* M4 implementation debt item 8 (health bars).
* The "no validity preview until M5" part of M4 remediation debt item 3.

**Carried unchanged from the M4 review decision:** every item of "Recorded debt to carry" in docs/STATUS.md, including:

* M4 review 2 P2-1 (projectile line of flight);
* M4 remediation debt items 1 to 4, except the resolved part of item 3;
* M4 implementation debt items 1 to 11, except the resolved item 8;
* M3 review 1 P2-1 to P2-4 and M3 implementation debt items 1 to 9, except the resolved items 5, 7, and 8;
* the M2, M1, and M0 items listed there.

The linker warning (M0 item 1) and the `Skipped event Destroyed` warning (M0 item 2) were seen again.

## Decision rationale

* There is no P0 or P1 finding.
* All six validation commands exited 0.
* Every acceptance criterion that can be verified automatically was verified; the human-play acceptance is listed under user verification.
* No test was removed, skipped, or weakened.
* No dependency was added, and no change falls outside the M5 scope apart from the extraction of `check_placement`, which the placement preview requires.
* All five findings are P2 and must be recorded in docs/STATUS.md.

Decision: GO WITH DOCUMENTED NON-BLOCKING DEBT
