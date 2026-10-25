"""Apply current closing prose only after actual required RA9 validation passes."""
from pathlib import Path
import datetime,hashlib,json,posixpath,re,collections
ROOT=Path('/Users/xingyuwang/GameProjects/MyGame001');OUT=ROOT/'docs/validation/ra2-parity/ra9';sha=lambda b:hashlib.sha256(b).hexdigest()
m=json.loads((OUT/'200-final-canonical-validation-summary.json').read_text());assert m['status']=='ACTUAL_CANONICAL_RA9_REQUIRED13_AND_ISOLATED5_PASS_NOT_REVIEW_OR_USER_APPROVAL'
handoff_date=datetime.datetime.now(datetime.timezone.utc).date().isoformat()
i=json.loads((OUT/'198-isolated-acceptance-summary.json').read_text());packet=json.loads((OUT/'195-actual-canonical-input-scope.json').read_text());w=m['workspace'];a=i['all21']['maps'];maxmean=max(x['mean_ms'] for x in a)
def link(f,target):return posixpath.relpath(target,str(Path(f).parent))
files=['README.md','docs/HANDOFF.md','docs/CONSTRUCTION_SYSTEM.md','docs/RA2_GAP_ANALYSIS.md','docs/STATUS.md','docs/RA9_INTEGRATION_CHECKLIST.md','docs/reviews/RA2-parity-review-requirements.md','docs/reviews/RA2-parity-reviews-outstanding.md','docs/prompts/ra2-parity-review/README.md']+['docs/prompts/ra2-parity-review/'+x+'.md' for x in ['slice-sim1','slice-sim2','slice-data','slice-tests','slice-client','lead']]
original={f:(ROOT/f).read_bytes() for f in files+['docs/RA2_HISTORICAL_DEBT.tsv','docs/RA9_LOADED_CATALOGUE.md']};protected={x['path']:x['sha256'] for x in packet['source_files']};changes={}
gap=original['docs/RA2_GAP_ANALYSIS.md'];start=gap.index(b'## How to perform user verification');end=gap.index(b'## Historical baseline and source provenance');how=gap[start:end];assert sha(how)=='8ae4edfadf97fc68138d03c9a12d9c1217cf0c1fb65cf6dd6010c77b1c5bd6c1'
def header(f):
 report=link(f,'docs/validation/ra2-parity/ra9/200-final-canonical-validation-summary.json');scope=link(f,'docs/validation/ra2-parity/ra9/195-actual-canonical-input-scope.json');review=link(f,'docs/reviews/RA2-parity-review-1.md')
 return f'''<!-- RA9_CURRENT_EVIDENCE_BEGIN -->
**Canonical implementation handoff — {handoff_date} UTC.** Required implementation validation is complete
for gameplay/audit source freeze `f58e8d9e1e6a8b8cf4f1ce31a80c229cfd6203c1` and unchanged inputs.
The actual full-gate launch commit is `{m['gate_launch_head']}`. [Packet195]({scope}) binds 656 source
files, 97 core Rust files and 64 exact authority spans;13 excluded factual binding outputs are
separately guarded. Root's195/196/196b checks authenticate facts and hashes, not source-policy,
original-engine, severity or review approval.

[Actual canonical results]({report}): all13 commands exit0; unfiltered release workspace
**{w['passed']} passed /0 failed /0 ignored /0 filtered across {w['result_targets']} result targets**,
including the entire181-case RA9 audit. The separate whole audit reports16,211 physical entries,
7,108 exact demanded/successful pairs and 5,022 source atoms (3,111 Compared+1,911 dispositions).
The required isolated five also exit0: all21 ten-player maps reach Won within the unchanged cap;
maximum measured mean {maxmean:.6f}ms (<5ms); fixed3 timing cases and three example guards pass.
Mean, p99, max and busy-window tails remain distinct. Production and duel collections each
record 756 Won. The soak retains 260 deterministic twin configurations × 36,000 ticks, with
{m['soak']['ended_matches_per_repeat']} completed matches and{m['soak']['final_worlds_still_running_per_repeat']} final Running worlds per repeat;
fixed windows are not all-to-Won evidence. R56 item6 and every original bound remain unchanged.

The earlier ordinary-copy work and failed/repaired/interrupted runs are historical, preserved
with exact source scopes under the portable evidence resolver.159's175/6 full target and168's
six-only pass are not relabeled; the complete canonical181 pass is the later actual full13.
All 183 retained implementation claims/debt IDs remain individually recorded. Their severity,
adequacy and acceptance belong to the fresh panel; no presumed nonblocking label waives P0/P1.
All 51 unchanged HOWTOSEE rows still require actual user visual/audio/feel and human/platform checks.

**Review status at this implementation handoff:** the fresh independent review of this freeze
has not begun. R172 requires five fresh slices (3+2) and a fresh lead; implementers/preparers are
ineligible. The dispatcher supplies the exact final clean documentation/evidence TIP and round;
its subsequent dated [round report]({review}) records the independent outcome. Current source,
counts, module ownership and actual evidence are supplied here; no TIP/ROUND placeholder is proof.

For any later reviewer command preserve configured
`CARGO_TARGET_DIR=/Users/xingyuwang/Library/Caches/Codex/cargo-target` and UV/PIP caches.
A single coordinated owner runs Cargo/native probes; other reviewers remain read-only unless
explicitly leased. These current rules override historical review-target/cache command examples.
No clone/worktree or substantial /tmp artifact is authorized by those older examples.
<!-- RA9_CURRENT_EVIDENCE_END -->'''
# Current operational sections only. Historical tails and the entire HOWTOSEE section stay exact.
markers={'docs/HANDOFF.md':'## Historical handoff —','docs/CONSTRUCTION_SYSTEM.md':'<details>','docs/RA2_GAP_ANALYSIS.md':'## How to perform user verification','docs/reviews/RA2-parity-review-requirements.md':'## Historical requirements —','docs/reviews/RA2-parity-reviews-outstanding.md':'## Historical report —','docs/prompts/ra2-parity-review/README.md':'## Historical panel procedure —'}
for f in files:
 s=original[f].decode();s,n=re.subn(r'<!-- RA9_CURRENT_EVIDENCE_BEGIN -->.*?<!-- RA9_CURRENT_EVIDENCE_END -->',lambda _:header(f),s,flags=re.S);assert n==1,f
 if f=='docs/STATUS.md':changes[f]=s;continue
 mark='## Preserved earlier substantive brief and module supplement' if f.startswith('docs/prompts/ra2-parity-review/') and f.endswith(('slice-sim1.md','slice-sim2.md','slice-data.md','slice-tests.md','slice-client.md','lead.md')) else markers.get(f)
 at=s.index(mark) if mark else len(s);body,tail=s[:at],s[at:]
 # Date preliminary body claims and replace completed operational obligations explicitly.
 body=body.replace('## Implementation status in this validation copy','## Canonical implementation and actual validation')
 body=body.replace('## Preliminary RA9 copy status — 2026-10-21','## Canonical RA9 handoff — 2026-10-21')
 body=body.replace('current RA1–RA8 model and preliminary RA9 client integration','current RA1–RA9 model and canonical client integration')
 body=body.replace('This supplement describes actual preliminary-copy source ownership. It is not dispatch,\nvalidation, review or acceptance. No review has begun; final canonical TIP, reachable-module/\nloader catalogue, exact source/input/route bindings and actual final gates must be supplied\nafter integration.', 'This supplement assigns the actual frozen canonical source and evidence to the forthcoming\nindependent panel. Implementation validation and source/loader/input bindings are now supplied\nin packet195 and final summary200; the header records the actual source and gate-launch commits.\nAt this implementation handoff no fresh panel review of this freeze has begun. The dispatcher\nwill provide its exact clean final documentation/evidence TIP and round in the dispatch message.')
 body=body.replace('Final canonical integration, full final gates and independent review remain **pending**.','Canonical integration and implementation gates are complete; fresh independent review remains\npending at this handoff.')
 body=body.replace('Canonical integration, full final gates and independent review remain **pending**.','Canonical integration and implementation gates are complete; fresh independent review remains\npending at this handoff.')
 body=body.replace('Canonical integration and final gates remain pending.','Canonical integration and required implementation gates now pass; independent review remains\npending at this handoff.')
 body=body.replace('complete mandatory gates; no combined final-source copy pass or independent acceptance is claimed.','complete mandatory gates. Those gates now pass on the canonical freeze as recorded above;\nno combined final-source copy pass or independent acceptance is claimed.')
 body=body.replace('Subsequent RA8 imports and the182 combat correction require fresh canonical source binding and the complete mandatory gates;', 'The subsequent RA8 imports and182 combat correction received actual canonical packet195 binding and required validation;')
 body=body.replace('later imports/combat changes await actual canonical rebinding','later imports/combat changes are bound by actual canonical packet195 and196/196b factual outputs')
 body=body.replace('new imports/combat correction still await canonical rebinding','new imports/combat correction are now factually bound by canonical195/196/196b')
 body=body.replace('later source imports/correction await actual canonical\nrebinding','later source imports/correction are factually bound by canonical195/196/196b')
 body=body.replace('Final canonical\nsource/execution binding remains required;', 'Actual canonical195/196/196b source binding is now recorded;')
 body=body.replace('final canonical source/execution binding remains owed','actual canonical195/196/196b source binding and200 gate outcomes are now supplied')
 body=body.replace('final canonical source/input/route binding remains owed at dispatch','actual canonical195/196/196b input binding and200 gate outcomes are supplied for dispatch')
 body=body.replace('final canonical gate/review bindings remain pending','canonical gate bindings are complete; the fresh panel result remains pending at this handoff')
 body=body.replace('Final canonical source/execution binding remains required','Actual canonical195/196/196b binding is supplied')
 body=body.replace('Final canonical binding/gates/review remain owed','Canonical binding and implementation gates now pass; independent review remains pending at this handoff')
 body=body.replace('Canonical integration, packaging/smoke obligations and the independent R172 panel remain pending.','Canonical integration, packaging and smoke gates now pass. The independent R172 panel remains\npending at this handoff; its subsequent report owns review acceptance.')
 body=body.replace('final canonical execution pending','canonical unfiltered workspace execution PASS; review adequacy remains pending')
 body=body.replace('installed native regression still needs final execution','installed native regression passes in the actual unfiltered canonical release workspace; review adequacy remains pending')
 body=body.replace('**Retained gathering P2:**','**Retained gathering limitation (historical P2 claim; fresh panel classification pending):**')
 body=body.replace('This remains\nnonblocking documented debt subject to the fresh final panel, separate from R97/R103 casualty fixes.','Its severity and acceptance remain for the fresh final panel; no assumed nonblocking label\nwaives the original criteria. It is separate from R97/R103 casualty fixes.')
 body=body.replace('complete13-command validation and R172 review remain pending','complete13-command implementation validation now passes; R172 review remains pending at this handoff')
 body=body.replace('all21 ending/performance checks, complete13-command validation and R172 review remain pending.','all21 ending/performance and complete13-command implementation checks now pass; R172 review\nremains pending at this handoff.')
 body=body.replace('Final canonical-source binding/gates stillrequired','Actual canonical195/196/196b binding and200 full13 gates now pass')
 body=body.replace('Final canonical execution required;','Actual unfiltered canonical workspace execution PASS;')
 body=body.replace('Rebind actual finalcanonicalinputs;','Actual195/196/196b finalcanonical binding complete;')
 body=body.replace('Final canonical13 plus integrated all21Won/ten-player<5ms, then fresh five slices3+2/freshlead;','Canonical13/all21Won/ten-player mean<5ms PASS; fresh five slices3+2/freshlead remain pending at handoff;')
 body=body.replace('Current implementation/proof inventory for the ordinary preliminary validation copy, updated\nthrough189 after the focused functional lease.','Current canonical implementation/proof inventory; historical through189 copy outcomes are\npreserved separately from actual195–200 canonical binding and required validation.')
 body=body.replace('canonical source and final acceptance remain separately controlled.','canonical implementation source and gates are now frozen/complete; independent acceptance\nremains separately controlled.')
 body=body.replace('The new real death-stage controls must still be executed and bound;', 'The real death-stage controls now pass in the canonical unfiltered release workspace;')
 body=body.replace('and final-source gates remain. Occupied air docks', 'and final-source gates pass in200. Occupied air docks')
 body=body.replace('fresh canonical binding and\nfinal gates remain pending.', 'actual canonical195/196/196b binding and200 required gates now pass.')
 body=body.replace('final binding is still due.', 'actual canonical195 binds the unchanged loader/data;200 required gates pass.')
 body=body.replace('final binding remains due.', 'actual canonical195 binds the unchanged loader/data;200 required gates pass.')
 body=body.replace('Repairs and populated records still require final canonical frozen-source positive and genuine accepted-baseline negatives.', 'The actual197c canonical whole positive and200 unfiltered workspace execute the frozen-source positive and existing negative controls; independent baseline/semantic adequacy remains for the panel.')
 body=body.replace('Current48-Rust/67-TSV/4-TXT counts are inspection-only.', 'The current48-Rust/67-TSV/4-TXT directory counts are exact inspection facts, separate from Cargo result counts.')
 body=body.replace('remain necessary for final integration/review;', 'remain necessary for historical evidence and independent review;')
 body=body.replace('Copy logs are\npreliminary provenance and must be archived explicitly into the canonical validation history\nbefore final acceptance.', 'Copy logs remain preliminary provenance. Actual193 archived the selected exact proofs/preimages into canonical history;190 resolves original paths and historical hashes. No historical receipt is relabeled as current-source acceptance.')
 # Avoid leaving undated operational placeholders in current review supplements.
 body=body.replace('A must refresh final counts/bindings/evidence destinations/TIP/ROUND and actual complete gates','The dispatcher supplies its exact final clean TIP/ROUND; verify supplied packet195 counts/bindings, portable destinations and actual200 complete gates')
 body=body.replace('A must refresh final count/binding/TIP/ROUND placeholders','Actual195/200 source/count/binding evidence is supplied; the dispatcher supplies its final clean documentation/evidence TIP and round')
 body=body.replace('A refreshes final counts/bindings/gates','Verify the supplied195/200 actual counts/bindings/gates')
 body=body.replace('A must refresh final counts/source/evidence/gate bindings','Verify supplied195/200 actual counts/source/evidence/gate bindings')
 body=body.replace('A still owes actual final canonical declared/reachable/source/portion/class/physical/Own counts, completion record/root Scope hashes and fresh whole-positive/accepted-baseline-negative outcomes','Actual195/197c/200 now supply canonical source/portion/class/physical/Own counts, completion/root hashes and whole-positive/unfiltered negative outcomes; verify them independently')
 body=body.replace('A must attach fresh final canonical complete-positive and accepted-baseline one-cause/restoration receipts and actual final census/hash/gate counts','Actual195/197c/200 complete-positive, unfiltered negative and final census/hash/gate receipts are supplied; verify each original assertion and genuine baseline independently')
 body=body.replace('Before dispatch A supplies final reachable/source/portion/class/physical/Own counts, acceptance/root/input/static/route hashes, portable evidence destinations, actual complete gates/performance and canonical TIP/ROUND','Actual195/197c/198/200 supply source/portion/class/physical/Own counts, factual root/input/static/route hashes, portable evidence destinations and required gates/performance; the dispatcher supplies its exact final clean TIP and round')
 body=body.replace('At final freeze A must refresh canonical TIP/ROUND;', 'At review dispatch verify the actual source freeze and dispatcher-provided final clean TIP/ROUND;')
 body=body.replace('These unfinished fields are obligations, never inferred passes.','Actual195–200 provides those implementation fields; independent review must verify them, never infer semantic acceptance.')
 # These paragraphs explicitly separate dated copy evidence from completed canonical implementation work.
 note=f'''\nCurrent canonical execution is recorded in [summary200]({link(f,'docs/validation/ra2-parity/ra9/200-final-canonical-validation-summary.json')})
and [isolated198]({link(f,'docs/validation/ra2-parity/ra9/198-isolated-acceptance-summary.json')}).
The source/input/route binding, whole positive, exact six repairs, new difficulty control,
all21/fixed3/example guards and complete unfiltered implementation gates are complete for this
freeze. Earlier conditional instructions about later canonical execution describe the dated
preparation history; they do not leave those commands pending. The actual loader catalogue
comes from181's real execution, bound to unchanged definition/loader/data bytes by195; no new
catalogue run or original-fidelity claim is invented. All retained implementation limits and
historical severity claims require independent classification under the unchanged P0/P1 rules.
'''
 body=body.rstrip()+'\n\n'+note+'\n';changes[f]=body+tail
# Rewrite the most visible current-origin paragraphs; retain the original copy material in preimages.
f='README.md';s=changes[f];start=s.index('The RA1–RA8 implementation is the starting point');end=s.index('The current typed source-policy table declares',start);s=s[:start]+f'''The cumulative RA0–RA9 implementation is now integrated on canonical `ra2-parity` after RA8
closure8f1b1dc. Source freezef58e8d9, packet195/root factual verification and all required own
implementation gates are complete. The actual unfiltered release workspace passes{w['passed']}/0/0,
including181/0/0 in the full RA9 audit target; the exact isolated five also pass. Historical
copy failures131/136/159 and repaired six-only168 remain preserved, not relabeled as complete
passes. [STATUS](docs/STATUS.md) and [summary200](docs/validation/ra2-parity/ra9/200-final-canonical-validation-summary.json)
record actual source, command, raw, timing and delivery scopes. Fresh independent review and
all user-only checks remain pending as of this implementation handoff.\n\n'''+s[end:];changes[f]=s
f='docs/HANDOFF.md';s=changes[f];start=s.index('Work is in the ordinary RA9 validation copy at');end=s.index('The current target is vanilla Red Alert 2',start);s=s[:start]+'''Work is now integrated in the canonical `ra2-parity` checkout. The ordinary copy at
`~/Library/Caches/Codex/reviews/MyGame001/ra9-integration-validation` remains preserved as
historical preparation/evidence, initially exported from26ad1e3 with exact R189/4a imports.
R192 and packet195 bind actual integration after RA8 closure8f1b1dc; summary200 and isolated198
record this candidate's own actual validation. The preserved copy, archives and configured
Cargo/package cache remain necessary; no clone/worktree or substantial /tmp tree was created.
\n'''+s[end:];changes[f]=s
f='docs/RA2_GAP_ANALYSIS.md';s=changes[f];start=s.index('The historical suggested order RA1–RA8');end=s.index('Local package is arm64/macOS',start);s=s[:start]+'''RA1–RA9 implementation and all required own canonical gates are complete for this freeze.
Packet195/root factual checks and196/196b derived pointer records bind actual source;197c records
bounded controls;198 records all21 endings/fixed3/guards;200 records full13, including the first
complete canonical181-case audit pass. The earlier copy chronology remains historical evidence.
The current51 HOWTOSEE rows below remain unchanged and require the user's actual verification.

The remaining independent work is R172's five fresh slices (3+2) and fresh lead, their own frozen
validation/reconciliation, and any resulting remediation. Implementers/preparers cannot supply
that acceptance. The183-ID ledger keeps every retained implementation claim/limit and exact
unverified scope; severity is for the fresh panel, never assumed nonblocking. In particular,
source nonreads/model differences are not original-engine nonuse; gathering through existing
footprints, broader flight timing, interface/event omissions, unmeasured scale/platform/license
scope and user-quality obligations retain their specific limitations. No blanket debt closure
or policy approval follows from passed commands.\n\n'''+s[end:];changes[f]=s
# The original report/table remains useful; fix obsolete current checklist port/lease status directly.
f='docs/RA9_INTEGRATION_CHECKLIST.md';s=changes[f];s=s.replace('Root release after RA8 closure, refresh owner final docs commit, apply coherent source/evidence hunks without replacing history, rebind actual live inputs','Actual193port after RA8release complete, canonical history retained;195/196/196b factual input binding complete')
s=s.replace('After explicit RA8 release, refresh once against actual closureHEAD and current output bytes, retain all canonical runtime/history, archive exact evidence and rebind actual live inputs','Actual193 applied261paths/174new against closure8f1b1dc;580 selected proof/preimage files archived, all4runtime deltas/history retained;195/196/196b factual binding complete')
s=s.replace('Any\nfurther command requires owner coordination.','Later review/native commands require explicit coordinated ownership.');changes[f]=s
# Replace stale current introductions and the completed operational checklist directly.
f='docs/CONSTRUCTION_SYSTEM.md';s=changes[f];start=s.index('This describes current source behavior in the isolated RA9 validation copy.');end=s.index('## Lobby, technology and production',start);s=s[:start]+'''This describes the canonical RA1–RA9 source at freezef58e8d9 and its actual required validation.
Packet195/root factual checks and summary200 record the source and complete implementation
results. Historical copy audits131/136/159, the six-only168 repair run and the M9 report below
retain their original scopes; they are not current-source acceptance or original-engine fidelity.
The plan and exact decisions retain authority over model assumptions. Fresh independent review
and user-only checks remain pending as of this implementation handoff.

'''+s[end:];changes[f]=s
f='docs/RA2_GAP_ANALYSIS.md';s=changes[f];start=s.index('This is the current source reconciliation in the ordinary preliminary RA9 validation copy,');end=s.index("The target is original Red Alert 2",start);s=s[:start]+'''This is the canonical RA1–RA9 source reconciliation at freezef58e8d9, after RA8 closure8f1b1dc.
Actual195/196/196b factual source binding,197c bounded controls,198 required isolated checks
and200 full13 results are complete. The unfiltered181-case audit now passes on canonical source;
preliminary failed/repaired/interrupted runs remain historical. Fresh independent review of this
freeze and user-only verification remain pending at this implementation handoff. Implementers
and preparers are not the independent panel; passing commands do not approve source policies.

'''+s[end:];s=s.replace('R165 release/re-entry/recovery is actual AI source policy, still requiring final-source native acceptance.','R165 release/re-entry/recovery is actual AI source policy with native regressions passing in\nthe canonical workspace; the fresh panel independently assesses the policy and its adequacy.')
s=s.replace('This documentation task runs\nno Cargo, native simulation, render, audio or package process.','The documentation reconciliation is separate from the actual canonical Cargo/native/package\nruns in200; neither document inspection nor those runs supplies user quality approval.')
notes='''## Current references for the unchanged user checklist

The51 rows below retain their original wording and PENDING user-quality status. The following
current expectations resolve their historical references to a later published implementation:

- **Garrisons / R165:** GI/Conscript enter eligible House/City/Tower hosts while capacity, owner
  and health gates permit. D requests evacuation; exits use legal room and retry when blocked,
  retaining passenger identity/health/rank. Red-health hosts evict and reject new occupation;
  once emptied, the garrison ownership/selection record clears. R165 concerns AI admission and
  recovery: a depleted AI preserves released troops for offence rather than immediately sending
  them back without actual income, source-cost cash, or a useful currently observed defensive
  target. It does not change the human D command or prove all original garrison semantics.
  See [garrison code](../crates/sim/src/garrison.rs), [R165](DECISIONS.md#r165--preserve-unfunded-garrison-recovery-beyond-the-release-command-ra8),
  [actual AI controls](../crates/sim/tests/ra8_ai.rs) and the garrison rows of the [debt ledger](RA2_HISTORICAL_DEBT.tsv).
- **C4 / Ivan / actorless Props:** mapped occupied civilian Props are real command targets
  without fabricated Actor entities. Their known/hidden/dead, owner and eligibility gates stay
  distinct. Tanya C4 is a delayed charge; ordinary naval attack remains a separate route. Ivan
  expiry uses400 base damage with actual armour/effect handling, so it need not destroy every
  host. A valid Engineer defuse prevents the later Ivan explosion; known non-C4 BombSight
  approach respects explicit priorities/Guard. Temporal channel freeze/release is another route.
  The actual C4, Ivan, defuse, beam-break and rejection expectations are in
  [connected Props controls](../crates/sim/tests/ra9_scenery_connected.rs) and
  [remaining Props controls](../crates/sim/tests/ra9_remaining_props.rs); their passive pre-tick
  fixtures are qualified there. Broad production/duel/soak worlds still omit Props and are not
  proof of these actorless routes.
- **Weighted / nested transports:** admission uses immediate-child Size, SizeLimit and capacity,
  not just passenger count. Four Size3 Grizzlies fill the landing craft's12 slots; a fifth is
  rejected. Two Size2 Drones plus one Size1 GI fill a Flak Track's5 slots. Nested stored cargo
  remains attached to its inner transport. Root control/unload propagates the current owner;
  invalid Chronoshift death removes the root and stored descendants. Exact death, identity,
  owner, build-limit and IFV restoration scopes are in [transport controls](../crates/sim/tests/ra9_transport.rs),
  [lifecycle controls](../crates/sim/tests/ra9_transport_lifecycle.rs),
  [build-limit controls](../crates/sim/tests/ra9_transport_buildlimit.rs) and the transport rows
  of the [debt ledger](RA2_HISTORICAL_DEBT.tsv). These current model rules do not certify every
  original loading animation or subjective control feel.

These are current implementation/test expectations bound by195 and executed in200; they are
available now, not unpublished promises. The fresh panel independently assesses adequacy and
severity, and the user still performs every visual/audio/feel check below.

'''
assert s.count('## How to perform user verification')==1;s=s.replace('## How to perform user verification',notes+'## How to perform user verification',1);changes[f]=s
f='docs/reviews/RA2-parity-review-requirements.md';s=changes[f];start=s.index('This is a current-source dispatch map for the ordinary preliminary RA9 validation copy,');end=s.index('R172 remains binding:',start);s=s[:start]+'''This assigns the frozen canonical cumulative RA0–RA9 implementation and retained regressions
to the forthcoming independent panel. Actual source/loader/input bindings and required gates
are supplied in195/197c/198/200; the header identifies source and gate-launch commits. The
stopped-RA5 scope below is historical. At this implementation handoff a fresh panel review of
this freeze has not begun. The dispatcher supplies its exact clean final TIP and round in the
dispatch message; the subsequent dated report owns independent severity and acceptance.

'''+s[end:];changes[f]=s
f='docs/prompts/ra2-parity-review/README.md';s=changes[f];start=s.index('No review has begun in this ordinary preliminary RA9 copy.');end=s.index('R172 authorizes five fresh independent slices',start);s=s[:start]+'''At this canonical implementation handoff, the fresh independent review of this freeze has
not begun. Source/loader/input bindings and actual complete validation are supplied in195–200;
this index is the current complete ownership map, not a review or approval. The dispatcher
provides the exact clean final documentation/evidence TIP and round in its dispatch message.
The subsequent dated report records the panel outcome.

'''+s[end:];s=s.replace('explicit current module/test/audit map. Fill final TIP/ROUND into these preserved briefs using\nthat supplement; do not dispatch the old stopped-RA5 scope as the complete programme.','explicit current module/test/audit map. Use that supplement and the dispatcher-provided actual\nTIP/round; the preserved older brief examples are historical and do not reduce cumulative scope.')
changes[f]=s
f='docs/RA9_INTEGRATION_CHECKLIST.md';old=changes[f];start=old.index('| Requirement |');end=old.index('No copy Cargo/native process remains;',start) if 'No copy Cargo/native process remains;' in old[start:] else old.index('No Cargo/native process remains;',start)
table=f'''| Requirement | Actual canonical implementation and execution | Independent/user boundary |
|---|---|---|
| Schema10/data | Required parser/loaded/hash/runtime/generator migrations; required generator/reference commands exit0. Actual181 catalogue values are bound to unchanged loader/data by195 | Review exact numerical/model applicability; user quality is separate |
| Whole audit |197c whole1/0:16,211physical=13,419Compared+2,792GameOwned;7,108pairs;5,022atoms=3,111Compared+1,911dispositions; full199 audit181/0/0/0filtered | Fresh panel independently checks coverage, demands and semantic promises |
| Exact ownership |1,911typed atoms=759current-model nonreads+1,152intentional differences;0original-engine-unused;80separate authored arrays. Actual unfiltered negatives pass | No source/default/alias/Own marker impersonates comparison or original-engine nonuse |
|174 completion atoms |151actual export;132Whole+19animation+19water+4inactive arguments across151keys. Historical131/136failures retained;159175/6 and168six-only results retained; later canonical181pass actual | Component coverage grants neither Whole nor sibling credit; exact original applicability remains reviewable |
| Native/client | Actual release workspace{w['passed']}/0/0/{w['filtered']}filtered across{w['result_targets']}result targets, with original assertion purposes retained | RulesNone and controlled input/view fixtures retain their narrower scopes; no user-quality approval |
| Yard/timing/order debt | Final workspace executes344scenario/21map bounded yard control, exact Lost200/Reload100 boundaries and retained27spacing, plus real AttackMove duplicate-event correction/regression | Keep broader flight/host/scenery/original-semantic scope explicit; no blanket debt-family closure |
| Late twins/causes |187actual positive/restored positive41738/72000,late444/8/6; RNG36001 and observer-only omission intendedFAIL; exact immediate/later restoration. Canonical workspace executes the unchanged selected positive | Functional copy timing grants no isolated performance credit; causal scopes remain individual |
| Ending distributions | Actual production756Won, full duels756Won and all21ten-playerWon within original limits; exact distributions/tails in198/200 | No all-seed/human balance or universal headroom theorem |
| Source/authority |195root verification656source/97core/64spans;196factual pointer binding;196bexact649derived branch tokens/103families; all669/64gate guards0drift | Factual verification is not policy, severity, native or final-review approval |
| Evidence history |193actual portable archive,190resolver and every failed/compiler/fixture/interrupted/causal/restoration scope preserved; later194–200 actual canonical receipts included | No all-pass aggregate erases failures or promotes a historical snapshot |
| Canonical port |193applied261paths/174new after closure8f1b1dc, retaining all4runtime deltas/R189–R191/history/prompts; source frozenf58e8d9 | Final dispatcher supplies exact clean documentation/evidence TIP; no stale-copy overwrite |
| Required13 | All13own canonical commands exit0, including entire unfiltered release workspace, debug600smoke, benchmark, soak, full duels and packaged600smoke | Fresh review's independent validation is separate; no extra broad copy/debug sweep claimed |
| Required isolated5 |21Won;maximum mean{maxmean:.6f}ms<5; fixed3 original timing windows and3example guards PASS. Same source carries these proofs through full13 | Mean/p99/max/busy windows are distinct; fixed/soak windows do not imply every final world Won |
| Documents/debt/users |183stable IDs/aliases retained, current loaded catalogue/module lists and exact51HOWTOSEE bytes retained | Every retained claim's severity/acceptance belongs fresh panel; user-only checks remain PENDING |
| Review | At implementation handoff, fresh5slices3+2 and freshlead pending underR172, with original P0/P1 rules | Implementers/preparers are ineligible; only actual dated independent reports supply GO/NO-GO |

'''
changes[f]=old[:start]+table+old[end:]

# Ledger: preserve all183 IDs/history and qualify evidence individually without accepting claims.
f='docs/RA2_HISTORICAL_DEBT.tsv';lines=original[f].decode().splitlines();hd=lines[0].split('\t');rows=[dict(zip(hd,line.split('\t'))) for line in lines[1:]];assert len(rows)==183 and len({x['ledger_id'] for x in rows})==183
sections=[]
for chunk in re.split(r'(?m)^\s+Running ',(OUT/'04-test.log').read_text())[1:]:
 title,_,body=chunk.partition('\n');match=re.search(r'^test result:.*? (\d+) passed; (\d+) failed; (\d+) ignored;',body,re.M)
 if match:sections.append((title,int(match[1])))
def target_evidence(refs):
 found=[]
 for path in sorted(set(re.findall(r'crates/(?:sim|game)/tests/[A-Za-z0-9_]+\.rs',refs))):
  name=Path(path).name
  for title,count in sections:
   if 'tests/'+name in title:found.append(path+f':{count}/0/0 in04-test.log')
 if 'ra9_audit/' in refs:found.append('ra9_data_parity.rs:181/0/0 unfiltered in04-test.log')
 if 'crates/game/src/' in refs:found.append('game binary unit-test target in04-test.log; exact App/fixture scope retained')
 if 'crates/sim/src/' in refs:found.append('sim library unit-test target in04-test.log; exact unit/native scope retained')
 return '; '.join(dict.fromkeys(found))
for row in rows:
 assert row['acceptance']=='NOT_ACCEPTED_BY_DOCUMENTATION'
 topic=row['reconciliation_topic'];old=row['execution_evidence'];actual=target_evidence(row['test_refs'])
 row['execution_evidence']='HISTORICAL PREPARATION/EVIDENCE (scope preserved): '+old+' | ACTUAL CANONICAL HANDOFF:195source/root factual binding;200required13PASS;'+(actual if actual else 'no new per-label targeted execution claim beyond the existing listed witness scopes')+'. This does not independently accept the historical claim.'
 if topic=='START-01':
  row['current_disposition']='CANONICAL_SCOPED_YARD_SWEEP_PASS_REVIEW_PENDING';row['current_source_assessment']='Actual185copy and final canonical unfiltered ra9_start_setup target pass:21maps/344supported growOFF/growON2–10 scenarios, player-owned yard setup/occupancy/I6 and public Move exits, initial path cost≤40/unchanged256ticks. Failed compiler/detour/neutral census fixtures remain history.';row['test_refs']+=';crates/sim/tests/ra9_start_setup.rs';row['remaining_obligation']='Fresh panel must assess exact original acceptance coverage; bounded setup/Move exits are not all-to-Won or a claim of every host/scenery combination. No documentation-only acceptance.'
 elif topic=='MOVE-DUP-01':
  row['current_disposition']='REPRODUCED_CORRECTED_CANONICAL_REGRESSION_PASS_REVIEW_PENDING';row['current_source_assessment']='181 reproduced twoMoveCompleted around actual AttackMove combat/death at destination;182 corrected settled-destination resumption. Samebody and no-enemy control pass, and final canonical ra7_orders target passes. No newstate/balance/cap change.';row['remaining_obligation']='Fresh panel must classify the original finding and independently assess the actual correction and regression adequacy under D29/D32; preserve before/fix/after raw evidence.'
 elif topic=='SPAWN-01':
  row['current_source_assessment']='Actual181/final canonical ra4_spawns pass exact current Lost200/relaunch and returned-carrier Reload100 boundaries; existing27spacing and broader tests retained. These scoped controls do not prove every original flight/lost-carrier timing.';row['remaining_obligation']='Retain broader original timing/unverified scope explicitly; fresh panel classifies adequacy and any remaining acceptance risk. No blanket SPAWN-family closure.'
 elif topic=='NEAR-01':
  row['current_disposition']='ACTUAL_FINAL_SOURCE_DISTRIBUTIONS_RECORDED_REVIEW_PENDING';row['current_source_assessment']='Actual final canonical production756Won and duels756Won plus all21Won; per-case original limits and actual tick/normalized distributions are recorded in200 and198. Historical near-limit winners/failures remain historical; no all-seed or universal headroom theorem.';row['remaining_obligation']='Fresh panel assesses current actual distributions, fixed limits and AI policy/financing assumptions; human strategy/balance remains user-only.'
 elif topic=='PACKAGESIZE-01':
  row['current_disposition']='ACTUAL_DELIVERY_SIZE_RECORDED_REVIEW_PENDING';row['current_source_assessment']=f"Actual final package binary{m['package']['binary']['bytes']}bytes/archive{m['package']['archive']['bytes']}bytes with SHA256 recorded in200; packaged600-frame smoke passes. No strip/sign/notarize/universal/other-Mac claim.";row['remaining_obligation']='Review actual distribution/license/platform scope; fresh user/platform checks remain separate from local packaged smoke.'
 elif topic=='SOAKPEAK-01':
  row['current_disposition']='ACTUAL_POOLED_MEASUREMENT_RECORDED_REVIEW_PENDING';row['current_source_assessment']=f"Actual fixed-window soak{m['soak']['total_ticks']}ticks; pooled concurrent peak{m['soak']['peak_heap_mib']}MiB within1024MiB;{m['soak']['final_worlds_still_running_per_repeat']}finalRunning worlds per repeat, not all-to-Won evidence. Per-world heap is not measured by this pooled peak.";row['remaining_obligation']='Preserve pooled/per-world and fixed-window/ending distinctions; fresh panel classifies original obligation using R56item6.'
 # Current remaining gaps are explicit implementation claims, never assumed panel-approved P2.
 row['remaining_obligation']+=' Severity and acceptance remain for the fresh independent panel; original P0/P1 rules are unchanged.'
changes[f]='\t'.join(hd)+'\n'+''.join('\t'.join(row[x] for x in hd)+'\n' for row in rows)
# Source/data catalogue values stay exact; only current provenance prose may be updated.
f='docs/RA9_LOADED_CATALOGUE.md';changes[f]=original[f].decode()+'\nCanonical handoff provenance:actual181 loader output above is retained byte-for-byte. Packet195\nbinds the unchanged loader/definition/data bytes after integration; required canonical200 gates\npass. No second catalogue execution, original-engine fidelity or review/user-quality approval\nis inferred. The fresh panel independently assesses the data/model promises.\n'
# Resolve current document links into the actual materialized archive; retain every historical receipt byte.
archive=json.loads((OUT/'193-canonical-port-and-archive.json').read_text())
portable={Path(x['source']).name:x['repository_path'] for x in archive['archives']}
link_repairs=[]
for f,s in list(changes.items()):
 if not f.endswith('.md'):continue
 saved=''
 if f=='docs/RA2_GAP_ANALYSIS.md':
  a0=s.index('## How to perform user verification');a1=s.index('## Historical baseline and source provenance');saved=s[a0:a1];s=s[:a0]+'HOWTOSEE_PROTECTED_SECTION_SENTINEL\n'+s[a1:]
 def repair(mt):
  old=mt.group(1);name=old.rsplit('/',1)[-1]
  if name not in portable:return mt.group(0)
  new=link(f,portable[name]);link_repairs.append({'document':f,'before':old,'after':new});return '('+new+')'
 s=re.sub(r'\(([^)\s]*ra9-preliminary-logs/[^)\s]+)\)',repair,s)
 if saved:s=s.replace('HOWTOSEE_PROTECTED_SECTION_SENTINEL\n',saved,1)
 changes[f]=s
# Write after preparing every output and preserving all preimages.
for f,s in changes.items():
 pre=OUT/('201-before-'+f.replace('/','__'));assert not pre.exists();pre.write_bytes(original[f]);(ROOT/f).write_text(s)
for path,h in protected.items():assert sha((ROOT/path).read_bytes())==h,path
for a in packet['authority_fragments']:
 b=(ROOT/a['document_path']).read_bytes();at=b.index(a['section_start'].encode());assert sha(b[at:at+a['span_bytes']])==a['sha256']
b=(ROOT/'docs/RA2_GAP_ANALYSIS.md').read_bytes();assert b[b.index(b'## How to perform user verification'):b.index(b'## Historical baseline and source provenance')]==how
record={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'status':'ACTUAL_CANONICAL_CURRENT_OUTPUTS_ONLY_NO_REVIEW_OR_POLICY_APPROVAL','outputs':[{'path':f,'before_sha256':sha(original[f]),'after_sha256':sha((ROOT/f).read_bytes()),'preimage':str((OUT/('201-before-'+f.replace('/','__'))).relative_to(ROOT))} for f in changes],'source656_drift':[],'authority64_drift':[],'HOWTOSEE_rows':51,'HOWTOSEE_sha256':sha(how),'debt_rows':len(rows),'debt_ids_preserved':True,'acceptance_counts':dict(collections.Counter(row['acceptance'] for row in rows)),'portable_current_document_link_repairs':link_repairs,'review':'Pending at this implementation handoff for this freeze; subsequent dated independent reports authoritative.','new_external_directories':[]}
(OUT/'201-current-canonical-output-refresh.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps({'outputs':len(changes),'debt_rows':len(rows),'source_drift':[],'authority_drift':[]},indent=2))
