# Independent DATA source-only review — frozen commit `437d69e9ca2038384970c86368e690c1c056ef33`

Reviewed the complete cumulative range `8230a95..437d69e9` under the DATA/RA9 slice requirements. I did not modify files, run native gates, inspect or credit the live `review-2-corrected-03` output, or contact other agents.

## Decision

**No confirmed DATA-slice P0 or P1 source finding remains. No new unique DATA P2 was found.**

This is a source-only conclusion. Final acceptance remains blocked until the frozen source receives the complete mandatory validation evidence and the fresh lead reviews it. It does not authorize GO, merge, or tagging.

## Source and binding integrity

- The C4 frozen manifest at `docs/validation/ra2-parity/remediation-2/C-corrected-04/frozen-validation-inputs.json` is **148,657 bytes**, SHA256 `9df40e2ce6a42fdb0dadd8d20f1295842de69940429bea78e7b975ee854aec26`.
- It contains **689 inputs**: **674 source** and **15 derived**.
- I independently rehashed all 689 listed paths: **0 size/hash mismatches**.
- I independently checked all **64 protected authority spans** at their recorded byte offsets: **0 mismatches**.
- C4’s substantive production change is confined to current Temporal command candidate selection in `crates/sim/src/ai.rs`, plus appended regression tests in `crates/sim/tests/ra8_ai.rs` and `crates/game/tests/m6_production_matches.rs`. It adds no schema, data value, persistent state, RNG draw, timer, roster, tick-limit, loader, or source-policy classification change.
- The 13 propagated factual outputs update current source hashes/bindings while preserving classification, exact value, reason, route, guard, decision, and qualified-receipt text. The root factual verification records **3,290 changed cells**, **71 changed branch families**, and **72 local acceptance rows**.

## RA9 data audit and disposition integrity

Source inspection and preserved frozen evidence consistently show:

- **16,211 physical entries** = 13,419 Compared + 2,792 GameOwned.
- **7,108** demanded/successful dependency pairs.
- **5,022 source atoms** = 3,111 Compared + **1,911 typed dispositions**.
- The 1,911 dispositions cover **1,888 source keys**:
  - **759 `CURRENT_MODEL_NONREAD`**
  - **1,152 `INTENTIONAL_DIFFERENCE`**
  - **0 `ORIGINAL_ENGINE_UNUSED`**
  - **1,869 Whole** atoms
  - **19 Component1 + 19 Component2 + 4 Component3**
- The separate 80 authored-array policies are not falsely included in the 1,911 source-atom total.
- `ra2_differences.tsv` contains **75 explicit differences**, all with a nonempty specific reason. The focused current reconciliation reports all 75 consumed.
- The current focused audit result is properly scoped as **1 pass / 184 filtered**, not a combined 185-case pass.

I found no source-side evidence that a read, hash pin, default, alias, or GameOwned marker is being substituted for an actual comparison. The bounded model-absence and intentional-difference classifications remain appropriately qualified as current-model policy, rather than claims about universal original-engine nonuse.

## Numerical, schema, provenance, and intentional differences

The previously reviewed DATA conclusions remain applicable because their inputs are byte-identical at C4:

- Seven shipped schemas remain `units 1`, `economy 2`, `combat 5`, `arsenal 10`, `rules 4`, `skirmish 3`, `ai 3`, with strict loader/version/range/reference behavior and the 192 KiB bound.
- The retained independent comparison has **2,655 checked values: 2,603 equal + 52 exact listed differences + 0 unexplained**.
- The raw reference set remains **4,941 rows**.
- Loader catalogue claims remain scoped to actual loaded values: 54 units, 46 buildings, 116 weapons, 61 warheads, 11 armours, 9 countries, 21 maps, 11 raw AI plans, and 3 difficulties.
- Asset/source provenance remains qualified. Source review does not replace Blender/media regeneration, renderer inspection, audio inspection, or user judgment.

The 75 differences remain explicit debt or policy, particularly generated footprint differences, bounded launcher/sentinel mappings, unified dog/engineer country variants, omitted engine helper weapons, and modeled specialist/projectile behavior. They are not silently presented as original parity.

## R4, R7, and R29

- **R4:** Map growth is a menu option, default off. This accurately implements later prompt authority and explicitly records that it reverses the user’s earlier default request. Both file-sized and grown-map behavior remain required in validation.
- **R7:** The speed conversion is dimensionally coherent: original leptons/frame converted from 15 frames/s to 20 ticks/s with the documented infantry factor and qualified `JumpjetSpeed` assumption. It intentionally makes 1x movement slower than the pre-RA1 game. This remains an assumption-backed decision, not recovered engine motion.
- **R29:** The production-match base limit remains 72,000 with map-span scaling. The factor-two derivation is documented from build-time pacing, and no C4 change raises the cap. Final production and duel distributions must still demonstrate all 756 outcomes under the unchanged limits.

## Debt integrity

`docs/RA2_HISTORICAL_DEBT.tsv` is unchanged from the prior C3 freeze:

- SHA256 `d0c93c3a6cb828449a0fb859265e40b0987eb85de003688c279c1668987eecbe`
- **183 rows**
- **183 unique ledger IDs**
- **0 blank historical-alias fields**
- Every row remains `NOT_ACCEPTED_BY_DOCUMENTATION`.

No debt ID or alias was deleted, renumbered, or marked accepted by prose. Installed/source-inspected items retain an execution or judgment obligation.

## User-only quality checks

The frozen manifest binds the complete HOWTOSEE section as:

- **51 rows**
- **13,855 bytes**
- SHA256 `8ae4edfadf97fc68138d03c9a12d9c1217cf0c1fb65cf6dd6010c77b1c5bd6c1`

All 51 remain pending user verification, including art, audio, readability, animation, pacing, feel, normal human victory/defeat, and packaged/platform launch. No source or automated test can close them.

## Evidence still required

The fresh lead must not decide GO until it has authenticated complete frozen-source evidence for:

- One complete unchanged 13-command validation run.
- All **185 RA9 audit cases together**, with no filtered cases.
- Workspace tests and the complete current AI target.
- **756/756 production matches Won** and **756/756 duels Won**, with final R29 distributions.
- The full 260 × 2 × 36,000 deterministic soak and heap limits.
- Release/debug 600-frame smoke, release build, benchmark budgets, package, generated-data equality, and reference-table equality.
- The four missing-asset failure/restoration checks.
- Final exact source, executable, dep-info, raw-log, and restoration bindings.
- Five complete independent slice reports/addenda and a fresh independent lead.
- Preservation of all 183 debt records and all 51 user-only checks.

Historical failed/interrupted C2/C3 runs and the incomplete live `review-2-corrected-03` run must remain preserved but cannot contribute pass credit.

No substantial directory or artifact was created by this review.
