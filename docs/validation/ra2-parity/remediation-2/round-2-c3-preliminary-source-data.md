# Independent DATA preliminary C3 cumulative source inspection

Cumulative base8230a955b251e9341d349f99eaeb86f7f8253664 through HEAD53a5225c272ff45b7d29d124befc6b1d26c5c832 plus the exact three candidate modifications below. Reviewer /root/review2_data remains independent and has never implemented, remediated or prepared this programme. This is preliminary whole-cumulative source examination, not a clean freeze, final factual binding, native acceptance or programme approval.

**No new DATA source P0/P1 or unresolved unique DATA P2 is established. The actual Liberty non-Won remains a blocking acceptance failure; exact replay, whole141, new factual producer/verification/propagation/freeze and all final gates remain OPEN.** Existing other-slice P2 findings are retained. This report grants no GO.

## Authentication and entire cumulative coverage

Independently recomputed the preliminary packet at the approved cache:150149bytes, SHA256 **74bfdc0b942caa065f08a8f58ce8270cdbd8bcdf80c6e683728abac35c8b65f2**. All689 current file lengths and SHA256 values match. Its exact path set equals C2; comparison with the C2 manifest SHA0e2edf2bce2a8e41581c382a923dce7a5057ccf0231256903d314acb21261b99 gives686 identical,3 changed,0added/removed. All64 authority records equal C2 and their actual protected byte spans independently match length/hash, including explicit fixed-length preliminary sections without end markers. A second689-file hash check immediately before this report again finds0mismatches.

| Changed actual input | Bytes | SHA256 |
|---|---:|---|
|crates/sim/src/ai.rs|208229|32e2f224eb66136803ce40c27a886d7346b00026cda4b9f9c8c5aa02abdbd2f8|
|crates/sim/tests/ra8_ai.rs|462426|2472e56bff3ff4ee6b048132619cc71d848dc4a281012930b4417358f69f851b|
|crates/game/tests/m6_production_matches.rs|37217|f9d3de4abf4fa787c189b03a32ee7a4436fe7b4e7e6338cc424b8e6abdb84b7b|

I authenticated and incorporate my complete initial and revised DATA reports and their precise limitations, not merely their conclusions:

- round-2-data.md:21872bytes/SHA705534565226d7237f7dbebdfbcf4b6f0c2b806c465578cdc860302ac875574f.
- round-2-revised-data.md:24893bytes/SHA7925fe85a973acb091affdbe17bc2a1db7e3dc3c2b04e75fdcdfa5058eaf7a3e.
- round-2-native-partial-data.md:9154bytes/SHA9e04c10cb11dae5cb4fdb60a9e660976de3d1631ba0a3ca9b8b31d5949f9b458.

Initial coverage read the original/continuation prompts, current requirements, DATA brief, common instructions/module ownership, panel1 DATA/lead and applicable decisions. The C2 report independently examined every revised interface and factual propagation across the entire cumulative RA0–RA9 scope. I reread the revised common instructions, R201 and relevant fresh C3 SIM1/SIM2/TESTS interface reports. The686 exact inputs authenticate all retained source-value/schema/audit judgments; the changed interfaces receive fresh examination below. This is not a patch-only review.

| Entire cumulative DATA area | Current assessment |
|---|---|
|RA0/definitions/loaders/fixtures|All seven shipped RONs, loaders, schemas and51fixtures remain exact C2 and initial-data bytes; units1/economy2/combat5/arsenal10/rules4/skirmish3/ai3, strict field/reference/version/range checks and192KiB bound retained. Arsenal175050bytes. No new serialized field.|
|RA1–RA5 numerical generation|Original INI identities, mappings, generator/parser/equations, source literals, map geometry/rosters, assets, production/refund/power/economy/ore/combat/veterancy/crew/spawn/flight data remain exact. The independently computed initial2655 comparisons retain their actual2603equal+52explicit differences/0unexplained result. This calculator was not rerun here; its exact inputs and logic are authenticated unchanged. R6/I6prime geometry, R7 dimensional15→20 conversion assumptions/quantization and R29 unchanged distinct caps retain all qualifications.|
|RA6–RA8 data/runtime/client consumers|Temporal current-actor guards, Hospital/pending jobs/voluntary exits, dead support refusals and bounded R195/R200 passive choices, honest AI terrain/capability/cargo and client scenery/bridge observations remain exact C2 except the AI file's pure opportunity predicate. The complete changed construction/callers and native economy interface are inspected below. No new original-engine behavior claim.|
|RA9 audit|All49 reachable Rust audit files,67TSV/4TXT siblings, schemas/fixtures/reference/generator inputs, admission/route/physical/array/component/original-closure controls remain exact C2. Initial380identity/399typed-route integrity,131family original-closure assessment and prior actual focused/current-whole controls remain historical evidence at their precise source scope. New185/current-source execution is not inferred.|
|Provenance/export boundary|All4941 reference keys,75 numerical/deliberate difference records,1911 exact disposition atoms, source closure/raw-original and176asset licence/generator inventory remain exact C2. No media/map/balance/source-literal change or historical asset import is present. Initial no-universal-renamed-fragment theorem/all-media-regeneration limitations remain.|

Fresh table parsing independently counts4941 reference rows,75 difference rows and1911 disposition atoms:1869Whole+19Component1+19Component2+4Component3. Classes remain759CURRENT_MODEL_NONREAD/1152INTENTIONAL_DIFFERENCE/0ORIGINAL_ENGINE_UNUSED. Routes remain static116,masked19,legacy-model543,residual117,legacy-ai361,model162,behavior87,metadata147,completion174,carry38,guard75,consumer62,spark7,tree1,mirage-elite1,vhp1. Literal/value/class/reason/route/guard/decision/receipt bytes did not change. The original separate raw-INI parser's4941 matches retain their authenticated exact inputs; I do not claim a fresh raw-original parser execution here.

Initial DATA2-P2-01 stays resolved: crates/sim/tests/reference/README.md:101–106 correctly gives1911atoms/1888keys and completion174=132Whole+42components. No factual count correction grants semantic acceptance.

## Pure refinery predicate and all affected consumers

Read the complete diff and connected View/building snapshot, visible_ore/worker_job/service_income/funding/broke/sale/command paths. At ai.rs:1324–1363 actual BuildingSnaps come from current native Health/Owner/Building and are retained only at hp>0. Garrison surrogates have economy=false. View::own_kinds at1066–1071 includes only owner==player and economy=true. Thus ai.rs:2510–2516's new refinery_has_visible_ore requires a real current positive-HP owned economic building whose actual definition has credits_per_ore, plus nonempty existing visible_ore. Enemy, dead or non-economic garrison kinds cannot satisfy it.

visible_ore at2496–2505 first tests current vision before reading grid ore. No private terrain/actor knowledge, new stored state/hash field, RNG, clock, timer/census reset, threshold/cap, balance literal or schema is introduced. The predicate represents conservative income potential; it does not prove reachability or immediate availability of a disabled refinery. That limitation is unchanged policy scope rather than original fidelity.

All three consumers retain their surrounding gates:

1. run_ai:1417–1432: opted-in Rules AI may draft an armed Idle worker only under its existing last-resort/service conditions; bare visible ore without a refinery no longer vetoes this admission. Existing lone-worker/worker_job reachability policy remains above it.
2. funding_force:2593–2603: the armed-worker branch now requires a real refinery before visible ore can protect it from optional-asset financing. Existing nonworker force, RulesNone/service alternatives and understrength test remain.
3. understrength_funding:2605 onward: visible ore vetoes funding only with a refinery and existing worker/stowed potential. Existing service/recoverable-service/air protection, actual fighting force, prior-wave or stationary engagement, recursive living ownership and cohort target all remain. The same funding_force still controls latch release.

broke and sell_for_army continue using real affordability, producer/prerequisite and optional-asset rules, then ordinary native SellBuilding/QueueProduction commands. No credit/load/free-unit/outcome is injected by production. This independently appears a coherent correction: visible ore alone cannot be delivered without receiving infrastructure, so treating it as assured recovery unjustifiably suppressed existing ordinary reinforcement finance. The change is general, side/map/country neutral. It does not recover original AI behavior or prove every remaining economic recovery path.

I reconsidered the unchanged disposition classes/semantic reasons against this changed consumer. The source-key choices continue truthfully describing bounded current-model or intentional differences; none newly marks an active mechanic original-unused, promotes a false/default literal, changes a permitted conversion or waives actual runtime performance. Root hashes authenticate source, not semantic policy approval. Current final-source controls remain essential.

## New native controls and exact scoped receipts

The original454402-byte ra8_ai.rs is an exact prefix of the new file. All139 old test declarations/bodies/assertions remain; two additions make141. I read the entire shared added helper and both controls at13359–13582. It authors ordinary initial ore before controller construction, removes controllers during controlled fixture preparation, genuinely harvests shipped capacity with real rate/interval/travel, and requires no OreDelivered, actual full Idle load, residual genuinely visible ore and zero cash. The live-refinery neighbor then adds a real owned refinery. The original fighter publicly attacks after actual scheduled vision.

Both arms restore the same controllers and require charged native queue, ProductionComplete, actual barracks UnitSpawned, original/recruit WeaponFired and surviving original fighter. No-refinery requires one real optional depot sale with the exact definition refund400 and no delivery; refinery requires actual delivery and no sale. Spawned fixture buildings/actors are controlled setup, not claimed paid production. The recruit is paid through the actual observed economy. These are meaningful end-to-end local controls, not direct calls of the helper.

Independently read process/raw/plan and rehashed evidence:

| Exact scope | Actual result |
|---|---|
|Cache C-ai-liberty-before-06|Exit101; selected2 gives1positive pass/1causal failure/139filtered. No-refinery saleNone/delivery0/chargedfalse/paidfalse/no recruits; original fighter fired but recruit did not. Assertion requires Some400. Raw1323bytes/SHA dbd6a42b4d881c388855fbece8151665023e02998ef884fc5ae76e10104fbee6.|
|Cache C-ai-liberty-after-01|Exit0;2pass/139filtered. No-refinery saleSome400,delivery0,charged/paid/recruits10and11/both shots; refinery saleNone,delivery1,paid recruit9/both shots. Raw818bytes/SHA d2f490f3e1aecd1626c7ebe353017efbb7b5672afb3a53fdc87e49f504909e83.|

Each plan hash agrees with its process; each before/after guarded snapshot pair is byte-identical. Both saved ra8_ai test preimages match final SHA2472e56…; AFTER saved AI preimage matches current SHA32e2f22…. BEFORE source snapshot records exact C2 AI SHAe67953f…. This establishes same-body production-only causal power for the demonstrated finance boundary. The changed armed-worker drafting branch is source-inspected; this pair does not independently establish that branch's execution. Earlier failed setup/compiler attempts are not accumulated into passes or intended causal failures.

m6 diff is109added/0removed lines: opt-in RA9_LIBERTY_TRACE observations, actual loss/stalled-state diagnostics and one exact regression. All old sweep, late-twin and regression bodies remain. The new test at466 calls existing duel at512/duel_with_length at517 with shipped Liberty Bay/France/Germany/Medium/Medium,seed0,starts0/1, actual map/Rules/AI/shared setup. It requires Some(Won). Existing tick_limit at53 and72000 base/scaled288000 cap, all756jobs/worker concurrency, invariant/rejection rules and actual Won length/table assertions remain. Diagnostics read/query/format only; their test-only debug-string unwrap assumption is not a production behavior correction or native win. Trace wall-time is not performance evidence.

## Real failed gate and current acceptance boundary

Read R201 and independently hashed docs/validation/ra2-parity/review-2/04-test.log:41128bytes/SHA **11a42ae9f5bf9ee738ab0cb192bf21d6832158baa89d1f297a9c1506f43b7565**. Actual Liberty Bay FranceMedium/GermanyMedium seed0 returns None at288000/288000. The preceding144/756non_Won0 progress is partial, not a complete756. Source-isolated21 wins/fixed3/guards cannot prove this different match.

R201 preserves direct shell-15 versus wrapper241 after root stopped the witnessed doomed run; a native grandchild exit was not individually observed. Only01–03 completed0;04 did not complete;05–13 were not run. No new756table or closing metadata exists for that attempt and old length-cache files cannot be promoted.

I also read/rehashed BEFORE02 exact trace376729bytes/SHAa3c8a87e7955dde24f3f0edf6976fbb7c222a3a8e7eb8febdf50afdc22d5525c: actual P0 loss SimId1415 at287834 is explicit. The speculative phantom-census/timer explanation is retracted and no such patch is present. It is neither a current finding nor grounds for a timer reset.

At the last inspection C-ai-liberty-after-02 remains RUNNING, with no actual completed exact replay. Whole141 is uncredited. The two local passing controls cannot close that original failure or the full programme.

The15 derived/factual outputs intentionally remain historical C2 bytes. Their byte authentication is not current consistency with changed AI/tests; this preliminary packet does not claim current model-census/input/consumer/native acceptance. After the exclusive lease, actual producer execution, independent root factual verification, strictly factual propagation/readback, protected span/checklist retention and a new clean freeze are necessary. Independent final ingestion must check all changed source bindings against actual bytes and semantic scope; it cannot mechanically trust pins.

Fresh strict/current whole audit, actual late twin positive, both one-cause native negatives and restored positive with new source/executable/depinfo/raw bindings remain owed. C2's benchmark-only compiled-closure qualification no longer applies after compiled AI/M6 changes. Then all mandatory corrected-source gates remain: final isolated five; one complete unchanged full13; whole185audit and final whole141AI together; actual756production/756duels with all outcomes/distributions/unchanged caps;260x2x36000 deterministic windows/heap; original benchmark5ms mean/busy/50msmax/heap/activity budgets; builds/debug/release600smokes;package/both regenerations; actual release/package missing-asset exit1 and restored600-frame exit0. All five full-cumulative final slice/addenda inspections and fresh independent lead are required. No future receipt is credited here.

## Stable debt, quality, limits and preservation

Freshly read docs/RA2_HISTORICAL_DEBT.tsv:182826bytes/SHA d0c93c3a6cb828449a0fb859265e40b0987eb85de003688c279c1668987eecbe, exact C2;183rows/183uniqueIDs, all retained with owned assessments and remaining obligations. The revised DATA per-ID judgments remain in force, including GENERATEDRULES/source-admission,REFUND/Soylent versus Cost-XP,FOOTPRINT47,SCHEMASIZE/MISSINGDEF/LOADORDER,source export/R6/R7/power/cargo/specialist/geometry/return-fire/VHP/animation/death boundaries. Alias, hash or documentation status closes none. Other slices' lost-Hornet exact800/notice-cap/FIFO-race and remaining retained P2 are not silently removed or downgraded.

The exact HOWTOSEE section in docs/RA2_GAP_ANALYSIS.md independently rehashes13855bytes/SHA8ae4edfadf97fc68138d03c9a12d9c1217cf0c1fb65cf6dd6010c77b1c5bd6c1. All51user-only quality rows stay pending. No visual/audio/animation/readability/pacing/human victory-defeat/platform approval is supplied.

Initial limits persist: no closed-engine universal fidelity proof; no new full2655/raw-original calculator execution here; no all-media/Blender generation or universal renamed-asset provenance theorem; no native malformed/mutation probe by this reviewer; no renderer/audio/user-feel acceptance or theorem for forged/cyclic internal states. These preliminary source conclusions must be reconciled with final exact factual bindings and actual native outcomes before any fresh lead.

The cumulative120active-minute limit remains respected; idle/implementation/outage time is excluded and no precise duration is invented. No native/Cargo/project executable, probe, source/helper/Git/process mutation, new agent, checkout or unrelated-project action was performed. Only this new report is written in the existing approved cache. No substantial directory or temporary artifact was created; existing review evidence remains necessary. Configured Cargo/uv/pip caches and both running projects are untouched.
