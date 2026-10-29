"""Buildings, modelled in metres as real installations (one cell is 10 m; see hd_arch.py).

Allied bases are modern prefabricated military engineering: profiled steel cladding on precast
plinths, flat roofs crowded with plant, glass and galvanised steel. Soviet bases are heavy
industry: brick, dark concrete, rusting sheet roofs, copper and riveted steelwork. The owner's
colour (`TEAM`) appears as real organisations mark property: a flag, a fascia stripe, door and
equipment paint, a roof recognition panel.
"""
from math import sin, cos, pi, radians
from hd_kit import *
from hd_arch import *
import hd_vehicles as veh

# --- Allied core ------------------------------------------------------------------------------------
def power_plant(k, w, d):
    # Gas-turbine power station: turbine hall, two exhaust stacks, fin-fan coolers, switchyard.
    z = pad(k, w, d)
    # Turbine hall with a low-pitch roof and a filter house for the intake air.
    k.box(1.5, 1.5, z, 11, 9, 7.2, CLAD_WHITE); k.box(1.44, 1.44, z, 11.12, 9.12, 1.2, PRECAST)
    gable(k, 1.5, 1.5, z + 7.2, 11, 9, 1.3, CLAD_GREY, ridge='x', wall=CLAD_WHITE)
    k.box(1.45, 10.48, z + 6.5, 11.1, .08, .6, TEAM); k.box(12.48, 1.45, z + 6.5, .08, 9.1, .6, TEAM)
    with face(k, 'y+', 1.5, 10.5, z):
        windows(k, 11, 1, z0=4.2, h=1.2, margin=1.0); door(k, 1.2); shutter(k, 5.5, 3.6, 3.8, open_=.0)
    with face(k, 'x+', 12.5, 10.5, z): windows(k, 9, 1, z0=4.2, h=1.2, margin=1.0)
    k.box(3.0, 3.2, z + 8.3, 4.5, 3.6, 2.2, GALV)
    for i in range(6): k.box(3.0 + i * .75 + .1, 6.78, z + 8.5, .55, .06, 1.8, DARK)
    k.box(7.5, 4.2, z + 8.0, 3.0, 1.6, 1.2, GALV)
    # Exhaust ducts into two banded stacks.
    for y in (3.6, 8.0):
        k.box(12.5, y - .9, z + 2.0, 2.2, 1.8, 2.4, rgb(124, 128, 130))
        chimney(k, 16.0, y, z, 17.0, 1.25, rgb(170, 172, 170), bands=3)
        k.rev((14.6, y, z + 3.2), 'x', [(0, 1.0), (.8, 1.2)], rgb(124, 128, 130), n=12)
    # Fin-fan cooler bank on a steel table.
    for x in (2.0, 9.0):
        for y in (12.6, 17.4): k.tube((x, y, z), (x, y, z + 3.2), .12, STEEL, n=6)
    k.box(1.8, 12.4, z + 3.2, 7.4, 5.2, .9, GALV)
    for i in range(3):
        k.cyl(3.1 + i * 2.4, 15.0, z + 4.1, 1.0, .35, tint(GALV, .9), n=16)
        k.cyl(3.1 + i * 2.4, 15.0, z + 4.42, .92, .04, DARK, n=16)
        for b in range(4):
            with k.at(3.1 + i * 2.4, 15.0, z + 4.48, yaw=b * 90 + i * 25): k.box(0, -.12, 0, .88, .24, .02, STEEL)
    k.rail([(1.8, 17.6, z + 4.1), (9.2, 17.6, z + 4.1), (9.2, 12.4, z + 4.1)], GALV)
    k.ladder(9.35, 16.5, z, z + 4.1, axis='y')
    pipe_run(k, [(4.0, 10.6, z + 2.4), (4.0, 12.0, z + 2.4), (4.0, 12.0, z + 3.6), (4.0, 12.6, z + 3.6)], .22, rgb(170, 60, 50))
    pipe_run(k, [(7.0, 10.6, z + 1.6), (7.0, 12.0, z + 1.6), (7.0, 12.0, z + 3.6), (7.0, 12.6, z + 3.6)], .22, STEEL)
    # Switchyard: transformers under a gantry, fenced.
    apron(k, 11.4, 11.8, 7.4, 7.2, z, GRAVEL)
    transformer(k, 13.4, 15.2, z + .15); transformer(k, 16.6, 15.2, z + .15)
    for x in (12.0, 18.2):
        k.mast(x, 13.0, z, 6.5, .7, .5, GALV, bays=4, t=.07)
    k.truss((12.0, 13.0, z + 6.0), (18.2, 13.0, z + 6.0), .6, .5, GALV, bays=6, t=.06)
    for x in (13.4, 15.0, 16.6):
        insulator(k, x, 13.0, z + 5.0, 1.0, .14); k.strut((x, 13.0, z + 5.0), (x, 15.4, z + 3.0), .03, COPPER)
    fence(k, [(11.5, 18.8, z), (18.8, 18.8, z), (18.8, 11.9, z)], h=2.0)
    k.box(11.6, 18.74, z + .9, 1.0, .05, .7, HAZ)
    flagpole(k, 13.2, 11.0, z, 8.0); floodlight(k, 18.6, 10.6, z, 7.0, yaw=200)

# How far the vehicle door of the war factory, or the entrance of the barracks, is open (0 shut, 1 up in its drum). The shipped sprite
# has it shut; render_hd_blender.py renders each of DOOR_FRAMES as `<ident>_door<n>`, which the
# game shows while a vehicle drives out (crates/game/src/arrival.rs).
DOOR = 0.0
DOOR_FRAMES = {'war_factory': (1 / 3, 2 / 3, 1.0), 'barracks': (1 / 3, 2 / 3, 1.0)}
# The vehicle door of the war factory, in metres of the model: it opens in the hall's y+ wall
# between x0 and x0 + w (arrival.rs holds the same numbers, in cells).
WF_DOOR_X0, WF_DOOR_W, WF_DOOR_H, WF_WALL_Y = 7.5, 8.0, 6.4, 19.0

def vehicle_door(k, x, w, h, open_=0.0):
    """A works' vehicle door in a wall's frame (see `face`): a sectional steel door that rolls
    up into a drum over the lintel, in a portal of hazard-striped jambs. Below the door's edge
    the doorway is a holdout: the render leaves it transparent, and the game draws the dark hall
    and the vehicle driving out of it behind the sprite."""
    hole = h * open_
    if hole > .05: k.box(x, -.6, 0, w, .62, hole, HOLDOUT)
    panels = 6
    shut = h - hole
    if shut > .05:
        ph = shut / panels
        for i in range(panels):
            zz = hole + i * ph
            k.box(x, -.06 + (i % 2) * .015, zz, w, .1, ph, tint(DOOR_GREY, .94 if i % 2 else 1.0))
            k.box(x, .04, zz + ph - .05, w, .03, .05, DARK)   # panel joint
        # Vision panels in the second section from the top, and the bottom rubber seal.
        zz = hole + (panels - 2) * ph + ph * .3
        for i in range(6): k.box(x + .45 + i * (w - .9) / 6, .05, zz, (w - .9) / 6 - .25, .03, ph * .4, GLASS_DARK)
        k.box(x, .02, hole, w, .1, .08, RUBBER)
    # Portal: hazard-striped steel jambs, a concrete lintel and the drum housing above it.
    for xx in (x - .45, x + w):
        k.box(xx, -.06, 0, .45, .3, h + .45, HAZ)
    k.box(x - .5, -.08, h, w + 1.0, .36, .45, CONC_LIGHT)
    k.box(x - .3, -.05, h + .45, w + .6, .55, .6, GALV)
    # Beacons at the lintel's ends turn while the door moves; a traffic light says when to go.
    lit = open_ > 0
    for xx in (x - .25, x + w + .25):
        k.cyl(xx, .3, h + .45, .16, .1, DARK, n=8)
        k.cyl(xx, .3, h + .55, .13, .26, HOT if lit else rgb(150, 80, 30, 'glass'), n=10)
    k.box(x + w + .55, .05, 2.2, .4, .22, 1.0, DARK)
    k.ball(x + w + .75, .3, 2.95, .1, RED_LAMP if not lit else rgb(70, 20, 16, 'glass'), n=6, k=3)
    k.ball(x + w + .75, .3, 2.45, .1, GREEN_LAMP if lit else rgb(16, 60, 24, 'glass'), n=6, k=3)
    # Lamps over the doorway light the apron.
    for xx in (x + w * .25, x + w * .75):
        k.box(xx - .3, .0, h + 1.15, .6, .5, .14, DARK); k.box(xx - .26, .3, h + 1.1, .52, .2, .05, LAMP)

def war_factory(k, w, d):
    # Armoured vehicle works: a saw-tooth assembly hall whose wide vehicle door opens onto a
    # marked drive lane, an office wing, and a gantry crane over the side of the test apron.
    z = pad(k, w, d)
    apron(k, 1.0, 19.0, 28.0, 10.0, z)
    # Assembly hall.
    k.box(2, 2, z, 19, 17, 8.5, CLAD_GREY); k.box(1.94, 1.94, z, 19.12, 16.9, 1.4, PRECAST)
    # The plinth's front runs either side of the vehicle door, not across it.
    k.box(1.94, 18.84, z, WF_DOOR_X0 - .45 - 1.94, .22, 1.4, PRECAST)
    k.box(WF_DOOR_X0 + WF_DOOR_W + .45, 18.84, z, 21.06 - WF_DOOR_X0 - WF_DOOR_W - .45, .22, 1.4, PRECAST)
    sawtooth(k, 2, 2, z + 8.5, 19, 17, 5, 2.6, CLAD_DARK)
    k.box(1.95, 18.98, z + 7.6, 19.1, .08, .7, TEAM)
    x0 = WF_DOOR_X0 - 2
    with face(k, 'y+', 2, WF_WALL_Y, z):
        vehicle_door(k, x0, WF_DOOR_W, WF_DOOR_H, open_=DOOR)
        door(k, 17.4)
        windows(k, x0 - .5, 1, z0=6.0, h=.8, margin=.8, bay=1.6)
        with k.at(x0 + WF_DOOR_W + 1.6, 0, 0): windows(k, 19 - x0 - WF_DOOR_W - 1.6, 1, z0=6.0, h=.8, margin=.5, bay=1.6)
        k.box(x0 + WF_DOOR_W / 2 - 1.1, .05, WF_DOOR_H + 1.15, 2.2, .06, .4, TEAM)   # bay number board
    with face(k, 'x+', 21, 19, z): windows(k, 17, 1, z0=5.2, h=1.6, margin=1.5, bay=1.6)
    # Extraction stacks and roof ventilators.
    for x in (5.5, 11.5, 17.5): vent(k, x, 4.0, z + 9.2, .45, 1.4)
    k.cyl(19.5, 3.5, z + 8.5, .5, 6.0, GALV, n=12); k.rev((19.5, 3.5, z + 14.5), 'z', [(0, .62), (.3, .62)], DARK, n=12)
    # Office wing.
    top = block(k, 21, 5, 6.5, 11, 6.8, PRECAST, plinth=CONC_DARK, floors=2, band=TEAM)
    roof_plant(k, 21.4, 5.4, top, 5.7, 10.2, seed=4)
    with face(k, 'y+', 21, 16, z): door(k, 4.6)
    antenna_mast(k, 26.2, 6.2, top, 6.0, w=.6, dishes=1)
    # Drive lane from the door to the apron's edge: yellow edge lines, a zebra threshold, arrows.
    lx0, lx1 = WF_DOOR_X0 - .4, WF_DOOR_X0 + WF_DOOR_W + .25
    for xx in (lx0, lx1): line(k, xx, 19.2, .15, 9.4, z, c=YELLOW)
    for i in range(7): k.box(WF_DOOR_X0 + .3 + i * 1.12, 19.15, z + .03, .56, 1.0, .014, HAZ)
    cx = WF_DOOR_X0 + WF_DOOR_W / 2
    for y in (22.6, 26.0):
        line(k, cx - .12, y, .24, 1.6, z)
        k.poly([(cx - .7, y + 1.6, z + .045), (cx + .7, y + 1.6, z + .045), (cx, y + 2.5, z + .045)], WHITE)
    # Right of the lane only low things, so that nothing tall stands between the camera and a
    # vehicle leaving: a turret on stands, a low-loader with a hull, a parts rack.
    with k.at(22.85, 21.4, z + .9, yaw=90, s=.72): veh.turret_welded(k, 0, 0, NATO_SAND, front=2.0, rear=2.1, w=1.5, h=.82, nose=.55)
    for dx in (-1.0, 1.0): k.cbox(22.85 + dx, 21.4, z, .3, 1.6, .9, IRON)
    k.box(18.6, 25.2, z + .5, 9.0, 2.6, .35, DARK); k.box(18.6, 25.2, z + .85, 9.0, 2.6, .08, rgb(92, 84, 70, 'wood'))
    for x in (19.4, 20.6, 25.6, 26.8):
        for y in (25.2, 27.5): k.cyl(x, y + .25 if y < 26 else y + .05, z, .45, .25, TYRE, n=10)
    with k.at(23.1, 26.5, z + .95, yaw=0, s=.66): veh.tank_hull(k, 7.9, 3.6, .62, NATO_SAND, deck=1.22, glacis=2.2, nose=.8)
    k.box(18.4, 19.6, z, 1.2, 4.2, 1.8, GALV)
    for i in range(3): k.box(18.5, 19.7 + i * 1.35, z + .4, 1.0, 1.1, .9, tint(NATO_SAND, .9))
    for i in range(3): crate(k, 1.6 + (i % 2) * 1.4, 20.0 + (i // 2) * 1.5, z, 1.2)
    crate(k, 2.2, 20.4, z + .98, 1.0, yaw=20)
    barrels(k, 4.6, 20.2, z, 2, rows=2)
    container(k, 1.2, 25.6, z, rgb(60, 92, 120), yaw=0, L=5.9)
    hazard_edge(k, 1.2, 28.6, 27.6, .35, z)
    flagpole(k, 27.8, 18.5, z, 9.0); floodlight(k, 1.6, 23.8, z, 9.0, yaw=30); floodlight(k, 28.6, 28.6, z, 9.0, yaw=215)

# The barracks' entrance, in metres of the model, as the war factory's door above
# (arrival.rs holds the same numbers, in cells).
BK_DOOR_X0, BK_DOOR_W, BK_DOOR_H, BK_WALL_Y = 4.6, 2.8, 2.7, 9.0

def entrance(k, x, w, h, open_=0.0):
    """A pair of glazed steel doors in a wall's frame (see `face`), swinging outwards about
    their jambs, under a canopy. Behind open doors the doorway is a holdout, as in
    `vehicle_door`: the game draws the lobby and the soldier walking out of it."""
    if open_ > 0: k.box(x, -.5, 0, w, .52, h, HOLDOUT)
    a = 90 * open_
    for hinge, yaw, sign in ((x, a, 1), (x + w, -a, -1)):
        with k.at(hinge, 0, 0, yaw=yaw):
            x0 = 0 if sign > 0 else -w / 2
            k.box(x0, -.03, 0, w / 2, .06, h, DARK)
            k.box(x0 + .1, -.05, .25, w / 2 - .2, .1, h - .4, GLASS)
            k.box(x0 + (w / 2 - .25 if sign > 0 else .15), .03, h * .45, .1, .08, .5, CHROME)  # push bar
    # Frame, threshold, the canopy on two posts and its lamps.
    for xx in (x - .18, x + w): k.box(xx, -.06, 0, .18, .18, h + .2, DARK)
    k.box(x - .18, -.06, h, w + .36, .18, .2, DARK)
    k.box(x - .5, 0, -.02, w + 1.0, 1.6, .1, CONC_LIGHT)
    k.box(x - 1.1, -.1, h + .55, w + 2.2, 2.4, .22, CONC_LIGHT)
    k.box(x - 1.1, 2.24, h + .55, w + 2.2, .08, .3, TEAM)
    for xx in (x - .95, x + w + .85): k.cyl(xx, 2.0, 0, .1, h + .55, GALV, n=8)
    for xx in (x + .3, x + w - .3): k.box(xx - .15, .9, h + .5, .3, .3, .05, LAMP)

def barracks(k, w, d):
    # Infantry training centre: a two-storey instruction block whose glazed entrance opens onto
    # a parade square, a rappelling tower, an obstacle course, and its colours flying high from
    # the roof.
    z = pad(k, w, d, c=TARMAC)
    wall = rgb(186, 178, 156, 'concrete')
    x1, y0 = 14.0, 1.5
    top = z + 7.0
    k.box(1.5, y0, z, x1 - 1.5, BK_WALL_Y - y0, 7.0, wall, top=ROOF)
    parapet(k, 1.5, y0, top, x1 - 1.5, BK_WALL_Y - y0, wall, .6)
    # Plinth either side of the entrance, and a band in the owner's colour under the parapet.
    k.box(1.44, y0 - .06, z, x1 - 1.38, BK_WALL_Y - y0 - .1, .6, CONC_DARK)
    k.box(1.44, BK_WALL_Y - .2, z, BK_DOOR_X0 - .6 - 1.44, .26, .6, CONC_DARK)
    k.box(BK_DOOR_X0 + BK_DOOR_W + .6, BK_WALL_Y - .2, z, x1 + .06 - BK_DOOR_X0 - BK_DOOR_W - .6, .26, .6, CONC_DARK)
    k.box(1.45, BK_WALL_Y - .02, top - .75, x1 - 1.4, .08, .5, TEAM); k.box(x1 - .02, y0 - .05, top - .75, .08, BK_WALL_Y - y0 + .1, .5, TEAM)
    with face(k, 'y+', 1.5, BK_WALL_Y, z):
        x0 = BK_DOOR_X0 - 1.5
        entrance(k, x0, BK_DOOR_W, BK_DOOR_H, open_=DOOR)
        punched(k, x0 - .9, 2, bay=1.9, margin=.6)
        with k.at(x0 + BK_DOOR_W + 1.3, 0, 0): punched(k, x1 - 1.5 - x0 - BK_DOOR_W - 1.3, 2, bay=1.9, margin=.5)
        with k.at(x0, 0, 0): punched(k, BK_DOOR_W, 1, z0=4.4, bay=1.4, margin=.2)
        # Crest board over the canopy: the owner's colours round a white star.
        k.box(x0 + BK_DOOR_W / 2 - 1.0, .05, 5.9, 2.0, .08, .7, TEAM)
        k.poly([(x0 + BK_DOOR_W / 2 + .35 * cos(radians(90 + 144 * i)), .14, 6.25 + .3 * sin(radians(90 + 144 * i))) for i in range(5)], WHITE)
    with face(k, 'x+', x1, BK_WALL_Y, z): punched(k, BK_WALL_Y - y0, 2, bay=2.0, margin=1.0)
    roof_plant(k, 2.0, 2.0, top, 7.0, 6.0, seed=8, density=.8)
    # The colours: a tall mast on the roof with a large flag in the owner's colour.
    flagpole(k, 11.6, 3.6, top, 8.5, w=3.6)
    # Rappelling tower beside the block: timber frame, platform, roof and ropes.
    tx, ty = 15.6, 2.6
    for x, y in [(tx, ty), (tx + 2.8, ty), (tx, ty + 2.8), (tx + 2.8, ty + 2.8)]: k.tube((x, y, z), (x, y, z + 8.0), .13, TIMBER, n=4)
    for zz in (2.6, 5.2):
        k.strut((tx, ty + 2.8, z + zz - 2.4), (tx + 2.8, ty + 2.8, z + zz), .08, TIMBER); k.strut((tx + 2.8, ty, z + zz - 2.4), (tx + 2.8, ty + 2.8, z + zz), .08, TIMBER)
    k.box(tx - .3, ty - .3, z + 8.0, 3.4, 3.4, .15, TIMBER)
    for bx, by, bw, bd in [(tx - .3, ty + 3.0, 3.4, .1), (tx + 3.0, ty - .3, .1, 3.4)]: k.box(bx, by, z + 8.15, bw, bd, 1.0, rgb(118, 104, 78, 'wood'))
    gable(k, tx - .4, ty - .4, z + 9.6, 3.6, 3.6, .8, rgb(86, 98, 92, 'corr'), ridge='x')
    for x, y in [(tx - .2, ty - .2), (tx + 3.0, ty - .2), (tx - .2, ty + 3.0), (tx + 3.0, ty + 3.0)]: k.strut((x, y, z + 8.15), (x, y, z + 9.6), .07, TIMBER)
    for y in (ty + .9, ty + 1.9): k.tube((tx + 3.15, y, z + 8.0), (tx + 3.6, y, z + .1), .03, CANVAS, n=4)
    k.box(tx + 3.0, ty, z, 1.4, 2.8, .25, rgb(60, 70, 56, 'rubber'))   # landing mat
    # Parade square left of the walk from the door: marker lines, a saluting base.
    for i in range(4): line(k, 1.4, 10.6 + i * 2.1, 2.6, .14, z)
    k.box(1.3, 17.6, z, 2.4, 1.6, .35, CONC_LIGHT); k.box(1.3, 19.1, z + .35, 2.4, .08, .7, TEAM)
    # Obstacle course right of it: low walls, a tyre run, balance beam, monkey bars, net frame.
    for i, y in enumerate((10.8, 13.4)): k.box(9.8, y, z, 3.2, .3, .9 + .4 * i, rgb(140, 120, 92, 'wood'))
    for i in range(6): k.rev((10.2 + (i % 2) * .9, 15.4 + (i // 2) * .8, z), 'z', [(.25, 0), (.42, .05), (.42, .22), (.25, .27)], TYRE, n=10)
    k.box(13.4, 10.6, z + .5, .2, 5.2, .15, TIMBER)
    for y in (10.8, 15.6): k.cbox(13.5, y, z, .3, .3, .5, TIMBER)
    for x in (15.2, 17.4):
        for y in (11.2, 14.0): k.tube((x, y, z), (x, y, z + 2.3), .07, STEEL, n=6)
    k.box(15.1, 11.1, z + 2.3, 2.4, .1, .06, STEEL); k.box(15.1, 13.9, z + 2.3, 2.4, .1, .06, STEEL)
    for i in range(6): k.tube((15.2 + i * .44, 11.2, z + 2.32), (15.2 + i * .44, 14.0, z + 2.32), .03, STEEL, n=4)
    for x in (15.4, 18.6): k.strut((x, 16.2, z), (17.0, 17.4, z + 3.6), .08, TIMBER)
    for x in (15.4, 18.6): k.strut((x, 18.6, z), (17.0, 17.4, z + 3.6), .08, TIMBER)
    for i in range(5): k.strut((15.6 + i * .7, 16.3, z + .2), (16.4 + i * .3, 17.3, z + 3.4), .02, CANVAS)
    sandbags(k, [(9.6, 19.0, z), (13.2, 19.0, z)], 2)
    crate(k, 2.0, 15.2, z, .9, c=OLIVE); barrels(k, 18.2, 9.6, z, 2, c=OLIVE)
    floodlight(k, 19.3, 8.8, z, 8.0, yaw=200)

# --- Soviet core ------------------------------------------------------------------------------------
def tesla_coil_tower(k, x, y, z, h=9.0, r=1.5):
    """A Tesla secondary: primary winding at the base, insulator column, copper toroid, terminal."""
    k.cyl(x, y, z, r * .95, .8, CONC_DARK, n=16)
    for i in range(4): k.torus(x, y, z + 1.0 + i * .22, r * (.85 - i * .08), .07, COPPER, n=20, k=5)
    k.cyl(x, y, z + .8, r * .34, h, rgb(96, 62, 44), n=14)
    n = int(h / .55)
    for i in range(n): k.rev((x, y, z + 1.2 + i * (h - .6) / n), 'z', [(0, r * .36), (.12, r * .56), (.24, r * .36)], rgb(150, 96, 66), n=14)
    k.torus(x, y, z + h + .9, r * .9, r * .28, COPPER, n=24, k=8)
    k.cyl(x, y, z + h + .7, r * .2, 1.1, COPPER, n=10)
    k.ball(x, y, z + h + 2.1, r * .34, TESLA, n=12, k=6)

def tesla_reactor(k, w, d):
    # Soviet Tesla power station: a brick generator hall feeding two resonant coil towers.
    z = pad(k, w, d, c=CONC_DARK)
    k.box(1.5, 1.5, z, 10, 10.5, 8.0, BRICK); k.box(1.42, 1.42, z, 10.16, 10.66, 1.0, CONC_DARK)
    for x in (1.5, 4.8, 8.2, 11.3): k.box(x - .1, 11.95, z, .5, .2, 8.0, BRICK_DARK)  # pilasters
    for y in (1.5, 5.0, 8.4, 11.6): k.box(11.45, y - .1, z, .2, .5, 8.0, BRICK_DARK)
    k.box(1.35, 1.35, z + 8.0, 10.3, 10.8, .5, CONC_DARK)
    gable(k, 1.5, 1.5, z + 8.5, 10, 10.5, 2.2, CLAD_RUST, ridge='y', wall=BRICK)
    k.box(4.0, 1.5, z + 10.2, 5.0, 10.5, .9, CLAD_RUST); k.box(3.9, 1.4, z + 11.1, 5.2, 10.7, .15, RUST)  # roof monitor
    with face(k, 'y+', 1.5, 12, z):
        for x in (1.2, 4.5, 7.8): k.box(x, -.08, 2.6, 1.6, .14, 3.6, GLASS_GREEN); k.box(x - .1, -.02, 2.45, 1.8, .16, .15, CONC_LIGHT); k.box(x + .76, -.02, 2.6, .08, .1, 3.6, DARK); k.box(x, -.02, 4.4, 1.6, .1, .08, DARK)
        door(k, 9.4, 1.0, c=rgb(84, 60, 50))
        k.box(3.2, .02, 6.6, 3.6, .06, .8, TEAM)
    with face(k, 'x+', 11.5, 12, z):
        for x in (1.4, 4.8): k.box(x, -.08, 2.6, 1.6, .14, 3.6, GLASS_GREEN); k.box(x - .1, -.02, 2.45, 1.8, .16, .15, CONC_LIGHT); k.box(x + .76, -.02, 2.6, .08, .1, 3.6, DARK)
        shutter(k, 7.4, 2.6, 3.4, c=remat(rgb(96, 70, 56), 'corr'), stripes=False)
    chimney(k, 3.0, 3.0, z + 8.5, 9.0, .8, BRICK_DARK, bands=0, platform=False, lamp=True)
    # Coil towers on their own plinth, with bus bars back to the hall and a transformer.
    k.box(12.0, 12.6, z, 7.0, 6.6, .4, CONC_DARK)
    tesla_coil_tower(k, 15.6, 16.0, z + .4, 9.5, 1.7)
    tesla_coil_tower(k, 15.8, 6.5, z, 7.5, 1.4)
    transformer(k, 6.0, 15.6, z + .15, rgb(92, 100, 84))
    for (a, b) in [((6.0, 15.9, z + 2.9), (13.9, 16.0, z + 1.6)), ((11.6, 6.5, z + 5.0), (14.4, 6.5, z + 1.4))]: k.strut(a, b, .06, COPPER)
    insulator(k, 11.7, 6.5, z + 4.0, 1.0, .16)
    pipe_run(k, [(2.5, 12.1, z + 1.2), (2.5, 14.0, z + 1.2), (4.6, 14.0, z + 1.2)], .2, RUST)
    fence(k, [(12.0, 19.3, z), (19.3, 19.3, z), (19.3, 12.6, z)], h=1.8, c=IRON)
    k.box(15.2, 19.24, z + .7, 1.2, .05, .8, HAZ)
    barrels(k, 9.0, 14.0, z, 3, c=rgb(84, 92, 70)); crate(k, 9.4, 17.2, z, 1.2, c=TIMBER)
    flagpole(k, 2.4, 18.4, z, 8.5)

# --- Defences ---------------------------------------------------------------------------------------
def gun_turret(k, w, d):
    # Allied gun emplacement: a tank turret on a sunk concrete barbette, ringed with sandbags.
    z = 0
    k.rev((5, 5, z), 'z', [(0, 4.3), (.5, 4.3), (1.5, 3.3), (1.7, 3.3)], CONCRETE, n=8)
    k.rev((5, 5, z + 1.7), 'z', [(0, 2.6), (.25, 2.6)], DARK, n=20)
    with k.at(5, 5, z + 1.95, yaw=-12, s=1.35):
        veh.turret_welded(k, 0, 0, NATO_SAND, front=2.0, rear=1.9, w=1.45, h=.85, nose=.55)
        k.cbox(1.85, 0, .1, .5, .9, .6, tint(NATO_SAND, .9)); veh.gun(k, 2.0, .42, 4.4, .09, evac=.52)
        veh.cupola(k, -.2, -.7, .85, NATO_SAND); veh.mg(k, -.2, -.7, 1.08, yaw=8)
        veh.hatch(k, -.3, .65, .85, .32, NATO_SAND); veh.roof_panel(k, -1.2, 0, .85, .9, 1.6)
        for s in (-1, 1): veh.smoke_launchers(k, .4, s * 1.45, .35, s)
        veh.stowage(k, -1.95, 0, .1, .7, 2.4, NATO_SAND); veh.antenna(k, -1.7, 1.1, .85, 2.0)
    sandbags(k, [(8.6, 2.2, z), (9.3, 5.0, z), (8.6, 7.8, z)], 3); sandbags(k, [(2.2, 8.6, z), (5.0, 9.3, z), (7.6, 8.7, z)], 3)
    crate(k, 1.3, 5.6, z, .9, c=OLIVE); crate(k, 1.5, 4.4, z, .8, c=OLIVE, yaw=15)
    k.ladder(5.0, 9.05, z, z + 1.7, axis='x')

MODELS = {f.__name__: f for f in [power_plant, war_factory, barracks, tesla_reactor, gun_turret]}
for _name in ('hd_buildings_a', 'hd_buildings_b', 'hd_buildings_c', 'hd_buildings_neutral'):
    try:
        MODELS.update(__import__(_name).MODELS)
    except ImportError:
        pass

def build(m, ident, w, d):
    MODELS[ident](Kit(m, UB), w, d)
