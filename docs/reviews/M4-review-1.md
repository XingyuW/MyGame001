# M4 review 1: Combat and fog of war

Independent review of milestone M4 under PLAN.md sections 10 and 12. The reviewer did not write or remediate any of this code, and changed no repository file.

## Scope and base

* **Repository:** `/Users/xingyuwang/GameProjects/MyGame001`, branch `milestone/m4`, at HEAD `7506cbd`. The working tree was clean before and after the review.
* **Base:** tag `m3-reviewed`, the most recent `mN-reviewed` or `mN-approved` tag (PLAN.md section 12 item 1).
* **Diff reviewed:** `git diff m3-reviewed..milestone/m4`, 33 files, +6,759 / −103. It covers three commits:
  * `9608923`: sim, combat, projectiles, fog of war;
  * `e19677d`: game, combat and fog;
  * `7506cbd`: decisions and report.
* **Read in full:** PLAN.md (including section 12), docs/STATUS.md, and docs/DECISIONS.md.
* **Code read in full:**
  * new files `crates/sim/src/{combat,combat_defs,visibility}.rs` and `crates/game/src/fog.rs`;
  * `crates/sim/tests/m4_combat.rs`;
  * the diffs of every other changed file;
  * the production and fixture data.
* **Third-party code that consumes external data:** the `ron` 0.12.2 recursion guard (`de/mod.rs:136-154`, and per-element guards at `:1117`) and `sim::data::parse`. The PNG path for `gun_turret.png` is the existing `check_png_file` subset.
* **Probes:** all ran outside the repository, under `…/scratchpad/m4-review-1-probe/`. Each ran in the foreground, one at a time, under `perl alarm`. No probe was killed by the watchdog: there was no exit 137, and every peak was ≤ 0.12 GB.

## Validation commands (PLAN.md section 9 and section 12 item 5)

These were run by the reviewer from the repository root, in this order, in the foreground (macOS, Apple M5, Rust 1.98.1). A display was available.

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 300 passed, 0 failed, 0 ignored (see below) |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | see below |
| `cargo run -p game -- --smoke-test 600` | 0 | see below; M0 debt item 1 linker warning only |

**Test counts.**

* `game`:
  * unit tests: 67;
  * integration tests: `m0_spike` 6, `m1_demo_map` 3, `m1_tileset_image` 6, `m2_unit_sprites` 5, `m3_building_sprites` 3.
* `sim`:
  * unit tests: 11;
  * integration tests: `m0_bootstrap` 12, `m1_map` 32, `m1_map_subset` 24, `m2_definitions` 14, `m2_movement` 37 (39.45 s), `m3_definitions` 11, `m3_economy` 21, `m3_harvest` 10, `m4_combat` 26, `m4_definitions` 12.

**Release smoke run:**

```
smoke test: 600 frames, 198 simulation ticks, map created: true, units spawned: 26, units moved: 18 in 198 ticks after the order, buildings spawned: 3, production charged 396 credits in 198 ticks, visible cells: 389, enemy views shown: 0, view mismatches: 0
```

**Debug smoke run:**

```
smoke test: 600 frames, 203 simulation ticks, …, production charged 406 credits in 203 ticks, visible cells: 399, enemy views shown: 0, view mismatches: 0
```

The `Skipped event Destroyed` warning (M0 debt item 2) appeared in both smoke runs.

**Additional runs:**

* `cargo test --release -p sim` passed: all 210 `sim` tests (11 unit tests plus 199 integration tests).
* `m4_combat::twenty_against_twenty…` with `--nocapture` gives the same result in debug and release: `ended at tick 665, survivors {0: 6, 1: 0}, 451 shots, 34 units destroyed, final hash 05af3b1ac550cf16`.

## Verification summary

### Acceptance criteria and required tests (PLAN.md section 7, M4)

| Item | Verdict | Evidence |
|------|---------|----------|
| Damage equation for every armour/warhead pair of the fixtures, minimum 1, zero case | Verified | `m4_combat.rs:252-369`: 3 weapons × 3 armour classes through `SimPlugin`/`CommandQueue`, compared with an independent formula; asserts the zero, minimum-one, floor, and exact cases. Unit test `combat.rs:812-827`. |
| Range at exactly `range` and at `range + 1` subcell | Verified | `m4_combat.rs:374-405`: a crawler at 1 subcell per tick; the first shot comes at a distance of exactly 768, after 769. Also building edges, 384 and 4096, and `combat.rs:831-841`. |
| A target that dies while a projectile is in flight | Verified | `m4_combat.rs:581-628`: `ProjectileExpired`, no damage, no projectile left. |
| Attacks on own, dead, and unseen units rejected | Verified | `m4_combat.rs:633-815`: each rejection reason is checked, with the hash equal to a control run. |
| A destroyed unit or building releases its cells | Verified | `m4_combat.rs:921-1023`: both cells of a step; the footprint is uncovered and passable. |
| Sight radius boundary | Verified | `m4_combat.rs:1143-1209` (the independent set of equation 6, corners, and the radius ±1); `visibility.rs:210-238`. |
| Determinism of a 20 against 20 battle | Verified | `m4_combat.rs:1699-1802`; per-tick hash trace identical across runs; same final hash in debug and release (rerun above). |
| Acceptance: two armies fight to a conclusion under fog, no panic, stable hash | Verified for the fixture battle in `sim` | As above. In the game, combat is not exercised by the smoke test (implementer debt 9); on screen it **requires user verification**. |
| Scope: weapons, warheads, armour classes, instant and travelling projectiles | Verified | `combat_defs.rs`, `combat.rs`; hostile-data probes below. |
| Scope: automatic target acquisition | **Defective**: P1-1 | It targets enemies the warhead cannot damage. |
| Scope: `Attack`, `AttackMove` | `Attack` verified; `AttackMove` **defective** (P1-1) | `m4_combat.rs:1303-1519`; probe `zero_versus`. |
| Scope: death and cleanup; building destruction frees cells; defensive structures (low power) | Verified | `combat.rs:768-803`; `m4_combat.rs:1522-1578`. |
| Scope: shroud and fog | Verified in `sim`; on screen **requires user verification** | `visibility.rs`; `fog.rs` tests. |
| Scope: hiding of unseen enemies in rendering | Views verified; instant-shot lines reveal attackers (P2-1) | `units.rs`, `buildings.rs`, smoke check; probe `leak_shot`. |
| Scope: hiding of unseen enemies in command validation | **Incomplete**: P1-2 | `PlaceBuilding` names unseen enemy units and buildings; probe `leak_place`. |

### Section 8 test categories

| Category | Coverage |
|----------|----------|
| Positive | Present. |
| Negative | Every new rejection reason except the unreachable `NotTargetable` (P2-2); 12 data-file tests. |
| Boundary | Present: range ±1 subcell, sight radius, data limits. |
| Integration | All `m4_combat` tests run through `SimPlugin` and `CommandQueue`. |
| End to end | The smoke runs (fog computed, view hiding consistent). |
| Regression | Not applicable: this is round 1. |

No test was removed, skipped, or weakened:

* `git diff` shows no removed assertion or `#[test]`;
* no `#[ignore]` was added;
* the M2 and M3 fixtures are unchanged;
* the test count went from 248 to 300, all additions.

### Invariants (sections 6 and 12)

| Invariant | Verdict and evidence |
|-----------|----------------------|
| I1 | No forbidden crate in `cargo tree -p sim -e normal`. `Cargo.lock`, the manifests, and the toolchain are unchanged since `m3-reviewed`. |
| I2 | The only float in `crates/sim/src` is the carried `map_subset.rs:524` (M1 P2-B). |
| I3 | Hash extended for health, armour, sight, weapon, vision, and projectiles. Probe `chaos`: 40 seeds × 2,000 ticks of random hostile commands (`Attack`, `AttackMove`, `Move`, `Stop`, `SellBuilding`, including foreign units and a non-player), with turrets, refineries, and harvesters. It gives the same combined per-tick hash digest `cafb9f045147ace2` in debug and release. |
| I4 | `BTreeMap`/`BTreeSet` and `SimId` order everywhere; vision is a union. |
| I5 | The game only reads `Players`, `Projectiles`, `Definitions`, `Weapon`, `Building`, `Owner`, `Mobile`; no `ResMut` or `&mut` of `sim` state (grep). |
| I6 | Checked on every tick of every `m4_combat` test and of the `chaos` probe (80,000 ticks): no violation; 1,461 units and buildings destroyed. |
| I7 | Credits ≥ 0 on every tick, in the tests and the probe. |
| I8 and I11 | `combat.ron` passes the RON subset checker before `ron`; recursion is bounded by the schema, with `ron`'s limit of 16 as a second line. See the hostile-data probes below. |
| I9 | Unchanged. |
| I10 | Production combat values are in `assets/data/combat.ron`; the tests use fixture data. |
| I12 | `i64` extents, `i128` range checks, saturating damage, cooldowns, and chases; vision computed in `i64`. The debug and release digests are equal (above). |

### Equations and units (section 5)

* **Equation 1** (`in_range`, `i128`): exact at the boundary (test and unit test).
* **Equation 2** (`applied_damage`, `i64`, `div_euclid`, `max(1)`, 0 for versus ≤ 0): correct.
* **Equation 6:** visibility in cells, inclusive. Matches an independent implementation at the radius and in corners.
* **Ticks:** reload and flight time are in ticks.
* **Projectile flight:** `max(1, ceil(isqrt(dx² + dy²) / speed))`, at most 32 ticks. Verified with the slowest projectile at the longest range.

### Hostile-input probes (real binaries, debug and release, scratch asset roots)

**Variants run.** Each binary was run as `--smoke-test 60` under a 60 s alarm against 59 variants:

* 55 `combat.ron` and economy variants;
* a 30,000-deep list nesting within 64 KiB;
* three `gun_turret.png` variants: missing, wrong size, and a named pipe.

**Rejected variants.** Every hostile variant exited 1 within about 0.25 s (release) or 0.65 s (debug), with a typed `data file <file>: <field>: <reason>` error. Peak memory was ≤ 0.11 GB. There was no panic and no hang. The variants:

* file: missing, empty, oversize, FIFO, symlink to `/dev/zero`, directory;
* syntax: byte order mark, `#![enable(implicit_some)]`, block comment, float, hex, struct name, truncation;
* numeric values: `i64` max, min, and overflow; every numeric field at min − 1 and max + 1 (hit points, range, reload, projectile speed (1, 127, 4097, −1), damage, percent, sight);
* lists and references: two weapons, 34 weapons, 17 armour classes; unknown warhead, weapon, armour, and unit;
* duplicates and missing entries: duplicate armour class, duplicate warhead class, missing warhead class; unit or building without an entry, and a new economy building without a combat entry;
* fields and schema: unknown field, missing field, schema 2, string of 65 bytes;
* sprite: turret sprite missing, wrong size, or a FIFO.

**Accepted variants.** The unchanged copy, CRLF line endings with tabs, all values at their valid extremes, and sight 0 everywhere each exited 0.

**Sample messages:**

* `buildings: building "wall" has no entry`;
* `warheads[0].versus: armour class "structure" has no entry`;
* `units[0].hit_points: value 9223372036854775808 is not supported, expected a 64-bit signed integer`;
* `file: cannot read the file: not a regular file`.

**Performance (informational, M7 budget).** Release, demo map, production data, attack-moving armies:

* 100 per side: mean 0.143 ms, max 1.31 ms per tick;
* 300 per side: mean 1.09 ms, max 2.84 ms, at most 27 projectiles in flight.

### Implementer's claims checked

Every validation result, test count, smoke log line, battle figure (tick 665, 451 shots, 34 destroyed, hash `05af3b1ac550cf16`), and probe outcome in the M4 report was reproduced. The claims that are not accurate:

* **D29 item 7:** "The order ends when a move toward the destination ends." This is false when an enemy that cannot be damaged is in range (P1-1).
* **D30 item 4:** "commands reveal nothing about unseen enemies." This is false for `PlaceBuilding` (P1-2). The same item says `NotTargetable` is "possible only without a combat file"; it is unreachable in all cases (P2-2).
* **D31 item 3:** shots are drawn "only where the human player sees them." This is false for instant shots (P2-1).

## Findings

### P1-1: Target acquisition engages enemies the warhead cannot damage, so attack-move orders never end and units do not return fire

**Files and lines:**

* `crates/sim/src/combat.rs:398-415` (`nearest_enemy_in_range`): it picks the nearest visible enemy in range by distance alone, without the warhead's percentage against that target's armour.
* `crates/sim/src/combat.rs:597-614` (`keep_or_acquire`): it keeps such a target.
* `crates/sim/src/combat.rs:712-726`: an `AttackMove` unit with any target "stops and fights" (`orders.current = None`) and does not resume until the target is gone.
* `crates/sim/src/combat.rs:741-749`: idle units.
* `combat.rs:567-576`: defensive structures use the same rule.
* `docs/DECISIONS.md` D29 items 5 and 7.

**Evidence** (probes `zero_versus` and `zero_idle`, release). Both use the M4 fixture data, in which `bullet` deals 0 % against `concrete`, a case that equation 2 explicitly specifies and the data format admits.

* **Attack move.** A rifleman attack-moves along y = 16 from (4, 16) to (20, 16) past an enemy `fence` (concrete, 50 hit points) at (12, 18).
  * After 20,000 ticks, the move has not ended.
  * The rifleman still stands at (9, 16), with order `AttackMove { destination: (20, 16), moving: false }` and target the fence.
  * It has fired 1,997 shots, and the fence still has 50 hit points.
  * No `MoveCompleted` or `MoveFailed` is ever written, so the order never ends.
* **Idle unit.** An idle human rifleman at (10, 16) has an enemy fence at (11, 16) and an enemy rifleman at (12, 17), both in range.
  * It fires 5 shots at the fence (0 damage) and none at the rifleman.
  * The enemy rifleman fires 5 shots at it and kills it at tick 41.
  * A unit under fire does not return fire because a harmless building is nearer.
* **Tests.** No test covers target acquisition when a versus-0 pair is in range, although the fixture contains one. Implementer debt item 4 covers only explicit `Attack` orders.

**Severity rationale:** P1.

* Automatic target acquisition and `AttackMove` are M4 scope items.
* The behaviour is reachable with valid data, and the decision record's claim that an attack-move ends when its move ends does not hold.
* An order that never ends is the class rated P1 in M2 review 1 (P1-1).
* The behaviour is not tested.
* It is not P0, because PLAN.md does not specify target-choice rules and nothing panics or breaks an invariant.

**Expected behaviour:**

* Automatic acquisition (idle units, attack-moving units, and defensive structures) considers only targets against which the weapon deals damage (`applied_damage > 0`), or at least prefers them.
* An attack-moving unit does not stop for a target it cannot damage, so its order ends when its move ends.
* An explicit `Attack` keeps the documented semantics.
* Regression tests through `SimPlugin`/`CommandQueue` with the fixture's bullet-against-concrete pair:
  * the attack move past a fence completes;
  * an idle rifleman next to a fence returns fire at the enemy rifleman.
* D29 items 5 and 7 are corrected.

### P1-2: `PlaceBuilding` rejections reveal the id and cell of unseen enemy units and buildings, including in unexplored shroud

**Files and lines:**

* `crates/sim/src/economy_commands.rs:223-240`: `PlaceBuilding` calls `check_footprint` with no fog handling.
* `crates/sim/src/building.rs:127-129` and `:188-191`: `PlacementError::Overlap { cell, building }` and `Occupied { cell, unit }` carry the blocker's `SimId` and cell. The `Display` text "cell (x, y) is held by unit N" is logged by the game.
* `docs/DECISIONS.md:465` (D30 item 4): "…so that commands reveal nothing about unseen enemies … Commands that name cells (… `PlaceBuilding` …) are not restricted to visible cells: … placement already requires an own building within the placement distance."
* `docs/STATUS.md:1310`: deviation 4.

**Evidence** (probe `leak_place`, release, M4 fixture data):

* **Setup.** The human yard is at (10, 10); its sight is 4, and the placement distance is 3. An enemy `crawler`, or in a second run an enemy `beacon`, stands at (16, 14). For the human player that cell is neither visible nor explored. The human queues a power plant (2 × 2) and places it at (15, 13), a legal distance of 3.
* **Result with the crawler:** `Placement(Occupied { cell: (16, 14), unit: SimId(1) })`, printed "cell (16, 14) is held by unit 1".
* **Result with the beacon:** `Placement(Overlap { cell: (16, 14), building: SimId(1) })`.
* **Comparison.** The same id named in `Attack` is reported as `UnknownTarget`, as D30 intends for unseen enemies.
* **Why the deviation's rationale fails.** The placement distance is a Chebyshev distance between footprints, while sight is Euclidean from the nearest cell. The far cells of a footprint 3 cells away therefore lie outside an own building's sight. The same holds for the production data: `placement_distance` 4, power plant sight 4, yard sight 5.

**Severity rationale:** P1.

* M4's scope requires "hiding of unseen enemies … in command validation", and PLAN.md section 5 requires every command to be validated for visibility.
* The implementation hides unseen enemies in the other commands but leaks them through `PlaceBuilding` rejections. Those rejections are what the interface (M5) and the AI (M6) will read.
* No test covers placement against an unseen blocker.
* The deviation from section 5 was recorded with a rationale that does not hold, and the user has not approved it.

**Expected behaviour:**

* Under fog, a placement whose footprint includes a cell the player does not see is rejected with a reason that names no hidden entity (for example a new `PlacementError::NotVisible(cell)`, or `Occupied`/`Overlap` without the id for unseen blockers), checked before the occupancy checks.
* Alternatively, the user explicitly accepts the leak and the decision record is corrected.
* A regression test covers an unseen unit and an unseen building inside an otherwise legal footprint, and asserts that the rejection reveals neither.
* D30 item 4 and deviation 4 are corrected to match.

### P2-1: Instant-shot lines are drawn from unseen attackers, revealing their position

**Files and lines:**

* `crates/game/src/fog.rs:190-200`: `if seen(from) || seen(to)` draws the whole line from the attacker's body point to the target.
* `docs/DECISIONS.md:473` (D31 item 3): "…instant shots as short lines for six frames, only where the human player sees them".

**Evidence:** probe `leak_shot` (release, production data, fixture map).

* A human `power_plant` at (20, 20) has sight 4 from its nearest cell. An enemy `scout` at (25, 23) has an instant `scout_gun` with range 1,024.
* At tick 1 the scout fires at the plant. The scout's cell is not visible to the human, while the plant's centre cell (21, 21) is.
* By the code above, the line is drawn from the hidden scout's exact body point. The scout's sprite stays hidden.

**Expected behaviour:**

* Draw an instant shot only if the attacker is visible, or clip the line to its visible part (or draw only the impact at the visible target).
* Alternatively, correct D31 item 3.
* Add a unit test of the decision.

### P2-2: `RejectReason::NotTargetable` is unreachable, contrary to D30

**Files and lines:**

* `crates/sim/src/command.rs:584`;
* `docs/DECISIONS.md:465` ("`NotTargetable`, possible only without a combat file").

**Evidence:**

* Without a combat file no unit has a `Weapon`, so `validate_combat` returns `NotArmed` before it looks at the target (`command.rs:552-560`). An empty unit list is `NoUnits` earlier still.
* With a combat file, every unit and building type has combat terms (`combat_defs.rs:653-667`).
* Hence the branch cannot be reached by any command, and no test covers it (`grep` finds no test use).

**Expected behaviour:** remove the variant, or document it as a defensive branch that cannot be reached, and correct D30.

### P2-3: The decision record of the M3 smoke test was altered

**File and lines:** `docs/DECISIONS.md:93-95` (D6).

**Evidence:**

* The M4 commit removed the sentence "The final log line also reports the buildings spawned and the credits charged." from the M3 amendment.
* It appended the same sentence to the end of the M4 amendment, where it follows "…the visible cells, the enemy views shown, and the mismatches." (`git diff m3-reviewed..milestone/m4 -- docs/DECISIONS.md`).

**Expected behaviour:** restore the sentence to the M3 amendment and remove it from the M4 amendment.

## Requires user verification

These are not findings and do not affect the decision (PLAN.md section 12 item 2); they block only `m4-approved`. Run `cargo run --release -p game` and check the eight M4 items in docs/STATUS.md under "M4 implementation report":

1. **Shroud:** at the start, only the area around the player's units and yard shows the map; everything else is black.
2. **Fog:** cells seen before are drawn dark but recognisable; cells seen now are drawn normally; moving units uncover the map.
3. **Hidden enemies:** the opponent's units, yard, and turret at (50, 41) are not drawn until a unit of the player comes close, and they disappear again when no longer seen.
4. **Attack:** a right click on a visible enemy makes the selected armed units close in and fire.
   * Tank shells are drawn as yellow dots; scout shots as short pale lines (see P2-1).
   * Destroyed units and buildings disappear, and units can walk over a destroyed building's cells.
5. **Attack-move:** `A` over a cell makes the selection fight enemies in range on the way, then continue.
6. **Automatic fire:** idle units fire at visible enemies in range; units on a plain move do not.
7. **Gun turret:**
   * the opponent's turret fires at units within about six cells;
   * the player's turret (key `6` after a power plant, placed with `B`) fires while the player has power and holds fire under `(low)`.
8. **Menu keys:** `7` light tank, `8` scout, `9` harvester (M3's item 7 named `6`, `7`, `8`).

The ten M3 items, the seven M2 items, and the five M1 items listed in docs/STATUS.md remain pending.

## Recorded debt that must be carried in docs/STATUS.md

**New in this review (P2):**

* **P2-1:** instant-shot lines reveal unseen attackers; D31 item 3 is inaccurate.
* **P2-2:** `NotTargetable` is unreachable; D30 is inaccurate.
* **P2-3:** the D6 M3 amendment sentence was moved into the M4 amendment.

**From the M4 implementation report (known debt), confirmed by this review:**

1. Cost per tick of target acquisition (O(A × T)), chase Dijkstra, and full vision recompute. Measured here: 300 per side, mean 1.09 ms and max 2.84 ms in release; to be measured against the M7 benchmark.
2. The projectile bound allows about 200 MB in the pathological case.
3. Idle and attack-moving units do not chase, and do not return fire against attackers that outrange them.
4. An explicit `Attack` on a target its warhead cannot damage lasts while the target is alive and visible. The acquisition part of this class is P1-1 and must be fixed.
5. An attack-move that fights at its destination writes a second `MoveCompleted`.
6. Missing weapon or percentage is treated as no damage, and a projectile whose weapon is missing as expired. These cases are unreachable.
7. `Definitions::with_economy` silently removes combat definitions loaded before it. This is documented and tested, and the game loads in the right order.
8. Health bars are M5's; the two-tick `MessageReader` window also applies to instant shots.
9. The smoke test does not exercise combat in the real binary; there is no automated screenshot.
10. M3 review 1 P2-1 (pending free units lost on sale without an event) is unchanged for selling; destruction now reports them.
11. Everything carried at the M3 review decision, unchanged.

**Carried from M3 review 1, unchanged by M4:**

* P2-1, narrowed as in item 10 above;
* P2-2: dock contention;
* P2-3: `docs/DECISIONS.md:360` still cites the nonexistent `two_placements_in_one_tick_see_each_other`;
* P2-4: `STARTING_CREDITS` in game code; M4 adds the opponent's starting gun turret to the same game-code match setup (D31 item 5);
* M3 implementation debt items 1 to 9.

**Carried from M2, M1, and M0, unchanged by M4:**

* M2:
  * review 3: P2-1 and P2-2;
  * remediation round 2: items 1 to 7;
  * review 1: P2-2 remainder.
* M1:
  * review 3: P2-1 (FIFO hang of the map and `.tsx` reader), P2-2, and the remaining items of P2-3;
  * review 1: P2-3 and P2-5;
  * review 2: P2-B (the `f32` parse in `map_subset.rs:524`);
  * round 1: item 4;
  * round 2: items 5 and 6.
* M0: items 1, 2, 3, 5, 7, 8, and 9. The linker warning and the `Skipped event Destroyed` warning were seen again in this review.

## Decision

Two P1 findings are unresolved:

* P1-1: target acquisition of undamageable enemies, so attack-move orders never end and units do not return fire;
* P1-2: `PlaceBuilding` rejections reveal unseen enemies.

Under PLAN.md section 10, any unresolved P1 makes NO-GO mandatory. All six validation commands passed. All required tests exist and pass.

Decision: NO-GO
