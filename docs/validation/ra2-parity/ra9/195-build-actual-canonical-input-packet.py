from pathlib import Path
import json,hashlib,datetime,subprocess,copy,re
r=Path.cwd();v=r/'docs/validation/ra2-parity/ra9';e=v/'evidence';old=json.loads((e/'166-six-test-repairs-input-scope.json').read_text());h=lambda b:hashlib.sha256(b).hexdigest()
def rec(path):
 b=(r/path).read_bytes();return {'path':path,'bytes':len(b),'sha256':h(b)}
paths=sorted({x['path'] for x in old['source_files']}|{'crates/sim/tests/ra9_start_setup.rs'});assert len(paths)==656
sources=[rec(f) for f in paths];auth=[]
for x in old['authority_fragments']:
 a=copy.deepcopy(x);b=(r/a['document_path']).read_bytes();at=b.index(a['section_start'].encode());assert h(b[at:at+a['span_bytes']])==a['sha256'];a['actual_start_byte']=at;auth.append(a)
assert len(auth)==64
before={x['path']:x for x in old['source_files']};delta=[{'path':x['path'],'before':before.get(x['path']),'after':x} for x in sources if before.get(x['path'])!=x]
core=[rec(f.as_posix()) for f in sorted(Path('crates').glob('*/src/**/*.rs'))];assert len(core)==97
canonical_runtime_paths=subprocess.check_output(['git','diff','--name-only','26ad1e384ff17aff2a0075a08f39d510f28d0067','8f1b1dc','--','crates','assets','scripts']).decode().splitlines();assert len(canonical_runtime_paths)==4
runtime=[]
for f in canonical_runtime_paths:
 base=subprocess.check_output(['git','show','8f1b1dc:'+f]);now=(r/f).read_bytes();runtime.append({'path':f,'closure_sha256':h(base),'actual_sha256':h(now),'exact':base==now})
# Both imported financing functions retain exact canonical function text; RA9 alterations elsewhere remain explicit.
def func(s,n):
 at=s.index('fn '+n+'(');start=s.index('{',at);depth=1;j=start+1
 while depth:
  depth+= (s[j]=='{')-(s[j]=='}');j+=1
 return s[at:j]
ca=subprocess.check_output(['git','show','8f1b1dc:crates/sim/src/ai.rs']).decode();aa=(r/'crates/sim/src/ai.rs').read_text()
for n in ['understrength_funding','sell_for_army']:assert func(ca,n)==func(aa,n)
rt=subprocess.check_output(['git','show','8f1b1dc:crates/sim/tests/ra8_ai.rs']).decode();ct=(r/'crates/sim/tests/ra8_ai.rs').read_text();start=rt.index('fn ra8_funding_') if 'fn ra8_funding_' in rt else -1
# Root already independently verified exact165-line appended RA8 block in178/191; verify by that saved receipt source tail.
block='\n'.join(rt.splitlines()[-165:])+'\n';assert ct.count(block)==1
prompts=[]
for f in ['docs/prompts/RA2_PARITY_PROMPT.md','docs/prompts/RA2_PARITY_CONTINUATION_PROMPT.md']:
 b=(r/f).read_bytes();assert b==subprocess.check_output(['git','show','8f1b1dc:'+f]);prompts.append(rec(f))
g=(r/'docs/RA2_GAP_ANALYSIS.md').read_bytes();a=g.index(b'## How to perform user verification');z=g.index(b'## Historical baseline and source provenance');assert h(g[a:z])=='8ae4edfadf97fc68138d03c9a12d9c1217cf0c1fb65cf6dd6010c77b1c5bd6c1'
outputs=['README.md','docs/HANDOFF.md','docs/CONSTRUCTION_SYSTEM.md','docs/RA2_GAP_ANALYSIS.md','docs/RA9_INTEGRATION_CHECKLIST.md','docs/RA9_LOADED_CATALOGUE.md','docs/STATUS.md','docs/DECISIONS.md']
obs=(r/'crates/sim/tests/reference/ra9_scope_observation.tsv').read_text().splitlines();keys=obs[0].split('\t');vals=obs[1].split('\t');ob=dict(zip(keys,vals))
packet={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'status':'ACTUAL_CANONICAL_FACTUAL_INPUT_PACKET_AWAITING_ROOT_READ_ONLY_VERIFICATION_NOT_APPROVAL','canonical_root':str(r),'canonical_predecessor':'8f1b1dcf399d23b89cbadce1cb1e322439ef85a8','immutable_copy_origin':old['immutable_origin'],'source_files':sources,'authority_fragments':auth,'documentation_outputs':[rec(f) for f in outputs],'excluded_files':old['excluded_files'],'authored_choices':old['authored_choices'],'source_delta_from166':delta,'core97':core,'actual_scope_producer':rec('docs/validation/ra2-parity/ra9/194-actual-canonical-scope-producer.json'),'source_row_refresh':rec('docs/validation/ra2-parity/ra9/194-actual-source-row-refresh.json'),'observed_scope':ob,'canonical_runtime_delta_preservation':runtime,'financing_function_preservation':'understrength_funding and sell_for_army byte-exact canonical closure function text;165-line helper/new2tests tail occurs exactly once byte-exact. Strategy and attrition whole files exact.','historical_prompts':prompts,'howtosee':{'rows':51,'bytes':z-a,'sha256':h(g[a:z])},'archive':rec('docs/validation/ra2-parity/ra9/193-canonical-port-and-archive.json'),'historical_qualification':'All actual copy evidence retains source/time scope.158 wholePASS,159 unfiltered175PASS/6FAIL,168 sixPASS;180–188 actual functional overlap failures/fixes/causal controls retained. New RA8 financing and RA9 combat/source/helper changes have actual limited receipts but no final canonical acceptance/performance. Historical pending lists remain historical only.','pending':['Root independent656source/64authority/core97/delta factual verification and explicit single excluded-output binding materialization.','Strict compilation; bounded wholepositive/six168cases/exactnewdifficulty test; freeze commit.','Own isolated all21Won/ten-player256mean<5ms/fixed3/threeguards then own fresh full13 unchanged source.','Final coherent docs/evidence commit and clean handoff; root freshfive3+2/freshlead/independentfull13.','User visual/audio/feel51HOWTOSEE rows unperformed.'],'binding_rules':'Exactly166source655paths plus newyard target =656, measured actual canonical bytes. Approval/root/derived receipt output pointers remain excluded to prevent self-reference; only actual root authorization may materialize factual hash pointers.64authorityliteralfragments unchanged, exact span lengths retained; no source-policy/original-engine/finalreview approval inferred.'}
f=v/'195-actual-canonical-input-scope.json';assert not f.exists();f.write_text(json.dumps(packet,indent=2)+'\n');binding=h(f.read_bytes());approval=(r/'crates/sim/tests/reference/ra9_scope_approval.tsv').read_text().splitlines();fields=approval[0].split('\t');row=approval[1].split('\t');row[:5]=vals;row[6]='PENDING_ROOT_ACTUAL_CANONICAL_195_FACTUAL_VERIFICATION';row[7]=binding;(v/'195-proposed-scope-approval.tsv').write_text('\t'.join(fields)+'\n'+'\t'.join(row)+'\n');print(json.dumps({'packet':str(f),'sha256':binding,'source_files':len(sources),'authority':len(auth),'core97':len(core),'source_delta_from166':[x['path'] for x in delta],'observed_scope':ob,'proposed_approval_sha256':h((v/'195-proposed-scope-approval.tsv').read_bytes())},indent=2))
