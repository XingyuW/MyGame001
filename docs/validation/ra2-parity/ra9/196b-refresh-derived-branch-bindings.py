from pathlib import Path
import csv,json,hashlib,datetime,collections
r=Path.cwd();v=r/'docs/validation/ra2-parity/ra9';p=r/'crates/sim/tests/ra9_audit';h=lambda b:hashlib.sha256(b).hexdigest()
def fnv(b):
 x=0xcbf29ce484222325
 for a in b:x=((x^a)*0x100000001b3)&((1<<64)-1)
 return f'{x:016x}'
pins=list(csv.DictReader((p/'legacy904-branch-files.tsv').open(),delimiter='\t'));oldpins=list(csv.DictReader((v/'194-before-legacy904-branch-files.tsv').open(),delimiter='\t'));families={x['family_id']:x for x in csv.DictReader((p/'legacy904-family-contracts.tsv').open(),delimiter='\t')}
for x in pins:
 b=(r/x['repository_path']).read_bytes();assert len(b)==int(x['bytes']) and fnv(b)==x['fnv1a64'] and h(b)==x['sampled_sha256']
f=p/'source-policy-acceptance.tsv';before=f.read_bytes();pre=v/'196b-before-source-policy-acceptance.tsv';assert not pre.exists();pre.write_bytes(before);lines=before.decode().splitlines();hd=lines[0].split('\t');col=hd.index('consumer_bindings');new=[lines[0]];changes=[];counts=collections.Counter()
def code_hash(table,want):return fnv(';'.join(f"{x['repository_path']}:{x['bytes']}:{x['fnv1a64']}" for x in table if x['repository_path'] in want).encode())
for line in lines[1:]:
 row=line.split('\t');tokens=dict(t.split('=',1) for t in row[col].split(';') if '=' in t)
 if 'branch-fnv' in tokens:
  family=families[tokens['family']];want=set(family['actual_paths'].split(';'));old=code_hash(oldpins,want);actual=code_hash(pins,want);assert tokens['branch-fnv']==old,(tokens['family'],'preimage branch does not match actual194before table')
  if old!=actual:
   oldcell=row[col];row[col]=oldcell.replace('branch-fnv='+old,'branch-fnv='+actual);assert row[col]!=oldcell;counts[family['family_id']]+=1;changes.append({'source_key':row[:3],'family':family['family_id'],'profile':family['profile'],'old_branch_fnv':old,'actual_branch_fnv':actual,'actual_paths':sorted(want)})
 new.append('\t'.join(row))
assert len(changes)==649 and len(counts)==103
for old,newline in zip(lines[1:],new[1:]):
 a=old.split('\t');b=newline.split('\t');assert all(x==y for i,(x,y) in enumerate(zip(a,b)) if i!=col)
after=('\n'.join(new)+'\n').encode();f.write_bytes(after)
scope=json.loads((v/'195-actual-canonical-input-scope.json').read_text())
for x in scope['source_files']:assert h((r/x['path']).read_bytes())==x['sha256']
rec={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'status':'DERIVED_FACTUAL_BRANCH_HASH_PROPAGATION_ONLY_NO_POLICY_OR_NATIVE_CREDIT','root_authorization_context':'ROOT_CANONICAL_195_FACTUAL_2026_10_21; necessary propagation of already verified actualai/combat metadata into existing excluded consumer digest fields. No source/input-scope change.','diagnosis':'197bwholepositive101 rejected exact consumer identity:194 actual source rows changed but196 had propagated only model-census/root/id, leaving per-family branch-fnv old. Everyold token now proved exact194preimage derivation; everynew token derives actual per-family path:bytes:FNV join in existing legacy904_support::current_code/legacy_*::live code. No hardcoded passing observations.','failed_raw':'197b-whole-positive.log','failed_raw_sha256':h((v/'197b-whole-positive.log').read_bytes()),'files':[{'path':str(f.relative_to(r)),'before_sha256':h(before),'after_sha256':h(after),'preimage':str(pre.relative_to(r)),'changed_column':'consumer_bindings','changed_token':'branch-fnv'}],'changed_rows':len(changes),'changed_families':len(counts),'family_counts':dict(counts),'exact_changes':changes,'preserved':'Every other token/column, exact literal/class/reason/route/guard/decision/qualifiedreceipt cell remains byte-exact. All656source/64authority unchanged. Original196 and197bfailed inputs/results retained.'}
(v/'196b-derived-branch-binding-refresh.json').write_text(json.dumps(rec,indent=2)+'\n');print(json.dumps({'rows':len(changes),'families':len(counts),'after_sha256':h(after),'receipt_sha256':h((v/'196b-derived-branch-binding-refresh.json').read_bytes())},indent=2))
