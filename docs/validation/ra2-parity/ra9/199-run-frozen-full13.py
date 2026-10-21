"""Run the unchanged mandatory validator with actual source/authority guards and receipts."""
from pathlib import Path
import datetime,hashlib,json,os,subprocess,time,sys,re
ROOT=Path(__file__).resolve().parents[4];OUT=Path(__file__).resolve().parent;os.chdir(ROOT)
sha=lambda b:hashlib.sha256(b).hexdigest()
packet=json.loads((OUT/'195-actual-canonical-input-scope.json').read_text())
files={x['path']:x['sha256'] for x in packet['source_files']}
for row in json.loads((OUT/'196-actual-root-authorized-binding-refresh.json').read_text())['files']:files[row['path']]=row['after_sha256']
for row in json.loads((OUT/'196b-derived-branch-binding-refresh.json').read_text())['files']:files[row['path']]=row['after_sha256']
def utc():return datetime.datetime.now(datetime.timezone.utc).isoformat()
def guard():
 bad=[p for p,h in files.items() if sha((ROOT/p).read_bytes())!=h]
 for a in packet['authority_fragments']:
  b=(ROOT/a['document_path']).read_bytes();at=b.index(a['section_start'].encode())
  if sha(b[at:at+a['span_bytes']])!=a['sha256']:bad.append(a['section_start'])
 assert not bad,bad
 return {'files':len(files),'authority_spans':64,'mismatches':bad}
path=OUT/'gate-process.json';assert not path.exists();assert not (OUT/'summary.txt').exists()
assert not subprocess.check_output(['git','status','--porcelain']), 'Commit final source/evidence before mandatory gate launch'
record={'start_utc':utc(),'pid':os.getpid(),'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'runner_sha256':sha(Path(__file__).read_bytes()),'status':'RUNNING','source_before':guard(),'commands':[],'performance_scope':'Sole canonical Cargo/native/process owner, no copy/reviewer native overlap; required workspace internal parallel collections retain original policy. Isolated198same-sourceproofs precede full13 under explicit coordinator ordering authorization.'}
def save():path.write_text(json.dumps(record,indent=2)+'\n')
env=os.environ.copy();assert env.get('CARGO_TARGET_DIR')=='/Users/xingyuwang/Library/Caches/Codex/cargo-target';env['CARGO_BUILD_JOBS']='1'
cache_lengths=Path(env['CARGO_TARGET_DIR'])/'tmp/m6_production_matches_lengths.tsv'
cache_before=None
if cache_lengths.is_file():
    b=cache_lengths.read_bytes();cache_before={'path':str(cache_lengths),'bytes':len(b),'sha256':sha(b),'mtime_ns':cache_lengths.stat().st_mtime_ns};(OUT/'199-before-cache-production-lengths.tsv').write_bytes(b)
record['production_cache_before']=cache_before
cmd=['env','-u','RA2_DUEL_SEED','-u','RA2_DUEL_PAIR','sh','scripts/validate_ra2_parity.sh','ra9'];record['command']=cmd;record['configured_environment']={k:env.get(k) for k in ['CARGO_TARGET_DIR','UV_CACHE_DIR','PIP_CACHE_DIR','TMPDIR','CARGO_BUILD_JOBS']};save();start=time.monotonic()
with (OUT/'199-full13-wrapper.log').open('wb') as out:
 process=subprocess.Popen(cmd,stdout=out,stderr=subprocess.STDOUT,env=env,start_new_session=True);record['child_pid']=process.pid;record['child_pgid']=process.pid;save()
 seen=0
 while True:
  lines=(OUT/'summary.txt').read_text().splitlines() if (OUT/'summary.txt').exists() else []
  for line in lines[seen:]:
   if re.match(r'^\d\d exit ',line):
    entry={'line':line,'observed_utc':utc(),'wrapper_elapsed_seconds':time.monotonic()-start,'source_after':guard()}
    if line.startswith('04 exit ') and cache_lengths.is_file():
     b=cache_lengths.read_bytes();(OUT/'production-lengths.tsv').write_bytes(b)
     entry['actual_production_length_table']={'cache_path':str(cache_lengths),'repository_path':str((OUT/'production-lengths.tsv').relative_to(ROOT)),'bytes':len(b),'sha256':sha(b),'cache_mtime_ns':cache_lengths.stat().st_mtime_ns,'mtime_changed_since_launch':cache_before is None or cache_lengths.stat().st_mtime_ns!=cache_before['mtime_ns']}
    record['commands'].append(entry);print(line,flush=True)
  seen=len(lines);save()
  if process.poll() is not None:
   final_lines=(OUT/'summary.txt').read_text().splitlines() if (OUT/'summary.txt').exists() else []
   if len(final_lines)>seen:continue  # Drain the actual last summary line after child exit.
   break
  time.sleep(1)
 code=process.wait()
record.update({'exit':code,'status':'PASSED' if code==0 else 'FAILED','end_utc':utc(),'seconds':time.monotonic()-start,'source_after':guard()})
record['raw_logs']=[{'path':str(p.relative_to(ROOT)),'bytes':p.stat().st_size,'sha256':sha(p.read_bytes())} for p in sorted(OUT.glob('[0-9][0-9]-*.log'))]
record['summary']={'path':str((OUT/'summary.txt').relative_to(ROOT)),'bytes':(OUT/'summary.txt').stat().st_size,'sha256':sha((OUT/'summary.txt').read_bytes())};save();print(record['status'],code,flush=True);sys.exit(code)
