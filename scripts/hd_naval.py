"""Ships, craft and sea creatures, modelled in metres from real references (in the comments).

Everything floats: the waterline is z = 0 and a hull is modelled from 0.3 m below it upwards.
A warship is far longer than the 0.93 cell reach of the sprite frame allows, so each one keeps
its true beam, deck heights and equipment sizes and is shortened lengthwise (the comment on each
gives the real and the drawn length); `SCALE` then sets cells per metre for each model.
Allied ships are haze grey with dark grey decks, Soviet ones a greener grey with red-brown
decks. Ownership is carried the way navies mark ships: the ensign, a funnel band, the pennant
number panel on the bow, the landing circle of the flight deck, in the owner's colour (`TEAM`).
"""
from math import sin, cos, pi, radians, sqrt
from hd_kit import *

SCALE = {'destroyer': .0145, 'aegis_cruiser': .0165, 'aircraft_carrier': .0086, 'amphibious_transport': .045,
         'dolphin': .215, 'typhoon': .0222, 'sea_scorpion': .026, 'dreadnought': .013, 'giant_squid': .105}

SOV_GREY = rgb(108, 118, 118); SOV_DECK = rgb(116, 72, 58); MAST_GREY = rgb(62, 64, 66)
# Wet surfaces take the 'skin' class: it is the one satin finish that gathers no dust.
WET_BLACK = rgb(26, 27, 29, 'skin'); WET_RED = rgb(112, 46, 38, 'skin')

# --- Vector helpers ----------------------------------------------------------------------------
def _mix(a, b, f): return tuple(a[i] + (b[i] - a[i]) * f for i in range(3))
def _sub(a, b): return tuple(a[i] - b[i] for i in range(3))
def _add(a, b, f=1.0): return tuple(a[i] + b[i] * f for i in range(3))
def _dot(a, b): return sum(a[i] * b[i] for i in range(3))
def _len(a): return sqrt(_dot(a, a))
def _unit(a):
    l = _len(a) or 1.0
    return tuple(q / l for q in a)
def _cross(a, b): return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])
def _px(k): return 1 / (181 * k.u)  # metres covered by one texel of the sprite

# --- Hull --------------------------------------------------------------------------------------
class Hull:
    """Shape of a hull: deck half-beam hb(x), deck height zd(x), shell point side(x, f, s)."""

def ship_hull(k, x0, x1, B, z_aft, z_bow, c, deck, entry=.42, run=.2, stern=.84, wl=.88, fine=.45, rake=6.0,
              sheer=.35, flare=1.7, bluff=1.75, red=WET_RED, boot=WET_BLACK, zr=.7, zb=1.6, n_bow=8, step=None, knuckle=None):
    """Hull from the transom at x0 to the stem head at x1, beam B: flared sides, raked stem,
    sheer rising to the bow, red antifouling and black boot-topping at the waterline.
    step = (x, dz): the weather deck drops by dz abaft x (a raised forecastle).
    knuckle = f: the flare ends in a hard knuckle at that fraction of the freeboard."""
    L = x1 - x0; H = Hull(); H.x0, H.x1, H.B = x0, x1, B
    def tt(x): return (x - x0) / L
    def hb(x):
        t = tt(x)
        if t >= 1 - entry: return B / 2 * max(0.0, 1 - ((t - (1 - entry)) / entry) ** bluff)
        if t < run: return B / 2 * (1 - (1 - stern) * (1 - t / run) ** 2)
        return B / 2
    def zd(x):
        z = z_aft + (z_bow - z_aft) * max(0.0, (tt(x) - sheer) / (1 - sheer)) ** 2
        return z - step[1] if step and x < step[0] else z
    def side(x, f, s=1, eps=0.0):
        t = tt(x); sb = max(0.0, (t - (1 - entry)) / entry)
        hw = hb(x) * (wl - (wl - fine) * sb); xw = x - rake * sb ** 1.5
        ff = max(0.0, f); fy = min(1.0, ff / knuckle) if knuckle else ff
        return (xw + (x - xw) * ff, s * (hw + (hb(x) - hw) * fy ** flare + eps), zd(x) * f)
    H.hb, H.zd, H.side = hb, zd, side
    ts = [0, .05, .1, run, (run + 1 - entry) / 2, 1 - entry] + [1 - entry + entry * j / n_bow for j in range(1, n_bow + 1)]
    xs = sorted(set([x0 + L * t for t in ts] + ([step[0] - .01, step[0]] if step else [])))
    def lev(x): z = zd(x); return [-.3 / z, zr / z, zb / z, knuckle or .55, 1.0]
    cols = [red, boot, c, c]
    with k.grp(True):
        for s in (-1, 1):
            for xa, xb in zip(xs, xs[1:]):
                la, lb = lev(xa), lev(xb)
                for i in range(4):
                    k.poly([side(xa, la[i], s), side(xb, lb[i], s), side(xb, lb[i + 1], s), side(xa, la[i + 1], s)], cols[i])
        la = lev(x0)
        k.poly([side(x0, f, -1) for f in la] + [side(x0, f, 1) for f in la[::-1]], c)
    with k.grp(False):
        for xa, xb in zip(xs, xs[1:]):
            k.poly([side(xa, 1, -1), side(xa, 1, 1), side(xb, 1, 1), side(xb, 1, -1)], deck)
    return H

def deck_edge(H, xa, xb, s, inset=.5, step=5.0, dz=0.0):
    n = max(1, int(round(abs(xb - xa) / step)))
    return [(xa + (xb - xa) * i / n, s * max(0.0, H.hb(xa + (xb - xa) * i / n) - inset), H.zd(xa + (xb - xa) * i / n) + dz) for i in range(n + 1)]

def rail(k, pts, c, h, t):
    """Guard rail: a top wire on stanchions (drawn a texel thick so that it survives)."""
    for a, b in zip(pts, pts[1:]): k.strut((a[0], a[1], a[2] + h), (b[0], b[1], b[2] + h), t, c)
    for p in pts: k.strut(p, (p[0], p[1], p[2] + h), t, c)

def rails(k, H, xa, xb, c, inset=.5, step=5.0, sides=(-1, 1)):
    px = _px(k)
    for s in sides: rail(k, deck_edge(H, xa, xb, s, inset, step), c, 3.0 * px, .75 * px)

def bulwark(k, H, xa, h, c):
    """Solid bulwark round the bow from xa to the stem."""
    n = 6
    xs = [xa + (H.x1 - xa) * i / n for i in range(n + 1)]
    with k.grp(True):
        for s in (-1, 1):
            for a, b in zip(xs, xs[1:]):
                pa, pb = H.side(a, 1, s), H.side(b, 1, s)
                fa, fb = 1 + h / H.zd(a) * min(1, (a - xa) / (H.x1 - xa) * 4 + .3), 1 + h / H.zd(b) * min(1, (b - xa) / (H.x1 - xa) * 4 + .3)
                k.poly([pa, pb, H.side(b, fb, s), H.side(a, fa, s)], c)

def anchors(k, H, x, c=DARK):
    """Stockless anchors housed in their hawsepipes, chains led aft over the forecastle."""
    px = _px(k)
    for s in (-1, 1):
        p = H.side(x, .74, s, .15)
        k.cbox(p[0], p[1], p[2] - 1.4, 1.6, .5, 2.6, c); k.cbox(p[0], p[1], p[2] - 1.6, 2.6, .5, .7, c)
        d = H.side(x - 1, 1, s); cap = (x - 9, s * 1.6, H.zd(x - 9))
        k.strut((d[0], d[1] - s * 1.2, d[2] + .1), (cap[0], cap[1], cap[2] + .2), .8 * px, IRON)
        k.cyl(cap[0], cap[1], cap[2], .8, .9, c, n=8)

def hull_panel(k, H, xa, xb, fa, fb, c):
    """Painted panel on both sides of the shell (pennant number, draught marks)."""
    for s in (-1, 1):
        with k.grp():
            xs = [xa + (xb - xa) * i / 3 for i in range(4)]
            for a, b in zip(xs, xs[1:]):
                k.poly([H.side(a, fa, s, .12), H.side(b, fa, s, .12), H.side(b, fb, s, .12), H.side(a, fb, s, .12)], c)

# --- Superstructure ----------------------------------------------------------------------------
def outline(x0, x1, w, ch=0.0):
    """Deckhouse plan from x0 (aft) to x1, w wide; ch chamfers the four corners."""
    if not ch: return [(x0, -w / 2), (x1, -w / 2), (x1, w / 2), (x0, w / 2)]
    return [(x0 + ch, -w / 2), (x1 - ch, -w / 2), (x1, -w / 2 + ch), (x1, w / 2 - ch),
            (x1 - ch, w / 2), (x0 + ch, w / 2), (x0, w / 2 - ch), (x0, -w / 2 + ch)]

def shrink(pts, fwd=0.0, aft=0.0, side=0.0, dx=0.0):
    """The plan pulled in at the front, the back and the sides (and slid by dx): a top outline."""
    xa, xb = min(p[0] for p in pts), max(p[0] for p in pts); w = max(p[1] for p in pts)
    return [(xa + aft + (p[0] - xa) * (xb - xa - fwd - aft) / (xb - xa) + dx, p[1] * (w - side) / w) for p in pts]

def prism(k, bot, top, z0, z1, c, cap=None):
    """Faceted block between a bottom and a top plan. Returns its side faces
    (b0, b1, t1, t0, outward normal) for windows, antenna arrays and bands."""
    rb = [(x, y, z0) for x, y in bot]; rt = [(x, y, z1) for x, y in top]
    k.loft([rb, rt], c, smooth=False, cap_c=cap)
    n = len(rb); ctr = tuple(sum(p[i] for p in rb + rt) / (2 * n) for i in range(3)); faces = []
    for i in range(n):
        b0, b1, t0, t1 = rb[i], rb[(i + 1) % n], rt[i], rt[(i + 1) % n]
        nrm = _unit(_cross(_sub(b1, b0), _sub(_mix(t0, t1, .5), _mix(b0, b1, .5))))
        if _dot(nrm, _sub(_mix(b0, t1, .5), ctr)) < 0: nrm = tuple(-q for q in nrm)
        faces.append((b0, b1, t1, t0, nrm))
    return faces

def house(k, x0, x1, w, z0, z1, c, ch=0.0, fwd=0.0, aft=0.0, side=0.0, dx=0.0, cap=None):
    o = outline(x0, x1, w, ch)
    return prism(k, o, shrink(o, fwd, aft, side, dx), z0, z1, c, cap)

def on_face(F, u, v, eps=.1):
    b0, b1, t1, t0, n = F
    return _add(_mix(_mix(b0, b1, u), _mix(t0, t1, u), v), n, eps)

def face_rect(k, F, u0, u1, v0, v1, c, eps=.1):
    k.plate([on_face(F, u0, v0, eps), on_face(F, u1, v0, eps), on_face(F, u1, v1, eps), on_face(F, u0, v1, eps)], c)

def band(k, faces, v0, v1, c, u0=.04, u1=.96, eps=.1):
    """Strip round a block at height fractions v0..v1: bridge windows, a funnel band."""
    for F in faces: face_rect(k, F, u0, u1, v0, v1, c, eps)

def face_disc(k, F, u, v, r, c, n=8, eps=.12, a0=None):
    """Regular polygon on a face: a phased-array panel (octagon), a porthole."""
    b0, b1, t1, t0, nrm = F
    o = on_face(F, u, v, eps); e1 = _unit(_sub(b1, b0)); e2 = _unit(_cross(nrm, e1))
    a0 = pi / n if a0 is None else a0
    k.plate([_add(_add(o, e1, r * cos(a0 + i * 2 * pi / n)), e2, r * sin(a0 + i * 2 * pi / n)) for i in range(n)], c)

def funnel(k, x0, x1, w, z0, z1, c, rake=1.5, team=True, pipes=2, ch=None):
    """Raked funnel casing with a black cap, exhaust uptakes and a band in the owner's colour."""
    o = outline(x0, x1, w, w * .22 if ch is None else ch)
    top = shrink(o, (x1 - x0) * .12, (x1 - x0) * .04, w * .2, -rake)
    F = prism(k, o, top, z0, z1, c)
    cap = shrink(top, .3, .3, .5)
    prism(k, cap, cap, z1, z1 + .7, BLACK)
    if team: band(k, F, .74, .87, TEAM, .0, 1.0)
    xa, xb = min(p[0] for p in cap), max(p[0] for p in cap)
    for i in range(pipes):
        x = xa + (xb - xa) * (i + .5) / pipes
        k.rev((x, 0, z1 + .5), (-.25, 0, 1), [(0, min(w, xb - xa) * .2), (1.8, min(w, xb - xa) * .17)], rgb(38, 38, 40, 'dark'), n=8)
    return F

# --- Weapons and sensors -----------------------------------------------------------------------
def gun_mount(k, x, y, z, c, yaw=0.0, L=4.6, W=3.2, Hh=2.6, barrel=7.0, r=.22, twin=0.0, top=None, elev=6.0):
    """Enclosed gun mount: ring, rounded gun house, barrel(s) with a thicker chase."""
    r = max(r, .6 * _px(k))
    with k.at(x, y, z, yaw=yaw):
        k.cyl(0, 0, 0, W * .6, .35, tint(c, .75), n=14)
        k.stack(.2, 0, [(.3, L, W, W * .3), (Hh * .55, L * .96, W * .94, W * .32), (Hh, L * .62, W * .7, W * .3, -L * .12)], c, k=3)
        for s in ((-1, 1) if twin else (0,)):
            with k.at(L * .36, s * twin / 2, Hh * .5, pitch=elev):
                k.rev((0, 0, 0), 'x', [(0, r * 2.2), (barrel * .14, r * 1.9), (barrel * .16, r * 1.2), (barrel - .5, r), (barrel - .5, r * 1.35), (barrel, r * 1.35)], GUN, n=8)
        if top: k.cbox(-L * .12, 0, Hh + .03, L * .3, W * .42, .05, top)

def ciws(k, x, y, z, yaw=0.0, c=NAVY_GREY):
    """Phalanx close-in weapon system: white radome drum over a 20 mm rotary cannon."""
    with k.at(x, y, z, yaw=yaw):
        k.cbox(0, 0, 0, 2.4, 2.2, 1.3, c)
        k.cyl(0, 0, 1.3, .85, 2.1, WHITE, n=12); k.dome(0, 0, 3.4, .85, WHITE, n=12, k=4)
        k.tube((.5, 0, 1.7), (2.4, 0, 1.85), max(.16, .5 * _px(k)), GUN, n=6)

def ak630(k, x, y, z, yaw=0.0, c=SOV_GREY):
    """AK-630 30 mm rotary cannon: a small domed turret."""
    with k.at(x, y, z, yaw=yaw):
        k.cyl(0, 0, 0, 1.0, .5, tint(c, .8), n=10)
        k.ball(0, 0, .5, 1.0, c, rz=1.3, n=10, k=5, lo=0.0)
        k.tube((.6, 0, 1.0), (2.6, 0, 1.25), max(.17, .55 * _px(k)), GUN, n=6)

def vls(k, x0, x1, w, z, nx, ny, c):
    """Vertical-launch missile cells: a coaming with a grid of armoured lids."""
    k.box(x0, -w / 2, z - .5, x1 - x0, w, .8, tint(c, .8))
    cx, cy = (x1 - x0 - .6) / nx, (w - .6) / ny
    with k.grp():
        for i in range(nx):
            for j in range(ny):
                xa, ya = x0 + .3 + i * cx + cx * .12, -w / 2 + .3 + j * cy + cy * .12
                k.poly([(xa, ya, z + .34), (xa + cx * .76, ya, z + .34), (xa + cx * .76, ya + cy * .76, z + .34), (xa, ya + cy * .76, z + .34)],
                       tint(c, 1.08 if (i * 5 + j * 3) % 7 else .9))

def director(k, x, y, z, r, c, yaw=0.0, pitch=18.0, h=1.6):
    """Fire-control illuminator: a dish on a pedestal."""
    k.cyl(x, y, z, r * .5, h, c, n=8)
    with k.at(x, y, z + h + r * .55, yaw=yaw, pitch=pitch):
        k.rev((-r * .3, 0, 0), 'x', [(0, r * .3), (r * .22, r * .8), (r * .5, r)], c, n=12)
        k.rev((r * .2, 0, 0), 'x', [(0, r * .92), (.04, r * .92)], tint(c, .7), n=12)
        k.tube((r * .2, 0, 0), (r * .75, 0, 0), max(.08, .35 * _px(k)), DARK, n=4)

def bar_radar(k, x, y, z, w, h, c, yaw=0.0, pitch=-12.0, t=.4, post=1.0):
    """Search-radar antenna: a slatted reflector on a turning post."""
    k.cyl(x, y, z, max(.3, .6 * _px(k)), post, DARK, n=6)
    with k.at(x, y, z + post, yaw=yaw, pitch=pitch):
        k.box(-t / 2, -w / 2, 0, t, w, h, c)
        for i in range(1, 4): k.box(-t / 2 - .08, -w / 2 - .05, h * i / 4 - .08, t + .16, w + .1, .16, tint(c, .6))
        k.tube((0, 0, h * .5), (w * .28, 0, h * .15), max(.1, .4 * _px(k)), DARK, n=4)

def flag(k, x, y, z, h, w, c=TEAM, yaw=180.0):
    """Staff with the ensign streaming from it in three folds (so no facing sees it edge-on)."""
    px = _px(k)
    k.tube((x, y, z), (x, y, z + h), max(.08, .45 * px), GALV, n=4)
    fh = w * .62; pts = [(0.0, 0.0)]
    for a in (30, -34, 28): pts.append((pts[-1][0] + w / 3 * cos(radians(a)), pts[-1][1] + w / 3 * sin(radians(a))))
    with k.at(x, y, z + h - fh, yaw=yaw):
        with k.grp():
            for a, b in zip(pts, pts[1:]):
                k.poly([(a[0], a[1], 0), (b[0], b[1], 0), (b[0], b[1], fh), (a[0], a[1], fh)], c)

def helipad(k, x, y, z, r, ring, lw, line=WHITE, half=None):
    """Landing circle, centre line and athwartships line painted on a flight deck."""
    n = 24; z += .07
    with k.grp():
        for i in range(n):
            a, b = i * 2 * pi / n, (i + 1) * 2 * pi / n
            k.poly([(x + r * cos(a), y + r * sin(a), z), (x + r * cos(b), y + r * sin(b), z),
                    (x + (r - lw) * cos(b), y + (r - lw) * sin(b), z), (x + (r - lw) * cos(a), y + (r - lw) * sin(a), z)], ring)
    half = half or r * 1.5
    k.rect(x - half, y - lw * .3, z, half * 2, lw * .6, line)
    k.rect(x - lw * .3, y - r * .8, z, lw * .6, r * 1.6, line)

def rafts(k, x, y, z, n, step, c=WHITE):
    """Life-raft canisters in a rack along the deck edge."""
    for i in range(n): k.rev((x + i * step - .7, y, z + .5), 'x', [(0, .5), (1.4, .5)], c, n=6)

def boat(k, x, y, z, L, c=rgb(54, 58, 62, 'rubber'), davits=GALV, s=1):
    """Rigid inflatable on its cradle under a davit."""
    k.ball(x, y, z + L * .13, L / 2, c, ry=L * .19, rz=L * .15, n=10, k=5, lo=-.85, hi=.35)
    k.cbox(x - L * .1, y, z + L * .17, L * .3, L * .16, L * .1, ORANGE)
    px = _px(k)
    for dx in (-L * .3, L * .3):
        k.strut((x + dx, y - s * L * .25, z), (x + dx, y - s * L * .2, z + L * .45), .8 * px, davits)
        k.strut((x + dx, y - s * L * .2, z + L * .45), (x + dx, y + s * L * .05, z + L * .5), .8 * px, davits)

def canister(k, x, y, z, yaw, L, r, c, n=2, elev=30.0, cap=None):
    """A rack of missile canisters (Harpoon) on a canted frame."""
    with k.at(x, y, z, yaw=yaw, pitch=elev):
        for i in range(n):
            for j in range(2):
                k.rev((-L / 2, (i - (n - 1) / 2) * r * 2.15, r + j * r * 2.1), 'x', [(0, r), (L, r)], c, n=8)
                k.rev((L / 2, (i - (n - 1) / 2) * r * 2.15, r + j * r * 2.1), 'x', [(0, r * .9), (.08, r * .9)], cap or tint(c, .6), n=8)
    k.cbox(x, y, z, r * 3, r * 3, L * .22, tint(c, .7))

def pole_mast(k, x, z, h, c, rake=5.0, spread=3.4, yards=((.48, 7.0), (.72, 4.5))):
    """Raked pole mast braced by two forward legs (Arleigh Burke), yardarms and aerials."""
    px = _px(k); r = max(.45, .85 * px)
    top = (x - rake, 0, z + h)
    k.tube((x, 0, z), top, r, c, r2=r * .6, n=6)
    j = _mix((x, 0, z), top, .62)
    for s in (-1, 1): k.tube((x + 5.5, s * spread, z), j, r * .7, c, n=4)
    for f, w in yards:
        p = _mix((x, 0, z), top, f)
        k.strut((p[0], -w, p[2]), (p[0], w, p[2]), 1.0 * px, c)
        for s in (-1, 1): k.strut((p[0], s * w * .7, p[2]), _mix((x, 0, z), top, f + .12), .7 * px, c)
    p = _mix((x, 0, z), top, .36)
    k.cbox(p[0] + 1.6, 0, p[2], 3.4, 3.0, .3, c)                       # radar platform
    bar_radar(k, p[0] + 2.0, 0, p[2] + .3, 4.2, 1.1, tint(c, 1.5), yaw=20, post=.7)
    k.cyl(top[0], 0, top[2], .75, 1.5, tint(c, 1.6), n=8)             # TACAN drum
    k.ball(top[0], 0, top[2] + 2.0, .25, RED_LAMP, n=6, k=3)
    return top

# --- Allied destroyer --------------------------------------------------------------------------
def destroyer(k):
    # Allied destroyer, after the Arleigh Burke class (Flight I): 154 x 20 m, drawn 114 m long.
    # Flared bow with a bulwark, 5 inch gun, forward and aft VLS, faceted deckhouse carrying the
    # four SPY-1D arrays, raked pole mast, two funnels, Phalanx fore and aft, flight deck.
    c = NAVY_GREY; dk = DECK_GREY; px = _px(k); rc = tint(c, 1.1)
    H = ship_hull(k, -56, 58, 20, 5.0, 9.6, c, dk, rake=7.5, knuckle=.62, flare=1.3)
    bulwark(k, H, 49, 1.3, c); anchors(k, H, 52.5)
    hull_panel(k, H, 43, 49, .52, .86, TEAM)
    rails(k, H, -37, 49, rc)
    # Forecastle: gun, missile cells.
    gun_mount(k, 42.5, 0, H.zd(42.5), c, top=TEAM)
    vls(k, 30.2, 36.8, 9.6, H.zd(36.8) + .2, 4, 8, c)
    # 01 level running most of the ship's length, the deckhouse and bridge on it.
    house(k, -30, 28.5, 16.6, 4.8, 8.7, c, ch=2.0, cap=dk)
    F = house(k, 8, 27, 16.6, 8.7, 15.6, c, ch=4.4, fwd=1.6, aft=.8, side=3.0, cap=dk)
    for i in (1, 3, 5, 7):
        face_disc(k, F[i], .5, .55, 2.15, tint(c, .8), eps=.1); face_disc(k, F[i], .5, .55, 1.8, rgb(150, 150, 142), eps=.16)
    B_ = house(k, 13, 22.8, 12.4, 15.6, 18.8, c, ch=2.6, side=1.0, fwd=.4, cap=dk)
    band(k, B_[1:5], .45, .8, GLASS_DARK)
    ciws(k, 24.6, 0, 15.6, c=c)
    director(k, 19.5, 0, 18.8, 1.3, c)
    for s in (-1, 1):
        k.cbox(17, s * 7.2, 15.6, 3.2, 2.0, 2.4, c)                     # electronic-warfare arrays
        rafts(k, 10, s * 8.0, 8.7, 4, 2.2)
        rail(k, [(9, s * 8.1, 8.7), (27, s * 8.1, 8.7)], rc, 3 * px, .75 * px)
    pole_mast(k, 13.5, 18.8, 20.0, MAST_GREY)
    # Funnels, with the Harpoon canisters and the boat between them.
    funnel(k, -3, 6.5, 9.6, 8.7, 19.0, c)
    funnel(k, -19.5, -10, 9.6, 8.7, 18.0, c)
    canister(k, -6.2, 2.6, 8.7, 72, 4.6, .42, c); canister(k, -6.2, -2.6, 8.7, -108, 4.6, .42, c)
    boat(k, -6.5, -6.6, 8.7, 6.5, s=-1); boat(k, 2.5, 7.0, 8.7, 5.0, s=1)
    # After deckhouse: Phalanx, two illuminators, the aft missile cells.
    house(k, -30, -19.5, 13.5, 8.7, 11.8, c, ch=1.5, side=.8, cap=dk)
    ciws(k, -21.8, 0, 11.8, yaw=180, c=c)
    director(k, -25.2, 0, 11.8, 1.3, c, yaw=180, h=3.2); director(k, -28.4, 0, 11.8, 1.3, c, yaw=180, h=1.2)
    house(k, -38.4, -30, 14.5, 4.8, 7.6, c, ch=1.2, cap=dk)
    vls(k, -37.6, -30.8, 12.6, 7.4, 4, 9, c)
    # Flight deck.
    k.rect(-55, -8.2, 5.07, 16, 16.4, tint(dk, .9))
    for y in (-8.0, 7.7): k.rect(-55, y, 5.1, 16, .3, WHITE)
    for x in (-55, -39.3): k.rect(x, -8.0, 5.1, .3, 16, WHITE)
    helipad(k, -47, 0, 5.05, 4.6, TEAM, 1.0, half=7.5)
    for s in (-1, 1):  # safety nets
        k.plate([(-55, s * H.hb(-55), 5.0), (-39, s * H.hb(-39), 5.0), (-39, s * (H.hb(-39) + 1.4), 4.7), (-55, s * (H.hb(-55) + 1.4), 4.7)], rgb(52, 54, 56, 'dark'))
    flag(k, -55.4, 0, 5.0, 5.0, 4.6)

# --- Allied air-defence cruiser ----------------------------------------------------------------
def aegis_cruiser(k):
    # Allied air-defence cruiser, after the Ticonderoga class: 173 x 16.8 m, drawn 104 m long.
    # High bulwarked bow, a 5 inch gun at each end, tall slab-sided superstructures fore and aft
    # carrying the four SPY-1A arrays, two lattice masts, two boxy funnels, four illuminators.
    c = NAVY_GREY; dk = DECK_GREY; px = _px(k); rc = tint(c, 1.1)
    H = ship_hull(k, -51, 53, 17, 5.4, 10.6, c, dk, rake=7.5, sheer=.3, entry=.4)
    bulwark(k, H, 43, 1.9, c); anchors(k, H, 47.5)
    hull_panel(k, H, 38, 44, .5, .86, TEAM)
    rails(k, H, -50.5, 43, rc)
    gun_mount(k, 40, 0, H.zd(40), c, top=TEAM)
    vls(k, 31.2, 36.4, 9.2, H.zd(36.4) + .2, 4, 7, c)
    house(k, -26.5, 30, 15.6, 5.2, 9.8, c, ch=1.6, cap=dk)
    # Forward superstructure: arrays face ahead and to starboard.
    F = house(k, 15, 29.4, 15.4, 9.8, 21.2, c, fwd=1.4, aft=.6, side=1.2, cap=dk)
    for Fc, u in ((F[1], .73), (F[2], .3)):
        face_disc(k, Fc, u, .66, 2.3, tint(c, .8), eps=.1); face_disc(k, Fc, u, .66, 1.95, rgb(150, 150, 142), eps=.16)
    B_ = house(k, 20.5, 28, 12.6, 21.2, 23.8, c, ch=1.6, fwd=.4, side=.8, cap=dk)
    band(k, B_[0:5], .4, .8, GLASS_DARK)
    director(k, 24.6, 0, 23.8, 1.25, c)
    k.mast(17.6, 0, 21.2, 14.5, 5.6, 1.9, MAST_GREY, bays=4, t=1.1 * px)
    k.cbox(17.6, 0, 35.7, 3.6, 3.6, .4, MAST_GREY); k.ball(17.6, 0, 37.4, 1.4, WHITE, n=10, k=5)
    k.strut((17.6, -5.5, 31.5), (17.6, 5.5, 31.5), 1.0 * px, MAST_GREY)
    bar_radar(k, 19.4, 0, 28.6, 3.6, .9, tint(MAST_GREY, 1.5), yaw=15, post=.6)
    # Funnels with the mainmast and its air-search radar between them.
    funnel(k, 3.5, 13, 10.4, 9.8, 21.5, c, rake=1.0); funnel(k, -15.5, -6, 10.4, 9.8, 20.5, c, rake=1.0)
    house(k, -5.5, 3, 9.0, 9.8, 13.2, c, ch=1.0, cap=dk)
    k.mast(-1.2, 0, 13.2, 20.5, 5.4, 2.0, MAST_GREY, bays=5, t=1.1 * px)
    k.cbox(-1.2, 0, 33.7, 3.4, 3.4, .4, MAST_GREY)
    bar_radar(k, -1.2, 0, 34.1, 7.4, 3.6, rgb(74, 76, 76, 'dark'), yaw=160, pitch=-14, post=.8)
    k.strut((-1.2, -5.0, 27.5), (-1.2, 5.0, 27.5), 1.0 * px, MAST_GREY)
    for s in (-1, 1):
        ciws(k, -1.0, s * 6.0, 9.8, yaw=s * 90, c=c)
        rafts(k, 5, s * 7.2, 9.8, 4, 2.2); rafts(k, -14, s * 7.2, 9.8, 4, 2.2)
        rail(k, [(-26, s * 7.6, 9.8), (15, s * 7.6, 9.8)], rc, 3 * px, .75 * px)
    boat(k, -4.2, 6.2, 13.2 - 3.4, 5.5, s=1)
    # After superstructure (hangar): arrays face astern and to port; three illuminators.
    A = house(k, -26.5, -15.5, 15.4, 9.8, 18.2, c, fwd=.5, aft=1.2, side=1.2, cap=dk)
    for Fc, u in ((A[3], .73), (A[0], .3)):
        face_disc(k, Fc, u, .62, 2.3, tint(c, .8), eps=.1); face_disc(k, Fc, u, .62, 1.95, rgb(150, 150, 142), eps=.16)
    director(k, -18, 0, 18.2, 1.25, c, yaw=0, h=1.4)
    director(k, -21.4, 0, 18.2, 1.25, c, yaw=180, h=3.4); director(k, -24.4, 0, 18.2, 1.25, c, yaw=180, h=1.2)
    # Helicopter deck, aft missile cells, after gun.
    k.rect(-36.2, -7.3, 5.47, 9.4, 14.6, tint(dk, .9))
    for y in (-7.2, 6.9): k.rect(-36.2, y, 5.5, 9.4, .3, WHITE)
    helipad(k, -31.6, 0, 5.45, 3.7, TEAM, .9, half=4.4)
    vls(k, -41.6, -37.2, 9.6, 5.65, 3, 7, c)
    gun_mount(k, -45.4, 0, 5.4, c, yaw=180, top=TEAM)
    flag(k, -50.4, 0, 5.4, 4.6, 4.2)

# --- Soviet fast attack craft ------------------------------------------------------------------
def sea_scorpion(k):
    # Soviet missile boat, after the Tarantul class (Project 1241): 56 x 10.5 m, drawn 48 m long.
    # 76 mm AK-176 forward, wheelhouse under the 'Band Stand' radome, lattice mast, four canted
    # missile tubes in pairs along the sides, two AK-630 rotary cannon aft.
    c = SOV_GREY; dk = SOV_DECK; px = _px(k); rc = tint(c, 1.15)
    H = ship_hull(k, -23, 25, 9.8, 2.9, 5.0, c, dk, rake=4.5, sheer=.3, entry=.46, stern=.9, zr=.3, zb=.7, flare=1.5)
    bulwark(k, H, 19, .6, c)
    hull_panel(k, H, 15.5, 19.5, .42, .86, TEAM)
    rails(k, H, -22.6, 19, rc, inset=.25, step=3.0)
    for s in (-1, 1):
        p = H.side(20.5, .7, s, .1); k.cbox(p[0], p[1], p[2] - .6, .8, .3, 1.1, DARK)
    k.cyl(17.5, 0, H.zd(17.5), .45, .5, DARK, n=8)
    gun_mount(k, 13.2, 0, H.zd(13.2), c, L=3.6, W=3.2, Hh=2.3, barrel=4.4, r=.11, elev=8)
    # Deckhouse, wheelhouse and the big fire-control radome on its roof.
    house(k, -8.5, 8.6, 5.2, 2.8, 5.6, c, ch=.8, cap=dk)
    Wh = house(k, 1.6, 7.6, 5.0, 5.6, 7.9, c, ch=1.1, fwd=.7, side=.5, cap=tint(c, .9))
    band(k, Wh[0:5], .42, .8, GLASS_DARK)
    k.cbox(6.0, 0, 7.92, 1.5, 3.2, .05, TEAM)
    k.cyl(3.4, 0, 7.9, 1.35, .7, c, n=14); k.ball(3.4, 0, 8.6, 1.6, rgb(168, 172, 166), n=14, k=6, lo=-.2)
    k.mast(-1.6, 0, 5.6, 9.0, 3.0, 1.1, MAST_GREY, bays=4, t=1.0 * px)
    k.cbox(-1.6, 0, 14.6, 1.8, 1.8, .2, MAST_GREY)
    bar_radar(k, -1.6, 0, 14.8, 3.4, .7, tint(MAST_GREY, 1.4), yaw=25, post=.5)
    k.strut((-1.6, -2.6, 11.4), (-1.6, 2.6, 11.4), 1.0 * px, MAST_GREY)
    for s in (-1, 1): k.ball(-1.6, s * 2.6, 11.9, .55, rgb(168, 172, 166), n=8, k=4)
    k.cyl(-6.2, 0, 5.6, .4, 1.4, c, n=6); k.cyl(-6.2, 0, 7.0, .95, 1.1, rgb(150, 154, 150), n=10)  # gun director drum
    # Missile tubes: two a side, one above the other, canted up and toed out.
    for s in (-1, 1):
        with k.at(-.6, s * 3.85, 3.9, yaw=s * 2.5, pitch=9):
            for j in range(2):
                z = .95 + j * 2.0; y = -s * j * .25
                k.rev((-5.4, y, z), 'x', [(0, .8), (.25, .96), (10.2, .96), (10.5, .86)], tint(c, .96), n=12)
                k.rev((5.1, y, z), 'x', [(0, .86), (.35, .62), (.5, .05)], rgb(122, 64, 50), n=12)   # muzzle cover
                for xr in (-3.2, 0, 3.2): k.rev((xr, y, z), 'x', [(0, 1.02), (.3, 1.02)], tint(c, .8), n=12)
            for xr in (-3.6, 3.4): k.cbox(xr, -s * .12, -.9, .8, 1.5, 4.6, tint(c, .85))
    # After deckhouse with the two rotary cannon; ensign at the stern.
    house(k, -17.5, -8.5, 6.4, 2.8, 4.9, c, ch=.8, cap=dk)
    for s in (-1, 1):
        ak630(k, -12.2 - (s + 1) * 1.4, s * 1.5, 4.9, yaw=180 - s * 12, c=c)
        k.cbox(-16.4, s * 3.22, 3.4, 2.6, .1, 1.0, rgb(44, 46, 46, 'dark'))   # engine-room intake louvres
    k.cbox(-9.9, 0, 4.92, 1.8, 4.2, .05, TEAM)
    k.cyl(-20.2, 0, 2.9, .5, 1.2, c, n=6); k.cbox(-20.2, 0, 4.1, 1.2, 1.6, .8, tint(c, .9))  # SA-N-5 launcher
    flag(k, -22.5, 0, 2.9, 4.0, 3.8)

# --- Soviet missile battlecruiser --------------------------------------------------------------
def dreadnought(k):
    # Soviet missile battlecruiser, after the Kirov class (Project 1144): 252 x 28.5 m, drawn
    # 141 m long. Sheered forecastle with fields of flush missile hatches and two canted
    # cruise-missile tubes, tiered superstructure under the 'Top Pair' radar, mast-funnel,
    # twin 130 mm turret aft, helicopter deck on the quarterdeck.
    c = SOV_GREY; dk = SOV_DECK; px = _px(k); rc = tint(c, 1.15); mesh = rgb(72, 76, 74, 'dark')
    H = ship_hull(k, -70, 70, 28.5, 7.0, 14.2, c, dk, rake=11.5, sheer=.3, entry=.4, stern=.8, zr=.9, zb=2.0, flare=1.9)
    bulwark(k, H, 60, 1.4, c); anchors(k, H, 63.5)
    hull_panel(k, H, 51, 59, .5, .86, TEAM); hull_panel(k, H, -65, -58, .5, .86, TEAM)
    rails(k, H, -69.5, 60, rc, step=6.0)
    # Forecastle: surface-to-air hatches (round), anti-ship missile hatches (inclined, square).
    for x in (50.2, 54.0):
        for y in (-4.8, -1.6, 1.6, 4.8): k.cyl(x, y, H.zd(x) - .1, 1.25, .45, tint(c, .9), n=10, top=tint(c, 1.06))
    for i in range(4):
        x = 36.2 + i * 3.4
        for y in (-6.6, -2.2, 2.2, 6.6):
            k.cbox(x, y, H.zd(x - 1.3) - .2, 2.5, 3.3, .6, tint(c, .86), top=tint(c, 1.04 if (i + int(y)) % 3 else .92))
    # The two cruise-missile launch tubes.
    for s in (-1, 1):
        with k.at(20.5, s * 5.6, H.zd(20.5) + .7, yaw=-s * 3, pitch=15):
            k.rev((0, 0, 1.9), 'x', [(0, 1.5), (.4, 1.85), (13.2, 1.85), (13.6, 1.7)], tint(c, .97), n=14)
            k.rev((13.6, 0, 1.9), 'x', [(0, 1.7), (.6, 1.3), (.9, .1)], rgb(122, 64, 50), n=14)
            for xr in (2.2, 6.6, 11.0): k.rev((xr, 0, 1.9), 'x', [(0, 1.98), (.45, 1.98)], tint(c, .8), n=14)
            k.cbox(2.6, 0, -1.2, 1.4, 3.0, 1.6, tint(c, .85)); k.cbox(10.6, 0, -3.6, 1.4, 3.0, 4.0, tint(c, .85))
    # Tiered superstructure.
    house(k, -40, 19, 22.5, 6.8, 13.0, c, ch=3.0, cap=dk)
    house(k, -8, 16, 17.5, 13.0, 17.6, c, ch=2.6, fwd=1.0, side=1.0, cap=dk)
    Bz = house(k, 0, 13.6, 13.5, 17.6, 21.2, c, ch=2.6, fwd=.8, side=.8, cap=dk)
    band(k, Bz[0:5], .42, .8, GLASS_DARK)
    k.cbox(10.6, 0, 21.22, 2.6, 7.0, .06, TEAM)
    T = house(k, -2.5, 8, 9.5, 21.2, 37.0, c, ch=1.6, fwd=2.2, aft=2.4, side=4.2, cap=dk)
    k.cbox(2.7, 0, 29.5, 8.6, 8.6, .5, c); k.cbox(2.7, 0, 37.0, 5.6, 5.6, .5, c)
    with k.at(2.7, 0, 37.5):   # Top Pair: two big antennas back to back
        k.cyl(0, 0, 0, .8, 1.6, MAST_GREY, n=8)
        with k.at(.9, 0, 1.4, pitch=-14):
            k.box(-.3, -5.2, 0, .6, 10.4, 5.4, mesh)
            for i in range(1, 5): k.box(-.42, -5.3, i * 1.08 - .1, .84, 10.6, .2, tint(mesh, .6))
        with k.at(-.9, 0, 1.4, yaw=180, pitch=-14):
            k.box(-.3, -3.8, 0, .6, 7.6, 3.6, mesh)
            for i in range(1, 4): k.box(-.42, -3.9, i * .9 - .1, .84, 7.8, .2, tint(mesh, .6))
    k.ball(2.7, 0, 43.8, .3, RED_LAMP, n=6, k=3)
    k.strut((2.7, -6.5, 30.0), (2.7, 6.5, 30.0), 1.0 * px, MAST_GREY)
    for s in (-1, 1): bar_radar(k, 8.5, s * 4.6, 21.2, 3.2, .7, tint(MAST_GREY, 1.4), yaw=s * 30, post=1.0)
    for x, yw in ((17.6, 0), (-35.4, 180)):   # 'Top Dome' missile directors
        zz = 13.0 if x > 0 else 17.6
        k.cyl(x, 0, zz, 1.5, 1.6, c, n=10)
        with k.at(x, 0, zz + 2.6, yaw=yw, pitch=30): k.ball(0, 0, 0, 2.2, rgb(170, 174, 168), n=14, k=6, lo=-.25)
    # Mast-funnel with the after air-search radar.
    funnel(k, -24.5, -11, 13.5, 13.0, 30.0, c, rake=2.2, pipes=2, ch=2.2)
    k.mast(-11.5, 0, 17.6, 14.0, 4.2, 1.8, MAST_GREY, bays=4, t=1.1 * px)
    bar_radar(k, -11.5, 0, 31.6, 7.0, 2.8, mesh, yaw=200, pitch=-14, post=.8)
    # After superstructure, gun director, rotary cannon, boats and rafts.
    house(k, -38.5, -26, 14.5, 13.0, 17.6, c, ch=2.0, side=1.0, cap=dk)
    director(k, -29.5, 0, 17.6, 1.7, c, yaw=180, h=2.6)
    for s in (-1, 1):
        ak630(k, 10.5, s * 9.6, 13.0, yaw=s * 80, c=c); ak630(k, -22, s * 9.6, 13.0, yaw=s * 100, c=c)
        boat(k, -3.5, s * 9.4, 13.0, 8.0, c=rgb(150, 154, 150), s=s)
        rafts(k, -18, s * 10.4, 13.0, 4, 2.4); rafts(k, 2.5, s * 10.4, 13.0, 3, 2.4)
        rail(k, [(-39, s * 11.0, 13.0), (17.5, s * 11.0, 13.0)], rc, 3 * px, .75 * px)
    gun_mount(k, -46.5, 0, 7.0, c, yaw=180, L=7.6, W=5.8, Hh=3.5, barrel=9.0, r=.3, twin=1.7, elev=10)
    # Helicopter deck with the hangar lift.
    k.rect(-68.5, -10.5, 7.07, 15.5, 21, tint(dk, .88))
    for y in (-10.4, 10.0): k.rect(-68.5, y, 7.1, 15.5, .4, WHITE)
    k.rect(-53.4, -10.4, 7.1, .4, 20.8, WHITE)
    helipad(k, -61, 0, 7.05, 5.6, TEAM, 1.2, half=7.0)
    k.rect(-56.2, -3.4, 7.09, 2.4, 6.8, tint(dk, .7))
    flag(k, -66.0, 0, 7.0, 6.4, 5.4)

# --- Allied aircraft carrier -------------------------------------------------------------------
def shell(k, sections, col, n=16, p=2.5, smooth=True):
    """Body along x through superelliptic sections [(x, yc, zc, ry, rz)]. `col` is a colour or a
    function (section index, angle of the panel in degrees: 0 = +y side, 90 = top)."""
    rings = [[(x, yc + a, zc + b) for a, b in superellipse(max(ry, 1e-3), max(rz, 1e-3), p, n)]
             for x, yc, zc, ry, rz in sections]
    pick = col if callable(col) else (lambda i, a: col)
    with k.grp(smooth):
        for i, (r0, r1) in enumerate(zip(rings, rings[1:])):
            for j in range(n):
                j2 = (j + 1) % n
                k.poly([r0[j], r0[j2], r1[j2], r1[j]], pick(i, (j + .5) * 360 / n))
        k.poly(rings[0][::-1], pick(0, 90)); k.poly(rings[-1], pick(len(rings) - 2, 90))

def _edge_y(line, x):
    """y of a deck-edge polyline (points sorted by x) at station x."""
    for (xa, ya), (xb, yb) in zip(line, line[1:]):
        if xa <= x <= xb: return ya + (yb - ya) * (x - xa) / (xb - xa) if xb > xa else ya
    return line[0][1] if x < line[0][0] else line[-1][1]

def deck_slab(k, port, stbd, z0, z1, top, side, grow=0.0):
    """Flight-deck slab between two edge polylines (each sorted by x), cut into convex strips."""
    xs = sorted(set(p[0] for p in port) | set(p[0] for p in stbd))
    P = lambda x: _edge_y(port, x) - grow; S = lambda x: _edge_y(stbd, x) + grow
    with k.grp(False):
        for xa, xb in zip(xs, xs[1:]):
            k.poly([(xa, P(xa), z1), (xa, S(xa), z1), (xb, S(xb), z1), (xb, P(xb), z1)], top)
            k.poly([(xa, P(xa), z0), (xa, S(xa), z0), (xb, S(xb), z0), (xb, P(xb), z0)], side)
            k.poly([(xa, P(xa), z0), (xb, P(xb), z0), (xb, P(xb), z1), (xa, P(xa), z1)], side)
            k.poly([(xa, S(xa), z0), (xb, S(xb), z0), (xb, S(xb), z1), (xa, S(xa), z1)], side)
        for x in (xs[0], xs[-1]): k.poly([(x, P(x), z0), (x, S(x), z0), (x, S(x), z1), (x, P(x), z1)], side)

def deck_jet(k, x, y, z, yaw, c=AIR_GREY):
    """A parked strike fighter, much simplified: 17 m long, twin canted fins, wings folded."""
    with k.at(x, y, z + 1.5, yaw=yaw):
        shell(k, [(8.5, 0, .3, .06, .06), (5.6, 0, .45, .6, .6), (2.0, 0, .55, 1.05, .8), (-5.0, 0, .55, 1.25, .72), (-8.0, 0, .55, .95, .5)], c, n=6)
        k.ball(4.5, 0, 1.0, 1.7, GLASS, ry=.5, rz=.55, n=6, k=3, lo=0.0)
        k.rev((-8.0, 0, .55), (-1, 0, 0), [(0, .8), (.5, .7)], DARK, n=6)
        for s in (-1, 1):
            k.extrude([(3.0, s * 1.0), (-1.0, s * 4.3), (-3.2, s * 4.3), (-3.4, s * 1.0)], 'xy', .75, 1.0, c)
            k.extrude([(-1.0, 1.0), (-3.2, 1.0), (-3.2, 3.5), (-2.2, 3.5)], 'xz', s * 4.2 - .12, s * 4.2 + .12, c)   # folded outer panel
            k.extrude([(-5.4, s * 1.0), (-7.6, s * 3.3), (-8.6, s * 3.3), (-8.2, s * 1.0)], 'xy', .55, .75, c)
            k.loft([[(-4.0, s * .95, 1.1), (-7.2, s * .95, 1.1), (-7.2, s * 1.15, 1.1), (-4.0, s * 1.15, 1.1)],
                    [(-5.9, s * 1.5, 2.9), (-7.5, s * 1.5, 2.9), (-7.5, s * 1.7, 2.9), (-5.9, s * 1.7, 2.9)]], c, smooth=False)
            k.loft([[(-5.9, s * 1.5, 2.9), (-7.5, s * 1.5, 2.9), (-7.5, s * 1.7, 2.9), (-5.9, s * 1.7, 2.9)],
                    [(-6.6, s * 1.75, 3.9), (-7.7, s * 1.75, 3.9), (-7.7, s * 1.95, 3.9), (-6.6, s * 1.95, 3.9)]], TEAM, smooth=False)
            k.cyl(-1.5, s * 1.5, -1.5, .45, .9, TYRE, n=6)
        k.cyl(5.5, 0, -1.5, .4, .9, TYRE, n=6)

def aircraft_carrier(k):
    # Allied aircraft carrier, after the Nimitz class: 333 m long, 77 m across the flight deck,
    # drawn 210 m long. Angled landing deck to port, island to starboard, four catapults, four
    # deck-edge lifts, parked aircraft, galleries and sponsons under the deck edge.
    c = NAVY_GREY; dk = rgb(74, 78, 82); px = _px(k)
    H = ship_hull(k, -101, 102, 39, 17.4, 17.4, c, dk, entry=.36, run=.24, stern=.74, wl=.86, fine=.2, rake=15,
                  flare=2.6, bluff=1.6, zr=1.2, zb=2.6)
    stbd = [(-104, 17), (-98, 21), (-66, 27), (-52, 34), (58, 34), (66, 30), (84, 16), (106, 12)]
    port = [(-104, -17), (-99, -22), (-64, -31), (-42, -38), (28, -38), (46, -24), (74, -18), (106, -12)]
    zf = 19.6
    # Sponsons carrying the deck overhangs.
    for s, xa, xb, yo in ((1, -62, 68, 31), (-1, -70, 40, 35), (1, -100, -66, 19), (-1, -100, -74, 20)):
        k.loft([[(xa + 9, s * 17, 8.5), (xb - 9, s * 17, 8.5), (xb - 9, s * 20, 8.5), (xa + 9, s * 20, 8.5)],
                [(xa, s * 17, 17.4), (xb, s * 17, 17.4), (xb, s * yo, 17.4), (xa, s * yo, 17.4)]], c, smooth=False)
    deck_slab(k, port, stbd, 16.4, 18.4, rgb(50, 52, 54, 'dark'), rgb(58, 60, 62, 'dark'), grow=1.7)   # galleries and nets
    deck_slab(k, port, stbd, 17.2, zf, dk, c)
    z = zf + .09
    # Hangar openings behind the lifts, boat booms.
    for s, xa, xb in ((1, 36, 54), (1, 8, 26), (1, -52, -34), (-1, -66, -48)):
        k.plate([(xa, s * 19.9, 9.5), (xb, s * 19.9, 9.5), (xb, s * 19.9, 16.3), (xa, s * 19.9, 16.3)], rgb(30, 32, 34, 'dark'))
        y0 = (_edge_y(stbd, (xa + xb) / 2) - 13.5) if s > 0 else (_edge_y(port, (xa + xb) / 2) + 1.5)
        for a, b, w, d in ((xa, y0, xb - xa, .5), (xa, y0 + 11.5, xb - xa, .5), (xa, y0, .5, 12), (xb - .5, y0, .5, 12)):
            k.rect(a, b, z, w, d, rgb(196, 176, 70))
    # Bow catapults and the waist catapult; jet-blast deflectors behind them.
    track = rgb(120, 124, 126, 'steel')
    for (xa, ya), (xb, yb) in (((22, 6.5), (101, 7.0)), ((14, -6.5), (101, -6.0))):
        k.plate([(xa, ya - .7, z), (xb, yb - .7, z), (xb, yb + .7, z), (xa, ya + .7, z)], track)
        k.box(xa - 7, ya - 3.4, zf, .5, 6.8, .25, tint(dk, 1.25))
    # Angled deck: centre line, edge lines, the waist catapult along it.
    with k.at(-101, -3, 0, yaw=-9.7):
        for i in range(13): k.rect(3 + i * 10, -.7, z, 6.4, 1.4, rgb(214, 196, 70) if i % 2 else WHITE)
        k.rect(4, -12.2, z, 124, .9, WHITE); k.rect(4, 11.4, z, 150, .9, WHITE)
        k.rect(60, -7.4, z, 66, 1.3, track)
        k.rect(4, -12.2, z, .9, 24.4, WHITE)
        for i in range(4): k.rect(22 + i * 9, -8, z + .02, .7, 16, rgb(38, 38, 40))   # arrester wires
    # Deck number forward and abaft the island, in the owner's colour.
    for xa in (81.5, 89.0): k.rect(xa, -3.6, z, 5.6, 7.2, TEAM)
    for xa in (-93.0, -86.2): k.rect(xa, 13.4, z, 5.2, 7.2, TEAM)
    # Island: three bridge levels, funnel, mast with its radars, domes.
    with k.at(-15, 27.8, 0):
        I1 = house(k, -12, 11, 9.4, zf, 27.4, c, ch=1.2, cap=dk)
        for F, u0, u1 in ((I1[0], .3, .72), (I1[4], .28, .7)): face_rect(k, F, u0, u1, .28, .82, TEAM, .15)
        I2 = house(k, -10, 11.5, 10.6, 27.4, 30.8, c, ch=1.4, cap=dk); band(k, I2, .4, .8, GLASS_DARK, eps=.15)
        I3 = house(k, -7.5, 9.5, 9.8, 30.8, 34.2, c, ch=1.4, cap=dk); band(k, I3, .4, .8, GLASS_DARK, eps=.15)
        house(k, -4, 6, 7.0, 34.2, 37.6, c, ch=1.0, cap=dk)
        F4 = house(k, -12.5, -6.5, 6.4, 27.4, 38.6, c, ch=1.0, aft=1.0, side=.8, dx=-.8)   # funnel
        band(k, F4, .76, .9, TEAM, .0, 1.0, eps=.15)
        prism(k, shrink(outline(-12.5, -6.5, 6.4, 1.0), .5, 1.4, 1.4, -.8), shrink(outline(-12.5, -6.5, 6.4, 1.0), .5, 1.4, 1.4, -.8), 38.6, 39.5, BLACK)
        k.tube((1, 0, 37.6), (-.5, 0, 57.0), max(.7, 1.0 * px), MAST_GREY, r2=.45, n=6)
        for zz, w in ((45.5, 8.0), (50.5, 5.5)):
            k.strut((.1, -w, zz), (.1, w, zz), 1.0 * px, MAST_GREY); k.cbox(.6, 0, zz - .6, 4.0, 3.4, .5, MAST_GREY)
        bar_radar(k, 1.2, 0, 46.0, 7.5, 2.6, rgb(74, 76, 76, 'dark'), yaw=30, post=.6)
        k.cyl(-.5, 0, 57.0, 1.0, 1.8, tint(MAST_GREY, 1.5), n=8); k.ball(-.5, 0, 59.4, .4, RED_LAMP, n=6, k=3)
        with k.at(-3.5, 0, 37.6, yaw=180, pitch=-24): k.box(-.4, -3.4, .8, .8, 6.8, 6.4, rgb(74, 76, 76, 'dark'))   # 3-D air-search array
        k.cyl(-3.5, 0, 37.6, .7, 1.6, MAST_GREY, n=6)
        for xx, yy in ((7.6, 2.6), (7.6, -2.6)): k.cyl(xx, yy, 34.2, .7, 1.0, c, n=6); k.ball(xx, yy, 36.4, 1.6, WHITE, n=10, k=5)
        ciws(k, 12.8, -2.0, zf, c=c)
    flag(k, -15.5, 27.8, 57.0 - 9.0, 9.0, 8.0)
    # Aircraft: a row ahead of the island, two abaft it, one on the starboard bow catapult.
    for x in (52, 40.5, 29, 17.5): deck_jet(k, x, 26.5, zf, -122)
    for x in (-42, -54): deck_jet(k, x, 24.5 - (x + 42) * -.05, zf, -58)
    deck_jet(k, 31, 6.6, zf, 0)
    deck_jet(k, 60, -13.5, zf, 148); deck_jet(k, 49, -17.5, zf, 148)
    # Whip aerials and a weapon sponson at each quarter.
    for x, s in ((70, 1), (-80, 1), (60, -1), (-20, -1), (10, -1)):
        yy = (_edge_y(stbd, x) if s > 0 else _edge_y(port, x)) + s * 1.6
        k.strut((x, yy, 18.4), (x, yy + s * 5.5, 22.5), .8 * px, MAST_GREY)
    ciws(k, -97, 20.6, 17.4, yaw=150, c=c); ciws(k, -97, -21.5, 17.4, yaw=210, c=c); ciws(k, 80, -19.5, 17.4, yaw=-30, c=c)

# --- Allied hovercraft -------------------------------------------------------------------------
def amphibious_transport(k):
    # Allied amphibious transport, after the LCAC landing craft: 26.4 x 14.3 m on cushion.
    # Inflated bag skirt, open cargo deck with bow and stern ramps, engine and cabin modules along
    # both sides, two shrouded propellers with twin rudders, swivelling bow thrusters.
    c = NAVY_GREY; dk = DECK_GREY; px = _px(k)
    k.stack(0, 0, [(-.3, 25.4, 13.3, 3.4), (.3, 26.3, 14.2, 3.9), (.95, 26.4, 14.3, 3.9), (1.5, 25.3, 13.2, 3.3)], WET_BLACK, k=5)
    for i, (x, y) in enumerate(rrect(26.5, 14.4, 3.95, 12)):   # finger seams
        if i % 2 == 0 or abs(y) > 7.1 or abs(x) > 13.1: k.strut((x * .985, y * .985, -.2), (x, y, .75), .06, rgb(52, 53, 55, 'skin'))
    k.stack(0, 0, [(1.4, 25.0, 13.0, 3.0), (2.15, 24.8, 12.8, 3.0)], c, k=4, cap_c=dk)
    # Cargo deck: non-skid lane, edge lines, tie-down pads.
    z = 2.19
    k.rect(-12.2, -4.1, z, 24.3, 8.2, tint(dk, .8))
    for y in (-4.05, 3.9): k.rect(-12.2, y, z + .01, 24.3, .15, YELLOW)
    with k.grp():
        for i in range(9):
            for y in (-2.6, 0, 2.6): k.poly([(-10.6 + i * 2.6, y - .12, z + .01), (-10.3 + i * 2.6, y - .12, z + .01), (-10.3 + i * 2.6, y + .12, z + .01), (-10.6 + i * 2.6, y + .12, z + .01)], tint(dk, 1.3))
    for s in (-1, 1):
        y = s * 5.35
        with k.at(0, y, 0):
            house(k, -8.6, 9.8, 2.5, 2.15, 4.4, c, fwd=1.3, aft=.3, side=.35, cap=tint(c, .92))
            k.cbox(-4.0, 0, 4.4, 3.4, 1.8, .75, tint(c, .86), top=rgb(40, 42, 44, 'dark'))      # engine intake plenum
            k.rev((-6.9, 0, 4.4), (-.35, 0, 1), [(0, .5), (1.4, .44)], rgb(48, 46, 44, 'dark'), n=8)  # exhaust
            k.cyl(1.2, 0, 4.4, 1.0, .3, tint(c, .85), n=14, top=rgb(34, 36, 38, 'dark'))         # lift-fan intake
            # Bow thruster: a swivelling elbow duct.
            k.cyl(7.3, 0, 4.2, .68, 1.3, c, n=10); k.ball(7.3, 0, 5.6, .74, c, n=10, k=5)
            k.rev((7.3, 0, 5.6), (.25, s * 1, -.12), [(0, .7), (1.5, .62)], c, n=10)
            k.rev((7.3 + .25 * 1.46, s * 1.46, 5.6 - .12 * 1.46), (.25, s * 1, -.12), [(0, .5), (.06, .5)], BLACK, n=8)
        # Propulsor: pylon, shroud with the owner's band on its lip, four-blade propeller, rudders.
        py = s * 4.9
        k.cbox(-10.5, py, 2.15, 2.0, 1.1, 1.4, c)
        with k.at(-10.75, py, 5.4, sx=3.0): k.torus(0, 0, 0, 1.98, .2, c, n=20, k=6, axis='x')
        with k.at(-10.05, py, 5.4, sx=1.0): k.torus(0, 0, 0, 1.99, .22, TEAM, n=20, k=6, axis='x')
        k.rev((-10.2, py, 5.4), (-1, 0, 0), [(0, .2), (.3, .45), (1.3, .4), (1.7, .1)], DARK, n=8)
        for i in range(4):
            with k.at(-10.7, py, 5.4, roll=25 + i * 90 + s * 15):
                k.loft([[(-.14, -.34, .3), (.14, .3, .3), (.2, .3, .3), (-.08, -.34, .3)], [(-.08, -.22, 1.85), (.08, .2, 1.85), (.14, .2, 1.85), (-.02, -.22, 1.85)]], rgb(40, 40, 42), smooth=False)
        for a in (30, 150, 270):
            k.strut((-11.1, py, 5.4), (-11.1, py + 1.95 * cos(radians(a)), 5.4 + 1.95 * sin(radians(a))), .1, c)
        for dy in (-.75, .75): k.box(-12.75, py + dy - .07, 3.45, 1.15, .14, 3.9, c)
    # Control cabin to starboard, loadmaster's cabin to port.
    with k.at(6.4, 5.35, 0):
        Cb = house(k, -2.5, 2.5, 2.7, 4.4, 6.2, c, ch=.5, fwd=.6, side=.35, cap=tint(c, .9))
        band(k, Cb, .42, .82, GLASS_DARK, eps=.04)
        k.cbox(-.3, 0, 6.22, 2.2, 1.6, .04, TEAM)
        bar_radar(k, .9, 0, 6.2, 1.9, .28, tint(c, 1.1), yaw=20, t=.14, post=.8)
        k.tube((-2.0, .8, 6.2), (-2.6, .9, 9.6), .035, DARK, n=4); k.tube((-2.0, -.8, 6.2), (-2.5, -.9, 8.8), .035, DARK, n=4)
    with k.at(6.9, -5.35, 0):
        Cp = house(k, -1.7, 1.7, 2.5, 4.4, 5.7, c, ch=.4, fwd=.4, side=.3, cap=tint(c, .9))
        band(k, Cp, .4, .8, GLASS_DARK, eps=.04)
    # Bow ramp raised for the run in, stern ramp, mooring bitts.
    with k.at(12.25, 0, 2.15, pitch=48):
        k.box(0, -4.1, -.12, 3.5, 8.2, .24, c, top=tint(dk, .85))
        for y in (-4.1, 3.9): k.box(0, y, .1, 3.5, .2, .35, c)
        k.box(3.2, -4.1, .12, .3, 8.2, .06, TEAM)
    k.box(-12.5, -2.6, 2.15, .25, 5.2, 1.7, c)
    for s in (-1, 1):
        for x in (-11.8, 10.9): k.cyl(x, s * 3.6, 2.15, .16, .4, DARK, n=6)
    flag(k, 4.6, 5.35, 6.2, 2.4, 1.8)

# --- Soviet attack submarine -------------------------------------------------------------------
def typhoon(k):
    # Soviet attack submarine with the tall rudder and towed-array pod of the Akula class
    # (110 x 13.6 m) and the missile deck of the Typhoon, drawn 82 m long and running awash:
    # anechoic-tiled hull, streamlined sail with raised masts, bow planes.
    hc = rgb(38, 40, 43, 'skin'); seam = rgb(64, 66, 70, 'skin'); px = _px(k); R, zc = 6.0, -2.7
    secs = [(41.2, -3.5, .4), (40.3, -3.2, 2.3), (38.4, -3.0, 4.0), (35, -2.8, 5.3), (30, zc, R), (-10, zc, R),
            (-20, -2.85, 5.3), (-27, -2.95, 4.2), (-33, -2.9, 3.3), (-37.5, -2.7, 2.6), (-40.5, -2.6, 1.6)]
    shell(k, [(x, 0, z, r, r) for x, z, r in secs], hc, n=24, p=2.0)
    def rad(x):
        for (xa, za, ra), (xb, zb, rb) in zip(secs, secs[1:]):
            if xb <= x <= xa:
                f = (x - xa) / (xb - xa); return za + (zb - za) * f, ra + (rb - ra) * f
        return zc, R
    # Tile seams: hoops and stringers a shade lighter than the rubber.
    for i in range(13):
        x = -25 + i * 5; z0, r = rad(x)
        pts = [(x, (r + .06) * sin(radians(a)), z0 + (r + .06) * cos(radians(a))) for a in range(-70, 71, 14)]
        for a, b in zip(pts, pts[1:]):
            if min(a[2], b[2]) > -.3: k.strut(a, b, .55 * px, seam)
    for a in (-52, -27, 27, 52):
        k.strut((-10, (R + .06) * sin(radians(a)), zc + (R + .06) * cos(radians(a))), (30, (R + .06) * sin(radians(a)), zc + (R + .06) * cos(radians(a))), .55 * px, seam)
    # Deck casing with the missile hatches forward of the sail.
    shell(k, [(34, 0, 2.3, .8, .4), (29, 0, 2.95, 2.5, .6), (-13, 0, 2.95, 2.5, .6), (-23, 0, 2.3, 1.4, .5)], rgb(46, 48, 51, 'skin'), n=12, p=4.0)
    for i in range(6):
        for s in (-1, 1): k.cyl(13.5 + i * 2.7, s * 1.2, 3.5, 1.02, .08, seam, n=10, top=rgb(52, 54, 57, 'skin'))
    helipad(k, -16.5, 0, 3.5, 1.4, WHITE, .35, line=SIGNAL_RED, half=.9)   # rescue hatch
    # Sail: faired base, hull-number band in the owner's colour, bridge windows, masts.
    k.stack(2.5, 0, [(3.3, 18.5, 5.6, 2.6), (5.4, 15.5, 4.4, 2.1)], hc, k=4)
    k.stack(2.5, 0, [(5.4, 14.2, 3.9, 1.85), (8.6, 13.8, 3.7, 1.75)], hc, k=4)
    k.stack(2.5, 0, [(8.6, 13.85, 3.75, 1.76), (10.2, 13.6, 3.6, 1.7)], TEAM, k=4)
    k.stack(2.5, 0, [(10.2, 13.5, 3.55, 1.68), (12.6, 12.4, 3.1, 1.5), (13.1, 11.2, 2.5, 1.2)], hc, k=4)
    k.plate([(9.18, -1.0, 11.2), (9.18, 1.0, 11.2), (8.98, .95, 12.1), (8.98, -.95, 12.1)], GLASS_DARK)
    zt = 13.0
    k.tube((-.4, .45, zt), (-.4, .45, zt + 3.6), max(.14, .5 * px), IRON, n=6)
    k.tube((1.8, -.4, zt), (1.8, -.4, zt + 2.6), max(.2, .6 * px), IRON, n=6); k.cbox(1.8, -.4, zt + 2.6, .5, 2.4, .5, IRON)
    k.tube((-3.0, 0, zt), (-3.0, 0, zt + 3.0), max(.34, .8 * px), rgb(52, 54, 56, 'dark'), n=8)
    k.tube((5.0, 0, zt), (4.8, 0, zt + 4.8), max(.07, .4 * px), IRON, n=4)
    k.ball(7.2, 0, zt + .15, .22, RED_LAMP, n=6, k=3)
    flag(k, -4.6, 0, zt, 3.6, 3.2)
    # Bow planes, the tall rudder with its towed-array pod.
    for s in (-1, 1):
        k.extrude([(34.6, s * 4.4), (31.6, s * 4.4), (31.4, s * 9.2), (33.2, s * 9.2)], 'xy', .25, .6, hc)
        k.plate([(-35.6, s * .32, 5.4), (-40.7, s * .32, 5.4), (-40.5, s * .32, 6.9), (-36.6, s * .32, 6.9)], TEAM)
    k.extrude([(-33.2, -1.5), (-41.3, -1.5), (-40.7, 8.6), (-37.4, 8.6)], 'xz', -.28, .28, hc)
    k.rev((-34.6, 0, 9.0), (-1, 0, 0), [(0, .08), (1.4, .8), (3.6, .85), (5.6, .5), (6.8, .1)], hc, n=10)
    for x in (37.5, 36.6, 35.7): k.plate([(x, -.5, rad(x)[0] + rad(x)[1] + .05), (x, .5, rad(x)[0] + rad(x)[1] + .05), (x - .3, .5, rad(x - .3)[0] + rad(x - .3)[1] + .05), (x - .3, -.5, rad(x - .3)[0] + rad(x - .3)[1] + .05)], WHITE)   # draught marks

# --- Sea creatures -----------------------------------------------------------------------------
def limb(k, pts, radii, col, n=8, cap=True):
    """Flesh along a path of 3D points; radii = [(sideways, upward)] at each point. `col` is a
    colour or a function (index along the path, angle: 90 = the upper side)."""
    rings = []
    for i, p in enumerate(pts):
        t = _unit(_sub(pts[min(len(pts) - 1, i + 1)], pts[max(0, i - 1)]))
        sd = _unit(_cross((0, 0, 1), t)); up = _cross(t, sd)
        ry, rz = radii[i]
        rings.append([_add(_add(p, sd, ry * cos(j * 2 * pi / n)), up, rz * sin(j * 2 * pi / n)) for j in range(n)])
    pick = col if callable(col) else (lambda i, a: col)
    with k.grp(True):
        for i, (r0, r1) in enumerate(zip(rings, rings[1:])):
            for j in range(n):
                j2 = (j + 1) % n
                k.poly([r0[j], r0[j2], r1[j2], r1[j]], pick(i, (j + .5) * 360 / n))
        if cap: k.poly(rings[0][::-1], pick(0, 90)); k.poly(rings[-1], pick(len(rings) - 2, 90))

def dolphin(k):
    # Allied trained dolphin: a bottlenose dolphin (Tursiops truncatus, 2.8 m), drawn about three
    # times life size, porpoising at the surface in a harness that carries a sonar projector.
    back = rgb(86, 94, 104, 'skin'); flank = rgb(128, 136, 144, 'skin'); belly = rgb(198, 196, 190, 'skin')
    dz = -.13
    spine = [(1.36, .14, .035, .03), (1.25, .17, .06, .05), (1.15, .20, .08, .075), (1.08, .25, .155, .145), (.9, .31, .2, .2),
             (.6, .38, .245, .255), (.2, .43, .265, .285), (-.2, .425, .25, .27), (-.6, .37, .19, .22), (-.9, .29, .12, .16),
             (-1.15, .2, .065, .11), (-1.36, .12, .035, .07)]
    def skin(i, a): return back if 25 < a < 155 else (belly if 215 < a < 325 else flank)
    limb(k, [(x, 0, z + dz) for x, z, _, _ in spine], [(ry, rz) for _, _, ry, rz in spine], skin, n=12)
    # Dorsal fin, flukes, flippers.
    k.loft([[(.16, -.06, .66 + dz), (.16, .06, .66 + dz), (-.34, .05, .62 + dz), (-.34, -.05, .62 + dz)],
            [(-.12, -.04, .88 + dz), (-.12, .04, .88 + dz), (-.36, .03, .84 + dz), (-.36, -.03, .84 + dz)],
            [(-.34, -.02, 1.0 + dz), (-.34, .02, 1.0 + dz), (-.42, .015, .97 + dz), (-.42, -.015, .97 + dz)]], tint(back, .85), smooth=True)
    for s in (-1, 1):
        k.loft([[(-1.3, 0, .07 + dz), (-1.3, 0, .17 + dz), (-1.46, 0, .16 + dz), (-1.46, 0, .08 + dz)],
                [(-1.42, s * .2, .08 + dz), (-1.42, s * .2, .16 + dz), (-1.6, s * .2, .15 + dz), (-1.6, s * .2, .09 + dz)],
                [(-1.6, s * .4, .1 + dz), (-1.6, s * .4, .14 + dz), (-1.68, s * .4, .14 + dz), (-1.68, s * .4, .1 + dz)]], tint(back, .85), smooth=True)
        k.loft([[(.74, s * .2, .24 + dz), (.74, s * .2, .33 + dz), (.5, s * .2, .31 + dz), (.5, s * .2, .23 + dz)],
                [(.5, s * .42, .12 + dz), (.5, s * .42, .19 + dz), (.34, s * .42, .18 + dz), (.34, s * .42, .12 + dz)],
                [(.3, s * .56, .06 + dz), (.3, s * .56, .1 + dz), (.24, s * .56, .1 + dz), (.24, s * .56, .06 + dz)]], tint(back, .92), smooth=True)
        k.ball(.98, s * .165, .31 + dz, .028, BLACK, n=6, k=3)
    # Harness: two straps and a back plate in the owner's colour, the projector on the melon.
    for x, r in ((.42, .268), (.0, .29)):
        k.rev((x - .04, 0, .405 + dz + (.02 if x < .2 else 0)), 'x', [(0, r), (.08, r)], TEAM, n=14)
    k.box(-.02, -.07, .69 + dz, .5, .14, .05, TEAM)
    k.cbox(.3, 0, .72 + dz, .22, .16, .07, DARK); k.tube((.3, 0, .78 + dz), (.22, 0, 1.02 + dz), .012, DARK, n=4)
    k.ball(.98, 0, .47 + dz, .07, rgb(52, 54, 58, 'dark'), n=8, k=4); k.ball(1.03, 0, .47 + dz, .035, PLASMA, n=6, k=3)
    k.strut((.98, .12, .36 + dz), (.98, 0, .5 + dz), .03, TEAM); k.strut((.98, -.12, .36 + dz), (.98, 0, .5 + dz), .03, TEAM)

def giant_squid(k):
    # Soviet giant squid (Architeuthis dux; drawn as a 13 m monster) lying at the surface: mantle
    # with its tail fins, a dinner-plate eye each side, eight arms and the two long clubbed
    # tentacles reaching ahead, a control harness strapped round the mantle.
    c = rgb(132, 66, 58, 'skin'); pale = rgb(188, 142, 128, 'skin'); zc = .22
    def skin(i, a): return c if 10 < a < 170 else pale
    man = [(-6.4, .06), (-5.95, .3), (-5.1, .6), (-3.9, .84), (-2.7, .93), (-1.8, .88), (-1.35, .76)]
    limb(k, [(x, 0, zc) for x, _ in man], [(r, r * .92) for _, r in man], skin, n=12)
    limb(k, [(-1.45, 0, zc), (-.9, 0, zc), (-.35, 0, zc), (.1, 0, zc - .02)], [(.56, .52), (.66, .58), (.6, .52), (.36, .34)], skin, n=12)
    for s in (-1, 1):
        k.extrude([(-6.35, s * .05), (-5.5, s * 1.75), (-4.5, s * 1.65), (-3.7, s * .6), (-4.6, s * .3)], 'xy', zc + .08, zc + .26, c, smooth=True)
        k.ball(-.62, s * .47, zc + .18, .3, rgb(176, 168, 132, 'skin'), n=10, k=5)
        k.ball(-.61, s * .62, zc + .2, .2, rgb(16, 16, 18, 'glass'), n=8, k=4)
    # Harness: two straps, a dorsal spine plate and the control unit with its aerial.
    for x, r in ((-2.6, .95), (-4.4, .76)):
        k.rev((x - .2, 0, zc), 'x', [(0, r), (.4, r)], TEAM, n=16)
    k.box(-4.5, -.22, zc + .7, 2.2, .44, .26, TEAM)
    k.cbox(-3.4, 0, zc + .9, .9, .6, .32, DARK); k.ball(-3.4, 0, zc + 1.26, .13, RED_LAMP, n=6, k=3)
    k.tube((-3.75, 0, zc + 1.2), (-4.1, 0, zc + 2.6), .03, DARK, n=4)
    # Arms and tentacles: deterministic meanders on the surface.
    def path(a0, steps, step, curl, ph, y0):
        pts = [(.05, y0, zc - .02)]; h = a0
        for i in range(steps):
            h = a0 * (1 - .35 * i / steps) + curl * sin(ph + i * .8)
            p = pts[-1]
            pts.append((p[0] + step * cos(radians(h)), p[1] + step * sin(radians(h)), max(.05, .14 + .09 * sin(ph * 1.7 + i * 1.1))))
        return pts
    for j, a0 in enumerate((-68, -46, -27, -10, 10, 27, 46, 68)):
        n_ = 9 + (j * 3) % 3
        pts = path(a0, n_, .46, 26, j * 1.9, a0 / 68 * .28)
        limb(k, pts, [(.2 * (1 - i / n_) + .03, .17 * (1 - i / n_) + .025) for i in range(n_ + 1)], skin, n=6)
    for s in (-1, 1):
        pts = path(s * 5, 13, .47, 13, 1.0 + s, s * .12)
        limb(k, pts, [(.085, .075)] * 11 + [(.2, .07), (.24, .07), (.1, .05)], skin, n=6)

MODELS = {f.__name__: f for f in [destroyer, aegis_cruiser, aircraft_carrier, amphibious_transport, dolphin,
                                    typhoon, sea_scorpion, dreadnought, giant_squid]}

def build(m, ident):
    MODELS[ident](Kit(m, SCALE[ident]))
