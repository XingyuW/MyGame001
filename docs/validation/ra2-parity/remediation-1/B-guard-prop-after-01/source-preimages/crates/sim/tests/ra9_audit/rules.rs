//! Historical preparation note: UNVALIDATED cache-only adapter body draft; never compiled/run. Current public APIs checked.
//! Integrate aliases/visibility, independent required-field/dependency enumeration, source display
//! for existing difference identities, and wrapper scopes only after explicit canonical release.
use super::actual::{check_int, optional, rule};
use super::combat::{Conversion, converted};
use super::{Audit, AuditResult, Expected, Reference, SourceKey, WitnessId, sequence};
use sim::{Definitions, MatchOptions, ProductionItem, Side, SkirmishDefinitions};
use std::collections::{BTreeMap, BTreeSet};
pub(super) struct MappingRow {
    pub(super) game_id: String,
    pub(super) section: String,
    pub(super) building: bool,
    pub(super) note: String,
    pub(super) side: String,
}
pub(super) const HOUSES: [(&str, &str); 9] = [
    ("Americans", "america"),
    ("Alliance", "korea"),
    ("French", "france"),
    ("Germans", "germany"),
    ("British", "great_britain"),
    ("Africans", "libya"),
    ("Arabs", "iraq"),
    ("Confederation", "cuba"),
    ("Russians", "russia"),
];

/// The ini's side names with the game's sides.
pub(super) const SIDES: [(&str, Side); 2] = [("GDI", Side::Allied), ("Nod", Side::Soviet)];

/// The prerequisite group tokens of the ini (upper case) with their `[General]` key and the
/// game's group id.
pub(super) const GROUPS: [(&str, &str, &str); 6] = [
    ("POWER", "PrerequisitePower", "power"),
    ("BARRACKS", "PrerequisiteBarracks", "barracks"),
    ("FACTORY", "PrerequisiteFactory", "factory"),
    ("RADAR", "PrerequisiteRadar", "radar"),
    ("TECH", "PrerequisiteTech", "tech"),
    ("PROC", "PrerequisiteProc", "proc"),
];

pub(super) fn csv(raw: &str) -> AuditResult<Vec<String>> {
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    let parts = raw
        .split(',')
        .map(str::trim)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if parts.iter().any(String::is_empty) {
        return Err(format!("empty interior source list element {raw:?}"));
    }
    Ok(parts)
}
pub(super) fn unique_set(values: Vec<String>, name: &str) -> AuditResult<String> {
    let set = values.iter().cloned().collect::<BTreeSet<_>>();
    if set.len() != values.len() {
        return Err(format!("duplicate semantic set member {name}"));
    }
    Ok(set.into_iter().collect::<Vec<_>>().join(","))
}
#[allow(
    clippy::too_many_arguments,
    reason = "Independent actual, expected, difference and evidence inputs remain explicit at the audit boundary"
)]
pub(super) fn text(
    a: &mut Audit,
    consumer: &str,
    path: &str,
    id: &str,
    field: &str,
    actual: String,
    expected: Expected<String>,
    display: String,
) -> AuditResult<()> {
    a.check(
        WitnessId::new(consumer, path),
        id,
        field,
        actual,
        expected,
        display,
    )
}
pub(super) fn mapped(
    source: Expected<String>,
    ids: &BTreeMap<String, String>,
) -> AuditResult<Expected<String>> {
    source.try_map(|native| {
        ids.get(&native)
            .cloned()
            .ok_or_else(|| format!("unmapped source type {native}"))
    })
}
pub(super) fn unit_id(d: &Definitions, k: sim::UnitKind) -> AuditResult<String> {
    d.unit(k)
        .map(|v| v.id.clone())
        .ok_or_else(|| format!("invalid actual unit kind {k:?}"))
}
pub(super) fn building_id(d: &Definitions, k: sim::BuildingKind) -> AuditResult<String> {
    d.building(k)
        .map(|v| v.id.clone())
        .ok_or_else(|| format!("invalid actual building kind {k:?}"))
}
pub(super) fn weapon_id(d: &Definitions, k: sim::WeaponKind) -> AuditResult<String> {
    d.weapon(k)
        .map(|v| v.id.clone())
        .ok_or_else(|| format!("invalid actual weapon kind {k:?}"))
}
pub(super) fn source_side(value: Expected<String>) -> AuditResult<Expected<String>> {
    value.try_map(|v| match v.as_str() {
        "GDI" => Ok("allied".into()),
        "Nod" => Ok("soviet".into()),
        _ => Err(format!("unexpected mapped house Side {v}")),
    })
}
pub(super) fn declare_difficulty_shapes(r: &mut Reference) -> AuditResult<()> {
    for key in [
        "AIIonCannonConYardValue",
        "AIIonCannonWarFactoryValue",
        "AIIonCannonPowerValue",
        "AIIonCannonEngineerValue",
        "AIIonCannonThiefValue",
        "AIIonCannonHarvesterValue",
        "AIIonCannonMCVValue",
        "AIIonCannonAPCValue",
        "AIIonCannonBaseDefenseValue",
        "AIIonCannonPlugValue",
        "AIIonCannonHelipadValue",
        "AIIonCannonTempleValue",
        "MultiplayerAICM",
        "AIVirtualPurifiers",
        "TeamDelays",
        "AlliedBaseDefenseCounts",
        "SovietBaseDefenseCounts",
    ] {
        r.require_components(SourceKey::new("rules", "General", key), 3)?;
    }
    Ok(())
}
pub(super) fn difficulties(a: &mut Audit, r: &Reference, d: &Definitions) -> AuditResult<()> {
    let rules = d.rules().ok_or_else(|| "missing Rules".to_owned())?;
    let rows = [
        ("easy", "Difficult", 2usize),
        ("medium", "Normal", 1),
        ("brutal", "Easy", 0),
    ];
    if rules.difficulties.len() != rows.len() {
        return Err("exact3 actual difficulty slots".into());
    }
    // Both actual order and IDs are checked, unlike find-only coverage of three named entries.
    for (position, (id, section, index)) in rows.into_iter().enumerate() {
        let game = &rules.difficulties[position];
        if game.id != id {
            return Err(format!(
                "difficulty position{position} ID {} != {id}",
                game.id
            ));
        }
        let consumer = format!("difficulty.{id}");
        a.game_owned(WitnessId::new(&consumer,"rules.difficulties.id"),game.id.clone(),
            "Game labels easy/medium/brutal map to native human-facing Difficult/Normal/Easy respectively; plan4.15".into(),
            "rules.rs DifficultyRules; RA2_PARITY_PLAN4.15".into())?;
        if game.target_weights.len() != 12 {
            return Err("actual exact12 target classes".into());
        }
        for (slot, key) in [
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
            let key = format!("AIIonCannon{key}Value");
            let raw = r.component(SourceKey::new("rules", "General", &key), index)?;
            let display = raw.value.clone();
            check_int(
                a,
                &consumer,
                &format!("rules.difficulties.target_weights[{slot}]"),
                "rules",
                &format!("difficulty.{id}.target_weights[{slot}]"),
                i128::from(game.target_weights[slot]),
                converted(raw, Conversion::Int)?,
                display,
            )?;
        }
        let expected = super::sequence(
            [
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
            .into_iter()
            .map(|name| {
                let key = format!("AIIonCannon{name}Value");
                super::combat::converted(
                    r.component(SourceKey::new("rules", "General", &key), index)?,
                    Conversion::Int,
                )
            })
            .collect::<AuditResult<Vec<_>>>()?,
        )
        .map(|v| {
            v.into_iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        });
        text(
            a,
            &consumer,
            "rules.difficulties.target_weights",
            "rules",
            &format!("difficulty.{id}.target_weights"),
            game.target_weights
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(","),
            expected,
            "exact12 native target classes in canonical source order".into(),
        )?;
        for (field, actual, key, conv) in [
            ("rof_pm", game.rof_pm, "ROF", Conversion::Permille),
            (
                "build_time_pm",
                game.build_time_pm,
                "BuildTime",
                Conversion::Permille,
            ),
        ] {
            let raw = rule(r, section, key)?;
            let display = raw.value.clone();
            check_int(
                a,
                &consumer,
                &format!("rules.difficulties.{field}"),
                "rules",
                &format!("difficulty.{id}.{field}"),
                i128::from(actual),
                converted(raw, conv)?,
                display,
            )?;
        }
        for (field, actual, key, conv) in [
            (
                "credits_bonus_percent",
                game.credits_bonus_percent,
                "MultiplayerAICM",
                Conversion::Int,
            ),
            (
                "virtual_purifiers",
                game.virtual_purifiers,
                "AIVirtualPurifiers",
                Conversion::Int,
            ),
            (
                "team_delay",
                game.team_delay,
                "TeamDelays",
                Conversion::Frames,
            ),
            (
                "allied_base_defences",
                game.allied_base_defences,
                "AlliedBaseDefenseCounts",
                Conversion::Int,
            ),
            (
                "soviet_base_defences",
                game.soviet_base_defences,
                "SovietBaseDefenseCounts",
                Conversion::Int,
            ),
        ] {
            let raw = r.component(SourceKey::new("rules", "General", key), index)?;
            let display = raw.value.clone();
            check_int(
                a,
                &consumer,
                &format!("rules.difficulties.{field}"),
                "rules",
                &format!("difficulty.{id}.{field}"),
                i128::from(actual),
                converted(raw, conv)?,
                display,
            )?;
        }
    }
    Ok(())
}
pub(super) fn countries(a: &mut Audit, r: &Reference, d: &Definitions) -> AuditResult<()> {
    let rules = d.rules().ok_or_else(|| "missing Rules".to_owned())?;
    if rules.countries.len() != 9 {
        return Err("actual nine playable countries".into());
    }
    for (index, (house, id)) in HOUSES.into_iter().enumerate() {
        let game = &rules.countries[index];
        let consumer = format!("country[{index}]");
        let registry = rule(r, "Countries", &index.to_string())?;
        if registry.value != house {
            return Err(format!(
                "country source position{index} differs from declared semantic identity"
            ));
        }
        let expected = registry.clone().try_map(|name| {
            HOUSES
                .iter()
                .find(|(native, _)| *native == name)
                .map(|(_, id)| id.to_string())
                .ok_or_else(|| format!("unknown house {name}"))
        })?;
        text(
            a,
            &consumer,
            "rules.countries.id",
            "rules",
            &format!("countries.{index}.id"),
            game.id.clone(),
            expected,
            house.into(),
        )?;
        let expected = source_side(rule(r, house, "Side")?)?.because(&registry);
        text(
            a,
            &consumer,
            "rules.countries.side",
            "rules",
            &format!("countries.{index}.side"),
            game.side.id().into(),
            expected,
            house.into(),
        )?;
        // Multiplay=yes is witnessed by actual playable presence, not source-only truth.
        let raw = optional(r, house, "Multiplay", "yes")?;
        let display = raw.value.clone();
        check_int(
            a,
            &consumer,
            "rules.countries.playable_membership",
            "rules",
            &format!("countries.{index}.playable"),
            i128::from(rules.countries.iter().any(|c| c.id == id)),
            converted(raw, Conversion::Flag)?.because(&registry),
            display,
        )?;
    }
    // [Sides] lists are semantic membership sets, not the order of the Countries menu.
    for (native, side) in SIDES {
        let raw = rule(r, "Sides", native)?;
        let display = raw.value.clone();
        let expected = raw.try_map(|v| {
            csv(&v)?
                .into_iter()
                .map(|house| {
                    HOUSES
                        .iter()
                        .find(|(name, _)| *name == house)
                        .map(|(_, id)| id.to_string())
                        .ok_or_else(|| format!("unmapped side house {house}"))
                })
                .collect::<AuditResult<Vec<_>>>()
                .and_then(|v| unique_set(v, "source side houses"))
        })?;
        let actual = unique_set(
            rules
                .countries
                .iter()
                .filter(|c| c.side == side)
                .map(|c| c.id.clone())
                .collect(),
            "actual side countries",
        )?;
        text(
            a,
            &format!("side.{}", side.id()),
            "rules.countries.side_membership",
            "rules",
            &format!("sides.{}.countries", side.id()),
            actual,
            expected,
            display,
        )?;
    }
    Ok(())
}
pub(super) fn groups_and_sides(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    ids: &BTreeMap<String, String>,
) -> AuditResult<()> {
    let rules = d.rules().ok_or_else(|| "missing Rules".to_owned())?;
    if rules.prerequisite_groups.len() != GROUPS.len() {
        return Err("exact6 prerequisite groups".into());
    }
    for (_, key, id) in GROUPS {
        let group = rules
            .group_id(id)
            .and_then(|k| rules.group(k))
            .ok_or_else(|| format!("missing actual group {id}"))?;
        let raw = rule(r, "General", key)?;
        let display = raw.value.clone();
        let expected = raw.try_map(|v| {
            csv(&v)?
                .into_iter()
                .map(|v| {
                    ids.get(&v)
                        .cloned()
                        .ok_or_else(|| format!("unmapped prerequisite {v}"))
                })
                .collect::<AuditResult<Vec<_>>>()
                .and_then(|v| unique_set(v, "source group membership"))
        })?;
        let actual = unique_set(
            group
                .buildings
                .iter()
                .map(|&k| building_id(d, k))
                .collect::<AuditResult<Vec<_>>>()?,
            "actual group membership",
        )?;
        text(
            a,
            &format!("group.{id}"),
            "rules.prerequisite_groups.buildings",
            "rules",
            &format!("prerequisite_groups.{id}"),
            actual,
            expected,
            display,
        )?;
    }
    if rules.sides.len() != 2 {
        return Err("exact2 actual sides".into());
    }
    let bases = rule(r, "General", "BaseUnit")?.try_map(|v| {
        let v = csv(&v)?;
        if v.len() != 2 {
            return Err("exact2 BaseUnit entries".into());
        }
        Ok(v)
    })?;
    for (index, (side, crew, divisor, disguise, infantry, count)) in [
        (
            Side::Allied,
            "AlliedCrew",
            "AlliedSurvivorDivisor",
            "AlliedDisguise",
            "AllyParaDropInf",
            "AllyParaDropNum",
        ),
        (
            Side::Soviet,
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
        let game = rules
            .side(side)
            .ok_or_else(|| format!("missing actual side {side:?}"))?;
        let consumer = format!("side.{}", side.id());
        let base = bases.clone().map(|v| v[index].clone());
        let display = base.value.clone();
        text(
            a,
            &consumer,
            "rules.sides.base_unit",
            "rules",
            &format!("sides.{}.base_unit", side.id()),
            unit_id(d, game.base_unit)?,
            mapped(base.clone(), ids)?,
            display,
        )?;
        let deploy = rule(r, &base.value, "DeploysInto")?;
        let display = deploy.value.clone();
        text(
            a,
            &consumer,
            "rules.sides.construction_yard",
            "rules",
            &format!("sides.{}.construction_yard", side.id()),
            building_id(d, game.construction_yard)?,
            mapped(deploy.clone(), ids)?.because(&base),
            display,
        )?;
        // Reverse deployment is compared to ACTUAL base kind too; source-source equality alone
        // cannot cover the yard's UndeploysInto value.
        let reverse = rule(r, &deploy.value, "UndeploysInto")?;
        let display = reverse.value.clone();
        text(
            a,
            &consumer,
            "rules.sides.reverse_deploy_base",
            "rules",
            &format!("sides.{}.reverse_deploy", side.id()),
            unit_id(d, game.base_unit)?,
            mapped(reverse, ids)?.because(&deploy).because(&base),
            display,
        )?;
        for (field, actual, key) in [
            ("crew", unit_id(d, game.crew)?, crew),
            ("disguise", unit_id(d, game.disguise)?, disguise),
            (
                "paradrop_infantry",
                unit_id(d, game.paradrop_infantry)?,
                infantry,
            ),
        ] {
            let raw = rule(r, "General", key)?;
            let display = raw.value.clone();
            text(
                a,
                &consumer,
                &format!("rules.sides.{field}"),
                "rules",
                &format!("sides.{}.{field}", side.id()),
                actual,
                mapped(raw, ids)?,
                display,
            )?;
        }
        for (field, actual, key) in [
            ("survivor_divisor", game.survivor_divisor, divisor),
            ("paradrop_count", game.paradrop_count, count),
        ] {
            let raw = rule(r, "General", key)?;
            let display = raw.value.clone();
            check_int(
                a,
                &consumer,
                &format!("rules.sides.{field}"),
                "rules",
                &format!("sides.{}.{field}", side.id()),
                i128::from(actual),
                converted(raw, Conversion::Int)?,
                display,
            )?;
        }
    }
    Ok(())
}
pub(super) fn ifv(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    row: &MappingRow,
    weapon_ids: &BTreeMap<String, String>,
) -> AuditResult<()> {
    let item = if row.building {
        d.building_kind(&row.game_id).map(ProductionItem::Building)
    } else {
        d.unit_kind(&row.game_id).map(ProductionItem::Unit)
    }
    .ok_or_else(|| format!("missing mapped {}", row.game_id))?;
    let m = d
        .metadata(item)
        .ok_or_else(|| "missing metadata".to_owned())?;
    let consumer = format!("{}@rules:{}", row.game_id, row.section);
    let gunner = converted(optional(r, &row.section, "Gunner", "no")?, Conversion::Flag)?;
    let display = gunner.value.to_string();
    check_int(
        a,
        &consumer,
        "arsenal.gunner",
        &row.game_id,
        "gunner",
        i128::from(m.gunner),
        gunner.clone(),
        display,
    )?;
    let mode = optional(r, &row.section, "IFVMode", "0")?;
    let display = mode.value.clone();
    check_int(
        a,
        &consumer,
        "arsenal.ifv_mode",
        &row.game_id,
        "ifv_mode",
        i128::from(m.ifv_mode),
        converted(mode, Conversion::Int)?,
        display,
    )?;
    if gunner.value == 0 {
        if !m.ifv_weapons.is_empty() {
            return Err("nongunner has IFV modes".into());
        }
        text(
            a,
            &consumer,
            "arsenal.ifv_weapons",
            &row.game_id,
            "ifv_weapons",
            String::new(),
            Expected::literal(String::new()).because(&gunner),
            "no Gunner".into(),
        )?;
        return Ok(());
    }
    let count = converted(rule(r, &row.section, "WeaponCount")?, Conversion::Int)?;
    if count.value != 13 {
        return Err("source IFV exact13 modes".into());
    }
    check_int(
        a,
        &consumer,
        "arsenal.ifv_weapons.len",
        &row.game_id,
        "ifv_weapons.len",
        m.ifv_weapons.len() as i128,
        count.clone().because(&gunner),
        "WeaponCount".into(),
    )?;
    if m.ifv_weapons.len() != 13 {
        return Err("actual IFV exact13 modes".into());
    }
    let expected = sequence(
        (1..=13)
            .map(|n| mapped(rule(r, &row.section, &format!("Weapon{n}"))?, weapon_ids))
            .collect::<AuditResult<Vec<_>>>()?,
    )
    .map(|v| v.join(","))
    .because(&gunner)
    .because(&count);
    let actual = m
        .ifv_weapons
        .iter()
        .map(|&k| weapon_id(d, k))
        .collect::<AuditResult<Vec<_>>>()?
        .join(",");
    text(
        a,
        &consumer,
        "arsenal.ifv_weapons",
        &row.game_id,
        "ifv_weapons",
        actual,
        expected,
        "Weapon1..13".into(),
    )?;
    // Actual special/vehicles.rs uses elite primary ONLY for mode0; other IFV modes keep the
    // ordinary fixed mode weapon. Compare every EliteWeaponN to that effective actual view.
    let combat = match item {
        ProductionItem::Unit(k) => d.unit_combat(k),
        ProductionItem::Building(k) => d.building_combat(k),
    }
    .ok_or_else(|| "missing IFV combat".to_owned())?;
    for index in 0..13 {
        let expected = mapped(
            rule(r, &row.section, &format!("EliteWeapon{}", index + 1))?,
            weapon_ids,
        )?;
        let actual = if index == 0 {
            combat.weapons(true)[0].ok_or_else(|| "IFV elite primary absent".to_owned())?
        } else {
            m.ifv_weapons[index]
        };
        text(
            a,
            &consumer,
            &format!("arsenal.ifv_effective_elite[{index}]"),
            &row.game_id,
            &format!("ifv_effective_elite[{index}]"),
            weapon_id(d, actual)?,
            expected,
            "EliteWeapon1..13 (mode0 actual elite-primary override)".into(),
        )?;
    }
    // Fixed semantic mode slots come from plan AppendixD's mode table and actual passenger
    // IFVMode data/runtime indexing. The expected native selector chooses only the SOURCE
    // numbered weapon; it never chooses the actual index (which would be a tautology).
    for (slot, name) in [
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
    ]
    .iter()
    .enumerate()
    {
        let selector = converted(
            rule(r, &row.section, &format!("{name}TurretWeapon"))?,
            Conversion::Int,
        )?;
        let index = usize::try_from(selector.value).map_err(|_| "negative IFV selector")?;
        if index >= 13 {
            return Err("source named IFV selector outside13".into());
        }
        let expected = mapped(
            rule(r, &row.section, &format!("Weapon{}", index + 1))?,
            weapon_ids,
        )?
        .because(&selector);
        text(
            a,
            &consumer,
            &format!("arsenal.ifv_named_mode[{name}]"),
            &row.game_id,
            &format!("ifv_named_mode.{name}"),
            weapon_id(d, m.ifv_weapons[slot])?,
            expected,
            format!("{name}TurretWeapon"),
        )?;
    }
    // Cow slot is shipped data, but no mapped cow passenger exists. This proves stored mode12,
    // not actual mapped-passenger reachability; native gameplay regression belongs to S.
    Ok(())
}

// A positive registry membership carries only its matching row; never all other types' rows.
// Absent membership is an explicit literal false, not invented source-key coverage.
pub(super) fn registered(
    r: &Reference,
    registry: &str,
    section: &str,
) -> AuditResult<Expected<bool>> {
    let found = r
        .rows
        .iter()
        .filter(|(k, v)| k.file == "rules" && k.section == registry && v.as_str() == section)
        .collect::<Vec<_>>();
    if found.len() > 1 {
        return Err(format!("duplicate {registry} registry identity {section}"));
    }
    match found.first() {
        Some((key, _)) => Ok(r.expect((*key).clone())?.map(|_| true)),
        None => Ok(Expected::literal(false)),
    }
}
pub(super) fn starting_eligible(
    r: &Reference,
    row: &MappingRow,
    houses: &Expected<Vec<String>>,
    bases: &Expected<Vec<String>>,
    top: &Expected<i128>,
) -> AuditResult<Expected<bool>> {
    let allowed = converted(
        optional(r, &row.section, "AllowedToStartInMultiplayer", "yes")?,
        Conversion::Flag,
    )?;
    if allowed.value == 0 {
        return Ok(allowed.map(|_| false));
    }
    let tech = converted(rule(r, &row.section, "TechLevel")?, Conversion::Int)?;
    if tech.value < 0 {
        return Ok(tech.map(|_| false).because(&allowed));
    }
    let eligible = tech
        .clone()
        .zip(top.clone())
        .map(|(tech, top)| tech <= top)
        .because(&allowed);
    if !eligible.value {
        return Ok(eligible);
    }
    let owners = optional(r, &row.section, "Owner", "")?.try_map(|v| csv(&v))?;
    if owners.value.is_empty() {
        return Ok(owners.map(|_| false).because(&eligible));
    }
    let belongs = owners
        .zip(houses.clone())
        .map(|(owners, houses)| !owners.is_empty() && owners.iter().all(|v| houses.contains(v)))
        .because(&eligible);
    if !belongs.value {
        return Ok(belongs);
    }
    let mut eligible = belongs;
    for key in [
        "RequiredHouses",
        "ForbiddenHouses",
        "RequiresStolenAlliedTech",
        "RequiresStolenSovietTech",
    ] {
        // Eligibility excludes the existence of these keys, regardless of their raw contents.
        // The contents are compared separately to actual metadata country/stolen-tech fields.
        let k = SourceKey::new("rules", &row.section, key);
        if r.rows.contains_key(&k) {
            return Ok(r.expect(k)?.map(|_| false).because(&eligible));
        }
    }
    eligible = bases
        .clone()
        .map(|v| !v.contains(&row.section))
        .because(&eligible);
    Ok(eligible)
}
pub(super) fn lobby(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    skirmish: &SkirmishDefinitions,
    rows: &[MappingRow],
) -> AuditResult<()> {
    d.rules().ok_or_else(|| "missing Rules".to_owned())?;
    let credits = rule(r, "MultiplayerDialogSettings", "Money")?;
    let display = credits.value.clone();
    check_int(
        a,
        "skirmish",
        "skirmish.starting_credits",
        "skirmish",
        "starting_credits",
        i128::from(skirmish.starting_credits),
        converted(credits, Conversion::Int)?,
        display,
    )?;
    if skirmish.factions.len() != HOUSES.len() {
        return Err("exact9 actual skirmish factions".into());
    }
    for (index, (_, id)) in HOUSES.into_iter().enumerate() {
        let f = &skirmish.factions[index];
        let consumer = format!("skirmish.faction[{index}]");
        let registry = rule(r, "Countries", &index.to_string())?;
        let house = registry.value.clone();
        let expected = registry.clone().try_map(|native| {
            HOUSES
                .iter()
                .find(|(v, _)| *v == native)
                .map(|(_, id)| id.to_string())
                .ok_or_else(|| format!("unmapped house {native}"))
        })?;
        text(
            a,
            &consumer,
            "skirmish.factions.id",
            "skirmish",
            &format!("factions[{index}].id"),
            f.id.clone(),
            expected,
            house.clone(),
        )?;
        text(
            a,
            &consumer,
            "skirmish.factions.side",
            "skirmish",
            &format!("factions[{index}].side"),
            f.side.map_or("none", |v| v.id()).into(),
            source_side(rule(r, &house, "Side")?)?.because(&registry),
            house.clone(),
        )?;
        if f.country.map(|c| usize::from(c.0)) != Some(index) {
            return Err(format!("actual country ordinal {id}"));
        }
        a.game_owned(WitnessId::new(&consumer,"skirmish.factions.country"),index.to_string(),
            "CountryId is index of independently checked Countries menu; actual source identities/order checked separately".into(),
            "skirmish_defs.rs FactionDef.country; rules CountryId".into())?;
    }
    if skirmish.sides.len() != 2 {
        return Err("exact2 skirmish starting pools".into());
    }
    let top = converted(
        rule(r, "MultiplayerDialogSettings", "TechLevel")?,
        Conversion::Int,
    )?;
    let bases = rule(r, "General", "BaseUnit")?.try_map(|v| csv(&v))?;
    for (name, side) in SIDES {
        let houses = rule(r, "Sides", name)?.try_map(|v| csv(&v))?;
        let pool = skirmish
            .side(side)
            .ok_or_else(|| format!("missing side starting pool {side:?}"))?;
        let actual_vehicle = pool
            .vehicles
            .iter()
            .map(|&k| unit_id(d, k))
            .collect::<AuditResult<Vec<_>>>()?;
        let actual_infantry = pool
            .infantry
            .iter()
            .map(|&k| unit_id(d, k))
            .collect::<AuditResult<Vec<_>>>()?;
        let _ = unique_set(actual_vehicle.clone(), "actual starting vehicles")?;
        let _ = unique_set(actual_infantry.clone(), "actual starting infantry")?;
        let mut expected_vehicle = Vec::new();
        let mut expected_infantry = Vec::new();
        for row in rows {
            if row.building || row.note == "twin" {
                continue;
            }
            let eligibility = starting_eligible(r, row, &houses, &bases, &top)?;
            let infantry = registered(r, "InfantryTypes", &row.section)?;
            // Each item gets its own actual membership comparison; a skipped item leaves an
            // independently required witness. Aggregate array comparison also rejects extras.
            for (is_foot, label, actual_ids) in [
                (false, "vehicles", &actual_vehicle),
                (true, "infantry", &actual_infantry),
            ] {
                let expected = if eligibility.value {
                    infantry
                        .clone()
                        .map(|foot| i128::from(foot == is_foot))
                        .because(&eligibility)
                } else {
                    Expected::literal(0).because(&eligibility)
                };
                check_int(a,&format!("start.{}.{}@{}",side.id(),row.game_id,row.section),&format!("skirmish.sides.{label}.membership"),"skirmish",&format!("starting.{}.{label}.{}",side.id(),row.game_id),
                    i128::from(actual_ids.contains(&row.game_id)),expected,"Allowed;Tech;Owner;country restrictions;BaseUnit;InfantryTypes, active branches only".into())?;
            }
            let target = if infantry.value {
                &mut expected_infantry
            } else {
                &mut expected_vehicle
            };
            target.push(if eligibility.value {
                Expected::literal(Some(row.game_id.clone()))
                    .because(&eligibility)
                    .because(&infantry)
            } else {
                Expected::literal(None).because(&eligibility)
            });
        }
        for (label, actual, parts) in [
            ("vehicles", actual_vehicle, expected_vehicle),
            ("infantry", actual_infantry, expected_infantry),
        ] {
            // Order matters: starting_units cycles each pool. Preserve committed mapping-row
            // order for the expected eligible sequence, and require actual exact length/order.
            let expected = sequence(parts).try_map(|v| {
                let v = v.into_iter().flatten().collect::<Vec<_>>();
                let _ = unique_set(v.clone(), "expected starting pool")?;
                Ok(v.join(","))
            })?;
            text(
                a,
                &format!("start.{}", side.id()),
                &format!("skirmish.sides.{label}"),
                "skirmish",
                &format!("starting.{}.{label}", side.id()),
                actual.join(","),
                expected,
                "eligible mapped starting types, exact order".into(),
            )?;
        }
    }
    // 475 is independently recomputed by mean of the actual, audited pool prices rather than
    // a new copied balance constant. Loader's average is still checked against explicit source
    // composition/Cost witnesses above; integration must carry that cost-source chain too.
    let listed = skirmish
        .sides
        .iter()
        .flat_map(|s| s.vehicles.iter().chain(s.infantry.iter()))
        .copied()
        .collect::<Vec<_>>();
    if listed.is_empty() {
        return Err("empty starting pool mean".into());
    }
    let sum = listed.iter().try_fold(0i64, |sum, k| {
        d.production(ProductionItem::Unit(*k))
            .ok_or_else(|| "missing starting production".to_owned())
            .and_then(|p| {
                sum.checked_add(p.cost)
                    .ok_or_else(|| "starting cost overflow".into())
            })
    })?;
    if skirmish.average_starting_cost(d) != sum / (listed.len() as i64) {
        return Err("starting mean calculation".into());
    }
    if skirmish.average_starting_cost(d) != 475 {
        return Err("retain original shipped475 mean assertion".into());
    }
    let options = MatchOptions::default();
    for (field, actual, key) in [
        ("mcv_start", options.mcv_start, "Bases"),
        ("short_game", options.short_game, "ShortGame"),
        ("crates", options.crates, "Crates"),
        ("mcv_repacks", options.mcv_repacks, "MCVRedeploys"),
        (
            "bridge_destruction",
            options.bridge_destruction,
            "BridgeDestruction",
        ),
        ("fog_of_war", options.fog_of_war, "FogOfWar"),
    ] {
        let raw = rule(r, "MultiplayerDialogSettings", key)?;
        let display = raw.value.clone();
        check_int(
            a,
            "match_options.default",
            &format!("match_options.{field}"),
            "match_options",
            field,
            i128::from(actual),
            converted(raw, Conversion::Flag)?,
            display,
        )?;
    }
    // Native global bridge capability also agrees with default effective option; no claim this
    // comparison proves a second global controller overrides a user-disabled lobby switch.
    let raw = rule(r, "CombatDamage", "DestroyableBridges")?;
    let display = raw.value.clone();
    check_int(
        a,
        "match_options.default",
        "match_options.bridge_capability_default",
        "match_options",
        "bridge_capability_default",
        i128::from(options.bridge_destruction),
        converted(raw, Conversion::Flag)?,
        display,
    )?;
    if !options.superweapons
        || options.free_for_all
        || options.starting_credits.is_some()
        || options.unit_count.is_some()
    {
        return Err("own lobby defaults".into());
    }
    // Remaining game-owned options, labels, maps/starts/rosters have their explicit raw-field
    // own-value policies; not new native source dependencies inferred from their names.
    Ok(())
}
pub(super) fn superweapon_links(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    row: &MappingRow,
) -> AuditResult<()> {
    let Some(sw) = r
        .rows
        .get(&SourceKey::new("rules", &row.section, "SuperWeapon"))
    else {
        return Ok(());
    };
    let sw = sw.clone();
    let link = rule(r, &row.section, "SuperWeapon")?;
    let item = d
        .building_kind(&row.game_id)
        .map(ProductionItem::Building)
        .ok_or_else(|| "mapped SuperWeapon is not actual building".to_owned())?;
    let m = d
        .metadata(item)
        .ok_or_else(|| "missing SuperWeapon metadata".to_owned())?;
    let consumer = format!("{}@rules:{}", row.game_id, row.section);
    for (field, actual, key, conversion) in [
        (
            "is_powered",
            i128::from(m.is_powered),
            "IsPowered",
            Conversion::Flag,
        ),
        (
            "show_timer",
            i128::from(m.show_timer),
            "ShowTimer",
            Conversion::Flag,
        ),
        (
            "cooldown",
            i128::from(m.cooldown),
            "RechargeTime",
            Conversion::Minutes,
        ),
    ] {
        let raw = rule(r, &sw, key)?;
        let display = raw.value.clone();
        check_int(
            a,
            &consumer,
            &format!("arsenal.{field}"),
            &row.game_id,
            if field == "cooldown" {
                "strategic_cooldown"
            } else {
                field
            },
            actual,
            converted(raw, conversion)?.because(&link),
            display,
        )?;
    }
    let expected = rule(r, &sw, "Type")?
        .try_map(|v| match v.as_str() {
            "MultiMissile" => Ok("Nuke".to_owned()),
            "IronCurtain" => Ok("Curtain".into()),
            "LightningStorm" => Ok("Storm".into()),
            "ChronoSphere" => Ok("Chronosphere".into()),
            "ParaDrop" | "AmerParaDrop" => Ok("Paradrop".into()),
            _ => Err(format!("unmapped native superweapon Type {v}")),
        })?
        .because(&link);
    text(
        a,
        &consumer,
        "arsenal.role.superweapon_type",
        &row.game_id,
        "superweapon_type",
        format!("{:?}", m.role),
        expected,
        sw.clone(),
    )?;
    let raw = rule(r, &sw, "DisableableFromShell")?;
    let display = raw.value.clone();
    check_int(
        a,
        &consumer,
        "arsenal.role.major_superweapon",
        &row.game_id,
        "major_superweapon",
        i128::from(m.role.major_superweapon()),
        converted(raw, Conversion::Flag)?.because(&link),
        display,
    )?;
    let registry = registered(r, "SuperWeaponTypes", &sw)?;
    check_int(
        a,
        &consumer,
        "arsenal.role.registered_superweapon",
        &row.game_id,
        "registered_superweapon",
        i128::from(m.role.superweapon()),
        registry.map(i128::from).because(&link),
        sw,
    )?;
    Ok(())
}
pub(super) fn tiberium_registry(a: &mut Audit, r: &Reference, d: &Definitions) -> AuditResult<()> {
    let rules = d.rules().ok_or_else(|| "missing Rules".to_owned())?;
    for (index, field, actual) in [
        (0, "ore_value", rules.economy.ore_value),
        (1, "gem_value", rules.economy.gem_value),
    ] {
        let source = rule(r, "Tiberiums", &index.to_string())?;
        let raw = rule(r, &source.value, "Value")?;
        let display = raw.value.clone();
        check_int(
            a,
            "rules.economy",
            &format!("rules.economy.{field}.registry"),
            "rules",
            &format!("economy.{field}.registry"),
            i128::from(actual),
            converted(raw, Conversion::Int)?.because(&source),
            display,
        )?;
    }
    Ok(())
}

pub(super) fn crate_units(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    rows: &[MappingRow],
) -> AuditResult<()> {
    let rules = d.rules().ok_or_else(|| "missing Rules".to_owned())?;
    let actual = rules
        .crate_units
        .iter()
        .map(|&k| unit_id(d, k))
        .collect::<AuditResult<Vec<_>>>()?;
    let _ = unique_set(actual.clone(), "actual crate unit pool")?;
    if actual.is_empty() {
        return Err("retain nonempty shipped crate pool assertion".into());
    }
    let mut parts = Vec::new();
    for row in rows.iter().filter(|r| !r.building) {
        let raw = optional(r, &row.section, "CrateGoodie", "no")?;
        let display = raw.value.clone();
        let goodie = converted(raw, Conversion::Flag)?;
        check_int(
            a,
            &format!("crate_unit.{}@{}", row.game_id, row.section),
            "rules.crate_units.membership",
            "rules",
            &format!("crate_units.{}", row.game_id),
            i128::from(actual.contains(&row.game_id)),
            goodie.clone(),
            display,
        )?;
        parts.push(goodie.map(|yes| {
            if yes == 1 {
                Some(row.game_id.clone())
            } else {
                None
            }
        }));
    }
    let game_order = d.units().iter().map(|u| u.id.clone()).collect::<Vec<_>>();
    let expected = sequence(parts).try_map(|v| {
        // Named v3 draft correction: the actual declared policy is canonical UnitKind order,
        // matching current generator self.units traversal, not the v1 draft's lexical claim.
        // Native twin memberships remain separate; only canonical game identity is deduped.
        // nested_plans independently binds actual UnitKind order to explicit game policy.
        let eligible = v.into_iter().flatten().collect::<BTreeSet<_>>();
        let selected = game_order
            .into_iter()
            .filter(|id| eligible.contains(id))
            .collect::<Vec<_>>();
        if selected.len() != eligible.len() {
            return Err("native CrateGoodie missing canonical unit identity".into());
        }
        Ok(selected.join(","))
    })?;
    text(
        a,
        "rules.crate_units",
        "rules.crate_units.ordered_ids",
        "rules",
        "crate_units.ordered_ids",
        actual.join(","),
        expected,
        "CrateGoodie membership; canonical UnitKind catalogue order".into(),
    )?;
    Ok(())
}
pub(super) fn american_paradrop_infantry(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    ids: &BTreeMap<String, String>,
) -> AuditResult<()> {
    // The actual American drop branch uses defs.unit_kind("gi") with allied side fallback;
    // this independently checks that hardcoded runtime selector against the native source.
    let rules = d.rules().ok_or_else(|| "missing Rules".to_owned())?;
    let side = rules
        .side(Side::Allied)
        .ok_or_else(|| "missing allied side".to_owned())?;
    let actual = d.unit_kind("gi").unwrap_or(side.paradrop_infantry);
    let raw = rule(r, "General", "AmerParaDropInf")?;
    let display = raw.value.clone();
    text(
        a,
        "american_paradrop",
        "runtime.american_paradrop_infantry",
        "rules",
        "american_paradrop_infantry",
        unit_id(d, actual)?,
        mapped(raw, ids)?,
        display,
    )?;
    Ok(())
}
