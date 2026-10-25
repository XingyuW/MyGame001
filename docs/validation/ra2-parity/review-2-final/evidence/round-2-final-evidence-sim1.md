# RA2 parity round 2 final-evidence review — SIM1

## Decision support

**GO WITH DOCUMENTED NON-BLOCKING DEBT** for the SIM1 slice.

Confirmed unresolved findings: **P0 0, P1 0, P2 1**. The one P2 is the retained SPAWN-01 / R56-43 / R56-63 exact lost-carrier-Hornet regeneration-boundary test gap. It is not a demonstrated simulation defect and does not leave the A4.4 positive regeneration criterion unverified. All **51 user-only checks remain PENDING** and are excluded from finding counts and from native approval.

This report supplies slice evidence to the fresh lead. It does not approve the complete programme, user-visible quality, a merge, or a tag.

## Binding identity and repository state

- Repository: `/Users/xingyuwang/GameProjects/MyGame001`
- Reviewed cumulative range: `8230a95..f6fdac5a5add10e62285141a2a4ccb3328234e2f`
- Exact final tip read from Git: `f6fdac5a5add10e62285141a2a4ccb3328234e2f`
- Frozen manifest: `docs/validation/ra2-parity/remediation-2/C-corrected-09/frozen-validation-inputs.json`, 148657 bytes, SHA-256 `17d992857c80567e6d8377a8b98c4a1b254454ce13ff9e58104b811c52d5d64d`
- Final combined summary: `docs/validation/ra2-parity/review-2-corrected-08/final-validation-summary.json`, SHA-256 `83030b60fa3530169c9868eb98adea664c61bf583c6aa05df0572ca7aaabbb41`
- Gate launch commit: `525636dac32eb8bae4919a6dccdde1c0dbe815eb`; isolated-five runtime commit: `05273ccf4ab8933c8ce667356c528f18b7a6a979`. Read-only `git diff --name-only` found no runtime, test, asset, script, Cargo manifest, or lock-file change from either of those commits to final tip; later commits preserve evidence and closing documents.
- The working tree was clean at entry and clean after review (`git status --short` emitted no paths). I made no repository, index, or ref change, ran no Cargo command or long simulation, and stopped no process.
- The frozen manifest contains 689 source/factual inputs and 64 authority fragments. I independently read and SHA-256 checked every current path: **689/689 matched**, with zero size or hash mismatch. Its HOWTOSEE record remains 51 rows / 13855 bytes / SHA-256 `8ae4edfadf97fc68138d03c9a12d9c1217cf0c1fb65cf6dd6010c77b1c5bd6c1`.

## Method

I read the binding review procedure and current SIM1 ownership in `docs/prompts/RA2_PARITY_CONTINUATION_PROMPT.md` section 6, `docs/prompts/ra2-parity-review/README.md`, `slice-sim1.md`, `docs/reviews/RA2-parity-review-requirements.md`, and `docs/reviews/RA2-parity-reviews-outstanding.md`. I treated their conclusions and prior slice reports only as leads.

I inspected the complete cumulative diff's SIM1 footprint and the current production/test bodies for command validation, production/payment/refunds/repair, map occupancy and movement, match setup, flight/bays/docks, harvesting/ore, deterministic ordering, ID exhaustion, schedules, RNG, and hashes. I traced the public occupancy route through `unit::spawn` -> `Grid::register`/`claim`, movement reservation and release, building coverage checks, destination allocation, and nested/transport lifecycle controls. I traced charging through `production_rate_pm`, `advance_item`, completion, cancellation/refund, repair, dock repair and shortage branches. I traced bay loss recognition, timer decrement, launch eligibility and hashing in `flight.rs`.

I independently parsed or read the raw full13 logs, all five isolated raw logs, the corrected four-command asset receipt and raws, and the source/debt bindings. I did not rerun native tests because the brief forbids competing long/native work and the existing exact frozen run is binding under R194.

## Validation evidence checked

The raw wrapper and summary both contain all 13 unchanged commands, every exit 0, terminal `validation review-2-corrected-08: PASSED`. Command 4's 13,138,458-byte raw log has SHA-256 `7d3f274aabaf33d8d38463810d2d7e0a0f94d7a11506f2b658524c0d2ef25b7b`; accounting distinguishes 117 primary unfiltered result blocks (**1835 passed, 0 failed, 0 ignored, 0 filtered**) from nine selected-child probes. The raw script total is 1844 passed, 0 failed, 0 ignored. The full RA9 audit is a single unfiltered **185/0/0/0** result, and the full AI integration target is **146/0/0/0**. Relevant primary targets visibly pass in the raw log: `ra2_charging` 2, `ra2_production` 15, `ra2_repair` 12, `ra3_cells` 22, `ra4_shipped` 17, `ra4_spawns` 18, `ra9_airdock_retry` 1, `ra9_pad_health` 10, `ra9_start_setup` 1, and the transport family 5+1+4.

The final production sweep records **756 Won**, no non-Won row, median/p90/max 29448.0/70403/286896 ticks. The duel raw records **756 Won**, no non-Won row, median/p90/max 29056.0/73899/263681 ticks under the unchanged 300000 cap. The deterministic soak records 260 configurations x2 x36000 = 18,720,000 ticks and 247.2 MiB concurrent peak against 1024 MiB; sampled final worlds are correctly reported as Running fixed windows, not silently promoted to Won. The 200-original-unit benchmark passes movement/combat and the original 5 ms mean/50 ms maximum guards.

The isolated-five summary is 17839 bytes, SHA-256 `718eccd049ec89f2e97bb0aa2ea5311cf456aba6e05b8e18a437caa93aeda0f0`; its process receipt is SHA-256 `7f2497f09eef44bce7c327e2228b7b6d4661bfe12b91b6b738a56d1d1fa4706d`. I checked every referenced raw hash:

- all21-to-Won: `6a9994780a41812c54a76256bbcf3bb062c6abaf4beaa6add933001977e74d08`
- fixed-three windows: `c5724f058b66cef9666fd21ab4a2c7fd4cffd1069d1d89312610bff93e83b2ed`
- m13 ten-player guards: `43c6ca9a376ea6be3cb0ae6460152a7d1ded3f4999f64c87bd89086f7981d5b2`
- m14 refusal guards: `36b036a13482ef1e236b9d8e6d77f5ec556bef7cde29a1ad36c19855d78666ee`
- m7 soak refusal guards: `ce11b2a662a976050e77291e1df851690e61be3173cabbc3bb38629b05599e1e`

All 21 shipped maps reach actual `Won` before the unchanged 400000 cap (median 86306, p90 108805, max 245510). All three fixed 256x256 windows and all three example refusal guards pass. Every measured overall mean is below 5 ms; observed busy-window/p99/max tails remain measurements rather than rendered-smoothness claims.

The corrected asset process receipt is 5218 bytes, SHA-256 `d0c5f62b1a7b11d78cdc5ea63d89be1c96beeaf418f73cb4fa7fa6e7a20ba583`. Both release and packaged executables use the same SHA-256 `b802513239d9d14b50b0029d2979032e72e452965b2c2a9e768b642c1754dcc3`: each exits 1 with the missing demo image/path and then exits 0 for 600 frames after overlay restoration. All four raw hashes match, `canonical_image_unchanged` is true, and `overlay_restored` is true. This proves those failure/restoration scopes, not signing, notarization, universal binaries, another Mac, or subjective quality.

## Verified SIM1 behavior

- **I6 prime and movement/placement:** `Grid` keeps separate surface and air layers; one vehicle consumes all three surface spots, infantry consume one of three, aircraft use the air occupant, and `passable_for` excludes buildings from surface cells. `spawn` checks room before ID allocation and unregisters on an unexpected failed claim. Movement proceeds in sorted SimId order, releases the old cell only after a completed reserved step, claims the next cell before entering, and releases crushed infantry before the vehicle claim. Group destinations reserve three-spot capacity in ordered maps. Current unit/movement/cargo/setup controls and the all-map yard/public-Move test pass; soak and all21 preserve invariant checks.
- **I7 and economy:** production rates use bounded integer arithmetic. `advance_item` pays only affordable whole progress, separately charges the final remainder, saturates accounting, and cannot drive credits below zero. Complete items remain queued until actual placement/exit; cancellation returns recorded paid credit. Repair and paid dock health steps check available credit before subtraction. Exact zero/exact-step/shortage/leaving/disabled/dead-pad controls pass. No live-factory shortcut was found: production exits and factory counts exclude dead, disabled, erased or unavailable producers as their specific policies require.
- **Commands and privacy:** ordinary commands are validated before their mutation runs; batching preserves atomic prerequisite/ownership/collision views. Target checks return unknown for unseen/hidden bodies and do not reveal absent-versus-concealed targets. Rejection, ID-exhaustion and late lifecycle controls are present and pass in the bound workspace run.
- **Determinism and state:** SIM1 state-changing iteration uses BTree collections or explicit SimId sorting; randomness routes through `SimRng`. I found no floating-point gameplay state/logic or wall-clock dependency (the `f32` occurrence is a lexical supported-subset parser check; `Instant` is loader-time fileguard accounting). Player queues/fractions/lanes, grid surface/air/spots, allocator, harvest state, ore/gems, bay slots/target/pending/wait, docks/ammo/reloading/spawned/missiles, schedule tick and RNG are included in fixed-order hash paths. The late-twin test in the full workspace run covers independently constructed shipped worlds, every-tick hashes/events/outcomes, I6 prime/I7, actual post-36000 activity and eventual Won; retained one-cause RNG and observer controls are separately recorded and restored.
- **Compatibility and data boundary:** rules-absent branches remain explicit in economy, flight and setup paths. File loaders validate bounded subsets before semantic use; generated-data and reference-table checks both exit 0. The final loaded catalogue and completion audit are source-bound through the 689-file manifest; the complete 185-case audit passes together rather than being synthesized from filtered retries.
- **Speed and limits:** the source/requirements retain R7's 15-frame-to-20-tick conversion, R29's doubled match limit policy, the base 72000 limit, map-derived production limits, unchanged 300000 duel cap and unchanged 400000 isolated cap. The evidence reports observed results without relabeling the 36000 soak windows as all-to-Won.

## Finding

### P2 — SPAWN-01 exact lost-Hornet 800-tick boundary remains unpinned

Files: `crates/sim/src/flight.rs:468-481, 671-697`; `crates/sim/tests/ra4_spawns.rs:326-354`; `crates/sim/tests/ra4_shipped.rs:864-912`; debt aliases `SPAWN-01`, `R56-43`, `R56-63` in `docs/RA2_HISTORICAL_DEBT.tsv`.

Evidence: when the launched Hornet disappears from the indexed live actors, `spawned_units` recognizes the loss and replaces its exact `Out(unit)` bay slot with `Lost(spawn_regen.max(1))`. In the same Economy advance, `bays` decrements the cloned slot once, changes Lost/Reloading to Ready when the remaining count is at most one, and hashes the slot tag/count plus target, pending and wait. The implementation is deterministic and consistent with its stated timer convention.

The fixture test observes real loss and asserts all slots Ready after a broad 500-tick wait for a 400-tick fixture timer. The shipped-data test proves a real Hornet loss, proves the other two returned aircraft are Ready while the lost slot is still Lost after 400 ticks, then observes all three Ready after another 410 ticks for stock `SpawnRegenRate=600` frames = 800 ticks. The bound full workspace run executes both test targets successfully. Existing exact 27-tick launch spacing, missile Lost200 and returned-aircraft Reload100 checks cover different timers.

Expected improvement: record the actual tick at which the loss becomes `Slot::Lost`, assert it remains Lost immediately before the configured 800-tick regeneration boundary, and becomes Ready at the boundary (and do the same for the fixture's configured boundary). Preserve the broad integration tests.

Classification: **P2 remains correct.** A4.4 requires that a lost carrier aircraft regenerates; genuine shipped-data and fixture paths prove that positive behavior and distinguish regeneration from the shorter returned-aircraft reload. No wrong timer result, violated invariant, missing milestone acceptance criterion, weakened/skipped test, silent fallback, or unverified source value is demonstrated. The missing assertion is precision/maintainability debt around an already working and executed behavior. If a future exact test shows an off-by-one incompatible with the documented convention, that concrete result should be reclassified as P0.

## Debt and user-only scope

`docs/RA2_HISTORICAL_DEBT.tsv` is 182826 bytes, SHA-256 `d0c93c3a6cb828449a0fb859265e40b0987eb85de003688c279c1668987eecbe`, with 183 data rows and 183 unique IDs. For this slice, SPAWN-01 and aliases remain explicitly `OPEN_EXACT_TIMING_OBLIGATION` / `NOT_ACCEPTED_BY_DOCUMENTATION`. Other current-model limitations remain owned by their assigned slices and the lead; this SIM1 report does not blanket-close the ledger.

All 51 HOWTOSEE rows remain **PENDING**. They include visual/art/audio/readability/animation/pacing/feel, normal human victory/defeat, packaged/platform and other user-observed quality. Native smokes, App tests, hashes and asset-failure checks do not substitute for those user checks.

## Limitations and lead handoff

I did not rerun any native workload, inspect the game visually, test audio/input feel, launch on another machine, or perform signing/notarization checks. I did not attempt an exhaustive manual proof of every one of the 169k+ cumulative test/data lines; I combined full cumulative diff census, targeted whole-path inspection of SIM1 ownership/interfaces, exact frozen-input rehashing, and direct raw-result verification. The data, tests, SIM2 and client reviewers remain responsible for their primary slices and interfaces. Subject to their reports and the lead's reconciliation, this SIM1 evidence supports **GO WITH DOCUMENTED NON-BLOCKING DEBT**, with SPAWN-01 recorded as the slice's one P2.
