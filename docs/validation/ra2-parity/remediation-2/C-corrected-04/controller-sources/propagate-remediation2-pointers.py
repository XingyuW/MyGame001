"""Bounded factual propagation for the existing15 excluded outputs. Default: preserve/preview only.
Requires a fresh independently verified root packet. No producer, native process or Git mutation.
"""
import argparse
import datetime
import hashlib
import json
import re
from pathlib import Path

ROOT = Path('/Users/xingyuwang/GameProjects/MyGame001')
AUDIT = 'crates/sim/tests/ra9_audit/'
REF = 'crates/sim/tests/reference/'
ACCEPT = [AUDIT + n + '-acceptance.tsv' for n in (
    'source-policy', 'guard75', 'own-policy', 'consumer62', 'carry38',
    'behavior87', 'model162', 'completion')]
OUTPUTS = set(ACCEPT + [AUDIT + n for n in (
    'root-policy-input-binding.txt', 'ra9-approved-model-census-sha256.txt',
    'ra9-approved-model-surface.txt', 'ra9-approved-static-evidence-sha256.txt')]
    + [REF + n for n in ('ra9_scope_approval.tsv', 'ra9_legacy_range_resolution.tsv',
                         'ra9_return_fire_policy.tsv')])
BRANCH = AUDIT + 'legacy904-branch-files.tsv'
LOCAL = AUDIT + 'completion-local-code.tsv'


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def record(path):
    raw = path.read_bytes()
    return {'path': str(path.relative_to(ROOT)), 'bytes': len(raw), 'sha256': sha(raw)}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def fnv(raw):
    value = 0xcbf29ce484222325
    for byte in raw:
        value = ((value ^ byte) * 0x100000001b3) & ((1 << 64) - 1)
    return f'{value:016x}'


def table(raw):
    text = raw.decode('utf-8')
    require(text.endswith('\n') and '\r' not in text, 'noncanonical TSV newline')
    lines = text[:-1].split('\n')
    header = lines[0].split('\t')
    require(len(set(header)) == len(header), 'duplicate TSV column')
    rows = [dict(zip(header, line.split('\t'), strict=True)) for line in lines[1:]]
    return header, rows


def rendered(header, rows):
    return ('\n'.join(['\t'.join(header)] + ['\t'.join(row[k] for k in header)
                                           for row in rows]) + '\n').encode()


def tokens(cell):
    return dict(part.split('=', 1) for part in cell.split(';') if '=' in part)


def replace_token(cell, name, before, after):
    pattern = re.compile(r'(^|;)' + re.escape(name) + r'=([^;]*)')
    matches = list(pattern.finditer(cell))
    require(len(matches) == 1 and matches[0][2] == before,
            f'old {name} token does not equal independent derivation')
    return pattern.sub(lambda m: m[1] + name + '=' + after, cell)


def physical(raw, digest):
    _, rows = table(raw)
    require(len({r['repository_path'] for r in rows}) == len(rows), 'duplicate physical path')
    require(all(re.fullmatch(r'[0-9a-f]{16}', r['fnv1a64']) and
                re.fullmatch(r'[0-9a-f]{64}', r[digest]) and r['bytes'].isdigit()
                for r in rows), 'malformed physical tuple')
    return rows


def aggregate(rows, paths=None, trailing=False):
    selected = rows if paths is None else [r for r in rows if r['repository_path'] in paths]
    if paths is not None:
        require({r['repository_path'] for r in selected} == paths, 'missing family physical path')
    text = ';'.join(f"{r['repository_path']}:{r['bytes']}:{r['fnv1a64']}" for r in selected)
    return fnv((text + (';' if trailing else '')).encode())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--packet', type=Path, required=True)
    parser.add_argument('--verification', type=Path, required=True)
    parser.add_argument('--verification-sha256', required=True, help='fresh root-provided hash pin')
    parser.add_argument('--out', type=Path, required=True, help='new unused provenance directory')
    parser.add_argument('--apply', action='store_true', help='requires root explicit authorization')
    args = parser.parse_args()
    packet_path, verification_path = args.packet.resolve(), args.verification.resolve()
    packet = json.loads(packet_path.read_bytes())
    verification_raw = verification_path.read_bytes()
    require(sha(verification_raw) == args.verification_sha256, 'root verification hash mismatch')
    verification = json.loads(verification_raw)
    require(verification['status'] == 'ROOT_INDEPENDENT_FACTUAL_VERIFICATION_PASS_NOT_POLICY_OR_NATIVE_APPROVAL',
            'fresh successful root factual verification required')
    require(verification['packet'] == record(packet_path), 'verified packet bytes differ')
    factual_id = verification['root_factual_id']
    require(factual_id.startswith('ROOT_REMEDIATION2_') and factual_id != 'ROOT_CANONICAL_195_FACTUAL_2026_10_21',
            'fresh root factual ID required')
    require(verification['verified_scope'] == packet['observed_scope'], 'verified observations differ')
    derived = {row['path']: row for row in packet['derived_outputs_before_binding']}
    require(set(derived) == OUTPUTS and len(derived) == 15, 'existing15 excluded output scope differs')
    require(verification['derived_count'] == 15, 'root did not verify all15 outputs')
    # Recheck the frozen source packet, without performing root policy approval or new discovery.
    for row in packet['source_files']:
        require(record(ROOT / row['path']) == row, 'source drift: ' + row['path'])
    before = {path: (ROOT / path).read_bytes() for path in sorted(OUTPUTS)}
    for path in sorted(OUTPUTS):
        require(record(ROOT / path) == derived[path], 'excluded output drift: ' + path)
    preserved = {r['source']: r for r in packet['preserved_producer_preimages']}
    for path in (BRANCH, LOCAL, REF + 'ra9_scope_observation.tsv'):
        item = preserved[path]
        raw = (ROOT / item['preimage']).read_bytes()
        require(sha(raw) == item['sha256'] and len(raw) == item['bytes'], 'physical preimage hash mismatch')
    old_branch = physical((ROOT / preserved[BRANCH]['preimage']).read_bytes(), 'sampled_sha256')
    new_branch = physical((ROOT / BRANCH).read_bytes(), 'sampled_sha256')
    old_local = physical((ROOT / preserved[LOCAL]['preimage']).read_bytes(), 'sha256')
    new_local = physical((ROOT / LOCAL).read_bytes(), 'sha256')
    for old_rows, new_rows, digest in ((old_branch, new_branch, 'sampled_sha256'),
                                       (old_local, new_local, 'sha256')):
        require([r['repository_path'] for r in old_rows] == [r['repository_path'] for r in new_rows],
                'physical path set/order changed; explicit scope reconciliation required')
        for row in new_rows:
            raw = (ROOT / row['repository_path']).read_bytes()
            require((row['bytes'], row['fnv1a64'], row[digest]) == (str(len(raw)), fnv(raw), sha(raw)),
                    'current physical tuple differs from actual file')
    _, family_rows = table((ROOT / (AUDIT + 'legacy904-family-contracts.tsv')).read_bytes())
    families = {r['family_id']: r for r in family_rows}
    require(len(families) == len(family_rows), 'duplicate family')
    branch_proofs = {}
    for family, row in families.items():
        paths = row['actual_paths'].split(';')
        require(len(set(paths)) == len(paths), 'duplicate family scope path')
        branch_proofs[family] = {'old': aggregate(old_branch, set(paths)),
                                'new': aggregate(new_branch, set(paths)),
                                'table_order_paths': [r['repository_path'] for r in new_branch
                                                      if r['repository_path'] in paths]}
    local_proof = {'old': aggregate(old_local, trailing=True),
                   'new': aggregate(new_local, trailing=True),
                   'trailing_semicolon': True,
                   'table_order_paths': [r['repository_path'] for r in new_local]}
    _, local_contracts = table((ROOT / (AUDIT + 'completion-local-contracts.tsv')).read_bytes())
    atom = lambda row: tuple(row[k] for k in ('file', 'section', 'key', 'portion'))
    local_atoms = {atom(r) for r in local_contracts}
    require(len(local_atoms) == len(local_contracts), 'duplicate completion local atom')
    _, old_observations = table((ROOT / preserved[REF + 'ra9_scope_observation.tsv']['preimage']).read_bytes())
    require(len(old_observations) == 1, 'old observation census')
    old_scope, new_scope = old_observations[0], packet['observed_scope']
    require(set(old_scope) == set(new_scope), 'scope observation schema changed')
    require(all(old_scope[k] == new_scope[k] for k in old_scope if k != 'model_census_sha256'),
            'nonmodel observation changed; explicit policy/source reconciliation required')
    old_binding = before[AUDIT + 'root-policy-input-binding.txt'].decode().strip()
    require(re.fullmatch(r'[0-9a-f]{64}', old_binding), 'old binding shape')
    new_binding = sha(packet_path.read_bytes())
    old_model, new_model = old_scope['model_census_sha256'], new_scope['model_census_sha256']
    require(before[AUDIT + 'ra9-approved-model-census-sha256.txt'] == (old_model + '\n').encode(),
            'old model binding differs from preserved observation')
    require(before[AUDIT + 'ra9-approved-model-surface.txt'] == (ROOT / (REF + 'ra9_model_surface.txt')).read_bytes(),
            'actual model surface changed; do not rewrite a policy surface silently')
    after = dict(before)
    after[AUDIT + 'root-policy-input-binding.txt'] = (new_binding + '\n').encode()
    after[AUDIT + 'ra9-approved-model-census-sha256.txt'] = (new_model + '\n').encode()
    deltas, changed_families, local_seen = [], set(), set()
    for path in sorted(OUTPUTS):
        if not path.endswith('.tsv'):
            continue
        header, old_rows = table(before[path])
        require(rendered(header, old_rows) == before[path], 'TSV roundtrip changed layout')
        rows = [dict(r) for r in old_rows]
        for index, row in enumerate(rows):
            permitted = {'final_input_sha256'}
            require(row['final_input_sha256'] == old_binding, 'old final input mismatch')
            row['final_input_sha256'] = new_binding
            if path == REF + 'ra9_scope_approval.tsv':
                require(len(rows) == 1 and all(row[k] == old_scope[k] for k in old_scope), 'old approval observations differ')
                for key, value in new_scope.items():
                    row[key] = value
                require(row['recorded_scope_review'].startswith('ROOT_'), 'unexpected old scope-review identity')
                row['recorded_scope_review'] = factual_id
                permitted.update(new_scope)
                permitted.add('recorded_scope_review')
            if path in ACCEPT and 'consumer_bindings' in header:
                cell = row['consumer_bindings']
                token_map = tokens(cell)
                if 'model-census' in token_map:
                    cell = replace_token(cell, 'model-census', old_model, new_model)
                if 'branch-fnv' in token_map:
                    require(path == AUDIT + 'source-policy-acceptance.tsv', 'foreign branch token owner')
                    family = token_map['family']
                    require(family in families, 'unowned family')
                    proof = branch_proofs[family]
                    cell = replace_token(cell, 'branch-fnv', proof['old'], proof['new'])
                    proof.setdefault('acceptance_rows', 0)
                    proof['acceptance_rows'] += 1
                    if proof['old'] != proof['new']:
                        changed_families.add(family)
                if path == AUDIT + 'completion-acceptance.tsv':
                    if atom(row) in local_atoms:
                        cell = replace_token(cell, 'code', local_proof['old'], local_proof['new'])
                        local_seen.add(atom(row))
                    else:
                        require('code' not in token_map, 'nonlocal completion unexpectedly has code token')
                row['consumer_bindings'] = cell
                permitted.add('consumer_bindings')
            for column in header:
                if row[column] != old_rows[index][column]:
                    require(column in permitted, 'nonfactual cell mutation: ' + column)
                    deltas.append({'path': path, 'row': index + 2, 'column': column,
                                   'before': old_rows[index][column], 'after': row[column]})
            # Every class/value/reason/route/guard/decision/qualified-receipt cell is untouched.
            require(all(row[k] == old_rows[index][k] for k in header if k not in permitted),
                    'policy cell changed')
        after[path] = rendered(header, rows)
    require(local_seen == local_atoms, 'missing completion local accepted row')
    require(all(p.get('acceptance_rows', 0) == int(families[f]['atom_count'])
                for f, p in branch_proofs.items()), 'family acceptance census differs')
    out = args.out.resolve()
    require(not out.exists(), 'provenance output directory already exists; preserve it')
    out.mkdir(parents=True)
    files = []
    for path in sorted(OUTPUTS):
        saved, preview = out / 'preimages' / path, out / 'preview' / path
        saved.parent.mkdir(parents=True, exist_ok=True)
        preview.parent.mkdir(parents=True, exist_ok=True)
        saved.write_bytes(before[path])
        preview.write_bytes(after[path])
        files.append({'path': path, 'before_sha256': sha(before[path]), 'after_sha256': sha(after[path]),
                      'preimage': str(saved), 'preview': str(preview), 'changed': before[path] != after[path]})
    # A frozen-source/old-output recheck immediately precedes any explicitly authorized writes.
    for row in packet['source_files']:
        require(record(ROOT / row['path']) == row, 'source changed while planning propagation')
    for path in OUTPUTS:
        require((ROOT / path).read_bytes() == before[path], 'excluded output changed while planning')
    if args.apply:
        for path in sorted(OUTPUTS):
            if before[path] != after[path]:
                (ROOT / path).write_bytes(after[path])
        require(all((ROOT / p).read_bytes() == after[p] for p in OUTPUTS), 'propagation readback differs')
    report = {'utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
              'status': 'FACTUAL_POINTERS_APPLIED' if args.apply else 'FACTUAL_POINTER_PREVIEW_ONLY',
              'packet': record(packet_path), 'verification': record(verification_path),
              'root_factual_id': factual_id, 'input_scope_sha256': new_binding,
              'files': files, 'cell_deltas': deltas, 'branch_proofs': branch_proofs,
              'changed_branch_families': sorted(changed_families), 'completion_local_code_proof': local_proof,
              'completion_local_rows_proved': len(local_seen),
              'limits': 'Factual tokens only; no class/value/reason/route/guard/policy decision/qualified receipt change. Not native evidence or policy approval. Old195/196/196b preserved.'}
    (out / 'propagation.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'report': str(out / 'propagation.json'), 'status': report['status'],
                      'outputs': len(files), 'changed_outputs': sum(x['changed'] for x in files),
                      'cell_deltas': len(deltas), 'changed_branch_families': len(changed_families),
                      'completion_local_rows_proved': len(local_seen)}))


if __name__ == '__main__':
    main()
