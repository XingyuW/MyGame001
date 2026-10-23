from pathlib import Path
import hashlib,json,subprocess,sys
R=Path('/Users/xingyuwang/GameProjects/MyGame001');S=Path(__file__).parent
label=sys.argv[1];tip=subprocess.check_output(['git','-C',str(R),'rev-parse','HEAD'],text=True).strip()
assert not subprocess.check_output(['git','-C',str(R),'status','--porcelain'],text=True).strip()
summary=R/'docs/validation/ra2-parity'/label/'final-validation-summary.json';D=json.loads(summary.read_bytes());assert D['status']=='ACTUAL_CORRECTED_SHARED_RA9_FINAL_REQUIRED13_AND_ISOLATED5_PASS_NOT_REVIEW_OR_USER_APPROVAL'
Fpath=Path(D['frozen_manifest']['path']);F=json.loads(Fpath.read_bytes());sha=lambda b:hashlib.sha256(b).hexdigest()
for row in F['source_files']:assert sha((R/row['path']).read_bytes())==row['sha256']
reports=[]
for role in ['sim1','sim2','client','data','tests']:
 for name in [f'round-2-c3-revised-{role}.md',f'round-2-final-evidence-{role}.md']:
  q=S/name;assert q.exists();reports.append({'path':str(q),'sha256':sha(q.read_bytes())})
for role in ['sim1','sim2','client','data','tests']:
 for phase in ['native-partial','workspace-evidence']:
  q=S/f'round-2-{phase}-{role}.md'
  if q.exists():reports.append({'path':str(q),'sha256':sha(q.read_bytes())})
body=f"""Fresh independent panel2 lead: actual complete corrected cumulative candidate

Review the entire cumulative 8230a955b251e9341d349f99eaeb86f7f8253664..{tip} programme.
Also write {S/'round-2-final-debt-dispositions.tsv'} with exactly the183 stable ledger IDs and
byte-exact historical_aliases copied from docs/RA2_HISTORICAL_DEBT.tsv, plus your independent
disposition, severity, evidence and remaining_obligation columns. Use one row per ID, keeping
aliases distinct. This is your individual adjudication, not a root-generated blanket closure.
The report may reference this companion instead of duplicating the183-row table inline.
Root will preserve both verbatim and verify IDs/aliases before importing; do not relabel user-only
quality as passed or silently remove a retained scope. Source absence and successful related
execution do not supply unproved per-ID acceptance.

You must never have implemented, remediated or prepared any portion. Read the canonical lead
brief, original/continuation prompts, current review requirements, R172/R194/R199/R200/R201/R202/R203,
previous panel1 binding NO-GO, all five immutable initial panel2 reports, all five revised
reports and their actual final-evidence addenda listed below. R199 kept this panel open before
any binding lead decision; correction pass2 of3 remains. All five full cumulative slices
have authenticated retained scope and inspected revised paths/interfaces. This is not a
changed-lines-only review or a new round shortcut. Independently confirm every prior P0/P1
resolution, reconcile duplicates and retained P2 with source/line/test/raw evidence, verify
full requirement and module coverage, R6/I6prime, R7, unchanged R29 caps and every data/policy
integrity condition. Root source/factual hashes and native passage do not semantically approve
model choices or prove original-engine nonuse. Inspect all183 stable debt IDs/aliases with
individual disposition/scope, rather than blanket nonblocking acceptance; preserve all51 user
verification rows as pending, never visual/audio/human-feel approval or manufactured failure.

The corrected code freeze is{len(F['source_files'])} inputs and64 authority spans, manifest {Fpath},
SHA256 {sha(Fpath.read_bytes())}. The manifest binds the exact corrected source. Independently verify its full path census,
protected spans, excluded metadata deltas, and the actual clean gate launch source. Actual final native summary {summary},
SHA256 {sha(summary.read_bytes())}. All13 original commands ran once completely on unchanged
frozen inputs under R194; no rerun of the entire expensive gate is required. Isolated five,
all185 audit and{D['workspace']['full_ai_integration_target']['passed']} AI target together (including all139 retained original C cases),756 production/756 duel outcomes and distributions,
260x2x36000 deterministic windows/heap, exact benchmark and600-frame smoke/package gates,
real release/package missing-image exit1 and restored600-frame exit0 all require independent
raw/result verification. Failed/interrupted/setup/compiler/history attempts remain intact;
never sum retries into a pass or count selected child probes as unfiltered primary tests.
Keep m13 overall-mean acceptance distinct from observed busy/p99/max and render smoothness.
Keep fixed-duration sampled Running worlds distinct from all-to-Won scenarios. Keep actual
phase11 packaged README bytes distinct from later closing prose. Source/data/reference original
hashes, physical atoms/pairs/class/reason/route integrity and policy admissibility need actual
checking; source fingerprints are not acceptance by themselves.

Canonical files are read-only during this review. Write the separate binding report only to
{S/'round-2-lead.md'}, including exact reviewed TIP/freeze/evidence binding, explicit complete
coverage/limits, every unresolved finding with severity and a GO, GO WITH DOCUMENTED NON-BLOCKING
DEBT, or NO-GO decision under unchanged criteria. Any unresolved P0/P1, failed mandatory gate,
unverified mandatory acceptance or unresolved numerical/data-integrity risk means NO-GO.
Do not preapprove absent evidence, silently downgrade a finding or choose the desired result.
No native processes or canonical edits without a concrete exclusive lease from root. No new
agents/checkouts/clones or unrelated process changes. User has authorized final local main merge
and reviewed tag only after your independent GO/GO WITH DEBT; root cannot supply your approval.

Reports and exact readback hashes:
"""
for row in reports:body+=row['path']+' SHA256 '+row['sha256']+'\n'
p=S/'dispatch-round-2-c3-corrected-lead.md';assert not p.exists();p.write_text(body);print(str(p))
