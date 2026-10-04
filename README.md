# MyGame001

An original single-player real-time strategy game for macOS, in the tradition of the classic
isometric base-building RTS: build a base, harvest ore, produce an army, and destroy every building
of the computer opponent before it destroys yours. Version 1.0 offers skirmish matches on three
maps with two factions against a computer player at three difficulties.

The game is written in Rust with [Bevy](https://bevyengine.org) 0.19. All rules live in the
`sim` crate (integer arithmetic, deterministic, tested headlessly); the `game` crate draws the
simulation and turns input into commands. `PLAN.md` is the project plan, `docs/DECISIONS.md` the
decision log, and `docs/STATUS.md` the milestone reports.

## Factions

| | Vanguard | Meridian |
|-|----------|----------|
| Style | quick and versatile | heavy and slow |
| Infantry (barracks) | rifleman, rocket soldier | conscript, grenadier |
| Vehicles (war factory) | scout, light tank, artillery | raider, heavy tank, rocket launcher |
| Defence | gun turret | rocket tower |
| Shared | construction yard, power plant, refinery (comes with a harvester), barracks, war factory, tech center, harvester | |

Artillery and rocket launchers need a tech center. Bullets are good against infantry, cannons and
rockets against vehicles, explosives against both, and artillery shells against buildings. The
values are in `assets/data/*.ron`.

## Maps

* **demo** (64 × 64): a rock ridge with a gap and a lake.
* **crossroads** (56 × 56): bases in opposite corners, three lanes between rock ridges and lakes.
* **riverlands** (64 × 48): bases on both banks of a river with three fords.

## Requirements

* macOS (version 1.0 targets macOS only).
* Rust through [rustup](https://rustup.rs); `rust-toolchain.toml` pins the toolchain (1.98.1),
  which rustup installs on first use.

## Building and running

```sh
cargo run --release -p game
```

The game opens on the main menu. Choose the map, your faction, the opponent's faction, and the
difficulty with the `<` and `>` buttons, then **Start**. **Options** sets the volume, sound on or
off, edge scrolling, and scroll speed; they are stored in
`~/Library/Application Support/MyGame001/settings.txt`.

## Release package

```sh
sh scripts/package_macos.sh
```

builds `target/package/MyGame001.app` (the executable with the `assets/` directory beside it) and
`target/package/MyGame001-macos.zip`, and runs the smoke test from the package. The application is
not signed: on another Mac, open it with a right click and **Open** the first time.

## Controls

### Camera

| Input | Action |
|-------|--------|
| Arrow keys | scroll the map |
| Cursor at a window edge | scroll the map (can be switched off in Options) |
| Middle mouse button, drag | drag the map |
| Mouse wheel or trackpad | zoom in and out |
| Left click or drag on the minimap | centre the view there |

### Units

| Input | Action |
|-------|--------|
| Left click on your unit | select it; a click on empty ground clears the selection |
| Left drag | select your units in the box |
| Shift + click or drag | add to the selection |
| Right click on ground | move the selection there |
| Right click on ore with harvesters selected | harvest there (other selected units move) |
| Right click on a visible enemy | attack it with the selected armed units |
| `A` | attack-move the selection to the cell under the cursor: fight enemies on the way |
| `S` | stop |
| Ctrl or Cmd + `0` to `9` | make the selection control group n |
| `0` to `9` | select control group n |
| Shift + `0` to `9` | add control group n to the selection |

### Base and production (sidebar)

| Input | Action |
|-------|--------|
| Left click on an item | queue it (only items your faction offers are shown; locked items need a prerequisite or producer) |
| Right click on an item | cancel the last queued entry, with a refund of what was paid |
| Left click on a building marked READY, or `B` | placement mode: the footprint is green where it can be placed and red where not; left click places it |
| `Tab` | switch between the Buildings and Units tabs |
| **Sell** button or `X` | sell mode: left click sells your building (refund of half its cost) |
| `Delete` or `Backspace` | sell your building under the cursor |
| **Rally** button | rally mode: left click sets where new units go |
| `R` | set the rally point to the cell under the cursor |
| `Escape` or right click | leave the current mode |

### Game

| Input | Action |
|-------|--------|
| `P` or `Pause`, or the **Pause** button | pause and resume |
| `-` and `=` | master volume down and up |
| `M` | mute and unmute |

### How to win

Destroy every building of the opponent; units do not count. Buildings need power: under low power
production slows down and defences stop firing. Harvesters bring ore to a refinery for credits.
Buildings can only be placed on ground you see, within four cells of your other buildings.

## Tests and validation

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release -p game
cargo run --release -p game -- --smoke-test 600
cargo run -p game -- --smoke-test 600
cargo run --release -p sim --example m7_benchmark
cargo run --release -p sim --example m7_soak
sh scripts/package_macos.sh
```

`--smoke-test <frames>` runs the real game for that many frames, plays the menu flow, and exits
with code 0 only if every check passes. The benchmark times ticks of 200 fighting units against the
budget in `docs/DECISIONS.md`; the soak plays computer players against each other for 36,000 ticks
on every map and faction pairing.

## Licences

Every sprite, map, and sound is generated by code in this repository (`crates/game/examples/`,
`crates/game/src/audio.rs`). The interface font is FiraMono, embedded by Bevy, under the SIL Open
Font License 1.1 (`assets/licenses/FiraMono-OFL.txt`). `assets/LICENSES.md` records the origin of
every asset file.
