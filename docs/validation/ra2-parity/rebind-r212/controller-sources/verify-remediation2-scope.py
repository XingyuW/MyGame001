"""Root independent byte verification. Never runs the producer or changes any input."""
import csv,datetime,hashlib,json,subprocess,sys
from pathlib import Path
root=Path('/Users/xingyuwang/GameProjects/MyGame001')
p=Path(sys.argv[1]).resolve(); assert p.is_relative_to(root/'docs/validation/ra2-parity/rebind-r212')
b=p.read_bytes(); packet=json.loads(b)
sha=lambda b:hashlib.sha256(b).hexdigest()
def record(path):
 b=path.read_bytes(); return {'path':str(path.relative_to(root)),'bytes':len(b),'sha256':sha(b)}
def fnv(b):
 n=14695981039346656037
 for c in b:n=((n^c)*1099511628211)%18446744073709551616
 return f'{n:016x}'
def table(path):return list(csv.DictReader(path.open(),delimiter='\t'))
oldpath=root/'docs/validation/ra2-parity/ra9/195-actual-canonical-input-scope.json'
old=json.loads(oldpath.read_bytes()); assert packet['base_packet']==record(oldpath)
excluded={x['path']:x for x in old['excluded_files']}
paths=set(subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard','--','crates','assets','scripts','.cargo','Cargo.toml','Cargo.lock','rust-toolchain.toml','.gitignore'],cwd=root,text=True).splitlines())
paths.update(x['path'] for x in old['source_files'])
actual=[record(root/x) for x in sorted(paths) if x not in excluded]
assert packet['source_files']==actual
assert packet['source_count']==len(actual) and not packet['unaccounted_candidates']
assert not packet['source_deleted_from195']
oldsrc={x['path']:x for x in old['source_files']}; assert set(oldsrc)<=set(x['path'] for x in actual)
assert packet['source_added_from195']==[x for x in actual if x['path'] not in oldsrc]
assert packet['source_changed_from195']==[{'before':oldsrc[x['path']],'after':x} for x in actual if x['path'] in oldsrc and x!=oldsrc[x['path']]]
derived=[record(root/x) for x in sorted(paths) if x in excluded and ('Approval/' in excluded[x]['reason'] or 'Validation-derived per-atom' in excluded[x]['reason'])]
assert packet['derived_outputs_before_binding']==derived
assert packet['all_candidates_count']==len(paths)
assert packet['other_candidate_exclusions']==[excluded[x] for x in sorted(paths) if x in excluded and x not in {r['path'] for r in derived}]
assert packet['authored_choices']==old['authored_choices']
authorities=[]
for a,before in zip(packet['authority_fragments'],old['authority_fragments'],strict=True):
 assert {k:v for k,v in a.items() if k!='actual_start_byte'}=={k:v for k,v in before.items() if k!='actual_start_byte'}
 raw=(root/a['document_path']).read_bytes(); at=raw.index(a['section_start'].encode()); assert at==a['actual_start_byte']; assert sha(raw[at:at+a['span_bytes']])==a['sha256']; authorities.append(a)
assert len(authorities)==64
corepaths=sorted(root.glob('crates/*/src/**/*.rs'))
assert packet['core_files']==[record(x) for x in corepaths] and packet['core_count']==len(corepaths)
census=table(root/'crates/sim/tests/reference/ra9_model_census.tsv')
assert [x['path'] for x in census]==[str(x.relative_to(root)) for x in corepaths]
for row in census:
 raw=(root/row['path']).read_bytes(); assert row['sha256']==sha(raw) and row['fnv1a64']==fnv(raw) and row['bytes']==str(len(raw))
ref=root/'crates/sim/tests/reference'; obs=table(ref/'ra9_scope_observation.tsv'); assert len(obs)==1 and obs[0]==packet['observed_scope']
model=''.join(x['path']+'\t'+x['sha256']+'\n' for x in census).encode(); assert sha(model)==obs[0]['model_census_sha256']
for key,name in [('model_surface_sha256','ra9_model_surface.txt'),('original_closure_sha256','ra9_scope75_original.tsv'),('special_original_sha256','ra9_special_original.tsv')]:assert sha((ref/name).read_bytes())==obs[0][key]
original=Path.home()/'Library/Caches/MyGame001/ra2-reference/rules.ini'; assert sha(original.read_bytes())==obs[0]['original_sha256']==old['observed_scope']['original_sha256']
branch=root/'crates/sim/tests/ra9_audit/legacy904-branch-files.tsv'; physical=table(branch)
for row in physical:
 raw=(root/row['repository_path']).read_bytes(); assert (row['bytes'],row['fnv1a64'],row['sampled_sha256'])==(str(len(raw)),fnv(raw),sha(raw))
for row in packet['preserved_producer_preimages']:
 pre=root/row['preimage']; assert sha(pre.read_bytes())==row['sha256'] and len(pre.read_bytes())==row['bytes']
oldbranch=table(p.parent/'preimages/crates/sim/tests/ra9_audit/legacy904-branch-files.tsv'); assert len(oldbranch)==len(physical)
for a,z in zip(oldbranch,physical,strict=True):assert a['repository_path']==z['repository_path']
assert packet['physical_branch_changes']==[{'path':z['repository_path'],'before':a,'after':z} for a,z in zip(oldbranch,physical) if a!=z]
local=root/'crates/sim/tests/ra9_audit/completion-local-code.tsv';current_local=table(local)
old_local=table(p.parent/'preimages/crates/sim/tests/ra9_audit/completion-local-code.tsv')
assert len(old_local)==len(current_local)
for a,z in zip(old_local,current_local,strict=True):
 assert a['repository_path']==z['repository_path']
 raw=(root/z['repository_path']).read_bytes(); assert (z['bytes'],z['fnv1a64'],z['sha256'])==(str(len(raw)),fnv(raw),sha(raw))
assert packet['completion_local_code_changes']==[{'path':z['repository_path'],'before':a,'after':z} for a,z in zip(old_local,current_local) if a!=z]
assert packet['producer']['exit']==0 and packet['producer']['command']==['python3','scripts/ra2_reference.py','--ra9-scope']
assert packet['producer']['raw']==record(root/packet['producer']['raw']['path'])
gap=(root/'docs/RA2_GAP_ANALYSIS.md').read_bytes(); start=gap.index(b'## How to perform user verification'); end=gap.index(b'## Historical baseline and source provenance'); assert packet['howtosee']==old['howtosee'] and sha(gap[start:end])==old['howtosee']['sha256']
for item in packet['historical_prompts']:
 path=root/item['path']; assert item==record(path); assert path.read_bytes()==subprocess.check_output(['git','show','8f1b1dc:'+item['path']],cwd=root)
assert packet['head']==subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
ID='ROOT_REBIND_R212_'+packet['phase'].upper().replace('-','_')+'_FACTUAL_'+datetime.datetime.now(datetime.timezone.utc).strftime('%Y_%m_%d_%H%M%S')
result={'status':'ROOT_INDEPENDENT_FACTUAL_VERIFICATION_PASS_NOT_POLICY_OR_NATIVE_APPROVAL','utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'root_factual_id':ID,'packet':record(p),'source_count':len(actual),'core_count':len(corepaths),'derived_count':len(derived),'authority_count':len(authorities),'source_added':[x['path'] for x in packet['source_added_from195']],'source_changed_count':len(packet['source_changed_from195']),'verified_scope':obs[0],'physical_branch_rows':len(physical),'physical_branch_changes':len(packet['physical_branch_changes']),'howtosee_sha256':old['howtosee']['sha256'],'limits':'Authenticates actual bytes, discovered source scope, producer observations, protected spans and factual pointers only. Does not authorize model-policy changes or infer native pass/review.'}
out=p.parent/'root-independent-factual-verification.json'; assert not out.exists(); out.write_text(json.dumps(result,indent=2)+'\n'); print(json.dumps(result,indent=2))
