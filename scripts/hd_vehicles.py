"""Ground vehicles, modelled in metres from real reference vehicles (dimensions in the comments).

Allied vehicles wear desert sand, Soviet ones dark protective green. Ownership is carried the way
real armies mark vehicles: an air-recognition panel on the roof, a band round the turret or cab
and painted fender tips, in the owner's colour (`TEAM`).
"""
from math import sin, cos, pi, radians
from hd_kit import *

# Field vehicles are sprayed in a disruptive pattern over their base colour (the `camo` shader).
NATO_SAND = remat(rgb(160, 142, 104), 'camo'); NATO_GREEN = remat(rgb(92, 102, 70), 'camo')
SOV_GREEN = remat(rgb(80, 92, 58), 'camo'); SOV_DARK = remat(rgb(60, 70, 48), 'camo')
TRACK = rgb(40, 37, 34, 'rubber')   # steel track links, dark and packed with earth

U = .125  # cells per metre: a 7.9 m battle tank is one cell long. Nothing may reach further
# than 7.4 m from the origin (0.93 cells), or it leaves the 96 px sprite frame at some facing.

# --- Running gear ------------------------------------------------------------------------------
def track_outline(x0, x1, ttop, rs=.30, n=5):
    zc = ttop - rs
    pts = [(x0 + .75, 0), (x1 - .9, 0)]
    for i in range(n + 1):
        a = radians(-65 + i * 155 / n); pts.append((x1 - rs + rs * cos(a), zc + rs * sin(a)))
    for i in range(n + 1):
        a = radians(90 + i * 155 / n); pts.append((x0 + rs + rs * cos(a), zc + rs * sin(a)))
    return pts

def tracks(k, x0, x1, W, tw, nw, wr, ttop, hubc, rs=.30, inner=0.0):
    """A track either side between x0 (rear) and x1 (front): band, road wheels, sprocket, idler.
    `inner` > 0 moves the pair inboard (the inner tracks of a four-track vehicle)."""
    for s in (-1, 1):
        ya, yb = sorted([s * (W / 2 - tw - inner), s * (W / 2 - inner)])
        k.extrude(track_outline(x0, x1, ttop, rs), 'xz', ya, yb, TRACK)
        yo = s * (W / 2 - inner - .11)
        a, b = x0 + .75 + wr * .55, x1 - .9 - wr * .55
        for i in range(nw):
            k.wheel(a + (b - a) * i / (nw - 1), yo, wr, wr, .26, tyre=RUBBER, hub=hubc)
        zc = ttop - rs
        k.wheel(x0 + rs, yo, zc, rs * .82, .26, tyre=IRON, hub=DARK, n=12)
        k.wheel(x1 - rs, yo, zc, rs * .78, .26, tyre=IRON, hub=hubc, n=12)
        # Grousers along the visible top and end runs.
        for i in range(int((x1 - x0 - .6) / .16)):
            k.box(x0 + .3 + i * .16, ya, ttop - .005, .09, yb - ya, .03, tint(TRACK, .8))

def road_wheels(k, xs, W, r, hubc, w=.36, dual=()):
    for i, x in enumerate(xs):
        for s in (-1, 1):
            k.wheel(x, s * (W / 2 - w / 2), r, r, w, tyre=TYRE, hub=hubc)
            if i in dual: k.wheel(x, s * (W / 2 - w * 1.6), r, r, w, tyre=TYRE, hub=DARK)

def tank_hull(k, L, W, tw, c, clear=.45, ttop=.92, deck=1.2, glacis=1.8, nose=.82, sponson=True):
    inner = W / 2 - tw - .04
    k.extrude([(-L / 2 + .05, ttop), (-L / 2 + .3, clear), (L / 2 - 1.05, clear), (L / 2 - .08, nose),
               (L / 2 - glacis, deck), (-L / 2 + .12, deck)], 'xz', -inner, inner, c)
    if sponson:
        k.extrude([(-L / 2 + .08, ttop + .03), (L / 2 - .45, ttop + .03), (L / 2 - glacis, deck),
                   (-L / 2 + .12, deck)], 'xz', -W / 2, W / 2, c)
    else:
        for s in (-1, 1):
            y0 = min(s * inner, s * (W / 2 + .03))
            k.box(-L / 2 + .02, y0, ttop + .03, L - .25, tw + .07, .05, c)
    # Mud flaps and painted fender tips.
    for s in (-1, 1):
        y0 = min(s * (W / 2 - tw), s * W / 2)
        k.box(-L / 2 - .02, y0, ttop - .35, .04, tw, .4, RUBBER)
        k.box(L / 2 - .4, y0, ttop + .035, .38, tw, .05, TEAM)

def skirts(k, L, W, c, z0=.5, z1=1.0, n=7, x0=None, x1=None):
    x0 = -L / 2 + .25 if x0 is None else x0; x1 = L / 2 - .55 if x1 is None else x1
    w = (x1 - x0) / n
    for s in (-1, 1):
        for i in range(n):
            k.box(x0 + i * w + .015, s * (W / 2 + .03) - .025, z0, w - .03, .05, z1 - z0, tint(c, .96 + .05 * (i % 2)))

def engine_deck(k, x, W, deck, c, n=2, l=1.3):
    for i in range(n):
        y = -W * .3 + i * W * .6 / max(1, n - 1) if n > 1 else 0
        k.cbox(x, y, deck, l, W * .24, .035, DARK)
        for j in range(6): k.cbox(x - l / 2 + .12 + j * (l - .24) / 5, y, deck + .03, .05, W * .22, .03, IRON)

def gun(k, x, z, length, r, c=GUN, pitch=0.0, y=0.0, brake=None, evac=.5, sleeve=True):
    """Tank gun from its trunnion at (x, y, z): tube, thermal sleeve, bore evacuator, muzzle."""
    with k.at(x, y, z, pitch=pitch):
        prof = [(0, r * 1.5), (length * .12, r * 1.5), (length * .12, r * 1.15)]
        if evac:
            e = length * evac
            prof += [(e - .32, r * 1.12), (e - .28, r * 1.75), (e + .28, r * 1.75), (e + .32, r * 1.08)]
        prof += [(length - .5, r), (length - .5, r * 1.02)]
        if brake == 'double':
            prof += [(length - .48, r * 2.0), (length - .3, r * 2.0), (length - .3, r), (length - .2, r), (length - .2, r * 2.0), (length, r * 2.0)]
        elif brake == 'single':
            prof += [(length - .34, r * 1.9), (length, r * 1.9)]
        elif brake == 'pepper':
            prof += [(length - .55, r * 1.45), (length, r * 1.45)]
        else:
            prof += [(length, r)]
        k.rev((0, 0, 0), 'x', prof, c, n=12)
        if sleeve:
            for f in (.26, .38, .7, .82): k.rev((length * f, 0, 0), 'x', [(0, r * 1.22), (.05, r * 1.22)], DARK, n=10)
        k.rev((length - .02, 0, 0), 'x', [(0, r * .7), (.03, r * .7)], BLACK, n=8)

def mg(k, x, y, z, yaw=0.0, c=GUN, length=1.2):
    """Pintle-mounted heavy machine gun with its ammunition box."""
    k.tube((x, y, z), (x, y, z + .3), .04, c, n=6)
    with k.at(x, y, z + .34, yaw=yaw, pitch=8):
        k.box(-.3, -.06, -.06, .5, .12, .14, c)
        k.tube((.2, 0, .01), (length, 0, .01), .028, c, n=6)
        k.box(-.1, .07, -.1, .26, .16, .18, OLIVE)

def antenna(k, x, y, z, h=2.2, pennant=True):
    k.tube((x, y, z), (x, y, z + .12), .04, DARK, n=6)
    k.tube((x, y, z + .12), (x - .12, y, z + h), .012, DARK, n=4)
    if pennant: k.plate([(x - .1, y, z + h - .1), (x - .55, y, z + h - .22), (x - .1, y, z + h - .38)], TEAM)

def smoke_launchers(k, x, y, z, s, c=GUN, n=4):
    for i in range(n):
        with k.at(x - i * .13, y, z + (i % 2) * .11, yaw=s * 40, pitch=28):
            k.tube((0, 0, 0), (.24, 0, 0), .04, c, n=6)

def stowage(k, x, y, z, w, d, c):
    """Bustle rack: rail cage with bedrolls, a tarp and a jerry can."""
    for zz in (z + .02, z + .36):
        k.pipe([(x, y - d / 2, zz), (x - w, y - d / 2, zz), (x - w, y + d / 2, zz), (x, y + d / 2, zz)], .02, c, n=4)
    for yy in (y - d / 2, y - d / 6, y + d / 6, y + d / 2): k.strut((x - w, yy, z + .02), (x - w, yy, z + .36), .03, c)
    k.tube((x - w * .5, y - d * .42, z + .16), (x - w * .5, y + d * .1, z + .16), .13, TARP, n=8)
    k.tube((x - w * .25, y - d * .1, z + .14), (x - w * .25, y + d * .42, z + .14), .11, CANVAS, n=8)
    k.box(x - w * .85, y + d * .15, z + .02, .16, .36, .42, OLIVE)
    k.box(x - w * .9, y - d * .42, z + .02, .3, .5, .26, tint(c, .85))

def headlights(k, x, W, z, c):
    for s in (-1, 1):
        k.cbox(x, s * (W / 2 - .45), z, .14, .2, .16, c)
        k.plate([(x + .075, s * (W / 2 - .45) - .07, z + .03), (x + .075, s * (W / 2 - .45) + .07, z + .03),
                 (x + .075, s * (W / 2 - .45) + .07, z + .13), (x + .075, s * (W / 2 - .45) - .07, z + .13)], LAMP)

def hatch(k, x, y, z, r=.3, c=None):
    k.cyl(x, y, z, r, .07, c, n=14); k.cyl(x, y, z + .07, r * .25, .03, DARK, n=6)

def cupola(k, x, y, z, c, r=.36):
    k.cyl(x, y, z, r, .2, c, n=14); k.cyl(x, y, z + .2, r * .9, .05, tint(c, .9), n=14)
    for i in range(6):
        a = i * pi / 3
        k.cbox(x + r * .95 * cos(a), y + r * .95 * sin(a), z + .07, .08, .08, .09, GLASS_DARK)

def tow_cable(k, L, W, z, s=1):
    k.pipe([(L / 2 - 2.2, s * (W / 2 - .2), z), (0, s * (W / 2 - .15), z), (-L / 2 + .8, s * (W / 2 - .2), z)], .025, IRON, n=5)

def era(k, cx, cy, z, r, c, a0=-70, a1=70, n=9, h=.3):
    """Explosive reactive armour bricks round a cast turret's front arc."""
    for i in range(n):
        a = a0 + (a1 - a0) * i / (n - 1)
        with k.at(cx + r * cos(radians(a)), cy + r * sin(radians(a)), z, yaw=a, pitch=-25):
            k.cbox(0, 0, 0, .12, .3, h, c)

# --- Turrets -----------------------------------------------------------------------------------
def turret_welded(k, x, z, c, front=1.9, rear=1.9, w=1.4, h=.8, nose=.5):
    """Angular welded turret with a long bustle (M1, Leopard 2): sloped cheeks, flat roof."""
    bot = [(x + front, nose), (x + front * .45, w), (x - rear, w * .94), (x - rear, -w * .94), (x + front * .45, -w), (x + front, -nose)]
    top = [(x + front * .72, nose * .9), (x + front * .4, w * .86), (x - rear * .97, w * .86), (x - rear * .97, -w * .86), (x + front * .4, -w * .86), (x + front * .72, -nose * .9)]
    k.loft([[(a, b, z) for a, b in bot], [(a, b, z + h) for a, b in top]], c, smooth=False)
    k.cyl(x, 0, z - .08, w * .8, .1, DARK, n=20)

def turret_cast(k, x, z, c, rx=1.3, ry=1.25, h=.78):
    """Low cast dome (T-72, T-80): a flattened ellipsoid, slightly longer forward."""
    k.cyl(x, 0, z - .06, rx * .82, .08, DARK, n=20)
    k.ball(x, 0, z, rx, c, ry=ry, rz=h, n=20, k=6, lo=0.0, hi=1.0)

def roof_panel(k, x, y, z, w, d):
    k.cbox(x, y, z, w, d, .03, TEAM)

# --- Battle tanks ------------------------------------------------------------------------------
def grizzly(k):
    # Allied main battle tank, after the M1 Abrams: 7.9 x 3.66 x 2.44 m, 120 mm gun.
    c = NATO_SAND; L, W = 7.9, 3.6
    tracks(k, -L / 2, L / 2, W, .62, 7, .32, .95, c)
    tank_hull(k, L, W, .62, c, deck=1.22, glacis=2.2, nose=.8)
    skirts(k, L, W, c, .45, 1.02, 7)
    engine_deck(k, -2.9, W, 1.22, c, 2, 1.4)
    k.box(-L / 2 + .02, -1.0, .75, .08, 2.0, .4, DARK)  # exhaust grille
    headlights(k, L / 2 - 1.2, W, 1.05, c); hatch(k, 2.1, 0, 1.1, .33, c)
    tow_cable(k, L, W, 1.24)
    x = .25; z = 1.24
    turret_welded(k, x, z, c, front=2.0, rear=2.1, w=1.5, h=.82, nose=.55)
    k.cbox(x + 1.85, 0, z + .1, .5, .9, .6, tint(c, .9))  # mantlet
    gun(k, x + 2.0, z + .42, 5.1, .085, evac=.52)
    k.tube((x + 2.05, .32, z + .45), (x + 2.6, .32, z + .45), .03, GUN, n=6)  # coaxial
    cupola(k, x - .2, -.7, z + .82, c); mg(k, x - .2, -.7, z + 1.05, yaw=8)
    hatch(k, x - .3, .65, z + .82, .32, c); mg(k, x + .1, .9, z + .86, yaw=-12, length=.9)
    k.cbox(x + .7, -.75, z + .82, .34, .34, .42, c); k.cbox(x + .88, -.75, z + .92, .02, .24, .22, GLASS_DARK)  # sight
    roof_panel(k, x - 1.25, 0, z + .82, .9, 1.5)
    for s in (-1, 1):
        smoke_launchers(k, x + .4, s * 1.48, z + .35, s)
        k.box(x - 1.95, s * 1.42 - .02, z + .32, 1.3, .04, .26, TEAM)
        k.box(x - 1.6, s * 1.48 - .15, z + .2, .9, .3, .32, tint(c, .9))  # stowage bins
    stowage(k, x - 2.12, 0, z + .08, .75, 2.5, c)
    antenna(k, x - 1.9, 1.2, z + .82); antenna(k, x - 1.9, -1.2, z + .82, 1.8, False)

def light_tank(k):
    # Allied light tank, after the M8 AGS / Stingray: 6.2 x 2.7 x 2.5 m, 105 mm gun.
    c = NATO_SAND; L, W = 6.2, 2.75
    tracks(k, -L / 2, L / 2, W, .45, 6, .29, .86, c, rs=.27)
    tank_hull(k, L, W, .45, c, clear=.42, ttop=.86, deck=1.12, glacis=1.9, nose=.74)
    engine_deck(k, -2.2, W, 1.12, c, 2, 1.0)
    headlights(k, L / 2 - 1.2, W, .98, c); hatch(k, 1.5, .55, 1.02, .27, c); tow_cable(k, L, W, 1.14)
    x = -.1; z = 1.14
    turret_welded(k, x, z, c, front=1.5, rear=1.35, w=1.05, h=.68, nose=.45)
    k.cbox(x + 1.4, 0, z + .08, .4, .7, .5, tint(c, .9))
    gun(k, x + 1.5, z + .36, 4.2, .07, brake='pepper', evac=.6)
    cupola(k, x - .25, -.45, z + .68, c, .3); mg(k, x - .25, -.45, z + .88, yaw=5, length=1.0)
    hatch(k, x - .2, .5, z + .68, .27, c)
    roof_panel(k, x - .85, 0, z + .68, .7, 1.5)
    for s in (-1, 1):
        smoke_launchers(k, x + .3, s * 1.05, z + .3, s, n=3)
        k.box(x - 1.3, s * 1.0 - .02, z + .1, 1.0, .04, .45, TEAM)
    stowage(k, x - 1.36, 0, z + .1, .5, 1.8, c)
    antenna(k, x - 1.1, .85, z + .68, 2.0)

def rhino(k):
    # Soviet main battle tank, after the T-72B: 6.95 x 3.59 x 2.23 m, 125 mm gun.
    c = SOV_GREEN; L, W = 6.95, 3.5
    tracks(k, -L / 2, L / 2, W, .58, 6, .375, .98, c, rs=.31)
    tank_hull(k, L, W, .58, c, clear=.45, ttop=.98, deck=1.12, glacis=1.9, nose=.78, sponson=False)
    for s in (-1, 1):  # rubber side skirts, fender fuel cells and stowage boxes
        k.box(-L / 2 + .3, s * (W / 2 + .03) - .02, .55, L - 1.0, .04, .5, remat(tint(c, .8), 'rubber'))
        y0 = min(s * (W / 2 - .56), s * W / 2)
        for xb, lb in [(-2.6, 1.1), (-1.3, 1.2), (.2, 1.0)]: k.box(xb, y0 + .04, 1.06, lb, .5, .24, tint(c, .92))
    k.cbox(2.35, 0, .9, .3, 2.2, .3, c)  # glacis wave breaker
    for i in range(5): k.cbox(1.75 - i * .02, -1.0 + i * .5, 1.0 + .12, .2, .4, .08, tint(c, .86))  # hull ERA
    engine_deck(k, -2.5, W, 1.12, c, 1, 1.3); k.cbox(-1.5, 0, 1.12, .5, 1.6, .06, DARK)
    for y in (-.75, .75): k.tube((-L / 2 - .32, y - .42, 1.3), (-L / 2 - .32, y + .42, 1.3), .29, tint(c, .92), n=12)  # fuel drums
    k.tube((-L / 2 + .05, -1.5, .78), (-L / 2 + .05, 1.5, .78), .11, TIMBER, n=8)  # unditching log
    headlights(k, L / 2 - 1.0, W, 1.0, c); hatch(k, 1.9, 0, 1.0, .3, c)
    x = .15; z = 1.14
    turret_cast(k, x, z, c, 1.34, 1.3, .74)
    era(k, x, 0, z + .14, 1.22, tint(c, .9), -75, 75, 10)
    k.cbox(x + 1.2, 0, z + .12, .4, .6, .45, tint(c, .88))
    gun(k, x + 1.3, z + .36, 5.9, .082, evac=.5)
    k.cyl(x + .75, -.62, z + .52, .19, .26, DARK, n=10); k.cyl(x + .86, -.62, z + .55, .15, .2, HOT, n=10)  # IR searchlight
    cupola(k, x - .2, -.55, z + .66, c, .34); mg(k, x - .2, -.55, z + .9, yaw=170, length=1.3)
    hatch(k, x - .2, .55, z + .64, .3, c)
    roof_panel(k, x - .55, 0, z + .735, .8, .7)
    for s in (-1, 1):
        smoke_launchers(k, x + .55, s * 1.1, z + .28, s, n=4)
        k.box(x - 1.25, s * .92 - .02, z + .22, .6, .04, .3, TEAM)
    k.tube((x - 1.25, -.8, z + .3), (x - 1.25, .8, z + .3), .12, tint(c, .9), n=10)  # snorkel
    k.cbox(x - 1.05, 0, z + .08, .5, 1.2, .3, tint(c, .86))
    antenna(k, x - .7, .95, z + .5, 2.4)

def heavy_tank(k):
    # Soviet heavy tank, after the T-10M / IS-3: 7.25 x 3.4 x 2.45 m, 122 mm gun.
    c = SOV_DARK; L, W = 7.3, 3.4
    tracks(k, -L / 2, L / 2, W, .6, 7, .31, .95, c)
    inner = W / 2 - .64
    # Pike nose: the glacis is two plates meeting on the centre line.
    k.extrude([(-L / 2 + .05, .95), (-L / 2 + .3, .45), (L / 2 - 1.1, .45), (L / 2 - .4, .85), (L / 2 - 1.7, 1.2), (-L / 2 + .12, 1.2)], 'xz', -inner, inner, c)
    k.loft([[(L / 2 - .4, -inner, .85), (L / 2 + .05, 0, .9), (L / 2 - .4, inner, .85)],
            [(L / 2 - 1.7, -inner, 1.2), (L / 2 - 1.5, 0, 1.22), (L / 2 - 1.7, inner, 1.2)]], c, smooth=False)
    for s in (-1, 1):
        y0 = min(s * inner, s * (W / 2 + .03)); k.box(-L / 2 + .02, y0, .98, L - .5, .67, .05, c)
        k.box(L / 2 - .85, y0, .985, .5, .67, .05, TEAM)
        k.box(-2.4, y0 + .08, 1.03, 2.2, .5, .3, tint(c, .9))  # external fuel tanks
        k.box(-L / 2 - .02, y0, .6, .04, .6, .4, RUBBER)
    engine_deck(k, -2.6, W, 1.2, c, 2, 1.3); headlights(k, L / 2 - 1.3, W, 1.02, c); hatch(k, 1.6, 0, 1.12, .3, c)
    k.tube((-L / 2 + .05, -1.4, .82), (-L / 2 + .05, 1.4, .82), .1, TIMBER, n=8)
    x = .3; z = 1.22
    k.cyl(x, 0, z - .06, 1.15, .08, DARK, n=20)
    k.ball(x - .1, 0, z, 1.55, c, ry=1.4, rz=.92, n=20, k=6, lo=0.0, hi=1.0)  # frying-pan turret
    k.cbox(x + 1.35, 0, z + .14, .4, .7, .46, tint(c, .88))
    gun(k, x + 1.45, z + .4, 5.6, .09, brake='double', evac=.62, sleeve=False)
    cupola(k, x - .45, -.6, z + .78, c, .36); mg(k, x - .45, -.6, z + 1.0, yaw=165, length=1.5)
    hatch(k, x - .4, .6, z + .78, .3, c)
    roof_panel(k, x - .1, 0, z + .925, .8, .9)
    for s in (-1, 1):
        k.box(x - 1.5, s * 1.0 - .02, z + .2, .7, .04, .34, TEAM)
        k.pipe([(x - 1.2, s * 1.22, z + .2), (x - 1.2, s * 1.32, z + .38), (x + .4, s * 1.46, z + .38), (x + .4, s * 1.36, z + .2)], .02, c, n=4)  # hand rails
    k.cbox(x - 1.45, 0, z + .08, .45, 1.4, .34, tint(c, .86))
    antenna(k, x - .9, .95, z + .6, 2.4)

def apocalypse(k):
    # Soviet super-heavy assault tank: four track units (Object 279) under a twin 125 mm turret
    # with anti-aircraft missile boxes. 9.4 x 4.8 x 3.2 m.
    c = SOV_DARK; L, W = 9.4, 4.8
    tracks(k, -L / 2, L / 2, W, .62, 7, .36, 1.05, c, rs=.33)
    tracks(k, -L / 2 + .2, L / 2 - .2, W, .55, 7, .36, 1.05, c, rs=.33, inner=.95)
    inner = W / 2 - 1.55
    k.extrude([(-L / 2 + .1, 1.05), (-L / 2 + .4, .55), (L / 2 - 1.2, .55), (L / 2 - .1, 1.0), (L / 2 - 2.2, 1.75), (-L / 2 + .15, 1.75)], 'xz', -inner, inner, c)
    k.extrude([(-L / 2 + .1, 1.1), (L / 2 - .5, 1.1), (L / 2 - 2.2, 1.75), (-L / 2 + .15, 1.75)], 'xz', -W / 2, W / 2, c)
    skirts(k, L, W, c, .55, 1.18, 8)
    for s in (-1, 1):
        y0 = min(s * (W / 2 - .7), s * W / 2)
        k.box(L / 2 - .58, y0, 1.105, .55, .7, .05, TEAM); k.box(-L / 2 - .02, y0, .7, .04, .7, .45, RUBBER)
        k.box(-3.6, s * 2.0 - .3, 1.75, 2.4, .6, .32, tint(c, .9))
        k.tube((-L / 2 - .1, s * 1.2 - .5, 1.5), (-L / 2 - .1, s * 1.2 + .5, 1.5), .32, tint(c, .92), n=12)
    engine_deck(k, -3.3, W * .7, 1.75, c, 2, 1.6); headlights(k, L / 2 - 1.4, W, 1.45, c)
    hatch(k, 2.6, -.8, 1.6, .32, c); hatch(k, 2.6, .8, 1.6, .32, c)
    x = .2; z = 1.77
    k.stack(x, 0, [(z, 4.4, 3.5, .5), (z + .5, 4.5, 3.6, .7), (z + 1.05, 3.6, 2.9, .8, -.2), (z + 1.15, 3.2, 2.5, .8, -.25)], c, k=4)
    k.cyl(x, 0, z - .08, 1.6, .1, DARK, n=20)
    for s in (-1, 1):
        k.cbox(x + 2.1, s * .7, z + .25, .5, .8, .6, tint(c, .88))
        gun(k, x + 2.3, z + .55, 4.9, .085, y=s * .7, evac=.5)
        # Quad anti-aircraft missile box on each cheek.
        with k.at(x - .6, s * 1.95, z + .8, pitch=22):
            k.cbox(0, 0, 0, 1.5, .5, .5, tint(c, .92))
            for i in range(2):
                for j in range(2): k.rev((.75, -.12 + i * .24, .13 + j * .24), 'x', [(0, .09), (.03, .09)], BLACK, n=8)
        k.box(x - 2.3, s * 1.82 - .02, z + .1, 1.4, .04, .42, TEAM)
        era(k, x + 1.2, s * 1.2, z + .2, .9, tint(c, .9), 10 if s > 0 else -80, 80 if s > 0 else -10, 5)
    cupola(k, x - .5, -.8, z + 1.15, c, .38); mg(k, x - .5, -.8, z + 1.38, yaw=170, length=1.5)
    hatch(k, x - .5, .8, z + 1.15, .32, c)
    roof_panel(k, x - 1.35, 0, z + 1.15, .9, 2.0)
    k.cbox(x - 2.45, 0, z + .1, .5, 2.6, .5, tint(c, .86))
    antenna(k, x - 1.9, 1.1, z + 1.15, 2.6); antenna(k, x - 1.9, -1.1, z + 1.15, 2.0, False)

def tank_destroyer(k):
    # Allied tank destroyer, after the Kanonenjagdpanzer: 6.24 x 2.98 x 2.09 m (8.75 m with gun).
    c = NATO_GREY; L, W = 6.3, 3.0
    tracks(k, -L / 2, L / 2, W, .5, 5, .34, .9, c, rs=.29)
    inner = W / 2 - .54
    k.extrude([(-L / 2 + .05, .9), (-L / 2 + .3, .44), (L / 2 - 1.0, .44), (L / 2 - .1, .8), (L / 2 - .5, 1.05), (-L / 2 + .12, 1.05)], 'xz', -inner, inner, c)
    # Casemate: one sloped box over the whole hull, the gun in its front plate.
    k.loft([[(L / 2 - .45, -W / 2, .95), (L / 2 - .45, W / 2, .95), (-1.2, W / 2, .95), (-1.2, -W / 2, .95)],
            [(.9, -W / 2 + .5, 1.95), (.9, W / 2 - .5, 1.95), (-1.0, W / 2 - .4, 1.95), (-1.0, -W / 2 + .4, 1.95)]], c, smooth=False)
    k.box(-L / 2 + .1, -W / 2, .95, L / 2 - 1.3 + .1, W, .3, c)
    engine_deck(k, -2.2, W, 1.25, c, 2, 1.2)
    for s in (-1, 1):
        y0 = min(s * (W / 2 - .5), s * W / 2)
        k.box(L / 2 - .5, y0, .955, .45, .5, .05, TEAM); k.box(-L / 2 - .02, y0, .55, .04, .5, .4, RUBBER)
        k.box(-.9, s * (W / 2 - .42) - .02, 1.3, 1.5, .04, .4, TEAM)
        smoke_launchers(k, -.2, s * 1.2, 1.7, s, n=4)
    with k.at(1.7, .25, 1.4): k.ball(0, 0, 0, .42, tint(c, .88), ry=.5, rz=.42, n=12, k=6)  # ball mantlet
    gun(k, 1.8, 1.4, 4.4, .075, y=.25, brake='single', evac=.55)
    cupola(k, -.2, -.6, 1.95, c, .34); mg(k, -.2, -.6, 2.17, yaw=6, length=1.1)
    hatch(k, -.3, .6, 1.95, .3, c); roof_panel(k, .3, 0, 1.95, .7, 1.2)
    headlights(k, L / 2 - .7, W, .98, c); tow_cable(k, L, W, 1.27, -1)
    antenna(k, -.9, .9, 1.95, 2.2)

def artillery(k):
    # Allied self-propelled howitzer, after the M109A6 Paladin: 6.8 x 3.15 x 3.25 m, 155 mm.
    c = NATO_SAND; L, W = 6.6, 3.15
    tracks(k, -L / 2, L / 2, W, .5, 7, .3, .9, c, rs=.28)
    tank_hull(k, L, W, .5, c, clear=.45, ttop=.9, deck=1.5, glacis=1.7, nose=.9)
    headlights(k, L / 2 - 1.1, W, 1.25, c); hatch(k, 1.6, .8, 1.36, .3, c)
    k.cbox(1.9, -.7, 1.3, .9, .9, .06, DARK)  # engine grille, front right
    x = -1.3; z = 1.52
    k.stack(x, 0, [(z, 3.9, 3.1, .12), (z + 1.0, 3.8, 3.05, .15), (z + 1.3, 3.3, 2.7, .3)], c, k=2, smooth=False)
    k.cbox(x + 2.0, 0, z + .3, .5, 1.0, .8, tint(c, .9))
    gun(k, x + 2.1, z + .75, 6.2, .1, pitch=24, brake='double', evac=.72, sleeve=False)
    k.tube((x + 2.2, 0, z + .5), (x + 3.2, 0, z + .95), .09, STEEL, n=8)  # recuperator
    cupola(k, x + .3, -.9, z + 1.3, c, .36); mg(k, x + .3, -.9, z + 1.52, yaw=10)
    hatch(k, x + .3, .8, z + 1.3, .32, c); roof_panel(k, x - .8, 0, z + 1.3, 1.1, 2.0)
    for s in (-1, 1):
        k.box(x - 1.9, s * 1.54 - .02, z + .2, 1.5, .04, .5, TEAM)
        k.box(x - 1.2, s * 1.58 - .2, z + .1, 1.6, .4, .5, tint(c, .92))  # side stowage baskets
        k.box(x - 2.3, s * 1.0 - .3, .5, .5, .6, .9, DARK)  # recoil spades
    k.cbox(x - 2.1, 0, z + .1, .4, 2.2, .8, tint(c, .9))
    antenna(k, x - 1.7, 1.3, z + 1.3, 2.6)

# --- Special tracked vehicles --------------------------------------------------------------------
def apc_hull(k, L, W, c, tw=.42, nw=5, deck=1.75, wr=.3):
    """Box-bodied tracked carrier (M113 / Bradley family)."""
    tracks(k, -L / 2, L / 2, W, tw, nw, wr, .85, c, rs=.27)
    inner = W / 2 - tw - .04
    k.extrude([(-L / 2 + .05, .85), (-L / 2 + .25, .42), (L / 2 - 1.0, .42), (L / 2 - .05, .9), (L / 2 - 1.3, deck), (-L / 2 + .1, deck)], 'xz', -inner, inner, c)
    k.extrude([(-L / 2 + .08, .88), (L / 2 - .5, .88), (L / 2 - 1.3, deck), (-L / 2 + .1, deck)], 'xz', -W / 2, W / 2, c)
    for s in (-1, 1):
        y0 = min(s * (W / 2 - tw), s * W / 2)
        k.box(L / 2 - .52, y0, .885, .45, tw, .05, TEAM); k.box(-L / 2 - .02, y0, .5, .04, tw, .4, RUBBER)
    headlights(k, L / 2 - .95, W, 1.3, c); hatch(k, L / 2 - 1.9, .6, deck, .28, c)
    k.cbox(L / 2 - 1.8, -.55, deck, .8, .8, .05, DARK)

def mirage_tank(k):
    # Allied deception tank: a light chassis under camouflage netting and cut foliage, with the
    # mirage projector where the gun would be.
    c = NATO_GREEN; L, W = 5.6, 2.7
    tracks(k, -L / 2, L / 2, W, .42, 5, .29, .85, c, rs=.27)
    tank_hull(k, L, W, .42, c, clear=.42, ttop=.85, deck=1.1, glacis=1.7, nose=.72)
    x = -.2; z = 1.12
    k.stack(x, 0, [(z, 2.4, 2.0, .5), (z + .5, 2.2, 1.8, .6), (z + .62, 1.7, 1.4, .6)], c, k=3)
    with k.at(x + 1.1, 0, z + .34):
        k.rev((0, 0, 0), 'x', [(0, .2), (.5, .2), (.5, .3), (1.2, .36), (1.25, .3)], GUN, n=12)
        k.rev((1.2, 0, 0), 'x', [(0, .28), (.03, .26)], PLASMA, n=12)
    roof_panel(k, x - .2, 0, z + .62, .8, .9)
    # Netting draped over the rear and sides, garnished with branches.
    for px, py, pz, r in [(-1.9, .7, 1.25, .75), (-2.0, -.6, 1.2, .7), (-1.0, 1.0, 1.15, .55), (-1.1, -1.05, 1.15, .55), (.9, 1.05, 1.05, .45), (.9, -1.05, 1.05, .45)]:
        k.ball(px, py, pz, r, TARP, rz=r * .45, n=10, k=4)
    import random
    rnd = random.Random(7)
    for i in range(16):
        px, py = rnd.uniform(-2.4, .4), rnd.uniform(-1.2, 1.2)
        pz = 1.5 + rnd.uniform(0, .5)
        k.tube((px, py, 1.2), (px + rnd.uniform(-.2, .2), py + rnd.uniform(-.2, .2), pz), .03, TIMBER, n=4)
        k.ball(px, py, pz + .2, rnd.uniform(.3, .5), LEAF if i % 2 else LEAF2, rz=.3, n=8, k=4)
    for s in (-1, 1): k.box(x - .9, s * .92 - .02, z + .12, .9, .04, .3, TEAM)
    antenna(k, x - .9, .7, z + .6, 2.0)

def prism_tank(k):
    # Allied directed-energy vehicle: a carrier hull with a beam director on a yoke, its
    # capacitor banks and cooling radiators behind.
    c = NATO_SAND; L, W = 6.0, 2.9
    apc_hull(k, L, W, c, deck=1.55)
    x = -.5; z = 1.57
    k.cyl(x, 0, z - .02, 1.1, .12, DARK, n=20)
    # Armoured mount: a welded housing that carries the beam director between two trunnion cheeks.
    k.loft([[(x + 1.0, -.7, z + .08), (x + .3, -1.1, z + .08), (x - 1.2, -1.05, z + .08), (x - 1.2, 1.05, z + .08), (x + .3, 1.1, z + .08), (x + 1.0, .7, z + .08)],
            [(x + .7, -.6, z + .75), (x + .2, -.95, z + .75), (x - 1.15, -.95, z + .75), (x - 1.15, .95, z + .75), (x + .2, .95, z + .75), (x + .7, .6, z + .75)]], c, smooth=False)
    for s_ in (-1, 1):
        k.box(x - .45, s_ * .78 - .11, z + .7, .8, .22, .95, c)                       # trunnion cheek
        k.rev((x - .05, s_ * .9, z + 1.25), (0, s_, 0), [(0, .2), (.06, .2), (.06, .1), (.1, .1)], IRON, n=10)   # trunnion cap
        k.box(x - 2.35, s_ * .72 - .32, z - .02, 1.05, .64, .62, tint(c, .9))         # capacitor bank
        for i in range(6): k.box(x - 2.3 + i * .18, s_ * .72 - .34, z + .6, .05, .68, .14, STEEL)   # cooling fins
        k.box(x - 1.1, s_ * .96 - .0, z + .2, .9, .03, .34, TEAM)
    # The director: a stepped optical tube, gold-coated primary mirror, and the prism itself, a
    # faceted crystal held in a cage of four struts in front of the aperture.
    with k.at(x - .05, 0, z + 1.25, pitch=5):
        k.rev((-1.05, 0, 0), 'x', [(0, .3), (.12, .42), (.9, .46), (.9, .56), (1.75, .6), (1.75, .68), (1.95, .68)], rgb(74, 78, 80, 'dark'), n=8, smooth=False)
        for t in (.25, .55, 1.25): k.rev((-1.05 + t, 0, 0), 'x', [(0, .64 if t > 1 else .5), (.07, .64 if t > 1 else .5)], IRON, n=8, smooth=False)
        k.rev((.86, 0, 0), 'x', [(0, .56), (.02, .5)], BRASS, n=16)                       # mirror
        k.rev((.88, 0, 0), 'x', [(0, .36), (.02, .3)], GLASS, n=12)
        for i in range(4):
            a_ = pi / 4 + i * pi / 2
            k.tube((.9, .6 * cos(a_), .6 * sin(a_)), (1.5, .3 * cos(a_), .3 * sin(a_)), .03, STEEL, n=5)
        k.rev((1.2, 0, 0), 'x', [(0, .02), (.3, .34), (.42, .34), (.74, .02)], PLASMA, n=6, smooth=False)   # the prism
        k.rev((1.5, 0, 0), 'x', [(0, .36), (.12, .36)], STEEL, n=6, smooth=False)                           # its collar
        k.cbox(-.2, 0, .5, 1.1, .46, .05, TEAM)                                           # recognition panel
        for s_ in (-1, 1): k.box(-.9, s_ * .5 - .05, -.2, .5, .1, .4, DARK)             # elevation jacks
    for s_ in (-1, 1):
        k.pipe([(x - 1.5, s_ * .45, z + .6), (x - 1.25, s_ * .4, z + .95), (x - .9, s_ * .3, z + 1.2)], .055, RUBBER, n=6)
    k.cbox(x - 1.7, 0, z + .0, .6, .5, .5, tint(c, .85)); k.cyl(x - 1.7, 0, z + .5, .16, .12, DARK, n=10)   # generator exhaust
    antenna(k, x - 2.3, 1.0, z + .6, 2.0)

def tesla_tank(k):
    # Soviet Tesla tank: generator hull, twin discharge electrodes on porcelain insulator stacks.
    c = SOV_GREEN; L, W = 6.3, 3.2
    tracks(k, -L / 2, L / 2, W, .55, 6, .34, .95, c, rs=.3)
    tank_hull(k, L, W, .55, c, clear=.45, ttop=.95, deck=1.15, glacis=1.8, nose=.78, sponson=False)
    for s in (-1, 1):
        k.box(-L / 2 + .3, s * (W / 2 + .03) - .02, .55, L - 1.0, .04, .46, remat(tint(c, .8), 'rubber'))
        y0 = min(s * (W / 2 - .5), s * W / 2)
        k.box(-2.5, y0 + .04, 1.03, 2.6, .44, .24, tint(c, .92))
    headlights(k, L / 2 - 1.0, W, 1.0, c); hatch(k, 1.8, 0, 1.03, .3, c)
    x = -.4; z = 1.17
    k.stack(x, 0, [(z, 3.2, 2.2, .3), (z + .7, 3.0, 2.0, .35), (z + .8, 2.6, 1.6, .35)], c, k=3)
    k.cbox(x - 1.0, 0, z + .8, .9, 1.4, .35, tint(c, .88))  # generator housing
    for i in range(5): k.cbox(x - 1.38 + i * .19, 0, z + 1.15, .05, 1.3, .08, DARK)
    roof_panel(k, x + .2, 0, z + .8, .7, .9)
    for s in (-1, 1):
        k.box(x - 1.2, s * 1.02 - .02, z + .15, 1.3, .04, .4, TEAM)
        with k.at(x + .8, s * .62, z + .8, pitch=-26, s=1.18):
            k.cyl(0, 0, 0, .26, .2, DARK, n=10)
            k.rev((0, 0, .2), 'z', [(0, .19), (.5, .17)], rgb(150, 96, 66, 'copper'), n=12)   # primary winding
            for i in range(5): k.torus(0, 0, .26 + i * .09, .2, .022, COPPER, n=14, k=4)
            for i in range(5): k.rev((0, 0, .72 + i * .18), 'z', [(0, .09), (.05, .2), (.11, .2), (.18, .09)], rgb(204, 198, 184, 'paint'), n=12)   # porcelain
            k.cyl(0, 0, 1.62, .055, .22, COPPER, n=8)
            k.torus(0, 0, 1.82, .3, .1, rgb(196, 200, 204, 'steel'), n=18, k=6)   # corona ring
            k.ball(0, 0, 2.02, .19, TESLA, n=12, k=6)
            k.tube((0, 0, 2.15), (.3, -s * .12, 2.38), .02, COPPER, n=5); k.ball(.32, -s * .13, 2.4, .05, TESLA, n=6, k=3)   # spark gap electrode
        k.pipe([(x - .6, s * .5, z + 1.0), (x + .2, s * .6, z + 1.25), (x + .8, s * .6, z + .95)], .05, RUBBER, n=6)
    antenna(k, x - 1.4, .9, z + .8, 2.2)

# --- Wheeled vehicles ------------------------------------------------------------------------------
def cab(k, x0, x1, W, z0, h, c, slope=.35, roof=None):
    """Truck cab between x0 and x1: raked windscreen, side windows, roof."""
    k.loft([[(x0, -W / 2, z0), (x1, -W / 2, z0), (x1, W / 2, z0), (x0, W / 2, z0)],
            [(x0, -W / 2, z0 + h * .5), (x1, -W / 2, z0 + h * .5), (x1, W / 2, z0 + h * .5), (x0, W / 2, z0 + h * .5)],
            [(x0 + .05, -W / 2 + .1, z0 + h), (x1 - slope, -W / 2 + .1, z0 + h), (x1 - slope, W / 2 - .1, z0 + h), (x0 + .05, W / 2 - .1, z0 + h)]], c, smooth=False)
    e = .012
    k.plate([(x1 + e - .02, -W / 2 + .14, z0 + h * .54), (x1 + e - .02, W / 2 - .14, z0 + h * .54), (x1 - slope + e + .03, W / 2 - .2, z0 + h * .95), (x1 - slope + e + .03, -W / 2 + .2, z0 + h * .95)], GLASS)
    for s in (-1, 1):
        y = s * (W / 2 + e)
        k.plate([(x0 + .25, y - s * .02, z0 + h * .55), (x1 - .1, y - s * .02, z0 + h * .55), (x1 - slope, y - s * .1, z0 + h * .93), (x0 + .3, y - s * .1, z0 + h * .93)], GLASS)
    if roof: k.cbox((x0 + x1 - slope) / 2, 0, z0 + h, (x1 - slope - x0) * .8, W * .7, .03, roof)

def bonnet(k, x0, x1, W, z0, h, c):
    """Engine bonnet ahead of a cab: tapering hood, grille, bumper, headlights."""
    k.loft([[(x0, -W / 2, z0), (x1, -W / 2 + .1, z0), (x1, W / 2 - .1, z0), (x0, W / 2, z0)],
            [(x0, -W / 2 + .15, z0 + h), (x1, -W / 2 + .3, z0 + h * .85), (x1, W / 2 - .3, z0 + h * .85), (x0, W / 2 - .15, z0 + h)]], c, smooth=False)
    k.cbox(x1 + .01, 0, z0 + .1, .03, W * .5, h * .6, DARK)
    for i in range(5): k.cbox(x1 + .03, -W * .2 + i * W * .1, z0 + .12, .02, .03, h * .55, STEEL)
    k.cbox(x1 + .12, 0, z0 - .25, .18, W, .22, DARK)
    for s in (-1, 1):
        k.cbox(x1 + .03, s * (W / 2 - .28), z0 + .25, .04, .22, .22, LAMP)

def mudguards(k, xs, W, r, c, w=.5):
    for x in xs:
        for s in (-1, 1):
            y0 = min(s * (W / 2 - w), s * W / 2)
            pts = [(x + (r + .12) * cos(radians(a)), r + (r + .12) * sin(radians(a))) for a in (10, 50, 90, 130, 170)]
            pts += [(x + (r + .06) * cos(radians(a)), r + (r + .06) * sin(radians(a))) for a in (170, 130, 90, 50, 10)]
            k.extrude(pts, 'xz', y0, y0 + w, c)

def chassis(k, x0, x1, W, z, c=DARK):
    k.box(x0, -W * .28, z - .3, x1 - x0, W * .56, .3, c)
    for s in (-1, 1): k.box(x0, s * W * .3 - .06, z - .12, x1 - x0, .12, .14, IRON)

def scout(k):
    # Allied scout car, after the HMMWV M1025: 4.6 x 2.16 x 1.83 m, ring-mounted .50 cal.
    c = NATO_SAND; L, W = 4.7, 2.2; r = .47
    xs = [-1.55, 1.55]
    road_wheels(k, xs, W, r, c, .32)
    chassis(k, -2.2, 2.2, W, .95)
    k.extrude([(-2.3, .62), (2.25, .62), (2.35, .95), (2.3, 1.25), (1.0, 1.36), (-2.3, 1.36)], 'xz', -W / 2 + .34, W / 2 - .34, c)
    k.extrude([(-2.3, .98), (2.3, .98), (2.3, 1.25), (1.0, 1.36), (-2.3, 1.36)], 'xz', -W / 2, W / 2, c)
    mudguards(k, xs, W, r, c, .36)
    # Cab and slant-back.
    k.loft([[(-2.3, -W / 2, 1.36), (.95, -W / 2, 1.36), (.95, W / 2, 1.36), (-2.3, W / 2, 1.36)],
            [(-.95, -W / 2 + .12, 1.9), (.55, -W / 2 + .12, 1.9), (.55, W / 2 - .12, 1.9), (-.95, W / 2 - .12, 1.9)]], c, smooth=False)
    e = .015
    k.plate([(.95 + e, -W / 2 + .12, 1.4), (.95 + e, W / 2 - .12, 1.4), (.57 + e, W / 2 - .2, 1.86), (.57 + e, -W / 2 + .2, 1.86)], GLASS)
    for s in (-1, 1):
        y = s * (W / 2 + e)
        k.plate([(-.85, y - s * .03, 1.42), (.85, y - s * .03, 1.42), (.5, y - s * .11, 1.84), (-.8, y - s * .11, 1.84)], GLASS)
        k.box(-2.1, s * (W / 2) - .02, 1.02, 1.1, .04, .3, TEAM)
        k.cbox(2.33, s * .72, 1.02, .04, .22, .2, LAMP)
    k.cbox(2.36, 0, .7, .14, W, .2, DARK); k.cbox(2.34, 0, 1.0, .03, .9, .22, DARK)
    k.cbox(1.6, 0, 1.3, 1.2, 1.5, .03, tint(c, .9))  # bonnet louvres
    roof_panel(k, -.2, 0, 1.9, .8, 1.2)
    k.cyl(-.25, 0, 1.92, .42, .1, DARK, n=14)
    mg(k, -.25, 0, 2.0, yaw=0, length=1.3)
    k.plate([(.1, -.4, 2.05), (.1, .4, 2.05), (.1, .4, 2.55), (.1, -.4, 2.55)], c)  # gun shield
    k.box(-2.28, -.25, 1.4, .1, .5, .42, OLIVE); antenna(k, -2.1, .8, 1.36, 2.4)

def raider(k):
    # Soviet armoured scout car, after the BRDM-2: 5.75 x 2.35 x 2.31 m, 14.5 mm turret.
    c = SOV_GREEN; L, W = 5.75, 2.35; r = .54
    xs = [-1.55, 1.55]
    road_wheels(k, xs, W, r, c, .36)
    inner = W / 2 - .4
    k.extrude([(-2.85, 1.0), (-2.6, .55), (1.9, .55), (2.9, 1.15), (2.2, 1.5), (-2.85, 1.5)], 'xz', -inner, inner, c)
    # Faceted upper hull, narrowing to the roof.
    k.loft([[(-2.85, -W / 2, 1.15), (2.2, -W / 2, 1.15), (2.9, -.5, 1.15), (2.9, .5, 1.15), (2.2, W / 2, 1.15), (-2.85, W / 2, 1.15)],
            [(-2.85, -W / 2, 1.5), (2.1, -W / 2, 1.5), (2.5, -.5, 1.4), (2.5, .5, 1.4), (2.1, W / 2, 1.5), (-2.85, W / 2, 1.5)],
            [(-2.7, -W / 2 + .35, 1.95), (.9, -W / 2 + .35, 1.95), (1.3, -.45, 1.9), (1.3, .45, 1.9), (.9, W / 2 - .35, 1.95), (-2.7, W / 2 - .35, 1.95)]], c, smooth=False)
    mudguards(k, xs, W, r, c, .38)
    for s in (-1, 1):
        k.plate([(1.55, s * .2 - .16, 1.62), (1.55, s * .2 + .16, 1.62), (1.38, s * .2 + .16, 1.84), (1.38, s * .2 - .16, 1.84)], GLASS_DARK)
        k.box(-2.5, s * (W / 2) - .02, 1.2, 1.3, .04, .26, TEAM)
        k.cbox(2.55, s * .75, 1.2, .1, .2, .18, LAMP)
    engine_deck(k, -2.0, W * .7, 1.95, c, 1, 1.0)
    k.cyl(-.2, 0, 1.95, .62, .12, DARK, n=16)
    k.rev((-.2, 0, 2.05), 'z', [(0, .6), (.4, .42), (.46, .3)], c, n=14)
    roof_panel(k, -.2, 0, 2.51, .36, .36)
    with k.at(.25, 0, 2.28, pitch=4):
        k.tube((0, 0, 0), (1.5, 0, 0), .035, GUN, n=6); k.tube((1.2, 0, 0), (1.5, 0, 0), .05, GUN, n=6)
    antenna(k, -2.5, .7, 1.95, 2.4)

def ifv(k):
    # Allied infantry fighting vehicle, after the LAV-25 / Stryker: 6.9 x 2.7 x 2.7 m, eight
    # wheels, 25 mm turret with a twin missile launcher.
    c = NATO_SAND; L, W = 6.9, 2.7; r = .52
    xs = [-2.45, -1.15, .75, 2.05]
    road_wheels(k, xs, W, r, c, .34)
    inner = W / 2 - .38
    k.extrude([(-3.4, 1.0), (-3.2, .5), (2.4, .5), (3.45, 1.15), (2.0, 1.85), (-3.4, 1.85)], 'xz', -inner, inner, c)
    k.loft([[(-3.4, -W / 2, 1.12), (2.6, -W / 2, 1.12), (3.45, -.8, 1.15), (3.45, .8, 1.15), (2.6, W / 2, 1.12), (-3.4, W / 2, 1.12)],
            [(-3.4, -W / 2 + .2, 1.85), (1.9, -W / 2 + .2, 1.85), (2.1, -.7, 1.8), (2.1, .7, 1.8), (1.9, W / 2 - .2, 1.85), (-3.4, W / 2 - .2, 1.85)]], c, smooth=False)
    for s in (-1, 1):
        k.box(-3.2, s * (W / 2) - .02, 1.2, 1.8, .04, .3, TEAM)
        k.cbox(3.0, s * .95, 1.3, .1, .2, .18, LAMP)
        k.box(-1.0, s * (W / 2 - .12) - .12, 1.5, 1.6, .24, .28, tint(c, .92))  # stowage
    k.cbox(2.2, -.6, 1.82, .9, .8, .05, DARK); hatch(k, 2.2, .6, 1.84, .28, c)
    k.cbox(-3.42, 0, 1.0, .06, 1.6, .8, tint(c, .9))  # rear ramp
    x = .1; z = 1.87
    k.cyl(x, 0, z - .05, .85, .08, DARK, n=18)
    k.stack(x, 0, [(z, 2.0, 1.7, .3), (z + .45, 1.9, 1.6, .35), (z + .58, 1.5, 1.3, .4)], c, k=3, smooth=False)
    with k.at(x + .9, 0, z + .3, pitch=3):
        k.tube((0, 0, 0), (2.3, 0, 0), .045, GUN, n=8); k.rev((1.9, 0, 0), 'x', [(0, .07), (.4, .07)], GUN, n=8)
    with k.at(x - .1, -1.0, z + .45, pitch=8):  # twin missile launcher
        k.cbox(0, 0, -.15, 1.3, .32, .55, tint(c, .9))
        for j in range(2): k.rev((.66, 0, -.02 + j * .26), 'x', [(0, .1), (.02, .1)], BLACK, n=8)
    cupola(k, x - .2, .4, z + .58, c, .3); hatch(k, x - .2, -.4, z + .58, .27, c)
    roof_panel(k, x - 1.9, 0, 1.85, 1.2, 1.7)
    for s in (-1, 1): smoke_launchers(k, x + .5, s * .88, z + .2, s, n=4)
    antenna(k, -3.0, .9, 1.85, 2.4)

def flak_track(k):
    # Soviet half-track with a twin 23 mm anti-aircraft mount (ZU-23-2) on an armoured bed.
    c = SOV_GREEN; L, W = 6.4, 2.5; r = .5
    road_wheels(k, [2.1], W, r, c, .32)
    for s in (-1, 1):
        ya, yb = sorted([s * (W / 2 - .42), s * W / 2])
        k.extrude(track_outline(-3.1, .6, .9, .28), 'xz', ya, yb, TRACK)
        for i in range(4): k.wheel(-2.25 + i * .68, s * (W / 2 - .1), .3, .3, .24, tyre=RUBBER, hub=c)
        k.wheel(-2.82, s * (W / 2 - .1), .62, .23, .24, tyre=IRON, hub=DARK, n=10); k.wheel(.32, s * (W / 2 - .1), .62, .23, .24, tyre=IRON, hub=c, n=10)
    chassis(k, -3.1, 3.1, W, 1.05)
    bonnet(k, 1.5, 3.1, W * .82, 1.0, .75, c)
    mudguards(k, [2.1], W, r, c, .34)
    cab(k, .3, 1.5, W * .92, 1.0, 1.35, c, slope=.22)
    roof_panel(k, .85, 0, 2.35, .7, 1.4)
    # Armoured bed with drop sides.
    k.box(-3.15, -W / 2, 1.0, 3.4, W, .12, c)
    for s in (-1, 1):
        y0 = s * W / 2 - (.06 if s > 0 else 0)
        k.box(-3.15, y0, 1.12, 3.4, .06, .62, c); k.box(-3.0, s * (W / 2 + .01) - .02, 1.25, 1.6, .04, .28, TEAM)
    k.box(-3.15, -W / 2, 1.12, .06, W, .62, c)
    # The gun: rotating base, twin barrels with flash hiders, ammunition boxes, gunner's seat, shield.
    k.cyl(-1.5, 0, 1.12, .7, .15, DARK, n=16)
    with k.at(-1.5, 0, 1.3, yaw=0):
        k.cbox(0, 0, 0, .7, .9, .5, tint(c, .9))
        with k.at(.1, 0, .55, pitch=38):
            for s in (-1, 1):
                k.box(-.5, s * .2 - .08, -.08, .9, .16, .18, GUN)
                k.tube((.3, s * .2, 0), (2.3, s * .2, 0), .035, GUN, n=6)
                k.rev((2.1, s * .2, 0), 'x', [(0, .035), (.08, .07), (.3, .055)], GUN, n=6)
                k.box(-.3, s * .5 - .12, -.2, .5, .24, .36, OLIVE)
        k.plate([(.42, -.75, .2), (.42, .75, .2), (.3, .75, 1.0), (.3, -.75, 1.0)], c)
        k.cbox(-.55, .25, .3, .3, .3, .3, DARK)
    antenna(k, .4, 1.0, 2.35, 2.2)

def truck(k, c, L=7.3, W=2.5, r=.6, xs=(-2.6, -1.2, 2.2), bed=True, bed_h=.55):
    """Soviet 6x6 cab-behind-engine truck (Ural-4320): 7.37 x 2.5 x 2.87 m."""
    road_wheels(k, xs, W, r, c, .4)
    chassis(k, -L / 2, L / 2 - .2, W, 1.25)
    bonnet(k, 1.55, L / 2, W * .8, 1.2, .85, c)
    mudguards(k, [xs[-1]], W, r, c, .42)
    cab(k, .2, 1.55, W * .94, 1.2, 1.5, c, slope=.25)
    k.cbox(.05, -.8, 1.3, .3, .5, .9, DARK)  # spare wheel carrier
    k.rev((.05, .55, 1.9), 'x', [(0, .58), (.06, .6), (.3, .6), (.36, .58)], TYRE, n=14)
    k.tube((-.2, -W / 2 + .1, 1.0), (.1, -W / 2 + .1, 1.0), .3, tint(c, .9), n=10)  # fuel tank
    if bed:
        k.box(-L / 2, -W / 2, 1.3, L / 2 + .05, W, .1, c)
        for s in (-1, 1):
            y0 = s * W / 2 - (.05 if s > 0 else 0); k.box(-L / 2, y0, 1.4, L / 2 + .05, .05, bed_h, c)
        k.box(-L / 2, -W / 2, 1.4, .05, W, bed_h, c); k.box(0, -W / 2, 1.4, .05, W, bed_h + .2, c)

def rocket_launcher(k):
    # Soviet multiple rocket launcher, after the BM-21 Grad: forty 122 mm tubes on a Ural 6x6.
    c = SOV_GREEN; W = 2.5
    truck(k, c, bed=False)
    roof_panel(k, .8, 0, 2.7, .8, 1.5)
    k.box(-3.65, -W / 2 + .1, 1.3, 3.7, W - .2, .15, c)
    for s in (-1, 1): k.box(-3.3, s * (W / 2 - .1) - .02, 1.32, 2.6, .04, .12, TEAM)
    k.cyl(-1.9, 0, 1.45, .8, .3, DARK, n=16)
    for s in (-1, 1): k.box(-2.3, s * .85 - .08, 1.75, .9, .16, .6, tint(c, .9))  # cradle cheeks
    with k.at(-1.9, 0, 2.3, pitch=22):
        k.cbox(.2, 0, -.32, 3.0, 2.1, .7, tint(c, .95))
        for i in range(10):
            for j in range(4):
                y = -.9 + i * .2; z = -.24 + j * .18
                k.rev((1.7, y, z), 'x', [(0, .075), (.06, .075)], GUN, n=8)
                k.rev((1.76, y, z), 'x', [(0, .055), (.01, .055)], BLACK, n=6)
        for xb in (-.9, .3, 1.3): k.cbox(xb, 0, -.34, .08, 2.16, .76, DARK)
        k.cbox(.2, 0, .385, 1.8, 1.4, .03, TEAM)
    antenna(k, .35, 1.0, 2.7, 2.2)

def demolition_truck(k):
    # A Ural cargo truck carrying a single large demolition charge, lashed down and placarded.
    c = OLIVE; W = 2.5
    truck(k, c, bed=True, bed_h=.5)
    roof_panel(k, .8, 0, 2.7, .8, 1.5)
    for s in (-1, 1): k.box(-3.4, s * (W / 2 + .01) - .02, 1.5, 2.6, .04, .3, HAZ)
    k.rev((-3.3, 0, 2.2), 'x', [(0, .2), (.3, .7), (.7, .78), (2.4, .78), (2.9, .6), (3.1, .3)], rgb(196, 190, 60), n=16)  # the charge
    for xb in (-2.5, -1.4): k.rev((xb, 0, 2.2), 'x', [(0, .8), (.1, .8)], IRON, n=16)
    k.rev((-3.32, 0, 2.2), 'x', [(0, .22), (.03, .22)], SIGNAL_RED, n=10)
    for xb in (-2.5, -1.4):
        for s in (-1, 1): k.tube((xb + .05, s * .78, 2.2), (xb + .05, s * 1.2, 1.4), .025, IRON, n=4)  # lashings
    for i in range(3): k.cyl(-.5, -.8 + i * .8, 1.4, .29, .88, SIGNAL_RED if i == 1 else rgb(60, 80, 110), n=12)
    k.cbox(.82, 0, 2.73, .25, 1.0, .14, ORANGE)  # beacon bar
    antenna(k, .35, 1.0, 2.7, 2.0, False)

def v3_launcher(k):
    # Soviet ballistic missile launcher, after the 9P117 (MAZ-543) with an R-17 "Scud":
    # 13.4 x 3.0 x 3.3 m in reality, shortened here to stay inside the sprite.
    c = SOV_GREEN; L, W = 11.2, 3.0; r = .72
    xs = [-3.9, -2.2, 1.6, 3.3]
    road_wheels(k, xs, W, r, c, .46)
    chassis(k, -L / 2, L / 2, W, 1.4)
    k.box(-L / 2, -W / 2, 1.4, L - 2.4, W, .5, c)
    for s in (-1, 1):
        # Split cabs either side of the missile's nose.
        with k.at(0, s * 1.02, 0): cab(k, 3.2, 5.5, .95, 1.4, 1.4, c, slope=.5)
        roof_panel(k, 4.1, s * 1.02, 2.8, 1.0, .6)
        k.box(-4.8, s * (W / 2 + .01) - .02, 1.5, 3.2, .04, .3, TEAM)
        k.box(-1.2, s * 1.0 - .45, 1.9, 2.6, .9, .7, tint(c, .92))  # equipment lockers
        k.cbox(5.5, s * 1.0, 1.55, .06, .3, .25, LAMP)
    k.cbox(5.52, 0, 1.2, .14, W, .25, DARK)
    # Erector cradle and the missile, raised for firing.
    with k.at(-4.9, 0, 2.0, pitch=24):
        k.truss((0, 0, -.25), (8.6, 0, -.25), 1.0, .3, IRON, bays=9, t=.07)
        m = rgb(92, 104, 84)
        k.rev((0, 0, .45), 'x', [(0, .3), (.2, .44), (7.6, .44), (9.2, .2), (9.8, .02)], m, n=16)
        k.rev((9.2, 0, .45), 'x', [(0, .21), (.6, .02)], SIGNAL_RED, n=12)
        for i in range(4):
            a = i * pi / 2 + pi / 4
            k.plate([(.1, .44 * cos(a), .45 + .44 * sin(a)), (1.3, .44 * cos(a), .45 + .44 * sin(a)), (.5, 1.0 * cos(a), .45 + 1.0 * sin(a)), (.1, 1.0 * cos(a), .45 + 1.0 * sin(a))], m)
        for xb in (1.6, 4.4, 7.0): k.torus(xb, 0, .45, .46, .04, IRON, n=14, k=4, axis='x')
        k.rev((3.0, 0, .45), 'x', [(0, .445), (.5, .445)], TEAM, n=16)
    for s in (-1, 1): k.tube((-2.0, s * .7, 1.9), (-1.0, s * .55, 3.3), .09, STEEL, n=8)  # hydraulic rams
    antenna(k, 3.4, 1.4, 2.8, 2.2)

def mcv(k):
    # Mobile construction vehicle: a 8x8 heavy tractor (after the Oshkosh HEMTT, 10.2 x 2.4 m,
    # widened) carrying the folded yard: site module, knuckle-boom crane and outriggers.
    c = NATO_SAND; L, W = 10.0, 3.3; r = .68
    xs = [-3.8, -2.3, 1.6, 3.1]
    road_wheels(k, xs, W, r, c, .5)
    chassis(k, -L / 2, L / 2, W, 1.4)
    with k.at(0, 0, 0): cab(k, 3.4, 5.0, W * .95, 1.25, 1.75, c, slope=.3)
    roof_panel(k, 4.1, 0, 3.0, .9, 2.0)
    k.cbox(5.02, 0, 1.0, .14, W, .3, DARK)
    for s in (-1, 1): k.cbox(5.02, s * 1.2, 1.4, .05, .3, .25, LAMP); k.cbox(4.2, s * 1.0, 3.03, .2, .15, .12, ORANGE)
    k.box(2.3, -W / 2 + .2, 1.4, 1.0, W - .4, 1.5, tint(c, .86))  # power pack
    for i in range(5): k.box(2.35 + i * .19, -W / 2 + .18, 2.0, .06, W - .36, .7, DARK)
    k.tube((2.6, W / 2 - .3, 2.9), (2.6, W / 2 - .3, 3.7), .09, STEEL, n=8)
    # The yard module: a ribbed container body with team doors.
    k.box(-4.9, -W / 2, 1.4, 7.0, W, 1.9, remat(c, 'corr'))
    k.box(-4.92, -W / 2 + .1, 1.5, .04, W - .2, 1.7, TEAM)
    for s in (-1, 1):
        k.box(-4.4, s * (W / 2 + .01) - .02, 2.5, 6.0, .04, .5, TEAM)
        for xb in (-4.2, 1.6):  # folded outriggers
            k.box(xb, s * W / 2 - (.16 if s < 0 else 0), .7, .3, .16, 1.9, YELLOW)
            k.cbox(xb + .15, s * (W / 2 + .05), .5, .5, .5, .1, IRON)
    k.box(-4.9, -W / 2, 3.3, 7.0, W, .08, tint(c, .8))
    # Knuckle-boom crane folded along the roof, and a stack of steel.
    k.cyl(-3.9, 0, 3.38, .45, .6, YELLOW, n=12)
    with k.at(-3.9, 0, 3.9, pitch=6):
        k.box(0, -.22, -.2, 4.6, .44, .44, YELLOW); k.box(1.0, -.16, .24, 3.9, .32, .3, CAT_YELLOW)
        k.tube((.4, 0, .5), (2.4, 0, .44), .08, CHROME, n=8)
    k.tube((.9, 0, 4.25), (.9, 0, 3.55), .02, DARK, n=4); k.cbox(.9, 0, 3.4, .2, .2, .2, YELLOW)
    for i in range(4): k.box(-1.2, -1.2 + i * .34, 3.38, 3.0, .26, .16 + (i % 2) * .06, RUST)
    k.cbox(-4.3, 1.0, 3.38, .8, .8, .5, GALV); k.cbox(-4.3, -1.0, 3.38, .6, .6, .7, tint(c, .9))
    antenna(k, 3.5, 1.4, 3.0, 2.2)

# --- Ore miners -------------------------------------------------------------------------------------
def ore_load(k, x0, x1, w, z, seed=3):
    import random
    rnd = random.Random(seed)
    k.cbox((x0 + x1) / 2, 0, z - .5, x1 - x0, w, .5, tint(ORE, .7))
    for i in range(38):
        px, py = rnd.uniform(x0 + .2, x1 - .2), rnd.uniform(-w / 2 + .2, w / 2 - .2)
        hump = 1 - ((px - (x0 + x1) / 2) / (x1 - x0) * 2) ** 2 * .6 - (py / w * 2) ** 2 * .6
        k.ball(px, py, z + hump * .45, rnd.uniform(.2, .36), tint(ORE, rnd.uniform(.8, 1.15)), rz=.22, n=6, k=3)

def dump_body(k, x0, x1, W, z, h, c, canopy=1.6):
    """Haul-truck body: V floor, ribbed sides, a canopy reaching over the cab."""
    k.loft([[(x0, -W / 2 + .5, z), (x1, -W / 2 + .5, z), (x1, W / 2 - .5, z), (x0, W / 2 - .5, z)],
            [(x0 - .5, -W / 2, z + h), (x1, -W / 2, z + h), (x1, W / 2, z + h), (x0 - .5, W / 2, z + h)]], c, smooth=False)
    k.box(x1 - .1, -W / 2, z + h - .1, canopy, W, .14, c)
    k.box(x1 - .12, -W / 2, z + .2, .14, W, h - .2, c)
    n = int((x1 - x0) / .7)
    for s in (-1, 1):
        for i in range(n + 1):
            xa = x0 + i * (x1 - x0) / n
            k.tube((xa, s * (W / 2 - .48), z), (xa - .25, s * (W / 2 + .02), z + h), .07, tint(c, .85), n=4)
        k.box(x0 - .4, s * (W / 2 + .02) - .03, z + h - .5, x1 - x0, .06, .4, TEAM)

def harvester(k):
    # Ore truck: a rigid mining hauler (after the Cat 777: 9.8 x 6.1 x 5.1 m, scaled down) with a
    # front-mounted scraper intake that feeds the body.
    c = CAT_YELLOW; L, W = 8.6, 4.2; r = .95
    road_wheels(k, [2.2], W, r, c, .7); road_wheels(k, [-2.3], W, r, c, .62, dual=(0,))
    chassis(k, -3.9, 3.6, W, 1.7, IRON)
    k.box(1.5, -1.5, 1.5, 2.3, 3.0, 1.0, c)  # engine housing
    for i in range(7): k.box(3.82, -1.2 + i * .37, 1.6, .04, .22, .8, DARK)
    k.box(3.6, -W / 2 + .2, 1.2, .5, W - .4, .35, IRON)  # bumper
    for s in (-1, 1): k.cbox(3.85, s * 1.55, 2.2, .05, .4, .3, LAMP); k.box(1.4, s * 1.7 - .25, 2.5, 2.2, .5, .08, IRON)
    k.rail([(1.5, -1.9, 2.58), (3.6, -1.9, 2.58), (3.6, 1.9, 2.58)], YELLOW, h=.9)
    with k.at(0, .95, 0): cab(k, 1.7, 3.0, 1.5, 2.5, 1.5, WHITE, slope=.2, roof=TEAM)
    k.ladder(3.5, -1.2, 1.3, 2.55, axis='y')
    dump_body(k, -3.9, 1.3, W, 2.0, 1.9, c, canopy=2.0)
    ore_load(k, -3.9, 1.1, W - .8, 3.75)
    # Scraper intake: a toothed drum between two arms.
    for s in (-1, 1): k.box(3.7, s * 1.6 - .12, .5, 1.7, .24, .4, IRON)
    k.rev((5.2, -1.9, .65), 'y', [(0, .55), (3.8, .55)], IRON, n=12)
    for i in range(12):
        for j in range(8):
            a = j * pi / 4 + (i % 2) * pi / 8
            k.cbox(5.2 + .58 * cos(a), -1.75 + i * .32, .65 + .58 * sin(a) - .06, .14, .12, .12, STEEL)
    k.box(4.6, -1.95, 1.0, 1.0, 3.9, .5, c)

def chrono_miner(k):
    # Allied chrono miner: an articulated six-wheel hauler with a loader bucket; the chrono
    # drive's coil ring sits behind the cab.
    c = NATO_SAND; L, W = 8.4, 3.6; r = .85
    road_wheels(k, [2.6, -.9, -2.9], W, r, c, .6)
    chassis(k, -3.9, 3.4, W, 1.6, IRON)
    k.box(2.0, -1.3, 1.4, 1.9, 2.6, .9, c)
    for i in range(6): k.box(3.92, -1.0 + i * .37, 1.5, .04, .22, .7, DARK)
    with k.at(0, 0, 0): cab(k, .9, 2.3, 2.2, 2.3, 1.45, c, slope=.3, roof=TEAM)
    for s in (-1, 1): k.cbox(3.95, s * 1.3, 2.0, .05, .35, .25, LAMP)
    mudguards(k, [2.6], W, r, c, .62)
    dump_body(k, -3.9, .2, W, 1.9, 1.6, c, canopy=.5)
    ore_load(k, -3.8, 0.0, W - .8, 3.35, seed=5)
    # Chrono ring: copper coil around a glowing core.
    k.torus(.55, 0, 3.6, .75, .12, COPPER, n=20, k=6, axis='x')
    k.ball(.55, 0, 3.6, .38, PLASMA, n=12, k=6)
    for s in (-1, 1): k.tube((.55, s * .75, 3.6), (.55, s * 1.0, 2.6), .07, STEEL, n=6)
    # Loader arms and bucket.
    for s in (-1, 1):
        k.tube((2.6, s * 1.2, 1.9), (4.6, s * 1.2, .9), .13, c, n=6)
        k.tube((3.0, s * 1.05, 1.5), (4.2, s * 1.05, 1.15), .06, CHROME, n=6)
    k.extrude([(4.4, .25), (5.6, .2), (5.7, .35), (4.9, .6), (4.9, 1.35), (4.4, 1.4)], 'xz', -1.9, 1.9, c)
    for i in range(9): k.box(5.55, -1.8 + i * .44, .18, .28, .12, .1, STEEL)

def war_miner(k):
    # Soviet war miner: an armoured, tracked ore carrier with a crusher drum and a machine-gun
    # cupola.
    c = SOV_GREEN; L, W = 8.2, 4.0
    tracks(k, -L / 2, L / 2 - .6, W, .75, 7, .38, 1.1, c, rs=.36)
    inner = W / 2 - .8
    k.extrude([(-L / 2 + .1, 1.1), (-L / 2 + .4, .55), (2.6, .55), (3.4, 1.1), (3.2, 2.2), (-L / 2 + .1, 2.2)], 'xz', -inner, inner, c)
    k.box(-L / 2 + .1, -W / 2, 1.15, L - 1.0, W, .12, c)
    skirts(k, L, W, c, .6, 1.2, 7, x1=3.0)
    # Armoured cab with vision slits.
    k.stack(2.2, 0, [(1.27, 2.2, 3.4, .1), (2.6, 2.1, 3.3, .1), (2.9, 1.5, 2.8, .2, -.2)], c, k=2, smooth=False)
    for y in (-1.0, 0, 1.0): k.cbox(3.3, y, 2.25, .04, .6, .2, GLASS_DARK)
    for s in (-1, 1):
        k.cbox(2.3, s * 1.72, 2.25, .9, .04, .2, GLASS_DARK); k.cbox(3.32, s * 1.3, 1.5, .05, .3, .25, LAMP)
        k.box(-3.6, s * (W / 2 + .01) - .02, 2.2, 4.6, .04, .4, TEAM)
    roof_panel(k, 2.0, 0, 2.9, 1.0, 2.0)
    k.cyl(2.0, -.7, 2.93, .4, .25, tint(c, .9), n=12); mg(k, 2.0, -.7, 3.1, yaw=0, length=1.4)
    # Ore bin.
    k.loft([[(-3.9, -1.4, 1.27), (1.0, -1.4, 1.27), (1.0, 1.4, 1.27), (-3.9, 1.4, 1.27)],
            [(-4.0, -W / 2, 2.9), (1.1, -W / 2, 2.9), (1.1, W / 2, 2.9), (-4.0, W / 2, 2.9)]], c, smooth=False)
    for i in range(7):
        for s in (-1, 1): k.tube((-3.8 + i * .8, s * 1.42, 1.3), (-3.9 + i * .8, s * (W / 2 + .02), 2.9), .07, tint(c, .85), n=4)
    ore_load(k, -3.8, .9, W - .6, 2.75, seed=9)
    # Crusher drum.
    for s in (-1, 1): k.box(3.2, s * 1.5 - .15, .5, 1.6, .3, .5, IRON)
    k.rev((4.6, -1.9, .7), 'y', [(0, .6), (3.8, .6)], IRON, n=12)
    for i in range(12):
        for j in range(8):
            a = j * pi / 4 + (i % 2) * pi / 8
            k.cbox(4.6 + .63 * cos(a), -1.75 + i * .32, .7 + .63 * sin(a) - .06, .15, .12, .12, STEEL)
    k.box(3.9, -1.95, 1.2, 1.2, 3.9, .5, c)
    for y in (-1.2, 1.2): k.tube((-4.05, y, 2.2), (-4.05, y, 3.5), .12, DARK, n=8)  # exhaust stacks
    antenna(k, 1.4, 1.5, 2.9, 2.0)

def terror_drone(k):
    # Soviet terror drone: a four-legged walking machine about a metre across (drawn larger).
    c = rgb(96, 98, 94, 'steel')
    with k.at(0, 0, 0, s=2.2):
        k.ball(0, 0, .5, .3, c, rz=.2, n=14, k=6)
        k.cyl(0, 0, .62, .16, .08, TEAM, n=10)
        k.ball(.26, 0, .52, .08, RED_LAMP, n=8, k=4)
        for i in range(4):
            a = pi / 4 + i * pi / 2
            hx, hy = .26 * cos(a), .26 * sin(a); kx, ky = .62 * cos(a), .62 * sin(a); fx, fy = .78 * cos(a), .78 * sin(a)
            k.tube((hx, hy, .5), (kx, ky, .78), .045, DARK, n=6); k.ball(kx, ky, .78, .06, c, n=6, k=3)
            k.tube((kx, ky, .78), (fx, fy, 0), .035, c, n=6, r2=.012)
        for s in (-1, 1): k.tube((.25, s * .08, .44), (.46, s * .14, .34), .02, STEEL, n=4, r2=.005)

MODELS = {f.__name__: f for f in [grizzly, light_tank, rhino, heavy_tank, apocalypse, tank_destroyer, artillery,
                                    mirage_tank, prism_tank, tesla_tank, scout, raider, ifv, flak_track,
                                    rocket_launcher, demolition_truck, v3_launcher, mcv, harvester,
                                    chrono_miner, war_miner, terror_drone]}

def build(m, ident):
    MODELS[ident](Kit(m, U))
