"""Original RA5 neutral installations; compact one-cell silhouettes, in metres.
No reference artwork is used. Blue fascia/flags use the renderer's ownership mask.
"""
from hd_kit import *
from hd_arch import *

def oil_derrick(k, w, d):
    z = pad(k, w, d)
    k.box(1, 1, z, 8, 8, .3, CONCRETE)
    for x in (2.5, 7.5):
        for y in (3, 7):
            k.tube((x, y, z), (5, 5, z + 7), .22, STEEL, n=6)
    k.tube((2, 5, z + 7), (8.5, 5, z + 8), .45, TEAM, n=6)
    k.tube((8, 5, z + 8), (8, 5, z + .4), .10, STEEL, n=6)
    k.cyl(3, 2.2, z + .3, 1.1, 2, RUST, n=16)
    k.box(1, 7.5, z + .3, 3, 1.4, 1.5, CLAD_GREY)
    k.box(1.2, 8.91, z + 1, 2.5, .06, .4, HAZ)

def hospital(k, w, d):
    z = pad(k, w, d)
    k.box(1, 1, z, 8, 7, 4.4, CLAD_WHITE)
    k.box(.8, .8, z + 4.4, 8.4, 7.4, .35, PRECAST)
    k.box(1, 7.99, z + 3.2, 8, .08, .5, TEAM)
    with face(k, 'y+', 1, 8, z):
        windows(k, 8, 1, z0=1.8, h=1.0, margin=.7)
        door(k, 3.4)
    # Medical cross on roof and front, clearly separate from ownership paint.
    red = rgb(226, 38, 38)
    k.box(3.8, 2, z + 4.76, 1.4, 4.8, .06, red)
    k.box(2.1, 3.7, z + 4.76, 4.8, 1.4, .06, red)
    k.box(7.4, 8.1, z + 1.4, .4, .05, 1.6, red)
    k.box(6.8, 8.1, z + 2, 1.6, .05, .4, red)

def airport(k, w, d):
    z = pad(k, w, d)
    k.box(1, 1, z, 5, 6, 2.6, CLAD_GREY)
    k.box(6.5, 2, z, 2.4, 2.4, 6.5, PRECAST)
    k.box(6.1, 1.6, z + 6.5, 3.2, 3.2, 1.5, GLASS)
    k.box(5.9, 1.4, z + 8, 3.6, 3.6, .3, TEAM)
    k.box(1, 7.6, z + .05, 8, 1.8, .05, DARK)
    for x in (1.5, 3.5, 5.5, 7.5): k.box(x, 8.35, z + .12, 1, .15, .02, WHITE)
    with face(k, 'y+', 1, 7, z): shutter(k, .8, 3.4, 2.4)
    k.tube((7.7, 2.8, z + 8.3), (7.7, 2.8, z + 10), .05, STEEL, n=6)

def tech_outpost(k, w, d):
    z = pad(k, w, d)
    k.box(1, 1, z, 7, 6, 4.2, CLAD_GREY)
    k.box(.8, .8, z + 4.2, 7.4, 6.4, .3, PRECAST)
    with face(k, 'y+', 1, 7, z): shutter(k, 1.2, 4.6, 3.6, open_=.5)
    k.box(1, 7.02, z + 3.7, 7, .08, .4, TEAM)
    k.cyl(5, 3.8, z + 4.5, 1.4, .4, DARK, n=16)
    for x in (4, 5, 6):
        k.tube((x, 2.3, z + 5.2), (x, 5.3, z + 5.8), .28, OLIVE, n=8)
    k.box(1.5, 8, z + .05, 6, .2, .03, HAZ)
    k.box(1.5, 9, z + .05, 6, .2, .03, HAZ)

def bridge_hut(k, w, d):
    z = pad(k, w, d)
    k.box(2, 2, z, 6, 5, 3.2, BRICK)
    gable(k, 1.7, 1.7, z + 3.2, 6.6, 5.6, 1.8, CLAD_DARK, ridge='x', wall=BRICK)
    with face(k, 'y+', 2, 7, z): door(k, 2); windows(k, 6, 1, z0=1.7, h=.8, margin=.5)
    k.box(2.2, 7.05, z + 2.8, 5.6, .08, .35, HAZ)
    for y in (8, 8.6, 9.2): k.box(1.5, y, z + .1, 6, .3, .4, TIMBER)

MODELS = {f.__name__: f for f in (oil_derrick, hospital, airport, tech_outpost, bridge_hut)}
