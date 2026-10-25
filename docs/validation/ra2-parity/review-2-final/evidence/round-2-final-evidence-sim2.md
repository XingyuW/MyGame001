# Binding RA2 parity final review, round 2 — SIM2 slice

## Decision support

**GO WITH DOCUMENTED NON-BLOCKING DEBT** for the SIM2 slice.

I found **0 P0, 0 P1, and 1 P2**. The P2 is the already recorded `SPAWN-01` exact lost-carrier regeneration-boundary test gap. I found no simulation defect behind it: the loaded carrier value flows into `Slot::Lost(spawn_regen)`, and the same ordered countdown used by the exact missile and returned-aircraft controls reaches `Ready` deterministically. The remaining gap is a direct before/at-boundary native control for a genuinely lost Hornet, so it remains documented debt rather than a reason to reject this candidate.

This is slice decision support for the fresh lead, not programme approval. All 51 user-only checks remain pending.

## Binding identity

- Reviewed cumulative range: `8230a955b251e9341d349f99eaeb86f7f8253664..f6fdac5a5add10e62285141a2a4ccb3328234e2f`.
- Exact final tip: `f6fdac5a5add10e62285141a2a4ccb3328234e2f`.
- Source/native freeze commit used by the final gates: `525636dac32eb8bae4919a6dccdde1c0dbe815eb`.
- Frozen manifest: `docs/validation/ra2-parity/remediation-2/C-corrected-09/frozen-validation-inputs.json`, 148,657 bytes, SHA-256 `17d992857c80567e6d8377a8b98c4a1b254454ce13ff9e58104b811c52d5d64d`.
- Final combined summary: `docs/validation/ra2-parity/review-2-corrected-08/final-validation-summary.json`, 379,468 bytes, SHA-256 `83030b60fa3530169c9868eb98adea664c61bf583c6aa05df0572ca7aaabbb41`.
- Isolated-five summary: `docs/validation/ra2-parity/remediation-2/C-isolated-five-04/acceptance-summary.json`, 17,839 bytes, SHA-256 `718eccd049ec89f2e97bb0aa2ea5311cf456aba6e05b8e18a437caa93aeda0f0`.
- Corrected asset receipt: `docs/validation/ra2-parity/review-2-corrected-08-assets-corrected-01/process.json`, 5,218 bytes, SHA-256 `d0c5f62b1a7b11d78cdc5ea63d89be1c96beeaf418f73cb4fa7fa6e7a20ba583`.

The two hashes supplied in the dispatch match the files at the final tip. I independently recomputed every one of the manifest's 689 listed file hashes; there were zero mismatches. The manifest comprises 674 source inputs and 15 derived inputs, preserves 64 authority spans, and binds the 51-row HOWTOSEE material at SHA-256 `8ae4edfadf97fc68138d03c9a12d9c1217cf0c1fb65cf6dd6010c77b1c5bd6c1`.

The final two commits after the gate freeze add documentation and preserved evidence only. `git diff 525636d..f6fdac5` contains no production, test, data, generator, or validation-script change. The tested source therefore matches the reviewed source.

## Method

I read the binding continuation review procedure, current panel README and SIM2 brief, the review requirements, outstanding-review record, current gap analysis, handoff, integration checklist, decision tail, and the 183-row historical debt ledger. I treated their statements as claims.

I inspected the full cumulative diff and the current SIM2 production graph: `ai.rs`, `ai_defs.rs`, all reachable `ai/*`; `combat.rs`, `combat_defs.rs`, reachable `combat/*`; `special.rs`, all reachable `special/*` and `special/infantry/*`; `bridges.rs`, `crates.rs`, `crew.rs`, `garrison.rs`, `props.rs`, `ra8_intents.rs`, `veterancy.rs`, `visibility.rs`, and `world_objects.rs`. I cross-checked shared `flight.rs`, movement/economy admission, state hashing, client-facing privacy/event boundaries, the assigned native controls, RA1–RA8 suites, and late-game AI evidence.

I did not run Cargo, native applications, long matches, or simulations. I did not edit the repository, Git index, or refs, and did not stop any process. Evidence inspection was read-only.

## Findings

### P2 — `SPAWN-01`: exact lost-Hornet regeneration edge remains unpinned

Files: `crates/sim/src/flight.rs:465`, `crates/sim/src/flight.rs:690`, `crates/sim/tests/ra4_spawns.rs:641`, `crates/sim/tests/ra4_spawns.rs:708`, `crates/sim/tests/ra9_guard75_readiness.rs`.

The carrier requirement specifies three Hornets, 27-tick launch spacing, returned-aircraft reload, and lost-aircraft regeneration. Source inspection confirms that a missing launched actor settles its actual bay slot to `Slot::Lost(regen.max(1))`, where `regen` is the parent's loaded `spawn_regen`; the ordered bay pass decrements `Lost(t)` and makes it `Ready` at `t <= 1`. The final suite passes the exact missile lost/relaunch boundary, exact returned-carrier reload boundary, and exact 27-tick first-three launch spacing. A broader carrier test proves a lost Hornet is eventually regenerated.

What remains absent is a native carrier test that shoots down an actual Hornet and asserts both the tick immediately before readiness and the readiness tick at the stock lost-aircraft interval. This is the narrow obligation recorded as `SPAWN-01` (with aliases `R56-43`, `R56-63`) and related source-qualified assumption `SPAWN-SPACING-02`. Production behavior is coherent and the acceptance criterion that a carrier regains a lost Hornet is exercised, so I retain the historical P2 classification. Expected follow-up: add the exact before/at lost-Hornet boundary without weakening the existing broad test.

## Independently verified corrected behavior

### Temporal current-state admission and AI recovery

`View::new` derives `erased_buildings` from current `SpecialState.erasures`, including actorless occupied Props synthesized into current building snapshots. Normal financing sales and last-stand sales exclude erased hosts. Recoverable garrisons remain recovery capability, but AI deploys only an unerased host; when all are erased it waits and resumes after native release. This preserves command-layer rejection rather than bypassing it. The derived view adds no mutable unhashed state.

The final corrections to `ra9_remaining_props.rs` and `ra9_scenery_connected.rs` strengthen rather than weaken the premise: each proves real occupied-host fire before spawning the Temporal source, requires the explicit Attack to start the only channel, observes an actual `WeaponFired`, and breaks the beam with a real Move. Both current tests pass in the unfiltered final workspace log. This closes the earlier fixture self-acquisition ambiguity without changing production code.

### Same-tick death, combat attribution, and veterancy

The revised crusher regression now states the actual invariant: a unit killed before Movement no longer participates in movement/crushing. Its live control still proves ordinary crushing and cost credit; the killed case proves neither a crush event nor XP credit. The named test passes in the final unfiltered run. I found no double attribution or source-owner leak in the connected projectile, spawned-unit, parasite, death-payload, crew, and rank paths.

### Specialists, neutral scenery, selectors, and privacy

I followed the real selector and effect routes for garrisons/Props, C4/Ivan/defuse, Temporal erasure, radiation, parasites/Squid, disguise/Mirage/submerged state, Spy infiltration, support chains, transports, strategic actors, and off-map payloads. Effects retain current actor/owner/HP checks at their execution boundaries; area damage uses the warhead/armour path; source identity is preserved for damage and XP. Hidden sets gate AI snapshots and launched-unit target knowledge, while observer-visible strategic notices do not grant hidden actor knowledge. Current public-event/client filtering remains a separate client responsibility and does not mutate simulation knowledge.

### AI, cross-module service/economy, and endings

AI uses per-player vision plus hashed terrain memory, excludes hidden entities, uses live producer/buildability and funding state, includes living nested cargo in ownership/capability, and keeps repair/service candidates tied to a live owned appropriate host. The corrected visible-ore finance predicate requires a live refinery and visible remaining ore. Difficulty/country plans and last-stand scheduling remain bounded and hashed.

The final late twins are two independently constructed shipped-data worlds with every-tick hash/event/outcome equality and I6-prime/I7 checks. They reached `Won { winner: PlayerId(0), tick: 42056 }` after 42,057 schedules and retained post-36,000 activity: 330 `WeaponFired`, 3 `ProductionComplete`, and 7 positive `OreDelivered` events; final hash `991a7990712f9fe8`, trajectory hash `cf968dbee1b99dd4`. The extra RNG draw diverged exactly at tick 36,001. Suppressing only late `ProductionComplete` observation reached the same world/winner but failed its late-event assertion. Source restoration and a separate later positive are recorded. These controls distinguish deterministic state from observer evidence.

### State hash and invariants

SIM2 mutable state is stored in ordered maps/sets or stable vectors and is included in the state hash: AI plan/difficulty/jobs/memory/terrain/last-stand state; tactical orders; experience/kills/wrecks; Props/bridges/crates/world objects; strategic state; erasures, protection/disable timers, radiation, bombs/zones, charge/outages, cargo/garrisons/control/interactions/auto-defuse, visibility concealment, docks/ammo/reload, bays/spawned missiles/surface state, and command queue. Entity rows are sorted by `SimId`. I found no wall-clock or unseeded randomness in the reviewed production paths, no new floating-point simulation, and no reachable data/command panic introduced in those paths. The exact late twins and two intentional one-cause failures give execution evidence for the hash boundary.

I6-prime remains one vehicle or three infantry in a surface cell and one aircraft in an air cell, with no unit on a building. The isolated map sweep and production/duel suites exercise the invariant through real schedules; the final workspace has zero ignored tests and the repository contains zero `#[ignore]` attributes. R4 remains option/default-off, R7 retains the 15-frame to 20-tick speed conversion, and R29's base `MATCH_TICK_LIMIT=72,000` and map scaling are unchanged.

## Final evidence inspection

All 13 required commands exited zero on the unchanged 689-file/64-span source set. Raw log SHA-256 values independently recomputed:

| Command | Raw SHA-256 |
|---|---|
| 01 fmt | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| 02 diff-check | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| 03 clippy | `7f7b61448abaf627f4b3932b843297443789fa492fb6eb45f296354f900857e9` |
| 04 workspace tests | `7d3f274aabaf33d8d38463810d2d7e0a0f94d7a11506f2b658524c0d2ef25b7b` |
| 05 release build | `a40c1884a95654da1e9ce0b9042cc3b906a7c879f74bfc6527def185ef03cdb2` |
| 06 release smoke | `2c3308be638069bbe5143d0e8870141fe1308b28732a48736f1fb51580019464` |
| 07 debug smoke | `4aad623d6c9ffea6c2585f90f33178bb08baf2e5d6e12da235db5fa8e9aaefd1` |
| 08 benchmark | `c5489ee4c50153a5c0ae5d7692421d444b952170b7c1e2f677f7993820c73660` |
| 09 soak | `7aff0f06d2b88fbcf10eb436c13b215f3aaf60e17c9776cd6abee958c0362950` |
| 10 duels | `3656ba662afe0e828d44f24a3690a13b6343f748e63479a82d1d6282e5d0eca6` |
| 11 package | `64afdf6b61e29cf1d2bebbda446ee987f07168d7c00775d550896ba2fa7578c5` |
| 12 generated-data check | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| 13 reference check | `049b7392cd329aff67646187d48320f1ac79b1430aba17f0dcea7345d442cda5` |

The workspace command contains 1,835 primary unfiltered passes, 0 failures, 0 ignored, and 0 filtered, plus 9 separately accounted selected child probes (1,844 total reported passes). The whole RA9 audit passes 185/0/0/0 together, and the full AI integration target passes 146/0/0/0.

Production is 756/756 Won, ticks min/median/p90/max `6659/29448/70403/286896`; the maximum is 0.9962 of its original map-scaled limit. Duels are 756/756 Won, median/p90/max `29056/73899/263681` under the unchanged 300,000 cap. The fixed soak executes 260 configurations twice for 18,720,000 deterministic ticks; sampled final worlds may remain Running under R56, while every completed outcome is guarded as Won. Peak concurrent heap is 247.2 MiB under 1,024 MiB. The benchmark's fixture/shipped mean/max are `0.099/1.031 ms` and `0.279/1.399 ms`, both within the original 5/50 ms guards.

The isolated raw hashes are `6a999478...` (all 21), `c5724f05...` (three fixed windows), `43c6ca9a...`, `36b036a1...`, and `ce11b2a6...` (the three refusal guards). All 21 maps reach Won below 400,000 ticks; median/p90/max are `86306/108805/245510`. All overall means are below 5 ms; the observed worst busy-1000 mean/p99/max are `4.897/24.65/299.238 ms`. These tails do not establish rendered smoothness.

The corrected asset receipt uses an external symlink overlay and the exact tested executable SHA-256 `b802513239d9d14b50b0029d2979032e72e452965b2c2a9e768b642c1754dcc3`. Release and packaged missing-image runs each exit 1 with path context; restored-image runs each exit 0 after 600 frames. The receipt states both overlay restoration and canonical-image unchanged. This validates failure handling, not visual quality or other-machine packaging.

## Documented non-blocking debt and limits

Besides `SPAWN-01`, I verified that the current documents preserve relevant bounded limitations instead of claiming them closed: `SPAWN-SPACING-02` (source-qualified 27-tick spacing), `GATHER-01` (contrived building-partition reachability), `VHP-01` (no projected-health target reservation), `RETURN-FIRE-01`, `MIRAGE-GATE-01`, `EXP-SPREAD-01`, `FIRE-ANIMATION-01`, `DEFAULT-DEATH-01`, `HARVEST-SOURCE-02`, and `REVEAL-SW-01`. The 1,911 specific source dispositions remain 759 `CURRENT_MODEL_NONREAD` plus 1,152 `INTENTIONAL_DIFFERENCE`, with zero claims of `ORIGINAL_ENGINE_UNUSED`; they are model boundaries, not blanket equivalence evidence.

The debt ledger has exactly 183 rows, 183 unique IDs, and 183 unique aliases. Native passage supplies evidence only within each test's actual scope. It does not erase the retained architecture, source uncertainty, or user-quality items.

## User-only verification

All **51** HOWTOSEE rows remain **PENDING**. This includes visual art and animation, audio quality and mix, readability, input/gameplay feel and pacing, normal human victory/defeat flows, and signing/notarization/universal/other-machine packaged launch. None is a P0/P1/P2 review finding and none was converted into automated approval.

## Repository and artifact hygiene

The repository was clean at start and at completion (`git status --short` empty), with HEAD exactly `f6fdac5a5add10e62285141a2a4ccb3328234e2f`. I made no repository, index, branch, tag, or ref change. I created no clone, worktree, build output, cache, dataset, or temporary validation environment and stopped no process. The requested review-cache directory already existed; the only artifact created by this review is this report, and it remains necessary for the binding panel handoff.
