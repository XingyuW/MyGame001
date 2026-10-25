# Independent final DATA slice review

## Binding and reviewer independence

I reviewed the complete cumulative `8230a955..f6fdac5` programme tree read-only under the canonical DATA slice brief and current review requirements. I did not implement, remediate, prepare, or validate this candidate, and I did not modify repository files, refs, the index, or active validation outputs.

This report binds:

- final evidence tip `f6fdac5a5add10e62285141a2a4ccb3328234e2f`;
- gate-launch/frozen C9 source commit `525636dac32eb8bae4919a6dccdde1c0dbe815eb`;
- C9 frozen manifest SHA256 `17d992857c80567e6d8377a8b98c4a1b254454ce13ff9e58104b811c52d5d64d`;
- final validation summary `docs/validation/ra2-parity/review-2-corrected-08/final-validation-summary.json`, SHA256 `83030b60fa3530169c9868eb98adea664c61bf583c6aa05df0572ca7aaabbb41`.

The C9 manifest contains 689 inputs: 674 source files and 15 derived factual outputs, plus 64 protected authority spans. I independently checked every listed Git object and authority span against the frozen commit: zero size, hash, or span mismatches. The final commits `05273cc` and `f6fdac5` preserve validation/evidence and add factual closing documentation; the complete DATA-owned byte set—seven shipped data files, generators, reference inputs/tables, disposition tables, audit adapters/contracts, model census, and policy tables—is byte-identical from C9 through the final tip.

## Findings and slice decision support

**DATA finding count: P0 0; P1 0; new DATA P2 0.**

The earlier DATA2-P2, a stale reference README description, is corrected in the final tree. I found no numerical, reference, source-disposition, loader/schema, provenance, factual-propagation, or policy-integrity defect that blocks DATA-slice acceptance.

This is slice decision support for the fresh lead. It is not a whole-programme GO decision, merge approval, release approval, or user-quality approval.

Three known P2 findings from the complete panel scope remain open outside this slice and must be carried into the lead decision:

1. `SPAWN-01`: lost carrier/Hornet replacement does not preserve the exact original ready-boundary precision.
2. Client batched notices can temporarily exceed `MAX_NOTICES` because deferred removals may select the same oldest notice.
3. Client PNG metadata/open handling has a FIFO replacement race.

## Independent data, reference, and numerical review

I re-opened the shipped RON, loader/schema code, fixture schemas, generator/reference scripts, original-source tables, all RA9 audit adapters/contracts/admissions/negatives, and the raw source caches. The original inputs remain:

- `rules.ini`: 518,789 bytes, SHA256 `adc869ca037db5141f562f18092d022045e194de0d1204a46b9ec8372b2b826f`;
- `art.ini`: 231,105 bytes, SHA256 `d732bfc8ffb6d52908342254c22731acb730ae6d2bbfb41983ef109e48a8f061`.

An independent case-insensitive INI parse, including repeated sections/last-key behavior, semicolon comments, and normalized comma whitespace, reproduces all 4,941 rows of `ra2_reference.tsv`. The final gate separately records `ra2_reference.py --check` matching all 4,941 rows and `generate_ra2_data.py --check` with no generated-data delta.

The final unfiltered audit records the same independently reviewed census on final frozen source:

- 16,211 physical facts: 13,419 Compared and 2,792 GameOwned;
- 7,108 demanded and successful dependency pairs;
- 5,022 source atoms: 3,111 Compared and 1,911 typed dispositions;
- 75 used explicit differences.

These counts agree with the complete declared/admitted/finished graph and final `RA9_AUDIT_SUCCESS` output. I did not infer semantic correctness from those counts or hashes alone: I checked the keyed values, portions, classes, nonblank reasons, consumer routes, typed qualified receipts, actual loader/model bindings, and negative guards.

The 75 explicit differences are individually reasoned and remain exactly: 47 footprint-axis rows, 6 weapon rows, 6 elite-weapon rows, 4 forbidden-country rows, 4 warhead rows, 3 range rows, 2 adjacent rows, 2 anti-ground rows, and 1 damage row. No reason is blank. They remain disclosed differences rather than silent parity claims.

The source-disposition census remains 1,911 unique atoms across 1,888 keys: 759 `CURRENT_MODEL_NONREAD`, 1,152 `INTENTIONAL_DIFFERENCE`, and zero `ORIGINAL_ENGINE_UNUSED`. Portion accounting is 1,869 Whole plus 19 component-1, 19 component-2, and 4 component-3 atoms. All class, exact value, reason, route, and code cells are populated. The union of the source acceptance tables matches every disposition identity/value/reason exactly, with no missing, duplicate, foreign, or mismatched atom. `own-policy-acceptance.tsv` remains a separate 80-row authored-array policy surface.

The reachable audit graph contains every Rust sibling and every referenced TSV/TXT input, with no missing include or unreferenced active audit module. Independently recomputed byte counts, FNV-1a values, and SHA256 values match all 105 model-census rows, all 29 legacy branch-file rows, and all 11 completion-local-code rows. This supports the exact current implementation model only; it does not prove an omitted key was unused by the original engine.

## C9 factual propagation and policy preservation

I independently reconstructed the C9 propagation receipt rather than accepting its aggregate claim. All 15 preimage, preview, and final file hashes match. Exactly 3,290 cells changed:

- 1,994 `final_input_sha256` cells;
- 1,294 `consumer_bindings` cells;
- 1 `model_census_sha256` cell;
- 1 `recorded_scope_review` cell.

All consumer-binding changes preserve the non-hash text and token order after normalizing only the named 64- and 16-hex factual tokens. Every class, value, semantic reason, guard, recorded policy decision, and typed qualified receipt remains byte-equal. There are zero changed physical branch families; all 72 completion-local rows remain proved. The independent post-propagation readback agrees and records 674 source files, 15 derived outputs, 64 authority spans, and 3,290 factual deltas. No policy change is hidden in the propagation.

## R4, R6/I6-prime, R7, and R29

- **R4:** map growth remains optional and default-off. The final data and game/skirmish paths do not silently enable it.
- **R6/I6-prime:** `SPOTS_PER_CELL` remains 3. Occupancy distinguishes one vehicle or ship, up to three infantry, and the separate aircraft layer; real `InfantryTypes` ownership and the corresponding positive/refusal tests are retained.
- **R7:** ordinary source Speed converts as `min(Speed * 256 / 100, 255)` leptons per frame; infantry applies `*3/2`; the simulation conversion rounds the 15-frame distance into 20-tick movement with `*3/4`. Matching jumpjets use explicit `JumpjetSpeed`. The shipped GI, Grizzly, and Harrier values remain 11, 13, and 26. The retained Jumpjet/default and vertical-physics assumptions are explicitly classified rather than claimed as original parity.
- **R29:** the production harness keeps the 72,000 base tick limit scaled by map span over `ARMY_SCALE_SPAN`; the deterministic soak remains a separate 36,000-tick fixed-window protocol, and the duel protocol remains separate. The final evidence preserves those scopes instead of treating Running soak snapshots as all-to-Won proof.

## Final full13, isolated, and raw-evidence review

The final summary and gate process bind the required 13-command run to frozen C9 source. I rehashed all 13 raw logs; every recorded byte count and SHA256 matches, every command exits 0, every per-command source readback reports 689 files / 64 authority spans / zero mismatch, and the final tracked source diff is empty.

Relevant results are:

- workspace release tests: 1,835 primary unfiltered passes, 0 failed/ignored/filtered; the raw aggregate is 1,844 passes because 9 selected child probes are reported separately with 1,348 filtered cases;
- complete RA9 audit target: 185 passed, 0 failed/ignored/filtered;
- complete AI integration target: 146 passed, 0 failed/ignored/filtered;
- production: 756/756 `Won`, with tick distribution 6,659 minimum, 29,448 median, 70,403 p90, 286,896 maximum;
- duels: 756 explicit `Won`, zero non-Won, with tick distribution 6,915 minimum, 29,056 median, 73,899 p90, 263,681 maximum;
- soak: 260 configurations × 2 repeats × 36,000 ticks = 18,720,000 deterministic ticks, 126 completed matches per repeat, and all completed-game guards requiring `Won`; final sampled worlds remain Running as the protocol permits;
- benchmark: both 200-participant scenarios remain under the stated mean and longest-tick budgets, with the exact >=150-alive windows separately reported;
- release and debug 600-frame smokes pass after R208's evidence-only cumulative movement observation fix;
- package creation and packaged 600-frame smoke pass; the binary and archive hashes match the summary;
- generated-data and 4,941-row reference checks pass.

The isolated acceptance receipt is SHA256 `718eccd049ec89f2e97bb0aa2ea5311cf456aba6e05b8e18a437caa93aeda0f0`. I rehashed all five raw logs: all exits and hashes match, all 21 maps end in `Won`, the three original 24,000-schedule windows retain their stated Running scope, and the M13, M14, and M7 example guards each pass 1/0/0. Its runtime evidence commit `05273cc` retains the same C9 manifest and records source unchanged.

The corrected release/package missing-asset evidence also rehashes correctly: each executable exits 1 with the owned demo image absent and exits 0 after restoration; the canonical image remains unchanged and the overlay is restored. This proves the scoped failure/restoration route, not visual quality.

## Debt, provenance, and user-only checks

`docs/RA2_HISTORICAL_DEBT.tsv` remains SHA256 `d0c93c3a6cb828449a0fb859265e40b0987eb85de003688c279c1668987eecbe`: 183 rows, 183 unique ledger IDs, 183 nonblank unique historical-alias cells, no blank required field, and every row still `NOT_ACCEPTED_BY_DOCUMENTATION`. No blanket debt waiver or acceptance was inferred from the passing gate.

The complete HOWTOSEE payload remains 51 rows / 13,855 bytes, SHA256 `8ae4edfadf97fc68138d03c9a12d9c1217cf0c1fb65cf6dd6010c77b1c5bd6c1`. All 51 art, audio, readability, animation, pacing, feel, ordinary human victory/defeat, and packaged/platform checks remain pending user verification.

The added asset-path census remains covered by `assets/LICENSES.md`, and no prohibited original `.ini`, `.mix`, `.csf`, `.shp`, `.vxl`, `.hva`, `.aud`, or `.pal` asset was added. This is path/license-record provenance evidence. It is not a universal authorship theorem or human quality judgment.

## Limits

This review establishes DATA-slice integrity for the frozen source and final evidence record. It does not establish closed-source original-engine behavior beyond the authenticated reference inputs and explicitly reasoned model boundaries. It does not convert `CURRENT_MODEL_NONREAD` into original-engine nonuse, proposals into acceptance, GameOwned values into source comparisons, fixed-duration soak windows into all-to-Won runs, or hashes into semantic approval. I did not perform the 51 user-only visual/audio/feel/platform checks. The fresh lead must incorporate the three retained cross-slice P2 findings and the other four independent slices.

No repository file, ref, index entry, validation output, or process was modified by this reviewer. This report was written only to the existing approved review cache and remains necessary as the final panel record; no new substantial directory was created.
