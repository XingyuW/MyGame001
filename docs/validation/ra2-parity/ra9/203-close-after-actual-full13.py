"""Finish prepared output provenance only after actual required native validation; no commit/review."""
from pathlib import Path
import datetime,hashlib,json,os,subprocess,sys,time
ROOT=Path(__file__).resolve().parents[4];OUT=Path(__file__).resolve().parent;os.chdir(ROOT)
sha=lambda b:hashlib.sha256(b).hexdigest()
def utc():return datetime.datetime.now(datetime.timezone.utc).isoformat()
path=OUT/'203-postgate-closing-process.json';assert not path.exists()
scripts=['200-summarize-final-validation.py','201-close-current-canonical-documents.py','202-prepare-clean-review-handoff.py']
record={'start_utc':utc(),'pid':os.getpid(),'status':'WAITING_FOR_ACTUAL_FULL13','scripts':{s:sha((OUT/s).read_bytes()) for s in scripts},'steps':[],'scope':'Only prepared required result/output reconciliation after actual199PASS. No Cargo/native, policy acceptance, review, user approval, Git mutation or ownership release.'}
def save():path.write_text(json.dumps(record,indent=2)+'\n')
save()
try:
 while True:
  c=json.loads((OUT/'198-to-199-continuation.json').read_text());status=c['status']
  if status=='FULL13_PASSED':break
  if status in ['FULL13_FAILED','STOPPED_198_FAILURE'] or status.startswith('FAILED'):
   record.update(status='STOPPED_VALIDATION_FAILURE',end_utc=utc(),upstream_status=status);save();sys.exit(1)
  time.sleep(5)
 gate=json.loads((OUT/'gate-process.json').read_text());assert gate['status']=='PASSED' and gate['exit']==0 and len(gate['commands'])==13
 for name in scripts:
  assert sha((OUT/name).read_bytes())==record['scripts'][name],name
  row={'script':name,'start_utc':utc()};record['steps'].append(row);record['status']='CLOSING_ACTUAL_OUTPUTS';save()
  raw=OUT/('203-'+name.removesuffix('.py')+'.log');assert not raw.exists()
  with raw.open('wb') as f:code=subprocess.call([sys.executable,str(OUT/name)],stdout=f,stderr=subprocess.STDOUT)
  row.update(exit=code,end_utc=utc(),raw={'path':str(raw.relative_to(ROOT)),'bytes':raw.stat().st_size,'sha256':sha(raw.read_bytes())});save()
  if code:record.update(status='STOPPED_CLOSING_SCRIPT_FAILURE',end_utc=utc());save();sys.exit(code)
 record.update(status='OUTPUTS_READY_FOR_FINAL_INSPECTION_AND_CLEAN_COMMIT',end_utc=utc());save();print(record['status'],flush=True)
except Exception as e:
 record.update(status='STOPPED_EXCEPTION',error=repr(e),end_utc=utc());save();raise
