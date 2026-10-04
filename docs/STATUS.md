# Milestone status

| Milestone | Status | Branch | Approved tag |
|-----------|--------|--------|--------------|
| M0: Bootstrap and dependency verification | **Approved** by the user on 2026-10-04 after independent review (decision: GO WITH DOCUMENTED NON-BLOCKING DEBT; no P0 or P1) | `milestone/m0` | `m0-approved` |
| M1: Map and camera | **Implemented, awaiting independent review and user verification** (not approved) | `milestone/m1` | |
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
