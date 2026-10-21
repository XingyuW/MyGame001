import datetime,hashlib,json,os,signal,subprocess,time
from pathlib import Path
r=Path.cwd();p=r/'ra9-preliminary-logs';canonical=Path('/Users/xingyuwang/GameProjects/MyGame001');tag='181'
sha=lambda b:hashlib.sha256(b).hexdigest()
def utc():return datetime.datetime.now(datetime.timezone.utc).isoformat()
def phase_done():return any(x.startswith('04 exit ') for x in (canonical/'docs/validation/ra2-parity/ra8/summary.txt').read_text().splitlines())
scope=json.loads((p/'166-six-test-repairs-input-scope.json').read_text());names=sorted([x['path'] for x in scope['source_files']]+['crates/sim/tests/ra9_start_setup.rs'])
def snapshot():return [{'path':n,'bytes':(r/n).stat().st_size,'sha256':sha((r/n).read_bytes())} for n in names]
commands=[['cargo','test','--release','-p','sim','--test','ra9_start_setup','--test','ra4_spawns','--test','ra7_orders','--no-run'],['cargo','test','--release','-p','sim','--test','ra9_start_setup','ra9_shipped_yard_starts_for_all_maps_two_to_ten_players_have_legal_occupancy_and_public_exits','--','--exact','--nocapture'],['cargo','test','--release','-p','sim','--test','ra4_spawns','ra9_missile_replacement_has_exact_ready_and_next_launch_boundaries','--','--exact','--nocapture'],['cargo','test','--release','-p','sim','--test','ra4_spawns','ra9_returned_carrier_aircraft_rearms_at_the_exact_scheduled_boundary','--','--exact','--nocapture'],['cargo','test','--release','-p','sim','--test','ra7_orders','ra9_attack_move_combat_at_destination_emits_only_one_completion_for_one_order','--','--exact','--nocapture'],['cargo','run','--release','-p','sim','--example','ra9_catalogue']]
all_before=snapshot();(p/(tag+'-before-source.json')).write_text(json.dumps(all_before,indent=2)+'\n');results=[]
for i,cmd in enumerate(commands):
 if phase_done():print('PHASE04_ENDED_NO_LAUNCH',flush=True);break
 before=snapshot();start=utc();began=time.monotonic();raw=p/(tag+'-'+str(i).zfill(2)+'.log');assert not raw.exists()
 start_path=p/(tag+'-'+str(i).zfill(2)+'-start.json');start_path.write_text(json.dumps({'utc':start,'command':cmd,'scope':'COPY_ONLY_FUNCTIONAL_OVERLAP_NOT_PERFORMANCE_OR_CANONICAL_GATE','source':before,'env':{k:os.environ.get(k) for k in ['CARGO_TARGET_DIR','UV_CACHE_DIR','PIP_CACHE_DIR']},'phase04_done':False},indent=2)+'\n')
 env=os.environ.copy();env['CARGO_BUILD_JOBS']='1'
 with raw.open('wb') as log:
  child=subprocess.Popen(cmd,cwd=r,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
  (p/(tag+'-current-process.json')).write_text(json.dumps({'pid':child.pid,'pgid':child.pid,'command':cmd,'utc':start})+'\n')
  cancelled=False
  while child.poll() is None:
   if phase_done():
    cancelled=True;os.killpg(child.pid,signal.SIGTERM)
    try:child.wait(timeout=10)
    except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait()
    break
   time.sleep(2)
  code=child.wait()
 after=snapshot();drift=[n['path'] for n,o in zip(after,before) if n!=o];record={'utc':start,'end_utc':utc(),'command':cmd,'exit_code':code,'seconds':round(time.monotonic()-began,3),'scope':'COPY_ONLY_FUNCTIONAL_OVERLAP_NO_PERFORMANCE_NO_CANONICAL_ACCEPTANCE','phase04_ended_cancellation':cancelled,'source_before':str(start_path.relative_to(r)),'source_after':after,'source_drift':drift,'raw':{'path':str(raw.relative_to(r)),'bytes':raw.stat().st_size,'sha256':sha(raw.read_bytes())}}
 (p/(tag+'-'+str(i).zfill(2)+'.json')).write_text(json.dumps(record,indent=2)+'\n');results.append({k:v for k,v in record.items() if k!='source_after'});print(json.dumps(results[-1]),flush=True)
 assert not drift
 if cancelled or (i==0 and code!=0):break
final={'end_utc':utc(),'results':results,'owned_child_processes':'ALL_WAITED_AND_EXITED','phase04_done':phase_done(),'source_before_after_drift':[n['path'] for n,o in zip(snapshot(),all_before) if n!=o]};(p/(tag+'-bounded-functional-summary.json')).write_text(json.dumps(final,indent=2)+'\n');print(json.dumps(final),flush=True)
