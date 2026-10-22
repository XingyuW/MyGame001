# Independent final panel, round 2 — DATA

Reviewed frozen TIP **1cedaee83b9cb299869a581d13a25e2736c8c41f**, cumulative base **8230a955b251e9341d349f99eaeb86f7f8253664**. Reviewer /root/review2_data is fresh and independent: no implementation, remediation or preparation role. Initial report completed 2026-10-22 UTC. This report covers the complete cumulative RA0–RA9 data pipeline, rather than only the panel1 correction.

**Initial DATA result: no new confirmed P0 or P1; one P2. Panel1 DATA-P1-01 is corrected at the audited identity/route boundary. Final acceptance remains OPEN, with no GO supplied: the unchanged complete185 audit and full13 have not passed on this TIP.** Other slices' confirmed runtime/private-view findings are not duplicated as independently discovered DATA findings. Root's decision to avoid starting the doomed full13 is a scheduling fact, not a waived gate or a completed lead decision.

## Findings and prior finding disposition

### DATA2-P2-01 — Reference README still describes the pre-completion 1,737-row compositor

Location: **crates/sim/tests/reference/README.md:96–109**, especially lines101–103.

The operative instructions describe composition of the installed packages but omit completion174 and say the result has1,737 whole-source rows. Current scripts/ra2_reference.py --ra9-dispositions --check exits0 and actually reports **1,911 rows; no admission granted**. Independently counted current rows are **1,869 Whole +19 Component(1) +19 Component(2) +4 Component(3)** across1,888 SourceKeys. Completion owns174 atoms across151 keys:132 Whole and42 components. The earlier1,737 were all Whole; the current programme has added the exact174 rather than replacing the earlier contracts.

Expected correction: update the current command description/count and include completion174, or label the old paragraph explicitly historical and add the actual current result. Preserve proposal/admission/original-engine/native distinctions and the mandatory gate limitations. This is nonblocking documentation debt. I made no correction.

### Panel1 DATA-P1-01 — Corrected, subject to the final unchanged gate

I read the whole actual path: audit_at_with_source_acceptance → Inputs/load_ruleset/load_audit → independent plans/routes declaration → physical adapters/fire observations → load_exact_unused → source_plans::admit → source-specific guard → add_unused/route record → every plan finish/route finish/Audit::finish.

At **source-plans.rs:284–307**, Static116/Metadata147/Residual117 now independently construct the expected current consumer and guard from the route, SourceKey and observed model/original scope. They call source_reason for exact equality and require their exact typed route sets. At **source-plans.rs:265–273**, Masked19 retains its real selector/source guard and now requires exactly static/exact-source-current-model/masked19. scope-inputs.rs::require_routes rejects absent, foreign, hashless/malformed and duplicate route tokens; it checks64 hexadecimal hash shape. Actual artifact authentication remains the separately required external factual check, not an implication of syntactic hash acceptance.

The replacement acceptance is immutable and local to one synchronous audit invocation and exact SourceKey (source-plans.rs:311–320). It does not mutate the installed OnceLock index or leak to parallel tests. The new single-source-acceptance-negatives.rs controls use the second real disposition of each family, run the complete unchanged accepted baseline before any mutation, replace one cause, require the intended source-row/predicate failure, and run a later complete positive. Their four selected identities are art:CABHUT:Height, rules:AEGIS:Points, rules:AAHeatSeeker2:ROT and rules:105mmE:Speed.

Raw **B-focused-03/01-audit-four-whole-path-after.log** contains exactly4passed/0failed/0ignored/181filtered, **28 intended rejections**,0escaped/wrong failures and **8 RA9_AUDIT_SUCCESS** complete positives. Every positive reports16,211 physical entries =13,419Compared +2,792GameOwned;7,108 demanded/successful dependency pairs;3,111Compared source atoms +1,911 dispositions =5,022 reference atoms;75 used differences. The focused before/after680-path snapshots are equal, and current corrected-frozen inputs authenticate independently below.

I independently reconstructed all **399** installed consumer/guard/route identities: all match. This corrects the prior380 missing identity/guard comparisons and399 missing exact-route checks. It does not establish that every original omitted mechanic is faithful, every historical receipt is current-source native execution, or all185 tests passed.

Panel1 DATA-P2-01's absolute UNCOMPILED/UNRUN headers were generally qualified as historical preparation/adapter notes. Some old draft language remains in helper comments, but I found no new blocking source admission from those comments. The precise new stale README finding is above.

## Fresh numerical and source recomputation

An inline Python parser/calculator, written for this review and executed through stdin, imported **neither the generator, ra2_ini nor any Rust audit**. It independently parsed the actual external INI files (case-insensitive section/key lookup, repeated sections merged, last key wins, semicolon comments removed), parsed the generated RON with a separate recursive token parser, and used Fraction/integer arithmetic with explicit half-up positive rounding. It wrote no script, validation copy, data file or cache.

Actual external hashes match R2 exactly:

- rules.ini: adc869ca037db5141f562f18092d022045e194de0d1204a46b9ec8372b2b826f.
- art.ini: d732bfc8ffb6d52908342254c22731acb730ae6d2bbfb41983ef109e48a8f061.

**Final independent sample:2,655 checks,2,603 equal,52 exact listed differences,0 unexplained mismatches.** These are comparisons, retaining mapped twins; they are not2,655 distinct source keys. Coverage:

| Comparison group | Count |
|---|---:|
| Cost / build time |102 /102|
| Hit points and sight |204|
| Movement speed |56|
| Present flags / aircraft and cargo values / spawn times |111 /60 /8|
| Building power / ordinary sale refund / death modifier |92 /46 /1|
| Verses components / spread and falloff / present warhead flags |649 /118 /59|
| Weapon damage/range/minimum/reload/burst / projectile AA/AG |580 /232|
| Production/economy/veterancy/crew/combat/radiation globals |7 /16 /8 /2 /20 /5|
| Superweapon/prism/crate/dialog globals |17 /5 /5 /16|
| Active crate share/argument values / miner storage / fixed Soylent refunds |16 /2 /3|
| All three difficulties' active multipliers and ordered source components |21|
| Resolved Image/Foundation axes |92|

BombDisarm and IvanBomb omit original Verses: the649-source-component count deliberately excludes their22 default components; no absent source key was manufactured. Foundation uses the resolved art Image section and width/height axes; art Height is a separate presentation operand.

The initial diagnostic calculator exposed recipe mistakes before reconciliation: scalar percentages versus percentages already divided by100, game minutes versus frames, raw negative range sentinels versus converted subcells, and exact difference-table reference formatting (no versus0). I corrected these by reading the full documented/actual conversion branches. Final52 unequal comparisons are47 exact Foundation differences plus ASW/MakeupKit range sentinels, ASW/Sapper ground flags and the death-only OilExplosion range placeholder. No generic mismatch exception was introduced.

All **4,941 committed reference rows** independently equal actual authenticated source values after documented comma normalization; no missing source row. Three read-only commands executed in this review, all exit0:

1. PYTHONDONTWRITEBYTECODE=1 python3 scripts/generate_ra2_data.py --check.
2. PYTHONDONTWRITEBYTECODE=1 python3 scripts/ra2_reference.py --check —4,941 actual-source rows.
3. PYTHONDONTWRITEBYTECODE=1 python3 scripts/ra2_reference.py --ra9-dispositions --check —1,911 exact proposed rows.

I read all75 difference records. The full inventory is47 Foundation axes,6base and6elite weapon-list differences,4shared-country restrictions,4launcher warhead substitutions,3range placeholders/sentinels,2shipyard adjacency choices,2ground-target adaptations and1dynamic radiation base. These are explicit scoped model differences with concrete reasons. Elite fallback retains source selector/ordered-slot conditions; unused scanners and spawned-only collision slots cannot authorize arbitrary weapon omission. Shipyard reach31 versus12 remains the recorded map-topology accommodation. A listed difference does not waive defects in an implemented runtime path.

## Equations, assumptions and interfaces

R7 is dimensionally justified **conditional on its documented source-rate assumptions**: after min(Speed×256 div100,255), the15-frame to20-tick conversion multiplies movement by3/4, reciprocal to the4/3 time conversion. The independent values include GI11, Grizzly13 and Harrier26. The3/2 walking multiplier, direct JumpjetSpeed units, constant2D movement, omitted terrain/slopes and quantization remain explicitly bounded assumptions/differences. I did not recover or run the closed original movement engine; the slower1x feel remains user-only verification. No claim of exact original Jumpjet physics or unparsed original-key nonuse follows.

Production uses Cost×BuildSpeed×1200/1000 with explicit wall coefficient and minimum1; the active difficulty ROF/BuildTime components were checked independently. The graded shortage function, ten-factory bound, five-tick burst gap, arcing/scatter approximation,12density stages, one-bail/19work ticks, spread-above-six, instant unload, survivor/volley/bay timing choices remain separately named model assumptions. These are not proof of original engine equivalence; actual implementation paths and specified boundary tests remain obligations at the SIM1/SIM2/TESTS interfaces.

R6/I6′ data ownership uses actual InfantryTypes membership, not game-name guesses; schema10 Size/SizeLimit, immediate-child capacity and aliases are distinct from cell occupancy. My schema/data checks do not establish all runtime movement/spawn/unload/teleport/capture exits.

R29's72,000 shipped-test cap remains unchanged. The isolated all21 benchmark's400,000cap and fixed24,000schedule observations are distinct protocols; neither permits raising the shipped test cap or replaces the full756matches/duels/soak. R56's36,000-tick paired soak windows may remain Running; completed results still require Won. No user-facing feel or whole-suite performance credit is inferred from these scoped observations.

## Schemas, fixtures and audit structure

Current schemas are units1, economy2, combat5, arsenal10, rules4, skirmish3, ai3. The generator emits those versions. Independently enumerated **51 fixture RON files** all have the current schema for their actual purpose, including named units/economy variants and bridge_ai.ron. Current arsenal size is175,050bytes, below192KiB (196,608); no bound increase is needed.

I traced the shared bounded opened-handle read and strict supported-subset checker (data.rs), unit/economy/combat/rules/arsenal loaders, skirmish and AI cross-validation. The pre-parser rejects unsupported grammar, duplicate/unknown/missing fields, excess list counts and old SchemaVersion with typed file/field context. Deserialize records deny unknown fields; no serde-default escape is used. Validators range-check values before narrowing and resolve source IDs, armour/warhead/weapon references, every armour's unique Verses entry, prerequisites/cycles, producer/buildability/country relations, required elite slots, repair-only negative damage, spawner/missile links, ordered IFV slots and difficulty arrays. load_ruleset loads units→economy→combat→rules→arsenal; its dependent rebuild/reset boundaries were inspected. I did not execute malformed-file native probes.

Fresh actual source graph: **49/49 audit Rust files reachable**,79 distinct include_str paths (106 occurrences),0missing includes, **67TSV +4TXT bindings**,16reference files. Every67table/fourbinding is referenced; every TSV has a unique header and correct row widths. Historical .orig/.rej residues are not active Rust modules. The five original named ra9_data_parity tests remain.

Coverage includes core/raw/schema/actual/items/combat/rules/owned/native/specialist/schema10 adapters; array and nested ledgers/projections/negatives; country rosters; fire-key observations/audit; source routes/plans/policy/scope; Static116/Metadata147/Residual117; paired consumer/carry/model/guard/behavior declarations and admissions; legacy361/543 and support/loaded/original-scope negatives; masked/special source admissions; completion and completion-whole; and the new single-source whole-path controls. The review followed the actual graph and each family boundary, rather than crediting an unreferenced master table.

Audit::finish requires exact physical-witness equality, exact independently demanded/successful dependency-pair equality, valid source shape/value, no Compared/disposition overlap, total source-atom closure and no stale unused difference. An adapter's read, failed comparison, alias or GameOwned value does not create source credit. The separate arrays ledger preserves ordered versus unique-set semantics, exact lengths/elements and source/component/physical-origin associations, and adds no global source credit. Accepted-baseline negatives inspect actual loaded paths before their single controlled failure. Their retained executed scopes remain bounded; I ran no mutation probe.

## Full source-policy census and semantic limits

Independently checked all **1,911 disposition atoms** for source identity, exact literal/portion, uniqueness, permitted class and nonempty specific reason/evidence. All match;1,869Whole +42components across1,888SourceKeys. Classes remain **759 CURRENT_MODEL_NONREAD /1,152 INTENTIONAL_DIFFERENCE /0 ORIGINAL_ENGINE_UNUSED**.

| Route | Atoms | Reviewed distinction retained |
|---|---:|---|
| static116 |116|Exact Height/instant/Jumpjet model premises and current consumer identity; no original nonuse claim.|
| metadata147 |147|Points/presentation/IFV/visual scopes remain distinct from active Cost, weapon, radiation and SW consumers.|
| residual117 |117|Specific turret/turn/electric/fire/light policies and exact active neighboring comparisons.|
| masked19 |19|Inviso/arcing masks nominal Speed only in its declared branch; Range/Gravity and actual projectile motion remain independent.|
| consumer62 |62|Parsed fields versus exact unparsed/selective interface choices; genuine pad/repair/native demands retained.|
| carry38 |38|Ignored difficulty/power/acquisition options do not borrow active multipliers' Compared credit.|
| model162 |162|2D/timing/lobby/presentation boundaries;33presentation/29MODEL-ABSENCE routes remain static evidence.|
| guard75 |75|Actual original/root/selector closure versus current exact absence or model choice.|
| behavior87 |87|Current source/consumer contracts for targetability, commandability, concealment and client controls; runtime/private-view correctness remains independently required.|
| completion174 |174|132Whole +42ordered components,81registry nonreads and21unit-Soylent omissions; no Whole/sibling substitution.|
| legacy-ai361 |361|69specific authored AI families; paid country plans/honest vision and active source multipliers stay distinct.|
| legacy-model543 |543|62specific helper/geometry/absence families; false/zero literals do not prove original nonuse.|
| spark/vhp/tree/mirage-elite |7/1/1/1|Exact source/helper absence, projected-health difference, contextual tree and primary-first elite boundaries.|

Original UnitReload remains unparsed, separately qualified from actual AirportBound/Ammo/Dock/NumberOfDocks, paid-health and same-Economy ownership/liveness obligations. Unit Soylent omission is distinct from real building refunds and Cost-based XP. Share0/active arguments do not cover animation/water/zero-weight siblings. F146/F147 helper absence and F251 eligibility do not fabricate generic runtime fallback.

A separate raw-original reconstruction matched all **131 legacy family closure hashes** and counts. F146/F147 each have105source sections/361incoming references; F251's complete positive eligibility census is exactly AMMOCRAT/CAMISC01/CAMISC02. Live production census is **103Rust files (64sim/39game)**: exact paths, SHA256, FNV1a64, byte counts and model digest all agree. Original-scope/special/model-surface output hashes agree with the observed packet. Static/current model evidence cannot discharge the fresh SIM1/SIM2/CLIENT findings; source-policy review here accepts only the truthfulness and integrity of these limited declarations, not every underlying implementation or original fidelity.

## Frozen-source and evidence integrity

I independently rehashed **all685 frozen input files (670source +15derived)** and **all64 protected authority spans**, using the exact recorded section starts and explicit byte lengths. Zero mismatches. B-corrected-02/actual-input-scope.json hashes to **74301b64e3ac002ff085d1cf87d31218b57110476b4078253f1428c29cb8bafe**, the actual root input binding. Counts and source predecessor HEAD inside preparation packets remain provenance, not the current frozen Git TIP.

I independently compared the15propagation preimages/current outputs and their actual hashes. Exactly3,290TSVcells changed:1,994final_input_sha256;1,294consumer_bindings digest tokens;1model_census_sha256;1recorded_scope_review factual ID. All1,294consumer strings retain identical surrounding text/token order after factual hexadecimal digest normalization. No exact literal, reason, classification, guard, policy-decision or qualified-receipt column changed. Root factual ID authenticates bytes only.

All **8unique qualified acceptance artifact hashes** resolve to actual preserved files and rehash. The **580archive entries** of193-canonical-port-and-archive.json all retain exact byte lengths/SHA256. Those include failed, interrupted and restored historical attempts; preserved bytes do not make their native source current. Qualified route artifacts identify static/loaded/native/client distinctions and the exact historical common-path limitations. No native approval is inferred from a filename, hash string or export.

B-focused-02 raw complete audit is **FAILED184passed/1failed/0ignored/0filtered**. The failing Static116 initial whole baseline reported the missing Model162 R84/R85 policy anchor during the separately documented root rewrite. Its transient-read explanation is an inference: no captured read-time bytes prove it. Stable four-control retry and its8whole positives do not synthesize185pass.

B-isolated-five-01 has equal before/after685-path snapshots and5zero-exit commands. The source-bound summary records21Won, max map mean3.096ms; it also records meaningful tails/busy windows, so means are not universal smoothness evidence. Fixed3 is24,000schedule observation, not an all-to-Won gate. These are separate isolated results and do not replace full13. Full13 was deliberately not started after fresh confirmed blockers; R194 still owes exactly one complete unchanged corrected-source run for both final gates, including unfiltered185 and debug600smoke. Real executable/package failure/restoration acceptance also remains owed; no gate was waived by this DATA review.

## Provenance and retained obligations

All176cumulative added assets under assets/ have literal inventory entries in assets/LICENSES.md. Generator source paths exist and were traced: original procedural HD geometry/render/composition, map recipes, RA5gem/crate primitives and synthesized audio/announcer motifs. Branch object-path inspection found no prohibited INI/MIX/CSF/SHP/VXL/HVA/AUD/palette/terrain asset extension. The4,941table rows carry numeric/flag/identifier values and source-key citations, rather than imported UI strings/comments. No imported original file/media was found by these checks.

Limits: I did not execute Blender or all media/map generators; a read-only in-memory RA5Pillow regeneration attempt could not run because Pillow is unavailable in the ordinary Python runtime. I installed nothing. I did not exhaustively compare every historical binary blob to every possible third-party asset or prove the absence of arbitrarily renamed fragments; current schema/value/source scans, generator source and path/history inventory establish the stated provenance checks, not such a universal claim.

All **183stable debt IDs** remain unique, fully populated and NOT_ACCEPTED_BY_DOCUMENTATION. I close none by alias or broad declaration. All **51HOWTOSEE rows remain PENDING—requires user verification**; the full13,855-byte checklist section retains SHA256 **8ae4edfadf97fc68138d03c9a12d9c1217cf0c1fb65cf6dd6010c77b1c5bd6c1**. Visual/audio quality, animation/readability, normal human victory/defeat, pacing/feel and packaged/platform launch have no fresh user approval. Source inspection and native App tests cannot supply it.

No Cargo, game, benchmark, match, soak, package, mutation probe, clone, worktree, Git mutation or canonical write was performed. No unrelated process was touched. No native lease was needed. Only this assigned report is written in the pre-existing approved reviews cache; no substantial external directory was created. Existing report/evidence/cache directories remain necessary. Configured CARGO_TARGET_DIR, UV_CACHE_DIR and PIP_CACHE_DIR were preserved.

This is the initial panel2 DATA report for the exact frozen TIP. A revised candidate requires authenticated changed-input/interface review and the unchanged required final gates before fresh-lead acceptance.
