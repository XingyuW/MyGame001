# Independent final RA0–RA9 review, round 1 — TESTS

Frozen cumulative scope: `8230a955b251e9341d349f99eaeb86f7f8253664..23647de4220408d42eeef6522deb4d4541528452`.
Reviewer role: fresh independent test-integrity/adequacy slice; no implementation, remediation, source edits, native probes, Cargo commands or extra agents. The current common instructions and actual dispatch override the historical separate-target/copy instructions.

**Recommendation: NO-GO.** This slice confirms **0 additional P0, 5 P1 test-adequacy findings and 2 P2 items**. The P1 findings are the missing causal controls corresponding to production defects independently identified by SIM2 and traced by root; the lead should consolidate related production/test findings rather than count each twice. Required final validation is interrupted, not passed. No conclusion here accepts the 1,911 policy choices or supplies original-engine fidelity, user quality approval or a merge/tag authorization.

## Findings

### T-P1-1 — The IFV liveness controls avoid the lethal parasite/repair boundary

**P1.** [ra6_vehicles.rs:1283](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/tests/ra6_vehicles.rs:1283), [ra6_vehicles.rs:457](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/tests/ra6_vehicles.rs:457), [ra1_commands.rs:929](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/tests/ra1_commands.rs:929), [ra9_refinery_repair_liveness.rs:194](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/tests/ra9_refinery_repair_liveness.rs:194).

The existing real IFV/drone test removes the parasite *before its first damage tick*. The repair-range/reload test heals living patients. The command-stage corpse regression deliberately changes IFV `gunner=0`/the mode list (R80 explicitly describes this narrower immediate-death fixture), and the new depot liveness test explicitly keeps its patient living while killing the depot. These bodies therefore do not exercise the real RepairBullet after another effect makes its patient or healer dead inside the same specialist pass.

I independently read the relevant production path: `special::advance` calls `vehicles::advance`; that function snapshots `actors`, applies parasite damage to live Health, then selects IFV healers/patients using the earlier `a.alive()/b.alive()` values before writing Health again ([vehicles.rs:262](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/src/special/vehicles.rs:262), [:360](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/src/special/vehicles.rs:360), [:427](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/src/special/vehicles.rs:427)). This verifies that the coverage gap can hide the reported production defect; it is not a hypothetical demand for arbitrary combinations.

**Expected:** retain existing controls and add genuine Rules-present, public attachment/loading scenarios at lethal versus one-HP-surviving boundaries. Assert a dead patient remains dead and receives no repair, a dead IFV performs no repair, correct cleanup/parasite escape, and the living control really heals. Preserve a failing-before/passing-after regression receipt on the corrected frozen source.

### T-P1-2 — FreeMCV refusal has no transported-MCV ownership control

**P1.** [ra5_crates.rs:431](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/tests/ra5_crates.rs:431), [crates.rs:595](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/src/crates.rs:595).

The FreeMCV tests cover the credits boundary, disabled rule, an owned building, either side's on-map base unit and enemy ownership. They never collect a crate while an owned MCV is loaded aboard a transport. The new recursive transport/lifecycle/build-limit tests contain no FreeMCV/crate assertion. The actual `stranded` predicate considers the on-map soldier list and buildings; this is exactly the interface where moving a live owned MCV into cargo removes the negative premise from the predicate. The current suite can therefore pass while granting an extra MCV to a player who already owns one.

**Expected:** public load → real crate collection → public unload controls using a still-live owned MCV, including a retained nested descendant where supported; ordinary crate content must remain selected. Keep no-MCV, enemy-MCV and dead/lost-cargo positive controls so a blanket refusal cannot satisfy the test.

### T-P1-3 — Rank tests mask specialist firepower loss through culling or overkill

**P1.** [ra6_vehicles.rs:577](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/tests/ra6_vehicles.rs:577), [ra6_infantry.rs:402](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/tests/ra6_infantry.rs:402), [vehicles.rs:286](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/src/special/vehicles.rs:286).

The attached Squid rank test sets the host to one HP above the rank's culling threshold and asserts eventual death after protection is removed. Both the unscaled and scaled periodic hit cross that threshold, so this is a good culling test but cannot test firepower. The newer attached-host witness measures the host's own WeaponFired/damage; the SquidPunch witness covers the ordinary secondary weapon. Neither measures periodic attached damage above culling. The Yuri PsiWave test uses a base-rank caster and a GI that dies, so it also cannot measure a veteran/elite multiplier. The real periodic Squid Blast currently supplies `firepower_pm:1000` despite attachment preserving passenger Experience, confirming the practical missing assertion identified by SIM2; the deployed pulse path needs the same independent check.

**Expected:** measure exact HP deltas of one native periodic hit/pulse on durable targets above all culling thresholds, at base/veteran/elite ranks and with a nonidentity crate bonus. Use independent integer equations and negative controls; do not substitute selected weapon identity, component existence or eventual death for damage magnitude. Keep the existing culling/immunity/reload tests.

### T-P1-4 — Guard tests have no nearer ineligible-target distractor

**P1.** [ra7_orders.rs:66](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/tests/ra7_orders.rs:66), [ra7_orders.rs:376](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/tests/ra7_orders.rs:376), [tactics.rs:170](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/src/tactics.rs:170).

The Guard firing/return test presents one valid ground victim; the GuardRange test isolates distance. Neither presents a nearer visible hostile that the guarding unit cannot damage and a farther eligible hostile inside the same Guard range. The reviewed tactics path chooses the nearest hostile by visibility/distance without the actual weapon/layer/Verses eligibility test; Combat then terminates that explicit attack with NoDamage, and the next Guard tick can choose the same distractor again. Thus these passing single-target controls can hide the SIM2-confirmed legal-target starvation.

**Expected:** public Guard controls with a nearer untargetable air unit (for example Rocketeer versus a Grizzly), separately a zero-damage armour/weapon control, and a farther valid target. Require genuine shots/damage to the eligible target within a bound, stable Guard/anchor behavior, and no attack on hidden/allied/missing actors. Retain the single-target and range controls. No native probe was run by this reviewer.

### T-P1-5 — Temporal freeze tests do not cover a pending spawned-unit volley

**P1.** [ra6_infantry.rs:1049](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/tests/ra6_infantry.rs:1049), [ra8_temporal.rs:403](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/tests/ra8_temporal.rs:403), [flight.rs:664](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/src/flight.rs:664).

Existing temporal tests verify immobility, damage immunity/removal, and release of a frozen Chrono source's existing beam. The spawn suites verify ordinary volleys, interruption by Stop/transport/death, readiness and loss, but do not establish a real pending Dreadnought/Carrier bay volley followed by native Temporal freeze. The production bay's stopped predicate checks disabled state while the frozen Combat branch retains Weapon.target; this permits later pending launches to continue during erasure, as SIM2/root traced. A passing movement or direct WeaponFired freeze assertion cannot observe the later UnitLaunched action.

**Expected:** public attack that really creates a pending bay volley, native Temporal hit before the next launch, no additional UnitLaunched while the spawner remains frozen, and a live/nonfrozen positive control. Check the specified unfreeze/erasure disposition. Preserve the separate rule that units/projectiles already launched before freezing may continue; do not fix the test by forbidding all in-flight activity.

### T-P2-1 — Lost-carrier regeneration still has only a broad interval assertion

**P2, retained test precision debt.** [ra4_spawns.rs:326](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/tests/ra4_spawns.rs:326), [ra4_shipped.rs:864](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/tests/ra4_shipped.rs:864).

The fixture waits 500 ticks after ending its targets, without a before-boundary assertion for the 400-tick lost slot. The shipped test establishes that the 800-tick lost slot is still Lost after roughly 403 ticks and Ready after roughly 813; it does distinguish regeneration from the shorter reload, but not an off-by-one or a substantial early replacement. The new exact Lost200 missile and returned-carrier Reload100 tests do not cover lost-carrier regeneration. I do not infer an actual timer bug, or missing “carrier regains a lost Hornet” functional acceptance, from this weakness.

**Expected:** pin the actual loss observation, assert Lost immediately before the appropriate regeneration boundary and Ready at it, retaining the older broad sortie controls and exact shipped 27-tick launch spacing. This remains narrower than original-engine flight-physics proof.

### T-P2-2 — The match harness still claims headless worlds omit scenery

**P2, documentation debt.** [m6_production_matches.rs:19](/Users/xingyuwang/GameProjects/MyGame001/crates/game/tests/m6_production_matches.rs:19), with corresponding inherited GAP/debt prose.

The header says the harness does not install Props and attributes that difference to host-only scenery. Current load_map calls Grid::mark_world_features ([map.rs:1213](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/src/map.rs:1213)); the harness keeps that loaded Grid; setup_match/configured calls world_objects::install ([match_setup.rs:1115](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/src/match_setup.rs:1115)), which builds Props::from_grid ([world_objects.rs:19](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/src/world_objects.rs:19)). No clearing path was found in the production/late-twin harness. Thus current headless matches include semantic map Props, even though they do not render them. I initially repeated the old omission in the unverified paragraph; that statement has been corrected before final delivery after independent tracing.

**Expected:** reconcile the stale header/GAP/debt claim with the current semantic installation path while preserving historical evidence and its original scope. Keep rendered/user quality verification separate.

## Scope, integrity and census

I read the original/continuation prompts, current common/dispatch and review requirements, PLAN invariant/category/severity clauses, RA1–RA9 test/acceptance paragraphs and relevant equations, current handoff/GAP/debt/checklist claims, decision ledgers for test adaptations, advisory test findings, the actual validator/examples/package paths and the named raw evidence described below. Claims were checked against current source and receipts, not treated as approval.

Read-only Git/census checks covered all tracked Rust source, integration tests, support modules and examples. Anchored `#[test]` declarations (including the multiline `should_panic` attribute) are **418 at base 8230a95, 524 at the user baseline 257a169, and 1,743 at frozen TIP** across 193 files containing tests. Current location totals: 399 declarations in src, 1,339 in integration/support trees and five in examples. These are **static declarations**, not a count of unique primary tests executed by Cargo or of native scenarios. The initial simple regex missed the multiline `m0_bootstrap` should-panic test; the corrected census includes it.

No `#[ignore]` was found under crates. No programme-era test file deletion was found. A full commit-history scan of removed function lines and before/after per-file test identities found retained/renamed tests rather than a missing programme test:

* `m1_map` presentation-only look test now preserves terrain and explicitly tests world-object metadata; its maximum-byte case and invalid bounds remain.
* `m3_definitions` and successive `m4_definitions` schema names advance with required schema migrations, retaining old/future/invalid rejection matrices.
* The longest-range projectile test advances 32→480 ticks under R32/R40, explicitly checks no earlier impact and the out-of-range seen target.
* The intermediate crate test rename under R60/R62 changes the intended indefinite crate lifetime to bounded expiration/replacement, keeps taking/replacement assertions, adds a minimum successful-seed guard, and preserves maximum-count assertions every tick.
* RA6's unit BuildLimit rejection becomes required support while all genuinely building-only fields still reject (R72); the immediate-death specialist test is honestly renamed to trait-injected (R80).
* Two removed baseline names predate programme work in the user's 116ffdd..257a169 commits: old fog projectile helper/test and the ring-only production-blocking test. The latter becomes the second-ring exit plus fully sealed waiting tests. These are not attributed to programme remediation.

The legacy `m9_arsenal` (24 tests), `m11_repair` and game `m10` assertions remain under the migrated fixture; their loader/placement API/schema changes do not turn their RulesNone evidence into shipped gameplay proof. Changed expectations/limits were checked against R8/R15/R21/R29/R31/R40/R45/R54/R56/R62 and later RA6/RA7/RA8 decision records; I found no additional confirmed unjustified deleted/skipped/weakened assertion. That statement is an integrity conclusion from diff/history and traced affected assertions, not a claim that every possible mutation of every test has been executed.

## Acceptance and production-path coverage

The following is a static crosswalk to actual test routes, with runtime limits stated separately:

| Milestone | Positive/negative/boundary/regression/integration coverage located | End-to-end/current-source status |
|---|---|---|
| RA0/baseline | Retained m0–m13 test identity/assertion migration, schedule ambiguity should-panic, fixture definitions/malformed-input tests, legacy mechanics | Historical baseline raw totals corroborated; no new baseline pass inferred |
| RA1 | ra1_buildability/match/catalogue/shipped/definitions; CommandQueue→SimSchedule setup/buildability/undeploy/win conditions. The real Engineer capture at ra1_buildability:314 is followed by paid Conscript/Militia/Engineer production and sale/lost-factory pause. Some other “captured” tests still use direct setup and have that narrower scope | 756 side/difficulty jobs and country-slot guards remain; final sweep interrupted |
| RA2 | production lanes/factory factors/queue limits/graded power/placement/refund/depot tests; ra2_charging walks actual shipped unit costs/rates; powered Prism positive control; actual game minimap pixel/radar tests | Smoke and full final workspace uncompleted |
| RA3 | damage/weapon/layer/range/splash/crush/cells tests, mixed 64-unit crossing for 2,000 ticks with per-tick I6′ and twin hashes; schema and old-fixture neutrality controls | Final soak/benchmark not reached |
| RA4 | rank thresholds/multiplier measurements, kills-to-elite long battle, crews, real capture dock loss, ammo/bay/air/submarine tests; RA9 crew last subcell, exact readiness and paid pad supplements | Functional earlier receipts retained; exact lost-carrier limitation above |
| RA5 | ore/gems/mixed bails/growth/spread/max/one-bail-room; crate effects/limits/FreeMCV; actual tech capture/garrisons/bridge repair/collapse/repath and map setup suites | FreeMCV cargo gap above; full native matches not completed |
| RA6 | shipped infantry/vehicle suites exercise public deployment/interactions/strategic launches, due-tick defuse, radiation edges, Prism cap/timing, Curtain/Chronosphere boundaries, warnings, replay/hash state, transport and actorless Prop/privacy supplements | Lethal IFV and specialist multiplier gaps above; no broad original-engine fidelity credit from loaded flags |
| RA7 | actual App input/CommandSink/SimHost selection/cycle/T/P/group/preview/wall/tree/context tests plus sim guard/scatter/follow/force/waypoint, vision/sensor/Gap and moved-key tests; real event/minimap drawing and public notice lifecycle cases | Captured game unit tests passed in partial199; controlled UnitView/HoveredCell remains input setup, not actor creation/visual approval |
| RA8 | ra8_ai/temporal/placement and private modules cover real think/queue/economy/combat routes, current vision, naval/landlocked, superweapons/options, engineer, financing and difficulty premises. Difficulty regression 197c is an actual named one-case pass, not whole-suite proof | Earlier canonical isolated198 all21 Won and fixed3 timings retain exact scope; final756/soak/duels incomplete |
| RA9 | All five original top-level parity tests retained; actual loaded physical adapters, independent demands, full source-policy compositor, exact one-cause negative controls, portions/aliases/value/order/kind/class/dependencies/source drift | 197c whole-positive1/0/180filtered and six repairs6/0/175filtered are real; complete current181 target has not passed |

R6/I6′ was explicitly checked at the **test-design** boundary: three infantry or one surface vehicle, separate air occupancy, held/entering-cell claims, standing-point checks, deliberate corruption detection, older-fixture neutrality and scheduled crossing/crew/transport paths. This is not a proof that every production caller satisfies it; the simulation slices own those code-path findings.

R7 tests/audits use leptons-per-frame→ticks (3/4), not the superseded GI anchor; frame durations use4/3, minutes1200 and cells256. The integer conversion tests include750frames→1000ticks and other rounding boundaries. R29 remains72,000 times span/128 for M6, fixture36,000, duel300,000, ten-player400,000 and soak36,000. No subsequent cap raise was found in the reviewed harnesses. R56's soak is a fixed deterministic window and may finish Running; every completed outcome must be Won. All-to-Won claims come from distinct measured matches, not the soak window.

## Audit negatives and source/evidence interface

My own reachability scan found **48/48 audit Rust files reachable**, all 67 TSV/4TXT siblings accounted for in the source/data inventory, no missing module or literal include_str path, and **181 test declarations in the top-level ra9_data_parity closure** (five top-level plus176 in audit modules). A simple literal-include scan finds106 include_str occurrences; this differs from earlier graph counts because it counts occurrences, including test fixtures/repeated paths, rather than asserting106 unique artifacts. Merge residues do not become Rust modules.

I read the eight dedicated negative files and completion/portion admission controls, and traced mod.rs→actual adapters/demands→source-plans/routes→policy-admission/scope-inputs and final finish. Specific checks verified:

* Loaded cost/HP/range/nonfirst Verses mutations first load genuine source and require the same adapter's positive; loader errors are explicitly excluded as intended failures.
* Dedicated compositor/legacy policy negatives call the real complete audit baseline before wrong class/route/membership/source/admission removal; successful Compared witnesses/dependency elements are actually removed. Missing/foreign/duplicate acceptance census is checked after a valid full baseline.
* Completion174 uses full Atom identity:132Whole+42components (19/19/4), not151 interchangeable SourceKeys. Water/animation reorder, inactive-share activation, sibling/Whole/noncanonical portion, literal and UNACCEPTED edits fail from real successful neighbors; ORCA alias removal and fake read demand are tested.
* GameOwned argumentless zero/cross-kind ammo sentinels cannot be promoted into source Compared credit; witness absence/value corruption/false Compared substitutions are separately rejected.
* Owned mutation data copies use unique process/atomic names under the approved reviews cache; FIFO native tests use an owned child with bounded wait and kill+wait on unwinding. No detached blocked worker is credited as a successful boundary probe.
* The export hooks require actual loaded adapters and scope but only emit proposals. Reading a source, an alias, static branch pin, current-model absence or a loaded getter does not itself satisfy a source comparison/native acceptance. Static33 presentation and29 MODEL-ABSENCE routes remain structural evidence.
* Existing ledger-only synthetic tests and focused pair checks are not promoted to whole accepted-baseline/native proofs; the dedicated whole-baseline controls supply that separate layer.

I independently rehashed current195+196+196b source/binding manifest inputs: **669 files,0 mismatches;64 authority spans,0 mismatches**. That factual binding does not approve policy. I rehashed all four187 raw late-control logs against recorded byte counts/SHA and reconstructed the exact RNG and observer source transformations in memory: both hashes match the recorded mutations. Current selected test SHA equals the separately restored SHA `e7f3b317c98e63368ae0e0dba733d7ae55274dd080c18e200d6998b776e4e97d`. The only positive/restored187 snapshot deltas to current canonical inventory are the three later derived model/branch observation bindings; runtime source/data and test body remain equal. 166's audit Rust bodies are also unchanged at current TIP; later differences include actual AI/combat and the appended focused tests, so historical suite results still cannot be relabeled a current whole-suite run.

## Execution evidence and honest counts

No native/Cargo command was run by this reviewer. Existing execution was inspected, not recreated.

*158 is one whole-positive audit pass.159 is the actual unfiltered181-case run:175passed/6failed/0ignored/0filtered. The five166 helper/test corrections fix exact diagnostic paths rather than remove negatives: exact dock/radius errors, actual skirmish loader refusal and installed M60 record identity.168 and canonical197c each pass exactly those six cases with175filtered.197c's whole-positive separately passes1case with180filtered. **175 earlier passing bodies +6later named passes is not a completed181-case pass.**
*181 preserves the real duplicate MoveCompleted failure with actual WeaponFired/UnitDestroyed;182 runs the same test body after the minimal combat fix and passes. The yard's compiler/detour/neutral-building fixture failures remain distinct from the actual185pass (21maps/344scenarios, unchanged256-tick movement bound).
*187 positive and later restored-positive each pass1/0/6filtered, P0Won tick41,738 within72,000; actual late counts444WeaponFired/8ProductionComplete/6positiveOreDelivered, equal every-tick hashes/events/outcomes, final hash78c81bd069fca660/trajectory5a8adabd01d20b05. The extra RNG draw fails at native36001; observer-only production omission leaves the same winner/hash then fails missing late production444/0/6. Exact restoration reads and a separate later positive are retained.186 launched native debug then was interrupted; it is neither a pass nor compiler-only.
*198's isolated five commands have actual passed receipts:21native ten-player Won cases (all means<5ms, slowest147,715/400,000), three fixed24,000-schedule timing cases (still Running allowed), and three exact example completion guards. These are scoped earlier canonical results, not a substitute for final full13 or a guarantee about p99/max/busy windows.
*199 actually completes01fmt/02diff-check/03clippy with exit0 and guards, then is intentionally interrupted during04after SIM2's production P0. The captured M6 target shows six individual tests passed, including late twins; its756-job case has no final result block. Commands05–13 were not reached. Root's `round-1-live-evidence.md`, stop receipt and source-after-stop audit explain the stale RUNNING/WAITING controller metadata. No direct shell-child final exit was observed, so neither0nor101 is invented for the interrupted workspace command.
*Every one of36 historical summary files containing a completed04line was compared to its actual raw `test result:` sums; all reported passed/failed/ignored counts match. Historical source scopes and failing attempts remain intact.
*Raw harness totals include child executions. In current partial game output there are four1-pass child blocks, each267filtered, in addition to268primary game tests. The map FIFO test likewise launches four exact child executions if reached. The validator's awk intentionally sums result blocks; it does **not** measure unique primary tests. Prepared200/202 metadata's zero-filter assertion is incorrect and unexecuted, and must be corrected during closure without rewriting raw logs. No final total or primary-unique count is claimed here.

## Unverified scope and user-only checks

The final unfiltered workspace/audit run, release build, both600-frame smokes, benchmark, deterministic soak, full756duels, package and regeneration commands on the final corrected freeze remain required. The shared199 run stopped before the sim audit/native targets were reached. I did not run mutations/probes because the common instructions reserved native ownership, then root stopped validation on known bad source. The fresh remediation must supply native causal regressions and the complete required gate; this report is not that evidence.

I did not independently re-prove every original-engine numerical assumption or adjudicate every per-atom source-model difference. I checked the test/admission boundary and exact evidence scopes; data/simulation/client findings still need the fresh lead's independent whole-program reconciliation. Current headless production/late-twin worlds include semantic Props: load_map retains world_features in Grid, setup_match_configured calls world_objects::install, and Props::from_grid installs those features. I independently traced this after root identified the stale omission prose. The old m6 header/GAP statement does not describe current execution; see T-P2-2. These semantic objects do not constitute a rendered gameplay session or user quality approval. The original lost-carrier/flight timing limitation remains as stated above.

All visual/audio/animation/readability/pacing/feel, normal human victory/defeat and packaged/platform quality checks remain user-only and pending. They are not findings.

## Preservation

Only this assigned report is written by this reviewer, into the pre-existing designated reviews-cache directory. No clone/worktree/build/cache/dataset directory or other substantial directory was created outside the canonical repository, and no repository/worktree or evidence log was deleted. Configured Cargo/uv/pip directories were not changed. Frozen source and64authority spans remain unchanged.

