"""Render the map scenery and ambient life of hd_map_art.py / hd_ambient.py with the exact scene,
materials, camera and sun of render_hd_blender.py (whose functions are loaded without running its
own main(), so the shipped sprites' renderer is untouched).

  /Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
      --python scripts/render_map_art.py -- OUT_DIR [t:26 | t:26:3 | u:<id> | w:<id> ...] [resume]

  t:<kind>        every slot of terrain atlas <kind> (26-35); t:<kind>:<slot> one slot
  u:<id>          an ambient vehicle's eight-facing sheet (hd_ambient.SHEETS)
  w:<id>          an ambient walker's eight frames x eight facings (hd_ambient.WALK_IDS)
  all             everything; resume: skip passes already on disk

Raw colour passes land in OUT_DIR/{terrain,units,walk}; finish_map_art.py composes the sheets.
No team-mask pass is rendered: nothing here wears team paint.
"""
import sys
from math import pi
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
_src_path = HERE / 'render_hd_blender.py'
_src = _src_path.read_text()
_src = _src[:_src.rindex('\nmain()')]          # everything but the module-level main() call
R = {'__name__': 'render_hd_blender_lib', '__file__': str(_src_path)}
exec(compile(_src, str(_src_path), 'exec'), R)

import generate_hd_art as g
import generate_hd_terrain as terrain
import hd_infantry as inf
import hd_map_art as H
import hd_ambient as A

def color_path(base):
    return Path(str(base) + '_color.png')

def main():
    argv = sys.argv[sys.argv.index('--') + 1:] if '--' in sys.argv else []
    out = Path(argv[0])
    items = argv[1:]
    resume = 'resume' in items
    if 'all' in items:
        items = [f't:{k}' for k in H.KINDS] + [f'u:{i}' for i in A.SHEETS] + [f'w:{i}' for i in A.WALK_IDS]
    g.Model = terrain.Model = R['BlenderModel']
    sc, cam, catcher, mat, mask = R['setup_scene']()
    mats = (mat, R['build_materials']())
    for d in ('terrain', 'units', 'walk'): (out / d).mkdir(parents=True, exist_ok=True)
    skip = lambda base: resume and color_path(base).exists()
    for item in items:
        if item.startswith('t:'):
            parts = item.split(':')
            kind = int(parts[1])
            seeds = [int(parts[2])] if len(parts) > 2 else range(H.KINDS[kind])
            for seed in seeds:
                base = out / 'terrain' / f'{kind}_{seed}'
                if skip(base): continue
                m = H.model(kind, seed)
                if not m.faces:   # an empty slot (35: 6-7)
                    continue
                R['render'](sc, cam, catcher, mats, mask, m, base, team_mask=False, shadow=kind not in H.NO_SHADOW)
        elif item.startswith('u:'):
            ident = item[2:]
            base = out / 'units' / ident
            if skip(Path(str(base) + '_7')): continue
            R['field_light'](sc, True)
            m = g.Model(96, 96, (48, 70), 0)
            A.vehicle(m, ident)
            ob = R['build_mesh'](m, *mats)
            R['frame'](cam, sc, m.w, m.h, m.o)
            for facing in range(8):
                ob.rotation_euler = (0, 0, -facing * pi / 4)
                R['shoot'](sc, catcher, mask, Path(str(base) + f'_{facing}'), False, True)
            R['drop'](ob)
            R['field_light'](sc, False)
        elif item.startswith('w:'):
            ident = item[2:]
            R['field_light'](sc, True)
            for pose in range(g.WALK_FRAMES):
                if skip(out / 'walk' / f'{ident}_{pose}_7'): continue
                inf.POSE = pose
                try:
                    m = g.Model(64, 64, (32, 46), 0)
                    A.walker(m, ident)
                finally:
                    inf.POSE = None
                ob = R['build_mesh'](m, *mats)
                R['frame'](cam, sc, m.w, m.h, m.o)
                for facing in range(8):
                    ob.rotation_euler = (0, 0, -facing * pi / 4)
                    # A flying gull carries no ground shadow (the game may draw one far below it).
                    R['shoot'](sc, catcher, mask, out / 'walk' / f'{ident}_{pose}_{facing}', False, ident not in A.FLYERS)
                R['drop'](ob)
            R['field_light'](sc, False)
    print('MAP ART RENDER COMPLETE', out)

main()
