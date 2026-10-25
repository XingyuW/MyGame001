"""Factual current metadata and review scope on an exact new freeze; no approval."""
from pathlib import Path
import datetime,hashlib,json,re,subprocess,sys,os
R=Path('/Users/xingyuwang/GameProjects/MyGame001');S=Path(__file__).parent;P=Path(sys.argv[1]).resolve();N=Path(sys.argv[2]).resolve();D=json.loads(P.read_bytes());notes=json.loads(N.read_bytes());Fpath=P.parent/'frozen-validation-inputs.json';F=json.loads(Fpath.read_bytes());sha=lambda b:hashlib.sha256(b).hexdigest()
def protect():
 for row in F['source_files']:assert sha((R/row['path']).read_bytes())==row['sha256'],row['path']
 for a in F['authority_fragments']:
  b=(R/a['document_path']).read_bytes();at=b.index(a['section_start'].encode());assert sha(b[at:at+a['span_bytes']])==a['sha256']
 b=(R/'docs/RA2_GAP_ANALYSIS.md').read_bytes();a=b.index(b'## How to perform user verification');z=b.index(b'## Historical baseline and source provenance');assert sha(b[a:z])==F['howtosee']['sha256']
protect();assert len(F['source_files'])==D['source_count']+15
for row in notes['receipts']:assert sha((R/row['path']).read_bytes())==row['sha256'],row['path']
pattern=re.compile(r'(?m)^\s*#\[test\]\s*(?:#\[[^\]]*\]\s*)*(?:async\s+)?fn\s+(\w+)');identity=[]
for n in subprocess.check_output(['git','-C',str(R),'diff','--name-only','1cedaee','--','crates'],text=True).splitlines():
 if not n.endswith('.rs'):continue
 old=subprocess.run(['git','-C',str(R),'show','1cedaee:'+n],capture_output=True,text=True)
 if old.returncode:continue
 a=set(pattern.findall(old.stdout));b=set(pattern.findall((R/n).read_text()));assert a<=b,(n,sorted(a-b));identity.append({'path':n,'original_test_names_preserved':len(a),'added_names':sorted(b-a)})
files=list(R.glob('crates/**/*.rs'));literal=sum(len(re.findall(r'(?m)^\s*#\[test\]\s*$',f.read_text())) for f in files);assert not [(str(f),x) for f in files for x in re.findall(r'(?m)^\s*#\[ignore[^\n]*',f.read_text())]
counts=json.loads((S/'round2-c3-module-scope.json').read_bytes())['groups'];c={k:len(v) for k,v in counts.items()};stamp=datetime.datetime.now(datetime.timezone.utc).isoformat()
text=f"""## Current corrected round2 candidate — {stamp}

This current checkpoint supersedes the dated execution headlines below. R201 preserves the
actual failed/interrupted review-2 attempt; none of that attempt is final acceptance.
R202 records the subsequent observed-cause correction and exact bounded regression evidence.

{notes['current_observations']}

The exact new source manifest is `{Fpath.relative_to(R)}`,
SHA256 {sha(Fpath.read_bytes())}: {len(F['source_files'])} inputs ({D['source_count']} source plus15 factual
outputs),64 protected authority spans and all51 HOWTOSEE bytes. Factual source/hash propagation
changes no class, reason, route, guard, decision, source literal or qualified native receipt.
The source census is {c['sim_source']} sim/{c['game_source']} game Rust files;
{c['audit_rust']} audit Rust/{c['audit_tsv']} TSV/{c['audit_txt']} TXT siblings and{c['ra9_top_level_sim_targets']} top-level RA9 sim targets.
Literal test attributes:{literal}, zero ignore attributes; original modified-file test names
remain. File/declaration counts are preservation observations, never native executions.

The previous corrected C-source all21 wins/fixed3/guards remain historical scoped evidence.
This revised source still requires actual all21-to-Won/fixed3/guards, ONE successful unchanged
full13 shared under R194, release/package missing-image failure/restoration, all five full
cumulative revised independent slices and a fresh binding lead. Every original scenario,
native worker, cap, deterministic/heap/performance and winning predicate remains binding.
All183 debt IDs/aliases and51 user-only quality checks remain; no GO, merge/tag or quality
approval exists. Both projects keep running; no unrelated process is changed. R172/R199 govern
3+2 slice batches and whole revised cumulative inspection. Earlier snapshots remain historical.
"""
changes={}
for n in ['README.md','docs/HANDOFF.md','docs/RA2_GAP_ANALYSIS.md','docs/reviews/RA2-parity-review-requirements.md','docs/reviews/RA2-parity-reviews-outstanding.md']:
 b=(R/n).read_bytes();block='<!-- RA9_REMEDIATION1_CURRENT_BEGIN -->\n'+text+'<!-- RA9_REMEDIATION1_CURRENT_END -->';t,k=re.subn(r'<!-- RA9_REMEDIATION1_CURRENT_BEGIN -->.*?<!-- RA9_REMEDIATION1_CURRENT_END -->',lambda _:block,b.decode(),flags=re.S);assert k==1;changes[n]=(b,t.encode())
brief=f"""<!-- RA9_REVISED_SCOPE_CURRENT_BEGIN -->
## Finished module list and revised source — {stamp}

R202 source has{c['sim_source']} sim/{c['game_source']} game Rust files. Entire path/hash census:
`{S/'round2-c3-module-scope.json'}`; complete primary/interface assignments:
`{S/'round2-c3-production-ownership.json'}`. Cover the entire cumulative programme and each revised
source/interface/policy/evidence path, authenticating byte-identical retained scope against all
preserved initial and revised source reports. These file counts are not native test counts.
Frozen manifest:`{Fpath.relative_to(R)}`, SHA256{sha(Fpath.read_bytes())}.
R201 actual non-Won is corrected only to the scope of R202's real evidence; final complete
mandatory acceptance remains pending until actually executed. R172/R194/R199 retain full3+2
independent coverage and a fresh binding lead. All51 user-only quality rows stay pending.
Dispatch supplies the actual clean TIP, native phase/results and all prior/final reports.
<!-- RA9_REVISED_SCOPE_CURRENT_END -->"""
for leaf in ['README.md','slice-sim1.md','slice-sim2.md','slice-client.md','slice-data.md','slice-tests.md','lead.md']:
 n='docs/prompts/ra2-parity-review/'+leaf;b=(R/n).read_bytes();t,k=re.subn(r'<!-- RA9_REVISED_SCOPE_CURRENT_BEGIN -->.*?<!-- RA9_REVISED_SCOPE_CURRENT_END -->',lambda _:brief,b.decode(),flags=re.S);assert k==1;changes[n]=(b,t.encode())
n='docs/DECISIONS.md';b=(R/n).read_bytes();assert b'### R202.' not in b;changes[n]=(b,b+('\n\n### R202. '+notes['decision_title']+'\n\n'+notes['decision_text']+'\n\n'+text).encode())
n='docs/STATUS.md';b=(R/n).read_bytes();assert b.startswith(b'# Milestone status\n\n');changes[n]=(b,b.replace(b'# Milestone status\n\n',('# Milestone status\n\n'+text+'\n').encode(),1))
out=R/'docs/validation/ra2-parity/remediation-2/C-preflight-freeze-02';assert not out.exists();out.mkdir()
for n,(before,after) in changes.items():
 q=out/'metadata-preimages'/n;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(before);(R/n).write_bytes(after)
protect();record={'status':'ACTUAL_NEW_FREEZE_FACTUAL_METADATA_FINAL_ACCEPTANCE_PENDING','utc':stamp,'packet_sha256':sha(P.read_bytes()),'freeze_sha256':sha(Fpath.read_bytes()),'notes_sha256':sha(N.read_bytes()),'input_count':len(F['source_files']),'authority_count':64,'howtosee_sha256':F['howtosee']['sha256'],'module_counts':c,'literal_test_attributes':literal,'test_identity_preservation':identity,'receipts':notes['receipts'],'changes':[{'path':n,'before_sha256':sha(b),'after_sha256':sha(a)} for n,(b,a) in changes.items()]};(out/'root-notes.json').write_bytes(N.read_bytes());(out/'metadata.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps({'output':str(out),'inputs':len(F['source_files']),'literal_tests':literal,'metadata_files':len(changes)}))
