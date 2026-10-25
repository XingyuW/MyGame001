# Independent final evidence review — tests and validation

## Binding and decision support

This is the independent tests/validation slice review required by `docs/prompts/RA2_PARITY_CONTINUATION_PROMPT.md` §6. I treated prior reports and programme documents as claims and reviewed the complete cumulative programme and committed evidence.

- Cumulative range: `8230a95..f6fdac5a5add10e62285141a2a4ccb3328234e2f`
- Exact final tip: `f6fdac5a5add10e62285141a2a4ccb3328234e2f`
- Frozen C9 source: `525636dac32eb8bae4919a6dccdde1c0dbe815eb`
- C9 manifest: `docs/validation/ra2-parity/remediation-2/C-corrected-09/frozen-validation-inputs.json`
- Manifest SHA-256: `17d992857c80567e6d8377a8b98c4a1b254454ce13ff9e58104b811c52d5d64d`
- Final summary: `docs/validation/ra2-parity/review-2-corrected-08/final-validation-summary.json`
- Final summary SHA-256: `83030b60fa3530169c9868eb98adea664c61bf583c6aa05df0572ca7aaabbb41`

**Finding count: 0 P0, 0 P1, 1 retained P2, 0 new P2.**

**Slice decision support: GO WITH DOCUMENTED NON-BLOCKING DEBT.** The fresh lead owns the panel decision.

## Findings

### P0

None.

### P1

None. I found no ignored test, deleted acceptance purpose, weakened threshold, raised limit, hidden failure, or failed run relabelled as passing evidence.

### P2 — retained SPAWN-01

`crates/sim/tests/ra4_shipped.rs` proves that the carrier launches three Hornets, each delivers its expected damage, two returned aircraft become ready, a destroyed Hornet remains a lost slot at an intermediate observation, and all three slots eventually become ready. It names the configured `SpawnRegenRate=600` frames / 800 ticks, but does not assert the immediately-before state and exact 800-tick transition for the lost Hornet.

This gap remains accurately recorded as `SPAWN-01` in `docs/RA2_HISTORICAL_DEBT.tsv`, with aliases `R56-43` and `R56-63`. Separate tests prove exact 27-tick carrier launch spacing, exact scheduled missile `Lost(200)` replacement/next launch, and returned-aircraft `Reload(100)`; they do not prove the distinct lost-Hornet boundary. Retain this non-blocking debt until a before-boundary and exact-boundary assertion is added.

## Source and test integrity

The final tree contains 1,834 Rust `#[test]` attributes and zero `#[ignore]` attributes. Comparing baseline names found four removed names, all documented migrations rather than lost purposes: the M1 optional-look presentation assertion became a stronger world-object name/behavior test; M3 and M4 schema negatives moved to current schema versions 2 and 5; and the M4 projectile-length contract moved from 32 to the actual 480. Assertions were retained or strengthened.

The preserved C7 run is genuinely failed: 1,790 passed and one scenery case failed. C8 corrected that fixture without weakening the substantive assertion. C8 then passed workspace tests (1,843/0/0) but failed debug smoke reproducibly at ticks 8,946/8,947 because the harness observed movement only in the final rendered frame after the moved unit had been destroyed.

C9/R208 accumulates the maximum movement observed for original ordered IDs across rendered frames. It still requires movement when the tick gap is at least 40 and leaves the exact debug smoke command at 600 frames. Its focused regression proves observed movement survives later disappearance; corrected debug smoke passes. This changes no simulation rule, data value, threshold, cap, or frame count. No source/data/build/validation-command file changes after C9; commits through the final tip add only documentation and evidence.

## Frozen input verification

Independent rehashing at the final tip confirms 689/689 manifest file hashes (674 source plus 15 derived), 64/64 authority spans, and the 51-row HOWTOSEE byte binding. Source-before/source-after checks report no mismatch or tracked drift. C8-to-C9 acceptance-table changes are factual SHA/consumer-binding propagation; policy requirements are preserved.

The debt ledger contains 183 rows, 183 unique `ledger_id` values, and no blank `historical_aliases`. SPAWN-01 remains open.

## Full 13-command gate

I inspected `gate-process.json`, `summary.txt`, all raw command logs, production lengths, and the final summary in `docs/validation/ra2-parity/review-2-corrected-08`. The gate binds C9 and its exact manifest, drains its owned process group, and records exit 0 for the unchanged 13 commands: fmt, diff check, workspace Clippy, workspace release tests, release build, release smoke 600, debug smoke 600, benchmark, soak, full duels, package, data-generation check, and reference-table check.

Limits remain unchanged:

- base match limit 72,000 ticks;
- soak window 36,000 ticks;
- M14 duel ceiling 300,000 ticks;
- M13 all-to-end ceiling 400,000 ticks;
- fixed observation window 24,000 ticks;
- both smoke commands 600 frames.

### Exact execution accounting

The raw workspace log has 126 `test result:` blocks:

- raw aggregate: **1,844 passed, 0 failed, 0 ignored**;
- 117 primary unfiltered blocks: **1,835 passed, 0 failed, 0 ignored, 0 filtered**;
- nine selected nested child-probe blocks: **9 passed, 0 failed, 0 ignored, 1,348 filtered**.

The nine child probes explain the exact 1,835-primary versus 1,844-raw distinction. The 1,834 source-attribute census is a separate measure. Final documentation states these distinctions correctly.

The complete RA9 audit target reports **185/0/0/0** and the complete AI target **146/0/0/0**.

Production evidence contains exactly **756 unique rows**, all below applicable per-map limits: minimum 6,659, median 29,448, nearest-rank p90 70,403, maximum 286,896; the maximum is about 0.99617 of its 288,000 limit. It is a watch item but passes the unchanged limit.

Duel evidence contains exactly **756 Won** rows, zero no-winner and zero draw rows: minimum 6,915, median 29,056, nearest-rank p90 73,899, maximum 263,681. Terminal accounting says 756/756 ended.

Soak evidence contains **260 progress plus 260 result rows**, representing 260 configurations × 2 deterministic worlds × 36,000 ticks = **18,720,000 ticks**. There is no mismatch, invariant failure, or failure line; peak concurrent heap is 247.2 MiB under the 1,024 MiB ceiling. Worlds may remain Running at fixed-window end; completed outcomes must be Won. Ending evidence is separately supplied by duels and all-to-end runs.

Benchmark evidence runs 200 original iterations for fixture and shipped cases. Mean/max times are 0.099/1.031 ms and 0.279/1.399 ms, under unchanged 5 ms mean and 50 ms max thresholds; peak heap is 1.3 MiB. Package evidence records binary/archive hashes.

## Isolated five-command evidence

`docs/validation/ra2-parity/remediation-2/C-isolated-five-04/process.json` binds an evidence-only tip with byte-identical C9 functional source, `source_unchanged=true`, and the exact C9 manifest SHA. All five required commands exit 0: M13 all shipped maps to end, three fixed windows, M13 guard, M14 guard, and M7 soak guard.

All **21 shipped maps** reach Won under the unchanged 400,000-tick ceiling: median 86,306, nearest-rank p90 108,805, maximum 245,510. Liberty, Continental, and Pacific each run the exact 24,000-tick observation window and may remain Running. Each guard reports 1/0/0. Acceptance summary SHA is `718ecc7348780d8313b8ee7d9b8fc07d037a46aaf34b542018c1558f8357a56b`.

## Asset negative receipts

The initial `review-2-corrected-08-assets` packet is correctly preserved as failed and not credited; its runner misclassified the restored-release step despite the correct executable status.

The corrected `review-2-corrected-08-assets-corrected-01` packet proves fail-closed behavior for both release and packaged routes: missing required image exits 1; restoration exits 0. Both routes use the same executable SHA, missing logs name the path, the canonical image remains unchanged, and the overlay is restored.

## Requirement-specific conclusions

- **R6/I6-prime:** exact surface/air/building occupancy limits are exercised through crossings, hashing, controlled invariant negatives, every-tick soak checks, and late deterministic twins.
- **R7:** the 15-frame source interval is deliberately converted to 20 simulation ticks; generator/audit controls pin GI 11 and Grizzly 13.
- **R29:** the 72,000 base limit and map-span scaling are unchanged; no correction raises a cap.
- **R4:** default-off and optional-on map growth are covered across shipped-map/player-count scenarios.
- **Late twins:** two independently built shipped-data worlds compare per-tick state/events/outcome and invariants, reach the same Won after 36,000, and contain late firing, production, and positive ore delivery. Extra-RNG and late-observer mutations fail as intended, followed by restored positives.

## Scope and limits

All **51 HOWTOSEE/user-only checks remain PENDING**. Automated evidence does not approve art, audio, animation, readability, pacing, gameplay feel, ordinary human victory/defeat flow, signing, notarization, another-machine launch, or every platform. These are limits, not new review findings.

The soak is fixed-duration determinism evidence, not a claim that every sample wins in 36,000 ticks. Static/source-policy evidence does not substitute for rendered or human checks. This slice does not replace the other independent slices or fresh lead decision.

## Tests-slice conclusion

The frozen manifest, shared full 13-command run, isolated five, asset negatives, accounting, unchanged limits, 183-row ledger, and R6/R7/R29 evidence are mutually consistent. There are **0 P0, 0 P1, 1 retained P2, and 0 new P2**. This slice supports **GO WITH DOCUMENTED NON-BLOCKING DEBT**, retaining SPAWN-01 and all 51 pending user checks.
