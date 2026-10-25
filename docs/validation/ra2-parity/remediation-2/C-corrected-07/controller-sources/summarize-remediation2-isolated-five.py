from pathlib import Path
import datetime,hashlib,json,math,re,statistics,sys
ROOT=Path('/Users/xingyuwang/GameProjects/MyGame001');OUT=ROOT/sys.argv[1];EXPECTED=int(sys.argv[2])
process_path=OUT/'process.json';process=json.loads(process_path.read_bytes())
assert process['status']=='COMPLETED_WITH_ACTUAL_EXITS_NOT_AUTOMATIC_PASS' and len(process['commands'])==5
assert all(r['exit']==0 and r['source_unchanged'] for r in process['commands']) and process['source_unchanged']
manifest_path=Path(process['frozen_manifest']['path']);packet=json.loads(manifest_path.read_bytes());manifest={r['path']:r['sha256'] for r in packet['source_files']};assert len(manifest)==EXPECTED
assert all(hashlib.sha256((ROOT/n).read_bytes()).hexdigest()==h for n,h in manifest.items())
for a in packet['authority_fragments']:
 b=(ROOT/a['document_path']).read_bytes();at=b.index(a['section_start'].encode());assert hashlib.sha256(b[at:at+a['span_bytes']]).hexdigest()==a['sha256']
def raw(path):
    return {'path': str(path.relative_to(ROOT)), 'bytes': path.stat().st_size,
            'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}

def milliseconds(value):
    match = re.fullmatch(r'([\d.]+)(ns|µs|ms|s)', value)
    assert match, value
    return float(match[1]) * {'ns': 0.000001, 'µs': 0.001, 'ms': 1, 's': 1000}[match[2]]

def parse_maps(name):
    path = ROOT / next(r['raw'] for r in process['commands'] if Path(r['raw']).name.endswith('-'+name+'.log'))
    text = path.read_text()
    pattern = (r'^([a-z_]+): (\d+) x (\d+) cells for 10 players \(file (\d+) x (\d+)\).*\n'
        r'  (\d+) ticks: mean (\S+), 99th centile (\S+), longest (\S+), busiest 1000 ticks mean (\S+), '
        r'(\d+) ticks over 50ms; measured total (\S+), elapsed (\S+)\n'
        r'  at most (\d+) units and (\d+) buildings; (\d+) players built; (.*)$')
    rows = []
    for match in re.finditer(pattern, text, re.M):
        values = match.groups()
        winner = re.fullmatch(r'ended in tick (\d+): Won \{ winner: PlayerId\((\d+)\), tick: (\d+) \}', values[16])
        row = {'map': values[0], 'width': int(values[1]), 'height': int(values[2]),
            'file_width': int(values[3]), 'file_height': int(values[4]), 'schedules': int(values[5]),
            'mean_ms': milliseconds(values[6]), 'p99_ms': milliseconds(values[7]),
            'max_ms': milliseconds(values[8]), 'busiest_1000_mean_ms': milliseconds(values[9]),
            'ticks_over_50ms': int(values[10]), 'measured_seconds': milliseconds(values[11]) / 1000,
            'elapsed_seconds': milliseconds(values[12]) / 1000,
            'peak_units': int(values[13]), 'peak_buildings': int(values[14]),
            'builders': int(values[15]), 'ending': values[16], 'won': winner is not None}
        if winner:
            assert winner[1] == winner[3]
            row.update({'won_tick': int(winner[1]), 'winner': int(winner[2])})
            assert row['schedules'] == row['won_tick'] + 1
        assert row['builders'] == 10 and row['mean_ms'] < 5
        rows.append(row)
    assert len({row['map'] for row in rows}) == len(rows)
    return {'raw': raw(path), 'maps': rows}

all21 = parse_maps('all21-to-won')
assert len(all21['maps']) == 21 and all(row['won'] for row in all21['maps'])
assert all(row['won_tick'] < 400000 for row in all21['maps'])
ticks = sorted(row['won_tick'] for row in all21['maps'])
all21['won_tick_distribution'] = {'min': min(ticks), 'median': statistics.median(ticks),
    'p90_nearest_rank': ticks[math.ceil(len(ticks) * 0.9) - 1], 'max': max(ticks)}
all21['original_cap'] = 400000
all21['won_ticks_over_original_cap'] = {key: value / 400000
    for key, value in all21['won_tick_distribution'].items()}
fixed3 = parse_maps('fixed-three-windows')
assert [row['map'] for row in fixed3['maps']] == ['liberty_bay', 'continental_divide', 'pacific_atoll']
assert all(row['width'] == 256 and row['height'] == 256 for row in fixed3['maps'])
assert all(row['schedules'] == 24000 or row['won'] for row in fixed3['maps'])
fixed3['original_schedule_bound'] = 24000
guards = []
for name in ['m13_ten_players-guards', 'm14_duels-guards', 'm7_soak-guards']:
    path = ROOT / next(r['raw'] for r in process['commands'] if Path(r['raw']).name.endswith('-'+name+'.log'))
    counts = re.findall(r'^test result:.*? (\d+) passed; (\d+) failed; (\d+) ignored;', path.read_text(), re.M)
    assert counts == [('1', '0', '0')], (name, counts)
    guards.append({'command': name, 'passed': 1, 'failed': 0, 'ignored': 0, 'raw': raw(path)})
record={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'status':'PASSED_ACTUAL_ISOLATED_FIVE_FULL13_AND_FRESH_LEAD_STILL_REQUIRED','runtime_commit':process['head'],'source_files':len(manifest),'authority_spans':64,'source_manifest':raw(manifest_path),'process':raw(process_path),'source_after':{'files':EXPECTED,'mismatches':[]},'all21':all21,'fixed3':fixed3,'guards':guards,'timing_scope':'Canonical exclusive native-validation lease; no simultaneous validation Cargo/game/probe. External host workloads are outside that lease and separately observed. Means do not establish p99/max/busy-window smoothness; no old-source timing credited.'}
destination=OUT/'acceptance-summary.json';assert not destination.exists();destination.write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps({'all21_won':len(ticks),'all21_lengths':all21['won_tick_distribution'],'all21_max_mean_ms':max(r['mean_ms'] for r in all21['maps']),'all21_max_tick_ms':max(r['max_ms'] for r in all21['maps']),'fixed3':fixed3['maps'],'guards':3},indent=2))
