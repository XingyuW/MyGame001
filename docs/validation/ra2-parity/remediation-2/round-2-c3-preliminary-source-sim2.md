# SIM2 — preliminary C3 full cumulative source review

**Preliminary source assessment: no newly identified production defect in the proposed refinery correction. R201's actual Liberty Bay non-Won remains a blocking functional result until an exact corrected-source replay succeeds; shared P1 S2R2-06 full acceptance also remains OPEN. This is neither a final clean binding nor approval.** Initial/C2/partial reports are retained unchanged. Exact lost-Hornet boundary P2 remains retained.

## Entire cumulative scope and exact provisional binding

I remain the independent nonimplementing SIM2 reviewer. This preliminary examination covers the entire cumulative8230a955b251e9341d349f99eaeb86f7f8253664..current candidate using authenticated retained full-scope source review plus inspection of all revised paths/interfaces, not changed-lines-only approval. Observed supplied HEAD is53a5225c272ff45b7d29d124befc6b1d26c5c832 with exactly three prospective source changes. The preliminary packet SHA independently matches `74bfdc0b942caa065f08a8f58ce8270cdbd8bcdf80c6e683728abac35c8b65f2`.

Independent readback of all689 actual inputs matches this preliminary packet. Against C2 manifest SHA0e2edf2bce2a8e41581c382a923dce7a5057ccf0231256903d314acb21261b99,686 inputs are byte-identical and only these3 differ:

| Path | Current SHA256 |
|---|---|
| crates/sim/src/ai.rs | 32e2f224eb66136803ce40c27a886d7346b00026cda4b9f9c8c5aa02abdbd2f8 |
| crates/sim/tests/ra8_ai.rs | 2472e56bff3ff4ee6b048132619cc71d848dc4a281012930b4417358f69f851b |
| crates/game/tests/m6_production_matches.rs | f9d3de4abf4fa787c189b03a32ee7a4436fe7b4e7e6338cc424b8e6abdb84b7b |

All64 protected authority spans independently rehash exactly.51 HOWTOSEE rows retain13855bytes/SHA8ae4edfadf97fc68138d03c9a12d9c1217cf0c1fb65cf6dd6010c77b1c5bd6c1. The183-ID ledger and retained production source are unchanged. The15 derived outputs remain historical C2 intentionally; I do not interpret their old factual bindings as current C3 semantic/native acceptance. Final producer/rebinding/root verification/freeze remain required after the implementer's exclusive lease.

My initial full cumulative examination and C2 revised report retain combat/selector/damage/warhead/origin equations; RA4rank/crate/elite/crew; recursive cargo/crates; all specialist/Prop/bridge/strategic/radiation paths and more-than12 deep RA6 table samples; RA7visibility/Guard/privacy; RA8observer/producer/queue/economy/wave/naval/service/last-stand/state hashes; RA9native/source-policy/completion consumers. Authenticated unchanged source preserves that coverage. All old P0/P1 production corrections—Hospital, pending actor/actorless Interact, retained voluntary garrison exit, current Purifier/Cloner, IFV, FreeMCV, firepower, Guard/occupied Prop, bays, Sell/Undeploy, AI metadata/recursive cargo/hidden terrain—remain installed byte-exact. No new timer/census patch was applied.

## Current production change and all three consumers

`refinery_has_visible_ore` at ai.rs:2508–2517 is a pure predicate: some `own_kinds` building definition has `credits_per_ore`, AND existing genuinely visible ore exists. The actual run_ai snapshot filters positive current HP before View creation (1322/1362). `own_kinds` (1066) filters actual owner==player and economy=true; hostile/allied refineries and garrison surrogates cannot grant eligibility. This is a living owned-refinery premise, not a lookup of hidden enemy state. It remains a conservative income-potential predicate, not proof a worker can reach every visible field or a disabled/erased refinery can pay immediately.

1. `run_ai` last-resort admission (1417–1433): a Rules-difficulty armed Idle worker, absent service income, may join combat when visible ore has no owned receiving refinery. Existing worker/armed/Idle/service conditions and ordinary command admission remain. No forged harvest/load, new attack potency or forced retirement is added.
2. `funding_force` (2595–2603): the armed-worker branch no longer treats bare visible ore as a funded economy without a refinery. The ordinary nonworker force, RulesNone/service-income alternatives and understrength decision remain intact. This is also used by funding-latch release, preserving the same predicate when avoiding construction reclaim of a refund.
3. `understrength_funding` (2605 onward): visible ore vetoes optional-asset financing only with an owned refinery and retained worker/stowed actor premise. Existing service income, recoverable service, air-force protection, real stationary engagement/prior-wave history, recursive living cohort and shipped wave-size threshold remain. Bare ore cannot indefinitely block replacing an understrength real fighting force.

No new controller state/hash field, timer, threshold, cap, difficulty multiplier, worker count or terrain privilege appears. The hidden-terrain `TerrainKnowledge`, real bombable metadata and recursive living-owned cargo corrections remain unchanged. Their model/performance qualifications remain those of my C2 report, not newly universal fidelity claims.

## Native controls and M6 test integrity

The old139-case `ra8_ai.rs` is an **exact byte-prefix** of the current141-case file: all original139 names/assertions/helpers are retained. The two additions share a real scenario with authored initial ore, ordinary harvesting to actual capacity (no fixture load), visible remaining ore, zero initial credits, a live original fighter and an optional depot asset. The no-refinery arm proves the full worker Idle and no native payment, then requires one actual400refund sale to pay ordinary production and a newly spawned recruit's real building shot. The refinery arm adds a real owned refinery and requires genuine OreDelivered, paid production/new recruit shot, zero optional sale, and surviving original fighter. Public Attack/normal scheduling and actual events are asserted; direct fixture building/unit spawning is setup rather than a paid original-creation claim.

I inspected final same-body preimages and raw logs: both BEFORE06 and AFTER01 test files SHA2472e56… match current exactly. BEFORE06 actual exit101: refinery positive1pass; no-refinery negative1fail (saleNone/delivered0/no charge/no recruit shot). AFTER01 actual exit0: both pass/139filtered; no-refinery saleSome400/delivered0/charged/paid/recruit shot, refinery saleNone/delivered1/charged/paid/recruit shot. Both raw SHA values match process receipts. Earlier setup/compiler failures are preserved, not reclassified as useful functional failure.

M6 changes add opt-in Liberty diagnostic printing, stalled-state printing and an exact seed-zero FranceMedium/GermanyMedium Liberty test using existing `duel`. The shared setup/seed/original288000 limit/Won predicate remain. Diagnostics inspect/print state and existing events; they do not change AI/world/RNG, actor counts or outcome. The formatting unwraps are diagnostic assumptions under an opt-in test environment, not production behavior. No completed corrected exact replay or whole141-suite result has been credited at this preliminary ingestion.

## Actual failure and remaining obligations

R201 preserves the real review-2 seed0LibertyFranceMedium/GermanyMedium None at288000/288000 and incomplete144/756 progress; commands01–03passed,04interrupted/failed,05–13notrun. I do not infer completed workspace/185/139/756/full13 from those logs. Its actual non-Won is blocking functional acceptance, not ordinary P2 debt. The prior census/timer explanation is retracted: the reported final real unit1415 dies287834; no evidence supports inventing a power-plant timer reset. The proposed ore/refinery correction has a demonstrated local causal control, but whether it repairs the exact failed match remains pending. No guaranteed winner or generic original-engine behavior is inferred.

A formal clean corrected-source freeze, actual exact replay, all141 together, producer/rebinding integrity, original complete gates and updated final revised/native addenda must precede the fresh independent lead. Earlier isolated all21/Running windows and timing tails retain their precise C2 scope and cannot substitute for this two-player/current-source acceptance. All183 debts remain individually scoped; lost-Hornet800 precision and GATHER/model/performance limitations remain; all51 quality checks remain user-only pending. I supply no overall GO or blanket semantic ledger acceptance.

This work used read-only source/hash/receipt inspection within the remaining cumulative active review bound; no artificial idle/outage time is counted and no precise active duration is invented. No Cargo/native/process/Git/helper/canonical edits were performed. Only this new assigned report was written in the existing approved cache; no substantial directory, clone/worktree/build or temporary artifact was created. Both projects and the implementer's exclusive native lease remain untouched.
