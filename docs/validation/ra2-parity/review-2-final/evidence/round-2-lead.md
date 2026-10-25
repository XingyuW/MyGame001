# RA2 parity programme — binding independent final review, round 2

**Decision: GO WITH DOCUMENTED NON-BLOCKING DEBT.**

Unique unresolved findings: **P0 0; P1 0; P2 3**. The P2 findings are the exact lost-Hornet regeneration-boundary test gap (`SPAWN-01`), same-frame notice-cap/deferred-despawn behavior, and the PNG metadata-before-open pathname replacement/FIFO race. All **51 user-only checks remain PENDING** and are outside the P0/P1/P2 count under PLAN section 12. No user-visible quality, approval tag, signing, notarization, universal build, or other-Mac launch is approved by this decision.

## Binding identity, independence, and repository state

- Repository: `/Users/xingyuwang/GameProjects/MyGame001`
- Cumulative reviewed range: `8230a955b251e9341d349f99eaeb86f7f8253664..f6fdac5a5add10e62285141a2a4ccb3328234e2f`
- Exact final tip: `f6fdac5a5add10e62285141a2a4ccb3328234e2f`
- Gate-launch source commit: `525636dac32eb8bae4919a6dccdde1c0dbe815eb`
- Frozen manifest: `docs/validation/ra2-parity/remediation-2/C-corrected-09/frozen-validation-inputs.json`, **148657 bytes**, SHA-256 `17d992857c80567e6d8377a8b98c4a1b254454ce13ff9e58104b811c52d5d64d`
- Final summary: `docs/validation/ra2-parity/review-2-corrected-08/final-validation-summary.json`, **379468 bytes**, SHA-256 `83030b60fa3530169c9868eb98adea664c61bf583c6aa05df0572ca7aaabbb41`
- Historical debt source: `docs/RA2_HISTORICAL_DEBT.tsv`, **182826 bytes**, SHA-256 `d0c93c3a6cb828449a0fb859265e40b0987eb85de003688c279c1668987eecbe`
- Companion disposition table: `/Users/xingyuwang/Library/Caches/Codex/reviews/MyGame001/ra2-final-review-20261023/round-2-final-debt-dispositions.tsv`, **166835 bytes**, SHA-256 `7eaea475dc284a57862b7dd97c1d2bb41c71ae05986e41ecba1583ed50b423c0`

I implemented, remediated, prepared, or slice-reviewed none of this programme. I launched no Cargo, game, package, benchmark, soak, or other native process. I made no repository, index, ref, source, test, canonical-evidence, or metadata edit. `git rev-parse HEAD` returned the exact tip above and `git status --short` was empty before writing these external review artifacts. No clone, worktree, validation copy, build output, cache, dataset, or substantial temporary directory was created.

## Review method and exact coverage

I read the corrected lead dispatch, the complete current lead brief, the original prompt's final-review rules, continuation prompt section 6, PLAN sections 10 and 12, current review requirements, R172/R194/R199 and R200–R209, the previous binding round-1 NO-GO, all five initial/revised cumulative slice histories, all correction reports named by the dispatch, and all five final-evidence reports. I verified every one of the **25 dispatch-listed report hashes**. I treated every conclusion as a claim and reconciled it with current source, the frozen manifest, raw results, and the 183-row ledger.

The final manifest contains **689 paths (674 source plus 15 derived)** and **64 protected authority spans**. I independently recomputed every path's byte length and SHA-256 and reconstructed every authority byte span at its recorded offset: **689/689 and 64/64 matched, with zero mismatch**. I also recursively checked every path/size/SHA object embedded in the final summary: **15/15 matched**. The HOWTOSEE payload is **51 rows, 13855 bytes**, SHA-256 `8ae4edfadf97fc68138d03c9a12d9c1217cf0c1fb65cf6dd6010c77b1c5bd6c1`.

The five final reports cover the full current ownership graph: 65 simulation Rust files, 40 game Rust files and their cross-slice interfaces; all audit modules/tables, source-policy routes, generators and original-input bindings; every native/client/fixture purpose; asset and package paths; and current gameplay/client behavior. The later final-tip commits preserve functional gate source and add documentation/evidence. I checked the three reported P2 source bodies directly. I found no uncovered mandatory acceptance criterion after the combined source and raw-evidence reconciliation.

One report-level transcription error was independently corrected: `round-2-final-evidence-tests.md` prints an isolated-summary SHA beginning `718ecc73`; the actual 17839-byte file, final summary, SIM1, SIM2, and DATA evidence agree on SHA-256 `718eccd049ec89f2e97bb0aa2ea5311cf456aba6e05b8e18a437caa93aeda0f0`. This does not change any executable result or finding.

## Mandatory validation and raw evidence

The validator faithfully runs the original eleven commands plus generated-data and reference checks, records each exit, continues after ordinary failures, and returns failure if any selected command fails. The single R194-authorized final run contains all thirteen unchanged commands; every command exited 0 and the terminal summary says PASSED.

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `git diff --check` | exit 0 |
| workspace/all-target Clippy with warnings denied | exit 0 |
| release workspace tests | exit 0; 1835 primary unfiltered passed, 0 failed, 0 ignored, 0 filtered; nine selected child probes separately account for 1844 raw passes and 1348 filtered |
| release game build | exit 0 |
| release 600-frame smoke | exit 0 |
| debug 600-frame smoke | exit 0 |
| release benchmark | exit 0 |
| deterministic soak | exit 0 |
| complete duels | exit 0 |
| macOS package and packaged smoke | exit 0 |
| generated data check | exit 0 |
| reference table check | exit 0 |

The complete RA9 audit target passed **185/0/0/0** together and the complete AI target passed **146/0/0/0**. This is a whole-target result, not a sum of filtered retries. Historical failed, interrupted, compiler, fixture, C7, C8, and runner-bookkeeping attempts remain preserved and receive no pass credit.

Production has **756/756 Won**, ticks min/median/p90/max **6659/29448/70403/286896**, under original per-map limits; the narrowest maximum uses 0.9961667 of its unchanged map-scaled cap and remains a watch measurement. Duels have **756/756 Won**, zero draws/non-Won, ticks **6915/29056/73899/263681**, under the unchanged 300000 cap. Isolated all-map acceptance has **21/21 Won**, median/p90/max **86306/108805/245510**, below the unchanged 400000 cap. Three fixed 256×256 windows and the M13/M14/M7 refusal guards pass.

The soak executes **260 configurations × 2 worlds × 36000 ticks = 18720000 deterministic ticks**, with every-tick equality/invariants and **247.2 MiB** peak concurrent heap under 1024 MiB. Its final sampled worlds remain Running by the fixed-window protocol; completed outcomes must be Won, and all-to-Won evidence comes from the separate production, duel, late-twin and all-21 workloads. The late twins compare independently constructed shipped worlds per tick, include post-36000 firing/production/positive ore delivery, and reach the same eventual winner. Extra-RNG and late-observer one-cause mutations fail as intended and restored positives pass.

The two 200-unit benchmark scenarios report overall mean/max **0.099/1.031 ms** and **0.279/1.399 ms**, within the 5 ms mean and 50 ms max guards. Isolated observed busy-window/p99/max tails include **4.897/24.65/299.238 ms**; these are retained measurements and do not establish render smoothness.

The corrected asset receipt is **5218 bytes**, SHA-256 `d0c5f62b1a7b11d78cdc5ea63d89be1c96beeaf418f73cb4fa7fa6e7a20ba583`. The release and packaged routes use executable SHA-256 `b802513239d9d14b50b0029d2979032e72e452965b2c2a9e768b642c1754dcc3`; each exits 1 with the required image absent and exits 0 for 600 frames after restoration. The overlay is restored and the canonical image is unchanged. The phase-11 package README snapshot is kept distinct from later closing prose.

## Round-1 blocking finding dispositions

Every earlier P0/P1 was traced through its current source boundary and corresponding final unfiltered execution:

| Earlier finding | Binding disposition |
|---|---|
| P0-01 stale IFV repair liveness | Resolved: current healer and patient HP are reread after parasite damage; dead actors cannot heal or revive. |
| P0-02 FreeMCV ignores transported MCV | Resolved: recursive live cargo participates in owned base-unit eligibility and lifecycle controls. |
| P0-03 specialist damage drops firepower multipliers | Resolved: Squid/Yuri actor-fired damage applies actual rank/crate firepower exactly once through the shared damage path. |
| P0-04 Guard starves on ineligible target | Resolved: Guard uses effective weapon/layer/Verses eligibility before ranking. |
| P0-05 pending bay launch while erased/dead | Resolved: launch admission checks current health and erased/disabled state while preserving already-launched flight. |
| P0-06 erased miners work/teleport | Resolved: erased harvest/unload/Chrono return jobs wait and resume after release. |
| P0-07 frozen building sell/undeploy | Resolved: active transformation is rejected while erased; passive/administrative policies remain explicitly bounded. |
| P1-01 weak legacy/masked audit admissions | Resolved: exact identities, guards, typed routes/hashes and accepted-baseline negatives participate in the complete 185-case audit. |
| P1-02 Spy hover identity leak | Resolved by observer-qualified apparent identity/owner presentation controls. |
| P1-03 stale mind-control selections/groups | Resolved by ownership/visibility pruning and marker filtering. |
| P1-04 concealed actor damage-smoke leak | Resolved by actor concealment plus terrain visibility admission. |
| P1-05 Interact pre-validation occupancy leak | Resolved through observer-known target selection with hidden/missing equality controls. |
| P1-06 fogged scenery live-state leak | Resolved through observer-qualified remembered scenery and re-observation reconciliation. |
| P1-07 AI bridge approach hidden-occupancy leak | Resolved through public static terrain and observed/remembered occupancy only. |
| P1-08 incomplete final acceptance | Resolved by the exact frozen full13, isolated five, asset failure/restoration, source binding, and five complete final slices. |

The later R201 Liberty non-Won was a real failed gate and remains preserved; R202's live-owned-refinery financing predicate corrected it without raising limits. R203's erased-Prop Deploy and subsequent erased Sell failures were real; R204 admits only current nonerased command candidates while retaining capability and recovery. R205 corrected a stale test expectation to match dead-unit movement exclusion. R206/R207 removed pre-acquisition fixture interference while preserving genuine occupied-Prop fire and explicit Temporal assertions. R208 corrected smoke observation so earlier movement survives later unit destruction without changing the 600-frame or 40-tick thresholds. The C9 final gate then passed unchanged acceptance criteria.

## Requirements, data, policies, and limits

- **R6 / I6′:** one vehicle or ship, up to three infantry spots, and one separate aircraft occupant per corresponding cell; buildings exclude surface occupancy. Registration, movement, factory exits, cargo, teleport, crew, spawn, crushing and setup routes share the invariant checks. Chosen geometric offsets remain documented assumptions.
- **R7:** source speed is converted from 15-frame motion to 20 simulation ticks with the documented rational factor, infantry factor, and explicit Jumpjet assumption. GI/Grizzly/Harrier generated values remain 11/13/26. Subjective 1× pacing remains user-only.
- **R29:** base 72000 and original map-span scaling remain; 300000 duel, 400000 all-to-end, 24000 fixed-window and 36000 soak scopes are distinct. No cap was raised during correction.
- **R4:** map growth is optional and default-off, with both states covered across shipped map/player-count setup; this explicit later decision reverses the earlier desired default and remains documented.
- **I7/determinism/privacy:** integer payment/refund/repair/ore paths retain nonnegative accounting; state-changing iteration is ordered; gameplay RNG uses `SimRng`; mutable state is hashed; command and observer boundaries retain hidden/missing equivalence. No floating gameplay state or wall-clock decision path was found.
- **Schemas/loaders:** current shipped schemas are units1, economy2, combat5, arsenal10, skirmish3, ai3 and rules4. Strict version/unknown/duplicate/range/reference/subset checks, 192 KiB bound, fixtures, and regeneration checks pass.
- **Catalogue:** 54 units, 46 buildings, 116 weapons, 61 warheads, 11 armours, 9 countries, 21 maps, 11 raw AI plans and 3 difficulties are loaded.
- **Source/data audit:** 16211 physical facts = 13419 Compared + 2792 GameOwned; 7108 demanded/successful dependencies; 5022 source atoms = 3111 Compared + 1911 typed dispositions. The 1911 are 759 `CURRENT_MODEL_NONREAD` + 1152 `INTENTIONAL_DIFFERENCE`, 1869 Whole + 42 components, and zero `ORIGINAL_ENGINE_UNUSED`. All 75 numeric differences retain specific reasons. Static absence, source fingerprints, proposals, or GameOwned labels are not treated as native execution or proof of original-engine nonuse.
- **Assets/delivery:** shipped assets remain generated/authored or attributed in the checked catalogue. The accepted automated scope is local arm64 macOS packaging. Signing, notarization, universal output, another-Mac launch, complete dependency notices, and subjective asset/audio quality remain outside automated approval.

## Unresolved findings

### P2 — SPAWN-01 exact lost-Hornet regeneration boundary

`flight.rs` deterministically replaces a missing launched Hornet with `Slot::Lost(spawn_regen.max(1))` and decrements it to Ready. The shipped and fixture integrations prove real loss and eventual replacement, distinguish lost replacement from returned-aircraft reload, and execute successfully. They do not assert Lost immediately before and Ready exactly at stock 800 ticks (or fixture 400). Add those two edge assertions without changing data or broad tests. `R56-43` and `R56-63` are retained alias rows of this one unique finding.

### P2 — same-frame notice cap/deferred despawn

`notices.rs` starts from the current Query count, while additions and despawns are deferred. Multiple qualifying events can repeatedly choose the same oldest entity and leave more than `MAX_NOTICES` after command application; the direct announcement path does not prove a global cap. This is bounded client presentation/resource robustness with no simulation, authority, privacy, or victory effect demonstrated. Enforce the cap over current plus pending notices with distinct removals.

### P2 — PNG metadata-before-open pathname race

`tileset_image.rs::read_bounded` validates `metadata(path).is_file()` and then separately opens the pathname. Replacement with a FIFO/special file between those operations can block the later open/read. Existing size, PNG structure/dimension/decode, missing-file, non-file and oversized-file controls remain valid. Open first, validate the opened handle, and retain typed error and regular-symlink behavior.

Earlier P2 audit-header and headless-scenery prose errors are corrected and are not carried forward. No unresolved numerical/data-integrity risk, compatibility break, failed mandatory gate, or unverified automated acceptance criterion remains.

## Individual debt reconciliation

The companion TSV is my per-row adjudication, not a blanket closure. It contains exactly **183 data rows**, **183 unique stable IDs**, and the `(ledger_id, historical_aliases)` pairs compare byte-for-byte equal and in the same order as `docs/RA2_HISTORICAL_DEBT.tsv`. Each row has an individual disposition, severity, evidence built from its own source/test/decision scope, and remaining obligation. `SPAWN-01` and `FIFO-01` carry the two ledger-backed P2s; `R56-43` and `R56-63` are explicit aliases of the single SPAWN finding. The notice-cap P2 has no distinct stable ledger row and is therefore recorded in this binding report without inventing a 184th ID.

Passing final execution closes prior final-binding obligations only to each assertion's real scope. It does not erase intentional model differences, source uncertainty, compatibility boundaries, delivery limits, or user-only checks.

## All 51 pending user checks

Every item below remains **PENDING — requires user verification**. The exact procedures are in `docs/RA2_GAP_ANALYSIS.md` under “How to perform user verification.”

**Display, camera and selection**
- PENDING — requires user verification: Launch/menu readability
- PENDING — requires user verification: Window scaling/text
- PENDING — requires user verification: Camera pan/zoom
- PENDING — requires user verification: Optional edge scrolling
- PENDING — requires user verification: Selection/hit precision
- PENDING — requires user verification: Groups/bookmarks
- PENDING — requires user verification: Menu/field isolation
- PENDING — requires user verification: Owner/color/readability

**Lobby, production, economy and world**
- PENDING — requires user verification: RA2 defaults
- PENDING — requires user verification: Options/restart
- PENDING — requires user verification: Side/national tree
- PENDING — requires user verification: Locks/lanes/queue/refunds
- PENDING — requires user verification: Placement
- PENDING — requires user verification: Power/radar
- PENDING — requires user verification: Mining/gems/regrowth
- PENDING — requires user verification: Crates
- PENDING — requires user verification: Paid repair
- PENDING — requires user verification: Oil/Hospital/Outpost
- PENDING — requires user verification: Airport
- PENDING — requires user verification: Garrisons
- PENDING — requires user verification: Bridge destruction/repair
- PENDING — requires user verification: Map environments/growth

**Units, specialists and strategy**
- PENDING — requires user verification: Weapon classes/splash
- PENDING — requires user verification: Infantry/crushing
- PENDING — requires user verification: Promotion/crews
- PENDING — requires user verification: Air docks/sorties
- PENDING — requires user verification: Missile interception/wings
- PENDING — requires user verification: Submerged concealment
- PENDING — requires user verification: GI/Desolator/Yuri deploy
- PENDING — requires user verification: Spy/detection
- PENDING — requires user verification: Stolen tech
- PENDING — requires user verification: C4/Ivan/defuse
- PENDING — requires user verification: Temporal/charging/support
- PENDING — requires user verification: Mirage/control
- PENDING — requires user verification: Parasites/IFV/transports
- PENDING — requires user verification: Radiation/death chains
- PENDING — requires user verification: Major abilities
- PENDING — requires user verification: Visibility/recon
- PENDING — requires user verification: Tactical controls
- PENDING — requires user verification: AI plan/difficulty

**Audio, feel, complete matches and delivery**
- PENDING — requires user verification: Music/effects balance
- PENDING — requires user verification: Announcements/text
- PENDING — requires user verification: Mute/volume persistence
- PENDING — requires user verification: Pause and pace
- PENDING — requires user verification: Frame/tick tails
- PENDING — requires user verification: Full normal victory
- PENDING — requires user verification: Full normal defeat
- PENDING — requires user verification: Extension isolation
- PENDING — requires user verification: Local packaged launch
- PENDING — requires user verification: Distribution/signing
- PENDING — requires user verification: Overall art/audio acceptance

## Decision and limits

Under the unchanged criteria, the final frozen source has complete mandatory gate passage, full five-slice coverage, no unresolved P0/P1, no unverified mandatory automated acceptance criterion, and no unresolved numerical/data-integrity or compatibility risk. The three remaining findings are bounded P2 maintenance/robustness debt. Therefore the binding decision is **GO WITH DOCUMENTED NON-BLOCKING DEBT**.

This decision authorizes the orchestrator's already user-authorized local main merge and reviewed tag under the prompt. It does not create the user approval tag and does not mark any of the 51 checks passed. The fixed-duration soak is not all-to-Won evidence; mean simulation budgets are not rendered-smoothness approval; source/static policy is not original-engine execution proof; local packaging is not signing, notarization, universal, or other-machine verification.
