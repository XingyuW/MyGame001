from pathlib import Path
import datetime,hashlib,json,os,re,signal,subprocess,sys,time
root=Path('/Users/xingyuwang/GameProjects/MyGame001');os.chdir(root)
packet_path=Path(sys.argv[1]).resolve();out=root/sys.argv[2];assert not out.exists();out.mkdir(parents=True)
packet=json.loads(packet_path.read_bytes());prop=json.loads((packet_path.parent/'binding-propagation/propagation.json').read_bytes());sha=lambda b:hashlib.sha256(b).hexdigest()
expected={x['path']:x['sha256'] for x in packet['source_files']};expected.update({x['path']:x['after_sha256'] for x in prop['files']});assert len(prop['files'])==15 and len(expected)==packet['source_count']+15
name='crates/game/tests/m6_production_matches.rs';target=root/name;original=target.read_bytes();(out/'original-m6_production_matches.rs').write_bytes(original)
positive_path=(root/sys.argv[3]).resolve();positive=positive_path.read_text();assert 'test result: ok. 1 passed; 0 failed;' in positive
pattern=r'RA9 late twins: winner=(.*?) outcome_tick=(\d+) steps=(\d+)/(\d+).*?late_counts=\[(\d+), (\d+), (\d+)\] final_hash=([a-f0-9]+) trajectory_hash=([a-f0-9]+) candidate=(.*)'
base=re.search(pattern,positive);assert base and all(int(base[i])>0 for i in [5,6,7])
def utc():return datetime.datetime.now(datetime.timezone.utc).isoformat()
def snapshot():
 result={n:sha((root/n).read_bytes()) for n in expected}
 for a in packet['authority_fragments']:
  raw=(root/a['document_path']).read_bytes();at=raw.index(a['section_start'].encode());assert sha(raw[at:at+a['span_bytes']])==a['sha256']
 return result
assert snapshot()==expected
state={'status':'RUNNING','start':utc(),'pid':os.getpid(),'runner_sha256':sha(Path(__file__).read_bytes()),'packet_sha256':sha(packet_path.read_bytes()),'positive_raw_sha256':sha(positive_path.read_bytes()),'original_target_sha256':sha(original),'scope':'Exact existing native36001 extraRNG and observer-only lateProductionComplete controls on corrected source; no cap, tuple, threshold, data or production mutation.','commands':[]};receipt=out/'process.json'
def save():
 tmp=receipt.with_suffix('.next');tmp.write_text(json.dumps(state,indent=2)+'\n');tmp.replace(receipt)
cmd=['cargo','test','--release','-p','game','--test','m6_production_matches','ra9_native_twins_remain_active_after_36000_and_reach_the_same_winner','--','--exact','--nocapture','--test-threads=1'];child=None
awake=subprocess.Popen(['caffeinate','-i','-w',str(os.getpid())],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);save()
try:
 for index,label in enumerate(['rng-causal','production-observer-causal','restored-positive'],1):
  assert snapshot()==expected
  text=original.decode()
  if label=='rng-causal':
   needle='        let hash = sim::state_hash(left.world_mut());';assert text.count(needle)==1;text=text.replace(needle,'        if native_tick == 36_001 {\n            let _ = right.world_mut().resource_mut::<sim::SimRng>().next_u32();\n        }\n'+needle)
  elif label=='production-observer-causal':
   needle='                    SimEvent::ProductionComplete { .. } => Some(1),';assert text.count(needle)==1;text=text.replace(needle,'                    SimEvent::ProductionComplete { .. } => None,')
  target.write_text(text);before=snapshot();raw=out/f'{index:02d}-{label}.log';(out/f'{index:02d}-{label}-target.rs').write_bytes(target.read_bytes());entry={'label':label,'command':cmd,'start':utc(),'expected_exit':0 if label=='restored-positive' else 101,'mutated_target_sha256':sha(target.read_bytes()),'raw':str(raw.relative_to(root))};state['commands'].append(entry);save()
  env=os.environ.copy();env['CARGO_BUILD_JOBS']='1'
  with raw.open('xb') as log:
   child=subprocess.Popen(cmd,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True);entry['pid']=child.pid;entry['pgid']=child.pid;save()
   while child.poll() is None:state['heartbeat']=utc();save();time.sleep(5)
   entry['exit']=child.wait();child=None
  assert snapshot()==before
  target.write_bytes(original);entry['restoration_immediate']={'utc':utc(),'sha256':sha(target.read_bytes())};time.sleep(.05);entry['restoration_separate_later_read']={'utc':utc(),'sha256':sha(target.read_bytes())};assert snapshot()==expected
  entry.update(end=utc(),raw_bytes=raw.stat().st_size,raw_sha256=sha(raw.read_bytes()));t=raw.read_text();assert entry['exit']==entry['expected_exit']
  if label=='rng-causal':assert 'state hash at native tick 36001, completed step 36002' in t and 'test result: FAILED. 0 passed; 1 failed;' in t
  else:
   m=re.search(pattern,t);assert m
   for i in [1,2,3,4,5,7,8,9,10]:assert m[i]==base[i],(label,i,m[i],base[i])
   if label=='production-observer-causal':assert m[6]=='0' and 'missing late ProductionComplete after native tick36000' in t
   else:assert m[6]==base[6] and 'test result: ok. 1 passed; 0 failed;' in t
  entry['expected_result_verified']=True;save()
 state['status']='PASSED_EXPECTED_CAUSAL_FAILURES_AND_RESTORED_POSITIVE'
except BaseException as error:state['status']='FAILED';state['error']=repr(error)
finally:
 if child is not None and child.poll() is None:
  os.killpg(child.pid,signal.SIGTERM)
  try:child.wait(timeout=10)
  except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait()
 target.write_bytes(original);state['source_restored_exact']=snapshot()==expected;state['end']=utc();awake.terminate();awake.wait();save()
print(state['status']);sys.exit(0 if state['status']=='PASSED_EXPECTED_CAUSAL_FAILURES_AND_RESTORED_POSITIVE' else 1)
