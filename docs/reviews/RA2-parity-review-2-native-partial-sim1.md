# Independent panel2 native evidence addendum — SIM1, isolated five

This addendum supplements and preserves `round-2-sim1.md` and `round-2-revised-sim1.md`. The corrected runtime candidate remains `5c93acb68d3f9f2a9c0f3926a16fb6ab9843b2e6`; current clean launch HEAD is `6bba9d2cf1bb0e987d326238af9cfade96f5f0d8`. Independent Git comparison shows the latter adds only the five revised source reports and isolated receipts/summary/source snapshots; no runtime/test/data/driver delta. The cumulative source assessment and retained lost-Hornet exact-boundary P2 remain unchanged.

**Isolated evidence verified: five actual zero exits, all21 maps Won/all10 players built, original caps retained. Overall native acceptance remains OPEN.** This is not a full13,185-audit,139-AI,production756,duels756,soak,benchmark,600-frame smoke,package or missing-assets/restoration pass. No overall programme approval is supplied.

## Independent source and receipt checks

Read canonical `C-isolated-five-01/process.json`, `acceptance-summary.json`, all five raw logs, both complete source snapshots, exact external plan and matching runner, and actual m13/duel/soak driver/guard source. I ran no Cargo, game, match, benchmark, soak or native probe and changed no process/source/Git state. Root's exclusive native lease remains intact.

Independently recomputed the recorded process SHA256 `2c88d3e7984f99ef5d58be009a6a921b3c067fc2bc060663432c1f61a5f05cfd` and148657-byte frozen manifest SHA256 `0e2edf2bce2a8e41581c382a923dce7a5057ccf0231256903d314acb21261b99`. Every one of689 current frozen file lengths/hashes matches; all64 authority span hashes match. Both689-entry before/after source maps are identical and match current files. Each five raw byte count/SHA equals its process receipt and summary. Plan SHA `b75386d7e5227eb80f20a996b6cd243f406610befd9be3915a306ae5dc9f92d7` matches actual plan. Matching runner is `run-frozen-bounded-v4.py`, SHA `dfecd0b723700a7726d6ac7acef982a8be0e28cdb0175354b95b405eb0a80d3a`.

The runner launches the exact recorded argument vectors with direct-child wait/exit and raw merged stdout/stderr, not inferred completion. It checks manifest/HEAD/clean tracked source, complete declared source scope and protected authority before and after each command; no snapshot exclusions are installed. It preserves configured caches, sets build jobs1, clears optional RA2 duel seed/pair overrides and serially runs these commands. Build parallelism is distinct from native scenario/test-worker coverage. The complete plan has no scenario/cap/worker reduction. Start/end receipts put all21 first, fixed3 next, then three guard executables, with all actual child exits0. The controller's completed label alone was not treated as semantic pass.

## All21 actual matches and driver contract

Actual command: `cargo run --release -p sim --example m13_ten_players -- to-the-end all`. The raw launcher line names that exact executable/arguments. Independent parser extracts21 completed map records; every summarized field (dimensions, ticks, mean,p99,max,busiest window,over50 count,total/elapsed,peak units/buildings,builder count,winner/tick) equals raw output. Extracted map IDs exactly equal the21 `maps` entries in shipped skirmish.ron, in order. Every line has10 players and10 builders, genuine Won, and schedules=zero-based outcome tick+1.

Driver source `m13_ten_players.rs:255–341,348–489` loads shipped rules/skirmish/AI and actual map, enlarges for MAX_MATCH_PLAYERS10, installs seed13 and nine countries in turn plus first again at the middle difficulty, uses actual setup_match, then runs native SimSchedule until real MatchState outcome or original END_LIMIT400000. `all` selects every actual skirmish map, with no map sampling/filter override. It accumulates actual BuildingPlaced player IDs and fails if fewer than10. to-the-end also fails unless the outcome is Won; Draw/None are rejected by the preserved guard. Durations measure SimSchedule with a separate actual elapsed figure; this is simulation time/performance, not rendered frame or user play evidence.

Independently recomputed outcome distribution: min35520,median90119,nearest-rank p90137458,max147715, all below unchanged400000. Maximum cap fraction0.3692875. All21 reported overall means are below5ms; maximum3.359ms on Ember Caldera. Driver gate is `mean > MEAN_BUDGET(5ms)` failure, plus build/winner checks, not an assertion on every long tick or busiest window.

| Map | Won tick | Overall mean ms | p99 ms | Longest ms | Busiest1000 mean ms |
|---|---:|---:|---:|---:|---:|
| demo | 49774 | 0.601036 | 4.340 | 79.562 | 2.251 |
| crossroads | 35520 | 0.864666 | 5.032 | 41.798 | 2.427 |
| riverlands | 93381 | 0.584114 | 4.272 | 31.635 | 2.528 |
| liberty_bay | 68548 | 2.455000 | 22.895 | 282.081 | 5.075 |
| frostline | 91338 | 2.259000 | 19.562 | 248.482 | 7.413 |
| oasis_gulf | 110391 | 1.277000 | 10.027 | 136.497 | 3.700 |
| coral_archipelago | 114969 | 2.407000 | 44.658 | 134.798 | 5.906 |
| continental_divide | 147715 | 0.949736 | 7.991 | 134.278 | 3.660 |
| steel_harbor | 67189 | 1.434000 | 14.348 | 125.106 | 3.206 |
| glacier_pass | 77956 | 1.761000 | 14.304 | 676.182 | 6.552 |
| red_canyon | 146991 | 2.213000 | 22.806 | 230.739 | 8.088 |
| jungle_delta | 83201 | 2.262000 | 18.224 | 465.481 | 6.343 |
| pacific_atoll | 91158 | 2.510000 | 35.065 | 164.870 | 5.811 |
| iron_isthmus | 90119 | 2.234000 | 32.215 | 261.189 | 6.914 |
| arctic_floes | 116869 | 2.436000 | 39.025 | 148.824 | 11.530 |
| ember_caldera | 71877 | 3.359000 | 47.332 | 406.262 | 10.591 |
| frozen_sound | 56148 | 2.461000 | 31.054 | 397.843 | 6.886 |
| harvest_plains | 49035 | 2.210000 | 31.085 | 722.734 | 8.778 |
| black_water | 70077 | 2.995000 | 32.784 | 320.261 | 9.693 |
| thousand_lakes | 137458 | 2.265000 | 13.444 | 276.513 | 6.506 |
| wheel_of_worlds | 96249 | 2.661000 | 25.142 | 378.292 | 8.326 |

These are actual isolated-run samples, not universal performance bounds. The longest observed tick is722.734ms (Harvest Plains), largest p9947.332ms (Ember Caldera), largest busiest1000 mean11.530ms (Arctic Floes), and5372 measured ticks across the21 runs exceed50ms. Those tails remain explicit. They are not silently passed as50ms-max/busy5ms or rendered smoothness; the mandatory original benchmark has its own mean/busy5ms/max50ms contract. No new P0/P1 is inferred merely by applying the benchmark's separate contract to m13. Existing scale/path/all-map worst-case and user-feel limitations remain.

## Fixed three windows and guard tests

Actual command: `cargo run --release -p sim --example m13_ten_players -- liberty_bay continental_divide pacific_atoll`. Without `to-the-end`, the same driver selects original MEASURED_TICKS24000; each raw record has256x256,10 players,10 builders and exactly24000 schedules. It reports9/6/5 players left, with no ending outcome. These are legitimate Running timing windows, not failed-to-Won matches or fabricated completed wins.

| Map | Schedules | Players left | Overall mean ms | p99 ms | Longest ms | Busiest1000 mean ms |
|---|---:|---:|---:|---:|---:|---:|
| liberty_bay | 24000 | 9 | 2.889 | 18.634 | 116.589 | 4.742 |
| continental_divide | 24000 | 6 | 2.595 | 15.431 | 257.598 | 6.065 |
| pacific_atoll | 24000 | 5 | 2.212 | 31.851 | 159.378 | 5.129 |

All three overall means meet their m13 mean gate. Their31/40/55 over50ms counts and busy/max tails remain measurements, not smoothness acceptance.

Each guard command is an actual `cargo test --release -p sim --example <name> -- --nocapture --test-threads=1`, with one passed,zero failed/ignored/filtered. Actual named bodies:

- m13: `ra8_ten_player_completion_requires_an_explicit_win` refuses None and Draw, accepts Won and asserts400000.
- m14: `ra8_duel_completion_requires_a_winner` refuses Draw, accepts Won and asserts300000.
- m7_soak: `ra8_completed_soak_matches_require_winners_while_duration_stays_fixed` refuses completed Draw, accepts Won and asserts36000.

These prove their specific completion/cap predicates execute. Three1/0 unit targets are not the actual756 duels,260x2x36000 soak windows or heap runs. Normal fixed soak windows can remain Running; only completed outcomes must be Won.

## Full13 and remaining obligations

Read current `review-2/gate-process.json`: actual start2026-10-22T22:18:19.362611UTC at launch HEAD6bba9d2, statusRUNNING. It records zero exits for commands1 fmt,2 diff-check,3 strict workspace/all-target Clippy. Command4 is compiling; there is no completed workspace test verdict in this snapshot. No full13/185/139/production756 result is credited from compilation, progress or historical scoped controls. Subsequent actual complete raw results, exact source checks, remaining mandatory gates and missing-assets/restored launches must be ingested independently when available. Fresh lead follows all actual gates and five reviewer addenda; this reviewer does not approve the programme.

The prior findings/counts,80 owned/shared debt assessment,all183 IDs/aliases and51 user-only pending quality rows remain preserved. This addendum closes only the isolated receipts' pending status in the source report; it does not alter substantive debt or gate requirements.

No substantial directory was created outside the repository. Only this addendum was written in the pre-existing approved review cache, where it remains necessary with the prior reports. No canonical files, build/cache paths, repositories/worktrees or unrelated processes were changed/deleted.
