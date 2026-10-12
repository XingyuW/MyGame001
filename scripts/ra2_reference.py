#!/usr/bin/env python3
# Extracts the committed reference table of the RA2 parity programme from the vanilla Red
# Alert 2 `rules.ini` and `art.ini` (docs/RA2_PARITY_PLAN.md section 3.1).
#
# Input, never committed: the two ini files in ~/Library/Caches/MyGame001/ra2-reference/ (or in
# the directory named by RA2_REFERENCE_DIR), read through scripts/ra2_ini.py.
# Input, committed: crates/sim/tests/reference/ra2_mapping.tsv (game id -> ini section).
# Output, committed: crates/sim/tests/reference/ra2_reference.tsv (file, section, key, value).
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
# the repository: no comments, no names, no interface strings, no image, voice, sound or
# animation names. Two rules enforce this:
#   * sections of units, buildings, weapons, warheads, projectiles, superweapons, countries,
#     land types and ore types are filtered by the explicit key lists below; a key that is
#     not listed is never written;
#   * the global sections of WHOLE_SECTIONS are taken whole, minus the keys listed with them,
#     and minus every key whose value is not a number, a flag or a list of identifiers.
# A value with a blank, a colon or any other character outside VALUE_SHAPE is never written.
#
# Usage (standard library only, Python 3):
#     python3 scripts/ra2_reference.py            regenerate the table
#     python3 scripts/ra2_reference.py --check    exit 1 if the committed table is stale;
#                                                 exit 0 with a note if the cache is absent
#     python3 scripts/ra2_reference.py --stats    counts per category, dangling references
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
        "DefaultSmallGreySmokeSystem", "DefaultSparkSystem", "DefaultLargeRedSmokeSystem",
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
    "AudioVisual": ("ConditionYellow", "ConditionRed"),
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
    "country", "land", "ore", "art",
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

    for section, excluded in sorted(WHOLE_SECTIONS.items()):
        table.add_whole("global", "rules", rules, section, excluded)
    for section, keys in sorted(PARTIAL_SECTIONS.items()):
        table.add_listed("global", "rules", rules, section, keys)
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


def main(argv):
    mode = argv[1] if len(argv) == 2 else None
    if len(argv) > 2 or mode not in (None, "--check", "--stats"):
        sys.stderr.write("usage: ra2_reference.py [--check | --stats]\n")
        return 2
    if mode == "--check" and not os.path.isdir(ra2_ini.reference_dir()):
        print("ra2_reference: no reference cache at %s (it is not part of the repository);"
              " check skipped" % ra2_ini.reference_dir())
        return 0
    try:
        rules, art = load_reference()
        mapping = load_mapping()
        check_mapping(rules, mapping)
        table = extract(rules, art, mapping)
    except (ExtractionError, OSError) as error:
        sys.stderr.write("ra2_reference: %s\n" % error)
        return 2
    if mode == "--check":
        return check(table.render())
    if mode == "--stats":
        stats(table, rules)
        return 0
    with open(TABLE_PATH, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(table.render())
    print("ra2_reference: wrote %d rows of %d sections to %s"
          % (len(table.rows), len(table.categories), os.path.relpath(TABLE_PATH, ROOT)))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
