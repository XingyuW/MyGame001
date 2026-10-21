"""Summarize actual required RA9 results only after the frozen full13 completes."""
from pathlib import Path
import csv,datetime,hashlib,json,math,re,statistics
ROOT=Path(__file__).resolve().parents[4];OUT=Path(__file__).resolve().parent
sha=lambda b:hashlib.sha256(b).hexdigest()
def raw(p):return {'path':str(p.relative_to(ROOT)),'bytes':p.stat().st_size,'sha256':sha(p.read_bytes())}
def dist(values):
 a=sorted(values);assert a
 return {'count':len(a),'min':a[0],'median':statistics.median(a),'p90_nearest_rank':a[math.ceil(len(a)*.9)-1],'max':a[-1]}
g=json.loads((OUT/'gate-process.json').read_text());assert g['status']=='PASSED' and g['exit']==0 and len(g['commands'])==13
commands=[]
for i,row in enumerate(g['commands'],1):
 m=re.match(r'^(\d\d) exit (\d+)  (\d+) s  (.*)$',row['line']);assert m and int(m[1])==i and int(m[2])==0 and not row['source_after']['mismatches'];commands.append({'number':i,'exit':0,'script_date_seconds':int(m[3]),'line':row['line'],'source_after':row['source_after']})
assert g['source_before']['files']==669 and g['source_after']['files']==669
assert not g['source_after']['mismatches']
text=(OUT/'04-test.log').read_text();results=re.findall(r'^test result:.*? (\d+) passed; (\d+) failed; (\d+) ignored;.*? (\d+) filtered out;',text,re.M);assert results
counts=[sum(int(x[i]) for x in results) for i in range(4)];assert counts[1:]==[0,0,0]
a=text.index('Running tests/ra9_data_parity.rs');z=text.find('\n     Running ',a+1);z=z if z>=0 else len(text);audit=re.findall(r'^test result:.*? (\d+) passed; (\d+) failed; (\d+) ignored;.*? (\d+) filtered out;',text[a:z],re.M);assert audit==[('181','0','0','0')],audit
# The actual test writes only Won results after every job and its failure guard complete.
phase4=g['commands'][3]['actual_production_length_table'];assert phase4['mtime_changed_since_launch'] and phase4['sha256']==sha((OUT/'production-lengths.tsv').read_bytes())
production=list(csv.DictReader((OUT/'production-lengths.tsv').open(),delimiter='\t'));assert len(production)==756 and len({x['match'] for x in production})==756
for x in production:assert 0<int(x['ticks'])<=int(x['limit'])
prod={'raw':raw(OUT/'production-lengths.tsv'),'actual_test_source':'crates/game/tests/m6_production_matches.rs: only MatchOutcome::Won appends; played/failure/length guards require every job. Fresh cache mtime and full workspace exit0 verified.','won':len(production),'ticks':dist([int(x['ticks']) for x in production]),'normalized_over_original_map_limit':dist([int(x['ticks'])/int(x['limit']) for x in production]),'rows':production}
duels=[]
pattern=r'^([a-z_]+) (\d+)x(\d+): (.+): ended in tick (\d+) \(Won \{ winner: PlayerId\((\d+)\), tick: (\d+) \}\) after ([\d.]+)s$'
for m in re.finditer(pattern,(OUT/'10-duels.log').read_text(),re.M):
 assert m[5]==m[7];w,h=int(m[2]),int(m[3]);duels.append({'match':m[1]+': '+m[4],'ticks':int(m[5]),'winner':int(m[6]),'original_production_limit':72000*max(w+h,128)//128,'duel_limit':300000,'rounded_seconds':float(m[8])})
assert len(duels)==756 and len({x['match'] for x in duels})==756
assert all(0<x['ticks']<300000 for x in duels)
duel={'raw':raw(OUT/'10-duels.log'),'explicit_Won':len(duels),'non_Won':0,'ticks':dist([x['ticks'] for x in duels]),'normalized_over_original_map_limit':dist([x['ticks']/x['original_production_limit'] for x in duels]),'normalized_over_duel_limit':dist([x['ticks']/300000 for x in duels]),'rows':duels}
soaktext=(OUT/'09-soak.log').read_text();windows=re.findall(r'^(.+): 36000 ticks, (\d+) matches \((\d+) ended, the last (still running|ended)\), ([\d.]+) s for both runs$',soaktext,re.M);assert len(windows)==260
for _,started,ended,last,_ in windows:assert int(started)==int(ended)+(last=='still running')
s=re.search(r'm7 soak: (\d+) ticks in ([\d.]+) s; peak heap in use ([\d.]+) MiB \(budget (\d+) MiB\)',soaktext);assert s and int(s[1])==260*2*36000
soak={'raw':raw(OUT/'09-soak.log'),'configurations':len(windows),'repeats':2,'ticks_each_repeat':36000,'total_ticks':int(s[1]),'seconds':float(s[2]),'peak_heap_mib':float(s[3]),'heap_budget_mib':int(s[4]),'ended_matches_per_repeat':sum(int(x[2]) for x in windows),'final_worlds_still_running_per_repeat':sum(x[3]=='still running' for x in windows),'final_worlds_ended_per_repeat':sum(x[3]=='ended' for x in windows),'scope':'R56item6 fixed-duration deterministic windows, actual completed-game Won guards. Final sampled Running worlds do not prove all-to-Won outcomes; separate production/duel/all21 endings retain independent scopes. Heap is concurrent pooled peak, not per-world heap.'}
benchtext=(OUT/'08-benchmark.log').read_text();scenarios=[]
for m in re.finditer(r'^(fixture|shipped): (\d+) units spawned; mean tick ([\d.]+) ms, longest tick ([\d.]+) ms; mean over the (\d+) ticks with at least 150 units alive ([\d.]+) ms; units alive every 500 ticks (\[[^\]]+\]); (\d+) units moved, (\d+) shots, (\d+) units destroyed$',benchtext,re.M):
 scenarios.append({'name':m[1],'spawned_units':int(m[2]),'mean_ms':float(m[3]),'max_ms':float(m[4]),'ticks_with_at_least_150_alive':int(m[5]),'mean_with_at_least_150_alive_ms':float(m[6]),'alive_samples':json.loads(m[7]),'units_moved':int(m[8]),'shots':int(m[9]),'units_destroyed':int(m[10])})
assert len(scenarios)==2 and all(x['mean_ms']<=5 and x['max_ms']<=50 and x['spawned_units']==200 for x in scenarios)
b=re.search(r'peak heap in use: ([\d.]+) MiB \(budget (\d+) MiB\)',benchtext);assert b
bench={'raw':raw(OUT/'08-benchmark.log'),'measured_ticks_each_scenario':2000,'original_mean_budget_ms':5,'original_longest_tick_budget_ms':50,'peak_heap_mib':float(b[1]),'heap_budget_mib':int(b[2]),'scenarios':scenarios,'scope':'200 original spawned participants over time; exact >=150-alive windows separately reported. No 4000-unit or render-smoothness claim.'}
package=Path('/Users/xingyuwang/Library/Caches/Codex/cargo-target/package');binary=package/'MyGame001.app/Contents/MacOS/MyGame001';archive=package/'MyGame001-macos.zip';assert binary.is_file() and archive.is_file()
pkg={'raw':raw(OUT/'11-package.log'),'binary':{'path':str(binary),'bytes':binary.stat().st_size,'sha256':sha(binary.read_bytes())},'archive':{'path':str(archive),'bytes':archive.stat().st_size,'sha256':sha(archive.read_bytes())},'readme_snapshot':{'path':str(binary.parent/'README.md'),'bytes':(binary.parent/'README.md').stat().st_size,'sha256':sha((binary.parent/'README.md').read_bytes()),'scope':'Actual phase11 gate-launch README snapshot; later excluded current-output closing prose does not change tested binary/assets or retroactively replace this snapshot.'},'scope':'Actual arm64/macOS package build and600-frame packaged executable smoke from external working directory without asset-root/manifest environment. Signing/notarization/universal/other-Mac launch, user quality and complete dependency-notice acceptance not inferred.'}
source=json.loads((OUT/'195-actual-canonical-input-scope.json').read_text())
for x in source['source_files']:assert sha((ROOT/x['path']).read_bytes())==x['sha256']
for a in source['authority_fragments']:
 b=(ROOT/a['document_path']).read_bytes();at=b.index(a['section_start'].encode());assert sha(b[at:at+a['span_bytes']])==a['sha256']
record={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'status':'ACTUAL_CANONICAL_RA9_REQUIRED13_AND_ISOLATED5_PASS_NOT_REVIEW_OR_USER_APPROVAL','source_freeze_commit':'f58e8d9e1e6a8b8cf4f1ce31a80c229cfd6203c1','gate_launch_head':g['source_commit'],'source_packet':raw(OUT/'195-actual-canonical-input-scope.json'),'source_files':656,'binding_outputs':13,'authority_spans':64,'source_after':g['source_after'],'gate_process':raw(OUT/'gate-process.json'),'summary':raw(OUT/'summary.txt'),'commands':commands,'workspace':{'raw':raw(OUT/'04-test.log'),'result_targets':len(results),'passed':counts[0],'failed':counts[1],'ignored':counts[2],'filtered':counts[3],'full_ra9_audit_target':{'passed':181,'failed':0,'ignored':0,'filtered':0},'scope':'Actual unfiltered release workspace; lexical inventory is separate.'},'production':prod,'duels':duel,'soak':soak,'benchmark':bench,'package':pkg,'isolated_acceptance':raw(OUT/'198-isolated-acceptance-summary.json'),'fail_fast':raw(OUT/'197c-fail-fast-process.json'),'retained_failures':['197 sandbox configured-cache denial before compilation','197b whole-positive stale derived consumer branch pointer failure; factual196b correction separately verified','All prior portable history retains failed and intended-causal scopes, including159 full175/6 and168 six-only repair results.'],'review':'Fresh independent five slices3+2 and freshlead pending; implementers/preparers ineligible. Retained implementation debt severity/acceptance belongs panel, no assumed nonblocking classification.','user_verification':'All51 HOWTOSEE rows unchanged, requires actual user visual/audio/feel/human-match/platform verification.'}
f=OUT/'200-final-canonical-validation-summary.json';assert not f.exists();f.write_text(json.dumps(record,indent=2)+'\n');print(json.dumps({'workspace':record['workspace'],'production_ticks':prod['ticks'],'duel_ticks':duel['ticks'],'soak':soak,'benchmark':bench,'package_sizes':{'binary':pkg['binary']['bytes'],'archive':pkg['archive']['bytes']},'summary_sha256':sha(f.read_bytes())},indent=2))
