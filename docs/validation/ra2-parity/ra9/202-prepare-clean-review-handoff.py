"""Close actual validated implementation records; never supplies independent/user approval."""
from pathlib import Path
import hashlib,json,datetime,re,subprocess
ROOT=Path('/Users/xingyuwang/GameProjects/MyGame001');OUT=ROOT/'docs/validation/ra2-parity/ra9';sha=lambda b:hashlib.sha256(b).hexdigest()
m=json.loads((OUT/'200-final-canonical-validation-summary.json').read_text());assert m['status']=='ACTUAL_CANONICAL_RA9_REQUIRED13_AND_ISOLATED5_PASS_NOT_REVIEW_OR_USER_APPROVAL'
c=json.loads((OUT/'201-current-canonical-output-refresh.json').read_text());assert not c['source656_drift'] and not c['authority64_drift']
p=json.loads((OUT/'195-actual-canonical-input-scope.json').read_text());a=json.loads((OUT/'193-canonical-port-and-archive.json').read_text())
for x in p['source_files']:assert sha((ROOT/x['path']).read_bytes())==x['sha256'],x['path']
for x in p['authority_fragments']:
 b=(ROOT/x['document_path']).read_bytes();at=b.index(x['section_start'].encode());assert sha(b[at:at+x['span_bytes']])==x['sha256']
assert all((ROOT/x['repository_path']).is_file() and sha((ROOT/x['repository_path']).read_bytes())==x['sha256'] for x in a['archives'])
now=datetime.datetime.now(datetime.timezone.utc).isoformat();dt=now[:10];w=m['workspace']
index='''# RA9 canonical implementation validation and preserved evidence

This is the implementation handoff for source freeze `f58e8d9e1e6a8b8cf4f1ce31a80c229cfd6203c1`.
The final dispatcher identifies the clean documentation/evidence TIP and independent round.
This index and passed implementation checks do not supply independent review or user approval.

- [Actual final summary](200-final-canonical-validation-summary.json): every required command,
  unfiltered workspace/audit counts, distributions, timing/heap and packaged delivery scope.
- [Isolated five](198-isolated-acceptance-summary.json): all21 actual Won, mean/p99/max/busy-window
  measurements, fixed3 and three example guards. These are separate from deterministic soak windows.
- [Actual input packet](195-actual-canonical-input-scope.json), [root factual verification](195-root-independent-verification.json),
  [factual substitutions](196-actual-root-authorized-binding-refresh.json) and
  [derived branch corrections](196b-derived-branch-binding-refresh.json). Source/policy approval
  is not inferred from root authentication of physical facts.
- [Bounded checks](197c-fail-fast-process.json): whole positive, exact six repairs and new difficulty
  control. Failed sandbox/native attempts197/197b retain their raw logs and preimages.
- [Port/archive](193-canonical-port-and-archive.json), [current output reconciliation](201-current-canonical-output-refresh.json).
- [Immutable through189 resolver](evidence/190-through189-portable-evidence-file-list.json): exact
  original path plus hash, aliases and historical preimages. It is authoritative for preserved
  receipt references, including unchanged HOWTOSEE text. Do not resolve historical records by
  basename alone when their recorded hash identifies an older preimage.

The original copy remains at `~/Library/Caches/Codex/reviews/MyGame001/ra9-integration-validation`.
Its historical packets171/190/191 and all compiler, fixture, full-target, interrupted and causal
failures remain preserved. Actual193 materialized580 selected archive files (33,167,644 bytes),
without copying cache trees or large old acceptance masters unnecessarily. Original receipts
remain byte-exact; archive location does not relabel them as current-source execution.

Source ownership:48 audit Rust files,67 TSV/4 TXT siblings; all are reachable through the existing
audit target. The actual generated loader catalogue is [RA9_LOADED_CATALOGUE](../../../RA9_LOADED_CATALOGUE.md).
The183 stable debt IDs remain [individually recorded](../../../RA2_HISTORICAL_DEBT.tsv), with
NOT_ACCEPTED_BY_DOCUMENTATION distinct from an actual defect or unverified acceptance criterion.
Fresh reviewers must classify all acceptance/invariant/data risks under unchanged P0/P1 rules;
no implementation severity claim or presumed nonblocking label waives them. All51 user quality
rows in [GAP](../../../RA2_GAP_ANALYSIS.md) retain exact bytes and PENDING status.

Preserve configured Cargo/UV/PIP caches and one coordinated native owner. Historical review-target
examples do not authorize cache overrides or parallel native probes. No clone/worktree or substantial
/tmp directory was created by this integration. The existing ordinary copy and Cargo/package caches
remain necessary for provenance, review and delivery; no repository/worktree was deleted.
'''
f=OUT/'README.md';assert not f.exists();f.write_text(index)
# Historical provisional decision remains unchanged; append the actual completion record.
f=ROOT/'docs/DECISIONS.md';before=f.read_bytes();nums=[int(x) for x in re.findall(r'^### R(\d+)\b',before.decode(),re.M)];n=max(nums)+1
pre=OUT/'202-before-DECISIONS.md';assert not pre.exists();pre.write_bytes(before)
text=f'''\n### R{n} — RA9 actual implementation validation and independent handoff ({dt})

The source frozen in f58e8d9 and gate-launch `{m['gate_launch_head']}` completes its own required
isolated five and unchanged full thirteen-command validator on identical guarded inputs.
Actual summary200 and isolated198 retain commands, raw outputs, source guards and timing scopes.
The unfiltered release workspace reports {w['passed']} passed,0 failed,0 ignored,0 filtered across
{w['result_targets']} result targets, including all181 RA9 audit cases. Production and duels each
record756 Won; all21 ten-player maps reach Won within original limits and each measured mean<5ms.
Fixed3 and all three example guards pass. Mean/p99/max/busy-window tails remain separate facts,
not a universal smoothness or all-seed balance claim. R56 item6 still retains36,000-tick soak
windows: completed outcomes require Won, while final sampled worlds may remain Running.

The sandbox failure, stale-derived-binding native failure and every historical copy failure,
repair, interruption and intended causal failure remain preserved. Root195/196/196b authenticate
factual source/binding observations only. No test name was deleted and no ignore attribute was
introduced; lexical inventory1389→1743 functions (+354) is separate from actual Cargo counts.
Original prompts,64 exact authority spans and all51 HOWTOSEE row bytes remain unchanged.

Current output prose and the183-ID ledger now identify actual canonical execution alongside
precisely retained implementation/model/measurement limitations. NOT_ACCEPTED_BY_DOCUMENTATION
means lack of independent acceptance, not automatic proof of a defect or panel-approved P2.
The fresh panel must independently classify every acceptance/invariant/data risk; original P0/P1
rules remain binding. No blanket closure or presumed nonblocking label waives them.

At this implementation handoff, R172's five fresh independent slices (3+2) and fresh lead have
not begun for this freeze. Implementers/preparers are ineligible. The dispatcher supplies the
exact clean final documentation/evidence TIP and round; subsequent dated independent reports
own review outcome. User-only visual/audio/feel, normal human win/loss and external platform
checks remain PENDING. No user approval, original-engine fidelity, signing/notarization/universal
binary or other-Mac launch is inferred from local packaged smoke. No approval tag is created.
'''
f.write_bytes(before+text.encode())
f=ROOT/'docs/STATUS.md';before=f.read_bytes();pre=OUT/'202-before-final-STATUS.md';assert not pre.exists();pre.write_bytes(before)
f.write_bytes(before+f'''\n## RA9 canonical implementation handoff — {dt}

R{n} records completion of the actual required implementation checks. Sourcef58e8d9 and actual
gate-launch `{m['gate_launch_head']}` remain bound by195/196/196b; summary200 and isolated198
record all13+five exit0, workspace{w['passed']}/0/0/0filtered including181audit, both756Won
collections,21ten-playerWon/mean<5ms and every original smoke/benchmark/heap/timing boundary.
See [validation index](validation/ra2-parity/ra9/README.md) for exact tails, fixed-window/ending
distinctions, package measurements and preserved failures. All669inputs/64authorityspans retain
zero drift through the gates. Closing edits are excluded output provenance, not gameplay inputs.

At this handoff, fresh5slices3+2/freshlead and the coordinator's independent full13 remain pending
for this freeze. The exact final clean documentation/evidence TIP/round is supplied at dispatch;
subsequent dated review reports are authoritative. Every183retained implementation claim/debt
and51user-only quality rows remains explicit; severity and adequacy require fresh independent
classification, with no presumed nonblocking waiver. Configured caches and the preserved
ordinary copy remain necessary; no clone/worktree or substantial/tmp artifact was created.
'''.encode())
for x in p['authority_fragments']:
 b=(ROOT/x['document_path']).read_bytes();at=b.index(x['section_start'].encode());assert sha(b[at:at+x['span_bytes']])==x['sha256']
receipt={'utc':now,'decision':f'R{n}','status':'IMPLEMENTATION_HANDOFF_NOT_INDEPENDENT_OR_USER_APPROVAL','source_freeze':'f58e8d9e1e6a8b8cf4f1ce31a80c229cfd6203c1','gate_launch_head':m['gate_launch_head'],'source656_drift':[],'authority64_drift':[],'archive580_drift':[],'substantial_external_directories':[{'path':'/Users/xingyuwang/Library/Caches/Codex/reviews/MyGame001/ra9-integration-validation','created_this_closure':False,'remains_necessary':'Historical preparation proofs/preimages/packets; no deletion authorized'},{'path':'/Users/xingyuwang/Library/Caches/Codex/cargo-target','created_this_closure':False,'remains_necessary':'Configured compilation/package cache, preserved under user storage instruction'}],'new_external_directories':[],'pending':'Fresh independent panel/lead/coordinator full13 and any resulting remediation; all user-only quality/platform checks.'}
(OUT/'202-final-implementation-handoff.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(receipt,indent=2))
