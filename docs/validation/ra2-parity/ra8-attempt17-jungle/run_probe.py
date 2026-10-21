"""Run one named targeted command with complete before/after frozen-source receipts."""
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
name, *command = sys.argv[1:]
assert name and command
receipt = OUT / (name + '.json')
log = OUT / (name + '.log')
manifest = OUT / (name + '-manifest.json')
assert not any(path.exists() for path in [receipt, log, manifest])
files = {}
for previous in ['attempt16-final2-runtime-manifest.json', 'attempt16-final2-test-example-manifest.json']:
    files.update(json.loads((ROOT / 'docs/validation/ra2-parity/ra8-preflight' / previous).read_text())['files'])
snapshot = {path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() for path in files}
manifest.write_text(json.dumps({'files': snapshot, 'unique': len(snapshot),
    'changes_from_checkpoint17': [path for path in files if files[path] != snapshot[path]]}, indent=2) + '\n')
record = {'name': name, 'command': command, 'source_manifest': str(manifest.relative_to(ROOT)),
    'manifest_sha256': hashlib.sha256(manifest.read_bytes()).hexdigest(),
    'start_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
    'status': 'RUNNING', 'source_files': len(snapshot)}
receipt.write_text(json.dumps(record, indent=2) + '\n')
env = os.environ.copy()
env['CARGO_TARGET_DIR'] = '/Users/xingyuwang/Library/Caches/Codex/cargo-target'
for key in ['RA2_DUEL_SEED', 'RA2_DUEL_PAIR']:
    env.pop(key, None)
started = time.monotonic()
with log.open('wb') as output:
    process = subprocess.Popen(command, stdout=output, stderr=subprocess.STDOUT, env=env)
    record['pid'] = process.pid
    receipt.write_text(json.dumps(record, indent=2) + '\n')
    code = process.wait()
bad = [path for path, digest in snapshot.items() if hashlib.sha256((ROOT / path).read_bytes()).hexdigest() != digest]
record.update({'exit': code, 'seconds': time.monotonic() - started,
    'end_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'status': 'COMPLETED',
    'source_mismatches': bad, 'raw': {'path': str(log.relative_to(ROOT)), 'bytes': log.stat().st_size,
    'sha256': hashlib.sha256(log.read_bytes()).hexdigest()}})
receipt.write_text(json.dumps(record, indent=2) + '\n')
assert not bad, bad
print(json.dumps(record, indent=2))
