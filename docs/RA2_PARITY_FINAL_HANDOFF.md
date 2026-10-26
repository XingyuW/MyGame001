# RA2 parity programme — final handoff

| Handoff field | Value |
|---|---|
| Date | 2026-10-25 |
| Repository | `/Users/xingyuwang/GameProjects/MyGame001` |
| Current branch | `main` |
| Binding review decision | **GO WITH DOCUMENTED NON-BLOCKING DEBT** |
| Findings | **0 P0, 0 P1, 3 P2** |
| User verification | **51 checks pending, 0 claimed passed** |

## 1. Final repository state

The RA2 parity programme is implemented through RA9, fully validated on frozen source, reviewed by
five independent slice reviewers and a fresh independent lead, and merged locally into `main`.

| Item | Exact state |
|---|---|
| Reviewed implementation tip | `f6fdac5a5add10e62285141a2a4ccb3328234e2f` |
| Review closeout commit on `ra2-parity` | `17f89be3680b36420140d8050a890fb4df1ea4f5` |
| Local merge commit on `main` | `af873b927558f42231461d3518d8c4cb281d71fb` |
| Merge parents | `8230a955b251e9341d349f99eaeb86f7f8253664`, `17f89be3680b36420140d8050a890fb4df1ea4f5` |
| Annotated reviewed tag | `ra2-parity-reviewed`, pointing to `af873b927558f42231461d3518d8c4cb281d71fb` |
| Remote publication | Not pushed by this work |
| User approval tag | Not created; only the user may approve the 51 hands-on checks |

The reviewed tag intentionally identifies the independently reviewed merge. This handoff document
is a later documentation-only addition and should not be used to rewrite or move that tag.

## 2. What was delivered

The programme completed the planned RA1–RA9 scope, with RA0 requirements and source research kept
as the governing foundation:

- **RA1:** Allied/Soviet sides, nine countries, match setup, starting units and occupancy rules.
- **RA2:** Paid construction, production queues, buildability, power, placement, refunds and base
  building.
- **RA3:** Armour, warheads, weapons, targeting, damage and deterministic combat behavior.
- **RA4:** Veterancy, crews, aircraft bays and sorties, launched actors, naval and submerged units.
- **RA5:** Ore and gems, harvest/refinery behavior, crates, tech buildings, garrisons, bridges and
  world objects.
- **RA6:** Specialists, deploy modes, spies, mind control, temporal weapons, C4/Ivan behavior,
  parasites, transports, radiation, death chains and major abilities.
- **RA7:** Fog and radar, observer privacy, selection and contextual controls, minimap, notices,
  presentation and interface behavior.
- **RA8:** Country-aware AI economy, production, recovery, composition, combat and difficulty.
- **RA9:** Complete data/source parity audit, generated-data and reference checks, deterministic
  balance/performance workloads, packaging, documentation, frozen-source validation and review.

The final loaded catalogue is 54 units, 46 buildings, 116 weapons, 61 warheads, 11 armours,
9 countries, 21 maps, 11 raw AI plans and 3 difficulties.

## 3. Decisions and boundaries that must remain explicit

- **R4:** map growth is optional and default-off.
- **R6 / I6-prime:** a corresponding cell supports one vehicle or ship, up to three infantry
  spots, and one separate aircraft occupant; buildings exclude surface occupancy.
- **R7:** speed retains the documented conversion from 15-frame source movement to 20 simulation
  ticks, including the documented infantry and Jumpjet assumptions.
- **R29:** original base and map-scaled match limits remain unchanged. The 300,000 duel cap,
  400,000 all-to-end cap, 24,000 fixed-window scope and 36,000-tick soak scope are distinct.
- The fixed-duration soak is a determinism and resource check. Its sampled final worlds may still
  be Running; it is not all-to-Won evidence.
- Passing simulation-time budgets does not approve rendered smoothness or subjective pacing.
- Static source classification, hashes and absence records do not prove original-engine execution.
- Local arm64 macOS packaging does not prove signing, notarization, universal binaries or launch
  on another machine.

The final decisions and correction history are recorded in
[`docs/DECISIONS.md`](DECISIONS.md), especially R193–R210.

## 4. Final frozen-source validation

The final gate launched from source commit
`525636dac32eb8bae4919a6dccdde1c0dbe815eb`. The exact C9 manifest contains 689 inputs and 64
protected authority spans:

- Manifest: [`C-corrected-09/frozen-validation-inputs.json`](validation/ra2-parity/remediation-2/C-corrected-09/frozen-validation-inputs.json)
- Manifest SHA-256: `17d992857c80567e6d8377a8b98c4a1b254454ce13ff9e58104b811c52d5d64d`
- Combined summary: [`review-2-corrected-08/final-validation-summary.json`](validation/ra2-parity/review-2-corrected-08/final-validation-summary.json)
- Summary SHA-256: `83030b60fa3530169c9868eb98adea664c61bf583c6aa05df0572ca7aaabbb41`

All 13 required commands exited 0:

1. `cargo fmt --all -- --check`
2. `git diff --check`
3. workspace/all-target Clippy with warnings denied
4. release workspace tests
5. release game build
6. release 600-frame smoke
7. debug 600-frame smoke
8. release benchmark
9. deterministic soak
10. complete duels
11. macOS package and packaged smoke
12. generated-data check
13. reference-table check

Important results:

| Gate | Result |
|---|---|
| Workspace tests | 1,835 primary unfiltered passes; 0 failures, ignores or filters. Selected child probes make 1,844 raw passes and are accounted separately. |
| Complete RA9 audit | 185 passed, 0 failed, 0 ignored, 0 filtered |
| Complete AI target | 146 passed, 0 failed, 0 ignored, 0 filtered |
| Production matrix | 756/756 Won; median 29,448 ticks; p90 70,403; max 286,896 |
| Duel matrix | 756/756 Won; median 29,056 ticks; p90 73,899; max 263,681 |
| Isolated all-map end checks | 21/21 Won; median 86,306; p90 108,805; max 245,510 |
| Fixed windows and refusal guards | All three fixed windows and all three guards passed |
| Soak | 260 configurations × 2 worlds × 36,000 ticks = 18,720,000 deterministic ticks; peak heap 247.2 MiB under 1,024 MiB |
| Benchmarks | Both 200-unit workloads passed movement, combat, mean, busy-window, maximum and heap guards |
| Smoke/package | Release and debug 600-frame smokes passed; package and packaged smoke passed |
| Asset failure/restoration | Release and packaged missing-image runs exited 1 with the path; restored runs exited 0 after 600 frames |

The first asset-acceptance v2 receipt is intentionally retained. It exposed a harness predicate
error because successful release binaries do not print the expected positive text. The corrected
v3 predicate uses process success, restored image presence and absence of the failure marker; all
four corrected cases pass. No game source or canonical asset was changed by that correction.

Historical failures and interrupted attempts remain preserved and receive no pass credit. This
includes the original round-2 non-Won, erased-actor command failures, C7/C8 failures, compiler and
fixture failures, intended causal negatives, and the asset v2 harness failure.

## 5. Audit and data accounting

The final audit records:

- 16,211 physical facts = 13,419 `Compared` + 2,792 `GameOwned`.
- 7,108 demanded and successful dependency pairs.
- 5,022 source atoms = 3,111 compared atoms + 1,911 typed dispositions.
- The 1,911 dispositions are 759 `CURRENT_MODEL_NONREAD` and 1,152
  `INTENTIONAL_DIFFERENCE`, with zero `ORIGINAL_ENGINE_UNUSED` claims.
- 1,869 dispositions cover whole values and 42 cover named components.
- 75 numeric differences remain individually documented.
- All original cached INI hashes, assertions and 51 HOWTOSEE procedures remain preserved.

The stable historical debt census contains exactly 183 unique IDs. The independent lead's
companion table preserves every ID and byte-exact historical alias in canonical order and supplies
an individual disposition, severity, evidence and remaining obligation:

[`docs/reviews/RA2-parity-review-2-final-debt-dispositions.tsv`](reviews/RA2-parity-review-2-final-debt-dispositions.tsv)

## 6. Independent review

The user authorized five independent slice reviewers in two batches, followed by a fresh lead.
All five final slices inspected the complete cumulative programme and final raw evidence. Their
reports and the lead report are preserved verbatim.

- Binding lead report: [`docs/reviews/RA2-parity-review-2.md`](reviews/RA2-parity-review-2.md)
- Final review overview: [`docs/validation/ra2-parity/review-2-final/README.md`](validation/ra2-parity/review-2-final/README.md)
- Evidence and hashes: [`docs/validation/ra2-parity/review-2-final/evidence-index.json`](validation/ra2-parity/review-2-final/evidence-index.json)
- Slice reports: [`sim1`](reviews/RA2-parity-review-2-final-sim1.md),
  [`sim2`](reviews/RA2-parity-review-2-final-sim2.md),
  [`client`](reviews/RA2-parity-review-2-final-client.md),
  [`data`](reviews/RA2-parity-review-2-final-data.md), and
  [`tests`](reviews/RA2-parity-review-2-final-tests.md).

The binding lead decision is **GO WITH DOCUMENTED NON-BLOCKING DEBT**, with 0 P0, 0 P1 and
3 unique P2 findings. The lead independently verified all 689 frozen inputs, all 64 authority
spans, all final summary path/hash records, the validation results, report hashes and 183 debt
rows. A nonbinding isolated-summary SHA transcription in the TESTS report is corrected by the
lead report; the actual file hash is
`718eccd049ec89f2e97bb0aa2ea5311cf456aba6e05b8e18a437caa93aeda0f0`.

## 7. Remaining non-blocking P2 debt

### 7.1 Exact lost-Hornet regeneration boundary

`flight.rs` replaces a missing launched Hornet with `Slot::Lost(spawn_regen.max(1))` and counts
down to Ready. Current integration tests prove real loss, regeneration and the distinction from
ordinary returned-aircraft reload. They do not assert Lost immediately before and Ready exactly
at the shipped 800-tick boundary, or the fixture's 400-tick boundary.

Suggested follow-up: add exact edge assertions without changing data, timers or broad tests.
`R56-43` and `R56-63` are aliases of this single `SPAWN-01` finding.

### 7.2 Same-frame notice cap with deferred despawn

`crates/game/src/notices.rs` starts from the current Query count, while additions and despawns are
deferred. Multiple qualifying events in one frame can choose the same oldest entity repeatedly and
may leave more than `MAX_NOTICES` after command application.

Suggested follow-up: enforce the cap over current plus pending notices and select distinct removals.
No simulation, authority, privacy or victory effect was demonstrated.

### 7.3 PNG metadata-before-open pathname/FIFO race

`crates/game/src/tileset_image.rs::read_bounded` validates pathname metadata and later opens that
pathname. Replacement with a FIFO or special file between those operations could block the open or
read.

Suggested follow-up: open first, validate the opened handle, and preserve typed errors plus current
regular-file and symlink behavior. Existing PNG size, structure, dimensions, decode, missing-file,
non-file and oversized-file controls remain valid.

## 8. Work intentionally left to the user

All 51 procedures under “How to perform user verification” in
[`docs/RA2_GAP_ANALYSIS.md`](RA2_GAP_ANALYSIS.md) remain **PENDING — requires user verification**.
They cover:

- display, scaling, camera, selection, groups and readability;
- lobby defaults, production, placement, economy and world interactions;
- weapons, units, specialists, superweapons, visibility, controls and AI feel;
- music, effects, announcements, volume persistence, pause and pacing;
- rendered frame/tick tails;
- a normal human victory and defeat;
- extension isolation, packaged launch, distribution/signing and overall art/audio acceptance.

Do not convert automated results into a pass for these checks. A separate user approval tag may be
created only after the user performs and accepts them.

## 9. Recommended next actions

1. Perform the 51 user checks and record each outcome against its exact HOWTOSEE procedure.
2. Address the three P2 findings with focused tests if desired; they do not block the reviewed
   merge.
3. If distributing the game, complete signing, notarization, universal-build and another-machine
   launch checks.
4. Push `main`, `ra2-parity` and `ra2-parity-reviewed` only when the repository owner intends to
   publish them. No remote push occurred in this work.
5. Preserve the frozen manifests, validation logs, failed attempts, slice reports, lead report and
   debt dispositions. They are the audit trail for the final decision.

## 10. Preserved external review material

The following substantial directory was used outside the canonical repository in compliance with
the local storage policy:

`/Users/xingyuwang/Library/Caches/Codex/reviews/MyGame001/ra2-final-review-20261023`

It is approximately 1.1 MiB and remains necessary as the original source for the independent
reports, dispatch packets, lead report, debt dispositions and review utilities. Portable copies of
the binding artifacts are committed under `docs/validation/ra2-parity/review-2-final/`. No Git
clone or worktree was created for this review, and no substantial artifact was created under
`/tmp` or `/private/tmp`.
