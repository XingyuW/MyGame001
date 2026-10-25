from pathlib import Path
import json,hashlib,datetime
r=Path.cwd();v=r/'docs/validation/ra2-parity/ra9';sha=lambda b:hashlib.sha256(b).hexdigest();packet=v/'195-actual-canonical-input-scope.json';binding=sha(packet.read_bytes());assert binding=='9c110cf4d961ac33b5025acb53d0f50eedfa3fe55fe76e9e2135789d388ba6b7';auth=v/'195-root-independent-verification.json';assert sha(auth.read_bytes())=='a9dd802cb25ff332a6f8d3a9f49a34a593059f1c92f9b3b8218e6427d0bfcbc0';scope=json.loads(packet.read_text());model=scope['observed_scope']['model_census_sha256'];oldroot=(r/'crates/sim/tests/ra9_audit/root-policy-input-binding.txt').read_text().strip();oldmodel=(r/'crates/sim/tests/ra9_audit/ra9-approved-model-census-sha256.txt').read_text().strip();paths=[x['path'] for x in json.loads((v/'evidence/167-test-only-binding-refresh.json').read_text())['files']]+['crates/sim/tests/ra9_audit/ra9-approved-model-census-sha256.txt'];records=[]
for path in paths:
 f=r/path;before=f.read_bytes();pre=v/('196-before-'+path.replace('/','__'));assert not pre.exists();pre.write_bytes(before);s=before.decode();changes=[]
 if path.endswith('root-policy-input-binding.txt'):after=binding+'\n';changes=['actual root input SHA256']
 elif path.endswith('ra9-approved-model-census-sha256.txt'):after=model+'\n';changes=['actual observed model census SHA256']
 else:
  lines=s.splitlines();header=lines[0].split('\t');new=[lines[0]]
  for line in lines[1:]:
   cells=line.split('\t');assert len(cells)==len(header)
   at=header.index('final_input_sha256');assert cells[at]==oldroot;cells[at]=binding
   if 'consumer_bindings' in header:
    at=header.index('consumer_bindings');cells[at]=cells[at].replace('model-census='+oldmodel,'model-census='+model)
   if path.endswith('ra9_scope_approval.tsv'):
    for k,x in scope['observed_scope'].items():cells[header.index(k)]=x
    cells[header.index('recorded_scope_review')]='ROOT_CANONICAL_195_FACTUAL_2026_10_21'
   new.append('\t'.join(cells))
  after='\n'.join(new)+'\n';changes=['final_input_sha256']+(['consumer_bindings model-census factual pointer only'] if 'consumer_bindings' in header else [])+(['observed scope SHA fields and explicit root factual verification id'] if path.endswith('ra9_scope_approval.tsv') else [])
  # Every other cell, including exact literals/reasons/routes/qualified native receipts, remains byte-exact.
  for oldline,newline in zip(lines[1:],new[1:]):
   a=oldline.split('\t');b=newline.split('\t')
   for i,key in enumerate(header):
    if key not in ['final_input_sha256','consumer_bindings'] and not(path.endswith('ra9_scope_approval.tsv') and key in [*scope['observed_scope'],'recorded_scope_review']):assert a[i]==b[i],(path,key)
 f.write_text(after);records.append({'path':path,'before_sha256':sha(before),'after_sha256':sha(f.read_bytes()),'preimage':str(pre.relative_to(r)),'changed_fields':changes})
for x in scope['source_files']:assert sha((r/x['path']).read_bytes())==x['sha256'],x['path']
for a in scope['authority_fragments']:
 b=(r/a['document_path']).read_bytes();at=b.index(a['section_start'].encode());assert sha(b[at:at+a['span_bytes']])==a['sha256']
out={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'root_authorization':'ROOT_CANONICAL_195_FACTUAL_2026_10_21','root_independent_verification':{'path':str(auth.relative_to(r)),'sha256':sha(auth.read_bytes())},'input_scope_sha256':binding,'observed_model_census_sha256':model,'status':'EXCLUDED_FACTUAL_HASH_ID_OUTPUTS_ONLY_NOT_POLICY_VALIDATION_OR_REVIEW_APPROVAL','files':records,'source656_drift':[],'authority64_drift':[],'preserved':'Exact literal/class/reason/route/decision/qualifiedreceipt cells, priorindividualrecords/preimages/history; only current factual pointer cells refreshed.'}
(v/'196-actual-root-authorized-binding-refresh.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps({'files':len(records),'source656_drift':[],'authority64_drift':[],'receipt_sha256':sha((v/'196-actual-root-authorized-binding-refresh.json').read_bytes())},indent=2))
