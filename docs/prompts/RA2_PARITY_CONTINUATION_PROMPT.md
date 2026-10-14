# Prompt: continue and finish the RA2 parity programme

You are the **orchestrator** of a run in the repository `/Users/xingyuwang/GameProjects/MyGame001`
that continues a programme which was stopped part way. The programme turns the game into a
single player skirmish game whose rules, units, buildings, economy, combat, specialist
abilities, superweapons, vision, controls, player settings and computer opponents are comparable
to the original Command & Conquer: Red Alert 2 (not Yuri's Revenge). Network play is a non goal.

Your job is to finish all of it: close the half finished milestone RA5, implement RA5's second
part and RA6, RA7, RA8 and RA9, and then have the whole programme reviewed independently, without
waiting for the user between milestones. Stop early only on the conditions of section 9.

The original prompt, `docs/prompts/RA2_PARITY_PROMPT.md`, remains binding. This prompt adds what
the first run learned and what has changed since; where the two conflict, this one prevails, and
`docs/DECISIONS.md` records why.

## 1. Read before doing anything

In this order:

1. `docs/prompts/RA2_PARITY_PROMPT.md`, all of it.
2. `docs/reviews/RA2-parity-review-requirements.md`: the exact state the first run left, what
   is implemented and validated, what is only partly validated, and what was never built.
3. `docs/RA2_PARITY_PLAN.md`: sections 1 to 4 (facts, conversions, architecture, equations),
   section 5 (one entry per milestone RA1 to RA9 with change list, files, equations, tests and
   acceptance criteria; **these entries are the specification of your remaining work**), section
   6, and appendix D (the researched engine rules, each with its ini keys, source and confidence).
4. `PLAN.md` sections 5, 6, 8, 9, 10, 11, 12 and 14 (equations, invariants I1 to I12 with I6
   amended to I6', test categories, validation, severities, failure conditions).
5. `docs/DECISIONS.md` from "RA2 parity programme" (R1 to R63). They are long; read at least the
   titles of all, and in full: R6, R7, R9, R10, R29, R53, R55 to R63, and every record of the
   milestone you are about to continue.
6. `docs/STATUS.md`, section "RA2 parity programme": every entry, especially their "Debt" and
   "Requires user verification" lists, and the last entry, "The run stopped during RA5".
7. `docs/reviews/RA2-parity-reviews-outstanding.md`, the five advisory reports
   `docs/reviews/RA2-parity-advisory-{1,2}-*.md`, and `docs/prompts/ra2-parity-review/`.
8. `README.md`, the module headers of every file in `crates/sim/src/`, `git status`,
   `git log --oneline 8230a95..ra2-parity`, `git tag`.

Treat every statement in those documents as a claim to check against the code before you rely
on it.

## 2. Where things stand (confirmed on 2026-10-13; verify)

* Branch `ra2-parity`, tip `d907bd9` plus the commit that adds this prompt. `main` is `8230a95`,
  untouched. No tag of this programme exists.
* RA0 to RA4 and the remediation of two advisory reviews (R56) are committed and were validated
  in full (13 commands; 925 tests passed, 0 failed, 0 ignored at the last full run, label
  `advisory-1`).
* RA5 items 1 to 4 (ore and gems in bails, growth and spread, the side's free miner, crates, the
  refinery refund `Soylent`) are committed in `9f625b3`, `c9e3604`, `212874a`, decisions R58 to
  R62. Of the validation only the duels (756 of 756 ended, slowest in tick 268,277 of 288,000)
  and formatting, `git diff --check`, clippy and the two regeneration checks were run. The test
  run, builds, smoke tests, benchmark, soak and package script were not.
* **The working tree is not clean:** `crates/game/src/battlefield.rs` holds an unfinished,
  uncommitted test module of 150 lines (tests of drawing gems, new ore and crates) left by the
  stopped implementer. It compiles and passes clippy; nobody has run it in a full test run.
* RA5 items 5 to 9, RA6, RA7, RA8 and RA9 do not exist. The final review was never run (the
  user stopped review work: R57, R63). `docs/RA2_GAP_ANALYSIS.md` and
  `docs/CONSTRUCTION_SYSTEM.md` are stale (they describe the game before the programme).

## 3. Fixed decisions for this run

1. Everything in section 2 of the original prompt stands (RA2 ruleset as default with the
   extensions as options; legacy armies only in fixtures; computer players allied against the
   human by default with free for all as an option; reference data never committed; original
   generated assets; invariants with only I6 amended, which is done; one branch; commit messages
   starting with the milestone name).
2. The decisions R1 to R63 stand unless you find one wrong; then record a new decision that
   supersedes it, with the evidence. In particular: R7 (speed is converted from leptons per frame
   with the 15 frames to 20 ticks time base, not the GI anchor of the original prompt); R4 (map
   growth with players is an option, default off); R29 (`MATCH_TICK_LIMIT` is 72,000 and must
   not be raised again); R55 (the final review is a panel, section 7 below).
3. Decision records continue at **R64**. Validation logs go to
   `docs/validation/ra2-parity/<label>/` with labels `ra5`, `ra6a`, `ra6`, `ra7`, `ra8`, `ra9`,
   `review-1` and so on.
4. **Advisory reviews during the run are not done** (the user stopped them, R57). The validation
   commands are the only gate between milestones; the one binding review comes at the end.
5. The user said "finish the rest of all the work". That includes the final review of section 7.
   If, when giving you this prompt, the user tells you to leave the review out, do everything
   else and write the review requirements' updates instead of reviewing.

## 4. How the first run worked, and what to keep doing

The first run used one orchestrator and one implementer subagent per milestone or part of a
milestone, with a fresh context each, briefed with the plan entry and a common set of rules.
That worked: each implementer took three to six hours and delivered its milestone validated.
Keep that arrangement. You stay responsible for every result: after each implementer, check the
commits, `git status`, the validation summary, that no test was deleted or ignored
(`grep -rn '#\[ignore' crates`, count of `#[test]` functions before and after), and read the
new decision records before you continue.

### 4.1 Rules every implementer must be given (copy them into each brief)

* Read the original prompt's sections 1, 2, 3, 6, 9 and 10; the plan's sections 1 to 4 and 6
  and the milestone's entry in section 5 with the equations it cites and appendix D; `PLAN.md`
  sections 5, 6, 8 and 14; the decision records named in the brief; the code it will change.
  Verify every path, symbol and assumption against the code; where the plan is wrong about the
  code, achieve the plan's intent and say so.
* Smallest coherent change; no unrelated refactoring; match the surrounding style.
* `crates/sim`: no rendering, windowing, audio or input dependency (I1); no floating point (I2);
  every new piece of state in `state_hash` (I3); ordered iteration, randomness only from
  `SimRng`, no wall clock (I4); state changes only through commands in `SimSchedule` (I5); I6'
  (at most one vehicle or three infantry in a surface cell, one aircraft in an air cell, nothing
  on a building: place or move units only through `Grid::room`, `register` and
  `standing_point`); credits never negative and exact charging (I7); no panic, `unwrap` or
  `expect` reachable from data or commands (I8); balance values only in data files, fixtures for
  balance independent tests (I10); external files checked before parsing (I11); checked or
  saturating arithmetic where overflow is possible (I12).
* Never delete, skip or weaken a test. A test whose expectation changes because RA2 behaviour is
  intended is updated with a line in the milestone's decision record naming it and why.
* Tests in every category of `PLAN.md` section 8, through `CommandQueue` and `SimSchedule`,
  shipped data for shipped behaviour, fixtures for balance independent behaviour. New test files
  are named `ra<N>_<topic>.rs`. Make tests able to fail: where cheap, mutate the rule and see a
  test fail (the advisory reviews found several tests that could not fail).
* Data: shipped data is generated by `scripts/generate_ra2_data.py` from the committed tables in
  `crates/sim/tests/reference/`; never hand edit generated files. A schema change increments
  `schema_version` and migrates every data file, fixture (`crates/sim/tests/fixtures/data/`
  including `legacy` and `ra1` to `ra5`) and the generator in the same change. Ported values
  carry `// [Section] Key=value`; the audit `crates/sim/tests/ra9_data_parity.rs` covers every
  ported field (its completeness guard fails on an unaudited field); intentional differences
  go to `ra2_differences.tsv` with a specific reason. Never copy the RA2 ini files or any text
  from them into the repository. Where a behaviour cannot be established from any source,
  choose the closest documented behaviour and record it as an assumption.
* Two rules about commands that the advisory reviews found broken and R56 fixed: a command
  never acts on, or on behalf of, an entity the same tick destroyed; a command naming an enemy
  thing the player does not see, or that is hidden (submerged, disguised, a Mirage tank), is
  rejected exactly as for an id that does not exist. `crates/sim/tests/ra1_commands.rs` walks
  every command variant: **every new command or `Interact` target kind must be added to it.**
* Computer players: all AI matches of the test suite, the soak and `m14_duels -- full` must end
  with a winner. Fix the computer player's play (with a regression test) or retune `ai.ron`;
  never raise a tick limit to hide a stall. Diagnose with `m14_duels -- trace <map>`. Report the
  distribution of match lengths against the limit (median, 90th percentile, maximum) as R29,
  R44 and R53 do.
* Assets: only original art and sound from the repository's generators, each new file in
  `assets/LICENSES.md`. Visual and audio quality is "requires user verification", never passed.
* Git: commit on `ra2-parity` in coherent commits whose message starts with the milestone name
  (`RA6: ...`) and ends with the co-author line the environment gives you; never amend, rebase,
  reset, stash someone else's work, force, push, tag or switch branches; add only the paths you
  changed.
* Storage: no clones or worktrees; nothing large under `/tmp`; plain `cargo` (target directory
  `target/` in the repository; no `CARGO_TARGET_DIR` is configured).
* Do not review or approve your own work; write "validated", never "reviewed" or "approved". Do
  not ask the user questions: decide, record, continue.
* Report at the end in at most 400 words: each item of the brief done or not and why, the
  validation summary with exact counts, decisions, assumptions, debt, items requiring user
  verification, and what the next milestone must know.

### 4.2 Facts the implementers need

* **Validation:** `sh scripts/validate_ra2_parity.sh <label> [first] [last]` runs thirteen
  commands (the eleven of the original prompt, then the two regeneration checks of R56) and
  writes `summary.txt` and one log per command. A full run took about 126 minutes at the last
  measurement: tests 54 (37 of them the 756 shipped matches), soak 43, duels 26 to 33. Run it in
  the background. A part of a milestone that is not its last part may skip the soak (command 9)
  and run it in the milestone's final full validation.
* **The display.** The smoke tests and the package script open a window; a display that sleeps
  or is removed closes it and fails the run ("Monitor removed"). The script keeps the display
  awake with `caffeinate`. In the first run the session's display was locked throughout: the
  smoke tests passed without a visible window, and window captures came out black (decision R20
  added `MYGAME_MENU_REVIEW=<dir>` for offscreen captures of the menu).
* **Reference data:** `~/Library/Caches/MyGame001/ra2-reference/rules.ini` and `art.ini`
  (provenance and hashes in R2; `scripts/ra2_reference.py` refuses other files);
  `python3 scripts/ra2_ini.py rules <Section> [Key]` prints values. If the cache is missing,
  fetch the two files again from the source R2 names into that directory, outside the
  repository.
* **APIs added so far that later milestones use:** `sim::check_buildable`, factory groups and
  `QueueKey { producer, lane }` (R10, R23); `PlayerState::powers` and `radar` (R24); the damage
  path through warheads with `Blast::source` for kill credit and `effects` in `combat.rs` (R33,
  R35); `SimEvent::UnitCrushed { by, .. }`; `sim::promote(world, unit, rank)` and
  `sim::set_bonus(world, unit, Multipliers { .. })` (per unit permille bonuses that multiply
  with the rank's; only trainable units; only from a `SimSchedule` system); hidden units in
  `SpecialState::hidden`; spawners and aircraft in `flight.rs`; ore and crates in `ore.rs` and
  `crates.rs`. Exclude anything moved without a step (teleport, spawn, dock, paradrop) in the
  computer player's `View::stays_visible`.
* **Fixtures with RA2 rules:** `crates/sim/tests/fixtures/data/ra2` to `ra5` with helpers in
  `crates/sim/tests/ra2_common` to `ra5_common`; extend the latest rather than copying again.
* **Limits:** `MAX_DATA_FILE_BYTES` is 128 KiB (R32); `arsenal.ron` was about 100 KiB after RA4:
  check early; raising the limit needs a decision record and tests that keep their meaning.
* **Match lengths are the fragile point.** After RA4 the ratio of match length to limit was
  median 0.17, 90th percentile 0.37, maximum 0.95; RA5-A's slowest duel is 0.93. Balanced wars
  on large maps end only when the ore runs out, and ore now regrows. RA8 (a computer player with
  the whole arsenal and superweapons) is meant to shorten them; until then, every milestone
  must keep them inside the limit by better play, not by a higher limit.
* **The M9 mechanisms still in force** for specialists and superweapons live in `special.rs`
  (about 1,300 dense lines before RA4; it has grown). 29 rows of `ra2_differences.tsv` name RA6
  as the milestone that removes them and 5 name RA5. The legacy fixture (no rules file, no
  `effects`) keeps M9's behaviour, and `m9_arsenal.rs` and `m11_repair.rs` run on it untouched.
* Known debt to keep in mind (details in the STATUS entries and R56): destroyed entities still
  act in the non command stages of their tick; specialist and transport mechanics have no tests
  on the shipped ruleset (deferred to RA6); the soak's 36,000 ticks no longer reach the end of
  most large map matches; the Allied computer player built no radar; at 1280 x 720 the in match
  sidebar does not fit.

## 5. The work, in order

Each step ends with all thirteen validation commands exiting 0 on its final tree (for a first
part of a milestone, all except the soak), commits, decision records, and for each finished
milestone an entry in `docs/STATUS.md` in the section "RA2 parity programme", in the form of the
earlier entries (what changed, validation table with exact counts, assumptions, decisions, debt,
requires user verification).

### Step 0. Close RA5 items 1 to 4

1. Look at the uncommitted test module in `crates/game/src/battlefield.rs`: run it; finish it if
   it tests something worth keeping (it was meant to test that gems, newly grown ore and crates
   are drawn), otherwise remove it, and say which in R64.
2. Do what the stopped implementer had left (its own words: "Fix the warning, compute the duel
   distribution, add it to the decision record, and commit"): record in R62 (or R64) the
   distribution of the duels' lengths against the limit.
3. Recompute independently a sample of RA5-A's values from the reference ini (bail values,
   storage, growth and spread periods and percentages, crate shares and arguments,
   `CrateRegen`, `CrateRadius`, `Soylent`), because nobody has.
4. Run the full validation (label `ra5a-close`). This is the first full validation of RA5-A:
   expect failures and fix them within RA5.

### Step 1. RA5 items 5 to 9 ("RA5-B")

The plan entry RA5 items 5 to 9 with plan sections 4.11 and 4.12: the neutral player (no team,
never defeated, never a winner) and the tech buildings Oil Derrick, Hospital, Airport and Tech
Outpost (capture by Engineer, `NeedsEngineer`, `Unsellable`, their effects; the Airport's
paradrop is wired in RA6); garrisons (only `Occupier` infantry, only neutral buildings, the
occupation weapons with the reload divided by the occupants, eviction and refused entry at
`ConditionRed`; the existing houses, city blocks and towers of `props.rs` become garrisonable and
stay destructible; choose their capacities from the reference's civilian buildings and record
it); destructible bridges (a bridge's strength from `BridgeStrength`, collapse to water, repair
huts that an entering Engineer uses to restore it, the option `bridge_destruction`); maps and
generators (`crates/game/examples/art/world_maps.rs`, `frontier_maps.rs`, `gen_maps.rs`; new
world objects are `look` features the game defines and the simulation knows, so the map format
stays at schema 2; record the allocation of feature values; the three small maps `demo`,
`crossroads`, `riverlands` are used by hundreds of tests by coordinate, so change them only where
no test is affected; `cargo test --release -p game --test m7_maps` checks symmetry, base sites,
ore and reachability; every map must still set up for 2 to 10 players); art from the generators.
The computer player must not fire at neutral objects unasked, must handle a destroyed bridge on
its route, and must still win where a bridge is the only land route. Then the full validation
(label `ra5`) and the RA5 status entry covering both parts.

### Step 2. RA6 specialists and superweapons

The plan entry RA6, the table of plan section 4.13 and appendix D topics 12, 13 and 14. Split it
in two implementers:

* **RA6-A, infantry and radiation:** GI deploy (`Para` weapon, stays deployed until ordered);
  Engineer defusing bombs; Attack Dog (instant kill by its weapon and warhead; disguise
  detection); Spy disguise and every infiltration effect (refinery theft by
  `SpyMoneyStealPercent`, power blackout by `SpyPowerBlackout`, superweapon reset, barracks and
  war factory veteran production through `sim::promote`, radar shroud reset, Battle Lab
  unlocking Chrono Commando and Psi Commando or Chrono Ivan and Yuri Prime with their weapons,
  abilities, build limits and prerequisites; one effect per entry); Tanya's C4 and `Sapper`
  (also against ships); Chrono Legionnaire erasure (`remaining = 10 x Strength`, frozen and
  invulnerable while erased, returns unharmed if firing stops) and the chrono move delay; Tesla
  Trooper charging Tesla Coils (two keep one firing under low power, three overcharge it); Crazy
  Ivan's bomb (400 through `IvanWH` after 600 ticks); Yuri's mind control (one unit, the first
  released when a second is taken, ends with the controller; immune types) and psi wave;
  Desolator's beam and deployed eruption; the radiation grid (levels per cell from a site's
  level and spread, summed up to `RadLevelMax`, decay in steps, damage every 21 ticks through
  `RadSite`, immune types), kept cheap: track active sites or cells, never scan the whole map;
  Terrorist.
* **RA6-B, vehicles, structures and superweapons:** Prism Tower support chain (up to 8, damage
  times `1 + 1.5 N`, supporters offline 80 ticks); Mirage Tank shown as a tree while still to
  players without a disguise detector; Terror Drone and Giant Squid as parasites (the depot
  removes a drone, a Dolphin's hit frees a ship, the squid destroys a ship at `ConditionRed`);
  the IFV weapon table by passenger `IFVMode` (appendix D); death weapons (Nuclear Reactor,
  Demolition Truck, Terrorist, Kirov crash); Chronosphere (3 x 3 area, vehicles of every owner,
  infantry die, land vehicles over water and ships over land destroyed), Iron Curtain (3 x 3,
  vehicles and buildings of any owner, 1000 ticks, infantry die), Lightning Storm (warning 333
  ticks, 240 ticks of bolts, enemy radar off), Nuclear Missile (launch, flight, `NukePayload`
  through `NUKE`, radiation), paradrops (America's command; the Airport of RA5-B); recharge
  times from `[SuperWeaponTypes]`, `IsPowered`, warnings to every player and timers visible to
  all; then the full validation and the RA6 status entry with a checklist of every row of gap
  analysis section 9 (closed, with its test, or a listed difference with its reason).

For both: replace the M9 stand-ins rather than adding beside them, and remove their rows from
`ra2_differences.tsv`; split `special.rs` into modules by mechanic, moving code without changing
it in a commit of its own before changing behaviour; all area damage through the RA3 damage
path; abilities are commands (`Deploy`, `Interact`, `Superweapon`, or new ones validated like
them, added to `ra1_commands.rs`); hidden things stay hidden in events, rejections and the
computer player's knowledge; add shipped ruleset tests for the specialist and transport
mechanics (the debt R56 deferred to RA6).

### Step 3. RA7 vision, interface and controls

The plan entry RA7 and section 4.14: shroud only vision by default (a menu row FOG OF WAR,
default OFF; with fog off an explored cell counts as visible for rendering, targeting, command
validation and the computer player's knowledge; hidden units stay hidden); the Gap Generator
(radius from the reference, re-shrouding every tick while powered for hostile players, a unit's
own sight still reveals); the Psychic Sensor (15 cells, disguise detection, enemy orders aimed
inside its radius shown to the owner: a simulation query and lines in the game); the Spy
Satellite (reveals all but gap areas; radar infiltration pointless); RA2 default hotkeys (Q W E
R tabs, K repair, L sell, G guard, X scatter, F follow, Z waypoint mode, S stop, D deploy, T type
select, P all combat units, H home, N and M next and previous, Ctrl+F1 to F4 bookmarks, Alt+digit
centring on a group, Ctrl force fire, Alt force move; attack move stays on `A` and also
Ctrl+Shift click; keys that move, such as sell from X to L, global rally from R, pause from P and
mute from M, get a decision record and a prominent README table; on a MacBook offer a second
binding where an F key is the only one); new orders `Guard`, `Scatter`, `Follow`, force fire,
force move, waypoint queues (validated like `Move` and `Attack`, state hashed, added to
`ra1_commands.rs`; choose the guard range from the reference and record it); context sensitive
right click (Engineer, Spy, infantry onto transports and garrisons, vehicles onto a depot, Tanya
onto buildings and ships); an announcer of original synthesised cues and text notices (unit
ready, construction complete, on hold, cannot deploy here, base under attack, unit lost, low
power, insufficient funds, superweapon built, ready and launched, radar online and offline),
rate limited, made in code like the existing sounds. Tests for every binding's input to command
mapping; smoke test with fog off and on. Full validation, RA7 status entry.

### Step 4. RA8 the computer opponent

The plan entry RA8 and section 4.15, appendix D topic 17: plans per side and country in
`ai.ron` (the whole tech tree including radar and Battle Lab, defences up to the side's count for
the difficulty, the national special, a superweapon when the option is on, depot, purifier or
vats, a second refinery, repairing its buildings); an army by tier (artillery behind tanks, anti
air once it has seen aircraft, dock aware aircraft, ships where shipyard water is within reach
and the enemy can be reached or hurt from the sea, a detector when it lost units to something
unseen); engineers capturing seen tech buildings and repairing bridges on its route; garrisons
near its base; superweapons fired at the best known target (the ini's `AIIonCannon*Value`
weights), Iron Curtain and Chronosphere on its attack wave, paradrops behind the enemy base;
difficulty multipliers from `rules.ron` (Brutal uses `[Easy]` and Easy uses `[Difficult]` by
ModEnc: reload, build time, five times the starting credits for Brutal, ore income bonus, wave
interval, defence counts), as hashed per player state applied in production, combat and
harvesting; honest vision kept (a listed difference to RA2's all seeing AI); the AI debt of
earlier milestones. It acts only through commands, a bounded number per turn, and stays cheap
(mean tick under 5 ms with ten players on 256 x 256, `m13_ten_players -- to-the-end all` must
end). Report match lengths before and after. Full validation, RA8 status entry.

### Step 5. RA9 audit, balance and documents

The plan entry RA9: close the audit (every key of `ra2_reference.tsv` for every mapped section
is either audited against a shipped value or on an explicit list of unused keys with a reason;
the audit fails on unexplained, stale or uncovered entries); review every row of
`ra2_differences.tsv` (remove what a milestone should have removed; make each remaining reason
specific); a balance and soundness pass over all recorded debt, fixing defects with regression
tests (especially matches near their tick limit); rewrite `docs/RA2_GAP_ANALYSIS.md` claim by
claim against the code so that it describes only the remaining differences (with the corrected
facts of the plan's appendices: 43 units and 29 buildings, the RA2 side corrections), keeps the
features this game has that RA2 lacks with how each is defaulted, and separates assumptions;
rewrite `docs/CONSTRUCTION_SYSTEM.md` for the RA2 ruleset with the catalogue generated from the
data by a script; update `README.md` (check every statement against the code), the top of
`docs/HANDOFF.md`, `assets/LICENSES.md`; update
`docs/reviews/RA2-parity-review-requirements.md` and `RA2-parity-reviews-outstanding.md` to the
finished state; a STATUS entry for RA9 and a closing section with the cumulative validation,
every assumption, all current debt, and the complete checklist of visual, audio and feel items
that require user verification, each with how to see it in the game. Full validation (`ra9`).

## 6. Final review (the original prompt's section 7, as arranged by R55)

The user asked for a review round to fit in three hours (R55). Run it as a panel with the briefs
in `docs/prompts/ra2-parity-review/` (procedure in its README): with a clean tree at the tip,
start `sh scripts/validate_ra2_parity.sh review-1` in the background and at the same moment five
independent slice reviewers (fresh contexts that never implemented or remediated anything:
`slice-sim1.md`, `slice-sim2.md`, `slice-data.md`, `slice-tests.md`, `slice-client.md`), each
for at most 120 minutes; then a lead reviewer (`lead.md`) who checks the validation logs,
verifies every P0 and P1 the slices report, writes `docs/reviews/RA2-parity-review-1.md` and
decides GO, GO WITH DOCUMENTED NON-BLOCKING DEBT or NO-GO by the rules of the original prompt's
section 7. Before using the briefs, update their module lists to the finished tree (RA5-B to RA9
added modules they do not name) and give every reviewer
`docs/reviews/RA2-parity-review-requirements.md` as a list of requirements to cover; its
section 4 names checks that must not be skipped (among them the explicit check of the I6
amendment R6 that the original prompt's fixed decision 6 requires, the doubled tick limit R29,
and the speed conversion R7).

On NO-GO: a remediation agent (fresh) treats each finding as a claim, fixes or refutes it with
evidence, adds a regression test for every real defect and reruns the validation; then a new
panel reviews the full diff and writes the next review file; at most three remediation rounds.
On GO or GO WITH DEBT: merge `ra2-parity` into `main`, tag `ra2-parity-reviewed`, and record the
P2 debt in `docs/STATUS.md`. Never approve the work yourself; only the user creates an approval
tag.

## 7. Final message to the user

One message at the end: the final review decision; what now matches RA2; every remaining
intentional difference; the validation results; the debt; the decisions that reverse or
reinterpret what the user asked for (R4, R7 and any new one); and the checklist of visual, audio
and feel items that require user verification. Nothing visual or audible is reported as passed:
the first run never saw the game on a screen.

## 8. Validation commands (all must exit 0)

The thirteen of `scripts/validate_ra2_parity.sh`: the eleven of the original prompt's section 9,
and `python3 scripts/generate_ra2_data.py --check` and `python3 scripts/ra2_reference.py --check`.
Keep logs under `docs/validation/ra2-parity/`. No clones, worktrees or large files in `/tmp`.

## 9. Stop and ask the user when

A validation command fails and cannot be fixed without abandoning a milestone's intended
behaviour; an invariant of `PLAN.md` section 6 would have to be broken; the reference data cannot
be obtained; a fixed decision proves unworkable; the benchmark budget (mean tick 5 ms) or soak
determinism cannot be restored; computer player matches cannot be made to end within the limits
without raising them; or the third remediation round ends NO-GO. When you stop, report the
evidence and the options. Otherwise continue to the end.
