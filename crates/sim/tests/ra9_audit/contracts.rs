//! Independent demanded-dependency contracts. No actual adapter result/deps are read here.
//! New/default/branch obligations are declared BEFORE any comparisons. Missing source defaults
//! create no invented source atom. This code is PENDING execution and exhaustive reconciliation.
use super::rules::MappingRow;
use super::{Atom, Audit, AuditResult, Portion, Reference, SourceKey, WitnessId};
use std::collections::BTreeSet;
type Keys = BTreeSet<Atom>;
fn key(r: &Reference, section: &str, name: &str) -> Keys {
    let key = SourceKey::new("rules", section, name);
    if r.rows.contains_key(&key) {
        BTreeSet::from([Atom {
            key,
            portion: Portion::Whole,
        }])
    } else {
        Keys::new()
    }
}
fn value<'a>(r: &'a Reference, section: &str, name: &str, default: &'a str) -> &'a str {
    r.rows
        .get(&SourceKey::new("rules", section, name))
        .map_or(default, String::as_str)
}
fn yes(r: &Reference, section: &str, name: &str, default: &str) -> bool {
    matches!(
        value(r, section, name, default)
            .to_ascii_lowercase()
            .as_str(),
        "yes" | "true"
    )
}
fn keys(r: &Reference, section: &str, names: &[&str]) -> Keys {
    names.iter().flat_map(|k| key(r, section, k)).collect()
}
fn registry(r: &Reference, section: &str, id: &str) -> AuditResult<Keys> {
    let found = r
        .rows
        .iter()
        .filter(|(k, v)| k.file == "rules" && k.section == section && v.as_str() == id)
        .collect::<Vec<_>>();
    if found.len() > 1 {
        return Err(format!("ambiguous registry {section}/{id}"));
    }
    Ok(found
        .into_iter()
        .map(|(key, _)| Atom {
            key: key.clone(),
            portion: Portion::Whole,
        })
        .collect())
}
fn union(mut a: Keys, b: Keys) -> Keys {
    a.extend(b);
    a
}
fn need(a: &mut Audit, consumer: &str, path: &str, deps: Keys) -> AuditResult<()> {
    let id = a.physical(WitnessId::new(consumer, path));
    // Only this independent contract catalogue can introduce a logical projection. Physical
    // fields have already been registered by raw item/record enumeration. No adapter registers.
    if !a.required.contains(&id) {
        a.require(id.clone())?;
    }
    for dep in deps {
        a.require_dependency(id.clone(), dep)?;
    }
    Ok(())
}
fn at(
    a: &mut Audit,
    r: &Reference,
    c: &str,
    path: &str,
    s: &str,
    names: &[&str],
) -> AuditResult<()> {
    need(a, c, path, keys(r, s, names))
}
fn factory(r: &Reference, s: &str) -> Keys {
    let mut v = key(r, s, "Factory");
    if value(r, s, "Factory", "") == "UnitType" {
        v.extend(key(r, s, "Naval"));
    }
    v
}
fn slot(r: &Reference, s: &str, names: &[&str]) -> Keys {
    let mut result = Keys::new();
    for name in names {
        result.extend(key(r, s, name));
        if value(r, s, name, "none") != "none" {
            break;
        }
    }
    result
}
pub(super) fn item(
    a: &mut Audit,
    r: &Reference,
    row: &MappingRow,
    rows: &[MappingRow],
) -> AuditResult<()> {
    let s = &row.section;
    let c = format!("{}@rules:{s}", row.game_id);
    for (path, k) in [
        ("economy.production.cost", "Cost"),
        ("combat.hit_points", "Strength"),
        ("combat.sight", "Sight"),
        ("arsenal.tech_level", "TechLevel"),
        ("arsenal.size", "Size"),
    ] {
        at(a, r, &c, path, s, &[k])?;
    }
    let mut build = union(key(r, s, "Cost"), key(r, "General", "BuildSpeed"));
    if row.building {
        build.extend(key(r, s, "Wall"));
        if yes(r, s, "Wall", "no") {
            build.extend(key(r, "General", "WallBuildSpeedCoefficient"));
        }
    }
    need(a, &c, "economy.production.build_time", build)?;
    for (path, k) in [
        ("combat.items.armour", "Armor"),
        ("combat.items.air_range_bonus", "AirRangeBonus"),
        ("combat.items.land_targeting", "LandTargeting"),
        ("combat.items.naval_targeting", "NavalTargeting"),
    ] {
        at(a, r, &c, path, s, &[k])?;
    }
    let p = slot(r, s, &["Primary", "Weapon1"]);
    let q = slot(r, s, &["Secondary"]);
    need(a, &c, "combat.items.weapons", union(p.clone(), q.clone()))?;
    let mut ep = slot(r, s, &["ElitePrimary", "EliteWeapon1"]);
    let mut eq = slot(r, s, &["EliteSecondary"]);
    if value(r, s, "ElitePrimary", "none") == "none"
        && value(r, s, "EliteWeapon1", "none") == "none"
    {
        ep.extend(p);
    }
    if value(r, s, "EliteSecondary", "none") == "none" {
        eq.extend(q);
    }
    need(a, &c, "combat.items.elite_weapons", union(ep, eq))?;
    if row.building {
        at(a, r, &c, "combat.items.wall", s, &["Wall"])?;
    } else {
        at(a, r, &c, "combat.items.underwater", s, &["Underwater"])?;
        at(a, r, &c, "combat.items.organic", s, &["Organic"])?;
    }
    for (field, k) in [
        ("needs_engineer", "NeedsEngineer"),
        ("capturable", "Capturable"),
        ("insignificant", "Insignificant"),
        ("immune", "Immune"),
        ("hospital", "Hospital"),
        ("bridge_repair_hut", "BridgeRepairHut"),
        ("occupier", "Occupier"),
        ("can_be_occupied", "CanBeOccupied"),
        ("crewed", "Crewed"),
        ("deployer", "Deployer"),
        ("deploy_fire", "DeployFire"),
        ("c4", "C4"),
        ("can_c4", "CanC4"),
        ("bombable", "Bombable"),
        ("spyable", "Spyable"),
        ("detect_disguise", "DetectDisguise"),
        ("immune_psionics", "ImmuneToPsionics"),
        ("immune_radiation", "ImmuneToRadiation"),
        ("teleporter", "Teleporter"),
        ("stolen_allied", "RequiresStolenAlliedTech"),
        ("stolen_soviet", "RequiresStolenSovietTech"),
        ("overpowerable", "Overpowerable"),
        ("explodes", "Explodes"),
        ("disguise_when_still", "DisguiseWhenStill"),
        ("max_occupants", "MaxNumberOccupants"),
        ("cash_startup", "ProduceCashStartup"),
        ("cash_amount", "ProduceCashAmount"),
        ("build_limit", "BuildLimit"),
        ("guard_range", "GuardRange"),
        ("cash_delay", "ProduceCashDelay"),
    ] {
        at(a, r, &c, &format!("arsenal.{field}"), s, &[k])?;
    }
    let mut sensors = key(r, s, "Sensors");
    if yes(r, s, "Sensors", "no") {
        sensors.extend(key(r, s, "SensorsSight"));
    }
    need(a, &c, "arsenal.sensors_sight", sensors)?;
    need(
        a,
        &c,
        "arsenal.domain",
        if row.building {
            Keys::new()
        } else {
            key(r, s, "MovementZone")
        },
    )?;
    need(
        a,
        &c,
        "arsenal.capacity",
        if row.building {
            Keys::new()
        } else {
            key(r, s, "Passengers")
        },
    )?;
    let foot = registry(r, "InfantryTypes", s)?;
    if row.building {
        for (field, k) in [
            ("base_normal", "BaseNormal"),
            ("radar", "Radar"),
            ("repairable", "Repairable"),
            ("powered", "Powered"),
            ("unit_repair", "UnitRepair"),
            ("adjacent", "Adjacent"),
            ("category", "BuildCat"),
        ] {
            at(a, r, &c, &format!("arsenal.{field}"), s, &[k])?;
        }
        at(a, r, &c, "arsenal.sellable", s, &["Wall", "Unsellable"])?;
        for field in [
            "infantry",
            "crusher",
            "crushable",
            "size",
            "trainable",
            "self_healing",
            "ammo",
            "airport_bound",
            "cloakable",
        ] {
            need(a, &c, &format!("arsenal.{field}.cross_kind"), Keys::new())?;
        }
    } else {
        need(a, &c, "arsenal.infantry", foot.clone())?;
        need(a, &c, "arsenal.category", foot.clone())?;
        for (field, k) in [
            ("crusher", "Crusher"),
            ("crushable", "Crushable"),
            ("trainable", "Trainable"),
            ("self_healing", "SelfHealing"),
            ("airport_bound", "AirportBound"),
            ("cloakable", "Cloakable"),
            ("veteran_abilities", "VeteranAbilities"),
            ("elite_abilities", "EliteAbilities"),
            ("ammo", "Ammo"),
        ] {
            let mut dependencies = key(r, s, k);
            if field == "crushable" && dependencies.is_empty() {
                dependencies.extend(foot.clone());
            }
            need(a, &c, &format!("arsenal.{field}"), dependencies)?;
        }
        let mut speed = key(r, s, "Locomotor");
        if value(r, s, "Locomotor", "").starts_with("{92612C46-") {
            speed.extend(key(r, s, "JumpjetSpeed"));
        } else {
            speed.extend(key(r, s, "Speed"));
            speed.extend(foot.clone());
        }
        need(a, &c, "units.units.speed", speed)?;
        at(a, r, &c, "economy.harvesters.membership", s, &["Harvester"])?;
        if yes(r, s, "Harvester", "no") {
            at(
                a,
                r,
                &c,
                "economy.harvesters.capacity",
                s,
                &["Storage", "Harvester"],
            )?;
            for f in ["harvest_rate", "harvest_interval"] {
                need(a, &c, &format!("economy.harvesters.{f}"), Keys::new())?;
            }
        }
    }
    let physical = if row.building { "buildings" } else { "units" };
    for path in [
        format!("economy.ron.{physical}.prerequisites"),
        "arsenal.ron.entries.prerequisite_groups".into(),
        "economy+arsenal.prerequisites".into(),
    ] {
        at(a, r, &c, &path, s, &["Prerequisite"])?;
    }
    for (field, k) in [
        ("required_countries", "RequiredHouses"),
        ("forbidden_countries", "ForbiddenHouses"),
    ] {
        at(a, r, &c, &format!("arsenal.{field}"), s, &[k])?;
    }
    let mut owners = key(r, s, "Owner");
    if !value(r, s, "Owner", "").is_empty() {
        owners.extend(keys(r, "Sides", &["GDI", "Nod"]));
    }
    need(a, &c, "arsenal.owner_sides", owners)?;
    need(a, &c, "arsenal.side", Keys::new())?;
    need(
        a,
        &c,
        "arsenal.factory",
        if row.building {
            factory(r, s)
        } else {
            Keys::new()
        },
    )?;
    let mut producer = Keys::new();
    for candidate in rows.iter().filter(|v| v.building) {
        producer.extend(factory(r, &candidate.section));
    }
    if !row.building {
        producer.extend(foot.clone());
        if foot.is_empty() {
            producer.extend(key(r, s, "Naval"));
            if !yes(r, s, "Naval", "no") {
                producer.extend(key(r, "General", "PadAircraft"));
            }
        }
    }
    need(a, &c, "economy.production.producer", producer)?;
    need(
        a,
        &c,
        "economy.production.lane",
        if row.building {
            key(r, s, "BuildCat")
        } else {
            Keys::new()
        },
    )?;
    let mut users = Keys::new();
    let mut uses = false;
    if row.building {
        for other in rows.iter().filter(|v| !v.building) {
            users.extend(key(r, &other.section, "AirportBound"));
            if yes(r, &other.section, "AirportBound", "no") {
                users.extend(key(r, &other.section, "Dock"));
                uses |= value(r, &other.section, "Dock", "")
                    .split(',')
                    .any(|v| v.trim() == s);
            }
        }
    }
    if uses {
        users.extend(key(r, s, "NumberOfDocks"));
    }
    need(a, &c, "arsenal.docks", users)?;
    let mut repair = key(r, s, "UnitRepair");
    if yes(r, s, "UnitRepair", "no") {
        repair.extend(key(r, s, "NumberOfDocks"));
    }
    need(a, &c, "arsenal.vehicle_docks", repair)?;
    if row.building {
        let image = value(r, s, "Image", s);
        let foundation = SourceKey::new("art", image, "Foundation");
        if !r.rows.contains_key(&foundation) {
            return Err(format!("missing art Foundation {s}/{image}"));
        }
        let mut dims = key(r, s, "Image");
        dims.insert(Atom {
            key: foundation,
            portion: Portion::Whole,
        });
        for field in ["width", "height"] {
            need(a, &c, &format!("economy.buildings.{field}"), dims.clone())?;
        }
        for field in ["power_produced", "power_consumed"] {
            at(
                a,
                r,
                &c,
                &format!("economy.buildings.{field}"),
                s,
                &["Power"],
            )?;
        }
        let refund = union(key(r, s, "Cost"), key(r, "General", "RefundPercent"));
        need(a, &c, "economy.buildings.sell_value", refund.clone())?;
        let soylent = key(r, s, "Soylent");
        let mut effective = soylent.clone();
        if value(r, s, "Soylent", "0") == "0" {
            effective.extend(refund);
        }
        need(a, &c, "economy.refunds.effective", effective)?;
        need(a, &c, "economy.refunds.membership", soylent.clone())?;
        if value(r, s, "Soylent", "0") != "0" {
            need(a, &c, "economy.refunds.credits", soylent)?;
        }
        at(a, r, &c, "economy.buildings.free_units", s, &["FreeUnit"])?;
        at(a, r, &c, "economy.refineries.membership", s, &["Refinery"])?;
        if yes(r, s, "Refinery", "no") {
            for (field, source) in [
                ("credits_per_ore", "Riparius"),
                ("credits_per_gem", "Cruentus"),
            ] {
                need(
                    a,
                    &c,
                    &format!("economy.refineries.{field}"),
                    union(key(r, source, "Value"), key(r, s, "Refinery")),
                )?;
            }
        }
    }
    let spawn = if row.building {
        Keys::new()
    } else {
        key(r, s, "Spawns")
    };
    need(a, &c, "arsenal.spawns", spawn.clone())?;
    for (field, k) in [
        ("spawns_number", "SpawnsNumber"),
        ("spawn_regen", "SpawnRegenRate"),
        ("spawn_reload", "SpawnReloadRate"),
    ] {
        let mut v = spawn.clone();
        if !row.building && value(r, s, "Spawns", "none") != "none" {
            v.extend(key(r, s, k));
        }
        need(a, &c, &format!("arsenal.{field}"), v)?;
    }
    let mut launcher = spawn.clone();
    if !row.building && value(r, s, "Spawns", "none") != "none" {
        for k in ["Secondary", "Primary"] {
            launcher.extend(key(r, s, k));
            let weapon = value(r, s, k, "none");
            if weapon != "none" {
                launcher.extend(key(r, weapon, "Spawner"));
                if yes(r, weapon, "Spawner", "no") {
                    break;
                }
            }
        }
    }
    need(a, &c, "arsenal.launcher", launcher)?;
    let mut missile = Keys::new();
    if !row.building {
        missile.extend(key(r, "General", "V3RocketType"));
        if value(r, "General", "V3RocketType", "") != s {
            missile.extend(key(r, "General", "DMislType"));
        }
    }
    need(a, &c, "arsenal.missile", missile)?;
    let mut death = key(r, s, "DeathWeapon");
    if value(r, s, "DeathWeapon", "none") == "none" {
        death.extend(key(r, s, "BalloonHover"));
        if yes(r, s, "BalloonHover", "no") {
            death.extend(key(r, s, "Primary"));
        }
    }
    need(a, &c, "arsenal.death_weapon", death)?;
    at(a, r, &c, "arsenal.gunner", s, &["Gunner"])?;
    at(a, r, &c, "arsenal.ifv_mode", s, &["IFVMode"])?;
    if yes(r, s, "Gunner", "no") {
        at(
            a,
            r,
            &c,
            "arsenal.ifv_weapons.len",
            s,
            &["WeaponCount", "Gunner"],
        )?;
        let mut all = keys(r, s, &["Gunner", "WeaponCount"]);
        for n in 1..=13 {
            all.extend(key(r, s, &format!("Weapon{n}")));
        }
        need(a, &c, "arsenal.ifv_weapons", all)?;
        for n in 0..13 {
            at(
                a,
                r,
                &c,
                &format!("arsenal.ifv_effective_elite[{n}]"),
                s,
                &[&format!("EliteWeapon{}", n + 1)],
            )?;
        }
        for name in [
            "Normal",
            "Repair",
            "MachineGun",
            "Flak",
            "Pistol",
            "Sniper",
            "Shock",
            "Explode",
            "BrainBlast",
            "RadCannon",
            "Chrono",
            "TerroristExplode",
            "Cow",
        ] {
            let selector = format!("{name}TurretWeapon");
            let n = value(r, s, &selector, "")
                .parse::<usize>()
                .map_err(|e| e.to_string())?;
            at(
                a,
                r,
                &c,
                &format!("arsenal.ifv_named_mode[{name}]"),
                s,
                &[&selector, &format!("Weapon{}", n + 1)],
            )?;
        }
    } else {
        at(a, r, &c, "arsenal.ifv_weapons", s, &["Gunner"])?;
    }
    if let Some(sw) = r.rows.get(&SourceKey::new("rules", s, "SuperWeapon")) {
        for (field, k) in [
            ("is_powered", "IsPowered"),
            ("show_timer", "ShowTimer"),
            ("cooldown", "RechargeTime"),
            ("role.superweapon_type", "Type"),
            ("role.major_superweapon", "DisableableFromShell"),
        ] {
            need(
                a,
                &c,
                &format!("arsenal.{field}"),
                union(key(r, s, "SuperWeapon"), key(r, sw, k)),
            )?;
        }
        need(
            a,
            &c,
            "arsenal.role.registered_superweapon",
            union(
                key(r, s, "SuperWeapon"),
                registry(r, "SuperWeaponTypes", sw)?,
            ),
        )?;
    }
    at(
        a,
        r,
        &c,
        "derived.arsenal.missile_spawn_marker",
        s,
        &["MissileSpawn"],
    )?;
    if r.rows
        .contains_key(&SourceKey::new("rules", s, "MovementRestrictedTo"))
    {
        if value(r, s, "MovementZone", "") == "Fly" {
            at(
                a,
                r,
                &c,
                "derived.arsenal.restricted_air_domain",
                s,
                &["MovementZone"],
            )?;
            for parent in rows
                .iter()
                .filter(|p| value(r, &p.section, "Spawns", "") == s)
            {
                need(
                    a,
                    &c,
                    &format!("derived.recovery_parent.{}.domain", parent.game_id),
                    union(
                        key(r, s, "MovementRestrictedTo"),
                        key(r, &parent.section, "Spawns"),
                    ),
                )?;
            }
        } else {
            at(
                a,
                r,
                &c,
                "derived.arsenal.restricted_terrain",
                s,
                &["MovementRestrictedTo", "MovementZone"],
            )?;
        }
    }
    if yes(r, s, "Explodes", "no")
        && ["DeathWeapon", "Primary", "Weapon1"]
            .iter()
            .all(|k| value(r, s, k, "none") == "none")
    {
        let fallback = value(r, "CombatDamage", "DeathWeapon", "");
        let mut v = keys(r, s, &["Explodes", "DeathWeapon", "Primary", "Weapon1"]);
        v.extend(key(r, "CombatDamage", "DeathWeapon"));
        v.extend(key(r, fallback, "Damage"));
        need(a, &c, "derived.runtime.death_fallback_damage", v)?;
    }
    Ok(())
}
pub(super) fn weapon(a: &mut Audit, r: &Reference, id: &str, s: &str) -> AuditResult<()> {
    let c = format!("weapon.{id}@rules:{s}");
    for (field, k) in [
        ("damage", "Damage"),
        ("range", "Range"),
        ("min_range", "MinimumRange"),
        ("reload", "ROF"),
        ("burst", "Burst"),
        ("rad_level", "RadLevel"),
        ("area_fire", "AreaFire"),
        ("suicide", "Suicide"),
        ("sonic", "IsSonic"),
        ("repair", "Damage"),
        ("warhead", "Warhead"),
    ] {
        at(a, r, &c, &format!("combat.weapons.{field}"), s, &[k])?;
    }
    let p = value(r, s, "Projectile", "");
    let link = key(r, s, "Projectile");
    for (field, ks) in [
        ("anti_air", vec!["AA"]),
        ("anti_ground", vec!["AG"]),
        ("naval_only", vec!["AA", "AG"]),
    ] {
        need(
            a,
            &c,
            &format!("combat.weapons.{field}"),
            union(link.clone(), keys(r, p, &ks)),
        )?;
    }
    let mut speed = link.clone();
    speed.extend(key(r, p, "Inviso"));
    // Independent demand follows the actual Inviso-first OR, without consulting the
    // successful expression or premarking masked LimboLaunch/nominal Arcing Speed.
    if !yes(r, p, "Inviso", "no") {
        speed.extend(key(r, s, "LimboLaunch"));
        if !yes(r, s, "LimboLaunch", "no") {
            speed.extend(key(r, p, "Arcing"));
            if yes(r, p, "Arcing", "no") {
                speed.extend(key(r, s, "Range"));
                speed.extend(key(r, "AudioVisual", "Gravity"));
            } else {
                speed.extend(key(r, s, "Speed"));
            }
        }
    }
    need(a, &c, "combat.weapons.projectile_speed", speed)?;
    let mut scatter = union(link, key(r, p, "Inaccurate"));
    if yes(r, p, "Inaccurate", "no") {
        scatter.extend(key(r, "CombatDamage", "BallisticScatter"));
        scatter.extend(key(r, p, "Inviso"));
    }
    need(a, &c, "combat.weapons.scatter", scatter)
}
pub(super) fn warhead(a: &mut Audit, r: &Reference, id: &str, s: &str) -> AuditResult<()> {
    let c = format!("warhead.{id}@rules:{s}");
    for (field, k) in [
        ("versus", "Verses"),
        ("cell_spread", "CellSpread"),
        ("percent_at_max", "PercentAtMax"),
        ("paralyzes", "Paralyzes"),
        ("wall", "Wall"),
        ("wood", "Wood"),
        ("temporal", "Temporal"),
        ("mind_control", "MindControl"),
        ("psychic_damage", "PsychicDamage"),
        ("radiation", "Radiation"),
        ("parasite", "Parasite"),
        ("ivan_bomb", "IvanBomb"),
        ("bomb_disarm", "BombDisarm"),
        ("electric_assault", "ElectricAssault"),
        ("makes_disguise", "MakesDisguise"),
        ("culling", "Culling"),
    ] {
        at(a, r, &c, &format!("combat.warheads.{field}"), s, &[k])?;
    }
    Ok(())
}
fn component(section: &str, key: &str, index: usize) -> Keys {
    BTreeSet::from([Atom {
        key: SourceKey::new("rules", section, key),
        portion: Portion::Component(index),
    }])
}
fn eligibility(r: &Reference, row: &MappingRow, side: &str) -> AuditResult<(bool, Keys)> {
    let s = &row.section;
    let mut d = key(r, s, "AllowedToStartInMultiplayer");
    if !yes(r, s, "AllowedToStartInMultiplayer", "yes") {
        return Ok((false, d));
    }
    d.extend(key(r, s, "TechLevel"));
    let tech = value(r, s, "TechLevel", "")
        .parse::<i128>()
        .map_err(|e| e.to_string())?;
    if tech < 0 {
        return Ok((false, d));
    }
    d.extend(key(r, "MultiplayerDialogSettings", "TechLevel"));
    let top = value(r, "MultiplayerDialogSettings", "TechLevel", "")
        .parse::<i128>()
        .map_err(|e| e.to_string())?;
    if tech > top {
        return Ok((false, d));
    }
    d.extend(key(r, s, "Owner"));
    let owner = value(r, s, "Owner", "");
    if owner.is_empty() {
        return Ok((false, d));
    }
    let native = if side == "allied" { "GDI" } else { "Nod" };
    d.extend(key(r, "Sides", native));
    let houses = value(r, "Sides", native, "").split(',').collect::<Vec<_>>();
    if !owner.split(',').all(|o| houses.contains(&o)) {
        return Ok((false, d));
    }
    for k in [
        "RequiredHouses",
        "ForbiddenHouses",
        "RequiresStolenAlliedTech",
        "RequiresStolenSovietTech",
    ] {
        let dep = key(r, s, k);
        if !dep.is_empty() {
            d.extend(dep);
            return Ok((false, d));
        }
    }
    d.extend(key(r, "General", "BaseUnit"));
    Ok((
        !value(r, "General", "BaseUnit", "")
            .split(',')
            .any(|b| b == s),
        d,
    ))
}
pub(super) fn globals(a: &mut Audit, r: &Reference, rows: &[MappingRow]) -> AuditResult<()> {
    for (group, field, section, k, _) in super::combat::SCALAR_ROWS {
        at(
            a,
            r,
            "rules",
            &format!("rules.{group}.{field}"),
            section,
            &[k],
        )?;
    }
    for (group, field) in [
        ("economy", "ore_max_bails"),
        ("economy", "ore_spread_above"),
        ("superweapons", "nuclear_flight"),
    ] {
        need(a, "rules", &format!("rules.{group}.{field}"), Keys::new())?;
    }
    for (id, s, index) in [
        ("easy", "Difficult", 2),
        ("medium", "Normal", 1),
        ("brutal", "Easy", 0),
    ] {
        let c = format!("difficulty.{id}");
        need(a, &c, "rules.difficulties.id", Keys::new())?;
        for (field, k) in [("rof_pm", "ROF"), ("build_time_pm", "BuildTime")] {
            at(a, r, &c, &format!("rules.difficulties.{field}"), s, &[k])?;
        }
        for (field, k) in [
            ("credits_bonus_percent", "MultiplayerAICM"),
            ("virtual_purifiers", "AIVirtualPurifiers"),
            ("team_delay", "TeamDelays"),
            ("allied_base_defences", "AlliedBaseDefenseCounts"),
            ("soviet_base_defences", "SovietBaseDefenseCounts"),
        ] {
            need(
                a,
                &c,
                &format!("rules.difficulties.{field}"),
                component("General", k, index),
            )?;
        }
        let mut aggregate = Keys::new();
        for (slot, k) in [
            "ConYard",
            "WarFactory",
            "Power",
            "Engineer",
            "Thief",
            "Harvester",
            "MCV",
            "APC",
            "BaseDefense",
            "Plug",
            "Helipad",
            "Temple",
        ]
        .iter()
        .enumerate()
        {
            let d = component("General", &format!("AIIonCannon{k}Value"), index);
            aggregate.extend(d.clone());
            need(
                a,
                &c,
                &format!("rules.difficulties.target_weights[{slot}]"),
                d,
            )?;
        }
        need(a, &c, "rules.difficulties.target_weights", aggregate)?;
    }
    for (index, (house, _)) in super::rules::HOUSES.iter().enumerate() {
        let registry = key(r, "Countries", &index.to_string());
        let c = format!("country[{index}]");
        need(a, &c, "rules.countries.id", registry.clone())?;
        need(
            a,
            &c,
            "rules.countries.side",
            union(registry.clone(), key(r, house, "Side")),
        )?;
        need(
            a,
            &c,
            "rules.countries.playable_membership",
            union(registry.clone(), key(r, house, "Multiplay")),
        )?;
        let c = format!("skirmish.faction[{index}]");
        need(a, &c, "skirmish.factions.id", registry.clone())?;
        need(
            a,
            &c,
            "skirmish.factions.side",
            union(registry, key(r, house, "Side")),
        )?;
        need(a, &c, "skirmish.factions.country", Keys::new())?;
    }
    for (native, id) in [("GDI", "allied"), ("Nod", "soviet")] {
        at(
            a,
            r,
            &format!("side.{id}"),
            "rules.countries.side_membership",
            "Sides",
            &[native],
        )?;
    }
    for (_, k, id) in super::rules::GROUPS {
        at(
            a,
            r,
            &format!("group.{id}"),
            "rules.prerequisite_groups.buildings",
            "General",
            &[k],
        )?;
    }
    let bases = value(r, "General", "BaseUnit", "")
        .split(',')
        .collect::<Vec<_>>();
    if bases.len() != 2 {
        return Err("two BaseUnit source identities".into());
    }
    for (index, (id, crew, divisor, disguise, para, count)) in [
        (
            "allied",
            "AlliedCrew",
            "AlliedSurvivorDivisor",
            "AlliedDisguise",
            "AllyParaDropInf",
            "AllyParaDropNum",
        ),
        (
            "soviet",
            "SovietCrew",
            "SovietSurvivorDivisor",
            "SovietDisguise",
            "SovParaDropInf",
            "SovParaDropNum",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let c = format!("side.{id}");
        let base = bases[index];
        let deploy = value(r, base, "DeploysInto", "");
        at(a, r, &c, "rules.sides.base_unit", "General", &["BaseUnit"])?;
        let d = union(key(r, "General", "BaseUnit"), key(r, base, "DeploysInto"));
        need(a, &c, "rules.sides.construction_yard", d.clone())?;
        need(
            a,
            &c,
            "rules.sides.reverse_deploy_base",
            union(d, key(r, deploy, "UndeploysInto")),
        )?;
        for (field, k) in [
            ("crew", crew),
            ("disguise", disguise),
            ("paradrop_infantry", para),
            ("survivor_divisor", divisor),
            ("paradrop_count", count),
        ] {
            at(a, r, &c, &format!("rules.sides.{field}"), "General", &[k])?;
        }
        let mut vehicles = Keys::new();
        let mut infantry = Keys::new();
        for row in rows.iter().filter(|v| !v.building && v.note != "twin") {
            let (eligible, deps) = eligibility(r, row, id)?;
            let foot = registry(r, "InfantryTypes", &row.section)?;
            let actual_side = if foot.is_empty() {
                &mut vehicles
            } else {
                &mut infantry
            };
            actual_side.extend(deps.clone());
            if eligible {
                actual_side.extend(foot.clone());
            }
            let d = if eligible { union(deps, foot) } else { deps };
            for label in ["vehicles", "infantry"] {
                need(
                    a,
                    &format!("start.{id}.{}@{}", row.game_id, row.section),
                    &format!("skirmish.sides.{label}.membership"),
                    d.clone(),
                )?;
            }
        }
        need(
            a,
            &format!("start.{id}"),
            "skirmish.sides.vehicles",
            vehicles,
        )?;
        need(
            a,
            &format!("start.{id}"),
            "skirmish.sides.infantry",
            infantry,
        )?;
    }
    at(
        a,
        r,
        "skirmish",
        "skirmish.starting_credits",
        "MultiplayerDialogSettings",
        &["Money"],
    )?;
    for (field, k) in [
        ("mcv_start", "Bases"),
        ("short_game", "ShortGame"),
        ("crates", "Crates"),
        ("mcv_repacks", "MCVRedeploys"),
        ("bridge_destruction", "BridgeDestruction"),
        ("fog_of_war", "FogOfWar"),
    ] {
        at(
            a,
            r,
            "match_options.default",
            &format!("match_options.{field}"),
            "MultiplayerDialogSettings",
            &[k],
        )?;
    }
    at(
        a,
        r,
        "match_options.default",
        "match_options.bridge_capability_default",
        "CombatDamage",
        &["DestroyableBridges"],
    )?;
    let mut crate_pool = Keys::new();
    for row in rows.iter().filter(|v| !v.building) {
        let d = key(r, &row.section, "CrateGoodie");
        crate_pool.extend(d.clone());
        need(
            a,
            &format!("crate_unit.{}@{}", row.game_id, row.section),
            "rules.crate_units.membership",
            d,
        )?;
    }
    need(
        a,
        "rules.crate_units",
        "rules.crate_units.ordered_ids",
        crate_pool,
    )?;
    for (index, field) in [(0, "ore_value"), (1, "gem_value")] {
        let t = value(r, "Tiberiums", &index.to_string(), "");
        need(
            a,
            "rules.economy",
            &format!("rules.economy.{field}.registry"),
            union(key(r, "Tiberiums", &index.to_string()), key(r, t, "Value")),
        )?;
    }
    at(
        a,
        r,
        "american_paradrop",
        "runtime.american_paradrop_infantry",
        "General",
        &["AmerParaDropInf"],
    )?;
    for (key, scale) in [
        ("Armor", true),
        ("Cloak", false),
        ("Darkness", false),
        ("Explosion", false),
        ("Firepower", true),
        ("Gas", false),
        ("HealBase", false),
        ("ICBM", false),
        ("Invulnerability", false),
        ("IonStorm", false),
        ("Money", true),
        ("Napalm", false),
        ("Pod", false),
        ("Reveal", false),
        ("Speed", true),
        ("Squad", false),
        ("Tiberium", false),
        ("Unit", false),
        ("Veteran", true),
    ] {
        let c = format!("powerup@rules:Powerups:{key}");
        need(
            a,
            &c,
            "rules.powerups.membership",
            component("Powerups", key, 0),
        )?;
        let raw = value(r, "Powerups", key, "");
        let share = raw
            .split(',')
            .next()
            .unwrap_or("")
            .parse::<i128>()
            .map_err(|e| e.to_string())?;
        if share > 0 {
            need(a, &c, "rules.powerups.share", component("Powerups", key, 0))?;
            need(
                a,
                &c,
                "rules.powerups.value",
                if scale {
                    component("Powerups", key, 3)
                } else {
                    Keys::new()
                },
            )?;
        }
    }
    for (id, damage, wh) in [
        (
            "nuke",
            Some(("NukePayload", "Damage")),
            ("NukePayload", "Warhead"),
        ),
        (
            "lightning",
            Some(("General", "LightningDamage")),
            ("General", "LightningWarhead"),
        ),
        (
            "ivan_bomb",
            Some(("CombatDamage", "IvanDamage")),
            ("CombatDamage", "IvanWarhead"),
        ),
        (
            "c4",
            Some(("CombatDamage", "MaxDamage")),
            ("CombatDamage", "C4Warhead"),
        ),
        ("radiation", None, ("Radiation", "RadSiteWarhead")),
        (
            "v3_rocket",
            Some(("General", "V3RocketDamage")),
            ("CombatDamage", "V3Warhead"),
        ),
        (
            "v3_rocket_elite",
            Some(("General", "V3RocketEliteDamage")),
            ("CombatDamage", "V3EliteWarhead"),
        ),
        (
            "dmisl",
            Some(("General", "DMislDamage")),
            ("CombatDamage", "DMislWarhead"),
        ),
        (
            "dmisl_elite",
            Some(("General", "DMislEliteDamage")),
            ("CombatDamage", "DMislEliteWarhead"),
        ),
    ] {
        let c = format!("effect.{id}");
        need(
            a,
            &c,
            "combat.effects.damage",
            damage.map_or_else(Keys::new, |(s, k)| key(r, s, k)),
        )?;
        need(a, &c, "combat.effects.warhead", key(r, wh.0, wh.1))?;
    }
    Ok(())
}
