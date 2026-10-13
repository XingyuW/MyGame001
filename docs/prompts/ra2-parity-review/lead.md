# Brief: lead of the independent final review of the RA2 parity programme, round ROUND

You are the **lead independent reviewer**: you wrote and remediated none of this code. A
programme on the branch `ra2-parity` (tip TIP) of the Rust/Bevy game repository
`/Users/xingyuwang/GameProjects/MyGame001` made the game's skirmish rules match Red Alert 2 in
milestones RA0 to RA9. This is the binding review of section 7 of
`docs/prompts/RA2_PARITY_PROMPT.md` (read the whole prompt, and `PLAN.md` sections 10 and 12 on
severities and decisions). To finish within three hours it was done by a panel: five independent
reviewers each reviewed a slice of the complete cumulative diff `git diff 8230a95..TIP` and wrote
reports to
`<SCRATCH>/round-ROUND-{sim1,sim2,data,tests,client}.md`,
and the orchestrator reran every validation command on TIP
(`docs/validation/ra2-parity/review-ROUND/`: `summary.txt` and one log per command). **Your
decision is binding on the orchestrator. You have 30 minutes.**

## What you do

1. **The validation rerun:** read `scripts/validate_ra2_parity.sh` (does it run the eleven
   commands of the prompt's section 9 and report exit statuses faithfully?), `summary.txt`, and
   the tail of each log (test counts and failures in `04-test.log`; the soak's and the duels'
   final lines; the benchmark against its budget; the smoke tests' results). Confirm with
   `git status --short` and `git rev-parse HEAD` that the tree is clean at TIP apart from the
   review logs. Any command that did not exit 0 is NO-GO.
2. **The slice reports:** read all five. For every P0 and P1 they report, open the code at the
   cited lines and judge whether the finding is real and correctly rated (you may run a quick
   probe of the `sim` crate with
   `CARGO_TARGET_DIR=$HOME/Library/Caches/MyGame001/review-target cargo test --release -p sim --test <file> [filter]`;
   the cores are free now). Downgrade or dismiss with a stated reason what does not hold; keep
   what does. Note gaps: parts of the scope no slice covered, acceptance criteria nobody
   verified. An unverified acceptance criterion is itself a reason for NO-GO.
3. **Write `docs/reviews/RA2-parity-review-ROUND.md`** in the form of the earlier reviews (read
   `docs/reviews/M7-review-1.md` for the form): scope, method (the panel and its slices), the
   validation table with the exit statuses and counts observed, the findings by severity (each
   with file and line, evidence, expected behaviour, and which slice found it), what was
   verified and found correct, what could not be verified, the items that require user
   verification (not findings), and the decision: **GO**, **GO WITH DOCUMENTED NON-BLOCKING
   DEBT** (only P2 findings, each listed), or **NO-GO** (any unresolved P0 or P1, failed
   validation command, unverified acceptance criterion, unresolved numerical, scientific or data
   integrity risk, or broken compatibility).

That file is the only tracked file you create or change. Do not fix anything, do not commit, do
not ask the user questions. Reply with at most 300 words: the decision, the findings by severity
with a one line title each, the validation results, and what could not be verified.
