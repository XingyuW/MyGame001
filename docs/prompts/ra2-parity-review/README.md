# The final review of the RA2 parity programme as a panel

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
3. At the same moment launch the five slice reviewers (fresh contexts, never implementers or
   remediators): `slice-{sim1,sim2,data,tests,client}.md` with TIP and N filled in.
   Each writes `<SCRATCH>/round-N-<slice>.md`. Deadline 120 minutes.
4. When the slices have reported and the validation has finished, launch the lead reviewer
   (`lead.md`, fresh context) who checks the validation logs, verifies every P0 and
   P1 claim of the slices, writes `docs/reviews/RA2-parity-review-N.md` and decides. 30 minutes.
5. Orchestrator commits the review file and the validation logs; GO or GO WITH DEBT: merge, tag
   `ra2-parity-reviewed`, record P2 debt in STATUS; NO-GO: remediation agent (fresh), then round
   N + 1, at most three remediation rounds.
