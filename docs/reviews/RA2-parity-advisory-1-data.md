# Advisory review at RA3, slice `data`: pipeline, ported values, schemas

Reviewed: the snapshot of commit `46e694d` (`~/Library/Caches/MyGame001/advisory-review/ra3/`).
Reviewer wrote none of the code. Advisory only: it gates and approves nothing.

Counts: **P0: 0. P1: 3. P2: 8. Suspected, not confirmed: 1.**

All paths are relative to the snapshot. My scripts and their output are in
`~/Library/Caches/MyGame001/advisory-review/scratch-data/` (`common.py`, `recompute.py`,
`rules_check.py`, `mutate.sh`, `regen/`). Every mutation of the snapshot was restored; the
SHA-256 of the seven data files, the reference tables, the three scripts and the audit equal
those of `git show 46e694d:<path>` at the end, and my probe test file was removed.

## P1 findings

### P1-1. Nothing in validation checks that the shipped files are what the generator writes, or that the reference table is what the ini holds

* **Where.** `scripts/validate_ra2_parity.sh:54` to `:64` (the eleven commands);
  `crates/sim/tests/ra9_data_parity.rs:73` to `:84` (`Reference::load` reads only
  `ra2_reference.tsv`); `scripts/ra2_reference.py:649` to `:652` (`--check` exits 0 when the
  cache is absent); `docs/RA2_PARITY_PLAN.md` section 6.1.
* **Evidence.** `grep -rn 'generate_ra2_data\|ra2_reference.py'` over every `*.rs`, `*.sh`,
  `*.toml` of the snapshot finds one hit, a doc comment in the audit. Neither
  `python3 scripts/generate_ra2_data.py --check` nor `python3 scripts/ra2_reference.py --check`
  is one of the eleven validation commands, and no test runs them. The audit trusts the TSV:
  it has no hash of it and never reads the ini. The mutation probes of P1-2 show that a hand
  edit of a generated file passes the audit whenever it touches a field the audit does not
  cover; an edit of `ra2_reference.tsv` together with the matching data value passes it by
  construction (read from the code, not run: I did not edit the shared reference table).
* **Expected.** The plan's section 3.1 pipeline (ini to table to data) holds at every commit:
  a generated file edited by hand, or a table that differs from the recorded ini, fails
  validation.
* **Minimal fix.** Add both `--check` calls to `scripts/validate_ra2_parity.sh` (and to plan
  section 6.1), or a test in `ra9_data_parity.rs` that runs the generator's `--check` and
  fails if `python3` is missing. Record the SHA-256 of `ra2_reference.tsv` beside the two ini
  hashes so that a table edited without the cache is caught too.

### P1-2. The audit does not cover several ported fields, and has no guard that every field of an item is audited

* **Where.** `crates/sim/tests/ra9_data_parity.rs:465` to `:915` (the item loop), `:672` to
  `:679` (power as a net number), `:1193` to `:1215` (effects: `None` for the damage of `c4`
  and `radiation`). Generator: `scripts/generate_ra2_data.py:921` to `:925` (`producer`,
  `free_units`), `:740` to `:743` (`factory`), `:294` to `:300` (`EFFECTS`).
* **Evidence.** With the audit binary built in the snapshot I changed one shipped value at a
  time, ran all five tests of `ra9_data_parity`, and restored the file (`mutate.sh`). Controls
  were caught (cost, speed, crusher, powered, a `Verses` entry, a reload, a rule scalar, a
  difficulty value, a starting list, the damage of the effect `nuke`). **These passed, not
  caught:**

  | File | Change | Ini key the value comes from |
  |---|---|---|
  | `economy.ron` | `refinery` `free_units` `["chrono_miner"]` to `[]` | `[GAREFN] FreeUnit=CMIN` |
  | `economy.ron` | `soviet_refinery` free unit `war_miner` to `chrono_miner` | `[NAREFN] FreeUnit=HARV` |
  | `economy.ron` | `gi` `producer` `barracks` to `soviet_barracks` | `Owner`, type list |
  | `economy.ron` | `kirov` `producer` to `soviet_naval_shipyard` | `[VehicleTypes]`, `Naval` |
  | `economy.ron` | `power_plant` 200 produced, 0 consumed to 250, 50 | `[GAPOWR] Power=200` |
  | `economy.ron` | `soviet_refinery` `credits_per_ore` 1 to 3; miner capacity and rate x 10 | game's own |
  | `arsenal.ron` | `naval_shipyard` `factory` `naval` to `vehicles` | `Factory=UnitType`, `Naval=yes` |
  | `arsenal.ron` | `radar_tower` `factory` `none` to `aircraft` | no `Factory` |
  | `arsenal.ron` | `power_plant` `side` `allied` to `soviet` | `ra2_mapping.tsv` side |
  | `arsenal.ron` | `harrier` `domain` `air` to `land`; `destroyer` `naval` to `land` | `MovementZone=Fly`, `Naval=yes` |
  | `arsenal.ron` | `flak_track` `capacity` 5 to 9 | `[HTK] Passengers=5` |
  | `combat.ron` | effect `c4` damage 200 to 9999; effect `radiation` damage 20 to 2000 | game's own (R35) |

  R9 item 2 names "the free unit of a refinery" and "factory kind" among the values RA1
  ports, and RA3 decides what a weapon may engage from the target's `domain`. For
  `rules.ron` the audit asserts that every scalar field has exactly one audit row
  (`:1854` to `:1883`); for units, buildings and arsenal entries there is no such guard, so a
  field added later is silently unaudited. The damages of `c4` and `radiation` are the game's
  own (documented in R35), yet they have no row in `ra2_differences.tsv`, although R36 says
  the audit lists each stand-in as a row and plan section 3.1 says every shipped value equals
  its conversion or is a listed difference.
* **Checked by hand instead.** My own recomputation from the ini finds all of these fields
  correct today (free units, producers, factory groups, sides, domains by `MovementZone` and
  `Naval`, `capacity` equal to `Passengers`). The finding is the missing test, not a wrong
  value.
* **Expected.** A changed ported value fails the audit; a game-owned value in a ported table
  is a listed difference or is named in an explicit list of fields the audit does not cover.
* **Minimal fix.** Audit `free_units` (`FreeUnit`), `factory` (`Factory`, `Naval`),
  `producer` (the factory of the item's side for its type list, `Naval`, `PadAircraft`),
  the arsenal `side`, the list of `refineries` (`Refinery=yes`), `domain`, and power as two
  numbers; add rows `effect.c4 damage` and `effect.radiation damage` (or audit them as
  "own value, must have a row"); add a list of every field of `BuildingDef`, production
  terms, `CombatTerms` and `ArsenalEntry` marked "audited" or "game's own", checked for
  completeness the way the rules scalars are.

### P1-3. `Soylent` of the two refineries is in the reference and in the plan's research, and is silently not applied: a sold refinery refunds 1000 credits where the reference gives 300

* **Where.** `crates/sim/tests/reference/ra2_reference.tsv:2054` (`GAREFN Soylent 300`),
  `:3465` (`NAREFN Soylent 300`); `scripts/generate_ra2_data.py:924`
  (`sell_value: cost x refund // 100`); `crates/sim/tests/ra9_data_parity.rs:680` to `:689`
  (the audit recomputes the same formula); `crates/sim/src/arsenal.rs:801` to `:814`
  (`Definitions::refund`: with a rules file `cost x refund_percent / 100`);
  `docs/RA2_PARITY_PLAN.md:1847` (appendix D topic 9: "`Soylent`: when non-zero it replaces
  the computed refund (47 vanilla sections set it)", single source ME/Soylent).
* **Evidence.** `grep -rni soylent` over the snapshot finds the plan's research row, the key
  list of `ra2_reference.py` and the TSV rows; no generator code, no audit code, no decision
  and no row of `ra2_differences.tsv`. A throwaway test (`probe_data_refund.rs`, removed)
  that loads the shipped files printed `refinery: cost 2000, refund with rules 1000` and the
  same for `soviet_refinery`. ModEnc's page `Soylent` (fetched today) states that a non-zero
  value "is used as the refund amount" for a building sold with the Sell button in Red
  Alert 2. Of the mapped buildings only `GAREFN`, `NAREFN` (300) and `GAOREP` (1250, equal to
  half its cost) set the key. Generator and audit share the omission, so the audit is green.
* **Consequence.** A refinery costs 2000 and brings a miner worth 1400; with a refund of 1000
  a player buys miners for 1000 net through the Construction Yard, which the reference value
  exists to prevent. Also: the audited field `sell_value` is not what the shipped game pays
  (with a rules file the refund is recomputed from `refund_percent`), so the audit's
  `sell_value` check does not test the shipped refund.
* **Expected.** The value is ported (sections 2 and 4.6 with the research of appendix D), or
  the difference is a row of `ra2_differences.tsv` with its reason and a decision.
* **Minimal fix.** Generator: `sell_value = Soylent if present and non-zero, else
  cost x RefundPercent`; `Definitions::refund` with a rules file returns the type's
  `sell_value`; the audit recomputes the same from `Soylent`; a shipped-data test sells a
  refinery for 300. Or list both rows as differences and record why.

## P2 findings

### P2-1. Two tests of "infantry" in the pipeline

`scripts/generate_ra2_data.py:596` and `crates/sim/tests/ra9_data_parity.rs:332` apply the
speed factor 3/2 by `Category=Soldier`; the arsenal's `infantry` flag (generator `:770`, audit
`:801`) and plan section 4.1 ("infantry on foot") use `[InfantryTypes]`; the audit of the
starting units (`:2197`) uses `Category` again. All 21 mapped infantry sections agree today
(my recomputation used the type list and matched every speed). A section where the two
differ would get an inconsistent speed with a green audit, since generator and audit share
the heuristic. Fix: one rule (the type list) in all four places.

### P2-2. Rule values in `rules.ron` that no code reads, beside precomputed copies

`production.build_ticks_per_1000`, `production.wall_build_time_pm`, `combat.min_damage`,
`combat.ballistic_scatter`, `combat.homing_scatter` and `combat.ivan_damage` are loaded,
range checked, hashed and audited, and have no reader outside `crates/sim/src/rules.rs`
(`grep -rn '\.<field>\b' crates/sim/src crates/game/src`): build times are precomputed into
`economy.ron`, scatter into each weapon, the minimum damage of 1 is a constant, the bomb's
damage is the effect `ivan_bomb`. Two sources of truth: a fixture rules file cannot change
build speed or minimum damage, and the two copies can drift without a test. These belong to
RA2 and RA3, which are done, so "for later milestones" (R9 item 3) does not explain them.
Fix: read them, or record them as documentation-only values in a decision.

### P2-3. The audit compares power as one net number

`ra9_data_parity.rs:672` to `:679` compares `power_produced - power_consumed` with `Power`.
The mutation 200/0 to 250/50 passes the audit and the loader. Fix: compare both numbers
(`max(Power, 0)`, `max(-Power, 0)`).

### P2-4. A reason in `ra2_differences.tsv` that the data contradicts

`crates/sim/tests/reference/ra2_differences.tsv:44` (`weapon.good_teeth damage 200 30`) says
the bite "kills every infantry type but the one with 200 hit points in two bites". With the
shipped `parasite_dog` warhead (100 percent against `none`, `flak`, `plate`) a bite of 200
kills every shipped infantry type in one bite, Yuri Prime (200 hit points, `flak`) included.
The difference itself is honest; the sentence is wrong. Fix the wording.

### P2-5. Statements of the pipeline's own documents that the files contradict

* `scripts/generate_ra2_data.py:12`: "It reads only the two committed tables"; it reads three
  (`ra2_combat.tsv` since RA3; the header's own list above it names three).
* `docs/RA2_PARITY_PLAN.md:80`: "`ra2_reference.tsv` holds only what the game uses"; it holds
  4888 rows of 425 sections, among them `[AI]`, `[IQ]`, `ThreatPosed`, `Points`, 82 rows of
  `art.ini` (`Foundation`, `Height`) that neither generator nor audit reads.
* `scripts/ra2_reference.py:18` to `:21` and `tests/reference/README.md:64`: "no image, voice,
  sound or animation names"; the `[Powerups]` rows carry the animation ids of the crate
  effects (`ARMOR`, `FIREPOWR`, `HEALALL`, `MONEY`, `REVEAL`, `SPEED`, `VETERAN`, `CLOAK`,
  `SHROUDX`, `CHEMISLE`; `ra2_reference.tsv:3774` onwards), repeated in the comments of
  `assets/data/rules.ron`. They are short identifiers, not text; the claim should name the
  exception as it does for a building's `Image`, or the script should drop the second entry.
* `crates/sim/tests/ra1_definitions.rs:2356`: comment "The arsenal is at 3 since RA2" above
  cases that test version 4.
* `scripts/generate_ra2_data.py:302`: `MIN_PROJECTILE_SPEED = 16` is defined and never used.

### P2-6. `ra2_reference.py --check` passes when the cache is absent

`scripts/ra2_reference.py:649` to `:652` exits 0 with a note. By design, but together with
P1-1 it means that on a machine without the cache nothing ties the table to the ini. Fix:
the recorded hash of the table (P1-1), or a distinct exit code for "skipped".

### P2-7. Game-owned values that will be differences are not yet visible anywhere near the data

Not defects of RA1 to RA3 (the plan defers them), listed so that RA9 does not miss them; none
has a row, and the audit covers none: 21 of 38 building footprints differ from `Foundation`
of `art.ini` (for example `construction_yard` 3x3 against 4x4, `war_factory` 3x3 against
5x3, `barracks` 2x2 against 3x2, `gap_generator` 2x2 against 1x1); the superweapon
`cooldown` of the arsenal (chronosphere 3600, iron curtain 3600, weather control 6000,
nuclear silo 6000 ticks) against the `RechargeTime` already in `rules.ron` (8400, 6000,
12000, 12000); harvester capacity and rate against `Storage`; the carrier's `capacity` 2
against `SpawnsNumber=3`.

### P2-8. Elite weapons of a type without elite keys are asserted, not audited

`ra9_data_parity.rs:581` to `:599`: for a type whose section has no `ElitePrimary`,
`EliteWeapon1` or `EliteSecondary` the audit asserts `weapons(true) == weapons(false)` and
compares nothing with the reference. Correct today; the fallback rule itself (a slot without
an elite weapon keeps its weapon, also when only `EliteSecondary` exists, as on `[MGTK]`)
is an engine assumption shared by generator and audit and has no label in R34.

## Suspected, not confirmed

### S-1. `JumpjetSpeed` read as leptons per frame

Plan section 4.1 and R7 label it an assumption; ModEnc's page gives no unit (fetched today:
only "the speed of units flying using the Jumpjet locomotor", default 14). The Kirov ships
at 4 subcells per tick (0.31 cells a second, half an Apocalypse; `[ZEP]` has `Speed=5` and
`JumpjetSpeed=5`), the Rocketeer and NightHawk at 23. If the key is on the scale of `Speed`
the Kirov would be 9 and the other two would exceed every aircraft, which is implausible,
so the assumption is the more likely reading. Only play against the original can settle it.

## Judgement of decision R7 (speed)

The justification holds. ModEnc confirms the formula `min(min(Speed,100) x 256 / 100, 255)`
and the hard-coded factor 3/2 for infantry that is not crawling (fetched today). Every other
rate of the port uses 15 frames to 20 ticks, so only the same factor keeps the ratio of
movement to reload, build time and recharge; the prompt's anchor `Speed x 14 / 4` would have
applied the infantry factor to vehicles as well (Grizzly 25 against 13). My recomputation of
all 49 shipped speeds from the ini agrees (infantry by `[InfantryTypes]`, jumpjets by
`JumpjetSpeed`, aircraft by `Speed`). Open points, all labelled in the plan:
`GameSpeedBias=1.6` is ignored (the ini comment calls it a multiplier of all movement;
ModEnc calls it obsolete "as a whole or in part"), and S-1. Whether 1x should feel like RA2
at 15 frames per second is for the user.

## What I checked and found correct

* **Independent recomputation** (`recompute.py`, `rules_check.py`; own ini reader and RON
  reader, no import of the programme's scripts): 3805 values of units, buildings, weapons
  and warheads straight from `rules.ini`, and 100 scalars, 8 powerups, 6 prerequisite groups,
  2 sides and 9 countries of `rules.ron`. Fields: cost, strength, sight, armour, speed, build
  time (walls x 3: 252), power, sell value by the plan's formula, prerequisites and groups,
  owner sides, required and forbidden countries, tech level, factory, lane, build limit,
  adjacent, base normal, radar, sellable, repairable, powered, is powered, unit repair,
  infantry, crusher, crushable, size, weapons and elite weapons, air range bonus, land and
  naval targeting, underwater, organic, wall; damage, range, minimum range, reload, burst,
  warhead, projectile speed, anti air, anti ground, scatter; `Verses` (all integer percent),
  cell spread, percent at the edge, wall, wood. **Every mismatch is a row of
  `ra2_differences.tsv`** except those of P1-2, P1-3 and P2-7, and every row of the table was
  met.
* **Differences table.** 58 rows, each with a specific reason; none hides an accidental
  mismatch (the stand-ins of R36, the single engineer and dog, the stolen technology units,
  the shipyard's reach, the 1.5 cell minimum range, ten later ids). One wrong sentence
  (P2-4).
* **Mapping.** Every section of the four type lists with `TechLevel` 1 to 10 and an owner
  among the nine houses is mapped; the twins differ only in `ForbiddenHouses`, names, voices
  and the dog's weapon and projectile image.
* **Audit.** Recomputes in Rust in integers, in code separate from the generator; a row
  matches only with its exact game and reference values; stale and unmet rows fail; unmapped
  shipped items, weapons and warheads fail. Its limits are P1-1, P1-2, P1-3, P2-1, P2-3, P2-8.
* **Difficulty lists.** The ini comment of `TeamDelays` says "(easy, medium, hard)"; ModEnc
  says hard, medium, easy, as the generator and audit index them. Correct.
* **Schemas.** `data.rs`: schema driven checker before `ron`, unknown, missing and duplicate
  fields rejected, `UnsupportedSchemaVersion` typed. Versions: units 1, economy 1, combat 2,
  arsenal 4, rules 1, skirmish 3, ai 2 in the generator, every shipped file and all 36
  fixture files; `rules.rs` has not changed since RA1, so its version 1 is right. Tests:
  earlier and later versions rejected for every file (`ra1_definitions.rs:2355`,
  `m4_definitions.rs:494`), every field required including the 9 of arsenal 3 and the 4 of
  arsenal 4 (`ra1_definitions.rs:1825`) and those of combat 2 (`m4_definitions.rs:1169`),
  limits at both ends and one step outside, cross references, truncations without panic.
  Range checks read in `rules.rs` (`credits_step` at least 1, so no division by zero) and
  `combat_defs.rs`.
* **Provenance.** The tables hold numbers, flags and identifiers only (no blank, colon or
  semicolon in any value; no `Name`, `UIName`, voice or sound key); searches of the snapshot
  and `git log -S` over `8230a95..46e694d` for ini comment fragments and string table keys
  find nothing; no ini file was ever added. All 2280 `[Section] Key=value` citations in the
  comments of `assets/data/*.ron` equal the ini. The ini hashes equal those recorded.
* **Reproduction.** `ra2_reference.py --check`: "matches the reference (4888 rows)";
  `generate_ra2_data.py --check`: exit 0; regeneration of the table and of all seven data
  files in a scratch copy (`scratch-data/regen/`) is byte for byte identical to the commit.

## What I could not check

* That the reference ini is that of patch 1.006 (assumption RA-A1; the file carries no
  version; spot values agree with what I know of 1.006).
* Engine facts that only the original game settles: the unit of `JumpjetSpeed` (S-1),
  `GameSpeedBias`, the arcing projectile speed of R34, the flak scatter factors, the elite
  weapon fallback, whether walls can be sold.
* The passing of an edited `ra2_reference.tsv` through the audit was read from the code, not
  run, to leave the shared reference table untouched.
* Whether other test files catch the mutations of P1-2 (I ran only `ra9_data_parity`; by
  search, `ra1_shipped.rs`, `ra2_shipped.rs` and `ra3_shipped.rs` do not mention free units
  or producers).
* Visual, audio and feel items (the pace at 1x) are for the user.
