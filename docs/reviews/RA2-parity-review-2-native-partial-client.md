# Independent CLIENT partial native evidence addendum

This addendum preserves both `round-2-client.md` and `round-2-revised-client.md` unchanged. Cumulative source base remains `8230a955b251e9341d349f99eaeb86f7f8253664`; reviewed implementation TIP remains `5c93acb68d3f9f2a9c0f3926a16fb6ab9843b2e6`. The launch TIP is `6bba9d2cf1bb0e987d326238af9cfade96f5f0d8`.

**The isolated five commands are now accepted as actual current-source evidence within their exact scopes. Full native acceptance remains OPEN. No overall GO or human quality approval is supplied.** My revised source conclusion remains 0 new P0, 0 unresolved source P1 in CLIENT, and 2 retained P2. The fresh lead must reconcile all five slices and complete mandatory gates.

## Independent binding and provenance

I inspected the Git delta from implementation TIP to launch TIP: exactly16 added files, consisting of the five revised source reports and eleven isolated evidence files. There are no source modifications, deletions or other paths in that delta. All five canonical source reports exactly match the external originals and their recorded SHA256 values; my revised CLIENT report SHA256 remains `6cebea9087971fd7553e0782815ee68ef4644f4bfd17466375f78a7013b09a5c`.

I independently rehashed all689 frozen files and all64 protected document spans at launch TIP, with zero mismatches. Manifest SHA256 remains `0e2edf2bce2a8e41581c382a923dce7a5057ccf0231256903d314acb21261b99`; packet binding remains unchanged. Both isolated source snapshots are byte-identical and their689 path/hash entries exactly match that manifest. Thus this evidence applies to the same full cumulative source reviewed in my initial and revised reports, including all40 game source paths and the revised selection/MapTerrain/AI interfaces. It does not create new source coverage or supersede the preserved scope/limits.

I read the entire isolated process receipt, acceptance summary and all five raw logs, independently checking recorded byte counts and SHA256 values: all five raw receipts match. The acceptance-summary process SHA256 also matches the actual process receipt (`2c88d3e7984f99ef5d58be009a6a921b3c067fc2bc060663432c1f61a5f05cfd`). Commands ran sequentially under root's native lease, from 2026-10-22T21:11:14Z to22:18:00Z on implementation TIP. The logs identify the configured cache release executables, not a new validation copy. I did not run those commands myself, rerun probes, alter processes or infer acceptance merely from the summary status label.

## Completed isolated scopes

* `01-all21-to-won.log`, actual exit0: raw output contains exactly21 named map records,21 explicit Won endings and21 records saying10 players built. Map order, winner IDs and both printed ending ticks agree with the summary. All end before the unchanged400000 cap: minimum35520, median90119, maximum147715 (summary nearest-rank p90=137458). This is actual complete21-map completion evidence, distinct from a fixed-duration survival window.
* `02-fixed-three-windows.log`, actual exit0: liberty_bay, continental_divide and pacific_atoll are each256×256 and execute24000 schedules, all10 players build. They end with9,6,5 players left respectively. These are successful fixed windows with Running outcomes, not three additional wins.
* `03-m13_ten_players-guards.log`, actual exit0: `ra8_ten_player_completion_requires_an_explicit_win` passes1/0/0ignored/0filtered.
* `04-m14_duels-guards.log`, actual exit0: `ra8_duel_completion_requires_a_winner` passes1/0/0ignored/0filtered.
* `05-m7_soak-guards.log`, actual exit0: `ra8_completed_soak_matches_require_winners_while_duration_stays_fixed` passes1/0/0ignored/0filtered. These are three refusal-guard tests, not execution of the full duel or soak programme.

The raw all21 timing observations have maximum map mean3.359ms (ember_caldera), maximum printed p99=47.332ms (ember_caldera), maximum tick722.734ms (harvest_plains), and maximum busiest1000-tick mean11.530ms (arctic_floes). For example coral_archipelago reports822 ticks over50ms. The fixed windows report means2.889/2.595/2.212ms, p99=18.634/15.431/31.851ms, maxima116.589/257.598/159.378ms and31/40/55 ticks over50ms. The recorded mean criterion is satisfied in this isolated run, but neither a mean nor a headless simulation measures rendered-frame smoothness. Tail observations remain explicit; no tail/frame threshold is silently relaxed or invented. These runs do not replace the original separate benchmark's mean/max/heap gate,600-frame smokes, client visual verification or human feel assessment. External host workload attribution is not proven by these logs.

## Full13 partial snapshot, not completion

At my read, `docs/validation/ra2-parity/review-2/gate-process.json` is RUNNING, launched at22:18:19Z on6bba9d2c. Its only completed command records are:

1. `cargo fmt --all -- --check`: exit0.
2. `git diff --check`: exit0.
3. `cargo clippy --workspace --all-targets -- -D warnings`: exit0.

I read the three raw outputs and matching summary lines. The first two outputs are empty (SHA256 `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` each); clippy is150bytes, SHA256 `2a4dbddc2c0831eab28fc866ae647330a251af122acc6c046eda70cde3064d04`, with the actual finished dev-profile message. The runner records zero frozen-file/span mismatches and clean tracked input after each. Unlike the completed isolated receipts, this live gate record is an explicitly partial snapshot, not an immutable final process receipt. The fourth raw log only shows game/sim compilation at inspection; no workspace/test result is credited.

Consequently no current full release workspace/game test pass, whole185 audit pass, whole139 AI suite pass, production756/duels756 programme, final benchmark/soak, both600-frame smokes, build/package/regeneration completion or executable/package missing-asset failure/restoration acceptance is established by this addendum. The earlier debug integration-tail interruption remains an interruption. All remaining mandatory native/process gates must actually complete and be independently ingested on the identical frozen inputs. No aggregation of selective retries into a complete suite is authorized.

## Debt, quality and preservation

All183 stable debt IDs and their prior CLIENT/quality/packaging classifications remain retained from the revised report. This evidence updates only the isolated native and first-three-check factual status. RENDERPERF is not granted rendered quality approval; SMOKEFAIL/CHECKEDASSET/PACKAGE still require actual process evidence. All51 HOWTOSEE user-only visual/audio/readability/feel/delivery rows remain PENDING—requires user verification. No user quality pass follows from headless timing, successful compilation or assertions.

I ran read-only Git, file/hash and text inspection only: no Cargo/game/benchmark/soak/probe/GUI, no canonical edit, no process control and no new agent. Canonical HEAD remained launch TIP, with only root-owned `docs/validation/ra2-parity/review-2/` untracked output. I created no substantial directory, checkout or /tmp artifact; only this report was written in the existing approved review cache, which remains necessary. Earlier reports are preserved. This partial addendum is released now; actual final evidence and fresh lead decision remain outstanding.
