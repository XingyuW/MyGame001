# Round 2 TESTS — bounded C3 helper guard addendum

This is an independent, read-only source inspection of three newly prepared helpers. It is not a final TESTS slice, a native execution receipt, a binding lead decision, or approval. No helper was imported or executed. Python AST parsing of all three files succeeded without creating bytecode. No canonical source, helper, Git state, native process, or existing report was changed. No agents were spawned. The cumulative 120 active-minute ceiling remains applicable; idle time is excluded and no minimum delay is inferred.

## Exact inspected bytes

| Prepared helper | Bytes | SHA-256 |
| --- | ---: | --- |
| run-frozen-candidate-c3-v2.py | 9278 | 4c08097f947c743c3716aef7940d074f9c2556ec330be5747c0b6981e37bc4ab |
| summarize-remediation2-full13-c3.py | 11778 | a39aac9697494163851151e675cddc64bf1417a1f52eae64d7368c4302f3a0f4 |
| continue-frozen-native-gates-c3-v2.py | 4646 | 4b1e2943c60da271eb8efb37f71a9db175beef3b6b3cffbb66e5ce7be676c384 |

All paths are in this report's approved cache directory. I read each entire file, inspected a byte-derived unified diff against its previous version, and reread the complete unchanged isolated summarizer and asset executor to verify the field contracts. The latter remain SHA-256 `03acd24d66ffafa872b307f00bcb226e10ad2d3481a32ee3a3264a16257e212b` and `24dd17e5494c9c61629fb22aec24c6d5bc81a13d12ccb93f41734e7130363e7e` respectively. The original preinspection report remains 11406 bytes, SHA-256 `2a83d70ea2bb29ae316baa466618c0bfa962c680d2720eaa5cd25b153ac03def`; its findings and historical helper hashes are preserved.

## Owned validator group guard

`run-frozen-candidate-c3-v2.py:83–97` adds a group audit immediately after the actual direct validator-shell `wait()` result. It snapshots `ps -axo pid=,pgid=,stat=,args=` up to 16 times, selecting only rows whose PGID equals the validator's recorded PID/PGID. Each observation records UTC and actual remaining PID/PGID/state/argv rows. It stops when the group is empty; otherwise it waits one second between checks, persists `empty: false`, and raises a guard failure. The checks are observations, not fabricated descendant exit codes. The script records the direct child's real exit separately.

This is a meaningful fail-closed improvement for the identified direct-shell-exit case. The normal acceptance calculation occurs only after an observed empty owned PGID. If descendants persist, the wrapper fails and the sequential pipeline stops before assets or later work. Its exception branch records `GUARD_OR_WRAPPER_FAILURE`; it does not manufacture a successful command or clear the native lease. It does not kill other PGIDs or manipulate the unrelated project. The previously inspected exact production non-Won witness remains an unconditional stop of that failed attempt, retaining the raw prefix, witness, direct-shell exit distinction and original command scopes.

Limits: this establishes one observed empty PGID at a point in time; it is not a general operating-system descendant tracer. It cannot identify a child that deliberately changes its own process group/session. No such escape is established for the invoked validator commands. Earlier exceptional wrapper exits need not reach this drain block; they remain failures and require the coordinator to establish actual lease release rather than treating wrapper exit alone as proof. Persistent group members cause failure and require bounded owned-process handling by the authorized coordinator; this new guard itself does not claim those members exited. No live process snapshot or termination was performed by this reviewer.

## Cross-artifact identity guard

`summarize-remediation2-full13-c3.py:49–62` retains all previous whole-gate requirements and adds:

- Exact equality between the isolated acceptance's manifest descriptor and the current full13 manifest's actual relative path, bytes and SHA-256.
- Actual rereading and hashing of the isolated process receipt against the descriptor retained by that isolated acceptance; then equality of its frozen-manifest SHA/path with the full13 process manifest, source-unchanged assertions, exactly five commands and actual zero exits/source-unchanged records for all five.
- Equality of asset acceptance TIP with the full13 launch commit; exactly four passing asset commands and unchanged canonical image/restored owned overlay, as before.
- Actual release executable and packaged executable SHA-256 matching for each asset row, with expected executable paths for the first two release and last two packaged invocations, plus actual asset raw-log size/SHA-256 equality.
- Full13 owned-group `empty` and its PGID equality with the actual recorded child PGID.

The unchanged asset executor's ordering matches these checks: release missing/restored, then packaged missing/restored. It records executable hashes before launching each process, actual exit/timeout, actual diagnostic/600-frame evidence, and raw-log hashes. The isolated summarizer records the actual process descriptor and manifest descriptor; its actual five-command records and raw parsing remain separately inspectable. The new checks bind the claimed evidence to the source manifest and executables instead of accepting only a successful status from an arbitrary similarly shaped artifact.

No actual identity mismatch or false acceptance is demonstrated. For the intended sequential helper-produced artifacts, the two pre-use weaknesses from the first inspection are addressed. These are data-integrity checks within the trusted execution/receipt chain, not a signature or defense against maliciously rewritten receipts. Final independent ingestion still needs to reread actual raw evidence and source bindings. A parser's successful exit cannot replace review of those records or native coverage.

## Pipeline selection and unchanged scopes

The sole change to `continue-frozen-native-gates-c3-v2.py` is selecting `summarize-remediation2-full13-c3.py` at its full-summary step. Its full13 runner is still selected from `plan['full13_runner']`; therefore the final immutable plan must explicitly select `run-frozen-candidate-c3-v2.py` and bind its exact hash. Merely having the new runner file present does not select it. That selection is a final plan obligation, not an observed defect in a supplied final plan: no such C3 plan/freeze has been accepted here.

Compilation remains two Cargo compiler jobs only. Original match/test workers, commands, scenarios and caps remain unchanged in the inspected diff. The five isolated executions still precede the single complete shared full13 attempt; actual command failure stops the pipeline. All previous parser coverage limits remain: whole unfiltered audit and AI integration execution, production and duel match identity/counts, fixed-duration soak scope, benchmark busy/max/mean distinction, package/assets and real source guards must be independently confirmed from actual final data.

The summarizer still requires exactly 185 audit and 139 AI integration passes in its current text at lines 19–20 and its emitted metadata. These are provisional C2 bindings. The coordinator explicitly states final new regression placement/count is not chosen yet. Final expected whole-target counts must be rebound to the actual frozen candidate if legitimate new tests change them; preserve every original test identity/assertion. Do not reject valid extra tests merely to preserve an old count, skip new tests to fit it, or synthesize an old whole-target pass from 137 passes plus two filtered retries. A whole unfiltered successful execution is still required.

## Findings and pending decision

No new confirmed unique P0/P1 product or test-integrity finding arose from this bounded three-file reinspection. The new guards address the intended earlier pre-use concerns at their stated scopes. This is not a blanket endorsement of prepared automation and does not close the mandatory native-gate P1, approve the implementation, or waive a criterion. The guard behavior was established by source inspection and AST parsing only; it was not exercised against synthetic or native failures.

The parent-dispatched R201 state remains a preserved failed/interrupted attempt: a real M6 Liberty Bay FranceMedium/GermanyMedium seed 0 non-Won at 288000/288000, with direct-shell -15/wrapper 241 reported by the coordinator. That report is not relabeled success here. No whole phase 04, complete all13, package/assets or revised-source acceptance is inferred. Old successful isolated and focused receipts remain scoped to their actual earlier frozen source. No final corrected C3 source freeze or actual C3 execution was provided or ingested in this addendum.

Pending independent TESTS work after exact dispatch remains the entire cumulative base `8230a955b251e9341d349f99eaeb86f7f8253664` through the final corrected candidate: authenticate unchanged reviewed portions, inspect all changed production/tests/interfaces and actual failed-before/passed-after obligations, reconcile original test identities/assertion/fixture premises and every owned debt row, then ingest final actual isolated/full13/package/assets data and raw execution accounting. All 183 debt IDs and all 51 user-only quality rows remain in scope; user quality checks are not converted to automated passes. Every required reviewer and the fresh lead still must decide from the actual final candidate/evidence.

Only this separate report file was created by this addendum. No substantial directory was created outside the canonical repository; the pre-existing approved review cache remains necessary to preserve review history and future independent evidence ingestion.
