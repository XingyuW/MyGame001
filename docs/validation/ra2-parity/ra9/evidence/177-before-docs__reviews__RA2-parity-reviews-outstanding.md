# RA2 parity programme: review work left for later

Current scheduling authority is R172: the user authorized all five fresh independent slices in
two batches (three, then two), followed by a fresh lead, with the round allowed to exceed three
hours. Full scope and independence are unchanged. Implementers and preparers cannot serve on
the panel. No review has begun in this preliminary validation copy; final TIP and canonical
validation/source receipts must be supplied after integration. Historical stopped-state scope
notes below do not remove RA5-B through RA9 from the final review.


On 2026-10-13 the user stopped all review work on the programme of
`docs/prompts/RA2_PARITY_PROMPT.md` ("Finish all the implementation and stop review. Write a
report about what reviews are left for later review work."). This is that report. It is written
by the orchestrator of the run, who is not a reviewer: it lists what has and has not been
reviewed, and it approves nothing.

**State of this report:** written after RA4 and updated twice: with the outcome of the
remediation of the advisory reviews, and on 2026-10-13 when the user also stopped the
implementation, during RA5 ("Stop where you are here, and write a comprehensive review
requirement for later review", decision R63). **The comprehensive requirements for the later
review are in `RA2-parity-review-requirements.md`**, which also states exactly what is
implemented, what is only partly validated and what was never built; where the two documents
differ, that one is the later and prevails. In particular, sections 3.2 and 6 below were written
when RA5 to RA9 were still expected: RA5 items 1 to 4 exist as three commits that are validated
only in part, and RA5 items 5 to 9, RA6, RA7, RA8 and RA9 were not implemented.

## 1. Summary

| | |
|---|---|
| Binding review required by the prompt's section 7 | **not done** |
| Decision (GO, GO WITH DEBT, NO-GO) | none exists |
| Merge of `ra2-parity` into `main`, tag `ra2-parity-reviewed` | **not done**, because section 7 allows them only after GO |
| Milestones examined by an independent reviewer at all | RA1 to RA4, by advisory reviews of limited scope (section 2) |
| Milestones never examined by an independent reviewer | RA5 to RA9, the fixes made after the advisory reviews, and the baseline fixes of RA0 |
| What every milestone does have | the eleven validation commands exiting 0 on its final tree (`docs/validation/ra2-parity/<milestone>/`), run by its implementer |

The validation commands show that the code builds, that the tests the implementers wrote pass,
and that computer players finish their matches deterministically. They do not show that the
tests are adequate, that the ported rules are the right rules, or that nothing was missed; that
is what the review was for.

## 2. Review work that was done (advisory only)

Advisory reviews were introduced on 2026-10-12 (decision R55) to shorten the binding review.
Each was done by independent reviewers on a read only snapshot of a commit, without building the
game crate and without running the long matches. They gate and approve nothing.

| Review | Scope | Commit | Slices | Findings | Reports |
|---|---|---|---|---|---|
| Advisory 1 | RA1 to RA3 (`21225b0..46e694d`) | `46e694d` | simulation, data, tests and client | 1 P0, 9 P1, 24 P2, 7 suspected | `RA2-parity-advisory-1-{sim,data,tests}.md` |
| Advisory 2 | RA4 (`46e694d..6a41bc3`) | `6a41bc3` | simulation, tests with data and client | 1 P0, 8 P1, 14 P2, 5 suspected | `RA2-parity-advisory-2-{sim,tests}.md` |

What these reviews established, in their own words and with their evidence, is in the reports.
In brief: no shipped value was found wrong (3,805 values of RA1 to RA3 and 1,032 of RA4 were
recomputed independently from the reference ini and matched, apart from listed differences); no
test was found deleted, ignored or stripped of assertions; provenance was clean; decision R7 (the
speed conversion) was judged justified. The defects they found were in command handling
(objects destroyed earlier in a tick could still act; commands told unseen and hidden things
from missing ones), aircraft ammunition, spawners, and in tests and checks that could not fail
or did not cover what they claimed.

**Remediation** (commits `43b5ef1` to `a9321c1`, decision R56, STATUS entry "Advisory reviews 1
and 2 (RA1 to RA4) and their remediation"). One remediation agent reconciled all 69 items of the
five reports against the code after RA4: 46 fixed, each real defect with a regression test; 1
settled by a decision (the soak stays at 36,000 ticks, and the 756 shipped matches now check I7
on every tick and I6' on every sixteenth to their end); 14 deferred as debt; 8 refuted, found to
be no defect, or not verifiable. Both P0 are recorded as fixed at the root (commands no longer
act on what the tick destroyed; a round is spent by a volley's first shot at any target). Of the
17 P1, 15 are recorded as fixed, 1 as decided, and 1 as deferred: `Soylent=300` on the refineries
is a listed difference, to be applied in RA5. Validation after the fixes: all commands exit 0,
925 tests, and every one of the 756 duels ends in the same tick as before the fixes.
**Nobody independent has checked those fixes or verdicts** (section 3.2).

## 3. Review work that is left

### 3.1 The binding final review (prompt section 7, item 1)

Never started. It has to cover the complete cumulative diff `8230a95..ra2-parity` and to:

1. rerun every validation command of the prompt's section 9 on the tip;
2. check the runtime execution of the production path for every milestone's acceptance
   criteria (plan section 5, RA1 to RA9);
3. check error context, silent fallbacks and data loss;
4. check API, command, event and schema changes and their migrations;
5. check determinism and the hashing of every new piece of state;
6. check test adequacy for each acceptance criterion, and that no test was deleted, skipped or
   weakened;
7. check the numerical and unit correctness of every ported value against rules.ini through the
   conversions;
8. check regressions in existing behaviour, and asset and reference data provenance;
9. check the rewritten gap analysis;

and to write `docs/reviews/RA2-parity-review-1.md` with findings rated P0, P1 or P2 and a
decision. On NO-GO a remediation by a fresh agent and a new review follow, at most three rounds
(section 7, item 2). Only after GO or GO WITH DEBT may the branch be merged and tagged (item 3).

Briefs for running it as a panel of five slice reviewers and a lead within about three hours a
round are prepared in `docs/prompts/ra2-parity-review/` (procedure in its README). They were
written before RA5 to RA9 existed and name modules by the plan; whoever runs the review should
check the slice texts against the final tree.

### 3.2 Scope no independent reviewer has looked at

| Scope | Why it matters |
|---|---|
| RA5: ore in bails, gems, growth and spread, crates, the neutral player, tech buildings, garrisons, destructible bridges, the regenerated maps | new world state and randomness; income rules (I7); map changes |
| RA6: every specialist and superweapon of gap analysis section 9, radiation, disguise, infiltration | the largest body of new rules; hidden information; area damage |
| RA7: shroud only vision, gap generator, sensors, the RA2 hotkeys, new orders (guard, scatter, follow, force fire and move, waypoints), announcer | vision changes what players and the computer player may know; key changes |
| RA8: the computer opponent with the full arsenal and difficulty multipliers | honest vision, bounded commands, match endings |
| RA9: the completed data parity audit, the rewritten gap analysis and documents | the audit is the programme's own proof of parity; its coverage claim needs an outside check |
| The remediation of advisory reviews 1 and 2 | every fix and every "refuted" verdict is the remediator's own; section 7 requires a new independent review after a remediation |
| RA0: the soak's parallel run, its allocator accounting and its accepted placement rejection (R8), the validation script | they changed a validation tool before the baseline was validated |

### 3.3 Inside the reviewed milestones, what the advisory reviewers said they could not check

* Anything that needs the game crate built or run: its unit tests, both smoke tests, the menu,
  sidebar, minimap, selection of three soldiers in a cell, rank insignia, hidden submarines in
  rendering. They read `crates/game/src` only.
* Every match of computer players: `m6_ai`, `m6_production_matches`, the soak, the duels, the
  ten player sweep, so the match length figures of decisions R29, R44 and R53 were not
  reproduced; the unit tests inside `crates/sim/src` were read, not run, for RA4.
* `ai.rs` and `ai_defs.rs` beyond the controller's hash (advisory 1); `harvest.rs`, `props.rs`,
  `visibility.rs`; the loaders in `skirmish_defs.rs`, `combat_defs.rs` and `rules.rs` beyond
  divisor and limit ranges.
* The hash trace of decision R6 (that worlds without the infantry trait hash as before) was not
  repeated.
* Whether the new missile sprites of RA4 come from the repository's generators (needs Blender).
* Engine facts that only the original game settles: the unit of `JumpjetSpeed`, `GameSpeedBias`,
  arcing projectile speed, flak scatter factors, the elite weapon fallback, whether walls can be
  sold, whether a cut volley costs ammunition, whether a spawner in a transport is reset, and
  whether the reference `rules.ini` is that of patch 1.006.

### 3.4 Decisions that the prompt or their own text hand to a reviewer or to the user

| Decision | What needs judging |
|---|---|
| R6 | The amendment of invariant I6 to I6' (three infantry to a cell). The prompt's fixed decision 6 requires "an explicit reviewer check". Advisory 1 looked for paths that break I6' and reported none, but that is not the binding check. |
| R7 | Speed converted from leptons per frame on the 15 frames to 20 ticks time base instead of the prompt's GI anchor. Advisory 1 judged the justification sound; the user has not confirmed that slower units at 1x are wanted. |
| R4 | Map growth with the player count became an option, default off, reversing a default the user asked for on 2026-10-10. |
| R29 | `MATCH_TICK_LIMIT` of the 756 match test doubled to 72,000 as a pacing factor of RA2 build times. Advisory 1 found that its measurements used other seeds than the test's (remediated by measuring the test's own matches: check that). |
| R8 | The soak plays its configurations on all cores, and since the pace halved most large map configurations no longer reach the end of a match in 36,000 ticks (advisory 1, tests P1-2). |
| R53 | The computer player's "last stand" (it sells its buildings when it can neither fight nor earn), added so that matches end. Is it reachable only when stated, and is it acceptable play? |
| R55 | The arrangement of the review itself (panel, advisory reviews), a departure from the prompt's "without any review between milestones". |
| R15, R21, R31, R40, R45, R54 and later "tests adapted" records | Each lists tests whose expectations changed because RA2 behaviour is intended, or that moved to fixture data. Advisory 1 and 2 checked those up to RA4 and found changes that the records did not list (remediated: check). |

### 3.5 What only the user can verify

Not review findings, and never a reason for NO-GO (PLAN.md section 12 item 2), but equally
unverified: every visual, audio and feel item. The display of the machine was locked for the
whole run, so nothing was seen on a screen; the menu was captured offscreen
(`docs/screenshots/ra1/`). The complete list is the closing section of the programme's part of
`docs/STATUS.md`.

## 4. Suggested order for the later review

1. Confirm the tree: `git status` clean on `ra2-parity`, and
   `sh scripts/validate_ra2_parity.sh review-1` exits 0 (about two hours and a half; the script
   keeps the display awake, because a display that sleeps closes the smoke tests' window).
2. Review the remediation of the advisory reviews first (3.2): it is small, it touched command
   handling everywhere, and it tells how far the earlier reports can be relied on.
3. Run the panel of `docs/prompts/ra2-parity-review/` over the whole diff, with the advisory
   reports as leads and with extra weight on RA5 to RA9, which nobody has examined.
4. Judge the decisions of 3.4.
5. Only then decide about merging and tagging.

## 5. Consequences of stopping here

* `main` is unchanged at `8230a95`. Everything is on `ra2-parity`.
* No tag `ra2-parity-reviewed` exists, and none should be created before a review decides.
* The STATUS entries say "committed and validated", never "reviewed" or "approved".

## 6. To be updated at the end of the implementation

The outcome of the remediation (counts of fixed, refuted and deferred findings), the final tip
commit, the final validation results and test count, the list of decision records of RA5 to RA9
that belong in 3.4, and any module that RA5 to RA9 added which the panel's slice texts do not
name.
