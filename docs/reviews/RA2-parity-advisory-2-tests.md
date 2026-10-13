# Advisory review at RA4, slice `tests`: test integrity, acceptance criteria, data, game client

Scope: `46e694d..6a41bc3` (RA4). Read in the snapshot `~/Library/Caches/MyGame001/advisory-review/ra4/`
(commit `6a41bc3`); history with read only git commands against the main repository. Advisory:
it gates nothing and approves nothing.

**Counts: P0 0, P1 4, P2 8, suspected and not confirmed 1.**

## How I worked, and what I touched

* Ran the seven new sim test files in the snapshot with the probe command: `ra4_definitions` 8,
  `ra4_veterancy` 20, `ra4_crew` 10, `ra4_aircraft` 14, `ra4_spawns` 14, `ra4_cloak` 8,
  `ra4_shipped` 16, all pass. `ra4_ai` was run only under a mutation (below).
* **I changed no file of the snapshot.** Mutations and probes ran in a copy of the snapshot's
  `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `crates/`, `assets/data/` and `scripts/` at
  `~/Library/Caches/MyGame001/advisory-review/scratch-tests2/mut/`, built with the shared target
  directory (`-j 4`, only `-p sim --test ...`), so that no other reviewer could build a mutated
  source. Every mutated file was restored from the snapshot (`diff -rq` of `crates/sim/src`
  is empty at the end). The probe tests are `scratch-tests2/mut/crates/sim/tests/probe_tests_a.rs`
  and `probe_tests_b.rs`; the mutation scripts and their log are `scratch-tests2/mutate*.py` and
  `scratch-tests2/mutations.log`; the data recomputation is `scratch-tests2/recompute.py`.
* Test sets: `git grep` of every `#[test]` function of `crates/` at both commits (787 and 893).

## Mutations of the rules (20 run, in the scratch copy)

M1 to M4 ran one at a time; the others ran in three combined builds (M5, M7, M9, M11, M13;
M8, M10, M12, M14, M16, M17; M6, M15, M18, M21) and M22 alone, and are attributed by the
assertion each failing test makes.

| # | Mutation (file under `crates/sim/src`) | Detected by |
|---|---|---|
| M1 | veteran threshold exclusive (`veterancy.rs`, `rank_of`) | 4 tests, among them `the_thresholds_are_inclusive_and_one_credit_less_is_not_enough` and the shipped Grizzly test |
| M2 | elite threshold exclusive | 3 tests |
| M3 | armour division skipped (`combat.rs`, `impact_damage`) | `stronger_armour_divides_the_damage_taken`, `a_veteran_grizzly_hits_harder_and_is_hit_less`, `a_hit_of_one_point_...` |
| M4 | aircraft release a crew (`crew.rs`) | `only_the_base_units_have_a_crew_...` (through the NightHawk only, see finding 5) |
| M5 | a vehicle in a step releases its crew | `nobody_leaves_a_vehicle_that_was_moving_or_is_not_crewed` |
| M6 | cloak delay compared with `>=` in `special::reconnaissance` | **not detected: an equivalent mutant** (`flight.rs:242-244` drops an entry with `until <= tick` in the `Economy` stage of the same tick, so the comparison in `special.rs:1758-1760` can never be false for an entry that exists; the 24 ticks themselves are pinned by both cloak delay tests) |
| M7 | kills of allies and own units give experience | `splash_counts_for_the_unit_that_fired_and_only_enemies_count` |
| M8 | a launched aircraft's kill is not credited to its spawner | `the_aircraft_of_a_carrier_are_destroyed_with_it_and_their_kills_are_its_own` |
| M9 | dock check `taken > docks` (`economy_commands.rs`) | three aircraft tests and the shipped fifth Harrier |
| M10 | reload interval plus one tick (`flight.rs`) | both reload tests (fixture, shipped) |
| M11 | a missile ignores its launcher's firepower | `the_missile_of_a_veteran_and_of_an_elite_launcher`, the shipped V3 test |
| M12 | `SPAWN_LAUNCH_GAP` 26 | `a_dreadnought_launches_two_missiles_with_their_damage` only (finding 7) |
| M13 | survivors rounded down instead of up (`crew.rs`) | the two fixture tests; **not** the shipped test (finding 6) |
| M14 | a lost spawn is replaced after `spawn_reload` instead of `spawn_regen` | the shipped carrier test and three missile tests; **not** the fixture test `a_carrier_regains_an_aircraft_that_was_shot_down` (finding 7) |
| M15 | `surfaced` not hashed | `what_keeps_a_unit_in_sight_is_hashed` |
| M16 | sensors serve every enemy of the owner | `allies_see_a_cloaked_unit_and_every_enemy_needs_sensors_of_its_own` |
| M17 | experience is `min(victim cost, own cost)` | not detected; contrived, no finding (no test has a victim dearer than its killer) |
| M18 | the last stand never sells (`ai.rs:914`) | `ra4_ai::two_players_who_have_spent_each_other_...`: "None after 72000 ticks" |
| M21 | an untrainable unit gains experience from kills | `an_untrainable_unit_and_a_building_gain_nothing` |
| M22 | an aircraft keeps its dock whoever owns the building (`flight.rs:273`) | **not detected** (finding 2) |

(M19 and M20 were planned and not run.)

## P1

### 1. Two assertions of `ra3_shipped.rs` on naval and land targeting now pass because the Typhoon is cloaked, and R54 does not list it

* **File**: `crates/sim/tests/ra3_shipped.rs:379` (`("sea_scorpion", "typhoon", 4, None)`, comment
  "The Sea Scorpion's guns do not reach under water") and `:393`
  (`first_shot(12, "grizzly", "typhoon", 4) == None`, "a tank engages a destroyer and never a
  submarine"); helper `first_shot`, lines 126-180.
* **Evidence**: probe `probe_typhoon_hidden_in_ra3_naval_targeting_cases` on the shipped data,
  with the scenario of `first_shot`: for both attackers the silenced Typhoon is in
  `SpecialState::hidden` for P0 **on every one of 60 ticks** (neither a Sea Scorpion nor a
  Grizzly has sensors; `special::reconnaissance`, RA4). A hidden unit is acquired by no weapon
  (R51 item 1), so `first_shot` returns `None` whatever `underwater`, `naval_targeting` and
  `land_targeting` say. With a silenced Destroyer of P0 six cells from the Typhoon (it is then
  not hidden on any tick) both attackers still fire 0 shots: the RA3 rule itself still holds;
  the two assertions no longer show it.
* **Why it matters**: these were the only shipped assertions that a surface gun and a land
  unit never fire at a submerged unit. Since RA4 they cannot fail for that reason. R54 says of
  `ra3_shipped.rs` "The assertions ... are unchanged" and names only the Harrier and the Aegis;
  STATUS says "No test was deleted, ignored or weakened". What is left of the rule on shipped
  data is the Destroyer choosing its launcher (`ra3_shipped.rs:364`,
  `ra4_shipped::a_destroyer_sends_its_osprey_against_a_typhoon_it_detects`).
* **Expected**: the two cases run with the Typhoon detected, so that only the targeting rule
  can keep the attacker silent.
* **Minimal fix**: in `first_shot`, when the target's type is `cloakable`, put a silenced unit
  with sensors of the attacker's player in range (a Destroyer at sea; for the shore case water
  from column 12 leaves room at (20, 10)) and assert `!hidden` before returning `None`; add the
  row to R54.

### 2. No test loses a dock by capture; an aircraft that keeps the dock of a captured building passes every test

* **File**: `crates/sim/src/flight.rs:270-275` (the `retain` with `owner == a.owner`);
  `crates/sim/tests/ra4_aircraft.rs` (section "The loss of the dock", lines 309-396).
* **Evidence**: mutation M22 (`owner == a.owner` made always true): `ra4_aircraft` 14 of 14 and
  `ra4_shipped` 16 of 16 pass. The tests lose a dock by sale (`:312`), by destruction
  (`:338`, `:374`) and never by a change of owner; `the_dock_of_another_player_is_no_dock`
  (`:390`) covers taking a dock, not keeping one. No `ra4_*` test issues a capture.
* **Specified**: R49 item 4, "If its building is destroyed, sold or captured it takes a free
  dock elsewhere; with none ... is destroyed"; R54 claims "the loss of the dock in every form".
* **Minimal fix**: a test in `ra4_aircraft.rs` in which an engineer of P1 captures the airfield
  through the command queue (the fixture has an engineer): the striker takes a dock of a second
  airfield, and without one is lost; `PlayerState::docked_aircraft` and `docks` follow.

### 3. "Integration: an elite unit in a long battle" has no test: the long battle has one veteran and no elite unit

* **File**: `crates/sim/tests/ra4_veterancy.rs:601-634`
  (`a_long_battle_promotes_units_and_runs_the_same_twice`; the assertion at `:631` asks for any
  promotion to `VETERAN`).
* **Evidence**: probe `probe_ranks_in_the_long_battle` (the same battle, seed 3, 1,500 ticks):
  the ranks gained are `[1]`: one tank becomes a veteran, none elite. So no test has a unit
  that reached elite rank by its kills and then fights on with its elite weapon, its four
  multipliers together and its self healing over many ticks; every elite unit of the suite is
  made by `sim::promote` and observed for at most 40 ticks, or (the shipped Grizzly) is only
  inspected after its last kill.
* **Specified**: plan entry RA4, "Tests": "Integration: an elite unit in a long battle".
* **Minimal fix**: give P1 more cheap victims (or the tanks of P0 ample hit points) so that a
  tank reaches 2,400 credits, assert `gained` contains `ELITE`, and assert that the elite tank
  fires the autocannon and regains hit points later in the same run; or state in R54 that the
  item is covered only piecewise.

### 4. A missile type whose effect the combat file does not list loads, and its missile silently deals nothing

* **File**: `crates/sim/src/flight.rs:425-428` and `:448` (`if let Some(effect) = effect`);
  `crates/sim/src/arsenal.rs` (no check of `missile` against the combat file's `effects`);
  `crates/sim/tests/ra4_definitions.rs:571-575` and `:230`.
* **Evidence** (read, and the suite itself): `ra4_definitions.rs:230` asserts that the fixture's
  combat file has no `dmisl` effect, and `:571-575` asserts that the arsenal with
  `missile: "dmisl"` on the rocket **is accepted**. At the end of such a missile's flight
  `defs.effect(missile.effect(flight.elite))` is `None`: `MissileDetonated` is written, the unit
  is removed, and `detonate` is skipped. The same happens for an elite launcher when only the
  plain effect is listed (`v3_rocket` without `v3_rocket_elite`). No test asserts either
  outcome, and no typed error names the missing effect. The shipped data is complete (the audit
  asserts all nine effects), so nothing is wrong in the shipped game today.
* **Why P1**: a silent fallback behind a data combination the loader accepts (R46 item 2 lists
  what is rejected; this is not among it), in the family "new fields required and range
  checked" of this slice.
* **Minimal fix**: `with_arsenal` rejects an entry with `missile` whose two effects are not in
  the combat file (`UnknownReference { kind: "effect", .. }` at `entries[n].missile`), with a
  case in `the_terms_of_a_spawner_must_fit_together`; or the fallback (elite to plain, plain to
  nothing) is decided, written in R46 and tested.

## P2

### 5. The Harrier row of the shipped crew test cannot fail

`crates/sim/tests/ra4_shipped.rs:534` runs `wrecks(P0, "harrier")` in a world without an
Airforce Command. Probe `probe_harrier_without_dock_after_first_tick`: the Harrier is destroyed
in its first tick (R49 item 4), whose events `wrecks` discards (`:513`); `set_hp(.., 0)` then
finds nothing, and the second tick has 0 events. The comment "the Harrier's pilot does not get
out ([ORCA] Crewed=yes)" is therefore shown only for the NightHawk (mutation M4 fails the test
through it); the Black Eagle, also `crewed: 1`, is in no test. Fix: collect the events of both
ticks, or give the aircraft an Airforce Command as `ra3_shipped::first_shot` does.

### 6. The shipped survivors test has only exact quotients

`ra4_shipped.rs:542-552`: 1500/500, 1500/250, 250/500 (the minimum of 1), 250/250, 1000/500,
1000/250. Mutation M13 (round down) passes it; only the fixture tests see the rounding. A
Soviet Tesla Reactor (300 / 250: 2 against 1) or an Allied Ore Purifier (1250 / 500: 3 against
2) would pin `ceil` on shipped data, which is the assumption R48 flags.

### 7. Loose bounds on the spawn timings

* `ra4_shipped.rs:859-874`: the 800 ticks of `[CARRIER] SpawnRegenRate` are bounded only to
  (403, 813] and the 200 of `SpawnReloadRate` to at most 403. R54 calls it "the 800 ticks to
  replace one".
* `ra4_spawns.rs:326-355` (`a_carrier_regains_an_aircraft_that_was_shot_down`) waits 500 ticks
  and has no lower bound: mutation M14 passes it.
* `ra4_spawns.rs:306-307` compares the launch gap with `SPAWN_LAUNCH_GAP` itself, so only the
  shipped Dreadnought test (`ra4_shipped.rs:813`) pins 27.
Fix: assert the slot is still `Lost` one tick before the time and `Ready` at it, as the reload
tests of `ra4_aircraft.rs` do for ammunition.

### 8. "The last free spot for a crew member" is tested with crew that hold whole cells

The fixture's crew types are not infantry (`fixtures/data/ra4/arsenal.ron`: `rifleman`,
`conscript`, `engineer` have `infantry: 0`), so `survivors_are_as_many_as_the_footprint_has_room_for`
(`ra4_crew.rs:295`) and the comment "one to a cell" (`:228`) exercise cells, not the three
spots of a cell that R48 item 1 and the shipped data use. The shipped test checks only that the
first three soldiers share the first cell (`ra4_shipped.rs:586-590`). No test fills a footprint
to its last spot with soldiers on foot. Fix: one shipped case with all but one cell of the
footprint closed and more than three survivors (a Soviet Construction Yard: 6, of which 3 find
room).

### 9. Destruction is set, not fought, in most crew and dock tests

`set_hp(id, 0)` stands for the kill in `ra4_crew.rs::destroy`, `ra4_shipped.rs:514`, `:598`,
`:712` and the dock tests, where an `Attack` command exists. One integration test fights a base
unit and a yard down (`ra4_crew.rs:350`); no test destroys an Airforce Command or airfield by
fire with aircraft on it. Acceptable as setup; noted because the brief asks for the list.

### 10. A rejection asserted without its reason

`ra4_shipped.rs:667`: "Without any Airforce Command nothing builds a Harrier" asserts only
`rejections(..).len() == 1`. The fixture counterpart names `NoProducer`
(`ra4_aircraft.rs:179`). Fix: assert the reason.

### 11. Range checks tested from above only for two fields

`ra4_definitions.rs:264-295`: `spawns_number` and `spawn_regen` are tested only above their
maximum, not with `-1` (the other four numbers have both). One row each.

### 12. A comparison that can never decide (found by mutation M6)

`special.rs:1758-1760` tests `until > tick` on entries that `flight.rs:242-244` has already
pruned with the same condition earlier in the tick. Harmless; one of the two is dead, and a
reader of `reconnaissance` takes it for the place where the delay ends.

## Suspected, not confirmed

* **A captured building with docks and the queue check.** `economy_commands.rs:305-316` counts
  `state.docked_aircraft()` "as of the last `Economy` stage". In the tick after an Airforce
  Command changes owner, the new owner's docks and the old owner's count may be one tick stale
  in either direction. I did not build the scenario (it needs the capture test of finding 2).

## Judgement of R53 and R54

* **R53 (last stand, match lengths)**. The reasoning is sound and the regression test can
  fail: with the rule disabled (M18) `two_players_who_have_spent_each_other_...` ends with "None
  after 72000 ticks", as R53 item 2 says. The unit test
  `ai::tests::a_player_that_can_neither_fight_nor_earn_makes_its_last_stand` (read, not run)
  feeds a changing queue fraction through the patience and pins the turn boundaries 8,191 /
  8,192 / 8,447 / 8,448. **No limit was touched**: `git diff 46e694d..6a41bc3` changes no file
  under `crates/game/tests`, no `m6_*`, `ra2_ai`, `ra3_ai` test, not `assets/data/ai.ron`, not
  `m7_soak`, `m7_benchmark` or `scripts/validate_ra2_parity.sh`; `m14_duels.rs` only prints more
  in its trace. The table of R53 item 4 **reproduces exactly** from the two logs (my script over
  `ra3/10-duels.log` and `ra4/10-duels.log`, limit `72,000 x max(128, w + h) / 128`): median
  0.164 and 0.168, 90th percentile 0.333 and 0.366, longest 0.961 and 0.954, matches above
  0.5 / 0.7 / 0.8: 25 / 3 / 1 and 33 / 9 / 4, totals 30,297,292 and 32,436,146 ticks, 0 without
  a winner. The honest reading is the one R53 gives: the tail is heavier and one duel is 4.6
  percent under its limit with this seed; that is debt, recorded.
* **R54 (tests adapted)**. The schema rows are accurate (every changed assertion and constant
  read in the diff: one case more in each version test, sixteen fields added to the required
  field test, no assertion removed; the removed lines of the game tests are only the `Sfx::ALL`
  length and code moves). The counts are right: 787 and 893 `#[test]` functions, one renamed
  (`schema_versions_other_than_2_are_rejected` to `..._3_...`), none removed, no `#[ignore]` at
  either commit; 100 new in `sim` (92 in `ra4_*`, 8 unit tests) and 6 in `game`. **Not
  accurate**: the row on `ra3_shipped.rs` (finding 1) and "the loss of the dock in every form"
  (finding 2).

## Data (item 3 of the slice)

* **Recomputed independently** (`recompute.py`, my own conversion code over
  `~/Library/Caches/MyGame001/ra2-reference/rules.ini` and `ra2_mapping.tsv`): all sixteen
  schema 5 fields of the 91 shipped arsenal entries, **1,032 values, 0 mismatches**
  (`Trainable` default yes for units, both ability lists token by token, `Crewed`,
  `SelfHealing`, `Ammo` with -1 as 0, `AirportBound`, `NumberOfDocks` only where an airport
  bound aircraft names the building, `Spawns`, `SpawnsNumber`, the two rates as
  `round(frames x 4 / 3)` (400 to 533, 600 to 800, 150 to 200, 80 to 107), the launcher slot
  from `Spawner=yes`, the missile from `[General] V3RocketType` / `DMislType`, `Cloakable`,
  `SensorsSight` under `Sensors=yes`).
* **By hand against the ini**: `[HORNET]` 75 / light / Speed 12 to 23 / Sight 2 / Cost 50;
  `[ASW]` 30 / light / 23 / 2 / 50; `[V3ROCKET]` 50 / special_2 / 15 to 29 / 1; `[DMISL]` 50 /
  special_2 / 20 to 38 / 0; `[HornetBomb]` 40, range 5, ROF 3 to 4, ORCAAP; `[HornetBombE]` 80,
  ARTYHE; `[ASWBomb]` 50, range 3, APSplash; `[V3Launcher]` 1, 18 and 5 cells, ROF 150 to 200;
  `[DredLauncher]` 50, 25 and 8 cells, ROF 50 to 67, burst 2; `[HornetLauncher]` 1, 25 cells,
  200; the four effects 200 / 400 / 300 / 600 with `V3WH`, `V3EWH`, `DMISLWH`, `DMISLEWH` and
  the spreads and `Verses` of the two new warheads; `rules.ron`: `VeteranRatio` 3000,
  `VeteranCombat` 1100, `VeteranSpeed` 1200, `VeteranSight` 0, `VeteranArmor` 1500,
  `VeteranROF` 600, `VeteranCap` 2, `InitialVeteran` 0, `CrewEscape` 50, `SurvivorRate` 400,
  divisors 500 and 250, crews E1 and E2, `ReloadRate` .3 to 360, `RepairRate` .016 to 19,
  `CloakDelay` .02 to 24. All equal.
* **Migration**: all thirteen `arsenal.ron` and `combat.ron` files of the tree are at schema 5
  and 3 (shipped, `fixtures/data`, `legacy`, `ra1` to `ra4`); the in-test generator
  `m4_definitions.rs::generated` writes schema 3. Old versions are rejected with
  `UnsupportedSchemaVersion` (arsenal 4, 3, 2, 1, 6; combat 0, 1, 2, 4, -1 and the extremes);
  each of the sixteen fields is required (`MissingField`).
* **Audit and differences**: `ra9_data_parity.rs` compares the sixteen fields for every mapped
  item and the four effects; `LATER` lost `"spawned"`, so the four spawned types are audited
  like every type. Eleven rows of `ra2_differences.tsv` are gone: the seven stand-ins, which are
  really gone from `combat.ron` (launcher damage 1 / 50, reload 200 / 67, projectile speed 0),
  and the four "not in the data yet" rows. Six launcher rows remain, reworded, and three new
  rows list the collision weapons that are not carried. Honest.

## Game crate (read only)

* Rank insignia (`health.rs`): drawn for every unit whose view is shown, from
  `sim::Experience` of the unit; never for a hidden view. Dock marks only on a shown building.
* Hidden submarines: `units.rs:645-652` sets the view `Hidden` for what `SpecialState::hidden`
  names for the human player; picking (`selection.rs:602-613`, `hover_info.rs:159`), health
  bars, chevrons and the minimap (`minimap.rs:316`) all go through that visibility or the same
  set; the promotion ring checks it (`weapon_fx.rs:1097-1101`). Launched units are filtered
  from selection in the three places that list own units.
* Notices: only the human player's promotions and crews (test through Bevy in `notices.rs`).
  A launched Hornet inherits its rank without `RankGained` (`veterancy.rs::inherit`), so there
  is no notice per sortie.
* Assets: the two new files `sprites/hd/units/v3_rocket.png` and `dred_missile.png` have models
  in `scripts/hd_aircraft.py` (`MODELS`, `SCALE`) and are in both the prose and the table of
  `assets/LICENSES.md`. No other asset file was added.

## Honest reporting (STATUS entry RA4 against the logs)

Reproduced from `docs/validation/ra2-parity/ra4/`: 893 passed, 0 failed, 0 ignored (sum of the
`test result` lines of `04-test.log`); the benchmark means and longest ticks; the soak's
18,720,000 ticks, 2,769 s and 214.5 MiB; the smoke lines (716 and 5,009 ticks, 4 and 36
commands, 27 sounds, 0 problems); 756 duels, the slowest in tick 274,692, and every number of
the "Match lengths" paragraph. The one statement I found wrong is "No test was deleted, ignored
or weakened" (finding 1).

## Could not check

* The game crate's tests and the smoke tests (not allowed to build `game`); in particular
  `crates/game/tests/m10_unlocks_and_enemies.rs`, which names the Typhoon, and whether a game
  test now meets a hidden submarine.
* The unit tests inside `crates/sim/src` (the probe command runs integration test files only):
  read, not run.
* `m6_ai`, the soak, the duels and `m6_production_matches` (excluded by the brief); the margin
  of that test's own 756 matches under RA4 is in no log.
* That the validation ran on `c6dda01` with a clean tree.
* That the two PNG files are what the generators render (needs Blender).
* Everything visual, audible and of feel (R52 item 8).
