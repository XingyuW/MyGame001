# Independent final evidence review — CLIENT slice

## Binding and decision

I independently reviewed the complete cumulative client/game/UI/assets scope from base `8230a955b251e9341d349f99eaeb86f7f8253664` through final evidence tip `f6fdac5a5add10e62285141a2a4ccb3328234e2f`, using `docs/prompts/ra2-parity-review/slice-client.md` and `docs/reviews/RA2-parity-review-requirements.md` as the controlling scope.

The native gate source is C9 commit `525636dac32eb8bae4919a6dccdde1c0dbe815eb`. Its frozen manifest is `docs/validation/ra2-parity/remediation-2/C-corrected-09/frozen-validation-inputs.json`, SHA-256 `17d992857c80567e6d8377a8b98c4a1b254454ce13ff9e58104b811c52d5d64d`. The final summary is `docs/validation/ra2-parity/review-2-corrected-08/final-validation-summary.json`, SHA-256 `83030b60fa3530169c9868eb98adea664c61bf583c6aa05df0572ca7aaabbb41`. The summary binds 689 source/factual inputs, 64 authority spans, no source mismatch, and an empty tracked source diff after every command. Changes from C9 to the exact final tip are documentation and preserved validation evidence; no production client, simulation, or asset source changed.

**CLIENT slice decision: GO WITH DOCUMENTED NON-BLOCKING DEBT.**

- P0: 0
- P1: 0
- P2: 2 retained client robustness findings

This is a slice decision for the fresh lead to reconcile. It does not approve any user-only visual, audio, feel, human-play, or platform check.

## Findings

### P2 — same-frame notice batches can exceed the notice cap

`crates/game/src/notices.rs:193-231` initializes `shown` from the current query, then uses deferred `despawn` and child-spawn commands. When several qualifying events are read in one update, the query does not reflect earlier deferred removals or additions. Once the cap is reached, repeated iterations can select the same oldest entity for despawn and can leave more than `MAX_NOTICES` after command application. The separate direct announcement path also does not establish a global cap.

This is a bounded presentation/resource robustness issue. It does not alter simulation state, command authority, fog/privacy, or victory logic, and the reviewed evidence does not show it causing a validation failure. Expected remediation is to enforce the cap over the complete pending batch, tracking distinct removals and pending additions.

### P2 — bounded PNG loading retains a path replacement/FIFO race

`crates/game/src/tileset_image.rs:166-175` calls `std::fs::metadata(path)`, confirms a regular file, and then separately calls `File::open(path)`. A pathname can be replaced with a FIFO or another special object between those operations; the subsequent bounded read can then block. Existing byte limit, PNG header/chunk validation, declared-dimension validation, decode validation, normal non-file refusal, missing-file behavior, and oversized-file refusal remain sound.

This is a local adversarial-filesystem robustness issue, not a failure of the tested shipped assets. Expected remediation is to open first and validate the opened handle's metadata before reading, with an appropriate no-follow/open policy where supported.

## C9 correction and failed C8 debug-smoke diagnosis

The C8 full run progressed through 1,843 workspace passes and its earlier gates, then the debug smoke reproducibly failed its movement predicate after roughly 8,946–8,947 simulation ticks. The failure was in the observation harness: movement was calculated only from actors alive on the final frame, so a unit that moved and was later destroyed disappeared from the count.

C9 changes `crates/game/src/smoke.rs` to retain the maximum observed moved-unit count on every rendered frame after the order. The focused regression `movement_observation_survives_the_unit_that_moved` proves that a later empty survivor set cannot erase a prior movement observation. The change affects smoke observation only: it does not change movement, combat, destruction, commands, simulation state, gameplay timing, or acceptance thresholds. The exact debug 600-frame smoke then passed in 154 seconds, and the complete corrected full13 passed. I found no new correctness or performance issue in this correction.

## Client/game/UI/assets scope review

I re-opened the cumulative source and reconciled the prior C4/C5/C6/C8 reports as claims rather than accepting their conclusions. The client scope includes all 40 game Rust modules named by the slice brief and their connected tests, assets and generators, `assets/LICENSES.md`, and `scripts/package_macos.sh`.

The reviewed implementation preserves these material boundaries:

- Match menu options, countries, teams, difficulties, map selection, restart, extension isolation, and conversion into `MatchOptions`/`PlayerSetup` remain explicit. R4 map growth remains optional and default-off.
- R6/I6-prime remains one vehicle, three infantry, and one aircraft per corresponding start cell. The evidence and final prose retain that exact occupancy rule.
- R7 retains the documented 15-frame to 20-tick speed conversion. The conversion is mechanically represented, while subjective pacing remains a pending user check.
- R29 retains the base `MATCH_TICK_LIMIT=72000`, original map-scaled production limits, and the separate 300,000 duel limit. All 756 production cases reached `Won`; median/p90/max were 29,448/70,403/286,896 ticks, with maximum utilization 0.9961667 of its original map-scaled cap. All 756 duels reached `Won`; median/p90/max were 29,056/73,899/263,681. The narrowest measured production headroom is retained as evidence and debt context; no cap was silently raised for this final run.
- `SimHost` retains mutable simulation ownership; gameplay mutations enter through command sinks. Selection, groups, bookmarks, context actions, placement, production, repair, rally, special controls, wall/tree targets, and strategic commands retain their command boundaries.
- Fog, shroud, powered-human radar, minimap, hover, unit/building views, apparent Spy identity, cloaked/submerged contacts, Gap effects, and event-only notices preserve the reviewed observer/privacy boundary. I found no new viewer API or AI vision grant.
- Dynamic prices, accepted-only construction transitions, settings lifecycle, opened asset handles, bounded data parsing, window/settings behavior, audio/announcer paths, HD presentation, health/effects, and menu-to-match lifecycle remain covered by source and tests at their stated scope.
- Shipped art is generated/authored or attributed through the checked asset catalogue and `assets/LICENSES.md`; packaging copies the binary, full assets, licenses, and README. Distribution remains arm64 macOS, unsigned, unnotarized, non-universal, and unverified on another Mac.

No client or asset source changed after the C9 freeze. C5 and C6 were test-fixture corrections outside client production code. C8's correction was also integration-test-only. Their narrow focused passes are consistent with the final complete evidence but are not used as substitutes for it.

## Native and raw evidence reconciliation

I inspected the raw full13 accounting, isolated-five receipts, the failed v2 asset process, and its corrected rerun.

The corrected shared full13 has 13/13 commands exiting zero:

1. formatting check;
2. diff check;
3. workspace/all-target Clippy with warnings denied;
4. release workspace tests;
5. release game build;
6. release 600-frame smoke;
7. debug 600-frame smoke;
8. benchmark;
9. deterministic soak;
10. complete duel sweep;
11. macOS packaging and packaged smoke;
12. generated-data equality;
13. reference-table equality.

The raw workspace log SHA-256 is `7d3f274aabaf33d8d38463810d2d7e0a0f94d7a11506f2b658524c0d2ef25b7b`. It records 1,844 aggregate passed results when selected child invocations are included; the independently accounted unfiltered primary contexts contain 1,835 passed, zero failed, zero ignored, and zero filtered. The complete RA9 audit passed 185/0/0/0 and the complete AI integration target passed 146/0/0/0.

The final performance/runtime evidence includes:

- Two 200-participant benchmark workloads, each with all 200 units moving and combat/destruction, overall means 0.099 ms and 0.279 ms and maxima 1.031 ms and 1.399 ms against the stated 5 ms mean and 50 ms maximum budgets.
- Deterministic soak: 260 configurations × 2 × 36,000 = 18,720,000 ticks, 247.2 MiB peak concurrent heap against 1,024 MiB. The final sampled worlds are correctly reported as fixed-duration `Running` windows, not all-to-Won evidence.
- All 21 isolated maps reached `Won` before the unchanged 400,000 cap; median/p90/max were 86,306/108,805/245,510 ticks. The three fixed 256×256 windows and all three example refusal guards passed. Their observed tail timings remain measurements, not rendered-smoothness approval.
- Release and debug 600-frame smoke both passed, and the packaged executable completed its 600-frame smoke from outside the repository.

The first v2 asset receipt, `docs/validation/ra2-parity/review-2-corrected-08-assets/process.json`, is correctly retained as `FAILED`. Its release missing-image case exited 1 as expected. Its release restored-image case exited 0 as expected, but the runner marked `pass:false` because its missing-image-context predicate was applied to the restored case; the runner then aborted before the packaged pair. This is runner bookkeeping failure, not product failure.

The corrected receipt, `docs/validation/ra2-parity/review-2-corrected-08-assets-corrected-01/process.json`, SHA-256 `d0c5f62b1a7b11d78cdc5ea63d89be1c96beeaf418f73cb4fa7fa6e7a20ba583`, runs all four required cases with the same release/package executable SHA-256 `b802513239d9d14b50b0029d2979032e72e452965b2c2a9e768b642c1754dcc3`: release missing image exits 1, release restored image exits 0, packaged missing image exits 1, and packaged restored image exits 0. All four expectations pass. The overlay was restored and the canonical image remained unchanged.

## Debt, aliases, and user-only checks

`docs/RA2_HISTORICAL_DEBT.tsv` contains exactly 183 rows, 183 unique nonblank `ledger_id` values, and a nonblank `historical_aliases` value for every row. The final metadata binds ledger SHA-256 `d0c93c3a6cb828449a0fb859265e40b0987eb85de003688c279c1668987eecbe`. I found no blanket closure of individual scopes or aliases.

The HOWTOSEE/user-quality binding remains exactly 51 rows, 13,855 bytes, SHA-256 `8ae4edfadf97fc68138d03c9a12d9c1217cf0c1fb65cf6dd6010c77b1c5bd6c1`. All 51 remain **PENDING — requires user verification**. This includes visual and audio quality, readability, animation, selection/input feel, camera behavior, pacing, frame feel, human victory and defeat, packaged launch, signing/distribution/portability, and overall art/audio acceptance. Native smoke, source inspection, automated tests, and this review do not approve those checks.

## Limits and reviewer conduct

The evidence establishes the stated automated and native scopes at the frozen C9 source. It does not establish signing, notarization, universal binaries, another-machine launch, complete dependency-notice acceptance, rendered smoothness, subjective presentation quality, or normal human-play quality.

I made no repository, index, ref, canonical evidence, or validation-process changes. I launched no Cargo/game/native validation process. This report is the only artifact I created, in the required external review directory.
