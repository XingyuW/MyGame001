# Current panel dispatch index — 2026-10-21

No review has begun in this ordinary preliminary RA9 copy. This is an implementation handoff
for the later final review, not dispatch authorization or acceptance. Refresh canonical TIP,
actual source/data/module catalogue, final validation and authenticated receipts first.

R172 authorizes five fresh independent slices in two batches (three, then two), followed by a
fresh lead, and permits exceeding three hours. Full coverage and independence are unchanged:
implementers, remediators and preparers cannot review their work. Historical timing/scope prose
below does not cap or omit a slice.

The [current ownership supplement](../../reviews/RA2-parity-review-requirements.md) is the
explicit current module/test/audit map. Fill final TIP/ROUND into these preserved briefs using
that supplement; do not dispatch the old stopped-RA5 scope as the complete programme.

| Brief | Current primary responsibility and required overlap |
|---|---|
| [sim1](slice-sim1.md) | Commands, grid/room/movement, economy/production/payment, flight/docks and queued capture, harvest/ore, setup, schedule/hash. Paid UnitReload health-after-ammo and normal blocked-dock release/retry with sim2/tests. |
| [sim2](slice-sim2.md) | Combat/selectors, specialist/world/neutral interactions, visibility/power, crew/rank, launched/strategic actors, AI and all new hashed state. Cross-check sim1 service/economy and client privacy. |
| [data](slice-data.md) | Actual generated/reference/original inputs and loaders; all 48 audit Rust files and 67 TSV/4 TXT siblings, whole-key/ordered-component source closure, physical dependency equality, exact proposed policies/receipts/decisions and source census/export. All reference files and final catalogue, not just old RA5 comparisons. |
| [tests](slice-tests.md) | Every existing/new native/client/fixture/helper/validator path; audit negatives, preserved assertions, source-bound actual results and genuine one-cause controls. Late twins require actual Won/activity after36000, every-tick hash/events/outcome/I6′/I7 and both restored causal controls. |
| [client](slice-client.md) | Actual SimHost/App/event/CommandSink flow, all40 type/group/cycle/T/P/preview, contextual tree/wall controls, genuine mission actors, human powered radar notices/Gap/Sniper/Spy/Squid privacy and lifecycle, asset/settings/file boundaries. Real current consumers without fake native getters or hidden-world lookup. |
| [fresh lead](lead.md) | Reconcile five complete slices, actual final canonical gates, every P0/P1 and exact source/receipt bindings; decide under unchanged programme rules. |

The current supplement gives exact filenames and interfaces; the observed audit directory file
census is not a count of compiled modules or accepted atoms. Refresh it against the final
reachable code/data graph. The [current GAP](../../RA2_GAP_ANALYSIS.md),
[183-ID debt ledger](../../RA2_HISTORICAL_DEBT.tsv), [handoff](../../HANDOFF.md) and
[integration checklist](../../RA9_INTEGRATION_CHECKLIST.md) distinguish implemented/source-read
paths, actual preliminary observations and remaining obligations. The generated current
loader/type/map/plan catalogue and final gate/review bindings remain pending.

Pure presentation and named original MODEL-ABSENCE policies need exact static/current consumer
scope and user-only visual verification; neither grants native proof or original-engine
equivalence. Actual exposed gameplay and privacy still need meaningful genuine native/client
controls. Preserve complete USER VERIFICATION/HOWTOSEE lists; implementers have not supplied
fresh user visual/audio/feel approval.

## Completion source-closure handoff

The current ownership supplement and all five briefs now include `completion.rs`/`completion-whole.rs`, four completion TSVs and `ra9_export_actual_completion174_atom_proposals`. Data/tests own atom-aware source/physical/admission integrity; each production slice retains its real consumers. Exact137 coverage is174 atoms/151 keys:132 Whole plus19/19/4 components1/2/3. Portion acceptance must not cover Whole or siblings.

Retain actual failed complete audits131/136 and their source-bound producer-membership/uncovered-atom diagnoses. Repairs and populated records require a later complete frozen-source positive and genuine accepted-baseline negatives. Current48-Rust/67-TSV/4-TXT counts are inspection-only. A must refresh final counts/bindings/evidence destinations/TIP/ROUND and actual complete gates; the observed index through129 is an earlier cutoff. This is implementation handoff, not review, acceptance or user visual/audio verification.

## Historical panel procedure — prepared 2026-10-12 (preserved verbatim)

The original procedure below is retained unchanged, including its later R172 preface. Current
full module/scope ownership above supersedes historical stopped-state and timing assumptions,
not reviewer independence, required validation, severities or review decision rules.

# The final review of the RA2 parity programme as a panel

Current scheduling authority is R172: the user authorized all five fresh independent slices in
two batches (three, then two), followed by a fresh lead, with the round allowed to exceed three
hours. Full scope and independence are unchanged. Implementers and preparers cannot serve on
the panel. No review has begun in this preliminary validation copy; final TIP and canonical
validation/source receipts must be supplied after integration. Historical stopped-state scope
notes below do not remove RA5-B through RA9 from the final review.


Prepared on 2026-10-12, when the user asked for the review to fit in three hours (decision R55),
and not used: on 2026-10-13 the user stopped all review work, so the binding review of section 7
of `../RA2_PARITY_PROMPT.md` is still to be done (`docs/reviews/RA2-parity-reviews-outstanding.md`).
These are the briefs it was to be run with. In them `TIP` is the commit to review, `ROUND` the
number of the round, `SLICE_NAME` is already filled in, and `<SCRATCH>` is a directory outside
the repository for the reviewers' notes and slice reports.

Orchestrator's procedure for review round N (N = 1, 2, 3):

1. Tree clean at the tip of `ra2-parity`, no implementer running. Note the tip commit TIP.
2. Start `sh scripts/validate_ra2_parity.sh review-N` in the background (the reviewers' rerun of
   every validation command; about 2 to 2.5 hours). Nobody edits tracked files while it runs.
3. Under R172 launch three fresh independent slices, then the remaining two within the
   four-agent limit: `slice-{sim1,sim2,data,tests,client}.md` with TIP and N filled in.
   Each writes `<SCRATCH>/round-N-<slice>.md`, with up to120 minutes per slice. All five
   scopes remain required. Implementers/remediators/preparers are ineligible.
4. When the slices have reported and the validation has finished, launch the lead reviewer
   (`lead.md`, fresh context) who checks the validation logs, verifies every P0 and
   P1 claim of the slices, writes `docs/reviews/RA2-parity-review-N.md` and decides. 30 minutes.
5. Orchestrator commits the review file and the validation logs; GO or GO WITH DEBT: merge, tag
   `ra2-parity-reviewed`, record P2 debt in STATUS; NO-GO: remediation agent (fresh), then round
   N + 1, at most three remediation rounds.
