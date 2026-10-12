# RA2 parity plan

Written 11 October 2026 on the branch `ra2-parity` for the programme of
`docs/prompts/RA2_PARITY_PROMPT.md` ("the prompt"). For this programme this file prevails over
PLAN.md sections 7, 10 and 12 where they conflict (PLAN.md section 14); PLAN.md sections 5, 6, 8, 9
and 11 (equations, invariants, test categories, validation commands, failure conditions) stay in
force, with the amendments recorded here and in `docs/DECISIONS.md` (decisions R1 onwards).

The goal is a single player skirmish game whose rules, units, buildings, economy, combat,
specialist abilities, superweapons, vision, controls, player settings and computer opponents are
comparable to the original Red Alert 2 (without Yuri's Revenge). Network play is a non goal.

Contents: 1 status of the facts this plan rests on; 2 units and conversions; 3 architecture shared
by all milestones; 4 equations; 5 milestones RA1 to RA9; 6 validation and failure conditions;
7 reconciliation of the gap analysis (appendix A, B, C); 8 rule research (appendix D).

## 1. Confirmed facts and assumptions

Confirmed on 2026-10-11 by inspection or execution:

| ID | Fact | Evidence |
|---|---|---|
| RF1 | The working tree at `8230a95` held 81 modified and 79 untracked paths (the unified arsenal, HD art, controls, several enemies, game speed, 21 maps). | `git status --short` |
| RF2 | The reference `rules.ini` and `art.ini` were obtained from the source named in section 16 of the gap analysis; hashes are in DECISIONS R2. | `shasum -a 256` |
| RF3 | No `CARGO_TARGET_DIR` is configured; builds use `target/`. | DECISIONS R3 |
| RF4 | The simulation has one schedule of ten ordered stages (`SimSet::ORDER`): Commands, Ai, Economy, Pathfinding, Movement, Combat, Cleanup, Visibility, WinCondition, EndOfTick. | `crates/sim/src/schedule.rs` |
| RF5 | Data files are `units.ron` (schema 1), `economy.ron` (1), `combat.ron` (1), `arsenal.ron` (1), `skirmish.ron` (2) and `ai.ron` (1); a scenario may omit `arsenal.ron`, and then no item has a category, domain or role. | `assets/data/`, `crates/sim/src/arsenal.rs` |
| RF6 | Definition tests build their negative cases by editing the text of the fixture files (`variant(file, name, edits)`), so a schema change is migrated by changing the fixtures and adding cases for the new fields. | `crates/sim/tests/m6_definitions.rs:192` |
| RF7 | The end to end tests play every pairing of factions and difficulties on every shipped map (756 matches for two factions), as `m14_duels -- full` does. | `crates/game/tests/m6_production_matches.rs`, `crates/sim/examples/m14_duels.rs` |

Assumptions (each is repeated where it is used):

| ID | Assumption | Consequence if false |
|---|---|---|
| RA-A1 | The reference `rules.ini` is that of patch 1.006. It carries no version. | Values could differ from the last patch; the audit of RA9 still proves the game matches the recorded reference. |
| RA-A2 | Engine behaviour that the ini does not encode is as ModEnc and the C&C Wiki describe it (appendix D lists each rule with its source and a confidence label). | The rule is an approximation; it is listed in the rewritten gap analysis. |
| RA-A3 | A factory's "side" for the `Owner` rule can be taken from its building type (section 3.3). | Capturing a factory would give or withhold items differently from RA2 in corner cases. |

## 2. Units and conversions

All simulation arithmetic stays integer and deterministic (I2, I3, I12). Multipliers are stored as
permille (1000 = 1.0) or as exact rationals. A data value ported from the ini is stored in game
units, with a comment naming the ini section, key and original value.

| Quantity | RA2 unit | Game unit | Conversion |
|---|---|---|---|
| Time | frames, nominally 15 per second (the rules.ini comment on `IronCurtainDuration`) | ticks, 20 per second at 1x | `ticks = round(frames x 4 / 3)`, at least 1 |
| Game minutes (`BuildSpeed`, `RechargeTime`, `GrowthRate`, `CrateRegen`, repair rates) | minutes of 900 frames | ticks | `ticks = round(minutes x 1200)` |
| Range, `CellSpread`, `Adjacent`, radii | cells (decimals allowed) | subcells, 256 per cell | `subcells = round(cells x 256)` |
| Leptons (`ParadropRadius`, `PrismSupportHeight`) | 256 per cell | subcells | identical |
| Speed | integer `Speed` (the engine reads it as leptons per frame) | subcells per tick | `round(lpf x 3 / 4)`, the same time base as every other rate (section 4.1 justifies this instead of the GI anchor) |
| Build time | `BuildSpeed=.7` minutes per 1000 credits | ticks | `round(cost x 21 / 25)`, at least 1, before the factory and power factors (section 4.2) |
| Damage | `Damage` x `Verses` percent | hit points | `floor(base x verses / 100)` after the multipliers of section 4.3 |
| Percentages (`RefundPercent`, `ConditionYellow`, `Verses`) | percent | percent | identical |
| Ratios (`VeteranCombat=1.1`, `MultipleFactory=0.8`) | decimal | permille | `round(value x 1000)` |
| Credits, hit points, power, sight in cells | integer | integer | identical |

Rounding is half away from zero everywhere (`round(x) = floor(x + 1/2)` for the positive values
used), computed in integers: `round(a / b) = (2a + b) / (2b)` with floor division.

## 3. Architecture shared by all milestones

### 3.1 The data pipeline

```
~/Library/Caches/MyGame001/ra2-reference/{rules,art}.ini      (never committed)
        |  scripts/ra2_reference.py   (reads the ini through scripts/ra2_ini.py)
        v
crates/sim/tests/reference/ra2_reference.tsv                  (committed: section, key, value)
crates/sim/tests/reference/ra2_mapping.tsv                    (committed: game id -> ini section)
crates/sim/tests/reference/ra2_differences.tsv                (committed: intentional differences)
        |  scripts/generate_ra2_data.py   (reads only the committed tables)
        v
assets/data/{rules,units,economy,combat,arsenal,skirmish,ai}.ron
        |  crates/sim/tests/ra9_data_parity.rs   (reads the tables and the shipped data)
        v
"every shipped value equals conversion(reference value), or is a listed difference"
```

* `ra2_reference.tsv` holds only what the game uses: for every mapped section the numeric keys and
  identifier keys (weapon, warhead, prerequisite, owner lists), no comments, no display strings.
  It is the committed "reference table of values (numbers and ini keys only)" of the prompt.
* The generator applies the conversions of section 2 and writes each ported value with a trailing
  comment `// [Section] Key=value`. Values that are not ported (sprites, labels, descriptions,
  footprints, game specific roles) come from a table in the generator and carry no such comment.
* The audit (a Rust test, so that it runs in `cargo test`) recomputes every conversion itself from
  the TSV, independently of the Python generator, and fails on any value that differs and is not
  in `ra2_differences.tsv` (columns: game id, field, game value, reference value, reason). It is
  introduced in RA1 for the fields RA1 ports (cost, strength, sight, prerequisites, owner rules,
  speed) and extended by each milestone; RA9 completes and closes it.
* A milestone that changes a schema increments its `schema_version` and migrates every data file,
  fixture and generator in the same commit (PLAN.md section 10).

### 3.2 Core files and extension files

The files of M2 to M7 keep their meaning, so that fixture scenarios keep their expectations (I10):

| File | Role in this programme | Schema changes planned |
|---|---|---|
| `units.ron` | id, speed, sprite of every unit type | none (values change) |
| `economy.ron` | cost, build time, producer, prerequisites, power, footprint, sell value, free units, harvesters, refineries | 1 to 2 in RA5 (bails, gems, storage) |
| `combat.ron` | armour types, warheads, weapons, hit points, sight, weapons per type | 1 to 2 in RA3 (splash, primary, secondary and elite weapons, projectile properties) |
| `skirmish.ron` | default starting credits, countries (as factions), maps | 2 to 3 in RA1 (side, base unit and starting units; countries replace Vanguard and Meridian) |
| `ai.ron` | the computer player's plan and difficulties | 1 to 2 in RA1 (a plan per side), 2 to 3 in RA8 (full arsenal, difficulty multipliers) |
| `arsenal.ron` | **extension**: per item label, category, domain, role and every RA2 trait the core files lack | 1 to 2 in RA1, then one step per milestone that adds traits |
| `rules.ron` | **extension, new in RA1**: global values (`[General]`, `[CombatDamage]`, `[Radiation]`, `[CrateRules]`, `[Powerups]`, `[MultiplayerDialogSettings]`, the difficulty sections, prerequisite groups, sides and countries) | new at 1 in RA1; a later milestone that needs a key the file lacks increments it |

A world without the extension files (the fixture scenarios of M2 to M8) behaves exactly as before:
every new rule is conditional on a trait or a rules value being present, and the legacy constants
(`MAX_QUEUE_LENGTH`, `REPAIR_*`, the even tick low power rule, `placement_distance`) remain the
behaviour of such a world. This is what keeps hundreds of fixture tests valid without weakening
them, and it is the same arrangement M9 made for `arsenal.ron`.

The legacy armies leave the shipped data in RA1. The shipped data files as they were before RA1
are copied to `crates/sim/tests/fixtures/data/legacy/` (the "legacy ruleset" fixture); every test
that asserted something about the Vanguard and Meridian data loads that fixture instead and keeps
its assertions (fixed decision 2, I10).

### 3.3 Sides, countries and who may build what

RA2 decides buildability with four tests, and the game adopts all four:

1. **Prerequisites.** Every listed building type, or one member of a listed group
   (`PrerequisitePower`, `PrerequisiteFactory`, `PrerequisiteBarracks`, `PrerequisiteRadar`,
   `PrerequisiteTech`, `PrerequisiteProc`), is owned.
2. **Factory.** The player owns a factory of the item's kind (construction yard, barracks, war
   factory, shipyard, airforce command).
3. **Owner.** That factory belongs to a side in the item's `Owner` list.
4. **Country.** The player's own country is in `RequiredHouses` if that list is not empty, and is
   not in `ForbiddenHouses`.

Because tests 1 to 3 look at buildings and not at the player, capturing an enemy Construction Yard
or factory opens the other side's tree, as in RA2; test 4 keeps national specials national.

To express this the shared production buildings are split by side, as in rules.ini. The existing
ids stay the Allied types; Soviet types are new and reuse the existing sprite until they have art
of their own (`art` in `arsenal.ron` names the miniature):

| RA2 | Game id | RA2 | Game id |
|---|---|---|---|
| GACNST | `construction_yard` | NACNST | `soviet_construction_yard` |
| GAPILE | `barracks` | NAHAND | `soviet_barracks` |
| GAREFN | `refinery` | NAREFN | `soviet_refinery` |
| GAWEAP | `war_factory` | NAWEAP | `soviet_war_factory` |
| GAYARD | `naval_shipyard` | NAYARD | `soviet_naval_shipyard` |
| GADEPT | `service_depot` | NADEPT | `soviet_service_depot` |
| GATECH | `tech_center` | NATECH | `soviet_battle_lab` |
| GAWALL | `fortress_walls` | NAWALL | `soviet_walls` |
| GAAIRC | `airforce_command` | AMRADR | `american_airforce_command` |
| AMCV | `mcv` | SMCV | `soviet_mcv` |
| LCRF | `amphibious_transport` | SAPC | `soviet_amphibious_transport` |

`ENGINEER`/`SENGINEER` and `ADOG`/`DOG` differ only in `ForbiddenHouses`, so the game keeps one
`engineer` and one `attack_dog` that any barracks trains (a listed difference).

A **factory group** (`arsenal.ron`, trait `factory`) names the kind of factory a building is:
`buildings`, `infantry`, `vehicles`, `naval`, `aircraft`. `economy.ron` keeps one `producer` per
item, which now means "a building of the producer's factory group"; a building without the trait
is its own group, which is the M3 rule. Queues are per factory group and lane (section 5, RA2).

`A factory's side is the side of its building type` (assumption RA-A3): an Allied Barracks trains
Allied infantry whoever built or captured it.

### 3.4 State, hashing and events

Every new piece of simulation state is hashed in `state_hash` in a fixed order (I3) and is written
only by systems of `SimSchedule` in response to commands, time or other state (I5). New state per
milestone is listed in section 5. New `Command` variants are validated for ownership, existence,
bounds and visibility and rejected with a `RejectReason` and no state change (PLAN.md section 5).
New `SimEvent` variants carry what the interface needs to draw and announce them; an event about
something a player does not see is not shown to that player by the game (D33).

Randomness (scatter, crates, lightning, starting unit choice, random country) comes only from
`SimRng`. Iteration that affects state is over `BTreeMap`/`BTreeSet` or entities sorted by `SimId`
(I4).

### 3.5 Match options

`MatchOptions` (`crates/sim/src/match_setup.rs`) is the single description of a match's rules and
is extended in RA1. `MatchOptions::default()` is the RA2 default; extensions default off.

| Option | Values | Default | Since | RA2 source |
|---|---|---|---|---|
| country per player | one of nine, or random | random for computer players, America for the human | RA1 | `[Countries]` |
| starting credits | 5000 to 10000 in steps of 100 | 10000 | RA1 | `MinMoney`, `Money`, `MaxMoney`, `MoneyIncrement` |
| unit count | 0 to 10 | 10 | RA1 | `MinUnitCount`, `UnitCount`, `MaxUnitCount` |
| start with an MCV | on, off (off: a deployed Construction Yard) | on | RA1 | `Bases=yes`, `BaseUnit` |
| short game | on, off | on | RA1 | `ShortGame=yes` |
| superweapons | on, off | on | RA1 | lobby checkbox |
| crates | on, off | on (functional from RA5) | RA1 | `Crates=yes` |
| MCV repacks | on, off | on | RA1 | `MCVRedeploys=yes` |
| bridge destruction | on, off | on | RA5 | `BridgeDestruction=yes` |
| difficulty per computer player | Easy, Medium, Brutal | Medium | RA1 | lobby |
| team mode | computer players allied against you, free for all | allied | RA1 | fixed decision 3 |
| fog of war | off (shroud only), on | on until RA7, then off | RA7 | `FogOfWar=no` |
| map growth with players | off, on | off | RA1 | extension (R4) |
| Super Me, unit speed, ALL UNLOCKED | as before | off | kept | extension |

## 4. Equations

Sources are abbreviated as in appendix D (`INI` the cached rules.ini, `ME/x` ModEnc page x, `RA1`
EA's released source of the ancestor engine, `PHOBOS`/`YRPP`/`ARES` the engine research projects).
Labels: **confirmed** (the ini, or two independent sources), **single source**, **assumption**.
`pm` means permille; `div` is floor division; every product is computed in `i64`.

### 4.1 Speed (amends the anchor of the prompt's section 3)

```
lpf       = min(Speed x 256 div 100, 255)                 leptons per frame        confirmed (ME/Speed, RA1)
lpf       = JumpjetSpeed                                  for jumpjet locomotors   assumption (unit undocumented)
lpf       = lpf x 3 div 2                                 for infantry on foot     single source (ME/Speed)
speed     = max(1, round(lpf x 3 / 4))                    subcells per tick
```

The prompt proposed `round(Speed x 14 / 4)`, anchored on the GI's current 14 subcells per tick, and
asked the plan to confirm that anchor or justify another. This plan justifies another: the
engine defines `Speed` as leptons per frame, so the same time base that converts rate of fire,
build time and recharge (15 frames to 20 ticks) converts speed, and every ratio between movement
and time in RA2 is kept. Under the proposed anchor a Grizzly would cover 1.95 cells a second
against a reload of 4 seconds, where RA2 at its nominal rate has 1.0 cell a second; under this
conversion the GI moves 11 subcells per tick (0.86 cells a second) instead of 14, the Grizzly 13,
the Harrier 26. The game therefore plays at 1x as RA2 does at 15 frames per second; the game
speed setting (2x equals RA2 at 30 frames per second) is unchanged. `GameSpeedBias` is ignored
(ME calls it obsolete; assumption). Terrain speed factors of `[Tiberium]`, `[Road]`, `[Ice]` and
slopes are not modelled (listed difference). Vehicles and ships at or below `ConditionYellow` move
at three quarters speed (confirmed, ME/Speed and RA1; implemented in RA3 with the damage states).

### 4.2 Production and power (RA2)

```
build_time = max(1, round(Cost x BuildSpeed x 1200 / 1000)) = round(Cost x 21 / 25)   ticks, in economy.ron
build_time = build_time x WallBuildSpeedCoefficient (3)      for Wall=yes               confirmed (INI comment, ME)
factories  = min(owned buildings of the item's factory group, 10)
rate_pm    = 1000 x 5^(factories-1) div 4^(factories-1)      MultipleFactory=0.8 per extra factory
ratio_pm   = 1000 if drain = 0, else min(1000, output x 1000 div drain)
power_pm   = 1000 if output >= drain, else
             clamp(1000 - LowPowerPenaltyModifier_pm x (1000 - ratio_pm) div 1000, 500, 800)
rate_pm    = rate_pm x power_pm div 1000   [x 10 with Super Me]   [x 1000 div BuildTime_pm for an AI, RA8]
each tick:  fraction += rate_pm;  units = fraction div 1000;  fraction = fraction mod 1000
            each unit of progress is charged by PLAN.md equation 3 (unchanged), and a unit the
            player cannot pay stops the tick's progress (the fraction is kept).
```

Confirmed: `BuildSpeed=.7`, `MultipleFactory=0.8`, the wall coefficient, the low power keys and
the clamp structure. Assumptions: shortage is `1 - output/drain` (RA1); no cap on factories in
RA2, the game counts at most 10 so that the rate is bounded. RA2 charges in 54 steps; the game
keeps its per progress unit charge, with the same total (listed difference).

Slots (confirmed, YRPP and ME/BuildCat): buildings, defences (`BuildCat=Combat`), infantry,
vehicles, ships, aircraft, all in parallel. A unit slot holds one item in production and
`MaximumQueuedObjects=29` queued; the two building slots hold one item each, because RA2 does not
queue buildings (single source, ME/MaximumQueuedObjects).

A building stops working under low power when it has `Powered=yes` and drains power (single
source, ME/Powered): Sentry Gun (`Power=0`) and Pillbox (not `Powered`) keep firing. A superweapon
with `IsPowered=true` stops charging (confirmed). Radar needs an owned `Radar=yes` building
(Airforce Command, American Airforce Command, Radar Tower, Spy Satellite) and no low power
(single source, ME/Radar).

Placement: a building must lie within its `Adjacent` cells (2 for most, 4 for the cheap defences,
8 for walls, 12 for shipyards; confirmed, INI) of an owned building that has `BaseNormal` unset or
yes (ME/Adjacent, ME/BaseNormal; single source).

### 4.3 Damage (RA3)

```
D0 = Damage x firepower_pm                               veteran FIREPOWER 1100, crate 2000, multiplied
R  = round(CellSpread x 256)                             subcells
d  = distance in subcells from the impact point to the object (an infantry's spot, a vehicle's
     position, each footprint cell centre of a building separately)
R = 0: only what stands in the impact cell is hit, with falloff 100
d > R: no damage
falloff_pc = 100 - (100 - PercentAtMax) x d / R          linear, PercentAtMax at d = R
D2 = D0 x (100 R - (100 - PercentAtMax) d) x Verses  div  (1000 x 100 R x 100)
D2 = max(D2, 1) when Verses > 0                           PLAN.md equation 2 (ME/MinDamage agrees)
D3 = D2 x 1000 div armour_pm                             veteran STRONGER 1500, crate 1500, AI Armor (RA8)
D  = min(D3, MaxDamage = 10000)
```

Confirmed: the `Verses` order, linear falloff, no damage beyond the spread, `MaxDamage`, division
by the armour multiplier (PHOBOS, ME). Single source: a building takes one hit per footprint cell
inside the radius, which is what lets a nuclear blast destroy a power plant and not a
Construction Yard. Not modelled: prone infantry (`ProneDamage`), height, walls stopping shots.
`Verses=0` makes a target illegal for that weapon; `Verses=1` means never acquired automatically
but attackable on order (single source, ME/Verses).

### 4.4 Weapons (RA3)

* Reload ticks `= max(1, round(ROF x 4 / 3))`, times the rate of fire multipliers in permille
  (veteran ROF 600; AI ROF, RA8). The engine's random 0 to 2 frames of jitter is not modelled.
* Range `= round(Range x 256)` subcells between positions, to the nearest cell of a building
  (the M4 rule; RA2 measures arcing shots to the centre: listed difference). A weapon does not
  fire inside `MinimumRange`. `AirRangeBonus` (IFV, 4 cells) is added against aircraft.
* `Burst` n fires n shots 5 ticks apart (RA2: 3 to 5 frames at random; assumption), then reloads.
* Selection: of primary and secondary, the first whose projectile may engage the target's layer
  (`AA` for aircraft, `AG` for ground and sea) and whose warhead has `Verses > 0` against the
  target's armour; elite weapons replace both at elite rank; deployed infantry use the secondary
  (`DeployFire`).
* Scatter: only the two flak projectiles are `Inaccurate` in vanilla (confirmed, INI). Their
  impact point lies at a random direction (64 fixed integer directions) and a distance drawn
  uniformly from 0 to `2 x BallisticScatter x distance / Range` for `[FlakProj]`, 0 to
  `BallisticScatter x distance / Range` for `[FlakTProj]` (single source, ME/FlakScatter), from
  `SimRng`; damage then comes from the warhead's spread alone.

### 4.5 Veterancy (RA4)

```
experience += victim.Cost                                 enemy and neutral victims only
rank = 1 (veteran) when experience x 1000 >= own.Cost x VeteranRatio_pm (3000)
rank = 2 (elite)   when experience x 1000 >= 2 x own.Cost x VeteranRatio_pm        VeteranCap=2
```

Confirmed (YRPP `VeterancyStruct::Add`, ME/VeteranRatio); the threshold is inclusive (YRPP; ME says
exclusive). A rank grants the tokens of the type's `VeteranAbilities`, and elite adds those of
`EliteAbilities`; a token applies once (confirmed): `FIREPOWER` damage x 1100 pm, `STRONGER`
incoming damage x 1000 / 1500, `ROF` reload x 600 pm, `FASTER` speed x 1200 pm, `SIGHT` nothing
(`VeteranSight=0.0`; assumption), `SELF_HEAL` +1 hit point every 19 ticks (`RepairRate`, 14
frames; single source) up to full health, `SCATTER` not modelled. Kills by a spawned missile or
Hornet count for the launcher. Types with `Trainable=no` and buildings gain nothing.

### 4.6 Sell, repair, reload (RA2, RA4)

```
refund      = Cost x RefundPercent / 100                  (50%)                    confirmed
repair step = every 19 ticks (RepairRate .016 min = 14 frames): + RepairStep (8) hit points for
              max(1, (Cost div max(Strength div 8, 1)) x RepairPercent div 100) credits   confirmed (PHOBOS, ME)
depot       = the same step and price from the vehicle's Cost and Strength, every 19 ticks
reload      = one round per 360 ticks (ReloadRate .3 min = 270 frames) on a dock        confirmed
```

A step the player cannot pay is skipped (I7). Walls cannot be repaired or sold.

### 4.7 Crews and survivors (RA4)

Vehicles: only `Crewed=yes` types (in vanilla the two MCVs) release, with `CrewEscape` 50%
probability, one infantry of the owner's side (`AlliedCrew=E1`, `SovietCrew=E2`), and not while
moving (single source). Buildings with `Crewed=yes`, sold or destroyed, release
`n = max(1, ceil(refund / divisor))` crew, divisor 500 for an Allied and 250 for a Soviet owner,
at most as many as free spots on the footprint (assumption: the ini comment says survivors are
"sell cost / divisor" and that `SurvivorRate` is unused, ModEnc gives `Cost x SurvivorRate /
divisor`); a destroyed or sold Construction Yard adds an Engineer with 25% probability
(confirmed, ME and RA1). Technicians are not modelled.

### 4.8 Aircraft, spawns, submarines (RA4)

* Harrier and Black Eagle: `Ammo=1`, four docks per Airforce Command, one volley per sortie,
  return when empty, reload by 4.6; an aircraft with no dock left is destroyed (`AirportBound`).
* Carrier: 3 Hornets (`[HORNET]` strength 75), 27 ticks between launches, 200 ticks to rearm a
  returned one (`SpawnReloadRate=150`), 800 ticks to replace a lost one (`SpawnRegenRate=600`);
  Hornets die with the carrier.
* V3: one `[V3ROCKET]` (strength 50, armour special_2, speed 15) per launch, 533 ticks between
  launches (`SpawnRegenRate=400`), `V3RocketDamage=200` through `V3WH` at impact. Dreadnought: two
  `[DMISL]` (speed 20), `DMislDamage=300` through `DMISLWH`. A missile shot down deals nothing.
  The pitch and turn keys are flight animation and are not simulated: a missile flies straight
  at its cruise speed.
* Submerged units (`SUB`, `DLPH`, `SQD`): hidden from a player unless one of that player's units
  with `SensorsSight` s is within s cells (4 for the three themselves, 8 for the Destroyer) or
  the unit fired within the last 24 ticks (`CloakDelay=.02` min = 18 frames).

### 4.9 Starting units (RA1)

```
eligible = types without AllowedToStartInMultiplayer=no: E1 200, E2 100, MTNK 700, HTNK 900
avg      = 1900 div 4 = 475;   budget = (UnitCount - 1) x avg   (0 when UnitCount = 0)
while spent < budget: a vehicle of the side while spent < budget x 2 div 3, else its infantry
```

Single source (the Tiberian Sun reimplementation and the Ares documentation agree on the budget).
Each side has one eligible vehicle and one infantry, so the result is fixed: unit count 10 gives
5 Grizzlies and 4 GIs, or 4 Rhinos and 7 Conscripts. They stand on the free cells nearest to the
base unit, at least 3 cells away.

### 4.10 Ore (RA5)

* Ore 25 and gems 50 credits a bail; a cell holds 1 to 12 bails (12 density stages; assumption
  from RA1 and the overlay count). A map's ore amount a becomes `clamp(round(a / 25), 1, 12)`.
* Miners take one bail per 19 ticks (assumption: nothing documents the time) up to `Storage`
  (40, 20) and are paid on unloading, at once (the M3 rule; RA2's unloading time is undocumented).
* Growth: every 2933 ticks (`[Riparius] Growth=2200` frames) each ore cell below 12 bails gains
  one with probability 6% (`GrowthPercentage=.06`). Spread: on the same period each ore cell with
  more than 6 bails (assumption, RA1) puts one bail on one random free neighbouring land cell with
  probability 6%. Gems neither grow nor spread (confirmed, INI). `[General] GrowthRate` is unused
  by the engine (single source, ME/GrowthRate), so the per ore keys are the ones ported.
* Each powered Ore Purifier adds 25% to every unload.

### 4.11 Crates (RA5)

At most `CrateMaximum`, kept at `CrateMinimum` (1); a taken crate is replaced after a random 1800
to 7200 ticks (`CrateRegen=3` minutes, half to double; single source). Shares of 110: money 20
(2000 plus a random 0 to 900), unit 20 (a random `CrateGoodie` vehicle, land only), veteran 20 (one
rank to units within `CrateRadius` 3 cells), heal base 10, armour 10 (x 1500 pm), firepower 10
(x 2000 pm), speed 10 (x 1200 pm), reveal 10; a unit that already has the multiplier, or is
already elite, gets money instead. `FreeMCV`: a collector with no building, no base unit and more
than 1500 credits gets its side's MCV.

### 4.12 Tech buildings, garrisons, bridges (RA5)

* Oil Derrick: 1000 credits on capture from the neutral player, then 20 every 133 ticks. Tech
  Outpost: `HoverMissile`, a one dock repair bay, strength 2000. Airport: the paradrop of 4.13.
  Hospital: own infantry ordered into it are healed (20 hit points a tick, `IRepairStep`) and
  leave. All need an Engineer and cannot be sold (confirmed, INI).
* Garrison: only `Occupier` infantry (GI, Conscript) and only neutral buildings; up to the
  building's `MaxNumberOccupants`; the building fires `AlliedOccupyW` (30, 20 ticks, 7 cells) or
  `SovietOccupyW` (20, 27 ticks, 7 cells) with the reload divided by the number of occupants; at
  `ConditionRed` the occupants are put out and entry is refused (confirmed, INI and ME).
* Bridges: strength `BridgeStrength=1500` for a whole bridge; the hut is indestructible; an
  Engineer entering it restores the bridge and is consumed.

### 4.13 Specialists and superweapons (RA6)

Every value below is from the ini (confirmed) unless marked.

| Item | Rule in game units |
|---|---|
| GI | deployed: cannot move, fires `Para` (15, 20 ticks, 5 cells; elite `ParaE`); stays deployed until ordered |
| C4 | Tanya (and Psi Commando) entering an enemy building destroys it after 36 ticks (`C4Delay`); `Sapper` 2500 against ships |
| Ivan bomb | 400 through `IvanWH` (spread 1.5 cells, 25% at the edge) after 600 ticks; an Engineer removes it |
| Spy | takes the look of a targeted infantry; default look by side; effects per building as RA6's table; one effect per entry |
| Mind control | Yuri range 7, one unit, the first is released when a second is taken, ends with the controller's death; buildings, miners, drones, aircraft and Yuri are immune |
| Psi wave | deploy: 250 through `PsiPulse` (3 cells, 85% at the edge, infantry only); Yuri undeploys after 200 ticks |
| Chrono erase | `remaining = 10 x Strength`; each tick subtract `3/4` of the summed weapon damage (8, elite 16) of the legionnaires firing; at 0 the target is removed; while fired on it is frozen and invulnerable; if firing stops it returns unharmed |
| Chrono move | after arriving, no move or fire for `max(distance in subcells div 48, 16) x 4 / 3` ticks |
| Tesla Trooper | adjacent to an own Tesla Coil: 2 keep it firing under low power; 3, or 1 with full power, make it fire `OPCoilBolt` (300, 133 ticks, 8 cells) |
| Prism Tower | damage `x (1 + 1.5 N)` for N supporters within 8 cells of the firing tower in a chain, N at most 8; a supporter cannot fire for 80 ticks |
| Mirage | looks like a tree to players without a disguise detector in range while it stands still; not acquired automatically by them |
| Terror Drone | kills infantry at once; inside a vehicle deals 50 through `Parasite` every 80 ticks; leaves when the host dies by it, dies with the host otherwise; removed by a Service Depot |
| Giant Squid | 15 (elite 40) every 132 ticks; the ship cannot move; destroyed outright at `ConditionRed`; a Dolphin's hit frees it |
| Desolator | `RadBeamWeapon` 125, 67 ticks, 6 cells; deployed `RadEruptionWeapon` every 80 ticks while the cell's level is below a third of 500 |
| Radiation | level at distance d of a site of level L and spread S cells: `L x (S x 256 - d) div (S x 256)`, summed up to `RadLevelMax=500`; lasts `L` frames, falling in steps every 120 ticks; every 21 ticks a unit in a cell takes `level x 0.2` through `RadSite` (no damage to buildings); immune types by `ImmuneToRadiation` |
| IFV | weapon by passenger `IFVMode` from the table of appendix D |
| Reactor, truck, terrorist | `NukePayload` (600, `NUKE`, level 500), `Demobomb` (300, spread 8, level 100), `TerrorBomb` (225, spread 2) |
| Lightning Storm | 333 ticks after firing; for 240 ticks one bolt on the target every 13 ticks and one on a random cell of the 10 x 10 cell square (assumption: square) every 7 ticks, 3 cells apart from active bolts; each 250 through `IonWH` (spread 2); enemy radar is off meanwhile |
| Nuclear Missile | impact 200 ticks after launch (assumption: the flight time is undocumented), `NukePayload` |
| Iron Curtain | 3 x 3 cells (single source); vehicles and buildings of any owner invulnerable for 1000 ticks; infantry there die (assumption) |
| Chronosphere | 3 x 3 cells; vehicles of every owner move by the same offsets and are frozen 80 ticks; infantry die; a land vehicle on water or a ship on land is destroyed (assumption) |
| Paradrop | America 8 GIs, Airport 6 GIs or 9 Conscripts, every 4800 ticks, from a plane (strength 400) that anti air can shoot down; troops jump within 4 cells of the target |
| Recharge | nuke and storm 12000 ticks, Iron Curtain 6000, Chronosphere 8400 |
| Warnings | construction of a superweapon is announced to every player, readiness to the owner, timers are public (single source, ARES) |

### 4.14 Vision (RA7)

Without fog of war every explored cell counts as visible, as in RA2, where revealed ground shows
what happens on it (confirmed: `FogOfWar=no`). A powered Gap Generator makes the cells within 10
cells unexplored again every tick for players hostile to its owner, even under a Spy Satellite; a
unit's own sight still reveals around it (single source). The Psychic Sensor detects disguise and
shows enemy orders whose target lies within 15 cells. The Spy Satellite explores the whole map
except gap areas and makes a radar infiltration pointless.

### 4.15 Computer player difficulty (RA8)

ModEnc (single source): the difficulty sections are named from the human's point of view, so a
Brutal computer player uses `[Easy]` and an Easy one `[Difficult]`; only `ROF` and `BuildTime`
act. Confirmed from the ini: the values below.

| | Easy AI | Medium AI | Brutal AI |
|---|---|---|---|
| reload multiplier (`ROF`) | 1200 pm | 1000 | 800 |
| build time multiplier (`BuildTime`) | 1000 | 1000 | 800 |
| starting credits (`MultiplayerAICM`) | x 1 | x 1 | x 5 |
| ore income (`AIVirtualPurifiers`) | + 0% | + 50% | + 100% |
| ticks between attack waves (`TeamDelays`) | 4667 | 3333 | 2667 |
| base defences aimed for (`AlliedBaseDefenseCounts`, `SovietBaseDefenseCounts`) | 6 | 20 / 22 | 25 |

The ini has no `Firepower` key in these sections, so RA2 does not scale firepower by difficulty
and neither does the game. RA2's computer player ignores `Owner`, prerequisites, shroud, cloaking
and gap generators (confirmed, ME/AI); the game's keeps honest vision and the normal tech tree
(listed difference, D43).

## 5. Milestones

Every milestone is carried out as the prompt's section 6 says: inspect and verify first, make the
smallest coherent change, write tests in every category of PLAN.md section 8 through the
production path (`CommandQueue`, `SimSchedule`, shipped data for shipped behaviour, fixtures for
balance independent behaviour), run every validation command, commit, and append an entry to
`docs/STATUS.md`. Common to all milestones, and not repeated below:

* **Validation commands:** the eleven of section 6.1, through `scripts/validate_ra2_parity.sh`.
* **Invariants:** I1 to I5 and I7 to I12 hold unchanged; a milestone lists only what it touches.
* **Failure conditions:** those of section 6.2.
* **Computer players:** every milestone leaves all AI matches of the test suite, `m7_soak` and
  `m14_duels -- full` ending with a winner. A milestone that changes balance retunes `ai.ron`
  (data) or fixes the computer player (code, with a regression test); a tick limit of a test is
  never raised to hide a stall.
* **Tests:** new test files are named `ra<N>_<topic>.rs` in `crates/sim/tests/` or
  `crates/game/tests/`; existing file names stay. A test whose expectation changes because RA2
  behaviour is intended is updated with a note in `docs/DECISIONS.md`.

### RA1. Sides, countries and match setup

**Problem and root cause.** The human owns one unified tree of 72 RA2 items plus 12 legacy items,
computer players field the Vanguard and Meridian armies, and a match always starts with 5000
credits, a deployed Construction Yard and a fixed unit group; a player is defeated when it owns no
building (gap analysis 2.1, 2.2, 2.9, 2.10, section 3 and 4). Root cause: M9 added the RA2 items to
one roster for the human (`match_setup.rs`, the unified roster) on top of the M7 two faction
design, and `MatchOptions` has only the extensions.

**Change.**

1. *Pipeline* (section 3.1): `scripts/ra2_reference.py`, the three tables under
   `crates/sim/tests/reference/`, `scripts/generate_ra2_data.py` (which replaces
   `scripts/generate_arsenal.py`), and `crates/sim/tests/ra9_data_parity.rs` for the RA1 fields.
2. *`rules.ron`, schema 1* (`crates/sim/src/rules.rs`, new; `Definitions::with_rules`): every
   global value this plan uses, in game units (section 2), grouped as `production`, `economy`,
   `veterancy`, `crew`, `combat`, `radiation`, `superweapons`, `prism`, `crates`, `powerups`,
   `dialog`, `difficulty`, `prerequisite_groups`, `sides`, `countries`. Checked like every data
   file (subset of RON, every field required, ranges, cross references to buildings and units).
3. *Ported values:* cost, strength, sight, speed, prerequisites, power and sell value
   (`cost x RefundPercent`) of every item; the 11 new ids of section 3.3; the four units a spy
   unlocks (`chrono_commando`, `psi_commando`, `chrono_ivan`, `yuri_prime`, not producible until
   RA6); Kirov and NightHawk move to the War Factory as in RA2. Build times, weapons, armour and
   footprints stay until RA2 and RA3 (footprints stay for good, section 5 RA9).
4. *`arsenal.ron`, schema 2:* adds `art`, `side` (`allied`, `soviet`, `none`), `factory`,
   `owner_sides`, `required_countries`, `forbidden_countries`, `tech_level`.
5. *Buildability* (section 3.3) in `economy_commands.rs`: new reject reasons `NoFactoryOfSide`,
   `CountryRequired`, `CountryForbidden`; prerequisite groups; `PlayerState` gains `country`.
6. *`skirmish.ron`, schema 3:* the nine countries as factions with `side` and `label`; per side
   the base unit and the starting unit lists; `starting_credits: 10000`. Fixtures gain
   `side: "none"`.
7. *`ai.ron`, schema 2:* one plan per side (build order, power building, miner, army) from ground
   units and base defences of that side; difficulties `easy`, `medium`, `brutal`. The computer
   player deploys its MCV first (`Command::Deploy`) and otherwise plays as before. RA8 completes it.
8. *Match options* (section 3.5): `MatchOptions` gains `starting_credits`, `unit_count`,
   `mcv_start`, `short_game`, `superweapons`, `crates`, `mcv_repacks`, `free_for_all` (replaces the
   meaning of `allied_ai: false`), `grow_map`; random countries are drawn from `SimRng` in
   `setup_match_configured`. Superweapon buildings cannot be queued when `superweapons` is off
   (`RejectReason::DisabledByOptions`).
9. *Start of match:* an MCV at the start position (so that deploying in place puts the yard where
   `map_scale` expects it) and `unit_count` units around it, chosen by section 4.9.
10. *Win condition* (`check_win_condition`): short game, a player is defeated when it owns no
    building and no base unit (`sides[..].base_unit`); long game, when it owns no building and no
    unit. A world without rules keeps "no building" (D41).
11. *MCV repacks:* `Command::Undeploy { building }` turns a Construction Yard into its side's MCV
    at the yard's cell when the option is on; queues of the yard's factory group pause as for any
    lost producer.
12. *Game:* menu rows for country (human and each computer player), difficulty per computer
    player, credits, unit count and the on/off options; the PLAYERS panel shows country and side;
    the sidebar lists only items some owned factory could build (RA2 shows no cameo otherwise);
    `check_catalogue`, `m6_production_matches`, `m14_duels`, `m7_soak` and `m13_ten_players` play
    side pairings with rotating countries instead of faction pairings (R5), so that the number of
    matches stays 756 and every country is played.
13. *Legacy ruleset fixture* (section 3.2) and migration of the tests that used shipped legacy
    data.

**Files.** `scripts/ra2_reference.py`, `scripts/generate_ra2_data.py`, `assets/data/*.ron`,
`crates/sim/src/{rules.rs (new), arsenal.rs, definitions.rs, skirmish_defs.rs, ai_defs.rs, ai.rs,
match_setup.rs, economy.rs, economy_commands.rs, command.rs, special.rs, event.rs, hash.rs,
lib.rs}`, `crates/game/src/{skirmish.rs, sim_host.rs, sidebar.rs, construction.rs, smoke.rs,
hd_art.rs, units.rs, buildings.rs, notices.rs, visual_review.rs}`, the sim examples, tests.

**Production paths.** `Commands` (`process_commands` to `economy_commands::apply` and
`special::apply`), `Ai` (`run_ai`), `WinCondition` (`check_win_condition`); setup before the first
tick (`setup_match_configured`). Game: `skirmish::match_plan`, `sim_host`, the sidebar.

**Equations.** Sections 2 (speed, credits), 4.1 and 4.9.

**Invariants touched.** I3 (country, options and the base unit rule are hashed), I5 (the computer
player's deploy is a command), I8 (rules file errors are typed), I10 (legacy fixture).

**Compatibility and non goals.** Old `skirmish.ron` and `ai.ron` files are rejected by version.
`settings.txt` is unchanged (options are per match, as before). Not in RA1: build times, queues,
weapons, new mechanics of the four unlockable units, crates (the option exists, RA5 implements).

**Tests.** Positive: every country sets up on every map with an MCV and 0, 1 and 10 starting
units; an Allied player builds the Allied tree to the Battle Lab and a Soviet the Soviet tree;
Korea builds the Black Eagle and not the Harrier, America the American Airforce Command; a
captured Soviet Barracks trains Conscripts for an Allied player; undeploy and redeploy. Negative:
each new reject reason with no state change; a Soviet item without a Soviet factory; a national
special for another country, also after capture; superweapons off; undeploy with the option off;
malformed `rules.ron` (every field required, unknown field, each range, each dangling reference).
Boundary: credits 5000 and 10000 and one step outside; unit count 0 and 10 and 11; a prerequisite
group satisfied by its last member only. Regression: the migrated legacy tests. Integration: short
game (a player with only an MCV lives, with neither is defeated) and long game. End to end: the
756 side pairing matches end with a winner; the data parity audit; both smoke tests with the new
menu.

**Acceptance.** Costs, strength, sight and prerequisites of every item equal the reference or are
listed differences (audit green); the default match is RA2's (10000 credits, MCV, 10 units, short
game, superweapons, MCV repacks); no legacy item can be built in a default match.

### RA2. Production, power and base building

**Problem and root cause.** Build time is a fixed number per item, extra factories do nothing,
Buildings and Armory share one queue, there are no build limits, low power halves production and
silences every armed building, the minimap is always on, and placement uses one distance for all
buildings (gap analysis 2.8, 2.12, 2.13, section 5). Root cause: `advance_production` advances one
progress unit per tick per producer type and only knows "low power" as a boolean
(`PlayerState::low_power`); `economy.ron` has one `placement_distance`.

**Change.**

1. *Build time data:* `build_time = round(cost x 21 / 25)` for every item, times 3 for walls
   (section 4.2).
2. *Rate based progress* (`economy.rs`): a queue's head advances by a rate in permille per tick,
   accumulated in a new `QueueItem::fraction` (hashed); each whole unit is charged by the existing
   `charge_for_tick`, so equation 3 and I7 hold unchanged. The rate is the product of the multiple
   factory factor, the low power factor and Super Me (section 4.2). A world without rules keeps
   one unit per tick and the even tick rule.
3. *Queues* keyed by factory group and lane: `buildings` and `defences` (the armoury) for the
   Construction Yard, `infantry`, `vehicles`, `naval`, `aircraft`. A unit lane holds one item in
   production and `rules.production.max_queued` (29) behind it; a building lane holds one item,
   as RA2 does not queue buildings (the computer player already queues one at a time).
4. *`arsenal.ron`, schema 3:* `build_limit`, `adjacent`, `base_normal`, `radar`, `sellable`,
   `powered`. `QueueProduction` is rejected with `BuildLimitReached` when owned plus queued
   reaches the limit.
5. *Power:* RA2 values; under low power a building stops only if it is `powered` and consumes
   power (defences, support effects), superweapons with `is_powered` stop charging and radar is
   lost; section 4.2 gives the production factor.
6. *Radar:* `PlayerState::radar` is true while the player owns a `radar` building, is not low on
   power and has no spy outage; the game shows the minimap only then (`minimap.rs`), with a
   "radar offline" panel otherwise.
7. *Placement:* a new building must lie within its own `adjacent` cells (2, 4, 8 or 12) of an
   owned building with `base_normal`. If a shipped map has no shipyard water within 12 cells of a
   start, the shipyard keeps the reach of 32 cells as a listed difference (the catalogue check
   and a map test decide; the maps are not reshaped for it).
8. *Sell and repair:* refund from `RefundPercent`; repair in steps of `RepairStep` hit points
   every `RepairRate`, a full repair costing `RepairPercent` of the price (section 4.6); walls
   are not sellable; buildings captured from the neutral player (RA5) are not sellable.
9. *Service Depot:* a damaged vehicle ordered onto the depot (`Command::Interact`) docks, is
   repaired in steps for credits and leaves; the free aura of M9 is removed. The computer player
   does not use it until RA8.
10. *Game:* armoury progress beside building progress in the sidebar; build limit and radar
    states; README.

**Files.** `assets/data/{economy,arsenal,rules}.ron`, the generator and audit,
`crates/sim/src/{economy.rs, economy_commands.rs, building.rs, combat.rs, special.rs, arsenal.rs,
hash.rs, ai.rs}`, `crates/game/src/{sidebar.rs, construction.rs, minimap.rs, smoke.rs}`.

**Production paths.** `Economy` (`update_power`, `advance_production`, `repair_buildings`,
`special::advance`), `Commands` (queue, place, sell, repair, interact), `Combat` (low power fire).

**Equations.** Sections 4.2 and 4.6.

**Invariants touched.** I3 (`fraction`, `radar`, dock state hashed), I7 (exact charging with
rates; repair and depot never overdraw).

**Schema.** `arsenal.ron` 2 to 3; `economy.ron` values only.

**Compatibility and non goals.** Fixture worlds keep the M3 rules. Not here: veterancy effects on
build time, the AI's difficulty build time factor (RA8).

**Tests.** Positive: build time of every shipped item equals the formula (audit); two and three
factories; a building and a defence built at once; graded low power at three deficits; a Pillbox
fires and a Prism Tower does not under low power; radar on, off by power, back on; depot repair
charges the exact cost. Negative: build limit, wall sale, placement beyond `adjacent`, a depot
order for an undamaged or foreign unit. Boundary: the factory factor at its cap; power exactly
equal to drain; queue at its last slot; a repair the player can pay to the last credit and not
one more. Regression: I7 with every rate (property test over costs and rates); the M3 fixture
tests unchanged. Integration: a base built from an MCV in RA2 order within the tick budget the
formula gives. End to end: AI matches, smoke.

**Acceptance.** Every row of gap analysis section 5 except ore, miners and crew is closed or a
listed difference; I7 holds in every test; the minimap follows radar.

### RA3. Armour, warheads and weapons

**Problem and root cause.** Seven armour classes and thirteen warheads of the game's own design,
one weapon per type, splash only as a role, area damage that ignores armour, every shot hits, no
crushing, no damage states, one unit per cell (gap analysis 2.5, 2.6, 2.14, section 6). Root
cause: `CombatTerms::weapon` is one optional weapon, `WarheadDef` has only `versus`, and area
damage (`special::blast`) subtracts hit points directly.

**Change.**

1. *`combat.ron`, schema 2.* Armour types `none, flak, plate, light, medium, heavy, wood, steel,
   concrete, special_1, special_2`. Warheads gain `cell_spread` (subcells), `percent_at_max` and
   the flags the rules need (`wall`, `wood`); weapons gain `burst`, `min_range`, `anti_air`,
   `anti_ground`, `inaccurate` and the projectile flight kind; a type lists `weapons` (primary,
   secondary) and `elite_weapons`. Every weapon and warhead used by a shipped item is ported
   (damage, range, rate of fire through section 2). Fixtures migrate with neutral values.
2. *Damage path* (`combat.rs`): one function resolves an impact at a point for a warhead:
   the struck target and everything within `cell_spread` take section 4.3's damage through the
   armour table. Superweapons, bombs, suicide units, radiation and the reactor blast (RA6) call
   the same function (`special::blast` is replaced), so no area damage ignores armour.
3. *Weapon selection* (section 4.4): primary or secondary by target (air or ground, armour the
   warhead can hurt); elite weapons when the unit is elite (RA4 supplies the rank).
4. *Scatter:* a projectile of an `inaccurate` weapon lands at a point drawn from `SimRng` within
   `BallisticScatter` of its target (section 4.4).
5. *Air and ground:* targeting uses `anti_air`/`anti_ground` and the target's domain instead of
   the `air`, `naval` and `submerged` armour classes, which disappear.
6. *Crushing:* `arsenal.ron` schema 4 adds `crusher`, `crushable`, `size`, `infantry`. A crusher
   may step into a cell held only by hostile crushable infantry, which die
   (`SimEvent::UnitCrushed`).
7. *Damage states:* `Health` exposes green, yellow (at or below `ConditionYellow`) and red
   (`ConditionRed`); `SimEvent::DamageStateChanged` on a crossing. Effects that depend on them
   (garrison eviction) come in RA5.
8. *Infantry sub cells* (amendment of I6, R6): a surface cell holds one vehicle or up to three
   infantry (`arsenal` trait `infantry`), each in one of three spots with a fixed offset from the
   cell centre; `Grid::claim`/`release`/`occupant` and the movement reservation work per spot.
   A world without the trait (the fixtures) has no infantry, so I6 holds there as written.
9. *Game:* three infantry drawn at their spots, damage smoke by state, new impact effects keyed
   by warhead; the hover text names armour.

**Files.** `assets/data/{combat,arsenal}.ron`, fixtures `combat.ron`, `economy_combat.ron`,
`units_combat.ron` and the legacy fixture, `crates/sim/src/{combat_defs.rs, combat.rs, special.rs,
map.rs, movement.rs, pathfinding.rs, unit.rs, building.rs, harvest.rs, props.rs, hash.rs,
event.rs}`, `crates/game/src/{units.rs, weapon_fx.rs, fog.rs, health.rs, hover_info.rs,
selection.rs}`.

**Production paths.** `Combat` (`combat`, `special::resolve`), `Movement` (`move_units`: spots and
crushing), `Pathfinding`, `Cleanup` (`remove_dead`).

**Equations.** Sections 4.3 and 4.4.

**Invariants touched.** I6 is amended to I6': *a surface cell holds at most one vehicle or at most
three infantry and never both; an air cell holds at most one aircraft; no unit occupies a cell
covered by a building.* The amendment has decision R6, a test of I6' on every tick of a crossing
of mixed infantry and vehicles, and is flagged for the final reviewer. I3 (spots, burst state,
damage states hashed), I12 (damage products in `i64`).

**Compatibility and non goals.** `applied_damage` keeps equation 2 including the minimum of 1 for
fixture worlds; where RA2's rule differs (section 4.3) the difference applies only with rules.
Not here: veterancy multipliers (RA4), prone infantry, elevation bonuses, walls blocking shots.

**Tests.** Positive: the damage of every shipped weapon against every armour type equals the
formula (generated table); splash at 0, half and full spread; secondary weapon against aircraft;
a tank crushes a Conscript; three GIs share a cell and a fourth waits. Negative: no anti air
weapon fires at ground and the reverse; crushing an ally or a non crushable unit does not happen;
a vehicle cannot enter a cell with a friendly soldier; malformed combat file per new field.
Boundary: exactly at `cell_spread` and one subcell beyond; health exactly at 50% and 25%; scatter
at its maximum radius stays within it. Regression: the M4 fixture tests with schema 2 fixtures and
unchanged numbers; determinism of a 20 against 20 battle of mixed types. Integration: a nuclear
blast (still the M9 effect) hurts a tank less than a soldier according to its warhead. End to
end: AI matches, benchmark budget, soak determinism.

**Acceptance.** Weapon damage, range and rate of fire of every shipped weapon match the reference
through section 2 (audit); every row of gap analysis section 6 that names armour, splash, hit
model, crushing, damage states or infantry in cells is closed.

### RA4. Veterancy, crews and air and naval mechanics

**Problem and root cause.** No experience or ranks; destroyed vehicles and buildings release
nobody; strike aircraft rearm on a timer with no dock limit; V3 and Dreadnought fire ordinary
projectiles; the carrier has two recoverable aircraft; submarines are only an armour class (gap
analysis 2.3, sections 5 and 6). Root cause: units carry no experience state, and M9 modelled
aircraft, missiles and submarines with timers and roles in `special.rs`.

**Change.**

1. *Veterancy* (`crates/sim/src/veterancy.rs`, new): component `Experience { value, rank }`
   (hashed). The killer of a unit or building gains the victim's cost; ranks by section 4.5.
   `arsenal.ron` schema 5 adds `trainable`, `veteran_abilities`, `elite_abilities`. Multipliers
   apply in the order of section 4.3 (firepower), 4.5 (armour, rate of fire, speed, sight);
   `SELF_HEAL` heals by the infantry and unit repair rates. Elite units use their elite weapons
   (RA3). `SimEvent::RankGained`. Spy infiltration (RA6) and crates (RA5) use the same component.
2. *Crews and survivors* (section 4.7): on the death of a `crewed` vehicle or building and on the
   sale of a building, crew of the owner's side leave at free cells around it
   (`SimEvent::CrewEscaped`); numbers from `CrewEscape`, `SurvivorRate` and the side's divisor.
3. *Aircraft:* `ammo` and `dock` traits. A Harrier or Black Eagle needs a free dock of an
   Airforce Command to be produced (4 per command, `RejectReason::NoFreeDock`), spends ammunition
   per shot, returns to its dock when empty and reloads one round per `ReloadRate`; an aircraft
   whose dock is lost takes a free one or, with none, is lost as in RA2 (section 4.8).
4. *Spawned missiles and Hornets:* `spawns`, `spawns_number`, `spawn_regen`, `spawn_reload`
   traits. V3 Launcher and Dreadnought launch `v3_rocket` and `dred_missile` air units with their
   own strength, which fly to the target, deal `V3RocketDamage`/`DMislDamage` through the armour
   table and can be shot down by anti air. The Aircraft Carrier holds three `hornet`s that
   attack, return, rearm and are rebuilt after `SpawnRegenRate` when lost.
5. *Submerged units:* `cloakable`, `sensors` and `sensors_sight` traits. Typhoon, Dolphin and
   Giant Squid are hidden from players without a detector in range (per player hidden set, as
   spies and Mirage tanks already are), surface while they fire, and are detected by units and
   buildings with sensors within their sensor radius (section 4.8).
6. *Game:* rank insignia over units, crew and parachute free sprites from the existing infantry,
   missile and Hornet art from the generators, dock positions at the Airforce Command, a ping for
   promotions.

**Files.** `assets/data/{arsenal,rules,units,economy,combat}.ron`, `crates/sim/src/{veterancy.rs
(new), combat.rs, special.rs, movement.rs, economy.rs, economy_commands.rs, visibility.rs,
unit.rs, hash.rs, event.rs, ai.rs}`, `crates/game/src/{units.rs, health.rs, weapon_fx.rs, fog.rs,
audio.rs, notices.rs}`, `scripts/hd_aircraft.py` and the render scripts, `assets/LICENSES.md`.

**Production paths.** `Combat` (kills credit experience; multipliers), `Cleanup` (`remove_dead`:
crew, spawner bookkeeping), `Economy` (`special::advance`: reload, regeneration, self heal),
`Movement` (speed multiplier), `Visibility` (`special::reconnaissance`: detection).

**Equations.** Sections 4.5, 4.7 and 4.8.

**Invariants touched.** I3 (experience, ammunition, docks, spawner state, detection hashed), I6'
(crew spawn only on free spots; missiles and Hornets obey the air layer).

**Schema.** `arsenal.ron` 4 to 5; `rules.ron` only if a key is missing.

**Compatibility and non goals.** Units of fixture worlds are not trainable and have no crew.
Not here: paratroopers (RA6), veteran production from infiltration (RA6).

**Tests.** Positive: a Grizzly becomes veteran after destroying three times its cost and elite
after six; each multiplier measured (damage dealt, damage taken, reload ticks, cells per second);
elite self heal; crew leave a destroyed Grizzly at the documented rate over a seeded sample; a
fifth Harrier is refused and built after a second command exists; a V3 rocket is shot down by a
Patriot and deals nothing; a carrier regains a lost Hornet; a Typhoon is unseen at 5 cells from a
Grizzly and seen by a Destroyer. Negative: no experience for killing allies or own units;
untrainable units stay at rank 0; no dock, no aircraft. Boundary: experience one credit below
each threshold; ammunition 0; the last free spot for a crew member. Regression: the M9 aircraft,
carrier and naval tests updated to the RA2 rules with notes in DECISIONS. Integration: an elite
unit in a long battle; determinism with spawned missiles in flight. End to end: AI matches.

**Acceptance.** Gap analysis 2.3 and the rows of sections 5 and 6 on crew, interception, aircraft
ammunition, carrier and submarine detection are closed.

### RA5. Economy and world objects

**Problem and root cause.** Ore is finite and worth 1 credit a unit, with no gems; the free miner
is the legacy harvester; there are no crates, tech buildings, garrisons or destructible bridges
(gap analysis 2.7, 2.11, sections 5 and 10). Root cause: `Cell::ore` is a plain amount that only
`take_ore` changes, and scenery (`props.rs`) is destructible but inert.

**Change.**

1. *Bails* (`economy.ron` schema 2, section 4.10): a cell holds ore or gems in bails; miners
   carry bails (`storage`: War Miner 40, Chrono Miner 20) and the refinery pays 25 or 50 per
   bail, plus the purifier bonus. Map ore amounts convert to bails in the map loader's result
   (`ore / bail value`, at least 1), so the map format is unchanged; gems are a new `look`
   feature that the simulation knows as it knows ore.
2. *Growth and spread* (`crates/sim/src/ore.rs`, new, in the `Economy` stage): on the period of
   `[Riparius] Growth` and `Spread` a pass over the ore cells in row order rolls `SimRng` for
   growth and spread (section 4.10; `[General] GrowthRate`, which the prompt names, is unused by
   the engine). Match option `ore_grows` (default on).
3. *Free miner:* a refinery gives its side's miner; the Chrono Miner heals itself.
4. *Crates* (`crates/sim/src/crates.rs`, new): with the option on, crates appear at
   `CrateRegen` on free land or water cells, at most `CrateMaximum`, and a unit that enters the
   cell takes the effect drawn from `[Powerups]` shares by `SimRng` (section 4.11): money, unit,
   heal base, veteran, armour, firepower, speed, reveal; `FreeMCV`.
5. *Neutral player and tech buildings:* a neutral owner (no team, never defeated, never a
   winner) holds Oil Derrick, Hospital, Airport and Tech Outpost placed by maps (new `look`
   features). Engineers capture them; their effects are those of section 4.12; a captured tech
   building cannot be sold.
6. *Garrisons:* houses, city blocks and towers become neutral garrisonable buildings (they stay
   destructible). `Command::Interact` with an `occupier` infantry (GI, Conscript) enters;
   occupants fire the occupation weapon from the building; `Command::Deploy` on the building or
   red health evicts them (section 4.12). `SpecialState::garrisons` (hashed).
7. *Bridges:* each connected run of bridge cells is a bridge with hit points and a repair hut
   beside each end (a neutral, capturable only by entering: an Engineer who enters is consumed
   and the bridge is restored). A destroyed bridge's cells become water. Match option
   `bridge_destruction`. Nobody fires at a bridge unasked.
8. *Maps:* the recipes and generators place gems, tech buildings and huts symmetrically
   (`crates/game/examples/art/*.rs`), every TMX is regenerated, and `m7_maps` additionally checks
   that tech buildings and huts mirror. The three small maps change only where no test coordinate
   is affected. Every map still sets up for 2 to 10 players.
9. *Game:* art for gems, crates, the four tech buildings and the hut from the generators
   (LICENSES), garrison pips, crate pick up notices, bridge states.

**Files.** `assets/data/*.ron`, `assets/maps/**`, `crates/sim/src/{ore.rs, crates.rs (new),
map.rs, map_scale.rs, harvest.rs, economy.rs, economy_defs.rs, props.rs, special.rs, combat.rs,
match_setup.rs, pathfinding.rs, hash.rs, event.rs, ai.rs}`, `crates/game/src/{battlefield.rs,
buildings.rs, units.rs, minimap.rs, notices.rs, hover_info.rs, selection.rs, special_controls.rs}`,
`crates/game/examples/{gen_maps.rs, art/*.rs}`, the art scripts.

**Production paths.** `Economy` (`harvest`, ore growth, crates, tech building income),
`Movement` (crate pick up on entering a cell), `Commands` (`special::apply`: enter, capture,
repair), `Combat` (garrison fire, bridge damage), `Cleanup` (eviction, bridge collapse),
`WinCondition` (the neutral player is ignored).

**Equations.** Sections 4.10, 4.11 and 4.12.

**Invariants touched.** I3 (ore kind and density, crates, garrisons, bridges, neutral buildings
hashed), I6' (evicted occupants and crate units take free spots), I7 (income is added, never
negative), I11 (new look values are checked by the game and the simulation).

**Schema.** `economy.ron` 1 to 2, `arsenal.ron` 5 to 6, `rules.ron` if needed; maps stay at
schema 2 (`look` is an existing optional property).

**Compatibility and non goals.** Fixture maps and economy fixtures migrate with one credit bails
so that their expectations hold. Not here: ore drills, civilian units, height.

**Tests.** Positive: a full War Miner pays 1000 for ore and 2000 for gems; an ore field regrows
and spreads on schedule; each crate type's effect; an Oil Derrick pays its owner; three GIs
garrison a house and fire; an Engineer repairs a destroyed bridge and land units cross again.
Negative: a tank cannot garrison; a full building refuses; selling a derrick; crates off;
bridge destruction off. Boundary: ore at maximum density neither grows nor loses bails; the crate
maximum; eviction exactly at red health; a miner with one free bail of room. Regression: M3
harvest tests with schema 2 fixtures; the map checks. Integration: economy of a full match in
bails (I7 over 20,000 ticks); a path that used a destroyed bridge fails and is replanned.
End to end: AI matches on every map with the new objects; catalogue for 2 to 10 players.

**Acceptance.** Gap analysis 2.7 and 2.11 and the ore, miner, crate, garrison, tech building and
bridge rows are closed.

### RA6. Specialists and superweapons

**Problem and root cause.** Every row of gap analysis section 9 differs: M9 gave each specialist a
compact effect (suppression, timers, flat blasts) instead of RA2's mechanic.

**Change.** One sub item per row of section 9, each with its equation in section 4.13:

| Item | Change |
|---|---|
| GI | `Command::Deploy` toggles the deployed stance (cannot move, uses the deployed weapon, undeploy delay). |
| Engineer | also repairs bridges (RA5) and defuses Ivan bombs. |
| Attack Dog | kills infantry in one bite as RA2's weapon and warhead give it; detects disguise. |
| Spy | `Command::Interact` on an enemy infantry takes its appearance; enemies see the disguise (per player view); dogs and detectors see through. |
| Infiltration | refinery: steals `SpyMoneyStealPercent`; power: `SpyPowerBlackout`; superweapon: reset; barracks and war factory: veteran production; radar: resets the owner's shroud; Battle Lab: unlocks the side's two units. |
| Tanya | C4 on buildings (and ships as RA2 allows), pistols otherwise. |
| Chrono Legionnaire | erasure: the target is frozen and invulnerable and vanishes when the accumulated time reaches its strength; teleport delay by distance. |
| Tesla Trooper | charges a Tesla Coil in range: powered without power, overcharged with enough troopers. |
| Prism Tower | support chain up to `PrismSupportMax`, each adding `PrismSupportModifier`. |
| Mirage Tank | appears as a tree to enemies while still. |
| Crazy Ivan | `IvanDamage`, `IvanTimedDelay`; bombs on units and buildings. |
| Yuri | mind control of one unit; psi wave secondary. |
| Desolator | rad beam primary; deployed eruption creates radiation with level and decay. |
| Terror Drone, Giant Squid | parasite inside the target dealing damage per interval; a Service Depot removes a drone; a Dolphin's hit frees a ship. |
| IFV | weapon by passenger type from the RA2 table. |
| Nuclear Reactor | death blast with its warhead and radiation. |
| Chronosphere | every unit in the 3 x 3 source area of any owner; infantry die; ground units over water sink, ships over land are destroyed. |
| Iron Curtain | 750 frames on vehicles and buildings in the area; infantry die. |
| Lightning Storm | warning, then random bolts of `LightningDamage` for `LightningStormDuration`. |
| Nuclear Missile | launch, flight, payload damage through its warhead, radiation. |
| Paradrop | America's command and the Airport drop infantry at a target on recharge. |
| Warnings | `SimEvent::SuperweaponBuilt`, `SuperweaponReady`, `SuperweaponLaunched` to every player. |

Radiation is a per cell level grid (`SpecialState`, hashed) that decays and damages by section
4.13. Recharge times come from `[SuperWeaponTypes]`. The four unlockable units get their RA2
weapons and abilities.

**Files.** `assets/data/{arsenal,rules,combat,units,economy}.ron`, `crates/sim/src/{special.rs
(split into `special/` modules as it grows: `superweapons.rs`, `infiltration.rs`, `parasite.rs`,
`radiation.rs`, `disguise.rs`), combat.rs, command.rs, visibility.rs, economy_commands.rs,
hash.rs, event.rs, ai.rs}`, `crates/game/src/{special_controls.rs, weapon_fx.rs, units.rs,
buildings.rs, notices.rs, audio.rs, sidebar.rs}`.

**Production paths.** `Commands` (`special::apply`), `Economy` (`special::advance`: charge,
timers, radiation decay), `Combat` (`special::resolve`: effects through the RA3 damage path),
`Visibility` (disguise and detection).

**Equations.** Section 4.13.

**Invariants touched.** I3 (all new specialist state hashed), I5 (every ability is a command),
I6' (teleport and paradrop land on free spots or kill by the documented rule).

**Schema.** `arsenal.ron` 6 to 7; `rules.ron` if needed.

**Compatibility and non goals.** The M9 command surface (`Deploy`, `Interact`, `Superweapon`) is
kept and extended, not replaced. Not here: Yuri's Revenge units, the SEAL (tech level -1).

**Tests.** One positive and one negative test per row above through commands; boundaries for the
3 x 3 Chronosphere area edge, the Iron Curtain's last tick, the ninth Prism Tower, radiation
exactly at the edge of its spread and at level zero, a bomb defused in its last tick; regression
for every M9 specialist test, updated to RA2 behaviour with notes; integration: a full
superweapon exchange between two bases with warnings to both; determinism of a storm; end to end
AI matches.

**Acceptance.** Every row of gap analysis section 9 is closed or a listed difference.

### RA7. Vision, interface and controls

**Problem and root cause.** Fog of war is always on; the Gap Generator and Psychic Sensor use M9
radii; hotkeys follow the game's own scheme; notices exist only for defeats (gap analysis 2.4,
sections 10 and 11).

**Change.**

1. *Shroud only by default:* `MatchOptions::fog_of_war` (default off). Without fog an explored
   cell counts as visible for rendering, targeting and command validation (section 4.14). With
   fog on, the M4 rules apply unchanged.
2. *Gap Generator:* re-shrouds its radius for enemies every tick while powered; units inside
   their own sight see normally. *Psychic Sensor:* radius 15, and enemy move and attack orders
   that end inside it are shown to the owner (`SimEvent`/query for the game). *Spy Satellite:*
   reveals the map and makes radar infiltration ineffective.
3. *Hotkeys* (`hotkeys.rs`, R7 with a README table): Q W E R tabs; K repair; L sell; G guard; X
   scatter; F follow; Z waypoint mode; S stop; D deploy; Ctrl force fire; Alt force move; T type
   select; P all combat units; H home; N and M next and previous unit; Ctrl+F1 to F4 set and F1
   to F4 jump to bookmarks; control groups with Alt+digit centring; attack move on Ctrl+Shift
   click and `A` kept (RA2's alliance key is not needed in single player). Keys that move:
   sell `X` to `L`, global rally `R` to `Y`, pause `P` to `Pause`/`F9`, mute `M` to `F10`.
4. *New commands:* `Guard { units }`, `Scatter { units }`, `Follow { units, target }`,
   `ForceFire { units, target cell or id }`, `ForceMove`, and waypoint queues (`Move` with
   `queued: true`), each validated like `Move`/`Attack`.
5. *Context sensitive right click:* an Engineer over a capturable or repairable building, a Spy
   over an enemy building or infantry, infantry over a transport or garrisonable building,
   vehicles over a Service Depot: the click issues `Interact` (cursor shows the action).
6. *Announcer:* synthesised cues and text notices for unit ready, construction complete, base
   under attack, low power, insufficient funds, unit lost, superweapon built, ready and launched
   (`audio.rs`, `notices.rs`), original and generated in code.

**Files.** `crates/sim/src/{visibility.rs, special.rs, command.rs, movement.rs, combat.rs,
match_setup.rs, hash.rs}`, `crates/game/src/{hotkeys.rs, selection.rs, camera.rs, sidebar.rs,
special_controls.rs, fog.rs, minimap.rs, notices.rs, audio.rs, options.rs, smoke.rs}`, README.

**Production paths.** `Visibility`, `Commands`, `Movement`/`Combat` (guard, follow, force fire);
game input systems to `CommandSink`.

**Equations.** Section 4.14.

**Invariants touched.** I3 (order queues, guard state hashed), I5.

**Compatibility and non goals.** `settings.txt` keeps its format; no key remapping screen.

**Tests.** The pure functions that map input to commands for every new binding (M5 rule); shroud
only: an explored cell stays visible after the unit leaves, an enemy unit there is hidden when
out of sight; gap generator boundary at its radius; sensor at 15 and 16 cells; each new command
positive, negative (foreign units, off map) and boundary (empty selection, waypoint limit);
regression for moved keys; smoke with fog off and on.

**Acceptance.** Gap analysis 2.4 and sections 10 (except elevation) and 11 are closed or listed.

### RA8. Computer opponent

**Problem and root cause.** The computer player builds a fixed order and a ground army and uses
no aircraft, ships, specialists or superweapons; difficulties change only timing and army size
(gap analysis section 12).

**Change.** `ai.ron` schema 3 and `ai.rs`:

1. Per side and country: build order with tech, defences by `AlliedBaseDefenseCounts`/
   `SovietBaseDefenseCounts`, national specials, superweapons when enabled.
2. Army composition by tier with aircraft (dock aware), ships when shipyard water lies within
   reach of its base (the placement rule decides), V3 and artillery behind tanks.
3. Engineers capture tech buildings it has seen and repair bridges on its route; GIs and
   Conscripts garrison buildings near its base; it repairs its buildings and uses the depot.
4. Superweapons: fires at the densest known enemy buildings; Chronosphere and Iron Curtain on
   its attack wave.
5. Difficulty (section 4.15): rate of fire, build time, bonus starting credits, bonus ore
   income, wave interval and base defence counts from the ini, applied as per player multipliers
   in the combat, production and harvest paths (hashed), in addition to the existing think
   interval and wave parameters. RA2 has no firepower multiplier by difficulty (the prompt's
   list is corrected by the ini).
6. Vision stays honest (D43): no reason to cheat was found, and RA2's shroud ignoring AI is a
   listed difference.

**Files.** `assets/data/{ai,rules}.ron`, `crates/sim/src/{ai.rs, ai_defs.rs, combat.rs,
economy.rs, hash.rs}`, the AI tests and examples.

**Production paths.** `Ai` (`run_ai`), and the per player multipliers in `Economy` and `Combat`.

**Invariants touched.** I5 (commands only), I3 (controller state hashed).

**Tests.** The M6 AI tests on the new plans; zero rejected commands in the standard scenario; an
AI on a naval map builds a shipyard and ships and on a landlocked map does not; it builds and
fires a superweapon and none with the option off; an engineer captures a derrick; difficulty
multipliers measured; all 756 matches, the soak and the ten player sweep end with a winner.

**Acceptance.** Gap analysis section 12 is closed except honest vision.

### RA9. Data parity audit, balance and documentation

**Change.** The audit covers every unit, building, weapon and warhead and every `rules.ron`
value; `ra2_differences.tsv` lists each intentional difference with its reason (footprints, the
single engineer and dog, honest AI vision, flat terrain, extensions). A balance pass corrects
anything the audit or the AI matches show. `README.md` and `docs/CONSTRUCTION_SYSTEM.md` are
updated, `docs/RA2_GAP_ANALYSIS.md` is rewritten to describe only the remaining differences, and
`docs/STATUS.md` lists debt and the items that require user verification. Then the final review
of the prompt's section 7.

**Tests.** `ra9_data_parity.rs` fails on an unexplained difference, on a difference entry that no
longer applies (stale), and on a mapped section missing from the reference.

**Acceptance.** The audit is green with every difference explained; documentation describes the
shipped game.

## 6. Validation and failure conditions

### 6.1 Validation commands (all must exit 0)

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

`sh scripts/validate_ra2_parity.sh <milestone>` runs them in this order and writes
`docs/validation/ra2-parity/<milestone>/summary.txt` with every exit status and the test counts.

### 6.2 Failure conditions (stop and ask the user)

A validation command fails and cannot be fixed without abandoning the milestone's intended
behaviour; an invariant other than the planned I6 amendment would have to be broken; a test would
have to be deleted, skipped or weakened; `sim` would need a forbidden dependency; a fixed decision
proves unworkable; the benchmark budget (mean 5 ms, D55) or soak determinism cannot be restored;
RA2 material would have to enter the repository; or the third remediation round ends NO-GO. Where
an RA2 behaviour cannot be established from any source, the closest documented behaviour is
chosen, recorded as an assumption in `docs/DECISIONS.md`, and the run continues.

## 7. Reconciliation of the gap analysis (RA0 step 2)

Every row of sections 2 to 12 of `docs/RA2_GAP_ANALYSIS.md` was checked on 2026-10-11 against the
working tree (the "this game" side, with file and line) and against the cached `rules.ini` (the RA2
side). Verdicts: `confirmed`, `already different from the claim`, `not verifiable`. Totals on the
game side: sections 2, 3, 5, 6: 56 confirmed, 3 already different; sections 4, 9 to 12: 79
confirmed, 1 already different; sections 7 and 8: all 43 unit rows and 29 building rows confirmed
on both sides (the gap analysis says 45 and 30 in its prose; the tables hold 43 and 29), and of 27
prose claims 23 confirmed, 3 already different, 1 not verifiable. Line numbers refer to the files
as they were before RA1.

### Appendix A. Sections 2, 3, 5 and 6

Checked against the working tree of branch `ra2-parity` (read only, no build run) and the cached vanilla `rules.ini`.
Paths are relative to the repository root. "grep" means a case-insensitive search of `crates/sim/src`, `crates/game/src` and
`assets/data` that returned no match. RA2 keys are cited as `[Section] Key=value`; `ai.ini` is not in the cache, so AI
behaviour could not be checked.

#### Gap analysis section 2

| Row | Game claim verdict | Code evidence | RA2 claim check | Note |
|---|---|---|---|---|
| 2.1 Sides and countries | confirmed | `crates/sim/src/match_setup.rs:569-582`; `assets/data/arsenal.ron:3-74`; `crates/game/src/skirmish.rs:499-511` | confirmed by `Multiplay=yes` on nine houses (`[British]`, `[French]`, `[Germans]`, `[Americans]`, `[Alliance]` with `Side=GDI`; `[Russians]`, `[Confederation]`, `[Africans]`, `[Arabs]` with `Side=Nod`) and national items such as `[SNIPE] RequiredHouses=British` | A human's roster is the 72 arsenal entries plus every faction roster; the union adds exactly 12 ids (gun_turret, rocket_tower, rifleman, rocket_soldier, scout, light_tank, artillery, harvester, grenadier, raider, heavy_tank, rocket_launcher). No country row in the menu. |
| 2.2 Opponent armies | already different from the claim | `crates/sim/src/match_setup.rs:583-585`; `assets/data/ai.ron:47-60`; `assets/data/skirmish.ron:76-83` | not in ini (behavioural; source needed); `[IQ] SuperWeapons=4` only shows the AI may fire superweapons | The AI does get only its faction roster and never builds aircraft, ships or superweapons (no `Superweapon`, `Deploy` or `Interact` command in `ai.rs`). But the Meridian roster and the AI army list contain `conscript`, which is an arsenal item (`arsenal.ron:33`), so "none of which exist in RA2" and "never build RA2 units" are wrong for the Conscript. The AI's base buildings (power plant, refinery, barracks, war factory, Battle Lab) are arsenal items too. |
| 2.3 Veterancy | confirmed | `crates/sim/src/combat.rs:213-219` (damage is base x versus only); grep `veteran`, `elite` | confirmed by `[General] VeteranRatio=3.0`, `VeteranCombat=1.1`, `VeteranSpeed=1.2`, `VeteranArmor=1.5`, `VeteranROF=0.6`, `VeteranCap=2`; `[E1] EliteAbilities=SELF_HEAL,STRONGER,FIREPOWER,ROF`, `[E1] ElitePrimary=M60E` | `VeteranROF=0.6` multiplies the reload delay (40% shorter delay, about 67% more shots), and each bonus applies only if the unit lists it, e.g. `[MTNK] VeteranAbilities=STRONGER,FIREPOWER,SIGHT,FASTER` has no ROF. |
| 2.4 Vision | confirmed | `crates/sim/src/visibility.rs:8-9`; `crates/game/src/fog.rs:1-5` | confirmed by `[General] FogOfWar=no`, `[MultiplayerDialogSettings] FogOfWar=no`, `Shroud=yes`, `[AudioVisual] ShroudGrow=no`, `[GAGAP] GapGenerator=yes`; spy radar reset not in ini (behavioural; source needed) | |
| 2.5 Weapons per unit | confirmed | `crates/sim/src/combat_defs.rs:61`; `crates/sim/src/combat_defs.rs:145-146`; `assets/data/combat.ron:10` | confirmed by `[E1] Secondary=Para` with `Deployer=yes`, `[TANY] Secondary=Sapper` and `C4=yes`, `[HTK] Secondary=FlakTrackAAGun`, `[APOC] Secondary=MammothTusk`, `[DEST] Secondary=ASWLauncher`, `[YURI] Secondary=PsiWave`, `[DESO] Secondary=RadEruptionWeapon`, `[E1] ElitePrimary=M60E` | The IFV swaps its single weapon for its passenger's (`special.rs:664-669`); still one weapon at a time. |
| 2.6 Armour model | confirmed | `assets/data/combat.ron:20`; `assets/data/combat.ron:21-79` (13 warheads) | confirmed: all 79 warheads carry an 11-value `Verses=`, e.g. `[HE] Verses=100%,90%,80%,70%,35%,35%,75%,40%,20%,80%,100%`, `[HE] CellSpread=.5`, `[HE] PercentAtMax=.5`; the 11 names appear as `Armor=` values | |
| 2.7 Ore economy | confirmed | `crates/sim/src/map.rs:257-266`; `crates/sim/src/map.rs:290-295`; `assets/data/economy.ron:289-294` | confirmed by `[Riparius] Value=25`, `[Cruentus] Value=50`, `[General] GrowthRate=5`, `TiberiumGrows=yes`, `TiberiumSpreads=yes` | A cell has one `ore: u32` (`map.rs:118`), so no second resource; ore only ever decreases during a match. |
| 2.8 Production speed | already different from the claim | `crates/sim/src/economy.rs:613-643`; `crates/sim/src/economy.rs:71`; `assets/data/economy.ron:237-278` | confirmed by `[General] BuildSpeed=.7`, `MultipleFactory=0.8`; separate main and armoury queues not in ini (behavioural; source needed) | Confirmed: the time is a per item field with no formula in code, extra factories give no bonus, one serial queue per producer type, every building has `producer: "construction_yard"`. Different: the time is not unrelated to cost in the data. 39 of 53 units and 2 buildings (tech_center, spy_satellite) have `build_time` = cost / 3 ticks exactly (16.7 s per 1000 credits), and gi, attack_dog and terrorist sit at an 80 tick floor; the first 11 units of the file (the older roster, conscript among them) and the other 29 buildings follow no single ratio. |
| 2.9 Win condition | confirmed | `crates/sim/src/match_setup.rs:711-729`; `crates/sim/src/match_setup.rs:736-751` | confirmed by `[MultiplayerDialogSettings] ShortGame=yes`, `[General] BaseUnit=AMCV,SMCV`; long game rule not in ini (behavioural; source needed) | An MCV does not keep a player alive. With `allied_ai` the match also ends once only allied players are left. |
| 2.10 Start of match | confirmed | `assets/data/skirmish.ron:37`; `assets/data/skirmish.ron:58-63`; `assets/data/skirmish.ron:84-89` | confirmed by `[MultiplayerDialogSettings] MinMoney=5000`, `MaxMoney=10000`, `Money=10000`, `MinUnitCount=0`, `MaxUnitCount=10`, `UnitCount=10`; the MCV start follows `[General] BaseUnit=AMCV,SMCV` | The Meridian start group includes 5 conscripts, which is an arsenal unit, not only legacy units. |
| 2.11 Garrisons, tech buildings, crates | confirmed | grep `garrison`; `crates/sim/src/props.rs:1-10` (scenery is only destructible); `assets/data/economy.ron:22-158` (31 buildings, none neutral) | confirmed by `[E1] Occupier=yes`, `[E2] Occupier=yes`, `Capturable=yes` on `[CAOILD]`, `[CAHOSP]`, `[CAAIRP]`, `[CAOUTP]`, and eight `[Powerups]` entries with a non-zero share (Armor, Firepower, HealBase, Money, Reveal, Speed, Veteran, Unit) | No crate code or data exists in the game. |
| 2.12 Low power | confirmed | `crates/sim/src/economy.rs:639`; `crates/sim/src/combat.rs:663-669`; `assets/data/combat.ron:280-281` | confirmed by `[General] MinLowPowerProductionSpeed=.5`, `MaxLowPowerProductionSpeed=.8`, `[GAPILL] Power=0`, `[NALASR] Power=0`; "keep firing" and "radar switches off" not in ini (behavioural; source needed) | `[NALASR] Powered=yes` is set in the ini while `[GAPILL]` has no `Powered` key, so the Sentry Gun half of "keep firing" is not provable from the ini alone. In the game low power also stops superweapon charge, Ore Purifier, Cloning Vats, Service Depot, Spy Satellite, Gap Generator and Psychic Sensor. |
| 2.13 Radar | confirmed | `crates/game/src/minimap.rs:438-489` (no power or building condition); `crates/game/src/minimap.rs:52-61` | confirmed by `[GAAIRC] Radar=yes`, `[NARADR] Radar=yes`; the power requirement is not in ini (behavioural; source needed) | The ini also sets `[AMRADR] Radar=yes` and `[GASPYSAT] Radar=yes`, so the Spy Satellite Uplink provides radar too; the row omits it. |
| 2.14 Infantry crush | confirmed | grep `crush`; `crates/sim/src/map.rs:126-130` (a held cell blocks entry) | confirmed by `[MTNK] Crusher=yes`, `[E1] Crushable=yes` (22 sections have `Crusher=yes`) | |

#### Gap analysis section 3

| Row | Game claim verdict | Code evidence | RA2 claim check | Note |
|---|---|---|---|---|
| 3 Players | confirmed | `crates/sim/src/skirmish_defs.rs:64-69`; `crates/game/src/skirmish.rs:564-568`; `crates/sim/src/map_scale.rs:145-165` | not in ini (behavioural; source needed); `[Maximums] Players=8` is only the engine cap | Growth is `players - 2` eighths per side, rounded up to an even number of cells, limited by `MAX_MAP_SIDE = 256` (`map.rs:26`). |
| 3 Country or side | confirmed | `crates/game/src/skirmish.rs:502`; `crates/sim/src/match_setup.rs:569-582`; `crates/sim/src/match_setup.rs:614-640` | confirmed for nine countries by `Multiplay=yes` on nine houses; random choice not in ini (behavioural; source needed) | True for the human only: a computer player's faction also sets its roster (`match_setup.rs:584`). |
| 3 Enemy force | confirmed | `crates/game/src/skirmish.rs:504`; `crates/game/src/skirmish.rs:682-689` | not in ini (behavioural; source needed) | One `opponent` index is given to every computer player. |
| 3 Teams | confirmed | `crates/game/src/sim_host.rs:174-175`; `crates/sim/src/match_setup.rs:595-600`; `crates/sim/src/economy.rs:247-254` | not in ini (behavioural; source needed); only `[MultiplayerDialogSettings] AlliesAllowed=no` is present | |
| 3 Difficulty | confirmed | `assets/data/ai.ron:63`, `:73`, `:83`; `crates/game/src/skirmish.rs:686-688` | not in ini (behavioural; source needed); sections `[Easy]`, `[Normal]`, `[Difficult]` and `[MultiplayerDialogSettings] AIDifficulty=0` exist, labels and per slot choice do not | |
| 3 Starting credits | confirmed | `assets/data/skirmish.ron:37`; `crates/game/src/skirmish.rs:463-480` (no credits row) | confirmed by `[MultiplayerDialogSettings] MinMoney=5000`, `MaxMoney=10000`, `Money=10000` | |
| 3 Starting units | confirmed | `assets/data/skirmish.ron:59-63`; `assets/data/skirmish.ron:85-89` | confirmed by `[MultiplayerDialogSettings] MinUnitCount=0`, `MaxUnitCount=10`, `UnitCount=10` | Meridian starts with 2 heavy tanks, 2 raiders, 5 conscripts. |
| 3 Tech level | confirmed | `crates/game/src/skirmish.rs:762-766`; `crates/sim/src/match_setup.rs:601-606`; `crates/sim/src/economy_commands.rs:246-248` | confirmed by `[MultiplayerDialogSettings] TechLevel=10`; absence of a slider not in ini (behavioural; source needed) | |
| 3 Superweapons | confirmed | `crates/game/src/skirmish.rs:463-480` (no such row); `crates/sim/src/special.rs:408-419` | not in ini (behavioural; source needed); `[MultiplayerDialogSettings]` has no superweapon key | Always available to the human; computer players never build them (row 2.2). |
| 3 Crates | confirmed | grep `crate` (only Rust `crate` paths and prose match); `crates/game/src/skirmish.rs:463-480` (no such row) | confirmed by `[MultiplayerDialogSettings] Crates=yes` | |
| 3 Short game | confirmed | `crates/sim/src/match_setup.rs:711-729`; `crates/game/src/skirmish.rs:463-480` | confirmed by `[MultiplayerDialogSettings] ShortGame=yes` | |
| 3 MCV repacks | confirmed | `crates/sim/src/special.rs:300-353`; `crates/sim/src/special.rs:377` | confirmed by `[MultiplayerDialogSettings] MCVRedeploys=yes` | `Deploy` on a Construction Yard is rejected with "This unit has no deploy action" (its arsenal role is `combat`). |
| 3 Multi engineer | confirmed | `crates/sim/src/special.rs:593-607` | confirmed by `[MultiplayerDialogSettings] MultiEngineer=no` | One Engineer changes the owner, restores full hit points and is consumed. |
| 3 Bridge destruction | confirmed | `crates/sim/src/props.rs:42-50`; `crates/sim/src/map.rs:290-295`; `crates/sim/src/map_scale.rs:95` | confirmed by `[MultiplayerDialogSettings] BridgeDestruction=yes`, `[CABHUT] BridgeRepairHut=yes` | A bridge is a cell look (feature 8) on passable terrain; only features 4 to 7 become destructible props. |
| 3 Game speed | confirmed | `crates/game/src/game_speed.rs:25-26`; `crates/game/src/game_speed.rs:17-18`; `crates/sim/src/schedule.rs:25` | not in ini (behavioural; source needed); only `[MultiplayerDialogSettings] GameSpeed=0` is present | |
| 3 Colour | confirmed | `crates/game/src/presentation.rs:24-35`; `crates/game/src/skirmish.rs:585` | not in ini (behavioural; source needed); `[Colors]` names more colours than a lobby offers | |
| 3 Additions with no RA2 equivalent | confirmed | `crates/sim/src/economy.rs:103-108`; `crates/sim/src/match_setup.rs:422-423`; `crates/game/src/skirmish.rs:483` | not in ini (behavioural; source needed) | Unit speed choices are 100, 150, 200, 300 and 400 percent, human with Super Me only. |

#### Gap analysis section 5

| Row | Game claim verdict | Code evidence | RA2 claim check | Note |
|---|---|---|---|---|
| 5 Ore value | confirmed | `assets/data/economy.ron:289-294`; `crates/sim/src/harvest.rs:304` | confirmed by `[Riparius] Value=25`, `[Cruentus] Value=50` | |
| 5 Miner capacity | confirmed | `assets/data/economy.ron:281-287` | confirmed by `[HARV] Storage=40`, `[CMIN] Storage=20` with `[Riparius] Value=25`; gems via `[Cruentus] Value=50` | Game capacities are ore units, equal to credits at 1 credit per unit. |
| 5 Ore growth | confirmed | `crates/sim/src/map.rs:257-266`; `crates/sim/src/map.rs:290-295` | confirmed by `[General] GrowthRate=5`, `TiberiumGrows=yes`, `TiberiumSpreads=yes` | |
| 5 Free miner with refinery | confirmed | `assets/data/economy.ron:51-64` (`free_units: ["harvester"]`) | confirmed by `[GAREFN] FreeUnit=CMIN`, `[NAREFN] FreeUnit=HARV` | The game has a single refinery type. |
| 5 Chrono Miner | confirmed | `crates/sim/src/special.rs:853-863`; `assets/data/arsenal.ron:47` | confirmed by `[CMIN] Teleporter=yes`, `[CMIN] SelfHealing=yes` | The teleport fires whenever the miner heads for a refinery, which is when it is full or carries ore and none is left. `[HARV] SelfHealing=yes` too, which the row does not mention. |
| 5 Ore Purifier | confirmed | `crates/sim/src/special.rs:1098-1111` | confirmed by `[General] PurifierBonus=.25`, `[GAOREP] BuildLimit=1` | Bonus is `credits / 4` per delivery (integer division), only while not on low power; several purifiers do not stack. |
| 5 Build time | already different from the claim | `assets/data/economy.ron:175-177` (conscript 100 credits, 100 ticks); `assets/data/economy.ron:66-71` (barracks 300 credits, 240 ticks); `assets/data/economy.ron:52-57` (refinery 2000 credits, 400 ticks) | differs: `[General] BuildSpeed=.7` confirms 0.7 minutes (42 nominal s) per 1000 credits, but not "for every item": `[General] WallBuildSpeedCoefficient=3.0` changes wall time | The range is 10 to 50 s per 1000 credits at 1x, not 10 to 33: conscript 50, barracks and rifleman 40, power plant 33.3, refinery 10. Conscript and barracks are arsenal (RA2) items, so the excess is not limited to legacy items. 39 units and 2 buildings sit at exactly 16.7 (cost / 3 ticks). |
| 5 Extra factories | confirmed | `crates/sim/src/economy.rs:613-643`; `crates/sim/src/economy.rs:106-108` | confirmed by `[General] MultipleFactory=0.8` | Owned producers only supply exits; the progress step is 1 tick (10 with Super Me) whatever their number. |
| 5 Queue | confirmed | `crates/sim/src/economy.rs:42`; `crates/sim/src/economy.rs:622`; `crates/sim/src/economy_commands.rs:265-270` | confirmed by `[General] MaximumQueuedObjects=29`; parallel main and armoury queues not in ini (behavioural; source needed) | All 31 buildings name `producer: "construction_yard"`, so walls and defences share its queue. |
| 5 Low power | confirmed | `crates/sim/src/economy.rs:599`; `crates/sim/src/economy.rs:639` | confirmed by `[General] MinLowPowerProductionSpeed=.5`, `MaxLowPowerProductionSpeed=.8` | |
| 5 Sell | confirmed | `crates/sim/src/economy_commands.rs:366-385`; `assets/data/economy.ron:34`; `assets/data/economy.ron:143` | confirmed by `[General] RefundPercent=50%`, `[CAOILD] Unsellable=yes` (also `[CAHOSP]`, `[CAAIRP]`, `[CAOUTP]`); walls unsellable not in ini (behavioural; source needed) | `sell_value` is exactly half the cost for all 31 buildings (checked by script); walls and captured buildings can be sold. |
| 5 Building repair | confirmed | `crates/game/src/hotkeys.rs:95`; `crates/sim/src/economy.rs:355-360`; `crates/sim/src/economy.rs:426-432` | confirmed by `[General] RepairPercent=15%`, `RepairStep=8`; the K key is not in ini | 80 steps every 5 ticks is 400 ticks, 20 s at 1x; each step heals `max / 80`. |
| 5 Engineer repair | confirmed | `crates/sim/src/special.rs:541`; `crates/sim/src/special.rs:593-607` | not in ini (behavioural; source needed); `[ENGINEER] Engineer=yes` only | The same branch captures an enemy building and also restores it to full hit points. |
| 5 Service Depot | confirmed | `crates/sim/src/special.rs:875-888`; `assets/data/arsenal.ron:13` | partly confirmed by `[GADEPT] UnitRepair=yes`, `[NADEPT] NumberOfDocks=1`; repair cost and Terror Drone removal not in ini (behavioural; source needed) | Free: once a second every own vehicle within radius 6 gains 20 hit points while the owner is not on low power. |
| 5 Placement | confirmed | `assets/data/economy.ron:21`; `crates/sim/src/economy_commands.rs:201-212`; `crates/sim/src/economy_commands.rs:222` | differs: `Adjacent=2` holds for 29 buildable buildings, but `[GAPILL] Adjacent=4`, `[NALASR] Adjacent=4`, `[NASAM] Adjacent=4`, `[GAWALL] Adjacent=8`, `[NAWALL] Adjacent=8`, `[GAYARD] Adjacent=12`, `[NAYARD] Adjacent=12` | RA2 also widens the reach for shipyards (12), so the game's 32 is a size difference, not a unique rule. |
| 5 Power output | confirmed | `assets/data/economy.ron:32`; `assets/data/economy.ron:46` | confirmed by `[GACNST] Power=0`, `[GAPOWR] Power=200` | Also `[NAPOWR] Power=150` matches the game's tesla_reactor 150 (`economy.ron:135`). |
| 5 Crew | confirmed | `crates/sim/src/combat.rs:881-916`; `crates/sim/src/economy_commands.rs:366-385` | confirmed by `[General] CrewEscape=50%`, `AlliedCrew=E1`, `SovietCrew=E2` | In the ini 40 types set `Crewed=no` (e.g. `[APOC]`, `[HARV]`, `[V3]`), so not every RA2 vehicle releases crew. |

#### Gap analysis section 6

| Row | Game claim verdict | Code evidence | RA2 claim check | Note |
|---|---|---|---|---|
| 6 Hit model | confirmed | `crates/sim/src/combat.rs:608-623`; `crates/sim/src/combat.rs:14-16` | confirmed by `[CombatDamage] BallisticScatter=1.0` | Only `[FlakProj]` and `[FlakTProj]` set `Inaccurate=yes` in this ini, so the RA2 scatter applies to flak only. |
| 6 Splash | confirmed | `crates/sim/src/special.rs:1054-1056`; `crates/sim/src/special.rs:234-266`; `assets/data/arsenal.ron:22`, `:24`, `:57`, `:65` | confirmed: 40 of 79 warheads have `CellSpread` above 0, e.g. `[HE] CellSpread=.5`, `[HE] PercentAtMax=.5` | Splash role items are prism_tower, grand_cannon, prism_tank and kirov. The blast is added to the normal single target hit and hurts friendly units too. |
| 6 Area damage and armour | confirmed | `crates/sim/src/special.rs:226-233`; `crates/sim/src/special.rs:258-264`; `crates/sim/src/special.rs:1048-1053` | confirmed by `[NukePayload] Warhead=NUKE`, `[NUKE] Verses=100%,100%,100%,200%,100%,100%,60%,100%,8%,100%,100%`, `[General] LightningWarhead=IonWH` | Falloff is `damage x (radius + 1 - d) / (radius + 1)`. Splash role blasts and Tanya's building damage (`special.rs:1057-1059`) ignore armour as well; radiation zones are flat, not falling off. |
| 6 Intercepting missiles | confirmed | `assets/data/combat.ron:190`; `assets/data/combat.ron:204`; `crates/sim/src/combat.rs:526-534` | confirmed by `[V3] Spawns=V3ROCKET`, `[DRED] Spawns=DMISL`, `[V3ROCKET] MissileSpawn=yes`; which defences can shoot them down is not in ini (behavioural; source needed) | A game projectile is a list entry with no hit points, so nothing can target it. |
| 6 Damage states | confirmed | `crates/game/src/health.rs:48-57` (bar colour only); grep `ConditionRed`, `damage_state` in sim | confirmed by `[AudioVisual] ConditionYellow=50%`, `ConditionRed=25%`; garrison eviction not in ini (behavioural; source needed) | The game's health bar uses the same 50% and 25% thresholds for colour, with no rule attached. |
| 6 Infantry in cells | confirmed | `crates/sim/src/map.rs:126-130`; `crates/sim/src/map.rs:308-329` | not in ini (behavioural; source needed) | Precisely: one ground or naval unit per cell plus one aircraft in a separate air layer. |
| 6 Low power defences | confirmed | `crates/sim/src/combat.rs:663-669` | partly confirmed by `[GAPILL] Power=0`, `[NALASR] Power=0`, `[NASAM] Power=-50`, `[TESLA] Power=-75`; the stop rule itself is not in ini (behavioural; source needed) | The ini has a separate `Powered` key (`[NALASR] Powered=yes`, `[NASAM] Powered=yes`), so "negative Power" may not be the engine's criterion. |
| 6 Aircraft ammunition | confirmed | `crates/sim/src/special.rs:1061-1067`; `crates/sim/src/special.rs:889-916` | confirmed by `[ORCA] Ammo=1` (the Harrier's section), `[BEAG] Ammo=1`, `[GAAIRC] NumberOfDocks=4` | Rearm is 100 ticks (5 s) counted down within 2 cells of the nearest own Airforce Command; nothing limits aircraft per building. |
| 6 Carrier | confirmed | `assets/data/arsenal.ron:70`; `crates/sim/src/special.rs:1275-1281`; `crates/sim/src/special.rs:902-908` | confirmed by `[CARRIER] Spawns=HORNET`, `SpawnsNumber=3`, `SpawnRegenRate=600` | The game tops the flight up to 2 at every launch at no cost or delay, so lost aircraft are in effect replaced; the difference is the count and the missing regeneration delay. |
| 6 Transports | confirmed | `assets/data/arsenal.ron:51-52`, `:64`, `:66`; `crates/sim/src/special.rs:569-580` | confirmed by `[FV] Passengers=1`, `SizeLimit=1`; `[HTK] Passengers=5`, `SizeLimit=2`; `[SHAD] Passengers=5`, `SizeLimit=2`; `[LCRF] Passengers=12`, `SizeLimit=6` | The row understates the game: every transport except the amphibious one rejects non-infantry ("This transport carries infantry only", `special.rs:569-571`), so tanks need the amphibious transport here as well. What is missing is per unit size. |
| 6 Sub detection | confirmed | `assets/data/combat.ron:255`, `:257`, `:259`; `crates/sim/src/special.rs:1191-1198` | confirmed by `[SUB] Cloakable=yes`, `[SUB] Underwater=yes`, `[DLPH] Cloakable=yes`, `[DEST] Sensors=yes` | Concealment in the game exists only for the spy and mirage roles. `[SQD] Cloakable=yes` as well. |

### Appendix B. Sections 4, 9, 10, 11 and 12

Read-only reconciliation of `docs/RA2_GAP_ANALYSIS.md` against the working tree of branch `ra2-parity` and the cached vanilla `rules.ini`. Paths are relative to the repository root. Game durations are ticks at 20 per second. "G" is this game, "R" is Red Alert 2. Where a row has no Red Alert 2 statement the RA2 column says so.

#### Gap analysis section 4

| Claim | Game claim verdict | Code evidence | RA2 claim check | Note |
|---|---|---|---|---|
| 4 Unified tree of 72 RA2 items vs two sides and nine countries | confirmed | `assets/data/arsenal.ron:3` to `:74` (72 entries); `crates/sim/src/match_setup.rs:566` | confirmed by `[Countries] 0=Americans` to `8=Russians`; `[British] Side=GDI` (five such), `[Russians] Side=Nod` (four such) | Human roster is the whole arsenal plus every faction roster; no country choice exists. |
| 4 Prism Tank and Apocalypse never together in RA2 | confirmed | `assets/data/economy.ron:261`; `assets/data/economy.ron:262` | confirmed by `[SREF] Owner=British,French,Germans,Americans,Alliance` and `[APOC] Owner=Russians,Confederation,Africans,Arabs`; capture exception not in ini (behavioural; source needed) | `[SREF] Prerequisite=GAWEAP,GATECH` names no Construction Yard, so "capturing a Construction Yard" as the only route is an assumption. |
| 4 Twelve original items | confirmed | `assets/data/economy.ron:161` to `:231`; `assets/data/economy.ron:108`; `assets/data/economy.ron:122` | differs: `[LTNK] Name=Light Tank` exists with `TechLevel=-1` (unbuildable); the other eleven have no section by name | The twelve are the faction roster items of `assets/data/skirmish.ron:41` to `:82` that are not in `arsenal.ron` (conscript is in both). |
| 4 Black Eagle (Korea) not exclusive | confirmed | `assets/data/economy.ron:267` | confirmed by `[BEAG] RequiredHouses=Alliance` | Ini name is `[Alliance] Name=Asian Alliance`; "Korea" comes from `ra2.csf`, not read. |
| 4 Grand Cannon (France) not exclusive | confirmed | `assets/data/economy.ron:150` | confirmed by `[GTGCAN] RequiredHouses=French` | |
| 4 Tank Destroyer (Germany) not exclusive | confirmed | `assets/data/economy.ron:263` | confirmed by `[TNKD] RequiredHouses=Germans` | |
| 4 Sniper (Great Britain) not exclusive | confirmed | `assets/data/economy.ron:244` | confirmed by `[SNIPE] RequiredHouses=British` | |
| 4 Demolition Truck (Libya) not exclusive | confirmed | `assets/data/economy.ron:265` | confirmed by `[DTRUCK] RequiredHouses=Africans` | Ini name is `[Africans] Name=African Empire`. |
| 4 Desolator (Iraq) not exclusive | confirmed | `assets/data/economy.ron:249` | confirmed by `[DESO] RequiredHouses=Arabs` | Ini name is `[Arabs] Name=Arab Union`. |
| 4 Terrorist (Cuba) not exclusive | confirmed | `assets/data/economy.ron:250` | confirmed by `[TERROR] RequiredHouses=Confederation` | Ini name is `[Confederation] Name=Latin Confederation`. |
| 4 Tesla Tank (Russia) not exclusive | confirmed | `assets/data/economy.ron:264` | confirmed by `[TTNK] RequiredHouses=Russians` | |
| 4 America's paradrop absent | confirmed | `crates/sim/src/arsenal.rs:26` to `:61` (no paradrop role); `crates/sim/src/command.rs:34` to `:103` | confirmed by `[AMRADR] RequiredHouses=Americans` and `[AMRADR] SuperWeapon=AmericanParaDropSpecial` | |
| 4 Navy SEAL missing here, unbuildable in RA2 skirmish | confirmed | `assets/data/arsenal.ron:32` to `:46` (infantry list, no SEAL) | confirmed by `[GHOST] Name=SEAL`, `[GHOST] TechLevel=-1` | |
| 4 Four spy unlocked units missing | confirmed | `assets/data/arsenal.ron:32` to `:46` | confirmed by `[CCOMAND] RequiresStolenAlliedTech=yes`, `[PTROOP] RequiresStolenSovietTech=yes`, `[CIVAN] RequiresStolenAlliedTech=yes`, `[YURIPR] RequiresStolenSovietTech=yes` | `[YURIPR] BuildLimit=1` also exists; "Psi Commando" is `Name=Psi-Corp Trooper` in the ini. |
| 4 Rocketeer prerequisite | confirmed | `assets/data/economy.ron:240` | confirmed by `[JUMPJET] Prerequisite=GAPILE,RADAR`; `[General] PrerequisiteRadar=GAAIRC,NARADR,AMRADR` | Game: `airforce_command`, produced at the barracks. |
| 4 Flak Trooper prerequisite | confirmed | `assets/data/economy.ron:245` | confirmed by `[FLAKT] Prerequisite=NAHAND,NARADR` | Game: none. |
| 4 Tesla Trooper prerequisite | confirmed | `assets/data/economy.ron:246` | confirmed by `[SHK] Prerequisite=NAHAND` | Game: `radar_tower`. |
| 4 Psychic Sensor prerequisite | confirmed | `assets/data/economy.ron:153` | confirmed by `[NAPSIS] Prerequisite=NATECH,NACNST` | Game: `radar_tower`. |
| 4 Tank Destroyer prerequisite | confirmed | `assets/data/economy.ron:263` | confirmed by `[TNKD] Prerequisite=GAWEAP,RADAR` | Game: none. |
| 4 Demolition Truck prerequisite | confirmed | `assets/data/economy.ron:265` | confirmed by `[DTRUCK] Prerequisite=NAWEAP,RADAR` | Game: `tech_center`. |
| 4 Dolphin prerequisite | confirmed | `assets/data/economy.ron:273` | confirmed by `[DLPH] Prerequisite=GAYARD,GATECH` | Game: none. |
| 4 Aegis prerequisite | confirmed | `assets/data/economy.ron:272` | confirmed by `[AEGIS] Prerequisite=GAYARD,RADAR` | Game: none. |
| 4 Sea Scorpion prerequisite | confirmed | `assets/data/economy.ron:276` | confirmed by `[HYD] Prerequisite=NAYARD,NARADR` | Game: none. RA2 names the Radar Tower specifically, not any radar. |
| 4 Chronosphere also needs Airforce Command | confirmed | `assets/data/economy.ron:154` | confirmed by `[GACSPH] Prerequisite=GATECH,GACNST` | Game: `tech_center`, `airforce_command`. |
| 4 Weather Control also needs Airforce Command | confirmed | `assets/data/economy.ron:155` | confirmed by `[GAWEAT] Prerequisite=GATECH,GACNST` | Game: `tech_center`, `airforce_command`. |
| 4 Cloning Vats also need a Barracks | confirmed | `assets/data/economy.ron:142` | confirmed by `[NACLON] Prerequisite=NATECH,NACNST` | Game: `tech_center`, `barracks`. |
| 4 Ore Purifier does not need a refinery | confirmed | `assets/data/economy.ron:136` | confirmed by `[GAOREP] Prerequisite=GATECH,PROC,GACNST` | Game: `tech_center` only. |
| 4 War Factory prerequisite | confirmed | `assets/data/economy.ron:87` | confirmed by `[GAWEAP] Prerequisite=PROC,GAPILE,GACNST`; `[NAWEAP] Prerequisite=PROC,NAHAND,NACNST` | Game: `refinery` only. |
| 4 Battle Lab prerequisite | confirmed | `assets/data/economy.ron:101` | confirmed by `[GATECH] Prerequisite=GAWEAP,RADAR,GACNST`; `[NATECH] Prerequisite=NAWEAP,RADAR,NACNST` | Game: `war_factory` only. |
| 4 Prerequisite list as a whole ("usually simpler or substitutes one building") | confirmed | `assets/data/economy.ron:250`; `assets/data/economy.ron:150`; `assets/data/economy.ron:157` | differs: the list omits differences the ini shows: `[TERROR] Prerequisite=NAHAND,RADAR` (game: none); `[GTGCAN] Prerequisite=RADAR,GACNST` (game: `tech_center`); `[NAMISL] Prerequisite=NATECH,NACNST` (game adds `nuclear_reactor`); `[GAWALL] Prerequisite=GAPILE` (game: none, `economy.ron:143`) | Also: Sniper, Prism Tower, Tesla Coil and Desolator need `radar_tower` specifically here (`economy.ron:244`, `:148`, `:149`, `:249`) where RA2 accepts any `RADAR`; Kirov and NightHawk are built at `airforce_command` here (`economy.ron:269`, `:268`) but have `Prerequisite=NAWEAP,NATECH` and `Prerequisite=GAWEAP` in RA2. Only power plants are interchangeable (`crates/sim/src/arsenal.rs:272`). |
| 4 Build limits | confirmed | `crates/sim/src/economy_commands.rs:9` to `:14` (queue checks, no limit); `assets/data/economy.ron:5` to `:10` (no limit field) | confirmed by `[GAOREP] BuildLimit=1`, `[NACLON] BuildLimit=1`, `[NAIRON] BuildLimit=1`, `[GAWEAT] BuildLimit=1`, `[GACSPH] BuildLimit=1`, `[NAMISL] BuildLimit=1` | The only other `BuildLimit` in the ini is `[YURIPR] BuildLimit=1`. |

#### Gap analysis section 9

| Claim | Game claim verdict | Code evidence | RA2 claim check | Note |
|---|---|---|---|---|
| 9 GI | confirmed | `crates/sim/src/special.rs:299` to `:377` (deploy roles: MCV, transport, IFV, radiation only); `assets/data/arsenal.ron:32` | confirmed by `[E1] Deployer=yes`, `[E1] Secondary=Para`, `[Para] ROF=15`, `[Para] Range=5` (primary `[M60] ROF=20`, `Range=4`) | Sandbags are art, not in `rules.ini`. |
| 9 Engineer | confirmed | `crates/sim/src/special.rs:593` to `:608` | confirmed by `[ENGINEER] Engineer=yes`, `[ENGINEER] Primary=DefuseKit`, `[BombDisarm] BombDisarm=yes`, `[CABHUT] BridgeRepairHut=yes` | A capture also restores full hit points. Engineer is consumed (`special.rs:607`). |
| 9 Attack Dog | confirmed | `assets/data/combat.ron:178`; `crates/sim/src/special.rs:1209` to `:1216` | confirmed by `[DOG] DetectDisguise=yes`, `[ADOG] DetectDisguise=yes`; kill is `[BadTeeth] Damage=30`, `Warhead=ParasiteDog`, so the outright kill is not in ini (behavioural; source needed) | The dog also reveals an idle Mirage Tank within 6 cells. |
| 9 Spy | confirmed | `crates/sim/src/special.rs:1191`; `crates/sim/src/special.rs:1204` to `:1220` | confirmed by `[SPY] CanDisguise=yes`, `[Snapshot] MakesDisguise=yes`, `[DOG] DetectDisguise=yes` | "Enemy" means any unit or building of the observing player; distance to a building is measured to its footprint origin. |
| 9 Spy infiltration | confirmed | `crates/sim/src/special.rs:614` to `:638` | confirmed by `[General] SpyMoneyStealPercent=.5`, `[General] SpyPowerBlackout=1000`; veteran, shroud reset and superweapon reset effects not in ini (behavioural; source needed); lab unlock indicated by `[CCOMAND] RequiresStolenAlliedTech=yes` | The "power plant" branch fires for any building with `power_produced > 0`, which includes the Construction Yard (`assets/data/economy.ron:32`). Both timers are 600 ticks. |
| 9 Tanya | confirmed | `assets/data/combat.ron:180`; `crates/sim/src/special.rs:1057` to `:1059`; `assets/data/arsenal.ron:38` | confirmed by `[DoublePistols] Damage=125`, `[TANY] C4=yes`, `[TANY] Secondary=Sapper`, `[Sapper] Damage=2500` | Extra damage is `strength 20 x 10`; it bypasses the armour table. |
| 9 Chrono Legionnaire | confirmed | `crates/sim/src/special.rs:835` to `:851`; `crates/sim/src/special.rs:1030` to `:1036`; `assets/data/combat.ron:181` | confirmed by `[CLEG] Primary=NeutronRifle`, `[ChronoBeam] Temporal=yes`; frozen and invulnerable target not in ini (behavioural; source needed) | Teleport and suppression both last `cooldown` 100 ticks (5 s). |
| 9 Tesla Trooper | confirmed | `assets/data/combat.ron:184`; `assets/data/arsenal.ron:42` | confirmed by `[SHK] Secondary=AssaultBolt`, `[ElectricAssault] ElectricAssault=yes`, `[TESLA] Overpowerable=true` | `Role::Tesla` is matched nowhere outside `arsenal.rs`, so it has no special effect. |
| 9 Prism Tower | confirmed | `crates/sim/src/special.rs:1054` to `:1056`; `assets/data/arsenal.ron:22`; `assets/data/combat.ron:192` | confirmed by `[General] PrismSupportMax=8`, `[ATESLA] Secondary=PrismSupport` | Splash is a 6 cell blast of 20 with linear falloff that also hurts the owner's units. |
| 9 Mirage Tank | confirmed | `crates/sim/src/special.rs:1191` to `:1198` | confirmed by `[MGTK] DisguiseWhenStill=yes`, `[General] DefaultMirageDisguises=TREE01,TREE02,TREE03,TREE04` | Idle means no move order and weapon cooldown 0. |
| 9 Crazy Ivan | confirmed | `crates/sim/src/special.rs:1037` to `:1047`; `crates/sim/src/special.rs:1113` to `:1121`; `assets/data/arsenal.ron:43` | confirmed by `[CombatDamage] IvanDamage=400`, `[CombatDamage] IvanTimedDelay=450` | Game values: 100 ticks (5 s), 800 damage, radius 2. |
| 9 Yuri | confirmed | `crates/sim/src/special.rs:1014` to `:1028`; `crates/sim/src/special.rs:793` to `:804` | confirmed by `[YURI] Primary=MindControl`, `[Controller] MindControl=yes`, `[YURI] Secondary=PsiWave`, `[PsiWave] Damage=250`; one unit at a time not in ini (behavioural; source needed) | Units only, never buildings or another Yuri; control ends when Yuri dies. |
| 9 Desolator | confirmed | `crates/sim/src/special.rs:355` to `:376`; `crates/game/src/special_controls.rs:189`; `assets/data/arsenal.ron:45` | confirmed by `[DESO] Deployer=yes`, `[RadEruptionWeapon] RadLevel=500`, `[Radiation] RadLevelDelay=90`, `[RadBeamWeapon] Damage=125` | Game zone: radius 3, 20 damage per second for 100 ticks, no decay; primary `radiation_weapon` does 50 (`combat.ron:187`). |
| 9 Terror Drone and Squid | confirmed | `crates/sim/src/special.rs:1030` to `:1036`; `assets/data/arsenal.ron:53`; `assets/data/arsenal.ron:73` | confirmed by `[DroneJump] Warhead=Parasite`, `[Parasite] Parasite=yes`, `[SquidGrab] Warhead=ParasitePlus`, `[ParasitePlus] Paralyzes=32767`; release by a Dolphin not in ini (behavioural; source needed) | Suppression is 100 ticks (5 s). |
| 9 IFV | confirmed | `crates/sim/src/special.rs:661` to `:669`; `crates/sim/src/special.rs:864` to `:875` | differs: `[FV] Gunner=yes`, `[FV] WeaponCount=13`, `[FV] TurretCount=4`, so thirteen passenger weapons on four turret models, not one turret for every infantry type | `[FV] Weapon2=RepairBullet` confirms the Engineer repair mode. |
| 9 Nuclear Reactor | confirmed | `assets/data/arsenal.ron:8` (role `combat`); `crates/sim/src/special.rs:1048` to `:1056` (the only blast callers are bombs, suicide, splash and zones) | confirmed by `[NANRCT] Explodes=yes`, `[NANRCT] DeathWeapon=NukePayload` | |
| 9 Chronosphere | confirmed | `crates/sim/src/special.rs:441` to `:482`; `assets/data/arsenal.ron:28` | not in ini (behavioural; source needed) | Units must lie within 3 cells of the first selected unit; destinations within 3 cells of a visible target. The ini only gives `[ChronoSphereSpecial] Type=ChronoSphere`. |
| 9 Iron Curtain | confirmed | `crates/sim/src/special.rs:483` to `:501`; `assets/data/arsenal.ron:30` | confirmed by `[CombatDamage] IronCurtainDuration=750`; effect on buildings and infantry not in ini (behavioural; source needed) | Game: 200 ticks, radius 3; the Vehicles category includes aircraft and ships. |
| 9 Weather Control | confirmed | `crates/sim/src/special.rs:502` to `:509`; `crates/sim/src/special.rs:1123` to `:1136`; `assets/data/arsenal.ron:29` | confirmed by `[General] LightningDamage=250`, `[General] LightningStormDuration=180`, `[General] LightningDeferment=250`, `[General] LightningCellSpread=10` | Each pulse is 160 at the centre with linear falloff (`special.rs:262`); invulnerable units are spared. |
| 9 Nuclear Missile | confirmed | `crates/sim/src/special.rs:510` to `:512`; `crates/sim/src/special.rs:234` to `:266`; `assets/data/arsenal.ron:31` | confirmed by `[NukePayload] Damage=600`, `[NukePayload] RadLevel=500`, `[NUKE] CellSpread=10` | 2500 is the centre value; damage falls off linearly to the 6 cell edge. |
| 9 Recharge at 1x | confirmed | `assets/data/arsenal.ron:28` to `:31` (3600, 6000, 3600, 6000 ticks); `crates/sim/src/special.rs:813` to `:823` | confirmed by `[ChronoSphereSpecial] RechargeTime=7`, `[IronCurtainSpecial] RechargeTime=5`, `[LightningStormSpecial] RechargeTime=10`, `[MultiSpecial] RechargeTime=10` | Charge only advances while powered and not disabled. |
| 9 Warning to other players | confirmed | `crates/game/src/special_controls.rs:248` to `:261`; `crates/game/src/audio.rs:2074`; `crates/game/src/notices.rs:101` | not in ini (behavioural; source needed) | Only a ring and an explosion sound, and only when the human fired or sees the target or building. |
| 9 Paradrop | confirmed | `crates/sim/src/arsenal.rs:26` to `:61`; `crates/sim/src/command.rs:34` to `:103` | confirmed by `[General] AmerParaDropInf=E1`, `[General] AmerParaDropNum=8`, `[AmericanParaDropSpecial] RechargeTime=4`, `[CAAIRP] SuperWeapon=ParaDropSpecial` | |

#### Gap analysis section 10

| Claim | Game claim verdict | Code evidence | RA2 claim check | Note |
|---|---|---|---|---|
| 10 Elevation | confirmed | `crates/sim/src/map.rs:110` to `:119` (a cell has no height); `crates/game/src/battlefield.rs:176` | not in ini (behavioural; source needed) | A cliff is the look of an impassable land cell. |
| 10 Bridges | confirmed | `crates/sim/src/map_scale.rs:95`; `crates/sim/src/props.rs:42` to `:49` | confirmed by `[MultiplayerDialogSettings] BridgeDestruction=yes`, `[CombatDamage] BridgeStrength=1500`, `[CABHUT] BridgeRepairHut=yes` | A bridge is a look feature only; it is not a prop, so nothing can damage it. |
| 10 Civilian buildings | confirmed | `crates/sim/src/props.rs:1` to `:10`; `crates/sim/src/props.rs:34` to `:39` | confirmed by `[CACITY01] CanBeOccupied=yes` (120 sections carry the key), `[E1] Occupier=yes` | |
| 10 Map growth | confirmed | `crates/sim/src/map_scale.rs:6` to `:8`; `crates/sim/src/map_scale.rs:14` to `:18`; `crates/sim/src/map.rs:26` | not in ini (behavioural; source needed) | |
| 10 Gap Generator | confirmed | `crates/sim/src/special.rs:1182` to `:1190`; `crates/sim/src/special.rs:1228` to `:1250`; `crates/sim/src/visibility.rs:48` to `:57` | confirmed by `[GAGAP] GapRadiusInCells=10`, `[GAGAP] GapGenerator=yes` | The game clears only the visible bit (cells turn to fog, not black shroud), over a square of 6 cells either side of the footprint origin, for every other player including allied computer players. |
| 10 Spy Satellite | confirmed | `crates/sim/src/special.rs:1164` to `:1172` | confirmed by `[GASPYSAT] SpySat=yes`; nullifying radar infiltration not in ini (behavioural; source needed) | |
| 10 Psychic Sensor | confirmed | `crates/sim/src/special.rs:1209` to `:1216`; `assets/data/arsenal.ron:27` | confirmed by `[NAPSIS] PsychicDetectionRadius=15`; showing movement orders not in ini (behavioural; source needed) | The 6 is a literal in `special.rs`, not the arsenal radius (also 6); the sensor needs power. |

#### Gap analysis section 11

| Claim | Game claim verdict | Code evidence | RA2 claim check | Note |
|---|---|---|---|---|
| 11 Sidebar tabs | confirmed | `crates/game/src/sidebar.rs:590` to `:594`; `crates/game/src/hotkeys.rs:100`; `crates/game/src/sidebar.rs:909` | not in ini (behavioural; source needed) | |
| 11 Repair and sell | confirmed | `crates/game/src/hotkeys.rs:94`; `crates/game/src/hotkeys.rs:95`; `crates/game/src/sidebar.rs:648` | not in ini (behavioural; source needed) | Not in the claim: Delete or Backspace sells the building under the cursor (`hotkeys.rs:97`) and the sidebar has a Repair button (`sidebar.rs:650`). |
| 11 Attack move | confirmed | `crates/game/src/hotkeys.rs:92`; `crates/game/src/selection.rs:877` | not in ini (behavioural; source needed) | |
| 11 Guard, scatter, follow | confirmed | `crates/game/src/hotkeys.rs:35` to `:56`; `crates/sim/src/command.rs:34` to `:103` | not in ini (behavioural; source needed) | No such hotkey or command variant exists. |
| 11 Waypoint mode | confirmed | `crates/game/src/hotkeys.rs:35` to `:56`; `crates/sim/src/command.rs:34` to `:103` | not in ini (behavioural; source needed) | |
| 11 Force fire and force move | confirmed | `crates/game/src/selection.rs:559` (Shift is the only modifier read); `crates/game/src/hotkeys.rs:86` to `:89` | not in ini (behavioural; source needed) | Scenery is attacked by an ordinary right click, not a modifier. |
| 11 Type select | confirmed | `crates/game/src/selection.rs:517` to `:544` | not in ini (behavioural; source needed) | A second click within 0.35 s selects the same unit type on screen. |
| 11 Unit cycling, bookmarks, home | confirmed | `crates/game/src/hotkeys.rs:90` to `:107`; `crates/game/src/camera.rs:261` to `:264` | not in ini (behavioural; source needed) | The camera reads only the arrow keys and Space drag. |
| 11 Control groups | confirmed | `crates/game/src/hotkeys.rs:76` to `:85`; `crates/game/src/hotkeys.rs:112` to `:118` | not in ini (behavioural; source needed) | No Alt + digit centring exists. |
| 11 Alliance toggle | confirmed | `crates/game/src/hotkeys.rs:92` (A is attack move); `crates/sim/src/command.rs:34` to `:103` | not in ini (behavioural; source needed) | |
| 11 Voices | confirmed | `crates/game/src/audio.rs:3`; `crates/game/src/notices.rs:2`; `crates/game/src/notices.rs:101` | not in ini (behavioural; source needed) | The ini only names voice lines, for example `[MultiSpecial] RechargeVoice=00-I154`. |
| 11 Rally points | confirmed | `crates/game/src/hotkeys.rs:96`; `crates/game/src/construction.rs:418`; `crates/game/src/selection.rs:284` to `:289` | not in ini (behavioural; source needed) | R and the sidebar Rally button are global; a right click with a factory selected is per building. |

#### Gap analysis section 12

| Claim | Game claim verdict | Code evidence | RA2 claim check | Note |
|---|---|---|---|---|
| 12 Fixed order of legacy buildings | confirmed | `assets/data/ai.ron:28` to `:44`; `crates/sim/src/ai.rs:20` to `:24` | no RA2 statement | Entries a faction roster does not offer are skipped. |
| 12 Army from the faction's legacy roster | confirmed | `assets/data/ai.ron:47` to `:60`; `assets/data/skirmish.ron:50` to `:57`; `assets/data/skirmish.ron:76` to `:83` | no RA2 statement | |
| 12 No naval, air, specialist or superweapon play vs full RA2 arsenal | confirmed | `assets/data/ai.ron:47` to `:60`; `crates/sim/src/ai.rs:18` to `:40` | not in ini (behavioural; source needed); partial hint `[General] AIMinorSuperReadyPercent=.7` | `ai.rs` only issues AttackMove, Harvest, PlaceBuilding, QueueProduction, SellBuilding and SetRallyPoint. |
| 12 No defence beyond its turret type | confirmed | `assets/data/ai.ron:33` to `:34`; `assets/data/skirmish.ron:48`; `assets/data/skirmish.ron:74` | no RA2 statement | |
| 12 AI knows only its vision vs RA2 AI seeing through shroud | confirmed | `crates/sim/src/ai.rs:10` to `:16`; `crates/sim/src/ai.rs:563`; `crates/sim/src/ai.rs:568` | not in ini (behavioural; source needed) | Also public to it: static terrain and start positions; it remembers buildings it has seen. `ai.rs` does not consult the concealment set, so a concealed unit inside its vision is listed. |
| 12 Difficulties change only think interval, harvester count, army cap and wave timing | already different from the claim | `assets/data/ai.ron:67`; `assets/data/ai.ron:70`; `crates/sim/src/ai_defs.rs:90` to `:108` | differs: `[Easy]`, `[Normal]` and `[Difficult]` set `ROF=`, `BuildTime=`, `RepairDelay=`, `BuildDelay=` and `Armor=` but no `Firepower=` line (for example `[Easy] Armor=1.2`, `[Difficult] Armor=.8`); bonus money is `[General] AIVirtualPurifiers=4,2,0` and `[General] MultiplayerAICM=400,0,0`; team frequency is `[General] TeamDelays=2000,2500,3500` | Game: `wave_size` (6, 8, 10) and `defence_radius` (8, 10, 12) also differ by difficulty; army cap and wave size also grow with map size for all difficulties (`crates/sim/src/ai.rs:160` to `:166`). |
| 12 All computer players form one team | confirmed | `crates/game/src/sim_host.rs:175`; `crates/sim/src/match_setup.rs:595` to `:599` | not in ini (behavioural; source needed) | `[MultiplayerDialogSettings] AlliesAllowed=no` is the only related key; the patch 1.004 detail stays unverified. |

### Appendix C. Sections 7 and 8

#### Gap analysis section 7

Checked every cell of the two tables of `docs/RA2_GAP_ANALYSIS.md` sections 7 and 8 against
`assets/data/{units,economy,combat,arsenal}.ron` in the working tree (game side) and against the
vanilla `rules.ini` and `art.ini` read with `scripts/ra2_ini.py` (RA2 side). The tables hold
43 unit rows and 29 building rows (the 72 entries of `arsenal.ron`), not 45 and 30. A side of a
row is `confirmed` when every cell of that side equals the data; `note:` entries are not errors
of the claim but data the single claimed value hides (the other faction's ini section, or a key
the claim's convention ignores).

Conventions of the check: game build seconds = `build_time` / 20; game range = `range` / 256
cells; game reload = `reload` / 20 seconds; RA2 nominal build seconds = `Cost` x `[General]
BuildSpeed=.7` x 60 / 1000, rounded half up; relative speed = speed / GI speed (gi speed 14 at
assets/data/units.ron:69; `[E1] Speed=4`); RA2 prerequisites drop the construction yard (`GACNST`,
`NACNST`), which every RA2 building lists and the gap table leaves implicit; RA2 footprint is
`Foundation=` of the art.ini section named by the rules `Image=` (or by the section id); a
`Burst=` above 1 is shown as `x<burst>`.

Game side: 43 confirmed. RA2 side: 43 confirmed. Where two ini sections are given the first is the one the
claim was checked against and the second is the other faction's version.

| Unit | ini section | Game side verdict | RA2 side verdict | Differences found (cell: claimed -> actual, with evidence) |
|---|---|---|---|---|
| gi | E1 | confirmed | confirmed | none |
| conscript | E2 | confirmed | confirmed | none |
| engineer | ENGINEER / SENGINEER | confirmed | confirmed | note: [ENGINEER] Primary=DefuseKit is a non-damaging tool weapon, shown as - in the claim |
| attack dog | ADOG / DOG | confirmed | confirmed | note: Soviet [DOG] weapon is BadTeeth 30 / 1.5c / 30f ([DOG] Primary=BadTeeth; [BadTeeth] Damage=30, Range=1.5, ROF=30) |
| rocketeer | JUMPJET | confirmed | confirmed | note: [JUMPJET] uses the jumpjet locomotor and also sets JumpjetSpeed=30; the claimed ratio uses Speed=9, and the ini does not say which of the two applies in flight |
| spy | SPY | confirmed | confirmed | note: [SPY] Primary=MakeupKit is a non-damaging tool weapon, shown as - in the claim |
| tanya | TANY | confirmed | confirmed | none |
| chrono legionnaire | CLEG | confirmed | confirmed | none |
| sniper | SNIPE | confirmed | confirmed | none |
| flak trooper | FLAKT | confirmed | confirmed | none |
| tesla trooper | SHK | confirmed | confirmed | none |
| crazy ivan | IVAN | confirmed | confirmed | none |
| yuri | YURI | confirmed | confirmed | none |
| desolator | DESO | confirmed | confirmed | none |
| terrorist | TERROR | confirmed | confirmed | none |
| chrono miner | CMIN | confirmed | confirmed | none |
| war miner | HARV | confirmed | confirmed | none |
| grizzly | MTNK | confirmed | confirmed | none |
| rhino | HTNK | confirmed | confirmed | none |
| ifv | FV | confirmed | confirmed | none |
| flak track | HTK | confirmed | confirmed | none |
| terror drone | DRON | confirmed | confirmed | none |
| v3 launcher | V3 | confirmed | confirmed | none |
| mcv | AMCV / SMCV | confirmed | confirmed | note: Soviet [SMCV] Prerequisite=NAWEAP,NADEPT (war factory(Sov), service depot(Sov)) |
| mirage tank | MGTK | confirmed | confirmed | none |
| prism tank | SREF | confirmed | confirmed | note: [SREF] Weapon1=Comet (the Primary= line of [SREF] is commented out in the ini) |
| apocalypse | APOC | confirmed | confirmed | none |
| tank destroyer | TNKD | confirmed | confirmed | none |
| tesla tank | TTNK | confirmed | confirmed | none |
| demolition truck | DTRUCK | confirmed | confirmed | none |
| harrier | ORCA | confirmed | confirmed | none |
| black eagle | BEAG | confirmed | confirmed | none |
| nighthawk | SHAD | confirmed | confirmed | note: [SHAD] uses the jumpjet locomotor and also sets JumpjetSpeed=30; the claimed ratio uses Speed=14, and the ini does not say which of the two applies in flight |
| kirov | ZEP | confirmed | confirmed | note: [ZEP] uses the jumpjet locomotor and also sets JumpjetSpeed=5; the claimed ratio uses Speed=5, and the ini does not say which of the two applies in flight |
| amphibious transport | LCRF / SAPC | confirmed | confirmed | note: Soviet [SAPC] Armor=heavy against [LCRF] Armor=light; note: Soviet [SAPC] Prerequisite=NAYARD (naval shipyard(Sov)) |
| destroyer | DEST | confirmed | confirmed | none |
| aegis cruiser | AEGIS | confirmed | confirmed | none |
| dolphin | DLPH | confirmed | confirmed | none |
| aircraft carrier | CARRIER | confirmed | confirmed | none |
| typhoon | SUB | confirmed | confirmed | none |
| sea scorpion | HYD | confirmed | confirmed | none |
| giant squid | SQD | confirmed | confirmed | none |
| dreadnought | DRED | confirmed | confirmed | none |

#### Gap analysis section 8

Game side: 29 confirmed. RA2 side: 29 confirmed. For a shared building the claim was checked against the
Allied section (first); the Soviet (or American) section is second and its differing values are
listed as notes. Game-side verdicts cover `economy.ron` (cost, build time, power, footprint,
prerequisites) and `combat.ron` (hit points, weapon).

| Building | ini section | Game side verdict | RA2 side verdict | Differences found (cell: claimed -> actual, with evidence) |
|---|---|---|---|---|
| construction yard | GACNST / NACNST | confirmed | confirmed | none |
| power plant | GAPOWR | confirmed | confirmed | none |
| tesla reactor | NAPOWR | confirmed | confirmed | none |
| refinery | GAREFN / NAREFN | confirmed | confirmed | none |
| ore purifier | GAOREP | confirmed | confirmed | none |
| nuclear reactor | NANRCT | confirmed | confirmed | none |
| barracks | GAPILE / NAHAND | confirmed | confirmed | note: Soviet [NAHAND] footprint 2x2 against 3x2 (art.ini [NAHAND] Foundation=2x2) |
| war factory | GAWEAP / NAWEAP | confirmed | confirmed | note: Soviet [NAWEAP] Prerequisite=PROC,NAHAND,NACNST (any refinery, barracks(Sov)) |
| naval shipyard | GAYARD / NAYARD | confirmed | confirmed | note: Soviet [NAYARD] Power=-20 against [GAYARD] Power=-25 |
| airforce command | GAAIRC / AMRADR | confirmed | confirmed | none |
| service depot | GADEPT / NADEPT | confirmed | confirmed | note: Soviet [NADEPT] Power=-20 against [GADEPT] Power=-25; note: Soviet [NADEPT] footprint 4x3 against 3x3 (art.ini [NADEPT] Foundation=4x3); note: Soviet [NADEPT] Prerequisite=NAWEAP,NACNST (war factory(Sov)) |
| tech center | GATECH / NATECH | confirmed | confirmed | note: Soviet [NATECH] footprint 3x3 against 3x2 (art.ini [NATECH] Foundation=3x3); note: Soviet [NATECH] Prerequisite=NAWEAP,RADAR,NACNST (war factory(Sov), any radar) |
| radar tower | NARADR | confirmed | confirmed | none |
| cloning vats | NACLON | confirmed | confirmed | none |
| fortress walls | GAWALL / NAWALL | confirmed | confirmed | note: [GAWALL] has no Power= key (taken as 0); note: Soviet [NAWALL] Prerequisite=NAHAND (barracks(Sov)); note: [General] WallBuildSpeedCoefficient=3.0 multiplies wall build time and is not in the nominal formula |
| pillbox | GAPILL | confirmed | confirmed | none |
| sentry gun | NALASR | confirmed | confirmed | none |
| patriot | NASAM | confirmed | confirmed | none |
| flak cannon | NAFLAK | confirmed | confirmed | none |
| prism tower | ATESLA | confirmed | confirmed | none |
| tesla coil | TESLA | confirmed | confirmed | none |
| grand cannon | GTGCAN | confirmed | confirmed | none |
| gap generator | GAGAP | confirmed | confirmed | none |
| spy satellite | GASPYSAT | confirmed | confirmed | none |
| psychic sensor | NAPSIS | confirmed | confirmed | none |
| chronosphere | GACSPH | confirmed | confirmed | none |
| weather control | GAWEAT | confirmed | confirmed | none |
| iron curtain | NAIRON | confirmed | confirmed | none |
| nuclear silo | NAMISL | confirmed | confirmed | none |

#### Section 7 and 8 prose claims

Each sentence-level claim of the paragraphs above the two tables, split where one sentence makes
several claims. Unit counts are over the 43 table rows; RA2 values are those of the first ini
section of the id mapping.

| Claim | Verdict | Evidence |
|---|---|---|
| 7: Costs match RA2 for every unit except the Black Eagle (1400 against 1200) | confirmed | cost mismatches: black_eagle; assets/data/economy.ron:267 cost 1400; [BEAG] Cost=1200 |
| 7: Hit points mostly do not match | confirmed | hit points equal for 19 of 43 units, so 24 differ (combat.ron units against Strength=) |
| 7: Sight mostly does not match | confirmed | sight equal for 15 of 43 units (combat.ron sight against Sight=) |
| 7: Weapons mostly do not match | confirmed | damage, range and reload (ROF frames / 15) all equal for 0 armed units; 5 units are unarmed on both sides |
| 7: Relative speed mostly does not match | confirmed | relative speed equal for 4 of 43 units (gi, conscript, engineer, dreadnought) |
| 7: Build time mostly does not match | confirmed | game seconds equal the RA2 nominal seconds for 0 of 43 units; the game is shorter for 42 and longer for conscript |
| 7: every arsenal unit except the Sniper sees 8 cells | already different from the claim | conscript sight 6 (assets/data/combat.ron:210); sniper sight 12 (assets/data/combat.ron:226); the conscript row of the table itself says 6 |
| 7: RA2 sight ranges from 4 to 12 | confirmed | minimum 4 (engineer, chrono_miner, war_miner, terror_drone, dolphin, typhoon, e.g. [ENGINEER] Sight=4), maximum 12 ([YURI] Sight=12) |
| 7: about half the infantry have more hit points than in RA2 | confirmed | 8 of 15 infantry have more (engineer, rocketeer, tanya, chrono_legionnaire, tesla_trooper, crazy_ivan, desolator, terrorist); 2 have fewer (conscript, sniper) |
| 7: several vehicles and ships have more hit points than in RA2 | confirmed | 14 of 28 non-infantry units have more (rhino, flak_track, v3_launcher, mirage_tank, prism_tank, apocalypse, tesla_tank, harrier, black_eagle, nighthawk, amphibious_transport, aircraft_carrier, giant_squid, dreadnought); 0 have fewer |
| 7: Tanya 250 against 125, Apocalypse 1000 against 800, Amphibious Transport 1000 against 300 | confirmed | tanya 250 (assets/data/combat.ron:224), [TANY] Strength=125; apocalypse 1000 (assets/data/combat.ron:244), [APOC] Strength=800; amphibious_transport 1000 (assets/data/combat.ron:252), [LCRF] Strength=300; [SAPC] Strength=300 |
| 7: the Harrier is about twice as fast relative to infantry | confirmed | game 6.86 (speed 96, assets/data/units.ron:98) against RA2 3.50 ([ORCA] Speed=14): ratio 1.96 |
| 7: the Black Eagle is about twice as fast relative to infantry | confirmed | game 7.86 (speed 110, assets/data/units.ron:99) against RA2 3.50 ([BEAG] Speed=14): ratio 2.25 |
| 7: the Kirov is less than half as fast | confirmed | game 0.57 (speed 8, assets/data/units.ron:101) against RA2 1.25 ([ZEP] Speed=5): ratio 0.46; [ZEP] also sets JumpjetSpeed=5, so the RA2 figure from Speed= is nominal |
| 7: the Terror Drone is almost twice as fast | confirmed | game 4.57 (speed 64, assets/data/units.ron:89) against RA2 2.50 ([DRON] Speed=10): ratio 1.83 |
| 7: RA2 V3 and Dreadnought damage come from `V3RocketDamage` and `DMislDamage` | confirmed | [General] V3RocketDamage=200, [General] DMislDamage=300; the launcher weapons carry placeholders: [V3Launcher] Damage=1, [DredLauncher] Damage=50 |
| 7: RA2 armour names are the ini names | confirmed | all 43 Armour R cells equal the ini `Armor=` value including case ([SHK] Armor=Plate, [DESO] Armor=plate) |
| 8: Most building costs match, except Power Plant (300 against 800), Barracks (300 against 500), Battle Lab (1500 against 2000), Nuclear Reactor (1500 against 1000), Chronosphere and Iron Curtain (3000 against 2500) | confirmed | power_plant 300 (assets/data/economy.ron:42) against [GAPOWR] Cost=800; nuclear_reactor 1500 (assets/data/economy.ron:137) against [NANRCT] Cost=1000; barracks 300 (assets/data/economy.ron:70) against [GAPILE] Cost=500; tech_center 1500 (assets/data/economy.ron:98) against [GATECH] Cost=2000; chronosphere 3000 (assets/data/economy.ron:154) against [GACSPH] Cost=2500; iron_curtain 3000 (assets/data/economy.ron:156) against [NAIRON] Cost=2500; the other 23 match |
| 8: Footprints are smaller than RA2 for most structures | already different from the claim | by cell count against the Allied section: 12 of 29 smaller (construction_yard, tesla_reactor, refinery, ore_purifier, nuclear_reactor, barracks, war_factory, naval_shipyard, service_depot, tech_center, chronosphere, iron_curtain), 15 equal, 2 larger (airforce_command, gap_generator); it holds only for the 14 ids of arsenal category buildings, of which 10 are smaller |
| 8: defensive towers have 33 to 75% of RA2 hit points | already different from the claim | pillbox 300/400 = 75%; sentry_gun 300/400 = 75%; patriot 300/900 = 33%; flak_cannon 300/900 = 33%; prism_tower 300/600 = 50%; tesla_coil 300/600 = 50%; grand_cannon 1000/900 = 111%; true for the six 1x1 towers, not for the grand cannon (assets/data/combat.ron:286 against [GTGCAN] Strength=900) |
| 8: the Power Plant, Refinery and War Factory have less hit points | confirmed | power_plant 450 (assets/data/combat.ron:264) against [GAPOWR] Strength=750; refinery 750 (assets/data/combat.ron:265) against [GAREFN] Strength=1000; war_factory 900 (assets/data/combat.ron:267) against [GAWEAP] Strength=1000; the list is not exhaustive: also less are barracks, naval_shipyard, service_depot |
| 8: most late support buildings have more hit points | not verifiable | "late support buildings" is not a defined set; among the 22 unarmed buildings 9 have more (construction_yard, tesla_reactor, ore_purifier, airforce_command, tech_center, gap_generator, psychic_sensor, chronosphere, iron_curtain), 7 are equal (nuclear_reactor, radar_tower, cloning_vats, fortress_walls, spy_satellite, weather_control, nuclear_silo), 6 have less (power_plant, refinery, barracks, war_factory, naval_shipyard, service_depot); of the 9 unarmed buildings that need the tech center in the game 4 have more and 5 are equal |
| 8: the Pillbox deals under a third of RA2 damage per shot | confirmed | rifle damage 14 (assets/data/combat.ron:84) against [Vulcan2] Damage=50: 28% |
| 8: the Sentry Gun deals under a third of RA2 damage per shot | confirmed | scout_gun damage 9 (assets/data/combat.ron:116) against [Vulcan] Damage=50: 18% |
| 8: the Tesla Coil deals half of RA2 damage per shot | confirmed | tesla_weapon damage 100 (assets/data/combat.ron:184) against [CoilBolt] Damage=200 |
| 8: The Patriot and Flak Cannon both use the Aegis missile in this game | confirmed | patriot weapons aegis_missile (assets/data/combat.ron:282); flak_cannon weapons aegis_missile (assets/data/combat.ron:283) |

#### Id mapping

Game id to rules.ini section, verified from each section's `Name=`, `UIName=`, cost, weapon and
prerequisites. `UIName=` is only a string-table key (`Name:<id>`), so the RA2 display name given is
the ini `Name=`; the game display name is the `label` of `assets/data/arsenal.ron`. Where two
sections are listed the first is the Allied one. RON lines are the unit or building entry in
`units.ron` (u), `economy.ron` (e), `combat.ron` (c) and `arsenal.ron` (a). The art.ini section
holds `Foundation=` for buildings.

| Game id | Game label | rules.ini section(s) | ini `Name=` | art.ini section (rules `Image=` or id) | Side by `Owner=` and country limits | RON lines |
|---|---|---|---|---|---|---|
| gi | GI | E1 | GI | GI | [E1] Allied | u:69 e:237 c:219 a:32 |
| conscript | Conscript | E2 | Conscript | CONS | [E2] Soviet | u:24 e:174 c:210 a:33 |
| engineer | Engineer | ENGINEER / SENGINEER | Engineer / Soviet Engineer | ENGINEER / ENGINEER | [ENGINEER] Owner= all houses, ForbiddenHouses=Russians,Confederation,Africans,Arabs; [SENGINEER] Owner= all houses, ForbiddenHouses=British,French,Germans,Americans,Alliance | u:70 e:238 c:220 a:34 |
| attack_dog | Attack Dog | ADOG / DOG | Allied Attack Dog / Attack Dog | ADOG / DOG | [ADOG] Owner= all houses, ForbiddenHouses=Russians,Confederation,Africans,Arabs; [DOG] Owner= all houses, ForbiddenHouses=British,French,Germans,Americans,Alliance | u:71 e:239 c:221 a:35 |
| rocketeer | Rocketeer | JUMPJET | Rocketeer | ROCK | [JUMPJET] Allied | u:72 e:240 c:222 a:36 |
| spy | Spy | SPY | Spy | SPY | [SPY] Allied | u:73 e:241 c:223 a:37 |
| tanya | Tanya | TANY | Tanya | TANY | [TANY] Allied | u:74 e:242 c:224 a:38 |
| chrono_legionnaire | Chrono Legionnaire | CLEG | Chrono Legionnaire | CLEG | [CLEG] Allied | u:75 e:243 c:225 a:39 |
| sniper | Sniper | SNIPE | Sniper | SNIPE | [SNIPE] Allied, RequiredHouses=British | u:76 e:244 c:226 a:40 |
| flak_trooper | Flak Trooper | FLAKT | Flak Trooper | FLAKT | [FLAKT] Soviet | u:77 e:245 c:227 a:41 |
| tesla_trooper | Tesla Trooper | SHK | Shock Trooper | SHK | [SHK] Soviet | u:78 e:246 c:228 a:42 |
| crazy_ivan | Crazy Ivan | IVAN | Crazy Ivan | IVAN | [IVAN] Soviet | u:79 e:247 c:229 a:43 |
| yuri | Yuri | YURI | Yuri | YURI | [YURI] Soviet | u:80 e:248 c:230 a:44 |
| desolator | Desolator | DESO | Desolater | DESO | [DESO] Soviet, RequiredHouses=Arabs | u:81 e:249 c:231 a:45 |
| terrorist | Terrorist | TERROR | Terrorist | TRST | [TERROR] Soviet, RequiredHouses=Confederation | u:82 e:250 c:232 a:46 |
| chrono_miner | Chrono Miner | CMIN | Chrono Miner | CMIN | [CMIN] Allied | u:83 e:251 c:233 a:47 |
| war_miner | War Miner | HARV | War Miner | HARV | [HARV] Soviet | u:84 e:252 c:234 a:48 |
| grizzly | Grizzly Battle Tank | MTNK | Grizzly Battle Tank | GTNK | [MTNK] Allied | u:85 e:253 c:235 a:49 |
| rhino | Rhino Heavy Tank | HTNK | Rhino Heavy Tank | HTNK | [HTNK] Soviet | u:86 e:254 c:236 a:50 |
| ifv | Infantry Fighting Vehicle (IFV) | FV | IFV | FV | [FV] Allied | u:87 e:255 c:237 a:51 |
| flak_track | Flak Track | HTK | Flak Track | HTK | [HTK] Soviet | u:88 e:256 c:238 a:52 |
| terror_drone | Terror Drone | DRON | Terror Drone | DRON | [DRON] Soviet | u:89 e:257 c:239 a:53 |
| v3_launcher | V3 Rocket Launcher | V3 | V3 Launcher | V3 | [V3] Soviet | u:90 e:258 c:240 a:54 |
| mcv | Mobile Construction Vehicle (MCV) | AMCV / SMCV | Allied Construction Vehicle / Soviet Construction Vehicle | MCV / SMCV | [AMCV] Allied; [SMCV] Soviet | u:91 e:259 c:241 a:55 |
| mirage_tank | Mirage Tank | MGTK | Mirage Tank | RTNK | [MGTK] Allied | u:92 e:260 c:242 a:56 |
| prism_tank | Prism Tank | SREF | Prism Tank | SREF | [SREF] Allied | u:93 e:261 c:243 a:57 |
| apocalypse | Apocalypse Tank | APOC | Apocalypse | MTNK | [APOC] Soviet | u:94 e:262 c:244 a:58 |
| tank_destroyer | Tank Destroyer | TNKD | Tank Destroyer | TNKD | [TNKD] Allied, RequiredHouses=Germans | u:95 e:263 c:245 a:59 |
| tesla_tank | Tesla Tank | TTNK | Tesla Tank | TTNK | [TTNK] Soviet, RequiredHouses=Russians | u:96 e:264 c:246 a:60 |
| demolition_truck | Demolition Truck | DTRUCK | Demolitions Truck | TRUCKA | [DTRUCK] Soviet, RequiredHouses=Africans | u:97 e:265 c:247 a:61 |
| harrier | Harrier | ORCA | Intruder | FALC | [ORCA] Allied, ForbiddenHouses=Alliance | u:98 e:266 c:248 a:62 |
| black_eagle | Black Eagle | BEAG | Black Eagle | BEAG | [BEAG] Allied, RequiredHouses=Alliance | u:99 e:267 c:249 a:63 |
| nighthawk | NightHawk Transport | SHAD | BlackHawk Transport | SHAD | [SHAD] Allied | u:100 e:268 c:250 a:64 |
| kirov | Kirov Airship | ZEP | Kirov Airship | ZEP | [ZEP] Soviet | u:101 e:269 c:251 a:65 |
| amphibious_transport | Amphibious Transport | LCRF / SAPC | Landing Craft / Armored Transport | LCRF / TRS | [LCRF] Allied; [SAPC] Soviet | u:102 e:270 c:252 a:66 |
| destroyer | Destroyer | DEST | Destroyer | DEST | [DEST] Allied | u:103 e:271 c:253 a:67 |
| aegis_cruiser | Aegis Cruiser | AEGIS | Aegis Cruiser | AEGIS | [AEGIS] Allied | u:104 e:272 c:254 a:68 |
| dolphin | Dolphin | DLPH | Dolphin | DLPH | [DLPH] Allied | u:105 e:273 c:255 a:69 |
| aircraft_carrier | Aircraft Carrier | CARRIER | Aircraft Carrier | CARRIER | [CARRIER] Allied | u:106 e:274 c:256 a:70 |
| typhoon | Typhoon Attack Submarine | SUB | Typhoon Attack Sub | SUB | [SUB] Soviet | u:107 e:275 c:257 a:71 |
| sea_scorpion | Sea Scorpion | HYD | Sea Scorpion | HYD | [HYD] Soviet | u:108 e:276 c:258 a:72 |
| giant_squid | Giant Squid | SQD | Giant Squid | SQD | [SQD] Soviet | u:109 e:277 c:259 a:73 |
| dreadnought | Dreadnought | DRED | Dreadnought | DRED | [DRED] Soviet | u:110 e:278 c:260 a:74 |
| construction_yard | Construction Yard | GACNST / NACNST | Allied Construction Yard / Soviet Construction Yard | GACNST / NACNST | [GACNST] Allied, TechLevel=-1; [NACNST] Soviet, TechLevel=-1 | e:23 c:263 a:3 |
| power_plant | Power Plant | GAPOWR | Power Plant | GAPOWR | [GAPOWR] Allied (Owner= all houses, Prerequisite has GACNST) | e:37 c:264 a:4 |
| tesla_reactor | Tesla Reactor | NAPOWR | Tesla Reactor | NAPOWR | [NAPOWR] Soviet (Owner= all houses, Prerequisite has NACNST) | e:135 c:271 a:5 |
| refinery | Ore Refinery | GAREFN / NAREFN | Allied Ore Refinery / Soviet Ore Refinery | GAREFN / NAREFN | [GAREFN] Allied (Owner= all houses, Prerequisite has GACNST); [NAREFN] Soviet (Owner= all houses, Prerequisite has NACNST) | e:51 c:265 a:6 |
| ore_purifier | Ore Purifier | GAOREP | Allied Ore Processor | GAOREP | [GAOREP] Allied (Owner= all houses, Prerequisite has GACNST) | e:136 c:272 a:7 |
| nuclear_reactor | Nuclear Reactor | NANRCT | Nuclear Reactor | NANRCT | [NANRCT] Soviet (Owner= all houses, Prerequisite has NACNST) | e:137 c:273 a:8 |
| barracks | Barracks | GAPILE / NAHAND | Allied Barracks / Soviet Barracks | GAPILE / NAHAND | [GAPILE] Allied (Owner= all houses, Prerequisite has GACNST); [NAHAND] Soviet (Owner= all houses, Prerequisite has NACNST) | e:65 c:266 a:9 |
| war_factory | War Factory | GAWEAP / NAWEAP | Allied War Factory / Soviet War Factory | GAWEAP / NAWEAP | [GAWEAP] Allied (Owner= all houses, Prerequisite has GACNST); [NAWEAP] Soviet (Owner= all houses, Prerequisite has NACNST) | e:79 c:267 a:10 |
| naval_shipyard | Naval Shipyard | GAYARD / NAYARD | Allied Shipyard / Soviet Shipyard | GAYARD / NAYARD | [GAYARD] Allied (Owner= all houses, Prerequisite has GACNST); [NAYARD] Soviet (Owner= all houses, Prerequisite has NACNST) | e:138 c:274 a:11 |
| airforce_command | Airforce Command Headquarters | GAAIRC / AMRADR | Airforce Command Headquarters / American Airforce Command Headquarters | GAAIRC / GAAIRC | [GAAIRC] Allied (Owner= all houses, Prerequisite has GACNST), ForbiddenHouses=Americans; [AMRADR] Allied (Owner= all houses, Prerequisite has GACNST), RequiredHouses=Americans | e:139 c:275 a:12 |
| service_depot | Service Depot | GADEPT / NADEPT | Allied Service Depot / Soviet Service Depot | GADEPT / NADEPT | [GADEPT] Allied (Owner= all houses, Prerequisite has GACNST); [NADEPT] Soviet (Owner= all houses, Prerequisite has NACNST) | e:140 c:276 a:13 |
| tech_center | Battle Lab | GATECH / NATECH | Allied Battle Lab / Soviet Battle Lab | GATECH / NATECH | [GATECH] Allied (Owner= all houses, Prerequisite has GACNST); [NATECH] Soviet (Owner= all houses, Prerequisite has NACNST) | e:93 c:268 a:14 |
| radar_tower | Radar Tower | NARADR | Radar Tower | NARADR | [NARADR] Soviet (Owner= all houses, Prerequisite has NACNST) | e:141 c:277 a:15 |
| cloning_vats | Cloning Vats | NACLON | Cloning Vats | NACLON | [NACLON] Soviet (Owner= all houses, Prerequisite has NACNST) | e:142 c:278 a:16 |
| fortress_walls | Fortress Walls | GAWALL / NAWALL | Allied Wall / Soviet Wall | GAWALL / NAWALL | [GAWALL] Owner= all houses; [NAWALL] Owner= all houses | e:143 c:279 a:17 |
| pillbox | Pillbox | GAPILL | Pill Box | GAPILL | [GAPILL] Allied (Owner= all houses, Prerequisite has GACNST) | e:144 c:280 a:18 |
| sentry_gun | Sentry Gun | NALASR | Sentry Gun | NALASR | [NALASR] Soviet (Owner= all houses, Prerequisite has NACNST) | e:145 c:281 a:19 |
| patriot | Patriot Missile System | NASAM | Patriot Missile | NASAM | [NASAM] Allied (Owner= all houses, Prerequisite has GACNST) | e:146 c:282 a:20 |
| flak_cannon | Flak Cannon | NAFLAK | Flak Cannon | NAFLAK | [NAFLAK] Soviet (Owner= all houses, Prerequisite has NACNST) | e:147 c:283 a:21 |
| prism_tower | Prism Tower | ATESLA | Prism Cannon | GAPRIS | [ATESLA] Allied (Owner= all houses, Prerequisite has GACNST) | e:148 c:284 a:22 |
| tesla_coil | Tesla Coil | TESLA | Tesla Coil | NATSLA | [TESLA] Soviet (Owner= all houses, Prerequisite has NACNST) | e:149 c:285 a:23 |
| grand_cannon | Grand Cannon | GTGCAN | Grand Cannon | GTGCAN | [GTGCAN] Allied, RequiredHouses=French | e:150 c:286 a:24 |
| gap_generator | Gap Generator | GAGAP | Gap Generator | GAGAP | [GAGAP] Allied (Owner= all houses, Prerequisite has GACNST) | e:151 c:287 a:25 |
| spy_satellite | Spy Satellite Uplink | GASPYSAT | SpySat Uplink | GASPST | [GASPYSAT] Allied (Owner= all houses, Prerequisite has GACNST) | e:152 c:288 a:26 |
| psychic_sensor | Psychic Sensor | NAPSIS | Psychic Sensor | NAPSIS | [NAPSIS] Soviet (Owner= all houses, Prerequisite has NACNST) | e:153 c:289 a:27 |
| chronosphere | Chronosphere | GACSPH | Chrono Sphere | GACSPH | [GACSPH] Allied (Owner= all houses, Prerequisite has GACNST) | e:154 c:290 a:28 |
| weather_control | Weather Control Device | GAWEAT | Weather Controller | GAWETH | [GAWEAT] Allied (Owner= all houses, Prerequisite has GACNST) | e:155 c:291 a:29 |
| iron_curtain | Iron Curtain Device | NAIRON | Iron Curtain Device | NAIRON | [NAIRON] Soviet (Owner= all houses, Prerequisite has NACNST) | e:156 c:292 a:30 |
| nuclear_silo | Nuclear Missile Silo | NAMISL | Nuclear Missile Silo | NAMISL | [NAMISL] Soviet (Owner= all houses, Prerequisite has NACNST) | e:157 c:293 a:31 |

Prerequisite group tokens of the ini, as used in the `Prereq R` columns: `any power` = `POWER`
(`[General] PrerequisitePower=GAPOWR,NAPOWR,NANRCT`), `any barracks` = `BARRACKS`
(`[General] PrerequisiteBarracks=NAHAND,GAPILE`), `any radar` = `RADAR`
(`[General] PrerequisiteRadar=GAAIRC,NARADR,AMRADR`), `any battle lab` = `TECH`
(`[General] PrerequisiteTech=GATECH,NATECH`), `any refinery` = `PROC`
(`[General] PrerequisiteProc=GAREFN,NAREFN`); `[General] PrerequisiteFactory=GAWEAP,NAWEAP` exists but no
mapped section uses `FACTORY`.

Almost every RA2 building lists all nine houses in `Owner=`; the faction split comes from the
construction yard in `Prerequisite=` (`GACNST` or `NACNST`), from `ForbiddenHouses=` for the
engineer and dog pairs, and from `RequiredHouses=` for the country specials. `Alliance` is the
ini house name of Korea. The prism tank section `[SREF]` has its `Primary=` line commented out
and names its weapon with `Weapon1=Comet`.

Entries of the ini type lists that have no section in this rules.ini: `APACHE` (AircraftTypes), `NAWAST` (BuildingTypes), `GARADR` (BuildingTypes), `NAHPAD` (BuildingTypes), `CAEURO01` (BuildingTypes), `CAIRSFGL` (BuildingTypes).
None is needed by the mapping. Arsenal ids not in the mapping: none.

## 8. Appendix D: rule research (RA0 step 3)

Research of 2026-10-11 into the engine rules that `rules.ini` does not define. The source prefixes
and confidence labels are defined at its head; section 4 states which rule the game adopts.

Units: frame = one logic tick at the nominal 15 per second; game minute = 900 frames; cell = 256 leptons.
Conversion used below: `frames = trunc(minutes * 900)`. For a 20 tick/s simulation multiply frame counts by 20/15.

Confidence labels: `confirmed` (ini value, or two independent sources agree), `single source`, `assumption`
(no source for vanilla RA2; closest documented behaviour is given).

Source prefixes:

| Prefix | Expands to |
|---|---|
| `INI` | cached vanilla `rules.ini`, cited as `[Section] Key=value` |
| `ME/x` | https://modenc.renegadeprojects.com/x |
| `ARES/x` | https://ares-developers.github.io/Ares-docs/new/x |
| `YRPP/x` | https://github.com/Phobos-developers/YRpp/blob/master/x (reverse-engineered engine headers) |
| `PHOBOS/x` | https://github.com/Phobos-developers/Phobos/blob/develop/src/x (hooks that restate vanilla code) |
| `VINIFERA` | https://github.com/Vinifera-Developers/Vinifera/blob/master/src/extensions/scenario/scenarioext.cpp (Tiberian Sun engine reimplementation) |
| `RA1/x` | https://github.com/electronicarts/CnC_Red_Alert/blob/main/CODE/x (EA GPL source of the ancestor engine) |
| `MGW` | https://www.magicgameworld.com/?p=128534 |
| `MANUAL` | https://www.scribd.com/document/740449111/Command-Conquer-Red-Alert-2 (RA2 manual keyboard section, seen through search snippet only) |

Pages that did not load (nothing below relies on them): every `cnc.fandom.com` page tried (HTTP 402),
`strategywiki.org` RA2 controls (403), `speedrun.com` guide (403), `ME/LightningStorm`, `ME/Lightning_Storm`,
`ME/Spy` (404), `ARES/radiation.html` (404). ModEnc pages for `MinLowPowerProductionSpeed`,
`MaxLowPowerProductionSpeed`, `Storage`, `Harvester`, `Growth`, `Spread`, `Tiberiums`, `JumpjetSpeed`,
`Jumpjet`, `Radiation`, `RadLevel`, `Spawns`, `UnitCount` loaded but are stubs without the formula.

#### 1. Build time

Rule (frames):

```
base   = Cost * BuildSpeed * 900 / 1000            ; Cost = unmodified base Cost of the type
t      = base * difficulty.BuildTime               ; AI houses only, see topic 17
t      = t / powerSpeed                            ; only when the house is low on power, see topic 2
t      = t * MultipleFactory^(n-1)                 ; n = owned factories able to build this category
t      = t * WallBuildSpeedCoefficient             ; only when the BuildingType has Wall=yes
frames = trunc(t)
```

Production advances in 54 equal steps: one step every `frames/54` frames; each step charges
`remaining balance / remaining steps` credits; a step waits while the house cannot pay.

| Item | Vanilla value | Source | Confidence |
|---|---|---|---|
| Time per 1000 credits | `[General] BuildSpeed=.7` (630 frames per 1000 credits, 0.63 frame per credit) | INI, ME/BuildSpeed, ME/Cost | confirmed |
| Cost used | base `Cost`; cost multipliers never change build time | ME/Cost | single source |
| Payment | proportional to progress, 54 steps | ME/Cost, YRPP/FactoryClass.h (`Production; // hardcoded to be 54 steps`), RA1/FACTORY.H (`STEP_COUNT=54`) | confirmed |
| Multiple factories | `[General] MultipleFactory=0.8`; time factor 1, 0.8, 0.64, 0.512 for 1, 2, 3, 4 factories | INI comment, ME/MultipleFactory | confirmed |
| Cap on factories | none documented for RA2 (RA1 Aftermath capped at 2) | ME/MultipleFactory, RA1/TECHNO.CPP | assumption: no cap |
| Which buildings count | per category counters: conyards, barracks, war factories, shipyards, airpads | YRPP/HouseClass.h (`NumConYards`, `NumBarracks`, `NumWarFactories`, `NumShipyards`, `NumAirpads`) | single source |
| Walls | `[General] WallBuildSpeedCoefficient=3.0` multiplies time (walls 3 times slower) | INI comment, ME/WallBuildSpeedCoefficient | confirmed |
| Per type multiplier | `BuildTimeMultiplier` does not exist in vanilla RA2 (YR only) | INI (0 hits), ME/BuildTimeMultiplier | confirmed |
| `GameSpeedBias` | `[General] GameSpeedBias=1.6`; ModEnc: multiplies unit speed and BuildSpeed but is "obsolete in whole or in part" in RA2/YR | ME/GameSpeedBias | assumption: ignore it |
| Queue length | `[General] MaximumQueuedObjects=29` queued plus one in production (30 per slot); buildings are not queueable | INI, ME/MaximumQueuedObjects | single source |

Production slots (each builds one item at a time, all run in parallel):

| Slot | Sidebar tab | Factory building | Types | Source |
|---|---|---|---|---|
| Buildings | Q | `Factory=BuildingType` (`GACNST`, `NACNST`) | BuildingTypes with `BuildCat` other than `Combat` | INI, ME/Factory, ME/BuildCat |
| Defences | W | same construction yard | BuildingTypes with `BuildCat=Combat` (walls, turrets, superweapons, Gap, SpySat, Psychic Sensor) | INI, ME/BuildCat |
| Infantry | E | `Factory=InfantryType` (`GAPILE`, `NAHAND`) | InfantryTypes | INI |
| Vehicles | R | `Factory=UnitType`, `Naval` unset (`GAWEAP`, `NAWEAP`) | land VehicleTypes | INI, ME/Factory |
| Ships | R | `Factory=UnitType` + `Naval=yes` (`GAYARD`, `NAYARD`) | VehicleTypes with `Naval=yes` | INI, ME/Factory |
| Aircraft | R | `Factory=AircraftType` (`GAAIRC`, `AMRADR`) | AircraftTypes (`ORCA`, `BEAG`) | INI |

Six independent slots are `confirmed` by YRPP/HouseClass.h (`Primary_ForAircraft`, `Primary_ForInfantry`,
`Primary_ForVehicles`, `Primary_ForShips`, `Primary_ForBuildings`, `Primary_ForDefenses`) plus ME/BuildCat.

#### 2. Low power

Rule:

```
lowPower   = PowerDrain > PowerOutput              ; sums of Power<0 and Power>0 of owned buildings
ratio      = PowerOutput / PowerDrain              ; 1.0 when PowerDrain = 0, clamped to at most 1.0
powerSpeed = 1 - LowPowerPenaltyModifier * (1 - ratio)
powerSpeed = clamp(powerSpeed, MinLowPowerProductionSpeed, MaxLowPowerProductionSpeed)
buildTime  = buildTime / powerSpeed                ; applied only when lowPower
```

With vanilla values the build time factor is between 1/0.8 = 1.25 (any shortage up to 20%) and 1/0.5 = 2.0
(shortage of 50% or more).

| Item | Vanilla value | Source | Confidence |
|---|---|---|---|
| Keys | `[General] MinLowPowerProductionSpeed=.5`, `MaxLowPowerProductionSpeed=.8`, `LowPowerPenaltyModifier=1` | INI | confirmed |
| Penalty = modifier times shortage, clamped to [Min, Max] | ini comments ("2 means 30% short = 60% penalty", "applies after modifier") | INI, ME/LowPowerPenaltyModifier, ARES/buildtime.html | confirmed (structure) |
| Shortage = 1 - output/drain | not spelled out for RA2; RA1 uses `Power_Fraction()` then clamps to [1/2, 3/4] | YRPP/HouseClass.h (`GetPowerPercentage`), RA1/TECHNO.CPP `Time_To_Build` | assumption |
| Unused keys | `WorstLowPowerBuildRateCoefficient=.1`, `BestLowPowerBuildRateCoefficient=.2` are Tiberian Sun leftovers | INI | assumption: ignore |
| `Powered=yes` + `Power<0` | building stops working: weapons do not fire, active animations freeze | ME/Powered | single source |
| Vanilla `Powered=yes` buildings | `GAOREP`, `GAYARD`, `NAYARD`, `GASPYSAT`, `GAGAP`, `NAIRON`, `GAWEAT`, `GACSPH`, `NARADR`, `NAPSIS`, `TESLA`, `ATESLA`, `NALASR`, `NASAM`, `GTGCAN`, `NAFLAK` | INI | confirmed |
| Not `Powered` | `GAPILL` (always fires), `NAMISL`, `GAAIRC`, `AMRADR`, factories, refineries | INI | confirmed |
| Radar | minimap is lost when the `Radar=yes` building stops operating (low power) | ME/Radar | single source |
| Superweapons | `IsPowered=true` pauses the recharge timer while the owner is low on power, even if the building drains nothing; vanilla: nuke, Iron Curtain, storm, Chronosphere true; both paradrops false | INI, ME/IsPowered | confirmed |
| Power plants | a `Powered=yes` plant with positive `Power` would stop supplying; vanilla plants are not `Powered` (`NANRCT Powered=no`) | INI, ME/Powered | confirmed |
| Low power building damage | `[General] DamageDelay=1` is obsolete in RA2 (no damage) | ME/DamageDelay | single source |
| Spy blackout | output forced to 0 for `[General] SpyPowerBlackout=1000` frames | INI, ME/Agent | confirmed |
| Tesla Troopers | 2 chargers keep one `Overpowerable` coil online in low power (topic 14) | ME/ElectricAssault, PHOBOS/Ext/Building/Hooks.cpp | confirmed |

#### 3. Damage

Recommended pipeline, integer truncation at each `trunc`:

```
D0 = weapon.Damage * firepowerMult                 ; veteran FIREPOWER 1.1, crate 2.0, multiplied
R  = warhead.CellSpread * 256                      ; leptons, exact (not rounded)
d  = distance in leptons, object position to detonation point (3D)
if R == 0: only objects in the impact cell are hit, at full damage
if d > R : no damage
D1 = D0 * (1 - (1 - PercentAtMax) * d / R)         ; linear, PercentAtMax reached exactly at d = R
D2 = trunc(D1 * Verses[target.Armor])              ; then ProneDamage for crawling infantry
D3 = trunc(D2 / armorMult)                         ; veteran STRONGER 1.5, crate 1.5, multiplied
D  = min(D3, MaxDamage)
```

| Item | Rule / value | Source | Confidence |
|---|---|---|---|
| Armour order of `Verses` | none, flak, plate, light, medium, heavy, wood, steel, concrete, special_1, special_2 | ME/Verses, INI (11 values) | confirmed |
| Falloff | `M = 1 - (1 - PercentAtMax) * d / (CellSpread*256)` | ME/PercentAtMax, PHOBOS/Ext/WarheadType/Hooks.cpp (`1.0 - percentDifference * Distance / cellSpread`) | confirmed |
| Beyond CellSpread | no damage (`Distance > cellSpread` skipped) | ME/CellSpread, PHOBOS same file | confirmed |
| Cells scanned | `trunc(CellSpread + 0.99)` rings of a lookup table; maximum CellSpread 11 (larger crashes) | ME/CellSpread | single source |
| Direct hit | the hit object is at d near 0, so it takes full damage; no separate direct-hit bonus | ME/CellSpread, ME/PercentAtMax | single source |
| Infantry sub-cell | distance uses the infantry sub-cell position, not the cell centre | ME/CellSpread | single source |
| Buildings | each foundation cell inside the radius is damaged separately, so multi-cell buildings take several hits from one area warhead | PHOBOS same file (`MergeBuildingDamage` is the opt-in fix) | single source |
| Aircraft | hit by area damage only if the detonation is above ground; distance halved for V3/Dreadnought missiles in flight and for flyers above 208 leptons | ME/CellSpread | single source |
| Rounding | fractions are dropped; `0 < Damage*Verses < 1` deals 0 | ME/Damage | single source |
| Minimum | `[CombatDamage] MinDamage=1` (ini comment: "obsolete"); falloff of a non-zero base never yields less than 1; a Verses product below 1 truncates to 0 and stays 0 | INI, ME/Damage, ME/MinDamage, PHOBOS/Ext/WarheadType/Hooks.cpp (`receiveDamage = sgn(baseDamage)`), RA1/COMBAT.CPP `Modify_Damage` | single source per half; ME/MinDamage alone claims a clamp after Verses |
| Maximum | `[CombatDamage] MaxDamage=10000` per hit | INI, ME/MaxDamage | confirmed |
| `Verses=0%` | target is illegal: no attack or force-fire order, no retaliation, no auto-acquire | ME/Verses | single source |
| `Verses=1%` | never auto-acquired, but can be ordered and retaliated against; 2% has no targeting effect | ME/Verses | single source |
| Negative damage | heals; Verses and PercentAtMax are not applied; effective radius about 8 leptons | ME/Damage, RA1/COMBAT.CPP | confirmed |
| Veteran armour | damage divided by 1.5, fraction dropped (100 becomes 66) | ME/VeteranArmor, PHOBOS/Ext/Techno/Body.cpp | confirmed |
| `Burst` | N shots per volley; ROF timer starts after the last shot; gap between burst shots random 3 to 5 frames | ME/Burst, ME/ROF | single source |
| Excluded warheads | `Temporal`, `MindControl`, `ElectricAssault`, `MakesDisguise`, `Parasite` ignore CellSpread | ME/CellSpread | single source |

Scatter (`[CombatDamage] BallisticScatter=1.0` cells, `HomingScatter=2.0`):

| Projectile flags | Vanilla users | Scatter of the impact point | Source | Confidence |
|---|---|---|---|---|
| `Inaccurate=yes`, `Arcing=yes`, no `FlakScatter` | none in vanilla | random direction, distance BallisticScatter/2 to BallisticScatter | ME/BallisticScatter | single source |
| `Inaccurate`, `Arcing`, `FlakScatter`, visible | `[FlakTProj]` (Flak Track and Flak Trooper ground guns) | 0 to `BallisticScatter * distance / Range` | ME/BallisticScatter | single source |
| `FlakScatter=yes`, `Inviso=yes` | `[FlakProj]` (Flak Cannon, Flak Track AA, Flak Trooper AA) | radius = random 0 to `2 * BallisticScatter`, times `distance / Range`, random angle | ME/FlakScatter | single source |
| `Inaccurate=yes` | both flak projectiles | never snaps to the target, so damage relies on CellSpread (FlakWH 1.0, 10% at edge) | ME/Inaccurate | single source |

#### 4. Rate of fire and range

| Item | Rule / value | Source | Confidence |
|---|---|---|---|
| `ROF` unit | frames between the last shot of a volley and the next volley | ME/ROF | confirmed (ini values are frame counts) |
| Rearm delay | `trunc(ROF * VeteranROF(0.6 if ROF ability) * house ROF mult) + random(0..2)` | ME/ROF | single source |
| Garrison | rearm divided by the number of occupants | ME/ROF | single source |
| No weapon / ammo building | delay 1 | ME/ROF | single source |
| `Range` unit | cells, fractional allowed; `Range * 256` leptons | ME/Range, INI | confirmed |
| Measurement | straight-line lepton distance between object coordinates; 2D when the shooter is on the ground, 3D when airborne | ME/Range | single source |
| Buildings as targets | arcing projectiles measure to the building centre and ignore the foundation | ME/Range | single source |
| Approach rule | for Range above 2 cells the mover subtracts 128 leptons before the check, so it closes half a cell further than the limit | ME/Range | single source |
| `Range=-2` | unlimited, skips MinimumRange and wall checks (`MakeupKit`, `ASWLauncher`) | ME/Range, INI | confirmed |
| `CellRangefinding=yes` | melee style weapons (`Sapper`, `IvanBomber`, dog bites, `SquidGrab`, `BlimpBomb`, `DefuseKit`) measure in whole cells | INI (flag), no page | assumption: Chebyshev cell distance |
| `MinimumRange` | cells; a mobile unit closer than this keeps backing away; vanilla: `RPGTower` 2, `HoverMissile` 1, `HowitzerGun` 2, `V3Launcher` 5, `DredLauncher` 8, `GrandCannonWeapon` 3 | INI, ME/MinimumRange | confirmed |
| `AirRangeBonus` | cells added to weapons whose projectile has `AA=yes` when the target is airborne; only `[FV] AirRangeBonus=4`; the unit still approaches to plain Range | INI, ME/AirRangeBonus | confirmed |
| Elevation | `[ElevationModel] ElevationIncrement=4`, `ElevationIncrementBonus=2`, `ElevationBonusCap=2` for `SubjectToElevation` projectiles | INI, ME/Range | confirmed (ignore) |
| Targeting scan period | `[General] NormalTargetingDelay=27`, `GuardAreaTargetingDelay=36` frames | INI | confirmed |

#### 5. Speed

Rule for ground, ship and winged aircraft locomotors:

```
leptonsPerFrame = min(trunc(min(Speed, 100) * 256 / 100), 255)
cellsPerSecond  = leptonsPerFrame * 15 / 256           ; at 15 frames per second
effective       = leptonsPerFrame * landType[SpeedType] * veteran FASTER (1.2) * crate (1.2)
```

| `Speed=` | leptons/frame | cells/s at 15 fps | Vanilla users |
|---|---|---|---|
| 4 | 10 | 0.586 | GI, Conscript, most infantry, Apocalypse, V3, miners, MCV, sub, Aegis, carrier, dreadnought |
| 5 | 12 | 0.703 | Tanya, Chrono Legionnaire, Tank Destroyer, Demolition Truck, Kirov (`Speed` unused, see below) |
| 6 | 15 | 0.879 | Rhino, Tesla Tank, Terrorist, destroyer, hover transports |
| 7 | 17 | 0.996 | Grizzly, Mirage |
| 8 | 20 | 1.172 | dogs, Flak Track, dolphin, squid, Sea Scorpion |
| 10 | 25 | 1.465 | IFV, Terror Drone |
| 12 | 30 | 1.758 | Hornet, Osprey |
| 14 | 35 | 2.051 | Harrier, Black Eagle |
| 15 / 20 | 38 / 51 | 2.227 / 2.988 | V3 rocket / Dreadnought missile (cruise, after launch acceleration) |

| Item | Rule / value | Source | Confidence |
|---|---|---|---|
| Conversion formula | as above; the result is leptons per frame (RA1: "A speed of 256 leptons per tick would be stored as 100") | ME/Speed, RA1/CCINI.CPP `Get_MPHType`, RA1/DRIVE.CPP | confirmed |
| Infantry | ModEnc: hard-coded factor 3/2 while walking, 2/3 while crawling (prone) | ME/Speed | single source |
| Damaged vehicles | Drive and Ship locomotors run at 3/4 speed when health ratio <= `ConditionYellow` (50%) | ME/Speed ("three-quarters when crippled"), RA1/DRIVE.CPP (`Health_Ratio() <= Rule.ConditionYellow`, `speed -= speed/4`) | confirmed |
| Land multipliers, Foot/Track/Wheel | `[Clear]`, `[Rough]`, `[Road]` 100/100/100; `[Tiberium]` (ore) 90/70/50; `[Railroad]` 90/100/50; `[Ice]` 50/80/50; `[Weeds]` 50/70/50; `[Water]`, `[Rock]`, `[Wall]`, `[Beach]` 0 | INI | confirmed |
| Float | `[Water] Float=100%`, everything else 0 | INI | confirmed |
| Hover (`LCRF`, `SAPC`) | `[Clear] Hover=50%`, `[Road] 75%`, `[Water] 100%`, `[Beach] 75%`; `[General] HoverBoost=150%` on straights | INI | confirmed (values), assumption (how Hover locomotor combines them) |
| Amphibious (Tanya) | `[Clear] 80%`, `[Road] 100%`, `[Water] 100%`, `[Beach] 60%`, `[Tiberium] 50%` | INI | confirmed |
| Winged | 100% everywhere | ME/SpeedType | single source |
| Default SpeedType | vehicles: `Track` if `Crusher=yes` else `Wheel`; infantry `Foot`; aircraft `Winged` | ME/SpeedType | single source |
| Jumpjet locomotor | uses `JumpjetSpeed`, not `Speed`: Rocketeer 30, Night Hawk 30, Kirov 5; default `[JumpjetControls] Speed=14`, `Acceleration=2`, `Climb=5`, `CruiseHeight=500` | INI, ME/Speed, ME/JumpjetSpeed | confirmed (which key) |
| JumpjetSpeed unit | not documented | none | assumption: leptons per frame directly (30 gives 1.758 cells/s, 5 gives 0.293 cells/s), reached at `JumpJetAccel` leptons/frame per frame |
| `GameSpeedBias=1.6` | see topic 1; not applied | ME/GameSpeedBias | assumption |
| Slopes | `TrackedUphill=1.0`, `TrackedDownhill=1.2`, `WheeledUphill=1.0`, `WheeledDownhill=1.2` | INI | confirmed (ignore on flat maps) |
| Projectile `Speed` | top speed of the projectile, leptons per frame scale; `ROT>0` only | ME/Speed | single source |

#### 6. Veterancy

```
xp += victim.Cost / (own.Cost * VeteranRatio)       ; float, capped at VeteranCap
veteran when xp >= 1.0, elite when xp >= 2.0        ; i.e. kills worth 3x and 6x own Cost
```

| Item | Rule / value | Source | Confidence |
|---|---|---|---|
| Keys | `[General] VeteranRatio=3.0`, `VeteranCap=2`, `InitialVeteran=no` | INI | confirmed |
| Formula | as above | YRPP/TechnoClass.h (`VeterancyStruct::Add`), ME/VeteranRatio, ME/Trainable | confirmed |
| Threshold edge | YRPP uses `>=`; ModEnc says strictly more (GI at 200 needs 601) | YRPP, ME/VeteranRatio | single source each: use `>=` |
| Cost used | base `Cost` of killer type and of victim type | ME/VeteranRatio | single source |
| Kills that count | enemy, neutral and civilian objects; not `Insignificant=yes` or `DontScore=yes`; own objects only via Temporal | ME/VeteranRatio | single source |
| Multipliers | `VeteranCombat=1.1` (damage), `VeteranSpeed=1.2`, `VeteranArmor=1.5` (damage divided), `VeteranROF=0.6` (ROF delay multiplied), `VeteranSight=0.0` | INI | confirmed |
| `VeteranSight=0.0` | ModEnc formula is Sight times VeteranSight capped at 10; value 0.0 yields no visible change in play | ME/VeteranSight | assumption: SIGHT gives no bonus in vanilla |
| Stacking | none: a token present in both lists applies once; veteran tokens persist at elite; elite adds only new tokens | ME/EliteAbilities, ME/VeteranArmor, ME/VeteranSight | confirmed |
| Elite weapons | `ElitePrimary`/`EliteSecondary` replace the weapons at elite; FIREPOWER and ROF apply to them | ME/EliteAbilities, INI | confirmed |
| Buildings | `Trainable` defaults to no for buildings, yes for infantry, vehicles, aircraft | ME/Trainable | single source |
| `Trainable=no` in vanilla | Engineer, dogs, Crazy Ivan, Spy, Terrorist, Chrono Ivan, Demolition Truck, Terror Drone, Chrono Miner, MCVs, hover transports | INI | confirmed |
| Missile spawns | V3 and Dreadnought missile kills credit the launcher; carrier Hornets likewise spawn-owned | ME/MissileSpawn | single source |
| Garrison | occupants gain experience from the building's kills, promoted on exit | ME/CanBeOccupied | single source |
| Forced promotion | veteran crate (+1 level), spy in barracks or war factory (new units veteran), Armory (`CAARMR Ammo=5`, promotes entering infantry to elite, 5 uses) | INI, ME/Powerups, ME/Agent, ME/Armory | confirmed |

Tokens (vanilla lists only use the first seven):

| Token | Effect | Works in RA2 | Source |
|---|---|---|---|
| STRONGER | incoming damage divided by `VeteranArmor` | yes | ME/VeteranAbilities |
| FIREPOWER | weapon damage times `VeteranCombat` | yes | ME/VeteranAbilities |
| ROF | weapon ROF times `VeteranROF` | yes | ME/VeteranAbilities |
| SIGHT | sight times `VeteranSight` | yes (no effect at 0.0) | ME/VeteranAbilities |
| FASTER | speed times `VeteranSpeed` | yes | ME/VeteranAbilities |
| SELF_HEAL | grants `SelfHealing=yes` | yes | ME/VeteranAbilities |
| SCATTER | auto-scatter when damaged, attacked from the air, or threatened by crushers and slow projectiles; elite units always scatter | yes | ME/VeteranAbilities |
| CLOAK | grants `Cloakable=yes` | yes (unused) | ME/VeteranAbilities |
| EXPLODES | grants `Explodes=yes` | yes (unused) | ME/VeteranAbilities |
| RADAR_INVISIBLE | grants `RadarInvisible=yes` | yes (unused) | ME/VeteranAbilities |
| SENSORS | grants `Sensors=yes` | yes (unused) | ME/VeteranAbilities |
| FEARLESS | grants `Fearless=yes` | yes (unused) | ME/VeteranAbilities |
| GUARD_AREA | grants `DefaultToGuardArea=yes` | yes (unused) | ME/VeteranAbilities |
| CRUSHER | grants `Crusher=yes` | yes (unused) | ME/VeteranAbilities |
| C4 | shows the C4 cursor but never plants (Tiberian Sun only) | no | ME/VeteranAbilities |
| TIBERIUM_HEAL, TIBERIUM_PROOF, VEIN_PROOF | listed as invalid or ineffective in RA2 | no | ME/VeteranAbilities |

Vanilla lists: 43 types have `EliteAbilities=SELF_HEAL,STRONGER,FIREPOWER,ROF`; `VeteranAbilities` is
`STRONGER,FIREPOWER,ROF,SIGHT,FASTER` (infantry, ships, Apocalypse, V3) or `STRONGER,FIREPOWER,SIGHT,FASTER`
(most vehicles and aircraft, Sniper); Tanya and SEAL add SCATTER; Harrier, Black Eagle and squid lack SELF_HEAL.

Self heal:

| Item | Rule / value | Source | Confidence |
|---|---|---|---|
| Rate | +1 hit point (hard-coded) every `[General] RepairRate=.016` minutes = 14 frames | ME/SelfHealing, ME/RepairRate | single source |
| Ceiling | full health in RA2/YR (the `ConditionYellow` ceiling is Tiberian Sun only) | ME/SelfHealing | single source |
| Innate `SelfHealing=yes` | Desolator, Apocalypse, squid, both miners, Kirov | INI | confirmed |
| `IRepairRate=.001`, `IRepairStep=20` | interval and step for infantry healed inside a `Hospital=yes` building | INI, ME/IRepairRate | single source |
| `URepairRate=.016` | interval for vehicles on a `UnitRepair=yes` building, step is `RepairStep` | INI, ME/URepairRate, ME/RepairStep | confirmed |
| `URepairStep` | not present in vanilla RA2 | INI (0 hits) | confirmed |
| `TiberiumHeal=.010` | no effect in RA2 | ME/RepairStep | single source |

#### 7. Crew and survivors

| Item | Rule / value | Source | Confidence |
|---|---|---|---|
| Vehicle crew | when a `Crewed=yes` vehicle dies, with probability `[General] CrewEscape=50%` one crew infantry appears | INI, ME/CrewEscape, ME/Crew_System | confirmed |
| No crew when | vehicle has `Explodes=yes`, died to its own `Suicide=yes` weapon, or was moving when destroyed | ME/CrewEscape, ME/Crew_System, ME/Crewed | single source |
| Crewed vehicles in vanilla | only `AMCV`, `SMCV` (and `TRUCKA`); all tanks default to `Crewed=no` | INI | confirmed |
| Aircraft | `ORCA`, `BEAG`, `SHAD` are `Crewed=yes` but aircraft never spawn survivors after RA1 (`Pilot=E1` unused) | INI, ME/Crew_System | single source |
| Crew type | by side of the owner: `[General] AlliedCrew=E1`, `SovietCrew=E2` | INI, ME/SurvivorRate | confirmed |
| Building survivors | `n = ceil(Cost * SurvivorRate / divisor)` infantry of the crew type, for `Crewed=yes` buildings sold or destroyed | ME/SurvivorRate, ME/Crew_System | single source |
| Vanilla values | `SurvivorRate=.4`, `AlliedSurvivorDivisor=500`, `SovietSurvivorDivisor=250` | INI | confirmed |
| Conflict | ini comments say SurvivorRate "is no longer used" and survivors are "sell cost / divisor" (sell cost = Cost * 50%) | INI comment | assumption: use `ceil(Cost * RefundPercent / divisor)`, minimum 1; differs from the ModEnc formula only by 0.5 versus 0.4 |
| Limit | survivors cannot exceed the free foundation cells | RA1/BUILDING.CPP (`count` bounded by cell list) | assumption |
| Engineer | 25% chance of one `[General] Engineer=ENGINEER` from a destroyed or sold `Factory=BuildingType` building; always this type for both sides | ME/Engineer, ME/CrewEscape, RA1/BUILDING.CPP (`Percent_Chance(25)`) | confirmed |
| Technician | 15% chance that a survivor is `[General] Technician=CTECH`; RA1 applies it to objects without a primary weapon | ME/CrewEscape, RA1/TECHNO.CPP (`Percent_Chance(15)`) | single source (condition differs between sources) |
| `SurvivorDivisor` | does not exist in vanilla RA2 (only the Allied and Soviet variants) | INI (0 hits) | confirmed |
| Uncrewed buildings | `Crewed=no`: base defences, `GAYARD`, `NAYARD`, `GAGAP`; walls have no key | INI | confirmed |

#### 8. Ore

| Item | Rule / value | Source | Confidence |
|---|---|---|---|
| Types | `[Tiberiums] 0=Riparius` (ore), `1=Cruentus` (gems), `2=Vinifera`, `3=Aboreus` (unused) | INI, ME/Tiberiums | confirmed |
| Bail value | `[Riparius] Value=25`, `[Cruentus] Value=50`, `[Vinifera] Value=25`, `[Aboreus] Value=25` | INI | confirmed |
| Miner capacity | `[HARV] Storage=40` bails (1000 credits ore, 2000 gems); `[CMIN] Storage=20` (500, 1000) | INI, web search summary of the C&C Wiki War Miner page | confirmed |
| Refinery | `[GAREFN] Storage=200`, `[NAREFN] Storage=200`: storage logic is disabled in RA2, credits are added on unload | INI, ME/PurifierBonus | single source |
| Density stages | 12 per ore cell (overlay data 0 to 11); a cell at stage s holds s+1 bails (25 to 300 credits ore, 50 to 600 gems) | ME/Tiberiums (20-overlay ore set, 12-overlay gem set), RA1/CELL.CPP (`OverlayData >= 11`) | assumption |
| Harvest step | 1 bail removed per harvest cycle; cell cleared when it reaches zero | RA1/UNIT.CPP `Harvesting` (`reducer = 1`) | assumption |
| Time per bail | not documented; `HarvesterLoadRate` and `HarvesterDumpRate` do not exist in vanilla RA2 | INI (0 hits) | assumption: one bail per `[Harvest] Rate=.016` minute tick = 14 frames (War Miner fill 560 frames, Chrono Miner 280) |
| Unload | not documented | none | assumption: 1 bail per 14 frames, credited per bail |
| Scan radius | `[AI] TiberiumNearScan=6`, `TiberiumFarScan=48` cells | INI | confirmed |
| Growth | every `Growth` frames each ore cell of the type rolls `GrowthPercentage`; success raises density by 1 up to stage 11 | ME/Growth, ME/GrowthPercentage, RA1/CELL.CPP `Grow_Tiberium` | single source |
| Spread | every `Spread` frames each ore cell rolls `SpreadPercentage`; success places stage 0 ore in one random free adjacent cell (8 neighbours) | ME/Spread, ME/SpreadPercentage, RA1/CELL.CPP `Spread_Tiberium` | single source |
| Spread threshold | not documented for RA2; RA1 requires density above stage 6 | RA1/CELL.CPP `Can_Tiberium_Spread` | assumption |
| Ore values | `[Riparius] Growth=2200`, `GrowthPercentage=.06`, `Spread=2200`, `SpreadPercentage=.06` | INI | confirmed |
| Gem values | `[Cruentus] Growth=10000`, `GrowthPercentage=0`, `Spread=10000`, `SpreadPercentage=0`: gems never grow or spread | INI | confirmed |
| Switches | `[General] TiberiumGrows=yes`, `TiberiumSpreads=yes`; lobby `[MultiplayerDialogSettings] TiberiumGrows=yes`; the map `[SpecialFlags]` copy is the one the engine uses | INI, ME/TiberiumSpreads | confirmed |
| `GrowthRate=5` | read but unused since Tiberian Sun | ME/GrowthRate | single source |
| Ore drill | terrain objects `TIBTRE01..03` have `SpawnsTiberium=yes`, `IsAnimated=yes`, `AnimationRate=3`, `AnimationProbability=.003`; each animation spawns ore of type 0 next to the drill | INI, ME/SpawnsTiberium | single source |
| Drill period | animation starts with probability 0.003 per frame (mean 333 frames); one bail placed or grown in a random adjacent cell per animation | none for the exact amount | assumption |
| Purifier | each owned `OrePurifier=yes` building adds `[General] PurifierBonus=.25` of the load value on every unload (additive); `GAOREP BuildLimit=1` | INI, ME/PurifierBonus | confirmed |
| AI bonus | `[General] AIVirtualPurifiers=4,2,0` (hard, medium, easy): +100%, +50%, +0% on every AI unload in skirmish | INI, ME/AIVirtualPurifiers, ME/AI | confirmed |
| Chrono Miner | `Teleporter=yes` + `Harvester=yes`: teleports back to the refinery with no chrono delay | INI, ME/Harvester | single source |
| War Miner gun | `20mmRapid` Damage 30, ROF 20, Range 5.5, warhead `HARVWH` | INI | confirmed |

#### 9. Repair and sell

| Item | Rule / value | Source | Confidence |
|---|---|---|---|
| Sell refund | `refund = Cost * RefundPercent` for human houses; AI houses receive full Cost | `[General] RefundPercent=50%`, ME/Soylent, RA1/TECHNO.CPP `Refund_Amount` | confirmed |
| `Soylent` | when non-zero it replaces the computed refund (47 vanilla sections set it) | INI, ME/Soylent | single source |
| Health scaling | none documented | ME/Soylent | assumption: none |
| Unsellable | `Unsellable=yes` on tech buildings (`CAOILD`, `CAAIRP`, `CAOUTP`, `CATHOSP`) | INI | confirmed |
| Sell time | build-up animation reversed; `[General] BuildupTime=.06` minutes = 54 frames average | INI | single source |
| Building repair | every `RepairRate=.016` minutes (14 frames) add `RepairStep=8` hit points and charge the step cost | INI, ME/RepairStep, ME/RepairRate | confirmed |
| Step cost | `max(1, trunc(Cost / max(Strength div RepairStep, 1) * RepairPercent))` with integer division | `[General] RepairPercent=15%`, ME/RepairPercent, PHOBOS/Misc/Hooks.BugFixes.cpp (`TechnoTypeClass_GetRepairStepCost`) | confirmed |
| Example | Construction Yard: 1000 div 8 = 125; 3000/125 = 24; 24 * 0.15 = 3 credits per 8 hit points | derived | confirmed |
| Stops when | credits run out; AI will not start below `[AI] CreditReserve=100` | INI | single source |
| `Repairable=false` | walls, fences, `CAMISC01/02` | INI | confirmed |
| Service Depot | `UnitRepair=yes` on `GADEPT`, `NADEPT`, `GAYARD`, `NAYARD`, `CAOUTP`; every `URepairRate=.016` minutes (14 frames) heal `RepairStep=8` at the same step cost formula using the unit's Cost and Strength | INI, ME/UnitRepair, ME/URepairRate, ME/RepairPercent | confirmed |
| Depot extras | removes a Terror Drone inside the vehicle; docked vehicles can be sold in RA2 | ME/Parasite, ME/UnitRepair | single source |
| Aircraft reload | on a `UnitReload=yes` pad (`GAAIRC`, `AMRADR`, `NumberOfDocks=4`) one ammo point per `[General] ReloadRate=.3` minutes = 270 frames; health is restored only after ammo is full | INI, ME/ReloadRate | confirmed |
| Engineer repair | an engineer entering an own damaged building restores it fully and is consumed | ME/Engineer | single source |

#### 10. Crates

`[Powerups]` line format: `Name=share,animation,waterAllowed,argument`. Probability = share / 110 in vanilla.

| Crate | Share | Argument | Effect in RA2 | Source | Confidence |
|---|---|---|---|---|---|
| Money | 20 | 2000 | credits = 2000 + random 0 to 900 | INI, ME/Powerups | confirmed (share), single source (random part) |
| Unit | 20 | none | one free vehicle; `[CrateRules] UnitCrateType=none` picks randomly among `CrateGoodie=yes` types; not on water | INI, ME/Powerups | single source |
| Veteran | 20 | 1 | +1 veterancy level to units within `CrateRadius`; Money if already elite | INI, ME/Powerups | single source |
| HealBase | 10 | none | all the collector's objects to full health | INI, ME/Powerups | single source |
| Armor | 10 | 1.5 | armour multiplier 1.5 (damage divided) on units within `CrateRadius`; Money if the collector already has one | INI, ME/Powerups | single source |
| Firepower | 10 | 2.0 | damage multiplier 2.0, same radius and substitution | INI, ME/Powerups | single source |
| Speed | 10 | 1.2 | speed multiplier 1.2, same radius and substitution | INI, ME/Powerups | single source |
| Reveal | 10 | none | reveals the whole map except under enemy Gap Generators | INI, ME/Powerups | single source |
| Invulnerability, IonStorm, Gas, Tiberium, Pod, Cloak, Darkness, Explosion, ICBM, Napalm, Squad | 0 | 1.0, -, 100, -, -, -, -, 500, -, 600, - | never spawn in vanilla (share 0) | INI | confirmed |

| `[CrateRules]` key | Value | Meaning | Source | Confidence |
|---|---|---|---|---|
| `CrateMaximum` | 255 | hard cap on crates | INI | confirmed |
| `CrateMinimum` | 1 | minimum crates kept on the map (player count no longer considered since Tiberian Sun) | INI, ME/CrateMinimum | single source |
| `CrateRadius` | 3.0 | cells, area for Armor, Firepower, Speed, Veteran, Cloak | INI | confirmed |
| `CrateRegen` | 3 | minutes; each respawn waits a random time between value/2 and value*2 (1350 to 5400 frames) | INI, ME/CrateRegen | single source |
| `UnitCrateType` | none | fixed unit type or random | INI | confirmed |
| `FreeMCV` | yes | multiplayer with bases: collector with no buildings, no `BaseUnit` and more than 1500 credits gets its side's MCV (engine behaves as always yes); a water crate still becomes Money | INI, ME/FreeMCV | single source |
| `SilverCrate`, `WoodCrate`, `WaterCrate`, `SoloCrateMoney` | HealBase, Money, Money, 5000 | campaign only | INI | confirmed |
| `HealCrateSound`, `CrateImg`, `WoodCrateImg`, `WaterCrateImg` | HealCrate, CRATE, CRATE, WCRATE | presentation | INI | confirmed |
| Lobby switch | `[MultiplayerDialogSettings] Crates=yes` | default on | INI | confirmed |

#### 11. Radar and vision

| Item | Rule / value | Source | Confidence |
|---|---|---|---|
| Radar available | owner has an operating `Radar=yes` building: `GAAIRC`, `AMRADR`, `NARADR`, `GASPYSAT`; lost while low on power | INI, ME/Radar | single source (power rule) |
| Radar jam | Lightning Storm disables enemy radar for its duration; spy in a radar building resets the victim's shroud | ME/LightningStormDuration, ME/Spyable | single source |
| `SpySat=yes` | `GASPYSAT` reveals the whole map to the owner except Gap Generator areas; does not need power; grants radar only because the building also has `Radar=yes` | INI, ME/SpySat | single source |
| Gap Generator | `[GAGAP] GapGenerator=yes`, `GapRadiusInCells=10`, `SuperGapRadiusInCells=10`, `Power=-100`, `ExtraPower=-9000`, `Powered=true`; re-shrouds the radius for non-allied human players even against SpySat; AI is unaffected | INI, ME/GapGenerator | single source |
| Psychic Sensor | `[NAPSIS] PsychicDetectionRadius=15` cells, `Powered=true`; draws lines to enemy units ordered to attack inside the radius and marks incoming nukes | INI, ME/PsychicDetectionRadius | single source |
| `Sensors=yes` | uncloaks cloaked or submerged objects in the 8 adjacent cells | ME/Sensors | single source |
| `SensorsSight` | radius in cells in which enemy cloaked units become visible and targetable for the owner: `SUB` 4, `DLPH` 4, `SQD` 4, `DEST` 8, `CDEST` 8, `NAPSIS` 15 (buildings ignore it without Ares) | INI, ME/SensorsSight | single source |
| Airborne detectors | flying units cannot detect cloaked units | ME/Sensors | single source |
| Disguise detection | `DetectDisguise=yes`: `DOG`, `ADOG`, `YURI`, `PTROOP`, `NAPSIS` (`DetectDisguiseRange=15`), `NAPSYB`; explicitly no: `GAPILL`, `NALASR`, `GHOST` | INI, ME/DetectDisguise | confirmed (list) |
| AI versus Mirage | `[General] DisabledDisguiseDetectionPercent=15,5,2` (hard, medium, easy) chance per unit to see through | INI | confirmed |
| Cloak | `Cloakable=yes` on `SUB`, `DLPH`, `SQD` only; `CloakingSpeed` 1, 1, 5 frames per stage; `[General] CloakingStages=9` (9 or 45 frames); `CloakDelay=.02` minutes = 18 frames forced on the surface | INI, ME/CloakingSpeed, ME/CloakDelay | confirmed |
| Sight | cells, at most 10 (11 in the lookup table); `VeteranSight` capped at 10 | ME/Sight, ME/VeteranSight | single source |
| Aircraft sight | `[General] AttackingAircraftSightRange=2`, `AircraftFogReveal=6` | INI | confirmed |
| Fog of war | `[General] FogOfWar=no`, lobby `FogOfWar=no`, `Shroud=yes`, `ShadowGrow=no` | INI | confirmed |
| `Visceroids`, `Meteorites`, `IonStorms` | all `no` | INI | confirmed |

#### 12. Superweapons

Vanilla `[SuperWeaponTypes]`: MultiSpecial, IronCurtainSpecial, LightningStormSpecial, ChronoSphereSpecial,
ChronoWarpSpecial, ParaDropSpecial, AmericanParaDropSpecial. SpyPlane, PsychicReveal, ForceShield,
GeneticConverter and PsychicDominator are YR only (`ME/Type`). `RechargeTime` is minutes (`ME/RechargeTime`).

| Superweapon | Building | RechargeTime | Frames | IsPowered | ShowTimer |
|---|---|---|---|---|---|
| Nuclear missile `[MultiSpecial]` | `NAMISL` | 10 | 9000 | true | yes |
| Lightning Storm `[LightningStormSpecial]` | `GAWEAT` | 10 | 9000 | true | yes |
| Iron Curtain `[IronCurtainSpecial]` | `NAIRON` | 5 | 4500 | true | yes |
| Chronosphere `[ChronoSphereSpecial]` | `GACSPH` | 7 | 6300 | true | yes |
| Chrono warp `[ChronoWarpSpecial]` | `GADUMY` (second click, `PreDependent=ChronoSphere`) | 1 | 900 | false | no |
| Paradrop `[ParaDropSpecial]` | `CAAIRP` tech airport | 4 | 3600 | false | no |
| American paradrop `[AmericanParaDropSpecial]` | `AMRADR` | 4 | 3600 | false | no |

All four major weapons: `BuildLimit=1`, cost 5000 (nuke, storm) or 2500 (curtain, sphere), `Power=-200`,
`TechLevel=10`, `DisableableFromShell=yes`.

Lightning Storm:

| Item | Rule / value | Source | Confidence |
|---|---|---|---|
| Delay | `[General] LightningDeferment=250` frames between firing and the first bolt; warning text shown at once | INI, ME/LightningDeferment | confirmed |
| Duration | `LightningStormDuration=180` frames | INI | confirmed |
| Direct bolts | one bolt on the target cell every `LightningHitDelay=10` frames (18 in total), ignoring separation | INI, ARES/superweapons/types/lightningstorm.html | confirmed |
| Random bolts | one bolt every `LightningScatterDelay=5` frames (36 in total) on a random cell of the area | INI, ARES same page | confirmed |
| Area | `LightningCellSpread=10`; ini comment "n by n square", Ares treats the value as a diameter, ModEnc calls it a radius | INI, ARES, ME/LightningCellSpread | assumption: 10 by 10 cell square centred on the target |
| Separation | random bolts keep `LightningSeparation=3` cells of Manhattan distance from other active clouds | INI, ARES | confirmed |
| Damage | `LightningDamage=250` per bolt, `LightningWarhead=IonWH` (`CellSpread=2.0`, `PercentAtMax=.5`, Verses 100% except concrete 3%) | INI | confirmed |
| Radar | enemy radar is off for the storm duration | ME/LightningStormDuration, ARES | confirmed |
| Exclusivity | no other Lightning Storm can be fired by anyone while one is active | ME/LightningStormDuration | single source |
| Friendly fire | bolts damage everything in range including the owner's units | no flag restricts it | assumption |

Nuclear missile:

| Item | Rule / value | Source | Confidence |
|---|---|---|---|
| Launch | `[MultiSpecial] WeaponType=NukeCarrier` (`Projectile=GiantNukeUp`, `Speed=100`, warhead `NukeMaker`) rises from the silo, then the payload descends on the target cell | INI, ARES/superweapons/types/multimissile.html | confirmed |
| Payload | `[NukePayload] Damage=600`, `Warhead=NUKE`, `Projectile=GiantNukeDown`, `Speed=10`, `RadLevel=500` | INI | confirmed |
| Warhead | `[NUKE] CellSpread=10`, `PercentAtMax=.02`, Verses 100,100,100,200,100,100,60,100,8,100,100, `WallAbsoluteDestroyer=yes` | INI | confirmed |
| Unused keys | `[CombatDamage] AtomDamage=1000`, `[SpecialWeapons] NukeWarhead=Nuke`, `NukeDown=NukeDown`, `NukeProjectile=NukeUp`; there is no `[NukeDown]` weapon section | INI, ARES (payload weapon supplies damage) | single source |
| Radiation | 500 at the centre over 10 cells, see topic 13 | INI | confirmed |
| Warning | psychic sensor mark `PSIWARN` at the target if inside a sensor radius | ME/PsychicDetectionRadius, ARES | confirmed |
| Flight time | not documented (`GiantNukeUp` and `GiantNukeDown` have `Acceleration=1`, `[General] FlightLevel=1500`) | INI | assumption: pick a fixed launch-to-impact delay |

Iron Curtain and Chronosphere:

| Item | Rule / value | Source | Confidence |
|---|---|---|---|
| Curtain duration | `[CombatDamage] IronCurtainDuration=750` frames of invulnerability | INI, ME/IronCurtainDuration | confirmed |
| Curtain area | 3 by 3 cells centred on the target | ARES/superweapons/types/protect.html (`SW.Range` default 3,3) | single source |
| Curtain targets | vehicles and buildings of any owner; infantry in the area are killed | gameplay knowledge; the wiki page failed to load | assumption |
| Chrono area | 3 by 3 cells at the source, same offsets at the destination | ARES/superweapons/types/chronosphere.html (`SW.Range` default 3,3) | single source |
| Chrono targets | vehicles of all houses are moved; infantry in the area are killed (`KillOrganic` default yes); buildings are untouched; units with `Teleporter=yes` are moved, not killed | ARES same page | single source |
| Chrono freeze | moved units are static for 60 frames (hard-coded; `[General] ChronoDelay=60` "seems to do nothing") | ME/ChronoDelay, INI | single source |
| Bad terrain | a land vehicle placed on water, or a ship on land, is destroyed | gameplay knowledge | assumption |
| `ChronoReinfDelay=180` | delay for chrono reinforcements (campaign triggers) | INI | confirmed |

Paradrops:

| Item | Rule / value | Source | Confidence |
|---|---|---|---|
| American | `[General] AmerParaDropInf=E1`, `AmerParaDropNum=8` | INI | confirmed |
| Tech airport, Allied owner | `AllyParaDropInf=E1`, `AllyParaDropNum=6` | INI | confirmed |
| Tech airport, Soviet owner | `SovParaDropInf=E2`, `SovParaDropNum=9` | INI | confirmed |
| Plane | `[PDPLANE]` Strength 400, armour light, Speed 15, enters from the map edge, can be shot down, one plane per use | INI, ARES/superweapons/types/paradrop.html | confirmed |
| Drop point | troops jump when the plane is within `[General] ParadropRadius=1024` leptons (4 cells) of the target | INI, ME/ParadropRadius | confirmed |
| Fall | `ParachuteMaxFallRate=-3` leptons per frame | INI | confirmed |

EVA and warnings: the construction of a superweapon building is announced to every other player, readiness only
to the owner, and timers with `ShowTimer=yes` are visible to all players (`ARES/superweapons/evaevents.html`,
INI; `single source`). The storm prints "Warning: Catastrophic lightning storm approaching" (`ME/LightningDeferment`).

#### 13. Radiation

```
spreadCells = trunc(warhead.CellSpread); spreadLeptons = spreadCells * 256
on detonation of a weapon with RadLevel > 0, for every cell with centre distance d <= spreadLeptons:
    cell.level = min(cell.level + trunc((spreadLeptons - d) / spreadLeptons * RadLevel), RadLevelMax)
site.duration   = RadLevel * RadDurationMultiple          ; frames
site.levelSteps = duration div RadLevelDelay
every RadLevelDelay frames: each cell loses trunc((spreadLeptons - d) / spreadLeptons * RadLevel / levelSteps)
every RadApplicationDelay frames (global frame counter modulo): each infantry or vehicle standing in a cell
    takes trunc(min(cell.level, RadLevelMax) * RadLevelFactor) from RadSiteWarhead
```

| Item | Vanilla value | Source | Confidence |
|---|---|---|---|
| Keys | `[Radiation] RadDurationMultiple=1`, `RadApplicationDelay=16`, `RadLevelMax=500`, `RadLevelDelay=90`, `RadLightDelay=90`, `RadLevelFactor=0.2`, `RadLightFactor=0.1`, `RadTintFactor=1.0`, `RadColor=0,255,0`, `RadSiteWarhead=RadSite` | INI | confirmed |
| Duration | level times `RadDurationMultiple` frames (500 frames for a nuke) | ME/RadDurationMultiple, INI comment, PHOBOS/Ext/RadSite/Body.cpp | confirmed |
| Per cell level | linear in distance, as above | PHOBOS/Ext/RadSite/Hooks.cpp (`RadSiteClass_UpdateLevel`) | single source |
| Damage | level times `RadLevelFactor` before Verses; centre of a 500 site: 100 per 16 frames | ME/RadLevelFactor, ME/RadApplicationDelay, PHOBOS same file | confirmed |
| Warhead | `[RadSite]` Verses 100,100,100,50,10,10,0,0,0,100,100, `Radiation=yes`: no building damage | INI | confirmed |
| Immune | `ImmuneToRadiation=yes` objects take nothing from `Radiation=yes` warheads (Desolator, miners, Terror Drone and others, 27 sections) | INI, ME/Radiation | confirmed |
| Re-irradiation | a new hit on an existing site of the same spread sets level = remaining level + new level and restarts the duration | PHOBOS/Ext/RadSite/Body.cpp (`RadSiteExt::Add`) | single source |
| Decay | stepwise every 90 frames; with duration 500 there are 5 steps of one fifth each | derived from the above | single source |
| Sources of radiation | `NukePayload` 500 (spread 10), `RadEruptionWeapon` 500 (warhead spread 10), `CRNuke` 500 (5), `Nukebomb` 500 (10), `Demobomb` 100 (8) | INI | confirmed |
| Desolator redeploy | the deployed Desolator fires again only when the cell level is below one third of the weapon RadLevel | PHOBOS/Ext/RadSite/Hooks.cpp (`InfantryClass_AIDeployment_CheckRad`) | single source |

#### 14. Specialists

Infantry abilities:

| Subject | Rule / keys | Source | Confidence |
|---|---|---|---|
| GI deploy | `[E1] Deployer=yes`, `DeployFire=yes`; deployed GI cannot move and fires `Secondary=Para` (Damage 15, ROF 15, Range 5, warhead `SSA`) instead of `M60` (15, 20, 4, `SA`); elite `ParaE` (25, 5, 7) | INI, ME/Deployer, ME/DeployFire | confirmed |
| Deployed armour bonus | no key in vanilla (`DeployedDamage`, `DeployFireWeapon` absent) | INI (0 hits) | confirmed: none |
| `UndeployDelay` | frames before automatic undeploy: `[YURI] 150`, `[YURIPR] 75`; GI and Desolator stay deployed | INI, ME/UndeployDelay | confirmed |
| C4 | `C4=yes` on `TANY`, `GHOST`, `PTROOP`: entering an enemy building plants a charge that destroys it after `[CombatDamage] C4Delay=.03` minutes = 27 frames with `C4Warhead=Super`; the infantry survives; `CanC4=no` buildings are immune | INI, ME/C4, ME/C4Delay | confirmed |
| Tanya versus ships | `Secondary=Sapper` (Damage 2500, ROF 100, Range 1.5, `Super`); swims (`SpeedType=Amphibious`) | INI | confirmed |
| Crazy Ivan | `IvanBomber` plants a bomb on any `Bombable` object; it explodes after `[CombatDamage] IvanTimedDelay=450` frames for `IvanDamage=400` with `IvanWarhead=IvanWH` (`CellSpread=1.5`, `PercentAtMax=.25`, steel 250%, concrete 20%) | INI, ME/IvanBomb, ME/IvanDamage, ME/IvanTimedDelay | confirmed |
| Bomb control | `CanDetonateTimeBomb=no`, `CanDetonateDeathBomb=no`; the target does not retaliate; Engineer `DefuseKit` (`BombDisarm` warhead) removes bombs | INI, ME/IvanBomb, ME/Engineer | confirmed |
| Elite Ivan | `IvanBomberE Damage=600`; whether weapon Damage overrides `IvanDamage` is undocumented | INI | assumption: bomb damage = weapon Damage |
| Engineer capture | one engineer captures any `Capturable=yes` enemy or neutral building and is consumed | ME/Engineer, INI | confirmed |
| Multi engineer | `[MultiplayerDialogSettings] MultiEngineer=no`; the option is de-supported in RA2; `EngineerCaptureLevel` and `EngineerDamage` are absent from vanilla rules | INI, ME/MultiEngineer, ME/EngineerCaptureLevel | confirmed |
| Spy disguise | `[SPY] CanDisguise=yes`, `PermaDisguise=yes`, weapon `MakeupKit` copies the look of a targeted infantry; default look `AlliedDisguise=E1` / `SovietDisguise=E2` | INI | confirmed |
| Spy effects | needs `Agent=yes`, `Infiltrate=yes` and target `Spyable=yes`; spy is consumed; see table below | ME/Agent, ME/Spyable, INI | confirmed |
| Mind control | `[MindControl]` (Yuri) Range 7, ROF 200, warhead `Controller` (`MindControl=yes`, Verses 0% against wood, steel, concrete); before YR a controller holds one unit and releases the first when it takes a second; control ends when the controller dies | INI, ME/MindControl | single source |
| Mind control immunity | `ImmuneToPsionics=yes` cannot be targeted; buildings default to immune; miners, drones, aircraft, Yuri are immune | INI, ME/MindControl | confirmed |
| Yuri Prime | `SuperMindControl` Range 30; `BuildLimit=1`; needs stolen Soviet tech | INI | confirmed |
| Psi wave | deploy fires `[PsiWave]` Damage 250, `AreaFire`, warhead `PsiPulse` (`CellSpread=3`, `PercentAtMax=.85`, infantry armours only) | INI | confirmed |
| Chrono Legionnaire erase | `NeutronRifle` Damage 8 (elite 16), Range 5, warhead `ChronoBeam` (`Temporal=yes`); target is frozen and untargetable; `remaining = 10 * target.Strength`, each frame subtract the sum of the Damage of all legionnaires on it; erased at 0; if firing stops the target returns unharmed | INI, ME/Temporal, YRPP/TemporalClass.h (`WarpRemaining`, `WarpPerStep`) | confirmed |
| Chrono move | Teleport locomotor: after arriving the unit cannot move or fire for `max(distanceLeptons div ChronoDistanceFactor, ChronoMinimumDelay)` frames but can be damaged; `[General] ChronoDistanceFactor=48`, `ChronoMinimumDelay=16`, `ChronoTrigger=yes`, `ChronoRangeMinimum=0` (5.33 frames per cell) | INI, ME/ChronoDistanceFactor | confirmed |
| Tesla Trooper | `[SHK] Secondary=AssaultBolt` (Range 1.83, ROF 25, warhead `ElectricAssault`) is used automatically on adjacent own `Overpowerable=true` buildings (`TESLA`); with chargers c: coil stays online in low power when c >= 2; coil uses `Secondary=OPCoilBolt` (300, ROF 100, Range 8) instead of `CoilBolt` (200, ROF 120, Range 7) when c >= 3, or c >= 1 with full power | INI, ME/ElectricAssault, PHOBOS/Ext/Building/Hooks.cpp (`UpdateOverpower`) | confirmed |
| Desolator | `RadBeamWeapon` 125, ROF 50, Range 6 (`RadBeamWarhead`, no building damage); deploy fires `RadEruptionWeapon` (`AreaFire`, ROF 60, `RadLevel=500`) | INI | confirmed |
| Terrorist | `[TerrorBomb]` Damage 225, Range 1.5, `Suicide=yes`, warhead `TerrorBombWH` (`CellSpread=2`, `PercentAtMax=.5`); also his `DeathWeapon` | INI | confirmed |

Spy infiltration (vanilla):

| Target | Effect | Source | Confidence |
|---|---|---|---|
| Power plant | victim's power output is 0 for `SpyPowerBlackout=1000` frames | INI, ME/Agent | confirmed |
| Refinery | steals `SpyMoneyStealPercent=.5` of the victim's current credits | INI, ME/SpyMoneyStealPercent (ME/Agent claims `Thief=yes` is needed; no vanilla unit has it) | single source |
| Radar | resets the victim's shroud to what it currently sees | ME/Spyable, ARES/buildings/spyeffects.html | confirmed |
| Barracks | infantry later built by the spy's owner start veteran | ME/Agent, ARES | confirmed |
| War factory | vehicles later built start veteran (not ships) | ME/Agent, ARES | confirmed |
| Battle lab | unlocks stolen-tech infantry: `CCOMAND` and `CIVAN` need `RequiresStolenAlliedTech`, `PTROOP` and `YURIPR` need `RequiresStolenSovietTech`; `CCOMAND`/`PTROOP` are Allied-barracks units, `CIVAN`/`YURIPR` Soviet-barracks units | INI, ME/Agent | confirmed |
| Superweapon | recharge timer restarts from zero | ME/Spyable, ARES | confirmed |
| One effect per entry | hard-coded precedence | ARES/buildings/spyeffects.html | single source |

Structures and vehicles:

| Subject | Rule / keys | Source | Confidence |
|---|---|---|---|
| Prism Tower | `[General] PrismType=ATESLA`, `PrismSupportModifier=150%`, `PrismSupportMax=8`, `PrismSupportDelay=60`, `PrismSupportDuration=15`, `PrismSupportHeight=420`; damage multiplier `1 + 1.5 * N` for N supporting towers; a supporter is offline 60 frames; support uses `Secondary=PrismSupport` range 8 cells between adjacent towers; firing weapon `PrismShot` 120, ROF 60, Range 8 | INI, ME/PrismSupportModifier, ME/PrismSupportMax | confirmed |
| Mirage Tank | `[MGTK] CanDisguise=yes`, `DisguiseWhenStill=yes`: when stationary it looks like a random `[General] DefaultMirageDisguises=TREE01..TREE04`; revealed while moving; `MirageGun` has `RevealOnFire=no`; not auto-targeted by units without `DetectDisguise` | INI, ME/DisguiseWhenStill | single source |
| Terror Drone | `DroneJump` Damage 50, ROF 60, Range 1.83, warhead `Parasite` (vehicles 100%, buildings 0%): kills infantry instantly; inside a vehicle deals 50 (times Verses) every ROF; host cannot hit it; exits unharmed if it kills the host, dies if the host dies otherwise; removed by Service Depot or repair IFV; `ParasiteDamage` key does not exist | INI, ME/Parasite | confirmed |
| Attack dog | `BadTeeth`/`GoodTeeth` Damage 30, Range 1.5, warhead `ParasiteDog` (infantry armours only): instant kill of infantry | INI, ME/Parasite | confirmed |
| Giant Squid | `SquidGrab` Damage 15 (elite 40), ROF 99, warhead `ParasitePlus` (`Paralyzes`, `Culling`): the ship cannot move but can fire; ship is destroyed outright when its health is at or below `ConditionRed` (elite squid: `ConditionYellow`); only on water; Dolphin `SonicZap` (`Sonic=yes`) evicts the squid | INI, ME/Parasite, ME/Paralyzes, ME/Culling | confirmed |
| IFV | `[FV] Gunner=yes`, `Passengers=1`, `SizeLimit=1`; passenger `IFVMode=N` selects `Weapon(N+1)`; see table below; `DeathWeapon=CRNuke` fires only with Crazy Ivan aboard | INI, ME/IFVMode, ME/Gunner | confirmed |
| Nuclear Reactor | `[NANRCT] Power=2000`, `Explodes=yes`, `DeathWeapon=NukePayload` (600, `NUKE`, `RadLevel=500`) | INI, ME/Explodes | confirmed |
| Demolition Truck | `[Demobomb]` Damage 300, `Suicide=yes`, `RadLevel=100`, warhead `DemobombWH` (`CellSpread=8`, `PercentAtMax=.1`, steel 150%, concrete 10%); also `DeathWeapon`; no crew | INI | confirmed |
| Death explosions | `Explodes=yes` fires `DeathWeapon`, else the current primary weapon, else `[CombatDamage] DeathWeapon=DefaultDeathWeapon` (`DeathWH`, `CellSpread=1.5`, `PercentAtMax=.5`); passengers die; not triggered by Temporal | INI, ME/Explodes, ME/DeathWeapon | confirmed |
| Kirov crash | crashing aircraft fire their primary weapon (or `DeathWeapon`) at the crash cell: `BlimpBomb` 250, `BlimpHE` (`CellSpread=2`, `PercentAtMax=.5`) | INI, ME/Explodes | single source |
| Oil Derrick | `[CAOILD] ProduceCashStartup=1000` on every capture from a passive (neutral) house, then `ProduceCashAmount=20` every `ProduceCashDelay=100` frames; `DeathWeapon=OilExplosion` (600, `CellSpread=4`) | INI, ME/ProduceCashStartup, ME/ProduceCashDelay | confirmed |
| Hospital | `[CATHOSP] Hospital=yes`: own infantry walk in and are healed (step `IRepairStep=20` per `IRepairRate`), then leave; map-wide self-heal is YR | INI, ME/Hospital, ME/IRepairRate | single source |
| Airport | `[CAAIRP] SuperWeapon=ParaDropSpecial` (topic 12) | INI | confirmed |
| Outpost | `[CAOUTP] Primary=HoverMissile` (25 x2, ROF 50, Range 6), `UnitRepair=yes`, `NumberOfDocks=1`, Strength 2000 | INI | confirmed |
| Tech rules | `NeedsEngineer=yes`: inert until captured by an engineer; `Unsellable=yes`; listed in `[AI] NeutralTechBuildings=CAAIRP,CATHOSP,CAOILD,CAOUTP` | INI, ME/NeedsEngineer | confirmed |
| Bridge hut | `[CABHUT] BridgeRepairHut=yes`, `Immune=yes`: an engineer entering it repairs the adjacent bridge and is consumed; C4 on the hut destroys the bridge; `[CombatDamage] DestroyableBridges=yes`, `BridgeStrength=1500` | INI, ME/BridgeRepairHut | single source |
| Garrison | `CanBeOccupied=yes` + `MaxNumberOccupants` (vanilla civilian buildings, mostly 10, some 3 to 8); only `Occupier=yes` infantry (`E1`, `E2`) and only neutral buildings; building fires `Primary=AlliedOccupyW` (30, ROF 15, Range 7, `SSAB`) for Allied occupants or `Secondary=SovietOccupyW` (20, ROF 20, Range 7) for Soviet ones; rearm divided by occupant count; occupants ejected and entry blocked at `ConditionRed`; `[General] ThreatPerOccupant=10` | INI, ME/Occupier, ME/CanBeOccupied, ME/ROF, ME/ConditionRed | confirmed |
| Garrison keys absent | `OccupyWeapon`, `OccupyDamageMultiplier`, `OccupyROFMultiplier`, `OccupyWeaponRange`, `CanOccupyFire`, `[UCPara]` are YR only | INI (0 hits) | confirmed |
| Crushing | `Crusher=yes` vehicles kill `Crushable=yes` infantry by driving over them; `[DESO] Crushable=no`, `[CLEG] Crushable=no`; `OmniCrusher`, `OmniCrushResistant` are YR only | INI, ME/Who_crushes_whom | confirmed |
| Crush AI | `[CombatDamage] Crush=1.8` cells (AI crushes instead of firing), `PlayerAutoCrush=no`, `[IQ] AutoCrush=2` | INI | confirmed |
| Health states | `[AudioVisual] ConditionYellow=50%`, `ConditionRed=25%` (health below the fraction): yellow = damaged art, sparks; red = garrison eviction, squid culling, AI sell check, engineer auto-repair in area guard; smoke starts below 50% (hard-coded) | INI, ME/ConditionYellow, ME/ConditionRed | single source |

IFV weapon table (`[FV] WeaponCount=13`, `TurretCount=4`; elite weapons identical except Weapon1 `HoverMissileE`):

| IFVMode | Passenger | Weapon | Damage / ROF / Range | Turret index |
|---|---|---|---|---|
| 0 | none, dogs | `HoverMissile` | 25 x2 / 50 / 6 | 0 |
| 1 | Engineer | `RepairBullet` | -50 / 80 / 1.8 | 2 |
| 2 | GI, Conscript, Spy | `CRM60` | 20 / 15 / 6 | 1 |
| 3 | Flak Trooper | `CRFlakGuyGun` | 30 / 15 / 7 | 1 |
| 4 | Tanya, SEAL, Chrono Commando, Psi Commando | `CRMP5` | 125 / 20 / 6 | 1 |
| 5 | Sniper | `AWPE` | 125 / 60 / 14 | 1 |
| 6 | Tesla Trooper | `CRElectricBolt` | 60 / 45 / 6 | 3 |
| 7 | Crazy Ivan, Chrono Ivan | `CRNuke` | 200, suicide, `RadLevel=500` | 3 |
| 8 | Yuri, Yuri Prime | `CRMindControl` | 200 area / 10 / 1 | 3 |
| 9 | Desolator | `CRRadBeamWeapon` | 175 / 30 / 7 | 3 |
| 10 | Chrono Legionnaire | `CRNeutronRifle` | 10 / 120 / 6 | 3 |
| 11 | Terrorist | `CRTerrorBomb` | 250, suicide | 3 |
| 12 | cow and other animals | `CowShot` | 300 / 75 / 12 | 3 |

Aircraft, spawners, submarines:

| Subject | Rule / keys | Source | Confidence |
|---|---|---|---|
| Harrier, Black Eagle | `Ammo=1`, `Fighter=yes`, `Landable=yes`, `AirportBound=yes` (crashes without a pad), `Dock=GAAIRC,AMRADR`; `Maverick` 150 x2 / `Maverick2` 200 x2; one volley per sortie; `[General] PadAircraft=ORCA,BEAG`; pads `NumberOfDocks=4` | INI, ME/Ammo | confirmed |
| Ammo | default -1 = unlimited; aircraft return to the pad when empty; reload 270 frames per point | INI, ME/Ammo, ME/ReloadRate | confirmed |
| Carrier | `[CARRIER] Spawns=HORNET`, `SpawnsNumber=3`, `SpawnRegenRate=600` (frames to rebuild a lost Hornet), `SpawnReloadRate=150` (frames to rearm a returned one); `HornetLauncher` Range 25; `[HORNET]` Strength 75, light, Speed 12, `HornetBomb` 40 | INI, ME/SpawnRegenRate, ME/SpawnReloadRate | confirmed |
| Destroyer | `Spawns=ASW`, `SpawnsNumber=1`, regen 400, reload 150; `[ASW]` Strength 30, `ASWBomb` 50 | INI | confirmed |
| Launch spacing | 20 frames between launches, 10 for `MissileSpawn=yes` | ME/MissileSpawn | single source |
| Spawner death | Hornets and Ospreys die with the carrier; missiles in flight continue | ME/MissileSpawn | single source |
| V3 | `[V3] Spawns=V3ROCKET`, 1 rocket, `SpawnRegenRate=400` (the reload time, started at launch); `V3Launcher` Range 18, `MinimumRange=5`, ROF 150; `[V3ROCKET]` Strength 50, armour special_2, Speed 15; `V3RocketDamage=200` with `V3Warhead=V3WH` (`CellSpread=1`, `PercentAtMax=.25`); elite 400 with `V3EWH` (`CellSpread=2`, `.5`) | INI, ME/SpawnRegenRate | confirmed |
| V3 flight | `V3RocketPauseFrames=0`, `V3RocketTiltFrames=60`, `V3RocketPitchInitial=0.21`, `V3RocketPitchFinal=0.5`, `V3RocketTurnRate=0.05`, `V3RocketAcceleration=0.4`, `V3RocketAltitude=768`, `V3RocketBodyLength=256`, `V3RocketLazyCurve=yes` | INI | confirmed |
| Dreadnought | `[DRED] Spawns=DMISL`, 2 missiles, regen 80; `DredLauncher` Range 25, `MinimumRange=8`, ROF 50, Burst 2; `[DMISL]` Strength 50, special_2, Speed 20; `DMislDamage=300` with `DMISLWH` (`CellSpread=1.5`, `.25`); elite 600 with `DMISLEWH` (`CellSpread=3`, `.5`) | INI | confirmed |
| Dreadnought flight | `DMislPauseFrames=20`, `DMislTiltFrames=60`, `DMislPitchInitial=0`, `DMislPitchFinal=0.5`, `DMislTurnRate=0.08`, `DMislAcceleration=0.8`, `DMislAltitude=768`, `DMislBodyLength=128`, `DMislLazyCurve=no` | INI | confirmed |
| Shooting missiles down | against special_2: `FlakWH`, `FlakGuyWH`, `SAMWH`, `SA`, `SSA` 100%; `V3WH`, `DMISLWH`, `Parasite` 0% | INI | confirmed |
| Submarine | `[SUB] Cloakable=yes`, `Underwater=yes`, `Naval=yes`, `CloakingSpeed=1`; `SubTorpedo` 100, ROF 120, Range 7, `DecloakToFire=no` (fires without surfacing); may reveal itself when retaliating | INI, ME/Cloakable | single source |
| Sub detectors | objects with `SensorsSight`: `SUB`, `DLPH`, `SQD` 4 cells, `DEST` 8 cells; Osprey depth charges need the destroyer's detection | INI, ME/SensorsSight | single source |

#### 15. Short game, base unit and lobby defaults

`[MultiplayerDialogSettings]` (all `confirmed` from INI):

| Key | Value | Key | Value | Key | Value |
|---|---|---|---|---|---|
| `MinMoney` | 5000 | `Money` | 10000 | `MaxMoney` | 10000 |
| `MoneyIncrement` | 100 | `MinUnitCount` | 0 | `UnitCount` | 10 |
| `MaxUnitCount` | 10 | `TechLevel` | 10 | `GameSpeed` | 0 |
| `AIDifficulty` | 0 | `AIPlayers` | 0 | `BridgeDestruction` | yes |
| `ShadowGrow` | no | `Shroud` | yes | `Bases` | yes |
| `TiberiumGrows` | yes | `Crates` | yes | `CaptureTheFlag` | no |
| `HarvesterTruce` | no | `MultiEngineer` | no | `AlliesAllowed` | no |
| `ShortGame` | yes | `FogOfWar` | no | `MCVRedeploys` | yes |

`[Maximums] Players=8`.

| Item | Rule / value | Source | Confidence |
|---|---|---|---|
| Short game | a house is defeated as soon as it owns no buildings and no `BaseUnit` vehicle; `Insignificant=yes` and `DontScore=yes` objects are ignored | ME/ShortGame, ME/BaseUnit | confirmed |
| Normal game | every remaining object must be destroyed | ME/ShortGame (implicit) | assumption |
| `BaseUnit` | `[General] BaseUnit=AMCV,SMCV`: first entry Allied, second Soviet; the unit given at start and counted as home base | INI, ME/BaseUnit | confirmed |
| `MCVRedeploys` | yes: a Construction Yard can undeploy (`UndeploysInto=AMCV`/`SMCV`) | INI, ME/MCVRedeploys | single source |
| `HarvesterTruce` | de-supported in RA2 | INI comment, ME/HarvesterTruce | confirmed |
| `GameSpeed=0` | lobby slider index (0 is the fastest setting); not a simulation rule | INI | confirmed |

Starting units (algorithm of the engine family; Ares confirms "Unit Count times average cost"):

```
eligible  = InfantryTypes and VehicleTypes without AllowedToStartInMultiplayer=no, excluding BaseUnit
avgCost   = sum(Cost of eligible) div count(eligible)        ; all sides together
budget    = (UnitCount - 1 if Bases else UnitCount) * avgCost
per house: place the BaseUnit at the start point (when Bases), then
  spent = 0
  while spent < budget:
      if spent < budget * 2 div 3 and the house has an eligible vehicle: pick a random eligible vehicle
      else: pick a random eligible infantry
      (eligible for the house = TechLevel <= game TechLevel, Owner includes the country,
       RequiredHouses/ForbiddenHouses satisfied)
      place it 3 or more cells from the start point; spent += Cost
```

| Item | Value | Source | Confidence |
|---|---|---|---|
| Algorithm | as above | VINIFERA (`Create_Units` reimplementation of the Tiberian Sun routine), ARES/sidescountries/startinmultiplayer.html, ME/UnitCount, ME/AllowedToStartInMultiplayer | single source for RA2 (two sources agree on the budget rule) |
| Vanilla eligible types | `E1` 200, `E2` 100, `MTNK` 700, `HTNK` 900 (95 other sections carry `AllowedToStartInMultiplayer=no`) | INI | confirmed |
| Vanilla numbers | avgCost = 1900 div 4 = 475; UnitCount 10 with bases: budget 4275; Allied: 5 Grizzly then 4 GI; Soviet: 4 Rhino then 7 Conscripts | derived | single source |
| Caveat | `[VehicleTypes]` also lists `DeathDummy` (no Cost, flag unset); if the engine counts it avgCost = 380 and the budget is 3420 (Allied 4 + 4, Soviet 3 + 8) | INI | assumption: exclude it |
| `AlliedBaseInfantry`, `SovietBaseInfantry` | no such keys in vanilla rules | INI (0 hits) | confirmed |
| Veteran start | `[General] InitialVeteran=no` | INI | confirmed |

#### 16. Countries

`[Countries]` 0 to 8 are playable; `[Sides] GDI=British,French,Germans,Americans,Alliance`,
`Nod=Russians,Africans,Confederation,Arabs`. Special types are `confirmed` from INI (`RequiredHouses=` on the type); the in-game country names come from
the string table, not from rules (`Name=` holds Asian Alliance, African Empire, Arab Union, Latin Confederation).

| Section | In-game name | Side | Special | Type and key facts |
|---|---|---|---|---|
| `Americans` | America | Allied | Paratroopers | `AMRADR` replaces `GAAIRC` (`RequiredHouses=Americans`), grants `AmericanParaDropSpecial`: 8 GI every 4 minutes |
| `Alliance` | Korea | Allied | Black Eagle | `BEAG` replaces `ORCA` (`ForbiddenHouses=Alliance`): Strength 200, `Maverick2` 200 x2 |
| `French` | France | Allied | Grand Cannon | `GTGCAN` cost 2000, Strength 900, `GrandCannonWeapon` 150, ROF 120, Range 15, min 3, `Power=-100` |
| `Germans` | Germany | Allied | Tank Destroyer | `TNKD` cost 900, Strength 400, `SABOT` 150, ROF 70, Range 5, warhead `UltraAP` (2% against infantry and buildings) |
| `British` | Great Britain | Allied | Sniper | `SNIPE` cost 600, `AWP` 125, ROF 150, Range 14, warhead `HollowPoint` |
| `Africans` | Libya | Soviet | Demolition Truck | `DTRUCK` cost 1500, Strength 150, `Demobomb` |
| `Arabs` | Iraq | Soviet | Desolator | `DESO` cost 600, Strength 150, armour plate |
| `Confederation` | Cuba | Soviet | Terrorist | `TERROR` cost 200, Strength 75, Speed 6 |
| `Russians` | Russia | Soviet | Tesla Tank | `TTNK` cost 1200, Strength 300, `TankBolt` 135, ROF 75, Range 4 |

Per-country multipliers: the vanilla country sections contain only `UIName`, `Name`, `Suffix`, `Prefix`, `Color`,
`Multiplay`, `Side`, `SmartAI`. None of `Firepower`, `Groundspeed`, `Airspeed`, `Armor`, `ROF`, `Cost`,
`BuildTime`, `ArmorInfantryMult`, `ArmorUnitsMult`, `CostUnitsMult`, `VeteranInfantry`, `VeteranUnits`,
`VeteranAircraft` appears in any vanilla country section (`confirmed`, INI 0 hits); all countries play at 1.0.

#### 17. AI difficulty

Difficulty sections (INI, `confirmed`):

| Key | `[Easy]` | `[Normal]` | `[Difficult]` |
|---|---|---|---|
| `Groundspeed` | 1.0 | 1.0 | 1.0 |
| `Airspeed` | 1.0 | 1.0 | 1.0 |
| `BuildTime` | .8 | 1 | 1.0 |
| `Armor` | 1.2 | 1.0 | .8 |
| `ROF` | .8 | 1.0 | 1.2 |
| `Cost` | 1.0 | 1.0 | 1.0 |
| `RepairDelay` | .02 | .02 | .05 |
| `BuildDelay` | .03 | .03 | .1 |
| `BuildSlowdown` | (absent) | yes | yes |
| `DestroyWalls` | no | no | no |
| `ContentScan` | yes | yes | (absent) |
| `Firepower` | (absent) | (absent) | (absent) |

| Item | Rule / value | Source | Confidence |
|---|---|---|---|
| Mapping in skirmish | human always uses `[Normal]`; hard AI uses `[Easy]`, medium AI `[Normal]`, easy AI `[Difficult]` (sections are named from the human point of view) | ME/Difficulty_Settings | single source |
| Functional keys in RA2 | `FirePower` and `ROF` (both sides), `BuildTime` and `RepairDelay` (AI only); `Armor`, `Cost`, `Airspeed`, `Groundspeed`, `BuildSlowdown`, `BuildDelay`, `ContentScan` are marked obsolete | ME/Difficulty_Settings | single source |
| Net effect | hard AI: ROF delay x0.8, build time x0.8; easy AI: ROF delay x1.2; medium: none | derived | single source |
| `RepairDelay` | multiplies hard-coded 225 and 1800 frame bounds for the AI repair start delay | ME/RepairDelay | single source |
| Start money bonus | `[General] MultiplayerAICM=400,0,0` (hard, medium, easy): hard AI gets +400% of the starting credits once at game start | INI, ME/MultiplayerAICM | confirmed |
| Harvest bonus | `AIVirtualPurifiers=4,2,0` (topic 8) | INI | confirmed |
| Team timer | `[General] TeamDelays=2000,2500,3500` frames (hard, medium, easy) between AI trigger checks; the AI waits one interval before its first team | INI, ME/TeamDelays | confirmed |
| Enemy choice | `AIHateDelays=30,50,70` frames | INI | confirmed |
| Team caps | `TotalAITeamCap=30,30,30`, `MinimumAIDefensiveTeams=1,1,1`, `MaximumAIDefensiveTeams=2,2,2`, `FillEarliestTeamProbability=100,100,100`, `DissolveUnfilledTeamDelay=5000`, `UseMinDefenseRule=yes` | INI | confirmed |
| Base defences | `AlliedBaseDefenseCounts=25,20,6`, `SovietBaseDefenseCounts=25,22,6` (hard, medium, easy); `AIPickWallDefensePercent=50,25,10`; `AIBuildsWalls=no` | INI | confirmed |
| Misc AI | `HarvestersPerRefinery=2`, `AIAutoDeployFrameDelay=15,25,100`, `AIRestrictReplaceTime=400`, `AISafeDistance=20`, `AIMinorSuperReadyPercent=.7`, `AINavalYardAdjacency=20`, `BaseDefenseDelay=.25`, `AIAlternateProductionCreditCutoff=1000`, `MaximumBuildingPlacementFailures=3` | INI | confirmed |
| Superweapon targets | `AIIonCannonConYardValue=10`, `WarFactoryValue=100`, `PowerValue=60`, `EngineerValue=1`, `ThiefValue=1`, `HarvesterValue=1`, `MCVValue=1`, `APCValue=1`, `BaseDefenseValue=35`, `PlugValue=40`, `HelipadValue=20`, `TempleValue=40` (same for all difficulties): weights for storm and nuke targeting | INI, ME/AI | confirmed |
| `[AI]` lists | `BuildConst=GACNST,NACNST`, `BuildPower=NAPOWR,GAPOWR`, `BuildRefinery=NAREFN,GAREFN`, `BuildBarracks=NAHAND,GAPILE`, `BuildTech=NATECH,GATECH`, `BuildWeapons=GAWEAP,NAWEAP`, `BuildDefense=TESLA,ATESLA`, `BuildPDefense=TESLA,ATESLA`, `BuildAA=NASAM,NAFLAK`, `BuildHelipad=GAAIRC,AMRADR`, `BuildRadar=GAAIRC,NARADR,AMRADR`, `BuildNavalYard=NAYARD,GAYARD`, `BuildDummy=GAPILL,NALASR`, `ConcreteWalls=GAWALL,NAWALL` | INI | confirmed |
| `[AI]` ratios | `RefineryRatio=.16`/`RefineryLimit=4`, `BarracksRatio=.16`/`2`, `WarRatio=.1`/`2`, `DefenseRatio=.4`/`40`, `AARatio=.14`/`10`, `TeslaRatio=.16`/`10`, `HelipadRatio=.1`/`2`, `AirstripRatio=.12`/`0`, `PowerSurplus=50`, `BaseSizeAdd=3`, `PowerEmergency=75%`, `AIBaseSpacing=1`, `CreditReserve=100`, `InfantryReserve=50000`, `InfantryBaseMult=1`, `Paranoid=yes`, `CompEasyBonus=no`, `MaximumBaseDefenseValue=60`, `ComputerBaseDefenseResponse=3` | INI | confirmed |
| Obsolete `[AI]` keys | `AttackInterval=0.5`, `AttackDelay=0.5` (ini comment: use TeamDelays), `GDIBaseDefenseCoefficient`, `NodBaseDefenseCoefficient` | INI comments | confirmed |
| `[IQ]` | `MaxIQLevels=5`, `SuperWeapons=4`, `Production=5`, `GuardArea=2`, `RepairSell=1`, `AutoCrush=2`, `Scatter=2`, `ContentScan=3`, `Aircraft=3`, `Harvester=2`, `SellBack=2` | INI | confirmed |
| Absent keys | `AIBaseDefenses`, `AIExtraRefineries`, `AISuperDefenseProbability`, `AISuperDefenseFrames`, `AlliedBaseDefenses`, `SovietBaseDefenses` are YR only | INI (0 hits) | confirmed |
| Cheats | AI houses can build any unit regardless of `Owner` and `Prerequisite`, see through shroud and unexplored terrain, recognise cloaked units, and are not affected by Gap Generators | ME/AI, ME/GapGenerator, ME/Sight | confirmed |
| Power AI | sells buildings when power falls below `PowerEmergency=75%`; builds plants until surplus is `PowerSurplus=50` | INI | single source |

#### 18. Hotkeys

Default RA2 keyboard commands. `confirmed` rows appear both in the RA2 manual keyboard section (`MANUAL`) and
in `MGW`; other rows rest on `MGW` alone. `MGW` covers RA2 and YR together and marks nothing as YR only.

| Key | Command | Source | Confidence |
|---|---|---|---|
| Q | Structure tab | MANUAL, MGW | confirmed |
| W | Armory (defence) tab | MANUAL, MGW | confirmed |
| E | Infantry tab | MANUAL, MGW | confirmed |
| R | Unit (vehicle) tab | MANUAL, MGW | confirmed |
| G | Guard area | MANUAL, MGW | confirmed |
| X | Scatter | MANUAL, MGW | confirmed |
| S | Stop | MANUAL, MGW | confirmed |
| F | Follow (view tracks the selected unit) | MANUAL, MGW | confirmed |
| T | Type select (all units of the selected type) | MANUAL, MGW | confirmed |
| H | Centre view on base (Construction Yard) | MANUAL, MGW | confirmed |
| K | Repair mode | MANUAL, MGW | confirmed |
| L | Sell mode | MANUAL, MGW | confirmed |
| D | Deploy | MGW | single source |
| A | Alliance with the selected unit's owner | MGW | single source |
| Z | Waypoint mode (hold and click) | MGW | single source |
| N / M | Next / previous unit | MGW | single source |
| P | Select all units | MGW | single source |
| Y | Select by veterancy (may be YR only; the source does not say) | MGW | single source |
| U | Cycle sidebar pages | MGW | single source |
| B | Place beacon (Delete removes it) | MGW | single source |
| Space | Jump to last radar event | MGW | single source |
| Ctrl + 1..9 | Create team | MGW | single source |
| 1..9 | Select team; press twice to centre on it | MGW | single source |
| Alt + 1..9 | Select team and centre view | none loaded | assumption |
| Shift + click | Add or remove a unit from the selection | MGW | single source |
| Ctrl + click | Force fire on a target or ground | MGW | single source |
| Alt + click | Force move (crush) | MGW | single source |
| Ctrl + Shift + click | Attack move | MGW lists "Ctrl + Click: attack-move"; the three-key form is from the engine's keyboard options | assumption |
| Ctrl + Alt + click | Guard target / area guard at destination | MGW, ME/ConditionRed ("area guard (Ctrl+Alt)") | confirmed |
| Ctrl + F1..F4 | Set map bookmark | MGW | single source |
| F1..F4 | Jump to bookmark | MGW | single source |
| F5..F8 | Multiplayer taunts | MGW | single source |
| Enter / Backspace | Chat to all / to allies | MGW | single source |
| Tab | Diplomacy screen | MANUAL (search snippet) | single source |
| Esc | Options menu | MGW | single source |

YR-only additions: none of the keys above could be shown to be YR only from the loaded sources; the keyboard
list on ModEnc (`ME/Keyboard.ini`) is an index page without bindings.
