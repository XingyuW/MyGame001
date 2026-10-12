# MyGame001

An original single-player real-time strategy game for Apple Silicon macOS, in the tradition of the classic
isometric base-building RTS: build a base, harvest ore, produce an army, and destroy every building
of the computer opponent before it destroys yours. It offers skirmish matches on eight maps,
from small duels to large battlefields with cities, coasts and different environments, with a
unified human arsenal, two opponent armies and three difficulties.

The game is written in Rust with [Bevy](https://bevyengine.org) 0.19. All rules live in the
`sim` crate (integer arithmetic, deterministic, tested headlessly); the `game` crate draws the
simulation and turns input into commands. `PLAN.md` is the project plan, `docs/DECISIONS.md` the
decision log, and `docs/STATUS.md` the milestone reports.

## Unified arsenal

The human player has a combined Allied/Soviet RA2-inspired arsenal: **Buildings | Armory |
Infantry | Vehicles**. All 72 requested items are available through a progressive technology tree;
aircraft and naval units stay under Vehicles. The starting-force choice only changes your initial
units. The opponent retains the existing Vanguard/Meridian AI armies and build orders.

Build power and ore income first, then Barracks and War Factory. Radar/Airforce Command unlock
specialists; Battle Lab infrastructure unlocks advanced units and strategic weapons. Cards show
cost, missing prerequisites, queue progress and ready/blocked states. Queues charge as work proceeds;
cancellation refunds exactly the credits already charged. A completed building waits for valid
placement. Low power slows production and stops powered defenses and strategic charging.

[Construction system report](docs/CONSTRUCTION_SYSTEM.md) documents the complete roster,
mechanics, integration, tests, and deliberate differences from RA2. Balance and artwork remain
original; this is not a byte-for-byte recreation of RA2's rules or assets.

## High-resolution battlefield presentation

The desktop client now uses a separate HD art set: **31 building identities and 53 unit
identities**, with eight facings per unit and four texture pixels per world pixel. The art
keeps an isometric battlefield, bold blue and red paint, oversized industrial equipment,
compact armour, gold hazard markings, and distinct infantry, aircraft and ship silhouettes.
Soviet reactors, Tesla equipment and heavy armour have their own red industrial designs;
Allied facilities use blue trim, pale metal, glass and cleaner shapes.

The rebuild adds detailed roof equipment, tank tracks, gantries, pipes, windows, aprons,
soft baked shadows, sixteen variations per terrain class, shoreline foam, water shimmer,
industrial exhaust, damage smoke, muzzle flashes, shell impacts, electric arcs, energy beams,
explosion clouds, shockwaves, debris and fading scorch marks. Production cards use larger HD
artwork, and closer camera zoom remains sharp. All presentation effects respect fog of war;
particle lifetimes pause with gameplay and are cleared when a match changes.

Field units are rendered as real equipment is finished: vehicles in three-colour disruptive
paint with dust, mud and chipped edges, soldiers in printed camouflage under darker
load-bearing kit, under a harder sun than the buildings and with a thin dark contour so that a
camouflaged tank still reads on ground of its own colour. Every unit that goes on foot has a
**walk cycle** (six frames in each of the eight facings, `assets/sprites/hd/walk`): the legs
stride, the knee folds as a foot swings forward, the body bobs and the shoulders turn against
the hips; commandos, irregulars and dogs run. The cycle advances with the ground covered, not
with the clock, so feet stay planted instead of sliding.

Weapons look and sound like what they are (`crates/game/src/weapon_fx.rs`,
`crates/game/src/audio.rs`). A prism weapon fires a beam of light: a white core in a blue
halo edged with the colours of a spectrum, a glare at the emitter and where it strikes, and
the ring of struck crystal. A Tesla weapon throws a lightning bolt that forks and jumps to a
new path many times a second, with the snap and 110 Hz buzz of an electric arc. Rifles crack
and send tracers, machine guns fire bursts, tank guns boom with a muzzle blast, smoke and a
dust ring, shells and rockets fly (rockets on a smoke trail, artillery on an arc) and burst
where they land with a sound of their own.

This remains a **2D isometric renderer**. Lighting and model shadows are baked into the
sprites; the animated light glows are composited effects. It does not yet have real-time 3D
shadows, physically based reflections or a general weather system.
No original game assets are bundled. Art quality and faithfulness remain subject to visual
review; the captures below show the implemented result.

Generate the original art (Blender 5.x for the pre-rendered miniatures; `uv` supplies Pillow
and NumPy). The full Blender pass of the buildings and units takes roughly an hour on Apple
Silicon:

```sh
uv run --with pillow --with numpy python scripts/generate_hd_terrain.py
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup --python scripts/render_hd_blender.py
uv run --with pillow --with numpy python scripts/finish_hd_art.py
```

Renders go to `~/Library/Caches/Codex/reviews/MyGame001/ra2-visual/render` by default; pass
`-- OUT_DIR b:<building> u:<unit> w:<unit> t:<terrain kind>` to the Blender step to re-render a
subset (`w:` is an infantry unit's walk cycle, 48 frames; terrain tops are rendered only when
named; `f0`..`f7` limit a test render to those facings). The walk cycles of the seventeen
units on foot take about as long again as all the unit sheets together.

Buildings and units are modelled from real-world references, in metres: `scripts/hd_kit.py` is
the modelling kit and material palette, `hd_arch.py` the architectural parts, and
`hd_buildings*.py`, `hd_vehicles.py`, `hd_infantry.py`, `hd_aircraft.py` and `hd_naval.py` hold
one function per building or unit. `scripts/generate_hd_art.py` keeps the sprite canvas, the
quick flat Pillow preview (`python scripts/generate_hd_art.py [OUT_DIR]`) and the helpers of the
terrain tops.

The legacy generators remain for their validation fixtures. HD artwork is loaded by definition
ID, checked through the bounded PNG loader, and rendered at an explicit world size so texture
resolution never changes unit placement or building footprints. No simulation rules change.

Native renderer captures are in `docs/screenshots/hd/`, including the developed base and the
mixed-technology review setup. Run the visual-review command below to regenerate them.

## Maps

* **demo** (64 × 64): a rock ridge with a gap and a lake.
* **crossroads** (56 × 56): bases in opposite corners, three lanes between rock ridges and lakes.
* **riverlands** (64 × 48): bases on both banks of a river with three fords.

Large maps, painted at or near the largest size the engine draws (256 × 256 cells, four times the
area of the earlier 128 × 128 files), so that bases lie far apart: the first two start positions
are the two farthest from each other, 190 to 276 cells apart on most maps (up to 137 before). Each mixes a
settlement, open water and nature, and scatters landmarks over the country between the bases:
copses, ponds in their marsh, crags, hamlets, farms in their fields, oases, ruins, small ore
fields. Six to ten start positions in the file.

* **liberty_bay** (256 × 256): a metropolis of towers and city blocks split by a river between two
  bays; avenues and country bridges cross it; a port town with piers on each bay, marshes on the
  river banks, farmland in crop, woods, lakes and suburbs around.
* **frostline** (224 × 224): arctic snowfields and pine forests, a snow-capped mountain chain with
  three passes, thawed valleys with farms, frozen lakes, pack ice along two coasts of an arctic sea
  and an industrial town at the centre.
* **oasis_gulf** (256 × 192): desert dunes and sandstone mesas between two gulf coasts, palm
  oases, adobe villages, a farmed green belt along the highway and a harbour city with towers.
* **coral_archipelago** (256 × 256): tropical islands with beaches, jungle, palms and mangrove
  shallows, joined by broad causeways; a resort city on the central island. Naval power matters.
* **continental_divide** (256 × 256): every environment at once: snowy highlands with frozen
  lakes, desert with sea inlets and temperate farmland around a lake with an island city reached
  by four bridges.
* **steel_harbor** (256 × 256): a whole-map city of boulevards. A river winds between two harbour
  basins with piers and splits around a downtown island of towers; avenue bridges cross it and the
  bases sit in cleared industrial lots, with parks and ponds holding the contested ore.
* **glacier_pass** (240 × 240): green valleys with farms, woods and fjords on the west and east,
  divided by a snowy massif. A winding pass in the north and south, a narrow goat trail on each
  side and a central pass around a glacial lake rimmed with ice are the only ways across.
* **red_canyon** (192 × 192): a red sandstone plateau cut by canyons. Each base has its own basin;
  canyons lead to a ruined desert town around an oasis pool at the centre, and long rim canyons
  past palm oases join neighbouring bases as flanking routes.
* **jungle_delta** (256 × 224): a tropical river delta. A great lagoon with jungle islets fills the
  centre; distributaries lined with marsh and mangrove cut the jungle into islands joined by sand
  bars and timber bridges, with fishing villages and two river ports. Naval power controls the
  middle.

Ocean maps (six starts each). Their land is one piece without loops, so the sea is one piece too:
a fleet can sail from any shore to any other, while armies have a single way across.

* **pacific_atoll** (256 × 256): a ring of reef islands around a lagoon in the open ocean. Two
  channels cut the ring into two arcs with three bases each; a broad winding sand bar joins each
  arc to the island in the middle of the lagoon. Palm islets, jungle and fishing villages.
* **iron_isthmus** (256 × 224): two continents in the sea, joined by one neck of land with a port
  town on it. Capes, lakes in their marsh, farmland and woods, and a sand spit out to an ore
  island on each side. Fleets can sail around either continent and shell any coast.
* **arctic_floes** (240 × 240): islands of snow and rock in a polar sea, joined by shelves of sea
  ice that armies cross and nobody can build on, past icebergs. A broad shelf joins each home
  island to the middle island, the only way across by land; each home island also has a northern
  and a western side island of its own at the end of a narrower shelf.

Six maps made for the largest size, each around a kind of country the others do not have:

* **ember_caldera** (256 × 256): a volcanic island. A lava lake fills the crater; a ring wall of
  basalt with four passes encloses the caldera floor and its rich ore; four lava rivers run to the
  sea and cut the island into quarters, crossed only where the flow has cooled. Ash above, jungle
  and fishing villages below, the sea all around.
* **frozen_sound** (256 × 256): two snowbound shores and a sheet of sea ice between them, cut by
  leads of dark water and studded with icebergs; islands frozen into it hold ore and a ruined
  research station. The whole middle of the map is open manoeuvre ground where nothing can be
  built.
* **harvest_plains** (256 × 256): farmland to the horizon: a patchwork of grain fields, ploughed
  earth and pasture with hedgerows, tracks and villages. Open tank country; a river with marshy
  banks crosses the middle, with three bridges and two broad fords.
* **black_water** (256 × 256): a swamp. Nearly all of it is marsh, cut by bayous and mangrove
  thickets and crossed by straight timber causeways; the only building ground is on dry hummocks,
  one under each base and smaller ones worth fighting for. A drowned city lies in the middle.
* **thousand_lakes** (256 × 256): a lake district: lakes of every size, frozen on the snowy fells
  in two corners, and between them a web of necks and peninsulas with many ways round and none of
  them straight; roads cross the lakes on long bridges.
* **wheel_of_worlds** (256 × 256): ten sectors around a ruined city, two each of farmland, snow,
  volcanic ash, desert and jungle, with a ridge on every border that opens only at the hub and on
  the ring road. One base in every sector.

Five grounds came with these maps. **Ice** and **marsh** carry units but no buildings, so a
frozen lake or a swamp is ground to fight over, not to settle. **Lava** blocks armies and ships
alike. **Fields in crop** and **volcanic ash** are ordinary building ground with a look of their
own.

A map file of the largest size is played as it is by any number of players (the smaller ones
still grow with the players, up to that size); start positions a file lacks are made as before.

Cities, forests, mountains and houses block movement. Forests, houses, city blocks and towers
are **destructible scenery**: hover one to see its name and health, select armed units and
right-click it to destroy it (the cursor shows ATTACK). Trees fall quickly, towers take a while;
bombs and strategic weapons flatten what they cover. Nobody fires at scenery unasked. The cleared
cell becomes open, buildable ground, so you can cut a road through a forest or level a city block
for a base. Mountains and cliffs stay. Bridges carry land units over water but
ships cannot pass under them. On these maps computer armies grow with the distance they cover and
flank instead of meeting head-on.

### Up to ten players, on maps that grow with them

Every map can be played by 2 to 10 players: you and 1 to 9 computer players. The sizes above are
those of a one-against-one match. **Every further player adds an eighth to the map's width and
height**, so ten players fight on a map twice as wide and high (four times the ground): Crossroads
grows from 56 × 56 to 112 × 112 cells, the 128 × 128 maps to 256 × 256, the largest a map can be.
The map row of the menu shows the size for the players chosen, e.g. `liberty bay  176 x 176` with
four enemies. The enlarged map is the same map, stretched: every forest, river, city and ore field
is where it was, larger, and all that was connected stays connected, over land and by sea. Each
map file lists four or six start positions; the players beyond them get bases of their own, made
when the match starts: on dry ground reachable from yours, as far from every other base as
possible, in mirrored pairs so that they are as fair as the map, each with cleared building ground
and an ore field. The map depends only on the map chosen and the number of players, so a restart
plays on the same ground.

## Requirements

* macOS (version 1.0 targets macOS only).
* Rust through [rustup](https://rustup.rs); `rust-toolchain.toml` pins the toolchain (1.98.1),
  which rustup installs on first use.

## Building and running

```sh
cargo run --release -p game
```

The game opens on the main menu. Choose the map, your starting force, the number of enemy players
(1 to 9 on every map; the map grows with them, see above), the enemy force, the difficulty, the **game speed** the match starts at (0.5× to 4×), Super Me,
the Super Me **unit speed** (1×, 1.5×, 2×, 3× or 4×), the tech tree (**STANDARD** or **ALL UNLOCKED**) and your team color with the `<` and `>`
buttons; the **PLAYERS** panel lists every player of the match with its color. Then **DEPLOY**. **Options** sets the volume, sound on or
off, edge scrolling, and scroll speed; they are stored in
`~/Library/Application Support/MyGame001/settings.txt`.

## Release package

```sh
sh scripts/package_macos.sh
```

builds `$CARGO_TARGET_DIR/package/MyGame001.app` (or `target/package/MyGame001.app` when no custom target directory is configured) (the executable with the `assets/` directory beside it) and
`MyGame001-macos.zip` in that same package directory, and runs the smoke test from the package. The application is
not signed: on another Mac, open it with a right click and **Open** the first time.

## Controls

### Camera

| Input | Action |
|-------|--------|
| Arrow keys | scroll the map |
| Cursor near a window edge | scroll the map (can be switched off in Options): a 28-pixel band on every edge, faster towards the edge, diagonal near corners; leaving the window through an edge (title bar, Dock, screen edge) keeps scrolling until the cursor returns or the window loses focus; interface buttons inside a band never scroll |
| Hold middle mouse, or Space + left mouse, then displace the pointer | continuous camera scrolling; hold still to keep moving, return to the starting point or release to stop |
| Mouse wheel or trackpad | zoom in and out |
| Left click or drag on the minimap | centre the view there |

### Units

| Input | Action |
|-------|--------|
| Left click on your unit | select it; double click selects the same type on screen; empty ground clears selection |
| Left drag | select units by their visible body centers, with live highlighting; the last endpoint is retained if the cursor leaves the window |
| Shift + click or drag | add to the selection |
| Right click on ground | move the selection there |
| Right click on ore with harvesters selected | assign that field; miners unload at their own refinery and return automatically (other units move) |
| Right click on a visible enemy | attack it with the selected armed units |
| Hover before right-clicking | the cursor previews the order: red reticle, red target outline and **ATTACK** over a valid enemy; green **MOVE**; gold **HARVEST** over ore with miners; blue **FRIENDLY** over your own units or buildings; grey **CANNOT GO** (and no order) over terrain none of the selection can enter or off the map |
| `A` | attack-move the selection to the cell under the cursor: fight enemies on the way |
| `S` | stop |
| `D` | deploy an MCV, unload a transport, or contaminate the area with a Desolator |
| `E`, then left click | engineer repair/capture, spy infiltration, or load selected ground passengers into a friendly transport |
| Ctrl or Cmd + `0` to `9` | make the selection control group n |
| `0` to `9` | select control group n |
| Shift + `0` to `9` | add control group n to the selection |

### Base and production (sidebar)

| Input | Action |
|-------|--------|
| Left click on an item | queue it (all unified items are shown; locked items explain missing prerequisites or producers) |
| Right click on an item | cancel the last queued entry, with a refund of what was paid |
| Left click on a building marked READY, or `B` | placement mode: the footprint is green where it can be placed and red where not; left click places it |
| `Tab` | cycle Buildings, Armory, Infantry and Vehicles |
| Strategic ability button, then map click | activate a charged superweapon; select vehicles first for Chronosphere |
| **Sell** button or `X` | sell mode: left click sells your building (refund of half its cost) |
| `Delete` or `Backspace` | sell your building under the cursor |
| Left click on your War Factory, Barracks, Cloning Vats, Naval Shipyard or Airforce Command | select that building: green brackets mark it and its rally point is shown as an orange flag |
| Right click on the map with such a building selected | set **that building's** rally point (the cursor shows RALLY POINT): every unit it builds from then on goes there; the Cloning Vats send their copies to their own point, apart from the Barracks' |
| **Rally** button | rally mode: left click sets where new units of **every** producing building go |
| `R` | set the rally point of every producing building to the cell under the cursor |
| `Escape` or right click | leave the current mode |

### Game

| Input | Action |
|-------|--------|
| `P` or `Pause`, or the **Pause** button | pause and resume |
| `[` and `]`, or the sidebar's **Speed** `-` `+` | game speed 0.5×, 1×, 1.5×, 2×, 3× or 4× (shown in the sidebar); it changes how many simulation ticks run per second, so movement, combat, production, mining, AI and every timer scale together |
| `-` and `=` | master volume down and up |
| `M` | mute and unmute |

### How to win

Destroy every building of every enemy; units do not count. With several enemies every computer
player has its own base, economy, AI and color, and they are **allied against you**: they never
fire at each other or march on each other's bases. You lose
as soon as your last building falls. Buildings need power: under low power
production slows down and defences stop firing. Harvesters bring ore to a refinery for credits.
Buildings require visible, unoccupied terrain within four cells of your base. Shipyards require a
visible water footprint touching shore within 32 cells of your base (scout the water first; every
shipped map has suitable water within that reach of your start); naval units stay in water, amphibious transports can use either, and
aircraft can cross land and water. Harriers and Black Eagles return to Airforce Command to rearm.

A new unit comes out beside its building. When every cell around the building is taken, it comes
out further away instead (the nearest free ground it could walk to, on the side of the rally
point), so a crowd around a factory never stops production; set a rally point to keep the exit
clear.

Aircraft, airships and rocketeers **fly**: they take off where they are built, cruise high above
the ground over their own shadow, and pass over units, buildings, cliffs and water. A selected
aircraft shows a plumb line down to the ground cell it is over. Harriers and Black Eagles land
beside their Airforce Command while they rearm.

When a player loses its last building, a notice in the top left corner says so in that player's
team colour ("Red Player has been defeated").

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
cargo run --release -p sim --example m14_duels -- full
sh scripts/package_macos.sh
```

`--smoke-test <frames>` runs the real game for that many frames, plays the menu flow, and exits
with code 0 only if every check passes. The benchmark times ticks of 200 fighting units against the
budget in `docs/DECISIONS.md`; the soak plays computer players against each other for 36,000 ticks
on every map and faction pairing. `m14_duels` plays computer players one against one to the end
on every map (all pairings of factions and difficulties with `full`, a few maps by naming them,
`trace` to see where a slow match stands) and reports the tick each match ended in: the first
check after adding or reshaping a map. The tests that play whole matches on every map use all
cores; `cargo test --workspace --release` runs them several times faster.

## Licences

The music is an original industrial-metal march (boots, siren, down-tuned guitars) synthesised at startup; run `MYGAME_DUMP_MUSIC=music.wav cargo test -p game --bin game the_music_can` to write it as a WAV file.

Every sprite, map, and sound is generated by code in this repository (`crates/game/examples/`, `scripts/generate_hd_art.py`, `scripts/generate_hd_terrain.py`,
`crates/game/src/audio.rs`). The interface font is FiraMono, embedded by Bevy, under the SIL Open
Font License 1.1 (`assets/licenses/FiraMono-OFL.txt`). `assets/LICENSES.md` records the origin of
every asset file.


For visual review, create an output directory and run
`MYGAME_VISUAL_REVIEW=/absolute/output/directory cargo run -p game`.
The game saves native renderer captures of the menu, an ordinary start, a labelled developed-base
setup, and ordinary starts on Crossroads (Meridian) and Riverlands (Vanguard), then exits.
The developed base uses a separate pre-tick setup; it does not change the shipped match data.
The mode is ignored during `--smoke-test`. It is a presentation aid, not a gameplay test.

### Skirmish options and battlefield controls

Before starting, choose **Super Me** and a **Team color**. Super Me defaults off. When on,
only the selected human player pays **1 credit per production item** and advances construction
and unit production **10 times faster**. Power shortages, prerequisites, queue limits and
exit/placement rules still apply. With Super Me on, the menu's **Unit speed** row also makes
your units move up to 4 times faster; it is separate from the game speed, which speeds up
everyone. Cancel returns only credits actually spent. Restart retains
the choices; a new normal game uses the original balance data. AI players cannot receive the bonus.

Team paint uses blue, red, gold, green, purple, cyan, orange, pink, lime or ivory; computer player n receives the
n-th color after yours, so every player of a match has its own color. Unit/building faction accents, production icons and radar markers use
these colors, including after capture or mind control. Metal, glass and ground remain natural.

Edge scrolling is optional and defaults off for new settings. Existing settings are preserved.
The held camera gesture works away from window edges; wheel zoom ignores the sidebar. Arrow
keys and minimap navigation remain available. `A` over ore starts the selected miners' cycles;
other selected units still attack-move. A direct Move, Stop or enemy Attack overrides mining.

The maps add connected roads, blocked woodland and civilian blocks, cliff terraces, shoreline
animation and broad ground-color variation. Bases retain clear building aprons and existing ore
fields. The shared layout writes obstacles to TMX so visible obstacles and pathfinding agree.
Cliffs are blocked terraces; this does not add traversable height levels or garrisonable civilians.
