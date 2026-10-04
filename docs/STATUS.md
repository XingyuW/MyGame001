# Milestone status

| Milestone | Status | Branch | Approved tag |
|-----------|--------|--------|--------------|
| M0: Bootstrap and dependency verification | **Approved** by the user on 2026-10-04 after independent review (decision: GO WITH DOCUMENTED NON-BLOCKING DEBT; no P0 or P1) | `milestone/m0` | `m0-approved` |
| M1: Map and camera | **Reviewed**: review 3 GO WITH DOCUMENTED NON-BLOCKING DEBT (0 P0, 0 P1, 3 P2) after reviews 1 and 2 NO-GO; merged into `main`. Awaiting user verification for `m1-approved` | `milestone/m1` | `m1-reviewed` |
| M2: Units and movement | **Review 2: NO-GO** (1 P0, 0 P1, 0 P2; `docs/reviews/M2-review-2.md`), in remediation round 2. Review 1: NO-GO (0 P0, 2 P1, 4 P2) | `milestone/m2` | |
| M3: Economy and construction | Not started | | |
| M4: Combat and fog of war | Not started | | |
| M5: Interface and audio | Not started | | |
| M6: Skirmish AI and game flow | Not started | | |
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

1. **A liveness guarantee measured by activity instead of progress (P1-1, with P2-2).** The guarantee "every order ends" was enforced by a counter that any movement reset, so units that kept moving without getting closer (taking turns stepping aside in a dead end one cell wide) defeated it. The counter also ran while a unit was only waiting for the pathfinder, so it reported `Blocked` for units that no unit blocked (P2-2). Every way an order can stay alive was enumerated: stepping along a path (now only counts if the path leads to the destination), stepping aside (never counts), taking a new path (counted, at most 64 without real progress), retargeting (does not reset the measure), terrain change (resets it, finitely often), and waiting for the pathfinder (not counted, bounded by the queue). D21 item 1 gives the resulting bound.
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
