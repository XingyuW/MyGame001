"""Pre-rendered realistic miniatures: the models of hd_buildings.py / hd_units.py (and the terrain
tops of generate_hd_terrain.py) rendered by Blender Cycles.

Every material class of hd_kit.py has its own physically based shader: painted steel with dust,
grime in the recesses and chipped edges; poured concrete with formwork seams and rain streaks;
brickwork; corrugated sheet; bitumen roofing; tarmac; reflective glazing; rubber; cloth; emissive
lamps. Edges are bevelled, curved surfaces smoothed, and a real sun casts the shadow onto a shadow
catcher under a graded sky. Terrain tops (plain tuple colours) keep the legacy single material.

Run (renders raw passes into the cache; finish_hd_art.py composes the shipped sprites):
  /Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
      --python scripts/render_hd_blender.py -- [OUT_DIR] [b:ident | u:ident | w:ident ...]

u:ident is a unit's eight-facing sheet, w:ident the walk cycle of an infantry unit (six frames of
eight facings, hd_infantry.py).

Each sprite is rendered twice: a colour pass and a flat team-paint mask. The mask lets the
finishing step encode team paint into the exact ramp presentation.rs recolours, independent of
lighting, so deep shadows on team paint still recolour and neutral materials never do.
"""
import sys, re
from math import radians, sin, cos, sqrt, pi
from pathlib import Path
import bpy, bmesh

sys.path.insert(0, str(Path(__file__).resolve().parent))
import generate_hd_art as g
import generate_hd_terrain as terrain
import hd_buildings

ROOT = g.ROOT
CELL_PX = 32 * sqrt(2)          # one cell edge, in world pixels, seen perpendicular to the view
Z_PX = CELL_PX * cos(radians(30))  # model z is in projected screen pixels

# ---------------------------------------------------------------------------------------------
# Material classes. Source colours stay authored in generate_hd_art.py; here they are mapped onto
# a weathered, lower-saturation RA2 palette and given physical properties.
TEAM, GLASS, PAINT, METAL, DARKMETAL, CONCRETE, ORGANIC = range(7)
#                 roughness metallic
PROPS = {TEAM: (.42, .15), GLASS: (.08, .0), PAINT: (.5, .2), METAL: (.38, .75),
         DARKMETAL: (.5, .6), CONCRETE: (.88, .0), ORGANIC: (.75, .0)}
REMAP = {  # authored constant -> (rendered base colour, class)
    g.BLUE: ((42, 133, 202), TEAM), g.RED: ((42, 133, 202), TEAM),
    g.STEEL: ((128, 134, 132), METAL), g.WHITE: ((176, 172, 158), CONCRETE),
    g.DARK: ((44, 46, 46), DARKMETAL), g.GOLD: ((196, 156, 66), PAINT),
    g.GLASS: ((52, 78, 84), GLASS), g.OLIVE: ((96, 102, 70), PAINT),
    (140, 137, 114): ((122, 112, 96), METAL),      # Soviet structural steel: warmer, rust-toned
    (137, 153, 152): ((126, 132, 124), METAL),     # Allied armour: blue-grey drab
    (117, 121, 85): ((96, 96, 72), METAL),         # Soviet armour: dark olive
    (102, 111, 109): ((118, 116, 108), CONCRETE),  # foundation slab
    (194, 163, 124): ((182, 146, 116), ORGANIC),   # skin
    (190, 150, 20): ((214, 166, 52), METAL),       # ore nuggets (generate_hd_terrain.ORE)
    # Faction architecture (generate_hd_art.building).
    g.A_CONC: ((198, 196, 186), CONCRETE),         # Allied white concrete
    g.A_TRIM: ((136, 146, 156), PAINT),            # Allied blue-grey cladding
    g.S_CONC: ((120, 116, 104), CONCRETE),         # Soviet heavy concrete
    g.BRICK: ((126, 68, 52), CONCRETE),            # Soviet brick
    g.RUST: ((112, 74, 54), PAINT),                # rusted corrugated steel
    g.COPPER: ((176, 106, 60), METAL),             # coils and domes
    g.GLOW: ((150, 220, 236), GLASS),              # energised crystal and plasma
}

def norm(c):
    m = sqrt(sum(v * v for v in c)) or 1
    return [v / m for v in c]

def classify(c):
    """Find the authored constant this (possibly shaded) colour came from."""
    cn, mag = norm(c), sqrt(sum(v * v for v in c))
    best, score = None, .9985
    for src in REMAP:
        s = sum(a * b for a, b in zip(cn, norm(src)))
        if s > score: best, score = src, s
    if best is None:
        return c, CONCRETE if max(c) - min(c) < 30 else ORGANIC
    base, cls = REMAP[best]
    f = mag / sqrt(sum(v * v for v in best))
    return tuple(min(255, v * f) for v in base), cls

def to_linear(v):
    v /= 255
    return v / 12.92 if v <= .04045 else ((v + .055) / 1.055) ** 2.4

# ---------------------------------------------------------------------------------------------
class BlenderModel(g.Model):
    """Same authoring API as g.Model; records closed primitives instead of rasterizing."""
    def __init__(self, w, h, origin, angle=0):
        super().__init__(w, h, origin, angle)
        self.meta = []  # per primitive: smooth flag
        self.group = None

    def _prim(self, smooth):
        self.meta.append(smooth)
        return len(self.meta) - 1

    def begin(self, smooth=False): self.group = self._prim(smooth)
    def end(self): self.group = None

    def face(self, pts, c, prim=None):
        if prim is None: prim = self.group if self.group is not None else self._prim(False)
        # unit() rewrites faces as (key, scaled_pts, c); the key carries the primitive id.
        self.faces.append((prim, list(pts), c))

    def box(self, x, y, z, w, d, h, c, bevel=True):
        p = self._prim(False)
        v = [(x, y, z), (x + w, y, z), (x + w, y + d, z), (x, y + d, z),
             (x, y, z + h), (x + w, y, z + h), (x + w, y + d, z + h), (x, y + d, z + h)]
        for ids in [(0, 4, 7, 3), (1, 2, 6, 5), (0, 1, 5, 4), (3, 7, 6, 2), (4, 5, 6, 7), (0, 3, 2, 1)]:
            self.face([v[i] for i in ids], c, p)

    def cyl(self, x, y, z, r, h, c, n=24, r2=None):
        r2 = r if r2 is None else r2
        p = self._prim(n >= 12)
        a = [i * 2 * pi / n for i in range(n)]
        lo = [(x + r * cos(t), y + r * sin(t), z) for t in a]
        hi = [(x + r2 * cos(t), y + r2 * sin(t), z + h) for t in a]
        for i in range(n):
            j = (i + 1) % n
            self.face([lo[i], lo[j], hi[j], hi[i]], c, p)
        self.face(hi, g.shade(c, 1.05), p)
        self.face(lo[::-1], c, p)

    def ellipsoid(self, x, y, z, rx, ry, rz, c):
        p = self._prim(True)
        for j in range(12):
            a = -pi / 2 + j * pi / 12; b = a + pi / 12
            for i in range(24):
                u = i * 2 * pi / 24; v = u + 2 * pi / 24
                self.face([(x + rx * cos(t) * cos(s), y + ry * cos(t) * sin(s), z + rz * sin(t))
                           for t, s in [(a, u), (a, v), (b, v), (b, u)]], c, p)

    def render(self):
        return self

# ---------------------------------------------------------------------------------------------
def setup_scene():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    sc = bpy.context.scene
    sc.render.engine = 'CYCLES'
    prefs = bpy.context.preferences.addons['cycles'].preferences
    try:
        prefs.compute_device_type = 'METAL'
        # No specialised kernels compiled in the background: saving their binary archive can
        # crash Blender mid-run (seen 2026-10-25 with several processes at once).
        if hasattr(prefs, 'kernel_optimization_level'): prefs.kernel_optimization_level = 'OFF'
        prefs.get_devices()
        for d in prefs.devices: d.use = True
        sc.cycles.device = 'GPU'
    except Exception as e:  # CPU is slower but produces the same image.
        print('Cycles GPU unavailable:', e)
    sc.cycles.samples = 96
    sc.cycles.use_denoising = True
    sc.cycles.max_bounces = 4
    sc.render.film_transparent = True
    sc.render.filter_size = 1.2
    sc.view_settings.view_transform = 'Standard'
    sc.view_settings.look = 'None'
    sc.view_settings.exposure = 0
    sc.render.image_settings.file_format = 'PNG'
    sc.render.image_settings.color_mode = 'RGBA'
    sc.render.image_settings.color_depth = '8'

    world = bpy.data.worlds.new('World'); sc.world = world
    world.use_nodes = True
    bg = world.node_tree.nodes['Background']
    # Graded sky: pale haze at the horizon, blue overhead, dull earth below. Glass and metal
    # mirror it; the average matches the flat fill the terrain tops were rendered under.
    wn = world.node_tree
    tc = wn.nodes.new('ShaderNodeTexCoord'); sp = wn.nodes.new('ShaderNodeSeparateXYZ')
    wn.links.new(tc.outputs['Generated'], sp.inputs[0])
    half = wn.nodes.new('ShaderNodeMath'); half.operation = 'MULTIPLY_ADD'   # z in -1..1 -> 0..1
    half.inputs[1].default_value = .5; half.inputs[2].default_value = .5
    wn.links.new(sp.outputs['Z'], half.inputs[0])
    cr = wn.nodes.new('ShaderNodeValToRGB'); wn.links.new(half.outputs[0], cr.inputs[0])
    e = cr.color_ramp.elements
    e[0].position = .0; e[0].color = (.20, .19, .17, 1)
    e[1].position = 1.0; e[1].color = (.26, .40, .70, 1)
    for pos, c in [(.47, (.22, .21, .19, 1)), (.52, (.78, .82, .88, 1)), (.72, (.42, .54, .74, 1))]:
        el = cr.color_ramp.elements.new(pos); el.color = c
    wn.links.new(cr.outputs['Color'], bg.inputs[0])
    bg.inputs[1].default_value = 1.0

    sun = bpy.data.objects.new('Sun', bpy.data.lights.new('Sun', 'SUN'))
    sun.data.energy = 4.2
    sun.data.color = (1.0, .93, .80)  # warm late-morning key light
    sun.data.angle = radians(3)
    # Light from the upper right; shadows fall up-left, into the sprite's headroom.
    sun.rotation_euler = (radians(42), 0, radians(160))
    sc.collection.objects.link(sun)

    cam = bpy.data.objects.new('Cam', bpy.data.cameras.new('Cam'))
    cam.data.type = 'ORTHO'
    cam.data.sensor_fit = 'HORIZONTAL'
    cam.rotation_euler = (radians(60), 0, radians(135))
    cam.data.clip_end = 400
    sc.collection.objects.link(cam); sc.camera = cam

    bpy.ops.mesh.primitive_plane_add(size=60)
    catcher = bpy.context.active_object
    catcher.name = 'Ground'
    catcher.is_shadow_catcher = True
    return sc, cam, catcher, build_material(), build_mask()

def node(nt, kind, **inputs):
    n = nt.nodes.new(kind)
    for k, v in inputs.items():
        if k.startswith('_'): setattr(n, k[1:], v)
        else: n.inputs[k].default_value = v
    return n

def build_material():
    mat = bpy.data.materials.new('Miniature'); mat.use_nodes = True
    nt = mat.node_tree; L = nt.links
    bsdf = nt.nodes['Principled BSDF']
    col = node(nt, 'ShaderNodeAttribute', _attribute_name='Col')
    prop = node(nt, 'ShaderNodeAttribute', _attribute_name='Mat')
    sep = node(nt, 'ShaderNodeSeparateColor'); L.new(prop.outputs['Color'], sep.inputs[0])
    pos = node(nt, 'ShaderNodeNewGeometry')
    # Broad blotchy weathering.
    blot = node(nt, 'ShaderNodeTexNoise', Scale=3.5, Detail=6.0, Roughness=.62)
    L.new(pos.outputs['Position'], blot.inputs['Vector'])
    blot_r = node(nt, 'ShaderNodeMapRange', **{'From Min': .3, 'From Max': .7, 'To Min': .80, 'To Max': 1.04})
    L.new(blot.outputs['Fac'], blot_r.inputs['Value'])
    # Vertical rain streaks: noise squashed along z.
    smap = node(nt, 'ShaderNodeMapping'); smap.inputs['Scale'].default_value = (55, 55, 2.2)
    L.new(pos.outputs['Position'], smap.inputs['Vector'])
    streak = node(nt, 'ShaderNodeTexNoise', Scale=1.0, Detail=2.0)
    L.new(smap.outputs['Vector'], streak.inputs['Vector'])
    streak_r = node(nt, 'ShaderNodeMapRange', **{'From Min': .35, 'From Max': .65, 'To Min': .86, 'To Max': 1.0})
    L.new(streak.outputs['Fac'], streak_r.inputs['Value'])
    # Fine surface grain.
    grain = node(nt, 'ShaderNodeTexNoise', Scale=260.0, Detail=1.0)
    L.new(pos.outputs['Position'], grain.inputs['Vector'])
    grain_r = node(nt, 'ShaderNodeMapRange', **{'To Min': .9, 'To Max': 1.06})
    L.new(grain.outputs['Fac'], grain_r.inputs['Value'])
    # Grime gathers in corners and recesses.
    ao = node(nt, 'ShaderNodeAmbientOcclusion', Distance=.12, _samples=8)
    L.new(col.outputs['Color'], ao.inputs['Color'])
    ao_r = node(nt, 'ShaderNodeMapRange', **{'To Min': .55, 'To Max': 1.0})
    L.new(ao.outputs['AO'], ao_r.inputs['Value'])
    m1 = node(nt, 'ShaderNodeMath', _operation='MULTIPLY'); L.new(blot_r.outputs[0], m1.inputs[0]); L.new(streak_r.outputs[0], m1.inputs[1])
    m2 = node(nt, 'ShaderNodeMath', _operation='MULTIPLY'); L.new(m1.outputs[0], m2.inputs[0]); L.new(grain_r.outputs[0], m2.inputs[1])
    m3 = node(nt, 'ShaderNodeMath', _operation='MULTIPLY'); L.new(m2.outputs[0], m3.inputs[0]); L.new(ao_r.outputs[0], m3.inputs[1])
    # Glass is kept clean: weathering only on opaque materials.
    glassmix = node(nt, 'ShaderNodeMix', _data_type='FLOAT')
    L.new(sep.outputs[2], glassmix.inputs['Factor']); L.new(m3.outputs[0], glassmix.inputs['A']); glassmix.inputs['B'].default_value = 1.0
    tint = node(nt, 'ShaderNodeMix', _data_type='RGBA', _blend_type='MULTIPLY', Factor=1.0)
    L.new(col.outputs['Color'], tint.inputs['A'])
    comb = node(nt, 'ShaderNodeCombineColor')
    for i in range(3): L.new(glassmix.outputs[0], comb.inputs[i])
    L.new(comb.outputs[0], tint.inputs['B'])
    L.new(tint.outputs['Result'], bsdf.inputs['Base Color'])
    L.new(sep.outputs[0], bsdf.inputs['Roughness'])
    L.new(sep.outputs[1], bsdf.inputs['Metallic'])
    return mat

def build_mask():
    mat = bpy.data.materials.new('TeamMask'); mat.use_nodes = True
    nt = mat.node_tree
    for n in list(nt.nodes):
        if n.type != 'OUTPUT_MATERIAL': nt.nodes.remove(n)
    team = node(nt, 'ShaderNodeAttribute', _attribute_name='Team')
    em = node(nt, 'ShaderNodeEmission', Strength=1.0)
    nt.links.new(team.outputs['Color'], em.inputs['Color'])
    nt.links.new(em.outputs[0], nt.nodes['Material Output'].inputs['Surface'])
    return mat

# ---------------------------------------------------------------------------------------------
# Material classes of hd_kit.py. Each is a Principled shader whose base colour comes from the
# face's `Col` attribute, weathered procedurally in object space (so it turns with the model).
class Graph:
    def __init__(self, mat):
        self.nt = mat.node_tree; self.L = self.nt.links
        self.bsdf = self.nt.nodes['Principled BSDF']
    def put(self, sock, v):
        if isinstance(v, bpy.types.NodeSocket): self.L.new(v, sock)
        elif v is not None: sock.default_value = v
    def N(self, kind, **kw):
        n = self.nt.nodes.new(kind)
        for k, v in kw.items():
            if k.startswith('_'): setattr(n, k[1:], v)
            else: self.put(n.inputs[k], v)
        return n
    def math(self, op, a, b=None, clamp=False):
        n = self.N('ShaderNodeMath', _operation=op); n.use_clamp = clamp
        self.put(n.inputs[0], a)
        if b is not None: self.put(n.inputs[1], b)
        return n.outputs[0]
    def mix(self, fac, a, b, blend='MIX'):
        n = self.N('ShaderNodeMix', _data_type='RGBA', _blend_type=blend)
        self.put(n.inputs[0], fac); self.put(n.inputs[6], a); self.put(n.inputs[7], b)
        return n.outputs[2]
    def lerp(self, fac, a, b):
        n = self.N('ShaderNodeMix', _data_type='FLOAT')
        self.put(n.inputs[0], fac); self.put(n.inputs[2], a); self.put(n.inputs[3], b)
        return n.outputs[0]
    def ramp(self, v, lo, hi, out_lo=0.0, out_hi=1.0):
        n = self.N('ShaderNodeMapRange')
        self.put(n.inputs['Value'], v)
        n.inputs['From Min'].default_value = lo; n.inputs['From Max'].default_value = hi
        n.inputs['To Min'].default_value = out_lo; n.inputs['To Max'].default_value = out_hi
        return n.outputs[0]
    def noise(self, vec, scale, detail=3.0, rough=.55):
        n = self.N('ShaderNodeTexNoise', Scale=scale, Detail=detail, Roughness=rough)
        self.put(n.inputs['Vector'], vec)
        return n.outputs['Fac'] if 'Fac' in n.outputs else n.outputs[0]
    def blotch(self, vec, scale, offset, lo, hi, distortion=1.1, detail=1.2):
        """Hard-edged blotches (0 or 1) of a disruptive pattern: warped noise through a threshold."""
        mp = self.N('ShaderNodeMapping'); mp.inputs['Location'].default_value = offset
        self.put(mp.inputs['Vector'], vec)
        n = self.N('ShaderNodeTexNoise', Scale=scale, Detail=detail, Roughness=.5, Distortion=distortion)
        self.put(n.inputs['Vector'], mp.outputs[0])
        return self.ramp(n.outputs['Fac'] if 'Fac' in n.outputs else n.outputs[0], lo, hi)
    def scaled(self, vec, sx, sy, sz):
        n = self.N('ShaderNodeMapping'); n.inputs['Scale'].default_value = (sx, sy, sz)
        self.put(n.inputs['Vector'], vec)
        return n.outputs[0]
    def grey(self, v):
        n = self.N('ShaderNodeCombineColor')
        for k in range(3): self.put(n.inputs[k], v)
        return n.outputs[0]

DUST = (.36, .31, .24, 1)      # settles on upward faces
SOOT = (.035, .033, .03, 1)    # collects in recesses
BARE = (.11, .11, .115, 1)     # steel under chipped paint
RUSTC = (.19, .075, .03, 1)

#        rough metal grime dust chips streak var  bump  texture
RECIPES = {
    'paint':    (.50, .10, .65, .32, .60, .30, .17, .15, None),
    'team':     (.46, .10, .35, .16, .30, .12, .06, .10, None),
    'steel':    (.36, .90, .45, .20, .0,  .25, .10, .10, None),
    'dark':     (.46, .75, .30, .30, .25, .10, .08, .10, None),
    'copper':   (.32, 1.0, .35, .10, .0,  .20, .10, .05, None),
    'rust':     (.80, .35, .50, .15, .0,  .50, .30, .50, 'rust'),
    'rubber':   (.86, .0,  .30, .55, .0,  .0,  .10, .40, None),
    'concrete': (.90, .0,  .75, .25, .0,  .60, .14, .45, 'concrete'),
    'brick':    (.88, .0,  .60, .15, .0,  .40, .10, .60, 'brick'),
    'corr':     (.48, .35, .60, .20, .35, .55, .10, .90, 'corr'),
    'roof':     (.92, .0,  .50, .35, .0,  .20, .16, .50, 'speckle'),
    'asphalt':  (.90, .0,  .40, .30, .0,  .10, .14, .40, 'speckle'),
    'cloth':    (.92, .0,  .50, .30, .0,  .10, .14, .50, None),
    'skin':     (.62, .0,  .25, .0,  .0,  .0,  .05, .0,  None),
    'wood':     (.80, .0,  .55, .20, .0,  .20, .16, .40, 'wood'),
    'foliage':  (.72, .0,  .55, .0,  .0,  .0,  .30, .60, None),
    'hazard':   (.55, .05, .50, .25, .40, .25, .08, .10, 'hazard'),
    # Field equipment: matt disruptive-pattern paint, worn harder than anything parked at a base.
    'camo':      (.62, .06, .85, .42, .85, .45, .10, .22, 'camo'),
    'camocloth': (.95, .0,  .60, .22, .0,  .0,  .08, .55, 'camocloth'),
    'leather':   (.58, .0,  .45, .0,  .0,  .0,  .12, .30, None),
}

def build_class_material(name):
    mat = bpy.data.materials.new('K_' + name); mat.use_nodes = True
    g_ = Graph(mat); b = g_.bsdf
    col = g_.N('ShaderNodeAttribute', _attribute_name='Col').outputs['Color']
    obj = g_.N('ShaderNodeTexCoord').outputs['Object']
    if name == 'holdout':
        # A doorway cut out of the sprite: alpha 0 wherever this surface is the nearest one.
        out = next(n for n in mat.node_tree.nodes if n.type == 'OUTPUT_MATERIAL')
        mat.node_tree.links.new(mat.node_tree.nodes.new('ShaderNodeHoldout').outputs[0], out.inputs['Surface'])
        return mat
    if name == 'emit':
        g_.put(b.inputs['Base Color'], col); g_.put(b.inputs['Emission Color'], col)
        # Bright enough to read as lit, not so bright that the colour clips to white.
        b.inputs['Emission Strength'].default_value = 2.2; b.inputs['Roughness'].default_value = .3
        return mat
    if name in ('glass', 'water'):
        # Dark glazing that mirrors the sky; panes differ a little, as real curtain walls do.
        cell = g_.N('ShaderNodeTexVoronoi', Scale=9.0 if name == 'glass' else 3.0)
        g_.put(cell.inputs['Vector'], obj)
        vary = g_.ramp(cell.outputs['Color'], 0.0, 1.0, .75, 1.3)
        g_.put(b.inputs['Base Color'], g_.mix(1.0, col, g_.grey(vary), 'MULTIPLY'))
        b.inputs['Roughness'].default_value = .05 if name == 'glass' else .02
        b.inputs['Metallic'].default_value = .0
        b.inputs['Specular IOR Level'].default_value = 1.0
        b.inputs['Coat Weight'].default_value = 1.0; b.inputs['Coat Roughness'].default_value = .03
        if name == 'water':
            g_.put(b.inputs['Emission Color'], col); b.inputs['Emission Strength'].default_value = .5
        return mat
    rough, metal, grime, dust, chips, streak, var, bump, tex = RECIPES[name]
    geo = g_.N('ShaderNodeNewGeometry')
    nz = g_.N('ShaderNodeSeparateXYZ'); g_.put(nz.inputs[0], geo.outputs['Normal'])
    up = g_.ramp(nz.outputs['Z'], .35, .9)
    sep = g_.N('ShaderNodeSeparateXYZ'); g_.put(sep.inputs[0], obj)
    along = g_.math('ADD', sep.outputs['X'], sep.outputs['Y'])  # runs along both wall directions
    height = None

    base = col
    if tex == 'brick':
        uv = g_.N('ShaderNodeCombineXYZ'); g_.put(uv.inputs[0], along); g_.put(uv.inputs[1], sep.outputs['Z'])
        br = g_.N('ShaderNodeTexBrick', Scale=34.0, **{'Mortar Size': .02, 'Brick Width': .62, 'Row Height': .26})
        br.offset = .5
        g_.put(br.inputs['Vector'], uv.outputs[0])
        g_.put(br.inputs['Color1'], g_.mix(1.0, col, (.82, .80, .78, 1), 'MULTIPLY'))
        g_.put(br.inputs['Color2'], g_.mix(1.0, col, (1.22, 1.12, 1.05, 1), 'MULTIPLY'))
        br.inputs['Mortar'].default_value = (.42, .40, .36, 1)
        base = br.outputs['Color']; height = g_.math('SUBTRACT', 1.0, br.outputs[1])
    elif tex == 'corr':
        # Ribs along the sheet: vertical on walls, along one axis on roofs.
        u = g_.lerp(up, along, sep.outputs['X'])
        height = g_.math('SINE', g_.math('MULTIPLY', u, 150.0))
        base = g_.mix(g_.ramp(height, -1, 1, .0, .16), col, (0, 0, 0, 1))
    elif tex == 'concrete':
        uv = g_.N('ShaderNodeCombineXYZ'); g_.put(uv.inputs[0], along); g_.put(uv.inputs[1], sep.outputs['Z'])
        br = g_.N('ShaderNodeTexBrick', Scale=5.5, **{'Mortar Size': .012, 'Brick Width': .9, 'Row Height': .5})
        g_.put(br.inputs['Vector'], uv.outputs[0])
        seam = g_.math('MULTIPLY', br.outputs[1], g_.math('SUBTRACT', 1.0, up))
        base = g_.mix(g_.math('MULTIPLY', seam, .35), col, (.05, .05, .05, 1))
        height = g_.math('SUBTRACT', g_.noise(obj, 60.0, 4.0), seam)
    elif tex == 'speckle':
        sp = g_.noise(obj, 320.0, 2.0, .7)
        base = g_.mix(1.0, col, g_.grey(g_.ramp(sp, .3, .7, .72, 1.28)), 'MULTIPLY'); height = sp
    elif tex == 'wood':
        gr = g_.noise(g_.scaled(obj, 6, 90, 90), 1.0, 3.0)
        base = g_.mix(1.0, col, g_.grey(g_.ramp(gr, .3, .7, .72, 1.2)), 'MULTIPLY'); height = gr
    elif tex == 'hazard':
        d = g_.math('ADD', along, g_.math('MULTIPLY', sep.outputs['Z'], 1.0))
        d = g_.lerp(up, d, g_.math('SUBTRACT', sep.outputs['X'], sep.outputs['Y']))
        band = g_.math('GREATER_THAN', g_.math('SINE', g_.math('MULTIPLY', d, 62.0)), 0.0)
        base = g_.mix(band, col, (.02, .02, .02, 1))
    elif tex == 'camo':
        # Three-colour disruptive paint: the authored colour, earth brown and near-black, in the
        # metre-wide sprayed blotches of NATO and Soviet vehicle schemes.
        brown = g_.mix(1.0, col, (.52, .40, .30, 1), 'MULTIPLY')
        black = g_.mix(1.0, col, (.16, .17, .16, 1), 'MULTIPLY')
        base = g_.mix(g_.blotch(obj, 4.6, (3.1, 7.7, 1.3), .535, .55), col, brown)
        base = g_.mix(g_.blotch(obj, 5.4, (9.4, 2.2, 6.8), .585, .60), base, black)
    elif tex == 'camocloth':
        # Printed uniform cloth: four tones in hand-sized shapes, plus the weave.
        dark = g_.mix(1.0, col, (.50, .46, .38, 1), 'MULTIPLY')
        deep = g_.mix(1.0, col, (.22, .24, .20, 1), 'MULTIPLY')
        pale = g_.mix(1.0, col, (1.32, 1.28, 1.16, 1), 'MULTIPLY')
        base = g_.mix(g_.blotch(obj, 21.0, (3.1, 7.7, 1.3), .52, .54), col, dark)
        base = g_.mix(g_.blotch(obj, 27.0, (5.9, 1.4, 8.2), .57, .59), base, pale)
        base = g_.mix(g_.blotch(obj, 34.0, (9.4, 2.2, 6.8), .60, .62), base, deep)
        height = g_.noise(obj, 520.0, 1.0)
    elif tex == 'rust':
        rn = g_.noise(obj, 26.0, 5.0, .7)
        base = g_.mix(g_.ramp(rn, .35, .7), col, RUSTC); height = rn

    # Broad tonal variation, fine grain.
    blot = g_.noise(obj, 4.2, 5.0, .62)
    base = g_.mix(1.0, base, g_.grey(g_.ramp(blot, .28, .72, 1 - var, 1 + var * .5)), 'MULTIPLY')
    grain = g_.noise(obj, 240.0, 1.0)
    base = g_.mix(1.0, base, g_.grey(g_.ramp(grain, 0, 1, .93, 1.05)), 'MULTIPLY')
    # Rain streaks down the walls.
    if streak:
        st = g_.noise(g_.scaled(obj, 46, 46, 1.8), 1.0, 2.0)
        wall = g_.math('SUBTRACT', 1.0, up)
        s_amt = g_.math('MULTIPLY', g_.ramp(st, .48, .72), g_.math('MULTIPLY', wall, streak))
        base = g_.mix(s_amt, base, RUSTC if name in ('corr', 'steel', 'rust') else SOOT)
    # Soot and grime where surfaces meet.
    ao = g_.N('ShaderNodeAmbientOcclusion', Distance=.10, _samples=8)
    occ = g_.math('MULTIPLY', g_.math('SUBTRACT', 1.0, g_.ramp(ao.outputs['AO'], .35, 1.0)), grime, clamp=True)
    base = g_.mix(occ, base, SOOT)
    # Dust on what faces the sky.
    if dust:
        dn = g_.noise(obj, 9.0, 4.0, .6)
        base = g_.mix(g_.math('MULTIPLY', g_.math('MULTIPLY', up, g_.ramp(dn, .3, .75)), dust), base, DUST)
        # ...and dirt thrown up from the ground onto whatever is near it.
        low = g_.math('MULTIPLY', g_.ramp(sep.outputs['Z'], .0, .11, 1.0, .0), g_.ramp(dn, .15, .7, .45, 1.0))
        base = g_.mix(g_.math('MULTIPLY', low, min(1.0, dust * 1.7), clamp=True), base, DUST)
    metallic, roughness = metal, rough
    if chips:
        # Paint wears through to steel along exposed edges.
        bev = g_.N('ShaderNodeBevel', Radius=.011, _samples=4)
        dot = g_.N('ShaderNodeVectorMath', _operation='DOT_PRODUCT')
        g_.put(dot.inputs[0], bev.outputs[0]); g_.put(dot.inputs[1], geo.outputs['Normal'])
        edge = g_.ramp(g_.math('SUBTRACT', 1.0, dot.outputs['Value']), .012, .12)
        cn = g_.noise(obj, 70.0, 3.0, .7)
        chip = g_.math('MULTIPLY', g_.math('MULTIPLY', edge, g_.ramp(cn, .42, .58)), chips, clamp=True)
        base = g_.mix(chip, base, BARE)
        metallic = g_.lerp(chip, metal, .9); roughness = g_.lerp(chip, rough, .38)
    # Surfaces are never evenly glossy.
    roughness = g_.math('ADD', roughness, g_.ramp(blot, .2, .8, -.07, .09), clamp=True)
    g_.put(b.inputs['Base Color'], base)
    g_.put(b.inputs['Metallic'], metallic); g_.put(b.inputs['Roughness'], roughness)
    if name == 'skin':
        b.inputs['Subsurface Weight'].default_value = .25; b.inputs['Subsurface Scale'].default_value = .01
    if bump:
        h = height if height is not None else g_.noise(obj, 120.0, 3.0)
        bn = g_.N('ShaderNodeBump', Strength=bump, Distance=.004)
        g_.put(bn.inputs['Height'], h); g_.put(b.inputs['Normal'], bn.outputs[0])
    return mat

CLASS_NAMES = list(RECIPES) + ['glass', 'water', 'emit', 'holdout']

def build_materials():
    return {name: build_class_material(name) for name in CLASS_NAMES}

def build_mesh(m, mat, kmats):
    bm = bmesh.new()
    col = bm.loops.layers.float_color.new('Col')
    prop = bm.loops.layers.float_color.new('Mat')
    team = bm.loops.layers.float_color.new('Team')
    slots = {None: 0}          # material class -> slot; slot 0 is the legacy terrain material
    cache = {}
    for prim, pts, c in m.faces:
        kind = getattr(c, 'mat', None)
        if kind is None:
            base, cls = classify(c)
            rough, metal = PROPS[cls]
            mp = (rough, metal, 1.0 if cls == GLASS else 0.0, 1)
            is_team = cls == TEAM
        else:
            base, mp, is_team = tuple(c), (0, 0, 0, 1), kind == 'team'
        lin = (*[to_linear(v) for v in base], 1)
        tm = (1.0, 1.0, 1.0, 1) if is_team else (0, 0, 0, 1)
        vs = []
        for p in pts:
            x, y, z = m.rotate(p)
            # Swap x/y: the sprite projection is mirrored relative to Blender's right-handed axes.
            key = (prim, round(y, 5), round(x, 5), round(z, 4))
            if key not in cache:
                cache[key] = bm.verts.new((y, x, z / Z_PX))
            v = cache[key]
            if not vs or vs[-1] is not v: vs.append(v)
        if len(vs) > 1 and vs[0] is vs[-1]: vs.pop()
        if len(set(vs)) < 3 or len(set(vs)) != len(vs): continue
        try:
            f = bm.faces.new(vs)
        except ValueError:  # coincident duplicate face
            continue
        f.smooth = m.meta[prim] if isinstance(prim, int) else False
        f.material_index = slots.setdefault(kind, len(slots))
        for loop in f.loops:
            loop[col] = lin; loop[prop] = mp; loop[team] = tm
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    me = bpy.data.meshes.new('M'); bm.to_mesh(me); bm.free()
    me.set_sharp_from_angle(angle=radians(35))
    ob = bpy.data.objects.new('M', me)
    for kind in sorted(slots, key=slots.get):
        ob.data.materials.append(mat if kind is None else kmats[kind])
    bev = ob.modifiers.new('Bevel', 'BEVEL')
    bev.width = .006; bev.segments = 2; bev.limit_method = 'ANGLE'; bev.angle_limit = radians(50)
    bev.harden_normals = False
    bpy.context.scene.collection.objects.link(ob)
    return ob

def frame(cam, sc, w, h, origin):
    """Match g.Model.project: pixel (o + (x-y)*32, o + (x+y)*16 - z) for model point (x,y,z)."""
    sc.render.resolution_x, sc.render.resolution_y = w * g.S, h * g.S
    sc.render.resolution_percentage = 100
    cam.data.ortho_scale = w / CELL_PX
    a = (w / 2 - origin[0]) / 32; b = (h / 2 - origin[1]) / 16
    mx, my = (a + b) / 2, (b - a) / 2   # model ground point under the image centre
    tx, ty = my, mx                     # Blender axes (swapped)
    d = 80
    el, az = radians(30), radians(45)
    cam.location = (tx + d * cos(el) * cos(az), ty + d * cos(el) * sin(az), d * sin(el))

def shoot(sc, catcher, mask, out, team_mask=True, shadow=True):
    sc.render.filepath = str(out) + '_color.png'
    # A tall top's shadow would be cut off at its slot's edge; it is drawn without one.
    catcher.hide_render = not shadow
    bpy.ops.render.render(write_still=True)
    catcher.hide_render = False
    if not team_mask: return
    # Team mask: flat emission, no catcher, minimal sampling.
    catcher.hide_render = True
    vl = sc.view_layers[0]; vl.material_override = mask
    samples, den = sc.cycles.samples, sc.cycles.use_denoising
    sc.cycles.samples, sc.cycles.use_denoising = 16, False
    sc.render.filepath = str(out) + '_mask.png'
    bpy.ops.render.render(write_still=True)
    sc.cycles.samples, sc.cycles.use_denoising = samples, den
    vl.material_override = None; catcher.hide_render = False

def drop(ob):
    me = ob.data; bpy.data.objects.remove(ob); bpy.data.meshes.remove(me)

def render(sc, cam, catcher, mats, mask, m, out, team_mask=True, shadow=True):
    ob = build_mesh(m, *mats)
    frame(cam, sc, m.w, m.h, m.o)
    shoot(sc, catcher, mask, out, team_mask, shadow)
    drop(ob)

def render_unit(sc, cam, catcher, mats, mask, ident, out, shadow, facings=range(8)):
    """One mesh per unit, turned on the spot for its eight facings (object-space shaders turn
    with it). A facing of angle a in the sprite projection is a turn of -a about Blender's z."""
    m = g.unit(ident, 0)
    ob = build_mesh(m, *mats)
    frame(cam, sc, m.w, m.h, m.o)
    for facing in facings:
        ob.rotation_euler = (0, 0, -facing * pi / 4)
        shoot(sc, catcher, mask, Path(str(out) + f'_{facing}'), True, shadow)
    drop(ob)

def render_walk(sc, cam, catcher, mats, mask, ident, out, facings=range(8)):
    """The walk cycle of an infantry unit: one mesh per frame of the cycle, eight facings each."""
    for pose in range(g.WALK_FRAMES):
        m = g.unit_walk(ident, pose)
        ob = build_mesh(m, *mats)
        frame(cam, sc, m.w, m.h, m.o)
        for facing in facings:
            ob.rotation_euler = (0, 0, -facing * pi / 4)
            shoot(sc, catcher, mask, Path(str(out) + f'_{pose}_{facing}'), True, True)
        drop(ob)

def field_light(sc, on):
    """Units are lit harder than buildings: a stronger sun against a dimmer sky, so that form,
    camouflage and wear read at the few dozen pixels a vehicle gets."""
    bpy.data.objects['Sun'].data.energy = 5.6 if on else 4.2
    sc.world.node_tree.nodes['Background'].inputs[1].default_value = .62 if on else 1.0

def roster():
    economy = (ROOT / 'assets/data/economy.ron').read_text().split('    units:')[0]
    blds = [(i, int(w), int(h)) for i, w, h in re.findall(r'id: "([^"]+)".*?width: (\d+),\s*height: (\d+)', economy, re.S)]
    units = re.findall(r'id: "([^"]+)"', (ROOT / 'assets/data/units.ron').read_text())
    # The game's own Super Me buildings (docs/DECISIONS.md R220) have their own data file.
    super_me = (ROOT / 'assets/data/super_me.ron').read_text().split('    buildings:')[1]
    blds += [(i, int(w), int(h)) for i, w, h in re.findall(r'id: "([^"]+)".*?width: (\d+),\s*height: (\d+)', super_me, re.S)]
    return blds, units

def airborne():
    """Units of the air domain. They are drawn high above the ground with a separate shadow
    sprite far below them (crates/game/src/units.rs), so no shadow is baked under the model."""
    arsenal = (ROOT / 'assets/data/arsenal.ron').read_text()
    return set(re.findall(r'id: "([^"]+)"[^\n]*domain: "air"', arsenal))

def main():
    argv = sys.argv[sys.argv.index('--') + 1:] if '--' in sys.argv else []
    out = Path(argv[0]) if argv else Path.home() / 'Library/Caches/Codex/reviews/MyGame001/ra2-visual/render'
    only = set(a for a in argv[1:] if ':' in a and '=' not in a)
    # since=<unix time>: resume an interrupted run, skipping sprites already rendered after it.
    since = next((float(a[6:]) for a in argv[1:] if a.startswith('since=')), None)
    done = lambda path: since is not None and path.exists() and path.stat().st_mtime > since
    facings = [int(a[1:]) for a in argv[1:] if a.startswith('f')] or range(8)  # f0 f3: those facings only
    g.Model = terrain.Model = BlenderModel  # terrain star-imported its own reference
    sc, cam, catcher, mat, mask = setup_scene()
    mats = (mat, build_materials())
    blds, units = roster()
    air = airborne()
    for d in ['buildings', 'units', 'walk', 'terrain']: (out / d).mkdir(parents=True, exist_ok=True)
    for ident, w, d in blds:
        if only and f'b:{ident}' not in only: continue
        if done(out / 'buildings' / f'{ident}_mask.png'): continue
        # Wall segments stand cell against cell: a baked shadow would fall across the neighbour.
        render(sc, cam, catcher, mats, mask, g.building(ident, w, d), out / 'buildings' / ident,
               shadow=ident != 'fortress_walls')
        # The frames of a vehicle door opening (hd_buildings.DOOR_FRAMES): the same model with
        # its door rolled further up each time.
        for n, open_ in enumerate(hd_buildings.DOOR_FRAMES.get(ident, ()), 1):
            hd_buildings.DOOR = open_
            try:
                render(sc, cam, catcher, mats, mask, g.building(ident, w, d), out / 'buildings' / f'{ident}_door{n}')
            finally:
                hd_buildings.DOOR = 0.0
    field_light(sc, True)
    for ident in units:
        if only and f'u:{ident}' not in only: continue
        if done(out / 'units' / f'{ident}_7_mask.png'): continue
        render_unit(sc, cam, catcher, mats, mask, ident, out / 'units' / ident, ident not in air, facings)
    # Walk cycles (w:ident): every infantry unit that goes on foot.
    for ident in g.walkers():
        # A walker the roster no longer names (the legacy fixtures' art) renders when asked for.
        if (ident not in units and f'w:{ident}' not in only) or (only and f'w:{ident}' not in only): continue
        if done(out / 'walk' / f'{ident}_{g.WALK_FRAMES - 1}_7_mask.png'): continue
        render_walk(sc, cam, catcher, mats, mask, ident, out / 'walk' / ident, facings)
    field_light(sc, False)
    for kind in terrain.TOP_KINDS:
        if f't:{kind}' not in only: continue   # terrain tops only on request
        for seed in range(16):
            render(sc, cam, catcher, mats, mask, terrain.top(kind, seed), out / 'terrain' / f'{kind}_{seed}', team_mask=False)
    for kind in terrain.TALL_KINDS:
        if f't:{kind}' not in only: continue
        for seed in range(8):
            render(sc, cam, catcher, mats, mask, terrain.tall(kind, seed), out / 'terrain' / f'{kind}_{seed}',
                   team_mask=False, shadow=False)
    print('RENDER COMPLETE', out)

main()
