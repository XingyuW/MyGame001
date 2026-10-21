import datetime,hashlib,json,os,signal,subprocess,time
from pathlib import Path
r=Path.cwd();p=r/'ra9-preliminary-logs';canonical=Path('/Users/xingyuwang/GameProjects/MyGame001');target=r/'crates/game/tests/m6_production_matches.rs';original=target.read_bytes();pre=p/'187-before-late-controls.rs';assert not pre.exists();pre.write_bytes(original)
sha=lambda b:hashlib.sha256(b).hexdigest()
def utc():return datetime.datetime.now(datetime.timezone.utc).isoformat()
def phase_done():return any(x.startswith('04 exit ') for x in (canonical/'docs/validation/ra2-parity/ra8/summary.txt').read_text().splitlines())
scope=json.loads((p/'166-six-test-repairs-input-scope.json').read_text());names=sorted([x['path'] for x in scope['source_files']]+['crates/sim/tests/ra9_start_setup.rs'])
def snapshot():return [{'path':n,'bytes':(r/n).stat().st_size,'sha256':sha((r/n).read_bytes())} for n in names]
cmd=['cargo','test','--release','-p','game','--test','m6_production_matches','ra9_native_twins_remain_active_after_36000_and_reach_the_same_winner','--','--exact','--nocapture','--test-threads=1']
phases=['positive','rng-causal','production-observer-causal','restored-positive'];results=[];all_before=snapshot();(p/'187-before-source.json').write_text(json.dumps(all_before,indent=2)+'\n');child=None
try:
 for i,label in enumerate(phases):
  if phase_done():print('PHASE04_ENDED_NO_LAUNCH',flush=True);break
  assert target.read_bytes()==original
  changed=original.decode()
  if label=='rng-causal':
   needle='        let hash = sim::state_hash(left.world_mut());';assert changed.count(needle)==1
   changed=changed.replace(needle,'        if native_tick == 36_001 {\n            let _ = right.world_mut().resource_mut::<sim::SimRng>().next_u32();\n        }\n'+needle)
  elif label=='production-observer-causal':
   needle='                    SimEvent::ProductionComplete { .. } => Some(1),';assert changed.count(needle)==1;changed=changed.replace(needle,'                    SimEvent::ProductionComplete { .. } => None,')
  target.write_text(changed);mutated_sha=sha(target.read_bytes());before=snapshot();start=utc();began=time.monotonic();raw=p/('187-'+str(i).zfill(2)+'-'+label+'.log');assert not raw.exists();start_path=p/('187-'+str(i).zfill(2)+'-'+label+'-start.json');start_path.write_text(json.dumps({'utc':start,'command':cmd,'label':label,'source':before,'selected_target_original_sha256':sha(original),'selected_target_current_sha256':mutated_sha,'scope':'ACTUAL_CURRENT_AI_COMBAT_COPY_RUNTIME_FUNCTIONAL_OVERLAP_NO_PERFORMANCE_OR_CANONICAL_GATE','mutation_authority':'Exact existing112 extraSimRng36001 /117observer-only ProductionComplete drop; no threshold/limit/config changes'},indent=2)+'\n')
  env=os.environ.copy();env['CARGO_BUILD_JOBS']='1'
  with raw.open('wb') as log:
   child=subprocess.Popen(cmd,cwd=r,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True);(p/'187-current-process.json').write_text(json.dumps({'pid':child.pid,'pgid':child.pid,'command':cmd,'label':label,'utc':start})+'\n');cancelled=False
   while child.poll() is None:
    if phase_done():
     cancelled=True;os.killpg(child.pid,signal.SIGTERM)
     try:child.wait(timeout=10)
     except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait()
     break
    time.sleep(2)
   code=child.wait();child=None
  after=snapshot();drift=[n['path'] for n,o in zip(after,before) if n!=o]
  # Immediate exact restoration, then a separate subsequent filesystem read before any next run.
  target.write_bytes(original);restore1={'utc':utc(),'sha256':sha(target.read_bytes())};time.sleep(.05);restore2={'utc':utc(),'sha256':sha(target.read_bytes())};assert restore1['sha256']==restore2['sha256']==sha(original)
  record={'utc':start,'end_utc':utc(),'label':label,'command':cmd,'exit_code':code,'seconds':round(time.monotonic()-began,3),'phase04_ended_cancellation':cancelled,'scope':'FUNCTIONAL_OVERLAP_NO_PERFORMANCE_NO_CANONICAL_OR_ORIGINAL_ENGINE_ACCEPTANCE','source_before':str(start_path.relative_to(r)),'source_after':after,'source_drift_during_command':drift,'selected_target_mutated_sha256':mutated_sha,'restoration_immediate':restore1,'restoration_separate_later_read':restore2,'raw':{'path':str(raw.relative_to(r)),'bytes':raw.stat().st_size,'sha256':sha(raw.read_bytes())}}
  (p/('187-'+str(i).zfill(2)+'-'+label+'.json')).write_text(json.dumps(record,indent=2)+'\n');results.append({k:v for k,v in record.items() if k!='source_after'});print(json.dumps(results[-1]),flush=True);assert not drift
  if cancelled or (label in ['positive','restored-positive'] and code!=0):break
finally:
 if child is not None and child.poll() is None:
  os.killpg(child.pid,signal.SIGTERM)
  try:child.wait(timeout=10)
  except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait()
 target.write_bytes(original)
 final={'end_utc':utc(),'results':results,'owned_child_processes':'ALL_WAITED_AND_EXITED','phase04_done':phase_done(),'target_restored_sha256':sha(target.read_bytes()),'source_before_after_drift':[n['path'] for n,o in zip(snapshot(),all_before) if n!=o]};(p/'187-late-bounded-summary.json').write_text(json.dumps(final,indent=2)+'\n');print(json.dumps(final),flush=True)
