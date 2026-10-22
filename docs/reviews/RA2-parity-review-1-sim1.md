# Independent final review, round 1 — SIM1

Reviewed frozen TIP `23647de4220408d42eeef6522deb4d4541528452`, cumulative base `8230a955b251e9341d349f99eaeb86f7f8253664`. Review date: 2026-10-22 UTC. Reviewer is a fresh independent slice reviewer and implemented/remediated none of the programme. This covers the completed RA0–RA9 cumulative programme and retained baseline interfaces, rather than the superseded stopped-RA5 scope.

**Decision: NO-GO. Findings: 7 P0, 1 P1, 0 P2.** F01–F05 independently confirm the five SIM2 leads; these are duplicate panel claims to consolidate, not five additional globally unique defects. F05 includes an additional independently traced live-health boundary. F06–F07 are additional SIM1 findings. F08 is the shared incomplete validation/acceptance gate and should be counted once by the lead.

## Method and source binding

Read the common dispatch first, then actual SIM1 dispatch and live interruption evidence, original and continuation prompts, applicable PLAN invariants/severities, current review ownership/requirements supplement and substantive historical requirements, relevant RA1–RA9 plan equations/acceptance, R6/R7/R29 and related specialist/flight decisions, current GAP/handoff/integration/debt/status material. Statements in those documents were treated as claims.

Inspected production paths and changed state in `command`, `economy_commands`, `economy`, `economy_defs`, `building`, `match_setup`, `flight`, `harvest`, `movement`, `pathfinding`, `map`, `map_scale`, `map_subset`, `unit`, `tactics`, `fileguard`, `hash`, `id`, `rng`, and connected definitions/arsenal/rules/combat/special/crew/cargo consumers. Read the primary SIM1 RA9 native bodies, connected RA1–RA8 tests, completion audit interface, and late-twin body/receipts. Checked cumulative change descriptions/diffs and actual TIP implementation, not implementation summaries alone.

No Cargo, native game, benchmark, match, soak, package, mutation probe, source change, Git mutation, new checkout, or extra agent was run. Confirmed findings below are by complete relevant source paths, permitted by the dispatch; suggested regression scenarios have not been executed by this reviewer.

Independently recomputed the validator's factual guard from packet195 plus the exact 196/196b binding refreshes: **669 source/binding files, 64 authority spans, zero mismatches**. Independently rehashed all four current raw gate logs against the preserved interruption record: all hashes match. `git rev-parse HEAD` returns the frozen TIP. `git status --short` contains only the coordinator-owned untracked gate outputs, and no tracked source delta. These facts establish immutability, not gameplay or source-policy acceptance.

## Confirmed findings

### F01 — P0: IFV repair can revive a parasite-killed host or act after the healer died

**Location:** `crates/sim/src/special/vehicles.rs:361`, `:390`, `:412`, `:425`.

`vehicles::advance` creates `index = actors(world)` at line263. Its parasite loop then applies `detonate_one` immediately (lines286–302), and can set a host's actual Health to zero through damage or culling. The following repair loop selects its healer with `a.alive()` and target with `b.alive()`/`b.hp` from that same earlier snapshot. Neither selection rechecks current Health. The final write reads current Health but performs `hp.hit_points.saturating_sub(w.damage)`; shipped RepairBullet has Damage=-50, so a target reduced from 25 to -25 by Parasite Damage50 becomes +25. Its kill has already been recorded and the parasite may already have escaped, but Cleanup now sees a living host. Conversely, a repair IFV killed in the parasite pass can still select and heal another vehicle because its earlier Actor was alive.

This is real same-stage production ordering, not a direct zero-HP test setup: parasite damage precedes the IFV loop within the actual scheduled Economy call. The source path allows the scenario with a naturally damaged host and a ready Engineer IFV in range. Existing dead-pad/depot/harvester guards do not protect this separate path.

**Expected:** Only currently living healers act; only currently positive-HP patients can receive repair. Re-read current Health at action/selection and preserve a true kill. Add ordinary parasite-hit/repair regressions with a living boundary control, including the dead healer case.

### F02 — P0: FreeMCV treats a live embarked MCV as absent

**Location:** `crates/sim/src/crates.rs:595`–`:611`, connected `:439`–`:449`.

`collect` builds `soldiers(world)` from live map entities, and `open` invokes `stranded` before drawing ordinary contents. `stranded` tests only that on-map soldier table for a base unit and a live Building query for a building. It does not inspect `SpecialState::cargo` or descendants. Actual Interact removes an embarked MCV's ECS entity and preserves it as a Passenger. The shipped MCV's Size6 is allowed by the Landing Craft's SizeLimit6/Passengers12, so this is supported gameplay. With no buildings, more than1500 credits, the MCV aboard a live owned transport and another owned collector taking a land crate, `stranded` returns true and awards a second MCV.

The player's existing MCV remains owned and living: `check_win_condition` at match_setup.rs1185–1194 deliberately includes cargo descendants, and `refresh_limits` also traverses them. Thus the player is not properly defeated before collecting, and the inconsistency is observable in an ordinary match. Recursive cargo retention/hashing does not repair the missing FreeMCV query.

**Expected:** The no-base-unit condition includes live owned off-map descendants under root ownership, using the same ownership/lifecycle semantics as win/build-limit checks. Add crate controls for on-map MCV, direct/nested cargo MCV, root-owner change, and destroyed payload.

### F03 — P0: Specialist damage loses the actual source firepower multiplier

**Location:** `crates/sim/src/special/vehicles.rs:294`; `crates/sim/src/special/infantry.rs:204`.

The periodic parasite/squid path stores the original Passenger, including Experience, and correctly uses that Experience for reload and elite culling. However, its Blast hardcodes `firepower_pm: 1000`. Shipped Giant Squid has FIREPOWER among its veteran abilities; crate firepower and AI firepower are also represented in Experience. The stored multiplier therefore exists and is hashed, yet the periodic damage ignores it. Yuri's deploy-only PsiWave similarly constructs a Blast with1000 while the same unit's ordinary/IFV area-fire route uses normal Arms firepower. A firepower crate on Yuri consequently changes ordinary modeled damage but not his deployed250-damage PsiWave. Both use the shared armour path, so merely verifying that path is insufficient.

This contradicts the RA3 damage equation and RA4 per-source damage multipliers, also documented by `veterancy.rs`. An unmodified source is unaffected, which explains why base-damage tests alone cannot catch it.

**Expected:** Supply the real live Yuri multiplier and stored off-map parasite multiplier to damage resolution. Verify normal/veteran/crate/AI cases and unrelated armour/type-immunity behavior. This is one common multiplier omission finding spanning two effect constructors, not two counted findings.

### F04 — P0: Guard repeatedly selects an ineligible nearby enemy and starves legal targets

**Location:** `crates/sim/src/tactics.rs:168`–`:194`; connected `crates/sim/src/combat.rs:2411`–`:2416`.

Guard's chosen-target filter checks hostility, concealment, vision and anchor range, then takes the minimum Manhattan-distance/id tuple. It never asks whether either actual weapon can engage the target layer/armour or obey the ordinary automatic-acquisition floor. It installs that target as an explicit Attack every Pathfinding stage. Combat then correctly rejects an air target for a ground-only cannon with NoDamage and clears the attack. Next tick Guard chooses the same nearer target again. The persistent Guard branch at combat.rs2294–2304 prevents ordinary idle acquisition from rescuing the order.

For a concrete shipped example, put a visible hostile Rocketeer nearer to a guarding Rhino than a visible hostile ground vehicle, both inside GuardRange. Every tick the Rhino attempts the Rocketeer and never fires at the ground vehicle. An unreachable nearer target can similarly dominate selection. This is more than a cosmetic AttackEnded event.

**Expected:** Guard acquisition must apply the effective weapon's legal target selection/readiness policy before ranking candidates and must not perpetually suppress a legal target. Include mixed-layer, zero/one-Verses, minimum-range/reachability and effective secondary/elite/gunner controls, as applicable.

### F05 — P0: Pending flight volleys launch while their spawner is erased, and can launch after same-stage death

**Location:** `crates/sim/src/flight.rs:625`, `:662`–`:685`; connected `:535`–`:550`, `crates/sim/src/combat.rs:1729`–`:1751`.

Once `flight::fired` has created a pending Carrier/Dreadnought volley, `bays` retains it while Weapon.target remains equal. Combat's Temporal suppression skips firing but deliberately preserves that target/order. `bays` only checks `disabled.contains_key(id)` and does not check `erasures`. At the27-tick pending launch boundary it therefore invokes `launch` and emits UnitLaunched while the ship is being erased. Launch does not add an erasure guard. The actual state is attainable by starting the multi-unit volley, then applying an enemy Chrono beam before a later launch; the erasure lasts long enough for that boundary.

The same readiness location also trusts the old Actor.hp. `flight::advance` calls `spawned_units` before `bays`, and an arrived enemy missile calls `detonate` immediately. `detonate_filtered` changes actual Health immediately (combat.rs2140–2149). If that impact kills a carrier with a pending ready slot, `bays` still sees its positive pre-impact Actor.hp and can launch a new unit after death. Current target health is likewise read only from the earlier index. These are independently traced source paths, not a claim that a probe ran.

**Expected:** Pending launch effects honor current positive Health/owner and current suppression/target validity at the actual launch point. Preserve the documented missile-survives-launcher-death rule for missiles already launched. Add erasure and actual incoming-impact controls at a pending launch boundary, retaining exact27 spacing and the existing timer tests.

### F06 — P0: Temporally erased miners keep mining/paying, and Chrono Miners can teleport out of the beam

**Location:** `crates/sim/src/harvest.rs:394`–`:464` and `:285`–`:297`; `crates/sim/src/special/advance.rs:190`–`:199`.

`harvest` has current Health checks but no SpecialState input or erasure check. A War Miner standing on ore with no MoveOrder continues incrementing work and taking a bail at its harvest interval while its erasure remains active. A loaded miner already beside its refinery can execute `Context::unload` and credit payment during erasure as well. Temporal targets retain positive Health until the erasure completes, so liveness checks do not suppress these actions. Shipped War Miner has immune=0, and ChronoBeam permits its heavy armour; it is a legal target.

The connected Chrono Miner path is more severe: after the harvest becomes full (or a return was already pending), `special::advance` unconditionally teleports every Role::ChronoMiner in ToRefinery. Its `teleport` helper checks only cell legality/room. The later `infantry::advance` sets frozen Mobile.speed to0, which cannot undo the teleport. The miner can move to a distant refinery and cause `retain_beams` to drop the now out-of-range beam. All this occurs through the installed Economy schedule, without accepting a new illegal Move command.

**Expected:** Erasure suspends the worker's economic and special return actions, preserving state for legal resumption. Add a real beam over a due harvest boundary and an already-returning Chrono Miner, with release controls. Existing ra9_harvester_liveness proves command-dead workers cannot mine; it does not test this positive-HP frozen case.

### F07 — P0: Erased buildings still accept sale and Construction Yard repack commands

**Location:** `crates/sim/src/special/commands.rs:205`–`:275`; `crates/sim/src/economy_commands.rs:609`–`:632`.

The ordinary unit command path rejects selected erased actors, and `special::commandable` explicitly returns "Target is being erased". Deploy/Interact use that guard. Undeploy does not: after finding a live owned yard it checks the match option/type/room and converts the footprint to an MCV without calling commandable or consulting erasures. The economy BuildingIndex includes a temporally erased building because its Health is still positive. SellBuilding likewise has no erasure guard or SpecialState input and immediately refunds/despawns the building and records its survivors.

An owner can therefore sell a construction yard/power plant under a sustained enemy beam, obtaining credits and possibly crew instead of losing the erased object. A yard can also repack during the freeze, changing the body and erasure target semantics. There is no documented exception authorizing either action; plan4.13 requires the target frozen while fired on. This is a separate command-boundary omission from F06's autonomous worker behavior.

**Expected:** Building commands that act on a frozen source apply the same commandability contract before any queue/credits/occupancy/type/wreck change. Add actual Temporal + Sell/Undeploy tests and unchanged-state/later-release positive controls.

### F08 — P1: Required final validation and remaining acceptance evidence are incomplete

**Location:** `docs/validation/ra2-parity/ra9/summary.txt:1`; `gate-process.json:8`; `scripts/validate_ra2_parity.sh:73` onward; original prompt section7 and current V1/A acceptance requirements.

The actual required full13 run completed only01 fmt,02 diff-check and03 Clippy with exit0. Command04 was deliberately interrupted during the756-match test; six individual m6_production_matches cases show `ok`, but the seventh sweep/target and workspace never completed.05–13 were not reached. The original gate controller's RUNNING receipt is stale after its disappearance and coordinator-owned SIGTERM/empty-group audit. The interrupted run supplies neither a complete milestone gate nor a complete review gate. A pending scheduling/combined-run approval is not approval and supplies no execution result.

The current entire unfiltered audit target has not passed in this run. A canonical whole audit and focused repaired-six positives are narrower; historical159's175/6 failure and168's6-pass/175-filtered repair remain distinct. Existing exact Lost200/returned-carrierReload100 positives and27 spacing do not certify every promised original flight/lost-carrier regeneration boundary. Documentation's open scope must remain open until the lead establishes the exact required acceptance or records a substantively justified disposition.

**Expected:** Complete all required unchanged commands and relevant acceptance controls on corrected frozen source, preserving caps, coverage, historical failures and accurate primary/child/filtered counts. The lead should consolidate this shared blocker once, not once per slice.

## Verified production/acceptance coverage and limits

| Area | Source/test evidence and conclusion |
|---|---|
| Command order and atomicity | `process_commands` drains FIFO, flushes ordinary batches before abilities and sale/place/Mirage boundaries, then refreshes concealment/power. Public group validation finishes existence/ownership/duplicate/size/bounds/commandability/combat/waypoint checks before accepted-order replacement. Rejected unit orders retain old jobs; private AI witnesses are distinct hashed inputs and do not grant public exemptions. Existing ra1_commands and ra8_intents controls are present. Building erasure exception is F07. |
| Buildability | Actual `check_buildable` checks tech/stolen access/roster/options/prerequisite groups/factory kind+side/country/build limits. Captured-side production is decided by actual owned factory kinds, not a fake source getter. Queue capacities are per lane. `check_placement` checks ready head/visibility before occupied-cell details, then site/reach; allocator exhaustion occurs before consuming queue/credits/grid state. |
| I7 production | `advance_item` adds bounded fractional rate, caps wanted progress, pays ordinary steps then exact last remainder; unavailable whole progress is discarded, not converted to unsigned credit. QueueItem records progress/charged/fraction; cancel refunds actual charged; completed building placement consumes the ready head without another charge. RA2 property/native exact-cost/one-credit-short, lane/power/cancel controls remain. No extra arithmetic defect found by inspection. |
| Liveness | advance_production excludes current dead producers, harvest excludes dead workers/refineries, movement excludes current dead movers, and paid depot/pad service rereads current patient/host Health. Primary ra9_liveness, harvester_liveness, liveness_movement and refinery_repair_liveness actually establish death by commands with living boundary controls. They do not cover every later same-stage writer; F01/F05/F06 demonstrate missing interfaces. |
| Paid aircraft pad service | flight::docks first fills ammo, uses a fresh interval for paid health, clamps healing, charges only with enough credits, resets elapsed service on leaving/ineligibility. It rereads actual pad/aircraft owner+HP, power and erasures; captured-pad snapshots cannot pay/heal. Primary pad_health includes actual queued Engineer capture in the same Economy stage with owned fallback and verifies both ammo and HP; read assertions are meaningful. |
| Aircraft retry and targeting | ra9_airdock_retry uses shipped data, actual ammo expenditure and public blocker movement; occupied air cells cannot bank service time, and a fresh complete landed interval is required. This proves its bounded scenario only. Aircraft on dock remain air targets under the expressly retained model; surface building exclusion is not silently generalized into a prohibition on that air layer. Pending launch eligibility remains F05. |
| I6′ movement/placement | Grid room/register/claim_spot enforces one surface whole unit or three foot spots and one air occupant. Spawn/unload/teleport/factory/crew paths use register→claim→standing_point and release/unregister; movement reserves before entry and releases old cells on arrival. Crusher victims are released before the vehicle claims. Building footprints are checked before cover. Pathfinding is deterministic octile10/14 with stable tie-breaking and bounded request order. No additional demonstrated occupancy defect found. |
| Cargo and lifecycle | Weighted direct-child Size/SizeLimit/Passengers admission, ancestor refusal, stable IDs/HP/Experience/harvester/bay restoration, nested retention, root ownership, descendant loss, build-limit census and short-game survival are connected in production. Primary transport/lifecycle/buildlimit tests drive real Interact/Deploy/capture/failed landing/blocker release. Loaded IFV mode is restored. F02 is a remaining census consumer omission. |
| Setup and win | MatchOptions defaults retain MCV/short game/superweapons/repacks/shroud with extensions off; options/players/start/faction validation precedes setup mutations. Random country/start choices use SimRng in player order. Short game uses actual base-unit definitions, long game counts units/off-map descendants, and allies yield a winner. START-01 body covers21 maps/344 supported growth/player-count scenarios and genuine public exit paths within256 ticks; this is setup/exit evidence, not all-to-Won evidence. |
| R7 and R29 | Generator speed uses min(Speed×256 div100,255), infantry3/2, then exact rational3/4 rounding; JumpjetSpeed has its separate documented source assumption. Build time uses cost×BuildSpeed×1200/1000 and wall multiplier. The selected late-twin retains72000 M6 cap; ordinary map limits scale by span; ten-player all21's400000, fixed24000 schedules and36000 soak windows remain different scopes. No limit increase inferred from one scope to another. |
| Hash/determinism | Walked PlayerState, QueueKey/QueueItem, MatchState, Mobile/Step/Orders, Harvester, Grid cells/spots/air/domains/gems/mined/world features, TacticalState, CommandQueue private witness/action state, Bay/MissileFlight, Passenger/cargo and SpecialState hash entry points. Fields above are represented in deterministic iteration order. Grid navigation is an execution view, not a retained alternate rules state. No demonstrated hash omission found in the owned paths. The conditional legacy grid length encoding was examined; no concrete different valid simulation-state pair with equal encoded stream was established. |
| I1/I2/I4/I8/I11/I12 | sim dependencies remain headless and pinned; Unix libc is explicit and pinned. Simulation arithmetic uses integer checked/saturating/wrapping/widened operations in reviewed paths. The map subset's existing f32 parse only validates an external typed property; it does not become simulation state/arithmetic. Test-only clock use is not production randomness. Cached-system expects are on fixed internally registered plugin systems, not data-selected handlers. Fileguard uses O_NONBLOCK then opened-handle is_file plus take(limit+1); data/map callers use that checked byte buffer. Data's current bound is192KiB, not stale128KiB prose. |
| 64/65 and allocator | m7_factions actual fixture loads64 distinct producible buildings and exactly64 roster entries, then gets native preparse TooManyEntries at65 (before semantic duplicate). Definition shape bounds and typed reference/range checks precede indexed arrays. id.rs private test consumes last u32::MAX-1 allocation and compares exhausted ready-placement world against a control; no public test-only allocator constructor was added. Source inspection confirms checked_add never hands out u32::MAX. |
| Completion audit interface | completion-whole checks active mapped registry kinds/categories/domain/role and real Cost/HP neighbors, preserving physical required dependencies. Registry81 policy cannot waive active kinds; unit Soylent21 nonread is distinct from real building fixed refund/rule_refund and Cost-based XP. Reviewed the native economy consumer interface; full5022-atom admission is data/tests ownership and is not certified by these neighboring getters or this slice. |

## Late determinism and validation receipts checked directly

Read `ra9_native_twins_remain_active_after_36000_and_reach_the_same_winner`: two separate App/world constructions/load/setup calls; every native tick hashes/events/outcomes and I6′/I7 checks; actual Won after36000 and positive late fire/production/ore. The source does not clone an already-run world or tick a frozen winner to manufacture equality. Queue-visible I7 observations are accurately narrower than a record of already-exited completed-unit payments.

Independently rehashed all four raw187 logs against187-late-bounded-summary and read their actual outcomes: positive and restored-positive each1passed/0failed/6filtered, Won41738/72000, late444/8/6, final78c81bd069fca660 and trajectory5a8adabd01d20b05; RNG causal fails at native36001; observer-only production omission reaches the same world/winner but fails missing late ProductionComplete with444/0/6. Immediate and separate later restoration SHA records are present. Current test file SHA is exactly e7f3b317c98e63368ae0e0dba733d7ae55274dd080c18e200d6998b776e4e97d. Independent comparison of the restored-positive656-entry manifest with current paths finds only three changed audit census/binding TSVs (`legacy904-branch-files.tsv`, `ra9_model_census.tsv`, `ra9_scope_observation.tsv`), no missing files or runtime/test-source differences. This supports exact functional provenance, not preliminary performance or an unrun whole gate. Current interrupted04 also contains this named case's `ok`, without captured diagnostic output.

Read198 isolated all21/fixed3/example-guard summary and source/cap scope. All21 actual Won is recorded separately from fixed24000/soak-window observations; max mean3.161ms does not establish p99/max/busy-window budgets. I did not rerun those native processes or replace their exact configurations with a new sample.

## Suspected / further verification, not additional confirmed findings

Other Temporal-building economic paths deserve explicit resolution during remediation: advance_production excludes disabled but not erasures; update_power still counts erased generators/radars; repair_docked guards an erased patient but does not check erasure on its host; building auto-repair and superweapon charging similarly have different suppression checks. Their actual source omissions are observable, but this slice has not established every original-engine/per-player-economic exception and does not manufacture separate P0 claims for all of them. F05–F07 already establish concrete incorrect action/command/movement results. Resolve connected boundaries with actual source-policy requirements and native controls, not an assumption that a zero movement speed freezes every other subsystem.

No native reproduction of the seven defects was run under the explicit no-native review lease. Corrected-source regressions, unfiltered complete audit, complete validation, all exact pending flight boundaries, and source-policy admission after remediation remain unverified. I have not independently re-extracted every numeric reference key or adjudicated all1911 intentional/nonread dispositions; those retain data/tests/fresh-lead ownership. Full client App, presentation privacy/art/audio/packaging acceptance remains the separate client slice and required gates, not credited from simulation source.

## User-only verification and storage

All visual/audio/readability/animation/pacing/feel and ordinary human victory/defeat/platform/package launch HOWTOSEE items remain pending for the user. They are not review findings and this report supplies no approval tag.

No substantial directory was created outside the canonical repository by this reviewer. Only this report was written into the pre-existing approved review cache directory; it remains necessary as independent review evidence. No repository/worktree or other artifact was deleted.
