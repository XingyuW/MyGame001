"""Architectural parts for the buildings (hd_buildings*.py), in metres.

A building is authored in its footprint's own frame: the footprint is x in 0..10w, y in 0..10d
(one cell is 10 m), z up. The camera looks at the corner (10w, 10d): the walls facing +x and +y
are the ones seen, so doors, windows and signs go there; -x and -y walls are hidden.

Scale references: a storey is 3.3 m, a door 1.0 x 2.1 m, a handrail 1.05 m, a vehicle door
4 x 4.5 m, an ISO container 6.06 x 2.44 x 2.59 m.
"""
from contextlib import contextmanager
from math import sin, cos, pi, radians, degrees, atan2, asin
import random
from hd_kit import *

UB = .1  # cells per metre
STOREY = 3.3

@contextmanager
def face(k, side, x, y, z=0.0):
    """A wall's own frame: +x runs along the wall, +y points out of it, origin at (x, y, z).
    side 'y+' is the wall facing +y (x runs towards +x); 'x+' faces +x (x runs towards -y, so
    give its origin at the wall's high-y end); 'y-' and 'x-' are the hidden walls."""
    yaw = {'y+': 0, 'x+': -90, 'y-': 180, 'x-': 90}[side]
    with k.at(x, y, z, yaw=yaw): yield k

# --- Ground ---------------------------------------------------------------------------------------
def pad(k, w, d, c=CONCRETE, inset=.5, joints=5.0, h=.16):
    """Poured hardstanding under the whole footprint, with expansion joints."""
    W, D = 10 * w, 10 * d
    k.box(inset, inset, 0, W - 2 * inset, D - 2 * inset, h, c)
    x = joints
    while x < W - inset - .5:
        k.box(x - .04, inset + .02, h, .08, D - 2 * inset - .04, .012, tint(c, .62)); x += joints
    y = joints
    while y < D - inset - .5:
        k.box(inset + .02, y - .04, h, W - 2 * inset - .04, .08, .012, tint(c, .62)); y += joints
    return h

def apron(k, x, y, w, d, z, c=ASPHALT):
    """Tarmac surface laid on the pad (vehicle bays, roads)."""
    k.box(x, y, z, w, d, .03, c)

def line(k, x, y, w, d, z, c=WHITE):
    k.box(x, y, z + .03, w, d, .012, c)

def hazard_edge(k, x, y, w, d, z):
    k.box(x, y, z + .03, w, d, .014, HAZ)

# --- Volumes --------------------------------------------------------------------------------------
def windows(k, length, floors=1, z0=1.0, h=1.4, floor_h=STOREY, bay=1.3, margin=.8, glass=GLASS, frame=None):
    """Window strips in a wall's frame (see `face`): a glazed band per floor with mullions."""
    frame = frame or rgb(70, 74, 78)
    L = length - 2 * margin
    if L <= .4: return
    n = max(1, int(round(L / bay)))
    for f in range(floors):
        z = z0 + f * floor_h
        k.box(margin, -.06, z, L, .1, h, glass)
        k.box(margin - .06, -.02, z - .08, L + .12, .1, .08, frame); k.box(margin - .06, -.02, z + h, L + .12, .1, .07, frame)
        for i in range(n + 1): k.box(margin + i * L / n - .035, -.02, z, .07, .09, h, frame)

def punched(k, length, floors=1, z0=1.1, h=1.3, w=.9, floor_h=STOREY, bay=2.2, margin=1.0, glass=GLASS, sill=None):
    """Separate punched windows (masonry buildings): a pane, a sill and a lintel each."""
    sill = sill or CONC_LIGHT
    n = max(1, int((length - 2 * margin) / bay) + 1)
    step = (length - 2 * margin - w) / max(1, n - 1) if n > 1 else 0
    for f in range(floors):
        for i in range(n):
            x = margin + i * step; z = z0 + f * floor_h
            k.box(x, -.08, z, w, .12, h, glass)
            k.box(x - .08, -.02, z - .1, w + .16, .14, .1, sill); k.box(x - .08, -.02, z + h, w + .16, .1, .14, sill)
            k.box(x + w / 2 - .025, -.01, z, .05, .06, h, sill)

def door(k, x, w=1.0, h=2.1, c=None, light=True):
    """Personnel door in a wall's frame, with a step and a lamp over it."""
    c = c or rgb(84, 90, 96)
    k.box(x, -.05, 0, w, .1, h, c); k.box(x - .08, -.03, 0, .08, .1, h + .08, DARK); k.box(x + w, -.03, 0, .08, .1, h + .08, DARK)
    k.box(x - .08, -.03, h, w + .16, .1, .08, DARK); k.box(x - .1, 0, -.02, w + .2, .5, .1, CONC_DARK)
    k.box(x + w * .78, .05, h * .48, .06, .05, .14, CHROME)
    if light: k.box(x + w / 2 - .12, .04, h + .2, .24, .14, .1, LAMP)

def shutter(k, x, w, h, c=None, open_=0.0, frame=None, stripes=True):
    """Roller shutter door in a wall's frame; open_ in 0..1 shows the dark interior below it."""
    c = c or remat(rgb(120, 126, 128), 'corr'); frame = frame or DARK
    hole = h * open_
    if hole > .05: k.box(x, -.3, 0, w, .32, hole, BLACK)
    n = max(2, int((h - hole) / .3))
    for i in range(n):
        z = hole + i * (h - hole) / n
        k.box(x, -.05 + (i % 2) * .02, z, w, .1, (h - hole) / n, tint(c, 1.0 if i % 2 else .9))
    k.box(x - .2, -.06, 0, .2, .16, h + .3, frame); k.box(x + w, -.06, 0, .2, .16, h + .3, frame)
    k.box(x - .2, -.06, h, w + .4, .2, .35, frame)
    if stripes:
        for xx in (x - .2, x + w): k.box(xx, .1, 0, .2, .02, 1.6, HAZ)

def parapet(k, x, y, z, w, d, c, h=.5, t=.25, cap=None):
    """Upstand round a flat roof."""
    cap = cap or tint(c, 1.08)
    for bx, by, bw, bd in [(x, y, w, t), (x, y + d - t, w, t), (x, y, t, d), (x + w - t, y, t, d)]:
        k.box(bx, by, z, bw, bd, h, c); k.box(bx - .04, by - .04, z + h, bw + .08, bd + .08, .07, cap)

def block(k, x, y, w, d, h, wall, roof=ROOF, plinth=None, floors=None, glaze='strip', z=0.0, para=.5,
          band=None, glass=GLASS, margin=.8):
    """A flat-roofed building volume: walls, plinth, parapet, roof finish, and windows on its
    two visible walls. glaze: 'strip', 'punched' or None. band: a fascia colour under the
    parapet (TEAM for the owner's stripe)."""
    k.box(x, y, z, w, d, h, wall, top=roof)
    if plinth: k.box(x - .06, y - .06, z, w + .12, d + .12, .5, plinth)
    if para: parapet(k, x, y, z + h, w, d, wall, para)
    if band:
        k.box(x - .05, y + d - .02, z + h - .55, w + .1, .08, .5, band); k.box(x + w - .02, y - .05, z + h - .55, .08, d + .1, .5, band)
    floors = floors if floors is not None else max(1, int(h / STOREY))
    if glaze:
        fn = windows if glaze == 'strip' else punched
        fh = min(STOREY, h / floors)
        with face(k, 'y+', x, y + d, z): fn(k, w, floors, floor_h=fh, glass=glass, margin=margin)
        with face(k, 'x+', x + w, y + d, z): fn(k, d, floors, floor_h=fh, glass=glass, margin=margin)
    return z + h

def gable(k, x, y, z, w, d, rise, c, ridge='x', over=.3, wall=None):
    """Pitched roof over the rectangle; `ridge` names the axis the ridge runs along. `wall`
    fills the two gable triangles."""
    e = .12
    if ridge == 'x':
        prof = [(y - over, z), (y + d / 2, z + rise), (y + d + over, z), (y + d + over, z + e), (y + d / 2, z + rise + e), (y - over, z + e)]
        k.extrude(prof, 'yz', x - over, x + w + over, c)
        if wall:
            for xx in (x, x + w - .02): k.extrude([(y, z), (y + d, z), (y + d / 2, z + rise)], 'yz', xx, xx + .02, wall)
    else:
        prof = [(x - over, z), (x + w / 2, z + rise), (x + w + over, z), (x + w + over, z + e), (x + w / 2, z + rise + e), (x - over, z + e)]
        k.extrude(prof, 'xz', y - over, y + d + over, c)
        if wall:
            for yy in (y, y + d - .02): k.extrude([(x, z), (x + w, z), (x + w / 2, z + rise)], 'xz', yy, yy + .02, wall)

def vault(k, x, y, z, w, d, rise, c, ridge='y', n=10, ends=None):
    """Barrel-vault roof (hangars, Quonset huts); `ridge` is the axis it runs along."""
    span = w if ridge == 'y' else d
    prof = [((x if ridge == 'y' else y) + span / 2 - span / 2 * cos(pi * i / n), z + rise * sin(pi * i / n)) for i in range(n + 1)]
    if ridge == 'y': k.extrude(prof, 'xz', y, y + d, c, smooth=True, cap=ends or c)
    else: k.extrude(prof, 'yz', x, x + w, c, smooth=True, cap=ends or c)

def sawtooth(k, x, y, z, w, d, n, rise, c, glass=GLASS):
    """North-light factory roof: n teeth along x, glazed on their steep side."""
    t = w / n
    for i in range(n):
        x0 = x + i * t
        k.extrude([(x0, z), (x0 + t, z), (x0 + t * .22, z + rise)], 'xz', y, y + d, c)
        k.plate([(x0 + .02, y + .3, z + .15), (x0 + .02, y + d - .3, z + .15), (x0 + t * .2, y + d - .3, z + rise * .88), (x0 + t * .2, y + .3, z + rise * .88)], glass)

# --- Industrial plant ---------------------------------------------------------------------------------
def chimney(k, x, y, z, h, r, c=CONCRETE, bands=3, taper=.75, platform=True, lamp=True):
    """Tapering stack with red and white warning bands, a rim, a gallery and an obstruction lamp."""
    rad = lambda f: r * (1 - (1 - taper) * f)
    k.rev((x, y, z), 'z', [(0, r), (h, r * taper)], c, n=16)
    for i in range(bands):
        f1 = 1 - i * .075; f0 = f1 - .075
        k.rev((x, y, z + h * f0), 'z', [(0, rad(f0) + .03), (h * .075, rad(f1) + .03)], SIGNAL_RED if i % 2 == 0 else WHITE, n=16)
    k.rev((x, y, z + h), 'z', [(0, r * taper + .08), (.25, r * taper + .08)], DARK, n=16)
    if platform:
        zp = z + h * .6; rp = rad(.6)
        k.rev((x, y, zp), 'z', [(0, rp + .75), (.08, rp + .75)], GALV, n=16)
        k.rail([(x + (rp + .72) * cos(i * pi / 4), y + (rp + .72) * sin(i * pi / 4), zp + .08) for i in range(9)], GALV, h=1.0, t=.04, posts=9)
        k.ladder(x + rp + .12, y, z, zp, axis='y')
    if lamp: k.ball(x + r * taper, y, z + h + .3, .16, RED_LAMP, n=8, k=4)

def tank(k, x, y, z, r, h, c=WHITE, roof='cone', band=None, stair=True, n=20):
    """Vertical storage tank: shell with ring stiffeners, roof, wind girder, stair and vent."""
    k.cyl(x, y, z, r, h, c, n=n)
    for f in (.03, .5, .97): k.rev((x, y, z + h * f - .06), 'z', [(0, r + .05), (.12, r + .05)], tint(c, .86), n=n)
    if roof == 'cone': k.rev((x, y, z + h), 'z', [(0, r), (r * .22, .15)], tint(c, .95), n=n)
    elif roof == 'dome': k.dome(x, y, z + h, r, tint(c, .97), h=r * .45, n=n)
    else: k.rev((x, y, z + h), 'z', [(0, r), (.06, r)], tint(c, .8), n=n)
    k.cyl(x, y, z + h + (r * .22 if roof == 'cone' else r * .45 if roof == 'dome' else .06), .12, .3, STEEL, n=8)
    if band: k.rev((x, y, z + h * .72), 'z', [(0, r + .03), (h * .12, r + .03)], band, n=n)
    if stair:
        steps = int(h / .25)
        for i in range(steps):
            a = radians(20 + i * 110 / steps)
            with k.at(x + (r + .45) * cos(a), y + (r + .45) * sin(a), z + (i + 1) * h / steps, yaw=20 + i * 110 / steps):
                k.cbox(0, 0, -.04, .8, .28, .04, GALV)
                if i % 3 == 0: k.strut((.38, 0, 0), (.38, 0, 1.0), .035, GALV)

def htank(k, x, y, z, length, r, c=WHITE, axis='x', band=None):
    """Horizontal pressure vessel on two saddles."""
    d = 'x' if axis == 'x' else 'y'
    k.rev((x, y, z + r + .5), d, [(0, r * .35), (r * .3, r * .85), (r * .6, r), (length - r * .6, r), (length - r * .3, r * .85), (length, r * .35)], c, n=14)
    for f in (.22, .78):
        if axis == 'x': k.cbox(x + length * f, y, z, .4, r * 1.6, r * .8 + .5, CONC_DARK)
        else: k.cbox(x, y + length * f, z, r * 1.6, .4, r * .8 + .5, CONC_DARK)
    if band: k.rev((x, y, z + r + .5), d, [(length * .42, r + .02), (length * .58, r + .02)], band, n=14)

def sphere_tank(k, x, y, z, r, c=WHITE, band=None):
    """Spherical gas holder on a ring of legs with an equator walkway."""
    k.ball(x, y, z + r + 1.2, r, c, n=20, k=10)
    for i in range(8):
        a = i * pi / 4
        k.tube((x + r * .98 * cos(a), y + r * .98 * sin(a), z), (x + r * .98 * cos(a), y + r * .98 * sin(a), z + r + 1.2), .12, STEEL, n=6)
        k.strut((x + r * .98 * cos(a), y + r * .98 * sin(a), z + .2), (x + r * .98 * cos(a + pi / 4), y + r * .98 * sin(a + pi / 4), z + r * .9), .08, STEEL)
    if band: k.rev((x, y, z + r + 1.2 - .25), 'z', [(0, r + .01), (.5, r + .01)], band, n=20)
    k.cyl(x, y, z + 2 * r + 1.2, .3, .25, STEEL, n=8)

def silo(k, x, y, z, r, h, c=PRECAST, cone=1.6, legs=True):
    """Raised hopper silo: cylinder over a discharge cone, on legs."""
    k.rev((x, y, z + (cone + 1.5 if legs else 0)), 'z', [(0, r), (h, r)], c, n=16)
    k.rev((x, y, z + (cone + 1.5 if legs else 0) + h), 'z', [(0, r), (r * .3, .3)], tint(c, .92), n=16)
    if legs:
        k.rev((x, y, z + 1.5), 'z', [(0, .35), (cone, r)], tint(c, .9), n=16)
        for i in range(4):
            a = pi / 4 + i * pi / 2
            k.tube((x + r * .9 * cos(a), y + r * .9 * sin(a), z), (x + r * .9 * cos(a), y + r * .9 * sin(a), z + cone + 1.6), .12, STEEL, n=6)

def cooling_tower(k, x, y, z, r, h, c=CONCRETE):
    """Hyperbolic natural-draught cooling tower on its ring of raking columns."""
    prof = [(1.2, r), (h * .25, r * .82), (h * .55, r * .62), (h * .75, r * .58), (h * .92, r * .62), (h, r * .66)]
    k.rev((x, y, z), 'z', prof, c, n=24, caps=False)
    k.rev((x, y, z + h - .1), 'z', [(0, r * .64), (.06, r * .3)], BLACK, n=24)
    for i in range(16):
        a = i * pi / 8
        k.tube((x + (r + .1) * cos(a), y + (r + .1) * sin(a), z), (x + r * .97 * cos(a + pi / 16), y + r * .97 * sin(a + pi / 16), z + 1.3), .1, CONC_DARK, n=4)
        k.tube((x + (r + .1) * cos(a + pi / 8), y + (r + .1) * sin(a + pi / 8), z), (x + r * .97 * cos(a + pi / 16), y + r * .97 * sin(a + pi / 16), z + 1.3), .1, CONC_DARK, n=4)

def pipe_run(k, pts, r=.15, c=STEEL, support=2.5, flanges=True):
    """Pipe along a polyline with flanges and trestle supports down to z = `support` below."""
    k.pipe(pts, r, c, n=8)
    for a, b in zip(pts, pts[1:]):
        if flanges:
            m = [(a[i] + b[i]) / 2 for i in range(3)]
            d = [b[i] - a[i] for i in range(3)]
            if sum(q * q for q in d) > 1: k.rev(m, tuple(d), [(-.04, r * 1.5), (.04, r * 1.5)], tint(c, .8), n=8)

def trestle(k, x, y, z0, z1, w=.8, c=STEEL, axis='y'):
    dx, dy = (0, w / 2) if axis == 'y' else (w / 2, 0)
    for s in (-1, 1): k.strut((x + s * dx, y + s * dy, z0), (x + s * dx, y + s * dy, z1), .1, c)
    k.strut((x - dx, y - dy, z1), (x + dx, y + dy, z1), .1, c)

def insulator(k, x, y, z, h=1.2, r=.16, c=None, n=5):
    """Porcelain post insulator: a stack of sheds."""
    c = c or rgb(150, 100, 70)
    k.cyl(x, y, z, r * .45, h, c, n=8)
    for i in range(n):
        k.rev((x, y, z + (i + .2) * h / n), 'z', [(0, r * .5), (h / n * .25, r), (h / n * .5, r * .5)], c, n=10)

def transformer(k, x, y, z, c=None, s=1.0):
    """Power transformer: tank with radiator fins, conservator and three bushings."""
    c = c or rgb(132, 140, 136)
    with k.at(x, y, z, s=s):
        k.cbox(0, 0, 0, 2.2, 1.4, 1.9, c)
        for i in range(8):
            k.box(-1.0 + i * .26, .72, .3, .08, .4, 1.3, tint(c, .85)); k.box(-1.0 + i * .26, -1.12, .3, .08, .4, 1.3, tint(c, .85))
        k.tube((-.8, 0, 2.25), (.8, 0, 2.25), .22, c, n=10)
        for i in (-1, 0, 1):
            insulator(k, i * .6, .3, 1.9, .9, .12, n=4); k.ball(i * .6, .3, 2.85, .06, STEEL, n=6, k=3)
        k.cbox(0, 0, -.12, 2.6, 1.8, .12, CONC_DARK)

def ac_unit(k, x, y, z, w=1.6, d=1.0, h=.9, c=GALV):
    k.box(x, y, z + .12, w, d, h, c); k.box(x + .1, y + .1, z, w - .2, d - .2, .12, DARK)
    k.cyl(x + w / 2, y + d / 2, z + h + .12, min(w, d) * .36, .04, DARK, n=12)
    for i in range(5): k.box(x + .1 + i * (w - .2) / 5, y + d, z + .3, .04, .02, h - .35, DARK)

def vent(k, x, y, z, r=.25, h=.7, c=GALV):
    k.cyl(x, y, z, r * .7, h, c, n=10); k.rev((x, y, z + h), 'z', [(0, r * 1.3), (.18, r * .2)], c, n=10)

def roof_plant(k, x, y, z, w, d, seed=1, density=1.0, skylights=True):
    """Scatter believable roof equipment over a flat roof: AC units, vents, ducts, skylights."""
    rnd = random.Random(seed)
    n = max(1, int(w * d / 28 * density))
    for i in range(n):
        px, py = x + rnd.uniform(.6, max(.7, w - 2.4)), y + rnd.uniform(.6, max(.7, d - 1.8))
        t = rnd.random()
        if t < .4: ac_unit(k, px, py, z, rnd.uniform(1.2, 1.9), rnd.uniform(.9, 1.2), rnd.uniform(.7, 1.0))
        elif t < .7: vent(k, px, py, z, rnd.uniform(.2, .3), rnd.uniform(.5, .9))
        elif t < .85 and skylights:
            k.box(px, py, z, 1.4, 1.0, .2, GALV); k.box(px + .1, py + .1, z + .2, 1.2, .8, .04, GLASS)
        else:
            k.box(px, py, z, rnd.uniform(1.5, 3.0), .4, .4, GALV)

def catwalk(k, a, b, w=1.0, c=GALV, legs=True, rails=True):
    """Grated walkway from a to b (horizontal, along x or y) with railings and legs."""
    (x0, y0, z), (x1, y1, _) = a, b
    along_x = abs(x1 - x0) >= abs(y1 - y0)
    if along_x:
        k.box(min(x0, x1), y0 - w / 2, z - .08, abs(x1 - x0), w, .08, c)
        if rails:
            for s in (-1, 1): k.rail([(x0, y0 + s * w / 2, z), (x1, y0 + s * w / 2, z)], c)
        if legs:
            for xx in (x0, x1): trestle(k, xx, y0, 0, z - .08, w, c, 'y')
    else:
        k.box(x0 - w / 2, min(y0, y1), z - .08, w, abs(y1 - y0), .08, c)
        if rails:
            for s in (-1, 1): k.rail([(x0 + s * w / 2, y0, z), (x0 + s * w / 2, y1, z)], c)
        if legs:
            for yy in (y0, y1): trestle(k, x0, yy, 0, z - .08, w, c, 'x')

def conveyor(k, a, b, w=1.2, c=CLAD_GREY, legs=2):
    """Covered belt conveyor from a to b on trestles."""
    import numpy as np
    A, B = np.array(a, float), np.array(b, float)
    d = B - A; L = float(np.linalg.norm(d))
    yaw = degrees(atan2(d[1], d[0])); pitch = degrees(asin(d[2] / L))
    with k.at(a[0], a[1], a[2], yaw=yaw, pitch=pitch):
        k.truss((0, 0, -.5), (L, 0, -.5), w, .5, STEEL, bays=max(3, int(L / 2.2)), t=.07)
        k.extrude([(-w / 2, 0), (w / 2, 0), (w / 2, .5), (0, .85), (-w / 2, .5)], 'yz', 0, L, c)
    for i in range(1, legs + 1):
        p = A + d * i / (legs + 1)
        if p[2] > 1.5: trestle(k, p[0], p[1], 0, p[2] - .5, w + .3, STEEL, 'y' if abs(d[0]) >= abs(d[1]) else 'x')

# --- Yard furniture ---------------------------------------------------------------------------------
def container(k, x, y, z, c, yaw=0.0, L=6.06):
    with k.at(x, y, z, yaw=yaw):
        k.box(0, 0, 0, L, 2.44, 2.59, remat(c, 'corr'))
        k.box(L, .1, .15, .05, 1.1, 2.3, tint(c, .85)); k.box(L, 1.24, .15, .05, 1.1, 2.3, tint(c, .85))
        for yy in (.55, 1.0, 1.45, 1.9): k.box(L + .05, yy, .2, .03, .04, 2.2, STEEL)

def crate(k, x, y, z, s=1.0, c=WOOD, yaw=0.0):
    with k.at(x, y, z, yaw=yaw):
        k.box(0, 0, 0, s, s, s * .8, c)
        for xx in (0, s - .06): k.box(xx, -.02, 0, .06, s + .04, s * .8 + .02, tint(c, .8))

def barrels(k, x, y, z, n=3, c=None, rows=1):
    """Rows of 200-litre drums; mixed colours unless one is given."""
    palette = [rgb(60, 84, 110), SIGNAL_RED, rgb(196, 170, 50), OLIVE]
    for j in range(rows):
        for i in range(n):
            col = c or palette[(i + j * 2) % 4]
            k.cyl(x + i * .64, y + j * .64, z, .29, .88, col, n=10)
            for zz in (.29, .58): k.rev((x + i * .64, y + j * .64, z + zz), 'z', [(0, .3), (.03, .3)], tint(col, .8), n=10)

def sandbags(k, pts, layers=3, c=SANDBAG):
    """Sandbag wall along a polyline: staggered courses."""
    rnd = random.Random(11)
    for a, b in zip(pts, pts[1:]):
        dx, dy = b[0] - a[0], b[1] - a[1]
        L = (dx * dx + dy * dy) ** .5; n = max(1, int(L / .62))
        yaw = degrees(atan2(dy, dx))
        for j in range(layers):
            for i in range(n + (j % 2)):
                f = (i + (.0 if j % 2 else .5)) / (n + (j % 2))
                with k.at(a[0] + dx * f, a[1] + dy * f, a[2] + j * .2 + .1, yaw=yaw + rnd.uniform(-6, 6)):
                    k.ball(0, 0, 0, .34, tint(c, rnd.uniform(.88, 1.06)), ry=.2, rz=.13, n=8, k=4)

def fence(k, pts, h=2.2, c=GALV, wire=True):
    """Chain-link security fence: posts, rails, and barbed-wire outriggers."""
    for a, b in zip(pts, pts[1:]):
        dx, dy = b[0] - a[0], b[1] - a[1]
        L = (dx * dx + dy * dy) ** .5; n = max(1, int(round(L / 2.5)))
        for i in range(n + 1):
            px, py = a[0] + dx * i / n, a[1] + dy * i / n
            k.strut((px, py, a[2]), (px, py, a[2] + h), .05, c)
            if wire: k.strut((px, py, a[2] + h), (px + .0, py, a[2] + h + .35), .03, c)
        for zz in ([.1, h * .5, h] + ([h + .3] if wire else [])): k.strut((a[0], a[1], a[2] + zz), (b[0], b[1], a[2] + zz), .03, c)
        for i in range(int(L / .5)):  # mesh, suggested by diagonal wires
            f0, f1 = i * .5 / L, min(1, (i + 1) * .5 / L)
            k.strut((a[0] + dx * f0, a[1] + dy * f0, a[2] + .1), (a[0] + dx * f1, a[1] + dy * f1, a[2] + h), .012, c)
            k.strut((a[0] + dx * f0, a[1] + dy * f0, a[2] + h), (a[0] + dx * f1, a[1] + dy * f1, a[2] + .1), .012, c)

def floodlight(k, x, y, z, h=8.0, yaw=45.0, heads=2):
    k.tube((x, y, z), (x, y, z + h), .09, GALV, n=6, r2=.06)
    with k.at(x, y, z + h, yaw=yaw):
        k.box(-.1, -.7, 0, .12, 1.4, .1, GALV)
        for i in range(heads):
            yy = -.5 + i * 1.0 / max(1, heads - 1) if heads > 1 else 0
            with k.at(.1, yy, -.05, pitch=-35):
                k.cbox(0, 0, -.18, .3, .45, .36, DARK); k.box(.15, -.2, -.15, .02, .4, .3, LAMP)

def flagpole(k, x, y, z, h=9.0, c=TEAM, w=2.4):
    """Flag flying towards +x with a soft wave."""
    k.tube((x, y, z), (x, y, z + h), .06, GALV, n=6, r2=.04); k.ball(x, y, z + h + .06, .09, BRASS, n=6, k=3)
    k.cbox(x, y, z, .5, .5, .25, CONC_DARK)
    n = 5; hh = w * .6
    for i in range(n):
        x0, x1 = x + .05 + w * i / n, x + .05 + w * (i + 1) / n
        y0, y1 = y + .16 * sin(i * 1.3), y + .16 * sin((i + 1) * 1.3)
        z0, z1 = z + h - .15 - .08 * i, z + h - .15 - .08 * (i + 1)
        k.plate([(x0, y0, z0), (x1, y1, z1), (x1, y1, z1 - hh), (x0, y0, z0 - hh)], c)

def dish(k, x, y, z, r, yaw=0.0, pitch=35.0, c=WHITE, mount=True):
    """Parabolic antenna on a pedestal: reflector, ribs, feed horn on struts, counterweights."""
    if mount:
        k.cyl(x, y, z, r * .22, r * .9, tint(c, .9), n=10, r2=r * .14)
        k.cbox(x, y, z + r * .9, r * .5, r * .5, r * .25, GALV)
    zc = z + (r * 1.05 if mount else 0)
    with k.at(x, y, zc, yaw=yaw, pitch=pitch):
        prof = [(0, .02), (r * .02, r * .3), (r * .08, r * .6), (r * .18, r * .85), (r * .3, r)]
        k.rev((0, 0, 0), 'x', prof, c, n=24, caps=False)
        k.rev((-.03, 0, 0), 'x', [(0, .02), (r * .02, r * .3), (r * .08, r * .6), (r * .18, r * .85), (r * .3, r * 1.0)], tint(c, .8), n=24, caps=False)
        k.torus(r * .3, 0, 0, r, .04, tint(c, .85), n=24, k=4, axis='x')
        for i in range(3):
            a = i * 2 * pi / 3 + pi / 6
            k.strut((r * .2, r * .8 * cos(a), r * .8 * sin(a)), (r * .95, 0, 0), .04, STEEL)
        k.rev((r * .85, 0, 0), 'x', [(0, .12), (r * .14, .05)], DARK, n=8)
        k.cbox(-r * .2, 0, -r * .2, r * .3, r * .4, r * .4, GALV)

def radome(k, x, y, z, r, c=WHITE):
    """Geodesic radome on a short drum."""
    k.cyl(x, y, z, r * .7, r * .35, tint(c, .86), n=16)
    k.ball(x, y, z + r * .9, r, c, n=16, k=8, lo=-.55, hi=1.0)
    for i in range(8):
        a = i * pi / 4
        k.strut((x + r * .84 * cos(a), y + r * .84 * sin(a), z + r * .35), (x + r * cos(a + pi / 8) * .98, y + r * sin(a + pi / 8) * .98, z + r * .95), .025, tint(c, .8))

def antenna_mast(k, x, y, z, h, c=None, w=.9, dishes=1, guy=False):
    """Lattice communications mast, red and white, with drum antennas and a lamp."""
    c = c or SIGNAL_RED
    bays = max(4, int(h / 1.6))
    k.mast(x, y, z, h, w, w * .35, c, bays=bays, t=.07)
    k.tube((x, y, z + h), (x, y, z + h + h * .15), .03, STEEL, n=4); k.ball(x, y, z + h + h * .15, .12, RED_LAMP, n=6, k=3)
    for i in range(dishes):
        zz = z + h * (.55 + .18 * i)
        with k.at(x, y, zz, yaw=40 + i * 110): k.rev((w * .3, 0, 0), 'x', [(0, .45), (.3, .45), (.34, .1)], WHITE, n=12)

def stencil_h(k, x, y, z, s=3.0, c=WHITE):
    """Helipad marking: ring and H."""
    k.torus(x, y, z + .02, s, .12, c, n=28, k=4)
    t = s * .16
    for sx in (-1, 1): k.box(x + sx * s * .38 - t / 2, y - s * .5, z + .02, t, s, .012, c)
    k.box(x - s * .38, y - t / 2, z + .02, s * .76, t, .012, c)

def bollards(k, pts, z, c=HAZ):
    for x, y in pts: k.cyl(x, y, z, .11, .9, c, n=8)

def lamp(k, x, y, z, c=LAMP, r=.1):
    k.ball(x, y, z, r, c, n=6, k=3)
