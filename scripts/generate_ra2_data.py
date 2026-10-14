#!/usr/bin/env python3
# Generates the shipped data files of the RA2 parity programme (docs/RA2_PARITY_PLAN.md section
# 3.1, docs/DECISIONS.md R9):
#
#     crates/sim/tests/reference/ra2_reference.tsv   (committed: file, section, key, value)
#     crates/sim/tests/reference/ra2_mapping.tsv     (committed: game id -> ini section)
#     crates/sim/tests/reference/ra2_combat.tsv      (committed: weapon, warhead id -> section)
#             |  this script, with its own tables of game specific values below
#             v
#     assets/data/{rules,units,economy,combat,arsenal,skirmish,ai}.ron
#
# It reads only the three committed tables: never the ini files. Every value ported from the
# reference is converted with the conversions of the plan's section 2 and written with a
# trailing comment `// [Section] Key=value` naming where it comes from. Values that are not
# ported (labels, descriptions, sprites, footprints, roles, the computer player's plans, the
# maps) come from the tables of this script and carry no such comment; footprints are ported by
# a later milestone. Build times are computed from the cost (RA2, docs/RA2_PARITY_PLAN.md
# section 4.2). Armour, warheads, weapons and which of them a type carries are ported since RA3
# (sections 4.3 and 4.4), with the third committed table `ra2_combat.tsv` (game id of a weapon or
# warhead -> ini section).
#
# The data parity audit (crates/sim/tests/ra9_data_parity.rs) recomputes the conversions in Rust
# from the same tables and fails on any shipped value that differs from its reference and is not
# listed in ra2_differences.tsv, so a change here that the reference does not support is caught.
#
# Usage (standard library only, Python 3):
#     python3 scripts/generate_ra2_data.py            write the seven files
#     python3 scripts/generate_ra2_data.py --check    exit 1 if a shipped file is stale
# Exit code 2 reports an error (a missing table, a reference the tables cannot resolve).
"""Writes assets/data/*.ron from the committed RA2 reference tables."""

import os
import sys
from fractions import Fraction
from math import isqrt

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REFERENCE_DIR = os.path.join(ROOT, "crates", "sim", "tests", "reference")
DATA_DIR = os.path.join(ROOT, "assets", "data")

# --- Game specific tables --------------------------------------------------------------------

# The houses of the ini in the order of [Countries], with the game's country id and label.
COUNTRIES = (
    ("Americans", "america", "America"),
    ("Alliance", "korea", "Korea"),
    ("French", "france", "France"),
    ("Germans", "germany", "Germany"),
    ("British", "great_britain", "Great Britain"),
    ("Africans", "libya", "Libya"),
    ("Arabs", "iraq", "Iraq"),
    ("Confederation", "cuba", "Cuba"),
    ("Russians", "russia", "Russia"),
)
# The ini's side names and prerequisite group tokens.
SIDES = {"GDI": "allied", "Nod": "soviet"}
GROUPS = (
    ("power", "PrerequisitePower"),
    ("barracks", "PrerequisiteBarracks"),
    ("factory", "PrerequisiteFactory"),
    ("radar", "PrerequisiteRadar"),
    ("tech", "PrerequisiteTech"),
    ("proc", "PrerequisiteProc"),
)
# The ini's `Factory=` values, with `Naval=yes` for a shipyard.
FACTORIES = {
    ("BuildingType", False): "buildings",
    ("InfantryType", False): "infantry",
    ("UnitType", False): "vehicles",
    ("UnitType", True): "naval",
    ("AircraftType", False): "aircraft",
}

# id: (label, description, category, domain, role, cooldown, radius, strength, capacity,
#      sprite, width, height)
WORLD_BUILDINGS = {
    'oil_derrick': ('Oil Derrick', 'Capture to earn credits.', 'buildings', 'land', 'combat', 100, 6, 20, 0, 'gun_turret', 1, 1),
    'hospital': ('Hospital', 'Capture; send own infantry here to heal.', 'buildings', 'land', 'combat', 100, 6, 20, 0, 'gun_turret', 1, 1),
    'airport': ('Airport', 'Capture for paradrop support.', 'buildings', 'land', 'combat', 100, 6, 20, 0, 'gun_turret', 1, 1),
    'tech_outpost': ('Tech Outpost', 'Capture for defence and one vehicle repair dock.', 'buildings', 'land', 'combat', 100, 6, 20, 0, 'gun_turret', 1, 1),
    'bridge_hut': ('Bridge Repair Hut', 'Send an Engineer to restore the adjacent bridge.', 'buildings', 'land', 'combat', 100, 6, 20, 0, 'gun_turret', 1, 1),
    'civilian_house': ('House', 'GI and Conscript garrison.', 'buildings', 'land', 'combat', 100, 6, 20, 0, 'gun_turret', 1, 1),
    'civilian_city': ('City Block', 'GI and Conscript garrison.', 'buildings', 'land', 'combat', 100, 6, 20, 0, 'gun_turret', 1, 1),
    'civilian_tower': ('Tower', 'GI and Conscript garrison.', 'buildings', 'land', 'combat', 100, 6, 20, 0, 'gun_turret', 1, 1),
}
BUILDINGS = {
    'construction_yard': ('Construction Yard', 'Base infrastructure.', 'buildings', 'land', 'combat', 100, 6, 20, 0,
        'construction_yard', 3, 3),
    'power_plant': ('Power Plant', 'Base infrastructure.', 'buildings', 'land', 'combat', 100, 6, 20, 0,
        'power_plant', 2, 2),
    'refinery': ('Ore Refinery', 'Base infrastructure.', 'buildings', 'land', 'combat', 100, 6, 20, 0,
        'refinery', 3, 3),
    'barracks': ('Barracks', 'Base infrastructure.', 'buildings', 'land', 'combat', 100, 6, 20, 0,
        'barracks', 2, 2),
    'war_factory': ('War Factory', 'Base infrastructure.', 'buildings', 'land', 'combat', 100, 6, 20, 0,
        'war_factory', 3, 3),
    'tech_center': ('Battle Lab', 'Base infrastructure.', 'buildings', 'land', 'combat', 100, 6, 20, 0,
        'tech_center', 2, 2),
    'tesla_reactor': ('Tesla Reactor', 'Base infrastructure.', 'buildings', 'land', 'combat', 100, 6, 20, 0,
        'power_plant', 2, 2),
    'ore_purifier': ('Ore Purifier', 'Increases ore delivery income by 25 percent while powered.', 'buildings', 'land', 'purifier', 100, 6, 20, 0,
        'tech_center', 2, 2),
    'nuclear_reactor': ('Nuclear Reactor', 'Base infrastructure.', 'buildings', 'land', 'combat', 100, 6, 20, 0,
        'power_plant', 2, 2),
    'naval_shipyard': ('Naval Shipyard', 'Place on water adjacent to shore. Produces naval units.', 'buildings', 'land', 'shipyard', 100, 6, 20, 0,
        'war_factory', 3, 3),
    'airforce_command': ('Airforce Command Headquarters', 'Base infrastructure.', 'buildings', 'land', 'combat', 100, 6, 20, 0,
        'war_factory', 3, 3),
    'service_depot': ('Service Depot', 'Repairs a damaged vehicle sent to it, for credits.', 'buildings', 'land', 'repair', 100, 6, 20, 0,
        'tech_center', 2, 2),
    'radar_tower': ('Radar Tower', 'Base infrastructure.', 'buildings', 'land', 'combat', 100, 6, 20, 0,
        'tech_center', 2, 2),
    'cloning_vats': ('Cloning Vats', 'Duplicates trained infantry.', 'buildings', 'land', 'cloner', 100, 6, 20, 0,
        'tech_center', 2, 2),
    'fortress_walls': ('Fortress Walls', 'Blocks ground movement.', 'armory', 'land', 'combat', 100, 6, 20, 0,
        'gun_turret', 1, 1),
    'pillbox': ('Pillbox', 'Automatically engages valid enemy targets.', 'armory', 'land', 'combat', 100, 6, 20, 0,
        'gun_turret', 1, 1),
    'sentry_gun': ('Sentry Gun', 'Automatically engages valid enemy targets.', 'armory', 'land', 'combat', 100, 6, 20, 0,
        'gun_turret', 1, 1),
    'patriot': ('Patriot Missile System', 'Automatically engages valid enemy targets.', 'armory', 'land', 'flak', 100, 6, 20, 0,
        'gun_turret', 1, 1),
    'flak_cannon': ('Flak Cannon', 'Automatically engages valid enemy targets.', 'armory', 'land', 'flak', 100, 6, 20, 0,
        'gun_turret', 1, 1),
    'prism_tower': ('Prism Tower', 'Automatically engages valid enemy targets.', 'armory', 'land', 'splash', 100, 6, 20, 0,
        'gun_turret', 1, 1),
    'tesla_coil': ('Tesla Coil', 'Automatically engages valid enemy targets.', 'armory', 'land', 'tesla', 100, 6, 20, 0,
        'gun_turret', 1, 1),
    'grand_cannon': ('Grand Cannon', 'Automatically engages valid enemy targets.', 'armory', 'land', 'splash', 100, 6, 20, 0,
        'tech_center', 2, 2),
    'gap_generator': ('Gap Generator', 'Shrouds this area against enemy vision.', 'armory', 'land', 'gap', 100, 6, 20, 0,
        'tech_center', 2, 2),
    'spy_satellite': ('Spy Satellite Uplink', 'Reveals the battlefield while powered.', 'armory', 'land', 'satellite', 100, 6, 20, 0,
        'tech_center', 2, 2),
    'psychic_sensor': ('Psychic Sensor', 'Detects concealed units nearby.', 'armory', 'land', 'sensor', 100, 6, 20, 0,
        'tech_center', 2, 2),
    'chronosphere': ('Chronosphere', 'Select vehicles, choose ability, then click a visible destination.', 'armory', 'land', 'chronosphere', 3600, 3, 0, 0,
        'tech_center', 2, 2),
    'weather_control': ('Weather Control Device', 'Click a visible area for a devastating lightning storm.', 'armory', 'land', 'storm', 6000, 5, 1600, 0,
        'war_factory', 3, 3),
    'iron_curtain': ('Iron Curtain Device', 'Click a visible area to protect friendly mechanical units for 10 seconds.', 'armory', 'land', 'curtain', 3600, 3, 200, 0,
        'tech_center', 2, 2),
    'nuclear_silo': ('Nuclear Missile Silo', 'Click a visible area for a nuclear strike.', 'armory', 'land', 'nuke', 6000, 6, 2500, 0,
        'war_factory', 3, 3),
}
# id: (label, description, category, domain, role, cooldown, radius, strength, capacity,
#      sprite)
UNITS = {
    'conscript': ('Conscript', 'Specialized ground combat. Select and right click an enemy to attack.', 'infantry', 'land', 'combat', 100, 2, 20, 0,
        'conscript'),
    'gi': ('GI', 'Specialized ground combat. Select and right click an enemy to attack.', 'infantry', 'land', 'combat', 100, 2, 20, 0,
        'rifleman'),
    'engineer': ('Engineer', 'E + click a nearby structure to capture or repair it.', 'infantry', 'land', 'engineer', 100, 2, 20, 0,
        'rifleman'),
    'attack_dog': ('Attack Dog', 'Specialized ground combat. Select and right click an enemy to attack.', 'infantry', 'land', 'dog', 100, 2, 20, 0,
        'scout'),
    'rocketeer': ('Rocketeer', 'Specialized air combat. Select and right click an enemy to attack.', 'infantry', 'air', 'combat', 100, 2, 20, 0,
        'rifleman'),
    'spy': ('Spy', 'E + click a nearby enemy building to infiltrate. Dogs detect spies.', 'infantry', 'land', 'spy', 100, 2, 20, 0,
        'rifleman'),
    'tanya': ('Tanya', 'Specialized ground combat. Select and right click an enemy to attack.', 'infantry', 'land', 'commando', 100, 2, 20, 0,
        'rifleman'),
    'chrono_legionnaire': ('Chrono Legionnaire', 'Teleports when given a move order; beam suppresses targets.', 'infantry', 'land', 'chrono', 100, 2, 20, 0,
        'rifleman'),
    'sniper': ('Sniper', 'Specialized ground combat. Select and right click an enemy to attack.', 'infantry', 'land', 'combat', 100, 2, 20, 0,
        'rifleman'),
    'flak_trooper': ('Flak Trooper', 'Specialized ground combat. Select and right click an enemy to attack.', 'infantry', 'land', 'flak', 100, 2, 20, 0,
        'rocket_soldier'),
    'tesla_trooper': ('Tesla Trooper', 'Specialized ground combat. Select and right click an enemy to attack.', 'infantry', 'land', 'tesla', 100, 2, 20, 0,
        'rocket_soldier'),
    'crazy_ivan': ('Crazy Ivan', 'Attaches a delayed explosive when attacking.', 'infantry', 'land', 'demolition', 100, 2, 800, 0,
        'grenadier'),
    'yuri': ('Yuri', 'Controls one enemy unit; control ends when Yuri dies.', 'infantry', 'land', 'mind_control', 100, 2, 20, 0,
        'rifleman'),
    'desolator': ('Desolator', 'D contaminates surrounding ground.', 'infantry', 'land', 'radiation', 100, 3, 20, 0,
        'grenadier'),
    'terrorist': ('Terrorist', 'Detonates on reaching attack range.', 'infantry', 'land', 'suicide', 100, 3, 500, 0,
        'conscript'),
    'chrono_miner': ('Chrono Miner', 'Harvests ore and teleports home to unload.', 'vehicles', 'land', 'chrono_miner', 100, 2, 20, 0,
        'harvester'),
    'war_miner': ('War Miner', 'Specialized ground combat. Select and right click an enemy to attack.', 'vehicles', 'land', 'combat', 100, 2, 20, 0,
        'harvester'),
    'grizzly': ('Grizzly Battle Tank', 'Specialized ground combat. Select and right click an enemy to attack.', 'vehicles', 'land', 'combat', 100, 2, 20, 0,
        'light_tank'),
    'rhino': ('Rhino Heavy Tank', 'Specialized ground combat. Select and right click an enemy to attack.', 'vehicles', 'land', 'combat', 100, 2, 20, 0,
        'heavy_tank'),
    'ifv': ('Infantry Fighting Vehicle (IFV)', 'Carries one infantry; passenger changes its weapon. D unloads.', 'vehicles', 'land', 'ifv', 100, 2, 20, 1,
        'scout'),
    'flak_track': ('Flak Track', 'Select passengers; E + click this transport to load. D unloads.', 'vehicles', 'land', 'transport', 100, 2, 20, 5,
        'raider'),
    'terror_drone': ('Terror Drone', 'Immobilizes and damages mechanical targets.', 'vehicles', 'land', 'drone', 100, 2, 20, 0,
        'scout'),
    'v3_launcher': ('V3 Rocket Launcher', 'Launches a rocket that flies to the target; anti air fire can shoot it down.', 'vehicles', 'land', 'combat', 100, 2, 20, 0,
        'rocket_launcher'),
    'mcv': ('Mobile Construction Vehicle (MCV)', 'D deploys into a Construction Yard on clear ground.', 'vehicles', 'land', 'mcv', 100, 2, 20, 0,
        'harvester'),
    'mirage_tank': ('Mirage Tank', 'Concealed while idle; revealed by nearby detectors.', 'vehicles', 'land', 'mirage', 100, 2, 20, 0,
        'light_tank'),
    'prism_tank': ('Prism Tank', 'Specialized ground combat. Select and right click an enemy to attack.', 'vehicles', 'land', 'splash', 100, 2, 20, 0,
        'artillery'),
    'apocalypse': ('Apocalypse Tank', 'Specialized ground combat. Select and right click an enemy to attack.', 'vehicles', 'land', 'combat', 100, 2, 20, 0,
        'heavy_tank'),
    'tank_destroyer': ('Tank Destroyer', 'Specialized ground combat. Select and right click an enemy to attack.', 'vehicles', 'land', 'combat', 100, 2, 20, 0,
        'artillery'),
    'tesla_tank': ('Tesla Tank', 'Specialized ground combat. Select and right click an enemy to attack.', 'vehicles', 'land', 'tesla', 100, 2, 20, 0,
        'heavy_tank'),
    'demolition_truck': ('Demolition Truck', 'Detonates on reaching attack range.', 'vehicles', 'land', 'suicide', 100, 3, 1600, 0,
        'harvester'),
    'harrier': ('Harrier', 'Fires one volley, then returns to its dock at an Airforce Command to reload.', 'vehicles', 'air', 'combat', 100, 2, 20, 0,
        'scout'),
    'black_eagle': ('Black Eagle', 'Fires one volley, then returns to its dock at an Airforce Command to reload.', 'vehicles', 'air', 'combat', 100, 2, 20, 0,
        'scout'),
    'nighthawk': ('NightHawk Transport', 'Select passengers; E + click this transport to load. D unloads.', 'vehicles', 'air', 'transport', 100, 2, 20, 5,
        'scout'),
    'kirov': ('Kirov Airship', 'Specialized air combat. Select and right click an enemy to attack.', 'vehicles', 'air', 'splash', 100, 2, 20, 0,
        'heavy_tank'),
    'amphibious_transport': ('Amphibious Transport', 'Select passengers; E + click this transport to load. D unloads.', 'vehicles', 'amphibious', 'transport', 100, 2, 20, 12,
        'harvester'),
    'destroyer': ('Destroyer', 'Detects submerged units and sends its Osprey against them.', 'vehicles', 'naval', 'combat', 100, 2, 20, 0,
        'artillery'),
    'aegis_cruiser': ('Aegis Cruiser', 'Specialized naval combat. Select and right click an enemy to attack.', 'vehicles', 'naval', 'flak', 100, 2, 20, 0,
        'artillery'),
    'dolphin': ('Dolphin', 'Specialized naval combat. Select and right click an enemy to attack.', 'vehicles', 'naval', 'combat', 100, 2, 20, 0,
        'artillery'),
    'aircraft_carrier': ('Aircraft Carrier', 'Launches its three Hornets at the target; lost ones are replaced.', 'vehicles', 'naval', 'combat', 100, 2, 20, 0,
        'artillery'),
    'typhoon': ('Typhoon Attack Submarine', 'Submerged: unseen unless a unit with sensors is near, or it has just fired.', 'vehicles', 'naval', 'combat', 100, 2, 20, 0,
        'artillery'),
    'sea_scorpion': ('Sea Scorpion', 'Specialized naval combat. Select and right click an enemy to attack.', 'vehicles', 'naval', 'flak', 100, 2, 20, 0,
        'artillery'),
    'giant_squid': ('Giant Squid', 'Immobilizes enemy vessels.', 'vehicles', 'naval', 'squid', 133, 2, 20, 0,
        'artillery'),
    'dreadnought': ('Dreadnought', 'Launches two missiles that fly to the target; anti air fire can shoot them down.', 'vehicles', 'naval', 'combat', 100, 2, 20, 0,
        'artillery'),
    # The spawned units of RA4 (docs/DECISIONS.md R50): launched by their carrier or launcher,
    # never produced and never commanded.
    'hornet': ('Hornet', 'Carrier aircraft: launched by its Aircraft Carrier, it bombs the target and returns.', 'vehicles', 'air', 'combat', 100, 2, 20, 0,
        'scout'),
    'osprey': ('Osprey', 'Destroyer aircraft: launched against submerged targets, it returns to its Destroyer.', 'vehicles', 'air', 'combat', 100, 2, 20, 0,
        'scout'),
    'v3_rocket': ('V3 Rocket', 'A rocket in flight. Anti air fire can shoot it down.', 'vehicles', 'air', 'combat', 100, 2, 20, 0,
        'rocket_launcher'),
    'dred_missile': ('Dreadnought Missile', 'A missile in flight. Anti air fire can shoot it down.', 'vehicles', 'air', 'combat', 100, 2, 20, 0,
        'rocket_launcher'),
}
# Items that show the art of another item (arsenal.ron `art`) instead of their own: the id whose
# miniature the game draws (docs/DECISIONS.md R52).
ART = {
    'hornet': 'harrier',
    'osprey': 'nighthawk',
}
# The tokens of `VeteranAbilities` and `EliteAbilities` the arsenal file knows
# (crates/sim/src/arsenal.rs, `Ability`).
ABILITIES = ("STRONGER", "FIREPOWER", "ROF", "SIGHT", "FASTER", "SELF_HEAL", "SCATTER")
# The missiles: the arsenal's `missile` by the key of [General] that names the type.
MISSILES = (("v3", "V3RocketType"), ("dmisl", "DMislType"))
# The armour types in the order of a warhead's `Verses` (docs/RA2_PARITY_PLAN.md appendix D).
ARMOURS = (
    "none", "flak", "plate", "light", "medium", "heavy", "wood", "steel", "concrete",
    "special_1", "special_2",
)
# Weapons of the ini that no type of the game carries yet, with the reason; a slot that names
# one is empty, and ra2_differences.tsv lists it. Everything else a mapped section names as
# `Primary`, `Secondary`, `ElitePrimary`, `EliteSecondary`, `Weapon1` or `EliteWeapon1` is
# carried.
NOT_CARRIED = {
    "none": "no weapon",
    "VirtualScanner": "NeverUse=yes: the engine never fires it",
    "MakeupKit": "the spy's disguise (RA6)",
    "DefuseKit": "the engineer's bomb disposal (RA6)",
    "AssaultBolt": "the charging of Tesla Coils (RA6)",
    "PsiWave": "the psi wave of a deployed Yuri (RA6)",
    "RadEruptionWeapon": "the eruption of a deployed Desolator, the game's radiation zone (RA6)",
    "Sapper": "C4 on ships (RA6)",
    "FakeC4": "C4 (RA6)",
    "PrismSupport": "the support beam of the prism chain (RA6)",
    "HornetCollision": "a spawned aircraft attacks only what its carrier fires at, never an aircraft",
    "ASWCollision": "a spawned aircraft attacks only what its destroyer fires at, never an aircraft",
}
# The largest range of the combat file (crates/sim/src/combat_defs.rs), in subcells.
MAX_WEAPON_RANGE = 7680
# Values of a weapon that are the game's own instead of the ini's, each a row of
# ra2_differences.tsv: stand-ins for mechanics of a later milestone (docs/DECISIONS.md R36).
# A value is a number, or (section, key, conversion) of the reference it is taken from; a
# warhead is the ini name of a mapped warhead.
#
# * The launchers (RA4, docs/DECISIONS.md R50): their shot launches a missile or an aircraft and
#   deals nothing, and the ini gives them the warhead `Special`, which is no warhead section.
#   What a launcher may fire at is decided by a warhead all the same, so each names the warhead
#   of what it launches. The destroyer's launcher has the ini's range -2 (unlimited), which is
#   the largest range the combat file admits here, and must engage what is at sea although the
#   ini's virtual projectile engages nothing.
# * The mechanics of RA6: a dog's bite and the neutron rifle keep the damage of M9 (the ini's
#   number is not hit points: a bite kills, a rifle erases), the sonic beam keeps M9's damage
#   per shot (the ini's is per frame of the wave), the bomb of Crazy Ivan names the warhead it
#   goes off with, and the Demolition Truck needs the smallest range the combat file admits.
WEAPON_VALUES = {
    "V3Launcher": {"warhead": "V3WH"},
    "DredLauncher": {"warhead": "DMISLWH"},
    "HornetLauncher": {"warhead": "ORCAAP"},
    "ASWLauncher": {"warhead": "APSplash", "range": MAX_WEAPON_RANGE, "anti_ground": 1},
    "GoodTeeth": {"damage": 200},
    "NeutronRifle": {"damage": 60},
    "NeutronRifleE": {"damage": 60},
    "SonicZap": {"damage": 100},
    "SonicZapE": {"damage": 100},
    "IvanBomber": {"warhead": "IvanWH"},
    "IvanBomberE": {"warhead": "IvanWH"},
    "Demobomb": {"range": 384},
}
# The area effects that are no weapon of a type (crates/sim/src/combat_defs.rs, `Effect`):
# id, base damage and warhead, each a number or the (section, key) of the reference.
# The charge of a commando and the pulse of a radiation zone are mechanics of M9 that RA6
# replaces: their damage is the game's own.
EFFECTS = (
    ("nuke", ("NukePayload", "Damage"), ("NukePayload", "Warhead")),
    ("lightning", ("General", "LightningDamage"), ("General", "LightningWarhead")),
    ("ivan_bomb", ("CombatDamage", "IvanDamage"), ("CombatDamage", "IvanWarhead")),
    ("c4", 200, ("CombatDamage", "C4Warhead")),
    ("radiation", 20, ("Radiation", "RadSiteWarhead")),
    # RA4: the impact of the spawned missiles, and of each when its launcher is elite.
    ("v3_rocket", ("General", "V3RocketDamage"), ("CombatDamage", "V3Warhead")),
    ("v3_rocket_elite", ("General", "V3RocketEliteDamage"), ("CombatDamage", "V3EliteWarhead")),
    ("dmisl", ("General", "DMislDamage"), ("CombatDamage", "DMislWarhead")),
    ("dmisl_elite", ("General", "DMislEliteDamage"), ("CombatDamage", "DMislEliteWarhead")),
)
# RA5: the rule values the ini has no key for (docs/RA2_PARITY_PLAN.md section 4.10; appendix D,
# topic 8: assumptions from the overlay count and the ancestor engine), by group of rules.ron.
OWN_RULES = {
    "economy": (
        ("ore_max_bails", 12, "the game's own: 12 density stages of an ore cell"),
        ("ore_spread_above", 6, "the game's own: ore spreads from a cell above this density"),
    ),
}

# RA5 (docs/RA2_PARITY_PLAN.md section 4.10): a miner takes one bail from its cell after every
# HARVEST_INTERVAL ticks of work. The ini has no key for it (assumption: the 14 frames of the
# other `.016` minute rates, x 4 / 3); its capacity is the ini's `Storage`.
HARVEST_RATE = 1
HARVEST_INTERVAL = 19

# The ids RA1 adds (docs/RA2_PARITY_PLAN.md section 3.3): id: (label, the id whose game specific
# values and art it takes until it has its own). Docs/DECISIONS.md R11.
NEW_BUILDINGS = {
    "soviet_construction_yard": ("Soviet Construction Yard", "construction_yard"),
    "soviet_refinery": ("Soviet Ore Refinery", "refinery"),
    "soviet_barracks": ("Soviet Barracks", "barracks"),
    "soviet_war_factory": ("Soviet War Factory", "war_factory"),
    "soviet_naval_shipyard": ("Soviet Naval Shipyard", "naval_shipyard"),
    "american_airforce_command": ("American Airforce Command", "airforce_command"),
    "soviet_service_depot": ("Soviet Service Depot", "service_depot"),
    "soviet_battle_lab": ("Soviet Battle Lab", "tech_center"),
    "soviet_walls": ("Soviet Walls", "fortress_walls"),
}
NEW_UNITS = {
    "soviet_mcv": ("Soviet MCV", "mcv"),
    "soviet_amphibious_transport": ("Soviet Amphibious Transport", "amphibious_transport"),
    "chrono_commando": ("Chrono Commando", "tanya"),
    "psi_commando": ("Psi Commando", "tanya"),
    "chrono_ivan": ("Chrono Ivan", "crazy_ivan"),
    "yuri_prime": ("Yuri Prime", "yuri"),
}
# The free cells allowed between a shipyard and a base building of its owner, instead of the
# ini's `Adjacent=12`: on the shipped maps water for a shipyard lies within 12 free cells of the
# Construction Yard at 28 of 176 start positions (the first start of 2 of 21 maps), within 31 at
# 132 (18 of 21); the maps are not reshaped for it (docs/DECISIONS.md R25, an intentional
# difference listed in ra2_differences.tsv).
SHIPYARD_ADJACENT = 31
# Mapped ids that enter the game in a later milestone, by the note of ra2_mapping.tsv.
LATER = ("support",)
# Units the ini produces only with stolen technology: in the data, not producible until RA6.
STOLEN = ("RequiresStolenAlliedTech", "RequiresStolenSovietTech")

# The computer player (docs/DECISIONS.md R14): a plan per side from ground units and base
# defences that every country of the side can build, and the three difficulties.
AI_PLANS = (
    (
        "allied",
        (
            "power_plant", "refinery", "barracks", "war_factory", "pillbox", "power_plant",
            "refinery", "pillbox", "war_factory", "barracks", "power_plant", "pillbox",
        ),
        "power_plant",
        "chrono_miner",
        ("grizzly", "gi", "grizzly", "gi", "grizzly", "ifv"),
    ),
    (
        "soviet",
        (
            "tesla_reactor", "soviet_refinery", "soviet_barracks", "soviet_war_factory",
            "sentry_gun", "tesla_reactor", "soviet_refinery", "sentry_gun", "soviet_war_factory",
            "soviet_barracks", "tesla_reactor", "sentry_gun",
        ),
        "tesla_reactor",
        "war_miner",
        ("rhino", "conscript", "rhino", "tesla_trooper", "rhino", "conscript"),
    ),
)
# id, think_interval, harvesters_per_refinery, max_army, wave_size, first_wave, wave_patience,
# defence_radius
AI_DIFFICULTIES = (
    ("easy", 40, 1, 8, 6, 6000, 3600, 8),
    ("medium", 20, 2, 16, 8, 3600, 2400, 10),
    ("brutal", 10, 2, 30, 10, 2400, 1800, 12),
)

# The maps of the skirmish menu and their start positions, as `cargo run -p game --example
# gen_maps -- starts` prints them.
MAPS = (
    ("demo", ((15, 37), (52, 42), (4, 1), (1, 19), (16, 54), (21, 2))),
    ("crossroads", ((9, 9), (44, 44), (3, 34), (50, 19), (18, 31), (35, 22))),
    ("riverlands", ((8, 21), (53, 24), (5, 3), (56, 42))),
    ("liberty_bay", ((29, 29), (224, 224), (25, 113), (228, 140), (113, 25), (140, 228), (159, 59), (94, 194), (59, 159), (194, 94))),
    ("frostline", ((49, 41), (172, 180), (41, 117), (180, 104), (121, 29), (100, 192), (161, 25), (60, 196), (31, 165), (190, 56))),
    ("oasis_gulf", ((29, 89), (224, 100), (69, 33), (184, 156), (121, 33), (132, 156), (181, 33), (72, 156), (225, 45), (28, 144))),
    ("coral_archipelago", ((41, 41), (212, 212), (129, 25), (124, 228), (25, 113), (228, 140), (193, 57), (60, 196), (229, 17), (24, 236))),
    ("continental_divide", ((33, 33), (220, 220), (201, 33), (52, 220), (73, 81), (180, 172), (125, 25), (128, 228), (25, 125), (228, 128))),
    ("steel_harbor", ((29, 29), (224, 224), (25, 217), (228, 36), (125, 25), (128, 228), (181, 25), (72, 228), (69, 189), (184, 64))),
    ("glacier_pass", ((25, 29), (212, 208), (21, 101), (216, 136), (25, 197), (212, 40), (49, 153), (188, 84))),
    ("red_canyon", ((17, 17), (172, 172), (13, 101), (176, 88), (101, 13), (88, 176), (147, 41), (42, 148))),
    ("jungle_delta", ((25, 33), (228, 188), (201, 25), (52, 196), (129, 21), (124, 200), (29, 153), (224, 68))),
    ("pacific_atoll", ((71, 71), (182, 182), (45, 141), (208, 112), (141, 45), (112, 208))),
    ("iron_isthmus", ((45, 45), (208, 176), (109, 37), (144, 184), (33, 109), (220, 112))),
    ("arctic_floes", ((45, 45), (192, 192), (137, 37), (100, 200), (37, 137), (200, 100))),
    ("ember_caldera", ((95, 33), (158, 220), (33, 95), (220, 158), (159, 33), (94, 220), (33, 159), (220, 94))),
    ("frozen_sound", ((29, 31), (224, 222), (221, 31), (32, 222), (125, 27), (128, 226), (77, 25), (176, 228), (173, 25), (80, 228))),
    ("harvest_plains", ((29, 29), (224, 224), (221, 37), (32, 216), (125, 21), (128, 232), (69, 81), (184, 172), (181, 77), (72, 176))),
    ("black_water", ((33, 33), (220, 220), (215, 45), (38, 208), (125, 25), (128, 228), (37, 125), (216, 128), (120, 70), (133, 183))),
    ("thousand_lakes", ((31, 31), (222, 222), (215, 41), (38, 212), (125, 23), (128, 230), (35, 121), (218, 132), (170, 90), (83, 163))),
    ("wheel_of_worlds", ((32, 96), (221, 157), (186, 46), (67, 207), (68, 46), (185, 207), (222, 96), (31, 157), (127, 27), (126, 226))),
)

# The scalar values of rules.ron: group, field, ini section, ini key, conversion, and for a key
# that holds a list the index of the entry. Conversions (docs/RA2_PARITY_PLAN.md section 2):
#   int      the number itself            percent  `50%` or `.25` as percent
#   pm       a ratio as permille          pm_pct   `150%` as permille
#   minutes  game minutes as ticks        frames   frames as ticks (at least 1)
#   cells    cells as subcells            flag     yes, true -> 1; no, false -> 0
RULES = (
    ("production", "build_ticks_per_1000", "General", "BuildSpeed", "minutes", None),
    ("production", "multiple_factory_pm", "General", "MultipleFactory", "pm", None),
    ("production", "wall_build_time_pm", "General", "WallBuildSpeedCoefficient", "pm", None),
    ("production", "low_power_penalty_pm", "General", "LowPowerPenaltyModifier", "pm", None),
    ("production", "min_low_power_speed_pm", "General", "MinLowPowerProductionSpeed", "pm", None),
    ("production", "max_low_power_speed_pm", "General", "MaxLowPowerProductionSpeed", "pm", None),
    ("production", "max_queued_objects", "General", "MaximumQueuedObjects", "int", None),
    ("economy", "refund_percent", "General", "RefundPercent", "percent", None),
    ("economy", "repair_percent", "General", "RepairPercent", "percent", None),
    ("economy", "repair_step", "General", "RepairStep", "int", None),
    ("economy", "repair_interval", "General", "RepairRate", "minutes", None),
    ("economy", "unit_repair_interval", "General", "URepairRate", "minutes", None),
    ("economy", "infantry_repair_step", "General", "IRepairStep", "int", None),
    ("economy", "infantry_repair_interval", "General", "IRepairRate", "minutes", None),
    ("economy", "reload_interval", "General", "ReloadRate", "minutes", None),
    ("economy", "purifier_bonus_percent", "General", "PurifierBonus", "percent", None),
    ("economy", "ore_value", "Riparius", "Value", "int", None),
    ("economy", "gem_value", "Cruentus", "Value", "int", None),
    ("economy", "ore_growth_interval", "Riparius", "Growth", "frames", None),
    ("economy", "ore_growth_pm", "Riparius", "GrowthPercentage", "pm", None),
    ("economy", "ore_spread_interval", "Riparius", "Spread", "frames", None),
    ("economy", "ore_spread_pm", "Riparius", "SpreadPercentage", "pm", None),
    ("economy", "harvesters_per_refinery", "General", "HarvestersPerRefinery", "int", None),
    ("veterancy", "veteran_ratio_pm", "General", "VeteranRatio", "pm", None),
    ("veterancy", "veteran_cap", "General", "VeteranCap", "int", None),
    ("veterancy", "combat_pm", "General", "VeteranCombat", "pm", None),
    ("veterancy", "speed_pm", "General", "VeteranSpeed", "pm", None),
    ("veterancy", "sight_pm", "General", "VeteranSight", "pm", None),
    ("veterancy", "armor_pm", "General", "VeteranArmor", "pm", None),
    ("veterancy", "rof_pm", "General", "VeteranROF", "pm", None),
    ("veterancy", "initial_veteran", "General", "InitialVeteran", "flag", None),
    ("crew", "crew_escape_percent", "General", "CrewEscape", "percent", None),
    ("crew", "survivor_rate_pm", "General", "SurvivorRate", "pm", None),
    ("combat", "max_damage", "CombatDamage", "MaxDamage", "int", None),
    ("combat", "min_damage", "CombatDamage", "MinDamage", "int", None),
    ("combat", "condition_yellow_percent", "AudioVisual", "ConditionYellow", "percent", None),
    ("combat", "condition_red_percent", "AudioVisual", "ConditionRed", "percent", None),
    ("combat", "ballistic_scatter", "CombatDamage", "BallisticScatter", "cells", None),
    ("combat", "homing_scatter", "CombatDamage", "HomingScatter", "cells", None),
    ("combat", "c4_delay", "CombatDamage", "C4Delay", "minutes", None),
    ("combat", "ivan_damage", "CombatDamage", "IvanDamage", "int", None),
    ("combat", "ivan_delay", "CombatDamage", "IvanTimedDelay", "frames", None),
    ("combat", "v3_rocket_damage", "General", "V3RocketDamage", "int", None),
    ("combat", "v3_rocket_elite_damage", "General", "V3RocketEliteDamage", "int", None),
    ("combat", "dmisl_damage", "General", "DMislDamage", "int", None),
    ("combat", "dmisl_elite_damage", "General", "DMislEliteDamage", "int", None),
    ("combat", "bridge_strength", "CombatDamage", "BridgeStrength", "int", None),
    ("combat", "cloak_delay", "General", "CloakDelay", "minutes", None),
    ("combat", "chrono_distance_factor", "General", "ChronoDistanceFactor", "int", None),
    ("combat", "chrono_minimum_delay", "General", "ChronoMinimumDelay", "frames", None),
    ("combat", "spy_power_blackout", "General", "SpyPowerBlackout", "frames", None),
    ("combat", "spy_money_steal_percent", "General", "SpyMoneyStealPercent", "percent", None),
    ("combat", "tree_strength", "General", "TreeStrength", "int", None),
    ("radiation", "application_delay", "Radiation", "RadApplicationDelay", "frames", None),
    ("radiation", "level_delay", "Radiation", "RadLevelDelay", "frames", None),
    ("radiation", "level_factor_pm", "Radiation", "RadLevelFactor", "pm", None),
    ("radiation", "level_max", "Radiation", "RadLevelMax", "int", None),
    ("radiation", "duration_multiple_pm", "Radiation", "RadDurationMultiple", "pm", None),
    ("superweapons", "nuclear_recharge", "MultiSpecial", "RechargeTime", "minutes", None),
    ("superweapons", "lightning_recharge", "LightningStormSpecial", "RechargeTime", "minutes", None),
    ("superweapons", "iron_curtain_recharge", "IronCurtainSpecial", "RechargeTime", "minutes", None),
    ("superweapons", "chronosphere_recharge", "ChronoSphereSpecial", "RechargeTime", "minutes", None),
    ("superweapons", "paradrop_recharge", "ParaDropSpecial", "RechargeTime", "minutes", None),
    ("superweapons", "american_paradrop_recharge", "AmericanParaDropSpecial", "RechargeTime", "minutes", None),
    ("superweapons", "iron_curtain_duration", "CombatDamage", "IronCurtainDuration", "frames", None),
    ("superweapons", "lightning_deferment", "General", "LightningDeferment", "frames", None),
    ("superweapons", "lightning_duration", "General", "LightningStormDuration", "frames", None),
    ("superweapons", "lightning_damage", "General", "LightningDamage", "int", None),
    ("superweapons", "lightning_hit_delay", "General", "LightningHitDelay", "frames", None),
    ("superweapons", "lightning_scatter_delay", "General", "LightningScatterDelay", "frames", None),
    ("superweapons", "lightning_cell_spread", "General", "LightningCellSpread", "int", None),
    ("superweapons", "lightning_separation", "General", "LightningSeparation", "int", None),
    ("superweapons", "american_paradrop_count", "General", "AmerParaDropNum", "int", None),
    ("superweapons", "paradrop_radius", "General", "ParadropRadius", "int", None),
    ("prism", "support_delay", "General", "PrismSupportDelay", "frames", None),
    ("prism", "support_duration", "General", "PrismSupportDuration", "frames", None),
    ("prism", "support_max", "General", "PrismSupportMax", "int", None),
    ("prism", "support_modifier_pm", "General", "PrismSupportModifier", "pm_pct", None),
    ("prism", "support_height", "General", "PrismSupportHeight", "int", None),
    ("crates", "maximum", "CrateRules", "CrateMaximum", "int", None),
    ("crates", "minimum", "CrateRules", "CrateMinimum", "int", None),
    ("crates", "radius", "CrateRules", "CrateRadius", "cells", None),
    ("crates", "regen", "CrateRules", "CrateRegen", "minutes", None),
    ("crates", "free_mcv", "CrateRules", "FreeMCV", "flag", None),
    ("dialog", "min_credits", "MultiplayerDialogSettings", "MinMoney", "int", None),
    ("dialog", "credits", "MultiplayerDialogSettings", "Money", "int", None),
    ("dialog", "max_credits", "MultiplayerDialogSettings", "MaxMoney", "int", None),
    ("dialog", "credits_step", "MultiplayerDialogSettings", "MoneyIncrement", "int", None),
    ("dialog", "min_unit_count", "MultiplayerDialogSettings", "MinUnitCount", "int", None),
    ("dialog", "unit_count", "MultiplayerDialogSettings", "UnitCount", "int", None),
    ("dialog", "max_unit_count", "MultiplayerDialogSettings", "MaxUnitCount", "int", None),
    ("dialog", "tech_level", "MultiplayerDialogSettings", "TechLevel", "int", None),
    ("dialog", "bases", "MultiplayerDialogSettings", "Bases", "flag", None),
    ("dialog", "short_game", "MultiplayerDialogSettings", "ShortGame", "flag", None),
    ("dialog", "crates", "MultiplayerDialogSettings", "Crates", "flag", None),
    ("dialog", "mcv_redeploys", "MultiplayerDialogSettings", "MCVRedeploys", "flag", None),
    ("dialog", "bridge_destruction", "MultiplayerDialogSettings", "BridgeDestruction", "flag", None),
    ("dialog", "fog_of_war", "MultiplayerDialogSettings", "FogOfWar", "flag", None),
    ("dialog", "shroud", "MultiplayerDialogSettings", "Shroud", "flag", None),
    ("dialog", "max_players", "Maximums", "Players", "int", None),
)
RULE_GROUPS = (
    "production", "economy", "veterancy", "crew", "combat", "radiation", "superweapons", "prism",
    "crates",
)
# The crate contents with a share above 0: game id, key of [Powerups], conversion of the
# argument (the fourth entry of the value; none: 0).
POWERUPS = (
    ("money", "Money", "int"),
    ("unit", "Unit", None),
    ("veteran", "Veteran", "int"),
    ("heal_base", "HealBase", None),
    ("armour", "Armor", "pm"),
    ("firepower", "Firepower", "pm"),
    ("speed", "Speed", "pm"),
    ("reveal", "Reveal", None),
)
# The computer player's difficulties: game id, the ini's difficulty section (named from the
# human's point of view, plan section 4.15), and the index in the lists of [General], which
# are ordered hard, medium, easy.
DIFFICULTIES = (("easy", "Difficult", 2), ("medium", "Normal", 1), ("brutal", "Easy", 0))
DIFFICULTY_FIELDS = (
    ("rof_pm", None, "ROF", "pm"),
    ("build_time_pm", None, "BuildTime", "pm"),
    ("credits_bonus_percent", "General", "MultiplayerAICM", "int"),
    ("virtual_purifiers", "General", "AIVirtualPurifiers", "int"),
    ("team_delay", "General", "TeamDelays", "frames"),
    ("allied_base_defences", "General", "AlliedBaseDefenseCounts", "int"),
    ("soviet_base_defences", "General", "SovietBaseDefenseCounts", "int"),
)
# What belongs to a side: field, key of [General] for the Allied and for the Soviet side.
SIDE_FIELDS = (
    ("crew", "AlliedCrew", "SovietCrew", "unit"),
    ("survivor_divisor", "AlliedSurvivorDivisor", "SovietSurvivorDivisor", "int"),
    ("disguise", "AlliedDisguise", "SovietDisguise", "unit"),
    ("paradrop_infantry", "AllyParaDropInf", "SovParaDropInf", "unit"),
    ("paradrop_count", "AllyParaDropNum", "SovParaDropNum", "int"),
)


class Failure(Exception):
    """A table the script cannot work with."""


# --- Conversions (docs/RA2_PARITY_PLAN.md sections 2 and 4.1) ----------------------------------


def number(text):
    """A decimal of the ini (`.7`, `1.5`, `50%` without its sign) as an exact fraction."""
    text = text.rstrip("%")
    if text.startswith("."):
        text = "0" + text
    return Fraction(text)


def rounded(value):
    """Half away from zero, for the positive values used: floor(x + 1/2)."""
    return (value + Fraction(1, 2)).__floor__()


def convert(kind, text):
    if kind == "int":
        return int(text)
    if kind == "percent":
        value = number(text)
        return rounded(value if text.endswith("%") else value * 100)
    if kind == "pm":
        return rounded(number(text) * 1000)
    if kind == "pm_pct":
        return rounded(number(text) * 10)
    if kind == "minutes":
        return rounded(number(text) * 1200)
    if kind == "frames":
        return max(1, rounded(number(text) * 4 / 3))
    if kind == "cells":
        return rounded(number(text) * 256)
    if kind == "flag":
        if text.lower() in ("yes", "true"):
            return 1
        if text.lower() in ("no", "false"):
            return 0
    raise Failure(f"cannot convert {text!r} as {kind}")


JUMPJET = "{92612C46-"


def speed(section, infantry):
    """Subcells per tick of a unit section, and the key it comes from (section 4.1).

    `infantry`: whether `[InfantryTypes]` lists the section, the one test of "infantry on foot"
    in the pipeline (the arsenal's `infantry` and the audit use the same list)."""
    if section["Locomotor"].startswith(JUMPJET):
        key, leptons = "JumpjetSpeed", int(section["JumpjetSpeed"])
    else:
        key, leptons = "Speed", min(int(section["Speed"]) * 256 // 100, 255)
        if infantry:
            leptons = leptons * 3 // 2
    return leptons_per_tick(leptons), key


def leptons_per_tick(leptons):
    """Leptons per frame as subcells per tick (section 2: 15 frames are 20 ticks), at least 1."""
    return max(1, rounded(Fraction(leptons * 3, 4)))


# --- The tables --------------------------------------------------------------------------------


def read_tsv(name, header):
    path = os.path.join(REFERENCE_DIR, name)
    try:
        with open(path, encoding="ascii") as handle:
            rows = [line.rstrip("\n").split("\t") for line in handle]
    except OSError as error:
        raise Failure(f"{path}: {error}") from error
    if not rows or tuple(rows[0]) != header:
        raise Failure(f"{path}: the header is not {header}")
    return rows[1:]


class Tables:
    def __init__(self):
        self.rules = {}
        for file, section, key, value in read_tsv(
            "ra2_reference.tsv", ("file", "section", "key", "value")
        ):
            if file == "rules":
                self.rules.setdefault(section, {})[key] = value
        self.mapping = read_tsv(
            "ra2_mapping.tsv", ("game_id", "kind", "section", "side", "note")
        )
        # The first section of a game id is the one its values are ported from; a `twin`
        # section is the same item on the other side.
        self.primary = {}
        self.sections = {}
        for game_id, kind, section, side, note in self.mapping:
            if note in LATER:
                continue
            self.sections.setdefault(game_id, []).append(section)
            if note != "twin":
                self.primary[game_id] = (kind, section, side, note)
        self.by_section = {
            section: game_id
            for game_id, _, section, _, note in self.mapping
            if note not in LATER
        }
        self.house_side = {}
        for ini_side, game_side in SIDES.items():
            for house in self.value("Sides", ini_side).split(","):
                self.house_side[house] = game_side
        self.house_country = {house: country for house, country, _ in COUNTRIES}
        # Weapons and warheads: the game id of every ini section (two sections may share
        # one, as the teeth of the two dogs do), and the ids in the order of the table.
        self.weapon_id, self.warhead_id = {}, {}
        self.weapons, self.warheads = [], []
        for game_id, kind, section in read_tsv("ra2_combat.tsv", ("game_id", "kind", "section")):
            ids, order = {
                "weapon": (self.weapon_id, self.weapons),
                "warhead": (self.warhead_id, self.warheads),
            }[kind]
            ids[section] = game_id
            if game_id not in [known for known, _ in order]:
                order.append((game_id, section))

    def section(self, name):
        if name not in self.rules:
            raise Failure(f"the reference has no section [{name}]")
        return self.rules[name]

    def value(self, section, key):
        values = self.section(section)
        if key not in values:
            raise Failure(f"the reference has no [{section}] {key}")
        return values[key]

    def game_id(self, section):
        if section not in self.by_section:
            raise Failure(f"[{section}] is not a mapped section")
        return self.by_section[section]

    def twins(self, game_id):
        """Whether the game item stands for a section of each side (the engineer, the dog)."""
        return len(self.sections[game_id]) > 1

    def listed(self, registry, section):
        """The entry `key=section` of the type list `registry`, or None if it has none."""
        for key, value in sorted(self.section(registry).items()):
            if value == section:
                return f"[{registry}] {key}={value}"
        return None


def cite(section, key, values):
    return f"[{section}] {key}={values[key]}"


def quoted(names):
    return "[" + ", ".join(f'"{name}"' for name in names) + "]"


class Item:
    """A unit or building of the game with the values ported from its primary section."""

    def __init__(self, tables, game_id):
        self.id = game_id
        self.kind, self.section_name, side, note = tables.primary[game_id]
        section = tables.section(self.section_name)
        self.ini = section
        twins = tables.twins(game_id)
        self.side = "none" if twins else side
        self.cost = int(section.get("Cost", "0"))
        self.hit_points = int(section["Strength"])
        self.sight = int(section.get("Sight", "0"))
        self.tech_level = -1 if any(key in section for key in STOLEN) else int(section["TechLevel"])
        houses = [house for house in section.get("Owner", "").split(",") if house]
        self.owner_sides = [
            side
            for side in ("allied", "soviet")
            if any(tables.house_side.get(house) == side for house in houses)
        ]
        countries = lambda key: (
            []
            if twins
            else [tables.house_country[house] for house in section.get(key, "").split(",") if house]
        )
        self.required = countries("RequiredHouses")
        self.forbidden = countries("ForbiddenHouses")
        self.prerequisites = []
        self.groups = []
        tokens = {token.upper(): group for group, _ in GROUPS for token in (group,)}
        for token in section.get("Prerequisite", "").split(","):
            if not token:
                continue
            if token.upper() in tokens:
                self.groups.append(tokens[token.upper()])
            else:
                self.prerequisites.append(tables.game_id(token))
        power = int(section.get("Power", "0"))
        self.power_produced, self.power_consumed = max(power, 0), max(-power, 0)
        self.free_units = [tables.game_id(section["FreeUnit"])] if "FreeUnit" in section else []
        self.factory = "none"
        if "Factory" in section:
            self.factory = FACTORIES[(section["Factory"], section.get("Naval") == "yes")]
        self.naval = section.get("Naval") == "yes"
        # The traits of RA2 (docs/RA2_PARITY_PLAN.md sections 4.2 and 4.6), buildings only.
        flag = lambda key, default: convert("flag", section.get(key, default))
        building = self.kind == "building"
        self.needs_engineer = flag("NeedsEngineer", "no")
        self.capturable = flag("Capturable", "no")
        self.insignificant = flag("Insignificant", "no")
        self.immune = flag("Immune", "no")
        self.hospital = flag("Hospital", "no")
        self.bridge_repair_hut = flag("BridgeRepairHut", "no")
        self.occupier = flag("Occupier", "no")
        self.can_be_occupied = flag("CanBeOccupied", "no")
        self.max_occupants = int(section.get("MaxNumberOccupants", "0"))
        self.vehicle_docks = int(section.get("NumberOfDocks", "0")) if flag("UnitRepair", "no") else 0
        self.cash_startup = int(section.get("ProduceCashStartup", "0"))
        self.cash_amount = int(section.get("ProduceCashAmount", "0"))
        self.cash_delay = convert("frames", section["ProduceCashDelay"]) if "ProduceCashDelay" in section else 0
        self.wall = building and flag("Wall", "no") == 1
        self.defence = building and section.get("BuildCat") == "Combat"
        self.build_limit = int(section.get("BuildLimit", "0"))
        self.adjacent = int(section.get("Adjacent", "2")) if building else 0
        # A shipyard reaches farther than the ini lets it (SHIPYARD_ADJACENT).
        self.far_shipyard = building and self.naval and "Factory" in section
        if self.far_shipyard:
            self.adjacent = SHIPYARD_ADJACENT
        self.base_normal = flag("BaseNormal", "yes") if building else 0
        self.radar = flag("Radar", "no") if building else 0
        self.sellable = int(building and not self.wall and flag("Unsellable", "no") == 0)
        self.repairable = flag("Repairable", "yes") if building else 0
        self.powered = flag("Powered", "no") if building else 0
        self.unit_repair = flag("UnitRepair", "no") if building else 0
        self.special = section.get("SuperWeapon") if building else None
        self.is_powered = (
            convert("flag", tables.value(self.special, "IsPowered")) if self.special else 0
        )
        # The traits of RA3 (docs/DECISIONS.md R6), units only: the type list the unit is in,
        # `Crusher` (default no), `Crushable` (default yes for infantry, no otherwise) and
        # `Size` (default 1).
        unit = self.kind == "unit"
        self.infantry_list = tables.listed("InfantryTypes", self.section_name) if unit else None
        self.infantry = int(self.infantry_list is not None)
        self.crusher = flag("Crusher", "no") if unit else 0
        self.crushable = (
            flag("Crushable", "yes" if self.infantry else "no") if unit else 0
        )
        self.size = int(section.get("Size", "1")) if unit else 0
        # The traits of RA4 (docs/DECISIONS.md R46). `Trainable` is yes for a unit and no for
        # a building without the key (ModEnc); the abilities are the tokens of the two lists.
        self.trainable = flag("Trainable", "yes") if unit else 0
        self.abilities = {}
        for key in ("VeteranAbilities", "EliteAbilities"):
            tokens = [token.upper() for token in section.get(key, "").split(",") if token]
            for token in tokens:
                if token not in ABILITIES:
                    raise Failure(f"{game_id}: the arsenal has no ability {token}")
            self.abilities[key] = [token.lower() for token in tokens] if unit else []
        self.crewed = flag("Crewed", "no")
        self.self_healing = flag("SelfHealing", "no") if unit else 0
        # `Ammo=-1` (the default) is unlimited, which the arsenal writes as 0.
        self.ammo = max(0, int(section.get("Ammo", "-1"))) if unit else 0
        self.airport_bound = flag("AirportBound", "no") if unit else 0
        self.cloakable = flag("Cloakable", "no") if unit else 0
        self.sensors_sight = (
            int(section.get("SensorsSight", "0")) if flag("Sensors", "no") else 0
        )
        self.spawns = tables.game_id(section["Spawns"]) if unit and "Spawns" in section else None
        self.spawns_number = int(section["SpawnsNumber"]) if self.spawns else 0
        # A rate of 0 frames stays 0 (the missiles never return to be reloaded).
        ticks = lambda key: (
            convert("frames", section[key]) if number(section.get(key, "0")) > 0 else 0
        )
        self.spawn_regen = ticks("SpawnRegenRate") if self.spawns else 0
        self.spawn_reload = ticks("SpawnReloadRate") if self.spawns else 0
        # The launcher is the weapon with `Spawner=yes`.
        self.launcher = "none"
        for slot, key in (("primary", "Primary"), ("secondary", "Secondary")):
            weapon = section.get(key)
            if self.spawns and weapon and weapon in tables.rules:
                if convert("flag", tables.rules[weapon].get("Spawner", "no")):
                    self.launcher = slot
        if self.spawns and self.launcher == "none":
            raise Failure(f"{game_id}: it spawns units and has no weapon with Spawner=yes")
        self.missile = "none"
        self.missile_key = None
        for missile, key in MISSILES:
            if unit and tables.value("General", key) == self.section_name:
                self.missile, self.missile_key = missile, key

    def build_time(self, tables):
        """Ticks of build time (section 4.2): the cost times `BuildSpeed` game minutes per 1000
        credits, at least 1, and for a wall times `WallBuildSpeedCoefficient`."""
        per_1000 = convert("minutes", tables.value("General", "BuildSpeed"))
        ticks = max(1, rounded(Fraction(self.cost * per_1000, 1000)))
        if self.wall:
            wall = convert("pm", tables.value("General", "WallBuildSpeedCoefficient"))
            ticks = max(1, rounded(Fraction(ticks * wall, 1000)))
        return ticks

    def comment(self, *keys):
        """The ported keys of the item's section as `[Section] Key=value, Key=value`."""
        values = ", ".join(f"{key}={self.ini[key]}" for key in keys if key in self.ini)
        return f"[{self.section_name}] {values}".rstrip()


def game_tables(tables):
    """The game specific values of every item: the existing ids, and the new ones from their
    twins with their own label and the twin's art."""
    buildings, units = {}, {}
    for game_id, values in BUILDINGS.items():
        buildings[game_id] = values + ("pillbox" if game_id.startswith("civilian_") else game_id,)
    for game_id, (label, twin) in NEW_BUILDINGS.items():
        buildings[game_id] = (label,) + BUILDINGS[twin][1:] + (twin,)
    for game_id, values in WORLD_BUILDINGS.items():
        buildings[game_id] = values + ("pillbox" if game_id.startswith("civilian_") else game_id,)
    for game_id, values in UNITS.items():
        units[game_id] = values + (ART.get(game_id, game_id),)
    for game_id, (label, twin) in NEW_UNITS.items():
        units[game_id] = (label,) + UNITS[twin][1:] + (twin,)
    for game_id, (kind, _, _, _) in tables.primary.items():
        if game_id not in (buildings if kind == "building" else units):
            raise Failure(f"{game_id} is mapped but has no game specific values")
    for game_id in list(buildings) + list(units):
        if game_id not in tables.primary:
            raise Failure(f"{game_id} has game specific values but is not mapped")
    return buildings, units


class Data:
    def __init__(self):
        self.tables = Tables()
        self.buildings, self.units = game_tables(self.tables)
        self.items = {game_id: Item(self.tables, game_id) for game_id in self.tables.primary}
        self.refund = convert("percent", self.tables.value("General", "RefundPercent"))
        self.pad_aircraft = [
            self.tables.game_id(section)
            for section in self.tables.value("General", "PadAircraft").split(",")
        ]
        # RA4: a building has the docks of its `NumberOfDocks` if an `AirportBound` aircraft
        # names it as its `Dock`; every such aircraft must name the same buildings, since the
        # arsenal lets an airport bound aircraft land on every building with docks.
        docks = None
        for item in self.items.values():
            item.docks = 0
            if item.airport_bound:
                named = sorted(
                    self.tables.game_id(section) for section in item.ini["Dock"].split(",")
                )
                if docks is not None and docks != named:
                    raise Failure(f"{item.id}: its docks are not those of the other aircraft")
                docks = named
        for game_id in docks or []:
            self.items[game_id].docks = int(self.items[game_id].ini["NumberOfDocks"])
            self.items[game_id].dock_users = ", ".join(
                f"[{item.section_name}] Dock={item.ini['Dock']}"
                for item in self.items.values()
                if item.airport_bound
            )

    # The factory of a side that produces the items of a factory group.
    def factory(self, group, side):
        candidates = [
            game_id
            for game_id in self.buildings
            if self.items[game_id].factory == group
        ]
        for game_id in candidates:
            if self.items[game_id].side == side:
                return game_id
        if not candidates:
            raise Failure(f"no factory of the group {group}")
        return candidates[0]

    def producer(self, game_id):
        item = self.items[game_id]
        side = "allied" if item.side == "none" else item.side
        if item.kind == "building":
            return self.factory("buildings", side)
        if self.units[game_id][2] == "infantry":
            return self.factory("infantry", side)
        if item.naval:
            return self.factory("naval", side)
        if game_id in self.pad_aircraft:
            return self.factory("aircraft", side)
        return self.factory("vehicles", side)

    # --- units.ron ---------------------------------------------------------------------------

    def units_file(self):
        lines = [
            "// Unit definitions, schema version 1 (PLAN.md section 5; docs/DECISIONS.md D18).",
            "//",
            "// id:     unique name, 1 to 32 characters from a-z, 0-9, and _, starting with a letter",
            "// speed:  subcells per tick, 1 to 128 (256 subcells per cell, 20 ticks per second;",
            "//         cells per second = speed x 20 / 256), converted from the ini's `Speed`",
            "//         (docs/RA2_PARITY_PLAN.md section 4.1, docs/DECISIONS.md R7)",
            "// sprite: name of the sprite sheet assets/sprites/units/<sprite>.png (8 facings of 64x64 px)",
            "//",
            GENERATED,
            "(",
            "    schema_version: 1,",
            "    units: [",
        ]
        for game_id, values in self.units.items():
            item = self.items[game_id]
            value, key = speed(item.ini, bool(item.infantry))
            source = item.comment(key)
            if key == "Speed" and item.infantry_list:
                source += f"; {item.infantry_list}"
            lines.append(
                f'        (id: "{game_id}", speed: {value}, sprite: "{values[9]}"),'
                f" // {source}"
            )
        lines += ["    ],", ")"]
        return "\n".join(lines) + "\n"

    # --- economy.ron -------------------------------------------------------------------------

    def economy_file(self):
        refund = cite("General", "RefundPercent", self.tables.section("General"))
        speed = cite("General", "BuildSpeed", self.tables.section("General"))
        wall = cite("General", "WallBuildSpeedCoefficient", self.tables.section("General"))
        lines = [
            "// Economy definitions, schema version 2 (PLAN.md sections 5 and 7, M3; docs/DECISIONS.md D23,",
            "// R58).",
            "//",
            "// placement_distance: a new building must lie within this many cells (Chebyshev distance between",
            "//                     the nearest cells) of a building of its owner",
            "// buildings:  id and sprite (the sprite is assets/sprites/buildings/<sprite>.png), footprint",
            "//             width and height in cells, cost in credits, build_time in ticks (20 per second),",
            "//             producer (a building of whose factory group produces it, arsenal.ron),",
            "//             prerequisites (building types the player must own to queue it; the prerequisite",
            "//             groups are in arsenal.ron), power produced and consumed, sell_value (credits",
            f"//             refunded when sold: the cost times {refund}), and free_units",
            "//             (units that leave it when it is placed)",
            "// units:      the production terms of the unit types of units.ron",
            "// harvesters: of the harvester unit types the capacity in bails (`Storage`), the bails removed",
            "//             from a cell at a time, and the ticks of work on the cell for each removal",
            "// refineries: credits paid per bail of ore and per bail of gems by the refinery building types",
            "// refunds:    the building types whose sale returns a fixed amount instead of the cost times",
            f"//             {refund} (`Soylent`, where it is not 0)",
            "//",
            "// Cost, prerequisites, power, sell value and free units are ported from the ini. The build",
            f"// time is the cost times {speed} game minutes per 1000 credits, in ticks",
            f"// (round(cost x 21 / 25), at least 1), and for a wall times {wall}",
            "// (docs/RA2_PARITY_PLAN.md section 4.2). A miner's capacity, the value of a bail and the",
            "// fixed refunds are ported (section 4.10); footprints and the pace of harvesting are the",
            "// game's own.",
            "//",
            GENERATED,
            "(",
            "    schema_version: 2,",
            "    placement_distance: 4,",
            "    buildings: [",
        ]
        for game_id, values in self.buildings.items():
            item = self.items[game_id]
            lines.append(
                f'        (id: "{game_id}", sprite: "{values[9]}", width: {values[10]}, '
                f"height: {values[11]}, cost: {item.cost}, "
                f"build_time: {item.build_time(self.tables)}, "
                f'producer: "{self.producer(game_id)}", '
                f"prerequisites: {quoted(item.prerequisites)}, "
                f"power_produced: {item.power_produced}, power_consumed: {item.power_consumed}, "
                f"sell_value: {item.cost * self.refund // 100}, "
                f"free_units: {quoted(item.free_units)}),"
                f" // {item.comment('Cost', 'Wall', 'Prerequisite', 'Power', 'FreeUnit')}"
            )
        lines += ["    ],", "    units: ["]
        for game_id, values in self.units.items():
            item = self.items[game_id]
            lines.append(
                f'        (unit: "{game_id}", cost: {item.cost}, '
                f"build_time: {item.build_time(self.tables)}, "
                f'producer: "{self.producer(game_id)}", '
                f"prerequisites: {quoted(item.prerequisites)}),"
                f" // {item.comment('Cost', 'Prerequisite')}"
            )
        lines += ["    ],", "    harvesters: ["]
        for game_id in self.units:
            item = self.items[game_id]
            if convert("flag", item.ini.get("Harvester", "no")):
                lines.append(
                    f'        (unit: "{game_id}", capacity: {int(item.ini["Storage"])}, '
                    f"harvest_rate: {HARVEST_RATE}, harvest_interval: {HARVEST_INTERVAL}),"
                    f" // {item.comment('Harvester', 'Storage')}"
                )
        ore = cite("Riparius", "Value", self.tables.section("Riparius"))
        gems = cite("Cruentus", "Value", self.tables.section("Cruentus"))
        ore_value = int(self.tables.value("Riparius", "Value"))
        gem_value = int(self.tables.value("Cruentus", "Value"))
        lines += ["    ],", "    refineries: ["]
        for game_id in self.buildings:
            if "Refinery" in self.items[game_id].ini:
                lines.append(
                    f'        (building: "{game_id}", credits_per_ore: {ore_value}, '
                    f"credits_per_gem: {gem_value}),"
                    f" // {self.items[game_id].comment('Refinery')}, {ore}, {gems}"
                )
        lines += ["    ],", "    refunds: ["]
        for game_id in self.buildings:
            item = self.items[game_id]
            if int(item.ini.get("Soylent", "0")) != 0:
                lines.append(
                    f'        (building: "{game_id}", credits: {int(item.ini["Soylent"])}),'
                    f" // {item.comment('Soylent')}"
                )
        lines += ["    ],", ")"]
        return "\n".join(lines) + "\n"

    # --- combat.ron --------------------------------------------------------------------------

    def carried(self, item):
        """The ini weapons of an item's slots, (primary, secondary) below and at elite rank;
        `None` for an empty slot (section 4.4: an elite weapon takes the place of the weapon
        of its slot, and a slot without one keeps its weapon)."""
        ini = item.ini
        slot = lambda *keys: next(
            (ini[key] for key in keys if ini.get(key) and ini[key] not in NOT_CARRIED), None
        )
        primary, secondary = slot("Primary", "Weapon1"), slot("Secondary")
        elite = (
            slot("ElitePrimary", "EliteWeapon1") or primary,
            slot("EliteSecondary") or secondary,
        )
        for pair in ((primary, secondary), elite):
            if pair[0] is None and pair[1] is not None:
                raise Failure(f"{item.id}: a secondary weapon without a primary one")
        return (primary, secondary), elite

    def weapon_entry(self, game_id, section_name):
        """The line of a weapon (sections 2 and 4.4) and what it names."""
        tables = self.tables
        weapon = tables.section(section_name)
        own = WEAPON_VALUES.get(section_name, {})
        sources = []

        def value(field, ported):
            """The ported value, or the game's own."""
            if field not in own:
                return ported()
            chosen = own[field]
            if isinstance(chosen, tuple):
                section, key, kind = chosen
                text = tables.value(section, key)
                sources.append(f"{field}: {cite(section, key, tables.section(section))}")
                if kind == "speed":
                    # The cruise speed of the missile unit, read as every unit's `Speed`.
                    return leptons_per_tick(min(int(text) * 256 // 100, 255))
                return convert(kind, text)
            sources.append(f"{field}: the game's own")
            return chosen

        projectile = tables.section(weapon["Projectile"])
        flag = lambda values, key, default: convert("flag", values.get(key, default))
        reach = value("range", lambda: convert("cells", weapon["Range"]))
        gravity = int(tables.value("AudioVisual", "Gravity"))

        def flight():
            # An invisible projectile, and the leap of a unit that is its own projectile,
            # arrive at once. An arcing shell flies at sqrt(1.2 x Gravity x range) leptons per
            # frame, whatever the weapon's `Speed` (an assumption, docs/DECISIONS.md R34);
            # every other projectile at the weapon's `Speed`, read as a unit's is.
            if flag(projectile, "Inviso", "no") or flag(weapon, "LimboLaunch", "no"):
                return 0
            if flag(projectile, "Arcing", "no"):
                return leptons_per_tick(isqrt(reach * gravity * 6 // 5))
            return leptons_per_tick(min(int(weapon["Speed"]) * 256 // 100, 255))

        warhead_section = own.get("warhead", weapon["Warhead"])
        if "warhead" in own:
            sources.append(f"warhead: [{warhead_section}], the game's own")
        scatter = 0
        if flag(projectile, "Inaccurate", "no"):
            # Section 4.4: twice `BallisticScatter` for the invisible flak, once for the shell.
            scatter = convert("cells", tables.value("CombatDamage", "BallisticScatter")) * (
                2 if flag(projectile, "Inviso", "no") else 1
            )
        fields = (
            f'id: "{game_id}", '
            f"damage: {value('damage', lambda: int(weapon['Damage']))}, "
            f"range: {reach}, "
            f"min_range: {convert('cells', weapon.get('MinimumRange', '0'))}, "
            f"reload: {value('reload', lambda: convert('frames', weapon['ROF']))}, "
            f"burst: {int(weapon.get('Burst', '1'))}, "
            f'warhead: "{self.tables.warhead_id[warhead_section]}", '
            f"projectile_speed: {value('projectile_speed', flight)}, "
            f"anti_air: {flag(projectile, 'AA', 'no')}, "
            f"anti_ground: {value('anti_ground', lambda: flag(projectile, 'AG', 'yes'))}, "
            f"scatter: {scatter}"
        )
        keys = ("Damage", "Range", "MinimumRange", "ROF", "Burst", "Warhead", "Projectile", "Speed")
        keys += ("LimboLaunch",)
        comment = f"[{section_name}] " + ", ".join(
            f"{key}={weapon[key]}" for key in keys if key in weapon
        )
        flags = ", ".join(
            f"{key}={projectile[key]}"
            for key in ("AA", "AG", "Inviso", "Arcing", "Inaccurate", "ROT")
            if key in projectile
        )
        if flags:
            comment += f"; [{weapon['Projectile']}] {flags}"
        if sources:
            comment += "; " + "; ".join(sources) + " (ra2_differences.tsv)"
        return f"        ({fields}), // {comment}", warhead_section

    def combat_file(self):
        tables = self.tables
        lines = [
            "// Combat definitions, schema version 3 (crates/sim/src/combat_defs.rs; PLAN.md sections 5 and",
            "// 7, M4; docs/RA2_PARITY_PLAN.md sections 4.3, 4.4 and 4.8; docs/DECISIONS.md D28, R32 to R36,",
            "// R46).",
            "//",
            "// armour_classes: the eleven armour types, in the order of a warhead's `Verses`",
            "// warheads:   the percentage of a weapon's base damage each warhead deals against every armour",
            "//             type (`Verses`; equation 2: damage = base x percent / 100, rounded down, at least",
            "//             1 unless the percentage is 0); cell_spread (`CellSpread` in subcells, 256 per",
            "//             cell: everything of the impact's layer within it is damaged) and percent_at_max",
            "//             (`PercentAtMax`: the percentage dealt at that distance, falling linearly from",
            "//             100); wall and wood (1 or 0: it damages walls, trees)",
            "// weapons:    base damage; range and min_range in subcells (`Range`, `MinimumRange`); reload in",
            "//             ticks (20 per second; `ROF` frames x 4 / 3); burst (shots of a volley, 5 ticks",
            "//             apart); warhead; projectile_speed in subcells per tick, or 0 for a weapon that",
            "//             hits at once (`Inviso`, `LimboLaunch`); anti_air and anti_ground (the",
            "//             projectile's `AA`, `AG`); scatter (subcells at full range between the target and",
            "//             the impact of an `Inaccurate` projectile, from `BallisticScatter`)",
            "// units:      hit points, armour type, sight radius in cells, the weapons (primary, secondary)",
            "//             and the elite weapons in their places of every unit type of units.ron;",
            "//             air_range_bonus (`AirRangeBonus` in subcells); land_targeting and",
            "//             naval_targeting (the ini's codes for which targets on land and on water the",
            "//             type attacks, and with which weapon); underwater and organic (1 or 0)",
            "// buildings:  the same for every building type of economy.ron, with wall (1 or 0) instead of",
            "//             underwater and organic; a building with a weapon is a defensive structure, which",
            "//             does not fire under low power if it is `powered` and consumes power",
            "//             (arsenal.ron, economy.ron)",
            "// effects:    base damage and warhead of the area effects that are no weapon of a type; since",
            "//             schema version 3 (RA4) also of the two spawned missiles, and of each when its",
            "//             launcher is elite",
            "//",
            "// Everything is ported from the ini, except what a comment calls the game's own: the warhead and",
            "// reach of the launchers of RA4 and stand-ins for the specialists of RA6, each listed with its",
            "// reason in crates/sim/tests/reference/ra2_differences.tsv.",
            "//",
            GENERATED,
            "(",
            "    schema_version: 3,",
            f"    armour_classes: {quoted(ARMOURS)},",
            "    warheads: [",
        ]
        weapons, used = [], set()
        for game_id, section_name in tables.weapons:
            line, warhead = self.weapon_entry(game_id, section_name)
            weapons.append(line)
            used.add(warhead)
        effects = []
        for effect, damage, (section, key) in EFFECTS:
            warhead = tables.value(section, key)
            used.add(warhead)
            source = cite(section, key, tables.section(section))
            if isinstance(damage, tuple):
                source = f"{cite(damage[0], damage[1], tables.section(damage[0]))}; {source}"
                damage = int(tables.value(*damage))
            else:
                source = f"{source}; the damage is the game's own"
            effects.append(
                f'        (id: "{effect}", damage: {damage}, '
                f'warhead: "{tables.warhead_id[warhead]}"), // {source}'
            )
        for game_id, section_name in tables.warheads:
            if section_name not in used:
                raise Failure(f"no weapon or effect uses the mapped warhead [{section_name}]")
            warhead = tables.section(section_name)
            percents = [rounded(number(text)) for text in warhead["Verses"].split(",")]
            if len(percents) != len(ARMOURS):
                raise Failure(f"[{section_name}] Verses has {len(percents)} entries")
            entries = ", ".join(
                f'(armour: "{armour}", percent: {percent})'
                for armour, percent in zip(ARMOURS, percents)
            )
            keys = ("CellSpread", "PercentAtMax", "Wall", "Wood", "Verses")
            comment = f"[{section_name}] " + ", ".join(
                f"{key}={warhead[key]}" for key in keys if key in warhead
            )
            lines.append(
                f'        (id: "{game_id}", '
                f"cell_spread: {convert('cells', warhead.get('CellSpread', '0'))}, "
                f"percent_at_max: {convert('percent', warhead.get('PercentAtMax', '1'))}, "
                f"wall: {convert('flag', warhead.get('Wall', 'no'))}, "
                f"wood: {convert('flag', warhead.get('Wood', 'no'))}, "
                f"versus: [{entries}]), // {comment}"
            )
        lines += ["    ],", "    weapons: ["] + weapons
        fired = set()
        for name, key, table in (
            ("units", "unit", self.units),
            ("buildings", "building", self.buildings),
        ):
            lines += ["    ],", f"    {name}: ["]
            for game_id in table:
                item = self.items[game_id]
                ini = item.ini
                slots, elite = self.carried(item)
                fired.update(weapon for weapon in slots + elite if weapon)
                ids = lambda pair: quoted(
                    [tables.weapon_id[weapon] for weapon in pair if weapon]
                )
                traits = (
                    f"underwater: {convert('flag', ini.get('Underwater', 'no'))}, "
                    f"organic: {convert('flag', ini.get('Organic', 'no'))}"
                    if key == "unit"
                    else f"wall: {int(item.wall)}"
                )
                keys = ("Strength", "Armor", "Sight", "Primary", "Weapon1", "Secondary")
                keys += ("ElitePrimary", "EliteWeapon1", "EliteSecondary", "AirRangeBonus")
                keys += ("LandTargeting", "NavalTargeting", "Underwater", "Organic", "Wall")
                lines.append(
                    f'        ({key}: "{game_id}", hit_points: {item.hit_points}, '
                    f'armour: "{ini.get("Armor", "none").lower()}", sight: {item.sight}, '
                    f"weapons: {ids(slots)}, elite_weapons: {ids(elite)}, "
                    f"air_range_bonus: {convert('cells', ini.get('AirRangeBonus', '0'))}, "
                    f"land_targeting: {int(ini.get('LandTargeting', '0'))}, "
                    f"naval_targeting: {int(ini.get('NavalTargeting', '0'))}, "
                    f"{traits}), // {item.comment(*keys)}"
                )
        for game_id, section_name in tables.weapons:
            twins = [name for name, known in tables.weapon_id.items() if known == game_id]
            if not fired.intersection(twins):
                raise Failure(f"no type carries the mapped weapon [{section_name}]")
        lines += ["    ],", "    effects: ["] + effects + ["    ],", ")"]
        return "\n".join(lines) + "\n"

    # --- arsenal.ron -------------------------------------------------------------------------

    def arsenal_file(self):
        lines = [
            "// The arsenal, schema version 6 (crates/sim/src/arsenal.rs; docs/DECISIONS.md R10, R11, R22, R6, R46): per",
            "// item its label, description, category, domain and role with the role's terms (durations in ticks,",
            "// 20 per second), the id of the item whose art it shows, its side, the factory group of a",
            "// building, and the terms of the ini's buildability rule: the sides whose factories build it",
            "// (`Owner`), the countries that alone may or may not build it (`RequiredHouses`,",
            "// `ForbiddenHouses`), its tech level (`TechLevel`; -1: cannot be queued), and the prerequisite",
            "// groups of rules.ron of which the player must own a member (the group tokens of",
            "// `Prerequisite`; the single buildings of that key are in economy.ron).",
            "//",
            "// Since schema version 3 (RA2) a building also has: build_limit (`BuildLimit`: how many a",
            "// player may own and have queued together; 0: any number), adjacent (`Adjacent`: the free",
            "// cells allowed between it and a base building of its owner when it is placed), and as 1",
            "// or 0: base_normal (`BaseNormal`: other buildings may be placed beside it), radar",
            "// (`Radar`), sellable (no `Wall`, no `Unsellable`), repairable (`Repairable`), powered",
            "// (`Powered`: it stops working under low power if it consumes power), is_powered (the",
            "// `IsPowered` of its `SuperWeapon`: the charge waits under low power) and unit_repair",
            "// (`UnitRepair`: a damaged vehicle sent to it is repaired for credits). The category of a",
            "// building is its production lane: \"armory\" is `BuildCat=Combat`, the defences. A unit",
            "// has 0 in all of them.",
            "//",
            "// Since schema version 4 (RA3, docs/DECISIONS.md R6) a unit also has, as 1 or 0: infantry (it",
            "// is in `[InfantryTypes]`: unless it flies it holds one of the three spots of a cell",
            "// instead of the cell), crushable (`Crushable`; without the key 1 for infantry and 0 for",
            "// every other unit: a crusher of an enemy may drive over it) and crusher (`Crusher`: on a",
            "// move or attack move it drives over hostile crushable infantry in its way); and size",
            "// (`Size`; 1 without the key: the room it takes in a transport, not read yet). A building",
            "// has 0 in all of them.",
            "//",
            "// Since schema version 5 (RA4, docs/DECISIONS.md R46): trainable (`Trainable`; 1 for a unit",
            "// without the key) with veteran_abilities and elite_abilities (the tokens of",
            "// `VeteranAbilities` and `EliteAbilities`); crewed (`Crewed`); self_healing",
            "// (`SelfHealing`); ammo (`Ammo`; 0: unlimited) and airport_bound (`AirportBound`) of an",
            "// aircraft, and docks of a building (`NumberOfDocks`, if an airport bound aircraft names",
            "// it as its `Dock`); spawns, spawns_number, spawn_regen and spawn_reload (`Spawns`,",
            "// `SpawnsNumber`, and `SpawnRegenRate` and `SpawnReloadRate` frames x 4 / 3 in ticks) with",
            "// launcher (which weapon has `Spawner=yes`); missile (the type is [General]",
            "// `V3RocketType` or `DMislType`); cloakable (`Cloakable`) and sensors_sight",
            "// (`SensorsSight` in cells of a type with `Sensors=yes`).",
            "//",
            "// Since schema version 6 (RA5, R65): needs_engineer (NeedsEngineer), capturable",
            "// (Capturable), insignificant (Insignificant), immune (Immune), hospital (Hospital),",
            "// bridge_repair_hut (BridgeRepairHut), occupier (Occupier), can_be_occupied",
            "// (CanBeOccupied), max_occupants (MaxNumberOccupants), vehicle_docks (NumberOfDocks",
            "// on UnitRepair), cash_startup (ProduceCashStartup), cash_amount (ProduceCashAmount),",
            "// cash_delay (ProduceCashDelay frames converted to ticks). Missing flags/numbers are 0.",
            "//",
            GENERATED,
            "(schema_version: 6, entries: [",
        ]
        for table in (self.buildings, self.units):
            for game_id, values in table.items():
                item = self.items[game_id]
                label, description, category, domain, role, cooldown, radius, strength, capacity = (
                    values[:9]
                )
                keys = ["Owner", "RequiredHouses", "ForbiddenHouses", "TechLevel", "Prerequisite"]
                keys += ["Factory", "Naval"] + list(STOLEN)
                keys += ["Crewed", "Sensors", "SensorsSight", "NeedsEngineer", "Capturable", "Insignificant", "Immune", "Hospital", "BridgeRepairHut", "Occupier", "CanBeOccupied", "MaxNumberOccupants", "NumberOfDocks", "ProduceCashStartup", "ProduceCashAmount", "ProduceCashDelay"]
                if item.kind == "unit":
                    keys += ["Crusher", "Crushable", "Size", "Trainable", "VeteranAbilities"]
                    keys += ["EliteAbilities", "SelfHealing", "Ammo", "AirportBound", "Spawns"]
                    keys += ["SpawnsNumber", "SpawnRegenRate", "SpawnReloadRate", "Cloakable"]
                    # The game's categories and domains are its own tables: they must agree
                    # with the type list, and a crusher must be a vehicle on the surface.
                    if (category == "infantry") != bool(item.infantry):
                        raise Failure(f"{game_id}: the category {category} is not its type list")
                    if item.crusher and (category != "vehicles" or domain == "air"):
                        raise Failure(f"{game_id}: a crusher that is not a surface vehicle")
                if item.kind == "building":
                    if (category == "armory") != item.defence:
                        raise Failure(f"{game_id}: the category {category} is not its BuildCat")
                    keys += ["BuildCat", "BuildLimit", "Adjacent", "BaseNormal", "Radar", "Wall"]
                    keys += ["Unsellable", "Repairable", "Powered", "UnitRepair", "SuperWeapon"]
                source = item.comment(*keys)
                if item.infantry_list:
                    source += f"; {item.infantry_list}"
                if item.kind == "building":
                    if item.special:
                        special = self.tables.section(item.special)
                        source += f"; {cite(item.special, 'IsPowered', special)}"
                    if item.far_shipyard:
                        source += "; adjacent is the game's own (ra2_differences.tsv)"
                    if item.docks:
                        source += f"; NumberOfDocks={item.ini['NumberOfDocks']}; {item.dock_users}"
                if item.launcher != "none":
                    weapon = item.ini["Primary" if item.launcher == "primary" else "Secondary"]
                    source += f"; [{weapon}] Spawner=yes"
                if item.missile_key:
                    source += f"; {cite('General', item.missile_key, self.tables.section('General'))}"
                # A unit's `BuildLimit` (Yuri Prime) is not ported until the unit can be built
                # (RA6); ra2_differences.tsv lists it.
                limit = item.build_limit if item.kind == "building" else 0
                lines.append(
                    f'    (id: "{game_id}", label: "{label}", description: "{description}", '
                    f'category: "{category}", domain: "{domain}", role: "{role}", '
                    f"cooldown: {cooldown}, radius: {radius}, strength: {strength}, "
                    f'capacity: {capacity}, art: "{values[-1]}", side: "{item.side}", '
                    f'factory: "{item.factory}", owner_sides: {quoted(item.owner_sides)}, '
                    f"required_countries: {quoted(item.required)}, "
                    f"forbidden_countries: {quoted(item.forbidden)}, "
                    f"tech_level: {item.tech_level}, "
                    f"prerequisite_groups: {quoted(item.groups)}, "
                    f"build_limit: {limit}, adjacent: {item.adjacent}, "
                    f"base_normal: {item.base_normal}, radar: {item.radar}, "
                    f"sellable: {item.sellable}, repairable: {item.repairable}, "
                    f"powered: {item.powered}, is_powered: {item.is_powered}, "
                    f"unit_repair: {item.unit_repair}, crusher: {item.crusher}, "
                    f"crushable: {item.crushable}, size: {item.size}, "
                    f"infantry: {item.infantry}, trainable: {item.trainable}, "
                    f"veteran_abilities: {quoted(item.abilities['VeteranAbilities'])}, "
                    f"elite_abilities: {quoted(item.abilities['EliteAbilities'])}, "
                    f"crewed: {item.crewed}, self_healing: {item.self_healing}, "
                    f"ammo: {item.ammo}, airport_bound: {item.airport_bound}, "
                    f"docks: {item.docks}, "
                    f'spawns: "{item.spawns or "none"}", '
                    f"spawns_number: {item.spawns_number}, "
                    f"spawn_regen: {item.spawn_regen}, spawn_reload: {item.spawn_reload}, "
                    f'launcher: "{item.launcher}", missile: "{item.missile}", '
                    f"cloakable: {item.cloakable}, sensors_sight: {item.sensors_sight}, "
                    f"needs_engineer: {item.needs_engineer}, "
                    f"capturable: {item.capturable}, "
                    f"insignificant: {item.insignificant}, "
                    f"immune: {item.immune}, "
                    f"hospital: {item.hospital}, "
                    f"bridge_repair_hut: {item.bridge_repair_hut}, "
                    f"occupier: {item.occupier}, "
                    f"can_be_occupied: {item.can_be_occupied}, "
                    f"max_occupants: {item.max_occupants}, "
                    f"vehicle_docks: {item.vehicle_docks}, "
                    f"cash_startup: {item.cash_startup}, "
                    f"cash_amount: {item.cash_amount}, "
                    f"cash_delay: {item.cash_delay}),"
                    f" // {source}"
                )
        lines.append("])")
        # Keep the bounded schema under MAX_DATA_FILE_BYTES without dropping source comments.
        import re
        return "\n".join(re.sub(r", (?=[a-z_]+:)", ",", line) for line in lines) + "\n"

    # --- rules.ron ---------------------------------------------------------------------------

    def rules_file(self):
        tables = self.tables
        lines = [
            "// Global rules, schema version 2 (crates/sim/src/rules.rs; docs/RA2_PARITY_PLAN.md sections 3.2",
            "// and 4; docs/DECISIONS.md R9, R10, R58): the values of the ruleset that belong to no single unit",
            "// or building, in game units (ticks, 20 per second; subcells, 256 per cell; permille; percent;",
            "// yes and no as 1 and 0). A value without a source is the game's own: the ini has no key for it.",
            "//",
            GENERATED,
            "(",
            "    schema_version: 2,",
        ]

        def scalars(group):
            lines.append(f"    {group}: (")
            for name, field, section, key, kind, _ in RULES:
                if name == group:
                    value = convert(kind, tables.value(section, key))
                    lines.append(
                        f"        {field}: {value}, // {cite(section, key, tables.section(section))}"
                    )
            for field, value, note in OWN_RULES.get(group, ()):
                lines.append(f"        {field}: {value}, // {note}")
            lines.append("    ),")

        for group in RULE_GROUPS:
            scalars(group)
        lines.append("    powerups: [")
        powerups = tables.section("Powerups")
        for game_id, key, kind in POWERUPS:
            parts = powerups[key].split(",")
            value = convert(kind, parts[3]) if kind else 0
            lines.append(
                f'        (id: "{game_id}", share: {int(parts[0])}, value: {value}),'
                f" // {cite('Powerups', key, powerups)}"
            )
        lines.append("    ],")
        # RA5: the vehicles a crate may hold, `CrateGoodie=yes`, in the order of the units.
        goodies = [
            game_id
            for game_id in self.units
            if convert("flag", self.items[game_id].ini.get("CrateGoodie", "no"))
        ]
        lines.append("    crate_units: [")
        for game_id in goodies:
            lines.append(f'        "{game_id}", // {self.items[game_id].comment("CrateGoodie")}')
        lines.append("    ],")
        scalars("dialog")
        lines.append("    difficulty: [")
        general = tables.section("General")
        for game_id, section, index in DIFFICULTIES:
            fields, sources = [], []
            for field, where, key, kind in DIFFICULTY_FIELDS:
                if where is None:
                    text = tables.value(section, key)
                    sources.append(cite(section, key, tables.section(section)))
                else:
                    text = general[key].split(",")[index]
                    sources.append(f"{cite(where, key, general)} (entry {index + 1})")
                fields.append(f"{field}: {convert(kind, text)}")
            lines.append(f'        (id: "{game_id}", {", ".join(fields)}),')
            lines.append(f"        // {'; '.join(sources)}")
        lines += ["    ],", "    prerequisite_groups: ["]
        for group, key in GROUPS:
            members = [tables.game_id(section) for section in general[key].split(",") if section]
            lines.append(
                f'        (id: "{group}", buildings: {quoted(members)}),'
                f" // {cite('General', key, general)}"
            )
        lines += ["    ],", "    sides: ["]
        base_units = general["BaseUnit"].split(",")
        for index, side in enumerate(("allied", "soviet")):
            base = tables.section(base_units[index])
            fields = [
                f'id: "{side}"',
                f'base_unit: "{tables.game_id(base_units[index])}"',
                f'construction_yard: "{tables.game_id(base["DeploysInto"])}"',
            ]
            sources = [
                f"{cite('General', 'BaseUnit', general)} (entry {index + 1})",
                cite(base_units[index], "DeploysInto", base),
            ]
            for field, allied, soviet, kind in SIDE_FIELDS:
                key = (allied, soviet)[index]
                value = general[key]
                fields.append(
                    f'{field}: "{tables.game_id(value)}"' if kind == "unit" else f"{field}: {int(value)}"
                )
                sources.append(cite("General", key, general))
            lines.append(f"        ({', '.join(fields)}),")
            lines.append(f"        // {'; '.join(sources)}")
        lines += ["    ],", "    countries: ["]
        houses = tables.section("Countries")
        for index, (house, country, _) in enumerate(COUNTRIES):
            if houses.get(str(index)) != house:
                raise Failure(f"[Countries] {index} is not {house}")
            side = SIDES[tables.value(house, "Side")]
            lines.append(
                f'        (id: "{country}", side: "{side}"),'
                f" // [Countries] {index}={house}, {cite(house, 'Side', tables.section(house))}"
            )
        lines += ["    ],", ")"]
        return "\n".join(lines) + "\n"

    # --- skirmish.ron ------------------------------------------------------------------------

    def starting(self, side, category):
        """The unit types of a side its players' starting units are drawn from (plan section
        4.9): no `AllowedToStartInMultiplayer=no`, a tech level of the lobby, built by the side,
        by all of its countries, and not a base unit."""
        base = [
            self.tables.game_id(section)
            for section in self.tables.value("General", "BaseUnit").split(",")
        ]
        top = int(self.tables.value("MultiplayerDialogSettings", "TechLevel"))
        found = []
        for game_id, values in self.units.items():
            item = self.items[game_id]
            if (
                (values[2] == "infantry") == (category == "infantry")
                and item.ini.get("AllowedToStartInMultiplayer") != "no"
                and 0 <= item.tech_level <= top
                and item.owner_sides == [side]
                and not item.required
                and not item.forbidden
                and game_id not in base
            ):
                found.append(game_id)
        return found

    def skirmish_file(self):
        dialog = self.tables.section("MultiplayerDialogSettings")
        lines = [
            "// Skirmish definitions, schema version 3 (crates/sim/src/skirmish_defs.rs; docs/DECISIONS.md D40,",
            "// D48, R13).",
            "//",
            "// starting_credits: credits of every player at the start of a match, unless its options name",
            "//            another amount within the range of rules.ron",
            "// sides:     the unit types a player's starting units are drawn from (the ini's",
            "//            `AllowedToStartInMultiplayer`; docs/RA2_PARITY_PLAN.md section 4.9)",
            "// factions:  the nine countries: id (a country of rules.ron), label, and side; a country's",
            "//            player starts with the base unit of its side and builds by the rules of",
            "//            arsenal.ron, so its roster and starting lists are empty",
            "// maps:      the maps of the skirmish menu: id (the map file is maps/<id>/<id>.tmx) and 2 to 10",
            "//            start positions; the human player starts at the first, computer players at the",
            "//            following ones in order, so a match with fewer players uses the first entries. Every",
            "//            start has clear building ground, ore within 10 cells and a land path to the others;",
            "//            on the point-symmetric generated maps they come in mirrored pairs, so bases are equal.",
            "//            A start is where the base unit stands and the origin of the 3 x 3 Construction Yard",
            "//            it deploys into. On the large maps (192 to 256 cells a side) the first pair is the",
            "//            two bases farthest apart, so a one-against-one match is fought across the whole map.",
            "//            The list is printed by `cargo run -p game --example gen_maps -- starts`.",
            "//",
            "//            A match with more players than a map lists starts is played on the map enlarged for",
            "//            them (sim::enlarged_map, crates/sim/src/map_scale.rs): every further player adds an",
            "//            eighth to the map's width and height while it stays within 256 cells a side, the",
            "//            starts below move with their ground, and the missing ones are made on dry ground",
            "//            reachable from the first start, at least 22 cells from every other start, in",
            "//            mirrored pairs, each with cleared building ground and an ore field. The game checks",
            "//            at startup that every map sets up for every player count.",
            "//",
            GENERATED,
            "(",
            "    schema_version: 3,",
            f"    starting_credits: {int(dialog['Money'])}, // {cite('MultiplayerDialogSettings', 'Money', dialog)}",
            "    sides: [",
        ]
        for side in ("allied", "soviet"):
            vehicles, infantry = self.starting(side, "vehicles"), self.starting(side, "infantry")
            sources = "; ".join(
                f"[{self.items[game_id].section_name}] Cost={self.items[game_id].cost}"
                for game_id in vehicles + infantry
            )
            lines.append(
                f'        (side: "{side}", starting_vehicles: {quoted(vehicles)}, '
                f"starting_infantry: {quoted(infantry)}),"
                f" // without AllowedToStartInMultiplayer=no: {sources}"
            )
        lines += ["    ],", "    factions: ["]
        for house, country, label in COUNTRIES:
            side = SIDES[self.tables.value(house, "Side")]
            lines.append(
                f'        (id: "{country}", label: "{label}", side: "{side}", '
                "roster_buildings: [], roster_units: [], buildings: [], units: []),"
                f" // {cite(house, 'Side', self.tables.section(house))}"
            )
        lines += ["    ],", "    maps: ["]
        for map_id, starts in MAPS:
            cells = ", ".join(f"(x: {x}, y: {y})" for x, y in starts)
            lines.append(f'        (id: "{map_id}", starts: [{cells}]),')
        lines += ["    ],", ")"]
        return "\n".join(lines) + "\n"

    # --- ai.ron ------------------------------------------------------------------------------

    def ai_file(self):
        lines = [
            "// The computer player's definitions, schema version 2 (crates/sim/src/ai_defs.rs;",
            "// docs/DECISIONS.md D42, D49, R14).",
            "//",
            "// plans:        one per side, followed by the computer players of its countries:",
            "//   build_order:  building types in the order the computer player builds them; a type listed n",
            "//                 times is kept at n buildings, so lost buildings are rebuilt. A power building",
            "//                 is built first whenever the next building would leave the player short of",
            "//                 power. Prerequisites come before the buildings that need them; the",
            "//                 Construction Yard the side's base unit deploys into counts as owned.",
            "//   power:        the building type built for power",
            "//   harvester:    the miner unit type (its producer and prerequisites are in build_order)",
            "//   army:         armed unit types, produced in this order, cyclically (their producers and",
            "//                 prerequisites are in build_order)",
            "//   Every entry can be built by every country of the side on its own.",
            "// difficulties: id; think_interval (ticks between two decisions, 20 per second);",
            "//               harvesters_per_refinery; max_army (armed units it builds up to); wave_size (armed",
            "//               units at home that start an attack wave); first_wave (the tick from which waves",
            "//               may start); wave_patience (ticks after first_wave or the last wave after which",
            "//               the units at home attack even if fewer than wave_size); defence_radius (visible",
            "//               enemies this many cells from its buildings are attacked by its army at home)",
            "//",
            "// Game specific data: the ini has no build orders. RA8 extends the plans to the full arsenal.",
            "//",
            GENERATED,
            "(",
            "    schema_version: 2,",
            "    plans: [",
        ]
        for side, build_order, power, harvester, army in AI_PLANS:
            lines += [
                "        (",
                f'            side: "{side}",',
                "            build_order: [",
            ]
            lines += [f'                "{building}",' for building in build_order]
            lines += [
                "            ],",
                f'            power: "{power}",',
                f'            harvester: "{harvester}",',
                f"            army: {quoted(army)},",
                "        ),",
            ]
        lines += ["    ],", "    difficulties: ["]
        names = (
            "think_interval", "harvesters_per_refinery", "max_army", "wave_size", "first_wave",
            "wave_patience", "defence_radius",
        )
        for difficulty in AI_DIFFICULTIES:
            lines += ["        (", f'            id: "{difficulty[0]}",']
            lines += [
                f"            {name}: {value}," for name, value in zip(names, difficulty[1:])
            ]
            lines.append("        ),")
        lines += ["    ],", ")"]
        return "\n".join(lines) + "\n"

    def files(self):
        return {
            "rules.ron": self.rules_file(),
            "units.ron": self.units_file(),
            "economy.ron": self.economy_file(),
            "combat.ron": self.combat_file(),
            "arsenal.ron": self.arsenal_file(),
            "skirmish.ron": self.skirmish_file(),
            "ai.ron": self.ai_file(),
        }


GENERATED = (
    "// Generated by scripts/generate_ra2_data.py from crates/sim/tests/reference; do not edit."
)


def main(argv):
    if argv not in ([], ["--check"]):
        print(__doc__, file=sys.stderr)
        print("usage: generate_ra2_data.py [--check]", file=sys.stderr)
        return 2
    try:
        files = Data().files()
    except (Failure, KeyError, ValueError) as error:
        print(f"generate_ra2_data.py: {error!r}", file=sys.stderr)
        return 2
    stale = []
    for name, text in files.items():
        path = os.path.join(DATA_DIR, name)
        if argv:
            try:
                with open(path, encoding="ascii") as handle:
                    if handle.read() != text:
                        stale.append(name)
            except OSError:
                stale.append(name)
        else:
            with open(path, "w", encoding="ascii") as handle:
                handle.write(text)
    if stale:
        print(f"stale, run scripts/generate_ra2_data.py: {', '.join(stale)}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
