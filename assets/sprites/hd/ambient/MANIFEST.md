# Map scenery and ambient life sprites

Original art, generated from geometry in `scripts/hd_map_art.py` (scenery) and
`scripts/hd_ambient.py` (vehicles, people, animals), rendered by `scripts/render_map_art.py`
with the scene, sun, camera and materials of `scripts/render_hd_blender.py`, and composed by
`scripts/finish_map_art.py`. No team paint anywhere: every file is checked to contain no pixel
inside either team-paint rule (blue ramp `b > r+60 && g > r+35`; red-dominant `r > g+65 && r > b+70`).

## Regenerate

```sh
RD=/path/to/render-cache        # any scratch directory
# 1. Render the raw passes (Blender Cycles, ~2 s per sprite after a ~70 s warm-up).
#    Append `resume` to skip passes already on disk (e.g. after a Metal kernel-cache crash).
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
    --python scripts/render_map_art.py -- $RD all
#    or a subset: t:26  t:28:5  u:bus  w:cow ...
# 2. Compose the atlases/sheets into assets/sprites/hd (only these files are written) and verify
#    sizes, < 4 MiB, and the team-paint rules. --debug writes each atlas over a diamond grid.
uv run --with pillow --with numpy python scripts/finish_map_art.py $RD [OUT_DIR] [--debug DIR]
```

## Terrain atlases (`assets/sprites/hd/terrain/`)

All are 1024 x 1024 RGBA at 4 texels per world px, rendered like the existing tops.

Slot layouts:
* **64 px slots** (27-33): sixteen 256 x 256 slots, slot `s` at `((s % 4) * 256, (s / 4) * 256)`;
  canvas `Model(64, 64, (32, 16))`: the cell diamond spans y 16..48 world px (texels 64..192).
  Baked ground shadow, like the existing tops.
* **Skyscrapers** (26): four 256 x 1024 slots (64 x 256 world px), slot `i` at `(256 i, 0)`;
  canvas `Model(64, 256, (32, 208))`: the cell diamond spans y 208..240 world px, 16 px margin below.
  No baked shadow (as the tall kinds 16/17).
* **Tall slots** (34, 35): eight 256 x 512 slots (64 x 128 world px), slot `s` at
  `((s % 4) * 256, (s / 4) * 512)`; canvas `Model(64, 128, (32, 80))`, exactly as kinds 16/17.
  No baked shadow.

| file | slot -> content |
|---|---|
| `26.png` skyscrapers | 0 glass supertall, four setbacks, needle spire (~212 px) · 1 slender stepped limestone tower, stacked-octagon crown (~220 px) · 2 twisted tapering glass tower, open crown frame and mast (~208 px) · 3 dark-glass tower with bronze fins, plant floors, lit lattice antenna mast (~222 px) |
| `27.png` temperate clutter (walkable, low) | 0 bush clump · 1 bush clump · 2 flowerbed pink/purple/white · 3 flowerbed yellow/orange/white · 4 single boulder · 5 rock pair · 6 tall grass tufts · 7 fallen log · 8 tree stumps · 9 berry shrub · 10 mossy stones · 11 wildflower patch · 12 pond-side reeds · 13 lone small tree (~25 px) · 14 two hay bales · 15 flowerbed rose/white/pink |
| `28.png` crags (mountain cells; base fills the diamond like tops 3/11/12) | 0-7 grey temperate rock (0, 3, 6 with a tree; turf cap) · 8-11 snow-capped grey rock · 12-15 banded red/orange sandstone pinnacles |
| `29.png` parked vehicles | 0-7 along the cell x axis (screen down-right), 8-15 the same set along the y axis (screen down-left): 0 sedan (maroon) · 1 sedan (silver) · 2 taxi (yellow, blank roof sign) · 3 pickup (green) · 4 delivery van (white) · 5 city bus (orange/cream, overhangs the diamond) · 6 box truck · 7 tractor |
| `30.png` urban props | 0 fountain · 1 statue on a plinth (generic seated figure) · 2 bandstand · 3 newspaper kiosk · 4 billboard on legs (abstract graphic) · 5 bus shelter with bench · 6 planters with small trees · 7 street lamps and benches · 8 water tower on legs* · 9 lattice radio mast* · 10 playground · 11 market stalls with striped awnings · 12 phone booth, bench, bins, post box · 13 three flagpoles on a plaza · 14 obelisk monument · 15 parking-lot markings with two parked cars |
| `31.png` rural props | 0 haystacks · 1 round hay bales · 2 wooden fence run · 3 stone well · 4 windmill* · 5 grain silo* · 6 scarecrow in crop rows · 7 beehives · 8 log pile · 9 two tents · 10 campfire with log seats · 11 wooden cart · 12 water trough · 13 chicken coop with run · 14 grain sacks on a pallet · 15 wind turbine* |
| `32.png` arid (0-7) and cold (8-15) props | 0 saguaro-type cactus group · 1 dead tree · 2 bleached rocks · 3 desert tent · 4 oil barrels · 5 dry grass tufts · 6 small rock arch · 7 rusted car wreck · 8 snowy rocks · 9 ice blocks · 10 snowy spruce sapling · 11 igloo · 12 frozen boat hull · 13 snowdrift · 14 research hut with antenna and dish · 15 dog sled |
| `33.png` coastal and water props | 0-7 on water cells (no ground drawn): 0 moored fishing boat · 1 small sailboat · 2 rowboat · 3 buoys (muted red/white, outside the team rule) · 4 rocks in surf · 5 pier stub on piles · 6 lily/kelp patch · 7 small rocky islet; 8-15 on sand: 8 beach umbrellas and towels · 9 beach huts · 10 lifeguard tower · 11 upturned boat · 12 stacked crab pots · 13 lighthouse* · 14 dock crates and barrels · 15 driftwood |
| `34.png` tall landmarks | 0 cathedral with twin towers · 1 TV tower with a pod · 2 stadium stand corner with a floodlight mast · 3 ferris wheel · 4 church with a tall spire · 5 clock tower · 6 chimney stack with factory shed · 7 hotel tower with an abstract lit rooftop sign |
| `35.png` tall props (extra) | the full-height versions of the starred props: 0 water tower (~46 px) · 1 lattice radio mast (~56 px) · 2 windmill (~50 px) · 3 grain silo (~45 px) · 4 wind turbine (~62 px) · 5 lighthouse (~60 px) · 6-7 empty |

\* A 64 x 64 slot has only 32 px of headroom above the cell centre (the canvas top is 16 px above
the diamond's top corner), so the starred props are drawn ~32-34 px tall in their 64 px slots
(standing a little forward of the centre). `35.png` carries them at the requested heights in the
64 x 128 tall-slot format; use those where the prop should be full height.

## Ambient vehicles (`assets/sprites/hd/ambient/units/<id>.png`)

3072 x 384 RGBA, eight 384-texel frames (96 world px), the same facing order, ground point and
darkroom finish (`finish_hd_art.develop`) as `assets/sprites/hd/units/*.png`: canvas
`Model(96, 96, (48, 70))`, facing `f` = the model turned `f * 45` degrees (column `f`; facing 0
heads down-right along the cell x axis, as the roster's units). Baked ground shadow.

ids: `car_sedan` (maroon) · `car_compact` (silver hatchback) · `taxi` (yellow) · `pickup` (green) ·
`bus` (orange/cream city bus, 10.5 m) · `truck` (box truck) · `tractor` · `sailboat` · `fishing_boat` ·
`ferry` (a 36 m car ferry shortened like the warships). Cars are at vehicle scale (0.125 cell/m: a
car is ~0.57 cell long). Boats float: the waterline is at the ground point, nothing is drawn below it.

## Ambient walkers (`assets/sprites/hd/ambient/walk/<id>.png`)

2048 x 2048 RGBA, as `assets/sprites/hd/walk/*.png`: 8 columns = the eight facings (same order as
the unit sheets), 8 rows = the frames of the cycle, 256-texel frames (64 world px) around the same
ground point as `unit_walk` (canvas `Model(64, 64, (32, 46))`). Baked ground shadow, except the gull.

ids: `civ_man` · `civ_woman` · `civ_worker` (hard hat, hi-vis yellow vest, shovel) — the infantry
figure and walk rig of `hd_infantry.py`, same height as the soldiers · `cow` · `sheep` · `deer` ·
`horse` · `camel` (a diagonal-pair gait like `attack_dog`, at infantry scale: a cow is ~2.5 m long,
~1.4 people's height) · `penguin` (a waddle) · `seagull` (eight frames of a wing-flap cycle, ~10 px
wingspan; drawn in flight with its body 14 world px above the ground point and no shadow).
