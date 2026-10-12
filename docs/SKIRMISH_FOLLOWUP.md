# Camera, unlocks, targeting, game speed and multiple enemies (2026-10-05)

Uncommitted follow-up on top of the M9/HD and RTS-controls working tree. Decisions are recorded
in `DECISIONS.md` ("Camera, unlocks, targeting, game speed and multiple enemies").

## 1. Vertical camera movement on macOS

**Root cause.** Edge scrolling (on in the user's settings) used an 8-pixel band at each content
edge and only while Bevy reported a cursor position. At the top the band lies directly under the
title bar (or under the menu-bar reveal zone in fullscreen); at the bottom it abuts the Dock trigger
and the resize region. Overshooting the 8 pixels moves the cursor out of the content view, Bevy then
reports no position, and scrolling stopped, so the player had to hold the cursor in a thin strip
against the frame. On the left and right the cursor stops at the screen edge, which is why those
directions worked. macOS cannot confine the cursor (winit supports only locking there).

**Change** (`crates/game/src/camera.rs`).
- A 28-pixel band on all four edges, with a speed ramp from 40 % at its inner border to 100 % at
  the edge, identical for every edge.
- Corner reach (72 px) for diagonals; diagonals are capped at unit speed.
- An exit latch: if the cursor leaves the window from near an edge, the camera keeps panning that
  way until the cursor returns or the window loses focus. This covers the title bar, menu bar,
  Dock, resize regions and adjacent screens.
- Eased velocity (0.05 s time constant), so the camera never lurches or flickers at the border.
- Hovering any interface button inside a band never scrolls; sidebar cards and strategic buttons
  stay usable.
- Arrow keys, rate scrolling (middle button / Space + drag), zoom, focus handling and the option
  to switch edge scrolling off are unchanged.

## 2. Unit unlocks

**Root causes.**
1. **Regression.** M9 replaced the human roster with the 72-entry arsenal. The original faction
   units vanished from the sidebar: hidden, not shown as locked.
   - Meridian (heavy, Soviet-style): grenadier, raider, heavy tank, rocket launcher, rocket tower.
   - Vanguard: rifleman, rocket soldier, scout, light tank, artillery, gun turret.
   - Both factions: harvester.
2. Five Soviet-style arsenal units (Tesla Trooper, Crazy Ivan, Desolator, V3, Tesla Tank) need the
   Radar Tower. It was on the last page of the Buildings tab because cards followed file order.
3. Naval units needed a shipyard on open water within 4 cells of a building. The shipped maps'
   water lies 18–28 cells from the human start, so naval production was reachable only by
   chaining buildings across the map.
4. A power-equivalence rule let any power plant satisfy the Nuclear Silo's Nuclear Reactor
   requirement, so that progression step was skipped.
5. The LOCKED badge hid the progress of a queued item that had lost a prerequisite.

No faction restriction, prerequisite typo or missing producer exists. Every chain starts at the
construction yard.

**Changes.**
- Human roster = arsenal ∪ every faction roster (`sim/src/match_setup.rs`). That is 84 items, all
  shown in the sidebar.
- Armed buildings outside the arsenal (gun turret, rocket tower) go to the Armory tab
  (`sim/src/arsenal.rs`).
- Basic generators remain interchangeable, but an advanced generator with prerequisites (Nuclear
  Reactor) is a real requirement (`arsenal.rs`).
- Shipyards may lie within 32 cells of an owned building: `SHIPYARD_PLACEMENT_DISTANCE` in
  `sim/src/economy_commands.rs`, shared by the command and the placement preview. The site must
  still be visible open water touching shore.
- Sidebar cards are ordered by tech tier within each tab (`construction.rs::tech_tier`). The Radar
  Tower is now on page 2, ahead of the Battle Lab.
- Queued progress stays visible on the badge.
- New human-only **Tech tree: ALL UNLOCKED** match option (`PlayerState::unlock_all`, set through
  `sim::setup_match_with_options`).
  - It waives prerequisites only; producers, cost, power, placement and exits still apply.
  - It is hashed in the existing Super Me byte, so state hashes are unchanged when it is off.
  - AI players are never affected.

## 3. Attack targeting feedback

`crates/game/src/selection.rs`: `preview_order` classifies what a right click would do, using the
same inputs as `issue_orders`. The result is shown before the click by the in-game marker, a label
beside the cursor, and the OS cursor:

| Intent | When | Marker and label | OS cursor |
|---|---|---|---|
| Attack | visible enemy unit/building, armed unit selected | **red** reticle on the cursor, red outline around the target, red `ATTACK` | crosshair |
| Move | ordinary ground (or enemy with only unarmed units) | green diamond, `MOVE` | default |
| Harvest | ore with miners selected | gold diamond, `HARVEST` | pointer |
| Friendly | your own unit or building | blue ring on the target, `FRIENDLY` | pointer |
| Invalid | off the map, or explored terrain none of the selected units can enter | grey ⊗, `CANNOT GO`; the click issues nothing | not-allowed |

Unexplored cells are not marked invalid (the preview reveals nothing about the shroud).

## 4. Game speed

`crates/game/src/game_speed.rs`.
- Speeds are 0.5×, 1×, 1.5×, 2×, 3× and 4×.
- `[` and `]`, or the sidebar's `Speed - / +`, change it; the current speed is always shown
  (gold when not 1×).
- The speed sets the `FixedUpdate` timestep (`1 / (20 × speed)`), i.e. the number of simulation
  ticks per real second. Movement, combat, cooldowns, projectiles, construction, production,
  mining, AI turns, superweapon charges and every other timer are per tick, so all of them scale
  together and the simulation is unchanged (deterministic; pathfinding budgets are per tick).
- Super Me's per-tick bonus multiplies with it.
- Pause stops ticks at any speed.
- Particles and infantry walk cycles follow a game clock that runs at the game speed and stops
  while paused; camera, menus and sounds keep real time.
- The speed is not saved in the settings file (schema unchanged).

## 5. Multiple enemy players

- **Schema.** Maps list 2 to 8 start positions (`MIN_/MAX_MATCH_PLAYERS`); a match uses the first
  n, and setup accepts 2 to that many players.
- **Shipped starts.** Demo has 6, Crossroads 6, Riverlands 4.
  - On the point-symmetric generated maps the starts come in mirrored pairs (`gen_maps.rs` spec),
    so each pair of bases is equal.
  - Every start has clear building ground, at least 9 ore cells within 10 cells, a land path to
    all others, and at least 12 cells to the next base.
  - New starts are on existing open terrain; no map tiles changed.
- **Menu.**
  - **Enemy players** (1 to the map's maximum, shrinking when a smaller map is chosen) and
    **Enemy force**.
  - A **PLAYERS** panel lists every player with its colour swatch, force and difficulty.
- **Independence.** Each computer player `PlayerId(k)` has its own start, base, credits, queues,
  vision and `AiController`. All of them are hostile to each other and to you (free-for-all).
- **Colours.** There are now 8 palette colours. The human uses the chosen one and computer player
  k the k-th colour after it, so all players are distinct. Sprites, minimap markers and the menu
  use the same assignment.
  - Copies for computer players beyond the first are painted on first use and freed with the match.
  - An HD unit sheet set is about 250 MB per colour, so painting every sprite per colour would cost
    gigabytes.
- **End of match.** Your defeat ends the match for you (DEFEAT screen, ticks stop) even while
  computer players remain. Victory requires every enemy to be defeated.

## Validation

All commands run on the working tree after the final edit.

- `cargo fmt --all -- --check`: passed. `cargo clippy --workspace --all-targets -- -D warnings`:
  passed.
- `cargo test --workspace --release --no-fail-fast`: **473 passed, 0 failed** (451 before).
  The suite includes the 108 shipped AI matches. New or extended tests:
  - `game/tests/m10_unlocks_and_enemies.rs` (real command path: queue → progress → place/exit →
    move order):
    - every one of the 84 human items has a reachable producer and prerequisite chain, and every
      map has shipyard water within reach of the human start;
    - from a construction yard on Demo, a human builds the chain (power, refinery, barracks, war
      factory, radar, Battle Lab, airforce, shipyard after scouting the lake, Tesla Coil, Prism
      Tower, rocket tower, gun turret). They train and command Soviet units (conscript, Tesla
      Trooper, Desolator, Rhino, Tesla Tank, V3, Apocalypse, Yuri, Kirov, Typhoon, Dreadnought),
      Allied units (GI, Sniper, Grizzly, Prism Tank, Tanya, Harrier, Destroyer) and restored
      faction units (grenadier, heavy tank, rocket launcher, artillery, rifleman, light tank). The
      AI opponent stays unchanged;
    - the Nuclear Silo requires the Nuclear Reactor. "Unlock all" lets the Tesla Coil and
      Apocalypse be built without prerequisites, while Yuri is still refused without a barracks (no
      producer); without the option the Tesla Coil is refused;
    - 6-, 6- and 4-player all-AI free-for-alls on Demo, Crossroads and Riverlands: every economy
      runs, each player has its own AI and ownership, each match ends with a winner, and the final
      state hash is identical across two runs;
    - human defeat with three AIs left: the human is defeated, there is no outcome yet, and the AIs
      continue.
  - Camera:
    - all four edges move the same distance and diagonals work;
    - edge behaviour is checked in windows from 800×500 to 3456×2234;
    - leaving through the top or bottom keeps scrolling, and focus loss or returning stops it;
    - easing; hovering a button never scrolls.
  - Targeting:
    - pure intent classification, with distinct colours and only Attack red;
    - a Bevy system test over a visible enemy shows a red `ATTACK` label and crosshair before the
      click, and the click issues the attack;
    - explored rock shows `CANNOT GO` and a not-allowed cursor, and the click issues nothing;
    - adjacent ground shows Move.
  - Game speed: in 2 real seconds every level runs 40 × speed ticks (±1), with the game clock
    matching; pause freezes the clock at 4×.
  - Menu and colours:
    - enemy count cycles 1 to 5 on Demo and shrinks to 3 on Riverlands;
    - five-player ids, starts and controllers are correct;
    - distinct colours for all 8 players under every choice;
    - lazy painting of a third player's copy preserves neutral materials, and the cache is freed on
      restart.
  - Sidebar: tech-tier order (Radar Tower before Battle Lab, on page 1–2), "unlock all" status
    text, and queued progress visible.
- Native smoke runs (debug and release, 600 frames). Three matches have 3 computer players each.
  - The first two use Super Me and ALL UNLOCKED, verified human-only, with distinct colours for all
    four players and independent AIs.
  - The last runs at 4×: fixed timestep 12.5 ms, no tick while paused.
  - 84 of 84 sidebar items; every computer player issued commands, with 0 rejected.
- Performance (release, measured in the free-for-all test):
  - 6 players: 0.12–0.14 ms mean per tick, 1.8 ms worst;
  - 4 players: 0.10 ms mean.
  - At 4× (80 ticks/s) that is about 1 % of a core on average.
- Native captures: `docs/screenshots/followup/` (menu with four enemies and the player list; a
  four-enemy match at 3× with the speed control). They exposed missing `·`/`×` glyphs in the
  interface font; the labels now use ASCII.

## Limitations

- The physical feel of edge scrolling on the user's Mac (trackpad and mouse, fullscreen Spaces)
  still needs hands-on verification. While the window keeps focus with the cursor outside (for
  example on a second screen), the exit latch keeps panning until the cursor returns or another
  app is clicked.
- Alliances/teams are not implemented: computer players are free-for-all, including against each
  other.
- Computer players still use their faction rosters and AI files (no arsenal units, no naval or air
  play).
- Fog keeps other players' bases hidden, as before; only the human's vision is rendered.
- Game speed is per session (not saved). Very slow frames (over 250 ms) cap catch-up as before.
- Two pre-existing labels (strategic-ability buttons and the "seconds • ready" production hint)
  also use glyphs the interface font lacks; they were left unchanged.
- The targeting preview cannot appear in screenshots of the OS cursor, and an attack-preview
  capture would require warping the user's mouse, so it is verified by tests rather than a capture.
- Balance with five opponents on one map (economy pressure, rush timing) was not tuned.
