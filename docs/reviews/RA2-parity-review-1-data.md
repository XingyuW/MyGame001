# Independent final review, round 1 — DATA

Reviewed frozen TIP `23647de4220408d42eeef6522deb4d4541528452`, cumulative base `8230a955b251e9341d349f99eaeb86f7f8253664`. Reviewer `/root/review1_data`, fresh independent slice; no implementation/remediation, canonical edits, Git mutations, Cargo, game, benchmark, match, soak, package, native probe, clone or worktree. Common dispatch overrides historical copy/probe instructions. Review completed 2026-10-22 UTC.

**Slice conclusion: NO-GO. New findings: P0 0, P1 1, P2 1.** DATA-P1-01 is an audit/admission integrity gap, independently established by the complete code path below. It is not a finding that the current installed source literals or all current receipts are false. The simulation P0 reported separately and the interrupted mandatory gate are additional programme blockers; I do not count another slice's defect as my independent DATA finding.

## DATA-P1-01 — Enforce accepted identity and receipt-route contracts for the legacy and masked source admissions

**Severity P1: missing required fail-closed source-policy validation and meaningful negative coverage.**

Primary locations:

- [legacy-source-admission.rs:13](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/tests/ra9_audit/legacy-source-admission.rs:13), including the three dispatch branches at lines 15, 25 and 35.
- [scope-inputs.rs:166](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/tests/ra9_audit/scope-inputs.rs:166).
- [masked-speed-admission.rs:147](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/tests/ra9_audit/masked-speed-admission.rs:147).
- [policy-admission.rs:163](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/tests/ra9_audit/policy-admission.rs:163).
- The independently derived binding/guard strings already exist in [source-plans.rs:220](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/tests/ra9_audit/source-plans.rs:220), but that export helper is not used to enforce these three admissions.

The actual complete path is `audit_at` → `load_exact_unused` → `source_routes::validate_row` → `source_plans::admit` → `legacy_source_admission::require` → `ObservedScope::accepted_source` → the selected legacy validator → `a.add_unused` → `source_routes::record` → the final source/physical reconciliation. `validate_row` checks the disposition's source atom/value/class/route/reason, and `accepted_source` checks source key/value/reason, a non-UNACCEPTED decision, and the final input binding. However, `accepted_source` checks `consumer_bindings` and `model_guard_id` only for nonemptiness; it checks `native_receipts` only for nonempty/not-UNRUN. Static116 and Metadata147 then discard the receipt text entirely; Residual117 carries it into a field whose only check is nonemptiness. None compares the accepted consumer/model-guard against the independently derived current identities, and none calls `source_reason` or `require_routes`. Their existing precise loaded/source/Compared-premise checks do not validate these omitted acceptance fields. The final `Audit::finish` and route finish compare atom/value/witness sets, not these fields.

Consequently, keeping the source/value/reason/decision/final binding correct while replacing one of these records' consumer, guard or receipt with a foreign nonempty string passes these admission conditions. **380 atoms** are affected by missing accepted-consumer/model-guard equality: Static116 + Metadata147 + Residual117.

Masked19 does call `source_reason`, which correctly compares its consumer and guard. But `source_reason` only tests receipt nonemptiness/not-UNRUN, and Masked19 ends at that helper without validating the required typed route set or hash shape. Thus **399 atoms** lack the required exact receipt-route check, including the 380 above and Masked19's 19 atoms. This differs from the explicit route-set checks in the paired packages, special admission and legacy361/543 plans.

**Current-valid-row distinction:** I independently recomputed all 399 installed consumer/guard identities and expected route names. Every installed row currently matches; all expected names are `static/exact-source-current-model/<the exact route>`. All eight unique artifact hashes referenced across the acceptance tables resolve and rehash. The finding is the missing audit rejection of stale/foreign/nonempty evidence, not a claim that these current static records are native execution or that their current cells are fabricated. Source/output hash pins detect drift against a particular frozen packet; root factual rehashing is explicitly not semantic policy approval and does not supply this missing admission contract.

Expected correction: enforce independently declared expected binding/guard and exact typed receipt routes before admitting any of these atoms. Do not derive the expected route set from the acceptance cell being validated, or replace checks with a broad root approval string. Keep static evidence static. Add actual accepted-baseline, one-cause negative controls for a nonfirst row of each of the three legacy families and Masked19: foreign consumer, foreign guard, missing route, foreign route, malformed/hashless receipt, and duplicate route. The legacy controls must reach the real admission path and its final reconciliation, rather than testing only a synthetic parser or failing during setup. Preserve the current valid positive records, all source literal/class/reason distinctions and existing controls. No native probe or mutation was necessary for this review conclusion; the complete accepting/rejecting predicates and downstream consumers were read.

Evidence: `data-399-current-records.json`, `data-static-evidence-v2.json`, and the actual source cited above. Actionable finding sent to root promptly.

## DATA-P2-01 — Integrated audit modules retain obsolete UNCOMPILED/UNRUN banners

Examples: [actual.rs:1](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/tests/ra9_audit/actual.rs:1), [source-routes.rs:1](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/tests/ra9_audit/source-routes.rs:1), [scope-inputs.rs:1](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/tests/ra9_audit/scope-inputs.rs:1), and the negative-module headers. These absolute status claims contradict actual canonical bounded compilation/execution in 197c. This is documentation/maintenance debt, overlapping the ledger's retained R56-17 qualification. Update status wording to preserve actual historical provenance and remaining acceptance limits; do not present bounded execution as a full181 or final13 pass. This P2 is not a separate blocker.

## Independent numerical recomputation and generation

My scratch Python parser/calculator imports neither the generator, `ra2_ini`, nor the Rust audit. It independently parses the actual external originals and shipped RON and uses exact rational/integer conversions. Both original hashes match R2:

- rules: `adc869ca037db5141f562f18092d022045e194de0d1204a46b9ec8372b2b826f`.
- art: `d732bfc8ffb6d52908342254c22731acb730ae6d2bbfb41983ef109e48a8f061`.

The final calculation contains **2,965 scalar/flag/conversion checks: 2,913 equal, 52 exact listed differences, zero unexplained mismatches**. Differences require exact game id, field, actual value and original reference value. Twin consumer repetitions are counted as comparisons, not invented distinct fields/types. Coverage includes every mapped unit/building's sampled cost/build-time/HP/sight, speeds, power/refunds, size/ammo/docks/spawn fields; all mapped weapon sampled damage/range/minimum/reload/burst, projectile targeting flags; all61 warheads' spread/falloff and671 Verses entries; production/economy/veterancy/crew/radiation/superweapon/crate globals; miner capacities and actual building Soylent; crate shares/arguments; all three difficulties and their ordered source components. The sample exceeds the required100 and includes RA5/RA6/RA8 values, not merely early RA1 values.

The initial raw recomputation and both later reconciliations are preserved. Initial observations deliberately exposed my generic recipe's invalid associations: building Ammo vs unit Ammo, aircraft docks vs `vehicle_docks`, zero spawn timers, and `weapon.` difference identities. They were reconciled by reading the complete actual branch and source distinctions, not by silently waiving mismatches. Final `data-independent-recompute-v3.json` is authoritative; the earlier JSON remains evidence of that diagnostic process.

All **4,941 committed reference rows** independently equal the authenticated actual INI value after the documented comma normalization, with no missing source row. Read-only `PYTHONDONTWRITEBYTECODE=1 python3 scripts/generate_ra2_data.py --check` exits0 and reproduces all seven committed RON files; `PYTHONDONTWRITEBYTECODE=1 python3 scripts/ra2_reference.py --check` exits0 and reports4941 actual-source rows. These are cheap static checks, not a duplicate native gate or full13 execution.

Every one of the75 difference rows was reviewed. The retained generated Foundation width/height differences are explicit geometry choices, with Image resolution and axes kept distinct from art Height. The remaining rows explicitly name shared Engineer/Dog country restrictions, actual base/elite scanner/collision omissions, shipyard reach31 vs12, the dynamic radiation normalization, bounded negative-range sentinels, spawner target-warhead substitution, Sapper ground-layer adaptation and death-only range placeholder. I found no additional unexplained numeric mismatch. The limited/no-fire/shared-slot premises remain necessary; a listed row is not blanket permission to omit arbitrary weapons or elite siblings.

## R7 conversion judgment and related interfaces

R7 is a defensible replacement for the original GI anchor: once speed is expressed per15-Hz source frame, multiplying by15/20 is the same time-base conversion used for the programme's other rates. The independent calculations reproduce GI11, Grizzly13 and all mapped speed rows with the stated integer flooring and rounding. This preserves the intended rate scale subject to quantization; it does not make rounded fixed-point motion mathematically identical to the original.

I independently checked the ancestor's integer speed conversion in [EA's CCINI.CPP](https://raw.githubusercontent.com/electronicarts/CnC_Remastered_Collection/master/REDALERT/CCINI.CPP), `_Scale_To_256` and `Get_MPHType` (lines235–241 and315–318). That primary source supports the ancestor scaling and the dimensional rationale, not recovered RA2 Jumpjet internals. The3/2 walking factor, direct JumpjetSpeed units, flat/2D movement and omitted terrain factors remain named bounded assumptions/differences; no original-engine motion execution was performed. The continuation explicitly permits a closest documented model when source behavior cannot be established. The slower1x feel stays user-only verification.

R6/I6′: the data's infantry origin is the actual InfantryTypes registry, not game-name guesses; size/capacity/schema10 associations and aliases remain independently compared. Data/schema parity does not establish every spawn/transport/teleport/capture/movement occupancy exit. Those actual schedule/grid obligations remain required at the sim1/tests interfaces and are not accepted from a catalogue count.

R29/caps: no authority to raise72,000 was inferred. The isolated198 all21 proof's400,000 cap and fixed24,000 observations are different configurations and obligations; neither supplies the interrupted756-match gate, soak-to-end evidence, or a new match-limit policy. I grant no current full distribution/performance credit from a stale production-length cache.

## Schema, loader and fixture audit

Current schemas are units1, economy2, combat5, arsenal10, skirmish3, ai3, rules4. All51 fixture RON files have the corresponding current version, including the separately named `bridge_ai.ron` schema3 and the named units/economy variants. The generator emits those exact current versions. No obsolete positive fixture version was found.

The actual loader path uses bounded regular opened-handle reading, a strict pre-RON supported-subset checker, exact required field sets, typed UnsupportedSchemaVersion/MissingField/UnknownField/DuplicateField errors with paths, and range/reference validation. The deserialization records use `deny_unknown_fields` and have no serde-default escape. Old schema versions fail the checker's explicit SchemaVersion shape before a third-party parse. Unit/economy/combat/rules/arsenal/skirmish/AI paths validate their references and ordered/set distinctions; armour rows require every class exactly once, negative repair damage has its separate bounded branch, and invalid range/minimum/rank/flag/capacity identities are not silently defaulted. `load_ruleset` uses the documented units→economy→combat→rules→arsenal order; reloading economy invalidates dependent definitions instead of pretending old cross references remain valid.

Current arsenal size is **175,050 bytes**, below the explicit192KiB bound (196,608), with no need to raise it. Raw unknown/missing fields cannot become automatically GameOwned: the independent fixed audit schemas and raw entry enumeration require exact fields. Raw country roster empty-list constraints are schema branch observations; the current country roster comparison derives actual loaded membership independently and does not fabricate an empty loaded Vec as a witness.

The five original named `ra9_data_parity.rs` tests remain. New adapters/negative files supplement them. Meaningful negatives first require an unchanged real loader/adapter baseline, then require the intended comparator failure rather than crediting fixture or parser failure. In particular, the retained current six repaired bodies' canonical197c results are6/0/175filtered; this is not another full181 run. DATA-P1-01 identifies the additional missing acceptance-field negatives.

## Full audit graph, source atoms, policy choices and receipts

Independent graph enumeration finds **48/48 audit Rust files reachable**,79 include paths,0 missing/unreached;67 TSV and4 TXT audit inputs;16 reference files. Historical `.orig/.rej` files are not compiled modules. All67 tables/four bindings are included in the actual structural/input audit; hashes or filenames alone confer no acceptance.

I independently rehashed all **656 frozen source/input files**,97 production Rust files,64 exact authority spans and the13 materialized binding outputs: zero mismatches. Root binding is the actual195 packet hash `9c110cf4d961ac33b5025acb53d0f50eedfa3fe55fe76e9e2135789d388ba6b7`. The complete production Rust census has no unrecorded/missing file; all per-file SHA/FNV/byte checks and observation digests agree. These are factual scope checks, never policy approval.

I reconstructed source-side observations from a separate actual-INI parser: guard75's216 facts, special's36 facts, and all131 legacy family section/incoming closure hashes byte-match the committed observations. F146/F147 each have105 sections/361 incoming references and the same independently reproduced closure; F251's complete positive eligibility census is exactly AMMOCRAT/CAMISC01/CAMISC02 and its closure matches. No expected-value table was used as an observed original source.

The full source ledger is **5,022 atoms**, obtained from4,941 rows plus the explicitly shaped19 Powerups tuples and17 three-component difficulty arrays. The actual canonical bounded whole-positive log197c reports16,211 physical witnesses =13,419 Compared +2,792 GameOwned,7,108 independently demanded/successful dependency pairs,3,111 Compared source atoms +1,911 dispositions,75 used differences. Reading a source, a successful Own value, an alias, or export does not create Compared coverage. The core finish checks exact physical witness equality, successful/demanded dependency equality, correct source shape/value, no Compared/disposition overlap, complete atom coverage and no stale difference.

Every one of the **1,911 disposition atoms** was independently checked for exact actual source value, identity/portion uniqueness, supported class and nonempty specific reason/evidence; all current rows match. Counts are1,869 Whole +19 component1 +19 component2 +4 component3, across1,888 source keys. Classes:759 CURRENT_MODEL_NONREAD and1,152 INTENTIONAL_DIFFERENCE, **zero ORIGINAL_ENGINE_UNUSED**. The route counts are recorded below and every row receives an independent scope/limitation assessment in `data-disposition-assessment.tsv`.

| Route | Atoms | Independent judgment / limit |
|---|---:|---|
| static116 |116| Exact current Height/instant/jumpjet boundary; original engine nonuse not inferred. DATA-P1-01 identity/route gap. |
| metadata147 |147| Explicit current Points/presentation/IFV/visual boundaries; actual Cost/weapon/radiation/SW consumers separate. DATA-P1-01. |
| residual117 |117| Exact turn/turret/electric/fire/light model choices; no replacement of actual damage controls. DATA-P1-01. |
| masked19 |19| Exact Inviso/arcing branch masks nominal operands; Range/Gravity/actual speed remain genuine comparisons. DATA-P1-01 receipt-route gap only. |
| consumer62 |62| Source-literal/current consumer distinctions retained; paid UnitReload health-after-ammo and same-Economy ownership/liveness remain genuine native obligations. |
| carry38 |38| Specific current difficulty/power/acquisition policy; ignored options do not borrow active multipliers' source credit. |
| model162 |162| Explicit movement/2D/timing/lobby/presentation boundaries;33 presentation/29 absence migrations are static, not execution proof. |
| guard75 |75| Reconstructed original graphs plus exact current selector/subsystem boundaries; current absence does not prove original inactivity. |
| behavior87 |87| Actual selectability/commandability/target/concealment models require each real consumer; no generic fake flag getter. |
| completion174 |174|132 Whole +42 precise components over151keys;81 active registry ordinals and21 unit Soylent omissions stay distinct from infantry kind, building refunds and Cost XP. |
| legacy-ai361 |361|69 exact families of authored paid country-plan/known-vision policy; source multipliers and production comparisons remain separate. |
| legacy-model543 |543|62 exact specific helper/geometry/absence/omission families; false/zero is never original nonuse or generic true/default semantics. |
| spark / vhp / tree / mirage-elite |7 /1 /1 /1| Exact Spark absence premises, declared Patriot targeting difference, contextual tree exclusion, and primary-first elite shadow remain separate original/current/native/client claims. |

The per-atom choices are specific current-model declarations, not proof of every original mechanic. I accept their truthful classification distinctions and their explicit limited-model explanations as reviewable proposals; I do not turn their existence or root rebinding into final native/source-policy acceptance. In particular, original UnitReload stays unparsed; actual AirportBound/Ammo/Dock/NumberOfDocks, paid health and ownership controls cannot be replaced by a loaded pad snapshot. Original unit Soylent is not a building refund or Cost kill-XP input. Art Height is not Foundation height. Crate animation/water/argument siblings do not inherit component0 weight coverage. F146/F147 helper source absence and F251 eligibility census are source boundaries, not native fallback or generic delayed-kill execution.

All eight unique qualified artifact hashes referenced across acceptance tables resolve and rehash. I also rehashed the88 historical command receipts and their88 raw logs:176 files match. Six original-cache-only logs (five zero-byte logs and the69-byte93 reference log) remain present and exact in the preserved integration-validation cache; no missing-evidence finding is raised for them. That historical cache remains necessary evidence. The actual linked packets explicitly retain mixed command failures, named-pass limits and old-source scope; a named pass inside an exit101 run is not a target pass. The174 packet has43 routes,17 loaded route labels and9 qualified native common-path labels, not174 independent native simulations. Static33 presentation and29 model-absence inputs establish inspected present implementation boundaries, not user visual quality or original engine nonuse. Old native receipts remain old-source observations even when current factual fingerprints are propagated.

## Historical debt and cross-slice obligations

All183 unique debt IDs and their aliases/remaining obligations were inspected; none is closed merely because the ledger says a test exists or proposes P2. `data-debt-assessment.tsv` records every ID, identifies32 primary DATA rows and keeps the remaining connected interfaces explicit. GENERATEDRULES-01/R56-11/R56-14 carry DATA-P1-01. The current byte bound, fixture versions, exact source rows, actual building Soylent and enumerated Foundation mismatches are supported as stated. R56-17 carries the stale banner P2. Harvest19 ticks, launcher27-tick spacing, native default-death absence, return-fire uncertainty, omitted animation damage, ExpSpread geometry and Mirage shadowing retain their exact assumptions/limits; no false/default/original-unused promotion is granted.

For live gameplay, command/privacy/liveness, I6′, recursive cargo, hidden actors, paid servicing, spawn timers, user control, source-dependent damage and late AI activity, the data ledger supplies configuration and necessary comparisons only. Actual scheduled/native/client controls and final execution remain required. The root-reported IFV/parasitic liveness P0 demonstrates why data equality cannot stand in for those controls. I found no basis to downgrade an actual invariant/result defect or missing required gate to P2 because a source-policy disposition exists. The51 HOWTOSEE visual/audio/readability/animation/pacing/feel/human victory-defeat/package-platform checks remain user-only and are not review findings.

## Provenance and assets

All176 asset files added in the cumulative range are individually named in `assets/LICENSES.md`. Generator sources for their original geometry, textures, maps and in-memory audio are present. I did not run Blender or regenerate/compare the media pixels; source/licence inventory is distinct from reproducing those binary outputs or approving quality.

The cumulative Git object inventory has6,908 objects. Neither complete authenticated original INI blob is present, and no original-format `.ini/.mix/.csf/.shp/.vxl/.hva/.aud` path appears. A read-only history scan checked5,209 text blobs,400,262,057 bytes, against504 distinctive original INI comment lines: zero verbatim hits, including the four large historical logs; no large text blob remains excluded from that scan. The committed numerical reference values have no copied display text: all actual4941 rows match the permitted numeric/flag/identifier source extraction and its whitelist/shape logic.

I investigated the appendix's original-name catalogue separately. Requested unit/building names and source identifiers/key=value provenance are intentionally part of this programme; ordinary short names matching Name fields are not evidence of imported descriptive INI prose. The distinctive-comment/full-file/history checks found no corresponding import. I therefore do not raise a speculative provenance P1 from those names. This is not a claim to detect all paraphrases, arbitrary short strings, or hidden media ancestry, and not a legal/media similarity determination.

## Actual validation status and unverified work

The shared199 gate was launched on the frozen TIP with source guards. It completed commands01–03 exit0. It was deliberately interrupted by root at00:40:25UTC during command04's production sweep after the separately reported simulation P0. The original controller statuses were stale; `gate-process.json` RUNNING is not an active/passing result. `round-1-live-evidence.md`, the actual stop receipt and post-stop669-file/64-span check preserve this chronology. **No full13 pass occurred; no full181 current audit-target pass occurred in that interrupted gate; later native/build/smoke/benchmark/soak/duel/package commands were not reached.** My cheap source regeneration checks do not complete or relabel the interrupted gate.

Canonical197c supplied a bounded whole-positive1/0/180filtered, six repaired6/0/175filtered and the separate difficulty1/0/133filtered. Actual hashes/raw result lines match. Earlier158 whole-positive,159's175pass/6fail unfiltered181 and168's six repaired passes retain their historical exact scopes. They are not combined into a whole181 final pass. Isolated198 all21/fixed3/three guards retain their earlier same-runtime-input scope, with no blanket performance/final-gate approval.

The code-path integrity finding requires remediation and actual accepted-baseline negative execution. All mandated corrected-source native validation remains required. As root clarified during this review, permission to share one corrected-source full13 between RA9/review gates is pending; urgency is not a waiver, and the interrupted199 supplies neither complete gate. Without explicit authorization, the original separate-run requirement remains. No metadata closure can turn pending commands or old-source route receipts into a current pass.

Unverified within/at the DATA boundary: complete current181 audit/negative execution; corrected399 acceptance rejection controls; final native premises for every policy requiring them; full required gates and final post-remediation source/route rebinding; exact original engine behavior for the explicitly bounded models; binary media regeneration/authenticity beyond generator/provenance inspection; user-only visual/audio/feel/platform checks. These limits are stated rather than converted into PASS. A fresh lead must combine all five independent slices and actual final validation.

## Retained review artifacts and storage

Authoritative local review artifacts alongside this report:

- `data-independent-recompute.py`, initial JSON, v2 JSON and final **v3 JSON** (2,965 checks plus all4,941 source rows).
- `data-static-evidence.py`, initial JSON and final **v2 JSON** (scope/graph/dispositions/receipts/results/assets/debt).
- `data-original-closures.py/.json` (216/36 original facts and131 legacy closure hashes).
- `data-provenance-history.py`, initial JSON and final **v2 JSON** (complete selected text-history scan).
- `data-399-current-records.json` (current correct cells versus missing admission enforcement).
- `data-disposition-assessment.tsv` (all1,911 atoms), `data-debt-assessment.tsv` (all183 IDs).

Initial scratch diagnostics were preserved, including corrected reviewer recipe/count/lookup versions; no failures were erased. I created no substantial directory outside the canonical repository, no checkout/build/venv/package cache/dataset, and nothing under `/tmp` or `/private/tmp`. Only these small analysis/report files were written in the supplied, pre-existing approved reviews directory. They remain necessary review evidence. The pre-existing integration-validation cache also remains necessary for the original-only logs described above. I deleted no repository, worktree or evidence.
