"""Modelling kit for the realistic HD miniatures.

Models are authored in metres from real-world reference dimensions and scaled onto the game's
cells by one factor per model (`Kit(m, u)`: `u` cells per metre). x points forward (a unit's
facing 0), y to its left, z up. Everything is emitted as polygons into a `generate_hd_art.Model`,
so the same definitions feed the Blender renderer and the flat Pillow preview.

Colours are `C(rgb, material)`: the material class picks the shader in render_hd_blender.py.

  paint      painted steel: armour, machinery, cladding (dust, grime, chipped edges)
  team       the owner's paint; recoloured per player by the game
  steel      bare or galvanised metal          dark     gunmetal, blackened steel
  rubber     tyres, tracks, skirts, hoses      concrete poured concrete with formwork seams
  brick      brickwork                         corr     corrugated sheet (ribs along its height)
  roof       bitumen and gravel roofing        asphalt  tarmac, hardstanding
  glass      glazing (reflects the sky)        emit     self-lit: lamps, plasma, molten metal
  cloth      uniforms, canvas, sandbags        skin     faces and hands
  wood       timber, crates                    foliage  leaves, camouflage garnish
  hazard     yellow and black warning chevrons copper   copper, brass, gold-coloured metal
  rust       heavily corroded steel            water    liquid in vats and basins
  camo       disruptive-pattern vehicle paint  camocloth printed camouflage uniform cloth
  leather    boots, gloves, holsters (takes no dust)
"""
from contextlib import contextmanager
from math import sin, cos, pi, sqrt, radians
import numpy as np
import generate_hd_art as g

C = g.C
ZC = g.ZC

# --- Palette: measured, low-saturation real-world colours -------------------------------------
TEAM = C(g.BLUE, 'team')
# Vehicles
NATO_GREEN = C((86, 96, 72), 'paint'); NATO_SAND = C((168, 150, 112), 'paint')
NATO_GREY = C((122, 128, 128), 'paint'); SOV_GREEN = C((74, 84, 58), 'paint')
SOV_DARK = C((58, 66, 50), 'paint'); NAVY_GREY = C((128, 136, 142), 'paint')
DECK_GREY = C((84, 88, 92), 'paint'); HULL_RED = C((112, 48, 40), 'paint')
AIR_GREY = C((132, 138, 144), 'paint'); AIR_DARK = C((70, 74, 80), 'paint')
YELLOW = C((204, 160, 38), 'paint'); CAT_YELLOW = C((212, 158, 30), 'paint')
WHITE = C((214, 212, 204), 'paint'); OFFWHITE = C((190, 188, 178), 'paint')
BLACK = C((26, 27, 28), 'paint'); SIGNAL_RED = C((170, 44, 34), 'paint')
ORANGE = C((204, 96, 34), 'paint'); OLIVE = C((98, 100, 68), 'paint')
# Metals
STEEL = C((150, 154, 156), 'steel'); GALV = C((168, 172, 172), 'steel')
DARK = C((44, 46, 48), 'dark'); GUN = C((58, 60, 62), 'dark'); IRON = C((84, 86, 86), 'dark')
COPPER = C((186, 112, 62), 'copper'); BRASS = C((188, 150, 70), 'copper')
RUST = C((122, 74, 48), 'rust'); CHROME = C((210, 214, 216), 'steel')
# Masonry and ground
CONCRETE = C((158, 156, 148), 'concrete'); CONC_DARK = C((116, 114, 108), 'concrete')
CONC_LIGHT = C((188, 186, 176), 'concrete'); PRECAST = C((176, 172, 160), 'concrete')
BRICK = C((132, 70, 52), 'brick'); BRICK_DARK = C((104, 58, 46), 'brick')
BRICK_PALE = C((168, 132, 100), 'brick')
ASPHALT = C((62, 62, 62), 'asphalt'); TARMAC = C((80, 80, 78), 'asphalt')
ROOF = C((66, 66, 68), 'roof'); ROOF_RED = C((112, 62, 50), 'roof'); GRAVEL = C((128, 122, 110), 'roof')
# Cladding
CLAD_GREY = C((146, 152, 154), 'corr'); CLAD_BLUE = C((98, 116, 132), 'corr')
CLAD_GREEN = C((92, 108, 92), 'corr'); CLAD_SAND = C((176, 164, 136), 'corr')
CLAD_RUST = C((128, 84, 58), 'corr'); CLAD_WHITE = C((204, 204, 198), 'corr')
CLAD_RED = C((126, 62, 50), 'corr'); CLAD_DARK = C((78, 82, 84), 'corr')
# Glass and light
GLASS = C((38, 58, 70), 'glass'); GLASS_GREEN = C((44, 70, 66), 'glass')
GLASS_DARK = C((22, 30, 36), 'glass')
LAMP = C((255, 236, 190), 'emit'); RED_LAMP = C((255, 60, 40), 'emit')
GREEN_LAMP = C((90, 255, 120), 'emit'); PLASMA = C((120, 210, 255), 'emit')
TESLA = C((150, 190, 255), 'emit'); HOT = C((255, 150, 50), 'emit'); PSI = C((190, 110, 255), 'emit')
TOXIC = C((150, 255, 90), 'emit')
# Soft materials
RUBBER = C((34, 34, 34), 'rubber'); TYRE = C((30, 30, 30), 'rubber'); TRACK = C((52, 48, 44), 'rubber')
CANVAS = C((112, 108, 78), 'cloth'); SANDBAG = C((158, 142, 108), 'cloth'); TARP = C((70, 86, 70), 'cloth')
SKIN = C((196, 156, 124), 'skin'); WOOD = C((132, 100, 66), 'wood'); TIMBER = C((98, 76, 52), 'wood')
LEAF = C((60, 92, 46), 'foliage'); LEAF2 = C((84, 112, 54), 'foliage')
HAZ = C((214, 172, 40), 'hazard'); ORE = C((214, 166, 52), 'copper')
WATER = C((40, 86, 96), 'water')

def rgb(r, g_, b, mat='paint'): return C((r, g_, b), mat)
def tint(c, f): return g.shade(c, f)
def remat(c, mat): return C(tuple(c), mat)

# --- Outlines (lists of 2D points, counter-clockwise) -----------------------------------------
def circle(r, n=16, ry=None, a0=0.0):
    ry = r if ry is None else ry
    return [(r * cos(a0 + i * 2 * pi / n), ry * sin(a0 + i * 2 * pi / n)) for i in range(n)]

def rrect(w, d, r, k=3):
    """Rounded rectangle w x d centred on the origin, corner radius r, k segments per corner."""
    r = min(r, w / 2 - 1e-4, d / 2 - 1e-4)
    out = []
    for cx, cy, a0 in [(w / 2 - r, d / 2 - r, 0), (-w / 2 + r, d / 2 - r, pi / 2),
                       (-w / 2 + r, -d / 2 + r, pi), (w / 2 - r, -d / 2 + r, 3 * pi / 2)]:
        for i in range(k + 1):
            a = a0 + i * pi / 2 / k
            out.append((cx + r * cos(a), cy + r * sin(a)))
    return out

def superellipse(a, b, p=2.5, n=16):
    out = []
    for i in range(n):
        t = i * 2 * pi / n
        c, s = cos(t), sin(t)
        out.append((a * abs(c) ** (2 / p) * (1 if c >= 0 else -1), b * abs(s) ** (2 / p) * (1 if s >= 0 else -1)))
    return out

# --- The kit -----------------------------------------------------------------------------------
class Kit:
    def __init__(self, m, u=1.0):
        self.m, self.u = m, u
        self.T = np.eye(4)

    # Transform stack: with k.at(x, y, z, yaw=deg): ...
    @contextmanager
    def at(self, x=0.0, y=0.0, z=0.0, yaw=0.0, pitch=0.0, roll=0.0, s=1.0, sx=1.0, sy=1.0, sz=1.0):
        old = self.T
        a, b, c = radians(yaw), radians(pitch), radians(roll)
        Rz = np.array([[cos(a), -sin(a), 0, 0], [sin(a), cos(a), 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]])
        Ry = np.array([[cos(b), 0, -sin(b), 0], [0, 1, 0, 0], [sin(b), 0, cos(b), 0], [0, 0, 0, 1]])  # +pitch lifts +x
        Rx = np.array([[1, 0, 0, 0], [0, cos(c), -sin(c), 0], [0, sin(c), cos(c), 0], [0, 0, 0, 1]])
        Tr = np.eye(4); Tr[:3, 3] = (x, y, z)
        Sc = np.diag([s * sx, s * sy, s * sz, 1.0])
        self.T = old @ Tr @ Rz @ Ry @ Rx @ Sc
        try: yield self
        finally: self.T = old

    def pt(self, p):
        v = self.T @ np.array([p[0], p[1], p[2], 1.0])
        return (float(v[0]) * self.u, float(v[1]) * self.u, float(v[2]) * self.u * ZC)

    @contextmanager
    def grp(self, smooth=False):
        nested = getattr(self, '_open', False)
        if not nested: self.m.begin(smooth); self._open = True
        try: yield
        finally:
            if not nested: self.m.end(); self._open = False

    def poly(self, pts, c):
        self.m.face([self.pt(p) for p in pts], c)

    # --- Boxes and plates ---
    def box(self, x, y, z, w, d, h, c, top=None):
        """Box from its minimum corner. `top` paints the roof face another colour."""
        v = [(x, y, z), (x + w, y, z), (x + w, y + d, z), (x, y + d, z),
             (x, y, z + h), (x + w, y, z + h), (x + w, y + d, z + h), (x, y + d, z + h)]
        with self.grp():
            for ids in [(0, 4, 7, 3), (1, 2, 6, 5), (0, 1, 5, 4), (3, 7, 6, 2), (0, 3, 2, 1)]:
                self.poly([v[i] for i in ids], c)
            self.poly([v[i] for i in (4, 5, 6, 7)], top or c)

    def cbox(self, cx, cy, z, w, d, h, c, top=None):
        self.box(cx - w / 2, cy - d / 2, z, w, d, h, c, top)

    def plate(self, pts, c):
        """A single flat polygon (decal, marking, flag)."""
        with self.grp(): self.poly(pts, c)

    def rect(self, x, y, z, w, d, c):
        """Horizontal marking just above height z."""
        self.plate([(x, y, z), (x + w, y, z), (x + w, y + d, z), (x, y + d, z)], c)

    # --- Extrusions ---
    def extrude(self, outline, plane, a0, a1, c, smooth=False, cap=None):
        """Prism of a 2D outline. plane 'xz': outline (x, z) swept along y from a0 to a1;
        'xy': (x, y) swept along z; 'yz': (y, z) swept along x."""
        def P(q, a):
            if plane == 'xz': return (q[0], a, q[1])
            if plane == 'xy': return (q[0], q[1], a)
            return (a, q[0], q[1])
        n = len(outline)
        with self.grp(smooth):
            for i in range(n):
                q0, q1 = outline[i], outline[(i + 1) % n]
                self.poly([P(q0, a0), P(q1, a0), P(q1, a1), P(q0, a1)], c)
            self.poly([P(q, a0) for q in outline], cap or c)
            self.poly([P(q, a1) for q in outline[::-1]], cap or c)

    def loft(self, rings, c, smooth=True, cap0=True, cap1=True, cap_c=None):
        """Skin through rings of equal point count (each a list of 3D points)."""
        n = len(rings[0])
        with self.grp(smooth):
            for r0, r1 in zip(rings, rings[1:]):
                for i in range(n):
                    j = (i + 1) % n
                    self.poly([r0[i], r0[j], r1[j], r1[i]], c)
            if cap0: self.poly(rings[0][::-1], cap_c or c)
            if cap1: self.poly(rings[-1], cap_c or c)

    def stack(self, x, y, levels, c, k=3, smooth=True, cap_c=None):
        """Rounded-rectangle sections stacked in z: levels = [(z, w, d, r[, dx[, dy]])].
        Cast turrets, cabs, hulls with tumblehome, tapered towers."""
        rings = []
        for lv in levels:
            z, w, d, r = lv[:4]; dx = lv[4] if len(lv) > 4 else 0; dy = lv[5] if len(lv) > 5 else 0
            rings.append([(x + dx + px, y + dy + py, z) for px, py in rrect(w, d, r, k)])
        self.loft(rings, c, smooth, cap_c=cap_c)

    def body(self, sections, c, n=16, p=2.5, smooth=True):
        """Fuselage along x: sections = [(x, yc, zc, ry, rz)] with superelliptic cross-sections."""
        rings = [[(x, yc + a, zc + b) for a, b in superellipse(max(ry, 1e-4), max(rz, 1e-4), p, n)]
                 for x, yc, zc, ry, rz in sections]
        self.loft(rings, c, smooth)

    # --- Round things ---
    def rev(self, a, d, prof, c, n=16, smooth=True, caps=True):
        """Surface of revolution about the axis from point a along direction d ('x', 'y', 'z' or a
        vector). prof = [(t, r)]: radius r at distance t along the axis."""
        if isinstance(d, str): d = {'x': (1, 0, 0), 'y': (0, 1, 0), 'z': (0, 0, 1)}[d]
        d = np.array(d, float); d /= np.linalg.norm(d)
        ref = np.array([0, 0, 1.0]) if abs(d[2]) < .9 else np.array([1.0, 0, 0])
        u = np.cross(ref, d); u /= np.linalg.norm(u); v = np.cross(d, u)
        a = np.array(a, float)
        rings = []
        for t, r in prof:
            o = a + d * t
            rings.append([tuple(o + max(r, 1e-4) * (cos(i * 2 * pi / n) * u + sin(i * 2 * pi / n) * v)) for i in range(n)])
        self.loft(rings, c, smooth and n >= 8, cap0=caps, cap1=caps)

    def tube(self, a, b, r, c, r2=None, n=10, caps=True):
        d = np.array(b, float) - np.array(a, float)
        L = float(np.linalg.norm(d))
        if L < 1e-6: return
        self.rev(a, d, [(0, r), (L, r if r2 is None else r2)], c, n, caps=caps)

    def cyl(self, x, y, z, r, h, c, n=20, r2=None, top=None):
        self.rev((x, y, z), 'z', [(0, r), (h, r if r2 is None else r2)], c, n)
        if top is not None:
            rr = (r if r2 is None else r2) * .98
            self.plate([(x + rr * cos(i * 2 * pi / n), y + rr * sin(i * 2 * pi / n), z + h + .004) for i in range(n)], top)

    def ball(self, x, y, z, rx, c, ry=None, rz=None, n=16, k=8, lo=-1.0, hi=1.0):
        """Ellipsoid; lo/hi in -1..1 cut it to a dome or a bowl."""
        ry = rx if ry is None else ry; rz = rx if rz is None else rz
        rings = []
        for j in range(k + 1):
            s = lo + (hi - lo) * j / k
            t = np.arcsin(max(-1, min(1, s))); cr = max(cos(t), 1e-3)
            rings.append([(x + rx * cr * cos(i * 2 * pi / n), y + ry * cr * sin(i * 2 * pi / n), z + rz * s) for i in range(n)])
        self.loft(rings, c, True)

    def dome(self, x, y, z, r, c, h=None, n=20, k=6):
        self.ball(x, y, z, r, c, rz=h or r, n=n, k=k, lo=0.0, hi=1.0)

    def torus(self, x, y, z, R, r, c, n=24, k=8, axis='z'):
        rings = []
        for i in range(n + 1):
            a = i * 2 * pi / n
            ring = []
            for j in range(k):
                b = j * 2 * pi / k
                rad, off = R + r * cos(b), r * sin(b)
                if axis == 'z': ring.append((x + rad * cos(a), y + rad * sin(a), z + off))
                elif axis == 'y': ring.append((x + rad * cos(a), y + off, z + rad * sin(a)))
                else: ring.append((x + off, y + rad * cos(a), z + rad * sin(a)))
            rings.append(ring)
        self.loft(rings, c, True, cap0=False, cap1=False)

    def pipe(self, pts, r, c, n=8):
        """Pipe run through points, with a ball at each bend."""
        for a, b in zip(pts, pts[1:]): self.tube(a, b, r, c, n=n, caps=False)
        for p in pts[1:-1]: self.ball(p[0], p[1], p[2], r * 1.02, c, n=n, k=4)

    def wheel(self, x, y, z, r, w, tyre=TYRE, hub=IRON, n=18):
        """Road wheel, axle along y, centred at (x, y, z)."""
        self.rev((x, y - w / 2, z), 'y', [(0, r * .86), (w * .12, r), (w * .88, r), (w, r * .86)], tyre, n)
        for s in (-1, 1):
            yo = y + s * w / 2
            self.rev((x, yo, z), (0, s, 0), [(0, r * .62), (w * .04, r * .56), (w * .08, r * .2)], hub, n)

    # --- Frames ---
    def strut(self, a, b, t, c):
        """Square-section member between two points (lighter than a tube: 4 sides)."""
        self.tube(a, b, t * .7, c, n=4, caps=False)

    def truss(self, a, b, w, h, c, bays=6, t=.05):
        """Box truss from a to b (horizontal run), w wide and h deep, with diagonal bracing."""
        ax = np.array(a, float); d = np.array(b, float) - ax
        L = float(np.linalg.norm(d)); d /= L
        side = np.cross(d, (0, 0, 1.0)); side /= np.linalg.norm(side)
        cs = [ax + side * sx * w / 2 + np.array([0, 0, sz]) for sx in (-1, 1) for sz in (0, h)]
        for o in cs: self.strut(tuple(o), tuple(o + d * L), t, c)
        for i in range(bays + 1):
            o = d * L * i / bays
            for p, q in [(0, 1), (2, 3), (0, 2), (1, 3)]: self.strut(tuple(cs[p] + o), tuple(cs[q] + o), t * .7, c)
            if i < bays:
                o2 = d * L * (i + 1) / bays
                for p, q in [(0, 1), (2, 3), (1, 3)]:
                    pa, pb = (p, q) if i % 2 == 0 else (q, p)
                    self.strut(tuple(cs[pa] + o), tuple(cs[pb] + o2), t * .6, c)

    def mast(self, x, y, z, h, w0, w1, c, bays=6, t=.06):
        """Tapering four-legged lattice tower."""
        def corner(i, f):
            w = (w0 + (w1 - w0) * f) / 2
            sx, sy = [(-1, -1), (1, -1), (1, 1), (-1, 1)][i]
            return (x + sx * w, y + sy * w, z + h * f)
        for i in range(4): self.strut(corner(i, 0), corner(i, 1), t, c)
        for b in range(bays):
            f0, f1 = b / bays, (b + 1) / bays
            for i in range(4):
                j = (i + 1) % 4
                self.strut(corner(i, f1), corner(j, f1), t * .6, c)
                self.strut(corner(i, f0), corner(j, f1), t * .5, c)
                self.strut(corner(j, f0), corner(i, f1), t * .5, c)

    def rail(self, pts, c=None, h=1.05, t=.035, posts=1.6):
        """Handrail along a polyline (points at walking level): top rail, mid rail, posts."""
        c = c or GALV
        for a, b in zip(pts, pts[1:]):
            a, b = np.array(a, float), np.array(b, float)
            L = float(np.linalg.norm(b - a)); n = max(1, int(round(L / posts)))
            for hh in (h, h * .52): self.strut(tuple(a + (0, 0, hh)), tuple(b + (0, 0, hh)), t, c)
            for i in range(n + 1):
                o = a + (b - a) * i / n
                self.strut(tuple(o), tuple(o + (0, 0, h)), t, c)

    def ladder(self, x, y, z0, z1, c=None, axis='x', w=.45):
        """Vertical ladder; its rungs run along `axis`."""
        c = c or GALV
        dx, dy = (w / 2, 0) if axis == 'x' else (0, w / 2)
        for s in (-1, 1): self.strut((x + s * dx, y + s * dy, z0), (x + s * dx, y + s * dy, z1), .04, c)
        n = max(2, int((z1 - z0) / .32))
        for i in range(n + 1):
            z = z0 + (z1 - z0) * i / n
            self.strut((x - dx, y - dy, z), (x + dx, y + dy, z), .03, c)

    def stairs(self, x, y, z0, z1, run, w, c=None, axis='x'):
        """Straight open stair rising from z0 to z1 over `run` metres along +axis (run < 0: -axis)."""
        c = c or GALV
        n = max(2, int(abs(z1 - z0) / .19))
        for i in range(n):
            f0 = i / n
            px = x + (run * f0 if axis == 'x' else 0); py = y + (run * f0 if axis == 'y' else 0)
            tw, td = (abs(run) / n, w) if axis == 'x' else (w, abs(run) / n)
            ox = min(0, run / n) if axis == 'x' else 0; oy = min(0, run / n) if axis == 'y' else 0
            self.box(px + ox, py + oy, z0 + (z1 - z0) * (i + 1) / n - .04, tw, td, .04, c)
        for s in (0, 1):
            if axis == 'x': a, b = (x, y + s * w, z0), (x + run, y + s * w, z1)
            else: a, b = (x + s * w, y, z0), (x + s * w, y + run, z1)
            self.strut(a, b, .05, c)
            self.strut((a[0], a[1], a[2] + 1.0), (b[0], b[1], b[2] + 1.0), .035, c)
            self.strut(b, (b[0], b[1], b[2] + 1.0), .035, c); self.strut(a, (a[0], a[1], a[2] + 1.0), .035, c)
