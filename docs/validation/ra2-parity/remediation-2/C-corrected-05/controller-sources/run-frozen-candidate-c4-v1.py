"""Run all thirteen unchanged commands on a clean frozen candidate with durable receipts."""
from pathlib import Path
import datetime,hashlib,json,os,re,signal,subprocess,sys,time
ROOT=Path('/Users/xingyuwang/GameProjects/MyGame001'); os.chdir(ROOT)
TIP,LABEL,manifest_arg=sys.argv[1:4]
assert re.fullmatch(r'[0-9a-f]{40}',TIP)
assert re.fullmatch(r'[a-z0-9][a-z0-9-]*',LABEL)
MANIFEST=Path(manifest_arg).resolve(); manifest_bytes=MANIFEST.read_bytes()
sha=lambda b:hashlib.sha256(b).hexdigest()
manifest_sha=sha(manifest_bytes); packet=json.loads(manifest_bytes)
files={x['path']:x['sha256'] for x in packet['source_files']}
assert len(files)==len(packet['source_files']) and files
AUTH=packet['authority_fragments']; OUT=ROOT/'docs/validation/ra2-parity'/LABEL
assert len(AUTH)==64 and not OUT.exists(),'Never overwrite an earlier attempt'
assert not re.search(r'#\s*\[\s*should_panic', (ROOT/'crates/game/tests/m6_production_matches.rs').read_text()), 'M6 early-panic stop requires no expected-panic tests'
def utc(): return datetime.datetime.now(datetime.timezone.utc).isoformat()
def guard(launch=False):
    assert sha(MANIFEST.read_bytes())==manifest_sha,'Frozen manifest changed'
    assert subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()==TIP,'HEAD changed'
    assert subprocess.check_output(['git','branch','--show-current'],text=True).strip()=='ra2-parity'
    assert subprocess.run(['git','diff','--quiet','HEAD']).returncode==0,'Tracked candidate changed'
    changed=subprocess.check_output(['git','status','--porcelain','--untracked-files=all'],text=True).splitlines()
    prefix='?? '+str(OUT.relative_to(ROOT))+'/'
    assert all(not launch and x.startswith(prefix) for x in changed),changed
    bad=[name for name,h in files.items() if sha((ROOT/name).read_bytes())!=h]
    for a in AUTH:
        b=(ROOT/a['document_path']).read_bytes(); at=b.index(a['section_start'].encode())
        if sha(b[at:at+a['span_bytes']])!=a['sha256']: bad.append(a['section_start'])
    assert not bad,bad
    return {'files':len(files),'authority_spans':len(AUTH),'mismatches':[],'tracked_diff_empty':True}
initial=guard(True); OUT.mkdir(); path=OUT/'gate-process.json'
record={'start_utc':utc(),'pid':os.getpid(),'controller_pgid':os.getpgrp(),'source_commit':TIP,'label':LABEL,'runner':str(Path(__file__).resolve()),'runner_sha256':sha(Path(__file__).read_bytes()),'frozen_manifest':{'path':str(MANIFEST),'bytes':len(manifest_bytes),'sha256':manifest_sha},'status':'RUNNING','source_before':initial,'commands':[],'scope':'Actual full13 execution only; no independent review or user quality approval. First failed command stops this failed attempt, preserving all completed raw results; only a new all13 successful attempt can pass.','diagnostics':'RUST_TEST_NOCAPTURE=1 changes output only. CARGO_BUILD_JOBS=2 controls compilation concurrency only (no configured build-job value was present); original commands, native test/match workers, scenarios, caps and acceptance unchanged. Both projects keep running; no priority changes.'}
def save():
    record['last_heartbeat_utc']=utc()
    temp=path.with_name(path.name+'.pending'); temp.write_text(json.dumps(record,indent=2)+'\n'); temp.replace(path)
env=os.environ.copy(); assert env.get('CARGO_TARGET_DIR')=='/Users/xingyuwang/Library/Caches/Codex/cargo-target'
env['CARGO_BUILD_JOBS']='2'; env['RUST_TEST_NOCAPTURE']='1'
env.pop('RA2_DUEL_SEED',None); env.pop('RA2_DUEL_PAIR',None)
record['configured_environment']={k:env.get(k) for k in ['CARGO_TARGET_DIR','UV_CACHE_DIR','PIP_CACHE_DIR','TMPDIR','CARGO_BUILD_JOBS','RUST_TEST_NOCAPTURE']}
cache=Path(env['CARGO_TARGET_DIR'])/'tmp/m6_production_matches_lengths.tsv'; before=None
if cache.is_file():
    b=cache.read_bytes(); before={'bytes':len(b),'sha256':sha(b),'mtime_ns':cache.stat().st_mtime_ns}
    (OUT/'before-cache-production-lengths.tsv').write_bytes(b)
record['production_cache_before']=before
cmd=['sh','scripts/validate_ra2_parity.sh',LABEL]; record['command']=cmd
start=time.monotonic(); process=None; code=1; stopped_on_failure=False
save()
def observe(line):
    global stopped_on_failure
    m=re.match(r'^(\d\d) exit (\d+)  (\d+) s  (.*)$',line); assert m
    assert int(m[1])==len(record['commands'])+1
    row={'line':line,'number':int(m[1]),'exit':int(m[2]),'observed_utc':utc(),'elapsed_seconds':time.monotonic()-start,'source_after':guard()}
    if int(m[1])==4 and cache.is_file():
        b=cache.read_bytes(); (OUT/'production-lengths.tsv').write_bytes(b)
        row['actual_production_length_table']={'repository_path':str((OUT/'production-lengths.tsv').relative_to(ROOT)),'bytes':len(b),'sha256':sha(b),'cache_mtime_ns':cache.stat().st_mtime_ns,'mtime_changed_since_launch':before is None or cache.stat().st_mtime_ns!=before['mtime_ns']}
    record['commands'].append(row); print(line,flush=True); save()
    if row['exit'] and process is not None and process.poll() is None:
        stopped_on_failure=True; record['stop_reason']='Actual command '+m[1]+' failed; preserve this attempt and require a fresh complete successful gate.'
        os.killpg(process.pid,signal.SIGTERM)
try:
    with (OUT/'full13-wrapper.log').open('xb') as raw:
        process=subprocess.Popen(cmd,stdout=raw,stderr=subprocess.STDOUT,stdin=subprocess.DEVNULL,env=env,start_new_session=True)
        record.update(child_pid=process.pid,child_pgid=process.pid); save(); seen=0
        while True:
            lines=(OUT/'summary.txt').read_text().splitlines() if (OUT/'summary.txt').exists() else []
            for line in lines[seen:]:
                if re.match(r'^\d\d exit ',line): observe(line)
            seen=len(lines)
            # These exact production non-Won and unexpected M6 panic witnesses are required failures.
            # The frozen M6 source was checked to contain no expected-panic tests.
            # Stop a doomed attempt promptly; no successful command/gate is inferred.
            test_log=OUT/'04-test.log'
            if not stopped_on_failure and test_log.exists() and process.poll() is None:
                data=test_log.read_bytes()
                witnesses=[x for x in data.decode(errors='replace').splitlines() if re.match(r'^M6 non-Won seed[0-9]+ ',x) or 'panicked at crates/game/tests/m6_production_matches.rs:' in x]
                if witnesses:
                    stopped_on_failure=True
                    record['stop_reason']='Actual required M6 failure witness (non-Won or unexpected test panic); failed/interrupted attempt, never acceptance.'
                    record['actual_m6_failure_stop']={'utc':utc(),'native_witnesses':witnesses,'raw_bytes_at_stop':len(data),'raw_sha256_at_stop':sha(data),'verified_owned_validator_pid':process.pid,'verified_owned_pgid':os.getpgid(process.pid),'argv':cmd,'signal':'SIGTERM','native_grandchild_exit':'Not individually observed; direct validator shell exit will be recorded separately.'}
                    assert os.getpgid(process.pid)==process.pid
                    (OUT/'witnessed-m6-failure-stop.json').write_text(json.dumps(record['actual_m6_failure_stop'],indent=2)+'\n')
                    save();os.killpg(process.pid,signal.SIGTERM)
            if process.poll() is not None: break
            save(); time.sleep(5)
        code=process.wait()
        # Direct exit does not establish that its owned native descendants drained.
        group_polls=[]
        for attempt in range(16):
            listing=subprocess.check_output(['ps','-axo','pid=,pgid=,stat=,args='],text=True)
            remaining=[]
            for line in listing.splitlines():
                parts=line.strip().split(None,3)
                if len(parts)==4 and parts[1].isdigit() and int(parts[1])==process.pid:
                    remaining.append({'pid':int(parts[0]),'pgid':int(parts[1]),'stat':parts[2],'argv':parts[3]})
            group_polls.append({'utc':utc(),'remaining':remaining})
            if not remaining:break
            time.sleep(1)
        record['owned_group_after_direct_exit']={'pgid':process.pid,'empty':not remaining,'observations':group_polls}
        save();assert not remaining,('Owned validator descendants remain; no further lease',remaining)
        lines=(OUT/'summary.txt').read_text().splitlines() if (OUT/'summary.txt').exists() else []
        for line in lines[seen:]:
            if re.match(r'^\d\d exit ',line): observe(line)
    terminal='validation '+LABEL+': PASSED'
    passed=code==0 and len(record['commands'])==13 and all(x['exit']==0 for x in record['commands']) and terminal in lines
    record.update(exit=code,status='PASSED' if passed else 'FAILED',terminal_result_observed=terminal in lines,source_after=guard())
    if not passed and code==0: code=1; record['wrapper_exit']=1
except BaseException as e:
    record['error']=repr(e)
    if process is not None and process.poll() is None:
        os.killpg(process.pid,signal.SIGTERM)
        try: process.wait(timeout=15)
        except subprocess.TimeoutExpired: os.killpg(process.pid,signal.SIGKILL); process.wait()
    record.update(status='GUARD_OR_WRAPPER_FAILURE',observed_child_exit=None if process is None else process.returncode)
    code=1
finally:
    record.update(end_utc=utc(),seconds=time.monotonic()-start)
    record['raw_logs']=[{'path':str(p.relative_to(ROOT)),'bytes':p.stat().st_size,'sha256':sha(p.read_bytes())} for p in sorted(OUT.glob('[0-9][0-9]-*.log'))]
    if (OUT/'summary.txt').exists():
        p=OUT/'summary.txt'; record['summary']={'path':str(p.relative_to(ROOT)),'bytes':p.stat().st_size,'sha256':sha(p.read_bytes())}
    save()
print(record['status'],code,flush=True);sys.exit(code)
