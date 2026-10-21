#!/usr/bin/env python3
# Extracts the committed reference table of the RA2 parity programme from the vanilla Red
# Alert 2 `rules.ini` and `art.ini` (docs/RA2_PARITY_PLAN.md section 3.1).
#
# Input, never committed: the two ini files in ~/Library/Caches/MyGame001/ra2-reference/ (or in
# the directory named by RA2_REFERENCE_DIR), read through scripts/ra2_ini.py.
# Input, committed: crates/sim/tests/reference/ra2_mapping.tsv (game id -> ini section).
# Output, committed: crates/sim/tests/reference/ra2_reference.tsv (file, section, key, value)
# and ra2_reference.tsv.sha256 (the SHA-256 of the table as it was written, so that a table
# edited by hand is found also where the ini files are not at hand).
#
# Provenance (docs/DECISIONS.md, R2): repository https://github.com/otherdeniz/CnC_RulesEditor,
# commit b7278f2e1dab098c5a2f05d4784fffe70a4e5f9c, files
# Deniz.TiberiumSunEditor.Gui/Resources/RA2rules.ini and RA2art.ini, with the SHA-256 values
#     rules.ini  adc869ca037db5141f562f18092d022045e194de0d1204a46b9ec8372b2b826f
#     art.ini    d732bfc8ffb6d52908342254c22731acb730ae6d2bbfb41983ef109e48a8f061
# The script refuses to run on files with other hashes, so that the table always describes
# the recorded reference.
#
# Only numbers, percentages, yes/no flags and identifiers (section names, weapon, warhead,
# projectile and type ids, house names) are extracted. Nothing else of the ini files reaches
# the repository: no comments, no names, no interface strings, no image, voice or sound names,
# and no animation names with one exception: a [Powerups] value is one list (share, animation,
# whether it works on water, argument), kept whole, so the table and the comments of
# `assets/data/rules.ron` carry the short identifier of each crate's animation (ARMOR, MONEY,
# ...), as they carry a building's `Image` id; the game reads the share and the argument only.
# Two rules enforce this:
#   * sections of units, buildings, weapons, warheads, projectiles, superweapons, countries,
#     land types and ore types are filtered by the explicit key lists below; a key that is
#     not listed is never written;
#   * the global sections of WHOLE_SECTIONS are taken whole, minus the keys listed with them,
#     and minus every key whose value is not a number, a flag or a list of identifiers.
# A value with a blank, a colon or any other character outside VALUE_SHAPE is never written.
# Of the type lists ([InfantryTypes], [VehicleTypes], [AircraftTypes], [BuildingTypes]) only the
# entries that name a mapped section are written (since RA3: the list a unit belongs to decides
# whether it is infantry).
#
# Usage (standard library only, Python 3):
#     python3 scripts/ra2_reference.py            regenerate the table
#     python3 scripts/ra2_reference.py --check    exit 1 if the committed table is not the one
#                                                 this script wrote (its recorded SHA-256) or
#                                                 is stale; without the cache only the first
#                                                 is checked, and the note says so (exit 0)
#     python3 scripts/ra2_reference.py --stats    counts per category, dangling references
#     python3 scripts/ra2_reference.py --ra9-dispositions [--check]
#         compose/check exact proposed RA9 dispositions; never records acceptance or receipts
# Exit code 2 reports an error (missing file, wrong hash, mapping that contradicts the ini).
"""Writes the committed RA2 reference table from the cached ini files."""

import hashlib
import os
import re
import sys

import ra2_ini

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REFERENCE_DIR = os.path.join(ROOT, "crates", "sim", "tests", "reference")
MAPPING_PATH = os.path.join(REFERENCE_DIR, "ra2_mapping.tsv")
TABLE_PATH = os.path.join(REFERENCE_DIR, "ra2_reference.tsv")
TABLE_HASH_PATH = TABLE_PATH + ".sha256"

SHA256 = {
    "rules": "adc869ca037db5141f562f18092d022045e194de0d1204a46b9ec8372b2b826f",
    "art": "d732bfc8ffb6d52908342254c22731acb730ae6d2bbfb41983ef109e48a8f061",
}

MAPPING_HEADER = ("game_id", "kind", "section", "side", "note")
TABLE_HEADER = ("file", "section", "key", "value")
KINDS = ("unit", "building")
SIDES = ("allied", "soviet", "none")

# A written value is a comma separated list of tokens of these characters (`<none>` is the
# empty slot of [Powerups]), or the class id of a locomotor.
VALUE_SHAPE = re.compile(r"^(<none>|[A-Za-z0-9_.%+-]*)(,(<none>|[A-Za-z0-9_.%+-]*))*$")
GUID_SHAPE = re.compile(r"^\{[0-9A-Fa-f-]+\}$")

# Keys of unit and building sections: the list the plan asks for.
OBJECT_KEYS = (
    "Cost", "Strength", "Armor", "Sight", "Speed", "JumpjetSpeed", "TechLevel",
    "Prerequisite", "PrerequisiteOverride", "Owner", "RequiredHouses", "ForbiddenHouses",
    "Primary", "Secondary", "ElitePrimary", "EliteSecondary", "WeaponCount", "TurretCount",
    "Power", "Powered", "Adjacent", "BaseNormal", "BuildLimit", "BuildCat", "Factory",
    "Naval", "WaterBound", "Radar", "SpySat", "GapGenerator", "GapRadiusInCells",
    "SuperGapRadiusInCells", "PsychicDetectionRadius", "SuperWeapon", "SuperWeapon2",
    "FreeUnit", "Dock", "NumberOfDocks", "UnitRepair", "UnitReload", "Refinery", "Storage",
    "Harvester", "Teleporter", "DeploysInto", "UndeploysInto", "Crusher", "Crushable",
    "Size", "SizeLimit", "Passengers", "PhysicalSize", "Gunner", "IFVMode", "Occupier",
    "CanBeOccupied", "MaxNumberOccupants", "Deployer", "DeployFire", "UndeployDelay", "C4",
    "CanC4", "Engineer", "Agent", "Infiltrate", "Spyable", "Capturable", "NeedsEngineer",
    "Unsellable", "Repairable", "Crewed", "Trainable", "VeteranAbilities", "EliteAbilities",
    "SelfHealing", "Ammo", "Fighter", "Landable", "AirportBound", "Spawns", "SpawnsNumber",
    "SpawnRegenRate", "SpawnReloadRate", "MissileSpawn", "Cloakable", "CloakingSpeed",
    "Underwater", "Sensors", "SensorsSight", "DetectDisguise", "DetectDisguiseRange",
    "CanDisguise", "DisguiseWhenStill", "PermaDisguise", "ImmuneToPsionics",
    "ImmuneToRadiation", "Bombable", "Explodes", "DeathWeapon", "Category", "Soylent",
    "Points", "CrateGoodie", "AllowedToStartInMultiplayer", "Wall", "Overpowerable",
    "OrePurifier", "Hospital", "BridgeRepairHut", "Immune", "ProduceCashStartup",
    "ProduceCashAmount", "ProduceCashDelay", "RequiresStolenAlliedTech",
    "RequiresStolenSovietTech", "ExtraPower", "AirRangeBonus", "MovementZone", "SpeedType",
    "Locomotor", "Turret", "ROT", "TooBigToFitUnderBridge", "Insignificant", "DontScore",
    "ThreatPosed",
)

# Further numeric and flag keys of the mapped sections that a rules implementation may need.
OBJECT_KEYS_ADDED = (
    "AIBasePlanningSide", "AIBuildThis", "Assaulter", "AutoCrush", "BalloonHover",
    "BombSight", "CanApproachTarget", "CanPassiveAquire", "CanRetaliate", "ChargedAnimTime",
    "Cloning", "ConsideredAircraft", "ConstructionYard", "Crashable",
    "DeathWeaponDamageModifier", "DefaultToGuardArea", "DeployTime", "DistributedFire",
    "DockUnload", "EligibileForAllyBuilding", "Fearless", "GDIBarracks", "GuardRange",
    "Helipad", "HoverAttack", "IsBase", "IsBaseDefense", "IsChargeTurret",
    "IsImmuneToRadiation", "IsSelectableCombatant", "Ivan", "JumpJet", "JumpJetAccel",
    "JumpJetTurnRate", "JumpjetClimb", "JumpjetHeight", "LandTargeting", "LeadershipRating",
    "LegalTarget", "MoveToShroud", "MovementRestrictedTo", "NODBarracks", "NavalTargeting",
    "Nominal", "NotHuman", "NukeSilo", "NumberImpassableRows", "OpportunityFire", "Organic",
    "Parasiteable", "PreventAttackMove", "ProtectWithWall", "RadarInvisible", "RevealToAll",
    "Selectable", "SensorArray", "Spawned", "SpecialThreatValue", "StupidHunt",
    "SuppressionThreshold", "ThreatAvoidanceCoefficient", "TiberiumProof", "ToProtect",
    "TogglePower", "TypeImmune", "Upgrades", "VHPScan", "WeaponsFactory", "Weight",
)

# `Image` of a building is written because it names the art.ini section that holds the
# building's `Foundation`; for units it is presentation only and is not written.
BUILDING_KEYS_ADDED = ("Image",)

# Numbered and prefixed keys: Weapon1..N, EliteWeapon1..N, and the passenger weapon table
# of the IFV (`<Mode>TurretIndex`, `<Mode>TurretWeapon`).
OBJECT_KEY_PATTERNS = (
    re.compile(r"^(Elite)?Weapon[0-9]+$", re.IGNORECASE),
    re.compile(r"^[A-Za-z]+Turret(Index|Weapon)$", re.IGNORECASE),
)

WEAPON_KEYS = (
    "Damage", "ROF", "Range", "MinimumRange", "Speed", "Warhead", "Projectile", "Burst",
    "RadLevel", "Suicide", "AreaFire", "Spawner", "DecloakToFire", "RevealOnFire",
    "CellRangefinding", "AmbientDamage", "TurboBoost", "OmniFire", "FireOnce", "Supress",
    "LimboLaunch", "DiskLaser", "IsElectricBolt", "IsHouseColor", "IsRadBeam", "IsSonic",
    "IsLaser",
)
WEAPON_KEYS_ADDED = ("DisguiseFireOnly", "IsBigLaser", "IsRadEruption", "Lobber", "NeverUse")

WARHEAD_KEYS = (
    "Verses", "CellSpread", "PercentAtMax", "Wall", "Wood", "WallAbsoluteDestroyer",
    "Conventional", "ProneDamage", "Temporal", "MindControl", "ElectricAssault",
    "MakesDisguise", "Parasite", "Paralyzes", "Culling", "Sonic", "Radiation",
    "PsychicDamage", "BombDisarm", "NukeMaker", "IvanBomb", "Sparky",
)
WARHEAD_KEYS_ADDED = (
    "Bullets", "CausesDelayKill", "CellInset", "DelayKillAtMax", "DelayKillFrames",
    "EMEffect", "Fire", "Tiberium",
)

PROJECTILE_KEYS = (
    "AA", "AG", "AS", "Arcing", "Inaccurate", "FlakScatter", "Inviso", "ROT",
    "Acceleration", "Ranged", "SubjectToCliffs", "SubjectToElevation", "SubjectToWalls",
    "Proximity", "Vertical", "Cluster", "ShrapnelWeapon", "ShrapnelCount",
)
PROJECTILE_KEYS_ADDED = ("Arm", "CourseLockDuration", "DetonationAltitude", "High", "Level")

SUPERWEAPON_KEYS = (
    "RechargeTime", "IsPowered", "ShowTimer", "WeaponType", "PreDependent", "Action",
    "Type", "PreClick", "PostClick", "DisableableFromShell",
)
COUNTRY_KEYS = ("Side", "Multiplay")
LAND_KEYS = (
    "Foot", "Track", "Wheel", "Float", "Hover", "Amphibious", "FloatBeach", "Buildable",
)
ORE_KEYS = ("Value", "Growth", "GrowthPercentage", "Spread", "SpreadPercentage", "Power")
ART_KEYS = ("Foundation", "Height")

LAND_SECTIONS = (
    "Clear", "Rough", "Road", "Water", "Rock", "Wall", "Tiberium", "Weeds", "Beach", "Ice",
    "Railroad", "Tunnel",
)
ORE_SECTIONS = ("Riparius", "Cruentus")

# Global sections taken whole, each with the keys that are left out because they name
# images, sounds, animations, particle systems or colours.
WHOLE_SECTIONS = {
    "General": (
        "Name", "UIName", "OKBuildingSmokeSystem", "DamagedBuildingSmokeSystem",
        "DamagedUnitSmokeSystem", "DebrisSmokeSystem", "ExplosiveVoxelDebris",
        "TireVoxelDebris", "ScrapVoxelDebris",
    ),
    "CombatDamage": (
        "Scorches", "Scorches1", "Scorches2", "Scorches3", "Scorches4", "Craters",
        "SplashList", "DefaultFirestormExplosionSystem", "DefaultLargeGreySmokeSystem",
        "DefaultSmallGreySmokeSystem", "DefaultLargeRedSmokeSystem",
        "DefaultSmallRedSmokeSystem", "DefaultDebrisSmokeSystem", "DefaultFireStreamSystem",
        "DefaultTestParticleSystem", "DefaultRepairParticleSystem",
    ),
    "Radiation": ("RadColor",),
    "CrateRules": ("HealCrateSound", "WoodCrateImg", "CrateImg", "WaterCrateImg"),
    "Powerups": (),
    "MultiplayerDialogSettings": (),
    "Maximums": (),
    "Easy": (),
    "Normal": (),
    "Difficult": (),
    "JumpjetControls": (),
    "SpecialWeapons": (),
    "SuperWeaponTypes": (),
    "AI": (),
    "IQ": (),
    "Sides": (),
    "Countries": (),
    "Tiberiums": (),
}

# Global sections of which only the listed keys are written.
PARTIAL_SECTIONS = {
    "AudioVisual": ("ConditionYellow", "ConditionRed", "Gravity"),
    "Harvest": ("Rate",),
}

# Keys of a unit or building section that name weapons.
WEAPON_REFERENCE = re.compile(
    r"^((Elite)?(Primary|Secondary)|(Elite)?Weapon[0-9]+|DeathWeapon)$", re.IGNORECASE
)
# Keys of global sections that name a weapon, a warhead or a projectile.
GLOBAL_WEAPON_KEYS = (("CombatDamage", "DeathWeapon"), ("General", "DropPodWeapon"))
GLOBAL_WARHEAD_KEYS = (
    ("General", "LightningWarhead"), ("CombatDamage", "C4Warhead"),
    ("CombatDamage", "V3Warhead"), ("CombatDamage", "V3EliteWarhead"),
    ("CombatDamage", "DMislWarhead"), ("CombatDamage", "DMislEliteWarhead"),
    ("CombatDamage", "IvanWarhead"), ("CombatDamage", "FlameDamage"),
    ("CombatDamage", "FlameDamage2"), ("CombatDamage", "FirestormWarhead"),
    ("CombatDamage", "IonCannonWarhead"), ("CombatDamage", "VeinholeWarhead"),
    ("Radiation", "RadSiteWarhead"), ("SpecialWeapons", "NukeWarhead"),
    ("SpecialWeapons", "EMPulseWarhead"),
)
GLOBAL_PROJECTILE_KEYS = (
    ("SpecialWeapons", "NukeProjectile"), ("SpecialWeapons", "NukeDown"),
    ("SpecialWeapons", "EMPulseProjectile"),
)
# Weapons the engine finds by name, which no key of a mapped section names.
ENGINE_WEAPONS = (
    "AlliedOccupyW", "SovietOccupyW", "NukePayload", "NukeCarrier", "IvanBomber",
    "PrismSupport", "OPCoilBolt", "AssaultBolt", "RadEruptionWeapon", "PsiWave",
    "DefuseKit", "Sapper", "MakeupKit", "HornetBomb", "ASWBomb", "ParaDropWeapon",
)
# Keys of a unit or building section that name other units or buildings.
OBJECT_REFERENCE_KEYS = (
    "Prerequisite", "PrerequisiteOverride", "Spawns", "DeploysInto", "UndeploysInto",
    "FreeUnit", "Dock",
)
# Prerequisite tokens that name a group of [General] and not a section.
PREREQUISITE_GROUPS = ("POWER", "FACTORY", "BARRACKS", "RADAR", "TECH", "PROC")
UNIT_REGISTRIES = ("InfantryTypes", "VehicleTypes", "AircraftTypes")
BUILDING_REGISTRIES = ("BuildingTypes",)

CATEGORIES = (
    "unit", "building", "weapon", "warhead", "projectile", "superweapon", "global",
    "country", "land", "ore", "art", "list",
    "presentation_premise",
)


class ExtractionError(Exception):
    """A condition that makes the extraction impossible or untrustworthy."""


def normalise(value):
    """Trim a value and remove the blanks around its commas."""
    return re.sub(r"\s*,\s*", ",", value.strip())


def tokens(value):
    """The non-empty items of a comma separated value, without `none`."""
    return [item for item in value.split(",") if item and item.lower() != "none"]


class Table:
    """The rows of the reference table and the category of every section."""

    def __init__(self):
        self.rows = {}  # (file, section as written, key) -> value
        self.categories = {}  # (file, lower-case section) -> category
        self.blank = 0  # allow-listed keys left out because their value is empty
        self.shaped = 0  # keys of whole sections left out by VALUE_SHAPE

    def _claim(self, file, section, category):
        self.categories.setdefault((file, section.lower()), category)

    def has_section(self, file, section):
        return (file, section.lower()) in self.categories

    def add_listed(self, category, file, ini, section, keys, patterns=()):
        """Write the keys of `section` that are in `keys` or match one of `patterns`."""
        values = ini.section(section)
        if values is None:
            return False
        name = ini.section_name(section)
        canonical = {key.lower(): key for key in keys}
        for key in sorted(values):
            written = canonical.get(key.lower())
            if written is None and any(pattern.match(key) for pattern in patterns):
                written = key
            if written is None:
                continue
            value = normalise(values[key])
            if not value:
                self.blank += 1
                continue
            shape = GUID_SHAPE if written == "Locomotor" else VALUE_SHAPE
            if not shape.match(value):
                raise ExtractionError(
                    "[%s] %s has a value that is not a number, flag or identifier list"
                    % (name, written)
                )
            self.rows[(file, name, written)] = value
            self._claim(file, name, category)
        return True

    def add_whole(self, category, file, ini, section, excluded):
        """Write every key of `section` that is not excluded and has a plain value."""
        values = ini.section(section)
        if values is None:
            raise ExtractionError("the ini has no section [%s]" % section)
        name = ini.section_name(section)
        skip = {key.lower() for key in excluded}
        for key in sorted(values):
            if key.lower() in skip:
                continue
            value = normalise(values[key])
            if not value:
                self.blank += 1
            elif not VALUE_SHAPE.match(value):
                self.shaped += 1
            else:
                self.rows[(file, name, key)] = value
                self._claim(file, name, category)

    def add_registry(self, file, ini, section, wanted):
        """Write the entries of the type list `section` whose value is in `wanted` (lower-case
        section names)."""
        values = ini.section(section)
        if values is None:
            raise ExtractionError("the ini has no section [%s]" % section)
        name = ini.section_name(section)
        for key in sorted(values):
            value = normalise(values[key])
            if value.lower() not in wanted:
                continue
            if not VALUE_SHAPE.match(key) or not VALUE_SHAPE.match(value):
                raise ExtractionError("[%s] %s is not an identifier" % (name, key))
            self.rows[(file, name, key)] = value
            self._claim(file, name, "list")

    def render(self):
        lines = ["\t".join(TABLE_HEADER)]
        for key in sorted(self.rows):
            lines.append("\t".join(key + (self.rows[key],)))
        return "\n".join(lines) + "\n"


def sha256_of(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 16), b""):
            digest.update(block)
    return digest.hexdigest()


def load_reference():
    """Load both ini files after checking their hashes; returns (rules, art)."""
    loaded = []
    for which in ("rules", "art"):
        path = ra2_ini.default_path(which)
        if not os.path.isfile(path):
            raise ExtractionError("reference file %s is missing (docs/DECISIONS.md R2)" % path)
        found = sha256_of(path)
        if found != SHA256[which]:
            raise ExtractionError(
                "%s has SHA-256 %s, but the recorded reference has %s; the table is only"
                " valid for the files of docs/DECISIONS.md R2, so nothing was written"
                % (path, found, SHA256[which])
            )
        loaded.append(ra2_ini.load(path))
    return loaded[0], loaded[1]


def load_mapping(path=MAPPING_PATH):
    """Rows of the mapping table as dicts, in file order."""
    with open(path, "r", encoding="utf-8", newline="") as handle:
        lines = handle.read().split("\n")
    if lines[0].split("\t") != list(MAPPING_HEADER):
        raise ExtractionError("%s: unexpected header" % path)
    rows = []
    seen = set()
    for number, line in enumerate(lines[1:], start=2):
        if not line:
            continue
        fields = line.split("\t")
        if len(fields) != len(MAPPING_HEADER):
            raise ExtractionError("%s:%d: expected %d columns" % (path, number, 5))
        row = dict(zip(MAPPING_HEADER, fields))
        if row["kind"] not in KINDS or row["side"] not in SIDES:
            raise ExtractionError("%s:%d: unknown kind or side" % (path, number))
        if row["section"].lower() in seen:
            raise ExtractionError("%s:%d: section listed twice" % (path, number))
        seen.add(row["section"].lower())
        rows.append(row)
    return rows


def house_sides(rules):
    """House name (lower case) -> `allied` or `soviet`, from [Sides]."""
    sides = {}
    for key, side in (("GDI", "allied"), ("Nod", "soviet")):
        for house in tokens(normalise(rules.get("Sides", key, ""))):
            sides[house.lower()] = side
    return sides


def registry(rules, names):
    """Lower-case type ids listed in the given registry sections."""
    listed = set()
    for name in names:
        listed.update(value.lower() for value in (rules.section(name) or {}).values())
    return listed


def derive_side(rules, section, houses, buildings, visiting=()):
    """Side of a type: by its houses, else by the buildings it requires, else `none`.

    The houses are `Owner` without `ForbiddenHouses`, restricted to `RequiredHouses` when
    that list is given. A type open to both sides takes the side of its prerequisite
    buildings (the construction yard for most buildings, the barracks for the walls).
    """
    def listed(key):
        return {item.lower() for item in tokens(normalise(rules.get(section, key, "")))}

    owners = listed("Owner") - listed("ForbiddenHouses")
    if listed("RequiredHouses"):
        owners &= listed("RequiredHouses")
    sides = {houses[house] for house in owners if house in houses}
    if len(sides) == 1:
        return sides.pop()
    sides = set()
    for item in sorted(listed("Prerequisite")):
        if item in buildings and item not in visiting:
            side = derive_side(rules, item, houses, buildings, visiting + (section.lower(),))
            if side != "none":
                sides.add(side)
    return sides.pop() if len(sides) == 1 else "none"


def check_mapping(rules, mapping):
    """Every mapping row names a section of its kind and the side the ini gives."""
    houses = house_sides(rules)
    units = registry(rules, UNIT_REGISTRIES)
    buildings = registry(rules, BUILDING_REGISTRIES)
    problems = []
    for row in mapping:
        section = row["section"]
        where = "%s (%s)" % (row["game_id"], section)
        if not rules.has_section(section):
            problems.append("%s: the ini has no such section" % where)
            continue
        if rules.section_name(section) != section:
            problems.append("%s: the ini writes the section differently" % where)
        kind = "building" if section.lower() in buildings else "unit"
        if section.lower() not in buildings and section.lower() not in units:
            problems.append("%s: the section is in no type list of the ini" % where)
        elif kind != row["kind"]:
            problems.append("%s: kind is %s, the ini says %s" % (where, row["kind"], kind))
        side = derive_side(rules, section, houses, buildings)
        if side != row["side"]:
            problems.append("%s: side is %s, the ini says %s" % (where, row["side"], side))
    if problems:
        raise ExtractionError("ra2_mapping.tsv contradicts the ini:\n  " + "\n  ".join(problems))


def extract(rules, art, mapping):
    """Build the reference table."""
    table = Table()
    object_keys = OBJECT_KEYS + OBJECT_KEYS_ADDED
    weapons = set(ENGINE_WEAPONS)
    for row in mapping:
        section = row["section"]
        keys = object_keys + (BUILDING_KEYS_ADDED if row["kind"] == "building" else ())
        table.add_listed(row["kind"], "rules", rules, section, keys, OBJECT_KEY_PATTERNS)
        for key, value in rules.section(section).items():
            if WEAPON_REFERENCE.match(key):
                weapons.update(tokens(normalise(value)))
        if row["kind"] == "building":
            image = rules.get(section, "Image") or section
            if not table.add_listed("art", "art", art, image, ART_KEYS):
                raise ExtractionError("art.ini has no section [%s] for [%s]" % (image, section))

    # The type lists, as far as they name mapped sections.
    mapped = {row["section"].lower() for row in mapping}
    for section in UNIT_REGISTRIES + BUILDING_REGISTRIES:
        table.add_registry("rules", rules, section, mapped)

    for section, excluded in sorted(WHOLE_SECTIONS.items()):
        table.add_whole("global", "rules", rules, section, excluded)
    for section, keys in sorted(PARTIAL_SECTIONS.items()):
        table.add_listed("global", "rules", rules, section, keys)
    # RA9 exact presentation-premise metadata closure. No generic particle engine or
    # copied assets/comments. The recorded original-source hash remains mandatory.
    # These seven present metadata atoms are guard inputs, never native coverage.
    if rules.get("CombatDamage", "DefaultSparkSystem") != "SparkSys":
        raise ExtractionError("RA9 original default Spark premise changed")
    spark_premises = {
        "SparkSys": ("HoldsWhat", "BehavesLike", "ParticleCap", "SparkSpawnFrames"),
        "Spark": ("BehavesLike", "MaxEC"),
    }
    for section, keys in sorted(spark_premises.items()):
        if not table.add_listed("presentation_premise", "rules", rules, section, keys):
            raise ExtractionError("RA9 source Spark premise section missing: " + section)
    if rules.get("SparkSys", "HoldsWhat") != "Spark" or any(
        rules.get(section, "BehavesLike") != "Spark" for section in spark_premises
    ):
        raise ExtractionError("RA9 default particle chain is not the exact Spark branch")
    if any(rules.get("Spark", key) is not None for key in ("Damage", "Warhead", "NextParticle")):
        raise ExtractionError("RA9 Spark non-damage/unchained absence premise changed")

    for section in LAND_SECTIONS:
        table.add_listed("land", "rules", rules, section, LAND_KEYS)
    for section in ORE_SECTIONS:
        table.add_listed("ore", "rules", rules, section, ORE_KEYS)
    for house in sorted(house_sides(rules)):
        table.add_listed("country", "rules", rules, house, COUNTRY_KEYS)
    for name in sorted(tokens(",".join(rules.section("SuperWeaponTypes").values()))):
        table.add_listed("superweapon", "rules", rules, name, SUPERWEAPON_KEYS)
        weapons.update(tokens(normalise(rules.get(name, "WeaponType", ""))))

    for section, key in GLOBAL_WEAPON_KEYS:
        weapons.update(tokens(normalise(rules.get(section, key, ""))))
    warheads = set()
    projectiles = set()
    for section, key in GLOBAL_WARHEAD_KEYS:
        warheads.update(tokens(normalise(rules.get(section, key, ""))))
    for section, key in GLOBAL_PROJECTILE_KEYS:
        projectiles.update(tokens(normalise(rules.get(section, key, ""))))

    # Weapons name warheads and projectiles; a projectile may name a shrapnel weapon.
    pending = sorted(weapons)
    done = set()
    while pending:
        weapon = pending.pop(0)
        if weapon.lower() in done:
            continue
        done.add(weapon.lower())
        table.add_listed("weapon", "rules", rules, weapon, WEAPON_KEYS + WEAPON_KEYS_ADDED)
        warheads.update(tokens(normalise(rules.get(weapon, "Warhead", ""))))
        for projectile in tokens(normalise(rules.get(weapon, "Projectile", ""))):
            projectiles.add(projectile)
            pending.extend(tokens(normalise(rules.get(projectile, "ShrapnelWeapon", ""))))
    for warhead in sorted(warheads):
        table.add_listed("warhead", "rules", rules, warhead, WARHEAD_KEYS + WARHEAD_KEYS_ADDED)
    for projectile in sorted(projectiles):
        table.add_listed(
            "projectile", "rules", rules, projectile, PROJECTILE_KEYS + PROJECTILE_KEYS_ADDED
        )
    return table


def references(table):
    """(section, key, target, wanted category) for every reference written to the table."""
    global_keys = {}
    for wanted, pairs in (
        ("weapon", GLOBAL_WEAPON_KEYS),
        ("warhead", GLOBAL_WARHEAD_KEYS),
        ("projectile", GLOBAL_PROJECTILE_KEYS),
    ):
        for section, key in pairs:
            global_keys[(section.lower(), key.lower())] = wanted
    object_keys = {key.lower() for key in OBJECT_REFERENCE_KEYS}
    found = []
    for (file, section, key), value in sorted(table.rows.items()):
        if file != "rules":
            continue
        category = table.categories[(file, section.lower())]
        wanted = global_keys.get((section.lower(), key.lower()))
        if category in KINDS and WEAPON_REFERENCE.match(key):
            wanted = "weapon"
        elif category in KINDS and key.lower() in object_keys:
            wanted = "object"
        elif category in KINDS and key in ("SuperWeapon", "SuperWeapon2"):
            wanted = "superweapon"
        elif category == "weapon" and key in ("Warhead", "Projectile"):
            wanted = key.lower()
        elif category == "projectile" and key == "ShrapnelWeapon":
            wanted = "weapon"
        elif category == "superweapon" and key == "WeaponType":
            wanted = "weapon"
        if wanted is None:
            continue
        for target in tokens(value):
            if wanted == "object" and target.upper() in PREREQUISITE_GROUPS:
                continue
            found.append((section, key, target, wanted))
    return found


def dangling(table, rules):
    """References of the table whose target section is not in the table, with the reason."""
    missing = []
    for section, key, target, wanted in references(table):
        if table.has_section("rules", target):
            continue
        if not rules.has_section(target):
            reason = "the ini has no such section"
        elif wanted == "object":
            reason = "not in ra2_mapping.tsv"
        else:
            reason = "the section has none of the %s keys" % wanted
        missing.append((wanted, section, key, target, reason))
    return sorted(missing)


def parse_table(text):
    """Table text -> {(file, section, key): value}."""
    rows = {}
    for line in text.split("\n")[1:]:
        if line:
            fields = line.split("\t")
            rows[tuple(fields[:3])] = "\t".join(fields[3:])
    return rows


def table_hash(text):
    """The SHA-256 of the table's text, as the file beside the table records it."""
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def check_recorded_hash():
    """Whether the committed table is the one this script wrote, by its recorded SHA-256.

    This needs no ini file, so it also runs where the reference cache is absent.
    """
    try:
        with open(TABLE_PATH, "r", encoding="utf-8", newline="") as handle:
            committed = handle.read()
        with open(TABLE_HASH_PATH, "r", encoding="ascii") as handle:
            recorded = handle.read().strip()
    except OSError as error:
        print("ra2_reference: cannot read the committed table or its hash: %s" % error)
        return False
    if table_hash(committed) != recorded:
        print("ra2_reference: the committed table does not have the SHA-256 recorded in %s:"
              " it was edited by hand, or written without its hash;"
              " run `python3 scripts/ra2_reference.py` and commit the result"
              % os.path.relpath(TABLE_HASH_PATH, ROOT))
        return False
    return True


def check(fresh):
    """Compare the committed table with a fresh extraction; returns the exit code."""
    try:
        with open(TABLE_PATH, "r", encoding="utf-8", newline="") as handle:
            committed = handle.read()
    except OSError as error:
        print("ra2_reference: cannot read the committed table: %s" % error)
        return 1
    if committed == fresh:
        print("ra2_reference: the committed table matches the reference (%d rows)"
              % (fresh.count("\n") - 1))
        return 0
    old, new = parse_table(committed), parse_table(fresh)
    removed = sorted(set(old) - set(new))
    added = sorted(set(new) - set(old))
    changed = sorted(key for key in set(old) & set(new) if old[key] != new[key])
    print("ra2_reference: the committed table differs from a fresh extraction:"
          " %d rows to add, %d to remove, %d with another value%s"
          % (len(added), len(removed), len(changed),
             "" if added or removed or changed else " (only order or layout differ)"))
    for label, keys in (("add", added), ("remove", removed), ("change", changed)):
        for file, section, key in keys[:10]:
            print("  %-6s %s [%s] %s" % (label, file, section, key))
        if len(keys) > 10:
            print("  %-6s ... and %d more" % (label, len(keys) - 10))
    print("run `python3 scripts/ra2_reference.py` and commit the result")
    return 1


def stats(table, rules):
    """Print counts per category and the references that lead nowhere."""
    sections = dict.fromkeys(CATEGORIES, 0)
    rows = dict.fromkeys(CATEGORIES, 0)
    for category in table.categories.values():
        sections[category] += 1
    for file, section, _ in table.rows:
        rows[table.categories[(file, section.lower())]] += 1
    print("%-12s %8s %6s" % ("category", "sections", "rows"))
    for category in CATEGORIES:
        print("%-12s %8d %6d" % (category, sections[category], rows[category]))
    print("%-12s %8d %6d" % ("total", len(table.categories), len(table.rows)))
    print("keys left out: %d with an empty value, %d of whole sections by value shape"
          % (table.blank, table.shaped))
    missing = dangling(table, rules)
    print("dangling references: %d" % len(missing))
    for wanted, section, key, target, reason in missing:
        print("  [%s] %s=%s (%s): %s" % (section, key, target, wanted, reason))



RA9_DISPOSITION_HEADER = (
    "file", "section", "key", "portion", "exact_value", "classification",
    "route", "specific_semantic_reason", "code_evidence",
)


def _ra9_text(root, relative):
    with open(os.path.join(root, relative), encoding="utf-8", newline="") as handle:
        return handle.read()


def _ra9_records(root, name, required):
    """Strict installed TSV input, retaining exact contract reason/value bytes."""
    path = "crates/sim/tests/ra9_audit/" + name
    lines = _ra9_text(root, path).splitlines()
    if not lines:
        raise ExtractionError("RA9 empty contract: " + name)
    header = lines[0].split("\t")
    if len(set(header)) != len(header) or not set(required) <= set(header):
        raise ExtractionError("RA9 missing/duplicate contract columns: " + name)
    rows = []
    for number, line in enumerate(lines[1:], 2):
        fields = line.split("\t")
        if len(fields) != len(header):
            raise ExtractionError("RA9 contract row width: %s:%d" % (name, number))
        rows.append(dict(zip(header, fields)))
    return rows


def _ra9_one(pattern, text, label):
    matches = re.findall(pattern, text, re.DOTALL)
    if len(matches) != 1:
        raise ExtractionError("RA9 ambiguous/changed installed interface: " + label)
    return matches[0]


def _ra9_rust(root, relative):
    """A lexical formatting view: preserve every literal token byte, normalize only
    separators between Rust tokens. Reuse the real scope producer's conservative scanner;
    comments/formatting are not contracts, while literal whitespace remains exact.
    """
    import ra9_scope
    parts = ra9_scope.tokens(_ra9_text(root, relative))
    out = []
    previous = ""
    word = re.compile(r"^[A-Za-z_0-9]+$")
    for token in parts:
        if word.fullmatch(previous) and word.fullmatch(token):
            out.append(" ")
        out.append(token)
        previous = token
    return "".join(out)


def _ra9_fnv(text):
    value = 0xcbf29ce484222325
    for byte in text.encode("utf-8"):
        value = ((value ^ byte) * 0x100000001b3) & 0xffffffffffffffff
    return "%016x" % value


def ra9_dispositions(root, reference):
    """Compose proposals only; Rust still declares/admit/finishes every real source gate.

    Counts and typed routes come from the installed Rust compositor and exact family/class
    contracts. This is a bounded parser of those interfaces, not a source-policy evaluator.
    A changed interface fails closed and needs an intentional generator update.
    """
    audit = "crates/sim/tests/ra9_audit/"
    routes = _ra9_rust(root, audit + "source-routes.rs")
    plans = _ra9_rust(root, audit + "source-plans.rs")
    route_names = dict((variant, literal) for literal, variant in re.findall(
        r'"([^"]+)"=>Ok\(Self::(\w+)\)', routes.split("impl Route", 1)[1].split(
            "#[derive(Clone)]", 1)[0]))
    class_names = dict((variant, literal) for literal, variant in re.findall(
        r'"([^"]+)"=>Ok\(Self::(\w+)\)', routes.split("impl Class", 1)[1].split(
            "#[derive(Clone,Copy", 1)[0]))
    if len(route_names) != len(set(route_names.values())) or set(class_names.values()) != {
        "ORIGINAL_ENGINE_UNUSED", "CURRENT_MODEL_NONREAD", "INTENTIONAL_DIFFERENCE"
    }:
        raise ExtractionError("RA9 changed/ambiguous typed route/class parser")
    census, original_count = _ra9_one(
        r'if counts!=\[(.*?)\]\.into_iter\(\)\.collect\(\)\|\|master.len\(\)!=(\d+)',
        routes, "original master census")
    expected_master = dict((name, int(count)) for name, count in re.findall(
        r'\("([^"]+)"\.into\(\),(\d+),?\)', census))
    master_rows = _ra9_records(root, "source-candidates.proposed.tsv",
                               ("file", "section", "key", "portion", "exact_value", "disposition",
                                "family_id", "primary_sha256"))
    identity = lambda row: (row["file"], row["section"], row["key"])
    master, actual_census = {}, {}
    for row in master_rows:
        key = identity(row)
        if key in master or row["portion"] != "whole" or row["disposition"] not in expected_master:
            raise ExtractionError("RA9 duplicate/foreign/non-whole original source: %r" % (key,))
        if (reference.get(key) != row["exact_value"]
                or row["primary_sha256"] != SHA256.get(row["file"])):
            raise ExtractionError("RA9 original master literal/provenance changed: %r" % (key,))
        master[key] = row
        actual_census[row["disposition"]] = actual_census.get(row["disposition"], 0) + 1
    if actual_census != expected_master or len(master) != int(original_count):
        raise ExtractionError("RA9 original master census changed")
    wanted, refinements = {}, set()

    def insert(row, route, classification, reason, evidence):
        key = identity(row)
        if (key in wanted or not reason.strip() or reason.startswith("PENDING")
                or not evidence.strip() or route not in route_names.values()
                or classification not in class_names.values()
                or classification == "ORIGINAL_ENGINE_UNUSED"):
            raise ExtractionError("RA9 ambiguous/unspecific source proposal: %r" % (key,))
        if row.get("portion", "whole") != "whole" or reference.get(key) != row["exact_value"]:
            raise ExtractionError("RA9 original/extracted value or whole shape changed: %r" % (key,))
        wanted[key] = (*key, "whole", row["exact_value"], classification, route, reason, evidence)

    # These three append_table calls are independently counted by source_routes::declare.
    append_specs = re.findall(
        r'append_table\(&mut wanted,include_str!\("([^"]+)"\),Route::(\w+),'
        r'Class::(\w+),"([^"]+)","([^"]+)",(\d+),?\)', routes)
    if len(append_specs) != 3:
        raise ExtractionError("RA9 installed append-table routes changed")
    for name, variant, cls, value, reason, count in append_specs:
        rows = _ra9_records(root, name, ("file", "section", "key", value, reason))
        if len(rows) != int(count):
            raise ExtractionError("RA9 exact route census changed: " + name)
        for line, row in enumerate(rows, 2):
            row = dict(row, exact_value=row[value])
            evidence = row.get("code_evidence", row.get("actual_code_evidence", ""))
            insert(row, route_names[variant], class_names[cls], row[reason],
                   audit + name + "#L%d; proposal only; " % line + (evidence or "guarded admission required"))
            if variant not in ("Static116", "Masked19"):
                refinements.add(identity(row))
        if variant == "Static116" and {identity(row) for row in rows} != {
            key for key, row in master.items() if row["disposition"] == "PROPOSED_STATIC_UNUSED"
        }:
            raise ExtractionError("RA9 static route does not own exact old static atoms")

    # Residual classifications are per-contract, not flattened into a module default.
    class_count, class_body = _ra9_one(
        r"fn residual_classes\(\)->\[\(&'static str,Class,usize\);(\d+)\]\{\[(.*?)\]\}",
        routes, "residual typed classes")
    class_specs = re.findall(r'\("([^"]+)",Class::(\w+),(\d+),?\)', class_body)
    residual_classes = {}
    for label, variant, count in class_specs:
        if label in residual_classes or variant not in class_names:
            raise ExtractionError("RA9 duplicate/unknown residual typed class")
        residual_classes[label] = (class_names[variant], int(count))
    if len(residual_classes) != int(class_count):
        raise ExtractionError("RA9 residual class table shape changed")
    residual_name = _ra9_one(
        r'fn append_residual.*?let rows=table\(include_str!\("([^"]+)"\)',
        routes, "residual table")
    residual_count = int(_ra9_one(r'if keys.len\(\)!=(\d+)\|\|counts!=classes',
                                 routes, "residual class census"))
    if not re.search(r"refinements\.extend\(append_residual\(&mut wanted,?\)\?,?\)", routes):
        raise ExtractionError("RA9 residual owner route changed")
    residual = _ra9_records(root, residual_name, ("file", "section", "key", "exact_value",
                                                "proposed_classification", "specific_proposed_reason"))
    seen_residual = {}
    for line, row in enumerate(residual, 2):
        label = row["proposed_classification"]
        if label not in residual_classes:
            raise ExtractionError("RA9 unknown exact residual classification: " + label)
        seen_residual[label] = seen_residual.get(label, 0) + 1
        insert(row, route_names["Residual117"], residual_classes[label][0],
               row["specific_proposed_reason"], audit + residual_name + "#L%d; proposal only; " % line
               + row.get("actual_code_evidence", "guarded admission required"))
        refinements.add(identity(row))
    if (len(residual) != residual_count
            or seen_residual != {label: count for label, (_, count) in residual_classes.items()}):
        raise ExtractionError("RA9 exact residual class census changed")

    mappings = _ra9_records(root, "legacy904-class-contracts.tsv",
                           ("module", "actual_contract_classification", "exact_atom_count", "typed_class"))
    mapped = {}
    for row in mappings:
        key = (row["module"], row["actual_contract_classification"])
        if key in mapped or row["typed_class"] not in class_names.values():
            raise ExtractionError("RA9 duplicate/foreign exact class mapping")
        mapped[key] = row
    used_classes = set()
    package_specs = re.findall(
        r'\(Route::(\w+),"([^"]+)",include_str!\("([^"]+)"\),'
        r'\w+::demanded_sources\(&p\.\w+,?\),(\d+),?\)', plans)
    if len(package_specs) != 4:
        raise ExtractionError("RA9 installed four exact class packages changed")
    for variant, module, name, count in package_specs:
        rows = _ra9_records(root, name, ("file", "section", "key", "exact_value",
                                       "classification", "specific_reason"))
        if len(rows) != int(count):
            raise ExtractionError("RA9 exact package census changed: " + module)
        seen_classes = {}
        for line, row in enumerate(rows, 2):
            class_key = (module, row["classification"])
            if class_key not in mapped:
                raise ExtractionError("RA9 uncontracted exact class: %r" % (class_key,))
            cls = mapped[class_key]["typed_class"]
            seen_classes[class_key] = seen_classes.get(class_key, 0) + 1
            insert(row, route_names[variant], cls, row["specific_reason"],
                   audit + name + "#L%d; proposal only; " % line + row.get("model_guard_id", ""))
            refinements.add(identity(row))
        for class_key, count in seen_classes.items():
            if count != int(mapped[class_key]["exact_atom_count"]):
                raise ExtractionError("RA9 exact per-class census changed: %r" % (class_key,))
        used_classes.update(seen_classes)
    if used_classes != set(mapped):
        raise ExtractionError("RA9 missing/extra exact module class mappings")
    carry_count = int(_ra9_one(r'carry38_admission::demanded_sources\(&p.c38,?\);'
                              r'if rows.len\(\)!=(\d+)', plans, "carry38 census"))
    if "Route::Carry38,Class::IntentionalDifference" not in plans:
        raise ExtractionError("RA9 carry38 typed route changed")
    carry = _ra9_records(root, "carry38-contracts.tsv",
                         ("file", "section", "key", "exact_value", "specific_reason", "model_guard_id"))
    if len(carry) != carry_count:
        raise ExtractionError("RA9 carry38 exact census changed")
    for line, row in enumerate(carry, 2):
        insert(row, route_names["Carry38"], class_names["IntentionalDifference"], row["specific_reason"],
               audit + "carry38-contracts.tsv#L%d; proposal only; " % line + row["model_guard_id"])
        refinements.add(identity(row))
    if refinements != {key for key, row in master.items()
                       if row["disposition"] == "PENDING_SEMANTICS_OR_NATIVE"}:
        raise ExtractionError("RA9 refinements do not partition exact original pending atoms")

    # Legacy policies build reasons from original atoms plus independently owned families.
    families = _ra9_records(root, "legacy904-family-contracts.tsv",
                            ("route", "family_id", "atom_count", "profile", "typed_class",
                             "specific_current_model_reason", "actual_paths", "independent_atom_fnv1a64"))
    legacy_seen = set()
    for route, disposition, filename in (
        ("legacy-ai361", "PROPOSED_REPLACED_AI_POLICY", "legacy-ai-policy.rs"),
        ("legacy-model543", "PROPOSED_SPECIFIC_MODEL_BOUNDARY", "legacy-model-policy.rs"),
    ):
        policy = _ra9_rust(root, audit + filename)
        family_count, atom_count = map(int, _ra9_one(
            r'if out.len\(\)!=(\d+)\|\|out.values\(\)\.map\(\|f\|f.count\)'
            r'\.sum::<usize>\(\)!=(\d+)', policy, route + " family census"))
        family_rows = [f for f in families if f["route"] == route]
        if len(family_rows) != family_count or sum(int(f["atom_count"]) for f in family_rows) != atom_count:
            raise ExtractionError("RA9 exact legacy family census changed: " + route)
        # Parse precisely the installed mode's five return labels; fail on altered branching.
        mode = _ra9_one(r'fn mode\(c:&Contract\)->&\x27static str\{(.*?)\}fn catalogue_from',
                        policy, route + " mode")
        pattern = (
            r'if matches!\(c.value.to_ascii_lowercase\(\).as_str\(\),"no"\|"false"\|"0"\|"0.0"\)'
            r'\{"([^"]+)"\}else if c.family.profile=="default-death"\{"([^"]+)"\}'
            r'else if matches!\(c.family.profile.as_str\(\),'
            r'"carrier-collision"\|"virtual-scanner"\|"nuke-up-carrier"\|"fire-helpers"\|"shrapnel"\)'
            r'\{"([^"]+)"\}else if c.family.profile=="campaign-economy"\{"([^"]+)"\}'
            r'else\{"([^"]+)"\}'
        )
        match = re.fullmatch(pattern, mode)
        reason_constructor = re.search(
            r'format!\("\[\{\}\] \{\}=\{\}; \{\}; \{\}",key\.section,key\.key,'
            r'c\.value,c\.family\.reason,mode\(&c,?\),?\)', policy)
        if match is None or reason_constructor is None:
            raise ExtractionError("RA9 legacy reason/mode constructor changed: " + route)
        labels = match.groups()
        seen_families = set()
        for family in family_rows:
            fid = family["family_id"]
            if fid in seen_families:
                raise ExtractionError("RA9 duplicate legacy family: " + fid)
            seen_families.add(fid)
            atoms = [row for row in master.values() if row["family_id"] == fid
                     and row["disposition"] == disposition]
            atom_text = "".join("\t".join((*identity(row), "whole", row["exact_value"])) + "\n"
                                for row in sorted(atoms, key=identity))
            if (len(atoms) != int(family["atom_count"])
                    or _ra9_fnv(atom_text) != family["independent_atom_fnv1a64"]):
                raise ExtractionError("RA9 exact legacy family atom/value membership changed: " + fid)
            for row in atoms:
                if row["file"] != "rules" or row["primary_sha256"] != SHA256["rules"]:
                    raise ExtractionError("RA9 foreign legacy original source")
                profile = family["profile"]
                index = (0 if row["exact_value"].lower() in ("no", "false", "0", "0.0") else
                         1 if profile == "default-death" else
                         2 if profile in ("carrier-collision", "virtual-scanner", "nuke-up-carrier",
                                          "fire-helpers", "shrapnel") else
                         3 if profile == "campaign-economy" else 4)
                reason = "[%s] %s=%s; %s; %s" % (
                    row["section"], row["key"], row["exact_value"],
                    family["specific_current_model_reason"], labels[index])
                insert(row, route, family["typed_class"], reason,
                       audit + filename + "::catalogue_from; family=" + fid +
                       "; proposal only; " + family["actual_paths"])
                legacy_seen.add(identity(row))
    if legacy_seen != {key for key, row in master.items() if row["disposition"] in (
        "PROPOSED_REPLACED_AI_POLICY", "PROPOSED_SPECIFIC_MODEL_BOUNDARY"
    )} or any(f["route"] not in ("legacy-ai361", "legacy-model543") for f in families):
        raise ExtractionError("RA9 legacy refinements do not partition old904 atoms")

    special_code = _ra9_rust(root, audit + "special-source-admission.rs")
    special_count = int(_ra9_one(r'if v.len\(\)!=(\d+)', special_code, "special metadata census"))
    special = _ra9_records(root, "special-source-contracts.tsv",
                          ("file", "section", "key", "exact_value", "route",
                           "classification", "specific_semantic_reason"))
    if len(special) != special_count:
        raise ExtractionError("RA9 special metadata census changed")
    for line, row in enumerate(special, 2):
        if row["route"] not in ("vhp", "tree", "mirage-elite", "spark"):
            raise ExtractionError("RA9 foreign special source route")
        insert(row, row["route"], row["classification"], row["specific_semantic_reason"],
               audit + "special-source-contracts.tsv#L%d; proposal only; actual guarded source routes required" % line)
    total = int(_ra9_one(r'if wanted.len\(\)!=(\d+)', routes, "composed source census"))
    if len(wanted) != total or not set(master) <= set(wanted):
        raise ExtractionError("RA9 composed exact source census changed")
    return "\t".join(RA9_DISPOSITION_HEADER) + "\n" + "".join(
        "\t".join(wanted[key]) + "\n" for key in sorted(wanted))


def write_ra9_dispositions(text, check_only):
    """Check mode does not rewrite any table, acceptance, receipt or reference hash."""
    path = os.path.join(REFERENCE_DIR, "ra9_source_dispositions.tsv")
    if check_only:
        with open(path, encoding="utf-8", newline="") as handle:
            if handle.read() != text:
                print("ra2_reference: proposed RA9 dispositions differ; regenerate with --ra9-dispositions")
                return 1
        print("ra2_reference: proposed RA9 dispositions match (%d rows; no admission granted)"
              % (text.count("\n") - 1))
    else:
        with open(path, "w", encoding="utf-8", newline="\n") as handle:
            handle.write(text)
        print("ra2_reference: wrote %d proposed RA9 dispositions; all acceptance/receipt gates remain required"
              % (text.count("\n") - 1))
    return 0


def main(argv):
    dispositions = len(argv) >= 2 and argv[1] == "--ra9-dispositions"
    disposition_check = dispositions and argv[2:] == ["--check"]
    mode = "--ra9-dispositions" if dispositions else argv[1] if len(argv) == 2 else None
    if ((dispositions and argv[2:] not in ([], ["--check"]))
            or (not dispositions and (len(argv) > 2 or mode not in
                                      (None, "--check", "--stats", "--ra9-scope")))):
        sys.stderr.write("usage: ra2_reference.py [--check | --stats | --ra9-scope | "
                         "--ra9-dispositions [--check]]\n")
        return 2
    if mode == "--check" and not check_recorded_hash():
        return 1
    if mode == "--check" and not os.path.isdir(ra2_ini.reference_dir()):
        print("ra2_reference: no reference cache at %s (it is not part of the repository);"
              " the committed table has its recorded SHA-256, the comparison with the ini"
              " files is skipped" % ra2_ini.reference_dir())
        return 0
    try:
        rules, art = load_reference()
        if mode == "--ra9-scope":
            # ra9_scope observes the actual complete original and actual Rust source files.
            # It never reads expected tables or approval/native receipts as observations.
            import ra9_scope
            outputs = ra9_scope.produce(ROOT, rules, SHA256["rules"])
            for name, text in sorted(outputs.items()):
                with open(os.path.join(os.path.dirname(TABLE_PATH), name), "w",
                          encoding="utf-8", newline="\n") as handle:
                    handle.write(text)
            return 0
        mapping = load_mapping()
        check_mapping(rules, mapping)
        table = extract(rules, art, mapping)
        if dispositions:
            if not check_recorded_hash():
                return 1
            with open(TABLE_PATH, encoding="utf-8", newline="") as handle:
                if handle.read() != table.render():
                    raise ExtractionError("RA9 needs the committed reference to match fresh original extraction")
            try:
                proposed = ra9_dispositions(ROOT, table.rows)
            except (ValueError, KeyError) as error:
                raise ExtractionError("RA9 malformed/changed installed contract: %s" % error) from error
            return write_ra9_dispositions(proposed, disposition_check)
    except (ExtractionError, OSError) as error:
        sys.stderr.write("ra2_reference: %s\n" % error)
        return 2
    if mode == "--check":
        return check(table.render())
    if mode == "--stats":
        stats(table, rules)
        return 0
    text = table.render()
    with open(TABLE_PATH, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(text)
    with open(TABLE_HASH_PATH, "w", encoding="ascii", newline="\n") as handle:
        handle.write(table_hash(text) + "\n")
    print("ra2_reference: wrote %d rows of %d sections to %s"
          % (len(table.rows), len(table.categories), os.path.relpath(TABLE_PATH, ROOT)))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
