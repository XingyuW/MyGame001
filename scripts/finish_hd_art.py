"""Compose Blender passes (render_hd_blender.py) into the shipped HD sprites.

  uv run --with pillow --with numpy python scripts/finish_hd_art.py [RENDER_DIR [OUT_DIR]]

Only units with all eight facings in RENDER_DIR are written, so a partial test render (f0 f3)
never replaces a shipped sheet with a broken one; likewise a walk cycle (walk/<ident>.png: six
rows of eight 256-texel frames) needs all of its 48 frames.

Team paint: presentation.rs recolours pixels on the authored blue ramp
(b > r+60 and g > r+35, brightness carried by r - 42) and, for the human colour pass,
red-dominant pixels. Lit renders cannot guarantee either, so the mask pass decides ownership:
masked pixels are re-encoded onto the exact ramp with their rendered luminance, and neutral
pixels that would accidentally match either rule are nudged just outside it.
"""
import sys
from pathlib import Path
import numpy as np
from PIL import Image, ImageFilter

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'assets/sprites/hd'
S = 4
TEAM_BASE = np.array([42, 133, 202], dtype=np.int32)
Y_REF = 134.0   # rendered luminance that maps to the palette colour itself (median paint sits a little darker)
Y_GAIN = 1.35

def develop(im):
    """The darkroom step for units: a unit is a few dozen pixels on screen, so its render is
    printed harder than a building's. A tone curve deepens the shadows, an unsharp mask restores
    the edges that the reduction to game zoom softens, and the silhouette gets the thin dark
    contour that keeps a camouflaged vehicle readable on ground of its own colour."""
    sharp = im.filter(ImageFilter.UnsharpMask(radius=2.2, percent=70, threshold=2))
    c = np.asarray(sharp).astype(np.float32)
    rgb, a = c[..., :3] / 255, c[..., 3]
    y = rgb @ np.array([.2126, .7152, .0722], dtype=np.float32)
    curved = np.where(y < .5, .5 * (2 * y) ** 1.22, 1 - .5 * (2 - 2 * y) ** 1.12)   # S-curve
    rgb = rgb * (curved / np.maximum(y, 1e-4))[..., None]
    rgb = y[..., None] * (1 - 1.12) + rgb * 1.12   # a little more colour than the haze leaves
    # Contour: opaque pixels within two texels of the outside (the shadow is not opaque).
    solid = Image.fromarray(((a > 215) * 255).astype(np.uint8))
    inner = np.asarray(solid.filter(ImageFilter.MinFilter(5))) > 0
    rim = (a > 215) & ~inner
    rgb = np.where(rim[..., None], rgb * .5, rgb)
    out = np.dstack([np.clip(rgb * 255, 0, 255), a]).astype(np.uint8)
    return Image.fromarray(out, 'RGBA')

def finish(color_path, mask_path, unit=False):
    im = Image.open(color_path).convert('RGBA')
    c = np.asarray(develop(im) if unit else im).astype(np.int32)
    m = np.asarray(Image.open(mask_path).convert('RGBA')).astype(np.int32)
    rgb, a = c[..., :3], c[..., 3]
    team = (m[..., 0] * m[..., 3] > 127 * 255) & (a > 0)
    y = rgb @ np.array([.2126, .7152, .0722])
    delta = np.clip(np.round((y - Y_REF) * Y_GAIN), -42, 53).astype(np.int32)
    enc = TEAM_BASE[None, None, :] + delta[..., None]
    rgb = np.where(team[..., None], enc, rgb)
    r, g_, b = rgb[..., 0], rgb[..., 1], rgb[..., 2]
    blue = ~team & (b > r + 60) & (g_ > r + 35)
    r = np.where(blue, np.maximum(r, np.minimum(b - 60, g_ - 35)), r)
    red = ~team & (r > g_ + 65) & (r > b + 70) & (r < 220)
    g_ = np.where(red, np.maximum(g_, r - 65), g_)
    out = np.stack([r, g_, b, a], axis=-1).clip(0, 255).astype(np.uint8)
    return Image.fromarray(out)

def main():
    global OUT
    src = Path(sys.argv[1]) if len(sys.argv) > 1 else Path.home() / 'Library/Caches/Codex/reviews/MyGame001/ra2-visual/render'
    if len(sys.argv) > 2: OUT = Path(sys.argv[2])
    for d in ('buildings', 'units', 'walk', 'terrain'): (OUT / d).mkdir(parents=True, exist_ok=True)
    n_b = n_u = n_w = 0
    for color in sorted((src / 'buildings').glob('*_color.png')):
        ident = color.name[:-len('_color.png')]
        finish(color, color.with_name(f'{ident}_mask.png')).save(OUT / 'buildings' / f'{ident}.png')
        n_b += 1
    idents = sorted({p.name.rsplit('_', 2)[0] for p in (src / 'units').glob('*_0_color.png')})
    for ident in idents:
        if not all((src / 'units' / f'{ident}_{f}_{p}.png').exists() for f in range(8) for p in ('color', 'mask')): continue
        frames = [finish(src / 'units' / f'{ident}_{f}_color.png', src / 'units' / f'{ident}_{f}_mask.png', True) for f in range(8)]
        size = frames[0].size[0]
        sheet = Image.new('RGBA', (size * 8, size))
        for f, im in enumerate(frames): sheet.paste(im, (size * f, 0))
        sheet.save(OUT / 'units' / f'{ident}.png')
        n_u += 1
    # Walk cycles: one row of eight facings per frame of the cycle (WALK_FRAMES rows).
    poses = 6
    for ident in sorted({p.name.rsplit('_', 3)[0] for p in (src / 'walk').glob('*_0_0_color.png')}):
        names = [(src / 'walk' / f'{ident}_{p}_{f}') for p in range(poses) for f in range(8)]
        if not all(Path(f'{n}_{k}.png').exists() for n in names for k in ('color', 'mask')): continue
        frames = [finish(f'{n}_color.png', f'{n}_mask.png', True) for n in names]
        size = frames[0].size[0]
        sheet = Image.new('RGBA', (size * 8, size * poses))
        for i, im in enumerate(frames): sheet.paste(im, (size * (i % 8), size * (i // 8)))
        sheet.save(OUT / 'walk' / f'{ident}.png')
        n_w += 1
    n_t = 0
    for kind in [1, 3, 5, 6, 7, 11, 12, 13, 14, 15, 16, 17, 18, 19]:
        if not (src / 'terrain' / f'{kind}_0_color.png').exists(): continue
        atlas = Image.new('RGBA', (1024, 1024))
        tall = kind in (16, 17)  # eight 256 x 512 slots (64 x 128 world px)
        for seed in range(8 if tall else 16):
            im = Image.open(src / 'terrain' / f'{kind}_{seed}_color.png').convert('RGBA')
            if tall:
                atlas.paste(im, ((seed % 4) * 256, (seed // 4) * 512))
                continue
            if kind in (5, 15):  # asphalt must close every seam with its neighbours
                a = np.asarray(im).copy()
                yy, xx = np.mgrid[0:256, 0:256]
                lift = 7.8 if kind == 15 else 0  # the bridge deck stands above the water
                inside = np.abs((xx + .5) / S - 32) / 32 + np.abs((yy + .5) / S - 32 + lift) / 16 <= (1.0 if kind == 5 else .985)
                a[..., 3] = np.where(inside, 255, a[..., 3]); im = Image.fromarray(a)
            atlas.paste(im, ((seed % 4) * 256, (seed // 4) * 256))
        atlas.save(OUT / 'terrain' / f'{kind}.png'); n_t += 1
    print(f'Finished {n_b} buildings, {n_u} unit sheets, {n_w} walk cycles and {n_t} terrain top atlases into {OUT}')

if __name__ == '__main__':
    main()
