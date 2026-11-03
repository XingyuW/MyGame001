"""Compose the render_map_art.py passes into the map-scenery atlases and ambient sheets.

  uv run --with pillow --with numpy python scripts/finish_map_art.py RENDER_DIR [OUT_DIR] [--debug DIR]

Writes (OUT_DIR defaults to assets/sprites/hd):
  terrain/26.png            four 256 x 1024 slots (64 x 256 world px), slot i at x = 256 i
  terrain/27-33.png         sixteen 256 x 256 slots, slot s at ((s % 4) 256, (s // 4) 256)
  terrain/34.png, 35.png    eight 256 x 512 slots, slot s at ((s % 4) 256, (s // 4) 512)
  ambient/units/<id>.png    3072 x 384: eight 384-texel facings, as assets/sprites/hd/units
  ambient/walk/<id>.png     2048 x 2048: columns = eight facings, rows = eight frames, as walk/

It never touches an existing sprite: only the files above, and only when every pass of a sheet is
present (a partial render never replaces a finished sheet). Nothing here is team paint, so every
pixel that would fall inside either team-paint rule (blue ramp b > r+60 and g > r+35; red-dominant
r > g+65 and r > b+70) is nudged just outside it, and `check()` verifies the written files.
--debug DIR also writes each atlas over a diamond grid for review.
"""
import sys
from pathlib import Path
import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parent))
from finish_hd_art import develop   # the units' darkroom step, shared with the roster's sheets

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'assets/sprites/hd'
S = 4
SMALL = (27, 28, 29, 30, 31, 32, 33)
TALL = (34, 35)
UNIT_IDS = ['car_sedan', 'car_compact', 'taxi', 'pickup', 'bus', 'truck', 'tractor', 'sailboat', 'fishing_boat', 'ferry']
WALK_IDS = ['civ_man', 'civ_woman', 'civ_worker', 'cow', 'sheep', 'deer', 'horse', 'camel', 'penguin', 'seagull']
MAX_BYTES = 4 * 1024 * 1024

def neutral(im):
    """Move every pixel out of both team-paint rules (the minimum change, as finish_hd_art does)."""
    c = np.asarray(im.convert('RGBA')).astype(np.int32)
    r, g, b, a = c[..., 0], c[..., 1], c[..., 2], c[..., 3]
    blue = (b > r + 60) & (g > r + 35)
    r = np.where(blue, np.maximum(r, np.minimum(b - 60, g - 35)), r)
    red = (r > g + 65) & (r > b + 70)
    g = np.where(red, np.maximum(g, r - 65), g)
    return Image.fromarray(np.stack([r, g, b, a], -1).clip(0, 255).astype(np.uint8), 'RGBA')

def team_pixels(path):
    c = np.asarray(Image.open(path).convert('RGBA')).astype(np.int32)
    r, g, b, a = c[..., 0], c[..., 1], c[..., 2], c[..., 3]
    hit = (a > 0) & (((b > r + 60) & (g > r + 35)) | ((r > g + 65) & (r > b + 70)))
    return int(hit.sum())

def slot_count(kind):
    return 4 if kind == 26 else 8 if kind in TALL else 16

def slot_box(kind, s):
    if kind == 26: return (256 * s, 0)
    if kind in TALL: return ((s % 4) * 256, (s // 4) * 512)
    return ((s % 4) * 256, (s // 4) * 256)

def atlas(src, kind):
    im = Image.new('RGBA', (1024, 1024))
    for s in range(slot_count(kind)):
        p = src / 'terrain' / f'{kind}_{s}_color.png'
        if not p.exists():
            if kind == 35 and s >= 6: continue      # the two spare slots of 35 stay empty
            return None
        im.paste(neutral(Image.open(p)), slot_box(kind, s))
    return im

def save(im, path):
    path.parent.mkdir(parents=True, exist_ok=True)
    im.save(path, optimize=True)
    if path.stat().st_size >= MAX_BYTES: raise SystemExit(f'{path} is {path.stat().st_size} bytes (limit 4 MiB)')

def debug_sheet(im, kind, path):
    """The atlas over a pale ground with each slot's cell diamond filled and outlined."""
    bg = Image.new('RGBA', im.size, (164, 164, 156, 255)); d = ImageDraw.Draw(bg)
    over = Image.new('RGBA', im.size); od = ImageDraw.Draw(over)
    for s in range(slot_count(kind)):
        x0, y0 = slot_box(kind, s)
        top = 208 if kind == 26 else 80 if kind in TALL else 16
        dia = [(x0 + 32 * S, y0 + top * S), (x0 + 64 * S, y0 + (top + 16) * S), (x0 + 32 * S, y0 + (top + 32) * S), (x0, y0 + (top + 16) * S)]
        d.polygon(dia, fill=(98, 112, 70, 255) if kind not in (33,) or s >= 8 else (52, 88, 96, 255))
        od.line(dia + [dia[0]], fill=(255, 255, 0, 150), width=2)
        w, h = (256, 1024) if kind == 26 else (256, 512) if kind in TALL else (256, 256)
        d.rectangle([x0, y0, x0 + w - 1, y0 + h - 1], outline=(90, 90, 90, 255))
    bg.alpha_composite(im); bg.alpha_composite(over)
    bg.save(path)

def sheet(frames, cols, rows):
    size = frames[0].size[0]
    out = Image.new('RGBA', (size * cols, size * rows))
    for i, im in enumerate(frames): out.paste(im, (size * (i % cols), size * (i // cols)))
    return out

def main():
    args = sys.argv[1:]
    debug = None
    if '--debug' in args:
        i = args.index('--debug'); debug = Path(args[i + 1]); del args[i:i + 2]
        debug.mkdir(parents=True, exist_ok=True)
    src = Path(args[0])
    out = Path(args[1]) if len(args) > 1 else OUT
    written = []
    for kind in (26,) + SMALL + TALL:
        im = atlas(src, kind)
        if im is None: continue
        path = out / 'terrain' / f'{kind}.png'
        save(im, path); written.append(path)
        if debug: debug_sheet(im, kind, debug / f'atlas_{kind}.png')
    for ident in UNIT_IDS:
        names = [src / 'units' / f'{ident}_{f}_color.png' for f in range(8)]
        if not all(p.exists() for p in names): continue
        path = out / 'ambient/units' / f'{ident}.png'
        save(sheet([neutral(develop(Image.open(p).convert('RGBA'))) for p in names], 8, 1), path); written.append(path)
    for ident in WALK_IDS:
        names = [src / 'walk' / f'{ident}_{p}_{f}_color.png' for p in range(8) for f in range(8)]
        if not all(p.exists() for p in names): continue
        path = out / 'ambient/walk' / f'{ident}.png'
        save(sheet([neutral(develop(Image.open(p).convert('RGBA'))) for p in names], 8, 8), path); written.append(path)
    check(written)

def check(paths):
    expect = {'terrain': (1024, 1024), 'units': (3072, 384), 'walk': (2048, 2048)}
    ok = True
    for p in paths:
        im = Image.open(p)
        want = expect[p.parent.name]
        n = team_pixels(p)
        size = p.stat().st_size
        good = im.size == want and im.mode == 'RGBA' and size < MAX_BYTES and n == 0
        ok &= good
        print(f'{"ok " if good else "BAD"} {p.relative_to(p.parents[2])}  {im.size[0]}x{im.size[1]}  {size / 1024:.0f} KiB  team-rule pixels {n}')
    print(f'{len(paths)} files written', 'all checks pass' if ok else 'CHECKS FAILED')
    if not ok: raise SystemExit(1)

if __name__ == '__main__':
    main()
