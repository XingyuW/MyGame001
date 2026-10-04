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
3. **Commands.** `Command::Move { units, target }` and `Command::Stop { units }` (at most `MAX_UNITS_PER_COMMAND` = 1,024 units). Validation in this order: at least one unit, not too many, no unit twice, every unit exists, every unit belongs to the issuing player, the target is inside the map. Any failure rejects the whole command with `SimEvent::CommandRejected { player, command, reason }` and changes no state. Visibility is not validated before M4: there is no fog of war yet, so every cell is visible to every player. `Stop` clears the order; a unit between two cells completes its step. A new `Move` replaces the current order; a unit between two cells continues to the next centre and plans from there.
4. **Group destinations.** For a valid `Move`, `pathfinding::destination_cells` takes the passable cells nearest to the target by path cost over passable terrain (a Dijkstra search from the target, which also works for a target on rock; cells cut off from the target are never chosen), skipping cells held by idle units outside the group. The units, ordered by octile distance to the target and then id, receive the cells in that order, so every unit of a group has its own destination cell. A unit for which no cell remains stops with `MoveFailed { NoDestination }`. (An alternative that fills the cells from the far side of the target first was measured on the fixture scenarios and was not better; the simpler order was kept.)
5. **A\* (PLAN.md section 5, equation 5).** Step costs 10 and 14, no diagonal step between two blocked orthogonal neighbours, octile heuristic, ties broken by (f, h, cell index), so paths are deterministic. Requests are served in the `Pathfinding` stage, at most `PATH_REQUESTS_PER_TICK` = 16 per tick, oldest first and then by unit id. A path starts at the cell whose centre the unit reaches next. No path over terrain ends the order with `MoveFailed { Unreachable }`.
6. **Blocking and repathing.** A unit at a centre whose next cell has become impassable (or whose diagonal is cut by terrain) requests a new path at once. One whose next cell is held by another unit waits; after `REPATH_AFTER_BLOCKED_TICKS` = 8 ticks it requests a path that avoids units, chosen in this order: around all waiting units (idle, or waiting at a centre); else, if it is in a wait-for cycle (following the holder of each unit's next cell leads back to it) and has the highest id among the cycle's members that have a free neighbouring cell, a step aside to the free neighbour nearest to its destination (exactly one member gives way); if no member can step aside, the other members count as lasting obstacles; else around lasting obstacles (idle units) only, queueing behind units that will move on; else, since lasting obstacles close every way to the destination, the reachable cell nearest to the destination becomes the new destination (`pathfinding::nearest_reachable`; `MoveOrder::retargets` counts this). A unit that does not advance for `MAX_STALLED_TICKS` = 200 ticks (10 s) ends its order with `MoveFailed { Blocked }`, so no order lasts forever.
7. **Events.** `SimEvent` is a Bevy `Message` (A1) with the variants `CommandRejected`, `MoveCompleted { unit }` (the unit reached its destination, possibly a retargeted one), and `MoveFailed { unit, reason }`. `SimPlugin` inserts `Messages<SimEvent>` as a plain resource (not registered for Bevy's per-frame update), and the first system of each tick calls `Messages::update` (`bevy_ecs-0.19.1/src/message/messages.rs:193`). After a tick, `iter_current_update_messages` (`messages.rs:265`) yields exactly that tick's events, and a `MessageReader` sees the last two ticks.
8. **Systems.** `SimSet::Commands`: `advance_events`, then `process_commands` (chained); `SimSet::Pathfinding`: `find_paths`; `SimSet::Movement`: `move_units`. All iterate units in ascending `SimId` order or use ordered collections (`BTreeMap`, `BTreeSet`), never query or hash order (I4).
9. **Hash (I3; D4 and D11 extended).** After each entity's id: a flag byte, and for a unit its owner, kind, speed, cell, step (flag, target, progress, length), position, facing, and order (flag, destination, path state, path cells, counters). Each grid cell's flag byte gains bit 2 (occupied), followed by the occupant's id when set. The definitions are hashed after the grid.
10. **Setup API.** `spawn_unit` (checks the unit type, bounds, passability, and occupancy; typed `SpawnError`) and `Grid::set_terrain` are scenario and match setup, like replacing the grid (D11): the game uses `spawn_unit` for the starting units before the first tick, and the tests use both to place units and obstacles. During a match, units are created by production (M3) and terrain changes through buildings (M3).
11. **Integer arithmetic (I12).** Positions and costs are bounded by the map size (at most 256 cells); the helpers that could see arbitrary values (`SubPos::centre_of`, `octile_distance`, step offsets) saturate, counters use `saturating_add`, and the tick counter wraps (D17).

### D20. Units in the game

1. **Sprite sheets.** One PNG per sprite name at `assets/sprites/units/<sprite>.png`: eight 64×64 frames side by side (512×64), frame i showing facing `Direction::ALL[i]` (east, south-east, south, south-west, west, north-west, north, north-east in cell axes; east is down-right on screen). The ground point under the unit lies at pixel (32, 44) of a frame, so the sprite is drawn 12 px above the unit's projected position. The placeholders are generated by `cargo run -p game --example gen_unit_sprites` (light grey, tinted per owner with `Sprite::color`); `crates/game/tests/m2_unit_sprites.rs` checks that the committed files match the generator, pass the PNG subset, and point each frame in its facing's screen direction.
2. **Checked bytes (I11, D16).** Every sprite sheet named by the definitions is checked with the tileset image's PNG subset (`tileset_image::check_png_file`, generalised from `check_tileset_image`) at the size 512×64 before the match starts (cross validation of the sprite references, PLAN.md section 5), and only the checked bytes are registered in `CheckedAssets`. A missing or invalid sheet ends the game with exit code 1 and an error naming the units file, the `units[i].sprite` field, the image, and the reason. The image reader now rejects anything that is not a regular file before opening it, which closes M1 review 3 P2-1 for tileset images and sprite sheets (the map reader in `sim` is unchanged).
3. **Drawing.** Each simulation unit gets a separate view entity (`UnitView`), so the game never writes simulation components. Its position is interpolated between the unit's position before and after the last tick with `Time<Fixed>::overstep_fraction` (`bevy_time-0.19.1/src/fixed.rs:205`), projected with equation 7, and drawn at z = 1 + (u + v)/1024 (in front of the map's layers at z = 0, and units lower on the screen in front). `Sprite::rect` (`bevy_sprite-0.19.1/src/sprite.rs:39`) selects the facing's frame.
4. **Starting units.** Until match setup in M6, the game spawns 12 light tanks and 6 scouts for player 0 (the human) around cells (10, 28) and (14, 34), and 5 light tanks and 3 scouts for player 1 around (50, 30) and (52, 36) of the demo map, each on the free cells nearest to its anchor.
5. **Controls.** Left click selects the player's own unit under the cursor (the nearest body within 20×16 world units of the cursor), or clears the selection on empty ground; a left drag of at least 4 px selects the own units whose ground point lies in the box (`Rect::contains` is inclusive, `bevy_math-0.19.1/src/rects/rect.rs:249`); Shift adds to the selection instead of replacing it. Right click on a map cell orders the selection to move there; `S` stops it (WASD stays free for M5 hotkeys, D10). Each order is one command for the whole selection, pushed into `CommandQueue` (I5). Units of player 1 cannot be selected.

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
