# RTS controls and battlefield follow-up

The implementation reuses the existing Bevy input/UI systems, `CommandSink` → `SimHost`
command path, deterministic simulation schedule, player queues, harvest states, sprite ownership,
and TMX navigation grid. It builds on the existing uncommitted M9/HD work.

## Player-visible changes

- Hold the middle button, or Space + left button, and move slightly away from the press point.
  The camera keeps scrolling while held, even with a stationary cursor. Distance controls speed;
  returning to the anchor stops scrolling. Arrow keys and minimap navigation remain available.
  Focus loss cancels the gesture; optional edge scrolling defaults off for new settings.
- Box selection follows the visible unit bodies, previews selected units during dragging, and
  preserves its last valid endpoint if the pointer leaves the window. Double-click selects the
  visible units of that type; Shift and existing control groups remain available. Sidebar clicks
  cannot issue battlefield orders; accepted commands show a short destination marker.
- The command sidebar uses a narrower metal palette, smaller icons, six production cards per
  page, compact status badges, and detailed prerequisite information on hover.
- Right-click or attack-move on ore gives miners a harvesting order. Their assigned field is
  remembered through delivery to an owned refinery. They repeat collection and delivery, seek
  remaining reachable ore after depletion, and abandon the cycle on explicit Move/Stop.
- The skirmish menu includes **Super Me**. Only the opted-in human gets 10 production progress
  units per eligible tick and a total price of 1 credit per item. Existing prerequisites, power
  penalties, placement and exit rules still apply. Normal and AI players use unchanged prices
  and progress. Discrete completion times round up to simulation ticks.
- Six selectable player colors repaint faction accents, preserving neutral materials and
  lighting; the opponent receives a different color. Units, buildings and minimap use the
  same palette. Restart repaints from original pixels to avoid accumulated recoloring.
- Shared map generation/rendering adds roads, forest obstacles, roadside cottages, rocky
  terraces, varied ground, water detail and visible ore depletion. Connected roads use exposed
  curbs instead of repeating individual road borders. Navigation is taken from the actual TMX
  terrain. Base aprons, resource access and symmetric competitive layouts are checked.

## Scope and compatibility

The current match format supports two players, so color assignment guarantees two distinct
colors. The settings file format is unchanged. Super Me is match state and is included in the
deterministic hash; ordinary setup defaults to off. Restart retains the selected menu options.
There was no game-save serialization pipeline to migrate. Terrain elevation is represented by
blocked rocky terraces, not a new traversable height/ramp system; civilian cottages are static
obstacles, not capturable/garrisonable structures.

## Verification

- `cargo test --workspace`: 451 passed, including all 108 shipped AI match combinations,
  normal economy/production, mining, map connectivity/symmetry, command rejection and initialization.
- After final UI/smoke-flow edits, all 132 game binary tests passed again. The additional complete
  HD ownership test passed with the existing palette tests (4 tests). It checks every unit's eight
  directions and every structure through the production decoder/recoloring function.
- Normal and Super Me catalogue tests execute queueing, cost charging, completion and actual
  spawn/placement for all 72 constructible items. Human-only setup and unchanged AI prices/speeds
  are asserted separately. Miners are tested through two deliveries and an explicit override.
- Actual Bevy camera/input systems verify continuous stationary-cursor panning, focus cancellation,
  sprite-aligned box selection, release outside the window, sidebar exclusion and order feedback.
- Formatting, diff whitespace checks and strict workspace Clippy passed.
- Debug and packaged release 600-frame native smoke runs passed. The flow selects Super Me and a
  color through menu actions, verifies player isolation and restart persistence, disables the bonus
  for the final match, and checks normal production, AI commands, movement, pause and settings.
- Release benchmark: shipped 200-unit scenario averaged 0.207 ms/tick, maximum 1.109 ms;
  peak tracked heap 0.7 MiB. Both scenario budgets passed.
- Deterministic soak: 3,168,000 ticks passed in 274.6 seconds; peak tracked heap 1.8 MiB.

Native captures are in `docs/screenshots/rts-controls/`; the menu, production cards and combat
views were inspected. They are native renderer output, with the developed-base/combat scenes
explicitly labeled as art-review fixtures. Captures exposed and guided fixes for road borders
and long card labels. Automated checks do not establish the subjective fidelity to Red Alert 2.
Physical macOS mouse/trackpad feel and audio still require hands-on verification; computer-use
permission was unavailable in this session.

## Retained artifacts

Build outputs reuse `/Users/xingyuwang/Library/Caches/Codex/cargo-target`. The final
`package/MyGame001.app` and `package/MyGame001-macos.zip` there are useful deliverables and remain.
No clone, worktree, virtual environment or dependency cache was created. Asset-only test copies
created by this work were removed after checking for Git data; unrelated older copies were left
alone. Disposable validation copies now use the prescribed review-cache location and are removed
by their test. Logs are retained under `docs/validation/rts-controls/` in the repository.
