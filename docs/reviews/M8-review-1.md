# M8 review 1: visual polish first

Independent review under PLAN.md sections 10, 12 and 13. The reviewer did not implement or
remediate this code. Only this report is changed by the reviewer.

## Scope and decision

**Decision: NO-GO.** One new P1 gameplay-presentation regression remains: five unit types have
identical friendly and enemy sprites in their north-west facing. This is a measurable loss of
ownership information, not a judgment of artistic quality. Remediate it in a fresh implementer
session, retain a regression test, and obtain independent re-review before merging/tagging M8.

Reviewed branch `milestone/m8`, head `2207a2b`, against `m7-reviewed` (`eb167f6`): 46 files,
1,244 insertions and 248 deletions, including implementation `3e302c2` and scope `8d19214`.
The reviewer read PLAN.md in full, the M8 implementation/status report, U3 and D58–D60, the
M7 independent review and its inherited debt, every changed production code path, the generators,
the packaging change and provenance records. `docs/HANDOFF.md` was already untracked and remains
untouched. No review code or asset fixes are included.

The approved scope is visual polish first on Apple Silicon macOS. Audio replacement, inherited
gameplay debt, signing/notarisation and full release completion are deferred. Generic names are
accepted; collecting Rust notices is not a user-requested M8 deliverable. None of those inherited
items is promoted into a new blocker here. User visual acceptance remains **requires user
verification**, and cannot itself cause NO-GO under PLAN.md section 12.

## P1-1: Preserve visible team paint in every unit facing

**Locations:** `crates/game/examples/gen_unit_sprites.rs:407–414` limits infantry paint to the
front of the torso, which is occluded in the rear view; `:234–246` draws the rocket rack before
the hull team panels at `:328–329`, hiding those panels in the north-west frame.
`crates/game/src/units.rs:289–309` now selects a team texture and uses white sprite tint.
`crates/game/src/presentation.rs:7–13,30–36` changes only the authored blue-paint pixels.

**Trigger and result:** a rifleman, conscript, rocket soldier, grenadier or rocket launcher faces
north-west (`Direction::ALL[5]`, cell delta `(-1,-1)`). Its 64×64 frame contains zero pixels
matched by the production recoloring predicate. Applying `team_pixel` to every pixel leaves
the entire frame unchanged, including alpha. Consequently the friendly and enemy textures
render identically for that frame, unlike the previous whole-sprite owner tint. This affects
ordinary movement and combat, including same-faction skirmishes. Unselected full-health units
have no health bar; health bars otherwise encode health rather than ownership
(`crates/game/src/health.rs:40–53`). Selection is not a persistent ownership indication for all
visible units. The minimap does not restore the missing distinction on the battlefield.

**Independent reproduction:**

1. A Python standard-library probe decoded every committed unit PNG (validated filter-zero
   generated RGBA8 data), applied the exact predicate/color arithmetic, and counted changed
   pixels in each frame.
2. A separate Rust probe directly included the actual unit and building generator modules and
   copied the production `team_pixel` function verbatim from `presentation.rs`. It counted
   differences after invoking that function on every generated pixel. It compiles successfully
   with the pinned toolchain; its assertion that no facing loses all ownership pixels exits
   **101**, listing exactly the five units above at frame 5. The unchanged reproducibility
   tests pass, connecting the generator pixels with the committed PNGs.
3. Both probes agree on every count below. Sources and logs are retained in
   `/Users/xingyuwang/Library/Caches/Codex/reviews/MyGame001/m8-review-1/` as
   `team-paint-probe.py`, `team-paint-probe.log`, `team-paint-production-probe.rs` and
   `team-paint-production-probe.log`. The first standalone Rust compile lacked Cargo's
   compile-time `CARGO_MANIFEST_DIR`; supplying the actual `crates/game` path corrected that
   harness setup issue. No repository source was modified.

Counts are pixels that differ between human and opponent after the production transform:

| Unit | E | SE | S | SW | W | NW | N | NE |
|------|--:|---:|--:|---:|--:|---:|--:|---:|
| light tank | 39 | 54 | 39 | 39 | 29 | 50 | 29 | 39 |
| scout | 23 | 16 | 23 | 10 | 25 | 18 | 25 | 10 |
| harvester | 70 | 66 | 70 | 72 | 54 | 60 | 54 | 72 |
| rifleman | 32 | 34 | 33 | 12 | 4 | **0** | 4 | 12 |
| conscript | 32 | 34 | 33 | 12 | 4 | **0** | 4 | 12 |
| rocket soldier | 31 | 34 | 33 | 12 | 4 | **0** | 4 | 12 |
| grenadier | 31 | 34 | 33 | 12 | 4 | **0** | 4 | 12 |
| raider | 25 | 40 | 27 | 41 | 22 | 40 | 24 | 41 |
| heavy tank | 61 | 89 | 62 | 61 | 47 | 81 | 48 | 61 |
| artillery | 29 | 55 | 29 | 48 | 26 | 43 | 26 | 48 |
| rocket launcher | 40 | 30 | 40 | 48 | 19 | **0** | 19 | 48 |

All eight building masks are nonempty: construction yard 472, power plant 620, refinery 1,210,
barracks 931, war factory 1,030, gun turret 214, tech center 1,185, rocket tower 266.

**Required remediation:** author a readable, nontrivial team-colored patch that remains visible
in every facing of every shipped unit, including the four-pixel infantry side/rear cases. Do not
close this with a single invisible marker pixel. Preserve natural materials, alpha, silhouette,
frame anchors and existing facing tests. Add a generated-sheet/production-transform regression
over the entire roster and all eight facings that enforces meaningful visible coverage, plus
material/alpha preservation. Inspect friendly and enemy rear views at normal gameplay scale.

**Why existing tests miss it:** the new palette test exercises sample pixels only; it never
checks that those pixels occur in a rendered frame. Existing facing-centroid tests inspect
gray front/barrel markers, not team paint. Neither test was weakened, but their passing results
cannot establish owner legibility. The five submitted captures show the human base and its
starting units rather than opposing rear-facing armies.

No additional new P0, P1 or P2 finding was established.

## Review of the remaining changes

### Production path and checked loading

`skirmish::load` still cross-validates every definition, sprite path and map before catalogue
installation. Each sprite passes `check_png_file` with the expected dimensions and bounded
read/decode before `sprite_pair` receives those same bytes. The new path does not reopen disk
files after checking. PNG constraints remain RGB/RGBA8, no interlace, bounded image dimensions,
bounded input/decode memory and the explicit uncompressed ancillary-chunk allowlist. Failures
at load remain contextual typed errors; an unexpected Bevy decode failure names the checked
asset path and requests error exit instead of a placeholder.

The pinned local `bevy_image-0.19.1` implementation was inspected: `Image::from_buffer` decodes
PNG through the same image library, and `from_dynamic` converts RGB8 to RGBA8 and retains RGBA8
as an `Rgba8UnormSrgb` texture. Thus iterating the checked image in four-byte groups is supported
by the established subset, and the optional `Image.data` branch is populated in this path.
No new third-party parser input or unsupported image format is introduced. The palette arithmetic
is bounded; alpha is unchanged. Missing image-map entries remain defensive skips from the prior
view-spawn design, with both maps populated together on successful catalogue load.

### Geometry, simulation and invariants

The generator/source changes were reviewed alongside the existing geometry tests. Buildings
retain footprint dimensions, the 32-pixel wall/image convention and ground placement. Units
retain 64×64 frames, the ground anchor and eight-direction order. All asset reproducibility,
PNG-subset, footprint-corner and facing-centroid assertions pass unchanged. The shared painter
uses only fixed authored shapes and clips raster writes; its arithmetic is outside `sim`.
Terrain cell data, flags and projection are unchanged.

`git diff m7-reviewed..HEAD -- crates/sim assets/data Cargo.toml Cargo.lock rust-toolchain.toml`
is empty. Equations and units of PLAN.md section 5, deterministic schedule and all simulation
arithmetic are unchanged (I1–I4, I6–I10, I12). The renderer continues to read the private
`SimHost` world through shared access. The diagnostic developed-base mode starts a new match
through existing pre-tick `install_match`/`setup_match`, then restores the production catalogue;
it does not mutate a live simulation through a bypass (I5). The separate diagnostic cannot
activate during smoke tests. The authored data and fixtures keep their schemas.

### Interface, assets and test preservation

The sidebar retains its 240-pixel input boundary, effects, hotkeys, roster filtering and production
semantics. The two-column cards retain names, costs, status badges and progress bars. The menu
keeps the existing selection/effect behavior with DEPLOY as the Start label. Native default-size
captures were inspected for clipped labels, control visibility and overlap: no concrete layout
failure was found there. Other window sizes, reading comfort and final art direction remain
user verification. Menu gallery/production icons use the same checked textures.

All five committed captures were opened: menu, demo start, labeled developed base, Crossroads
with Meridian and Riverlands with Vanguard. The developed-base composition is clearly labeled
as a diagnostic rather than gameplay evidence. Building roles have distinct visible forms;
the terrain and interface palette is coherent. This does not constitute user artistic approval.
All original generated asset paths have provenance in `assets/LICENSES.md`; the generators
and byte-for-byte reproducibility checks support the recorded original origin.

No existing test file changed, no test was removed or ignored, and no assertion was weakened.
Test attributes increased from 414 to 417: pixel-palette preservation, executable menu bundle
spawning and valid isolated developed-base setup. The latter checks the opponent faction was
not extended. Those additions are useful, but the missing all-facing ownership regression is
part of P1-1.

The package change uses the configured target directory consistently for the release executable
and package outputs, including relative environment paths. It does not alter signing or release
policy. Existing asset/font notices still travel with the package.

## Inherited debt and user verification

Carry the M7 review's six P2 items and earlier debt as already recorded in STATUS: settings
temporary-file handling, bundle signature/distribution guidance, suite time documentation,
benchmark movement/window evidence, normal smoke map-0-only coverage and roster-limit boundary
coverage. M8's native captures do not resolve the ordinary smoke flow's map coverage. Apple
Silicon-only output and generic names are now accepted scope, not defects. Audio and full release
completion remain deferred. No inherited problem was silently treated as fixed by this review.

The user still needs to verify the art direction, building/unit readability, terrain, menu/sidebar
comfort, controls at their preferred window/zoom sizes, and whether the presentation approaches
their requested original classic-RTS style. After P1-1 is remediated, perceived blue/red clarity
in combat is also a user acceptance item. No `m8-approved` tag is authorized by this report.

## Independent validation results

Every mandated command was rerun from the repository root against `2207a2b` in sequence. The
configured `CARGO_TARGET_DIR=/Users/xingyuwang/Library/Caches/Codex/cargo-target` was preserved.
Build/cache and native Metal/display commands used the required elevated execution. Runtime
commands used `RUST_LOG=info` to retain affirmative smoke evidence. No simulation, code, data or
asset file was altered between gates. Review logs use the command names below in
`/Users/xingyuwang/Library/Caches/Codex/reviews/MyGame001/m8-review-1/`.

| Command | Exit | Independent result |
|---------|-----:|--------------------|
| `cargo fmt --all -- --check` | 0 | Clean, 0.24 s (`fmt.log`). |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | Clean, 0.25 s (`clippy.log`). |
| `cargo test --workspace` | 0 | **417 passed, 0 failed, 0 ignored**, 185.67 s (`workspace-tests.log`). |
| `cargo build --release -p game` | 0 | Apple Silicon release build, 0.17 s (`release-build.log`). |
| `cargo run --release -p game -- --smoke-test 600` | 0 | 600 frames, 169 ticks, 3 matches; no view mismatch or out-of-window sidebar button, 11 menu buttons, 11 options buttons, settings saved, zero rejected AI commands; 11.46 s (`release-smoke.log`). |
| `cargo run -p game -- --smoke-test 600` | 0 | Same smoke checks, 600 frames, 181 ticks, 14.14 s (`debug-smoke.log`). |
| `cargo run --release -p sim --example m7_benchmark` | 0 | Fixture mean/busy/max **0.075/0.239/0.570 ms**; shipped **0.158/0.364/0.855 ms**. Peak heap 0.6 MiB, within 5 ms mean / 50 ms max budgets (`benchmark.log`). |
| `cargo run --release -p sim --example m7_soak` | 0 | **3,168,000 ticks**, 44 configurations twice, 149.5 s simulation run, peak heap 1.6 MiB; all checks pass (`soak.log`). |
| `sh scripts/package_macos.sh` | 0 | Package smoke600 passed: 168 ticks, 3 matches, all UI/view/options checks; app and zip generated under configured target/package, 13.34 s (`package.log`). |
| `MYGAME_VISUAL_REVIEW=<review-cache>/captures cargo run -p game` | 0 | Independently generated all five native 2560×1440 captures, 14.32 s; all are byte-identical to the inspected committed captures (`native-review.log`). |

The smoke runs check real rendering, movement, production charging, minimap, pause, roster,
menu and options paths. They report `enemy views shown: 0`; they therefore do not contradict
P1-1. The all-facing reviewer probe deliberately fails (exit 101) for that defect, while every
mandated pre-existing gate passes. The known M0 linker `__eh_frame` warning and winit
`Skipped event Destroyed` shutdown warning recur; no new runtime warning was found.

The test breakdown is game unit 125, game integration 6/3/6/5/3/1/5, sim unit 11, and sim
integration 12/32/24/14/37/11/21/10/32/12/2/11/14/13/7. The 108-production-match integration
takes 120.10 s and the fixture AI suite 39.30 s. Tests and soak continue to support the unchanged
deterministic simulation and occupancy/economy invariants. The packaged executable is verified
as Mach-O arm64. Signing is inherited debt and was not claimed fixed.

## Storage and repository state

The only new substantial external directory created for this review is
`/Users/xingyuwang/Library/Caches/Codex/reviews/MyGame001/m8-review-1` (about 4.2 MiB), holding
validation logs, the deliberately failing reproduction executable/source and five native captures.
Retain it through remediation and re-review; it is useful reproducibility evidence. Build and
package outputs remain in the pre-existing configured Cargo target cache; `package/` is about
145 MiB and remains useful for packaging validation. No clone, worktree, virtual environment or
substantial `/tmp` artifact was created. The package script's empty transient working directory
and smoke settings are short-lived temporary files. No Git repository or worktree was deleted.
`docs/HANDOFF.md` remains untracked user material. No merge, tag or push was performed.

**Final decision: NO-GO for P1-1.** Complete that focused remediation and independently re-review
the complete cumulative M8 diff. The outstanding user-only visual acceptance does not change
this decision in either direction.
