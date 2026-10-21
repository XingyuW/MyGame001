"""Recover environmentally interrupted checks, retaining original failed evidence."""
from pathlib import Path
import datetime
import hashlib
import json
import os
import subprocess
import time

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
PREFLIGHT = ROOT / 'docs/validation/ra2-parity/ra8-preflight'
os.chdir(ROOT)
files = {}
for name in ['attempt16-final2-runtime-manifest.json', 'attempt16-final2-test-example-manifest.json']:
    for path, digest in json.loads((PREFLIGHT / name).read_text())['files'].items():
        assert path not in files or files[path] == digest
        files[path] = digest
assert len(files) == 495

def guard():
    mismatches = [path for path, digest in files.items()
                  if hashlib.sha256((ROOT / path).read_bytes()).hexdigest() != digest]
    assert not mismatches, mismatches
    return {'files': len(files), 'mismatches': mismatches}

def utc():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()

env = os.environ.copy()
for key in ['RA2_DUEL_SEED', 'RA2_DUEL_PAIR']:
    env.pop(key, None)
env['CARGO_TARGET_DIR'] = '/Users/xingyuwang/Library/Caches/Codex/cargo-target'
env['UV_CACHE_DIR'] = '/Users/xingyuwang/Library/Caches/Codex/uv-cache'
env['PIP_CACHE_DIR'] = '/Users/xingyuwang/Library/Caches/Codex/pip-cache'
record = {'start_utc': utc(), 'pid': os.getpid(),
          'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
          'runtime_commit': 'eb308a231e302048c9dc8d1e8b9543b7c5f64ef7',
          'scope': 'Component recovery10/11, then unchanged-source isolated postgate5. Original failed all13 invocation remains failed. No fresh all13 invocation claimed.',
          'source_before': guard(), 'commands': [], 'status': 'RUNNING'}

def save():
    (OUT / 'recovery-process.json').write_text(json.dumps(record, indent=2) + '\n')

save()
awake = subprocess.Popen(['caffeinate', '-d', '-i', '-w', str(os.getpid())])
commands = [
    ('components-10-11', ['sh', 'scripts/validate_ra2_parity.sh', 'ra8-recovery17', '10', '11']),
    ('postgate-all21', ['cargo', 'run', '--release', '-p', 'sim', '--example', 'm13_ten_players', '--', 'to-the-end', 'all']),
    ('postgate-fixed3', ['cargo', 'run', '--release', '-p', 'sim', '--example', 'm13_ten_players', '--', 'liberty_bay', 'continental_divide', 'pacific_atoll']),
    ('postgate-m13-guard', ['cargo', 'test', '--release', '-p', 'sim', '--example', 'm13_ten_players']),
    ('postgate-m14-guard', ['cargo', 'test', '--release', '-p', 'sim', '--example', 'm14_duels']),
    ('postgate-soak-guard', ['cargo', 'test', '--release', '-p', 'sim', '--example', 'm7_soak']),
]
try:
    for name, command in commands:
        before = guard()
        row = {'name': name, 'command': command, 'start_utc': utc(), 'source_before': before, 'status': 'RUNNING'}
        record['commands'].append(row)
        save()
        started = time.monotonic()
        path = OUT / (name + '.log')
        assert not path.exists(), path
        with path.open('wb') as log:
            child = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT, env=env)
            row['pid'] = child.pid
            save()
            code = child.wait()
        row.update({'exit': code, 'seconds': time.monotonic() - started, 'end_utc': utc(),
                    'source_after': guard(), 'status': 'PASS' if code == 0 else 'FAIL',
                    'raw': {'path': str(path.relative_to(ROOT)), 'bytes': path.stat().st_size,
                            'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}})
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
