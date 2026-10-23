"""Read-only accounting draft. Parsing output never confers test/validator pass status."""
from pathlib import Path
import argparse,hashlib,json,re

def account(path):
    data=path.read_bytes(); text=data.decode('utf-8',errors='replace')
    pattern=re.compile(r'^test result: (?P<status>[^.]+)\. (?P<passed>\d+) passed; (?P<failed>\d+) failed; (?P<ignored>\d+) ignored;.*? (?P<filtered>\d+) filtered out;')
    rows=[]; context=None
    for n,line in enumerate(text.splitlines(),1):
        if re.match(r'^\s*(?:Running (?:unittests|tests/)|Doc-tests )',line):context={'line':n,'heading':line.strip()}
        m=pattern.match(line)
        if m:
            row={'line':n,'context':context,'status_text':m['status'],**{k:int(m[k]) for k in ['passed','failed','ignored','filtered']}}
            row['classification']='selected_child_probe' if row['filtered'] else 'primary_unfiltered_result'
            rows.append(row)
    def totals(items):return {k:sum(r[k] for r in items) for k in ['passed','failed','ignored','filtered']}
    primary=[r for r in rows if not r['filtered']];children=[r for r in rows if r['filtered']]
    contexts={}
    for r in primary:
        key=r['context']['heading'] if r['context'] else '<unassociated>'
        contexts.setdefault(key,[]).append(r['line'])
    return {'raw':{'path':str(path.resolve()),'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()},
        'scope':'Read-only result accounting, including partial/interrupted logs; no validation verdict.',
        'raw_result_blocks':len(rows),'raw_execution_totals':totals(rows),
        'validator_awk_equivalent':{k:totals(rows)[k] for k in ['passed','failed','ignored']},
        'primary_unfiltered_result_blocks':len(primary),'primary_unfiltered_totals':totals(primary),
        'selected_child_result_blocks':len(children),'selected_child_totals':totals(children),
        'primary_contexts':contexts,'ambiguous_primary_contexts':{k:v for k,v in contexts.items() if k=='<unassociated>' or len(v)>1},
        'rows':rows,'acceptance_checks_still_required':['Observed actual command exit and complete13 terminal validator status','No failed/ignored rows; primary suite completion independently checked','Inspect filtered child context; filtered child probes do not narrow parent command','Verify entire primary RA9 audit and all required real scenario/ending/performance guards','Keep lexical functions, executions, result blocks, executables and match counts distinct']}

if __name__=='__main__':
    ap=argparse.ArgumentParser();ap.add_argument('raw_log',type=Path);args=ap.parse_args()
    print(json.dumps(account(args.raw_log),indent=2))
