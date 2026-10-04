# Decision log

Every entry records the date, the decision, and the evidence consulted. Changes to `PLAN.md` require the user's explicit approval recorded here; none has been made.

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

`--smoke-test <frames>` (an integer from 1 to `u32::MAX`; anything else exits with code 2 and a usage message) builds exactly the same `App` as a normal run, adds a frame counter in `Last`, and exits after the given number of frames. The exit code is 0 only if no asset failed to load (`UntypedAssetLoadFailedEvent`) and the spike map was created (`TiledEvent<MapCreated>`); otherwise 1. The final log line reports frames, simulation ticks, and map creation.

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

## User decisions

### U1 (2026-10-03): macOS is the only platform that must run

The user stated: "Just make it run on Mac is good enough." Verification on macOS is therefore sufficient, and Windows and Linux are not verified. PLAN.md section 1 still names Windows, macOS, and Linux for version 1.0. If this decision applies to version 1.0 and not only to M0, PLAN.md section 1 must be amended, and only the user may make that change.

The user also verified the M0 spike map on screen with `cargo run --release -p game`.
