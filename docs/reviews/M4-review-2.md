# M4 review 2: Combat and fog of war

This is the independent review, round 2, of milestone M4 under PLAN.md sections 10 and 12. The reviewer did not write or remediate any of this code and changed no repository file. All scratch copies, probes, and build output for probes are outside the repository, under `…/scratchpad/m4-review-2-probe/`.

## Scope and base

* **Repository:** `/Users/xingyuwang/GameProjects/MyGame001`, branch `milestone/m4`, HEAD `dad9bb0`. The working tree was clean before and after the review.
* **Base:** tag `m3-reviewed`. It is the most recent `mN-reviewed` or `mN-approved` tag (PLAN.md section 12 item 1).
* **Cumulative diff reviewed:** `git diff m3-reviewed..milestone/m4`, 34 files, +7,791 / −106. Commits:
  * `9608923`: sim (combat, projectiles, fog of war);
  * `e19677d`: game;
  * `7506cbd`: implementation report;
  * `0cb2199`: review 1;
  * `652d180`: remediation code;
  * `dad9bb0`: remediation report, D32 and D33.
* **Remediation diff:** `git diff 0cb2199..milestone/m4`, 9 files, +736 / −66. Read line by line.
* **Read in full:**
  * PLAN.md, including section 12;
  * docs/STATUS.md, including the M4 implementation report and the M4 remediation round 1 report;
  * docs/DECISIONS.md, including D28 to D33 and the D6 amendments;
  * docs/reviews/M4-review-1.md.
* **Code read in full at HEAD:**
  * `crates/sim/src/combat.rs`;
  * `crates/sim/src/visibility.rs`;
  * `crates/game/src/fog.rs`;
  * the validation paths in `crates/sim/src/command.rs`: `validate`, `player_fog`, `validate_player`, `validate_combat`;
  * the `PlaceBuilding` path and `fog` in `crates/sim/src/economy_commands.rs`;
  * `check_visible` and `check_footprint` in `crates/sim/src/building.rs`;
  * the M4 diffs of `hash.rs`, `schedule.rs`, `unit.rs`, `economy.rs`, and `combat_defs.rs` (validation indexing);
  * every test changed or added by the remediation in `crates/sim/tests/m4_combat.rs`.
* **Third-party code that consumes external data:**
  * `combat.ron` goes through `data::read_bounded`, then `data::check`, the subset checker driven by `COMBAT_FILE_SHAPE` (`combat_defs.rs:363-379`). Only then does `ron` 0.12.2 parse it (`data::parse`, `data.rs:617-629`), with `with_recursion_limit`. `ron`'s guard is at `ron-0.12.2/src/de/mod.rs:136-154`; it returns `Error::ExceededRecursionLimit`.
  * `gun_turret.png` passes the existing PNG subset check before the Bevy image loader.
  * The remediation changed none of these paths.

## Validation commands (PLAN.md section 9 and section 12 item 5)

The reviewer ran these from the repository root, in this order, in the foreground (macOS, Apple M5, rustc 1.98.1, cargo 1.98.1). A display was available.

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 307 passed, 0 failed, 0 ignored; peak memory footprint 0.13 GB; only the known M0 linker warning |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | see below |
| `cargo run -p game -- --smoke-test 600` | 0 | see below; only the M0 debt item 1 linker warning |

**Test counts:**

* `game`:
  * unit tests: 68;
  * integration tests: `m0_spike` 6, `m1_demo_map` 3, `m1_tileset_image` 6, `m2_unit_sprites` 5, `m3_building_sprites` 3.
* `sim`:
  * unit tests: 11;
  * integration tests: `m0_bootstrap` 12, `m1_map` 32, `m1_map_subset` 24, `m2_definitions` 14, `m2_movement` 37 (39.3 s), `m3_definitions` 11, `m3_economy` 21, `m3_harvest` 10, `m4_combat` 32, `m4_definitions` 12.
* doc-tests: 0.

These match the remediation report.

**Release smoke run:**

```
smoke test: 600 frames, 198 simulation ticks, map created: true, units spawned: 26, units moved: 18 in 198 ticks after the order, buildings spawned: 3, production charged 396 credits in 198 ticks, visible cells: 389, enemy views shown: 0, view mismatches: 0
```

**Debug smoke run:**

```
smoke test: 600 frames, 203 simulation ticks, map created: true, units spawned: 26, units moved: 18 in 203 ticks after the order, buildings spawned: 3, production charged 406 credits in 203 ticks, visible cells: 399, enemy views shown: 0, view mismatches: 0
```

Both smoke runs logged the `Skipped event Destroyed` warning (M0 debt item 2).

**Additional runs:**

* `cargo test --release -p sim` passed: 216 tests (11 unit tests and 205 integration tests).
* `m4_combat::twenty_against_twenty…` with `--nocapture` gives the same result in debug and release: `ended at tick 665, survivors {0: 6, 1: 0}, 451 shots, 34 units destroyed, final hash 05af3b1ac550cf16`. This is identical to review 1 and to the remediation report.

## Status of the findings of review 1

| Finding | Status | Evidence |
|---------|--------|----------|
| **P1-1:** acquisition engages enemies the warhead cannot damage; attack moves never end; units do not return fire | **Resolved** | See P1-1 below. |
| **P1-2:** `PlaceBuilding` rejections reveal unseen enemies | **Resolved** | See P1-2 below. |
| **P2-1:** instant-shot lines drawn from unseen attackers | **Resolved** for instant shots | `fog.rs:152-176` (`ShotView`, `shot_view`): line only when both ends are seen, else a mark at the seen end. Used at `fog.rs:217-232`. Unit test `an_instant_shot_shows_only_the_ends_the_human_player_sees`. A residual for projectiles, and the overstated claim built on this fix, are new P2-1 below. |
| **P2-2:** `RejectReason::NotTargetable` unreachable | **Resolved** | See P2-2 below. |
| **P2-3:** D6 M3 sentence moved into the M4 amendment | **Resolved** | `git diff m3-reviewed..milestone/m4 -- docs/DECISIONS.md`: the M3 amendment of D6 is unchanged from `m3-reviewed`. The M4 amendment is a separate paragraph without the M3 sentence. |

### P1-1

**Code.**

* `Field::damages` (`combat.rs:407-413`) is `applied_damage(damage, versus) > 0`.
* `nearest_enemy_in_range` (`:417-440`) and `keep_or_acquire` (`:625-640`) consider only visible enemies in range that the weapon damages.
* These rules cover idle units (`:777`), attack-moving units (`:746`), and defensive structures (`:598`).

**Regression tests.** The four new tests named in the remediation report fail on `0cb2199` and pass at HEAD. The reviewer reproduced this on a `git archive` of `0cb2199` with the HEAD `m4_combat.rs` and only `AttackEnd::NoDamage` and `PlacementError::NotVisible` added:

* 5 failed: the four P1-1 tests and the P1-2 test;
* 27 passed, including the guard test `a_placement_over_a_visible_enemy_still_names_it`.

**Adversarial probe `zero_chaos`.** The probe uses a variant of the fixture combat file in which the bullet deals 0 % to plate and concrete, armour piercing deals 0 % to plate, and high explosive deals 0 % to concrete.

* **Setup:** 30 seeds, each with 24 random units and 6 random buildings (fences, turrets, beacons, flags). The probe issues random `Attack`, `AttackMove`, `Move`, and `Stop` commands from both players for 3,000 ticks, then runs 3,000 quiet ticks.
* **Every tick:** I6 holds, no hit points ≤ 0 survive, and credits are ≥ 0.
* **Zero-damage shots:** all 3,468 were ordered shots. Each was accompanied in the same tick by `AttackEnded { NoDamage }` for that unit and target.
* **Other order ends:** 88 `TargetGone`, 249 `TargetHidden`, 42 `OutOfReach`.
* **After the quiet ticks:** every unit has `weapon.order == None`, no move order, and no step. Every order ended.
* **Determinism:** the combined per-tick hash digest is `860015e07bd08abf` in both debug and release (I3, I12).

**Further probes, release.**

* **`attack_move_under_fire_from_undamageable`.** A rifleman that cannot damage tanks attack-moves past an enemy tank that shoots it. It fires 0 shots and completes its move at tick 183.
* **`attack_nodamage_projectile_and_mixed`.** A tank, whose shell is set to 0 % against concrete, and a rifleman together `Attack` a fence. Each fires exactly once, the tank's shot being a projectile, and each ends with `NoDamage`. Neither keeps a target, and nothing panics.

### P1-2

**Code.** `check_visible` (`building.rs:209-226`, function at `:214`) is called under fog (`economy_commands.rs:242-244`) after `NotReady` and before `check_footprint` and `check_distance`. A footprint outside the map is left to `OutsideMap`.

**Probe `placement_indistinguishable`.** The probe runs 12 random layouts of 14 enemy crawlers, harvesters, fences, and beacons around a human yard at (10, 10). It places a power plant at each of the 169 origins x 5..17, y 5..17. Each outcome is computed in two worlds:

1. with all enemies;
2. with only the enemies in visible cells, spawned first so that their ids match.

All 2,028 outcomes are identical between the two worlds: 1,212 `NotVisible`, 66 `Occupied`, 288 `Overlap`, and 462 placed. A placement therefore reveals nothing about cells the player does not see.

**Audit of the other command variants.** Validation reads only own entities, visible foreign entities (`UnknownUnit`, `UnknownBuilding`, or `UnknownTarget` otherwise), and map bounds (`command.rs:443-500`, `:531-571`). D33 item 1's audit holds.

### P2-2

* `NotTargetable` is gone from `crates/` (grep).
* `validate_combat` (`command.rs:531-571`) now decides between `OwnTarget` and `UnknownTarget` only.
* A target without `Health` is not in the `Combat` index (`combat.rs:502-523`), so an order on it ends with `TargetGone`.

## Judgement on the two remediation claims named by the orchestrator

### Claim 1: three existing tests changed their stimulus, not their assertions

The claim holds. No test was weakened under PLAN.md section 8.

**Against `m3-reviewed`.** The three tests are new in M4, so no M1 to M3 test changed. `git diff m3-reviewed..milestone/m4 -- crates` removes no assertion and no `#[test]`, and adds no `#[ignore]`. The diff of `crates/sim/tests` and `crates/game/tests` against `m3-reviewed` contains additions only.

**Against `0cb2199`.** `git diff 0cb2199..milestone/m4 -- crates/sim/tests crates/game` removes only two comment lines in tests, plus the old `draw_combat` body. Test counts go from 26 to 32 (`m4_combat`) and from 4 to 5 (`fog`).

* **`the_damage_equation_holds_for_every_warhead_and_armour_pair_of_the_fixtures`:**
  * The only change is an `Attack` command before tick 1, for the single pair with a percentage of 0 (rifle against concrete).
  * Every assertion is unchanged: the shot comes in tick 1, damage equals the independent equation 2 after the flight time, and the zero, minimum-one, floor, and exact cases are all present.
  * The zero case still runs through `SimPlugin` and `CommandQueue`.
* **`a_building_is_in_range_when_its_nearest_edge_is`:**
  * It runs on a variant in which the bullet deals 1 % against concrete. The range assertions (640 fires, 896 does not, target set accordingly) are unchanged.
  * The variant is needed for the test to keep its strength. With 0 % under the new rule, the "does not fire at 896" half would pass vacuously, because the rifleman would never engage concrete.
  * The edit is asserted to apply (`combat_variant` asserts the text changed). The fixture file is unchanged.
* **`the_shortest_range_reaches_every_neighbouring_cell_and_an_adjacent_building`:**
  * `extreme_scenario` adds the same 1 % edit, with `assert!(text.contains(…))`.
  * The other users of `extreme_scenario` involve no concrete target (`:519`, `:544`, `:572`), so the edit does not affect them.

**Old tests against the new code.** The reviewer ran the `0cb2199` version of `m4_combat.rs` against a `git archive` of HEAD. Exactly these three tests fail, and each fails at its "did not fire" point (old file lines 331, 417, and 484). These are the stimulus dependence on the removed behaviour; no other assertion fails.

### Claim 2: an explicit `Attack` on a target the unit cannot damage ends after one shot

This is accepted as a design decision. It is not a finding.

* **Departure from review 1.** Review 1's "expected behaviour" said the explicit `Attack` keeps its documented semantics. That was a suggestion. The finding's substance was that orders must end and that acquisition must not prefer harmless targets.
* **Effect of the chosen rule** (`combat.rs:695-705`): every `Attack` order ends, closing the defect class, and the event is typed and not silent (`AttackEnded { NoDamage }`).
* **Mixed selections** are handled per unit.
* **Equation 2's zero case** stays reachable through the production path, which the required M4 test uses.
* **Documentation:** D29 item 6, D32 item 3 (with the alternatives rejected), and STATUS remediation debt item 4.
* **Probes** (above): the rule holds for projectile weapons and in random play. An idle unit never re-acquires the target.
* PLAN.md does not specify target-choice rules, so no plan text is contradicted.

## Verification summary

### Acceptance criteria and required tests (PLAN.md section 7, M4)

| Item | Verdict | Evidence |
|------|---------|----------|
| Damage equation for every armour and warhead pair of the fixtures, minimum 1, zero case | Verified | `m4_combat.rs:252-382` (test starts at `:252`) (3 weapons × 3 armour classes; zero case now through an ordered attack, above); unit test `combat.rs:841-857` |
| Range at exactly `range` and at `range + 1` subcell | Verified | `a_target_is_in_range_at_exactly_the_range_and_not_one_subcell_beyond` (crawler at 1 subcell per tick: first shot at 768 after 769); building edges 640/896; 384 and 4,096; unit test `combat.rs:861-871` |
| A target that dies while a projectile is in flight | Verified | `a_target_that_dies_while_a_projectile_is_in_flight_makes_it_expire` |
| Attacks on own, dead, and unseen units are rejected | Verified | `attacks_on_own_dead_and_unseen_units_are_rejected_without_changing_state` (hash equal to a control run) |
| A destroyed unit or building releases its cells | Verified | `a_destroyed_unit_releases_both_cells_of_its_step`, `a_destroyed_building_frees_its_cells` |
| Sight radius boundary | Verified | `a_unit_sees_exactly_the_cells_within_its_sight_radius`; `visibility.rs` unit tests |
| Determinism of a 20 against 20 battle | Verified | Same per-tick trace across runs; same final hash `05af3b1ac550cf16` in debug and release (rerun above) |
| Acceptance: two armies fight to a conclusion under fog, no panic, stable hash | Verified in `sim` | Battle test; `zero_chaos` probe. In the game, combat is not exercised by the smoke test (carried debt); on screen it **requires user verification**. |
| Scope: weapons, warheads, armour classes, instant and travelling projectiles | Verified | `combat_defs.rs`, `combat.rs`; hostile-data probes below |
| Scope: automatic target acquisition | Verified | P1-1 resolved (above) |
| Scope: `Attack`, `AttackMove` | Verified | Existing tests plus the P1-1 regression tests; every order ends (`zero_chaos`) |
| Scope: death and cleanup; building destruction frees cells; defensive structures (low power) | Verified | `remove_dead`; `defensive_structures_hold_fire_under_low_power`; `a_defensive_structure_fires_at_an_enemy_it_can_damage_and_not_at_a_nearer_one_it_cannot` |
| Scope: shroud and fog | Verified in `sim`; on screen **requires user verification** | `visibility.rs`; `fog.rs` tests; smoke run (fog computed, 0 view mismatches) |
| Scope: hiding of unseen enemies in rendering | Views and instant shots verified; projectile trajectories show their bearing (new P2-1) | `units.rs`, `buildings.rs`, smoke check, `shot_view`; probe `projectile_from_unseen_attacker` |
| Scope: hiding of unseen enemies in command validation | Verified | P1-2 resolved; probe `placement_indistinguishable` |

### Section 8 test categories

| Category | Coverage |
|----------|----------|
| Positive | Present. |
| Negative | Every rejection reason with no state change (including `Placement(NotVisible)`, with a control-run hash); 12 data-file tests; 47 hostile asset roots per profile (below). |
| Boundary | Range ±1 subcell, sight radius, data limits, percentage 0 and 1. |
| Regression | Every review 1 defect with a code change has a test that fails before and passes after (verified above for P1-1 and P1-2; P2-1 by the pure decision function). P2-2 removes an unreachable branch, so no failing test is possible. P2-3 is documentation. |
| Integration | All `m4_combat` tests run through `SimPlugin` and `CommandQueue`. |
| End to end | Both smoke runs. |

No test was removed, skipped, or weakened (claim 1 above).

### Invariants (sections 6 and 12)

| Invariant | Verdict and evidence |
|-----------|----------------------|
| I1 | `cargo tree -p sim -e normal` lists no windowing, rendering, input, or audio crate. `Cargo.lock`, the manifests, and `rust-toolchain.toml` are unchanged since `m3-reviewed`. |
| I2 | The only float in `crates/sim/src` is the carried `map_subset.rs:524` (M1 P2-B). |
| I3 | Hash covers combat components, weapons, visions, and projectiles (`hash.rs`). Battle trace stable. `zero_chaos` digest equal in debug and release. |
| I4 | `BTreeMap` iteration and `SimId` order in acquisition (`nearest_enemy_in_range` breaks ties by id); vision is a union. |
| I5 | `crates/game/src` holds no `ResMut` of `sim` resources and no `&mut` query of `sim` components (grep). `fog.rs` reads `Players`, `Projectiles`, and `Definitions` only. |
| I6 | Checked on every tick of every `m4_combat` test and of the probes: 30 × 6,000 ticks in `zero_chaos`, plus every placement world. |
| I7 | Credits ≥ 0 on every probe tick. |
| I8 and I11 | 47 hostile `combat.ron` and sprite variants rejected with typed errors, with no panic in either profile (below). No `unwrap`, `expect`, or `panic!` in `combat.rs`, `combat_defs.rs`, `visibility.rs`, `building.rs`, `economy_commands.rs`, or `command.rs`. Slice indexing in `combat_defs.rs:532/612/637` uses indices returned by `reference` into the same list. |
| I9 | Unchanged. |
| I10 | Production combat values only in `assets/data/combat.ron`. Tests use fixtures; the remediation's variants are written to `CARGO_TARGET_TMPDIR`. |
| I12 | `applied_damage` in `i64` clamped; `in_range` in `i128`; saturating cooldown, chases, damage sums, and flight; `damages` reuses `applied_damage`. Debug and release digests are equal. |

### Equations and units (section 5)

* **Equation 1** (`in_range`, `i128`): exact at the boundary in both the test and the unit test.
* **Equation 2** (`applied_damage`): `div_euclid` floor, `max(1)` for versus > 0, and 0 for versus ≤ 0. `damages` is equation 2 > 0, so acquisition uses exactly the equation.
* **Equation 6** (`Vision::reveal`): inclusive `dx² + dy² ≤ s²` in cells, measured to the nearest footprint cell, clipped to the map, with saturating arithmetic.
* **Ticks:** reload and flight are in ticks. The projectile flight time is `max(1, ceil(isqrt(dx² + dy²) / speed))`, at most 32.

### Hostile-input probes (real binaries, both profiles)

**Method.**

* `make_roots.py` built 47 scratch asset roots (copies of `assets/`).
* Each root was run with `target/{release,debug}/game --smoke-test 120`, with `BEVY_ASSET_ROOT` set to the root, under `perl alarm 60` and `/usr/bin/time -l`.
* Every run had its working directory in the scratchpad and ran in the foreground, one at a time.
* There was no watchdog kill (no exit 137) and no panic in any log.
* Peak footprint was ≤ 0.120 GB.
* Rejected roots exited 1 within 0.26 s (release) or 0.64 s (debug).

**Rejected, exit 1, typed `data file … : <field>: <reason>`** (identical messages in both profiles):

* **File:** missing, empty, oversize (> 65,536 bytes), FIFO, symlink to `/dev/zero`, directory, symlink loop (`Too many levels of symbolic links`).
* **Syntax:** byte order mark (`found byte 0xef`), non-ASCII comment, `#![enable(implicit_some)]`, block comment, float `300.0`, hex `0x12C`, truncation, 20,000-deep list nesting (`units[0].weapons[0]: … expected a string, found '['`).
* **Numeric values:**
  * percent 1,001 and −1;
  * range 383 and 4,097;
  * projectile speed 127 and 4,097;
  * reload 0 and 1,201;
  * hit points 0 and 1,000,001;
  * hit points `i64::MIN`, and `i64::MAX` + 1 (`expected a 64-bit signed integer`);
  * sight 17.
* **References and entries:**
  * duplicate armour class in `versus` (`warheads[0].versus[1].armour: identifier "light" is defined more than once`);
  * duplicate weapon id;
  * missing `versus` entry (`armour class "light" has no entry`);
  * missing `gun_turret` entry (`building "gun_turret" has no entry`);
  * unknown warhead, unknown armour class, two weapons on one unit (`more than 1 entries`).
* **Fields and schema:** unknown field, schema 2, string of 65 bytes.
* **Sprite:** `gun_turret.png` as a FIFO (`not a regular file`) and truncated (`chunk "IDAT": the file ends inside this chunk`).

**Accepted, exit 0, with 0 view mismatches:** the unchanged copy, CRLF line endings with tabs, every percentage 0 (nothing can damage anything), damage 100,000 with a percentage of 1,000, range 4,096 with projectile speed 128, sight 0 everywhere (27 visible cells), and an armed harvester. Release and debug differ only in the number of ticks run within 120 frames.

### Implementer's claims checked

The following claims of the remediation report were reproduced:

* every validation result and test count;
* both smoke log lines;
* the battle figures;
* the "fails before" claims;
* the statement that no dependency, manifest, lock file, data file, fixture, or asset changed (`git diff --stat 0cb2199..milestone/m4`).

Inaccurate claims:

* **D33's title** ("Commands and the screen reveal nothing about what the player does not see");
* **D33 item 4;**
* **the report's statement that the on-screen member of the class is fixed.**

All three are too strong for projectiles (new P2-1).

## New findings

There are no P0 and no P1 findings.

### P2-1: Projectiles from an unseen attacker are drawn along their line of flight, which points back to the attacker; D33 claims the screen reveals nothing

**Files and lines:**

* `crates/game/src/fog.rs:205-215`: every projectile is drawn at `projectile_point(projectile.origin, target, progress)` whenever that point's cell is visible.
* `crates/game/src/fog.rs:146-150`: `projectile_point`, a linear interpolation from the origin.
* `crates/sim/src/combat.rs:479`: `origin: from.centre()`, the attacker's exact centre.
* `docs/DECISIONS.md:499`: the D33 title, "Commands and the screen reveal nothing about what the player does not see".
* `docs/DECISIONS.md:514`: D33 item 4, "so a shot no longer shows where an unseen attacker stands".
* `docs/STATUS.md:1358`: "The on-screen member of the same class (P2-1 …) is fixed".

**Evidence:** probe `projectile_from_unseen_attacker` (release, fixture data). It computes, with the same interpolation as `fog.rs`, the points the game draws.

* **Setup:** a human crawler (sight 2) stands at (10, 16). An unseen enemy tank at (14, 16) fires a shell (speed 128) at it.
* **Points drawn:**
  * the shell's points at x 3200, 3072, 2944, and 2816 (cells (12, 16) and (11, 16)) are visible and drawn;
  * the origin cell (14, 16) is not visible.
* **What the player learns:** the drawn dots move along y = 4224 by exactly 128 subcells per tick, on the straight line from the hidden tank's centre. The player sees the bearing of the unseen attacker, as review 1 P2-1 found for instant shots.
* **Accuracy of the records:** D31 item 3 ("Projectiles are drawn … only in cells the human player sees") is accurate. D33's general claim, and the report's statement that the class is fixed on screen, are not.

**Severity rationale:** P2.

* This is documentation accuracy, plus a minor information leak that is conventional in the genre.
* Simulation state, command validation, and invariants are unaffected.
* The precedent is review 1 P2-1, rated P2.

**Expected behaviour:** either of the following.

* Correct D33 (title and item 4) and the remediation report to state that projectiles in flight are drawn in visible cells, so their line of flight shows the bearing of an unseen attacker, and record it as accepted behaviour.
* Or draw a projectile only once its attacker has been seen, or not before it is within a bounded distance of a visible target. A unit test of that decision would accompany this option.

## Observations (not findings)

* **No in-repository test of `NoDamage` with a projectile weapon.** The regression tests for `AttackEnd::NoDamage` use the instant rifle only. The projectile path goes through the same `fire_if_ready` return value, and the reviewer's probe `attack_nodamage_projectile_and_mixed` confirms that it ends after one shell.
* **Production consequence of the `NotVisible` rule** (D33 item 5, STATUS remediation debt item 3). With the production data (placement distance 4; yard sight 5; power plant sight 4), corner placements within the placement distance are rejected until something of the player sees them. This is documented and listed for user verification (item 9 below).
* **Debug versus release smoke counts.** The two profiles differ in simulation ticks per 600 frames (198 against 203), which is wall-clock frame pacing (M0 debt). Only the counts tied to tick number differ.

## Requires user verification

These are not findings and do not affect the decision (PLAN.md section 12 item 2); they block only `m4-approved`. Run `cargo run --release -p game` and check the M4 items from docs/STATUS.md, as amended by the remediation report:

1. **Shroud:** at the start, only the area around the player's units and construction yard shows the map; everything else is black.
2. **Fog:** cells seen before and not now are drawn dark but recognisable; cells seen now are drawn normally; moving units uncover the map.
3. **Hidden enemies:** the opponent's units, yard, and gun turret at (50, 41) are not drawn until a unit of the player comes close, and they disappear again when no longer seen.
4. **Attack:**
   * A right click on a visible enemy makes the selected armed units close in and fire.
   * Tank shells are drawn as small yellow dots.
   * Scout shots are drawn as short pale lines only when both the scout and its target are on visible cells; otherwise a small pale circle is drawn at the visible end.
   * Destroyed units and buildings disappear, and units can walk over a destroyed building's cells.
5. **Attack-move:** `A` over a cell makes the selection fight enemies in range on the way, then continue.
6. **Automatic fire:** idle units and turrets fire only at visible enemies in range that their weapon can damage; units on a plain move do not fire.
7. **Gun turret:**
   * the opponent's turret fires at the player's units within about six cells;
   * the player's turret (key `6` after a power plant, placed with `B`) fires while the player has power and holds fire under `(low)`.
8. **Menu keys:** `7` light tank, `8` scout, `9` harvester.
9. **Placement under fog:** `B` is rejected ("cell (x, y) is not visible to the player") while any footprint cell is not currently visible, and succeeds once every cell is seen.

The ten M3 items, the seven M2 items, and the five M1 items listed in docs/STATUS.md remain pending.

## Recorded debt that must be carried in docs/STATUS.md

**New in this review (P2):**

* **P2-1:** projectiles from unseen attackers show their line of flight, and the D33 title, D33 item 4, and the remediation report's claim are too strong.

**From the M4 remediation round 1 report (known debt), confirmed by this review:**

1. Outcomes of accepted orders use the true state: group destinations, paths, and harvester ore claims (D33 item 3).
2. The `SimEvent` buffer is one stream for all players. The game's terminal log writes every event of both players.
3. Placement is limited to cells the player sees (D33 item 5). There is no validity preview until M5.
4. An ordered attack on an undamageable target fires one harmless shot before it ends (D32 item 3). It is confirmed for instant and projectile weapons.

**From the M4 implementation report, confirmed by review 1 and unchanged except as noted:**

1. **Cost per tick.** Acquisition is O(A × T), a chase runs a Dijkstra, and vision is recomputed in full. Review 1 measured 300 per side at a mean of 1.09 ms and a max of 2.84 ms in release. To be measured against the M7 benchmark.
2. **Projectile bound.** It allows about 200 MB in the pathological case.
3. **No chase or return fire beyond range.** Idle and attack-moving units do not chase, and do not return fire against attackers that outrange them.
4. *Resolved by D32.* (This was the `Attack` on an undamageable target that lasted while the target stood.)
5. **Second `MoveCompleted`.** An attack-move that fights at its destination writes a second `MoveCompleted`.
6. **Unreachable missing-entry cases.** A missing weapon or percentage is treated as no damage (now also in `Field::damages`), and a projectile whose weapon is missing as expired.
7. **`with_economy` order.** `Definitions::with_economy` removes combat definitions loaded before it. This is documented and tested.
8. **Health bars and shot events.** Health bars are M5's. The two-tick `MessageReader` window also applies to instant shots.
9. **No combat in the smoke test.** The smoke test does not exercise combat in the real binary, and there is no automated screenshot.
10. **Pending free units.** M3 review 1 P2-1 is unchanged for selling; destruction reports the lost units.
11. **Earlier debt.** Everything carried at the M3 review decision, unchanged.

**Carried from M3 review 1, unchanged by M4:**

* P2-1, narrowed as in item 10 above;
* P2-2: dock contention;
* P2-3: `docs/DECISIONS.md` still cites the nonexistent `two_placements_in_one_tick_see_each_other`;
* P2-4: `STARTING_CREDITS` and the starting setup in game code, which M4 extends with the opponent's gun turret;
* M3 implementation debt items 1 to 9.

**Carried from M2, M1, and M0, unchanged by M4:**

* M2:
  * review 3: P2-1 and P2-2;
  * remediation round 2: items 1 to 7;
  * review 1: P2-2 remainder.
* M1:
  * review 3: P2-1 (FIFO hang of the map and `.tsx` reader), P2-2, and the remaining items of P2-3;
  * review 1: P2-3 and P2-5;
  * review 2: P2-B (the `f32` in `map_subset.rs:524`);
  * round 1: item 4;
  * round 2: items 5 and 6.
* M0: items 1, 2, 3, 5, 7, 8, and 9. This review saw the linker warning and the `Skipped event Destroyed` warning again.

**Resolved and confirmed in this review:** review 1 P1-1, P1-2, P2-1 (instant shots), P2-2, and P2-3; M4 implementation debt item 4.

## Decision

The review finds:

* no unresolved P0 or P1: both P1 findings of review 1 are resolved, with regression tests that fail before the fix and pass after it;
* all six validation commands pass;
* every M4 acceptance criterion and required test verified. The on-screen items are listed for user verification and, under section 12 item 2, do not block;
* no test removed, skipped, or weakened;
* no unresolved numerical or data-integrity risk;
* no broken compatibility: no dependency or schema change. The public enum changes are confined to variants introduced in M4.

The one new finding is P2 and is recorded above for docs/STATUS.md.

Decision: GO WITH DOCUMENTED NON-BLOCKING DEBT
