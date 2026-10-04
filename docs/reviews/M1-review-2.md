# M1 independent review 2: Map and camera

Reviewer: independent review subagent. I did not implement or remediate this code. Date: 2026-10-04.

## Scope and base

* Repository `/Users/xingyuwang/GameProjects/MyGame001`, branch `milestone/m1`, head `deb2859`. The working tree was clean before and after the review.
* Base: tag `m0-approved`, the most recent reviewed or approved tag. The reviewed diff is `git diff m0-approved..milestone/m1`: 28 files, +5806/−73. Its commits are `41c4d51`, `b2862c9`, `712bd3b`, `af7f0f4`, `d495afb`, `d1304e6`, and `deb2859`.
  * `af7f0f4` is the user-approved PLAN.md amendment (DECISIONS U2) and is not milestone code. Its content is applied as the review standard: macOS only, the debug smoke command, and section 12 with I11 and I12.
  * `deb2859` is remediation round 1.
* Read in full:
  * PLAN.md sections 1 to 12, docs/STATUS.md, docs/DECISIONS.md, and docs/reviews/M1-review-1.md.
  * Every changed source file: `crates/sim/src/{map,map_subset,lib,hash,schedule}.rs` and `crates/game/src/{map,projection,main,smoke}.rs`.
  * Of the test files: the structure and the key tests of `crates/sim/tests/m1_map_subset.rs`, and the remediation diffs of the game tests.
* Third party code that consumes the external files, read at the call sites:
  * `tiled` 0.16.0: `util.rs` (`get_attrs!`, `read_text_or_cdata`), `layers/tile/{mod,finite,util}.rs`.
  * `bevy_ecs_tiled` 0.13.4: `tiled/reader.rs` (`extract_external_paths`, `preload_external_resources`, `BytesResourceReader`), `tiled/map/loader.rs`, and the panic sites in `tiled/map/spawn.rs` and `tiled/helpers.rs`.
  * `bevy_ecs_tilemap` 0.19.0: `render/texture_array_cache.rs` and `render/extract.rs`.
  * The image path: `bevy_image` 0.19.1 (`image.rs:1596-1604`, `image_texture_conversion.rs`), `image` 0.25.10 (`codecs/png.rs:47-60`, `io/limits.rs`), and `png` 0.18.1 (`decoder/stream.rs:1685-1740`, `1856-1875`).
* Probes ran only from copies under `scratchpad/m1-review-2-probe/`:
  * Asset roots per case were selected with `BEVY_ASSET_ROOT`, and the real `target/debug/game` and `target/release/game` binaries were run with a `perl alarm` timeout.
  * A scratch crate `simprobe` (path dependency on `crates/sim`, the repository's `Cargo.lock`, its own target directory) calls `sim::load_map` in debug and release.
  * The reviewer's own seeded mutation fuzzer, `fuzz.py`, covers the map, an external-tileset map, and a mutated `.tsx`.

## Validation commands (PLAN.md section 9, with section 12 item 5), rerun by the reviewer

All commands ran from the repository root, in the foreground and in order. A display was available.

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 100 passed, 0 failed, 0 ignored: `game` unit 26, `m0_spike` 6, `m1_demo_map` 3, `sim` unit 2, `m0_bootstrap` 12, `m1_map` 32, `m1_map_subset` 19 (the mutation test took 5.79 s in debug), doc-tests 0. Only the known M0 linker warning appeared. |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | `map maps/demo/demo.tmx: 64x64 cells, tile 64x32 px`; `smoke test: 600 frames, 198 simulation ticks, map created: true` |
| `cargo run -p game -- --smoke-test 600` | 0 | `smoke test: 600 frames, 204 simulation ticks, map created: true` |

Additional checks:

* `cargo test --release -p sim` exited 0 (2 + 12 + 32 + 19 passed).
* `cargo tree -p sim -e normal` contains no `wgpu`, `winit`, `bevy_render`, `bevy_window`, `bevy_input`, `bevy_audio`, `cpal`, `rodio`, `gilrs`, `zstd`, `image`, or `png` crate. Direct dependencies: `bevy_app`, `bevy_ecs`, `quick-xml` 0.31.0, `rand_chacha`, `tiled` 0.16.0. All are pinned exactly (I1, I9).
* No test was removed, skipped, or weakened:
  * `m1_map.rs`, `m0_bootstrap.rs`, and `m0_spike.rs` are unchanged since review 1. The only `should_panic` test is in M0.
  * In the game tests, only the scratch directory of `offsets_and_parallax_are_reported` changed (P2-4), and its assertions are unchanged.
  * Remediation added 19 `sim` tests, 3 game map tests, and 1 projection test.

## Status of the round 1 findings

Each finding was checked against the code. The cases marked "binaries" were reproduced in scratch asset roots with both the debug and the release game binary (`--smoke-test 300`).

| Round 1 finding | Status | Evidence |
|-----------------|--------|----------|
| P0-1 tileset `margin`/`spacing` panic in `tiled` | **Fixed** | Binaries: the cases `margin="1000"` without `columns`, `spacing="4294967232"`, and `margin` in an external `.tsx` all exit 1 with `attribute "margin"`/`"spacing"`: `attribute is not supported`. The `.tsx` case names `ext.tsx`. No panic. `map_subset.rs:98-114` (allow-lists) and `:620-680` (geometry). |
| P0-2 deep nesting overflows the stack | **Fixed** | Binaries: 10,000 nested `<group>` exits 1 with `layer "": unexpected layer`, and 20,000 nested class properties exit 1 with `value "class" is not supported`. Both profiles; no abort. The checker is iterative and rejects at the first element outside the grammar (`map_subset.rs:382-388`, `:489-498`). |
| P0-3 unbounded allocation from `tilecount` | **Fixed for `tilecount`** | Binaries: `tilecount="4294967295"` exits 1 with `expected 1 to 1024`. Test `tilecount_is_bounded_before_tiled_allocates` runs under a 256 MiB allocator budget. **The same defect class is still reachable through the tileset PNG: see new P0-A.** |
| P0-4 renderer loaders panic on maps accepted by `sim` | **Fixed** | Binaries: `columns="8" margin="1000"` and `source="../../../demo_tiles.png"` exit 1 with a typed error before the hand-off (`is_plain_file_name`, `map_subset.rs:902-919`). |
| P1-1 inputs outside a subset reach the renderer | **Fixed for the reported cases** | Binaries: all of the following exit 1 with typed errors: tile id 100000, a 1×1 PNG for a declared 512×32 image, a single-quoted external reference, layer `offsetx`, and 128×64 tiles. **A remaining gap in the same class is new P1-A.** |
| P1-2 renderer-side load failure leaves a normal run going | **Fixed** | A normal (non-smoke) run whose PNG has a valid header and a corrupt body exits 1 in debug and release, with `Failed to load asset 'maps/demo/demo_tiles.png' ... Corrupt deflate stream`. Unit test `an_asset_load_failure_ends_the_game_with_an_error`. `exit_on_asset_load_failure`, `crates/game/src/map.rs:155-163`. |
| P1-3 errors in external tilesets name the map | **Fixed** | Binaries: `ext.tsx` with `margin` is reported as `.../ext.tsx: tileset "demo_tiles" attribute "margin"`. Test `errors_in_external_tilesets_name_the_tileset_file`. |
| P2-1 map file read twice | Open; recorded as debt in STATUS (item 1) | Unchanged. It now also covers the PNG: `check_tileset_image` reads the header and Bevy reads the file again. |
| P2-2 unterminated `</map` accepted | **Fixed** | `simprobe`: a file ending in `</map` returns `the root element's end tag is not terminated` (`map_subset.rs:220-244`). |
| P2-3 camera and picking systems tested through pure functions only | Open; recorded as debt (item 2) | Unchanged. |
| P2-4 fixed temporary directory in a game test | **Fixed** | `scratch_dir` is unique per process and test (`crates/game/src/map.rs:280-288`). |
| P2-5 `Grid` inserted directly as match setup | Open; recorded as debt (item 3) | Unchanged. |
| P2-6 D8 does not state that every tile id below `tilecount` needs a definition | **Fixed** | DECISIONS D8 item 4 now states it. Test `every_tile_id_below_tilecount_needs_a_definition`. |

## What was verified and holds

**Sim-side subset checker (D13, I8, I11).** The checker's reading rules match `tiled`'s:

* the same `quick-xml` 0.31.0 with default settings;
* `unescape_value` for attributes, as in `tiled`'s `get_attrs!`;
* the first text node, unescaped, for CSV, as in `read_text_or_cdata`;
* full-name matching, while `tiled` matches by local name.

The checker reproduces `bevy_ecs_tiled`'s `extract_external_paths` exactly (`map_subset.rs:863-879` against `reader.rs:69-89`). `tiled` reads only the checked bytes (`map.rs:343-356`).

Targeted `simprobe` cases, identical in debug and release, all returned a map or a typed `MapError` and never panicked:

* CR-only and CRLF line endings
* CSV commas written as `&#44;`
* negative, overflowing, and bit-28 gids
* a trailing comma
* a layer without `<data>`
* a tileset after the layer, and no tileset at all
* an empty root
* two roots
* a property value spanning lines that contains `source="evil.tsx"`
* `</map   >`
* a 1×1 map with empty `<data/>`
* a missing `.tsx`

**Reviewer's mutation fuzz.** `fuzz.py`, seed 20261004, generated 4,000 cases in three groups: the demo map, an external-tileset map, and a mutated `.tsx` (the implementer's mutation test does not mutate `.tsx` files). The results:

* 0 panics, and debug and release gave identical outcomes.
* 244 cases were accepted (183 unique), and **every unique accepted case was run through the release game binary: 183/183 exited 0**. Inside this corpus, everything `sim` accepts is renderable.

**Boundaries through the binaries.** These ran in both profiles, with exit 0 and the map created:

* 256×256 (`MAX_MAP_SIDE`), 1×1, and 1×256 maps;
* an external tileset;
* horizontal and diagonal flip flags (D8 item 5);
* grayscale and 16-bit RGBA PNGs of the declared size. `bevy_image` converts luma to RGBA8 (`image_texture_conversion.rs:22-27`), so grayscale is not mis-rendered.

**Section 5, equation 7.** `crates/game/src/projection.rs:30-90` is unchanged since review 1 except for one added test:

* `world = ((u−v)W/2, −(u+v)H/2)`;
* the inverse `a = x/(W/2)`, `b = −y/(H/2)`, followed by `floor`;
* the centre at `c + ½`, and the bounds rectangle.

All of these are correct. With 64×32 tiles, every coordinate up to 256 cells is a multiple of 0.5 px and lies far inside f32's 24-bit mantissa, so exact picking extends to the largest accepted map.

**Invariants:**

* I1 and I9 hold (see the validation section).
* I3 and I4: the grid is hashed in row-major order, and properties and tiles are sorted; `same_map_and_seed_give_equal_hashes_at_every_tick` passes.
* I5: no `Command` variants yet; the grid is installed as setup (P2-5 debt).
* I10: the tests use fixtures.
* I12: the map arithmetic in `sim` uses `u64` products and `try_from`. See P2-D for the M0 tick counter.

**Required M1 tests and acceptance criteria.** All required tests exist and pass, unchanged since review 1:

* the round trip for 1×1, 1×37, 37×1, and 64×64 maps;
* picking at centres, edges, and corners;
* picking outside the map;
* malformed maps (missing layer, unknown property, zero size);
* camera clamping at all four borders.

The section 8 categories are covered:

* positive, negative, and boundary tests (`maximum_map_size_loads`, 1025 and 0 tiles rejected);
* regression tests for each round 1 finding;
* integration tests through `SimPlugin`;
* end to end: the smoke runs.

## New findings

### P0

#### P0-A: A tileset PNG of a few megabytes makes the game allocate gigabytes, then render normally (unbounded allocation; I8, I11; debug and release)

* **Files and lines:**
  * `crates/game/src/map.rs:124-150`: `check_tileset_image` reads only the first 24 bytes (signature, IHDR, size).
  * `crates/game/src/map.rs:113-117`: hand-off to `bevy_ecs_tiled`, which loads the image through `bevy_image`.
  * DECISIONS D13, "Rendering input outside `sim`", defines the PNG's supported subset as that header check alone.
  * Third party: `bevy_image-0.19.1/src/image.rs:1602` calls `reader.no_limits()`. `image-0.25.10/src/codecs/png.rs:50-51` then passes `png::Limits { bytes: usize::MAX }`. During `read_info`, `png-0.18.1/src/decoder/stream.rs:1726` decompresses the iCCP chunk with `fdeflate::decompress_to_vec_bounded(buf, self.limits.bytes)`, which has no effective bound.
* **Evidence:** the demo map is unchanged, and `demo_tiles.png` is the original image with an iCCP chunk of compressed zeros inserted after IHDR. The file still reports `PNG image data, 512 x 32, 8-bit/color RGBA` and passes `check_tileset_image`. Measured with `/usr/bin/time -l`, smoke runs:

  | PNG file size | Profile | Peak memory footprint | Exit | Map created |
  |---------------|---------|-----------------------|------|-------------|
  | original (65,641 B) | release | 0.11 GB | 0 | true |
  | 587,490 B | release | 0.58 GB | 0 | true |
  | 2,152,926 B | release | 2.20 GB | 0 | true |
  | 2,152,926 B | debug (1,500 frames) | 2.25 GB | 0 | true |
  | 7,371,038 B | release | **7.58 GB** | 0 | true |

  * The amplification is about 1,000×, and it is linear in the chunk size. One PNG chunk may hold up to 2^31 − 1 bytes, so the allocation is bounded only by the machine.
  * No error or warning is logged, the map renders, and the smoke test passes. This is the same class as round 1 P0-3 (a small external file causes unbounded allocation), reached through the one external file whose subset is only a header check.
  * The tileset image file size is not bounded either: Bevy reads the whole file into memory. Unlike the map and `.tsx` files, there is no `MAX_*_BYTES` limit.
* **Expected behaviour (I8, I11):**
  * The PNG's supported subset is listed explicitly in D13, for example:
    * a maximum file size;
    * the critical chunks plus an allow-list of ancillary chunks, with no `iCCP`, `zTXt`, `iTXt`, `eXIf`, or animation chunks;
    * or a full decode inside the game under a fixed `image::Limits` budget.
  * It is checked before the hand-off and returns a typed error naming the image file.
  * A regression test feeds an iCCP or zTXt bomb and asserts the typed error within a memory budget, in debug and release.

### P1

#### P1-A: The tileset geometry subset admits images the renderer cannot create, and the failure has no file or field context (I11 subset incomplete; section 5 error context)

* **Files and lines:**
  * `crates/sim/src/map_subset.rs:612-680` accepts any `columns × rows ≤ MAX_TILESET_TILES` (1024), so an image can be up to 65,536 px wide or 32,768 px tall.
  * `crates/game/src/map.rs:124-150` checks only that the PNG size equals the declared size.
  * Third party: `bevy_ecs_tiled` hands the image to `bevy_ecs_tilemap` as `TilemapTexture::Single`, and Bevy creates a 2D texture of the full image size. On this machine's adapter (Apple M5, Metal) the limit is 16,384.
* **Evidence:** each case is the demo map with a tileset of `columns × rows` tiles, all tiles defined, every cell set to tile 0, and a valid PNG of exactly the declared size:

  | Case | Image | sim and header check | Release smoke | Debug smoke | Release normal run |
  |------|-------|----------------------|---------------|-------------|--------------------|
  | 256 × 1 | 16384×32 | accepted | exit 0, map created | | |
  | 257 × 1 | 16448×32 | accepted | **exit 1** | **exit 1** | **exit 1** |
  | 1024 × 1 (maximum accepted) | 65536×32 | accepted | **exit 1** | | |
  | 1 × 513 | 64×16416 | accepted | **exit 1** | | |
  | 1 × 1024 | 64×32768 | accepted | | | **exit 1** (debug) |

  * The only diagnostics are `ERROR bevy_render::error_handler: Caught rendering error: Validation Error ... In Device::create_texture  Dimension X value 16448 exceeds the limit of 16384`, then follow-on errors (`Texture with '' label is invalid` in `create_texture_array_from_atlas`), then `Quitting the application due to Validation RenderError`. No panic was observed.
  * Neither the map nor the image file nor the offending attribute is named. PLAN.md section 5 requires that a load error names the file, the field, and the reason.
  * The subset that D13 lists as supported therefore contains inputs the renderer cannot handle. Those inputs are rejected only after the hand-off, by the third party, without context. That is the condition I11 exists to prevent.
  * No test exercises the largest accepted tileset (`tilecount` = `MAX_TILESET_TILES`, or the widest or tallest image) as a positive boundary. The tests cover only 1025 and 0 as rejected values.
* **Expected behaviour:**
  * D13 bounds the image size to a dimension the renderer is guaranteed to support. Examples are `columns × 64 ≤ 8192` and `rows × 32 ≤ 8192`, which is wgpu's downlevel default `max_texture_dimension_2d`, or the device limit queried before the hand-off.
  * The value of `MAX_TILESET_TILES` and the geometry rules are made consistent with that bound.
  * A violation is a typed `MapError` naming the file, the image attribute, and the limit.
  * Boundary tests cover the largest accepted image, which must render through the binary's smoke test, and the smallest rejected one.

### P2

* **P2-A Misleading rejection reasons.** All of these inputs are rejected correctly and without panic, but the reason is wrong or imprecise:
  1. A UTF-8 byte order mark is reported as `the root element's end tag is not terminated`, observed in `simprobe` and in the binaries. `quick-xml` skips the 3 BOM bytes, so `buffer_position` in `end_of_document` (`map_subset.rs:221-222`) is offset by 3. STATUS debt item 5 says the checker treats the BOM as "text before the root"; that is not what happens.
  2. A map without a tileset, or with the tileset after the layer, is reported as `cell (0, 0): cell has no tile`. D13 says "exactly one `tileset`", but the checker enforces only "at most one" (`map_subset.rs:366-372`).
  3. A missing `.tsx` is reported with field `file` rather than the tileset reference's `source` attribute (`map_subset.rs:564-572`).
  4. Errors in a `.tsx` are printed as `map <path>.tsx: ...`, because `MapError`'s `Display` always says "map" (`map.rs:153-163`).

  Expected: each reason names the actual cause, and D13 and the checker agree on "exactly one".
* **P2-B Floating point type in `sim`.** `map_subset.rs:503` uses `value.parse::<f32>()` to validate `float` properties. No simulation state results from it, and the parse is deterministic, so I2 ("state and logic") is arguably not violated. However, review 1 recorded "no `f32`/`f64` in `crates/sim/src`", and the subset admits `float` and `string` property types although the schema has no property of those types; such properties are rejected only afterwards, as unknown. Expected: narrow the property types to `int` and `bool` (or record why not), which removes the f32 use.
* **P2-C The double read now spans three files.** This extends carried debt P2-1. The map, the `.tsx`, and the PNG are each checked by one reader and read again by Bevy. This is already recorded as debt and is listed here only because P0-A's remedy (decode or limit in the game) should also close the PNG part of it.
* **P2-D I12 and the M0 tick counter.** `crates/sim/src/schedule.rs:111` uses `tick.0 += 1` on a `u64`. It panics in debug and wraps in release at `u64::MAX`, which is unreachable in practice (about 2.9 × 10^10 years at 20 Hz). I12 was introduced during M1. Expected: `wrapping_add`, `checked_add`, or a recorded justification.

## Implementer claims checked

| Claim | Outcome |
|-------|---------|
| 100 tests pass; all six validation commands exit 0 | Confirmed. |
| `cargo test --release -p sim --test m1_map --test m1_map_subset` passes | Confirmed (with all `sim` tests). |
| Round 1 inputs exit 1 with a typed error in both profiles; a corrupt PNG exits 1 in a normal run | Confirmed (table above). |
| D13: an image that passes the header check but cannot be decoded fails to load, and D14 makes that fatal | True as stated, but incomplete: a decodable hostile PNG allocates without bound (P0-A), and an over-size PNG fails in the renderer without context (P1-A). |
| The game hands `bevy_ecs_tiled` only maps that passed `sim::load_map` and the image check | True. The claim in the module documentation is correctly limited to the inputs those checks cover. |
| D15: the "fails before" regression evidence | Not rerun against the pre-fix code, because the new tests use API added by the fix. The pre-fix behaviour is documented by review 1, and the post-fix behaviour was confirmed above. |
| STATUS debt item 5: a BOM is rejected as text before the root | Inaccurate mechanism (P2-A). |
| The 6,000-case mutation test passes in about 6 s (debug) | Confirmed (5.79 s). |

## Requires user verification

These items are not findings and do not affect the decision (section 12 item 2). Run `cargo run --release -p game` and check, as listed in docs/STATUS.md:

1. **Map rendering:** a 64×64 green diamond map, tiles edge to edge. Red at the top corner (0, 0), magenta at the right corner (63, 0), white at the left corner (0, 63). A grey rock ridge with a gap, a blue lake with a sand shore right of centre, and gold ore fields.
2. **Picking:** the white outline and centre dot sit on the tile under the cursor, including along all four edges, and disappear outside the map. A left click logs the cell, or `click outside the map`.
3. **Panning:** arrow keys (diagonals not faster); the screen edge within 8 px, only while the window has focus; middle mouse drag keeps the grabbed point under the cursor.
4. **Zoom:** the wheel or trackpad, limited to 2× and 0.5× magnification.
5. **Bounds:** the view never leaves the map's bounding rectangle at any zoom or window size, and stays vertically centred when the view is taller than the map.

## Decision rationale

All six validation commands pass. Every round 1 P0 and P1 finding is fixed as reported, the sim-side subset checker is sound against everything probed, and nothing `sim` accepted in the reviewer's fuzz failed to render. However, the remediation's claim to cover rendering inputs does not hold for the tileset image:

* **P0-A:** a few-megabyte PNG that passes the game's header check drives the game to multi-gigabyte allocations in both profiles, with no error. This is an unbounded allocation from a data file, the same class as round 1 P0-3, and a violation of I8 and I11.
* **P1-A:** tileset images inside the listed subset exceed the renderer's texture limit and end the game through a third party error without file or field context.

Section 10 makes NO-GO mandatory for any unresolved P0 or P1.

Findings: 1 P0, 1 P1, 4 P2.

Decision: NO-GO
