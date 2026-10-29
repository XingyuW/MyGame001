"""Defensive structures, modelled in metres as real fortification and air-defence engineering
(one cell is 10 m; see hd_arch.py for the frame: the camera sees the +x and +y sides).

Each is a real weapon on a believable field foundation: a gravel or earth pad, a revetment, a
poured barbette. Allied works are sand and grey with poured light concrete; Soviet works are dark
protective green on dark concrete, timber and rusting steel. The owner's colour (`TEAM`) is used
the way armies mark kit: a recognition band, a roof panel, a pennant.
"""
from contextlib import contextmanager
from math import sin, cos, pi, radians, degrees, atan2, sqrt
import random
import numpy as np
from hd_kit import *
from hd_arch import *
import hd_vehicles as veh

EARTH = rgb(80, 73, 58, 'roof')          # trampled earth and compacted fill
GRAVEL_DARK = rgb(86, 83, 76, 'roof')
TURF = rgb(82, 96, 58, 'foliage')
NET = [TARP, rgb(66, 84, 52, 'foliage'), rgb(84, 96, 58, 'foliage'), rgb(96, 90, 62, 'foliage'), rgb(62, 70, 50, 'foliage'), rgb(78, 74, 54, 'foliage')]
CERAMIC = rgb(112, 66, 46)                # glazed brown porcelain
SOV_STEEL = rgb(70, 74, 70)               # blackened riveted plate

# --- Shared parts ---------------------------------------------------------------------------------
def ngon(cx, cy, r, z, n=8, a0=pi / 8):
    return [(cx + r * cos(a0 + i * 2 * pi / n), cy + r * sin(a0 + i * 2 * pi / n), z) for i in range(n)]

def prism(k, cx, cy, levels, c, n=8, a0=pi / 8, smooth=False):
    """Polygonal tower: levels = [(z, circumradius)]. With the default a0 an octagon has flats
    facing +x, the camera diagonal and +y."""
    k.loft([ngon(cx, cy, r, z, n, a0) for z, r in levels], c, smooth=smooth)

def annulus(k, cx, cy, z, r0, r1, h, c, n=24, batter=0.0):
    """Ring wall: inner radius r0, outer r1, battered outwards at its foot."""
    k.rev((cx, cy, z), 'z', [(0, r1 + batter), (h, r1), (h, r0), (0, r0)], c, n=n, caps=False)

def ground(k, cx, cy, r, c=EARTH, seed=1, n=26, h=.08):
    """Irregular pad of trampled earth or gravel the work stands on."""
    rnd = random.Random(seed)
    rr = [r * (.93 + .05 * sin(i * 2 * pi / n * 3 + seed) + rnd.uniform(-.02, .02)) for i in range(n)]
    k.loft([[(cx + q * cos(i * 2 * pi / n), cy + q * sin(i * 2 * pi / n), 0) for i, q in enumerate(rr)],
            [(cx + (q - .35) * cos(i * 2 * pi / n), cy + (q - .35) * sin(i * 2 * pi / n), h) for i, q in enumerate(rr)]], c, smooth=False)

@contextmanager
def wall_face(k, cx, cy, yaw, a0, z0, a1, z1):
    """Frame on a battered wall of a polygonal tower: origin on the wall at its foot (apothem a0
    at z0, a1 at z1), +x out of the wall, +y along it, +z up the slope."""
    tilt = degrees(atan2(a0 - a1, z1 - z0))
    with k.at(cx, cy, 0, yaw=yaw):
        with k.at(a0, 0, z0, pitch=tilt): yield k

def bag_ring(k, cx, cy, r, z, layers=3, a0=0.0, a1=360.0, c=SANDBAG, seed=5):
    """Sandbag courses along an arc (degrees)."""
    rnd = random.Random(seed)
    n = max(2, int(radians(a1 - a0) * r / .62))
    for j in range(layers):
        for i in range(n):
            a = a0 + (a1 - a0) * (i + (.5 if j % 2 else 0)) / n
            with k.at(cx + r * cos(radians(a)), cy + r * sin(radians(a)), z + j * .2 + .1, yaw=a + 90 + rnd.uniform(-6, 6)):
                k.ball(0, 0, 0, .34, tint(c, rnd.uniform(.86, 1.06)), ry=.21, rz=.13, n=8, k=3)

def wire_coil(k, a, b, r=.4, pitch=.5, c=IRON, t=.05, seg=8):
    """Concertina wire lying along the line a-b."""
    A, B = np.array(a, float), np.array(b, float)
    d = B - A; L = float(np.linalg.norm(d)); d /= L
    side = np.cross(d, (0, 0, 1.0)); side /= np.linalg.norm(side)
    n = max(seg, int(L / pitch) * seg)
    pts = [A + d * L * i / n + r * (cos(i * 2 * pi / seg) * side + (1 + sin(i * 2 * pi / seg)) * np.array([0, 0, 1.0])) for i in range(n + 1)]
    for p, q in zip(pts, pts[1:]): k.strut(tuple(p), tuple(q), t, c)

def wire_fence(k, pts, h=1.2, c=IRON, strands=3):
    """Barbed wire on screw pickets along a polyline."""
    for a, b in zip(pts, pts[1:]):
        L = sqrt((b[0] - a[0]) ** 2 + (b[1] - a[1]) ** 2); n = max(1, int(round(L / 1.6)))
        for i in range(n + 1):
            px, py = a[0] + (b[0] - a[0]) * i / n, a[1] + (b[1] - a[1]) * i / n
            k.strut((px, py, a[2]), (px, py, a[2] + h), .06, c)
        for j in range(strands):
            zz = a[2] + h * (j + 1) / strands
            k.strut((a[0], a[1], zz), (b[0], b[1], zz), .035, c)
        k.strut((a[0], a[1], a[2] + h), (b[0], b[1], a[2] + h / 3), .03, c)

def camo_net(k, x0, y0, x1, y1, zf, nx=8, ny=8, seed=3, keep=None, cols=None):
    """Garnished net over a rectangle: zf(x, y) gives its height, keep(x, y) masks cells."""
    rnd = random.Random(seed); cols = cols or NET
    P = [[(x0 + (x1 - x0) * i / nx, y0 + (y1 - y0) * j / ny) for j in range(ny + 1)] for i in range(nx + 1)]
    with k.grp(True):
        for i in range(nx):
            for j in range(ny):
                q = [P[i][j], P[i + 1][j], P[i + 1][j + 1], P[i][j + 1]]
                mx, my = sum(p[0] for p in q) / 4, sum(p[1] for p in q) / 4
                c = rnd.choice(cols)
                if keep and not keep(mx, my): continue
                k.poly([(p[0], p[1], zf(p[0], p[1])) for p in q], c)

def decal(k, quad, uv, c, lift=.03):
    """Flat marking on a planar quad (p00, p10, p11, p01), its outline given in the quad's u, v."""
    p00, p10, p11, p01 = [np.array(p, float) for p in quad]
    nrm = np.cross(p10 - p00, p01 - p00); nrm /= np.linalg.norm(nrm)
    for s in (1, -1):
        k.plate([tuple((p00 * (1 - u) + p10 * u) * (1 - v) + (p01 * (1 - u) + p11 * u) * v + nrm * lift * s) for u, v in uv], c)

def blob(cu, cv, ru, rv, seed, n=9):
    rnd = random.Random(seed)
    return [(cu + ru * rnd.uniform(.65, 1.0) * cos(i * 2 * pi / n), cv + rv * rnd.uniform(.65, 1.0) * sin(i * 2 * pi / n)) for i in range(n)]

def helix(k, cx, cy, z0, z1, r0, r1, turns, rt, c, seg=16, n=6):
    m = int(turns * seg)
    pts = [(cx + (r0 + (r1 - r0) * i / m) * cos(2 * pi * i / seg), cy + (r0 + (r1 - r0) * i / m) * sin(2 * pi * i / seg), z0 + (z1 - z0) * i / m) for i in range(m + 1)]
    for a, b in zip(pts, pts[1:]): k.tube(a, b, rt, c, n=n, caps=False)

def log_wall(k, a, b, courses=3, r=.14, c=TIMBER):
    """Revetment of horizontal logs held by pairs of driven posts; returns its top height."""
    (x0, y0), (x1, y1) = a, b
    L = sqrt((x1 - x0) ** 2 + (y1 - y0) ** 2); ux, uy = (x1 - x0) / L, (y1 - y0) / L
    for j in range(courses):
        e = .12 * (j % 2)
        k.tube((x0 - ux * e, y0 - uy * e, r + j * r * 1.85), (x1 + ux * e, y1 + uy * e, r + j * r * 1.85), r, tint(c, .92 + .12 * (j % 2)), n=6)
    top = r + (courses - 1) * r * 1.85 + r
    n = max(1, int(round(L / 1.7)))
    for i in range(n + 1):
        px, py = x0 + (x1 - x0) * (i + .0) / n, y0 + (y1 - y0) * i / n
        for s in (-1, 1): k.tube((px - uy * s * (r + .09), py + ux * s * (r + .09), 0), (px - uy * s * (r + .09), py + ux * s * (r + .09), top + .12), .09, tint(c, .8), n=5)
    return top

def cage_ladder(k, x, y, z0, z1, axis='x', c=None, out=(0, 1), cage_from=2.4):
    """Fixed ladder with safety hoops; `out` is the unit direction away from the wall."""
    c = c or GALV
    k.ladder(x, y, z0, z1, c, axis=axis)
    z = z0 + cage_from
    while z < z1 - .1:
        k.torus(x + out[0] * .36, y + out[1] * .36, z, .38, .025, c, n=10, k=4); z += 1.1
    for a in (-60, 0, 60):
        ang = atan2(out[1], out[0]) + radians(a)
        px, py = x + out[0] * .36 + .38 * cos(ang), y + out[1] * .36 + .38 * sin(ang)
        k.strut((px, py, z0 + cage_from), (px, py, z1), .03, c)

def placard(k, y, z, w=.9, h=.6):
    """Warning sign on a wall, in a wall frame (+x out of the wall)."""
    k.box(.02, y - w / 2, z, .04, w, h, HAZ)

# --- Pillbox --------------------------------------------------------------------------------------
def embrasure(k, x, z, w=2.0, h=.95, c=None):
    """Stepped firing slit, in a frame whose +x points out of the wall (wall face at x)."""
    c = c or tint(CONCRETE, .96)
    for ww, hh, bar, out in [(w, h, .2, .3), (w - .4, h - .4, .11, .16)]:
        z0 = z + (h - hh) / 2
        k.box(x - .3, -ww / 2, z0, .3 + out, ww, bar, c); k.box(x - .3, -ww / 2, z0 + hh - bar, .3 + out, ww, bar, c)
        k.box(x - .3, -ww / 2, z0, .3 + out, bar, hh, c); k.box(x - .3, ww / 2 - bar, z0, .3 + out, bar, hh, c)
        c = CONC_DARK
    k.box(x - .3, -w / 2 + .3, z + .3, .36, w - .6, h - .6, BLACK)

def pillbox(k, w, d):
    # Allied reinforced-concrete machine-gun bunker: an octagonal pillbox with battered walls,
    # stepped embrasures on the faces towards the enemy, a porch with a steel door, a net over
    # the roof, a sandbag apron and a wire obstacle.
    cx, cy = 4.8, 4.8
    ground(k, 5, 5, 4.8, EARTH, 3)
    prism(k, cx, cy, [(0, 3.95), (.3, 3.95), (.3, 3.65), (2.55, 3.3), (2.55, 3.44), (2.95, 3.44), (3.15, 3.12)], CONCRETE)
    zr = 3.15
    # Formwork lift line and damp staining round the foot.
    prism(k, cx, cy, [(.3, 3.67), (.62, 3.62)], CONC_DARK)
    for a in (0, 45):
        with k.at(cx, cy, 0, yaw=a): embrasure(k, 3.2, 1.05)
    with k.at(cx, cy, 0, yaw=0):  # the gun: a water-cooled heavy machine gun in the front slit
        k.tube((3.1, 0, 1.52), (4.25, 0, 1.5), .075, GUN, n=8); k.tube((4.25, 0, 1.5), (4.5, 0, 1.495), .045, GUN, n=6)
        k.rev((4.42, 0, 1.495), 'x', [(0, .045), (.05, .09), (.14, .07)], GUN, n=6)
    with k.at(cx, cy, 0, yaw=45):
        k.tube((3.1, .2, 1.52), (3.95, .3, 1.5), .06, GUN, n=8)
    with k.at(cx, cy, 0, yaw=90):  # entrance porch on the flank
        k.box(2.8, -1.35, 0, .85, 1.5, 2.3, tint(CONCRETE, .94)); k.box(2.8, -1.45, 2.3, .95, 1.7, .18, CONC_LIGHT)
        k.box(3.65, -1.1, .12, .07, 1.0, 1.9, rgb(88, 96, 86))
        for zz in (.5, 1.75): k.box(3.72, -1.1, zz, .03, 1.0, .1, DARK)
        k.box(3.72, -1.1, 1.25, .02, 1.0, .36, TEAM)
        k.torus(3.76, -.6, 1.1, .16, .03, DARK, n=10, k=4, axis='x')
        k.box(3.65, -1.3, 0, .6, 1.4, .12, CONC_DARK)
        k.box(3.1, .55, 1.35, .22, .5, .32, CONC_LIGHT); k.box(3.14, .65, 1.43, .2, .3, .14, BLACK)  # pistol port
        k.box(3.68, -.75, 2.05, .1, .3, .12, LAMP)
    # Roof: net over the rear, a recognition panel, periscope, ventilator, aerial.
    def net_z(x, y):
        r = sqrt((x - cx) ** 2 + (y - cy) ** 2)
        return zr + .14 + .07 * sin(x * 3.1) * cos(y * 2.7) if r < 2.85 else max(.25, zr + .1 - (r - 2.85) * 2.4)
    camo_net(k, cx - 4.2, cy - 4.2, cx + 2.6, cy + 2.6, net_z, 22, 22, seed=4,
             keep=lambda x, y: (x - cx) + (y - cy) < .9 + .5 * sin(3 * (x - y)) and sqrt((x - cx) ** 2 + (y - cy) ** 2) < 4.1)
    rnd = random.Random(2)
    for i in range(12):
        a, r = rnd.uniform(pi * .8, pi * 1.75), rnd.uniform(.3, 2.7)
        k.ball(cx + r * cos(a), cy + r * sin(a), zr + .2, rnd.uniform(.2, .34), NET[1 + i % 4], rz=.16, n=7, k=3)
    k.box(cx + .35, cy + .35, zr, 1.6, 1.6, .03, TEAM)
    k.tube((cx + 2.1, cy - .7, zr), (cx + 2.1, cy - .7, zr + .7), .07, DARK, n=6); k.cbox(cx + 2.14, cy - .7, zr + .62, .2, .18, .2, DARK)
    k.cbox(cx + 2.25, cy - .7, zr + .66, .02, .12, .12, GLASS)
    vent(k, cx - .4, cy + 2.1, zr, .24, .45, IRON)
    veh.antenna(k, cx + 1.9, cy + 1.2, zr, 2.6)
    k.cyl(cx + 2.0, cy + .2, zr, .32, .1, IRON, n=10)  # roof hatch
    # Sandbag apron below the slits, wire on screw pickets across the front corner.
    bag_ring(k, cx, cy, 4.25, .06, 3, -32, 30, seed=5); bag_ring(k, cx, cy, 4.25, .06, 2, 30, 62, seed=6)
    bag_ring(k, cx, cy, 4.3, .06, 3, 108, 140, seed=7)
    wire_fence(k, [(9.55, 6.3, .05), (9.55, 9.55, .05), (6.3, 9.55, .05)], 1.25)
    wire_coil(k, (9.2, 7.0, .05), (9.2, 9.2, .05), .33); wire_coil(k, (9.2, 9.2, .05), (7.0, 9.2, .05), .33)
    crate(k, 1.2, 7.6, .06, .8, c=OLIVE, yaw=20)

# --- Sentry gun -----------------------------------------------------------------------------------
def sentry_gun(k, w, d):
    # Soviet automatic weapon station: a remotely laid twin heavy machine gun on a steel
    # pedestal, in a log-and-sandbag revetment, cabled to a junction box.
    ground(k, 5, 5, 4.8, EARTH, 5)
    walls = [((8.3, 1.8), (8.3, 8.3)), ((8.3, 8.3), (3.6, 8.3)), ((1.7, 1.8), (8.3, 1.8)), ((1.7, 6.2), (1.7, 1.8))]
    for a, b in walls:
        top = log_wall(k, a, b, 3)
        sandbags(k, [(a[0], a[1], top - .02), (b[0], b[1], top - .02)], 2)
    k.box(2.0, 2.1, .06, 6.0, 5.9, .06, rgb(116, 96, 68, 'wood'))  # duckboard floor
    for i in range(9): k.box(2.0 + i * .68, 2.1, .12, .05, 5.9, .012, tint(TIMBER, .7))
    k.cbox(5, 5, .06, 2.3, 2.3, .26, CONC_DARK)
    c = SOV_GREEN
    with k.at(5, 5, .32, s=1.5):
        k.cyl(0, 0, 0, .62, .09, DARK, n=14)
        k.rev((0, 0, .09), 'z', [(0, .44), (.22, .3), (1.0, .25), (1.1, .4), (1.2, .4)], c, n=14)
        for a in range(4):
            with k.at(0, 0, .09, yaw=a * 90 + 45): k.extrude([(.2, 0), (.58, 0), (.24, .5)], 'xz', -.03, .03, c)
        with k.at(0, 0, 1.2, yaw=-16):
            k.cyl(0, 0, 0, .42, .1, DARK, n=14); k.cbox(0, 0, .1, .7, 1.0, .14, c)
            for s in (-1, 1): k.extrude([(-.3, .24), (.3, .24), (.14, .98), (-.14, .98)], 'xz', s * .44 - .05, s * .44 + .05, c)
            with k.at(0, 0, .8, pitch=5):
                k.cbox(-.1, 0, -.17, 1.3, .62, .36, SOV_DARK)  # twin receiver housing
                k.tube((0, -.52, 0), (0, .52, 0), .07, DARK, n=8)  # trunnions
                for s in (-1, 1):
                    k.tube((.5, s * .16, .02), (2.35, s * .16, .02), .04, GUN, n=8)
                    k.rev((.55, s * .16, .02), 'x', [(0, .085), (1.0, .085), (1.06, .045)], IRON, n=8)
                    k.rev((2.25, s * .16, .02), 'x', [(0, .04), (.08, .1), (.36, .07)], GUN, n=8)
                    k.box(-.5, s * .7 - .17, -.36, .66, .34, .52, OLIVE)  # ammunition boxes
                    k.box(-.5, s * .7 - .17, .16, .66, .34, .03, tint(OLIVE, .8))
                    k.box(-.16, min(s * .3, s * .56), .0, .2, .26, .1, STEEL)  # feed chutes
                # Shield with a recognition band along its top edge.
                with k.at(.66, 0, 0, pitch=12):
                    for s in (-1, 1): k.box(0, min(s * .32, s * .95), -.5, .05, .63, 1.0, c)
                    k.box(0, -.95, .18, .05, 1.9, .32, c); k.box(.05, -.95, .2, .02, 1.9, .3, TEAM)
                # Sensor head: day camera, thermal sight, laser rangefinder.
                k.cbox(-.3, 0, .19, .14, .14, .28, DARK)
                k.cbox(-.25, 0, .46, .56, .6, .36, tint(c, .9)); k.cbox(-.25, 0, .82, .44, .48, .02, TEAM)
                k.rev((.03, -.14, .64), 'x', [(0, .11), (.07, .11)], GLASS, n=10)
                k.rev((.03, .14, .68), 'x', [(0, .07), (.07, .07)], GLASS_DARK, n=8); k.cbox(.04, .14, .52, .02, .1, .06, RED_LAMP)
    # Junction box on a post, armoured cable across the floor, ready ammunition.
    k.box(7.3, 2.25, 0, .12, .12, 1.7, DARK); k.box(7.05, 2.38, .9, .62, .26, .75, rgb(92, 98, 90))
    k.box(7.12, 2.64, 1.25, .48, .02, .3, HAZ)
    k.pipe([(5.75, 4.55, .42), (6.3, 3.9, .16), (7.0, 2.9, .16), (7.36, 2.55, .16), (7.36, 2.5, .9)], .06, RUBBER, n=6)
    k.pipe([(7.36, 2.3, 1.65), (7.36, 2.3, 2.0), (8.1, 1.9, 1.0)], .04, RUBBER, n=5)
    veh.antenna(k, 7.36, 2.31, 1.7, 3.0)
    crate(k, 2.3, 2.3, .12, .95, c=OLIVE); crate(k, 3.4, 2.25, .12, .85, c=rgb(84, 92, 64, 'wood'), yaw=8); crate(k, 2.4, 2.4, .9, .7, c=OLIVE, yaw=30)
    for i in range(3): k.box(2.2, 3.6 + i * .42, .12, .62, .34, .4, tint(OLIVE, .85 + .1 * i))
    k.box(2.25, 3.65, .52, .6, .34, .38, OLIVE)
    k.tube((2.0, 7.9, 0), (2.0, 7.9, 3.6), .08, TIMBER, n=6)
    with k.at(2.0, 7.9, 3.4, yaw=-45, pitch=-30): k.cbox(.22, 0, 0, .34, .4, .34, DARK); k.box(.39, -.15, .04, .02, .3, .26, LAMP)

# --- Flak cannon ----------------------------------------------------------------------------------
def shell_rack(k, x, y, z, n=5, yaw=0.0, r=.085):
    """Ready-use rack: fixed rounds standing nose-up in a timber frame."""
    with k.at(x, y, z, yaw=yaw):
        k.box(-.15, -.2, 0, n * .26 + .3, .4, .14, TIMBER); k.box(-.15, -.2, .5, n * .26 + .3, .06, .08, TIMBER); k.box(-.15, .14, .5, n * .26 + .3, .06, .08, TIMBER)
        for xx in (-.15, n * .26 + .07): k.box(xx, -.2, 0, .08, .4, .6, TIMBER)
        for i in range(n):
            k.cyl(.13 + i * .26, 0, .14, r, .7, BRASS, n=8)
            k.rev((.13 + i * .26, 0, .84), 'z', [(0, r * .92), (.22, r * .9), (.42, .02)], rgb(72, 78, 64), n=8)

def flak_cannon(k, w, d):
    # Soviet heavy anti-aircraft gun in a concrete-ringed pit, after the 100 mm KS-19: a long
    # barrel at high elevation on a cruciform platform, with shield, recuperator, fuse setter
    # and ready-use racks.
    cx, cy = 5.0, 5.0
    ground(k, 5, 5, 4.85, EARTH, 7)
    annulus(k, cx, cy, 0, 3.8, 4.3, .9, CONC_DARK, n=24, batter=.3)
    k.cyl(cx, cy, 0, 3.8, .1, GRAVEL_DARK, n=24)
    bag_ring(k, cx, cy, 4.05, .9, 2, 118, 345, seed=9)
    c = SOV_GREEN
    with k.at(cx - .45, cy + .1, .1, s=1.28):
        for a in (45, 135, 225, 315):  # cruciform platform with levelling jacks
            with k.at(0, 0, 0, yaw=a):
                k.box(.3, -.18, .16, 2.35, .36, .26, c)
                k.cyl(2.5, 0, 0, .3, .07, IRON, n=10); k.tube((2.5, 0, .07), (2.5, 0, .62), .06, STEEL, n=6); k.cbox(2.5, 0, .6, .34, .06, .05, DARK)
        k.cbox(0, 0, .1, 1.5, 1.5, .34, c)
        k.rev((0, 0, .44), 'z', [(0, .74), (.3, .56), (.72, .5), (.8, .64), (.9, .64)], c, n=14)
        with k.at(0, 0, 1.34, yaw=-16):
            k.cyl(0, 0, 0, .62, .1, DARK, n=14)
            k.cbox(-.05, 0, .1, 1.5, 1.3, .2, c)
            for s in (-1, 1):
                k.extrude([(-.6, .3), (.6, .3), (.22, 1.22), (-.22, 1.22)], 'xz', s * .52 - .06, s * .52 + .06, c)
                k.torus(-.3, s * .82, .78, .19, .03, DARK, n=10, k=4, axis='y'); k.tube((-.3, s * .56, .78), (-.3, s * .82, .78), .03, DARK, n=5)
                k.cbox(-.85, s * .78, .3, .34, .34, .08, DARK); k.cbox(-.85, s * .78, 0.1, .08, .08, .2, DARK)  # layers' seats
                k.tube((.62, s * .34, .34), (.3, s * .34, 1.0), .1, tint(c, .9), n=8)  # equilibrators
            k.box(-.8, .62, .3, .4, .34, .5, tint(c, .86))  # fuse setter
            for yy in (.7, .88): k.rev((-.6, yy, .8), 'z', [(0, .075), (.04, .075)], BRASS, n=8)
            with k.at(.72, 0, .12, pitch=14):  # shield, open in the centre for the elevating barrel
                for s in (-1, 1):
                    out = [(.32, 0), (1.3, 0), (1.62, .45), (1.62, 1.45), (1.25, 1.8), (.32, 1.8)]
                    k.extrude([(s * a, b) for a, b in (out if s > 0 else out[::-1])], 'yz', 0, .05, c)
                    k.box(.05, min(s * .36, s * 1.6), .8, .02, 1.24, .62, TEAM)
            with k.at(0, 0, 1.05, pitch=50):  # elevating mass
                k.cbox(.2, 0, -.44, 2.7, .52, .34, c)
                k.cbox(-1.25, 0, -.34, .62, .62, .66, GUN)
                k.rev((-.95, 0, 0), 'x', [(0, .27), (1.3, .27), (1.3, .22), (2.7, .19), (2.7, .165), (5.3, .12)], tint(c, .92), n=12)
                k.rev((4.32, 0, 0), 'x', [(0, .12), (0, .2), (.14, .2), (.14, .13), (.2, .13), (.2, .2), (.36, .2), (.36, .13), (.42, .13), (.42, .2), (.56, .2), (.56, .09)], GUN, n=10)
                k.rev((4.9, 0, 0), 'x', [(0, .07), (.01, .07)], BLACK, n=8)
                k.rev((-.9, 0, .37), 'x', [(0, .13), (2.3, .13), (2.4, .08)], c, n=10)  # recuperator
                for s in (-1, 1): k.rev((-.6, s * .36, -.2), 'x', [(0, .1), (1.9, .1), (1.9, .05), (2.6, .05)], tint(c, .9), n=8)
    # Ready-use racks and crates against the far wall, the battery cable, a pennant.
    shell_rack(k, 1.9, 3.9, .1, 5, yaw=-62); shell_rack(k, 3.3, 2.0, .1, 5, yaw=-22)
    crate(k, 5.3, 1.6, .1, .9, c=OLIVE, yaw=8); k.box(6.4, 1.75, .1, 1.3, .5, .42, rgb(84, 92, 64, 'wood'))
    for i in range(4): k.tube((2.0 + i * .2, 5.6, .2), (2.9 + i * .2, 6.6, .2), .085, BRASS, n=6)  # spent cases
    k.pipe([(5.0, 5.9, .3), (4.6, 7.4, .16), (4.2, 8.55, .16), (4.2, 8.75, 1.0)], .05, RUBBER, n=5)
    flagpole(k, 1.7, 8.4, 0, 5.6, w=1.7)
    k.box(8.86, 3.9, .3, .05, .9, .55, HAZ)

# --- Rocket tower ---------------------------------------------------------------------------------
def missile(k, L=5.4, r=.25, c=None):
    """Two-stage surface-to-air missile lying along +x: booster, sustainer, fins, nose."""
    c = c or rgb(176, 178, 172)
    k.rev((0, 0, 0), 'x', [(0, r * 1.2), (.08, r * 1.36), (1.75, r * 1.36), (1.95, r), (L - 1.3, r), (L - .4, r * .56), (L, .03)], c, n=10)
    k.rev((-.12, 0, 0), 'x', [(0, r * .8), (.12, r * 1.05)], DARK, n=8)
    k.rev((1.5, 0, 0), 'x', [(0, r * 1.38), (.26, r * 1.38)], SOV_GREEN, n=10)
    k.rev((L - 1.75, 0, 0), 'x', [(0, r + .012), (.42, r + .012)], TEAM, n=10)
    k.rev((L - .4, 0, 0), 'x', [(0, r * .57), (.4, .035)], SIGNAL_RED, n=8)
    for a in (45, 135, 225, 315):
        with k.at(0, 0, 0, roll=a):
            k.plate([(.05, 0, r), (1.0, 0, r), (.7, 0, r + .62), (.05, 0, r + .62)], SOV_GREEN)
            k.plate([(L - 2.6, 0, r), (L - 2.0, 0, r), (L - 2.15, 0, r + .34), (L - 2.5, 0, r + .34)], c)

def rocket_tower(k, w, d):
    # Soviet rocket battery on a reinforced tower: a battered concrete drum with a steel collar,
    # a railed deck, and a trainable four-rail launcher (after the S-125's 5P73) with a davit
    # for reloading.
    cx, cy = 5.0, 5.0
    ground(k, 5, 5, 4.7, EARTH, 2)
    R = lambda z: 3.3 - (z - .4) / 5.5 * .75
    prism(k, cx, cy, [(0, 3.75), (.4, 3.75), (.4, 3.3), (5.9, 2.55)], CONC_DARK)
    prism(k, cx, cy, [(3.3, R(3.3) + .05), (4.5, R(4.5) + .05)], SOV_STEEL)  # riveted steel collar
    for zz in (3.3, 4.42): prism(k, cx, cy, [(zz, R(zz) + .11), (zz + .1, R(zz + .1) + .11)], IRON)
    prism(k, cx, cy, [(4.72, R(4.72) + .04), (5.85, R(5.85) + .04)], TEAM)
    prism(k, cx, cy, [(5.9, 2.6), (6.05, 3.55), (6.35, 3.55)], CONCRETE)
    zd = 6.35
    k.plate(ngon(cx, cy, 3.4, zd + .012), CONC_DARK)
    a0, a1 = 3.3 * cos(pi / 8), 2.55 * cos(pi / 8)
    for i in range(8):  # steel ribs up the arrises, corbels under the deck
        a = pi / 8 + i * pi / 4
        k.strut((cx + 3.33 * cos(a), cy + 3.33 * sin(a), .4), (cx + 2.59 * cos(a), cy + 2.59 * sin(a), 5.9), .14, SOV_STEEL)
    for yaw in (0, 45, 90):
        with wall_face(k, cx, cy, yaw, a0, .4, a1, 5.9):
            k.box(-.02, -.12, 1.9, .06, .24, .9, BLACK)  # loophole
            k.box(.0, -.5, 3.02, .04, .12, 1.3, RUST); k.box(.0, .35, 3.4, .04, .1, .9, RUST)  # rust runs below the collar
    with wall_face(k, cx, cy, 45, a0, .4, a1, 5.9):  # cable conduits down to the junction cabinet
        for yy in (-.55, -.35): k.pipe([(.1, yy, 5.5), (.1, yy, .9), (.5, yy, .5)], .06, IRON, n=6)
        for zz in (1.6, 3.0, 4.9): k.box(0, -.68, zz, .14, .46, .08, DARK)
        placard(k, .45, 1.2, .9, .6)
    with k.at(cx, cy, 0, yaw=45):
        k.box(3.2, -.95, .0, .7, 1.0, 1.25, rgb(84, 90, 80)); k.box(3.9, -.85, .25, .03, .8, .85, tint(SOV_GREEN, .8))
    with k.at(cx, cy, 0, yaw=90):  # door in a concrete surround, with steps and a lamp
        k.box(2.6, -.75, .0, .75, 1.5, 2.7, CONCRETE); k.box(3.35, -.5, .45, .06, 1.0, 2.0, rgb(74, 62, 50))
        k.box(3.35, -.75, 0, .7, 1.5, .22, CONC_DARK); k.box(3.35, -.6, .22, .4, 1.2, .22, CONC_DARK)
        k.box(3.38, -.15, 2.5, .1, .3, .12, LAMP)
    cage_ladder(k, cx + 3.2, cy + .2, .1, zd + 1.0, axis='y', c=IRON, out=(1, 0))
    for zz in (1.5, 3.4, 5.3): k.strut((cx + 3.2, cy + .2, zz), (cx + R(zz) * cos(pi / 8), cy + .2, zz), .05, IRON)
    pts = ngon(cx, cy, 3.42, zd)
    k.rail(pts + [pts[0]], IRON, h=1.05, t=.05, posts=1.4)
    # Launcher.
    c = SOV_GREEN
    with k.at(cx, cy, zd, yaw=-17, s=1.1):
        k.cyl(0, 0, 0, 1.3, .18, DARK, n=18)
        k.rev((0, 0, .18), 'z', [(0, 1.05), (.8, .85), (.9, .95), (1.0, .95)], c, n=14)
        k.cbox(-.1, 0, 1.18, 1.3, 1.5, .3, SOV_DARK)
        for s in (-1, 1): k.extrude([(-.65, 1.18), (.65, 1.18), (.3, 2.3), (-.3, 2.3)], 'xz', s * .8 - .1, s * .8 + .1, c)
        k.box(-1.15, -.5, 1.2, .5, 1.0, .8, tint(c, .86))  # drive cabinet
        k.tube((.5, 0, 1.3), (.9, 0, 2.45), .09, CHROME, n=6)  # elevating ram
        with k.at(0, 0, 2.1, pitch=30):
            k.tube((0, -1.75, 0), (0, 1.75, 0), .17, c, n=8)
            k.cbox(.2, 0, -.25, 1.9, 1.3, .36, SOV_DARK)
            for i in range(4):
                y = -1.35 + i * .9
                k.box(-2.3, y - .1, .08, 4.5, .2, .16, c)
                for xx in (-1.5, 0, 1.4): k.box(xx - .08, y - .14, -.05, .16, .28, .16, DARK)
                with k.at(-2.5, y, .24 + .34): missile(k)
    # Reloading davit, a tracking sight, a spare round on a trestle below.
    with k.at(cx, cy, zd, yaw=142):
        k.cyl(2.7, 0, 0, .22, .25, DARK, n=8)
        k.tube((2.7, 0, .2), (2.7, 0, 3.0), .1, rgb(168, 128, 44), n=8)
        k.box(.9, -.08, 2.9, 2.1, .16, .22, rgb(168, 128, 44)); k.strut((2.6, 0, 2.0), (1.7, 0, 2.9), .08, rgb(168, 128, 44))
        k.strut((1.0, 0, 2.9), (1.0, 0, 1.9), .03, DARK); k.cbox(1.0, 0, 1.7, .16, .1, .22, DARK)
    with k.at(cx, cy, zd, yaw=62):
        k.tube((2.85, 0, 0), (2.85, 0, 1.3), .06, DARK, n=6); k.cbox(2.85, 0, 1.3, .5, .4, .36, tint(c, .9)); k.cbox(3.1, 0, 1.38, .02, .26, .2, GLASS)

# --- Patriot --------------------------------------------------------------------------------------
def canister(k, L=5.6, w=1.04, h=.98, c=NATO_SAND):
    """Missile canister along +x from 0, sitting on z = 0, centred on y."""
    k.box(0, -w / 2, 0, L, w, h, c)
    for i in range(8):
        x = .12 + i * (L - .34) / 7
        k.box(x, -w / 2 - .035, -.02, .1, w + .07, h + .055, tint(c, .86))
    k.box(L, -w / 2 + .06, .06, .03, w - .12, h - .12, tint(c, 1.1))
    k.box(L + .03, -w / 2 + .16, .16, .012, w - .32, h - .32, tint(c, .96))
    k.box(-.03, -w / 2 + .08, .08, .03, w - .16, h - .16, DARK)

def patriot(k, w, d):
    # Allied Patriot launching station (M901): four ribbed missile canisters, two by two, raised
    # 38 degrees over a semi-trailer on its outriggers, with the generator on the gooseneck, the
    # electronics module, the data-link mast and the cable reel.
    c = NATO_SAND
    ground(k, 5, 5, 4.85, GRAVEL_DARK, 4)
    with k.at(5.0, 5.0, .08, yaw=-17, s=1.1):
        # Trailer: frame, bogie, gooseneck, mudguards.
        k.box(-3.6, -1.22, .95, 5.9, 2.44, .26, c); k.box(-3.6, -.5, .7, 5.9, 1.0, .26, DARK)
        k.box(2.0, -1.22, 1.21, 1.9, 2.44, .3, c); k.box(2.0, -1.22, .95, .3, 2.44, .3, c)
        for x in (-2.75, -1.55):
            for s in (-1, 1):
                k.wheel(x, s * 1.05, .5, .5, .3); k.wheel(x, s * .72, .5, .5, .3, hub=DARK)
        for s in (-1, 1):
            y0 = min(s * 1.24, s * .56)
            k.box(-3.4, y0, 1.06, 2.5, .68, .05, tint(c, .9)); k.box(-3.42, y0, .55, .04, .68, .5, RUBBER)
            k.box(-3.62, s * .8 - .12, 1.0, .03, .24, .14, SIGNAL_RED)
        k.box(-3.66, -1.22, .82, .08, 2.44, .16, DARK)
        # Outriggers on timber packing, and the front landing legs.
        for x in (-3.3, 1.5):
            for s in (-1, 1):
                k.box(x - .13, min(s * 1.2, s * 2.15), .98, .26, .95, .2, c)
                k.tube((x, s * 2.05, .04), (x, s * 2.05, 1.25), .08, STEEL, n=6)
                k.cbox(x, s * 2.05, .08, .5, .5, .06, DARK); k.cbox(x, s * 2.05, 0, .7, .7, .09, WOOD)
        for s in (-1, 1):
            k.tube((3.2, s * .8, .04), (3.2, s * .8, 1.22), .07, STEEL, n=6); k.cbox(3.2, s * .8, 0, .34, .34, .06, DARK)
        # Generator set on the gooseneck: louvres, exhaust, fuel cans.
        k.box(2.25, -1.05, 1.51, 1.5, 2.1, 1.12, tint(c, .97)); k.box(2.2, -1.1, 2.63, 1.6, 2.2, .06, tint(c, .88))
        for i in range(5): k.box(2.4 + i * .26, 1.05, 1.75, .16, .02, .7, DARK)
        for i in range(5): k.box(3.75, -.8 + i * .34, 1.75, .02, .22, .7, DARK)
        k.tube((2.6, -.7, 2.69), (2.6, -.7, 3.1), .06, DARK, n=6); k.tube((2.6, -.7, 3.1), (2.45, -.7, 3.22), .06, DARK, n=6)
        k.box(2.25, 1.07, 1.51, .5, .03, .2, TEAM)
        # Launcher electronics module and the cable entry panel.
        k.box(.35, -1.1, 1.21, 1.5, 2.2, 1.0, tint(c, .95)); k.box(.5, 1.1, 1.4, 1.1, .03, .6, tint(c, .8))
        for i in range(3): k.cbox(.7 + i * .35, 1.14, 1.5, .1, .04, .1, DARK)
        # Turntable, launcher platform, trunnion towers, elevating struts.
        k.cyl(-1.75, 0, 1.21, 1.18, .18, DARK, n=20)
        k.box(-3.2, -1.12, 1.39, 3.4, 2.24, .2, tint(c, .92))
        for s in (-1, 1):
            k.extrude([(-2.65, 1.59), (-1.55, 1.59), (-1.9, 2.25), (-2.3, 2.25)], 'xz', s * 1.22 - .07, s * 1.22 + .07, c)
            k.tube((-.2, s * .9, 1.6), (-.1, s * .9, 3.25), .09, c, n=8); k.tube((-.15, s * .9, 2.5), (-.07, s * .9, 3.7), .055, CHROME, n=6)
            k.strut((-.2, s * .9, 1.6), (-1.25, s * .9, 2.5), .1, c)
        with k.at(-2.1, 0, 2.05, pitch=38):
            # Canister support frame and the four rounds.
            for s in (-1, 1): k.box(-.7, s * 1.14 - .06, -.14, 5.6, .12, .2, c)
            for x in (-.6, 1.3, 3.2, 4.75): k.box(x, -1.2, -.16, .14, 2.4, .16, c)
            k.box(-.75, -1.14, -.1, .14, 2.28, 2.3, tint(c, .9))  # rear blast frame
            for row in range(2):
                for s in (-1, 1):
                    with k.at(-.55, s * .56, .04 + row * 1.08): canister(k, c=c)
            # Recognition band round the pack.
            for s in (-1, 1): k.box(3.5, s * 1.13 - .02, .0, .65, .04, 2.14, TEAM)
            k.box(3.5, -1.1, 2.12, .65, 2.2, .03, TEAM)
        # Data-link antenna mast, clamped to the gooseneck.
        k.tube((3.65, -1.0, 1.5), (3.65, -1.0, 4.4), .075, c, n=8); k.tube((3.65, -1.0, 4.4), (3.65, -1.0, 6.6), .05, c, n=6)
        k.rev((3.65, -1.0, 4.36), 'z', [(0, .1), (.1, .1)], DARK, n=8)
        k.cyl(3.65, -1.0, 6.6, .13, .7, WHITE, n=8); k.tube((3.65, -1.0, 7.3), (3.65, -1.0, 7.9), .015, DARK, n=4)
        k.strut((3.65, -1.0, 5.4), (3.2, -.2, 2.7), .02, DARK); k.strut((3.65, -1.0, 5.4), (2.6, -1.05, 2.7), .02, DARK)
        veh.antenna(k, 2.3, .95, 2.69, 2.4)
    # Cable reel, power and fibre cables, earthing stake, a few sandbags on the packing.
    with k.at(7.7, 7.9, .08, yaw=35):
        for yy in (-.36, .36): k.rev((0, yy - .03, .62), 'y', [(0, .62), (.06, .62)], rgb(96, 104, 92), n=14)
        k.rev((0, -.33, .62), 'y', [(0, .42), (.66, .42)], RUBBER, n=12)
        for xx in (-.5, .5): k.box(xx - .04, -.45, 0, .08, .9, .12, DARK)
    k.pipe([(7.4, 7.6, .14), (6.9, 7.2, .14), (6.4, 6.9, .14), (6.0, 6.3, .5), (5.8, 5.9, 1.5)], .05, RUBBER, n=5)
    k.pipe([(8.0, 8.2, .14), (8.3, 8.4, .14), (8.5, 8.1, .12)], .05, RUBBER, n=5)
    k.tube((6.4, 8.5, .05), (6.4, 8.5, .7), .03, COPPER, n=4); k.strut((6.4, 8.5, .6), (6.1, 7.0, .2), .025, COPPER)
    sandbags(k, [(1.0, 6.3, .08), (2.3, 7.4, .08)], 2); sandbags(k, [(4.6, 8.7, .08), (5.8, 8.5, .08)], 2)
    crate(k, 3.1, 8.3, .08, .8, c=OLIVE, yaw=-17)

# --- Prism tower ----------------------------------------------------------------------------------
def prism_tower(k, w, d):
    # Allied directed-energy tower: a slim steel monopole carrying a faceted beam-combining lens
    # in a steel cage, over a stack of cooling fins, fed by conduits from a capacitor bank.
    cx, cy = 4.9, 4.9
    ground(k, 5, 5, 4.8, GRAVEL_DARK, 8)
    prism(k, cx, cy, [(0, 2.9), (.25, 2.9), (.25, 2.6), (.8, 2.45), (.8, 2.2), (.95, 2.2)], CONCRETE)
    col = OFFWHITE; zb, zt = .95, 10.4
    rc = lambda z: .82 - (z - zb) / (zt - zb) * .3
    k.rev((cx, cy, zb), 'z', [(0, 1.15), (.1, 1.15), (.1, .95), (.5, .84)], GALV, n=16)  # base flange
    for i in range(8):
        a = i * pi / 4 + pi / 8
        k.cyl(cx + 1.05 * cos(a), cy + 1.05 * sin(a), zb + .1, .07, .12, DARK, n=6)
        with k.at(cx, cy, zb + .1, yaw=degrees(a)): k.extrude([(.8, 0), (1.1, 0), (.8, .7)], 'xz', -.03, .03, GALV)
    k.rev((cx, cy, zb), 'z', [(0, rc(zb)), (zt - zb, rc(zt))], col, n=16)
    for zz in (3.0, 5.2, 7.36): k.rev((cx, cy, zz), 'z', [(0, rc(zz) + .07), (.14, rc(zz) + .07)], GALV, n=16)
    k.rev((cx, cy, 7.5), 'z', [(0, rc(7.5) + .015), (1.9, rc(9.4) + .015)], TEAM, n=16)
    # Power conduits and a coolant line up the column.
    for a, cc, r in [(18, DARK, .07), (34, DARK, .07), (64, COPPER, .05)]:
        ca, sa = cos(radians(a)), sin(radians(a))
        k.pipe([(cx + 2.3 * ca, cy + 2.3 * sa, .95), (cx + (rc(1.6) + .12) * ca, cy + (rc(1.6) + .12) * sa, 1.6),
                (cx + (rc(zt) + .12) * ca, cy + (rc(zt) + .12) * sa, zt), (cx + .75 * ca, cy + .75 * sa, zt + .5)], r, cc, n=6)
    cage_ladder(k, cx - .1, cy + 1.02, zb, zt + .1, axis='x', out=(0, 1), cage_from=2.6)
    for zz in (3.0, 6.0, 9.0): k.strut((cx - .1, cy + 1.02, zz), (cx - .1, cy + rc(zz), zz), .04, GALV)
    # Service platform.
    for i in range(8):
        a = i * pi / 4 + pi / 8
        k.strut((cx + rc(9.4) * cos(a), cy + rc(9.4) * sin(a), 9.4), (cx + 1.85 * cos(a), cy + 1.85 * sin(a), zt), .07, GALV)
    k.loft([ngon(cx, cy, 2.05, zt), ngon(cx, cy, 2.05, zt + .1)], GALV, smooth=False)
    k.plate(ngon(cx, cy, 1.95, zt + .112), rgb(92, 98, 100, 'steel'))
    pts = ngon(cx, cy, 2.0, zt + .1); k.rail(pts + [pts[0]], GALV, h=1.05, t=.045, posts=1.2)
    # Cooling fins round the laser housing.
    k.cyl(cx, cy, zt + .1, .62, 1.9, rgb(104, 110, 114), n=16)
    for i in range(8): k.rev((cx, cy, zt + .5 + i * .19), 'z', [(0, .66), (.03, 1.12), (.07, 1.12), (.1, .66)], GALV, n=16)
    # The lens: a faceted glass body in a cage of steel ribs, with the emitter at its crown.
    z0 = zt + 2.0
    lev = [(0, .6), (1.2, 1.5), (1.9, 1.75), (2.5, 1.5), (4.6, .42)]
    k.rev((cx, cy, z0 - .12), 'z', [(0, .8), (.12, .8)], GALV, n=16)
    prism(k, cx, cy, [(z0 + z, r) for z, r in lev], GLASS, n=6, a0=pi / 4)
    for i in range(6):
        a = pi / 4 + i * pi / 3
        for (za, ra), (zb_, rb) in zip(lev, lev[1:]):
            k.strut((cx + (ra + .03) * cos(a), cy + (ra + .03) * sin(a), z0 + za), (cx + (rb + .03) * cos(a), cy + (rb + .03) * sin(a), z0 + zb_), .06, GALV)
    for z, r in lev[1:4]:
        q = ngon(cx, cy, r + .03, z0 + z, 6, pi / 4)
        for a, b in zip(q, q[1:] + q[:1]): k.strut(a, b, .045, GALV)
    k.rev((cx, cy, z0 + 4.6), 'z', [(0, .48), (.12, .48), (.12, .26), (.26, .26)], GALV, n=12)
    k.ball(cx, cy, z0 + 5.1, .27, PLASMA, n=12, k=6)
    for i in range(3):
        a = i * 2 * pi / 3 + .4
        k.pipe([(cx + .42 * cos(a), cy + .42 * sin(a), z0 + 4.72), (cx + .48 * cos(a), cy + .48 * sin(a), z0 + 5.15), (cx + .1 * cos(a), cy + .1 * sin(a), z0 + 5.5)], .035, BRASS, n=5)
    k.cyl(cx, cy, z0 + 5.46, .12, .1, GALV, n=8); k.tube((cx, cy, z0 + 5.56), (cx, cy, z0 + 6.2), .02, DARK, n=4)
    # Relay mirrors on the gallery that fold neighbouring towers' beams into the lens.
    for a in (-10, 100, 190, 280):
        with k.at(cx, cy, zt + .1, yaw=a):
            k.tube((1.75, 0, 0), (1.75, 0, 1.25), .05, GALV, n=5)
            with k.at(1.75, 0, 1.45, pitch=-38):
                k.cbox(0, 0, -.5, .06, .9, 1.0, GALV); k.cbox(-.04, 0, -.46, .02, .8, .92, GLASS)
    # Capacitor bank and switchgear at the foot, a transformer, bollards.
    with k.at(8.1, 4.3, .08, yaw=0):
        k.box(-.75, -1.5, 0, 1.5, 3.0, .15, CONC_DARK)
        k.box(-.6, -1.35, .15, 1.2, 2.7, 1.9, rgb(150, 156, 152)); k.box(-.65, -1.4, 2.05, 1.3, 2.8, .06, rgb(120, 126, 124))
        for i in range(3):
            k.box(.6, -1.25 + i * .88, .3, .03, .76, 1.6, rgb(136, 142, 138)); k.box(.63, -1.2 + i * .88, 1.5, .02, .66, .26, DARK)
        for i in range(3): k.box(.6, -1.25 + i * .88, .95, .04, .76, .4, TEAM); k.box(.6, .51, .9, .04, .5, .36, HAZ)
        for i in range(3): insulator(k, 0, -.9 + i * .9, 2.11, .7, .13, n=4)
        k.pipe([(0, .9, 2.85), (-1.2, .9, 2.85), (-2.2, .7, 1.2)], .04, COPPER, n=5)
        k.pipe([(0, 0, 2.85), (-1.2, 0, 2.85), (-2.2, .2, 1.2)], .04, COPPER, n=5)
    transformer(k, 2.3, 7.4, .08, rgb(142, 148, 140), s=.62)
    k.pipe([(3.0, 7.0, .2), (3.6, 6.4, .2), (4.0, 6.0, .95)], .08, DARK, n=6)
    bollards(k, [(8.0, 8.0), (8.7, 6.9), (6.9, 8.7)], .08)
    k.box(5.9, 8.2, .08, .08, .08, 1.5, GALV); k.box(5.6, 8.26, 1.0, .7, .03, .5, HAZ)

# --- Tesla coil -----------------------------------------------------------------------------------
def tesla_coil(k, w, d):
    # Soviet Tesla coil at weapon scale: a concrete resonator base, a conical primary of copper
    # tube inside a strike rail, a wound secondary on porcelain, and a spun-copper toroid with a
    # discharge terminal.
    cx, cy = 5.0, 5.0
    ground(k, 5, 5, 4.8, EARTH, 6)
    prism(k, cx, cy, [(0, 3.9), (.3, 3.9), (.3, 3.6), (1.6, 3.3), (1.6, 3.4), (1.85, 3.4), (1.95, 3.15)], CONC_DARK)
    prism(k, cx, cy, [(1.1, 3.415 + .03), (1.58, 3.305 + .03)], TEAM)
    a0, a1 = 3.6 * cos(pi / 8), 3.3 * cos(pi / 8)
    with wall_face(k, cx, cy, 0, a0, .3, a1, 1.6):
        placard(k, -.6, .25, 1.1, .6); k.box(0, .35, .1, .05, .1, .8, RUST)
    with wall_face(k, cx, cy, 45, a0, .3, a1, 1.6):
        k.box(.0, -.5, .05, .08, 1.0, .85, rgb(70, 78, 62)); k.box(.08, -.42, .3, .02, .84, .1, DARK)  # inspection door
        k.box(0, .8, .2, .05, .1, .7, RUST)
    with wall_face(k, cx, cy, 90, a0, .3, a1, 1.6): placard(k, .5, .25, 1.1, .6)
    zt = 1.95
    k.cyl(cx, cy, zt, 2.95, .1, rgb(52, 50, 48, 'rubber'), n=24)  # insulating deck
    # Primary: six turns of copper tube on porcelain combs, rising outwards.
    helix(k, cx, cy, zt + .32, zt + .95, 1.15, 2.45, 6, .085, COPPER, seg=18)
    for i in range(6):
        a = i * pi / 3 + .3
        with k.at(cx, cy, zt + .1, yaw=degrees(a)): k.extrude([(1.0, 0), (2.6, 0), (2.6, .85), (2.45, .85), (1.0, .2)], 'xz', -.05, .05, CERAMIC)
    # Strike rail on stand-off insulators, earthed by copper straps down the base.
    k.torus(cx, cy, zt + 1.55, 2.78, .06, GALV, n=28, k=6)
    for i in range(8):
        a = i * pi / 4 + pi / 8
        px, py = cx + 2.78 * cos(a), cy + 2.78 * sin(a)
        insulator(k, px, py, zt + .1, 1.4, .13, CERAMIC, n=5)
    for a in (22.5, 67.5):
        ca, sa = cos(radians(a)), sin(radians(a))
        k.pipe([(cx + 2.78 * ca, cy + 2.78 * sa, zt + 1.5), (cx + 3.3 * ca, cy + 3.3 * sa, zt), (cx + 3.75 * ca, cy + 3.75 * sa, .35), (cx + 4.2 * ca, cy + 4.2 * sa, .1)], .045, COPPER, n=5)
        k.tube((cx + 4.2 * ca, cy + 4.2 * sa, 0), (cx + 4.2 * ca, cy + 4.2 * sa, .6), .04, COPPER, n=5)
    # Secondary: porcelain foot, the winding, corona ring, top insulator.
    k.cyl(cx, cy, zt + .1, .95, .3, DARK, n=16)
    for i in range(5): k.rev((cx, cy, zt + .4 + i * .3), 'z', [(0, .62), (.1, .98), (.2, .98), (.3, .62)], CERAMIC, n=16)
    zw0, zw1 = zt + 1.9, 10.1
    prof = []
    nw = int((zw1 - zw0) / .16)
    for i in range(nw + 1): prof.append((i * (zw1 - zw0) / nw, .66 if i % 2 else .7))
    k.rev((cx, cy, zw0), 'z', prof, COPPER, n=16)
    for zz in (zw0 - .06, zw1): k.rev((cx, cy, zz), 'z', [(0, .78), (.1, .78)], DARK, n=16)
    for i in range(3): k.rev((cx, cy, zw1 + .1 + i * .26), 'z', [(0, .5), (.08, .86), (.18, .86), (.26, .5)], CERAMIC, n=16)
    zc = zw1 + 1.5
    k.cyl(cx, cy, zw1 + .88, .42, 1.6, COPPER, n=12)
    k.torus(cx, cy, zc, 1.95, .66, COPPER, n=32, k=12)
    k.cyl(cx, cy, zc - .1, 1.95, .2, tint(COPPER, .8), n=24)  # spun web of the toroid
    k.torus(cx, cy, zc + 1.0, 1.0, .34, COPPER, n=24, k=10)
    k.rev((cx, cy, zc + 1.0), 'z', [(0, .5), (.5, .22), (1.2, .1), (1.9, .06)], COPPER, n=10)
    k.ball(cx, cy, zc + 2.98, .2, TESLA, n=12, k=6)
    for f in (.34, .67): k.torus(cx, cy, zw0 + (zw1 - zw0) * f, .84, .05, GALV, n=20, k=6)  # grading rings
    # Feed: bus bars from a switch cabinet through a wall bushing, and an earth mat stake.
    with k.at(8.5, 2.3, 0, yaw=0):
        k.box(-.6, -.8, 0, 1.2, 1.6, .15, CONC_DARK); k.box(-.5, -.7, .15, 1.0, 1.4, 1.7, rgb(78, 86, 70))
        k.box(.5, -.6, .4, .03, 1.2, 1.2, tint(SOV_GREEN, .8)); k.box(.53, -.2, 1.25, .02, .4, .24, HAZ)
        for yy in (-.35, .35): insulator(k, 0, yy, 1.85, .7, .13, CERAMIC, n=4)
        k.box(.54, .3, .7, .03, .12, .12, RED_LAMP)
    for yy in (1.95, 2.65): k.pipe([(8.5, yy, 2.6), (7.4, yy + .6, 2.6), (6.6, yy + 1.3, zt + .5)], .045, COPPER, n=5)
    k.box(1.7, 8.1, 0, .1, .1, 1.6, IRON); k.box(1.35, 8.2, 1.0, .8, .03, .55, HAZ)
    barrels(k, 1.4, 6.6, .05, 2, c=rgb(84, 92, 70))

# --- Grand cannon ---------------------------------------------------------------------------------
def shell(k, L=1.55, r=.19):
    """Heavy shell along +x from its base."""
    k.rev((0, 0, 0), 'x', [(0, r * .9), (.05, r), (L * .55, r), (L * .8, r * .72), (L, .03)], rgb(84, 90, 78), n=10)
    k.rev((.12, 0, 0), 'x', [(0, r + .012), (.1, r + .012)], COPPER, n=10)
    k.rev((L * .56, 0, 0), 'x', [(0, r + .008), (.12, r * .98)], YELLOW, n=10)

def grand_cannon(k, w, d):
    # Allied coastal-artillery battery: one heavy gun in an armoured turret on a concrete
    # barbette, with the shell hoist, the earth-covered magazine and its munition railway, a
    # battery command post with a coincidence rangefinder, and a blast apron under the muzzle.
    bx, by = 7.0, 10.0
    OUTB, SLAB = rgb(142, 140, 132, 'concrete'), rgb(116, 114, 108, 'concrete')
    ground(k, 10, 10, 9.7, GRAVEL_DARK, 12, n=32, h=.1)
    # Blast apron.
    k.extrude([(10.0, 1.6), (19.7, 1.2), (19.7, 10.6), (12.6, 13.6)], 'xy', 0, .16, rgb(86, 84, 80, 'concrete'))
    for i in range(4): k.box(13.0 + i * 1.7, 1.8 - i * .05, .16, .07, 11.0 - i * 1.0, .012, rgb(60, 60, 58, 'concrete'))
    k.plate([(13.6, 5.0, .175), (19.4, 2.4, .175), (19.5, 5.6, .175), (14.0, 8.2, .175)], rgb(54, 52, 50, 'concrete'))  # scorched
    # Barbette.
    k.rev((bx, by, 0), 'z', [(0, 6.2), (.5, 6.2), (2.6, 5.4), (2.9, 5.4)], CONCRETE, n=20, smooth=False)
    k.rev((bx, by, .5), 'z', [(0, 6.23), (.45, 6.06)], CONC_DARK, n=20, smooth=False, caps=False)
    zb = 2.9
    k.cyl(bx, by, zb, 4.7, .28, DARK, n=28)
    pts = [(bx + 5.25 * cos(radians(a)), by + 5.25 * sin(radians(a)), zb) for a in range(-150, 151, 20)]
    k.rail(pts, GALV, h=1.0, t=.05, posts=2.0)
    with k.at(bx, by, 0, yaw=22):
        k.stairs(8.4, -.6, 0, zb, -3.1, 1.2, GALV, axis='x')
    c = NATO_GREY
    with k.at(bx, by, zb + .28, yaw=-24):
        bot = [(4.4, 1.9), (2.6, 3.5), (-4.2, 3.3), (-4.2, -3.3), (2.6, -3.5), (4.4, -1.9)]
        top = [(3.2, 1.7), (2.0, 3.0), (-4.0, 2.9), (-4.0, -2.9), (2.0, -3.0), (3.2, -1.7)]
        H = 2.9
        k.loft([[(a, b, 0) for a, b in bot], [(a, b, H) for a, b in top]], c, smooth=False)
        k.loft([[(a * 1.01, b * 1.01, 0) for a, b in bot], [(a * 1.012, b * 1.012, .3) for a, b in bot]], tint(c, .82), smooth=False)
        # Disruptive camouflage on the roof and the plates the camera sees.
        green, dark = NATO_GREEN, rgb(88, 92, 90)
        roof = [(-4.0, -2.9, H), (3.2, -2.9, H), (3.2, 2.9, H), (-4.0, 2.9, H)]
        for cu, cv, ru, rv, cc, sd in [(.82, .35, .14, .2, green, 1), (.62, .82, .16, .14, dark, 2), (.12, .16, .12, .16, green, 3), (.08, .78, .1, .16, dark, 4), (.45, .1, .12, .1, dark, 5)]:
            decal(k, roof, blob(cu, cv, ru, rv, sd), cc, .02)
        side = [(2.6, 3.5, 0), (-4.2, 3.3, 0), (-4.0, 2.9, H), (2.0, 3.0, H)]
        for cu, cv, ru, rv, cc, sd in [(.25, .5, .16, .42, green, 6), (.62, .45, .12, .4, dark, 7), (.9, .55, .09, .4, green, 8)]:
            decal(k, side, blob(cu, cv, ru, rv, sd), cc)
        cheek = [(4.4, 1.9, 0), (2.6, 3.5, 0), (2.0, 3.0, H), (3.2, 1.7, H)]
        decal(k, cheek, blob(.5, .5, .4, .4, 9), green)
        face_ = [(4.4, -1.9, 0), (4.4, 1.9, 0), (3.2, 1.7, H), (3.2, -1.7, H)]
        decal(k, face_, blob(.2, .6, .16, .36, 10), dark)
        # The gun: blast bag, hooped barrel, muzzle swell.
        with k.at(3.35, 0, 1.3, pitch=9):
            k.ball(.55, 0, 0, 1.25, CANVAS, ry=1.2, rz=1.15, n=14, k=6)
            k.rev((.3, 0, 0), 'x', [(0, .98), (1.5, .9), (1.5, .84)], tint(c, .8), n=16)
            k.rev((0, 0, 0), 'x', [(1.6, .82), (3.0, .8), (3.0, .7), (5.4, .64), (5.4, .56), (7.8, .48), (7.8, .42), (10.0, .34), (10.0, .42), (10.4, .42), (10.4, .28)], tint(c, .9), n=16)
            k.rev((10.39, 0, 0), 'x', [(0, .22), (.02, .22)], BLACK, n=12)
            k.rev((3.5, 0, 0), 'x', [(0, .7), (1.2, .665)], green, n=16); k.rev((8.1, 0, 0), 'x', [(0, .425), (1.2, .385)], green, n=16)
            k.rev((5.7, 0, 0), 'x', [(0, .565), (.5, .55)], TEAM, n=16)
        # Rangefinder arms, sighting hoods, hatches, ventilators.
        for s in (-1, 1):
            k.box(-3.0, min(s * 2.7, s * 4.3), 1.65, 1.0, 1.6, .85, c)
            k.box(-3.1, min(s * 4.3, s * 4.75), 1.55, 1.2, .45, 1.05, tint(c, .9))
            k.box(-1.9, min(s * 4.36, s * 4.7), 1.75, .03, .34, .6, GLASS)
            k.cbox(2.2, s * 1.9, H, 1.0, .8, .5, c); k.cbox(2.71, s * 1.9, H + .2, .02, .5, .16, GLASS_DARK)
            vent(k, -3.4, s * 2.2, H, .3, .5, tint(c, .9))
        k.cbox(-1.4, 0, H, 2.5, 3.4, .04, TEAM)
        veh.hatch(k, .9, -1.6, H, .5, c); veh.cupola(k, .8, 1.5, H, c, .6)
        k.box(-4.25, -.6, .3, .06, 1.2, 1.9, tint(c, .8))  # rear door
        for i in range(6): k.box(-.3, 3.42 - i * .012, .4 + i * .42, .5, .12, .06, DARK)  # rungs
        veh.antenna(k, -3.6, 2.5, H, 4.0)
        k.rail([(-3.9, -2.8, H), (-3.9, 2.8, H)], GALV, h=.9, t=.04, posts=1.9)
    # Shell hoist on the barbette's flank, its beam over the railhead.
    hx, hy = 8.5, 15.5
    k.box(hx - 1.3, hy - 1.2, 0, 2.6, 2.4, 4.7, OUTB); k.box(hx - 1.45, hy - 1.35, 4.7, 2.9, 2.7, .3, SLAB)
    k.box(hx - .75, hy + 1.2, .1, 1.5, .08, 2.4, rgb(86, 92, 88)); k.box(hx - .04, hy + 1.28, .1, .08, .03, 2.4, DARK)
    k.box(hx - 1.31, hy + 1.2, 2.5, 2.62, .06, .28, HAZ)
    k.box(hx + 1.3, hy - .5, 2.6, .06, 1.0, .5, GLASS_DARK)
    k.box(hx - .12, hy + .2, 5.0, .24, 2.9, .34, YELLOW); k.box(hx - .3, hy + 2.3, 4.72, .6, .5, .3, DARK)
    k.strut((hx, hy + 2.55, 4.72), (hx, hy + 2.55, 3.1), .04, DARK)
    with k.at(hx, hy + 2.55, 1.5, pitch=90): shell(k)
    vent(k, hx + .6, hy - .5, 5.0, .25, .5)
    # Magazine: an earth-covered concrete vault with steel doors.
    k.ball(3.3, 17.2, 0, 3.0, TURF, ry=2.45, rz=3.1, n=16, k=6, lo=0.0)
    k.box(4.9, 15.2, 0, 1.6, 4.0, 3.2, OUTB); k.box(4.8, 15.05, 3.2, 1.8, 4.3, .3, SLAB)
    for yy in (15.0, 19.1): k.extrude([(6.5, 0), (8.0, 0), (6.5, 2.6)], 'xz', yy, yy + .35, OUTB)
    for i in range(2):
        k.box(6.5, 15.85 + i * 1.36, .08, .09, 1.3, 2.5, rgb(82, 92, 84)); k.box(6.59, 15.95 + i * 1.36, 1.2, .02, 1.1, .12, DARK)
    k.box(6.5, 15.8, 2.6, .1, 2.8, .3, HAZ); k.box(6.52, 17.0, 3.0, .12, .4, .14, LAMP)
    for px, py in [(2.2, 16.4), (3.6, 18.2)]: vent(k, px, py, 2.75, .22, .7, IRON)
    rnd = random.Random(21)
    for i in range(14):
        a, r = rnd.uniform(0, 2 * pi), rnd.uniform(.3, 2.3)
        px, py = 3.3 + r * cos(a), 17.2 + r * sin(a) * .8
        zz = 3.1 * sqrt(max(.05, 1 - ((px - 3.3) / 3.0) ** 2 - ((py - 17.2) / 2.45) ** 2))
        k.ball(px, py, zz, rnd.uniform(.3, .5), LEAF if i % 2 else LEAF2, rz=.22, n=7, k=3)
    # Munition railway from the magazine doors to the hoist, a shell trolley.
    for yy in (16.92, 17.52): k.box(6.6, yy, .14, 6.6, .08, .1, IRON)
    for i in range(12): k.box(6.8 + i * .56, 16.7, .06, .2, 1.05, .1, TIMBER)
    k.box(13.15, 16.8, .1, .2, .85, .5, HAZ)
    with k.at(11.0, 17.22, .24):
        k.box(-.95, -.5, .22, 1.9, 1.0, .12, rgb(86, 92, 88))
        for xx in (-.6, .6):
            for s in (-1, 1): k.rev((xx, s * .3 - .04, .16), 'y', [(0, .17), (.08, .17)], IRON, n=10)
        for yy in (-.24, .24):
            with k.at(-.8, yy, .34 + .19): shell(k)
    # Battery command post with a coincidence rangefinder, part under a net.
    px, py = 14.6, 15.4
    k.box(px, py, 0, 4.6, 3.8, 2.3, OUTB); k.box(px - .2, py - .2, 2.3, 5.0, 4.2, .35, SLAB)
    k.box(px + 4.6, py + .5, 1.45, .05, 2.8, .4, BLACK); k.box(px + 1.4, py + 3.8, 1.45, 2.8, .05, .4, BLACK)
    k.box(px + .3, py + 3.8, .0, .9, .07, 1.9, rgb(86, 92, 88))
    k.cyl(px + 3.2, py + 1.9, 2.65, .22, .8, tint(c, .9), n=8)
    k.tube((px + 3.2, py + .5, 3.55), (px + 3.2, py + 3.3, 3.55), .15, tint(c, .9), n=10)
    for yy in (py + .5, py + 3.3): k.box(px + 3.2, yy - .14, 3.4, .22, .28, .3, DARK); k.box(px + 3.42, yy - .09, 3.46, .02, .18, .18, GLASS)
    def nz(x, y): return 2.75 + .9 * max(0, 1 - ((x - px - 1.0) / 1.5) ** 2 - ((y - py - 1.9) / 2.2) ** 2) + .05 * sin(x * 5) * cos(y * 4)
    camo_net(k, px - .3, py - .3, px + 2.1, py + 4.1, nz, 8, 12, seed=11)
    for qx, qy in [(px - .25, py - .25), (px - .25, py + 4.05), (px + 2.05, py - .25), (px + 2.05, py + 4.05)]: k.strut((qx, qy, 2.65), (qx, qy, 2.8), .06, TIMBER)
    k.tube((px + 1.0, py + 1.9, 2.65), (px + 1.0, py + 1.9, 3.7), .05, TIMBER, n=5)
    veh.antenna(k, px + 4.2, py + .5, 2.65, 3.2, False)
    # Flag, floodlight, stores.
    flagpole(k, 1.0, 12.4, 0, 8.0, w=2.4)
    floodlight(k, 18.9, 1.4, 0, 7.0, yaw=135)
    crate(k, 9.4, 18.6, .1, 1.0, c=OLIVE); crate(k, 10.6, 18.7, .1, .9, c=OLIVE, yaw=12); barrels(k, 12.6, 14.4, .16, 3, c=rgb(84, 92, 70))
    sandbags(k, [(13.4, 19.3, .1), (13.4, 17.9, .1)], 3)

# --- Fortress wall --------------------------------------------------------------------------------
def fortress_walls(k, w, d):
    # One cell of a reinforced-concrete rampart. Its plinth, corner piers and deck reach the
    # cell border exactly, so segments laid along either axis join into one wall; the battered
    # panels, buttresses, corner merlons and the wire along the parapets repeat with them.
    W = 10.0
    c = rgb(122, 120, 114, 'concrete'); pier = rgb(108, 106, 101, 'concrete')
    k.box(0, 0, 0, W, W, .7, rgb(82, 80, 77, 'concrete'))
    sq = lambda i, z: [(i, i, z), (W - i, i, z), (W - i, W - i, z), (i, W - i, z)]
    k.loft([sq(.25, .7), sq(.95, 4.0)], c, smooth=False)
    k.box(0, 0, 4.0, W, W, .5, c, top=rgb(88, 88, 85, 'concrete'))  # corbelled deck
    P = 1.5
    for px in (0, W - P):
        for py in (0, W - P):
            k.box(px, py, 0, P, P, 4.5, pier)
            k.loft([[(px, py, 4.5), (px + P, py, 4.5), (px + P, py + P, 4.5), (px, py + P, 4.5)],
                    [(px, py, 5.75), (px + P, py, 5.75), (px + P, py + P, 5.75), (px, py + P, 5.75)],
                    [(px + .2, py + .2, 6.0), (px + P - .2, py + .2, 6.0), (px + P - .2, py + P - .2, 6.0), (px + .2, py + P - .2, 6.0)]], pier, smooth=False)
    # The four sides are alike; build each in its own wall frame.
    for side, ox, oy in [('y+', 0, W), ('x+', W, W), ('y-', W, 0), ('x-', 0, 0)]:
        vis = side in ('y+', 'x+')
        with face(k, side, ox, oy, 0):
            # Buttress and corbels (the wall's outside is at y = 0, the block lies towards -y).
            k.extrude([(0, .7), (0, 1.1), (-.9, 3.9), (-.95, 3.9), (-.95, .7)], 'yz', 4.3, 5.7, pier)
            for x in (2.2, 3.3, 6.7, 7.8): k.extrude([(0, 4.0), (-.95, 4.0), (-.95, 3.3), (0, 3.85)], 'yz', x - .15, x + .15, pier)
            # Low parapet between the piers, carrying concertina wire on raked pickets.
            k.box(P, -.5, 4.5, W - 2 * P, .5, .5, c); k.box(P, -.56, 5.0, W - 2 * P, .62, .08, pier)
            wire_coil(k, (P + .1, -.36, 5.08), (W - P - .1, -.36, 5.08), .32, .5, IRON, .055)
            for x in (2.2, 3.6, 5.0, 6.4, 7.8):
                k.strut((x, -.62, 5.08), (x, -.1, 5.9), .07, IRON); k.strut((x, -.1, 5.08), (x, -.62, 5.9), .07, IRON)
            if vis:
                k.box(4.6, 0, 4.1, .1, .02, .5, RUST); k.box(5.3, 0, 4.2, .08, .02, .4, RUST)
                # Steel wall plates with tie-rod heads on the panels, rust below them, damp at the foot.
                tilt = degrees(atan2(.7, 3.3))
                for x0 in (1.9, 6.1):
                    with k.at(x0, -.25, .7, roll=tilt):
                        k.box(.2, -.05, 1.5, 1.6, .08, .9, SOV_STEEL)
                        for bx_ in (.4, 1.0, 1.6):
                            for bz in (1.7, 2.2): k.box(bx_ - .06, .03, bz - .06, .12, .04, .12, IRON)
                        k.box(.5, -.01, .5, .12, .03, 1.0, RUST); k.box(1.3, -.01, .8, .1, .03, .7, RUST)
                        k.box(0, -.01, 0, 2.0, .03, .35, tint(c, .72))
                k.box(.3, 0, 2.7, .9, .03, .6, TEAM)
    # Deck: an armoured hatch with the recognition panel, drainage joints.
    z = 4.5
    k.box(4.1, 4.1, z, 1.8, 1.8, .12, SOV_STEEL); k.box(4.25, 4.25, z + .12, 1.5, 1.5, .03, TEAM)
    k.torus(5.0, 5.0, z + .2, .22, .04, DARK, n=10, k=4)
    for x0, y0, x1, y1 in [(.2, 5.0, 2.0, 5.0), (8.0, 5.0, 9.8, 5.0), (5.0, .2, 5.0, 2.0), (5.0, 8.0, 5.0, 9.8)]:
        k.box(min(x0, x1) - .04, min(y0, y1) - .04, z, abs(x1 - x0) + .08, abs(y1 - y0) + .08, .012, rgb(58, 58, 56, 'concrete'))  # deck joints

def futa(k, w, d):
    # Futa, the game's own Super Me defence (docs/DECISIONS.md R220): a giant terry bath towel,
    # 12 m wide, hung over the heavy steel rail of a towel stand on a poured plinth. Woven stripe
    # bands near both hems carry the owner's colour as dyed bands do on real towels; the light it
    # strikes with gathers in the cut-crystal finials on the stand's posts.
    terry = rgb(224, 216, 196, 'cloth'); border = rgb(200, 190, 168, 'cloth')
    shade = rgb(170, 162, 144, 'cloth'); crystal = C((206, 238, 255), 'emit')
    cx, cy = 10.0, 10.0
    ground(k, cx, cy, 9.6, GRAVEL_DARK, 11)
    # The plinth: a low poured slab with a chamfered step.
    k.stack(cx, cy, [(0, 17.0, 17.0, 1.2), (.45, 17.0, 17.0, 1.2), (.45, 15.6, 15.6, 1.0),
                     (.8, 15.6, 15.6, 1.0)], CONCRETE, smooth=False)
    zbar, r_bar, y0, y1 = 13.0, .38, 2.6, 17.4
    # The stand: a splayed pair of legs at each end, a footing under each, the rail between.
    for y in (y0, y1):
        for dx in (-2.6, 2.6):
            k.cbox(cx + dx, y, .8, 1.3, 1.3, .5, CONC_DARK)
            k.tube((cx + dx, y, 1.3), (cx, y, zbar), .3, STEEL, n=12)
        k.tube((cx - 1.6, y, 5.0), (cx + 1.6, y, 5.0), .16, STEEL, n=8)
        k.ball(cx, y, zbar, .62, GALV, n=14, k=7)
        with k.at(cx, y, zbar + .55):
            k.ball(0, 0, .75, .55, crystal, rz=1.0, n=6, k=3)       # cut-crystal finial
            k.cyl(0, 0, 0, .3, .25, GALV, n=10)
    k.rev((cx, y0, zbar), 'y', [(0, r_bar), (y1 - y0, r_bar)], GALV, n=14)
    # The towel: a cloth sheet hanging from the rail down both sides, in soft vertical folds.
    ta, tb = 4.0, 16.0                        # its edges along the rail
    T = .16                                   # thickness of heavy terry
    def drape(side, length, nu=18, nv=12):
        def point(u, v, off=0.0):
            y = ta + (tb - ta) * u
            fold = .32 * sin(u * 7 * pi + .6) * v ** .8      # folds open towards the hem
            lean = .55 * v ** 1.6                              # the hem stands off the stand
            x = cx + side * (r_bar + .04 + lean + fold + off)
            z = zbar - v * length
            return (x, y, z)
        def colour(v0, v1):
            m = (v0 + v1) / 2 * length
            if length - 2.4 < m < length - 1.6: return TEAM     # the dyed stripe band
            if length - 1.2 < m < length - .9: return TEAM
            if m > length - .5 or length - 1.6 < m < length - 1.2: return border
            return terry
        us = [i / nu for i in range(nu + 1)]; vs = [j / nv for j in range(nv + 1)]
        with k.grp(smooth=True):
            for i in range(nu):
                for j in range(nv):
                    q = [(us[i], vs[j]), (us[i + 1], vs[j]), (us[i + 1], vs[j + 1]), (us[i], vs[j + 1])]
                    outer = [point(u, v, T / 2) for u, v in q]
                    inner = [point(u, v, -T / 2) for u, v in q]
                    k.poly(outer if side > 0 else outer[::-1], colour(vs[j], vs[j + 1]))
                    k.poly(inner[::-1] if side > 0 else inner, shade)
        # Hemmed side edges and the bottom hem.
        for u in (0.0, 1.0):
            for j in range(nv):
                a, b = vs[j], vs[j + 1]
                pts = [point(u, a, T / 2), point(u, b, T / 2), point(u, b, -T / 2), point(u, a, -T / 2)]
                k.plate(pts if (u > 0) == (side > 0) else pts[::-1], border)
        for i in range(nu):
            a, b = us[i], us[i + 1]
            pts = [point(a, 1, T / 2), point(b, 1, T / 2), point(b, 1, -T / 2), point(a, 1, -T / 2)]
            k.plate(pts if side < 0 else pts[::-1], border)
        # Fringe tassels along the hem.
        for i in range(25):
            u = .02 + i * .96 / 24
            x, y, z = point(u, 1)
            k.tube((x, y, z), (x + side * .05, y + .05 * sin(i * 1.9), z - .55), .05, border, n=4, r2=.025)
        return point
    drape(+1, 10.6)                           # the front fall, to just above the plinth
    drape(-1, 8.2)                            # the back fall, shorter
    # Where it folds over the rail: a soft roll of cloth round the bar.
    k.rev((cx, ta, zbar), 'y', [(0, r_bar + .2), (tb - ta, r_bar + .2)], terry, n=16)

MODELS = {f.__name__: f for f in [rocket_tower, pillbox, sentry_gun, patriot, flak_cannon, prism_tower, tesla_coil,
                                  grand_cannon, fortress_walls, futa]}
