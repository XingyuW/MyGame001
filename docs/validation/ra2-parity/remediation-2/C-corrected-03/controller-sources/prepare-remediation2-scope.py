"""Prepare actual source observations; NEVER authorize or propagate a root binding."""
from pathlib import Path
import csv,datetime,hashlib,json,os,re,subprocess,sys
ROOT=Path('/Users/xingyuwang/GameProjects/MyGame001');os.chdir(ROOT)
PHASE=sys.argv[1];assert re.fullmatch(r'[ABC]-[a-z0-9-]+',PHASE)
OUT=ROOT/'docs/validation/ra2-parity/remediation-2'/PHASE;assert not OUT.exists();OUT.mkdir(parents=True)
BASE=ROOT/'docs/validation/ra2-parity/ra9';old=json.loads((BASE/'195-actual-canonical-input-scope.json').read_text())
sha=lambda b:hashlib.sha256(b).hexdigest()
def utc():return datetime.datetime.now(datetime.timezone.utc).isoformat()
def fnv(b):
    n=0xcbf29ce484222325
    for x in b:n=((n^x)*0x100000001b3)&((1<<64)-1)
    return f'{n:016x}'
def file_record(path):
    b=path.read_bytes();return {'path':str(path.relative_to(ROOT)),'bytes':len(b),'sha256':sha(b)}
old_source={x['path']:x for x in old['source_files']};excluded={x['path']:x for x in old['excluded_files']}
roots=['crates','assets','scripts','.cargo','Cargo.toml','Cargo.lock','rust-toolchain.toml','.gitignore']
def candidates():
    output=subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard','--',*roots],text=True)
    return sorted(set(output.splitlines())|set(old_source))
initial={x:file_record(ROOT/x) for x in candidates()}
# The producer and physical branch observations are the only permitted mutation in this script.
producer_files=[ROOT/'crates/sim/tests/reference'/n for n in json.loads((BASE/'194-actual-canonical-scope-producer.json').read_text())['outputs']]
branch=ROOT/'crates/sim/tests/ra9_audit/legacy904-branch-files.tsv'
local=ROOT/'crates/sim/tests/ra9_audit/completion-local-code.tsv'
preserved=[]
for f in [*producer_files,branch,local]:
    dst=OUT/'preimages'/f.relative_to(ROOT);dst.parent.mkdir(parents=True,exist_ok=True);dst.write_bytes(f.read_bytes());preserved.append({'source':str(f.relative_to(ROOT)),'preimage':str(dst.relative_to(ROOT)),**file_record(f)})
cmd=['python3','scripts/ra2_reference.py','--ra9-scope'];env=os.environ.copy();env['PYTHONDONTWRITEBYTECODE']='1';start=utc()
raw=OUT/'actual-source-producer.log'
with raw.open('xb') as out: code=subprocess.call(cmd,env=env,stdout=out,stderr=subprocess.STDOUT)
producer={'command':cmd,'start_utc':start,'end_utc':utc(),'exit':code,'raw':file_record(raw)}
assert code==0,producer
def refresh_physical(table, digest_name):
    lines=table.read_text().splitlines();header=lines[0].split('\t');names={n:i for i,n in enumerate(header)};rows=[];changes=[]
    for line in lines[1:]:
        fields=line.split('\t');assert len(fields)==len(header)
        actual=(ROOT/fields[names['repository_path']]).read_bytes();next_fields=fields.copy()
        for name,value in [('bytes',str(len(actual))),('fnv1a64',fnv(actual)),(digest_name,sha(actual))]:next_fields[names[name]]=value
        if fields!=next_fields:changes.append({'path':fields[names['repository_path']],'before':dict(zip(header,fields)),'after':dict(zip(header,next_fields))})
        assert all(a==b for i,(a,b) in enumerate(zip(fields,next_fields)) if i not in [names['bytes'],names['fnv1a64'],names[digest_name]])
        rows.append('\t'.join(next_fields))
    table.write_text('\n'.join([lines[0],*rows])+'\n')
    return changes
changes=refresh_physical(branch,'sampled_sha256')
local_changes=refresh_physical(local,'sha256')
now=candidates();allowed={str(x.relative_to(ROOT)) for x in [*producer_files,branch,local]}
assert set(now)==set(initial),'Concurrent file-set mutation during producer'
assert all(file_record(ROOT/x)==initial[x] for x in now if x not in allowed),'Concurrent input mutation during producer'
sources=[file_record(ROOT/x) for x in now if x not in excluded]
assert set(old_source)<=set(x['path'] for x in sources),'An existing input disappeared'
# All self-referential acceptance/root outputs stay outside the packet, including completion174.
# Read every excluded input under the code/assets/script candidate roots; final frozen manifest will guard all of them.
derived=[file_record(ROOT/x) for x in now if x in excluded and ('Approval/' in excluded[x]['reason'] or 'Validation-derived per-atom' in excluded[x]['reason'])]
other_exclusions=[excluded[x] for x in now if x in excluded and x not in {r['path'] for r in derived}]
authority=[]
for a in old['authority_fragments']:
    a=a.copy();b=(ROOT/a['document_path']).read_bytes();at=b.index(a['section_start'].encode());assert sha(b[at:at+a['span_bytes']])==a['sha256'];a['actual_start_byte']=at;authority.append(a)
assert len(authority)==64
core=[file_record(x) for x in sorted(ROOT.glob('crates/*/src/**/*.rs'))]
model=list(csv.DictReader((ROOT/'crates/sim/tests/reference/ra9_model_census.tsv').open(),delimiter='\t'))
assert {x['path'] for x in core}=={x['path'] for x in model}
for x in model:
    b=(ROOT/x['path']).read_bytes();assert sha(b)==x['sha256'] and fnv(b)==x['fnv1a64'] and len(b)==int(x['bytes'])
gap=(ROOT/'docs/RA2_GAP_ANALYSIS.md').read_bytes();at=gap.index(b'## How to perform user verification');end=gap.index(b'## Historical baseline and source provenance');assert sha(gap[at:end])==old['howtosee']['sha256']
prompts=[]
for name in ['docs/prompts/RA2_PARITY_PROMPT.md','docs/prompts/RA2_PARITY_CONTINUATION_PROMPT.md']:
    b=(ROOT/name).read_bytes();assert b==subprocess.check_output(['git','show','8f1b1dc:'+name]);prompts.append(file_record(ROOT/name))
observation=list(csv.DictReader((ROOT/'crates/sim/tests/reference/ra9_scope_observation.tsv').open(),delimiter='\t'));assert len(observation)==1
record={'utc':utc(),'status':'ACTUAL_FACTUAL_PACKET_PENDING_INDEPENDENT_ROOT_VERIFICATION_NOT_POLICY_OR_NATIVE_APPROVAL','phase':PHASE,'head':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'base_packet':file_record(BASE/'195-actual-canonical-input-scope.json'),'source_files':sources,'source_count':len(sources),'authority_fragments':authority,'core_files':core,'core_count':len(core),'derived_outputs_before_binding':derived,'other_candidate_exclusions':other_exclusions,'all_candidates_count':len(now),'unaccounted_candidates':[],'source_added_from195':[x for x in sources if x['path'] not in old_source],'source_deleted_from195':[],'source_changed_from195':[{'before':old_source[x['path']],'after':x} for x in sources if x['path'] in old_source and x!=old_source[x['path']]],'observed_scope':observation[0],'producer':producer,'physical_branch_changes':changes,'completion_local_code_changes':local_changes,'preserved_producer_preimages':preserved,'historical_prompts':prompts,'howtosee':old['howtosee'],'authored_choices':old['authored_choices'],'limits':'Actual observations and byte scope only; no class/reason/route/decision/qualified receipt changed or policy accepted. Root must independently verify before any excluded factual pointer propagation. Old195/196/196b remain immutable.'}
p=OUT/'actual-input-scope.json';p.write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps({'packet':str(p),'sha256':sha(p.read_bytes()),'source_files':len(sources),'core_files':len(core),'derived_outputs':len(derived),'authority':len(authority),'added':[x['path'] for x in record['source_added_from195']],'physical_branch_changed':len(changes),'observed_scope':record['observed_scope']}))
