from pathlib import Path
import json,hashlib
p=Path('/Users/xingyuwang/Library/Caches/Codex/reviews/MyGame001/ra9-integration-validation');logs=p/'ra9-preliminary-logs'
updates={
'docs/prompts/ra2-parity-review/slice-sim1.md':('Retain failed whole audits131/136 and require later frozen-source positive plus genuine accepted-baseline negatives.','Retain failed whole audits131/136 alongside the actual158 whole positive and159/168 negative/repair outcomes; require fresh final canonical frozen-source positive and accepted-baseline negatives.'),
'docs/prompts/ra2-parity-review/slice-sim2.md':('Preserve failed whole audits131/136 and require later complete frozen-source baseline and meaningful accepted-record negatives.','Preserve failed whole audits131/136,158 whole positive and159/168 outcomes. Require a fresh complete final canonical frozen-source baseline and meaningful accepted-record negatives.'),
'docs/prompts/ra2-parity-review/slice-data.md':('A still owes final declared/reachable/source/portion/class/physical/Own counts, completion record/root Scope hashes and whole-positive/accepted-baseline-negative outcomes; index129 is an earlier cutoff.','158 now records the preliminary whole-positive counts and159/168 the full-target175/6 and six-repair outcomes. A still owes actual final canonical declared/reachable/source/portion/class/physical/Own counts, completion record/root Scope hashes and fresh whole-positive/accepted-baseline-negative outcomes; index129 is a historical cutoff.'),
'docs/prompts/ra2-parity-review/slice-tests.md':('Earlier index129/focused positives/populated records are not a final whole-audit pass. A must attach later complete-positive and accepted-baseline one-cause/restoration receipts and final census/hash/gate counts.','158 is the actual preliminary whole-audit pass;159 full-target175/6 and168 six-repair results retain their exact scopes. Index129 is historical. A must attach fresh final canonical complete-positive and accepted-baseline one-cause/restoration receipts and actual final census/hash/gate counts.'),
'docs/prompts/ra2-parity-review/slice-client.md':('Retain failed full audits131/136. Later accepted baseline, bad-record negatives, source/route/receipt bindings and complete validation remain separate obligations;174-atom census is not acceptance.','Retain failed full audits131/136, the158 whole positive and159/168 actual results. Fresh final canonical accepted baseline, bad-record negatives, source/route/receipt bindings and complete validation remain separate obligations;174-atom census is not acceptance.'),
'docs/prompts/ra2-parity-review/lead.md':('Retain failed whole audits131/136 and exact producer-membership/uncovered diagnoses, then require later frozen-source whole positive and valid accepted-baseline negatives.','Retain failed whole audits131/136 and exact producer-membership/uncovered diagnoses,158 whole positive and159/168 actual outcomes; require a fresh final canonical frozen-source whole positive and valid accepted-baseline negatives.')}
rpath=logs/'192-current-output-body-reconciliation.json';r=json.loads(rpath.read_text())
# Preserve the initial cleanup receipt and intermediate source for every subsequent refinement.
pre=logs/'192-before-review-obligation-refinement.json';assert not pre.exists();pre.write_bytes(rpath.read_bytes())
for f,(old,new) in updates.items():
 b=(p/f).read_bytes();s=b.decode();assert s.count(old)==1
 mid=logs/('192-intermediate-'+f.replace('/','__'));assert not mid.exists();mid.write_bytes(b)
 (p/f).write_text(s.replace(old,new,1))
for e in r['changed_outputs']:
 e['after_sha256']=hashlib.sha256((p/e['path']).read_bytes()).hexdigest()
for f,h in r['frozen_files'].items():assert hashlib.sha256(Path(f).read_bytes()).hexdigest()==h
r['refinement']='Current review obligations now explicitly distinguish actual158/159/168 copy outcomes from the fresh canonical freeze, rather than using undated later-baseline wording. Intermediate bytes and initial receipt retained.'
rpath.write_text(json.dumps(r,indent=2)+'\n')
print('Final receipt SHA256 '+hashlib.sha256(rpath.read_bytes()).hexdigest())
