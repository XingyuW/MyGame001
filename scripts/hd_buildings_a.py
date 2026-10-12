"""Allied economy, technology and support installations (house style: see hd_buildings.py).

Each is modelled on the real thing it would be: a contractor's depot under a tower crane, a
mineral processing plant, a research campus, a process unit, a shipbuilding berth, an airbase
headquarters, a field workshop. Footprints are x in 0..10w, y in 0..10d; the camera sees the +x and
+y sides, so yards open to the front and tall plant stands at the back.
"""
from math import sin, cos, pi, radians, degrees
import random
from hd_kit import *
from hd_arch import *
import hd_vehicles as veh

ALU = rgb(176, 180, 178, 'steel')      # aluminium-jacketed insulation
PRIMER = rgb(140, 64, 48)              # red-oxide primed steel
MULLION = rgb(196, 198, 196)
SKYLIGHT = rgb(172, 182, 178)

# --- Shared parts -----------------------------------------------------------------------------------
def sliding(k, x, w, h, open_=.4, c=None, band=None):
    """Pair of sliding doors in a wall's frame (see `face`), parted by open_ of the opening."""
    c = c or CLAD_GREY
    gap = w * open_; leaf = w / 2
    k.box(x + w / 2 - gap / 2, -.3, 0, gap, .32, h, BLACK)
    for s in (-1, 1):
        x0 = x + w / 2 + s * gap / 2 - (leaf if s < 0 else 0)
        k.box(x0, .04, .05, leaf, .14, h, c)
        k.box(x0, .18, .05, leaf, .03, .55, HAZ)
        for f in (0.0, .5, 1.0): k.box(x0 + f * (leaf - .12), .16, .6, .12, .06, h - .6, tint(c, .72))
        k.box(x0, .16, h * .55, leaf, .05, .14, tint(c, .72))
        if band: k.box(x0 + .12, .185, h * .55 + .3, leaf - .24, .03, .8, band)
    k.box(x - gap / 2 - .3, 0, h + .05, w + gap + .6, .32, .32, DARK)
    k.box(x - gap / 2 - .3, 0, 0, w + gap + .6, .28, .05, STEEL)

def curtain(k, length, z0, z1, floor_h=STOREY, bay=1.25, margin=.6, glass=GLASS, frame=None, spandrel=None):
    """Glazed curtain wall in a wall's frame: glass, a spandrel band per floor, mullions."""
    frame = frame or MULLION; spandrel = spandrel or rgb(74, 82, 88)
    L = length - 2 * margin
    k.box(margin, -.05, z0, L, .12, z1 - z0, glass)
    zz = z0
    while zz < z1 - .5:
        k.box(margin, -.02, zz, L, .12, .85, spandrel); k.box(margin, .02, zz + .82, L, .1, .07, frame)
        zz += floor_h
    k.box(margin, -.02, z1 - .3, L, .13, .3, frame)
    n = max(1, int(round(L / bay)))
    for i in range(n + 1): k.box(margin + i * L / n - .045, 0, z0, .09, .14, z1 - z0, frame)

def rooflights(k, x, y, z, w, d, rise, xs, lw=1.0, c=SKYLIGHT, over=.3):
    """Translucent sheets in a gable roof whose ridge runs along x (z is its eaves level)."""
    half = d / 2 + over
    for s in (-1, 1):
        for xx in xs:
            ya, yb = sorted((y + d / 2 + s * half * .14, y + d / 2 + s * half * .86))
            za = z + rise * (1 - abs(ya - y - d / 2) / half) + .15; zb = z + rise * (1 - abs(yb - y - d / 2) / half) + .15
            k.plate([(xx, ya, za), (xx + lw, ya, za), (xx + lw, yb, zb), (xx, yb, zb)], c)

def hollow(k, x, y, z, L, r, c, axis='x', wall=.12):
    """Open-ended pipe section lying on its side."""
    dv = (1, 0, 0) if axis == 'x' else (0, 1, 0)
    k.rev((x, y, z), dv, [(0, r), (L, r)], c, n=12)
    k.rev((x - .02 * dv[0], y - .02 * dv[1], z), dv, [(0, r - wall), (L + .04, r - wall)], BLACK, n=12)

def pipe_stack(k, x, y, z, L, r, n=3, c=CONC_LIGHT, axis='x'):
    """Pyramid of precast pipes."""
    for row in range(n):
        for i in range(n - row):
            off = (i + row * .5) * 2 * r + r; zz = z + r + row * r * 1.73
            if axis == 'x': hollow(k, x, y + off, zz, L, r, c, 'x')
            else: hollow(k, x + off, y, zz, L, r, c, 'y')

def beam_stack(k, x, y, z, L, n=4, layers=3, c=RUST, yaw=0.0, s=.38):
    """Steel sections stacked on timber dunnage."""
    with k.at(x, y, z, yaw=yaw):
        for j in range(layers):
            zz = .1 + j * (s + .1)
            for t in (.12, .5, .88): k.box(L * t - .07, -.05, zz - .1, .14, n * (s + .1) + .02, .1, TIMBER)
            for i in range(n):
                o = i * (s + .1); cc = tint(c, .86 + .1 * ((i * 2 + j) % 3))
                k.box(0, o, zz, L, s, .06, cc); k.box(0, o + s / 2 - .04, zz + .06, L, .08, s - .12, cc); k.box(0, o, zz + s - .06, L, s, .06, cc)

def rebar(k, x, y, z, L, n=5, yaw=0.0):
    """Bundles of reinforcing bar on bearers."""
    with k.at(x, y, z, yaw=yaw):
        for t in (.15, .5, .85): k.box(L * t - .08, -.1, 0, .16, n * .36 + .1, .12, TIMBER)
        for i in range(n):
            k.box(0, i * .36, .12, L, .28, .22, tint(RUST, .78 + .12 * (i % 3)))
            for t in (.2, .8): k.box(L * t, i * .36 - .01, .12, .06, .3, .23, STEEL)

def pallet(k, x, y, z, c, h=.9, yaw=0.0, w=1.2, d=1.0):
    with k.at(x, y, z, yaw=yaw):
        k.box(0, 0, 0, w, d, .14, WOOD); k.box(.04, .04, .14, w - .08, d - .08, h, c)

def cabin(k, x, y, z, L=6.0, W=2.5, H=2.6, c=OFFWHITE, yaw=0.0):
    """Portable site cabin: door and windows in its +y long side, a window in its +x end."""
    with k.at(x, y, z, yaw=yaw):
        k.box(0, 0, .15, L, W, H - .15, remat(c, 'corr'), top=tint(c, .8))
        k.box(.15, .15, 0, L - .3, W - .3, .15, DARK)
        k.box(-.04, -.04, H - .16, L + .08, W + .08, .16, tint(c, .7)); k.box(-.04, -.04, .15, L + .08, W + .08, .12, tint(c, .7))
        k.box(.6, W - .02, .15, .9, .06, 2.0, tint(c, .55))
        for xx in (2.1, 3.9):
            k.box(xx, W - .02, 1.05, 1.3, .06, .95, GLASS); k.box(xx - .05, W, .98, 1.4, .06, .07, tint(c, .6))
        k.box(L - .02, W / 2 - .6, 1.05, .06, 1.2, .95, GLASS)

def jersey(k, x, y, z, L=2.0, c=CONC_LIGHT, yaw=0.0):
    with k.at(x, y, z, yaw=yaw): k.extrude([(-.3, 0), (.3, 0), (.11, .8), (-.11, .8)], 'yz', 0, L, c)

def tyres(k, x, y, z, n=4, R=.38, r=.16):
    for i in range(n): k.torus(x, y, z + r + i * 2 * r * .96, R, r, TYRE, n=14, k=6)

def heap(k, x, y, z, r, h, c, seed=1, lumps=60, base=None, size=.36):
    """Tipped stockpile at its angle of repose: a rough mound studded with lumps."""
    rnd = random.Random(seed)
    k.rev((x, y, z), 'z', [(0, r), (h * .5, r * .5), (h * .82, r * .2), (h * .95, r * .05)], base or tint(c, .7), n=12)
    for i in range(lumps):
        a = rnd.uniform(0, 2 * pi); f = rnd.uniform(0, 1) ** .6
        rr = rnd.uniform(.6, 1.0) * size
        k.ball(x + r * f * cos(a), y + r * f * sin(a), z + h * (1 - f) * .95, rr, tint(c, rnd.uniform(.62, 1.1)), rz=rr * .7, n=6, k=3)

def mooring(k, x, y, z):
    k.cyl(x, y, z, .2, .42, DARK, n=8); k.cyl(x, y, z + .42, .32, .13, DARK, n=8)

def shrub(k, x, y, z, L=2.4, seed=1):
    """Precast planter with clipped shrubs."""
    rnd = random.Random(seed)
    k.box(x, y, z, L, .9, .5, CONC_LIGHT)
    for i in range(int(L / .7)):
        k.ball(x + .4 + i * .7, y + .45, z + .7 + rnd.uniform(-.05, .1), rnd.uniform(.38, .5), LEAF if i % 2 else LEAF2, rz=.42, n=8, k=4)

def pipe_rack(k, x, y, z, L, axis='x', zt=4.5, w=2.0, tiers=((.16, STEEL), (.12, ALU), (.2, STEEL)), bay=4.0, c=None):
    """Steel pipe rack running L metres from (x, y) along `axis`: bents, bracing, pipes."""
    c = c or rgb(120, 126, 122)
    tiers = tiers if isinstance(tiers[0][0], tuple) else (tiers,)
    n = max(1, int(round(L / bay))); top = zt + (len(tiers) - 1) * 1.3
    with k.at(x, y, z, yaw=0 if axis == 'x' else 90):
        for i in range(n + 1):
            px = L * i / n
            for s in (-1, 1): k.box(px - .11, s * w / 2 - .11, 0, .22, .22, top, c)
            for t in range(len(tiers)): k.box(px - .11, -w / 2 - .3, zt + t * 1.3 - .22, .22, w + .6, .22, c)
            if i < n and i % 2 == 0:
                for s in (-1, 1): k.strut((px, s * w / 2, .3), (L * (i + 1) / n, s * w / 2, zt - .3), .1, c)
        for s in (-1, 1): k.box(0, s * w / 2 - .07, zt - .5, L, .14, .16, c)
        for t, pipes in enumerate(tiers):
            m = len(pipes)
            for j, (r, col) in enumerate(pipes):
                yy = -w / 2 + (j + .5) * w / m
                k.tube((-.3, yy, zt + t * 1.3 + r), (L + .3, yy, zt + t * 1.3 + r), r, col, n=8)

# --- Construction yard ------------------------------------------------------------------------------
def tower_crane(k, x, y, z, h, yaw, jib=20.0, cj=6.5, trolley=.6, drop=10.0, c=YELLOW):
    """Hammerhead tower crane: lattice mast on a ballasted cross base, slewing unit, cab, jib with
    trolley and hook, counter-jib with winch and concrete ballast."""
    k.cbox(x, y, z, 5.2, 5.2, .45, CONC_DARK)
    for sx, sy in [(-1, -1), (1, -1), (1, 1), (-1, 1)]:
        k.cbox(x + sx * 1.75, y + sy * 1.75, z + .45, 1.5, 1.5, .9, CONCRETE)
        k.strut((x + sx * 2.1, y + sy * 2.1, z + 1.3), (x + sx * .9, y + sy * .9, z + 4.6), .18, c)
    k.mast(x, y, z + .45, h - .45, 1.8, 1.8, c, bays=int(h / 2.1), t=.19)
    k.ladder(x, y, z + .5, z + h - .2, c=GALV, axis='x')
    with k.at(x, y, z + h, yaw=yaw):
        k.cyl(0, 0, -.1, 1.35, .55, DARK, n=16); k.cbox(0, 0, .45, 2.6, 2.3, .55, c)
        k.box(.7, -2.6, -1.5, 2.0, 1.6, 2.2, WHITE, top=OFFWHITE)                     # cab
        k.box(2.7, -2.5, -.75, .04, 1.4, 1.3, GLASS); k.box(.85, -2.64, -.7, 1.75, .04, 1.2, GLASS)
        k.box(.7, -2.62, -1.5, 2.0, .04, .55, TEAM)
        k.mast(0, 0, 1.0, 5.4, 1.8, .4, c, bays=3, t=.16); k.ball(0, 0, 6.6, .16, RED_LAMP, n=6, k=3)
        k.truss((1.0, 0, 1.0), (jib, 0, 1.0), 1.3, 1.35, c, bays=int(jib / 1.8), t=.14)
        k.box(jib - .1, -.75, .9, .25, 1.5, 1.6, c)
        k.truss((-cj, 0, 1.0), (-1.0, 0, 1.0), 1.7, .5, c, bays=4, t=.13)
        k.box(-cj, -.85, 1.5, cj - 1.0, 1.7, .07, GALV)
        k.rail([(-cj, -.85, 1.57), (-1.2, -.85, 1.57)], GALV); k.rail([(-cj, .85, 1.57), (-1.2, .85, 1.57)], GALV)
        k.box(-3.7, -.7, 1.57, 1.9, 1.4, 1.35, GALV, top=DARK)                         # winch house
        for i in range(3): k.box(-cj + .05 + i * .62, -.8, -.5, .5, 1.6, 2.0, CONCRETE)  # ballast
        k.box(-cj + .1, -.93, 1.75, 2.4, .05, .95, TEAM)                               # owner's board
        for tx in (jib * .42, jib * .82): k.strut((0, 0, 6.3), (tx, 0, 2.35), .08, DARK)
        k.strut((0, 0, 6.3), (-cj + .9, 0, 1.6), .08, DARK)
        tx = jib * trolley
        k.cbox(tx, 0, .62, 1.6, 1.3, .32, DARK)
        for s in (-1, 1): k.strut((tx + s * .45, 0, .62), (tx + s * .12, 0, .7 - drop), .045, DARK)
        k.cbox(tx, 0, .2 - drop, .55, .3, .55, YELLOW); k.cbox(tx, 0, -.1 - drop, .14, .14, .35, DARK)
        # The load: a bundle of beams in a two-leg sling.
        zl = -drop - 2.4
        for s in (-1, 1): k.strut((tx, 0, -.1 - drop), (tx, s * 2.2, zl + .5), .035, DARK)
        for i in range(2):
            k.box(tx - .45 + i * .5, -3.4, zl, .4, 6.8, .07, PRIMER); k.box(tx - .29 + i * .5, -3.4, zl + .07, .08, 6.8, .36, PRIMER)
            k.box(tx - .45 + i * .5, -3.4, zl + .43, .4, 6.8, .07, PRIMER)

def mixer_truck(k, x, y, z, yaw=0.0, c=WHITE):
    """Truck mixer: a 6x6 chassis carrying an inclined drum, charging hopper and chute."""
    with k.at(x, y, z, yaw=yaw):
        veh.truck(k, c, bed=False)
        k.rev((-3.55, 0, 3.15), (1, 0, -.32), [(0, .5), (.9, 1.2), (2.2, 1.25), (3.35, .7)], OFFWHITE, n=14)
        k.rev((-3.55, 0, 3.15), (1, 0, -.32), [(1.3, 1.27), (1.75, 1.28)], ORANGE, n=14)
        k.box(-3.75, -.45, 1.3, .3, .9, 1.5, DARK); k.box(-3.3, -.4, 1.25, .25, .8, .5, DARK)
        k.rev((-3.7, 0, 3.6), 'z', [(0, .25), (.5, .6)], STEEL, n=8)
        k.strut((-3.7, 0, 2.5), (-4.1, .5, 1.5), .3, STEEL)

def excavator(k, x, y, z, yaw=0.0, slew=20.0, c=CAT_YELLOW, s=1.0):
    """Tracked hydraulic excavator, boom raised and bucket at the ground."""
    with k.at(x, y, z, yaw=yaw, s=s):
        for sd in (-1, 1):
            k.extrude([(-2.0, .2), (-1.75, 0), (1.75, 0), (2.0, .2), (2.0, .62), (1.75, .82), (-1.75, .82), (-2.0, .62)], 'xz', sd * 1.2 - .3, sd * 1.2 + .3, TRACK)
        k.cbox(0, 0, .3, 2.2, 2.0, .45, IRON); k.cyl(0, 0, .75, .75, .22, DARK, n=12)
        with k.at(0, 0, .97, yaw=slew):
            k.box(-2.3, -1.3, 0, 3.5, 2.6, 1.15, c); k.box(-2.65, -1.3, .05, .55, 2.6, 1.25, IRON)
            k.box(-1.9, -1.0, 1.15, 1.4, 1.5, .25, DARK)
            k.box(-.2, .25, 1.15, 1.45, 1.0, 1.45, c, top=tint(c, .8))
            k.box(1.24, .32, 1.35, .04, .86, 1.1, GLASS); k.box(-.1, 1.24, 1.5, 1.25, .03, .95, GLASS)
            ya, yb = -.55, -.1
            k.extrude([(.7, .7), (1.4, .65), (4.5, 3.95), (4.0, 4.45)], 'xz', ya, yb, c)
            k.extrude([(4.0, 4.45), (4.5, 3.95), (6.45, 3.3), (6.35, 3.75)], 'xz', ya, yb, c)
            k.extrude([(6.1, 3.85), (6.65, 3.6), (5.75, .95), (5.35, 1.05)], 'xz', ya + .05, yb - .05, c)
            k.extrude([(5.8, 1.1), (5.25, 1.2), (4.55, .65), (4.85, .1), (5.55, .2)], 'xz', ya - .25, yb + .25, IRON)
            k.tube((1.7, -.32, 1.0), (3.3, -.32, 2.9), .1, CHROME, n=6); k.tube((4.5, -.32, 4.55), (6.2, -.32, 3.95), .09, CHROME, n=6)

def construction_yard(k, w, d):
    # Construction depot: a portal-frame fabrication shed with sliding doors, a hammerhead tower
    # crane, a cement batching plant, site cabins and a laydown yard of steel, pipe and block.
    z = pad(k, w, d)
    apron(k, 1.0, 14.4, 28.0, 14.6, z, GRAVEL)
    # Fabrication shed: cladding on a precast plinth, the portal columns standing proud of it.
    sx, sy, sw, sd, eh = 1.5, 1.5, 16.0, 12.0, 8.0
    k.box(sx, sy, z, sw, sd, eh, CLAD_SAND); k.box(sx - .06, sy - .06, z, sw + .12, sd + .12, 1.2, PRECAST)
    gable(k, sx, sy, z + eh, sw, sd, 2.2, CLAD_DARK, ridge='x', wall=CLAD_SAND)
    rooflights(k, sx, sy, z + eh, sw, sd, 2.2, [sx + 1.5 + i * 3.4 for i in range(5)])
    k.box(sx + 1.0, sy + sd / 2 - .6, z + eh + 2.1, sw - 2.0, 1.2, .45, GALV); k.box(sx + .9, sy + sd / 2 - .8, z + eh + 2.55, sw - 1.8, 1.6, .1, CLAD_DARK)
    for i in range(5): k.box(sx + i * 4.0 - .15, sy + sd, z, .3, .24, eh, IRON)
    for j in range(4): k.box(sx + sw, sy + j * 4.0 - .15, z, .24, .3, eh, IRON)
    k.box(sx - .05, sy + sd - .02, z + eh - .95, sw + .1, .08, .6, TEAM); k.box(sx + sw - .02, sy - .05, z + eh - .95, .08, sd + .1, .6, TEAM)
    k.box(sx - .3, sy + sd + .24, z + eh - .2, sw + .6, .22, .2, GALV)
    for xx in (sx + .6, sx + sw - .7): k.box(xx, sy + sd + .26, z, .13, .13, eh - .2, GALV)
    with face(k, 'y+', sx, sy + sd, z):
        sliding(k, 2.6, 9.0, 6.2, .42, band=TEAM); door(k, 13.9)
    with face(k, 'x+', sx + sw, sy + sd, z):
        windows(k, sd, 1, z0=4.4, h=1.5, margin=1.5, bay=1.5); door(k, 1.6)
        k.box(sd / 2 - .9, -.02, 8.25, 1.8, .1, 1.0, DARK)
    # Tower crane, lifting a bundle of beams out of the yard.
    tower_crane(k, 22.5, 6.2, z, 23.0, 132.0, jib=20.5, cj=6.5, trolley=.62, drop=10.0)
    # Batching plant: cement silos, mixer tower over the loading lane, a truck mixer under it.
    for yy in (11.2, 14.8):
        silo(k, 27.2, yy, z, 1.45, 6.5, OFFWHITE, cone=1.8)
        k.pipe([(25.72, yy, z + 1.0), (25.72, yy, z + 9.9), (26.6, yy, z + 10.3)], .09, STEEL)
        k.tube((26.8, yy, z + 1.8), (25.1, 13.0 + (yy - 13.0) * .5, z + 6.4), .2, STEEL, n=8)
    k.box(26.7, 11.2, z + 9.8, 1.0, 3.6, .08, GALV); k.rail([(27.7, 11.4, z + 9.88), (27.7, 14.6, z + 9.88)], GALV)
    k.ladder(27.2, 16.32, z, z + 9.8, axis='x')
    for xx in (21.5, 25.1):
        for yy in (10.9, 15.1): k.box(xx - .13, yy - .13, z, .26, .26, 8.1, STEEL)
    k.box(21.3, 10.7, z + 4.7, 4.0, 4.6, 3.4, CLAD_GREY, top=CLAD_DARK)
    k.rev((23.3, 12.9, z + 3.8), 'z', [(0, .3), (.9, 1.0)], GALV, n=10)
    k.box(21.9, 15.3, z + 6.2, 2.8, .05, 1.0, TEAM)
    mixer_truck(k, 23.3, 16.2, z, yaw=90)
    # Site cabins, stacked, with a stair to the upper one.
    cabin(k, 22.6, 25.4, z); cabin(k, 22.6, 25.4, z + 2.6, c=rgb(150, 156, 150))
    k.box(22.6, 27.9, z + 2.52, 6.0, 1.0, .08, GALV); k.rail([(22.6, 28.9, z + 2.6), (28.6, 28.9, z + 2.6), (28.6, 27.9, z + 2.6)], GALV)
    k.stairs(18.9, 27.95, z, z + 2.6, 3.7, .9, axis='x')
    k.box(21.2, 25.6, z, 1.1, 1.1, 2.3, rgb(70, 118, 92)); k.box(21.15, 25.55, z + 2.3, 1.2, 1.2, .1, WHITE)
    # Laydown yard.
    beam_stack(k, 2.2, 15.3, z, 9.0, n=5, layers=3)
    beam_stack(k, 2.2, 18.3, z, 7.0, n=4, layers=2, c=PRIMER)
    rebar(k, 10.4, 18.4, z, 8.0, n=5)
    pipe_stack(k, 2.2, 21.4, z, 2.4, .62, 3); pipe_stack(k, 4.9, 21.4, z, 2.4, .62, 3)
    for i in range(3):
        for j in range(2):
            pallet(k, 8.6 + i * 1.5, 21.3 + j * 1.3, z, BRICK if (i + j) % 2 else CONC_LIGHT, h=1.0)
    pallet(k, 9.3, 21.9, z + 1.14, CONC_LIGHT, h=.9)
    for i in range(4): k.box(14.0 + i * .26, 21.0, z + .1, .16, 2.6, 2.4 - i * .0, PRECAST)   # precast panels in a rack
    k.box(13.8, 21.0, z, 1.4, .2, .1, TIMBER); k.box(13.8, 23.4, z, 1.4, .2, .1, TIMBER)
    k.strut((15.1, 22.3, z), (14.95, 22.3, z + 2.3), .1, STEEL)
    k.box(16.4, 21.2, z, 3.4, 1.7, .9, rgb(196, 150, 40)); k.box(16.5, 21.3, z + .9, 3.2, 1.5, .03, DARK)   # skip
    heap(k, 6.2, 27.0, z, 1.9, 1.3, rgb(150, 128, 96, 'concrete'), seed=5, lumps=40, size=.3)
    # Aggregate bays for the batching plant.
    k.box(24.7, 20.2, z, 4.9, .3, 1.5, CONCRETE)
    for xx in (24.7, 27.0): k.box(xx, 20.5, z, .3, 3.6, 1.5, CONCRETE)
    SAND, STONE = rgb(196, 176, 130, 'concrete'), rgb(120, 118, 112, 'concrete')
    heap(k, 26.0, 21.6, z, 1.0, 1.5, SAND, seed=8, lumps=36, size=.24); heap(k, 26.1, 23.0, z, 1.05, 1.0, SAND, seed=9, lumps=30, size=.24)
    heap(k, 28.3, 21.6, z, 1.05, 1.6, STONE, seed=10, lumps=44, size=.3); heap(k, 28.4, 23.1, z, 1.1, 1.0, STONE, seed=11, lumps=30, size=.3)
    # Spare mast sections laid out beside the crane base.
    k.truss((27.4, 1.6, z + .12), (27.4, 6.4, z + .12), 1.7, 1.7, YELLOW, bays=3, t=.17)
    k.truss((19.0, 1.8, z + .12), (19.0, 5.4, z + .12), 1.7, 1.7, YELLOW, bays=2, t=.17)
    excavator(k, 12.6, 26.2, z, yaw=186, slew=-8, s=.92)
    for i in range(4): jersey(k, 1.2 + i * 2.3, 29.1, z, 2.0, yaw=-90)
    hazard_edge(k, 10.6, 28.9, 9.0, .35, z)
    flagpole(k, 19.6, 22.6, z, 8.5); floodlight(k, 1.5, 14.6, z, 9.0, yaw=45); floodlight(k, 29.2, 18.6, z, 9.0, yaw=150)

# --- Ore refinery -----------------------------------------------------------------------------------
def conc_silo(k, x, y, z, r, h, band=None):
    """Slip-formed concrete storage silo with a roof slab, handrail and caged ladder."""
    k.cyl(x, y, z, r, h, PRECAST, n=20)
    k.rev((x, y, z), 'z', [(0, r + .12), (.9, r + .12)], CONC_DARK, n=20)
    for f in (.25, .5, .75): k.rev((x, y, z + h * f), 'z', [(0, r + .015), (.07, r + .015)], tint(PRECAST, .86), n=20)
    k.rev((x, y, z + h), 'z', [(0, r + .2), (.25, r + .2)], CONC_LIGHT, n=20)
    k.rail([(x + (r + .1) * cos(i * pi / 6), y + (r + .1) * sin(i * pi / 6), z + h + .25) for i in range(13)], GALV, posts=9)
    if band: k.rev((x, y, z + h - 2.2), 'z', [(0, r + .03), (1.0, r + .03)], band, n=20)
    k.ladder(x + r * .72, y + r * .72, z + .9, z + h + .2, axis='x')

def refinery(k, w, d):
    # Ore refinery: a truck tipping bay over the receiving hopper, an inclined conveyor gallery up
    # to the crusher house, concentrate silos, a smelter with its stack, thickener and stockpiles.
    z = pad(k, w, d)
    apron(k, 8.4, 21.4, 21.0, 8.0, z)
    # Tipping bay: grizzly over the hopper, wing walls, hazard-striped dock.
    bx0, bx1, by0, by1 = 9.8, 18.2, 21.8, 27.8
    k.box(bx0, by0, z, bx1 - bx0, by1 - by0, .07, CONC_DARK)
    k.box(bx0 + .7, by0 + .5, z + .07, bx1 - bx0 - 1.4, 3.7, .08, BLACK)
    rnd = random.Random(7)
    for i in range(26): k.ball(rnd.uniform(bx0 + 1.2, bx1 - 1.2), rnd.uniform(by0 + .9, by0 + 3.8), z + .2, rnd.uniform(.24, .4), tint(ORE, rnd.uniform(.65, 1.1)), rz=.22, n=6, k=3)
    for i in range(10): k.box(bx0 + .7 + i * (bx1 - bx0 - 1.5) / 9, by0 + .5, z + .15, .1, 3.7, .1, STEEL)
    for yy in (by0 + 1.7, by0 + 3.0): k.box(bx0 + .7, yy, z + .13, bx1 - bx0 - 1.4, .1, .1, STEEL)
    k.box(bx0 - .5, by0 - .5, z, bx1 - bx0 + 1.0, .5, 2.6, CONCRETE)
    for xx in (bx0 - .5, bx1):
        k.box(xx, by0, z, .5, 4.6, 1.7, CONCRETE); k.box(xx - .02, by0, z + 1.7, .54, 4.6, .04, HAZ); k.box(xx - .02, by0 + 4.6, z, .54, .05, 1.7, HAZ)
    k.box(bx0 - .5, by0 - .52, z + 2.6, bx1 - bx0 + 1.0, .54, .04, HAZ)
    k.box(bx0 + .7, by0 + 4.3, z + .07, bx1 - bx0 - 1.4, .4, .32, HAZ)
    hazard_edge(k, bx0, by1 - .45, bx1 - bx0, .45, z + .05)
    for xx in (bx0, bx1 - .4): hazard_edge(k, xx, by0 + 4.8, .4, by1 - by0 - 5.2, z + .05)
    bollards(k, [(bx0 - .25, by0 + 5.2), (bx1 + .25, by0 + 5.2)], z)
    k.tube((bx1 + .25, by0 + .2, z + 2.6), (bx1 + .25, by0 + .2, z + 5.0), .07, GALV, n=6)       # signal
    k.box(bx1 + .0, by0 + .25, z + 4.2, .5, .3, 1.1, DARK); lamp(k, bx1 + .25, by0 + .58, z + 4.95, RED_LAMP, .13); lamp(k, bx1 + .25, by0 + .58, z + 4.5, GREEN_LAMP, .13)
    # Transfer house behind the head wall and the conveyor gallery.
    k.box(12.7, 19.4, z, 2.6, 1.9, 3.4, CLAD_GREY, top=CLAD_DARK)
    conveyor(k, (14.0, 20.2, z + 2.8), (14.0, 9.0, z + 10.2), 1.6, CLAD_GREY, legs=3)
    # Crusher and screen house.
    cx, cy, cw, cd, ch = 10.5, 2.0, 7.0, 7.0, 13.0
    k.box(cx, cy, z, cw, cd, ch, CLAD_GREY); k.box(cx - .06, cy - .06, z, cw + .12, cd + .12, 1.4, PRECAST)
    gable(k, cx, cy, z + ch, cw, cd, 1.3, CLAD_DARK, ridge='y', wall=CLAD_GREY)
    k.box(cx - .05, cy + cd - .02, z + ch - .85, cw + .1, .08, .65, TEAM); k.box(cx + cw - .02, cy - .05, z + ch - .85, .08, cd + .1, .65, TEAM)
    with face(k, 'y+', cx, cy + cd, z):
        k.box(2.5, 0, 9.3, 2.0, .5, 1.9, DARK)
        windows(k, cw, 1, z0=4.6, h=1.1, margin=.8, bay=1.35); door(k, 5.2)
        for i in range(2): k.box(.6 + i * 4.6, -.02, 7.4, 1.2, .1, 1.2, DARK)
    with face(k, 'x+', cx + cw, cy + cd, z): windows(k, cd, 3, z0=2.4, h=1.1, floor_h=3.6, margin=.8, bay=1.35)
    vent(k, cx + 1.6, cy + 1.6, z + ch + .5, .4, 1.3); k.cyl(cx + 5.4, cy + 5.0, z + ch + .3, .35, 3.2, GALV, n=10)
    # Concentrate silos fed by a gallery from the crusher house.
    for i, xx in enumerate((20.5, 24.2, 27.9)): conc_silo(k, xx, 4.0, z, 1.72, 11.2, band=TEAM if i == 1 else None)
    conveyor(k, (17.5, 4.0, z + 12.4), (28.9, 4.0, z + 12.4), 1.3, CLAD_GREY, legs=0)
    # Smelter: furnace hall with a louvred roof monitor, off-gas duct and stack.
    fx, fy, fw, fd, fh = 1.5, 7.0, 7.5, 11.5, 7.5
    k.box(fx, fy, z, fw, fd, fh, CLAD_DARK); k.box(fx - .06, fy - .06, z, fw + .12, fd + .12, 2.4, PRECAST)
    gable(k, fx, fy, z + fh, fw, fd, 1.6, CLAD_GREY, ridge='y', wall=CLAD_DARK)
    k.box(fx + fw / 2 - 1.1, fy + .9, z + fh + 1.2, 2.2, fd - 1.8, 1.0, CLAD_DARK); k.box(fx + fw / 2 - 1.4, fy + .7, z + fh + 2.2, 2.8, fd - 1.4, .12, CLAD_GREY)
    for i in range(7): k.box(fx + fw / 2 + 1.1, fy + 1.3 + i * 1.4, z + fh + 1.45, .04, 1.0, .6, BLACK)
    with face(k, 'x+', fx + fw, fy + fd, z):
        shutter(k, 1.6, 3.6, 4.2, open_=.55); k.box(2.2, -.28, .2, 2.4, .04, 1.3, HOT)
        windows(k, fd, 1, z0=5.5, h=1.0, margin=1.2, bay=1.6); door(k, 9.2)
    with face(k, 'y+', fx, fy + fd, z): windows(k, fw, 1, z0=5.5, h=1.0, margin=1.0, bay=1.6); door(k, 1.2)
    k.box(fx + fw - .02, fy - .05, z + fh - .8, .08, fd + .1, .55, TEAM)
    chimney(k, 3.4, 3.4, z, 21.0, 1.15, rgb(170, 172, 170), bands=3)
    k.box(4.4, 7.4, z + fh + .6, 1.4, 1.4, 2.8, GALV); k.tube((5.1, 8.0, z + fh + 3.0), (3.7, 4.4, z + 8.6), .62, GALV, n=10)
    # Ingots stacked for dispatch, pipe rack to the reagent tanks.
    for i in range(2):
        for j in range(2): pallet(k, 9.9 + i * 1.45, 14.4 + j * 1.3, z, ORE, h=.45 + .3 * ((i + j) % 2))
    pipe_rack(k, 9.2, 10.9, z, 10.4, 'x', zt=4.4, w=1.8)
    tank(k, 21.2, 9.4, z, 1.7, 4.6, WHITE, roof='cone'); tank(k, 25.4, 9.4, z, 1.7, 3.8, rgb(170, 176, 170), roof='flat', stair=False)
    # Stockpile bunkers.
    k.box(21.6, 12.2, z, 7.8, .4, 2.4, CONCRETE)
    for xx in (21.6, 25.5): k.box(xx, 12.6, z, .4, 5.4, 2.4 if xx < 22 else 1.8, CONCRETE)
    ROCK = rgb(140, 108, 44, 'rust')
    heap(k, 23.75, 14.4, z, 1.6, 1.7, ORE, seed=2, base=ROCK); heap(k, 23.9, 16.2, z, 1.4, 1.1, ORE, seed=4, lumps=40, base=ROCK)
    heap(k, 27.6, 14.6, z, 1.8, 2.1, ORE, seed=3, lumps=70, base=ROCK); heap(k, 27.9, 16.6, z, 1.5, 1.2, ORE, seed=6, lumps=40, base=ROCK)
    # Control cabin on stilts overlooking the bay.
    ax, ay = 19.6, 18.4
    for xx in (ax + .2, ax + 3.8):
        for yy in (ay + .2, ay + 2.6): k.box(xx - .1, yy - .1, z, .2, .2, 3.2, STEEL)
    k.box(ax - .1, ay - .1, z + 3.1, 4.2, 3.9, .12, GALV)
    k.box(ax, ay, z + 3.2, 4.0, 2.8, 2.5, OFFWHITE, top=ROOF); k.box(ax - .25, ay - .25, z + 5.7, 4.5, 3.4, .16, TEAM)
    with face(k, 'y+', ax, ay + 2.8, z + 3.2): windows(k, 4.0, 1, z0=.9, h=1.2, margin=.25, bay=1.0)
    with face(k, 'x+', ax + 4.0, ay + 2.8, z + 3.2): windows(k, 2.8, 1, z0=.9, h=1.2, margin=.25, bay=.9)
    k.rail([(ax - .1, ay + 3.8, z + 3.22), (ax + 4.1, ay + 3.8, z + 3.22)], GALV)
    k.stairs(28.2, ay + 2.85, z, z + 3.2, -4.1, .9, axis='x')
    # Thickener.
    tx, ty = 4.6, 24.6
    k.cyl(tx, ty, z, 3.3, 1.5, CONCRETE, n=24, top=WATER)
    k.truss((tx - 3.5, ty, z + 1.6), (tx + 3.5, ty, z + 1.6), .9, .8, GALV, bays=5, t=.07)
    k.cyl(tx, ty, z + 1.5, .5, 1.6, rgb(92, 108, 92), n=10); k.box(tx - .4, ty - .4, z + 3.1, .8, .8, .5, GALV)
    k.pipe([(tx + 3.3, ty - 1.5, z + .6), (tx + 4.4, ty - 1.5, z + .6), (tx + 4.4, ty - 3.4, z + .6)], .16, STEEL)
    barrels(k, 10.0, 18.2, z, 3); crate(k, 12.0, 18.0, z, 1.1)
    flagpole(k, 2.0, 20.2, z, 8.5); floodlight(k, 8.9, 28.8, z, 8.0, yaw=-30); floodlight(k, 29.0, 28.6, z, 8.0, yaw=200)

# --- Battle laboratory ------------------------------------------------------------------------------
def tech_center(k, w, d):
    # Battle laboratory: a four-storey research block with a glazed curtain wall, a service core
    # and penthouse, a two-storey annex carrying the satellite ground station, and a guarded gate.
    z = pad(k, w, d)
    apron(k, .8, 14.8, 18.4, 4.5, z)
    # Annex.
    top2 = block(k, 11.5, 4.0, 7.0, 10.3, 6.8, PRECAST, plinth=CONC_DARK, floors=2, band=TEAM)
    # Research block: precast frame, curtain wall on the two public faces.
    tx, ty, tw, td, th = 2.0, 2.0, 9.8, 10.6, 14.0
    k.box(tx, ty, z, tw, td, th, PRECAST, top=ROOF); k.box(tx - .06, ty - .06, z, tw + .12, td + .12, .6, CONC_DARK)
    parapet(k, tx, ty, z + th, tw, td, PRECAST, .7)
    k.box(tx - .05, ty + td - .02, z + th + .08, tw + .1, .08, .5, TEAM); k.box(tx + tw - .02, ty - .05, z + th + .08, .08, td + .1, .5, TEAM)
    with face(k, 'y+', tx, ty + td, z):
        curtain(k, tw, .6, th - .5, floor_h=3.35)
        k.box(3.4, -.02, 0, 3.0, .16, 2.9, GLASS_DARK); k.box(3.3, .02, 2.9, 3.2, .16, .12, MULLION)      # lobby doors
        for xx in (3.4, 4.85, 6.3): k.box(xx, .04, 0, .1, .14, 2.9, MULLION)
        k.box(2.6, 0, 3.1, 4.6, 2.2, .22, WHITE); k.box(2.55, 2.1, 3.05, 4.7, .12, .32, TEAM)             # canopy
        for xx in (2.9, 6.9): k.tube((xx, 1.9, 0), (xx, 1.9, 3.1), .1, GALV, n=8)
        k.box(2.8, .1, 0, 4.2, 2.4, .12, CONC_LIGHT)
    with face(k, 'x+', tx + tw, ty + td, z): curtain(k, td, .6, th - .5, floor_h=3.35)
    # Penthouse over the core, the antenna farm on it.
    k.box(2.3, 2.3, z + th, 4.2, 4.0, 2.9, PRECAST, top=ROOF)
    for i in range(4): k.box(2.8 + i * .8, 6.3, z + th + 1.0, .6, .05, 1.3, DARK)
    k.box(6.5, 3.4, z + th + .4, .05, 1.0, 2.0, rgb(84, 90, 96))
    antenna_mast(k, 3.3, 3.3, z + th + 2.9, 5.4, w=.8, dishes=2)
    for (xx, yy, hh) in [(5.6, 2.9, 3.4), (5.9, 5.6, 2.6), (2.9, 5.7, 3.0)]:
        k.tube((xx, yy, z + th + 2.9), (xx, yy, z + th + 2.9 + hh), .045, STEEL, n=4)
        for j in range(3): k.strut((xx - .5, yy, z + th + 2.9 + hh * (.55 + .18 * j)), (xx + .5, yy, z + th + 2.9 + hh * (.55 + .18 * j)), .035, STEEL)
    radome(k, 8.9, 5.2, z + th, 2.2)
    # Chillers and ducts along the front of the roof.
    for i in range(2):
        bx = 3.0 + i * 4.1
        k.box(bx, 9.3, z + th + .2, 3.4, 2.0, 1.5, GALV); k.box(bx + .15, 9.4, z + th, 3.1, 1.8, .2, DARK)
        for j in range(3):
            k.cyl(bx + .6 + j * 1.1, 10.3, z + th + 1.7, .46, .08, DARK, n=12)
            k.box(bx + .2 + j * 1.1, 11.3, z + th + .35, .75, .03, 1.2, DARK)
    k.box(2.9, 7.6, z + th, 3.6, .5, .45, GALV); k.box(6.3, 7.6, z + th, .5, 1.8, .45, GALV)
    vent(k, 10.6, 8.2, z + th, .28, .8); vent(k, 10.8, 3.0, z + th, .25, .7)
    # Ground station on the annex roof.
    dish(k, 14.4, 7.2, top2, 1.7, yaw=35, pitch=42); dish(k, 16.6, 11.6, top2, 1.15, yaw=70, pitch=38)
    ac_unit(k, 12.4, 11.6, top2, 1.8, 1.2, 1.0); ac_unit(k, 16.2, 4.8, top2, 1.6, 1.1, .9)
    k.box(12.2, 9.8, top2, 3.6, .4, .35, GALV); vent(k, 17.6, 8.6, top2, .22, .6)
    with face(k, 'y+', 11.5, 14.3, z): door(k, 4.6)
    # Forecourt: gate house and barrier, bollards, planters, perimeter fence, flag.
    k.box(14.6, 16.0, z, 2.0, 2.0, 2.5, WHITE); k.box(14.4, 15.8, z + 2.5, 2.4, 2.4, .14, DARK)
    k.box(14.7, 17.96, z + 1.0, 1.8, .06, 1.0, GLASS); k.box(16.56, 16.1, z + 1.0, .06, 1.8, 1.0, GLASS)
    k.box(14.55, 15.95, z, 2.1, 2.1, .35, TEAM)
    k.cbox(13.9, 18.9, z, .35, .35, 1.1, WHITE); k.tube((13.9, 18.9, z + 1.0), (9.0, 18.9, z + 1.15), .07, SIGNAL_RED, n=6)
    for i in range(3): k.rev((12.9 - i * 1.5, 18.9, z + 1.02 + i * .045), 'x', [(-.3, .075), (.3, .075)], WHITE, n=6)
    fence(k, [(.9, 19.3, z), (8.6, 19.3, z)], h=2.2); fence(k, [(14.2, 19.3, z), (19.3, 19.3, z), (19.3, 14.6, z)], h=2.2)
    bollards(k, [(4.6 + i * 1.1, 15.4) for i in range(5)], z, c=rgb(196, 196, 190))
    shrub(k, 2.2, 13.2, z, 2.2, seed=3); shrub(k, 9.4, 13.2, z, 1.9, seed=4)
    flagpole(k, 1.8, 17.6, z, 9.0)
    for xx in (3.2, 10.8): k.tube((xx, 18.6, z), (xx, 18.6, z + 3.6), .06, GALV, n=6); k.box(xx - .1, 18.1, z + 3.55, .2, .6, .1, DARK); lamp(k, xx, 18.2, z + 3.52, r=.09)

# --- Ore purifier -----------------------------------------------------------------------------------
def column(k, x, y, z, r, h, plats=(), c=None):
    """Process column: skirt, jacketed shell, circular platforms with ladders, overhead line."""
    c = c or ALU
    k.cyl(x, y, z, r * 1.1, 1.5, CONC_DARK, n=16)
    k.cyl(x, y, z + 1.5, r, h - 1.5 - r * .55, c, n=16); k.dome(x, y, z + h - r * .55, r, c, h=r * .55, n=16)
    zz = z + 3.2
    while zz < z + h - 1.2:
        k.rev((x, y, zz), 'z', [(0, r + .025), (.1, r + .025)], tint(c, .8), n=16); zz += 2.2
    prev = z
    for i, zp in enumerate(plats):
        k.rev((x, y, z + zp - .1), 'z', [(0, r + 1.0), (.1, r + 1.0)], GALV, n=16)
        k.rail([(x + (r + .95) * cos(j * pi / 6), y + (r + .95) * sin(j * pi / 6), z + zp) for j in range(13)], GALV, posts=9, t=.045)
        if i % 2 == 0: k.ladder(x + r + .14, y + .3, prev, z + zp + 1.0, axis='y')
        else: k.ladder(x + .3, y + r + .14, prev, z + zp + 1.0, axis='x')
        prev = z + zp
    a = radians(118)
    px, py = x + (r + .42) * cos(a), y + (r + .42) * sin(a)
    k.pipe([(x, y, z + h - .2), (x, y, z + h + .55), (px, py, z + h + .55), (px, py, z + 4.6)], .2, tint(c, .92))

def flare(k, x, y, z, h):
    """Derrick-supported flare stack with a small pilot flame."""
    k.mast(x, y, z, h * .82, 2.0, .8, GALV, bays=int(h * .82 / 2.2), t=.12)
    k.cyl(x, y, z, .24, h, STEEL, n=8); k.cyl(x, y, z + h, .36, 1.0, DARK, n=8)
    k.rev((x, y, z + h + 1.0), 'z', [(0, .22), (.35, .3), (.75, .16), (1.1, .03)], HOT, n=8)
    k.cyl(x - 1.4, y + 1.2, z, .7, 1.6, STEEL, n=12); k.dome(x - 1.4, y + 1.2, z + 1.6, .7, STEEL, h=.3, n=12)   # knock-out drum

def ore_purifier(k, w, d):
    # Ore purification plant: fractionating columns, a process frame of drums and air coolers, a
    # product sphere, exchangers and pipe racks, a flare, and a blast-resistant control room.
    z = pad(k, w, d)
    apron(k, 1.0, 1.0, 18.0, 12.4, z, GRAVEL)
    column(k, 3.5, 3.6, z, 1.15, 19.5, plats=(6.0, 11.0, 16.0)); k.rev((3.5, 3.6, z + 17.4), 'z', [(0, 1.19), (.9, 1.19)], TEAM, n=16)
    column(k, 7.3, 3.0, z, .9, 15.5, plats=(5.5, 11.5))
    column(k, 3.2, 7.4, z, .75, 12.0, plats=(5.0, 9.4))
    # Process frame: two grated decks, a reflux drum, a vertical vessel, fin-fan coolers on top.
    fx, fy, fw, fd = 10.0, 1.6, 6.4, 5.4
    for xx in (fx, fx + fw / 2, fx + fw):
        for yy in (fy, fy + fd): k.box(xx - .13, yy - .13, z, .26, .26, 8.2, STEEL)
    for zz in (4.0, 7.6):
        k.box(fx - .15, fy - .15, z + zz - .25, fw + .3, fd + .3, .25, rgb(120, 126, 122)); k.box(fx, fy, z + zz, fw, fd, .05, GALV)
    k.rail([(fx, fy + fd, z + 4.05), (fx + fw, fy + fd, z + 4.05), (fx + fw, fy, z + 4.05)], GALV)
    k.rail([(fx, fy + fd, z + 7.65), (fx + fw, fy + fd, z + 7.65), (fx + fw, fy, z + 7.65)], GALV)
    for s in (0, 1): k.strut((fx + s * fw / 2, fy + fd, z + .2), (fx + (s + 1) * fw / 2, fy + fd, z + 3.7), .1, STEEL)
    k.strut((fx + fw, fy, z + .2), (fx + fw, fy + fd, z + 3.7), .1, STEEL)
    htank(k, fx + .7, fy + 3.4, z + 4.05, 5.0, .85, ALU, axis='x')
    k.cyl(fx + 1.6, fy + 1.4, z + 4.05, .8, 2.6, WHITE, n=12); k.dome(fx + 1.6, fy + 1.4, z + 6.65, .8, WHITE, h=.35, n=12)
    k.box(fx + .3, fy + .4, z + 8.3, fw - .6, fd - .8, .9, GALV)
    for xx in (fx + .5, fx + fw - .5):
        for yy in (fy + .6, fy + fd - .6): k.box(xx - .08, yy - .08, z + 7.65, .16, .16, .7, STEEL)
    for i in range(2):
        k.cyl(fx + 1.8 + i * 2.8, fy + fd / 2, z + 9.2, 1.15, .3, tint(GALV, .9), n=16); k.cyl(fx + 1.8 + i * 2.8, fy + fd / 2, z + 9.48, 1.05, .04, DARK, n=16)
    k.stairs(fx + fw + .2, fy + fd + .2, z, z + 4.0, -5.4, .9, axis='x')
    k.ladder(fx + fw + .2, fy + 1.4, z + 4.0, z + 8.6, axis='y')
    flare(k, 18.2, 1.9, z, 22.0)
    # Pipe rack across the unit and feeders up to the columns.
    pipe_rack(k, 1.4, 9.6, z, 17.2, 'x', zt=4.4, w=2.2, tiers=(((.2, ALU), (.14, STEEL), (.18, rgb(170, 60, 50)), (.12, STEEL)), ((.16, STEEL), (.24, ALU), (.12, rgb(196, 170, 50)))), bay=4.3)
    for (xx, yy, rr) in [(4.4, 5.2, .18), (7.6, 4.4, .16), (3.6, 8.5, .14), (12.4, 7.4, .2), (15.0, 7.4, .16)]:
        k.pipe([(xx, yy, z + 1.2), (xx, yy, z + 4.7), (xx, 9.0, z + 4.7)], rr, ALU if rr > .15 else STEEL)
    # Exchangers, pumps.
    htank(k, 1.8, 11.9, z, 5.2, .62, ALU, axis='x'); htank(k, 1.8, 11.9, z + 1.72, 5.2, .62, ALU, axis='x')
    for s in (0, 1): k.rev((1.8 + s * 5.2, 11.9, z + 1.12 + 1.72 * 0), 'x', [(-.06, .74), (.06, .74)], STEEL, n=14); k.rev((1.8 + s * 5.2, 11.9, z + 2.84), 'x', [(-.06, .74), (.06, .74)], STEEL, n=14)
    for i in range(3):
        px = 8.6 + i * 1.5
        k.box(px, 11.2, z, 1.1, .6, .2, CONC_DARK); k.tube((px + .1, 11.5, z + .5), (px + .7, 11.5, z + .5), .24, rgb(70, 110, 84), n=10)
        k.cbox(px + .9, 11.5, z + .2, .4, .5, .6, STEEL); k.tube((px + .9, 11.5, z + .8), (px + .9, 11.5, z + 4.3), .09, STEEL, n=6)
    # Product sphere.
    sphere_tank(k, 15.6, 14.0, z, 2.55, WHITE, band=TEAM)
    k.stairs(12.6, 16.2, z, z + 3.6, 1.6, .7, axis='x'); k.pipe([(15.6, 11.5, z + 2.2), (15.6, 10.6, z + 2.2), (15.6, 10.6, z + 4.6)], .16, STEEL)
    k.ladder(15.6, 16.62, z, z + 3.8, axis='x')
    # Control room.
    top = block(k, 2.0, 14.2, 7.4, 4.2, 3.7, CONCRETE, plinth=CONC_DARK, floors=1, glaze=None, band=TEAM, para=.35)
    with face(k, 'y+', 2.0, 18.4, z):
        door(k, 1.0, 1.0, c=rgb(84, 90, 96)); k.box(3.2, -.05, 1.3, 3.2, .1, 1.0, GLASS_DARK); k.box(3.1, -.02, 1.2, 3.4, .14, .1, CONC_LIGHT); k.box(3.1, -.02, 2.3, 3.4, .14, .1, CONC_LIGHT)
        for xx in (4.25, 5.3): k.box(xx, 0, 1.3, .08, .1, 1.0, CONC_LIGHT)
    with face(k, 'x+', 9.4, 18.4, z): k.box(1.2, -.05, 1.3, 1.6, .1, 1.0, GLASS_DARK); k.box(1.1, -.02, 1.2, 1.8, .14, .1, CONC_LIGHT)
    ac_unit(k, 2.8, 15.0, top, 1.8, 1.2, .9); ac_unit(k, 5.6, 15.4, top, 1.4, 1.0, .8); vent(k, 8.2, 17.2, top, .22, .6)
    k.tube((8.6, 15.0, top), (8.6, 15.0, top + 3.4), .04, STEEL, n=4)
    # Refined product on pallets, drums, fence, flag.
    for i in range(2): pallet(k, 10.4 + i * 1.5, 17.0, z, ORE, h=.55)
    pallet(k, 11.1, 15.6, z, ORE, h=.4); barrels(k, 10.2, 13.2, z, 3, rows=2)
    fence(k, [(10.0, 19.3, z), (19.3, 19.3, z), (19.3, 10.6, z)], h=2.0); k.box(14.0, 19.24, z + .8, 1.2, .05, .8, HAZ)
    flagpole(k, 1.5, 19.0, z, 8.0); floodlight(k, 18.8, 9.6, z, 8.0, yaw=160)

# --- Naval shipyard ---------------------------------------------------------------------------------
def quay(k, x, y, w, d, z=1.2, t=.55, c=CONCRETE, fend='xy'):
    """Concrete deck on tubular piles over the water: painted coping, fender piles, tyre fenders."""
    k.box(x, y, z - t, w, d, t, c)
    jx = x + 5.0
    while jx < x + w - 1: k.box(jx - .04, y + .02, z, .08, d - .04, .012, tint(c, .62)); jx += 5.0
    jy = y + 5.0
    while jy < y + d - 1: k.box(x + .02, jy - .04, z, w - .04, .08, .012, tint(c, .62)); jy += 5.0
    nx, ny = max(1, int(round((w - 1.0) / 2.6))), max(1, int(round((d - 1.0) / 2.6)))
    for i in range(nx + 1):
        px = x + .5 + i * (w - 1.0) / nx
        for yy in (y + .42, y + d - .42): k.cyl(px, yy, 0, .3, z - t, RUST, n=8)
        if 'y' in fend:
            k.cyl(px, y + d + .2, 0, .2, z + .12, TIMBER, n=6)
            if i < nx and i % 2 == 0: k.torus(px + (w - 1.0) / nx / 2, y + d + .2, z - .6, .4, .17, TYRE, n=12, k=6, axis='y')
    for j in range(ny + 1):
        py = y + .5 + j * (d - 1.0) / ny
        for xx in (x + .42, x + w - .42): k.cyl(xx, py, 0, .3, z - t, RUST, n=8)
        if 'x' in fend:
            k.cyl(x + w + .2, py, 0, .2, z + .12, TIMBER, n=6)
            if j < ny and j % 2 == 0: k.torus(x + w + .2, py + (d - 1.0) / ny / 2, z - .6, .4, .17, TYRE, n=12, k=6, axis='x')
    if 'y' in fend: k.box(x, y + d - .3, z, w, .3, .02, tint(YELLOW, .92))
    if 'x' in fend: k.box(x + w - .3, y, z, .3, d, .02, tint(YELLOW, .92))

def ship_hull(k):
    """An 18 m steel hull on the building ways (bow at +x, keel at z = 0): plated and primed but
    not fitted out, on keel blocks and shores, with staging along one side."""
    st = [(-8.0, 2.2, 1.4, 3.75), (-6.2, 2.78, .35, 3.6), (-2.0, 2.9, 0, 3.55), (2.6, 2.85, 0, 3.6),
          (5.2, 2.3, .08, 3.85), (6.9, 1.25, .4, 4.2), (8.0, .12, 1.5, 4.6)]
    def ring(x, b, z0, z1):
        m = z0 + (z1 - z0) * .42; lo = z0 + (z1 - z0) * .1
        return [(x, b, m), (x, b, z1), (x, -b, z1), (x, -b, m), (x, -b * .8, lo), (x, 0, z0), (x, b * .8, lo)]
    k.loft([ring(*s) for s in st], HULL_RED)
    k.loft([[(x, b + .05, max(1.9, z0 + .1)), (x, b + .05, 2.3), (x, -b - .05, 2.3), (x, -b - .05, max(1.9, z0 + .1))] for x, b, z0, z1 in st], TEAM, cap_c=HULL_RED)   # boot-topping
    k.loft([[(x, b + .03, max(2.2, z0 + .25)), (x, b + .03, z1 + .02), (x, -b - .03, z1 + .02), (x, -b - .03, max(2.2, z0 + .25))] for x, b, z0, z1 in st], NAVY_GREY, cap_c=NAVY_GREY)
    for (x0, b0, _, t0), (x1, b1, _, t1) in zip(st, st[1:]):
        k.plate([(x0, -b0 + .12, t0 + .05), (x1, -b1 + .12, t1 + .05), (x1, b1 - .12, t1 + .05), (x0, b0 - .12, t0 + .05)], DECK_GREY)
        if x0 >= 2.6:                                                                # bow bulwark
            for s in (-1, 1): k.plate([(x0, s * (b0 + .03), t0), (x1, s * (b1 + .03), t1), (x1, s * (b1 + .03), t1 + .85), (x0, s * (b0 + .03), t0 + .85)], NAVY_GREY)
    k.box(-3.7, -1.7, 3.6, 3.3, 3.4, .5, IRON, top=BLACK)                            # open hold
    k.box(.5, -1.7, 3.62, 3.3, 3.4, .5, IRON, top=PRIMER)                            # hatch covers fitted
    for i in range(4): k.box(.55 + i * .82, -1.7, 4.12, .08, 3.4, .1, IRON)
    k.box(-7.7, -1.9, 3.7, 3.2, 3.8, 2.3, PRIMER); k.box(-7.5, -1.7, 6.0, 2.8, 3.4, .03, BLACK)   # deckhouse shell, no roof yet
    for yy in (-1.2, .3): k.box(-4.52, yy, 4.7, .04, .9, .7, BLACK)
    for xx in (-7.2, -6.0): k.box(xx, 1.9, 4.7, .8, .04, .7, BLACK)
    k.cyl(6.2, 0, 4.1, .12, 2.4, PRIMER, n=6)
    k.box(-8.1, -.07, .2, 1.9, .14, 1.35, HULL_RED); k.box(-8.8, -.06, .15, .62, .12, 1.8, HULL_RED)   # skeg, rudder
    k.rev((-8.18, 0, .9), 'x', [(0, .1), (.35, .22)], BRASS, n=8)
    for i in range(4):
        a = i * pi / 2 + .4
        k.ball(-8.02, .45 * cos(a), .9 + .45 * sin(a), .07, BRASS, ry=.14 + .32 * abs(cos(a)), rz=.14 + .32 * abs(sin(a)), n=8, k=4)
    for i in range(8):                                                                # keel blocks, bilge shores
        x = -6.6 + i * 1.9
        k.box(x - .3, -.5, -.8, .6, 1.0, .8, TIMBER)
        if i % 2 == 1: k.strut((x, 3.9, -.8), (x, 2.5, 1.0), .16, TIMBER)
    for i in range(6):                                                                # staging along the far side
        x = -6.5 + i * 2.3
        for yy in (-3.2, -4.2): k.strut((x, yy, -.8), (x, yy, 5.6), .08, GALV)
        for zz in (2.2, 4.3): k.strut((x, -3.2, zz), (x, -4.2, zz), .06, GALV)
    for zz in (2.2, 4.3): k.box(-6.5, -4.15, zz, 11.5, .9, .07, WOOD)
    k.strut((-6.5, -4.2, 5.4), (5.0, -4.2, 5.4), .05, GALV)
    k.ladder(-1.2, 3.02, -.8, 3.9, axis='x')

def portal_crane(k, x, y, z, yaw, c=None, gauge=5.0, base=6.0, ph=7.5, jib=17.0, luff=56.0, drop=9.0, side=-1):
    """Level-luffing dockside crane: rail portal, slewing machinery house, A-frame, lattice jib;
    a hull plate hangs from the hook. `side` puts the driver's cab on the house's -y or +y."""
    c = c or rgb(84, 112, 120)
    for sx in (-1, 1):
        for sy in (-1, 1):
            k.cbox(x + sx * gauge / 2, y + sy * base / 2, z, .7, 1.9, .7, DARK)
            k.loft([[(x + sx * gauge / 2 + a, y + sy * base / 2 + b, z + .7) for a, b in ((-.35, -.35), (.35, -.35), (.35, .35), (-.35, .35))],
                    [(x + sx * (gauge / 2 - .9) + a, y + sy * (base / 2 - 1.2) + b, z + ph) for a, b in ((-.3, -.3), (.3, -.3), (.3, .3), (-.3, .3))]], c, smooth=False)
        k.box(x + sx * gauge / 2 - .22, y - base / 2, z + .9, .44, base, .5, c)
    for sy in (-1, 1): k.box(x - gauge / 2, y + sy * (base / 2 - .25) - .2, z + 3.4, gauge, .4, .4, c)
    k.cbox(x, y, z + ph, gauge - .6, base - 1.2, .8, c); k.cyl(x, y, z + ph + .8, 1.9, .5, DARK, n=16)
    k.ladder(x + gauge / 2 - .5, y + base / 2 - .9, z + .7, z + ph + .8, axis='x')
    with k.at(x, y, z + ph + 1.3, yaw=yaw):
        k.box(-3.6, -1.9, 0, 5.8, 3.8, 3.0, WHITE, top=ROOF); k.box(-3.65, -1.95, 2.35, 5.9, 3.9, .4, TEAM)
        for s in (-1, 1):
            for i in range(3): k.box(-2.9 + i * 1.5, s * 1.9 - .03, .9, .9, .06, .9, GLASS_DARK)
        k.box(-4.5, -1.6, .4, .9, 3.2, 2.2, CONC_DARK)                                # counterweight
        cy = -2.6 if side < 0 else 1.1
        k.box(2.2, cy, .9, 1.9, 1.5, 2.0, WHITE, top=DARK); k.box(4.1, cy + .1, 1.6, .04, 1.3, 1.2, GLASS)
        k.box(2.4, cy - .03 if side < 0 else cy + 1.5, 1.6, 1.6, .04, 1.2, GLASS)
        for s in (-1, 1):
            k.strut((.4, s * 1.4, 3.0), (-1.4, s * .3, 9.4), .22, c); k.strut((-3.0, s * 1.4, 3.0), (-1.4, s * .3, 9.4), .2, c)
        k.box(-1.6, -.4, 9.3, .5, .8, .4, DARK)
        a = radians(luff)
        with k.at(1.9, 0, 2.6, pitch=luff):
            k.truss((0, 0, -.55), (jib, 0, -.55), 1.5, 1.1, c, bays=9, t=.13)
            k.box(jib - .2, -.85, -.7, .5, 1.7, 1.4, c)
        tipx, tipz = 1.9 + jib * cos(a), 2.6 + jib * sin(a)
        for f in (.55, .96): k.strut((-1.4, 0, 9.5), (1.9 + jib * f * cos(a), 0, 2.6 + jib * f * sin(a) + .5), .06, DARK)
        for s in (-1, 1): k.strut((tipx + .2, s * .25, tipz), (tipx + .2, s * .12, tipz - drop), .045, DARK)
        k.cbox(tipx + .2, 0, tipz - drop - .6, .6, .4, .6, YELLOW); k.cbox(tipx + .2, 0, tipz - drop - .95, .14, .14, .35, DARK)
        zl = tipz - drop - 3.0
        for s in (-1, 1): k.strut((tipx + .2, 0, tipz - drop - .95), (tipx + .2, s * 1.5, zl + .1), .035, DARK)
        k.cbox(tipx + .2, 0, zl, 2.2, 3.4, .1, PRIMER)

def hull_block(k, x, y, z, yaw=0.0):
    """Prefabricated hull section waiting on blocks: bottom shell, side frames, a bulkhead."""
    with k.at(x, y, z, yaw=yaw):
        for xx in (.3, 2.9): k.box(xx, -1.9, 0, .4, 3.8, .4, TIMBER)
        k.box(0, -2.2, .4, 3.6, 4.4, .12, PRIMER)
        for s in (-1, 1): k.extrude([(s * 2.2, .4), (s * 2.5, 1.3), (s * 2.5, 2.9), (s * 2.38, 2.9), (s * 2.38, 1.3), (s * 2.1, .5)], 'yz', 0, 3.6, PRIMER)
        for xx in (.05, 1.75, 3.45):
            k.box(xx, -2.2, .5, .1, 4.4, .5, RUST)
            for s in (-1, 1): k.box(xx, s * 2.3 - .15, .5, .1, .3, 2.3, RUST)
        k.box(1.75, -2.3, .5, .08, 4.6, 1.6, PRIMER)

def naval_shipyard(k, w, d):
    # Shipbuilding berth, standing in the water on piles: a slipway with a coaster hull on the
    # ways, a level-luffing crane on its rails, the plate shop, offices, and a fitting-out quay.
    zq = 1.2
    quay(k, .5, .5, 29.0, 10.8, zq, fend='x')                  # landward deck
    quay(k, .5, 11.3, 8.3, 18.2, zq, fend='y')                 # crane pier
    quay(k, 20.4, 11.3, 9.1, 18.2, zq, fend='xy')              # fitting-out pier
    # Slipway between the piers: sheet-piled sides, an inclined slab into the water, launch ways.
    k.extrude([(11.3, 0), (28.6, 0), (11.3, 1.2)], 'yz', 8.8, 20.4, CONC_DARK)
    for xx in (8.8, 20.1): k.box(xx, 11.3, 0, .3, 18.0, .68, remat(RUST, 'corr'))
    for xx in (12.4, 16.4): k.extrude([(11.3, 1.2), (28.6, 0), (28.6, .14), (11.3, 1.34)], 'yz', xx, xx + .4, TIMBER)
    with k.at(14.6, 19.9, 1.2 * (28.6 - 19.9) / 17.3 + .87, yaw=-90, pitch=3.97, s=1.04): ship_hull(k)
    # Plate shop.
    sx, sy, sw, sd, eh = 2.0, 1.5, 15.0, 8.2, 7.4
    k.box(sx, sy, zq, sw, sd, eh, CLAD_GREEN); k.box(sx - .06, sy - .06, zq, sw + .12, sd + .12, 1.1, PRECAST)
    gable(k, sx, sy, zq + eh, sw, sd, 1.9, CLAD_GREY, ridge='x', wall=CLAD_GREEN)
    rooflights(k, sx, sy, zq + eh, sw, sd, 1.9, [sx + 1.6 + i * 3.2 for i in range(5)], lw=.9)
    k.box(sx + 1.0, sy + sd / 2 - .5, zq + eh + 1.8, sw - 2.0, 1.0, .45, GALV)
    k.box(sx - .05, sy + sd - .02, zq + eh - .9, sw + .1, .08, .6, TEAM); k.box(sx + sw - .02, sy - .05, zq + eh - .9, .08, sd + .1, .6, TEAM)
    with face(k, 'y+', sx, sy + sd, zq): sliding(k, 7.6, 6.4, 5.6, .45, c=CLAD_GREY); door(k, 1.4); windows(k, 6.4, 1, z0=3.6, h=1.3, margin=.9, bay=1.5)
    with face(k, 'x+', sx + sw, sy + sd, zq): windows(k, sd, 1, z0=4.0, h=1.4, margin=1.2, bay=1.5)
    for xx in (5.0, 13.0): vent(k, xx, sy + 2.2, zq + eh + .7, .35, 1.1)
    # Offices and stores.
    top = block(k, 18.8, 2.0, 9.6, 7.0, 6.8, PRECAST, plinth=CONC_DARK, floors=2, z=zq, band=TEAM)
    roof_plant(k, 19.2, 2.4, top, 7.0, 6.2, seed=9, density=1.8)
    with face(k, 'y+', 18.8, 9.0, zq): door(k, 4.2)
    antenna_mast(k, 27.4, 3.2, top, 5.0, w=.6, dishes=1)
    # Crane on its rails, swinging a shell plate over the hull.
    for xx in (2.2, 7.2): k.box(xx - .08, 11.6, zq, .16, 17.6, .12, IRON)
    portal_crane(k, 4.7, 20.2, zq + .1, -16, jib=16.5, luff=57, drop=12.0, side=1)
    for i in range(3): k.box(3.0 + i * .12, 12.2 + i * .1, zq + .1 + i * .14, 3.4, 2.4, .12, PRIMER if i % 2 else RUST)
    for yy in (12.5, 14.2): k.box(2.9, yy, zq, 3.7, .2, .1, TIMBER)
    for (xx, yy) in [(3.4, 26.0), (5.0, 26.8)]:
        k.rev((xx, yy, zq + .9), 'x', [(0, .9), (.1, .9), (.1, .45), (1.0, .45), (1.0, .9), (1.1, .9)], WOOD, n=14)
    tyres(k, 6.4, 28.6, zq, 3)
    # Fitting-out pier: a hull section, containers, a propeller, crates and drums.
    container(k, 22.0, 12.0, zq, rgb(60, 92, 120)); container(k, 22.0, 14.7, zq, rgb(140, 70, 50)); container(k, 22.3, 12.1, zq + 2.59, rgb(150, 150, 144))
    hull_block(k, 22.2, 21.2, zq)
    px, py = 26.6, 26.2
    k.cyl(px, py, zq, .55, .5, IRON, n=10); k.rev((px, py, zq + .5), 'z', [(0, .32), (.7, .22)], BRASS, n=10)        # spare propeller
    for i in range(4):
        a = i * pi / 2 + .3
        with k.at(px, py, zq + .8, yaw=degrees(a)): k.ball(1.0, 0, 0, .8, BRASS, ry=.48, rz=.08, n=10, k=4)
    barrels(k, 21.4, 17.8, zq, 3); crate(k, 22.0, 26.4, zq, 1.2); crate(k, 23.5, 26.8, zq, 1.0, c=OLIVE, yaw=12)
    for i in range(3): k.cyl(27.6 + i * .42, 18.2, zq, .15, 1.4, rgb(110, 60, 50) if i else rgb(70, 110, 84), n=8)
    for (xx, yy) in [(8.2, 13.5), (8.2, 21.5), (8.2, 28.6), (21.0, 13.5), (21.0, 21.5), (21.0, 28.6), (28.8, 16.0), (28.8, 22.5), (28.8, 28.6), (25.0, 28.9), (1.6, 28.9), (28.8, 5.5)]: mooring(k, xx, yy, zq)
    for (xx, yy) in [(1.2, 11.0), (28.9, 11.0), (20.9, 29.0), (28.9, 24.6)]:
        k.tube((xx, yy, zq), (xx, yy, zq + 6.0), .07, GALV, n=6); k.box(xx - .12, yy - .55, zq + 5.95, .24, .6, .1, DARK); lamp(k, xx, yy - .32, zq + 5.92, r=.1)
    flagpole(k, 17.8, 10.4, zq, 8.5); floodlight(k, 20.8, 11.7, zq, 9.0, yaw=135)

# --- Airforce command -------------------------------------------------------------------------------
def stand(k, x, y, z, r=2.7, c=WHITE, t=.32):
    """Helicopter stand: a painted ring and H, flat on the tarmac."""
    k.rev((x, y, z), 'z', [(0, r), (.012, r)], c, n=28)
    k.rev((x, y, z), 'z', [(0, r - t), (.02, r - t)], ASPHALT, n=28)
    s = r * .52
    for sx in (-1, 1): k.box(x + sx * s * .62 - t / 2, y - s * .85, z + .02, t, s * 1.7, .012, c)
    k.box(x - s * .62, y - t / 2, z + .02, s * 1.24, t, .012, c)

def radar_head(k, x, y, z, yaw, wd=4.6, ht=1.9):
    """Airfield surveillance radar: an orange parabolic-cylinder reflector with its feed horn and a
    secondary-radar bar, on a turning gear."""
    k.cyl(x, y, z, .35, .8, GALV, n=8); k.cbox(x, y, z + .8, 1.0, 1.0, .5, DARK)
    with k.at(x, y, z + 1.4, yaw=yaw):
        n = 8
        pts = [(.75 * (2 * i / n - 1) ** 2 - .4, -wd / 2 + wd * i / n) for i in range(n + 1)]
        for (xa, ya), (xb, yb) in zip(pts, pts[1:]):
            k.plate([(xa, ya, 0), (xb, yb, 0), (xb + .35, yb, ht), (xa + .35, ya, ht)], ORANGE)
        for i, (xa, ya) in enumerate(pts):
            if i % 2 == 0: k.strut((xa - .04, ya, 0), (xa + .31, ya, ht), .07, tint(ORANGE, .7))
        for (xa, ya), (xb, yb) in zip(pts, pts[1:]):
            k.strut((xa - .04, ya, 0), (xb - .04, yb, 0), .07, tint(ORANGE, .7)); k.strut((xa + .31, ya, ht), (xb + .31, yb, ht), .07, tint(ORANGE, .7))
        k.box(-.1, -wd * .36, ht + .12, .16, wd * .72, .28, WHITE)
        k.strut((-.3, 0, .1), (1.7, 0, .55), .08, STEEL); k.cbox(1.7, 0, .4, .3, .45, .3, DARK)
        k.box(-.9, -.35, -.1, .5, .7, .6, GALV)

def bowser(k, x, y, z, yaw=0.0):
    """Airfield fuel bowser on a 6x6 chassis."""
    with k.at(x, y, z, yaw=yaw):
        veh.truck(k, YELLOW, bed=False)
        k.rev((-3.6, 0, 2.3), 'x', [(0, .5), (.25, .98), (3.3, .98), (3.55, .5)], GALV, n=14)
        k.rev((-3.6, 0, 2.3), 'x', [(1.5, 1.0), (2.0, 1.0)], SIGNAL_RED, n=14)
        k.box(-3.6, -1.0, 1.25, 3.6, 2.0, .2, DARK); k.box(-2.4, -.3, 3.25, 1.0, .6, .14, DARK)
        k.box(-3.75, -1.15, 1.2, .12, 2.3, .5, HAZ)

def airforce_command(k, w, d):
    # Airbase headquarters: control tower with a slanted glazed cab and surveillance radar, the
    # operations block, an arched hangar, and an apron marked with four stands.
    z = pad(k, w, d)
    apron(k, .7, 14.4, 28.6, 14.9, z, ASPHALT)
    for sx_ in (4.6, 11.5, 18.4, 25.3):
        stand(k, sx_, 23.4, z + .03, 2.7)
        line(k, sx_ - .09, 17.0, .18, 3.4, z, YELLOW)
    line(k, 1.2, 16.85, 27.6, .2, z, YELLOW)
    line(k, 1.0, 28.85, 28.0, .15, z, WHITE); line(k, 1.0, 14.7, 28.0, .15, z, WHITE)
    for i in range(9): lamp(k, 1.3 + i * 3.42, 29.15, z + .2, PLASMA, .11)
    # Hangar: steel arch on knee walls, ribbed, sliding doors parted on the nose of an aircraft.
    hx, hy, hw, hd, hk, hr = 1.8, 1.5, 12.6, 12.0, 2.6, 5.2
    k.box(hx, hy, z, hw, hd, hk, CLAD_GREY); k.box(hx - .08, hy - .06, z, hw + .16, hd + .12, .9, PRECAST)
    vault(k, hx, hy, z + hk, hw, hd, hr, CLAD_GREY, ridge='y', n=14, ends=CLAD_SAND)
    for yy in (hy - .05, hy + 3.0, hy + 6.0, hy + 9.0, hy + hd - .6): vault(k, hx - .1, yy, z + hk, hw + .2, .3, hr + .1, IRON, ridge='y', n=14)
    vault(k, hx - .12, hy + hd - 1.5, z + hk, hw + .24, .8, hr + .12, TEAM, ridge='y', n=14)
    k.box(hx + hw / 2 - .6, hy + 1.0, z + hk + hr - .1, 1.2, hd - 2.8, .4, GALV)
    k.box(hx, hy + hd - .02, z, hw, .04, hk, CLAD_SAND)
    with face(k, 'y+', hx, hy + hd, z):
        sliding(k, 2.3, 8.0, 4.6, .42, c=CLAD_WHITE)
        k.box(4.8, .02, 5.6, 3.0, .06, 1.0, TEAM)
    k.rev((hx + hw / 2, hy + hd - 2.4, z + 2.2), 'y', [(0, .95), (1.8, .8), (3.0, .42), (3.7, .05)], AIR_GREY, n=12)
    k.ball(hx + hw / 2, hy + hd - 1.7, z + 3.0, .5, GLASS_DARK, ry=1.2, rz=.4, n=10, k=5)
    k.tube((hx + hw / 2, hy + hd + .3, z + 1.4), (hx + hw / 2, hy + hd + .3, z + .4), .06, STEEL, n=6); k.wheel(hx + hw / 2, hy + hd + .3, z + .3, .3, .2)
    # Operations block.
    top = block(k, 16.0, 2.4, 12.4, 9.4, 6.8, PRECAST, plinth=CONC_DARK, floors=2, band=TEAM)
    roof_plant(k, 16.4, 2.8, top, 7.4, 8.4, seed=12, density=1.2)
    with face(k, 'y+', 16.0, 11.8, z): door(k, 2.6, 1.4)
    # Control tower at the block's front corner.
    tx, ty, ts = 24.2, 7.9, 4.6
    k.box(tx, ty, z, ts, ts, 15.0, PRECAST); k.box(tx - .06, ty - .06, z, ts + .12, ts + .12, .6, CONC_DARK)
    for i in range(1, 5): k.box(tx - .02, ty - .02, z + i * 3.0, ts + .05, ts + .05, .07, tint(PRECAST, .78))
    k.box(tx + ts / 2 - .35, ty + ts - .02, z + 1.2, .7, .07, 12.8, GLASS_DARK); k.box(tx + ts - .02, ty + ts / 2 - .35, z + 1.2, .07, .7, 12.8, GLASS_DARK)
    with face(k, 'y+', tx, ty + ts, z): door(k, .5)
    cxm, cym = tx + ts / 2, ty + ts / 2
    k.stack(cxm, cym, [(z + 15.0, ts, ts, .1), (z + 16.3, 6.6, 6.6, 1.2)], PRECAST, k=2, smooth=False)
    k.rev((cxm, cym, z + 16.3), 'z', [(0, 3.9), (.2, 3.9)], CONC_DARK, n=8)
    k.rail([(cxm + 3.8 * cos(i * pi / 4), cym + 3.8 * sin(i * pi / 4), z + 16.5) for i in range(9)], GALV, posts=9, t=.045)
    k.rev((cxm, cym, z + 16.5), 'z', [(0, 2.7), (.8, 2.75)], WHITE, n=8)
    ring0 = [(cxm + 2.7 * cos(i * pi / 4), cym + 2.7 * sin(i * pi / 4), z + 17.3) for i in range(8)]
    ring1 = [(cxm + 3.35 * cos(i * pi / 4), cym + 3.35 * sin(i * pi / 4), z + 19.6) for i in range(8)]
    k.loft([ring0, ring1], GLASS_GREEN, smooth=False)
    for a, b in zip(ring0, ring1): k.strut(a, b, .09, WHITE)
    k.rev((cxm, cym, z + 19.6), 'z', [(0, 3.7), (.3, 3.7), (.55, 3.3)], WHITE, n=8)
    k.rev((cxm, cym, z + 19.6), 'z', [(-.02, 3.72), (.14, 3.72)], TEAM, n=8)
    radar_head(k, cxm - .4, cym - .4, z + 20.15, 35)
    for (dx, dy, hh) in [(2.2, 1.6, 3.0), (-2.0, 2.2, 2.4), (2.4, -1.8, 2.0)]: k.tube((cxm + dx, cym + dy, z + 20.1), (cxm + dx, cym + dy, z + 20.1 + hh), .04, STEEL, n=4)
    lamp(k, cxm + 2.2, cym + 1.6, z + 23.2, RED_LAMP, .13)
    # Bowser, ground power cart, wind sock, floodlights, flag.
    bowser(k, 19.4, 13.3, z, yaw=180)
    k.box(14.7, 14.9, z + .25, 1.6, 1.0, .9, YELLOW); k.box(14.6, 14.85, z + 1.15, 1.8, 1.1, .08, DARK)
    for xx in (15.0, 16.0): k.wheel(xx, 14.85, z + .25, .25, .16); k.wheel(xx, 15.95, z + .25, .25, .16)
    k.strut((14.7, 15.4, z + .5), (13.6, 15.6, z + .3), .05, DARK)
    wx_, wy_ = 1.5, 15.9
    k.tube((wx_, wy_, z), (wx_, wy_, z + 6.5), .08, WHITE, n=6); k.cbox(wx_, wy_, z, .6, .6, .2, CONC_DARK)
    for i, (t0, t1) in enumerate([(.1, .7), (.7, 1.3), (1.3, 1.9), (1.9, 2.5)]):
        k.rev((wx_, wy_, z + 6.3), (1, .35, -.16), [(t0, .46 - t0 * .1), (t1, .46 - t1 * .1)], ORANGE if i % 2 == 0 else WHITE, n=8, caps=False)
    floodlight(k, 1.2, 13.9, z, 10.0, yaw=50, heads=3); floodlight(k, 14.9, 14.1, z, 10.0, yaw=75, heads=3); floodlight(k, 29.0, 13.6, z, 10.0, yaw=110, heads=3)
    flagpole(k, 15.3, 12.4, z, 8.5)

# --- Service depot ----------------------------------------------------------------------------------
def gantry(k, x0, x1, y, z, h=6.4, c=YELLOW, hook_x=None):
    """Rail-mounted portal gantry: A-frame legs on bogies, box girder, hoist trolley and hook."""
    for xx in (x0, x1):
        for s in (-1, 1):
            k.strut((xx, y + s * 1.3, z + .45), (xx, y + s * .15, z + h), .24, c)
            k.cbox(xx, y + s * 1.3, z + .08, .5, .9, .4, DARK)
        k.box(xx - .12, y - 1.3, z + .45, .24, 2.6, .22, c)
        k.strut((xx, y - .68, z + h * .5), (xx, y + .68, z + h * .5), .14, c)
    k.box(x0 - .4, y - .28, z + h, x1 - x0 + .8, .56, .7, c)
    for i in range(6): k.box(x0 - .4 + i * (x1 - x0 + .5) / 5, y + .28, z + h + .05, .3, .02, .6, BLACK if i % 2 else HAZ)
    hx = hook_x or (x0 + x1) / 2
    k.cbox(hx, y, z + h - .5, .9, .8, .5, DARK)
    k.strut((hx, y, z + h - .5), (hx, y, z + h - 2.6), .04, STEEL); k.cbox(hx, y, z + h - 2.95, .3, .16, .38, c)

def service_depot(k, w, d):
    # Vehicle repair depot: an open-fronted workshop with a tank hull stripped down in one bay, a
    # marked repair pad under a rail gantry, a bunded fuel tank, parts racks, tyres and drums.
    z = pad(k, w, d)
    # Repair pad: flat and empty, a vehicle parks here.
    px, py, pw, pd = 5.5, 9.0, 9.6, 10.0
    apron(k, px, py, pw, pd, z, ASPHALT)
    for (ax, ay, aw, ad) in [(px, py, pw, .45), (px, py + pd - .45, pw, .45), (px, py, .45, pd), (px + pw - .45, py, .45, pd)]: hazard_edge(k, ax, ay, aw, ad, z)
    for (ax, ay) in [(px + 1.3, py + 1.3), (px + pw - 3.3, py + 1.3), (px + 1.3, py + pd - 1.5), (px + pw - 3.3, py + pd - 1.5)]: line(k, ax, ay, 2.0, .18, z)
    for (ax, ay) in [(px + 1.3, py + 1.3), (px + pw - 1.5, py + 1.3), (px + 1.3, py + pd - 3.3), (px + pw - 1.5, py + pd - 3.3)]: line(k, ax, ay, .18, 2.0, z)
    for xx in (4.9, 15.7): k.box(xx - .07, 8.0, z, .14, 11.5, .1, IRON)
    gantry(k, 4.9, 15.7, 9.5, z + .1, 6.6, hook_x=12.6)
    # Workshop: mono-pitch roof on steel posts, two open bays and an enclosed parts store.
    wx, wy, ww, wd_, hf, hb = 1.5, 1.5, 14.2, 6.0, 6.6, 5.6
    k.box(wx, wy, z, ww, .2, hb, CLAD_GREY); k.box(wx, wy, z, ww, wd_, .08, rgb(112, 120, 112, 'concrete'))
    for xx in (wx, wx + ww - .2): k.extrude([(wy, z), (wy + wd_, z), (wy + wd_, z + hf - .06), (wy, z + hb)], 'yz', xx, xx + .2, CLAD_GREY)
    k.box(wx + ww - .26, wy - .05, z, .3, wd_ + .1, 1.0, PRECAST)
    k.box(wx + .2, wy + .2, z, 9.2, .05, hb - .6, OFFWHITE)
    k.extrude([(wy - .3, z + hb - .05), (wy + wd_ + .6, z + hf), (wy + wd_ + .6, z + hf + .16), (wy - .3, z + hb + .11)], 'yz', wx - .3, wx + ww + .3, CLAD_DARK)
    zr = lambda y: z + hb + (hf - hb) * (y - wy + .3) / (wd_ + .9) + .14
    for xx in (wx + 1.6, wx + 6.4, wx + 11.2):
        k.plate([(xx, wy + 1.0, zr(wy + 1.0)), (xx + 1.4, wy + 1.0, zr(wy + 1.0)), (xx + 1.4, wy + 4.6, zr(wy + 4.6)), (xx, wy + 4.6, zr(wy + 4.6))], SKYLIGHT)
    vent(k, wx + 4.4, wy + 2.0, zr(wy + 2.0) - .1, .3, .9); vent(k, wx + 9.2, wy + 2.0, zr(wy + 2.0) - .1, .3, .9)
    k.cyl(wx + 13.2, wy + 1.2, z + hb, .2, 2.6, GALV, n=8); k.rev((wx + 13.2, wy + 1.2, z + hb + 2.6), 'z', [(0, .32), (.2, .1)], GALV, n=8)
    for xx in (wx, wx + 4.7, wx + 9.4, wx + ww - .25): k.box(xx, wy + wd_ - .25, z, .25, .25, hf - .1, IRON)
    k.box(wx, wy + wd_ - .2, z + hf - 1.1, ww, .22, 1.0, CLAD_GREY); k.box(wx - .02, wy + wd_ + .02, z + hf - .85, ww + .04, .06, .5, TEAM)
    k.box(wx + 9.4, wy, z, .15, wd_, hb, CLAD_GREY)                                   # store
    k.box(wx + 9.4, wy + wd_ - .2, z, ww - 9.4, .2, hf - 1.1, CLAD_GREY)
    with face(k, 'y+', wx + 9.4, wy + wd_, z): shutter(k, .7, 2.6, 3.4, open_=0.0, c=TEAM); door(k, 3.65, .9)
    with face(k, 'x+', wx + ww, wy + wd_, z): windows(k, wd_, 1, z0=2.0, h=1.3, margin=1.0, bay=1.2)
    for xx in (wx + 2.3, wx + 7.0): k.box(xx - .7, wy + wd_ - .55, z + hf - 1.2, 1.4, .2, .08, LAMP)
    # In the bays: tool chests, a bench, an engine on a stand; a tank hull with its turret lifted.
    k.box(2.0, 6.5, z, 1.6, .7, 1.3, SIGNAL_RED); k.box(2.0, 6.5, z + 1.3, 1.6, .7, .05, DARK); k.box(1.9, 5.0, z, .7, .9, 1.0, SIGNAL_RED)
    k.box(1.8, 2.0, z + .85, 3.6, .8, .08, WOOD)
    for xx in (1.9, 5.2): k.box(xx, 2.1, z, .1, .6, .85, STEEL)
    k.box(4.2, 6.3, z + .5, 1.2, .8, .8, IRON); k.box(4.4, 6.4, z + 1.3, .8, .6, .25, ALU)
    for xx in (4.1, 5.5): k.strut((xx, 6.7, z), (xx, 6.7, z + .6), .08, YELLOW)
    with k.at(8.55, 5.7, z + .08, yaw=90, s=.7):
        veh.tank_hull(k, 7.9, 3.6, .62, NATO_SAND, deck=1.22, glacis=2.2, nose=.8); veh.tracks(k, -3.95, 3.95, 3.6, .62, 7, .32, .95, NATO_SAND)
        k.cyl(-.3, 0, 1.22, 1.05, .06, DARK, n=16)
    tyres(k, 6.3, 7.9, z, 2, .45, .18)
    # Fuel tank in its bund, dispenser.
    for (bx, by, bw, bd) in [(16.4, 1.6, 3.3, .25), (16.4, 8.0, 3.3, .25), (16.4, 1.6, .25, 6.6), (19.45, 1.6, .25, 6.65)]: k.box(bx, by, z, bw, bd, .7, CONCRETE)
    htank(k, 18.0, 2.3, z + .05, 5.2, 1.05, WHITE, axis='y', band=TEAM)
    k.ladder(18.0, 7.56, z, z + 2.7, axis='x'); k.box(17.6, 4.6, z + 2.65, .8, .6, .25, STEEL)
    k.box(16.8, 9.0, z, .8, .5, 1.5, SIGNAL_RED); k.box(16.85, 9.5, z + .9, .7, .03, .45, WHITE)
    k.pipe([(17.2, 8.3, z + .3), (17.2, 8.9, z + .3)], .07, STEEL)
    k.pipe([(17.6, 9.25, z + 1.2), (18.1, 9.6, z + .5), (18.6, 9.9, z + .05)], .04, RUBBER)
    # East side: truck tyres, drums, crates of spares.
    tyres(k, 17.6, 11.8, z, 4, .52, .2); tyres(k, 18.7, 12.6, z, 3, .52, .2); tyres(k, 17.5, 13.2, z, 2, .52, .2)
    barrels(k, 17.2, 14.9, z, 3, rows=2)
    crate(k, 17.2, 16.7, z, 1.3, c=OLIVE); crate(k, 18.4, 18.0, z, 1.0)
    # West side: parts racking, compressor, gas bottles.
    for zz in (.3, 1.3, 2.3): k.box(1.4, 8.6, z + zz, 1.1, 5.0, .07, GALV)
    for yy in (8.6, 11.1, 13.55):
        for xx in (1.4, 2.45): k.box(xx, yy, z, .06, .06, 3.1, rgb(60, 84, 110))
    rnd = random.Random(21)
    for zz in (.37, 1.37, 2.37):
        yy = 8.8
        while yy < 13.0:
            L = rnd.uniform(.5, 1.0); k.box(1.5, yy, z + zz, .9, L, rnd.uniform(.35, .7), rnd.choice([WOOD, OLIVE, rgb(150, 132, 100), IRON, WOOD])); yy += L + rnd.uniform(.1, .5)
    htank(k, 2.6, 14.6, z - .25, 2.2, .45, rgb(60, 84, 110), axis='y')
    k.box(2.2, 15.0, z + 1.1, .8, 1.0, .55, DARK); k.wheel(2.6, 16.9, z + .3, .3, .16)
    for i in range(3): k.cyl(1.7 + i * .42, 17.8, z, .15, 1.4, rgb(110, 60, 50) if i else rgb(70, 110, 84), n=8); k.cyl(1.7 + i * .42, 17.8, z + 1.4, .06, .15, BRASS, n=6)
    k.box(1.5, 17.62, z, 1.3, .06, 1.0, STEEL)
    tyres(k, 3.6, 8.7, z, 3); tyres(k, 3.0, 9.6, z, 2)
    flagpole(k, 1.6, 19.2, z, 8.0, w=2.2); floodlight(k, 19.2, 19.2, z, 8.0, yaw=215); floodlight(k, 4.0, 7.9, z, 8.5, yaw=60)

MODELS = {f.__name__: f for f in [construction_yard, refinery, tech_center, ore_purifier, naval_shipyard,
                                  airforce_command, service_depot]}
