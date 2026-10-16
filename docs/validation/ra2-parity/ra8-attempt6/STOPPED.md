# Sixth frozen RA8 attempt stopped after completed phase4 failure

Frozen HEAD e01d7e5; runtime d1aa0f0. Phases1/2/3 exit0; phase4 exits101 in3458s,
366 passed,3 failed,0 ignored. Exact failures are the three original m0_bootstrap RNG tests:
same_seed_consuming_randomness_gives_equal_hashes_at_every_tick,
hash_reflects_consumed_randomness,different_seeds_consuming_randomness_give_different_hashes.
Their injected Ai-stage RNG system conflicts with exclusive coreAi World access under strict
schedule ambiguity detection. No RNG/hash assertion has failed; schedule initialization aborts.
Diagnosis/compatibility fix remains pending; do not reclassify this gate as passed.

Production target6/6 passes in2650.17s with756/756 explicit winners; fresh raw duration table
and production-metrics are retained here. All19 retained exact matches and simlib67 also pass.
The wrapper stops its owned process group only after completed phase4 failure, preserving
partial following05 output. Phases6–13 were not run. Every captured raw byte/hash is retained;
no trim/suppression/overwrite. Full13 must repeat on the corrected committed final source.

Source diagnosis correction: the initial message's "exclusive run_ai" shorthand was inaccurate.
run_ai remains parameter-based. The exclusive system is command::certify_ai_strategic_intents,
chained after run_ai in SimCoreSet(Ai). The raw release error hides names but identifies the
World conflict between coreAi and the injectedAi RNG system. Source signatures identify this
certificate system unambiguously. Preserve this correction alongside the original observation.
