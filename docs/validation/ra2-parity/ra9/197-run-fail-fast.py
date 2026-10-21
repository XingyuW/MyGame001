"""Bounded canonical fail-fast checks using the existing serial source-guarded runner."""
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

receipt = OUT / '197-fail-fast-process.json'
assert not receipt.exists()
record = {'start_utc': utc(), 'pid': os.getpid(),
    'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
    'source_manifest': str(manifest.relative_to(ROOT)), 'source_manifest_sha256': hashlib.sha256(manifest.read_bytes()).hexdigest(),
    'runner_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    'source_before': guard(), 'status': 'RUNNING', 'commands': [],
    'order': 'Root-directed strict compilation, actual whole-positive, exactly six repaired cases, and the one unexecuted difficulty regression before isolated5 and mandatory fresh full13. No full181 target or debug/workspace native sweep.'}

def save():
    receipt.write_text(json.dumps(record, indent=2) + '\n')

save()
env = os.environ.copy()
assert env.get('CARGO_TARGET_DIR') == '/Users/xingyuwang/Library/Caches/Codex/cargo-target'
env['CARGO_BUILD_JOBS'] = '1'
for key in ['RA2_DUEL_SEED', 'RA2_DUEL_PAIR']:
    env.pop(key, None)
awake = subprocess.Popen(['caffeinate', '-i', '-w', str(os.getpid())])
commands = [('fmt', ['cargo', 'fmt', '--all', '--', '--check']), ('clippy', ['cargo', 'clippy', '--workspace', '--all-targets', '--', '-D', 'warnings']), ('whole-positive', ['cargo', 'test', '--release', '-p', 'sim', '--test', 'ra9_data_parity', 'every_physical_consumer_and_reference_atom_has_an_exact_successful_disposition', '--', '--nocapture']), ('six-repaired', ['cargo', 'test', '--release', '-p', 'sim', '--test', 'ra9_data_parity', '--no-fail-fast', '--', '--nocapture', '--exact', 'ra9_audit::country_rosters::tests::ra9_raw_roster_nonempty_is_schema_rejection_not_fake_loaded_empty', 'ra9_audit::consumer62_admission::tests::ra9_consumer62_nonfirst_pad_dock_count_fails_real_dock_adapter', 'ra9_audit::model162_admission::tests::ra9_model162_actual_gap_radius_change_fails_native_comparison_and_no_boost_policy', 'ra9_audit::source_disposition::tests::ra9_static_speed_rejects_case_alias_after_actual_adapter_control', 'ra9_audit::source_disposition::tests::ra9_static_speed_rejects_changed_real_instant_selector', 'ra9_audit::source_disposition::tests::ra9_static_speed_rejects_demanded_actual_source_overlap']), ('difficulty', ['cargo', 'test', '--release', '-p', 'sim', '--test', 'ra8_ai', 'ra9_difficulty_neutral_cost_ground_and_air_motion_use_actual_same_rules', '--', '--exact', '--nocapture'])]
try:
    for name, command in commands:
        row = {'name': name, 'command': command, 'start_utc': utc(), 'source_before': guard(), 'status': 'RUNNING'}
        record['commands'].append(row)
        save()
        log = OUT / ('197-' + name + '.log')
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
