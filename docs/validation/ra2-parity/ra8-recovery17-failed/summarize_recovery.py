"""Build source-bound component acceptance receipts after recovery completes."""
from pathlib import Path
import csv
import datetime
import hashlib
import json
import math
import re
import statistics

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
BASE = ROOT / 'docs/validation/ra2-parity'

def raw(path):
    return {'path': str(path.relative_to(ROOT)), 'bytes': path.stat().st_size,
            'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}

def distribution(values):
    values = sorted(values)
    return {'count': len(values), 'min': values[0], 'median': statistics.median(values),
            'p90_nearest_rank': values[math.ceil(len(values) * .9) - 1], 'max': values[-1]}

def production(path):
    rows = list(csv.DictReader(path.open(), delimiter='\t'))
    assert len(rows) == 756 and len({r['match'] for r in rows}) == 756
    assert all(int(r['ticks']) <= int(r['limit']) for r in rows)
    return {'raw': raw(path), 'ticks': distribution([int(r['ticks']) for r in rows]),
            'normalized_over_original_map_limit': distribution([int(r['ticks']) / int(r['limit']) for r in rows])}

def duels(path):
    text = path.read_text()
    rows = []
    pattern = r'^(\w+) (\d+)x(\d+): (.+): ended in tick (\d+) \(Won \{ winner: PlayerId\((\d+)\), tick: (\d+) \}\) after ([\d.]+)s$'
    for match in re.finditer(pattern, text, re.M):
        name, width, height, pair, tick, winner, event_tick, elapsed = match.groups()
        assert tick == event_tick
        rows.append({'match': name + ': ' + pair, 'ticks': int(tick), 'winner': int(winner),
                     'original_production_limit': 72000 * max(int(width) + int(height), 128) // 128,
                     'duel_limit': 300000})
    assert len(rows) == 756 and len({r['match'] for r in rows}) == 756
    assert '756 matches, 756 ended' in text and 'NO WINNER' not in text
    return {'raw': raw(path), 'explicit_Won': len(rows), 'non_Won': 0,
            'ticks': distribution([r['ticks'] for r in rows]),
            'normalized_over_original_map_limit': distribution([r['ticks'] / r['original_production_limit'] for r in rows]),
            'normalized_over_duel_limit': distribution([r['ticks'] / r['duel_limit'] for r in rows]),
            'rows': rows}

def duration_ms(text):
    value, unit = re.fullmatch(r'([\d.]+)(ns|µs|us|ms|s)', text).groups()
    return float(value) * {'ns': .000001, 'µs': .001, 'us': .001, 'ms': 1, 's': 1000}[unit]

def tenplayers(path, expected, ending):
    lines = path.read_text().splitlines()
    rows = []
    for i, line in enumerate(lines):
        match = re.match(r'^(\w+): (\d+) x (\d+) cells for 10 players', line)
        if not match:
            continue
        name, width, height = match.groups()
        timing = re.fullmatch(r'  (\d+) ticks: mean (\S+), 99th centile (\S+), longest (\S+), busiest 1000 ticks mean (\S+), (\d+) ticks over 50ms; measured total (\S+), elapsed (\S+)', lines[i + 1])
        ticks, mean, p99, longest, busy, late, total, elapsed = timing.groups()
        built = re.search(r'; (\d+) players built;', lines[i + 2])
        assert int(built[1]) == 10
        outcome = re.search(r'ended in tick (\d+): Won \{ winner: PlayerId\((\d+)\), tick: (\d+) \}', lines[i + 2])
        assert not ending or (outcome and outcome[1] == outcome[3])
        row = {'map': name, 'width': int(width), 'height': int(height), 'measured_ticks': int(ticks),
               'mean_ms': duration_ms(mean), 'p99_ms': duration_ms(p99), 'max_ms': duration_ms(longest),
               'busy1000_mean_ms': duration_ms(busy), 'ticks_over_50ms': int(late), 'builders': 10,
               'measured_seconds': duration_ms(total) / 1000, 'elapsed_seconds': duration_ms(elapsed) / 1000,
               'Won': bool(outcome)}
        assert row['mean_ms'] < 5
        if ending:
            row['end_tick'] = int(outcome[1])
            assert row['end_tick'] <= 400000
        else:
            assert row['width'] == row['height'] == 256
            assert 0 < row['measured_ticks'] <= 24000
            assert row['measured_ticks'] == 24000 or outcome
        rows.append(row)
    assert len(rows) == expected and len({r['map'] for r in rows}) == expected
    result = {'raw': raw(path), 'maps': rows, 'mean_budget_ms': 5,
              'worst_mean_ms': max(r['mean_ms'] for r in rows),
              'worst_tick_ms': max(r['max_ms'] for r in rows),
              'worst_busy1000_mean_ms': max(r['busy1000_mean_ms'] for r in rows)}
    if ending:
        result['end_ticks'] = distribution([r['end_tick'] for r in rows])
        result['end_ticks_over_400000'] = distribution([r['end_tick'] / 400000 for r in rows])
    return result

process = json.loads((OUT / 'recovery-process.json').read_text())
assert process['status'] == 'PASSED' and len(process['commands']) == 6
assert all(row['exit'] == 0 and not row['source_after']['mismatches'] for row in process['commands'])
archive = BASE / 'ra8-attempt17-interrupted'
preserved = json.loads((archive / 'PRESERVED.json').read_text())
for row in preserved['files']:
    assert raw(archive / row['path'])['sha256'] == row['sha256']
    assert raw(BASE / 'ra8' / row['path'])['sha256'] == row['sha256']
original = (archive / 'summary.txt').read_text()
recovered = (OUT / 'summary.txt').read_text()
partial_pattern = r'^(.*): ended in tick (\d+) \(Won \{ winner: PlayerId\((\d+)\)'
partial = {name: (tick, winner) for name, tick, winner in re.findall(partial_pattern, (archive / '10-duels.log').read_text(), re.M)}
complete = {name: (tick, winner) for name, tick, winner in re.findall(partial_pattern, (OUT / '10-duels.log').read_text(), re.M)}
assert len(partial) == 692 and len(complete) == 756
assert all(complete.get(name) == result for name, result in partial.items())
rows = {}
for line in original.splitlines():
    match = re.match(r'^(\d+) exit (\d+) ', line)
    if match and int(match[1]) not in (10, 11):
        assert match[2] == '0'
        rows[int(match[1])] = {'line': line, 'source': 'ra8-attempt17-interrupted/summary.txt'}
for line in recovered.splitlines():
    match = re.match(r'^(\d+) exit (\d+) ', line)
    if match:
        assert int(match[1]) in (10, 11) and match[2] == '0'
        rows[int(match[1])] = {'line': line, 'source': 'ra8-recovery17/summary.txt'}
assert sorted(rows) == list(range(1, 14))
guards = []
for name in ['m13', 'm14', 'soak']:
    path = OUT / f'postgate-{name}-guard.log'
    assert re.search(r'test result: ok\. 1 passed; 0 failed; 0 ignored;', path.read_text())
    guards.append(raw(path))
report = {'utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
          'runtime_commit': process['runtime_commit'], 'source_head': process['head'],
          'scope': '13 successful source-bound command components, combining original1-9/12/13 with recovery10/11. Original invocation FAILED; this is not a new single all13 invocation. Five isolated postgate commands follow successful component recovery.',
          'source_guards': {'unique_files': 495, 'all_before_after_mismatches': []},
          'original_attempt': {'summary': raw(archive / 'summary.txt'), 'archive_leaves': len(preserved['files']),
                               'archive_bytes': preserved['total_bytes'], 'unchanged_current_archive_pairs': len(preserved['files']),
                               'phase10_exit': 137, 'explicit_Won_before_interruption': (archive / '10-duels.log').read_text().count('(Won {'),
                               'partial_recovery_same_tick_and_winner': len(partial),
                               'phase11_exit': 1, 'smoke_frames': '503/600'},
          'components': rows, 'workspace': json.loads((archive / 'checkpoint17-workspace.json').read_text()),
          'soak': json.loads((archive / 'checkpoint17-soak.json').read_text()),
          'production_before_RA8': production(BASE / 'ra7/production-lengths.tsv'),
          'production_after_RA8': production(archive / 'checkpoint17-m6-production-lengths.tsv'),
          'duels_before_RA8': duels(BASE / 'ra7/10-duels.log'),
          'duels_after_RA8': duels(OUT / '10-duels.log'),
          'isolated_all21': tenplayers(OUT / 'postgate-all21.log', 21, True),
          'isolated_fixed3': tenplayers(OUT / 'postgate-fixed3.log', 3, False), 'example_guards': guards,
          'timing_qualification': 'Original workspace/soak and earlier R189 replay overlapped authorized RA9 copy work; no isolated timing credit. Recovery postgate commands are serial under exclusive canonical/Cargo/native ownership. Long individual ticks remain reported.'}
(OUT / 'closure-metrics.json').write_text(json.dumps(report, indent=2) + '\n')
(OUT / 'component-summary.txt').write_text('\n'.join(rows[i]['line'] + '  [evidence: ' + rows[i]['source'] + ']' for i in range(1, 14)) + '\nSource-bound components PASSED; original all13 invocation FAILED and retained.\nPostgate: all21/fixed3/three guards PASSED.\n')
print(json.dumps({key: value for key, value in report.items() if key in ('source_guards', 'original_attempt')}, indent=2))
for key in ['production_before_RA8', 'production_after_RA8', 'duels_before_RA8', 'duels_after_RA8']:
    print(key, report[key]['ticks'], report[key]['normalized_over_original_map_limit'])
print('all21', report['isolated_all21']['end_ticks'], 'worstmean', report['isolated_all21']['worst_mean_ms'])
print('fixed3', [(r['map'], r['mean_ms']) for r in report['isolated_fixed3']['maps']])
