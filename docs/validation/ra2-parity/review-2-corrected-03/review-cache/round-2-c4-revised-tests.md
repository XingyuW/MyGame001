# Independent final review — tests and validation design

**Source-only result at `437d69e9ca2038384970c86368e690c1c056ef33`: 0 P0, 0 P1, 1 retained P2.** I found no new blocking source defect. This is not a GO decision because the live `review-2-corrected-03` validation output remains incomplete.

## P2 retained

**T2-P2-01 — lost-Hornet regeneration lacks exact boundary precision.**  
Files: `crates/sim/tests/ra4_spawns.rs:402`, `crates/sim/tests/ra9_guard75_readiness.rs`; ledger `SPAWN-01`.

The broad carrier loss/regeneration cases, exact 27-tick launch spacing, missile `Lost(200)` boundary, and returned-aircraft `Reload(100)` boundary provide useful coverage. They still do not assert the lost Hornet’s promised 800-tick regeneration immediately before and exactly at the boundary. Preserve this as documented nonblocking debt.

## Verified

- The frozen manifest has SHA-256 `9df40e2ce6a42fdb0dadd8d20f1295842de69940429bea78e7b975ee854aec26`. All **689/689** frozen inputs match current bytes and hashes; all **64** protected authority spans and **51** HOWTOSEE rows remain bound.
- The debt ledger contains **183 unique IDs**, each with nonempty historical aliases.
- Current source contains **1,833** `#[test]` declarations and **zero** `#[ignore]`.
- Against the programme start `257a169`, no test was removed without a documented migration. The four renamed tests preserve or strengthen their assertions: map look/schema migrations, projectile boundary 32→480 ticks, and combat schema migration.
- The C4 test files preserve their C3 predecessors as exact byte prefixes:
  - `ra8_ai.rs`: 141→146 tests.
  - `m6_production_matches.rs`: 8→9 tests.
- The five Temporal controls at `ra8_ai.rs:13587-13978` use real public commands and native schedules. They prove frozen garrison waiting, recovery after release, alternative-host progress, cargo interrupting existing last stand, blocked sale, alternative sale, and resumed original sale.
- Causal history is genuine: the same five controls failed **0/5** before the correction and passed **5/5** afterward; the whole AI target passed **146/146**. The exact Crossroads Korea Brutal/Iraq Easy seed-0 regression passes its unchanged Won assertion.
- `ai.rs:1006-1010`, `2793-2858`, and `2871-2940` exclude erased buildings only where command admission forbids Deploy/Sell. They retain living cargo, passive capability, queues and normal post-release behavior.
- R6/I6′ remains explicit and covered through actual crossing, hashing, negative-invariant and older-fixture tests; late twins check I6′ and I7 every tick.
- R4 is implemented as map growth default-off at `skirmish.rs:942`, with both settings covered.
- R7 uses the recorded lepton conversion in generator and parity tests, including GI 11 and Grizzly 13.
- R29 remains unchanged: base limit **72,000**, map-span scaling, separate fixed/soak/duel/ten-player limits, and no correction raises a bound.
- The 13-command validator preserves all original commands plus generated-data and reference-table checks. No filtered or historical partial run can satisfy the full gate.

## Evidence still required

Do not conclude validation from the live directory yet. The final decision still needs the completed, source-bound 13-command run, whole unfiltered workspace/audit/AI results, both 756-match sweeps and distributions, benchmark/soak/smokes/package/regeneration checks, asset failure/restoration evidence, and confirmation that the validator process group drained. The fresh lead must retain all 183 debt dispositions and all 51 user-only visual, audio, readability, pacing, feel, human-match and packaged-platform checks.
