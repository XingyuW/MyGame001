# Unified construction and production system — M9

Implemented on 2026-10-05 in the existing deterministic Rust/Bevy game. The human player has all
72 requested original RA2 entries: 14 Buildings, 15 Armory, 15 Infantry and 28 Vehicles (15 ground,
4 aircraft, 9 naval). Yuri is the original RA2 infantry unit; there is no Yuri's Revenge roster.
The original Vanguard/Meridian computer opponents, match flow and starting armies remain available.

## Integration and rules

The implementation extends the existing `ProductionItem`, `Definitions`, queues per producer type,
`CommandQueue`, ECS ownership/health/weapons, deterministic schedule, economy, fog grid and movement
system. It does not run a parallel economy or production simulation.

- Cards show names, existing original sprite icons, credits, missing producers/prerequisites,
  current progress, queued count, ready placement and blocked exits. All four tabs stay visible;
  arrows page through four cards at a time and Tab cycles categories. Vehicles contains ground,
  air and naval groups in that order. Hover text explains the role.
- Construction is charged incrementally using the original integer accounting. Insufficient
  funds pause progress. Cancellation refunds exactly the paid amount, including a ready building.
  Losing required infrastructure or every eligible producer pauses work; cancellation remains
  available for refunds. Each producing building type has one serial queue per player.
- A completed structure waits for placement. Footprints must be visible, within base range,
  unoccupied and on suitable terrain. Shipyards occupy water touching shore and need a water exit.
  Unit exit checks use the unit's mobility domain; blocked exits retain the completed queue item.
- Land, air, naval and amphibious navigation reuse the pathfinder and occupancy reservations.
  Land cannot cross water; naval units cannot cross land; aircraft cross either, and amphibious
  transports cross both. Rally orders use the same navigation rules.
- Existing power generation/consumption remains authoritative. Power Plant, Tesla Reactor and
  Nuclear Reactor offer different capacity/cost tradeoffs. Any power generator satisfies basic
  power prerequisites. Low power slows production, disables powered defensive fire, pauses
  superweapon charging and switches off powered support effects.
- Barracks, War Factory, Airforce Command and Naval Shipyard are the respective production
  sources. Radar and Battle Lab gate specialists/advanced equipment. The MCV requires Service
  Depot and deploys into a real, functional Construction Yard after footprint validation.
- Refineries accept the original collector and both new miners. Chrono Miner teleports to an
  available refinery dock; War Miner uses conventional travel and carries a gun. Purifier adds
  25% to ore delivery while powered. Capturing a refinery cannot pay its former owner's miner.
- Service Depot repairs nearby vehicles. Powered Cloning Vats duplicates trained infantry once,
  using normal exit handling without recursively cloning clones.

## Specialist mechanics that execute in the simulation

| Group | Implemented behavior |
|---|---|
| Engineer | E then target: approaches, repairs friendly structures or captures enemy structures; consumed on success. |
| Spy / Dog | Spy is concealed until detected, approaches and infiltrates an enemy building. Refineries lose half their credits; generators cause a 30-second power outage; strategic weapons lose charge; other infrastructure is disabled for 30 seconds. Dogs reveal concealed spies and use lethal anti-infantry attacks. |
| Rocketeer / Flak Trooper | Air-domain infantry; flak weapons can engage aircraft. |
| Tanya / Sniper | Commando anti-infantry fire and extra structural demolition damage; long-range infantry-only sniper weapon. |
| Chrono Legionnaire | Teleports to a valid commanded destination, with recovery delay; attacks suppress targets. |
| Tesla units | Electrical anti-ground weapons with differentiated armor effectiveness; no anti-air targeting. |
| Crazy Ivan | Attacks attach a timed explosive; it detonates with area damage. |
| Yuri | Controls one eligible enemy unit at a time; ownership, targeting and team visuals change, and original ownership returns when the controller dies. Controlled units cannot embark. |
| Desolator / Terrorist | D creates persistent ground radiation, sparing airborne units and Desolators; Terrorist detonates a suicide area attack. |
| IFV / Flak Track / NightHawk / Amphibious Transport | Load selected ground passengers with E + transport click; D unloads into valid free cells. IFV takes its passenger's weapon, or repairs nearby vehicles with an Engineer. Passenger health, identity and carried ore survive unloading. Amphibious Transport accepts ground vehicles as well as infantry. |
| Terror Drone / Giant Squid | Rapid vehicle attacks and repeated disabling; Squid disables vessels. |
| Mirage / Prism | Mirage is concealed while idle and revealed by proximity/detectors; Prism has long range, structure effectiveness and splash damage. |
| V3 / Dreadnought / Grand Cannon | Long-range projectile bombardment; Grand Cannon adds area damage. |
| Apocalypse / Tank Destroyer | Heavy ground-and-air combat versus specialized anti-vehicle fire. |
| Demolition Truck / Kirov | Large suicide explosion versus slow heavily armored air bombardment. |
| Harrier / Black Eagle | Airbase production, strike, return toward an air facility and timed rearming; cannot fire during recovery. |
| Aircraft Carrier | Launches up to two actual recoverable aircraft, using ordinary movement, attack and damage paths. Aircraft return to the parent carrier, not an airbase. Loss of the carrier removes its wing. |
| Other naval combat | Destroyer can attack submerged armor; Aegis is air-only; Dolphin/Typhoon specialize in naval/submerged targets; Sea Scorpion provides light surface and air fire. |
| Defenses | Automatic visible-target acquisition, armor/domain-valid attacks and range checks. Patriot and Flak Cannon are air-only. Walls block movement. |
| Reconnaissance | Powered satellite reveals the map; powered gap generators shroud enemy vision locally, countered by direct local sight. Powered Psychic Sensors extend concealed-unit detection. |

## Superweapons

All four are Battle Lab technologies built through Armory. Buttons appear when the structure exists,
show recharge seconds or LOW POWER, and accept a visible map target only when ready. Invalid
activation does not consume charge. Successful activation resets charge. Each structure charges
independently, and destruction removes its charge. Strategic effects produce renderer events.

| Ability | Cost | Build time at full power | Recharge | Effect |
|---|---:|---:|---:|---|
| Chronosphere | 3000 | 80 s | 180 s | Select a grouped set of Vehicles-category units, click ability then destination; validates the whole group before teleporting. |
| Weather Control Device | 5000 | 100 s | 300 s | Persistent 10-second lightning storm over a large area. |
| Iron Curtain Device | 3000 | 80 s | 180 s | Protects friendly eligible mechanical units near the target from damage for 10 seconds. |
| Nuclear Missile Silo | 5000 | 100 s | 300 s | Powerful area strike with distance falloff. Also requires Nuclear Reactor. |

## Deliberate differences from original RA2

This reproduces the requested production organization and functional roles, not exact RA2 balance,
animations or every rules.ini mechanic. Existing economic values for shared infrastructure and
legacy AI units are preserved. All 72 entries use the project's existing original sprite families;
there are no imported RA2 graphics or bespoke art for each new entity.

- Queues are per producer type, rather than RA2's exact category queue and factory-speed rules.
  Buildings and Armory share the Construction Yard's queue. No AI redesign: opponents use their
  working legacy build orders rather than the entire new arsenal or strategic abilities.
- There is one occupancy layer. Aircraft can cross terrain/buildings but do not stack with other
  mobile units; no aircraft runway, landing animation, finite pad count or fuel model. Carrier
  wings reuse strike-aircraft visuals/stats rather than dedicated Hornet art.
- Submerged armor and valid weapons distinguish underwater combat, but there is no separate sonar,
  depth or surfacing layer. Squid and Drone use timed suppression rather than attached parasite
  entities. Amphibious passenger capacity counts units, not RA2 transport-size weights.
- Chrono attacks use damage and suppression, not cumulative erasure. Spy/Mirage concealment affects
  targeting, rendering and minimap visibility, but there is no enemy uniform or tree transformation.
  Psychic Sensor detects concealment; it does not draw enemy order lines.
- IFV copies passenger weapons plus the Engineer repair role; it does not reproduce every RA2
  crew-specific turret. GI/Conscript remain conventional basic infantry, without a deployed GI
  sandbag stance. Tesla Troopers do not charge friendly coils; Prism towers do not network beams.
- Splash support damage is applied on firing; direct artillery damage still follows the existing
  projectile path. Nuclear impact is immediate at the clicked target, without a missile flight
  cutscene or lingering nuclear fallout. Storm/Curtain/Chronosphere use compact effects.
- Original map sizes, ore regeneration rules, repair rules and match victory rules are retained.
  Map schema is now version 2 with explicit water tiles; all shipped maps, generators and fixtures
  were migrated together. Existing external schema-1 maps need the new field/version.

## Files and systems changed

| Files | Responsibility |
|---|---|
| `assets/data/arsenal.ron`, `scripts/generate_ra2_data.py` | Checked category/domain/role metadata and, since RA1, the terms of the buildability rule; every shipped data file is generated from the committed RA2 reference tables (`crates/sim/tests/reference/`). |
| `assets/data/{units,economy,combat}.ron`, `assets/LICENSES.md` | Shared production, prerequisites, health, armor, weapons, economics and provenance. |
| `crates/sim/src/{arsenal,special}.rs` | Metadata loading; specialist state, abilities, interaction, transport, reconnaissance and carrier sorties. |
| `crates/sim/src/{definitions,data,combat_defs,lib,hash,schedule,event}.rs` | Bounded validation, public APIs, schedule integration, events and deterministic hashing. |
| `crates/sim/src/{command,economy_commands,economy,building,harvest}.rs` | Ordered commands, queues, prerequisites, placement, exits, power and refinery ownership. |
| `crates/sim/src/{map,pathfinding,movement,unit,combat,visibility,match_setup}.rs` | Mobility domains, combat gating, common unit creation, fog and unified human roster. |
| `crates/game/src/{sidebar,construction,special_controls}.rs` | Four tabs, production details/progress, specialist input, ability buttons and effects. |
| `crates/game/src/{units,buildings,minimap,skirmish,audio,main,visual_review}.rs` | Metadata loading, captured-unit visuals, concealment, starting-force copy and native captures. |
| `assets/maps/*`, map generators, simulation map fixtures | Explicit water map schema migration. |
| `crates/sim/examples/m7_{benchmark,soak}.rs` | Existing regression/performance scenarios load the shipped arsenal extension. |
| `crates/sim/tests/m9_arsenal.rs`, existing map/schedule tests, sidebar tests | End-to-end production/specialist coverage and regression adjustments for the intended schema/UI. No tests removed or ignored. |
| `README.md`, `docs/{DECISIONS,STATUS,CONSTRUCTION_SYSTEM}.md`, `docs/screenshots/m9` | Controls, decisions, evidence and report. |

## Validation evidence

Tests drive the actual deterministic schedule and commands, not a mock of menu state. Every one
of the 72 items is queued, charged, completed and either placed/sold or spawned/rallied in the
integration suite. Validation ran on the local Apple Silicon macOS environment using the configured
Cargo cache. Logs are retained there with the `m9-` prefix.

| Command / execution | Exit | Result / log |
|---|---:|---|
| `cargo fmt --all -- --check` | 0 | No formatting changes. |
| `git diff --check` | 0 | No whitespace errors. |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | No lint warnings; `m9-clippy-final.log`. |
| `cargo test --workspace` | 0 | **439 passed, 0 failed, 0 ignored**, including 20 M9 tests; `m9-tests-final.log`. |
| `cargo test -p sim --test m9_arsenal` | 0 | All 20 focused production/specialist tests; `m9-arsenal.log`. |
| `cargo build --release -p game` | 0 | Playable release binary; `m9-release-final.log`. |
| `cargo run --release -p game -- --smoke-test 600` | 0 | Actual 600-frame release window/menu/match run; `m9-smoke-release-final.log`. |
| `cargo run -p game -- --smoke-test 600` | 0 | Actual 600-frame debug run; `m9-smoke-debug-final.log`. |
| `MYGAME_VISUAL_REVIEW=... cargo run --release -p game` | 0 | Eight native captures, including all four production tabs; `m9-visual-final.log`. |
| `cargo run --release -p sim --example m7_benchmark` | 0 | Arsenal enabled: shipped mean **0.263 ms**, max **1.225 ms** against 5/50 ms budgets; 200 starting units, 2,000 measured ticks; `m9-benchmark-unified.log`. |
| `cargo run --release -p sim --example m7_soak` | 0 | Arsenal enabled: **3,168,000 ticks**, 297.6 seconds, peak heap **1.8 MiB**; 36,000 ticks per configuration, repeated with per-tick hash comparison; `m9-soak-unified.log`. |
| Original legacy-only soak execution | 0 | 3,168,000 ticks, 223.9 seconds, peak heap 1.7 MiB; `m9-soak.log`. |

The debug linker retains the pre-existing compact-unwind warning. Native window shutdown can emit
the existing harmless winit “unknown Window Id” warning. Both actual smoke processes returned 0.
The final benchmark/soak example edits only enable the same arsenal metadata used by the game;
they were separately compiled, linted and executed after the workspace regression run.

### Requested acceptance checks

| Check | Execution evidence |
|---|---|
| 1–2: four selectable tabs and complete roster | Sidebar UI tests, native captures of all four tabs; `exact_unified_roster_and_categories` asserts 14/15/15/28 and no Magnetron/YR extension. |
| 3: technology locks | Real rejected Apocalypse queue at starting tech; advanced-item Battle Lab terms checked; lost-prerequisite test pauses existing work; all-item test succeeds with infrastructure. |
| 4–5: costs and progress | Every item traverses `CommandQueue`/`SimSchedule` for its real build duration and exact full cost; low-power test verifies half-speed progression. |
| 6: valid placement | Every building traverses placement/sale; existing placement edge/fog/occupied/base-distance suites remain; new shoreline test rejects land and deep-water shipyard sites. |
| 7: producer and spawn | Every mobile type produces through its configured source, exits and accepts movement; blocked-exit/queue behavior remains covered by the existing economy suite. |
| 8: aircraft | Air-domain movement over water plus real strike/return/rearm test; carrier test observes real aircraft, delayed target damage and recovery without an airbase. |
| 9: navy | Water-only spawn and travel, shoreline factory validation, all naval types produced through Shipyard; amphibious navigation uses the shared domain rules. |
| 10: MCV | Deploys through a command and the resulting Yard successfully constructs a power plant. |
| 11: collectors | Both miners deliver credits to refineries; Chrono Miner return teleport and Purifier income bonus asserted. |
| 12: defenses | Automatic AA ignores ground targets; power loss stops powered fire; existing targeting/range/projectile regression suite retained. |
| 13: power | Queue slowdown, defensive shutdown, superweapon charge pause, supported generator prerequisites, refinery power infrastructure. |
| 14: strategic abilities | All four structures charged, activated through commands and reset; effects and repeat-activation rejection asserted. |
| 15: cancellation | Partial and fully paid cancellation, duplicate cancellation rejection, lost-prerequisite refund and exact credit conservation. |
| 16: compatibility | Full workspace regression suite; real debug/release 600-frame smoke tests; AI match matrix, benchmark and deterministic soak matrix. Shipped benchmark/soak now load arsenal metadata just as the game does. |

Additional specialist tests cover transport identity and IFV weapons, engineer capture, spy theft
and detection, cloning without recursive duplication, mind-control release, timed bombs, radiation
immunity, Drone suppression, gap/satellite/local sight and deterministic extended state hashing.

Native images are in `docs/screenshots/m9`. Layout was inspected by the implementing agent;
visual quality, sound quality, strategic balance and subjective game feel **require user verification**.
No independent review or approval tag is claimed by this implementation report.

### Storage and preservation

No repository clones, worktrees, virtual environments or substantial directories were created
outside the canonical repository. Build products and validation logs use the already configured
`/Users/xingyuwang/Library/Caches/Codex/cargo-target`; that existing cache remains useful for builds
and reproducible validation. No substantial artifacts were placed in `/tmp` or `/private/tmp`.
The pre-existing untracked `docs/HANDOFF.md` was preserved. No commit, tag or remote push was made.

## Complete production catalogue

Times assume full power, sufficient credits and an available exit. The producer is also a prerequisite.
Power values are output / consumption. These are live data values, not claims of exact RA2 balance.

### Buildings

| Item | Cost | Seconds | Producer | Additional prerequisites | Power |
|---|---:|---:|---|---|---|
| Construction Yard | 3000 | 60 | Construction Yard | — | 20 / 0 |
| Power Plant | 300 | 10 | Construction Yard | — | 100 / 0 |
| Tesla Reactor | 600 | 9 | Construction Yard | — | 150 / 0 |
| Ore Refinery | 2000 | 20 | Construction Yard | Power Plant | 0 / 40 |
| Ore Purifier | 2500 | 35 | Construction Yard | Battle Lab | 0 / 150 |
| Nuclear Reactor | 1500 | 35 | Construction Yard | Battle Lab | 2000 / 0 |
| Barracks | 300 | 12 | Construction Yard | Power Plant | 0 / 20 |
| War Factory | 2000 | 25 | Construction Yard | Ore Refinery | 0 / 30 |
| Naval Shipyard | 1000 | 20 | Construction Yard | Ore Refinery | 0 / 30 |
| Airforce Command Headquarters | 1000 | 20 | Construction Yard | Ore Refinery | 0 / 50 |
| Service Depot | 800 | 15 | Construction Yard | War Factory | 0 / 30 |
| Battle Lab | 1500 | 25 | Construction Yard | War Factory | 0 / 60 |
| Radar Tower | 1000 | 17.5 | Construction Yard | Ore Refinery | 0 / 50 |
| Cloning Vats | 2500 | 40 | Construction Yard | Battle Lab, Barracks | 0 / 100 |

### Armory

| Item | Cost | Seconds | Producer | Additional prerequisites | Power |
|---|---:|---:|---|---|---|
| Fortress Walls | 100 | 2 | Construction Yard | — | 0 / 0 |
| Pillbox | 500 | 9 | Construction Yard | Barracks | 0 / 0 |
| Sentry Gun | 500 | 9 | Construction Yard | Barracks | 0 / 0 |
| Patriot Missile System | 1000 | 15 | Construction Yard | Barracks | 0 / 50 |
| Flak Cannon | 1000 | 15 | Construction Yard | Barracks | 0 / 50 |
| Prism Tower | 1500 | 22.5 | Construction Yard | Radar Tower | 0 / 75 |
| Tesla Coil | 1500 | 22.5 | Construction Yard | Radar Tower | 0 / 75 |
| Grand Cannon | 2000 | 32.5 | Construction Yard | Battle Lab | 0 / 100 |
| Gap Generator | 1000 | 20 | Construction Yard | Battle Lab | 0 / 100 |
| Spy Satellite Uplink | 1500 | 25 | Construction Yard | Battle Lab | 0 / 100 |
| Psychic Sensor | 1000 | 17.5 | Construction Yard | Radar Tower | 0 / 50 |
| Chronosphere | 3000 | 80 | Construction Yard | Battle Lab, Airforce Command Headquarters | 0 / 200 |
| Weather Control Device | 5000 | 100 | Construction Yard | Battle Lab, Airforce Command Headquarters | 0 / 200 |
| Iron Curtain Device | 3000 | 80 | Construction Yard | Battle Lab | 0 / 200 |
| Nuclear Missile Silo | 5000 | 100 | Construction Yard | Battle Lab, Nuclear Reactor | 0 / 200 |

### Infantry

| Item | Cost | Seconds | Producer | Additional prerequisites | Power |
|---|---:|---:|---|---|---|
| GI | 200 | 4 | Barracks | — | — |
| Conscript | 100 | 5 | Barracks | — | — |
| Engineer | 500 | 8.3 | Barracks | — | — |
| Attack Dog | 200 | 4 | Barracks | — | — |
| Rocketeer | 600 | 10 | Barracks | Airforce Command Headquarters | — |
| Spy | 1000 | 16.65 | Barracks | Battle Lab | — |
| Tanya | 1000 | 16.65 | Barracks | Battle Lab | — |
| Chrono Legionnaire | 1500 | 25 | Barracks | Battle Lab | — |
| Sniper | 600 | 10 | Barracks | Radar Tower | — |
| Flak Trooper | 300 | 5 | Barracks | — | — |
| Tesla Trooper | 500 | 8.3 | Barracks | Radar Tower | — |
| Crazy Ivan | 600 | 10 | Barracks | Radar Tower | — |
| Yuri | 1200 | 20 | Barracks | Battle Lab | — |
| Desolator | 600 | 10 | Barracks | Radar Tower | — |
| Terrorist | 200 | 4 | Barracks | — | — |

### Vehicles

| Item | Cost | Seconds | Producer | Additional prerequisites | Power |
|---|---:|---:|---|---|---|
| Chrono Miner | 1400 | 23.3 | War Factory | Ore Refinery | — |
| War Miner | 1400 | 23.3 | War Factory | Ore Refinery | — |
| Grizzly Battle Tank | 700 | 11.65 | War Factory | — | — |
| Rhino Heavy Tank | 900 | 15 | War Factory | — | — |
| Infantry Fighting Vehicle (IFV) | 600 | 10 | War Factory | — | — |
| Flak Track | 500 | 8.3 | War Factory | — | — |
| Terror Drone | 500 | 8.3 | War Factory | — | — |
| V3 Rocket Launcher | 800 | 13.3 | War Factory | Radar Tower | — |
| Mobile Construction Vehicle (MCV) | 3000 | 50 | War Factory | Service Depot | — |
| Mirage Tank | 1000 | 16.65 | War Factory | Battle Lab | — |
| Prism Tank | 1200 | 20 | War Factory | Battle Lab | — |
| Apocalypse Tank | 1750 | 29.15 | War Factory | Battle Lab | — |
| Tank Destroyer | 900 | 15 | War Factory | — | — |
| Tesla Tank | 1200 | 20 | War Factory | Radar Tower | — |
| Demolition Truck | 1500 | 25 | War Factory | Battle Lab | — |
| Harrier | 1200 | 20 | Airforce Command Headquarters | — | — |
| Black Eagle | 1400 | 23.3 | Airforce Command Headquarters | — | — |
| NightHawk Transport | 1000 | 16.65 | Airforce Command Headquarters | — | — |
| Kirov Airship | 2000 | 33.3 | Airforce Command Headquarters | Battle Lab | — |
| Amphibious Transport | 900 | 15 | Naval Shipyard | — | — |
| Destroyer | 1000 | 16.65 | Naval Shipyard | — | — |
| Aegis Cruiser | 1200 | 20 | Naval Shipyard | — | — |
| Dolphin | 500 | 8.3 | Naval Shipyard | — | — |
| Aircraft Carrier | 2000 | 33.3 | Naval Shipyard | Battle Lab | — |
| Typhoon Attack Submarine | 1000 | 16.65 | Naval Shipyard | — | — |
| Sea Scorpion | 600 | 10 | Naval Shipyard | — | — |
| Giant Squid | 1000 | 16.65 | Naval Shipyard | Battle Lab | — |
| Dreadnought | 2000 | 33.3 | Naval Shipyard | Battle Lab | — |
