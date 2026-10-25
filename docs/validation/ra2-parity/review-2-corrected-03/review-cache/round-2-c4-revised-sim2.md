# Independent final source review — SIM2

**Frozen source:** `437d69e9ca2038384970c86368e690c1c056ef33`  
**Cumulative range:** `8230a955b251e9341d349f99eaeb86f7f8253664..437d69e9ca2038384970c86368e690c1c056ef33`

## Decision

**No confirmed P0 or P1 source finding remains in the SIM2 slice.**

The SIM2 source is suitable for a **conditional GO WITH DEBT**, subject to completion and independent review of the mandatory native validation. This is a source-only conclusion, not overall programme approval.

I found no new P2 defect. The existing **SPAWN-01 exact lost-carrier/Hornet regeneration-boundary P2** remains open, along with the documented model differences and all 51 user-quality checks.

## Binding and method

I independently reviewed the assigned production paths and their reachable modules:

- `ai.rs`, `ai_defs.rs`, `ai/*.rs`
- `combat.rs`, `combat_defs.rs`, `combat/*.rs`
- `special.rs`, `special/*.rs`, `special/infantry/*.rs`
- `bridges.rs`, `crates.rs`, `crew.rs`, `garrison.rs`, `props.rs`
- `ra8_intents.rs`, `veterancy.rs`, `visibility.rs`, `world_objects.rs`
- The 17 currently assigned RA9 SIM2 native targets
- Shared late-game AI match, source-policy, completion-audit, R4, R7, and R29 interfaces

The live source census matches the current brief:

- 65 simulation Rust files
- 40 game Rust files
- 49 RA9 audit Rust files
- 67 audit TSVs
- 4 audit TXT bindings
- 36 top-level RA9 simulation targets
- 16 reference files

The C4 manifest is present with SHA-256 `9df40e2ce6a42fdb0dadd8d20f1295842de69940429bea78e7b975ee854aec26`, 689 listed inputs and 64 authority fragments. The 183-row debt ledger has 183 unique IDs and 183 unique historical aliases. The user-verification section contains exactly 51 substantive rows.

Commits after source freeze `045b91c` add preserved validation evidence only; there is no production, test, or data difference between `045b91c` and the reviewed commit. The only working-tree entry was the orchestrator-owned, incomplete `docs/validation/ra2-parity/review-2-corrected-03/`, which I did not inspect for conclusions.

## C4 correction assessment

The Temporal AI correction is narrowly placed and internally consistent.

- [`View::erased_buildings`](crates/sim/src/ai.rs:884) records current owned erased hosts. Construction at [`ai.rs:1006`](crates/sim/src/ai.rs:1006) uses current `SpecialState.erasures`.
- Actorless occupied Props are covered because `run_ai` synthesizes their current `BuildingSnap` values at [`ai.rs:1346`](crates/sim/src/ai.rs:1346), so the erasure set is not limited to ECS buildings.
- Ordinary reinforcement sales exclude erased buildings in both sale branches at [`ai.rs:2793`](crates/sim/src/ai.rs:2793) and [`ai.rs:2821`](crates/sim/src/ai.rs:2821).
- Last stand preserves living garrison cargo as recovery capability, chooses another unblocked host when one exists, and waits if every recoverable host is erased at [`ai.rs:2907`](crates/sim/src/ai.rs:2907).
- Last-stand liquidation also excludes erased buildings at [`ai.rs:2928`](crates/sim/src/ai.rs:2928).
- After release, the unchanged host remains eligible for Deploy or sale. No actor identity, ownership, hit points, timer, RNG, cap, or command-limit state is replaced.
- `erased_buildings` is an ephemeral observer view derived from already hashed state. It introduces no mutable unhashed state and does not require an `AiController` hash extension.
- The correction preserves the execution-boundary refusal in the command layer; it prevents repeated invalid admission without weakening the real command validator.

The five added focused bodies in `crates/sim/tests/ra8_ai.rs` cover:

1. Frozen occupied host waits and returns its original infantry.
2. A frozen lower-ID host does not starve another live recovery host.
3. New cargo interrupts an already active last stand.
4. A frozen building is not sold.
5. A live alternative is sold while the frozen building waits, and the original resumes after release.

I found no path by which the new filtering suppresses an available live alternative or permanently strands a released host.

## Prior finding reconciliation

The previously confirmed SIM2 P0/P1 findings remain correctly remediated in source:

| Finding | Source conclusion |
|---|---|
| Hospital healing/unload after erasure | Current identity, HP, owner, and type are reread before passenger mutation or exit. Frozen jobs wait and resume. |
| Pending actor and actorless specialist interactions | Actor identity and commandability are revalidated at execution. Actorless Prop and Occupier paths have equivalent admission boundaries. |
| Voluntary garrison eviction during erasure | Living erased hosts retain cargo and pending voluntary exit; forced cleanup for removed/red hosts remains separate. |
| AI hypothetical building-damage capability | Current target metadata supplies bombability and radiation immunity instead of a false default. |
| Nested AI cargo ownership/capability | Positive-HP descendants of living owned roots participate in memory, cohorts, finance, limits, and cleanup. |
| Stale IFV repair liveness | Current source and target HP are read after parasite damage before repair. |
| Free MCV with a live transported MCV | Recursive living cargo participates in owned base-unit eligibility. |
| Specialist rank/crate damage | Squid and Yuri actor-fired effects use the actual firepower multiplier exactly once. |
| Guard starvation on an ineligible nearest target | Guard selection now uses the real weapon/layer/Verses eligibility boundary. |
| Frozen launcher creating later volley members | Bay launch checks both disabled and erased state at [`flight.rs:718`](crates/sim/src/flight.rs:718). |
| Hidden-terrain AI privilege | AI planning uses immutable setup terrain plus observed sparse changes; mutable observations enter the controller hash. |
| C3 visible-ore finance regression | The current predicate requires a live owned refinery and visible remaining ore, without introducing hidden terrain knowledge or changing caps. |

## Requirements coverage

- **Combat and selectors:** Shared base/elite selection, layer and Verses eligibility, minimum/range boundaries, TypeImmune, bombable/radiation metadata, blast provenance, experience attribution, and off-map actor identity have coherent current consumers.
- **Specialists:** GI, Desolator, Yuri, Spy, Ivan/C4/defuse, Temporal, Tesla/Prism support, Mirage, Drone, Squid, IFV, transports, death payloads, Chronosphere, Curtain, Nuke, Storm, and paradrop paths retain their distinct selection, ownership, source, timing, and refusal boundaries.
- **Visibility/privacy:** Concealment precedes commands, AI and combat; hidden actor identity stays observer-specific; Gap, Satellite, sensor and direct sight remain distinct. Public strategic notices do not themselves grant AI knowledge.
- **Crew/rank:** Thresholds, cost normalization, firepower/reload/speed effects, source-killer separation, crew rounding, deterministic room selection and source-owner behavior remain intact.
- **AI:** Observer-only knowledge, terrain memory, producer/buildability gates, financing, queue intent certificates, naval reachability, country plans, difficulty, recovery, last stand, bounded commands and hashed mutable state remain coherent.
- **R4:** Map growth remains a game option defaulting off; simulation setup consumes the chosen map and does not silently grow it.
- **R7:** The 15-frame to 20-tick conversion and named infantry/Jumpjet assumptions remain consistent. Slower nominal 1× feel stays user verification.
- **R29:** `MATCH_TICK_LIMIT` remains 72,000 with unchanged map-span scaling at `m6_production_matches.rs:47–55`. No source path raises or bypasses the accepted limits.
- **Audit/completion interface:** Physical adapters, typed dispositions and completion atoms remain separated from proposal export. Static/current-model absence does not claim original-engine nonuse or native execution.
- **Determinism:** SIM2 mutable state remains represented in ordered collections or stable vectors and is included in the state hash. I found no new nondeterministic iteration or unhashed gameplay field.

## Retained P2 and debt

### P2 — SPAWN-01 exact lost-carrier regeneration boundary remains open

At [`flight.rs:690`](crates/sim/src/flight.rs:690), both returned-aircraft reload and lost-aircraft regeneration use the same deterministic countdown transition. The source is coherent, and exact 27-tick launch spacing plus the exact returned-carrier reload boundary are represented by tests.

The current exact scheduled addition at [`ra4_spawns.rs:708`](crates/sim/tests/ra4_spawns.rs:708) proves the returned-aircraft `Reloading` boundary. It does not independently pin the full stock Carrier lost-Hornet regeneration boundary immediately before and at readiness. Retain `SPAWN-01` as an evidence/debt item; do not reinterpret it as a production failure or silently close it from the returned-flight control.

Other specific SIM2 debts remain accurately bounded:

- `GATHER-01`: terrain flood does not certify movement through contrived own-building partitions.
- `VHP-01`: no projected-health targeting/reservation model.
- `RETURN-FIRE-01`: original applicability remains unresolved; current explicit order policy is retained.
- `MIRAGE-GATE-01`: source semantics remain uncertain.
- `EXP-SPREAD-01`: death geometry uses selected warhead spread.
- `FIRE-ANIMATION-01`: burning-animation damage is omitted.
- `DEFAULT-DEATH-01`: no runtime default-death-weapon fallback.
- `HARVEST-SOURCE-02`: 19-tick harvest rate remains a documented game-owned assumption.
- `SPAWN-SPACING-02`: retained 27-tick launcher spacing is source-qualified rather than claimed native 10-frame fidelity.
- `REVEAL-SW-01`: coordinate-only public reveal and nine-real-second UI lifetime retain their precise privacy/presentation limits.

None of these grants blanket acceptance for the 1,911 source dispositions.

## Evidence still required

Before the lead may issue the programme decision:

1. Finish the single complete, unchanged 13-command validation run on the reviewed source.
2. Confirm all 185 audit cases together, rather than combining focused or historical subsets.
3. Confirm both 756-match production and duel sweeps end 756/756 with unchanged R29 limits and produce the required before/after distributions.
4. Confirm the 260×2×36,000 deterministic soak, with completed outcomes Won while Running windows remain permitted.
5. Confirm the all-21 ending sweep, fixed three windows, example guards, release/debug smoke, benchmark tail limits, package checks, generated/reference checks and all four asset failure/restoration processes.
6. Bind the final late-twin positive and both expected causal failures/restoration to the final reviewed source.
7. Have all five independent slices inspect the complete final evidence, followed by a fresh lead.
8. Keep all 183 debt IDs and aliases individually reconciled.
9. Leave all 51 visual, audio, readability, animation, pacing, gameplay-feel, human victory/defeat and packaged/platform checks pending for the user.

I did not run native gates, modify files, create artifacts, contact other agents, or consume the incomplete current validation output. The repository remains unchanged apart from the orchestrator-owned untracked validation directory.
