"""Sequential original gates on a new exact freeze; stops on any actual failure."""
from pathlib import Path
import datetime,hashlib,json,os,subprocess,sys,time,traceback
R=Path('/Users/xingyuwang/GameProjects/MyGame001');os.chdir(R);S=Path(__file__).parent
PLAN=Path(sys.argv[1]).resolve();plan=json.loads(PLAN.read_bytes());Fpath=Path(plan['frozen_manifest']).resolve();fb=Fpath.read_bytes();F=json.loads(fb)
LABEL=plan['full13_label'];I=R/plan['isolated_output'];receipt=S/plan['pipeline_receipt'];assert not receipt.exists() and not I.exists()
sha=lambda b:hashlib.sha256(b).hexdigest()
def now():return datetime.datetime.now(datetime.timezone.utc).isoformat()
state={'status':'PREPARING_ACTUAL_ISOLATED_FIVE','start':now(),'pid':os.getpid(),'runner_sha256':sha(Path(__file__).read_bytes()),'plan_sha256':sha(PLAN.read_bytes()),'commands':[],'limits':'Sequential unchanged gates only. No policy waiver, independent approval, merge or tag. Stops on failed command/guard.'}
def save():
 state['heartbeat']=now();q=receipt.with_suffix('.next');q.write_text(json.dumps(state,indent=2)+'\n');q.replace(receipt)
def frozen():
 assert Fpath.read_bytes()==fb
 for row in F['source_files']:assert sha((R/row['path']).read_bytes())==row['sha256'],row['path']
 for a in F['authority_fragments']:
  b=(R/a['document_path']).read_bytes();at=b.index(a['section_start'].encode());assert sha(b[at:at+a['span_bytes']])==a['sha256']
 b=(R/'docs/RA2_GAP_ANALYSIS.md').read_bytes();a=b.index(b'## How to perform user verification');z=b.index(b'## Historical baseline and source provenance');assert sha(b[a:z])==F['howtosee']['sha256']
def run(name,args):
 log=S/(plan['pipeline_log_prefix']+name+'.log');assert not log.exists();row={'name':name,'command':list(map(str,args)),'start':now(),'raw':str(log)};state['commands'].append(row);save()
 with log.open('xb') as out:
  child=subprocess.Popen(row['command'],stdout=out,stderr=subprocess.STDOUT);row['pid']=child.pid;save()
  while child.poll() is None:save();time.sleep(5)
  row['exit']=child.wait()
 row.update(end=now(),raw_sha256=sha(log.read_bytes()),raw_bytes=log.stat().st_size);save();assert row['exit']==0,(name,row['exit'])
try:
 frozen();assert subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()==plan['frozen_tip'];assert not subprocess.check_output(['git','status','--porcelain'],text=True).strip();save()
 run('isolated-five',[sys.executable,S/'run-frozen-bounded-v4.py',plan['isolated_plan']]);frozen()
 d=json.loads((I/'process.json').read_bytes());assert d['status']=='COMPLETED_WITH_ACTUAL_EXITS_NOT_AUTOMATIC_PASS' and len(d['commands'])==5 and d['source_unchanged'] and all(c['exit']==0 and c['source_unchanged'] for c in d['commands'])
 state['status']='SUMMARIZING_ACTUAL_ISOLATED_PASS';save();run('isolated-summary',[sys.executable,S/'summarize-remediation2-isolated-five.py',I.relative_to(R),str(len(F['source_files']))]);frozen()
 assert not subprocess.check_output(['git','diff','HEAD','--name-only'],text=True).strip();rows=subprocess.check_output(['git','status','--porcelain','--untracked-files=all'],text=True).splitlines();assert all(x.startswith('?? '+str(I.relative_to(R))+'/') for x in rows),rows
 run('preserve-isolated',['git','add','--',str(I.relative_to(R))]);run('commit-isolated',['git','commit','-q','-m','RA9 preserve corrected-source isolated match acceptance'])
 assert not subprocess.check_output(['git','status','--porcelain'],text=True).strip();tip=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip();state['full13_launch_tip']=tip;state['status']='RUNNING_SINGLE_SHARED_FULL13';save()
 run('full13',[sys.executable,S/plan['full13_runner'],tip,LABEL,Fpath]);frozen()
 state['status']='RUNNING_RELEASE_AND_PACKAGE_ASSET_FAILURE_RESTORATION';save();run('assets',[sys.executable,S/'run-asset-failure-acceptance-v2.py',tip,LABEL+'-assets']);frozen()
 state['status']='SUMMARIZING_ACTUAL_ALL_GATES';save();run('full-summary',[sys.executable,S/'summarize-remediation2-full13-c3-v2.py','docs/validation/ra2-parity/'+LABEL,I.relative_to(R)/'acceptance-summary.json','docs/validation/ra2-parity/'+LABEL+'-assets/process.json',str(len(F['source_files'])),str(plan['expected_ai_tests'])]);frozen()
 run('factual-closing-metadata',[sys.executable,S/'prepare-final-evidence-metadata-c3.py',LABEL]);frozen();state['status']='ALL_NATIVE_GATES_COMPLETE_FIVE_FINAL_EVIDENCE_ADDENDA_AND_FRESH_LEAD_REQUIRED'
except BaseException as e:state.update(status='STOPPED_ON_FAILURE_OR_GUARD',error=repr(e),traceback=traceback.format_exc())
finally:state['end']=now();save()
print(state['status'],flush=True);sys.exit(0 if state['status'].startswith('ALL_NATIVE_GATES_COMPLETE') else 1)
