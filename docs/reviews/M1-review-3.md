# M1 independent review 3: Map and camera

Reviewer: independent review subagent. I did not implement or remediate this code. Date: 2026-10-04.

## Scope and base

* Repository `/Users/xingyuwang/GameProjects/MyGame001`, branch `milestone/m1`, head `352e526`. The working tree was clean before and after the review (`git status --short` empty). No repository file was created or modified; only `target/` build output changed.
* Base: tag `m0-approved`, the most recent reviewed or approved tag. Reviewed diff: `git diff m0-approved..milestone/m1`, 33 files, +7657/−74. Commits: `41c4d51`, `b2862c9`, `712bd3b`, `af7f0f4`, `d495afb`, `d1304e6`, `deb2859`, `efb25df`, `352e526`.
  * `af7f0f4` is the user-approved PLAN.md amendment (DECISIONS U2) and is not milestone code. Its content (macOS only, the debug smoke command, section 12 with I11 and I12) is applied as the review standard.
  * `deb2859` is remediation round 1 and `352e526` is remediation round 2. The round 2 delta (`efb25df..352e526`, 15 files, +1693/−86) was read line by line.
* Read in full: PLAN.md sections 1 to 12, docs/STATUS.md, docs/DECISIONS.md, docs/reviews/M1-review-1.md, and docs/reviews/M1-review-2.md. Source files: `crates/sim/src/{map,map_subset,schedule,hash,lib}.rs` and `crates/game/src/{map,tileset_image,checked_assets,main,smoke,projection}.rs`. Also `camera.rs` (`clamp_centre` and the constants), and the structure and key tests of `crates/game/tests/m1_tileset_image.rs` and `crates/sim/tests/m1_map_subset.rs`.
* Third party code that consumes the external files, read at the relevant call sites:
  * `png` 0.18.1: `decoder/mod.rs` (`next_frame`, `finish_decoding`), `decoder/read_decoder.rs` (`finish_decoding_image_data`), `decoder/zlib.rs` (`decompress`, `finish`, `UnfilterBuf::decompress`), and `decoder/unfiltering_buffer.rs`.
  * `image` 0.25.10: `codecs/png.rs` (`with_limits`).
  * `bevy_ecs_tilemap` 0.19.0: `render/texture_array_cache.rs` (the tile count and the extra layer for counts of 1 or a multiple of 6).
* Probes ran only from `scratchpad/m1-review-3-probe/`:
  * The asset root of each case was a copy selected with `BEVY_ASSET_ROOT`. The real `target/debug/game` and `target/release/game` binaries ran under `perl -e 'alarm N'` and `/usr/bin/time -l`.
  * A scratch crate `simprobe` (outside the repository, with a copy of the repository's `Cargo.lock`, its own target directory, and a path dependency on `crates/sim`) includes `crates/game/src/tileset_image.rs` by path. It calls `sim::load_map_with_files` and then `check_tileset_image`, with `catch_unwind`, in debug and release.
  * Two seeded mutation fuzzers of my own: `fuzz.py` (maps and external `.tsx` files) and `fuzzpng.py` (tileset PNGs).
  * The memory watchdog killed no probe. The highest peak memory footprint of any probe was 0.12 GB.

## Validation commands (PLAN.md section 9, with section 12 item 5), rerun by the reviewer

All commands ran from the repository root, in the foreground and in order. A display was available.

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 115 passed, 0 failed, 0 ignored: `game` unit 29, `m0_spike` 6, `m1_demo_map` 3, `m1_tileset_image` 6 (2.74 s), `sim` unit 3, `m0_bootstrap` 12, `m1_map` 32, `m1_map_subset` 24 (5.72 s), doc-tests 0. Only the known M0 linker warning (`__eh_frame section too large`) appeared. |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | `map maps/demo/demo.tmx: 64x64 cells, tile 64x32 px`; `smoke test: 600 frames, 198 simulation ticks, map created: true` |
| `cargo run -p game -- --smoke-test 600` | 0 | `smoke test: 600 frames, 203 simulation ticks, map created: true` |

Additional checks:

* `cargo test --release -p sim -p game` exited 0 with the same 115 tests.
* `cargo tree -p sim -e normal` contains no `wgpu`, `winit`, `bevy_render`, `bevy_window`, `bevy_input`, `bevy_audio`, `cpal`, `rodio`, `gilrs`, `zstd`, `image`, or `png` crate (I1).
* Every direct dependency is pinned with `=` (I9). Across all of M1, `Cargo.lock` changed only in the dependency lists of `sim` (`quick-xml`, `tiled`) and `game` (`image`, and the dev-dependencies `flate2` and `crc32fast`). No package was added.
* CLI regression (M0): `--smoke-test 0`, `--smoke-test`, `--bogus`, and `--smoke-test 4294967296` exit 2. A missing asset root exits 1 and names the map path.
* Normal (non-smoke) runs:
  * an `iCCP` bomb and a 129×1 tileset exit 1 with a typed error in both profiles (D14);
  * a valid map keeps running until killed by the alarm (exit 142), with no error.

## Tests removed, skipped, or weakened

None.

* `m0_bootstrap.rs`, `m0_spike.rs`, and `cli.rs` are unchanged since `m0-approved`. The only `should_panic` test is in M0.
* Between `efb25df` and `352e526`, `m1_map_subset.rs` only gained lines.
* In `m1_map::malformed_xml_and_wrong_root_are_parse_errors`, the missing-`.tsx` case now asserts the exact field, the file, and a `Read` kind, which is stricter than before.
* In `game::map::tests::tileset_images_other_than_the_declared_png_are_rejected`, the five rejections are unchanged. The assertion that a header-only file is accepted became a rejection, which is the behaviour review 2 P0-A required, and an assertion that a complete PNG is accepted was added.
* `crates/game/src/spike.rs` (deleted in M1) had no tests.

## Scope

All changes serve the M1 scope (map loading, `Grid`, rendering, camera, picking) or a review finding. The one change to M0 code is `advance_tick` using `wrapping_add` (I12, review 2 P2-D), which comes with a unit test.

## Status of earlier findings

### Review 1 (re-verified with `simprobe`, debug and release, identical results)

| Finding | Status | Evidence |
|---------|--------|----------|
| P0-1 `margin`/`spacing` panic in `tiled` | **Fixed** | `margin="1000"` without `columns`, and `spacing="4294967232"`: `attribute "margin"`/`"spacing"`: `attribute is not supported`. |
| P0-2 deep nesting overflows the stack | **Fixed** | 10,000 nested `<group>`: `unexpected layer`. 20,000 nested class properties: `value "class" is not supported`. No abort. |
| P0-3 unbounded allocation from `tilecount` | **Fixed** | `tilecount="4294967295"`: `expected 1 to 256`. |
| P0-4 renderer loaders panic on accepted maps | **Fixed** | `columns="8" margin="1000"` and `source="../../../demo_tiles.png"` are rejected with typed errors. |
| P1-1 inputs outside a subset reach the renderer | **Fixed** | All rejected with typed errors: tile id 100000, a single-quoted reference, 128×64 tiles, and layer `offsetx`. |
| P1-2 renderer-side load failure leaves a normal run going | **Fixed** | `exit_on_asset_load_failure` and its unit test. Normal runs with rejected inputs exit 1 (above). |
| P1-3 errors in external tilesets name the map | **Fixed** | Errors inside a `.tsx` name the `.tsx` and print `tileset <path>` (probes `tsx_big`, `tsx_bom`, `tsx_devzero`). |
| P2-1 double read | **Fixed** | D16 item 3: `CheckedAssetReader` serves only the checked bytes. |
| P2-2 unterminated `</map` | **Fixed** | `the root element's end tag is not terminated`. |
| P2-3 camera and picking systems tested through pure functions | Open; recorded as STATUS debt | Unchanged. |
| P2-4 fixed temporary directory | **Fixed** | `scratch_dir` is unique per process. |
| P2-5 `Grid` inserted as match setup | Open; recorded as STATUS debt | Unchanged. |
| P2-6 D8 tile-definition rule | **Fixed** | D8 item 4. |

### Review 2

| Finding | Status | Evidence |
|---------|--------|----------|
| P0-A PNG with a compressed ancillary chunk allocates gigabytes | **Fixed** | `crates/game/src/tileset_image.rs` reads the image with a bound, checks the signature, `IHDR`, and a chunk allow-list, and decodes under `image::Limits` (16 MiB). Binaries, debug and release, `--smoke-test 150`: a 500 MiB `iCCP` bomb exits 1 with `chunk "iCCP": chunk is not supported`, peak 0.10 GB. Remaining PNG allocation paths: in `png` 0.18.1, data after the end of the image's zlib stream is skipped without being inflated (`zlib.rs:115-118` and `:145-150`, `read_decoder.rs:128-134`). Confirmed by these accepted cases, each exiting 0 with the map created at peak 0.12 GB: 3 GiB of zeros appended inside the same zlib stream (3.1 MB file); a second 3 GiB zlib stream in a later `IDAT`; 300,000 empty `IDAT` chunks; and a file of exactly 4 MiB. A file of 4 MiB + 1 byte is rejected, as are a symlink to `/dev/zero` (`the file is larger than 4194304 bytes`), a directory, and an unreadable file. |
| P1-A tileset images beyond texture limits, failing without context | **Fixed** | `MAX_TILESET_TILES` = 256 and `MAX_TILESET_IMAGE_SIDE_PX` = 8192, checked in `map_subset.rs:637-694`. The test `tileset_limits_fit_the_texture_limits_every_gpu_supports` matches `texture_array_cache.rs:74-83` and `:156-171`. Binaries, debug and release, each with a matching PNG: the 128×2, 2×128, 1×256, 126×2 (252 tiles, 253 layers), 64×4, 6×1, and 1×1 tilesets exit 0 with the map created and no `ERROR`/`WARN` from the renderer. The 129×1, 1×257, and 16×17 tilesets exit 1 with a typed `MapError` naming the image attribute or `tilecount`. |
| P2-A.1 BOM reported as an unterminated end tag | **Fixed** | Map and `.tsx`: `a UTF-8 byte order mark at the start of the file is not supported`. |
| P2-A.2 missing or misplaced tileset | **Fixed** | `map element <tileset>: required element is missing` / `must come before the layer "terrain"`. |
| P2-A.3 missing `.tsx` at field `file` | **Fixed** | Field `tileset "ext.tsx" attribute "source"`, kind `Read` naming the `.tsx` path (also for a directory). |
| P2-A.4 `.tsx` errors printed as `map` | **Fixed** | `tileset <path>.tsx: …`. |
| P2-B `f32` parse in `sim` | Open; recorded as STATUS debt (round 2 item 4) | `map_subset.rs:524`. The result is a discarded `bool`; no simulation state uses it. |
| P2-C the double read spans three files | **Fixed** | As for review 1 P2-1. A 400 MB sparse `demo_tiles.png.meta` and a malformed `demo.tmx.meta` have no effect (exit 0, peak 0.11 GB). |
| P2-D tick counter and I12 | **Fixed** | `wrapping_add` and the test `tick_counter_wraps_instead_of_overflowing`. |

## What was verified and holds

**Sim-side subset (D8, D13, D17; I8, I11).** My map fuzzer (`fuzz.py`, seed 20261004) generated 6,000 cases in three groups: the embedded demo map, an external-tileset map, and a mutated `.tsx`. It used 14 operators:

* numeric boundary values;
* insertion of attributes and elements from pools of Tiled features outside the subset;
* deletion, duplication, and reordering of lines;
* CSV gid edits, including flag bits;
* CRLF and CR line endings;
* string and entity-escaped values;
* single quotes;
* truncation and byte replacement;
* element renames and namespaced names.

Results:

* 0 panics; debug and release gave identical outcomes.
* 5,238 cases were rejected with a `MapError` and 762 were accepted (453 unique).
* **All 453 unique accepted cases ran through the release game binary with exit 0 and no `ERROR`/`WARN`/panic line.** A sample of 90 also ran through the debug binary, with the same result.
* Everything `sim` and the image check accept in this corpus is renderable.

**Tileset image subset (D16 item 2).** My PNG fuzzer (`fuzzpng.py`, seed 7) generated 2,000 mutants of four valid bases. It used byte flips, chunk insertion from a pool (`tEXt`, `tRNS`, `PLTE`, `acTL`, `sBIT`, `eXIf`, `cICP`, extra `IDAT`/`IEND`, and allowed ancillary chunks), reordering, duplication, deletion, truncation, length-field edits, and `IHDR` edits with a recomputed CRC.

Results:

* 0 panics; debug and release gave identical outcomes.
* 1,900 cases were rejected with a typed `TilesetImageError` naming the image file and a field.
* 100 were accepted (23 unique); **all 23 exit 0 in the release binary** with the map created.

Hand-made cases:

* Rejected with a typed error in both profiles:
  * a CRC error in `IDAT`;
  * an unknown filter byte;
  * a truncated zlib stream;
  * a zlib preset dictionary;
  * a missing `IEND`;
  * interlacing;
  * 16-bit depth;
  * greyscale.
* Accepted and rendered:
  * RGB8;
  * all four allowed ancillary chunks with odd values;
  * a bad CRC on `sRGB`, which `png` ignores for ancillary chunks, so the check and Bevy agree.

**Renderer input set (D16 item 3).** `CheckedAssetReader` serves only the registered bytes, never reads `.meta` files, and refuses every other path. These map variants all render, so the registered keys match the paths `bevy_ecs_tiled` and Bevy request: an external tileset `ext.tsx`; dotted names `ts.v2.tsx` and `tiles.v2.png`; mixed-case names `EXT.tsx` and `Tiles-01.png`; and embedded images named `a.tmx.png` and `x.tsx.png` (Bevy falls back from the full extension to `png`).

**Section 5, equation 7** (`crates/game/src/projection.rs:30-90`, unchanged since review 1 apart from tests). The implementation matches the plan:

* `world_x = (u − v)·W/2` and `world_y = −(u + v)·H/2`;
* the inverse `a = x/(W/2)`, `b = −y/(H/2)`, `cx = floor((a + b)/2)`, `cy = floor((b − a)/2)`;
* the centre at `c + ½`, consistent with `SubPos = c·256 + 128`;
* the boundaries: shared edges belong to the larger coordinate, non-finite points return `None`, and `as i32` saturates.

The reconciliation with `bevy_ecs_tilemap` (D7, D9) is tested for every cell of six sizes. 64×32 is the only accepted tile size, so exact picking holds for every accepted map. The required tests pass:

* `cell_world_cell_round_trip_for_every_cell` (1×1, 1×37, 37×1, 64×64);
* `picking_at_centres_edges_and_corners`;
* `picking_outside_the_map_returns_none`;
* `missing_layer_is_rejected`, `unknown_tile_property_is_rejected`, `zero_size_is_rejected`;
* `clamping_at_all_four_borders`.

The other section 5 equations (1 to 6) concern later milestones.

**Invariants.**

* I1 and I9 hold (see above).
* I2: the only floating point use in `crates/sim/src` is the discarded `f32` parse (P2-B debt).
* I3 and I4: the grid is hashed in row-major order, properties and tiles are sorted, and `same_map_and_seed_give_equal_hashes_at_every_tick` passes.
* I5: no `Command` variants yet. The grid is installed as match setup (P2-5 debt).
* I6 and I7: not applicable in M1.
* I8: no panic from any malformed input I generated (8,000 fuzz cases plus hand-made cases, debug and release).
* I10: the tests use fixtures.
* I11: every external input the renderer receives (map, `.tsx`, image, `.meta`) is restricted before the hand-off. `tiled` parses only checked bytes, and errors that `tiled` itself returns are typed `MapError`s (the "checking the parsed result" mode of I11). See P2-2 for their context.
* I12: every arithmetic operation in `crates/sim/src` was checked. Products are taken in `u64` of values bounded by the subset, and the code otherwise uses `try_from`, `checked_add`, or `wrapping_add`.

**Section 8 categories.**

* Positive and negative tests.
* Boundary tests: 256 tiles and 8192 px accepted; 257 tiles, 8193 px, and 4 MiB + 1 byte rejected.
* A regression test for every review 1 and review 2 finding.
* Integration through `SimPlugin`.
* End to end: the smoke runs in both profiles.

## New findings

### P0

None.

### P1

None.

### P2

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

## Implementer claims checked (round 2 report)

| Claim | Outcome |
|-------|---------|
| 115 tests pass; all six validation commands exit 0 | Confirmed (also in release). |
| `iCCP` bomb, 16-bit greyscale, and 257×1/256×1/129×1 tilesets exit 1 with a typed error in both profiles | Confirmed (`iCCP`, 16-bit, greyscale, 129×1, 1×257, 16×17). |
| The 128×2, 128×1, 1×256, and 16×16 tilesets render in both profiles | Confirmed (128×2, 2×128, 1×256, 126×2, 64×4, 6×1, 1×1). |
| A 400 MB `.meta` file has no effect; the renderer reads only checked bytes | Confirmed by binary run and source. |
| An image with 500 MiB of compressed zeros after its pixel data inside `IDAT` is accepted, with bounded memory | Confirmed with 3 GiB, in the same stream and as a second stream (peak 0.12 GB). Source reading agrees. |
| `Cargo.lock` gained no package | Confirmed. |
| "Fails before" evidence for the round 2 tests | Not rerun: the tests use API added by the fix. The pre-fix behaviour is documented by review 2 and is consistent with the code at `efb25df`. |
| Every other integer operation in `crates/sim/src` cannot overflow | Confirmed by inspection. |

## Requires user verification

These items are not findings and do not affect the decision (PLAN.md section 12 item 2). They block only the user's tag `m1-approved`. Run `cargo run --release -p game` and check, as listed in docs/STATUS.md:

1. **Map rendering:** a 64×64 green diamond map, tiles edge to edge with no gaps or overlaps. Red at the top corner (0, 0), magenta at the right corner (63, 0), and white at the left corner (0, 63). A grey rock ridge with a gap, a blue lake with a sand shore right of centre, and gold ore fields.
2. **Picking:** the white outline and centre dot sit on the tile under the cursor, including along all four edges, and disappear outside the map. A left click logs the cell (for example `cell (0, 0): passable true, buildable true, ore 0`), or `click outside the map`.
3. **Panning:** the arrow keys pan, and diagonals are not faster. The screen edge pans within 8 px, only while the window has focus. Dragging with the middle mouse button keeps the grabbed point under the cursor.
4. **Zoom:** the wheel or trackpad zooms, limited to 2× and 0.5× magnification.
5. **Bounds:** the view never leaves the map's bounding rectangle at any zoom or window size, and stays vertically centred when the view is taller than the map.

## Recorded debt that must be carried in docs/STATUS.md

* The new items P2-1, P2-2, and P2-3 above.
* Still open from earlier rounds and already recorded in STATUS:
  * review 1 P2-3 and P2-5;
  * review 2 P2-B;
  * round 1 item 4 (an asset load failure is logged twice under `--smoke-test`);
  * round 2 items 5 (WebGL2 limits are unsupported) and 6 (every asset must be registered in `CheckedAssets`);
  * M0 items 1, 2, 3, 5, 7, 8, and 9.

## Decision rationale

All six validation commands pass, and the 115 tests also pass in release. Every review 1 and review 2 P0 and P1 finding is fixed, which I re-verified with the real binaries in both profiles. The rendering inputs are now restricted before the hand-off:

* the map and `.tsx` by the D13 subset;
* the image by the D16 PNG subset and a bounded decode with Bevy's decoder;
* `.meta` files are never read;
* the renderer receives only the checked bytes.

My own fuzzing of maps, external tilesets, and PNGs (8,000 cases) found no panic, no debug/release divergence, and no unbounded allocation. Everything accepted rendered with exit 0. The required M1 tests and the section 5 projection equations hold, and no test was removed, skipped, or weakened.

The three new findings are P2: a liveness gap for non-regular files, imprecise error context for non-numeric CSV entries, and stale documentation. None is a crash, data loss, invariant violation, incorrect result, missing acceptance criterion, or silent fallback. The visual acceptance items require user verification under section 12 item 2 and do not affect this decision. The decision requires that P2-1, P2-2, and P2-3 be recorded in docs/STATUS.md, together with the carried debt listed above.

Findings: 0 P0, 0 P1, 3 P2.

Decision: GO WITH DOCUMENTED NON-BLOCKING DEBT
