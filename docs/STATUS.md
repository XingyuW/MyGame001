# Milestone status

| Milestone | Status | Branch | Approved tag |
|-----------|--------|--------|--------------|
| M0: Bootstrap and dependency verification | **Approved** by the user on 2026-10-04 after independent review (decision: GO WITH DOCUMENTED NON-BLOCKING DEBT; no P0 or P1) | `milestone/m0` | `m0-approved` |
| M1: Map and camera | **Reviewed**: review 3 GO WITH DOCUMENTED NON-BLOCKING DEBT (0 P0, 0 P1, 3 P2) after reviews 1 and 2 NO-GO; merged into `main`. Awaiting user verification for `m1-approved` | `milestone/m1` | `m1-reviewed` |
| M2: Units and movement | **Reviewed**: review 3 GO WITH DOCUMENTED NON-BLOCKING DEBT (0 P0, 0 P1, 2 P2) after reviews 1 and 2 NO-GO; merged into `main`. Awaiting user verification for `m2-approved` | `milestone/m2` | `m2-reviewed` |
| M3: Economy and construction | **Reviewed**: review 1 GO WITH DOCUMENTED NON-BLOCKING DEBT (0 P0, 0 P1, 4 P2); merged into `main`. Awaiting user verification for `m3-approved` | `milestone/m3` | `m3-reviewed` |
| M4: Combat and fog of war | **Reviewed**: review 2 GO WITH DOCUMENTED NON-BLOCKING DEBT (0 P0, 0 P1, 1 P2) after review 1 NO-GO; merged into `main`. Awaiting user verification for `m4-approved` | `milestone/m4` | `m4-reviewed` |
| M5: Interface and audio | **Reviewed**: review 1 GO WITH DOCUMENTED NON-BLOCKING DEBT (0 P0, 0 P1, 5 P2); merged into `main`. Awaiting user verification for `m5-approved` | `milestone/m5` | `m5-reviewed` |
| M6: Skirmish AI and game flow | **Review 1: NO-GO** (0 P0, 1 P1, 4 P2; `docs/reviews/M6-review-1.md`), in remediation round 1 | `milestone/m6` | |
| M7: Content, balance, performance, and packaging | Not started | | |

## M0 implementation report (2026-10-03)

### Validation commands (PLAN.md section 9)

Run from the repository root on macOS 27.0.1, Apple M5, Rust 1.98.1, in this order, after the last code change:

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 27 passed, 0 failed, 0 ignored: `game` unit 7, `game` `tests/m0_spike.rs` 6, `sim` unit 2, `sim` `tests/m0_bootstrap.rs` 12, doc-tests 0 |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | log: `smoke test: 600 frames, 198 simulation ticks, spike map created: true` (a display was available) |

### Acceptance criteria

| Criterion | Evidence |
|-----------|----------|
| Every assumption has a recorded outcome with evidence | `docs/DECISIONS.md`: A1 true (with API mappings), A2 true (with an I8 caveat for M1), A3 false as stated, recorded fallback applied, A4 resolved (pin 1.98.1, Bevy requires 1.95.0), A5 true. F1 to F3 re-checked. |
| All versions are pinned | Every direct dependency uses `=x.y.z` (`crates/sim/Cargo.toml`, `crates/game/Cargo.toml`); `Cargo.lock` committed; toolchain pinned in `rust-toolchain.toml`. |
| All validation commands pass | Table above. |
| Test: tick counter equals N after N runs | `tick_counter_equals_n_after_n_schedule_runs` (N = 1, 2, 20, 1000), `tick_counter_increments_by_exactly_one_per_run`, `tick_counter_starts_at_zero` |
| Test: same seed and empty command log give equal hashes | `same_seed_and_empty_command_log_give_equal_hashes_at_every_tick` (every tick of 500) |
| Test: different seeds that consume randomness give different hashes | `different_seeds_consuming_randomness_give_different_hashes` (every tick of 500), `hash_reflects_consumed_randomness` |
| Scope: workspace, `SimPlugin` with `SimSchedule`, tick counter, `CommandQueue`, `state_hash` | `crates/sim/src/{schedule,command,hash,id,rng}.rs` |
| Scope: `game` binary opens a window; `--smoke-test <frames>` | `crates/game/src/{main,cli,smoke}.rs`; smoke run above; CLI errors exit with code 2 (unit tests in `cli.rs`; also run by hand: `--smoke-test 0`, `--smoke-test`, `--bogus`) |
| Scope: isometric Tiled map spike | `crates/game/src/spike.rs`, `assets/maps/spike/`; tests in `crates/game/tests/m0_spike.rs`; on-screen result **requires user verification** |

### Deviations from PLAN.md

1. `SimSchedule` has a tenth stage, `SimSet::EndOfTick`, after the plan's nine, to increment the tick counter (DECISIONS D3).
2. `Command` has no variants in M0 (DECISIONS D5), so `CommandQueue::push` cannot be exercised by a test until M2.
3. Pinned Bevy 0.19.1 (the plan names 0.19); F1's date differs from crates.io by three days (DECISIONS F1).
4. Files not listed in PLAN.md section 4: `.cargo/config.toml` (asset root, DECISIONS D1), `crates/game/examples/gen_spike_tileset.rs` (tileset generator), `crates/game/tests/m0_spike.rs` (spike and projection tests).
5. The `cargo init` stub (`src/main.rs` printing "Hello, world!", and a root `Cargo.toml` package named `MyGame001`, both never committed) was replaced by the workspace layout of section 4.

### Known debt (P2, accepted with the M0 approval)

Items 1 to 7 come from the implementation report and were confirmed by the independent review. Items 8 to 11 were raised by the review ("Milestone 0 code review" session, 2026-10-04).

1. On macOS, dev-profile links of `game` print a linker warning (`__eh_frame section too large ... compact unwind table`). It is a warning only; clippy and release builds are unaffected.
2. Shutdown sometimes logs `WARN bevy_winit::state: Skipped event Destroyed for unknown winit Window Id` (seen in some smoke runs, including the reviewer's, not in others). Exit code is unaffected.
3. `game` uses a constant simulation seed (`MATCH_SEED = 0`) until match setup in M6.
4. `crates/game/src/spike.rs` is spike code to be replaced by the M1 map loading and rendering.
5. The smoke test fails if the spike map has not been created by the last frame. For small frame counts the outcome therefore depends on asset loading speed: the reviewer observed `--smoke-test 1` exit 0 with the map created. (Corrected per review item 10; the earlier wording "fail by design" was wrong.)
6. `bevy_ecs_tiled` panics on staggered and oblique maps (`helpers.rs`); the M1 loader must validate maps itself before handing them over (DECISIONS A2 caveat, invariant I8).
7. Asset location: `assets/` is found only when the binary is started through cargo (`.cargo/config.toml` sets `BEVY_ASSET_ROOT`). Run directly, `target/release/game` looks in `target/release/assets/` and the smoke test exits 1, naming the missing path. Packaged builds must ship `assets/` next to the executable (DECISIONS D1, M7).
8. `SimIdAllocator::allocate` (`crates/sim/src/id.rs`) is documented to return `None` when the id space is exhausted, but no test covers that boundary; `next` is private and there is no constructor to start near `u32::MAX`. Expected: a boundary test using a test-only constructor.
9. The smoke test's failure path (exit 1 on an asset load failure or a missing map, `crates/game/src/smoke.rs`) is not tested automatically; the reviewer verified it only by hand with a missing asset root. Expected: an automated end-to-end check.
10. Correction of item 5 (done above).
11. Platform statement conflict: PLAN.md section 1 names Windows, macOS, and Linux for version 1.0, while the user decided that macOS is sufficient (DECISIONS U1). The user must decide before M7 whether to amend PLAN.md.

### User verification

| Item | Outcome |
|------|---------|
| 1. `cargo run --release -p game` shows the spike map: 16×16 green diamond map, **red** tile at the **top** corner (cell 0, 0), **blue** tile at the **right** corner (cell 15, 0), 3×3 **gold** ore patch left of and slightly below the centre, tiles edge to edge | **Verified by the user, 2026-10-03.** |
| 2. Platforms | The user decided that running on macOS is sufficient (DECISIONS U1). Verified on macOS only; Windows and Linux were not tested. |
| 3. Toolchain pin 1.98.1 (Bevy minimum is 1.95.0) | Not explicitly confirmed; the user asked to proceed without objecting. The reviewer may raise it. |

### Approval

The independent review reran all validation commands with exit code 0 and found no P0 or P1. Its condition, that every P2 item be recorded here, is met by the debt list above. The user approved M0 on 2026-10-04, and the tag `m0-approved` marks the approved state. M1 is reviewed against this tag.

## M1 implementation report (2026-10-04)

Branch `milestone/m1`, based on the tag `m0-approved`. Commits: `41c4d51` (sim: `Grid` and map loader), `b2862c9` (game: map rendering, camera, picking, demo map), and the documentation commit that adds this report.

### Files changed

| File | Change |
|------|--------|
| `crates/sim/Cargo.toml`, `Cargo.lock` | `sim` depends on `tiled` =0.16.0 (no default features) and `quick-xml` =0.31.0; no package added to the lock file (DECISIONS, A2 follow-up) |
| `crates/sim/src/map.rs` (new) | `CellPos`, `Cell`, `Grid` resource, `LoadedMap`, `MapError`/`MapErrorKind`, `load_map` with XML precheck and validation (DECISIONS D8) |
| `crates/sim/src/{lib,schedule,hash}.rs` | Exports; `SimPlugin` inserts an empty `Grid`; `state_hash` covers the grid (D11) |
| `crates/sim/tests/m1_map.rs`, `crates/sim/tests/fixtures/maps/*.tmx` (new) | 32 tests; fixtures `valid_4x3`, `missing_layer`, `unknown_property`, `zero_size` |
| `crates/game/src/projection.rs` (new) | Equation 7, inverse, picking, map bounds, map entity offset (D9), 6 unit tests |
| `crates/game/src/camera.rs` (new) | Camera spawn, keyboard, edge, and drag panning, zoom, clamping (D10), 7 unit tests |
| `crates/game/src/map.rs` (new) | Validated map loading, grid installation, tilemap spawn, alignment check, hovered-cell outline, click log, 2 unit tests |
| `crates/game/src/spike.rs` (deleted) | Replaced by `map.rs` (M0 debt item 4). The spike assets and `tests/m0_spike.rs` are unchanged. |
| `crates/game/src/{main,smoke}.rs` | Plugins wired; the smoke test requires the validated map to be created |
| `crates/game/examples/gen_demo_map.rs`, `assets/maps/demo/{demo.tmx,demo_tiles.png}` (new) | Generated 64×64 demo map and tileset |
| `crates/game/tests/m1_demo_map.rs` (new) | 3 tests: generator output, `sim` validation of the demo map, terrain contents |
| `assets/LICENSES.md`, `docs/DECISIONS.md`, `docs/STATUS.md` | Provenance of the demo map; M1 decisions; this report |

### Validation commands (PLAN.md section 9)

Run from the repository root on macOS (Apple M5, Rust 1.98.1) after the last code change, in this order:

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 77 passed, 0 failed, 0 ignored: `game` unit 22 (cli 7, projection 6, camera 7, map 2), `game` `tests/m0_spike.rs` 6, `game` `tests/m1_demo_map.rs` 3, `sim` unit 2, `sim` `tests/m0_bootstrap.rs` 12, `sim` `tests/m1_map.rs` 32, doc-tests 0 |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | log: `map maps/demo/demo.tmx: 64x64 cells, tile 64x32 px` and `smoke test: 600 frames, 198 simulation ticks, map created: true` (a display was available) |

Additional manual check: with `BEVY_ASSET_ROOT` pointing at a copy of `assets/` whose demo map was made staggered, or had one CSV entry removed, the smoke run exited with code 1. It logged `... demo.tmx: map attribute "orientation": orientation "staggered" is not supported, expected "isometric"` and `... demo.tmx: layer "terrain" data: layer data has 4095 entries, expected 4096` respectively, with no panic. Without the loader, these inputs reach a `panic!` in `bevy_ecs_tiled` (source review) and an out-of-bounds index in `tiled` (reproduced by a probe), respectively (DECISIONS, A2 follow-up).

### Acceptance criteria and required tests (PLAN.md section 7, M1)

| Criterion | Evidence |
|-----------|----------|
| Scope: map loading with terrain flags and ore; `Grid` resource | `sim::load_map`, `sim::Grid`; `valid_fixture_loads_every_cell_in_tiled_coordinates`, `single_cell_and_thin_maps_load`, `maximum_map_size_loads`, `external_tileset_loads`, `ore_amount_limits`, `demo_map_loads_as_a_64_by_64_grid_matching_the_generator` |
| Scope: map rendering | `game::map` spawns the validated map through `bevy_ecs_tiled` at `map_origin_offset`; alignment with the projection: `map_origin_offset_aligns_bevy_ecs_tilemap_with_the_projection`, `demo_map_is_aligned`, `offsets_and_parallax_are_reported`; smoke run above. On-screen result **requires user verification**. |
| Scope: camera panning by keyboard, screen edge, and middle mouse drag | `keyboard_directions`, `edge_directions`, `drag_keeps_the_grabbed_world_point_under_the_cursor` (pure functions used by the systems); behaviour in the window **requires user verification** |
| Scope: zoom with limits | `zoom_stays_within_its_limits`; behaviour in the window **requires user verification** |
| Scope: camera bounds | `clamping_at_all_four_borders`, `centre_inside_the_bounds_is_unchanged`, `view_larger_than_the_bounds_is_centred_on_that_axis` |
| Scope: cursor to cell picking | `game::map::update_hovered_cell` (outline and centre dot drawn, cell logged on left click); on-screen result **requires user verification** |
| Test: cell to world to cell round trip for every cell of 1×1, 1×N, N×1, and 64×64 maps | `cell_world_cell_round_trip_for_every_cell` (1×1, 1×37, 37×1, 64×64) |
| Test: picking at tile centres, edges, and corners | `picking_at_centres_edges_and_corners` (16 points per cell, every cell of those maps), `shared_edges_and_corners_belong_to_the_larger_coordinate` |
| Test: picking outside the map returns none | `picking_outside_the_map_returns_none` (just beyond all four edges along their length, far points, NaN, infinities) |
| Test: malformed maps (missing layer, unknown property, zero size) return descriptive errors | `missing_layer_is_rejected`, `unknown_tile_property_is_rejected`, `zero_size_is_rejected` (committed fixtures). Each asserts the error kind and field, and that the message contains the file, the field, and the reason. 18 further tests with negative cases cover orientation, infinite maps, size limits, tile size, schema version, unknown map, layer, and tileset properties, missing and mistyped tile properties, negative ore, image-collection tilesets, extra, renamed, and object layers, layer size, CSV entry count, encoding, empty and undefined cells, unreadable files, malformed XML, and every truncation of a valid file. |
| Test: camera clamping at all four borders | `clamping_at_all_four_borders` |
| Acceptance: a 64×64 demo map renders correctly | Demo map generated and validated (tests above); smoke run creates it. Visual correctness **requires user verification**. |
| Acceptance: picking has zero error in tests | All picking tests assert exact equality on exactly representable coordinates (DECISIONS D9). |
| Acceptance: the user confirms the visual result | **Pending** (see below). |
| Section 8: integration through `SimPlugin` | `installed_grid_is_unchanged_by_ticks`, `same_map_and_seed_give_equal_hashes_at_every_tick`, `hash_covers_every_cell_attribute`, `sim_plugin_starts_with_an_empty_grid`. M1 adds no `Command` variant, so `CommandQueue` is not exercised (see deviations). |
| Section 8: end to end | Smoke run of the real binary (above). |

### Assumptions verified and how

| Item | Verification |
|------|--------------|
| `tiled` carries no rendering dependency (DECISIONS A2 caveat) | `cargo tree -p sim -e normal`; no forbidden crate |
| `tiled` and `bevy_ecs_tiled` panic sites | Source review plus a probe reproducing the `tiled` panic (DECISIONS, A2 follow-up) |
| Bevy APIs used: `Projection::Orthographic`/`OrthographicProjection::scale`, `Camera::viewport_to_world_2d`, `Camera::logical_viewport_size`, `AccumulatedMouseScroll`/`MouseScrollUnit`, `ButtonInput`, `Window::cursor_position`/`size`/`focused`, `Single`, `Gizmos::linestrip_2d`/`circle_2d`, `TransformSystems::Propagate`, `CameraUpdateSystems`, `FileAssetReader::root_path` | Read in the 0.19.1 sources (paths in DECISIONS D9 to D12) and compiled |
| `bevy_ecs_tiled` places layers as children of the map entity, adds only layer and tileset offsets, and uses the map's tile size for the grid | `map/spawn.rs:142-161, 313-335`, `helpers.rs:116-121`; checked by `map_origin_offset_aligns_bevy_ecs_tilemap_with_the_projection` |
| `quick-xml` 0.31.0 API (`Reader::from_reader`, `read_event_into`, `try_get_attribute`, `unescape`, `read_to_end_into`) | Sources and compilation |

### Deviations from PLAN.md

1. `Grid` has no cell occupant yet (PLAN.md section 5 lists it); it is added in M2 with the units that occupy cells (DECISIONS D11).
2. M1 introduces no simulation command, so `Command` remains without variants and the M1 tests cannot drive `CommandQueue`. The grid is installed as match setup (DECISIONS D11).
3. The map format (DECISIONS D8) is stricter than the plan states: CSV layer data only, single-image tilesets only, one layer, at most 256×256 cells.
4. Keyboard panning uses the arrow keys only (DECISIONS D10).
5. Projection, camera, and map tests are unit tests inside the `game` binary crate (the projection is game-only per PLAN.md section 5, and a binary crate's modules cannot be imported by integration tests). The demo map tests are integration tests in `crates/game/tests/`.
6. Files not listed in PLAN.md section 4: `crates/game/examples/gen_demo_map.rs` (demo map generator).

### Known debt (P2 proposals for the reviewer)

1. The game reads the map file twice at startup: once through `sim::load_map` (validation and grid) and once through Bevy's asset server (rendering). If the file changed between the two reads, the rendered map could differ from the simulated one. The window is milliseconds at startup, and packaged assets are read-only.
2. The camera and picking systems are covered through their pure functions and the tilemap reconciliation test, not by an automated test of the running window; their wiring is part of the user verification below.
3. The XML parser accepts an unterminated final `</map` at the end of a file (documented in `truncated_map_files_never_panic`); the map content is complete in that case.

M0 debt status: item 4 (spike code) and item 6 (`bevy_ecs_tiled` panics on staggered and oblique maps; the loader now rejects them) are resolved by M1. Item 5 now applies to the demo map: the smoke test fails if the map has not been created by the last frame. Items 1, 2, 3, 7, 8, 9, and 11 are unchanged. Regarding item 11: the working tree contains an uncommitted edit of PLAN.md section 1 (Windows, macOS, and Linux changed to macOS) that this session did not make, commit, or record. DECISIONS.md does not yet record the user's approval of that change.

### Requires user verification

Run `cargo run --release -p game` and check:

1. **Map rendering:** a 64×64 green diamond map, tiles edge to edge with no gaps or overlaps. A **red** tile at the **top** corner (cell 0, 0), a **magenta** tile at the **right** corner (cell 63, 0), and a **white** tile at the **left** corner (cell 0, 63). A grey rock ridge runs down the vertical centre line with a gap near the middle. A blue lake with a sand shore sits right of centre. Gold ore fields appear on the centre line in the upper and lower parts and to the left of centre.
2. **Picking:** a white diamond outline with a dot at its centre sits exactly on the tile under the cursor, including along all four map edges, and disappears when the cursor leaves the map. A left click logs the cell, e.g. `cell (0, 0): passable true, buildable true, ore 0` on the red tile, `passable false` on water or rock, `ore 500` on ore, and `click outside the map` outside it.
3. **Panning:** the arrow keys pan, and diagonals are not faster. Resting the cursor within 8 px of a window edge pans that way, only while the window has focus. Dragging with the middle mouse button moves the map with the cursor.
4. **Zoom:** the mouse wheel or trackpad zooms in and out, stopping at 2× magnification and at 0.5× (scale 2).
5. **Bounds:** the view cannot leave the map's bounding rectangle at any zoom or window size. When the view is taller than the map (zoomed out), the map stays vertically centred.

## M1 remediation round 1 report (2026-10-04)

Implementer session for remediation round 1 of M1 review 1 (`docs/reviews/M1-review-1.md`, NO-GO: 4 P0, 3 P1, 6 P2), on branch `milestone/m1`, base `d1304e6`. Every finding and every user note was treated as a claim and reconciled against the code; pre-fix behaviour was reproduced on a scratch copy of `d1304e6` (`git archive`, outside the repository) by running the new tests against the unmodified loader, with only their final assertion relaxed to "the map is rejected". The M1 row above stays "in remediation" until review 2.

### Class of defect

All four P0 findings, P1-1, and P1-3 are one class: **unrestricted hand-off of external files to third party parsers and renderers (invariant I11)**. The loader rejected a list of known-bad features (orientation, infinite maps, short CSV data, image-collection tilesets, unknown properties) and passed everything else to `tiled`, `bevy_ecs_tiled`, and `bevy_asset` unchecked, so any feature not on the list reached code that panics, recurses, or allocates on it. The class is fixed by replacing the deny-list with an allow-list (DECISIONS D13), checked on the raw bytes of the map and of its external tileset before any third party code sees them, with `tiled` parsing only the checked bytes, `bevy_ecs_tiled`'s own external-file scan reproduced and matched, the tileset image header checked by the game before the hand-off, and every asset load failure made fatal (D14). The previous claims "never panics on malformed input" (`crates/sim/src/map.rs`) and "`bevy_ecs_tiled` only ever sees maps that cannot make it panic" (`crates/game/src/map.rs`) were replaced by statements limited to what the checks and tests cover, and the incomplete list of reviewed panic sites in DECISIONS (A2 follow-up, item 3) carries a correction.

### Findings

| Finding | Outcome | Evidence | Change | Regression test (fails before, passes after) |
|---------|---------|----------|--------|-----------------------------------------------|
| P0-1 tileset `margin`/`spacing` panic in `tiled` | **Confirmed** | Pre-fix, debug: `panicked at tiled-0.16.0/src/tileset.rs:577:27` for `margin="1000"` without `columns`; release: the same map was accepted. | D13: `margin` and `spacing` are unsupported attributes, `columns` is required and ≥ 1, image width = `columns × 64`, image height a multiple of 32, `tilecount = columns × rows`. Same rules for external tilesets. | `m1_map_subset::tileset_margin_spacing_and_missing_columns_are_rejected_before_tiled_parses`, `tileset_image_must_hold_exactly_tilecount_tiles` |
| P0-2 deep nesting overflows the stack | **Confirmed** | Pre-fix, debug and release: 10,000 nested `<group>` abort the test process with "has overflowed its stack" (exit 134). | D13: `group`, `objectgroup`, `imagelayer` are rejected at their start tag, `class` and `list` property types are rejected, and the grammar is at most five elements deep, so the checker itself recurses at most five levels. File size ≤ 4 MiB. | `deeply_nested_groups_and_class_properties_are_rejected_without_recursion` (also 90,000 nested class properties), `files_above_the_size_limit_are_rejected` |
| P0-3 unbounded allocation from `tilecount` | **Confirmed** | Pre-fix, debug and release: under the test binary's 256 MiB budget, `tilecount="4294967295"` aborts with "memory allocation of 1726576853000 bytes failed" (exit 134). | D13: `tilecount` from 1 to 1024 and equal to `columns × rows` of the declared image, checked before `tiled` runs. | `tilecount_is_bounded_before_tiled_allocates` (largest value first, so a regression hits the memory budget at once) |
| P0-4 renderer loaders panic on maps accepted by `sim` | **Confirmed** | Source of `bevy_ecs_tiled` `loader.rs:421-422` (column formula evaluated even with `columns`) and the reviewer's runs; pre-fix, `sim` accepted `source="../../../demo_tiles.png"`. After the fix the game binary exits 1 with a typed error for the reviewer's `margin_columns` and `image_parent` maps in debug and release (smoke run of a copied asset root). | D13: no `margin`/`spacing`; image and tileset `source` must be a plain file name in the map's directory (no `/`, `\`, `..`, `#`, `:`), `.png` and `.tsx` respectively. | `image_and_tileset_paths_outside_the_map_directory_are_rejected`, the `margin_with_columns` case of the P0-1 test |
| P1-1 inputs outside a subset reach the renderer silently | **Confirmed** | Pre-fix: a complete definition of tile 100000 used by a cell, a single-quoted external tileset reference, a 128×64 tile size, and layer `offsetx` were all accepted by `sim`; a 1×1 PNG in place of the declared 512×32 image was rendered without error (review). | D13: tile ids unique and below `tilecount`; tile size exactly 64×32 for the map and its tileset (the only size with established exact picking); `bevy_ecs_tiled`'s line-based reference scan reproduced and required to find exactly the accepted references; offsets, parallax, opacity, tile offsets and other rendering attributes rejected in `sim` before the hand-off. Game: `check_tileset_image` compares the PNG header with the declared size before spawning the map. | `tile_ids_outside_the_tileset_and_duplicate_tiles_are_rejected`, `tile_sizes_without_established_exact_picking_are_rejected`, `external_references_the_renderer_would_resolve_differently_are_rejected`, `offsets_parallax_and_other_rendering_attributes_are_rejected`; game: `map::tests::tileset_images_other_than_the_declared_png_are_rejected`, `projection::tests::the_tested_tile_size_is_the_only_one_maps_may_use` |
| P1-2 renderer-side load failure leaves a normal run without a map | **Confirmed** | `crates/game/src/smoke.rs` held the only handler of `UntypedAssetLoadFailedEvent`; the reviewer's normal run stayed alive until killed (exit 142). | D14: `exit_on_asset_load_failure` in `MapPlugin` logs the path and reason and exits with code 1 in every mode. After the fix a normal (non-smoke) run with a corrupt `demo_tiles.png` exits 1 within 50 ms of the failure, debug and release. | `map::tests::an_asset_load_failure_ends_the_game_with_an_error`. Before the fix the handler did not exist, so this test cannot be built against the reviewed code; the pre-fix behaviour is the reviewer's run above. |
| P1-3 external tileset errors name the map | **Confirmed** | `crates/sim/src/map.rs` (reviewed version) built every `MapError` with the map path, and `tiled`'s parse errors of a `.tsx` were reported as field `file` of the map (review). | Checker errors inside a `.tsx` carry the `.tsx` path; validation errors in a tileset carry `tiled`'s `Tileset::source` (the `.tsx` for external tilesets). A missing `.tsx` is reported in the map, naming the `.tsx` path in the message. | `errors_in_external_tilesets_name_the_tileset_file` (unknown property, unsupported attribute, truncated XML, and a nested reference inside the `.tsx`; each asserts `error.file` is the `.tsx`) |
| P2-1 map file read twice | **Recorded as debt** (below) | Unchanged: `sim` reads and checks the bytes, the asset server reads the files again. | None. | None |
| P2-2 unterminated final `</map` accepted | **Fixed** | Pre-fix, accepted (A2 follow-up item 4). | The checker requires the root end tag to end in `>` and allows only comments and whitespace after it. | `xml_constructs_outside_the_subset_are_parse_errors` (case `unterminated_end`); `m1_map::truncated_map_files_never_panic` still passes unchanged (it permits either outcome for that prefix). |
| P2-3 camera and picking systems tested through pure functions only | **Recorded as debt** (below) | Unchanged. | None. | None |
| P2-4 fixed temporary directory in a game unit test | **Fixed** | `offsets_and_parallax_are_reported` wrote to `temp_dir()/mygame001_game_map_tests`, shared by concurrent runs. | Game map tests use a directory unique to the test process and test, removed afterwards (`scratch_dir`). The test's assertions are unchanged. | None: the defect is in test infrastructure, not in behaviour that a test could observe. |
| P2-5 `Grid` inserted directly as match setup | **Recorded as debt** (below) | Unchanged. | None. | None |
| P2-6 D8 does not state that every tile id below `tilecount` must be defined | **Fixed** (documentation) | Behaviour confirmed: `tiled` creates property-less entries below `tilecount` and the loader rejects them. | DECISIONS D8 item 4. | `every_tile_id_below_tilecount_needs_a_definition` documents the behaviour (passes before and after; not a defect). |

Further members of the class found while defining the subset, fixed by the same change and covered by `m1_map_subset`: duplicate attributes and namespaced attribute or element names (`tiled` matches by local name), duplicate property names and duplicate tile definitions (`tiled` keeps the last silently), tile animations, collision shapes and per-tile images, `class`/`list`/custom property types, properties whose value is element text or does not parse as its type, `wangsets`/`transformations`, a second tileset, a `firstgid` other than 1, a non-empty external reference element (`tiled` would read its content as map content), DTDs, processing instructions, CDATA, text content, content after the root, non-UTF-8 files, and comments containing `source="….tsx"` that the renderer's scan would follow.

Seeded mutation test (user note 3): `seeded_mutations_of_the_demo_map_return_a_map_or_a_typed_error_and_never_panic` applies 1 to 3 of 11 mutation operators (number and attribute-value replacement, attribute insertion, replacement, and removal with tags drawn by element kind, snippet insertion including 2,000-fold repetition, span deletion and duplication, byte replacement, truncation, single quotes) to the demo map for 6,000 cases from a fixed SplitMix64 seed. It asserts that every case returns either a map within the format's limits or a `MapError` naming the file and a field, and never panics, under the memory and time bounds of D15. Against the reviewed code it aborts on the memory budget in debug and in release; after the fix it passes in about 6 s (debug).

### User notes (reconciled by content; their IDs referred to an earlier report)

| Note | Outcome |
|------|---------|
| 1. Fix P0-1 and P1-1, each with a regression test that fails before the fix | Done for the current report's P0-1 to P0-4 and P1-1, which carry the content of the note (hostile tileset geometry; renderer accepting unsupported input). Tests in the table above. |
| 2. One class under I11: explicit subset, reject the rest, including duplicate or namespaced attributes, tile animations, tile ids outside the tileset, and tile sizes without established exact picking | Done (D13). "Tile identifiers outside the tileset" corresponds to P1-1 item 1 of the current report and "tile sizes for which exact picking is not established" to P1-1 item 4; both are rejected and tested. |
| 3. Deterministic, seeded mutation test over the demo map | Done (above), bounded in memory and time (D15). |
| 4. Correct the claim in `crates/game/src/map.rs` and the list of reviewed panic sites in DECISIONS | Done: module documentation of `crates/game/src/map.rs` and of `sim::load_map` rewritten; DECISIONS A2 follow-up item 3 corrected with the missed sites. |
| 5. P1-2 deferred to the user under section 12 item 2; record P2-3 and the carried M0 debt | **Refuted for P1-2 of the current report**: it is a silent fallback (exit status of a normal run), which is automatable and is not a visual or audio item that only the user can verify, so section 12 item 2 does not apply and the finding was fixed (D14). P2-3 and the carried M0 debt are recorded below. |

### Files changed

| File | Change |
|------|--------|
| `crates/sim/src/map_subset.rs` (new) | Allow-list checker for map and external tileset files (D13), bounded file reading, reproduction of `bevy_ecs_tiled`'s external-file scan |
| `crates/sim/src/map.rs` | `load_map` uses the checker and serves `tiled` only the checked bytes; old precheck removed; validation errors in tilesets name the tileset file; new constants `TILE_WIDTH_PX`, `TILE_HEIGHT_PX`, `MAX_TILESET_TILES`, `MAX_MAP_FILE_BYTES`; new `MapErrorKind` variants `FileTooLarge`, `UnsupportedElement`, `UnsupportedAttribute`, `MissingAttribute`, `Duplicate`, `InvalidValue`, `ExternalReferenceMismatch`; `LoadedMap::tileset_image` (`TilesetImage`) |
| `crates/sim/src/lib.rs` | Module and exports |
| `crates/sim/tests/m1_map_subset.rs` (new) | 19 tests, bounded by a budget allocator and time limits (D15) |
| `crates/game/src/map.rs` | Tileset image header check before the hand-off; `exit_on_asset_load_failure`; corrected module documentation; per-process test directories; 3 new tests |
| `crates/game/src/projection.rs` | Test tying the tested tile size to the only accepted one |
| `docs/DECISIONS.md` | Correction of A2 follow-up item 3; D8 amended (P2-6, reference to D13); D9 updated; new D13, D14, D15 |
| `docs/STATUS.md` | This report |

### Validation commands (PLAN.md section 9)

Run from the repository root on macOS (Apple M5, Rust 1.98.1), in this order, in the foreground, after the last code change. A display was available.

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 100 passed, 0 failed, 0 ignored: `game` unit 26 (cli 7, projection 7, camera 7, map 5), `game` `tests/m0_spike.rs` 6, `game` `tests/m1_demo_map.rs` 3, `sim` unit 2, `sim` `tests/m0_bootstrap.rs` 12, `sim` `tests/m1_map.rs` 32, `sim` `tests/m1_map_subset.rs` 19, doc-tests 0. Known M0 linker warning only. |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | `map maps/demo/demo.tmx: 64x64 cells, tile 64x32 px`; `smoke test: 600 frames, 198 simulation ticks, map created: true` |
| `cargo run -p game -- --smoke-test 600` | 0 | `smoke test: 600 frames, 203 simulation ticks, map created: true` |

Additionally, `cargo test --release -p sim --test m1_map --test m1_map_subset` passed (32 and 18 tests at the time of the run, before the P2-6 documentation test was added), and the debug and release game binaries were run against copied asset roots: the unmodified demo map and an external-tileset variant exit 0; the reviewer's `margin_columns`, `image_parent`, tile id 100000, 1×1 image, 10,000 nested groups, `tilecount="4294967295"`, and single-quoted reference variants exit 1 with a typed error; a corrupt `demo_tiles.png` exits 1 in a normal (non-smoke) run.

### Acceptance criteria (PLAN.md section 7, M1)

| Criterion | Evidence |
|-----------|----------|
| Map loading with terrain flags and ore; `Grid` resource | Unchanged positive tests in `m1_map.rs` and `m1_demo_map.rs` pass; `m1_map_subset::files_inside_the_subset_load` (demo map, fixture, external tileset) |
| Map rendering | Smoke runs above (map created in both profiles); the game renders only maps that passed `sim::load_map` and the image check. On-screen result **requires user verification**. |
| Camera panning, zoom with limits, camera bounds | Unchanged pure-function tests pass; behaviour in the window **requires user verification** |
| Cursor to cell picking; zero error in tests | Unchanged picking tests pass; maps are now restricted to the tile size for which they establish exactness (D13) |
| Required M1 tests (round trip, picking, outside the map, malformed maps, camera clamping) | All unchanged and passing (`projection`, `camera`, `m1_map`) |
| I8 (no panic from malformed data) and I11 (subset before hand-off) | D13, D14; `m1_map_subset` (19 tests including the mutation test); game tests; binary runs above |
| A 64×64 demo map renders correctly; the user confirms the visual result | **Requires user verification** (list below) |

### Assumptions verified and how

| Item | Verification |
|------|--------------|
| `tiled` 0.16.0 matches attributes and elements by local name, parses tileset geometry without overflow checks, sizes a map by `tilecount`, recurses on groups and class properties, re-reads a non-empty reference element's content as map content | Source: `util.rs` (`get_attrs!`, `parse_tag!`), `tileset.rs` (`parse_xml_in_map`, `finish_parsing_xml`, `calculate_columns`), `properties.rs`, `layers/mod.rs`, `layers/tile/{mod,util,finite}.rs`, `image.rs`, `tile.rs`; reproduced by the pre-fix test runs |
| `quick-xml` 0.31.0 default reader settings and the slice reader's `read_event` | Source: `reader/state.rs` (`Default for ReaderState`), `reader/slice_reader.rs`; `Attributes::with_checks` |
| `bevy_ecs_tiled` 0.13.4 external-file preloading, image and atlas computation | Source: `tiled/reader.rs` (`extract_external_paths`, `normalize_path`), `tiled/map/loader.rs` (`tileset_to_tiled_map_tileset`) |
| `App::should_exit` prefers an error exit; `UntypedAssetLoadFailedEvent` fields are public | Source: `bevy_app-0.19.1/src/app.rs:1421`, `bevy_asset-0.19.1/src/event.rs:28` |

### Deviations from PLAN.md

1. The map format is narrower than before (D13): exactly one tileset, tile size 64×32 only, no `margin`/`spacing`, at most 1,024 tiles per tileset, files at most 4 MiB, and the listed elements and attributes only. The demo map and all fixtures are inside the subset and unchanged.
2. `sim::LoadedMap` gained the field `tileset_image` and `MapErrorKind` gained seven variants (public API change of `sim`; no data schema change, so `schema_version` stays 1).
3. File not listed in PLAN.md section 4: `crates/sim/src/map_subset.rs`. The test binary `m1_map_subset` contains `unsafe` code (a counting `GlobalAlloc` wrapper around `System`) to bound memory (D15).

### Known debt (P2)

From M1 review 1, recorded rather than fixed:

1. **P2-1:** the game reads the map file twice (`sim::load_map`, then the asset server), and the asset server also re-reads the external tileset and the tileset image after `sim` and `check_tileset_image` have checked them, so the rendered bytes are not guaranteed to be the checked bytes if a file changes in between. Packaged assets are read-only; serving the checked bytes to the asset server (for example through an in-memory asset source) is the remedy.
2. **P2-3:** the camera and picking systems are covered through their pure functions; their wiring in the window is part of the user verification below.
3. **P2-5:** the game inserts `Grid` directly as match setup (DECISIONS D11); the user's acknowledgement of this reading of I5 is to be recorded when M6 adds match setup.

New in this round:

4. With `--smoke-test`, an asset load failure is logged twice (by the smoke test's handler, D6, and by `exit_on_asset_load_failure`, D14). Exit code unaffected.
5. A UTF-8 byte order mark before the XML declaration is rejected (the checker treats it as text before the root). The repository's generators write none.
6. `check_tileset_image` reads only the PNG signature and IHDR size; a file that is corrupt beyond the header is caught when Bevy fails to decode it (D14), after the map entity has been spawned.

Carried from M0 (unchanged unless noted): item 1 (dev-profile linker warning `__eh_frame section too large`), item 2 (occasional `Skipped event Destroyed` warning at shutdown, seen again in this round's smoke runs), item 3 (constant `MATCH_SEED` until M6), item 5 (the smoke test fails if the map is not created within the frame count; now applies to the demo map), item 7 (assets found only when started through cargo or with `assets/` next to the executable), item 8 (no boundary test for `SimIdAllocator` exhaustion), item 9 (the smoke test's failure path is not tested automatically; the new handler of D14 is unit tested, the smoke test's own handler is not). Items 4 and 6 were resolved in M1, and item 11 by the PLAN.md amendment U2.

### Requires user verification

Unchanged from the M1 implementation report (section 12 item 2; not review findings). Run `cargo run --release -p game` and check map rendering (64×64 green diamond, red top corner (0, 0), magenta right corner (63, 0), white left corner (0, 63), rock ridge with a gap, lake with sand shore, ore fields), picking (outline and dot on the tile under the cursor, click log), panning (arrow keys, screen edge while focused, middle mouse drag), zoom (2× to 0.5×), and bounds (the view stays within the map's bounding rectangle; vertically centred when taller than the map). This round changed no rendering, camera, or picking code; it only rejects more maps before rendering.

## M1 remediation round 2 report (2026-10-04)

Implementer session for remediation round 2 of M1 review 2 (`docs/reviews/M1-review-2.md`, NO-GO: 1 P0, 1 P1, 4 P2), on branch `milestone/m1`, base `efb25df`. Every finding was treated as a claim and reconciled against the code. The reviewed behaviour was reproduced with the reviewed binaries (built from `efb25df`, whose code equals `deb2859`) on scratch asset roots selected with `BEVY_ASSET_ROOT`, under a `perl alarm` timeout, with `/usr/bin/time -l` for memory. "Fails before" evidence comes from a scratch copy of `efb25df` (`git archive`, outside the repository) into which the new tests were copied; where the tests name new API, only that API was added as an inert shim (the new constant and two error variants in `sim`; for the game, a module with the new types whose check is the reviewed 24-byte header check verbatim). The M1 row above is maintained by the orchestrator.

### Class of defect

P0-A and P1-A are one class, the rendering-side half of round 1's class: **external input reaches third party rendering code outside a listed subset (invariant I11)**, so it can make that code allocate without bound (P0-A) or fail without file context (P1-A). Round 1 restricted the map and its tileset but left the tileset image's subset as a header check, and admitted geometry the renderer cannot create. The class was closed by enumerating every external input the renderer receives and restricting each (DECISIONS D16):

| External input | Before | Now |
|----------------|--------|-----|
| Map file, external tileset | Checked by `sim` (D13), then read again by Bevy from disk | Checked by `sim`; the renderer receives exactly the checked bytes from memory |
| Tileset geometry | Up to 1024 tiles, images up to 65,536 px | At most 256 tiles and 8192 px per side, the limits every GPU supports (`wgpu::Limits::default()`) |
| Tileset image | 24-byte header check, then read and decoded by Bevy without limits | Bounded read (4 MiB), chunk and `IHDR` allow-list, full decode with Bevy's decoder under a 16 MiB limit, RGB8/RGBA8 only; the renderer receives exactly the checked bytes |
| `<asset>.meta` files | Read whole, without a size limit, for every asset, and allowed to choose the loader and settings | Never read |
| Any other file through the default asset source | Read from disk | Refused (load failure, exit code 1 by D14) |

Two members of the class that the review did not report were found and fixed with it: a 16-bit greyscale PNG (Bevy makes an `R16Uint` texture, and the reviewed release binary quit with "Texture binding 0 expects sample type Float { filterable: true }, but was given a view with format R16Uint", no file named), and `.meta` files (a 400 MB `demo_tiles.png.meta` made the reviewed release binary peak at 0.65 GB).

### Findings

| Finding | Outcome | Evidence | Change | Regression test (fails before, passes after) |
|---------|---------|----------|--------|-----------------------------------------------|
| P0-A PNG with a compressed ancillary chunk allocates gigabytes | **Confirmed** | Reviewed release binary, demo map with a 575,260-byte `demo_tiles.png` (an `iCCP` chunk of 500 MiB of compressed zeros): exit 0, map created, peak memory footprint 0.50 GB. Source: `bevy_image` `image.rs:1602` `no_limits()`, `image` `codecs/png.rs:50-51`, `png` `stream.rs:1726`. | D16 items 2 and 3: `crates/game/src/tileset_image.rs` (bounded read, chunk and `IHDR` allow-list, decode under `image::Limits`), `crates/game/src/checked_assets.rs` (the renderer reads only the checked bytes). After the fix, the same file exits 1 in debug and release with `tileset image …/demo_tiles.png: chunk "iCCP": chunk is not supported`, peak 0.10 GB. | `crates/game/tests/m1_tileset_image.rs` (6 tests, 64 MiB allocator budget, D16 item 5), in particular `compressed_and_other_unlisted_chunks_are_rejected_before_any_decoding` (`iCCP`, `zTXt`, `iTXt` bombs of 256 MiB, plus 8 other chunk types before and after the image data) and `image_files_above_the_size_limit_are_rejected_without_being_read_whole` (4 MiB + 1 and a 1 GiB sparse file). All 6 fail against the reviewed check; with the chunk check removed and `no_limits()` restored, the bomb test aborts on the budget ("memory allocation of 69206016 bytes failed"). Also `map::tests::tileset_images_other_than_the_declared_png_are_rejected` (see "Tests changed"). |
| P1-A tileset images beyond the renderer's texture limits, failing without context | **Confirmed** | Reviewed release binary: a 257 × 1 tileset (16448×32 image) exits 1 with only `Caught rendering error: Validation Error`; 256 × 1 (16384 px) renders on this Apple M5, which is above the limit wgpu guarantees (8192). Source: `bevy_ecs_tilemap` `texture_array_cache.rs` (one 2D texture of the image, one array layer per tile), `bevy_render` `renderer/mod.rs:299-310` (adapter limits), `wgpu-types` `limits.rs:308-378`. | D16 item 1: `MAX_TILESET_TILES` 1024 → 256, new `MAX_TILESET_IMAGE_SIDE_PX` = 8192, checked in `map_subset.rs` with a typed `MapError` naming the file, the image attribute, and the limit. The texture format is restricted by the image subset (P0-A row). | `m1_map_subset::tileset_images_are_bounded_by_the_texture_limits_every_gpu_supports`: accepted 128×2 (8192×64), 128×1, 1×256 (64×8192), 16×16; rejected 129×1 (8256 px, the smallest rejected width), 256×1, a 1×257-row image with `tilecount` 256 (8224 px high), and `tilecount` 257 and 1024. Against the reviewed code the 129×1 map loads. `game::map::tests::tileset_limits_fit_the_texture_limits_every_gpu_supports` ties both constants to `WgpuLimits::default()` (fails with 1024 tiles). `m1_tileset_image::pixel_formats_other_than_8_bit_rgb_and_rgba_are_rejected` (16-bit greyscale → `R16Uint` in Bevy, rejected) and `images_inside_the_subset_pass_and_their_checked_bytes_are_returned` (the largest images, 8192×64 and 64×8192, pass and become `Rgba8UnormSrgb` in Bevy). Boundary through the binaries, debug and release, `--smoke-test 300`: 128×2, 128×1, 1×256, 16×16 exit 0 with the map created and no error; 129×1, 256×1, 257×1 exit 1 with the typed error. |
| P2-A.1 byte order mark reported as an unterminated end tag | **Confirmed, fixed** | Reviewed code: `bom.tmx` → "the root element's end tag is not terminated" (scratch run of the new test). | D17 item 1: rejected first, as such. | `m1_map_subset::a_byte_order_mark_is_named_as_the_cause` (map and `.tsx`) |
| P2-A.2 missing tileset or tileset after the layer reported as an empty cell | **Confirmed, fixed** | Reviewed code: `no_tileset.tmx` → `layer "terrain" cell (0, 0): cell has no tile`. | D17 item 2: exactly one tileset, before the layer (`MissingElement`, `OutOfOrder`); D13 amended. | `m1_map_subset::a_map_has_exactly_one_tileset_before_its_layer` (missing, after the layer, empty `<map/>`) |
| P2-A.3 missing `.tsx` reported at field `file` | **Confirmed, fixed** | Reviewed code: field `file`, kind `Parse("external tileset … cannot be read")`. | D17 item 3: field `tileset "x.tsx" attribute "source"`, kind `Read` naming the `.tsx` path; an oversized `.tsx` is reported in the `.tsx`. | `m1_map_subset::a_missing_external_tileset_is_a_read_error_of_its_source_attribute`; `m1_map::malformed_xml_and_wrong_root_are_parse_errors` (see "Tests changed") |
| P2-A.4 `.tsx` errors printed as `map <path>.tsx` | **Confirmed, fixed** | `MapError`'s `Display` (reviewed `map.rs:153-163`). | D17 item 4: `tileset <path>` for a `.tsx`. | `m1_map_subset::errors_in_external_tilesets_are_reported_as_tileset_files` |
| P2-B `f32` parse in `sim` for `float` properties | **Confirmed as stated; recorded as debt, not narrowed** | `map_subset.rs:503`. The parse result is a `bool` that is discarded, so no simulation state or logic uses a float (I2 is about state and logic). | None. Narrowing the subset to `int` and `bool` would change the required M1 malformed-map tests in `m1_map.rs` (unchanged since review 1), which rely on `string` and `float` values reaching validation so that the error states the wrong type precisely (`m1_map::schema_version_is_required_and_checked` with a `string` value, `missing_and_mistyped_tile_properties_are_rejected` with `float` and `string` values, each asserting `WrongPropertyType` with the type found). Debt item 4 below. | None |
| P2-C the double read spans three files | **Confirmed, fixed** | Reviewed `game/src/map.rs`: `sim::load_map` and `check_tileset_image` read the files, and the asset server read them again. | D16 item 3: the asset server receives the checked bytes from memory; nothing is read twice. Closes round 1 debt item 1 (P2-1). | `checked_assets::tests::only_registered_bytes_are_served_and_meta_files_are_never_read`, `checked_assets::tests::the_plugin_replaces_the_default_asset_source`; end to end, the smoke runs render the demo map and an external-tileset variant from the checked bytes. |
| P2-D `tick.0 += 1` and I12 | **Confirmed, fixed** | Panics in debug at `u64::MAX` (scratch run: "attempt to add with overflow" at `schedule.rs:111`). | D17 item 5: `wrapping_add`. Every other integer operation in `crates/sim/src` was reviewed; none can overflow. | `sim::schedule::tests::tick_counter_wraps_instead_of_overflowing` |

### Tests changed (none removed, skipped, or weakened)

1. `m1_map::malformed_xml_and_wrong_root_are_parse_errors`: of its three cases, the missing external tileset asserted field `file` and any `Parse` error, which is the behaviour P2-A.3 reports as wrong. The case is kept and now asserts the exact field (`tileset "nowhere.tsx" attribute "source"`), the file (the map), and a `Read` kind naming the `.tsx` path; the other two cases are unchanged. The assertion is stricter than before.
2. `game::map::tests::tileset_images_other_than_the_declared_png_are_rejected`: the five rejections are unchanged (the error is now a typed value, compared through its message as before). Its last line asserted that a file holding only a 24-byte PNG header is accepted, which is the header-only behaviour P0-A reports. That file is now asserted to be rejected (`chunk "IHDR": the file ends inside this chunk`), and a complete 512×32 PNG is asserted to be accepted with its bytes returned.
3. `game::map::tests::demo_tileset_image_matches_its_declared_size` now also asserts that the returned bytes equal the file.

### Files changed

| File | Change |
|------|--------|
| `crates/sim/src/map.rs` | `MAX_TILESET_TILES` = 256, `MAX_TILESET_IMAGE_SIDE_PX` = 8192; `MapErrorKind::MissingElement`, `OutOfOrder`; `Display` says `tileset` for a `.tsx`; `CheckedFile` and `load_map_with_files` (`load_map` unchanged in behaviour) |
| `crates/sim/src/map_subset.rs` | Image side limits; exactly one tileset before the layer; byte order mark; missing `.tsx` at its `source` attribute |
| `crates/sim/src/schedule.rs` | `wrapping_add`; unit test |
| `crates/sim/src/lib.rs` | Exports |
| `crates/sim/tests/m1_map_subset.rs` | 5 tests |
| `crates/sim/tests/m1_map.rs` | One case made stricter (above) |
| `crates/game/src/tileset_image.rs` (new) | Tileset image subset and bounded decode (D16 item 2) |
| `crates/game/src/checked_assets.rs` (new) | Default asset source serving only checked bytes, no `.meta` reads (D16 item 3); 2 tests |
| `crates/game/src/map.rs` | Loads through `load_map_with_files` and `check_tileset_image`, registers the checked bytes; header-only check removed; texture limit test; tests adapted (above) |
| `crates/game/src/main.rs` | Modules; `CheckedAssetsPlugin` added before `AssetPlugin` |
| `crates/game/tests/m1_tileset_image.rs` (new) | 6 bounded tests (D16 item 5) |
| `crates/game/Cargo.toml`, `Cargo.lock` | `image` =0.25.10; dev `flate2` =1.1.10, `crc32fast` =1.5.2; the lock gained no package, only `game`'s dependency list |
| `docs/DECISIONS.md` | D13 amended; D16, D17 |
| `docs/STATUS.md` | This report |

### Validation commands (PLAN.md section 9, with section 12 item 5)

Run from the repository root on macOS (Apple M5, Rust 1.98.1), in this order, in the foreground, after the last code change. A display was available.

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 115 passed, 0 failed, 0 ignored: `game` unit 29 (cli 7, projection 7, camera 7, map 6, checked_assets 2), `m0_spike` 6, `m1_demo_map` 3, `m1_tileset_image` 6 (2.8 s), `sim` unit 3, `m0_bootstrap` 12, `m1_map` 32, `m1_map_subset` 24 (5.8 s), doc-tests 0. Known M0 linker warning only. |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | `map maps/demo/demo.tmx: 64x64 cells, tile 64x32 px`; `smoke test: 600 frames, 198 simulation ticks, map created: true` |
| `cargo run -p game -- --smoke-test 600` | 0 | `smoke test: 600 frames, 203 simulation ticks, map created: true` |

Additionally `cargo test --release -p sim -p game` passed (the same 115 tests), and both fixed binaries were run (`--smoke-test 300`) against scratch asset roots: `iCCP` bomb, 16-bit greyscale image, 257×1, 256×1, and 129×1 tilesets exit 1 with a typed error naming the file and field, peak 0.10 GB; 128×2, 128×1, 1×256, 16×16, an external `.tsx` variant of the demo map, the demo map with a 400 MB `demo_tiles.png.meta` and a malformed `demo.tmx.meta`, and an image with 500 MiB of compressed zeros after its pixel data inside `IDAT` exit 0 with the map created and no error, peak 0.11 GB.

### Acceptance criteria (PLAN.md section 7, M1)

| Criterion | Evidence |
|-----------|----------|
| Map loading with terrain flags and ore; `Grid` resource | Unchanged positive tests pass; `load_map_with_files` returns the same `LoadedMap` |
| Map rendering | Smoke runs above (map created in both profiles, from the checked bytes); on-screen result **requires user verification** |
| Camera panning, zoom with limits, camera bounds | Unchanged tests pass; behaviour in the window **requires user verification** |
| Cursor to cell picking; zero error in tests | Unchanged tests pass |
| Required M1 tests | All present and passing; one case of `malformed_xml_and_wrong_root_are_parse_errors` made stricter (above) |
| I8, I11 | D13, D16, D17; `m1_map_subset` (24), `m1_tileset_image` (6), game unit tests; binary runs above |
| I12 | `wrapping_add` and its test; review of `crates/sim/src` (D17 item 5) |
| A 64×64 demo map renders correctly; the user confirms the visual result | **Requires user verification** (below) |

### Assumptions verified and how

| Item | Verification |
|------|--------------|
| Bevy reads `<asset>.meta` whole for every asset unless the reader reports it missing, and a missing meta means default settings | `bevy_asset` `server/mod.rs` `get_meta_loader_and_reader`; probe with a 400 MB meta file (0.65 GB before, no effect after) |
| A pre-registered default source is kept by `AssetPlugin` | `bevy_asset` `io/source.rs:382-385` (`get_or_insert_with`); `checked_assets::tests::the_plugin_replaces_the_default_asset_source` |
| `bevy_ecs_tiled` requests the `.tsx` and the image through the default source at `maps/demo/<name>` | `bevy_ecs_tiled` `tiled/reader.rs` (`read_asset_bytes`), `tiled/map/loader.rs` (`AssetPath::from(img.source)`); smoke runs of the demo map and of an external-tileset variant |
| `image::Limits::max_alloc` bounds `png`'s ancillary chunk decompression cumulatively | `image` `codecs/png.rs:47-51`, `png` `decoder/mod.rs:65-87`, `stream.rs:783-791, 1726-1728`; the bounded test with the chunk check removed |
| Bevy's PNG texture formats | `bevy_image` `image_texture_conversion.rs`; `bevy_texture_format` assertions in `m1_tileset_image` |
| Texture limits guaranteed on every modern backend | `wgpu-types` 29.0.4 `limits.rs:308-378` (8192, 256); `bevy_render` `settings.rs:91-110`, `renderer/mod.rs:297-310` |

### Deviations from PLAN.md

1. The map format is narrower (D16, D17): at most 256 tiles and 8192 px per image side, exactly one tileset before the layer, no byte order mark. The demo map and all fixtures are inside it and unchanged.
2. The tileset image format is now an explicit subset (D16): 8-bit RGB or RGBA PNG, no interlacing, listed chunks only, at most 4 MiB.
3. Public API of `sim`: `CheckedFile`, `load_map_with_files`, `MAX_TILESET_IMAGE_SIDE_PX`, and two `MapErrorKind` variants were added; `MAX_TILESET_TILES` changed from 1024 to 256. No data schema change, so `schema_version` stays 1.
4. Files not listed in PLAN.md section 4: `crates/game/src/{tileset_image,checked_assets}.rs` and `crates/game/tests/m1_tileset_image.rs`, which contains `unsafe` code (the counting allocator of D15).

### Known debt (P2)

1. Round 1 debt item 1 (P2-1, double read) is **resolved** by D16 item 3, together with P2-C.
2. Round 1 debt items 2 (P2-3, camera and picking systems tested through pure functions) and 3 (P2-5, `Grid` inserted as match setup) are unchanged.
3. Round 1 debt item 4 (an asset load failure is logged twice under `--smoke-test`) is unchanged. Items 5 (byte order mark) and 6 (header-only image check) are **resolved** (D17 item 1, D16 item 2).
4. **New, P2-B:** `map_subset.rs` validates `float` property values with `str::parse::<f32>`, as `tiled` parses them. The result is a discarded `bool`, so no simulation state or logic uses a floating point value (I2). The subset keeps `string` and `float` so that the required malformed-map tests of `m1_map.rs` keep reporting the precise wrong type; narrowing it is possible once those tests' expectations are allowed to change.
5. **New:** the texture limits are wgpu's guaranteed defaults. A machine forced to WebGL2 limits (`WGPU_SETTINGS_PRIO=webgl2`) could not create an 8192 px texture; this configuration is outside the supported one (D16 item 1).
6. **New:** every asset must now be registered in `CheckedAssets` after its check before Bevy can load it. Later milestones that add sprites, audio, or data files through the asset server must give each a checked subset and register it, or the load fails with exit code 1 (fail-safe).

Carried from M0 unchanged: items 1, 2 (seen again in this round's smoke runs), 3, 5, 7, 8, and 9, as listed in the round 1 report.

### Requires user verification

Unchanged (section 12 item 2; not review findings). Run `cargo run --release -p game` and check map rendering (64×64 green diamond, red top corner (0, 0), magenta right corner (63, 0), white left corner (0, 63), rock ridge with a gap, lake with sand shore, ore fields), picking (outline and dot on the tile under the cursor, click log), panning (arrow keys, screen edge while focused, middle mouse drag), zoom (2× to 0.5×), and bounds (the view stays within the map's bounding rectangle; vertically centred when taller than the map). This round changed no rendering, camera, or picking code; the renderer now receives the same files from memory instead of from disk.

## M1 review decision (2026-10-04)

Three independent review rounds (`docs/reviews/M1-review-1.md`: NO-GO, 4 P0, 3 P1, 6 P2; `M1-review-2.md`: NO-GO, 1 P0, 1 P1, 4 P2; `M1-review-3.md`: GO WITH DOCUMENTED NON-BLOCKING DEBT, 0 P0, 0 P1, 3 P2). Before review 3 the orchestrator reran all six validation commands of PLAN.md section 9 on commit `352e526` with exit code 0 (115 tests passed). Under PLAN.md section 12 item 1, `milestone/m1` was merged into `main` and tagged `m1-reviewed`. The tag `m1-approved` is created by the user alone after the verification below.

### Debt recorded from review 3 (P2, verbatim from the report)

#### P2-1: A FIFO (named pipe) at the map, tileset, or image path hangs the game indefinitely at startup, with no message, in both profiles

* **Files and lines:** `crates/sim/src/map_subset.rs:47-48` (`read_file`: `std::fs::File::open(path)`), used for the map and the external `.tsx`; `crates/game/src/tileset_image.rs:151-152` (`read_bounded`: `File::open`).
* **Evidence:**
  * A FIFO named `demo_tiles.png` or `demo.tmx` in a scratch asset root, run with `--smoke-test 150`. Debug and release binaries both stay blocked in the `Startup` system `load_map` after `Creating new window`, until the 15 s alarm kills them (exit 142). Nothing is logged and the smoke test never exits.
  * `simprobe` with a FIFO `ext.tsx` blocks the same way (exit 142 at the 5 s alarm).
  * On macOS, opening a FIFO for reading blocks until a writer appears.
  * By contrast, a symlink to `/dev/zero`, a directory, and an unreadable file are all rejected with typed errors.
* **Severity rationale:** the cause is a non-regular file in the asset directory, not malformed file content. There is no panic, no allocation, and no wrong simulation result, so I rate it P2 robustness debt rather than an I8 violation.
* **Expected behaviour:** before opening, `read_file` and `read_bounded` require `std::fs::metadata(path)?.is_file()` (or open non-blocking and check the file type). Anything else is a typed `Read` error naming the file. A test covers a FIFO or another special file.

#### P2-2: Non-numeric CSV entries are rejected by `tiled` with field `file` and a bare integer-parse message, without the layer or cell

* **Files and lines:**
  * `crates/sim/src/map_subset.rs:835-886` (`data`) counts the comma-separated entries but does not check their content. DECISIONS D13 (`data` row) lists only the entry count.
  * `crates/sim/src/map.rs:400-406` then reports `tiled`'s failure as `field: "file"`, `MapErrorKind::Parse`.
* **Evidence:** `simprobe` on the demo map with the first CSV entry replaced, debug and release:

  | Entry | Error |
  |-------|-------|
  | `x` or `-1` | `map …/demo.tmx: file: not a valid Tiled map: invalid digit found in string` |
  | empty | `… file: not a valid Tiled map: cannot parse integer from empty string` |
  | `4294967296` | `… file: not a valid Tiled map: number too large to fit in target type` |
  | `0` (for comparison) | `layer "terrain" cell (0, 0): cell has no tile` |
  | `268435457` (for comparison) | `layer "terrain" cell (0, 0): tile 268435456 … is not defined` |

  * The rejections themselves are correct and typed, and there is no panic.
  * In the fuzz corpus, 109 rejections have this form.
  * No test covers a non-numeric CSV entry.
  * PLAN.md section 5 requires a load error to name the field. This one names neither the layer data nor the cell, and the reason is Rust's `ParseIntError` text.
* **Expected behaviour:**
  * The checker parses each entry as `tiled` does (`trim()`, then `u32`) and reports `layer "terrain" data` with the entry index or cell and the offending value.
  * D13 lists the rule.
  * A test covers `x`, an empty entry, `-1`, and `4294967296`.

  This is the same class as review 2 P2-A (correct rejection, imprecise reason).

#### P2-3: Stale or inaccurate documentation after the remediation rounds

* `crates/game/src/map.rs:173`: the doc comment of `misalignment` says "These are rendering properties, so `sim` does not check them". Since round 1, `sim` rejects offsets and parallax before the hand-off (D13), and D9 describes `misalignment` as a second line of defence.
* `crates/sim/src/map.rs:268`: `MapErrorKind::Parse` prints "not a valid Tiled map: …" for `.tsx` files and for faults that the subset checker itself finds. Example: `tileset …/ext.tsx: file: not a valid Tiled map: a UTF-8 byte order mark at the start of the file is not supported`.
* `crates/sim/src/map.rs:136-137`: `LoadedMap::tileset_image` cites D13 for the image check, whose image paragraph is superseded by D16.
* `docs/DECISIONS.md:89` (D6): the smoke test still exits 0 only if "the spike map was created". Since M1, the condition is that the validated demo map is created, but D6 carries no M1 amendment.
* **Expected behaviour:** the comments and D6 match the current behaviour. The `Parse` wording is neutral, for example "cannot be parsed".

### Debt carried from earlier rounds (as listed by review 3)

* The new items P2-1, P2-2, and P2-3 above.
* Still open from earlier rounds and already recorded in STATUS:
  * review 1 P2-3 and P2-5;
  * review 2 P2-B;
  * round 1 item 4 (an asset load failure is logged twice under `--smoke-test`);
  * round 2 items 5 (WebGL2 limits are unsupported) and 6 (every asset must be registered in `CheckedAssets`);
  * M0 items 1, 2, 3, 5, 7, 8, and 9.

### Requires user verification (blocks only `m1-approved`; verbatim from review 3)

These items are not findings and do not affect the decision (PLAN.md section 12 item 2). They block only the user's tag `m1-approved`. Run `cargo run --release -p game` and check, as listed in docs/STATUS.md:

1. **Map rendering:** a 64×64 green diamond map, tiles edge to edge with no gaps or overlaps. Red at the top corner (0, 0), magenta at the right corner (63, 0), and white at the left corner (0, 63). A grey rock ridge with a gap, a blue lake with a sand shore right of centre, and gold ore fields.
2. **Picking:** the white outline and centre dot sit on the tile under the cursor, including along all four edges, and disappear outside the map. A left click logs the cell (for example `cell (0, 0): passable true, buildable true, ore 0`), or `click outside the map`.
3. **Panning:** the arrow keys pan, and diagonals are not faster. The screen edge pans within 8 px, only while the window has focus. Dragging with the middle mouse button keeps the grabbed point under the cursor.
4. **Zoom:** the wheel or trackpad zooms, limited to 2× and 0.5× magnification.
5. **Bounds:** the view never leaves the map's bounding rectangle at any zoom or window size, and stays vertically centred when the view is taller than the map.

## M2 implementation report (2026-10-04)

Branch `milestone/m2`, based on `main` at the merge of M1 (tag `m1-reviewed`). Commits: `d690d4c` (sim: definitions, units, commands, pathfinding, movement, tests), `7ac87d1` (game: units, selection, sprites, smoke test), and the documentation commit that adds this report. Decisions: DECISIONS A5 follow-up, D18 to D20, and the M2 amendment of D6.

### Files changed

| File | Change |
|------|--------|
| `crates/sim/Cargo.toml`, `Cargo.lock` | `ron` =0.12.2 and `serde` =1.0.229 (derive); the lock gained no package, only `sim`'s dependency list |
| `crates/sim/src/data.rs` (new) | RON subset checker driven by the file's schema, bounded reading of regular files only, `DataError` (D18) |
| `crates/sim/src/definitions.rs` (new) | `Definitions` resource, `UnitDef`, `UnitKind`, `load_unit_definitions` (units file schema 1) |
| `crates/sim/src/unit.rs` (new) | Unit components (`Owner`, `Unit`, `Mobile`, `Position`, `Facing`, `Orders`), `Direction` (eight facings), `spawn_unit`, hash layout (D19) |
| `crates/sim/src/pathfinding.rs` (new) | `find_path` (A*), `destination_cells`, `nearest_reachable`, the budgeted `find_paths` system with repath, wait-cycle, and retarget rules |
| `crates/sim/src/movement.rs` (new) | `move_units`: reservation before entry, exact speed, blocking, stall limit |
| `crates/sim/src/event.rs` (new) | `SimEvent` (`CommandRejected`, `MoveCompleted`, `MoveFailed`), `MoveFailure`, per-tick event buffer |
| `crates/sim/src/command.rs` | `Command::Move`/`Stop`, `RejectReason`, validation, group destination assignment; the M0 `#[expect(clippy::never_loop)]` is gone with the first variants (D5) |
| `crates/sim/src/map.rs` | `SubPos`, `SUBCELLS_PER_CELL`; `Grid` occupancy (`occupant`, `claim`, `release`), `is_passable`, `set_terrain` (setup); occupancy in the hash |
| `crates/sim/src/{schedule,hash,lib}.rs` | Systems in `Commands`, `Pathfinding`, `Movement`; `Definitions` and `Messages<SimEvent>` resources; units and definitions in `state_hash`; exports |
| `crates/sim/tests/m2_definitions.rs` (new), `tests/fixtures/data/units.ron` (new) | 14 tests, bounded as in D15 (64 MiB allocator budget, 30 s per load) |
| `crates/sim/tests/m2_movement.rs` (new), `tests/fixtures/maps/{open_16x16,obstructed_16x16,arena_32x32,wall_32x32}.tmx` (new) | 26 tests through `SimPlugin` and `CommandQueue` |
| `crates/game/src/units.rs` (new) | Match setup (definitions, sprite checks, starting units), unit views with facings and interpolation, event log; 6 tests |
| `crates/game/src/selection.rs` (new) | Click, box, and Shift selection; right click `Move`, `S` `Stop`; selection drawing; 4 tests |
| `crates/game/src/tileset_image.rs` | `check_png_file` (the PNG subset for any declared size); non-regular files rejected before opening |
| `crates/game/src/{main,map,smoke}.rs` | Plugins and the position recording before each tick; `load_map` visible to the crate; smoke test requires units and orders a move |
| `crates/game/examples/gen_unit_sprites.rs` (new), `assets/sprites/units/{light_tank,scout}.png` (new) | Placeholder sprite sheet generator and its output |
| `crates/game/tests/m2_unit_sprites.rs` (new) | 5 tests: generator output, PNG subset, facing order and direction, named pipe |
| `assets/data/units.ron` (new) | Production unit definitions (light tank speed 24, scout speed 40) |
| `assets/LICENSES.md`, `docs/DECISIONS.md`, `docs/STATUS.md` | Provenance of the new assets; decisions; this report |

### Validation commands (PLAN.md section 9, with section 12 item 5)

Run from the repository root on macOS 27.0.1 (Apple M5, Rust 1.98.1), in this order, in the foreground, after the last code change (`7ac87d1`). A display was available.

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 170 passed, 0 failed, 0 ignored: `game` unit 39 (cli 7, projection 7, camera 7, map 6, checked_assets 2, units 6, selection 4), `m0_spike` 6, `m1_demo_map` 3, `m1_tileset_image` 6, `m2_unit_sprites` 5, `sim` unit 3, `m0_bootstrap` 12, `m1_map` 32, `m1_map_subset` 24, `m2_definitions` 14, `m2_movement` 26 (1.7 s), doc-tests 0. Known M0 linker warning only. |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | `26 units spawned`; `smoke test: 600 frames, 198 simulation ticks, map created: true, units spawned: 26, units moved: 18 in 198 ticks after the order` |
| `cargo run -p game -- --smoke-test 600` | 0 | `smoke test: 600 frames, 203 simulation ticks, map created: true, units spawned: 26, units moved: 18 in 203 ticks after the order` |

Additionally: `cargo test --release -p sim --test m2_movement --test m2_definitions` passed (26 and 14 tests). The release binary, run (`--smoke-test 300`) against scratch asset roots, exited 1 with typed errors for a units file starting with `#![enable(implicit_some)]` (`data file …/units.ron: file: line 1, column 1: expected a value or punctuation of the supported subset, found '#'`) and for a missing `scout.png` (`data file …/units.ron: units[1].sprite: sprite image …/scout.png: file: cannot be read: …`).

### Acceptance criteria and required tests (PLAN.md section 7, M2)

| Criterion | Evidence |
|-----------|----------|
| Scope: unit definitions from RON | `sim::load_unit_definitions`, D18; `m2_definitions` (14 tests: fixture, layout variations, inclusive limits, unknown/missing/duplicate fields, wrong kinds with line and column, schema versions, ranges, list bounds, duplicate ids, 18 RON features outside the subset, non-ASCII bytes and BOM, content after the root and every truncation, 60,000-deep nesting, missing/directory/oversized/1 GiB sparse/named-pipe files, 4,000 seeded mutations); production file and sprite cross validation: `units::tests::the_production_units_load_and_every_sprite_passes_its_check` |
| Scope: spawning | `spawn_unit`; `spawning_is_checked` (unknown type, outside, impassable, occupied, no state change); game starting units (smoke run: 26 spawned) |
| Scope: selection by click, drag box, and additive modifier | Pure functions `unit_at`, `units_in_box`, `apply_selection` with tests `a_click_selects_the_unit_whose_body_is_under_the_cursor`, `a_box_selects_the_units_whose_ground_point_lies_inside`, `shift_adds_to_the_selection_and_otherwise_replaces_it`; behaviour in the window **requires user verification** |
| Scope: `Move` and `Stop` | `orders_are_commands_for_the_whole_selection` (interface); `stop_ends_the_order_at_the_next_cell_centre`, `a_new_move_replaces_the_current_order`, smoke runs (the ordered move moved 18 units) |
| Scope: A* with a per tick request budget | `path_cost_is_optimal_on_open_and_obstructed_fixture_maps`, `no_diagonal_step_between_two_blocked_orthogonal_neighbours`, `path_requests_are_served_within_the_per_tick_budget_oldest_first` |
| Scope: cell reservation before entry | `move_units`; the I6 checks below (a reserved cell is held before the unit's position enters it) |
| Scope: distinct destination cells for group orders | `a_group_order_gives_every_unit_its_own_destination_cell` |
| Scope: repathing when blocked | `a_newly_placed_obstacle_triggers_a_repath`, `a_unit_standing_in_the_way_is_bypassed`, `a_destination_held_by_an_idle_unit_is_replaced_by_the_nearest_reachable_cell`, `two_units_that_want_each_others_cell_resolve_by_one_stepping_aside` (asserts the step aside), `units_meeting_head_on_in_a_one_cell_corridor_both_pass`, `every_order_ends_even_when_units_are_walled_in_by_waiting_units` |
| Scope: eight sprite facings | `facing_follows_each_step_direction` (sim, all eight); `every_facing_has_its_own_frame`, `the_facing_order_is_the_order_of_sim_directions`, `every_frame_points_in_the_screen_direction_of_its_facing`; on screen **requires user verification** |
| Scope: placeholder sprites generated by the repository | `gen_unit_sprites`; `committed_sprite_sheets_match_generator_output`, `sprite_sheets_pass_the_png_subset_with_their_declared_size`, `the_sprite_layout_matches_the_generator` |
| Test: path cost is optimal on open and obstructed fixture maps | `path_cost_is_optimal_on_open_and_obstructed_fixture_maps` (A* against an independent Dijkstra, every start cell and every third goal of both maps, more than 10,000 compared paths each, path legality and cost re-summed); `executed_route_is_optimal_through_the_production_path` (the cells a unit walks through `CommandQueue` cost exactly the optimum) |
| Test: an unreachable target ends the order with an event and no panic | `unreachable_target_ends_the_order_with_an_event_and_no_panic` (sealed pocket: `MoveFailed { Unreachable }`; group into the pocket: one `Unreachable`, two `NoDestination`; target in solid rock: `NoDestination`); obstacle on the destination: `Unreachable` |
| Test: a move outside the map is rejected | `a_move_outside_the_map_is_rejected` (seven targets including `i32` extremes; `CommandRejected { OutsideMap }`, state hash equal to a control run at every tick); further rejections: `commands_for_missing_foreign_or_repeated_units_are_rejected` |
| Test: a unit ordered to its own cell stays | `a_unit_ordered_to_its_own_cell_stays` (`MoveCompleted` in the first tick, unchanged for 100 ticks) |
| Test: a newly placed obstacle triggers a repath | `a_newly_placed_obstacle_triggers_a_repath` (a wall placed across the path mid-route; repath counted; arrival without entering rock) |
| Test: I6 on every tick while 50 units cross each other for 2,000 ticks | `invariant_i6_holds_while_fifty_units_cross_each_other_for_2000_ticks` (two groups of 25 swap sides repeatedly; I6 checked from units and grid independently on every tick; 4 crossings started and 199 orders completed, no failure); also `invariant_i6_holds_and_every_order_ends_under_seeded_random_orders` (3 seeds × 3,000 ticks of random orders, then every order ends) |
| Test: determinism of the same scenario | `the_same_scenario_gives_the_same_hash_at_every_tick` (50 units, 600 ticks, identical traces; a different command sequence diverges after it); `the_hash_covers_unit_state` |
| Acceptance: 50 units move as a group to a clicked destination without overlap or deadlock in the fixture scenarios | `fifty_units_reach_a_clicked_destination_without_overlap_or_deadlock`: four scenarios (open arena, both directions; wall with two gaps, both directions); all 50 units complete within 3,000 ticks (540 to 945 ticks), I6 holds on every tick, the final cells are distinct, all lie within one ring of the 50 assigned cells, and 39 to 45 units stand on their assigned cells. In the real game the group order is issued by a right click (smoke runs); on screen **requires user verification** |
| Section 8: negative, boundary, integration, end to end | Rejections with no state change (above); boundaries: speed 1 and 128, 32-character names, 64 unit types, 64 KiB file, `MAX_UNITS_PER_COMMAND` ± 1, map edges, budget exactly 16 per tick; integration through `SimPlugin` and `CommandQueue` (all `m2_movement` tests); end to end: both smoke runs |

### Assumptions verified and how

| Item | Verification |
|------|--------------|
| `ron` 0.12.2 and `serde` 1.0.229 are in the lock and pure Rust | `Cargo.lock` diff (dependency list only), `cargo tree -p sim -e normal` |
| `ron` API, defaults, whitespace, and extension attribute | Source (`options.rs`, `parse.rs`, `error.rs`, `de/mod.rs`); `m2_definitions` (the mutation test asserts `ron` accepts every file the checker accepts) |
| Bevy 0.19.1: `Messages::update`/`iter_current_update_messages`, `MessageReader`, `World::write_message`, `Sprite::rect`, `Time<Fixed>::overstep_fraction`, `ButtonInput::{just_released, any_pressed}`, `Rect::{from_corners, contains}` (inclusive), `Camera::viewport_to_world_2d`, `Gizmos::linestrip_2d` | Source (paths in D19, D20) and compiled code exercised by the tests and smoke runs |
| `bevy_ecs_tiled` places the map's layer at z = 0 | `tiled/map/spawn.rs:48, 145` (accumulated offset starting at −100 per layer, +100 per layer) |
| `mkfifo` exists on the development machine (used by two tests) | The tests ran |

### Deviations from PLAN.md

1. Command validation checks ownership, existence, and bounds, but not visibility: there is no fog of war before M4, so every cell is visible (D19 item 3). Visibility checks arrive with fog in M4.
2. A group's destination cells are distinct when assigned. A unit whose cell is later closed off by units that arrived first takes the nearest cell it can reach (D19 item 6), so in the fixture scenarios 5 to 11 of 50 units end next to the assigned cells instead of on them; the final cells are always distinct (I6).
3. Additional rejection reasons not named in the plan: `NoUnits`, `TooManyUnits` (more than 1,024), and `DuplicateUnit`. Additional events: `MoveCompleted` and `MoveFailed` (`NoDestination`, `Unreachable`, `Blocked`).
4. `spawn_unit` and `Grid::set_terrain` change state directly; they are scenario and match setup (D19 item 10), as permitted by PLAN.md section 4. The game's starting units are hard-coded until match setup in M6 (D20 item 4).
5. The units file has only `id`, `speed`, and `sprite`; hit points, armour, sight, and weapons arrive with the milestones that use them, each with a schema version increment.
6. The stop key is `S` (hotkeys are M5's scope; D20 item 5). Only the player's own units can be selected.
7. Files not listed in PLAN.md section 4: `crates/sim/src/{data,definitions,unit,pathfinding,movement,event}.rs`, `crates/game/src/{units,selection}.rs`, `crates/game/examples/gen_unit_sprites.rs`, and the M2 test files. `m2_definitions.rs` contains `unsafe` code (the counting allocator of D15).
8. The image reader now rejects non-regular files (D20 item 2), a behaviour change of M1 code made because the sprite sheets share the reader; it resolves M1 review 3 P2-1 for images. The map reader in `sim` (`map_subset::read_file`) is unchanged.

### Known debt (P2 proposals for the reviewer)

1. Group arrival leaves holes in the cluster when late units find their cells closed off (deviation 2); a smarter assignment (for example, swapping destinations with arrived units) would fill them.
2. The game's event log and the smoke test read `SimEvent` with a `MessageReader`, which sees the last two ticks; if more than two ticks run between two frames, older events are not seen. Only logging and the smoke test's rejection check are affected.
3. The selection, order, and unit drawing systems are tested through their pure functions (as M1 review 1 P2-3 for the camera); their wiring in the window is part of the user verification below.
4. Each path search allocates arrays of the map's size (at most 65,536 cells); performance is measured in M7.
5. `Grid::set_terrain` and `spawn_unit` are public, so the game could call them during a match; invariant I5 is a discipline until M5 enforces the interface boundary.
6. A screenshot taken with Bevy's `Screenshot` during a smoke run in this environment was entirely black (checked by decoding the PNG), so no automated image of the running game exists; rendering is verified by the user.

M1 debt status: review 3 P2-1 is resolved for tileset images (D20 item 2) and still open for the map and tileset files read by `sim`; review 3 P2-3 item 4 (D6) is resolved by the D6 amendment; review 3 P2-2 and the other items of P2-3, review 1 P2-3 and P2-5, review 2 P2-B, round 1 item 4, round 2 items 5 and 6, and M0 items 1, 2 (seen again in this round's smoke runs), 3, 5, 7, 8, and 9 are unchanged. Round 2 item 6 (every asset must be registered in `CheckedAssets`) was followed for the sprite sheets.

### Requires user verification

Not review findings (PLAN.md section 12 item 2). Run `cargo run --release -p game`, pan to the left of the rock ridge (the player's units start around cells (10, 28) and (14, 34), left of the map's centre and in its upper half on screen), and check:

1. **Unit rendering:** 18 blue-tinted units of the player (12 light tanks with a turret and barrel, 6 smaller wedge-shaped scouts) and, right of the ridge around cells (50, 30) and (52, 36), 8 red-tinted units of the opponent. Each unit stands on its tile (the hull centred on the cell's diamond), and units lower on the screen are drawn in front of units above them.
2. **Facings:** a moving unit turns to face its direction of travel in all eight directions (the barrel and the darker front of the hull point the way it moves), and keeps its facing when it stops.
3. **Smooth movement:** units glide between cells without jumps or jitter at any zoom.
4. **Click selection:** a left click on one of the player's units draws a green diamond under it; a click on empty ground clears the selection; the opponent's units cannot be selected.
5. **Box selection:** dragging with the left button draws a green rectangle and selects the player's units inside it when released.
6. **Additive selection:** with Shift held, clicks and boxes add units to the selection instead of replacing it.
7. **Orders:** a right click on a cell sends the selected units there; they spread over distinct cells around it without overlapping or getting stuck, route around the rock ridge through its gap and around the lake, and a click on rock or water sends them to the nearest cells around it. `S` stops the selection at the next cell centre.

The M1 items above (map rendering, picking, panning, zoom, bounds) remain pending for `m1-approved`.

## M2 remediation round 1 report (2026-10-04)

Implementer session for remediation round 1 of M2 review 1 (`docs/reviews/M2-review-1.md`, NO-GO: 0 P0, 2 P1, 4 P2), on branch `milestone/m2`, base `6d9c332`. No notes accompanied the findings. Every finding was treated as a claim and reconciled against the code. Reproductions used the reviewer's probe crate `simprobe` (copied to the scratchpad, `…/scratchpad/m2-remediation-1/`), built once against the reviewed `sim` (a `git archive` of `6d9c332`) and once against the fixed `sim`, and the reviewed and fixed `game` binaries on the reviewer's scratch asset roots (`BEVY_ASSET_ROOT`), all under a `perl alarm` timeout. "Fails before" evidence for the `sim` tests comes from that archive with the new test file copied in and one inert shim (the `retargeted` field of `MoveCompleted`, always `false`, so that the tests compile). The milestone table above is maintained by the orchestrator. Decisions: DECISIONS D21, with amendments to D19 items 3, 6, 7, and 9 and D20 item 4.

### Classes of defect

1. **A liveness guarantee measured by activity instead of progress (P1-1, with P2-2).** The guarantee "every order ends" was enforced by a counter that any movement reset, so units that kept moving without getting closer (taking turns stepping aside in a dead end one cell wide) defeated it. The counter also ran while a unit was only waiting for the pathfinder, so it reported `Blocked` for units that no unit blocked (P2-2). Every way an order can stay alive was enumerated: stepping along a path (now only counts if the path leads to the destination), stepping aside (never counts), taking a new path (counted, at most 64 without real progress), retargeting (does not reset the measure), terrain change (resets it, finitely often), and waiting for the pathfinder (not counted, bounded by the queue). D21 item 1 gives the resulting bound. (Corrected in M2 remediation round 2: the enumeration missed the time a unit spends inside its own step and waiting behind units that are moving on, both of which grow with 1/speed and were counted as stalled. See M2 review 2 P0-1 and DECISIONS D22.)
2. **A silent shortfall from a best-effort helper (P1-2).** `destination_cells` returns *up to* `count` cells; match setup used the result without checking it, and the smoke test checked only for more than zero units. The other caller, `apply_move`, already reports each unit without a cell (`MoveFailed { NoDestination }`). Match setup now fails with a typed error, and the smoke test requires the configured total. The game's other silent `continue`s were reviewed: `spawn_unit_views` skips a unit whose type has no sprite sheet, which cannot happen because `load_units` returns a checked sheet for every definition (left unchanged); the rest are documented behaviour (a right click outside the map orders nothing; a view whose unit no longer exists is removed).
3. **Unordered systems sharing state within a frame (P2-3).** The test added for it checks the whole class for the game's `PostUpdate`: Bevy's recorded list of conflicting, unordered system pairs, which held exactly the reported pair. The game's `Update` and `Last` schedules contain only writer-writer pairs on independent data (two systems that write `AppExit` messages, which Bevy reduces to the first error regardless of order; and the smoke test's `order_move` and `watch_events`, which write different fields of `SmokeTest`), so no reader can see a stale value there.

### Findings

| Finding | Outcome | Evidence | Change | Regression test (fails before, passes after) |
|---------|---------|----------|--------|-----------------------------------------------|
| P1-1 move orders can livelock forever | **Confirmed** | `simprobe repro 0` against the reviewed `sim`: after 20,000 ticks both orders live with 1,702 repaths each, no end event. `simprobe livelock 300`: seeds 62 and 154 never settled (as reported); `livelock 1000`: 7 never settled (62, 154, 524, 754, 818, 909, 928). A new probe, `deadend`, reproduced it without a sealed room: a dead end one cell wide closed by three idle units, 2 of 3 placements livelock. | D21 item 1: `MoveOrder` gains `remaining`, `best`, and `paths_since_progress`; `MAX_STALLED_TICKS` counts ticks without a step along a path to the destination (step-asides excluded), and the new `MAX_PATHS_WITHOUT_PROGRESS` = 64 ends an order that takes 65 paths without lowering `best` (`crates/sim/src/{movement,pathfinding,unit}.rs`). After the fix: repro ends at ticks 775/791, dead ends at 753–791, `livelock 1000` has 0 unsettled (slowest 1,032 ticks after the last command); I6 held on every tick of every probe. | `m2_movement`: `an_order_ends_when_two_units_keep_giving_way_to_each_other_in_a_sealed_room` (the reviewer's room); `an_order_ends_when_units_keep_giving_way_in_a_dead_end_closed_by_idle_units` (three placements); `every_order_ends_on_random_maps_under_random_group_orders` (the reviewer's generator, seeds 0–47 and the 7 livelock seeds, I6 every tick, every order ends within 3,000 ticks; about 23 s in a debug build). All three fail against the reviewed code ("orders still live after 3000 ticks", about 255 repaths each; seed 62 for the random test). |
| P1-2 match setup silently spawns fewer starting units | **Confirmed** | Reviewed release binary, `--smoke-test 150`: the arena fixture as demo map logs `12 units spawned` and exits 0; the 16×16 fixture logs `0 units spawned` and exits 1 only through the "no unit was spawned" check. | D21 item 5: `game::units::plan_starting_units` plans every group's cells before spawning; a shortfall is a `StartError` (owner, unit type, anchor, cells found and required), logged, exit code 1. The smoke test requires `starting_unit_total()` units. After the fix both scratch roots exit 1 in both profiles with, for example, `starting units of player 0: 6 "scout" units are placed around cell (14, 34), but only 0 free cells are reachable there (the cell is outside the map or walled in)`; the normal (non-smoke) release run on the arena root also exits 1 instead of running on. | `game` unit tests `units::tests::starting_units_that_do_not_fit_are_a_setup_error` (anchor outside the map, the exact message, a three-cell pocket with 3 required (accepted) and 4 required (rejected), cells of an earlier group not reused), `units::tests::the_demo_map_holds_every_starting_unit_on_its_own_free_cell`, `smoke::tests::a_run_passes_only_with_every_configured_starting_unit` (0, 12, 25, and 27 of 26 fail), `smoke::tests::the_demo_match_expects_all_of_its_starting_units`. They name new functions, so against the reviewed code they do not compile; the failing behaviour before the fix is the binary evidence in this row. |
| P2-1 `MoveCompleted` for retargeted orders | **Confirmed, fixed** | `simprobe repro 1` against the reviewed code: B ends in (7, 5), farther from its target than its start, with `MoveCompleted { unit: SimId(1) }` at tick 24. | D21 item 4: `MoveCompleted { unit, retargeted }`. The game's event log says "stopped near its destination" for a retargeted completion. | `a_completion_says_whether_the_unit_stopped_short_of_its_destination` (A `retargeted: false`, B `retargeted: true`); `a_destination_held_by_an_idle_unit_is_replaced_by_the_nearest_reachable_cell` now asserts `retargeted: true`. Both fail against the reviewed code with the inert shim. |
| P2-2 large group orders end with `Blocked` without moving | **Confirmed; fixed for the pathfinder queue, the rest recorded as debt** | `simprobe budget` against the reviewed code (square blocks of walkers across an open 256×256 map, 3,000 ticks, release): 400 units, 52 `Blocked`, all without moving; 1,024, 831; 4,000, 3,950. | D21 item 2: ticks spent waiting for the pathfinder do not count toward `MAX_STALLED_TICKS`. After: 400 units, 0 `Blocked` (completions 270 → 284); 1,024, 45 (all without moving: they waited inside the crowd behind units that advance; completions 193 → 301); 4,000, 0 (3,245 still waiting for a path after 3,000 ticks). 50 and 200 units: 0 and 0 `Blocked` (before: 0 and 1). | `ticks_spent_waiting_for_the_pathfinder_do_not_count_as_stalled` (50 units; queued units keep `stalled_ticks` 0 for three ticks); fails against the reviewed code (`stalled_ticks: 1` at tick 1). |
| P2-3 a right click can use the previous frame's hovered cell | **Confirmed, fixed** | `issue_orders` and `update_hovered_cell` were unordered in `PostUpdate`; Bevy's schedule build lists them as the only conflicting pair. | `issue_orders` (with its chain) runs `.after(map::update_hovered_cell)`, which is now `pub(crate)`. | `selection::tests::game_systems_that_share_state_in_a_frame_are_ordered` builds `PostUpdate` from the map, units, and selection plugins and asserts no conflicting unordered pair; before the fix it failed with `[("update_hovered_cell", "issue_orders")]`. |
| P2-4 D19 item 3 states a validation order the code does not follow | **Confirmed (documentation), fixed** | `command.rs:182-195` checks repetition, existence, and ownership per id in one pass; D19 item 3 listed "no unit twice" as a separate earlier check. | D19 item 3 now describes the per-id order; the code is unchanged. | `validation_checks_each_named_unit_in_turn` (`[unknown, Y, Y]` → `UnknownUnit`, `[Y, Y, unknown]` → `DuplicateUnit`, `[foreign, unknown]` → `NotOwned`, units before the target). It passes before and after: the defect was in the documentation, and the test pins the documented order. |

### Tests changed (none removed, skipped, or weakened)

`SimEvent::MoveCompleted` gained a field, so every existing use in `m2_movement.rs` was updated: the seven assertions that a unit arrived at its ordered cell now also require `retargeted: false` (stricter), the one in `a_destination_held_by_an_idle_unit_is_replaced_by_the_nearest_reachable_cell` requires `retargeted: true` (stricter), and the three `match` patterns that accept any completion use `MoveCompleted { unit, .. }` (unchanged meaning). No other existing test changed; the 26 previous `m2_movement` tests pass with the same reported arrival times (540, 558, 651, 945 ticks; 45, 39, 43, 39 of 50 at the assigned cells; 4 crossings, 199 completed orders).

### Files changed

| File | Change |
|------|--------|
| `crates/sim/src/unit.rs` | `MoveOrder::{remaining, best, paths_since_progress}`; `stalled_ticks` redefined; hash layout extended (D21 item 3) |
| `crates/sim/src/movement.rs` | Progress-based end rule, `MAX_PATHS_WITHOUT_PROGRESS`, `start_step`, terrain-change reset, `MoveCompleted::retargeted`; module documentation with the bound |
| `crates/sim/src/pathfinding.rs` | Records each served path's remaining cost and counts paths without progress |
| `crates/sim/src/event.rs`, `crates/sim/src/lib.rs` | `MoveCompleted { unit, retargeted }`, `Blocked` documentation; export of `MAX_PATHS_WITHOUT_PROGRESS` |
| `crates/sim/tests/m2_movement.rs` | 6 new tests (above); `MoveCompleted` uses updated |
| `crates/game/src/units.rs` | `StartingGroup`, `starting_unit_total`, `StartError`, `plan_starting_units`, setup error; retargeted completions logged as such; 2 new tests |
| `crates/game/src/smoke.rs` | `SmokeOutcome`, `smoke_failures` (requires the configured total); 2 new tests |
| `crates/game/src/selection.rs`, `crates/game/src/map.rs` | `issue_orders` after `update_hovered_cell`; 1 new test |
| `docs/DECISIONS.md`, `docs/STATUS.md` | D21; amendments to D19 and D20; this report |

### Validation commands (PLAN.md section 9, with section 12 item 5)

Run from the repository root on macOS 27.0.1 (Apple M5, Rust 1.98.1), in this order, in the foreground, after the last code change. A display was available.

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 181 passed, 0 failed, 0 ignored: `game` unit 44 (cli 7, projection 7, camera 7, map 6, checked_assets 2, units 8, selection 5, smoke 2), `m0_spike` 6, `m1_demo_map` 3, `m1_tileset_image` 6, `m2_unit_sprites` 5, `sim` unit 3, `m0_bootstrap` 12, `m1_map` 32, `m1_map_subset` 24, `m2_definitions` 14, `m2_movement` 32 (27.9 s), doc-tests 0. Peak memory footprint of the whole run 0.13 GB. Known M0 linker warning only. |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | `26 units spawned`; `smoke test: 600 frames, 198 simulation ticks, map created: true, units spawned: 26, units moved: 18 in 198 ticks after the order` |
| `cargo run -p game -- --smoke-test 600` | 0 | `smoke test: 600 frames, 203 simulation ticks, map created: true, units spawned: 26, units moved: 18 in 203 ticks after the order` |

Additionally: `simprobe hash` (60 units, 3 seeds, 4,000 ticks of random orders) gives identical per-tick hash digests in debug and release builds of the fixed code (I3, I12).

### Acceptance criteria and required tests (PLAN.md section 7, M2)

Unchanged from the M2 implementation report above, with these updates:

| Criterion | Evidence |
|-----------|----------|
| Acceptance: 50 units move as a group to a clicked destination without overlap or deadlock in the fixture scenarios | `fifty_units_reach_a_clicked_destination_without_overlap_or_deadlock` passes unchanged with the same times; on screen **requires user verification** |
| Test: an unreachable target ends the order with an event and no panic; and every order ends (D19 item 6, now D21 item 1) | `unreachable_target_ends_the_order_with_an_event_and_no_panic`, plus the three P1-1 termination tests above |
| Test: I6 on every tick while 50 units cross each other for 2,000 ticks | `invariant_i6_holds_while_fifty_units_cross_each_other_for_2000_ticks` (unchanged, 199 orders completed); I6 is also checked on every tick of the new random test (55 scenarios) and of the probes (1,000 scenarios) |
| Test: determinism of the same scenario | `the_same_scenario_gives_the_same_hash_at_every_tick` (unchanged); debug and release digests identical |
| Scope: spawning (starting units) | All 26 configured starting units are planned before any is spawned, or the game exits 1 (P1-2 row) |

### Assumptions verified and how

| Item | Verification |
|------|--------------|
| Bevy 0.19.1 records conflicting unordered system pairs whatever the ambiguity log level, and `Schedule::systems` gives each system's `system_type` after initialization | Source: `bevy_ecs-0.19.1/src/schedule/schedule.rs:1246-1251` (`get_conflicting_systems` always runs), `:693` (`systems`), `system/system.rs:58` (`system_type`); system names need Bevy's `debug` feature, so the test identifies systems by type. Exercised by the new selection test, which failed before the fix with exactly the reported pair. |
| `App::should_exit` prefers an error among several `AppExit` messages | Relied on only for the statement in "Classes of defect" item 3 that the writer-writer `AppExit` pairs in `Last` are harmless; behaviour unchanged by this round. |

### Deviations from PLAN.md

None new. The M2 deviations above still apply; deviation 3's list of events now reads `MoveCompleted { unit, retargeted }`.

### Known debt (P2)

1. **P2-2 remainder (from review 1):** units deep inside a large crowd (about 1,000 units in one command) can still end with `MoveFailed { Blocked }` without having moved, because waiting behind units that advance counts as stalled (45 of 1,024 in `simprobe budget`); and path throughput (16 requests per tick) makes a 4,000-unit order take thousands of ticks. Excluding waits behind advancing units would make the end of an order depend on other units' movement, which can continue indefinitely, so it was not done; the request budget and crowd behaviour are to be measured against the M7 benchmark of 200 units (D21 item 2).
2. Units that take turns giving way to each other now end their orders after about 800 ticks (40 s of game time) rather than never; a unit whose legitimate route around other units needs more than 64 new paths without getting closer than before, or that takes no step toward its destination for 200 counted ticks, also ends with `Blocked`. In the fixture scenarios and probes this produced fewer `Blocked` events than the reviewed code (D21 item 1).
3. The M2 implementation debt items 1 to 6 above are unchanged, as confirmed by review 1 (item 2: the `MessageReader` window of two ticks also affects the new "stopped near its destination" log line).
4. Carried from M1 and M0, unchanged: as listed in review 1's "Recorded debt" (M1 review 3 P2-1 for the map and `.tsx` reader in `sim`, P2-2, and the remaining items of P2-3; review 1 P2-3 and P2-5; review 2 P2-B; round 1 item 4; round 2 items 5 and 6; M0 items 1, 2 (the `Skipped event Destroyed` warning was seen again in this round's smoke runs), 3, 5, 7, 8, and 9).

### Requires user verification

Unchanged: the seven M2 items in the M2 implementation report above and the M1 items, as listed in review 1's "Requires user verification". These are not findings and block only `m2-approved` (and `m1-approved`).

## M2 remediation round 2 report (2026-10-04)

This is the implementer session for remediation round 2 of M2 review 2 (`docs/reviews/M2-review-2.md`, NO-GO: 1 P0, 0 P1, 0 P2).

* **Branch and base:** `milestone/m2`, base `4fa123b`.
* **Notes:** none accompanied the findings. The finding and the review's debt notes were treated as claims and reconciled against the code.
* **Probes:** reproductions and measurements used a probe crate outside the repository (`…/scratchpad/m2-remediation-2/probe`), with unit types of speeds 1, 2, 3, 7, 24, 32, 100, 127, and 128. It was built once against the fixed `sim` and once against a `git archive` of the reviewed commit `4fa123b` (`probe_base`). Every probe ran under a `perl alarm` timeout. The largest measured peak resident size was 0.02 GB (the 1,024-unit crowd).
* **"Fails before" evidence:** the new test file and fixture were copied into that archive and run there.
* **Decisions:** DECISIONS D22, with amendments to D19 items 6 and 9 and D21 item 1. The milestone table above is maintained by the orchestrator.

### Class of defect

**Tick limits applied to waits whose length depends on unit speed.**

The units file admits speeds 1 to 128 subcells per tick, so a step lasts from 2 to 362 ticks. The rules that end a move order measured elapsed ticks. Some of those ticks grow with 1/speed:

* the unit's own steps;
* waits behind a unit in a step, or behind a queue led by one;
* the paths a waiting unit asks for every 8 ticks.

So the same situation ended or continued depending on speed. Every rule that ends an order, or counts toward its end, was checked against the whole speed range. D22 item 1 lists them:

* three were defective and are fixed;
* the others were checked and are unaffected: the repath interval, the path request budget, the movement arithmetic at nine speeds, and the smoke test's movement check.

The round 1 report's claim that "every way an order can stay alive was enumerated" is corrected in place.

### Findings

#### P0-1: a unit of speed 1 fails every move longer than one cell with `Blocked` (regression of round 1)

**Outcome: confirmed. The class is fixed.**

**Evidence:**

* **Lone unit.** `probe_base lone` (one unit alone on the open 16×16 map; nine speeds × 12 orders) gives 10 of 108 orders wrong, all at speed 1. Examples:
  * (1, 1) → (3, 1): `MoveFailed { Blocked }` at tick 256, where completion at tick 512 is expected.
  * (1, 1) → (4, 4): `Blocked` at tick 362, where 1,086 is expected.

  This is exactly the review's table.
* **Cause.** `movement.rs` counted every tick without a step start as stalled, including the 256 or 362 ticks of the unit's own step. At its next centre, with `stalled_ticks` ≥ 200, the order ended.
* **Further members of the class, reproduced on the reviewed code:**
  * Followers in a one-cell corridor:
    * behind a leader of speed 1, all six of a convoy of six end as `Blocked`;
    * followers of speed 3 behind any leader: 2 of 6 end as `Blocked`.
  * A follower on the open diagonal behind a speed-1 leader: 6 of 12 scenarios have `Blocked`.
  * 50-unit groups at speeds 1 to 7: 7 to 50 `Blocked` per group.
  * Mixed groups of speeds 1 and 128: 41 and 43 `Blocked`.
* **Third member.** With the stalled-tick rule fixed alone (an intermediate build), the three rearmost units of a speed-1 convoy still ended as `Blocked`, with `paths_since_progress` 64 and `best` still `u32::MAX`. They had asked for a path every 8 ticks for 512 ticks before they could take their first step.

**Change** (D22, in `crates/sim/src/{movement,pathfinding,unit,event}.rs`):

1. A stalled tick is now only a tick in which the unit waits at a cell centre for cells held by units that are not moving on.
   * "Moving on" means:
     * the unit is in a step; or
     * it waits for a cell that is free, or held by a unit that is moving on in turn (the chain of next cells is followed to the head of the queue).
   * These are not moving on:
     * a wait-for cycle;
     * a chain that ends at an idle unit or at an impassable cell.
   * These are not stalled ticks:
     * the unit's own steps;
     * waits for the pathfinder;
     * waits behind moving units.
2. A path request made during such a wait is marked `queued`. A path served for it that only confirms the current way does not count toward `MAX_PATHS_WITHOUT_PROGRESS`.

Movement itself is unchanged. Only the decisions that end an order differ. Every probe scenario whose units all have speeds of 24 or more gives the reviewed code's arrival times.

**After the fix (release):**

* Lone units: 0 of 108 wrong. Arrival is exactly at `ceil(length / speed)` at all nine speeds.
* Convoys, diagonal follow, head-on passages, and single-speed and mixed groups (124 scenarios): 0 `Blocked`.
* The reviewer's random-map generator:
  * seeds 0–999: 0 unsettled; slowest settle 1,033 → 996 ticks; `Blocked` 398 → 116;
  * seeds 0–199, with six speed mixes from 1 to 128 and a 400,000-tick limit: every order ends.
* The D21 livelocks: they end at speed 32 at 778–816 ticks (previously 778–792) and at speed 1 at 17,146–17,928 ticks (previously both orders failed after the first step).
* The real binaries (debug and release, `--smoke-test 1300`):
  * on the reviewer's speed-1 asset root: no `unit N stopped` line (the review saw 18), exit 0;
  * on the speed-128 root: also no such line, exit 0.
* Per-tick hash digests of the probe's random-order scenario are identical in debug and release (`f3748c5ee280f84c`).

**Regression tests.** All are in `crates/sim/tests/m2_movement.rs` and use the new fixture `tests/fixtures/data/units_boundary_speeds.ron` (`crawler`, speed 1; `sprinter`, speed `MAX_UNIT_SPEED` = 128). All pass after the fix:

* `a_unit_of_either_boundary_speed_crosses_an_open_map_and_arrives_on_time`: 9 orders × 2 speeds, orthogonal, diagonal, and mixed.
  * The unit completes with `retargeted: false` exactly at `ceil(length / speed)`, with `stalled_ticks` 0 on every tick.
  * Along the straight path it moves exactly `speed` subcells per tick.
  * **Before the fix:** it fails, `stalled_ticks` 1 at tick 2 for the crawler; the probe shows the `Blocked` at tick 256.
* `units_queued_behind_a_unit_of_the_slowest_speed_follow_it_to_their_destinations`:
  * The scenarios:
    * a corridor convoy of six, led by a crawler, with crawler followers;
    * the same convoy with sprinter followers;
    * a sprinter behind a crawler on the open diagonal.
  * Every unit completes at its own cell. `stalled_ticks` stays 0 and `paths_since_progress` stays ≤ 1 on every tick.
  * **Before the fix:** it fails at tick 1 (`stalled_ticks` 1 for the first follower). With the stalled-tick rule fixed alone, the probe's same convoy ended three units as `Blocked` with 64 paths, which both the test's path assertion and its end-event check reject.
* `orders_end_at_both_boundary_speeds_when_units_keep_giving_way`: the sealed room and the three dead ends at speeds 1 and 128.
  * Every order ends, and in the sealed room exactly one ends as `Blocked`.
  * **Before the fix:** it fails, because both crawlers end as `Blocked` at ticks 228 and 256.
* `fifty_units_of_the_boundary_speeds_reach_a_clicked_destination`: the acceptance scenario with crawlers and sprinters taking turns, on the arena and the wall map. Every unit arrives (13,559 and 14,695 ticks), with 44 and 43 units at their assigned cells.
  * **Before the fix:** it fails, `unexpected MoveFailed { unit: SimId(7), reason: Blocked }`.
* `the_boundary_speed_fixture_holds_the_slowest_and_fastest_speeds`: pins the fixture to speeds 1 and `MAX_UNIT_SPEED`. It passes before and after.

Against the reviewed code, the four behavioural tests fail and the other 33 `m2_movement` tests pass.

#### Review 2 debt notes

* **"A unit following a slower unit through a one-cell corridor waits behind an advancing unit, and that wait counts as stalled"** (part of round 1 debt item 1). **Outcome: confirmed and resolved** by the same change: waits behind units that are moving on are no longer stalled ticks. The convoy test above covers it.
* **"Orders that cannot get closer end after about 800 ticks"** (round 1 debt item 2). **Outcome: confirmed for speed 32** (778–816 ticks). At other speeds the number of turns is the same and the time scales with the step time (see Known debt).
* **The review's other carried debt items.** Outside this finding; carried unchanged (Known debt below).

### Tests changed (none removed, skipped, or weakened)

* `m2_movement.rs` gains 5 tests and their helpers (`boundary_scenario`, `termination_limit`, `run_until_each_completes`) and one new fixture file.
* The only changed existing line is the `use sim::{…}` list, which adds `MAX_UNIT_SPEED`.
* No assertion of an existing test changed. The existing 32 tests pass with the same reported numbers:
  * the acceptance scenario: 540, 558, 651, and 945 ticks; 45, 39, 43, and 39 units at their assigned cells;
  * the termination tests pass within `TERMINATION_LIMIT`.
* The existing fixture `tests/fixtures/data/units.ron` is unchanged, so `m2_definitions` is unaffected.

### Files changed

| File | Change |
|------|--------|
| `crates/sim/src/movement.rs` | Stalled ticks count only waits for units that are not moving on (`Activity`, `moving_on`, `AtCentre::Blocked`); `queued` requests; module and constant documentation with the re-derived bound |
| `crates/sim/src/pathfinding.rs` | A path served for a `queued` request that equals the current path does not count toward `MAX_PATHS_WITHOUT_PROGRESS`; documentation |
| `crates/sim/src/unit.rs` | `PathState::Requested { since, avoid_units, queued }`; hash layout (`queued` byte); `stalled_ticks` documentation |
| `crates/sim/src/event.rs` | `MoveFailure::Blocked` documentation |
| `crates/sim/tests/m2_movement.rs` | 5 new tests (above) |
| `crates/sim/tests/fixtures/data/units_boundary_speeds.ron` | New fixture: unit types of speed 1 and 128 |
| `docs/DECISIONS.md`, `docs/STATUS.md` | D22; amendments to D19 items 6 and 9 and to D21 item 1; correction note in the round 1 report; this report |

### Validation commands (PLAN.md section 9, with section 12 item 5)

The commands ran from the repository root on macOS 27.0.1 (Apple M5, Rust 1.98.1), in this order, in the foreground, after the last code change. A display was available.

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 186 passed, 0 failed, 0 ignored (breakdown below). Peak memory footprint of the whole run 0.13 GB. Known M0 linker warning only. |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | `26 units spawned`; `smoke test: 600 frames, 198 simulation ticks, map created: true, units spawned: 26, units moved: 18 in 198 ticks after the order` |
| `cargo run -p game -- --smoke-test 600` | 0 | `smoke test: 600 frames, 203 simulation ticks, map created: true, units spawned: 26, units moved: 18 in 203 ticks after the order` |

Test breakdown for `cargo test --workspace`:

* `game` unit 44.
* `m0_spike` 6, `m1_demo_map` 3, `m1_tileset_image` 6, `m2_unit_sprites` 5.
* `sim` unit 3.
* `m0_bootstrap` 12, `m1_map` 32, `m1_map_subset` 24, `m2_definitions` 14, `m2_movement` 37 (31.5 s).
* doc-tests 0.

The `Skipped event Destroyed` warning (M0 debt item 2) appeared in both smoke runs.

### Acceptance criteria and required tests (PLAN.md section 7, M2)

Unchanged from the M2 implementation report and the round 1 report above, with these updates:

| Criterion | Evidence |
|-----------|----------|
| Acceptance: 50 units move as a group to a clicked destination without overlap or deadlock in the fixture scenarios | `fifty_units_reach_a_clicked_destination_without_overlap_or_deadlock` passes unchanged, with the same times. It now also holds at the boundary speeds (`fifty_units_of_the_boundary_speeds_reach_a_clicked_destination`). On screen: **requires user verification** |
| Test: an unreachable target ends the order with an event and no panic; and every order ends (D21, D22) | `unreachable_target_ends_the_order_with_an_event_and_no_panic`, the three D21 termination tests, and `orders_end_at_both_boundary_speeds_when_units_keep_giving_way` |
| Test: I6 on every tick while 50 units cross each other for 2,000 ticks | `invariant_i6_holds_while_fifty_units_cross_each_other_for_2000_ticks` (unchanged). I6 is also checked on every tick of the five new tests and of every probe scenario |
| Test: determinism of the same scenario | `the_same_scenario_gives_the_same_hash_at_every_tick` (unchanged). The probe's per-tick digests are identical in debug and release |
| Section 8 boundary category: zero and maximum quantities | Movement at speed 1 and `MAX_UNIT_SPEED` = 128: four behavioural tests, plus the fixture test |

### Assumptions verified and how

No new crate API is used. The activity map and the chain walk use `std::collections::{BTreeMap, BTreeSet}`, which iterate in key order, and units are processed in `SimId` order, as before (I4). Determinism was checked by identical per-tick hash digests in debug and release builds (I3, I12).

### Deviations from PLAN.md

None new. The fix differs from the review's three suggested options. D22 item 7 gives the measurements behind that choice.

### Known debt (P2)

1. **Round 1 debt item 1, narrowed.** Waits behind units that are moving on no longer count as stalled. What remains is path throughput: 16 requests per tick, and a waiting unit still asks for a way around every 8 ticks, which at speed 1 is many requests per cell. This is to be measured against the M7 benchmark. In this round's crowd probe (open 256×256 map, 3,000 ticks, speed 32), the reviewed and the fixed code agree:
   * 50 units: 0 `Blocked`;
   * 200 units: 0 `Blocked`;
   * 400 units: 1 `Blocked`;
   * 1,024 units: 0 `Blocked`.
2. **Time to end a livelock.** Orders that cannot get closer end after the same number of turns at every speed. That is about 800 ticks at speed 32, and 17,000 to 18,000 ticks (about 15 minutes of game time) at speed 1, because a turn takes a step (D22 item 8).
3. **Cost of the chain walk.** A unit waiting at a centre follows the chain of units ahead of it, which costs O(queue length) per waiting unit per tick. With 1,024 units, the mean tick time went from 3.25 to 3.34 ms. To be measured against the M7 benchmark.
4. **No overtaking.** A unit right behind a slower unit in a step does not overtake it on open ground, because the pathfinder avoids only waiting units. This is behaviour of the reviewed code and is unchanged; it is conspicuous at speed 1.
5. **Waiting under continuous orders.** While new commands keep units moving through a unit's way, that unit waits and does not fail, because it is not blocked (D22 item 5).
6. **Weak smoke check.** The smoke test passes even if every ordered unit ends as `Blocked`, because it checks only that some unit moved (noted by review 2). The `sim` defect behind that observation is fixed, and the fixed binaries log no stopped unit at speed 1 or 128.
7. **Earlier M2 debt.** The M2 implementation debt items 1 to 6 are unchanged, as confirmed by reviews 1 and 2.
8. **Carried from M1 and M0, unchanged,** as listed in review 2's "Recorded debt":
   * M1 review 3: P2-1 for the map and `.tsx` reader in `sim`, P2-2, and the remaining items of P2-3;
   * M1 review 1: P2-3 and P2-5;
   * M1 review 2: P2-B;
   * M1 round 1: item 4;
   * M1 round 2: items 5 and 6;
   * M0: items 1, 2 (the `Skipped event Destroyed` warning was seen again in this round's smoke runs), 3, 5, 7, 8, and 9.

### Requires user verification

Unchanged: the seven M2 items and the M1 items, as listed in review 2's "Requires user verification". These are not findings and block only `m2-approved` (and `m1-approved`).

## M2 review decision (2026-10-04)

Three independent review rounds (`docs/reviews/M2-review-1.md`: NO-GO, 0 P0, 2 P1, 4 P2; `M2-review-2.md`: NO-GO, 1 P0; `M2-review-3.md`: GO WITH DOCUMENTED NON-BLOCKING DEBT, 0 P0, 0 P1, 2 P2). Before review 3 the orchestrator reran all six validation commands of PLAN.md section 9 on commit `1a165c8` with exit code 0 (186 tests passed). Under PLAN.md section 12 item 1, `milestone/m2` was merged into `main` and tagged `m2-reviewed`. The tag `m2-approved` is created by the user alone after the verification below.

### Debt recorded from review 3 (P2, verbatim from the report)

#### P2-1: The rule that ends an order still depends on unit speed in jams, because a unit waiting behind a slow unit in a step alternates its path every 8 ticks onto a way that counts as stalled. D22's claim that "the rules that end a move order do not depend on unit speed" is too strong.
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

#### P2-2: A box selection released outside the window silently becomes a click at the drag's start point

**Files and lines:** `crates/game/src/selection.rs:162`: `let end = window.cursor_position().unwrap_or(start);`.

**Evidence:**

* `bevy_window` 0.19.1 `Window::cursor_position` / `physical_cursor_position` (`window.rs:620-632`) documents "Returns `None` if the cursor is outside the window area".
* `bevy_winit` 0.19.1 `state.rs:304-306` sets the position to `None` on `CursorLeft`.
* A left drag released outside the window therefore yields `start.distance(end) = 0 < DRAG_THRESHOLD_PX`. It is handled as a click at the drag's start point: it selects the unit there, or clears the selection, and the box is discarded. Nothing is logged.
* No test covers it: the pure functions are tested, and the system wiring is M2 debt item 3. Whether macOS reports `CursorLeft` during a drag was not observed; that part is listed under "requires user verification".

**Expected behaviour:** the system keeps the last cursor position seen during the drag, or clamps it to the window, and uses it as the box's end corner. A test of the selection decision covers a release with no cursor position.

### Recorded debt to carry (verbatim from review 3)

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

### Requires user verification (blocks only `m2-approved`; verbatim from review 3)

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

## M3 implementation report (2026-10-04)

Branch `milestone/m3`, based on `main` at the merge of M2 (tag `m2-reviewed`). Commits: `0f9507d` (sim: players, production, buildings, harvesting, tests), `f460221` (game: data, sprites, buildings, interim controls, smoke test), and the documentation commit that adds this report. Decisions: DECISIONS "M3: economy and construction" (A1 follow-up, D23 to D27) and the M3 amendment of D6.

### Files changed

| File | Change |
|------|--------|
| `crates/sim/src/economy_defs.rs` (new) | Economy file schema 1: `BuildingDef`, `Production`, `HarvesterDef`, `BuildingKind`, `ProductionItem`, limits, subset schema, validation with references, uniqueness, and an acyclic prerequisite check (D23) |
| `crates/sim/src/definitions.rs` | `Definitions` holds the economy definitions; `load_definitions`, `Definitions::with_economy`, accessors (`buildings`, `building`, `building_kind`, `production`, `harvester`, `produces_units`, `placement_distance`); hash extended |
| `crates/sim/src/data.rs` | `DataErrorKind::UnknownReference`, `PrerequisiteCycle`; documentation of the deeper schema |
| `crates/sim/src/building.rs` (new) | `Footprint`, `Building`, `Producer`, `PlacementError`, footprint and distance checks, exit cells, `spawn_building` (setup), hash (D24) |
| `crates/sim/src/economy.rs` (new) | `Players`, `PlayerState`, `QueueItem`, `charge_for_tick` (equation 3), `update_power`, `advance_production` with unit and free-unit exits; unit test (D25) |
| `crates/sim/src/economy_commands.rs` (new) | Validation and application of `QueueProduction`, `CancelProduction`, `PlaceBuilding`, `SellBuilding`, `SetRallyPoint`; same-tick building index (D25) |
| `crates/sim/src/harvest.rs` (new) | `Harvester`, `HarvestState`, `MAX_HARVEST_ATTEMPTS`, the `harvest` system, ore and dock search (D26) |
| `crates/sim/src/command.rs` | New `Command` variants and `RejectReason`s; unit commands include `Harvest`; `Move`/`Stop` stop harvesting; economy commands dispatched in push order |
| `crates/sim/src/map.rs` | `Grid` building layer (`building`, `cover`, `uncover`), `is_passable` false under buildings, `take_ore`, hash bit 3 |
| `crates/sim/src/{event,hash,lib,pathfinding,schedule,unit}.rs` | New events and `HarvestIdle`; hash of harvesters, buildings, and players; exports; `nearest_cell`; `Economy` stage systems and `Players` resource; `unit_bundle`, harvesters spawn with their cycle, `spawn_unit` reports an unknown type instead of speed 0 |
| `crates/sim/tests/m3_definitions.rs`, `m3_economy.rs`, `m3_harvest.rs` (new) | 11, 21, and 10 tests (below) |
| `crates/sim/tests/fixtures/data/{economy,units_economy}.ron`, `fixtures/maps/economy_32x32.tmx` (new) | Fixture data and a 32×32 map with ore, sand, rock, and two sealed pockets |
| `crates/game/src/buildings.rs` (new) | Building sprite checks, starting players, credits, and construction yards, building views, title status; 6 tests |
| `crates/game/src/construction.rs` (new) | Interim keys for production, placement, selling, and rally points; placement outline; 4 tests |
| `crates/game/src/{units,selection,smoke,main}.rs` | Match setup with the economy file; harvest by right click on ore (2 tests); smoke test checks buildings and charged production (2 tests); plugins |
| `crates/game/examples/gen_building_sprites.rs` (new), `gen_unit_sprites.rs` | Building sprite generator; harvester sheet |
| `crates/game/tests/m3_building_sprites.rs` (new) | 3 tests: generator output, PNG subset, layout |
| `assets/data/economy.ron` (new), `assets/data/units.ron` | Production economy definitions; harvester unit type (units schema unchanged, version 1) |
| `assets/sprites/buildings/*.png`, `assets/sprites/units/harvester.png` (new), `assets/LICENSES.md` | Generated placeholders and their provenance |
| `docs/DECISIONS.md`, `docs/STATUS.md` | M3 decisions, D6 amendment; this report |

### Validation commands (PLAN.md section 9, with section 12 item 5)

Run from the repository root on macOS 27.0.1 (Apple M5, Rust 1.98.1), in this order, in the foreground, after the last code change (`f460221`). A display was available.

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 248 passed, 0 failed, 0 ignored: `game` unit 58 (cli 7, projection 7, camera 7, map 6, checked_assets 2, units 8, selection 7, smoke 4, buildings 6, construction 4), `m0_spike` 6, `m1_demo_map` 3, `m1_tileset_image` 6, `m2_unit_sprites` 5, `m3_building_sprites` 3, `sim` unit 6, `m0_bootstrap` 12, `m1_map` 32, `m1_map_subset` 24, `m2_definitions` 14, `m2_movement` 37 (36.8 s), `m3_definitions` 11, `m3_economy` 21, `m3_harvest` 10, doc-tests 0. Peak memory footprint of the run 0.13 GB. Known M0 linker warning only. |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | `2 buildings spawned`, `26 units spawned`; `smoke test: 600 frames, 198 simulation ticks, map created: true, units spawned: 26, units moved: 18 in 198 ticks after the order, buildings spawned: 2, production charged 396 credits in 198 ticks` |
| `cargo run -p game -- --smoke-test 600` | 0 | `smoke test: 600 frames, 203 simulation ticks, ..., buildings spawned: 2, production charged 406 credits in 203 ticks` |

Additionally: `cargo test --release -p sim --test m3_economy --test m3_harvest --test m3_definitions` passed (21, 10, 11). The release binary, run (`--smoke-test 100`, under a 60 s alarm) against scratch asset roots, exited 1 with typed errors for an economy file with a prerequisite cycle (`data file …/economy.ron: buildings[2].prerequisites: the prerequisites form a cycle: power_plant -> war_factory -> refinery -> power_plant`) and for a missing building sprite (`data file …/economy.ron: buildings[3].sprite: sprite image …/barracks.png: file: cannot be read: …`). The `Skipped event Destroyed` warning (M0 debt item 2) appeared in both smoke runs.

### Acceptance criteria and required tests (PLAN.md section 7, M3)

| Criterion | Evidence |
|-----------|----------|
| Scope: players, credits, power | `Players`, `PlayerState`, `update_power` (D25); `power_follows_the_buildings_placed_and_sold`, `low_power_halves_progress` (boundary: consumption equal to production is not low) |
| Scope: construction yard; building and unit production queues | Production per producing building type, head only (D25); `a_completed_item_is_charged_exactly_its_cost`, `every_item_of_the_fixture_is_charged_exactly_its_cost` (all 10 producible fixture items), `production_needs_a_producer_and_continues_with_another`, `invalid_queue_commands_are_rejected_without_changing_state` (unknown items, missing prerequisite, no producer, unknown player, a full queue of 16) |
| Scope: placement rules (buildable terrain, free footprint, within a data defined distance of an owned building) | D24; tests below |
| Scope: ore, the harvester cycle, refinery | D26; `m3_harvest` (below), `harvesters_share_the_field_and_every_unit_of_ore_is_paid_once` (four harvesters, 3,000 ticks: distinct targets, ore removed = ore carried + ore paid, credits = payments) |
| Scope: rally points | `a_produced_unit_leaves_its_building_and_moves_to_the_rally_point`, `rally_point_commands_are_validated`, `later_commands_of_a_tick_see_the_buildings_placed_before_them` (rally on a building placed in the same tick) |
| Scope: selling | `selling_refunds_the_sell_value_and_frees_the_cells` (refund, cells freed and walkable, foreign, unknown, and a unit's id rejected), `later_commands_of_a_tick_see_the_buildings_sold_before_them` |
| Test: I7 for completion | `a_completed_item_is_charged_exactly_its_cost` (charges 10 × 9 + 15 = 105; build time 1; cost below the build time; cost 0), `every_item_of_the_fixture_is_charged_exactly_its_cost`, unit test `the_charges_of_an_item_sum_to_its_cost_at_the_limits` (cost 0 to 10^6, build time 1 to 72,000); every M3 test asserts credits ≥ 0 after every tick |
| Test: I7 pause on insufficient credits | `progress_pauses_while_credits_are_insufficient` (100 credits: paused at 9 of 10 ticks with 10 credits for 100 ticks, resumed by a sale; 0 credits: no progress; exactly 105 credits: completes at 0) |
| Test: I7 cancel refund | `cancelling_refunds_exactly_what_was_charged` (in progress: 50; not started: 0; complete: 105; not queued: rejected with no state change) |
| Test: low power halves progress | `low_power_halves_progress` (10 ticks normally; under low power progress and charges only on even ticks: 19 ticks from an even start, 20 from an odd one; still charged exactly 105) |
| Test: placement rejections for overlap, off map, non buildable terrain, and excessive distance | `placement_rejections_change_no_state` (12 cases: beyond each edge, `i32` extremes, sand, ore, rock, overlap, 4 cells away; state hash equal to a control run for 10 ticks); `placement_is_rejected_on_cells_held_by_units` (standing and entering); `placement_needs_a_ready_building_and_an_owned_building_nearby` (not ready, other type, unknown type, no own building, another player's building); boundary accepted: `placement_within_the_distance_is_accepted_and_covers_the_cells` (exactly 3 cells, diagonally) |
| Test: a harvester with no reachable ore idles | `a_harvester_with_no_reachable_ore_idles` (sealed in rock, and on open ground with the only ore sealed: `HarvesterIdle { NoOre }` on the first tick, then 500 ticks without any event; a `Harvest` command idles it again; a loaded harvester without refinery: `NoRefinery`, load kept) |
| Test: a refinery destroyed during the return trip causes a retarget or idle state without panic | `a_refinery_sold_during_the_return_trip_makes_the_harvester_retarget` (turns to the other refinery and delivers there) and `…_with_no_other_leaves_the_harvester_idle` (idle, load kept, completes its step, resumes by `Harvest` after a new refinery). Buildings are removed by selling in M3; destruction arrives with combat in M4, and the harvester logic checks only that the refinery no longer exists. |
| Test: an ore cell depletes exactly to zero | `an_ore_cell_depletes_exactly_to_zero` (22 units, rate 3, capacity 20: loads 3…18, 20 (room-limited), cell 19…4, 2; then the last 2 (cell-limited), cell exactly 0, deliveries 20 and 2, credits 44, harvester idles `NoOre`); `harvest_rate_equal_to_the_capacity_fills_in_one_tick` |
| Test: integration: a scripted command sequence builds a power plant, a refinery, and a barracks, and income becomes positive within a fixed tick budget | `a_base_is_built_and_funded_from_ore_through_commands_alone`: starting credits pay exactly for the power plant and the refinery, so the barracks is paid with ore; within 3,000 ticks all three are placed, ore is delivered after the barracks, final credits exceed those at its placement, and the ledger balances (final = 305 − 455 + payments); the run is repeated with an identical hash at every tick |
| Acceptance: a base can be built and funded from ore through commands alone | The integration test above (through `SimPlugin` and `CommandQueue` only, after setup of the yard). In the game the same is possible with the interim keys; on screen **requires user verification** |
| Section 8: negative, boundary, integration, end to end | Negative: every rejection reason with no state change; data: `m3_definitions` (every field required, unknown and duplicate fields, wrong kinds, schema versions, every numeric field at its minimum and maximum and beyond including `i64` extremes, list limits, references, names, duplicates, cycles of 1 to 3, files, truncations, 2,000 seeded mutations, all bounded as D15); boundary: as listed; integration: all `m3_economy` and `m3_harvest` tests; end to end: both smoke runs |
| Invariants | I3: `the_same_economy_scenario_gives_the_same_hash_at_every_tick`, the integration test's repeated trace, `the_hash_covers_the_economy_state`, `the_hash_covers_the_harvester_cycle`; I6: every M3 test checks after every tick that no unit holds a building cell and the grid's building layer matches the footprints (`m3_harvest` also checks the unit occupancy); I7 above; I8/I11: the economy file passes the RON subset before `ron`, sprites pass the PNG subset before the renderer; I12: D23 item 5, saturating or checked operations in every new computation, footprints in `i64` (`footprints_at_the_extremes_of_i32_do_not_overflow`) |

### Assumptions verified and how

| Item | Verification |
|------|--------------|
| Bevy 0.19.1 applies deferred `Commands` at automatic sync points between ordered systems, and those sync points do not count as ambiguities | Source (DECISIONS A1 follow-up of M3); observed by the tests that see a building placed in the `Commands` stage in the `Economy` stage of the same tick, and by `stages_run_once_per_tick_in_plan_order` (unchanged, passes) |
| Exclusive systems conflict with every unordered system | Source (`schedule/node.rs:608-610`); the reason the M3 systems are ordinary systems |
| `#[derive(SystemParam)]`, `EntityCommands::id`, `Window::title`, `ButtonInput::any_just_pressed` | Source paths in DECISIONS; compiled and exercised |
| No dependency added | `Cargo.lock` and the manifests are unchanged by M3 (I9); `cargo tree` of `sim` therefore unchanged (I1) |

### Deviations from PLAN.md

1. The economy terms of units, the buildings, harvesters, and refineries are in a new data file, `assets/data/economy.ron` (schema 1), not in the units file, which keeps schema 1 (D23 item 1).
2. `Players` holds no faction or visibility grid yet (M7, M4). `Health`, `Armour`, `Weapon`, and `Sight` arrive in M4; buildings are removed only by selling in M3.
3. New `Command` variants: `Harvest`, `QueueProduction`, `CancelProduction`, `PlaceBuilding`, `SellBuilding`, `SetRallyPoint` (`Attack` and `AttackMove` are M4's). Visibility is still not validated (no fog before M4). New rejection reasons and events are listed in D25 items 7 and 9. Unit commands do not require the issuing player to be a player of the match (as in M2); economy commands do.
4. Under low power, "defensive structures do not fire" is M4's (there are no weapons in M3).
5. The game's starting credits and construction yards are match setup in game code until M6 (D27 item 3), like the starting units of M2 (D20 item 4).
6. Interim keyboard controls for the economy (D27 item 4) until the sidebar of M5, and credits and power in the window title.
7. Files not listed in PLAN.md section 4: `crates/sim/src/{building,economy,economy_commands,economy_defs,harvest}.rs`, `crates/game/src/{buildings,construction}.rs`, `crates/game/examples/gen_building_sprites.rs`, the M3 test files and fixtures. `m3_definitions.rs` contains `unsafe` code (the counting allocator of D15). `buildings.rs` includes the building sprite generator in its tests with `#[allow(clippy::duplicate_mod)]`, because the unit sprite generator already includes the shared PNG encoder in the same test build.
8. `spawn_unit` now reports `UnknownUnitType` where it would have used speed 0 for an inconsistent definition (unreachable through `unit_kind`; a change of M2 code made while adding the harvester component).

### Known debt (P2 proposals for the reviewer)

1. Units waiting to leave a building (a complete item, or a free unit) wait as long as every ring cell is held; a free unit of a building that is sold before it leaves is lost. A complete unit can be cancelled with a full refund.
2. A stopped harvester (`NoOre`, `NoRefinery`, `Blocked`) does not resume by itself when ore or a refinery becomes available; it waits for a `Harvest` command.
3. `Harvest` chooses the ore cell nearest to the target over terrain without checking that the harvester can reach it; if it cannot, the move ends with `MoveFailed { Unreachable }` and the harvester then looks from where it stands (seen in `a_harvester_with_no_reachable_ore_idles`).
4. Each ore or dock search is a Dijkstra over the map (at most 65,536 cells) per harvester decision; production builds the list of buildings every tick. To be measured against the M7 benchmark. `m2_movement` took 36.8 s against 31.5 s in the M2 report (same tests; the `Economy` stage now runs every tick).
5. The interim controls give no validity feedback before a placement; a rejected placement is logged as a warning (the sidebar's placement preview with validity colouring is M5's).
6. Building draw order uses the footprint's centre; units close to a large building's side may be drawn on the wrong side of it (user verification below).
7. `Players::add` and `spawn_building` are public setup APIs, like `spawn_unit` and `Grid::set_terrain` (M2 debt item 5).
8. The window title is the only display of credits and power until M5.
9. Earlier debt is unchanged: M2 review 3 P2-1 (speed dependence of the end rule in jams) and P2-2 (box selection released outside the window), the M2 remediation round 2 items 1 to 7, M2 review 1 P2-2 remainder, M1 review 3 P2-1 (map and `.tsx` reader), P2-2, P2-3, M1 review 1 P2-3 and P2-5, M1 review 2 P2-B, M1 round 1 item 4, round 2 items 5 and 6 (followed for the building sprites), and M0 items 1, 2 (seen again in this round's smoke runs), 3, 5, 7, 8, and 9.

### Requires user verification

Not review findings (PLAN.md section 12 item 2); they block only `m3-approved`. Run `cargo run --release -p game`. The human's construction yard is at cells (15..17, 37..39), below the human's starting units on screen; the ore field around (22, 44) is right of it.

1. **Buildings:** a blue-tinted grey box (one roof band) stands exactly on the 3×3 footprint of the human's yard (the hovered-cell outline on its base cells matches its base corners), and a red-tinted one on the opponent's yard at (52..54, 42..44). Units walk around buildings, never through them.
2. **Draw order:** units in front of a building (lower on screen) are drawn over it, and units behind it are hidden by it.
3. **Title:** the window title reads `MyGame001 - credits 5000 - power 0/20` and updates as credits and power change; `(low)` appears under low power.
4. **Production:** `2` queues a power plant (cost 300, 200 ticks): the credits fall by 1 per tick for 10 s and by 101 on the last tick (equation 3: 300 / 200 = 1, remainder 100), and the log reports it complete; Shift+`2` cancels it and refunds what was charged.
5. **Placement:** with a building ready, a yellow outline of its footprint follows the cursor; `B` places it there if the cells are buildable, free, and within 4 cells of an own building (otherwise the log shows the rejection), and the building appears. After a power plant the title shows power 0/120.
6. **Refinery and harvesting:** `3` queues a refinery (after a power plant); placed near the ore field, a harvester (a broad box) leaves it, drives to the ore, harvests for about 5 s, returns, and the credits rise by 700 per delivery.
7. **Units and rally points:** `4` barracks and `5` war factory; then `6` light tank, `7` scout (barracks), `8` harvester leave their buildings. `R` over a cell sets the rally point of the player's unit-producing buildings, and later units walk there.
8. **Selling:** `Delete` or `Backspace` over an own building removes it, the credits rise by its sell value (half its cost), and units can walk over its cells.
9. **Harvest order:** with a harvester selected, a right click on an ore cell sends it to harvest there; with other units selected too, they move there.
10. **Low power:** with more power consumed than produced (for example two refineries, a war factory, and a barracks with one power plant: 130 against 120), the title shows `(low)` and production takes about twice as long.

The M2 items (unit rendering, facings, movement, selection, orders) and the M1 items (map rendering, picking, panning, zoom, bounds) remain pending for `m2-approved` and `m1-approved`.

## M3 review decision (2026-10-04)

One independent review round (`docs/reviews/M3-review-1.md`: GO WITH DOCUMENTED NON-BLOCKING DEBT, 0 P0, 0 P1, 4 P2). Before the review the orchestrator reran all six validation commands of PLAN.md section 9 on commit `9aab887` with exit code 0 (248 tests passed). Under PLAN.md section 12 item 1, `milestone/m3` was merged into `main` and tagged `m3-reviewed`. The tag `m3-approved` is created by the user alone after the verification below.

### Debt recorded from review 1 (P2, verbatim from the report)

#### P2-1: A building's pending free unit is silently lost when the building is sold before the unit leaves
* **Files and lines:**
  * `crates/sim/src/economy_commands.rs:253-271` (`SellBuilding` despawns the building with its `pending_units`);
  * `crates/sim/src/economy.rs:389-400` (pending units leave only from buildings that still exist).
* **Evidence:** `probe pending`, release, fixture data.
  * A refinery was placed while all 14 passable cells of its ring were held by walkers. The placement tick emitted only `BuildingPlaced`, and the refinery kept `pending [UnitKind(2)]`.
  * It was then sold. The sale tick emitted only `BuildingSold { refund: 100 }`.
  * 200 more ticks produced no event, there were 0 harvesters on the map, and the credits were 900. Nothing reports that the harvester was lost.
* **Context:** the implementer recorded this behaviour as known debt item 1. It needs every ring cell held at placement time, and the state stays consistent (the hash covers `pending_units`). It is therefore not a P1 silent fallback of an error path; it is an undocumented-in-events loss of game state.
* **Expected behaviour:** either the free units are refunded or delivered, or `BuildingSold` (or a separate event) reports the units that were lost. A test should pin the chosen behaviour.

#### P2-2: Dock cells are not claimed, so crowded refineries make harvesters stop as `Blocked` with ore that is never delivered

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

#### P2-3: DECISIONS cites a test that does not exist

* **File and line:** `docs/DECISIONS.md:358` (A1 follow-up of M3) cites `m3_economy::two_placements_in_one_tick_see_each_other`.
* **Evidence:** `grep -rn two_placements_in_one_tick_see_each_other crates docs` finds only that line. The test that observes the effect is `later_commands_of_a_tick_see_the_buildings_placed_before_them` (`crates/sim/tests/m3_economy.rs:995`).
* **Expected behaviour:** the decision cites the existing test.

#### P2-4: A balance value lives in game code

* **File and line:** `crates/game/src/buildings.rs:41` (`STARTING_CREDITS = 5_000`, with the starting yard positions next to it).
* **Evidence:** invariant I10 says that balance values live only in data files. The implementer records this as deviation 5 and D27 item 3 ("match setup in game code until M6"), following the precedent of M2's starting units (D20 item 4). `sim` and its tests are unaffected.
* **Expected behaviour:** carry this as debt, and move starting credits (and starting buildings and units) to data with the match setup of M6.

### Recorded debt to carry (verbatim from review 1)

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

### Requires user verification (blocks only `m3-approved`; verbatim from review 1)

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

## M4 implementation report (2026-10-04)

Branch `milestone/m4`, based on `main` at the merge of M3 (tag `m3-reviewed`). Commits: `9608923` (sim: combat file, combat, projectiles, death and cleanup, visibility, commands, tests and fixtures), `e19677d` (game: combat file, gun turret, fog rendering, hiding, interim controls, smoke test), and the documentation commit that adds this report. Decisions: DECISIONS "M4: combat and fog of war" (A1 follow-up, D28 to D31) and the M4 amendment of D6.

### Files changed

| File | Change |
|------|--------|
| `crates/sim/src/combat_defs.rs` (new) | Combat file schema 1: armour classes, warheads, weapons, combat terms; limits; subset schema; validation with references, uniqueness, and completeness (D28) |
| `crates/sim/src/combat.rs` (new) | `Health`, `Armour`, `Sight`, `Weapon`, `AttackOrder`, `Projectile`, `Projectiles`; `applied_damage` (equation 2), `in_range` (equation 1); the `combat` system (projectiles, target choice, `Attack` and `AttackMove`, firing, damage) and `remove_dead` (D29); 3 unit tests |
| `crates/sim/src/visibility.rs` (new) | `Vision` (visible and explored bits), `sees_unit`, `sees_footprint`, the `update_visibility` system (equation 6, D30); 2 unit tests |
| `crates/sim/src/definitions.rs` | `Definitions` holds the combat definitions; `with_combat`, `load_match_definitions`, accessors (`has_combat`, `armour_classes`, `warheads`, `weapons`, `weapon_kind`, `armour_class`, `versus`, `unit_combat`, `building_combat`); `with_economy` removes combat definitions loaded before it; hash extended |
| `crates/sim/src/data.rs` | `DataErrorKind::MissingEntry`; documentation of the six-level schema |
| `crates/sim/src/command.rs` | `Command::Attack`, `Command::AttackMove`; `RejectReason::{NotArmed, OwnTarget, UnknownTarget, NotTargetable}`; player check for both, `Attack` target checks, unseen foreign units reported as `UnknownUnit` under fog; `Move`/`Stop`/`Harvest` end combat orders; `apply_move` returns the assigned destinations |
| `crates/sim/src/economy_commands.rs` | Combat components for placed buildings; unseen foreign buildings reported as `UnknownBuilding` under fog; building index lookup |
| `crates/sim/src/{building,unit,economy}.rs` | Combat components on every spawn path; `PlayerState::vision`, hashed |
| `crates/sim/src/{event,schedule,hash,lib}.rs` | Events `WeaponFired`, `ProjectileExpired`, `UnitDestroyed`, `BuildingDestroyed`, `AttackEnded` and `AttackEnd`; systems in `Combat`, `Cleanup`, `Visibility`; `Projectiles` resource; hash of combat components, weapons, visions, projectiles; exports |
| `crates/sim/tests/m4_combat.rs` (new), `tests/fixtures/data/{units_combat,economy_combat,combat}.ron` (new) | 26 tests through `SimPlugin` and `CommandQueue` (below) |
| `crates/sim/tests/m4_definitions.rs` (new) | 12 tests of the combat file, bounded as in D15 |
| `crates/game/src/fog.rs` (new) | Tile tint by the human player's vision, recent shots, projectile and shot drawing; 4 tests |
| `crates/game/src/{units,buildings}.rs` | Combat file loaded (`COMBAT_PATH`); views of unseen enemies hidden (`unit_shown`, `building_shown`); new events logged; opponent's starting gun turret; 1 test |
| `crates/game/src/selection.rs` | Right click on a visible enemy: `Attack`; `A`: `AttackMove`; 2 tests |
| `crates/game/src/{projection,smoke,main}.rs` | `tile_cell` (1 test); smoke checks of the fog and of hidden views (1 test); `FogPlugin` |
| `crates/game/examples/gen_building_sprites.rs`, `assets/sprites/buildings/gun_turret.png` (new) | Gun turret placeholder sprite |
| `assets/data/combat.ron` (new), `assets/data/economy.ron` | Production combat definitions; gun turret building type |
| `assets/LICENSES.md`, `docs/DECISIONS.md`, `docs/STATUS.md` | Provenance; decisions; this report |

`Cargo.toml` files and `Cargo.lock` are unchanged (no dependency added, I9), so `sim`'s dependency tree is unchanged (I1). No existing test was removed, skipped, or changed; the M2 and M3 fixtures are unchanged.

### Validation commands (PLAN.md section 9, with section 12 item 5)

Run from the repository root on macOS 27.0.1 (Apple M5, Rust 1.98.1), in this order, in the foreground, after the last code change (`e19677d`'s content). A display was available.

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 300 passed, 0 failed, 0 ignored: `game` unit 67 (M3: 58; + fog 4, selection 2, smoke 1, units 1, projection 1), `m0_spike` 6, `m1_demo_map` 3, `m1_tileset_image` 6, `m2_unit_sprites` 5, `m3_building_sprites` 3, `sim` unit 11 (M3: 6; + combat 3, visibility 2), `m0_bootstrap` 12, `m1_map` 32, `m1_map_subset` 24, `m2_definitions` 14, `m2_movement` 37 (39.5 s), `m3_definitions` 11, `m3_economy` 21, `m3_harvest` 10, `m4_combat` 26 (1.3 s), `m4_definitions` 12 (1.5 s), doc-tests 0. Peak memory footprint of the run 0.13 GB. Known M0 linker warning only. |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | `3 buildings spawned`, `26 units spawned`; `smoke test: 600 frames, 198 simulation ticks, map created: true, units spawned: 26, units moved: 18 in 198 ticks after the order, buildings spawned: 3, production charged 396 credits in 198 ticks, visible cells: 389, enemy views shown: 0, view mismatches: 0` |
| `cargo run -p game -- --smoke-test 600` | 0 | `smoke test: 600 frames, 203 simulation ticks, ..., buildings spawned: 3, production charged 406 credits in 203 ticks, visible cells: 399, enemy views shown: 0, view mismatches: 0` |

The `Skipped event Destroyed` warning (M0 debt item 2) appeared in both smoke runs.

Additionally:

* `cargo test -p sim --release --test m4_combat` passed; the 20 against 20 battle ends at tick 665 with 6 human units left, 451 shots, and 34 units destroyed, and its final hash is `05af3b1ac550cf16` in both debug and release builds (I3, I12).
* The release binary was run (`--smoke-test 100`, under a 60 s alarm, `/usr/bin/time -l`) against copies of `assets/` selected with `BEVY_ASSET_ROOT`. The unchanged copy exits 0. Each of these exits 1 with a typed error naming the file and field, at a peak of 0.11 GB: no `combat.ron` (`data file …/combat.ron: file: cannot read the file: No such file or directory`); `#![enable(implicit_some)]` prepended (`file: line 1, column 1: expected a value or punctuation of the supported subset, found '#'`); the gun turret's entry removed (`buildings: building "gun_turret" has no entry`); `combat.ron` a named pipe (`file: cannot read the file: not a regular file`, at once); a range of 383 (`weapons[1].range: value 383 is not supported, expected 384 to 4096 subcells`); an unknown warhead (`weapons[1].warhead: warhead "laser" is not defined`); eight nested lists (`units[1].weapons[0]: line 71, column 79: expected a string, found '['`); `gun_turret.png` removed (`data file …/economy.ron: buildings[5].sprite: sprite image …/gun_turret.png: file: cannot be read: …`).

### Acceptance criteria and required tests (PLAN.md section 7, M4)

| Criterion | Evidence |
|-----------|----------|
| Scope: weapons, warheads, armour classes | Combat file (D28); `m4_definitions` (12 tests: every field, unknown and duplicate fields, wrong kinds, schema versions, every numeric field at its minimum and maximum and beyond including `i64` extremes and the projectile speed's 0 / 1 / 127 / 128 / 4096 / 4097, list bounds, references, missing entries, duplicates, names, missing / oversized / named-pipe / directory files, truncations, 2,000 seeded mutations); production data: `units::tests::the_production_data_loads_with_the_combat_file` |
| Scope: instant and travelling projectiles | `a_projectile_hits_after_its_flight_time` (fired in tick 1, 6 ticks of flight, damage exactly in tick 7), the damage test (instant hits at once, projectiles after `ceil(gap / speed)`), `the_longest_range_with_the_slowest_projectile_flies_32_ticks` |
| Scope: automatic target acquisition | Idle units and buildings fire at visible enemies in range (the damage, range, and projectile tests rely on it); `a_unit_on_a_plain_move_does_not_fire_and_an_attack_move_fights_on_the_way`; `a_turret_fires_beyond_its_own_sight_only_at_enemies_its_owner_sees` |
| Scope: `Attack` and `AttackMove` | `an_attack_order_closes_in_fires_and_ends_when_the_target_is_gone` (moves into range, fires from within range, `AttackEnded { TargetGone }`, stays), `an_attack_order_ends_when_the_target_leaves_sight` (`TargetHidden`), `an_attack_order_on_a_target_out_of_reach_ends_at_once` (`OutOfReach`), the attack move (stops, fires 3 times, kills, then arrives), `unarmed_units_follow_an_attack_move_as_a_move`; interface: `selection::tests::a_right_click_on_a_visible_enemy_orders_the_armed_units_to_attack_it`, `the_attack_move_key_orders_the_whole_selection` |
| Scope: death and cleanup | `remove_dead`; every tick of every M4 test asserts that no entity with hit points ≤ 0 survives the tick and that the grid's occupants and buildings match the entities exactly |
| Scope: building destruction that frees cells | `a_destroyed_building_frees_its_cells`, `a_destroyed_building_reports_the_free_units_lost_with_it` |
| Scope: defensive structures | Gun turret (production), turret (fixture); `defensive_structures_hold_fire_under_low_power` (fires; stops when the plant is sold; fires again after a new plant is placed through commands) |
| Scope: shroud and fog | `Vision` (visible and explored); `enemies_are_visible_exactly_within_sight_and_explored_cells_stay_explored`; game: `fog::tests::cells_are_tinted_by_what_the_human_player_sees`; on screen **requires user verification** |
| Scope: hiding of unseen enemies in rendering and in command validation | Rendering: `fog::tests::enemies_are_shown_exactly_while_the_human_player_sees_them`, and the smoke check (0 mismatches between views and vision in both smoke runs); on screen **requires user verification**. Validation: `attacks_on_own_dead_and_unseen_units_are_rejected_without_changing_state`, `commands_that_name_an_unseen_enemy_report_it_as_unknown` |
| Test: the damage equation for every armour and warhead pair in the fixtures, including the minimum of 1 and the zero case | `the_damage_equation_holds_for_every_warhead_and_armour_pair_of_the_fixtures`: 3 weapons (all 3 warheads) × 3 targets (all 3 armour classes), each first shot through the production path compared with equation 2 written out independently in the test; it asserts that the cases include the zero case (bullet vs concrete, 0 %), the minimum of 1 (20 × 1 % = 0.2 → 1), rounding down (25 × 30 % = 7.5 → 7), and exact products. Unit test `applied_damage_follows_equation_2_at_the_limits` (maximum damage and percentage, values beyond the format) |
| Test: range at exactly `range` and at `range + 1` subcell | `a_target_is_in_range_at_exactly_the_range_and_not_one_subcell_beyond`: a crawler (speed 1) walks toward an idle rifleman (range 768) one subcell per tick; the first shot comes in exactly the tick in which the distance falls from 769 to 768. `a_building_is_in_range_when_its_nearest_edge_is` (640 in range, 896 not), `the_longest_range_...` (4,096 fires, 17 cells does not), unit test `in_range_includes_exactly_the_range` |
| Test: a target that dies while a projectile is in flight | `a_target_that_dies_while_a_projectile_is_in_flight_makes_it_expire`: three riflemen kill the crawler in the tick in which the tank fires; six ticks later the shell expires with `ProjectileExpired`, no damage, no panic, no projectile left |
| Test: attacks on own units, dead units, and unseen units are rejected | `attacks_on_own_dead_and_unseen_units_are_rejected_without_changing_state`: own unit, own building, the attacker itself (`OwnTarget`); a dead unit, an unseen unit and building, a never existing id, any target before the first tick (`UnknownTarget`); unarmed attacker (`NotArmed`), foreign attacker (`NotOwned`), no units, unknown player, attack-move outside the map (including `i32` extremes); the state hash equals a control run after every rejection; the unseen unit is accepted once seen |
| Test: a destroyed unit or building releases its cells | `a_destroyed_unit_releases_both_cells_of_its_step` (killed between two cells, both freed, re-entered), `a_destroyed_building_frees_its_cells` (footprint uncovered and passable, power recomputed, a unit walks onto it) |
| Test: sight radius boundary | `a_unit_sees_exactly_the_cells_within_its_sight_radius` (the visible set equals equation 6 written out independently, in the open and in two corners; the cells at sight and sight + 1 along both axes, 3-4-5 on the diagonal), `a_building_sees_from_the_nearest_cell_of_its_footprint`, `a_destroyed_unit_stops_seeing_in_the_tick_it_dies`, unit test `reveal_follows_equation_6_and_stays_inside_the_map` (sight 0 and 16, `i32` extremes) |
| Test: determinism of a 20 against 20 battle | `twenty_against_twenty_fight_to_a_conclusion_under_fog_with_a_stable_hash`: identical hash at every tick of two runs, same end; debug and release give the same final hash |
| Acceptance: two armies fight to a conclusion under fog with no panic and a stable hash across runs | The battle test: two armies of 20 that start out of each other's sight, ordered only through `CommandQueue` toward the enemies they see (else the far side), fight until the enemy army is destroyed at tick 665, without a rejected command, with I6 checked on every tick and the same hash at every tick across runs. In the game, the same is possible with the interim controls; on screen **requires user verification** |
| Section 8: negative, boundary, integration, end to end | Negative: every rejection with no state change, malformed combat files; boundary: equation 1 at the range, sight at the radius, range 384 and 4,096, projectile speed 128, reload 1, sight 0 and 16, percentages 0 and 1,000, map corners; integration: all `m4_combat` tests; end to end: both smoke runs (fog computed, hiding consistent) |
| Invariants | I1/I9: no dependency added. I2: no float in `sim`. I3: hash of all new state, determinism tests. I4: `BTreeMap`/`BTreeSet` and `SimId` order everywhere; revealing is a union. I5: the game pushes commands only. I6: checked every tick, including units killed between two cells and destroyed buildings. I7: credits never negative (checked every tick). I8/I11: the combat file passes the RON subset before `ron`; the turret sprite passes the PNG subset. I12: D29 item 11, D30 (saturating vision arithmetic) |

### Assumptions verified and how

| Item | Verification |
|------|--------------|
| `bevy_ecs_tilemap` 0.19.0 tints a tile by `TileColor` and re-extracts it on change; `bevy_ecs_tiled` gives every tile a white `TileColor` and the `TiledTile` marker | Source (DECISIONS A1 follow-up of M4); the game compiles and both smoke runs create the map |
| `Sprite` requires `Visibility`; `Visibility::Hidden` hides an entity | Source; the smoke test reads back every view's `Visibility` and finds it consistent with the vision |
| `Without` filters make two queries writing the same component disjoint for Bevy's ambiguity check | The PostUpdate conflict test failed with the unit and building view systems unordered and passes with the filters |
| `u64::isqrt` is stable in Rust 1.98.1 | Toolchain documentation (1.84.0); compiled and tested |
| Cleanup's despawns are applied before the `Visibility` stage of the same tick | `a_destroyed_unit_stops_seeing_in_the_tick_it_dies` |
| No new crate | `Cargo.lock` and the manifests unchanged |

### Deviations from PLAN.md

1. The combat terms of units and buildings, armour classes, warheads, and weapons are in a new data file, `assets/data/combat.ron` (schema 1), not in the units or economy file, which keep schema 1 (D28 item 1, as D23 item 1 in M3).
2. Additional events (`WeaponFired`, `ProjectileExpired`, `UnitDestroyed`, `BuildingDestroyed`, `AttackEnded`) and rejection reasons (`NotArmed`, `OwnTarget`, `UnknownTarget`, `NotTargetable`); an unseen foreign unit or building named by a command is `UnknownUnit` / `UnknownBuilding` under fog (D30 item 4).
3. `Attack` and `AttackMove` require the issuing player to be a player of the match (`UnknownPlayer`); `Move`, `Stop`, and `Harvest` keep the M2 rule (D30 item 4).
4. Commands that name cells are not restricted to visible cells (D30 item 4). PLAN.md section 5 says every command is validated for visibility; the restriction applies to the units and buildings commands name.
5. Fog of war is active when the definitions include a combat file; without one (the M2 and M3 fixtures) nothing has sight and the M2 and M3 command validation is unchanged (D30 item 4).
6. Projectiles are guided (they always hit their target if it still exists) and kept in a `Projectiles` resource rather than as entities (D29 item 3).
7. Engagement of attack-moving and idle units uses the weapon's range; units do not chase targets they were not ordered to attack (D29 items 5 and 7).
8. Interim controls (right click on an enemy, `A`) until M5's hotkeys (D31 item 4); the opponent's starting gun turret is match setup in game code until M6 (D31 item 5). Because the gun turret is the sixth building type, the production menu's unit keys moved by one: `7` light tank, `8` scout, `9` harvester (M3's user verification item 7 named `6`, `7`, `8`).
9. Files not listed in PLAN.md section 4: `crates/sim/src/{combat,combat_defs,visibility}.rs`, `crates/game/src/fog.rs`, the M4 test files and fixtures. `m4_definitions.rs` contains `unsafe` code (the counting allocator of D15).

### Known debt (P2 proposals for the reviewer)

1. Cost per tick: target acquisition compares every armed entity with every target (O(A × T)), a chase runs a Dijkstra over the map, and every vision is recomputed in full (at most 33 × 33 cells per source). To be measured against the M7 benchmark of 200 units. `m2_movement` took 39.5 s against 36.8 s in the M3 report (the new stages run every tick, with nothing to do there).
2. The projectile bound of D28 item 5 (32 per weapon) allows about 200 MB in the pathological case of a map filled with armed entities of reload 1 and the slowest projectiles; no real match comes near it.
3. Attack-moving and idle units do not chase enemies beyond their weapon range, and do not return fire against an attacker that outranges them (for example, a gun turret); an explicit `Attack` order chases.
4. An `Attack` order on a target its warhead cannot damage (0 %) lasts as long as the target is alive and visible.
5. An attack-moving unit that fights at its destination writes a second `MoveCompleted` when it resumes its (zero-length) move.
6. `combat` treats a weapon or percentage missing from the definitions as no damage, and a projectile whose weapon is missing as expired. Both are unreachable: the definitions are validated complete and do not change during a match (they can only be replaced by setup).
7. `Definitions::with_economy` removes combat definitions loaded before it (documented on both functions and tested); the game loads them in the right order.
8. Health bars are M5's; until then damage is not visible on screen, only destroyed units and buildings disappearing. Instant shots are drawn from `WeaponFired` events read with a `MessageReader`, so M2 debt item 2 (two-tick window) also applies to them.
9. The smoke test does not exercise combat in the real binary (the starting armies are 40 cells apart); combat in the game is part of the user verification below. No automated screenshot exists (M2 debt item 6).
10. M3 review 1 P2-1 (a building's pending free units are lost on sale without an event) is unchanged for selling; destruction now reports them (`BuildingDestroyed::lost_units`).
11. Carried, unchanged by M4: every item under "Recorded debt to carry (verbatim from review 1)" of the M3 review decision above, with M3 review 1 P2-1 narrowed as in item 10. M1 round 2 item 6 (register every asset in `CheckedAssets`) was followed for the gun turret sprite; the `Skipped event Destroyed` warning of M0 item 2 appeared again in this round's smoke runs.

### Requires user verification

Not review findings (PLAN.md section 12 item 2); they block only `m4-approved`. Run `cargo run --release -p game`. The player's units start around (10, 28) and (14, 34), left of the rock ridge; the opponent's units around (50, 30) and (52, 36), its construction yard at (52..54, 42..44), and its gun turret at (50, 41).

1. **Shroud:** at the start only the area around the player's units and construction yard shows the map; everything else is black.
2. **Fog:** cells the player's units have seen and no longer see are drawn dark but recognisable; cells they see now are drawn normally. Moving units uncover the map as they go.
3. **Hidden enemies:** the opponent's units, yard, and turret are not drawn until one of the player's units comes close (light tanks see 6 cells, scouts 8); they disappear again when no unit of the player sees them.
4. **Attack:** select some light tanks and scouts, move them toward the opponent, and right click on a visible enemy unit or building: the units move until it is in range, then fire. Light tank shells are drawn as small yellow dots flying to the target; scout shots as short pale lines. A unit or building whose hit points run out disappears, and units can then walk over a destroyed building's cells.
5. **Attack-move:** with units selected, press `A` over a cell: they move there and stop to fight enemies that come into range on the way, then continue.
6. **Automatic fire:** idle units fire at visible enemies within range without an order; units on a plain move (right click on ground) do not fire.
7. **Gun turret:** the opponent's turret fires at the player's units that come within six cells; the player can build a turret with `6` (after a power plant), place it with `B`, and it fires at enemies within its range while the player has power (under low power, shown as `(low)` in the title, it does not fire).
8. **Menu keys:** the unit production keys are now `7` light tank, `8` scout, `9` harvester (M3's item 7 named `6`, `7`, `8`).

The ten M3 items, the seven M2 items, and the five M1 items listed in docs/STATUS.md remain pending.

## M4 remediation round 1 report (2026-10-04)

This is the implementer session for remediation round 1 of M4 review 1 (`docs/reviews/M4-review-1.md`, NO-GO: 0 P0, 2 P1, 3 P2).

* **Branch and base:** `milestone/m4`, base `0cb2199`. Code commit `652d180`; this report and the decisions are in the documentation commit that follows it.
* **Notes:** none accompanied the findings beyond the orchestrator's instruction to close each class of defect (every command whose rejection reason or event could reveal hidden information; every acquisition or order rule that can loop forever on a target it cannot affect). Each finding was treated as a claim and reconciled against the code.
* **"Fails before" evidence:** the regression tests were written first and run against the reviewed code with only the two new enum variants (`AttackEnd::NoDamage`, `PlacementError::NotVisible`) added, so that they compiled; the game's new decision function was first added with the old rule. The P1-2 leak was also reproduced on a `git archive` of `0cb2199` with a probe test outside the repository (`…/scratchpad/m4-remediation-1/base`), run under a `perl alarm` timeout.
* **Decisions:** DECISIONS D32 and D33 (new section "M4 remediation round 1"), with amendments in place to D29 items 5, 6, and 7, D30 item 4, D31 item 3, and the restoration of D6 (P2-3). The milestone table above is maintained by the orchestrator.

### Classes of defect

1. **Rules that keep a unit or building engaged with a target it cannot affect, so that the engagement, and any order waiting for it, never ends** (P1-1). Every rule that chooses, keeps, or fires at a target, or that waits for a fight, was checked (D32 item 1): automatic acquisition and keeping (idle units, attack-moving units, defensive structures) were defective and are fixed; the explicit `Attack` on such a target (M4 implementation debt item 4) belonged to the same class and is fixed; the chase limit, projectile flight, low power, move orders, and the harvester cycle are bounded and unaffected.
2. **Command validation whose decision or reason depends on cells or entities the issuing player does not see** (P1-2). All ten command variants were audited (D33 item 1, with a table): only `PlaceBuilding` read hidden state, and it is fixed. The on-screen member of the same class (P2-1, an instant shot drawn from an unseen attacker) is fixed. The boundary of the claim (outcomes of accepted orders are computed on the true state; the event stream is shared) is stated in D33 item 3 and recorded as debt.

### Findings

#### P1-1: target acquisition engages enemies the warhead cannot damage

**Outcome: confirmed. The class is fixed.**

* **Evidence (code).** `combat.rs` `nearest_enemy_in_range` filtered only by owner, visibility, and range, and `keep_or_acquire` kept any visible enemy in range; neither looked at the warhead's percentage. `act_unit`'s `AttackMove` branch stops the move whenever a target exists and resumes only when none does. The fixture's bullet deals 0 % to concrete (`crates/sim/tests/fixtures/data/combat.ron`).
* **Evidence (tests before the fix).** Each new test below failed on the reviewed code: the attack move past a fence never wrote `MoveCompleted` within 600 ticks; the idle rifleman did not fire at the enemy rifleman in tick 1 (it fired at the fence); the turret of a variant whose howitzer deals 0 % to concrete never fired at the crawler; an explicit `Attack` on a fence never ended.
* **Change** (D32; `crates/sim/src/combat.rs`, `event.rs`):
  1. `Field::damages(weapon, target)`: equation 2 gives more than 0.
  2. Acquisition and keeping consider only visible enemies in range that the weapon damages, for idle units, attack-moving units, and defensive structures alike.
  3. An explicit `Attack` on a target the warhead deals 0 to is carried out once: the unit closes in, fires one shot (0 damage), and the order ends in that tick with `AttackEnded { reason: NoDamage }`. D32 item 3 gives the reason for this choice over keeping the documented semantics, rejecting at validation, or ending before the shot (the zero case of equation 2 must stay reachable through `SimPlugin`).
  4. D29 items 5, 6, and 7 are corrected.
* **Regression tests** (`crates/sim/tests/m4_combat.rs`; all fail before the fix and pass after it):
  * `an_attack_move_past_an_enemy_it_cannot_damage_ends_when_its_move_ends`: the reviewer's scenario (rifleman from (4, 16) to (20, 16), enemy fence at (12, 18), which it sees in range on the way). The move completes at (20, 16) with no shot; the fence keeps 50 hit points; order and target are cleared.
  * `an_idle_unit_fires_at_an_enemy_it_can_damage_and_not_at_a_nearer_one_it_cannot`: the reviewer's scenario (fence at (11, 16), enemy rifleman at (12, 17)). The first shot in tick 1 is at the rifleman; no shot at the fence in 61 ticks; the enemy rifleman is destroyed.
  * `a_defensive_structure_fires_at_an_enemy_it_can_damage_and_not_at_a_nearer_one_it_cannot`: a variant with the howitzer at 0 % against concrete; the turret fires once at the crawler three cells away (killing it) and never at the adjacent fence, and has no target afterwards.
  * `an_attack_order_on_a_target_it_cannot_damage_ends_after_one_shot`: in range (from x 10) and out of range (from x 8, it moves first). Exactly one `WeaponFired`, the fence keeps 50 hit points, `AttackEnded { NoDamage }`, no rejection, and in the 100 idle ticks after it no shot and no further `AttackEnded`.

#### P1-2: `PlaceBuilding` rejections reveal the id and cell of unseen enemy units and buildings

**Outcome: confirmed. The class is fixed.**

* **Evidence (code).** `economy_commands.rs` called `check_footprint` without any fog handling, and `PlacementError::{Overlap, Occupied}` carry the blocker's `SimId` and cell.
* **Evidence (reproduction on `0cb2199`).** The reviewer's setup (human yard at (10, 10), placement distance 3; enemy at (16, 14), not visible to the human player; power plant placed at (15, 13)) gives `Placement(Occupied { cell: (16, 14), unit: SimId(1) })`, "cell (16, 14) is held by unit 1", with a crawler, and `Placement(Overlap { cell: (16, 14), building: SimId(1) })` with a beacon. The new test also failed before the fix: with nothing at (16, 14), the placement over the unseen cells was accepted.
* **Change** (D33; `crates/sim/src/building.rs`, `economy_commands.rs`): under fog of war (a combat file loaded), `PlaceBuilding` checks after `OutsideMap` and before the terrain, building, and unit checks that the player sees every cell of the footprint (`check_visible`), and otherwise rejects with the new `PlacementError::NotVisible(cell)` for the first unseen cell. Every unit or building in a visible cell is visible (D30 item 3), so `Overlap` and `Occupied` now name only visible entities. Without a combat file nothing changes (M3 tests unchanged). D30 item 4 and its rationale, which did not hold (Chebyshev placement distance against Euclidean sight), are corrected; implementation deviation 4 is corrected by this report (see "Deviations").
* **Regression tests** (`m4_combat.rs`):
  * `a_placement_over_cells_the_player_does_not_see_reveals_nothing_about_them`: nothing, an enemy crawler, or an enemy beacon at (16, 14). In all three runs, the rejection is the same `Placement(NotVisible((16, 13)))`, contains no blocker id, and leaves the state hash equal to a control run. The building stays ready and is then placed on visible cells. Fails before the fix (above).
  * `a_placement_over_a_visible_enemy_still_names_it`: an enemy crawler the player sees in the footprint is still reported as `Occupied { cell, unit }`, as in M3, so hiding is limited to unseen cells. Passes before and after (a guard against over-hiding).

#### P2-1: instant-shot lines are drawn from unseen attackers

**Outcome: confirmed and fixed.**

* **Evidence.** `fog.rs` drew the line when `seen(from) || seen(to)`, so a seen target made the line start at the hidden attacker's exact body point.
* **Change** (D31 item 3, D33 item 4; `crates/game/src/fog.rs`): the pure function `shot_view(attacker_seen, target_seen)` decides: a line when both ends are seen, a small circle at the end that is seen when only one is, nothing otherwise. `draw_combat` uses it.
* **Regression test:** `fog::tests::an_instant_shot_shows_only_the_ends_the_human_player_sees` (all four cases). It failed with the old rule (`Line` where `AtTarget` is expected) and passes now. On screen: **requires user verification** (item 4 below).

#### P2-2: `RejectReason::NotTargetable` is unreachable

**Outcome: confirmed and fixed.**

* **Evidence.** Without a combat file no unit has a weapon, so `validate_combat` returns `NotArmed` first. With one, every unit and building type has combat terms (D28 item 4). No test used the variant.
* **Change:** the variant, its message, and the check are removed (`crates/sim/src/command.rs`; D32 item 5; D30 item 4 corrected). A target without hit points cannot arise; if setup ever created one, the order would end in the next tick with `TargetGone`, because the `Combat` stage considers only entities with hit points.
* **Test:** none can fail before the fix, because the branch was unreachable. Removing the variant makes any use a compile error, and the existing rejection test still covers every remaining `Attack` reason.

#### P2-3: the D6 sentence of the M3 amendment was moved into the M4 amendment

**Outcome: confirmed and fixed.**

* **Evidence:** `git show m3-reviewed:docs/DECISIONS.md` shows the M3 amendment of D6 ending with "The final log line also reports the buildings spawned and the credits charged."; the M4 commit moved that sentence to the end of the M4 amendment.
* **Change:** the sentence is restored to the M3 amendment and removed from the M4 amendment.
* **Test:** documentation only, no test.

#### Implementer's claims named by the review

* **D29 item 7** ("the order ends when a move toward the destination ends"): now true, because an attack-moving unit stops only for an enemy it damages, and such a fight ends (D29 item 7, amended).
* **D30 item 4** ("commands reveal nothing about unseen enemies"): now true for command validation (D33 item 1); its `NotTargetable` clause is removed.
* **D31 item 3** (shots drawn "only where the human player sees them"): now true (amended wording).

### Tests changed (none removed, skipped, or weakened)

* `m4_combat.rs` gains 6 tests and the helpers `combat_variant`, `assert_bullet_cannot_damage_concrete`, `placement_scenario`, `place_power`, and the constant `BULLET_CONCRETE`. `fog.rs` gains 1 test.
* **Three existing tests change their stimulus, not their assertions.** Each relied on an idle rifleman automatically engaging a concrete building (0 %), which is exactly the behaviour P1-1 removes (D32 item 4):
  * `the_damage_equation_holds_for_every_warhead_and_armour_pair_of_the_fixtures`: for the one pair with a percentage of 0, the attacker is ordered to `Attack` before tick 1. It still fires in tick 1, and the test still asserts 0 damage and the presence of the zero, minimum-one, floor, and exact cases.
  * `a_building_is_in_range_when_its_nearest_edge_is`: runs on a fixture variant in which the bullet deals 1 % against concrete; the range assertions (640 in range, 896 not) are unchanged.
  * `the_shortest_range_reaches_every_neighbouring_cell_and_an_adjacent_building`: `extreme_scenario` also raises the bullet to 1 % against concrete; the assertions (a shot at the plant from the ring's corner in each of five ticks) are unchanged.
* The fixture files are unchanged. No other existing test changed, and all pass with their previously reported numbers.
* The battle gives the same figures as before in debug and release: tick 665, survivors 6 to 0, 451 shots, 34 destroyed, final hash `05af3b1ac550cf16`.

### Files changed

| File | Change |
|------|--------|
| `crates/sim/src/combat.rs` | `Field::damages`; acquisition and keeping only of damageable visible enemies in range; `Attack` on an undamageable target ends after its shot; module documentation |
| `crates/sim/src/event.rs` | `AttackEnd::NoDamage` |
| `crates/sim/src/building.rs` | `PlacementError::NotVisible`, its message, `check_visible` |
| `crates/sim/src/economy_commands.rs` | `PlaceBuilding` checks visibility under fog before the footprint checks; module documentation |
| `crates/sim/src/command.rs` | `RejectReason::NotTargetable` and its check removed; documentation |
| `crates/game/src/fog.rs` | `ShotView`, `shot_view`, used by `draw_combat`; 1 test |
| `crates/sim/tests/m4_combat.rs` | 6 new tests and helpers; stimulus of 3 tests (above) |
| `docs/DECISIONS.md` | D32, D33; amendments to D29 items 5–7, D30 item 4, D31 item 3; D6 restored |
| `docs/STATUS.md` | This report |

No dependency, manifest, `Cargo.lock`, data file, fixture, or asset changed (I1, I9, I10).

### Validation commands (PLAN.md section 9, with section 12 item 5)

The commands ran from the repository root on macOS (Apple M5, Rust 1.98.1), in this order, in the foreground, after the last code change (`652d180`'s content). A display was available.

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 307 passed, 0 failed, 0 ignored (breakdown below). Peak memory footprint of the run 0.13 GB. Known M0 linker warning only. |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | `3 buildings spawned`, `26 units spawned`; `smoke test: 600 frames, 198 simulation ticks, map created: true, units spawned: 26, units moved: 18 in 198 ticks after the order, buildings spawned: 3, production charged 396 credits in 198 ticks, visible cells: 389, enemy views shown: 0, view mismatches: 0` |
| `cargo run -p game -- --smoke-test 600` | 0 | `smoke test: 600 frames, 203 simulation ticks, map created: true, units spawned: 26, units moved: 18 in 203 ticks after the order, buildings spawned: 3, production charged 406 credits in 203 ticks, visible cells: 399, enemy views shown: 0, view mismatches: 0` |

Test breakdown for `cargo test --workspace`:

* `game`: unit 68 (+1: `fog`); integration `m0_spike` 6, `m1_demo_map` 3, `m1_tileset_image` 6, `m2_unit_sprites` 5, `m3_building_sprites` 3.
* `sim`: unit 11; integration `m0_bootstrap` 12, `m1_map` 32, `m1_map_subset` 24, `m2_definitions` 14, `m2_movement` 37 (39.3 s), `m3_definitions` 11, `m3_economy` 21, `m3_harvest` 10, `m4_combat` 32 (+6; 1.3 s), `m4_definitions` 12.
* doc-tests 0.

The `Skipped event Destroyed` warning (M0 debt item 2) appeared in both smoke runs. Additionally, `cargo test --release -p sim --test m4_combat` passed (32 tests).

### Acceptance criteria and required tests (PLAN.md section 7, M4)

Unchanged from the M4 implementation report, with these updates:

| Criterion | Evidence |
|-----------|----------|
| Scope: automatic target acquisition | Idle units, attack-moving units, and defensive structures engage only visible enemies in range that their weapon damages: the three new acquisition tests above, and the existing acquisition, range, projectile, and battle tests |
| Scope: `Attack` and `AttackMove` | As before, plus `an_attack_move_past_an_enemy_it_cannot_damage_ends_when_its_move_ends` and `an_attack_order_on_a_target_it_cannot_damage_ends_after_one_shot` (every order ends) |
| Scope: hiding of unseen enemies in command validation | As before, plus the audit of D33 item 1 and `a_placement_over_cells_the_player_does_not_see_reveals_nothing_about_them`, `a_placement_over_a_visible_enemy_still_names_it` |
| Scope: hiding of unseen enemies in rendering | As before, plus `fog::tests::an_instant_shot_shows_only_the_ends_the_human_player_sees`; on screen **requires user verification** |
| Test: the damage equation for every pair, including the minimum of 1 and the zero case | Unchanged assertions; the zero case is now produced by an ordered attack (D32 item 3), still through `SimPlugin` and `CommandQueue` |
| Acceptance: two armies fight to a conclusion under fog with no panic and a stable hash | Unchanged figures and hash in debug and release |

### Assumptions verified and how

No new crate API is used. `Gizmos::circle_2d` was already used by `draw_combat` (DECISIONS A1 follow-up of M4). The new rules use `BTreeMap` iteration and `SimId` order as before (I4). Determinism: the battle's per-tick hash trace is identical across runs, and its final hash is identical in debug and release (I3, I12).

### Deviations from PLAN.md

* Implementation deviation 4 ("commands that name cells are not restricted to visible cells") is narrowed. `PlaceBuilding` now requires visible cells under fog (D33). `Move`, `Harvest`, `AttackMove`, and `SetRallyPoint` still accept any cell inside the map, and their validation reads only the map's bounds, so it reveals nothing.
* Implementation deviation 2 is updated: the rejection reasons added in M4 are `NotArmed`, `OwnTarget`, and `UnknownTarget` (`NotTargetable` is removed). New in this round: the placement error `NotVisible` and the attack end `NoDamage`.
* No new deviation.

### Known debt (P2)

1. **Outcomes of accepted orders use the true state** (D33 item 3). Group destinations skip cells held by idle units and covered by buildings, seen or not. Paths avoid unseen buildings. A harvester avoids ore cells that unseen harvesters have taken. The movement events report the results. Making these fog-aware would need pathfinding on each player's knowledge, which PLAN.md does not specify.
2. **The `SimEvent` buffer is one stream for all players.** Only `CommandRejected` is addressed to a player. The M5 interface and the M6 AI must use only events about what their player sees. The game's terminal log (`units::log_sim_events`) writes every event of both players as a diagnostic.
3. **Placement is limited to cells the player sees** (D33 item 5). With the production data, some footprints within the placement distance include cells beyond sight, and these are rejected with `NotVisible` until something of the player sees them. A placement preview with validity colouring is M5's.
4. **An ordered attack on an undamageable target fires one harmless shot** before it ends (D32 item 3). This is by design, so that the zero case of equation 2 stays reachable.
5. **Carried, unchanged, from the M4 implementation report and confirmed by review 1:** items 1, 2, 3, 5, 6, 7, 8, 9, 10, and 11.
   * Item 4 (an `Attack` on a target its warhead cannot damage lasts while the target stands) is resolved by D32.
   * Item 6 now also covers `Field::damages`, which treats a missing percentage as 0. That case is unreachable.
6. **Carried from M3 review 1, unchanged:** P2-1 (narrowed as in M4 item 10), P2-2, P2-3, P2-4, and M3 implementation debt items 1 to 9.
7. **Carried from M2, M1, and M0, unchanged,** as listed in M4 review 1's "Recorded debt". The linker warning and the `Skipped event Destroyed` warning were seen again in this round.

### Requires user verification

Not review findings (PLAN.md section 12 item 2); they block only `m4-approved`. The eight M4 items listed under "M4 implementation report" remain pending, with these changes:

* **Item 4 (Attack):** scout shots are drawn as short pale lines only when both the scout and its target are on visible cells. When only one end is visible, a small pale circle is drawn there instead.
* **Item 6 (Automatic fire):** idle units and turrets fire only at enemies their weapon can damage.
* **New item 9 (Placement under fog):** placing a building with `B` is rejected (logged as "cell (x, y) is not visible to the player") while any cell of its footprint is not currently visible. It succeeds once units or buildings of the player see every cell.

The ten M3 items, the seven M2 items, and the five M1 items listed in docs/STATUS.md remain pending.

## M4 review decision (2026-10-04)

Two independent review rounds (`docs/reviews/M4-review-1.md`: NO-GO, 0 P0, 2 P1, 3 P2; `M4-review-2.md`: GO WITH DOCUMENTED NON-BLOCKING DEBT, 0 P0, 0 P1, 1 P2). Before review 2 the orchestrator reran all six validation commands of PLAN.md section 9 on commit `dad9bb0` with exit code 0 (307 tests passed). Review 2 judged that the three test setups changed by remediation round 1 were not weakened. Under PLAN.md section 12 item 1, `milestone/m4` was merged into `main` and tagged `m4-reviewed`. The tag `m4-approved` is created by the user alone after the verification below.

### Debt recorded from review 2 (P2, verbatim from the report)

#### P2-1: Projectiles from an unseen attacker are drawn along their line of flight, which points back to the attacker; D33 claims the screen reveals nothing
**Files and lines:**

* `crates/game/src/fog.rs:205-215`: every projectile is drawn at `projectile_point(projectile.origin, target, progress)` whenever that point's cell is visible.
* `crates/game/src/fog.rs:146-150`: `projectile_point`, a linear interpolation from the origin.
* `crates/sim/src/combat.rs:479`: `origin: from.centre()`, the attacker's exact centre.
* `docs/DECISIONS.md:499`: the D33 title, "Commands and the screen reveal nothing about what the player does not see".
* `docs/DECISIONS.md:514`: D33 item 4, "so a shot no longer shows where an unseen attacker stands".
* `docs/STATUS.md:1358`: "The on-screen member of the same class (P2-1 …) is fixed".

**Evidence:** probe `projectile_from_unseen_attacker` (release, fixture data). It computes, with the same interpolation as `fog.rs`, the points the game draws.

* **Setup:** a human crawler (sight 2) stands at (10, 16). An unseen enemy tank at (14, 16) fires a shell (speed 128) at it.
* **Points drawn:**
  * the shell's points at x 3200, 3072, 2944, and 2816 (cells (12, 16) and (11, 16)) are visible and drawn;
  * the origin cell (14, 16) is not visible.
* **What the player learns:** the drawn dots move along y = 4224 by exactly 128 subcells per tick, on the straight line from the hidden tank's centre. The player sees the bearing of the unseen attacker, as review 1 P2-1 found for instant shots.
* **Accuracy of the records:** D31 item 3 ("Projectiles are drawn … only in cells the human player sees") is accurate. D33's general claim, and the report's statement that the class is fixed on screen, are not.

**Severity rationale:** P2.

* This is documentation accuracy, plus a minor information leak that is conventional in the genre.
* Simulation state, command validation, and invariants are unaffected.
* The precedent is review 1 P2-1, rated P2.

**Expected behaviour:** either of the following.

* Correct D33 (title and item 4) and the remediation report to state that projectiles in flight are drawn in visible cells, so their line of flight shows the bearing of an unseen attacker, and record it as accepted behaviour.
* Or draw a projectile only once its attacker has been seen, or not before it is within a bounded distance of a visible target. A unit test of that decision would accompany this option.

### Recorded debt to carry (verbatim from review 2)

**New in this review (P2):**

* **P2-1:** projectiles from unseen attackers show their line of flight, and the D33 title, D33 item 4, and the remediation report's claim are too strong.

**From the M4 remediation round 1 report (known debt), confirmed by this review:**

1. Outcomes of accepted orders use the true state: group destinations, paths, and harvester ore claims (D33 item 3).
2. The `SimEvent` buffer is one stream for all players. The game's terminal log writes every event of both players.
3. Placement is limited to cells the player sees (D33 item 5). There is no validity preview until M5.
4. An ordered attack on an undamageable target fires one harmless shot before it ends (D32 item 3). It is confirmed for instant and projectile weapons.

**From the M4 implementation report, confirmed by review 1 and unchanged except as noted:**

1. **Cost per tick.** Acquisition is O(A × T), a chase runs a Dijkstra, and vision is recomputed in full. Review 1 measured 300 per side at a mean of 1.09 ms and a max of 2.84 ms in release. To be measured against the M7 benchmark.
2. **Projectile bound.** It allows about 200 MB in the pathological case.
3. **No chase or return fire beyond range.** Idle and attack-moving units do not chase, and do not return fire against attackers that outrange them.
4. *Resolved by D32.* (This was the `Attack` on an undamageable target that lasted while the target stood.)
5. **Second `MoveCompleted`.** An attack-move that fights at its destination writes a second `MoveCompleted`.
6. **Unreachable missing-entry cases.** A missing weapon or percentage is treated as no damage (now also in `Field::damages`), and a projectile whose weapon is missing as expired.
7. **`with_economy` order.** `Definitions::with_economy` removes combat definitions loaded before it. This is documented and tested.
8. **Health bars and shot events.** Health bars are M5's. The two-tick `MessageReader` window also applies to instant shots.
9. **No combat in the smoke test.** The smoke test does not exercise combat in the real binary, and there is no automated screenshot.
10. **Pending free units.** M3 review 1 P2-1 is unchanged for selling; destruction reports the lost units.
11. **Earlier debt.** Everything carried at the M3 review decision, unchanged.

**Carried from M3 review 1, unchanged by M4:**

* P2-1, narrowed as in item 10 above;
* P2-2: dock contention;
* P2-3: `docs/DECISIONS.md` still cites the nonexistent `two_placements_in_one_tick_see_each_other`;
* P2-4: `STARTING_CREDITS` and the starting setup in game code, which M4 extends with the opponent's gun turret;
* M3 implementation debt items 1 to 9.

**Carried from M2, M1, and M0, unchanged by M4:**

* M2:
  * review 3: P2-1 and P2-2;
  * remediation round 2: items 1 to 7;
  * review 1: P2-2 remainder.
* M1:
  * review 3: P2-1 (FIFO hang of the map and `.tsx` reader), P2-2, and the remaining items of P2-3;
  * review 1: P2-3 and P2-5;
  * review 2: P2-B (the `f32` in `map_subset.rs:524`);
  * round 1: item 4;
  * round 2: items 5 and 6.
* M0: items 1, 2, 3, 5, 7, 8, and 9. This review saw the linker warning and the `Skipped event Destroyed` warning again.

**Resolved and confirmed in this review:** review 1 P1-1, P1-2, P2-1 (instant shots), P2-2, and P2-3; M4 implementation debt item 4.

### Requires user verification (blocks only `m4-approved`; verbatim from review 2)

These are not findings and do not affect the decision (PLAN.md section 12 item 2); they block only `m4-approved`. Run `cargo run --release -p game` and check the M4 items from docs/STATUS.md, as amended by the remediation report:

1. **Shroud:** at the start, only the area around the player's units and construction yard shows the map; everything else is black.
2. **Fog:** cells seen before and not now are drawn dark but recognisable; cells seen now are drawn normally; moving units uncover the map.
3. **Hidden enemies:** the opponent's units, yard, and gun turret at (50, 41) are not drawn until a unit of the player comes close, and they disappear again when no longer seen.
4. **Attack:**
   * A right click on a visible enemy makes the selected armed units close in and fire.
   * Tank shells are drawn as small yellow dots.
   * Scout shots are drawn as short pale lines only when both the scout and its target are on visible cells; otherwise a small pale circle is drawn at the visible end.
   * Destroyed units and buildings disappear, and units can walk over a destroyed building's cells.
5. **Attack-move:** `A` over a cell makes the selection fight enemies in range on the way, then continue.
6. **Automatic fire:** idle units and turrets fire only at visible enemies in range that their weapon can damage; units on a plain move do not fire.
7. **Gun turret:**
   * the opponent's turret fires at the player's units within about six cells;
   * the player's turret (key `6` after a power plant, placed with `B`) fires while the player has power and holds fire under `(low)`.
8. **Menu keys:** `7` light tank, `8` scout, `9` harvester.
9. **Placement under fog:** `B` is rejected ("cell (x, y) is not visible to the player") while any footprint cell is not currently visible, and succeeds once every cell is seen.

The ten M3 items, the seven M2 items, and the five M1 items listed in docs/STATUS.md remain pending.

## M5 implementation report (2026-10-04)

Branch `milestone/m5`, based on `main` at the merge of M4 (tag `m4-reviewed`). Commits: `defa3e0` (sim: `check_placement` and its test), `ac45883` (game: simulation host, sidebar, minimap, health bars, hotkeys, control groups, pause, audio, smoke test), `b55afed` (game: match setup test; sidebar clicks kept out of the picking log; minimap drags), and the documentation commit that adds this report. Decisions: DECISIONS "M5: interface and audio" (A1 follow-up, D34 to D39), the M5 amendment of D6, and notes on D27 item 4 and D31 item 4.

### Files changed

| File | Change |
|------|--------|
| `crates/sim/src/economy_commands.rs`, `crates/sim/src/lib.rs` | `pub fn check_placement`: the checks of `PlaceBuilding`, which its validation now calls (D35 item 5). No other `sim` change. |
| `crates/sim/tests/m5_placement_preview.rs` (new) | 2 tests through `SimPlugin` and `CommandQueue` with fixture data |
| `crates/game/src/sim_host.rs` (new) | `SimHost` (the simulation's own world, read-only to the rest of the game), `CommandSink`, match setup, the tick system, event copying (D34, D39); 5 tests |
| `crates/game/src/sidebar.rs` (new) | Sidebar UI, build tabs, item status and button effects, pause banner, volume buttons (D35); 6 tests |
| `crates/game/src/minimap.rs` (new) | Minimap layout, drawing from what the human player may know, camera frame, minimap clicks (D36); 6 tests |
| `crates/game/src/health.rs` (new) | Health bars (D37); 2 tests |
| `crates/game/src/hotkeys.rs` (new) | The hotkey table and control groups (D35 items 4 and 6); 3 tests |
| `crates/game/src/audio.rs` (new) | Synthesised effects and music, the `Pcm` audio source, what plays, volume settings (D38); 5 tests |
| `crates/game/src/construction.rs` | Interface modes (place, sell, rally), placement preview and ghost sprite, hotkeys at the cursor; the interim keys `1`-`9` removed (D35); 3 new tests |
| `crates/game/src/selection.rs` | Reads through `SimHost`, pushes through `CommandSink`; control groups; ignores the sidebar and the modes; ore as the map shows it (D36 item 3) |
| `crates/game/src/{units,buildings,fog,map}.rs` | Read through `SimHost`; match setup split into `load_match_assets` (game side) and `sim_host::start_match`; `MapTerrain`; hovered cell `None` over the sidebar; title ordered before the camera |
| `crates/game/src/camera.rs` | `clamp_centre_beside`: the view beside the sidebar stays inside the map (D35 item 7); 1 new test |
| `crates/game/src/smoke.rs` | Orders through the interface's functions and `CommandSink`; checks of the sidebar, minimap, sounds, music, and pause (D6 amended); 2 new tests |
| `crates/game/src/main.rs` | `#![forbid(unsafe_code)]`; `SimHostPlugin` replaces `SimPlugin` and `run_sim_tick`; the M5 plugins |
| `assets/LICENSES.md` | The embedded FiraMono font (third-party, OFL 1.1) and the synthesised sounds and minimap |
| `docs/DECISIONS.md`, `docs/STATUS.md` | M5 decisions; this report |

`Cargo.toml` files and `Cargo.lock` are unchanged (no dependency added, I9), so `sim`'s dependency tree is unchanged (I1). No existing test was removed, skipped, or weakened; no existing assertion changed. The existing construction tests keep their names and assertions although the keys `1`-`9` no longer queue production (they test `menu_command`, which the sidebar uses). One existing test module gained imports only (`construction::tests`: `CommandQueue`, `IssuedCommand`, `Players`, which it used to reach through the module's own imports, now removed with the interim keys).

### Validation commands (PLAN.md section 9, with section 12 item 5)

Run from the repository root on macOS (Apple M5, Rust 1.98.1), in this order, in the foreground, after the last code change (`b55afed`). A display and an audio device were available.

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 342 passed, 0 failed, 0 ignored (breakdown below). Peak memory footprint of the run 0.13 GB. Known M0 linker warning only. |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | `3 buildings spawned`, `26 units spawned`; `smoke test: 600 frames, 188 simulation ticks, map created: true, units spawned: 26, units moved: 18 in 188 ticks after the order, buildings spawned: 3, production charged 376 credits in 188 ticks, visible cells: 391, enemy views shown: 0, view mismatches: 0, sidebar items: 9 of 9, sidebar buttons outside the window: 0, sidebar credits: Some("Credits  4624"), minimap drawings: 189, minimap pixels lit: 2260, sounds: 9, music players: 1, pause: Done { ticks_while_paused: 0, refused: true }`; peak memory footprint 0.14 GB |
| `cargo run -p game -- --smoke-test 600` | 0 | `smoke test: 600 frames, 195 simulation ticks, ..., production charged 390 credits in 195 ticks, visible cells: 391, enemy views shown: 0, view mismatches: 0, sidebar items: 9 of 9, sidebar buttons outside the window: 0, sidebar credits: Some("Credits  4610"), minimap drawings: 192, minimap pixels lit: 2260, sounds: 9, music players: 1, pause: Done { ticks_while_paused: 0, refused: true }`; peak memory footprint 0.14 GB |

Test breakdown for `cargo test --workspace`:

* `game`: unit 101 (M4: 68; + sim_host 5, sidebar 6, minimap 6, audio 5, hotkeys 3, health 2, construction 3, smoke 2, camera 1); integration `m0_spike` 6, `m1_demo_map` 3, `m1_tileset_image` 6, `m2_unit_sprites` 5, `m3_building_sprites` 3.
* `sim`: unit 11; integration `m0_bootstrap` 12, `m1_map` 32, `m1_map_subset` 24, `m2_definitions` 14, `m2_movement` 37 (39.2 s), `m3_definitions` 11, `m3_economy` 21, `m3_harvest` 10, `m4_combat` 32, `m4_definitions` 12, `m5_placement_preview` 2 (2.0 s).
* doc-tests 0.

The `Skipped event Destroyed` warning (M0 debt item 2) appeared in both smoke runs. Additionally, the release binary was run (`--smoke-test 100`, 60 s alarm) against copies of `assets/` selected with `BEVY_ASSET_ROOT`, to check the restructured startup: the unchanged copy exits 0; without `combat.ron` it exits 1 with `data file …/combat.ron: file: cannot read the file: No such file or directory`; with a demo map of 9 bytes of text it exits 1 with `map …/demo.tmx: file: not a valid Tiled map: text before the root element is not supported`. M5 adds no external input file (no audio, font, or settings file is read), so there is no new hostile-input surface to probe.

### Acceptance criteria and required tests (PLAN.md section 7, M5)

| Criterion | Evidence |
|-----------|----------|
| Scope: sidebar with credits and power | `sidebar` (D35 item 1); `item_status_follows_the_players_buildings_and_queue` (`Credits  4700`, `Power  0 / 20` after a power plant through the simulation), `texts_and_colours_of_the_sidebar`; smoke runs: 9 of 9 item buttons, none outside the window, the credits text equal to the player's credits. On screen **requires user verification** |
| Scope: build tabs and queue progress | `Tab`, badges and progress bars; `item_status_follows_the_players_buildings_and_queue` (locked items, `50% +1` at half of the head with one more queued, `READY`), `item_buttons_queue_cancel_and_start_placement`, `mode_tab_pause_and_volume_buttons` |
| Scope: placement preview with validity colouring | `sim::check_placement` (D35 item 5); `m5_placement_preview::the_placement_preview_predicts_the_outcome_of_place_building_at_every_origin` (262 origins: accepted and every placement rejection, including `NotVisible` under fog; preview equals the command's outcome; the state hash unchanged by the preview), `the_preview_of_a_building_that_is_not_ready_is_not_ready`; game: `the_placement_preview_is_the_commands_validation_for_the_human_player`, `modes_follow_keys_clicks_and_the_ready_building` (red footprint: no command, mode kept; problem cell). On screen **requires user verification** |
| Scope: minimap with fog and camera rectangle | `minimap` (D36); `the_layout_fills_the_image_with_the_maps_bounding_rectangle` (1×1, 1×37, 37×1, 64×64, 256×256), `every_pixel_shows_the_cell_under_it`, `the_minimap_shows_shroud_fog_and_only_shown_units`, `the_demo_match_shows_no_enemy_the_human_player_does_not_see`, `an_enemy_is_marked_exactly_while_the_human_player_sees_it` (edge of sight and one cell beyond), `the_camera_frame_is_the_visible_part_of_the_view`; smoke runs: drawn every tick, 2,260 lit pixels. On screen **requires user verification** |
| Scope: health bars | `health` (D37); `the_bar_shows_the_fraction_of_hit_points_left`, `bars_are_drawn_only_over_shown_views_of_damaged_or_selected_entities`. On screen **requires user verification** |
| Scope: control groups | `hotkeys::ControlGroups`; `digit_keys_select_add_and_assign_control_groups`, `control_groups_assign_select_add_and_forget_dead_units` (groups 0 and 9, empty selection, pruning) |
| Scope: hotkeys | `hotkeys::hotkey`; `every_listed_key_has_one_action` (every listed key, unlisted keys, Ctrl or Cmd with letters) |
| Scope: pause | D39; `the_sink_accepts_the_human_players_commands_until_paused_or_full`, `the_tick_system_does_not_tick_while_paused_and_copies_the_events`; smoke runs pause 30 frames: 0 ticks, a command refused; `a_run_passes_only_if_the_pause_stops_the_ticks_and_refuses_commands` |
| Scope: sound effects and music | D38; `every_sound_is_synthesised_within_bounds_and_the_same_on_every_run`, `the_decoder_hands_out_the_samples_unchanged`, `events_play_only_what_the_human_player_may_know` (every `SimEvent` variant, both players, seen and unseen), `effects_are_limited_in_number_and_repetition`; smoke runs: 9 sounds, 1 music player. Audible result **requires user verification** |
| Scope: volume settings | `VolumeSettings`; `volume_steps_stay_within_their_range` (0 and 10, beyond the range, mute) |
| Test: the functions that map interface actions to `Command` values are pure and unit tested for every command variant | `construction::tests::every_command_variant_is_built_by_a_pure_interface_function`: all ten variants, each from the function the interface calls (`selection::{right_click_commands, attack_commands, attack_move_command, stop_command}`, `sidebar::button_effect`, `construction::mode_click`), checked with a `match` without wildcard so that a new variant does not compile until it is mapped; plus the earlier per-function tests (`orders_are_commands_for_the_whole_selection`, `a_right_click_on_ore_sends_the_selected_harvesters_to_harvest`, `a_right_click_on_a_visible_enemy_orders_the_armed_units_to_attack_it`, `the_attack_move_key_orders_the_whole_selection`, `menu_keys_queue_and_cancel_the_menu_entries`, `the_ready_building_is_placed_centred_on_the_cursor`, `sell_and_rally_keys_name_the_players_buildings`, `item_buttons_queue_cancel_and_start_placement`, `modes_follow_keys_clicks_and_the_ready_building`) |
| Test: the interface cannot obtain mutable access to simulation state (enforced by the type or module boundary) | D34: the simulation's `World` is a private field of `SimHost`, reachable only as `&World`; no public constructor or `&mut self` method; `#![forbid(unsafe_code)]`; the game's world holds no simulation state. `sim_host::tests::the_game_world_holds_no_simulation_state` (the game world has no `Players`, `Grid`, `Definitions`, `CommandQueue`, `SimTick`, or `Projectiles`; a `&mut Health` query on the host's `&World` yields read-only items). The enforcement itself is the compiler's (privacy and borrowing); a test cannot show that code which does not compile does not exist. |
| Test: the smoke test passes with the full interface loaded | Both smoke runs above, with every M5 plugin and the M5 checks of D6 |
| Acceptance: a full match can be played by a human without debug shortcuts, confirmed by the user | Every action of a match is available from the sidebar and the mouse (production, placement, selling, rally points, orders, attack, pause, volume), with hotkeys as shortcuts; the interim keys of D27 are gone. The opponent stays passive until the AI of M6. **Requires user verification** (below) |
| Section 8: negative, boundary, integration, end to end | Negative: refused commands (paused, full sink), red footprints, locked items, unknown items; boundary: sink capacity 256 exactly, groups 0 and 9, volume 0 and 10, health fraction at 0, maximum, beyond, `MAX_HIT_POINTS`, minimap of 1×1 and 256×256 maps, the edge of sight on the minimap, placement origins over every edge and at `i32` extremes, the camera inset wider than the view; integration: `m5_placement_preview` and the host tests through `SimSchedule`; end to end: both smoke runs |
| Invariants | I1/I9: no dependency added. I2: `sim` gained no float (`check_placement` is a moved sequence of integer checks). I3/I4: `sim` behaviour unchanged (all `sim` tests pass unchanged); the game's systems are ordered (`the_game_systems_share_no_state_without_an_order`). I5: enforced by `SimHost` (D34). I8: no new data file; setup errors still end the game with exit code 1 (probes above). I11: no new external file is handed to a parser or decoder (D38). I12: no new simulation arithmetic. |

### Assumptions verified and how

| Item | Verification |
|------|--------------|
| Bevy 0.19.1 APIs of the A1 follow-up table in DECISIONS (M5): `World::try_query`, `QueryState::iter(&World)`, `World: Send + Sync + Default`, `App::empty`, `MessageWriter::write_batch`, UI, text, image, and audio APIs | Read in the sources (paths in DECISIONS) and exercised by code that compiles and runs in the tests and the smoke runs |
| A `&World` cannot hand out mutable state | `World`'s mutable accessors take `&mut self`; `as_unsafe_world_cell_readonly` needs `unsafe` to write, which the game crate forbids; `QueryState::iter` yields `D::ReadOnly` (test above) |
| Bevy's audio playback accepts a custom `Decodable` | `bevy_audio` `AddAudioSource::add_audio_source`, `play_queued_audio_system::<T>`; the smoke runs spawn the music player and no audio error is logged |
| No system font is read | `cargo tree -p game -e features -i parley`: `parley` with feature `std` only; `bevy_text`'s `system_font_discovery` is not enabled |
| `RelativeCursorPosition::normalized` is relative to the node's centre, (−0.5, −0.5) at the top left | `bevy_ui-0.19.1/src/focus.rs:263-271` |
| No new crate | `git diff 386d8af -- Cargo.lock crates/*/Cargo.toml` is empty |

### Deviations from PLAN.md

1. **Simulation in a world of its own.** The simulation's resources and entities no longer live in the game's `World` but in `SimHost` (D34). PLAN.md section 4 describes the path (the game reads simulation state without mutating it, input pushes commands into `CommandQueue`); this is how M5 enforces it. Tests still run `SimSchedule` directly.
2. **Audio is synthesised, not stored.** No file is added under `assets/audio/` (PLAN.md section 4 lists the directory for media); the sounds and music are generated in memory at startup (D38).
3. **Volume settings last for the session only** (options are M7's, D38 item 5).
4. **Rally points** are set for every unit-producing building of the player at once (there is no building selection); this was already the interim rule of D27 item 4.
5. **Pause refuses commands** until the game resumes (D39); the plan does not say whether orders may be given while paused.
6. **The interim keys of D27 item 4 are replaced:** the digits are control groups; production is queued from the sidebar; `B` enters placement mode with a preview instead of placing at once (D35).
7. **Terrain shown by the interface is the map file's** (D36 item 3), as the tiles draw it; current ore depletion is not shown.
8. **Files not listed in PLAN.md section 4:** `crates/game/src/{sim_host,sidebar,minimap,health,hotkeys,audio}.rs`, `crates/sim/tests/m5_placement_preview.rs`.

### Known debt (P2 proposals for the reviewer)

1. The interface's wiring in the window (button clicks, right clicks on buttons, the minimap click and drag, the placement ghost, health bar placement, sound) is tested through pure functions, the conflict test, and the smoke runs, not by an automated test of user input in the window (as M1 review 1 P2-3 for the camera); it is part of the user verification below.
2. The sidebar has a fixed width (240 logical pixels) and is laid out for the default 1280 × 720 window; the smoke test checks that every displayed button lies inside that window, but in a much smaller window the lower buttons would be clipped.
3. There is no building selection: rally mode sets the rally point of every unit-producing building, and the sidebar shows no per-building information.
4. Queueing while the queue is full (16) is not shown as unavailable in the sidebar; the simulation rejects it with `QueueFull` (logged, error sound).
5. Effects are limited to 8 at once and one start per effect per 80 ms, so in large battles most shots are not heard (by design, D38 item 4). A changed effects volume applies to effects started afterwards.
6. Each frame the game builds a few `QueryState`s on the host's world (`World::try_query`), and the minimap is redrawn every tick (at most 131,072 pixels for the largest map). To be measured against the M7 benchmark.
7. Control groups have no "centre the camera" on a double press.
8. Unchanged and still relevant to the interface: the terminal event log writes every event of both players (M4 remediation debt item 2); M2 review 3 P2-2 (a box selection released outside the window becomes a click), which M5 did not touch beyond the sidebar check.

Resolved by M5: M2 implementation debt item 2 (the two-tick `MessageReader` window, D34 item 5); M2 implementation debt item 5 and M3 implementation debt item 7 as far as the interface is concerned (the setup APIs stay public in `sim`, but the game can reach the simulation's world only through `sim_host`, D34); M3 implementation debt items 5 (no validity feedback before a placement) and 8 (the title as the only display of credits and power); M4 implementation debt item 8 (health bars); the "no validity preview until M5" part of M4 remediation debt item 3 (placement itself stays limited to visible cells by design, D33). Carried unchanged: every other item of "Recorded debt to carry" of the M4 review decision above, including M4 review 2 P2-1 (projectile line of flight). The `Skipped event Destroyed` warning (M0 item 2) and the linker warning (M0 item 1) were seen again.

### Requires user verification

Not review findings (PLAN.md section 12 item 2); they block only `m5-approved`. Run `cargo run --release -p game` (with sound on). The 16 M5 items:

1. **Sidebar:** a dark panel on the right, 240 px wide: the minimap at the top, `Credits  5000`, `Power  0 / 20`, two tabs (Buildings, Units), a button per item with its name and cost (e.g. `power plant  $300`), Sell, Rally, and Pause buttons, Master, Music, and Effects volume rows with `-` and `+`, a Mute button, and a hint line at the bottom. All text is legible.
2. **Production:** a left click on `power plant` starts it: its badge counts up in percent, a green bar fills, and the credits fall; at the end it shows `READY` and a chime plays. A right click on an item with entries queued cancels the most recent one and refunds what was charged. Items whose prerequisites are missing show `locked` and do nothing.
3. **Units tab:** `Tab` or the Units tab shows scout, light tank, and harvester, locked until a barracks or war factory (and refinery for the harvester) is owned; produced units leave their building.
4. **Placement preview:** a left click on a `READY` building (or `B`) shows the building's sprite at the cursor, green where it may be placed and red where not (sand, rock, water, ore, units, more than 4 cells from an own building, cells not currently visible), with the offending cell crossed out; a left click on green places it (a thud plays); a right click or `Escape` leaves the mode without cancelling the building.
5. **Selling:** the Sell button or `X`, then a left click on an own building sells it (refund, jingle); the mode stays until right click or `Escape`. `Delete`/`Backspace` over an own building still sells it at once.
6. **Rally points:** the Rally button, then a left click on a cell, makes later produced units walk there; `R` over a cell does the same.
7. **Minimap:** the whole map as a diamond, oriented like the main view; black where never seen, dark where seen before, normal colours where seen now; own units and buildings blue, enemies red only while seen; a white frame shows the part of the map visible beside the sidebar and follows panning and zoom; a left click or drag on the minimap moves the view there.
8. **Camera beside the sidebar:** panning to the right edge of the map shows its rightmost tiles left of the sidebar; with the cursor over the sidebar no cell outline is drawn and clicks do not select or order.
9. **Health bars:** a short bar appears above damaged units and buildings (green, yellow below half, red below a quarter) and above selected units; enemies hidden by the fog never show a bar.
10. **Control groups:** select units, press Ctrl+1 (or Cmd+1); select others; press 1 to select the group again, Shift+1 to add it to the selection; destroyed units leave their group.
11. **Hotkeys:** `S` stop, `A` attack-move to the cursor, `B`, `X`, `R`, `Delete`/`Backspace`, `Escape`, `P`, `Tab`, `-`/`=` (master volume), `M` (mute) behave as described in items 4 to 14 and in docs/DECISIONS.md D35 item 4.
12. **Pause:** `P` or the Pause button shows `PAUSED` over the map and freezes the simulation (units, production, combat); orders and sidebar production clicks are refused (an error buzz plays); the camera, selection, and sidebar still work; `P` again resumes.
13. **Sound effects:** a short click when an order is accepted, a buzz when a command is refused or rejected, a chime when production completes, a thud when a building is placed, a jingle when one is sold, a ding when ore is delivered, gunfire and cannon shots and explosions only where the player's units see the fight; nothing is heard of the opponent's production or of fights in the fog.
14. **Music:** a calm loop plays from the start and repeats without a gap or click; the Music `-`/`+` buttons change its volume at once, Effects `-`/`+` change later effects, Master and `-`/`=` both, and Mute silences everything.
15. **Full match (acceptance):** without any debug key, build a power plant, refinery, barracks, and war factory from the sidebar, harvest ore, produce an army, and destroy the opponent's units, gun turret, and construction yard. (The opponent does not act until the AI of M6.)
16. **Window title:** still shows the credits and power, consistent with the sidebar.

The nine M4 items, the ten M3 items, the seven M2 items, and the five M1 items listed in docs/STATUS.md remain pending.

## M5 review decision (2026-10-04)

One independent review round (`docs/reviews/M5-review-1.md`: GO WITH DOCUMENTED NON-BLOCKING DEBT, 0 P0, 0 P1, 5 P2). Before the review the orchestrator reran all six validation commands of PLAN.md section 9 on commit `522f70b` with exit code 0 (342 tests passed). Under PLAN.md section 12 item 1, `milestone/m5` was merged into `main` and tagged `m5-reviewed`. The tag `m5-approved` is created by the user alone after the verification below.

### Debt recorded from review 1 (P2, verbatim from the report)

#### P2-1: A right click on the sidebar or minimap can order an attack on an enemy drawn behind the sidebar
* **File and line:**
  * `crates/game/src/selection.rs:353-357`: `issue_orders` gates the right click only on `*mode == InterfaceMode::Normal`. It then computes `cursor_world(&window, …)` from the raw cursor position (`selection.rs:223-224`) and calls `enemy_at(point, &shown_enemy_units(&views), building)`.
  * There is no `over_sidebar` check. `over_sidebar` is applied only in `map.rs:222`, to the hovered cell; in `map.rs:246-249`, to the picking log; and in `selection.rs:269`, to the left press.
* **Evidence:**
  * The camera draws the map under the whole window, and the sidebar is a 0.96-alpha UI overlay. An enemy unit view standing behind the sidebar therefore keeps `Visibility::Inherited` and is in `shown_enemy_units`.
  * Suppose armed units are selected and the player right-clicks a sidebar item to cancel production (`sidebar.rs:610-616`) or right-clicks the minimap, and that point lies over such an enemy's body. Then `enemy_at` returns the enemy, and `attack_commands(&selection.0, &armed, target, None)` pushes `Command::Attack` into the sink (the acknowledgement sound plays) alongside the `CancelProduction`.
  * This contradicts D35 item 7 ("clicks on the sidebar never act on the map", `docs/DECISIONS.md:556`) and user verification item 8 ("with the cursor over the sidebar … clicks do not select or order", `docs/STATUS.md:1736`). No test covers right clicks over the sidebar.
* **Expected behaviour:** a right click whose cursor is over the sidebar issues no map order. Gate `issue_orders` (and `cursor_world`) with `over_sidebar`, or derive the enemy from a cursor point that is `None` over the sidebar. Add a test of that decision, for example a pure function that takes the over-sidebar flag.
* **Severity rationale:** P2. Simulation state, validation, and the invariants are unaffected: the command passes through `CommandSink` and is a valid, visible-target order. The effect is an unintended user-facing order, and an inaccurate claim in D35 and the report.

#### P2-2: On maps with width + height > 128 the minimap never draws half of all cells, so units and buildings on them are not marked

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

#### P2-3: A placement or rally click made while paused, or refused by a full sink, leaves the mode although nothing was issued

* **File and line:** `crates/game/src/construction.rs:381-391`. `mode_click` returns `InterfaceMode::Normal` for a valid placement or rally click. Line 386 assigns `next = after` before line 391 calls `sink.issue(commands)`, whose result is ignored. `CommandSink::push` refuses every command while paused (`sim_host.rs:222-225`, D39).
* **Evidence:**
  * While paused, a left click on a green footprint in placement mode, or on a cell in rally mode, ends the mode. The command is refused and logged with the error sound, but the player must re-enter the mode. The building stays ready, so no state is lost.
  * D39 says only that commands are refused while paused; it does not say that modes end.
  * No test covers this interaction, because `mode_click` is tested without the sink.
* **Expected behaviour:** the mode changes only if the sink accepted the command (use `sink.issue`'s count), or modes are kept while paused. A test of the decision should accompany the fix.
* **Severity rationale:** P2 (user experience; no data loss).

#### P2-4: Pre-existing M4 test race, found in this review: four `m4_combat` tests write and read the same temporary file concurrently

* **File and line:** `crates/sim/tests/m4_combat.rs:452-471`. `extreme_scenario()` writes `CARGO_TARGET_TMPDIR/m4_combat/extreme_combat.ron` and reads it back. It is called by four tests (lines 499, 519, 544, and 572), which the test harness runs in parallel. The race was introduced in `9608923` (M4) and is unchanged by M5.
* **Evidence:**
  * `cargo test --release -p sim --test m4_combat` failed in 3 of 16 runs, each time in one of `the_longest_range_with_the_slowest_projectile_flies_32_ticks` or `the_shortest_range_reaches_every_neighbouring_cell_and_an_adjacent_building`, with `panicked at crates/sim/tests/m4_combat.rs:477:29: data file …/target/tmp/m4_combat/extreme_combat.ron: file: line 1, column 1: expected a struct "(", found the end of the file`. Another test had truncated the file while this one read it.
  * Debug: 0 of 20 runs failed, and the section 9 command `cargo test --workspace` passed here.
  * All writers write identical text, so the race can only cause spurious failures, never a false pass.
* **Expected behaviour:** each call writes its own file, for example named per test (as `combat_variant(name, …)` does) or with a unique suffix, so that `cargo test` is deterministic in both profiles.
* **Severity rationale:** P2. It concerns test reliability; no assertion is weakened and production behaviour is unaffected. It is reported because it is not in the recorded debt and can make the validation command fail spuriously.

#### P2-5: D34 overstates what a `&World` withholds: the simulation world's entity allocator is reachable through `SimHost::world()`

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

### Recorded debt to carry (verbatim from review 1)

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

### Requires user verification (blocks only `m5-approved`; verbatim from review 1)

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

## M6 implementation report (2026-10-04)

Branch `milestone/m6`, based on `main` at the merge of M5 (tag `m5-reviewed`). Commits: `3279e84` (sim: skirmish and AI files, match setup, win condition, the computer player, tests and fixtures, the shipped data files; the game only gains log lines for the two new events), `30ea562` (game: catalogue, menu, match flow, end screen, restart, smoke test, tests), and the documentation commit that adds this report. Each commit builds. Decisions: DECISIONS "M6: skirmish AI and game flow" (A1 follow-up, D40 to D44, notes on D6, D9, D20, D27, D31, D34).

### Files changed

| File | Change |
|------|--------|
| `crates/sim/src/skirmish_defs.rs` (new) | Skirmish file schema 1: starting credits, factions (starting buildings and unit groups with offsets), maps (ids, two start positions); limits, schema, validation; `map_asset_path` (D40) |
| `crates/sim/src/ai_defs.rs` (new) | AI file schema 1: build order, power building, harvester, army, difficulties with seven parameters; limits; cross checks (prerequisites earlier in the build order, power, harvester, armed army that can damage every building) (D42) |
| `crates/sim/src/match_setup.rs` (new) | `setup_match`, `PlayerSetup`, `Controller`, `MatchStart`, `MatchSetupError`; `plan_starting_units` and `StartError` (moved from `game::units`); `MatchState`, `MatchOutcome`, the `check_win_condition` system (D40, D41) |
| `crates/sim/src/ai.rs` (new) | `AiPlayers`, `AiController`, the `run_ai` system: what a computer player may see (`View`), placement, build order with power and rebuilding, harvesters, army composition, rally points, defence, waves with search and fall-back, selling when broke (D43) |
| `crates/sim/src/{lib,schedule,event,hash,command,economy_defs}.rs` | Exports; `run_ai` in `Ai`, `check_win_condition` in `WinCondition`, `MatchState` and `AiPlayers` resources; `SimEvent::{PlayerDefeated, MatchEnded}`; both resources hashed; `CommandQueue::iter` (read only); validation helpers shared with the new data files |
| `crates/sim/tests/m6_definitions.rs` (new) | 11 tests of both files, bounded as in D15 |
| `crates/sim/tests/m6_match.rs` (new) | 9 tests of setup and the win condition |
| `crates/sim/tests/m6_ai.rs` (new) | 11 tests of the computer player, including the plan's four required tests |
| `crates/sim/tests/fixtures/data/{skirmish,ai}.ron`, `fixtures/maps/skirmish_40x40.tmx` (new) | Fixture data (two factions, two difficulties) and a point-symmetric 40×40 map |
| `assets/data/skirmish.ron`, `assets/data/ai.ron` (new) | The shipped skirmish (one faction, the demo map with two starts, 5,000 credits) and AI file (three difficulties) |
| `crates/game/src/skirmish.rs` (new) | Catalogue loading, `MenuSelection`, `menu_effect`, `end_effect`, `end_text`, `begin_match`, `end_match`, the menu and end screen (D44); 6 tests |
| `crates/game/src/sim_host.rs` | `SimHostPlugin` without a seed; `MatchPlan`, `install_match`, `uninstall_match`; `SimHost::{running, outcome, match_state, ai}`; ticks only while running; the sink closes at the end of a match (`CommandRefused::MatchOver`) and is reset for the next; the startup match setup (`start_match`, `MatchGrid`, `MatchDefinitions`) removed; 2 new tests |
| `crates/game/src/map.rs` | `check_map` (the checks of D13 and D16 for any map), `spawn_map`, `despawn_map`, `MapRoot`, `MapAsset`; the startup `load_map` removed; `MAP_PATH` kept for the tests |
| `crates/game/src/{units,buildings}.rs` | The hard-coded starting units, credits, and buildings removed (now data, D40); `load_match_assets` removed (now in `skirmish`); log lines for the new events |
| `crates/game/src/minimap.rs` | `size_minimap` (a new image per match) |
| `crates/game/src/smoke.rs` | The game flow (menu start, restart, menu, start), M6 checks, early-exit failure; 2 new tests |
| `crates/game/src/{main,audio,construction,sidebar}.rs` | Plugins and startup ordering; no sound for the new events; two systems made visible to the crate for ordering |
| `crates/game/tests/m6_production_matches.rs` (new) | Computer players with the shipped data on every shipped map, every difficulty pairing |
| `assets/LICENSES.md`, `docs/DECISIONS.md`, `docs/STATUS.md` | Provenance of the two data files; decisions; this report |

`Cargo.toml` files and `Cargo.lock` are unchanged (no dependency added, I9), and `cargo tree -p sim -e normal` contains no rendering, windowing, audio, or input crate (I1).

### Validation commands (PLAN.md section 9, with section 12 item 5)

Run from the repository root on macOS 27.0.1 (Apple M5, Rust 1.98.1), in this order, in the foreground, after the last code change (the tree of `30ea562`). A display and an audio device were available.

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 384 passed, 0 failed, 0 ignored (breakdown below). Peak memory footprint of the run 0.13 GB. Known M0 linker warning only. |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | three matches started (`match on map demo (seed 6)`, `(seed 7)`, `(seed 6)`: 2 buildings and 16 units each); `smoke test: 600 frames, 168 simulation ticks, map created: true, units spawned: 16, units moved: 8 in 168 ticks after the order, buildings spawned: 2, production charged 336 credits in 168 ticks, visible cells: 370, enemy views shown: 0, view mismatches: 0, sidebar items: 9 of 9, sidebar buttons outside the window: 0, sidebar credits: Some("Credits  4664"), minimap drawings: 169, minimap pixels lit: 2216, sounds: 9, music players: 1, pause: Done { ticks_while_paused: 0, refused: true }, menu buttons: Some(10), matches started: 3, flow: Last, views: 16 of 16 units and 2 of 2 buildings, computer player's commands: 2` |
| `cargo run -p game -- --smoke-test 600` | 0 | the same three matches; `smoke test: 600 frames, 179 simulation ticks, ..., units moved: 8 in 179 ticks after the order, ..., production charged 358 credits in 179 ticks, visible cells: 365, ..., minimap drawings: 179, minimap pixels lit: 2232, ..., menu buttons: Some(10), matches started: 3, flow: Last, views: 16 of 16 units and 2 of 2 buildings, computer player's commands: 2` |

Test breakdown:

* `game`: unit 111 (M5: 101; + skirmish 6, sim_host 2, smoke 2); integration `m0_spike` 6, `m1_demo_map` 3, `m1_tileset_image` 6, `m2_unit_sprites` 5, `m3_building_sprites` 3, `m6_production_matches` 1 (52.2 s).
* `sim`: unit 11; integration `m0_bootstrap` 12, `m1_map` 32, `m1_map_subset` 24, `m2_definitions` 14, `m2_movement` 37 (41.2 s), `m3_definitions` 11, `m3_economy` 21, `m3_harvest` 10, `m4_combat` 32, `m4_definitions` 12, `m5_placement_preview` 2, `m6_ai` 11 (39.9 s), `m6_definitions` 11, `m6_match` 9.
* doc-tests 0.

The `Skipped event Destroyed` warning (M0 debt item 2) appeared in the smoke runs. An earlier debug smoke run of this session exited with code 0 without its checks: the log showed `Monitor removed` and `No windows are open, exiting` before frame 600, and Bevy's `exit_on_all_closed` exits with success. That run is not counted; the smoke test now fails such a run (D44 item 7, `smoke::tests::an_exit_before_the_last_frame_fails_the_run`), and the debug run above completed all 600 frames.

Additionally, the release binary was run (`--smoke-test 100`, 60 s alarm, `/usr/bin/time -l`) against copies of `assets/` selected with `BEVY_ASSET_ROOT`. The unchanged copy exits 0 (one match, peak memory footprint of the game process 0.41 GB; earlier reports measured the `cargo` process, so the numbers are not comparable). Each of these exits 1 with a typed error naming the file and field, at a peak of 0.14 GB: no `skirmish.ron` (`data file …/skirmish.ron: file: cannot read the file: No such file or directory`); no `ai.ron`; `#![enable(implicit_some)]` prepended to `ai.ron` (`file: line 1, column 1: expected a value or punctuation of the supported subset, found '#'`); `ai.ron` a named pipe (`file: cannot read the file: not a regular file`, at once); `ai.ron` above 64 KiB (`file: the file is larger than 65536 bytes`); a map id without a map (`data file …/skirmish.ron: map "nowhere": map …/maps/nowhere/nowhere.tmx: file: cannot read the file: …`); the map id `../demo` (`maps[0].id: value "../demo" is not supported, expected 1 to 32 characters from a-z, 0-9, and _, starting with a letter`); a group count of 10^20 (`factions[0].units[0].count: value 99999999999999999999 is not supported, expected a 64-bit signed integer`); an army of harvesters (`army[0]: value "harvester" is not supported, expected an armed unit type with production terms`); and a start position at (63, 42), where the yard does not fit (`map demo: starting building "construction_yard" of player 1 at (63, 42): the 3x3 footprint at (63, 42) does not lie inside the map`, when the smoke test starts the match).

Computer players were also measured with probe programs outside the repository (release; D43 item 9): every pairing of the shipped difficulties on the demo map ends with a winner within 23,022 ticks and every difficulty against a passive player within 7,611; every fixture pairing within 5,697 ticks on the fixture map and 20,443 on the demo map; no command of a computer player was rejected except once, explained by the loss of the unit it named in the tick before.

### Acceptance criteria and required tests (PLAN.md section 7, M6)

| Criterion | Evidence |
|-----------|----------|
| Scope: an AI player inside `sim` that issues commands only through `CommandQueue` | `sim::ai::run_ai` (D43 item 1) pushes `IssuedCommand`s and changes no other simulation state; the tests read its commands from the queue after each tick; `I5` holds by construction (the system's only writes are its own resource and the queue) |
| Scope: a data defined build order | `assets/data/ai.ron` `build_order` (D42); `the_ai_completes_its_build_order_within_the_tick_budget` |
| Scope: harvester management | `harvesters_per_refinery`, harvesters queued per refinery, idle harvesters sent back to work with back-off (D43 item 3); `idle_harvesters_without_ore_are_sent_to_work_at_most_once_per_retry_interval` |
| Scope: army composition | `army` cyclic composition up to `max_army` (D42, D43 item 3); exercised by every match test |
| Scope: attack waves | `wave_size`, `first_wave`, `wave_patience`, targets, search, fall-back (D43 item 7); `the_ai_issues_no_rejected_command_in_the_standard_scenario` (the computer player destroys a passive player's base) and the end-to-end matches |
| Scope: base defence | `defence_radius` (D43 item 6); `the_ai_attacks_a_visible_enemy_near_its_base_with_its_army_at_home`; gun turrets in the build order |
| Scope: rebuilding | build order counts (D43 item 3); `the_ai_rebuilds_a_building_it_lost` (a power building destroyed by the test's tanks through commands is placed again) |
| Scope: difficulty set by data parameters | seven parameters per difficulty (D42 item 3); `m6_definitions` tests every limit at both ends; the menu chooses the difficulty |
| Scope: main menu, map and faction selection | `game::skirmish` (D44 items 1 to 3); `menu_buttons_cycle_through_each_row_and_start_only_a_valid_choice`, `the_human_plays_the_first_start_and_the_computer_the_second`, `row_texts_name_the_chosen_entries`, `the_production_catalogue_loads_every_map_faction_and_difficulty`, `every_shipped_match_can_be_set_up_for_every_faction_pairing`; smoke runs start every match from the menu's effect. On screen **requires user verification** |
| Scope: win and lose detection when all buildings of a player are destroyed | `sim::match_setup::check_win_condition` (D41); `a_player_without_buildings_is_defeated_and_the_last_player_wins`, `players_who_lose_their_last_buildings_in_the_same_tick_draw`, `a_destroyed_last_building_defeats_its_owner`, `without_a_match_set_up_there_is_no_win_condition`; every end-to-end match ends this way |
| Scope: end screen, restart | `end_text`, `end_effect`, `begin_match`, `end_match` (D44 items 4 and 5); `the_end_screen_names_the_outcome_and_restarts_the_last_match`, `ticks_run_only_while_a_match_is_running`, `the_sink_refuses_commands_once_the_match_is_over_and_reopens_for_the_next`; the smoke runs restart, return to the menu, and start again, and end with exactly as many views as simulated units and buildings. On screen **requires user verification** |
| Test: on a fixture map the AI completes its build order within a fixed tick budget | `m6_ai::the_ai_completes_its_build_order_within_the_tick_budget`: both fixture factions (one starts with a turret of the build order), both difficulties, budget 1,000 ticks; every building it did not start with placed; no rejection; I6 and I7 on every tick |
| Test: the AI issues zero rejected commands in the standard scenario | `m6_ai::the_ai_issues_no_rejected_command_in_the_standard_scenario` (the standard scenario of D43 item 5, both difficulties, until it wins); the end-to-end tests also assert that every rejection is explained (D43 item 5) |
| Test: an AI with no credits and no harvester neither panics nor floods the queue | `m6_ai::an_ai_with_no_credits_and_no_harvester_neither_panics_nor_floods_the_queue` (two computer players, 0 credits, a yard each, a turn every tick, 3,000 ticks: two commands in all, at most one item queued, no rejection); also `a_broke_ai_without_army_sells_a_building_it_does_not_need_to_fund_units` and the harvester back-off test |
| Test: end to end AI against AI matches on every shipped map finish with a winner within a fixed tick limit, without panic, and with equal hashes across two runs | `m6_ai::ai_against_ai_matches_on_every_shipped_map_end_with_a_winner` (every shipped map of the production skirmish file, every pairing of the fixture difficulties, each run twice: a winner within 36,000 ticks, identical `state_hash` on every tick, every rejection explained, I6 and I7 every 10 ticks); `ai_against_ai_matches_on_the_fixture_map_end_with_a_winner`; `a_run_with_another_seed_or_difficulty_diverges`; with the shipped data, `game/tests/m6_production_matches.rs` (all nine pairings) |
| Acceptance: the user can lose to and defeat the AI in a complete match started from the menu | **Requires user verification** (below) |
| Fog of war (the AI must not use what its player cannot see) | D43 item 2; `the_ai_ignores_what_its_player_does_not_see` (identical commands on all 2,000 ticks with and without an unseen enemy within the defence radius and an unseen enemy building; the building is not remembered) |
| Section 8: negative, boundary, regression, integration, end to end | Negative: every data rejection with file, field, and reason (`m6_definitions`), setup errors with no state change, refused commands after the match; boundary: every data limit at both ends, a turn every tick, 0 credits, the tick budget and limit, the draw in one tick, the early exit at frame 599 of 600; regression: the early-exit hole of the smoke test; integration: all `m6_ai` and `m6_match` tests through `SimPlugin` and `CommandQueue`; end to end: the AI matches and both smoke runs |
| Invariants | I1/I9: no dependency added. I2: no float in `sim` (`ai.rs` uses `i64` arithmetic). I3: `MatchState` and `AiPlayers` hashed; equal hashes on every tick of two runs. I4: ordered collections and id order only; `the_game_systems_share_no_state_without_an_order` with `SkirmishPlugin`. I5: D43 item 1; match setup is setup (D40 item 5). I6, I7: checked in the M6 tests. I8, I11: both new files pass the RON subset before `ron`, all limits checked, probes above. I10: the plan's tests use fixture data; starting credits, buildings, and units are now data. I12: `i64` and saturating arithmetic in the new code. |

### Assumptions verified and how

| Item | Verification |
|------|--------------|
| Bevy 0.19.1 APIs of the A1 follow-up table in DECISIONS (M6) | Read in the sources (paths in DECISIONS) and exercised by code that compiles and runs in the tests and smoke runs |
| Despawning the map entity removes the tilemap `bevy_ecs_tiled` spawned under it | `World::despawn` documentation (recursive over `Children`); smoke runs restart twice and end with the views and the map of the last match only; `bevy_ecs_tiled` logs `Map removed` |
| A restart reads no file again | the catalogue holds the checked grids; the asset reader serves only the bytes registered at startup (D16 item 3) |
| Bevy's runner reads `AppExit` only when its event loop waits, so one more update may follow the last frame | `bevy_winit-0.19.1/src/state.rs:735`; the smoke checks run exactly once, at the last frame |
| No dependency change | `git diff 20116fb -- Cargo.lock Cargo.toml crates/*/Cargo.toml` is empty |

### Deviations from PLAN.md

1. **One faction and one map are shipped.** The menu's faction and map rows list the skirmish file's entries; the shipped file has one faction and the demo map, because the second faction and three maps are M7's scope. The selection mechanism is tested with the two fixture factions and the pure menu functions with several entries per row.
2. **Two players per match** (`MATCH_PLAYERS`): the human player at a map's first start position and one computer player at the second.
3. **What the AI knows beyond its vision** is a recorded decision (D43 item 2): the static terrain of the map and the start positions of the match, as public match setup.
4. **Starting setup changed.** Both players start alike (a yard, 5 light tanks, 3 scouts); until M5 the human player had 18 units and the opponent 8 units and a gun turret. Earlier user verification items that name these are restated below.
5. **The win condition applies only to a match set up by `setup_match`**, so the M0 to M5 tests, whose players often own no building, are unchanged (D41 item 2).
6. **An additional end-to-end test with the shipped data** (`game/tests/m6_production_matches.rs`) beside the plan's fixture-data test (D43 item 10).
7. **New public API of `sim`:** `load_skirmish`, `load_ai`, `setup_match` and its types, `MatchState`, `MatchOutcome`, `AiPlayers`, `AiController`, `plan_starting_units` and `StartError` (moved from the game), `SimEvent::{PlayerDefeated, MatchEnded}`, `CommandQueue::iter`. No existing data schema changed (all existing files keep schema version 1).
8. **Files not listed in PLAN.md section 4:** `crates/sim/src/{ai,ai_defs,match_setup,skirmish_defs}.rs`, `crates/game/src/skirmish.rs`, the M6 test files and fixtures. `m6_definitions.rs` contains `unsafe` code (the counting allocator of D15).

### Tests changed (none removed, skipped, or weakened)

The starting setup moved from game constants to the skirmish file (D40), so the tests that read those constants now read the data. Each keeps its assertion or makes it stronger:

1. `units::tests::the_production_units_load_and_every_sprite_passes_its_check` and `buildings::tests::the_production_buildings_load_and_every_sprite_passes_its_check`: "the starting units (buildings) name defined types" now loops over every faction's groups (buildings) of the skirmish file.
2. `units::tests::the_production_data_loads_with_the_combat_file`: the last assertion said that the opponent starts with a gun turret. The opponent no longer starts with one (deviation 4); the assertion now requires the gun turret, still a building type with a weapon, to be in the computer player's build order, so the demo match still has a defensive structure that fires. All other assertions are unchanged.
3. `units::tests::the_demo_map_holds_every_starting_unit_on_its_own_free_cell`: now checks the match that the menu's first choices set up (every unit of every group on its own passable cell, none on a building, every group complete), and keeps its original assertions on the M2 groups, now given as a test constant (`DEMO_GROUPS`, the same values) to `sim::plan_starting_units`.
4. `units::tests::starting_units_that_do_not_fit_are_a_setup_error`: unchanged assertions; the groups are `DEMO_GROUPS`, and `plan_starting_units` and `StartError` come from `sim`.
5. `buildings::tests::the_demo_map_holds_the_starting_buildings_and_units`: now checks the match set up from the data (both factions' buildings placed, no unit on a building, both players with the file's starting credits) instead of `setup_economy` and the constants.
6. `sim_host::tests::demo_host` (helper) installs the match as the menu does; `match_setup_installs_the_loaded_match_or_ends_the_game_with_an_error` checks the same two outcomes through `install_match` and `begin_match` (the demo map holds the match; a 16×16 map ends the game with exit code 1 with nothing reported as spawned), plus that the previous simulation is kept.
7. `smoke::tests::the_demo_player_can_queue_a_building_with_its_starting_buildings`: the human player's buildings come from its faction. `the_demo_match_expects_all_of_its_starting_units` asserted `starting_unit_total() == 12 + 6 + 5 + 3`, the constant now removed; it now asserts that the expected totals are those of both factions' groups and buildings, and that the set-up match has exactly that many units and buildings.
8. `smoke::tests::PASSING` gained the M6 fields; `the_game_systems_share_no_state_without_an_order` now also builds `SkirmishPlugin`; `SimHostPlugin { seed: 0 }` became `SimHostPlugin` in three tests.

### Known debt (P2 proposals for the reviewer)

1. Termination of computer-player matches is measured, not proven (D43 item 9): two computer players that both keep armed units they never risk, or have nothing left to sell, could stall. The shipped and fixture data end in every measured pairing.
2. The computer player uses no randomness (the same play gives the same responses); the match seed changes only `state_hash` until something uses `SimRng`.
3. A ready building waits while no visible spot satisfies the placement rules (D43 item 4); the computer player does not move units to gain vision for it. With the fixture's small sights this delayed a 3 × 3 building by about 1,000 ticks before the ring rule was relaxed; it is not seen with the shipped data.
4. Waves use `AttackMove` only (no explicit `Attack`, no chase beyond weapon range, no retreat for repairs); harvesters are not targeted on purpose.
5. Balance: in the measured pairings easy sometimes beats hard, depending on the start position. The balance pass is M7's.
6. The menu shows ids (underscores as spaces), not display names; the end screen does not block clicks on the map, which after the end are refused by the closed sink (logged, error sound).
7. A match setup error (shipped data that does not fit a map) ends the game with exit code 1 instead of returning to the menu with a message; every shipped combination is tested (`every_shipped_match_can_be_set_up_for_every_faction_pairing`).
8. The terminal event log also writes the computer player's events and rejections (M4 remediation debt item 2).
9. Test time: `m6_production_matches` takes 52 s and `m6_ai` 40 s in a debug build; `cargo test --workspace` takes about 2.5 minutes.
10. Hotkeys work in the menu (for example `P` toggles the sink's pause flag); every match starts unpaused.

Debt status: resolved by M6: M0 item 3 (constant seed; D44 item 3); M3 review 1 P2-4 and D20 item 4, D27 item 3, D31 item 5 (starting setup in game code; D40); M1 review 3 P2-3 first item (the `misalignment` documentation); M3 implementation debt item 2 for computer players (their stopped harvesters are sent back to work; the human player's still wait for a command). Awaiting the user: M1 review 1 P2-5 (acknowledgement of match setup as setup, D40 item 5; item 12 below). Unchanged and still relevant: M5 review 1 P2-2 (minimap on maps with W + H > 128: no such map is shipped; it must be resolved before M7's maps), P2-4 (the `m4_combat` temporary file race: it did not make a validation command fail in this session's debug runs, so it was not touched), P2-1, P2-3, P2-5, and every other item of "Recorded debt to carry" of the M5 review decision. The linker warning (M0 item 1) and the `Skipped event Destroyed` warning (M0 item 2) were seen again.

### Requires user verification

Not review findings (PLAN.md section 12 item 2); they block only `m6-approved`. Run `cargo run --release -p game` with sound on. The 13 M6 items:

1. **Menu:** the game opens on a dark full-window menu with the title, four rows (Map `demo`, Your faction `vanguard`, Opponent `vanguard`, Difficulty `normal`), each with `<` and `>`, and Start and Quit; all text legible; the sidebar is not visible or clickable through it.
2. **Selection:** `<` and `>` cycle each row and wrap around (the difficulty through easy, normal, hard; the map and faction rows have one entry each, deviation 1).
3. **Start:** Start shows the demo map with the camera on your base: your construction yard at (15, 37) with 5 light tanks and 3 scouts beside it, credits 5000 in the sidebar; the music keeps playing.
4. **The computer player builds:** where you scout its base (the right half of the map, around (52, 42)), it has a power plant, refinery, barracks, war factory, and later gun turrets and a second refinery and power plant; its harvesters collect ore; its units gather a few cells toward the map's centre.
5. **It attacks and defends:** from about 3 minutes (normal) its army attacks your base in waves; units of yours that come near its base are attacked by its units at home and its turrets; a building of its that you destroy is built again.
6. **Difficulty:** on easy the first wave comes later (about 5 minutes) and smaller; on hard earlier (about 2 minutes) and larger.
7. **Defeat:** letting it destroy all your buildings ends the match: the simulation stops, DEFEAT is shown over the map with Restart and Main menu, and orders are refused with the error sound.
8. **Victory:** destroying all of its buildings shows VICTORY (its remaining units do not matter).
9. **Restart:** Restart starts a new match with the same choices: the old map, units, selection, control groups, and minimap content are gone, credits are 5000 again.
10. **Main menu:** Main menu returns to the menu; Start with other choices starts a match with them.
11. **Quit:** Quit closes the game.
12. **Acknowledgement (M1 review 1 P2-5):** match setup installs the map's grid and places the starting players, buildings, and units directly into the simulation before the first tick, not through `CommandQueue` (D11, D40 item 5); during a match the interface and the computer player change state only through `CommandQueue` (I5). Please confirm this reading of I5.
13. **Acceptance:** in complete matches started from the menu, you can lose to the computer player and you can defeat it.

The earlier items still pending (the 16 M5 items, the 9 M4 items, the 10 M3 items, the 7 M2 items, and the 5 M1 items) apply with the M6 starting setup: each player starts with its yard and 5 light tanks and 3 scouts (M2 item 1 named 18 and 8 units), the human player's title and sidebar show `power 0/20` at the start (unchanged), and the opponent has no gun turret at (50, 41) at the start (M4 items 3 and 7): its turrets appear as it builds them, and yours can be built from the sidebar. The opponent now acts (M5 item 15 said it would not until M6).
