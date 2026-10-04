# M1 independent review 1: Map and camera

Reviewer: independent review subagent (did not implement or remediate this code). Date: 2026-10-04.

## Scope and base

* Repository `/Users/xingyuwang/GameProjects/MyGame001`, branch `milestone/m1`, head `af7f0f4`.
* Base: tag `m0-approved` (the most recent reviewed or approved tag). Reviewed diff: `git diff m0-approved..milestone/m1` (25 files, +3073/−73). Commits `41c4d51`, `b2862c9`, `712bd3b`, `af7f0f4`. `af7f0f4` is the user-approved PLAN.md amendment (DECISIONS U2) and is not milestone code; its content (macOS only, debug smoke command, section 12 with I11 and I12) was applied as the review standard.
* Read in full: PLAN.md (sections 1 to 12), docs/STATUS.md, docs/DECISIONS.md, every changed source, test, and fixture file, and the third party code that consumes the map: `tiled` 0.16.0 (`map.rs`, `tileset.rs`, `util.rs`, `layers/tile/{mod,finite,util}.rs`, `image.rs`), `bevy_ecs_tiled` 0.13.4 (`tiled/map/loader.rs`, `tiled/map/asset.rs`, `tiled/reader.rs`, `tiled/helpers.rs`), and the `bevy_asset` 0.19.1 panic site found by probing (`loader_builders.rs:251`).
* No repository file was created or modified. All probes ran from copies under `scratchpad/m1-review-1-probe/` (assets copied per case and selected with `BEVY_ASSET_ROOT`; a scratch crate `simprobe` calls `sim::load_map` directly in debug and release).

## Validation commands (PLAN.md section 9), rerun by the reviewer

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 77 passed, 0 failed, 0 ignored (game unit 22, `m0_spike` 6, `m1_demo_map` 3, sim unit 2, `m0_bootstrap` 12, `m1_map` 32, doc-tests 0); matches the implementer's count. Known M0 linker warning (`__eh_frame section too large`) only. |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | `map maps/demo/demo.tmx: 64x64 cells, tile 64x32 px`; `smoke test: 600 frames, 198 simulation ticks, map created: true` |
| `cargo run -p game -- --smoke-test 600` (debug, section 12 item 5) | 0 | `smoke test: 600 frames, 210 simulation ticks, map created: true` |

A display was available. All six commands pass. No test was removed, skipped (`#[ignore]`), or weakened: the only removed file is `crates/game/src/spike.rs`, which had no tests; `m0_spike.rs` and `m0_bootstrap.rs` are unchanged and pass.

## What was verified and holds

* **Section 5, equation 7 and its inverse** (`crates/game/src/projection.rs:30-64`): `world_x = (u − v)·W/2`, `world_y = −(u + v)·H/2`; inverse `a = x/(W/2)`, `b = −y/(H/2)`, `cx = floor((a + b)/2)`, `cy = floor((b − a)/2)`; cell centre at `c + ½`, consistent with `SubPos = c·256 + 128`. Reconciliation with `bevy_ecs_tilemap` (D7/D9) is checked against `TilePos::center_in_world`/`from_world_pos` for every cell of six sizes. Picking boundaries (shared edges belong to the larger coordinate, far corners outside, NaN/infinite rejected, saturating `as i32`) are correct and tested.
* **Required M1 tests exist and pass:** round trip for every cell of 1×1, 1×37, 37×1, 64×64; picking at centres, edges, corners (16 points per cell, exact equality); picking outside returns `None`; malformed maps (missing layer, unknown property, zero size) with committed fixtures asserting kind, field, file, and message; camera clamping at all four borders (`clamp_centre`).
* **Section 8 categories:** positive, negative, boundary (256 max side and 257, ore 0 and `i32::MAX`, `i32::MIN` cell queries), regression (`layer_data_with_wrong_entry_count_is_rejected_without_panic`), integration through `SimPlugin` (hash at every tick, grid unchanged by ticks, every cell attribute affects the hash), end to end (smoke runs above).
* **CSV entry-count precheck (DECISIONS A2 follow-up 2):** compared line by line with `tiled`'s `read_text_or_cdata`/`decode_csv`/`parse_tag`: same `quick-xml` 0.31.0 (single version used by `tiled` per `Cargo.lock`), same default reader settings, same first-text-or-CDATA rule, same comma split; duplicated attributes are rejected by both; layers without `<data>` default to a 0×0 layer and are caught by the layer-size check; layers inside `<group>` are counted too. 6,000 mutation-fuzzed variants of `valid_4x3.tmx` produced **no** panic at `tiled` `finite.rs:69`. The precheck itself is sound.
* **Invariants:** I1 (`cargo tree -p sim -e normal` has no `wgpu`, `winit`, `bevy_render`, `bevy_window`, `bevy_input`, `bevy_audio`, `cpal`, `rodio`, `gilrs`, `zstd`, image crate); I2 (no `f32`/`f64` in `crates/sim/src`); I3 (grid appended to `state_hash`, D11 layout, deterministic); I4 (properties sorted, `BTreeMap` for terrain, no hash-map-order dependence); I9 (`quick-xml =0.31.0`, `tiled =0.16.0`, lock committed, no package added); I10 (tests use fixtures); I12 (sim map arithmetic: `u64` index and entry-count products cannot overflow, `u32::try_from` for ore, sides capped at 256).
* **Production path:** `game::map::load_map` validates through `sim::load_map` before spawning `TiledMap`; a sim rejection exits with code 1 in smoke and in normal runs (probed with a staggered map: both profiles and a normal release run exit 1 with `map attribute "orientation": orientation "staggered" is not supported`).
* **Camera arithmetic** (`camera.rs`): pan normalisation, edge band, drag sign convention, zoom clamp to [0.5, 2.0], centring when the view exceeds the bounds; reviewed and covered by unit tests.

## Findings

### P0

#### P0-1: Tileset `margin`/`spacing` values panic inside `sim::load_map` (I8, I11; debug and release)

* **File/line:** `crates/sim/src/map.rs:282-284` (the hand-off to `tiled`) and `crates/sim/src/map.rs:299-357` (`precheck` checks only the map header and layer data). Third party panic site: `tiled-0.16.0/src/tileset.rs:577`, `(image.width as u32 - margin + spacing) / (tile_width + spacing)`, evaluated whenever a single-image tileset has no `columns` attribute.
* **Evidence:**
  * `valid_4x3.tmx` with `tilecount="3" margin="1000"` (no `columns`): `simprobe` debug: `panicked at tiled-0.16.0/src/tileset.rs:577:27: attempt to subtract with overflow`. Release: silently loads (`OK 4x3`) with a wrapped column count.
  * `tilecount="3" spacing="4294967232"` (no `columns`): debug `attempt to add with overflow`; **release** `panicked at tiled-0.16.0/src/tileset.rs:577:26: attempt to divide by zero`.
  * Through the real binary (copy of `assets/`, demo map edited the same way, `--smoke-test 200`): `margin_nocolumns` debug exit **101** (panic in `Compute Task Pool`, i.e. the Startup `load_map` system), release exit 0 (map accepted and rendered); `spacing_divzero` debug exit **101**, release exit **101**.
  * Mutation fuzzing (6,000 variants, each run in its own process): debug 433 panics (310 subtract overflow, 123 add overflow, all at `tileset.rs:577`); release (4,446 inputs completed) 94 panics, all `attempt to divide by zero` at `tileset.rs:577`.
  * Debug and release behave differently on the same input (I12 spirit; I8 letter).
* **Expected:** I8 and I11: every tileset attribute that `tiled` evaluates (`margin`, `spacing`, `columns`, `tilewidth`, `tileheight`, `tilecount`, image `width`/`height`) is restricted to an explicitly listed subset in D8 (for example `margin = spacing = 0`, `columns` required and equal to `image width / tilewidth`, tileset tile size equal to the map tile size) and checked on the raw XML **before** `tiled` parses it, in the map and in every external `.tsx`, returning a typed `MapError`. Regression tests for each case in debug and release.

#### P0-2: Deeply nested elements abort the process with a stack overflow (I8, I11; debug and release)

* **File/line:** `crates/sim/src/map.rs:282-284`; recursion in `tiled-0.16.0/src/layers/group.rs:31-70` (`<group>` calls `LayerData::new` recursively) and `tiled-0.16.0/src/properties.rs:140-178` (nested class properties). `precheck` (`map.rs:299`) is iterative but does not bound depth.
* **Evidence:** `valid_4x3.tmx` with 10,000 nested `<group>` elements (≈310 KB) appended: `simprobe` debug and release: `thread 'main' has overflowed its stack` / `fatal runtime error: stack overflow, aborting`. Depth 3,000 is rejected normally (`UnexpectedLayer`). 200,000 nested class properties abort the same way. Through the game binary (demo map with 10,000 nested groups): debug exit **134**, release exit **134** (`thread 'Compute Task Pool (0)' has overflowed its stack`). An abort cannot be caught; no typed error is produced.
* **Expected:** the precheck rejects any element nesting deeper than the supported format needs (map → tileset/layer → tile → properties → property, about 6 levels) and rejects `<group>`, `<objectgroup>`, `<imagelayer>`, and class/list properties before `tiled` parses the file, with a typed `MapError`; regression test.

#### P0-3: Unbounded memory allocation from `tilecount` (I8, I11)

* **File/line:** `crates/sim/src/map.rs:282-284`; `tiled-0.16.0/src/tileset.rs:484` (`HashMap::with_capacity(prop.tilecount as usize)`) and `:534` (`for tile_id in 0..prop.tilecount { tiles.entry(tile_id).or_default(); }`), both executed before `validate` can reject undefined tiles.
* **Evidence:** `tilecount="2000000"` (release `simprobe`): 921 MB resident in 0.28 s before the error `tile 3 ... required property is missing`. During fuzzing, 9 inputs with `tilecount` 2147483647, 2147483648, or 4294967295 never returned; one (`f01586.tmx`, preserved by the orchestrator at `scratchpad/oom-input-f01586.tmx`, debug build) reached a **246 GB** memory footprint within about one minute, pushed the machine into 21 GB of swap, and was killed by the orchestrator with SIGKILL; the other 8 were killed (exit 137) by the watchdog. A one-attribute edit to a map file can therefore exhaust the machine's memory in both profiles.
* **Expected:** `tilecount` is part of the listed subset and bounded on the raw XML before `tiled` parses (for example equal to `columns × rows` of the declared image and at most a small fixed maximum), with a typed `MapError` and a regression test that runs within a bounded memory budget.

#### P0-4: Maps accepted by `sim::load_map` panic inside the renderer's loaders (I8, I11)

* **File/line:** hand-off at `crates/game/src/map.rs:93-97` (`TiledMap(asset_server.load(MAP_PATH))`) after validation that does not cover rendering inputs (`crates/sim/src/map.rs:563` only checks `tileset.image.is_some()`). Third party panic sites: `bevy_ecs_tiled-0.13.4/src/tiled/map/loader.rs:421-422` (`(img.width as u32 - tileset.margin + tileset.spacing) / (tileset.tile_width + tileset.spacing)`, computed even when `columns` is present) and `bevy_asset-0.19.1/src/loader_builders.rs:251` (`unwrap` on an unapproved path).
* **Evidence:**
  * Demo map with `columns="8" margin="1000"`: `sim` accepts it (`map maps/demo/demo.tmx: 64x64 cells`), then debug: `thread 'IO Task Pool (1)' panicked at bevy_ecs_tiled-0.13.4/src/tiled/map/loader.rs:422:28: attempt to subtract with overflow`, smoke exit 1; release: accepted and rendered with a wrapped column count, smoke exit 0.
  * Demo map whose tileset image is `source="../../../demo_tiles.png"` (file present there): `sim` accepts it; **both** profiles: `Asset path maps/demo/../../../demo_tiles.png is unapproved` followed by `panicked at bevy_asset-0.19.1/src/loader_builders.rs:251:42: called Result::unwrap() on an Err value: UuidNotSupportedError`; smoke exit 1. The same happens for an external tileset whose `source` escapes the asset root.
  * The asset server catches these panics, so the process survives, but a panic is reachable from a malformed data file (I8), and the inputs are outside any supported subset yet are handed to the third party code (I11).
* **Expected:** D8 lists the rendering-relevant subset (tileset geometry as in P0-1; image and external tileset paths relative, without `..`, inside the map's directory, double- or single-quoted forms both handled), the loader checks it before the hand-off, and the game renders only maps that passed it. Regression tests per case.

### P1

#### P1-1: Inputs outside a supported subset reach the renderer and are accepted silently (I11; silent fallback)

* **File/line:** `crates/sim/src/map.rs:553-574` (`tile_terrain`) and `:448-512` (`validate`); `crates/game/src/map.rs:102-150` (`on_map_created`/`misalignment` run only **after** `bevy_ecs_tiled` has loaded and spawned the map, i.e. after the hand-off, contrary to I11's "before the hand off").
* **Evidence (each probe: sim accepts, both profiles, `--smoke-test 200` exit 0 unless noted):**
  * A tile defined with `id="100000"` (all three properties) placed at cell (0, 0) of the demo map: accepted, `map created: true`. The renderer receives texture index 100000 for an 8-tile image; the simulation reports a passable, buildable cell whose drawing is undefined.
  * `demo_tiles.png` replaced by a valid 1×1 PNG while the tileset still declares `width="512" height="32"`: accepted, `map created: true`, no warning. The real image size is never compared with the declared one.
  * External tileset referenced with single quotes (`<tileset firstgid='1' source='ext.tsx'/>`): `sim` accepts it (D8 item 5 allows external tilesets), but `bevy_ecs_tiled`'s line-based, double-quote-only preloader (`tiled/reader.rs`, `extract_external_paths`) does not find it: `External tileset/template 'maps/demo/ext.tsx' not found in cache`, smoke exit 1. The same file is therefore "valid" for the simulation and unloadable for the renderer; the loader does not read external references "with the same rules as the third party code".
  * Tileset tile size different from the map tile size is not restricted, although the projection assumes the map tile size.
* **Expected:** the supported subset in D8 covers every property the renderer consumes (tile ids within the image's tile grid, declared versus actual image size or a documented decision, tile size equality, external reference syntax and location, layer/tileset offsets and parallax), and violations are rejected with a typed error before `bevy_ecs_tiled` sees the file. Tests per case.

#### P1-2: A renderer-side load failure leaves a normal run going without a map (silent fallback)

* **File/line:** `crates/game/src/smoke.rs:31-42` is the only handler of `UntypedAssetLoadFailedEvent`; `crates/game/src/map.rs:26-40` registers no handler for a failed `TiledMap` load, so outside `--smoke-test` nothing reacts.
* **Evidence:** the `margin_columns` case from P0-4 run without `--smoke-test` (`target/debug/game`, 12 s alarm): the log shows the loader panic, `Failed to load asset 'maps/demo/demo.tmx'`, and `bevy_ecs_tiled::tiled::map: Map failed to load, despawn it`, yet the process keeps running until killed by the alarm (exit 142). The simulation holds the validated grid while nothing is drawn; the game's own code neither reports the failure nor exits. Only the smoke test turns it into an error. Contrast: a `sim` rejection and a misaligned map both exit with code 1.
* **Expected:** the game treats a failed map (or tileset image) load as fatal in every mode, logging the map path and the reason and exiting with code 1, consistent with the `sim` rejection path; an automated test or smoke case covering it.

#### P1-3: Errors in external tilesets name the wrong file (section 5 error context)

* **File/line:** `crates/sim/src/map.rs:261-265` (every `MapError` carries the map path) and `:553-574` (tileset errors use the map path); `:284` (parse errors of an external tileset are reported with field `file` of the map).
* **Evidence:** an external `bad_prop.tsx` with an extra tile property: `map ext_bad_prop.tmx: tileset "fixture" tile 1 property "height": unknown property` (the `.tsx` that contains the fault is not named). An external `bad_xml.tsx` truncated mid-attribute: `map ext_bad_xml.tmx: file: not a valid Tiled map: error while parsing attribute: position 38: ...` (the position refers to the `.tsx`, which the message does not name). No test covers errors inside an external tileset; `external_tileset_loads` covers only the positive case.
* **Expected:** PLAN.md section 5: "A load error names the file, the field, and the reason." For a fault in an external tileset the error names the `.tsx` path (and, optionally, the referencing map); tests for an invalid external tileset.

### P2

* **P2-1** `crates/game/src/map.rs:68` and `:94`: the map file is read twice (once by `sim::load_map`, once by the asset server), so the rendered bytes are not the validated bytes (implementer debt 1). Under I11 the renderer should receive the validated input (for example by building the `TiledMap` asset from the bytes `sim` checked, or by re-validating inside the asset loader). Low likelihood, but it also widens the gap described in P1-1.
* **P2-2** `crates/sim/src/map.rs:299-357`: an unterminated final `</map` is accepted (implementer debt 3, documented and tested in `truncated_map_files_never_panic`). Acceptable as documented debt.
* **P2-3** `crates/game/src/camera.rs:119-199`, `crates/game/src/map.rs:152-196`: the camera and picking systems are tested only through their pure functions; the wiring is left to user verification (implementer debt 2). Acceptable as documented debt.
* **P2-4** `crates/game/src/map.rs:247`: the unit test `offsets_and_parallax_are_reported` writes to a fixed directory `std::env::temp_dir()/mygame001_game_map_tests`, outside Cargo's target directory and shared by concurrent test runs. Expected: `CARGO_TARGET_TMPDIR`-style unique location, as `m1_map.rs` does.
* **P2-5** `crates/game/src/map.rs:91-92`: the game inserts `Grid` directly instead of through `CommandQueue`. DECISIONS D11 justifies it as match setup and PLAN.md I5 concerns the interface during a match, so this is not a violation, but the interpretation is the implementer's; record the user's acknowledgement when M6 adds match setup.
* **P2-6** DECISIONS D8 says "Every tileset uses a single image and has no properties" but does not state that every tile id below `tilecount` must be defined (the loader rejects undefined tiles because `tiled` creates property-less entries for all ids below `tilecount`). Document it in D8.

## Implementer claims checked

| Claim | Outcome |
|-------|---------|
| 77 tests pass; all validation commands exit 0 | Confirmed (debug smoke command added by U2 also passes). |
| "`bevy_ecs_tiled` only ever sees maps that cannot make it panic" (`crates/game/src/map.rs:3-5`) | **Refuted** (P0-4). |
| "Never panics on malformed input" (`crates/sim/src/map.rs:258`) | **Refuted** (P0-1, P0-2, P0-3). |
| DECISIONS A2 follow-up 3: other `tiled` panic sites reviewed, only `template.rs:79` and `util.rs:76` found and both unreachable | Incomplete: `tileset.rs:577` (arithmetic panics), `tileset.rs:484/534` (unbounded allocation), and recursive descent (stack overflow) were missed. |
| CSV entry-count precheck mirrors `tiled` exactly | Confirmed by source comparison and fuzzing. |
| Staggered map and short CSV exit 1 without panic | Confirmed for staggered (both profiles and a normal run). |
| `tiled` adds no forbidden dependency to `sim` | Confirmed. |

## Requires user verification (not findings; section 12 item 2)

Run `cargo run --release -p game` and check (as listed in docs/STATUS.md, "Requires user verification"):

1. Map rendering: 64×64 green diamond, tiles edge to edge; red tile at the top corner (0, 0), magenta at the right corner (63, 0), white at the left corner (0, 63); grey rock ridge with a gap, blue lake with sand shore right of centre, gold ore fields.
2. Picking: the white outline and centre dot sit on the tile under the cursor, including along the map edges, and disappear outside the map; a left click logs the cell (for example `cell (0, 0): passable true, buildable true, ore 0`) or `click outside the map`.
3. Panning: arrow keys (diagonals not faster), screen edge within 8 px only while focused, middle mouse drag keeps the grabbed point under the cursor.
4. Zoom: wheel or trackpad, limited to 2× and 0.5× magnification.
5. Bounds: the view cannot leave the map's bounding rectangle at any zoom or window size; vertically centred when the view is taller than the map.

## Decision rationale

All validation commands pass and the required M1 tests exist, but four P0 findings show that malformed map files reachable through the production path panic or abort the game (debug and release) or exhaust memory, violating I8 and I11, and three P1 findings show silent acceptance of unsupported inputs, a silent fallback in normal runs, and incorrect error context. Section 10 makes NO-GO mandatory for any unresolved P0 or P1.

Decision: NO-GO
