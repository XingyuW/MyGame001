"""Original deterministic RA5 faceted gem atlas and timber supply crate.
Run with Pillow; no external imagery. Four-times game-resolution sprites.
"""
from pathlib import Path
import random
from PIL import Image, ImageDraw
ROOT = Path(__file__).resolve().parents[1] / 'assets/sprites/hd'

def gems():
    image = Image.new('RGBA', (1024, 1024))
    draw = ImageDraw.Draw(image)
    for slot in range(16):
        rnd = random.Random(501 + slot)
        ox, oy = slot % 4 * 256, slot // 4 * 256
        for _ in range(16):
            x = rnd.randint(65, 185); y = rnd.randint(112, 174)
            if abs(x - 128) + 2 * abs(y - 145) > 80: continue
            x += ox; y += oy
            w, h = rnd.randint(8, 15), rnd.randint(18, 38)
            draw.ellipse((x-w-3,y-4,x+w+5,y+8), fill=(30,48,42,75))
            top=(x,y-h); left=(x-w,y-h//3); mid=(x,y); right=(x+w,y-h//3)
            draw.polygon([top,left,mid], fill=(100,144,154,255))
            draw.polygon([top,mid,right], fill=(208,244,236,255))
            draw.line([top,right,mid], fill=(242,255,250,255), width=2)
    image.save(ROOT / 'terrain/25.png')

def crate():
    image = Image.new('RGBA',(128,128)); d=ImageDraw.Draw(image)
    top=[(64,20),(110,44),(64,68),(18,44)]
    left=[(18,44),(64,68),(64,112),(18,88)]
    right=[(64,68),(110,44),(110,88),(64,112)]
    for poly,color in [(left,(114,73,36,255)),(right,(154,103,48,255)),(top,(196,150,83,255))]:
        d.polygon(poly,fill=color);d.line(poly+[poly[0]],fill=(58,44,28,255),width=3)
    for y in (57,70,83):
        d.line([(20,y),(62,y+24)],fill=(75,51,27,255),width=2)
        d.line([(66,y+24),(108,y)],fill=(100,68,32,255),width=2)
    for x in (34,88):
        d.polygon([(x,34 if x==34 else 32),(x+7,38 if x==34 else 36),(x+7,97 if x==34 else 97),(x,93 if x==34 else 101)],fill=(76,84,74,255))
    d.polygon([(53,48),(64,42),(76,48),(64,55)],fill=(222,214,175,255))
    image.save(ROOT / 'crate.png')

if __name__ == '__main__':
    gems(); crate()
