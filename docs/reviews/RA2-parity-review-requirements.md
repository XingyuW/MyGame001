# RA2 parity programme: requirements for the review that is still to be done

Written 2026-10-13 by the orchestrator of the run, at the user's instruction ("Stop where you are
here, and write a comprehensive review requirement for later review"). The orchestrator is not a
reviewer and this document approves nothing. It tells a later reviewer what exists, what state
it is in, what must be checked, how, and by what rules the result is to be judged.

Related documents: the programme's prompt (`docs/prompts/RA2_PARITY_PROMPT.md`), its plan
(`docs/RA2_PARITY_PLAN.md`), the decision log (`docs/DECISIONS.md`, R1 to R63), the status entries
(`docs/STATUS.md`, section "RA2 parity programme"), the shorter account of what was and was not
reviewed (`RA2-parity-reviews-outstanding.md`), the advisory reports
(`RA2-parity-advisory-{1,2}-*.md`), and ready made briefs for a review by a panel
(`docs/prompts/ra2-parity-review/`).

## 1. What there is to review

### 1.1 The run was stopped part way

The programme was planned as milestones RA0 to RA9 followed by one independent review. The user
stopped review work on 2026-10-13 (decision R57) and then the implementation, while the first
part of RA5 was being finished (decision R63). The state is:

| Milestone | Scope (plan section 5) | State |
|---|---|---|
| RA0 | baseline committed unchanged, plan, reference data | complete, validated |
| RA1 | sides, nine countries, RA2 match options, MCV start, win conditions | complete, validated |
| RA2 | production by rate, lanes, build limits, power, radar, placement, sell, repair, depot | complete, validated |
| RA3 | eleven armour types, warheads, weapons, one damage path, crushing, infantry sub cells (I6') | complete, validated |
| RA4 | veterancy, crews, aircraft docks and ammunition, spawned missiles and Hornets, submerged units | complete, validated |
| Remediation | fixes for the findings of advisory reviews 1 and 2 (decision R56) | complete, validated |
| RA5 items 1 to 4 ("RA5-A") | ore and gems in bails, growth and spread, the side's free miner, crates, the refinery refund | **committed, only partly validated, agent stopped before it finished** (1.3) |
| RA5 items 5 to 9 ("RA5-B") | neutral player, tech buildings, garrisons, destructible bridges, map objects | **not implemented** |
| RA6 | specialists and superweapons (every row of gap analysis section 9) | **not implemented** |
| RA7 | shroud only vision, gap generator, sensors, RA2 hotkeys, new orders, announcer | **not implemented** |
| RA8 | the computer opponent with the full arsenal and difficulty multipliers | **not implemented** |
| RA9 | completed parity audit, rewritten gap analysis, rewritten construction document | **not implemented** |
| Final review (prompt section 7) | | **not done**; no decision exists |

"Validated" means only that the validation commands of section 2.2 exited 0 on the milestone's
final tree, run by its implementer. Nothing is reviewed or approved.

### 1.2 Commits

Base: `8230a95` (`main`, unchanged). Branch: `ra2-parity`, 53 commits and one commit of documents
after them. Whole diff `8230a95..ra2-parity`: 720 files, 146,876 insertions. Of that the
programme's own code, scripts and data (`21225b0..`, paths `crates`, `scripts`, `assets/data`) are
180 files, 64,134 insertions and 3,562 deletions.

| Range | Content |
|---|---|
| `116ffdd`..`257a169` (5 commits) | the user's earlier uncommitted work, committed unchanged ("RA0 baseline: ..."). Context, not the programme's work; regressions the programme caused in it are in scope. |
| `21225b0` | RA0: plan, gap reconciliation, R1 to R8, the soak's parallel run and accepted placement rejection, the validation script |
| `13ebadc`..`58e25af` | RA1 (reference tables; simulation and data; game client) |
| `a3e5471`..`296303d` | RA2 |
| `1160c41`..`d43169b` | RA3-A (armour, warheads, weapons, damage path) |
| `f03aecd`..`46e694d` | RA3-B (infantry sub cells, crushing, I6') |
| `598db00`..`6a41bc3` | RA4 |
| `ba26de1`, `42f15c4` | R55 and the advisory reports |
| `43b5ef1`..`a9321c1` | remediation of advisory reviews 1 and 2 (R56) |
| `2292fcf` | R57 and the first report of outstanding review work |
| `9f625b3`, `c9e3604`, `212874a` | RA5-A (R58 to R62) |
| the commit after `212874a` | this document, R63, the status entry of the stop, the logs of RA5-A's one validation command |

### 1.3 The state of RA5-A exactly

Confirmed facts (checked by the orchestrator on 2026-10-13 without changing any code):

* Three commits are on the branch (`9f625b3`, `c9e3604`, `212874a`): `economy.ron` schema 2 and
  `rules.ron` schema 2, `crates/sim/src/ore.rs` and `crates.rs`, changes to `harvest.rs`,
  `map.rs`, `map_scale.rs`, `match_setup.rs`, `special.rs`, `visibility.rs`, gems as a `look`
  feature on five large maps (regenerated TMX and tilesets), drawing of gems, new ore and
  crates, five new test files (`ra5_ore.rs`, `ra5_crates.rs`, `ra5_definitions.rs`,
  `ra5_shipped.rs`, game `ra5_gem_maps.rs`), the `Soylent` refund of the refineries, decisions
  R58 to R62.
* On the working tree as left, these exit 0: `cargo fmt --all -- --check`, `git diff --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `python3 scripts/generate_ra2_data.py --check`, `python3 scripts/ra2_reference.py --check`.
* `m14_duels -- full` exited 0 during RA5-A's work (`docs/validation/ra2-parity/ra5a/`: 756
  matches, 756 ended, the slowest in tick 268,277 of a limit of 288,000). The log is from 05:16,
  after the last commit of 04:42, so it ran on the committed code or on that code plus
  uncommitted changes; which is not established.
* **Not run on this tree by anyone:** `cargo test --workspace --release` (the suite has 979 test
  functions at the tip; the last full run, before RA5-A, had 925 passed), the release build, both
  smoke tests, the benchmark, the soak, the package script. Whether RA5-A's own new tests pass in
  a full run is therefore not established here; the agent ran them while developing, by its
  commit messages, but no log is kept.
* The agent was stopped while it had four things left, in its own last words: "Fix the warning,
  compute the duel distribution, add it to the decision record, and commit." So decision R62
  lacks the distribution of match lengths that R29, R44 and R53 give, and no status entry
  exists for RA5 (RA5-A's brief left that to RA5-B).
* **The working tree is not clean.** `crates/game/src/battlefield.rs` holds an uncommitted,
  unfinished test module of 150 lines (it compiles and passes clippy; it was not run in a full
  test run). It is not part of any commit. A reviewer should either review the committed tip and
  set the file aside (`git stash` or `git diff > file`), or finish it deliberately; it must not
  be lost or committed unread.

Assumption, not checked: that RA5-A's work is complete for its four items. Its decision records
R58 to R62 describe it as such; no report from the agent exists, because it was stopped before
reporting.

### 1.4 What the game is, as a consequence

A reviewer must not judge the branch against the whole prompt as if it were finished. With RA5-B
to RA9 missing, the following of the prompt's goal is **known to be absent** and is not a
finding:

* Tech buildings, garrisons, destructible bridges and their map objects (RA5-B); crates and gems
  exist, but only five large maps have gems.
* Every RA2 specialist and superweapon mechanic (RA6): the M9 stand-ins of `special.rs` are
  still in force with RA2 warheads where RA3 gave them one; 29 of the 53 rows of
  `crates/sim/tests/reference/ra2_differences.tsv` name RA6 as the milestone that removes them,
  and 5 name RA5.
* Vision is still shroud plus fog of war; hotkeys are the game's own, not RA2's; there is no
  guard, scatter, follow, force fire, waypoint mode or announcer (RA7).
* The computer player plays ground units and base defences of its side only; no aircraft,
  ships, specialists, superweapons or difficulty multipliers (RA8). The option names Easy,
  Medium and Brutal exist with the parameters of the game's own AI file.
* The data parity audit covers what RA1 to RA5-A ported; it is not closed. `docs/RA2_GAP_ANALYSIS.md`
  is **the old document of 2026-10-11** and is now wrong in many rows (sides, production,
  armour, veterancy and the ore economy have changed); `docs/CONSTRUCTION_SYSTEM.md` still
  describes M9. `README.md` was updated by each finished milestone and not for RA5-A.

What a review of this branch can decide is therefore whether **RA0 to RA4, the remediation and
RA5-A** are sound as an intermediate state. Whether to merge an intermediate state is the
user's decision (section 7).

## 2. Before reviewing

### 2.1 Environment

* macOS on Apple Silicon, Rust 1.98.1 (`rust-toolchain.toml`), Bevy 0.19.1. No `CARGO_TARGET_DIR`
  is configured; builds go to `target/` (decision R3).
* The RA2 reference files are not in the repository (fixed decision 4). They are in
  `~/Library/Caches/MyGame001/ra2-reference/` (`rules.ini`, `art.ini`; provenance and SHA-256 in
  decision R2). `python3 scripts/ra2_ini.py rules <Section> [Key]` prints values. If the cache
  is gone, obtain the files again as R2 says and compare the hashes; `scripts/ra2_reference.py`
  refuses files with other hashes.
* The smoke tests and the package script open a window. A display that sleeps or is removed
  closes it and fails the run ("Monitor removed", RA0). The validation script keeps the display
  awake with `caffeinate`. During the whole run the session's display was locked: the smoke
  tests passed without a visible window, and **nobody has seen the game on a screen**.

### 2.2 The validation commands

`sh scripts/validate_ra2_parity.sh <label> [first] [last]` runs them in order and keeps one log
per command and `summary.txt` under `docs/validation/ra2-parity/<label>/`. Read the script first.

| # | Command | Time at the last full run |
|---|---|---|
| 1 | `cargo fmt --all -- --check` | seconds |
| 2 | `git diff --check` | seconds |
| 3 | `cargo clippy --workspace --all-targets -- -D warnings` | seconds when cached |
| 4 | `cargo test --workspace --release` | 54 minutes (37 of them the 756 shipped matches) |
| 5 | `cargo build --release -p game` | seconds when cached |
| 6 | `cargo run --release -p game -- --smoke-test 600` | 16 s |
| 7 | `cargo run -p game -- --smoke-test 600` | 2 minutes |
| 8 | `cargo run --release -p sim --example m7_benchmark` | seconds |
| 9 | `cargo run --release -p sim --example m7_soak` | 43 minutes |
| 10 | `cargo run --release -p sim --example m14_duels -- full` | 26 minutes (33 in RA5-A) |
| 11 | `sh scripts/package_macos.sh` | 20 s |
| 12 | `python3 scripts/generate_ra2_data.py --check` | under a second (added by R56) |
| 13 | `python3 scripts/ra2_reference.py --check` | under a second (added by R56) |

History of full runs (all commands exit 0 in each; logs in the named directories):

| Label | Tree | Tests passed (0 failed, 0 ignored in each) |
|---|---|---|
| `ra0-baseline` | baseline | 529 |
| `ra1` | RA1 | 626 |
| `ra2` | RA2 | 690 |
| `ra3` | RA3 | 787 |
| `ra4` | RA4 | 893 |
| `advisory-1` | after the remediation | 925 |
| `ra5a` | RA5-A | **only command 10 was run** |

### 2.3 First step of any review (requirement V1)

**V1.** With the working tree clean at the commit to be reviewed (1.3 on the uncommitted file),
run all thirteen commands and record exit statuses and test counts. Any command that does not
exit 0 is a blocking finding. This run is the first evidence about RA5-A that will exist.

## 3. Rules the review is judged by

From `PLAN.md` sections 10 and 12 and the prompt's section 7:

* **P0:** crash, data loss, broken build, violated invariant, incorrect simulation result.
  **P1:** missing acceptance criterion, inadequate test for specified behaviour, silent fallback,
  unverified assumption (and, by the programme's rules, a deleted, skipped or weakened test, or
  RA2 material in the repository). **P2:** maintainability, naming, documentation debt.
* **Decision:** GO; GO WITH DOCUMENTED NON-BLOCKING DEBT (only P2, each recorded in
  `docs/STATUS.md`); NO-GO, mandatory for any unresolved P0 or P1, failed validation command,
  unverified acceptance criterion, unresolved numerical, scientific or data integrity risk, or
  broken compatibility.
* The reviewer must not have implemented or remediated the code. Every statement of the
  programme's own documents, including this one, is a claim to check.
* Items only the user can verify (visual, audio, feel) are listed, are not findings, and never
  justify NO-GO.
* On NO-GO: a remediation by a fresh agent that treats each finding as a claim, fixes or refutes
  it with evidence and adds a regression test for every real defect; then a new independent
  review of the full diff; at most three rounds.
* Output: `docs/reviews/RA2-parity-review-<n>.md` in the form of the earlier reviews
  (`M7-review-1.md`): scope, method, validation table as observed, findings with file, line,
  evidence and expected behaviour, what was verified and found correct, what could not be
  verified, items for the user, decision.
* Because the programme is incomplete, the review file must state its scope as "RA0 to RA4,
  the remediation, and RA5 items 1 to 4" and must not be read as a review of RA2 parity.

Invariants in force (`PLAN.md` section 6 and 12): I1 no rendering, windowing, audio or input
dependency in `sim`; I2 no floating point in simulation state and logic; I3 determinism, with
every piece of state in `state_hash`; I4 no dependence on hash map order, system ambiguity or
wall clock; I5 state changes only through `CommandQueue`; **I6' (amended by R6): a surface cell
holds at most one vehicle or at most three infantry and never both; an air cell holds at most
one aircraft; no unit occupies a cell covered by a building**; I7 credits never negative and the
charge for a completed item equals its cost; I8 no panic, `unwrap` or `expect` reachable from
data files or commands; I9 pinned dependencies; I10 balance values only in data files, fixture
data for balance independent tests; I11 external files checked against a supported subset
before parsing; I12 explicit checked, saturating or wrapping arithmetic.

## 4. Requirements

Each requirement has an identifier for the review file to cite. "Verify" means by reading the
whole code path or by running something, not by reading a status entry.

### 4.1 Acceptance criteria of the implemented milestones (A)

For every row: find the production path (a command through `CommandQueue`, a system of
`SimSchedule`) and the test that exercises it through that path with the stated data; a
criterion without such a test is P1.

| ID | Criterion (plan section 5) | Where the programme says the evidence is |
|---|---|---|
| A1.1 | Costs, strength, sight and prerequisites of every item equal the reference or are listed differences | `ra9_data_parity.rs`, `ra2_differences.tsv`, R9 |
| A1.2 | The default match is RA2's: 10,000 credits, an MCV, 10 units, short game, superweapons on, MCV repacks | `ra1_match.rs`, `MatchOptions::default`, R13; game: menu defaults, R17 |
| A1.3 | No legacy item can be built in a default match | `ra1_buildability.rs`, `ra1_catalogue.rs` |
| A1.4 | The buildability rule (prerequisites and groups, factory of the kind and owner side, required and forbidden countries), including capture | `ra1_buildability.rs`, R10 |
| A1.5 | Short and long game win conditions; undeploy and redeploy | `ra1_match.rs`, R12, R13 |
| A1.6 | The 756 side pairing matches end with a winner | `m6_production_matches.rs`, `m14_duels`, R5, R14 |
| A2.1 | Build time of every item is `round(cost x 21 / 25)`, walls times 3 | audit; `ra2_production.rs`, R22, R23 |
| A2.2 | Multiple factories, the two Construction Yard lanes, unit queues of 1 + 29, build limits | `ra2_production.rs` |
| A2.3 | I7 holds with every rate (exact charging, pause, cancel refund) | `ra2_charging.rs`; advisory 1 called its "property test" one cost (R56 says fixed) |
| A2.4 | Graded low power; only powered buildings that drain power stop; superweapon charging; radar and the minimap | `ra2_power.rs`, R24; game smoke check (advisory 1 found it could not fail; R56 says fixed) |
| A2.5 | Placement by `adjacent` and `base_normal`; the shipyard's kept reach as a listed difference | `ra2_placement.rs`, R25 |
| A2.6 | Refund, repair steps and cost, unsellable walls, depot docking for credits | `ra2_repair.rs`, R26, R27 |
| A3.1 | Damage, range and rate of fire of every shipped weapon match the reference through the conversions | audit; `ra3_shipped.rs` (990 weapon and armour pairs), R32, R34 |
| A3.2 | The damage equation with falloff, one hit per footprint cell, the minimum of 1, `MaxDamage` | `ra3_damage.rs`, R33 |
| A3.3 | All area damage goes through the armour table | `ra3_damage.rs`, R35 (worlds without `effects` keep M9's blast: check the boundary) |
| A3.4 | Weapon selection, layers by projectile flags, `Verses` 0 and 1, burst, minimum range, air range bonus, scatter | `ra3_weapons.rs`, R34 |
| A3.5 | Damage states at 50% and 25%, three quarter speed of damaged vehicles and ships | `ra3_weapons.rs`, R37 |
| A3.6 | Crushing: only hostile crushable infantry, only on move and attack move | `ra3_crush.rs`, R42 |
| A3.7 | I6' on every tick; worlds without the infantry trait unchanged | `ra3_cells.rs`, the soak's check, R6 |
| A4.1 | Ranks at 3 and 6 times own cost, inclusive; each multiplier; abilities once; elite weapons; self heal | `ra4_veterancy.rs`, R47 |
| A4.2 | Crews and survivors (only `Crewed` types; numbers; free spots) | `ra4_crew.rs`, R48 |
| A4.3 | Ammunition, four docks, `NoFreeDock`, return and reload, loss of the dock | `ra4_aircraft.rs`, R49 (advisory 2: loss by capture untested, and the ammunition defect; R56 says fixed) |
| A4.4 | V3 rocket, Dreadnought missile, Hornet, Osprey as air units that can be shot down; launcher credit; regeneration | `ra4_spawns.rs`, R50 |
| A4.5 | Submerged units hidden unless detected or recently fired, per player, in targeting, commands and the computer player's knowledge | `ra4_cloak.rs`, R51 |
| A4.6 | "An elite unit in a long battle" (integration) | advisory 2 found it missing; R56 says added |
| A5.1 | A full War Miner pays 1000 for ore and 2000 for gems; storage 40 and 20 bails; the side's free miner | `ra5_ore.rs`, `ra5_shipped.rs`, R58 |
| A5.2 | Ore regrows and spreads on schedule from `SimRng`; gems do not; maximum density | `ra5_ore.rs`, R59 |
| A5.3 | Each crate type's effect, shares, regeneration, maximum, `FreeMCV`, crates off | `ra5_crates.rs`, R60 |
| A5.4 | Maps: gems placed symmetrically, every map still sets up for 2 to 10 players | game `ra5_gem_maps.rs`, `m7_maps.rs`, R61 |
| A5.5 | The refund of a sold refinery (`Soylent`) | audit, R58 |

The plan's "Tests" paragraphs list more cases per milestone (positive, negative, boundary,
regression, integration, end to end); **A6:** check that each listed case exists and that every
category of `PLAN.md` section 8 is present for each milestone.

### 4.2 Determinism and hashing (D)

* **D1.** List every field of simulation state added or changed since `8230a95` and confirm it
  is in `state_hash` in a fixed order. At least: `PlayerState` (country, options, `powers`,
  `radar`, repair set, queues with `QueueItem::fraction`, `QueueKey` with lane), `MatchState`
  and options, the grid's spots and air layer, weapon burst state, damage states, `Experience`
  (value, rank, per unit multipliers), ammunition and docks, spawner and missile state
  (`flight.rs`), crew bookkeeping, `SpecialState` (docked units, hidden sets, everything else
  it grew), the computer player's controllers (including "last stand" state), ore kind and
  bails per cell, ore growth timers, `Crates` (positions, timers, per unit crate multipliers).
* **D2.** Iteration that affects state is over ordered collections or entities sorted by
  `SimId`; randomness only from `SimRng`, drawn in a fixed order (scatter, crews, random
  countries, ore growth and spread, crate type and place); no floating point in `crates/sim`;
  no wall clock.
* **D3.** R56 changed the grid hash so that it "writes its lengths only when there are units on
  foot, because an existing test pins the pre-RA3 hash". Judge whether that is a sound hash
  (can two different states collide through it?) or a way of keeping an old expectation.
* **D4.** The soak compares hashes of two runs on every tick for 260 configurations, but since
  the pace halved most large map configurations no longer reach a match's end in 36,000 ticks
  (advisory 1). R56 added per tick invariant checks to the 756 shipped matches but no hash
  comparison to their end ("late game hash comparison" is recorded debt). Decide whether
  determinism of late game states (superweapon era does not exist yet; veterancy, crews, spawned
  units, ore growth and crates do) is sufficiently shown, for example by running a sample of
  the shipped matches twice and comparing hashes per tick.

### 4.3 Invariants and arithmetic (I)

* **I1.** I6' cannot be broken by any path: every caller of the grid's entry rule
  (`Grid::room`, `register`, `standing_point`, release) in movement, exits from factories,
  transports unloading, deploy and undeploy, teleport, crews and survivors, spawned units,
  crate units, crushing, harvesting. `sim::cell_invariant_violations` and `stray_claims` are the
  checks the soak and the shipped matches use: read them for what they would miss.
* **I2.** I7 under: rate based production with fractions, both lanes, cancel and refund, repair
  steps, depot repair, sell refunds (now per type for the refineries), survivors, ore income in
  bails with the purifier bonus, crate money, spy theft.
* **I3.** I8 and I12 in all new code: no `unwrap`, `expect`, panic, unchecked indexing or
  unchecked arithmetic reachable from a data file or a command. The data files grew to 128 KiB
  (`MAX_DATA_FILE_BYTES`, R32): the bound and its tests must still mean something.
* **I4.** I5: nothing in `crates/game` mutates simulation state except through commands (D34);
  the computer player issues only commands, a bounded number per turn.
* **I5.** I1 and I9: `crates/sim/Cargo.toml` and `Cargo.lock` unchanged in dependencies.

### 4.4 Commands, events and hidden information (C)

* **C1.** The command set is `Deploy`, `Undeploy` (new), `Interact`, `Superweapon`, `Move`,
  `Stop`, `Harvest`, `QueueProduction`, `CancelProduction`, `PlaceBuilding`, `SellBuilding`,
  `RepairBuilding`, `SetRallyPoint`, `Attack`, `AttackMove`. New reject reasons include
  `NoFactoryOfSide`, `CountryRequired`, `CountryForbidden`, `DisabledByOptions`,
  `BuildLimitReached`, `NotSellable`, `NotRepairable`, `NoFreeDock`. For each: rejected with no
  state change, and tested positive, negative and at the boundary.
* **C2.** Two rules were found broken by the advisory reviews and are recorded as fixed
  (R56): (a) a command never acts on, or on behalf of, an entity the same tick destroyed;
  (b) a command naming an enemy thing the player does not see, or that is hidden (submerged,
  disguised), is rejected exactly as for an id that does not exist. `ra1_commands.rs` is said to
  walk every command variant. Verify both rules independently for every command and for every
  `Interact` target kind, including what RA5-A added (crates are entered by moving; check that
  no rejection or event reveals a crate, ore or unit in ground the player does not see).
  Recorded debt: "destroyed entities still act in non command stages of their tick": judge it.
* **C3.** Events: every new `SimEvent` variant carries what the interface needs and is not
  shown to a player who must not learn of it (D33). Crate pick ups, promotions, crews, crushes.

### 4.5 Data, schemas and numerical correctness (N)

* **N1.** Schema versions now: `units.ron` 1, `economy.ron` 2, `combat.ron` 3, `arsenal.ron` 5,
  `skirmish.ron` 3, `ai.ron` 2, `rules.ron` 2. For each bump: every shipped file, every fixture
  (`crates/sim/tests/fixtures/data/` including `legacy`, `ra1` to `ra5`) and the generator were
  migrated in the same change; an old version is rejected with a typed error naming file, field
  and reason; every field is required, range checked and cross validated; unknown fields are
  rejected. The definition tests are `m2`, `m3`, `m4`, `m6_definitions.rs` and
  `ra1` to `ra5_definitions.rs`.
* **N2.** Conversions (prompt section 3, plan sections 2 and 4): ticks = round(frames x 4 / 3);
  game minutes x 1200; cells x 256; build time round(cost x 21 / 25); damage
  floor(base x verses / 100) with the multipliers in the order of plan 4.3. **Speed is not the
  prompt's anchor**: decision R7 converts leptons per frame (`min(Speed x 256 div 100, 255)`,
  times 3/2 for infantry on foot, `JumpjetSpeed` taken as leptons per frame) by 3/4. Advisory 1
  judged R7 justified; judge it again, and its consequence (units slower at 1x than before).
* **N3.** Independently of the generator (`scripts/generate_ra2_data.py`) and of the audit,
  recompute a sample of shipped values from the reference ini (at least 100, spread over units,
  buildings, weapons, warheads, projectiles, `rules.ron`, veterancy, crews, aircraft, spawns,
  ore and crates). Advisory 1 and 2 recomputed 3,805 and 1,032 values up to RA4 and found no
  mismatch outside the listed differences; **nothing of RA5-A has been recomputed by anyone
  independent**: bail values, storage, growth and spread periods and percentages, crate shares
  and arguments, `CrateRegen`, `CrateRadius`, `Soylent`.
* **N4.** The audit (`crates/sim/tests/ra9_data_parity.rs` with the tables in
  `crates/sim/tests/reference/`): does it recompute independently of the generator; does its
  completeness guard (added by R56) really fail on an unaudited field (mutate one); is every one
  of the 53 rows of `ra2_differences.tsv` an honest intentional difference with a specific
  reason. The 34 rows that name RA5 or RA6 as the milestone that removes them are now permanent
  until someone resumes: they must be read as open work, not as accepted differences.
* **N5.** Commands 12 and 13 reproduce the committed data and table byte for byte; R56 added a
  recorded hash of the table (`ra2_reference.tsv.sha256`): check what it protects.
* **N6.** Assumptions carried in the data and rules, each labelled in plan section 4 and
  appendix D, need a decision whether they are acceptable as they stand: the infantry speed
  factor, `JumpjetSpeed`, shortage as `1 - output/drain`, the ten factory cap, burst gap of 5
  ticks, arcing shell speed, flak scatter factors, twelve density stages and one bail per 19
  ticks, spread above six bails, instant unloading, survivors from the sell cost, a cut volley
  costing its round, a bay's timers standing still aboard a transport, and the reference file
  being that of patch 1.006.

### 4.6 Test integrity (T)

* **T1.** No test deleted, skipped, ignored or weakened in `21225b0..tip`: compare the set of
  `#[test]` functions per file at `257a169` and at the tip, and every changed assertion, limit
  and constant in `crates/sim/tests/`, `crates/game/tests/`, unit tests in `src/`, the examples
  used as validation and the validation script. There is no `#[ignore]` in `crates` at the tip.
* **T2.** Every intentional expectation change has a decision record: R15 (legacy fixture and
  migrated tests), R21, R31, R40, R45, R54, R56 (three RA4 tests changed expectation or
  scenario), R62. Advisory 1 and 2 found changes that records did not list; verify the records
  are now complete, and review R62 for the first time.
* **T3.** `MATCH_TICK_LIMIT` of `m6_production_matches.rs` was doubled to 72,000 (R29) as the
  pacing factor of RA2 build times. It was justified with duels on another seed; R56 measured
  the test's own matches (seed 0: median 0.17, 90th percentile 0.37, maximum 0.95 of the
  limit). In RA5-A the slowest duel is 268,277 of 288,000 (0.93). Judge whether the limit is
  honest and whether matches this close to it make the suite fragile.
* **T4.** Tests that could not fail were found twice by the advisory reviews (a smoke check, a
  regression test that lost its failing condition). Look for more: run a handful of mutations
  of rules of each milestone (advisory 2 ran twenty on RA4; three survived and were reported)
  and see that a test fails. RA5-A has had no mutation testing.
* **T5.** The mechanics tests of M9 (`m9_arsenal.rs`, `m11_repair.rs`, game `m10`) run on the
  legacy fixture. R56 added a shipped ruleset catalogue test (every item of every country's tree
  built and sold through commands) and deferred the specialist and transport mechanics to RA6,
  which does not exist. So **specialist and transport mechanics on the shipped ruleset have no
  tests at all**: record it as a gap of the stopped state.

### 4.7 The computer player (P)

* **P1.** Every fix made to `ai.rs` during the programme has a regression test that can fail:
  armed miners drafted into the army and the sweep from its own base (RA1), both lanes and
  adjacency (RA2, R28), the last armed harvester and the teleporting miner's sight (RA3, R44),
  the "last stand" (RA4, R53), ore growing before the computer players look (RA5-A, R62).
* **P2.** The "last stand" (a player that can neither fight nor earn sells its buildings): is
  it reachable only when stated, bounded, and acceptable play. Advisory 2 suspected a case of a
  player with credits stalled on a building it cannot place.
* **P3.** Honest vision (D43): the controller reads only what its player's vision shows; it
  must not see submerged units or crates and ore in unseen ground.
* **P4.** Match lengths: reproduce the distribution for the tip (RA5-A's is not recorded).

### 4.8 The game client (G)

Nobody independent has built or run the game crate. Requirements:

* **G1.** The game's unit tests and `crates/game/tests/` pass (part of V1) and test what they
  claim (`ra5_gem_maps.rs` is new and unreviewed).
* **G2.** Menu to `MatchOptions` and `PlayerSetup` with RA2 defaults and the extensions off
  (R4: map growth default off; R17), restart replaying the choices, the PLAYERS panel.
* **G3.** Sidebar listing rule (R18) and lanes, radar and the minimap, three soldiers to a cell
  (selection, hover, health bars), rank insignia, hidden submarines neither drawn nor pickable,
  gems, ore that appears, crates and their notices.
* **G4.** The smoke flow (`smoke.rs`) checks what R20 and R56 say it checks; advisory 1 found
  that it bypasses input and that the README overstated it.
* **G5.** Open items the remediation recorded as not probed: card cost labels frozen after the
  first match, headless matches playing without scenery, two-line card names, the in match
  sidebar not fitting 1280 x 720 (pre-existing).

### 4.9 Provenance and assets (R)

* **R1.** No RA2 file and no text from one (ini comments, display strings) anywhere in the tree
  or the history of the branch; `ra2_reference.tsv` (4,888 rows) holds numbers, flags and
  identifiers only; data comments cite ini section and key (advisory 1 checked 2,280 citations).
* **R2.** Every asset file added is produced by a generator in the repository and listed in
  `assets/LICENSES.md`: the two missile sprites of RA4 (not confirmed against the generator:
  needs Blender), and RA5-A's gem, new ore and crate art and regenerated tilesets (unreviewed).

### 4.10 Documents and honest reporting (H)

* **H1.** Each status entry's numbers match the logs under `docs/validation/ra2-parity/`
  (advisory 1 and 2 confirmed this up to RA4).
* **H2.** `README.md` against the code (advisory 1 sampled about thirty rule values and found
  them right; it has no RA5-A content). `docs/RA2_GAP_ANALYSIS.md` and
  `docs/CONSTRUCTION_SYSTEM.md` are stale by design of the stop (1.4): a reviewer should require
  at least a notice at the top of each, or their rewrite, before a merge.
* **H3.** Nothing visual or audible is reported as passed anywhere.

### 4.11 The remediation of the advisory reviews (M)

The 69 items of the five advisory reports have verdicts in decision R56: 46 fixed, 1 settled by
decision, 14 deferred, 8 refuted or not verifiable. **M1:** verify each of the two P0 and
seventeen P1 fixes at the code and by its regression test (fail before, pass after: the
remediator's notes say so; reproduce it for the P0). **M2:** judge each "refuted" verdict.
**M3:** judge the deferred items as debt or as findings.

### 4.12 Decisions that need a reviewer's or the user's judgment (J)

| ID | Decision | Question |
|---|---|---|
| J1 | R6 | The amendment of I6 to I6'. The prompt's fixed decision 6 requires an explicit reviewer check of the decision record, the updated tests and the design. |
| J2 | R7 | The speed conversion replacing the prompt's anchor, and slower units at 1x as its consequence. |
| J3 | R4 | Map growth with the player count as an option, default off, reversing what the user asked for on 2026-10-10. |
| J4 | R29, T3 | The doubled tick limit. |
| J5 | R8, D4 | The soak in parallel, its allocator accounting, its accepted placement rejection, and its reduced reach into late game. |
| J6 | R53, P2 | The computer player's last stand. |
| J7 | R10 | Assumption RA-A3: a factory's side is the side of its building type. |
| J8 | R25 | Shipyards keep a reach of 32 cells because only 28 of 176 starts have shipyard water within RA2's 12. |
| J9 | R33, R35 | Equation 2's minimum of 1 kept; buildings hit once per footprint cell with the game's smaller footprints (footprints are a listed difference). |
| J10 | R55, R57, R63 | The process itself: advisory reviews during the run, then review and implementation stopped. |

### 4.13 For the user only (U)

Everything visual, audible and of feel since the baseline, none of it seen on a screen: the menu
(offscreen captures in `docs/screenshots/ra1/`), the slower pace (R7), the blank minimap before
radar, tab progress bars and badges, depot use, three soldiers to a cell, crushing, bursts and
damage smoke, rank chevrons and the promotion ping, crews, dock marks, interceptable V3s, the
computer player selling out, gems, growing ore, crates and their notices, and that Soviet
buildings and several new units reuse their twins' art. The status entries list them per
milestone under "Requires user verification".

## 5. How to do it

Either one reviewer with enough time, or the panel prepared in `docs/prompts/ra2-parity-review/`
(five slices and a lead, about three hours a round with the validation run in parallel). The
panel's slice texts were written for the whole programme: for this stopped state, slice `sim2`
shrinks to RA4 and RA5-A, slice `client` loses RA7 and RA8, and every slice should be given
section 1 of this document so that missing milestones are not reported as findings.

Suggested order: V1; M (the remediation, because it touched command handling everywhere and
tells how far the advisory reports can be relied on); RA5-A in depth (A5, D, I, N3, T4, R2:
it is the only implemented part nobody has examined and the only one not validated); then the
rest of section 4; then J.

Advisory reports are leads, not conclusions: they were made on snapshots without building the
game crate and without running the long matches, and they state what they could not check.

## 6. Work that remains to be implemented

Not review work, but a reviewer and the user should know it is owed if the programme is to be
finished. In order: finish RA5-A (the four closing steps of 1.3 and its full validation);
RA5-B; RA6; RA7; RA8; RA9, each as its plan entry says and gated by the validation commands;
then the final review of the whole. Briefs written for the implementers of RA5 to RA9 exist
only in the stopped session's scratch directory and are not in the repository; the plan entries
in `docs/RA2_PARITY_PLAN.md` section 5 are the binding description.

## 7. What must not happen before a review decides

* No merge of `ra2-parity` into `main` and no tag `ra2-parity-reviewed`: the prompt's section 7
  allows them only after GO or GO WITH DOCUMENTED NON-BLOCKING DEBT, and no approval tag is ever
  created by an agent. If the user wants the intermediate state on `main` regardless, that is
  the user's decision to take knowingly: `main` would then hold a game whose RA2 conversion is
  half done (1.4).
* No status entry or commit message may call anything "reviewed" or "approved".
