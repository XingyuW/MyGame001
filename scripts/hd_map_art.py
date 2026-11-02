"""Scenery for the redesigned maps: terrain atlases 26-35, rendered by render_map_art.py and
composed by finish_map_art.py. Nothing here changes how any existing sprite renders.

  26  skyscrapers        four 64 x 256 px slots, canvas Model(64, 256, (32, 208))
  27  temperate clutter  sixteen 64 x 64 px slots, canvas Model(64, 64, (32, 16)) (as the tops)
  28  crags              sixteen 64 x 64 (mountain cells: the base fills the diamond)
  29  parked vehicles    sixteen 64 x 64 (0-7 along the cell x axis, 8-15 along y)
  30  urban props        31 rural props     32 arid (0-7) and cold (8-15) props
  33  coastal props      (0-7 float on water cells, 8-15 stand on sand)
  34  tall landmarks     eight 64 x 128 px slots, canvas Model(64, 128, (32, 80)) (as kinds 16/17)
  35  tall props         the full-height versions of the props a 64 px slot cannot hold (32 px of
                         headroom above the cell centre): water tower, radio mast, windmill, silo,
                         wind turbine, lighthouse; 64 x 128 slots like 34 (slots 6-7 empty)

Props are authored in metres with hd_kit (x along the cell's x axis, y along its y axis, z up) at
about vehicle scale, so parked cars, kiosks and benches agree. Rock and crags use plain colours
(the legacy terrain material, like the existing mountain tops); everything man-made a material
class. No colour is team paint, and none of the saturated ones falls inside the team-paint rules.
"""
import random
from contextlib import contextmanager
from math import sin, cos, pi, radians, sqrt, atan2, degrees
from hd_kit import *
import generate_hd_art as g
import generate_hd_terrain as terrain
import hd_ambient as amb

U_PROP = .13      # cells per metre for props (vehicles are .125, buildings .1)
U_TALL = .1       # landmarks: building scale

def px(h, u):
    """Metres that rise h world px at scale u."""
    return h / (u * ZC)

# --- Palette --------------------------------------------------------------------------------------
ROCK_T = (124, 120, 106); ROCK_D = (98, 96, 88); ROCK_L = (146, 142, 128)
TURF = (92, 104, 58); SNOW = (232, 236, 242); ICEROCK = (112, 118, 126)
SAND_R = (176, 118, 78); SAND_D = (138, 90, 62); SAND_L = (196, 150, 104)
PINE = (46, 70, 44); MOSS = rgb(92, 112, 52, 'foliage')
SOIL = rgb(86, 66, 48, 'cloth'); STRAW = rgb(198, 170, 102, 'cloth'); STRAW_D = rgb(160, 134, 76, 'cloth')
GRASS = rgb(96, 120, 56, 'foliage'); GRASS_D = rgb(70, 92, 44, 'foliage'); DRYGRASS = rgb(176, 156, 102, 'cloth')
BARK = rgb(96, 74, 52, 'wood'); PALEWOOD = rgb(176, 150, 112, 'wood'); GREYWOOD = rgb(150, 144, 132, 'wood')
STONE = rgb(196, 188, 170, 'concrete'); STONE_D = rgb(150, 144, 132, 'concrete'); LIMESTONE = rgb(206, 198, 178, 'concrete')
WATERC = rgb(46, 82, 90, 'water'); FOAM = rgb(226, 230, 228, 'cloth')
FL_PINK = rgb(214, 132, 168, 'foliage'); FL_PURPLE = rgb(128, 92, 160, 'foliage'); FL_YELLOW = rgb(222, 194, 64, 'foliage')
FL_WHITE = rgb(232, 230, 220, 'foliage'); FL_ORANGE = rgb(222, 160, 62, 'foliage'); FL_ROSE = rgb(176, 72, 112, 'foliage')
BERRY = rgb(112, 40, 72, 'foliage')
GLASS_T = C((40, 62, 76), 'glass'); GLASS_D = C((22, 28, 34), 'glass'); GLASS_W = C((92, 98, 86), 'glass')
MULLION = C((176, 180, 180), 'steel'); BRONZE = C((118, 92, 62), 'copper')
FLAG_Y = rgb(220, 188, 60, 'cloth'); FLAG_G = rgb(60, 120, 76, 'cloth'); FLAG_W = rgb(230, 228, 218, 'cloth')
AWN_G = rgb(70, 120, 80, 'cloth'); AWN_Y = rgb(214, 176, 70, 'cloth'); AWN_T = rgb(176, 118, 84, 'cloth')
CREAM = rgb(226, 218, 194, 'cloth'); PAINT_Y = rgb(214, 176, 52); PAINT_G = rgb(70, 116, 70)
PAINT_O = rgb(214, 148, 56); BUOY_R = rgb(158, 94, 82); SIGN_EMIT = C((255, 214, 140), 'emit')
WARM = C((255, 228, 170), 'emit'); AMBER = C((255, 196, 104), 'emit')

@contextmanager
def centre(k, cx=.5, cy=.5, yaw=0.0, s=1.0):
    """Origin at cell position (cx, cy) in cells, turned by yaw degrees."""
    with k.at(cx / k.u, cy / k.u, 0, yaw=yaw, s=s): yield k

def rock(k, rng, x, y, r, h, c, n=7, z=0.0, top=.45, sq=1.0, a0=None):
    """A faceted rock: four irregular rings lofted without smoothing."""
    a0 = rng.uniform(0, 2 * pi) if a0 is None else a0
    rad = [r * rng.uniform(.72, 1.15) for _ in range(n)]
    tops = [rng.uniform(.82, 1.06) for _ in range(n)]
    def ring(f, zf, jag=False):
        return [(x + rad[i] * f * cos(a0 + i * 2 * pi / n), y + rad[i] * f * sin(a0 + i * 2 * pi / n) * sq,
                 z + h * zf * (tops[i] if jag else 1)) for i in range(n)]
    k.loft([ring(.9, -.05), ring(1.0, .3), ring(.78, .72), ring(top, 1.0, True)], c, smooth=False)

def shard(k, rng, x, y, r, h, c, cap=None, capf=.62, n=6, z=0.0, lean=(0, 0)):
    """A jagged crag: an irregular frustum leaning over, with a (snow) cap above capf."""
    a0 = rng.uniform(0, 2 * pi)
    rad = [r * rng.uniform(.65, 1.2) for _ in range(n)]
    def ring(f, t):
        lx, ly = lean[0] * t * r, lean[1] * t * r
        return [(x + lx + rad[i] * f * cos(a0 + i * 2 * pi / n), y + ly + rad[i] * f * sin(a0 + i * 2 * pi / n),
                 z + h * t * rng.uniform(.96, 1.04)) for i in range(n)]
    rings = [ring(1.0, 0), ring(.82, .3), ring(.55, capf)]
    top = ring(.12, 1.0)
    if cap is None:
        k.loft(rings + [ring(.32, .85), top], c, smooth=False)
    else:
        k.loft(rings, c, smooth=False)
        k.loft([[(p[0], p[1], p[2] - .05) for p in rings[-1]], ring(.4, .82), top], cap, smooth=False)

def foliage(k, rng, x, y, z, r, c, n=4, spread=.6, h=.8):
    for _ in range(n):
        k.ball(x + rng.gauss(0, r * spread * .5), y + rng.gauss(0, r * spread * .5), z + rng.uniform(.0, r * .4),
               r * rng.uniform(.65, 1.0), g.shade(c, rng.uniform(.82, 1.15)), rz=r * h * rng.uniform(.8, 1.1), n=10, k=5)

def blades(k, rng, x, y, n, h, c, spread=.35, w=.05):
    for _ in range(n):
        a = rng.uniform(0, 2 * pi); lean = rng.uniform(.1, .5) * h
        bx, by = x + rng.gauss(0, spread * .3), y + rng.gauss(0, spread * .3)
        tip = (bx + lean * cos(a), by + lean * sin(a), h * rng.uniform(.65, 1.1))
        k.plate([(bx - w * sin(a), by + w * cos(a), 0), (bx + w * sin(a), by - w * cos(a), 0), tip],
                g.shade(c, rng.uniform(.8, 1.15)))

def small_tree(k, rng, x, y, h, leaf=LEAF):
    k.cyl(x, y, 0, h * .045, h * .55, BARK, n=8, r2=h * .03)
    for _ in range(6):
        k.ball(x + rng.gauss(0, h * .08), y + rng.gauss(0, h * .08), h * rng.uniform(.6, .85), h * rng.uniform(.14, .2),
               g.shade(leaf, rng.uniform(.82, 1.18)), n=10, k=5)

def bench(k, x, y, yaw=0.0, L=1.8):
    with k.at(x, y, 0, yaw=yaw):
        for s in (-1, 1): k.cbox(s * (L / 2 - .15), 0, 0, .08, .5, .45, DARK)
        for i in range(3): k.box(-L / 2, -.25 + i * .17, .42, L, .13, .05, PALEWOOD)
        k.box(-L / 2, -.3, .5, L, .06, .4, PALEWOOD)

def lamp_post(k, x, y, h=4.5):
    k.cyl(x, y, 0, .12, .3, DARK, n=8)
    k.tube((x, y, .3), (x, y, h), .06, DARK, n=8)
    k.ball(x, y, h + .15, .22, WARM, rz=.25, n=10, k=5)
    k.cyl(x, y, h + .32, .26, .08, DARK, n=10, r2=.05)

def bin_(k, x, y, c=rgb(70, 92, 70)):
    k.cyl(x, y, 0, .28, .9, c, n=10); k.cyl(x, y, .9, .3, .06, DARK, n=10)

def crate(k, x, y, z=0.0, s=1.0, c=PALEWOOD, yaw=0.0):
    with k.at(x, y, z, yaw=yaw, s=s):
        k.cbox(0, 0, 0, 1, 1, 1, c)
        for d in (-.45, .45): k.cbox(d, 0, 0, .1, 1.02, 1.0, g.shade(c, .8))

def barrel(k, x, y, z=0.0, c=rgb(84, 92, 70), lying=False):
    if lying: k.rev((x - .45, y, z + .3), 'x', [(0, .28), (.05, .3), (.45, .32), (.85, .3), (.9, .28)], c, n=12)
    else:
        k.rev((x, y, z), 'z', [(0, .28), (.05, .3), (.45, .32), (.85, .3), (.9, .28)], c, n=12)
        k.cyl(x, y, z + .89, .27, .02, DARK, n=12)

def rope_coil(k, x, y, z=0.0):
    k.torus(x, y, z + .06, .32, .07, rgb(176, 156, 112, 'cloth'), n=16, k=6)
    k.torus(x, y, z + .17, .26, .07, rgb(176, 156, 112, 'cloth'), n=16, k=6)

# =================================================================================================
# 26 Skyscrapers (Model coordinates: cells and px). The cell is 0..1 on both axes.
def rbox(m, cx, cy, z, w, d, h, ang, c):
    ca, sa = cos(ang), sin(ang)
    base = [(-w / 2, -d / 2), (w / 2, -d / 2), (w / 2, d / 2), (-w / 2, d / 2)]
    P = [(cx + a * ca - b * sa, cy + a * sa + b * ca) for a, b in base]
    lo = [(p[0], p[1], z) for p in P]; hi = [(p[0], p[1], z + h) for p in P]
    m.begin(False)
    for i in range(4):
        j = (i + 1) % 4; m.face([lo[i], lo[j], hi[j], hi[i]], c)
    m.face(hi, c); m.face(lo[::-1], c)
    m.end()

def curtain(m, x, y, w, d, z0, h, glass, frame, fh=4.0, bays=8, spandrel=.8):
    """Glazed block: horizontal spandrels at every floor and vertical mullions on the two seen faces."""
    m.box(x, y, z0, w, d, h, glass, False)
    e = .006
    for f in range(int(h // fh) + 1):
        z = z0 + f * fh
        if z > z0 + h - spandrel: break
        m.box(x - e, y + d - e, z, w + 2 * e, 2 * e, spandrel, frame, False)
        m.box(x + w - e, y - e, z, 2 * e, d + 2 * e, spandrel, frame, False)
    for i in range(1, bays):
        m.box(x + i * w / bays - .004, y + d - e * .5, z0, .008, 1.6 * e, h, frame, False)
        m.box(x + w - e * .5, y + i * d / bays - .004, z0, 1.6 * e, .008, h, frame, False)

def roof_bits(m, rng, x, y, w, d, z, n=3):
    m.box(x, y, z, w, d, .6, C((84, 84, 82), 'roof'), False)
    for _ in range(n):
        bw, bd = rng.uniform(.08, .16), rng.uniform(.08, .16)
        m.box(x + rng.uniform(.03, w - bw - .03), y + rng.uniform(.03, d - bd - .03), z + .6, bw, bd, rng.uniform(2, 4), C((150, 154, 156), 'steel'))

def sky_glass(m, rng):
    # Glass supertall in four setbacks over a stone podium, a needle spire on the crown.
    m.box(.06, .06, 0, .88, .88, 12, LIMESTONE)
    m.box(.06, .94 - .002, 2, .88, .01, 7, GLASS_T, False); m.box(.94 - .002, .06, 2, .01, .88, 7, GLASS_T, False)
    tiers = [(.1, 12, 100), (.17, 100, 148), (.24, 148, 180), (.31, 180, 192)]
    for i, (s, z0, z1) in enumerate(tiers):
        curtain(m, s, s, 1 - 2 * s, 1 - 2 * s, z0, z1 - z0, GLASS_T, MULLION, bays=8 - i)
        m.box(s - .012, s - .012, z1, 1 - 2 * s + .024, 1 - 2 * s + .024, 1.4, LIMESTONE)
    m.box(.34, .34, 193.4, .32, .32, 3, C((150, 154, 156), 'steel'))
    m.cyl(.5, .5, 196, .07, 8, C((176, 180, 182), 'steel'), n=8, r2=.04)
    m.cyl(.5, .5, 204, .035, 18, C((196, 200, 202), 'steel'), n=8, r2=.006)
    m.cyl(.5, .5, 212, .012, 1.2, WARM, n=6)
    for s, z in [(.17, 100), (.24, 148)]:   # maintenance gantries on the setbacks
        m.box(.1 + .04, .1 + .04, z + 1.4, .06, .06, 2.5, C((150, 154, 156), 'steel'))

def sky_stepped(m, rng):
    # Slender stepped limestone tower: piers and recessed glazing, a crown of stacked octagons.
    stone = LIMESTONE
    tiers = [(.16, 0, 112), (.2, 112, 150), (.245, 150, 172), (.285, 172, 186)]
    for s, z0, z1 in tiers:
        w = 1 - 2 * s
        m.box(s, s, z0, w, w, z1 - z0, stone)
        n = max(3, int(w * 9))
        for i in range(n):
            t = s + (i + .5) * w / n
            m.box(t - .022, 1 - s - .002, z0 + 2, .044, .008, z1 - z0 - 4, GLASS_D, False)
            m.box(1 - s - .002, t - .022, z0 + 2, .008, .044, z1 - z0 - 4, GLASS_D, False)
        for f in range(int((z1 - z0 - 4) // 4)):
            z = z0 + 2 + f * 4 + 3
            m.box(s, 1 - s - .001, z, w, .009, .8, stone, False); m.box(1 - s - .001, s, z, .009, w, .8, stone, False)
        m.box(s - .01, s - .01, z1, w + .02, w + .02, 1.2, C((180, 172, 152), 'concrete'))
    crown = C((196, 196, 188), 'steel')
    for i, (r, h) in enumerate([(.2, 5), (.16, 5), (.12, 5), (.08, 4)]):
        z = 187.2 + sum([5, 5, 5, 4][:i])
        m.cyl(.5, .5, z, r, h, crown, n=8, r2=r * .9)
        for k_ in range(4):   # lit triangular windows on the seen faces
            a = pi / 8 + k_ * pi / 4
            if not (-.1 < a < pi / 2 + .5): continue
            m.face([(.5 + r * .96 * cos(a) - .02, .5 + r * .96 * sin(a), z + .8), (.5 + r * .96 * cos(a) + .02, .5 + r * .96 * sin(a), z + .8),
                    (.5 + r * .9 * cos(a), .5 + r * .9 * sin(a), z + h - .6)], WARM)
    m.cyl(.5, .5, 206, .03, 14, crown, n=8, r2=.004)

def sky_twist(m, rng):
    # Twisted, tapering glass tower: 36 floor plates each turned a little further.
    n = 36; fh = 5.0
    for i in range(n):
        t = i / (n - 1)
        w = .72 - .24 * t; ang = radians(85) * t
        z = 2 + i * fh
        rbox(m, .5, .5, z, w, w, fh - .9, ang, GLASS_T)
        rbox(m, .5, .5, z + fh - .9, w + .02, w + .02, .9, ang, C((214, 214, 208), 'concrete'))
    m.box(.12, .12, 0, .76, .76, 2.4, LIMESTONE)
    zt = 2 + n * fh
    for i in range(4):   # open crown frame, still turning
        t = 1 + i * .03
        rbox(m, .5, .5, zt + i * 3.4, .44 - i * .07, .44 - i * .07, .6, radians(85) * t, MULLION)
        for sx in (-1, 1):
            for sy in (-1, 1):
                a = radians(85) * t; r = (.22 - i * .035)
                x0 = .5 + r * (sx * cos(a) - sy * sin(a)); y0 = .5 + r * (sx * sin(a) + sy * cos(a))
                m.tube((x0, y0, zt + i * 3.4), (x0, y0, zt + i * 3.4 + 3.4), .006, MULLION, n=4)
    m.tube((.5, .5, zt), (.5, .5, zt + 26), .012, C((196, 200, 202), 'steel'), n=6)

def sky_dark(m, rng):
    # Dark-glass tower: black glazing behind bronze fins, a mechanical crown and a lit mast.
    fin = C((96, 84, 66), 'copper')
    m.box(.12, .12, 0, .76, .76, 172, GLASS_D, False)
    for i in range(13):
        t = .12 + (i + .5) * .76 / 13
        m.box(t - .006, .88 - .002, 0, .012, .018, 172, fin, False)
        m.box(.88 - .002, t - .006, 0, .018, .012, 172, fin, False)
    for f in range(21):
        z = 8 * f + 6
        m.box(.12, .88 - .001, z, .76, .006, .5, C((60, 62, 64), 'dark'), False)
        m.box(.88 - .001, .12, z, .006, .76, .5, C((60, 62, 64), 'dark'), False)
    m.box(.12, .12, 0, .76, .76, 8, C((40, 42, 44), 'dark'))
    m.box(.88, .3, 0, .01, .4, 6, C((150, 128, 96), 'emit'), False)     # lit lobby
    m.box(.3, .88, 0, .4, .01, 6, C((150, 128, 96), 'emit'), False)
    m.box(.16, .16, 172, .68, .68, 10, C((54, 56, 58), 'dark'))      # louvred plant floors
    for f in range(5): m.box(.155, .155, 173 + f * 2, .69, .69, .5, C((96, 98, 100), 'steel'), False)
    m.box(.14, .14, 182, .72, .72, 1, fin)
    roof_bits(m, rng, .2, .2, .6, .6, 183, 2)
    # Lattice antenna mast with aviation lamps (warm white: no red in the sprite).
    st = C((150, 154, 156), 'steel')
    for sx, sy in [(-1, -1), (1, -1), (1, 1), (-1, 1)]:
        m.tube((.5 + sx * .05, .5 + sy * .05, 184), (.5 + sx * .015, .5 + sy * .015, 214), .006, st, n=4)
    for z in range(186, 214, 4):
        r = .05 - (z - 184) / 30 * .035
        for (ax, ay), (bx, by) in [((-1, -1), (1, -1)), ((1, -1), (1, 1)), ((1, 1), (-1, 1)), ((-1, 1), (-1, -1))]:
            m.tube((.5 + ax * r, .5 + ay * r, z), (.5 + bx * r, .5 + by * r, z + 2), .004, st, n=4, caps=False)
    m.tube((.5, .5, 214), (.5, .5, 222), .006, st, n=4)
    for z in (196, 208, 222):
        m.ellipsoid(.5, .5, z, .022, .022, 1.6, AMBER if z < 222 else WARM)
    for sx, sy in [(-1, -1), (1, 1), (1, -1), (-1, 1)]:
        m.ellipsoid(.5 + sx * .3, .5 + sy * .3, 184, .02, .02, 1.4, AMBER)

SKYSCRAPERS = [sky_glass, sky_stepped, sky_twist, sky_dark]

def skyscraper(seed):
    m = g.Model(64, 256, (32, 208))
    SKYSCRAPERS[seed](m, random.Random(2600 + seed))
    return m

# =================================================================================================
# 27 Temperate clutter: small and low (walkable cells), at prop scale around the cell centre.
def c_bush(k, rng):
    for x, y, r in [(0, 0, 1.0), (.9, .5, .8), (-.6, .8, .7), (.3, -.8, .65)][:3 + rng.randint(0, 1)]:
        foliage(k, rng, x, y, .2, r, LEAF if rng.random() < .6 else LEAF2, n=3)

def c_flowerbed(k, rng, colours):
    k.rev((0, 0, 0), 'z', [(0, 2.1), (.12, 2.05), (.22, 1.9)], SOIL, n=20)
    k.torus(0, 0, .12, 2.15, .14, STONE_D, n=24, k=5)
    for _ in range(14):
        a, r = rng.uniform(0, 2 * pi), sqrt(rng.random()) * 1.6
        k.ball(r * cos(a), r * sin(a), .3, .32, g.shade(GRASS_D, rng.uniform(.8, 1.2)), rz=.22, n=8, k=4)
    for _ in range(44):
        a, r = rng.uniform(0, 2 * pi), sqrt(rng.random()) * 1.7
        k.ball(r * cos(a), r * sin(a), rng.uniform(.38, .55), .14, g.shade(rng.choice(colours), rng.uniform(.9, 1.08)), n=6, k=3)

def c_boulder(k, rng):
    rock(k, rng, 0, 0, 1.5, 1.7, ROCK_T)
    blades(k, rng, 1.2, .9, 6, .5, GRASS)

def c_rockpair(k, rng):
    rock(k, rng, -.8, -.3, 1.1, 1.3, ROCK_T); rock(k, rng, .9, .5, .8, .9, ROCK_L)
    blades(k, rng, .1, 1.1, 6, .45, GRASS)

def c_tallgrass(k, rng):
    for _ in range(6):
        blades(k, rng, rng.uniform(-1.6, 1.6), rng.uniform(-1.6, 1.6), 9, rng.uniform(.8, 1.3), GRASS if rng.random() < .6 else DRYGRASS, w=.06)

def c_log(k, rng):
    with k.at(0, 0, 0, yaw=rng.uniform(15, 40)):
        k.tube((-2.2, 0, .35), (2.1, 0, .32), .36, BARK, r2=.3, n=12)
        k.cyl(2.1, 0, .02, .01, .01, BARK)
        k.rev((2.1, 0, .32), 'x', [(0, .29), (.02, .25)], PALEWOOD, n=12)
        k.rev((-2.2, 0, .35), 'x', [(0, .35), (-.02, .3)], PALEWOOD, n=12)
        k.tube((.4, .2, .5), (.9, .8, .9), .08, BARK, n=6)
        for x in (-1.2, .2, 1.3): k.ball(x, 0, .62, .32, MOSS, ry=.25, rz=.1, n=8, k=4)
    blades(k, rng, -1.0, .9, 6, .5, GRASS)

def c_stumps(k, rng):
    for x, y, r, h in [(-.7, -.4, .42, .55), (.8, .6, .34, .4), (.5, -.9, .25, .3)]:
        k.cyl(x, y, 0, r, h, BARK, n=12, r2=r * .94)
        k.cyl(x, y, h, r * .9, .02, PALEWOOD, n=12)
        for i in range(4):
            a = i * pi / 2 + .4
            k.tube((x + r * .8 * cos(a), y + r * .8 * sin(a), .15), (x + r * 1.7 * cos(a), y + r * 1.7 * sin(a), 0), .07, BARK, n=5)
    blades(k, rng, 0, .3, 5, .4, GRASS)

def c_berries(k, rng):
    foliage(k, rng, 0, 0, .2, 1.0, LEAF2, n=5)
    for _ in range(26):
        a, b = rng.uniform(0, 2 * pi), rng.uniform(.1, 1.2)
        k.ball(.95 * cos(a) * cos(b), .95 * sin(a) * cos(b), .45 + .7 * sin(b), .1, BERRY, n=6, k=3)

def c_mossy(k, rng):
    for x, y, r, h in [(-.6, -.2, .9, .9), (.7, .4, .7, .6), (.1, .9, .5, .45), (-.2, -1.0, .45, .4)]:
        rock(k, rng, x, y, r, h, ROCK_D)
        k.ball(x, y, h * .95, r * .75, MOSS, rz=h * .25, n=10, k=4)

def c_wildflowers(k, rng):
    for _ in range(5): blades(k, rng, rng.uniform(-1.5, 1.5), rng.uniform(-1.5, 1.5), 7, .6, GRASS)
    for _ in range(40):
        a, r = rng.uniform(0, 2 * pi), sqrt(rng.random()) * 1.9
        x, y = r * cos(a), r * sin(a)
        k.tube((x, y, 0), (x, y, .45), .02, GRASS_D, n=4, caps=False)
        k.ball(x, y, .5, .12, rng.choice([FL_WHITE, FL_YELLOW, FL_PURPLE, FL_PINK]), n=6, k=3)

def c_reeds(k, rng):
    k.rev((.6, .6, 0), 'z', [(0, 1.3), (.04, 1.25)], WATERC, n=16)
    for _ in range(26):
        x, y = rng.gauss(-.4, .6), rng.gauss(-.3, .6)
        h = rng.uniform(1.3, 2.2); lx, ly = rng.gauss(0, .12), rng.gauss(0, .12)
        k.tube((x, y, 0), (x + lx, y + ly, h), .03, GRASS if rng.random() < .7 else DRYGRASS, n=4)
        if rng.random() < .4: k.tube((x + lx * .9, y + ly * .9, h * .78), (x + lx * .95, y + ly * .95, h * .92), .07, rgb(96, 64, 40, 'cloth'), n=6)
    for _ in range(3):
        a = rng.uniform(0, 2 * pi)
        k.cyl(.6 + .7 * cos(a), .6 + .7 * sin(a), .045, .22, .02, GRASS, n=10)

def c_tree(k, rng):
    small_tree(k, rng, 0, 0, px(25, k.u))
    blades(k, rng, .6, .8, 6, .5, GRASS)

def c_haybales(k, rng):
    for x, y in [(-.8, -.3), (.9, .4)]:
        with k.at(x, y, 0, yaw=rng.uniform(-30, 30)):
            k.rev((0, -.6, .75), 'y', [(0, .7), (.04, .75), (1.16, .75), (1.2, .7)], STRAW, n=16)
            k.cyl(0, -.62, .75, .01, .01, STRAW)
            for s in (-1, 1): k.rev((0, s * .6, .75), (0, s, 0), [(0, .7), (.01, .4), (.02, .05)], STRAW_D, n=16)

CLUTTER = [c_bush, c_bush,
           lambda k, r: c_flowerbed(k, r, [FL_PINK, FL_PURPLE, FL_WHITE]),
           lambda k, r: c_flowerbed(k, r, [FL_YELLOW, FL_ORANGE, FL_WHITE]),
           c_boulder, c_rockpair, c_tallgrass, c_log, c_stumps, c_berries, c_mossy, c_wildflowers,
           c_reeds, c_tree, c_haybales,
           lambda k, r: c_flowerbed(k, r, [FL_ROSE, FL_WHITE, FL_PINK])]

# =================================================================================================
# 28 Crags: a rock shoulder over the whole diamond (like tops 3, 11 and 12) and jagged outcrops.
def crag(m, seed):
    rng = random.Random(2800 + seed)
    k = Kit(m, .1)          # one cell is 10 m: x, y in 0..10
    style = 'grey' if seed < 8 else 'snow' if seed < 12 else 'sand'
    base_c = {'grey': ROCK_T, 'snow': ICEROCK, 'sand': SAND_R}[style]
    cap_c = {'grey': TURF, 'snow': SNOW, 'sand': SAND_L}[style]
    base_h = 2.2
    if style == 'sand':
        for i, z in enumerate([-.5, .4, 1.3]):
            k.box(.05 * (i % 2), .05 * (i % 2), z, 10 - .1 * (i % 2), 10 - .1 * (i % 2), .9, SAND_R if i % 2 == 0 else SAND_D)
    else:
        k.box(0, 0, -.5, 10, 10, base_h + .5, base_c)
    k.box(.15, .15, base_h - .05, 9.7, 9.7, .3, cap_c)
    # Fallen blocks and buttresses against the two seen faces.
    for _ in range(8):
        side = rng.random() < .5
        x, y = (rng.uniform(2.0, 9.0), 9.8) if side else (9.8, rng.uniform(2.0, 9.0))
        rock(k, rng, x, y, rng.uniform(.6, 1.1), rng.uniform(1.4, 2.6), g.shade(base_c, rng.uniform(.8, 1.08)), n=6, z=-.4)
    # Outcrops: one or two big peaks and a ring of lesser crags; height kept inside the slot.
    def limit(x, y):   # metres that still fit under the top of the 64 px canvas
        return (12.5 + (x + y) / 10 * 16) / (ZC * .1) / 1.06 - base_h
    peaks = []
    n_big = 1 + seed % 2
    for i in range(n_big):
        x, y = (rng.uniform(4.0, 6.5), rng.uniform(4.0, 6.5)) if i == 0 else (rng.uniform(5.5, 7.8), rng.uniform(5.5, 7.8))
        peaks.append((x, y, rng.uniform(2.0, 2.8), min(limit(x, y), rng.uniform(6.5, 9.5))))
    for _ in range(4 + seed % 3):
        a = rng.uniform(0, 2 * pi); d = rng.uniform(2.2, 3.6)
        x, y = 5 + d * cos(a), 5 + d * sin(a)
        peaks.append((x, y, rng.uniform(1.0, 1.7), min(limit(x, y), rng.uniform(2.5, 5.5))))
    for x, y, r, h in sorted(peaks, key=lambda p: p[0] + p[1]):
        lean = (rng.uniform(-.25, .25), rng.uniform(-.25, .25))
        if style == 'sand':
            # Banded pinnacles: stacked frusta of two sandstones, a hard cap-rock.
            z, rr, i = base_h, r * .55, 0
            h = max(h, min(limit(x, y), 5.0))
            n = rng.choice([6, 7])
            while z < base_h + h - 1.9:
                hh = min(rng.uniform(.8, 1.4), base_h + h - z)
                r2 = rr * rng.uniform(.86, .99)
                k.cyl(x + lean[0] * (z - base_h) * .1, y + lean[1] * (z - base_h) * .1, z, rr, hh,
                      SAND_R if i % 2 == 0 else SAND_D, n=n, r2=r2)
                z += hh; rr = r2; i += 1
            k.cyl(x + lean[0] * (z - base_h) * .1, y + lean[1] * (z - base_h) * .1, z, rr * 1.15, .5, SAND_D, n=n, r2=rr * 1.05)
        else:
            col = g.shade(base_c, rng.uniform(.84, 1.08))
            cap = SNOW if style == 'snow' and h > 3 else None
            shard(k, rng, x, y, r, h, col, cap=cap, capf=.55 if h > 6 else .4, n=rng.choice([5, 6, 7]), z=base_h - .2, lean=lean)
    if style == 'grey':
        if seed % 3 == 0:
            for _ in range(1 + seed % 2):
                u, v = rng.uniform(.66, .78), rng.uniform(.66, .78)
                terrain.tree(m, random.Random(seed), u, v, base_h * ZC * .1)
        for _ in range(4):
            k.ball(rng.uniform(1, 9), rng.uniform(1, 9), base_h + .1, rng.uniform(.4, .7), g.shade(LEAF, rng.uniform(.8, 1.1)), rz=.3, n=8, k=4)
    if style == 'snow':
        for _ in range(4):
            k.ball(rng.uniform(1, 9), rng.uniform(1, 9), base_h + .1, rng.uniform(.6, 1.0), SNOW, rz=.35, n=10, k=4)
        for _ in range(3):
            k.ball(rng.uniform(1, 9), 9.8, .2, rng.uniform(.8, 1.4), SNOW, ry=.6, rz=.6, n=10, k=4)

# =================================================================================================
# 29 Parked vehicles: one per cell, centred, along the cell's x axis (0-7) or y axis (8-15).
PARKED = [lambda k: amb.sedan(k, amb.MAROON), lambda k: amb.sedan(k, amb.SILVER), amb.taxi, amb.pickup,
          amb.van, amb.bus, amb.box_truck, amb.tractor]

def parked(m, seed):
    k = Kit(m, amb.U_CAR)
    with centre(k, yaw=0 if seed < 8 else 90): PARKED[seed % 8](k)

# =================================================================================================
# 30 Urban props.
def u_fountain(k, rng):
    k.rev((0, 0, 0), 'z', [(0, 2.9), (.1, 2.9), (.12, 2.6)], STONE_D, n=24)
    k.rev((0, 0, .12), 'z', [(0, 2.6), (.5, 2.6), (.5, 2.75), (.62, 2.75), (.62, 2.45), (.2, 2.4)], STONE, n=24)
    k.cyl(0, 0, .2, 2.42, .3, WATERC, n=24)
    k.rev((0, 0, .5), 'z', [(0, .35), (1.0, .25), (1.1, .9), (1.3, 1.0), (1.35, .9), (1.4, .2), (2.1, .14), (2.2, .5), (2.3, .5), (2.35, .1), (2.8, .08)], STONE, n=16)
    k.cyl(0, 0, 1.75, .85, .1, WATERC, n=16)
    for i in range(10):
        a = i * 2 * pi / 10
        k.tube((0, 0, 3.3), (1.1 * cos(a), 1.1 * sin(a), 1.9), .05, rgb(200, 214, 214, 'water'), n=4)
        k.tube((.9 * cos(a), .9 * sin(a), 1.8), (2.1 * cos(a), 2.1 * sin(a), .55), .05, rgb(200, 214, 214, 'water'), n=4)
    k.ball(0, 0, 3.3, .2, rgb(214, 226, 226, 'water'), n=8, k=4)
    for a in (0.6, 2.2, 3.8, 5.3): bench(k, 4.0 * cos(a), 4.0 * sin(a), degrees(a) + 90, 1.6)

def u_statue(k, rng):
    k.cbox(0, 0, 0, 4.0, 4.0, .25, STONE_D); k.cbox(0, 0, .25, 3.2, 3.2, .25, STONE_D)
    k.cbox(0, 0, .5, 1.8, 1.8, 2.2, STONE); k.cbox(0, 0, 2.7, 2.1, 2.1, .25, STONE)
    with k.at(0, 0, 2.95, yaw=45, s=1.25):   # a seated figure reading a book (generic, unnamed)
        k.cbox(0, 0, 0, 1.1, .9, .15, BRONZE)
        k.cbox(-.15, 0, .15, .7, .8, .45, BRONZE)                          # bench block
        k.stack(-.2, 0, [(.6, .36, .5, .14), (1.2, .32, .5, .12), (1.35, .18, .24, .07)], BRONZE, k=3)
        k.ball(-.16, 0, 1.55, .15, BRONZE, rz=.17, n=10, k=5)
        for s in (-1, 1):
            k.tube((-.15, s * .13, .62), (.3, s * .13, .62), .08, BRONZE, n=6)    # thighs
            k.tube((.3, s * .13, .62), (.34, s * .13, .15), .07, BRONZE, n=6)    # shins
            k.tube((-.2, s * .25, 1.15), (.12, s * .12, .9), .06, BRONZE, n=6)   # arms to the book
        k.cbox(.16, 0, .9, .1, .34, .24, BRONZE)
    for a in (.8, 3.9): bench(k, 3.3 * cos(a), 3.3 * sin(a), degrees(a) + 90, 1.6)

def u_bandstand(k, rng):
    n = 8
    k.cyl(0, 0, 0, 3.0, .6, STONE, n=n)
    k.cyl(0, 0, .6, 2.85, .08, PALEWOOD, n=n)
    for i in range(n):
        a = i * 2 * pi / n
        k.tube((2.6 * cos(a), 2.6 * sin(a), .6), (2.6 * cos(a), 2.6 * sin(a), 3.4), .08, CREAM, n=6)
        b = (i + 1) * 2 * pi / n
        if i != 1: k.strut((2.6 * cos(a), 2.6 * sin(a), 1.4), (2.6 * cos(b), 2.6 * sin(b), 1.4), .06, CREAM)
    k.cyl(0, 0, 3.4, 3.0, .25, CREAM, n=n)
    k.cyl(0, 0, 3.65, 3.1, 1.5, rgb(88, 110, 96, 'roof'), n=n, r2=.3)
    k.cyl(0, 0, 5.15, .3, .5, CREAM, n=n, r2=.05)
    k.box(1.6, -.5, 0, 1.4, 1.0, .3, STONE)

def u_kiosk(k, rng):
    k.cbox(0, 0, 0, 2.4, 2.0, 2.5, rgb(64, 96, 76))
    k.cbox(0, 0, 2.5, 2.8, 2.4, .2, rgb(52, 76, 62))
    k.cyl(0, 0, 2.7, 1.0, .5, rgb(52, 76, 62), n=8, r2=.2)
    k.plate([(1.21, -.8, .9), (1.21, .8, .9), (1.21, .8, 2.1), (1.21, -.8, 2.1)], GLASS)
    k.plate([(-1.2, -.9, 2.3), (1.6, -.9, 2.3), (1.6, .9, 2.3), (-1.2, .9, 2.3)], rgb(64, 96, 76))
    for i in range(4):   # magazine racks: coloured covers, nothing legible
        for j in range(3):
            k.box(1.22, -.85 + i * .44, .2 + j * .22, .04, .36, .18, rgb(*[(214, 200, 160), (196, 172, 90), (160, 160, 150), (120, 150, 110)][(i + j) % 4], 'cloth'))
    k.plate([(1.4, -1.2, 2.45), (1.4, 1.2, 2.45), (1.9, 1.2, 2.1), (1.9, -1.2, 2.1)], AWN_Y)
    bin_(k, -1.2, 1.6)

def u_billboard(k, rng):
    with k.at(.6, .6, 0, yaw=45, s=.8):
        for y in (-1.8, 1.8):
            k.cyl(-.3, y, 0, .18, 4.0, STEEL, n=8)
            k.strut((-.3, y, 3.6), (-1.2, y, 0), .08, STEEL)
        k.box(-.2, -3.2, 3.6, .25, 6.4, 3.2, rgb(70, 72, 72, 'dark'))
        x = .06
        k.plate([(x, -3.0, 3.8), (x, 3.0, 3.8), (x, 3.0, 6.6), (x, -3.0, 6.6)], rgb(232, 226, 206, 'paint'))
        # Abstract graphic: a sun disc, two hills and a band (no lettering, no brand).
        k.plate([(x + .01, -3.0, 3.8), (x + .01, 3.0, 3.8), (x + .01, 3.0, 4.6), (x + .01, -3.0, 4.6)], rgb(66, 112, 80, 'paint'))
        k.plate([(x + .02, -3.0, 4.6), (x + .02, -.6, 4.6), (x + .02, -1.8, 5.6)], rgb(92, 140, 90, 'paint'))
        k.plate([(x + .02, -1.2, 4.6), (x + .02, 1.6, 4.6), (x + .02, .2, 5.3)], rgb(80, 124, 82, 'paint'))
        pts = [(x + .02, 1.6 + .7 * cos(a * pi / 8), 5.7 + .7 * ZC / ZC * sin(a * pi / 8)) for a in range(16)]
        k.plate(pts, rgb(224, 172, 60, 'paint'))
        k.box(.1, -3.0, 6.6, .4, 6.0, .08, DARK)
        for y in (-2, 0, 2): k.tube((.5, y, 6.7), (.9, y, 6.5), .05, DARK, n=4); k.cbox(.95, y, 6.3, .2, .3, .2, WARM)

def u_shelter(k, rng):
    with k.at(0, 0, 0, yaw=45):
        k.cbox(0, 0, 0, 2.0, 4.6, .1, STONE_D)
        for y in (-2.1, 2.1):
            for x in (-.85, .7): k.cyl(x, y, 0, .06, 2.5, STEEL, n=6)
        k.box(-.9, -2.2, .3, .06, 4.4, 2.0, rgb(150, 170, 172, 'glass'))
        for y in (-2.15, 2.15): k.box(-.9, y - .03, .3, 1.5, .06, 2.0, rgb(150, 170, 172, 'glass'))
        k.box(-1.1, -2.4, 2.5, 2.2, 4.8, .14, rgb(60, 66, 66, 'paint'))
        bench(k, -.5, 0, 90, 2.2)
        k.box(-.85, 2.15, .4, .08, 1.0, 1.6, rgb(220, 214, 196, 'emit'))    # lit poster case (blank)
    k.cyl(2.0, -2.0, 0, .05, 2.8, STEEL, n=6); k.cyl(2.0, -2.0, 2.8, .35, .06, rgb(220, 200, 80, 'paint'), n=12)

def u_planters(k, rng):
    for x, y in [(-2.2, -1.2), (.6, 1.4), (2.2, -1.6)]:
        k.cbox(x, y, 0, 1.6, 1.6, .7, PRECAST)
        k.cbox(x, y, .7, 1.4, 1.4, .02, SOIL)
        small_tree(k, rng, x, y, 4.0, LEAF2 if rng.random() < .5 else LEAF)
    bench(k, -.8, -.2, 45)

def u_lamps(k, rng):
    for x, y in [(-2.5, -1.0), (1.5, 2.0), (2.6, -2.2)]: lamp_post(k, x, y, 4.2)
    bench(k, -.6, 1.0, 45); bench(k, 1.2, -.8, 45)
    bin_(k, 2.4, .4)
    k.rect(-3.4, -3.4, .02, 6.8, 6.8, rgb(150, 146, 136, 'concrete'))

def u_watertower(k, rng, H=34):
    h = px(H, k.u)
    legs = h * .55
    for sx, sy in [(-1, -1), (1, -1), (1, 1), (-1, 1)]:
        k.tube((sx * 1.6, sy * 1.6, 0), (sx * 1.1, sy * 1.1, legs), .12, STEEL, n=6)
    for f in (.3, .65):
        z = legs * f; r = 1.6 - .5 * f
        for (ax, ay), (bx, by) in [((-1, -1), (1, -1)), ((1, -1), (1, 1)), ((1, 1), (-1, 1)), ((-1, 1), (-1, -1))]:
            k.strut((ax * r, ay * r, z), (bx * r, by * r, z), .05, STEEL)
    k.ladder(1.3, 1.3, 0, legs, axis='x', w=.4)
    k.rev((0, 0, legs), 'z', [(0, .3), (.3, 1.5), (.5, 1.9), (h - legs - 1.2, 1.9), (h - legs - .3, .6), (h - legs, .1)], rgb(196, 192, 176, 'paint'), n=20)
    k.torus(0, 0, legs + .5, 1.95, .06, STEEL, n=20, k=4)
    k.cyl(0, 0, 0, 2.2, .1, CONCRETE, n=12)

def u_mast(k, rng, H=34):
    h = px(H, k.u)
    k.cbox(0, 0, 0, 2.6, 2.6, .2, CONCRETE)
    k.mast(0, 0, .2, h - 1.0, 1.8, .3, rgb(176, 180, 178, 'steel'), bays=max(6, int(h / 1.4)), t=.06)
    k.tube((0, 0, h - .8), (0, 0, h + .2), .05, STEEL, n=4)
    for z, a in [(h * .7, 30), (h * .5, 150)]:
        with k.at(0, 0, z, yaw=a):
            k.cbox(.6, 0, -.4, .1, .4, .8, rgb(200, 200, 194, 'paint'))
            k.rev((.75, 0, 0), 'x', [(0, .38), (.12, .3), (.16, .02)], rgb(214, 214, 208, 'paint'), n=12)
    k.ball(0, 0, h + .25, .14, AMBER, n=8, k=4)
    k.ball(0, 0, h * .5, .1, AMBER, n=6, k=3)
    k.cbox(2.2, 1.2, 0, 1.8, 1.4, 1.6, CLAD_GREY)

def u_playground(k, rng):
    k.rect(-3.6, -3.6, .02, 7.2, 7.2, rgb(150, 120, 90, 'cloth'))
    # Slide tower and chute.
    for sx, sy in [(0, 0), (1.2, 0), (0, 1.2), (1.2, 1.2)]: k.cyl(-2.4 + sx, -2.4 + sy, 0, .07, 2.6, PAINT_Y, n=6)
    k.box(-2.5, -2.5, 1.5, 1.4, 1.4, .1, PALEWOOD)
    k.cyl(-1.8, -1.8, 2.6, 1.0, .8, PAINT_G, n=4, r2=.05)
    k.extrude([(-1.1, 1.55), (1.0, .1), (1.4, .1), (1.4, .2), (1.0, .25), (-1.1, 1.7)], 'xz', -2.2, -1.5, PAINT_O)
    # Swing frame and two swings.
    for x in (-1.6, 1.6):
        k.strut((x, 1.4, 0), (x, 2.2, 2.6), .08, PAINT_G); k.strut((x, 3.0, 0), (x, 2.2, 2.6), .08, PAINT_G)
    k.strut((-1.6, 2.2, 2.6), (1.6, 2.2, 2.6), .08, PAINT_G)
    for x in (-.7, .7):
        for d in (-.2, .2): k.strut((x + d, 2.2, 2.6), (x + d, 2.2, .55), .015, DARK)
        k.cbox(x, 2.2, .5, .55, .25, .06, rgb(40, 40, 40, 'rubber'))
    # See-saw and a sandpit.
    k.cbox(2.3, -1.2, 0, .3, .3, .5, PAINT_Y)
    with k.at(2.3, -1.2, .55, roll=0, pitch=12): k.cbox(0, 0, 0, .3, 3.2, .08, PAINT_O)
    k.cbox(.4, -2.4, 0, 2.0, 1.6, .25, PALEWOOD); k.cbox(.4, -2.4, .05, 1.8, 1.4, .22, rgb(214, 196, 150, 'cloth'))

def u_market(k, rng):
    for i, (x, y, awn) in enumerate([(-2.0, -1.8, AWN_G), (1.6, -2.2, AWN_Y), (.2, 1.4, AWN_T)]):
        with k.at(x, y, 0, yaw=rng.choice([0, 90]) + 0):
            for sx in (-1, 1):
                for sy in (-1, 1): k.cyl(sx * 1.1, sy * .7, 0, .05, 2.3, GREYWOOD, n=5)
            k.cbox(0, 0, .8, 2.3, 1.5, .08, PALEWOOD)
            for j in range(6):   # stripes on the awning
                y0 = -.85 + j * .29
                c = awn if j % 2 == 0 else CREAM
                k.plate([(-1.25, y0, 2.6 - abs(y0) * .4), (1.25, y0, 2.6 - abs(y0) * .4), (1.25, y0 + .29, 2.6 - abs(y0 + .29) * .4), (-1.25, y0 + .29, 2.6 - abs(y0 + .29) * .4)], c)
            for j in range(5):
                c = rng.choice([FL_YELLOW, rgb(140, 164, 70, 'foliage'), FL_ORANGE, rgb(150, 110, 70, 'foliage')])
                k.cbox(-.9 + j * .45, .2, .88, .4, .5, .18, PALEWOOD)
                for q in range(4): k.ball(-.98 + j * .45 + (q % 2) * .16, .12 + (q // 2) * .16, 1.1, .1, c, n=6, k=3)
            crate(k, .8, -1.1, 0, .5, PALEWOOD)

def u_phone(k, rng):
    with k.at(-.8, -.6, 0):
        k.cbox(0, 0, 0, 1.0, 1.0, .1, STONE_D)
        for sx in (-1, 1):
            for sy in (-1, 1): k.cbox(sx * .45, sy * .45, .1, .1, .1, 2.3, rgb(66, 96, 82, 'paint'))
        for side in range(4):
            with k.at(0, 0, 0, yaw=side * 90): k.box(.43, -.4, .5, .03, .8, 1.7, rgb(150, 170, 172, 'glass'))
        k.cbox(0, 0, 2.4, 1.15, 1.15, .25, rgb(66, 96, 82, 'paint'))
        k.cbox(0, 0, 2.65, .7, .7, .12, rgb(220, 214, 196, 'emit'))
        k.cbox(-.2, 0, 1.1, .2, .4, .5, DARK)
    bench(k, 1.2, 1.0, 45)
    bin_(k, 1.8, -1.6, rgb(70, 92, 70)); bin_(k, 2.5, -1.0, rgb(120, 120, 112))
    k.cbox(-.4, 2.0, 0, .9, .6, 1.1, rgb(140, 120, 70, 'paint'))   # post box (ochre, unbranded)

def u_flags(k, rng):
    k.cbox(0, 0, 0, 7.2, 7.2, .12, rgb(178, 172, 158, 'concrete'))
    for i in range(5): k.box(-3.6 + i * 1.8, -3.6, .12, .05, 7.2, .02, rgb(140, 136, 126, 'concrete'))
    flags = [FLAG_Y, FLAG_W, FLAG_G]
    for i, x in enumerate((-2.0, 0.0, 2.0)):
        y = -x * .4
        k.cyl(x, y, .12, .12, .3, STONE_D, n=8)
        k.tube((x, y, .4), (x, y, 5.6), .06, CHROME, n=6)
        k.ball(x, y, 5.65, .1, BRASS, n=6, k=3)
        wave = [(0, 0), (.8, .12), (1.6, -.05), (2.3, .1)]
        pts_top = [(x + .05 + a * .7 + b * .5, y - a * .7 + b * .5, 5.5) for a, b in wave]
        pts_bot = [(x + .05 + a * .7 + b * .5, y - a * .7 + b * .5, 4.3) for a, b in wave[::-1]]
        k.plate(pts_top + pts_bot, flags[i])
        if i == 1: k.plate([(x + .09 + .2, y - .2, 5.3), (x + .09 + 1.4, y - 1.4, 4.9), (x + .09 + 1.4, y - 1.4, 4.6), (x + .09 + .2, y - .2, 5.0)], FLAG_G)

def u_obelisk(k, rng, H=31):
    h = px(H, k.u)
    k.cbox(0, 0, 0, 4.4, 4.4, .3, STONE_D); k.cbox(0, 0, .3, 3.4, 3.4, .3, STONE_D); k.cbox(0, 0, .6, 2.4, 2.4, .9, STONE)
    k.stack(0, 0, [(1.5, 1.4, 1.4, .02), (h - 1.4, .9, .9, .02)], LIMESTONE, k=1, smooth=False)
    k.cyl(0, 0, h - 1.4, .64, 1.4, BRASS, n=4, r2=.02)
    for a in (.4, 2.0, 3.6, 5.2): k.cyl(2.8 * cos(a), 2.8 * sin(a), 0, .25, .9, STONE_D, n=8)

def u_parking(k, rng):
    k.rect(-3.8, -3.8, .02, 7.6, 7.6, rgb(78, 78, 76, 'asphalt'))
    for i in range(5):
        x = -3.4 + i * 1.7
        k.rect(x, -3.4, .03, .1, 4.0, rgb(226, 224, 214, 'paint'))
    k.rect(-3.4, .6, .03, 6.9, .1, rgb(226, 224, 214, 'paint'))
    for i in range(4): k.cbox(-2.55 + i * 1.7, -3.1, .02, 1.0, .18, .14, rgb(200, 190, 140, 'concrete'))
    with k.at(-2.55, -1.3, 0, yaw=90): amb.sedan(k, rgb(64, 84, 70))
    with k.at(.85, -1.4, 0, yaw=-90): amb.sedan(k, rgb(120, 120, 116), L=3.9, W=1.72, H=1.48, compact=True)

URBAN = [u_fountain, u_statue, u_bandstand, u_kiosk, u_billboard, u_shelter, u_planters, u_lamps,
         lambda k, r: u_watertower(k, r), lambda k, r: u_mast(k, r, 32), u_playground, u_market, u_phone,
         u_flags, lambda k, r: u_obelisk(k, r), u_parking]
URBAN_AT = {8: (.62, .62), 9: (.62, .62), 14: (.58, .58)}   # tall props stand forward to fit the slot

# =================================================================================================
# 31 Rural props.
def r_haystacks(k, rng):
    for x, y, r, h in [(-1.6, -1.0, 1.5, 3.0), (1.4, 1.2, 1.3, 2.6), (1.8, -1.8, 1.0, 2.0)]:
        k.rev((x, y, 0), 'z', [(0, r), (h * .45, r * 1.05), (h * .8, r * .6), (h, .08)], STRAW, n=16)
        k.tube((x, y, h * .9), (x, y, h + .5), .04, GREYWOOD, n=4)

def r_bales(k, rng):
    for x, y, yaw in [(-1.8, -.8, 20), (.2, 1.8, -30), (1.9, -1.2, 70)]:
        with k.at(x, y, 0, yaw=yaw):
            k.rev((0, -.6, .75), 'y', [(0, .7), (.04, .75), (1.16, .75), (1.2, .7)], STRAW, n=16)
            for s in (-1, 1): k.rev((0, s * .6, .75), (0, s, 0), [(0, .7), (.01, .4), (.02, .05)], STRAW_D, n=16)
    k.rev((-.4, -2.0, 0), 'z', [(0, .7), (.04, .75), (1.16, .75), (1.2, .7)], STRAW, n=16)
    k.cyl(-.4, -2.0, 1.2, .7, .02, STRAW_D, n=16)

def r_fence(k, rng):
    pts = [(-3.8, 2.4), (-.4, -1.0), (3.6, 1.6)]
    for (ax, ay), (bx, by) in zip(pts, pts[1:]):
        L = sqrt((bx - ax) ** 2 + (by - ay) ** 2); n = int(L / 1.6)
        for i in range(n + 1):
            x, y = ax + (bx - ax) * i / n, ay + (by - ay) * i / n
            k.cbox(x, y, 0, .14, .14, 1.3, GREYWOOD)
        for z in (.55, 1.05): k.strut((ax, ay, z), (bx, by, z), .1, PALEWOOD)
    blades(k, rng, 0, 1.0, 6, .5, GRASS); blades(k, rng, 2.0, -1.0, 6, .5, GRASS)

def r_well(k, rng):
    k.cyl(0, 0, 0, 1.2, 1.0, rgb(150, 146, 134, 'brick'), n=16)
    k.cyl(0, 0, 1.0, 1.28, .12, STONE, n=16)
    k.cyl(0, 0, .85, .95, .02, WATERC, n=14)
    for y in (-1.0, 1.0): k.cbox(0, y, 0, .18, .18, 2.6, TIMBER)
    with k.at(0, 0, 2.5):
        k.plate([(-1.0, -1.3, 0), (0, -1.3, .9), (0, 1.3, .9), (-1.0, 1.3, 0)], rgb(112, 78, 58, 'roof'))
        k.plate([(1.0, -1.3, 0), (0, -1.3, .9), (0, 1.3, .9), (1.0, 1.3, 0)], rgb(100, 70, 52, 'roof'))
    k.tube((0, -1.1, 2.0), (0, 1.3, 2.0), .07, TIMBER, n=6); k.tube((0, 1.3, 2.0), (.35, 1.4, 1.7), .04, DARK, n=4)
    k.tube((0, 0, 2.0), (0, 0, 1.4), .01, DARK, n=4)
    k.rev((0, 0, 1.05), 'z', [(0, .2), (.35, .24)], TIMBER, n=10)
    k.rev((1.5, .6, 0), 'z', [(0, .2), (.35, .24)], TIMBER, n=10)

def r_windmill(k, rng, H=34):
    h = px(H, k.u)
    body = h * .62
    k.rev((0, 0, 0), 'z', [(0, 2.1), (body, 1.3)], rgb(212, 204, 184, 'concrete'), n=8)
    k.rev((0, 0, body), 'z', [(0, 1.45), (.6, 1.45), (1.6, .8), (2.0, .1)], rgb(96, 78, 62, 'roof'), n=12)
    k.cbox(1.6, 1.6, 0, .1, .1, .1, DARK)
    for i in range(3):   # windows and the door on the seen faces
        z = body * (.25 + i * .25)
        r = 2.1 - .8 * z / body
        k.plate([(r * .72, r * .72, z), (r * .78, r * .58, z), (r * .78, r * .58, z + .7), (r * .72, r * .72, z + .7)], GLASS_D)
    k.plate([(1.48, 1.48, 0), (1.62, 1.2, 0), (1.62, 1.2, 1.6), (1.48, 1.48, 1.6)], TIMBER)
    hub = (1.25, 1.25, body + .8)
    k.tube((0, 0, body + .8), hub, .15, TIMBER, n=6)
    with k.at(*hub, yaw=45):
        L = h - body - .6
        for i in range(4):
            a = radians(20 + i * 90)
            with k.at(0, 0, 0, roll=degrees(a)):
                k.box(-.04, -.08, 0, .08, .16, L, TIMBER)
                for j in range(7):
                    k.box(-.02, .08, L * (.25 + j * .1), .04, .9, .05, PALEWOOD)
                k.box(-.01, .1, L * .25, .02, .85, L * .72, rgb(206, 200, 184, 'cloth'))

def r_silo(k, rng, H=33):
    h = px(H, k.u)
    k.cyl(0, 0, 0, 2.0, h - 1.6, rgb(184, 186, 182, 'corr'), n=20)
    k.dome(0, 0, h - 1.6, 2.0, rgb(196, 198, 194, 'steel'), h=1.4)
    k.cyl(0, 0, h - .3, .3, .3, STEEL, n=8)
    for i in range(int((h - 2) / 1.6)): k.torus(0, 0, 1 + i * 1.6, 2.02, .04, STEEL, n=20, k=4)
    k.ladder(1.45, 1.45, 0, h - 1.4, axis='y', w=.4)
    k.cyl(2.6, -1.4, 0, .9, 3.2, rgb(150, 152, 146, 'corr'), n=14); k.cyl(2.6, -1.4, 3.2, .9, .8, STEEL, n=14, r2=.1)
    k.tube((2.6, -1.4, 4.0), (1.0, -.6, h - 1.0), .12, STEEL, n=6)
    k.cbox(-1.4, 2.4, 0, 1.6, 1.4, 1.8, CLAD_GREY)

def r_scarecrow(k, rng):
    for i in range(7):
        y = -3.2 + i * 1.0
        k.box(-3.4, y - .2, 0, 6.8, .4, .18, SOIL)
        for j in range(9): blades(k, rng, -3.0 + j * .75, y, 4, .7, GRASS if i % 2 else GRASS_D, spread=.15, w=.05)
    k.cyl(0, 0, 0, .06, 3.0, GREYWOOD, n=5)
    k.tube((0, -1.0, 2.2), (0, 1.0, 2.2), .05, GREYWOOD, n=5)
    k.stack(0, 0, [(1.3, .45, .7, .12), (2.35, .4, .9, .12)], rgb(120, 104, 70, 'cloth'), k=2)
    for s in (-1, 1):
        k.tube((0, s * .4, 2.25), (0, s * 1.0, 2.15), .12, rgb(120, 104, 70, 'cloth'), n=6)
        blades(k, rng, 0, s * 1.05, 4, .3, STRAW, spread=.05, w=.04)
    k.ball(0, 0, 2.65, .25, rgb(208, 190, 140, 'cloth'), n=10, k=5)
    k.rev((0, 0, 2.75), 'z', [(0, .55), (.04, .55), (.05, .26), (.35, .2), (.4, .02)], STRAW_D, n=12)

def r_beehives(k, rng):
    cols = [rgb(214, 206, 180), rgb(206, 186, 110), rgb(160, 180, 140), rgb(214, 206, 180)]
    for i, (x, y) in enumerate([(-2.0, -1.4), (-.4, -1.8), (1.2, -2.2), (.6, 1.0)]):
        k.cbox(x, y, 0, .8, .8, .35, TIMBER)
        for j in range(2 + i % 2): k.cbox(x, y, .35 + j * .32, .64, .6, .3, g.shade(cols[i], 1 - .04 * j))
        k.cbox(x, y, .35 + (2 + i % 2) * .32, .74, .7, .1, rgb(150, 150, 146, 'steel'))
    blades(k, rng, -1.0, 1.4, 8, .6, GRASS); blades(k, rng, 2.0, .2, 8, .6, GRASS)
    c_flowerbed_small(k, rng, -2.2, 1.8)

def c_flowerbed_small(k, rng, x0, y0):
    for _ in range(14):
        a, r = rng.uniform(0, 2 * pi), sqrt(rng.random()) * .8
        x, y = x0 + r * cos(a), y0 + r * sin(a)
        k.tube((x, y, 0), (x, y, .4), .02, GRASS_D, n=4, caps=False)
        k.ball(x, y, .45, .12, rng.choice([FL_PURPLE, FL_YELLOW, FL_WHITE]), n=6, k=3)

def r_logpile(k, rng):
    rows = [(5, 0), (4, .5), (3, 1.0)]
    for r_, (n, z) in enumerate(rows):
        for i in range(n):
            y = -1.6 + (5 - n) * .3 + i * .62
            L = rng.uniform(2.6, 3.2)
            k.tube((-L / 2, y, .3 + z * .9), (L / 2, y, .3 + z * .9), .3, BARK, n=10)
            k.rev((L / 2, y, .3 + z * .9), 'x', [(0, .29), (.01, .26)], PALEWOOD, n=10)
    k.cbox(1.6, 2.0, 0, .9, .9, .5, BARK); k.cyl(1.6, 2.0, .5, .3, .02, PALEWOOD)
    k.tube((1.6, 2.0, .6), (1.9, 2.4, 1.4), .04, TIMBER, n=4); k.cbox(1.9, 2.4, 1.3, .3, .1, .2, DARK)

def tent(k, x, y, yaw, L, W, H, c):
    with k.at(x, y, 0, yaw=yaw):
        k.extrude([(-W / 2, 0), (W / 2, 0), (0, H)], 'yz', -L / 2, L / 2, c)
        k.plate([(L / 2 + .01, -W * .2, 0), (L / 2 + .01, W * .2, 0), (L / 2 + .01, 0, H * .6)], DARK)
        for s in (-1, 1): k.tube((s * (L / 2 + .3), 0, 0), (s * L / 2, 0, H + .15), .03, TIMBER, n=4)

def r_tents(k, rng):
    tent(k, -1.4, -1.0, 10, 2.6, 2.0, 1.5, rgb(112, 120, 82, 'cloth'))
    tent(k, 1.6, .8, 70, 2.4, 1.8, 1.4, rgb(196, 146, 82, 'cloth'))
    k.cbox(.4, -2.2, 0, .6, .4, .4, rgb(90, 96, 92, 'paint'))
    k.torus(.2, 2.2, .1, .5, .1, ROCK_D, n=10, k=4)

def r_campfire(k, rng):
    for i in range(9):
        a = i * 2 * pi / 9
        rock(k, rng, .9 * cos(a), .9 * sin(a), .22, .25, ROCK_D, n=5)
    k.cyl(0, 0, 0, .7, .04, rgb(40, 36, 32, 'rubber'), n=12)
    for i in range(5):
        a = i * 2 * pi / 5
        k.tube((.6 * cos(a), .6 * sin(a), .05), (.05 * cos(a), .05 * sin(a), .7), .07, BARK, n=6)
    k.rev((0, 0, .1), 'z', [(0, .4), (.3, .35), (.8, .15), (1.2, .02)], C((255, 196, 96), 'emit'), n=8)
    k.rev((0, 0, .12), 'z', [(0, .22), (.5, .2), (.9, .02)], C((255, 232, 170), 'emit'), n=8)
    for x, y, yaw in [(-2.2, .2, 80), (.4, 2.2, 10)]:
        with k.at(x, y, 0, yaw=yaw): k.tube((-1.1, 0, .3), (1.1, 0, .3), .3, BARK, n=10)
    k.cbox(2.0, -1.4, 0, .5, .3, .5, rgb(70, 74, 72, 'dark'))

def r_cart(k, rng):
    with k.at(0, 0, 0, yaw=30):
        for s in (-1, 1):
            k.torus(-.3, s * 1.0, .9, .85, .06, TIMBER, n=16, k=4, axis='y')
            for i in range(6):
                a = i * pi / 3
                k.strut((-.3, s * 1.0, .9), (-.3 + .85 * cos(a), s * 1.0, .9 + .85 * sin(a)), .05, TIMBER)
        k.tube((-.3, -1.05, .9), (-.3, 1.05, .9), .06, DARK, n=6)
        k.box(-1.6, -.85, 1.0, 2.8, 1.7, .1, PALEWOOD)
        for s in (-1, 1): k.box(-1.6, s * .82 - .03, 1.1, 2.8, .06, .5, PALEWOOD)
        k.box(-1.6, -.85, 1.1, .06, 1.7, .5, PALEWOOD)
        for s in (-1, 1): k.tube((1.2, s * .5, 1.05), (3.0, s * .45, .5), .05, TIMBER, n=5)
        for i in range(3): k.ball(-1.0 + i * .7, 0, 1.4, .35, CANVAS, ry=.3, rz=.25, n=10, k=4)
        k.cbox(1.1, 0, .5, .1, .1, .5, TIMBER)

def r_trough(k, rng):
    with k.at(0, 0, 0, yaw=30):
        for x in (-1.4, 1.4): k.cbox(x, 0, 0, .3, .9, .45, STONE_D)
        k.box(-1.8, -.45, .45, 3.6, .9, .1, TIMBER)
        for s in (-1, 1): k.box(-1.8, s * .42 - .04, .55, 3.6, .08, .45, TIMBER)
        for x in (-1.8, 1.72): k.box(x, -.45, .55, .08, .9, .45, TIMBER)
        k.box(-1.72, -.38, .8, 3.44, .76, .02, WATERC)
        k.tube((1.9, .3, 0), (1.9, .3, 1.5), .05, STEEL, n=6); k.tube((1.9, .3, 1.5), (1.6, .1, 1.3), .04, STEEL, n=4)
    blades(k, rng, -1.5, 1.5, 7, .5, GRASS)

def r_coop(k, rng):
    with k.at(-1.0, -.8, 0):
        for sx in (-1, 1):
            for sy in (-1, 1): k.cbox(sx * .8, sy * .6, 0, .12, .12, .6, TIMBER)
        k.cbox(0, 0, .6, 2.0, 1.6, 1.2, rgb(150, 112, 78, 'wood'))
        k.plate([(-1.1, -.9, 1.8), (0, -.9, 2.5), (0, .9, 2.5), (-1.1, .9, 1.8)], rgb(100, 82, 66, 'roof'))
        k.plate([(1.1, -.9, 1.8), (0, -.9, 2.5), (0, .9, 2.5), (1.1, .9, 1.8)], rgb(90, 74, 60, 'roof'))
        k.plate([(1.0, -.8, 1.8), (1.0, .8, 1.8), (1.0, 0, 2.4)], rgb(150, 112, 78, 'wood'))
        k.plate([(1.006, -.2, .7), (1.006, .2, .7), (1.006, .2, 1.1), (1.006, -.2, 1.1)], DARK)
        k.box(1.0, -.2, .02, 1.2, .4, .05, PALEWOOD)
    for x, y in [(.8, .6), (1.6, -.4), (.2, 1.6), (1.9, 1.2), (-.3, 2.0)]:
        with k.at(x, y, 0, yaw=rng.uniform(0, 360)):
            k.ball(0, 0, .25, .2, rgb(226, 222, 210, 'cloth'), ry=.14, rz=.15, n=8, k=4)
            k.ball(.17, 0, .42, .09, rgb(226, 222, 210, 'cloth'), n=6, k=3)
            k.ball(.24, 0, .42, .03, PAINT_Y, n=4, k=2)
    k.rail([(-.2, 2.8, 0), (2.8, 2.8, 0), (2.8, -1.2, 0)], GREYWOOD, h=.9, t=.03, posts=1.0)

def r_sacks(k, rng):
    k.cbox(0, 0, 0, 2.4, 2.0, .15, PALEWOOD)
    for i in range(3): k.cbox(0, -.8 + i * .8, .15, 2.4, .2, .02, TIMBER)
    for layer in range(3):
        for i in range(3 - layer):
            for j in range(2):
                x = -.75 + i * .75 + layer * .37; y = -.45 + j * .9
                k.ball(x, y, .45 + layer * .45, .38, rgb(200, 186, 150, 'cloth'), ry=.45, rz=.24, n=10, k=4)
    k.ball(1.6, 1.4, .3, .4, rgb(200, 186, 150, 'cloth'), ry=.5, rz=.3, n=10, k=4)

def r_turbine(k, rng, H=34):
    h = px(H, k.u)
    hub = h * .72
    k.cyl(0, 0, 0, 1.4, .3, CONCRETE, n=12)
    k.cyl(0, 0, .3, .45, hub - .3, rgb(222, 222, 216, 'paint'), n=14, r2=.22)
    k.rev((-.3, 0, hub + .2), (1, 1, 0), [(0, .2), (.2, .3), (1.6, .3), (1.8, .22)], rgb(222, 222, 216, 'paint'), n=10)
    hb = (1.25 * .707 + .0, 1.25 * .707, hub + .2)
    k.ball(hb[0], hb[1], hb[2], .25, rgb(222, 222, 216, 'paint'), n=10, k=5)
    L = h - hub - .4
    with k.at(*hb, yaw=45):
        for i in range(3):
            with k.at(.1, 0, 0, roll=15 + i * 120):
                k.loft([[(0, -.15, .2), (.06, -.15, .2), (.06, .2, .2), (0, .2, .2)],
                        [(0, -.05, L), (.03, -.05, L), (.03, .05, L), (0, .05, L)]], rgb(230, 230, 224, 'paint'), smooth=False)

RURAL = [r_haystacks, r_bales, r_fence, r_well, lambda k, r: r_windmill(k, r), lambda k, r: r_silo(k, r),
         r_scarecrow, r_beehives, r_logpile, r_tents, r_campfire, r_cart, r_trough, r_coop, r_sacks,
         lambda k, r: r_turbine(k, r)]
RURAL_AT = {4: (.6, .6), 5: (.6, .6), 15: (.62, .62)}

# =================================================================================================
# 32 Arid (0-7) and cold (8-15) props.
CACTUS = rgb(86, 118, 70, 'foliage'); DESERT_ROCK = (206, 194, 170)
def cactus(k, x, y, h, arms):
    k.rev((x, y, 0), 'z', [(0, .32), (h * .9, .3), (h, .05)], CACTUS, n=10)
    for a, z0, up in arms:
        dx, dy = cos(a), sin(a)
        k.pipe([(x + .2 * dx, y + .2 * dy, z0), (x + .75 * dx, y + .75 * dy, z0 + .1), (x + .8 * dx, y + .8 * dy, z0 + up)], .2, CACTUS, n=8)
        k.ball(x + .8 * dx, y + .8 * dy, z0 + up, .2, CACTUS, n=8, k=4)

def a_cactus(k, rng):
    cactus(k, -.6, -.5, 4.6, [(.4, 1.8, 1.4), (3.4, 2.4, 1.2)])
    cactus(k, 1.4, .9, 3.2, [(1.2, 1.4, .9)])
    k.ball(-.2, 1.6, .3, .45, CACTUS, rz=.45, n=10, k=5)
    for _ in range(3): rock(k, rng, rng.uniform(-2, 2), rng.uniform(-2, 2), .3, .3, DESERT_ROCK, n=5)

def a_deadtree(k, rng):
    wood = GREYWOOD
    k.tube((0, 0, 0), (.2, .1, 3.0), .3, wood, r2=.18, n=8)
    def branch(p, a, el, L, r, depth):
        q = (p[0] + L * cos(a) * cos(el), p[1] + L * sin(a) * cos(el), p[2] + L * sin(el))
        k.tube(p, q, r, wood, r2=r * .6, n=6)
        if depth > 0:
            for d in (-.6, .5): branch(q, a + d + rng.uniform(-.2, .2), el + rng.uniform(-.2, .3), L * .65, r * .6, depth - 1)
    for a in (.3, 2.2, 4.1): branch((.2, .1, 2.6 + a * .1), a, radians(35), 1.5, .14, 2)
    k.tube((-.3, .4, 0), (-.9, .9, .05), .1, wood, n=5); k.tube((.3, -.2, 0), (1.0, -.6, .05), .1, wood, n=5)

def a_rocks(k, rng):
    rock(k, rng, -.8, -.6, 1.3, 1.4, DESERT_ROCK); rock(k, rng, 1.0, .4, 1.0, 1.0, (214, 204, 182))
    rock(k, rng, -.1, 1.4, .6, .5, DESERT_ROCK)
    k.ball(1.5, -1.2, .1, .4, rgb(214, 210, 196, 'concrete'), ry=.25, rz=.15, n=8, k=4)   # a bleached skull-like stone

def a_tent(k, rng):
    cloth = rgb(70, 58, 48, 'cloth'); stripe = rgb(176, 150, 112, 'cloth')
    W, L, H = 3.4, 4.6, 1.8
    with k.at(0, 0, 0, yaw=15):
        for i in range(3):
            y = -L / 2 + i * L / 2
            k.cyl(0, y, 0, .06, H, TIMBER, n=5)
        k.plate([(-W / 2, -L / 2, .9), (0, -L / 2, H), (0, L / 2, H), (-W / 2, L / 2, .9)], cloth)
        k.plate([(W / 2, -L / 2, .9), (0, -L / 2, H), (0, L / 2, H), (W / 2, L / 2, .9)], cloth)
        for i in range(4):
            y = -L / 2 + .3 + i * 1.2
            k.plate([(W / 2 + .01, y, .91), (.01, y, H + .01), (.01, y + .2, H + .01), (W / 2 + .01, y + .2, .91)], stripe)
        k.plate([(-W / 2, L / 2, .9), (0, L / 2, H), (W / 2, L / 2, .9), (W / 2, L / 2, 0), (-W / 2, L / 2, 0)], g.shade(cloth, .9))
        k.plate([(-W / 2, -L / 2, .9), (-W / 2, L / 2, .9), (-W / 2, L / 2, 0), (-W / 2, -L / 2, 0)], g.shade(cloth, .85))
        for s in (-1, 1): k.tube((s * W / 2, L / 2, .9), (s * (W / 2 + .7), L / 2 + .4, 0), .015, STRAW_D, n=3)
        k.box(-1.0, L / 2, .02, 2.0, 1.4, .03, rgb(150, 92, 66, 'cloth'))
    k.cbox(1.8, -1.6, 0, .6, .6, .7, rgb(176, 150, 112, 'wood'))

def a_barrels(k, rng):
    cols = [rgb(84, 92, 70), rgb(120, 82, 56, 'rust'), rgb(196, 160, 60), rgb(96, 96, 92), rgb(120, 82, 56, 'rust')]
    for i, (x, y) in enumerate([(-.7, -.7), (0, -.7), (-.7, 0), (0, 0), (.7, -.7)]):
        barrel(k, x, y, 0, cols[i])
    barrel(k, -.35, -.35, .92, cols[2])
    with k.at(1.4, 1.2, 0, yaw=40): barrel(k, 0, 0, 0, cols[1], lying=True)
    k.cbox(1.4, -1.4, 0, 1.0, 1.0, .12, PALEWOOD)
    k.cyl(-1.6, 1.4, 0, .9, .015, rgb(30, 30, 30, 'rubber'), n=14)

def a_drygrass(k, rng):
    for _ in range(7):
        blades(k, rng, rng.uniform(-1.8, 1.8), rng.uniform(-1.8, 1.8), 10, rng.uniform(.6, 1.0), DRYGRASS if rng.random() < .7 else STRAW_D, w=.05)
    for _ in range(3): rock(k, rng, rng.uniform(-2, 2), rng.uniform(-2, 2), .25, .25, DESERT_ROCK, n=5)

def a_arch(k, rng):
    with k.at(0, 0, 0, yaw=45):
        for y in (-1.9, 1.9): shard(k, rng, 0, y, 1.0, 3.0, SAND_R, n=6)
        prev = None
        for i in range(10):
            a = pi * i / 9
            p = (0, -1.9 * cos(a), 2.4 + 2.0 * sin(a))
            if prev: k.tube(prev, p, .75 - .2 * sin(a), SAND_R if i % 3 else SAND_D, n=7)
            prev = p
        for _ in range(4): rock(k, rng, rng.uniform(-1.5, 1.5), rng.uniform(-3, 3), .4, .4, SAND_D, n=5)

def a_wreck(k, rng):
    rusty = rgb(124, 80, 56, 'rust')
    with k.at(0, 0, -.15, yaw=25, roll=4, pitch=-3):
        amb.car_body(k, 4.4, 1.8, .9, .7, .85, rusty)
        k.loft([[(-1.4, -.85, .9), (.6, -.85, .9), (.6, .85, .9), (-1.4, .85, .9)],
                [(-1.0, -.7, 1.4), (0, -.7, 1.4), (0, .7, 1.4), (-1.0, .7, 1.4)]], rgb(44, 40, 36, 'dark'), smooth=False)
        k.box(-1.05, -.72, 1.4, 1.1, 1.44, .05, rusty)
        k.wheel(-1.4, -.8, .3, .3, .2, tyre=rgb(46, 42, 40, 'rubber'), hub=rusty)
    with k.at(2.0, 1.6, 0, roll=90): k.wheel(0, 0, 0, .3, .2, tyre=rgb(46, 42, 40, 'rubber'), hub=rusty)
    a_drygrass_patch(k, rng)

def a_drygrass_patch(k, rng):
    for _ in range(3): blades(k, rng, rng.uniform(-2.2, 2.2), rng.uniform(-2.2, 2.2), 7, .6, DRYGRASS)

def s_rocks(k, rng):
    for x, y, r, h in [(-.7, -.5, 1.2, 1.4), (1.0, .6, .9, 1.0), (-.2, 1.5, .5, .5)]:
        rock(k, rng, x, y, r, h, ICEROCK)
        k.ball(x, y, h * .92, r * .8, rgb(232, 236, 242, 'cloth'), rz=h * .28, n=10, k=4)
    k.ball(.3, -1.4, 0, .9, rgb(232, 236, 242, 'cloth'), rz=.4, n=10, k=4)

ICE = C((176, 204, 212), 'glass')
def s_ice(k, rng):
    for _ in range(6):
        x, y = rng.uniform(-1.8, 1.8), rng.uniform(-1.8, 1.8)
        with k.at(x, y, 0, yaw=rng.uniform(0, 90), pitch=rng.uniform(-12, 12), roll=rng.uniform(-10, 10)):
            s = rng.uniform(.6, 1.1)
            k.cbox(0, 0, -.1, s, s * rng.uniform(.8, 1.3), s * rng.uniform(.6, 1.2), ICE)
    k.ball(0, 0, 0, 1.8, rgb(232, 236, 242, 'cloth'), rz=.15, n=12, k=4)

def s_spruce(k, rng):
    k.cyl(0, 0, 0, .1, 1.0, BARK, n=6)
    for z, r in [(.4, 1.1), (1.2, .85), (1.9, .6), (2.5, .35)]:
        k.cyl(0, 0, z, r, 1.1, rgb(46, 70, 46, 'foliage'), n=10, r2=.03)
        k.cyl(0, 0, z + .3, r * .78, .3, rgb(232, 236, 242, 'cloth'), n=10, r2=r * .45)
    k.ball(.6, .7, 0, .6, rgb(232, 236, 242, 'cloth'), rz=.25, n=8, k=4)

def s_igloo(k, rng):
    snowc = rgb(226, 232, 238, 'cloth'); seam = rgb(190, 200, 212, 'cloth')
    k.dome(0, 0, 0, 2.2, snowc, h=1.9, n=24, k=8)
    for i in range(1, 5):
        a = i * pi / 2 / 5
        k.torus(0, 0, 1.9 * sin(a), 2.2 * cos(a) + .01, .03, seam, n=24, k=3)
    with k.at(1.6, 1.6, 0, yaw=45):
        k.rev((0, 0, 0), 'x', [(0, .0), (.01, .75), (1.3, .75), (1.31, .0)], snowc, n=12)
        k.rev((1.31, 0, .0), 'x', [(0, .5), (.01, .5)], rgb(40, 44, 50, 'dark'), n=12)
    k.ball(0, 0, -.1, 3.0, snowc, rz=.25, n=14, k=4)

def s_boat(k, rng):
    with k.at(0, 0, -.2, yaw=30, roll=12, pitch=4):
        amb.hull(k, 5.0, 1.8, .7, rgb(110, 84, 62, 'wood'), deck=rgb(96, 74, 56, 'wood'), transom=.7)
        for x in (-1.0, .4): k.cbox(x, 0, .5, .25, 1.5, .08, PALEWOOD)
        k.ball(0, 0, .65, 1.2, rgb(232, 236, 242, 'cloth'), ry=.5, rz=.12, n=10, k=4)
    k.ball(.6, .8, 0, 1.6, rgb(232, 236, 242, 'cloth'), ry=1.0, rz=.5, n=12, k=5)
    k.ball(-1.2, -1.0, 0, 1.2, rgb(232, 236, 242, 'cloth'), ry=1.0, rz=.35, n=12, k=5)
    for _ in range(3):
        with k.at(rng.uniform(-2, 2), rng.uniform(-2, 2), 0, yaw=rng.uniform(0, 90)): k.cbox(0, 0, -.05, .6, .5, .3, ICE)

def s_drift(k, rng):
    for x, y, rx, ry, rz in [(-.6, -.4, 2.0, 1.2, .8), (1.0, .7, 1.4, 1.0, .55), (-1.4, 1.2, 1.0, .8, .4)]:
        k.ball(x, y, 0, rx, rgb(232, 236, 242, 'cloth'), ry=ry, rz=rz, n=16, k=6, lo=0, hi=1)
    rock(k, rng, 1.6, -1.4, .4, .4, ICEROCK, n=5)

def s_hut(k, rng):
    hut = rgb(208, 150, 56)
    for sx in (-1, 1):
        for sy in (-1, 1): k.cbox(-.4 + sx * 1.3, sy * .9, 0, .14, .14, .6, STEEL)
    k.cbox(-.4, 0, .6, 3.0, 2.2, 2.0, hut)
    k.cbox(-.4, 0, 2.6, 3.1, 2.3, .15, rgb(196, 198, 194, 'paint'))
    k.plate([(1.11, -.5, 1.4), (1.11, .5, 1.4), (1.11, .5, 2.1), (1.11, -.5, 2.1)], GLASS)
    k.plate([(-1.4, 1.11, .7), (-.7, 1.11, .7), (-.7, 1.11, 2.3), (-1.4, 1.11, 2.3)], rgb(150, 110, 50))
    k.plate([(0, 1.11, 1.4), (.8, 1.11, 1.4), (.8, 1.11, 2.1), (0, 1.11, 2.1)], GLASS)
    k.box(-1.6, 1.1, .5, .9, .8, .08, STEEL)
    k.tube((-1.6, -.8, 2.75), (-1.6, -.8, 5.0), .05, STEEL, n=6)
    for z in (3.8, 4.6): k.strut((-1.9, -.8, z), (-1.3, -.8, z), .03, STEEL)
    with k.at(.2, -.6, 2.75, yaw=45): k.rev((0, 0, .4), (1, 0, .6), [(0, .5), (.15, .35), (.2, .02)], rgb(214, 214, 208, 'paint'), n=12)
    k.cyl(.2, -.6, 2.75, .06, .4, STEEL, n=6)
    k.ball(2.0, 1.5, 0, 1.0, rgb(232, 236, 242, 'cloth'), rz=.4, n=10, k=4)
    k.cbox(2.0, -1.5, 0, .8, .6, .6, rgb(96, 100, 96, 'paint'))

def s_sled(k, rng):
    with k.at(0, 0, 0, yaw=25):
        for s in (-1, 1):
            k.pipe([(-1.8, s * .45, .06), (1.2, s * .45, .06), (1.6, s * .45, .3), (1.5, s * .45, .5)], .04, rgb(150, 152, 150, 'steel'), n=5)
            for x in (-1.5, -.6, .3, 1.0): k.strut((x, s * .45, .06), (x, s * .42, .5), .05, PALEWOOD)
            k.strut((-1.6, s * .42, .5), (-1.9, s * .45, 1.3), .05, PALEWOOD)
        k.box(-1.6, -.45, .5, 2.6, .9, .06, PALEWOOD)
        k.strut((-1.9, -.45, 1.3), (-1.9, .45, 1.3), .05, PALEWOOD)
        k.ball(-.6, 0, .85, .55, rgb(110, 92, 64, 'cloth'), ry=.4, rz=.35, n=10, k=4)
        k.ball(.4, 0, .75, .4, rgb(84, 100, 84, 'cloth'), ry=.35, rz=.25, n=10, k=4)
        k.tube((1.5, 0, .3), (3.2, 0, .1), .015, rgb(100, 90, 70, 'cloth'), n=3)
    k.ball(-1.0, 1.6, 0, 1.0, rgb(232, 236, 242, 'cloth'), rz=.3, n=10, k=4)

ARID_COLD = [a_cactus, a_deadtree, a_rocks, a_tent, a_barrels, a_drygrass, a_arch, a_wreck,
             s_rocks, s_ice, s_spruce, s_igloo, s_boat, s_drift, s_hut, s_sled]

# =================================================================================================
# 33 Coastal: 0-7 sit on water (no ground drawn), 8-15 on sand.
def w_fishing(k, rng):
    with k.at(.3, .3, 0, yaw=40, s=.55): amb.fishing_boat(k)
    k.tube((-1.8, -1.6, .5), (-3.0, -3.0, 0), .02, rgb(176, 156, 112, 'cloth'), n=3)
    k.ball(-3.1, -3.1, .1, .25, amb.FLOAT_ORANGE, n=8, k=4)

def w_sail(k, rng):
    with k.at(0, 0, 0, yaw=-30, s=.55): amb.sailboat(k)

def w_rowboat(k, rng):
    with k.at(0, 0, 0, yaw=35):
        amb.hull(k, 3.6, 1.4, .45, rgb(150, 112, 78, 'wood'), transom=.75, draft=.2)
        for x in (-.6, .3): k.cbox(x, 0, .3, .25, 1.3, .06, PALEWOOD)
        for s in (-1, 1): k.tube((-.2, s * .7, .4), (.6, s * 1.8, -.05), .03, PALEWOOD, n=4)
        k.tube((1.6, 0, .5), (3.2, -.4, 0), .015, rgb(176, 156, 112, 'cloth'), n=3)

def buoy(k, x, y, c1, c2, h=1.6):
    k.rev((x, y, -.2), 'z', [(0, .55), (.3, .6), (.55, .45)], c1, n=12)
    k.cyl(x, y, .55, .25, h * .3, c2, n=8, r2=.2)
    k.cyl(x, y, .55 + h * .3, .2, h * .3, c1, n=8, r2=.14)
    k.cyl(x, y, .55 + h * .6, .14, .12, DARK, n=8)
    k.ball(x, y, .75 + h * .6, .1, AMBER, n=6, k=3)
    k.torus(x, y, 0, .62, .06, FOAM, n=14, k=3)

def w_buoys(k, rng):
    buoy(k, -1.0, -.8, BUOY_R, rgb(226, 224, 216))
    buoy(k, 1.4, 1.0, rgb(226, 224, 216), BUOY_R, 1.2)
    k.ball(-1.6, 1.6, 0, .25, BUOY_R, n=8, k=4); k.ball(1.6, -1.4, 0, .2, rgb(226, 224, 216), n=8, k=4)

def w_surf(k, rng):
    for x, y, r, h in [(-.6, -.3, 1.3, 1.6), (.9, .6, .9, 1.1), (-.2, 1.4, .6, .6), (1.4, -1.2, .5, .5)]:
        rock(k, rng, x, y, r, h, ROCK_D, z=-.3)
        for _ in range(4):
            a = rng.uniform(0, 2 * pi)
            k.ball(x + r * 1.05 * cos(a), y + r * 1.05 * sin(a), 0, r * .4, FOAM, rz=.08, n=8, k=3)

def w_pier(k, rng):
    with k.at(0, 0, 0, yaw=0):
        for x in (-2.4, -.8, .8, 2.4):
            for y in (-1.1, 1.1): k.cyl(x, y, -.3, .14, 1.9, TIMBER, n=8)
        k.box(-3.0, -1.3, 1.5, 6.0, 2.6, .15, PALEWOOD)
        for i in range(12): k.box(-3.0 + i * .5, -1.3, 1.65, .02, 2.6, .01, TIMBER)
        for x in (-2.6, 2.6): k.cyl(x, 1.1, 1.65, .14, .4, DARK, n=8)
        k.ladder(3.05, .0, 0, 1.6, axis='y', w=.45)
        k.rail([(-3.0, -1.25, 1.65), (3.0, -1.25, 1.65)], GREYWOOD, h=.9, t=.04, posts=1.5)
        for x in (-2.4, .8): k.torus(x, 1.1, 0, .3, .05, FOAM, n=10, k=3)
        rope_coil(k, 1.8, .5, 1.65)

def w_kelp(k, rng):
    for _ in range(9):
        x, y = rng.uniform(-2.2, 2.2), rng.uniform(-2.2, 2.2)
        k.cyl(x, y, .01, rng.uniform(.25, .4), .03, g.shade(GRASS, rng.uniform(.8, 1.1)), n=10)
    for _ in range(3):
        x, y = rng.uniform(-1.5, 1.5), rng.uniform(-1.5, 1.5)
        k.ball(x, y, .1, .14, rng.choice([FL_WHITE, FL_PINK]), rz=.1, n=6, k=3)
    for _ in range(6):
        x, y, a = rng.uniform(-2.4, 2.4), rng.uniform(-2.4, 2.4), rng.uniform(0, 2 * pi)
        k.plate([(x, y, .02), (x + 1.2 * cos(a) - .1 * sin(a), y + 1.2 * sin(a) + .1 * cos(a), .02), (x + 1.4 * cos(a), y + 1.4 * sin(a), .02),
                 (x + .1 * sin(a), y - .1 * cos(a), .02)], rgb(104, 96, 52, 'foliage'))

def w_islet(k, rng):
    for _ in range(6):
        a, d = rng.uniform(0, 2 * pi), rng.uniform(0, 1.6)
        rock(k, rng, d * cos(a), d * sin(a), rng.uniform(.8, 1.4), rng.uniform(.6, 1.4), ROCK_T, z=-.3)
    k.ball(0, 0, .8, 1.5, rgb(96, 112, 60, 'foliage'), rz=.4, n=12, k=5)
    foliage(k, rng, -.3, -.2, 1.0, .7, LEAF, n=3)
    blades(k, rng, .5, .5, 6, .6, DRYGRASS)
    for _ in range(8):
        a = rng.uniform(0, 2 * pi)
        k.ball(2.1 * cos(a), 2.1 * sin(a), 0, .5, FOAM, rz=.06, n=8, k=3)

def b_umbrellas(k, rng):
    for x, y, c in [(-1.2, -1.0, AWN_Y), (1.4, .6, AWN_G)]:
        k.tube((x, y, 0), (x, y, 2.3), .04, CHROME, n=4)
        for i in range(8):
            a, b = i * pi / 4, (i + 1) * pi / 4
            k.plate([(x, y, 2.5), (x + 1.4 * cos(a), y + 1.4 * sin(a), 1.95), (x + 1.4 * cos(b), y + 1.4 * sin(b), 1.95)], c if i % 2 else CREAM)
    for x, y, yaw, c in [(-.3, .2, 30, FLAG_W), (.6, -1.6, 60, rgb(196, 150, 70, 'cloth')), (-2.0, .9, 10, FLAG_G)]:
        with k.at(x, y, .01, yaw=yaw): k.box(-.9, -.4, 0, 1.8, .8, .02, c)
    k.cbox(2.2, -1.2, 0, .6, .4, .4, rgb(206, 206, 198, 'paint')); k.cbox(2.2, -1.2, .4, .62, .42, .08, PAINT_Y)

def b_huts(k, rng):
    cols = [rgb(220, 206, 150), rgb(150, 186, 156), rgb(214, 170, 168), rgb(210, 210, 200)]
    for i in range(3):
        x, y = -2.0 + i * 1.8, -1.0 - i * .2
        c = cols[(i + 1) % 4]
        k.cbox(x, y, 0, 1.5, 1.6, 1.9, c)
        k.plate([(x - .85, y - .9, 1.9), (x, y - .9, 2.6), (x, y + .9, 2.6), (x - .85, y + .9, 1.9)], rgb(86, 84, 80, 'roof'))
        k.plate([(x + .85, y - .9, 1.9), (x, y - .9, 2.6), (x, y + .9, 2.6), (x + .85, y + .9, 1.9)], rgb(80, 78, 74, 'roof'))
        k.plate([(x - .75, y + .8, 1.9), (x + .75, y + .8, 1.9), (x, y + .8, 2.55)], c)
        k.plate([(x - .3, y + .81, 0), (x + .3, y + .81, 0), (x + .3, y + .81, 1.6), (x - .3, y + .81, 1.6)], g.shade(c, .8))
        k.box(x - .75, y + .8, 0, 1.5, .6, .12, PALEWOOD)

def b_lifeguard(k, rng):
    for sx in (-1, 1):
        for sy in (-1, 1): k.strut((sx * 1.1, sy * 1.1, 0), (sx * .8, sy * .8, 2.6), .12, PALEWOOD)
    k.cbox(0, 0, 2.6, 2.4, 2.4, .12, PALEWOOD)
    k.cbox(-.2, -.2, 2.72, 1.6, 1.6, 1.6, rgb(222, 196, 92))
    k.plate([(.61, -.8, 3.4), (.61, .4, 3.4), (.61, .4, 4.1), (.61, -.8, 4.1)], GLASS)
    k.cyl(-.2, -.2, 4.32, 1.4, .8, rgb(214, 214, 206, 'paint'), n=4, r2=.1)
    k.rail([(1.15, -1.15, 2.72), (1.15, 1.15, 2.72), (-1.15, 1.15, 2.72)], PALEWOOD, h=.8, t=.04, posts=.8)
    k.ladder(1.0, 1.4, 0, 2.6, axis='x', w=.5)
    k.tube((-1.0, -1.0, 2.7), (-1.0, -1.0, 5.0), .04, STEEL, n=4)
    k.plate([(-1.0, -1.0, 5.0), (-.4, -.4, 4.8), (-1.0, -1.0, 4.4)], FLAG_Y)
    with k.at(2.0, -1.8, .3, pitch=0): k.box(-1.3, -.3, 0, 2.6, .6, .12, rgb(222, 196, 92))

def b_upturned(k, rng):
    with k.at(0, 0, .5, yaw=30, roll=180): amb.hull(k, 4.6, 1.6, .5, rgb(84, 110, 96, 'paint'), transom=.7, draft=.0)
    for x in (-1.0, 1.0):
        with k.at(0, 0, 0, yaw=30): k.cbox(x, 0, 0, .25, 1.8, .2, TIMBER)
    with k.at(1.6, -1.6, .05, yaw=70):
        k.box(-1.2, -.06, 0, 2.4, .12, .05, PALEWOOD); k.box(.8, -.12, 0, .4, .24, .05, PALEWOOD)

def b_crabpots(k, rng):
    mesh = rgb(70, 74, 72, 'dark')
    for i, (x, y, z) in enumerate([(-1.0, -.8, 0), (0, -.8, 0), (-1.0, .2, 0), (0, .2, 0), (-.5, -.3, .8), (1.2, 1.0, 0)]):
        with k.at(x, y, z, yaw=rng.uniform(-6, 6)):
            k.cbox(0, 0, 0, .9, .9, .78, rgb(130, 128, 116, 'cloth'))
            for d in (-.43, .43):
                k.cbox(d, 0, 0, .06, .92, .8, mesh); k.cbox(0, d, 0, .92, .06, .8, mesh)
            k.cbox(0, 0, .74, .92, .92, .06, mesh)
    rope_coil(k, 1.4, -1.0)
    for x, y in [(1.8, .0), (2.0, .5)]: k.ball(x, y, .25, .25, PAINT_O, n=8, k=4)

def b_lighthouse(k, rng, H=34):
    h = px(H, k.u)
    body = h - 1.8
    k.cyl(0, 0, 0, 1.6, .5, STONE_D, n=16)
    k.rev((0, 0, .5), 'z', [(0, 1.25), (body - .5, .8)], rgb(226, 224, 216, 'paint'), n=16)
    for i in range(3):
        z0 = .5 + (body - .5) * (.2 + i * .28); z1 = z0 + (body - .5) * .12
        r0 = 1.25 - .45 * (z0 - .5) / (body - .5); r1 = 1.25 - .45 * (z1 - .5) / (body - .5)
        k.rev((0, 0, z0), 'z', [(0, r0 + .01), (z1 - z0, r1 + .01)], rgb(44, 46, 48, 'paint'), n=16)
    k.cyl(0, 0, body, 1.05, .15, DARK, n=16)
    k.rail([(1.0 * cos(a * pi / 4), 1.0 * sin(a * pi / 4), body + .15) for a in range(9)], DARK, h=.4, t=.02, posts=.6)
    k.cyl(0, 0, body + .15, .62, .9, rgb(200, 210, 200, 'glass'), n=12)
    k.ball(0, 0, body + .55, .3, WARM, n=8, k=4)
    k.cyl(0, 0, body + 1.05, .7, .7, DARK, n=12, r2=.05)
    k.cbox(1.6, 1.4, 0, 1.6, 1.2, 1.4, rgb(226, 224, 216, 'paint'))
    k.plate([(.8, .8, 1.4), (2.4, .8, 1.4), (2.4, 2.0, 1.4), (.8, 2.0, 1.4)], rgb(70, 70, 70, 'roof'))

def b_dock(k, rng):
    crate(k, -1.2, -.8, 0, 1.1, PALEWOOD, 10); crate(k, -.1, -1.0, 0, .9, rgb(150, 118, 80, 'wood'), -5)
    crate(k, -1.0, -.7, 1.1, .8, rgb(150, 118, 80, 'wood'), 20)
    for x, y in [(1.0, .4), (1.6, .4), (1.3, .95)]: barrel(k, x, y, 0, rgb(120, 82, 56, 'rust'))
    k.cbox(-.4, 1.4, 0, 1.4, 1.1, .15, PALEWOOD)
    k.ball(-.4, 1.4, .45, .5, CANVAS, ry=.4, rz=.3, n=10, k=4)
    rope_coil(k, 1.6, -1.4)
    k.cyl(-2.0, 1.0, 0, .2, .7, DARK, n=8)

def b_driftwood(k, rng):
    for _ in range(4):
        x, y, a = rng.uniform(-1.5, 1.5), rng.uniform(-1.5, 1.5), rng.uniform(0, pi)
        L = rng.uniform(1.6, 3.2)
        p, q = (x - L / 2 * cos(a), y - L / 2 * sin(a), .15), (x + L / 2 * cos(a), y + L / 2 * sin(a), .12)
        k.tube(p, q, rng.uniform(.12, .22), GREYWOOD, r2=.08, n=7)
        k.tube(q, (q[0] + .6 * cos(a + 1), q[1] + .6 * sin(a + 1), .1), .07, GREYWOOD, r2=.03, n=5)
    k.ball(1.4, -1.4, .05, .3, rgb(214, 206, 186, 'concrete'), rz=.12, n=8, k=4)

COASTAL = [w_fishing, w_sail, w_rowboat, w_buoys, w_surf, w_pier, w_kelp, w_islet,
           b_umbrellas, b_huts, b_lifeguard, b_upturned, b_crabpots, lambda k, r: b_lighthouse(k, r), b_dock, b_driftwood]
COASTAL_AT = {1: (.58, .58), 13: (.6, .6)}

ATLASES = {27: (CLUTTER, {}), 30: (URBAN, URBAN_AT), 31: (RURAL, RURAL_AT), 32: (ARID_COLD, {}), 33: (COASTAL, COASTAL_AT)}

def prop(kind, seed):
    m = g.Model(64, 64, (32, 16))
    if kind == 28: crag(m, seed); return m
    if kind == 29: parked(m, seed); return m
    fns, at = ATLASES[kind]
    rng = random.Random(kind * 1000 + seed)
    k = Kit(m, U_PROP)
    cx, cy = at.get(seed, (.5, .5))
    with centre(k, cx, cy): fns[seed](k, rng)
    return m

# =================================================================================================
# 34 Tall landmarks and 35 tall props: 64 x 128 slots, the cell's centre at (32, 96).
def l_cathedral(k, rng):
    st = LIMESTONE; roof = rgb(84, 96, 92, 'roof')
    # Nave along x with a pitched roof, transept, apse; the west front with twin towers at +x.
    k.box(-3.6, -2.0, 0, 6.6, 4.0, 7.0, st)
    k.extrude([(-2.0, 7.0), (2.0, 7.0), (0, 10.0)], 'yz', -3.6, 3.0, roof)
    k.box(-1.6, -3.4, 0, 2.4, 6.8, 6.4, st)
    k.extrude([(-1.6, 6.4), (.8, 6.4), (-.4, 9.2)], 'xz', -3.4, 3.4, roof)
    k.cyl(-3.6, 0, 0, 1.9, 6.0, st, n=12)
    k.cyl(-3.6, 0, 6.0, 2.0, 2.4, roof, n=12, r2=.2)
    for i in range(5):   # buttresses and tall windows along the seen (+y) side
        x = -3.0 + i * 1.3
        k.cbox(x, 2.1, 0, .35, .4, 6.4, st)
        if i < 4: k.plate([(x + .3, 2.006, 2.0), (x + 1.0, 2.006, 2.0), (x + 1.0, 2.006, 5.6), (x + .65, 2.006, 6.2), (x + .3, 2.006, 5.6)], GLASS_W)
    for y in (-1.9, 1.9):
        k.box(3.0, y - 1.2, 0, 2.4, 2.4, 17.5, st)
        for z in (7.5, 12.0):
            k.cbox(5.41, y, z, .02, .9, 2.8, GLASS_D)
            k.cbox(4.2, y + 1.21, z, .9, .02, 2.8, GLASS_D) if y > 0 else None
        k.box(2.9, y - 1.3, 17.5, 2.6, 2.6, .5, st)
        k.cyl(4.2, y, 18.0, 1.15, 5.5, rgb(76, 92, 88, 'roof'), n=8, r2=.05)
        for sx in (-1, 1):
            for sy in (-1, 1): k.cyl(4.2 + sx * 1.15, y + sy * 1.15, 18.0, .18, 1.6, st, n=6, r2=.02)
        k.tube((4.2, y, 23.5), (4.2, y, 24.3), .05, BRASS, n=4)
    k.box(3.0, -.8, 0, 2.2, 1.6, 9.5, st)
    k.extrude([(-.8, 9.5), (.8, 9.5), (0, 11.0)], 'yz', 3.0, 5.2, st)
    k.rev((5.42, 0, 6.6), 'x', [(0, 1.0), (.02, 1.0)], GLASS_W, n=16)             # rose window
    k.rev((5.43, 0, 6.6), 'x', [(0, .25), (.02, .25)], st, n=10)
    k.plate([(5.42, -.5, 0), (5.42, .5, 0), (5.42, .5, 2.6), (5.42, 0, 3.4), (5.42, -.5, 2.6)], TIMBER)

def l_tvtower(k, rng):
    c = rgb(196, 194, 186, 'concrete')
    k.cbox(0, 0, 0, 6.0, 6.0, .3, CONC_DARK)
    for i in range(3):
        a = i * 2 * pi / 3 + .3
        k.tube((2.6 * cos(a), 2.6 * sin(a), 0), (.4 * cos(a), .4 * sin(a), 5.0), .35, c, r2=.25, n=8)
    k.rev((0, 0, 0), 'z', [(0, 1.0), (6, .6), (15.5, .5)], c, n=16)
    k.rev((0, 0, 14.6), 'z', [(0, .5), (.8, 2.0), (1.6, 2.4), (2.6, 2.4), (3.0, 1.8), (3.4, .5)], c, n=24)
    k.rev((0, 0, 16.0), 'z', [(0, 2.42), (1.1, 2.42)], GLASS_T, n=24)
    k.rev((0, 0, 16.4), 'z', [(0, 2.44), (.12, 2.44)], c, n=24)
    k.rev((0, 0, 18.0), 'z', [(0, .4), (2.5, .3)], rgb(214, 214, 208, 'paint'), n=12)
    k.tube((0, 0, 20.5), (0, 0, 24.0), .12, rgb(214, 214, 208, 'paint'), r2=.05, n=8)
    for z in (21.0, 22.5): k.cyl(0, 0, z, .3, .2, rgb(214, 214, 208, 'paint'), n=8)
    k.ball(0, 0, 24.0, .12, AMBER, n=6, k=3)
    k.cbox(2.5, 2.5, 0, 3.0, 2.2, 2.2, CLAD_GREY)

def l_stadium(k, rng):
    seat = [rgb(168, 168, 160, 'concrete'), rgb(196, 176, 92), rgb(200, 200, 194)]
    # Two tiers of stepped terraces in an L across the back corner, a cantilevered roof.
    for i in range(9):
        z = .3 + i * .6; inset = i * .45
        k.box(-5 + 0, -5, 0, 10, 1.0 + inset + .5, z + .6, seat[0])
        k.box(-5, -5, 0, 1.0 + inset + .5, 10, z + .6, seat[0])
        c = seat[1] if i % 3 else seat[2]
        k.box(-5, -5 + 1.0 + inset + .5 - .02, z + .6, 10, .04, .25, c)
        k.box(-5 + 1.0 + inset + .5 - .02, -5, z + .6, .04, 10, .25, c)
    k.box(-5, -5, 0, 10, 1.0, 9.0, CONC_DARK); k.box(-5, -5, 0, 1.0, 10, 9.0, CONC_DARK)
    for i in range(4):
        k.strut((-4.5 + i * 3.0, -4.5, 9.0), (-4.5 + i * 3.0, -.5, 9.8), .18, rgb(196, 198, 194, 'steel'))
        k.strut((-4.5, -4.5 + i * 3.0, 9.0), (-.5, -4.5 + i * 3.0, 9.8), .18, rgb(196, 198, 194, 'steel'))
    k.plate([(-5, -5, 9.6), (5, -5, 9.6), (5, -.4, 10.2), (-.4, -.4, 10.2), (-.4, 5, 10.2), (-5, 5, 9.6)], rgb(214, 214, 210, 'paint'))
    k.rect(-.3, -.3, .02, 5.2, 5.2, rgb(78, 120, 62, 'foliage'))
    k.rect(3.0, -.3, .03, .1, 5.2, rgb(226, 226, 220, 'paint'))
    # Floodlight mast at the front corner.
    x, y, h = 3.4, 3.4, 23.0
    k.cbox(x, y, 0, 1.2, 1.2, .4, CONCRETE)
    k.mast(x, y, .4, h - 2.6, 1.0, .5, rgb(176, 180, 178, 'steel'), bays=12, t=.07)
    with k.at(x, y, h - 2.2, yaw=225):
        k.box(-.2, -1.6, 0, .25, 3.2, 2.4, DARK)
        for i in range(4):
            for j in range(3): k.box(-.26, -1.4 + i * .8, .2 + j * .75, .05, .6, .55, C((255, 244, 214), 'emit'))
    k.ladder(x + .55, y, .4, h - 2.6, axis='y', w=.4)

def l_ferris(k, rng):
    R, zc = 8.4, 10.4
    st = rgb(222, 222, 216, 'paint')
    k.cbox(0, 0, 0, 9.0, 6.0, .4, CONCRETE)
    with k.at(0, 0, 0, yaw=0):     # the wheel turns in the cell's x/z plane
        for s in (-1, 1):
            k.torus(0, s * .5, zc, R, .12, st, n=40, k=5, axis='y')
            k.torus(0, s * .5, zc, R * .8, .06, st, n=36, k=4, axis='y')
            for i in range(16):
                a = i * 2 * pi / 16
                k.strut((0, s * .5, zc), (R * cos(a), s * .5, zc + R * sin(a)), .06, st)
            k.strut((0, s * 2.2, 0), (0, s * .6, zc), .3, rgb(196, 198, 194, 'steel'))
            k.strut((-3.5, s * 2.2, 0), (0, s * .6, zc), .3, rgb(196, 198, 194, 'steel'))
            k.strut((3.5, s * 2.2, 0), (0, s * .6, zc), .3, rgb(196, 198, 194, 'steel'))
        k.tube((0, -1.0, zc), (0, 1.0, zc), .35, DARK, n=10)
        cols = [PAINT_Y, rgb(222, 222, 216), PAINT_G, PAINT_O]
        for i in range(16):
            a = i * 2 * pi / 16 + .1
            px_, pz = R * cos(a), zc + R * sin(a)
            k.tube((px_, -.5, pz), (px_, .5, pz), .05, st, n=4)
            k.stack(px_, 0, [(pz - 1.6, .9, .8, .2), (pz - .6, 1.0, .9, .25), (pz - .45, .7, .6, .2)], cols[i % 4], k=2)
            k.strut((px_, 0, pz - .45), (px_, 0, pz), .03, DARK)
        for i in range(24):
            a = i * 2 * pi / 24
            k.ball(R * 1.0 * cos(a), .62, zc + R * sin(a), .08, AMBER, n=4, k=2)
    k.cbox(3.4, 2.4, 0, 2.0, 1.4, 2.4, rgb(214, 176, 70))
    k.cbox(3.4, 2.4, 2.4, 2.3, 1.7, .2, rgb(206, 206, 198, 'paint'))

def l_spire(k, rng):
    st = rgb(196, 186, 168, 'brick'); roof = rgb(92, 70, 60, 'roof')
    k.box(-4.0, -1.8, 0, 6.0, 3.6, 5.2, st)
    k.extrude([(-1.9, 5.2), (1.9, 5.2), (0, 7.8)], 'yz', -4.0, 2.0, roof)
    for i in range(3): k.plate([(-3.2 + i * 1.8, 1.806, 1.6), (-2.6 + i * 1.8, 1.806, 1.6), (-2.6 + i * 1.8, 1.806, 4.0), (-2.9 + i * 1.8, 1.806, 4.5), (-3.2 + i * 1.8, 1.806, 4.0)], GLASS_W)
    k.box(2.0, -1.3, 0, 2.6, 2.6, 12.0, st)
    for z in (6.0, 9.6): k.cbox(4.61, 0, z, .02, .8, 1.8, GLASS_D); k.cbox(3.3, 1.31, z, .8, .02, 1.8, GLASS_D)
    k.box(1.9, -1.4, 12.0, 2.8, 2.8, .4, st)
    k.cyl(3.3, 0, 12.4, 1.3, 11.2, rgb(74, 96, 90, 'roof'), n=8, r2=.04)
    for sx in (-1, 1):
        for sy in (-1, 1): k.cyl(3.3 + sx * 1.2, sy * 1.2, 12.4, .22, 2.0, st, n=4, r2=.02)
    k.tube((3.3, 0, 23.6), (3.3, 0, 24.6), .04, BRASS, n=4); k.strut((3.3, -.3, 24.2), (3.3, .3, 24.2), .03, BRASS)
    k.plate([(4.61, -.5, 0), (4.61, .5, 0), (4.61, .5, 2.4), (4.61, 0, 3.0), (4.61, -.5, 2.4)], TIMBER)

def clock_face(k, x, y, z, normal, r=.9):
    d = {'x': (1, 0, 0), 'y': (0, 1, 0)}[normal]
    k.rev((x, y, z), d, [(0, r * 1.12), (.03, r * 1.12)], BRASS, n=20)
    k.rev((x + d[0] * .03, y + d[1] * .03, z), d, [(0, r), (.02, r)], rgb(232, 228, 214, 'paint'), n=20)
    e = .06
    hx, hy = d[1], d[0]
    k.strut((x + d[0] * e, y + d[1] * e, z), (x + d[0] * e + hx * .1, y + d[1] * e + hy * .1, z + r * .75), .05, DARK)
    k.strut((x + d[0] * e, y + d[1] * e, z), (x + d[0] * e + hx * r * .55, y + d[1] * e + hy * r * .55, z - r * .2), .06, DARK)
    for i in range(12):
        a = i * pi / 6
        k.ball(x + d[0] * e + hx * r * .85 * cos(a), y + d[1] * e + hy * r * .85 * cos(a), z + r * .85 * sin(a), .05, DARK, n=4, k=2)

def l_clock(k, rng):
    st = rgb(176, 150, 120, 'brick'); trim = LIMESTONE
    k.cbox(0, 0, 0, 7.0, 7.0, .3, STONE_D)
    k.cbox(0, 0, .3, 4.0, 4.0, 15.0, st)
    for z in (5.0, 10.0, 15.0): k.cbox(0, 0, z, 4.3, 4.3, .4, trim)
    for z in (6.5, 11.0):
        k.cbox(2.01, 0, z, .02, .8, 2.0, GLASS_D); k.cbox(0, 2.01, z, .8, .02, 2.0, GLASS_D)
    k.cbox(0, 0, 15.4, 3.6, 3.6, 3.4, st)
    clock_face(k, 1.82, 0, 17.1, 'x'); clock_face(k, 0, 1.82, 17.1, 'y')
    k.cbox(0, 0, 18.8, 4.0, 4.0, .4, trim)
    for sx in (-1, 1):
        for sy in (-1, 1): k.cbox(sx * 1.5, sy * 1.5, 19.2, .5, .5, 2.2, trim)
    k.cbox(0, 0, 19.2, 1.4, 1.4, 2.2, rgb(150, 120, 60, 'copper'))
    k.cbox(0, 0, 21.4, 4.0, 4.0, .3, trim)
    k.cyl(0, 0, 21.7, 2.6, 3.0, rgb(84, 110, 100, 'copper'), n=4, r2=.05)
    k.tube((0, 0, 24.5), (0, 0, 25.4), .04, BRASS, n=4)
    k.plate([(2.01, -.6, .3), (2.01, .6, .3), (2.01, .6, 2.8), (2.01, -.6, 2.8)], TIMBER)

def l_chimney(k, rng):
    from hd_arch import sawtooth
    k.box(-4.6, -4.0, 0, 6.4, 8.0, 5.0, BRICK)
    for i in range(4): k.cbox(-4.6 + 1.6 * i + .8, 4.01, 1.2, 1.0, .02, 2.6, GLASS_D)
    sawtooth(k, -4.6, -4.0, 5.0, 6.4, 8.0, 3, 1.8, CLAD_GREY)
    k.box(1.8, 1.0, 0, 2.6, 3.4, 3.6, CLAD_GREY)
    k.plate([(4.41, 1.6, 0), (4.41, 3.6, 0), (4.41, 3.6, 2.8), (4.41, 1.6, 2.8)], rgb(140, 142, 136, 'corr'))
    k.rev((2.4, -2.2, 0), 'z', [(0, 1.7), (.8, 1.7), (.8, 1.35), (23.0, .85), (23.4, .95), (23.8, .95)], BRICK, n=16)
    for z in (8.0, 15.0, 22.0): k.rev((2.4, -2.2, z), 'z', [(0, 1.36 - z * .022), (.25, 1.36 - z * .022)], DARK, n=16)
    k.cyl(2.4, -2.2, 23.8, .8, .05, BLACK, n=16)
    k.pipe([(1.8, 2.6, 2.0), (2.4, -.8, 2.0), (2.4, -1.2, 1.2)], .3, STEEL, n=8)

def l_hotel(k, rng):
    wall = rgb(206, 200, 186, 'concrete'); rail = rgb(200, 202, 198, 'steel')
    k.box(-4.5, -4.5, 0, 9.0, 9.0, 3.4, rgb(150, 144, 132, 'concrete'))
    k.box(4.45, -3.0, .2, .1, 6.0, 2.8, GLASS_T); k.box(-3.0, 4.45, .2, 6.0, .1, 2.8, GLASS_T)
    k.box(4.5, -2.0, 3.0, 1.2, 4.0, .2, wall)
    k.box(-3.4, -2.4, 3.4, 6.8, 4.8, 15.2, wall)
    for f in range(5):
        z = 3.4 + .4 + f * 3.0
        k.box(-3.3, 2.4, z + .6, 6.6, .02, 1.9, GLASS_T)
        k.box(3.4, -2.3, z + .6, .02, 4.6, 1.9, GLASS_T)
        k.box(-3.4, 2.4, z, 6.8, .9, .15, wall)
        k.rail([(-3.4, 3.3, z + .15), (3.4, 3.3, z + .15)], rail, h=.9, t=.03, posts=1.0)
    k.box(-3.6, -2.6, 18.6, 7.2, 5.2, .4, rgb(150, 144, 132, 'concrete'))
    # Rooftop sign: a frame carrying an abstract lit emblem (a ring and a bar, no lettering).
    with k.at(0, 0, 19.0, yaw=45):
        for y in (-2.6, 2.6): k.strut((0, y, 0), (0, y, 4.2), .1, DARK)
        for z in (.8, 4.2): k.strut((0, -2.6, z), (0, 2.6, z), .08, DARK)
        k.box(-.1, -2.5, .9, .1, 5.0, 3.2, rgb(44, 46, 50, 'dark'))
        k.rev((.02, 0, 2.5), 'x', [(0, 1.3), (.06, 1.3)], SIGN_EMIT, n=24)        # an abstract ring and dot
        k.rev((.05, 0, 2.5), 'x', [(0, .9), (.06, .9)], rgb(44, 46, 50, 'dark'), n=24)
        k.rev((.08, 0, 2.5), 'x', [(0, .42), (.06, .42)], AMBER, n=16)
        for y in (-2.0, 2.0): k.rev((.02, y, 2.5), 'x', [(0, .2), (.06, .2)], AMBER, n=10)
    k.cbox(-2.0, -1.4, 18.6, 1.2, 1.2, 1.4, STEEL)

LANDMARK_SCALE = {5: .9}
LANDMARKS = [l_cathedral, l_tvtower, l_stadium, l_ferris, l_spire, l_clock, l_chimney, l_hotel]
TALL_PROPS = [lambda k, r: u_watertower(k, r, 46), lambda k, r: u_mast(k, r, 56), lambda k, r: r_windmill(k, r, 50),
              lambda k, r: r_silo(k, r, 45), lambda k, r: r_turbine(k, r, 62), lambda k, r: b_lighthouse(k, r, 60)]

def landmark(kind, seed):
    m = g.Model(64, 128, (32, 80))
    rng = random.Random(kind * 1000 + seed)
    if kind == 34:
        k = Kit(m, U_TALL)
        with centre(k, s=LANDMARK_SCALE.get(seed, 1.0)): LANDMARKS[seed](k, rng)
    elif seed < len(TALL_PROPS):
        k = Kit(m, U_PROP)
        with centre(k): TALL_PROPS[seed](k, rng)
    return m

# --- Catalogue ------------------------------------------------------------------------------------
KINDS = {26: 4, 27: 16, 28: 16, 29: 16, 30: 16, 31: 16, 32: 16, 33: 16, 34: 8, 35: 8}
NO_SHADOW = {26, 34, 35}   # tall slots: a baked shadow would be cut at the slot edge (as kinds 16/17)

def model(kind, seed):
    if kind == 26: return skyscraper(seed)
    if kind in (34, 35): return landmark(kind, seed)
    return prop(kind, seed)
