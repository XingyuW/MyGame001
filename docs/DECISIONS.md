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

### R6. RA3-B: invariant I6 becomes I6' (infantry sub cells); crushing

Recorded in RA3-B (2026-10-12) under the number the programme reserved for it. PLAN.md section
14 authorises the amendment, and the prompt's fixed decision 6 asks for this record, updated
tests and an explicit check by the final reviewer.

1. **The amendment.** I6 read: *at most one ground unit occupies a cell, and no unit occupies a
   cell covered by a building.* I6' reads: ***a surface cell holds at most one vehicle or at
   most three infantry and never both; an air cell holds at most one aircraft; no unit occupies
   a cell covered by a building.*** "Holds" is what it was: a unit holds the cell it stands in
   and, while it steps, the cell it enters. "Infantry" is a unit **on foot**: of a type with
   the arsenal's new trait `infantry` that does not fly (`Definitions::on_foot`; the Rocketeer
   is infantry and an aircraft to the grid). "Vehicle" is every other unit of the surface
   (vehicles, ships, and every unit of a type without the trait). Owners do not matter:
   soldiers of two enemies share a cell, as in Red Alert 2. PLAN.md section 6 has a note under
   its table that points here.
2. **A world without the trait is as before, bit for bit.** No unit of a world without an
   arsenal (the fixtures of M2 to M8), of the legacy ruleset, of the RA1 and RA2 fixtures, or
   of the types of RA3-A in the RA3 fixture is on foot, so each holds its cell whole and I6
   holds there as written. Shown in three ways: by the code (a grid without units on foot has
   no table of spots at all, and every rule below reduces to the old one for a unit that needs
   the whole cell); by `ra3_cells.rs::no_unit_of_the_older_fixtures_is_on_foot_or_a_crusher`;
   and by measurement: the state hash without the definitions was folded over every tick of
   every test of the simulation crate before the change (`d43169b`) and after it (item 9).
3. **Spots.** `SPOT_OFFSETS` = (-64, -64), (80, -16), (-16, 80) subcells from the centre of the
   cell (`SubPos::spot_of`). Chosen, not ported (the original's offsets are in no ini;
   **assumption**): on screen spot 0 lies 8 pixels above the centre and spots 1 and 2 lie 4
   pixels below it, 12 to the right and to the left, mirror images of each other; the three
   add up to zero, so a full cell is centred; any two are at least 135 subcells (half a cell)
   apart; none is nearer than 48 subcells to an edge. A soldier's `Position` is its spot, also
   when it is alone in the cell (as in the original). It takes the first free spot when it is
   put into a cell (spawned, trained, unloaded, teleported) and keeps the index of its spot
   from cell to cell while that one is free in the next cell, else takes the first free one:
   a group keeps its formation and walks straight lines.
4. **The grid decides** (`crates/sim/src/map.rs`). `Grid::claim` (with `claim_spot` for the
   preferred spot), `release` and `occupant*` are still the one place. New beside them:
   `register(id, domain, on_foot)` (was `register_domain`) and `unregister`; `holders`,
   `holders_for` (every holder of a cell; `occupant` is the first of them, and `None` still
   means that nothing holds the cell); `spot`, `standing_point`; and one primitive that
   answers "may this unit enter", **`room(cell, on_foot, stays)`**: how much room the cell has
   for a unit of that kind, in spots (a vehicle needs 3, a soldier 1, `room_needed`), if only
   the holders for which `stays` holds remained. `has_room`, `room_for`, `blocked` and
   `blockers` are that primitive with a fixed `stays`. The spots are a table beside the old
   one (12 bytes a cell), allocated when the first unit on foot is registered.
5. **Movement** (`movement.rs`, `unit.rs`). A soldier reserves a spot of the next cell before
   it leaves its own and frees its old spot on arrival. Its position on the way is the
   step's share (`progress / length`, the same 256 or 362 as between centres) of the straight
   line from spot to spot, rounded toward the start (`Mobile::position`): **a step takes
   exactly the ticks it takes a vehicle of that speed.** A soldier waits for a cell that a
   vehicle holds or whose three spots are held; a vehicle for a cell in which any soldier
   holds a spot. The rules that end orders (D21, D22) are unchanged; where they ask whether
   the units in the way are moving on, a soldier before a full cell needs one of the three to
   move on, a vehicle all (`make_room`). The diagonal rule (no step between two cells that one
   unit holds both of) applies to every holder of the two cells. Two soldiers that pass each
   other may be at one point for a tick (their lines between spots cross); two that stand
   never are. (R56, advisory review 1, sim P2-3: a soldier that changes its spot on a
   diagonal step is likewise, for a part of the step, at a point of a third cell that it does
   not hold, since its line runs from spot to spot; an impact finds units through the cells
   they hold and measures to their positions, so with the spreads of the shipped warheads no
   such soldier is missed, and a warhead without a spread hits only what holds the impact
   cell.) A unit's own state has no spot field: the grid's table is the only record, and
   `Position` follows from it.
6. **Callers reviewed** (every use of `occupant`, `claim`, `release` in both crates):
   | Caller | What it asks now |
   |---|---|
   | `movement::at_centre`, `moving_on` | `blockers` of the next cell for this unit; every holder of the two cells beside a diagonal |
   | `pathfinding::find_paths`, `wait_cycle`, `step_aside` | `blocked` by the units that count as obstacles; the first blocker in a wait-for cycle; `has_room` for a step aside |
   | `command::apply_move` (destinations of a group) | `room` beside the idle units outside the group, less what the group was promised: one vehicle or up to three soldiers to a cell, the vehicles of a domain first |
   | `building::exit_cell` (trained and free units, carrier aircraft, the cell a rearmed aircraft flies to) | `has_room` for the kind of unit; three soldiers come out into one cell |
   | `economy::Spawner::leave` (rally point) | `room_for` the new unit |
   | `unit::spawn_unit` | `room_in` the unit's layer; `Occupied` names the first holder |
   | `harvest` (ore cells, docks) | `room_for` the harvester (a vehicle: no holder but itself) |
   | `building::check_footprint`, `check_building_footprint` (placement) | unchanged: `occupant`, any holder blocks |
   | `special`: teleport (Chrono Legionnaire, Chrono Miner, Chronosphere) | `room_for`; the unit stands at `standing_point`; the Chronosphere moves vehicles and still wants an empty cell |
   | `special`: unload, undeploy | `has_room` for each passenger's kind (three soldiers to a ring cell); `register` with the kind |
   | `special`: walking to a transport, a building or a depot, and leaving a depot | `has_room` for the unit's kind |
   | `special`: deploying an MCV, `release` | unchanged check (any holder blocks); `unregister` |
   | `special::blast` (the M9 area damage of worlds without an `effects` entry) | unchanged: it goes by the units' cells, and serves worlds without infantry on foot only |
   | `combat::blast_hits` (splash by cell) | every holder of a cell, each by the distance of its own position |
   | `combat::act_unit` (the cell an attacker moves to) | `room` less what was promised in the tick (`chase_cells` counts spots); a soldier wants the target in range from each of the three spots of the cell, since it cannot know which it will get |
   | `combat::remove_dead` | `unregister` |
   | `ai::deploy_base_unit` (ring of the footprint) | every holder of a ring cell must be an idle unit of the player |
   | `ai::place_ready_building`, `match_setup` (starting units) | unchanged: `occupant`, an empty cell; starting units get a cell each |
   | `examples/m7_soak.rs` | item 8 |
   | game `special_controls` (the unit under a click of the `E` mode) | unchanged: the first holder; its targets are vehicles and buildings |
   | game selection, hover, health bars | by the drawn body, which is at the spot (R43) |
7. **Hashing** (invariant I3). New state: the grid's table of spots and its set of units on
   foot; `MoveOrder::crush` (R42); the four traits in the arsenal's part of the definitions.
   Layout: the flag byte of a cell gains bits 5 to 7 (spot 0 to 2 held) and the ids of the
   holders follow the cell's occupant and building; the ids of the units on foot follow the air
   layer; the flag byte of an order is 2 instead of 1 if it may crush; an arsenal entry ends
   with three more flag bytes and the size. **What changed for existing worlds:** nothing in
   the grid, the units or any other state of a world without units on foot
   (`map::tests::the_hash_covers_the_spots_and_is_unchanged_without_units_on_foot` compares the
   grid's hash with the layout before RA3 written out in the test;
   `unit::tests::the_hash_covers_whether_an_order_may_crush`). The state hash of a world
   **without** an arsenal is therefore the same number as before. The state hash of a world
   **with** an arsenal (legacy, RA1, RA2, RA3 fixtures; shipped data) is another number,
   because its definitions hash four more fields per entry; no test holds an expected hash
   (they compare runs with each other), so no expectation is affected.
8. **The soak's check** (`examples/m7_soak.rs`, amends R8 and D56). Its I6 check assumed one
   holder per cell (`grid.occupant(cell) == Some(unit)`). It now calls
   `sim::cell_invariant_violations` on every tick: from the units' own cells, at most one
   vehicle or three infantry and never both per surface cell, at most one aircraft per air
   cell, nothing of the surface on a building, the grid's record of every holder and spot,
   and every unit's position; and `sim::stray_claims` every 256 ticks and on the last tick: no
   holder in the grid that holds nothing (it reads every cell of the map; a claim that is not
   released stays, so it is found at the next look). Both functions are public in `sim` so
   that the soak and the tests share them, and are no part of a tick.
   `ra3_cells.rs::the_check_of_the_invariant_reports_every_kind_of_violation` feeds them two
   vehicles in a cell, a vehicle with infantry, four infantry, a vehicle and a soldier on a
   building, two aircraft in a cell, a unit off its spot, and two stray claims.
9. **Measured: the older scenarios did not move.** With a scratch patch (kept as
   `docs/validation/ra2-parity/ra3/hash-trace.patch`; never part of a commit of the code) a
   system at the end of every tick folds the state hash without the definitions into a sum
   per test thread, and writes the sums when the thread ends: a line is the thread's test,
   its ticks, the fold in order of the ticks, and the plain sum. `cargo test --release -p sim`
   with it at `d43169b` (`hash-trace-before.tsv` in the same directory) and on the code of
   RA3-B (`hash-trace-after.tsv`): of the 267 test threads of `d43169b` that run ticks,
   **254 have the same fold**. The 13 others: 11 are the tests of the shipped data
   (`ra1_shipped.rs` 5, `ra2_shipped.rs` 2, `ra3_shipped.rs` 4), whose infantry is on foot
   now; 2 are matches of computer players on the RA1 and RA2 fixtures (`ra1_match.rs`,
   `ra2_ai.rs`), whose Soviet miner is armed, changed by the fix of R44 item 2 and by nothing
   else: the same run on the code before that fix (`hash-trace-after-without-r44.tsv`) has
   256 equal folds and only the 11. The ten worker threads of `m6_ai.rs` draw their matches
   in an order that varies from run to run, so their lines differ between any two runs; their
   plain sums add up to the same number in all three files (2,187,194 ticks). So every
   fixture scenario of the 495 tests the simulation had before RA3-B has, tick for tick, the
   state it had, but for the two matches in which the computer player now plays better.
10. **Tests of the amendment** (R45 lists all). `ra3_cells.rs`:
    `invariant_i6_prime_holds_while_mixed_infantry_and_vehicles_cross_for_2000_ticks` (the form
    of M2's test: 48 soldiers and 16 vehicles of two enemies cross a field back and forth,
    `ra3_common::assert_cells` after every tick, which checks I6' from the units' components
    and the grid independently of the simulation's own check, and also runs that);
    `the_crossing_gives_the_same_hash_at_every_tick`; three soldiers in a cell and a fourth
    that waits; a vehicle and a soldier that keep each other out; range and splash from the
    spot; the group move; leaving a barracks and a transport. `ra3_crush.rs`:
    `invariant_i6_prime_holds_while_crushers_and_infantry_of_two_players_cross` and
    determinism with crushing. `m2_movement.rs` (the M2 tests of I6, 50 units for 2,000 ticks
    and the seeded random orders) is unchanged and passes.
11. **For the final reviewer.** This is the one amendment of an invariant in the programme.
    Check: item 2 (no change without the trait), item 6 (no caller left that takes "the
    occupant" for the only holder), item 7 (the hash), and that I6' is asserted on every tick
    in the tests of item 10 and in the soak.

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

### R17. RA1-C: the lobby in the menu

1. **Layout.** The menu is three blocks side by side under the heading lines: the rows of the
   match (map, enemy players, team mode, map growth, game speed, team colour, credits, unit
   count), the rows of its rules (MCV start, short game, superweapons, crates, MCV repacks, then
   the extensions Super Me, unit speed, tech tree) and the PLAYERS panel, with DEPLOY, Options
   and Quit in one row below. Every piece has a fixed size, so the menu covers the same ground in
   every window that holds it: 1247 x 566 logical pixels (`skirmish::menu_size`), within the
   1280 x 720 of the smallest window the brief names (`MENU_FIT`). A unit test adds the measures
   up and checks every text a row can show against the width of its place (the interface font
   is FiraMono, 0.6 of the font size per character); the smoke test lays the fullest menu out
   and measures it (item 4 of R20).
2. **The PLAYERS panel holds the country and difficulty rows.** One line per player: colour,
   name (`You - Blue`, `AI 2 - Gold`), country with `<` `>`, and for a computer player its
   difficulty with `<` `>`. The former rows "Starting force", "Enemy force" and "Difficulty" are
   gone. A country row lists the factions of the skirmish file in its order, each with its side
   (`France (Allied)`), and RANDOM as the last entry. The human player starts on the first
   country (America), every computer player on RANDOM and the middle difficulty (Medium). The
   panel is as high as ten players need whatever their number, so that the buttons under it do
   not move with the enemy players row.
3. **Menu state to match options.** `MenuSelection::players` and `MenuSelection::options` are
   the pure functions that make `PlayerSetup` and `sim::MatchOptions` from the rows; `MatchPlan`
   carries the options whole (`options`, in place of its four option fields). The credits and
   the unit count are `None` until their row is changed, which is the simulation's "the default
   of the data files" and what the row shows; a row wraps around at both ends, so one press of
   `>` from 10000 gives 5000. The lobby's switches start from `sim::MatchOptions::default()`,
   so the game holds no second copy of the lobby defaults. `MenuSelection::initial` is the
   lobby; `MenuSelection::versus` is the same with a fixed country for every computer player
   (America against the Soviet countries in turn), which the review scenes and the tests use
   that must know what a match holds before it is set up.
4. **MAP GROWTH** (R4) is a row of the game, not an option of the simulation: off, `match_map`
   returns the map of the file with its start positions, and the enemy players row ends at the
   start positions the file lists less one (3 to 9 with the shipped maps); switching it off, or
   moving to a map with fewer starts, keeps as many enemies as fit. The catalogue check at
   startup sets every map up for 2 to 10 players with growth on, as before, and once more as
   filed with a player on every start position of the file; it logs how many trial setups each
   country was a player of.
5. **RANDOM is drawn by the simulation** (`MatchOptions::random_countries`, from `SimRng`), so a
   restart draws again. The game reads the countries back after the setup (`MatchPlayers`): a
   notice says which country the human player plays ("You play France (Allied)"); the notices
   and the hover label name another player by colour and country ("Blue Player (France) has
   been defeated"); and a list of the players with country, side, difficulty and "defeated" is
   shown over the map while the match is paused or over. This list is the "PLAYERS panel once
   the match starts" of the brief: the menu's own panel is not on screen during a match.
6. **End screen.** Under VICTORY, DEFEAT or DRAW a line says what decided the match by its
   rule: "Short game: you have no building and no MCV left", "Long game: no enemy has a
   building or a unit left", and so on (`skirmish::end_reason`).
7. **Crates.** The row exists and reaches `MatchRules::crates`; the interface says nothing
   about when crates arrive.

### R18. RA1-C: what the sidebar lists (the brief's rule, made to fit the data)

The brief asks that the sidebar list "only items that some building the player owns could build
under the RA2 rule (factory of the item's group and owner side, country not excluded)", with
items whose prerequisites are missing listed as locked, and that an Allied player see no Soviet
card. With the ported data the first does not give the second: the ini lets every country own
every *building* (`Owner` lists all nine), so the owner side of a factory separates the units of
the sides but not their buildings. What ties a building to a side is a prerequisite: the
Construction Yard of that side, or (the walls) a barracks that only that yard builds.

The rule implemented (`sidebar::item_listed`, `reachable_buildings`):

1. some building the player owns is a factory of the item's kind and of a side in its owner
   list (`sim::produces`); and
2. the player could queue the item if it owned every building it can come to own **by building
   alone** from what it owns (the closure of `sim::check_buildable` over the building types,
   which for a player with its yard is the tech tree of its country, `sim::country_tree`).

So a card is locked when its missing prerequisites are within the player's reach, and absent
when one of them can only be captured. Consequences, each asserted in
`sidebar::tests::a_tab_lists_only_what_a_building_of_the_player_could_build`: no card before the
MCV has deployed; no infantry card before a barracks stands (rule 1; Red Alert 2 shows none
either); an Allied player with its yard sees the Allied buildings and no Soviet one; after
capturing a Soviet Barracks it sees the Conscript, the Tesla Trooper, the engineer and dog, and
the Soviet walls, and not Yuri or the Flak Trooper, which need buildings of the Soviet yard; a
country's special is no card for another country; with superweapons off their four buildings
are no cards; an item in a queue stays listed whatever became of its factory. With ALL UNLOCKED
the simulation waives every prerequisite (R10), so a yard builds the buildings of both sides,
and the sidebar lists what can be queued.

**Difference from Red Alert 2**, kept on purpose ("as today" in the brief): the original shows
no cameo at all for an item whose prerequisites are missing; this game shows it as LOCKED with
the reason when the player can build what is missing.

### R19. RA1-C: repacking the Construction Yard (replaces the last item of R12)

A left click on the player's Construction Yard selects it (`selection::own_yards`; it has no
rally point, so the cursor offers none and a right click orders nothing). While it is selected,
no unit is selected and the match has "MCV repacks" on, `D` issues `Command::Undeploy` for it,
and a button "Repack the Construction Yard (D)" stands over the superweapon buttons at the bottom
left; the hint line says the same. With the option off there is neither button, hint nor command
(`special_controls::repack_command`). The interim control of RA1-B, D with the cursor over a
building and nothing selected, is removed: it sent `Undeploy` for any building under the cursor.
The superweapon buttons moved 18 pixels up, off the hint line they used to cover.

### R20. RA1-C: the smoke flow, the review captures, two labels, the starting credits

1. **Smoke flow.** A long run starts four matches instead of three, each chosen by the menu's
   pure functions as its buttons choose it, and compares every one with its choices after ten
   ticks (`smoke::match_problems`): (a) every lobby row away from its default (a Soviet country,
   5000 credits, unit count 0, long game, free for all, superweapons and crates off, MAP GROWTH
   on with five players on the four starts of Riverlands, fixed countries and other difficulties
   for two computer players, Super Me, ALL UNLOCKED), in which the run deploys the MCV, repacks
   the yard with the game's control and deploys it again, checking the simulation and the cards
   after each step; (b) its restart; (c) a match that starts with the yards, MCV repacks off and
   the standard tech tree; (d) the lobby's defaults with RANDOM computer players, to which the
   checks of M1 to M7 apply. No tick limit of the smoke test was changed.
2. **Unit tests of the flow.**
   `smoke::tests::the_flows_menu_presses_choose_its_matches_and_restore_the_lobby` makes the
   same presses without a window and sets the matches up, so a change of the menu that would
   break the smoke run fails in `cargo test` first.
3. **Captures with the display off.** Captures of the window are black while the session is
   locked, which it was for this work. `MYGAME_MENU_REVIEW=<dir>` draws the interface to an
   image (1280 x 720, or `MYGAME_MENU_REVIEW_SIZE`) and saves that; `docs/screenshots/ra1/` holds
   five such captures. They show what the game draws; whether it looks right **requires user
   verification**.
4. **Layout check in the smoke run.** In the first menu the run shows the fullest menu (ten
   players, the longest map, country and option texts) for four frames and measures the laid-out
   nodes: all 73 buttons, none overlapping, every changing text inside its place, the whole no
   larger than `menu_size()` and 1280 x 720, and inside the window.
5. **Two labels shortened** in the generator's own table (`scripts/generate_ra2_data.py`, not
   ported values): "American Airforce Command" (was "... Headquarters") and "Soviet MCV" (was
   "Soviet Mobile Construction Vehicle (MCV)"), which did not fit a production card.
6. **Starting credits** (debt of R9 item 8). The game's catalogue check now rejects a skirmish
   file whose `starting_credits` the lobby of the rules file does not offer (outside its range
   or off its step), naming file, field, value and range. It is checked there and not in
   `sim::load_skirmish`, whose tests load fixtures with credits of 0 to 1,000,000 beside rules
   files and must keep passing unchanged.

### R21. RA1-C: tests adapted (none deleted, ignored or weakened)

All in `crates/game/src` (the tests of `crates/game/tests/` name no menu row and are unchanged).

| Test | What changed and why |
|---|---|
| `skirmish::tests::menu_bundles_spawn_without_duplicate_components` | 73 buttons (`Row::COUNT * 2 + 3`) instead of 23; also asserts one `<` and one `>` per row |
| `skirmish::tests::the_production_catalogue_loads_every_map_faction_and_difficulty` | the middle difficulty is asserted for every computer player's row |
| `skirmish::tests::every_shipped_match_can_be_set_up_for_every_faction_pairing` | the selection is made with `with_countries` and a difficulty per slot; same 5103 setups and assertions |
| `skirmish::tests::the_menu_chooses_several_independent_enemies_with_distinct_colours` | runs with MAP GROWTH on (off is the default, R4) and fixed countries (`versus`); the player names come from `player_tag` (`player_line` held the country too, which now has its own row); assertions unchanged |
| `skirmish::tests::menu_buttons_cycle_through_each_row_and_start_only_a_valid_choice` | the faction, opponent and difficulty rows are the country and difficulty rows of a player; **expectation changed**: `<` from the first country gives RANDOM (the last entry), then the last country; `>` on a computer player's row goes from RANDOM to the first country; rows that do not exist do nothing |
| `skirmish::tests::the_human_plays_the_first_start_and_the_computer_the_second`, `the_end_screen_names_the_outcome_and_restarts_the_last_match` | the selection literal (`with_countries`, per slot difficulties); same expected players and effects |
| `skirmish::tests::row_texts_name_the_chosen_entries` | the difficulty row of computer player 1; **expectation changed**: the id is shown with a capital ("Medium") |
| `sim_host::tests::demo_host`, `menu_host` and the tests on them; `match_setup_installs_the_loaded_match_or_ends_the_game_with_an_error`, `commands_pending_when_a_match_is_replaced_are_discarded_with_a_log_line` | `MenuSelection::versus` (the menu's own first choice now has a RANDOM opponent) and `MatchPlan::options`; assertions unchanged |
| `sim_host::tests::menu_matches_ally_the_computer_players_and_install_speed_and_scenery` | the selection has MAP GROWTH on, which the enlarged map it asserts needs since R4 |
| `units::tests::the_demo_map_holds_every_starting_unit_on_its_own_free_cell`, `buildings::tests::the_demo_map_holds_the_starting_buildings_and_units` | `versus`, `selection.players()`, `selection_totals`; assertions unchanged |
| `sidebar::tests::items_outside_the_roster_of_the_players_faction_are_neither_available_nor_shown`, `cards_follow_the_tech_tree_and_unlock_all_waives_only_prerequisites` | `item_status` takes whether superweapons are on (true, as before) |
| `smoke::tests::a_run_passes_only_with_the_menu_the_game_flow_and_the_computer_player` | **expectation changed**: a long run starts 4 matches ("2 of 4 matches started") |
| `smoke::tests::the_demo_match_expects_all_of_its_starting_units`, `the_demo_player_can_queue_a_building_with_its_starting_buildings`, `an_exit_before_the_last_frame_fails_the_run` | `versus`, `selection_totals`, `selection.faction()`, `SmokeTest::new`; assertions unchanged |
| `visual_review::tests` (4 tests) | the scene's choices carry the yard start (`scene(choice)`), `selection_totals`; assertions unchanged |
| `hover_info::tests::labels_name_the_type_owner_and_health`, `notices::tests::a_defeat_is_reported_in_the_colour_name_of_the_defeated_player` | `label_text` and `defeat_text` take the name or country of the player (none: the texts asserted before); assertions with countries added |

New in RA1-C (16 tests, all in the game crate): `skirmish` 9
(`the_menu_opens_with_the_lobby_of_red_alert_2`,
`the_credits_and_unit_count_rows_cover_their_ranges_and_reach_the_match`,
`every_on_off_row_sets_its_option_of_the_match_and_no_other`,
`country_and_difficulty_rows_set_each_player_of_the_match`,
`random_countries_are_drawn_by_the_simulation_and_named_once_the_match_starts`,
`map_growth_is_an_option_and_off_limits_the_enemies_to_the_starts_of_the_file`,
`the_menu_and_its_longest_texts_fit_the_smallest_window`,
`the_end_screen_says_what_decided_the_match_by_its_rule`,
`the_catalogue_rejects_starting_credits_the_lobby_does_not_offer`), `sidebar` 1
(`a_tab_lists_only_what_a_building_of_the_player_could_build`), `special_controls` 1
(`a_selected_construction_yard_is_repacked_only_with_the_option_on`), `selection` 1
(`a_click_on_the_construction_yard_selects_it_without_a_rally_point`), `notices` 1
(`notices_name_the_country_a_player_plays`), `smoke` 3
(`a_run_passes_only_with_the_menu_laid_out_the_cards_listed_and_the_flow_checked`,
`the_layout_check_finds_a_menu_that_is_too_large_overlaps_or_cuts_a_text`,
`the_flows_menu_presses_choose_its_matches_and_restore_the_lobby`).

### R22. RA2: build times, the arsenal file of schema version 3, and the audit

1. **Build times** (plan section 4.2). `scripts/generate_ra2_data.py` no longer holds a build
   time per item: it writes `max(1, round(cost x 840 / 1000))` ticks, 840 being
   `[General] BuildSpeed=.7` game minutes per 1000 credits, and for a building with `Wall=yes`
   that number times `WallBuildSpeedCoefficient=3.0`. A Grizzly (700 credits) takes 588 ticks
   where it took 233, a Power Plant 672 where it took 200, a wall 252.
2. **`arsenal.ron`, schema 3.** Nine fields per entry, all required: `build_limit`
   (`BuildLimit`; 0: none), `adjacent` (`Adjacent`, see R25), and as 1 or 0 `base_normal`
   (`BaseNormal`, default yes), `radar` (`Radar`), `sellable` (no `Wall` and no `Unsellable`),
   `repairable` (`Repairable`, default yes), `powered` (`Powered`), `is_powered` (the
   `IsPowered` of the section its `SuperWeapon` names) and `unit_repair` (`UnitRepair`).
   Beyond the plan's list: `repairable` and `unit_repair`, which items 8 and 9 need, since the
   plan's "walls are not repairable" and "the Service Depot repairs" are properties of types
   in the ini too. They are traits of buildings: on a unit each must be 0, and the loader
   rejects anything else ("a building trait on a unit type"). Ranges: `build_limit` 0 to 255,
   `adjacent` 0 to 32 (`MAX_PLACEMENT_DISTANCE`), flags 0 or 1. All are hashed with the
   definitions.
3. **The traits are read only with a rules file.** A world without `rules.ron` (the fixtures of
   M2 to M9, the legacy ruleset) keeps every rule it had: one queue of 16 items per producer,
   one unit of progress per tick and the even tick rule under low power, `placement_distance`
   from any own building, `sell_value`, the `REPAIR_*` constants, every building stopped by low
   power, the free repair around a Service Depot, and a minimap that is always on. This is the
   arrangement of plan section 3.2, and it is why `m9_arsenal.rs`, `m11_repair.rs` and the
   fixture tests of M3 to M7 pass unchanged.
4. **The lane of a building is its category.** `BuildCat=Combat` is the arsenal's category
   `armory`, which was already the Armory tab; the generator fails if the two disagree, and the
   audit compares them (`lane`). No field was added for it.
5. **The build limit of a unit is not ported.** The reference has one, `[YURIPR] BuildLimit=1`
   (Yuri Prime, which cannot be built before RA6). `check_buildable` counts buildings from what
   it is given; counting a player's units, also those in transports, belongs to the milestone
   that makes the unit producible. The data has 0, and `ra2_differences.tsv` lists it.
6. **The audit** (`ra9_data_parity.rs`) now also recomputes, for every mapped item, the build
   time from its cost, and compares `build_limit`, and for every building `lane`, `adjacent`
   and the seven flags; it asserts that a unit has none of them. Differences added: Yuri
   Prime's build limit, and the `adjacent` of the two shipyards (R25). 21 rows in all.
7. **Fixtures.** `legacy/arsenal.ron` and `ra1/arsenal.ron` are migrated to schema 3 (the
   legacy one with neutral values that nothing reads; the RA1 one with `adjacent: 5`, which
   gives the reach of its `placement_distance` of 6, its radars as radar buildings and its
   turret and storm device powered). A third fixture, `fixtures/data/ra2/`, is the world of the
   RA1 fixture with a bunker, a sentry, a wall, a depot and a vault, and rules whose values
   differ from the shipped ones (a unit queue of 1 + 3, a refund of 40 percent, repair steps of
   10 hit points every 5 ticks, 4 on a depot, at 20 percent), for the tests of RA2 (I10).
8. **Debt.** `arsenal.ron` is 60,159 of the 65,536 bytes a data file may have
   (`MAX_DATA_FILE_BYTES`). The next milestone that adds a trait to every entry must raise
   the limit or split the file, with a decision record.

### R23. RA2: production by rate, the lanes of the Construction Yard, queue lengths, build limits

1. **Rate.** With a rules file the head of a queue advances by `rate_pm` permille of a unit of
   progress per tick (`sim::production_rate_pm`), accumulated in `QueueItem::fraction` (0 to
   999, hashed): `1000 x 1000^(n-1) div multiple_factory_pm^(n-1)` for `n` working factories of
   the item's group (1000, 1250, 1562, 1953, ...; the plan's `1000 x 5^(n-1) div 4^(n-1)`),
   times the low power factor in permille div 1000, times 10 with Super Me. Every whole unit
   is charged by equation 3 as before (`cost div T`, and the remainder with the last), so I7
   holds for every rate; `advance_item` does this in closed form, and a unit test compares it
   with the unit by unit definition over 4000 generated cases. A unit the player cannot pay
   ends the tick's progress; the fraction below one unit is kept, whole units that were not
   made are not carried over (a player who gets credits again does not catch up at once).
2. **Which factories count** (single source, YRPP's per category counters): the player's
   buildings of the item's factory group that a spy has not switched off, of either side, at
   most `MAX_COUNTED_FACTORIES` = 10. **Assumption** (the plan's): the ini knows no cap; ten
   bounds the rate at 7.45 units per tick. The rate is also capped at
   `MAX_PRODUCTION_RATE_PM` whatever a rules file holds (I12).
3. **Low power** (plan section 4.2; **assumption**: the shortage is `1 - produced / consumed`):
   `clamp(1000 - penalty_pm x (1000 - produced x 1000 div consumed) div 1000, min, max)`: 800
   for any shortage up to a fifth, 500 for half or more, with the values of the ini. Nothing
   else of production waits for power.
4. **Lanes.** A queue is kept under `QueueKey { producer, lane }`: the producer as before
   (`Definitions::queue_key`), and `Lane::Defences` for a building of the category `armory`
   with a rules file, `Lane::Main` for everything else. `PlayerState::queue(producer)` is the
   main lane, `PlayerState::lane(key)` any; `Definitions::queue_of(item)` names the key of an
   item. The two lanes of the yards progress in the same tick.
5. **Queue lengths** (`Definitions::queue_capacity`). With a rules file a lane of buildings
   holds one item (single source, ModEnc: Red Alert 2 does not queue buildings), a queue of
   units `1 + max_queued_objects` = 30. A complete building holds its lane until it is placed.
   `QueueFull` reports that capacity. Without a rules file: `MAX_QUEUE_LENGTH` = 16.
6. **Build limits.** `check_buildable` ends with a new check, after the country: with a rules
   file and a `build_limit` above 0, the buildings of the type the player owns and those in
   its queues together are fewer than the limit, else `BuildLimitReached { item, limit }`.
   It is the last check on purpose: what the sidebar lists is "what could be queued but for
   the limit" (R30). "Unlock all" does not waive it.
7. **Difference from Red Alert 2**, as the plan lists it: the original charges in 54 steps,
   the game per unit of progress, with the same total.

### R24. RA2: what low power stops, superweapon charging, radar

1. **One criterion** (`PlayerState::powers(definitions, kind)`), replacing "the owner is not
   low on power" at every place the M9 code asked it for a building: with a rules file a
   building works unless its owner is low on power **and** the type is `powered` **and**
   consumes power (single source, ModEnc `Powered`). The mechanisms are unchanged: the fire of
   armed buildings (`combat.rs`), the Cloning Vats, the Ore Purifier, the Spy Satellite, the Gap
   Generator, the Psychic Sensor (`special.rs`). So a Pillbox (not `Powered`) and a Sentry Gun
   (`Powered`, `Power=0`) keep firing, a Prism Tower does not; the Cloning Vats (not
   `Powered`) keep cloning; the Spy Satellite (`Powered=true`, `Power=-100`) stops revealing,
   although the plan's appendix D notes one source that says it needs no power: the rule of
   section 4.2 was followed. A unit's ability (the repair of an IFV with an Engineer) does not
   depend on the base's power with a rules file.
2. **Superweapons.** The charge advances unless the owner is low on power and the type
   `is_powered` (confirmed, the ini's `IsPowered`); all four shipped superweapons are.
   **Assumption**: a charged superweapon that `is_powered` cannot be fired under low power
   either (the M9 rule, kept; no source says what the engine does with a ready weapon).
3. **Radar** (`PlayerState::radar`, hashed, set by `update_power` in the `Economy` stage): with
   a rules file a player has radar while it owns a `radar` building that a spy has not switched
   off, is not low on power, and suffers no power outage from a spy in a power plant; a world
   without a rules file always has it. Following the plan, any low power takes the radar away,
   also from an Airforce Command, which is not `Powered` in the ini (single source, ModEnc
   `Radar`). The spy's outage itself is unchanged (600 ticks, the M9 value; RA6 ports
   `SpyPowerBlackout`).
4. The computer player does not read the radar.

### R25. RA2: placement by `adjacent` and `base_normal`; the reach of a shipyard

1. **The rule** (`sim::placement_reach`, `sim::extends_base`, used by `check_placement`, the
   interface's preview and the computer player). With a rules file a new building must lie
   within its own `adjacent` free cells of a building of its owner whose type is
   `base_normal`. ModEnc defines `Adjacent=0` as touching and `Adjacent=1` as "at most a 1
   cell gap", so the reach in the game's measure (`Footprint::distance`: 1 for touching) is
   `adjacent + 1`: 3 for most buildings, 5 for Pillbox, Sentry Gun and Patriot, 9 for walls.
   `adjacent` is stored in cells, as the ini writes it, not in subcells as the conversion table
   of the plan's section 2 says for distances: footprints are measured in cells.
2. **Assumption**: the rule applies to every player. ModEnc says the engine places the
   buildings of its own AI by `AIBaseSpacing` instead; the game's computer player issues
   `PlaceBuilding` like anyone (I5), and its `find_placement` looks for sites within the reach
   of each building from its base buildings.
3. `check_placement` now takes the types with the footprints of the player's buildings
   (`(BuildingKind, Footprint)`), since it must know which are base buildings.
4. **The shipyard keeps its reach of 32, as a listed difference.** Measured by
   `ra2_shipped.rs::shipyard_water_lies_farther_from_the_starts_than_the_shipyards_adjacent_distance`
   over the 176 start positions of the 21 shipped maps: the distance from the Construction
   Yard at the start to the nearest place for a shipyard (open water of 3 x 3 cells with shore
   beside it) is at most 13 (`Adjacent=12`) at 28 starts, and at the first start, the human
   player's, on 2 maps (Frostline 7, Pacific Atoll 8); it is at most 32 at 132 starts, and at
   the first start on 18 maps (not on Oasis Gulf 48, Red Canyon 41, Steel Harbor 54, where a
   player builds towards the water). No map has every start within 13, and 13 maps have none.
   The maps are not reshaped (plan item 7), so the two shipyards have `adjacent: 31` in
   `arsenal.ron` where the reference has 12, listed in `ra2_differences.tsv`; the special case
   in code (`SHIPYARD_PLACEMENT_DISTANCE`) remains only for a world without a rules file. A
   shipyard is no base building (`BaseNormal=no`), as in the ini.

### R26. RA2: selling and repairing

1. **Sale.** With a rules file the refund is `cost x refund_percent div 100`
   (`Definitions::refund`), which the economy file's `sell_value` already equalled for the
   shipped data (the audit compares both with the reference); without one, `sell_value`.
   **Assumption** (appendix D): no scaling with the building's health. A type that is not
   `sellable` is rejected with the new `NotSellable`: the two walls. The ini makes only the
   tech buildings `Unsellable`; that walls cannot be sold is the prompt's and the plan's
   requirement, expressed in the data as "no `Wall`".
2. **Repair** (section 4.6). With a rules file `RepairBuilding` starts only on a `repairable`
   type (else the new `NotRepairable`: the walls, `Repairable=false`); every `repair_interval`
   (19) ticks the building regains `repair_step` (8) hit points, at most what it lacks, for
   `max(1, (cost div max(strength div repair_step, 1)) x repair_percent div 100)` credits
   (`sim::repair_step_cost`; 3 credits for the Construction Yard, the plan's example). The last
   step costs a whole step whatever it restores. A step the player cannot pay is skipped and
   the repair waits (I7). Super Me pays at most 1 credit a step. The step ticks are the ticks
   divisible by the interval, for all buildings alike.

### R27. RA2: the Service Depot

1. **The order** is the existing `Command::Interact { unit, target }` (the plan's item 9): for
   a unit that is neither an Engineer nor a Spy, and a target that is a building with
   `unit_repair`, it means "go there and be repaired". Checked in this order: the building is
   the player's own ("Choose your own repair building"), the unit is a vehicle ("Only vehicles
   are repaired here"), of the building's element (a ship at a shipyard, a ground vehicle
   elsewhere, an amphibious one at either, no aircraft: "This unit is not repaired here"), and
   damaged ("This unit is not damaged"); all as `AbilityUnavailable`, with no state change.
   Another player's unit is `NotOwned` if the player sees it, and `UnknownUnit` if it does
   not or the unit is hidden from it, as for every unit command (D30, D33). (Corrected by
   R56: this sentence said `NotOwned` for every unit of another player, and the code did
   that; advisory review 1, sim P1-1.)
2. **Docking.** The vehicle drives to a free cell beside the building (the approach of
   transports and engineers) and is then recorded in `SpecialState::docked` (unit to
   building, hashed). A building covers its cells, so "onto the depot" is "beside it"; any
   number of vehicles may be docked at once (**difference**: the original repairs one at a
   time on the pad). A docked harvester stops its cycle.
3. **Repair** (`special::repair_docked`, in the `Economy` stage): every
   `unit_repair_interval` (19) ticks a docked vehicle regains `repair_step` hit points for the
   step cost of its own price and hit points (2 credits per 8 hit points for a Grizzly). A
   step the owner cannot pay is skipped; no step while a spy has switched the building off.
   When it is whole it leaves the dock and moves to the nearest free cell two cells from the
   building, to make room. The repair also ends when the vehicle gets any order, is no longer
   beside the building, or either is lost or changes owner.
4. **The free repair around a Service Depot of M9 is removed** with a rules file. The repair
   by an IFV that carries an Engineer stays (RA6 owns it). `UnitRepair=yes` on the two
   shipyards gives ships the same paid repair, which the plan does not name; it follows from
   the trait.
5. Not done here: a docked vehicle cannot be sold from the depot, and a Terror Drone is not
   removed (appendix D lists both; RA6). The computer player does not use the depot (plan).

### R28. RA2: the computer player

1. **Lanes.** `queue_buildings` (was `queue_building`) looks, per lane, for the first building
   of the build order that the player owns less often than the order lists it, and queues it
   if its lane is empty; so a defence is built beside the next building. The Allied plan's
   Pillboxes and the Soviet plan's Sentry Guns are built that way; the build orders in
   `ai.ron` are unchanged.
2. **Power.** It still builds a power building first when the next building would leave it
   short, now counting what the buildings in its queues will produce and consume; a lane
   whose building must wait for power queues nothing.
3. **Placement.** `find_placement` searches within `placement_reach` of the building from the
   player's base buildings (`extends_base`), and `place_ready_building` tries every ready
   building, not only the first, so that a defence without a site does not hold the other
   lane back. `sell_for_army` does not try to sell what cannot be sold.
4. **Queues.** It asks `Definitions::queue_capacity` instead of `MAX_QUEUE_LENGTH`.
5. Tested on the RA2 fixture (`ra2_ai.rs`: all pairings end with a winner, no rejected command
   that the fighting does not explain, in particular no `QueueFull` and no
   `BuildLimitReached`; both lanes used; every placed building within its reach) and with the
   shipped data (`ra2_shipped.rs`, and the 756 matches).

### R29. RA2: the tick limit of the shipped matches is rescaled by the pacing factor 2

`crates/game/tests/m6_production_matches.rs` requires each of its 756 matches of computer
players with the shipped data to end with a winner within `MATCH_TICK_LIMIT x span / 128`
ticks (`span`: the map's width plus height, at least 128). `MATCH_TICK_LIMIT` is now 72,000
(was 36,000). No other limit was changed: `m6_ai.rs`, `m7_factions.rs`, `m10`, `ra1_match.rs`
play fixture data, whose build times did not change; `m13_grown_maps.rs` (72,000 for ten
players) and the soak (36,000 ticks per configuration, with no requirement to end) pass as
they are; `m14_duels` gives every match 300,000 ticks, as before.

**Derivation.** RA2 gives every item the build time of Red Alert 2, `cost x 21 / 25` ticks
(R22). Before, a vehicle took `cost / 3` ticks (Grizzly 233 for 700 credits, Rhino 300 for 900,
the miners 466 for 1400), so vehicles are built 0.84 x 3 = 2.52 times as slowly. The armies of
both plans are mostly vehicles (the Allied cycle of six units costs 3100 credits, 2700 of them
vehicles), and both plans build two War Factories, which the multiple factory rule of RA2
speeds up by 1 / 0.8. The pace at which a computer player's army is built is therefore
2.52 x 0.8 = 2.016 times slower than before RA2, and the limit, which bounds matches whose
length is governed by that pace, is rescaled by 2. (Infantry: a GI took 80 ticks and takes
168, with two Barracks 134, a factor of 1.68; buildings 1.75 to 4.2, once per match.)

**Measured** (`m14_duels -- full`, the 756 pairings of the test, played with another seed
(14, the test's is 0) and so along other courses; ticks; "old limit" is 36,000 x span / 128.
Corrected by R56, which has the lengths of the test's own matches: this sentence said "the
same 756 matches as the test"). Before: `docs/validation/ra2-parity/ra1/10-duels.log`. After:
`docs/validation/ra2-parity/ra2/10-duels.log` (the run is deterministic: a run before the
validation gave the same ticks).

| | Before RA2 | After RA2 |
|---|---|---|
| Length over the old limit: median, 90th percentile, longest | 0.15, 0.34, 0.86 | 0.22, 0.49, 1.49 |
| Matches over the old limit | 0 | 5 |
| Small maps (span up to 128): median of each map | 9,076 to 10,797 | 12,814 to 16,066 |
| Small maps: longest | 30,979 (demo) | 53,699 (crossroads, Korea against France, both easy) |
| Large maps: median of each map | 11,419 to 29,058 | 12,920 to 46,123 |
| Large maps: longest | 103,519 (Liberty Bay) | 161,774 (Oasis Gulf, Germany against Great Britain, both brutal) |
| Length over the new limit: median, 90th percentile, longest | | 0.11, 0.25, 0.75 |

The five matches over the old limit: three on Crossroads (53,699, 49,468 and 41,022, limit
36,000), one on Riverlands (38,262) and the one on Oasis Gulf (limit 126,000). All 756 ended
with a winner. Matches are 1.4 to 1.5 times as long at the median and at the 90th percentile,
less than the factor of the limit, because moving and fighting did not slow down.

**Not a stall.** `m14_duels -- full trace oasis_gulf` shows the longest match as a war of
attrition between two equal Allied players, not a standstill: from tick 9,000 to its end both
keep 10 to 44 units in the field, the armies meet between the bases (unit centres around
x = 60 to 100 and x = 160 to 210 of 256), and the loser's buildings fall from 13 to 8 over
100,000 ticks. Its length is debt for RA8, which gives the computer player the rest of the
arsenal; the limit was not raised to cover it alone (it would fit a factor of 1.3).

### R30. RA2: the interface

1. **Two lanes in the sidebar.** A card shows the progress of its own item, as before; the
   item's queue is now that of its lane (`ItemStatus` reads `Definitions::queue_of`). Each tab
   button carries a thin bar with the progress of its lane (`sidebar::tab_progress`: the item
   of that tab that is in production, the most advanced one for the Vehicles tab, whose
   ships and aircraft have queues of their own), so the lane of the tab that is not open is
   seen too. This is the plan's "armoury progress beside building progress".
2. **A taken lane.** `ItemStatus::full`: the item's queue holds what it can. Its other cards
   are drawn recessed and say `busy`, the hint says "Busy: the yard builds one of these at a
   time", and a left click orders nothing (before, the simulation would have rejected it).
3. **Build limit.** `ItemStatus::limited`: the card says `LIMIT`, the hint "Build limit
   reached: 1 per player". The card stays listed: `item_listed` and `reachable_buildings`
   treat `BuildLimitReached` like "can be queued", since the limit says nothing about what
   the player's buildings can build. (The sidebar knows which building types the player owns,
   not how many of each, which is exact for the limits of one that the shipped data has; the
   command itself counts the buildings.)
4. **Two ready buildings.** `construction::ready_in_mode`: placement mode keeps the building
   it is placing while that one is ready, whatever else is ready; `B` and the mode's start
   still take the first ready building. Before, the mode ended whenever the first ready
   building was another one.
5. **Hints.** The time of an item is its build time over the production rate
   (`sidebar::build_ticks`, with the player's factories and power); under low power the hint
   says "Low power: production at 80%" (the factor of R23). `availability_text` takes the
   number of factories.
6. **Radar** (`minimap.rs`). While the human player has no radar the minimap image is the
   map's diamond in one dark colour, the sidebar shows "RADAR OFFLINE" with "no radar
   building" or "low power" over it (`SidebarText::Radar`), the camera's rectangle is hidden
   and a click on the minimap moves nothing. `MinimapState::offline` says which was drawn;
   the smoke test fails if the drawing of a tick and the player's radar disagree. A default
   match starts without radar, so the minimap is blank until the player builds an Airforce
   Command, a Radar Tower or a Spy Satellite: Red Alert 2's behaviour, and a visible change
   for the user.
7. **Depot.** No new control: `E` and a click on the Service Depot sends the selected
   vehicles (`special_controls`, as for transports); the mode's hint names it. A rejected
   order shows its reason in the action hint, as every rejected command does.
8. The superweapon buttons say `LOW POWER` only for a superweapon that `is_powered`, and the
   glow of a working building follows `PlayerState::powers`.
9. Nothing was drawn or recorded for RA2: no asset, no sound, no entry in
   `assets/LICENSES.md`.

### R31. RA2: tests adapted (none deleted or ignored; two checks lost their force, see the note)

**Note (R56, advisory review 1, tests P1-1, P1-3 and P1-4).** The limit raised for the four
tests of `m6_production_matches.rs` (the last row of the third table) left the RA1 regression
`the_last_wave_looks_for_the_last_buildings_near_the_enemys_start_first` without its failing
condition: without its fix the match takes 53,431 ticks, inside the new limit. And since a
default match has no radar, the smoke test's "minimap pixels lit" is satisfied by the offline
screen. Both are made good in R56 (a unit test of the sweep order; the smoke test and a game
test look at the drawn image), and this title no longer says "none weakened".

**Compile migrations** (the assertions are unchanged):

| Test | What changed |
|---|---|
| `m3_economy.rs::a_completed_item_is_charged_exactly_its_cost` | the two `QueueItem` literals gained `fraction: 0` (the new field; 0 in a world without rules) |
| `m5_placement_preview.rs` (helper `preview`), `m9_arsenal.rs` (catalogue helper), `ra1_shipped.rs` and `ra1_buildability.rs` (helper `build`), game `tests/m10_unlocks_and_enemies.rs` (helper `site`, new helper `anchors`) | `check_placement` takes `(BuildingKind, Footprint)` pairs (R25 item 3) |
| game `sidebar::tests::cards_follow_the_tech_tree_and_unlock_all_waives_only_prerequisites`, `a_tab_lists_only_what_a_building_of_the_player_could_build` | `availability_text` takes the number of factories (1); an `ItemStatus` literal gained `limited` and `full` |

**Schema migration** (`ra1_definitions.rs`, the arsenal at schema 3): the text edits that
matched `(schema_version: 2, entries: [` match `3`; `other_schema_versions_are_rejected_in_every_file`
rejects 2, 1 and 4 against 3 (one case more); `every_field_of_the_arsenal_file_is_required`
also removes each of the nine new fields in turn (nine cases more).

**Expectation changed because RA2 behaviour is intended:**

| Test | What changed and why |
|---|---|
| `ra1_buildability.rs::a_superweapon_building_is_rejected_while_the_option_is_off` | its helper `queued` reads both lanes of the yard: the storm device is a defence and waits in the defence lane (R23 item 4). The assertion (`[device]`) is unchanged |
| game `sidebar::tests::item_status_follows_the_players_buildings_and_queue` | two Power Plants are ordered as before; the lane of the buildings holds one, so one is queued and the badge at half time says `50%` (before: two queued, `50% +1`). Added: the lane is `full`, another card of the lane says `busy` once it is available, a click on it orders nothing, and the tab bars show 50 percent for Buildings and nothing for Armory and Vehicles |
| game `tests/m6_production_matches.rs` (all four tests) | `MATCH_TICK_LIMIT` 72,000 instead of 36,000 (R29) |

`m9_arsenal.rs` (24 tests) and `m11_repair.rs` (4 tests) assert the unified arsenal of M9 on
the legacy fixture, which has no rules file and so keeps the M9 rules (R22 item 3): apart from
the helper named above they are unchanged, and none of their expectations changed.

New in RA2: `ra2_definitions.rs` (7), `ra2_production.rs` (15), `ra2_power.rs` (6),
`ra2_placement.rs` (6), `ra2_repair.rs` (12), `ra2_ai.rs` (3), `ra2_shipped.rs` (8), with the
shared helpers in `tests/ra2_common/mod.rs`; 2 unit tests in `sim::economy`; in the game
`minimap` 2, `sidebar` 2, `construction` 1, and one more case in
`smoke::tests::a_run_passes_only_with_the_full_interface_loaded`.

### R32. RA3-A: the combat file of schema version 2, its limits, and the size of a data file

1. **`combat.ron`, schema 2** (`crates/sim/src/combat_defs.rs`; plan item 1). Every field is
   required. A warhead gains `cell_spread` (subcells, 0 to 2816: the 11 cells the original
   engine supports), `percent_at_max` (0 to 100) and the flags `wall` and `wood`. A weapon
   gains `min_range` (below its range), `burst` (1 to 8), `anti_air` and `anti_ground` (at
   least one), and `scatter` (0 to 2048 subcells: the largest distance between target and
   impact at the weapon's full range; 0 for an accurate weapon). A type lists up to two
   `weapons` (primary, secondary) and the `elite_weapons` that take their places at elite
   rank (none for an unarmed type, at least one for an armed one), and has `air_range_bonus`
   (0 to 2048 subcells), `land_targeting` (0 to 2) and `naval_targeting` (0 to 6, the ini's
   codes, R34); a unit type also `underwater` and `organic`, a building type `wall`. A new
   list `effects` names base damage and warhead of the area effects that are no weapon of a
   type (`nuke`, `lightning`, `ivan_bomb`, `c4`, `radiation`; each at most once, R35).
2. **Beyond the plan's list**, each because a rule of the milestone needs the data:
   `land_targeting`, `naval_targeting`, `underwater` and `organic` (the brief's item 4: which
   weapons reach submerged units is a property of the attacker's type in the ini, R34);
   `air_range_bonus` (plan section 4.4); `wall` on a building (a warhead's `wall` flag means
   nothing without it); `effects` (the plan's item 2 routes the area effects through
   warheads, which the data must name). **Instead of the plan's "projectile flight kind"**
   the file keeps `projectile_speed` (0: the shot hits at once) and has `scatter`: no rule of
   the simulation needs more of the flight than its time and whether it is aimed, and the
   interface draws a shot by its weapon id.
3. **Neutral values** (the fixtures of M2 to M9, the legacy ruleset, the RA1 and RA2
   fixtures): no spread and 100 percent at the edge, `wall` and `wood` 1, no minimum range,
   burst 1, both layers, no scatter, elite weapons equal to the weapons, no bonus,
   `land_targeting` 0, `naval_targeting` 5 (every target on water like one on land), nothing
   underwater, organic or a wall, and no effects. With them every rule of M4 and M9 is what
   it was, and every fixture test keeps its numbers.
4. **Limits changed** (they bound what the simulation derives, invariant I12, and are tested as
   bounds): `MAX_WARHEADS` 64 (was 32) and `MAX_WEAPONS` 128 (was 64) for the 45 warheads and
   90 weapons of the port; `MAX_WEAPONS_PER_TYPE` 2 (was 1); `MAX_WEAPON_RANGE` 7680 subcells,
   30 cells (was 16: the ini has ranges of 18, 25 and 30 cells); `MIN_PROJECTILE_SPEED` 16
   subcells per tick (was 128: the ini's missiles and torpedoes fly at 29 to 191). A
   projectile therefore flies at most (7680 + 2048) / 16 = 608 ticks instead of 32, which is
   the new bound of D28 on the projectiles a weapon has in flight; the soak's limit of 10,000
   projectiles is unchanged. `MIN_WEAPON_RANGE` stays 384 (1.5 cells, D28: a unit reaches
   every neighbouring cell), so the Demolition Truck's `Range=1` is a listed difference.
5. **`MAX_DATA_FILE_BYTES` is 128 KiB (was 64 KiB).** The shipped `combat.ron` has 81 KiB:
   45 warheads with eleven percentages each, 90 weapons, 88 types, and the ini section and key
   of every value in a comment, which the programme requires. The hand-over note of RA2 asked
   for this decision rather than shorter data. The tests of the limit use the constant (a
   file one byte over it is rejected with `FileTooLarge`), so they test the bound as before.
   `arsenal.ron` (60 KiB) no longer stands at the limit.

### R33. RA3-A: one damage path

1. **Impacts** (`crates/sim/src/combat.rs`: `Blast`, `blast_hits`, `splash_damage`; plan
   section 4.3). Every shot ends in an impact of its warhead at a point, and `special.rs`
   detonates its area effects through the same function (`detonate`, R35). With `R` the
   warhead's spread, `P` its percentage at the edge and `d` the distance of a victim from the
   point, the damage is `floor(base x (100 R - (100 - P) d) x versus / (100 R x 100))`, at
   least 1 if `versus > 0` (equation 2's minimum, kept in every world as the brief asks: the
   research note that a product below 1 stays 0 in the original is not followed), 0 for
   `versus = 0`, nothing beyond `R` (`dx^2 + dy^2 <= R^2`, exact), and with a rules file at
   most `max_damage` (10000) per hit. Computed in `i128`. The veterancy and crate
   multipliers of the plan's `D0` and `D3` come with RA4 and RA5.
2. **What an impact hits.** The target of an accurate shot is damaged as at the point itself
   (`d = 0`), wherever its position is. With a spread, everything else of the impact's layer
   within it is damaged by its distance: units at their position, scenery at its cell
   centre, and **a building once for every cell of its footprint whose centre lies within
   the spread** (single source, plan section 4.3; a direct hit counts as the hit of its
   cell). Without a spread a shot damages only its target, as in M4; an impact without a
   target and without a spread (no shipped case) damages what stands in its cell.
   **Friend and foe alike**, the attacker included (Red Alert 2's rule; M9's `blast` already
   hurt everyone).
3. **Layers.** An impact is in the air (the shot was fired at an aircraft) or on the ground
   and at sea (everything else, and every area effect). It damages only its layer, so flak
   that bursts beside an aircraft leaves the tank below alone and a nuclear strike leaves the
   aircraft above alone (appendix D: the original measures in three dimensions, with the same
   outcome at flight height). **Assumption**: an aircraft is always airborne (aircraft do not
   land before RA4).
4. **Where a shot is aimed.** At a unit: its position. At a building: the centre of the cell
   of its footprint nearest to the attacker (**assumption**: the original aims at the
   building's centre or the clicked cell; range is still measured to the nearest point, the
   M4 rule the plan keeps). An accurate projectile follows its target and hits where the
   target is then (the M4 rule; the original's unguided shells can miss a moving target: a
   listed difference). If the target is gone, the projectile is reported as expired as
   before, and if its warhead has a spread it still bursts where it was aimed.
5. **Walls and trees.** A building type that is a `wall` is damaged only by warheads with
   `wall`, and trees (scenery) only by warheads with `wood`; for every other warhead they
   are targets at 0 percent (never acquired; an ordered attack ends with its one shot,
   `AttackEnd::NoDamage`).
6. **Cost.** `blast_hits` looks only at the cells the spread reaches, through the grid's
   occupants and building cover, so an impact does not scan the units of the match
   (`detonate`, used by the rare area effects, builds an index of them once per call).
7. **`SimEvent::Impact`** reports an impact where no shot's event says it would be: an
   inaccurate shot, a projectile whose target was gone, a bomb, a suicide unit, a
   superweapon. The pulses of a radiation zone are not reported.

### R34. RA3-A: weapons, their choice, and what a type may attack

1. **The port** (`scripts/generate_ra2_data.py`, the table `ra2_combat.tsv`, the audit). Every
   weapon a shipped type names as `Primary`, `Secondary`, `ElitePrimary`, `EliteSecondary`,
   `Weapon1` or `EliteWeapon1` is in `combat.ron` with its warhead, except the ten of R36;
   `[SREF]` and `[FV]` name theirs `Weapon1`. Damage is `Damage`; range and minimum range
   `round(cells x 256)`; reload `max(1, round(ROF x 4 / 3))`; burst `Burst`; the layers are
   the projectile's `AA` (default no) and `AG` (default yes); `Verses` in the order of the
   eleven armour types; `CellSpread` in subcells; `PercentAtMax` as percent (default 100);
   `Wall`, `Wood`. Armour, `AirRangeBonus`, `LandTargeting`, `NavalTargeting`, `Underwater`,
   `Organic` of every type. The audit recomputes all of it from the reference and found the
   generator's every value equal; the 37 intentional differences are rows of
   `ra2_differences.tsv` (R36).
2. **Projectile speed** (the ini does not define a flight time; **assumptions**). A
   projectile with `Inviso=yes`, and the leap of a unit that is its own projectile
   (`LimboLaunch=yes`: the dogs, the Terror Drone, the squid's grab), hit at once. A
   projectile with `Arcing=yes` flies at `floor(sqrt(1.2 x Gravity x range))` leptons per
   frame with `[AudioVisual] Gravity=6`, whatever the weapon's `Speed`: this is the
   original's rule for ballistic shots as the engine research describes it (single weak
   source, from the engine's `WeaponTypeClass::GetSpeed`; it is what makes the Grand Cannon's
   `Speed=1` meaningful), taken at the weapon's full range so that the data holds one speed
   (a nearer shot arrives sooner than in the original, where the speed falls with the
   distance). Every other projectile flies at the weapon's `Speed` read as a unit's
   (`min(Speed x 256 div 100, 255)` leptons per frame; documented by ModEnc for projectiles
   with `ROT`). All times `x 3 / 4` for subcells per tick.
3. **The choice** (`sim::weapon_against`, used by the combat stage and by the AI file's
   check): of the weapons the type's targeting allows for where the target is, the first
   whose projectile may engage the target's layer and whose warhead damages the target.
   `Weapon` carries the primary and the secondary weapon (the type's, or what a passenger
   gave an IFV); RA4 puts the elite weapons there on promotion (`CombatTerms::weapons(true)`;
   an elite weapon takes the place of the weapon of its slot, and a slot without one keeps
   its weapon, as the Mirage Tank's `EliteSecondary` without an `ElitePrimary` requires;
   **assumption**, labelled by R56: no source states the engine's rule for a slot without an
   elite weapon, the generator and the audit share it, and for a type without any elite key
   the audit only asserts that its weapons stay the same).
   One reload timer serves both weapons, as in the original.
4. **Land, sea and under water** (the brief's item 4; decided from the ini). The ini says per
   attacking type what it may attack on water: `NavalTargeting` 0 none that is underwater
   (the default: every tank and turret), 1 underwater ones with the secondary weapon (the
   Destroyer), 2 only underwater ones, 3 organic ones with the secondary weapon (the Giant
   Squid against Dolphins and squids), 4 all with the secondary (Tanya), 5 all like targets
   on land (Typhoon, Dolphin), 6 none (dogs, the Terror Drone, the Aegis, the Patriot, the
   Flak Cannon); and `LandTargeting` 0 yes, 1 no (the three submerged types, the Aegis, the
   Patriot, the Flak Cannon), 2 with the secondary weapon. The game ports both codes and the
   target's `Underwater` and `Organic`. So until RA4 adds cloaking a submerged unit (Typhoon,
   Dolphin, Giant Squid) is an ordinary target for exactly: the Typhoon's torpedo, the
   Dolphin's sonic beam, the squid, and the Destroyer's stand-in for its Osprey (R36).
   **Assumptions**: a target is "on water" if it is a ship, an amphibious unit over a water
   cell, or a building whose footprint's origin is water (so tanks may shell a shipyard and
   submarines may torpedo it); neither code applies to an aircraft (the Aegis attacks
   aircraft over land and sea).
5. **Percentages of 0 and 1.** A target none of the type's weapons may be fired at (layer or
   targeting code) is never acquired, and an `Attack` order on it is accepted and ends at
   once with `AttackEnd::NoDamage` without a shot or a move. A target the chosen weapon
   damages by 0 percent is never acquired, and an order ends with its one shot (the meaning
   `NoDamage` has had since D32). **With a rules file** a target damaged by 1 percent or
   less is not acquired either, but an order makes the unit fire on (single source, ModEnc
   `Verses`: the Sniper and Tanya against vehicles and buildings); without a rules file 1
   percent is engaged as before, which the M4 fixture's bullet against plate relies on.
6. **Burst.** `BURST_GAP` = 5 ticks between the shots of a volley (**assumption**: the
   original waits 3 to 5 frames at random), then the reload. A shot of a volley that cannot
   be fired when it is due (no target, out of range, a target that calls for the other
   weapon) ends the volley, and the reload starts then. A strike aircraft (M9's role) turns
   for home after the last shot of its volley.
7. **Minimum range and air range bonus.** A weapon does not fire at a target nearer than
   `min_range` and does not acquire it; a unit ordered to attack one moves to the nearest
   free cell from which it may fire (the M4 chase, which now also keeps its distance). The
   bonus lengthens the reach of an anti air weapon against an aircraft; a unit approaches to
   the weapon's plain range and fires as soon as the lengthened one covers the target.
8. **Scatter** (plan section 4.4; single source). An `Inaccurate` projectile lands on a
   point in one of 64 fixed integer directions at a distance drawn uniformly from 0 to
   `scatter x distance / range`, `scatter` being `BallisticScatter` (1 cell) for the flak
   shell (`[FlakTProj]`) and twice that for the invisible flak (`[FlakProj]`); it has no
   target, and damages by its warhead's spread alone. Two draws from `SimRng` per shot, in
   the id order of the attackers; a world without inaccurate weapons draws nothing, so the
   fixtures' sequences are unchanged. The direction table is rounded toward zero, so no
   impact lies beyond the radius.

### R35. RA3-A: the area effects of the specialists and superweapons, and the scenery

1. **Through warheads.** The combat file's `effects` give the M9 effects the warhead and base
   damage Red Alert 2 uses, **with their M9 timing** until RA6:
   `nuke` 600 through `[NUKE]` (`[NukePayload]`; at once on the command),
   `lightning` 250 through `[IonWH]` (`LightningDamage`; M9's ten bolts on the target cell, one
   every 20 ticks), `ivan_bomb` 400 through `[IvanWH]` (`IvanDamage`; the delay is now the
   rules' `ivan_delay`, 600 ticks, instead of the arsenal's 100), `c4` through `[Super]`
   (`C4Warhead`) and `radiation` through `[RadSite]`. The damage of `c4` (200 a shot at a
   building, M9's number) and of `radiation` (20 a pulse, M9's number) is the game's own:
   the original destroys a building with C4 outright and scales radiation with a level,
   which RA6 brings. A suicide unit's blast is the impact of its own weapon (`[TerrorBomb]`
   225 through `[TerrorBombWH]`, `[Demobomb]` 300 through `[DemobombWH]`) at its own
   position; `combat` deals no other damage for that shot, nor for the shot that plants a
   bomb or takes a mind over. M9's extra blast of the "splash" role (Prism Tower, Prism
   Tank, Grand Cannon, Kirov) is gone with a rules file: the warhead's own spread does it.
2. **A world without the entry keeps M9's rule** (`special::blast`: damage that falls with the
   distance in cells and ignores armour), as a world without a rules file keeps M9's suicide
   blast and splash role. This is the arrangement of R22 item 3, and it is why
   `m9_arsenal.rs` passes unchanged on the legacy fixture; `special::blast` is therefore not
   deleted, as the plan's wording has it, but no shipped effect reaches it.
3. **Scenery** (`props.rs`; the brief's item 3). Trees, houses, city blocks and towers take
   damage as the armour type `wood`, the armour of the lightest civilian buildings of the
   reference (of its 223 `CA...` buildings 113 are `steel`, 67 `concrete`, 40 `wood`), since
   all scenery has one armour type in `props.rs`; and trees only from warheads with
   `Wood=yes`. With the legacy ruleset the game still looks for `structure`. The hit points
   of scenery stay the game's own (`TreeStrength=200` is in `rules.ron`, unused: the four
   kinds of scenery are not the original's terrain objects). **Assumption**: `Wood` is the
   flag for trees (ModEnc describes it for wooden overlays and terrain objects).

### R36. RA3-A: stand-ins, and the differences left for RA4 and RA6

The audit lists each as a row of `ra2_differences.tsv`; the generator's tables `NOT_CARRIED`
and `WEAPON_VALUES` hold them.

**RA4 removes** (spawned missiles and aircraft):

* V3 Launcher: its shot is a projectile with the rocket's damage (`V3RocketDamage` 200), warhead
  (`[V3WH]`) and cruise speed (29 subcells a tick), fired every 533 ticks (`SpawnRegenRate`),
  with the launcher's range (18 cells) and minimum range (5); it cannot be shot down.
* Dreadnought: the same with `DMislDamage` 300, `[DMISLWH]`, 38 subcells a tick, volleys of 2
  every 107 ticks, range 25, minimum range 8.
* Aircraft Carrier: M9's mechanism (its shot launches two recoverable aircraft of the Harrier's
  type, which attack with their own weapon); the launcher's warhead, which decides what the
  carrier may attack, is that of the Hornet's bomb.
* Destroyer against submerged targets: its secondary weapon is the Osprey's depth charge
  (`[ASWBomb]` 50 through `[APSplash]`) every 200 ticks (`ROF=150`) at the range of its gun.
* Elite weapons are in the data and unused: no unit has a rank. Aircraft do not land, have no
  ammunition and need no dock. Submerged units are not cloaked.

**RA6 removes** (specialists):

* Not carried: the GI's deploy (its `Para` is carried, and never chosen: it fires the same
  targets as the `M60`), the Engineer's `DefuseKit`, the Spy's `MakeupKit`, Tanya's and the
  Chrono Commando's C4 weapons (`Sapper`, `FakeC4`), the Tesla Trooper's `AssaultBolt`, the
  Prism Tower's `PrismSupport` (the Tesla Coil's `OPCoilBolt` is carried and never chosen),
  Yuri's `PsiWave`, the Desolator's `RadEruptionWeapon` (M9's deploy zone stands in), and the
  dogs', the Engineer's and the Terror Drone's `VirtualScanner` (never fired in the original).
* M9's mechanism with the ini's range, reload and warhead, and M9's damage where the ini's
  number is not hit points: the dog's bite (200; the original kills infantry outright), the
  Chrono Legionnaire's rifle (60 a shot and the target frozen for 100 ticks; the original
  erases), the Dolphin's sonic beam (100 a shot; the original damages along the wave).
  With the ini's damage: the Terror Drone (50 every 80 ticks and the vehicle immobile; it
  does not kill infantry outright or enter the vehicle), the Giant Squid (15 every 132 ticks
  and the ship held; no destruction at red health), Yuri (one unit, no damage).
* Crazy Ivan's weapon names `[IvanWH]`, the warhead its bomb goes off with, instead of the
  ini's `IvanBomb`, which has no `Verses`; the Demolition Truck's range is 1.5 cells, not 1.
* The effects of R35 with M9's timing; no radiation after a nuclear strike or a Demolition
  Truck; no death weapons (`[NANRCT]`, `[DTRUCK]`, `[TERROR]`); the IFV takes the primary
  weapon of its passenger instead of its own table.
* Not modelled at all (for RA9's list): the shrapnel of the Prism Tank's and the elite Tesla
  weapons' projectiles (`CometFragment`, `TeslaFragment`), prone infantry (`ProneDamage`),
  height, walls that stop shots, the 0 to 2 frames of reload jitter, shells that miss a
  moving target.

### R37. RA3-A: damage states, and the damaged vehicle

`Condition` (a component of every unit and building, hashed) holds green, yellow (hit points
at or below `condition_yellow_percent`, 50, of the type's) or red (`condition_red_percent`,
25); `update_conditions` sets it at the end of the `Cleanup` stage, in id order, and reports
every change with `SimEvent::DamageStateChanged`. Only with a rules file: a fixture world's
units stay green. A vehicle or ship (the arsenal's category `vehicles`, not an aircraft) that
is not green moves at `speed - speed div 4` (plan section 4.1; the form of the released
Red Alert source), set where M9 already sets a unit's speed every tick (`special::advance`),
so it follows the state of the tick before. Infantry and aircraft are not slowed. Nothing
else reads the state yet (garrison eviction is RA5's).

### R38. RA3-A: the computer player, and how long matches are now

1. **Nothing was retuned and no limit was changed.** With the weapons and armour of Red Alert 2
   the plans of `ai.ron` (Grizzlies, GIs and IFVs; Rhinos, Conscripts and Tesla Troopers;
   Pillboxes and Sentry Guns) still end every match: `m14_duels -- full` plays the 756
   matches of `m6_production_matches.rs` and all 756 end with a winner within the limits of
   that test (`72,000 x span / 128`, R29).
2. **The army checks** (`ai_defs.rs`, `ai.rs`). A unit of an `army` list must have a weapon
   against targets on the ground: a unit that attacks aircraft only cannot win a match, the
   AI file rejects it, and a computer player counts only units with such a weapon as armed
   (`View::fights`). The check that an army can damage every building type now asks
   `weapon_against` for the weapon a unit would use against the building on land (its
   targeting codes, the layer, the `wall` flag) and, with a rules file, requires more than 1
   percent (R34 item 5). Both shipped plans pass: the Grizzly's and the Rhino's shells
   (`[AP]`, `Wall=yes`) damage every armour type, the walls included, where the GI's and the
   Conscript's bullets do nothing to walls.
3. **Measured** (`m14_duels -- full`; ticks; "limit" is the test's). Before: the validation of
   RA2 (`docs/validation/ra2-parity/ra2/10-duels.log`). After: the run of RA3-A before its
   validation; the validation's log has the final numbers.

   | | Before RA3-A | After RA3-A |
   |---|---|---|
   | Length over the limit: median, 90th percentile, longest | 0.11, 0.24, 0.75 | 0.19, 0.36, 0.76 |
   | Small maps (span up to 128): median of each map | 12,492 to 15,685 | 20,868 to 25,089 |
   | Small maps: longest | 53,699 (Crossroads) | 50,108 (Riverlands, Great Britain easy against America brutal) |
   | Large maps: median of each map | 12,647 to 45,742 | 19,447 to 70,077 |
   | Large maps: longest | 161,774 (Oasis Gulf) | 206,967 (Ember Caldera, Germany against Great Britain, both brutal) |
   | Matches over the limit, matches without a winner | 0, 0 | 0, 0 |

   A match is 1.4 times as long at the median and all matches together take 1.5 times the
   ticks: a Grizzly needs five shells for another Grizzly where the tank of M9 needed eight
   shots at less than half the reload, bullets deal a quarter to tanks and to concrete, and a
   Construction Yard's concrete takes 60 percent of a shell. The longest match stands at
   0.76 of its limit (Iron Isthmus, Libya against Iraq, both medium: 204,250 of 270,000),
   as the longest did before (0.75). The duels and the end to end tests therefore take about
   half as long again to run.
4. **Benchmark** (`m7_benchmark`, budget: mean 5 ms): fixture 0.09 ms, shipped 0.18 ms a tick
   (0.10 and 0.19 before); an impact looks only at the cells its spread reaches (R33 item 6).
   In the shipped scenario, which fields every armed land unit, the Demolition Trucks'
   8 cell blasts now thin both armies in the first ticks, so its "busy" mean covers one tick.
5. **Debt for RA8.** The plans still use ground units only; a computer player builds nothing
   against aircraft or ships and no unit of the other tiers.

### R39. RA3-A: the interface

1. **Weapon effects by id** (`crates/game/src/weapon_fx.rs`, the one table the game keys by
   weapon id; `audio.rs` plays by the class it yields, `fog.rs` names no weapon).
   `shipped_class` gives every weapon of the shipped combat file its class (rifle, machine
   gun, sniper, cannon, artillery, rocket, flak, prism, Tesla, radiation, psychic, chrono,
   sonic, bomb, torpedo, melee, trigger); a test fails on a shipped weapon the table lacks.
   The ids of the arsenal before RA3 keep their classes, for the legacy ruleset. The ground
   guns of the flak units are drawn as shells, and a shot with a flight time is drawn in
   flight only if it is a shell, rocket, bomb or torpedo (`flies`): the sonic beam, which now
   takes a few ticks, is drawn when it is fired.
2. **Impacts by warhead.** `SimEvent::Impact` (R33 item 7) draws a burst where the warhead
   went off, as large as its spread (`impact_size`), if the human player sees the cell: the
   scattered flak, a bomb, a suicide unit, the nuclear strike, the bolts of the storm. A
   burst on the ground counts as a heard impact for `audio.rs`.
3. **Damage smoke by state** (`weapon_fx::smoulder`, `damage_smoke`): a yellow vehicle or
   building trails thin grey smoke, a red one thick black smoke more often; infantry none.
4. **Hover text**: a third line names the armour type (`hover_info::armour_line`: "Heavy
   armour", "No armour").
5. No asset was added: the effects are drawn from the textures the game already makes at
   startup, so `assets/LICENSES.md` is unchanged. **Requires user verification**: how the
   bursts, the smoke and the classes of the new weapons look and sound.

### R40. RA3-A: tests adapted (none deleted, ignored or weakened)

**Schema migration** (the assertions are unchanged; the combat fixture is at schema 2 with the
neutral values of R32 item 3):

| Test | What changed |
|---|---|
| `m4_definitions.rs` (the 12 tests of M4) | the text edits match the fixture's new layout: an entry of `units` or `buildings` continues on a second line, so the fragments that ended it with `)` end with `,`; `weapons: ["rifle"]` and `weapons: ["howitzer"]` are matched with the `sight` before them, since `elite_weapons: [...]` contains the same text; the unknown weapon field is `salvo` (`burst` is a field now); the expected `WeaponDef` and `CombatTerms` literals have the new fields with the neutral values; `generated()` writes schema 2 |
| `m4_definitions.rs::schema_versions_other_than_1_are_rejected` | renamed `schema_versions_other_than_2_are_rejected`; rejects 0, 1, 3, -1 and the two extremes against 2 (one case more) |

**Expectation changed because RA2 behaviour is intended** (the limits of R32 item 4):

| Test | What changed and why |
|---|---|
| `m4_combat.rs` (helper `extreme_scenario`) | writes the cannon's range and projectile speed from `MAX_WEAPON_RANGE` and `MIN_PROJECTILE_SPEED` instead of the literals 4096 and 128; its assertions that the fixture holds exactly these limits are unchanged |
| `m4_combat.rs::the_longest_range_with_the_slowest_projectile_flies_32_ticks` | renamed `..._flies_480_ticks`: the longest range is 30 cells and the slowest projectile 16 subcells a tick, so the tank stands at x = 0, the target 30 cells away is seen by a crawler of the tank's owner two cells from it (a sight is at most 16 cells), the shell flies 7680 / 16 = 480 ticks and the damage lands exactly then; one cell farther (31, seen) the tank still does not fire. Same assertions on the new numbers, one added (the far target is visible) |
| game `weapon_fx::tests::every_shipped_weapon_has_the_class_its_name_promises` | every row is kept (the ids are the legacy ruleset's now); 36 rows for the shipped weapons of Red Alert 2, `flies` and `impact_size` and `damage_smoke` are added |
| game `weapon_fx::tests::the_shipped_combat_file_classifies_light_and_electricity` | the shipped ids it names are Red Alert 2's (`prism_shot`, `comet`, `super_comet` for light; `coil_bolt`, `tank_bolt`, `electric_bolt` for electricity; `gun_105mm`, `m60`) instead of `prism_beam`, `tesla_weapon`, `tank_cannon`, `rifle`; added: no shipped weapon is left to the fallback |

`m9_arsenal.rs` (24 tests) and `m11_repair.rs` run on the legacy fixture, which has no rules
file and no effects and so keeps M9's damage rules (R35 item 2): unchanged.

**New in RA3-A:** `m4_definitions.rs` 4 tests for the fields of schema 2 and the effects (each
required, its range, its references, the rules between them); `ra3_damage.rs` (14: the
equation at its boundaries, splash at 0, half and full spread and one subcell beyond, a
building's hit per cell, layers, friendly fire, walls and trees, the largest damage, and each
area effect through its warhead, with M9's rule where the file lists none); `ra3_weapons.rs`
(16: the choice of the weapon, layers, 1 percent, every targeting code, burst and its
interruption, minimum range at its boundary, the air range bonus, scatter within its radius
and its determinism, damage states exactly at 50 and 25 percent, the damaged vehicle's speed,
a deterministic battle of 20 against 20 mixed types, hashing, the army checks of the AI file)
on the new fixture `fixtures/data/ra3` with `tests/ra3_common/mod.rs`; `ra3_shipped.rs` (7:
every shipped weapon against every armour type, 990 pairs, against the reference; 40 shots
fired through the simulation against numbers worked out from the ini; the nuclear strike on a
Construction Yard and a Power Plant; the Grizzly's damage states and speed; the shipped
plans); `ra9_data_parity.rs` one test more and the first extended (R34 item 1); 2 unit tests
in `sim::combat`; in the game `hover_info` 1.

### R41. RA3-B: the arsenal file of schema version 4, and the type lists in the reference

1. **`arsenal.ron`, schema 4** (`crates/sim/src/arsenal.rs`; the plan's item 6). Every field is
   required. A unit gains `infantry` (1 or 0: the ini's `[InfantryTypes]` lists it), `crushable`
   (`Crushable`), `crusher` (`Crusher`) and `size` (`Size`, 0 to 255); a building has 0 in all
   four. Rejected: a value outside its range; any of the four on a building; `infantry` outside
   the category `infantry`; `crushable` without `infantry`; `crusher` on a unit that is not of
   the category `vehicles` or that flies (a ship or an amphibious vehicle may be one: the
   Soviet Amphibious Transport is). Unlike the traits of schema version 3 they are read in
   every world, with or without a rules file: movement has no second rule to fall back on, and
   a world whose arsenal gives no unit the traits is the world before RA3 (R6 item 2).
2. **Nothing reads `size` yet.** It is the room a unit takes in a transport; the brief asks for
   it with the other three so that the schema changes once. RA4 or RA6 (transports by
   `SizeLimit` and `Passengers`) will read it; until then the transports keep M9's capacity.
3. **The reference table** (`scripts/ra2_reference.py`). It gains the entries of the four type
   lists (`[InfantryTypes]`, `[VehicleTypes]`, `[AircraftTypes]`, `[BuildingTypes]`) that name
   a mapped section: 99 rows, index and section id only. The generator
   (`scripts/generate_ra2_data.py`) reads `[InfantryTypes]` for `infantry`, cites the entry in
   the comment of the arsenal line, and fails if its own category of a unit disagrees with
   the list or a crusher is no surface vehicle. The audit (`ra9_data_parity.rs`) recomputes the
   four fields of every mapped unit from the table; there is no difference row.
4. **Defaults** where a section has no key. `Crusher`: no (ModEnc, `Crusher`; the ini writes
   it on every vehicle that has it). `Size`: 1 (the ini's own comment on the key).
   `Crushable`: **yes for a unit of `[InfantryTypes]`, no for every other unit. Assumption.**
   The ini writes `Crushable=yes` on four infantry types (GI, Rocketeer, Tanya and the SEAL)
   and `Crushable=no` on eleven (Tesla Trooper, Desolator, Chrono Legionnaire, the four stolen
   technology infantry and four animals), and nothing on the Conscript, the Engineers, the dogs,
   the Sniper, the Flak Trooper, Crazy Ivan, Yuri, the Spy and the Terrorist; its comment on
   the key says "def=no", and so does ModEnc's page on `Crushable`. But the explicit `no` on
   those eleven would then say nothing, the plan's research (appendix D, "Crushing")
   and its test list ("a tank crushes a Conscript") take the Conscript for crushable, and
   driving tanks over Conscripts is what the original's players do. If the assumption is
   false, nine shipped infantry types that are crushable here are not in the original.
5. **Shipped values.** `infantry`: 19 types; on foot all of them but the Rocketeer, which
   flies. Crushable: 12 of them (GI, Conscript, Engineer, Attack Dog, Spy, Tanya, Sniper,
   Flak Trooper, Crazy Ivan, Yuri, Terrorist; and the Rocketeer by its data, which is never on
   foot). Crushers: the two miners, Grizzly, Rhino, Flak Track, V3 Launcher, both MCVs, Mirage
   Tank, Prism Tank, Apocalypse, Tank Destroyer, Tesla Tank, Soviet Amphibious Transport (14).
   Not: IFV, Terror Drone, Demolition Truck, the Allied Amphibious Transport, ships, aircraft.
6. **Fixtures.** The arsenals of the legacy ruleset and of the RA1 and RA2 fixtures are at
   schema 4 with 0 in the four fields of every entry. The RA3 fixture keeps 0 on every type it
   had (the tests of RA3-A place their riflemen at cell centres) and gains seven unit types at
   the end of its files: `recruit` and `trooper` (crushable infantry on foot, the second with
   the rifle), `hazmat` (infantry on foot that is not crushable), `jumper` (crushable infantry
   that flies), `truck` (a vehicle), `roller` and `brute` (crushers; the brute carries the sam,
   which engages aircraft only).

### R42. RA3-B: crushing

1. **The rule** (`movement.rs`, `Crushable`; the plan's item 6). A unit whose move order may
   crush does not wait for a cell that is held only by crushable infantry on foot of its
   enemies (`Players::hostile`: another player that is not of its team): it drives in, and
   every one of them dies. Not crushed, and so in the way like any unit: its own and its
   allies' soldiers; infantry that is not crushable; a cell that holds any of these beside
   crushable enemies; vehicles; and whatever flies (it is in another layer).
2. **Which orders may crush.** `MoveOrder::crush` (hashed) is set for the move of a `Move` or
   `AttackMove` command to a unit of a `crusher` type (`MoveOrder::commanded`), including the
   move an attack-moving unit takes up again after a fight, and for no other order: not the
   approach of an `Attack` order, not a harvester's trips, not the way to a rally point, a
   transport, a building or a depot. This is the brief's reading of `PlayerAutoCrush=no`: units
   do not seek infantry to crush, and what the player sends across infantry crushes it. The
   pathfinder ignores infantry that the unit would crush when it looks for a way around
   standing units, for such an order only. **Difference from the original**: there the miners
   (`AutoCrush=yes`) crush on their own trips, and a computer player's unit near a crushable
   target (`[CombatDamage] Crush=1.8`) drives at it instead of firing; here the computer
   player crushes only what its attack moves happen to cross (the brief: it "does not use it
   deliberately").
3. **When they die.** In the `Movement` stage, at the moment the crusher starts the step into
   the cell: every soldier that holds a spot there (standing, walking in, or walking out) is
   removed from the grid before the crusher claims the cell, so that the cell never holds a
   vehicle and infantry (I6'), and its entity is despawned by the stage's commands, before the
   `Combat` stage of the tick: a crushed soldier fires no more and is nobody's target. The
   crusher does not slow down. **Boundary**: a soldier is crushed until the tick in which it
   has left the cell; in that tick the units move in the order of their ids (D19), so a
   soldier with a lower id than the crusher has left and lives, one with a higher id still
   holds its spot and dies (`ra3_crush.rs::a_soldier_is_crushed_until_the_tick_in_which_it_has_left_the_cell`).
4. **Events.** `SimEvent::UnitCrushed { player, unit, crusher, by, at }` (the victim's owner,
   the victim, the crusher, **the crusher's owner, who is the killer for the experience of
   RA4**, and where the victim stood in the crushed cell), followed in the same tick by the
   `UnitDestroyed` that everything that counts losses already reads (the soak's rule for
   rejected commands, the game's views).
5. **Hit points are not involved**: a crushed soldier is removed whatever its health, also an
   invulnerable one if RA6 ever makes infantry invulnerable (the Iron Curtain kills infantry
   in the original, so the case should not arise).

### R43. RA3-B: the interface

1. **Soldiers at their spots.** Nothing to draw anew: a unit is drawn at its `Position`
   between ticks, which is the spot, and the walk cycles follow the ground covered as before.
2. **Picking and hover.** `selection::unit_at` already picks the unit whose body centre is
   nearest to the cursor among those whose pick area contains it, and the hover label uses the
   same function: with three soldiers 17 to 24 pixels apart a click on a soldier's body picks
   that soldier (`health::tests::the_bars_of_three_soldiers_in_a_cell_do_not_overlap_and_each_can_be_picked`).
   A box selects every soldier whose body centre (its ground point raised by
   `GROUND_OFFSET_PX`, as `selection.rs` and the README say) lies in it, as before.
   (Corrected by R56: this sentence said "ground point".)
3. **Health bars.** The bar of a unit on foot is 16 pixels long instead of 32
   (`health::unit_bar_length`): the two soldiers that stand side by side are 24 pixels apart,
   and the third stands 12 pixels higher, so no two bars meet.
4. **Crushing.** A ring of dust and three brown puffs where the soldier stood
   (`weapon_fx::Painter::crush`, from the smoke and ring textures the effects already use),
   if the human player sees the cell; and a new synthesised sound, `Sfx::Crush` (a dull thud
   and a short dry crunch, `audio::crush`), heard by the soldier's owner, by the crusher's
   owner and by who was shown the soldier. The bang that every destroyed unit has is not
   played for a crushed one. No asset file was added; `assets/LICENSES.md` is unchanged.
5. **`E` mode.** A click still takes the first holder of the cell under the cursor as its
   target; its targets are transports, buildings and depots, none of which shares a cell.
6. **Requires user verification**: that three soldiers in a cell read as three, that the
   short bars are legible, how the dust and the crunch look and sound, and how crushing feels
   in play (a tank does not stop for infantry).

### R44. RA3-B: the computer player, and how long matches are now

1. **It does not use sub cells or crushing deliberately.** Its `Move` and `AttackMove` commands
   crush what they cross, as a player's do (R42 item 2); its group moves fill cells with three
   soldiers, as everyone's. One check changed: before it deploys a base unit, every holder of
   a cell around the footprint must be one of its own idle units (was: the occupant).
   `ai.ron` is unchanged, and no limit of a test was changed.
2. **A defect of RA1 that the changed battles exposed, fixed** (`ai.rs`,
   `put_harvesters_to_work`). An armed harvester fights when its owner has no other armed
   unit and its cycle has stopped (R14). In the wave it has no move order whenever it stands
   and fires, and the computer player then gave it the `Harvest` command it gives every idle
   harvester every 200 ticks: that ended its attack, and with no ore left it was idle at the
   enemy's base, was called home to the gathering point, and set out again a wave's patience
   later: a few hundred ticks of fire for every several thousand ticks. With three soldiers
   to a cell one of the 756 matches of `m14_duels -- full` ended in exactly this state:
   Riverlands, Libya
   against Great Britain, both brutal, 108,205 ticks against a limit of 72,000 (traced: from
   tick 27,000 on one War Miner against four buildings). A harvester of the wave is no longer
   sent to harvest; it leaves the wave, and is sent to work again, when its owner has another
   armed unit. The match takes 39,902 ticks. Regression test:
   `ra3_ai.rs::an_armed_harvester_that_fights_as_a_last_resort_is_not_called_back_to_harvest`
   (on the RA2 fixture, whose Soviet miner is armed; it fails without the fix in the tick of
   the first `Harvest` command). **To revisit in RA5**, when ore grows back: a harvester in
   the wave then has something to return to.
3. **A second defect of RA1, exposed the same way, fixed** (`ai.rs`, `View::stays_visible`).
   A placement is applied one tick after the computer player issues it, so it places only on
   cells that are certain to stay visible: within the sight of one of its buildings, or of one
   of its units both from where the unit stands and from where it steps (D43). A unit that
   teleports breaks the "or": a Chrono Miner with a full load is at its refinery in the next
   tick. In the first validation run of RA3-B one of the 756 matches of
   `m6_production_matches.rs` (Thousand Lakes, Iraq against Korea, both brutal, tick 13,622)
   had a refinery placed on cells that only a Chrono Miner parked beside them lit; the miner
   teleported, and the command was rejected as not visible without any loss, which that test
   and the soak do not accept (`docs/validation/ra2-parity/ra3/first-run/`). Units of the
   arsenal's roles `chrono_miner` and `chrono` are no longer counted. Regression test:
   `ai::tests::a_cell_lit_only_by_a_unit_that_teleports_is_not_counted_on` (a unit test on the
   legacy fixture, which has both roles; it fails without the fix).
4. **Measured** (`m14_duels -- full`: the 756 pairings of `m6_production_matches.rs`, which
   plays them with another seed and so along other courses; ticks; "limit" is that test's,
   `72,000 x span / 128`). Before: the validation of RA3-A
   (`docs/validation/ra2-parity/ra3a/10-duels.log`). After: the validation of RA3
   (`docs/validation/ra2-parity/ra3/10-duels.log`).

   | | Before RA3-B | After RA3-B |
   |---|---|---|
   | Length over the limit: median, 90th percentile, longest | 0.19, 0.36, 0.76 | 0.16, 0.33, 0.96 |
   | Small maps (span up to 128): median of each map | 20,868 to 25,089 | 20,855 to 26,320 |
   | Small maps: longest | 50,108 (Riverlands) | 69,221 (Riverlands, America against Korea, both brutal) |
   | Large maps: median of each map | 19,447 to 70,077 | 13,096 to 51,825 |
   | Large maps: longest | 206,967 (Ember Caldera) | 167,856 (Iron Isthmus, Russia brutal against Libya medium; limit 270,000) |
   | All 756 matches together | 34.6 million ticks | 30.3 million ticks |
   | Matches over the limit, matches without a winner | 0, 0 | 0, 0 |

   Matches are shorter: three soldiers to a cell bring three times the rifles to a fight, and
   the large maps, where armies meet in the open, gain most (their medians fall by a quarter
   to a third). The small maps are as they were at the median.
5. **The one match near its limit is not a stall.** Riverlands (64 x 48, the smallest map),
   America against Korea, both brutal, ends in tick 69,221 of 72,000 (0.96; before RA3-B the
   nearest was 0.76). Traced: the ore within the miners' reach is used up by tick 12,000
   (every miner stands idle from then on); both armies are spent by tick 30,000, when Korea
   has one GI left and America four Chrono Miners that stand idle and three buildings. The GI
   fires without a pause from then on: at the miners first, which are the nearest targets
   (1,000 hit points each, of which a rifle takes about 3 a shot: 33,000 ticks for the four),
   then at the buildings (6,000 ticks). Nothing is stuck and nobody waits; the match is long
   because a rifle is weak against a miner, and because a wave fires at what is nearest and
   not at what ends the match. **Debt for RA8** (and less likely once RA5 lets ore grow
   back): a wave that sees no armed enemy should go for the buildings. The limit was not
   raised; the matches of `m6_production_matches.rs` itself, which have another seed, all
   end within it.
6. **Benchmark, soak** (`docs/validation/ra2-parity/ra3/`; budget: mean 5 ms a tick, longest
   tick 50 ms). Mean tick: fixture 0.10 ms, shipped 0.21 ms (0.09 and 0.18 in the validation
   of RA3-A). The longest tick of the shipped scenario is 5.7 ms (1.1 before).
   Measured: the ticks above 1.5 ms are ticks 10, 12, 20 and 28 after the order, with 188 to
   191 units alive. Not measured further; the likely cause (**assumption**) is that the units
   of the two packed groups that have waited eight ticks for a cell then ask for paths
   around each other, that with three soldiers to a cell more of them wait for the same
   cells, and that a search no longer stops at a cell that holds one or two soldiers. A
   tenth of the budget. The soak takes 2,677 s for its 18.72 million ticks (2,208 s before);
   the difference was not broken down (its check of I6' now reads every unit on every tick,
   and the matches are other matches). A grid with infantry
   on foot is 12 bytes a cell larger; a path request of a ship or an aircraft copies it with
   the grid, as before.

### R45. RA3-B: tests adapted (none deleted, ignored or weakened)

**Schema migration** (the assertions are unchanged; the arsenals of the fixtures are at schema
4 with 0 in the four new fields of every older entry):

| Test | What changed |
|---|---|
| `ra1_definitions.rs` (helper `arsenal_of` and the text edits of four tests) | the header they match and write is `(schema_version: 4, entries: [` |
| `ra1_definitions.rs::other_schema_versions_are_rejected_in_every_file` | the arsenal rejects 3, 2, 1 and 5 against 4 (one case more) |
| `ra1_definitions.rs::every_field_of_the_arsenal_file_is_required` | also removes each of the four new fields in turn (four cases more) |
| `ra2_definitions.rs::the_fixture_loads_with_the_traits_of_schema_version_3` | asserts `ARSENAL_SCHEMA_VERSION == 4`, which keeps the traits of version 3; the traits asserted are unchanged |
| `ra9_data_parity.rs::every_shipped_unit_and_building_equals_its_reference_or_is_a_listed_difference` | extended: `infantry`, `crusher`, `crushable` and `size` of every mapped unit against the reference, and none of them on a building |

**Compile migration**: game `audio::tests::events_play_only_what_the_human_player_may_know`:
the `SoundContext` literal gained `crushed`; five assertions added (who hears a crushing, and
that the destruction after it is silent); the others are unchanged.

**Expectations changed: none.** The tests of the shipped data (`ra1_shipped.rs`,
`ra2_shipped.rs`, `ra3_shipped.rs`) pass unchanged, though their infantry now stands at spots
and some of their scenarios take a few ticks more or less (R6 item 9). `m2_movement.rs` and
every other test of a fixture without the trait run tick for tick as before.

**The soak** (`examples/m7_soak.rs`): its check of I6 is the check of I6' (R6 item 8); the
other checks are unchanged.

**New in RA3-B:** `ra3_cells.rs` (22: the spots; three soldiers in a cell and what does not
fit; a cell holds a vehicle or infantry; walking from spot to spot in the ticks of a step, and
keeping the spot; a fourth soldier waits, or stops beside a full cell; group moves; a vehicle
cannot enter a cell with a friendly soldier, a soldier none with a vehicle; a soldier passes a
soldier and a vehicle follows soldiers; range from the spot; the attack approach; splash on
three soldiers; leaving a barracks and a transport; I6' for 2,000 ticks; determinism; the hash
and the spots; the check itself; the older fixtures), `ra3_crush.rs` (13: positive on `Move`
and `AttackMove` and through the pathfinder, three at once, gone at once; negative for its
own, an ally's, what is not crushable, mixed cells, vehicles, what flies, units that are no
crushers, and an attack's approach; the boundary tick both ways and a soldier walking in; I6'
and determinism under crushing), `ra3_definitions.rs` (6), `ra3_ai.rs` (1, R44),
`ra3_shipped.rs` 3 more (the shipped traits; three GIs in a cell; tanks over Conscripts, GIs
and Engineers and round a Tesla Trooper, a Desolator and a Chrono Legionnaire, and an IFV
round a Conscript); 4 unit tests in `sim::map` and `sim::unit` (the grid's rule, the hash of a
grid with and without spots against the layout before RA3, a soldier's position between
spots, the hash of the crush flag); in the game `audio` 1 and `health` 1.

### R46. RA4: the arsenal file of schema version 5 and the combat file of schema version 3

1. **`arsenal.ron`, schema 5** (`crates/sim/src/arsenal.rs`; the plan's items 1 to 5). Every
   field is required. An entry gains sixteen fields: `trainable` (`Trainable`),
   `veteran_abilities` and `elite_abilities` (the tokens of `VeteranAbilities` and
   `EliteAbilities`: `stronger`, `firepower`, `rof`, `sight`, `faster`, `self_heal`, `scatter`,
   the seven the ini uses; each at most once in a list), `crewed` (`Crewed`), `self_healing`
   (`SelfHealing`), `ammo` (`Ammo`; 0 for the ini's -1, unlimited), `airport_bound`
   (`AirportBound`), `docks` (`NumberOfDocks`), `spawns`, `spawns_number`, `spawn_regen`,
   `spawn_reload` (`Spawns`, `SpawnsNumber`, and the two rates in ticks), `launcher`
   (`"primary"`, `"secondary"` or `"none"`: which weapon has `Spawner=yes`), `missile` (`"v3"`,
   `"dmisl"` or `"none"`: the type is `[General] V3RocketType` or `DMislType`), `cloakable`
   (`Cloakable`) and `sensors_sight` (`SensorsSight` of a type with `Sensors=yes`, else 0).
   **Instead of the plan's `sensors` and `dock` traits**: `sensors_sight` 0 is "no sensors",
   and an airport bound aircraft lands on every building with `docks` (the generator fails if
   two such aircraft name different buildings as their `Dock`; in the ini both name the two
   Airforce Commands), so the arsenal holds no list of buildings per aircraft. `self_healing`,
   `launcher` and `missile` are beyond the plan's list: the first is the brief's ("self
   healing"), the other two are what tells the simulation which weapon launches and what a
   launched unit is, which the ini says with `Spawner=yes` and with two keys of `[General]`.
2. **Rejected**: a flag outside 0 and 1; a number outside its range (`MAX_AMMO` 255,
   `MAX_DOCKS` 16, `MAX_SPAWNS` 8, `MAX_SPAWN_TICKS` 72,000, `MAX_SIGHT` for the sensors); an
   unknown or repeated ability; every trait of a unit on a building (all but `crewed`,
   `sensors_sight` and `docks`) and `docks` on a unit; more docks than the footprint has cells;
   `airport_bound` on a unit that is no aircraft with ammunition; a spawned type that is not
   defined, is the type itself, is no aircraft of the arsenal, or spawns something itself;
   spawn terms without a spawned type and a spawned type without a number, a time to replace
   or a launcher; a missile that does not fly. Like the traits of schema version 3 they are
   read only with a rules file.
3. **`combat.ron`, schema 3.** The list `effects` may name four more impacts: `v3_rocket`,
   `v3_rocket_elite`, `dmisl`, `dmisl_elite` (`[General] V3RocketDamage`, `V3RocketEliteDamage`,
   `DMislDamage`, `DMislEliteDamage` through `[CombatDamage] V3Warhead`, `V3EliteWarhead`,
   `DMislWarhead`, `DMislEliteWarhead`). Nothing else changed; the version is raised because
   a reader of version 2 rejects such a file. The rules file also holds the four damages since
   RA1 (`combat.v3_rocket_damage` and its like); the simulation reads the effects, which name
   the warhead with the damage, and the audit compares both with the reference.
4. **The data** (`scripts/generate_ra2_data.py`, `ra2_combat.tsv`, `ra2_mapping.tsv`). The four
   spawned types are in the shipped data now (`hornet`, `osprey`, `v3_rocket`, `dred_missile`;
   tech level -1, so that nothing queues them), with the warheads `[V3EWH]` and `[DMISLEWH]`
   and the weapons `[HornetBomb]`, `[HornetBombE]` and `[ASWBomb]`. **Defaults** where a section
   has no key: `Trainable` yes for a unit and no for a building (ModEnc), `Crewed` no,
   `SelfHealing` no, `Ammo` -1, a rate of 0 frames stays 0 ticks. The audit
   (`ra9_data_parity.rs`) recomputes all sixteen fields of every mapped item and the four
   effects from the reference.
5. **Fixtures.** The arsenals of the legacy ruleset and of the RA1, RA2 and RA3 fixtures are at
   schema 5 with the neutral value in every new field, their combat files at schema 3 with
   the lists they had. New: `fixtures/data/ra4` (the RA3 fixture with an airfield, a launcher
   and its rocket, a carrier and its drone, the boat's hunter, a striker, and the traits on
   the types its header names) with `tests/ra4_common`.
6. **Size.** The shipped `arsenal.ron` has 100 KiB of the 128 KiB a data file may have
   (`MAX_DATA_FILE_BYTES`, R32): sixteen fields on 91 entries, and the ini keys of each in its
   comment. **Debt for RA5 and RA6**: another step of this size does not fit; the next schema
   should shorten the entries (the label and description could move to a file of their own)
   or the limit be raised again with a record.

### R47. RA4: veterancy

1. **State** (`crates/sim/src/veterancy.rs`). A unit of a `trainable` type has the component
   `Experience { value, rank, ranked, bonus, self_heal }`: the summed cost of what it
   destroyed, its rank (0, 1 veteran, 2 elite), the multipliers of its rank and the bonus
   multipliers (each `firepower_pm`, `armour_pm`, `reload_pm`, `speed_pm`), and whether a rank
   ability makes it heal itself. It is given in the `Economy` stage of the first tick the unit
   exists in (`enlist`), so every way a unit comes into being is covered; a unit without it
   counts as rank 0 with neutral multipliers. Buildings and untrainable units have none.
   Hashed at the end of `state_hash`, in id order, under a tag, and only if any unit has it.
2. **Who gains** (decided from the plan's section 4.5 and appendix D topic 6). The unit whose
   hit brings a victim's hit points to 0 gains the victim's cost (the economy file's `cost` of
   the victim's type, whatever was paid). The hits of a tick are walked in the order they
   happen (projectiles in the order they were fired, then the shots of the attackers in id
   order), and the hit that crosses 0 names the killer; damage is still applied only after
   all have fired. A kill by splash counts for the unit that fired; by crushing for the
   crusher (`UnitCrushed` already named it); by a spawned missile or aircraft for the unit
   that launched it. **Nothing** for a victim of the killer's own player or of an ally
   (`Players::hostile` is false), nothing for scenery (it has no cost), nothing to a building
   and nothing to an untrainable unit. A victim of a player who is no player of the match
   counts as an enemy's (the neutral buildings of RA5). **Not modelled**: `Insignificant=yes`
   (walls and the tech buildings give their cost like anything else), and a unit destroyed by
   an area effect of `special.rs` that names no unit (a superweapon, a planted bomb, radiation)
   has no killer.
3. **Ranks.** Veteran when `value x 1000 >= cost x veteran_ratio_pm`, elite at twice that, both
   inclusive, `cost` being the unit's own type's; `veteran_cap` (2) is the highest rank.
   `SimEvent::RankGained { player, unit, rank }` once per promotion, at the rank reached.
4. **What a rank grants.** The abilities of `veteran_abilities` at rank 1, and also those of
   `elite_abilities` at rank 2; an ability counts once however often it is listed (the tank's
   elite list repeats three of its veteran list). `firepower`: `firepower_pm` = `combat_pm`
   (1100); `stronger`: `armour_pm` = `armor_pm` (1500); `rof`: `reload_pm` = `rof_pm` (600);
   `faster`: `speed_pm` = `speed_pm` (1200); `self_heal`; `sight` nothing (`sight_pm` is 0 in
   the ini: **assumption**, appendix D); `scatter` not modelled. An elite unit carries the
   elite weapons of its type in the places of the weapons it has by its type (a weapon a
   passenger gave an IFV stays).
5. **Where the multipliers act**, each the product of the rank's and the bonus's, in permille:
   * damage dealt and taken, in one function (`sim::impact_damage`, the plan's section 4.3):
     `D2 = floor(base x firepower_pm x share x versus / (1000 x whole x 100))`, at least 1 if
     `versus > 0`; `D3 = floor(D2 x 1000 / armour_pm)`; at most `max_damage`. The firepower is
     that of the attacker when it fired (a projectile carries it, hashed). **A hit of 1 on
     stronger armour deals 0**: the plan's equations apply the minimum before the armour
     multiplier, and so does the original; an order on such a target does not end by itself,
     as on an invulnerable one.
   * reload: `max(1, floor(reload x reload_pm / 1000))` ticks after a volley; the gap between
     the shots of a burst is not changed.
   * speed: `clamp(floor(speed x speed_pm / 1000), 1, MAX_UNIT_SPEED)` before the slowing of a
     damaged vehicle (`s - s div 4`), set where M9 sets a unit's speed every tick.
   * self healing, by a rank ability or by the type's `self_healing`: 1 hit point in every
     tick whose number is a multiple of the rules' `repair_interval` (19; **single source**),
     up to the type's hit points. The plan's item 1 says "by the infantry and unit repair
     rates"; its section 4.5, which the brief names, says 19 ticks, and that is what is done
     for every unit.
6. **The API for RA5 and RA6** (public in `sim`; they change state, so they may be called only
   from a system of `SimSchedule` or from a scenario setup between ticks, invariant I5):
   `promote(world, unit, rank) -> bool` raises a unit to at least `rank` (its experience
   becomes what the rank needs unless it has more), applies what the rank grants and writes
   `RankGained` (the veteran crate: `promote(world, id, rank + 1)`; veteran production after
   an infiltration: `promote(world, id, VETERAN)` when the unit leaves its factory);
   `set_bonus(world, unit, Multipliers { .. }) -> bool` sets the bonus multipliers, which
   multiply with the rank's (the armour, firepower and speed crates: read
   `Experience::bonus`, change one field, set it). Both return false for a unit that does not
   exist or is not trainable. The rules' `initial_veteran` (0 in the ini) makes every
   trainable unit start as a veteran.
7. **Passengers** keep their experience aboard (`Passenger::experience`, hashed if present)
   and their elite weapons when they leave.

### R48. RA4: crews and survivors

1. **The rule** (`crates/sim/src/crew.rs`; the plan's section 4.7). Whatever is destroyed
   (`remove_dead`) or sold (`SellBuilding`) in a tick is recorded, and at the end of the
   `Cleanup` stage, in the order of the records (the sales in command order, then the destroyed
   in id order), the `crewed` ones release their crew:
   * a vehicle that was not in a step: with `crew_escape_percent` (50) one unit of the crew
     type of its owner's side (`sides[..].crew`: GI, Conscript), in its cell or the first cell
     of the ring around it with room; an aircraft releases nobody (appendix D: `[ORCA]` is
     `Crewed=yes` and aircraft never spawn survivors);
   * a building: `max(1, ceil(refund / survivor_divisor))` units of that type, `refund` being
     `Definitions::refund` (cost x `refund_percent`) and the divisor that of the owner's side
     (500, 250), into the cells of its footprint in their order, as many as find room (three
     soldiers on foot to a cell); and from a factory of buildings (a construction yard) with
     `ENGINEER_ESCAPE_PERCENT` (25) one engineer, the first unit type with the role `engineer`.
2. **Decided here.** The side is that of the owner's country, not of the building's type: a
   captured Soviet barracks sold by an Allied player releases GIs by the Allied divisor (the
   ini's keys are per side of the house). An owner without a country has no crew. "Not while
   moving" is "not in a step" at the moment of destruction. The 25 percent of the engineer is
   a constant of the code: it is in no ini (hard coded in the original and its ancestor), as
   `BURST_GAP` is. The technician is not modelled (the plan).
3. **Randomness**: one draw from `SimRng` per crewed vehicle that stood still, one per
   construction yard; nothing else draws, so a world without crewed types has the sequence it
   had. `SimEvent::CrewEscaped { player, from, at, units }` names the new units; they are
   ordinary units (the cloning vats do not copy them: no `UnitSpawned`).
4. **I6'**: the new units are put through `spawn_unit`, which asks `Grid::room_in`; the tests
   run `assert_cells` after every release.

### R49. RA4: aircraft ammunition and docks

1. **Ammunition** (`crates/sim/src/flight.rs`). An aircraft with `ammo` n fires n volleys; a
   volley is the whole burst of its weapon (the Harrier's two Mavericks are its one round).
   With none left it does not fire and its attack order ends. Only an airport bound aircraft
   reloads.
2. **Docks.** A building with `docks` n holds n airport bound aircraft of its owner, each
   above a cell of its footprint: the corners first, then the other cells row by row
   (`sim::dock_cell`; the four docks of an Airforce Command are its four corners). An aircraft
   takes the nearest free dock of its owner when it has none (ties by the building's id) and
   keeps it while the building stands and is its owner's. **Whenever it has no order** (no
   move, no attack) it flies to its dock, and while it stands there it regains one volley
   every `reload_interval` (360) ticks; leaving the dock starts the round anew.
3. **Production.** `QueueProduction` of such an aircraft is refused with
   `RejectReason::NoFreeDock(item)` unless the docks of the player's buildings outnumber the
   aircraft of this kind it owns (`PlayerState::docked_aircraft`, as of the last `Economy`
   stage) and has queued. One that is complete stays in its factory while no dock is free
   (one was lost meanwhile). It leaves the factory like any unit and flies to its dock.
4. **Loss of the dock.** If its building is destroyed, sold or captured it takes a free dock
   elsewhere; with none it flies on while it has an order and is destroyed as soon as it has
   none (`UnitDestroyed`): the ini's `AirportBound`. **Consequence for tests and scenarios**:
   a Harrier put into a world without an Airforce Command of its owner is lost in its first
   tick.
5. **Assumptions and differences.** A docked aircraft is in the air to the rest of the
   simulation: anti air weapons engage it and ground weapons do not (R33 item 3 assumed that
   aircraft never land; the original's landed aircraft is a ground target). An idle aircraft
   on its dock fires at what comes into its range like every unit (`CanPassiveAquire=no` is
   not ported for any type). The M9 mechanism (the role `strike` with its rearm timer beside
   any building of the producer's group) remains for a world without a rules file, which is
   what `m9_arsenal.rs` asserts on the legacy fixture; no shipped type has the role any more.
6. **State**, all hashed under tags of their own and only when not empty: `SpecialState::ammo`
   (the volleys left, for aircraft below their full number), `reloading`, `docks`;
   `PlayerState::aircraft`.

### R50. RA4: spawned missiles and aircraft

1. **Real units.** A V3 rocket, a Dreadnought's missile, a Hornet and an Osprey are units of
   the air layer with the reference's strength, armour, speed and sight: they hold their cell,
   are seen, and are shot down by whatever engages aircraft. They are launched by
   `crates/sim/src/flight.rs` and move by the ordinary movement of the air layer. **Invariant
   I6' is not amended**: an air cell holds one aircraft, also a missile; the tests assert it
   on every tick of sorties and bombardments, and nothing jammed: a launched unit appears
   above its spawner or in the nearest free air cell, units are launched 27 ticks apart, and
   a returning aircraft is taken aboard within one cell of its spawner, so that none waits
   for the cell above it.
2. **The bay.** A spawner has `spawns_number` places (`SpecialState::bays`). Its launcher
   weapon (the arsenal's `launcher`) fires only while a place is ready; the shot deals
   nothing and launches every unit that is ready then, one every `SPAWN_LAUNCH_GAP` (27 ticks:
   the 20 frames of the original, plan section 4.8; a constant, in no ini) while the spawner
   keeps the target. A place whose unit is lost (shot down, or a missile that went off) is
   filled `spawn_regen` ticks later; one whose aircraft returned, `spawn_reload` ticks later.
   So a V3 Launcher launches every 533 ticks **plus the flight of its rocket**, where the
   plan's section 4.8 says "533 ticks between launches": the time runs from the loss of the
   unit (ModEnc describes `SpawnRegenRate` as the time to regenerate a lost spawn; single
   source), and the launcher's own reload (`ROF=150`, 200 ticks) is shorter than either.
3. **Missiles** fly to where the target was when they were launched (a unit's position, or the
   cell of a building nearest to the launcher) at their type's speed, **along the cells of a
   path of the air layer** and not in a straight line (up to 8 percent longer than the
   straight way; the plan: pitch and turn are not simulated), and go off there with the combat
   file's effect (`v3_rocket`: 200 through `[V3WH]`; `dmisl`: 300 through `[DMISLWH]`; the elite
   effects if the launcher was elite when it launched), **multiplied by the launcher's
   firepower** then (**assumption**: nothing documents whether `FIREPOWER` scales the rocket;
   the V3 and the Dreadnought have the ability). If its way ends elsewhere (its last cell is
   held) it goes off where it is. Shot down it deals nothing. A missile in flight outlives its
   launcher. It is removed without `UnitDestroyed` (`MissileDetonated`, and `Impact`).
4. **Carried aircraft** (Hornet, Osprey) attack the spawner's target with their own weapon
   and ammunition (one bomb), fly back when it is spent or the target is gone, and are taken
   aboard (`UnitRecovered`); they come back whole (**assumption**). They are launched with
   the rank of their spawner (an elite carrier's Hornets drop `[HornetBombE]`) and are
   destroyed with it. **Not carried**: `[HornetCollision]` and `[ASWCollision]`, their second
   weapons against aircraft (a launched aircraft attacks only what its spawner fires at,
   which is never an aircraft; this holds because a unit with a move order does not fire on
   its own, `combat.rs`, not by a rule of `flight.rs`: a change to idle fire would have to
   keep it, R56); listed differences.
5. **The Destroyer** launches its Osprey with its secondary weapon, which its
   `naval_targeting` 1 chooses against what is underwater (R34): the Osprey's `[ASWBomb]`
   (50 through `[APSplash]`) is what reaches a Typhoon; the ship's gun does not.
6. **The launcher weapons in the data.** `[V3Launcher]`, `[DredLauncher]`, `[HornetLauncher]`
   and `[ASWLauncher]` are ported with their own damage, range, minimum range, reload and
   burst. Three differences remain, each a row of `ra2_differences.tsv`: their warhead (the
   ini's `Special` is no warhead section; what a launcher may fire at is decided by the
   warhead of what it launches), and for the Destroyer's the range (the ini's -2, unlimited:
   the largest the combat file admits, 30 cells) and `anti_ground` (the ini's virtual
   projectile engages nothing). **Removed** from the differences: the seven rows of the
   stand-ins of R36 (damage, projectile speed and reload of the V3's and the Dreadnought's
   launcher, the damage of the Destroyer's) and the four "not in the data yet" rows.
7. **Credit and commands.** What a launched unit destroys counts for its spawner
   (`SpecialState::spawned`; a projectile of an aircraft that is gone when it lands is the
   spawner's too). A launched unit takes no command (`RejectReason::NotCommandable`).
8. **`CanPassiveAquire=no`** (V3, Dreadnought, carrier) is not ported: they fire at what
   comes into reach like every unit. Listed for RA9.

### R51. RA4: submerged units

1. **The rule** (`special::reconnaissance`; the plan's section 4.8). A unit of a `cloakable`
   type (Typhoon, Dolphin, Giant Squid) is hidden from a player who is an enemy of its owner
   unless a unit or building of **that player** with `sensors_sight` s stands within s cells
   (`dx^2 + dy^2 <= s^2` between the cells, to the nearest cell of a building), or the unit
   fired within the last `cloak_delay` (24) ticks (`SpecialState::surfaced`, hashed). Hidden
   is the one mechanism spies and Mirage Tanks use (`SpecialState::hidden`, per player): no
   weapon acquires the unit, an `Attack` command on it is rejected as on an unknown target,
   an attack order on it ends with `TargetHidden`, and the game does not draw it.
2. **Decided here.** Allies of the owner see it (the spy and the Mirage Tank of M9 are
   hidden from allies too; that is left as it was). Sensors serve only their own player, not
   its allies, as sight does. A hidden unit is still hurt by a blast that reaches it. The
   three cloakable types have sensors of 4 cells themselves, the Destroyer 8; the Psychic
   Sensor's `SensorsSight=15` is not ported (it has no `Sensors=yes`; RA7). `CloakingSpeed` is
   not modelled.
3. **The computer player** (`ai.rs`) does not see a unit that is hidden from it, and does not
   count a launched unit among its own (it is no army unit and its sight is not counted on).
   Unit test: `ai::tests::a_hidden_enemy_is_not_seen_and_a_launched_unit_is_not_the_players_to_command`.
   Its plans build no ship and no aircraft before RA8, so no match of the suite has a navy;
   a navy without a detector could not fight submarines (**debt for RA8**: the plans must
   include a Destroyer or a submerged type of their own where they build a navy).

### R52. RA4: the interface

1. **Rank insignia** (`crates/game/src/health.rs`): one chevron for a veteran and three for an
   elite unit, gold with a dark edge, under the right end of where the unit's health bar is,
   within the bar's width (also the short bar of a soldier, so that three soldiers in a cell
   keep their own), over every such unit that is shown, damaged or not. The hover text names
   the rank on a line of its own (`hover_info::rank_name`).
2. **A promotion** of one of the human player's units: a ping (`Sfx::Promotion`, two bright
   notes; synthesised, no asset file), a notice in the player's colour ("Grizzly Battle Tank
   promoted to veteran", `notices::promotion_text`), and a golden ring on the ground under the
   unit with a short glow (`weapon_fx::Painter::promotion`), the last for every promotion the
   player sees. The ping and the notice are for its own units only.
3. **Crews**: the survivors are units and appear where the simulation puts them; a notice says
   how many left a wreck of the human player's ("3 survivors escaped"). No parachute or
   climbing-out animation.
4. **Docks**: each dock of a building that is shown is marked by a small flat diamond above its
   cell, brighter while an aircraft holds it. An aircraft that stands on its dock is drawn
   landed (`units::aircraft_landed`), as M9's rearming aircraft was beside its base. The
   sidebar's card of an aircraft whose docks are all taken stays listed like a card at its
   build limit and says "No free dock: every dock is taken".
5. **Launches**: a launcher's shot is drawn as the flash and smoke of a launch at the
   launcher and nothing at the target (`Definitions::launcher_weapons`); the missile or
   aircraft is a unit with a view of its own. A missile's end is the burst of its
   `Impact`. Launched units cannot be selected (they take no commands).
6. **Art** (`assets/LICENSES.md`). New, original, from the existing generators: the sheets of
   the V3 rocket and the Dreadnought's missile (`scripts/hd_aircraft.py`: `v3_rocket`,
   `dred_missile`; rendered with Blender by `scripts/render_hd_blender.py` and composed by
   `scripts/finish_hd_art.py`). **Reused through the `art` alias**: the Hornet shows the
   Harrier's sheet and the Osprey the NightHawk's (the generator's table `ART`); models of
   their own were not worth a pass for aircraft a few pixels long.
7. **Submarines** are drawn only when seen: the unit views and the minimap already hide what
   `SpecialState::hidden` names for the human player, and the promotion ring respects it.
8. **Requires user verification**: that the chevrons are legible over every unit and do not
   crowd a cell of three soldiers; the ping, the ring and the notices in a busy battle (their
   frequency); the two missile sprites in flight and their size beside their launchers; the
   Hornets as Harriers and the Osprey as a NightHawk; the dock diamonds on the Airforce
   Command and the landed aircraft on its roof; that an aircraft's automatic flight home
   reads as intended; and how a submarine that surfaces to fire and vanishes again looks.

### R53. RA4: the computer player, and how long matches are now

1. **Honest vision and launched units** (`ai.rs`; R50, R51). A unit that is hidden from the
   computer player is none of the enemies it sees; a launched missile or aircraft is none of
   its units (not commanded, not counted as armed, its sight not counted on in
   `View::stays_visible`, since it is gone when it has done its work). Its plans build no
   aircraft and no ship before RA8, so no match of the suite has docks, launchers or
   submarines; crews and veterancy are in every match. `ai.ron` is unchanged, and no limit of
   a test was changed (`MATCH_TICK_LIMIT` is 72,000).
2. **The last stand** (`ai.rs`, `last_stand`; a fix of the end game, as the brief asks). With
   survivors that fight on, two players can spend each other: on the RA4 fixture the two
   brutal players stood from tick 4,400 on with a power plant, two bunkers and two idle miners
   against a factory without credits, and neither could ever change anything (no winner after
   72,000 ticks). The rule of D43 for a player that is broke (`sell_for_army`) never sells what
   its army needs, and does nothing for a player that can produce no army unit. Now a player
   that has no unit that fights and no harvester at work, and whose credits, units, buildings
   and queue progress have not changed for `LAST_STAND_PATIENCE` (1,200) ticks, sells its
   buildings, the dearest first, one each time it thinks (so with the shipped think
   intervals of 10 to 40 ticks a whole base goes within one turn; R56 corrects "one a
   turn"), and only in the turn of its player id
   (`LAST_STAND_TURN`, 256 ticks: two players never sell their last building in one tick,
   which would end a match without a winner). The survivors of crewed buildings are armed
   units that join its next wave; a player without a building is defeated in a short game. It
   stops when it has a unit that fights or a harvester at work again. This is close to the
   original's computer player, which sells its base and attacks with everything when it is
   beaten. Regression tests: `ra4_ai.rs::two_players_who_have_spent_each_other_make_their_last_stand_and_the_match_ends`
   (it fails without the rule: no outcome after 72,000 ticks) and
   `ai::tests::a_player_that_can_neither_fight_nor_earn_makes_its_last_stand`.
3. **A defect of the first version of that rule, found by the duels and fixed.** The picture
   of "nothing has changed" included the fraction of a step of progress that a queue carries
   from tick to tick while it cannot be paid, so a player with a soldier queued and no credits
   never stood still. One of the 756 duels then took 94,396 ticks against a limit of 72,000
   (Crossroads, Libya easy against Korea medium; traced with `m14_duels -- full trace
   crossroads`): from tick 45,000 on a single Conscript fired at four idle Chrono Miners, which
   since RA4 heal themselves (R47 item 5) and so took twice as long to wear down as the miners
   of R44 item 5, while Korea had two barracks, a war factory and no credits. Without the
   fraction in the picture Korea makes its last stand at tick 46,000 and the match ends in
   tick 55,159. The unit test above feeds the rule a changing fraction.
4. **Measured** (`m14_duels -- full`: the 756 pairings of `m6_production_matches.rs` with
   another seed; ticks; "limit" is that test's, `72,000 x span / 128`). Before: the validation
   of RA3 (`docs/validation/ra2-parity/ra3/10-duels.log`). After: the validation of RA4
   (`docs/validation/ra2-parity/ra4/10-duels.log`).

   | | Before RA4 | After RA4 |
   |---|---|---|
   | Length over the limit: median, 90th percentile, longest | 0.16, 0.33, 0.96 | 0.17, 0.37, 0.95 |
   | Matches above 0.5, 0.7, 0.8 of their limit | 25, 3, 1 | 33, 9, 4 |
   | Small maps (span up to 128): median of each map | 20,855 to 26,320 | 22,440 to 28,292 |
   | Small maps: longest | 69,221 (Riverlands, America against Korea, both brutal) | 57,054 (Crossroads, Korea against France, both easy: 0.79) |
   | Large maps: median of each map | 13,096 to 51,825 | 16,114 to 61,399 |
   | Large maps: longest | 167,856 (Iron Isthmus; limit 270,000: 0.62) | 274,692 (Thousand Lakes, France against Germany, both brutal; limit 288,000: 0.95) |
   | All 756 matches together | 30.3 million ticks | 32.4 million ticks |
   | Matches over the limit, matches without a winner | 0, 0 | 0, 0 |

   The bulk of the matches is as long as it was (the median moved from 0.16 to 0.17); the
   tail is heavier. The matches near their limits are wars between players of the same
   difficulty on the large maps, in which both keep their thirteen buildings and ten to twenty
   units for a hundred thousand ticks and more, each spending what its miners bring on waves
   that the other's army and pillboxes stop; they end when one side's ore is used up. Traced:
   Thousand Lakes, France against Germany, both brutal. What RA4 adds to such a war: every
   building that falls gives its owner one to six soldiers at the spot, units that survive a
   fight come back stronger and heal, and miners heal themselves. The match that R44 item 5
   traced at 0.96 (Riverlands, America against Korea) now ends at 0.43 (tick 30,778; this
   run was not traced).
5. **Debt for RA8** (and for RA5, when ore grows back and such a war no longer ends by
   itself): a computer player in a balanced war should gather larger waves instead of sending
   what it has when its patience runs out; a wave that sees no armed enemy should go for the
   buildings (it still fires at what is nearest, R44 item 5).
6. **Benchmark, soak** (`docs/validation/ra2-parity/ra4/`; budget: mean 5 ms a tick, longest
   tick 50 ms): the benchmark on the shipped data has a mean tick of 0.218 ms and a longest of
   5.237 ms (0.209 and 5.749 after RA3), on the fixture 0.094 and 0.787; the soak plays its 260
   configurations (18,720,000 ticks) in 2,769 s (2,677 after RA3) with a peak heap of 214.5 MiB
   of 1,024, and I6' holds on every tick. The state limits of the soak (`MAX_UNITS`,
   `MAX_PROJECTILES`) were not raised, and the soak passes with them as they were.

### R54. RA4: tests adapted (none deleted, ignored or weakened)

**Schema migration** (the assertions are unchanged; the arsenals of the fixtures are at schema
5 with the neutral value in every new field, the combat files at schema 3):

| Test | What changed |
|---|---|
| `ra1_definitions.rs` (helper `arsenal_of`, the text edits of four tests) | the header they match and write is `(schema_version: 5, entries: [` |
| `ra1_definitions.rs::other_schema_versions_are_rejected_in_every_file` | the arsenal rejects 4, 3, 2, 1 and 6 against 5 (one case more) |
| `ra1_definitions.rs::every_field_of_the_arsenal_file_is_required` | also removes each of the sixteen new fields in turn; `infantry` is no longer the last field of an entry, so its text edit ends with the comma and that of `sensors_sight` begins with it |
| `ra2_definitions.rs::the_fixture_loads_with_the_traits_of_schema_version_3`, `ra3_definitions.rs::the_fixture_loads_with_the_traits_of_schema_version_4` | assert `ARSENAL_SCHEMA_VERSION == 5`, which keeps the traits they test |
| `m4_definitions.rs::schema_versions_other_than_2_are_rejected` | renamed `..._other_than_3_...`: rejects 0, 1, 2, 4, -1 and the two extremes against 3 (one case more); the helper `generated()` writes schema 3 |
| `ra9_data_parity.rs` | extended: the sixteen fields of schema 5 of every mapped item and the four missile effects against the reference; the spawned types are audited like every type (they were "not in the data yet") |

**Expectation or setup changed because RA2 behaviour is intended** (one line each, as the
brief asks; `m9_arsenal.rs` itself runs on the legacy fixture, which has no rules file and so
keeps M9's aircraft, carrier and naval rules: unchanged, all 24 tests):

| Test | What changed and why |
|---|---|
| `ra3_shipped.rs` (helper `first_shot`) | a Harrier is lost without a dock (R49): as the attacker it stands on the first dock of an Airforce Command of its owner, as the silent target it has an Airforce Command far away. The assertions (which weapon, what damage, or no shot) are unchanged |
| `ra3_shipped.rs::shipped_ships_follow_their_naval_targeting` | the Aegis's target over open sea is a Kirov instead of a Harrier (both `light`; over open sea a Harrier has no dock): the same weapon and the same 100 damage. The Destroyer's hit on a Typhoon is still `asw_launcher` and 50, now dealt by its Osprey's depth charge (R50); only the comment says so |
| game `weapon_fx::tests::every_shipped_weapon_has_the_class_its_name_promises` | three rows more (`hornet_bomb`, `hornet_bomb_e`, `asw_bomb`: bombs) |
| game `audio::tests::events_play_only_what_the_human_player_may_know` | assertions added for a promotion (heard for the human player's own unit only) and for the silent launch and crew events |

**Compile migration**: game `audio.rs` and `units.rs` match the five new events;
`Sfx::ALL` has 27 entries (the test that synthesises every sound covers the new one).

**New in RA4** (sim): `ra4_definitions.rs` 8 (the traits loaded, each range and combination
rejected, hashing, the missile effects); `ra4_veterancy.rs` 20 (thresholds inclusive and one
credit below, each multiplier measured on damage dealt and taken, reload ticks and the ticks of
a way, self healing, the elite weapon, each ability once, splash, crushing and buildings as
kills, allies, own units, untrainable units and buildings, the API, passengers, a long battle
twice, hashing, a world without rules); `ra4_crew.rs` 10 (the rates over 400 seeds, the side,
moving and uncrewed vehicles with no draw, the nearest cell and no cell, the counts of every
case, sales, the limit of room, the engineer, a battle twice); `ra4_aircraft.rs` 14 (the dock
check at its boundary, production that waits, volleys and reloading at their ticks, no
ammunition, leaving the dock, the loss of the dock in every form, aircraft without the traits,
sorties twice with I6' on every tick, hashing); `ra4_spawns.rs` 14 (launch, flight, impact and
its numbers, the time to replace, a missile shot down, veteran and elite launchers, credit,
the moving target and the dead launcher, minimum range and commands, the carrier's three
aircraft, their return, loss and replacement, their end with the carrier, elite aircraft, the
boat's hunter, a bombardment twice with I6' on every tick, hashing); `ra4_cloak.rs` 8 (hidden
and detected at the boundary of the sensors, sensors of cloaked units and buildings, the 24
ticks after a shot, commands and weapons on a hidden unit, the end of an attack, allies and
third parties, a type without the trait, hashing); `ra4_shipped.rs` 16 (the Grizzly's ranks
and multipliers, the shipped abilities, the Apocalypse's healing, the crews of the base units
over 120 seeds, the survivors of six buildings, the fifth Harrier, its one volley and its 360
ticks, Harriers lost with their Airforce Command, the V3 rocket's flight, damage and pace and
the elite rocket, a rocket shot down by a Patriot, the Dreadnought's two missiles, the
carrier's three Hornets and the 800 ticks to replace one, the Typhoon unseen at 5 cells and
seen by a Destroyer at 8, the Osprey, the 24 ticks after a torpedo); `ra4_ai.rs` 2 (the last
stand of two players who have spent each other, and matches of computer players on the RA4
fixture that end with a winner; R53); unit tests in `sim`: `veterancy` 3, `flight` 2,
`crew` 1, `ai` 2. Game: `health` 1, `notices` 2, `hover_info` 1, `units` 1, `sidebar` 1.
Together 106: 787 tests before RA4, 893 after.

### R55. The final review within three hours: a panel, and advisory reviews during the run (user request, 2026-10-12)

While RA4 was being implemented the user asked: "Can you speed up the review process to make sure
that all the review can be done within 3 hours." The prompt's section 7 describes one reviewer
who reviews the whole diff and reruns every validation command; the diff was 45,000 lines of code
and data at RA3 and the validation commands take about two hours, so one reviewer cannot do it in
that time. The orchestrator therefore arranged the following (its interpretation of the request;
the user was told and can veto):

1. **The binding review is a panel.** Five independent reviewers with fresh contexts, none of
   whom implemented or remediated anything, each review one slice of the complete cumulative
   diff (the simulation of RA1 to RA3; of RA4 to RA7; data, ported values and provenance; test
   integrity and acceptance criteria; the game client, the computer opponent, documents and
   regressions) for at most 120 minutes. The orchestrator starts the rerun of every validation
   command at the same moment. A lead reviewer, also independent, then checks the validation
   logs, verifies every P0 and P1 of the slices, writes `docs/reviews/RA2-parity-review-N.md` and
   makes the binding decision by the rules of the prompt's section 7. Everything section 7
   requires of the reviewer is still done; it is divided among six sessions.
2. **Advisory reviews during the run.** Independent reviewers examine committed milestones on a
   read only snapshot while later milestones are implemented, so that defects are found and
   fixed before the binding review, which is what keeps it to one round. They are advisory: they
   gate no milestone (the validation commands remain the only gate) and approve nothing; their
   reports are `docs/reviews/RA2-parity-advisory-<n>-<slice>.md`, and a remediation agent treats
   each finding as a claim, as section 7 item 2 prescribes for the binding review. This is a
   departure from the prompt's "without any review between milestones", made on the strength of
   the user's later request.
3. **What cannot be promised.** If the binding review says NO-GO, section 7 requires a
   remediation and a new review of the full diff with the validation rerun: each such round
   adds the time of the fixes and up to three more hours.

Advisory review 1 (RA1 to RA3, at commit `46e694d`, three slices, about 17 minutes of wall
time): 1 P0, 9 P1, 24 P2 and 7 suspected items; see the three reports and the remediation's
decision record.

### R56. Remediation of advisory reviews 1 and 2 (RA1 to RA4)

Advisory review 1 (R55: three slices on the snapshot of `46e694d`, RA1 to RA3: 1 P0, 9 P1, 24 P2
and 7 suspected items) and advisory review 2 (two slices on the snapshot of `6a41bc3`, RA4: 1 P0,
8 P1, 14 P2 and 5 suspected items; `docs/reviews/RA2-parity-advisory-2-sim.md` and `-tests.md`)
were remediated together by one agent that wrote none of the reviewed code and is no reviewer.
Every finding was treated as a claim: reconciled against the code at `ba26de1`, reproduced
(the reviewers' probe files, or a new test run against the code before the fix), then fixed or
refuted. "Fails before" below means that the named test was run against the unfixed source and
failed; the logs of those runs are not committed. **The fixes and the verdicts of this record
have not been checked by an independent reviewer**: they are the remediation agent's own.
Validation: all thirteen commands exit 0 on commit `f4a9aa5`, 925 tests
(`docs/validation/ra2-parity/advisory-1/`; `docs/STATUS.md`). No match of computer players
changed: the 756 duels end in the same ticks as in RA4's validation, since the computer player
meets none of the changed paths (it fires no superweapon, flies no aircraft, launches and hides
nothing).

**Of the 69 items:** 46 fixed (both P0; 15 of the 17 P1, one of them in part: the shipped
counterpart of the legacy mechanics tests, whose specialist part waits for RA6); 1 P1 settled
by a recorded decision (the soak, decision 6); 14 deferred (1 P1, `Soylent`, as a listed
difference for RA5, decision 7; the others P2 or suspected, as debt); 8 refuted, found to be no
defect, not a product finding, or not verifiable. The table of each review follows the
decisions.

#### Decisions

1. **What a command of the tick destroyed is gone for the commands after it** (review 1 sim
   P0-1, and its suspected item 1). A command can take the last hit points of a unit or
   building: the nuclear strike lands in the `Commands` stage (the only such command today; the
   computer player issues none). `Cleanup` removes the dead at the end of the tick, and until
   then it was an ordinary unit or building for every later command, so it could be deployed,
   undeployed (both gave it one hit point back and so kept it alive), sold for its refund,
   moved, embarked, repaired, fired as a superweapon, and counted as a prerequisite, a factory
   and an anchor of a placement. The fix is at the indexes, not at the three reported symptoms:
   the units and buildings a command may name or rely on are those with hit points left
   (`EconomyParams::alive` for the unit index and the `BuildingIndex` of the regular commands;
   `special::living` for the actors of `Deploy`, `Undeploy`, `Interact` and `Superweapon`). A
   command that names such a unit or building is rejected as for an id that names nothing
   (`UnknownUnit`, `UnknownBuilding`, `UnknownTarget`). The same root cause in the `Economy`
   stage of that tick: an order waiting to be carried out (`SpecialState::interactions`: an
   engineer that arrives, a unit that reaches its transport or depot) is dropped when either
   side has no hit points left (before, the capture gave a destroyed building all its hit
   points and a new owner), and the repair of an IFV that carries an Engineer adds nothing to a
   vehicle without hit points. Not changed: in the stages between `Commands` and `Cleanup` of
   that one tick the destroyed building still counts for power and production, and a destroyed
   unit still moves and fires, as everything that a missile or a radiation zone destroys in the
   `Economy` stage does (debt, below).
2. **One rule for what a rejection may tell** (review 1 sim P1-1, P2-4; review 2 sim P1-3,
   P1-4). The rule since M4 (D30, D33) is that a command naming something of another player
   that the issuing player does not see is rejected exactly as for something that does not
   exist. It now holds for every command: `special::own` (used by `Deploy`, `Interact`,
   `Superweapon` and the unit list of a Chronosphere) and `command::validate` (the unit
   commands) answer `UnknownUnit` unless the player sees a cell of the unit or building **and**
   it is not in `SpecialState::hidden` for that player (a submerged unit, RA4); `Attack` on a
   hidden unit goes through the ordinary checks (`NoUnits`, `NotArmed`, ...) before
   `UnknownTarget`, so that an attack by no unit no longer tells a hidden unit anywhere on the
   map from nothing; `Deploy` asks whether the player sees the footprint before it looks at
   it, as `PlaceBuilding` does. One test walks every command variant that names a unit or
   building against a missing id, an unseen unit, an unseen building and a hidden unit and
   compares the answers (`ra1_commands.rs`). R27 item 1 described the old behaviour and is
   corrected in place.
3. **Ammunition: a round is a volley, spent when the volley begins** (review 2 sim P0-1;
   amends R49 item 1). `flight::fired` runs for every shot, whatever it is fired at (it ran
   only for a shot at a unit or building, so shots at scenery cost nothing, gave a launcher's
   bay no target and did not surface a cloaked unit). A round is taken with the **first** shot
   of a volley, and an aircraft whose last round is spent finishes the volley it has paid for
   and then turns for home. **A volley cut short costs its round** (its target is destroyed by
   the first shot and nothing else is in reach when the next is due): the reference gives the
   Harrier `Ammo=1`, one attack run per sortie, and a run that was begun is used up. No source
   says what the original does with a burst that loses its target; this is the closest reading
   (**assumption**).
4. **A launcher reaches scenery; a bay goes aboard with its spawner** (review 2 sim P1-1,
   P1-2; amends R50 items 2 and 3). A launcher ordered onto scenery launches at the cell the
   scenery stands in (a missile's aim is a point; a launched aircraft is given the same attack
   order), instead of firing its launching shot for ever. The bay of a spawner that boards a
   transport is kept with the passenger (`Passenger::bay`, hashed): what it had out counts as
   lost from then on, each place to be filled after the type's `spawn_regen`, and the timers
   stand still aboard (**assumption**: nothing is reloaded in a transport); before, the bay
   was dropped and the spawner had everything again when it left.
5. **Smaller rules of RA4** (review 2). The unit list of a Chronosphere refuses a launched
   missile or aircraft (`NotCommandable`; suspected item 4, confirmed). A crusher gains
   nothing for driving over a unit whose hit points were gone already (suspected item 1,
   confirmed with a strike of the same tick). `QueueProduction` of an aircraft counts the
   docks of the buildings that stand in its tick, not those of the last `Economy` stage (sim
   P2-1). An arsenal whose `missile` has an effect the combat file lacks, plain or elite, is
   rejected (`UnknownReference`, kind `effect`, at `entries[n].missile`; tests P1-4; amends
   R46 item 2). The last stand of a player with credits that stand still (a finished building
   without a site) is the rule's intent, not a wrongful sale: what counts is that nothing the
   player has changes (suspected item 2; the unit test of the rule says so now).
6. **The soak stays at 36,000 ticks, and the late game is checked where matches are played
   out** (review 1 tests P1-2). PLAN.md section 7 defines the soak as 36,000 ticks, so its
   length is not a weakened test; what it covers shrank when the pace of the matches halved.
   Counted from the validation logs, of the 252 shipped configurations those in which no match
   ended within 36,000 ticks, and the matches ended: RA0 91 and 317; RA1 60 and 339; RA2 110
   and 243; RA3 177 and 95; RA4 189 and 78. Doubling `SOAK_TICKS` would add about 46 minutes to
   every validation. Instead the 756 matches of `m6_production_matches.rs`, each played to its
   end, now assert on every tick that no player's credits are negative (I7) and on every
   sixteenth tick and the last that `sim::cell_invariant_violations` and `sim::stray_claims`
   find nothing (I6'), besides the rejection rule they always checked. Not made good: the
   comparison of two runs' hashes over the late game of the shipped matches (the soak compares
   it over its 36,000 ticks; `m6_ai.rs` and `ra4_ai.rs` compare whole matches on fixture
   data), and the state limits. Debt. The test run of the validation took 3,263 s with these
   checks and the new tests, against 3,137 s at RA4.
7. **`Soylent` is a listed difference until RA5** (review 1 data P1-3). The reference gives
   the two refineries `Soylent=300`, and the plan's research (appendix D, topic 9; a single
   source, ModEnc) says a non-zero value replaces the computed refund, so a sold refinery
   returns 300 credits in Red Alert 2 and 1000 in the game, which lets a player keep the free
   miner for 1000 credits net. It is **not applied here**: the refund of a type also sets the
   number of survivors of every sold or destroyed building of that type (R48), so applying it
   changes every match of computer players, and it needs a per type value that the rule
   `refund_percent` of a fixture cannot express (four fixture tests assert the rule with 40
   percent against a `sell_value` of 50). It belongs with RA5, which changes the schema of
   `economy.ron` for the miners and refineries. The audit now recomputes the refund with
   `Soylent` and the two rows are in `ra2_differences.tsv`; the Ore Purifier's `Soylent=1250`
   equals half its cost. **For RA5:** give the type its refund (`sell_value` as the override
   of the rule), decide with the research whether survivors follow it, and delete the two rows.
8. **The pipeline is gated** (review 1 data P1-1, P2-6). `scripts/validate_ra2_parity.sh`
   runs `generate_ra2_data.py --check` and `ra2_reference.py --check` as commands 12 and 13;
   the eleven commands of the prompt keep their numbers. The reference script records the
   SHA-256 of the table it writes (`ra2_reference.tsv.sha256`), and `--check` compares the
   committed table with it before anything else, so a table edited by hand fails also on a
   machine without the ini files; there the comparison with the ini files is skipped with a
   notice and the command passes.
9. **The audit covers every field of an item** (review 1 data P1-2, P2-1, P2-3). Added: the
   free unit and `Refinery` of a building, the producer of every item, the arsenal's factory
   group, side, domain, category and capacity, power as two numbers, the refund with
   `Soylent`; a row is required for the game's own damage of the effects `c4` and `radiation`
   (reference value `-`); and `ITEM_FIELDS` lists every field of the entries of units and
   buildings in the four shipped files as audited (with the field name of the audit, which
   must have been compared) or as the game's own with a reason: a field that is in a shipped
   entry and not in the list, or the reverse, fails. Eleven of the twelve mutations of the
   review fail the audit now, and four more of the same kind; the twelfth changed the ore
   units of the miners and refineries, which are the game's own until RA5 and are on the list
   as such. New rows: Tanya's domain (her amphibious movement zone is not
   modelled), the two effects, the two refunds. "Infantry on foot" is `[InfantryTypes]`
   everywhere (the generator's and the audit's speed factor used `Category=Soldier`; the two
   agree for all mapped sections, and `units.ron` changed in its comments only).
10. **Rule values that no code reads** (review 1 data P2-2): `production.build_ticks_per_1000`
    and `wall_build_time_pm`, `combat.min_damage`, `ballistic_scatter`, `homing_scatter` and
    `ivan_damage` of `rules.ron` are the reference's values as the generator has already
    applied them (to the build times of `economy.ron`, the scatter of each weapon, the effect
    `ivan_bomb`; the minimum damage of 1 is PLAN.md's equation 2). They are loaded, range
    checked, hashed and audited, and the simulation reads the applied values: a rules file of a
    fixture cannot change them. Recorded as it is; debt.
11. **The Service Depot under low power** (review 1 sim, suspected item 2): `[GADEPT]` and
    `[NADEPT]` have no `Powered` key in the reference, so by the plan's rule (section 4.2: a
    building stops working when it is `Powered` and drains power) they keep repairing. The two
    shipyards are `Powered=true` and repair ships (R27 item 4); a shipyard's production only
    slows under low power, and no source says that its repair stops, so it does not
    (**assumption**). No change.
12. **The tick limit and its justification** (review 1 tests P1-3, P1-4). R29 measured the
    duels (seed 14), not the test's own matches (seed 0); it is corrected in place, and R31
    says that the raised limit took the failing condition from the RA1 regression test of the
    sweep order. Measured with the test's seed at RA4: that match (Crossroads, Libya against
    Iraq, both easy) takes 45,345 ticks with the rule and 53,431 without, both inside 72,000.
    The failing condition is restored where the rule is:
    `sim::ai::tests::a_sweep_starts_at_the_waypoint_nearest_to_an_enemy_start` fails without
    it. No bound on the match's length was added to the end to end test: such a bound would
    have to be measured anew with every change of the rules. `m6_production_matches.rs` now
    writes the length of every match with its limit to
    `target/tmp/m6_production_matches_lengths.tsv`, so each run measures its own margin.
    Measured in the validation of this remediation (`advisory-1`; the test's seed 0, all 756
    matches): length over the limit median 0.17, 90th percentile 0.37, longest 0.95 (Thousand
    Lakes, France against Germany, both brutal: 274,693 of 288,000 ticks); 32, 9, 2 and 1
    matches above 0.5, 0.7, 0.8 and 0.9 of their limits; on the small maps the longest takes
    57,055 of 72,000 (Crossroads, Korea against France, both easy); 32.4 million ticks in all.
    Against the duels of RA4's validation (seed 14): 691 of the 756 pairings take exactly as
    long with both seeds, and 65 take another course (the widest apart: Oasis Gulf, Great
    Britain against Russia, both easy, 66,468 ticks against 40,410). So the tables of R29, R38,
    R44 and R53 describe the test's matches closely, not exactly, and the match at 0.95 of its
    limit (debt of R53 item 5) is the test's own as well.
    `MATCH_TICK_LIMIT` was not changed.

#### Tests adapted (none deleted or ignored)

| Test | What changed and why |
|---|---|
| `ra4_aircraft.rs::an_aircraft_fires_its_volleys_returns_and_reloads_a_round_at_a_time` | it waited for the ammunition to drop and then counted two shots; the round now drops with the first shot (decision 3), so it asserts one shot at that moment and waits for the end of the volley for the two. The counts of shots, the damage and the reload are unchanged |
| `ra4_shipped.rs::a_harrier_fires_one_volley_and_reloads_it_on_its_dock` | the target is taken away before the reload is timed: the reloaded Harrier fires at a target in reach in the tick in which it has its round again, and the round is now spent with that first shot. The 360 ticks and both assertions on the ammunition are unchanged |
| `ra4_definitions.rs::the_terms_of_a_spawner_must_fit_together` | the arsenal with `missile: "dmisl"` on the rocket was asserted to be accepted with a combat file that lacks the effect; it is now asserted to be rejected (decision 5), accepted with both effects listed, and rejected with one |
| `ra4_definitions.rs::every_trait_of_schema_version_5_is_hashed` | its `missile` row changes `"v3"` to `"none"` instead of `"dmisl"`, which the fixture's combat file no longer admits; the assertion (another hash) is unchanged |
| `ra3_shipped.rs` (helper `first_shot`) | for a cloakable target a silent Destroyer of the attacker's player lies in sensor range, and the helper asserts on every tick that the target is not hidden (review 2 tests P1-1); every expectation is unchanged. This is the row R54 lacked: since RA4 the two "never at a submarine" cases had passed because the Typhoon was hidden |
| `ra4_shipped.rs::only_the_base_units_have_a_crew_and_it_is_of_their_owners_side` | an airport bound aircraft stands on an Airforce Command and is asserted to be destroyed in the tick whose events are read (the Harrier row could not fail: the aircraft was lost a tick earlier); the Black Eagle is added |
| `ra4_shipped.rs::the_survivors_of_shipped_buildings_follow_the_refund_and_the_divisors`, `a_fifth_harrier_is_refused_and_built_after_a_second_airforce_command_stands` | stricter: two rows with quotients that are not whole (Tesla Reactor 2, Ore Purifier 3); the rejection is asserted with its reason |
| `ra9_data_parity.rs::the_conversions_round_half_away_from_zero_in_integers` | `speed` takes whether the section is in `[InfantryTypes]` instead of reading `Category`; the five expected speeds are unchanged. The audit's field `power` became `power_produced` and `power_consumed` (no row of the differences used it) |
| `ra2_production.rs::a_player_id_outside_the_match_changes_nothing`, `sim::ai::tests::a_player_that_can_neither_fight_nor_earn_makes_its_last_stand`, game `tests/m6_production_matches.rs` | assertions added (the unchanged state hash; credits that stand still; I7 and I6' in every match) |

New: `ra1_commands.rs` 16, `ra1_catalogue.rs` 1, `ra2_charging.rs` 2, `ra3_weapons.rs` 1,
`ra4_aircraft.rs` 4, `ra4_spawns.rs` 2, `ra4_cloak.rs` 1, `ra4_veterancy.rs` 2,
`ra4_shipped.rs` 1, `sim::ai` 1, game `minimap` 1: 32 tests, 893 before and 925 after.

#### Advisory review 1 (RA1 to RA3)

| Id | Claim | Verdict | Evidence | Test |
|---|---|---|---|---|
| sim P0-1 | a yard or base unit destroyed by a command of the tick lives on through `Undeploy` or `Deploy`; a destroyed building is sold | **fixed** (decision 1) | reproduced with the review's probe on `ba26de1` (the yard a unit with 1 hit point, the base unit a yard, the sale paid 800); nine of the new tests fail before, their three controls pass | `ra1_commands.rs`: the eight tests `a_..._destroyed_by_a_strike_...` and `a_destroyed_building_is_no_prerequisite_and_no_anchor_in_the_same_tick` |
| sim P1-1 | `Deploy`, `Interact`, `Superweapon` tell an unseen enemy from a missing id | **fixed** (decision 2) | reproduced with the probe (`NotOwned` against `UnknownUnit`); fails before | `ra1_commands.rs::every_command_answers_for_an_unseen_enemy_as_for_an_id_that_names_nothing`, `a_seen_enemy_is_reported_as_not_owned`, `the_chronosphere_answers_for_an_unseen_enemy_vehicle_as_for_an_id_that_names_nothing` |
| sim P2-1 | the results of `Grid::claim` are dropped in four places | **fixed** | teleport, undeploy, unload and `spawn_unit` handle a refusal (the unit keeps its cells, the yard stays, the passenger stays aboard, no unit is spawned) | none possible: the checks before each call are the grid's own, so the refusal cannot be provoked |
| sim P2-2 | the tail of the grid's hash has no lengths | **fixed** | the two counts are written when there are units on foot; without any the hash is what it was before RA3, which `map::tests::the_hash_covers_the_spots_and_is_unchanged_without_units_on_foot` asserts (an unconditional count failed it) | that test; no two reachable grids collided before |
| sim P2-3 | a soldier on a diagonal step is for some ticks in a cell it does not hold | **fixed** (record) | R6 item 5 says so now, with why no impact misses it | - |
| sim P2-4 | `Deploy` looks at the footprint before it asks whether the player sees it | **fixed** | real, but not reachable through a Gap Generator as the review thought (a unit's own sight counters it, `special::reconnaissance`): only with a base unit that sees less than its footprint; fails before | `ra1_commands.rs::deploying_names_nothing_in_a_footprint_the_player_does_not_see` |
| sim P2-5 | a volley is kept while its unit is disabled or rearming | **fixed** | fails before (`burst_left` 2 after the due tick) | `ra3_weapons.rs::a_volley_whose_next_shot_is_due_while_the_unit_is_switched_off_ends` |
| sim S1 | embarking in the tick of death | **fixed** with P0-1 (confirmed) | fails before: the soldier went aboard | `ra1_commands.rs::a_unit_destroyed_by_a_strike_does_not_embark_and_a_destroyed_transport_takes_nobody` |
| sim S2 | Service Depot and low power | **no defect** (decision 11) | `[GADEPT]`, `[NADEPT]` have no `Powered` key (`ra2_reference.tsv`) | - |
| data P1-1 | nothing in validation checks generator and reference table | **fixed** (decision 8) | a table edited by hand exits 1 with and without the cache; a shipped file left edited by a failed script of this remediation was reported stale by the generator's check | commands 12 and 13 |
| data P1-2 | the audit misses ported fields and has no completeness guard | **fixed** (decision 9) | the review's mutations rerun: eleven of twelve fail the audit; the twelfth changes values the list names as the game's own | `ra9_data_parity.rs` (`ITEM_FIELDS`) |
| data P1-3 | `Soylent` of the refineries is not applied | **deferred**, listed as a difference (decision 7) | two rows of `ra2_differences.tsv`; the audit recomputes with `Soylent` (the mutation to 300 fails as a stale row) | `ra9_data_parity.rs` |
| data P2-1 | two tests of "infantry" in the pipeline | **fixed** | the type list in the generator and the audit; `units.ron` differs in comments only | `ra9_data_parity.rs` |
| data P2-2 | rule values that no code reads | **deferred** as debt (decision 10) | - | - |
| data P2-3 | power audited as one net number | **fixed** | the mutation 200/0 to 250/50 fails | `ra9_data_parity.rs` |
| data P2-4 | a reason in the differences that the data contradicts | **fixed** | no shipped infantry has more than 200 hit points; the warhead deals 100 percent to their three armours | - |
| data P2-5 | five statements of the pipeline's documents | **fixed** (four), one left | generator header, plan section 3.1, the animation identifiers of `[Powerups]`, the unused constant; the comment "The arsenal is at 3 since RA2" in `ra1_definitions.rs` stands (debt) | - |
| data P2-6 | `ra2_reference.py --check` passes without the cache | **fixed** (decision 8) | the recorded hash is checked first | command 13 |
| data P2-7 | game owned values that will be differences (footprints, superweapon recharge, harvesters) | **deferred** as debt for RA5, RA6 and RA9 | `ITEM_FIELDS` names each as the game's own with its milestone; the carrier's capacity is gone since RA4 | - |
| data P2-8 | the elite fallback is asserted, not audited | **fixed** (record) | labelled an assumption in R34 | - |
| data S-1 | `JumpjetSpeed` read as leptons per frame | **not verifiable**; stays the labelled assumption of R7 and plan 4.1 | no source gives the unit | - |
| tests P1-1 | the smoke check "the minimap follows the radar" cannot fail | **fixed** | with the map drawn without radar the new test and the smoke test fail ("the minimap does not follow the player's radar", exit 1) | game `minimap::tests::the_drawing_is_the_map_with_radar_and_the_offline_screen_without`; the smoke test |
| tests P1-2 | the soak covers less of a match since RA2 | **decided** (decision 6) | the counts of five validation logs | `m6_production_matches.rs` (I7, I6') |
| tests P1-3 | the RA1 regression test lost its failing condition | **fixed** (decision 12) | confirmed: 53,431 ticks without the rule; the unit test fails without it | `sim::ai::tests::a_sweep_starts_at_the_waypoint_nearest_to_an_enemy_start` |
| tests P1-4 | R29's limit rests on other matches than the test's | **fixed** (decision 12) | R29 and R31 corrected; the test's own lengths measured | `m6_production_matches_lengths.tsv` |
| tests P1-5 | the legacy mechanics tests have no shipped counterpart | **fixed** for the catalogue, charging and sale; the rest **deferred** to RA6 | the review's two probes adopted; deferred: loading and unloading transports, cloning, mind control, the spy's effects, bombs and C4 on shipped data, which RA6 replaces by the mechanics of Red Alert 2 and tests then (plan, RA6) | `ra1_catalogue.rs`, `ra2_charging.rs` |
| tests P2-6 | a "captured" building is set up directly in two tests | **deferred** as debt | the capture itself is tested on fixtures (`ra1_buildability.rs`, and now `ra4_aircraft.rs` and `ra1_commands.rs`) | - |
| tests P2-7 | the "property test over costs and rates" tests one cost | **fixed** | the probe adopted: every shipped unit, 1 to 4 factories, with and without power | `ra2_charging.rs` |
| tests P2-8 | the low power test has no positive control | **fixed** | the probe adopted | `ra2_charging.rs::the_prism_tower_fires_with_power_in_the_scenario_of_the_low_power_test` |
| tests P2-9 | silent fallbacks in the validation examples | **fixed** | `m14_duels` and `m7_soak` fail on a side without a country | - |
| tests P2-10 | stale comments in `m6_production_matches.rs` | **fixed** | - | - |
| tests P2-11 | a test name promises more than it asserts | **fixed** | the state hash is compared | `ra2_production.rs::a_player_id_outside_the_match_changes_nothing` |
| tests P2-12 | the smoke flow drives no input and compares less than the README says | **fixed** for the README; the rest **deferred** as debt | no `Interaction::Pressed` in `smoke.rs`; game speed and difficulty are not compared | - |
| tests P2-13 | card cost labels are those of the first match of a session | **deferred** as debt (not reproduced: the interface was not driven) | - | - |
| tests P2-14 | the headless matches play without scenery | **deferred** as debt; the test's header says so | - | - |
| tests P2-15 | R43 item 2 against the code | **fixed** | R43 corrected in place | - |
| tests P2-16 | the shared snapshot was changed during the review | **no product finding** | process of the review | - |
| tests S1 | yard starts with "MCV start" off are not swept over all maps | **deferred** (not probed) | - | - |
| tests S2 | the sidebar's hint counts disabled factories | **deferred** (not probed) | - | - |
| tests S3 | `offline_text` says "low power" during a spy's outage | **no defect** | the function's documentation states it (`minimap.rs`: "which a spy in a power plant also causes") | - |
| tests S4 | whether the unfixed sweep still exceeds the limit | **answered**: it does not (53,431 of 72,000) | measured by removing the rule | see P1-3 |

#### Advisory review 2 (RA4)

| Id | Claim | Verdict | Evidence | Test |
|---|---|---|---|---|
| sim P0-1 | ammunition is spent only by the last shot of a volley at a unit or building | **fixed** (decision 3) | reproduced with the review's probe on the code before the fix (50 shots at scenery and six cut volleys with full ammunition) | `ra4_aircraft.rs::an_aircraft_spends_its_rounds_on_scenery_as_on_everything_else`, `a_volley_that_is_cut_short_costs_its_round`; `ra4_cloak.rs::a_submerged_unit_that_fires_at_scenery_is_seen_for_the_cloak_delay` (on a fixture variant: no shipped or fixture cloakable unit fires at anything on land) |
| sim P1-1 | a launcher ordered onto scenery fires blanks for ever | **fixed** (decision 4) | reproduced (11 shots, no launch in 600 ticks) | `ra4_spawns.rs::a_launcher_ordered_onto_scenery_sends_its_rocket_to_the_cell` |
| sim P1-2 | a spawner that boards a transport has its missiles back at once | **fixed** (decision 4) | reproduced (launches at ticks 2 and 8) | `ra4_spawns.rs::a_launcher_that_boards_a_transport_and_leaves_it_has_no_rocket_sooner` |
| sim P1-3 | `Attack` tells a hidden unit from a missing id | **fixed** (decision 2) | fails before (`UnknownTarget` where a missing id gets `NoUnits`, `NotArmed`) | `ra1_commands.rs::every_command_answers_for_a_submerged_enemy_as_for_an_id_that_names_nothing` |
| sim P1-4 | unit commands that name a submerged enemy answer `NotOwned` | **fixed** (decision 2) | fails before | the same test |
| sim P2-1 | an aircraft more than the docks can be queued in the tick a dock is sold | **fixed** (decision 5) | reproduced (four accepted for two docks) | `ra4_aircraft.rs::a_dock_sold_earlier_in_the_tick_takes_no_aircraft` |
| sim P2-2 | an aircraft whose dock cell is held never lands | **deferred** as debt | reproduced by the probe (31 moves in 300 ticks) | - |
| sim P2-3 | an order that ends for lack of ammunition has no event | **deferred** as debt | a new `AttackEnd` reason changes the interface | - |
| sim P2-4 | the turn of the last stand never comes for some think intervals | **deferred** as debt | by arithmetic; the shipped intervals (10, 20, 40) meet every turn | - |
| sim P2-5 | `rank_of` and `experience_for` disagree for a type that costs nothing | **deferred** as debt | no trainable shipped type costs 0 | - |
| sim P2-6 | three decision texts against the code | **fixed** | R50 item 4 and R53 item 2 annotated; the module documentation of `veterancy.rs` | - |
| sim S1 | two killers for one victim | **fixed** (confirmed; decision 5) | fails before (the roller gained 100 for a recruit a strike had destroyed) | `ra4_veterancy.rs::a_crusher_gains_nothing_for_driving_over_what_was_destroyed_already` |
| sim S2 | the last stand of a player stalled on a building it cannot place | **no defect** (decision 5) | the rule's picture is "nothing changes"; pinned | `sim::ai::tests::a_player_that_can_neither_fight_nor_earn_makes_its_last_stand` (credits that stand still) |
| sim S3 | a missile or aircraft bought with ALL UNLOCKED | **refuted** | `check_buildable` refuses a tech level outside 0 to the rules' level before it asks for the option (`economy_commands.rs`), and the sidebar and the computer player use that function (R18) | - |
| sim S4 | a launched unit is moved by the Chronosphere | **fixed** (confirmed; decision 5) | fails before (the command was accepted) | `ra4_shipped.rs::the_chronosphere_does_not_move_a_launched_rocket` |
| tests P1-1 | two targeting assertions pass because the Typhoon is cloaked | **fixed** | the cases run with the Typhoon detected and still hold | `ra3_shipped.rs::shipped_ships_follow_their_naval_targeting` |
| tests P1-2 | no test loses a dock by capture | **fixed** | the review's mutation M22 now fails | `ra4_aircraft.rs::an_aircraft_whose_airfield_is_captured_takes_a_dock_of_another_or_is_lost` |
| tests P1-3 | "an elite unit in a long battle" has no test | **fixed** | a tank earns both ranks by its kills and fights three duels over more than 1,500 ticks, twice with equal hashes | `ra4_veterancy.rs::a_unit_that_became_elite_by_its_kills_fights_on_with_all_that_its_rank_gives` |
| tests P1-4 | a missile type without its effect loads and deals nothing | **fixed** (decision 5) | the test that asserted "accepted" now asserts the typed error | `ra4_definitions.rs::the_terms_of_a_spawner_must_fit_together` |
| tests P2-5 | the Harrier row of the crew test cannot fail | **fixed** | the aircraft has a dock and is asserted destroyed | `ra4_shipped.rs::only_the_base_units_have_a_crew_and_it_is_of_their_owners_side` |
| tests P2-6 | the shipped survivors test has only exact quotients | **fixed** | two rows that round up | `ra4_shipped.rs::the_survivors_of_shipped_buildings_follow_the_refund_and_the_divisors` |
| tests P2-7 | loose bounds on the spawn timings | **deferred** as debt | - | - |
| tests P2-8 | crew members that hold whole cells in the fixture | **deferred** as debt | - | - |
| tests P2-9 | destruction is set, not fought, in crew and dock tests | **no change** | the review accepts it as scenario setup | - |
| tests P2-10 | a rejection asserted without its reason | **fixed** | `MissingPrerequisite` of the Airforce Command | `ra4_shipped.rs::a_fifth_harrier_is_refused_and_built_after_a_second_airforce_command_stands` |
| tests P2-11 | two range checks tested from above only | **fixed** | rows with `-1` | `ra4_definitions.rs` |
| tests P2-12 | a comparison that can never decide | **fixed** (comment) | `special::reconnaissance` says where the delay ends | - |
| tests S | a captured building with docks and the queue check | **fixed** with sim P2-1 | the command counts the buildings that stand and are the player's in its tick; the count of aircraft is one stage old only for the player that lost the docks | `ra4_aircraft.rs` (the capture test asserts both players' docks) |

### R57. Review work stopped; the implementation is finished without it (user instruction, 2026-10-13)

During the remediation of the advisory reviews the user wrote: "Finish all the implementation
and stop review. Write a report about what reviews are left for later review work." From then:

1. No further advisory review is run, and the binding final review of the prompt's section 7
   is not run in this programme's run. No reviewer's decision exists.
2. Because section 7 allows the merge and the tag only after GO or GO WITH DOCUMENTED
   NON-BLOCKING DEBT, `ra2-parity` is not merged into `main` and no tag `ra2-parity-reviewed`
   is created. `main` stays at `8230a95`.
3. The remediation of the findings the advisory reviews had already reported (R56) was finished,
   as implementation work on known defects.
4. RA5 to RA9 are implemented as planned, each gated by the validation commands, which stay the
   only gate between milestones. The validation script runs thirteen commands since R56 (the
   eleven of the prompt and the two regeneration checks).
5. `docs/reviews/RA2-parity-reviews-outstanding.md` is the report the user asked for: what was
   and was not reviewed, what the advisory reviewers could not check, the decisions that need a
   reviewer's or the user's judgment, and a suggested order. The briefs prepared for the panel
   review (R55) are kept in `docs/prompts/ra2-parity-review/` for whoever does it.
6. Status entries and commit messages say "validated", never "reviewed" or "approved".

### R58. RA5-A: the economy file of schema version 2 and the rules file of schema version 2

RA5 is carried out in two parts (R57 item 4): RA5-A is the plan's items 1 to 4 (bails and gems
with their map side, growth and spread, the side's free miner, crates) and the refund of a sold
refinery that R56 deferred; RA5-B is items 5 to 9. This record and R59 to R62 are RA5-A's.

1. **`economy.ron` is at schema version 2.** The ore unit of a cell, of a harvester's `capacity`
   and `harvest_rate` and of a refinery's `credits_per_ore` is the **bail**. New fields:
   `harvesters[].harvest_interval` (ticks of work on a cell for each removal of `harvest_rate`
   bails; 1 is the rule of M3), `refineries[].credits_per_gem`, and the list `refunds` (the
   building types whose sale returns a fixed amount, from 0 to the cost). The names `capacity`,
   `harvest_rate` and `credits_per_ore` are kept (the plan calls the capacity `storage`): the
   meaning of the M3 fields is unchanged for a file whose bail is worth what its ore unit was,
   which is how every fixture is migrated (plan section 5, RA5: "economy fixtures migrate with
   one credit bails"; here with their own 2 credits, and `credits_per_gem` at twice that), so
   every M3 test keeps its numbers and its fixture edits.
2. **Shipped values.** A miner's capacity is the ini's `Storage` (War Miner 40, Chrono Miner 20);
   a refinery pays `[Riparius] Value` 25 and `[Cruentus] Value` 50; `refunds` lists the types
   with a `Soylent` that is not 0 (the two refineries at 300, the Ore Purifier at 1250, which is
   half its cost). The pace of harvesting is the game's own: one bail after every 19 ticks of
   work (**assumption**, plan appendix D topic 8: the ini has no key for it; the 14 frames of
   its other `.016` minute rates). A full War Miner takes 760 ticks of work, where it took 143
   before. Paid at once on unloading, as before (**assumption**: the reference's unloading time
   is undocumented).
3. **`rules.ron` is at schema version 2.** New in `economy`: `ore_max_bails` (12) and
   `ore_spread_above` (6), the game's own values (**assumptions**, appendix D topic 8: the
   twelve density stages of an ore cell, and the ancestor engine's rule that ore spreads from
   above stage 6); the generator writes them without an ini source and the audit holds them in
   a list of its own (`OWN_RULES`). New list `crate_units`: the mapped units with
   `CrateGoodie=yes` (ten vehicles, the two MCVs and the two miners among them). A `powerups`
   entry must name one of the eight contents a crate may hold (`CrateContent`); before, any
   name loaded and nothing read it. `ore_value` and `gem_value` stay in the rules file: match
   setup divides a map's ore amounts by them (item 5); what a refinery pays is the economy
   file's, which the generator writes from the same two reference rows, and a test of the
   shipped data holds the two together.
4. **A fixed refund, and who follows it** (R56 decision 7). `Definitions::refund` is the
   type's fixed refund if `refunds` lists it, else what the rule gives
   (`Definitions::rule_refund`: with a rules file the cost times `refund_percent`, without one
   `sell_value`). A sold refinery returns 300 credits. **The survivors of a building follow
   the rule's value, not the fixed refund** (`crew::escape` reads `rule_refund`): the plan's
   research gives the survivors as `Cost x SurvivorRate / divisor` (ModEnc) or "sell cost /
   divisor" (the ini comment), and no source says that `Soylent` enters either; the closest
   documented behaviour is the formula on the cost (**assumption**). So a sold or destroyed
   refinery releases 2 GIs or 4 Conscripts as before, and no match of computer players changes
   through the survivors. The computer player's last stand values a building by what its sale
   returns (`refund`). The two rows of `ra2_differences.tsv` are deleted; the audit compares
   `sell_value` with the rule's value and `Definitions::refund` with `Soylent`, and requires
   that a type is in `refunds` exactly if its section has a `Soylent` that is not 0.
5. **A map's ore amounts become bails at match setup** (`setup_match_configured`, with a rules
   file only): a cell with the amount `a` holds `clamp(round(a / value), 1, ore_max_bails)`
   bails, `value` being `gem_value` for a cell of gems and `ore_value` for any other. The plan
   puts this "in the map loader's result"; the loader does not know the rules, and every match
   (the game, the soak, the duels, the tests of matches) goes through the setup, so it is
   done there, once, before the first tick. The map format stays at schema version 2. With the
   shipped maps an ore tile (500) is 12 bails, 300 credits where it was 500, and a gem tile
   (600) 12 bails, 600 credits. A world built without `setup_match` (scenario tests) holds
   whatever its cells say, in bails. The fixtures of RA1 to RA4 have `ore_value` 1 and an
   `ore_max_bails` of a million, so that their maps hold what they held.
6. **Gems.** Which cells of ore are gems is state of the grid (`Grid::is_gems`, a set beside
   the cells, so that `Cell` and every test that writes one stay as they are), hashed under a
   tag only when there are any. A map says it with the look feature 10 (`LOOK_FEATURE_GEMS`,
   `Feature::Gems`): the loader marks every cell with ore and that feature, and an enlarged map
   keeps the marks where a made start position changed nothing. **Look features in use:** 0
   none, 1 ore, 2 cliff, 3 road, 4 forest, 5 house, 6 city, 7 tower, 8 bridge, 9 lava, 10
   gems; 11 to 15 are free (five values) for RA5-B's tech buildings and huts. A harvester
   counts the bails of gems in its load (`Harvester::gems`), and `OreDelivered` has the field
   `gems`. An emptied cell of gems is no cell of gems.
7. **The purifier's and the spy's values are the rules'.** Every powered Ore Purifier of a
   player adds `purifier_bonus_percent` of a load's value (the plan's section 4.10 says each;
   before, any number of them added one quarter); a spy in a refinery takes
   `spy_money_steal_percent` of the owner's credits. Without a rules file the literals of M9
   stay (a quarter, a half), which the M9 fixtures assert.
8. **The free miner** was already the side's (`[GAREFN] FreeUnit=CMIN`, `[NAREFN] FreeUnit=HARV`
   since RA1, audited) and the Chrono Miner's self healing is RA4's; RA5-A adds the test that
   builds each refinery through commands and finds the miner of its side.

### R59. RA5-A: ore that grows and spreads

1. **The rule** (`crates/sim/src/ore.rs`; plan section 4.10). The pass runs at the end of
   the `Commands` stage, not in the `Economy` stage the plan names: nothing is built on ore
   (item 4), a computer player decides where to build in the `Ai` stage and its command is
   checked in the next tick's `Commands` stage, and a pass between the two could put ore under
   the footprint and have the command rejected, which the tests of the shipped matches do not
   allow. After the commands and before the `Ai` stage no decision is pending. Only with a rules file and the match rule `ore_grows` (new in `MatchRules`
   and `MatchOptions`, default on; hashed only when off). On every tick whose index is a
   positive multiple of `ore_growth_interval` (2933) each cell of ore below `ore_max_bails`
   gains a bail with the probability `ore_growth_pm` (60); on the multiples of
   `ore_spread_interval` each cell of ore with more than `ore_spread_above` bails puts, with
   the probability `ore_spread_pm`, one bail on one of the free cells around it, each equally
   likely. Gems neither grow nor spread. `[General] GrowthRate`, which the prompt names, is
   unused by the engine (plan section 4.10); the per ore keys are the ones ported.
2. **Order and randomness.** A pass visits the cells of ore in row-major order with the bails
   they held when it began; for each cell the growth is drawn before the spread; a cell at the
   densest draws nothing for growth, and a cell with no free cell around it draws nothing for
   spread; a probability of 0 draws nothing. So a rules file that turns both off (the fixtures
   of RA1 to RA4) leaves the random sequence of its world as it was. The whole grid is scanned
   once per pass (65,536 cells every 2933 ticks at most), which the benchmark does not notice.
3. **Where ore may spread** (`Grid::takes_ore`): a land cell a ground unit may enter, without
   ore and without a building, that is building ground **or was emptied by a harvester**. The
   map format's ore ground is no building ground, and neither are its roads, beaches, ice and
   bridges, which the simulation cannot tell from one another; the grid therefore remembers the
   cells a harvester emptied (`mined`, hashed under a tag when there are any) so that a field
   can grow back onto its own ground, and roads and bridges stay free of ore (**decided
   here**; the plan says "free neighbouring land cell").
4. **Nothing is built on ore.** `check_footprint` refuses a cell that holds ore
   (`PlacementError::NotBuildable`), also for the deploying of a base unit: ore that spread
   onto building ground keeps it free until it is harvested, as in the reference. For the
   map's own ore ground nothing changes.
5. **Not modelled:** ore drills (the plan's non goal), and the lobby has no row for
   `ore_grows` yet (the option exists in the simulation and defaults to the reference's "on";
   debt for the interface milestone).

### R60. RA5-A: crates

1. **State and stages** (`crates/sim/src/crates.rs`; plan section 4.11). `Crates` (a resource,
   hashed if the world ever had a crate): the cells with a crate and the tick until which each
   lies, the ticks at which a crate is due, and whether the match's first crates were asked
   for. `crates::appear` runs in the
   `Economy` stage, `crates::collect` in the `Movement` stage after the units have moved. No
   command names a crate: it is taken by standing on it.
2. **Where crates come from.** Only with a rules file, in a match with players
   (`setup_match*`) whose rule `crates` is on: `crates.minimum` (1) appear in the first tick,
   and a crate that is taken is replaced after a time drawn between half and twice
   `crates.regen`, both inclusive (1800 to 7200 ticks), unless `crates.maximum` (255) lie on
   the map then. So a shipped match has one crate at a time, as the reference's `CrateMinimum`
   keeps it (single source). A crate appears in a cell drawn from the whole map that is land a
   ground unit may enter, or water, with no building, unit, ore or crate; after 64 draws
   without such a cell it is tried again 20 ticks later. **A crate nobody takes lies for a
   time drawn from the same range** (1800 to 7200 ticks) **and is then replaced at once by one
   somewhere else** (`CrateGone`): the plan does not say what becomes of a crate that is never
   taken, and with one crate a match, drawn from the whole map, a crate on a pond or behind a
   ridge would be the match's only crate for good. The ancestor engine gives a crate a lifetime
   of half to twice its `CrateRegen` and puts another down when it runs out (**assumption**,
   from the same source as the plan's rule of the minimum; not in the plan's research). A
   crate on whose cell a building is placed is replaced at once in the same way (decided here:
   a placement does not look at crates). A crate a scenario puts down lies until it is taken. **A world without a match gets no
   crates by itself** (decided here: the scenario worlds of several hundred tests have the
   default rules of a lobby, and a crate that appeared in each would give random effects to
   whatever drove over it); a scenario puts crates down with `Crates::place`.
3. **Who takes a crate.** The unit with the lowest id that stands in the cell: a vehicle, a
   ship or a soldier whose own cell it is (a unit on its way there has not arrived; one
   passing through takes it), with hit points left, of a player of the match. Aircraft do not
   (they hold the air layer). Computer players take crates when they happen to cross them.
4. **What a crate holds** is drawn when it is taken, by the shares of `powerups` (110 in all):
   `money` (the value 2000 and a draw from 0 to 900, `CRATE_MONEY_SPREAD`: **assumption**
   that every amount in between is possible), `unit` (a type drawn from `crate_units`, in the
   nearest cell with room within 3 cells, `CRATE_UNIT_RADIUS`: the game's own),
   `veteran` (every trainable unit of the collector's owner within `crates.radius` rises one
   rank), `armour`, `firepower`, `speed` (every such unit that has none yet gets the
   multiplier 1500, 2000 or 1200 through `Experience::bonus`), `heal_base` (every unit and
   building of the owner regains its type's hit points), `reveal` (the owner has explored the
   whole map; **not** "except under enemy Gap Generators", which waits for the shroud rules of
   RA7: a listed remainder). The radius is measured between the centres of the cells.
5. **Money instead.** A collector that cannot gain from `veteran`, `armour`, `firepower` or
   `speed` (it is not trainable, is elite, or has the multiplier already) gets money instead,
   and then no other unit gains either; the same for a `unit` on water, without a type to
   draw, or without room. The multipliers need `Experience`, which only trainable types have
   (R47), so an untrainable unit beside the crate gains nothing (the reference gives it the
   multiplier; a listed remainder).
6. **The free base unit** (`crates.free_mcv`). A collector whose owner has no building and no
   base unit with hit points left and more than 1500 credits (`FREE_MCV_CREDITS`, hard coded
   in the original) gets the base unit of its side instead of a drawn content, free of
   charge; money on water. In a short game such a player is defeated already, so the rule
   acts in long games.
7. **Randomness**, from `SimRng` only, for each crate taken (the crates of a tick in the order
   of their cells): the content (unless the free base unit applies), then what the content
   draws (the unit's type; the credits of money, also of money given instead), then the time
   until the replacement. A crate that appears draws two numbers an attempt and then the time
   it lies.
8. **Events.** `CrateAppeared { cell }`, `CrateGone { cell }` and `CrateTaken { player, unit,
   cell, content, credits, spawned }`. The game announces the human player's own crate with a notice and the
   credits sound, and marks a crate the player sees now with a plain box (R61).

### R61. RA5-A: gems on the maps, and what the game shows

1. **Tiles.** Five gem tiles join the shared tileset (ids 46 to 50: on grass, desert, snow,
   ash and marsh): ore ground (passable, not building ground) with the amount 600 and the look
   feature `Gems`. Every generated map file and tileset image changes with the tileset;
   existing tile ids keep their numbers.
2. **Where.** `Painter::ore` puts a seam of gems (the ore within 2 cells of the centre, 21
   cells) at the heart of every explicit field of radius 5 or more whose centre is not within
   20 cells of any of the map's start positions; a map without such a field has one seam pair
   placed by its recipe (`Painter::gem_seam`: Coral Archipelago, Glacier Pass, Red Canyon,
   Pacific Atoll, Continental Divide, Frozen Sound, Black Water; Wheel of Worlds has one in
   each of the ten fields of its hub ring). Only ore tiles become gem tiles, so where units go,
   where buildings stand and where ore lies is what it was: symmetry, base sites, ore within
   reach and reachability (`m7_maps`) hold as before. The eighteen large maps have 34 to 210
   cells of gems, at most a tenth of their ore cells, none within 15 cells of a yard.
3. **The three small maps.** `demo` (its own generator and tileset): nothing changes.
   `crossroads` and `riverlands`: no gems and the same cells; their files change only in the
   tileset definition (51 tiles instead of 46).
4. **The game.** Gems are drawn with the ore top stained blue-green (no new image; the art
   of RA5-B may replace it), and are teal on the minimap. Ore that spread onto ground without
   an ore top gets one when the player sees the cell (the grid is searched every 20 ticks); a
   mined-out top is shown again when ore spreads back. A crate the player sees now is a plain
   two tone box: a placeholder. **Requires user verification:** how gems, grown ore and the
   crate mark look.
5. **Not done here:** no marker for crates on the minimap, no hover text for gems or crates,
   no lobby row for `ore_grows` (R59 item 5).

### R62. RA5-A: tests adapted (none deleted, ignored or weakened), and the computer player

Adapted because RA2 behaviour is intended or a schema moved:

* `m3_definitions.rs`: `schema_versions_other_than_1_are_rejected` is
  `schema_versions_other_than_2_are_rejected` (versions 0, 1, 3, -1 and the largest are
  rejected; before 0, 2, -1 and the largest); the two files written in the test have
  `refunds: []` and version 2; the two duplicate entries the test appends carry the new fields;
  one edit of the fixture names the line that now follows `unit:`; the expected
  `HarvesterDef` has `harvest_interval: 1`. Every assertion is the same.
* `ra1_definitions.rs`: the rules file's version is 2 (rejected: 0, 1, 3); the table of the
  scalar rules has the two new fields, so their limits and the step outside are tested like
  the others'.
* `ra1_catalogue.rs`: a sold refinery returns 300, not half its cost (R58 item 4).
* `ra1_shipped.rs`, `crates/game/src/skirmish.rs`, `smoke.rs`: `MatchRules` and `MatchOptions`
  have `ore_grows`.
* `ra9_data_parity.rs`: the audit of the new fields (R58 items 3 and 4).
* `crates/game/src/sim_host.rs`, two tests: the first tick of a match also has the event of
  its first crate.
* The fixtures of RA1 to RA4 (`fixtures/data/ra1..ra4/rules.ron`): `ore_value` 1, `gem_value`
  2, growth and spread at 0 permille, `ore_max_bails` a million, so that their matches hold
  the ore they held; their crates stay on (minimum 1), and every test on them passes with
  crates in their matches.

New: `ra5_definitions.rs` (9 tests), `ra5_ore.rs` (19), `ra5_crates.rs` (18), `ra5_shipped.rs`
(9), `crates/game/tests/ra5_gem_maps.rs` (3), one unit test in `map.rs`, and additions to the
notices test. The fixture `fixtures/data/ra5` is the RA4 fixture with the ore and crates of
RA5, a purifier and a spy.

**The computer player** is unchanged. It takes crates only by chance, deploys a base unit a
crate gives it (its rule of RA1), and never builds on ore (the placement check). Income fell
(a load takes five times as long to gather; an ore cell holds 300 credits instead of 500),
so matches are decided by smaller armies; how long they take is in the validation of RA5-A
(`docs/validation/ra2-parity/ra5a/`) and in RA5's status entry.

### R63. The implementation stopped during RA5 (user instruction, 2026-10-13)

The implementer of RA5-A (ore and gems in bails, growth and spread, the free miner, crates, the
refinery refund) was stopped from the user's side while it was finishing. Asked how to proceed,
the user answered: "Stop where you are here, and write a comprehensive review requirement for
later review". So:

1. Nothing further is implemented. RA5 items 5 to 9, RA6, RA7, RA8 and RA9 do not exist; the
   final review does not exist (R57); the branch is neither merged nor tagged.
2. RA5-A's three commits (`9f625b3`, `c9e3604`, `212874a`, decisions R58 to R62) stay on the
   branch as they are. Of the validation commands only `m14_duels -- full` was run for them
   (756 of 756 matches ended; logs in `docs/validation/ra2-parity/ra5a/`); the orchestrator
   additionally observed that formatting, `git diff --check`, clippy and the two regeneration
   checks exit 0 on the tree as left. The test run, the builds, both smoke tests, the
   benchmark, the soak and the package script were not run on it. The agent's four closing
   steps (fix a warning, compute the distribution of duel lengths, add it to R62, commit) were
   not done, and RA5 has no status entry of its own.
3. An unfinished, uncommitted test module of 150 lines was left in
   `crates/game/src/battlefield.rs`. It is deliberately not committed: the orchestrator did not
   write, finish or run it. It remains in the working tree.
4. `docs/reviews/RA2-parity-review-requirements.md` is the document the user asked for.
5. `docs/RA2_GAP_ANALYSIS.md` and `docs/CONSTRUCTION_SYSTEM.md` were to be rewritten in RA9 and
   were not: they describe the game of 2026-10-11 and of M9, not the branch.

### R64. RA5-A closure after the stopped run

The continuation prompt's Step 0 resumes only RA5 items 1 to 4. R58 to R62 remain the rules;
the two AI command/assembly corrections below are needed to finish the unchanged shipped-match
gate. Generated data, schemas, commands and match tick limits remain unchanged.

1. **Preserve the unfinished tests.** The 150-line module left in `battlefield.rs` contains
   two useful tests: a crate is hidden before sight, drawn exactly once while seen, and removed
   after a unit takes it through `Command::Move`; ore on new ground receives exactly one top,
   initially hidden. Both are retained with their assertions intact. A third test,
   `gems_have_a_distinct_tint_in_both_terrain_paths`, checks ordinary ore against gems through
   both `terrain` and `sprout_ore`, with two actual spawned tops and a literal expected tint;
   dynamically discovered tops are initially transparent. These are presentation-state tests,
   not a claim that the art looks right. Gems, grown ore and crates still require user verification.
   Static `#[test]` counts across `crates`: 979 at the incoming committed tip, 981 in the
   inherited working tree, 984 after the gem and two AI regressions; `#[ignore]` remains absent. All three
   battlefield tests pass. Runtime workspace totals are reported separately in the status entry.
2. **Independent reference sample.** Read the cached ini directly with a small independent
   section/key parser, not the generator, reference TSV or audit. Its SHA-256 is
   `adc869ca037db5141f562f18092d022045e194de0d1204a46b9ec8372b2b826f`, matching R2.
   Exact rational arithmetic and half-up integer rounding give the following, all matching
   the generated `rules.ron` and `economy.ron`:

   | Reference | Recomputed shipped value |
   |---|---|
   | `[Riparius] Value=25`, `[Cruentus] Value=50` | 25/50 credits per bail |
   | `[HARV] Storage=40`, `[CMIN] Storage=20` | 40/20 bails; full ore loads 1000/500 credits, gem loads 2000/1000 |
   | `[Riparius] Growth=2200`, `Spread=2200` | `round(2200 * 4 / 3)` = 2933 ticks each |
   | `[Riparius] GrowthPercentage=.06`, `SpreadPercentage=.06` | 60 permille each |
   | `[Cruentus] Growth=10000`, `Spread=10000`, both percentages 0 | neither growth nor spread, as the gem exclusion in `Grid::ore_cells` implements |
   | `[CrateRules] CrateRegen=3`, `CrateRadius=3.0` | 3600 ticks, 768 subcells; replacement range 1800 to 7200 ticks |
   | `[CrateRules] CrateMinimum=1`, `CrateMaximum=255`, `FreeMCV=yes` | 1, 255, enabled |
   | `[Powerups]` Money, Unit, Veteran | shares 20 each; arguments 2000, absent (0), 1 |
   | `[Powerups]` HealBase, Armor, Firepower, Speed, Reveal | shares 10 each; arguments absent (0), 1500 pm, 2000 pm, 1200 pm, absent (0); all shares total 110 |
   | `[GAREFN] Soylent=300`, `[NAREFN] Soylent=300`, `[GAOREP] Soylent=1250` | refunds 300, 300, 1250 credits |

   The cached free units are `GAREFN` → `CMIN`, `NAREFN` → `HARV`. No new engine-behaviour
   assumption is inferred from this numerical sample; the assumptions about density, harvesting,
   instantaneous unloading, crate lifetime and money randomness remain those of R58 to R60.
3. **The distribution missing from R62.** Independently parsed all 756 winning rows of
   `docs/validation/ra2-parity/ra5a/10-duels.log`. A ratio is the length divided by the
   shipped test's `72000 * max(width + height, 128) / 128`, as in R29 and R53. Median is the
   mean of the two central ordered ratios; p90 is nearest rank (`ceil(.9 * 756)`). Median
   **0.1215044974**, p90 **0.3362777778**, maximum **0.9315173611**; total **27,237,240 ticks**.
   Counts above 0.5, 0.7, 0.8 and 0.9: **22, 5, 2, 1**; none over 1. The slowest and largest
   ratio are Steel Harbor, France against Germany, both medium: **268,277 / 288,000**.
   The duel example itself still has its separate **300,000** tick cap; the test's base limit
   is still **72,000**. No limit was increased. The near-limit tail remains debt for RA8.
4. **Storage correction.** The continuation prompt's inherited statement that no target
   directory is configured is stale. This run preserves
   `/Users/xingyuwang/Library/Caches/Codex/cargo-target`, `UV_CACHE_DIR` and `PIP_CACHE_DIR`.
   No clone, worktree, validation copy or substantial temporary artifact is created. The
   configured Cargo directory holds the compilation cache and the package output.
5. **The first full gate exposed a real AI stall.** The all-map test failed on Thousand
   Lakes, France against Germany, both brutal, seed 0: no winner at the unchanged 288,000
   tick limit. The old duel sample uses seed 14 for this case and finishes at 47,118 ticks;
   its success did not establish the shipped test's success. Failed and diagnostic logs are
   preserved under `docs/validation/ra2-parity/ra5a-close-attempt-1/`.
   Large maps already scale army and wave sizes by up to two, but left their assembly patience
   unchanged. With the slower RA5 economy the patience expires before the larger wave forms,
   repeatedly feeding small forces into intact bases. `scaled_for_map` now scales assembly
   patience by the same capped factor; small maps are unchanged. The existing scaling test
   gains exact mid/large patience assertions. A shipped-data seed-0 regression reproduces the
   old failure and now ends at tick **52,499**, well within the original limit.
   Separately, an idle wave member previously waited for every comrade to stop before receiving
   another order. A stopped member outside the target's existing gathering radius now receives
   the existing target's `AttackMove` when comrades have weapon orders, together with new recruits;
   active and arrived members retain their orders/rest. A direct unit regression checks the exact
   command and selected member, and that travelling/arrived members are left alone. An initially
   broader retry prevented search waypoints from retiring and failed the ten-player Crossroads
   test at 72,000 ticks. That failed second attempt is preserved under
   `docs/validation/ra2-parity/ra5a-close-attempt-2/`; the arrival-aware correction passes both
   that unchanged test and the Thousand Lakes regression. This correction alone did not fix the reproduced match, as its diagnostic
   log shows. Neither correction adds simulation state or changes victory rules, vision,
   damage, income, or the match limit. The computer still uses ordinary commands.
6. **Validation.** The full thirteen-command run is recorded under
   `docs/validation/ra2-parity/ra5a-close/`. Completion and exact counts are recorded in the
   RA5-A closure status entry. All thirteen commands exited 0; workspace runtime totals are
   **989 passed, 0 failed, 0 ignored**. This record is not an independent review.
7. **Final-source distributions, distinct from the inherited sample above.** The workspace
   production sweep's `production-match-lengths.tsv` records 756 winners, 27,031,638 ticks:
   median **0.1306853009**, nearest-rank p90 **0.2849236111**, maximum **0.7354740741**
   (Glacier Pass, **Korea against Russia, both brutal**,
   198,578 / 270,000). Counts above 0.5/0.7/0.8/0.9 are **21/2/0/0**.
   The fresh duel example records 756 winners, 28,558,760 ticks: median **0.1349340278**,
   p90 **0.3227777778**, maximum **0.8229027778** (Demo, America against Korea, both easy,
   59,249 / 72,000); counts above 0.5/0.7/0.8/0.9 are **18/5/1/0**. Its longest match in
   absolute ticks is Ember Caldera, France medium against Libya easy, **221,221 / 288,000**.
   Both use the unchanged scaled test limit as denominator, not the example's separate
   300,000 cap. The inherited near-limit tail is improved on these samples, not a proof for
   every seed; broader AI capability and stress coverage remain RA8 work.

### R65. RA5 neutral objects, occupation and bridge simulation

RA5-B completes items 5–9 using the existing deterministic simulation stages and ordinary
`Interact`, `Deploy`, `Attack` and production commands. The new implementation is validated,
not independently reviewed or approved.

- `PlayerId(255)` is reserved for neutral objects: `Players::add` rejects it, hostility never
  selects it, and victory excludes it. Captured `Insignificant` buildings do not keep a player
  alive. Living occupants and hospital patients still count as units in a long game.
- Arsenal schema 6 adds thirteen required reference-derived fields: `NeedsEngineer`,
  `Capturable`, `Insignificant`, `Immune`, `Hospital`, `BridgeRepairHut`, `Occupier`,
  `CanBeOccupied`, `MaxNumberOccupants`, repair `NumberOfDocks`, and the three `ProduceCash`
  terms. Every legacy and RA1–RA5 fixture migrates. The schema tests intentionally now reject
  version 5, accept 6 and reject 7. Missing reference Cost/Sight are zero, Armor is none and
  Adjacent is two; missing new flags/numbers are zero. All mappings enter the completeness
  audit; the RA5 deferred-present rows are removed. The reference roster remains 43 units and
  29 buildings; the eight supplementary world definitions are not roster-parity credit.
- Four tech buildings and repair huts are one-cell map anchors. A consumed Engineer captures
  a tech building. Neutral capture of the Oil Derrick pays 1000, then its current owner receives
  20 every 133 ticks; enemy recapture pays no startup grant and restarts the deadline. Captured
  buildings cannot be sold. Hospitals heal entered friendly land infantry by 20 per tick and
  return the same ID, health and experience when fully healed. Tech Outposts have exactly one
  paid vehicle repair berth, distinct from aircraft docks, and the reference HoverMissile.
  Airport ownership is ready for the RA6 paradrop implementation.
- Civilian House/City/Tower props use CAEUR1/CAWA2A/CACITY01 as the representative reference
  types: capacities 3/10/10, strength 300/600/1000, and their reference armour. This is a
  documented mapping of the game's three visual categories, not a claim that all RA2 civilian
  buildings share these values. Only GI/Conscript Occupiers enter. Allied/Soviet occupation
  weapons fire with reload divided by current crew count, floored at one tick. Red health
  (inclusive 25 percent) refuses entry and evacuates; Deploy also evacuates. Blocked exits
  retain passengers until ordinary Grid registration and standing-point allocation succeed.
  Occupation uses the first surviving occupant's side when mixed allied-country crews enter.
  Cargo health, identity and experience are retained; occupation kills do not grant crew XP.
- A hospital destroyed or captured by an enemy loses its patients, matching the existing
  transport-host loss convention. This is an explicit closest implementation assumption where
  the reference does not specify the game's queued-entry/cargo representation. A destroyed
  civilian host instead attempts emergency evacuation, retaining blocked exits until room
  exists. No dead actor acts later in its command tick; unseen or hidden enemy IDs reject as
  missing IDs. Occupied props are owned, currently visible combat targets; RA7's optional
  explored-is-visible behavior is not enabled early.
- Connected bridge decks share the reference 1500 health. One area blast damages a run once,
  at its strongest affected deck cell. Collapse turns every deck cell to water and destroys
  land units standing on or stepping into it; air and amphibious units remain. An Engineer
  entering a mapped repair hut restores the entire saved deck and is consumed. Ordinary damage
  cannot destroy huts. The default-on `bridge_destruction` match option is hashed and exposed
  in the lobby; its new row changes the rules panel from eight to nine rows. Nobody acquires
  neutral props or tech buildings as autonomous weapon targets.
- AI bridge recovery learns collapsed decks and huts only through current sight, retains that
  knowledge after sight is lost, and produces an ordinary Engineer. It approaches remembered
  huts with Move and requires current hut/deck sight for Interact (R69). A shipped-rule Brutal
  match regression commands the collapse of the sole route through a full-height water barrier, observes an Engineer consumed
  in repair and ground forces crossing, and requires a winner within the unchanged 72,000
  ticks, without aircraft or alternate land paths. Occupied civilian targets enter the existing
  visibility-filtered AI building snapshots.

### R66. RA5 shared map installation, discrete scaling and original world art

`Grid::world_features` retains semantic looks so headless matches cannot silently drop the new
objects with `LoadedMap.looks`. Match setup installs props, tech buildings, huts and connected
bridges through the same installer used by the client. Stable map IDs occupy their own reserved
range. All new future-affecting state is included in the deterministic hash.

Discrete tech/hut anchors retain source-cell identity when a map grows: two adjacent source
objects remain two objects, and repeated terrain bands do not create extra buildings. Repeated
land foundation cells become a clear apron; repeated water foundation cells remain water.
Bridge and civilian-prop cells retain their connected/terrain semantics. Start clearing may
remove an anchor where it actually changes that cell. Tests cover adjacent identity, absence of
invisible blocked remnants, real installed objects, symmetry and setup from two to ten players.

The eighteen large map recipes place point-symmetric pairs of all four tech types and bridge-end
huts. The three small map layouts keep their existing coordinates. Existing maps include bent
decks and one-bank piers, so a seaward terminal may have no shore. Prefer a shore hut; otherwise
put its hut on a raised foundation beside the deck, retaining water beneath it and adding no
traversable land. The shore hut remains the recovery entrance after that pier collapses. This
is a map-design assumption for the original decorative piers, not a reference-engine rule.

Original models in `scripts/hd_buildings_neutral.py` provide the four tech buildings and repair
hut; `scripts/generate_ra5_world_art.py` provides a faceted gem atlas and timber crate. Civilian
simulation definitions reuse the existing civilian scenery presentation. Neutral ownership is
gray, capture repaints it to the owner, occupied buildings show crew pips and support selection
and Deploy, crate effects retain their pickup notices, and bridge decks hide/reappear only when
the viewer sees collapse/repair. Art and audio quality still require user verification.

Storage policy is unchanged: the configured Cargo, uv and pip caches are preserved. The original
render passes are retained in
`/Users/xingyuwang/Library/Caches/Codex/reviews/MyGame001/ra5b-render` for visual verification and
reproducibility; no clone, worktree, build output or dataset was created under `/tmp`.

R66 validation also intentionally extends the existing scenery assertion to distinguish passable
intact decks from blocked civilian scenery; the old assertion had no bridge props. Lobby layout
expectations become 17 rows and 75 buttons, with all existing fit/bounds checks retained. The
asset inventory explicitly includes the new crate texture and the two occupation weapons get
Rifle presentation classes. Supplementary definitions are appended after existing roster entries
so legacy production indices stay stable; no old type is reordered to accommodate world objects.

The grown-map test's former “nothing is made” assertion assumed every map look was repeatable
terrain. Its new discrete-foundation branch checks the exact apron/water transformation before
applying all original made-start and symmetry checks to other changes; repeated offshore huts
must retain water. The original assertion is retained for every other cell. Separate RA5 tests
verify adjacent object identity, one anchor per source, and the generated pier's reachable shore
repair after commanded collapse. No layout, victory, or performance limit is relaxed.

Generated tech sites require three cardinal land neighbours so scaled clear aprons stay joined
to the existing land. Hut placement additionally preserves local connectivity between its
neighbours: blocking a narrow peninsula neck can strand a pocket even with three neighbours.
The existing all-passable-cells reachability assertion remains unchanged and exposed this case
on Black Water. The required-field deletion matrix now anchors `, docks` to distinguish it from
`vehicle_docks`; it still requires exactly one fixture edit and rejects the missing field.

Cheap RA5 mutation evidence is retained in `docs/validation/ra2-parity/ra5-mutations/`:
disabling neutral startup income fails the exact 11,000-credit assertion; disabling AI bridge
repair fails the sole-route winner regression at its unchanged 72,000-tick cap with no repair,
Engineer consumption or crossing. Both exit 101 from the intended test assertion, and original
source SHA-256 values equal the restored values. Focused deterministic replay compares 141
state hashes across two capture/hospital runs; separate sensitivity checks cover cash host and
deadline, bridge HP/collapse/saved decks/hut links, hospital cargo HP and garrison owner/location/
reload/eviction/cargo HP. The full required-field matrix includes all thirteen new fields.

### R67. RA5 occupied-building targets and safe bridge restoration

A completeness check during the first RA5 gate found that garrison firing enumerated ECS
actors but not enemy occupied civilian Props. A commanded GI/Conscript two-house regression
failed with neither garrison firing; the retained log is in `ra5-attempt-1/`. Garrison target
selection now includes living hostile occupied Props using current visibility, hidden-target
rejection, their armour and the ordinary occupation weapon/warhead eligibility. All candidates
share the same distance/SimId tie break. Neutral civilian buildings remain autonomous non-targets.
The full gate was stopped during its production sweep; completed targets passed, but that
incomplete run is not authoritative.

Bridge restoration must preserve the simulation's single surface layer. A naval unit may be
standing in collapsed water or hold it as an in-flight step destination; restoring land under
it would violate navigation. Repair therefore refuses while any deck cell has a naval holder,
including reserved destinations. The Engineer and ship remain intact. Validation repeats while
the Engineer approaches and immediately before restoration, so a later entering ship cancels
the pending interaction. After clearance, a new ordinary Interact restores the bridge and
consumes the Engineer. Air and amphibious occupants remain valid and undamaged. Tests cover
stationary, entering and post-order arrival cases, clearance, and compatible occupants.

This refusal policy is an explicit simulation assumption, **not a verified RA2 engine fact**;
RA9 and the final differences report must retain it. It avoids inventing ship destruction or
teleportation to fit the one-layer grid. It has no ini scalar to add to the generated-data
exception table. No match limit or existing assertion is relaxed.

### R68. RA5 inactive world-object systems avoid deep rule copies

The new garrison fire/cleanup and capture-income advance paths cloned `Definitions` before
checking whether they had work. `Definitions` owns vectors and strings, so this made three deep
copies every tick even in an ordinary AI match with no garrison or captured derrick. The first
two gate attempts' multi-enemy target took about 330 seconds against RA5-A's 120 seconds.
Return before the clone when the corresponding state map is empty. Active behavior, state and
ordering are unchanged; no old system is refactored. Other new paths clone only when installing
objects or copy their small live state, and do not add an equivalent inactive ruleset copy.

Attempt two was stopped in the production sweep with no observed test failure and is retained
in `ra5-attempt-2/`. The 22 focused world-object tests pass. The replay test prints a digest of
its 141 state checkpoints; `ra5-empty-state-equivalence/` compares the unoptimized and optimized
sources, requires both runs to pass and their digests to match, and records exact source
restoration. The complete final gate reruns all thirteen commands after this change.

### R69. RA5 remembered bridge repair objectives after scouts lose sight

The third full gate failed its production sweep only on seed 0, Coral Archipelago, Libya versus
Germany (both brutal): no winner at the unchanged 288,000-tick cap. The exact focused reproduction
also failed. Its trace showed collapsed routes left unrepaired after the observing combat units
moved away or died; Engineers remained at home while army waves became stranded. The previous
repair policy required the hut and collapsed deck to be currently visible on every decision.

AI controllers now remember hut identity, associated bridge and location only after seeing the
hut, and remember a collapse only after seeing a deck cell. They forget that collapse when a
currently visible deck is intact. An idle Engineer may Move toward a remembered reachable hut
while it is unseen, but fresh Interact requires the hut and a deck cell currently visible.
This last condition prevents an obsolete remembered objective from producing an invalid repair
when somebody else restored the bridge out of sight; the first focused fix exposed that at tick
63,762 and its failure log is retained. Reachability starts at the Engineer, or the home gather
point before training one, rather than an arbitrary lowest-ID army member already across the
river. Unreachable huts are excluded in favour of reachable remembered alternatives. Existing
current-visibility rules remain in force; RA7's explored-is-visible option is not enabled.

All remembered hut IDs, bridge IDs, locations and observed-collapse IDs are hashed in ordered
collections. A focused sensitivity test changes each field and proves clearing memory restores
the initial hash. The exact Coral regression now wins at tick 196,067 / 288,000 (0.680788), without
changing a unit, AI data value or limit. The sole-route command-level regression additionally
requires a fresh AI Move toward the unseen hut after losing sight, consumed-Engineer repair,
a ground crossing and a winner within the existing 72,000-tick cap. Its first movement-only
assertion admitted an Engineer already walking from a visible Interact; the retained mutation
survived, so the assertion was strengthened to inspect the actual queued Move. Clearing memory
each think now fails that assertion with `approached_from_memory=false`; exact source SHA-256
restoration is recorded. No old assertion was removed or relaxed.

Attempt-three gate logs, the exact before/after traces and both mutation outcomes remain in
`docs/validation/ra2-parity/ra5-attempt-3/`. Commands 01–03 and 05–08 passed; 04 failed as above;
09 was stopped at startup and 10–13 were not run. This failed attempt is superseded only by the
complete final RA5 gate. The mutation backup used the existing approved `ra5b-render` review
cache directory and was removed by its restoration trap; no additional external directory was
created. An initial /tmp backup command was rejected before execution by automatic approval
review; the safer cache-backed retry succeeded without a further user approval request.

### R70. RA5 repair-hut paint and semantic look expectations

The fourth gate's complete production sweep passed all 756 matches, then the existing HD
ownership-art check found zero accent pixels in the new repair-hut sprite. Its original model
had hazard paint but no team-mask material. Author a broad front fascia in `TEAM`, regenerate
its colour/mask passes with Blender and compose the shipped PNG with the existing finishing
script. Neutral presentation renders that mask gray; the hut remains uncapturable. The unchanged
32-pixel ownership check now passes, as do all 42 cheap game asset/map integration tests. The
render was inspected for a visible fascia and intact silhouette; artistic acceptance remains
user verification. The render cache is the same directory recorded in R66, with no new external
directory. No image was downloaded or reference artwork copied.

The simulation suite, not reached by the interrupted gate, also exposed an old expectation in
`m1_map`: look byte 255 was previously called presentation-only, but RA5 intentionally allocates
its high nibble 15 to repair huts. The named test now uses pure-ground look 15 for its original
exact Grid-equality assertion, still accepts 255, asserts its exact three world-object anchors,
and removes only metadata before requiring exact equality to the original terrain Grid. The
negative -1/256 boundary tests remain. This is the intended semantic-map change from R66, not a
relaxation of terrain or parser coverage.

Attempt four, including the failed art check and the initial simulation expectation failure,
is retained in `docs/validation/ra2-parity/ra5-attempt-4/`. Its production TSV records 756 winners,
27,606,823 total ticks, median ratio 0.1323229167, nearest-rank p90 0.3186944444 and maximum
0.7095694444 (Riverlands, France/Germany easy, 51,089 / 72,000); counts above 0.5/0.7/0.8/0.9 are
20/1/0/0. Commands 01–03, 05–06 passed; 04 failed only at the later art assertion, 07 was stopped
and 08–13 were not run. The source and art are frozen again only after the full simulation suite
and asset/integration checks; the next all-thirteen RA5 run remains the required authority.

The complete `sim --no-fail-fast` run exercised 764 tests: 760 passed and four failures were
old schema-test expectations, not runtime behavior. The RA2/RA3/RA4 fixture positive tests now
assert arsenal schema 6 while retaining every older trait check. RA4's invalid unit-docks
mutation now matches `, docks: 0` exactly so it cannot also match the new `vehicle_docks: 0`;
its exactly-one-edit and rejection assertions are unchanged. These three targets are rerun
before the final full gate. All other simulation targets, including the fixture AI sweeps and
all new RA5 behavior tests, passed. No runtime code changed after the winning attempt-four
production sweep.


**Exact intended test-expectation ledger (RA5-B).** These are explicit expectation migrations;
all other checks inside the named tests remain, and no test is deleted or ignored.

- `crates/sim/tests/ra1_definitions.rs::every_field_of_the_arsenal_file_is_required` uses
  schema 6, adds all thirteen required fields, and anchors `, docks` to avoid matching
  `vehicle_docks`. `unknown_and_duplicate_fields_are_rejected_in_every_file` uses schema-6
  mutation input; `other_schema_versions_are_rejected_in_every_file` rejects the old version 5
  and future version 7. Their `arsenal_of` fixture helper emits schema 6 as well.
- `crates/sim/tests/ra2_definitions.rs::the_fixture_loads_with_the_traits_of_schema_version_3`,
  `ra3_definitions.rs::the_fixture_loads_with_the_traits_of_schema_version_4`, and
  `ra4_definitions.rs::the_fixture_loads_with_the_traits_of_schema_version_5` assert current
  schema 6 while preserving their complete older-trait assertions.
- `crates/sim/tests/ra4_definitions.rs::a_trait_of_a_unit_on_a_building_and_docks_on_a_unit_are_rejected`
  anchors `, docks: 0` so exactly the aircraft-dock field is mutated; the one-edit and invalid
  unit-trait rejection checks are unchanged.
- `crates/sim/tests/m1_map.rs::the_optional_look_property_is_presentation_data_per_cell` is
  renamed `the_optional_look_property_preserves_terrain_and_retains_world_objects`: pure-ground
  look 15 retains exact Grid equality, maximum look 255 has three exact feature-15 anchors,
  and clearing only that metadata restores exact terrain equality. Invalid byte bounds remain.
- `crates/game/tests/m13_grown_maps.rs::every_shipped_map_grows_with_every_player_count` adds
  the exact discrete-anchor apron/water branch before its existing terrain/start/symmetry
  assertions. Repeated tech/hut foundations are not repeated buildings; all other cells retain
  the original checks. The companion new `ra5_all_large_maps_have_symmetric_tech_and_real_objects_after_two_to_ten_player_scaling`
  supplies the plan's new object-specific symmetry/setup coverage; existing `m7_maps` reachability
  and mirrored-terrain tests remain unchanged.
- `crates/game/src/sim_host.rs::menu_matches_ally_the_computer_players_and_install_speed_and_scenery`
  distinguishes passable intact bridge decks from blocked civilian props, requiring the exact
  passability for each kind after the shared installer now includes both.
- `crates/game/src/skirmish.rs::the_menu_and_its_longest_texts_fit_the_smallest_window` expects
  17 distinct rows after adding Bridge Destruction; every window/text fit check remains.
  `crates/game/src/smoke.rs::a_run_passes_only_with_the_menu_laid_out_the_cards_listed_and_the_flow_checked`
  expects 75 menu buttons for that added row and keeps its flow/layout failures.
- `skirmish.rs::every_on_off_row_sets_its_option_of_the_match_and_no_other`,
  `smoke.rs::the_flows_menu_presses_choose_its_matches_and_restore_the_lobby`, and
  `crates/sim/tests/ra1_shipped.rs::the_default_match_is_that_of_red_alert_2s_lobby` extend their
  option expectations to default-on bridge destruction without changing existing options.
  `skirmish.rs::every_production_data_file_loads_passes_cross_validation_and_is_licensed` adds
  the new original crate texture to the exact known asset inventory.

The unchanged `every_hd_unit_facing_and_building_has_team_accents` test was fixed by generated
art, not by changing its 32-pixel requirement. The final gate has 1,018 static test attributes
(984 at RA5-A closure), 1,024 runtime passes because shared modules execute in multiple targets,
zero failures and zero ignores. All thirteen commands passed on source `6d6de28`; only closure
documentation and validation records follow that source. Duel wall time is the runner's recorded
19,122 seconds, not a CPU-time measurement.

### R71. RA6-A specialist module boundaries precede behavior changes

**2026-10-14.** Commit `c8a394f` moves the original `special.rs` functions into
`special/{state,commands,interaction,transport,advance,repair,resolve,reconnaissance,carrier}.rs`.
This is a code-move-only commit: all 38 extracted function bodies compare identically after
whitespace normalization, and the facade keeps the same entry points and schedule ordering.
Before any RA6 behavior edit, `cargo check -p sim` passed, and the unchanged `m9_arsenal`
(24 tests) and `m11_repair` (4 tests) passed. RA6-A adds infantry and radiation modules behind
that facade; the later RA6-B vehicle/structure work uses the same boundaries.

### R72. Explicit specialist schemas, size bound and expectation migrations

**2026-10-14.** Arsenal schema 7 carries independent deploy/C4/disguise/immunity/teleport,
stolen-tech and Tesla traits, plus `UndeployDelay`. Combat schema 4 carries the warhead's
Temporal, MindControl, PsychicDamage, Radiation, Parasite, IvanBomb, BombDisarm,
ElectricAssault and MakesDisguise flags, and weapon RadLevel, AreaFire, Suicide and naval-only
layer restriction. The latter is derived from a projectile with neither AA nor AG; it still
engages surface-water units through this simulation's ground layer. All legacy and RA1–RA5
fixtures explicitly supply zero for the new fields. Worlds without rules retain their M9
specialist paths. The generator remains the only author of shipped data; the committed
reference table is unchanged, and `ra9_data_parity` compares every new field and guards the
complete weapon/warhead/item field inventories.

The arsenal grows from 126,827 bytes beyond the former 131,072-byte ceiling to 148,986 bytes (145.5 KiB). The explicit new maximum is 192 KiB, not an unbounded parser exception. The existing
oversized/special-file test now also loads a valid file padded to exactly that bound, then
rejects one extra byte before parsing. `ra6_definitions` requires every new flag, rejects -1/2,
and proves each affects the hash; new numeric fields retain explicit upper bounds.
PsiWave's reference Range=1 is permitted only for AreaFire weapons (minimum 256 subcells);
ordinary weapons retain their 384-subcell minimum, with negative tests for both branches.
MakeupKit's negative range sentinel is represented by the bounded maximum, never fired:
visible infantry copying uses Interact. Sapper and ASW's sea-only projectiles use the ground
layer with the independent naval-only restriction. Missing Verses on the non-damaging
BombDisarm/IvanBomb utility warheads defaults to 100%; their handlers never apply it
as damage. These adaptations are documented in the precise remaining-difference rows.

**Intended expectation migrations, retaining every other assertion:**

- `m4_definitions::the_fixture_loads_with_every_field` extends the exact weapon value with
  default specialist fields; helper `generated` emits combat 4 and explicit zero traits.
  `schema_versions_other_than_3_are_rejected` is renamed
  `schema_versions_other_than_4_are_rejected`, with 3 rejected and 4 accepted.
  `unreadable_oversized_and_special_files_are_rejected` adds exact-limit acceptance while
  retaining over-limit rejection. Historical `every_field_of_schema_version_2_is_required`
  still checks its original field blocks; its name describes the trait generation.
- `ra1_definitions::every_field_of_the_arsenal_file_is_required` adds all fourteen traits and
  uses arsenal 7, as do helper `arsenal_of`,
  `unknown_and_duplicate_fields_are_rejected_in_every_file`, and
  `other_schema_versions_are_rejected_in_every_file`. All malformed-field checks remain.
- `ra2_definitions::the_fixture_loads_with_the_traits_of_schema_version_3`,
  `ra3_definitions::the_fixture_loads_with_the_traits_of_schema_version_4`, and
  `ra4_definitions::the_fixture_loads_with_the_traits_of_schema_version_5` assert current
  arsenal 7; the last also asserts combat 4. Their historical names describe retained trait
  generations, not the current file schema.
  `ra2_definitions::a_trait_of_a_building_on_a_unit_is_rejected` becomes
  `building_only_traits_on_units_are_rejected_but_unit_build_limits_are_supported`: unit
  BuildLimit=1 is accepted and checked; every genuinely building-only trait still rejects.
  `traits_outside_their_ranges_are_rejected` retains its numeric build-limit bounds.
- The reference difference rows for implemented infantry mechanics are removed. The Engineer
  row now records only the unused VirtualScanner; the radiation effect's normalized static
  damage is 1 because every live application supplies its computed cell damage. C4's effect
  uses reference MaxDamage=10000. RA6-B rows remain for prism support, sonic weapons and paradrop.

No tests are removed, ignored, weakened or given larger simulation tick budgets.

### R73. RA6-A infantry commands, provenance and temporal ordering

**2026-10-14.** GI deployment selects Para/elite Para, holds its registered spot, rejects
movement while deployed and adds no armour modifier. D toggles it. Requiring explicit D to
undeploy, rather than implicitly undeploying on Move, is a control interpretation of the
plan's “cannot move while deployed”; reference engine order handling has not been verified
and this remains an explicit simulation assumption. Engineer Interact defuses
Ivan charges, including in the Commands stage of their due tick. Dog bites use ParasiteDog's
infantry targeting and instant removal with source kill credit. Spy copies an infantry type,
or initially uses its side's GI/Conscript. Undetected enemy spies render that type with the
observer's friendly paint; their true identity remains absent from targeting, AI and hover
information. DetectDisguise requires a live detector and, for a building, power; in the
absence of a separate reference range key, effective Sight is the documented range assumption.
Visual appearance and paint require user verification.

Each infiltration consumes the spy and applies exactly one effect: refinery half current
credits; power blackout 1333 ticks; superweapon charge reset; future infantry/war-factory
vehicle training; radar shroud reset (powered satellite prevents it); or Allied/Soviet stolen
tech. Training calls `sim::promote` only from the paid-production completion branch, never a
blanket spawn-event listener. Ships and free/scenario/crate-style spawns stay unpromoted.
CCOMAND/PTROOP require Allied barracks, CIVAN/YURIPR Soviet barracks; the two stolen flags gate
the correct pairs. Unit build limits count live ownership, passengers and pending units plus
queues, refreshed at command boundaries, so two same-tick queues cannot evade Yuri Prime's one.

Tanya's amphibious movement and Sapper 2500 naval attack are independent of delayed C4:
Interact plants Super/MaxDamage after 36 ticks on CanC4 buildings. An immune bridge hut is the
specific appendix-D exception: C4 collapses its linked bridge only when bridge destruction
is enabled, and leaves the hut. Friendly transport and hospital entry take precedence over
spy copying or Ivan planting; amphibious infantry can enter their hospital on land.

Temporal tracks 10×Strength remaining, subtracts three quarters of the sum of active normal8 /
elite16 beam rates each tick, freezes and protects the target, and releases it unharmed when
all beams stop. Combat establishes new freezes before that tick's ordinary impacts; a later
attacker in the same pass cannot act after being frozen. Frozen sources cannot sustain beams.
Removal is marked Temporal so no crew/death explosion follows. Chrono infantry movement uses
integer distance/48 frames converted to ticks, with the converted 16-frame minimum. Teleports
remain excluded from AIView's observed stepped movement extension.

Idle adjacent Tesla Troopers supply coil charge: two allow low-power operation, three select
OPCoilBolt, or one selects OPCoilBolt with full base power. Ivan charges use IvanWH after600
ticks, normal400 and the documented elite600 interpretation. IVAN and CIVAN remain
Trainable=no: the elite test injects scenario rank to exercise the otherwise unreachable
ElitePrimary, without claiming normal promotion reaches it. Yuri holds one target, an explicit
new attack releases the old target, and death restores the true original owner even through
multiple controllers. Automatic acquisition does not churn an existing controlled unit.
Psionic immunity also covers PsychicDamage; deploy uses PsiWave250/PsiPulse and its weapon
reload, with automatic undeploy at200 ticks (Yuri Prime100). Repeat deploy toggles cannot
bypass weapon reload. Desolator and radiation details follow in R74.

The new shipped command matrix covers hidden/unseen/missing Interact targets and specialist
Deploy/Interact after same-tick lethal commands. Transport tests exercise specialist entry,
identity/health/rank restoration and limited passengers. The existing M9 tests stay untouched.

### R74. Active radiation sites and the infantry death-weapon handoff

**2026-10-14.** Desolator keeps RadBeam125/67 ticks and, deployed, immediately creates a
RadEruption500 site, then may re-fire every80 ticks only below one third of that cell's level.
Radiation stores active sites only and visits living units, never every map cell per tick.
Spread is truncated to whole cells; integer radial attenuation is applied before the per-cell
step loss is truncated, as appendix D13 requires. The diagonal cell of a spread10/level500
site starts429 and becomes344 after its first120-tick decay, not343. Contributions sum and
cap at500; duration is converted from level×RadDurationMultiple frames; global application is
every21 ticks through RadSite with level×0.2 base damage, preserving immunity and excluding
buildings. Expired sites are pruned; same-centre/same-spread re-irradiation adds the remaining
centre level, caps it and restarts duration. Empty radiation returns before cloning definitions.
Infantry helpers reuse the caller's definitions instead of adding deep clones each tick.

The new `radiation::add` and source-bearing site representation are the RA6-B payload entry
point. `infantry::finish_deaths` is the current infantry death-weapon dispatcher: Terrorist's
225 blast uses RA3 `Blast::source`, occurs once for suicide or ordinary death, and never for
Temporal. RA6-B extends this to vehicle/building payloads, cargo loss, crashes and IFV modes;
those mechanisms are not claimed complete here. Optional radiation source presence is itself
hashed, so no source and source SimId(0) differ. All timers, charge, erasure, disguise, sites,
control and infiltration/build-limit state are ordered and hashed. R67's ship-blocked bridge
repair remains an explicit final difference, and no RA5 AI repair/assembly fix is removed.

### R75. Specialist weapons participate in the presentation inventory

**2026-10-14.** The first frozen RA6-A gate on372d48c passed commands1–3 and5–8,
but command4 stopped at the unchanged
`weapon_fx::tests::the_shipped_combat_file_classifies_light_and_electricity`: newly ported
MakeupKit had no explicit visual class. The complete new weapon inventory now maps AssaultBolt
to Tesla, RadEruptionWeapon to Radiation, PsiWave to Psychic, and MakeupKit/DefuseKit/Sapper/
FakeC4 to Trigger. Utility interactions never emit ordinary fire events, but their definitions
still have an explicit class. The existing test and its no-fallback assertion are unchanged.
The failed gate logs remain in `docs/validation/ra2-parity/ra6a-attempt1`; the full required
gate restarts on the corrected source. Visual/audio acceptance still needs user verification.

### R76. Utility weapons do not recruit specialists into AI armies

**2026-10-14.** The second frozen gate on `ef7298d` failed command 4 after 5,045 seconds:
Coral Archipelago, seed 0, Libya (brutal) versus Iraq (medium), tick 105851. Unit 566 was a live
P0 Engineer with 9 HP, with no mind control, Temporal or deployment state. The exact pre-fix
queue contained bridge-hut Interact(unit 566,target 2684394882), then AttackMove(unit 579), then
AttackMove(unit 566). Repair consumed the Engineer before the conflicting army order. Porting
DefuseKit exposed the old AI heuristic “any anti-ground weapon means combat armed”.

Army weapon eligibility now excludes zero-damage, deploy-only AreaFire, BombDisarm,
ElectricAssault and MakesDisguise utilities. Engineer and Spy keep their specialist jobs;
Tesla, Yuri, Temporal, Ivan, Tanya and the stolen units retain genuinely offensive weapons
(including FakeC4's normal 5,000-damage weapon). Frozen/deployed units are also excluded from
current army command candidates and rejoin after release; no new persistent AI state is added.
The original strict rejection assertion in `m6_production_matches::run` is unchanged.
`ra6_coral_archipelago_brutal_libya_medium_iraq_seed_zero_finishes` reproduces the failure
before the fix and wins after it in 60,119/288,000 ticks. New shipped eligibility and transient
state tests are `ai::tests::ra6_shipped_utility_weapons_do_not_recruit_specialists_into_the_army`
and `ra6_deployed_and_temporally_frozen_units_wait_for_release_before_army_orders`.
No existing test expectation or simulation tick limit changes.

`m14_duels` accepts optional diagnostic `RA2_DUEL_SEED` and `RA2_DUEL_PAIR` selectors to replay
that exact match (`0`, `libya:brutal,iraq:medium`, arguments `full coral_archipelago trace`).
With neither set, the required full sweep still uses seed 14 and every existing pairing.
The example's 300,000 diagnostic ceiling is separate from the unchanged production budget
`72000 * max(width + height,128) / 128`. Pre-fix state/queue traces, exact regression output
and the failed full-gate logs are retained in `ra6a-preflight` and `ra6a-attempt2`.

**Vision handoff, unproven risk:** neither `economy::Spawner::leave` nor `unit::spawn_unit`
initializes `SpecialState.hidden`; reconnaissance inserts concealment after Combat. A newly
produced Spy/cloaked unit in an already visible cell may therefore be targetable on its first
spawn tick. This inherited scheduling path has been inspected but an actual attack/leak has
not been reproduced. RA7 must add a first-spawn concealment regression and resolve it if
confirmed; no engine parity or security completion is claimed for that path here.

### R77. Stolen-unit denial now reports the missing infiltration

**2026-10-14.** The third gate on `05ef109` passed all 756 production matches and the AI
regressions, then found two stale assertions in `crates/sim/tests/ra1_shipped.rs`:
`an_allied_player_builds_the_allied_tree_to_the_battle_lab` expected Chrono Commando to be
`UnknownItem`, and `a_soviet_player_builds_the_soviet_tree_to_the_battle_lab` expected that
for Yuri Prime. RA6 now supplies their real tech level and stolen-tech prerequisites, so both
must remain denied with the exact `AbilityUnavailable("Infiltrate the required Battle Lab first")`
reason. Only these two expected reasons change; every existing tree, country and prerequisite
assertion remains. The RA6 shipped acceptance tests separately prove successful unlocking
and same-tick Prime limits. This is an intended expectation migration, not a runtime fix.
The failed gate remains in `ra6a-attempt3`, including its successful 756-row production timing
table. Remaining small simulation/shipped and client targets run before the next full gate.


### R78. Reference specialist traits opt in without replacing fixture roles

**2026-10-14.** The remaining-target preflight caught the reference dispatch swallowing
role-only fixture Commando, Demolition and Suicide fire and Spy refinery entry whenever a
rules file existed. Reference handling now requires its explicit weapon/metadata traits;
legacy role-only paths remain available (including suppression of direct bomb/control shot
damage), and the new infantry death hook requires Suicide
on the weapon. The unchanged `ra3_damage::a_commandos_charge_and_a_radiation_zone_go_through_their_warheads`,
`a_planted_bomb_goes_off_after_the_rules_delay_through_its_warhead` and
`a_suicide_unit_blows_up_with_its_own_weapon_where_it_stands` tests and unchanged
`ra5_ore::a_spy_in_a_refinery_takes_the_rules_share_of_the_owners_credits` are regression
proof. This is a runtime compatibility fix, not an expectation change.

Two exact schema/audit migrations are intended:
`ra3_weapons::an_army_must_attack_the_ground_and_damage_every_building_with_a_weapon_it_uses`
anchors its two one-percent mutations at the unique armour triples because schema 4 inserts
warhead traits before Versus; its one-edit guards and rejection assertions remain unchanged.
`ra3_shipped::every_shipped_weapon_deals_the_references_damage_to_every_armour_type` includes
explicit 100% defaults for only IvanBomb and BombDisarm, whose reference sections omit Verses.
It asserts those rows are absent and still checks every weapon against all eleven armours;
all other missing Verses remain errors. ElectricAssault has explicit zero Verses and is not
a default case; R72's earlier prose naming it has been corrected. Preflight logs are retained.

Final small-target preflight after the fix passes all selected simulation and client targets
(exit 0 each), including all shipped RA1–RA6 and complete data audit targets. The already
healthy heavy AI/production sweeps were excluded only from this preflight; the fresh frozen
gate runs the entire workspace again. `final-small-sim.log`, `final-small-game.log` and
`trait-opt-in-fixed.log` preserve the results alongside the failed outputs.


### R79. RA6-A validated closure and RA6-B handoff

**2026-10-14.** Frozen source `c86d33f` passes all twelve required commands: 1–8 and
10–13, each exit 0, using `validate_ra2_parity.sh ra6a 1 8` then `ra6a 10 13` with both
`RA2_DUEL_SEED` and `RA2_DUEL_PAIR` unset. Only command 9 soak is deferred, as explicitly
permitted for the first RA6 part; the complete soak remains required in the continuation.
The workspace run passes **1,061 runtime tests, zero failures, zero ignored** (1,055 static
`#[test]` declarations). The final small-target preflight also passed 56 sim targets / 788
tests and nine game targets / 247 tests. No tests were deleted, skipped or weakened; R72,
R77 and R78 name intended schema/audit/rejection migrations. Failed attempts 1–3 and
preflight/mutation evidence are preserved. No independent review is claimed.

Both production and fresh default-seed sweeps have **756/756 winners**. Production takes
4,840.53 seconds; its unchanged `72000 * max(width + height,128) / 128` budget ratios are
median **0.1323229167**, nearest-rank p90 **0.3186944444**, maximum **0.7095694444**.
Fresh duels take 4,625 seconds; against that same production budget, ratios are median
**0.1390937500**, p90 **0.3121031746**, maximum **0.8229027778** (Demo, America easy versus
Korea easy, 59,249/72,000 ticks). Their slowest absolute finish is 221,221 ticks. The
example's independent 300,000-tick diagnostic ceiling is unchanged and is not the ratio
denominator. Both complete timing TSVs and logs are in `validation/ra2-parity/ra6a`.

Workspace testing took 6,089 seconds versus RA5's recorded 5,579; fresh duels took 4,625
versus 19,122. These are observed wall times, not a controlled performance comparison or
causal claim. The shipped benchmark remains within its unchanged budgets: mean 0.268 ms,
longest 1.674 ms, peak heap 1.3 MiB. Release/debug 600-tick smokes, macOS package, generated
data and all 4,934 reference rows pass. No tick limits changed.

The acceptance ledger covers GI, Engineer defuse (including due tick), Dog, Spy disguise
and every infiltration effect, all four stolen units/limits, Tanya C4/naval/swimming/hut
behavior, Temporal release/multiple firers/chrono delay, Tesla thresholds, Ivan normal/elite,
Yuri ownership release/immunity/PsiWave, Desolator and radiation overlap/decay/edge/zero,
Terrorist death weapons, shipped transports, command visibility/dead-actor guards and hashing.
The pure-move prerequisite and its 38-body/28-legacy-test proof remain R71. RA6-B extends
`special::infantry::finish_deaths` and `special::radiation::add` for vehicle/building payloads,
and completes IFV modes, structures, superweapons, crashes/cargo loss and paradrops. Veteran
infiltration belongs only to paid production. Preserve RA5 bridge/AI fixes and R67's
ship-blocked bridge repair assumption. R76's first-spawn concealment path is an unproven
RA7 risk requiring a reproduction test, not a demonstrated defect. The wider RA7–RA9 debts
and independent review remain open. Explicit R72/R73 assumptions and user visual/audio/play
verification remain; validation does not substitute for that verification.

**Process deviation:** `c86d33f` omitted the mandatory `RA6:` commit-subject prefix. Its coherent
content is preserved without amendment/rewrite; this closure and subsequent RA6 commits use
the prefix. No new substantial external directory, clone or worktree was created. The
configured Cargo target (including package outputs) was reused and remains needed for
validation/package evidence; UV/PIP caches and RA5's render directory were preserved.

### R80. RA6-B reference vehicles, strategic payloads, and explicit state

**2026-10-15.** RA6-B adds new `special/vehicles.rs` and `special/superweapons.rs`
mechanics; it does not perform a further extraction of existing bodies. R71's isolated
pure-move commit remains the prerequisite. Arsenal schema 8, combat schema 5, and rules
schema 3 migrate every committed legacy/RA1–RA5 fixture and the generator. Required fields
include all thirteen IFV weapons, death weapons, Gunner/Explodes/DisguiseWhenStill,
SuperWeapon/ShowTimer, IsSonic, repair-only negative damage, Paralyzes and Culling. The
existing 192 KiB pre-parse arsenal bound remains unchanged. Death-only NukePayload lacks
ROF, and OilExplosion lacks ROF and Range: the exact permitted source absences are audited;
unused bounded scheduling placeholders are one tick and 384 subcells. Ordinary weapons
retain the complete ROF/Range audit. Negative damage is accepted only for explicit repair
weapons; ordinary negative-damage variants remain rejected.

`Paralyzes=32767` is retained as its exact numeric source value, not relabeled as a source
boolean. A positive value enables host movement paralysis for the attached parasite's
lifetime; no arbitrary 32767-frame timeout is substituted. The ship keeps its weapon.
Squid culling checks the post-hit red/yellow threshold, honors Curtain/Temporal protection,
and records its source kill credit. Drone and Squid retain identity, owner, HP and experience
off-map. A self-kill or sonic eviction uses the shared ring allocator; a blocked exit retains
an explicit hashed pending cell and retries. An externally destroyed host loses its parasite.
A blocked escape still earns kill experience. Repair removes a Drone, and RepairBullet uses
its exact squared subcell range and reload. IFV mode 12 is tested with an explicit independent
mode fixture because the representative civilian roster does not include cows.

Prism support walks ordered ready same-owner towers through secondary range links, takes
at most eight donors, applies `1 + 1.5*N`, and disables donors for 80 ticks. Readiness includes
the cooldown-one boundary before combat decrements it and excludes Temporal-frozen towers.
Mirage uses an original procedural tree silhouette per observer and a hashed SimRng variant;
DetectDisguise is required even for adjacent ordinary units. No reference art is copied.
SonicZap damage is now the exact 4/8 source value and frees Squid. **Remaining difference:**
this deterministic shot model applies that value once per shot, without the original engine's
persistent sonic-wave frame accumulation; the available plan does not establish its lifetime.
This is an explicit approximation, not a claim of sonic-wave parity.

Death payloads use RA3 `Blast::source`, effects and radiation. The ordered dispatcher repeats
after bridge/garrison cleanup until no additional payload is discovered. Each actor detonates
at most once. Stored transport cargo dies with its host and does not independently explode; Ivan IFV
instead uses its mode-7 CRNuke payload. Kirov uses its primary as its ground crash blast, explicitly generated into death metadata
from BalloonHover/Primary so legacy role-only fixtures do not opt in. Garrison evacuation
retains RA5 semantics and is revisited within the cleanup fixed point.
Temporal removal suppresses payloads and crew. These are deterministic cleanup policies,
including deaths occurring after the ordinary Combat stage.

Chronosphere has a distinct validated source/destination command and two-click client flow.
It vacates the whole 3x3 selection together, preserves offsets, kills organic infantry except
Teleporters, and kills invalid/occupied destination units instead of violating I6'. Moved units
freeze for 80 ticks. Curtain affects all owners in its 3x3 footprint-intersection area, kills
infantry and protects remaining targets for exactly 1000 ticks. Nuclear payload flight is a
200-tick game timing assumption; the 600 NUKE payload and 500 radiation are ported data.
Storm warns for 333 ticks, then runs for 240 with 13/7-tick direct/random attempts, a 10x10
square, source damage and enemy radar loss. Direct bolts ignore separation, random bolts
respect every active cloud including the fixed target. **Cloud lifetime assumption:** one
direct-bolt interval (13 ticks); no precise cloud animation lifetime is established by the
reference contract. Storm exclusivity includes the warning period.

Paradrops use one ordinary 400-HP aircraft, vulnerable to AA, entering from a legal map-edge
cell. The side payload is six GI/nine Conscripts; America receives eight GI. Drop attempts
begin within four cells. **Landing/exit approximation:** infantry materialize on legal spots
within the target radius without a parachute fall simulation, and the aircraft exits when its
payload is placed. If landing is blocked it retains the undispatched count and retries; AA
loss discards the remainder. Free troops do not inherit paid-production spy veterancy.

Public built/launch messages and timer views expose only owner, role and remaining time,
never an unseen source id/cell or target. Readiness is shown/audible only to its owner.
Nuclear target marks are owner-visible or require a powered Psychic Sensor within its ported
15-cell radius; ordinary vision alone does not reveal that warning. Visual/audio feel still
requires user verification.

Regression migrations retain their assertions: `m4_definitions::schema_versions_other_than_4_are_rejected`
is renamed for schema 5 and its unsupported-version matrix advances; generated limit
fixtures gain required fields. `ra1_commands` adds Chronoshift to every naming/rejection
matrix. Its immediate-death setup is explicitly separated as `immediate_death_fixture`,
modifying only silo `strategic=0` and IFV `gunner=0`/empty mode list. It is used only by
`strike_beside_a_repair_ifv`, `ra5_same_tick_dead_actors_cannot_capture_repair_enter_hospital_or_occupy`,
`ra5_garrison_destroyed_earlier_in_the_command_tick_cannot_deploy`, and the renamed
`ra6_trait_injected_specialists_and_transport_passengers_cannot_act_after_same_tick_death`.
Those tests exercise unchanged same-command-stage death guards, not actual shipped nuclear
flight or RepairBullet. `shipped_world` remains the unmodified shipped ruleset for the
visibility/security matrix; `ra6_vehicles` separately exercises actual strategic/gunner paths,
including Curtain's immediate infantry death. Legacy m9_arsenal and m11_repair remain untouched.

### R81. RA6-B exact existing-test expectation ledger

**2026-10-15.** The following changes migrate an intended contract; they do not remove or
relax assertions. New positive/negative/boundary tests run alongside them.

- `m4_definitions::the_fixture_loads_with_every_field` supplies explicit false Sonic/repair
  fields in the expected WeaponDef. `schema_versions_other_than_4_are_rejected` becomes
  `schema_versions_other_than_5_are_rejected`, rejecting old 4 and future 6 as well as the
  previous unsupported values. Its `generated` helper (used by `list_lengths_are_bounded`)
  writes schema5 and all required neutral fields.
- `ra1_definitions::the_rules_fixture_loads_with_every_field`,
  `every_field_of_the_rules_file_is_required`, `every_scalar_rule_is_rejected_one_step_outside_its_range`,
  and `the_limits_of_every_scalar_rule_are_accepted` extend `SCALAR_GROUPS` with both required
  strategic timing fields and their 1..72000 bounds. Every fixture retains the parser helper's
  one-field-per-line shape. `every_field_of_the_arsenal_file_is_required` advances its schema
  literals to8; `unknown_and_duplicate_fields_are_rejected_in_every_file` advances only rules3
  and arsenal8 literals; `other_schema_versions_are_rejected_in_every_file` rejects old/future
  versions against those current schemas. The `arsenal_of` helper writes schema8.
- `ra2_definitions::the_fixture_loads_with_the_traits_of_schema_version_3` and
  `ra3_definitions::the_fixture_loads_with_the_traits_of_schema_version_4` expect arsenal8;
  `ra4_definitions::the_fixture_loads_with_the_traits_of_schema_version_5` expects arsenal8
  and combat5. Their historical trait expectations remain unchanged.
- `ra5_definitions::files_of_schema_version_1_are_rejected` expects rules3 while still
  expecting economy2; both continue rejecting schema1.
- `ra1_commands::a_seen_enemy_is_reported_as_not_owned` extends its exact name-to-result
  matrix with Chronoshift. The shared `naming` helper extends
  `every_command_answers_for_an_unseen_enemy_as_for_an_id_that_names_nothing` and
  `every_command_answers_for_a_submerged_enemy_as_for_an_id_that_names_nothing` with the
  same new command. `the_chronosphere_answers_for_an_unseen_enemy_vehicle_as_for_an_id_that_names_nothing`
  preserves all unseen/missing rejection assertions and uses explicit source/destination
  Chronoshift for its positive own-vehicle move. R80 names the four scoped immediate-death
  fixture users and the renamed specialist test; other shipped helpers retain original data.
- `ra3_shipped::shipped_ships_follow_their_naval_targeting` changes only Dolphin's expected
  heavy-armour hit from80 to3: source Damage4 times80%, truncated, replacing the deferred M9
  Damage100. Squid's first periodic hit remains15. The `strike` helper used by
  `a_nuclear_strike_destroys_a_power_plant_and_not_a_construction_yard` now waits exactly200
  ticks for actual flight; every original footprint/armour/damage assertion remains.
- `ra4_shipped::the_chronosphere_does_not_move_a_launched_rocket` retains the exact
  NotCommandable denial and rocket impact assertions; only its successful tank relocation
  uses the new area command. Area selection itself also excludes launched units and paradrop
  aircraft. Temporal-erased targets are untargetable by Chronosphere/Curtain.
- `sidebar::tests::a_tab_lists_only_what_a_building_of_the_player_could_build` identifies the
  same four *major* superweapons for the lobby-off assertions. The new Paradrop role remains
  an ability but does not disable America's radar/airfield when the major-weapon switch is off;
  the existing comparison that every other card is unaffected remains intact.
- `ra9_data_parity` expands its independent inventories and complete field guards for the
  plane, all IFV/death weapons, exact elite-mode equivalence, strategic recharge/timers,
  Psychic Sensor radius and the new schema traits. It permits only the named, verified
  absent death-weapon scheduling keys. No ordinary weapon/source field is omitted.

The added long-game win-condition test verifies that an attached or pending parasite retains
its living owner, like other off-map transported units. Short-game building/MCV rules remain.
A pending self-kill escape may use the host's cell once cleanup frees it, after trying the
ordinary ring, preventing permanent entrapment by an occupied ring on otherwise legal terrain.
R76's first-spawn Spy/cloak visibility risk remains an explicit RA7 handoff, not closed here.

R81 additional timing migration: `ra5_world::ra5_one_area_blast_hits_a_connected_bridge_once_at_its_strongest_deck_cell`
waits the actual200-tick nuclear flight, then retains its exact1452-HP assertions for every
connected bridge cell. This changes observation time, not the one-blast/strongest-cell rule.

RA6-B freeze preflight: the complete sim pass reported828 passed/1 failed/0 ignored (only
the bridge test's old immediate-strike observation); complete client/asset preflight reported
254 passed/1 failed/0 ignored (only the lobby-card major-weapon classification). Both exact
failures were corrected and rerun successfully, with all original substantive assertions.
The final security/repair target run passes50 tests (21 command +29 vehicle/SW), and final
strict workspace Clippy, formatting, diff hygiene and generator/reference checks pass.
All prior failure logs remain under `validation/ra2-parity/ra6b-*`. This is preflight evidence;
full frozen-source gate results follow separately, including soak and both756-match sweeps.

### R82. RA6 gunner AreaFire execution and interrupted gate (2026-10-15)

The first RA6 gate on041b1fc was deliberately stopped during command4 compilation after
commands1–3 passed. Root inspection found that mode8 selected CRMindControl correctly, but
`combat::weapon_against` rejected every AreaFire weapon. Mapping-only mode coverage therefore
did not establish firing behavior. Its incomplete logs remain in `validation/ra2-parity/ra6-attempt1`;
this attempt is not a completed gate. No long duel sweep from that attempt was started.

The combat selector now permits AreaFire only for an actor whose reference metadata says
Gunner. Normal hostile acquisition, range, cooldown, veterancy and RA3 source credit apply.
The blast originates at the vehicle, has no direct-target override, and uses PsiPulse's
ordinary psychic damage, armour, immunity and friendly-fire rules. It does not control units.
Yuri's non-gunner deploy-only secondary remains excluded from automatic fire. This follows
[ModEnc AreaFire](https://modenc.renegadeprojects.com/AreaFire), which describes detonation at
the firer's cell and the infantry deployment limitation. No schema or generated-data change
is necessary. The Chronosphere first-click hint now explicitly names the3x3 source area.

New command/schedule tests `ra6_ifv_psi_pulse_attacks_from_vehicle_with_immunity_and_reload`
and `ra6_gunner_area_fire_does_not_enable_yuri_automatic_deploy_pulse` exercise actual pulse
fire, one-cell acquisition versus two-cell non-acquisition, vehicle-centred splash, no friendly
acquisition, immune Yuri, source XP,13-tick reload and the infantry negative case.
`ra6_ifv_temporal_and_suicide_modes_execute_on_attack` exercises living modes7/10/11:
actual CRNuke/CRTerrorBomb suicide attacks and the exact572-tick Rhino erasure
(Strength400*10 divided by floor(Damage10*3/4)). These supplement selection/unload tests,
not replace them. Pulse timing fixtures remove return-fire Weapon components and raise the
victim's HP explicitly while keeping shipped definitions. Initial failures from return fire
and too-short Temporal observation windows are retained in `ra6b-ifv-pulse-fix1/2.log`;
the corrected focused cases pass in `ra6b-ifv-pulse-fix3.log`. No existing test assertion was
weakened or removed. The combined acceptance ledger now distinguishes mapping and execution.

Sonic confidence clarification: [ModEnc IsSonic](https://modenc.renegadeprojects.com/IsSonic)
documents a20-frame persistent wave and repeated damage, but labels its damage analysis as
Yuri's Revenge executable behavior and warns other games can differ. Vanilla RA2 lifetime,
path coverage and accumulation have not been established here. The4/8-per-shot implementation
remains an explicitly recorded approximation, not a claim that no wave reference exists or
that vanilla persistent-wave parity has been achieved.

R82 integration preflight passes125 tests,0 failed,0 ignored across `ra6_vehicles`,
`ra6_infantry`, `ra1_commands`, `ra3_damage`, untouched `m9_arsenal` and `m11_repair`.
The first integration log also preserves a range-fixture failure caused by its friendly GI
shooting the distant victim before return fire was disabled; the corrected fixture disables
that Weapon before the observation, and retains the exact2000-HP negative assertion.

R82 client/lint preflight: strict workspace Clippy passes; the game binary's208 tests and
six core integration tests pass. `ra6b-ifv-client.log` preserves an invalid `--lib` invocation
(the package is a binary). The corrected package-wide invocation's partial log is retained
as `ra6b-ifv-client2.log`: it was stopped during the unchanged multiplayer integration after
recognizing that it would duplicate the forthcoming full production sweep. It is not claimed
as a complete package test pass. The final frozen gate runs all workspace tests without filters.

### R83. RA6 complete frozen validation and handoff (2026-10-15)

The combined RA6 implementation at `f9b1e55` passes all thirteen commands in
`sh scripts/validate_ra2_parity.sh ra6`, with `RA2_DUEL_SEED` and `RA2_DUEL_PAIR`
explicitly unset. Runtime, data, tests and client source remained unchanged throughout.
This closure changes documentation and retained evidence only; it is validation, not an
independent review or user approval. Full command lines, exits and times are in
`validation/ra2-parity/ra6/summary.txt`; every command exits0.

Workspace release tests:1,096 passed,0 failed,0 ignored (6,844 s), including756/756 production
winners and both exact Coral regressions. Formatting, diff hygiene, strict all-target Clippy,
release build, both600-frame smokes, package smoke, generated-data check and reference check
all pass; the reference comparison covers4,934 rows. Shipped benchmark mean0.306 ms,
maximum1.805 ms, loaded mean0.709 ms, peak heap1.5 MiB, within unchanged5/50 ms and1,024 MiB
budgets. Required soak:260 configurations,18,720,000 ticks in5,327.7 s (command5,331 s),
peak heap224.5 MiB, passed. The separate unfiltered duel sweep passes756/756 matches;
each outcome was independently counted as `Won`, not merely ended (4,524 s, longest221,221 ticks).

Match-length evidence is retained in `production-lengths.tsv`, `duel-lengths.tsv` and
`metrics.json` under the gate directory. Ratios use R29's
`72,000 * max(128, width + height) / 128` comparison limit, with nearest-rank p90:

| Sweep | Winners | Median ratio | p90 ratio | Maximum ratio |
|---|---:|---:|---:|---:|
| Production |756/756|0.1323229167|0.3186944444|0.7095694444|
| Separate duels |756/756|0.1390937500|0.3121031746|0.8229027778|

The worst production ratio is Riverlands France(easy) versus Germany(easy),51,089/72,000.
The worst duel ratio is Demo America(easy) versus Korea(easy),59,249/72,000. The diagnostic
duel runner's existing300,000-tick outer ceiling was not changed; the table consistently
compares both sweeps with R29. No limit was raised and no failed or partial log was removed.
The interrupted first gate remains `ra6-attempt1`; R80–R82 explain every earlier preflight
failure, fixture migration and the mode8 correction. The combined acceptance ledger maps
all gap-analysis section9 rows to exact evidence or named remaining differences.

**Handoff and remaining evidence obligations.** R76's first-spawn Spy/cloak concealment risk
belongs to RA7: spawning precedes Combat, so require an actual first-spawn visibility regression.
Retain paid-production-only Spy veterancy, neutral Airport inactivity until capture, separate
vehicle/air docks, R64/R69 arrival-aware bridge behavior, and R76's utility-weapon/deployed/Temporal
army filters. Chronosphere automation must use explicit `Chronoshift { building, source, target }`.

RA9 must close the explicit Squid firing-coverage gap before final review: the current
`ra6_squid_prevents_motion_but_ship_keeps_its_weapon` proves blocked movement and retained
Weapon, not an actual shot. Add a controlled shipped test that asserts host `WeaponFired`
while the parasite remains attached, with another hostile target and return fire disabled
so the Squid is not incidentally evicted. This is an evidence gap, not an observed runtime defect.

RA9's transport audit must also account for existing slot-count/category capacity behavior:
boarding uses `cargo.len()` and category/amphibious checks, lacks per-unit Size/SizeLimit
(including the appendix A gap6 LCRF Passengers12/SizeLimit6 contract), and refuses boarding an
already loaded transport. Identity-preservation tests do not establish complete capacity/size
parity. RA9 must implement the missing behavior or give a precise justified remaining difference;
these predate the RA6 specialist changes. Preserve R67's blocked-bridge ship repair difference.

R80/R82's Sonic per-shot approximation (YR wave documentation is not established vanilla
RA2 accumulation),13-tick cloud lifetime, instantaneous landing/plane exit policy, representative
cow omission and RA6-A assumptions remain explicit. Original visual/audio/play feel requires
user verification. No new substantial external directory, clone or worktree was created;
existing configured Cargo target/test scratch and UV/PIP caches were preserved. Build cache
remains useful; the app/archive are retained under the existing
`/Users/xingyuwang/Library/Caches/Codex/cargo-target/package/` for delivery and validation.
No substantial /tmp artifact was created. `main` remains8230a95. RA7 may begin
only after root checks this closure; RA8/RA9 and the independent final review remain outstanding.

### R84. RA7 shroud-only matches, concealment timing and reconnaissance

**2026-10-15.** Actual match setup installs `MatchOptions::fog_of_war`, default false, into
hashed MatchRules and Vision. `Vision::is_visible`, bodies and all existing consumers treat
explored cells as visible when fog is off: normal enemies remain rendered, targetable,
commandable and known to AI. With fog on, the current-sight behavior remains. SpecialState's
per-observer concealment is independent. **Compatibility:** `SimPlugin` alone initializes
scenario-only MatchState with fog true, retaining the older fixture/scenario contract; this
is distinct from the RA2 match-setup default. The continuation's explicit explored-is-visible
contract supersedes the stale RA7 plan test sentence saying an explored enemy stays hidden.

Gap uses the committed `[GAGAP] GapRadiusInCells=10`, a clipped squared-distance circle,
and clears both current and explored bits every tick for hostile players. Allies are excluded;
local living sight reveals again after shrouding, while Satellite alone cannot counter Gap.
Legacy no-rules worlds retain the M9 concealment method and fixture behavior. Spy Satellite
continues to require its actual powered operating state and suppresses radar infiltration.
Psychic Sensor uses radius15 for disguise detection and provides `psychic_orders(viewer)`:
ordered source/destination coordinates only, no actor IDs; own/allied, dead and specially
concealed sources are excluded. Purple client lines consume this owner-scoped query. Boundary
15 is inclusive;16 is outside. Nuclear marks retain RA6's independent owner/sensor filtering.

**R76 reproduced and fixed:** with prior P0 vision, a newly spawned shipped P1 Spy at12,10 was
accepted by Attack and fired upon in its first tick. `first-spawn-before2.log` retains that
failure. Concealment refresh now runs before command batches, after abilities and before
Combat following production/movement. It borrows Definitions through resource_scope instead
of adding deep clones. Tests separately exercise command rejection, uncommanded submarine
acquisition and actual paid Spy production on its exact spawn tick. Living/powered building
detectors and non-airborne cloak sensors are required. Power totals exclude buildings with
zero HP in ruled worlds, closing the relevant stale-generator contribution before Economy.
No claim about unrelated destroyed-actor production or other remaining RA9 debt is implied.

Arsenal9 adds bounded required `guard_range` (0..64 cells), generated from per-type GuardRange;
zero explicitly means fallback to the ported primary weapon Range. Every legacy/RA1–RA5
arsenal fixture and all parser/audit helpers migrate together. Gap radius and guard range
are independently checked by `ra9_data_parity`, and the field inventory includes guard_range.
Shipped data remains generated only from committed reference tables; the192KiB ceiling is
unchanged. No reference ini/text/art/sound enters the repository.

### R85. RA7 orders, controls and original rate-limited announcements

**2026-10-15.** New commands are Guard, Scatter, Follow, ForceFire, ForceMove and QueueMove.
QueueMove is a separate explicit queued-Move variant, avoiding a default-bearing change to
every older Move constructor. Each shares actor ownership, bounds, liveness and hidden/missing
validation. Follow/ForceFire entity targets have complete missing/unseen/concealed rejection
coverage, including empty/unarmed selections; friendly ForceFire targets are permitted.
Guard/Follow/waypoint maps and ForceFire's combat order are hashed in ordered form. At most32
pending waypoints per unit are accepted atomically. Stop and successful replacement commands
clear prior tactical state; successful Interact/Deploy/Undeploy also clear it, as do specialist
clear_orders paths for teleport/transport. A Follow forgets an unseen or concealed target and
never updates its route from that target's hidden position.

Guard is anchored where ordered and selects hostile visible targets inside the reference
GuardRange; when absent, primary weapon Range is the explicit fallback assumption. It returns
to the anchor when idle. Ordinary idle acquisition is suppressed while this area guard has no
eligible target, so long-range V3 fire cannot bypass its explicit9-cell GuardRange. The engine's
undocumented generic guard/chase defaults are not claimed byte-for-byte equivalent. Scatter
assigns deterministic nearby directions by SimId and asks the ordinary navigation allocator
for free destinations. ForceMove shares ordinary move/navigation/crush behavior: the game's
existing Move already permitted hostile crushing. It does not acquire combat targets en route.

ForceFire has a distinct AttackOrder and genuine ordinary weapon choice, reload/burst,
projectile, RA3 warhead, friendly splash, source credit, ammunition and surfacing paths. Ground
shots use reserved SimId(u32::MAX), which the allocator never emits; it is not inserted into the
automatic target index. Ground projectile aim is fixed. Launchers derive ground aim from the
hashed ForceFire order and launch real missiles/aircraft; flight retains its independent aim
after the source stops. Entity orders retain normal special concealment checks as targets move.

Q/W/E/R select tabs; K repair,L sell,G guard,X scatter,F follow,Z held waypoints,S stop,D deploy,
T type,P all combat,H home,N/M next/previous, Alt+digit centre group, Ctrl/Cmd+F1–F4 set and
F1–F4 jump bookmarks. Ctrl+Shift click wins over Ctrl force fire, Alt force move and Z queues;
A retains attack move. Global rally moves R→Y, pause P→Pause/F9/backslash, mute M→F10/apostrophe,
and the old E interaction mode moves to I because E is Infantry. MacBook bookmarks also use
Alt+arrows, with Ctrl/Cmd+Alt to set (up/right/down/left =1/2/3/4). Modified arrows do not pan.
README has a prominent previous/current table. F issues a Follow order, as continuation Step3
requires, superseding appendix D18's camera-follow-only description. Context right click and
cursor labels use the same read-only interaction validator as simulation execution, including
Engineer/Spy/transports/garrisons/depot/hospital/Tanya buildings. Ship right clicks keep the ordinary
Attack path, which selects Tanya's already ported naval Sapper weapon.

Thirteen original synthesized motifs plus text cover unit ready, construction complete,
on hold,cannot deploy,base attack,unit lost,low power,insufficient funds,superweapon built/ready/
launch,radar on/off. Owner-only events remain owner-only; public SW notices contain no hidden
source IDs/positions. A bounded queue, eight-second category cooldown and one-second global
spacing limit repetition. No audio file or recording is added; LICENSES records the source.
All visual/audio/play feel requires user verification.

**Exact intended existing-test migrations (no deleted/ignored/weakened tests):**
- `ra1_definitions::every_field_of_the_arsenal_file_is_required` adds guard_range;
  `unknown_and_duplicate_fields_are_rejected_in_every_file`,
  `other_schema_versions_are_rejected_in_every_file` and arsenal_of advance arsenal8→9,
  retaining old-version rejection (including8) and future10 rejection.
- `ra2_definitions::the_fixture_loads_with_the_traits_of_schema_version_3`,
  `ra3_definitions::the_fixture_loads_with_the_traits_of_schema_version_4`, and
  `ra4_definitions::the_fixture_loads_with_the_traits_of_schema_version_5` expect current9;
  every retained earlier-trait assertion remains.
- `ra1_commands::a_seen_enemy_is_reported_as_not_owned` extends its exact name/result table;
  naming adds all new actors, Follow/ForceFire targets and empty/unarmed target variants to
  both existing missing/unseen/submerged matrices. Existing positional checks keep their order.
- `ra1_shipped::the_default_match_is_that_of_red_alert_2s_lobby` includes default-off fog.
- `hotkeys::tests::every_listed_key_has_one_action` migrates X/R/P/M to L/Y/F9/F10 with the
  new secondary keys; Q/W become valid tabs and F5/F6 remain explicit unbound negatives.
- `skirmish::tests::every_on_off_row_sets_its_option_of_the_match_and_no_other` adds Fog/Ore
  rows and exact propagation. `the_menu_and_its_longest_texts_fit_the_smallest_window` expects
  nineteen distinct rows instead of seventeen and retains every original size/text bound.
- `smoke::tests::a_run_passes_only_with_the_menu_laid_out_the_cards_listed_and_the_flow_checked`
  expects79 buttons instead of75. `the_flows_menu_presses_choose_its_matches_and_restore_the_lobby`
  includes fog-on/ore-off first matches and restored fog-off/ore-on final defaults. Runtime
  smoke checks compare both choices to actual MatchRules. No layout, victory or timing limit changes.

R85 preflight compatibility clarifications: the exact
`ra5_ai_remembers_a_seen_collapse_and_approaches_its_hut_after_losing_sight` scenario explicitly
requests fog on; its unseen-hut memory, Engineer consumption and unchanged winner limit
assertions remain. Its companion normal-vision match retains the new actual-match default.
The early concealment refresh preserves explicit prop/garrison filters until the normal
Visibility pass, keeping `ra5_enemy_garrisons_exchange_occupation_fire_but_hidden_garrisons_are_not_targets`
unchanged. Power is refreshed again after Cleanup so lost generators/radar cannot leave a
stale operating detector in client queries. Concealment also refreshes before tactical
pathfinding, preventing Follow from reading a newly concealed target after Economy effects.
The initial small preflight's failures remain in `ra7-preflight/small-sim1.log` and
`focused2.log`; the guarded-field fixture-prefix failure remains in `focused3.log`.

R85 same-batch command regression: root identified Sensor sale followed by Attack/Follow as a
boundary worth probing. The shipped `ra7_selling_the_only_sensor_hides_spy_from_later_same_tick_regular_commands`
failed with both commands accepted after the sole detector was sold; `sensor-sale-before.log`
retains the exact sequence. Sale/placement and orders involving actual Mirage IDs now flush
the regular batch, apply deferred entities, recompute live power and refresh concealment before
later commands. Normal army commands retain contiguous processing, without a per-command
Definitions clone or full unit scan. A separate moving-Mirage Stop→Attack regression checks the
concealment transition. ForceFire also shares the legal firing-position/minimum-range search,
including backing away before firing a V3, and its source-credit test asserts the exact900 XP.


R85 final client privacy regression: `ra7_visible_disguise_is_not_an_enemy_cursor_target`
reproduced a concealed Spy's true enemy identity in the cursor's hit list, despite the intended
friendly disguise rendering. The enemy hit list now excludes observer-hidden IDs while retaining
the visible disguise. `disguise-cursor-before2.log` records the failing probe. Modifiers show
FORCE FIRE/FORCE MOVE/WAYPOINT/ATTACK MOVE labels and preserve Ctrl+Shift precedence; cursor
hit targets take priority over a cell's first occupant for multi-infantry cells and Follow.
`ra7_deploy_and_chronoshift_clear_pending_waypoints` adds explicit successful Deploy and
teleport queue clearing coverage. The Gap exploration mutation must fail its edge regression;
its original source is restored before the final gate.


R85 preflight process deviation: the first expanded small-target invocation continued after
an added fixture compile correction, and its replacement was started before the first process
finished. Their existing m6_definitions scratch FIFO path collided (`mkfifo failed`) in the
first run. Both complete logs are retained; the replacement's exact result is recorded at
closure, with any affected targets rerun serially. The full frozen gate runs as one serial
invocation with no competing validator. No production code or existing test was changed for
the scratch collision.


R85 power integration correction: unchanged
`ra6_storm_warning_exclusivity_duration_and_cloud_separation_are_deterministic` exposed that
RA7's new post-Cleanup power refresh restored enemy radar during the active Storm. Power/radar
recomputation now includes the active hostile Storm's exact starts/ends interval. The original
RA6 assertion remains unchanged; the failing small preflight logs and serial passing rerun
are retained. Scratch collisions also affected m2/m3/m4 parser variants during the overlapping
preflight; the complete small suite is rerun serially before freeze.


Exact overlapping parser scratch failures: m2_definitions::{
ron_features_outside_the_subset_are_rejected_before_ron_parses,
unit_values_outside_their_ranges_are_rejected, schema_versions_other_than_1_are_rejected,
value_limits_are_inclusive}; m3_definitions::numeric_fields_accept_their_limits_and_reject_beyond;
m4_definitions::the_fields_of_schema_version_2_accept_their_limits_and_reject_beyond;
m6_definitions::unreadable_oversized_and_special_files_are_rejected (mkfifo). Independent
processes wrote the same per-target named scratch variants; ordinary within-binary parallel
cases use distinct names. The required unmodified `cargo test --workspace --release` invokes
each target once and no competing validator will run, so this cross-process collision does
not occur. No test-thread restriction, gate serialization wrapper or assertion change is added.
The serial preflight means separate Cargo invocations do not overlap, not fewer test threads.


### R86. RA7 last-stand fixture preserves its explicit current-sight scenario

**2026-10-15.** Frozen `0ef4a53`'s first full gate is preserved as `ra7-attempt1`, not a full
validation pass: workspace 6,432 s, 855 passed / 1 failed / 0 ignored. The failure was solely
`ra4_ai::two_players_who_have_spent_each_other_make_their_last_stand_and_the_match_ends`'s
unchanged lower-bound assertion `played.ticks > 4400 + LAST_STAND_PATIENCE`. Winner, upper
limit and unexplained-command assertions already passed. With seed 7, actual default fogOFF
changes observations/battle/last-stand timing and wins for P1 at tick 4,193 (4,194 iterations),
with no unexplained commands. With explicit fogON, the original scenario wins P1 at 8,221
(8,222 iterations), no unexplained commands, and every original assertion passes. Exact winner
and BuildingSold traces are retained in the last-stand diagnostic preflight logs.

The fixture `start` now receives MatchOptions; `play_with_options` runs an explicit option
set, while existing `play` retains MatchOptions::default(). Only that specific original
last-stand regression opts into fogON, preserving every assertion and the 72,000 limit.
`computer_players_finish_their_matches_with_veterans_and_survivors` retains nine actual-default
fogOFF cases, including the identical brutal ALPHA/GAMMA pairing and exact replay checks.
No runtime AI delay/cadence/vision rule changes are made to force a legitimately earlier win
to wait for the fixture's historical 4,400-tick timeline. This is an intended fixture scenario
migration, analogous to the explicit RA5 lost-sight bridge-memory scenario in R85.

First-attempt production target passed all 756 matches in 5,141.62 s, versus RA6's 5,462.06 s; its
complete timing table is retained in `ra7-attempt1/production-lengths.tsv`. Gate 1/2/3/5/6
exited 0. The failed attempt was stopped during debug smoke before expensive soak/duels;
that interruption is not a smoke validation pass. All AI fixture targets are added to the
corrected preflight before a new coherent freeze and fresh all-thirteen-command gate.


R86 corrected preflight: all seven AI fixture targets (`m6_ai`, `m7_factions`, `ra1_match`,
`ra2_ai`, `ra3_ai`, `ra4_ai`, `ra5_ai`) pass all 45 tests, zero failed/ignored. Combined with
the previous small-target coverage this is 1,119 unique runtime tests before the expensive
sweeps; the remaining eleven heavy game tests passed in attempt1 and will rerun in the full
gate. Strict Clippy remains green. The corrected freeze changes no runtime source/data.


### R87. RA7 frozen validation, measured timings and handoff

**2026-10-15. Validated, not independently reviewed or approved.** Runtime/data/client source
is frozen at `dd647f5` (implementation `0ef4a53`, explicit fixture migration `dd647f5`).
`env -u RA2_DUEL_SEED -u RA2_DUEL_PAIR sh scripts/validate_ra2_parity.sh ra7` runs all thirteen
commands unchanged on that source. Every exit is zero; 1,130 runtime tests pass, zero failed
or ignored, with 1,124 static test attributes and no ignore attributes. Source remains unchanged
through this documentation-only closure. `main` remains `8230a95`; no approval/review tag exists.
The acceptance ledger is `docs/validation/ra2-parity/RA7_ACCEPTANCE.md`.

| # | Check | Exit | Wall seconds / evidence |
|---|---|---:|---|
|1|Formatting|0|1|
|2|Diff hygiene|0|0|
|3|Strict workspace/all-target Clippy|0|0; warnings denied|
|4|Workspace release tests|0|9,495; 1,130 passed / 0 failed / 0 ignored|
|5|Release build|0|1|
|6|Release smoke|0|16; 600 frames, fog ON/OFF flow|
|7|Debug smoke|0|124; 600 frames, fog ON/OFF flow|
|8|Benchmark|0|5; shipped mean 0.337 ms / maximum 2.023 ms, loaded mean 0.769 ms; peak 1.5 MiB|
|9|Full soak|0|6,081; 260 configurations, 18,720,000 ticks, measured 6,076.9 s; peak 234.5 MiB|
|10|Separate full duels|0|6,816; 756/756 Won, longest 157,564 ticks|
|11|macOS package|0|26; app/archive in configured Cargo target/package|
|12|Generated data|0|0; generator check|
|13|Reference table|0|0; cached reference matches all 4,934 committed rows|

The command-duration sum is 22,565 s (6 h 16 m 5 s), not a three-hour assumption. The successful
production target reports 5,729.66 s in its test harness; attempt1 reports 5,141.62 s.
These harness durations are recorded separately from the shell's command wall durations.
All 756 production rows (name/ticks/limit) are byte-identical between attempt1 and final:
zero changed matches. Both targets passed their mandatory winner assertions. Independent
parsing of the separate duel log confirms 756 ended records and 756 Won records, with no Draw.
The observed timing variation therefore changes neither this sweep's match identities nor
its deterministic ending ticks. No cause is inferred from wall time alone.

R29 scaled-budget ratios (median / nearest-rank p90 / maximum): production
0.1314704861 / 0.2807579365 / 0.7824722222; separate duels
0.1386996528 / 0.3236736111 / 0.7664722222. The production worst case is Crossroads,
Libya easy versus Iraq easy, 56,338 / 72,000 ticks; duel worst is Riverlands, Iraq easy versus
Cuba easy, 55,186 / 72,000 ticks. Limits remain unchanged. Both exact Coral regressions pass.
`ra7/{summary.txt,metrics.json,production-lengths.tsv,duel-lengths.tsv}` retain the commands,
values and match identities. `ra7-attempt1` and every failed/partial preflight remain preserved.
The debug linker retains the same Apple compact-unwind 16 MiB warning present in RA6;
strict Clippy and both smoke checks pass. No new exception-handling performance claim is made.

**Assumptions and next work.** R84's standalone SimPlugin fogON scenario contract is distinct
from actual shipped match/lobby defaults OFF; R86 preserves only the original last-stand
fogON stalemate, with all assertions/limits unchanged and all nine companions default OFF.
R85 records Guard's explicit reference range or primary-weapon fallback and the separate
QueueMove variant; no undocumented generic guard/chase equivalence is claimed. Original
visual/audio/play feel requires user verification.

Proven first-spawn coverage is Command/Combat and actual paid production, with live/powered
and sold/dead detector boundaries. Root identified a further AI ordering boundary by reading:
raw `spawn_unit` Spy/sub into a previously revealed cell immediately before an AI turn with an
empty CommandQueue could reach AI View before the later concealment refresh (Commands→AI→
Economy). No actual shipped-game leak is proven; paid production and current starting rosters
do not establish that raw-helper case. RA8 is assigned to reproduce with the actual controller/
shipped definitions and fix if confirmed. This gate's proven coverage is not a claim that this
additional AI boundary is closed. Preserve the RA7 phase/privacy regressions when extending AI.
RA8 also retains broader arsenal use/performance/shorter-match work. R83's transport Size/
SizeLimit/loaded-boarding and actual attached Squid-host WeaponFired assertions remain RA9
obligations. Earlier R67 naval bridge repair refusal, RA5/RA6 assumptions and final independent
review remain unchanged; RA7 does not silently close them.

**Storage.** No clone, worktree, validation environment or /tmp build artifact was created.
Configured `CARGO_TARGET_DIR=/Users/xingyuwang/Library/Caches/Codex/cargo-target` and UV/PIP cache
variables are preserved. The existing target/package app/archive remains useful for delivery.
Tests created about 5.6 MiB of recreatable RON parser variants in that target's
`tmp/ra7_definitions`; this scratch is retained for reruns and is not required for release.
All requested logs/evidence remain in the canonical repository.

R87 closure process note: the cached diff hygiene check flagged only a terminal blank line in
raw captured `ra7/04-test.log`. The docs/log commit `9179d7b` proceeded because the tool
orchestration did not stop after that nonzero check. Preserve the raw stdout unchanged.
This artifact-formatting issue does not alter the frozen source or the successful gate
step02; the final working-tree diff check is recorded separately. No gate is rerun or
represented as passing to conceal that closure check result.

### R88. RA8 concealment before an otherwise empty-command AI turn

**2026-10-15.** R87's raw-helper boundary is confirmed using shipped definitions and an
actual match controller. A new enemy Spy is spawned on already visible ground with an empty
CommandQueue immediately before P0's next think. With idle ordinary army responders, the AI
queued AttackMove for all nine starting combat units to the concealed Spy's exact cell.
`ra8-preflight/raw-spawn-before3.log` preserves the failure. The initial probe passed because
its responders still had orders; it did not exercise defense selection. Its two preliminary
logs remain, including the configured-cache permission failure before any test ran.

An otherwise empty Commands pass now refreshes live power and concealment if any controller
is due at that exact tick. The predicate uses the same interval and player-offset arithmetic
as run_ai. Worlds with no controller avoid this work; ordinary nonempty command batches keep
RA7's existing refresh and batching. The refresh borrows Definitions, with no new deep copy.
`ra8_raw_spy_spawn_with_empty_queue_is_concealed_before_actual_ai_thinks` now passes and never
inserts hidden state itself. This closes the separate raw-spawn AI boundary, not a claim that
the already protected paid-production path leaked. No data, match limit or vision rule changes.

### R89. RA8 measured immutable definition sharing

**2026-10-15.** The immutable `Definitions` resource previously deep-cloned owned vectors,
strings, maps and nested rules in several exclusive systems every tick and for each external
blast. The repeatable `ra8_profile` harness measures 20,000 copies and a fixed 5,000-tick
shipped match with a per-tick value-hash digest. Before:628.801ms copying and1.853s ticks/hash.
After sharing the five immutable table handles using Arc:203.542microseconds copying and
1.062s ticks/hash. Both produce exactly `6696b039de9f6296`. These are local measurements,
not a prediction of final gate wall time. The raw logs remain under ra8-preflight.

Hashing/equality still visit values; pointers and reference counts never enter simulation
state or iteration. Consuming loaders replace their own handles, so variants cannot mutate
another clone. The new loader-independence test checks shared read-only references, a typed
failed reload leaving the source equal, and successful economy loading leaving the units-only
source unchanged. The serial parser/schema/hash/data-audit preflight passes all selected
targets. Its initial compile failure used a nonexistent helper name in the new test; that log
is preserved, and the corrected invocation completes. No runtime rule, fixture, assertion,
match limit or generated file changes in this optimization.

### R90. RA8 separate site geometry from production readiness

**2026-10-15.** A naval controller must establish that a coastal site is legal before
queueing a shipyard. The existing placement preview also requires a finished queue head,
which made that planning circular. Extract its unchanged geometry into the crate-private
`check_placement_site`: visibility is checked before footprint contents, then the same
water, occupancy, footprint, owned base-anchor and adjacency rules apply. Public
`check_placement` and actual `PlaceBuilding` retain their original readiness check and
`NotReady` result before calling the extracted helper.

`ra8_legal_site_can_be_inspected_without_consuming_or_bypassing_a_ready_item` checks a
legal pre-production site, unchanged player state on rejection and the same site's
success with a finished item. The schedule-level
`ra8_place_without_a_completed_item_rejects_without_state_mutation` issues the actual
command and compares its entire resulting simulation hash with an empty-command twin.
Both pass; their raw logs are retained. This extraction changes no command behavior.

### R91. RA8 country plans, source difficulty and declared AI policy

**2026-10-15.** AI schema 3 adds required country overrides, defense/support lists,
three tier reserves, an advanced-tech cutoff, bounded policy fields and an explicit
`rules_difficulty` switch. The generator supplies two side fallbacks and all nine country
plans; the neutral legacy/RA1–RA5 fixtures opt out of shipped house difficulty and strategy.
Rules schema 4 adds all twelve source AIIonCannon target weights for each difficulty.
Every fixture is migrated, and independent reference audits cover the generated values.

The shipped house settings follow source Easy/Medium/Brutal: weapon ROF 1200/1000/800,
build time 1000/1000/800, initial credits 1x/1x/5x, virtual purifiers 0/2/4, wave delays
4667/3333/2667, and defense counts 6/(20 Allied,22 Soviet)/25. The existing runtime had no
old difficulty firepower/armor multipliers to remove; first-hit damage remains equal.
Physical powered purifiers and virtual counts independently add the source 25 percent
bonus of the base ore load, with integer accounting; their bonuses do not compound.
Weapon ROF also covers parasite attacks, RepairBullet, PsiWave, eruption and occupation
weapons (occupation then divides by crew). Global superweapon, bomb, rearm and deployment
timers remain separate source rules. Player multipliers and all controller policy/memory
are value-hashed.

Full-tech Allied plans distinguish radar/airfield, America paradrops and Korean aircraft;
Soviet plans include radar and Battle Lab. Both include second refinery, depot, national
special, late economy, defense and optional major superweapons. America paradrops remain
available with major superweapons disabled. Unit production retains ordinary prerequisites,
ownership and dock checks. These plans and their composition are game-owned AI assumptions,
not a claim to reproduce the closed-source original decision algorithm. Shipped maximum
armies/waves are 16/8,24/12,36/16, with air/advanced/naval reserves 1/2/2 and advanced cutoff6,
so the initial 9/11 combat units leave room for actual tech progression. Policy declares
garrison quota3/radius12, depot radius16, naval rescan600, artillery range8/standoff3,
paradrop offset4 and Chronoshift offsets3–8. A naval detector quota1 fits within the declared
naval reserve2. Parser tests require and bound every field.

### R92. RA8 honest specialist strategy and non-starving endings

**2026-10-15.** Controllers remember observed air and locally unexplained losses, recruit
AA/detectors reactively, assign engineers to seen tech and known broken bridges, garrison
nearby buildings, repair owned buildings and send damaged vehicles to compatible service
bays. Specialist reservations prevent a subsequent army order from replacing their jobs.
Occupied civilian Props remain combat targets but cannot become economy buildings; a
depleted house deploys recoverable garrison infantry before liquidating its actual base.

Naval recruitment requires a legal reachable coastal placement and an observed enemy that
the chosen weapon can hurt from that water component. Candidate components use terrain,
owned buildings and visible enemy buildings; hidden occupancy never filters a route.
The remembered site, target and rescan time are hashed. Focused tests include an isolated
pond nearer than the useful coast, actual ship firing, and twin worlds differing only by
an unseen blocking shipyard. Removing the visibility filter makes that last test fail;
the exact source SHA before and after restoration matches.

The exact Coral regression trace then exposed R51's remaining escort gap: Soviet
Dreadnoughts could be built against a shore target while Typhoon/Squid were withheld because
their individual weapons could not hurt that building. The generated detector quota now
recruits a sensor escort from the existing qualified route before offensive naval reserves,
still within the naval and whole-army bounds. It reads no unseen enemy state. The dedicated
test requires actual fully paid Typhoon production and movement toward the known fleet goal.

Strategic strikes rank only known buildings/visible units with all twelve source weights.
Other ordinary buildings use the source Plug category; ordinary unclassified units receive
zero strategic weight. This category mapping is an explicit AI assumption. Curtain and
Chronoshift select real own vehicle waves; Chronoshift uses its explicit source and target
command. Landing checks use known occupancy. Normal production and recharge are exercised.

R53 retains patience1200 and player sale slots256/modulo16. An activated last stand gains
its exact sale-slot decision in addition to its ordinary cadence, avoiding starvation at
512/1024 and other supported intervals without a 19,200-tick cycle or simultaneous last
building sales. Capability checks exclude Dog/Drone/Squid building-incapable remnants and
stranded naval-only forces while preserving Temporal, legitimate mind-control opportunities
and active naval travel. Explicit winners are now required by m13, m14 and completed soak
matches; existing 400000/300000/36000 outer limits and R29 scaled limits are unchanged.

### R93. RA8 concealment refresh has no appearance RNG side effect

**2026-10-15.** In shipped human-controlled matches with crates enabled, a newly stationary
Mirage plus a rejected empty Guard produced a different crate than the otherwise identical
no-command world: `mirage-rng-before1.log` retains positions34,29 versus30,34 and differing
lifetimes. No AI is due in this probe. Early concealment refresh had allocated Mirage art
variants and consumed SimRng before Economy. Variant allocation now occurs only in the
regular revealing Visibility stage. Early refresh still derives concealment and removes
moving disguises. The complete resulting hashes, crates and next RNG value now match,
preserving command rejection's no-mutation contract and RA7 same-batch visibility rules.

### R94. RA8 fixture migrations and preserved preflight evidence

**2026-10-15.** Existing test migrations are limited to the declared schemas and stronger
hash coverage. Exact functions: `ra1_definitions::the_rules_fixture_loads_with_every_field`
adds neutral twelve target weights; `the_ai_fixture_loads_with_a_plan_per_side` adds the
neutral policy/plan fields (naval_rescan1, its minimum); `every_field_of_the_ai_file_is_required`
also requires rules_difficulty and policy; `unknown_and_duplicate_fields_are_rejected_in_every_file`
uses schema anchors4/3; `other_schema_versions_are_rejected_in_every_file` rejects versions
around the new current ones; `ai_plans_are_checked_for_their_side` tests the new maximum12.
`ra3_shipped::the_armies_of_the_shipped_plans_can_destroy_every_shipped_building` expects
eleven plans instead of two and retains every damage assertion.
`ra5_definitions::files_of_schema_version_1_are_rejected` expects rules4.
`economy::tests::fractions_lanes_and_radar_are_hashed` additionally checks all three new
house fields. Compile-only snapshot literals gain health/economy/new neutral plan fields.
No original assertion or match limit is relaxed; no test is deleted, ignored or skipped.

`serial-preflight1.log` stopped at the AI fixture's expected neutral naval_rescan0 versus
its required minimum1; that expected value was corrected. `serial-preflight2.log` stopped
at the unchanged RA4 last-stand lower bound: an unrelated army-cursor traversal correction
had changed its historical composition. Removing that change restores both RA4 tests in
`legacy-cursor1.log`; no fixture, timing assertion or game win was delayed. The shipped
reserve strategy is verified separately with the retained historical traversal policy.

Earlier failures remain evidence: the rejected initial broad shell-edit proposal ran no
mutation and was replaced with exact-context patches; strategy-tests1 exposed the rejected
1200-tick sale-slot design. The initial raw-spawn probe lacked idle responders and therefore
failed to expose the leak; raw-spawn-before3 is the actual negative result. Initial specialist
ROF probes used incorrect test APIs or missed the immediate occupation shot; corrected tests
measure real cadence. Capability acceptance4 recorded legitimate early Chrono wins. Its
isolated production fixture now uses an explicitly inaccessible reserve: extra HP alone
cannot delay Temporal strength-based erasure. Production and normal recharge assertions
remain. Navy3 selected zero tests and is not positive evidence; subsequent actual firing and
component/honesty tests are. The first unseen-blocker fixture placed its blocker in actual
Sight10 and was corrected to genuinely unseen ground. Exact negative mutation/restoration
SHA files and logs remain. Nothing rewrites these failed or incomplete raw captures.

R90 commit process deviation: the first git add failed on index permissions, and an empty
commit attempt failed. After the explicit escalated add, the staged whitespace check flagged
only terminal blank lines in raw `ra8-preflight/site1.log:9` and `site-command1.log:9`.
The tool batch nevertheless committed a9ad1b1. Preserve that commit and both raw logs; this
check did not pass. This repeats the earlier R87 orchestration mistake. Subsequent commits
must inspect a separate staged-check result before invoking commit. Any source/data/test
whitespace error must be fixed. Intentional raw-log EOF diagnostics must be classified by
exact path/line before proceeding, without suppressing checks or changing captured evidence.

The separate RA8 feature staged check returns2 with32 intentional raw-capture diagnostics,
not a pass:22 terminal blank-line diagnostics plus Coral trace trailing spaces at
`coral-regression1.log:4` through`:13`. Exact paths/lines, raw lines and unfiltered stdout
are preserved in `ra8-preflight/feature-staged-whitespace-check1.json`. No source/data/test
diagnostic occurs. The captures remain unchanged; the manifest and this classification are
staged, then a separate check must confirm the identical diagnostic set before commit.

### R95. RA8 Frostline control-holding attack-move lifecycle

**2026-10-15.** The first default ten-player all-map run fails Frostline seed13 at its
unchanged 400000-tick limit. Preserve `ten-player-all-end1.log`, including the subsequent
partial Oasis Gulf result, and both diagnostic trace replays. At84000 the survivors are
America0 (credits0, five buildings, one Carrier), Iraq6 (credits54248, about32 buildings,
55 commandable units), and Russia8 (credits25414,43 buildings,53 commandable units).
Iraq's48-member wave is at Waypoint8 and Russia's42-member wave at Waypoint0. Later traces
show the same idle formations and growing money. The Carrier is legitimately attacking a
Tesla Coil; it is not attacking an occupied Prop. The supposed garrison-control cause is
therefore refuted for this stall.

The actual cause is `combat::act_unit` returning for a mind controller holding a victim
before its existing AttackMove can resume or finish. Weapon.target is already None, but
the permanent order marker prevents `ai::at_rest` from advancing the entire wave. The
shared continuation now processes that movement lifecycle while preserving the original
ban on automatic retargeting and the explicit Attack exception. No timeout, winner delay,
limit change or broad AI attack override is added.

`yuri-boundaries-before1` proves the isolated marker failure. The first AI-wave fixture
failed to fail because a building-only target caused explicit Attack to clear the marker;
`yuri-wave-before4` adds a real distant Kirov and proves the actual held-controller stall
at Waypoint94. The strengthened fixture requires both surviving controllers to remain in
the wave and retain their victims. Both lifecycle regressions pass in
`yuri-lifecycle-after1`, including preserved explicit retargeting and six later AI waypoints.
The all-map attempt also retains four earlier explicit winners and Oasis Gulf's winner;
it is not an all-map acceptance pass.
After the lifecycle/classification fixes and R98 cleanup, the exact unsampled default
ten-player Frostline replay exits0: Won(P6) at113712,113713 ticks measured, mean687.947µs,
measured total78.229s/elapsed78.237s. All ten players build; peak320 units/141 buildings.
`frostline-end-after1.log` retains the unchanged400000-limit positive result. It closes the
specific reproduced stall, while the required all-map run remains separate acceptance.

### R96. RA8 synthetic civilian-building traits and RA9 actor boundary

**2026-10-15.** Occupied civilian Props were inserted into the combat target index with a
Building body but default traits: in particular, building=false and immune_psionics=false.
`ra8_yuri_cannot_acquire_a_hostile_occupied_building_as_a_control_target` uses shipped
Conscript occupation and a normal Yuri Attack command. Before the fix, the schedule emits
WeaponFired from Yuri SimId3 to host3221225869 with weapon61 (Controller) and then ends for
NoDamage. The automatic-fire-only initial probe passed because Wood versus0 suppresses
automatic acquisition; it was insufficient negative evidence. The explicit-command probe
fails before and passes after the shared physical-trait construction.

Mapped House/CityBlock/Tower Props now derive building combat and specialist metadata by
the same pipeline as real buildings, in the combat index, explicit scenery lookup,
occupation targeting and blast impact paths. Unmapped Trees/Bridge traits keep their
previous behavior. This reconciles building/psionic/radiation/bombable classification; it
does not supply actor-backed specialist abilities to scenery.

RA9 must investigate and reconcile these exact actorless boundaries against specified
mechanics and remaining differences: `special/resolve.rs::resolve` requires the target in
`actors` before infantry/vehicle fired handling, Ivan role charges and legacy C4 handling;
`special/infantry.rs::retain_beams` requires actor-index targets for Temporal retention;
`special/commands.rs` routes Interact with any Prop directly to garrison order, and
`special.rs::interaction_label` similarly offers only validated GARRISON. These are source
inspection findings, not newly reproduced C4/Ivan/Temporal schedule tests or a claim that
those abilities work on occupied Props. RA8 reproduces only the impossible Yuri control
shot and fixes classification independently of the Frostline order lifecycle.
In particular, `special/infantry.rs::fired` documents FakeC4/Sapper as ordinary impacts;
that physical combat path differs from actor-backed legacy C4 charges. The actor boundary
must not be misreported as proof that every C4 weapon fails against scenery.

The broader `yuri-boundaries-after1.log` stops with21 passing RA5-world cases and two
failed ground-army preconditions: `ra5_ai_repairs_the_only_land_route_with_a_consumed_engineer_and_wins`
and `ra5_ai_remembers_a_seen_collapse_and_approaches_its_hut_after_losing_sight` load the
expanded shipped AI, which now contains aircraft. Their shared `only_land_route_ai` fixture
loads `fixtures/data/ra5/bridge_ai.ron`, the exact RA7 shipped ordinary ground plans and
difficulties migrated to neutral RA8 fields. This preserves the existing no-air precondition,
bridge collapse/observation/repair/consumption/crossing/winner assertions and72000 limit.
The actual shipped default composition remains covered separately by RA8 and default-path
sweeps. Merely retaining all Land types introduces new specialists; restoring only the old
six-type armies or disabling sourced difficulty alone does not restore the original neutral
strategy. Failed probes `yuri-boundaries-after2/3/4` remain unchanged.

### R97. RA9 handoff: queued AI intent can outlive a combat casualty

**2026-10-15.** `bridge-command-trace3.log` reproduces a generic Commands→Ai→Combat race
under expanded strategy in the fog-on bridge-memory scenario. At AI decision tick12391,
P1 Attack Dog (UnitKind3) SimId202 is alive with HP7, no MoveOrder/Weapon.order and weapon6 cooldown4.
That tick's combat emits UnitDestroyed(P1,202). The next Commands stage rejects P1's queued
AttackMove([202,210,212],21,23) with UnknownUnit202 (reported posttick12393). Thus the
controller did not select an already-dead snapshot; a valid queued intent outlived its actor.
`bridge-command-trace1` is a diagnostic compile failure, trace2 lacks the preceding actor,
and trace3 is the complete negative evidence.
The initial report incorrectly called UnitKind3 a Conscript; verifying the actual unit table
corrects that label to Attack Dog. The raw numeric evidence is unchanged.

RA9 must decide whether internal AI intent needs lifecycle pruning while public missing/dead
unit IDs continue to reject exactly as specified. RA8 does not silently remove IDs from
public commands, relax validation, weaken the bridge no-rejection assertion or claim that
neutral difficulty alone diagnoses the race. The isolated bridge fixture retains its
documented original ordinary ground strategy; actual expanded default-path runs remain
independent acceptance evidence.

### R99. RA8 preserves useful observed infantry control opportunities

**2026-10-15.** The prior capability predicate excluded every on-foot enemy. A lone Easy
Yuri at15,8 with no income and a normally visible hostile Conscript at24,8 therefore sells
its base before the usable capture opportunity (`infantry-opportunity-before1`). Eligibility
now uses the actual mind-control weapon/warhead and target immunity, requires a recruit that
can damage buildings, and checks static-terrain reach to firing range. It reads only visible
hostile snapshots and public terrain; temporary traffic and building occupancy, including
hidden occupancy, cannot prove permanent isolation. No state, timer or balance field is added.

The schedule regression retains the base beyond its normal first sale slot for Conscript
and GI, then proves ordinary-command capture and actual building HP loss from the recruit.
Immune Tanya, building-incapable Dog and Conscript across an impassable water strip do not
postpone liquidation. The first after probe additionally demanded automatic capture by an
arbitrary14000-tick waypoint deadline; it retained the base correctly but failed that unrelated
expectation. The corrected capability probe uses normal commands after4200 retained ticks,
without changing target-selection policy or weakening an existing test. Raw failures remain.

**Unproven RA9 audit lead, explicitly deferred by root:**
`crates/sim/src/ai/strategy.rs::damages_buildings` constructs hypothetical building traits
with default bombable=false, whereas physical building targets derive metadata through
`crates/sim/src/combat.rs::{traits_of,specialist_traits}`. This may undercount charge-planter
victory capability (for example Ivan) or useful controlled charge recruits. No match-ending
defect is reproduced for this lead; it is not a claim that all charge planters fail. RA9
must reconcile broader specialist capability classification during its data/soundness audit.

### R100. RA8 artillery follows the surface front in mixed waves

**2026-10-15.** The front-line predicate admitted aircraft. The real scheduled mixed-wave
probe (`artillery-line-before4`) places Rhino14,8, V3Launcher12,8 and Kirov22,8 with a known
enemy barracks34,8 and visible armed Rocketeer24,14. The V3 receives AttackMove19,8 behind
the nearer Kirov rather than11,8 behind the Rhino. Requiring Domain::Land in the existing
front predicate fixes this single eligibility error; the declared range/standoff policy
remains unchanged. The fixture preserves actual controller memory/wave formation, pauses
decisions while ordinary commands arrange legal positions, and resumes the scheduled AI.

Earlier fixture failures remain: before1 used nonexistent unit ID `v3`; before2 removed a
required AiPlayers resource rather than leaving an empty one; before3 selected10,8 inside
the yard footprint, so ordinary Move correctly stopped adjacent. Only before4 reaches and
fails the actual artillery assertion. All27 RA8 scenarios pass in `final-capabilities1`;
the final boundary batch and default-path evidence cover the final source separately.
