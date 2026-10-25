"""Require fresh full13 and same-source isolated acceptance before writing closure metrics."""
from pathlib import Path
import csv
import datetime
import hashlib
import json
import math
import re
import statistics

ROOT = Path(__file__).resolve().parents[4]
BASE = ROOT / 'docs/validation/ra2-parity'
OUT = BASE / 'ra8'
META = Path(__file__).resolve().parent

def raw(path):
    return {'path': str(path.relative_to(ROOT)), 'bytes': path.stat().st_size,
            'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}

def distribution(values):
    values = sorted(values)
    return {'count': len(values), 'min': values[0], 'median': statistics.median(values),
            'p90_nearest_rank': values[math.ceil(len(values) * .9) - 1], 'max': values[-1]}

def production(path):
    rows = list(csv.DictReader(path.open(), delimiter='\t'))
    assert len(rows) == 756 and len({row['match'] for row in rows}) == 756
    assert all(int(row['ticks']) <= int(row['limit']) for row in rows)
    return {'raw': raw(path), 'ticks': distribution([int(row['ticks']) for row in rows]),
        'normalized_over_original_map_limit': distribution([int(row['ticks']) / int(row['limit']) for row in rows]),
        'rows': rows}

def duels(path):
    text = path.read_text()
    rows = []
    pattern = (r'^(\w+) (\d+)x(\d+): (.+): ended in tick (\d+) '
        r'\(Won \{ winner: PlayerId\((\d+)\), tick: (\d+) \}\) after ([\d.]+)s$')
    for match in re.finditer(pattern, text, re.M):
        name, width, height, pair, tick, winner, event_tick, elapsed = match.groups()
        assert tick == event_tick
        rows.append({'match': name + ': ' + pair, 'ticks': int(tick), 'winner': int(winner),
            'original_production_limit': 72000 * max(int(width) + int(height), 128) // 128,
            'duel_limit': 300000, 'rounded_seconds': float(elapsed)})
    assert len(rows) == 756 and len({row['match'] for row in rows}) == 756
    assert '756 matches, 756 ended' in text and 'NO WINNER' not in text
    assert all(row['ticks'] < 300000 for row in rows)
    return {'raw': raw(path), 'explicit_Won': len(rows), 'non_Won': 0,
        'ticks': distribution([row['ticks'] for row in rows]),
        'normalized_over_original_map_limit': distribution([row['ticks'] / row['original_production_limit'] for row in rows]),
        'normalized_over_duel_limit': distribution([row['ticks'] / 300000 for row in rows]), 'rows': rows}

process = json.loads((OUT / 'gate-process.json').read_text())
assert process['attempt'] == 18 and process['status'] == 'PASSED' and process['script_exit'] == 0
assert [row['number'] for row in process['completed_rows']] == list(range(1, 14))
assert all(row['exit'] == 0 and not row['source_after']['mismatches'] for row in process['completed_rows'])
assert not process['source_after']['mismatches']
manifest_path = META / 'checkpoint18-source-manifest.json'
files = json.loads(manifest_path.read_text())['files']
assert len(files) == 495
assert all(hashlib.sha256((ROOT / path).read_bytes()).hexdigest() == digest for path, digest in files.items())
early_path = BASE / 'ra8-early18/acceptance-summary.json'
early = json.loads(early_path.read_text())
assert early['runtime_commit'] == process['runtime_commit']
assert early['source_manifest']['sha256'] == raw(manifest_path)['sha256']
assert len(early['all21']['maps']) == 21 and all(row['won'] for row in early['all21']['maps'])
assert len(early['fixed3']['maps']) == 3 and len(early['guards']) == 3
assert process['fresh_production_table']['preserved']['sha256'] == raw(OUT / 'production-lengths.tsv')['sha256']
workspace_path = OUT / '04-test.log'
workspace = workspace_path.read_text()
counts = re.findall(r'^test result:.*? (\d+) passed; (\d+) failed; (\d+) ignored;', workspace, re.M)
assert counts and all(failed == ignored == '0' for _, failed, ignored in counts)
assert 'M6 production progress completed756/756 non_Won0' in workspace
assert 'test computer_players_finish_matches_with_the_shipped_data_on_every_shipped_map ... ok' in workspace
benchmark_path = OUT / '08-benchmark.log'
benchmark = benchmark_path.read_text()
benchmark_rows = re.findall(r'(?m)^(fixture|shipped): (\d+) units spawned; mean tick ([\d.]+) ms, longest tick ([\d.]+) ms; mean over the (\d+) ticks with at least 150 units alive ([\d.]+) ms;', benchmark)
assert len(benchmark_rows) == 2 and {row[0] for row in benchmark_rows} == {'fixture', 'shipped'}
assert all(row[1] == '200' and float(row[2]) <= 5 and float(row[3]) <= 50 for row in benchmark_rows)
assert '2000 measured ticks per scenario' in benchmark and 'm7 benchmark: passed' in benchmark
benchmark_heap = re.search(r'peak heap in use: ([\d.]+) MiB \(budget (\d+) MiB\)', benchmark)
assert benchmark_heap and float(benchmark_heap[1]) < int(benchmark_heap[2]) == 1024
soak_path = OUT / '09-soak.log'
soak = soak_path.read_text()
configurations = re.findall(r'(?m)^(.*): (\d+) ticks, (\d+) matches \((\d+) ended, the last (still running|ended)\), ([\d.]+) s for both runs$', soak)
assert len(configurations) == 260 and len({row[0] for row in configurations}) == 260
assert all(row[1] == '36000' for row in configurations)
total = re.search(r'm7 soak: (\d+) ticks in ([\d.]+) s; peak heap in use ([\d.]+) MiB \(budget (\d+) MiB\)', soak)
assert total and int(total[1]) == 18720000 and 'm7 soak: passed' in soak
assert float(total[3]) < int(total[4]) == 1024
current_production = production(OUT / 'production-lengths.tsv')
baseline_production = production(BASE / 'ra7/production-lengths.tsv')
assert {row['match']: row['limit'] for row in current_production['rows']} == {row['match']: row['limit'] for row in baseline_production['rows']}
record = {'utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'status': 'All thirteen fresh command exits zero plus five isolated required acceptance commands on unchanged frozen source',
    'runtime_commit': process['runtime_commit'], 'gate_launch_head': process['head'],
    'source_manifest': raw(manifest_path), 'source_after': {'files': len(files), 'mismatches': []},
    'gate_process': raw(OUT / 'gate-process.json'), 'summary': raw(OUT / 'summary.txt'),
    'commands': process['completed_rows'],
    'workspace': {'raw': raw(workspace_path), 'result_targets': len(counts),
        'passed': sum(int(row[0]) for row in counts), 'failed': 0, 'ignored': 0},
    'benchmark': {'raw': raw(benchmark_path), 'measured_ticks_each_scenario': 2000,
        'original_mean_budget_ms': 5, 'original_longest_tick_budget_ms': 50,
        'peak_heap_mib': float(benchmark_heap[1]), 'heap_budget_mib': int(benchmark_heap[2]),
        'scenarios': [{'name': row[0], 'spawned_units': int(row[1]), 'mean_ms': float(row[2]),
            'max_ms': float(row[3]), 'ticks_with_at_least_150_alive': int(row[4]),
            'mean_with_at_least_150_alive_ms': float(row[5])} for row in benchmark_rows]},
    'production': current_production, 'production_baseline_ra7': baseline_production,
    'duels': duels(OUT / '10-duels.log'), 'duels_baseline_ra7': duels(BASE / 'ra7/10-duels.log'),
    'soak': {'raw': raw(soak_path), 'configurations': len(configurations), 'repeats': 2,
        'ticks_each_repeat': 36000, 'total_ticks': int(total[1]), 'seconds': float(total[2]),
        'peak_heap_mib': float(total[3]), 'heap_budget_mib': int(total[4]),
        'native_ended_matches_per_repeat': sum(int(row[3]) for row in configurations),
        'native_final_worlds_still_running_per_repeat': sum(row[4] == 'still running' for row in configurations),
        'scope': 'All fixed-duration configurations passed determinism/invariants/winner guards for completed games; no assertion that every final world ended.'},
    'isolated_acceptance': raw(early_path),
    'length_scope': 'Median and nearest-rank p90; M6 table world tick can be one greater than Won event tick. Duel original-map normalization uses unchanged M6 limits; duel fixed cap remains300000. No claim that every tail improves.',
    'timing_scope': 'Only final-source canonical isolated acceptance is credited for ten-player performance; RA9 preliminary copy timing is excluded.'}
destination = OUT / 'checkpoint18-closure-metrics.json'
lease_path = OUT / 'checkpoint18-functional-overlap-lease.json'
if lease_path.exists():
    lease = json.loads(lease_path.read_text())
    assert lease['status'] == 'CLOSED' and lease['confirmed_no_copy_native_cargo']
    phase4 = next(row for row in process['completed_rows'] if row['number'] == 4)
    assert datetime.datetime.fromisoformat(lease['closed_utc']) < datetime.datetime.fromisoformat(phase4['observed_utc'])
    record['bounded_copy_functional_lease'] = raw(lease_path)
    record['workspace']['elapsed_scope'] = 'Functional validation only: bounded RA9 copy checks overlapped part of phase04. All copy Cargo/native stopped before phase04 completed, before benchmark/soak/duels. No workspace/copy elapsed timing is performance evidence.'
assert not destination.exists()
destination.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps({key: record[key] for key in ['runtime_commit', 'workspace', 'soak']}, indent=2))
print(json.dumps({key: {name: record[key][name] for name in ['ticks', 'normalized_over_original_map_limit']}
    for key in ['production', 'production_baseline_ra7', 'duels', 'duels_baseline_ra7']}, indent=2))
