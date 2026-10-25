"""Capture unfiltered staged hygiene and classify exact immutable raw evidence diagnostics."""
from pathlib import Path
import collections
import datetime
import hashlib
import json
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
label = sys.argv[1]
assert label in ['first', 'final', 'closure-first', 'closure-final']
destination = OUT / ('checkpoint18-evidence-check-' + label + '.json')
assert not destination.exists()
result = subprocess.run(['git', 'diff', '--cached', '--check'], cwd=ROOT, capture_output=True)
diagnostics = []
for path, line_number, message in re.findall(rb'(?m)^([^\n]+?):(\d+): (trailing whitespace\.|new blank line at EOF\.)$', result.stdout):
    relative = path.decode()
    native = relative.startswith('docs/validation/ra2-parity/') and relative.endswith('.log')
    patch = relative.startswith('docs/validation/ra2-parity/') and relative.endswith('.patch')
    assert native or patch, ('Non-raw diagnostic requires correction before commit', relative)
    body = (ROOT / relative).read_bytes()
    staged = subprocess.check_output(['git', 'show', ':' + relative], cwd=ROOT)
    assert body == staged, relative
    line = body.splitlines(keepends=True)[int(line_number) - 1]
    ending = b'\r\n' if line.endswith(b'\r\n') else b'\n' if line.endswith(b'\n') else b''
    diagnostics.append({'path': relative, 'line': int(line_number), 'message': message.decode(),
        'kind': 'native_log' if native else 'raw_patch', 'bytes_unchanged': True,
        'file_sha256': hashlib.sha256(body).hexdigest(),
        'staged_blob_sha256': hashlib.sha256(staged).hexdigest(),
        'line_with_eol_bytes': len(line), 'line_with_eol_sha256': hashlib.sha256(line).hexdigest(),
        'eol_hex': ending.hex(), 'eol_sha256': hashlib.sha256(ending).hexdigest()})
assert result.returncode in [0, 2]
assert (result.returncode == 0) == (len(diagnostics) == 0)
assert not result.stderr
record = {'utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'command': ['git', 'diff', '--cached', '--check'], 'exit': result.returncode,
    'passed': result.returncode == 0, 'stdout': result.stdout.decode(), 'stderr': result.stderr.decode(),
    'stdout_sha256': hashlib.sha256(result.stdout).hexdigest(),
    'staged_count': len(subprocess.check_output(['git', 'diff', '--cached', '--name-only', '-z'], cwd=ROOT).split(b'\0')) - 1,
    'diagnostics': len(diagnostics), 'files': len({row['path'] for row in diagnostics}),
    'classifications': dict(collections.Counter(row['kind'] + ':' + row['message'] for row in diagnostics)),
    'source_data_test_prose_errors': 0, 'raw_bytes_unchanged': True, 'items': diagnostics,
    'scope': 'Nonzero check remains a failure; deliberate raw-evidence preservation is documented separately. No bytes were sanitized or ignored.'}
if label.endswith('final'):
    first_label = 'closure-first' if label.startswith('closure-') else 'first'
    first = json.loads((OUT / ('checkpoint18-evidence-check-' + first_label + '.json')).read_text())
    assert record['exit'] == first['exit'] and record['stdout_sha256'] == first['stdout_sha256']
    record['identical_to_first_unfiltered_check'] = True
destination.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps({key: value for key, value in record.items() if key not in ['stdout', 'items']}, indent=2))
