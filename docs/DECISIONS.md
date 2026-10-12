# Decision log

Every entry records the date, the decision, and the evidence consulted. Changes to `PLAN.md` require the user's explicit approval recorded here; the approved changes are recorded under "User decisions" (U2).

## M0 (2026-10-03): dependency verification

Evidence sources: the crates.io API (`https://crates.io/api/v1/crates/<name>`), the resolved `Cargo.lock`, and the source of each pinned crate as downloaded by Cargo into `~/.cargo/registry/src/` (the same code docs.rs renders). Paths below are relative to that registry directory.

### Facts F1 to F3 (re-checked)

| ID | Plan statement | Observed on 2026-10-03 | Outcome |
|----|----------------|------------------------|---------|
| F1 | Bevy 0.19 is current, published 2026-06-22 | crates.io: `bevy` 0.19.0 created 2026-06-19, 0.19.1 created 2026-08-13 (max stable). 0.20.0-rc.1/rc.2 exist (2026-09-15/28) but are pre-releases. | Holds for the 0.19 line; the plan's date differs by three days from the crates.io timestamp (likely the announcement date). Pinned **0.19.1**, the latest stable patch. |
| F2 | `bevy_ecs_tilemap` 0.19.0 (2026-07-04) targets Bevy 0.19, isometric support | crates.io: 0.19.0 created 2026-07-04; its manifest depends on `bevy` 0.19.0 (semver-compatible with 0.19.1); `src/helpers/square_grid/diamond.rs` implements `IsoCoordSystem::Diamond`. | Holds. Pinned **0.19.0**. |
| F3 | `bevy_ecs_tiled` has a release on Bevy 0.19 + `bevy_ecs_tilemap` 0.19 | `bevy_ecs_tiled` 0.13.4 (2026-08-24) depends on `bevy` 0.19, `bevy_ecs_tilemap` 0.19, `tiled` 0.16 (with feature `world`). `Cargo.lock` resolves it to exactly bevy 0.19.1, bevy_ecs_tilemap 0.19.0, tiled 0.16.0. | Holds. Pinned **0.13.4**. |

### A1: APIs exist under the expected names (outcome: TRUE, with mappings)

All names below were checked in the Bevy 0.19.1 sources and exercised by code that compiles and runs in M0 unless marked "compiled in, used from Mn".

| Concept | Bevy 0.19.1 API | Evidence |
|---------|-----------------|----------|
| Custom schedule | `#[derive(ScheduleLabel)]`, `Schedule::new(label)`, `App::add_schedule`, `World::run_schedule(label)` | `bevy_app-0.19.1/src/app.rs:1271`, `bevy_ecs-0.19.1/src/world/mod.rs:3922`; used in `crates/sim/src/schedule.rs` |
| Ordered stages | `#[derive(SystemSet)]`, `Schedule::configure_sets((..).chain())` via trait `IntoScheduleConfigs` | **Mapping:** older Bevy named the trait `IntoSystemConfigs`/`IntoSystemSetConfigs`. `chain()` is implemented for tuples, not arrays (compile error E0599 observed with an array). |
| Ambiguity checking | `Schedule::set_build_settings(ScheduleBuildSettings { ambiguity_detection: LogLevel::Error, .. })`; build errors panic in `Schedule::run` | `bevy_ecs-0.19.1/src/schedule/schedule.rs:522, 560-571, 1572-1630`; test `conflicting_unordered_systems_are_a_schedule_build_error` |
| Fixed timestep | `Time::<Fixed>::from_hz(f64)`, schedule `FixedUpdate` | `bevy_time-0.19.1/src/fixed.rs:105`; used in `crates/game/src/main.rs` |
| Buffered events | `#[derive(Message)]`, `MessageReader<T>::read`, `MessageWriter<T>::write`, `App::add_message` | **Mapping:** what the plan calls an "Event" (`SimEvent`) is a Bevy **`Message`**. In Bevy 0.19 `Event` denotes observer-triggered events (`bevy_ecs-0.19.1/src/event/mod.rs:88`), buffered ones are `Message` (`src/message/mod.rs:100`, `message_writer.rs:74`). `SimEvent` will derive `Message`. |
| Exit codes | `AppExit::{Success, error(), from_code(u8)}` is a `Message`; `fn main() -> AppExit` (implements `Termination`) | `bevy_app-0.19.1/src/app.rs:1560-1615` |
| 2D | `Camera2d` (`bevy_camera-0.19.1/src/components.rs:16`), `Sprite` (`bevy_sprite-0.19.1/src/sprite.rs:19`) | `Camera2d` used in M0; `Sprite` compiled in, used from M2 |
| UI | `Node` (`bevy_ui-0.19.1/src/ui_node.rs:492`) | compiled in (feature `ui`), used from M5 |
| Audio | `AudioPlayer`, `GlobalVolume` (`bevy_audio-0.19.1/src/audio.rs:251`, `volume.rs:10`) | compiled in (feature `audio`), used from M5 |

Name collision: `bevy_ecs::prelude` exports a trait `Command` (deferred world mutation). The plan's `Command` enum lives in `sim`; modules that use it must not glob-import the Bevy prelude.

### A2: `bevy_ecs_tiled` loads isometric diamond maps and exposes custom tile properties (outcome: TRUE, with a caveat)

Evidence:

1. `bevy_ecs_tiled-0.13.4/src/tiled/helpers.rs:87-112`: `tiled::Orientation::Isometric` maps to `TilemapType::Isometric(IsoCoordSystem::Diamond)`.
2. Custom properties: the loaded asset `TiledMapAsset` keeps the full `tiled::Map` (`src/tiled/map/asset.rs:30-32`); `tiled::Tile` dereferences to `TileData`, whose `properties` field is a `HashMap<String, PropertyValue>` (`tiled-0.16.0/src/tile.rs:31-68`, `src/properties.rs:66,138`). No optional feature is needed for read access (the `user_properties` feature only adds automatic reflection into components).
3. Tests in `crates/game/tests/m0_spike.rs` load `assets/maps/spike/spike.tmx` with the same `tiled` crate and assert the orientation, size, tile layout, and string, bool, and int properties.
4. The `game` binary spawns the map through `TiledPlugin` and logs every tile's properties on `TiledEvent<MapCreated>`; the smoke test fails if the map is not created.

**Caveat (relevant to invariant I8):** `tilemap_type_from_map` calls `panic!` for staggered and oblique maps (`helpers.rs:106-111`), and the spawn code contains `expect` calls. M1 must therefore validate a map (orientation, layers, properties) with its own loader and return typed errors *before* handing it to `bevy_ecs_tiled`. Because `sim` cannot depend on `bevy_ecs_tiled` (it pulls in all of Bevy), the M1 loader in `sim` should use the `tiled` crate directly (to be verified in M1 to carry no rendering dependency) or the recorded fallback of a RON map format.

### A3: `sim` can depend on Bevy with rendering, windowing, and audio disabled (outcome: FALSE as stated; fallback applied)

Evidence: a probe crate depending on `bevy = { version = "=0.19.1", default-features = false }` resolves to `bevy_internal` plus `bevy_app, bevy_derive, bevy_diagnostic, bevy_ecs, bevy_input, bevy_math, bevy_reflect, bevy_tasks, bevy_time, bevy_transform, bevy_utils, bevy_platform, bevy_ptr` (`cargo tree -e normal`). `bevy_input` is an input dependency forbidden by I1, so the umbrella crate is unsuitable.

Fallback (recorded in PLAN.md section 2): `sim` depends on `bevy_ecs` and `bevy_app` directly, each with `default-features = false, features = ["std"]`. `cargo tree -p sim -e normal` contains no `wgpu`, `winit`, `bevy_render`, `bevy_window`, `bevy_input`, `bevy_audio`, `cpal`, `rodio`, or `gilrs` crate. (`ctrlc`, a terminal signal handler pulled in by `bevy_app/std`, is not an input-device dependency.)

### A4: Rust toolchain (outcome: resolved)

`bevy` 0.19.1 declares `rust-version = "1.95.0"` (`bevy-0.19.1/Cargo.toml:14`; crates.io reports msrv 1.95.0 for 0.19.0 and 0.19.1). `rust-toolchain.toml` pins **1.98.1** (the stable toolchain installed on the development machine, released 2026-09-01, which satisfies the requirement) with `rustfmt` and `clippy`. The workspace declares `rust-version = "1.98.1"` so that a mismatched toolchain fails loudly. Edition 2024.

### A5: Maintained RON and portable RNG crates (outcome: TRUE)

| Need | Crate | Evidence | Use |
|------|-------|----------|-----|
| RON parsing | `ron` 0.12.2 (2026-06-22) | crates.io | Added with the first data file (M1/M2), pinned exactly. Not added in M0, which has no data files. |
| Seedable portable RNG | `rand_chacha` 0.10.0 (2026-02-02, rust-random organisation) | Crate docs: "These generators are all deterministic and portable ... with testing against reference vectors" (`rand_chacha-0.10.0/src/lib.rs:20`). `SeedableRng::seed_from_u64` (`rand_core-0.10.1/src/seedable_rng.rs`): changing it "should be considered a value-breaking change". | `SimRng` wraps `ChaCha8Rng`. Its full state (`get_seed`, `get_stream`, `get_word_pos`) is hashed by `state_hash`. **Mapping:** in `rand_core` 0.10 the generator trait is `rand_core::Rng` (was `RngCore`). |

The PCG fallback is not needed.

## M0 (2026-10-03): design decisions

### D1. Asset root

Bevy 0.19.1 resolves the asset directory from `BEVY_ASSET_ROOT`, then `CARGO_MANIFEST_DIR`, then the executable's directory (`bevy_asset-0.19.1/src/io/file/mod.rs:19-29`). Under `cargo run -p game` the second would be `crates/game/`, but the plan places `assets/` at the workspace root. `.cargo/config.toml` sets `BEVY_ASSET_ROOT` to the workspace root (`[env]`, `relative = true`) for every `cargo run`/`cargo test` process. Release packaging (M7) must place `assets/` next to the executable.

### D2. Bevy features for `game`

`bevy` with `default-features = false, features = ["2d", "ui", "audio"]`. These are Bevy 0.19 feature collections; `2d` and `ui` include `default_app` and `default_platform` (windowing, input, logging, assets). The default set additionally includes `3d`, which the game does not use.

### D3. Tick structure

`SimSchedule` chains the plan's nine stages (`SimSet::Commands` ... `SimSet::WinCondition`) and one added stage, `SimSet::EndOfTick`, which increments `SimTick`. During a tick, systems observe the 0-based index of the tick in progress; after N runs `SimTick` is N. Ambiguity detection is set to `LogLevel::Error`, so conflicting systems without an order make the schedule fail to build (enforces I4).

### D4. `state_hash` layout

FNV-1a 64-bit (crate-local implementation, checked against the reference vectors for "", "a", "foobar"), integers little-endian, in this order: `SimTick`, `SimRng` (32-byte key, `u64` stream, `u128` word position), the next unallocated `SimId`, the count of entities with a `SimId`, then each `SimId` ascending. Later milestones append each entity's components after its id. Including the RNG position means two runs that consume a different amount of randomness hash differently even before that randomness affects other state.

### D5. `Command` in M0

`Command` is declared with no variants (uninhabited), so `CommandQueue` exists and is drained every tick but no command can be constructed yet. Variants and their validation are added by the milestones that implement them. `process_commands` carries `#[expect(clippy::never_loop)]`, which turns into a build failure under `-D warnings` once the first variant makes the loop reachable, forcing its removal.

### D6. Smoke test semantics

`--smoke-test <frames>` (an integer from 1 to `u32::MAX`; anything else exits with code 2 and a usage message) builds exactly the same `App` as a normal run, adds a frame counter in `Last`, and exits after the given number of frames. The exit code is 0 only if no asset failed to load (`UntypedAssetLoadFailedEvent`) and the spike map was created (`TiledEvent<MapCreated>`); otherwise 1. The final log line reports frames, simulation ticks, and map creation. For small frame counts the result depends on asset loading speed (the review observed `--smoke-test 1` pass); see STATUS debt item 5.

**Amended in M2 (2026-10-04).** Since M1 the map whose creation is required is the validated demo map (M1 review 3, P2-3 item 4). Since M2 the smoke test also requires that the starting units were spawned, orders all of the player's units through `CommandQueue` to cell (20, 26) of the demo map as soon as they exist, fails if that command is rejected, and, if at least 40 ticks ran after the order, fails unless some unit moved. The final log line also reports the units spawned and moved.

**Amended in M3 (2026-10-04).** The smoke test also requires the starting buildings and checks that the production it queues is charged for (D27 item 5). The final log line also reports the buildings spawned and the credits charged.

**Amended in M4 (2026-10-04).** The smoke test also requires, once a tick has run, that the human player sees some cells (the fog is computed), and that every unit and building view is shown exactly when the human player sees what it draws (unseen enemies hidden, D31 item 3). The final log line also reports the visible cells, the enemy views shown, and the mismatches.

**Amended in M6 (2026-10-04):** see "Notes on earlier decisions (M6)" below.

**Amended in M5 (2026-10-04).** The smoke test loads the full interface and issues its orders the interface's way: the move with `selection::move_command` and the production with the sidebar's `button_effect`, both pushed into `CommandSink` (D34). It also requires a sidebar button for every production item, every displayed sidebar button inside the window, the sidebar's credits equal to the human player's credits, the minimap drawn (once a tick has run) with lit pixels, all nine sound effects synthesised, exactly one music player, and, in runs of at least 300 frames, a pause of 30 frames in the middle without a tick and with a command refused (D39). The final log line also reports these.

### D7. Isometric projection reconciled with `bevy_ecs_tilemap` (PLAN.md section 5, equation 7)

Conventions observed in the sources:

* Tiled (and the plan's equation 7): cell (0, 0) is the top corner of the diamond, +x runs down-right, +y runs down-left. With Bevy's y axis pointing up, equation 7 maps *continuous* cell coordinates `(u, v)` to `world_x = (u − v)·W/2`, `world_y = −(u + v)·H/2`; integer coordinates are diamond corners, so the centre of cell c is at `c + 0.5`, matching `SubPos` (`c·256 + 128`, i.e. `SubPos / 256`). Its inverse followed by `floor` returns the cell containing a point.
* `bevy_ecs_tilemap` diamond (`helpers/square_grid/diamond.rs:72-186`): tile centre at `((x + y)·W/2, (y − x)·H/2)` relative to the tilemap origin, i.e. `TilePos (0, 0)` is the *left* corner, +x runs down-right, +y runs up-right; picking rounds with `floor(v + 0.5)`.
* `bevy_ecs_tiled` stores Tiled cell `(cx, cy)` at `TilePos { x: cx, y: height − 1 − cy }` (`map/asset.rs:191-196`), and adds only the Tiled layer and tileset offsets (zero in our maps) and a z offset per layer (`map/spawn.rs:142-146, 328-331`).

Reconciled form, for a map `height` cells tall, relative to the map entity's `Transform`:

```
tile_centre_world(cx, cy) = anchor_offset
                          + equation7(cx + 0.5, cy + 0.5)
                          + ((height − 1)·W/2, height·H/2)
```

and conversely a world point `p` is in cell `floor(equation7⁻¹(p − anchor_offset − ((height − 1)·W/2, height·H/2)))`. `anchor_offset` is `TilemapAnchor::as_offset` (zero for `TilemapAnchor::None`; the spike uses `TilemapAnchor::Center`). Evidence: `plan_projection_matches_bevy_ecs_tilemap_diamond_for_every_cell` compares this against `TilePos::center_in_world` and `TilePos::from_world_pos` for every cell of 1×1, 1×7, 7×1, 3×50, 16×16, and 64×64 maps.

## M1 (2026-10-04): map loading, projection, and camera

Evidence sources as for M0: the crate sources in `~/.cargo/registry/src/`, `Cargo.lock`, and probes run on the development machine. All APIs used in M1 were checked in the Bevy 0.19.1, `bevy_ecs_tiled` 0.13.4, `bevy_ecs_tilemap` 0.19.0, `tiled` 0.16.0, and `quick-xml` 0.31.0 sources, and are exercised by code that compiles and runs.

### A2 follow-up: the map loader in `sim` uses `tiled` directly

1. `sim` depends on `tiled = "=0.16.0"` with `default-features = false` (the default feature `zstd` builds C code) and on `quick-xml = "=0.31.0"`. This is how `bevy_ecs_tiled` 0.13.4 itself depends on `tiled` (`Cargo.toml`: `default-features = false`, `features = ["world"]`), and `tiled` 0.16.0 depends on `quick-xml` 0.31.0. Both were already resolved, so `Cargo.lock` changed only in `sim`'s dependency list and gained no package. `cargo tree -p sim -e normal` contains no `wgpu`, `winit`, `bevy_render`, `bevy_window`, `bevy_input`, `bevy_audio`, `cpal`, `rodio`, `gilrs`, or `zstd` crate (invariant I1). Within the workspace, Cargo's feature unification enables `tiled`'s `world` feature (pure Rust: `regex`, `serde`), which adds no forbidden crate.
2. **Panic found in `tiled` 0.16.0 (invariant I8).** `FiniteTileLayerData::get_tile_data` (`tiled-0.16.0/src/layers/tile/finite.rs:69`) indexes the tile vector without a bounds check, and the parser does not check the number of CSV entries against the layer size. A probe that loaded a 4×3 map with 11 entries succeeded and then panicked on `get_tile(3, 2)` with "index out of bounds: the len is 11 but the index is 11". `bevy_ecs_tiled`'s `for_each_tile` makes the same call. Mitigation: before `tiled` parses the file, `sim` counts each layer's CSV entries exactly as `tiled` decodes them (the first text or CDATA node of `<data>`, split on commas, using the same `quick-xml` version and reader settings) and rejects any count other than `width × height`. `tiled` then parses the same bytes from memory. Regression test: `layer_data_with_wrong_entry_count_is_rejected_without_panic`.
3. Other panic sites reviewed: `tiled` `template.rs:79` (`unwrap` of a value set on the preceding lines) and `layers/tile/util.rs:76` (`unwrap` on a 4-byte `chunks_exact` slice) cannot fail. In `bevy_ecs_tiled`, the `panic!` for staggered and oblique maps (`helpers.rs:106-111`, `map/asset.rs:147`) is unreachable because only isometric maps pass the loader, and the `expect` in `map/spawn.rs:378` (image-collection tilesets) is unreachable because only single-image tilesets pass. ~~This resolves the A2 caveat of M0.~~

   **Correction (M1 remediation round 1, 2026-10-04).** This list was incomplete, and the conclusion drawn from it was wrong. M1 review 1 (`docs/reviews/M1-review-1.md`) reproduced failures at sites the list missed, all reachable from a map file that the loader accepted or handed to `tiled` unchecked:

   | Site | Failure | Trigger | Review |
   |------|---------|---------|--------|
   | `tiled` `tileset.rs:577` (`calculate_columns`) | subtract or add overflow (debug), division by zero (release), wrapped value (release) | `margin` or `spacing` without `columns` | P0-1 |
   | `tiled` `tileset.rs:484, 534` | allocation sized by `tilecount` (246 GB observed) | large `tilecount` | P0-3 |
   | `tiled` `layers/group.rs:31-70`, `properties.rs:140-178` | stack overflow (process abort) | deeply nested `<group>` or class properties | P0-2 |
   | `bevy_ecs_tiled` `tiled/map/loader.rs:421-422` | subtract overflow (debug), wrapped column count (release) | `margin` even with `columns` | P0-4 |
   | `bevy_asset` `loader_builders.rs:251` | `unwrap` on an unapproved asset path | image or tileset `source` leaving the asset root | P0-4 |

   Listing panic sites one by one cannot show that none is left. The approach is therefore replaced by D13: the loader accepts only an explicitly listed subset of the format, checked on the raw bytes before `tiled` or `bevy_ecs_tiled` sees them (invariant I11), and the claim is now limited to what the tests in `crates/sim/tests/m1_map_subset.rs` (including a seeded mutation test under a memory and time budget) and the game's tests verify.
4. `truncated_map_files_never_panic` loads every prefix of the valid fixture. Every prefix that cuts before the closing tag is rejected. The XML parser accepts an unterminated final end tag (`</map` without `>`) at the end of the file; such a prefix contains the complete map and loads it unchanged.

### D8. Map format, schema version 1

A map is a Tiled `.tmx` file. `sim::load_map` accepts it only if all of the following hold, and otherwise returns a `MapError { file, field, kind }` whose message names the file, the field, and the reason:

1. Orientation `isometric` (diamond), `infinite="0"`, width and height from 1 to `MAX_MAP_SIDE` = 256 cells, tile width and height greater than 0.
2. Exactly one map property, `schema_version` (int) = 1.
3. Exactly one layer: a tile layer named `terrain`, with the map's size, no properties, and uncompressed CSV data with exactly `width × height` entries.
4. Every tileset uses a single image and has no properties. Every tile definition has exactly the properties `passable` (bool), `buildable` (bool), and `ore` (int ≥ 0). No property is optional, so there are no silent defaults. Every tile id below the tileset's `tilecount` must be defined: `tiled` creates an entry without properties for each such id, and the loader rejects it as a tile whose required properties are missing (review 1, P2-6).
5. Every cell holds a tile with a definition. Flip flags are allowed and ignored by the simulation. External tilesets (`.tsx`) are allowed.

Amended by D13 (M1 remediation round 1): items 1 to 5 still hold, and D13 restricts the file further to an explicitly listed subset (exactly one tileset, tile size 64×32, and the element, attribute, and value rules listed there).

Errors found while checking properties are reported in sorted name order, so the reported error does not depend on hash map iteration order (I4). The 256-cell limit keeps sub-cell coordinates (`c × 256 + 128`) far below `i32::MAX` and bounds memory. It can be raised in a later milestone if larger maps are needed.

### D9. Projection in the game (final form of D7)

`crates/game/src/projection.rs` implements equation 7 for continuous cell coordinates, its inverse followed by `floor`, and picking that returns `None` outside the map or for a non-finite point. Cell c spans [c, c + 1), so a point on a shared edge or corner belongs to the cell with the larger coordinate. The map entity is spawned with `TilemapAnchor::None` at `map_origin_offset = −((height − 1)·W/2, height·H/2)`. Under it, `bevy_ecs_tiled` draws the centre of cell c exactly at equation 7 evaluated at c + ½, so world coordinates are equation 7 with no further offset. `map_origin_offset_aligns_bevy_ecs_tilemap_with_the_projection` checks this against `TilePos::center_in_world` and `TilePos::from_world_pos` for every cell of six map sizes up to 64×64. Layer offsets, layer parallax, and tileset tile offsets would shift the drawing away from the projection. Since M1 remediation round 1 the attributes and elements that carry them are outside the supported subset (D13), so `sim::load_map` rejects them before the renderer sees the map; the game's check when the map is created (`map::misalignment`) remains as a second line.

Picking accuracy: with 64×32 tiles, every coordinate used in the picking tests is a multiple of 1/64 cell and is exactly representable in `f32`, so the tests assert exact equality (zero error) for the round trip of every cell of 1×1, 1×37, 37×1, and 64×64 maps, and for 16 points per cell at centres, edges, and corners.

### D10. Camera

| Parameter | Value |
|-----------|-------|
| Keyboard pan | Arrow keys (WASD stays free for M5 hotkeys) |
| Pan speed | 1000 world units per second × projection scale |
| Edge pan | Cursor within 8 logical pixels of a window edge, only while the window has focus |
| Drag pan | Middle mouse button; the world point under the cursor stays under it |
| Zoom | Mouse wheel, ×1.1 per line (20 pixels count as one line for touchpads), projection scale clamped to [0.5, 2.0] |
| Bounds | The camera centre is clamped so that the view (logical viewport size × scale) stays inside the map's bounding rectangle. Along an axis on which the view is larger than the map, the view is centred on the map. |

Bevy 0.19.1 sets the orthographic projection from the camera's logical viewport size (`bevy_render-0.19.1/src/camera.rs:425-429`), and with the default `ScalingMode::WindowSize` the view spans that size × `scale` (`bevy_camera-0.19.1/src/projection.rs:693-696`). Picking runs in `PostUpdate` after `TransformSystems::Propagate` and `CameraUpdateSystems`, so it uses the current frame's camera.

### D11. The grid in the simulation and in `state_hash`

`SimPlugin` inserts an empty 0×0 `Grid`. Match setup (in M1, the game's startup system, and in tests, the scenario setup) replaces it with the grid of a `LoadedMap` before the first tick. This is setup, not a gameplay change, so it does not go through `CommandQueue` (I5 concerns the interface and the AI during a match). `state_hash` (D4) now appends, after the entities: the grid width and height (`u32`), then every cell in row-major order as one flag byte (bit 0 passable, bit 1 buildable) and the ore amount (`u32`), all little-endian. The cell occupant listed in PLAN.md section 5 is added in M2, together with the units that occupy cells.

### D12. Locating the map file

The game resolves the map path with `FileAssetReader::new(AssetPlugin::default().file_path).root_path()`, the same directory Bevy's asset server reads from (`bevy_asset-0.19.1/src/io/file/mod.rs:19-64`). D1 (asset root under `cargo run`, packaging in M7) therefore applies unchanged. `AssetPlugin::DEFAULT_UNPROCESSED_FILE_PATH` is private in 0.19.1, so the default plugin value is used instead.

## M1 remediation round 1 (2026-10-04)

Evidence sources as for M1, plus the findings of `docs/reviews/M1-review-1.md`, reproduced in scratch copies of the repository at the reviewed commit (`git archive`), and the user's notes supplied with the remediation task (reconciled in `docs/STATUS.md`).

### D13. Supported subset of the Tiled format (invariant I11)

`sim::load_map` checks every map file, and the external tileset it references, against the following allow-list on the raw bytes before `tiled` parses anything (`crates/sim/src/map_subset.rs`). Anything outside it is a typed `MapError` naming the file that contains the fault (the `.tsx` for faults in an external tileset), the field, and the reason. `tiled` then parses only the checked bytes: its resource reader serves the map and the checked tileset from memory and refuses every other path.

Reading rules (the same as the third party code's): the XML is read with `quick-xml` 0.31.0 and default reader settings, as `tiled` does. Element and attribute names are compared in full, because `tiled` matches them by local name: a namespaced name such as `a:width` would reach it as `width`, so it is rejected, as is any attribute that appears twice. Property values must parse as their declared type with the same `str::parse` calls as `tiled` (`i32`, `f32`, `bool`). The file must be UTF-8 without a byte order mark (amended in M1 remediation round 2, D17) and at most `MAX_MAP_FILE_BYTES` = 4 MiB (read with a bound, so a larger file is never read whole). Between elements only whitespace and comments may appear; DTDs, processing instructions, CDATA sections, text content, content after the root element, and an end tag cut off at the end of the file (review P2-2) are rejected.

| Element (parent) | Attributes (all others rejected) | Content and limits |
|------------------|----------------------------------|--------------------|
| `map` (root) | `version` (required), `tiledversion`, `orientation` = `isometric`, `renderorder` = `right-down`, `width`, `height` (1 to 256), `tilewidth` = 64 and `tileheight` = 32, `infinite` = `0`, `nextlayerid`, `nextobjectid` (unsigned) | at most one `properties`; exactly one `tileset`, before the layer (enforced by the checker since M1 remediation round 2, D17); one `layer` named `terrain`; `objectgroup`, `imagelayer`, and `group` are rejected as unexpected layers, any other element as unsupported |
| `tileset` (map), embedded | `firstgid` = `1`, `name`, `tilewidth` = 64, `tileheight` = 32, `tilecount` (1 to `MAX_TILESET_TILES` = 256; 1024 until M1 remediation round 2, D16), `columns` (≥ 1) | at most one `properties`, exactly one `image`, `tile` elements |
| `tileset` (map), reference | `firstgid` = `1`, `source` | empty element only (`tiled` would read the content of a non-empty reference as map content) |
| `tileset` (root of a `.tsx`) | `version`, `tiledversion`, `name`, `tilewidth`, `tileheight`, `tilecount`, `columns`, as for an embedded tileset | as for an embedded tileset |
| `image` (tileset) | `source`, `width`, `height` (positive `i32`) | none |
| `tile` (tileset) | `id` (unique, below `tilecount`) | at most one `properties`; animations, collision shapes, and per-tile images are rejected |
| `layer` (map) | `id`, `name`, `width`, `height` | at most one `properties`, at most one `data` |
| `data` (layer) | `encoding` = `csv`, no `compression` | one plain text node with exactly `width × height` comma-separated entries |
| `properties` (map, tileset, tile, layer) | none | `property` elements with unique names |
| `property` (properties) | `name`, `value` (both required), `type` ∈ {`string` (default), `int`, `float`, `bool`} | none; `class` and `list` are rejected (`tiled` parses them recursively) |

Tileset geometry (what `tiled` and `bevy_ecs_tiled` compute with): no `margin` or `spacing` attribute (both default to 0), image width and height at most `MAX_TILESET_IMAGE_SIDE_PX` = 8192 (since M1 remediation round 2, D16), image width = `columns × 64`, image height a positive multiple of 32, and `tilecount = columns × rows`. Tile ids used by cells must have definitions (D8 item 4), so every rendered texture index is inside the image's tile grid.

Tile size: 64×32 is the only accepted tile size, because it is the only one for which exact picking is established (D9; `projection::tests::the_tested_tile_size_is_the_only_one_maps_may_use` ties the test tile size to `sim::TILE_WIDTH_PX`/`TILE_HEIGHT_PX`). Supporting another size requires extending the picking tests to it first.

Referenced files: `image` and tileset `source` values are plain file names in the map's directory (ASCII letters, digits, `_`, `-`, `.`, not starting with `.`, ending in `.png` and `.tsx` respectively). They cannot leave the asset root or contain the characters Bevy's asset paths interpret (`#`, `:`). `bevy_ecs_tiled` finds external tilesets with its own line-based scan for `source="` and `template="` (`tiled/reader.rs`, `extract_external_paths`). The check reproduces that scan exactly and requires it to find exactly the accepted references: the map's tileset reference, and nothing in a tileset file. A single-quoted reference (which the scan misses) or a comment containing `source="x.tsx"` (which the scan would follow) is therefore rejected.

Rendering input outside `sim`: ~~the game checks the tileset image before handing the map to `bevy_ecs_tiled` (`game::map::check_tileset_image`): the file must start with a PNG signature and an IHDR chunk whose size equals the size the map declares. An image that passes this header check but cannot be decoded fails to load in Bevy, which D14 makes fatal.~~ **Superseded by D16 (M1 remediation round 2):** the header check left the image's subset unlisted, and a decodable PNG with a compressed ancillary chunk made the renderer allocate gigabytes (M1 review 2, P0-A).

Limits of the claim: the subset was derived from the source of `tiled` 0.16.0, `bevy_ecs_tiled` 0.13.4, and `bevy_asset` 0.19.1, and is verified by `crates/sim/tests/m1_map_subset.rs` (one regression test per review finding on the loader, and a seeded mutation test of 6,000 cases over the demo map) and by the game's unit tests; it is not a proof that the third party code cannot fail on every input inside the subset.

### D14. Asset load failures end the game

`game::map::exit_on_asset_load_failure` (registered in `MapPlugin`, schedule `Last`) logs the asset path and the reason of every `UntypedAssetLoadFailedEvent` and writes `AppExit::error()`, in every mode, so that a failed map or tileset image load exits with code 1 like a map rejected by `sim` (review P1-2). Before, only `--smoke-test` reacted, and a normal run kept going without a map. The smoke test's own handler (D6) is unchanged.

### D15. Bounded hostile-input tests

`crates/sim/tests/m1_map_subset.rs` feeds hostile maps to the loader. Its test binary installs a global allocator that refuses any allocation raising the bytes in use above 256 MiB (Rust then aborts the process, failing the run), every load runs on a worker thread whose result must arrive within 30 s (the mutation test: 300 s for all cases), and a stack overflow aborts the run. A regression of P0-2 or P0-3 therefore fails the tests instead of exhausting the machine: run against the reviewed code, the tilecount test and the mutation test abort with "memory allocation of 1726576853000 bytes failed" in debug and release, and the nesting test aborts with a stack overflow. The mutation test uses its own SplitMix64 generator with a fixed seed, so its 6,000 cases are the same on every run.

## M1 remediation round 2 (2026-10-04)

Evidence sources as for round 1, plus the findings of `docs/reviews/M1-review-2.md`, reproduced with the reviewed binaries (`deb2859` + `efb25df`, unchanged code) on scratch asset roots selected with `BEVY_ASSET_ROOT`, and the sources of `bevy_asset` 0.19.1 (`server/mod.rs` `get_meta_loader_and_reader`, `io/source.rs`, `io/mod.rs`), `bevy_image` 0.19.1 (`image.rs` `from_buffer`, `image_texture_conversion.rs`), `image` 0.25.10 (`codecs/png.rs`, `io/image_reader_type.rs`, `io/limits.rs`), `png` 0.18.1 (`decoder/stream.rs`, `decoder/mod.rs` `Limits`), `bevy_ecs_tilemap` 0.19.0 (`render/texture_array_cache.rs`), `bevy_render` 0.19.1 (`settings.rs`, `renderer/mod.rs`), and `wgpu-types` 29.0.4 (`limits.rs`).

### D16. Rendering inputs: texture limits, the tileset image subset, and checked bytes only (invariant I11)

The class of defect behind M1 review 2's P0-A and P1-A is the same as in round 1, on the rendering side: **external input reaches third party rendering code outside a listed subset**, so it can make that code allocate without bound or fail without file context. Every external input the renderer receives was enumerated: the map file, its external tileset, the tileset image, and the `.meta` file Bevy looks for next to every asset it loads. Each is now restricted, and the renderer receives nothing else.

1. **Texture limits (P1-A).** `bevy_ecs_tiled` hands the tileset image to `bevy_ecs_tilemap`, which uploads it as one 2D texture of the image's size and copies the tiles into a texture array with one layer per tile, plus one when the count is 1 or a multiple of 6 (`texture_array_cache.rs`). Bevy requests the adapter's limits (`renderer/mod.rs`, priority `Functionality`), and `wgpu::Limits::default()` documents the limits "guaranteed to work on all modern backends": a 2D texture side of 8192 and 256 array layers. `sim` therefore bounds `tilecount` to `MAX_TILESET_TILES` = 256 (was 1024) and the image width and height to `MAX_TILESET_IMAGE_SIDE_PX` = 8192, with a typed `MapError` naming the file, the image attribute, and the limit. With 64×32 tiles the largest accepted images are 8192×64 (128 × 2 tiles), 64×8192 (1 × 256), and any grid of at most 256 tiles within those sides; at most 2 MiB of RGBA pixels. `game::map::tests::tileset_limits_fit_the_texture_limits_every_gpu_supports` ties the two constants to `WgpuLimits::default()`. A configuration with lower limits (the environment variable `WGPU_SETTINGS_PRIO=webgl2`) is not supported.
2. **The tileset image subset (P0-A).** `bevy_image` decodes PNG with `image::ImageReader` and `no_limits()`, and `png` then inflates `iCCP` (and text chunks) with no effective bound. `game::tileset_image::check_tileset_image` checks the image before the hand-off and returns the checked bytes:
   * at most `MAX_TILESET_IMAGE_BYTES` = 4 MiB, read with that bound (a tileset image of the subset is at most about 2 MiB even uncompressed);
   * signature, then `IHDR` with the size the map declares, bit depth 8, colour type 2 (RGB) or 6 (RGBA), compression and filter method 0, no interlacing;
   * then at most one each of `sRGB` (1 byte), `gAMA` (4), `cHRM` (32), and `pHYs` (9), then one or more consecutive `IDAT`, then an empty `IEND` at the end of the file; every other chunk (among them `iCCP`, `zTXt`, `iTXt`, `tEXt`, `eXIf`, `PLTE`, `tRNS`, `bKGD`, and the animation chunks) is rejected;
   * a complete decode with the decoder and settings Bevy uses (`ImageReader` over the bytes, format PNG), but with `image::Limits { max_alloc: 16 MiB, max_image_width/height: 8192 }`, which `image` passes to `png` as its allocation budget; the result must be an RGB8 or RGBA8 image of the declared size.

   Bevy converts RGB8 and RGBA8 to `Rgba8UnormSrgb`, which every GPU samples as a filterable float texture. Other PNG formats are outside the subset because Bevy maps them to textures the tilemap shader cannot sample: a 16-bit greyscale PNG becomes `R16Uint`, and the release binary quit with a render validation error naming no file (found in this round). Because the decode runs before the hand-off with Bevy's decoder, an image that Bevy could not decode is now a typed error before the hand-off rather than an asset load failure after it (D14 stays as the second line). Errors are `TilesetImageError { file, field, kind }`, printed as `tileset image <file>: <field>: <reason>`.
3. **The renderer reads only checked bytes.** `game::checked_assets::CheckedAssetsPlugin`, added to `DefaultPlugins` before `AssetPlugin`, replaces Bevy's default asset source with `CheckedAssetReader`. It serves, from memory, exactly the files registered by the map loader: the map and external tileset bytes that `sim::load_map_with_files` checked and parsed (new API; `load_map` is unchanged), and the image bytes that `check_tileset_image` checked. Every other read fails (`NotFound`), and `read_meta` always reports "not found", so Bevy uses each loader's default settings. Before, Bevy read `<asset>.meta` for every asset whole and without a size limit and let it choose the loader and its settings: a 400 MB `demo_tiles.png.meta` made the reviewed release binary peak at 0.65 GB (found in this round). This also removes the second read of the map, tileset, and image files (M1 review 1 P2-1, M1 review 2 P2-C), so a file changed after its check cannot reach the renderer. The embedded asset source (Bevy's shaders) is compiled into the binary and unaffected.
4. **Dependencies.** `game` depends on `image` =0.25.10 (`default-features = false`, feature `png`), the version `bevy_image` already uses, and gains the dev-dependencies `flate2` =1.1.10 and `crc32fast` =1.5.2 (to write hostile PNG files in tests), both already used by `png`. `Cargo.lock` gained no package; only `game`'s dependency list changed (I9).
5. **Bounded tests (as D15).** `crates/game/tests/m1_tileset_image.rs` includes `src/tileset_image.rs` by path, installs a global allocator with a 64 MiB budget, and runs every check on a worker thread with a 60 s limit. Its `iCCP`, `zTXt`, and `iTXt` bombs inflate to 256 MiB, four times the budget. Run against the reviewed header-only check, all six tests fail (the bombs, 16-bit greyscale, and oversized files are accepted); run against the fix with the chunk check removed and the decoder's limits replaced by `no_limits()`, the bomb test aborts with "memory allocation of 69206016 bytes failed" instead of growing.

### D17. Precise rejection reasons and the tick counter (M1 review 2, P2-A and P2-D)

1. A UTF-8 byte order mark is rejected first, as such (`quick-xml` skips it and shifts its reported positions, so it was reported as an unterminated end tag).
2. The checker enforces exactly one `tileset` (`MapErrorKind::MissingElement`, field `map element <tileset>`), and the tileset must precede the layer (`MapErrorKind::OutOfOrder`), because `tiled` resolves the layer's tile ids against the tilesets read before it. Both were reported as "cell (0, 0): cell has no tile". An empty `<map …/>` element is rejected the same way.
3. An external tileset that cannot be read is reported at the reference's `source` attribute with `MapErrorKind::Read` naming the `.tsx` path; one above the size limit is reported in the `.tsx` itself (`FileTooLarge`).
4. `MapError`'s message begins with `tileset <path>` when the faulty file is a `.tsx`, and with `map <path>` otherwise.
5. `SimTick` advances with `wrapping_add` (invariant I12), so a counter at `u64::MAX` behaves the same in debug and release builds. A review of every integer operation in `crates/sim/src` found no other operation that can overflow: the map arithmetic uses `u64` products of values bounded by the subset, `checked_add`, or `try_from`, and `SimIdAllocator` uses `checked_add`.

## M2 (2026-10-04): units and movement

Evidence sources as for M1: the sources of the pinned crates in `~/.cargo/registry/src/` (`ron` 0.12.2, `serde` 1.0.229, Bevy 0.19.1), `Cargo.lock`, and runs on the development machine. Every API named below is exercised by code that compiles and runs in M2.

### A5 follow-up: `ron` and `serde` in `sim`

`sim` depends on `ron = "=0.12.2"` and `serde = { version = "=1.0.229", features = ["derive"] }`, both already resolved through Bevy; `Cargo.lock` changed only in `sim`'s dependency list (I9). `cargo tree -p sim -e normal` contains no `wgpu`, `winit`, `bevy_render`, `bevy_window`, `bevy_input`, `bevy_audio`, `cpal`, `rodio`, `gilrs`, `zstd`, `image`, or `png` crate (I1). APIs: `ron::Options::default()` (no extensions, recursion limit 128, `options.rs:58-65`), `Options::with_recursion_limit` and `Options::from_str` (`options.rs:84, 114`), `ron::error::SpannedError` with `Display` (`error.rs:22, 118`); `#[derive(serde::Deserialize)]` with `#[serde(deny_unknown_fields)]`. `ron` treats space, tab, line feed, carriage return, vertical tab, form feed, and some Unicode characters as whitespace (`parse.rs:37`) and enables extensions through an `#![enable(...)]` attribute at the start of the file.

### D18. Data files: the supported subset of RON (invariants I8 and I11)

A data file reaches `ron` only after `sim::data::check` has accepted it (`crates/sim/src/data.rs`):

1. It is a regular file (directories, devices, and named pipes are rejected before they are opened) of at most `MAX_DATA_FILE_BYTES` = 64 KiB, read with that bound.
2. Every byte is printable ASCII, space, tab, line feed, or carriage return. This excludes a byte order mark and every character for which `ron`'s and the checker's notions of whitespace could differ.
3. Between tokens only whitespace and `//` line comments. Block comments, `#` (and with it `#![enable(...)]`, which switches on `implicit_some`, `unwrap_newtypes`, and other extensions that change how a file reads), raw and byte strings, characters, floats, hexadecimal, `_` separators, type suffixes, a leading `+`, leading zeros, maps, tuples, enum variants, `Some`/`None`, booleans, and struct names are outside the subset.
4. Values: an integer (`-`, then decimal digits, within `i64`), a string (`"`, printable ASCII without `"` and `\`, on one line, at most `MAX_STRING_BYTES` = 64 bytes, `"`), a list (`[` ... `]`, comma separated, optional trailing comma, at most the schema's number of entries), or a struct (`(` `name: value` pairs `)`, comma separated, optional trailing comma).
5. The checker is driven by the file's schema (`Shape`): each struct has exactly its schema's fields, each once, in any order, with values of the schema's kind; the schema version is checked as soon as it is read, so a file of another version is reported as such. No field is optional (no silent defaults).
6. The checker recurses only on the schema (at most three levels), never on the input: deeper nesting in the file is a kind error at the first unexpected bracket.

`ron` then parses exactly the checked text with a recursion limit of 16 into types with `deny_unknown_fields` whose integers are `i64`, so `ron` cannot fail on range; ranges, patterns, uniqueness, and the schema version are validated on the parsed result. Every rejection is a `DataError { file, field, kind }` printed as `data file <file>: <field>: <reason>`, where the field is a path such as `units[2].speed`, and syntax errors carry the line and column. A `ron` error on checked text would be reported as `DataErrorKind::Parse` rather than ignored; the seeded mutation test asserts that this never happens on its 4,000 cases.

Units file, schema version 1 (`sim::load_unit_definitions`; production file `assets/data/units.ron`, test fixture `crates/sim/tests/fixtures/data/units.ron`): `(schema_version: 1, units: [(id: "...", speed: N, sprite: "..."), ...])` with 1 to `MAX_UNIT_TYPES` = 64 unit types; `id` and `sprite` are 1 to 32 characters from `a`-`z`, `0`-`9`, `_`, starting with a letter (a sprite name becomes a file name, so nothing else is allowed), ids are unique; `speed` is 1 to `MAX_UNIT_SPEED` = 128 subcells per tick. Fields of later milestones (hit points, weapons, ...) are added with a schema version increment (PLAN.md section 10).

Limits of the claim: the subset was derived from the source of `ron` 0.12.2 and is verified by `crates/sim/tests/m2_definitions.rs` (bounded by a 64 MiB allocator budget and a 30 s limit per load, as in D15); it is not a proof that `ron` cannot fail on every input inside the subset.

### D19. Units, commands, pathfinding, and movement in `sim`

1. **Positions.** `SubPos` (subcells, 256 per cell, centre of cell c at `c·256 + 128`), `SubPos::cell` by Euclidean division. A unit has `Mobile { speed, cell, step }`, `Position`, `Facing`, `Owner`, `Unit { kind }`, and `Orders`. A step goes from the centre of `cell` to the centre of a neighbour; its length is 256 subcells orthogonally and 362 diagonally (256·√2 rounded), and the position is the integer interpolation between the two centres (exact at both ends). Each tick a unit covers exactly `speed` subcells while it is not blocked, carrying speed left over at a centre into the next step (PLAN.md section 5: cells per second = speed × 20 / 256).
2. **Occupancy (I6).** `Grid` holds one occupant per cell. A unit holds its cell; before it starts a step it reserves the target cell, and it releases the cell it left on reaching the next centre, so it holds at most two cells and no cell is held twice. A diagonal step is not started while another unit holds both cells it passes between (that unit is crossing the same corner the other way). On an anti-diagonal step the midpoint lies exactly on the corner shared with two other cells; the I6 test therefore checks that a unit's position lies in the closed squares of the cells it holds.
3. **Commands.** `Command::Move { units, target }` and `Command::Stop { units }` (at most `MAX_UNITS_PER_COMMAND` = 1,024 units). Validation in this order: at least one unit, not too many; then each named unit in the order given, checked for repetition (named before in the same command), then existence, then ownership by the issuing player; then the target is inside the map. The first failure is the reason, so `[unknown X, Y, Y]` is rejected with `UnknownUnit(X)` and `[Y, Y, unknown X]` with `DuplicateUnit(Y)` (amended in M2 remediation round 1, P2-4: this item previously listed "no unit twice" as a check of its own before existence and ownership, which the code never did). Any failure rejects the whole command with `SimEvent::CommandRejected { player, command, reason }` and changes no state. Visibility is not validated before M4: there is no fog of war yet, so every cell is visible to every player. `Stop` clears the order; a unit between two cells completes its step. A new `Move` replaces the current order; a unit between two cells continues to the next centre and plans from there.
4. **Group destinations.** For a valid `Move`, `pathfinding::destination_cells` takes the passable cells nearest to the target by path cost over passable terrain (a Dijkstra search from the target, which also works for a target on rock; cells cut off from the target are never chosen), skipping cells held by idle units outside the group. The units, ordered by octile distance to the target and then id, receive the cells in that order, so every unit of a group has its own destination cell. A unit for which no cell remains stops with `MoveFailed { NoDestination }`. (An alternative that fills the cells from the far side of the target first was measured on the fixture scenarios and was not better; the simpler order was kept.)
5. **A\* (PLAN.md section 5, equation 5).** Step costs 10 and 14, no diagonal step between two blocked orthogonal neighbours, octile heuristic, ties broken by (f, h, cell index), so paths are deterministic. Requests are served in the `Pathfinding` stage, at most `PATH_REQUESTS_PER_TICK` = 16 per tick, oldest first and then by unit id. A path starts at the cell whose centre the unit reaches next. No path over terrain ends the order with `MoveFailed { Unreachable }`.
6. **Blocking and repathing.** A unit at a centre whose next cell has become impassable (or whose diagonal is cut by terrain) requests a new path at once. One whose next cell is held by another unit waits; after `REPATH_AFTER_BLOCKED_TICKS` = 8 ticks it requests a path that avoids units, chosen in this order: around all waiting units (idle, or waiting at a centre); else, if it is in a wait-for cycle (following the holder of each unit's next cell leads back to it) and has the highest id among the cycle's members that have a free neighbouring cell, a step aside to the free neighbour nearest to its destination (exactly one member gives way); if no member can step aside, the other members count as lasting obstacles; else around lasting obstacles (idle units) only, queueing behind units that will move on; else, since lasting obstacles close every way to the destination, the reachable cell nearest to the destination becomes the new destination (`pathfinding::nearest_reachable`; `MoveOrder::retargets` counts this). How an order ends when it cannot get closer is D21 (M2 remediation round 1), with the stalled ticks and paths it counts made independent of unit speed by D22 (M2 remediation round 2); the original rule here, "a unit that does not advance for `MAX_STALLED_TICKS` = 200 ticks ends its order with `MoveFailed { Blocked }`", did not end orders whose units kept moving without getting closer (M2 review 1, P1-1).
7. **Events.** `SimEvent` is a Bevy `Message` (A1) with the variants `CommandRejected`, `MoveCompleted { unit, retargeted }` (the unit reached its destination; `retargeted` is `true` when that is the cell chosen by item 6's retarget rather than the destination the order assigned, D21 item 4), and `MoveFailed { unit, reason }`. `SimPlugin` inserts `Messages<SimEvent>` as a plain resource (not registered for Bevy's per-frame update), and the first system of each tick calls `Messages::update` (`bevy_ecs-0.19.1/src/message/messages.rs:193`). After a tick, `iter_current_update_messages` (`messages.rs:265`) yields exactly that tick's events, and a `MessageReader` sees the last two ticks.
8. **Systems.** `SimSet::Commands`: `advance_events`, then `process_commands` (chained); `SimSet::Pathfinding`: `find_paths`; `SimSet::Movement`: `move_units`. All iterate units in ascending `SimId` order or use ordered collections (`BTreeMap`, `BTreeSet`), never query or hash order (I4).
9. **Hash (I3; D4 and D11 extended).** After each entity's id: a flag byte, and for a unit its owner, kind, speed, cell, step (flag, target, progress, length), position, facing, and order (flag, destination, path state, path cells, counters, and from D21 the remaining cost, best cost, and paths since progress; from D22 a requested path state also carries its `queued` flag). Each grid cell's flag byte gains bit 2 (occupied), followed by the occupant's id when set. The definitions are hashed after the grid.
10. **Setup API.** `spawn_unit` (checks the unit type, bounds, passability, and occupancy; typed `SpawnError`) and `Grid::set_terrain` are scenario and match setup, like replacing the grid (D11): the game uses `spawn_unit` for the starting units before the first tick, and the tests use both to place units and obstacles. During a match, units are created by production (M3) and terrain changes through buildings (M3).
11. **Integer arithmetic (I12).** Positions and costs are bounded by the map size (at most 256 cells); the helpers that could see arbitrary values (`SubPos::centre_of`, `octile_distance`, step offsets) saturate, counters use `saturating_add`, and the tick counter wraps (D17).

### D20. Units in the game

1. **Sprite sheets.** One PNG per sprite name at `assets/sprites/units/<sprite>.png`: eight 64×64 frames side by side (512×64), frame i showing facing `Direction::ALL[i]` (east, south-east, south, south-west, west, north-west, north, north-east in cell axes; east is down-right on screen). The ground point under the unit lies at pixel (32, 44) of a frame, so the sprite is drawn 12 px above the unit's projected position. The placeholders are generated by `cargo run -p game --example gen_unit_sprites` (light grey, tinted per owner with `Sprite::color`); `crates/game/tests/m2_unit_sprites.rs` checks that the committed files match the generator, pass the PNG subset, and point each frame in its facing's screen direction.
2. **Checked bytes (I11, D16).** Every sprite sheet named by the definitions is checked with the tileset image's PNG subset (`tileset_image::check_png_file`, generalised from `check_tileset_image`) at the size 512×64 before the match starts (cross validation of the sprite references, PLAN.md section 5), and only the checked bytes are registered in `CheckedAssets`. A missing or invalid sheet ends the game with exit code 1 and an error naming the units file, the `units[i].sprite` field, the image, and the reason. The image reader now rejects anything that is not a regular file before opening it, which closes M1 review 3 P2-1 for tileset images and sprite sheets (the map reader in `sim` is unchanged).
3. **Drawing.** Each simulation unit gets a separate view entity (`UnitView`), so the game never writes simulation components. Its position is interpolated between the unit's position before and after the last tick with `Time<Fixed>::overstep_fraction` (`bevy_time-0.19.1/src/fixed.rs:205`), projected with equation 7, and drawn at z = 1 + (u + v)/1024 (in front of the map's layers at z = 0, and units lower on the screen in front). `Sprite::rect` (`bevy_sprite-0.19.1/src/sprite.rs:39`) selects the facing's frame.
4. **Starting units.** Until match setup in M6, the game spawns 12 light tanks and 6 scouts for player 0 (the human) around cells (10, 28) and (14, 34), and 5 light tanks and 3 scouts for player 1 around (50, 30) and (52, 36) of the demo map, each on the free cells nearest to its anchor. A map on which some group does not get all of its cells (an anchor outside the map, or too few free cells reachable from it) is a setup error that ends the game with exit code 1 (D21 item 5).
5. **Controls.** Left click selects the player's own unit under the cursor (the nearest body within 20×16 world units of the cursor), or clears the selection on empty ground; a left drag of at least 4 px selects the own units whose ground point lies in the box (`Rect::contains` is inclusive, `bevy_math-0.19.1/src/rects/rect.rs:249`); Shift adds to the selection instead of replacing it. Right click on a map cell orders the selection to move there; `S` stops it (WASD stays free for M5 hotkeys, D10). Each order is one command for the whole selection, pushed into `CommandQueue` (I5). Units of player 1 cannot be selected.

### D21. M2 remediation round 1: every move order ends, accurate events, complete match setup

Decisions taken in remediation of M2 review 1 (`docs/reviews/M2-review-1.md`). Evidence: the reviewer's probe crate (`simprobe`) rerun against the reviewed code and the fixed code, and the regression tests named in `docs/STATUS.md`.

1. **Progress, not movement, keeps an order alive (P1-1).** The rule of D19 item 6 reset the stall counter on any advance, so two units that take turns stepping aside for each other (a wait-for cycle in a dead end one cell wide) kept their orders forever while moving. Now `MoveOrder` records `remaining`, the path cost (10/14 per step) of the rest of a path that leads to the destination (`None` for a step aside or while waiting for a path), and `best`, the least `remaining` with which the unit has started a step during the order. An order ends with `MoveFailed { Blocked }` at the next cell centre when either:
   * `stalled_ticks` reaches `MAX_STALLED_TICKS` = 200: ticks without a step along a path that leads to the destination (a step aside does not count), not counting ticks spent waiting for the pathfinder; or (amended in M2 remediation round 2, D22: this counted every other tick, including the unit's own steps, which last up to 362 ticks at speed 1, and waits behind units that were moving on, so a unit of speed 1 failed every move longer than one cell (M2 review 2, P0-1); a stalled tick is now only a tick spent waiting at a cell centre for cells held by units that are not moving on)
   * `paths_since_progress` exceeds `MAX_PATHS_WITHOUT_PROGRESS` = 64: paths served since the unit last lowered `best` (real progress).

   Bound: `best` starts at `u32::MAX`, is reset to it only when terrain changes under the unit's path (the next cell became impassable or a diagonal is cut), and otherwise only falls, by at least 1 per real progress, so there are finitely many real progresses; between two of them the unit takes at most 65 paths; on each path it starts at most one step per path cell (at most 65,536) and waits at most 200 counted ticks between steps; a wait for the pathfinder is bounded by the number of units divided by `PATH_REQUESTS_PER_TICK` (requests are served oldest first); and a step takes at most 362 ticks. (This bound did not reconcile the 362-tick step with the 200-tick limit, which counted the ticks of the step; D22 item 5 re-derives it.) A retarget (D19 item 6) does not reset `best`, so the bound does not depend on how often units that stand still close the way; the retarget's destination is reachable around them and usually near the unit, and the step onto it leaves remaining cost 0, which is real progress unless the unit already stood on a cell of remaining cost 0.

   Measured with the reviewer's `simprobe livelock` (random 16×16 maps, 0–34 % rock, 5–64 units, random group orders for 1,500 ticks, then none): reviewed code, 2 of 300 scenarios never settled within 20,000 ticks; fixed code, 0 of 1,000, the slowest settling 1,032 ticks after the last command (744 within the first 300). The reviewed code left 7 of the 1,000 unsettled (seeds 62, 154, 524, 754, 818, 909, 928). Over the first 300 seeds `Blocked` events went from 174 to 134 and completions from 74,665 to 74,725; over all 1,000, from 456 to 398 and from 249,067 to 249,011. The two units of the reviewer's sealed room now end at ticks 775 and 791 (about 40 s of game time). The fixture scenarios of `m2_movement` give the same arrival times as before (540, 558, 651, 945 ticks).

   Alternatives rejected: ending an order after `MAX_STALLED_TICKS` without lowering `best` alone made unit 36 of the 50-unit acceptance scenario fail while it walked a legitimate 20-cell detour around the units that had arrived first; a limit of 32 paths ended livelocks twice as fast (about 400 ticks) but ended 4 of 200 units of a group order with `Blocked` (`simprobe budget 200`, against 1 with the reviewed code and 0 with 64).
2. **Waiting for the pathfinder is not being blocked (P2-2, partly).** Ticks in which the order's path request is still queued do not count toward `MAX_STALLED_TICKS`. With the reviewer's `simprobe budget` (walkers in a square block ordered across an open 256×256 map, 3,000 ticks, release), `Blocked` before → after, all of them units that never moved unless stated: 50 units, 0 → 0; 200 units, 1 (had moved) → 0, completions 195 → 196; 400 units, 52 → 0, completions 270 → 284; 1,024 units in one command, 831 → 45, completions 193 → 301; 4,000 units in four commands, 3,950 → 0. The remaining cost is throughput: with 4,000 units 3,245 still waited for a path after 3,000 ticks, and units deep inside a large crowd can still end with `Blocked` while waiting behind units that advance (recorded as debt).
3. **Hash (I3).** `hash_unit` appends, after the retarget count, the remaining cost (flag byte, then the value), `best`, and `paths_since_progress`, then `stalled_ticks` and `repaths` as before. Per-tick hash digests of the reviewer's `simprobe hash` (60 units, 3 seeds, 4,000 ticks) are identical in debug and release builds.
4. **Retargeted arrivals are reported as such (P2-1).** `SimEvent::MoveCompleted` gains `retargeted: bool`, `true` when `MoveOrder::retargets > 0`, so the interface (M5) and the AI (M6) can tell an arrival at the ordered cell from a stop at the reachable cell nearest to it. A field rather than a new `MoveFailure` reason keeps the event that ends such an order a completion, as D19 item 6 defines it.
5. **Match setup spawns all configured units or fails (P1-2).** `game::units::plan_starting_units` computes the cells of every starting group before any unit is spawned; a group that gets fewer cells than its count is a `StartError` naming the owner, unit type, anchor, and the cells found and required, which is logged and ends the game with exit code 1, as a `SpawnError` does. The smoke test requires exactly the configured number of starting units (`starting_unit_total()`), not merely more than zero.
6. **Same-frame input (P2-3).** `selection::issue_orders` runs after `map::update_hovered_cell` in `PostUpdate`, so a right click orders the selection to the cell under the cursor in that frame. A test builds `PostUpdate` from the game's plugins and asserts that Bevy reports no pair of game systems with conflicting access and no ordering (the reviewed code had exactly this one pair).

### D22. M2 remediation round 2: the rules that end an order do not depend on unit speed

Decisions taken in remediation of M2 review 2 (`docs/reviews/M2-review-2.md`, P0-1). Evidence: a probe crate outside the repository (`…/scratchpad/m2-remediation-2/probe`, built against the fixed `sim`, and `probe_base`, built against a `git archive` of the reviewed commit `4fa123b`), with unit types of speeds 1, 2, 3, 7, 24, 32, 100, 127, and 128, and the regression tests named in `docs/STATUS.md`.

1. **Class of defect: tick limits applied to waits whose length depends on unit speed.** A unit's speed is 1 to `MAX_UNIT_SPEED` = 128 subcells per tick (D18), so a step lasts from 2 ticks (orthogonal at 128) to 362 ticks (diagonal at 1). Every rule that ends an order or counts toward its end was checked against that range:
   1. `MAX_STALLED_TICKS` = 200 counted every tick in which the unit did not start a step toward its destination, including the ticks of its own steps. At speed 1 a step lasts 256 or 362 ticks, so a unit alone on an open map ended every move longer than one cell with `MoveFailed { Blocked }` at its first centre (P0-1). At speed 2 a diagonal step lasts 181 ticks, so any wait of 19 ticks after one ended the order.
   2. The same counter counted waits behind units that were moving on. A unit waits behind a unit in a step for up to that step's length (up to 362 ticks), and a unit further back in a queue waits as long behind a unit that is itself waiting. Measured on the reviewed code: a convoy of six in a one-cell corridor led by a unit of speed 1 ended all six orders as `Blocked`; with speed-3 followers, 2 of 6.
   3. `MAX_PATHS_WITHOUT_PROGRESS` = 64 counted the path a waiting unit asks for every `REPATH_AFTER_BLOCKED_TICKS` = 8 ticks, so it ended an order after about 512 ticks of waiting without real progress. With items 1 and 2 fixed alone, the three rearmost units of a convoy of six units of speed 1 still ended as `Blocked` with 64 paths: they could not start their first step for 512 ticks.
   4. Checked and not affected: `REPATH_AFTER_BLOCKED_TICKS` itself (when to look for a way around; only the counting of its paths was the defect), `PATH_REQUESTS_PER_TICK` (independent of speed), and the movement arithmetic (a unit alone arrives exactly at `ceil(path length / speed)` ticks for all nine probe speeds, 108 orders; at most one cell centre per tick because 128 < 256). In the game, the smoke test's `MOVE_CHECK_TICKS` = 40 requires only some movement, and a unit of speed 1 moves one subcell per tick from its first tick.
2. **Stalled ticks.** A tick counts toward `MAX_STALLED_TICKS` only when the unit waits at a cell centre for cells (its next cell, or for a diagonal step the two cells it passes between) held by units that are not moving on. A unit is moving on when it is in a step (which always ends within 362 ticks), or when it waits at a centre for a cell that is free or held by a unit that is moving on in turn: the chain of next cells is followed to the head of the queue. A unit waiting for the pathfinder keeps the next cell of its previous path. A chain that comes back to a unit already visited (a wait-for cycle), that ends at a unit without a path (idle, or ending its order), or whose next cell has become impassable is not moving on. The unit's own steps, a tick at a centre with no movement left, and waits for the pathfinder are not stalled ticks. `move_units` keeps every unit's activity in a `BTreeMap` that it updates as it moves the units in id order, so the result is deterministic (I3, I4).
3. **Paths that only confirm the way.** A request made while the unit waits behind units that are moving on carries `PathState::Requested { queued: true }`. A path served for it that equals the path the unit already follows does not count toward `MAX_PATHS_WITHOUT_PROGRESS`. Every other path counts as before, including identical paths served to a unit that waits behind units that stand still, so the D21 measure of livelocks is unchanged there.
4. **Unchanged.** Movement itself: which unit steps where and when, repath timing, and pathfinding. Every probe group, convoy, follow, and head-on scenario whose units all have speeds of 24 or more gives the same arrival times as the reviewed code (the acceptance scenario: 540, 558, 651, and 945 ticks, with 45, 39, 43, and 39 units at their assigned cells). Only the decisions that end an order differ.
5. **Bound (re-derives D21 item 1).** Real progress lowers `best`, finitely often. Between two real progresses an order takes at most 65 new paths (a confirmation of item 3 is not a new path and adds no step), so it takes a bounded number of steps, each of at most 362 ticks. Between two steps toward the destination it has fewer than `MAX_STALLED_TICKS` stalled ticks. Each wait for the pathfinder is bounded by the request queue. A wait behind units that are moving on lasts only while some unit is in a step or about to take a free cell. With no new command, every unit's order takes a bounded number of steps, and no step lasts more than 362 ticks, so such waits are bounded too. So every order ends. While new commands keep units moving through a unit's way, that unit keeps waiting instead of failing, because it is not blocked.
6. **Hash (I3).** A requested path state hashes its flag byte, `avoid_units`, `queued`, and the request tick. Per-tick hash digests of the probe's `hash` scenario (60 units of five speeds, 3 seeds, 4,000 ticks of random orders) are identical in debug and release builds (`f3748c5ee280f84c`).
7. **Alternatives rejected.** Each was measured with the probe's lone-unit, convoy (72 scenarios), diagonal-follow (12), and single-speed and mixed group (24) scenarios:
   * **Raising `MAX_STALLED_TICKS` above the longest step, to 400** (the reviewer's third option, applied to the reviewed code). The lone unit is fixed: 0 of 108 orders wrong. But `Blocked` remained in 11 of the 72 convoys (four of the six crawlers of the speed-1 convoy) and in 13 of the 24 groups, and the limit would be tied to the minimum speed.
   * **Resetting the counter at the end of a step toward the destination as well** (the reviewer's second option, applied to the reviewed code). The lone unit is fixed, but 24 convoys, 3 diagonal follows, and 14 groups still had `Blocked`.
   * **Items 2 and 3, but excluding only waits for the unit directly ahead when it is in a step.** 20 convoys and 8 groups still had `Blocked`; in the speed-1 convoy of six, the third to sixth units failed.
   * **Asking for a way around only once per queued wait.** This removes the repeated paths, but it changes movement at normal speeds (acceptance group on the wall map: 651 → 731 ticks).
   * **Never counting a confirming path.** The D21 livelocks then take twice as long to end (sealed room at speed 32: 792 → 1,544 ticks), because the unit waiting for the one that gives way no longer uses up its paths.
8. **Measured** (release, the reviewed code → the fixed code):
   * A unit alone on the open map, nine speeds, 12 orders each: 10 of 108 wrong (all at speed 1) → 0 of 108.
   * Convoys of 2 and 6 in a corridor, leaders of nine speeds and followers of speed 1, 3, 32, or 128: 32 of 72 scenarios had `Blocked` → 0. Two units following each other on the diagonal of the open map: 6 of 12 → 0. Head-on passages in a corridor: 7 of 16 → 0.
   * 50 units of one speed to a target:
     * on the arena or the wall map at speeds 1, 2, 3, and 7: 50, 50, 38, 41, 35, 37, 7, and 7 `Blocked` → 0;
     * from speed 24 up: unchanged, no `Blocked`.
   * Mixed groups (speeds 1 and 128; 1 and 32; 3, 100, 1, and 128), each on the arena and the wall map: 41, 43, 40, 43, 30, and 31 `Blocked` → 0.
   * The reviewer's random-map generator over seeds 0–999 (speeds 32 and 100): 0 unsettled; slowest settle 1,033 → 996 ticks; `Blocked` 398 → 116; completions 249,011 → 249,112.
   * Over seeds 0–199 with speed 1 alone, 1 and 128, 1 and 32, 3 and 100, 2 and 127, and 128 alone: every order ends (slowest 35,959 ticks at speed 1 alone); `Blocked` falls from 8,003, 6,021, 5,870, 2,674, 3,711, and 28 to 124, 140, 173, 70, 100, and 24.
   * The D21 livelocks (the sealed room and the first two dead-end placements) end:
     * at speed 32, at 778–816 ticks (previously 778–792);
     * at speed 128, at 505–506 ticks (unchanged);
     * at speed 1, at 17,146–17,928 ticks (previously both orders failed after the first step).

     The number of turns is the same at every speed, so the time grows with the time a step takes. The third dead-end placement ends within 33, 19, and 777 ticks at these speeds.
   * Crowds of 50, 200, 400, and 1,024 units of speed 32 on an open 256 × 256 map, 3,000 ticks: the same `Blocked` counts (0, 0, 1, and 0) and completions (except 373 → 374 for 400 units). Mean tick time for 1,024 units: 3.25 → 3.34 ms (the chain walk of item 2).
   * 200 units of speed 1 for 20,000 ticks: `Blocked` 200 (133 never moved) → 0.

## M3 (2026-10-04): economy and construction

Evidence sources as for M2: the sources of the pinned crates in `~/.cargo/registry/src/` (Bevy 0.19.1, `ron` 0.12.2, `serde` 1.0.229), `Cargo.lock` (unchanged by M3: no dependency was added), and runs on the development machine. Every API named below is exercised by code that compiles and runs in M3.

### A1 follow-up: Bevy APIs used by M3

| Concept | Bevy 0.19.1 API | Evidence |
|---------|-----------------|----------|
| Deferred entity changes in simulation systems | `Commands::spawn` returns `EntityCommands`, whose `id()` is the reserved entity (`bevy_ecs-0.19.1/src/system/commands/mod.rs:398, 1324`); `EntityCommands::insert`, `despawn` | `economy_commands.rs`, `economy.rs`; `m3_economy::later_commands_of_a_tick_see_the_buildings_placed_before_them` (cited as `two_placements_in_one_tick_see_each_other` until M7, M3 review 1 P2-3) and `a_placed_refinery_delivers_its_free_harvester` observe the effects |
| Sync points between stages | `ScheduleBuildSettings::new()` enables `auto_insert_apply_deferred` (`schedule/schedule.rs:1605, 1629`), which inserts `ApplyDeferred` on dependency edges leaving a system with deferred parameters and marks it `ambiguous_with_all` (`schedule/auto_insert_apply_deferred.rs:50-57`); the executor applies what is left at the end of the run (`schedule/executor/single_threaded.rs:166`) | Buildings placed in `SimSet::Commands` exist in `SimSet::Economy` of the same tick (the free harvester leaves in the placement tick) |
| Why the new systems are not exclusive | An exclusive system conflicts with every system it is not ordered against (`schedule/node.rs:608-610`), so it would turn the systems that `m0_bootstrap::stages_run_once_per_tick_in_plan_order` adds to each stage into an ambiguity error (invariant I4, D3). All M3 systems are ordinary systems. | That test passes unchanged |
| Grouped system parameters | `#[derive(SystemParam)]` (`system/system_param.rs:67-110`) | `economy_commands::EconomyParams` |
| Window title, key sets | `Window::title` (`bevy_window-0.19.1/src/window.rs:174`), `ButtonInput::any_just_pressed` (`bevy_input-0.19.1/src/button_input.rs:193`) | `game::buildings::show_status`, `game::construction` |

### D23. Economy definitions: the economy file, schema version 1 (invariants I8, I10, I11)

1. **A file of its own.** Buildings, the production terms of units, harvesters, and refineries are defined in `assets/data/economy.ron` (fixture `crates/sim/tests/fixtures/data/economy.ron`), loaded by `sim::load_definitions(units_file, economy_file)` or `Definitions::with_economy`, which resolves its references to unit types against the units file. The units file keeps schema version 1 unchanged: adding the economy fields to it would have required a schema version increment and changed the expectations of the M2 data tests (for example `m2_definitions::schema_versions_other_than_1_are_rejected`). PLAN.md section 4 lists the data files by subject ("units, buildings, ..."); this file holds the buildings and the economy terms of units.
2. **Subset and schema.** The file passes the RON subset checker of D18 driven by its schema (`economy_defs::ECONOMY_FILE_SHAPE`) before `ron` parses exactly the checked text into types with `deny_unknown_fields`. The schema nests five levels (names inside lists inside structs inside lists inside the file's struct); the checker still recurses only on the schema. Every field is required; there are no silent defaults.
3. **Fields and ranges** (checked on the parsed values, at both ends, with the field path in the error): `placement_distance` 1 to `MAX_PLACEMENT_DISTANCE` = 32 cells; `buildings` 1 to `MAX_BUILDING_TYPES` = 64 entries, each with `id` and `sprite` (the name rules of D18), `width` and `height` 1 to `MAX_FOOTPRINT_SIDE` = 8, `cost` 0 to `MAX_COST` = 1,000,000 credits, `build_time` 1 to `MAX_BUILD_TIME` = 72,000 ticks, `producer` (a building id), `prerequisites` (at most `MAX_PREREQUISITES` = 8 distinct building ids), `power_produced` and `power_consumed` 0 to `MAX_POWER` = 10,000, `sell_value` 0 to the building's cost, `free_units` (at most `MAX_FREE_UNITS` = 8 unit ids); `units` (unit id, cost, build time, producer, prerequisites; each unit at most once); `harvesters` (unit id, `capacity` 1 to `MAX_ORE_CAPACITY` = 100,000, `harvest_rate` 1 to the capacity); `refineries` (building id, `credits_per_ore` 1 to `MAX_CREDITS_PER_ORE` = 1,000).
4. **Cross validation (PLAN.md section 5).** Every reference names a defined building or unit type (`DataErrorKind::UnknownReference`); ids, prerequisites within a list, and the units of the `units`, `harvesters`, and the buildings of `refineries` lists are unique (`DuplicateId`); the prerequisites of the buildings are acyclic, checked by an iterative depth-first search and reported at the prerequisite list that closes the cycle, naming it (`PrerequisiteCycle`). The game checks that every building sprite exists (D27).
5. **Why these limits.** They keep every derived quantity inside its integer type (invariant I12): a player's power is at most 65,536 buildings (every building covers at least one cell of a 256 × 256 map) × 10,000 < `i32::MAX`; a delivery pays at most 100,000 × 1,000 = 10^8 credits; equation 3's per-tick charge is at most 10^6. The sell value is at most the cost, so building and selling never creates credits.
6. **Hash.** `Definitions::hash_into` appends the economy definitions after the unit definitions.

Limits of the claim: as D18; verified by `crates/sim/tests/m3_definitions.rs` (bounded as in D15, with a seeded mutation test of 2,000 cases that asserts that `ron` never rejects a file the checker accepted).

### D24. Buildings and placement (invariant I6)

1. **Footprint.** A building covers `width` × `height` cells from its origin, the cell with the smallest coordinates (`sim::Footprint`, computed in `i64` so that no origin a command can carry overflows). `Command::PlaceBuilding { building, origin }` names the origin.
2. **Grid.** `Grid` gains a building layer: the building covering each cell. `Grid::is_passable` is false for covered cells whatever their terrain, so pathfinding routes around buildings, a unit whose path crosses a new building repaths (D19 item 6), and no unit can reserve a covered cell (`Grid::claim`), which is the second half of I6. The hash writes bit 3 of each cell's flag byte and the building's id (D11 extended).
3. **Placement rules (PLAN.md section 7, M3),** checked in this order, the first failure being the reason (`RejectReason::Placement(PlacementError)`): every cell inside the map (`OutsideMap`); then, cell by cell in row-major order, buildable terrain (`NotBuildable`), no building (`Overlap`), and no unit holding or entering the cell (`Occupied`); then the distance rule (`TooFar`). The distance is the Chebyshev distance between the nearest cells of the new footprint and a footprint of the player's buildings (1 when they touch, also at a corner), and it must be at most `placement_distance`; with no building of the player the placement is `TooFar { distance: None }`.
4. **Components.** A building entity has `SimId`, `Owner`, `Building { kind, footprint, pending_units }`, and, if its type produces units, `Producer { rally }`. `pending_units` are the free units of a building placed during the match (a refinery's harvester), which leave it as soon as a ring cell is free.
5. **Ring and exits.** The ring of a footprint is the cells at Chebyshev distance 1. A unit leaves a building at the free, passable ring cell nearest to the rally point (octile distance, then row, then column), or without a rally point at the ring cell lowest on screen (largest x + y). While no ring cell is free the unit waits (a complete item stays at the head of its queue, a free unit stays pending).
6. **Selling** removes the building (deferred to the end of the `Commands` stage), frees its cells at once, and refunds its `sell_value`.
7. **Setup API.** `sim::spawn_building` places a building as scenario or match setup: the footprint rules apply, the distance rule does not, and no free units are delivered. During a match, buildings are produced and placed only through commands (I5).

### D25. Players, power, production, and the economy commands (invariant I7)

1. **Players.** The `Players` resource holds, per player, credits (`i64`, never negative), the power produced and consumed (recomputed from the player's buildings as the first system of the `Economy` stage), and one production queue per producing building type. `Players::add` adds a player as setup (D11). Faction and visibility arrive with the milestones that use them (M7, M4).
2. **Queues.** `QueueProduction` appends an item to the queue of its producing building type (at most `MAX_QUEUE_LENGTH` = 16 items). Only the head progresses, only while the player owns a building of that type, and production is processed by player id, then building type, so the credits of a player go to its queues in a fixed order (I4).
3. **Equation 3.** On a progressing tick the head of cost `C` and build time `T` is charged `C / T`, and on its last tick `C / T + C mod T`; the total charged at completion is exactly `C` (I7). If the player's credits are below the tick's charge, the item does not progress and nothing is charged, so credits never fall below zero. Credits exactly equal to the charge suffice.
4. **Low power** (consumed > produced; equal is not low): an item progresses only on even ticks (the index of the tick in progress, `SimTick`). That defensive structures do not fire under low power is M4's.
5. **Completion.** `SimEvent::ProductionComplete` is written on the tick of the last progress. A complete building stays at the head of its queue until `PlaceBuilding` places it (or it is cancelled); a complete unit leaves the first producing building of the player (in id order) with a free ring cell (D24 item 5), moving to the building's rally point if one is set; a harvester starts its cycle instead (D26).
6. **Cancelling** removes the most recently queued entry of the item and refunds exactly what was charged for it (a complete building: its full cost; an item not started: nothing).
7. **Validation** (`economy_commands.rs`; every check before any change, so a rejected command changes no state): the issuing player exists (`UnknownPlayer`); `QueueProduction`: the item has production terms (`UnknownItem`), the player owns each prerequisite (`MissingPrerequisite`, checked when queued only) and a producer (`NoProducer`), the queue is not full (`QueueFull`); `CancelProduction`: producible and queued (`NotQueued`); `PlaceBuilding`: the type is defined, a complete building of that type is at the head of its queue (`NotReady`), and D24 item 3; `SellBuilding`: the building exists (`UnknownBuilding`, also for a unit's id) and is the player's (`NotOwned`); `SetRallyPoint`: as `SellBuilding`, its type produces units (`NotAProducer`), and the cell is inside the map (`OutsideMap`). Unit commands do not require a registered player, as in M2.
8. **Same-tick consistency.** Entity changes from the `Commands` stage are deferred to the sync point before `Economy`, but the commands of one tick see each other: `process_commands` keeps an index of the buildings that it updates on every placement and sale, and the grid, credits, and queues change at once. A second placement on the cells of a building placed earlier in the tick is `Overlap`, and a second sale of the same building is `UnknownBuilding`.
9. **Events.** `ProductionComplete`, `ProductionCancelled { refund }`, `UnitSpawned { unit, building }`, `BuildingPlaced`, `BuildingSold { refund }`, `OreDelivered { ore, credits }`, `HarvesterIdle { reason }`.
10. **Hash (I3).** Each entity gains a harvester flag byte and cycle (D26) and a building flag byte with its owner and `building::hash_building` (kind, footprint, pending units, rally point); the `Players` are hashed last (id, credits, power, then each queue's items with kind, progress, and credits charged).
11. **Stage order.** `SimSet::Economy` runs `update_power`, `advance_production`, and `harvest`, chained, between the AI and pathfinding stages (PLAN.md section 4).

### D26. The harvester cycle (PLAN.md section 5, equation 4)

1. **Harvester.** A unit whose type is listed in the economy file's `harvesters` carries `Harvester { load, state, attempts }` from its spawn; `state` is `Idle`, `Seeking`, `ToOre(cell)`, or `ToRefinery { refinery, dock }`. Movement uses ordinary move orders, so harvesters obey reservation and the end rules of D21 and D22.
2. **Each tick, in id order** (`harvest.rs`): a harvester whose target refinery no longer exists turns to the nearest other refinery of its owner (a new order replaces the old; a unit between two cells completes its step first) or stops with `HarvestIdle::NoRefinery`, keeping its load, even while moving. Otherwise, a harvester that is moving does nothing. At a cell centre without an order: in `ToRefinery`, it unloads if it stands in the refinery's ring, else sets out again; in `Seeking` or `ToOre`, it harvests if its cell holds ore and it has room, else goes to a refinery if full, else sets out for the nearest ore, else goes to a refinery if it carries ore, else stops (`NoOre`).
3. **Harvesting** removes `min(harvest_rate, ore in the cell, room left)` per tick, so the ore falls exactly to zero and the load never exceeds the capacity. Full, the harvester sets out for a refinery in the same tick.
4. **Nearest ore**: the passable cell with ore nearest by path cost over terrain (`pathfinding::nearest_cell`, the Dijkstra of `destination_cells`), not held by another unit and not the target of another harvester, so harvesters spread over a field. Ore that is not reachable over terrain is never chosen, so a harvester with no reachable ore stops at once and asks for no paths.
5. **Refineries.** The dock is the free ring cell of a refinery of the harvester's owner nearest by path cost (a held ring cell if none is free); unloading takes one tick and pays `load × credits_per_ore` (saturating, I12). A refinery whose owner is not a player of the match does not accept ore.
6. **Bounded attempts.** Every set-out (for ore or a refinery) counts; harvesting or unloading resets the count. After `MAX_HARVEST_ATTEMPTS` = 8 set-outs without either, the harvester stops (`HarvestIdle::Blocked`). Set-outs, not ticks, are counted, so the rule does not depend on unit speed (the class of M2 review 2 P0-1).
7. **Commands.** `Harvest { units, target }` (each unit must be a harvester: `NotAHarvester`; target inside the map) starts the cycle at the cell with ore nearest to the target not taken by another harvester, or, if there is none, from where the harvester stands; a full harvester goes to a refinery first. `Move` and `Stop` stop the cycle (`Idle`, load kept). A stopped harvester waits for a `Harvest` command; it does not resume by itself when ore or a refinery appears.
8. **Hash.** Load, a state tag with its cell or refinery and dock, and the attempts.

### D27. Economy in the game: sprites, match setup, interim controls

1. **Building sprites.** One PNG per sprite name at `assets/sprites/buildings/<sprite>.png`, `(w + h) × 32` by `(w + h) × 16 + 32` pixels for a footprint of `w × h` cells: a box whose base is the footprint's diamond and whose roof is that diamond lifted by 32 px, light grey for tinting, with 0 to 4 roof bands per type. Generated by `cargo run -p game --example gen_building_sprites`; `crates/game/tests/m3_building_sprites.rs` checks the committed files against the generator and the PNG subset. Before the match starts every building type's sprite is checked with the PNG subset at its footprint's size (D16, D20 item 2) and only the checked bytes are registered in `CheckedAssets`; a missing or wrongly sized sprite ends the game with exit code 1, naming the economy file, the `buildings[i].sprite` field, and the image. The harvester gets a placeholder sheet from `gen_unit_sprites` (a broad box).
2. **Drawing.** A building's view is placed once at its footprint's centre raised by 16 px, at depth `1 + (u + v)/1024` of the footprint's centre (the units' scale, D20 item 3), tinted with its owner's colour, and removed when the building is sold.
3. **Match setup until M6.** Each player starts with `STARTING_CREDITS` = 5,000 credits and a construction yard: the human's at (15, 37), near its starting units and the ore field around (22, 44), and the opponent's at (52, 42), near the field around (51, 51). Like the starting units (D20 item 4) these values are match setup in game code until the match setup of M6 moves them to data; the definitions themselves are in the data files (I10). A setup error ends the game with exit code 1.
4. **Interim controls until the sidebar of M5** (`game::construction`): `1` to `9` queue the entries of the production menu (building types in file order, then producible unit types in units-file order), Shift cancels; `B` places the ready building centred on the hovered cell; `Delete` or `Backspace` sells the player's building under the cursor; `R` sets the rally point of every unit-producing building of the player; a right click on ore with harvesters selected sends them to harvest there and the other selected units move there. A yellow outline shows the ready building's footprint at the cursor (validity colouring is M5's). The decisions are pure functions with unit tests; the systems push commands into `CommandQueue` only (I5). The window title shows the player's credits and power. (Superseded in M5 by D35: production is driven from the sidebar, the digit keys select control groups, `B` enters placement mode with a validity preview instead of placing at once, and `Delete`/`Backspace` and `R` are kept as hotkeys. The window title is kept beside the sidebar.)
5. **Smoke test** (D6 amended): it also requires both starting buildings, queues the first building the player can produce (no prerequisites, producer owned), fails if any of its commands is rejected, and, if at least `PRODUCTION_CHECK_TICKS` = 2 ticks ran after the order, fails unless credits were charged for it.

## M4 (2026-10-04): combat and fog of war

Evidence sources as for M3: the sources of the pinned crates in `~/.cargo/registry/src/` (Bevy 0.19.1, `bevy_ecs_tilemap` 0.19.0, `bevy_ecs_tiled` 0.13.4, `ron` 0.12.2, `serde` 1.0.229), the Rust 1.98.1 standard library documentation installed with the toolchain, `Cargo.lock` (unchanged by M4: no dependency was added), and runs on the development machine. Every API named below is exercised by code that compiles and runs in M4.

### A1 follow-up: APIs used by M4

| Concept | API | Evidence |
|---------|-----|----------|
| Tinting map tiles | `bevy_ecs_tilemap::tiles::TileColor(pub Color)` on every tile entity, with `TilePos` (`bevy_ecs_tilemap-0.19.0/src/tiles/mod.rs:15, 79`); the renderer re-extracts a tile when its `TileColor` changes (`render/extract.rs:194-202`) and multiplies the texture by it (`render/shaders/common.wgsl:63, 69`) | `game::fog::tint_tiles`; `bevy_ecs_tiled` spawns every tile with a default `TileBundle` (white, `bevy_color-0.19.1/src/color.rs:512`) and the marker `TiledTile` (`bevy_ecs_tiled-0.13.4/src/tiled/map/spawn.rs:392`, `tiled/tile.rs:22`), both re-exported by `bevy_ecs_tiled::prelude` |
| Tile to cell | `TilePos { x, y: height − 1 − y }` (D7) inverted by `game::projection::tile_cell` | `projection::tests::tile_cell_inverts_the_tilemap_placement_of_every_cell` (every cell of five map sizes, through `TilePos::from_world_pos`) |
| Hiding views | `Visibility::{Inherited, Hidden}` (`bevy_camera-0.19.1/src/visibility/mod.rs:83`), a required component of `Sprite` (`bevy_sprite-0.19.1/src/sprite.rs:16`) | `game::units::place_unit_views`, `game::buildings::update_building_views`; the smoke test checks every view against the vision |
| Disjoint queries | `Without<T>` makes two queries that write `Visibility` disjoint, so Bevy's ambiguity check does not report them | `selection::tests::no_two_game_systems_share_state_in_post_update_without_an_order` failed with the two view systems unordered and passes with the filters; `fog::tests::the_fog_systems_share_no_state_with_other_game_systems_without_an_order` |
| Drawing shots | `Gizmos::line_2d` (`bevy_gizmos-0.19.1/src/gizmos.rs:726`), `Gizmos::circle_2d` (`circles.rs:167`) | `game::fog::draw_combat` |
| Projectile flight time | `u64::isqrt` (stable since Rust 1.84.0; `share/doc/rust/html/std/primitive.u64.html` of the 1.98.1 toolchain) | `sim::combat`; `m4_combat::a_projectile_hits_after_its_flight_time` |

### D28. Combat definitions: the combat file, schema version 1 (invariants I8, I10, I11, I12)

1. **A file of its own.** Armour classes, warheads, weapons, and the hit points, armour, sight, and weapon of every unit and building type are defined in `assets/data/combat.ron` (fixture `crates/sim/tests/fixtures/data/combat.ron`, with `units_combat.ron` and `economy_combat.ron`), loaded by `Definitions::with_combat` after the economy file, or `sim::load_match_definitions(units, economy, combat)`. As with the economy file (D23 item 1), adding these fields to the units or economy file would have required a schema version increment and changed the expectations of the M2 and M3 data tests; the units and economy files keep schema version 1 unchanged, and the M2 and M3 fixtures are untouched. `Definitions::with_economy` removes combat definitions loaded before it, because they refer to the previous building types; the order is documented on both functions and tested (`m4_definitions::an_economy_file_loaded_after_the_combat_file_removes_it`).
2. **Subset and schema.** The file passes the RON subset checker of D18, driven by `combat_defs::COMBAT_FILE_SHAPE`, before `ron` parses exactly the checked text into types with `deny_unknown_fields`. The schema nests six levels (a warhead's percentages inside structs inside the `versus` list inside the warhead struct inside the `warheads` list inside the file's struct); the checker still recurses only on the schema, and `ron`'s recursion limit (16) stays a second line. Every field is required.
3. **Fields and ranges** (checked on the parsed values at both ends, with the field path in the error): `armour_classes` 1 to `MAX_ARMOUR_CLASSES` = 16 names; `warheads` up to `MAX_WARHEADS` = 32, each with `versus`, one `(armour, percent)` per armour class, `percent` 0 to `MAX_VERSUS_PERCENT` = 1,000; `weapons` up to `MAX_WEAPONS` = 32, each with `damage` 1 to `MAX_DAMAGE` = 100,000, `range` `MIN_WEAPON_RANGE` = 384 to `MAX_WEAPON_RANGE` = 4,096 subcells, `reload` 1 to `MAX_RELOAD` = 1,200 ticks, `warhead`, and `projectile_speed` 0 (instant) or `MIN_PROJECTILE_SPEED` = 128 to `MAX_PROJECTILE_SPEED` = 4,096 subcells per tick; `units` and `buildings`, each entry naming its type with `hit_points` 1 to `MAX_HIT_POINTS` = 1,000,000, `armour`, `sight` 0 to `MAX_SIGHT` = 16 cells, and `weapons`, a list of at most `MAX_WEAPONS_PER_TYPE` = 1 weapon (the RON subset has no optional values).
4. **Cross validation (PLAN.md section 5).** Every reference (armour class, warhead, weapon, unit type of the units file, building type of the economy file) is defined (`UnknownReference`); names follow D18; armour classes, warhead ids, weapon ids, the armour classes of one warhead, and the unit and building entries are unique (`DuplicateId`); and nothing is left to a default: every warhead lists every armour class, and every unit and building type has exactly one entry (new `DataErrorKind::MissingEntry { kind, id }`, reported at the list). With a combat file loaded, every unit and building therefore has hit points, armour, and sight.
5. **Why these limits.** The damage of one shot is at most `MAX_DAMAGE × MAX_VERSUS_PERCENT / 100` = 10^6 and fits an `i32`; the shots on one target in one tick are summed in `i64` with saturation, and hit points are clamped back into `i32` (I12). `MIN_WEAPON_RANGE` = 1.5 cells lets a unit at a cell centre reach a target at the centre of any neighbouring cell (362 subcells diagonally) and a building whose ring it stands in (181 from a ring corner), so every weapon can hit what its unit can walk up to (`m4_combat::the_shortest_range_reaches_every_neighbouring_cell_and_an_adjacent_building`). A weapon fires only at a target in range, so a projectile flies at most `MAX_WEAPON_RANGE / MIN_PROJECTILE_SPEED` = 32 ticks (`the_longest_range_with_the_slowest_projectile_flies_32_ticks`), and one weapon has at most 32 projectiles in flight; with at most one armed unit per cell and one building per cell of a 256 × 256 map, the projectiles in flight are bounded by 2 × 65,536 × 32 (about 200 MB of 48-byte entries in that pathological case, far below it in any real match). `MAX_SIGHT` = 16 bounds the cells a source reveals per tick to 33 × 33 (plus its footprint).
6. **Hash.** `Definitions::hash_into` appends the combat definitions after the economy definitions.

Limits of the claim: as D18; verified by `crates/sim/tests/m4_definitions.rs` (bounded as in D15, with a seeded mutation test of 2,000 cases that asserts that `ron` never rejects a file the checker accepted).

### D29. Combat in the simulation (PLAN.md section 5, equations 1 and 2)

1. **Components.** A unit or building of a type with combat terms gets `Health { hit_points }`, `Armour`, and `Sight` from every spawn path (`spawn_unit`, `spawn_building`, production, free units, `PlaceBuilding`), and `Weapon { kind, cooldown, target, order }` if its type has a weapon. Without a combat file (the M2 and M3 fixtures) no entity has them, so nothing fights and nothing sees.
2. **Stages.** `SimSet::Combat` runs `combat`, `SimSet::Cleanup` runs `remove_dead`, and `SimSet::Visibility` runs `update_visibility` (D30); all are ordinary systems (A1 follow-up of M3). Bevy's automatic sync point between the chained stages applies the despawns of `Cleanup` before `Visibility` (`m4_combat::a_destroyed_unit_stops_seeing_in_the_tick_it_dies`).
3. **Order within `Combat`.** First every projectile in flight counts down one tick; one that reaches 0 deals its damage to its target or, if the target no longer exists, writes `ProjectileExpired` and deals nothing. Then every armed unit and building, in `SimId` order, counts down its reload, chooses its target, and fires if the weapon is loaded (`cooldown` 0) and the target is in range; firing sets `cooldown = reload`, so a weapon fires every `reload` ticks, and writes `WeaponFired`. An instant weapon's damage counts in this tick; any other weapon adds a projectile with `flight = max(1, ceil(distance / speed))`, `distance` the integer square root of `dx² + dy²`, which always hits its target if the target still exists when it arrives (guided). Finally the tick's damage, summed per target, is subtracted in `SimId` order. All entities fire from the state at the start of the stage, so processing order does not decide which of two units that shoot each other fires first.
4. **Equations.** Equation 1 (`sim::in_range`): a target is in range when `dx² + dy² ≤ range²`, computed in `i128`; `dx` and `dy` are measured in subcells between the nearest points of the two entities: a unit is its fine position, a building the rectangle of subcells its footprint covers (`x·256` to `(x + w)·256 − 1`). Equation 2 (`sim::applied_damage`): `floor(base × versus / 100)` in `i64`, at least 1 if `versus > 0`, 0 if `versus = 0`.
5. **Target choice.** A unit with an `Attack` order fires only at its target. A unit with an `AttackMove` order, a unit without any order, and a building with a weapon keep their current target while it is a visible enemy in range that their weapon damages, else take the nearest visible enemy in range that their weapon damages (by squared distance, then `SimId`); "damages" means equation 2 gives more than 0, that is, the warhead's percentage against the target's armour is above 0 (amended in M4 remediation round 1, D32: acquisition previously ignored the percentage, so units and buildings engaged enemies they could not damage, M4 review 1 P1-1). "Visible" is the owner's vision as of the previous tick's `Visibility` stage (D30); a player who is not a player of the match sees nothing, so its units never acquire targets. A unit with a plain move order (including a produced unit on its way to a rally point) does not fire. A building with a weapon (a defensive structure) does not fire while its owner has low power (PLAN.md section 5, equation 3; `m4_combat::defensive_structures_hold_fire_under_low_power`).
6. **`Attack { units, target }`.** Each unit stops moving and aims at the target. In range, it stops (its move order ends, a unit between two cells completes its step) and fires when loaded. Out of range, it moves to the free cell nearest to it by path cost from whose centre the target is in range (`pathfinding::nearest_cell`; cells chosen by other units in the same tick are skipped, so attackers spread), and moves again whenever its destination is no longer in range of the target (the target moved). The order ends with `SimEvent::AttackEnded { reason }` when the target no longer exists (`TargetGone`), is no longer visible to the owner (`TargetHidden`), or no such cell is reachable or the unit has started `MAX_CHASES` = 8 such moves without firing in between (`OutOfReach`). Moves are counted, not ticks, so the rule does not depend on unit speed (the class of M2 review 2 P0-1): a chase without a shot ends after at most 8 moves, each of which ends by D21 and D22. An `Attack` order on a target in range lasts as long as the target exists and is visible, except that an order on a target the weapon's warhead deals 0 to ends with that unit's first shot at it, with `AttackEnded { reason: NoDamage }` (amended in M4 remediation round 1, D32: such an order previously lasted, firing, as long as the target stood).
7. **`AttackMove { units, target }`.** The units get distinct destinations as for `Move` (D19 item 4). An armed unit with an enemy in range that its weapon damages (item 5) stops and fights; with none, it moves (again) to its destination. The order ends when a move toward the destination ends; that move's `MoveCompleted` or `MoveFailed` reports how. A fight lasts at most until the target dies, leaves the range, or is no longer visible, because every shot at it deals at least 1 (amended in M4 remediation round 1, D32: a unit previously stopped for an enemy it could not damage and never moved on, M4 review 1 P1-1). Unarmed units simply move. Engagement uses the weapon's range, not the sight radius: an attack-moving unit does not chase (a chase can be ordered with `Attack`).
8. **`Move`, `Stop`, `Harvest`** end a unit's combat order and target; `Attack` and `AttackMove` end a harvester's cycle (as `Move` does, D26 item 7).
9. **Death and cleanup.** In `Cleanup`, every unit and building whose hit points are ≤ 0 is removed in `SimId` order: a unit frees its cell and the cell it was stepping into, a building uncovers its footprint, and `UnitDestroyed { player, unit }` or `BuildingDestroyed { player, building, kind, lost_units }` is written; `lost_units` lists the free units that were still waiting to leave the building, so their loss is reported (cf. M3 review 1 P2-1, which concerns selling and stays open). Orders that name a removed entity end in the next tick with an event (`AttackEnded`, `ProjectileExpired`, and for harvesters `HarvesterIdle` by D26). Power is recomputed from the remaining buildings in the next `Economy` stage.
10. **Hash (I3).** After a building's components each entity hashes a flag byte and its hit points, armour class, and sight, then a flag byte and its weapon (kind, cooldown, target, order with its fields); after the `Players` the projectiles in flight are hashed in order. A 20 against 20 battle (`m4_combat::twenty_against_twenty_fight_to_a_conclusion_under_fog_with_a_stable_hash`) gives the same final hash (`05af3b1ac550cf16`) in debug and release builds (I12).
11. **Integer arithmetic (I12).** Extents and gaps are `i64` (at most 2^40); range checks `i128`; distances `u64` with saturation; damage sums, hit points, cooldowns, chase counts, and flight times saturate or are clamped.

### D30. Visibility, shroud and fog, and hiding of unseen enemies (PLAN.md section 5, equation 6)

1. **Vision.** Each player of the match (`Players`) has a `Vision` in its `PlayerState` (PLAN.md section 5 lists the visibility grid there): one byte per cell with a visible bit and an explored bit. The `Visibility` stage clears the visible bits and lets every unit and building of the player with `Sight` `s` reveal the cells within `dx² + dy² ≤ s²` (in cells, measured from the unit's cell, or from the nearest cell of the building's footprint), clipped to the map; revealed cells become explored and stay explored. A grid of another size (only match setup installs one) starts a new vision. The order of the sources does not matter, because revealing is a union (I4). Units and buildings of players who are not players of the match reveal nothing.
2. **Timing.** Commands and the `Combat` stage of a tick see the vision computed at the end of the previous tick. Before the first tick nothing is visible, so the first `Attack` can be accepted in tick 1 at the earliest (`m4_combat::attacks_on_own_dead_and_unseen_units_are_rejected_without_changing_state`).
3. **Visible entities.** Another player's unit is visible to a player when the cell it stands in or the cell it steps into is visible (`Vision::sees_unit`); a building, when some cell of its footprint is (`Vision::sees_footprint`). A player always sees its own units and buildings.
4. **Hiding in command validation.** `Attack` and `AttackMove` require the issuing player to be a player of the match (`UnknownPlayer`, checked first). `Attack` checks, after the M2 checks of the named units: every named unit has a weapon (`NotArmed`); the target is not the player's own (`OwnTarget`); the target exists and is visible to the player (`UnknownTarget`, the same reason for a dead or never existing id and for an unseen enemy, so that commands reveal nothing about unseen enemies). (Amended in M4 remediation round 1, D32 item 5: a further check that the target has hit points, `NotTargetable`, could not be reached and was removed, M4 review 1 P2-2.) Under fog of war, that is with a combat file loaded, every other command that names another player's unit or building as its own (`Move`, `Stop`, `Harvest`, `AttackMove`, `SellBuilding`, `SetRallyPoint`) reports an unseen one as `UnknownUnit` or `UnknownBuilding` instead of `NotOwned` (`m4_combat::commands_that_name_an_unseen_enemy_report_it_as_unknown`). Without a combat file nothing has sight, and these commands are validated exactly as in M2 and M3, so their tests are unchanged. Commands that name cells to move to or toward (`Move`, `Harvest`, `AttackMove`, `SetRallyPoint`) are not restricted to visible cells: ordering units into the shroud is how the map is explored, and their validation looks only at the map's bounds. `PlaceBuilding`, whose validation looks at what stands in the cells, requires under fog of war that the player sees every cell of the footprint (`Placement(NotVisible)`), checked before the terrain, buildings, and units in it (amended in M4 remediation round 1, D33: placement was not restricted, so its rejections named unseen enemy units and buildings with their ids, M4 review 1 P1-2; the previous rationale, that placement requires an own building within the placement distance, does not hold, because the placement distance is a Chebyshev distance between footprints and sight a Euclidean radius, so cells within the distance can lie beyond sight).
5. **Hiding in target acquisition.** Units and buildings acquire and keep only targets their owner sees (D29 item 5); a defensive structure with a short sight fires at an enemy within its range only while another unit of its owner sees it (`a_turret_fires_beyond_its_own_sight_only_at_enemies_its_owner_sees`).
6. **Hash.** Each player's vision (width, height, every cell's bits) is appended to the player's part of the hash.

### D31. Combat and fog in the game

1. **Data.** `game::units::load_match_data` loads `assets/data/combat.ron` (`COMBAT_PATH`) after the economy file; a missing or invalid file ends the game with exit code 1 and a `DataError` naming the file and field (probes in docs/STATUS.md). The production data gains a defensive structure, `gun_turret` (1 × 1, produced by the construction yard, prerequisite power plant, 10 power), with a placeholder sprite from `gen_building_sprites` checked like the other building sprites (D27 item 1). The production combat values are balance data (I10) and are not used by any `sim` test.
2. **Fog on screen.** `game::fog::tint_tiles` sets each map tile's `TileColor` from the human player's vision: white (unchanged) for visible cells, dark grey for fog, black for shroud; it writes only changed colours. Before the first tick the whole map is shroud.
3. **Hiding in rendering.** Views of other players' units and buildings are spawned hidden and shown only while the human player sees them (`units::unit_shown`, `buildings::building_shown`, the same rules as D30 item 3). Projectiles are drawn as small dots, only in cells the human player sees. An instant shot is drawn for six frames as a line from the attacker to the target when the human player sees the cells of both, as a small circle at the attacker or the target when it sees only that one, and not at all otherwise (`fog::shot_view`; amended in M4 remediation round 1, D33 item 4: the line was drawn when either end was seen, so it showed where an unseen attacker stood, M4 review 1 P2-1). Destroyed units' and buildings' views are removed.
4. **Interim controls until the hotkeys and sidebar of M5.** A right click on a visible enemy unit (its body under the cursor, as `selection::unit_at`) or on a visible enemy building (covering the cell under the cursor) orders the selected armed units to `Attack` it and the other selected units to move to the cell; `A` orders the selection to `AttackMove` to the cell under the cursor (D10 kept WASD free for M5's hotkeys; M5 may reassign `A`). The decisions are pure functions with unit tests, and the system pushes commands only (I5). (Kept in M5, D35 item 4.)
5. **Starting setup until M6.** The opponent starts with a gun turret at (50, 41), left of its construction yard, so that a defensive structure can be seen to fire (D27 item 3: match setup in game code until M6). With the yard's 20 power produced and the turret's 10 consumed, it has power.
6. **Smoke test** (D6 amended): see D6.

## M4 remediation round 1 (2026-10-04)

Decisions taken in remediation of M4 review 1 (`docs/reviews/M4-review-1.md`, NO-GO: 0 P0, 2 P1, 3 P2). Evidence: the regression tests named in `docs/STATUS.md` ("M4 remediation round 1 report"), each run against the code before the fix (with only the new enum variants added) and after it.

### D32. No engagement with a target the weapon cannot damage outlasts one ordered shot (M4 review 1 P1-1, P2-2)

1. **Class of defect: rules that keep a unit or building engaged with a target it cannot affect, so that the engagement, and any order that waits for it to end, never ends.** Equation 2 gives 0 when the warhead's percentage against the target's armour is 0, which the combat file admits (D28 item 3). Every rule that chooses, keeps, or fires at a target, or that waits for a fight, was checked:
   1. Automatic acquisition (`combat::Field::nearest_enemy_in_range`), used by idle units, attack-moving units, and defensive structures: it chose the nearest visible enemy in range by distance alone. **Defective** (P1-1): an idle rifleman fired at a fence beside it (0 damage) and not at the enemy rifleman that killed it; a turret would do the same.
   2. Keeping the current target (`keep_or_acquire`): it kept any visible enemy in range. **Defective**, the same way.
   3. `AttackMove` resumes its move only when the unit has no target (D29 item 7), so with items 1 and 2 an attack move that passed an enemy it could not damage never ended (P1-1, the reviewer's probe: 20,000 ticks, 1,997 shots). **Fixed by items 1 and 2.**
   4. An explicit `Attack` on a target in range fired for as long as the target stood (M4 implementation debt item 4). **Defective** in the same class: such an order never ends by itself. **Fixed** (item 3).
   5. Checked and not affected: the chase of an `Attack` order (at most `MAX_CHASES` moves without a shot, D29 item 6), projectiles (at most 32 ticks of flight, D28 item 5), defensive structures under low power (no target at all), move orders (D21, D22), and the harvester cycle (D26). A fight against a target the weapon damages ends, because every shot deals at least 1 (equation 2) and hit points are finite.
2. **Rule.** A weapon *damages* a target when `applied_damage(damage, versus) > 0`, that is, when its warhead's percentage against the target's armour class is above 0 (`combat::Field::damages`). Acquisition and keeping consider only visible enemies in range that the weapon damages (D29 item 5, amended). So an idle unit returns fire at the enemy it can damage, a defensive structure ignores what it cannot damage, and an attack move does not stop for such an enemy, so it ends when its move ends (D29 item 7, amended).
3. **Explicit orders.** An `Attack` order whose unit's warhead deals 0 to the target is carried out once: the unit closes in as for any target, fires one shot (`WeaponFired`, 0 damage), and the order ends in the same tick with the new `SimEvent::AttackEnded { reason: AttackEnd::NoDamage }`; the unit is then idle and, by item 2, does not take the target again. Alternatives rejected:
   * **Keeping the documented semantics** (the review's suggestion): the order then never ends by itself, which is the class of item 1.
   * **Rejecting the command at validation, or ending the order before the first shot:** no shot with a percentage of 0 could then be fired through `SimPlugin`, so the zero case of equation 2, a required M4 test (PLAN.md section 7), could be checked only by a unit test; a validation rejection would also reject a whole mixed selection (as `NotArmed` does) when one unit's weapon cannot damage the target.
   * **Preferring targets the weapon damages but still engaging others:** an attack move past only a fence would still never end.
4. **Tests changed** (stimulus only; no assertion removed or relaxed). Three M4 tests relied on an idle rifleman automatically engaging a concrete building, the behaviour item 1 removes: `the_damage_equation_holds_for_every_warhead_and_armour_pair_of_the_fixtures` now orders the attack for the one pair with a percentage of 0 (it still fires in tick 1 and still asserts 0 damage and the four cases); `a_building_is_in_range_when_its_nearest_edge_is` and `the_shortest_range_reaches_every_neighbouring_cell_and_an_adjacent_building` run on fixture variants in which the bullet deals 1 % against concrete (`BULLET_CONCRETE`), so that idle riflemen still engage the building whose distance they check. The fixture files themselves are unchanged.
5. **`RejectReason::NotTargetable` removed** (M4 review 1 P2-2). It was unreachable: without a combat file no unit has a weapon, so `NotArmed` comes first, and with one every unit and building type has combat terms (D28 item 4). D30 item 4 is corrected. A target without hit points, should setup ever create one, is accepted and the order ends in the next tick with `TargetGone`, because the `Combat` stage considers only entities with hit points.
6. **Hash and determinism.** No hashed state changed its layout. The 20 against 20 battle (units only, all percentages above 0) gives the same result as before in debug and release: tick 665, 6 human units left, 451 shots, 34 destroyed, final hash `05af3b1ac550cf16`.

### D33. Commands and the screen reveal nothing about what the player does not see (M4 review 1 P1-2, P2-1)

1. **Class of defect: command validation whose decision or reason depends on cells or entities the issuing player does not see.** PLAN.md section 5 requires every command to be validated for visibility, and M4's scope requires hiding unseen enemies in command validation. Every command variant was audited for what its checks read:

   | Command | What validation reads | Verdict |
   |---------|----------------------|---------|
   | `Move`, `Stop`, `Harvest`, `AttackMove` | The named units (an unseen foreign one is `UnknownUnit`, D30 item 4); the target cell's bounds | Hidden |
   | `Attack` | The named units; the target (`OwnTarget` only for the player's own, `UnknownTarget` for a missing or unseen one) | Hidden |
   | `QueueProduction`, `CancelProduction` | The player's own buildings and queues | Nothing foreign |
   | `SellBuilding`, `SetRallyPoint` | The named building (an unseen foreign one is `UnknownBuilding`); the rally cell's bounds | Hidden |
   | `PlaceBuilding` | The player's queue; the footprint's bounds; the terrain, buildings, and units in the footprint; the player's own buildings (distance) | **Defective** (P1-2): `Overlap` and `Occupied` named the building or unit and its cell, also in unexplored shroud |
   | all | `UnknownPlayer`; `IdsExhausted` (the global id counter, 2^32 ids, unreachable in practice) | Nothing about a foreign entity |

2. **Rule.** Under fog of war (a combat file loaded, D30 item 4), `PlaceBuilding` requires that the player sees every cell of a footprint that lies inside the map; otherwise it is rejected with the new `PlacementError::NotVisible(cell)`, naming the first unseen cell in row-major order. The check comes after `OutsideMap` (bounds reveal nothing) and before the terrain, building, and unit checks, so a rejection names only what the player sees, and it is the same whether or not something stands in an unseen cell. Every unit or building in a visible cell is visible to the player (D30 item 3), so `Overlap` and `Occupied` now name only visible entities. Without a combat file placement is validated as in M3, so the M3 tests are unchanged.
3. **Boundary of the claim.** Validation is hidden; the outcome of an accepted order is computed on the true state, as in M2 and M3: group destinations skip cells held by idle units and cells covered by buildings, seen or not; paths avoid buildings; a harvester avoids ore cells other harvesters have taken; and the movement events (`MoveFailed`, `MoveCompleted { retargeted }`) report the result. These are the physical consequences of the order; making them depend on the player's knowledge would need pathfinding on each player's view, which PLAN.md does not specify (recorded as debt). The `SimEvent` buffer is one stream for all players, as since M2; only `CommandRejected` is addressed to a player, and a consumer acting for one player (the interface, M6's AI) must use only events about what that player sees.
4. **On screen (M4 review 1 P2-1).** The game's use of events follows item 3: an instant shot is drawn as a line only when the human player sees both ends, and otherwise as a mark at the end it sees (`fog::shot_view`, D31 item 3 amended), so a shot no longer shows where an unseen attacker stands. The game's terminal log still writes every event of both players; it is a diagnostic, not the interface (recorded as debt).
5. **Consequence for play.** A building can be placed only on cells the player sees. With the production data (placement distance 4, construction yard sight 5, power plant sight 4) a footprint within the placement distance can include cells beyond sight, which are now rejected with `NotVisible` until a unit or building of the player sees them (user verification item in `docs/STATUS.md`).

## M5 (2026-10-04): interface and audio

Evidence sources as for M4: the sources of the pinned crates in `~/.cargo/registry/src/` (Bevy 0.19.1 with `bevy_ecs`, `bevy_app`, `bevy_ui`, `bevy_text`, `bevy_image`, `bevy_asset`, `bevy_audio`, `bevy_input`; `rodio` 0.22.2), `Cargo.lock` and the manifests (unchanged by M5: no dependency was added, I1 and I9), `cargo tree -p game -e features`, and runs on the development machine. Every API named below is exercised by code that compiles and runs in M5.

### A1 follow-up: APIs used by M5

| Concept | API | Evidence |
|---------|-----|----------|
| A world of its own, read through `&World` | `World` is `Send + Sync` (`bevy_ecs-0.19.1/src/world/mod.rs:3955-3957`) and `Default` (`:117`); `App::empty` (`bevy_app-0.19.1/src/app.rs:146`); `World::try_query`, `try_query_filtered` take `&self` (`world/mod.rs:1878, 1905`); `QueryState::iter(&mut self, &World)` yields `D::ReadOnly` items (`query/state.rs:1170`); `World::get`, `get_resource` | `game::sim_host`; `sim_host::tests::the_game_world_holds_no_simulation_state` (a `&mut Health` query on `&World` yields read-only items) |
| Copying a tick's events | `Messages::iter_current_update_messages` (`message/messages.rs:265`), `MessageWriter::write_batch` (`message/message_writer.rs:85`), `App::add_message` (`bevy_app-0.19.1/src/app.rs:427`) | `sim_host::run_tick`; `the_tick_system_does_not_tick_while_paused_and_copies_the_events` |
| UI | `Node` (`bevy_ui-0.19.1/src/ui_node.rs:492`), `Button` (`widget/button.rs:9`), `Interaction` (`focus.rs:51`, `Pressed` on a left press only, `focus.rs:148-175`), `RelativeCursorPosition` (`focus.rs:85`; `normalized` is (−0.5, −0.5) at the top left, `focus.rs:263-271`), `ImageNode::new` (`widget/image.rs:74`), `BorderColor::all` (`ui_node.rs:2280`), `ComputedNode::{size, inverse_scale_factor}` (`ui_node.rs:88, 216`), `UiGlobalTransform` (derefs to `Affine2`, `ui_transform.rs:199-206`), `EntityCommands::with_children` (`bevy_ecs-0.19.1/src/hierarchy.rs:383`) | `game::sidebar`, `game::minimap`; the smoke test checks that every displayed sidebar button lies inside the window |
| Text and its font | `Text` (`bevy_ui-0.19.1/src/widget/text.rs:111`), `TextFont::from_font_size`, `TextColor` (`bevy_text-0.19.1/src/text.rs`); `FontSource::default()` is `Handle::default()` (`text.rs:342-346`), under which `TextPlugin` inserts the embedded default font (`lib.rs:81-82, 141-147`, `FiraMono-subset.ttf`, feature `default_font`, part of `default_platform` since M0). `parley` is built with its `std` feature only (`cargo tree -p game -e features -i parley`); `bevy_text`'s `system_font_discovery` (`parley/system`) is off, so no system font is ever read. | `game::sidebar` uses `TextFont::from_font_size` only, never a font family or file |
| Minimap image | `Image::new_fill` (`bevy_image-0.19.1/src/image.rs:1185`), `Image::data: Option<Vec<u8>>` (`:618`), `Assets::get_mut` (returns a guard that marks the asset changed, `bevy_asset-0.19.1/src/assets.rs:440`), `RenderAssetUsages::default()` keeps the pixels in the main world | `game::minimap::draw_minimap` |
| Audio from samples | `Decodable` (`bevy_audio-0.19.1/src/audio_source.rs:83-91`: `type Decoder: rodio::Source + Send + Iterator<Item = rodio::Sample>`), `AddAudioSource::add_audio_source` (`lib.rs:108`), `AudioPlayer<T>`, `PlaybackSettings::{LOOP, DESPAWN, with_volume}` (`audio.rs:100, 106, 130`), `AudioSinkPlayback::set_volume` (`sinks.rs:26`), `Volume::Linear` (`volume.rs:36`); `rodio` 0.22.2 `Source` (`src/source/mod.rs:166-201`), `SampleRate = NonZero<u32>`, `ChannelCount = NonZero<u16>`, `Sample = f32` without its `64bit` feature (`src/common.rs:5, 8, 31-43`) | `game::audio::{Pcm, PcmDecoder}`; `the_decoder_hands_out_the_samples_unchanged` |
| No audio device | `AudioOutput::default` logs "No audio device found." and keeps no stream; `audio_output_available` then keeps the playback systems from running (`audio_output.rs:19-31, 336-338`), so effects with `PlaybackMode::Despawn` are never removed by Bevy | `game::audio::expire_effects` removes them itself |
| Bevy's own audio decoders | `impl Decodable for AudioSource` calls `rodio::Decoder::builder()...build().unwrap()` (`audio_source.rs:93-102`); the feature collection `audio` enables `vorbis` (`bevy-0.19.1/Cargo.toml`, `audio = ["bevy_audio", "vorbis"]`), which registers `AudioSource` and its `.ogg` loader (`lib.rs:96-100`) | Not used: see D38 |
| Keys | `ButtonInput::get_just_pressed` (`bevy_input-0.19.1/src/button_input.rs:265`), `any_pressed` | `game::hotkeys::pressed_hotkeys` |

### D34. The simulation host: the interface cannot obtain mutable access to simulation state

PLAN.md section 7 (M5) requires that the interface cannot obtain mutable access to simulation state, enforced by the type or module boundary. Until M4 the simulation's resources and entities lived in the game's own `World`, so any game system could have declared `ResMut<Players>` (M2 implementation debt item 5, M3 debt item 7: invariant I5 was a discipline).

1. **A world of its own.** `game::sim_host::SimHost` holds the simulation's `World` in a private field. It is built once by `SimHost::new` (private) from `SimPlugin` added to `App::empty()`, after which the world is taken out of that app; every simulation component is registered at once, so that read-only queries can be built from a `&World` before the first entity exists. The game's world, in which every interface, rendering, and audio system runs, contains no simulation resource or component (`sim_host::tests::the_game_world_holds_no_simulation_state`).
2. **Read access only.** The public API of `SimHost` is `world() -> &World` and read accessors (`tick`, `players`, `grid`, `definitions`, `projectiles`). For a `&World`, Bevy gives only shared resources and read-only query items (A1 follow-up above). `SimHost` has no public constructor and no public `&mut self` method, so `ResMut<SimHost>` grants nothing more, and no other module can build a `SimHost` to swap in. The game crate is `#![forbid(unsafe_code)]`, which closes `World::as_unsafe_world_cell_readonly`. The boundary is the type (`&World`) and the module (`sim_host.rs`): the only code that mutates the simulation is `SimHost::install` (match setup before the first tick) and `SimHost::step` (one tick), both private and called only by the module's two systems.
3. **Commands.** The interface pushes `sim::Command`s into the game resource `CommandSink`; `run_tick` moves them, in push order, into the simulation's `CommandQueue` as `IssuedCommand { player: HUMAN, .. }` before it runs the tick. The sink has no way to name another player. It holds at most `MAX_PENDING_COMMANDS` = 256 commands (it is emptied every tick); beyond that, and while the game is paused (D39), `push` returns a typed `CommandRefused` and `issue` logs each refused command, so no command is lost silently and nothing accumulates.
4. **Match setup** (`sim_host::start_match`, an exclusive `Startup` system after `map::load_map` and `units::load_match_assets`): the map loader leaves the grid in `MatchGrid` and the data loader the definitions in `MatchDefinitions` (both game resources, consumed here); the host installs them and runs the M3 and M4 setup (`buildings::setup_economy`, `units::plan_starting_units`, `sim::spawn_unit`) on its own world. A setup error is logged and ends the game with exit code 1, as before (`match_setup_installs_the_loaded_match_or_ends_the_game_with_an_error`).
5. **Events.** After each tick the host copies the tick's `SimEvent`s into the game world's `Messages<SimEvent>`, which Bevy advances once per frame, so every reader that runs every frame sees every event once, also when several ticks run in one frame. This resolves M2 implementation debt item 2 (the two-tick `MessageReader` window), which also applied to the M3 production rejection check and the M4 shot drawing.
6. **What changed in M1 to M4 code.** Only how state is read and commands are pushed: the views, fog, selection, construction, and smoke test read through `SimHost`, and push through `CommandSink`. `run_sim_tick` in `main.rs` is replaced by `sim_host::run_tick`. The simulation crate is unchanged except for `sim::check_placement` (D35 item 5).

### D35. Sidebar, interface modes, hotkeys, control groups, and the placement preview

1. **Sidebar** (`game::sidebar`, Bevy UI, 240 logical pixels on the right of the window): the minimap (D36); the human player's credits and power (`used / produced`, `LOW` and red under low power); two build tabs (buildings, units); one button per entry of the production menu (`construction::production_menu`, the order of the data files) with its name, cost, a badge (`READY`, progress in percent, `+n` further entries queued, or `locked`), and a progress bar of the head of its queue; Sell, Rally, and Pause buttons; master, music, and effects volume (D38); and a line that explains the current mode. It shows only the human player's own state.
2. **Clicks on items** (`sidebar::button_effect`, pure): a left click queues the item (`QueueProduction`) if the player owns its prerequisites and a producer (`item_status`, the rule of `QueueProduction`'s validation; a full queue is still rejected by the simulation and logged); on a building that is ready, it enters placement mode instead; a right click cancels the most recently queued entry (`CancelProduction`) if one is queued. A locked item does nothing.
3. **Modes** (`construction::InterfaceMode`): in *placement* mode a left click on the map places the ready building centred on the cursor (`PlaceBuilding`) where the preview is valid and leaves the mode; a click on an invalid footprint issues nothing and stays in the mode. In *sell* mode a left click sells the player's building under the cursor (`SellBuilding`) and the mode stays. In *rally* mode a left click sets the rally point of every unit-producing building of the player (`SetRallyPoint`) and leaves the mode. A right click or `Escape` leaves any mode; placement mode also ends when its building is no longer the ready one (placed or cancelled). In a mode, left and right clicks on the map belong to it: they neither select nor order units.
4. **Hotkeys** (`game::hotkeys::hotkey`, one table, so that no key has two meanings): `S` stop, `A` attack-move to the cell under the cursor (kept from D31 item 4), `B` placement mode for the ready building, `X` sell mode, `R` rally point at the cursor, `Delete`/`Backspace` sell the building under the cursor (kept from D27 item 4), `Escape` leave the mode, `P`/`Pause` pause, `Tab` next build tab, `-`/`=` master volume, `M` mute; `0`-`9` select control group n, Shift adds it to the selection, Ctrl or Cmd assigns the selection to it. Ctrl or Cmd with a letter is left to the operating system. The arrow keys stay the camera's (D10). The interim production keys `1`-`9` of D27 item 4 are replaced by the sidebar, and the digit keys now belong to control groups.
5. **Placement preview.** `sim::check_placement(grid, definitions, state, owned, building, origin)` is the sequence of checks of `PlaceBuilding` (D24 item 3, D25 item 7, D33): readiness, bounds, visibility under fog of war, terrain, buildings, units, and the placement distance. The command's validation now calls it, so the preview the game computes from the same state predicts the command's outcome exactly (`crates/sim/tests/m5_placement_preview.rs`: 262 origins, every kind of placement rejection, the state hash unchanged by the preview). In placement mode the footprint at the cursor is outlined and the building's sprite is drawn over it, green where `check_placement` accepts it and red where it rejects it, with the cell the rejection names crossed out. Because `NotVisible` is checked before anything in the footprint is looked at, the preview reveals nothing a rejected command would not (D33).
6. **Control groups** (`hotkeys::ControlGroups`, pure): ten groups of the player's units; units that no longer exist leave every group (and the selection) in the frame their view is removed.
7. **The map under the sidebar.** The hovered cell is `None` while the cursor is over the sidebar, and a left press there does not start a selection, so clicks on the sidebar never act on the map. The camera's clamp keeps the visible part of the view, the window less the sidebar, inside the map's bounding rectangle (`camera::clamp_centre_beside`), so every part of the map can be brought into view beside the sidebar.
8. **Ordering.** All input systems run in `PostUpdate` in one order: the hovered cell, the sidebar's input, the selection chain (with control groups), then the construction chain. `sidebar::tests::the_game_systems_share_no_state_without_an_order` builds every game plugin and asserts that Bevy finds no two systems with conflicting access and no order in `Startup`, `FixedUpdate`, `Update`, or `PostUpdate` (it found and led to ordering the title against the camera's window reads, a pair present since M3 but outside the earlier tests, and the M5 audio and health-bar systems against the view registries and the selection).

### D36. The minimap and what the interface knows of the terrain

1. **Layout.** The minimap is an image of the whole map in the main view's projection (equation 7), scaled so that the map's bounding rectangle fills it: continuous cell coordinates (u, v) of a W × H map lie at the normalised point `((u − v + H) / (W + H), (u + v) / (W + H))`. The image is `(W + H)·k` by half that many pixels with `k = max(1, 256 / (W + H))` (256 × 128 for the demo map; at most 512 × 256 for the largest map), shown at 224 × 112 logical pixels. The camera's visible view (D35 item 7), an axis-aligned world rectangle, is an axis-aligned rectangle on the minimap, drawn as a white frame. A left click or drag on the minimap centres the visible view on that point.
2. **Content.** Each pixel shows its cell: the terrain colour (rock and water, ore, grass, sand) where the human player sees the cell, dimmed where it was seen before, black where it was never seen; the human player's units and buildings in blue; other players' units and buildings in red, only while `units::unit_shown` and `buildings::building_shown` show them (the rules of the main view, D30 item 3, D31 item 3). The image is redrawn once per simulation tick.
3. **The terrain the interface uses is the map file's** (`map::MapTerrain`, kept by the map loader): passable, buildable, and ore as at the start, which is what the tiles draw. The simulation's current ore (which harvesters of either player deplete) is never shown: on the minimap, in the choice between `Harvest` and `Move` for a right click with harvesters selected (until M4 it read the simulation's current ore, so a right click on a fogged, depleted ore cell could reveal its depletion), or in the picking log of a left click (M1).

### D37. Health bars

A bar above a unit's sprite or a building's sprite shows its hit points as a fraction of its type's (`CombatTerms::hit_points`, D28), green above one half, yellow above one quarter, red below. It is drawn for every damaged unit or building whose view is shown and for every selected unit, and never over a hidden view, so the hit points of an enemy the human player does not see are never drawn (`health::bar_drawn`).

### D38. Sound effects, music, and volume settings

1. **Synthesised, not decoded.** Every sound is synthesised by `game::audio` when the game starts (sine, square, and triangle notes with envelopes and pitch glides, and noise from a fixed-seed xorshift generator), mono at 22,050 Hz, normalised to a peak of 0.8 (0.7 for the music): nine effects (acknowledgement, error, production ready, building placed, building sold, credits, instant shot, cannon, explosion) of at most 1.5 s and a 19.2 s music loop (eight bars at 100 beats per minute: bass, pad, drums, and an arpeggio in the second half, over A minor, F, C, and G). No audio file exists in the repository or is read, so invariant I11 has no audio input to restrict and `assets/audio/` stays empty.
2. **Played without a decoder.** The samples reach Bevy as `audio::Pcm`, an asset implementing `bevy_audio::Decodable` whose decoder hands out the samples unchanged (mono, 22,050 Hz, one span of the whole sound). Bevy's `AudioSource`, whose `decoder()` unwraps a fallible decoder build, is never used. Its `.ogg` loader is registered by the `audio` feature collection but cannot run: the asset reader serves only registered checked bytes (D16 item 3), and no audio file is registered.
3. **What plays** (`audio::event_sound`, pure): the human player's own `ProductionComplete`, `BuildingPlaced`, `BuildingSold`, `OreDelivered`, and `CommandRejected`; `WeaponFired` (instant or projectile sound) only when the human player sees the attacker or the target; `UnitDestroyed` and `BuildingDestroyed` for the human player's own, or for an enemy whose view was shown in the last frame. Events of the opponent's economy never play. An accepted interface command plays an acknowledgement and a refused one (D34 item 3) an error, from the sink's counters.
4. **Bounds.** At most `MAX_ACTIVE_EFFECTS` = 8 effects at once, the same effect at most once per 80 ms, and every effect entity is removed one second after its sound's end by the game itself, also when the machine has no audio device and Bevy never starts it (A1 follow-up above), so effect entities cannot accumulate.
5. **Volume.** Master, music, and effects levels from 0 to 10 and a mute switch (`VolumeSettings`; defaults 8, 5, 7). The music's gain is master × music, applied to the playing music when a setting changes; an effect's gain is master × effects, fixed when it starts. The settings last for the session only; storing options is M7's ("options"), and storing them now would add a settings file that the game reads back, an external input.
6. **Font.** The sidebar's text uses Bevy's embedded default font, FiraMono (subset), which `bevy_text` 0.19.1 compiles into the binary under the `default_font` feature (enabled since M0 by `default_platform`). It is third-party material under the SIL Open Font License 1.1 and is now recorded in `assets/LICENSES.md`. It is not a file of the repository and is not read from disk; no system font is discovered (A1 follow-up above).

### D39. Pause

`P`, `Pause`, or the sidebar's Pause button pauses the game: `sim_host::run_tick` runs no tick while `CommandSink::paused`, so no simulation state changes, and the sink refuses every command until the game resumes (logged, with the error sound), so nothing accumulates while paused. The camera, selection, control groups, sidebar, and audio keep working; a "PAUSED" banner covers the map. The fixed timestep keeps running and simply skips the ticks, so resuming does not run a backlog of ticks. The smoke test pauses for 30 frames in the middle of every run of at least 300 frames and fails if a tick runs or a command is accepted meanwhile.

## M6 (2026-10-04): skirmish AI and game flow

Evidence sources as for M5: the sources of the pinned crates in `~/.cargo/registry/src/` (Bevy 0.19.1 with `bevy_ecs`, `bevy_ui`, `bevy_window`; `ron` 0.12.2), `Cargo.lock` and the manifests (unchanged by M6: no dependency was added, I1 and I9; `cargo tree -p sim -e normal` still contains no rendering, windowing, audio, or input crate), and runs on the development machine (probe programs outside the repository that played computer players against each other, in debug and release builds). Every API named below is exercised by code that compiles and runs in M6.

### A1 follow-up: APIs used by M6

| Concept | API | Evidence |
|---------|-----|----------|
| A menu above the sidebar that takes the clicks | `GlobalZIndex(i32)` (`bevy_ui-0.19.1/src/ui_node.rs:2450`); `FocusPolicy::Block` (`focus.rs:108`; the default is `Pass`, `focus.rs:116-123`), so the sidebar's buttons under the menu see no interaction | `game::skirmish::spawn_screens`; the smoke runs find all 10 menu buttons and start the match through the menu's effect |
| Spawning children | `children!` (`bevy_ecs-0.19.1/src/hierarchy.rs:519`), `EntityCommands::with_children` | `game::skirmish::spawn_screens` |
| A system that starts or ends a match with full world access | `Commands::queue(impl Command)` (`bevy_ecs-0.19.1/src/system/commands/mod.rs:641`) with a closure `FnOnce(&mut World)`, applied at the end of the system's schedule | `game::skirmish::screen_input`, `game::smoke::play_flow` |
| Removing a match's entities | `World::query_filtered` (`world/mod.rs:1827`); `World::despawn`, which also despawns children (`world/entity_access/world_mut.rs:1795-1805`), so the map entity takes the layers and tiles `bevy_ecs_tiled` spawned under it | `game::map::despawn_map`, `game::skirmish::end_match`; the smoke runs end with as many views as simulated units and buildings after two matches were replaced |
| Bevy's exit when no window is left | `exit_on_all_closed` writes `AppExit::Success` in `Last`, set `ExitSystems` (`bevy_window-0.19.1/src/system.rs:18-26`, `lib.rs:144`) | `game::smoke::fail_on_early_exit` is ordered after `ExitSystems` (D44 item 7) |
| Reading a tick's commands in tests | `sim::CommandQueue::iter` (new, read-only) | `crates/sim/tests/m6_ai.rs` |

### D40. Skirmish definitions and match setup (invariants I5, I8, I10, I11)

1. **A file of its own.** The starting credits, the factions with their starting buildings and units, and the maps of the menu with their start positions are defined in `assets/data/skirmish.ron` (fixture `crates/sim/tests/fixtures/data/skirmish.ron`), schema version 1, loaded by `sim::load_skirmish(path, &definitions)` through the RON subset checker of D18 (schema `skirmish_defs::SKIRMISH_FILE_SHAPE`, six levels) before `ron` parses the checked text into types with `deny_unknown_fields`. Every field is required. This moves the starting credits (D27 item 3), the starting buildings (D27 item 3, D31 item 5), and the starting units (D20 item 4) out of game code into data, as invariant I10 requires (M3 review 1 P2-4), and `game::buildings::{STARTING_CREDITS, STARTING_BUILDINGS, PLAYERS, setup_economy}` and `game::units::{STARTING_UNITS, starting_unit_total}` are removed.
2. **Fields and ranges** (checked on the parsed values at both ends, with the field path): `starting_credits` 0 to `MAX_STARTING_CREDITS` = 10^6; `factions` 1 to `MAX_FACTIONS` = 8, each with an `id` (the rules of D18), 1 to `MAX_STARTING_BUILDINGS` = 16 `buildings` (a building type and an offset `x`, `y` from the start position, each −32 to 32 cells; at least one building, since a player without buildings is defeated, D41) and 0 to `MAX_STARTING_GROUPS` = 16 `units` (a unit type, an offset, and a `count` of 1 to `MAX_GROUP_UNITS` = 64); `maps` 1 to `MAX_MAPS` = 16, each with an `id` and exactly `MATCH_PLAYERS` = 2 `starts` (cells 0 to 255, distinct; the file is read without its maps, so each start is checked against its map's grid by every loader of the map and by match setup: a cell of the map with passable terrain, D45, amended in M6 remediation round 1). Building and unit names must be defined (`UnknownReference`); faction and map ids are unique (`DuplicateId`).
3. **Maps.** A map id `m` names the file `maps/m/m.tmx` under the asset directory (`sim::map_asset_path`). Ids are names of `a`-`z`, `0`-`9`, and `_`, so they cannot leave the asset directory (a probe with `../demo` is rejected at `maps[0].id`). The game checks every listed map when it starts (D44).
4. **Match setup** (`sim::setup_match(world, &skirmish, &ai, map, &players)`): the world must have the map's grid and the match definitions installed (`NotInstalled` otherwise) and no players yet. The configuration is checked first, and on an error nothing is changed (`m6_match::setup_errors_are_typed_and_change_nothing_when_the_configuration_is_wrong`): exactly two players, no player or start position twice, factions, start positions, and difficulties that exist; since M6 remediation round 1 also every start position of the map a cell of the map with passable terrain (`InvalidStart`, D45) and every computer player's faction with the producers its AI file needs (`UnobtainableProducer`, D46). Then, in player order, every player is added with the starting credits, its faction's buildings are placed with their footprint origin at the start position plus their offset (`spawn_building`: the footprint rules apply, the distance rule does not), and its faction's units on the free cells nearest to the start position plus their offsets (`sim::plan_starting_units`, moved from `game::units` with `StartError`, unchanged in behaviour). A building or group that does not fit is a typed `MatchSetupError` naming the player, the building or unit type, and the cell; after such an error the world is partly set up and is discarded by the caller (the game keeps its previous simulation, D44). Finally every computer player gets its controller (D43), or the setup fails with `NoGatheringPoint` if it has no gathering point (D45), and the players are recorded in `MatchState` (D41).
5. **Setup, not gameplay.** Like installing the grid (D11), match setup changes state directly before the first tick; during a match the interface and the computer players change state only through `CommandQueue` (I5). M1 review 1 P2-5 asked that the user acknowledge this reading of I5 when M6 adds match setup; the acknowledgement is listed in `docs/STATUS.md` under "requires user verification".
6. **Shipped data.** One faction, `vanguard`: a construction yard at the start position, 5 light tanks around the start plus (4, 1), and 3 scouts around the start plus (1, 4); 5,000 starting credits; one map, `demo`, with the human player's yard at (15, 37), as before M6, and the computer player's at (52, 42). Both players start alike (until M5 the human player had 18 units and the opponent 8 and a gun turret; a skirmish needs equal starts). The second faction is M7's (PLAN.md section 7); with one faction the menu's faction rows offer one entry, and the mechanism is tested with the two fixture factions.

### D41. The win condition (PLAN.md section 7, M6: "win and lose detection when all buildings of a player are destroyed")

1. **Rule.** `sim::MatchState` holds the players of the match, who has been defeated (and in which tick), and the outcome. In the `WinCondition` stage of every tick (`check_win_condition`), a player of the match who owns no building is defeated (`SimEvent::PlayerDefeated`); when at most one player is left, the match ends (`SimEvent::MatchEnded { winner }`) with that player as the winner, or as a draw (`winner: None`) if the last players lost their last buildings in the same tick. Buildings destroyed in the `Cleanup` stage or sold in the `Commands` stage of the tick no longer count: Bevy applies their removal at the sync points before the stage. The outcome is final: later ticks change and report nothing (`a_player_without_buildings_is_defeated_and_the_last_player_wins`, `players_who_lose_their_last_buildings_in_the_same_tick_draw`, `a_destroyed_last_building_defeats_its_owner`).
2. **Without a match set up** (`MatchState` with no players, as `SimPlugin` inserts it) there is no win condition, so the M0 to M5 tests, whose players often own no building, are unchanged (`without_a_match_set_up_there_is_no_win_condition`).
3. **Units do not count.** A player with units but no building has lost, as the plan states; its remaining units stay on the map and still fire at what comes into range until the game stops ticking (D44).
4. **Hash (I3).** `state_hash` appends, after the projectiles, the match state (players, defeated players with their ticks, outcome) and the computer players (D43). Hash values recorded in earlier decisions (D22, D29) were computed with the earlier layout.

### D42. The AI file: build order, army, and difficulty parameters (invariants I8, I10, I11)

1. **A file of its own.** `assets/data/ai.ron` (fixture `crates/sim/tests/fixtures/data/ai.ron`), schema version 1, loaded by `sim::load_ai(path, &definitions)` through the RON subset checker (schema `ai_defs::AI_FILE_SHAPE`), against the match definitions, which must include a combat file. Every field is required.
2. **Fields.** `build_order`: 1 to `MAX_BUILD_ORDER` = 32 building types, in the order they are built; a type listed n times is kept at n buildings. `power`: the building type built for power; it must produce power. `harvester`: a harvester unit type with production terms. `army`: 1 to `MAX_ARMY_ENTRIES` = 16 armed unit types with production terms, produced in this order, cyclically. `difficulties`: 1 to `MAX_DIFFICULTIES` = 8, each with an `id` and the parameters below.
3. **Difficulty parameters** (the plan's "difficulty set by data parameters"), with their ranges: `think_interval` 1 to `MAX_THINK_INTERVAL` = 1,200 ticks between two decisions; `harvesters_per_refinery` 0 to 8; `max_army` 1 to `MAX_AI_ARMY` = 200 armed units; `wave_size` 1 to `max_army`; `first_wave` 0 to 72,000 ticks (no attack wave before); `wave_patience` 1 to 72,000 ticks (after `first_wave` or the last wave, the units at home attack even if fewer than `wave_size`); `defence_radius` 0 to `MAX_DEFENCE_RADIUS` = 32 cells. `MAX_AI_ARMY` keeps every command of a computer player far below `MAX_UNITS_PER_COMMAND` and bounds the memory its units use.
4. **Cross checks that keep a computer player from getting stuck in its data** (rules that must hold for all values, not only the shipped ones): every prerequisite of a building of the build order is listed earlier in it (otherwise the build order would wait for a building it builds only later); and for the armour class of every building type, some army unit's weapon deals damage (equation 2 above 0), so that the army can destroy every building and a match between computer players can end (the fixture's riflemen alone cannot damage concrete and are rejected as such, `MissingEntry { kind: "unit able to damage building", .. }`). **Amended in M6 remediation round 1 (D46):** as reviewed, this item claimed more than the code checked: the producers and prerequisites of the army and the harvester, and the producers of the buildings, were not checked, and a valid-looking AI file stalled every match (M6 review 1 P2-1). D46 adds the missing checks. The checks keep the *data* from making a computer player unable to build its buildings, harvesters, or army; they do not make every match end (D43 item 9; a player that loses a producer and has no credits to rebuild it can still stall, recorded as debt).
5. **Shipped values.** Build order: power plant, refinery, barracks, war factory, gun turret, power plant, refinery, gun turret; army: light tank, light tank, scout; difficulties `easy` (think every 40 ticks, 1 harvester per refinery, up to 8 armed units, waves of 6 from tick 6,000, patience 3,600, defence radius 8), `normal` (20; 2; 16; 8 from 3,600; 2,400; 10), and `hard` (10; 2; 30; 10 from 2,400; 1,800; 12). They are balance data (I10), set so that computer players finish their matches (D43 item 9); the balance pass is M7's.

### D43. The computer player (PLAN.md section 7, M6)

1. **Inside the simulation, through the command queue.** `sim::AiPlayers` holds the AI file and one `AiController` per computer player, created by match setup. `run_ai`, the only system of the `Ai` stage, lets each controller whose turn it is (`tick mod think_interval = player mod think_interval`, and not defeated, and no outcome yet) decide from a snapshot of the state and push `IssuedCommand`s into `CommandQueue`; the `Commands` stage of the next tick validates and applies them like the human player's (I5). The controller's own state (memory, waves, waypoints, counters) is part of `state_hash` (I3); iteration is in id order or over ordered collections (I4); its arithmetic is in `i64` or saturating (I12).
2. **What it may know (the class of information leaks through fog of war).** `think` reads only a `View` built for its player: its own units and buildings, credits, power, and queues; other players' units and buildings only while its player's vision shows them (`Vision::sees_unit`, `Vision::sees_footprint`); a memory of the other players' buildings it has seen, from which a building is forgotten when a cell of its footprint is seen and it is not (it is gone); the ore of a cell, and the units and buildings in a cell, only while the cell is visible (placement and refinery search check visibility before they read a cell; `check_placement` itself checks visibility first, D33). As public match setup it knows the static terrain of the map (passable and buildable, as the map file defines them; the human player knows the map from the menu) and the start positions of the match (computed into the gathering point and the waypoints when the match is set up). It never reads another player's credits, queues, or unseen units or buildings. The outcomes of its accepted orders use the true state as for every player (D33 item 3). `m6_ai::the_ai_ignores_what_its_player_does_not_see` runs two matches that differ only in an unarmed enemy unit within the computer player's defence radius but out of its sight, and an enemy building in the shroud: its commands are identical tick by tick while it sees neither (all 2,000 ticks of the test; it requires at least 500), and it does not remember the building.
3. **A turn**, in this order, each a bounded number of commands (at most 16 in the tests' bound): place a building that is ready (item 4); queue the next building of the build order that it owns fewer times than listed up to that entry, if its producer's queue is empty, or the power building first if the building would consume more power than is spare, or if the build order is complete and power is low (so lost buildings are rebuilt, `the_ai_rebuilds_a_building_it_lost`); queue harvesters up to `harvesters_per_refinery` per refinery (counting those queued and those waiting to leave a refinery) and army units of the composition up to `max_army`, at most `AI_UNIT_QUEUE` = 2 items per producing building type; set the rally point of each unit producer without one to the gathering point; send idle harvesters to work at its refinery nearest to its start, at most once every `HARVEST_RETRY_TICKS` = 200 ticks (an idle harvester found no ore or was blocked, and every order makes the simulation search the map; M3 review 1 P2-2's stopped harvesters now resume without the player); defend (item 6); attack (item 7).
4. **Placement** (`find_placement`): origins within the placement distance of its buildings, tried nearest to its start first (a refinery nearest to visible ore first); an origin is chosen only if the command will still be valid when it is validated in the next tick, unless the player loses a unit or building in between: the footprint is visible and stays visible (within the sight of one of its buildings, or of one of its units from both the cell it stands in and the cell it steps into, because in one tick a unit's own cell can only become the cell it steps into); no unit holds a footprint cell or a visible cell around it, and none of its own units holds an unseen cell around it (no unit moves more than one cell per tick: speed is below 256 subcells); no building covers a visible cell around it; it keeps one free cell to its own buildings and to the gathering point, and two to its buildings from which a unit may leave in between (a unit producer whose queue head completes in that tick, or a building with free units waiting), because such a unit may step on at once; some visible cell around it is passable; and `check_placement` accepts it. A ready building with no such place waits for the next turn.
5. **What a rejected command means.** With data that passes the checks of D45 and D46 (as reviewed, a start position outside its map made every gathering order and rally point of the computer player invalid, M6 review 1 P1-1), a computer player's command can be rejected only because of what happens in the tick between its decision and the command's validation: a unit or building it names is destroyed (`UnknownUnit`, `UnknownBuilding`), a building it needs is destroyed (`MissingPrerequisite`, `NoProducer`, `Placement(NotVisible | TooFar)`), or an enemy unit it could not see next to a footprint steps into it (`Placement(Occupied)` naming another player's unit). The end-to-end tests assert that every rejection is so explained. The **standard scenario** of the plan's test is a computer player against a player who gives no order (whose units and turret still fire), on the fixture map with the fixture data, until the match ends: no command of the computer player is rejected there, at either fixture difficulty (`the_ai_issues_no_rejected_command_in_the_standard_scenario`).
6. **Defence and gathering.** Armed units not in the attack wave are its army at home. When a visible enemy unit or building lies within `defence_radius` (Chebyshev distance) of one of its buildings, the home units without a combat order attack-move to the one nearest to its start (`the_ai_attacks_a_visible_enemy_near_its_base_with_its_army_at_home`); otherwise idle home units farther than `GATHER_RADIUS` = 4 cells from the gathering point attack-move there. The gathering point is computed at setup from the static terrain: of the cells reachable from the start over passable terrain and free of buildings, the one nearest to the point `GATHER_DISTANCE` = 6 cells from the start toward the map's centre (amended in M6 remediation round 1, D45: it was the passable cell nearest to that point by path cost from it, or the start itself if none was reachable from it, a fallback to a cell that can lie outside the map or under a building; a computer player without a gathering point is now a setup error). For the shipped and fixture data the gathering points are the same cells as before.
7. **Attack waves.** From tick `first_wave`, when no enemy threatens its base and `wave_size` armed units are at home, or any after `wave_patience` more ticks, the home units join the wave. The wave attack-moves to the nearest building it remembers (to the footprint cell nearest to the wave's centre: the centre of a 3 × 3 footprint has no passable neighbour, so `destination_cells` would find no cell there); else to an enemy start position it has never seen; else it sweeps the waypoints (one passable cell per block as wide as the shortest sight of the army, so that a unit near a block's waypoint sees the block), the one seen longest ago first. It retargets when its target is gone or all its units are at rest; a remembered building at which the wave came to rest `MAX_TARGET_FAILURES` = 3 times is skipped until the wave is reinforced, and a waypoint at which it came to rest counts as seen, so an unreachable target never holds a wave. A wave reduced below half the wave size falls back to the gathering point and rejoins the units at home, so that the next wave attacks at full size instead of feeding units into a defended passage one by one; a wave sent after the patience ran out with fewer units does not fall back.
8. **When it can neither fight nor pay.** A computer player with no armed unit, no working harvester (all stopped, or none), the producer and prerequisites of some army unit type, and fewer credits than the cheapest army unit in its queues still costs (or the cheapest it can produce) sells, one per turn, the building with the highest sell value that no army unit type needs (as producer or prerequisite), a building that produces buildings only when nothing else is left; until it has an armed unit again it queues no building, which would take the credits first. Without this rule two computer players without ore, credits, or armed units kept their buildings forever (seen with the shipped data, easy against normal: no outcome in 40,000 ticks). Its last building is never sold (it needs the army producer), and a player that cannot produce any army unit sells nothing (`a_broke_ai_without_army_sells_a_building_it_does_not_need_to_fund_units`, `an_ai_with_no_credits_and_no_harvester_neither_panics_nor_floods_the_queue`).
9. **Matches end.** Measured with the probes, every pairing of the shipped difficulties on the demo map ends with a winner within 23,022 ticks (19 minutes of game time; release 0.1–0.3 s per match), every pairing of the fixture difficulties within 5,697 ticks on the fixture map and within 20,443 ticks on the demo map, and every difficulty against a player who gives no order within 7,611 ticks. The tests bound a match at 36,000 ticks (30 minutes, the length of M7's soak test). Termination is not proven for all data: two computer players that both keep armed units they never risk, or both lose everything they could sell, can stall; the rules above remove the stalls found.
10. **Tests and data (I10).** The plan's tests run on fixture data (`crates/sim/tests/m6_ai.rs`): the build order on the fixture map within 1,000 ticks for both factions and difficulties; the standard scenario; no credits and no harvester (3,000 ticks with a turn every tick: one command per player in all, at most one item queued); and the end-to-end matches on the fixture map and on every shipped map (the maps and start positions of the production skirmish file with the fixture factions, units, buildings, and AI), each pairing run twice with equal hashes on every tick. In addition, `crates/game/tests/m6_production_matches.rs` plays every pairing of the shipped difficulties with the shipped data on every shipped map once: it checks a property of the shipped game, so a balance change that breaks it is meant to fail it (like the production-data tests of M1 to M5).
11. **Bounds.** A controller holds at most the waypoints of one map (one per block plus one per enemy start: at most 65,537 at sight 1; at most 11 × 11 + 1 on the demo map, whose shortest army sight is 6), the other players' buildings it has seen, its wave (at most `max_army`), and fixed counters; a placement search visits at most the origins within the placement distance of its buildings inside a 336 × 336 area. Commands per turn are bounded by item 3, and turns by the think interval.

### D44. Game flow in the game: catalogue, menu, end screen, restart

1. **Catalogue at startup** (`game::skirmish::load_catalogue`, replacing M1's `map::load_map` and M5's `units::load_match_assets` at startup): the unit, economy, and combat files with every sprite (as before), the skirmish and AI files, and every map of the skirmish file with its tileset image (`map::check_map`, the checks of D13 and D16) are loaded and checked once, and only their checked bytes are registered for the renderer (D16 item 3). Since M6 remediation round 1 the files are then checked against each other (`skirmish::check_catalogue`, D45, D46): every start position against its map, the AI file against every faction, and every match the menu can choose (every map with every pair of factions) is set up once in a simulation of its own that is then dropped. Any error is logged naming the file and field and ends the game with exit code 1, as before. Starting or restarting a match reads no file.
2. **Menu.** The game starts in a full-window menu above the sidebar (`GlobalZIndex`, `FocusPolicy::Block`): rows for the map, the human player's faction, the computer player's faction, and the difficulty, each with previous and next buttons that wrap around, and Start and Quit. The rows show ids with underscores as spaces. The first choices are the first map and factions and the middle difficulty (the AI file lists them from the easiest). The effect of each button is the pure function `menu_effect`, tested for every button, wrap-around, and a selection outside the catalogue.
3. **Starting a match** (`begin_match`): the current match ends (item 5); a new simulation is set up through `sim_host::install_match` with `sim::setup_match` (human player 0 at the map's first start, computer player 1 at the second); if setup fails the previous simulation is kept, the error is logged, and the game ends with exit code 1; otherwise the map is spawned from its checked bytes, the camera bounds and the minimap are sized for it, and the camera is centred on the human player's start. **Seed:** each match's seed is taken from the clock (the low 64 bits of the nanoseconds since 1970) and logged with the match; the simulation itself never reads the clock (I4). This resolves M0 debt item 3 (the constant `MATCH_SEED`). The simulation uses no randomness so far, so the seed changes only `state_hash`.
4. **The end of a match.** `SimHost::running` is true while a match is set up and has no outcome; `run_tick` runs ticks only then. When a tick reports the outcome, ticks stop and `CommandSink` closes: it refuses every command (`CommandRefused::MatchOver`, logged, with the error sound) and discards pending ones with a log line, so nothing accumulates and nothing is lost silently (amended in M6 remediation round 1, D47: ending a running match reset the sink, which cleared pending commands without a log line, before closing it; every discard now goes through one logging path). The end screen shows VICTORY, DEFEAT, or DRAW for the human player (`end_text`) over the last state of the map, with Restart (a new match with the same choices and a new seed) and Main menu (`end_effect`). Sound effects for `PlayerDefeated` and `MatchEnded` are not played; the end screen announces the outcome.
5. **Ending a match** (`end_match`, also the first step of every start): the simulation is replaced by one without a match; the map entity (with the layers and tiles under it), every unit and building view, and the match's resources are removed or reset (`MapView`, `MapTerrain`, `MapAsset`, `MapStatus`, `HoveredCell`, `UnitViews`, `BuildingViews`, `UnitStatus`, `BuildingStatus`, `Selection`, `DragBox`, `ControlGroups`, `InterfaceMode`, `RecentShots`, `CameraBounds`, the minimap's image and `MinimapState`), and `CommandSink` is reset (its accepted and refused counters keep counting, because the sound system compares them with the counts it answered). Kept across matches: the catalogue, the sprites, the sidebar and its item buttons (the production menu depends only on the definitions), the volume settings, the build tab, the placement ghost entity, and the music.
6. **Ordering.** The menu's and end screen's systems run in `Update` after the sidebar's and the minimap's systems that also write texts and nodes; `sidebar::tests::the_game_systems_share_no_state_without_an_order` now includes `SkirmishPlugin` and still finds no unordered conflicting pair. Starting and ending matches happen in queued world commands, not in systems that run beside others.
7. **Smoke test** (D6 amended): see D6. An exit before the last frame now fails the run: until M6, Bevy's `exit_on_all_closed` (item A1 above) let a run whose window closed early (seen here when a display was removed during a debug run) exit with code 0 before its checks ran.

### Notes on earlier decisions (M6)

* **D6 amended in M6 (2026-10-04).** The game starts in the menu. The smoke test requires the menu with all 10 buttons, starts the match with the Start button's effect (`menu_effect`), and, in runs of at least 300 frames, restarts it with the Restart button's effect after 10 ticks, returns to the menu with the Main menu button's effect after 10 more, and starts again; the checks of M1 to M5 then apply to the last match, with the starting units and buildings of both factions of the skirmish file. It also requires as many unit and building views as the simulation has units and buildings (nothing of an earlier match is left), and, once 60 ticks have run, commands of the computer player. The human player's rejected commands fail the run as before; the computer player's are logged, and since M6 remediation round 1 (D47) any of them fails the run as well. A run that exits before its last frame fails (D44 item 7). The final log line also reports the menu buttons, the matches started, the flow, the views, and the computer player's commands.
* **D20 item 4, D27 item 3, D31 item 5** (starting units, credits, buildings, and the opponent's gun turret in game code "until M6") are superseded by D40. The demo match's opponent no longer starts with a gun turret; the computer player builds gun turrets through its build order (D42 item 5), and the human player can build them.
* **D34 item 4** (match setup at startup) is superseded by D44: matches are set up from the menu, and the setup itself is `sim::setup_match` (D40).
* **D9.** `map::misalignment`'s documentation no longer says that `sim` does not check rendering properties (M1 review 3 P2-3, first item): the function was touched by the M6 split of the map loader.

## M6 remediation round 1 (2026-10-04)

Decisions taken in remediation of M6 review 1 (`docs/reviews/M6-review-1.md`, NO-GO: 0 P0, 1 P1, 4 P2). Evidence: the regression tests named in `docs/STATUS.md` ("M6 remediation round 1 report"), the probe tests run against the reviewed code before the fix (all five configurations were accepted), and the real binaries run against scratch asset roots (`BEVY_ASSET_ROOT`) in both profiles.

### D45. Start positions are cells of their map, and the computer player falls back to no unusable cell (M6 review 1 P1-1)

1. **Class of defect: cross-file references of the new data files that were not validated at load.** Every reference of the skirmish and AI files to another file was audited:

   | Reference | Checked before | Verdict |
   |-----------|----------------|---------|
   | Building and unit types of the factions, the build order, the power building, the harvester, the army | `load_skirmish`, `load_ai` (`UnknownReference`) | Checked |
   | Map ids to map files | The game's catalogue (`check_map` for every map, D44 item 1) | Checked |
   | Prerequisites of the build order | `load_ai` | Checked |
   | **Start positions to their map's size and terrain** | Only 0 to 255 | **Defective** (P1-1), fixed here |
   | **A faction's buildings and units at a start of a map** | Only when the match started (exit code 1, implementation debt item 7) | **Defective** (debt item 7), fixed in item 4 |
   | **Producers and prerequisites of the harvester and the army; producers of the build order and of the power building; prerequisites of the power building** | Not checked | **Defective** (P2-1), fixed by D46 |

2. **Rule.** A start position is a cell of its map whose terrain is passable (`sim::start_problem`; its own buildings may cover it). The skirmish file is read without its maps, so its range check stays 0 to 255, and the map check is separate: `sim::check_map_starts(path, skirmish, map, grid)` returns a `DataError` naming the file, `maps[i].starts[j]`, the cell, the map's size and id, and the reason; and `setup_match` checks every start position of the map before anything is changed (`MatchSetupError::InvalidStart { start, cell, problem }`, with `StartProblem::{OutsideMap { width, height }, Impassable}`).
3. **Class of defect: fallbacks the computer player takes when its data or state is unusable.** Every fallback in `sim::ai` was audited:
   * the gathering point fell back to the start position when no passable cell was reachable from the point toward the centre (`unwrap_or(start)`), a cell outside the map in the reviewer's case and under the computer player's own yard otherwise. **Defective**: the gathering point is now, of the cells reachable from the start over passable terrain (four-neighbour connectivity, which connects the same cells as the eight-neighbour steps of equation 5) and free of buildings, the one nearest to that point (octile distance, then row, then column); if there is none, `AiController::new` returns `None` and setup fails with `MatchSetupError::NoGatheringPoint`. For the shipped and fixture data the gathering points are the same cells as before (checked by a probe against the reviewed rule), so the behaviour and the hashes of valid matches are unchanged.
   * the enemy start positions used as priority waypoints: now cells of the map with passable terrain by item 2; the sweep waypoints were already passable cells of the map.
   * the build order, harvesters, and army when a producer or prerequisite is not available: the data cases are closed by D46; a player that loses a producer in play rebuilds it when it can pay (recorded termination debt).
   * unreachable by construction and left as they are: the conversion of the point toward the centre (it lies between the start and the centre), the waypoint spacing when the army has no sight (the army is armed, so it has combat terms), the empty-wave centroid, the controller without definitions or without a player state (match setup creates both).
4. **The game.** `skirmish::check_catalogue` runs after the catalogue is loaded: `check_map_starts` for every map, `check_ai_factions` (D46), and a trial setup of every match the menu can choose, every map with every faction for the human player and every faction for the computer player (the difficulty does not change the setup), each in a simulation of its own that is dropped (`sim_host::check_match`). A start position outside its map, or a faction that does not fit at a start, now ends the game at startup with the file and field named, instead of when that match is chosen. With eight factions on the demo map the 64 trial setups take a few milliseconds in release.
5. **Tests changed** (stimulus only): `sim_host::tests::match_setup_installs_the_loaded_match_or_ends_the_game_with_an_error` used the demo map's start positions on a 16 x 16 map to make the yard not fit; that is now rejected first as a start position outside the map (asserted in addition), so the test also uses start positions on the small map near its edge, where the yard still does not fit, for its unchanged assertion.

### D46. The AI file's buildings, harvester, and army can be obtained (M6 review 1 P2-1)

1. **Rules of `load_ai`** (in addition to D42 item 4): the producer and every prerequisite of the harvester and of every army unit type are listed in the build order, so that a computer player can obtain them and rebuild them; and the power building has no prerequisites, because it is queued whenever the next building would leave the player short of power, from the first turn on. The error names the field (`harvester`, `army[i]`, `power`), the unit or building, and the missing producer or prerequisite.
2. **Rules against the factions** (the producers of buildings are usually starting buildings, which only the skirmish file defines): for every faction a computer player may play, the producer of every building of the build order is a starting building of the faction or is listed before it in the build order, and the producer of the power building is a starting building of the faction. `sim::check_ai_factions(path, ai, skirmish, definitions)` checks every faction when both files are loaded (the game's catalogue) and names the AI file, the field, the building, its producer, and the faction; `setup_match` checks every computer player's faction (`MatchSetupError::UnobtainableProducer`). A human player may play any faction.
3. **Shipped and fixture data** satisfy every rule unchanged (the yard produces every building; the war factory, barracks, and refinery are in the build orders). The shipped AI file's comment states the rules.
4. **Tests changed** (stimulus only): `m6_definitions::limits_are_accepted_at_both_ends` loaded the longest build order as a power building and 31 turrets, which no longer provides the harvester's and the army's producers; it now lists the power building, refinery, barracks, factory, and 28 turrets, and still asserts that 32 entries load.

### D47. Nothing a player or the computer player issues is lost or rejected unnoticed (M6 review 1 P2-2, P2-4)

1. **Class of defect: paths on which commands leave the game without a trace or a failed check.** `CommandSink` has one discard path, `discard`, which logs each command with the reason and counts it; `reset` (a new match) and `close` (the match is over) use it, and `uninstall_match` closes the sink before it unpauses it, so ending a running match logs the commands it drops (P2-2). The only other way commands leave the sink is `take`, into the simulation's queue.
2. **The smoke test** counts the computer player's rejected commands in every match of the run and fails if there is any (P2-4): the smoke matches end long before the players meet, so no loss can explain a rejection (D43 item 5). Its final log line reports the count.
3. **`m6_ai::a_run_with_another_seed_or_difficulty_diverges`** is renamed `a_run_with_another_difficulty_takes_another_course` (P2-3) and also asserts that the commands of the computer player whose difficulty differs and the length of the match differ; its first-hash assertion, which holds by construction, is kept. The seed is not claimed: the computer players use no randomness (implementation debt item 2).

## M7 (2026-10-04): content, balance, performance, and packaging

Evidence sources as for M6: the sources of the pinned crates in `~/.cargo/registry/src/` (Bevy 0.19.1, `ron` 0.12.2, `bevy_feathers` 0.19.1 for the FiraMono licence file), `Cargo.lock` (unchanged by M7: no dependency was added or changed, I9; `cargo tree -p sim -e normal` still contains no rendering, windowing, audio, or input crate, I1), the rustc flags Cargo passes (`cargo build -v`), runs on the development machine (macOS 27.0.1, Apple M5, Rust 1.98.1), and probe programs outside the repository (`…/scratchpad/m7-impl/probe`, built against the repository's `sim` with its `Cargo.lock`) that played computer players against each other and fought cost-equal duels.

### A1 follow-up: APIs and tools used by M7

| Concept | API or tool | Evidence |
|---------|-------------|----------|
| Optimising one package in the dev profile | Cargo profile overrides, `[profile.dev.package.sim] opt-level = 3` | `cargo build -v -p sim` passes `-C opt-level=3 -C debug-assertions=on` for `sim`; a scratch crate with the same override panics with "attempt to add with overflow" on `255u8 + 1` in the dev profile, so overflow checks stay on (I12) |
| Atomic replacement of the settings file | `std::fs::write` to `settings.txt.tmp`, then `std::fs::rename` | `options::tests::the_file_is_read_with_a_bound_and_only_if_it_is_a_regular_file` (no temporary file is left) |
| The asset directory of a packaged executable | `bevy_asset-0.19.1/src/io/file/mod.rs:19-29` (D1): `BEVY_ASSET_ROOT`, then `CARGO_MANIFEST_DIR`, then the executable's directory | `scripts/package_macos.sh` runs the packaged executable with neither variable set, from a temporary directory, and its smoke test passes; the unpackaged `target/release/game` started the same way exits 1 naming `target/release/assets/data/units.ron` |
| Packaging tools | `env -u`, `mktemp -d`, `ditto -c -k --keepParent` (macOS) | the package script exits 0 and writes `target/package/MyGame001.app` and `MyGame001-macos.zip` |

### D48. Faction rosters: the skirmish file, schema version 2 (PLAN.md section 7, M7: "the second faction, the full roster"; invariants I5, I8, I10, I11)

1. **Data.** Every faction of the skirmish file lists `roster_buildings` and `roster_units`, the building and unit types a player of the faction may produce (each 0 to `MAX_ROSTER_ENTRIES` = 64 entries, the limits of the definitions files). Every entry names a defined type with production terms (`UnknownReference`, `InvalidValue`), and none is listed twice (`DuplicateId`); a missing field is `MissingField` (no silent default). An empty roster is allowed (a faction that produces nothing). Starting buildings and units, and the free units of a placed building (a refinery's harvester), need not be in the roster. The schema version is 2 (PLAN.md section 10: a schema change increments the version and updates every data file and fixture in the same change): the shipped file and the fixture are version 2, and a file of version 1 is rejected as such. The fixture gives both fixture factions every item with production terms, so the M6 matches are unchanged.
2. **Players.** `PlayerState` holds the roster of the player's faction (`roster()`, `offers(item)`), set by match setup (`Players::add_with_roster`). A player added with `Players::add` (the scenario setups of the M3 to M6 tests) has no roster and may produce every item; the hash writes a flag byte, and with a roster its items (I3).
3. **Validation.** `QueueProduction` of an item the player's roster does not offer is rejected with the new `RejectReason::NotInRoster(item)`, checked after `UnknownItem` and before the prerequisites, so the reason does not depend on what the player owns; a rejected command changes no state (`m7_factions::production_outside_the_roster_is_rejected_without_changing_state`). `CancelProduction` and `PlaceBuilding` need no check: nothing outside the roster can be queued. The reason reveals nothing about other players (D33).
4. **The interface** shows the item buttons of the human player's roster only (`sidebar::item_shown`) and never offers an item outside it (`item_status`: not available), so a faction's sidebar holds its seven buildings and six units.

### D49. The computer player's plan per faction (amends D42 and D46)

1. **Rule.** The AI file keeps schema version 1. A computer player builds the entries of `build_order` and `army` that the roster of its faction offers, in the file's order, and skips the others (`AiDefinitions::plan_for`, the controller's `AiPlan`, which is part of the hash). `power` and `harvester` must be offered by every faction a computer player may play. This is the specified meaning of the lists, not a fallback: the checks below make the plan of every faction complete.
2. **Checks** (`check_ai_factions`, which the game's catalogue runs at startup, D44 item 1; and `setup_match` for every computer player, `MatchSetupError::AiPlan { player, faction, field, problem }`): first, for every faction, the producers of its plan (D46 item 2, now on the faction's plan); then every entry of `build_order` and `army` is offered by some faction (a dead entry is rejected at its field); then, for every faction, its roster offers the power building and the harvester, and its plan keeps every rule of D42 item 4 and D46 item 1: prerequisites listed before, a power building that produces power without prerequisites, a harvester whose producer and prerequisites the plan lists, an army that is not empty and whose producers and prerequisites the plan lists, and an army that can damage every building type. The messages name the AI file, the field (with the index in the file), the faction, and the reason (`m7_factions::the_ai_file_is_checked_against_the_roster_of_every_faction`, eight cases).
3. **Why not one plan per faction in the AI file.** That would need AI schema version 2 and a rewrite of the M6 tests that edit the AI fixture's text and expect its top-level fields; filtering keeps the file and those tests unchanged, and the checks reject everything a separate plan would have prevented.
4. **Test.** `computer_players_build_only_what_their_faction_offers_and_finish_the_match`: with red lacking the rifleman and blue the turret, neither queues what it does not offer, both queue the rest, no command is rejected with `NotInRoster`, the match ends with a winner, and two runs give equal hashes on every tick.

### D50. M7 content: the full roster, three maps, and generated art and audio

1. **Factions** (`assets/data/skirmish.ron`). The **Vanguard** (quick and versatile) fields riflemen and rocket soldiers (barracks), scouts, light tanks, and artillery (war factory), and builds gun turrets; the **Meridian** (heavy and slow) fields conscripts and grenadiers, raiders, heavy tanks, and rocket launchers, and builds rocket towers. Both share the construction yard, power plant, refinery, barracks, war factory, the new tech center (prerequisite of artillery and rocket launchers), and the harvester. Each starts with its yard and nine units (Vanguard: 3 light tanks, 2 scouts, 4 riflemen; Meridian: 2 heavy tanks, 2 raiders, 5 conscripts). Eleven unit types and eight building types in all (the "full roster"; PLAN.md does not define its size, and this is the size recorded here).
2. **Maps.** The M1 demo map (64 × 64) and two new maps generated by `crates/game/examples/gen_maps.rs`: `crossroads` (56 × 56, bases in opposite corners, three lanes between ridges and lakes) and `riverlands` (64 × 48, two banks of a river with three fords). Both new maps are point-symmetric about their centre, so the two starts are equal; their home ore fields lie off the line from the base to the map's centre, where the computer player gathers its army (a first layout put the field on that line, and the gathered army blocked the harvesters' way to the refinery for 2,700 ticks in the probe). `crates/game/tests/m7_maps.rs` checks the committed files against the generator, the terrain, the symmetry, buildable ground around each start, ore within 10 cells, that every passable cell is reachable from the first start, and that the skirmish file lists the generator's starts. All three maps have W + H ≤ 128.
3. **Art.** Every sprite and tileset is generated by code in this repository: the unit generator draws the eight new unit types (infantry as standing figures with a weapon held low at the right side, so that the facing reads in all eight frames; the raider, heavy tank, artillery, and rocket launcher as hulls with guns or a rocket rack), the building generator the tech center and the rocket tower, and the map generator textured tilesets (a per-pixel integer hash gives grass, ore nuggets, ripples, cracks, and sand grain). The three M2 and M3 unit sheets and the six M3 and M4 building sprites are byte-identical to before. The sounds and music remain synthesised (D38); no audio file exists. No third-party image, sound, or music file entered the repository (PLAN.md section 3 item 7). **Whether this procedural art and audio counts as the "final art and audio" of PLAN.md section 7 cannot be decided by a test; it is listed for the user's verification.** The licensed alternative would need third-party media with recorded licences, which this implementation did not add.

### D51. The balance pass (amends D42 item 5, D40 item 6, D23, D28)

1. **Criteria** (measured with the probe, release builds, the shipped data, every shipped map × every faction pairing × every pairing of the three difficulties = 108 matches of computer players, each with seats as set up by the menu):
   * every match ends with a winner within 36,000 ticks, with a margin (the tests' limit; `m6_production_matches` now plays all 108);
   * the factions win about equally: in the 54 matches between different factions, and in the 18 of them at equal difficulty;
   * the harder difficulty beats the easier one in most of the 72 matches between different difficulties;
   * neither seat (start position) is favoured in mirror matches.
2. **Method.** A cost-equal duel matrix of every pair of armed unit types (budget 4,200 credits, open ground, attack-move into each other) and the full set of AI matches, iterated over the values. Findings that set the values: (a) with heavy tanks behind the tech center the Meridian had no early tank and lost 13 of 17 decided equal-difficulty matches; (b) with every Meridian unit, cost, and starting group copied from its Vanguard counterpart (and the speeds equalised), the Vanguard still won 15 to 3, because the AI file's army list gave the Meridian its tank at only one position of its filtered cycle; with the same mapped order the result was 9 to 9 and no match stalled; (c) more money made matches *longer*: with 2 credits per ore and 1,000 ore per cell on the new maps, 19 of 108 matches had no winner within 36,000 ticks (a timeline showed both easy players with over 40,000 credits banked and no building lost until the ore ran out), because the armies are capped by `max_army` and production throughput, and rich bases rebuild faster than waves destroy them; (d) the last stalls were end games in which one surviving rifleman needed thousands of ticks per building, so bullets now deal 60 % to structures.
3. **Values** (balance data, I10; the files are the record): warheads against infantry/light/heavy/structure in percent: bullet 100/50/15/60, armour-piercing 35/80/100/50, rocket 30/100/100/70, high explosive 100/75/50/80, shell 90/60/40/100. Unit costs (credits / ticks): rifleman 150/120, rocket soldier 300/180, conscript 100/100, grenadier 250/160, scout 400/200, raider 450/200, light tank 700/300, heavy tank 1200/450, artillery 900/400 and rocket launcher 1000/420 (both need the tech center), harvester 1400/400. Building hit points were lowered (yard 1100, power plant 450, refinery 750, barracks 450, war factory 900, tech center 650, gun turret 450, rocket tower 400); the rocket tower fires explosive rockets (45 damage, 7 cells). One credit per ore unit (unchanged). AI file: build order power plant, refinery, barracks, war factory, gun turret, rocket tower, power plant, tech center, refinery, power plant, war factory, barracks, gun turret, rocket tower, power plant (each faction skips the other's defence); army light tank, heavy tank, rifleman, conscript, rocket soldier, grenadier, light tank, heavy tank, artillery, rocket launcher, scout, raider (each faction's filtered cycle: two tanks, two infantry, one siege unit, one fast vehicle). Difficulties unchanged from M6.
4. **Result** (final data): 108 of 108 matches end with a winner; the longest at tick 29,984 (17 % below the limit; means per map 12,973 demo, 12,448 crossroads, 9,959 riverlands); different factions: Vanguard 26, Meridian 28 wins; equal difficulty: 9 and 9; the harder difficulty won 57 of 72 (79 %); mirror matches: first seat 9, second seat 9. Harvesters stopped as `Blocked`: 0 in all 108 matches.
5. **Limits.** These are computer players against each other; how the balance feels to a person is for the user to judge. Termination is still measured, not proven (M6 debt).

### D52. Options and the settings file (PLAN.md section 7, M7: "options"; supersedes the session-only rule of D38 item 5)

1. **Options.** Master, music, and effects volume (0 to 10), sound on or off, edge scrolling on or off, and the scroll speed (1 to 10, default 5 = the M1 speed; the pan speed is multiplied by speed / 5). They are set on an options screen opened from the main menu (`MenuButton::Options`), with pure effects (`options::options_effect`), and the volume also from the sidebar as in M5. The camera reads `CameraOptions`.
2. **The settings file** `~/Library/Application Support/MyGame001/settings.txt` (macOS only, PLAN.md section 12 item 3) is read once at startup and written whenever the options differ from what it holds (a temporary file renamed over it). It is read with the game's own parser, so invariant I11 has no third-party hand-off to restrict; the parser accepts exactly: at most `MAX_SETTINGS_BYTES` = 4,096 bytes, read with that bound from a regular file (a directory, device, or named pipe is rejected before it is opened); printable ASCII and whitespace only; lines that are empty, `#` comments, or `key = value`; each of the seven keys exactly once, no other key; `version = 1` first checked; decimal integers without sign or leading zeros within their ranges; `true` or `false`. Every rejection is a `SettingsError` naming the file, line, key, and reason.
3. **No silent fallback or loss.** No file: the defaults (logged). An invalid file: a warning naming the file and reason, the defaults, and the file is replaced (with a log line saying so) only when the player changes an option. No `HOME`: a warning that the options are kept for the session only. A write error is logged. Probes with the release binary (`HOME` pointing to a scratch directory): a named pipe, a 10 MB file, and `master_volume = 99` each log the warning and the game keeps running with the defaults; the invalid file is left unchanged.
4. **Smoke runs** use a settings file of their own in the temporary directory (`smoke::settings_path`), change two options the options screen's way, require that the file reads back equal to the options, and remove it, so a smoke run never reads or changes the player's settings.

### D53. The simulation is optimised in the dev profile

`Cargo.toml` sets `[profile.dev.package.sim] opt-level = 3`. Debug assertions and overflow checks stay on (A1 follow-up above), so debug builds still trap overflow (I12) and the debug smoke test keeps its purpose. Reason: M7's tests play 108 matches of computer players with the shipped data (`m6_production_matches`, 9 matches took 52 s unoptimised in M6) and further matches with fixture data; with the override the 108 matches take about 120 s and `cargo test --workspace` about 190 s (180 s in M6 with fewer matches). This is a build setting, not a dependency or toolchain change (`Cargo.lock` unchanged). Optimising `bevy_ecs` as well was measured (28 s → 21 s for 27 matches) and not adopted, because it would change the game's debug build too.

### D54. Release package for macOS (resolves D1's packaging note, M0 debt item 7, and the OFL note of the M5 review)

1. `sh scripts/package_macos.sh [frames]` builds the release binary and assembles `target/package/MyGame001.app`: `Contents/MacOS/MyGame001` (the executable), `Contents/MacOS/assets/` (a copy of `assets/`, so Bevy finds it beside the executable, D1), `Contents/MacOS/README.md`, and `Contents/Info.plist` (bundle id `local.mygame001.game`, version from the workspace, minimum macOS 11.0). It then runs the packaged executable's smoke test from a temporary directory with `BEVY_ASSET_ROOT` and `CARGO_MANIFEST_DIR` unset, and writes `MyGame001-macos.zip` with `ditto`. Exit code 0 only if all of this succeeds.
2. **Licences in the package.** `assets/LICENSES.md` and `assets/licenses/FiraMono-OFL.txt` travel with the assets. The OFL text is copied unchanged from `bevy_feathers` 0.19.1 (`src/assets/fonts/FiraMono-LICENSE`, SHA-256 `587dd954…ecbe53`, the licence file of the Bevy 0.19.1 release for FiraMono), which the SIL OFL 1.1 requires to accompany redistributed copies of the font `bevy_text` embeds.
3. **Not done.** The application is not code-signed or notarised (Gatekeeper asks on first open on another Mac), and the licence notices of the Rust crates compiled into the executable (MIT, Apache-2.0, and others) are not collected into the package; both are listed as remaining debt for the user's release decision. The crate version stays 0.1.0.

### D55. The benchmark and its budget (PLAN.md section 7, M7; section 9 from M7)

1. **Command:** `cargo run --release -p sim --example m7_benchmark` (`crates/sim/examples/m7_benchmark.rs`), from the repository root; exit code 0 within the budget, 1 otherwise.
2. **Budget** (recorded here as the plan requires): **mean tick time ≤ 5 ms** in a release build on the development machine (the plan's initial target), over all measured ticks and over the busy part of the battle (ticks that start with at least 150 units alive); **longest tick ≤ 50 ms** (one tick period at 20 Hz, so the game never falls behind real time).
3. **Scenarios:** 200 armed units, 100 per player, set up as match setup places units and attack-moving into each other through `CommandQueue`: (1) fixture data on the 40 × 40 skirmish fixture map (riflemen and tanks; independent of the shipped balance, I10); (2) the shipped data on the demo map, each player with the armed units of one faction's roster. One warm-up tick, then 2,000 ticks timed one by one. The run also fails if fewer than 200 units were spawned, or no unit moved, fired, or was destroyed, or a command was rejected; heap use is bounded by a 1 GiB allocator budget and reported.
4. **Result** (development machine, release): fixture mean 0.075 ms (busy part 0.239 ms), longest 0.574 ms, 2,617 shots, 172 units destroyed; shipped mean 0.159 ms (busy part 0.364 ms), longest 0.854 ms, 1,192 shots, 145 destroyed; peak heap 0.6 MiB. A run with the budget set to 1 µs exits 1 with the reasons.

### D56. The soak test (PLAN.md section 7, M7; section 9 from M7)

1. **Command:** `cargo run --release -p sim --example m7_soak` (`crates/sim/examples/m7_soak.rs`), from the repository root; exit code 0 if every check holds, 1 otherwise.
2. **What it plays.** For every shipped map × faction pairing × difficulty (both players the same difficulty), 36 configurations with the shipped data, and every fixture faction pairing × fixture difficulty on the fixture map, 8 with fixture data: 36,000 ticks of computer players against each other. When a match ends before, the next match of the configuration starts in a new simulation (as the game's Restart does), so all 36,000 ticks are played by computer players.
3. **Checks on every tick:** I6 (every unit holds the cell it stands in and the cell it steps into, and neither is covered by a building), I7 (no negative credits), every rejected command explained by a loss in the tick before (D43 item 5), and the state bounded (at most 1,000 units, 300 buildings, 10,000 projectiles, 64 pending commands). Every configuration is played twice and must give the same state hash on every tick (I3). Heap use is bounded by a 1 GiB allocator budget and reported.
4. **Result** (development machine): 3,168,000 ticks (44 configurations × 36,000 × 2) in 150 s; 223 matches (179 ended within the soak, the rest were running at its end); no check failed; peak heap 1.6 MiB. A run with the unit limit set to 1 exits 1 naming the configuration, tick, and counts.

### D57. Remaining M7 changes

1. **Minimap** (M5 review 1 P2-2): the image has at least two pixels per unit of `u − v` (`scale = max(2, 256 / (W + H))`), so every cell of every map the loader accepts (1 × 1 to 256 × 256) has a pixel; the largest image is 1,024 × 512. `minimap::tests::every_cell_of_every_accepted_map_size_is_drawn_by_a_pixel` covers 11 sizes across W + H = 128 and 129 (it fails with the old `max(1)`). Amends D36 item 1.
2. **`m4_combat` temporary file race** (M5 review 1 P2-4): every call of `extreme_scenario` writes a file named by the process and a call counter; six release runs of the test binary passed.
3. **Smoke test** (D6 amended): see the notes below.
4. **Dock contention** (M3 review 1 P2-2, "at the latest with the M7 benchmark"): claiming dock cells as ore cells are claimed was implemented and measured with the reviewer's scenario (one refinery on the demo map, N harvesters, 20,000 ticks): `Blocked` events 0/0/0/0/0/4/14/43 for N = 1/4/8/12/15/20/30/60 before and 0/0/0/0/0/4/14/44 after, ore stuck 2,000/4,300/24,000 before and 2,100/5,400/23,500 after for N = 20/30/60. Claims do not help, because the harvesters are blocked on their way through the crowd, not at the same dock; the change was reverted, and the debt stays (it starts at 20 harvesters per refinery; the computer player keeps at most 2, and no harvester stopped as `Blocked` in the 108 balance matches).
5. **End rule in jams** (M2 review 3 P2-1): not revised. The benchmark shows no `Blocked` order among its 200 units at the shipped speeds, and the residue at speed 1 stays as recorded debt.

### Notes on earlier decisions (M7)

* **D6 amended in M7 (2026-10-04).** The menu has 11 buttons (Options added). In runs of at least 300 frames the last match is started after the faction row's `>` button (the human player plays the second faction; the log line of every match start names the factions and the difficulty). The run also requires that no sidebar button of an item outside the human player's roster is displayed, that the options screen has all 11 of its buttons, and that two options changed the options screen's way are written to the run's own settings file and read back unchanged (D52 item 4). The final log line reports these.
* **D1, M0 debt item 7:** packaged builds ship `assets/` beside the executable (D54).
* **D25 item 1:** "Faction ... arrive with the milestones that use them (M7)": the roster is the faction state of a player (D48).
* **D36 item 1:** the minimap scale is at least 2 (D57 item 1).
* **D38 item 5:** the volume settings are stored between sessions (D52).
* **D40 item 6, D42 item 5:** the shipped data are those of D50 and D51.
* **D44 item 1:** the catalogue's AI check is now D49's.

## User decisions

### U1 (2026-10-03): macOS is the only platform that must run

The user stated: "Just make it run on Mac is good enough." Verification on macOS is therefore sufficient, and Windows and Linux are not verified. PLAN.md section 1 still names Windows, macOS, and Linux for version 1.0. If this decision applies to version 1.0 and not only to M0, PLAN.md section 1 must be amended, and only the user may make that change.

The user also verified the M0 spike map on screen with `cargo run --release -p game`.

### U2 (2026-10-04): PLAN.md amendment approved by the user

The user approved the following changes to `PLAN.md`, applied verbatim in one commit on `milestone/m1`:

1. Section 1: version 1.0 is a desktop executable for macOS only (previously Windows, macOS, and Linux). This resolves the open question of U1 and M0 debt item 11.
2. Section 9 gains the validation command `cargo run -p game -- --smoke-test 600` (debug profile).
3. New section 12, "Orchestrated workflow and amendments": an orchestrator session may run the cycle of section 10 with a fresh subagent per implementation, review, and remediation step; a GO or GO WITH DOCUMENTED NON-BLOCKING DEBT decision leads to a merge and the tag `mN-reviewed` without waiting for the user; items that only the user can verify are listed in `docs/STATUS.md`, are not review findings, and block only the user's tag `mN-approved`; version 1.0 targets macOS only; new invariants I11 (external files handed to third party parsers or renderers are restricted to an explicitly listed supported subset, with a typed error before the hand off) and I12 (overflow-prone simulation arithmetic uses explicit checked, saturating, or wrapping operations). Section 12 prevails over sections 1 to 11 where they conflict.

No other part of `PLAN.md` was changed.

### U3 (2026-10-04): continuation, visual polish first

The user asked to read the handoff and continue toward completion, specified Apple Silicon Mac
only, accepted the existing generic names that also occur in Command & Conquer, made collection
of Rust-crate notices optional as a requested deliverable, and said the existing generated art and
audio are not final: the game should look visually good and resemble Red Alert 2.

Asked explicitly to approve an M8 amendment covering either the whole finishing backlog or visual
polish first, the user selected **"Focus on visual polish first"**. PLAN.md section 13 records this
approved narrower scope. M8 changes presentation; audio, remaining gameplay debt, signing, and
release completion remain outstanding. Existing asset/font notices are preserved. The requested
look uses original designs, with no downloaded game media.

The user's question about match setup was answered: D11/D40 initialize the world directly before
tick 0, while player and AI gameplay actions use CommandQueue. M8 does not change that boundary.
No milestone approval tag or remote push is implied by this scope decision.

## M8 (2026-10-04): visual polish first

### D58. Original industrial art with stable geometry

The existing Rust generators now draw eight distinct industrial buildings: a gantry construction
yard, twin-stack power plant with cooling fans, tank-and-pipe refinery, barracks with watch post,
factory with open vehicle bay and roof lights, radar technology center, gun turret and missile
rack. Shared `examples/art/painter.rs` supplies deterministic raster polygons, cylinders, beams,
subtle surface grain and isometric blocks. Unit sheets retain 64 × 64 frames and eight facings;
raised hulls, tracks, turrets, grilles, team panels, infantry equipment and shadows replace the
old flat grayscale shapes. `examples/art/terrain.rs` gives all three shipped maps the same muted
temperate palette, mineral clusters, rippled water and faceted stone, without the old heavy grid
outline. No map cell, terrain flag, definition, balance value, schema or simulation rule changed.

The old geometry tests remain unchanged: footprint corners, 32-pixel wall/image convention,
64-pixel unit frames and ground anchor, PNG subset, reproducibility, and front-pixel direction
centroids all pass. North antenna tips preserve the building top anchor. The asset provenance is
in `assets/LICENSES.md`; no external game media was imported. The user must still judge visual
quality and whether this first polish pass approaches the requested style sufficiently.

### D59. Team paint and military command interface

Sprites use natural material colors. At startup, only the already checked PNG bytes are decoded
through the pinned Bevy 0.19.1 `Image::from_buffer`; the human and opponent images are inserted
into `Assets<Image>` once. The opponent variant recolors the authored blue paint range to red,
preserving luminance differences and alpha; all other pixels retain their materials. Both views
use white sprite tint, so concrete and metal no longer wash into the player's color. Decode
failure names the checked sprite path and exits with an error. The disk file is never reopened,
so the existing checked-input boundary remains intact. This supersedes D20/D27's whole-sprite
tint and asset-server registration for sprite PNGs; maps retain the checked asset reader.

The sidebar uses a two-column grid of sprite icons, names, costs, lock/progress badges and queue
bars. Its 240-pixel width and input bounds remain unchanged. Buttons retain the same effects and
hotkeys. The menu retains the product name, adds an original sprite lineup and military section
labels, and labels Start as DEPLOY. The main sidebar and menu were inspected in native renderer
captures at the default 1280 × 720 logical resolution (2560 × 1440 physical pixels). Usability at
other user-selected window sizes and visual preferences still require user verification.

API evidence: the local pinned `bevy_image-0.19.1/src/image.rs` (`Image::from_buffer`, `data`),
`bevy_ui-0.19.1/src/widget/image.rs` (`ImageNode::with_rect`), and
`bevy_render-0.19.1/src/view/window/screenshot.rs` (`Screenshot::primary_window`, `save_to_disk`).
No dependency or toolchain changes. Palette regression tests preserve steel/glass/facing-marker
pixels and alpha; the menu has an executable ECS spawn regression after a duplicate TextColor
bundle was caught during development.

### D60. Native visual review and configured build directory

`MYGAME_VISUAL_REVIEW=/existing/directory cargo run -p game` is an explicit diagnostic mode,
ignored during smoke tests. It saves five native renderer captures and exits: menu, ordinary demo
start, a labelled developed-base setup, Crossroads with Meridian, and Riverlands with Vanguard.
The developed base is installed through the existing pre-tick match setup using a cloned
catalogue; the production catalogue is restored immediately afterward. No running simulation is
mutated. A regression test verifies the developed setup is valid and the opponent's starting
faction is not extended. It is a visual inspection aid, not proof of gameplay completion.

The packaging script minimally follows `CARGO_TARGET_DIR` for its executable and output package
instead of hardcoding the repository's target tree; the unset default remains `target/`. This is
required to validate this milestone with the user's configured external Cargo cache. Signing,
notarisation and the wider packaging backlog remain outside M8. The orchestrator moved the old
ignored 32 GB repository target cache to `/Users/xingyuwang/Library/Caches/Codex/cargo-target`,
which remains the configured target; no clone or worktree was created or removed. Validation
logs live in `/Users/xingyuwang/Library/Caches/Codex/reviews/MyGame001/m8`. Both are retained for
build reuse and independent review. No substantial artifact was created under `/tmp`.

### D61. Team identification survives equipment occlusion in every facing

M8 review 1 P1-1 is confirmed: front-only infantry tunic paint and hull panels hidden beneath
the rocket rack left five north-west frames with zero ownership pixels, and eight infantry
west/north frames with only four. The remediation changes only authored colors on existing
surfaces. Infantry have a wraparound upper tunic and shoulder panels; backpack upper panels
continue that identification area in a slightly darker blue. Rocket tubes carry a transverse
paint band, retaining their neutral seams and forward mouths. The production selective palette
transform and white sprite tint are unchanged; whole-sprite tint is not restored.

`m8_team_paint` passes checked committed PNG bytes through the actual production `sprite_pair`
(and thus `team_pixel`) and verifies equality with generated pixels. Each of all eleven units'
eight facings must contain at least ten opaque, changed team pixels: the established minimum
of the unaffected scout. An explicit authored palette independently checks every nonpaint pixel
and all alpha values remain identical between teams. The new test first failed on all thirteen
subthreshold frames before the art change, then passed; existing facing and geometry assertions
remain unchanged. Repaired infantry have 54–86 team pixels per frame; the rocket launcher's
minimum is 24. A separate comparison against `1a67c58` confirms every sheet's alpha and front
markers are unchanged and only uniform, backpack and rocket-tube surface colors were altered.

`cargo run -p game --example team_paint_review -- /existing/output.png` creates a deterministic
normal-scale contact sheet through the checked production decoder/palette. Rows follow the
generator roster (light tank, scout, harvester, rifleman, conscript, rocket soldier, grenadier,
raider, heavy tank, artillery, rocket launcher); columns follow E, SE, S, SW, W, NW, N, NE, with
friendly left and enemy right in each pair. The committed `docs/screenshots/m8-team-paint-1x.png`
shows all 88 pairs at their native 64-pixel frame size. Inspection confirms visible rear patches
with neutral equipment, shadows and silhouettes; the user's judgment of combat readability and
artistic quality remains required. Existing native captures are refreshed after regeneration.

This bounded correction adds no dependency, schema, simulation, audio or control changes.
Remediation logs and repeatable comparison evidence are retained in
`/Users/xingyuwang/Library/Caches/Codex/reviews/MyGame001/m8-remediation-1`; configured build/package
outputs remain in the existing Cargo cache. The first Clippy attempt caught a duplicate PNG
encoder module import in the contact-sheet example; reusing the generator's existing module
resolved it without a lint suppression. Final validation results are recorded in STATUS.

## M9 scope — unified RA2 production (2026-10-05)

The user's detailed construction/production request authorizes extending the M8 game rules,
roster and interface. Preserve the deterministic simulation and existing legacy AI/scenarios.
Use a separately versioned, bounded and cross-validated arsenal metadata file for shared
categories, mobility and specialist behavior. Allied/Soviet content is unified for the human
player; Yuri means the original RA2 infantry, not the expansion faction. Existing art is reused
as functional role icons; no third-party assets are introduced.


## RTS controls and battlefield follow-up (2026-10-05)

The user's seven-part request authorizes improvements to camera/selection, classic RTS interface,
mining, a human-only Super Me option, ownership colors, and map terrain while retaining existing
architecture. This follow-up is uncommitted work on top of the existing M9/HD working tree.
Super Me is match-local PlayerState (included in state hashing), installed only for explicitly
selected human controllers. Production retains its existing progress units and advances ten units
per eligible tick; rounding is upward to a whole simulation tick. Global definitions are unchanged.
Team colors remain presentation state because the supported match format has exactly two players.
The six-choice palette assigns distinct colors, repaints from retained source pixels on restart,
and remaps both authored faction paint ranges without tinting whole sprites.
Rate scrolling (middle or Space+left) replaces absolute dragging; arrow/minimap controls remain.
The shared terrain layout preserves base aprons, original resources, map symmetry and navigation.
New forest/civilian collisions use the existing blocked-terrain representation. This is not a new
height, bridge, civilian combat or save-file system. Existing settings schema is unchanged.

## Camera, unlocks, targeting, game speed and multiple enemies (2026-10-05)

The user's five-part request authorizes the following, uncommitted on top of the RTS controls
follow-up; existing architecture, determinism and the AI are retained.

1. Edge scrolling uses 28-pixel bands on every edge with a speed ramp, corner reach for
   diagonals, an eased velocity, and an exit latch: when the cursor leaves the window through a
   band (macOS title bar, menu bar reveal, Dock, resize region, another screen) the camera keeps
   panning that way until the cursor returns or focus is lost. macOS cannot confine the cursor
   (winit supports only locking there), so the latch replaces confinement. Hovering an interface
   button never edge-scrolls.
2. The human roster is the arsenal plus every faction's original roster: the arsenal-only roster
   of M9 had hidden the M7 Meridian/Vanguard units. Basic generators remain interchangeable, but
   an advanced generator with prerequisites (Nuclear Reactor) is a real requirement. Shipyards may
   lie 32 cells (Chebyshev) from a building of their owner instead of 4, still on visible water
   touching shore. Sidebar cards are ordered by tech tier. "Unlock all" is a human-only match
   option (PlayerState flag, hashed in the Super Me byte so existing hashes are unchanged) that
   waives prerequisites only; producers, cost, power, placement and exits still apply.
3. A right click's order is classified before the click (move, attack, harvest, friendly,
   invalid) by the same inputs as the order itself. Attack is shown in red; "invalid" (off the
   map, or explored terrain none of the selection can enter) issues no order.
4. Game speed (0.5× to 4×) sets the fixed timestep, i.e. ticks per real second. All gameplay is
   per tick, so nothing in the simulation changes; presentation animations showing gameplay use a
   separate game clock. The interface keeps real time. The speed is not saved in settings.
5. Maps have 2 to 8 start positions (`MAX_MATCH_PLAYERS`); a match uses the first n. Computer
   players are independent and hostile to everyone (free-for-all); alliances are not implemented.
   The human's defeat ends the match for the human even while computer players remain. Team
   colours are per player (8 colours); copies for computer players beyond the first are painted
   on first use rather than for every sprite.

## Large maps with environments and RA2 faction architecture (2026-10-05)

The user asked for units and buildings that look more like Red Alert 2, and for more, larger and
more complicated maps with nature, cities, ocean and different environments. Uncommitted, on top of
the follow-ups above.

1. **Cell looks.** Tiles may carry an optional integer property `look` (0 to 255), loaded into
   `sim::LoadedMap::looks` as presentation data beside the tile size; `Cell`, `Grid` and the
   state hash are unchanged, so a look can never change gameplay. Map schema version 2 is kept:
   the property is optional and every existing map and fixture stays valid; a reader without it
   rejects a map that uses it with the existing "unknown property" error. The game defines the
   values (`crates/game/examples/art/looks.rs`, shared by generators and renderer): ground in the
   low four bits (grass, dirt, sand, snow, pavement, water), feature in the high four (ore,
   cliff, road, forest, house, city block, tower, bridge). Look 0 derives both from the terrain,
   as before, and the game rejects unknown values when it checks a map. The renderer no longer
   guesses roads and villages from the map's dimensions (`battlefield_layout::detail` is now only
   used by the M1/M7 generators).
2. **Rendering.** Sand and snow are overlays with ragged edges like soil, pavement is a base;
   cliffs, forests and houses take the variant of their ground (snowy mountains, mesas; snowy
   pines, palms; snowy cottages, adobe). City blocks and towers are 64 × 128 px tops that sort
   with units and buildings (`units::unit_depth`) and take the least-revealed sight of the cells
   they cover. Open water darkens with distance to shore. Ground sprites other than water are
   updated only on a new simulation tick.
3. **Bridges** are passable, non-water, non-buildable tiles drawn over water: land units cross,
   ships cannot pass under them. No new navigation domain was added.
4. **Maps.** Five painted maps, 112 to 128 cells a side, each mixing a dominant environment with a
   city, coast or sea and nature (`world_maps.rs`): Liberty Bay, Frostline, Oasis Gulf, Coral
   Archipelago, Continental Divide. Each has six starts in mirrored pairs. Recipes paint shapes
   whose placing tools also paint their point mirror, with symmetrised integer noise; shared passes
   add beaches, clear base sites and fill unreachable pockets, and a final mirror guarantees exact
   point symmetry. The M7 checks apply unchanged (byte-identical regeneration, starts, ore within
   10 cells, connectivity, symmetry), extended to compare looks and water.
5. **Computer players on large maps.** On maps wider and higher than `ARMY_SCALE_SPAN` (128) cells
   together, `sim::scaled_for_map` scales a difficulty's `max_army` and `wave_size` with the span,
   up to twice; and a new wave first attack-moves through a staging waypoint beside the direct
   route, alternating sides (`staging_point`), giving up after `STAGING_TICKS` or once it rests.
   Without both, waves of small-map size met head-on halfway on these maps, wore each other below
   half strength and fell back for over 70,000 ticks. Point symmetry preserves orientation, so
   two players flanking on the same side take mirrored routes on opposite sides of the map. A
   wave sweeping for buildings it has not seen looks near the enemy starts first, among
   waypoints last seen in the same five-minute era (`SWEEP_ERA_TICKS`); sweeping by distance
   alone left the winner of one match wandering far corners for 40,000 ticks. All
   M7 maps keep their exact previous AI behaviour. The shipped-AI match test scales its tick limit
   by the same span (30 minutes up to 128 cells, up to an hour at 256).
6. **Art.** Buildings are remodelled with original RA2-inspired silhouettes and two material sets:
   Allied (white concrete, blue-grey cladding, glass, domes) for the shared core and Allied
   structures, Soviet (heavy concrete, brick, rust, copper) for Soviet structures. Team paint
   stays on the authored ramp, so `presentation.rs` is unchanged. Ships have hulls with bows,
   decks and superstructures, aircraft have airframes, and tanks carry team paint on their decks.
   This is original geometry, not RA2 media.

## Battle music and weapon sounds (2026-10-06; amends D38 item 1)

The music and the three weapon effects are re-synthesised, still in code with no audio file, mono at 22,050 Hz and deterministic (fixed-seed noise). The music is an original 20-bar loop (34.8 s) in E minor at about 138 beats per minute (2,400 samples per sixteenth): an opening hit, an eight-bar A section (bass riff, drums, palm-muted power chords, a brass counter-line), an eight-bar B section (brass lead over sustained power chords, march beat), and a two-bar breakdown with a snare roll and riser back into the hit. Its sources are PolyBLEP saws and sines through state-variable filters with envelopes, saturated with `tanh`, with a dotted-eighth echo and a small Schroeder reverb; two extra bars are rendered and folded onto the start so that the loop has no seam, and the mix is soft-clipped and normalised to a peak of 0.5 (about -17 dBFS RMS). `Shot`, `Cannon` (which also covers rockets) and `Explosion` are layered (transient, band-passed noise crack, pitched low body, decaying filtered tail; a whoosh for projectiles; debris for explosions) and stay at most 1.4 s. Synthesis takes about 0.5 s at startup in a debug build.

## Building repair, aircraft overflight, readable silhouettes (2026-10-06)

1. **Repair** (`Command::RepairBuilding`, amends D25). The command toggles the repair of one of the
   issuing player's buildings (validated like `SellBuilding`: `UnknownBuilding`, `NotOwned`, and
   unseen foreign buildings reported as unknown). Repaired buildings are a per-player set in
   `PlayerState::repairing`, hashed with the player. `economy::repair_buildings` runs in the
   `Economy` stage after production: every `REPAIR_INTERVAL` (5) ticks a building regains
   `max / REPAIR_STEPS` (80) hit points, so a ruin is whole in 20 s, for `repair_cost` credits — a
   complete repair costs a quarter of the building's price, each step rounded up, at most 1 credit
   under Super Me. A step the player cannot pay is skipped and the repair waits, so credits never
   go negative (I7). A repair ends when the building is whole, destroyed, sold or captured. The
   interface adds a Repair sidebar button and the `K` key (repair mode: left clicks toggle, as sell
   mode sells), and a pulsing wrench beside the health bar of every shown building under repair.
   The computer players do not repair yet, so the shipped AI matches are unchanged.
2. **Aircraft fly over ground and naval units.** The grid keeps two occupancy layers: surface
   units (land, naval, amphibious) and aircraft. `Grid::occupant` answers for the layer of the
   view's navigation domain (`navigation_for`), so movement, pathfinding, wait cycles and
   destination allocation are domain-correct without change; `occupant_for(cell, domain)` serves
   callers holding the main grid. Aircraft still do not share a cell with other aircraft
   (invariant I6 now holds per layer). `release` clears both layers, because `remove_dead`
   forgets a unit's domain before releasing its cells. The air layer is hashed.
3. **Distinct silhouettes.** `generate_hd_art.ground_vehicle` no longer derives every vehicle from
   one template: each has its own size, running gear (jeep, four- and six-wheelers, half-track,
   tracks, the Apocalypse's double tracks), turret and signature feature (Mirage tree, Tesla coil
   tower, Prism crystal, V3 missile, Katyusha rack, twin flak guns, raised howitzer, hazard
   tanker, MCV crane). Team paint moved from the whole deck to the turret or cab and a hull band.
   Infantry have per-type uniform colour, headgear, pose and oversized props
   (`generate_hd_art.INFANTRY`). Production cameos are cropped to the miniature's solid pixels
   (`hd_art::content_rect`), and hovering any shown unit or building names it with its owner and
   health (`hover_info.rs`); like the health bars, only shown views are named.

## Four more painted maps (2026-10-06; extends "Large maps with environments")

The user asked for more maps; their stated taste is many large, complex maps mixing nature, city,
ocean and environments. Four recipes were added to `world_maps.rs`, each built around one idea not
used by the first five, and registered in `gen_maps.rs` and `skirmish.ron` (six starts each):

| Map | Size | Identity |
|---|---|---|
| `steel_harbor` | 128 × 128 | A whole-map city grid (period 7); a winding river between two harbour basins with piers splits around a downtown island of towers; five avenue bridges, promenades, industrial lots for the bases, parks with ponds and contested ore. |
| `glacier_pass` | 120 × 120 | Temperate valleys with farms, woods and a fjord each, divided by a snowy massif; crossings are a winding pass north and south, a three-cell goat trail on each side and a central pass around a glacial lake with a causeway and a mining hamlet. |
| `red_canyon` | 96 × 96 | A sandstone plateau (mesa) carved into a basin per base, spokes to a central basin with a ruined adobe town around an oasis pool, and rim canyons past palm oases that join neighbouring bases (the long flanking routes). The first medium-size painted map. |
| `jungle_delta` | 128 × 112 | A lagoon with jungle islets at the centre; distributaries to the northern and southern sea and to the west and east coasts cut the jungle into six islands joined by sand bars and timber bridges; two river ports, fishing villages. |

1. **No new tiles or looks.** A new tile kind would change every map's tileset and TMX, so the
   existing maps' byte-for-byte output would change. The glacial lake is open water (its "ice" is
   the causeway, a bridge); ruins are rubble (plaza), sand and adobe houses replacing blocks.
2. **New painting helpers** (`Painter::meander`, `stroke`, `near_start`) are additions only; the
   five earlier recipes are untouched and regenerate identically. `meander` is an odd function
   about the map's centre line, so a river along it is its own point mirror and still winds.
3. **Fairness and reachability** are enforced by the existing checks (`tests/m7_maps.rs`):
   point symmetry, buildable yards, ore within 10 cells, every passable cell reachable. Unreachable
   ore is turned into forest by `fill_pockets`, so ore was placed on reachable ground (one field on
   Glacier Pass moved for this). Jungle Delta keeps the jungle thinner within 13 cells of a base so
   that bases have room to build.
4. **Computer players** finish every pairing on the new maps without changes to the AI or the tick
   limits: the slowest of the 144 new matches ended at tick 48,651 (Glacier Pass; limit 67,500).

## Ten players on maps that grow with them (2026-10-10; amends item 5 of "Camera, unlocks, targeting, game speed and multiple enemies")

The user asked for up to ten players (one human, nine computer players) on every map, and for the
map to grow with the number of players. Uncommitted, on top of the follow-ups above.

1. **Ten players.** `MAX_MATCH_PLAYERS` is 10. A map file still lists 2 to `MAX_MATCH_PLAYERS`
   start positions (the shipped files list four or six and are unchanged, byte for byte), but the
   menu offers 1 to 9 enemies on every map (`MenuCounts::enemies`). `TEAM_PALETTE` has ten
   colours: the eight of before in their order, then Lime (168, 220, 48) and Ivory (240, 230,
   196), the two candidates farthest from the eight and from each other (CIE76 in L\*a\*b\*: 42
   and 50 from their nearest colour, where Red and Orange are 30 apart). Ivory rather than white,
   which would vanish on the minimap's snow. Because the palette index is the choice plus the
   player number modulo the palette's length, the third and later colours of a match whose human
   chose Orange or Pink differ from before (Lime now follows Pink, not Blue).
2. **The map of a match is enlarged before match setup** by `sim::enlarged_map`
   (`crates/sim/src/map_scale.rs`), a pure integer function of the checked map file and the
   number of players. Map files cannot be regenerated larger (their recipes use absolute
   coordinates), and they stay the unit that is checked and compared byte for byte. Two players
   get the file's own grid, looks and start positions, cell for cell, so every one-against-one
   match, hash and AI test is as before.
3. **Size.** Every player beyond the second adds an eighth of the file's width and height; the
   cells added to a side are rounded up to an even number, half on either side of the middle
   (`enlarged_size`: 56 becomes 64, 70, 78, 84, 92, 98, 106, 112; 128 becomes 144 … 256; 128 × 96
   becomes 144 × 108 … 256 × 192). Rounding to an even number keeps the middle of the map its
   middle: with an odd number of added cells no column-wise enlargement of a point-symmetric map
   is point-symmetric. Where a step would exceed `MAX_MAP_SIDE` on either side, both sides stop
   at the last step that fits, so the map keeps its proportions (a 256-cell map is not enlarged).
4. **Terrain** is repeated, not interpolated: each column and row of the file is shown one or
   more times, in order (`axis_map`: the lower half samples cell centres, the upper half mirrors
   it). So every separate piece of land and of water stays one piece (`m13_grown_maps.rs` counts
   them), aprons around yards only grow, ore fields grow with their cells, and a point-symmetric
   file gives a point-symmetric map with mirrored starts. A start position moves with the ground
   under its yard's centre (the start plus (1, 1): the shipped factions place their 3 × 3 yard at
   the start, which `map_scale` assumes and the catalogue check enforces, below).
5. **Missing start positions are made** when the match has more players than the file lists
   starts. Candidates are cells reachable over land from the first start, 8 cells from the edge,
   whose 13 × 13 surroundings are at most a third water and at least half open ground, with no
   water or ore under the yard and the ring around it and no bridge nearby. They are chosen
   farthest first (the largest squared distance to the nearest start, ties by row and column)
   and, while two or more are missing, as a cell together with its point mirror, so that made
   bases are as fair as the map. A made start is at least 22 cells from every other
   (`MIN_START_DISTANCE`), so pads and ore fields never touch. Its pad (the 13 × 13 square less
   its corners) becomes open building ground that keeps each cell's ground look and drops its
   feature; water, existing ore and bridges beside the yard stay, and only ground joined to the
   yard is cleared. An ore field (a disc of radius 3, as the generators paint it, with the
   amount per cell of the map's own ore, at least 20 cells) is planted 9 to 10 cells from the
   yard where most of it fits on land a harvester can walk to, never within 5 cells of any yard.
   Cells only ever become passable, so no land route is cut. If no such site is left, a relaxed
   tier fills in water under the yard; no shipped map needs it at any player count, so no water
   is filled in on any of them. A map without room is an error (`MapScaleError::NoStartSite`),
   never fewer starts than players.
6. **The simulation knows three look values** (`LOOK_FEATURE_ORE`, `LOOK_FEATURE_BRIDGE`,
   `LOOK_GROUND_WATER`, and the ground mask), as `props.rs` already knew the scenery features;
   it still does not depend on the game crate (I1). A game test checks that they agree with
   `battlefield::looks`. Looks stay presentation data: the grid alone decides gameplay, and the
   made cells' looks are derived from it.
7. **Game integration.** `skirmish::match_plan` is the one place that turns the menu's choices
   into a `MatchPlan`: the enlarged grid and looks, and a copy of the skirmish definitions whose
   start positions for the chosen map are the enlarged ones, so match setup and the computer
   players use the cells the bases stand on. `MapView`, `MapTerrain`, `MapLooks`, the camera
   bounds, the minimap and the fog all read the enlarged map through the resources `spawn_map`
   inserts; `MapStarts` holds the match's start positions. The `bevy_ecs_tiled` layer still shows
   the file, which over an enlarged map would be the wrong terrain in the wrong place: it is
   spawned hidden (`HiddenTiles`; its creation still reports that the checked file loaded and is
   aligned) and not tinted, and the battlefield renderer, which already draws every cell on top
   of it, draws the map alone. A restart replays the same choices, hence the same map. The map
   row of the menu shows the size, `liberty bay  176 x 176`.
8. **The catalogue check** sets up every map for every player count from 2 to 10 at startup:
   every faction pairing for 2 and for 10 players, one pairing (changing with the count) for the
   counts between. A skirmish file whose factions do not fit at made start positions (a yard far
   from the start) is rejected there, as a file whose start lies off its map is.
9. **Computer players** are unchanged. Enlarged small maps cross `ARMY_SCALE_SPAN`, so their
   armies scale and flank as on the large maps; that is intended.
10. **Measured** (Apple M5; `cargo run --release -p sim --example m13_ten_players`, optionally
    `-- to-the-end all`). Enlarging a 128 × 128 map for ten players takes 2 to 5 ms. Ten computer
    players on 256 × 256 cells: mean tick 0.42 to 0.45 ms over 24,000 ticks, 99th centile 1.9 to
    3.2 ms, longest tick 13 to 20 ms (budget: mean 5 ms, D55), with at most 277 units and 97
    buildings. The catalogue check now sets up 225 matches instead of 120 and takes 0.45 s
    instead of 0.06 s in a release build (0.60 s instead of 0.12 s in a debug build); a ten-player
    setup on 256 × 256 cells takes about 8.5 ms, a two-player one 0.6 ms. The battlefield draws one to four sprites per cell (four
    times as many on a doubled map) and the minimap image of a 256 × 256 map is 1024 × 512
    pixels, redrawn every tick; a tour of seven ten-player maps with all terrain drawn
    (`MYGAME_MAP_REVIEW` with `MYGAME_MAP_REVIEW_ENEMIES=9`, release build) ran at 16.6 to 16.7 ms
    a frame, the 60 Hz of the display, with a longest frame of 22 to 42 ms (the captures), as
    the tour of the two-player maps does, so no cap on the scale was needed. With the display's
    rate as the floor this shows no loss, not how much headroom is left.
11. **Known limits.** All-against-all matches of ten computer players (a test format; the menu's
    computer players are allied against the human) end on 13 of the 15 maps within 15,577
    (Crossroads) to 58,622 ticks (Steel Harbor). Pacific Atoll takes 152,818 ticks: its last
    three players wear each other down around the hub of the atoll with their credits at zero. Coral
    Archipelago does not end within 400,000 ticks: the last two players are left with one or two
    buildings each, no construction yard, no unit and no way to produce one, and neither ever
    attacks. That is the win condition and the computer player of D41 and D43 (a player lives
    while it owns a building; `broke` only helps a player that can still produce an army unit),
    not the enlargement, but the more players a match has, the likelier such a remnant survives
    its conqueror. Enlargement repeats cells, so roads, bridges and rivers become wider rather
    than more detailed, and a made start position is plain cleared ground. `map_scale` assumes
    the yard at the start position (item 4).

## Maps at the engine's largest size (2026-10-11; extends "Large maps with environments" and "Ten players on maps that grow with them")

The user asked for three things: maps "much much larger, so that the distances between each team
can be very far", more diversity in them, and more maps that meet both. Uncommitted, on top of the
follow-ups above.

1. **How large.** `sim::MAX_MAP_SIDE` stays 256. The battlefield draws one to four sprites per
   cell and re-tints them every tick (65,536 cells is 60 Hz with headroom unmeasured, see item 10
   of "Ten players"); 512 a side would be four times that and needs a renderer that draws only
   what the camera sees, which is a rewrite of `battlefield.rs`, not a map change. So the large
   map files are now painted at or near that limit instead of at half of it: the twelve painted
   maps doubled (128 to 256 a side, 112 to 224, 120 to 240, 96 to 192: four times the area), and
   the six new ones are 256 x 256. The first pair of start positions is the pair farthest apart,
   so a one-against-one match is fought across 186 to 276 cells (255 or more on eleven of the
   eighteen large maps; 157 on Pacific Atoll, whose bases stand on a ring) where it was 78 to
   137. The three M1/M7 maps (`demo`, `crossroads`, `riverlands`) are unchanged: hundreds of
   tests place units on them by coordinate, and they remain as quick small matches. The menu
   opens on the first of the largest maps (`MenuSelection::opening`) instead of on `demo`.
2. **Repainted, not repeated.** Enlarging by `sim::enlarged_map` repeats cells (wider roads and
   rivers, no new detail). The recipes in `world_maps.rs` were instead rewritten in the
   coordinates of the larger map: positions and radii doubled, noise scales about doubled or
   less (so woods and fields are more numerous rather than merely bigger), roads and city blocks
   keep their width, so a doubled town has four times the blocks. Because a doubled noise scale
   samples the same lattice, the coasts and ridges keep their old outline, drawn smoothly.
3. **Growth with players** (`enlarged_size`) is unchanged, and so is its cap: a 256-cell file is
   played as it is by any number of players, a 240-cell one too (the first step would pass 256),
   224 grows once, 192 twice. Ten players on a 256 x 256 map have the room ten players had before
   on a doubled 128 x 128 one. The large maps now list six to ten authored starts, so fewer are
   made at run time.
4. **New grounds and one feature** (`looks.rs`; the simulation still only knows passable, water,
   buildable and ore): `Ice` and `Marsh` are passable and not buildable (the properties of the
   beach), `Crops` and `Ash` are building ground, and the feature `Lava` is impassable and not
   water, so neither armies nor ships cross it. Each ground is a world-periodic overlay like snow
   (`terrain/20.png` to `24.png`, `scripts/generate_hd_terrain.py`); lava is drawn as an overlay
   over its ash with an uneven glow that hides the texture's four-cell period, and the rock
   terrace on ash is darkened to basalt (`Ground::stain`) rather than rendered anew. Existing
   tops are reused on new grounds without new art: orchards and farm tracks on earth, farmhouses
   in crop, city blocks and towers on snow and sand (`Biome::block`, `tower`), mangroves on
   marsh, icebergs on ice. The tileset grew from 26 to 46 kinds, so every generated TMX changed
   (the terrain of `crossroads` and `riverlands` did not).
5. **Diversity by tool, not by hand.** `Painter::landmarks` places a landmark on a jittered grid
   over the open country (never within about 15 cells of a base), chosen by a hash and by the
   environment at that cell: copses, crags, hamlets, small ore fields and ruins everywhere;
   ponds in their marsh, farms in their fields and orchards in green country; frozen ponds in
   snow; oases and dry washes in desert; vents and hot springs on ash. Landmarks only take open
   ground (`Painter::patch`), so no road, river, town or ore field is touched, and roads are
   painted after them, so every road stays whole. `Painter::home_ore` gives every base two ore
   fields nine cells from its yard, on the two sides with the most room, so a start position can
   be moved or added without placing its ore by hand (the tests still require ore within ten
   cells of every start).
6. **The six new maps** (`frontier_maps.rs`) are each built on one of the new grounds or on a
   topology the others lack: `ember_caldera` (lava rivers quarter a volcanic island; fords where
   the flow has cooled), `frozen_sound` (a sheet of sea ice between two shores: the middle third
   of the map is unbuildable manoeuvre ground), `harvest_plains` (a patchwork of fields and
   hedgerows: open country, one river), `black_water` (marsh everywhere, building ground only on
   hummocks), `thousand_lakes` (a web of necks between lakes; frozen lakes on the fells) and
   `wheel_of_worlds` (ten sectors of five environments, whose borders wander by a displacement
   that is odd about the centre, so that opposite sectors mirror). `MAX_MAPS` is 24.
7. **Computer players are unchanged** (`ai.rs` untouched). Their army scaling was already capped
   at twice the file's values from 128 + 128 cells up, and that is enough: with the shipped data
   all 756 one-against-one matches (21 maps, every pairing of two factions and three
   difficulties; `cargo run --release -p sim --example m14_duels -- full`) end with a winner,
   the slowest on each large map between tick 47,310 (Frostline) and 126,125 (Glacier Pass), within the
   limits of `m6_production_matches.rs` (36,000 ticks times the map's width plus height over
   128; 144,000 on a 256 x 256 map, 135,000 on Glacier Pass). One map needed a change for it:
   Coral Archipelago's three-cell causeways, twice as long as before, let an easy-against-easy
   match run to tick 172,302; at five and six cells its slowest match ends at 86,587. That is the
   narrow-link stalemate recorded under "Large maps" again: a link's width has to grow with its
   length.
8. **Tests.** `m7_maps.rs` checks the 20 generated maps as before (byte-identical regeneration,
   symmetry, base sites, ore, every passable cell reachable) and no longer caps a file at
   128 x 128. The end-to-end AI tests (`m6_production_matches.rs`, and `m6_ai.rs` on the shipped
   maps) play their matches on all cores, and `m6_ai.rs` scales its tick limit with the map as
   the production test does (its fixed 36,000 ticks were set when no map was larger than 64
   cells). `m6_match.rs` checks the army scaling on grids made for the purpose instead of on two
   map files of a particular size. The production test now accepts what D43 and `find_placement`
   always allowed and `m6_ai.rs` already accepted: a placement rejected because another player's
   unit, unseen beside the footprint, stepped into it. Four of the 756 matches show it (an army
   reaching a base that is still building); none of the 540 on the smaller maps did. The
   several-player test of `m10_unlocks_and_enemies.rs` plays at most six players per map, as
   before the files listed more, scales its limit with the map and uses all cores; the growth
   test of `m13_grown_maps.rs` knows that a map stops growing at the last eighth that fits. `MapSpec` lists each mirrored pair's first start (`homes`) and
   derives the rest.
9. **Known limits.** 256 cells a side is the ceiling until the terrain renderer culls. On the
   largest maps a lava lake or an ice sheet of dozens of cells shows the overlay's four-cell
   period at close zoom (the lava's glow variation hides most of it). A made start position on
   ice or marsh keeps that ground's look although its pad is building ground (`map_scale` keeps
   each cell's ground look); the large maps list enough starts that this needs eight or more
   players on a map of six. Pacific Atoll and the other ocean maps leave much of their 256 x 256
   cells to open sea, which is their point, but a land-only player sees less of the map's size.
10. **Evidence.** `docs/screenshots/largest-maps/layouts/` holds the generator's picture of every
    map (`gen_maps preview`, three pixels a cell), and `docs/screenshots/largest-maps/` native
    captures of the new maps and grounds (`MYGAME_MAP_REVIEW`, release build, scaled to 1600
    pixels wide). That tour drew every 256 x 256 map at 16.67 ms a frame, the display's 60 Hz,
    with a longest frame of 18 to 19 ms.
11. **Ten players** (`cargo run --release -p sim --example m13_ten_players -- to-the-end all`,
    which now also takes map ids). All-against-all matches of ten computer players end on all 21
    maps, between tick 15,577 (Crossroads) and 96,367 (Ember Caldera); Coral Archipelago (49,881)
    and Pacific Atoll (51,699), which did not end or took 152,818 ticks on their doubled 128-cell
    files, end like the others now that every base has its two ore fields and the causeways are
    broad. Tick times on an idle machine over 24,000 ticks, at most 360 units and 138 buildings:
    mean 0.39 to 0.71 ms and longest 8 to 22 ms on Liberty Bay, Continental Divide, Pacific Atoll
    and Glacier Pass (budget: mean 5 ms). Frostline is the exception: mean 1.5 ms, 99th centile
    12 ms, and 8 ticks of 24,000 over the 50 ms of a tick period (longest 89 ms). The time is
    path searches (`find_path` and its heap; then `combat::keep_or_acquire`): a unit that waits
    behind others asks for a way around them, and when there is none the search visits every
    cell it can reach before the next fallback starts, which on a map of 63,504 cells of mostly
    connected land costs four times what it did on 12,544. Wider passes and boulevards through
    the town changed nothing measurable (tried and reverted), so it is not one bottleneck of the
    map; it is the cost of the existing pathfinder on a large open map with ten armies, and is
    left as it is: results would have to stay identical cell for cell for the hashes the tests
    compare. One-against-one matches do not show it (756 matches in about 25 minutes of ten
    cores).
12. **Verified** on 2026-10-11: `cargo fmt --all -- --check`, `cargo clippy --workspace
    --all-targets -- -D warnings`, `cargo test --workspace` (529 passed, 0 failed, 0 ignored;
    about 35 minutes, of which the 756 production matches take 26 and the fixture-AI matches 6)
    and the same in release (about 25 minutes), the release and debug smoke tests of 600 frames, the M7
    benchmark, and the two sweeps above.

## RA2 parity programme (2026-10-11; `docs/prompts/RA2_PARITY_PROMPT.md`)

The user asked for the prompt in `docs/prompts/RA2_PARITY_PROMPT.md` to be carried out in one run on
the branch `ra2-parity`: milestones RA0 to RA9, gated only by the validation commands, with one
independent review of the whole programme at the end. `docs/RA2_PARITY_PLAN.md` is the plan and
prevails for this programme (PLAN.md section 14). The decisions below are numbered R1, R2, ... and
continue per milestone.

### R1. Fixed decisions of the prompt (recorded, not chosen here)

1. The RA2 ruleset is the default skirmish ruleset. Super Me, ALL UNLOCKED, the Super Me unit
   speed, fog of war, the global rally key and maps that grow with the player count stay as menu
   options or extensions; where one conflicts with RA2 behaviour, RA2 behaviour is the default.
2. The Vanguard and Meridian rosters and their twelve original items leave the default skirmish.
   Tests that rely on them move to fixture data with the same assertions (invariant I10).
3. Computer players are allied against the human by default; a free for all option is added.
4. RA2 reference files are obtained into `~/Library/Caches/MyGame001/ra2-reference/` and are never
   committed, nor is any other RA2 file. Ported values carry a comment naming the ini section and
   key.
5. New visuals and sounds are original and come from the repository's generators.
6. Invariants I1 to I12 stay in force; only I6 may be amended (infantry sub cells, RA3).
7. One branch, one or more commits per milestone, existing M tags and test file names unchanged;
   the per milestone review cycle of PLAN.md sections 10 and 12 is replaced, for this programme
   only, by the single final review of the prompt's section 7.

### R2. Reference data provenance (confirmed facts)

| Item | Value |
|---|---|
| Repository | `https://github.com/otherdeniz/CnC_RulesEditor` |
| Commit | `b7278f2e1dab098c5a2f05d4784fffe70a4e5f9c` (the remote's `HEAD` on 2026-10-11) |
| `Deniz.TiberiumSunEditor.Gui/Resources/RA2rules.ini` | saved as `rules.ini`, 518,789 bytes, 23,126 lines, SHA-256 `adc869ca037db5141f562f18092d022045e194de0d1204a46b9ec8372b2b826f` |
| `Deniz.TiberiumSunEditor.Gui/Resources/RA2art.ini` | saved as `art.ini`, 231,105 bytes, 14,720 lines, SHA-256 `d732bfc8ffb6d52908342254c22731acb730ae6d2bbfb41983ef109e48a8f061` |
| Location | `~/Library/Caches/MyGame001/ra2-reference/` (outside the repository) |

Assumption, not proven: the file is the rules.ini of patch 1.006. It carries no version number;
its `[General]` name is "Red Alert 2 -- Official Rules of Engagement" and it has no Yuri's Revenge
sections. What the repository commits instead of the files is a table of the numbers and ini keys
the game uses (`crates/sim/tests/reference/`, written by `scripts/ra2_reference.py` from the
cached files), which the data parity audit of RA9 compares the shipped data with.

### R3. Build directory

The prompt says to use the configured `CARGO_TARGET_DIR`. None is configured in this environment
(the variable is unset and neither `.cargo/config.toml` nor `~/.cargo/config.toml` sets a build
directory; the `~/Library/Caches/Codex/cargo-target` of earlier reports no longer exists), so
Cargo's default `target/` in the repository is used, which `.gitignore` excludes. Validation logs
are kept under `docs/validation/ra2-parity/<milestone>/` by `scripts/validate_ra2_parity.sh`, which
runs the eleven commands of the prompt's section 9 in order and records each exit status.

### R4. Maps that grow with the players become an option, default off

Fixed decision 1 names "maps that grow with the player count" among the features kept "as menu
options or extensions, with RA2 behaviour as the default wherever the two conflict". RA2 plays a
map at its own size with at most as many players as it has start positions, so the two conflict.
From RA1 the menu has a row MAP GROWTH (OFF, ON), default OFF: the map is played as its file
defines it, and the number of enemies is limited to the start positions the file lists less one
(3 to 9). With ON the behaviour of "Ten players on maps that grow with them" is unchanged
(`sim::enlarged_map`, made start positions, up to ten players). The catalogue check and the
grown map tests keep covering both. This reverses a default the user asked for on 2026-10-10, on
the strength of the later prompt; it is listed for the user in the final report.

### R5. End to end matches pair sides, with rotating countries

`m6_production_matches`, `m14_duels -- full`, `m7_soak`, `m13_ten_players` and the game's
catalogue check played every pairing of the two factions. With nine countries every pairing
would be 81 instead of 4 (15,309 one against one matches instead of 756). From RA1 they play
every pairing of the two sides, and the country of each side rotates with the map and the pairing
so that every country is played on several maps; the number of matches (756) and of soak
configurations is unchanged. The tests' assertions are unchanged: every match ends with a winner
within its tick limit, without a rejected command the test does not allow, and with equal hashes
where the test compares them.

### R6 is reserved for the amendment of invariant I6 in RA3.

### R7. Speed is converted with the time base of the other rates, not anchored on the GI

The prompt's section 3 proposed `subcells per tick = round(Speed x 14 / 4)`, which keeps the GI at
its present 14, and required the plan to confirm that anchor or justify another. ModEnc and EA's
released Red Alert source agree that the engine reads `Speed` as leptons per frame,
`min(Speed x 256 div 100, 255)`, and ModEnc adds a factor of 3/2 for infantry on foot. The plan
therefore converts speed like every other rate, with 15 frames to 20 ticks:
`round(leptons per frame x 3 / 4)`. Confirmed fact: the definition of `Speed`. Single source: the
infantry factor. Assumption: `JumpjetSpeed` is leptons per frame (undocumented). Consequence: at
1x game speed units move as in RA2 at its nominal 15 frames per second, slower than before RA1
(GI 11 instead of 14 subcells per tick, Grizzly 13 instead of 28); the ratio of movement to rate
of fire, build time and recharge is RA2's at every game speed, which the proposed anchor would
have distorted by a factor of 1.2 for infantry and 1.9 for vehicles. The game speed setting is
unchanged (2x corresponds to RA2 at 30 frames per second).

### R8. The soak plays its configurations on all cores and accepts the documented placement rejection (amends D56)

Found by the baseline validation of RA0, before any change to the game; both changes are in
`crates/sim/examples/m7_soak.rs` only.

1. **Time.** With the 18 large maps painted at 192 to 256 cells a side (2026-10-11) a
   configuration takes 45 to 60 seconds for its two runs instead of about one, and the 260
   configurations, played one after the other, about three hours; the first baseline run was
   stopped after 47 (`docs/validation/ra2-parity/ra0-baseline/09-soak-sequential-partial.log`).
   The soak is a validation command of every milestone, so it now plays its configurations on
   all cores, as the end to end tests and `m14_duels` already do. Nothing it checks changed: the
   same configurations, 36,000 ticks each, played twice on one thread with the state hash
   compared on every tick, I6, I7, the rejected command rule and the state limits on every tick,
   and a report in the same order. The 47 lines of the sequential run and the first 47 of the
   parallel one are identical apart from the seconds.
2. **The allocator.** The first parallel run took 9,578 seconds, no faster than one thread: the
   soak's budget allocator updated two shared counters on every allocation, and ten threads
   waited for each other's cache line. Each thread now keeps a credit of bytes it has already
   counted (`CREDIT_CHUNK`, 256 KiB), so the shared counter is touched once per chunk. The budget
   (1 GiB) is unchanged and is enforced at most 512 KiB per thread early, never late; it and the
   reported peak now cover all configurations that run at once (207 MiB for ten at a time, where
   one match used 1.8 MiB on the small maps). The parallel run takes 1,894 seconds.
3. **A rejected placement that the tests already accept.** The soak had not been run since the
   maps were doubled, and failed on two of the 260 configurations (Liberty Bay and Steel Harbor,
   Vanguard against Meridian, both easy): a `PlaceBuilding` of the computer player was rejected
   because a unit of the other player, which it could not see beside the footprint, had stepped
   into it. D43 and item 8 of "Maps at the engine's largest size" allow exactly this, and
   `m6_production_matches.rs` and `m6_ai.rs` accept it; the soak's rule now accepts it too (a
   placement rejected as occupied by a unit of another player), and still fails on any other
   unexplained rejection, including a footprint occupied by the player's own unit.

### R9. RA1: the data pipeline, the generated data files and their schemas

1. **Pipeline** (plan section 3.1). `scripts/generate_ra2_data.py` replaces
   `scripts/generate_arsenal.py` and writes all seven files of `assets/data/` from the committed
   tables `crates/sim/tests/reference/ra2_reference.tsv` and `ra2_mapping.tsv` and from its own
   tables of game specific values (labels, descriptions, categories, roles, sprites, footprints,
   build times, armour, weapons, harvesters, the computer player's plans, the maps). It never
   reads the ini files. Every ported value carries a comment naming its ini section and key; the
   files say that they are generated. `--check` fails on a stale file.
2. **What RA1 ports**: cost, hit points (`Strength`), sight, speed (R7), prerequisites, power,
   sell value (`Cost x RefundPercent`), the free unit of a refinery, owner sides, required and
   forbidden countries, tech level, factory kind, and every global value of `rules.ron`. Build
   times, footprints, armour, warheads and weapons are unchanged from before RA1 (the seven
   weapons that only the legacy armies used stay in `combat.ron` until RA3 replaces the table).
   The eleven new ids of plan section 3.3 and the four stolen technology units take the game
   specific values of an existing item (their Allied twin; Tanya, Crazy Ivan and Yuri for the
   four). The Kirov and the NightHawk are built at the War Factory.
3. **`rules.ron`, schema 1** (`crates/sim/src/rules.rs`, `Definitions::with_rules`): groups
   `production`, `economy`, `veterancy`, `crew`, `combat`, `radiation`, `superweapons`, `prism`,
   `crates`, `powerups`, `dialog`, `difficulty`, `prerequisite_groups`, `sides`, `countries`;
   100 scalar values in game units, yes and no as 1 and 0. It is an extension file: a world
   without it keeps the behaviour of M2 to M9. RA1 consumes `sides`, `countries`,
   `prerequisite_groups` and of `dialog` the tech level and the ranges of credits and unit
   count; the rest is loaded, range checked, hashed and audited for the later milestones.
   **The base unit of a side lives here** (`sides[].base_unit`, with the yard it deploys into),
   not in `skirmish.ron` as the plan's item 6 words it: `Deploy`, `Undeploy` and the win
   condition read it from the world's definitions, where the skirmish file is not.
4. **`arsenal.ron`, schema 2**: `art`, `side`, `factory`, `owner_sides`, `required_countries`,
   `forbidden_countries`, `tech_level`, and, beyond the plan's list, `prerequisite_groups`: the
   group tokens of the ini's `Prerequisite` (`POWER`, `RADAR`, ...). The single buildings of that
   key stay in `economy.ron`, whose schema the plan leaves at 1; a group cannot be named there.
   The rules are loaded before the arsenal, which names their countries and groups.
5. **`skirmish.ron`, schema 3**: `sides` (the starting unit types per side) and per faction
   `label` and `side`. A faction with a side is a country: a country of the rules file, with
   empty roster and starting lists; its `FactionDef::roster` holds its own tech tree
   (`sim::country_tree`) for callers that ask what a faction offers. `MAX_FACTIONS` is 16 (was 8).
6. **`ai.ron`, schema 2**: `plans`, one per side (`"none"` for the factions without a side);
   error field paths are now `plans[i].build_order[j]` and so on.
7. **The audit** (`crates/sim/tests/ra9_data_parity.rs`) recomputes the conversions in integer
   arithmetic and compares, for every mapped unit and building, cost, strength, sight, speed,
   prerequisites, power, sell value, owner sides, required and forbidden countries and tech
   level, every value of `rules.ron`, and the ported values of `skirmish.ron`. It fails on an
   unexplained difference, on a stale row of `ra2_differences.tsv`, on a mapped section or key
   the reference lacks, on a scalar of `rules.ron` without an audit row, and on a shipped item
   that is not mapped. The differences of RA1 (18 rows): the single engineer and attack dog
   (four rows of `forbidden_countries`), the four stolen technology units (tech level -1
   instead of 9 until RA6), and ten mapped ids that enter the game in RA4 to RA6 (`present`).
8. **Debt.** `arsenal.ron` is 46 KiB of the 64 KiB a data file may have
   (`MAX_DATA_FILE_BYTES`); a later milestone that adds traits must shorten its comments or
   raise the limit. The default `starting_credits` of a skirmish file is not checked against
   the lobby range of the rules file by the loader (the audit checks the shipped value).

### R10. RA1: who may build what (assumption RA-A3)

`QueueProduction` is validated by `sim::check_buildable`, which the interface and the computer
player call too. In this order, the first failure being the reason:

1. `UnknownItem`: no production terms, or an arsenal tech level below 0 or above that of the
   rules (`[MultiplayerDialogSettings] TechLevel=10`). This is how the Construction Yards
   (`TechLevel=-1`) and the stolen technology units stay out of the queue; no new reason was
   added for it, since `UnknownItem` already means "cannot be produced".
2. `NotInRoster`: the roster of a faction without a side, as since M7. A country's player has
   no roster.
3. `DisabledByOptions`: a superweapon building while the match has superweapons off.
4. `MissingPrerequisite`: a building of the economy file's list, then a prerequisite group of
   the arsenal of which the player owns no member (reported with the group's first member).
   "Unlock all" waives this step only.
5. `NoProducer`: no factory of the item's kind. `NoFactoryOfSide` (new): factories of the kind,
   but none of a side in the item's `owner_sides`.
6. `CountryRequired`, `CountryForbidden` (new): the player's country.

A **factory group** (`buildings`, `infantry`, `vehicles`, `naval`, `aircraft`) is a trait of a
building; an item's `producer` means "a building of the producer's group", and a queue is kept
per group (`Definitions::queue_key`: the first building type of the group), so the Allied and
the Soviet War Factory share the vehicle queue, as do the two Airforce Commands. A building
without the trait is its own group, which is the rule of M3. The head of a queue progresses
while the player owns a factory that produces it (`sim::produces`: of the group and of an owner
side) and its prerequisites; a unit leaves such a factory.

**Assumption RA-A3** (the plan's): a factory's side is the side of its building type. An
Allied player who captures a Soviet Barracks trains Conscripts there; a country's special stays
that country's, whoever owns the factory. Because almost every ini `Prerequisite` names the
item's own factory, the reason a player usually sees for an item of the other side is
`MissingPrerequisite`; `NoFactoryOfSide` appears for items without such a prerequisite and with
"unlock all".

Without a rules file the basic power plants still stand in for each other as since M9
(`Definitions::prerequisite_met`); with one, power is a prerequisite group like the others.
The engineer and the attack dog are one type each, trained by any barracks (owner sides both,
group `barracks`; listed differences).

### R11. RA1: art for the new ids

`arsenal.ron` names per item the id whose art the game draws (`art`, a defined item of the same
kind). The nine new buildings and `soviet_mcv`, `soviet_amphibious_transport` show the art of
their Allied twins, the four stolen technology units that of Tanya, Crazy Ivan and Yuri; every
other item its own. The game looks HD sheets and walk cycles up through `Definitions::art`; the
legacy sprite sheet of a type is still named by `sprite` in `units.ron` and `economy.ron`. No
asset was added in RA1-B, so `assets/LICENSES.md` gained no file. That the two sides' buildings
look alike until they have art of their own **requires user verification** as a visual
shortcoming, and is debt for the milestones that add art.

### R12. RA1: deploying and undeploying

* `Deploy` of a base unit turns it into the Construction Yard of its side (the rules file's
  `sides[].construction_yard`) with the footprint's origin in the unit's cell; without a rules
  file, into `construction_yard` as since M9.
* `Command::Undeploy { building }` (new) turns a Construction Yard into the base unit of the
  yard's side, with the same id, standing in the cell of the footprint's origin; a yard of the
  other side in a player's hands undeploys into that side's base unit. Rejections:
  `UnknownBuilding` (no such building, or another player's that the player does not see, D30),
  `NotOwned`, `DisabledByOptions` ("MCV repacks" off), `AbilityUnavailable` (not a yard, or no
  room). Free units still waiting in the building are lost, a repair ends, and the queue of the
  buildings group waits like for any lost producer. `SimEvent::BuildingUndeployed` reports it.
* **Assumption** (no source states the engine's rule): with a rules file the yard and the unit
  keep the share of hit points across both changes, rounded down, at least 1, so that neither
  repairs anything. Without a rules file `Deploy` gives a whole yard, as before.
* In the game D with no unit selected and the cursor on a building issues `Undeploy`
  (`special_controls`); RA1-C or RA7 may rebind it.

### R13. RA1: match options, the start of a match and the win conditions

* `MatchOptions` (default: Red Alert 2's lobby): `free_for_all` replaces `allied_ai` with the
  opposite sense (fixed decision 3: computer players are allied against the human unless the
  match is free for all); `random_countries` lists the players whose faction is drawn from the
  countries of the skirmish file with `SimRng`, in ascending player id, after the checks that
  do not depend on the draw; `starting_credits` and `unit_count` are optional and default to the
  skirmish file's credits and the rules file's unit count, and are checked against the ranges
  of the rules file (`MatchSetupError::Options`); `mcv_start`, `short_game`, `superweapons`,
  `crates`, `mcv_repacks` default to on (the audit compares them with
  `[MultiplayerDialogSettings]`). `short_game`, `superweapons`, `mcv_repacks` and `crates` are
  kept in `MatchState` (`MatchRules`) and hashed. `crates` has no effect before RA5.
* **`grow_map` is not an option of the simulation.** The map is enlarged before the setup by
  whoever installs the grid and the start positions (`sim::enlarged_map`), which is the game;
  the option belongs to the game's menu (RA1-C, R4).
* The entry points `setup_match`, `setup_match_with_super_me` and `setup_match_with_options`
  behave as before for fixture data: they pass `free_for_all: true` (no teams, their old
  behaviour), and a faction without a side still starts with its lists and its roster.
* A country's player starts with the base unit of its side on the start position (or, with
  `mcv_start` off, the yard with its footprint's origin there, which is where deploying the base
  unit in place puts it) and the starting units of plan section 4.9 (`sim::starting_units`) on
  the free cells nearest to the start, 3 or more cells from it. A type is drawn with `SimRng`
  only when a side lists several; vanilla lists one vehicle and one infantry type per side.
* Win condition with a rules file: in a short game a player is defeated without buildings and
  without a base unit of any side; in a long game without buildings and without units
  (**assumption**, appendix D topic 15: "every remaining object"). A unit carried by one of the
  player's transports counts. Without a rules file: no building (D41). A defeated player's
  remaining units stay on the map, as before.

### R14. RA1: the computer player per side

* `ai.ron` has a plan for the Allied and one for the Soviet side, from ground units and base
  defences that every country of the side can build on its own (the loader checks each entry
  against `country_tree` of every country of the side): no national special and, because
  America may not build the common Airforce Command, no radar building on the Allied side yet.
  RA8 extends the plans. The difficulties are `easy`, `medium`, `brutal` with the values the
  former `easy`, `normal`, `hard` had.
* A plan of a side starts from the yard its base unit deploys into: the yard counts as listed
  before every entry, and the power building may need it.
* In its turn a computer player that owns a base unit deploys it where it stands and gives no
  other command in that turn; in its first two turns it waits while it does not see the yard's
  footprint (the first turn of player 0 comes before the first `Visibility` stage), so that its
  starting units do not walk into the footprint. Its own idle units beside the footprint do not
  hold the deployment back. The gathering point keeps clear of the future yard.
* **An armed harvester is no army unit** (a defect found by `m13_grown_maps` with the shipped
  data, fixed in `ai.rs` with the regression test
  `ra1_match.rs::an_armed_miner_of_a_computer_player_mines_and_is_not_drafted_into_its_army`):
  the computer player took every unit with a weapon for its army, so it called the Soviet War
  Miner, which has one, to the gathering point and sent it with the waves, and a Soviet
  computer player earned nothing. The army is now the armed units that do not harvest, and the
  AI file rejects a harvester in an `army` list.
  Only as a last resort does an armed harvester fight: while its player has no other armed
  unit and the harvester's cycle has stopped (`View::fights`). Without that, on riverlands
  France (brutal) and Cuba (easy) wore each other down until neither had an army, a producer
  or income, and the match did not end in 300,000 ticks; regression test
  `m6_production_matches.rs::a_soviet_player_left_with_an_armed_miner_still_finishes_the_match`.
  **Debt** (M6 behaviour, not met in any match): two players that are both left without army,
  producers, income and armed harvester would still never finish.
* **A sweep starts near the enemy's start on every map.** A wave that knows no building swept
  the waypoints seen longest ago, nearest to itself first, on maps up to 128 cells of span, and
  near the enemy starts first only on larger ones. A wave formed at its own base therefore
  searched its own half of the map first. With the slower units of R7 that cost 13,000 ticks
  on crossroads (Libya against Iraq, both easy: 39,983 ticks against a limit of 36,000). The
  ordering of the large maps now applies everywhere (`choose_target`); regression test
  `m6_production_matches.rs::the_last_wave_looks_for_the_last_buildings_near_the_enemys_start_first`.
* No tick limit was changed. `m14_duels -- full` with the RA1 data: all 756 matches end with a
  winner; the validation log of RA1-B has the slowest per map.

### R15. RA1: the legacy ruleset fixture and every test that was migrated or adapted

**The fixture.** `crates/sim/tests/fixtures/data/legacy/` holds the six data files shipped before
RA1 (commit `13ebadc`), migrated only as the schema changes require (`arsenal.ron` schema 2 with
`art` = own id, no side, no factory group, no owner or country lists, tech level 1; `skirmish.ron`
schema 3 with `sides: []` and per faction a label and `side: "none"`; `ai.ron` schema 2 with one
plan of side `"none"`). It has no `rules.ron`, so it behaves exactly as the game did before RA1.
A second fixture, `fixtures/data/ra1/`, is a small world with two sides and three countries for
the balance independent tests of RA1 (I10).

**Moved to the legacy fixture, assertions unchanged** (they assert the unified M9 arsenal, the
Vanguard and Meridian armies, or the roster of a faction):

| Test | Note |
|---|---|
| `crates/sim/tests/m9_arsenal.rs` (all 24 tests) | loads `fixtures/data/legacy` instead of `assets/data` |
| `crates/sim/tests/m11_repair.rs` (all tests) | the same; repair keeps the pre RA1 constants |
| `crates/game/tests/m10_unlocks_and_enemies.rs` (all 5 tests) | the same, on the shipped maps |
| game `skirmish::tests::loading_rejects_a_start_position_outside_its_map` | scratch asset root with the legacy data |
| game `sidebar::tests::items_outside_the_roster_of_the_players_faction_are_neither_available_nor_shown` | a roster only limits a faction without a side |
| game `skirmish::tests::the_catalogue_rejects_starts_off_their_map_unbuildable_ai_files_and_unplayable_matches` | the parts that edit a faction's starting buildings run on the legacy catalogue; the start checks run on both; two cases on the shipped data were added (an AI file without the Soviet plan, a country with an extra yard off the map) |
| game `smoke::tests::the_demo_player_can_queue_a_building_with_its_starting_buildings` | the original loop also runs on the two legacy factions; on the shipped data see below |

**Schema migration only** (the same assertions on the migrated fixture files; error field paths
of the AI file are now `plans[0]....`):

| Test | What changed |
|---|---|
| `m6_definitions.rs` | AI field paths and the texts its edits match; the schema version cases of the skirmish file are 4, 2 and 1 against 3 (one case more); `ai.plans[0]` |
| `m6_match.rs::a_computer_player_whose_faction_lacks_a_producer_of_its_build_order_is_a_setup_error` | field `plans[0].build_order[0]` in the error and its message |
| `m7_factions.rs` | AI field paths; the anchor text of its roster edits; `plan_for` returns an `Option` |
| `m6_ai.rs` | `ai.plans[0]`; the shipped skirmish file is loaded against the whole shipped ruleset |
| `m12_match_options.rs` | `allied()` is `free_for_all: false` and every use of the default options is `free()` (`free_for_all: true`), because the default changed sense (R13); what each test asserts is unchanged |
| game `sim_host::tests::commands_pending_when_a_match_is_replaced_are_discarded_with_a_log_line` | the plan literal gained `mcv_start` |
| game `construction` test of the command variants | 15 variants: `Undeploy` is mapped by `special_controls::undeploy_command` |

**Adapted to the countries** (tests of the shipped data as such; RA2 behaviour is intended):

| Test | What changed and why |
|---|---|
| `m6_match.rs::computer_armies_grow_with_the_map_on_large_maps_only` | loads the shipped ruleset (`sim::load_ruleset`); assertions unchanged |
| `m6_ai.rs::ai_against_ai_matches_on_every_shipped_map_end_with_a_winner` | the same loading; the matches still use fixture data |
| `crates/game/tests/m6_production_matches.rs` (both tests) | pairings of the two sides with rotating countries (R5), 756 matches, each player with its MCV; asserts that every country is played in both slots; the crossroads regression plays the first Soviet country against itself (before: the Meridian) |
| `crates/game/tests/m7_maps.rs::the_shipped_skirmish_file_lists_every_generated_map_with_its_starts` | loads the shipped ruleset |
| `crates/game/tests/m13_grown_maps.rs::every_shipped_map_grows_with_every_player_count` | sides alternate with rotating countries, for either order of the sides; "every yard at its start" is now: every base unit stands on its start, and every computer player's yard stands there once it has deployed (within 100 ticks); the expected units are the starting units of plan section 4.9 |
| `m13_grown_maps.rs::ten_computer_players_build_bases_and_bring_in_ore_on_enlarged_maps`, `a_ten_player_match_on_the_smallest_map_is_played_to_its_end` | a player must have placed 3 buildings instead of 2: the deployed yard is a placed building now, and the power plant and the refinery are still required |
| game `hd_art::tests::every_roster_identity_has_distinct_valid_hd_art`, `every_foot_soldier_has_a_walk_cycle` | art is looked up through the arsenal's `art`; a type that names another's art must name a type with art of its own, every other building still has distinct artwork (R11) |
| game `buildings::tests::the_production_buildings_load_and_every_sprite_passes_its_check`, `the_demo_map_holds_the_starting_buildings_and_units` | a country's starting building is the yard of its side; expected counts from `skirmish::starting_totals` |
| game `skirmish::tests::every_production_data_file_loads_passes_cross_validation_and_is_licensed` | nine countries with pairwise different trees instead of two factions with different rosters; `data/rules.ron` is among the loaded files; the two `rocket_tower` sprites are named only by the legacy fixture now, and the test pins exactly those two |
| game `skirmish::tests::every_shipped_match_can_be_set_up_for_every_faction_pairing` | every pairing of the nine countries (5103 setups); expected units from the MCV start |
| game `skirmish::tests::the_menu_chooses_several_independent_enemies_with_distinct_colours` | 0 starting buildings with the MCV start; a yard each with `mcv_start: false` |
| game `sim_host::tests::match_setup_installs_the_loaded_match_or_ends_the_game_with_an_error` | the menu's match has no starting building; the "starting building sticks out" case uses the yard start |
| game `sim_host::tests::a_tick_applies_the_sinks_commands_as_the_human_players_and_returns_its_events`, `minimap::tests::the_demo_match_shows_no_enemy_the_human_player_does_not_see` | unchanged assertions on `demo_host()`, which is now the demo match with the yards (`mcv_start: false`); `menu_host()` is the match as the menu starts it |
| game `sidebar::tests::item_buttons_queue_cancel_and_start_placement`, `item_status_follows_the_players_buildings_and_queue`, `texts_and_colours_of_the_sidebar`, `cards_follow_the_tech_tree_and_unlock_all_waives_only_prerequisites` | legacy ids replaced by RA2 ids in the same role (`light_tank` by `grizzly` or `prism_tank`, the turrets by `pillbox` and `sentry_gun`, the Soviet examples by their Allied counterparts for the American player); costs, build times and power read from the data instead of the legacy numbers; "the Radar Tower is no longer on the last page" is asserted against the first index of the last page (22 buildings on 4 pages) |
| game `units::tests::the_production_units_load_and_every_sprite_passes_its_check`, `the_production_data_loads_with_the_combat_file`, `the_demo_map_holds_every_starting_unit_on_its_own_free_cell` | the base unit and starting unit types per country; `pillbox` as the armed building and an armed building in every side's plan; starting units by plan section 4.9, on both starts |
| game `smoke::tests::the_demo_match_expects_all_of_its_starting_units`, `a_run_passes_only_with_its_buildings_and_charged_production`, and the shipped half of `the_demo_player_can_queue_a_building_with_its_starting_buildings` | the MCV start (22 units, no building, one human base unit); "every prerequisite is owned" instead of "no prerequisite", since every RA2 building lists its yard; the failure text names the deploy |
| game `visual_review::tests` (4 tests) | scenes start with the yards and RA2 ids; the developed base still has 8 buildings (6 of the scene and the 2 yards); the line-up is 28 types (the ten legacy unit types left it) and gives 48 units and 8 buildings |

No test was deleted, ignored or weakened. New in RA1-B: `ra1_buildability.rs` (11), `ra1_match.rs`
(20), `ra1_definitions.rs` (36), `ra1_shipped.rs` (8), `ra9_data_parity.rs` (4).

### R16. RA1-B: the state of the game until RA1-C

RA1-B changed in the game only what keeps it compiling, starting matches and passing its tests
and the smoke tests:

* It loads `data/rules.ron` before the arsenal and looks art up through `Definitions::art`.
* The menu rows are unchanged; "Starting force" and "Enemy force" now choose among the nine
  countries. The menu opens with America against the first Soviet country, and the computer
  players take the countries in turn from the chosen one (`skirmish::match_players`), so that
  several enemies field several countries. Every match uses `MatchOptions::default()` (10000
  credits, MCV start, unit count 10, short game, superweapons, MCV repacks, computer players
  allied); `MatchPlan::mcv_start` exists for the tests and the review scenes.
* The catalogue check at startup pairs sides with rotating countries (R5).
* The sidebar's availability is `sim::check_buildable` (superweapons counted as on); it still
  shows every item of a tab, also those of the other side, as locked. Filtering is RA1-C's.
* D with no unit selected and the cursor on a building issues `Undeploy`.
* The smoke test deploys the human's base unit after the first tick, moves the other units, and
  queues the first buildable building once the yard stands; it fails if nothing was queued 40
  ticks after the deploy order.
* Maps still grow with the number of players as before; the MAP GROWTH option of R4 is RA1-C's.
