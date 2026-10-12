"""Infantry and the attack dog, modelled in metres on a 1.8 m figure.

Figures are drawn at about three times vehicle scale, as every game of this kind does, or a
soldier would be four pixels tall. Within the figure the proportions are human (eight heads),
slightly sturdier in the head and limbs so that they survive the reduction. Each soldier is told
from the others by what real troops differ in: headgear, the cut and colour of the uniform, the
load on the back, and above all the weapon and how it is carried. The owner's colour is worn as
real identification is: brassards on both arms, a helmet band or beret, a recognition panel on
the pack.
"""
from contextlib import contextmanager
from math import sin, cos, pi, radians
from hd_kit import *

U = .38  # cells per metre

# Cloth and kit colours.
MULTICAM = rgb(124, 118, 86, 'camocloth'); DESERT = rgb(176, 156, 118, 'camocloth'); KHAKI = rgb(132, 118, 84, 'cloth')
SOV_UNI = rgb(98, 104, 66, 'camocloth'); GREATCOAT = rgb(98, 90, 72, 'cloth'); NAVY = rgb(46, 52, 66, 'cloth')
BLACK_CLOTH = rgb(38, 38, 42, 'cloth'); JEANS = rgb(62, 72, 96, 'cloth'); SHIRT = rgb(186, 180, 164, 'cloth')
COVERALL = rgb(170, 150, 104, 'cloth'); HIVIS = rgb(226, 120, 30, 'cloth'); HAZMAT = rgb(168, 170, 62, 'cloth')
SILVER = rgb(188, 192, 196, 'cloth'); PURPLE = rgb(84, 52, 96, 'cloth'); FLIGHT = rgb(84, 96, 104, 'cloth')
RUBBERISED = rgb(72, 66, 58, 'rubber'); VEST = rgb(66, 70, 50, 'cloth'); VEST_TAN = rgb(118, 96, 64, 'cloth')
WEB = rgb(86, 78, 54, 'cloth'); BOOT = rgb(36, 30, 27, 'leather'); BOOT_TAN = rgb(118, 92, 62, 'leather')
GLOVE = rgb(40, 40, 38, 'leather'); PAD = rgb(44, 46, 42, 'leather')
HELMET = rgb(112, 110, 80, 'camocloth'); HELMET_TAN = rgb(166, 148, 112, 'camocloth'); HELMET_SOV = rgb(70, 78, 54)
FUR = rgb(96, 84, 70, 'cloth'); HAIR = rgb(72, 50, 34, 'cloth'); HAIR_DARK = rgb(34, 28, 26, 'cloth')
SKIN2 = rgb(170, 128, 98, 'skin'); WOODSTOCK = rgb(120, 74, 42, 'wood'); GHILLIE = rgb(88, 96, 58, 'foliage')
DYNAMITE = rgb(168, 50, 40); LEATHER = rgb(92, 62, 40, 'leather'); GOLD_VISOR = rgb(176, 140, 60, 'glass')

# Joint positions of the standing figure (it faces +x; y is its left).
HIP, KNEE, SHO = .93, .5, 1.44

# --- Pose -----------------------------------------------------------------------------------------------
# POSE is None for the standing figure of the eight-facing sheet, or the frame of the walk cycle
# (generate_hd_art.unit_walk). The cycle is driven from the legs: each hip swings as a pendulum,
# the knee stays nearly straight while its foot is planted and folds while it swings forward, and
# the pelvis rides at whatever height puts the lower foot on the ground, which gives the bob of a
# real walk for free. Everything above the hips leans into the walk and turns against the legs.
POSE = None
WALK_FRAMES = 6
RUNNERS = ('tanya', 'terrorist', 'attack_dog')   # units of speed 20 and over (assets/data/units.ron) run
UNRIGGED = ('rocketeer',)                        # flies: it has no walk cycle
_RUN = False
L1, L2 = HIP - KNEE, KNEE - .1

def phase():
    return None if POSE is None else POSE / WALK_FRAMES

def gait(f):
    """Hip and knee angle in degrees at fraction f of one leg's cycle (0: heel strike, .5: toe-off)."""
    f %= 1.0
    swing, fold, brace = (37, 92, 18) if _RUN else (27, 62, 9)
    hip = swing * cos(2 * pi * f) + (9 if _RUN else 2)
    knee = 6 + (brace * sin(2 * pi * f) if f < .5 else fold * sin(2 * pi * (f - .5)))
    return hip, knee

def leg_joints(f):
    """Knee and ankle of a leg relative to its hip joint, as (x, z) pairs."""
    hip, knee = gait(f)
    a, b = radians(hip), radians(hip - knee)
    kx, kz = L1 * sin(a), -L1 * cos(a)
    return (kx, kz), (kx + L2 * sin(b), kz - L2 * cos(b))

def pelvis():
    f = phase()
    return HIP if f is None else .1 + max(-leg_joints(f + o)[1][1] for o in (0, .5))

def foot_pitch(f):
    """Toe up at heel strike, flat while planted, heel up into toe-off, levelling through the swing."""
    f %= 1.0
    if f < .08: return 14 * (1 - f / .08)
    if f < .34: return 0
    if f < .5: return -32 * (f - .34) / .16
    return -32 + 46 * (f - .5) / .5

@contextmanager
def swinging(k, s, amp=1.0):
    """A free arm on side s, and whatever its hand carries, swings against the leg below it."""
    if POSE is None:
        yield; return
    a = -(40 if _RUN else 22) * amp * cos(2 * pi * (phase() + (0 if s > 0 else .5)))
    with k.at(0, s * .225, SHO - .03, pitch=a):
        with k.at(0, -s * .225, -(SHO - .03)): yield

def limb(k, a, b, r0, r1, c):
    k.tube(a, b, r0, c, r2=r1, n=8)
    k.ball(a[0], a[1], a[2], r0 * 1.02, c, n=8, k=4)

def boot_shape(k, boot):
    boot = remat(boot, 'leather')   # worn leather: unlike a tyre, it is not caked in dust
    k.stack(.05, 0, [(0, .3, .13, .05), (.1, .3, .13, .05), (.2, .16, .12, .05, -.06)], boot, k=2)
    k.box(-.1, -.068, -.012, .31, .136, .03, tint(boot, .6))  # sole

def legs(k, c, boot=BOOT, stride=.07, spread=.11, kneel=False, r=.085, pads=False):
    """Two legs with boots; returns nothing. kneel: right knee on the ground."""
    if POSE is not None:
        hz, body = pelvis(), k.T
        k.T = k.base   # the legs stay under the hips; only what is above them leans and turns
        try:
            for s in (1, -1):
                f = phase() + (0 if s > 0 else .5)
                (kx, kz), (ax, az) = leg_joints(f)
                hip = (0, s * spread, hz); knee = (kx, s * spread * 1.05, hz + kz); ankle = (ax, s * spread * 1.1, hz + az)
                limb(k, hip, knee, r, r * .8, c); limb(k, knee, ankle, r * .8, r * .62, c)
                if pads: k.ball(knee[0] + r * .35, knee[1], knee[2], r * .8, PAD, n=8, k=4)
                with k.at(ankle[0], ankle[1], ankle[2] - .1, pitch=foot_pitch(f)): boot_shape(k, boot)
        finally:
            k.T = body
        return
    if kneel:
        # Left foot forward flat, right knee down, right shin back.
        limb(k, (0, spread, HIP - .32), (.34, spread, .52), r, r * .82, c); limb(k, (.34, spread, .52), (.3, spread, .1), r * .82, r * .66, c)
        k.cbox(.36, spread, 0, .3, .13, .13, boot)
        limb(k, (0, -spread, HIP - .32), (.12, -spread, .1), r, r * .82, c); limb(k, (.12, -spread, .1), (-.34, -spread, .1), r * .82, r * .66, c)
        k.cbox(-.4, -spread, 0, .14, .13, .28, boot)
        return
    for s, fwd in ((1, stride), (-1, -stride)):
        hip = (0, s * spread, HIP); knee = (fwd * .55 + .03, s * spread * 1.05, KNEE); ankle = (fwd, s * spread * 1.1, .1)
        limb(k, hip, knee, r, r * .8, c); limb(k, knee, ankle, r * .8, r * .62, c)
        if pads: k.ball(knee[0] + r * .35, knee[1], knee[2], r * .8, PAD, n=8, k=4)
        with k.at(fwd, s * spread * 1.1, 0): boot_shape(k, boot)

def torso(k, c, bulk=1.0, z0=0.0):
    k.stack(0, 0, [(HIP - .1 + z0, .2 * bulk, .34 * bulk, .08), (HIP + .12 + z0, .2 * bulk, .32 * bulk, .08),
                   (1.25 + z0, .24 * bulk, .4 * bulk, .1), (SHO + z0, .23 * bulk, .46 * bulk, .1), (SHO + .07 + z0, .14, .2, .06)], c, k=3)

def vest(k, c, bulk=1.0, z0=0.0, pouches=True):
    """Plate carrier with magazine pouches."""
    k.stack(.005, 0, [(1.06 + z0, .25 * bulk, .37 * bulk, .08), (1.3 + z0, .285 * bulk, .43 * bulk, .1), (SHO - .03 + z0, .27 * bulk, .4 * bulk, .1)], c, k=3)
    if pouches:
        for y in (-.11, 0, .11): k.box(.115 * bulk, y - .045, 1.05 + z0, .06, .09, .16, tint(c, .86))
        k.box(-.03, -.2 * bulk, HIP + .06 + z0, .06, .4 * bulk, .07, tint(c, .8))  # belt
        for s in (-1, 1): k.cbox(-.02, s * .215 * bulk, HIP + .0 + z0, .13, .07, .15, tint(c, .9))  # hip pouches
        k.cbox(-.1, .17 * bulk, 1.3 + z0, .07, .09, .2, BLACK); k.tube((-.1, .17 * bulk, 1.5 + z0), (-.12, .17 * bulk, 1.74 + z0), .008, BLACK, n=4)  # radio

def head(k, skin=SKIN, r=.118, z0=0.0, x=.01):
    k.tube((x, 0, SHO + .04 + z0), (x, 0, SHO + .16 + z0), .055, skin, n=8)
    k.ball(x, 0, SHO + .25 + z0, r, skin, rz=r * 1.12, n=12, k=6)
    return SHO + .25 + z0

def helmet(k, kind, c, zc, x=.01, band=True):
    """Headgear over a head centred at height zc."""
    r = .118
    if kind == 'pasgt':  # modern combat helmet: deep shell, brow rim, ear cut
        k.ball(x - .01, 0, zc + .02, r * 1.2, c, rz=r * 1.15, n=12, k=5, lo=-.25, hi=1.0)
        k.box(x + .09, -.1, zc + .0, .06, .2, .025, tint(c, .85))
        if band: k.rev((x - .01, 0, zc + .035), 'z', [(0, r * 1.22), (.055, r * 1.19)], TEAM, n=12)
        k.box(x + .075, -.07, zc + .09, .05, .14, .05, GLASS_DARK)  # goggles pushed up
    elif kind == 'ssh':  # Soviet steel helmet: rounder, flared rim
        k.ball(x, 0, zc + .02, r * 1.18, c, rz=r * 1.2, n=12, k=5, lo=-.12, hi=1.0)
        k.rev((x, 0, zc - .005), 'z', [(0, r * 1.26), (.02, r * 1.2)], c, n=12)
        if band: k.rev((x, 0, zc + .04), 'z', [(0, r * 1.2), (.055, r * 1.165)], TEAM, n=12)
    elif kind == 'ushanka':  # fur cap with tied-up flaps
        k.rev((x, 0, zc + .0), 'z', [(0, r * 1.22), (.1, r * 1.24), (.16, r * 1.0), (.17, .02)], c, n=12)
        for s in (-1, 1): k.box(x - .07, s * r * 1.2 - .02, zc - .02, .14, .04, .12, tint(c, .9))
        k.box(x + r * 1.18, -.04, zc + .04, .02, .08, .07, TEAM)
    elif kind == 'hardhat':
        k.ball(x, 0, zc + .03, r * 1.18, c, rz=r * 1.05, n=12, k=5, lo=0.0, hi=1.0)
        k.rev((x + .03, 0, zc + .02), 'z', [(0, r * 1.4), (.015, r * 1.4)], c, n=12)
    elif kind == 'beret':
        k.ball(x - .02, .02, zc + .09, r * 1.12, c, rz=r * .5, n=12, k=4)
    elif kind == 'fedora':
        k.rev((x, 0, zc + .06), 'z', [(0, r * 1.75), (.015, r * 1.7)], c, n=14)
        k.rev((x, 0, zc + .07), 'z', [(0, r * 1.05), (.1, r * .98), (.13, r * .8)], c, n=12)
        k.rev((x, 0, zc + .075), 'z', [(0, r * 1.07), (.03, r * 1.06)], TEAM, n=12)
    elif kind == 'boonie':
        k.rev((x, 0, zc + .05), 'z', [(0, r * 1.7), (.02, r * 1.3)], c, n=12)
        k.ball(x, 0, zc + .05, r * 1.1, c, rz=r * .8, n=10, k=4, lo=0.0, hi=1.0)
    elif kind == 'dome':  # sealed pressure helmet with a visor
        k.ball(x, 0, zc + .02, r * 1.4, c, rz=r * 1.45, n=14, k=7)
        k.rev((x, 0, zc - .12), 'z', [(0, r * 1.25), (.05, r * 1.3)], DARK, n=12)

def hair(k, c, zc, long_=False):
    k.ball(-.02, 0, zc + .03, .122, c, rz=.12, n=10, k=5, lo=-.1, hi=1.0)
    if long_: k.tube((-.1, 0, zc + .04), (-.24, 0, zc - .14), .045, c, r2=.02, n=6)

def arm(k, s, elbow, hand, c, skin=SKIN, r=.062, bare=False):
    """Arm on side s (+1 left, -1 right) through an elbow to a hand position."""
    sh = (0, s * .225, SHO - .03)
    limb(k, sh, elbow, r * 1.12, r * .95, skin if bare else c); limb(k, elbow, hand, r * .95, r * .78, skin if bare else c)
    k.ball(hand[0], hand[1], hand[2], r * .95, skin, n=8, k=4)
    if not bare: k.ball(elbow[0], elbow[1], elbow[2], r * 1.0, tint(c, .8), n=8, k=4)  # reinforced elbow
    return sh

def brassards(k, z0=0.0, r=.082):
    """The owner's colour on both upper arms."""
    for s in (-1, 1): k.rev((0, s * .245, SHO - .2 + z0), 'z', [(0, r), (.15, r)], TEAM, n=8)

def pack(k, c, x=-.2, z=1.08, w=.18, d=.3, h=.36, panel=True):
    k.stack(x, 0, [(z, w, d, .05), (z + h * .8, w, d, .06), (z + h, w * .7, d * .8, .05)], c, k=2)
    if panel: k.box(x - w / 2 - .012, -d * .32, z + h * .25, .02, d * .64, h * .45, TEAM)

# --- Weapons (built along +x from the grip at the origin) --------------------------------------------
def rifle(k, grip, yaw=12, pitch=-12, kind='m4'):
    with k.at(*grip, yaw=yaw, pitch=pitch):
        if kind == 'ak':
            k.box(-.12, -.022, -.01, .34, .044, .075, GUN); k.box(-.36, -.02, -.04, .25, .04, .07, WOODSTOCK)
            k.box(.22, -.022, .0, .2, .044, .05, WOODSTOCK); k.tube((.42, 0, .035), (.7, 0, .035), .012, GUN, n=6)
            k.tube((.3, 0, .06), (.52, 0, .06), .008, GUN, n=4)
            with k.at(.1, 0, -.01, pitch=-70): k.box(0, -.016, -.03, .2, .032, .055, GUN)
            k.box(-.05, -.016, -.11, .04, .032, .1, WOODSTOCK)
        elif kind == 'lmg':
            k.box(-.14, -.03, -.01, .42, .06, .09, GUN); k.box(-.4, -.022, -.03, .27, .044, .08, BLACK)
            k.tube((.28, 0, .04), (.82, 0, .04), .016, GUN, n=6); k.box(.28, -.025, .015, .3, .05, .05, BLACK)
            k.box(-.02, -.07, -.14, .16, .14, .13, OLIVE)  # belt box
            for s in (-1, 1): k.tube((.72, 0, .02), (.84, s * .1, -.2), .008, GUN, n=4)
            k.box(.0, -.02, .08, .14, .04, .04, GUN)
        elif kind == 'sniper':
            k.box(-.14, -.022, -.01, .4, .044, .07, GHILLIE); k.box(-.44, -.02, -.05, .3, .04, .09, GHILLIE)
            k.tube((.26, 0, .03), (1.0, 0, .03), .013, GUN, n=6); k.tube((.9, 0, .03), (1.02, 0, .03), .02, GUN, n=6)
            k.tube((-.04, 0, .1), (.28, 0, .1), .024, BLACK, n=8); k.rev((.28, 0, .1), 'x', [(0, .03), (.02, .03)], GLASS, n=8)
            for s in (-1, 1): k.tube((.7, 0, .02), (.78, s * .09, -.22), .007, GUN, n=4)
        elif kind == 'smg':
            k.box(-.08, -.02, -.01, .26, .04, .07, GUN); k.tube((.18, 0, .025), (.34, 0, .025), .013, GUN, n=6)
            k.box(.04, -.014, -.16, .035, .028, .15, GUN); k.box(-.22, -.012, .0, .14, .024, .03, GUN)
        else:  # m4 carbine
            k.box(-.1, -.022, -.01, .3, .044, .07, GUN); k.box(-.33, -.02, -.03, .23, .04, .065, BLACK)
            k.tube((.2, 0, .03), (.36, 0, .03), .026, BLACK, n=8); k.tube((.36, 0, .03), (.56, 0, .03), .011, GUN, n=6)
            k.box(.03, -.015, -.15, .045, .03, .14, GUN); k.box(-.04, -.015, -.1, .035, .03, .09, BLACK)
            k.box(-.02, -.014, .06, .1, .028, .045, BLACK); k.tube((.36, 0, .03), (.36, 0, .09), .006, GUN, n=4)

def launcher(k, base, length=1.1, r=.06, c=OLIVE, yaw=4, pitch=8, flare=True):
    """Shoulder-fired tube lying on the right shoulder."""
    with k.at(*base, yaw=yaw, pitch=pitch):
        prof = [(-length * .45, r * 1.25 if flare else r), (-length * .38, r), (length * .5, r), (length * .5, r * 1.2), (length * .55, r * 1.2)]
        k.rev((0, 0, 0), 'x', prof, c, n=10)
        k.rev((length * .55, 0, 0), 'x', [(0, r * .8), (.01, r * .8)], BLACK, n=8)
        k.box(.1, .0, -.14, .04, .03, .1, GUN); k.box(.28, .0, -.14, .04, .03, .1, GUN); k.box(.02, r, .0, .14, .05, .07, BLACK)

# --- Soldiers -------------------------------------------------------------------------------------------
def soldier(k, uniform, vestc=VEST, hat=('pasgt', HELMET), boot=BOOT, skin=SKIN, bulk=1.0, with_pack=True, packc=None):
    """The common infantryman, arms left to the caller. Returns the head-centre height."""
    legs(k, uniform, boot, pads=True)
    torso(k, uniform, bulk)
    if vestc: vest(k, vestc, bulk)
    zc = head(k, skin)
    if hat: helmet(k, hat[0], hat[1], zc)
    if with_pack: pack(k, packc or tint(vestc or uniform, .9))
    return zc

def rifle_arms(k, c, skin=GLOVE, kind='m4', bare=False):
    """Low-ready carry: right hand at the grip, left under the fore-end."""
    arm(k, -1, (-.03, -.3, 1.16), (.2, -.12, 1.12), c, skin, bare=bare)
    arm(k, 1, (.2, .26, 1.2), (.42, .04, 1.18), c, skin, bare=bare)
    rifle(k, (.2, -.1, 1.14), yaw=14, pitch=-8, kind=kind)

def rifleman(k):
    # Allied rifleman: combat helmet, plate carrier, M4 carbine at the low ready.
    soldier(k, MULTICAM, VEST, ('pasgt', HELMET)); rifle_arms(k, MULTICAM); brassards(k)

def gi(k):
    # Allied machine-gunner: desert uniform, M249 light machine gun with its belt box.
    soldier(k, DESERT, VEST_TAN, ('pasgt', HELMET_TAN), boot=BOOT_TAN)
    rifle_arms(k, DESERT, kind='lmg'); brassards(k)
    k.tube((-.26, -.12, 1.5), (-.26, .12, 1.5), .06, TARP, n=8)  # bedroll on the pack

def conscript(k):
    # Soviet conscript: greatcoat to the knee, fur cap, AK with wooden furniture.
    legs(k, SOV_UNI, BOOT)
    k.stack(0, 0, [(.52, .34, .46, .12), (HIP + .1, .24, .36, .09), (1.25, .26, .42, .1), (SHO, .25, .47, .1), (SHO + .07, .14, .2, .06)], GREATCOAT, k=3)
    k.box(-.03, -.2, HIP + .08, .06, .4, .07, LEATHER); k.box(.1, -.03, HIP + .07, .04, .06, .08, BRASS)
    zc = head(k); helmet(k, 'ushanka', FUR, zc)
    rifle_arms(k, GREATCOAT, kind='ak'); brassards(k)
    k.stack(-.2, 0, [(1.02, .14, .28, .05), (1.3, .12, .26, .05)], WEB, k=2); k.box(-.285, -.09, 1.08, .02, .18, .14, TEAM)

def grenadier(k):
    # Soviet grenadier: steel helmet, grenade satchels, a stick grenade drawn back to throw.
    soldier(k, SOV_UNI, WEB, ('ssh', HELMET_SOV), with_pack=False)
    arm(k, -1, (-.22, -.32, 1.5), (-.34, -.26, 1.78), SOV_UNI)
    k.tube((-.34, -.26, 1.76), (-.36, -.26, 2.0), .018, WOODSTOCK, n=6); k.tube((-.36, -.26, 1.98), (-.37, -.26, 2.12), .04, OLIVE, n=8)
    arm(k, 1, (.24, .26, 1.3), (.46, .16, 1.38), SOV_UNI)
    for s in (-1, 1): k.stack(-.02, s * .22, [(.95, .16, .12, .04), (1.15, .16, .12, .04)], KHAKI, k=2)
    for y in (-.1, .02): k.tube((.13, y, 1.08), (.13, y, 1.3), .03, OLIVE, n=6)
    k.box(-.16, -.14, 1.1, .03, .28, .22, TEAM); brassards(k)

def rocket_soldier(k):
    # Allied anti-tank soldier: launch tube on the shoulder, a spare round across the pack.
    soldier(k, MULTICAM, VEST, ('pasgt', HELMET))
    arm(k, -1, (.1, -.34, 1.3), (.26, -.22, 1.5), MULTICAM); arm(k, 1, (.22, .24, 1.3), (.44, -.08, 1.46), MULTICAM)
    launcher(k, (.1, -.2, 1.62), 1.15, .065, OLIVE)
    k.tube((-.3, -.2, 1.2), (-.3, .2, 1.52), .055, OLIVE, n=8); brassards(k)

def engineer(k):
    # Combat engineer: coveralls, hi-vis vest, hard hat, toolbox and a pipe wrench.
    legs(k, COVERALL, BOOT_TAN); torso(k, COVERALL)
    k.stack(.005, 0, [(1.02, .25, .37, .08), (1.3, .28, .43, .1), (SHO - .03, .265, .4, .1)], HIVIS, k=3)
    for z in (1.1, 1.3): k.rev((.005, 0, z), 'z', [(0, .225), (.035, .225)], rgb(220, 220, 210), n=10)
    zc = head(k); helmet(k, 'hardhat', rgb(226, 190, 40), zc)
    with swinging(k, -1, .6):
        arm(k, -1, (-.04, -.28, 1.14), (.06, -.27, .86), COVERALL)
        k.box(-.12, -.34, .52, .36, .15, .22, SIGNAL_RED); k.tube((-.08, -.27, .78), (.2, -.27, .78), .012, STEEL, n=4)
    arm(k, 1, (.14, .28, 1.2), (.34, .2, 1.1), COVERALL)
    k.tube((.34, .2, 1.08), (.34, .2, 1.5), .016, STEEL, n=6); k.box(.31, .18, 1.46, .1, .04, .08, STEEL)
    k.box(-.16, -.13, 1.12, .03, .26, .26, TEAM); brassards(k)

def spy(k):
    # Agent in a trench coat and hat, briefcase in hand.
    legs(k, BLACK_CLOTH, rgb(30, 26, 24, 'rubber'), stride=.12)
    coat = rgb(96, 86, 70, 'cloth')
    k.stack(0, 0, [(.46, .32, .44, .12), (HIP + .1, .23, .34, .09), (1.25, .25, .4, .1), (SHO, .24, .46, .1), (SHO + .07, .14, .2, .06)], coat, k=3)
    k.box(-.03, -.19, HIP + .1, .06, .38, .06, tint(coat, .7))
    k.box(.1, -.06, 1.3, .03, .12, .2, SHIRT); k.box(.118, -.02, 1.28, .02, .04, .2, TEAM)  # shirt and tie
    zc = head(k); helmet(k, 'fedora', rgb(66, 58, 50, 'cloth'), zc)
    k.box(.1, -.09, zc + .0, .03, .18, .04, GLASS_DARK)
    with swinging(k, -1, .6):
        arm(k, -1, (-.03, -.28, 1.14), (.04, -.28, .86), coat)
        k.box(-.14, -.33, .5, .4, .1, .28, LEATHER); k.tube((-.02, -.28, .8), (.12, -.28, .8), .012, DARK, n=4)
    with swinging(k, 1): arm(k, 1, (.02, .28, 1.14), (.1, .27, .88), coat)
    brassards(k)

def tanya(k):
    # Commando: no helmet, bare arms, twin pistols levelled.
    legs(k, rgb(72, 76, 60, 'cloth'), BOOT, stride=.2)
    torso(k, rgb(86, 92, 70, 'cloth'), .92)
    k.box(-.03, -.18, HIP + .06, .06, .36, .07, LEATHER)
    for s in (-1, 1): k.box(-.06, s * .2 - .04, HIP - .22, .1, .08, .2, LEATHER)  # thigh holsters
    k.tube((.1, -.16, 1.42), (-.1, .16, 1.08), .022, LEATHER, n=6)  # bandolier
    zc = head(k); hair(k, HAIR, zc, long_=True); helmet(k, 'beret', TEAM, zc)
    for s in (-1, 1):
        arm(k, s, (.26, s * .26, 1.4), (.56, s * .17, 1.46), SKIN, bare=True)
        k.box(.56, s * .17 - .015, 1.45, .2, .03, .05, GUN); k.box(.57, s * .17 - .015, 1.36, .04, .03, .1, GUN)
        k.rev((0, s * .245, SHO - .13), 'z', [(0, .075), (.06, .075)], TEAM, n=8)

def sniper(k):
    # Sniper in a ghillie suit, kneeling behind a long scoped rifle on its bipod.
    legs(k, GHILLIE, BOOT, kneel=True)
    walking = POSE is not None
    with k.at(0 if walking else .06, 0, 0 if walking else -.3, pitch=-4 if walking else -14):
        torso(k, GHILLIE, 1.05); zc = head(k); helmet(k, 'boonie', GHILLIE, zc)
        for i in range(9):
            a = i * 2.3
            k.ball(-.12 + .1 * cos(a), .2 * sin(a), 1.3 + .05 * i, .1, tint(GHILLIE, .85 + .04 * (i % 4)), rz=.06, n=6, k=3)
        k.rev((0, 0, zc + .04), 'z', [(0, .2), (.03, .2)], TEAM, n=10)
        if walking:
            arm(k, -1, (-.03, -.3, 1.16), (.2, -.12, 1.12), GHILLIE, GLOVE); arm(k, 1, (.2, .26, 1.2), (.42, .04, 1.18), GHILLIE, GLOVE)
            rifle(k, (.2, -.1, 1.14), yaw=16, pitch=-16, kind='sniper')
        else:
            arm(k, -1, (.1, -.3, 1.2), (.34, -.12, 1.32), GHILLIE, GLOVE); arm(k, 1, (.3, .26, 1.24), (.6, .06, 1.3), GHILLIE, GLOVE)
            rifle(k, (.34, -.1, 1.34), yaw=10, pitch=10, kind='sniper')
        brassards(k)

def flak_trooper(k):
    # Soviet anti-aircraft trooper: padded jacket, steel helmet, a shoulder-fired flak gun fed
    # from a drum, ammunition frame on the back.
    soldier(k, rgb(92, 92, 70, 'cloth'), WEB, ('ssh', HELMET_SOV), bulk=1.1, with_pack=False)
    arm(k, -1, (.1, -.36, 1.3), (.28, -.24, 1.5), SOV_UNI); arm(k, 1, (.24, .26, 1.3), (.5, -.06, 1.46), SOV_UNI)
    with k.at(.1, -.22, 1.64, yaw=4, pitch=24):
        k.rev((0, 0, 0), 'x', [(-.5, .06), (-.3, .075), (.2, .075), (.2, .045), (.95, .04), (.95, .06), (1.08, .06)], SOV_DARK, n=10)
        k.rev((.0, 0, -.16), 'y', [(-.05, .13), (.05, .13)], GUN, n=12)
        k.box(.1, .07, -.02, .16, .04, .1, BLACK)
    k.box(-.34, -.19, 1.02, .16, .38, .5, SOV_DARK)
    for y in (-.12, 0, .12): k.tube((-.26, y, 1.52), (-.26, y, 1.72), .04, BRASS, n=6)
    k.box(-.36, -.14, 1.12, .02, .28, .22, TEAM); brassards(k, r=.085)

def tesla_trooper(k):
    # Soviet Tesla trooper: a heavy insulated suit, sealed helmet with a slit visor, coil pack and
    # a discharge gauntlet.
    suit = RUBBERISED
    legs(k, suit, rgb(34, 32, 30, 'rubber'), r=.105, spread=.13)
    torso(k, suit, 1.22)
    k.stack(.005, 0, [(1.08, .32, .46, .1), (1.32, .36, .54, .12), (SHO, .33, .5, .12)], rgb(86, 80, 70, 'paint'), k=3)
    k.ball(.01, 0, SHO + .22, .17, rgb(86, 80, 70, 'paint'), rz=.19, n=14, k=7)
    k.box(.13, -.11, SHO + .2, .05, .22, .05, rgb(220, 180, 60, 'emit'))
    for s in (-1, 1): k.ball(0, s * .29, SHO, .13, rgb(86, 80, 70, 'paint'), n=10, k=5)
    # The pauldrons hide the arms, so the owner's colour goes on the helmet and the pauldron rims.
    k.rev((.01, 0, SHO + .27), 'z', [(0, .176), (.06, .172)], TEAM, n=14)
    for s in (-1, 1): k.rev((0, s * .29, SHO + .02), 'z', [(0, .134), (.05, .128)], TEAM, n=10)
    arm(k, -1, (.04, -.38, 1.14), (.36, -.24, 1.16), suit, suit, r=.085); arm(k, 1, (.1, .38, 1.14), (.3, .3, 1.02), suit, suit, r=.085)
    # Gauntlet: twin electrodes with a live tip.
    k.tube((.3, -.24, 1.16), (.62, -.2, 1.18), .07, DARK, n=8)
    for dz in (-.04, .04): k.tube((.6, -.2, 1.18 + dz), (.8, -.19, 1.19 + dz * 1.6), .016, COPPER, n=5)
    k.ball(.82, -.19, 1.19, .05, TESLA, n=8, k=4)
    # Coil pack.
    k.box(-.4, -.2, 1.02, .2, .4, .5, DARK)
    for y in (-.1, .1):
        k.cyl(-.3, y, 1.52, .035, .34, rgb(150, 96, 66), n=8)
        for i in range(4): k.torus(-.3, y, 1.58 + i * .07, .055, .014, COPPER, n=10, k=4)
        k.ball(-.3, y, 1.9, .05, TESLA, n=8, k=4)
    k.pipe([(-.3, -.2, 1.3), (-.1, -.36, 1.2), (.2, -.3, 1.16)], .022, RUBBER, n=5)
    k.box(-.42, -.14, 1.12, .02, .28, .24, TEAM); brassards(k, r=.1)

def chrono_legionnaire(k):
    # Allied chrono legionnaire: a silvered pressure suit with a gold visor, the chrono pack and
    # its hand-held projector.
    legs(k, SILVER, rgb(150, 154, 158, 'rubber'), r=.092)
    torso(k, SILVER, 1.1)
    k.stack(.005, 0, [(1.1, .28, .4, .09), (1.34, .31, .47, .11), (SHO - .02, .29, .44, .1)], rgb(206, 208, 210), k=3)
    zc = SHO + .25
    k.ball(.01, 0, zc, .16, rgb(206, 208, 210), rz=.17, n=14, k=7); k.ball(.07, 0, zc + .01, .115, GOLD_VISOR, ry=.12, rz=.1, n=12, k=5)
    k.rev((.0, 0, SHO + .06), 'z', [(0, .13), (.04, .15)], DARK, n=12)
    arm(k, -1, (0, -.32, 1.16), (.26, -.18, 1.14), SILVER, SILVER, r=.07); arm(k, 1, (.2, .28, 1.2), (.46, .04, 1.16), SILVER, SILVER, r=.07)
    with k.at(.26, -.14, 1.14, yaw=14, pitch=-4):
        k.box(-.12, -.05, -.04, .3, .1, .12, rgb(206, 208, 210)); k.rev((.18, 0, .02), 'x', [(0, .045), (.3, .045), (.3, .08), (.42, .1)], STEEL, n=10)
        k.rev((.42, 0, .02), 'x', [(0, .085), (.01, .08)], PLASMA, n=10)
        for i in range(3): k.torus(.26 + i * .06, 0, .02, .06, .012, COPPER, n=10, k=4, axis='x')
    k.stack(-.24, 0, [(1.04, .2, .34, .06), (1.5, .2, .34, .06), (1.56, .14, .26, .05)], rgb(206, 208, 210), k=2)
    k.torus(-.36, 0, 1.3, .12, .025, COPPER, n=14, k=5, axis='x'); k.ball(-.35, 0, 1.3, .07, PLASMA, n=8, k=4)
    k.pipe([(-.22, -.16, 1.2), (-.02, -.34, 1.12), (.2, -.2, 1.12)], .018, RUBBER, n=5)
    k.box(-.345, -.13, 1.46, .02, .26, .08, TEAM); brassards(k, r=.088)

def crazy_ivan(k):
    # Soviet demolitions man: long coat, fur cap, bandoliers of dynamite, a charge ready to throw.
    legs(k, SOV_UNI, BOOT)
    coat = rgb(84, 70, 54, 'cloth')
    k.stack(0, 0, [(.5, .34, .46, .12), (HIP + .1, .24, .36, .09), (1.25, .26, .42, .1), (SHO, .25, .47, .1), (SHO + .07, .14, .2, .06)], coat, k=3)
    zc = head(k); helmet(k, 'ushanka', FUR, zc)
    k.ball(.09, 0, zc - .07, .07, HAIR_DARK, rz=.06, n=8, k=4)  # beard
    for i in range(6):
        t = i / 5
        k.tube((.14 - .02 * abs(t - .5), -.18 + t * .36, 1.36 - t * .3), (.14, -.18 + t * .36, 1.48 - t * .3), .024, DYNAMITE, n=6)
    k.tube((.1, -.2, 1.44), (.1, .2, 1.08), .02, LEATHER, n=6)
    arm(k, -1, (-.2, -.34, 1.5), (-.3, -.3, 1.8), coat)
    for dy in (-.03, 0, .03): k.tube((-.33, -.3 + dy, 1.76), (-.33, -.3 + dy, 1.98), .024, DYNAMITE, n=6)
    k.box(-.37, -.345, 1.84, .08, .09, .05, BLACK); k.tube((-.33, -.3, 1.98), (-.3, -.3, 2.06), .006, WOODSTOCK, n=4)
    arm(k, 1, (.2, .28, 1.24), (.4, .24, 1.12), coat)
    k.box(.34, .2, .98, .16, .1, .14, BLACK)
    k.box(-.15, -.14, 1.1, .03, .28, .24, TEAM); brassards(k)

def yuri(k):
    # Psychic officer: a long high-collared coat, shaven head with the neural band, one hand raised.
    coat = PURPLE
    legs(k, BLACK_CLOTH, rgb(30, 26, 24, 'rubber'), stride=.08)
    k.stack(0, 0, [(.2, .44, .54, .14), (HIP + .1, .25, .36, .09), (1.25, .26, .42, .1), (SHO, .25, .47, .1), (SHO + .07, .16, .22, .06)], coat, k=3)
    k.rev((0, 0, SHO + .04), 'z', [(0, .13), (.12, .15)], tint(coat, .85), n=12)  # collar
    k.box(.11, -.015, .5, .02, .03, .95, BRASS); k.box(-.03, -.2, HIP + .12, .06, .4, .06, BLACK)
    zc = head(k, rgb(206, 176, 150, 'skin'))
    k.rev((.01, 0, zc + .04), 'z', [(0, .124), (.03, .122)], STEEL, n=12); k.ball(.12, 0, zc + .055, .03, PSI, n=6, k=3)
    k.ball(.1, 0, zc - .08, .05, HAIR_DARK, rz=.04, n=8, k=4)
    arm(k, -1, (.12, -.34, 1.3), (.34, -.3, 1.56), coat, rgb(206, 176, 150, 'skin')); 
    with swinging(k, 1, .7): arm(k, 1, (-.02, .29, 1.14), (.06, .27, .9), coat, rgb(206, 176, 150, 'skin'))
    k.ball(.38, -.3, 1.64, .05, PSI, n=8, k=4)
    for s in (-1, 1): k.box(-.1, s * .245 - .03, SHO - .02, .2, .06, .03, TEAM)  # shoulder boards
    k.box(-.15, -.12, 1.1, .03, .24, .3, TEAM); brassards(k)

def desolator(k):
    # Soviet desolator: full chemical suit and respirator, a glowing isotope tank feeding the
    # radiation projector.
    legs(k, HAZMAT, rgb(40, 44, 34, 'rubber'), r=.095)
    torso(k, HAZMAT, 1.14)
    zc = SHO + .25
    k.ball(.01, 0, zc, .14, HAZMAT, rz=.16, n=12, k=6)  # hood
    k.ball(.09, 0, zc, .09, BLACK, ry=.1, rz=.1, n=10, k=5)  # mask
    for s in (-1, 1):
        k.rev((.13, s * .04, zc + .03), 'x', [(0, .032), (.01, .03)], GLASS_GREEN, n=8)
        k.rev((.1, s * .09, zc - .07), (1, s * .8, -.3), [(0, .045), (.08, .045)], DARK, n=8)
    arm(k, -1, (0, -.33, 1.16), (.26, -.2, 1.12), HAZMAT, rgb(40, 44, 34, 'rubber'), r=.072); arm(k, 1, (.2, .28, 1.2), (.48, .04, 1.14), HAZMAT, rgb(40, 44, 34, 'rubber'), r=.072)
    with k.at(.26, -.16, 1.12, yaw=14, pitch=-4):
        k.rev((-.1, 0, 0), 'x', [(0, .05), (.5, .05), (.5, .075), (.62, .075), (.72, .11)], rgb(92, 96, 60), n=10)
        k.rev((.72, 0, 0), 'x', [(0, .095), (.01, .09)], TOXIC, n=10); k.box(.02, -.03, -.14, .05, .06, .12, DARK)
    k.cyl(-.3, 0, 1.0, .13, .5, rgb(92, 96, 60), n=12); k.ball(-.3, 0, 1.5, .13, rgb(92, 96, 60), rz=.07, n=12, k=4, lo=0, hi=1)
    k.rev((-.3, 0, 1.18), 'z', [(0, .135), (.14, .135)], TOXIC, n=12)
    k.pipe([(-.3, -.12, 1.5), (-.1, -.34, 1.3), (.2, -.2, 1.1)], .022, RUBBER, n=5)
    k.rev((-.3, 0, 1.4), 'z', [(0, .134), (.07, .134)], TEAM, n=12); brassards(k, r=.09)

def terrorist(k):
    # Irregular fighter: civilian clothes, a headscarf in the owner's colour, a satchel charge.
    legs(k, JEANS, rgb(70, 56, 44, 'rubber')); torso(k, SHIRT)
    k.stack(.005, 0, [(1.06, .25, .37, .08), (1.3, .28, .43, .1), (SHO - .04, .26, .4, .1)], rgb(84, 78, 62, 'cloth'), k=3)
    for y in (-.13, -.045, .045, .13): k.tube((.14, y, 1.1), (.14, y, 1.3), .024, DYNAMITE, n=6)
    k.tube((.15, -.16, 1.2), (.15, .16, 1.2), .008, BLACK, n=4)
    zc = head(k, SKIN2); k.ball(-.01, 0, zc + .03, .125, TEAM, rz=.11, n=10, k=5, lo=-.1, hi=1.0)
    k.plate([(-.1, -.03, zc + .02), (-.26, -.06, zc - .12), (-.1, .03, zc + .02)], TEAM)
    arm(k, -1, (-.04, -.3, 1.16), (.14, -.24, 1.0), SHIRT, SKIN2)
    k.box(.02, -.36, .76, .26, .14, .26, rgb(104, 96, 68, 'cloth')); k.tube((.06, -.28, 1.44), (.12, -.3, 1.0), .014, LEATHER, n=4)
    with swinging(k, 1, .5):
        arm(k, 1, (.16, .28, 1.22), (.36, .22, 1.14), SHIRT, SKIN2)
        k.box(.3, .18, 1.06, .14, .06, .1, BLACK); k.ball(.37, .21, 1.18, .02, RED_LAMP, n=6, k=3)
    brassards(k)

def rocketeer(k):
    # Allied rocketeer: flight suit and visored helmet under a twin-turbine jet pack, carbine
    # slung forward; he hangs in flight, legs trailing.
    with k.at(0, 0, .3, pitch=-14):
        for s, back in ((1, -.1), (-1, -.22)):
            limb(k, (0, s * .1, HIP), (back * .5, s * .1, .52), .085, .07, FLIGHT); limb(k, (back * .5, s * .1, .52), (back - .12, s * .1, .16), .07, .055, FLIGHT)
            k.stack(back - .1, s * .1, [(.02, .24, .12, .05), (.16, .2, .12, .05)], BOOT, k=2)
        torso(k, FLIGHT); vest(k, rgb(70, 78, 84, 'cloth'), pouches=False)
        zc = head(k); k.ball(.0, 0, zc + .02, .145, WHITE, rz=.15, n=12, k=6); k.ball(.07, 0, zc + .02, .1, GLASS_DARK, ry=.11, rz=.075, n=10, k=5)
        k.rev((0, 0, zc + .06), 'z', [(0, .147), (.035, .145)], TEAM, n=12)
        rifle_arms(k, FLIGHT, kind='smg'); brassards(k)
        # Jet pack: frame, two nacelles with intakes and hot nozzles, stub wings.
        k.box(-.3, -.16, 1.04, .14, .32, .5, DARK)
        for s in (-1, 1):
            k.rev((-.3, s * .26, .86), 'z', [(0, .06), (.1, .1), (.62, .1), (.72, .08)], AIR_GREY, n=12)
            k.rev((-.3, s * .26, 1.58), 'z', [(0, .075), (.01, .07)], BLACK, n=10)
            k.rev((-.3, s * .26, .84), 'z', [(0, .055), (.03, .058)], HOT, n=10)
            k.plate([(-.36, s * .36, 1.36), (-.2, s * .36, 1.36), (-.24, s * .78, 1.3), (-.36, s * .78, 1.3)], AIR_GREY)
            k.plate([(-.36, s * .6, 1.31), (-.24, s * .6, 1.31), (-.25, s * .77, 1.29), (-.36, s * .77, 1.29)], TEAM)
        k.box(-.38, -.12, 1.16, .02, .24, .26, TEAM)

def attack_dog(k):
    # German shepherd in a working harness, mid-stride.
    tan = rgb(150, 108, 62, 'cloth'); black = rgb(46, 38, 32, 'cloth')
    f = phase()
    with k.at(0, 0, 0 if f is None else .03 * abs(sin(2 * pi * f)), s=1.5):
        k.body([(-.42, 0, .5, .09, .1), (-.2, 0, .52, .12, .13), (.1, 0, .54, .13, .15), (.3, 0, .56, .11, .13), (.4, 0, .6, .08, .09)], tan, n=10)
        k.body([(-.4, 0, .58, .08, .05), (-.1, 0, .62, .1, .06), (.28, 0, .65, .09, .05)], black, n=8)  # saddle
        k.body([(.34, 0, .62, .08, .09), (.44, 0, .72, .075, .085), (.52, 0, .78, .08, .085), (.6, 0, .76, .055, .055), (.74, 0, .72, .035, .035)], tan, n=10)
        k.ball(.75, 0, .725, .025, BLACK, n=6, k=3)
        for s in (-1, 1):
            k.plate([(.46, s * .05, .84), (.5, s * .07, .97), (.54, s * .045, .85)], black)
            for x0, fwd, hz, lead in ((.28, .08, .5, 0), (-.34, -.1, .48, .5)):
                if f is None:
                    joint = (x0 + fwd, .26); paw = (x0 + fwd * .4 + .02, .03)
                else:
                    # Diagonal pairs strike together; a paw is lifted while it reaches forward.
                    g_ = (f + lead + (0 if s > 0 else .5)) % 1.0
                    reach = .2 * cos(2 * pi * g_); lift = .1 * max(0.0, sin(2 * pi * (g_ - .5)))
                    paw = (x0 + reach + (.04 if x0 > 0 else -.02), .03 + lift)
                    joint = (x0 + reach * .45 + fwd, .27 + lift * .7)
                k.tube((x0, s * .08, hz), (joint[0], s * .085, joint[1]), .045, tan, r2=.032, n=6); k.tube((joint[0], s * .085, joint[1]), (paw[0], s * .085, paw[1]), .03, tan, r2=.024, n=6)
                k.ball(paw[0] + .02, s * .085, paw[1], .035, tan, n=6, k=3)
        k.tube((-.42, 0, .54), (-.62, 0, .4), .035, tan, r2=.018, n=6); k.tube((-.62, 0, .4), (-.72, 0, .44), .018, black, r2=.008, n=5)
        k.rev((.02, 0, .54), 'x', [(0, .155), (.2, .16)], TEAM, n=10)  # harness
        k.rev((.36, 0, .62), (1, 0, .6), [(0, .09), (.03, .09)], BLACK, n=8)

MODELS = {f.__name__: f for f in [rifleman, gi, conscript, grenadier, rocket_soldier, engineer, spy, tanya, sniper,
                                  flak_trooper, tesla_trooper, chrono_legionnaire, crazy_ivan, yuri, desolator,
                                  terrorist, rocketeer, attack_dog]}

WALKERS = [ident for ident in MODELS if ident not in UNRIGGED]

def build(m, ident):
    global _RUN
    k = Kit(m, U); k.base = k.T
    _RUN = ident in RUNNERS
    if POSE is None or ident in UNRIGGED or ident == 'attack_dog':
        MODELS[ident](k); return
    f = phase()
    turn = -(10 if _RUN else 6) * cos(2 * pi * f)   # shoulders against the hips
    with k.at(0, 0, pelvis(), yaw=turn, pitch=-(12 if _RUN else 3)):
        with k.at(0, 0, -HIP): MODELS[ident](k)
