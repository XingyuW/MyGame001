# Brief: independent final review of the RA2 parity programme, round ROUND, slice sim2

You are an **independent reviewer**: you wrote and remediated none of this code. A programme on
the branch `ra2-parity` of the Rust/Bevy game repository `/Users/xingyuwang/GameProjects/MyGame001`
made the game's skirmish rules match Red Alert 2 in milestones RA0 to RA9. This is the binding
review of the prompt's section 7 (`docs/prompts/RA2_PARITY_PROMPT.md`). To finish within three
hours it is done by a panel: five reviewers each take a slice of the complete cumulative diff,
and a lead reviewer then verifies their findings, checks the validation rerun and decides. Be
sceptical: every statement of the programme's own documents (plan, decisions, status entries,
gap analysis, commit messages, earlier advisory reviews) is a claim to check against the code,
the data and what commands actually do.

## Scope

The cumulative diff `git diff 8230a95..TIP` (`8230a95` is `main` before the programme; TIP is the
tip of `ra2-parity`). Commits `116ffdd` to `257a169` are the user's earlier uncommitted work,
committed unchanged: context, but regressions the programme caused in it are in scope. The
programme's own work is `21225b0..TIP`. Requirements: `docs/prompts/RA2_PARITY_PROMPT.md`,
`docs/RA2_PARITY_PLAN.md` (conversions and equations in sections 2 and 4, milestone entries with
acceptance criteria in section 5, research in appendix D), `PLAN.md` sections 5, 6, 8, 10, 11, 14
(invariants I1 to I12 with I6 amended to I6' by decision R6; severities), `docs/DECISIONS.md`
from R1, `docs/STATUS.md` section "RA2 parity programme", `docs/RA2_GAP_ANALYSIS.md`.
Earlier advisory reviews, if any, are in `docs/reviews/RA2-parity-advisory-*.md`: use them as
leads, verify that what they found was really fixed, and trust none of their conclusions.

## Rules while you work

* The validation commands are being rerun right now by the orchestrator on TIP
  (`docs/validation/ra2-parity/review-ROUND/`); they use every core for about two hours. Do NOT
  run cargo in the repository's default target directory, do not start the long AI match tests,
  the soak, the duels or the game. For probes build only the `sim` crate with
  `CARGO_TARGET_DIR=$HOME/Library/Caches/MyGame001/review-target cargo test --release -j 3 -p sim --test <file> [filter]`
  (throwaway probe tests go in your scratch directory and are passed with a path dependency, or
  are placed as `crates/sim/tests/probe_sim2_*.rs` and DELETED before you finish: leave
  the tree exactly as you found it; check with `git status --short`).
* Do not edit, stage or commit anything; no git command that changes the index, working tree or
  refs. Do not fix anything. Do not ask the user questions.
* The RA2 reference files are in `~/Library/Caches/MyGame001/ra2-reference/`
  (`python3 scripts/ra2_ini.py rules <Section> [Key]`).
* Scratch: `<SCRATCH>/`.

## Severity and output

**P0**: crash, data loss, broken build, violated invariant, incorrect simulation result.
**P1**: missing acceptance criterion, inadequate test for specified behaviour, silent fallback,
unverified assumption, a deleted, skipped or weakened test, RA2 material in the repository.
**P2**: maintainability, naming, documentation debt. Items only the user can verify (visual,
audio, feel) are not findings (PLAN.md section 12 item 2); list them separately.

Each finding: title, severity, file and line, evidence (what you read or ran), expected
behaviour. Verify before you report: confirm every P0 and P1 by reading the whole code path or
by a probe; what you could not confirm goes under "suspected, not confirmed". Also list what you
verified and found correct (specific, with evidence), and what you could not check in the time.

Write your report to
`<SCRATCH>/round-ROUND-sim2.md`
and reply with at most 300 words: counts by severity, a one line title per P0 and P1, and what
you could not check. **You have 120 minutes; budget your depth accordingly, most important
checks first, and report in time even if incomplete, saying what is left.**

## Your slice

**Slice `sim2`: the simulation's rules of RA4 to RA7.** In `crates/sim/src/`: `veterancy.rs`,
`ore.rs`, `crates.rs`, `harvest.rs`, `props.rs`, `visibility.rs`, `special.rs` and its modules
(superweapons, infiltration, parasites, radiation, disguise, spawners, aircraft, garrisons,
bridges, tech buildings, the neutral player), the new commands of RA7 (guard, scatter, follow,
force fire, force move, waypoints), and their use in `combat.rs`, `movement.rs`, `economy.rs`.
Check the same properties as slice `sim1` for this code (hashing of every new piece of state:
experience, multipliers, ammunition, docks, spawner state, hidden sets, ore kind and density,
crates, garrisons, bridges, radiation, superweapon state, order queues, guard state; ordered
iteration; `SimRng` only; I5, I6', I7, I8, I12), and the equations of plan 4.5, 4.7, 4.8, 4.10
to 4.14 at their boundaries; every row of the table of plan 4.13 against the code (sample at
least twelve rows deeply: thresholds, durations and areas converted correctly, who is affected,
what happens at the edges); that all area damage goes through the warhead and armour table;
that hidden things (spies, Mirage tanks, submarines, gap areas) leak through no event, command
rejection or computer player knowledge; the acceptance criteria of plan entries RA4 to RA7
against the code.
