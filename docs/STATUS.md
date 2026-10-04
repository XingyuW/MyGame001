# Milestone status

| Milestone | Status | Branch | Approved tag |
|-----------|--------|--------|--------------|
| M0: Bootstrap and dependency verification | **Approved** by the user on 2026-10-04 after independent review (decision: GO WITH DOCUMENTED NON-BLOCKING DEBT; no P0 or P1) | `milestone/m0` | `m0-approved` |
| M1: Map and camera | **Review 2: NO-GO** (1 P0, 1 P1, 4 P2; `docs/reviews/M1-review-2.md`), in remediation round 2. Review 1: NO-GO (4 P0, 3 P1, 6 P2) | `milestone/m1` | |
| M2: Units and movement | Not started | | |
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
