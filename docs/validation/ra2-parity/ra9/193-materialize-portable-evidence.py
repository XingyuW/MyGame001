from pathlib import Path
import json,hashlib,shutil,datetime
r=Path.cwd();c=Path('/Users/xingyuwang/Library/Caches/Codex/reviews/MyGame001/ra9-integration-validation');p=c/'ra9-preliminary-logs';dst=r/'docs/validation/ra2-parity/ra9/evidence';h=lambda b:hashlib.sha256(b).hexdigest();plan=json.loads((p/'190-through189-portable-evidence-file-list.json').read_text());written=[]
def put(src,dest,expected=None):
 b=src.read_bytes();x=h(b);assert expected is None or expected==x,(str(src),expected,x)
 assert not dest.exists() or dest.read_bytes()==b,str(dest)
 if not dest.exists():dest.write_bytes(b)
 written.append({'source':str(src),'repository_path':str(dest.relative_to(r)),'bytes':len(b),'sha256':x})
for x in plan['actual_files']:put(c/x['current_path'],r/x['future_path'],x['sha256'])
for name in ['190-through189-portable-evidence-file-list.json','190-through189-portable-evidence-file-list.tsv','190-through189-portable-evidence-file-list-completion.json','193-prospective-canonical-port-manifest.json']:
 put(p/name,dst/name)
for f in sorted(p.glob('192-*')):
 if f.is_file():put(f,dst/f.name)
# Original path+hash identity and alias resolution stay in190; no proof contents are rewritten.
m=json.loads((p/'193-prospective-canonical-port-manifest.json').read_text())
for x in m['records']:assert h((r/x['path']).read_bytes())==x['proposed_port_sha256'],x['path']
receipt={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'status':'ACTUAL_CANONICAL_PORT_AND_PORTABLE_EVIDENCE_MATERIALIZED_NOT_GATE_OR_REVIEW_ACCEPTANCE','canonical_predecessor':m['canonical_commit'],'manifest_sha256':h((p/'193-prospective-canonical-port-manifest.json').read_bytes()),'patch_sha256':m['patch']['sha256'],'ported_paths':m['changed_count'],'new_paths':m['new_count'],'apply_check_exit':0,'apply_exit':0,'apply_warnings':'211 legitimate terminal-empty TSV fields flagged by git apply; source values/columns preserved byte-exact. git diff --check currently reports only STATUS extra EOF blank line, to normalize before freeze. No whitespace-fix option used.','archives':written,'source_and_evidence_history':'All canonical closure history retained, all64 authority spans exact, original prompts unported; full4a runtime fixes retained. Historical166/168/171/190/191 remain immutable in cache. Actual current source binding and RA9 gates still pending.','cache_preserved':str(c),'new_external_directories':[]}
(r/'docs/validation/ra2-parity/ra9/193-canonical-port-and-archive.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps({'archived_files':len(written),'archived_bytes':sum(x['bytes'] for x in written),'port_paths':len(m['records']),'new_paths':m['new_count']},indent=2))
