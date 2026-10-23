"""Summarize actual required RA9 results only after the frozen full13 completes."""
from pathlib import Path
import csv,datetime,hashlib,json,math,re,statistics,sys,importlib.util,os
ROOT=Path('/Users/xingyuwang/GameProjects/MyGame001');OUT=ROOT/sys.argv[1];ISOLATED=ROOT/sys.argv[2];ASSETS=ROOT/sys.argv[3];EXPECTED=int(sys.argv[4]);EXPECTED_AI=int(sys.argv[5])
sha=lambda b:hashlib.sha256(b).hexdigest()
def raw(p):return {'path':str(p.relative_to(ROOT)),'bytes':p.stat().st_size,'sha256':sha(p.read_bytes())}
def dist(values):
 a=sorted(values);assert a
 return {'count':len(a),'min':a[0],'median':statistics.median(a),'p90_nearest_rank':a[math.ceil(len(a)*.9)-1],'max':a[-1]}
g=json.loads((OUT/'gate-process.json').read_text());assert g['status']=='PASSED' and g['exit']==0 and len(g['commands'])==13
commands=[]
for i,row in enumerate(g['commands'],1):
 m=re.match(r'^(\d\d) exit (\d+)  (\d+) s  (.*)$',row['line']);assert m and int(m[1])==i and int(m[2])==0 and not row['source_after']['mismatches'];commands.append({'number':i,'exit':0,'script_date_seconds':int(m[3]),'line':row['line'],'source_after':row['source_after']})
assert g['source_before']['files']==EXPECTED and g['source_after']['files']==EXPECTED
assert not g['source_after']['mismatches']
text=(OUT/'04-test.log').read_text();results=re.findall(r'^test result:.*? (\d+) passed; (\d+) failed; (\d+) ignored;.*? (\d+) filtered out;',text,re.M);assert results
primary=[x for x in results if x[3]=='0'];children=[x for x in results if x[3]!='0'];counts=[sum(int(x[i]) for x in primary) for i in range(4)];assert counts[1:]==[0,0,0];assert all(x[1:3]==('0','0') for x in results)
spec=importlib.util.spec_from_file_location('accounting',Path(__file__).resolve().parent/'ra9-result-accounting-draft.py');accounting=importlib.util.module_from_spec(spec);spec.loader.exec_module(accounting);execution_accounting=accounting.account(OUT/'04-test.log');assert not execution_accounting['ambiguous_primary_contexts']
a=text.index('Running tests/ra9_data_parity.rs');z=text.find('\n     Running ',a+1);z=z if z>=0 else len(text);audit=re.findall(r'^test result:.*? (\d+) passed; (\d+) failed; (\d+) ignored;.*? (\d+) filtered out;',text[a:z],re.M);assert audit==[('185','0','0','0')],audit
a=text.index('Running tests/ra8_ai.rs');z=text.find('\n     Running ',a+1);z=z if z>=0 else len(text);ai_full=re.findall(r'^test result:.*? (\d+) passed; (\d+) failed; (\d+) ignored;.*? (\d+) filtered out;',text[a:z],re.M);assert ai_full==[(str(EXPECTED_AI),'0','0','0')],ai_full
# Bind legitimate additions while retaining every original C-source integration test identity.
identity_pattern=re.compile(r'(?m)^\s*#\[test\]\s*fn\s+(\w+)')
old_ai_text=__import__('subprocess').check_output(['git','-C',str(ROOT),'show','5c93acb:crates/sim/tests/ra8_ai.rs'],text=True)
old_ai_names=set(identity_pattern.findall(old_ai_text));current_ai_names=set(identity_pattern.findall((ROOT/'crates/sim/tests/ra8_ai.rs').read_text()))
assert len(old_ai_names)==139 and old_ai_names<=current_ai_names and len(current_ai_names)==EXPECTED_AI
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
package=Path(os.environ['CARGO_TARGET_DIR'])/'package';binary=package/'MyGame001.app/Contents/MacOS/MyGame001';archive=package/'MyGame001-macos.zip';assert binary.is_file() and archive.is_file()
pkg={'raw':raw(OUT/'11-package.log'),'binary':{'path':str(binary),'bytes':binary.stat().st_size,'sha256':sha(binary.read_bytes())},'archive':{'path':str(archive),'bytes':archive.stat().st_size,'sha256':sha(archive.read_bytes())},'readme_snapshot':{'path':str(binary.parent/'README.md'),'bytes':(binary.parent/'README.md').stat().st_size,'sha256':sha((binary.parent/'README.md').read_bytes()),'scope':'Actual phase11 gate-launch README snapshot; later excluded current-output closing prose does not change tested binary/assets or retroactively replace this snapshot.'},'scope':'Actual arm64/macOS package build and600-frame packaged executable smoke from external working directory without asset-root/manifest environment. Signing/notarization/universal/other-Mac launch, user quality and complete dependency-notice acceptance not inferred.'}
source_path=Path(g['frozen_manifest']['path']);source=json.loads(source_path.read_bytes());assert len(source['source_files'])==EXPECTED
for x in source['source_files']:assert sha((ROOT/x['path']).read_bytes())==x['sha256']
for a in source['authority_fragments']:
 b=(ROOT/a['document_path']).read_bytes();at=b.index(a['section_start'].encode());assert sha(b[at:at+a['span_bytes']])==a['sha256']
isolated=json.loads(ISOLATED.read_bytes());assert isolated['status'].startswith('PASSED_ACTUAL_ISOLATED_FIVE')
assert isolated['source_manifest']==raw(source_path),'Isolated and full13 manifests differ'
isolated_process=ROOT/isolated['process']['path'];assert isolated['process']==raw(isolated_process)
ip=json.loads(isolated_process.read_bytes());assert ip['frozen_manifest']['sha256']==g['frozen_manifest']['sha256'] and Path(ip['frozen_manifest']['path']).resolve()==source_path.resolve()
assert ip['source_unchanged'] and len(ip['commands'])==5 and all(x['exit']==0 and x['source_unchanged'] for x in ip['commands'])
asset_acceptance=json.loads(ASSETS.read_bytes());assert asset_acceptance['status']=='PASSED' and len(asset_acceptance['commands'])==4 and all(r['pass'] for r in asset_acceptance['commands']) and asset_acceptance['canonical_image_unchanged'] and asset_acceptance['overlay_restored']
assert asset_acceptance['tip']==g['source_commit'],'Asset processes used another gate TIP'
release_executable=Path(os.environ['CARGO_TARGET_DIR'])/'release/game';release_sha=sha(release_executable.read_bytes())
for index,row in enumerate(asset_acceptance['commands']):
 expected_exe=release_executable if index<2 else binary
 assert Path(row['command'][0]).resolve()==expected_exe.resolve()
 assert row['executable_sha256']==(release_sha if index<2 else pkg['binary']['sha256'])
 p=ROOT/row['raw'];assert p.stat().st_size==row['raw_bytes'] and sha(p.read_bytes())==row['raw_sha256']
assert g['owned_group_after_direct_exit']['empty'] and g['owned_group_after_direct_exit']['pgid']==g['child_pgid']
record={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'status':'ACTUAL_CORRECTED_SHARED_RA9_FINAL_REQUIRED13_AND_ISOLATED5_PASS_NOT_REVIEW_OR_USER_APPROVAL','gate_launch_head':g['source_commit'],'frozen_manifest':raw(source_path),'source_files':EXPECTED,'authority_spans':64,'source_after':g['source_after'],'gate_process':raw(OUT/'gate-process.json'),'summary':raw(OUT/'summary.txt'),'commands':commands,'workspace':{'raw':raw(OUT/'04-test.log'),'primary_result_blocks':len(primary),'primary_passed':counts[0],'primary_failed':counts[1],'primary_ignored':counts[2],'primary_filtered':counts[3],'full_ra9_audit_target':{'passed':185,'failed':0,'ignored':0,'filtered':0},'full_ai_integration_target':{'passed':EXPECTED_AI,'failed':0,'ignored':0,'filtered':0},'execution_accounting':execution_accounting,'scope':'Actual unchanged unfiltered release workspace; selected nested probes are separately identified. Lexical declarations/configurations are not executions.'},'production':prod,'duels':duel,'soak':soak,'benchmark':bench,'package':pkg,'isolated_acceptance':raw(ISOLATED),'actual_release_and_package_asset_failure_and_restoration':raw(ASSETS),'gate_authorization':'R194 explicit user approval permits this single complete unchanged13 run to satisfy both RA9 and final review; no previous/interrupted run credited.','review':'Five fresh slices3+2 and fresh lead must inspect all cumulative source/evidence and decide; implementers/preparers cannot approve. All183 debt IDs require independent reconciliation, no blanket nonblocking assumption.','user_verification':'All51 HOWTOSEE rows remain pending user visual/audio/feel/human-match/platform verification.'}
f=OUT/'final-validation-summary.json';assert not f.exists();f.write_text(json.dumps(record,indent=2)+'\n');print(json.dumps({'primary_workspace_passed':counts[0],'raw_execution_accounting':execution_accounting['raw_execution_totals'],'production_ticks':prod['ticks'],'duel_ticks':duel['ticks'],'soak_total_ticks':soak['total_ticks'],'soak_peak_heap_mib':soak['peak_heap_mib'],'benchmark':bench['scenarios'],'summary_sha256':sha(f.read_bytes())},indent=2))
