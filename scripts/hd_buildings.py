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

def war_factory(k, w, d):
    # Armoured vehicle works: a saw-tooth assembly hall with two vehicle doors, an office wing,
    # an outdoor overhead crane over the test apron.
    z = pad(k, w, d)
    apron(k, 1.0, 19.0, 28.0, 10.0, z)
    # Assembly hall.
    k.box(2, 2, z, 19, 17, 8.5, CLAD_GREY); k.box(1.94, 1.94, z, 19.12, 17.12, 1.4, PRECAST)
    sawtooth(k, 2, 2, z + 8.5, 19, 17, 5, 2.6, CLAD_DARK)
    k.box(1.95, 18.98, z + 7.6, 19.1, .08, .7, TEAM)
    with face(k, 'y+', 2, 19, z):
        shutter(k, 2.2, 5.6, 6.0, open_=.62); shutter(k, 10.6, 5.6, 6.0, open_=.0)
        door(k, 17.4); windows(k, 19, 1, z0=6.6, h=.8, margin=1.2, bay=1.6)
    with face(k, 'x+', 21, 19, z): windows(k, 17, 1, z0=5.2, h=1.6, margin=1.5, bay=1.6)
    # A hull on the line, seen through the open door.
    with k.at(5.0, 17.6, z + .05, yaw=90, s=.72): veh.tank_hull(k, 7.9, 3.6, .62, NATO_SAND, deck=1.22, glacis=2.2, nose=.8); veh.tracks(k, -3.95, 3.95, 3.6, .62, 7, .32, .95, NATO_SAND)
    # Extraction stacks and roof ventilators.
    for x in (5.5, 11.5, 17.5): vent(k, x, 4.0, z + 9.2, .45, 1.4)
    k.cyl(19.5, 3.5, z + 8.5, .5, 6.0, GALV, n=12); k.rev((19.5, 3.5, z + 14.5), 'z', [(0, .62), (.3, .62)], DARK, n=12)
    # Office wing.
    top = block(k, 21, 5, 6.5, 11, 6.8, PRECAST, plinth=CONC_DARK, floors=2, band=TEAM)
    roof_plant(k, 21.4, 5.4, top, 5.7, 10.2, seed=4)
    with face(k, 'y+', 21, 16, z): door(k, 4.6)
    antenna_mast(k, 26.2, 6.2, top, 6.0, w=.6, dishes=1)
    # Outdoor gantry crane on its runway.
    for x in (3.0, 15.5):
        for y in (20.5, 28.0): k.tube((x, y, z), (x, y, z + 6.5), .16, YELLOW, n=6)
        k.box(x - .2, 20.3, z + 6.5, .4, 7.9, .45, YELLOW)
    k.box(2.8, 23.2, z + 6.95, 13.0, .9, .8, YELLOW); k.box(8.4, 23.0, z + 6.4, 1.3, 1.3, .55, DARK)
    k.tube((9.05, 23.65, z + 6.4), (9.05, 23.65, z + 3.2), .03, DARK, n=4); k.cbox(9.05, 23.65, z + 2.9, .4, .2, .35, YELLOW)
    # A turret on stands under the hook, crates, drums, a parked forklift, floodlights.
    with k.at(9.05, 23.65, z + .9, yaw=90, s=.72): veh.turret_welded(k, 0, 0, NATO_SAND, front=2.0, rear=2.1, w=1.5, h=.82, nose=.55)
    for dx in (-1.0, 1.0): k.cbox(9.05 + dx, 23.65, z, .3, 1.6, .9, IRON)
    for i in range(3): crate(k, 17.5 + (i % 2) * 1.4, 22.0 + (i // 2) * 1.5, z, 1.2)
    crate(k, 18.2, 22.4, z + .98, 1.0, yaw=20)
    barrels(k, 22.5, 20.5, z, 4, rows=2)
    container(k, 21.5, 23.5, z, rgb(60, 92, 120), yaw=0); container(k, 21.5, 26.3, z, rgb(140, 70, 50), yaw=0)
    line(k, 4.0, 19.6, .15, 8.6, z); line(k, 9.2, 19.6, .15, 8.6, z); line(k, 12.4, 19.6, .15, 8.6, z); line(k, 17.6, 19.6, .15, 8.6, z)
    hazard_edge(k, 1.2, 28.6, 27.6, .35, z)
    flagpole(k, 27.8, 18.5, z, 9.0); floodlight(k, 1.6, 20.0, z, 9.0, yaw=45); floodlight(k, 28.4, 28.4, z, 9.0, yaw=215)

def barracks(k, w, d):
    # Troop barracks: a two-storey accommodation block, a mess hall, a guard tower and the gate.
    z = pad(k, w, d, c=TARMAC)
    k.box(1.5, 1.8, z, 13.5, 6.4, 6.6, rgb(182, 176, 160, 'concrete')); k.box(1.44, 1.74, z, 13.62, 6.52, .6, CONC_DARK)
    gable(k, 1.5, 1.8, z + 6.6, 13.5, 6.4, 2.0, rgb(86, 98, 92, 'corr'), ridge='x', wall=rgb(182, 176, 160, 'concrete'))
    with face(k, 'y+', 1.5, 8.2, z):
        punched(k, 13.5, 2, bay=1.9, margin=1.0); door(k, 6.2, 1.3, c=TEAM)
        k.box(5.6, 0, 2.5, 2.5, 1.4, .15, TEAM)  # entrance canopy
        for x in (5.75, 7.95): k.strut((x, 1.3, 0), (x, 1.3, 2.5), .08, GALV)
    with face(k, 'x+', 15, 8.2, z): punched(k, 6.4, 2, bay=2.0, margin=1.2)
    for x in (4.0, 11.5): vent(k, x, 4.9, z + 8.3, .25, .9)
    # Mess hall with roof plant.
    top = block(k, 2, 11.2, 8.5, 6.6, 3.9, CLAD_SAND, plinth=CONC_DARK, floors=1, band=None)
    roof_plant(k, 2.4, 11.6, top, 7.6, 5.6, seed=8, density=1.3)
    with face(k, 'y+', 2, 17.8, z): door(k, 6.6)
    # Guard tower.
    for x, y in [(15.2, 12.2), (17.8, 12.2), (15.2, 14.8), (17.8, 14.8)]: k.tube((x, y, z), (x, y, z + 7.0), .12, TIMBER, n=4)
    for zz in (2.3, 4.6):
        k.strut((15.2, 14.8, z + zz - 2.2), (17.8, 14.8, z + zz), .08, TIMBER); k.strut((17.8, 12.2, z + zz - 2.2), (17.8, 14.8, z + zz), .08, TIMBER)
    k.box(14.7, 11.7, z + 7.0, 3.6, 3.6, .15, TIMBER)
    for bx, by, bw, bd in [(14.7, 15.2, 3.6, .1), (18.2, 11.7, .1, 3.6), (14.7, 11.7, 3.6, .1), (14.7, 11.7, .1, 3.6)]: k.box(bx, by, z + 7.15, bw, bd, 1.1, rgb(118, 104, 78, 'wood'))
    for x, y in [(14.8, 11.8), (18.1, 11.8), (14.8, 15.1), (18.1, 15.1)]: k.strut((x, y, z + 8.25), (x, y, z + 9.5), .08, TIMBER)
    gable(k, 14.5, 11.5, z + 9.5, 4.0, 4.0, .9, rgb(86, 98, 92, 'corr'), ridge='x')
    k.ladder(16.5, 15.45, z, z + 7.1, axis='x')
    with k.at(17.9, 15.0, z + 8.4, yaw=40): k.cbox(0, 0, 0, .5, .4, .4, DARK); k.box(.25, -.15, .05, .03, .3, .3, LAMP)
    # Gate post, sandbagged checkpoint, parade square markings, flag.
    sandbags(k, [(11.5, 17.6, z), (13.6, 17.6, z), (13.6, 15.8, z)], 4)
    sandbags(k, [(11.5, 13.0, z), (11.5, 14.6, z)], 3)
    k.box(11.9, 16.0, z, 1.3, 1.3, 2.3, rgb(92, 104, 86)); k.box(11.8, 15.9, z + 2.3, 1.5, 1.5, .12, DARK)
    k.box(13.2, 16.3, z + 1.0, .05, .7, .8, GLASS)
    k.tube((14.2, 18.6, z + 1.0), (19.4, 18.6, z + 1.2), .06, SIGNAL_RED, n=6); k.cbox(14.2, 18.6, z, .3, .3, 1.1, WHITE)
    for i in range(4): line(k, 11.6 + i * 1.9, 9.2, 1.2, .14, z)
    flagpole(k, 18.6, 17.4, z, 9.0, w=2.6)
    crate(k, 3.0, 9.0, z, 1.0); crate(k, 4.2, 9.1, z, 1.0, c=OLIVE); barrels(k, 16.0, 9.4, z, 3)
    floodlight(k, 18.9, 1.6, z, 8.0, yaw=135)

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
for _name in ('hd_buildings_a', 'hd_buildings_b', 'hd_buildings_c'):
    try:
        MODELS.update(__import__(_name).MODELS)
    except ImportError:
        pass

def build(m, ident, w, d):
    MODELS[ident](Kit(m, UB), w, d)
