#!/usr/bin/env python3
# Generates the shipped data files of the RA2 parity programme (docs/RA2_PARITY_PLAN.md section
# 3.1, docs/DECISIONS.md R9):
#
#     crates/sim/tests/reference/ra2_reference.tsv   (committed: file, section, key, value)
#     crates/sim/tests/reference/ra2_mapping.tsv     (committed: game id -> ini section)
#             |  this script, with its own tables of game specific values below
#             v
#     assets/data/{rules,units,economy,combat,arsenal,skirmish,ai}.ron
#
# It reads only the two committed tables: never the ini files. Every value ported from the
# reference is converted with the conversions of the plan's section 2 and written with a
# trailing comment `// [Section] Key=value` naming where it comes from. Values that are not
# ported (labels, descriptions, sprites, footprints, roles, armour, weapons, the computer
# player's plans, the maps) come from the tables of this script and carry no such comment;
# weapons, armour and footprints are ported by later milestones. Build times are computed from
# the cost (RA2, docs/RA2_PARITY_PLAN.md section 4.2).
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
#      sprite, width, height, armour, weapons)
BUILDINGS = {
    'construction_yard': ('Construction Yard', 'Base infrastructure.', 'buildings', 'land', 'combat', 100, 6, 20, 0,
        'construction_yard', 3, 3, 'structure', []),
    'power_plant': ('Power Plant', 'Base infrastructure.', 'buildings', 'land', 'combat', 100, 6, 20, 0,
        'power_plant', 2, 2, 'structure', []),
    'refinery': ('Ore Refinery', 'Base infrastructure.', 'buildings', 'land', 'combat', 100, 6, 20, 0,
        'refinery', 3, 3, 'structure', []),
    'barracks': ('Barracks', 'Base infrastructure.', 'buildings', 'land', 'combat', 100, 6, 20, 0,
        'barracks', 2, 2, 'structure', []),
    'war_factory': ('War Factory', 'Base infrastructure.', 'buildings', 'land', 'combat', 100, 6, 20, 0,
        'war_factory', 3, 3, 'structure', []),
    'tech_center': ('Battle Lab', 'Base infrastructure.', 'buildings', 'land', 'combat', 100, 6, 20, 0,
        'tech_center', 2, 2, 'structure', []),
    'tesla_reactor': ('Tesla Reactor', 'Base infrastructure.', 'buildings', 'land', 'combat', 100, 6, 20, 0,
        'power_plant', 2, 2, 'structure', []),
    'ore_purifier': ('Ore Purifier', 'Increases ore delivery income by 25 percent while powered.', 'buildings', 'land', 'purifier', 100, 6, 20, 0,
        'tech_center', 2, 2, 'structure', []),
    'nuclear_reactor': ('Nuclear Reactor', 'Base infrastructure.', 'buildings', 'land', 'combat', 100, 6, 20, 0,
        'power_plant', 2, 2, 'structure', []),
    'naval_shipyard': ('Naval Shipyard', 'Place on water adjacent to shore. Produces naval units.', 'buildings', 'land', 'shipyard', 100, 6, 20, 0,
        'war_factory', 3, 3, 'structure', []),
    'airforce_command': ('Airforce Command Headquarters', 'Base infrastructure.', 'buildings', 'land', 'combat', 100, 6, 20, 0,
        'war_factory', 3, 3, 'structure', []),
    'service_depot': ('Service Depot', 'Repairs a damaged vehicle sent to it, for credits.', 'buildings', 'land', 'repair', 100, 6, 20, 0,
        'tech_center', 2, 2, 'structure', []),
    'radar_tower': ('Radar Tower', 'Base infrastructure.', 'buildings', 'land', 'combat', 100, 6, 20, 0,
        'tech_center', 2, 2, 'structure', []),
    'cloning_vats': ('Cloning Vats', 'Duplicates trained infantry.', 'buildings', 'land', 'cloner', 100, 6, 20, 0,
        'tech_center', 2, 2, 'structure', []),
    'fortress_walls': ('Fortress Walls', 'Blocks ground movement.', 'armory', 'land', 'combat', 100, 6, 20, 0,
        'gun_turret', 1, 1, 'structure', []),
    'pillbox': ('Pillbox', 'Automatically engages valid enemy targets.', 'armory', 'land', 'combat', 100, 6, 20, 0,
        'gun_turret', 1, 1, 'structure', ['rifle']),
    'sentry_gun': ('Sentry Gun', 'Automatically engages valid enemy targets.', 'armory', 'land', 'combat', 100, 6, 20, 0,
        'gun_turret', 1, 1, 'structure', ['scout_gun']),
    'patriot': ('Patriot Missile System', 'Automatically engages valid enemy targets.', 'armory', 'land', 'flak', 100, 6, 20, 0,
        'gun_turret', 1, 1, 'structure', ['aegis_missile']),
    'flak_cannon': ('Flak Cannon', 'Automatically engages valid enemy targets.', 'armory', 'land', 'flak', 100, 6, 20, 0,
        'gun_turret', 1, 1, 'structure', ['aegis_missile']),
    'prism_tower': ('Prism Tower', 'Automatically engages valid enemy targets.', 'armory', 'land', 'splash', 100, 6, 20, 0,
        'gun_turret', 1, 1, 'structure', ['prism_beam']),
    'tesla_coil': ('Tesla Coil', 'Automatically engages valid enemy targets.', 'armory', 'land', 'tesla', 100, 6, 20, 0,
        'gun_turret', 1, 1, 'structure', ['tesla_weapon']),
    'grand_cannon': ('Grand Cannon', 'Automatically engages valid enemy targets.', 'armory', 'land', 'splash', 100, 6, 20, 0,
        'tech_center', 2, 2, 'structure', ['grand_shell']),
    'gap_generator': ('Gap Generator', 'Shrouds this area against enemy vision.', 'armory', 'land', 'gap', 100, 6, 20, 0,
        'tech_center', 2, 2, 'structure', []),
    'spy_satellite': ('Spy Satellite Uplink', 'Reveals the battlefield while powered.', 'armory', 'land', 'satellite', 100, 6, 20, 0,
        'tech_center', 2, 2, 'structure', []),
    'psychic_sensor': ('Psychic Sensor', 'Detects concealed units nearby.', 'armory', 'land', 'sensor', 100, 6, 20, 0,
        'tech_center', 2, 2, 'structure', []),
    'chronosphere': ('Chronosphere', 'Select vehicles, choose ability, then click a visible destination.', 'armory', 'land', 'chronosphere', 3600, 3, 0, 0,
        'tech_center', 2, 2, 'structure', []),
    'weather_control': ('Weather Control Device', 'Click a visible area for a devastating lightning storm.', 'armory', 'land', 'storm', 6000, 5, 1600, 0,
        'war_factory', 3, 3, 'structure', []),
    'iron_curtain': ('Iron Curtain Device', 'Click a visible area to protect friendly mechanical units for 10 seconds.', 'armory', 'land', 'curtain', 3600, 3, 200, 0,
        'tech_center', 2, 2, 'structure', []),
    'nuclear_silo': ('Nuclear Missile Silo', 'Click a visible area for a nuclear strike.', 'armory', 'land', 'nuke', 6000, 6, 2500, 0,
        'war_factory', 3, 3, 'structure', []),
}
# id: (label, description, category, domain, role, cooldown, radius, strength, capacity,
#      sprite, armour, weapons)
UNITS = {
    'conscript': ('Conscript', 'Specialized ground combat. Select and right click an enemy to attack.', 'infantry', 'land', 'combat', 100, 2, 20, 0,
        'conscript', 'infantry', ['carbine']),
    'gi': ('GI', 'Specialized ground combat. Select and right click an enemy to attack.', 'infantry', 'land', 'combat', 100, 2, 20, 0,
        'rifleman', 'infantry', ['rifle']),
    'engineer': ('Engineer', 'E + click a nearby structure to capture or repair it.', 'infantry', 'land', 'engineer', 100, 2, 20, 0,
        'rifleman', 'infantry', []),
    'attack_dog': ('Attack Dog', 'Specialized ground combat. Select and right click an enemy to attack.', 'infantry', 'land', 'dog', 100, 2, 20, 0,
        'scout', 'infantry', ['dog_bite']),
    'rocketeer': ('Rocketeer', 'Specialized air combat. Select and right click an enemy to attack.', 'infantry', 'air', 'combat', 100, 2, 20, 0,
        'rifleman', 'air', ['air_rifle']),
    'spy': ('Spy', 'E + click a nearby enemy building to infiltrate. Dogs detect spies.', 'infantry', 'land', 'spy', 100, 2, 20, 0,
        'rifleman', 'infantry', []),
    'tanya': ('Tanya', 'Specialized ground combat. Select and right click an enemy to attack.', 'infantry', 'land', 'commando', 100, 2, 20, 0,
        'rifleman', 'infantry', ['commando_gun']),
    'chrono_legionnaire': ('Chrono Legionnaire', 'Teleports when given a move order; beam suppresses targets.', 'infantry', 'land', 'chrono', 100, 2, 20, 0,
        'rifleman', 'infantry', ['chrono_beam']),
    'sniper': ('Sniper', 'Specialized ground combat. Select and right click an enemy to attack.', 'infantry', 'land', 'combat', 100, 2, 20, 0,
        'rifleman', 'infantry', ['sniper_rifle']),
    'flak_trooper': ('Flak Trooper', 'Specialized ground combat. Select and right click an enemy to attack.', 'infantry', 'land', 'flak', 100, 2, 20, 0,
        'rocket_soldier', 'infantry', ['flak_weapon']),
    'tesla_trooper': ('Tesla Trooper', 'Specialized ground combat. Select and right click an enemy to attack.', 'infantry', 'land', 'tesla', 100, 2, 20, 0,
        'rocket_soldier', 'infantry', ['tesla_weapon']),
    'crazy_ivan': ('Crazy Ivan', 'Attaches a delayed explosive when attacking.', 'infantry', 'land', 'demolition', 100, 2, 800, 0,
        'grenadier', 'infantry', ['bomb_trigger']),
    'yuri': ('Yuri', 'Controls one enemy unit; control ends when Yuri dies.', 'infantry', 'land', 'mind_control', 100, 2, 20, 0,
        'rifleman', 'infantry', ['control_beam']),
    'desolator': ('Desolator', 'D contaminates surrounding ground.', 'infantry', 'land', 'radiation', 100, 3, 20, 0,
        'grenadier', 'infantry', ['radiation_weapon']),
    'terrorist': ('Terrorist', 'Detonates on reaching attack range.', 'infantry', 'land', 'suicide', 100, 3, 500, 0,
        'conscript', 'infantry', ['suicide_trigger']),
    'chrono_miner': ('Chrono Miner', 'Harvests ore and teleports home to unload.', 'vehicles', 'land', 'chrono_miner', 100, 2, 20, 0,
        'harvester', 'heavy', []),
    'war_miner': ('War Miner', 'Specialized ground combat. Select and right click an enemy to attack.', 'vehicles', 'land', 'combat', 100, 2, 20, 0,
        'harvester', 'heavy', ['scout_gun']),
    'grizzly': ('Grizzly Battle Tank', 'Specialized ground combat. Select and right click an enemy to attack.', 'vehicles', 'land', 'combat', 100, 2, 20, 0,
        'light_tank', 'heavy', ['tank_cannon']),
    'rhino': ('Rhino Heavy Tank', 'Specialized ground combat. Select and right click an enemy to attack.', 'vehicles', 'land', 'combat', 100, 2, 20, 0,
        'heavy_tank', 'heavy', ['heavy_cannon']),
    'ifv': ('Infantry Fighting Vehicle (IFV)', 'Carries one infantry; passenger changes its weapon. D unloads.', 'vehicles', 'land', 'ifv', 100, 2, 20, 1,
        'scout', 'light', ['flak_weapon']),
    'flak_track': ('Flak Track', 'Select passengers; E + click this transport to load. D unloads.', 'vehicles', 'land', 'transport', 100, 2, 20, 5,
        'raider', 'light', ['flak_weapon']),
    'terror_drone': ('Terror Drone', 'Immobilizes and damages mechanical targets.', 'vehicles', 'land', 'drone', 100, 2, 20, 0,
        'scout', 'light', ['drone_claw']),
    'v3_launcher': ('V3 Rocket Launcher', 'Specialized ground combat. Select and right click an enemy to attack.', 'vehicles', 'land', 'combat', 100, 2, 20, 0,
        'rocket_launcher', 'light', ['v3_missile']),
    'mcv': ('Mobile Construction Vehicle (MCV)', 'D deploys into a Construction Yard on clear ground.', 'vehicles', 'land', 'mcv', 100, 2, 20, 0,
        'harvester', 'heavy', []),
    'mirage_tank': ('Mirage Tank', 'Concealed while idle; revealed by nearby detectors.', 'vehicles', 'land', 'mirage', 100, 2, 20, 0,
        'light_tank', 'heavy', ['mirage_gun']),
    'prism_tank': ('Prism Tank', 'Specialized ground combat. Select and right click an enemy to attack.', 'vehicles', 'land', 'splash', 100, 2, 20, 0,
        'artillery', 'light', ['prism_beam']),
    'apocalypse': ('Apocalypse Tank', 'Specialized ground combat. Select and right click an enemy to attack.', 'vehicles', 'land', 'combat', 100, 2, 20, 0,
        'heavy_tank', 'heavy', ['apocalypse_gun']),
    'tank_destroyer': ('Tank Destroyer', 'Specialized ground combat. Select and right click an enemy to attack.', 'vehicles', 'land', 'combat', 100, 2, 20, 0,
        'artillery', 'heavy', ['destroyer_gun']),
    'tesla_tank': ('Tesla Tank', 'Specialized ground combat. Select and right click an enemy to attack.', 'vehicles', 'land', 'tesla', 100, 2, 20, 0,
        'heavy_tank', 'heavy', ['tesla_weapon']),
    'demolition_truck': ('Demolition Truck', 'Detonates on reaching attack range.', 'vehicles', 'land', 'suicide', 100, 3, 1600, 0,
        'harvester', 'light', ['suicide_trigger']),
    'harrier': ('Harrier', 'Fires a strike then returns to an air facility to rearm.', 'vehicles', 'air', 'strike', 100, 2, 20, 0,
        'scout', 'air', ['air_bomb']),
    'black_eagle': ('Black Eagle', 'Fires a strike then returns to an air facility to rearm.', 'vehicles', 'air', 'strike', 100, 2, 20, 0,
        'scout', 'air', ['eagle_bomb']),
    'nighthawk': ('NightHawk Transport', 'Select passengers; E + click this transport to load. D unloads.', 'vehicles', 'air', 'transport', 100, 2, 20, 5,
        'scout', 'air', ['air_rifle']),
    'kirov': ('Kirov Airship', 'Specialized air combat. Select and right click an enemy to attack.', 'vehicles', 'air', 'splash', 100, 2, 20, 0,
        'heavy_tank', 'air', ['kirov_bomb']),
    'amphibious_transport': ('Amphibious Transport', 'Select passengers; E + click this transport to load. D unloads.', 'vehicles', 'amphibious', 'transport', 100, 2, 20, 12,
        'harvester', 'naval', []),
    'destroyer': ('Destroyer', 'Specialized naval combat. Select and right click an enemy to attack.', 'vehicles', 'naval', 'combat', 100, 2, 20, 0,
        'artillery', 'naval', ['naval_gun']),
    'aegis_cruiser': ('Aegis Cruiser', 'Specialized naval combat. Select and right click an enemy to attack.', 'vehicles', 'naval', 'flak', 100, 2, 20, 0,
        'artillery', 'naval', ['aegis_missile']),
    'dolphin': ('Dolphin', 'Specialized naval combat. Select and right click an enemy to attack.', 'vehicles', 'naval', 'combat', 100, 2, 20, 0,
        'artillery', 'submerged', ['sonic']),
    'aircraft_carrier': ('Aircraft Carrier', 'Launches recoverable strike aircraft; no airbase required.', 'vehicles', 'naval', 'carrier', 100, 2, 20, 2,
        'artillery', 'naval', ['carrier_strike']),
    'typhoon': ('Typhoon Attack Submarine', 'Specialized naval combat. Select and right click an enemy to attack.', 'vehicles', 'naval', 'combat', 100, 2, 20, 0,
        'artillery', 'submerged', ['torpedo']),
    'sea_scorpion': ('Sea Scorpion', 'Specialized naval combat. Select and right click an enemy to attack.', 'vehicles', 'naval', 'flak', 100, 2, 20, 0,
        'artillery', 'naval', ['flak_weapon']),
    'giant_squid': ('Giant Squid', 'Immobilizes enemy vessels.', 'vehicles', 'naval', 'squid', 100, 2, 20, 0,
        'artillery', 'submerged', ['squid_grip']),
    'dreadnought': ('Dreadnought', 'Specialized naval combat. Select and right click an enemy to attack.', 'vehicles', 'naval', 'combat', 100, 2, 20, 0,
        'artillery', 'naval', ['dread_missile']),
}
ARMOUR_CLASSES = ['infantry', 'light', 'heavy', 'structure', 'air', 'naval', 'submerged']
WARHEADS = {
    'bullet': [100, 50, 15, 60, 0, 60, 0],
    'armour_piercing': [35, 80, 100, 50, 0, 50, 0],
    'rocket': [30, 100, 100, 70, 0, 70, 0],
    'high_explosive': [100, 75, 50, 80, 0, 80, 0],
    'shell': [90, 60, 40, 100, 0, 100, 0],
    'anti_air': [0, 0, 0, 0, 100, 0, 0],
    'flak': [50, 30, 15, 15, 100, 40, 0],
    'anti_infantry': [100, 0, 0, 0, 0, 0, 0],
    'anti_vehicle': [0, 100, 100, 0, 0, 80, 0],
    'naval': [25, 50, 50, 50, 0, 100, 100],
    'torpedo': [0, 0, 0, 0, 0, 100, 100],
    'omni': [50, 100, 100, 100, 100, 100, 0],
    'control': [100, 100, 100, 0, 0, 0, 0],
}
# id: (damage, range, reload, warhead, projectile_speed)
WEAPONS = {
    'rifle': (14, 1024, 15, 'bullet', 0),
    'carbine': (12, 896, 14, 'bullet', 0),
    'infantry_rocket': (45, 1280, 40, 'rocket', 192),
    'grenade': (35, 1024, 35, 'high_explosive', 128),
    'scout_gun': (9, 1024, 8, 'bullet', 0),
    'raider_gun': (10, 1024, 8, 'bullet', 0),
    'tank_cannon': (40, 1280, 30, 'armour_piercing', 160),
    'heavy_cannon': (60, 1280, 36, 'armour_piercing', 160),
    'artillery_shell': (80, 2560, 75, 'shell', 128),
    'rocket_salvo': (70, 2304, 70, 'rocket', 192),
    'turret_gun': (50, 1536, 35, 'high_explosive', 192),
    'tower_rocket': (45, 1792, 40, 'high_explosive', 224),
    'dog_bite': (200, 384, 12, 'anti_infantry', 0),
    'air_rifle': (25, 1280, 15, 'bullet', 0),
    'commando_gun': (150, 1536, 12, 'bullet', 0),
    'chrono_beam': (60, 1280, 30, 'control', 0),
    'sniper_rifle': (250, 3072, 70, 'anti_infantry', 0),
    'flak_weapon': (30, 1536, 20, 'flak', 0),
    'tesla_weapon': (100, 1280, 40, 'high_explosive', 0),
    'bomb_trigger': (1, 384, 120, 'high_explosive', 0),
    'control_beam': (1, 1536, 100, 'control', 0),
    'radiation_weapon': (50, 1024, 30, 'high_explosive', 0),
    'suicide_trigger': (1, 384, 100, 'high_explosive', 0),
    'drone_claw': (30, 384, 20, 'anti_vehicle', 0),
    'v3_missile': (160, 3840, 140, 'shell', 128),
    'mirage_gun': (100, 1792, 35, 'high_explosive', 0),
    'prism_beam': (100, 2560, 65, 'shell', 0),
    'apocalypse_gun': (120, 1792, 45, 'omni', 160),
    'destroyer_gun': (140, 1536, 40, 'anti_vehicle', 160),
    'air_bomb': (180, 1536, 60, 'shell', 0),
    'eagle_bomb': (260, 1536, 60, 'shell', 0),
    'kirov_bomb': (400, 512, 55, 'shell', 0),
    'naval_gun': (80, 1792, 40, 'naval', 160),
    'aegis_missile': (100, 2560, 20, 'anti_air', 192),
    'sonic': (100, 1536, 30, 'torpedo', 0),
    'carrier_strike': (250, 3840, 160, 'shell', 128),
    'torpedo': (120, 1792, 50, 'torpedo', 128),
    'squid_grip': (50, 384, 30, 'torpedo', 0),
    'dread_missile': (300, 4096, 180, 'shell', 128),
    'grand_shell': (200, 3584, 80, 'shell', 128),
}
HARVESTERS = {'chrono_miner': (500, 7), 'war_miner': (1000, 7)}

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
LATER = ("spawned", "tech_building", "support")
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


def speed(section):
    """Subcells per tick of a unit section, and the key it comes from (section 4.1)."""
    if section["Locomotor"].startswith(JUMPJET):
        key, leptons = "JumpjetSpeed", int(section["JumpjetSpeed"])
    else:
        key, leptons = "Speed", min(int(section["Speed"]) * 256 // 100, 255)
        if section.get("Category") == "Soldier":
            leptons = leptons * 3 // 2
    return max(1, rounded(Fraction(leptons * 3, 4))), key


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
        self.cost = int(section["Cost"])
        self.hit_points = int(section["Strength"])
        self.sight = int(section["Sight"])
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
        self.wall = building and flag("Wall", "no") == 1
        self.defence = building and section.get("BuildCat") == "Combat"
        self.build_limit = int(section.get("BuildLimit", "0"))
        self.adjacent = int(section["Adjacent"]) if building else 0
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
        buildings[game_id] = values + (game_id,)
    for game_id, (label, twin) in NEW_BUILDINGS.items():
        buildings[game_id] = (label,) + BUILDINGS[twin][1:] + (twin,)
    for game_id, values in UNITS.items():
        units[game_id] = values + (game_id,)
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
            value, key = speed(item.ini)
            keys = (key,) if key == "JumpjetSpeed" else (key, "Category")
            lines.append(
                f'        (id: "{game_id}", speed: {value}, sprite: "{values[9]}"),'
                f" // {item.comment(*keys)}"
            )
        lines += ["    ],", ")"]
        return "\n".join(lines) + "\n"

    # --- economy.ron -------------------------------------------------------------------------

    def economy_file(self):
        refund = cite("General", "RefundPercent", self.tables.section("General"))
        speed = cite("General", "BuildSpeed", self.tables.section("General"))
        wall = cite("General", "WallBuildSpeedCoefficient", self.tables.section("General"))
        lines = [
            "// Economy definitions, schema version 1 (PLAN.md sections 5 and 7, M3; docs/DECISIONS.md D23).",
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
            "// harvesters: ore capacity and ore removed per tick of the harvester unit types",
            "// refineries: credits paid per ore unit by the refinery building types",
            "//",
            "// Cost, prerequisites, power, sell value and free units are ported from the ini. The build",
            f"// time is the cost times {speed} game minutes per 1000 credits, in ticks",
            f"// (round(cost x 21 / 25), at least 1), and for a wall times {wall}",
            "// (docs/RA2_PARITY_PLAN.md section 4.2). Footprints, harvesters and refineries are the",
            "// game's own (the latter two until RA5).",
            "//",
            GENERATED,
            "(",
            "    schema_version: 1,",
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
        for game_id, (capacity, rate) in HARVESTERS.items():
            lines.append(f'        (unit: "{game_id}", capacity: {capacity}, harvest_rate: {rate}),')
        lines += ["    ],", "    refineries: ["]
        for game_id in self.buildings:
            if "Refinery" in self.items[game_id].ini:
                lines.append(
                    f'        (building: "{game_id}", credits_per_ore: 1),'
                    f" // {self.items[game_id].comment('Refinery')}"
                )
        lines += ["    ],", ")"]
        return "\n".join(lines) + "\n"

    # --- combat.ron --------------------------------------------------------------------------

    def combat_file(self):
        lines = [
            "// Combat definitions, schema version 1 (PLAN.md sections 5 and 7, M4; docs/DECISIONS.md D28).",
            "//",
            "// armour_classes: the armour classes",
            "// warheads:   the percentage of a weapon's base damage each warhead deals against every armour",
            "//             class (equation 2: damage = base x percent / 100, rounded down, at least 1 unless",
            "//             the percentage is 0); every armour class exactly once",
            "// weapons:    base damage; range in subcells (256 per cell; at least 384); reload in ticks",
            "//             (20 per second); warhead; projectile_speed in subcells per tick, or 0 for an",
            "//             instant weapon",
            "// units:      hit points, armour class, sight radius in cells, and at most one weapon of every",
            "//             unit type of units.ron",
            "// buildings:  the same for every building type of economy.ron; a building with a weapon is a",
            "//             defensive structure, which does not fire under low power if it is `powered`",
            "//             and consumes power (arsenal.ron, economy.ron)",
            "//",
            "// Hit points and sight are ported from the ini; armour classes, warheads and weapons are the",
            "// game's own until RA3.",
            "//",
            GENERATED,
            "(",
            "    schema_version: 1,",
            f"    armour_classes: {quoted(ARMOUR_CLASSES)},",
            "    warheads: [",
        ]
        for warhead, versus in WARHEADS.items():
            entries = ", ".join(
                f'(armour: "{armour}", percent: {percent})'
                for armour, percent in zip(ARMOUR_CLASSES, versus)
            )
            lines.append(f'        (id: "{warhead}", versus: [{entries}]),')
        lines += ["    ],", "    weapons: ["]
        for weapon, (damage, reach, reload, warhead, projectile) in WEAPONS.items():
            lines.append(
                f'        (id: "{weapon}", damage: {damage}, range: {reach}, reload: {reload}, '
                f'warhead: "{warhead}", projectile_speed: {projectile}),'
            )
        for name, key, table, armour, weapons in (
            ("units", "unit", self.units, 10, 11),
            ("buildings", "building", self.buildings, 12, 13),
        ):
            lines += ["    ],", f"    {name}: ["]
            for game_id, values in table.items():
                item = self.items[game_id]
                lines.append(
                    f'        ({key}: "{game_id}", hit_points: {item.hit_points}, '
                    f'armour: "{values[armour]}", sight: {item.sight}, '
                    f"weapons: {quoted(values[weapons])}),"
                    f" // {item.comment('Strength', 'Sight')}"
                )
        lines += ["    ],", ")"]
        return "\n".join(lines) + "\n"

    # --- arsenal.ron -------------------------------------------------------------------------

    def arsenal_file(self):
        lines = [
            "// The arsenal, schema version 3 (crates/sim/src/arsenal.rs; docs/DECISIONS.md R10, R11, R22): per",
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
            GENERATED,
            "(schema_version: 3, entries: [",
        ]
        for table in (self.buildings, self.units):
            for game_id, values in table.items():
                item = self.items[game_id]
                label, description, category, domain, role, cooldown, radius, strength, capacity = (
                    values[:9]
                )
                keys = ["Owner", "RequiredHouses", "ForbiddenHouses", "TechLevel", "Prerequisite"]
                keys += ["Factory", "Naval"] + list(STOLEN)
                if item.kind == "building":
                    if (category == "armory") != item.defence:
                        raise Failure(f"{game_id}: the category {category} is not its BuildCat")
                    keys += ["BuildCat", "BuildLimit", "Adjacent", "BaseNormal", "Radar", "Wall"]
                    keys += ["Unsellable", "Repairable", "Powered", "UnitRepair", "SuperWeapon"]
                source = item.comment(*keys)
                if item.kind == "building":
                    if item.special:
                        special = self.tables.section(item.special)
                        source += f"; {cite(item.special, 'IsPowered', special)}"
                    if item.far_shipyard:
                        source += "; adjacent is the game's own (ra2_differences.tsv)"
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
                    f"unit_repair: {item.unit_repair}),"
                    f" // {source}"
                )
        lines.append("])")
        return "\n".join(lines) + "\n"

    # --- rules.ron ---------------------------------------------------------------------------

    def rules_file(self):
        tables = self.tables
        lines = [
            "// Global rules, schema version 1 (crates/sim/src/rules.rs; docs/RA2_PARITY_PLAN.md sections 3.2",
            "// and 4; docs/DECISIONS.md R9, R10): the values of the ruleset that belong to no single unit or",
            "// building, in game units (ticks, 20 per second; subcells, 256 per cell; permille; percent;",
            "// yes and no as 1 and 0).",
            "//",
            GENERATED,
            "(",
            "    schema_version: 1,",
        ]

        def scalars(group):
            lines.append(f"    {group}: (")
            for name, field, section, key, kind, _ in RULES:
                if name == group:
                    value = convert(kind, tables.value(section, key))
                    lines.append(
                        f"        {field}: {value}, // {cite(section, key, tables.section(section))}"
                    )
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
