"""Ambient life and civilian vehicles for the redesigned maps, modelled in metres like the roster.

Nothing here wears team paint: these are presentation-only wanderers and parked scenery, so every
colour is a neutral material, and saturated colours are chosen to stay outside the two team-paint
rules of presentation.rs / finish_hd_art.py (blue ramp b > r+60 and g > r+35; red-dominant
r > g+65 and r > b+70). finish_map_art.py nudges any rendered pixel that still drifts into them.

  VEHICLES  civilian cars, trucks and boats: `vehicle(k, ident)` at vehicle scale (U = 0.125 cells
            per metre, as hd_vehicles.py: a 4.6 m car is 0.57 cell long); x forward, waterline or
            road at z = 0. Rendered as eight-facing sheets like hd_units (render_map_art.py u:<id>)
            and parked on cells in terrain atlas 29 (hd_map_art.py).
  WALKERS   civilians and animals at infantry scale (hd_infantry.U = 0.38) with an eight-frame walk
            cycle driven by hd_infantry.POSE, and a seagull's wing-flap cycle.
"""
from math import sin, cos, pi, radians, sqrt
from hd_kit import *
import hd_infantry as inf

U_CAR = .125      # cells per metre, the scale of every ground vehicle (hd_vehicles.U)
U_FERRY = .045    # a ferry is shortened like the warships of hd_naval.py
U_GULL = .2       # a gull of ~1.3 m wingspan spans ~10 world px

# --- Civilian paint (all outside the team-paint rules) -----------------------------------------
MAROON = rgb(98, 40, 54); SILVER = rgb(170, 172, 170); TAXI = rgb(214, 172, 40)
PICKUP_GREEN = rgb(66, 92, 58); VAN_WHITE = rgb(212, 212, 206); BUS_ORANGE = rgb(208, 134, 48)
BUS_CREAM = rgb(222, 212, 182); TRUCK_BOX = rgb(196, 198, 192); TRUCK_CAB = rgb(70, 84, 92)
TRACTOR_GREEN = rgb(64, 104, 52); TRACTOR_RIM = rgb(212, 176, 52); CHROME = C((200, 204, 206), 'steel')
TAIL = rgb(150, 52, 52, 'emit')   # tail lamps: small, emissive, nudged out of the red rule later
HULL_WHITE = rgb(220, 220, 214); HULL_GREEN = rgb(48, 74, 62); HULL_SLATE = rgb(56, 64, 74)
DECK_TEAK = rgb(150, 112, 74, 'wood'); SAILCLOTH = rgb(226, 224, 214, 'cloth')
NET = rgb(84, 92, 70, 'cloth'); FLOAT_ORANGE = rgb(220, 156, 52); FUNNEL = rgb(206, 170, 76)

def wheels(k, xs, W, r, w=.22, hub=CHROME):
    for x in xs:
        for s in (-1, 1): k.wheel(x, s * (W / 2 - w / 2 - .02), r, r, w, tyre=TYRE, hub=hub)

def lamps(k, L, W, z, front=True, rear=True):
    for s in (-1, 1):
        if front: k.cbox(L / 2 + .005, s * (W / 2 - .22), z, .03, .26, .12, LAMP)
        if rear: k.cbox(-L / 2 - .005, s * (W / 2 - .2), z, .03, .24, .14, TAIL)

def greenhouse(k, x0, x1, xt0, xt1, W, z0, z1, body, inset=.12):
    """Cabin of a car: dark glass loft from the beltline to the roof, a body-coloured roof and
    pillars. x0..x1 at the beltline, xt0..xt1 under the roof."""
    w0, w1 = W / 2 - .04, W / 2 - inset - .1
    ring0 = [(x0, -w0, z0), (x1, -w0, z0), (x1, w0, z0), (x0, w0, z0)]
    ring1 = [(xt0, -w1, z1), (xt1, -w1, z1), (xt1, w1, z1), (xt0, w1, z1)]
    k.loft([ring0, ring1], GLASS, smooth=False)
    k.box(xt0 - .03, -w1 - .02, z1, xt1 - xt0 + .06, 2 * w1 + .04, .06, body)
    for s in (-1, 1):
        k.strut((x1, s * w0, z0), (xt1, s * w1, z1), .07, body)   # A pillar
        k.strut((x0, s * w0, z0), (xt0, s * w1, z1), .09, body)   # C pillar
        k.strut(((x0 + x1) / 2, s * w0, z0), ((xt0 + xt1) / 2, s * w1, z1), .05, body)

def car_body(k, L, W, belt, nose, tail, body, clear=.28):
    """Lower body: a side profile extruded across the car, slightly tucked at the sills."""
    prof = [(-L / 2, clear + .12), (-L / 2 + .15, clear), (L / 2 - .15, clear), (L / 2, clear + .12),
            (L / 2, nose), (L / 2 - .25, belt - .02), (L * .15, belt), (-L / 2 + .2, belt), (-L / 2, tail)]
    k.extrude(prof, 'xz', -W / 2, W / 2, body)
    k.cbox(L / 2 - .02, 0, clear + .05, .1, W * .96, .18, rgb(40, 40, 42, 'rubber'))     # bumpers
    k.cbox(-L / 2 + .02, 0, clear + .05, .1, W * .96, .18, rgb(40, 40, 42, 'rubber'))
    k.cbox(L / 2 + .01, 0, clear + .26, .02, W * .4, .12, DARK)                         # grille

def sedan(k, body=MAROON, L=4.6, W=1.8, H=1.44, compact=False):
    r = .31
    wheels(k, (-L * .31, L * .3), W, r)
    belt = .9
    car_body(k, L, W, belt, .72, .86, body)
    if compact:   # hatchback: short bonnet, cabin to the tail
        greenhouse(k, -L / 2 + .12, L * .18, -L / 2 + .3, -L * .02, W, belt, H, body)
    else:
        greenhouse(k, -L * .3, L * .14, -L * .2, -L * .02, W, belt, H, body)
    lamps(k, L, W, .66)
    for s in (-1, 1): k.box(L * .1, s * (W / 2 + .03) - .03, belt - .05, .1, .06, .07, body)   # mirrors

def taxi(k):
    sedan(k, TAXI)
    k.cbox(-.4, 0, 1.47, .5, .26, .2, rgb(230, 228, 214, 'paint'))   # roof sign (no lettering)
    k.cbox(-.4, 0, 1.52, .44, .28, .08, rgb(255, 230, 170, 'emit'))
    for s in (-1, 1): k.box(-1.3, s * .9 - .01, .6, 2.4, .02, .06, rgb(40, 40, 40))  # side stripe

def pickup(k, body=PICKUP_GREEN):
    L, W, r = 5.3, 1.95, .38
    wheels(k, (-L * .3, L * .3), W, r, .26)
    belt = 1.08
    car_body(k, L, W, belt, .92, 1.02, body, clear=.38)
    greenhouse(k, -L * .1, L * .17, -L * .07, L * .06, W, belt, 1.82, body)
    # Open bed with its walls and tailgate.
    x0, x1 = -L / 2, -L * .12
    k.box(x0 + .1, -W / 2 + .1, belt - .02, x1 - x0 - .15, W - .2, .04, rgb(52, 52, 50, 'rubber'))
    for s in (-1, 1): k.box(x0, s * (W / 2 - .04) - .04, belt, x1 - x0, .08, .38, body)
    k.box(x0, -W / 2, belt, .08, W, .38, body); k.box(x1 - .08, -W / 2, belt, .08, W, .45, body)
    lamps(k, L, W, .88)

def van(k, body=VAN_WHITE):
    L, W, H, r = 5.4, 2.0, 2.4, .36
    wheels(k, (-L * .32, L * .32), W, r, .24)
    prof = [(-L / 2, .3), (L / 2 - .1, .3), (L / 2, .5), (L / 2, .95), (L / 2 - .7, 1.25), (L / 2 - 1.15, H - .05),
            (L / 2 - 1.3, H), (-L / 2 + .05, H), (-L / 2, H - .05)]
    k.extrude(prof, 'xz', -W / 2, W / 2, body)
    e = .012
    k.plate([(L / 2 - .7 + e, -W / 2 + .12, 1.3), (L / 2 - .7 + e, W / 2 - .12, 1.3),
             (L / 2 - 1.13 + e, W / 2 - .14, H - .12), (L / 2 - 1.13 + e, -W / 2 + .14, H - .12)], GLASS)
    for s in (-1, 1):
        y = s * (W / 2 + e)
        k.plate([(L / 2 - 1.9, y, 1.3), (L / 2 - .75, y, 1.3), (L / 2 - 1.15, y, H - .2), (L / 2 - 1.9, y, H - .2)], GLASS)
        k.box(-L / 2 + .4, s * (W / 2 + e) - .005, 1.0, 3.0, .01, .08, rgb(96, 100, 100, 'paint'))   # rub strip
    k.cbox(L / 2 - .02, 0, .3, .12, W * .96, .2, rgb(40, 40, 42, 'rubber'))
    k.cbox(-L / 2 + .02, 0, .3, .12, W * .96, .2, rgb(40, 40, 42, 'rubber'))
    k.cbox(-L / 2 - .006, 0, .6, .01, .02, 1.5, rgb(90, 90, 90, 'dark'))   # rear door seam
    k.box(-L / 2 + .5, -.6, H, 2.6, .08, .06, STEEL); k.box(-L / 2 + .5, .52, H, 2.6, .08, .06, STEEL)  # roof bars
    lamps(k, L, W, .8)

def bus(k):
    L, W, H, r = 10.5, 2.5, 3.0, .5
    wheels(k, (-L * .27, L * .3), W, r, .3)
    with k.grp():
        k.stack(0, 0, [(.32, L, W, .2), (1.25, L, W, .22), (H - .25, L, W, .22), (H, L - .2, W - .2, .15)], BUS_ORANGE, k=2)
    k.stack(0, 0, [(1.25, L + .01, W + .01, .22), (H - .25, L + .01, W + .01, .22), (H + .002, L - .19, W - .19, .15)], BUS_CREAM, k=2)
    e = .02
    for s in (-1, 1):   # window band
        y = s * (W / 2 + e)
        for i in range(7):
            x = -L / 2 + .5 + i * 1.32
            k.plate([(x, y, 1.4), (x + 1.18, y, 1.4), (x + 1.18, y, 2.5), (x, y, 2.5)], GLASS)
    k.plate([(L / 2 + e, -W / 2 + .1, 1.0), (L / 2 + e, W / 2 - .1, 1.0), (L / 2 + e, W / 2 - .1, 2.55), (L / 2 + e, -W / 2 + .1, 2.55)], GLASS)
    k.plate([(L / 2 + e + .005, -W / 2 + .35, 2.6), (L / 2 + e + .005, W / 2 - .35, 2.6), (L / 2 + e + .005, W / 2 - .35, 2.85), (L / 2 + e + .005, -W / 2 + .35, 2.85)], rgb(30, 30, 30, 'dark'))
    k.plate([(-L / 2 - e, -W / 2 + .2, 1.6), (-L / 2 - e, W / 2 - .2, 1.6), (-L / 2 - e, W / 2 - .2, 2.4), (-L / 2 - e, -W / 2 + .2, 2.4)], GLASS)
    k.cbox(0, 0, H, 2.6, 1.6, .32, rgb(190, 190, 184, 'paint'))        # roof air conditioning
    for i in range(3): k.cbox(-.8 + i * .8, 0, H + .32, .5, 1.2, .03, DARK)
    k.box(L / 2 - 1.6, W / 2 + e - .005, .35, .9, .01, 2.3, rgb(60, 60, 58, 'dark'))     # door
    k.box(-.5, W / 2 + e - .005, .35, .9, .01, 2.3, rgb(60, 60, 58, 'dark'))
    lamps(k, L, W, .7)

def box_truck(k):
    L, W, r = 7.6, 2.4, .48
    wheels(k, (-L * .26, L * .32), W, r, .3)
    k.box(-L / 2, -W * .3, .45, L - .3, W * .6, .3, DARK)                          # chassis
    cx0 = L / 2 - 2.1
    k.extrude([(cx0, .55), (L / 2, .55), (L / 2, 1.5), (L / 2 - .35, 2.6), (cx0, 2.7)], 'xz', -W / 2 + .05, W / 2 - .05, TRUCK_CAB)
    e = .012
    k.plate([(L / 2 + e - .03, -W / 2 + .2, 1.6), (L / 2 + e - .03, W / 2 - .2, 1.6), (L / 2 - .33 + e, W / 2 - .2, 2.5), (L / 2 - .33 + e, -W / 2 + .2, 2.5)], GLASS)
    for s in (-1, 1):
        k.plate([(cx0 + .6, s * (W / 2 - .05 + e), 1.6), (L / 2 - .2, s * (W / 2 - .05 + e), 1.6), (L / 2 - .4, s * (W / 2 - .05 + e), 2.45), (cx0 + .6, s * (W / 2 - .05 + e), 2.45)], GLASS)
    k.cbox(L / 2 + .02, 0, .5, .12, W * .95, .25, DARK)
    k.box(-L / 2, -W / 2, .78, L - 2.2, W, 2.8, TRUCK_BOX)                       # cargo box
    k.box(-L / 2 - .01, -W / 2 + .1, .85, .02, W - .2, 2.6, rgb(150, 152, 148, 'corr'))  # roller door
    for i in range(6): k.box(-L / 2 + .3 + i * .95, -W / 2 - .01, .78, .05, W + .02, 2.8, rgb(170, 172, 166, 'steel'))
    lamps(k, L, W, .9)

def tractor(k):
    W = 2.0
    for s in (-1, 1):
        k.wheel(-.9, s * .82, .82, .82, .45, tyre=TYRE, hub=TRACTOR_RIM)
        k.wheel(1.25, s * .7, .45, .45, .28, tyre=TYRE, hub=TRACTOR_RIM)
    k.box(-.6, -.38, .6, 2.3, .76, .5, DARK)                                       # chassis
    k.loft([[(-.2, -.42, .95), (2.0, -.38, .95), (2.0, .38, .95), (-.2, .42, .95)],
            [(-.2, -.36, 1.65), (1.95, -.3, 1.5), (1.95, .3, 1.5), (-.2, .36, 1.65)]], TRACTOR_GREEN, smooth=False)  # bonnet
    k.cbox(2.02, 0, 1.0, .04, .5, .45, DARK)
    k.tube((1.2, -.25, 1.6), (1.2, -.25, 2.5), .06, DARK, n=8)                      # exhaust
    for s in (-1, 1):                                                              # rear fenders
        k.extrude([(-1.75, 1.2), (-1.5, 1.7), (-.3, 1.7), (-.05, 1.2)], 'xz', s * .55, s * 1.08, TRACTOR_GREEN)
    k.box(-1.5, -.6, .95, 1.3, 1.2, .3, TRACTOR_GREEN)                             # cab floor
    with k.grp():
        for x in (-1.5, -.25):
            for s in (-1, 1): k.strut((x, s * .58, 1.25), (x + (.05 if x > -1 else 0), s * .55, 2.75), .07, DARK)
    k.loft([[(-1.48, -.56, 1.4), (-.27, -.56, 1.4), (-.27, .56, 1.4), (-1.48, .56, 1.4)],
            [(-1.48, -.54, 2.65), (-.22, -.54, 2.65), (-.22, .54, 2.65), (-1.48, .54, 2.65)]], GLASS, smooth=False)
    k.box(-1.6, -.66, 2.65, 1.5, 1.32, .14, TRACTOR_GREEN)                         # roof
    k.cbox(-.9, 0, 1.35, .45, .5, .35, BLACK)                                      # seat
    k.cbox(-1.9, 0, .7, .3, 1.2, .25, DARK)                                        # hitch

# --- Boats --------------------------------------------------------------------------------------
def hull(k, L, B, F, c, deck=None, bow_sheer=.35, transom=.8, draft=.3, n=10, stations=9, fine=1.6):
    """Planing/displacement hull from the transom (-L/2) to the stem (+L/2): U-shaped sections of
    half-beam b(x), freeboard F rising to the bow, cut 0.3 m below the waterline (z = 0)."""
    rings = []
    for i in range(stations + 1):
        t = i / stations
        x = -L / 2 + L * t
        b = B / 2 * (transom + (1 - transom) * sin(pi * min(1, t / .55) / 2)) if t < .55 else B / 2 * cos((t - .55) / .45 * pi / 2) ** fine
        b = max(b, .02)
        f = F + bow_sheer * t * t
        ring = []
        for j in range(n + 1):
            a = pi * j / n
            y = b * cos(a)
            z = f - (f + draft) * sin(a) ** .5
            ring.append((x, y, z))
        rings.append(ring)
    k.loft(rings, c, smooth=True, cap0=True, cap1=False)
    if deck:   # a deck plate just inside the sheer
        pts = [(r[0][0], r[0][1] * .94, r[0][2] - .02) for r in rings] + [(r[-1][0], r[-1][1] * .94, r[-1][2] - .02) for r in rings[::-1]]
        k.plate(pts, deck)
    # Boot top: a dark band at the waterline.
    band = []
    for i in range(stations + 1):
        r = rings[i]
        band.append(r)
    return rings

def sailboat(k):
    L, B = 9.0, 2.9
    hull(k, L, B, .9, HULL_WHITE, DECK_TEAK)
    k.stack(-.6, 0, [(.85, 2.8, 1.7, .4), (1.45, 2.5, 1.5, .35)], HULL_WHITE, k=2)   # coachroof
    for s in (-1, 1): k.box(-1.6, s * .76 - .02, 1.15, 1.8, .04, .18, GLASS)
    k.tube((.4, 0, 1.4), (.4, 0, 12.0), .07, CHROME, n=8)                            # mast
    k.tube((.3, 0, 2.6), (-3.4, 0, 2.6), .06, CHROME, n=6)                           # boom
    k.plate([(.32, 0, 2.7), (-3.3, 0, 2.7), (.32, 0, 11.6)], SAILCLOTH)             # mainsail
    k.plate([(.55, .02, 2.0), (4.3, .02, 1.4), (.55, .02, 10.8)], SAILCLOTH)        # jib
    k.tube((.4, 0, 12.0), (4.4, 0, 1.2), .015, DARK, n=4)                            # forestay
    k.tube((.4, 0, 12.0), (-4.3, 0, 1.0), .015, DARK, n=4)
    k.rail([(-4.2, .9, 1.0), (1.5, 1.3, 1.1), (3.6, .5, 1.2)], CHROME, h=.6, t=.02, posts=1.4)
    k.rail([(-4.2, -.9, 1.0), (1.5, -1.3, 1.1), (3.6, -.5, 1.2)], CHROME, h=.6, t=.02, posts=1.4)

def fishing_boat(k):
    L, B = 12.0, 4.0
    hull(k, L, B, 1.3, HULL_GREEN, rgb(120, 110, 94, 'wood'), bow_sheer=.8, transom=.85)
    k.box(-L / 2 + .3, -B / 2 + .2, 1.0, L - 3.0, .1, .5, HULL_WHITE)                 # bulwarks
    k.box(-L / 2 + .3, B / 2 - .3, 1.0, L - 3.0, .1, .5, HULL_WHITE)
    k.box(.8, -1.2, 1.25, 2.6, 2.4, 2.1, HULL_WHITE)                                 # wheelhouse
    k.box(.75, -1.25, 3.35, 2.7, 2.5, .14, rgb(70, 74, 76, 'paint'))
    k.plate([(3.41, -1.0, 2.2), (3.41, 1.0, 2.2), (3.41, 1.0, 3.05), (3.41, -1.0, 3.05)], GLASS)
    for s in (-1, 1): k.plate([(1.0, s * 1.21, 2.25), (3.2, s * 1.21, 2.25), (3.2, s * 1.21, 3.0), (1.0, s * 1.21, 3.0)], GLASS)
    k.tube((2.0, 0, 3.5), (2.0, 0, 7.5), .08, CHROME, n=6); k.cbox(2.0, 0, 6.2, .1, 1.6, .06, CHROME)
    k.tube((-.4, 0, 1.2), (-.4, 0, 6.2), .1, rgb(70, 74, 76, 'paint'), n=8)         # gantry mast
    k.tube((-.4, 0, 5.8), (-4.5, 0, 2.5), .06, rgb(70, 74, 76, 'paint'), n=6)       # derrick
    k.tube((-4.5, 0, 2.5), (-4.5, 0, 1.5), .015, DARK, n=4)
    k.cyl(-1.6, 0, 1.3, .7, .9, NET, n=12)                                           # net drum
    k.ball(-3.6, .6, 1.5, .7, NET, ry=.6, rz=.35, n=10, k=4)                          # heaped net
    for i in range(5): k.ball(-4.8 + i * .5, -1.3, 1.6, .16, FLOAT_ORANGE, n=8, k=4)  # floats
    k.cbox(-2.7, .9, 1.25, 1.0, .8, .6, rgb(150, 154, 150, 'paint'))                  # fish boxes

def ferry(k):
    L, B = 36.0, 9.0
    hull(k, L, B, 2.6, HULL_SLATE, rgb(110, 112, 108, 'concrete'), bow_sheer=.8, transom=.95, fine=1.2)
    with k.grp():
        k.stack(-1.5, 0, [(2.5, 26, 8.4, 1.2), (5.0, 26, 8.4, 1.2)], HULL_WHITE, k=2)   # car deck sides
    k.stack(-2.5, 0, [(5.0, 20, 7.6, 1.5), (7.6, 20, 7.6, 1.5)], HULL_WHITE, k=2)       # passenger deck
    k.stack(-4.0, 0, [(7.6, 11, 6.4, 1.2), (10.0, 11, 6.4, 1.2)], HULL_WHITE, k=2)      # bridge deck
    e = .03
    for s in (-1, 1):
        k.box(-14.4, s * (4.2 + e) - .01, 3.2, 25.0, .02, .5, rgb(74, 80, 84, 'dark'))  # car deck openings
        for i in range(16):
            x = -11.8 + i * 1.2
            k.box(x, s * (3.8 + e) - .01, 5.7, .95, .02, 1.3, GLASS)
        for i in range(8):
            x = -9.0 + i * 1.2
            k.box(x, s * (3.2 + e) - .01, 8.3, .95, .02, 1.2, GLASS)
    k.box(6.9, -2.8, 8.2, .05, 5.6, 1.4, GLASS)                                       # bridge windows
    k.box(7.48, -3.6, 5.6, .05, 7.2, 1.4, GLASS)
    for s in (-1, 1): k.box(-8.0, s * 3.0 - .3, 10.0, 2.0, .6, 2.6, FUNNEL)           # twin funnels
    for s in (-1, 1): k.box(-8.05, s * 3.0 - .35, 12.4, 2.1, .7, .3, BLACK)
    k.tube((-1.0, 0, 10.0), (-1.0, 0, 14.0), .12, CHROME, n=6); k.cbox(-1.0, 0, 13.2, .1, 2.2, .1, CHROME)
    for i in range(3):                                                                # lifeboats
        for s in (-1, 1): k.ball(-11 + i * 4, s * 3.85, 7.9, 1.6, rgb(222, 162, 56), ry=.6, rz=.5, n=10, k=4)
    k.box(13.0, -2.5, 2.6, 3.0, 5.0, .2, rgb(104, 106, 102, 'steel'))                 # bow ramp
    for x in (-10, -6, -2, 2):   # parked cars on the open car deck aft
        pass

# --- Dispatch -----------------------------------------------------------------------------------
VEHICLES = {
    'car_sedan': (U_CAR, lambda k: sedan(k, MAROON)),
    'car_compact': (U_CAR, lambda k: sedan(k, SILVER, L=3.9, W=1.72, H=1.48, compact=True)),
    'taxi': (U_CAR, taxi),
    'pickup': (U_CAR, pickup),
    'van': (U_CAR, van),
    'bus': (U_CAR, bus),
    'truck': (U_CAR, box_truck),
    'tractor': (U_CAR, tractor),
    'sailboat': (U_CAR, sailboat),
    'fishing_boat': (U_CAR, fishing_boat),
    'ferry': (U_FERRY, ferry),
}
SHEETS = ['car_sedan', 'car_compact', 'taxi', 'pickup', 'bus', 'truck', 'tractor', 'sailboat', 'fishing_boat', 'ferry']

def vehicle(m, ident):
    u, fn = VEHICLES[ident]
    fn(Kit(m, u))

# =================================================================================================
# Walkers: civilians on the infantry figure of hd_infantry.py, animals on a shared quadruped rig.
SKIN_A = rgb(196, 156, 124, 'skin'); SKIN_B = rgb(150, 108, 80, 'skin'); SKIN_C = rgb(214, 180, 150, 'skin')
SHOE = rgb(44, 36, 32, 'leather')

def free_arms(k, c, skin, amp=1.0):
    for s in (1, -1):
        with inf.swinging(k, s, amp): inf.arm(k, s, (.0, s * .28, 1.14), (.06, s * .27, .87), c, skin)

def civ_man(k):
    jacket = rgb(128, 98, 66, 'cloth'); trousers = rgb(70, 68, 64, 'cloth')
    inf.legs(k, trousers, SHOE, spread=.1)
    inf.torso(k, jacket)
    k.box(.1, -.05, 1.25, .03, .1, .18, rgb(196, 192, 180, 'cloth'))                  # shirt collar
    zc = inf.head(k, SKIN_A); inf.hair(k, inf.HAIR_DARK, zc)
    free_arms(k, jacket, SKIN_A)

def civ_woman(k):
    top = rgb(170, 146, 70, 'cloth'); skirt = rgb(70, 60, 74, 'cloth'); tights = rgb(60, 50, 52, 'cloth')
    inf.legs(k, tights, SHOE, spread=.09, r=.08)
    k.stack(0, 0, [(.5, .4, .46, .14), (inf.HIP + .12, .22, .32, .09)], skirt, k=3)
    inf.torso(k, top, .9)
    zc = inf.head(k, SKIN_C); inf.hair(k, rgb(96, 62, 40, 'cloth'), zc, long_=True)
    k.ball(-.06, 0, zc - .06, .1, rgb(96, 62, 40, 'cloth'), ry=.13, rz=.14, n=10, k=4)
    with inf.swinging(k, 1, .8): inf.arm(k, 1, (.0, .28, 1.14), (.06, .27, .87), top, SKIN_C)
    with inf.swinging(k, -1, .5):
        inf.arm(k, -1, (.0, -.28, 1.14), (.06, -.28, .87), top, SKIN_C)
        k.box(-.06, -.35, .6, .26, .1, .22, rgb(92, 62, 44, 'leather'))              # handbag
        k.tube((.0, -.31, .82), (.08, -.31, .86), .01, DARK, n=4)

def civ_worker(k):
    trousers = rgb(58, 62, 66, 'cloth'); shirt = rgb(150, 140, 120, 'cloth')
    vestc = rgb(206, 196, 58, 'cloth'); band = rgb(206, 210, 210, 'steel')
    inf.legs(k, trousers, inf.BOOT_TAN)
    inf.torso(k, shirt)
    k.stack(.005, 0, [(1.02, .25, .37, .08), (1.3, .28, .43, .1), (inf.SHO - .03, .265, .4, .1)], vestc, k=3)
    for z in (1.1, 1.28): k.rev((.005, 0, z), 'z', [(0, .228), (.035, .228)], band, n=10)
    zc = inf.head(k, SKIN_B); inf.helmet(k, 'hardhat', rgb(232, 168, 44, 'paint'), zc, band=False)
    # A shovel over the right shoulder; the left arm swings free.
    inf.arm(k, -1, (.14, -.3, 1.2), (.2, -.2, 1.5), shirt, SKIN_B)
    k.tube((.4, -.2, 1.62), (-.5, -.2, 1.42), .02, rgb(132, 100, 66, 'wood'), n=6)
    k.box(-.66, -.31, 1.32, .2, .22, .04, rgb(70, 72, 72, 'dark'))
    with inf.swinging(k, 1): inf.arm(k, 1, (.0, .28, 1.14), (.06, .27, .87), shirt, SKIN_B)

PEOPLE = {'civ_man': civ_man, 'civ_woman': civ_woman, 'civ_worker': civ_worker}

# --- Quadrupeds ---------------------------------------------------------------------------------
def leg(k, top, joint, hoof, r0, r1, c, hoofc):
    k.tube(top, joint, r0, c, r2=r0 * .7, n=10); k.ball(joint[0], joint[1], joint[2], r0 * .72, c, n=8, k=4)
    k.tube(joint, hoof, r0 * .62, c, r2=r1, n=8)
    k.cyl(hoof[0], hoof[1], 0, r1 * 1.15, max(.05, hoof[2] + .06), hoofc, n=8)

def gait4(phase, lead, s, front, reach, lift):
    """Diagonal walk: a hoof reaches forward while lifted, pushes back while planted."""
    g = (phase + lead + (0 if s > 0 else .5)) % 1.0
    return reach * cos(2 * pi * g), lift * max(0.0, sin(2 * pi * (g - .5)))

def quadruped(k, P):
    f = inf.phase()
    bob = 0 if f is None else P.get('bob', .02) * abs(sin(2 * pi * f))
    with k.at(0, 0, bob):
        k.body(P['body'], P['coat'], n=14)
        for extra in P.get('extras', ()): extra(k, f)
        hz = P['hip']; fx, bx = P['legs_x']; sp = P['spread']
        for s in (-1, 1):
            for x0, lead in ((fx, 0.0), (bx, .5)):
                if f is None: reach, lift = (.08 if x0 > 0 else -.06) * s, 0.0
                else: reach, lift = gait4(f, lead, s, x0 > 0, P['reach'], P['lift'])
                knee = .45 if x0 > 0 else .5
                bend = 1 if x0 > 0 else -1      # knees forward on the front legs, hocks back behind
                hoof = (x0 + reach, s * sp, lift)
                joint = (x0 + reach * .5 + bend * P['knee_off'] + (lift * .3 if x0 > 0 else -lift * .3), s * sp * 1.02, hz * knee + lift * .8 - bob)
                leg(k, (x0, s * sp * .95, hz - bob), joint, (hoof[0], hoof[1], hoof[2] - bob), P['leg_r'], P['leg_r'] * .55, P.get('legc', P['coat']), P['hoof'])
        if 'neck' in P: k.body(P['neck'], P.get('neckc', P['coat']), n=12)
        if 'head' in P: k.body(P['head'], P.get('headc', P['coat']), n=12)
        tx, tz = P['tail']
        sw = 0 if f is None else .06 * sin(2 * pi * f)
        k.tube((tx, 0, tz), (tx - .1, sw, tz - P.get('tail_len', .5)), P.get('tail_r', .03), P.get('tailc', P['coat']), r2=.015, n=6)

def eyes(k, x, y, z, r=.025):
    for s in (-1, 1): k.ball(x, s * y, z, r, BLACK, n=6, k=3)

def cow(k):
    white = rgb(214, 208, 196, 'cloth'); black = rgb(36, 34, 34, 'cloth'); pink = rgb(196, 150, 140, 'skin')
    def patches(k, f):
        for x, s, z, r in ((-.55, 1, 1.2, .3), (.3, -1, 1.22, .28), (-.05, 1, .98, .22), (.5, 1, 1.25, .2), (-.75, -1, 1.05, .24), (-.2, -1, 1.3, .22)):
            y = s * (.33 if z < 1.3 else .24)
            k.ball(x, y, z, r, black, ry=.08, rz=r * .8, n=10, k=5)
        for x, r in ((-.35, .26), (.2, .22)):
            k.ball(x, 0, 1.44, r, black, ry=.2, rz=.06, n=10, k=4)
        k.ball(-.35, 0, .62, .17, pink, rz=.12, n=10, k=4)                              # udder
    def horns(k, f):
        for s in (-1, 1): k.tube((1.18, s * .1, 1.25), (1.2, s * .24, 1.33), .03, rgb(204, 196, 170, 'paint'), r2=.012, n=5)
        eyes(k, 1.3, .12, 1.15)
    quadruped(k, dict(
        coat=white, hoof=rgb(40, 36, 34, 'rubber'),
        body=[(-.95, 0, 1.08, .26, .3), (-.75, 0, 1.12, .32, .36), (-.2, 0, 1.08, .36, .4), (.35, 0, 1.1, .34, .4), (.72, 0, 1.12, .26, .32)],
        neck=[(.65, 0, 1.12, .2, .28), (.95, 0, 1.1, .16, .22), (1.1, 0, 1.12, .13, .17)],
        head=[(1.08, 0, 1.17, .12, .13), (1.3, 0, 1.08, .12, .12), (1.48, 0, .98, .1, .09)], headc=white,
        extras=(patches, horns), hip=1.0, legs_x=(.5, -.72), spread=.18, leg_r=.11, knee_off=.03,
        reach=.22, lift=.12, tail=(-1.0, 1.12), tail_len=.75, tail_r=.03, tailc=black, bob=.02))
    k.ball(1.5, 0, .96, .08, pink, ry=.09, rz=.06, n=8, k=4)                            # muzzle

def sheep(k):
    wool = rgb(214, 206, 186, 'cloth'); face = rgb(48, 42, 40, 'cloth')
    def fleece(k, f):
        for x, y, z in ((-.3, .14, .78), (0, -.15, .8), (.25, .12, .8), (-.1, .0, .9), (.3, -.1, .88)):
            k.ball(x, y, z, .2, wool, n=10, k=5)
        eyes(k, .66, .07, .77, .02)
        for s in (-1, 1): k.tube((.58, s * .07, .8), (.56, s * .2, .76), .03, face, r2=.02, n=5)  # ears
    quadruped(k, dict(
        coat=wool, legc=face, hoof=rgb(30, 28, 26, 'rubber'),
        body=[(-.55, 0, .7, .2, .2), (-.35, 0, .72, .3, .3), (.1, 0, .74, .32, .3), (.38, 0, .74, .26, .26), (.5, 0, .74, .16, .18)],
        neck=[(.42, 0, .74, .1, .12), (.56, 0, .8, .08, .09)], neckc=wool,
        head=[(.54, 0, .82, .08, .09), (.68, 0, .76, .07, .07), (.76, 0, .7, .04, .05)], headc=face,
        extras=(fleece,), hip=.55, legs_x=(.3, -.35), spread=.12, leg_r=.045, knee_off=.02,
        reach=.15, lift=.08, tail=(-.55, .74), tail_len=.18, tail_r=.05, tailc=wool, bob=.015))

def deer(k):
    coat = rgb(140, 96, 62, 'cloth'); pale = rgb(214, 200, 176, 'cloth'); antler = rgb(176, 160, 128, 'wood')
    def bits(k, f):
        k.ball(-.66, 0, 1.0, .1, pale, ry=.1, rz=.08, n=8, k=4)                          # white rump
        eyes(k, .98, .07, 1.5, .02)
        for s in (-1, 1):
            k.plate([(.86, s * .06, 1.56), (.84, s * .2, 1.68), (.9, s * .1, 1.58)], coat)
            base = (.88, s * .05, 1.6)
            k.tube(base, (.82, s * .16, 1.92), .02, antler, r2=.012, n=5)
            k.tube((.84, s * .12, 1.75), (.96, s * .16, 1.86), .012, antler, n=4)
            k.tube((.82, s * .16, 1.92), (.74, s * .24, 2.02), .01, antler, n=4)
            k.tube((.82, s * .16, 1.92), (.9, s * .2, 2.04), .01, antler, n=4)
    quadruped(k, dict(
        coat=coat, hoof=rgb(34, 30, 28, 'rubber'),
        body=[(-.7, 0, 1.0, .14, .17), (-.5, 0, 1.02, .19, .22), (-.05, 0, 1.0, .2, .25), (.35, 0, 1.03, .18, .25), (.5, 0, 1.08, .13, .18)],
        neck=[(.42, 0, 1.12, .1, .13), (.62, 0, 1.3, .08, .1), (.78, 0, 1.48, .065, .08)],
        head=[(.78, 0, 1.53, .07, .08), (.94, 0, 1.48, .06, .06), (1.06, 0, 1.42, .035, .04)],
        extras=(bits,), hip=.95, legs_x=(.34, -.48), spread=.12, leg_r=.05, knee_off=.03,
        reach=.22, lift=.14, tail=(-.72, 1.06), tail_len=.12, tail_r=.03, tailc=pale, bob=.025))

def horse(k):
    coat = rgb(112, 70, 44, 'cloth'); mane = rgb(36, 28, 24, 'cloth'); sock = rgb(196, 190, 176, 'cloth')
    def bits(k, f):
        k.body([(.5, 0, 1.62, .04, .1), (.75, 0, 1.9, .04, .1), (.9, 0, 2.06, .03, .06)], mane, n=8)   # mane
        eyes(k, 1.12, .1, 1.98)
        for s in (-1, 1): k.plate([(1.0, s * .07, 2.12), (.98, s * .1, 2.26), (1.04, s * .08, 2.14)], coat)
        k.tube((-1.0, 0, 1.4), (-1.12, 0, .8), .07, mane, r2=.05, n=8)
    quadruped(k, dict(
        coat=coat, hoof=rgb(40, 34, 30, 'rubber'),
        body=[(-.95, 0, 1.3, .2, .24), (-.75, 0, 1.33, .27, .32), (-.2, 0, 1.3, .28, .34), (.35, 0, 1.33, .27, .35), (.6, 0, 1.38, .2, .28)],
        neck=[(.5, 0, 1.45, .17, .25), (.75, 0, 1.75, .13, .19), (.92, 0, 1.98, .1, .14)],
        head=[(.92, 0, 2.06, .1, .12), (1.15, 0, 1.92, .09, .1), (1.38, 0, 1.76, .07, .08)],
        extras=(bits,), hip=1.25, legs_x=(.45, -.72), spread=.17, leg_r=.085, knee_off=.04,
        reach=.3, lift=.18, tail=(-.98, 1.42), tail_len=.1, tail_r=.02, bob=.03))

def camel(k):
    coat = rgb(184, 150, 104, 'cloth'); dark = rgb(150, 116, 80, 'cloth')
    def bits(k, f):
        k.ball(-.15, 0, 1.92, .42, coat, ry=.3, rz=.36, n=14, k=7, lo=-.2, hi=1.0)     # hump
        eyes(k, 1.42, .09, 2.04)
        for s in (-1, 1): k.ball(1.3, s * .1, 2.12, .04, dark, n=6, k=3)
        k.ball(-.05, 0, 1.3, .2, dark, ry=.22, rz=.1, n=8, k=4)                        # knee pad of the chest
    quadruped(k, dict(
        coat=coat, hoof=rgb(120, 96, 70, 'rubber'),
        body=[(-.85, 0, 1.6, .2, .24), (-.6, 0, 1.65, .28, .32), (-.15, 0, 1.66, .3, .36), (.35, 0, 1.64, .27, .34), (.55, 0, 1.62, .2, .26)],
        neck=[(.5, 0, 1.55, .16, .2), (.82, 0, 1.4, .12, .14), (1.05, 0, 1.55, .1, .12), (1.2, 0, 1.85, .09, .1), (1.3, 0, 2.02, .09, .1)],
        head=[(1.26, 0, 2.06, .1, .1), (1.45, 0, 2.02, .08, .08), (1.6, 0, 1.95, .06, .06)],
        extras=(bits,), hip=1.5, legs_x=(.38, -.62), spread=.18, leg_r=.095, knee_off=.05,
        reach=.32, lift=.18, tail=(-.88, 1.7), tail_len=.6, tail_r=.025, tailc=dark, bob=.03))

def penguin(k):
    black = rgb(30, 32, 36, 'cloth'); white = rgb(222, 222, 214, 'cloth')
    yellow = rgb(224, 172, 64, 'paint'); beak = rgb(196, 146, 76, 'paint')
    f = inf.phase()
    roll = 0 if f is None else 9 * sin(2 * pi * f)
    for s in (-1, 1):   # feet: the planted one flat, the other shuffling forward
        step = 0 if f is None else .06 * cos(2 * pi * f + (0 if s > 0 else pi))
        k.box(-.04 + step, s * .08 - .05, 0, .16, .1, .03, beak)
    with k.at(0, 0, .03, roll=roll):
        k.ball(0, 0, .44, .22, black, ry=.2, rz=.44, n=14, k=8)
        k.ball(.06, 0, .42, .18, white, ry=.17, rz=.38, n=14, k=8)
        k.ball(.02, 0, .9, .12, black, rz=.12, n=12, k=6)
        k.ball(.08, 0, .86, .08, yellow, ry=.1, rz=.07, n=10, k=4)                      # cheek patch
        k.tube((.12, 0, .9), (.24, 0, .86), .025, beak, r2=.008, n=6)
        eyes(k, .11, .06, .93, .015)
        flap = 0 if f is None else 10 * sin(2 * pi * f)
        for s in (-1, 1):
            with k.at(0, s * .2, .7, roll=s * (18 + flap)):
                k.box(-.06, -.02, -.4, .14, .04, .4, black)

def gull(k):
    """A herring gull gliding with a slow flap; the body sits 14 world px above the ground point."""
    white = rgb(232, 232, 226, 'cloth'); grey = rgb(150, 156, 158, 'cloth'); tip = rgb(30, 30, 32, 'cloth')
    beak = rgb(220, 186, 70, 'paint')
    f = inf.phase() or 0.0
    flap = 38 * sin(2 * pi * f)          # wing angle above the horizontal
    z = 14 / (ZC * U_GULL)               # metres of lift for 14 px
    with k.at(0, 0, z + .05 * sin(2 * pi * f + pi)):
        k.body([(-.3, 0, 0, .04, .03), (-.15, 0, 0, .08, .07), (.1, 0, .01, .09, .08), (.22, 0, .04, .06, .06)], white, n=10)
        k.ball(.27, 0, .06, .055, white, n=8, k=4)
        k.tube((.31, 0, .06), (.38, 0, .05), .012, beak, n=4)
        k.plate([(-.3, -.06, 0), (-.42, 0, -.01), (-.3, .06, 0)], white)                # tail
        for s in (-1, 1):
            with k.at(.05, s * .07, .03, roll=s * flap):
                inner = [(.08, 0, 0), (-.12, 0, 0), (-.16, s * .32, 0), (.06, s * .3, 0)]
                k.plate(inner, grey)
                with k.at(0, s * .31, 0, roll=s * flap * .5):
                    k.plate([(.06, 0, 0), (-.16, 0, 0), (-.1, s * .2, 0), (.02, s * .26, 0)], grey)
                    k.plate([(.02, s * .18, .002), (-.1, s * .18, .002), (-.1, s * .2, .002), (.02, s * .26, .002)], tip)

ANIMALS = {'cow': cow, 'sheep': sheep, 'deer': deer, 'horse': horse, 'camel': camel, 'penguin': penguin}
FLYERS = {'seagull': gull}
WALK_IDS = list(PEOPLE) + list(ANIMALS) + list(FLYERS)

def walker(m, ident):
    """The figure in the current hd_infantry.POSE, as hd_infantry.build poses its soldiers."""
    if ident in FLYERS:
        FLYERS[ident](Kit(m, U_GULL)); return
    k = Kit(m, inf.U); k.base = k.T
    inf._RUN = False
    if ident in ANIMALS:
        ANIMALS[ident](k); return
    f = inf.phase()
    if f is None:
        PEOPLE[ident](k); return
    turn = -9 * cos(2 * pi * f); roll = 3 * sin(2 * pi * f)
    with k.at(0, 0, inf.pelvis(), yaw=turn, pitch=-4, roll=roll):
        with k.at(0, 0, -inf.HIP): PEOPLE[ident](k)
