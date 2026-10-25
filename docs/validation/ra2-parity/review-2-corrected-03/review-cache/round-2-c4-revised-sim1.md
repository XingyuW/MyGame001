# Independent final review — SIM1 source slice

**Candidate:** `437d69e9ca2038384970c86368e690c1c056ef33`  
**Cumulative base:** `8230a955b251e9341d349f99eaeb86f7f8253664`

## Findings

**0 P0, 0 P1, 1 retained P2. No new source-level blocker was found.**

### P2 — Lost-carrier regeneration is not tested at the exact ready boundary

- Production timer: `crates/sim/src/flight.rs:690-695`
- Fixture test: `crates/sim/tests/ra4_spawns.rs:325-354`
- Shipped-data test: `crates/sim/tests/ra4_shipped.rs:863-912`

`Slot::Lost(ticks)` becomes `Ready` when `ticks <= 1`. The shipped test establishes that a Hornet remains lost after 400 ticks and is ready after another 410; the fixture waits 500 ticks. Neither pins `Lost` immediately before the configured 800-tick boundary and `Ready` exactly at it. This is the existing `SPAWN-01` / `R56-63` precision debt. Positive regeneration behavior is covered and no timer defect is demonstrated.

Expected follow-up: observe the actual loss-recognition tick, then assert `Lost` immediately before and `Ready` at the configured boundary without changing stock launch spacing or regeneration values.

## Source authentication and coverage

I independently checked the 689 entries in `docs/validation/ra2-parity/remediation-2/C-corrected-04/frozen-validation-inputs.json`: **689 matched, zero missing or mismatched**. The manifest file SHA-256 is `9df40e2ce6a42fdb0dadd8d20f1295842de69940429bea78e7b975ee854aec26`.

All SIM1 primary modules listed in the C4 ownership packet were inspected or authenticated through the unchanged cumulative review:

`building`, `command`, `economy`, `economy_commands`, `economy_defs`, `event`, `fileguard`, `flight`, `harvest`, `hash`, `id`, `lib`, `map`, `map_scale`, `map_subset`, `match_setup`, `movement`, `ore`, `pathfinding`, `rng`, `schedule`, `skirmish_defs`, `tactics`, and `unit`.

These files remain byte-identical to the previously reviewed corrected candidate. The later production change is confined to `ai.rs`, primarily owned by SIM2; I cross-checked its SIM1 economy/command interfaces.

## Requirements assessment

- **I1–I5, I8–I12 and determinism:** fixed schedule ordering, integer simulation, ordered collections and `SimRng`, command-mediated AI mutation, bounded loaders/arithmetic, and state hashing remain intact. The new `View::erased_buildings` field is derived temporary decision state, so it creates no persistent-state hash obligation.
- **I6′ / R6:** `Grid::room`, registration, claims, reservations, standing points, release, separate air occupancy, three infantry spots, vehicle exclusion and building-footprint exclusion remain coherent. No new fourth-infantry, mixed vehicle/infantry, duplicate-air or building-overlap route was found. Chosen subcell offsets and swept-path geometry remain documented assumptions.
- **I7 / economy:** queue charging, fractional progress, final remainder, cancellation refunds, placement admission, paid repair, captured production and dock service retain exact nonnegative-credit behavior. No free-production or unsigned-credit path was found.
- **RA1–RA3:** setup defaults, country/side restrictions, buildability, queues, power, placement, movement, armour/warhead interfaces and win conditions retain their reviewed behavior.
- **RA4–RA8 interfaces:** flight/bay state, harvest and refinery payment, recursive cargo ownership, command rejection ordering and AI public-command routing remain consistent.
- **C4 Temporal correction:** `ai.rs:1006-1010` derives erased owned buildings; sale selectors exclude them at `ai.rs:2793-2797`, `2821-2824`, and `2928-2931`. Garrison recovery at `2907-2916` selects the first nonerased host and waits when every host is erased, preserving recoverability without submitting a command native validation will reject. The set is deterministic and does not expose hidden enemy state.
- **C3 financing correction:** `ai.rs:2514-2522` requires both an owned refinery kind and visible ore before treating ore as recoverable income. Its consumers at `1435`, `2607`, and `2617` retain real service, worker, producer, payment and command-admission requirements.
- **R4:** map growth remains an option with the recorded default-off policy.
- **R7:** the documented 15-frame/20-tick speed conversion and its explicit Jumpjet assumption remain unchanged.
- **R29:** `crates/game/tests/m6_production_matches.rs:47-55` retains the 72,000 scaled limit; no limit increase or assertion weakening was found.
- **Debt integrity:** all **183** ledger IDs remain present and all retain historical aliases. This slice does not reclassify or erase any debt.

## Evidence still required

This is source-only review. I deliberately did not use the untracked `review-2-corrected-03` output because it is live and incomplete. Final acceptance still requires the completed frozen 13-command validation, full production and duel distributions under unchanged caps, soak/determinism, benchmark, smoke, package and asset failure/restoration evidence, followed by all five evidence addenda and the fresh lead’s adjudication.

All **51** visual, audio, readability, animation, pacing, gameplay-feel, human victory/defeat and packaged-platform checks remain **PENDING — requires user verification**.
