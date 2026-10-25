"""Run the five required isolated checks before the expensive unchanged-source full gate."""
from pathlib import Path
import datetime
import hashlib
import json
import os
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
os.chdir(ROOT)
manifest = ROOT / 'docs/validation/ra2-parity/ra8-attempt17-jungle/checkpoint18-source-manifest.json'
files = json.loads(manifest.read_text())['files']
assert len(files) == 495

def guard():
    bad = [path for path, digest in files.items() if hashlib.sha256((ROOT / path).read_bytes()).hexdigest() != digest]
    assert not bad, bad
    return {'files': len(files), 'mismatches': bad}

def utc():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()

receipt = OUT / 'acceptance-process.json'
assert not receipt.exists()
record = {'start_utc': utc(), 'pid': os.getpid(),
    'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
    'source_manifest': str(manifest.relative_to(ROOT)), 'source_manifest_sha256': hashlib.sha256(manifest.read_bytes()).hexdigest(),
    'runner_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    'source_before': guard(), 'status': 'RUNNING', 'commands': [],
    'order': 'Coordinator-directed required isolated acceptance before expensive freshall13 after actual latepostgate failure. All13 still mandatory; same-source guards carry these isolated proofs through full13 without a duplicate for ordering alone.'}

def save():
    receipt.write_text(json.dumps(record, indent=2) + '\n')

save()
env = os.environ.copy()
env['CARGO_TARGET_DIR'] = '/Users/xingyuwang/Library/Caches/Codex/cargo-target'
for key in ['RA2_DUEL_SEED', 'RA2_DUEL_PAIR']:
    env.pop(key, None)
awake = subprocess.Popen(['caffeinate', '-i', '-w', str(os.getpid())])
commands = [
    ('all21', ['cargo', 'run', '--release', '-p', 'sim', '--example', 'm13_ten_players', '--', 'to-the-end', 'all']),
    ('fixed3', ['cargo', 'run', '--release', '-p', 'sim', '--example', 'm13_ten_players', '--', 'liberty_bay', 'continental_divide', 'pacific_atoll']),
    ('guard-m13', ['cargo', 'test', '--release', '-p', 'sim', '--example', 'm13_ten_players', '--', '--nocapture', '--test-threads=1']),
    ('guard-m14', ['cargo', 'test', '--release', '-p', 'sim', '--example', 'm14_duels', '--', '--nocapture', '--test-threads=1']),
    ('guard-soak', ['cargo', 'test', '--release', '-p', 'sim', '--example', 'm7_soak', '--', '--nocapture', '--test-threads=1']),
]
try:
    for name, command in commands:
        row = {'name': name, 'command': command, 'start_utc': utc(), 'source_before': guard(), 'status': 'RUNNING'}
        record['commands'].append(row)
        save()
        log = OUT / (name + '.log')
        assert not log.exists(), log
        started = time.monotonic()
        with log.open('wb') as output:
            process = subprocess.Popen(command, stdout=output, stderr=subprocess.STDOUT, env=env)
            row['pid'] = process.pid
            save()
            code = process.wait()
        row.update({'exit': code, 'end_utc': utc(), 'seconds': time.monotonic() - started, 'source_after': guard(),
            'status': 'PASS' if code == 0 else 'FAIL', 'raw': {'path': str(log.relative_to(ROOT)),
            'bytes': log.stat().st_size, 'sha256': hashlib.sha256(log.read_bytes()).hexdigest()}})
        save()
        print(name, code, row['seconds'], flush=True)
        if code:
            record['status'] = 'FAILED'
            break
    else:
        record['status'] = 'PASSED'
    record.update({'end_utc': utc(), 'source_after': guard()})
    save()
finally:
    awake.terminate()
sys.exit(0 if record['status'] == 'PASSED' else 1)
