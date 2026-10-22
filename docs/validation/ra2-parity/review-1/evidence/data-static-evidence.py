from pathlib import Path
import csv,json,hashlib,re,collections,subprocess,ast
ROOT=Path('/Users/xingyuwang/GameProjects/MyGame001'); OUT=Path(__file__).parent; V=ROOT/'docs/validation/ra2-parity/ra9'; AD=ROOT/'crates/sim/tests/ra9_audit'; RF=ROOT/'crates/sim/tests/reference'
h=lambda b:hashlib.sha256(b).hexdigest()
def rows(p):return list(csv.DictReader(p.open(),delimiter='\t'))
def fnv(b):
 x=0xcbf29ce484222325
 for n in b:x=((x^n)*0x100000001b3)&0xffffffffffffffff
 return f'{x:016x}'
result={};packet=json.loads((V/'195-actual-canonical-input-scope.json').read_text());errors=[]
for x in packet['source_files']+packet['core97']:
 p=ROOT/x['path'];b=p.read_bytes()
 if len(b)!=x['bytes'] or h(b)!=x['sha256']:errors.append(x['path'])
for x in packet['authority_fragments']:
 b=(ROOT/x['document_path']).read_bytes();a=b.index(x['section_start'].encode());s=b[a:a+x['span_bytes']]
 if h(s)!=x['sha256']:errors.append(x['section_start'])
result['scope']={'source_files':len(packet['source_files']),'core':len(packet['core97']),'authority_spans':len(packet['authority_fragments']),'errors':errors,'packet_sha256':h((V/'195-actual-canonical-input-scope.json').read_bytes()),'root_binding':(AD/'root-policy-input-binding.txt').read_text().strip()}
model=rows(RF/'ra9_model_census.tsv');actual={p.relative_to(ROOT).as_posix() for p in (ROOT/'crates').glob('*/src/**/*.rs') if p.is_file()};recorded={x['path'] for x in model};err=[]
for x in model:
 b=(ROOT/x['path']).read_bytes()
 if h(b)!=x['sha256'] or fnv(b)!=x['fnv1a64'] or str(len(b))!=x['bytes']:err.append(x['path'])
legacy=''.join(x['path']+'\t'+x['sha256']+'\n' for x in model)
obs=rows(RF/'ra9_scope_observation.tsv')[0]
result['model_census']={'count':len(model),'unrecorded':sorted(actual-recorded),'nonexistent':sorted(recorded-actual),'hash_errors':err,'legacy_digest_match':h(legacy.encode())==obs['model_census_sha256'],'surface_digest_match':h((RF/'ra9_model_surface.txt').read_bytes())==obs['model_surface_sha256'],'scope75_file_hash_match':h((RF/'ra9_scope75_original.tsv').read_bytes())==obs['original_closure_sha256'],'special_file_hash_match':h((RF/'ra9_special_original.tsv').read_bytes())==obs['special_original_sha256']}
# Actual include graph from integration entry; verify all 48 audit Rust files reachable.
root=ROOT/'crates/sim/tests/ra9_data_parity.rs';seen=set();includes=set();missing=[]
def walk(p):
 p=p.resolve()
 if p in seen:return
 seen.add(p);text=p.read_text()
 for n in re.findall(r'#\[path\s*=\s*"([^"]+)"\]\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*;',text):
  target=p.parent/n
  if not target.exists():missing.append(str(target))
  else:walk(target)
 for n in re.findall(r'include_str!\s*\(\s*"([^"]+)"\s*\)',text):
  target=(p.parent/n).resolve();includes.add(str(target))
  if not target.exists():missing.append(str(target))
 # ordinary mod ra9_audit; is the integration hook.
 for name in re.findall(r'^mod (\w+);',text,re.M):
  if name=='ra9_audit':walk(p.parent/name/'mod.rs')
walk(root)
allrs={p.resolve() for p in AD.glob('*.rs')};reachable=allrs & seen
result['graph']={'audit_rust':len(allrs),'reachable_audit_rust':len(reachable),'unreachable':[str(x) for x in sorted(allrs-seen)],'include_paths':len(includes),'missing':missing,'tsv':len(list(AD.glob('*.tsv'))),'txt':len(list(AD.glob('*.txt'))),'reference_files':len(list(RF.iterdir()))}
# Full source atom ledger: 19 explicitly split Powerups tuples; otherwise Whole.
reference=rows(RF/'ra2_reference.tsv');raw={(r['file'],r['section'],r['key']):r['value'] for r in reference};dis=rows(RF/'ra9_source_dispositions.tsv');atoms=set();bad=[]
for n,r in enumerate(dis,2):
 key=(r['file'],r['section'],r['key']);portion=r['portion'];a=key+(portion,)
 if a in atoms:bad.append([n,'duplicate'])
 atoms.add(a)
 value=raw.get(key)
 if portion!='whole':
  try:value=value.split(',')[int(portion)]
  except Exception:value=None
 if value!=r['exact_value']:bad.append([n,'value'])
 if not r['specific_semantic_reason'].strip() or not r['code_evidence'].strip():bad.append([n,'empty reason/evidence'])
 if r['classification'] not in ('CURRENT_MODEL_NONREAD','INTENTIONAL_DIFFERENCE'):bad.append([n,'unexpected class'])
result['dispositions']={'atoms':len(dis),'source_keys':len({a[:3] for a in atoms}),'whole':sum(r['portion']=='whole' for r in dis),'components':dict(collections.Counter(r['portion'] for r in dis if r['portion']!='whole')),'classes':dict(collections.Counter(r['classification'] for r in dis)),'routes':dict(collections.Counter(r['route'] for r in dis)),'errors':bad,'reference_rows':len(reference),'expected_atoms':len(reference)-19+sum(len(raw['rules','Powerups',k].split(',')) for k in {r['key'] for r in dis if r['section']=='Powerups' and r['portion']!='whole'})+17*2}
# Resolve every route hash to actual retained artifacts, then hash the underlying command receipts.
receipt_hashes=set()
for p in AD.glob('*acceptance.tsv'):
 for r in rows(p):
  if 'native_receipts' in r:
   receipt_hashes.update(t.rsplit('@',1)[-1] for t in r['native_receipts'].split(';') if '@' in t)
resolved={x:[] for x in receipt_hashes}
for p in V.rglob('*'):
 if p.is_file() and p.stat().st_size<=3000000:
  digest=h(p.read_bytes())
  if digest in resolved:resolved[digest].append(str(p.relative_to(ROOT)))
result['receipt_hashes']={'unique':len(receipt_hashes),'unresolved':[x for x,ps in resolved.items() if not ps],'resolved':resolved}
old=json.loads((V/'evidence/166-six-test-repairs-input-scope.json').read_text());checked=[];fail=[]
def locate(x):
 p=ROOT/x['path']
 if not p.exists() and x['path'].startswith('ra9-preliminary-logs/'):
  p=V/'evidence'/Path(x['path']).name
 if not p.exists() and x['path'].startswith('ra9-preliminary-logs/'):
  p=Path('/Users/xingyuwang/Library/Caches/Codex/reviews/MyGame001/ra9-integration-validation')/x['path']
 return p
for cmd in old['actual_command_evidence']:
 for field in ('receipt','log'):
  x=cmd[field];p=locate(x)
  if not p.exists():fail.append([x['path'],'missing']);continue
  b=p.read_bytes();checked.append(str(p.relative_to(ROOT)) if p.is_relative_to(ROOT) else str(p))
  if len(b)!=x['bytes'] or h(b)!=x['sha256']:fail.append([x['path'],'hash/size'])
result['historical_receipts']={'commands':len(old['actual_command_evidence']),'hashed_files':len(checked),'errors':fail,'qualification':'Historical scopes only; immutable bytes do not turn old native source into frozen-source execution.'}
# Current source-bound bounded results. A full target is deliberately not inferred.
rawresults=[]
for n in ('197c-whole-positive','197c-six-repaired','197c-difficulty'):
 text=(V/(n+'.log')).read_text();rawresults.append({'name':n,'sha256':h(text.encode()),'results':re.findall(r'test result:[^\n]*',text),'audit_success':re.findall(r'RA9_AUDIT_SUCCESS[^\n]*',text)})
result['bounded_results']=rawresults
# Every newly added asset is individually covered by a licence-table literal.
added=subprocess.check_output(['git','diff','--name-only','--diff-filter=A','8230a955b251e9341d349f99eaeb86f7f8253664','23647de4220408d42eeef6522deb4d4541528452','--','assets'],cwd=ROOT).decode().splitlines();lic=(ROOT/'assets/LICENSES.md').read_text();unlisted=[x for x in added if x.removeprefix('assets/') not in lic]
result['asset_inventory']={'added':len(added),'not_literal_listed':unlisted}
# All 183 debt rows: preserve stable IDs, aliases and claims; do not adopt author severity.
debt=rows(ROOT/'docs/RA2_HISTORICAL_DEBT.tsv');result['debt']={'rows':len(debt),'unique_ids':len({r['ledger_id'] for r in debt}),'empty_required':[(r['ledger_id'],k) for r in debt for k in ('remaining_obligation','current_source_assessment','source_refs') if not r[k]],'all_unaccepted':all(r['acceptance']=='NOT_ACCEPTED_BY_DOCUMENTATION' for r in debt)}
(OUT/'data-static-evidence-v2.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:({kk:vv for kk,vv in v.items() if kk!='resolved'} if isinstance(v,dict) else v) for k,v in result.items()},indent=2))
