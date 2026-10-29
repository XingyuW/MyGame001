from pathlib import Path
import csv,hashlib,io,json,re,sys
root=Path('/Users/xingyuwang/GameProjects/MyGame001')
p=Path(sys.argv[1]).resolve();packet=json.loads(p.read_bytes());q=p.parent/'binding-propagation/propagation.json';report=json.loads(q.read_bytes())
sha=lambda b:hashlib.sha256(b).hexdigest()
assert report['status']=='FACTUAL_POINTERS_APPLIED' and report['input_scope_sha256']==sha(p.read_bytes())
assert all(sha((root/x['path']).read_bytes())==x['sha256'] for x in packet['source_files'])
actual_deltas=[]
for r in report['files']:
 before=Path(r['preimage']).read_bytes();after=(root/r['path']).read_bytes()
 assert sha(before)==r['before_sha256'] and sha(after)==r['after_sha256']
 if r['path'].endswith('.tsv'):
  a=list(csv.DictReader(io.StringIO(before.decode()),delimiter='\t'));z=list(csv.DictReader(io.StringIO(after.decode()),delimiter='\t'));assert len(a)==len(z)
  for n,(old,new) in enumerate(zip(a,z,strict=True),2):
   assert old.keys()==new.keys()
   for k in old:
    if old[k]==new[k]:continue
    actual_deltas.append({'path':r['path'],'row':n,'column':k,'before':old[k],'after':new[k]})
    if k=='final_input_sha256':assert new[k]==sha(p.read_bytes())
    elif k=='consumer_bindings':
     def normalized(s):return re.sub(r'(^|;)(model-census|branch-fnv|code)=[^;]*',lambda m:m[1]+m[2]+'=FACTUAL',s)
     assert normalized(old[k])==normalized(new[k]),(r['path'],n,k)
    elif r['path'].endswith('/ra9_scope_approval.tsv'):
     if k=='recorded_scope_review':assert new[k]==report['root_factual_id']
     else:assert k in packet['observed_scope'] and new[k]==packet['observed_scope'][k]
    else:raise AssertionError((r['path'],n,k))
 elif r['path'].endswith('/root-policy-input-binding.txt'):assert after==(sha(p.read_bytes())+'\n').encode()
 elif r['path'].endswith('/ra9-approved-model-census-sha256.txt'):assert after==(packet['observed_scope']['model_census_sha256']+'\n').encode()
 # R219: the approved surface may only have become the observed one, with PlaceUnit added.
 elif r['path'].endswith('/ra9-approved-model-surface.txt'):assert after==(root/'crates/sim/tests/reference/ra9_model_surface.txt').read_bytes() and sha(after)==packet['observed_scope']['model_surface_sha256'] and after==before.replace(b'PlaceBuilding,SellBuilding',b'PlaceBuilding,PlaceUnit,SellBuilding')
 else:assert before==after
assert actual_deltas==report['cell_deltas']
assert len(report['files'])==15
for a in packet['authority_fragments']:
 raw=(root/a['document_path']).read_bytes();at=raw.index(a['section_start'].encode());assert sha(raw[at:at+a['span_bytes']])==a['sha256']
result={'status':'ROOT_POSTPROPAGATION_SOURCE_AND_NONFACTUAL_CELL_READBACK_PASS','packet_sha256':sha(p.read_bytes()),'propagation_sha256':sha(q.read_bytes()),'source_files':len(packet['source_files']),'derived_outputs':15,'authority_spans':64,'actual_cell_deltas':len(actual_deltas),'unchanged_policy_fields':'Every class/value/reason/guard/recorded policy decision/typed qualified receipt byte-equal; consumer token order/text exact except named factual tokens.'}
out=p.parent/'root-postpropagation-readback.json';assert not out.exists();out.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
