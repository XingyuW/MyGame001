# RA8 acceptance record

Status: seventh final-source checkpoint is ready for committed milestone checks. Current source
passes1201 preflight tests/81 target invocations,all21 retained exact winners,and21/21 actual
default ten-player winners. The fixed256×256 three-map mean budget passes below5ms. All13
frozen commands and both fresh756-winner R29 distributions remain required. This is validated
preflight,not independent review or completed RA8 acceptance. All earlier raw evidence remains.

## Acceptance checklist

- Country/side plans: eleven generated plans, complete tech, radar/airfield distinction,
  Battle Lab, second refinery, national specials, depot, purifier/vats, repairs and sourced
  defense counts. America paradrops remain independent of the major-superweapon option.
- Production: tier reserves, observed-air AA, dock-aware aircraft, reachable/hurtable naval
  targets, detector escorts and legal shipyard placement, engineers, garrisons and service bays use ordinary
  bounded commands. Artillery retains standoff behind the tank line.
- Strategy: known-target source weights, normal strategic production/recharge, vehicle-wave
  Curtain and explicit-source Chronoshift, behind-base paradrops and honest occupancy.
- Difficulty: exact ROF/build/start/income/wave/defense values, additive physical/virtual
  purifier accounting, unchanged first-hit damage and all weapon-ROF paths.
- Endings: non-starving R53 cadence, incapable/stranded remnants, deployed garrison recovery,
  explicit Won outcomes in all completed-match harnesses; no raised limits or delayed wins.
- Privacy/determinism: raw-spawn empty-command pre-AI concealment, hidden naval-blocker
  equivalence, rejected Guard/Mirage/crate RNG equivalence, all new state value-hashed.
- Compatibility: RA5 bridge behavior, RA6 utility/transient exclusions and paid Spy veterancy,
  RA7 commands, guard range, live-power and first-spawn/same-batch concealment regressions.
- Performance: measured immutable sharing and Prop membership with identical seeded digests;
  final-source ten-player 256×256 means pass below5ms and all21 maps have explicit winners.
- Validation: serial relevant preflight, all thirteen frozen commands, both756-winner sweeps,
  fixed soak, package/smokes and before/after match-length median/p90/max remain to close.

## Policy and evidence

R88–R98 in `docs/DECISIONS.md` record the implementation, exact existing-test migrations,
game-owned AI assumptions, failure diagnoses, negative mutation and process deviations.
Source difficulty and target weights come from the audited reference table. Closed-source
AI policy is not claimed: composition, tier reserves, target-category mapping, garrison/depot
radii, artillery standoff, naval rescan and strategic offsets are declared data assumptions.

The default game path is `setup_match` with actual `MatchOptions::default`, no Super Me or
unlock players. m13 uses that path for ten normal Medium controllers; production/duel sweeps
also exercise real shipped definitions. Isolated capability scenarios declare their terrain,
credits and vision setup and do not replace the default-path acceptance runs.

No clone, worktree or validation environment was created. Configured Cargo/UV/PIP caches are
preserved. New recreatable parser scratch is at configured Cargo target `tmp/ra8_definitions`
(716 KiB measured after the slow-speed parser fixture;58 files/584038 file bytes), retained for reruns and unnecessary for release.
Earlier RA5 render and RA7 parser evidence remain.

## Ten-player benchmark preflight

Serial preflight3 passes 157 tests across 15 selected targets; the subsequent detector-escort
batch passes 119 tests across 5 targets, including all 22 RA8 controller scenarios, required
policy/parser/hash cases and the independent source audit. These batches overlap and their
counts are not added together. Both unchanged RA4 AI regressions pass separately after the
unrelated cursor change is removed. The exact Coral LibyaBrutal/IraqMedium seed0 regression
has an explicit winner at 28,041/288,000 ticks; full frozen sweeps remain required.

`cargo run --release -p sim --example m13_ten_players -- liberty_bay continental_divide pacific_atoll`
exits 0 using actual default options and no cheats. Each map measures 24,000 ticks, has 256×256
cells, and all ten players build. Raw evidence: `ra8-preflight/ten-player256-benchmark1.log`.

| Map | Mean ms | p99 ms | Maximum ms | Busiest 1000 mean ms | Peak units/buildings |
| --- | ---: | ---: | ---: | ---: | ---: |
| Liberty Bay | 3.430 | 11.800 | 103.346 | 5.194 | 335/109 |
| Continental Divide | 1.969 | 4.655 | 29.034 | 3.150 | 329/104 |
| Pacific Atoll | 1.066 | 4.242 | 33.452 | 1.965 | 318/85 |

The required mean budget is 5 ms. Liberty Bay has two ticks above 50 ms; no maximum-tick
guarantee is claimed. These timed benchmark cutoffs are not completed-match evidence.

Independent review, user visual/audio/play-feel verification and RA9's R83 transport
Size/SizeLimit/loaded-boarding plus attached Squid-host WeaponFired debt remain pending.
R96 also requires RA9 to reconcile actorless occupied-Prop C4/Ivan/Temporal resolution
boundaries; physical building classification is not a claim of actor-backed ability support.
R97 records the exact queued-AI casualty race (tick12391 Attack Dog202 dies after AI decides;
the next Commands rejects its stale AttackMove). Its mechanism is closed in R103; public validation remains strict.
R99 also carries a source-only, unproven RA9 lead: hypothetical building traits in
`ai/strategy.rs::damages_buildings` default bombable=false and may undercount charge-planter
or controlled charge-recruit capability. Root explicitly defers that broader audit to RA9.

## Frostline and cleanup follow-up

The first all-map attempt fails Frostline at400000 and is retained with partial subsequent
Oasis output. Traces refute the initial occupied-garrison hypothesis for that stall: held
Yuri AttackMove markers prevent otherwise capable waves from resting. The focused lifecycle
fix preserves control and explicit retargeting while finishing/resuming existing movement.
The separate impossible Yuri→occupied-building control-shot regression passes after shared
physical traits. Negative and failed-to-fail probes remain; R95–R97 explain each.

The affected boundary batch passes90 tests; after the equivalent Prop-membership cleanup,
the expanded boundary batch passes175 tests (52 core,23 RA5-world,30 RA6-infantry,32 RA6-vehicle,
1 RA7-context,12 RA7-orders,25 RA8-AI), zero failures/ignores. Counts overlap earlier batches.
Both original bridge tests load the explicit original ordinary-ground neutral AI fixture;
every original assertion and72000 limit remains. There are1160 static tests under crates,
including three example winner-helper tests; full frozen executed counts remain pending.

Exact unsampled default ten-player Frostline seed13 now exits0: Won(P6) at113712/400000,
113713 ticks measured, mean687.947µs, measured total78.229s/elapsed78.237s, p994.115ms,
max105.489ms, all10 players build, peak320 units/141 buildings. The separate full all-map
rerun remains required. [Prop cleanup measurement](RA8_PROP_PERFORMANCE.md) records identical
5000-tick state and660-event digests and measured1.079447ms→348.383µs mean; final runtime
benchmark and gate timing are measured independently.

## Post-cleanup 256×256 mean-budget evidence

The repeated unsampled three-map benchmark exits0 after the lifecycle/classification/cleanup
fixes and before the final R99/R100 corrections; each map runs
24000 normal scheduled ticks with actual default options and all ten players building.
Raw capture: `ra8-preflight/ten-player256-benchmark2.log`.

| Map | Mean ms | p99 ms | Maximum ms | Busiest1000 mean ms | Peak units/buildings |
| --- | ---: | ---: | ---: | ---: | ---: |
| Liberty Bay | 0.752960 | 4.213 | 50.781 | 1.512 | 335/108 |
| Continental Divide | 0.609479 | 1.955 | 15.993 | 1.123 | 329/104 |
| Pacific Atoll | 0.612278 | 2.470 | 19.233 | 1.055 | 318/85 |

All means are below the unchanged5ms budget. Liberty Bay has one tick above50ms; no maximum
guarantee is claimed. These cutoffs are benchmark evidence, not completed-match evidence.

The complete `ten-player-all-end2.log` exits0 with21/21 explicit winners and all ten players
building on every map. Maximum ending tick296072 (Arctic Floes), below the unchanged400000
limit. This successful sweep predates the final two narrow corrections and is retained as
evidence; final-source benchmark/all-map repeats remain required before the frozen gate.

R99 proves and fixes premature liquidation before a useful visible infantry capture, with
ordinary capture plus actual building damage, immunity/incapability/terrain-isolation cases.
R100 proves and fixes aircraft being chosen as the artillery front line. All27 RA8 scenarios
pass in `final-capabilities1.log`; static test count is now1162. The final serial preflight,
final-source default runs and thirteen-command gate must still close this record.

## Final-source pre-freeze evidence

`final-serial-preflight1.log` exits0:236 passed,0 failed,0 ignored across14 selected targets,
including unchanged RA4 AI and bridge-route assertions, all27 RA8 scenarios and source audits.
`final-clippy1`, `final-fmt1`, `final-generated1` and `final-reference1` exit0, the latter
confirming4934 reference rows. The final profile retains the exact before/after state/event
digests, with350.554µs mean in its separate5000-tick workload.

The final-source unsampled 256×256 benchmark (`ten-player256-benchmark3.log`) exits0:

| Map | Mean ms | p99 ms | Maximum ms | Busiest1000 mean ms | Peak units/buildings |
| --- | ---: | ---: | ---: | ---: | ---: |
| Liberty Bay | 0.749938 | 4.209 | 52.834 | 1.476 | 335/108 |
| Continental Divide | 0.619921 | 1.946 | 15.166 | 1.153 | 329/104 |
| Pacific Atoll | 0.605007 | 2.392 | 19.353 | 1.074 | 318/85 |

Each runs24000 ticks and all ten players build. All means are below the unchanged5ms budget;
Liberty Bay has one tick over50ms, with no maximum guarantee claimed.

The final-source `ten-player-all-end3.log` exits0:21/21 explicit Won outcomes, all ten players
build on every map, every mean below5ms. Ending median54329/p90 nearest-rank106700/max260363
(Pacific Atoll), within unchanged400000 outer limit. Measured total822.836s, elapsed822.946s.
Frostline Won(P6) at128289, mean647.710µs, elapsed83.104s. The actual normal setup path,
per-map winners/ticks/budgets and final256 metrics are in
`ra8-preflight/checkpoint-m13-metrics.json`; these ten-player statistics do not replace the
required two-player R29 before/after ratios from both frozen756-match sweeps.

Full-tree inventory (`checkpoint-test-inventory.json` and its check2 log) compares RA7
f5505cd to the current tree:1124→1162 static tests,38 added,0 removed/renamed,0 ignores in
either tree. Every attribute maps to a named function. Its initial regex missed the same
unchanged multiline should_panic attribute in both trees; the corrected inventory records
that diagnostic explicitly. Intended existing-test fixture/expectation migrations remain
exactly listed in R94/R96. No original assertion, limit or valid win is weakened/delayed.

## Frozen milestone checkpoint

Feature commit `bb1f87e9496236ecf3bcdcd797fd1446f732d203`; separate measured cleanup/frozen runtime
`81149b64e6ce30929f0a94dd47b92f8cf6226c13`. Earlier R88/R89/R90 commits remain intact. The
full thirteen-command gate has not started; root explicitly holds it for required milestone
checks of these committed artifacts. `ra8-preflight/checkpoint.json` records the exact next
command, metrics/inventory/ledger paths, source hashes, storage and RA9 handoff. This is a
validated preflight checkpoint, not independent review or a completed RA8 gate.

Separate staged checks did not pass because raw captures are preserved: feature exit2 has
32 classified diagnostics (22 blank EOF lines,10 Coral trace trailing-space lines); cleanup
exit2 has the sole `prop-boundaries-after1.log:219` EOF line. Exact paths/lines and unfiltered
output are retained in the feature/performance staged-whitespace JSON manifests. Separate
rechecks confirmed identical sets before their commits. No source/data/test diagnostic,
suppression or raw-log edit occurred; the required unfiltered validation script is unchanged.

## Test-only refreeze after the first gate failure

The historical checkpoint above is retained. `ra8-attempt1/STOPPED.md` maps the failed/incomplete
first gate:01–03/05–06 exited0;04 exited101 (216 passed,1 failed,0 ignored);07 partial compilation
was stopped, outer exit143;08–13 were not run. Raw logs and summary remain unchanged.

R102 names the sole existing-test migration:
`game::units::tests::the_production_data_loads_with_the_combat_file` checks the complete plan's
armed defences and pillbox membership across both build_order and defences. It keeps every
original armed/combat requirement. The new actual-controller test proves paid armed placement
under all eleven shipped plans, including the separately declared lists under all nine country
overrides and the original build orders under both isolated side fallbacks. Earlier new-probe
mistakes and the successful placement trace are preserved in the plan-defence captures.

Only test code changed after runtime `81149b6`; the actual default21-winner sweep and final
256×256 timing/equivalence evidence remain final-runtime evidence. Refreeze inventory is
1124→1163 static tests,39 added,zero removed/renamed/ignored. The new complete frozen gate,
both756-winner distributions and final command timings remain required before RA8 closure.

## Admission and lifecycle correction after broader preflight

The preceding test-only checkpoint is historical and superseded by R103's runtime fixes.
The complete failed `refreeze-game1.log` preserves all actual assertions: occupied Soviet bays,
same-decision aircraft over-admission and queued target invalidation. The original Crossroads
functions now pass unchanged, the second at21153/72000 after accounting for incoming garrison
reservations. A new focused default-path test replays all ten failed Demo pairings with their
original rejection exemptions/invariants/72000 limit; all ten have explicit winners in33.84s.

Internal AI intents carry value-hashed decision witnesses, with FIFO and ordinary validation;
public push is strict even for an actual AI owner. Mixed/all-stale actors, real detector-loss
concealment, target death/capture, retained intrinsic errors and Deploy boundaries are covered.
The final affected serial boundary batch (`refreeze-boundaries2.log`) exits0:115 passed,
zero failed/ignored across core57, public commands22, RA8 AI31, definitions4 and placement1.
The aircraft admission negative test fails exactly with two orders for one remaining dock;
its sole condition mutation is restored and byte-verified in the integrity JSON. Its original
SHA is explicitly reconstructed rather than claimed contemporaneously captured. No old test,
assertion or limit is weakened; R102 is the only additional existing-test migration.

Runtime now differs from81149b6; final default21-map completion and256×256 means must repeat.
Earlier seeded Prop-optimization equality remains evidence for that isolated optimization;
the new queued-command state format intentionally extends hashing. R97's lifecycle mechanism
is fixed here because it became a concrete gate failure, rather than deferred as known broken
default play. R83/R96 specialist/transport and R99 unproven capability leads remain RA9 work.
No new external directory or schema was introduced by this correction.

The subsequent unchanged compatibility batch passes110 tests and source audit5, bringing the
disjoint corrected sim preflight to230 passed/0 failed/0 ignored across13 targets. Strict
all-target Clippy, fmt, generated data and4934-row reference checks pass. Both original Coral
functions pass at31367/288000 (LibyaBrutal/IraqMedium) and33213/288000
(LibyaBrutal/GermanyBrutal). `refreeze2-test-inventory.json` records1124→1172 static tests,
48 added,zero removed/renamed/ignored. Final-source default-match/budget repeats and all13
gate commands remain pending; no independent review is claimed.

## Corrected-runtime refreeze checkpoint

Runtime `d840b44797a81b66ae284d0bcb623499c6cc8a86` is unchanged through both repeated default-path
runs. `ten-player-all-end4.log` exits0 with21/21 explicit winners and all ten players building
on every map; all means are below5ms and all endings satisfy the unchanged400000 limit.
Ending median63573/p90 nearest-rank106700/max186405 (Arctic Floes); measured744.679s,
elapsed744.791s. Frostline Won(P6) at63573, mean902.399µs/elapsed57.375s. Pacific Atoll Won(P3)
at66076, mean508.810µs. No cheat or alternate setup path is used.

The final `ten-player256-benchmark4.log` exits0, each map24000 normal ticks/all ten building:

| Map | Mean ms | p99 ms | Maximum ms | Busiest1000 mean ms | Peak units/buildings |
| --- | ---: | ---: | ---: | ---: | ---: |
| Liberty Bay | 0.739225 | 4.269 | 52.777 | 1.495 | 337/107 |
| Continental Divide | 0.628747 | 3.346 | 48.205 | 1.093 | 332/104 |
| Pacific Atoll | 0.560533 | 2.287 | 31.618 | 1.070 | 318/84 |

All means satisfy the unchanged5ms budget; Liberty has one tick above50ms, so no maximum
guarantee is claimed. `refreeze-m13-metrics.json` retains all per-map fields and exact runtime.

Witness validity source explanation: `ai.rs::View::new` filters own_units through
`army_commandable` before witness capture. It excludes spawned/flights/erasures/deployed,
interacting/docked/rearming/disabled units. Recoverable occupied Props use the distinct Deploy
branch. The parent checked this concern and confirmed the filter already resolves it; no
source mutation or interruption of the healthy sweep was needed.

`checkpoint2.json` records1172 static tests/48 added/zero removed/renamed/ignored, exact
migration/evidence/storage/handoff paths and both RA7 R29 baselines, with the final two756-match
distributions still pending. R97's lifecycle mechanism is fixed in R103; R83/R96/R99 handoffs
remain. No new external directory was created; parser scratch remains708KiB. The gate is held
for required parent milestone checks of committed artifacts before the exact all13 rerun.


## Second frozen failure and concrete recovery corrections

The preceding corrected-runtime checkpoint is historical. `ra8-attempt2/STOPPED.md` and
`captured-sha256.json` preserve the complete untouched failure: workspace04 exited101 after
4512s (253 passed/1 failed/0 ignored); production target4156.59s, five companions passed.
01–03/05–06 exited0;07 partial compilation stopped, outer143;08–13 did not run. No successful
756-row production table was emitted. Its older configured-target table is not copied or
claimed. All eight captured raw files reverify against their stop-time SHA256 manifest.

R105–R108 record the six exact seed0 no-winner pairs and three ship-blocked repair rejections,
actual surviving resources/forces/queues/objectives, distinct diagnoses and ordinary-command
fixes. New nine-case `ra8_gate_matches` preserves original R29 limits, command rejection
exemptions, credit/cell/stray/queue invariants and requires Won. Before the final legacy-boundary
correction, all nine exact default replays and the existing ten-pair Demo replay pass; Liberty
wins at143617/288000. Final-source default evidence is repeated before the next freeze.

Known live/departing/reserved/imminently arriving naval occupants defer bridge repair; an idle
adjacent ship does not. The nine-mode schedule probe checks actual movement/reservations and
live Engineers, real concealment and identical emitted hidden-twin planning before Interact.
Public R67 repair refusal is unchanged; no unknown ship influences the decision. Recovery of
incomplete paid queue heads uses economy's exact physical producer/prerequisite predicates,
ordinary charged-credit refund and no temporary power/funds/capacity shortcut. Completed heads
retain placement and FIFO. The actual aircraft reload cycle passes; aircraft code is unchanged.

Observed scouting objectives can complete without every crowded/slow member becoming idle.
Their actual-observation continuation searches nearby stale ground after the initial enemy-start
search, with deterministic ordering and no invented remote-target knowledge/new persistent state.
A real Engineer-captured remote refinery remains unseen to the Kirov until its search discovers
it. The old policy fails at144000; corrected search wins at35202. Known-building attacks and
legitimate unobserved travel remain. An over-broad intermediate phase rule broke the paid naval
escort test; the actual-visibility predicate restores its existing assertion/deadline.

The neutral fog-on RA4 regression then caught a missing R91 opt-out on these new strategy
branches: winner5028 violated its unchanged patience lower bound. Bounded source-restored
probes exclude queue recovery and isolate scouting; the documented rules_difficulty strategy
boundary restores winner8211, all assertions unchanged. Only diagnostic output/assertion text
is added to that existing function. No fixture setting, duration bound or valid shipped win
is altered/delayed. All prior migration entries remain R94/R96/R102; there is no new existing
assertion/fixture migration. Raw failed probes and contemporaneous restoration hashes remain.

Corrected disjoint serial sim preflight passes236 tests across13 targets, zero failed/ignored:
core57, public commands22, RA4 AI2, RA5 world23, RA6 infantry30/vehicles32, RA7 context1/orders12/
vision10, RA8 AI37/definitions4/placement1 and audit5. Full-tree inventory now1124→1187,
63 added,zero removed/renamed/ignored; all test attributes map to named functions. The failed
first serial preflight and both bounded negative/probe logs are retained. Final companion,
strict lint/data, default m13/budget and the complete frozen thirteen-command gate remain
required. No independent review is claimed and no new external directory was created.

Strict all-target Clippy, fmt, generated-data and4934-row reference audit now pass; the sole
new-test clone_on_copy failure and corrected exact test pass are preserved. All five original
production companions pass unchanged (raw attempt2-companion captures).


## Recovery-runtime refreeze checkpoint

Correction runtime `65fa8d669ac1e14adadc30b2745887071b580bec` is unchanged for all final-source
evidence. `attempt2-game-exact-final2.log` passes10 functions/19 default pairings, every match
Won; all nine previously failing pairs have identical winners/ticks to the earlier true-strategy
run. All original invariants/rejection exemptions/R29 limits remain. R109 preserves the exact
results and the failed game --lib command alongside the correct217-test binary run.

`ten-player-all-end5.log` exits0:21/21 explicit winners, all ten players building on every map,
every mean<5ms, unchanged400000 limit. Ending median55554/p90 nearest91393/max331947 (Oasis).
Measured721.741s/per-map elapsed721.845s; command726.132s includes compilation/setup. Frostline
Won(P6)53261, mean908.996µs/elapsed48.419s. Oasis's longer ending remains visible; no timing
workaround is applied. Normal default setup, no cheat/unlock, is used throughout.

The final unprofiled `ten-player256-benchmark5.log` exits0, each24000 ticks/all ten building:

| Map | Mean ms | p99 ms | Maximum ms | Busiest1000 mean ms | Peak units/buildings |
| --- | ---: | ---: | ---: | ---: | ---: |
| Liberty Bay | 0.867841 | 5.049 | 58.054 | 1.840 | 337/107 |
| Continental Divide | 0.602632 | 1.925 | 50.904 | 1.172 | 332/104 |
| Pacific Atoll | 0.704461 | 4.170 | 34.359 | 1.310 | 318/84 |

All means satisfy the unchanged5ms budget. First two maps retain1/2 ticks over50ms; no maximum
guarantee is claimed. `attempt2-m13-metrics.json` contains every map, budget and command result.

`checkpoint3.json` maps1187 static tests/63 added/zero removed/renamed/ignored/unmapped,
236-test/13-target disjoint sim preflight, five unchanged production companions, all strict
lint/data checks, exact R94/R96/R102 migration ledger and R108 compatibility diagnosis. No
source/data/script edits follow65fa8d6; View's army_commandable witness filter remains intact.
Both failed frozen attempts and all failed probes remain. Both756-winner final sweeps/R29
comparisons and all13 commands remain pending; the committed checkpoint is held for the
parent's required milestone check before launch. Validated preflight, not independent review.

R83/R96/R99 remain RA9 handoffs; R97's mechanism is closed by R103. R91/R92 AI policy/source
mapping and R67 bridge assumptions, earlier programme debt and user visual/audio/play-feel
verification remain explicit. No new external directory: configured Cargo parser scratch
708KiB is retained/recreatable/not required for release, with earlier RA5/RA7 evidence preserved.


## Third frozen failure: strengthen the legacy economy observation

`ra8-attempt3` preserves the complete third gate failure: workspace04 exit101/331s,
227passed/1failed/0ignored; m10 target162.84s.01–03/05–06 exit0;07 debug smoke stopped,
outer143;08–13 not reached. All eight raw capture SHA256 values reverify unchanged.

R110 names `several_independent_computer_players_play_complete_matches_on_every_map` and
`several_players_play`: the old single tick3000 balance comparison missed Player5 despite
1053 actual changes, paid2800 production and FOUR700-credit deliveries, because its balance
returned to5000 at executed tick1382. Exact credit history/economic events and immediate
source-probe restoration hashes remain in the attempt3 preflight captures. Actual economy
activity is proven, so runtime65fa8d6 stays unchanged. The helper now records actual movement
from each initial balance through the SAME3000 checkpoint; the final all-player requirement
and every independence/starting-yard/winner/replay/limit assertion remain intact. No fixture,
valid win or timing is altered. This is a named observation-strengthening migration, not a
new exception. Static inventory stays1187 (+63,none removed/renamed/ignored); no temporary
probe test is retained. R109's actual final-runtime m13/256/default evidence remains applicable.
The full affected target/strict hygiene and all13 frozen commands remain required.

Full affected m10 target now passes5/5/0failed/0ignored, all original maps/replays,156.648s
command elapsed. Strict all-target Clippy/fmt pass. `checkpoint4.json` records the unchanged
65fa8d6 runtime and repeated default evidence; all13 gate commands still must rerun.


## Fourth gate failure and final-source correction checkpoint (R111–R114)

`ra8-attempt4` preserves the frozen4dfeeed/runtime65fa8d6 failure: workspace04 exits101/
2703s,253 passed/1 failed/0 ignored; production target5 passed/1 failed in2497.71s. The three
seed0 failures are Steel Harbor AmericaBrutal/CubaBrutal bridge readiness, Thousand Lakes
GreatBritainEasy/AmericaEasy strategic-cell visibility loss, and Thousand Lakes FranceBrutal/
GermanyBrutal active attrition with no winner at unchanged288000.01–03/05–06 exit0,07 stopped,
08–13 unrun. No successful756-row table is claimed; raw hashes and partial outputs are retained.

R111 traces establish hidden Typhoons already reserving the deck at decision, so a larger
admission guess/false decision-clear witness cannot solve it. R112 adds an explicitly tagged,
value-hashed private conditional repair goal: validate Engineer/known hut/type/collapsed job
first, wait without controller feedback when physically blocked, otherwise dispatch ordinary
Interact after preceding FIFO effects. Public Interact retains its original occupied-deck
rejection. Actual repair still checks R67 current/reserved Naval holders before restoring or
consuming an Engineer. Scheduled tests cover hidden occupancy→clear→actual repair, occupancy
after acceptance, prior/later Stop, invalid/dead/hidden actor/target and action hash. An actual
controller hidden-ship twin proves identical decisions/witnesses/goal queue hash.

R113 extracts full read-only strategic validation and certifies only completely valid engine
intents at core Ai end. Only a later exact cell-visibility rejection may cancel, and only when
all other current checks still pass ignoring visibility. Real scheduled Gap shrouding tests
retain strict public/initially invalid/later unready/source-form/map/argument behavior and
prove certificate value hashing. Both new queued state tags are included by value, with no
pointer/refcount/random appearance contribution.

Lifecycle-only exact replays: Steel Harbor winsP1 at48279; GreatBritain/America winsP0 at78811;
France/Germany still None288000. A read-only economy probe proves actual late ore deliveries,
paid production/repair/cancellations, losses and meaningful combat. Actual empty aircraft are
returning/reloading; the real late Storm is not a timing exception. No forced liquidation or
limit/valid-win timing change is justified. The controlled AA scenario fails before when
Harriers attack an economic building through a known longer-range Patriot, and passes after:
eligible ground explicitly hits/destroys AA, then aircraft resume real economic-target fire.
The isolated economic target's explicit durable HP prevents premature observation-target loss;
no normal game receives it. The resulting exact default France/Germany replay winsP1 at164850/
288000 (112.08s harness/147.064s command), without changing any public rejection exemption.

The observed-AA target/approach rule is a game-owned AI policy assumption derived from actual
weapon eligibility/armour/positive damage/source ranges/air bonus and existing gather/failure
policy. It adds no balance quota/weight/radius/schema, changes no source defense/difficulty/
economy values, and preserves actual empty-aircraft return/reload. Two single negative probes
fail the intended bridge and strategic assertions, immediately restoring command.rs to exact
SHA256 f34246f19382754be318b576cbaf19a11245768dfff3543f99c9240ef77acfe4 before further work;
raw failures/mutations/distinct hashes/restoration evidence remain in integrity JSONs.

All14 RA8 internal lib tests pass; strict all-target Clippy/fmt/generated/reference checks
pass after a retained needless-borrow diagnostic is corrected. Static inventory1197 versus
RA7's1124:73 added,none removed/renamed/ignored/unmapped. No existing assertion/fixture/limit
migration is added after R110; prior exact migration ledger R94/R96/R102/R110 and neutral
strategy compatibility R108 remain. Earlier65fa8d6 m13 metrics are historical after these
runtime changes; final-source21-winner and256-mean evidence must be repeated before freeze.
All previous probes/attempts and original validation logs remain. New external directories:
none; the retained708KiB configured-Cargo parser scratch and prior RA5/RA7 evidence remain.


The completed affected serial preflight now passes483 tests across22 targets,zero failed/
ignored:243 sim,217 game unit,Demo one test/ten games,twelve exact maps,m10 five,and five
unchanged production companions. See `ra8-preflight/attempt4-final-preflight-summary.json`.
All twelve exact cases have explicit winners; maximum242941 remains below its unchanged
288000 limit. R114 records longer outcomes transparently. No existing migration/assertion/
rejection exception is added after R110. No validator is active at the source-commit point;
final-source m13/all21,256-budget and all13 frozen commands remain required.


## Fifth committed freeze checkpoint (R115)

Final runtime: `bbb09feab392b31e6d627d0060586c73b100e6db`; lifecycle correction:`3b81f76`.
`ra8-preflight/checkpoint5.json` carries1197 static tests/73 additions,zero removals/renames/
ignores,483 passing affected tests/22 targets,strict checks,precise old-test migration ledger,
all failed attempts,negative source restoration evidence,assumptions/debt/storage and the
unchanged before-R29 distributions. No source/assets/scripts/dependencies change afterward.

Repeated actual default ten-player `-- to-the-end all` exits0,21/21 Won,ten builders each,
every mean<5ms. Endings median56614/p90108259/max288884 below unchanged400000; command918.242s,
measured914.394s/per-map elapsed914.567s. Prior65fa's median/p90/max55554/91393/331947 are
preserved: median/p90 rise while max falls. Raw `ten-player-all-end6.log` and structured
`attempt4-m13-metrics.json` retain all fields,including all-map longest tick290.853ms.

Exact fixed24000-tick256×256 unprofiled normal-game benchmark exits0,ten builders per map:

| Map | Mean ms | p99 ms | Maximum ms | Busiest 1000 mean ms | Ticks above50ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Liberty Bay | 0.842308 | 4.762 | 109.899 | 1.790 | 2 |
| Continental Divide | 0.623839 | 2.023 | 113.223 | 1.150 | 4 |
| Pacific Atoll | 0.780129 | 4.540 | 76.099 | 1.418 | 2 |

The unchanged mean budget is5ms. There is no maximum-tick or smooth-play guarantee. Raw
`ten-player256-benchmark6.log` and default-results JSON retain command54.550s and all values.
No Super Me/unlock,manual injected HP or altered startup options enter these normal runs.

Review scope explicitly includes R112's private conditional goal/authorization/FIFO/actual
restoration safety and R113's complete strategic decision certificate/narrow cell-loss
cancellation. Public Interact remains strict. R114's AA target/approach algorithm is a declared
source-derived game-owned policy,with no new balance quota/weight/radius/schema. All prior
assumptions and RA9 R83/R96/R99 handoffs remain; R97's mechanism is closed in R103. Independent
review and user visual/audio/play-feel verification remain. No new external directory; retained
configured-Cargo708KiB parser scratch is recreatable and unnecessary for release. Full13 and
both756-winner final R29 before/after distributions still remain required before RA8 closure.


## Sixth committed runtime checkpoint (R116–R123)

Runtime: `d1aa0f09bb7b69cb3827c2a79acdbbb5cdac66a3`. Final serial preflight passes512 tests across22 targets,zero failures or
ignores:265 simulation/13 targets,217 game unit,one demo harness/10 wins,five multi-player,
five original ending companions,and19 retained exact seed0 wins. All seven fifth-gate failures
and retained Thousand Lakes France/Germany win again on final R122 source. No existing test
assertion,limit or rejection exemption changes. Full-tree inventory is1226 vsRA7's1124 (+102),
none removed/renamed/ignored. Strict fmt/unfiltered diff/Clippy/generator/reference pass.

Actual default ten-player all-map run:21/21 explicit winners,ten builders each,all means<5ms;
median65538/p9090705/max99602 against unchanged400000. Command871.883s,measured866.706s,
map elapsed866.824s. Prior checkpoint5's56614/108259/288884 remain historical:median rises,
p90 and maximum fall. Raw seventh-run logs and attempt5-final-m13-metrics.json retain all data.

| Fixed24000-tick256×256 map | Mean ms | p99 ms | Maximum ms | Busiest1000 mean ms | >50ms ticks |
| --- | ---: | ---: | ---: | ---: | ---: |
| Liberty Bay | 0.938850 | 5.434 | 124.145 | 1.819 | 4 |
| Continental Divide | 0.783365 | 2.951 | 119.145 | 1.393 | 4 |
| Pacific Atoll | 0.770953 | 4.803 | 73.141 | 1.284 | 1 |

The mean budget remains5ms; no maximum-tick/smooth-play guarantee. Fixed benchmark command
60.300s. Normal runs use actual default options and no Super Me/unlock/HP injection. Final
339-file runtime/assets/scripts/examples/dependency manifest is unchanged through these runs.

Review scope includes R116 same-AA progress/reinforcement failure,R117 service exclusion versus
support eligibility,R118 honest forward endpoints (not global route safety),R119 genuine
no-income service release,R120 frozen mission/whole-group known-fire endpoints,R121 complete
later-erasure revalidation and R122 deferred casualty retreat. All prior policy assumptions,
R83/R96/R99 RA9 handoffs remain. R97's actual lifecycle mechanism is CLOSED in R103. Public
validation stays strict. Negative probes/insufficient evidence and exact restoration records
remain; no old failed gate is reclassified as passed. Full13 plus fresh756-winner production
and duel before/after R29 metrics remain required before closure. Not independently reviewed.

Parser scratch is freshly measured58 files/584038 file bytes/733184 allocated bytes (716KiB),
retained recreatable for reruns,unnecessary for release. No other new external directory or
substantial tmp artifact;configured Cargo/UV/PIP caches and RA5/RA7 evidence are preserved.

Runtime commit's separate staged unfiltered whitespace check exits0. Checkpoint evidence's
separate unfiltered check exits2,not passed:71 exact preserved raw-log EOF/trailing-space
diagnostics,all classified by path/line/raw-file and lineSHA in checkpoint6-staged-hygiene.json.
No source/test/data/document diagnostic is present. Raw bytes are retained;no suppression.
The exact final staged set must reproduce the same stdoutSHA before a separate commit.


## Seventh final-source checkpoint (R124–R131)

The sixth frozen gate stopped after its completed workspace M0 schedule failure; raw gate
and partial build output are preserved in ra8-attempt6. Later complete failed/insufficient
preflights remain under ra8-preflight/attempt6-*. No old failure is reclassified as a pass.
R124–R130 record the exact diagnoses,source/fixture boundaries and all before/after evidence.

Current strict fmt,unfiltered working diff,workspace Clippy -D warnings,generator and reference
checks all exit0. Current all21 retained exact cases explicitly win under unchanged original
limits/rejection rules:2093.413839s,raw attempt6-retained-all7.log SHA44b8b88cba79795d64a56713edaea533122747e1b5e7421d9de9fd749939bf94.
Their median66530/p90107940/max254201 are a focused-regression distribution,not the required
756-pair acceptance distribution or ten-player distribution. Current complete simulation
all-targets passes952/0failed/0ignored across72 targets,976.514759s,raw attempt6-sim-all7.log
SHA35b60447bea23a082dc56358138b87d9980c06b96a4254976b08d069c68f04f6.
All ten cheap default Demo pairings independently pass on this same R130 runtime in
attempt6-demo-subset-all1.log. Remaining game compatibility and both m13 runs also complete as recorded below.

Whole-tree inventory4 counts1238 versus RA7's1124:114 added,none removed/renamed/ignored/
unmapped. Runtime/assets/scripts/examples/dependency manifest5 has339 files,SHA132667c0730673308068963e09047ad8d8a118fa823212ed8e90b4db5bd47d27.
No source/data/script expansion occurs during healthy validation. This manifest is
rechecked at the committed checkpoint; previous d1aa0f0 ten-player metrics are historical.

Exact new existing-test migration/strengthening ledger:

- R124 shared helper sim_app_consuming_randomness explicitly orders its one-RNG-per-tick
  injector after public SimCoreSet(Ai),whose exclusive certifier caused the ambiguity.
  same_seed_consuming_randomness_gives_equal_hashes_at_every_tick,
  different_seeds_consuming_randomness_give_different_hashes and
  hash_reflects_consumed_randomness retain every original seed/tick/hash assertion.
  The unordered-system ambiguity error test remains unchanged.
- R125 shared M2 helper compares initial equality before enqueue,requires unequal hashed
  pending state,then retains every rejection/event and30-posttick equality assertion in
  a_move_outside_the_map_is_rejected,
  commands_for_missing_foreign_or_repeated_units_are_rejected and
  validation_checks_each_named_unit_in_turn. No hash is weakened.
- R126 a_computer_player_builds_in_both_lanes_of_its_yard retains its actual default setup,
  all assertions and6000 bound; only comments/diagnostic observation change. Runtime
  candidate selection stops a locked later-tech entry monopolizing the legal defence lane.
- R128 ra8_aircraft_complete_real_ammo_return_and_reload_before_ai_retasks_them strengthens
  its shot-count break/final predicates from>=2 to greater than the actual source burst.
  Two Harrier shots are one Burst,so this proves actual firing after refill. Every original
  empty-ammo/uninterrupted-reload assertion remains. No ammo/range/recharge/ROF value changes.
- R129/R130 add regression functions;no existing assertion,fixture,limit or rejection
  exemption changes. Earlier exact migrations R94/R96/R102/R110 and neutral-strategy
  compatibility R108 remain binding and are not replaced by the new defaults.

R127 uses observed source weapon eligibility plus static-terrain firing reach for home
threat admission,ordinary healthy Attack and the current-position AirRangeBonus versus
plain-range chase distinction. R129 stops futile paid repair through the ordinary toggle
when no conservative owned finishing/income/production recovery opportunity exists;R53's
external credit-change signature,patience and cadence are unchanged. R130 gives eligible
airport-bound actors their own bounded strike job inside an escorted mixed wave,while
retaining the original surface/undocked-air/artillery policy. These are declared game-owned
AI policy assumptions using existing source capabilities/commands,not closed-AI claims.
No new quota,timer,schema or persistent state is introduced by these corrections.

Controlled capability evidence and exact ending attribution remain distinct: the Dog/Harrier
old-source scenario has actual reload but only one two-shot Burst/no resumed focus;new source
has resumed focus while both belong to the wave,six shots and real base destruction. Exact
Liberty Iraq/America Brutal now ends with P0 at107940/288000;this is not a claim that P1's
planes alone caused its ending. Exact Demo Germany/Great Britain now ends P1 at45057/72000
while the real paid-repair positive/negative cases establish the separate repair correction.

All earlier performance-equivalence proofs and R88–R123 scope remain. Fresh ten-player
means and all21 normal winners are recorded below;all13 gate timings remain required. No clone/worktree
or new substantial external directory is created;configured Cargo/UV/PIP caches remain.
Parser scratch was measured58 files/584038 file bytes/733184 allocated bytes(716KiB),retained
recreatable for reruns and unnecessary for release;the final checkpoint remeasurement is below. RA5 render
and RA7 parser evidence remain. RA9 inherits R83 transport/attached-Squid firing,R96 actorless
occupied-Prop ability boundaries and R99's explicitly unproven capability lead. R97's actual
queued-AI lifecycle mechanism is CLOSED in R103. Independent review and user visual/audio/
play-feel verification remain required;no independent review claim is made here.


Final source is `b6eac0c7c31e69ff8dff91dc154a9497143b7484`. All16 final preflight commands exit0;with the earlier
same-source Demo harness this totals1201 passed/0failed/0ignored across81 test-target
invocations. Overlapping focused proof repeats are not added. All339 manifest5 files rehash
without mismatch. No validator is active before committing the evidence checkpoint.

Actual default ten-player all-map run exits0:21/21 explicit winners,all ten builders each,
all means<5ms. Endings median85128/p90125479/max174450 remain below unchanged400000. All three
summary values rise versus checkpoint6's65538/90705/99602;both sets remain without causal
attribution or artificial timing. Command1408.635261s,measured1403.896s/mapelapsed1404.023s.
Full-parse worst mean is Arctic Floes1.051ms;the earlier compact Red Canyon1.035ms worst-mean
observation was preliminary and is explicitly corrected. Raw attempt6-m13-all8.log SHA
109fb9c6fd57d7626d5e5d0b0262833cc06253aa056a6320383974dde0ac7cc3.

| Fixed24000-tick256×256 map | Mean ms | p99 ms | Maximum ms | Busiest1000 mean ms | >50ms ticks |
| --- | ---: | ---: | ---: | ---: | ---: |
| Liberty Bay | 0.953828 | 6.675 | 93.376 | 1.704 | 1 |
| Continental Divide | 1.006000 | 5.588 | 87.715 | 2.170 | 1 |
| Pacific Atoll | 0.757202 | 5.956 | 59.132 | 1.213 | 2 |

Fixed benchmark exits0/65.758103s,all ten builders on every map. Raw attempt6-m13-budget8.log
SHA312bb9b3a5923e75804715fbb0380b410716519c640484047fb508abb6238239. The unchanged budget
is whole-run mean5ms. Frostline's actual longest725.867ms/busiest1000mean8.033ms and all other
long ticks remain;no maximum-tick or smooth-play guarantee is claimed. These normal runs use
actual default options,seed13,ten ordinary Medium controllers,no Super Me/unlock/injected HP.
They have no competing validation simulation or profiler. Full current/historical fields are
in attempt6-final-m13-metrics.json. No additional performance optimization is required by the
measured mean budget;the prior separately measured value-equivalence optimizations remain.

Storage is freshly remeasured after final runs:58 parser scratch files/584038 file bytes/
733184 allocated bytes(716KiB),retained recreatable in configured Cargo target and unnecessary
for release. No other new substantial external directory. R83/R96/R99 remain RA9 handoff;
R97 is closed R103. R126–R130 policies/assumptions and exact R124/R125/R128 migrations above
enter RA9/final-review scope. Full13,both756-winner before/after R29 metrics,independent review
and user visual/audio/play-feel verification remain pending.


The runtime commit's standalone staged unfiltered check exits0. The separate evidence check
exits2,not passed:80 exact intentional raw-log/captured-snapshot EOF or trailing-space
diagnostics,zero source/data/test/document errors. checkpoint7-staged-hygiene.json retains
full stdout/stderr,every exact path/line/raw-fileSHA/lineSHA and stdoutSHA
be046cb06220eec38ad474775eee4e91d2d8a401551e8b2b38297137f071d1b5. Raw bytes are unchanged;no suppression.
Require an identical final staged stdout/stderr/exit recheck before a separate evidence commit.
