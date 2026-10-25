"""Continue the already authorized dependent 198 -> clean evidence commit -> 199 sequence."""
from pathlib import Path
import datetime,hashlib,json,os,subprocess,time,sys
ROOT=Path(__file__).resolve().parents[4];OUT=Path(__file__).resolve().parent;os.chdir(ROOT)
def utc():return datetime.datetime.now(datetime.timezone.utc).isoformat()
def save():receipt.write_text(json.dumps(record,indent=2)+'\n')
receipt=OUT/'198-to-199-continuation.json';assert not receipt.exists();record={'start_utc':utc(),'pid':os.getpid(),'status':'WAITING_FOR_198','scope':'Only the pre-authorized required sequence.198 failure stops here;198PASS parses actual metrics, commits owned evidence/STATUS, then runs unchanged mandatory199full13. No new tests, source changes, caps, approval or policy decisions.'};save()
while True:
 p=json.loads((OUT/'198-isolated-acceptance-process.json').read_text())
 if p['status']!='RUNNING':break
 time.sleep(2)
if p['status']!='PASSED':
 record.update({'status':'STOPPED_198_FAILURE','end_utc':utc()});save();sys.exit(1)
subprocess.run(['python3',str(OUT/'198-summarize-isolated-acceptance.py')],check=True)
s=json.loads((OUT/'198-isolated-acceptance-summary.json').read_text());assert s['all21']['maps'] and len(s['all21']['maps'])==21
f=ROOT/'docs/STATUS.md';pre=OUT/'198-before-isolated-summary-STATUS.md';assert not pre.exists();pre.write_bytes(f.read_bytes())
maxmean=max(x['mean_ms'] for x in s['all21']['maps']);lengths=s['all21']['won_tick_distribution'];guardcounts=[x['passed'] for x in s['guards']]
f.write_text(f.read_text().rstrip()+'\n\n## RA9 isolated acceptance complete; fresh full13 follows — 2026-10-21\n\n'+f"Actual198 required commands all exited0 on unchanged frozen RA9 inputs. All21 ten-player\n"+f"matches reached Won within the original400,000 cap; ticks min/median/p90/max are\n{lengths['min']}/{lengths['median']}/{lengths['p90_nearest_rank']}/{lengths['max']}. Maximum measured mean was\n{maxmean:.6f}ms (required<5ms). Fixed3 original24,000-schedule timing cases and all three example\n"+"guards passed. Mean, busy-window, p99 and max timing remain distinct; sampled fixed/soak windows\nare not all-to-Won claims.198-isolated-acceptance-summary.json preserves every actual map, tail\nand command source/raw binding. Full13 remains mandatory and is now the next owned action;\nno previous RA8/copy execution transfers acceptance or performance credit.\n")
# No runtime/source or authority change. All owned logs/metrics are stable before commit.
packet=json.loads((OUT/'195-actual-canonical-input-scope.json').read_text())
for x in packet['source_files']:assert hashlib.sha256((ROOT/x['path']).read_bytes()).hexdigest()==x['sha256']
for a in packet['authority_fragments']:
 b=(ROOT/a['document_path']).read_bytes();at=b.index(a['section_start'].encode());assert hashlib.sha256(b[at:at+a['span_bytes']]).hexdigest()==a['sha256']
record.update({'status':'ISOLATED_PASSED_READY_FOR_COMMIT_AND_FULL13','isolated_summary_sha256':hashlib.sha256((OUT/'198-isolated-acceptance-summary.json').read_bytes()).hexdigest(),'isolated_complete_utc':utc()});save()
subprocess.run(['git','add','--','docs/validation/ra2-parity/ra9','docs/STATUS.md'],check=True)
subprocess.run(['git','commit','-q','-m','RA9 record isolated ending timing and guard validation'],check=True)
assert not subprocess.check_output(['git','status','--porcelain'])
# Keep receipt unchanged through199's clean-tip assertion.199 records actual launchHEAD/PGID.
print('198PASS committed; launching exact mandatory199full13',flush=True)
code=subprocess.run(['python3',str(OUT/'199-run-frozen-full13.py')]).returncode
record.update({'status':'FULL13_PASSED' if code==0 else 'FULL13_FAILED','full13_exit':code,'end_utc':utc()});save();sys.exit(code)
