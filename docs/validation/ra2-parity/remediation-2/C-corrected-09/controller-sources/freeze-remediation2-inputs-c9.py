"""Freeze actual corrected inputs; no acceptance or policy approval is inferred."""
from pathlib import Path
import datetime,hashlib,json,subprocess,sys
R=Path('/Users/xingyuwang/GameProjects/MyGame001')
P=Path(sys.argv[1]).resolve(); D=json.loads(P.read_bytes()); S=Path(__file__).parent
sha=lambda b:hashlib.sha256(b).hexdigest()
prop_path=P.parent/'binding-propagation/propagation.json';prop=json.loads(prop_path.read_bytes())
expected={x['path']:x['sha256'] for x in D['source_files']}
assert len(expected)==D['source_count']
assert len(prop['files'])==15
for x in prop['files']:
 assert x['path'] not in expected
 expected[x['path']]=x['after_sha256']
paths=set(subprocess.check_output(['git','-C',str(R),'ls-files','--cached','--others','--exclude-standard','--','crates','assets','scripts','.cargo','Cargo.toml','Cargo.lock','rust-toolchain.toml','.gitignore','PLAN.md','docs/RA2_PARITY_PLAN.md','docs/prompts/RA2_PARITY_CONTINUATION_PROMPT.md'],text=True).splitlines())
assert paths==set(expected),(sorted(paths-set(expected)),sorted(set(expected)-paths))
assert all(sha((R/n).read_bytes())==h for n,h in expected.items())
assert len(D['authority_fragments'])==64
for x in D['authority_fragments']:
 b=(R/x['document_path']).read_bytes();at=b.index(x['section_start'].encode());assert sha(b[at:at+x['span_bytes']])==x['sha256']
G=(R/'docs/RA2_GAP_ANALYSIS.md').read_bytes();a=G.index(b'## How to perform user verification');b=G.index(b'## Historical baseline and source provenance');assert sha(G[a:b])==D['howtosee']['sha256']
for x in D['historical_prompts']:assert sha((R/x['path']).read_bytes())==x['sha256']
freeze={'status':'EXACT_CORRECTED_INPUT_FREEZE_NOT_REVIEW_OR_NATIVE_PASS','utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'input_packet_sha256':sha(P.read_bytes()),'propagation_sha256':sha(prop_path.read_bytes()),'source_files':[{'path':n,'bytes':(R/n).stat().st_size,'sha256':h} for n,h in sorted(expected.items())],'source_count':D['source_count'],'derived_count':15,'authority_fragments':D['authority_fragments'],'howtosee':D['howtosee'],'limits':'Every actual source and15 derived inputs frozen;64 protected spans and51 user checks retained. R194 one shared full13; R199 all five independent slices inspect revised full cumulative candidate before fresh lead. No acceptance inferred.'}
p=P.parent/'frozen-validation-inputs.json';assert not p.exists();p.write_text(json.dumps(freeze,indent=2)+'\n')
patterns={'sim_source':'crates/sim/src/**/*.rs','game_source':'crates/game/src/**/*.rs','audit_rust':'crates/sim/tests/ra9_audit/*.rs','audit_tsv':'crates/sim/tests/ra9_audit/*.tsv','audit_txt':'crates/sim/tests/ra9_audit/*.txt','ra9_top_level_sim_targets':'crates/sim/tests/ra9_*.rs'}
groups={k:[{'path':str(f.relative_to(R)),'sha256':sha(f.read_bytes())} for f in sorted(R.glob(pattern))] for k,pattern in patterns.items()}
old=json.loads((S/'round2-current-module-scope-v2.json').read_bytes())
for k,rows in old['groups'].items():assert {x['path'] for x in rows}<={x['path'] for x in groups[k]},k
catalog={'status':'ACTUAL_REVISED_FILE_CENSUS_NOT_COMPILED_OR_EXECUTED_COUNTS','packet_sha256':sha(P.read_bytes()),'frozen_manifest_sha256':sha(p.read_bytes()),'groups':groups}
c=S/'round2-c9-module-scope.json';assert not c.exists();c.write_text(json.dumps(catalog,indent=2)+'\n')
own=json.loads((S/'round2-current-production-ownership.json').read_bytes())
for x in groups['sim_source']:
 n=x['path']
 if n not in own['sim_source']:
  assert n.startswith('crates/sim/src/ai/'),n
  own['sim_source'][n]={'primary':'sim2','cross_checks':['tests','client for honest-navigation interface']}
 own['sim_source'][n]['sha256']=x['sha256']
assert set(own['sim_source'])=={x['path'] for x in groups['sim_source']}
own['game_source']['inventory']=str(c)+' game_source';own['audit_and_reference']['inventory']=str(c)+' audit groups'
o=S/'round2-c9-production-ownership.json';assert not o.exists();o.write_text(json.dumps(own,indent=2)+'\n')
print(json.dumps({'manifest':str(p),'sha256':sha(p.read_bytes()),'source_count':len(expected),'catalog':str(c),'counts':{k:len(v) for k,v in groups.items()},'ownership':str(o)}))
