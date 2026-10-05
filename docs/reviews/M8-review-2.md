# M8 review 2: visual polish and ownership-paint remediation

Independent review under PLAN.md sections 10, 12 and 13, dated 2026-10-05. This reviewer did
not implement or remediate M8. Only this report is changed by the reviewer.

**Decision: GO WITH DOCUMENTED NON-BLOCKING DEBT.** Review 1 P1-1 is closed; no new P0, P1
or P2 finding remains. All mandatory gates passed independently. Carry the existing P2 debt in
STATUS; final visual quality remains requires user verification.

## Scope

Reviewed the complete cumulative `m7-reviewed` (`eb167f6`) to `8e13fe4` diff on `milestone/m8`:
50 files, 1,747 insertions and 249 deletions, including the original implementation, scope
amendment, prior review record and remediation. This is not a review of the remediation alone.
Read PLAN.md in full, the M8 STATUS sections, U3, D58–D61, the binding M8 review 1 and the
inherited M7 debt. Inspected every changed production path, generator, test, provenance entry
and packaging change. The user's untracked `docs/HANDOFF.md` was read but remains untouched.

The authorized milestone is visual polish first, targeting Apple Silicon macOS, using original
art. Audio replacement, the earlier gameplay and release backlog, signing/notarisation and
version 1.0 completion are outside this milestone. Generic names and Apple Silicon-only support
are accepted user decisions; collecting Rust-crate notices is not a requested M8 deliverable.
The existing asset and font notices are retained. Artistic acceptance is **requires user
verification** and is not a NO-GO criterion under PLAN.md section 12.

## Review 1 P1-1: closed

The former five zero-paint NW frames and eight four-pixel W/N infantry frames now have
substantive ownership-colored surfaces. Infantry upper tunics and shoulders wrap around the
body; the backpack has a darker identification panel. The rocket launcher has a band on its
visible rack, where its equipment previously hid the hull paint. The production palette function
and white sprite tint have not changed during remediation.

Independent evidence, retained under the review cache described below:

1. `production-paint-probe.rs` includes the **actual current production**
   `crates/game/src/presentation.rs`, links the pinned existing Bevy dependency, and invokes
   `sprite_pair` on each input sheet. It checks every pixel's alpha, limits transformed pixels
   to the two independently enumerated authored unit-paint colors, and requires at least ten
   opaque changed pixels in each of the 88 frames. The standalone build exits 0.
2. Old PNGs extracted read-only from `1a67c58` into `prior-units/` fail that probe with exit
   **101** (`probe-red.log`), reproducing exactly the same thirteen deficient frames. The
   current committed PNGs pass with exit **0** (`probe-green.log`). No repository source or
   asset was swapped to produce the red result. The implementer's recorded regression-red
   log was also inspected and agrees.
3. The committed `m8_team_paint` integration regression checks the PNG subset first, runs the
   real `sprite_pair`, compares committed bytes/pixels with the generator, and enforces all
   88 coverage minima and exact neutral-material/alpha preservation. The explicit authored
   palette is independent of the production recoloring predicate, so whole-sprite tinting
   cannot satisfy the test. Its freshly rerun output agrees with the independent probe.
4. Inspected and independently reran `verify-paint-only.py` against every old/new RGBA pixel.
   Exit 0 (`paint-only.log`): six sheets are entirely unchanged; rifleman and rocket soldier
   each change 404 surface pixels, conscript and grenadier each 490, and rocket launcher 236.
   Every change is from the intended uniform/backpack/rack material into paint; alpha and
   front-marker masks are unchanged. Thus silhouette, shadows, frame extents and anchors are
   preserved by the remediation. The original facing and geometry assertions are untouched.
5. Opened `docs/screenshots/m8-team-paint-1x.png` at its native 1024 × 704 resolution: all
   eleven rows and eight friendly/enemy pairs were inspected, especially NW/W/N infantry and
   rocket-launcher views. The repair is visible as panels on existing surfaces, not isolated
   marker pixels. Neutral tracks, helmets, skin, weapons and equipment remain recognizable.
   The ten-pixel minimum is the existing scout minimum; repaired infantry have at least 54
   changed pixels and the repaired rocket launcher at least 24. This establishes closure of
   the objective missing-owner-information defect, while combat readability preferences and
   artistic quality remain for the user.

Opaque changed-pixel counts, production transform, E/SE/S/SW/W/NW/N/NE order:

| Unit | E | SE | S | SW | W | NW | N | NE |
|------|--:|---:|--:|---:|--:|---:|--:|---:|
| light tank | 39 | 54 | 39 | 39 | 29 | 50 | 29 | 39 |
| scout | 23 | 16 | 23 | 10 | 25 | 18 | 25 | 10 |
| harvester | 70 | 66 | 70 | 72 | 54 | 60 | 54 | 72 |
| rifleman | 76 | 72 | 77 | 54 | 69 | 64 | 69 | 54 |
| conscript | 85 | 72 | 86 | 71 | 82 | 72 | 82 | 71 |
| rocket soldier | 75 | 72 | 77 | 54 | 69 | 64 | 69 | 54 |
| grenadier | 84 | 72 | 86 | 71 | 82 | 72 | 82 | 71 |
| raider | 25 | 40 | 27 | 41 | 22 | 40 | 24 | 41 |
| heavy tank | 61 | 89 | 62 | 61 | 47 | 81 | 48 | 61 |
| artillery | 29 | 55 | 29 | 48 | 26 | 43 | 26 | 48 |
| rocket launcher | 68 | 54 | 66 | 88 | 47 | 24 | 45 | 88 |

## Cumulative production-path review

### Checked loading and runtime rendering

`skirmish::load` still validates definitions, referenced sprites, the AI/faction catalogue and
every map before installing the catalogue. Unit/building PNGs pass `check_png_file` at their
expected dimensions. That bounded reader checks the explicit RGB/RGBA8, noninterlaced PNG
subset and ancillary-chunk allowlist, then decodes under its existing limits. The exact checked
bytes are given to `sprite_pair`; files are not reopened for a later decode. A decoder failure
logs the checked asset path and requests error exit. No fallback texture is introduced.

Inspected the local pinned `bevy_image-0.19.1` `Image::from_buffer` and `from_dynamic` sources:
the accepted RGB8/RGBA8 cases produce populated RGBA8 CPU bytes and `Rgba8UnormSrgb` textures.
Therefore the four-byte pixel traversal and populated-data assumption are justified for this
path. Team arithmetic is bounded and leaves alpha unchanged. The pair of image maps is filled
together before successful installation. Both unit and building views select the correct map
from `Owner` and retain white tint; existing unseen-enemy visibility filtering is unchanged.
Menu icons and sidebar cards use those same checked image handles. I8 and I11 are preserved.

### Geometry, simulation and numerical contracts

All eight original industrial building shapes, eleven eight-facing unit sheets and three
shipped tilesets are generated by repository code, with provenance in `assets/LICENSES.md`.
Shared raster primitives draw fixed authored geometry and clip writes to canvas bounds.
Building footprint/image conventions, unit 64 × 64 frame size and ground anchors remain fixed;
the unchanged reproducibility, PNG subset, footprint-corner and facing-centroid tests verify
the resulting contracts. Terrain drawing changes neither map cell flags nor projection.

`git diff m7-reviewed..8e13fe4 -- crates/sim assets/data Cargo.toml Cargo.lock rust-toolchain.toml`
is empty. No dependency/toolchain or data-schema upgrade occurred. Section 5 equations,
integer simulation arithmetic, deterministic scheduling/hashing, occupancy and economy rules
are unchanged (I1–I4, I6–I7, I9–I10, I12). Views continue to read `SimHost` through shared
access; interface/AI gameplay changes still use the command boundary. The opt-in visual aid
installs its developed base through existing pre-tick match setup using a cloned catalogue,
then restores the production catalogue. It does not mutate a running match via a new bypass
(I5), and the diagnostic is disabled during smoke tests. Its test verifies valid placement and
that the opponent's faction was not extended.

### Interface, evidence and test adequacy

The sidebar retains its 240-pixel input boundary, command effects, keyboard controls, roster
filtering, costs, queue/progress and readiness handling. Presentation changes add two-column
sprite cards and colors without changing production behavior. The menu keeps its selections
and effects; DEPLOY is the Start label. Its new executable ECS regression catches duplicate
bundle components. Existing tests were neither removed, ignored nor weakened; the sole changed
existing test-module content is an added menu-spawn test. The new integration target also
executes the imported production palette unit test, so 419 executions represent four distinct
new tests beyond M7's 414, with the palette test executed in two targets.

Opened all five committed native captures: menu, normal demo start, clearly labeled developed
base, Crossroads/Meridian and Riverlands/Vanguard. The default 1280 × 720 logical layout shows
the full control set, readable labels, status badges and no concrete clipping/overlap failure.
Building roles are differentiated by crane, stacks, tanks, barracks, vehicle bay, radar and
defense shapes. User-selected small windows, preferred zoom, interaction comfort and artistic
judgment remain unverified user acceptance. These screenshots are presentation evidence, not
proof of a completed human-played match or opposing-army battle.

The package script consistently resolves the configured target directory for binary and package
outputs, including relative environment paths. This is a bounded compatibility change needed
for the user's cache policy. Existing asset/font notices still ship. Signing and distribution
claims remain inherited debt; no release-hardening claim is made here.

## Independent validation

All required commands ran from the canonical repository against `8e13fe4`, sequentially,
preserving `CARGO_TARGET_DIR=/Users/xingyuwang/Library/Caches/Codex/cargo-target`. Cache writes
and native Metal/display access used elevated execution. `RUST_LOG=info` retained affirmative
runtime checks. No source or game asset changed during the gates. Logs and `results.json` are
in `/Users/xingyuwang/Library/Caches/Codex/reviews/MyGame001/m8-review-2/`; each log uses the
basename listed below.

| Command | Exit | Independent result |
|---------|-----:|--------------------|
| `cargo fmt --all -- --check` | 0 | Clean, 0.25 s; `fmt.log`. |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | Clean, 0.27 s; `clippy.log`. |
| `cargo test --workspace` | 0 | **419 passed, 0 failed, 0 ignored**, 186.59 s; `workspace-tests.log`. |
| `cargo build --release -p game` | 0 | Apple Silicon release binary, 0.18 s; `release-build.log`. |
| `cargo run --release -p game -- --smoke-test 600` | 0 | 600 frames, 169 ticks, 3 matches; all smoke checks pass, 11.48 s; `release-smoke.log`. |
| `cargo run -p game -- --smoke-test 600` | 0 | 600 frames, 181 ticks, 3 matches; all smoke checks pass, 14.24 s; `debug-smoke.log`. |
| `cargo run --release -p sim --example m7_benchmark` | 0 | Fixture mean/busy/max **0.074/0.231/0.535 ms**; shipped **0.158/0.364/0.847 ms**, peak heap 0.6 MiB; 0.78 s; `benchmark.log`. |
| `cargo run --release -p sim --example m7_soak` | 0 | **3,168,000 ticks**, 44 configurations twice, 151.4 s simulation (151.75 s command), peak heap 1.6 MiB, all checks pass; `soak.log`. |
| `sh scripts/package_macos.sh` | 0 | Packaged smoke600 passes, 168 ticks, 3 matches; 13.49 s; `package.log`. App and zip are in the configured target's `package/`. |
| `MYGAME_VISUAL_REVIEW=<review-cache>/captures cargo run -p game` | 0 | All five native 2560 × 1440 captures, 14.19 s; `native-review.log`. All five are byte-identical to the inspected committed captures. |
| `cargo run -p game --example team_paint_review -- <review-cache>/team-paint-1x.png` | 0 | 1.41 s; `contact-sheet.log`. Fresh production-palette contact sheet is byte-identical to the committed 1:1 sheet and was opened again. |
| `cargo test -p game --test m8_team_paint -- --nocapture` | 0 | 2 passed, 0 failed/ignored, 0.50 s; `team-regression.log`. All 88 counts match the independent probe. |

`<review-cache>` in the two diagnostic commands is the absolute directory named above. The
workspace breakdown is game unit 125; game integration 6/3/6/5/3/1/5/2; sim unit 11; sim
integration 12/32/24/14/37/11/21/10/32/12/2/11/14/13/7; doc-tests 0. There are no skipped tests.

Both ordinary smoke runs and the packaged run report zero view mismatches, zero sidebar
buttons outside the window, 19 of 19 sidebar items, 11 menu buttons, 11 options buttons,
settings saved, no out-of-roster items and zero rejected AI commands. Movement, production,
minimap, pause and match restart paths execute successfully. They also report **zero enemy
views shown**; ownership-fix evidence comes from the production-transform regression and
all-facing contact sheet, not from these smoke counts. The benchmark meets the recorded 5 ms
mean/busy and 50 ms maximum budgets. The soak verifies deterministic replay, occupancy,
credits, rejected-command conditions and bounded state throughout its configurations.

`file` confirms the packaged executable is Mach-O arm64. Known M0 `__eh_frame` linker and
winit `Skipped event Destroyed` shutdown warnings recur; no new runtime warning or error was
found. `git diff --check m7-reviewed..8e13fe4` passes.

| M8 acceptance criterion | Review result |
|-------------------------|---------------|
| All shipped factions/maps use the new presentation | Supported by regenerated 11 unit sheets, 8 building sprites, 3 tilesets, unchanged reproducibility checks and native captures spanning all three maps and both factions. |
| Building roles and unit facings remain legible | Objective geometry/facing and ownership contracts pass; distinct building forms inspected. Artistic/combat readability **requires user verification**. |
| Sidebar and menu remain usable | Production behavior preserved, menu regression and real smokes pass; default-window native captures inspected. Preferred sizes/zoom and interaction comfort **require user verification**. |
| Real game and smoke tests run with new assets | Release, debug, packaged smoke600 and native capture mode pass independently. |
| Scope, original provenance, checked loading and invariants | Reviewed above; no simulation/schema/dependency change, no third-party game media, checked-byte decode retained. |

## Inherited debt and user verification

No new P0, P1 or P2 finding was established. Review 1 P1-1 is closed. Carry the already recorded
M7 P2 items: settings temporary-file symlink/FIFO behavior; bundle signing and other-Mac guidance;
test-suite-time documentation; benchmark movement/window evidence; normal smoke map-0-only
coverage; and roster-limit boundary coverage. Earlier recorded gameplay, input, fog, error-context
and documentation debt remains as listed in STATUS. Apple Silicon-only output is accepted scope.
Native review captures do not repair normal smoke map coverage, and palette/contact-sheet
evidence does not claim that ordinary smoke runs exercise an enemy army.

The user must still verify the art direction and original classic-RTS style, building and unit
readability, blue/red identification during actual combat, terrain presentation, menu/sidebar
comfort and controls at preferred window/zoom sizes. Earlier human-playability acceptance and
final audio acceptance remain pending. These user-only items do not prevent `m8-reviewed`, but
only the user can create `m8-approved`. Audio replacement and broader release work remain later
work, and this report does not declare version 1.0 complete.

## Storage and final decision

The only new substantial external directory created by this reviewer is
`/Users/xingyuwang/Library/Caches/Codex/reviews/MyGame001/m8-review-2` (about **124 MiB**).
It holds command logs/results, five fresh native captures, the contact sheet, prior committed
PNG inputs, the integrity comparison source, and the standalone production probe's source and
executable (most of the size). Retain it for review reproducibility; it remains useful through
the milestone merge and user verification. Build/package outputs reuse the pre-existing
configured Cargo target cache; refreshed `package/` is about **145 MiB** and remains useful for
packaging validation. No clone, Git worktree, virtual environment, repository `target/` or
substantial `/tmp` artifact was created. Smoke settings and the package script's transient
empty working directory use the existing short-lived temporary-file paths. No Git repository
or worktree was deleted.

`docs/HANDOFF.md` remains untracked user material. No implementation, game asset, STATUS or
decision-log edit is part of this review. No merge, tag or push was performed by the reviewer.

**Final decision: GO WITH DOCUMENTED NON-BLOCKING DEBT.** The orchestrator may continue the
PLAN.md section 12 merge/`m8-reviewed` workflow while carrying existing P2 debt. This is not
user artistic approval, does not authorize `m8-approved`, and does not complete version 1.0.
