"""Preserve failed postgate collection, then replay only its actual failing map."""
from pathlib import Path
import datetime
import hashlib
import json
import os
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
BASE = ROOT / 'docs/validation/ra2-parity'
os.chdir(ROOT)
prior = BASE / 'ra8-recovery17'
process = json.loads((prior / 'recovery-process.json').read_text())
assert process['status'] == 'FAILED'
assert len(process['commands']) == 2 and process['commands'][-1]['exit'] == 1
archive = BASE / 'ra8-recovery17-failed'
assert not archive.exists()
archive.mkdir()
preserved = []
for path in sorted(prior.rglob('*')):
    if path.is_file():
        relative = path.relative_to(prior)
        target = archive / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, target)
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        assert hashlib.sha256(target.read_bytes()).hexdigest() == digest
        preserved.append({'path': str(relative), 'bytes': path.stat().st_size, 'sha256': digest})
(archive / 'PRESERVED.json').write_text(json.dumps({'utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'source': str(prior), 'files': preserved, 'total_bytes': sum(row['bytes'] for row in preserved),
    'scope': 'Successful recovery10/11 followed by actual all21 ending failure. Fixed3 and three example guards unrun. No RA8 closure.'}, indent=2) + '\n')
files = {}
for name in ['attempt16-final2-runtime-manifest.json', 'attempt16-final2-test-example-manifest.json']:
    for path, digest in json.loads((BASE / 'ra8-preflight' / name).read_text())['files'].items():
        assert path not in files or files[path] == digest
        files[path] = digest
assert len(files) == 495

def guard():
    bad = [path for path, digest in files.items() if hashlib.sha256((ROOT / path).read_bytes()).hexdigest() != digest]
    assert not bad, bad
    return {'unique_files': len(files), 'mismatches': bad}

command = ['cargo', 'run', '--release', '-p', 'sim', '--example', 'm13_ten_players', '--', 'to-the-end', 'trace', 'jungle_delta']
record = {'command': command, 'utc_start': datetime.datetime.now(datetime.timezone.utc).isoformat(),
          'source_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
          'runtime_commit': 'eb308a231e302048c9dc8d1e8b9543b7c5f64ef7', 'source_before': guard(),
          'status': 'RUNNING', 'purpose': 'One unchanged exact seed13/ten normal Medium players replay with existing observational trace. Functional diagnosis only; not isolated performance evidence.',
          'preservation': str((archive / 'PRESERVED.json').relative_to(ROOT))}
receipt = OUT / 'trace1.json'
assert not receipt.exists()
receipt.write_text(json.dumps(record, indent=2) + '\n')
env = os.environ.copy()
env['CARGO_TARGET_DIR'] = '/Users/xingyuwang/Library/Caches/Codex/cargo-target'
for name in ['RA2_DUEL_SEED', 'RA2_DUEL_PAIR']:
    env.pop(name, None)
log = OUT / 'trace1.log'
assert not log.exists()
started = time.monotonic()
with log.open('wb') as output:
    child = subprocess.Popen(command, stdout=output, stderr=subprocess.STDOUT, env=env)
    record['pid'] = child.pid
    receipt.write_text(json.dumps(record, indent=2) + '\n')
    code = child.wait()
record.update({'exit': code, 'seconds': time.monotonic() - started, 'utc_end': datetime.datetime.now(datetime.timezone.utc).isoformat(),
               'source_after': guard(), 'status': 'COMPLETED', 'raw': {'path': str(log.relative_to(ROOT)),
               'bytes': log.stat().st_size, 'sha256': hashlib.sha256(log.read_bytes()).hexdigest()}})
receipt.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(record, indent=2))
