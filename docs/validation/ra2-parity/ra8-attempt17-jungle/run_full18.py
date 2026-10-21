"""Fresh exact full gate on committed checkpoint18 source, with owned failure cancellation."""
from pathlib import Path
import datetime
import hashlib
import json
import os
import re
import select
import shutil
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[4]
BASE = ROOT / 'docs/validation/ra2-parity'
OUT = BASE / 'ra8'
META = Path(__file__).resolve().parent
os.chdir(ROOT)
assert os.getsid(0) == os.getpid(), 'Launch controller in its own detached session'
assert signal.getsignal(signal.SIGHUP) == signal.SIG_IGN, 'Launch controller through nohup'
manifest = META / 'checkpoint18-source-manifest.json'
files = json.loads(manifest.read_text())['files']
assert len(files) == 495
early_path = BASE / 'ra8-early18/acceptance-process.json'
early = json.loads(early_path.read_text())
assert early['status'] == 'PASSED' and len(early['commands']) == 5
assert all(row['exit'] == 0 and not row['source_after']['mismatches'] for row in early['commands'])
assert not subprocess.check_output(['git', 'status', '--porcelain'], text=True).strip()
assert subprocess.check_output(['git', 'branch', '--show-current'], text=True).strip() == 'ra2-parity'
assert subprocess.check_output(['git', 'rev-parse', 'main'], text=True).strip() == '8230a955b251e9341d349f99eaeb86f7f8253664'

def utc():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()

def file_record(path):
    if not path.exists():
        return {'path': str(path), 'exists': False}
    return {'path': str(path), 'exists': True, 'bytes': path.stat().st_size,
            'sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'mtime_ns': path.stat().st_mtime_ns}

def source_guard():
    bad = [path for path, digest in files.items() if hashlib.sha256((ROOT / path).read_bytes()).hexdigest() != digest]
    assert not bad, bad
    return {'files': len(files), 'mismatches': bad}

# A fresh check of every original capture precedes any canonical-label overwrite.
preservation = []
for current_name, archive_name in [('ra8', 'ra8-attempt17-interrupted'), ('ra8-recovery17', 'ra8-recovery17-failed')]:
    archive = BASE / archive_name
    capture = json.loads((archive / 'PRESERVED.json').read_text())
    for row in capture['files']:
        current = BASE / current_name / row['path']
        stored = archive / row['path']
        assert file_record(current)['sha256'] == row['sha256']
        assert file_record(stored)['sha256'] == row['sha256']
    preservation.append({'current': current_name, 'archive': archive_name,
                         'files': len(capture['files']), 'bytes': capture['total_bytes'],
                         'preserved_sha256': file_record(archive / 'PRESERVED.json')['sha256']})

env = os.environ.copy()
assert env.get('CARGO_TARGET_DIR') == '/Users/xingyuwang/Library/Caches/Codex/cargo-target'
for key in ['RA2_DUEL_SEED', 'RA2_DUEL_PAIR']:
    env.pop(key, None)
env['RUST_TEST_NOCAPTURE'] = '1'
native_table = Path(env['CARGO_TARGET_DIR']) / 'tmp/m6_production_matches_lengths.tsv'
record = {'attempt': 18, 'checkpoint': 18, 'start_utc': utc(), 'pid': os.getpid(),
    'controller_session': os.getsid(0), 'controller_process_group': os.getpgrp(),
    'controller_sighup_ignored': signal.getsignal(signal.SIGHUP) == signal.SIG_IGN,
    'controller_launch': 'nohup python3 in a new session, stdin detached, stdout/stderr in committed repository controller log',
    'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
    'runtime_commit': '4a103fa836f596af70a222f402fbea1f56bb0942',
    'main': subprocess.check_output(['git', 'rev-parse', 'main'], text=True).strip(),
    'clean_before_launch': True, 'source_manifest': file_record(manifest), 'source_before': source_guard(),
    'runner': file_record(Path(__file__).resolve()),
    'early_isolated_acceptance': file_record(early_path), 'preservation': preservation,
    'preexisting_native_production_table': file_record(native_table),
    'configured_caches': {key: env.get(key) for key in ['CARGO_TARGET_DIR', 'UV_CACHE_DIR', 'PIP_CACHE_DIR', 'TMPDIR']},
    'command': ['env', '-u', 'RA2_DUEL_SEED', '-u', 'RA2_DUEL_PAIR', 'RUST_TEST_NOCAPTURE=1', 'sh', 'scripts/validate_ra2_parity.sh', 'ra8'],
    'completed_rows': [], 'status': 'RUNNING',
    'failure_policy': 'Stop only this owned gate process group promptly on concrete build/test failure; preserve partial/failed/unrun scope, never credit an interrupted command. All13 zero remain mandatory on final source.'}
receipt = OUT / 'gate-process.json'

def save():
    temporary = receipt.with_suffix('.json.tmp')
    temporary.write_text(json.dumps(record, indent=2) + '\n')
    temporary.replace(receipt)

def preserve_fresh_diagnostic_table():
    if native_table.exists() and native_table.stat().st_mtime_ns > start_ns:
        destination = OUT / 'checkpoint18-native-production-lengths.tsv'
        shutil.copy2(native_table, destination)
        assert file_record(native_table)['sha256'] == file_record(destination)['sha256']
        record['fresh_native_diagnostic_table'] = {
            'native': file_record(native_table), 'preserved': file_record(destination),
            'scope': 'Raw native diagnostic table; command pass credit depends on completed phase4 exit.'}

save()
started = time.monotonic()
start_ns = time.time_ns()
script = None
stop_reason = None
buffer = b''
try:
    with (OUT / 'gate-process-output.log').open('wb') as output:
        script = subprocess.Popen(['sh', 'scripts/validate_ra2_parity.sh', 'ra8'],
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, env=env, start_new_session=True)
        record.update({'script_pid': script.pid, 'script_pgid': script.pid})
        save()
        while True:
            ready, _, _ = select.select([script.stdout], [], [], 2.0)
            if ready:
                data = os.read(script.stdout.fileno(), 65536)
                if not data:
                    break
                output.write(data)
                output.flush()
                buffer += data
                while b'\n' in buffer:
                    line, buffer = buffer.split(b'\n', 1)
                    match = re.match(rb'^(\d\d) exit (\d+)  (\d+) s  (.*)$', line)
                    if not match:
                        continue
                    number, code, seconds = map(int, match.groups()[:3])
                    row = {'number': number, 'exit': code, 'script_date_seconds': seconds,
                           'line': line.decode(errors='replace'), 'observed_utc': utc(),
                           'wrapper_elapsed_seconds': time.monotonic() - started,
                           'source_after': source_guard()}
                    if number == 4:
                        text = (OUT / '04-test.log').read_text(errors='replace')
                        counts = re.findall(r'^test result:.*? (\d+) passed; (\d+) failed; (\d+) ignored;', text, re.M)
                        row['rust_result_targets'] = len(counts)
                        row['rust_counts'] = [sum(int(c[i]) for c in counts) for i in range(3)]
                        if code == 0:
                            assert native_table.stat().st_mtime_ns > start_ns
                            destination = OUT / 'production-lengths.tsv'
                            shutil.copy2(native_table, destination)
                            assert file_record(native_table)['sha256'] == file_record(destination)['sha256']
                            record['fresh_production_table'] = {'native': file_record(native_table),
                                'preserved': file_record(destination), 'bound_after_successful_phase4': True}
                    record['completed_rows'].append(row)
                    save()
                    print(row['line'], flush=True)
                    if code != 0 and number in (3, 4, 5):
                        stop_reason = {'component': number, 'reason': 'Concrete build/test command failure', 'line': row['line']}
            current = record['completed_rows'][-1]['number'] + 1 if record['completed_rows'] else 1
            if current == 4 and not stop_reason and (OUT / '04-test.log').exists():
                text = (OUT / '04-test.log').read_text(errors='replace')
                failure = re.search(r'(?m)^test [^\n]*\.\.\. FAILED\s*$|^test result: FAILED\.', text)
                if failure:
                    stop_reason = {'component': 4, 'reason': 'Actual Rust test failure observed during running command',
                                   'line': failure.group(0), 'scope': 'Interrupted phase4, not a completed or passed component'}
            if stop_reason and script.poll() is None:
                record['stopped_on_concrete_failure'] = stop_reason
                save()
                os.killpg(script.pid, signal.SIGTERM)
        script_code = script.wait()
    preserve_fresh_diagnostic_table()
    if script_code == 0 and not stop_reason:
        assert [row['number'] for row in record['completed_rows']] == list(range(1, 14))
        assert all(row['exit'] == 0 for row in record['completed_rows'])
    record.update({'script_exit': script_code, 'end_utc': utc(), 'wrapper_seconds': time.monotonic() - started,
                   'source_after': source_guard(), 'status': 'PASSED' if script_code == 0 and not stop_reason else 'FAILED'})
    if stop_reason:
        record['stopped_on_concrete_failure'] = stop_reason
    record['completed_component_count'] = len(record['completed_rows'])
    record['components_without_completion_credit'] = [number for number in range(1, 14)
        if number not in [row['number'] for row in record['completed_rows']]]
    record['summary'] = file_record(OUT / 'summary.txt')
    save()
except BaseException as error:
    if script is not None and script.poll() is None:
        os.killpg(script.pid, signal.SIGTERM)
        script.wait()
    preserve_fresh_diagnostic_table()
    record.update({'status': 'FAILED_WRAPPER', 'end_utc': utc(), 'error': repr(error)})
    save()
    raise
sys.exit(0 if record['status'] == 'PASSED' else 1)
