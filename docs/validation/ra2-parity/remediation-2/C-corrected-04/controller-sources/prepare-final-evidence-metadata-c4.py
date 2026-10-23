"""Factual closing metadata only after all native gates pass; independent decision remains pending."""
from pathlib import Path
import csv,datetime,hashlib,json,re,subprocess,sys
R=Path('/Users/xingyuwang/GameProjects/MyGame001');S=Path(__file__).parent
V=R/'docs/validation/ra2-parity';G=V/sys.argv[1];summary=G/'final-validation-summary.json';A=json.loads(summary.read_bytes())
assert A['status']=='ACTUAL_CORRECTED_SHARED_RA9_FINAL_REQUIRED13_AND_ISOLATED5_PASS_NOT_REVIEW_OR_USER_APPROVAL'
Fpath=Path(A['frozen_manifest']['path']);F=json.loads(Fpath.read_bytes());sha=lambda b:hashlib.sha256(b).hexdigest()
def protect():
 for x in F['source_files']:assert sha((R/x['path']).read_bytes())==x['sha256'],x['path']
 for a in F['authority_fragments']:
  b=(R/a['document_path']).read_bytes();at=b.index(a['section_start'].encode());assert sha(b[at:at+a['span_bytes']])==a['sha256']
 b=(R/'docs/RA2_GAP_ANALYSIS.md').read_bytes();at=b.index(b'## How to perform user verification');end=b.index(b'## Historical baseline and source provenance');assert sha(b[at:end])==F['howtosee']['sha256']
protect();ledger=(R/'docs/RA2_HISTORICAL_DEBT.tsv').read_bytes();rows=list(csv.DictReader(ledger.decode().splitlines(),delimiter='\t'));assert len(rows)==183 and len({r['ledger_id'] for r in rows})==183
I=json.loads((R/A['isolated_acceptance']['path']).read_bytes());assert I['source_files']==len(F['source_files'])
out=V/'remediation-2/C-final-evidence-03';assert not out.exists();out.mkdir()
for name in ['round2-c4-module-scope.json','round2-c4-production-ownership.json']:(out/name).write_bytes((S/name).read_bytes())
p=A['production'];d=A['duels'];so=A['soak'];bench=A['benchmark'];w=A['workspace']
text=f"""## Current corrected-source native acceptance — independent decision pending

This section supersedes earlier copy/pending execution wording below. Dated execution passages
remain historical evidence; their retained model boundaries and user-only checks still apply.

The corrected cumulative candidate is bound by the exact frozen input manifest recorded below.
The single complete shared RA9/final validation launched at {A['gate_launch_head']}; all 13
unchanged commands exited 0 with terminal PASSED. R194 authorizes sharing this one full run.
Exact raw counts and scope are in [final validation](/{summary.relative_to(R)}).
Workspace: {w['primary_passed']} primary unfiltered passes, zero failures/ignores/filters;
selected child probes are accounted separately in that report. The whole RA9 audit passed
185/0/0/0 together. File/declaration counts are not test executions.

Production: {p['won']} Won, median/p90/max ticks {p['ticks']['median']}/{p['ticks']['p90_nearest_rank']}/{p['ticks']['max']}.
Duels: {d['explicit_Won']} Won, median/p90/max ticks {d['ticks']['median']}/{d['ticks']['p90_nearest_rank']}/{d['ticks']['max']}.
Original map-scaled production caps, 300000 duel cap and base MATCH_TICK_LIMIT=72000 remain.
Isolated all21 each reached Won before the unchanged400000 cap; median/p90/max
{I['all21']['won_tick_distribution']['median']}/{I['all21']['won_tick_distribution']['p90_nearest_rank']}/{I['all21']['won_tick_distribution']['max']}.
All three fixed256x256 windows and three example refusal guards passed. All overall means stayed below5ms.
Across those measured map runs the largest busy1000 mean was {max(x["busiest_1000_mean_ms"] for x in I["all21"]["maps"]+I["fixed3"]["maps"])}ms,
largest p99 {max(x["p99_ms"] for x in I["all21"]["maps"]+I["fixed3"]["maps"])}ms and maximum tick
{max(x["max_ms"] for x in I["all21"]["maps"]+I["fixed3"]["maps"])}ms. The m13 gate is overall mean;
these observed tails are retained limits and do not establish rendered smoothness.

Soak: {so['configurations']} configurations ×2×36000={so['total_ticks']} deterministic ticks;
peak concurrent heap {so['peak_heap_mib']} MiB against {so['heap_budget_mib']} MiB. Final sampled
Running worlds ({so['final_worlds_still_running_per_repeat']} per repeat) are fixed-duration windows,
not Won claims; actual completed outcomes and the independent all-to-Won workloads retain scope.
Both benchmark200-original-unit workloads passed movement/combat, original5ms mean/busy and50ms
maximum/heap guards. Release/debug600-frame smokes and actual macOS package smoke passed.
Actual release and packaged missing-image processes exited1 with the path, then restored-image
processes each completed600 frames with map created:true; canonical assets remain unchanged.
Package/executable/archive and the actual phase11 README snapshot hashes are preserved; later
closing prose was not retroactively packaged. Signing/notarization/universal and other-machine
launch, notice completeness and subjective quality are not established by these processes.

All{len(F['source_files'])} source/factual inputs and64 authority spans remain exact. Both original cached INI hashes,
all51 HOWTOSEE bytes, original assertions and zero ignore attributes remain. The actual catalogue
is54units/46buildings/116weapons/61warheads/11armours/9countries/21maps/11plans/3difficulties.
16211 physical facts reconcile as13419Compared+2792GameOwned;7108 pairs succeed.
5022 source atoms are3111Compared+1911 specific dispositions:759CURRENT_MODEL_NONREAD and
1152INTENTIONAL_DIFFERENCE, zeroORIGINAL_ENGINE_UNUSED. Seventy-five numeric differences remain
explicit in ra2_differences.tsv; declarations do not establish original-engine consumption.

All183 historical debt IDs/aliases are preserved, with exact per-row scopes in the ledger and
independent reports. Native passage supplies execution within each assertion's actual scope;
it does not blanket-close retained model limitations or user-only items. All51 visual, audio,
readability, animation, pacing/feel, human victory/defeat and platform checks remain pending.
R4 map growth remains optional/default-off; R7 uses15-frame-to20-tick speed conversion;
R6/I6prime retains onevehicle/threeinfantry/oneaircraft per corresponding cell rules.
R172 two-batch full panel, R199 whole revised cumulative inspection and a fresh independent lead
remain binding. All five revised reports and lead must ingest these actual results; no root
self-approval, GO, merge/tag or user-quality approval is supplied by this factual update.
"""
# Resolve local links as normal repository-relative Markdown for portable documentation.
text=text.replace(f'(/{summary.relative_to(R)})',f'({summary.relative_to(R)})')
changes={}
for n in ['README.md','docs/HANDOFF.md','docs/RA2_GAP_ANALYSIS.md','docs/reviews/RA2-parity-review-requirements.md','docs/reviews/RA2-parity-reviews-outstanding.md']:
 b=(R/n).read_bytes();block='<!-- RA9_REMEDIATION1_CURRENT_BEGIN -->\n'+text+'<!-- RA9_REMEDIATION1_CURRENT_END -->';t,count=re.subn(r'<!-- RA9_REMEDIATION1_CURRENT_BEGIN -->.*?<!-- RA9_REMEDIATION1_CURRENT_END -->',lambda _:block,b.decode(),flags=re.S);assert count==1
 # Link relative to each document's location.
 relative=__import__('os').path.relpath(summary,(R/n).parent);t=t.replace('('+str(summary.relative_to(R))+')','('+relative+')');changes[n]=(b,t.encode())
for n in ['docs/RA9_INTEGRATION_CHECKLIST.md','docs/RA9_LOADED_CATALOGUE.md','docs/CONSTRUCTION_SYSTEM.md']:
 b=(R/n).read_bytes();relative=__import__('os').path.relpath(summary,(R/n).parent);prefix='<!-- RA9_FINAL_FACTUAL_CURRENT_BEGIN -->\n'+text.replace('('+str(summary.relative_to(R))+')','('+relative+')')+'<!-- RA9_FINAL_FACTUAL_CURRENT_END -->\n\n';assert b'RA9_FINAL_FACTUAL_CURRENT_BEGIN' not in b;changes[n]=(b,prefix.encode()+b)
n='docs/STATUS.md';b=(R/n).read_bytes();changes[n]=(b,b+('\n\n## RA9 complete corrected-source native acceptance — '+datetime.datetime.now(datetime.timezone.utc).date().isoformat()+'\n\n'+text).encode())
n='docs/DECISIONS.md';b=(R/n).read_bytes();assert b'### R205.' not in b;notes='\n\n### R205. Actual single shared corrected-source full13 and isolated acceptance\n\n'+text+'\nPreserved exact manifest SHA256 '+sha(Fpath.read_bytes())+'; final summary SHA256 '+sha(summary.read_bytes())+'. All183 ledger bytes remain exact; independent adjudication is still required.\n';changes[n]=(b,b+notes.encode())
for n,(before,after) in changes.items():
 q=out/'metadata-preimages'/n;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(before);target=R/n;tmp=target.with_name(target.name+'.ra9-next');tmp.write_bytes(after);tmp.replace(target)
protect();assert (R/'docs/RA2_HISTORICAL_DEBT.tsv').read_bytes()==ledger
record={'status':'FACTUAL_NATIVE_GATES_COMPLETE_INDEPENDENT_DECISION_PENDING','utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'native_summary_sha256':sha(summary.read_bytes()),'source_count':len(F['source_files']),'authority_spans':64,'howtosee_sha256':F['howtosee']['sha256'],'ledger_sha256':sha(ledger),'ledger_ids':len(rows),'changes':[{'path':n,'before_sha256':sha(b),'after_sha256':sha(a)} for n,(b,a) in changes.items()]};(out/'metadata.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record,indent=2))
