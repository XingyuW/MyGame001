# Prompt: bring MyGame001 to Red Alert 2 parity

You are the **orchestrator** of a single uninterrupted run in the repository
`/Users/xingyuwang/GameProjects/MyGame001`. Your goal is to turn the existing game into a single
player skirmish game whose rules, units, buildings, economy, combat, specialist abilities,
superweapons, vision, controls, player settings and computer opponents are comparable to the
original Command & Conquer: Red Alert 2 (not Yuri's Revenge), using
`docs/RA2_GAP_ANALYSIS.md` as the requirement source. Network play is a non goal. You carry out
every milestone below in one run, one after another, without waiting for the user and without any
review between milestones; the only gate between milestones is that the validation commands pass.
A single independent review of the whole programme happens at the very end (section 7), and you stop
early only on the conditions in section 10.

## 1. Read before doing anything

Read, in this order, and treat them as binding unless this prompt amends them:
`PLAN.md` (sections 5, 6, 8, 9, 10, 11 and 12 define equations, invariants, test categories,
validation commands, roles, severities, decisions and failure conditions), `docs/RA2_GAP_ANALYSIS.md`,
`docs/HANDOFF.md`, `docs/CONSTRUCTION_SYSTEM.md`, `docs/DECISIONS.md`, the latest part of
`docs/STATUS.md`, `README.md`, every file in `assets/data/`, and the module headers of every file in
`crates/sim/src/`. Then inspect `git status`, `git log --oneline -15` and `git tag`.

Treat every statement in the gap analysis as a **claim to reconcile** against current code and data,
not as fact. Distinguish confirmed facts from assumptions in everything you write.

## 2. Fixed decisions for this programme

1. **Default ruleset.** The RA2 ruleset becomes the default skirmish ruleset. Features this game has
   and RA2 lacks (Super Me, ALL UNLOCKED, Super Me unit speed, fog of war, the global rally key,
   maps that grow with the player count) are kept as menu options or extensions, with RA2 behaviour
   as the default wherever the two conflict. Record each such choice in `docs/DECISIONS.md`.
2. **Legacy armies.** The Vanguard and Meridian rosters and their original units (rifleman, rocket
   soldier, grenadier, scout, raider, light tank, heavy tank, artillery, rocket launcher, legacy
   harvester, gun turret, rocket tower) leave the default skirmish. Tests that rely on them are
   migrated to fixture data with equivalent assertions (invariant I10); no test is deleted, skipped
   or weakened. If removal turns out to break more than it fixes, keep them as a non default menu option instead and record why.
3. **Computer teams.** Keep the current default (computer players allied against the human) and add
   a free for all option, unless the user decides otherwise.
4. **Reference data provenance.** Obtain the vanilla `rules.ini` and `art.ini` from the source named in
   section 16 of the gap analysis into `~/Library/Caches/MyGame001/ra2-reference/`, record the
   repository, commit and file hashes in `docs/DECISIONS.md`, and **never commit those files or any
   other RA2 file** (art, audio, strings, maps). Numeric values ported into `assets/data/*.ron` carry a
   comment naming the source ini section and key. Behaviour not encoded in the ini is taken from
   ModEnc or the C&C Wiki, cited by URL; anything supported by one weak source is marked as an
   assumption in the plan and in the code comment.
5. **Assets.** All new visuals and sounds are original and produced by the repository's existing
   generators (`scripts/`, `crates/game/examples/`), recorded in `assets/LICENSES.md`. Where art
   quality cannot be judged automatically, report it as "requires user verification", never as passed.
6. **Invariants.** PLAN.md invariants I1 to I12 stay in force. The only planned amendment is I6 (one
   ground unit per cell), which may change to allow RA2 style infantry sub cell positions in RA3;
   that amendment needs a decision record, updated tests and an explicit reviewer check.
7. **Branching.** All work happens on one branch, `ra2-parity`, with at least one commit per
   milestone whose message starts with the milestone name (for example `RA3: ...`). Existing M tags
   and test file names are not renamed. This prompt overrides the per milestone review cycle of
   PLAN.md sections 10 and 12 for this programme only; the severities, decisions, invariants, test
   categories and failure conditions of PLAN.md still apply.

## 3. Units and conversions (state them in the plan, use them in code and tests)

All simulation arithmetic stays integer and deterministic (I2, I3, I12); multipliers are stored as
permille or as exact rationals, never floats.

| Quantity | RA2 unit | Game unit | Conversion |
|---|---|---|---|
| Time | frames at the nominal 15 frames per second given by the rules.ini comment on `IronCurtainDuration` | ticks, 20 per second at 1x | ticks = round(frames x 4 / 3) |
| Game minutes (`BuildSpeed`, `RechargeTime`, `GrowthRate`, `CrateRegen`) | minutes of 900 frames | ticks | ticks = minutes x 1200 |
| Range, `CellSpread`, `Adjacent` | cells (decimal allowed) | subcells, 256 per cell | subcells = round(cells x 256) |
| Speed | integer `Speed` | subcells per tick | anchored so the GI keeps its current speed: subcells per tick = round(RA2 Speed x 14 / 4); the plan must confirm the anchor or justify another |
| Build time | `BuildSpeed=.7` minutes per 1000 credits | ticks | base ticks = cost x 0.7 x 1200 / 1000 = cost x 21 / 25, then x 0.8 per additional factory of the producing type (`MultipleFactory`), then the low power factor |
| Damage | `Damage` x `Verses` percent | hit points | damage = floor(base x verses / 100), with veterancy and crate multipliers applied in permille in a documented order |

Formulas the ini does not fully define (low power production between `MinLowPowerProductionSpeed`
and `MaxLowPowerProductionSpeed`, splash falloff between `CellSpread` and `PercentAtMax`, elite
threshold, ore growth stages) must be researched on ModEnc, written into the plan as equations with
their source, and marked as assumptions where the source is weak.

## 4. Step RA0: baseline and plan

1. The working tree holds a large amount of uncommitted work (the unified arsenal, HD art, controls,
   multiple enemies, game speed, maps). Do not discard, reset, stash or rewrite it. Create branch
   `ra2-parity` from the current `main`, run the full validation (section 9), and commit the existing
   work in coherent commits titled `RA0 baseline: ...`. If validation fails before any change of
   yours, fix only what is needed to make the baseline green, record each fix in `docs/STATUS.md`,
   and continue; stop only if the failure cannot be fixed without changing intended behaviour.
2. Reconcile the gap analysis against the code. For every row of its sections 2 to 12, record
   "confirmed", "already different from the claim" or "not verifiable", with file and line evidence,
   in `docs/RA2_PARITY_PLAN.md`.
3. In the same file write the plan, and add a short section 14 to `PLAN.md` that points to it and
   states that it prevails for this programme. For each milestone RA1 to RA9 the plan defines: the
   problem and root cause; affected files, APIs, commands, events and data structures; production
   execution paths (which `SimSchedule` stages and which game systems); equations and units;
   invariants touched; data schema changes (increment `schema_version` and migrate every data file
   and fixture in the same change); compatibility and non goals; positive, negative, boundary,
   regression, integration and end to end tests; validation commands; acceptance criteria; and
   failure conditions. Do not wait for approval of the plan; start RA1 as soon as it is written.

## 5. Milestones (scope; the plan details them)

**RA1. Sides, countries and match setup.** A rules data file for global values (`[General]` and
related keys used by later milestones). Allied and Soviet sides and the nine countries, with
ownership rules equivalent to `Owner`, `RequiredHouses` and `ForbiddenHouses`, exclusive trees and
national specials (Black Eagle replaces Harrier for Korea, America's Airforce Command gives the
paradrop). Skirmish options: country per player including random, starting credits 5000 to 10000
(default 10000), unit count 0 to 10 (default 10) using RA2 starting units, start with an MCV,
short game on by default (defeat when no buildings and no `BaseUnit` remain) and long game, super
weapons on or off, crates on or off (functional from RA5), MCV repacks (Construction Yard undeploys),
difficulty per computer player named Easy, Medium and Brutal, team mode. Existing extensions stay as
options defaulting off. Costs, strength, sight and prerequisites of every item match rules.ini.

**RA2. Production, power and base building.** Cost proportional build time, the multiple factory
multiplier, separate main and armoury queues at the Construction Yard, `MaximumQueuedObjects`,
`BuildLimit` (Ore Purifier, Cloning Vats, superweapons), `WallBuildSpeedCoefficient`, graded low
power production, only buildings with negative power stop when power is low, minimap gated on a
powered radar building (Airforce Command, Radar Tower, Spy Satellite Uplink), per building
`Adjacent` placement distance, RA2 power values (Construction Yard 0), `RefundPercent`,
`RepairPercent` and `RepairStep`, unsellable walls, Service Depot docking repair for credits.

**RA3. Armour, warheads and weapons.** The eleven RA2 armour types and the `Verses` table of every
warhead used; `CellSpread` and `PercentAtMax` splash for ordinary weapons; area damage from
superweapons, bombs and suicide units routed through the armour table; primary, secondary and elite
weapons with RA2 selection rules; `BallisticScatter`; crushing (`Crusher`, `Crushable`); yellow and
red damage states (`ConditionYellow`, `ConditionRed`); infantry sub cell occupancy with the I6
amendment. Weapon damage, range and rate of fire match rules.ini through the section 3 conversions.

**RA4. Veterancy, crews and air and naval mechanics.** Experience from destroyed value
(`VeteranRatio`), veteran and elite ranks with the `VeteranCombat`, `VeteranSpeed`, `VeteranArmor`
and `VeteranROF` multipliers and the per unit `VeteranAbilities` and `EliteAbilities` lists, elite
self heal and rank insignia; `CrewEscape` and survivor rules; aircraft `Ammo` and four docks per
Airforce Command; V3 rockets and Dreadnought missiles as spawned air units that anti air can
intercept; Aircraft Carrier with three regenerating Hornets; submerged units cloaked with RA2
detection.

**RA5. Economy and world objects.** Ore and gems in bails (25 and 50 credits), ore growth and
spread (`GrowthRate`), miner storage 40 and 20 bails, the side's own free miner per refinery, Chrono
Miner self heal; crates with the `[Powerups]` shares, `CrateRegen`, `CrateRadius` and `FreeMCV`;
capturable Oil Derrick, Hospital, Airport and Tech Outpost; civilian buildings that GIs and
Conscripts garrison, with occupant fire and forced exit at red health; destructible bridges with
repair huts. Map generators and shipped maps gain gems, tech buildings, garrisonable buildings and
bridge huts; every map still sets up for every player count.

**RA6. Specialists and superweapons.** Every row of gap analysis section 9: GI deploy, Tanya C4,
spy disguise and all infiltration effects (veteran production, radar shroud reset, Battle Lab unlocks
of Chrono Commando, Psi Commando, Chrono Ivan and Yuri Prime), dog detection, chrono erasure, Tesla
Trooper coil charging, Prism Tower support chain, Mirage tree disguise, Crazy Ivan values, Yuri psi
wave, Terror Drone and Giant Squid parasites with Dolphin release, IFV crew weapons, Nuclear Reactor
death blast, Chronosphere (3 x 3 area, any owner, infantry die, ground units over water sink), Iron
Curtain (750 frames, vehicles and buildings, infantry die), Lightning Storm, Nuclear Missile with
radiation, paradrops, and superweapon warnings to all players.

**RA7. Vision, interface and controls.** Shroud only vision by default with fog of war as an option;
Gap Generator re-shrouding with its RA2 radius; Psychic Sensor range 15 and movement display; Spy
Satellite blocking radar infiltration; RA2 default hotkeys (Q W E R tabs, K repair, L sell, G guard,
X scatter, F follow, Z waypoint mode, Ctrl force fire, Alt force move, T, P, H, N and M, Ctrl+F1 to F4
bookmarks, control groups with Alt centring); context sensitive right click for Engineer, Spy and
transport loading; attack move kept on a non conflicting binding; original synthesised announcer
cues and text notices for unit ready, base under attack, low power, insufficient funds and
superweapon events. Keys the user already relies on are remapped only with a decision record and a
README update.

**RA8. Computer opponent.** Computer players play their country with the full side arsenal,
including air, naval (when the map has shipyard water), defences, engineers on tech buildings,
garrisons and superweapons; difficulty scaling equivalent to RA2's difficulty sections (firepower,
rate of fire, build time, team frequency, bonus money), documented with its source. The AI keeps its
current honest vision unless the plan records a reason to cheat. All AI matches in the test suite,
`m7_soak` and `m14_duels -- full` still end with a winner.

**RA9. Data parity audit, balance and documentation.** A test or checked tool compares every unit,
building, weapon and warhead in shipped data against a committed reference table of values (numbers
and ini keys only) and fails on any unexplained difference; intentional differences are listed with
reasons. Update `README.md`, `docs/CONSTRUCTION_SYSTEM.md`, and rewrite `docs/RA2_GAP_ANALYSIS.md`
to describe only the remaining differences. Then run the final review of section 7.

## 6. How each milestone is carried out

1. Before editing, inspect the code the milestone touches and verify every path, symbol, crate API
   and assumption against current evidence. Make the smallest coherent change for the milestone,
   with no unrelated refactoring. To keep your context small you may delegate a milestone's
   implementation to a fresh subagent with this prompt's sections 1 to 3, the milestone's plan entry
   and these rules; you remain responsible for its result.
2. Write tests in every category of PLAN.md section 8 that drive the production path
   (`CommandQueue`, `SimSchedule`, real data files for shipped behaviour and fixtures for balance
   independent behaviour). Never delete, skip or weaken a test; a test whose expectation changes
   because RA2 behaviour is intended is updated only with a note in `docs/DECISIONS.md`.
3. Run every validation command of section 9. If any fails, fix it within the milestone before
   moving on. Then commit, append a short milestone entry to `docs/STATUS.md` (what changed,
   validation results with exact test counts, assumptions, debt, items requiring user verification),
   record decisions in `docs/DECISIONS.md`, and continue immediately with the next milestone.
4. Do not review your own milestones and do not claim approval. Self checks are not reviews.

## 7. Final review at the end (the only review)

After RA9 is committed and validation passes:

1. **Independent reviewer subagent** with a fresh context, which did not write any of the code. It
   reviews the complete cumulative diff of `ra2-parity` against `main` as it was before RA0, reruns
   every validation command, and checks: runtime execution of the production path for every
   milestone's acceptance criteria; error context, silent fallbacks or data loss; API, command,
   event and schema changes and their migrations; determinism and hashing of every new state;
   test adequacy for each acceptance criterion; numerical and unit correctness of every ported
   value against rules.ini through section 3; regressions in existing behaviour; asset and reference
   data provenance; and the rewritten gap analysis. It writes `docs/reviews/RA2-parity-review-1.md`
   with findings rated P0, P1 or P2 (PLAN.md section 10) and a decision: GO, GO WITH DOCUMENTED
   NON-BLOCKING DEBT, or NO-GO. Any unresolved P0 or P1, failed validation, unverified acceptance
   criterion, unresolved numerical, scientific or data integrity risk, or broken compatibility is
   NO-GO.
2. On NO-GO, a **remediation subagent** with a fresh context treats each finding as a claim,
   reconciles it against current code, fixes it or refutes it with evidence, adds a regression test
   for every real defect and reruns validation. A new independent reviewer then re-reviews the full
   diff and writes the next review file. At most three remediation rounds.
3. On GO or GO WITH DEBT: merge `ra2-parity` into `main`, tag `ra2-parity-reviewed`, and record P2
   debt in `docs/STATUS.md`. You never approve the work yourself; only the user creates an approval
   tag.

## 8. Final message to the user

Send one message at the end, not during the run: the final review decision; what now matches RA2;
every remaining intentional difference; validation results; debt; and the checklist of visual,
audio and feel items that require user verification.

## 9. Validation commands (all must exit 0)

```
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --release
cargo build --release -p game
cargo run --release -p game -- --smoke-test 600
cargo run -p game -- --smoke-test 600
cargo run --release -p sim --example m7_benchmark
cargo run --release -p sim --example m7_soak
cargo run --release -p sim --example m14_duels -- full
sh scripts/package_macos.sh
```

Use the configured `CARGO_TARGET_DIR`. Keep logs under `docs/validation/ra2-parity/`. Do not create clones,
worktrees or large files in `/tmp`.

## 10. Stop and ask the user when

Stop only when one of these holds, because otherwise the run must continue to the end: a validation
command fails and cannot be fixed without abandoning a milestone's intended behaviour; an invariant
of PLAN.md section 6 would have to be broken (other than the planned I6 amendment); the reference
data cannot be obtained at all; a fixed decision of section 2 proves unworkable; the benchmark
budget or soak determinism cannot be restored; or the third remediation round ends NO-GO. Where an
RA2 behaviour cannot be established from any source, choose the closest documented behaviour,
record it as an assumption in `docs/DECISIONS.md`, and continue. When you stop, report the evidence
and the options.
