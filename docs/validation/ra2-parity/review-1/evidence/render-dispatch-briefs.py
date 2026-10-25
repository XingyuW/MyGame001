"""Render the preserved canonical panel briefs at an exact frozen TIP; preparation only until invoked."""
from pathlib import Path
import datetime,hashlib,json,re,subprocess,sys
ROOT=Path('/Users/xingyuwang/GameProjects/MyGame001')
BASE=Path(__file__).resolve().parent
TIP=sys.argv[1];ROUND=int(sys.argv[2])
assert re.fullmatch(r'[0-9a-f]{40}',TIP) and ROUND>=1
assert subprocess.check_output(['git','-C',str(ROOT),'rev-parse','HEAD'],text=True).strip()==TIP
assert subprocess.run(['git','-C',str(ROOT),'diff','--quiet','HEAD']).returncode==0
sha=lambda b:hashlib.sha256(b).hexdigest()
common=BASE/'common-review-instructions.md'
record={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'tip':TIP,'round':ROUND,'base':'8230a955b251e9341d349f99eaeb86f7f8253664','common_instructions':{'path':str(common),'sha256':sha(common.read_bytes())},'rendered':[]}
meta=BASE/f'dispatch-round-{ROUND}.json';assert not meta.exists()
for role in ['data','tests','sim2','sim1','client','lead']:
 source=ROOT/'docs/prompts/ra2-parity-review'/('lead.md' if role=='lead' else f'slice-{role}.md')
 b=source.read_bytes();text=b.decode()
 text=re.sub(r'\bTIP\b',TIP,text);text=re.sub(r'\bROUND\b',str(ROUND),text);text=text.replace('<SCRATCH>',str(BASE))
 prefix=f'# Actual independent dispatch binding: round {ROUND}, {role}\n\nExact frozen TIP: `{TIP}`. Cumulative base: `8230a955b251e9341d349f99eaeb86f7f8253664`. Read `{common}` first; its current storage, native-lease and independence instructions override historical command examples. The preserved current brief follows with its placeholders filled. This header binds the actual dispatch; historical candidate-freeze pending prose is not the current panel status.\n\n'
 out=BASE/f'dispatch-round-{ROUND}-{role}.md';assert not out.exists();out.write_text(prefix+text)
 record['rendered'].append({'role':role,'canonical_brief':str(source.relative_to(ROOT)),'canonical_bytes':len(b),'canonical_sha256':sha(b),'rendered_path':str(out),'rendered_bytes':out.stat().st_size,'rendered_sha256':sha(out.read_bytes())})
meta.write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps({'tip':TIP,'round':ROUND,'rendered_briefs':len(record['rendered']),'metadata':str(meta)}))
