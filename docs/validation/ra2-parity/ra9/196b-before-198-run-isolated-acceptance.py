"""Required isolated five checks using the same frozen source and existing serial runner."""
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
manifest = ROOT / 'docs/validation/ra2-parity/ra9/195-actual-canonical-input-scope.json'
packet = json.loads(manifest.read_text())
files = {x['path']: x['sha256'] for x in packet['source_files']}
assert len(files) == 656
binding_receipt = json.loads((OUT / '196-actual-root-authorized-binding-refresh.json').read_text())
for row in binding_receipt['files']:
    files[row['path']] = row['after_sha256']

def guard():
    bad = [path for path, digest in files.items() if hashlib.sha256((ROOT / path).read_bytes()).hexdigest() != digest]
    assert not bad, bad
    for a in packet['authority_fragments']:
        b = (ROOT / a['document_path']).read_bytes()
        at = b.index(a['section_start'].encode())
        assert hashlib.sha256(b[at:at+a['span_bytes']]).hexdigest() == a['sha256']
    return {'files': len(files), 'authority_spans': 64, 'mismatches': bad}

def utc():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()

receipt = OUT / '198-isolated-acceptance-process.json'
assert not receipt.exists()
record = {'start_utc': utc(), 'pid': os.getpid(),
    'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
    'source_manifest': str(manifest.relative_to(ROOT)), 'source_manifest_sha256': hashlib.sha256(manifest.read_bytes()).hexdigest(),
    'runner_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    'source_before': guard(), 'status': 'RUNNING', 'commands': [],
    'order': 'Required all21/fixed3/threeguards BEFORE costly fresh full13, under coordinator ordering authorization. Same frozen source guards preserve these proofs through full13 without a duplicate solely for ordering.'}

def save():
    receipt.write_text(json.dumps(record, indent=2) + '\n')

save()
env = os.environ.copy()
assert env.get('CARGO_TARGET_DIR') == '/Users/xingyuwang/Library/Caches/Codex/cargo-target'
env['CARGO_BUILD_JOBS'] = '1'
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
        log = OUT / ('198-' + name + '.log')
        assert not log.exists(), log
        started = time.monotonic()
        with log.open('wb') as output:
            process = subprocess.Popen(command, stdout=output, stderr=subprocess.STDOUT, env=env, start_new_session=True)
            row['pgid'] = process.pid
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
