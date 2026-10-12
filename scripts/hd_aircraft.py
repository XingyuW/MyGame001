"""Aircraft, modelled in metres from real reference airframes (dimensions in the comments).

They are drawn in flight: the origin lies on the ground under the centre of the airframe and the
belly rides about a metre above it (the game lifts the sprite and draws the shadow separately).
Each airframe has its own scale (`SCALE`, cells per metre) so that a 14 m strike jet, a 20 m
fighter, a helicopter with its rotor disc and a 120 m airship all fit the 0.93 cell reach of the
sprite frame. Ownership is carried the way air forces mark aircraft: a fin flash, a roundel on
each wing, a band round the fuselage, in the owner's colour (`TEAM`).
"""
from math import sin, cos, pi, radians, degrees, sqrt
from hd_kit import *

SCALE = {'harrier': .096, 'black_eagle': .0745, 'nighthawk': .092, 'kirov': .0154}

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

# --- Airframe parts ----------------------------------------------------------------------------
def shell(k, sections, col, n=16, p=2.5, smooth=True):
    """Fuselage along x through superelliptic sections [(x, yc, zc, ry, rz)]. `col` is a colour
    or a function (section index, angle of the panel in degrees: 0 = +y side, 90 = top)."""
    rings = [[(x, yc + a, zc + b) for a, b in superellipse(max(ry, 1e-3), max(rz, 1e-3), p, n)]
             for x, yc, zc, ry, rz in sections]
    pick = col if callable(col) else (lambda i, a: col)
    with k.grp(smooth):
        for i, (r0, r1) in enumerate(zip(rings, rings[1:])):
            for j in range(n):
                j2 = (j + 1) % n
                k.poly([r0[j], r0[j2], r1[j2], r1[j]], pick(i, (j + .5) * 360 / n))
        k.poly(rings[0][::-1], pick(0, 90)); k.poly(rings[-1], pick(len(rings) - 2, 90))

_FOIL = [(0, 0), (.1, .62), (.34, 1), (.72, .66), (1, 0)]

def _foil(v):
    for (v0, s0), (v1, s1) in zip(_FOIL, _FOIL[1:]):
        if v <= v1: return s0 + (s1 - s0) * (v - v0) / (v1 - v0)
    return 0.0

def _normal(w):
    rl, rt, tl, tt = w[:4]
    n = _unit(_cross(_sub(tl, rl), _sub(rt, rl)))
    return tuple(-q for q in n) if n[2] < -.3 else n

def wing(k, w, c):
    """Aerofoil surface. w = (root leading point, root trailing, tip leading, tip trailing,
    thickness / chord at the root[, at the tip])."""
    rl, rt, tl, tt, tk = w[:5]; tk2 = w[5] if len(w) > 5 else tk
    n = _normal(w)
    def ring(le, te, t):
        h = _len(_sub(te, le)) * t / 2
        P = lambda f, s: _add(_mix(le, te, f), n, h * s)
        top = [P(v, s) for v, s in _FOIL]
        return top + [P(v, -s) for v, s in _FOIL[-2:0:-1]]
    k.loft([ring(rl, rt, tk), ring(tl, tt, tk2)], c, smooth=True)

def on_wing(w, u, v, side=1, eps=.025):
    """Point just above (side 1) or below (-1) the wing skin at span fraction u, chord fraction v."""
    rl, rt, tl, tt, tk = w[:5]; tk2 = w[5] if len(w) > 5 else tk
    le, te = _mix(rl, tl, u), _mix(rt, tt, u)
    h = _len(_sub(te, le)) * (tk + (tk2 - tk) * u) / 2
    return _add(_mix(le, te, v), _normal(w), side * (h * _foil(v) + eps))

def wing_patch(k, w, u0, u1, v0, v1, c, side=1, eps=.025):
    """Painted panel on the wing skin (split at the aerofoil's facets so it lies on the skin)."""
    vs = sorted({v0, v1} | {v for v, _ in _FOIL if v0 < v < v1})
    with k.grp():
        for a, b in zip(vs, vs[1:]):
            k.poly([on_wing(w, u0, a, side, eps), on_wing(w, u1, a, side, eps),
                    on_wing(w, u1, b, side, eps), on_wing(w, u0, b, side, eps)], c)

def roundel(k, w, u, v, r, c, n=14, eps=.03):
    """Disc painted on the upper wing skin."""
    o = on_wing(w, u, v, 1, eps)
    e1 = _unit(_sub(on_wing(w, u, v + .05, 1, eps), on_wing(w, u, v - .05, 1, eps)))
    s = _sub(on_wing(w, u + .05, v, 1, eps), on_wing(w, u - .05, v, 1, eps))
    e2 = _unit(_add(s, e1, -_dot(s, e1)))
    k.plate([_add(_add(o, e1, r * cos(i * 2 * pi / n)), e2, r * sin(i * 2 * pi / n)) for i in range(n)], c)

def control_lines(k, w, c, spans=((.08, .5, .74), (.54, .92, .76))):
    """Flap and aileron hinge lines: thin dark strips on the upper skin."""
    for u0, u1, v in spans:
        wing_patch(k, w, u0, u1, v, v + .028, c)
        for u in (u0, u1): wing_patch(k, w, u - .006, u + .006, v, .985, c)

def store(k, x, y, z, L, r, c, nose=None, fins=0.0, n=8):
    """Underwing store pointing along +x, centred on x: tank, bomb or missile."""
    x0 = x - L / 2
    k.rev((x0, y, z), 'x', [(0, r * .25), (L * .1, r * .8), (L * .22, r), (L * .7, r), (L * .82, r * .82)], c, n)
    k.rev((x0 + L * .82, y, z), 'x', [(0, r * .82), (L * .12, r * .45), (L * .18, .01)], nose or c, n)
    if fins:
        for a in (45, 135, 225, 315):
            dy, dz = cos(radians(a)) * fins, sin(radians(a)) * fins
            k.plate([(x0 + L * .2, y, z), (x0 + L * .02, y + dy, z + dz), (x0 - L * .02, y + dy, z + dz), (x0, y, z)], c)

def pylon(k, w, u, v0, v1, drop, c):
    """Pylon hanging `drop` metres under the wing between chord fractions v0 and v1; returns
    the point under its middle where the store hangs."""
    a, b = on_wing(w, u, v0, -1, -.03), on_wing(w, u, v1, -1, -.03)
    k.extrude([(a[0], a[2]), (b[0], b[2]), (b[0] + .1, b[2] - drop), (a[0] - .35, a[2] - drop)], 'xz', a[1] - .05, a[1] + .05, c)
    return ((a[0] + b[0]) / 2, a[1], min(a[2], b[2]) - drop)

def nozzle(k, x, y, z, r, L, c, yaw=180.0, pitch=0.0):
    """Jet pipe with a dark throat, exhausting along the given heading."""
    with k.at(x, y, z, yaw=yaw, pitch=pitch):
        k.rev((0, 0, 0), 'x', [(0, r), (L * .5, r * 1.03), (L, r * .84)], c, n=12)
        k.rev((L - .04, 0, 0), 'x', [(0, r * .7), (.05, r * .7)], BLACK, n=10)

def wire(k, pts, t, c):
    """Thin member through a run of points."""
    for a, b in zip(pts, pts[1:]): k.strut(a, b, t, c)

def canopy(k, x, z, rx, ry, rz, frames=(), c=GLASS):
    """Blown canopy with its frame hoops (positions as fractions of the half length)."""
    k.ball(x, 0, z, rx, c, ry=ry, rz=rz, n=14, k=8, lo=-.15, hi=1.0)
    for f in frames:
        s = sqrt(max(0.0, 1 - f * f)) * 1.02
        wire(k, [(x + f * rx, ry * s * cos(radians(a)), z + rz * s * sin(radians(a))) for a in range(-10, 191, 25)], .06, DARK)

# --- Strike jet --------------------------------------------------------------------------------
def harrier(k):
    # Allied V/STOL strike jet, after the AV-8B Harrier II: 14.12 m long, 9.25 m span, 3.55 m
    # high; shoulder wing with 12 degrees of anhedral, four swivelling nozzles, outrigger gear.
    c = AIR_GREY; up = tint(c, .78); lo = tint(c, 1.08); line = tint(c, .5)
    def skin(i, a): return GLASS_DARK if i == 0 else (up if 28 < a < 152 else lo if a > 200 and a < 340 else c)
    shell(k, [(7.05, 0, 1.80, .03, .03), (6.75, 0, 1.82, .15, .15), (6.0, 0, 1.88, .34, .36), (5.2, 0, 1.95, .50, .56),
              (4.4, 0, 2.02, .60, .78), (3.4, 0, 2.05, .68, .95), (2.2, 0, 2.05, .82, 1.02), (.5, 0, 2.05, .95, 1.05),
              (-1.5, 0, 2.08, .92, 1.0), (-3.2, 0, 2.2, .66, .78), (-4.8, 0, 2.38, .40, .5), (-6.2, 0, 2.5, .2, .26),
              (-7.05, 0, 2.52, .06, .08)], skin, n=16)
    k.plate([(6.3, -.2, 2.2), (6.3, .2, 2.2), (5.05, .3, 2.5), (5.05, -.3, 2.5)], BLACK)  # anti-glare panel
    canopy(k, 4.05, 2.72, 1.4, .44, .66, frames=(.55, -.35))
    k.box(2.2, -.3, 3.0, .8, .6, .14, up)  # air-conditioning scoop behind the canopy
    for s in (-1, 1):
        # Semicircular 'elephant ear' intakes and the engine bay they feed.
        shell(k, [(3.55, s * .98, 2.0, .36, .80), (3.1, s * 1.04, 2.0, .47, .9), (1.6, s * 1.02, 2.05, .48, .92),
                  (.2, s * .82, 2.1, .3, .72)], lambda i, a: up if 30 < a < 150 else c, n=12)
        k.plate([(3.57, s * .98 + .30 * cos(i * pi / 6), 2.0 + .72 * sin(i * pi / 6)) for i in range(12)], BLACK)
        k.plate([(3.59, s * .98 + .12 * cos(i * pi / 4), 2.0 + .3 * sin(i * pi / 4)) for i in range(8)], IRON)  # fan face
        # Cold front and hot rear nozzles, swept aft in wing-borne flight.
        nozzle(k, 1.5, s * 1.22, 1.6, .33, .75, IRON, yaw=180 - s * 8, pitch=14)
        nozzle(k, -.9, s * 1.12, 1.6, .3, .8, rgb(92, 78, 66, 'dark'), yaw=180 - s * 8, pitch=14)
        k.box(-2.4, s * .86 - .03, 1.25, 1.5, .06, .7, rgb(96, 88, 80, 'steel'))  # heat shield
        k.rev((-.9, s * .46, 1.05), 'x', [(0, .05), (.4, .22), (2.8, .22), (3.3, .05)], lo, n=8)  # gun pods
        # Wing: 36 degrees of sweep, marked anhedral, the outrigger fairing at mid span.
        w = ((2.35, s * .5, 3.02), (-1.9, s * .5, 3.02), (-1.2, s * 4.62, 2.22), (-2.5, s * 4.62, 2.22), .095, .07)
        wing(k, w, up)
        wing(k, ((3.9, s * .66, 2.84), (1.9, s * .66, 2.95), (2.1, s * 1.5, 2.9), (1.8, s * 1.5, 2.92), .05), up)  # root extension
        control_lines(k, w, line)
        wing_patch(k, w, .955, 1.0, .0, 1.0, lo)
        roundel(k, w, .6, .5, .46, TEAM)
        o = on_wing(w, .56, .8, -1, -.1)
        k.rev((o[0] + 1.2, o[1], o[2] - .12), 'x', [(0, .04), (.5, .17), (2.2, .17), (3.1, .04)], c, n=8)  # outrigger
        # Stores: drop tank inboard, bomb amidships, Sidewinder outboard.
        p = pylon(k, w, .27, .25, .75, .3, c); store(k, p[0] + .2, p[1], p[2] - .3, 4.2, .3, lo)
        p = pylon(k, w, .44, .3, .75, .26, c); store(k, p[0] + .2, p[1], p[2] - .19, 2.3, .19, OLIVE, fins=.22)
        p = pylon(k, w, .8, .25, .8, .2, c); store(k, p[0] + .3, p[1], p[2] - .08, 2.6, .075, WHITE, nose=DARK, fins=.2)
        # All-moving tailplane with 15 degrees of anhedral.
        t = ((-4.4, s * .25, 2.5), (-6.15, s * .25, 2.5), (-5.8, s * 2.12, 1.95), (-6.65, s * 2.12, 1.95), .07)
        wing(k, t, up)
    fin = ((-3.2, 0, 2.8), (-6.4, 0, 2.62), (-5.9, 0, 4.42), (-6.8, 0, 4.42), .075, .06)
    wing(k, fin, up)
    for s in (-1, 1):
        wing_patch(k, fin, .56, .86, .04, .97, TEAM, s)
        wing_patch(k, fin, .0, .9, .68, .7, line, s)  # rudder hinge
    k.box(-3.6, -.16, 2.85, .9, .32, .3, up)  # fin-root ram intake
    k.extrude([(-4.2, 1.95), (-5.6, 1.95), (-5.9, 1.45), (-5.2, 1.45)], 'xz', -.03, .03, c)  # ventral fin
    k.tube((6.9, .28, 1.95), (7.05, .28, 1.9), .02, DARK, n=4)
    k.ball(-7.0, 0, 2.52, .09, RED_LAMP, n=6, k=3)

# --- Air-superiority fighter -------------------------------------------------------------------
def black_eagle(k):
    # Allied air-superiority fighter, after the F-15C Eagle (19.43 m long, 13.05 m span, 5.63 m
    # high) with the outward-canted fins of the F-22: twin engines in a broad lifting fuselage,
    # raked two-dimensional intakes, missile rails on the clipped wing tips.
    c = AIR_GREY; up = tint(c, .84); dk = tint(c, .68); line = tint(c, .5); radome = rgb(98, 102, 108)
    shell(k, [(9.7, 0, 1.78, .03, .03), (9.2, 0, 1.8, .2, .2), (8.2, 0, 1.86, .42, .45), (7.4, 0, 1.92, .56, .6),
              (6.2, 0, 1.98, .66, .72), (4.6, 0, 2.02, .7, .74), (3.0, 0, 2.08, .72, .66), (1.2, 0, 2.1, .7, .55)],
          lambda i, a: radome if i < 2 else (up if 20 < a < 160 else c), n=16)
    k.plate([(8.2, -.22, 2.32), (8.2, .22, 2.32), (7.2, .34, 2.55), (7.2, -.34, 2.55)], BLACK)
    canopy(k, 5.35, 2.56, 2.0, .52, .7, frames=(.52, -.1))
    # Broad centre fuselage and the two engine tunnels.
    shell(k, [(4.4, 0, 1.92, 1.8, .7), (2.6, 0, 1.9, 1.9, .76), (-3.5, 0, 1.9, 1.9, .76), (-6.2, 0, 1.92, 1.86, .6),
              (-7.0, 0, 1.95, 1.7, .4)], lambda i, a: up if 12 < a < 168 else c, n=20, p=5.0)
    shell(k, [(3.6, 0, 2.5, .55, .34), (1.5, 0, 2.56, .5, .26), (-2.5, 0, 2.55, .34, .16), (-4.5, 0, 2.5, .2, .1)], up, n=10)
    k.box(-.6, -.55, 2.66, 2.6, 1.1, .05, dk)  # air brake
    for s in (-1, 1):
        # Intake: raked lip, the ramp roof leading the floor.
        k.extrude([(5.1, 2.62), (4.2, 1.2), (2.4, 1.16), (2.4, 2.64)], 'xz', min(s * .74, s * 1.86), max(s * .74, s * 1.86), c)
        ya, yb = s * .82, s * 1.78
        k.plate([(5.06, ya, 2.53), (5.06, yb, 2.53), (4.26, yb, 1.28), (4.26, ya, 1.28)], BLACK)
        shell(k, [(-1.0, s * .84, 2.12, .7, .62), (-4.0, s * .84, 2.08, .74, .7), (-6.9, s * .84, 2.0, .68, .66)], up, n=12)
        # Afterburner nozzle: titanium petals.
        k.rev((-6.9, s * .84, 2.0), (-1, 0, 0), [(0, .68), (.5, .7), (1.5, .52)], rgb(84, 80, 76, 'dark'), n=14)
        k.rev((-8.36, s * .84, 2.0), (-1, 0, 0), [(0, .44), (.05, .44)], BLACK, n=12)
        for i in range(14):
            a = i * 2 * pi / 14
            k.strut((-7.45, s * .84 + .71 * cos(a), 2.0 + .71 * sin(a)), (-8.38, s * .84 + .535 * cos(a), 2.0 + .535 * sin(a)), .05, BLACK)
        # Tail boom carrying the fin and the stabilator.
        k.extrude([(-4.0, 1.74), (-9.0, 1.9), (-9.0, 2.14), (-4.0, 2.4)], 'xz', min(s * 1.72, s * 2.3), max(s * 1.72, s * 2.3), up)
        # Wing: 45 degrees of leading-edge sweep, raked tip with a launch rail.
        w = ((3.1, s * 1.8, 2.5), (-3.5, s * 1.8, 2.5), (-1.85, s * 6.4, 2.42), (-3.75, s * 6.4, 2.42), .055, .04)
        wing(k, w, up)
        control_lines(k, w, line, ((.04, .52, .76), (.56, .94, .74)))
        wing_patch(k, w, .0, .45, .3, .62, dk)
        roundel(k, w, .66, .52, .62, TEAM)
        k.box(-3.9, s * 6.42 - .06, 2.3, 2.6, .12, .14, c)
        store(k, -2.3, s * 6.56, 2.4, 2.9, .075, WHITE, nose=DARK, fins=.24)
        p = pylon(k, w, .3, .25, .8, .35, c); store(k, p[0] + .3, p[1], p[2] - .34, 5.6, .36, c)
        store(k, p[0] + .2, p[1] + s * .32, p[2] + .12, 3.6, .09, WHITE, nose=DARK, fins=.26)
        store(k, .8, s * 1.72, 1.12, 3.7, .1, WHITE, nose=DARK, fins=.28)   # fuselage-corner missiles
        store(k, -3.6, s * 1.72, 1.12, 3.7, .1, WHITE, nose=DARK, fins=.28)
        t = ((-5.4, s * 2.2, 2.02), (-8.5, s * 2.2, 2.02), (-8.1, s * 4.3, 2.0), (-9.3, s * 4.3, 2.0), .05)
        wing(k, t, up); wing_patch(k, t, .0, .2, .1, .95, dk)
        f = ((-4.2, s * 1.98, 2.25), (-7.7, s * 1.98, 2.25), (-7.1, s * 2.55, 5.25), (-8.3, s * 2.55, 5.25), .06, .05)
        wing(k, f, up)
        for q in (-1, 1):
            wing_patch(k, f, .68, .9, .03, .97, TEAM, q)
            wing_patch(k, f, .06, .96, .7, .72, line, q)
        k.rev((-7.0, s * 2.57, 5.3), (-1, 0, 0), [(0, .02), (.3, .1), (1.2, .1), (1.5, .03)], dk, n=6)  # fin-tip pod
    k.ball(9.72, 0, 1.78, .04, DARK, n=6, k=3)

# --- Transport helicopter ----------------------------------------------------------------------
def rotor_blade(k, R, chord, droop, c, tip=None, root=.9, t=.07):
    """One main-rotor blade along +x from the hub: cuff, constant-chord blade, swept tip."""
    z = lambda r: -droop * (r / R) ** 2
    def ring(r, ch, back=0.0):
        return [(r, -ch / 2 - back, z(r) - t / 2), (r, ch / 2 - back, z(r) - t / 2), (r, ch / 2 - back, z(r) + t / 2), (r, -ch / 2 - back, z(r) + t / 2)]
    k.loft([ring(.25, chord * .42), ring(root, chord * .42)], IRON, smooth=False)
    stations = [root, R * .3, R * .5, R * .7, R * .86, R * .94]  # the droop is a curve
    k.loft([ring(r, chord) for r in stations], c, smooth=False)
    k.loft([ring(R * .94, chord), ring(R * .975, chord * .92, chord * .2)], c, smooth=False)
    k.loft([ring(R * .975, chord * .92, chord * .2), ring(R, chord * .82, chord * .42)], tip or c, smooth=False)

def nighthawk(k):
    # Allied transport helicopter, after the UH-60 Black Hawk: 19.76 m over the turning rotors,
    # fuselage 15.26 m, cabin 2.36 m wide, four-blade 16.36 m main rotor, canted tail rotor.
    c = rgb(78, 84, 76); dk = tint(c, .72); lt = tint(c, 1.16)
    hx = 1.7  # the rotor hub rides ahead of the origin so rotor disc and tail fit the frame together
    def skin(i, a):
        if i == 2 and 30 < a < 150: return GLASS                 # windscreens
        if i == 3 and (a < 30 or 150 < a < 180): return GLASS    # cockpit door windows
        if i == 8 and not 60 < a < 120: return TEAM              # tail-boom band
        return dk if 40 < a < 140 else c
    shell(k, [(6.06, 0, 1.72, .1, .12), (5.75, 0, 1.76, .55, .44), (5.2, 0, 1.94, .92, .8), (4.5, 0, 2.1, 1.1, 1.05),
              (3.5, 0, 2.15, 1.18, 1.12), (.2, 0, 2.15, 1.18, 1.12), (-1.0, 0, 2.22, 1.1, 1.06), (-2.3, 0, 2.5, .8, .86),
              (-3.6, 0, 2.7, .52, .66), (-4.5, 0, 2.78, .45, .58), (-7.0, 0, 3.0, .28, .4), (-8.0, 0, 3.1, .2, .3)],
          skin, n=20, p=3.2)
    k.strut((5.72, 0, 2.2), (4.5, 0, 3.14), .09, c)  # windscreen centre post
    k.ball(5.85, 0, 1.36, .22, DARK, n=10, k=5)      # FLIR turret
    for s in (-1, 1):
        y = s * 1.2
        # Sliding cabin door with its two windows, the door track above it.
        k.plate([(3.3, y, 1.25), (.75, y, 1.25), (.75, y, 3.0), (3.3, y, 3.0)], tint(c, .9))
        for x0 in (2.25, 1.0): k.plate([(x0, y + s * .012, 2.2), (x0 + .82, y + s * .012, 2.2), (x0 + .82, y + s * .012, 2.86), (x0, y + s * .012, 2.86)], GLASS)
        k.box(-1.6, min(y, y + s * .04), 3.02, 5.0, .04, .06, DARK)
        k.plate([(-.2, y, 2.25), (-.9, y, 2.3), (-.9, y, 2.8), (-.2, y, 2.8)], GLASS_DARK)  # gunner's window
        # Fixed main gear: wheel, shock strut and drag link.
        k.wheel(3.55, s * 1.42, .42, .36, .26, n=12)
        k.tube((3.55, s * 1.3, .5), (3.3, s * 1.15, 1.9), .07, lt, n=6)
        k.tube((3.5, s * 1.28, .48), (1.9, s * 1.05, 1.25), .06, c, n=6)
        k.box(2.9, min(s * 1.18, s * 1.34), 1.2, 1.3, .16, .18, c)  # landing-gear sponson step
        # Engine nacelle: intake, cowl, upturned exhaust suppressor.
        shell(k, [(2.45, s * .8, 3.38, .26, .26), (2.0, s * .82, 3.4, .4, .38), (-.5, s * .84, 3.4, .42, .4),
                  (-1.4, s * .84, 3.38, .36, .32)], lambda i, a: dk, n=10, p=2.8)
        k.plate([(2.47, s * .8 + .2 * cos(i * pi / 4), 3.38 + .2 * sin(i * pi / 4)) for i in range(8)], BLACK)
        nozzle(k, -1.3, s * .86, 3.4, .27, .95, rgb(60, 56, 52, 'dark'), yaw=180 - s * 28, pitch=-6)
        # Stabilator.
        st = ((-7.45, 0, 3.02), (-8.55, 0, 3.02), (-7.6, s * 2.18, 3.02), (-8.5, s * 2.18, 3.02), .1)
        wing(k, st, c); wing_patch(k, st, .5, .97, .1, .92, TEAM)
        k.tube((5.9, s * .45, 2.0), (6.3, s * .45, 1.98), .025, DARK, n=4)  # pitot heads
    # Transmission fairing ('doghouse') between the engines.
    shell(k, [(3.4, 0, 3.18, .5, .2), (2.5, 0, 3.42, .68, .4), (.6, 0, 3.5, .74, .46), (-1.2, 0, 3.42, .6, .4),
              (-2.9, 0, 3.2, .3, .2)], dk, n=12, p=3.0)
    k.cbox(3.0, 0, 3.3, .7, 1.5, .1, TEAM)  # recognition panel on the cabin roof
    k.box(-7.4, -.07, 3.3, 3.9, .14, .1, dk)  # tail drive-shaft cover
    # Tail pylon, canted tail rotor on the starboard side, tail wheel.
    fin = ((-7.1, 0, 3.05), (-8.45, 0, 2.95), (-8.0, 0, 5.05), (-8.8, 0, 5.05), .16, .12)
    wing(k, fin, c)
    with k.at(-8.23, .34, 4.55, roll=-20):
        k.tube((0, -.3, 0), (0, .12, 0), .12, IRON, n=8)
        for i in range(4):
            a = radians(45 + i * 90)
            k.loft([[(.18 * cos(a) - .13 * sin(a), .08, .18 * sin(a) + .13 * cos(a)), (.18 * cos(a) + .13 * sin(a), .08, .18 * sin(a) - .13 * cos(a)),
                     (.18 * cos(a) + .13 * sin(a), .12, .18 * sin(a) - .13 * cos(a)), (.18 * cos(a) - .13 * sin(a), .12, .18 * sin(a) + .13 * cos(a))],
                    [(1.67 * cos(a) - .13 * sin(a), .08, 1.67 * sin(a) + .13 * cos(a)), (1.67 * cos(a) + .13 * sin(a), .08, 1.67 * sin(a) - .13 * cos(a)),
                     (1.67 * cos(a) + .13 * sin(a), .12, 1.67 * sin(a) - .13 * cos(a)), (1.67 * cos(a) - .13 * sin(a), .12, 1.67 * sin(a) + .13 * cos(a))]],
                   BLACK, smooth=False)
    k.wheel(-4.9, 0, .5, .24, .2, n=10)
    k.tube((-4.9, 0, .55), (-4.5, 0, 2.4), .06, lt, n=6)
    # Main rotor: mast, swashplate, hub with its vibration absorber, four drooping blades.
    k.cyl(hx, 0, 3.9, .2, .5, IRON, n=10); k.cyl(hx, 0, 4.12, .34, .08, DARK, n=12)
    k.cyl(hx, 0, 4.36, .46, .2, IRON, n=12); k.cyl(hx, 0, 4.56, .3, .14, DARK, n=10)
    for i in range(4):
        with k.at(hx, 0, 4.46, yaw=45 + i * 90):
            rotor_blade(k, 8.18, .53, .42, BLACK, tip=tint(YELLOW, .85))
            k.strut((.3, .16, -.3), (.75, .2, -.02), .05, STEEL)  # pitch link
    k.strut((-2.4, .7, 2.9), (-6.9, .28, 3.3), .03, DARK)  # HF aerial along the boom
    k.ball(-8.4, 0, 5.08, .08, RED_LAMP, n=6, k=3)

# --- Heavy bomber airship ----------------------------------------------------------------------
def kirov(k):
    # Soviet heavy bomber airship: a rigid hull 120 m long and 37 m across (half a Hindenburg),
    # gores laced over ring frames, control car under the bow, four pusher engine cars on
    # outriggers, cruciform fins with rudders and elevators.
    hull = rgb(112, 116, 104); belly = rgb(60, 63, 60); R, zc, xm = 18.5, 24.5, 10.0
    def rad(x):
        if x >= xm: return R * sqrt(max(0.0, 1 - ((x - xm) / 49.0) ** 2))
        return R * max(0.0, 1 - ((xm - x) / 70.2) ** 2.1) ** .8
    xs = [59, 58.3, 56.5, 53.5, 49, 43, 37, 32, 26, 19, 11, 3, -6, -15, -24, -32, -39, -45, -50, -54.5, -58, -60]
    n = 24
    rings = [[(x, max(rad(x), .25) * sin(j * 2 * pi / n), zc + max(rad(x), .25) * cos(j * 2 * pi / n)) for j in range(n)] for x in xs]
    def gore(i, j):
        a = (j + .5) * 360 / n; a = a if a <= 180 else 360 - a   # 0 = top, 180 = keel
        if i == 6: return TEAM
        f = (.97 + .06 * ((i + j) % 2)) * (1.0 if a < 100 else .9)
        return tint(belly if a > 128 else hull, f)
    with k.grp(False):
        for i, (r0, r1) in enumerate(zip(rings, rings[1:])):
            for j in range(n):
                j2 = (j + 1) % n
                k.poly([r0[j], r0[j2], r1[j2], r1[j]], gore(i, j))
    k.rev((58.6, 0, zc), 'x', [(0, 1.5), (.9, .9), (1.6, .25)], STEEL, n=10)  # mooring cone
    # Longitudinal girders standing proud of the fabric on the upper hull.
    for j in range(n):
        a = j * 2 * pi / n
        if cos(a) < -.55: continue
        wire(k, [(x, (rad(x) + .1) * sin(a), zc + (rad(x) + .1) * cos(a)) for x in xs[2:-2]], .3, tint(hull, .8))
    # Shark mouth and eyes on the bow, as bomber crews paint them.
    def S(x, th, lift): r = rad(x) + lift; return (x, r * sin(radians(th)), zc + r * cos(radians(th)))
    for s in (-1, 1):
        lip = lambda t: (132 - 22 * t - 17 * sin(pi * t) ** .7, 132 - 22 * t + 14 * sin(pi * t) ** .7)
        ts = [i / 9 for i in range(10)]
        with k.grp():
            for ta, tb in zip(ts, ts[1:]):
                xa, xb = 56 - 23 * ta, 56 - 23 * tb; (ua, la), (ub, lb) = lip(ta), lip(tb)
                for q in range(3):
                    fa, fb = q / 3, (q + 1) / 3
                    k.poly([S(xa, s * (ua + (la - ua) * fa), .45), S(xb, s * (ub + (lb - ub) * fa), .45),
                            S(xb, s * (ub + (lb - ub) * fb), .45), S(xa, s * (ua + (la - ua) * fb), .45)], rgb(124, 30, 26))
                tm = (ta + tb) / 2; xq = 56 - 23 * tm; um, lm = lip(tm)
                if 0 < ta and tb < 1:
                    k.poly([S(xa, s * ua, .55), S(xb, s * ub, .55), S(xq, s * (um + (lm - um) * .42), .55)], WHITE)
                    k.poly([S(xa, s * la, .55), S(xb, s * lb, .55), S(xq, s * (lm - (lm - um) * .36), .55)], WHITE)
        for rr, cc, lift in ((1.7, WHITE, .5), (.85, BLACK, .6)):
            k.plate([S(47.5 + rr * cos(i * pi / 5), s * (84 + degrees(rr * sin(i * pi / 5) / rad(47.5))), lift) for i in range(10)], cc)
    # Gas-vent hoods and gun cupolas along the ridge.
    for x in (40, 28, 16, 4, -8, -20): k.cbox(x, 0, zc + rad(x) - .2, 2.4, 1.6, .9, tint(hull, .78))
    for x in (34, -14):
        k.cyl(x, 0, zc + rad(x) - .3, 1.7, .9, tint(hull, .85), n=10); k.dome(x, 0, zc + rad(x) + .6, 1.3, GLASS_DARK, n=10, k=3)
    # Control car under the bow: glazed nose, a row of lit windows, bomb-aimer's blister.
    shell(k, [(42.5, 0, 9.0, .4, .5), (40.5, 0, 7.6, 2.0, 2.3), (36, 0, 6.2, 2.9, 3.4), (22, 0, 5.4, 3.1, 3.3),
              (17, 0, 5.9, 2.4, 2.6), (13.5, 0, 6.6, .5, .8)],
          lambda i, a: GLASS if i == 0 or (i == 1 and 15 < a < 165) else tint(hull, .9), n=16, p=3.0)
    for s in (-1, 1):
        for i in range(7):
            x = 23 + i * 1.9
            k.plate([(x, s * 3.14, 5.4), (x + 1.3, s * 3.14, 5.4), (x + 1.3, s * 3.14, 6.5), (x, s * 3.14, 6.5)], LAMP if i in (1, 4) else GLASS_DARK)
        k.strut((34, s * 2.2, 8.5), (34, s * 6, zc - rad(34) * .92), .3, tint(hull, .7))
        k.strut((22, s * 2.4, 8.3), (22, s * 7, zc - rad(22) * .92), .3, tint(hull, .7))
    # Bomb bay along the keel.
    k.box(-16, -2.6, zc - R - .9, 28, 5.2, 1.6, belly)
    for i in range(6): k.rev((-13 + i * 4.4, 0, zc - R - 2.6), 'x', [(0, .1), (.6, .7), (2.4, .7), (3.3, .2)], rgb(52, 56, 48), n=6)
    # Four engine cars on outriggers, pusher propellers.
    for x in (6.0, -27.0):
        r = rad(x)
        for s in (-1, 1):
            a = radians(116)
            py, pz = s * (r + 6.5) * sin(a), zc + (r + 6.5) * cos(a)
            k.rev((x + 5.4, py, pz), (-1, 0, 0), [(0, .3), (1.2, 1.7), (4.4, 2.0), (8.2, 1.6), (9.6, .7)], tint(hull, .82), n=10)
            k.rev((x + 5.3, py, pz), (-1, 0, 0), [(0, 1.1), (.5, 1.1)], DARK, n=8)  # radiator
            for b in (100, 132):
                hy, hz = s * (r - .3) * sin(radians(b)), zc + (r - .3) * cos(radians(b))
                k.strut((x + 2.8, py, pz), (x + 3.6, hy, hz), .5, tint(hull, .7)); k.strut((x - 2.0, py, pz), (x - 2.6, hy, hz), .5, tint(hull, .7))
            with k.at(x - 4.4, py, pz):
                k.rev((0, 0, 0), (-1, 0, 0), [(0, .7), (1.1, .1)], DARK, n=8)
                for i in range(3):
                    with k.at(roll=20 + i * 120 + (40 if s > 0 else 0)):
                        k.loft([[(-.14, -.42, .4), (.16, .42, .4), (.3, .42, .4), (0, -.42, .4)],
                                [(-.06, -.24, 4.4), (.06, .24, 4.4), (.2, .24, 4.4), (.08, -.24, 4.4)]], rgb(70, 60, 48, 'wood'), smooth=False)
    # Cruciform tail: fins with balanced rudders and elevators, bracing wires to the hull.
    for a, span in ((0, 23.5), (90, 23.0), (180, 19.5), (270, 23.0)):
        sy, sz = sin(radians(a)), cos(radians(a))
        P = lambda x, r_: (x, r_ * sy, zc + r_ * sz)
        f = (P(-25, rad(-25) - 1.2), P(-53.5, rad(-53.5) - .6), P(-39.5, span), P(-54.5, span), .1, .08)
        wing(k, f, tint(hull, .93))
        for q in (-1, 1):
            wing_patch(k, f, .28, .97, .73, 1.0, tint(hull, .74), q, eps=.08)   # rudder / elevator
            wing_patch(k, f, .5, .86, .08, .66, TEAM, q, eps=.08)
            n_ = _normal(f)
            tip = _add(on_wing(f, .9, .3, q), n_, 0)
            foot = (-36, (rad(-36) - .2) * (sy * cos(radians(q * 28)) + sz * sin(radians(q * 28))),
                    zc + (rad(-36) - .2) * (sz * cos(radians(q * 28)) - sy * sin(radians(q * 28))))
            k.strut(tip, foot, .16, DARK)
    k.ball(59.9, 0, zc, .3, RED_LAMP, n=6, k=3)

MODELS = {f.__name__: f for f in [harrier, black_eagle, nighthawk, kirov]}

def build(m, ident):
    MODELS[ident](Kit(m, SCALE[ident]))
