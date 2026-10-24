import datetime,hashlib,json,os,subprocess,sys,time,traceback
from pathlib import Path
root=Path('/Users/xingyuwang/GameProjects/MyGame001');os.chdir(root)
planfile=Path(sys.argv[1]).resolve();plan=json.loads(planfile.read_bytes());out=root/plan['output'];assert not out.exists();out.mkdir(parents=True)
sha=lambda b:hashlib.sha256(b).hexdigest()
def utc():return datetime.datetime.now(datetime.timezone.utc).isoformat()
def scope():
 paths=subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard','--','crates','assets','scripts','.cargo','Cargo.toml','Cargo.lock'],text=True).splitlines()
 return {x:sha(Path(x).read_bytes()) for x in sorted(paths) if x not in plan.get('snapshot_exclude',[])}
initial=scope();(out/'source-before.json').write_text(json.dumps(initial,indent=2)+'\n')
changed=set(subprocess.check_output(['git','diff','--name-only','HEAD','--','crates','assets','scripts','.cargo','Cargo.toml','Cargo.lock'],text=True).splitlines())
tracked=set(subprocess.check_output(['git','ls-files','--','crates','assets','scripts','.cargo','Cargo.toml','Cargo.lock'],text=True).splitlines())
preimages=[]
for name in sorted(x for x in initial if x in changed or x not in tracked):
 raw=Path(name).read_bytes();assert sha(raw)==initial[name]
 dst=out/'source-preimages'/name;dst.parent.mkdir(parents=True,exist_ok=True);dst.write_bytes(raw)
 preimages.append({'source':name,'preimage':str(dst.relative_to(root)),'bytes':len(raw),'sha256':initial[name]})
(out/'source-preimages.json').write_text(json.dumps({'status':'EXACT_CHANGED_AND_UNTRACKED_GUARDED_INPUT_PREIMAGES','files':preimages,'tracked_unchanged_source':'Available in recorded Git predecessor'},indent=2)+'\n')
state={'status':'RUNNING','start':utc(),'pid':os.getpid(),'head':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'plan':str(planfile),'plan_sha256':sha(planfile.read_bytes()),'runner_sha256':sha(Path(__file__).read_bytes()),'snapshot_exclude':plan.get('snapshot_exclude',[]),'commands':[]}
receipt=out/'process.json'
def save():
 tmp=receipt.with_suffix('.next');tmp.write_text(json.dumps(state,indent=2)+'\n');tmp.replace(receipt)
save()
try:
 for index,command in enumerate(plan['commands'],1):
  assert scope()==initial,'source mutated before command'
  raw=out/f'{index:02d}-{command["name"]}.log';entry={'command':command['args'],'start':utc(),'raw':str(raw.relative_to(root))};state['commands'].append(entry);save()
  env=os.environ.copy();env['CARGO_BUILD_JOBS']='1'
  for k,v in plan.get('env',{}).items():env[k]=v
  with raw.open('xb') as log:
   child=subprocess.Popen(command['args'],env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
   entry['pid']=child.pid;entry['pgid']=os.getpgid(child.pid);save()
   while child.poll() is None:
    state['heartbeat']=utc();save();time.sleep(5)
   entry['exit']=child.wait()
  entry['end']=utc();entry['raw_bytes']=raw.stat().st_size;entry['raw_sha256']=sha(raw.read_bytes());entry['source_unchanged']=scope()==initial;save()
  assert entry['source_unchanged'],'source mutated during command'
  if entry['exit']!=0 and command.get('stop_on_nonzero',False):break
 state['status']='COMPLETED_WITH_ACTUAL_EXITS_NOT_AUTOMATIC_PASS'
except BaseException as error:
 state['status']='CONTROLLER_ERROR';state['error']=repr(error);state['traceback']=traceback.format_exc()
finally:
 state['end']=utc();state['source_unchanged']=scope()==initial;save();(out/'source-after.json').write_text(json.dumps(scope(),indent=2)+'\n')
