"""High-technology and superweapon structures, modelled in metres (see hd_arch.py for the frame).

Their fiction is fantastical, so each one is grounded in the nearest real installation (a nuclear
power station, an early-warning radar post, a bioprocess plant, an ICBM complex, an ionospheric
heater, an accelerator ring, a pulsed-power hall, a direction-finding array, a jamming station, a
satellite earth station) and the invented element is one well-engineered machine at its heart.
"""
from math import sin, cos, pi, radians, degrees, atan2, sqrt
import random
from hd_kit import *
from hd_arch import *

# --- Local parts ------------------------------------------------------------------------------------
SOV_STEEL = rgb(88, 98, 80)            # Soviet equipment green
SOV_CONC = rgb(128, 124, 114, 'concrete')
CERAMIC = rgb(150, 100, 70)            # glazed brown porcelain
CERAMIC_PALE = rgb(196, 190, 176)      # grey-white porcelain (Allied)
LAGGING = rgb(186, 188, 186, 'steel')  # aluminium-clad pipe insulation
FROST = rgb(226, 232, 236)

def ring(x, y, z, r, n, a0=0.0):
    return [(x + r * cos(a0 + i * 2 * pi / n), y + r * sin(a0 + i * 2 * pi / n), z) for i in range(n)]

def placard(k, x, z, s=.8):
    """Radiation placard in a wall's frame (see `face`): yellow board, black trefoil."""
    k.box(x, 0, z, s, .05, s, YELLOW)
    cx, cz = x + s / 2, z + s / 2
    for i in range(3):
        a = pi / 2 + i * 2 * pi / 3
        k.plate([(cx + .1 * s * cos(a), .06, cz + .1 * s * sin(a)), (cx + .4 * s * cos(a - .5), .06, cz + .4 * s * sin(a - .5)),
                 (cx + .4 * s * cos(a + .5), .06, cz + .4 * s * sin(a + .5))], BLACK)

def sign(k, x, z, w=1.0, h=.7, c=HAZ):
    """Warning board in a wall's frame."""
    k.box(x, 0, z, w, .05, h, c)

def beacon(k, x, y, z, c=RED_LAMP, r=.16):
    k.cyl(x, y, z, r * .7, .18, DARK, n=6); k.ball(x, y, z + .18 + r * .7, r, c, n=8, k=4)

def cable_tray(k, pts, z0, c=GALV, w=.5, span=3.0):
    """Ladder-type cable tray through axis-aligned points, on posts down to z0; risers where z changes."""
    for a, b in zip(pts, pts[1:]):
        x0, x1 = sorted((a[0], b[0])); y0, y1 = sorted((a[1], b[1])); zl, zh = sorted((a[2], b[2]))
        if zh - zl > .2:
            k.box(a[0] - w / 2, a[1] - .06, zl, w, .12, zh - zl, c); k.box(a[0] - w / 2 + .08, a[1] - .1, zl, w - .16, .2, zh - zl, RUBBER)
        elif x1 - x0 >= y1 - y0:
            k.box(x0, a[1] - w / 2, a[2], x1 - x0, w, .05, c); k.box(x0, a[1] - w / 2 + .08, a[2] + .05, x1 - x0, w - .16, .09, RUBBER)
            for s in (-1, 1): k.box(x0, a[1] + s * w / 2 - .02, a[2], x1 - x0, .04, .14, c)
            n = max(1, int((x1 - x0) / span))
            for i in range(n + 1): k.strut((x0 + (x1 - x0) * i / n, a[1], z0), (x0 + (x1 - x0) * i / n, a[1], a[2]), .09, c)
        else:
            k.box(a[0] - w / 2, y0, a[2], w, y1 - y0, .05, c); k.box(a[0] - w / 2 + .08, y0, a[2] + .05, w - .16, y1 - y0, .09, RUBBER)
            for s in (-1, 1): k.box(a[0] + s * w / 2 - .02, y0, a[2], .04, y1 - y0, .14, c)
            n = max(1, int((y1 - y0) / span))
            for i in range(n + 1): k.strut((a[0], y0 + (y1 - y0) * i / n, z0), (a[0], y0 + (y1 - y0) * i / n, a[2]), .09, c)

def cable_duct(k, x, y, w, d, z, c=PRECAST):
    """Ground-level concrete cable trench with lifting lids."""
    k.box(x, y, z, w, d, .2, c)
    if w >= d:
        for i in range(1, int(w / .9) + 1): k.box(x + i * .9 - .03, y + .05, z + .2, .06, d - .1, .012, tint(c, .6))
    else:
        for i in range(1, int(d / .9) + 1): k.box(x + .05, y + i * .9 - .03, z + .2, w - .1, .06, .012, tint(c, .6))

def cable_drum(k, x, y, z, r=.9, w=.8, yaw=0.0, c=WOOD, cable=RUBBER):
    """Wooden cable reel standing on its flanges."""
    with k.at(x, y, z, yaw=yaw):
        for yy in (-w / 2, w / 2 - .07): k.rev((0, yy, r), 'y', [(0, r), (.07, r)], c, n=14)
        k.rev((0, -w / 2 + .07, r), 'y', [(0, r * .72), (w - .14, r * .72)], cable, n=12)
        for yy in (-w / 2 - .02, w / 2): k.rev((0, yy, r), 'y', [(0, r * .16), (.02, r * .16)], DARK, n=8)

def cryo_tank(k, x, y, z, r, h, c=WHITE, band=None):
    """Vertical vacuum-insulated cryogenic tank on a skirt, with frost, fill piping and a relief stack."""
    for i in range(4):
        a = pi / 4 + i * pi / 2
        k.tube((x + r * .8 * cos(a), y + r * .8 * sin(a), z), (x + r * .8 * cos(a), y + r * .8 * sin(a), z + 1.1), .08, STEEL, n=6)
    k.cyl(x, y, z, r * .95, .12, CONC_DARK, n=12)
    k.rev((x, y, z + .7), 'z', [(0, r * .45), (r * .3, r * .9), (r * .55, r), (h - r * .55, r), (h - r * .3, r * .9), (h, r * .45)], c, n=16)
    k.rev((x, y, z + .7 + r * .55), 'z', [(0, r + .02), (h * .16, r + .02)], FROST, n=16, caps=False)
    if band: k.rev((x, y, z + .7 + h * .62), 'z', [(0, r + .02), (h * .1, r + .02)], band, n=16, caps=False)
    k.cyl(x, y, z + .7 + h, .12, .5, STEEL, n=6)
    k.pipe([(x + r * .5, y + r + .12, z + .5), (x + r * .5, y + r + .12, z + h * .7), (x + r * .5, y + r * .7, z + h * .7)], .05, STEEL, n=6)
    k.pipe([(x - r * .2, y + r + .12, z + .5), (x - r * .2, y + r + .12, z + h * .45)], .05, COPPER, n=6)
    k.box(x - r * .1, y + r + .02, z + 1.2, .5, .16, .6, GALV)

def genset(k, x, y, z, L=4.2, c=None, yaw=0.0, stack=1.6):
    """Skid-mounted diesel generator in its enclosure: louvres, doors, silencer and exhaust on top."""
    c = c or rgb(96, 108, 92)
    with k.at(x, y, z, yaw=yaw):
        k.box(0, 0, 0, L, 1.9, .3, DARK)
        k.box(.08, .08, .3, L - .16, 1.74, 1.9, c)
        for i in range(6): k.box(.3, 1.82, .62 + i * .22, 1.1, .05, .1, tint(c, .55))
        for xx in (1.7, 2.6): k.box(xx, 1.82, .5, .8, .04, 1.5, tint(c, .86)); k.box(xx + .64, 1.86, 1.2, .05, .03, .16, CHROME)
        k.box(L - .1, .35, .55, .06, 1.2, 1.4, tint(c, .5))
        k.rev((.5, .95, 2.42), 'x', [(0, .1), (.15, .24), (1.5, .24), (1.65, .1)], RUST, n=10)
        k.tube((2.15, .95, 2.42), (2.15, .95, 2.42 + stack), .09, RUST, n=8)
        k.rev((2.15, .95, 2.42 + stack), 'z', [(0, .13), (.12, .13)], DARK, n=8)
        k.box(L - 1.2, .3, 2.2, .9, 1.3, .18, tint(c, .8))

def cabinets(k, x, y, z, n, c=None, w=.8, d=.7, h=2.1, side='y+'):
    """Row of outdoor power-supply cabinets on a plinth; doors on the wall named by `side`."""
    c = c or rgb(176, 180, 176)
    with face(k, side, x, y, z):
        k.box(-.1, -d - .1, 0, n * w + .2, d + .2, .15, CONC_DARK)
        for i in range(n):
            k.box(i * w + .02, -d, .15, w - .04, d, h, c)
            k.box(i * w + .08, 0, .3, w - .16, .03, h - .35, tint(c, .9))
            for j in range(4): k.box(i * w + .16, .03, .5 + j * .09, w - .32, .02, .04, tint(c, .5))
            k.box(i * w + w - .2, .03, 1.3, .05, .03, .18, DARK)
            k.box(i * w + .16, .03, h - .3, .1, .02, .08, GREEN_LAMP if i % 3 else RED_LAMP)
        k.box(-.04, -d - .04, h + .15, n * w + .08, d + .12, .06, tint(c, .8))

def capacitor_bank(k, x, y, z, n=6, tiers=2, yaw=0.0):
    """Outdoor capacitor bank: tiers of cans in a galvanised rack standing on post insulators."""
    L = n * .5 + .3
    can = rgb(150, 156, 150)
    with k.at(x, y, z, yaw=yaw):
        k.box(-.2, -.2, 0, L + .4, 1.5, .12, CONC_DARK)
        for px in (.15, L - .15):
            for py in (.15, .95): insulator(k, px, py, .12, .95, .14, CERAMIC_PALE, n=4)
        for t_ in range(tiers):
            zz = 1.07 + t_ * 1.3
            k.box(0, 0, zz, L, 1.1, .08, GALV)
            for i in range(n):
                for py in (.1, .62):
                    k.box(.2 + i * .5, py, zz + .08, .36, .4, .74, can)
                    k.cyl(.38 + i * .5, py + .2, zz + .82, .05, .22, CERAMIC, n=6)
            for px in (0, L - .06):
                for py in (0, 1.04): k.box(px, py, zz, .06, .06, 1.3, GALV)
            k.strut((.1, .3, zz + 1.08), (L - .1, .3, zz + 1.08), .05, COPPER); k.strut((.1, .82, zz + 1.08), (L - .1, .82, zz + 1.08), .05, COPPER)
        k.box(0, 0, 1.07 + tiers * 1.3, L, 1.1, .06, GALV)

def lightning_mast(k, x, y, z, h, c=GALV):
    """Slender lattice lightning-protection mast with an air terminal and an obstruction lamp."""
    k.cbox(x, y, z, 1.7, 1.7, .3, CONC_DARK)
    k.mast(x, y, z + .3, h * .86, 1.3, .25, c, bays=max(5, int(h / 2.6)), t=.07)
    k.tube((x, y, z + .3 + h * .86), (x, y, z + h), .035, STEEL, n=4)
    k.ball(x, y, z + .3 + h * .86, .13, RED_LAMP, n=6, k=3)

def hyperbolic_tower(k, x, y, z, r, h, c=CONCRETE):
    """Natural-draught cooling tower: a slip-formed hyperboloid shell with its lift joints, on a
    ring of raking columns over the pond; the throat is open, dark and stained inside."""
    zt, rt = h * .76, r * .58
    b = (zt - 1.3) / sqrt((r / rt) ** 2 - 1)
    R = lambda t: rt * sqrt(1 + ((t - zt) / b) ** 2)
    ts = [1.3 + (h - 1.3) * i / 12 for i in range(13)]
    k.cyl(x, y, z - .3, r + .3, .4, CONCRETE, n=28, top=WATER)
    k.rev((x, y, z), 'z', [(t, R(t)) for t in ts], c, n=28, caps=False)
    k.rev((x, y, z), 'z', [(t, R(t) - .22) for t in ts[7:]], tint(c, .5), n=28, caps=False)
    k.rev((x, y, z + ts[7]), 'z', [(0, R(ts[7]) - .2), (.02, .1)], BLACK, n=28)
    k.rev((x, y, z + h - .02), 'z', [(0, R(h) + .04), (.02, R(h) - .24)], tint(c, .8), n=28, caps=False)
    for i in range(1, 6):
        t = 1.3 + (h - 1.3) * i / 6
        k.rev((x, y, z + t), 'z', [(0, R(t) + .025), (.14, R(t + .14) + .025)], tint(c, .88), n=28, caps=False)
    k.rev((x, y, z + 1.3), 'z', [(-.1, r + .08), (.25, R(1.55) + .08)], tint(c, .8), n=28, caps=False)
    for i in range(20):
        a = i * pi / 10
        for s in (0, 1):
            k.tube((x + (r + .12) * cos(a + s * pi / 10), y + (r + .12) * sin(a + s * pi / 10), z), (x + r * .985 * cos(a + pi / 20), y + r * .985 * sin(a + pi / 20), z + 1.4), .1, CONC_DARK, n=4)
    return R(h)

def _icosphere(freq):
    t = (1 + 5 ** .5) / 2
    V = [(-1, t, 0), (1, t, 0), (-1, -t, 0), (1, -t, 0), (0, -1, t), (0, 1, t), (0, -1, -t), (0, 1, -t),
         (t, 0, -1), (t, 0, 1), (-t, 0, -1), (-t, 0, 1)]
    F = [(0, 11, 5), (0, 5, 1), (0, 1, 7), (0, 7, 10), (0, 10, 11), (1, 5, 9), (5, 11, 4), (11, 10, 2), (10, 7, 6), (7, 1, 8),
         (3, 9, 4), (3, 4, 2), (3, 2, 6), (3, 6, 8), (3, 8, 9), (4, 9, 5), (2, 4, 11), (6, 2, 10), (8, 6, 7), (9, 8, 1)]
    def nrm(p):
        L = sqrt(sum(q * q for q in p)); return tuple(q / L for q in p)
    tris = []
    for a, b, c in F:
        A, B, D = V[a], V[b], V[c]
        def P(i, j): return nrm(tuple((A[q] * (freq - i - j) + B[q] * i + D[q] * j) / freq for q in range(3)))
        for i in range(freq):
            for j in range(freq - i):
                tris.append((P(i, j), P(i + 1, j), P(i, j + 1)))
                if i + j < freq - 1: tris.append((P(i + 1, j), P(i + 1, j + 1), P(i, j + 1)))
    return tris

def geodesic(k, x, y, z, r, c=WHITE, freq=3, lo=-.4, seed=3, skirt=.6):
    """Geodesic radome of triangular panels, truncated at height z (its ring beam); `lo` is the
    cut as a fraction of the radius below the centre. Stand it on a roof or a drum."""
    rnd = random.Random(seed); zc = z - lo * r
    with k.grp(False):
        for tri in _icosphere(freq):
            if min(p[2] for p in tri) < lo - .02: continue
            k.poly([(x + r * p[0], y + r * p[1], zc + r * p[2]) for p in tri], tint(c, rnd.uniform(.93, 1.03)))
    lowest = min(min(p[2] for p in tri) for tri in _icosphere(freq) if min(p[2] for p in tri) >= lo - .02)
    rc = r * sqrt(max(.01, 1 - lowest * lowest))
    zr = zc + lowest * r
    k.cyl(x, y, z - skirt, rc + .1, zr - z + skirt + .5, tint(c, .84), n=24)
    k.rev((x, y, zr + .5), 'z', [(0, rc + .16), (.14, rc + .16)], tint(c, .7), n=24)

def mesh_reflector(k, W, H, s0, X0, F, F2, c, ny=10, ns=4, fy=.5, fs=.5, clip=.45, t=.1):
    """Open-mesh parabolic radar reflector in the local frame: it faces +x, spans y in +-W/2 and
    z in s0..s0+H, with main ribs and fine mesh wires. Returns its surface function X(y, s)."""
    sc = s0 + H / 2
    X = lambda y, s: X0 + y * y / (4 * F) + (s - sc) ** 2 / (4 * F2)
    half = lambda y: H / 2 * (1 - clip * (abs(y) / (W / 2)) ** 2.5)
    def ylim(s):
        q = (1 - abs(s - sc) / (H / 2)) / clip
        return W / 2 * min(1.0, max(0.0, q) ** (1 / 2.5))
    nh = max(2, int(round(H / fs))); mh = max(1, nh // ns)
    for j in range(nh + 1):
        s = s0 + H * j / nh; yl = ylim(s)
        if yl < .3: continue
        tt = t if j % mh == 0 else t * .4
        for i in range(ny):
            ya, yb = -yl + 2 * yl * i / ny, -yl + 2 * yl * (i + 1) / ny
            k.strut((X(ya, s), ya, s), (X(yb, s), yb, s), tt, c)
    nv = max(2, int(round(W / fy))); mv = max(1, nv // ny)
    for i in range(nv + 1):
        y = -W / 2 + W * i / nv; hh = half(y)
        tt = t if i % mv == 0 else t * .4
        for j in range(ns):
            sa, sb = sc - hh + 2 * hh * j / ns, sc - hh + 2 * hh * (j + 1) / ns
            k.strut((X(y, sa), y, sa), (X(y, sb), y, sb), tt, c)
    for sg in (-1, 1):   # rim
        for i in range(ny * 2):
            ya, yb = -W / 2 + W * i / (ny * 2), -W / 2 + W * (i + 1) / (ny * 2)
            k.strut((X(ya, sc + sg * half(ya)), ya, sc + sg * half(ya)), (X(yb, sc + sg * half(yb)), yb, sc + sg * half(yb)), t * 1.2, c)
    return X

def big_dish(k, x, y, z, r, az=35.0, el=52.0, c=WHITE):
    """Fully steerable Cassegrain earth-station antenna: concrete ring, conical pedestal, azimuth
    turntable, yoke, elevation axle with bull gear and counterweights, panelled reflector on a
    truss backing structure, quadripod and subreflector, feed cone."""
    hp = r * .62                # pedestal height to the azimuth bearing
    ha = r * .58                # yoke height from the turntable to the elevation axis
    f = r * .72                 # focal length
    x0 = r * .16                # vertex ahead of the elevation axis
    k.cyl(x, y, z, r * .46, .7, CONCRETE, n=20)
    k.rev((x, y, z + .7), 'z', [(0, r * .34), (2.3, r * .34), (hp - 1.1, r * .26), (hp - .9, r * .31), (hp - .5, r * .31)], c, n=20)
    with face(k, 'y+', x - .5, y + r * .335, z + .7): door(k, 0, 1.0, 2.0, c=rgb(96, 104, 110), light=True)
    k.ladder(x + r * .34, y + .3, z + .7, z + hp + .2, axis='y')
    zt = z + .2 + hp
    with k.at(x, y, zt, yaw=az):
        k.cyl(0, 0, 0, r * .36, .35, GALV, n=20)
        k.rev((0, 0, -.12), 'z', [(0, r * .5), (.1, r * .5)], GALV, n=16)
        k.rail(ring(0, 0, -.02, r * .48, 12) + ring(0, 0, -.02, r * .48, 12)[:1], GALV, h=1.0, t=.04, posts=9)
        for s in (-1, 1):
            yy = s * r * .3
            k.extrude([(-r * .2, .35), (r * .2, .35), (r * .07, ha + .35), (-r * .07, ha + .35)], 'xz', yy - .22, yy + .22, c)
            k.rev((0, yy - s * .3, ha), (0, s, 0), [(0, r * .08), (.62, r * .08), (.66, r * .05)], DARK, n=12)
        k.box(-r * .34, -r * .2, .35, r * .2, r * .4, r * .3, tint(c, .9))            # drive cabin
        k.box(-r * .345, -r * .08, .6, .03, r * .16, r * .2, rgb(96, 104, 110))
        with k.at(0, 0, ha, pitch=el):
            k.tube((0, -r * .3, 0), (0, r * .3, 0), r * .05, STEEL, n=10)
            k.torus(-r * .1, 0, 0, r * .27, .07, IRON, n=24, k=5, axis='y')              # elevation bull gear
            k.rev((-r * .06, 0, 0), 'x', [(0, r * .2), (x0 + r * .01, r * .2)], tint(c, .86), n=16)   # hub drum
            for s in (-1, 1):
                k.box(-r * .46, s * r * .3 - .2, -.22, r * .46, .4, .44, tint(c, .86))
                k.box(-r * .52, s * r * .3 - .45, -.5, r * .16, .9, 1.0, IRON)            # counterweights
            prof = lambda q: x0 + (q * r) ** 2 / (4 * f)
            rs = [.12, .34, .56, .78, 1.0]
            for i in range(4):   # panel rings, front and back
                seg = [(prof(rs[i] + (rs[i + 1] - rs[i]) * j / 3), r * (rs[i] + (rs[i + 1] - rs[i]) * j / 3)) for j in range(4)]
                k.rev((0, 0, 0), 'x', seg, tint(c, (1.0, .96, 1.02, .97)[i]), n=32, caps=False)
                k.rev((-.05, 0, 0), 'x', seg, tint(c, .74), n=32, caps=False)
                k.torus(prof(rs[i + 1]) + .01, 0, 0, r * rs[i + 1], .03 if i < 3 else .07, tint(c, .7), n=32, k=4, axis='x')
            for i in range(16):   # panel seams
                a = i * pi / 8
                k.strut((prof(.12) + .02, r * .12 * cos(a), r * .12 * sin(a)), (prof(.56) + .02, r * .56 * cos(a), r * .56 * sin(a)), .03, tint(c, .7))
                k.strut((prof(.56) + .02, r * .56 * cos(a), r * .56 * sin(a)), (prof(1.0) + .02, r * cos(a), r * sin(a)), .03, tint(c, .7))
            xb = -r * .06
            for i in range(16):   # backing structure: radial trusses and hoops
                a, b = i * pi / 8, (i + 1) * pi / 8
                P = lambda q, xx, aa=a: (xx, r * q * cos(aa), r * q * sin(aa))
                k.strut(P(.2, xb), P(.98, prof(.98) - .12), .09, GALV)
                for q in (.48, .74):
                    xm = xb + (prof(.98) - .12 - xb) * (q - .2) / .78
                    k.strut(P(q, xm), P(q, prof(q) - .05), .06, GALV)
                    k.strut(P(q, xm), (xm, r * q * cos(b), r * q * sin(b)), .06, GALV)
                    k.strut(P(q, xm), P(q + .24, prof(q + .24) - .05), .05, GALV)
            xs = x0 + f * .9
            for i in range(4):    # quadripod
                a = pi / 4 + i * pi / 2
                k.strut((prof(.8), r * .8 * cos(a), r * .8 * sin(a)), (xs + .1, r * .1 * cos(a), r * .1 * sin(a)), .1, tint(c, .9))
            k.rev((xs + .3, 0, 0), (-1, 0, 0), [(0, r * .13), (r * .03, r * .125), (r * .07, r * .03)], tint(c, .95), n=16)
            k.rev((xs + .3, 0, 0), 'x', [(0, r * .13), (.12, r * .12)], tint(c, .8), n=16)
            k.rev((x0, 0, 0), 'x', [(0, r * .13), (r * .32, r * .06), (r * .34, r * .075)], tint(c, .92), n=12)
    return zt

# --- Soviet ------------------------------------------------------------------------------------------
def nuclear_reactor(k, w, d):
    # Soviet nuclear power station (VVER type): prestressed-concrete containment under its dome,
    # natural-draught cooling tower, brick turbine hall, braced ventilation stack, switchyard.
    z = pad(k, w, d, c=CONC_DARK)
    shell = rgb(172, 168, 158, 'concrete')
    # Cooling tower over its basin (back left).
    tx, ty, tr, th = 5.5, 5.6, 4.7, 17.6
    rtop = hyperbolic_tower(k, tx, ty, z + .3, tr, th, shell)
    for i in range(4): beacon(k, tx + (rtop - .1) * cos(.6 + i * pi / 2), ty + (rtop - .1) * sin(.6 + i * pi / 2), z + .3 + th, r=.14)
    # Reactor containment: buttressed drum, ring girder, dome.
    cx, cy, cr, ch = 14.4, 5.8, 3.7, 9.2
    k.cyl(cx, cy, z, cr + .5, 1.0, CONC_DARK, n=24)
    k.cyl(cx, cy, z, cr, ch, CONCRETE, n=24)
    for i in range(8):
        with k.at(cx, cy, z, yaw=22.5 + i * 45): k.box(cr - .12, -.4, 0, .5, .8, ch, CONC_LIGHT)
    k.rev((cx, cy, z + 7.7), 'z', [(0, cr + .035), (.8, cr + .035)], TEAM, n=24, caps=False)
    k.rev((cx, cy, z + ch), 'z', [(0, cr + .45), (.75, cr + .45), (.75, cr - .2)], CONC_DARK, n=24)
    k.dome(cx, cy, z + ch + .75, cr - .1, CONC_LIGHT, h=3.2, n=24, k=7)
    k.rail(ring(cx, cy, z + ch + .75, cr + .38, 16) + ring(cx, cy, z + ch + .75, cr + .38, 16)[:1], GALV, h=1.0, t=.04, posts=9)
    k.cyl(cx, cy, z + ch + 3.85, .5, .5, STEEL, n=10); k.tube((cx, cy, z + ch + 4.3), (cx, cy, z + ch + 6.0), .035, STEEL, n=4)
    with k.at(cx, cy, z, yaw=45):    # equipment hatch and its placard
        k.rev((cr - .15, 0, 4.2), 'x', [(0, 1.45), (.4, 1.45), (.5, 1.2)], rgb(120, 126, 120), n=16)
        for i in range(8): k.ball(cr + .27, 1.3 * cos(i * pi / 4), 4.2 + 1.3 * sin(i * pi / 4), .09, DARK, n=6, k=3)
    k.ladder(cx + cr * cos(1.9) + .08, cy + cr * sin(1.9) + .12, z + 1.0, z + ch + .8, axis='x')
    # Auxiliary and fuel-handling building against the containment.
    aux = rgb(134, 128, 116, 'concrete')
    top = block(k, 12.4, 9.2, 6.3, 3.5, 5.6, aux, roof=ROOF, plinth=CONC_DARK, floors=1, glaze=None)
    with face(k, 'y+', 12.4, 12.7, z):
        punched(k, 6.3, 1, z0=3.2, h=1.2, bay=1.6, margin=.8); door(k, 4.6, 1.0, c=rgb(84, 60, 50)); placard(k, 3.3, 1.1, .8)
    with face(k, 'x+', 18.7, 12.7, z): punched(k, 3.5, 1, z0=3.2, h=1.2, bay=1.6, margin=.7)
    ac_unit(k, 13.0, 9.8, top, 1.6, 1.0, .8); vent(k, 16.2, 10.4, top, .3, .9); vent(k, 17.6, 10.4, top, .3, .9)
    k.box(14.6, 10.9, top, 2.6, .5, .5, GALV)
    # Turbine hall: brick with concrete pilasters and tall machine-hall windows.
    hx, hy, hw, hd, hh = 1.5, 11.0, 9.7, 5.2, 6.4
    k.box(hx, hy, z, hw, hd, hh, BRICK); k.box(hx - .08, hy - .08, z, hw + .16, hd + .16, .9, CONC_DARK)
    for x in (hx, hx + 3.2, hx + 6.4, hx + hw - .4): k.box(x, hy + hd - .02, z, .4, .2, hh, BRICK_DARK)
    k.box(hx - .15, hy - .15, z + hh, hw + .3, hd + .3, .45, CONC_DARK)
    gable(k, hx, hy, z + hh + .45, hw, hd, 1.5, CLAD_RUST, ridge='x', wall=BRICK)
    k.box(hx + 1.0, hy + hd / 2 - .6, z + hh + 1.55, hw - 2.0, 1.2, .6, CLAD_RUST); k.box(hx + .9, hy + hd / 2 - .75, z + hh + 2.15, hw - 1.8, 1.5, .12, RUST)
    with face(k, 'y+', hx, hy + hd, z):
        for x in (.9, 4.1, 7.3):
            k.box(x, -.08, 2.0, 1.8, .14, 3.2, GLASS_GREEN); k.box(x - .1, -.02, 1.86, 2.0, .16, .14, CONC_LIGHT)
            k.box(x + .86, -.02, 2.0, .08, .1, 3.2, DARK); k.box(x, -.02, 3.5, 1.8, .1, .08, DARK)
        door(k, 3.0, 1.0, c=rgb(84, 60, 50))
        k.box(.5, .02, 5.55, hw - 1.0, .06, .6, TEAM)
    with face(k, 'x+', hx + hw, hy + hd, z):
        shutter(k, 1.0, 3.0, 3.6, c=remat(rgb(96, 70, 56), 'corr'), stripes=True)
        k.box(1.4, -.08, 4.4, 2.2, .14, 1.2, GLASS_GREEN); k.box(1.3, -.02, 4.28, 2.4, .16, .12, CONC_LIGHT)
    # Main steam lines on a pipe bridge from the reactor building to the turbine hall.
    for i, (yy, zz) in enumerate(((13.3, 4.3), (13.95, 3.5))):
        pipe_run(k, [(12.8 + i * .7, 12.7, z + zz), (12.8 + i * .7, yy, z + zz), (hx + hw, yy, z + zz)], .28, LAGGING)
    trestle(k, 12.2, 13.6, z, z + 3.1, 1.3, STEEL, 'y')
    # Braced ventilation stack (back right).
    sx, sy = 18.1, 2.6
    chimney(k, sx, sy, z, 21.0, .78, rgb(178, 176, 168, 'concrete'), bands=4, taper=.8)
    k.mast(sx, sy, z, 12.4, 2.8, 2.2, rgb(110, 96, 84), bays=5, t=.1)
    # Switchyard: unit transformers under a take-off gantry, fenced.
    apron(k, 12.4, 14.6, 7.0, 4.8, z, GRAVEL)
    transformer(k, 14.3, 17.0, z + .15, rgb(92, 100, 84)); transformer(k, 17.3, 17.0, z + .15, rgb(92, 100, 84))
    for x in (12.9, 18.9): k.mast(x, 15.2, z, 6.3, .7, .5, GALV, bays=4, t=.07)
    k.truss((12.9, 15.2, z + 5.8), (18.9, 15.2, z + 5.8), .6, .5, GALV, bays=6, t=.06)
    for x in (14.3, 15.8, 17.3):
        insulator(k, x, 15.2, z + 4.8, 1.0, .14); k.strut((x, 15.2, z + 4.8), (x, 17.2, z + 3.0), .03, COPPER)
    fence(k, [(12.4, 19.5, z), (19.5, 19.5, z), (19.5, 14.0, z)], h=2.0, c=IRON)
    with face(k, 'y+', 15.2, 19.56, z + .8): sign(k, 0, 0, 1.1, .7)
    # Yard: diesel day tanks, perimeter fence with radiation placards, flag.
    htank(k, 2.0, 17.9, z, 4.0, .75, rgb(108, 112, 96), axis='x')
    barrels(k, 7.0, 17.3, z, 3, c=YELLOW); crate(k, 9.6, 17.2, z, 1.1, c=TIMBER)
    fence(k, [(.8, 19.5, z), (11.6, 19.5, z)], h=2.0, c=IRON)
    for x in (2.6, 7.4):
        with face(k, 'y+', x, 19.56, z + .9): placard(k, 0, 0, .8)
    flagpole(k, 11.0, 18.3, z, 8.5, w=2.2)
    floodlight(k, 1.2, 16.9, z, 7.5, yaw=20)

def radar_tower(k, w, d):
    # Soviet early-warning radar post (P-14 class): an open-mesh parabolic array turning on a
    # lattice tower, brick operations block, trailer-mounted height-finder, generator shed.
    z = pad(k, w, d, c=CONC_DARK)
    col = SOV_STEEL
    tx, ty, th = 6.6, 6.6, 14.5
    for sx in (-1, 1):
        for sy in (-1, 1): k.cbox(tx + sx * 2.2, ty + sy * 2.2, z, 1.2, 1.2, .5, CONCRETE)
    k.mast(tx, ty, z + .4, th, 4.4, 2.6, col, bays=5, t=.17)
    zt = z + .4 + th
    k.cbox(tx, ty, zt, 4.4, 4.4, .16, GALV)
    k.rail([(tx - 2.2, ty - 2.2, zt + .16), (tx + 2.2, ty - 2.2, zt + .16), (tx + 2.2, ty + 2.2, zt + .16), (tx - 2.2, ty + 2.2, zt + .16), (tx - 2.2, ty - 2.2, zt + .16)], GALV, h=1.0)
    k.ladder(tx, ty + .5, z + .4, zt, axis='x'); k.tube((tx + .5, ty - .3, z + .4), (tx + .5, ty - .3, zt), .09, RUBBER, n=6)
    for f in (.4, .8): k.cbox(tx, ty, z + .4 + th * f, 2.0, 2.0, .08, GALV)
    with k.at(tx, ty, zt + .16, yaw=40):
        k.cyl(0, 0, 0, 1.25, .4, DARK, n=16)
        k.cbox(-.3, 0, .4, 2.6, 2.3, 1.7, col); k.box(1.01, -.5, .7, .03, 1.0, 1.1, tint(col, .8))
        k.box(-1.62, -1.17, 1.5, 2.64, 2.34, .5, TEAM)
        W, H, s0 = 11.0, 6.0, 2.5
        X = mesh_reflector(k, W, H, s0, .7, 5.0, 9.0, col, ny=10, ns=4, t=.15)
        sc = s0 + H / 2
        k.truss((-.5, -4.4, sc - .6), (-.5, 4.4, sc - .6), 1.0, 1.2, col, bays=8, t=.09)
        for y in (-4.4, -2.2, 0, 2.2, 4.4):
            for s, sz in ((sc - 2.2, sc - .6), (sc + 2.2, sc + .6)): k.strut((0, y, sz), (X(y, s), y, s), .08, col)
        for y in (-1.0, 1.0):
            k.strut((-.9, y, 2.1), (-.9, y * 1.6, sc - .6), .14, col); k.strut((.4, y, 2.1), (0, y * 1.6, sc - .6), .14, col)
        fx, fz = .7 + 4.0, sc - .4       # feed horn on its boom
        k.strut((X(0, s0), 0, s0), (fx, 0, fz), .12, col)
        for y in (-3.0, 3.0): k.strut((X(y, s0 + .6), y, s0 + .6), (fx, 0, fz), .06, col)
        k.rev((fx + .1, 0, fz), (-1, 0, .05), [(0, .14), (.8, .5)], DARK, n=4)
    # Operations block.
    top = block(k, 1.6, 11.6, 8.6, 6.4, 6.8, BRICK, roof=ROOF, plinth=CONC_DARK, floors=2, glaze='punched', band=TEAM)
    k.box(1.45, 11.45, z + 6.8, 8.9, 6.7, .18, CONC_DARK)
    with face(k, 'y+', 1.6, 18.0, z):
        door(k, 3.8, 1.1, c=rgb(84, 60, 50)); k.box(3.4, 0, 2.5, 1.9, 1.0, .12, CONC_DARK)
    for x, y in ((3.0, 13.2), (5.4, 13.2), (8.2, 16.0)): vent(k, x, y, top, .28, .8)
    k.box(6.4, 14.6, top, 2.0, 1.4, 1.1, CLAD_RUST); k.box(6.3, 14.5, top + 1.1, 2.2, 1.6, .1, RUST)
    k.tube((2.6, 16.6, top), (2.6, 16.6, top + 4.6), .06, GALV, n=6)        # IFF yagi mast
    for i, zz in enumerate((3.2, 4.3)):
        with k.at(2.6, 16.6, top + zz, yaw=40 + i * 90):
            k.strut((-.3, 0, 0), (1.9, 0, 0), .05, GALV)
            for j in range(5): k.strut((-.2 + j * .45, -.55 + j * .05, 0), (-.2 + j * .45, .55 - j * .05, 0), .035, GALV)
    # Height-finder on its trailer cabin (front right).
    hx, hy = 12.8, 12.6
    k.box(hx, hy, z + .85, 5.0, 2.5, 2.3, col); k.box(hx - .05, hy - .05, z + 3.15, 5.1, 2.6, .1, tint(col, .8))
    for x in (hx + .9, hx + 3.9):
        for y in (hy + .25, hy + 2.25): k.wheel(x, y, z + .5, .5, .3)
    for x in (hx + .2, hx + 4.8):
        for y in (hy + .1, hy + 2.4): k.tube((x, y, z), (x, y, z + .85), .06, STEEL, n=6)
    with face(k, 'y+', hx, hy + 2.5, z + .85): k.box(3.4, 0, .1, .8, .04, 1.8, tint(col, .8)); k.box(.6, -.04, 1.2, 1.2, .08, .6, GLASS)
    k.stairs(hx + 3.8, hy + 3.5, z, z + .9, -1.0, .8, axis='y')
    with k.at(hx + 1.6, hy + 1.25, z + 3.25, yaw=62):
        k.cyl(0, 0, 0, .55, .5, DARK, n=12)
        for y in (-.5, .5): k.strut((0, y, .5), (.1, y, 2.0), .12, col)
        with k.at(0, 0, 2.0, pitch=-8):
            Xh = mesh_reflector(k, 2.0, 5.2, -2.1, .35, 3.0, 2.4, col, ny=4, ns=8, fy=.33, fs=.4, clip=.25, t=.08)
            k.strut((0, -.5, 0), (0, .5, 0), .14, col)
            for s in (-1.4, 1.4): k.strut((0, 0, 0), (Xh(0, s), 0, s), .08, col)
            k.strut((Xh(0, -2.0), 0, -2.0), (2.2, 0, .5), .08, col); k.rev((2.3, 0, .52), (-1, 0, 0), [(0, .1), (.5, .32)], DARK, n=4)
    # Generator shed, fuel tank, cable trays.
    sx, sy = 13.2, 2.6
    k.box(sx, sy, z, 5.4, 4.2, 3.0, CLAD_GREEN); k.box(sx - .05, sy - .05, z, 5.5, 4.3, .5, CONC_DARK)
    gable(k, sx, sy, z + 3.0, 5.4, 4.2, 1.0, CLAD_RUST, ridge='x', wall=CLAD_GREEN)
    with face(k, 'y+', sx, sy + 4.2, z):
        door(k, .6, 1.0, c=rgb(84, 60, 50))
        for i in range(6): k.box(2.4, 0, 1.0 + i * .24, 2.2, .06, .1, DARK)
    k.pipe([(sx + 4.6, sy + 4.2, z + 2.2), (sx + 4.6, sy + 4.7, z + 2.2), (sx + 4.6, sy + 4.7, z + 5.4)], .11, RUST, n=8)
    k.rev((sx + 4.6, sy + 4.7, z + 3.0), 'z', [(0, .22), (1.2, .22)], RUST, n=8)
    htank(k, 13.6, 9.2, z + .1, 4.2, .75, rgb(104, 110, 92), axis='x')
    k.box(13.2, 8.1, z, 5.0, .15, .6, CONC_DARK); k.box(13.2, 10.2, z, 5.0, .15, .6, CONC_DARK); k.box(18.05, 8.1, z, .15, 2.25, .6, CONC_DARK)
    cable_tray(k, [(13.2, 5.8, z + 2.4), (9.0, 5.8, z + 2.4)], z)
    cable_tray(k, [(7.6, 8.9, z + 2.4), (7.6, 11.6, z + 2.4)], z)
    # Fence, gate, flag, lights.
    fence(k, [(.8, 19.5, z), (9.6, 19.5, z)], h=2.0, c=IRON); fence(k, [(13.2, 19.5, z), (19.5, 19.5, z), (19.5, 11.6, z)], h=2.0, c=IRON)
    for x in (9.6, 13.2): k.cbox(x, 19.5, z, .35, .35, 2.4, BRICK_DARK)
    with face(k, 'y+', 15.6, 19.56, z + .8): sign(k, 0, 0, 1.1, .7)
    flagpole(k, 10.6, 17.2, z, 8.0, w=2.2)
    barrels(k, 10.9, 13.0, z, 2, c=rgb(84, 92, 70), rows=2); crate(k, 11.0, 11.4, z, 1.0, c=TIMBER)
    floodlight(k, 18.9, 11.0, z, 7.0, yaw=200)

def culture_vessel(k, x, y, z, r=1.45, h=8.2):
    """Glass bioreactor column: steel skirt and bottom head, glass shell full of lit culture
    medium, steel top head with agitator drive and nozzles."""
    liquid = C((36, 104, 62), 'water')
    k.cyl(x, y, z, r + .25, .35, CONC_DARK, n=20)
    k.rev((x, y, z + .35), 'z', [(0, r * .72), (.5, r * .76), (1.1, r + .07), (1.4, r + .07)], STEEL, n=20)
    k.cyl(x, y, z + 1.75, r, h - 4.2, liquid, n=20)
    k.cyl(x, y, z + 1.75, r + .01, .9, tint(liquid, .6), n=20)
    k.cyl(x, y, z + h - 2.6, r + .01, .16, C((120, 170, 120), 'water'), n=20)
    k.cyl(x, y, z + h - 2.45, r, 1.1, GLASS_GREEN, n=20)
    for zz in (1.72, 1.75 + (h - 4.2) / 3, 1.75 + (h - 4.2) * 2 / 3, h - 2.5, h - 1.42): k.rev((x, y, z + zz), 'z', [(0, r + .07), (.14, r + .07)], STEEL, n=20, caps=False)
    for i in range(8):
        a = i * pi / 4 + .2
        k.strut((x + (r + .09) * cos(a), y + (r + .09) * sin(a), z + 1.7), (x + (r + .09) * cos(a), y + (r + .09) * sin(a), z + h - 1.3), .08, STEEL)
    k.rev((x, y, z + h - 1.35), 'z', [(0, r + .09), (.32, r + .09), (.72, r * .8), (.98, r * .36)], STEEL, n=20)
    k.rev((x, y, z + h - 1.33), 'z', [(0, r + .11), (.28, r + .11)], TEAM, n=20, caps=False)
    k.cyl(x, y, z + h - .4, .3, .95, SOV_STEEL, n=10); k.cyl(x, y, z + h + .55, .42, .3, DARK, n=10)
    for a in (.8, 2.6): k.cyl(x + r * .6 * cos(a), y + r * .6 * sin(a), z + h - .8, .12, .55, STEEL, n=6)
    k.box(x - .25, y + r + .05, z + 2.4, .5, .12, .7, GALV); k.box(x - .12, y + r + .17, z + 2.9, .1, .02, .1, GREEN_LAMP)

def cloning_vats(k, w, d):
    # Soviet biotechnology plant: a laboratory block with heavy extract ventilation, and a row of
    # four glass culture vessels on a pipe manifold with an access gantry and liquid-nitrogen store.
    z = pad(k, w, d, c=CONC_DARK)
    # Laboratory block.
    top = block(k, 1.5, 1.5, 12.0, 6.8, 8.4, BRICK, roof=ROOF, plinth=CONC_DARK, floors=2, glaze=None, band=TEAM)
    for x in (1.5, 5.4, 9.3, 13.1): k.box(x, 8.28, z, .4, .2, 8.4, SOV_CONC)
    for zz in (4.0, 7.6): k.box(1.5, 8.28, z + zz, 12.0, .12, .35, SOV_CONC)
    with face(k, 'y+', 1.5, 8.3, z):
        punched(k, 12.0, 1, z0=5.1, h=1.7, w=1.3, bay=1.95, margin=.9, glass=GLASS_GREEN)
        punched(k, 12.0, 1, z0=1.3, h=1.7, w=1.3, bay=1.95, margin=.9, glass=GLASS_GREEN)
    with face(k, 'x+', 13.5, 8.3, z):
        punched(k, 6.8, 2, z0=1.3, h=1.7, w=1.2, bay=2.2, margin=1.0, floor_h=3.8, glass=GLASS_GREEN)
    # Extract ventilation: fan housings, ducts and fume stacks on the roof, a filter house below.
    for i, x in enumerate((3.0, 7.2)):
        k.box(x, 2.6, top, 2.6, 2.2, 1.5, GALV); k.box(x - .05, 2.55, top + 1.5, 2.7, 2.3, .08, tint(GALV, .8))
        for j in range(5): k.box(x + .2, 4.8, top + .25 + j * .22, 2.2, .05, .1, DARK)
        k.pipe([(x + 1.3, 3.2, top + 1.5), (x + 1.3, 3.2, top + 2.3), (x + 1.3, 5.6, top + 2.3), (x + 1.3, 5.6, top + .2)], .42, GALV, n=10)
    for x in (10.6, 11.6, 12.6):
        k.cyl(x, 2.6, top, .26, 5.2, rgb(150, 150, 144, 'steel'), n=10); k.rev((x, 2.6, top + 5.2), 'z', [(0, .26), (.25, .4), (.3, .4)], DARK, n=10)
        k.strut((x, 2.6, top + 3.2), (x, 3.6, top), .03, STEEL)
    k.box(10.2, 4.6, top, 2.8, 1.8, 1.2, CLAD_RUST); k.box(10.1, 4.5, top + 1.2, 3.0, 2.0, .08, RUST)
    k.pipe([(13.3, 5.4, top + .7), (14.5, 5.4, top + .7), (14.5, 5.4, z + 3.0)], .5, GALV, n=10)
    k.box(13.7, 2.4, z, 1.9, 4.0, 3.0, CLAD_GREEN); k.box(13.65, 2.35, z + 3.0, 2.0, 4.1, .1, RUST)
    for j in range(6): k.box(15.6, 3.0, z + .8 + j * .28, .05, 2.6, .12, DARK)
    # The culture vessels and their gantry.
    vx = (3.3, 6.9, 10.5, 14.1); vy = 12.4; vh = 8.2
    for x in vx: culture_vessel(k, x, vy, z, 1.45, vh)
    gz = z + 6.9
    k.box(1.6, 9.9, gz - .1, 15.4, 1.0, .1, GALV)
    k.rail([(1.6, 9.9, gz), (17.0, 9.9, gz)], GALV); k.rail([(1.6, 10.9, gz), (1.6, 9.9, gz)], GALV)
    for x0, x1 in ((1.6, vx[0] - .5), (vx[0] + .5, vx[1] - .5), (vx[1] + .5, vx[2] - .5), (vx[2] + .5, vx[3] - .5), (vx[3] + .5, 16.1)):
        k.rail([(x0, 10.9, gz), (x1, 10.9, gz)], GALV)
    for x in (1.8, 5.1, 8.7, 12.3, 15.9):
        for y in (10.0, 10.8): k.strut((x, y, z), (x, y, gz - .1), .12, STEEL)
        k.strut((x, 10.0, z + 3.0), (x, 10.8, gz - .2), .07, STEEL)
    k.stairs(16.1, 18.3, z, gz, -7.4, .9, axis='y')
    for y in (14.6,): k.strut((16.1, y, z), (16.1, y, z + 3.4), .09, STEEL); k.strut((17.0, y, z), (17.0, y, z + 3.4), .09, STEEL)
    # Feed header over the vessel heads and harvest manifold along their feet, with valves.
    hz = z + vh + .25
    k.pipe([(1.9, 11.4, z + .4), (1.9, 11.4, hz), (vx[3], 11.4, hz)], .16, rgb(84, 118, 92), n=8)
    for x in vx: k.pipe([(x, 11.4, hz), (x, 11.9, hz), (x, 11.9, z + vh - .6)], .1, rgb(84, 118, 92), n=6)
    k.pipe([(2.0, 14.6, z + .7), (16.0, 14.6, z + .7)], .2, LAGGING, n=8)
    k.pipe([(2.0, 15.2, z + .45), (15.2, 15.2, z + .45)], .14, RUST, n=8)
    for x in vx:
        k.pipe([(x, vy + 1.1, z + .9), (x, 14.6, z + .9), (x, 14.6, z + .7)], .12, LAGGING, n=6)
        k.torus(x + .5, 14.6, z + 1.15, .2, .035, SIGNAL_RED, n=10, k=4); k.cyl(x + .5, 14.6, z + .7, .04, .45, STEEL, n=4)
    for x in (2.6, 5.1, 8.7, 12.3, 15.6): k.cbox(x, 14.9, z, .9, .25, .35, CONC_DARK)
    # Liquid-nitrogen store and vaporiser.
    cryo_tank(k, 17.9, 3.4, z, .95, 6.4); cryo_tank(k, 17.9, 6.2, z, .95, 5.6)
    for i in range(6): k.box(16.9 + i * .28, 7.8, z + .3, .06, 1.1, 2.4, GALV)
    k.box(16.8, 7.7, z, 1.9, 1.3, .3, CONC_DARK)
    k.pipe([(17.6, 8.9, z + 1.4), (17.6, 9.4, z + 1.4), (17.6, 9.4, z + 5.6), (16.0, 9.4, z + 5.6)], .08, FROST, n=6)
    # Yard: chemical drums, gas cylinders, crates, markings, flag.
    barrels(k, 2.2, 16.6, z, 4, c=rgb(60, 84, 110), rows=2); barrels(k, 5.4, 17.0, z, 2, c=YELLOW)
    for i in range(5): k.cyl(8.0 + i * .34, 17.6, z, .14, 1.5, (rgb(84, 118, 92), rgb(120, 60, 50))[i % 2], n=8); k.cyl(8.0 + i * .34, 17.6, z + 1.5, .05, .15, BRASS, n=5)
    k.box(7.75, 17.35, z, 1.9, .06, 1.0, GALV); k.box(7.75, 17.8, z, 1.9, .06, 1.0, GALV)
    crate(k, 11.2, 16.9, z, 1.2, c=TIMBER); crate(k, 12.7, 17.2, z, 1.0, c=TIMBER, yaw=12)
    hazard_edge(k, 1.2, 15.9, 14.2, .25, z)
    with face(k, 'y+', 9.9, 8.42, z + 1.6): sign(k, 0, 0, .9, .7)
    flagpole(k, 2.0, 18.9, z, 8.0, w=2.2)
    floodlight(k, 19.1, 13.2, z, 7.5, yaw=200)

def nuclear_silo(k, w, d):
    # Soviet ICBM launch complex: a hardened pad with the silo's split blast doors run open on
    # their rails and the missile raised in the shaft, a service gantry with a loading jib, a
    # buried control bunker, propellant tank farm, lightning masts, double perimeter fence.
    z = pad(k, w, d, c=CONC_DARK, joints=6.0)
    cx, cy = 13.0, 13.0
    ph = 1.4; zp = z + ph
    pc = rgb(150, 146, 136, 'concrete')
    # The pad, cast round the shaft.
    for bx, by, bw, bd in ((5, 6, 5, 14), (16, 6, 5, 14), (10, 6, 6, 4), (10, 16, 6, 4)): k.box(bx, by, z, bw, bd, ph, pc)
    k.extrude([(9.0, 0), (17.0, 0), (17.0, ph), (9.0, ph)], 'xz', 19.98, 20.0, pc)
    k.extrude([(20.0, z), (24.6, z), (20.0, zp)], 'yz', 9.5, 16.5, pc)              # vehicle ramp
    for x in (9.5, 16.35): k.box(x, 20.0, z, .15, 4.6, .3, CONC_DARK)
    hazard_edge(k, 5.05, 19.6, 4.4, .35, zp - .03); hazard_edge(k, 16.6, 19.6, 4.35, .35, zp - .03)
    hazard_edge(k, 20.6, 6.1, .35, 13.8, zp - .03)
    # Shaft collar, the dark shaft and scorch round it.
    Ro, Ri = 4.35, 3.0
    k.rev((cx, cy, zp), 'z', [(0, Ro), (.55, Ro - .25), (.55, Ri), (-ph + .1, Ri)], CONC_LIGHT, n=28, caps=False)
    k.rev((cx, cy, z + .12), 'z', [(0, Ri), (.02, .1)], BLACK, n=28)
    k.rev((cx, cy, zp + .55), 'z', [(0, Ri + .5), (.02, Ri + .02)], HAZ, n=28, caps=False)
    k.rev((cx, cy, zp - .01), 'z', [(0, Ri - .02), (ph * -.6, Ri - .02)], rgb(44, 42, 40, 'concrete'), n=28, caps=False)
    # The missile: first stage in the shaft, interstage, upper stage and warhead fairing above it.
    mc = SOV_GREEN
    k.rev((cx, cy, z + .15), 'z', [(0, 1.5), (5.2, 1.5)], mc, n=20)
    k.rev((cx, cy, z + 3.4), 'z', [(0, 1.515), (.45, 1.515)], WHITE, n=20, caps=False)
    k.rev((cx, cy, z + 3.95), 'z', [(0, 1.515), (.6, 1.515)], TEAM, n=20, caps=False)
    k.rev((cx, cy, z + 5.35), 'z', [(0, 1.5), (.25, 1.5), (.25, 1.38), (.7, 1.3)], DARK, n=20)          # interstage truss ring
    k.rev((cx, cy, z + 6.05), 'z', [(0, 1.3), (1.9, 1.3), (2.5, 1.12)], mc, n=20)
    k.rev((cx, cy, z + 6.9), 'z', [(0, 1.315), (.3, 1.315)], WHITE, n=20, caps=False)
    k.rev((cx, cy, z + 8.55), 'z', [(0, 1.12), (.6, 1.04), (1.4, .82), (2.2, .52), (2.8, .24), (3.1, .03)], rgb(186, 184, 174), n=20)
    k.rev((cx, cy, z + 11.15), 'z', [(0, .34), (.25, .2), (.5, .03)], DARK, n=12)
    # Blast doors on their rails, driven by hydraulic rams.
    dc = rgb(98, 104, 94)
    for y in (cy - 3.45, cy + 3.45):
        k.box(5.1, y - .25, zp, 15.8, .5, .5, CONC_DARK); k.box(5.1, y - .09, zp + .5, 15.8, .18, .1, STEEL)
    for sgn, x0 in ((-1, 5.2), (1, 17.5)):
        k.box(x0, cy - 3.9, zp + .62, 3.3, 7.8, .85, dc)
        for i in range(4): k.box(x0 + .25 + i * .8, cy - 3.8, zp + 1.47, .22, 7.6, .14, tint(dc, .8))
        for yy in (cy - 3.9, cy + 3.72): k.box(x0, yy, zp + 1.47, 3.3, .18, .16, tint(dc, .75))
        k.box(x0 + (.02 if sgn < 0 else 2.63), cy - 3.7, zp + 1.47, .65, 7.4, .15, TEAM)
        xe = x0 + (3.3 if sgn < 0 else -.06)
        k.box(xe, cy - 3.9, zp + .62, .06, 7.8, .85, HAZ)
        for yy in (cy - 3.45, cy + 3.45):
            for xx in (x0 + .5, x0 + 2.8): k.rev((xx, yy - .22, zp + .68), 'y', [(0, .32), (.44, .32)], DARK, n=10)
        xr = x0 + (3.3 if sgn < 0 else 0)
        for yy in (cy - 2.4, cy + 2.4):
            k.tube((xr, yy, zp + .9), (xr - sgn * 1.3, yy, zp + .9), .12, CHROME, n=8)
    # Exhaust-duct grilles in the pad.
    for x in (10.6, 13.6):
        k.box(x, 17.2, zp, 1.9, 2.0, .04, BLACK)
        for i in range(7): k.box(x + .05 + i * .29, 17.2, zp + .04, .08, 2.0, .05, IRON)
    # Service gantry behind the shaft: lattice tower, work levels, swing arms, loading jib.
    gx, gy, gh = 8.9, 7.8, 17.0
    yaw = degrees(atan2(cy - gy, cx - gx)); dist = sqrt((cx - gx) ** 2 + (cy - gy) ** 2)
    gc = rgb(150, 84, 60)
    with k.at(gx, gy, zp, yaw=yaw):
        k.mast(0, 0, 0, gh, 2.8, 2.8, gc, bays=7, t=.15)
        k.box(-.7, -.7, 0, 1.4, 1.4, gh, rgb(112, 116, 108, 'corr'))                # lift shaft
        for hz in (4.4, 7.4, 10.4):
            k.box(-1.6, -1.6, hz, 3.2 + 1.2, 3.2, .1, GALV)
            k.rail([(2.8, -1.6, hz + .1), (-1.6, -1.6, hz + .1), (-1.6, 1.6, hz + .1), (2.8, 1.6, hz + .1)], GALV, h=1.0)
        for hz, yy in ((4.4, -.9), (7.4, .9)):                                       # swing arms and umbilicals
            k.truss((2.8, yy, hz - .5), (dist - 1.55, yy * .4, hz - .5), .6, .6, gc, bays=3, t=.07)
            k.tube((dist - 1.6, yy * .4, hz - .2), (dist - 1.45, yy * .2, hz - 1.2), .07, RUBBER, n=6)
        k.box(-1.9, -1.9, gh, 3.8, 3.8, .14, GALV)
        k.box(-1.7, -1.3, gh + .14, 2.6, 2.6, 2.0, rgb(112, 116, 108, 'corr')); k.box(-1.75, -1.35, gh + 2.14, 2.7, 2.7, .1, RUST)
        k.box(.9, -.8, gh + .9, .03, 1.6, .8, GLASS); k.box(-1.72, -1.31, gh + 1.3, 2.64, 2.62, .45, TEAM)
        k.truss((-3.6, 0, gh + 2.3), (dist + 1.0, 0, gh + 2.3), 1.0, 1.0, gc, bays=8, t=.09)
        k.box(-3.7, -.7, gh + 1.5, 1.4, 1.4, 1.0, IRON)                              # counterweight
        for s in (-1, 1): k.strut((0, s * .5, gh + 5.4), (dist + .8, s * .5, gh + 3.3), .05, STEEL); k.strut((0, s * .5, gh + 5.4), (-3.4, s * .5, gh + 3.3), .05, STEEL)
        k.mast(0, 0, gh + 2.24, 3.2, 1.0, .3, gc, bays=2, t=.08); beacon(k, 0, 0, gh + 5.45)
        k.cbox(dist, 0, gh + 1.9, .9, .9, .4, DARK)
        k.tube((dist, 0, gh + 1.9), (dist, 0, 12.4 - ph), .03, DARK, n=4); k.cbox(dist, 0, 12.0 - ph, .35, .35, .4, YELLOW)
        k.ladder(-1.45, 0, 0, gh, axis='y')
    # Control bunker: battered concrete under an earth cover, blast door, periscope, vents.
    bx, by = 23.3, 23.6
    k.stack(bx, by, [(z, 7.6, 6.6, .4), (z + 2.6, 6.4, 5.4, .4), (z + 3.0, 5.6, 4.6, .6)], rgb(120, 116, 106, 'concrete'), smooth=False)
    k.stack(bx, by, [(z + 3.0, 5.0, 4.0, .8), (z + 3.3, 4.2, 3.2, .8)], rgb(92, 96, 66, 'foliage'), smooth=True)
    with face(k, 'y+', bx - 1.4, by + 3.3 + .9, z):
        k.box(-.5, -1.6, 0, 3.8, 1.6, 2.9, rgb(120, 116, 106, 'concrete')); k.box(-.6, -1.7, 2.9, 4.0, 1.8, .3, CONC_DARK)
        k.box(.5, -.05, 0, 1.8, .14, 2.3, rgb(84, 88, 84, 'dark')); k.box(.5, .09, 1.5, 1.8, .03, .35, TEAM)
        k.torus(1.4, .12, 1.05, .28, .04, STEEL, n=10, k=4, axis='y')
        for xx in (.3, 2.3): k.box(xx, .02, 0, .2, .04, 2.3, HAZ)
        k.box(1.2, .06, 2.45, .4, .12, .14, RED_LAMP)
    with face(k, 'x+', bx + 3.28, by + 1.6, z + 1.7): k.box(0, -.3, 0, 3.2, .5, .45, CONC_DARK); k.box(.25, .2, .1, 2.7, .03, .22, GLASS_DARK)
    for x, y in ((bx - 1.2, by - .6), (bx + 1.4, by + .5)): vent(k, x, y, z + 3.25, .28, .7, IRON)
    k.tube((bx + .2, by - .9, z + 3.25), (bx + .2, by - .9, z + 5.4), .07, DARK, n=6); k.cbox(bx + .2, by - .9, z + 5.4, .3, .22, .3, DARK)
    k.tube((bx - 2.0, by + .9, z + 3.0), (bx - 2.0, by + .9, z + 6.2), .03, STEEL, n=4)
    # Propellant farm in its bund: oxidiser and fuel tanks, transfer lines to the pad.
    k.box(22.2, 2.6, z, 5.4, 12.2, .12, CONCRETE)
    for bx2, by2, bw2, bd2 in ((22.2, 14.6, 5.4, .2), (27.4, 2.6, .2, 12.2), (22.2, 2.6, .2, 12.2), (22.2, 2.6, 5.4, .2)): k.box(bx2, by2, z, bw2, bd2, .75, CONC_DARK)
    tank(k, 24.9, 5.4, z + .12, 2.05, 6.4, rgb(196, 192, 180), roof='dome', band=rgb(176, 96, 44), stair=True)
    tank(k, 24.9, 11.2, z + .12, 2.05, 5.4, rgb(176, 166, 120), roof='dome', band=None, stair=True)
    cryo_tank(k, 22.2, 16.4, z, .8, 4.2)
    pipe_run(k, [(22.9, 5.4, z + 1.0), (21.6, 5.4, z + 1.0), (21.6, 9.0, z + 1.0), (21.0, 9.0, z + 1.0)], .16, rgb(176, 96, 44))
    pipe_run(k, [(22.9, 11.2, z + .7), (21.6, 11.2, z + .7), (21.6, 9.6, z + .7), (21.0, 9.6, z + .7)], .16, LAGGING)
    with face(k, 'y+', 23.2, 14.82, z + .9): sign(k, 0, 0, 1.2, .7); sign(k, 2.2, 0, .7, .7, SIGNAL_RED)
    # Lightning masts at the pad corners with catenary wires.
    lm = [(5.7, 4.6), (20.4, 4.4), (4.0, 20.6)]
    for x, y in lm: lightning_mast(k, x, y, z, 21.0)
    for (xa, ya), (xb, yb) in ((lm[0], lm[1]), (lm[0], lm[2])):
        pts = [(xa + (xb - xa) * i / 6, ya + (yb - ya) * i / 6, z + 21.0 - 2.2 * (1 - (2 * i / 6 - 1) ** 2)) for i in range(7)]
        for a, b in zip(pts, pts[1:]): k.strut(a, b, .03, STEEL)
    # Approach road, gate, double fence with a patrol strip, floodlights, stores, flag.
    apron(k, 9.8, 24.6, 6.4, 4.9, z); line(k, 12.9, 24.8, .15, 1.6, z); line(k, 12.9, 27.2, .15, 1.6, z)
    for fy, fx, hgt in ((29.4, 29.4, 2.4), (27.9, 27.9, 2.0)):
        fence(k, [(.7, fy, z), (9.6, fy, z)], h=hgt, c=IRON); fence(k, [(16.4, fy, z), (fx, fy, z), (fx, 16.0 if fx < 29 else .8, z)], h=hgt, c=IRON)
    for x in (9.6, 16.4): k.cbox(x, 29.4, z, .4, .4, 2.7, CONC_DARK); beacon(k, x, 29.4, z + 2.7, r=.13)
    k.tube((9.8, 28.6, z + 1.0), (16.2, 28.6, z + 1.2), .07, SIGNAL_RED, n=6); k.cbox(9.8, 28.6, z, .35, .35, 1.2, WHITE)
    for x in (3.0, 6.0):
        with face(k, 'y+', x, 29.46, z + 1.0): placard(k, 0, 0, .8)
    with face(k, 'y+', 20.0, 29.46, z + 1.0): sign(k, 0, 0, 1.2, .7)
    k.box(2.2, 23.4, z, 3.2, 2.6, 2.6, BRICK_DARK); k.box(2.1, 23.3, z + 2.6, 3.4, 2.8, .16, CONC_DARK)       # guardhouse
    with face(k, 'y+', 2.2, 26.0, z): door(k, .4, .9, c=rgb(84, 60, 50)); k.box(1.8, -.06, 1.1, 1.0, .1, 1.0, GLASS)
    with face(k, 'x+', 5.4, 26.0, z): k.box(.7, -.06, 1.1, 1.2, .1, 1.0, GLASS)
    genset(k, 3.4, 9.0, z, 3.8, rgb(96, 104, 84), yaw=90)
    crate(k, 6.6, 21.4, z, 1.2, c=OLIVE); crate(k, 8.0, 21.7, z, 1.0, c=OLIVE, yaw=10); barrels(k, 17.4, 22.2, z, 3, c=rgb(84, 92, 70))
    floodlight(k, 1.2, 28.0, z, 10.0, yaw=-30, heads=3); floodlight(k, 28.65, 21.0, z, 10.0, yaw=180, heads=3)
    floodlight(k, 21.6, 16.6, z, 9.0, yaw=200); floodlight(k, 20.4, 6.8, zp, 8.0, yaw=120)
    flagpole(k, 18.6, 25.4, z, 9.5, w=2.6)

def iron_curtain(k, w, d):
    # Soviet high-energy field generator: a pulsed-power machine hall of riveted plate on a
    # concrete base, its emitter sphere caged in girders on ceramic insulator columns, with bus
    # bars from the transformer yard and a bank of oil coolers.
    z = pad(k, w, d, c=CONC_DARK)
    plate_c = rgb(96, 92, 86, 'dark')
    hx, hy, hw, hd, hh = 2.0, 2.0, 11.4, 10.6, 6.4
    k.box(hx - .25, hy - .25, z, hw + .5, hd + .5, 2.4, SOV_CONC)
    k.box(hx, hy, z + 2.4, hw, hd, hh - 2.4, plate_c, top=ROOF)
    k.box(hx - .2, hy - .2, z + hh, hw + .4, hd + .4, .5, SOV_CONC, top=ROOF)
    for side, ox, oy, L in (('y+', hx, hy + hd, hw), ('x+', hx + hw, hy + hd, hd)):
        with face(k, side, ox, oy, z):
            x = 0.0
            while x <= L + .01:       # riveted lap seams and stiffening ribs
                k.box(x - .09, 0, 2.4, .18, .08, hh - 2.4, tint(plate_c, .78)); x += L / 7
            for zz in (3.7, 5.1): k.box(0, 0, zz - .06, L, .05, .12, tint(plate_c, .78))
            for i in range(7):
                for zz in (3.0, 4.4, 5.8):
                    for q in (.25, .75): k.box(L / 7 * (i + q) - .05, .0, zz, .1, .035, .1, tint(plate_c, 1.25))
            for i in (1, 3, 5): k.box(L / 7 * i + .35, -.05, 4.25, L / 7 - .7, .1, .55, GLASS_DARK)
            k.box(0, .01, hh - .62, L, .07, .5, TEAM)
    with face(k, 'y+', hx, hy + hd + .25, z):
        k.box(6.6, -.05, 0, 3.0, .16, 3.4, rgb(84, 80, 76, 'dark')); k.box(6.5, 0, 3.4, 3.2, .2, .25, SOV_CONC)
        k.box(8.06, .1, 0, .08, .05, 3.4, BLACK)
        for xx in (6.45, 9.6): k.box(xx, .0, 0, .15, .06, 3.4, HAZ)
        for xx in (6.9, 9.1): k.box(xx, .1, 3.75, .22, .14, .2, RED_LAMP)
        door(k, 1.4, 1.0, c=rgb(84, 60, 50)); placard(k, 3.2, 1.0, .8)
    # The emitter: sphere on four insulator columns, a girder cage, top terminal on insulators.
    ex, ey = hx + hw / 2 - .2, hy + hd / 2
    zr = z + hh + .5
    sr = 2.9; zs = zr + 3.3 + sr
    k.cyl(ex, ey, zr, 4.3, .35, SOV_CONC, n=8)
    for i in range(4):
        a = pi / 4 + i * pi / 2
        px, py = ex + 2.5 * cos(a), ey + 2.5 * sin(a)
        k.cyl(px, py, zr + .35, .62, .3, IRON, n=12)
        insulator(k, px, py, zr + .65, 2.5, .58, CERAMIC, n=6)
        k.cyl(px, py, zr + 3.15, .5, .25, IRON, n=12)
    k.torus(ex, ey, zr + 3.5, 2.5, .26, IRON, n=24, k=6)
    for i in range(4):
        a = pi / 4 + i * pi / 2
        k.strut((ex + 2.5 * cos(a), ey + 2.5 * sin(a), zr + 3.5), (ex + 2.5 * cos(a + pi / 2), ey + 2.5 * sin(a + pi / 2), zr + 3.5), .22, IRON)
    k.ball(ex, ey, zs, sr, C((98, 24, 22), 'paint'), n=28, k=14)
    for zz in (-.5, 0, .5):      # welded segment seams
        rr = sr * sqrt(1 - zz * zz) + .02
        k.rev((ex, ey, zs + sr * zz - .05), 'z', [(0, rr), (.1, rr)], C((70, 20, 20), 'paint'), n=28, caps=False)
    gird = rgb(70, 68, 64, 'dark'); R = sr + .5
    for i in range(8):
        a = i * pi / 4
        pts = [(ex + R * cos(t) * cos(a), ey + R * cos(t) * sin(a), zs + R * sin(t)) for t in [radians(q) for q in (-52, -30, -8, 14, 36, 58, 76)]]
        for p, q in zip(pts, pts[1:]): k.strut(p, q, .26, gird)
        k.strut(pts[-1], (ex, ey, zs + R + .25), .22, gird)
    for t in (-30, 14, 58):
        k.torus(ex, ey, zs + R * sin(radians(t)), R * cos(radians(t)), .1, gird, n=24, k=5)
    zc = zs + R + .15
    k.cyl(ex, ey, zc, .7, .35, IRON, n=12)
    insulator(k, ex, ey, zc + .35, 2.2, .5, CERAMIC, n=5)
    k.torus(ex, ey, zc + 2.75, 1.0, .2, COPPER, n=20, k=6); k.cyl(ex, ey, zc + 2.55, .3, .5, COPPER, n=10)
    for i in range(4): k.strut((ex, ey, zc + 2.75), (ex + cos(i * pi / 2), ey + sin(i * pi / 2), zc + 2.75), .08, COPPER)
    beacon(k, ex, ey, zc + 3.05, r=.2)
    for i in range(4):
        a = pi / 4 + i * pi / 2
        beacon(k, hx + hw / 2 + (hw / 2 - .1) * (1 if cos(a) > 0 else -1), hy + hd / 2 + (hd / 2 - .1) * (1 if sin(a) > 0 else -1), z + hh + .5, r=.15)
    k.ladder(hx + 1.2, hy + hd + .28, z, z + hh + .5, axis='x')
    zr2 = z + hh + .5
    k.box(hx + .8, hy + hd - 2.2, zr2, 1.6, 1.4, .9, IRON); k.box(hx + .75, hy + hd - 2.25, zr2 + .9, 1.7, 1.5, .08, RUST)      # roof access
    for x, y in ((hx + 1.2, hy + 1.4), (hx + hw - 1.3, hy + 1.2), (hx + hw - 1.2, hy + hd - 1.4)): vent(k, x, y, zr2, .34, .9, IRON)
    k.box(hx + 3.0, hy + hd - 1.3, zr2, 5.6, .5, .3, GALV); k.box(hx + hw - 1.0, hy + 3.0, zr2, .5, 5.0, .3, GALV)
    k.rail([(hx, hy + hd + .1, zr2), (hx + hw + .1, hy + hd + .1, zr2), (hx + hw + .1, hy, zr2)], IRON, h=1.0, t=.05)
    # Transformer yard and copper bus bars up to the emitter ring.
    apron(k, 2.2, 13.6, 9.6, 5.2, z, GRAVEL)
    for x in (4.2, 8.2):
        transformer(k, x, 15.8, z + .15, rgb(92, 100, 84), s=1.15)
        insulator(k, x, 13.1, z + hh + .5, 1.3, .2, CERAMIC, n=4)
        k.strut((x, 16.1, z + 3.45), (x, 13.1, z + hh + 1.85), .1, COPPER)
        k.strut((x, 13.1, z + hh + 1.85), (ex + (x - 6.2) * .5, ey + 2.3, zr + 3.6), .1, COPPER)
    fence(k, [(2.2, 18.9, z), (11.9, 18.9, z), (11.9, 13.3, z)], h=1.9, c=IRON)
    with face(k, 'y+', 6.2, 18.96, z + .8): sign(k, 0, 0, 1.1, .7)
    # Oil coolers along the east wall: finned radiator banks with fans, headers into the hall.
    for i in range(3):
        y0 = 2.6 + i * 3.3
        k.box(14.2, y0, z, 3.6, 2.7, .25, CONC_DARK)
        for x in (14.4, 17.5):
            for y in (y0 + .15, y0 + 2.45): k.box(x, y, z + .25, .12, .12, 3.2, IRON)
        k.box(14.35, y0 + .1, z + 1.0, 3.3, 2.5, 2.0, rgb(84, 88, 84, 'dark'))
        for j in range(14): k.box(14.45 + j * .23, y0 + 2.6, z + 1.05, .07, .06, 1.9, GALV)
        for j in range(11): k.box(17.65, y0 + .2 + j * .22, z + 1.05, .06, .07, 1.9, GALV)
        k.box(14.3, y0 + .05, z + 3.0, 3.4, 2.6, .4, IRON)
        for x in (15.15, 16.85):
            k.cyl(x, y0 + 1.35, z + 3.4, .72, .3, IRON, n=14); k.cyl(x, y0 + 1.35, z + 3.66, .64, .05, BLACK, n=14)
            for b in range(4):
                with k.at(x, y0 + 1.35, z + 3.72, yaw=b * 90 + i * 20): k.box(0, -.1, 0, .6, .2, .02, STEEL)
        k.pipe([(14.4, y0 + .7, z + 2.2), (13.4, y0 + .7, z + 2.2)], .16, RUST, n=8)
        k.pipe([(14.4, y0 + 2.0, z + 1.3), (13.4, y0 + 2.0, z + 1.3)], .16, RUST, n=8)
    k.pipe([(13.75, 2.9, z + 5.0), (13.75, 11.6, z + 5.0)], .2, RUST, n=8)
    # Yard.
    htank(k, 14.0, 14.4, z, 4.0, .8, rgb(104, 110, 92), axis='x')
    barrels(k, 13.6, 17.0, z, 3, c=rgb(84, 92, 70), rows=2); crate(k, 16.6, 16.8, z, 1.2, c=TIMBER)
    fence(k, [(13.0, 19.4, z), (19.4, 19.4, z), (19.4, 12.6, z)], h=1.9, c=IRON)
    flagpole(k, 16.6, 18.7, z, 8.5, w=2.2)

def psychic_sensor(k, w, d):
    # Soviet signals-intelligence listening station: a circular direction-finding array (a
    # compressed "Krug": a ring of monopoles round a wire reflector screen) with the receiver
    # building at its centre and, on the roof mast, the sensor head; dishes, generator shed.
    z = pad(k, w, d, c=CONC_DARK)
    ax, ay = 10.4, 10.6
    Ro, Ri, ho, hi = 8.0, 5.7, 5.6, 8.8
    k.cyl(ax, ay, z, Ro + .45, .05, GRAVEL, n=32)
    # Receiver building at the centre of the array.
    bw, bd, bh = 7.2, 5.6, 6.6
    bx, by = ax - bw / 2, ay - bd / 2
    top = block(k, bx, by, bw, bd, bh, BRICK, roof=ROOF, plinth=CONC_DARK, floors=2, glaze='punched', band=TEAM)
    k.box(bx - .15, by - .15, z + bh, bw + .3, bd + .3, .16, CONC_DARK)
    with face(k, 'y+', bx, by + bd, z): door(k, 4.9, 1.0, c=rgb(84, 60, 50)); k.box(4.6, 0, 2.5, 1.6, .9, .12, CONC_DARK)
    dish(k, bx + 5.6, by + 3.6, top, 1.2, yaw=70, pitch=38, c=rgb(170, 170, 160)); dish(k, bx + 5.9, by + 1.2, top, .8, yaw=10, pitch=30, c=rgb(170, 170, 160))
    for x, y in ((bx + 1.0, by + 4.6), (bx + 3.6, by + 4.4)): vent(k, x, y, top, .26, .8)
    k.box(bx + 3.2, by + .7, top, 1.8, 1.3, 1.0, CLAD_RUST); k.box(bx + 3.1, by + .6, top + 1.0, 2.0, 1.5, .1, RUST)
    # The array: outer ring of monopoles on base insulators, inner reflector screen of poles and wires.
    for i in range(20):
        a = i * 2 * pi / 20 + .1
        px, py = ax + Ro * cos(a), ay + Ro * sin(a)
        k.cyl(px, py, z, .26, .3, CONCRETE, n=8); k.cyl(px, py, z + .3, .13, .4, CERAMIC, n=8)
        k.tube((px, py, z + .7), (px, py, z + ho), .09, GALV, n=6, r2=.04)
        k.cyl(px, py, z + ho * .6, .14, .12, DARK, n=6)
    n = 16
    poles = ring(ax, ay, z, Ri, n, a0=pi / n)
    for (px, py, _) in poles:
        k.cyl(px, py, z, .28, .25, CONCRETE, n=8)
        k.tube((px, py, z + .25), (px, py, z + hi), .13, rgb(92, 78, 62, 'wood'), n=6, r2=.09)
    for j in range(8):
        zz = z + 1.6 + j * 1.0
        for i in range(n): k.strut(poles[i][:2] + (zz,), poles[(i + 1) % n][:2] + (zz,), .06 if j in (0, 7) else .035, STEEL)
    for i in range(n):
        for q in (1 / 3, 2 / 3):
            px = poles[i][0] + (poles[(i + 1) % n][0] - poles[i][0]) * q; py = poles[i][1] + (poles[(i + 1) % n][1] - poles[i][1]) * q
            k.strut((px, py, z + 1.6), (px, py, z + 8.6), .028, STEEL)
    for bx2, by2, bw2, bd2 in ((ax - .35, by + bd, .7, Ro - bd / 2), (bx + bw, ay - .35, Ro - bw / 2, .7)): cable_duct(k, bx2, by2, bw2, bd2, z)
    # Roof mast and the sensor head.
    mx, my, mh = ax - 1.6, ay - .2, 10.5
    k.cbox(mx, my, top, 2.0, 2.0, .25, CONC_DARK)
    k.mast(mx, my, top + .25, mh, 1.5, .6, SOV_STEEL, bays=6, t=.1)
    for gx, gy in ((bx + .2, by + .2), (bx + bw - .2, by + .2), (bx + .2, by + bd - .2), (bx + bw - .2, by + bd - .2)):
        k.strut((mx, my, top + mh * .7), (gx, gy, top + .1), .03, STEEL)
    zh = top + .25 + mh
    k.cyl(mx, my, zh, .7, .7, SOV_STEEL, n=12); k.cyl(mx, my, zh + .7, .26, 1.1, IRON, n=8)
    for i in range(8):       # turnstile of pick-up probes
        a = i * pi / 4
        k.strut((mx, my, zh + .4), (mx + 2.2 * cos(a), my + 2.2 * sin(a), zh + .4), .07, STEEL)
        k.rev((mx + 2.2 * cos(a), my + 2.2 * sin(a), zh + .12), 'z', [(0, .13), (.56, .13)], DARK, n=6)
    hc = zh + 3.3; ring_c = rgb(70, 68, 64, 'dark')
    k.torus(mx, my, hc, 1.6, .09, ring_c, n=24, k=5, axis='z')
    k.torus(mx, my, hc, 1.4, .08, COPPER, n=24, k=5, axis='x')
    k.torus(mx, my, hc, 1.2, .08, COPPER, n=24, k=5, axis='y')
    for s in (-1, 1):
        k.strut((mx, my, hc + s * .75), (mx, my, hc + s * 1.6), .08, IRON)
        k.strut((mx + s * .5, my, hc), (mx + s * 1.6, my, hc), .05, IRON); k.strut((mx, my + s * .5, hc), (mx, my + s * 1.6, hc), .05, IRON)
        k.rev((mx, my, hc + s * .82 - .1), 'z', [(0, .2), (.2, .2)], IRON, n=8)
    k.rev((mx, my, hc - .8), 'z', [(0, .03), (.5, .42), (.8, .5), (1.1, .42), (1.6, .03)], C((52, 40, 78), 'glass'), n=6, smooth=False)
    k.rev((mx, my, hc - .07), 'z', [(0, .52), (.14, .52)], PSI, n=6, smooth=False)
    k.tube((mx, my, hc + 1.6), (mx, my, hc + 3.0), .03, STEEL, n=4); beacon(k, mx, my, hc + 1.62, r=.11)
    k.ladder(mx, my + .3, top + .25, zh, axis='x')
    # Generator shed in the east corner, stores and cable drums in the west one.
    gx, gy = 16.3, .9
    k.box(gx, gy, z, 3.1, 3.0, 2.6, CLAD_GREEN); k.box(gx - .05, gy - .05, z, 3.2, 3.1, .45, CONC_DARK)
    gable(k, gx, gy, z + 2.6, 3.1, 3.0, .8, CLAD_RUST, ridge='x', wall=CLAD_GREEN)
    with face(k, 'y+', gx, gy + 3.0, z):
        door(k, .3, .9, 2.0, c=rgb(84, 60, 50))
        for i in range(5): k.box(1.6, 0, 1.0 + i * .22, 1.2, .06, .1, DARK)
    k.pipe([(gx + 2.7, gy + 3.0, z + 2.0), (gx + 2.7, gy + 3.4, z + 2.0), (gx + 2.7, gy + 3.4, z + 4.6)], .1, RUST, n=8)
    cable_drum(k, 2.4, 17.2, z, 1.0, .9, yaw=20); cable_drum(k, 4.2, 18.6, z, .8, .7, yaw=75)
    crate(k, 1.2, 15.4, z, 1.0, c=TIMBER)
    barrels(k, 17.6, 17.9, z, 3, c=rgb(84, 92, 70)); crate(k, 18.2, 16.4, z, 1.0, c=TIMBER, yaw=14)
    with face(k, 'y+', 15.6, 19.4, z):
        for xx in (0, 1.4): k.strut((xx, 0, 0), (xx, 0, 1.9), .07, IRON)
        sign(k, .05, 1.0, 1.3, .8)
    flagpole(k, 1.4, 18.9, z, 8.0, w=2.2)
    floodlight(k, 19.2, 14.6, z, 7.0, yaw=190)

# --- Allied ------------------------------------------------------------------------------------------
def electrode(k, h, r=.3, ball=.4, c=CERAMIC_PALE):
    """Post insulator with a polished terminal sphere, standing at the local origin."""
    k.cyl(0, 0, 0, r * 1.2, .2, GALV, n=10)
    insulator(k, 0, 0, .2, h, r, c, n=max(4, int(h / .45)))
    k.cyl(0, 0, h + .2, r * .5, .25, STEEL, n=8); k.ball(0, 0, h + .45 + ball * .8, ball, CHROME, n=12, k=6)

def dipole_mast(k, x, y, z, h=7.4, arm=1.6):
    """One element of a phased HF array: crossed dipoles for two bands on a guyed pole."""
    k.cbox(x, y, z, .55, .55, .2, CONC_DARK)
    k.tube((x, y, z), (x, y, z + h), .12, GALV, n=6)
    for zz, L, a0 in ((h - .15, arm, 0), (h - 1.7, arm * .62, 45)):
        with k.at(x, y, z + zz, yaw=a0):
            for i in range(4): k.strut((0, 0, 0), (L * cos(i * pi / 2), L * sin(i * pi / 2), -.2), .1, rgb(196, 198, 196, 'steel'))
            k.cbox(0, 0, -.14, .32, .32, .28, OFFWHITE)
    for i in range(4):
        a = pi / 4 + i * pi / 2
        k.strut((x, y, z + h * .6), (x + 1.5 * cos(a), y + 1.5 * sin(a), z + .1), .025, STEEL)

def weather_control(k, w, d):
    # Allied ionospheric research station (a HAARP-like heater): a phased field of crossed-dipole
    # masts over a ground screen, a central emitter tower ringed with skyward electrodes, an
    # instrument radome on the diagnostics building, control block, capacitor banks, cable ducts.
    z = pad(k, w, d, joints=6.0)
    # Diagnostics building under the instrument radome (back left).
    top = block(k, 2.0, 2.0, 9.4, 9.4, 4.2, CLAD_WHITE, roof=ROOF, plinth=PRECAST, floors=1, glaze='strip', band=TEAM)
    with face(k, 'x+', 11.4, 11.4, z): door(k, 6.6, 1.0)
    geodesic(k, 6.7, 6.7, top + .9, 4.5, WHITE, freq=3, lo=-.35, seed=5, skirt=.9)
    ac_unit(k, 2.6, 9.6, top, 1.6, 1.0, .8)
    # Central emitter tower.
    ex, ey, eh = 17.6, 8.4, 14.0
    for sx in (-1, 1):
        for sy in (-1, 1): k.cbox(ex + sx * 2.0, ey + sy * 2.0, z, 1.3, 1.3, .6, CONCRETE)
    k.mast(ex, ey, z + .5, eh, 4.0, 2.6, GALV, bays=5, t=.16)
    zt = z + .5 + eh
    k.rev((ex, ey, zt), 'z', [(0, 3.3), (.18, 3.3)], GALV, n=8)
    for i in range(8):
        a = i * pi / 4 + pi / 8
        k.strut((ex + 1.3 * cos(a), ey + 1.3 * sin(a), zt - 2.2), (ex + 3.1 * cos(a), ey + 3.1 * sin(a), zt), .09, GALV)
    k.rail(ring(ex, ey, zt + .18, 3.2, 8) + ring(ex, ey, zt + .18, 3.2, 8)[:1], GALV, h=1.0, t=.04, posts=9)
    for i in range(8):
        a = i * pi / 4
        with k.at(ex + 2.35 * cos(a), ey + 2.35 * sin(a), zt + .18, yaw=degrees(a), pitch=-16): electrode(k, 2.3, .26, .36)
    k.cyl(ex, ey, zt + .18, 1.0, .9, rgb(176, 180, 176), n=12)
    insulator(k, ex, ey, zt + 1.08, 4.0, .46, CERAMIC_PALE, n=9)
    k.torus(ex, ey, zt + 5.35, 1.15, .32, CHROME, n=24, k=8); k.cyl(ex, ey, zt + 5.08, .32, .6, STEEL, n=10)
    for i in range(4): k.strut((ex, ey, zt + 5.35), (ex + 1.1 * cos(i * pi / 2), ey + 1.1 * sin(i * pi / 2), zt + 5.35), .09, STEEL)
    k.tube((ex, ey, zt + 5.6), (ex, ey, zt + 7.6), .09, STEEL, n=6, r2=.03); k.ball(ex, ey, zt + 7.7, .2, PLASMA, n=8, k=4)
    k.ladder(ex, ey + .4, z + .5, zt, axis='x')
    for f in (.4, .8): k.cbox(ex, ey, z + .5 + eh * f, 1.8, 1.8, .08, GALV)
    k.strut((ex + 1.9, ey - 1.9, z + .6), (ex + 1.25, ey - 1.25, zt), .1, COPPER)
    for i in range(4): beacon(k, ex + 3.2 * cos(i * pi / 2 + pi / 8), ey + 3.2 * sin(i * pi / 2 + pi / 8), zt + 1.2, r=.12)
    # Capacitor banks and the supply transformer in a fenced compound (back right).
    apron(k, 21.6, 1.6, 7.4, 10.4, z, GRAVEL)
    for i in range(3): capacitor_bank(k, 22.4, 2.4 + i * 2.5, z, n=6, tiers=2)
    capacitor_bank(k, 26.4, 2.4, z, n=4, tiers=2, yaw=90)
    transformer(k, 24.0, 10.4, z + .15, s=1.0)
    k.strut((22.4, 3.1, z + 3.8), (22.4, 8.2, z + 3.8), .06, COPPER)
    fence(k, [(21.6, 12.1, z), (29.1, 12.1, z)], h=2.0)
    with face(k, 'y+', 25.6, 12.16, z + .8): sign(k, 0, 0, 1.1, .7)
    cable_duct(k, 19.6, 5.6, 2.2, .8, z); cable_duct(k, 12.0, 12.6, 17.0, .8, z); cable_duct(k, 19.9, 10.4, .8, 2.2, z)
    # Control building (front left).
    top2 = block(k, 2.0, 15.4, 9.2, 7.2, 6.8, CLAD_WHITE, roof=ROOF, plinth=PRECAST, floors=2, glaze='strip', band=TEAM)
    roof_plant(k, 2.4, 15.8, top2, 8.4, 6.4, seed=6, density=1.2)
    dish(k, 9.6, 16.8, top2, 1.0, yaw=60, pitch=40)
    with face(k, 'x+', 11.2, 22.6, z):
        door(k, 2.6, 1.3, c=TEAM); k.box(2.0, 0, 2.5, 2.5, 1.4, .14, GALV)
        for xx in (2.15, 4.35): k.strut((xx, 1.3, 0), (xx, 1.3, 2.5), .08, GALV)
    # The antenna field: 4 x 4 crossed dipoles over a ground screen.
    fx0, fy0, sp = 14.8, 15.2, 4.1
    apron(k, fx0 - 1.8, fy0 - 1.8, 3 * sp + 3.6, 3 * sp + 3.6, z, GRAVEL)
    for i in range(4):
        for j in range(4): dipole_mast(k, fx0 + i * sp, fy0 + j * sp, z, 7.6, 1.55)
    gs = z + 3.2
    for i in range(13):
        q = fx0 - 1.2 + i * (3 * sp + 2.4) / 12
        k.strut((q, fy0 - 1.2, gs), (q, fy0 + 3 * sp + 1.2, gs), .035 if i % 4 else .06, STEEL)
        q2 = fy0 - 1.2 + i * (3 * sp + 2.4) / 12
        k.strut((fx0 - 1.2, q2, gs), (fx0 + 3 * sp + 1.2, q2, gs), .035 if i % 4 else .06, STEEL)
    for i in range(4):
        cable_duct(k, fx0 + i * sp + .5, fy0 - 2.6, .5, 3 * sp + 3.4, z, tint(PRECAST, .92))
    # Met enclosure: instrument screen, rain gauge, anemometer mast.
    k.box(3.0, 25.0, z, 5.4, 3.6, .04, rgb(104, 122, 74, 'foliage'))
    for x, y in ((3.9, 26.0), (4.9, 26.0), (3.9, 26.8), (4.9, 26.8)): k.strut((x, y, z), (x, y, z + 1.2), .06, WHITE)
    k.box(3.8, 25.9, z + 1.2, 1.2, 1.0, .8, WHITE); k.extrude([(25.8, 2.0), (26.4, 2.3), (27.0, 2.0)], 'yz', 3.7, 5.1, WHITE)
    k.cyl(6.6, 26.2, z, .18, .9, GALV, n=8); k.tube((7.6, 27.6, z), (7.6, 27.6, z + 6.5), .05, GALV, n=6)
    with k.at(7.6, 27.6, z + 6.5):
        for i in range(3): k.strut((0, 0, 0), (.5 * cos(i * 2.1), .5 * sin(i * 2.1), 0), .03, STEEL); k.ball(.5 * cos(i * 2.1), .5 * sin(i * 2.1), 0, .09, DARK, n=6, k=3)
        k.plate([(0, 0, .3), (-.7, 0, .25), (-.7, 0, .6)], SIGNAL_RED)
    # Perimeter, lights, flag.
    fence(k, [(.8, 29.3, z), (12.2, 29.3, z)], h=2.0)
    for x in (12.2, 13.4): k.cbox(x, 29.3, z, .3, .3, 2.3, GALV)
    floodlight(k, 12.6, 13.6, z, 9.0, yaw=45); floodlight(k, 29.0, 13.4, z, 8.0, yaw=120)
    bollards(k, [(12.2, 23.6), (12.2, 25.0), (12.2, 26.4)], z)
    flagpole(k, 12.0, 28.2, z, 9.0, w=2.4)

def chronosphere(k, w, d):
    # Allied particle-physics installation: a storage ring (beam pipe through dipole and quadrupole
    # magnets on girders, RF cavity, cryogenic transfer line) feeding a spherical target chamber
    # held in a steel cradle; control room, helium dewars and cold box, magnet power supplies.
    z = pad(k, w, d)
    cx, cy, R, zb = 11.8, 11.8, 5.7, 2.5
    pipe_c = C((176, 182, 188), 'steel')
    # Control room (left).
    top = block(k, 1.2, 3.4, 4.3, 9.0, 4.2, CLAD_WHITE, roof=ROOF, plinth=PRECAST, floors=1, glaze='strip', band=TEAM)
    roof_plant(k, 1.5, 3.8, top, 3.6, 8.2, seed=3, density=1.6, skylights=False)
    with face(k, 'y+', 1.2, 12.4, z): door(k, 1.6, 1.0)
    # The ring.
    dip = rgb(196, 104, 44); quad = rgb(206, 178, 62)
    k.torus(cx, cy, z + zb, R, .2, pipe_c, n=48, k=8)
    k.torus(cx, cy, z + zb + 1.0, R + .05, .11, FROST, n=48, k=6)
    k.torus(cx, cy, z + .2, R + 1.3, .06, YELLOW, n=48, k=4)
    for i in range(16):
        a = i * 2 * pi / 16 + pi / 16
        with k.at(cx + R * cos(a), cy + R * sin(a), z, yaw=degrees(a) + 90):
            k.cbox(0, 0, 0, 1.5, .8, .3, CONC_DARK)
            for s in (-1, 1): k.box(s * .55 - .08, -.3, .3, .16, .6, zb - .85, GALV)
            k.box(-.85, -.4, zb - .56, 1.7, .8, .14, GALV)
            if i == 5:      # RF cavity
                k.rev((-.7, 0, zb), 'x', [(0, .3), (.1, .62), (1.3, .62), (1.4, .3)], COPPER, n=14)
                k.cyl(0, 0, zb + .6, .16, .7, COPPER, n=8); k.box(-.3, -.25, zb + 1.3, .6, .5, .3, GALV)
            elif i % 2 == 0:   # dipole: yoke with coil ends
                k.cbox(0, 0, zb - .42, 1.5, .78, .84, dip)
                for s in (-1, 1): k.box(s * .75 - .1, -.3, zb - .2, .2, .6, .4, COPPER)
                k.strut((0, 0, zb + .42), (0, 0, zb + 1.0), .05, STEEL)
            else:              # quadrupole and a corrector
                k.rev((-.45, 0, zb), 'x', [(0, .52), (.5, .52)], quad, n=8)
                k.rev((.2, 0, zb), 'x', [(0, .34), (.3, .34)], rgb(84, 118, 92), n=8)
                k.box(-.3, -.5, zb - .42, .2, 1.0, .1, GALV)
    # Target chamber in its cradle, with a service gallery at the equator.
    sr = 2.75; zs = z + 6.9
    ch = C((168, 176, 184), 'steel')
    k.cyl(cx, cy, z, 4.0, .3, CONC_DARK, n=6)
    cols = ring(cx, cy, z + .3, 3.45, 6, a0=pi / 6)
    for i, (px, py, pz) in enumerate(cols):
        k.box(px - .17, py - .17, pz, .34, .34, zs - .3 - pz, GALV)
        nx, ny, _ = cols[(i + 1) % 6]
        for zz in (zs - 2.3, zs - .45): k.strut((px, py, zz), (nx, ny, zz), .2, GALV)
        k.strut((px, py, pz + .2), (nx, ny, zs - 2.3), .09, GALV); k.strut((nx, ny, pz + .2), (px, py, zs - 2.3), .09, GALV)
        k.strut((px, py, zs - 2.3), (cx + (px - cx) * .5, cy + (py - cy) * .5, zs - 2.3), .18, GALV)
    k.torus(cx, cy, zs - 2.3, 1.75, .16, GALV, n=20, k=6)
    k.rev((cx, cy, zs - .4), 'z', [(0, 4.1), (.1, 4.1)], GALV, n=6)
    gal = ring(cx, cy, zs - .3, 4.0, 6)
    k.rail(gal + gal[:1], GALV, h=1.0, t=.04, posts=9)
    k.ball(cx, cy, zs, sr, ch, n=28, k=14)
    k.rev((cx, cy, zs - .12), 'z', [(0, sr + .05), (.24, sr + .05)], tint(ch, .8), n=28, caps=False)
    for t in (-45, 45):
        rr = sr * cos(radians(t)) + .03
        k.rev((cx, cy, zs + sr * sin(radians(t)) - .05), 'z', [(0, rr), (.1, rr)], tint(ch, .84), n=28, caps=False)
    for i in range(20):       # diagnostic ports; some are lit viewports
        a = i * 2.39996 + .3; t = radians((12, 34, 56, 22, 44)[i % 5])
        dx, dy, dz = cos(t) * cos(a), cos(t) * sin(a), sin(t)
        pr = (.36, .24, .3)[i % 3]
        k.rev((cx + dx * sr * .96, cy + dy * sr * .96, zs + dz * sr * .96), (dx, dy, dz), [(0, pr), (.32, pr), (.32, pr * 1.3), (.4, pr * 1.3)], tint(ch, .8), n=10)
        if i % 3 == 1: k.rev((cx + dx * (sr * .96 + .405), cy + dy * (sr * .96 + .405), zs + dz * (sr * .96 + .405)), (dx, dy, dz), [(0, pr * .9), (.02, pr * .9)], PLASMA, n=10)
    k.cyl(cx, cy, zs + sr - .1, .55, .7, tint(ch, .8), n=12); k.cyl(cx, cy, zs + sr + .6, .75, .12, DARK, n=12)
    k.box(cx - .2, cy - .2, zs + sr + .72, .4, .4, .5, GALV); beacon(k, cx, cy, zs + sr + 1.22, r=.14)
    k.rev((cx, cy, zs - 2.3), 'z', [(0, 1.6), (-.8, 1.1), (-.9, .45), (-2.2, .45)], tint(ch, .82), n=12)       # pumping stack
    k.cyl(cx, cy, z + .3, 1.0, 2.1, rgb(84, 118, 92), n=12)
    k.ladder(cols[0][0] + .25, cols[0][1] + .05, z + .3, zs - .3, axis='y')
    # Transfer lines from the ring up into the chamber.
    for a in (radians(20), radians(140), radians(260)):
        p0 = (cx + R * cos(a), cy + R * sin(a), z + zb); p1 = (cx + 4.4 * cos(a), cy + 4.4 * sin(a), z + zb + .1)
        p2 = (cx + 2.2 * cos(a), cy + 2.2 * sin(a), zs - sr * .78)
        k.pipe([p0, p1, p2], .15, pipe_c, n=8)
        mx, my, mz = [(p1[q] + p2[q]) / 2 for q in range(3)]
        k.rev((mx, my, mz), (p2[0] - p1[0], p2[1] - p1[1], p2[2] - p1[2]), [(-.35, .42), (.35, .42)], quad, n=8)
    # Helium plant (back right): dewars, cold box, transfer line to the ring.
    cryo_tank(k, 15.2, 2.7, z, 1.0, 6.0); cryo_tank(k, 17.8, 2.7, z, 1.0, 6.0, band=TEAM)
    k.box(17.9, 4.6, z, 1.5, 1.5, 3.6, WHITE); k.box(17.85, 4.55, z + 3.6, 1.6, 1.6, .1, GALV)
    for i in range(4): k.box(18.1 + i * .3, 6.1, z + .5, .06, .04, 2.6, GALV)
    k.pipe([(16.5, 3.9, z + 4.4), (16.5, 5.4, z + 4.4), (16.5, 5.4, z + zb + 1.0), (16.5, 8.4, z + zb + 1.0)], .11, FROST, n=6)
    k.pipe([(15.2, 3.8, z + 3.0), (17.8, 3.8, z + 3.0)], .08, FROST, n=6)
    # Magnet power supplies (front left) with a cable tray to the ring.
    cabinets(k, 2.0, 19.2, z, 5, side='x+')
    transformer(k, 3.4, 14.1, z + .15, s=.85)
    cable_tray(k, [(2.3, 17.2, z + 2.9), (9.4, 17.2, z + 2.9)], z)
    # Crossover stile, gas bottles, markings, flag.
    with k.at(cx, cy, z, yaw=45):
        k.stairs(R + 2.0, -.45, 0, zb + .9, -1.6, .9, axis='x'); k.box(R - .4, -.45, zb + .82, .8, .9, .08, GALV)
        for yy in (-.45, .45): k.rail([(R - .4, yy, zb + .9), (R + .4, yy, zb + .9)], GALV)
    for i in range(4): k.cyl(17.9 + i * .34, 18.9, z, .14, 1.5, rgb(120, 96, 60), n=8); k.cyl(17.9 + i * .34, 18.9, z + 1.5, .05, .15, BRASS, n=5)
    k.box(17.65, 18.65, z, 1.5, .06, 1.0, GALV)
    bollards(k, [(19.0, 15.0), (15.0, 19.0), (19.0, 11.8), (11.8, 19.0)], z)
    flagpole(k, 6.4, 18.9, z, 8.0, w=2.2)
    floodlight(k, 19.2, 8.6, z, 7.0, yaw=170)

def camo_net(k, x0, y0, x1, y1, zf, seed=2, n=12):
    """Camouflage net draped over poles: a sagging sheet garnished in drab patches, with holes."""
    rnd = random.Random(seed)
    cols = [rgb(78, 88, 60, 'foliage'), rgb(92, 96, 68, 'cloth'), rgb(66, 76, 54, 'foliage'), rgb(104, 98, 74, 'cloth'), rgb(84, 90, 62, 'cloth')]
    P = [[(x0 + (x1 - x0) * i / n + rnd.uniform(-.06, .06), y0 + (y1 - y0) * j / n + rnd.uniform(-.06, .06), zf(i / n, j / n) + rnd.uniform(-.05, .05))
          for j in range(n + 1)] for i in range(n + 1)]
    patch = [[rnd.randrange(5) for j in range(n // 2 + 1)] for i in range(n // 2 + 1)]
    with k.grp(True):
        for i in range(n):
            for j in range(n):
                if rnd.random() < .07: continue
                c = cols[patch[i // 2][j // 2] if rnd.random() < .7 else rnd.randrange(5)]
                k.poly([P[i][j], P[i + 1][j], P[i + 1][j + 1], P[i][j + 1]], c)

def gap_generator(k, w, d):
    # Allied electronic-warfare jamming station: a hardened equipment shelter, a guyed telescopic
    # mast stacked with log-periodic and panel jammer antennas under a rotating emitter head,
    # generator set, cable drums, a camouflage net over the stores.
    z = pad(k, w, d)
    # Hardened shelter: thick walls, overhanging burster slab, blast door, filtered intake.
    sx, sy, sw, sd, sh = 2.0, 10.6, 8.6, 6.8, 3.7
    k.box(sx, sy, z, sw, sd, sh, CONCRETE); k.box(sx - .1, sy - .1, z, sw + .2, sd + .2, .5, CONC_DARK)
    k.box(sx - .35, sy - .35, z + sh, sw + .7, sd + .7, .55, CONC_DARK, top=ROOF)
    for x in (sx + 2.6, sx + 5.6): k.box(x, sy + sd, z, .5, .3, sh, CONC_LIGHT)
    with face(k, 'y+', sx, sy + sd, z):
        k.box(3.35, -.04, 0, 1.6, .16, 2.4, rgb(96, 104, 110)); k.box(3.35, .12, 1.55, 1.6, .03, .4, TEAM)
        k.torus(4.15, .14, 1.0, .25, .04, STEEL, n=10, k=4, axis='y'); k.box(3.2, .0, 2.4, 1.9, .2, .2, CONC_DARK)
        k.box(3.9, .1, 2.75, .5, .14, .12, LAMP)
        for j in range(5): k.box(6.6, 0, 1.2 + j * .25, 1.5, .06, .12, DARK)
        sign(k, .9, 1.3, .9, .6)
    with face(k, 'x+', sx + sw, sy + sd, z):
        for xx in (1.0, 4.2):
            k.box(xx, -.02, 1.0, 1.7, .1, 1.6, GALV)
            for j in range(6): k.box(xx + .08, .08, 1.1 + j * .25, 1.54, .04, .12, DARK)
    zr = z + sh + .55
    ac_unit(k, sx + .8, sy + .9, zr, 1.8, 1.1, .9); ac_unit(k, sx + 3.2, sy + .9, zr, 1.8, 1.1, .9)
    k.box(sx + 6.0, sy + 3.6, zr, 1.4, 1.4, .8, GALV); vent(k, sx + 1.4, sy + 4.6, zr, .3, .7)
    for x, y, hh in ((sx + 7.8, sy + .8, 3.6), (sx + .5, sy + 6.0, 2.6)): k.tube((x, y, zr), (x, y, zr + hh), .03, DARK, n=4)
    k.rail([(sx - .2, sy + sd + .2, zr), (sx + sw + .2, sy + sd + .2, zr), (sx + sw + .2, sy - .2, zr)], GALV, h=1.0)
    # The mast.
    mx, my = 8.4, 5.6
    mc = rgb(120, 128, 118)
    k.cbox(mx, my, z, 2.8, 2.8, .5, CONCRETE)
    for i in range(4):
        a = pi / 4 + i * pi / 2
        k.strut((mx + .3 * cos(a), my + .3 * sin(a), z + 2.2), (mx + 1.7 * cos(a), my + 1.7 * sin(a), z + .5), .12, mc)
    k.box(mx - 1.2, my - .55, z + .5, .8, 1.1, 1.3, rgb(96, 108, 92)); k.box(mx - 1.22, my - .3, z + 1.3, .03, .6, .4, DARK)   # winch
    rs = (.52, .44, .36, .29, .22, .16); zz = z + .5
    for i, r in enumerate(rs):
        L = 3.3 if i < 5 else 2.4
        k.cyl(mx, my, zz, r, L + .3, mc, n=12)
        k.rev((mx, my, zz + L - .05), 'z', [(0, r + .07), (.35, r + .07)], DARK, n=12)
        zz += L
    ztop = zz
    def lpda(zh, yaw, L=3.4, big=1.5, c=GALV):
        with k.at(mx, my, zh, yaw=yaw):
            k.strut((.2, 0, 0), (L + .4, 0, 0), .12, c); k.strut((.2, 0, -.9), (1.6, 0, 0), .06, c)
            for j in range(11):
                q = j / 10; e = big * (1 - .8 * q)
                k.strut((.5 + L * q * (1.0 - .25 * q), -e, 0), (.5 + L * q * (1.0 - .25 * q), e, 0), .07, c)
    lpda(z + 6.2, 20); lpda(z + 6.2, 200); lpda(z + 9.6, 110, 2.8, 1.2); lpda(z + 9.6, 290, 2.8, 1.2); lpda(z + 12.6, 65, 2.2, .9); lpda(z + 12.6, 245, 2.2, .9)
    for tier, zh in enumerate((z + 14.4, z + 16.4)):       # sector panel jammers
        for i in range(4):
            with k.at(mx, my, zh, yaw=45 + i * 90 + tier * 45):
                k.box(.55, -.3, 0, .16, .6, 1.6, OFFWHITE); k.box(.5, -.25, .1, .05, .5, 1.4, GALV)
                k.strut((.1, 0, .3), (.52, 0, .3), .05, GALV); k.strut((.1, 0, 1.3), (.52, 0, 1.3), .05, GALV)
    with k.at(mx, my, ztop + .3, yaw=25):                      # rotating emitter head
        k.cyl(0, 0, 0, .5, .45, DARK, n=12)
        k.cyl(0, 0, .45, 1.0, .8, rgb(96, 108, 92), n=12)
        for i in range(8):
            a = i * pi / 4
            k.rev((.95 * cos(a), .95 * sin(a), .85), (cos(a), sin(a), 0), [(0, .14), (.7, .34)], DARK, n=4)
        k.rev((0, 0, 1.25), 'z', [(0, 1.02), (.22, 1.02)], TEAM, n=12)
        k.dome(0, 0, 1.47, .8, OFFWHITE, h=.7, n=12, k=4)
        beacon(k, 0, 0, 2.15, r=.12)
    anchors = [(3.0, 1.6), (14.4, 1.4), (15.4, 10.4), (1.6, 8.8)]
    for axx, ayy in anchors:
        k.cbox(axx, ayy, z, .7, .7, .3, CONC_DARK)
        for zh in (z + 7.0, z + 13.4, z + 17.4): k.strut((mx, my, zh), (axx, ayy, z + .3), .03, STEEL)
    # Generator set, fuel tank, cable tray into the shelter.
    genset(k, 12.8, 3.4, z, 4.6, stack=2.2)
    htank(k, 13.2, 7.4, z, 3.6, .65, rgb(96, 108, 92), axis='x')
    cable_tray(k, [(12.8, 4.4, z + 2.0), (10.6, 4.4, z + 2.0)], z)
    cable_duct(k, 9.6, 6.6, .7, 4.0, z)
    # Stores under a camouflage net slung from the shelter.
    crate(k, 12.0, 11.6, z, 1.3, c=OLIVE); crate(k, 13.6, 11.8, z, 1.1, c=OLIVE, yaw=12); crate(k, 12.2, 11.8, z + 1.05, 1.0, c=OLIVE, yaw=20)
    barrels(k, 12.0, 13.8, z, 3, c=OLIVE, rows=2)
    nz = lambda u, v: (3.7 - .95 * u - .32 * sin(pi * v) * sin(pi * min(1.0, u / .86)) - .9 * max(0, (u - .86) / .14) ** 1.5
                       - .8 * max(0, (v - .88) / .12) ** 1.5 - .6 * max(0, (.1 - v) / .1) ** 1.5)
    camo_net(k, 10.9, 10.5, 17.0, 16.3, nz, seed=4, n=12)
    for x, y in ((16.15, 11.05), (16.15, 13.4), (16.15, 15.6), (13.2, 15.6)):
        k.tube((x, y, z), (x, y, z + nz((x - 10.9) / 6.1, (y - 10.5) / 5.8) + .12), .06, TIMBER, n=5)
    for (x, y), (px, py) in (((16.15, 11.05), (17.9, 10.6)), ((16.15, 15.6), (17.9, 16.6)), ((13.2, 15.6), (13.0, 17.0))):
        k.strut((x, y, z + 2.8), (px, py, z + .05), .025, CANVAS); k.cyl(px, py, z, .05, .25, TIMBER, n=4)
    # Cable drums, flag.
    cable_drum(k, 13.0, 18.2, z, 1.0, .9, yaw=10); cable_drum(k, 15.6, 18.0, z, .8, .75, yaw=70)
    with k.at(17.8, 17.6, z): k.cyl(0, 0, 0, .8, .07, WOOD, n=14); k.cyl(0, 0, .07, .55, .6, RUBBER, n=12); k.cyl(0, 0, .67, .8, .07, WOOD, n=14)
    k.pipe([(13.6, 18.2, z + .2), (11.6, 18.6, z + .08), (10.6, 17.8, z + .08)], .05, RUBBER, n=6)
    hazard_edge(k, 1.2, 18.9, 9.6, .3, z)
    flagpole(k, 1.8, 18.4, z, 8.0, w=2.2)

def spy_satellite(k, w, d):
    # Allied satellite ground station: a fully steerable Cassegrain dish on an elevation-over-
    # azimuth pedestal, a tracking dish, a geodesic radome, equipment building with roof plant.
    z = pad(k, w, d)
    big_dish(k, 7.9, 7.6, z, 6.2, az=98, el=48)
    bollards(k, [(11.4, 9.6), (10.6, 11.0), (5.0, 11.0)], z)
    # Equipment building (front right).
    top = block(k, 12.4, 11.6, 6.6, 5.6, 3.9, CLAD_WHITE, roof=ROOF, plinth=PRECAST, floors=1, glaze='strip', band=TEAM)
    roof_plant(k, 12.8, 12.0, top, 5.8, 4.8, seed=9, density=1.8)
    antenna_mast(k, 18.0, 12.6, top, 5.0, w=.6, dishes=2)
    with face(k, 'y+', 12.4, 17.2, z): door(k, 4.4, 1.0); k.box(4.0, 0, 2.5, 1.8, 1.0, .12, GALV)
    cable_tray(k, [(13.2, 11.6, z + 2.6), (13.2, 8.2, z + 2.6)], z); cable_tray(k, [(13.2, 8.2, z + 2.6), (10.6, 8.2, z + 2.6)], z)
    # Radome on its drum (back right).
    k.cyl(16.2, 4.0, z, 2.0, 2.4, CLAD_WHITE, n=20); k.cyl(16.2, 4.0, z, 2.08, .5, PRECAST, n=20)
    geodesic(k, 16.2, 4.0, z + 2.6, 2.5, WHITE, freq=2, lo=-.3, seed=2, skirt=.3)
    with k.at(16.2, 4.0, z, yaw=60): k.box(1.98, -.5, 0, .06, 1.0, 2.0, rgb(96, 104, 110))
    # Tracking dish and generator (front left).
    k.cbox(4.2, 16.6, z, 2.4, 2.4, .4, CONCRETE)
    dish(k, 4.2, 16.6, z + .4, 1.9, yaw=20, pitch=42)
    genset(k, 1.4, 11.6, z, 3.8, rgb(150, 156, 150))
    # Yard.
    fence(k, [(7.0, 19.5, z), (19.5, 19.5, z), (19.5, 9.4, z)], h=2.0)
    with face(k, 'y+', 14.6, 19.56, z + .8): sign(k, 0, 0, 1.1, .7)
    hazard_edge(k, 4.6, 12.0, 6.4, .25, z)
    flagpole(k, 10.4, 17.8, z, 8.0, w=2.2)
    floodlight(k, 19.0, 9.0, z, 7.0, yaw=150)

MODELS = {f.__name__: f for f in [nuclear_reactor, radar_tower, cloning_vats, nuclear_silo, iron_curtain, psychic_sensor,
                                  weather_control, chronosphere, gap_generator, spy_satellite]}
