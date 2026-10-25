# RA2 parity programme — authoritative independent review, round 1

**Decision: NO-GO.** Frozen TIP `23647de4220408d42eeef6522deb4d4541528452`; cumulative base `8230a955b251e9341d349f99eaeb86f7f8253664`. Decision date: 2026-10-22 UTC. There are **seven unique P0 production findings, eight unique P1 findings (one admission, six privacy/honest-vision, one shared validation/acceptance gate), and five P2 items**. Missing regression controls are incorporated into the corresponding production findings rather than counted twice. No merge, reviewed tag or approval tag is authorized by this decision.

## Scope, independence and method

I am the fresh independent lead `/root/review1_lead`. I implemented, remediated and prepared none of the reviewed code. I read the common dispatch, bound lead dispatch, factual interruption addendum, the complete original/continuation prompts, current requirements/module supplement, applicable PLAN severity/invariant/category/decision clauses, R6/R7/R29 and relevant parity-plan acceptance/equation sections, current GAP/handoff/checklist/debt, and all five COMPLETE independent reports: DATA, TESTS, SIM1, SIM2 and CLIENT. The historical stopped-RA5 scope is superseded; the entire cumulative RA0–RA9 programme and retained baseline/regressions are in scope. The cumulative Git diff contains 5,527 changed paths, including extensive retained evidence; file/declaration counts are not a claim that every evidence line was newly executed.

The panel covered the five current slices under R172's three-then-two scheduling and fresh-lead arrangement. I independently opened and traced every reported P0/P1 through the actual relevant caller, predicate, mutation and downstream path; I checked the implicated regression bodies and deduplicated overlapping reports. Root factual checks, preparer notes, current acceptance cells and policy labels were treated as claims, not independent approval. Prospective remediators' external planning supplies no evidence for this decision.

No Cargo, game, benchmark, match, soak, package, mutation probe, process launch, clone, worktree, canonical edit or Git mutation was performed by this reviewer. Verification is complete source-path analysis plus existing raw evidence, not fresh native reproductions. This is sufficient to confirm the accepting/mutating branches below; remediation must supply fail-before/pass-after native regressions.

I independently reconstructed the 195 manifest plus 196/196b factual binding outputs and rehashed **669 source/binding files and 64 authority spans: zero mismatches**. HEAD remains the frozen TIP; tracked diff is empty. Only coordinator-owned untracked gate outputs are present. I independently rehashed all four interrupted gate logs, four 187 causal/positive logs and five 198 isolated logs against their receipts: zero mismatches. Immutability does not establish gameplay correctness.

## Mandatory validation: actual observed results

`scripts/validate_ra2_parity.sh` runs the original eleven commands plus the two regeneration checks in order, captures each actual exit, continues after ordinary failure and exits nonzero if any selected command fails. Its test summary sums raw `test result:` blocks; it is not a unique-primary-test counter. A signal before a run() call completes leaves no exit line and must never be interpreted as success.

| # | Required command | Actual current 199 result |
|---|---|---|
|01|`cargo fmt --all -- --check`|exit 0, 3 seconds|
|02|`git diff --check`|exit 0, 0 seconds|
|03|`cargo clippy --workspace --all-targets -- -D warnings`|exit 0, 6 seconds|
|04|`cargo test --workspace --release`|**INTERRUPTED**, no observed final command exit; no workspace pass|
|05|`cargo build --release -p game`|not reached|
|06|release game `--smoke-test 600`|not reached|
|07|debug game `--smoke-test 600`|not reached|
|08|release `m7_benchmark`|not reached|
|09|release `m7_soak`|not reached|
|10|release `m14_duels -- full`|not reached|
|11|`sh scripts/package_macos.sh`|not reached|
|12|`python3 scripts/generate_ra2_data.py --check`|not reached in this gate; DATA's separate read-only check exits 0|
|13|`python3 scripts/ra2_reference.py --check`|not reached in this gate; DATA's separate actual-original check exits 0|

The current 04 raw log contains 268 completed primary game unit tests, 0 failed/ignored/filtered, plus four separate one-test child blocks with 267 filtered each. Completed integration target blocks before M6 contain another 31 primary passes. The M6 target started seven tests and records six individual `ok` lines, including late twins, but its 756-configuration sweep and target have no final result. Neither six individual successes nor 299 completed earlier primary results supplies a command04 exit. No final total is claimed. Prepared 200/202 metadata's zero-filter assumption is incorrect for child probes and remains unexecuted; fix accounting without altering historical raw script totals.

At 2026-10-22T00:40:25 UTC root intentionally sent SIGTERM to the owned group30304 after source confirmation of P0-01. The original controllers were already missing; the preserved stop receipt records the orphaned native shell/Cargo/M6 members before termination and the empty group afterward. There is no observed direct child final exit code to invent as 0 or 101. Canonical RUNNING/WAITING controller strings are stale. The detailed receipt and unchanged raw hashes govern. **199 satisfies neither the RA9 gate nor the independent-review gate. Do not wait for it.**

The request to let one future corrected-source complete full13 serve both gates is still pending at decision time. Urgency and the dispatch's earlier shared-run assumption do not grant it. **Separate complete RA9 and review full13 runs remain binding unless root receives and records explicit user authorization to combine them.** Even if authorized later, it can apply only to an actual complete corrected-source run with all commands, coverage, caps, source guards and independent review intact.

## P0 findings: independently confirmed production results

### P0-01 — IFV repair revives a parasite-killed patient or acts after healer death

Locations: `crates/sim/src/special/vehicles.rs:262`, `:286`, `:361`, `:390`, `:412`, `:425`; actual schedule `schedule.rs:137`. Sources: SIM2 S2-01, SIM1 F01, TESTS T-P1-1.

`vehicles::advance` snapshots Actors before its parasite loop. Due damage is applied immediately by `detonate_one`; actual Health may become nonpositive, kill credit is recorded and a parasite can escape. The later IFV loop selects both healer and patient using the earlier Actor.alive/hp. Its final write reads live HP but unconditionally subtracts negative RepairBullet damage. Stock Damage50 followed by repair−50 can turn a 10HP host into−40 and back to10 before Cleanup. A killed repair IFV can similarly heal another patient. The command-stage positive-HP guards and the paid-depot/pad rechecks do not cover this later specialist loop.

Expected remediation: require current positive Health for source and patient at the action boundary; a genuine kill cannot be undone by ordinary repair. Preserve living healing/parasite removal. Add Rules-present public attachment/loading and due-tick controls for dead patient, dead healer and one-HP-surviving/living positives; assert HP, cleanup, parasite escape/removal and no spurious repair. Existing before-first-Drone-hit, living range/reload and depot-death tests deliberately avoid this boundary. Retain them.

### P0-02 — FreeMCV grants another MCV while the player owns one in cargo

Locations: `crates/sim/src/crates.rs:439`, `:595`; `special/interaction.rs:87`, `:217`; `special/state.rs:76`; `match_setup.rs:1185`. Sources: SIM2 S2-02, SIM1 F02, TESTS T-P1-2.

`stranded` checks only the on-map living Soldier table and live ECS buildings. Real embark stores a positive-HP Passenger then removes its ECS body; stock MCV Size6 fits the stock Landing Craft's SizeLimit6/capacity12. With no buildings, credits>1500 and another own collector, the FreeMCV override therefore treats the still-owned viable cargo MCV as absent. Short-game survival and build-limit traversal already count descendants under a living owned root, so defeat does not eliminate this scenario.

Expected remediation: use consistent live-root ownership and recursive cargo semantics in the base-unit census. Test public embark→controlled ordinary non-unit crate outcome→unload, direct/nested supported descendants, root-owner change, destroyed payload, genuinely stranded and enemy-MCV controls. A random Unit reward must not obscure an erroneous guaranteed override. Keep exact credits and water boundaries.

### P0-03 — Specialist actor damage discards real firepower multipliers

Locations: `crates/sim/src/special/vehicles.rs:294`; `special/infantry.rs:204`; shared damage at `combat.rs:423`; `veterancy.rs:121`, `:233`. Sources: SIM2 S2-03, SIM1 F03, TESTS T-P1-3.

Periodic parasite/Squid damage has preserved Passenger Experience and uses it for reload/elite culling, but passes `firepower_pm:1000` to its Blast. Ordinary parasite firing does not apply an earlier ordinary Blast. Yuri's real Deploy pulse likewise passes1000 although it selects actual base/elite area-fire weapons and applies reload. Stock Squid/Yuri FIREPOWER rank and crate products are therefore lost. For a veteran Squid Grab15 against heavy100%, the declared equation yields16; the periodic path yields15. Culling/overkill tests can pass either result.

Expected remediation: carry the current live Yuri and stored off-map parasite damage product exactly once into the shared armour equation, preserving provenance, selected weapon, immunity, culling and reload. Measure one native hit/pulse on durable valid targets above culling at base/veteran/elite rank and with nonidentity crate bonus; independently calculate integer expected HP deltas. AI-owned actors use the same rank×crate product. SIM1’s suggestion of an additional AI firepower factor is refuted: `rules.rs:377` DifficultyRules has no firepower member and `veterancy.rs:378` enlist inserts default Experience/rank only. Do not add a difficulty multiplier or invent such a regression premise.

### P0-04 — Guard selects an ineligible nearby threat forever

Locations: `crates/sim/src/tactics.rs:168`; `combat.rs:2294`, `:2400`. Sources: SIM2 S2-04, SIM1 F04, TESTS T-P1-4.

Guard ranks hostile visible unconcealed candidates by distance/id without actual weapon/layer/armour eligibility. A closer Rocketeer and farther legal ground target make a guarding stock Grizzly repeatedly install Attack on the aircraft. Combat correctly ends that explicit attack with NoDamage; next Pathfinding reinstalls it. The persistent Guard/no-order branch prevents ordinary idle acquisition from rescuing the order. A legal threat is starved indefinitely.

Expected remediation: use the real effective base/secondary/elite/gunner selector and automatic-acquisition eligibility before ranking, retaining Guard's anchor and information policy. Add nearer-ineligible/farther-eligible scheduled controls for both layer directions and applicable zero/one-Verses/minimum-range/reachability boundaries; require actual shots/damage to the legal target, not merely an order. Existing one-target and range tests remain.

### P0-05 — Pending bay launches ignore Temporal freeze and current same-stage death

Locations: `crates/sim/src/flight.rs:243`, `:535`, `:625`, `:662`, `:745`; `combat.rs:1729`, `:2140`. Sources: SIM2 S2-05, SIM1 F05, TESTS T-P1-5.

After real firing establishes a pending Carrier/Dreadnought volley, Combat's Temporal suppression retains the target. `bays` checks disabled state but not erasures, so a later27-tick launch still creates UnitLaunched while its source is frozen. The launch helper has no suppression or current-liveness guard. Independently, `spawned_units` can detonate an incoming missile and kill a spawner before `bays`; bays still trusts the earlier positive Actor.hp and can create a new unit after death. Target aim also relies on the old snapshot.

Expected remediation: validate current source Health/owner, suppression and legitimate current target at the actual launch boundary. Preserve already-launched missile continuation, legitimate existing aircraft lifecycle and exact27 spacing. Add real public pending volley→Temporal controls and real incoming-impact death at a due pending slot, with living/released positives. Do not replace the source freeze with a fake disabled-only fixture or forbid all in-flight activity.

### P0-06 — Erased miners work/pay and Chrono Miners escape by teleport

Locations: `crates/sim/src/harvest.rs:285`, `:394`; `special/advance.rs:190`; `special/infantry.rs:482`, `:508`, `:616`; `special.rs:208`. Source: SIM1 F06, independently confirmed here.

Harvest's real worker/refinery checks cover Health only, with no erasure input. A positive-HP erased miner already standing on ore keeps advancing work/taking bails; a loaded adjacent worker can unload and credit funds. More directly, `special::advance` teleports every ChronoMiner in ToRefinery without an erasure check. Teleport only checks legal room. This happens before infantry's frozen-speed handling/beam retention; it can move the frozen target out of range and release the beam. Setting speed0 later does not undo the mutation. The declared contract explicitly freezes the target while fired on.

Expected remediation: suspend the erased worker's economic and special return actions, preserving its legitimate job for release. Use an actual beam spanning a due harvest/unload boundary and an already-returning Chrono Miner, asserting position, job/load/ore/credits/events and legal resume. Existing command-dead worker tests do not cover frozen positive-HP actors.

### P0-07 — Frozen buildings can be sold or repacked out of erasure

Locations: `crates/sim/src/special/commands.rs:205`; `economy_commands.rs:609`; command routing `command.rs:1340`; commandability `special.rs:379`. Source: SIM1 F07, independently confirmed here.

Ordinary selected-unit commands and Deploy/Interact apply erasure commandability. Undeploy finds a live owned yard but omits it, then changes its footprint/type into an MCV. SellBuilding's real economy path also omits erasure, refunds and despawns immediately. `process_commands` provides no earlier generic building erasure rejection, and the BuildingIndex includes these positive-HP erased buildings. The owner can collect sale value/crew or repack a body that is explicitly frozen under a sustained enemy beam.

Expected remediation: reject these mutations through the same frozen-source contract before credits, queues, occupancy, type or wreck changes, with real Temporal→Sell/Undeploy unchanged-state tests and later-release positives. Privacy/ownership/death rejection precedence must remain correct. No documented model difference permits these escape actions.

## Temporal production/power/service adjudication

SIM1 correctly identified additional **source omissions**: `economy.rs:965` excludes disabled factories but not erased factories; `repair_docked` checks erasure on the patient at `special/repair.rs:46` but not the host; `special/advance.rs:109` charging excludes disabled but not erased sources; `repair_buildings` and `update_power` at `economy.rs:702/782` have no erasure input/check. These observations are confirmed. They must be resolved explicitly at the same Temporal remediation interface, not hidden by a movement-speed fix.

The proven contracts above establish no movement, new launch or own destructive transformation by a frozen target. They do **not**, by themselves, recover every closed-engine passive building rule. I do not manufacture five additional independent P0s from absent `contains_key` calls. Continued **active factory completion/spawning and active host service/charging** are connected frozen-source actions that remediation must either suppress coherently and test at a real beam boundary or justify by a precise pre-existing, substantively compatible rule; a new broad waiver is insufficient. Snapshot/capture/disabled/power/liveness distinctions must survive. Whether a frozen building still contributes passive power, radar, prerequisites or permits an external repair effect is not specified in enough detail to assert original-engine fidelity here. Record and test the exact chosen model with evidence; do not claim “RA2 stops all power” from these source omissions, nor silently grant a new exemption. If these connected active boundaries remain unresolved, the shared acceptance P1 remains open even after P0-05/06/07's narrow reproductions pass.

## P1 findings: independently confirmed

### P1-01 — Legacy/masked admissions do not enforce accepted identity/typed evidence routes

Locations: `crates/sim/tests/ra9_audit/legacy-source-admission.rs:13`; `scope-inputs.rs:166`; `masked-speed-admission.rs:147`; `policy-admission.rs:163`; expected independent identities `source-plans.rs:220`. Source: DATA-P1-01.

I traced `audit_at→load_exact_unused→validate_row→source_plans::admit→legacy require→accepted_source→family guard→add_unused→record→finish`. Static116/Metadata147/Residual117 check accepted consumer/guard only for nonemptiness; the first two discard receipts and Residual checks only nonemptiness. They never compare those accepted identities against their independent declaration. Masked19 correctly checks consumer/guard through source_reason but never enforces its exact typed route set/hash shape. Final finish reconciles atoms/witnesses, not those omitted fields. Thus foreign nonempty identity/evidence can be admitted while legitimate key/value/reason/final-input fields stay fixed.

Scope: **380 identity/guard admissions and399 exact receipt-route admissions**. I independently recomputed all380 current identities: they match. DATA checked all399 current records and eight resolved artifact hashes. This is a real missing fail-closed audit contract, not an assertion that present values are forged or that static records are native execution.

Expected remediation: enforce independent exact consumer, guard, typed kind/route and hash/duplicate syntax at the true admission boundary. Keep expected sets independent of the cell being validated and static evidence static. Add accepted-baseline one-cause negatives on a nonfirst row of each legacy family and Masked19 for foreign consumer/guard, missing/foreign route, malformed/hashless and duplicate receipts, reaching real complete reconciliation. Existing positive/source/physical controls remain.

### P1-02 — Spy hover exposes undiscovered true identity/owner

Locations: `crates/game/src/hover_info.rs:158`, `:218`; actual presentation `units.rs:647`. Source: CLIENT-P1-1. The hidden disguised Spy is deliberately rendered with apparent friendly infantry art, but hovered_entity returns true Unit.kind/view.owner and the label adds enemy country/real health/armour/rank. Ordinary hover defeats concealment. Use observer-qualified apparent information or omit sensitive values; test actual scheduled disguise/detection through hover. Correct cursor targeting exclusion alone does not cover it.

### P1-03 — Mind-controlled selections/groups track hidden enemies

Locations: `crates/game/src/selection.rs:425`, `:1161`; owner change `special/infantry.rs:725`; live view update `units.rs:650`. Source: CLIENT-P1-2. Pruning uses registry existence only, while draw_selection draws a selected ID's live Transform without current Owner/Visibility. Mind control retains the entity/view; an old selected/group unit moving beyond sight remains marked. Prune loss of ownership/commandability in both current selection/groups and independently filter hidden/nonowned draw markers. Test native control/release followed by actual group/selection/render behavior.

### P1-04 — Damage smoke discloses concealed submarines/Mirages

Locations: `crates/game/src/weapon_fx.rs:1692`, `:1975`; concealment `special/reconnaissance.rs:120`. Source: CLIENT-P1-3. Smoulder finds real damaged actors and positions, gates only terrain visibility, then creates independent effects whose animation also gates only cells. Damaged recloaked naval units and stationary Mirages remain allowed concealed states. Require the observer's actor concealment as well as terrain sight for persistent actor effects; test genuine damaged/reconcealed actors and detector positives. Promotion's separate hidden check does not protect smoke.

### P1-05 — Explicit I/Interact reads hidden occupancy before native validation

Locations: `crates/game/src/special_controls.rs:363`, `:419`; `sim/src/command.rs:353`. Source: CLIENT-P1-4. I-click looks up raw ground/air/building/Props ID with no known-target predicate. Hidden occupied versus empty worlds differ in sink issue, mode transition, acknowledgement and the rejection text containing the real ID. Native UnknownTarget equality cannot undo the earlier lookup. Use observer-known target selection; equal-knowledge hidden/missing worlds must have identical mode/sink/audio/text effects. Keep own/visible/scenery positives and correct right-click semantics.

### P1-06 — Fogged scenery reflects unseen live damage/destruction

Locations: `crates/game/src/battlefield.rs:521`; `hover_info.rs:256`. Source: CLIENT-P1-5. Raze consumes all nonbridge PropDestroyed events, removes scenery and rewrites terrain/looks without a vision input. Hover uses explored-only admission and live Props HP/existence. Optional fog therefore exposes unseen changes. Retain observed scenery state until re-observation, including hover/minimap consequences; test actual fog→unseen damage/destruction→visible reconciliation. Shroud-only defaults do not waive the retained fog option.

### P1-07 — Remembered bridge approach leaks unseen building occupancy into AI commands

Locations: `crates/sim/src/ai.rs:1506`, `:1535`; `pathfinding.rs:216`. Source: CLIENT-P1-6. `repair_bridge` legitimately remembers a hut but chooses its Move endpoint using full grid building occupancy/passability. Nearest_cell's candidate acceptance changes for an unseen hostile building. Equal own/seen/memory states can therefore emit different decisions. This is decision-time hidden-world selection, outside the retained physical movement/path resolution boundary. Plan from public static terrain and legitimately observed/remembered occupancy, then let native movement encounter unknown blockers. Compare real AI emitted commands in equal-knowledge/different-hidden-building worlds; retain R69 approach/memory-removal controls.

### P1-08 — Required final validation, acceptance and corrected-source proof remain incomplete

Locations: original prompt section7; continuation sections5/6/8; current review V1/A/T requirements; current `summary.txt:1`; validator `:73`. Shared SIM1 F08 and every slice's execution limitations are consolidated here.

Complete04/full181 audit, current full756 outcomes/distributions, release build, both600-frame smokes, benchmark, soak, full duels, package and generator/reference gate results are absent from199. All defects above need meaningful production regressions and final execution. Native current-policy/static/loaded/client route premises must retain exact scope and binding after correction. SMOKE-FAIL-01's actual executable/packaged failure acceptance is not supplied merely by App handlers; establish its scoped process result or substantively resolve the explicit obligation. No result can be inferred from a declaration, filtered positive, stale cache, root hash approval string or summed historical pass count.

## P2: nonblocking items, separately justified

1. **P2-01 — Obsolete UNCOMPILED/UNRUN audit headers.** `ra9_audit/actual.rs:1`, `source-routes.rs:1`, `scope-inputs.rs:1` and peers contradict actual bounded canonical compilation/execution. Correct present status while preserving original provenance, failed attempts and incomplete full gates (DATA-P2-01; R56-17).
2. **P2-02 — Current headless-scenery omission prose is false.** `m6_production_matches.rs:19/504`, GAP current scenery paragraphs and SCENERY-01/R56 aliases say broad matches omit Props. Actual load_map marks semantic world features, setup_match calls world_objects::install, and Props::from_grid installs them; the harness never removes them. Correct present scope, preserve historical statements. This does not establish all scenery stress/rendered coverage (SIM2 S2-06, CLIENT-P2-3, TESTS T-P2-2 deduplicated).
3. **P2-03 — Ordinary notice burst cap is ineffective.** `crates/game/src/notices.rs:193`: deferred additions/deletions are not in the loop's Query, so seven same-frame notices can exceed MAX_NOTICES and repeated pruning can pick the same oldest entity. Track the whole batch or trim after application; retain correct synchronous PublicBuildingNotices behavior (CLIENT-P2-1). Bounded presentation spill, no demonstrated simulation loss.
4. **P2-04 — PNG pathname metadata/open race.** `crates/game/src/tileset_image.rs:166`: metadata.is_file then blocking File::open can race replacement with a FIFO. Direct FIFO and bounded regular-file checks work. Align with opened-handle/nonblocking regular-file semantics and preserve typed errors/symlink compatibility. Historical same-boundary severity remains P2; no new race probe was run (CLIENT-P2-2).
5. **P2-05 — Lost-carrier regeneration boundary remains broadly tested.** `ra4_spawns.rs:326`, `ra4_shipped.rs:864` prove actual loss/replacement and distinguish800 from short reload, but not exact Lost immediately before/Ready at800 (or fixture400). The new exact missileLost200/returned-carrierReload100 tests do not cover lost Hornet regeneration. No timer bug is shown; the plan's explicit positive “carrier regains a lost Hornet” is covered, so this precision debt alone is not upgraded to P1. Preserve and preferably add the exact boundary without claiming original-engine flight physics (TESTS T-P2-1; SPAWN aliases).

## Coverage and explicit decisions

| Requirement/interface | Independent judgment |
|---|---|
|R6 / I6′|Accept the explicit invariant amendment and logical design: surface one whole unit or up to3 on-foot infantry, separate one-air layer, no unit on a building. Grid room/register/claim_spot/standing_point/release, movement reservation, factory/crew/cargo/teleport paths and corruption/crossing controls were covered by SIM1/SIM2/TESTS and cross-checked here. On-foot derives from actual InfantryTypes plus nonair domain. Chosen offsets/geometric crossings remain documented assumptions; logical held-cell claims are the invariant. No further demonstrated occupancy defect found; final invariant execution still owed.|
|R7 conversion|Accept the reasoned replacement explicitly allowed by the original prompt/continuation: `min(Speed×256 div100,255)`, infantry3/2, then exact rational3/4 to20ticks; frame durations4/3, minutes1200, range256. GI11/Grizzly13 are intentional quantized outputs. The infantry factor and JumpjetSpeed interpretation remain bounded assumptions, not recovered RA2 internals. Slower1× feel stays user-only.|
|R29 / limits|Retain base72000 and span scaling; no further increase authorized. Fixture36000, m13 ten-player400000 all21 diagnostic configuration, m14's300000 base/actual per-map limits, fixed24000 performance windows and soak36000 windows are distinct. R56 permits Running at soak-window end; completed outcomes must be Won. No cap/configuration transfer or full-distribution claim from one sample.|
|Hash/determinism|Ordered state/hash coverage in the five slices includes queues/fractions/options, Grid spots/air/domains, Experience, cargo/parasites/bays/missions, strategic/special state and AI controllers. Conditional Grid no-foot hash compatibility was examined, including source lengths; no concrete different valid-state equal-stream collision was established. FNV is not a cryptographic proof. The late-twin causal protocol supplies real bounded late activity, not universal determinism.|
|Economy / I7|Rate/fraction/exact-last-remainder/cancel/payment/order paths and real native bodies are coherent in reviewed scope; credits nonnegative and completed cost require final execution. A late queue observer cannot independently see completed units already removed. New IFV/Temporal actions are not waived by correct ordinary production charging.|
|Commands / I5 / privacy|FIFO/atomic public validations, command-dead checks and observer-hidden/missing native rejection matrices are real. Private AI witnesses remain distinct. P0-07 and six client/AI P1s prevent blanket command/privacy acceptance.|
|Loaders / schemas / I8/I11/I12|Current versions units1/economy2/combat5/arsenal10/skirmish3/ai3/rules4; DATA checked all51 RON fixtures and generators. Exact required fields/unknown/duplicate/version/range/reference/subset checks, opened-handle sim/settings reads and integer arithmetic were covered. Arsenal175050<196608 bytes; no limit increase needed. PNG race remains P2.|
|Test integrity|TESTS compared cumulative history/assertions/fixtures, retained names and decision-authorized expectation changes; no additional unjustified removal/skip/weakening confirmed. Static declarations418base/524user-baseline/1743TIP,193 files,181 audit declarations and358 client declarations differ from primary executions and child probes. No ignore found. Existing RulesNone tests remain compatibility evidence; current shipped production requires Rules-present paths. New demonstrated defects have inadequate missing causal controls, incorporated above.|
|Provenance/assets|DATA independently checked permitted4941 numeric/flag/identifier rows, authenticated external hashes,176 asset/license entries and generator sources; the large selected history scan found no authenticated INI blob, original-format path or distinctive comment import. No speculative finding from requested short unit names. Binary media regeneration and art/audio quality were not proved.|
|RA0–RA9 acceptance|All five ownership slices cover the finished programme, including baseline interfaces. Data/country/production/armour/rank/air/world/specialists/controls/AI/audit implementations exist. Source/filtered receipts are meaningful limited evidence, not final acceptance. The findings and gate table identify remaining blockers rather than declaring absent RA5–RA9 from historical prose.|

## Numerical audit and all 1,911 typed dispositions

DATA's independent calculator (no generator/audit imports) reports2,965 checks:2,913 exact and52 specifically listed differences, zero unexplained; its independent actual-source check covers all4,941 reference rows. SIM2 separately recomputed345 weapon scalars with only the two listed−2 range-sentinel interpretations. These are independent panel calculations, not a claim that I reran their calculators. All75 numeric-difference rows retain specific scope; smaller Foundation geometry, exact helper/elite omissions, shipyard reach, sentinel interpretations, dynamic radiation, spawner substitution and Sapper/death context remain differences rather than false source equality.

I additionally wrote a separate in-memory original INI parser for this review, authenticated rules SHA `adc869ca037db5141f562f18092d022045e194de0d1204a46b9ec8372b2b826f` and art SHA `d732bfc8ffb6d52908342254c22731acb730ae6d2bbfb41983ef109e48a8f061`, and compared **every one of the1,911 installed disposition atom values** (Whole or exact tuple index) directly to those originals: zero mismatch. Independently counted1,911 unique atoms across1,888 keys, all with specific nonempty reasons:759 CURRENT_MODEL_NONREAD+1,152 INTENTIONAL_DIFFERENCE;1,869Whole+19component1+19component2+4component3; **zero ORIGINAL_ENGINE_UNUSED**.

The bounded canonical197c whole-positive raw log establishes16,211 physical witnesses=13,419Compared+2,792GameOwned;7,108 independently demanded/successful dependency pairs;5,022 source atoms=3,111Compared+1,911dispositions;75used differences. This is1passed/180filtered, not181. The actual successful/required physical dependencies and source finish check were traced. Export hooks emit proposals only.

| Route | Atoms | Lead disposition and obligations |
|---|---:|---|
|static116|116|Current Height/Image/instant/jumpjet boundary is a legitimate explicit current-model distinction, not original nonuse. P1-01 blocks admission integrity.|
|metadata147|147|Specific Points/presentation/IFV/visual omissions may remain declared; real Cost/weapon/radiation/SW neighbors retain independent comparisons. P1-01 blocks admission.|
|residual117|117|Specific turn/turret/electric/light/selector policies remain differences/nonreads with exact reasons, no replacement of implemented native effects. P1-01 blocks admission.|
|masked19|19|Actual branch masking with genuine successful speed/Range/Gravity dependencies; no false nominal-speed credit. Current identity equality is valid, typed receipt-route enforcement remains P1-01.|
|consumer62|62|Exact source/current-consumer distinctions allowed within their contracts; real paid health-after-ammo, ownership/liveness/disabled/power/retry and client mission/selection premises remain mandatory. No label substitutes for those paths.|
|carry38|38|Authored difficulty/power/acquisition policy choices are explicit differences; omitted options cannot borrow active multiplier credit. Guard starvation remains P0-04 regardless of these declarations.|
|model162|162|Bounded2D/movement/timing/lobby/presentation models are explicit;33 static presentation and29 absence migrations establish inspected boundaries only. They do not waive privacy failures or user quality.|
|guard75|75|Exact reconstructed original graphs/current selector boundaries and real neighbors retained; source absence does not prove engine inactivity. Launched/frozen actions remain P0-05.|
|behavior87|87|Actual command/select/target/concealment consumers are separate requirements; client P1s show why static type labels cannot close them.|
|completion174|174|Exactly132Whole+42components over151keys.81 active registry ordinals remain active aliases;21 unitSoylent is omitted resale/Grinder behavior, separate from building refunds and Cost XP. Active shares/arguments retain real comparisons; animation/water/inactive arguments do not cover siblings.|
|legacy-ai361|361|69 exact authored AI families may replace closed-engine strategy, with actual source difficulty/production comparisons separate. Honest vision and required winners remain binding; P1-07 is not waived.|
|legacy-model543|543|62 specific helper/geometry/absence/omission families remain explicitly qualified; false/zero/unparsed is not original unused and no generic delayed-kill/default fallback is inferred. Native interfaces remain required.|
|spark|7|Exact absence premises and real neighbors retained; no fictitious native spark field consumption.|
|vhp|1|Explicit nearest/legal-target model difference, not projected-health reservation. Current-policy controls and final execution remain required.|
|tree|1|Ordinary tree cursor exclusion versus Ctrl ForceFire is a client consumer contract, not damage illegality. Preserve App/sink controls and user cursor quality check.|
|mirage-elite|1|Primary-first elite shadow is exact and limited; no inference that all DisguiseFireOnly semantics are unused. Actual base/elite undisguised controls remain required.|

These are my scoped judgments of acceptable **documented model distinctions**, subject to their real premises and corrected-source validation. They are not an unconditional approval of the acceptance tables, original-engine fidelity, or all1,911 native routes. In particular P1-01's399 affected atoms and the exposed production/client defects remain unaccepted. The source-policy choice to omit a mechanic cannot retrospectively exempt an implemented mechanic's incorrect common path. Keep every literal, typed class, reason, portion, alias and route exact; remove or reclassify only with substantive evidence and a new source binding.

## Historical/current execution reconciliation

The failed131 producer-membership audit and136 exact-uncovered174 diagnosis remain history.158 whole-positive is real.159 ran181 unfiltered and failed175/6/0ignored/0filtered;166 changed five helpers/bodies,168 and canonical197c pass the exact six repairs with175filtered.175old passes+6later passes is not181final passes.197b's stale branch-binding failure and196b factual refresh remain distinct. No failure/preimage/restoration is erased.

The182 duplicate-MoveCompleted repair has an actual same-body failed-before/passed-after scope;185 yard-start proof is21maps/344 supported growth/player-count setups with genuine public exits within unchanged256ticks, not all-to-Won.181 exactLost200/reload100 controls and retained27 launch spacing are real but do not establish every flight/lost-Hornet boundary.

I read the actual late-twin body: two fresh load/setup App worlds, every-tick hash/ordered-events/outcome and I6′/I7, unfinished before each step, actual Won after36000 and positive late WeaponFired/ProductionComplete/OreDelivered.187 positive/restored-positive logs each pass1/0/6filtered, P0Won41738/72000, late444/8/6, final `78c81bd069fca660`, trajectory `5a8adabd01d20b05`. The extra RNG draw actually fails hash at36001; observer-only late production omission reaches the same winner/hash then fails missing ProductionComplete (444/0/6). Immediate/separate later restorations and later positive match test SHA `e7f3b317c98e63368ae0e0dba733d7ae55274dd080c18e200d6998b776e4e97d`. Panel source comparisons establish unchanged runtime/test inputs with later derived audit-binding differences.186 was interrupted after native launch, not compiler-only. These functional receipts cannot grant performance credit or future-source acceptance; reconcile affected runtime changes and rerun/rebind needed late controls after remediation.

198's five isolated commands really pass at their stated source/runtime scope: all21 ten-player maps Won within400000, median85959/p90114969/max147715; all per-map means<5ms, slowest3.161ms. Three fixed24000 schedules are valid timing windows while Running; three example guards pass1/0 each. Their raw hashes match. Significant actual tails remain (for example Coral p9953.216ms and Glacier max1094ms); passing mean thresholds does not certify smooth rendering or the separate benchmark longest/busy budget. No full756 distribution, soak, final benchmark or required package/smoke result transfers from these isolated tests. Corrected runtime needs explicit affected-evidence reconciliation and required final measured acceptance, retaining exact worker/configuration scope.

## Remediation and next independent round

The shortest compliant route is one coherent fresh remediation against these exact claims: reproduce each true defect with a meaningful scheduled/client regression, make the smallest correction, retain existing controls, and refresh factual source/branch/input/route bindings. Readiness/ownership/liveness/privacy must be shared consistently; introducing fake flag getters or weakening winning caps, thresholds, probes, workers, audit coverage or baseline assertions is unacceptable.

Before a future GO: all seven P0s and seven substantive P1s must be fixed or specifically refuted with evidence; connected Temporal active-source semantics and remaining explicit native/executable acceptance must be resolved; the complete unfiltered audit/negative target and every required command must actually pass on the corrected freeze. Preserve raw failed-before/passed-after and restoration evidence. Report primary versus child/filtered results accurately. Complete RA9 full13 and independent review full13 separately unless an explicit user answer authorizes one complete shared corrected-source run. No extra unfiltered debug sweep is implied; the required debug600 smoke remains. A new fresh independent panel/lead must review the complete cumulative diff, including remediation, under the existing up-to-three-remediation-round rule. Implementers/preparers/current remediators are ineligible.

## Unverified and user-only work

No source review proves all original closed-engine behavior, all-seed AI termination, exhaustive valid command states or unseen binary media ancestry. The exact bounded assumptions above stay visible. Final corrected-source native acceptance, complete gates, source-policy negatives, current winners/distributions/performance and required executable failure results remain open for the reasons given; no GO can be issued from the present incomplete run.

All **51 HOWTOSEE rows** in `docs/RA2_GAP_ANALYSIS.md:900` onward remain **PENDING — requires user verification**, including actual display/readability/selection, animation/terrain/unit/effect art, audio/announcements, pacing/responsiveness, ordinary human victory/defeat, local packaged/platform launch and chosen distribution/signing checks. Source or App tests do not approve them. They are not findings and do not independently require NO-GO; only the user approves quality or creates an approval tag.

## All183 historical debt IDs and aliases: lead reconciliation

The complete ledger and DATA's full per-ID assessment were read and reconciled with the other slices. Stable IDs/aliases must remain. This appendix gives an explicit current disposition for **each of183 rows**, including69 R56 aliases; aliases are not independent defects/native runs. “Retain scoped boundary” accepts the limited documented model/API/presentation distinction, not execution or original fidelity. “Final gate” retains required relevant final-source evidence under P1-08; it does not demand an invented universal stress theorem. Specific positive historical receipts above supersede stale UNRUN prose only in their real scope. No ledger row is closed simply because it says P2 or NOT_ACCEPTED_BY_DOCUMENTATION.

| Ledger ID | Topic/alias | Lead disposition |
|---|---|---|
|LIVE-01|LIVE-01|P0-01/P0-05 show remaining same-stage liveness defects; retain narrower correct producer/worker/depot controls; final gate.|
|TRANS-01|TRANS-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|TRANS-02|TRANS-02|Existing recursive lifecycle controls valid; P0-02 exposes missing FreeMCV census consumer; final gate.|
|SQUID-01|SQUID-01|Host WeaponFired control remains valid; P0-03 separately blocks attached source firepower; final gate.|
|PROP-01|PROP-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|CAP-01|CAP-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|GATHER-01|GATHER-01|Retain terrain-flood heuristic limitation; no proved stock setup/invariant failure; do not claim arbitrary reachability.|
|XP-01|XP-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|CREW-01|CREW-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|SPAWN-01|SPAWN-01|P0-05 pending-source launch; P2-05 lost-carrier precision; exactLost200/Reload100/27 scope retained.|
|CAPTURE-01|CAPTURE-01|Real Engineer capture followed by paid opposite-side production exists at ra1_buildability:314; final execution.|
|START-01|START-01|18521maps/344yard setups/public exits within256 is real scoped positive; refresh affected-source/final execution.|
|LATE-01|LATE-01|187positive/causal/restored evidence valid scoped functional proof; refresh affected runtime and final gate.|
|NEAR-01|NEAR-01|No current full756 distribution; preserve failed/near-limit history, caps and required final measurement.|
|AI-CASUALTY-01|AI-CASUALTY-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|CONCEAL-01|CONCEAL-01|Retain real early-native concealment controls; client leaks P1-02..06 remain independent.|
|LASTSTAND-01|LASTSTAND-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|REFUND-01|REFUND-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|DRONE-01|DRONE-01|P0-01; prior before-first-hit removal is valid but does not close lethal IFV boundary.|
|FIFO-01|FIFO-01|Opened-handle sim/settings controls valid; image pathname race remains separate P2-04; final gate.|
|SETTINGS-01|SETTINGS-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|CSV-01|CSV-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|ID-01|ID-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|BENCH-01|BENCH-01|Real participant movement accounting present; mandatory final two-scenario200 benchmark/budgets owed.|
|ROSTER-01|ROSTER-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|MODE-01|MODE-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|CARD-01|CARD-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|SIDEBAR-01|SIDEBAR-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|CREDITS-01|CREDITS-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|SMOKE-FAIL-01|SMOKE-FAIL-01|App handlers covered; exact executable/packaged failure acceptance remains P1-08 obligation.|
|MOVE-DUP-01|MOVE-DUP-01|181failure/182same-body correction/pass are real; retain final-source regression execution.|
|DOCK-01|DOCK-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|AIRDOCK-01|AIRDOCK-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|AMMO-EVENT-01|AMMO-EVENT-01|Retain scoped explicit interface difference; no asserted NoAmmo event contract; native return controls/final gate.|
|CAPTURE-DOCK-01|CAPTURE-DOCK-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|DRAG-01|DRAG-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|SIDEBAR-CLICK-01|SIDEBAR-CLICK-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|BUILDSELECT-01|BUILDSELECT-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|QUEUE-UI-01|QUEUE-UI-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|GROUPCENTER-01|GROUPCENTER-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|LAYOUT-01|LAYOUT-01|Retain exact stated boundary and all relevant51pending user checks; no automatic quality approval.|
|SMOKE-MAPS-01|SMOKE-MAPS-01|Retain opening/growth smoke scope; no21-map rendered tour inferred; required smokes owed.|
|SMOKE-INPUT-01|SMOKE-INPUT-01|Retain truthful helper-driven smoke scope; actual App input tests separate; required smokes owed.|
|SCENERY-01|SCENERY-01|P2-02: refute current omission premise; semantic Props installed; preserve historical/stress/render limits.|
|TICK-EVENT-01|TICK-EVENT-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|GROUPOCC-01|GROUPOCC-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|CROWD-01|CROWD-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|JAM-01|JAM-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|OVERTAKE-01|OVERTAKE-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|CONTINUOUS-01|CONTINUOUS-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|PATHALLOC-01|PATHALLOC-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|FREEUNIT-01|FREEUNIT-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|HUMANWORK-01|HUMANWORK-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|ORETARGET-01|ORETARGET-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|NOCHASE-01|NOCHASE-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|TRUESTATE-01|TRUESTATE-01|Retain physical movement/pathfinding boundary; does not exempt P1-07 decision-time endpoint leak.|
|EVENTAUDIENCE-01|EVENTAUDIENCE-01|Retain developer-log boundary; no permission for player-facing P1-02..07 leaks.|
|PLACEVISION-01|PLACEVISION-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|HARMLESS-01|HARMLESS-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|MISSINGDEF-01|MISSINGDEF-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|LOADORDER-01|LOADORDER-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|PROJECTILEBOUND-01|PROJECTILEBOUND-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|PROJECTILEVIS-01|PROJECTILEVIS-01|Retain exact stated boundary and all relevant51pending user checks; no automatic quality approval.|
|AUDIOCAP-01|AUDIOCAP-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|RENDERPERF-01|RENDERPERF-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|SETUPAPI-01|SETUPAPI-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|AIDISCARD-01|AIDISCARD-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|AIPLACE-01|AIPLACE-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|AIPOLICY-01|AIPOLICY-01|Accept explicit authored strategy replacement; real difficulty/production/honest vision/winner obligations separate.|
|AICONTROL-01|AICONTROL-01|Actual current source routes plus final required Won/distributions/gates owed; no every-seed theorem.|
|AIHOTKEY-01|AIHOTKEY-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|AINAMES-01|AINAMES-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|SETUPERROR-01|SETUPERROR-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|AIBALANCE-01|AIBALANCE-01|Final measured AI distributions owed; human balance/ordinary matches stay user-only.|
|TESTTIME-01|TESTTIME-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|PLATFORM-01|PLATFORM-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|WINDOW-01|WINDOW-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|SMOKETIMING-01|SMOKETIMING-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|INPUTVIS-01|INPUTVIS-01|Retain exact stated boundary and all relevant51pending user checks; no automatic quality approval.|
|DOUBLELOG-01|DOUBLELOG-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|FLOATVALID-01|FLOATVALID-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|WEBGL-01|WEBGL-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|CHECKEDASSET-01|CHECKEDASSET-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|MAPWORDING-01|MAPWORDING-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|DRAWORDER-01|DRAWORDER-01|Retain exact stated boundary and all relevant51pending user checks; no automatic quality approval.|
|SCHEMASIZE-01|SCHEMASIZE-01|Current192KiB bound/175050bytes and migrated schema10 supported; no raise needed; final checks owed.|
|GENERATEDRULES-01|GENERATEDRULES-01|P1-01 admission integrity blocks; exact1911 scoped dispositions above; no blanket policy acceptance.|
|FOOTPRINT-01|FOOTPRINT-01|Accept enumerated geometry differences with source-resolved axes; no Own credit for actual mismatches.|
|DOCKCOUNTCACHE-01|DOCKCOUNTCACHE-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|ALIASART-01|ALIASART-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|RA6ASSUME-01|RA6ASSUME-01|Retain each named bounded specialist model; no closed-engine fidelity; actual exposed native effects/gates remain.|
|CREWXPSOURCE-01|CREWXPSOURCE-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|INFANTRYPATH-01|INFANTRYPATH-01|Accept logical-I6prime/geometric-interpolation distinction in R6; no continuous collision guarantee.|
|COMBATDIFF-01|COMBATDIFF-01|Retain explicit accurate-impact/prone/shrapnel/wall-occlusion differences; cannot waive implemented damage defects.|
|SOAKPEAK-01|SOAKPEAK-01|Report actual configured workers and concurrent aggregate budget; final soak mandatory.|
|PACKAGE-01|PACKAGE-01|Required local package/smoke gate owed; signing/notarization/other-Mac launch are retained user scope.|
|PACKAGESIZE-01|PACKAGESIZE-01|Report actual final artifact sizes after required package; no stale historical size credit.|
|VERSION-01|VERSION-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|DISPLAY-01|DISPLAY-01|Retain exact stated boundary and all relevant51pending user checks; no automatic quality approval.|
|LICENSE-01|LICENSE-01|Retain original/media/font inventory distinction; dependency-notice collection outside user release criteria; no imported-assets finding.|
|OLD-CLOSURES-01|OLD-CLOSURES-01|Retain individual parser/setup/health/client historical tests; no generic closure or new execution from alias.|
|R56-01|LIVE-01|P0-01/P0-05 show remaining same-stage liveness defects; retain narrower correct producer/worker/depot controls; final gate.|
|R56-02|_PRIVACY|Native hidden/missing matrix remains valid; player-facing/honest-AI failures P1-02..07 independently block; final gate.|
|R56-03|_CLAIM|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-04|_GRIDHASH|No concrete valid-state equal-stream collision established; retain legacy/spot hash controls and final determinism gate.|
|R56-05|INFANTRYPATH-01|Accept logical-I6prime/geometric-interpolation distinction in R6; no continuous collision guarantee.|
|R56-06|PLACEVISION-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-07|_BURST|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-08|LIVE-01|P0-01/P0-05 show remaining same-stage liveness defects; retain narrower correct producer/worker/depot controls; final gate.|
|R56-09|_DEPOT|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-10|_REGEN|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-11|GENERATEDRULES-01|P1-01 admission integrity blocks; exact1911 scoped dispositions above; no blanket policy acceptance.|
|R56-12|REFUND-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-13|_INFANTRY|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-14|GENERATEDRULES-01|P1-01 admission integrity blocks; exact1911 scoped dispositions above; no blanket policy acceptance.|
|R56-15|_POWER|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-16|_IVANREASON|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-17|_STALECOMMENT|P2-01 current status banners/documentation; preserve actual bounded run/failure scope.|
|R56-18|_REGEN|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-19|FOOTPRINT-01|Accept enumerated geometry differences with source-resolved axes; no Own credit for actual mismatches.|
|R56-20|_ELITEFALLBACK|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-21|_SPEED|R7 accepted as reasoned dimensional conversion with named infantry/Jumpjet assumptions; user feel pending.|
|R56-22|_MINIMAP|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-23|LATE-01|187positive/causal/restored evidence valid scoped functional proof; refresh affected runtime and final gate.|
|R56-24|_SWEEP|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-25|NEAR-01|No current full756 distribution; preserve failed/near-limit history, caps and required final measurement.|
|R56-26|TRANS-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|R56-27|CAPTURE-01|Real Engineer capture followed by paid opposite-side production exists at ra1_buildability:314; final execution.|
|R56-28|_CHARGING|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-29|_POWER|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-30|_COUNTRY|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-31|SCENERY-01|P2-02: refute current omission premise; semantic Props installed; preserve historical/stress/render limits.|
|R56-32|_PLAYERHASH|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-33|SMOKE-INPUT-01|Retain truthful helper-driven smoke scope; actual App input tests separate; required smokes owed.|
|R56-34|CARD-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|R56-35|SCENERY-01|P2-02: refute current omission premise; semantic Props installed; preserve historical/stress/render limits.|
|R56-36|_R43|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-37|_PROCESS|Historical process only; fresh independent panel completed; P1-08 final gates remain mandatory.|
|R56-38|START-01|18521maps/344yard setups/public exits within256 is real scoped positive; refresh affected-source/final execution.|
|R56-39|SIDEBAR-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|R56-40|_MINIMAPOFF|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-41|_SWEEP|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-42|_AMMO|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-43|SPAWN-01|P0-05 pending-source launch; P2-05 lost-carrier precision; exactLost200/Reload100/27 scope retained.|
|R56-44|_SPAWNBAY|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-45|_PRIVACY|Native hidden/missing matrix remains valid; player-facing/honest-AI failures P1-02..07 independently block; final gate.|
|R56-46|_PRIVACY|Native hidden/missing matrix remains valid; player-facing/honest-AI failures P1-02..07 independently block; final gate.|
|R56-47|CAPTURE-DOCK-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|R56-48|AIRDOCK-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-49|AMMO-EVENT-01|Retain scoped explicit interface difference; no asserted NoAmmo event contract; native return controls/final gate.|
|R56-50|LASTSTAND-01|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-51|XP-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|R56-52|_TEXTS|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-53|_CRUSHXP|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-54|_PATIENCE|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-55|_BUILDABLE|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-56|_MISSILE|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-57|_NAVAL|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-58|CAPTURE-DOCK-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|R56-59|_ELITE|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-60|_SPAWNDEF|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-61|_CREWBASE|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-62|_CREWROUND|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-63|SPAWN-01|P0-05 pending-source launch; P2-05 lost-carrier precision; exactLost200/Reload100/27 scope retained.|
|R56-64|CREW-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|R56-65|_SETUP|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-66|_DOCKLIMIT|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-67|_RANGE|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-68|_RECON|Retain scoped model/API/documentation boundary as stated; preserve exact controls/assumptions and relevant final gate; no additional confirmed blocking defect or universal proof.|
|R56-69|CAPTURE-DOCK-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|DEATH-MOD-01|DEATH-MOD-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|SPAWN-SPACING-02|SPAWN-SPACING-02|Keep27 and exact existing boundary controls; no inferred10frame launcher rule; final gate.|
|HARVEST-SOURCE-02|HARVEST-SOURCE-02|Accept named19work-tick GameOwned assumption with18/19 control; not Harvest.Rate exactness; Temporal P0 remains.|
|TYPE-IMMUNE-01|TYPE-IMMUNE-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|BOMBSIGHT-01|BOMBSIGHT-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|REVEAL-SW-01|REVEAL-SW-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|VHP-01|VHP-01|Accept specifically declared projected-health omission; retain current-policy native controls and final gate.|
|RETURN-FIRE-01|RETURN-FIRE-01|Accept narrow explicit current acquisition/retaliation assumption; no original applicability/nonuse claim; Guard P0 remains.|
|TREE-CURSOR-01|TREE-CURSOR-01|Retain exact source/test and real scoped historical receipt; relevant final-source native/acceptance execution remains P1-08; no new proven defect beyond findings above.|
|MIRAGE-GATE-01|MIRAGE-GATE-01|Accept exact current primary-first/undisguised model boundary; no recovered original boolean meaning; native/final gate.|
|EXP-SPREAD-01|EXP-SPREAD-01|Accept explicit selected-warhead geometry difference; originalExpSpread remains unparsed, not obsolete.|
|FIRE-ANIMATION-01|FIRE-ANIMATION-01|Accept explicit omitted burning-animation damage difference; real weapon/HE/radiation premises stay separate.|
|DEFAULT-DEATH-01|DEFAULT-DEATH-01|Accept current no-default-fallback selector boundary; no fake zero runtime field/original nonuse; preserve positives/negative.|

## Preservation and final decision

This reviewer created no substantial directory outside the canonical repository. Only this report was written into the pre-existing approved reviews cache `/Users/xingyuwang/Library/Caches/Codex/reviews/MyGame001/ra2-final-review-20261021`; it and the five reports/supporting evidence remain necessary. The pre-existing integration-validation cache retains original-only historical raw logs and also remains necessary. No repository, worktree, evidence or failed attempt was deleted. Configured CARGO_TARGET_DIR, UV_CACHE_DIR and PIP_CACHE_DIR were preserved.

The authoritative decision is **NO-GO** for the exact frozen TIP: seven confirmed incorrect simulation-result families, seven substantive P1 defects and the shared incomplete acceptance/validation P1 remain unresolved. Five P2 items alone would be nonblocking. No pending stopped process or future approval can change the actual evidence in this decision. Root may materialize this exact report into `docs/reviews/RA2-parity-review-1.md` as required; that metadata copy does not change the reviewed source or supply remediation/acceptance.
