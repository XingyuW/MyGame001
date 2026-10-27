#!/usr/bin/env python3
"""R216: record the RA9 game-array policy of the maps merged in R215.

The RA9 audit compares every map of assets/data/skirmish.ron with an individually recorded
authored-map policy (game-arrays.tsv, own-policy-contracts.tsv, own-policy-acceptance.tsv;
decision RA9-P8). It knew the 21 maps of RA9. This script, run once from the repository root:

- rewrites the menu registry row `skirmish.ron.maps.identities` to the 42 maps, in order;
- appends a `skirmish.map.<id>.starts.xy` row for every map the tables lack, after the last
  map row, with the wording of the existing rows and the starts read from skirmish.ron;
- writes the declaration of those rows (r216-game-array-map-policy-declaration.json) and
  records it, by path and SHA-256, as their receipt, as RA9-P8's rows record theirs.

`contract_sha256` of a new or changed row is the SHA-256 of its game-arrays.tsv line
(`array_id`, value and policy joined by tabs, no newline). The final input binding is the
current one; the rebind that follows rewrites it like every other factual pointer.
Every other row is left byte for byte as it was.
"""
import hashlib
import json
import re
from pathlib import Path

ROOT = Path.cwd()
T = ROOT / 'crates/sim/tests/ra9_audit'
OUT = ROOT / 'docs/validation/ra2-parity/rebind-r216/map-policy'
DECLARATION = OUT / 'r216-game-array-map-policy-declaration.json'
DECISION = 'R216/authored-specific-current-model'
REGISTRY = 'skirmish.ron.maps.identities'


def sha(text):
    return hashlib.sha256(text.encode()).hexdigest()


ron = (ROOT / 'assets/data/skirmish.ron').read_text()
maps = [
    (m.group(1), [int(v) for xy in re.findall(r'\(x: (\d+), y: (\d+)\)', m.group(2)) for v in xy])
    for m in re.finditer(r'\(id: "([a-z0-9_]+)", starts: \[([^\]]*)\]\)', ron)
]
ids = [m for m, _ in maps]
assert len(ids) == len(set(ids)) == 42, ids


def starts_policy(map_id, values):
    return (
        f"Original authored map '{map_id}': exact ordered {len(values) // 2} player-start (x,y) "
        f"pairs, components='{','.join(map(str, values))}'; actual MapDef positions and raw tuple "
        "identities must match. These are generated game map choices, not original RA2 map "
        "assets or native source-key coverage."
    )


registry_value = ','.join(ids)
registry_policy = (
    f"Exact ordered original generated map menu registry '{registry_value}'; each actual loaded "
    "map/start tuple is separately compared; no original RA2 map-data coverage."
)

files = {name: (T / f'{name}.tsv').read_text().split('\n') for name in
         ['game-arrays', 'own-policy-contracts', 'own-policy-acceptance']}
for lines in files.values():
    assert lines[-1] == '', 'tables end with a newline'
known = {line.split('\t')[0] for line in files['game-arrays'][1:-1]}
new_rows = [(f'skirmish.map.{m}.starts.xy', ','.join(map(str, v)), starts_policy(m, v))
            for m, v in maps if f'skirmish.map.{m}.starts.xy' not in known]
assert len(new_rows) == 21, len(new_rows)
changed = [(REGISTRY, registry_value, registry_policy)] + new_rows

rows = [{'array_id': a, 'contract_sha256': sha(f'{a}\t{v}\t{p}'), 'specific_policy': p, 'value': v}
        for a, v, p in changed]
OUT.mkdir(parents=True, exist_ok=True)
DECLARATION.write_text(json.dumps({
    'status': 'R216_STATIC_POLICY_DECLARATION_OF_THE_MERGED_MAPS',
    'decision': 'docs/DECISIONS.md#R216',
    'follows': 'docs/validation/ra2-parity/ra9/evidence/32-game-array-policy-declaration.json',
    'contract_rule': 'contract_sha256 = sha256(array_id TAB value TAB specific_policy)',
    'source': 'assets/data/skirmish.ron',
    'native_or_loaded_comparison_claim': False,
    'meaning': 'The menu registry of 42 maps and the starts of the 21 maps merged in R215, '
               'authored current-model choices recorded like RA9-P8; the audit compares each '
               'with the loaded map.',
    'rows': rows,
}, indent=2) + '\n')
receipt = f'static/{DECLARATION.name}@{hashlib.sha256(DECLARATION.read_bytes()).hexdigest()}'
binding = (T / 'root-policy-input-binding.txt').read_text().strip()


def line(name, a, v, p):
    c = sha(f'{a}\t{v}\t{p}')
    return {'game-arrays': f'{a}\t{v}\t{p}',
            'own-policy-contracts': f'{a}\t{v}\t{p}\t{c}',
            'own-policy-acceptance': f'{a}\t{c}\t{DECISION}\t{binding}\t{receipt}\t{v}\t{p}'}[name]


for name, lines in files.items():
    body = lines[1:-1]
    position = {row.split('\t')[0]: n for n, row in enumerate(body)}
    body[position[REGISTRY]] = line(name, *changed[0])
    last = max(n for a, n in position.items() if a.startswith('skirmish.map.'))
    body[last + 1:last + 1] = [line(name, *row) for row in new_rows]
    (T / f'{name}.tsv').write_text('\n'.join([lines[0]] + body + ['']))
print(json.dumps({'changed_rows': len(changed), 'receipt': receipt}))
