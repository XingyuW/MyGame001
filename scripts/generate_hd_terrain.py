"""Deterministic RA2-style temperate terrain, built as layers (see battlefield.rs):

  0.png  grass base   1024^2, 8x8 cells, 2 texels/px (128 texel slots), slot = (x%8, y%8)
  2.png  water base   1024^2, 8x8 cells, 2 texels/px (128 texel slots), slot = (x%8, y%8)

Every atlas must pass the bounded loader (tileset_image.rs: 4 MiB file, 16 MiB decoded).
  4.png  soil overlay 1024^2, 4x4 cells, 2 texels/px; each slot is 128 px wide with a ragged,
         feathered edge that spills past its own diamond.
  8.png  sand overlay, 9.png snow overlay: as the soil overlay (beaches, desert, snowfields).
 10.png  pavement base: as the grass base; concrete slabs of city plazas and sidewalks.
 20-24.png  ice, crops, marsh, ash and lava overlays: as the soil overlay.
  1,3,5,6,7,11-15,18,19.png  tops (ore, cliffs, road, grove, cottage, snowy cliffs, mesas,
         snowy pines, palms, bridge, snowy cottage, adobe): 1024^2, sixteen 64 px slots,
         rendered by render_hd_blender.py (t:<kind>) and assembled by finish_hd_art.py.
 16,17.png  tall tops (city blocks, towers): 1024^2, eight 64 x 128 px slots (TALL_KINDS).

Every base/overlay texel is a function of *world* position that is periodic over the atlas'
cell lattice, so neighbouring tiles always agree along shared edges: no visible grid. Overlapping
soil sprites sample the same world colour, so only their outer edges blend into the grass.

  uv run --with pillow --with numpy python scripts/generate_hd_terrain.py
  uv run --with pillow --with numpy python scripts/generate_hd_terrain.py 20 24   # only these
"""
from generate_hd_art import *

ORE = (190, 150, 20)      # mapped to crystalline gold by the Blender renderer
ROCK = (120, 116, 100); TURF = (92, 104, 58); BARK = (92, 72, 50)
LEAF = (62, 84, 42); PINE = (46, 70, 44); ASPHALT = (74, 73, 68); CURB = (150, 146, 128)
SNOWCAP = (232, 236, 242); ICEROCK = (112, 118, 126); SANDSTONE = (176, 112, 70)
SANDSTONE_DARK = (138, 84, 56); PALM = (82, 112, 44); PALM_TRUNK = (118, 96, 66)
ADOBE_WALL = (196, 160, 116); TERRACOTTA = (164, 86, 58); RAIL = (96, 100, 98)
GIRDER = (104, 92, 80); BRICKWALL = (142, 82, 62); CONCRETE_W = (168, 166, 158)
CITYGLASS = (86, 118, 138); ROOFTAR = (64, 64, 62); AWNING = (156, 52, 48)

# ------------------------------------------------------------------------------------------------
# Periodic fields on the cell lattice.
def torus(n, lo, hi, seed, aniso=(1.0, 1.0)):
    """n x n periodic band-limited noise, frequencies in cycles per torus; unit variance."""
    w = np.random.default_rng(seed).normal(size=(n, n))
    k = np.fft.fftfreq(n) * n
    ku, kv = np.meshgrid(k, k, indexing='ij')
    r = np.sqrt((ku * aniso[0]) ** 2 + (kv * aniso[1]) ** 2)
    f = np.exp(-(r / hi) ** 2) * (1 - np.exp(-(r / lo) ** 2) if lo > 0 else 1)
    out = np.real(np.fft.ifft2(np.fft.fft2(w) * f))
    return (out - out.mean()) / (out.std() + 1e-9)

def sample(field, u, v, period):
    """Bilinear, wrapping lookup of a lattice field at lattice coordinates (u, v)."""
    n = field.shape[0]
    fu = (u % period) / period * n; fv = (v % period) / period * n
    i0 = np.floor(fu).astype(int); j0 = np.floor(fv).astype(int)
    tu, tv = fu - i0, fv - j0
    i0 %= n; j0 %= n; i1 = (i0 + 1) % n; j1 = (j0 + 1) % n
    return (field[i0, j0] * (1 - tu) * (1 - tv) + field[i1, j0] * tu * (1 - tv)
            + field[i0, j1] * (1 - tu) * tv + field[i1, j1] * tu * tv)

def lattice(cells, slot_px, texels, centre):
    """Lattice coordinates and diamond metric for every texel of a cells x cells atlas."""
    size = cells * slot_px
    Y, X = np.mgrid[0:size, 0:size].astype(np.float64)
    col, row = X // slot_px, Y // slot_px
    wx = (X % slot_px + .5) / texels - centre; wy = (Y % slot_px + .5) / texels - centre
    du = (wx / 32 + wy / 16) / 2; dv = (wy / 16 - wx / 32) / 2
    return col + .5 + du, row + .5 + dv, np.abs(wx) / 32 + np.abs(wy) / 16

def smooth(a, b, x):
    t = np.clip((x - a) / (b - a), 0, 1)
    return t * t * (3 - 2 * t)

def mix(c0, c1, t):
    return np.asarray(c0, float) * (1 - t[..., None]) + np.asarray(c1, float) * t[..., None]

def relief(h, u, v, period, strength):
    """Micro-relief lit from the upper right, matching the miniature renders."""
    gu = sample((np.roll(h, -1, 0) - np.roll(h, 1, 0)), u, v, period)
    gv = sample((np.roll(h, -1, 1) - np.roll(h, 1, 1)), u, v, period)
    # Light comes from screen upper-right: decreasing v, slightly decreasing u.
    return 1 + strength * (gv * .8 - gu * .25)

def finish(rgb, alpha, seed):
    grain = np.random.default_rng(seed).normal(0, 1, alpha.shape)
    rgb = rgb * (1 + .035 * grain[..., None])
    out = np.dstack([np.clip(rgb, 0, 255), np.clip(alpha, 0, 255)]).astype(np.uint8)
    return Image.fromarray(out, 'RGBA')

def grass():
    P, n = 8, 2048
    u, v, d = lattice(P, 128, 2, 32)
    low = sample(torus(n, 0, 2.5, 1), u, v, P)
    mid = sample(torus(n, 3, 12, 2), u, v, P)
    clump = sample(torus(n, 14, 48, 3), u, v, P)
    fine = sample(torus(n, 60, 220, 4), u, v, P)
    blades = sample(torus(n, 120, 420, 5, (1, .35)), u, v, P)
    lush, dry, dark = (66, 82, 40), (108, 106, 62), (44, 58, 30)
    dirt, pale = (102, 86, 58), (126, 120, 84)
    t = smooth(-1.1, 1.4, low * .85 + mid * .4)
    rgb = mix(lush, dry, t * .8)
    rgb *= (1 + .10 * clump[..., None])
    rgb = mix(rgb, dirt, smooth(1.35, 2.0, mid * .7 + clump * .55 + low * .25) * .85)
    rgb = mix(rgb, pale, smooth(1.6, 2.4, -mid * .6 + fine * .5) * .5)
    rgb = mix(rgb, dark, smooth(.9, 1.9, fine * .7 + blades * .5) * .7)
    h = torus(n, 4, 40, 6)
    rgb *= relief(h, u, v, P, .55)[..., None]
    rgb *= (1 + .07 * blades[..., None] + .05 * fine[..., None])
    return finish(rgb, (d <= 1.02) * 255.0, 7)

def water():
    P, n = 8, 2048
    u, v, d = lattice(P, 128, 2, 32)
    depth = sample(torus(n, 0, 3, 11), u, v, P)
    swell = sample(torus(n, 6, 30, 12, (1, .3)), u, v, P)
    chop = sample(torus(n, 40, 160, 13, (1, .45)), u, v, P)
    # Gentle low-frequency variation: open sea spans many periods of this texture, and the
    # renderer darkens it with depth, so strong blotches would show the 8-cell repeat.
    rgb = mix((36, 72, 82), (44, 86, 94), smooth(-1.6, 1.6, depth))
    rgb *= (1 + .035 * swell[..., None] + .055 * chop[..., None])
    rgb = mix(rgb, (150, 186, 180), smooth(2.0, 2.9, chop + swell * .3) * .5)
    return finish(rgb, (d <= 1.02) * 255.0, 14)

def soil():
    P, n = 4, 1024
    u, v, d = lattice(P, 256, 2, 64)
    low = sample(torus(n, 0, 3, 21), u, v, P)
    mid = sample(torus(n, 3, 16, 22), u, v, P)
    fine = sample(torus(n, 30, 140, 23), u, v, P)
    edge = sample(torus(n, 4, 24, 24), u, v, P)
    rgb = mix((84, 68, 48), (118, 98, 68), smooth(-1.2, 1.2, low * .8 + mid * .5))
    rgb = mix(rgb, (74, 60, 42), smooth(1.2, 2.2, -mid * .6 + fine * .6) * .7)   # damp hollows
    rgb = mix(rgb, (146, 136, 112), smooth(1.9, 2.6, fine) * .7)                 # pebbles
    rgb = mix(rgb, (98, 104, 56), smooth(.9, 2.0, mid * .5 + edge * .6) * .45)    # surviving grass
    h = torus(n, 4, 36, 25)
    rgb *= relief(h, u, v, P, .6)[..., None]
    rgb *= (1 + .06 * fine[..., None])
    alpha = smooth(1.5, .9, d + .28 * edge) * 255.0
    return finish(rgb, alpha, 26)

def sand():
    P, n = 4, 1024
    u, v, d = lattice(P, 256, 2, 64)
    low = sample(torus(n, 0, 3, 31), u, v, P)
    mid = sample(torus(n, 3, 16, 32), u, v, P)
    ripple = sample(torus(n, 40, 120, 33, (1, .18)), u, v, P)   # wind ripples
    fine = sample(torus(n, 60, 200, 34), u, v, P)
    edge = sample(torus(n, 4, 24, 35), u, v, P)
    rgb = mix((182, 160, 112), (212, 192, 142), smooth(-1.2, 1.2, low * .8 + mid * .4))
    rgb = mix(rgb, (160, 134, 92), smooth(1.1, 2.1, -mid * .7 + low * .3) * .6)     # darker drifts
    rgb *= (1 + .045 * ripple[..., None] + .03 * fine[..., None])
    h = torus(n, 6, 50, 36, (1, .3))
    rgb *= relief(h, u, v, P, .45)[..., None]
    alpha = smooth(1.5, .9, d + .28 * edge) * 255.0
    return finish(rgb, alpha, 37)

def snow():
    P, n = 4, 1024
    u, v, d = lattice(P, 256, 2, 64)
    low = sample(torus(n, 0, 3, 41), u, v, P)
    mid = sample(torus(n, 3, 18, 42), u, v, P)
    fine = sample(torus(n, 60, 220, 43), u, v, P)
    edge = sample(torus(n, 4, 24, 44), u, v, P)
    rgb = mix((206, 214, 226), (238, 241, 246), smooth(-1.2, 1.2, low * .7 + mid * .5))
    rgb = mix(rgb, (168, 182, 202), smooth(1.2, 2.2, -mid * .7 + fine * .3) * .55)   # blue hollows
    rgb = mix(rgb, (104, 112, 84), smooth(2.0, 2.8, fine * .8 + mid * .3) * .45)     # grass tufts
    h = torus(n, 4, 40, 45)
    rgb *= relief(h, u, v, P, .5)[..., None]
    rgb *= (1 + .025 * fine[..., None])
    alpha = smooth(1.5, .9, d + .28 * edge) * 255.0
    return finish(rgb, alpha, 46)

def pavement():
    P, n = 8, 2048
    u, v, d = lattice(P, 128, 2, 32)
    low = sample(torus(n, 0, 2.5, 51), u, v, P)
    stain = sample(torus(n, 6, 30, 52), u, v, P)
    fine = sample(torus(n, 80, 300, 53), u, v, P)
    rgb = mix((140, 138, 130), (164, 162, 154), smooth(-1.2, 1.2, low))
    rgb = mix(rgb, (110, 106, 98), smooth(1.4, 2.4, stain) * .6)                     # oil, rain
    # Slabs: half-cell joints along both lattice axes, every slab a slightly different shade.
    ju, jv = np.abs((u * 2) % 1 - .5), np.abs((v * 2) % 1 - .5)
    joint = np.maximum(smooth(.46, .49, ju), smooth(.46, .49, jv))
    slab = (np.floor(u * 2).astype(int) * 7 + np.floor(v * 2).astype(int) * 13) % 5 - 2
    rgb *= (1 + .018 * slab[..., None])
    rgb = mix(rgb, (92, 90, 86), joint * .75)
    rgb *= (1 + .04 * fine[..., None])
    return finish(rgb, (d <= 1.02) * 255.0, 54)

def plates(points, u, v, period):
    """Periodic Voronoi plates on the lattice: how far each texel is from the nearest plate
    boundary (0 on the boundary, in cells) and which plate it belongs to."""
    d1 = np.full(u.shape, 1e9); d2 = d1.copy(); index = np.zeros(u.shape, int)
    for i, (pu, pv) in enumerate(points):
        du = (u - pu + period / 2) % period - period / 2
        dv = (v - pv + period / 2) % period - period / 2
        dd = np.hypot(du, dv)
        near = dd < d1
        d2 = np.where(near, d1, np.minimum(d2, dd))
        index = np.where(near, i, index); d1 = np.where(near, dd, d1)
    return d2 - d1, index

def spill(d, edge):
    """Overlay alpha whose ragged edge only ever spills outward, so the cells an overlay covers
    stay covered: for ice, ash and lava, where the base must not show through the middle."""
    return smooth(1.5, .9, d + .28 * (.3 - .9 * np.minimum(np.abs(edge), 2.2))) * 255.0

def ice():
    """Lake and sea ice: a flat glassy sheet, wind-swept clear patches, snow drifts, cracks."""
    P, n = 4, 1024
    u, v, d = lattice(P, 256, 2, 64)
    low = sample(torus(n, 0, 3, 101), u, v, P)
    mid = sample(torus(n, 3, 10, 102), u, v, P)
    drift = sample(torus(n, 3, 16, 103, (1, .3)), u, v, P)      # snow combed along the wind
    streak = sample(torus(n, 14, 70, 104, (1, .2)), u, v, P)    # the same wind, as fine dusting
    cloud = sample(torus(n, 14, 60, 105), u, v, P)              # trapped air, refrozen slush
    fine = sample(torus(n, 60, 220, 106), u, v, P)
    edge = sample(torus(n, 4, 24, 107), u, v, P)
    wu = sample(torus(n, 1, 5, 108), u, v, P); wv = sample(torus(n, 1, 5, 109), u, v, P)
    rgb = mix((138, 166, 182), (170, 192, 204), smooth(-1.3, 1.3, low * .5 + mid * .4 + cloud * .3))
    clear = smooth(-.3, 1.7, -low * .55 - mid * .8 + cloud * .1)
    rgb = mix(rgb, (84, 118, 132), clear * .8)                                   # clear ice
    rgb = mix(rgb, (192, 208, 216), smooth(.8, 2.3, cloud) * .3)                 # milky ice
    # Pressure cracks: a sparse plate network, long and nearly straight; a finer second set.
    rng = np.random.default_rng(110)
    major, _ = plates(rng.uniform(0, P, (7, 2)), u + .05 * wu, v + .05 * wv, P)
    minor, _ = plates(rng.uniform(0, P, (24, 2)), u + .03 * wv, v - .03 * wu, P)
    shown = smooth(-.5, .6, mid * .6 + cloud * .4)                               # healed stretches
    line = (smooth(.08, .03, major) * .8 + smooth(.26, .04, major) * .14
            + smooth(.045, .015, minor) * .45 * shown)
    rgb = mix(rgb, (218, 232, 238), np.clip(line, 0, 1))
    rgb = mix(rgb, (62, 92, 106), smooth(.02, .0, major) * .45)                  # the open seam
    snowed = np.clip(smooth(.95, 1.7, drift * .9 + low * .3 + streak * .2)
                     + smooth(1.1, 2.4, streak + drift * .5) * .4, 0, 1)
    h = torus(n, 5, 36, 111, (1, .4))
    lit = relief(h, u, v, P, .4)
    rgb = mix(rgb, (228, 234, 239), snowed * .9)                                 # snow drifts
    rgb *= (1 + (lit - 1) * (.08 + .92 * snowed))[..., None]
    rgb *= (1 + .012 * fine[..., None])
    return finish(rgb, spill(d, edge), 112)

def crops():
    """A ripe grain field: drilled rows along the lattice u axis, tramlines, patchy ripeness."""
    P, n = 4, 1024
    u, v, d = lattice(P, 256, 2, 64)
    low = sample(torus(n, 0, 3, 121), u, v, P)
    mid = sample(torus(n, 3, 14, 122), u, v, P)
    patch = sample(torus(n, 5, 24, 123), u, v, P)
    lodge = sample(torus(n, 4, 20, 124, (.55, 1)), u, v, P)      # wind-flattened swathes
    ears = sample(torus(n, 50, 200, 125, (1, .3)), u, v, P)      # ears streaked along the rows
    fine = sample(torus(n, 60, 220, 126), u, v, P)
    edge = sample(torus(n, 4, 24, 127), u, v, P)
    wobble = sample(torus(n, 1, 7, 128), u, v, P)
    ROWS = 6                                                     # per cell: periodic by design
    across = v + .018 * wobble
    turn = 2 * np.pi * ROWS * across
    crown = .5 + .5 * np.cos(turn)                               # 1 on a row, 0 in the furrow
    rgb = mix((176, 146, 80), (200, 172, 104), smooth(-1.3, 1.3, low * .35 + mid * .6 + patch * .4))
    rgb = mix(rgb, (146, 144, 80), smooth(.7, 1.9, mid * .5 - patch * .8) * .55)  # still green
    rgb = mix(rgb, (212, 190, 132), smooth(1.0, 2.1, patch * .8 + fine * .25) * .45)  # bleached
    flat = smooth(1.0, 1.9, lodge + mid * .25)
    rgb = mix(rgb, (138, 112, 66), flat * .6)                                    # lodged grain
    stand = 1 - .8 * flat
    rgb *= (1 - .17 * stand * (1 - crown) - .07 * stand * np.sin(turn))[..., None]
    rgb *= (1 + .07 * ears[..., None] + .03 * fine[..., None])
    # Tramlines: a pair of wheel tracks in two furrows, every second cell.
    track = np.zeros_like(v)
    for furrow in (6, 8):
        offset = (across - (furrow + .5) / ROWS + 1) % 2 - 1
        track = np.maximum(track, smooth(.05, .02, np.abs(offset)))
    rgb = mix(rgb, (114, 94, 60), track * (.6 + .08 * np.clip(patch, -1.5, 1.5)))
    h = torus(n, 4, 30, 129)
    rgb *= relief(h, u, v, P, .3)[..., None]
    alpha = smooth(1.5, .9, d + .28 * edge) * 255.0
    return finish(rgb, alpha, 130)

def marsh():
    """Waterlogged lowland: olive mud and sedge, still dark pools, straw reed tufts, moss."""
    P, n = 4, 1024
    u, v, d = lattice(P, 256, 2, 64)
    low = sample(torus(n, 0, 3, 141), u, v, P)
    mid = sample(torus(n, 3, 14, 142), u, v, P)
    pool = sample(torus(n, 2, 7, 143), u, v, P)
    creek = sample(torus(n, 2, 6, 144), u, v, P)
    shore = sample(torus(n, 10, 44, 145), u, v, P)
    tussock = sample(torus(n, 20, 70, 146), u, v, P)
    fine = sample(torus(n, 60, 220, 147), u, v, P)
    tuft = sample(torus(n, 70, 260, 148, (.6, 1)), u, v, P)
    edge = sample(torus(n, 4, 24, 149), u, v, P)
    sky = sample(torus(n, 2, 10, 150), u, v, P)
    rgb = mix((56, 60, 38), (82, 76, 48), smooth(-1.3, 1.3, low * .5 + mid * .7))
    rgb = mix(rgb, (80, 96, 46), smooth(.0, 1.5, tussock * .6 + mid * .5) * .7)             # sedge
    rgb = mix(rgb, (110, 114, 54), smooth(1.0, 2.1, shore * .7 - mid * .4 + tussock * .3) * .6)  # moss
    rgb = mix(rgb, (48, 42, 31), smooth(.9, 2.0, -tussock * .5 - mid * .6) * .55)           # peat
    h = torus(n, 8, 44, 151)
    rgb *= relief(h, u, v, P, .4)[..., None]
    rgb *= (1 + .04 * fine[..., None] + .04 * tussock[..., None])
    # Open water: a few broad pools, the winding channels that join them, scattered puddles.
    wet = pool + .3 * shore + .15 * low
    channel = smooth(.11, .05, np.abs(creek) + .03 * shore) * smooth(-.6, .3, pool)
    puddle = smooth(1.9, 2.2, shore * .9 + tussock * .5)
    water = np.maximum(np.maximum(smooth(.78, .98, wet), channel), puddle)
    margin = np.maximum(smooth(.25, .85, wet), smooth(.26, .08, np.abs(creek)) * smooth(-.6, .3, pool))
    rgb = mix(rgb, (54, 48, 33), margin * .6)                                    # sodden margin
    still = mix((22, 32, 36), (50, 62, 68), smooth(-1.4, 1.6, sky + shore * .15))  # sky in the pools
    still = mix(still, (50, 58, 38), smooth(1.2, 2.2, tussock) * .5)             # duckweed, algae
    rgb = mix(rgb, still, water)
    reed = smooth(1.55, 2.4, tuft * .75 + fine * .3 + margin * (1 - water) * .7)
    rgb = mix(rgb, (160, 150, 102), reed * .75)                                  # reed tufts
    alpha = smooth(1.5, .9, d + .28 * edge) * 255.0
    return finish(rgb, alpha, 152)

def ash():
    """Volcanic ash and black sand: charcoal with wind ripples, pumice, oxidised patches."""
    P, n = 4, 1024
    u, v, d = lattice(P, 256, 2, 64)
    low = sample(torus(n, 0, 3, 161), u, v, P)
    mid = sample(torus(n, 3, 16, 162), u, v, P)
    dune = torus(n, 10, 32, 163, (1, .22))
    ripple = sample(dune, u, v, P)
    fine = sample(torus(n, 60, 220, 164), u, v, P)
    grit = sample(torus(n, 90, 320, 165), u, v, P)
    edge = sample(torus(n, 4, 24, 166), u, v, P)
    rust = sample(torus(n, 2, 7, 167), u, v, P)
    rgb = mix((54, 52, 51), (78, 75, 71), smooth(-1.3, 1.3, low * .3 + mid * .8))
    rgb = mix(rgb, (42, 38, 35), smooth(.7, 2.2, -mid * .85) * .45)              # damp black sand
    rgb = mix(rgb, (100, 72, 55), smooth(1.1, 2.3, rust + mid * .2) * .5)        # oxidised scoria
    rgb *= (1 + .08 * ripple[..., None] + .03 * fine[..., None])
    rgb *= relief(dune + .4 * torus(n, 4, 30, 168), u, v, P, .65)[..., None]
    rgb = mix(rgb, (132, 128, 120), smooth(2.2, 3.0, grit * .8 + fine * .4) * .65)  # pumice
    rgb = mix(rgb, (30, 28, 27), smooth(2.2, 3.0, -grit * .8 + fine * .4) * .5)     # cinders
    return finish(rgb, spill(d, edge), 169)

def lava():
    """A lava field: basalt crust in irregular plates, glowing cracks, a few molten pools."""
    P, n = 4, 1024
    u, v, d = lattice(P, 256, 2, 64)
    low = sample(torus(n, 0, 3, 181), u, v, P)
    mid = sample(torus(n, 3, 14, 182), u, v, P)
    heat = sample(torus(n, 2, 7, 183), u, v, P)
    pond = sample(torus(n, 3, 10, 194), u, v, P)
    ropy = sample(torus(n, 20, 80, 184, (1, .35)), u, v, P)
    fine = sample(torus(n, 60, 220, 185), u, v, P)
    edge = sample(torus(n, 4, 24, 186), u, v, P)
    wu = sample(torus(n, 2, 10, 187), u, v, P); wv = sample(torus(n, 2, 10, 188), u, v, P)
    ju = sample(torus(n, 24, 90, 189), u, v, P); jv = sample(torus(n, 24, 90, 190), u, v, P)
    rng = np.random.default_rng(191)
    crack, plate = plates(rng.uniform(0, P, (22, 2)),
                          u + .11 * wu + .012 * ju, v + .11 * wv + .012 * jv, P)
    fissure, _ = plates(rng.uniform(0, P, (90, 2)),
                        u + .05 * wv + .012 * jv, v - .05 * wu + .012 * ju, P)
    tone = rng.uniform(-1, 1, 22)[plate]
    rgb = mix((36, 31, 29), (68, 58, 51), smooth(-1.4, 1.4, low * .4 + mid * .5 + tone * .7))
    rgb = mix(rgb, (86, 79, 72), smooth(1.4, 2.4, ropy * .7 + fine * .4) * .5)   # ash-grey skin
    h = torus(n, 8, 60, 192, (1, .45))
    rgb *= relief(h, u, v, P, .7)[..., None]
    rgb *= (1 + .07 * ropy[..., None] + .05 * fine[..., None])
    # Heat: wide bright cracks where the flow is live, swelling into pools; dark seams elsewhere.
    live = smooth(-1.3, 1.3, heat + mid * .25)
    swell = smooth(1.0, 2.1, pond + heat * .35)
    width = .018 + .05 * live + .11 * swell
    hot = np.maximum((1 - crack / width) * (.45 + .55 * np.maximum(live, swell)),
                     (1 - fissure / (.004 + .018 * live * live)) * .5 * live)
    hot *= 1 - .45 * swell * smooth(.8, 2.0, ropy)                               # skin on the pools
    rgb = mix(rgb, (18, 15, 14), smooth(.05, .0, crack) * .75)                    # cold seams
    rgb = mix(rgb, (112, 36, 20), smooth(3.0, .6, crack / width) * .34 * live)    # glow on the crust
    ramp = mix((116, 20, 10), (218, 80, 18), smooth(.05, .55, hot))
    ramp = mix(ramp, (255, 194, 86), smooth(.55, 1.0, hot))
    ramp *= (1 + .05 * fine[..., None])
    rgb = mix(rgb, ramp, smooth(.0, .22, hot))
    return finish(rgb, spill(d, edge), 193)

# ------------------------------------------------------------------------------------------------
# Tops: geometry for the Blender renderer (Model API; origin at the cell's top corner).
def top(kind, seed):
    rng = random.Random(seed * 7919 + kind * 104729)
    m = Model(64, 64, (32, 16))
    if kind == 1:  # ore: dense faceted gold nuggets in loose clusters
        for _ in range(3):
            cu, cv = rng.uniform(.25, .75), rng.uniform(.25, .75)
            for i in range(rng.randint(6, 10)):
                u = min(.92, max(.08, cu + rng.gauss(0, .14))); v = min(.92, max(.08, cv + rng.gauss(0, .14)))
                r = rng.uniform(.035, .085)
                m.cyl(u, v, 0, r, rng.uniform(1.5, 4.5), ORE, n=rng.choice([5, 6, 7]), r2=r * rng.uniform(.25, .6))
    elif kind == 3:  # rocky terrace: blocky cliff with boulders and a turf cap
        m.box(0, 0, -2, 1, 1, 15, ROCK)
        m.box(.015, .015, 12.6, .97, .97, 1.8, TURF, False)
        for _ in range(7):
            side = rng.random() < .5
            u, v = (rng.uniform(.05, .95), .98) if side else (.98, rng.uniform(.05, .95))
            r = rng.uniform(.06, .13)
            m.cyl(u, v, -1, r, rng.uniform(6, 13), shade(ROCK, rng.uniform(.8, 1.1)), n=6, r2=r * .55)
        for _ in range(2 + seed % 3):
            r = rng.uniform(.06, .12)
            m.cyl(rng.uniform(.2, .8), rng.uniform(.2, .8), 14, r, rng.uniform(2, 4), ROCK, n=6, r2=r * .5)
        if seed % 4 == 0: tree(m, rng, .45, .4, 14)
    elif kind == 5:  # road: one continuous asphalt surface; curbs only on exposed edges
        m.box(0, 0, 0, 1, 1, .5, ASPHALT, False)
        # Bits name connected neighbours (see battlefield.rs): 1 = -y, 2 = +x, 4 = +y, 8 = -x.
        for bit, box in [(1, (0, 0, 1, .03)), (2, (.97, 0, .03, 1)), (4, (0, .97, 1, .03)), (8, (0, 0, .03, 1))]:
            if not seed & bit: m.box(box[0], box[1], 0, box[2], box[3], 1.2, CURB)
        if seed == 5: m.box(.49, .3, .5, .02, .4, .1, (194, 180, 126), False)
        if seed == 10: m.box(.3, .49, .5, .4, .02, .1, (194, 180, 126), False)
    elif kind == 6:  # grove: mixed pines and broadleaf trees with undergrowth
        for _ in range(2 + seed % 2):
            tree(m, rng, rng.uniform(.25, .75), rng.uniform(.25, .75), 0)
        for _ in range(3):
            m.ellipsoid(rng.uniform(.1, .9), rng.uniform(.1, .9), 1, .07, .07, 2.5, shade(LEAF, rng.uniform(.8, 1.1)))
    elif kind == 7:  # cottage
        wall = (172, 162, 136)
        m.box(.18, .18, 0, .63, .65, 15, wall)
        roof = [(118, 64, 44), (92, 95, 84), (138, 92, 58)][seed % 3]
        m.face([(.12, .12, 15), (.5, .12, 23), (.5, .87, 23), (.12, .87, 15)], roof)
        m.face([(.5, .12, 23), (.88, .12, 15), (.88, .87, 15), (.5, .87, 23)], shade(roof, .9))
        m.face([(.12, .12, 15), (.88, .12, 15), (.5, .12, 23)], shade(wall, .95))
        m.face([(.12, .87, 15), (.5, .87, 23), (.88, .87, 15)], shade(wall, .95))
        m.box(.65, .35, 17, .08, .09, 9, (103, 98, 84))
        for u in [.27, .6]: m.box(u, .835, 5, .12, .015, 5, (48, 66, 66), False)
        m.box(.815, .3, 0, .02, .14, 9, (67, 58, 43), False)
        for _ in range(2): m.ellipsoid(rng.uniform(.05, .3), rng.uniform(.7, .95), 1, .08, .08, 3, LEAF)
    elif kind == 11:  # snowy mountain: a rock shoulder under jagged snow-capped peaks
        m.box(0, 0, -2, 1, 1, 9, ICEROCK)
        m.box(.015, .015, 6.6, .97, .97, 1.6, SNOWCAP, False)
        peaks = [(.5, .5, .42, 22 + seed % 4 * 2)] + [
            (rng.uniform(.22, .78), rng.uniform(.22, .78), rng.uniform(.2, .3), rng.uniform(11, 18))
            for _ in range(1 + seed % 3)]
        for u, v, r, h in peaks:
            sides = rng.choice([5, 6, 7])
            rock = shade(ICEROCK, rng.uniform(.82, 1.05))
            m.cyl(u, v, 7, r, h * .55, rock, n=sides, r2=r * .5)
            m.cyl(u, v, 7 + h * .55, r * .5, h * .45, SNOWCAP, n=sides, r2=.0)
        for _ in range(6):
            side = rng.random() < .5
            u, v = (rng.uniform(.05, .95), .98) if side else (.98, rng.uniform(.05, .95))
            r = rng.uniform(.06, .12)
            m.cyl(u, v, -1, r, rng.uniform(5, 9), shade(ICEROCK, rng.uniform(.8, 1.1)), n=6, r2=r * .55)
        for _ in range(3):  # drifts banked against the face
            m.ellipsoid(rng.uniform(.1, .9), .98, 1, rng.uniform(.08, .16), .08, 3, SNOWCAP)
    elif kind == 12:  # desert mesa: banded sandstone, flat cap, fallen blocks
        h = 14 + seed % 4 * 2
        for i, z in enumerate(range(-2, h, 4)):
            inset = .01 * (i % 2)
            m.box(inset, inset, z, 1 - 2 * inset, 1 - 2 * inset, min(4, h - z),
                  SANDSTONE if i % 2 == 0 else SANDSTONE_DARK, False)
        m.box(.02, .02, h, .96, .96, 1, shade(SANDSTONE, 1.08), False)
        for _ in range(6):
            side = rng.random() < .5
            u, v = (rng.uniform(.05, .95), 1.0) if side else (1.0, rng.uniform(.05, .95))
            r = rng.uniform(.05, .11)
            m.cyl(u, v, -1, r, rng.uniform(3, 8), shade(SANDSTONE_DARK, rng.uniform(.85, 1.1)), n=5, r2=r * .6)
        if seed % 3 == 0:
            m.cyl(rng.uniform(.3, .7), rng.uniform(.3, .7), h + 1, .12, rng.uniform(3, 6), SANDSTONE, n=6, r2=.07)
    elif kind == 13:  # snowy pine forest
        for _ in range(2 + seed % 3):
            snow_pine(m, rng, rng.uniform(.2, .8), rng.uniform(.2, .8), 0)
        for _ in range(3):
            m.ellipsoid(rng.uniform(.1, .9), rng.uniform(.1, .9), 0, .09, .08, 2.5, SNOWCAP)
    elif kind == 14:  # palms over desert scrub
        for _ in range(1 + seed % 3):
            palm(m, rng, rng.uniform(.4, .8), rng.uniform(.4, .8), 21)
        for _ in range(3):
            m.ellipsoid(rng.uniform(.1, .9), rng.uniform(.1, .9), 1, .07, .07, 2.5, shade(PALM, rng.uniform(.7, 1.0)))
        if seed % 2: m.cyl(rng.uniform(.1, .9), rng.uniform(.1, .9), 0, .06, 3, SANDSTONE, n=5, r2=.03)
    elif kind == 15:  # bridge: deck on girders and piers; rails on the open sides
        deck = 7
        for u in [.2, .8]:
            for v in [.2, .8]:
                m.cyl(u, v, -2, .07, deck, CONCRETE_W, n=10)
        for v in [.08, .92]:
            m.box(0, v - .04, deck - 3, 1, .08, 3, GIRDER)
        for u in [.08, .92]:
            m.box(u - .04, 0, deck - 3, .08, 1, 3, GIRDER)
        m.box(0, 0, deck, 1, 1, .8, CONCRETE_W, False)
        m.box(.12, .12, deck + .8, .76, .76, .2, shade(ASPHALT, 1.15), False)
        for bit, (x, y, w, d) in [(1, (0, 0, 1, .04)), (2, (.96, 0, .04, 1)), (4, (0, .96, 1, .04)), (8, (0, 0, .04, 1))]:
            if seed & bit: continue
            m.box(x, y, deck, w, d, 1.6, CURB)
            for i in range(6):
                t = (i + .5) / 6
                px, py = (x + t * w, y + d / 2) if w > d else (x + w / 2, y + t * d)
                m.box(px - .012, py - .012, deck + 1.6, .024, .024, 4, RAIL, False)
            m.box(x, y, deck + 5.2, w, d, .9, RAIL, False)
    elif kind == 18:  # cottage under snow
        wall = (164, 150, 128)
        m.box(.18, .18, 0, .63, .65, 15, wall)
        m.face([(.12, .12, 15), (.5, .12, 23), (.5, .87, 23), (.12, .87, 15)], SNOWCAP)
        m.face([(.5, .12, 23), (.88, .12, 15), (.88, .87, 15), (.5, .87, 23)], shade(SNOWCAP, .93))
        m.face([(.12, .12, 15), (.88, .12, 15), (.5, .12, 23)], shade(wall, .95))
        m.face([(.12, .87, 15), (.5, .87, 23), (.88, .87, 15)], shade(wall, .95))
        m.box(.65, .35, 17, .08, .09, 9, (103, 98, 84)); m.box(.64, .34, 26, .1, .11, 1, SNOWCAP)
        for u in [.27, .6]: m.box(u, .835, 5, .12, .015, 5, (210, 170, 90), False)  # lit windows
        m.box(.815, .3, 0, .02, .14, 9, (67, 58, 43), False)
        m.box(.05, .7, 0, .25, .22, 4, (112, 84, 58))  # woodpile
        m.box(.05, .7, 4, .25, .22, 1, SNOWCAP, False)
        for _ in range(2): m.ellipsoid(rng.uniform(.05, .95), rng.uniform(.05, .95), 0, .09, .09, 2.5, SNOWCAP)
    elif kind == 19:  # flat-roofed adobe house
        wall = shade(ADOBE_WALL, rng.uniform(.92, 1.05))
        h = 13 + seed % 3 * 3
        m.box(.18, .2, 0, .62, .6, h, wall)
        m.box(.16, .18, h, .66, .64, 1.5, shade(wall, 1.05))
        if seed % 4 == 1:
            m.cyl(.5, .5, h + 1.5, .2, 6, shade(wall, 1.08), n=16, r2=.05)
        elif seed % 4 == 2:
            m.box(.2, .22, h + 1.5, .3, .3, 7, wall); m.box(.18, .2, h + 8.5, .34, .34, 1, shade(wall, 1.05))
        for u in [.3, .58]: m.box(u, .795, 5, .1, .015, 5, (54, 46, 40), False)
        m.box(.8, .45, 0, .015, .14, 9, (92, 62, 40), False)
        for u in [.25, .45, .65]: m.cyl(u, .82, h - 2, .015, 1, (110, 80, 50), n=6)  # vigas
        m.box(.02, .82, 0, .3, .14, 3, TERRACOTTA)  # garden wall
        if seed % 2: palm(m, rng, .1, .78, 15)
    return m

def tall(kind, seed):
    """City blocks (16) and towers (17): one cell, up to ~110 px tall, origin at the cell's top
    corner in a 64 x 128 canvas."""
    rng = random.Random(seed * 6151 + kind * 3571)
    m = Model(64, 128, (32, 80))
    if kind == 16:
        city_block(m, rng, seed)
    else:
        tower(m, rng, seed)
    return m

def windows(m, x, y, w, d, z0, floors, fh, cols, glass, frame):
    """Punched windows on the two faces the camera sees (+y and +x)."""
    for f in range(floors):
        z = z0 + f * fh + fh * .3
        for i in range(cols):
            u = x + (i + .5) * w / cols
            m.box(u - w / cols * .28, y + d, z, w / cols * .56, .012, fh * .45, glass, False)
            v = y + (i + .5) * d / cols
            m.box(x + w, v - d / cols * .28, z, .012, d / cols * .56, fh * .45, glass, False)
        m.box(x, y + d, z0 + f * fh, w, .016, .5, frame, False)
        m.box(x + w, y, z0 + f * fh, .016, d, .5, frame, False)

def roof_kit(m, rng, x, y, w, d, z):
    m.box(x + .02, y + .02, z, w - .04, d - .04, .8, ROOFTAR, False)
    m.box(x, y, z, w, .03, 1.6, CONCRETE_W, False); m.box(x, y + d - .03, z, w, .03, 1.6, CONCRETE_W, False)
    m.box(x, y, z, .03, d, 1.6, CONCRETE_W, False); m.box(x + w - .03, y, z, .03, d, 1.6, CONCRETE_W, False)
    for _ in range(rng.randint(1, 3)):
        m.box(x + rng.uniform(.05, w - .2), y + rng.uniform(.05, d - .2), z, .12, .1, 3, STEEL)

def city_block(m, rng, seed):
    x, y, w, d = .1, .1, .8, .8
    variant = seed % 8
    if variant in (0, 4):  # brick apartments with a water tank
        floors = 3 + variant // 4
        wall = shade(BRICKWALL, rng.uniform(.9, 1.08))
        m.box(x, y, 0, w, d, floors * 9 + 2, wall)
        windows(m, x, y, w, d, 2, floors, 9, 4, GLASS, CONCRETE_W)
        top = floors * 9 + 2
        roof_kit(m, rng, x, y, w, d, top)
        m.cyl(x + .25, y + .3, top, .1, 4, GIRDER, n=8)
        m.cyl(x + .25, y + .3, top + 4, .13, 7, (120, 92, 66), n=14)
        m.cyl(x + .25, y + .3, top + 11, .13, 2, ROOFTAR, n=14, r2=.02)
    elif variant in (1, 5):  # shop row with awnings
        wall = shade(CONCRETE_W, rng.uniform(.9, 1.05))
        m.box(x, y, 0, w, d, 20, wall)
        m.box(x, y + d, 1, w, .015, 6, GLASS, False); m.box(x + w, y, 1, .015, d, 6, GLASS, False)
        m.face([(x, y + d, 8), (x + w, y + d, 8), (x + w, y + d + .12, 6), (x, y + d + .12, 6)], AWNING if variant == 1 else GOLD)
        m.face([(x + w, y, 8), (x + w, y + d, 8), (x + w + .12, y + d, 6), (x + w + .12, y, 6)], AWNING if variant == 1 else GOLD)
        windows(m, x, y, w, d, 10, 1, 9, 3, GLASS, wall)
        roof_kit(m, rng, x, y, w, d, 20)
        m.box(x + .1, y + d - .02, 21, .5, .03, 5, (60, 88, 120))  # sign board
    elif variant in (2, 6):  # warehouse with a sawtooth roof
        wall = (138, 136, 126)
        m.box(x, y, 0, w, d, 16, wall)
        for i in range(8): m.box(x + .03 + i * .1, y + d, 1, .02, .012, 14, shade(wall, .8), False)
        m.box(x + .25, y + d, 0, .3, .015, 10, (92, 92, 88), False)  # loading door
        for i in range(3):
            u = x + i * w / 3
            m.prism([(u, 16), (u + w / 3, 16), (u + w / 3, 22)], y, y + d, shade((150, 146, 136), .95 + .05 * i))
            m.face([(u + w / 3 - .001, y, 16.5), (u + w / 3 - .001, y + d, 16.5), (u + w / 3 - .001, y + d, 21.5), (u + w / 3 - .001, y, 21.5)], GLASS)
    elif variant == 3:  # small church with a steeple
        wall = (196, 190, 176)
        m.box(.2, .15, 0, .6, .7, 18, wall)
        m.face([(.17, .15, 18), (.5, .15, 28), (.5, .85, 28), (.17, .85, 18)], (96, 70, 62))
        m.face([(.5, .15, 28), (.83, .15, 18), (.83, .85, 18), (.5, .85, 28)], shade((96, 70, 62), .9))
        m.face([(.2, .85, 18), (.5, .85, 28), (.8, .85, 18)], wall)
        m.box(.38, .02, 0, .24, .24, 44, wall)
        m.cyl(.5, .14, 44, .17, 16, (96, 70, 62), n=4, r2=.0)
        for u in [.3, .55]: m.box(u, .85, 5, .08, .012, 10, (80, 110, 140), False)
    else:  # variant 7: parking deck
        for f in range(3):
            m.box(x, y, f * 8, w, d, 1.5, CONCRETE_W)
            for (u, v) in [(x, y), (x + w - .06, y), (x, y + d - .06), (x + w - .06, y + d - .06)]:
                m.box(u, v, f * 8 + 1.5, .06, .06, 6.5, CONCRETE_W)
        m.box(x, y, 24, w, d, 1.5, CONCRETE_W)
        for i in range(4):
            car = [(152, 44, 40), (60, 80, 120), (200, 200, 196), (40, 42, 44)][rng.randrange(4)]
            m.box(x + .1 + (i % 2) * .38, y + .12 + (i // 2) * .38, 25.5, .26, .14, 2.5, car)

def tower(m, rng, seed):
    variant = seed % 8
    if variant in (0, 4):  # glass curtain-wall tower with setbacks
        h1, h2 = 44 + seed, 22
        m.box(.12, .12, 0, .76, .76, h1, CONCRETE_W)
        for f in range(int(h1 // 5)):
            m.box(.12, .88, 2 + f * 5, .76, .012, 3.4, CITYGLASS, False)
            m.box(.88, .12, 2 + f * 5, .012, .76, 3.4, CITYGLASS, False)
        m.box(.22, .22, h1, .56, .56, h2, CONCRETE_W)
        for f in range(int(h2 // 5)):
            m.box(.22, .78, h1 + 2 + f * 5, .56, .012, 3.4, CITYGLASS, False)
            m.box(.78, .22, h1 + 2 + f * 5, .012, .56, 3.4, CITYGLASS, False)
        roof_kit(m, rng, .22, .22, .56, .56, h1 + h2)
        m.tube((.5, .5, h1 + h2), (.5, .5, h1 + h2 + 8), .012, STEEL, n=6)
    elif variant in (1, 5):  # art-deco stepped tower with a spire
        c = (196, 186, 164)
        for i, (inset, h) in enumerate([(.1, 34), (.18, 18), (.26, 12), (.33, 8)]):
            z = sum([34, 18, 12, 8][:i])
            m.box(inset, inset, z, 1 - 2 * inset, 1 - 2 * inset, h, c)
            n = 5 - i
            for k in range(n):
                t = inset + (k + .5) * (1 - 2 * inset) / n
                m.box(t - .02, 1 - inset, z + 2, .04, .012, h - 4, CITYGLASS, False)
                m.box(1 - inset, t - .02, z + 2, .012, .04, h - 4, CITYGLASS, False)
        m.cyl(.5, .5, 72, .06, 8, c, n=8, r2=.0)
    elif variant in (2, 6):  # residential slab with balconies
        h = 56 + seed
        c = shade(CONCRETE_W, .95)
        m.box(.15, .2, 0, .7, .6, h, c)
        windows(m, .15, .2, .7, .6, 3, int((h - 3) // 7), 7, 4, GLASS, c)
        for f in range(int((h - 3) // 7)):
            m.box(.15, .8, 3 + f * 7, .7, .05, .8, c, False)
        roof_kit(m, rng, .15, .2, .7, .6, h)
    elif variant == 3:  # cylindrical tower
        m.cyl(.5, .5, 0, .38, 66, CONCRETE_W, n=24)
        for f in range(12): m.cyl(.5, .5, 3 + f * 5, .385, 3, CITYGLASS, n=24)
        m.cyl(.5, .5, 66, .3, 4, STEEL, n=24); m.cyl(.5, .5, 70, .12, 5, CONCRETE_W, n=12)
    else:  # variant 7: dark glass block with a roof crown
        m.box(.13, .13, 0, .74, .74, 70, (58, 66, 74))
        for f in range(14):
            m.box(.13, .87, 2 + f * 5, .74, .012, .6, STEEL, False)
            m.box(.87, .13, 2 + f * 5, .012, .74, .6, STEEL, False)
        m.box(.1, .1, 70, .8, .8, 3, STEEL)
        for u in [.2, .45, .7]: m.box(u, .1, 73, .1, .8, 3, STEEL)

def snow_pine(m, rng, u, v, z):
    m.cyl(u, v, z, .03, 10, BARK, n=8)
    for zz, r in [(6, .2), (12, .16), (18, .11), (23, .06)]:
        m.cyl(u, v, z + zz, r, 8, shade(PINE, rng.uniform(.8, 1.0)), n=12, r2=.01)
        m.cyl(u, v, z + zz + 2.5, r * .78, 2.2, SNOWCAP, n=12, r2=r * .45)

def palm(m, rng, u, v, height=26):
    lean = (rng.uniform(-.12, .12), rng.uniform(-.12, .12))
    pts = [(u + lean[0] * t * t, v + lean[1] * t * t, height * t) for t in [0, .25, .5, .75, 1]]
    for a, b in zip(pts, pts[1:]): m.tube(a, b, .028, PALM_TRUNK, n=8)
    tx, ty, tz = pts[-1]
    for i in range(7):
        ang = i * 2 * pi / 7 + rng.uniform(-.2, .2)
        mid = (tx + .16 * cos(ang), ty + .16 * sin(ang), tz + 2)
        tip = (tx + .3 * cos(ang), ty + .3 * sin(ang), tz - 5)
        side = (-sin(ang) * .05, cos(ang) * .05)
        for a, b in [((tx, ty, tz), mid), (mid, tip)]:
            m.face([(a[0] - side[0], a[1] - side[1], a[2]), (b[0] - side[0], b[1] - side[1], b[2]),
                    (b[0] + side[0], b[1] + side[1], b[2]), (a[0] + side[0], a[1] + side[1], a[2])],
                   shade(PALM, rng.uniform(.85, 1.1)))
    m.ellipsoid(tx, ty, tz - 1, .04, .04, 2, (92, 70, 40))

def tree(m, rng, u, v, z):
    if rng.random() < .5:  # pine
        m.cyl(u, v, z, .03, 10, BARK, n=8)
        for zz, r in [(6, .2), (12, .16), (18, .11), (23, .06)]:
            m.cyl(u, v, z + zz, r, 8, shade(PINE, rng.uniform(.85, 1.15)), n=12, r2=.01)
    else:  # broadleaf: a trunk under a cluster of foliage lobes
        m.cyl(u, v, z, .035, 12, BARK, n=8)
        for _ in range(6):
            m.ellipsoid(u + rng.gauss(0, .07), v + rng.gauss(0, .07), z + rng.uniform(14, 22),
                        rng.uniform(.09, .14), rng.uniform(.09, .14), rng.uniform(4, 6),
                        shade(LEAF, rng.uniform(.8, 1.2)))

TOP_KINDS = [1, 3, 5, 6, 7, 11, 12, 13, 14, 15, 18, 19]
TALL_KINDS = [16, 17]

LAYERS = {0: grass, 2: water, 4: soil, 8: sand, 9: snow, 10: pavement,
          20: ice, 21: crops, 22: marsh, 23: ash, 24: lava}

def main(only=()):
    """Writes every base and overlay atlas and the particles, or just the atlases named."""
    (OUT / 'terrain').mkdir(exist_ok=True)
    for index, layer in LAYERS.items():
        if not only or index in only: layer().save(OUT / 'terrain' / f'{index}.png')
    if only: return print('Generated terrain layers', *sorted(only))
    # Soft particles with a smooth premultiplied-looking edge, used at runtime for emission,
    # smoke, explosions, dust, shock waves, and local light pools.
    for name in ['glow', 'smoke', 'ring']:
        n = 128; yy, xx = np.mgrid[0:n, 0:n]; r = np.sqrt(((xx - 63.5) / 63.5) ** 2 + ((yy - 63.5) / 63.5) ** 2)
        alpha = np.maximum(0, 1 - r) ** (2 if name == 'glow' else 1.2)
        if name == 'smoke': alpha *= .65 + .35 * np.sin(xx * .14) * np.cos(yy * .11)
        if name == 'ring': alpha = np.exp(-((r - .73) / .075) ** 2) * np.maximum(0, 1 - r)
        ar = np.full((n, n, 4), 255, dtype=np.uint8); ar[:, :, 3] = np.uint8(alpha * 255)
        Image.fromarray(ar).save(OUT / f'{name}.png')
    print('Generated every base and overlay layer; render tops with render_hd_blender.py t:<kind>.')

if __name__ == '__main__':
    import sys
    main({int(a) for a in sys.argv[1:]})
