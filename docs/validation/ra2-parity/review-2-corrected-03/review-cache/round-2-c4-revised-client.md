# Independent CLIENT source-only review

Reviewed frozen commit `437d69e9ca2038384970c86368e690c1c056ef33` against base `8230a955b251e9341d349f99eaeb86f7f8253664`.

**Result: 0 P0, 0 P1, 2 retained P2. No P0/P1 source finding remains in this slice.** This is source review only. The incomplete `review-2-corrected-03` run receives no validation credit.

## Findings

- **P2 — batched notices may exceed the cap.** [notices.rs](/Users/xingyuwang/GameProjects/MyGame001/crates/game/src/notices.rs:193) counts deferred additions but repeatedly selects removals from the unchanged query. Multiple events in one update can schedule the same oldest notice for deletion or exceed `MAX_NOTICES`. `announce` also lacks a cap. Expected: enforce the cap across the complete pending batch.

- **P2 — PNG reader retains a metadata/open race.** [tileset_image.rs](/Users/xingyuwang/GameProjects/MyGame001/crates/game/src/tileset_image.rs:166) checks path metadata before opening it. Replacement with a FIFO between those operations can still block the later read. Existing size, format, chunk and decode bounds remain correct.

## Source coverage

All 689 frozen inputs match the manifest byte for byte; manifest SHA-256 is `9df40e2ce6a42fdb0dadd8d20f1295842de69940429bea78e7b975ee854aec26`. All 40 `crates/game/src` files, asset generators, `assets/LICENSES.md`, and `scripts/package_macos.sh` are byte-identical to the reviewed C3 client scope.

I rechecked:

- Menu/options conversion to `MatchOptions` and `PlayerSetup`, RA2 defaults, restart behavior, countries, teams, difficulty, and extension isolation.
- R4 map growth remains an optional setting defaulting off.
- R7’s documented speed conversion and slower nominal pace remain assumptions requiring user feel judgment.
- R29’s raised match limit remains explicit and unchanged; native distribution/headroom evidence is still required.
- Sidebar lanes, dynamic prices, factory state, radar/minimap, fog/shroud, selection, context actions, hotkeys, groups, bookmarks, special controls, notices, synthesized audio, and event privacy.
- `SimHost` keeps mutable simulation state private; client gameplay mutations enter through command sinks.
- Honest-vision client presentation, apparent Spy identity, hidden submarine/mission exclusion, remembered scenery, bridge state, and public radar notices.
- AI command bounds and commands-only mutation. The new temporal correction builds an ephemeral set of owned erased buildings and excludes them from sale and garrison-deploy candidates at [ai.rs](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/src/ai.rs:1006), [ai.rs](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/src/ai.rs:2793), and [ai.rs](/Users/xingyuwang/GameProjects/MyGame001/crates/sim/src/ai.rs:2910). It adds no mutable/hash state or hidden enemy observation. Five native-style regression bodies cover frozen garrisons, alternative hosts, existing last stand, frozen sale, alternative sale, and recovery after real release.
- Asset provenance: shipped assets are repository-generated/authored or licensed and named in `assets/LICENSES.md`; the catalogue test enumerates all asset files and requires a license entry. Missile sprites trace to `scripts/hd_aircraft.py`; gem/crate art traces to `scripts/generate_ra5_world_art.py`.
- Packaging copies the release binary, full assets, licenses and README, then launches the packaged executable from outside the repository with asset-root environment variables removed.
- The package remains local arm64/macOS, unsigned, unnotarized and non-universal. Complete dependency notices, final package size and another-Mac launch remain open documented delivery debt.
- The 183-ID debt ledger remains intact. Client debt includes settings, sidebar, selection/groups, privacy/event audience, audio capacity, checked assets, smoke scope, window behavior, packaging, display, art aliases and licensing.

## User verification

All 51 checklist rows remain pending:

- Display/client: launch/menu readability; window scaling/text; camera pan/zoom; edge scrolling; selection precision; groups/bookmarks; menu/field isolation; owner/color readability.
- Lobby/world: RA2 defaults; options/restart; national trees; locks/lanes/refunds; placement; power/radar; mining/gems; crates; repairs; tech buildings; airport; garrisons; bridges; map environments/growth.
- Combat/specialists: weapon effects; infantry/crushing; promotion/crews; aircraft; missiles/wings; concealment; deploy modes; Spy; stolen tech; C4/Ivan; temporal/charging/support; Mirage/control; parasites/IFV/transports; radiation/death chains; major abilities; visibility; tactical controls; AI difficulty.
- Audio/delivery: music/effects; announcements; volume persistence; pause/pace; frame tails; normal victory; normal defeat; extension isolation; packaged launch; distribution/signing; overall art/audio acceptance.

The detailed procedures remain in [RA2_GAP_ANALYSIS.md](/Users/xingyuwang/GameProjects/MyGame001/docs/RA2_GAP_ANALYSIS.md:948).

## Evidence still required

The final decision still requires the corrected frozen source to complete:

- The full 13-command validation, including unfiltered workspace tests, release/debug 600-frame smoke, benchmark, deterministic soak, complete duels, packaging, and both data regenerations.
- All 756 production matches and required winner/distribution/headroom checks.
- Executable and packaged missing-asset failures followed by restored successful launches.
- Final result ingestion by all five independent slices, followed by a fresh lead review.
- User execution of all 51 visual, audio, feel and platform checks.

I made no edits, ran no Cargo/native gates, and created no artifacts. The only working-tree item remains the coordinator-owned untracked `docs/validation/ra2-parity/review-2-corrected-03/`.
