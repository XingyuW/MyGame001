# RA8 acceptance record

Status: first frozen attempt failed and was stopped. R102 records the complete-plan test
correction; the broader game preflight then exposed real default-path admission/lifecycle
defects, fixed in R103. Corrected final-runtime match/budget repeats pass; the complete
thirteen-command rerun remains required. This is not an independent review.
All earlier validation directories and failed RA8 probes are retained.

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
(708 KiB after the policy/parser preflight), retained for reruns and unnecessary for release.
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
the next Commands rejects its stale AttackMove). Public validation remains strict.
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
