//! Historical preparation note: UNVALIDATED independently declared array contracts plus actual native loaded projections.
//! Declaration never reads Definitions, successful adapter outputs, or actual raw lengths.
//! Comparison builds its own ExpectedElement provenance; exact dependency equality detects
//! omitted/foreign component associations. The separate ledger grants no global source credit.
use super::arrays::{
    Deps, Element, ExpectedElement, Ledger, PhysicalOrigin, Semantics, SourcePart, csv, merge,
    values, whole,
};
use super::*;
const ARMOURS: [&str; 11] = [
    "none",
    "flak",
    "plate",
    "light",
    "medium",
    "heavy",
    "wood",
    "steel",
    "concrete",
    "special_1",
    "special_2",
];
const IFV_NAMES: [&str; 13] = [
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
];
const TARGETS: [&str; 12] = [
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
];
fn origin(file: &str, list: &str, item: &str, field: &str, index: Option<usize>) -> PhysicalOrigin {
    PhysicalOrigin {
        file: file.into(),
        list: list.into(),
        item: item.into(),
        field: field.into(),
        index,
    }
}
fn id(c: &str, field: &str) -> String {
    format!("{c}/{field}")
}
fn text<'a>(r: &'a Reference, s: &str, key: &str, default: &'a str) -> &'a str {
    r.rows
        .get(&SourceKey::new("rules", s, key))
        .map_or(default, String::as_str)
}
fn from_expected(r: &Reference, parts: &BTreeSet<Atom>) -> AuditResult<Deps> {
    parts
        .iter()
        .map(|a| {
            Ok(match a.portion {
                Portion::Whole => SourcePart::Whole(a.key.clone()),
                Portion::Component(index) => SourcePart::Csv {
                    key: a.key.clone(),
                    index,
                    exact_len: *r
                        .shapes
                        .get(&a.key)
                        .ok_or("undeclared expected component shape")?,
                },
            })
        })
        .collect()
}
fn raw_entry<'a>(
    input: &'a Inputs,
    file: &str,
    container: &str,
    key: &str,
    wanted: &str,
) -> AuditResult<&'a Raw> {
    let rows = list(field(&input.documents[file], container, file)?, container)?;
    let found = rows
        .iter()
        .filter(|v| {
            field(v, key, container)
                .and_then(|v| atom(v, key))
                .is_ok_and(|id| id == wanted)
        })
        .collect::<Vec<_>>();
    if found.len() != 1 {
        return Err(format!(
            "actual physical row identity not unique {file}/{container}/{wanted}"
        ));
    }
    Ok(found[0])
}
fn raw_strings(
    input: &Inputs,
    file: &str,
    container: &str,
    key: &str,
    wanted: &str,
    name: &str,
) -> AuditResult<Vec<String>> {
    let row = raw_entry(input, file, container, key, wanted)?;
    list(field(row, name, container)?, name)?
        .iter()
        .map(|v| atom(v, name).map(str::to_owned))
        .collect()
}
fn declared_slots(r: &Reference, s: &str, elite: bool) -> AuditResult<(Vec<(String, Deps)>, Deps)> {
    // Independent source selector traversal, not items::choose_slot's Expected deps.
    fn take(r: &Reference, s: &str, keys: &[&str]) -> (Option<String>, Deps) {
        let mut d = Deps::new();
        for key in keys {
            d.extend(whole(r, s, key));
            let v = text(r, s, key, "none");
            if v != "none" {
                return (Some(v.into()), d);
            }
        }
        (None, d)
    }
    let (p, pd) = take(r, s, &["Primary", "Weapon1"]);
    let (q, qd) = take(r, s, &["Secondary"]);
    let parts = if elite {
        let (ep, mut ed) = take(r, s, &["ElitePrimary", "EliteWeapon1"]);
        let (eq, mut eed) = take(r, s, &["EliteSecondary"]);
        if ep.is_none() {
            ed.extend(pd);
        }
        if eq.is_none() {
            eed.extend(qd);
        }
        vec![(ep.or(p), ed), (eq.or(q), eed)]
    } else {
        vec![(p, pd), (q, qd)]
    };
    if parts[0].0.is_none() && parts[1].0.is_some() {
        return Err("source secondary-only slot needs an explicit array representation".into());
    }
    let length = parts.iter().flat_map(|(_, d)| d.iter().cloned()).collect();
    Ok((
        parts
            .into_iter()
            .filter_map(|(value, d)| value.map(|v| (v, d)))
            .collect(),
        length,
    ))
}
fn expected_slots(r: &Reference, s: &str, elite: bool) -> AuditResult<(Vec<(String, Deps)>, Deps)> {
    // Successful expected expressions use the existing actual conversion adapter's selectors;
    // demands above are independently authored and never copied from these dependencies.
    let p = items::choose_slot(r, s, &["Primary", "Weapon1"])?;
    let q = items::choose_slot(r, s, &["Secondary"])?;
    let parts = if elite {
        let ep = items::choose_slot(r, s, &["ElitePrimary", "EliteWeapon1"])?;
        let eq = items::choose_slot(r, s, &["EliteSecondary"])?;
        vec![
            if ep.value.is_some() {
                ep
            } else {
                p.because(&ep)
            },
            if eq.value.is_some() {
                eq
            } else {
                q.because(&eq)
            },
        ]
    } else {
        vec![p, q]
    };
    if parts[0].value.is_none() && parts[1].value.is_some() {
        return Err("expected secondary-only hole cannot be compacted".into());
    }
    let mut length = Deps::new();
    for part in &parts {
        length.extend(from_expected(r, &part.deps)?);
    }
    let retained = parts
        .into_iter()
        .filter_map(|v| v.value.clone().map(|value| (value, v.deps)))
        .map(|(value, deps)| Ok((value, from_expected(r, &deps)?)))
        .collect::<AuditResult<Vec<_>>>()?;
    Ok((retained, length))
}
fn omission(
    input: &Inputs,
    row: &MappingRow,
    label: &str,
    parts: &[(String, Deps)],
) -> AuditResult<(BTreeSet<String>, Deps)> {
    let source = parts
        .iter()
        .map(|v| v.0.clone())
        .collect::<Vec<_>>()
        .join(",");
    let differences = input
        .differences
        .iter()
        .filter(|d| {
            d.id.game_id == row.game_id && d.id.field == label && d.id.reference_value == source
        })
        .collect::<Vec<_>>();
    if differences.is_empty() {
        return Ok((BTreeSet::new(), Deps::new()));
    }
    if differences.len() != 1 {
        return Err("ambiguous listed weapon-slot projection".into());
    }
    let d = differences[0];
    let r = &input.reference;
    let mut dropped = BTreeSet::new();
    let mut premises = Deps::new();
    for (native, _) in parts {
        match native.as_str() {
            "VirtualScanner" => {
                if text(r, native, "NeverUse", "no") != "yes" {
                    return Err(
                        "listed VirtualScanner exclusion lost native NeverUse premise".into(),
                    );
                }
                dropped.insert(native.clone());
                premises.extend(whole(r, native, "NeverUse"));
            }
            "HornetCollision" | "ASWCollision" => {
                if !matches!(
                    (row.game_id.as_str(), native.as_str()),
                    ("hornet", "HornetCollision") | ("osprey", "ASWCollision")
                ) {
                    return Err("foreign collision slot exclusion".into());
                }
                let parents = input
                    .mappings
                    .iter()
                    .filter(|p| text(r, &p.section, "Spawns", "") == row.section)
                    .collect::<Vec<_>>();
                if parents.is_empty() {
                    return Err("R50 collision exclusion lost native parent Spawns link".into());
                }
                for p in parents {
                    premises.extend(whole(r, &p.section, "Spawns"));
                }
                dropped.insert(native.clone());
            }
            _ => {}
        }
    }
    if dropped.is_empty() {
        return Err(format!(
            "listed weapon difference has no explicit component projection policy {}",
            row.game_id
        ));
    }
    let weapon_ids = index(&input.weapons);
    let projected = parts
        .iter()
        .filter(|(name, _)| !dropped.contains(name))
        .map(|(name, _)| {
            weapon_ids
                .get(name)
                .cloned()
                .ok_or_else(|| format!("unknown retained source weapon {name}"))
        })
        .collect::<AuditResult<Vec<_>>>()?
        .join(",");
    if projected != d.id.game_value {
        return Err(
            "listed aggregate weapon difference and explicit component projection disagree".into(),
        );
    }
    Ok((dropped, premises))
}
fn numeric_csv(
    r: &Reference,
    s: &str,
    key: &str,
    index: usize,
    exact_len: usize,
    conversion: super::combat::Conversion,
) -> AuditResult<ExpectedElement> {
    let dep = SourcePart::Csv {
        key: SourceKey::new("rules", s, key),
        index,
        exact_len,
    };
    let raw = dep.read(r)?;
    let value = super::combat::converted(Expected::literal(raw.to_owned()), conversion)?
        .value
        .to_string();
    Ok(ExpectedElement {
        value,
        deps: BTreeSet::from([dep]),
    })
}
fn prereq_token(
    ids: &BTreeMap<String, String>,
    token: &str,
    groups: bool,
) -> AuditResult<Option<String>> {
    if let Some((_, _, id)) = rules::GROUPS
        .iter()
        .find(|(name, _, _)| name.eq_ignore_ascii_case(token))
    {
        return Ok(groups.then(|| id.to_string()));
    }
    if groups {
        return Ok(None);
    }
    Ok(Some(ids.get(token).cloned().ok_or_else(|| {
        format!("unmapped native prerequisite {token}")
    })?))
}
fn array17() -> Vec<(String, super::combat::Conversion, usize)> {
    let mut result = TARGETS
        .iter()
        .enumerate()
        .map(|(slot, k)| {
            (
                format!("AIIonCannon{k}Value"),
                super::combat::Conversion::Int,
                slot,
            )
        })
        .collect::<Vec<_>>();
    for (key, c, n) in [
        ("MultiplayerAICM", super::combat::Conversion::Int, 12),
        ("AIVirtualPurifiers", super::combat::Conversion::Int, 13),
        ("TeamDelays", super::combat::Conversion::Frames, 14),
        (
            "AlliedBaseDefenseCounts",
            super::combat::Conversion::Int,
            15,
        ),
        (
            "SovietBaseDefenseCounts",
            super::combat::Conversion::Int,
            16,
        ),
    ] {
        result.push((key.into(), c, n));
    }
    result
}
pub(super) fn declare(ledger: &mut Ledger, input: &Inputs) -> AuditResult<()> {
    let r = &input.reference;
    let item_ids = input.item_ids()?;
    let weapon_ids = index(&input.weapons);
    for (game, s) in &input.warheads {
        let c = format!("warhead.{game}@rules:{s}");
        let parts = values(r, s, "Verses")?;
        if !parts.is_empty() && parts.len() != 11 {
            return Err("native Verses exact11 contract".into());
        }
        let demands = ARMOURS
            .iter()
            .enumerate()
            .map(|(n, name)| {
                let value = if parts.is_empty() {
                    "100".into()
                } else {
                    super::actual::decimal(parts[n].trim_end_matches('%')).and_then(|(v, d)| {
                        if d == 1 {
                            Ok(v.to_string())
                        } else {
                            Err("noninteger native Verses percentage".into())
                        }
                    })?
                };
                Ok((
                    Element::Member(format!("{name}:{value}")),
                    if parts.is_empty() {
                        Deps::new()
                    } else {
                        csv(s, "Verses", n, 11)
                    },
                ))
            })
            .collect::<AuditResult<Vec<_>>>()?;
        ledger.declare(
            id(&c, "combat.ron.warheads.versus"),
            Semantics::UniqueSet,
            demands,
            whole(r, s, "Verses"),
        )?;
    }
    for row in &input.mappings {
        let c = format!("{}@rules:{}", row.game_id, row.section);
        let s = &row.section;
        for (elite, label) in [(false, "weapons"), (true, "elite_weapons")] {
            let (parts, slot_length) = declared_slots(r, s, elite)?;
            let (dropped, premises) = omission(input, row, label, &parts)?;
            let length = merge(slot_length, premises);
            let retained = parts
                .into_iter()
                .filter(|(name, _)| !dropped.contains(name))
                .collect::<Vec<_>>();
            let demands = retained
                .into_iter()
                .enumerate()
                .map(|(n, (_, d))| (Element::Index(n), d))
                .collect();
            let file_list = if row.building { "buildings" } else { "units" };
            ledger.declare(
                id(&c, &format!("combat.ron.{file_list}.{label}")),
                Semantics::Ordered,
                demands,
                length,
            )?;
        }
        let source = values(r, s, "Prerequisite")?;
        for (groups, field) in [
            (
                false,
                if row.building {
                    "economy.ron.buildings.prerequisites"
                } else {
                    "economy.ron.units.prerequisites"
                },
            ),
            (true, "arsenal.ron.entries.prerequisite_groups"),
        ] {
            let mut demands = Vec::new();
            for (index, token) in source.iter().enumerate() {
                if let Some(member) = prereq_token(&item_ids, token, groups)? {
                    demands.push((
                        Element::Member(member),
                        csv(s, "Prerequisite", index, source.len()),
                    ));
                }
            }
            ledger.declare(
                id(&c, field),
                Semantics::UniqueSet,
                demands,
                whole(r, s, "Prerequisite"),
            )?;
        }
        if row.building {
            let free = values(r, s, "FreeUnit")?;
            for name in &free {
                if !item_ids.contains_key(name) {
                    return Err("unmapped FreeUnit native component".into());
                }
            }
            ledger.declare(
                id(&c, "economy.ron.buildings.free_units"),
                Semantics::Ordered,
                (0..free.len())
                    .map(|n| (Element::Index(n), csv(s, "FreeUnit", n, free.len())))
                    .collect(),
                whole(r, s, "FreeUnit"),
            )?;
        }
        let gunner = text(r, s, "Gunner", "no") == "yes";
        if gunner {
            if text(r, s, "WeaponCount", "") != "13" {
                return Err("native IFV exact13 demand".into());
            }
            for n in 1..=13 {
                if !weapon_ids.contains_key(text(r, s, &format!("Weapon{n}"), "")) {
                    return Err("missing/unmapped numbered IFV weapon".into());
                }
            }
            let selectors = merge(whole(r, s, "Gunner"), whole(r, s, "WeaponCount"));
            let demands = (0..13)
                .map(|n| {
                    (
                        Element::Index(n),
                        merge(selectors.clone(), whole(r, s, &format!("Weapon{}", n + 1))),
                    )
                })
                .collect();
            ledger.declare(
                id(&c, "arsenal.ron.entries.ifv_weapons"),
                Semantics::Ordered,
                demands,
                selectors,
            )?;
            let mut named = Vec::new();
            for (slot, name) in IFV_NAMES.iter().enumerate() {
                let selector = format!("{name}TurretWeapon");
                let index = text(r, s, &selector, "")
                    .parse::<usize>()
                    .map_err(|e| e.to_string())?;
                if index >= 13 {
                    return Err("named native IFV selector outside13".into());
                }
                named.push((
                    Element::Index(slot),
                    merge(
                        whole(r, s, &selector),
                        whole(r, s, &format!("Weapon{}", index + 1)),
                    ),
                ));
            }
            ledger.declare(
                id(&c, "derived.ifv_named_modes"),
                Semantics::Ordered,
                named,
                merge(whole(r, s, "Gunner"), whole(r, s, "WeaponCount")),
            )?;
            ledger.declare_origins(
                &id(&c, "derived.ifv_named_modes"),
                (0..13)
                    .map(|n| {
                        origin(
                            "arsenal.ron",
                            "entries",
                            &row.game_id,
                            "ifv_weapons",
                            Some(n),
                        )
                    })
                    .collect(),
            )?;
            let demands = (0..13)
                .map(|n| {
                    (
                        Element::Index(n),
                        whole(r, s, &format!("EliteWeapon{}", n + 1)),
                    )
                })
                .collect();
            ledger.declare(
                id(&c, "derived.ifv_effective_elite"),
                Semantics::Ordered,
                demands,
                merge(whole(r, s, "Gunner"), whole(r, s, "WeaponCount")),
            )?;
            ledger.declare_origins(
                &id(&c, "derived.ifv_effective_elite"),
                (0..13)
                    .map(|n| {
                        if n == 0 {
                            origin(
                                "combat.ron",
                                if row.building { "buildings" } else { "units" },
                                &row.game_id,
                                "elite_weapons",
                                Some(0),
                            )
                        } else {
                            origin(
                                "arsenal.ron",
                                "entries",
                                &row.game_id,
                                "ifv_weapons",
                                Some(n),
                            )
                        }
                    })
                    .collect(),
            )?;
        } else {
            ledger.declare(
                id(&c, "arsenal.ron.entries.ifv_weapons"),
                Semantics::Ordered,
                Vec::new(),
                whole(r, s, "Gunner"),
            )?;
        }
    }
    for field in [
        "rules.ron.countries.id",
        "skirmish.ron.factions.id",
        "rules.ron.countries.side",
    ] {
        let mut demands = Vec::new();
        let mut length = Deps::new();
        for (index, (native, _)) in rules::HOUSES.iter().enumerate() {
            let mut deps = merge(
                whole(r, "Countries", &index.to_string()),
                whole(r, native, "Multiplay"),
            );
            length.extend(deps.clone());
            if field.ends_with(".side") {
                deps.extend(whole(r, native, "Side"));
            }
            demands.push((Element::Index(index), deps));
        }
        ledger.declare(field.into(), Semantics::Ordered, demands, length)?;
    }
    ledger.declare(
        "rules.ron.difficulty.ids".into(),
        Semantics::Ordered,
        (0..3).map(|n| (Element::Index(n), Deps::new())).collect(),
        Deps::new(),
    )?;
    for (key, _, field_index) in array17() {
        let array_id = format!("rules.ron.difficulty.by_source.{key}");
        ledger.declare(
            array_id.clone(),
            Semantics::Ordered,
            [2, 1, 0]
                .into_iter()
                .enumerate()
                .map(|(slot, n)| (Element::Index(slot), csv("General", &key, n, 3)))
                .collect(),
            whole(r, "General", &key),
        )?;
        let field = if field_index < 12 {
            "target_weights"
        } else {
            match field_index {
                12 => "credits_bonus_percent",
                13 => "virtual_purifiers",
                14 => "team_delay",
                15 => "allied_base_defences",
                16 => "soviet_base_defences",
                _ => return Err("unknown independent difficulty origin".into()),
            }
        };
        ledger.declare_origins(
            &array_id,
            ["easy", "medium", "brutal"]
                .into_iter()
                .map(|name| {
                    origin(
                        "rules.ron",
                        "difficulty",
                        name,
                        field,
                        (field_index < 12).then_some(field_index),
                    )
                })
                .collect(),
        )?;
    }
    for (id_, source_index) in [("easy", 2), ("medium", 1), ("brutal", 0)] {
        let demands = TARGETS
            .iter()
            .enumerate()
            .map(|(slot, k)| {
                (
                    Element::Index(slot),
                    csv("General", &format!("AIIonCannon{k}Value"), source_index, 3),
                )
            })
            .collect();
        let length = TARGETS
            .iter()
            .flat_map(|k| whole(r, "General", &format!("AIIonCannon{k}Value")))
            .collect();
        ledger.declare(
            format!("difficulty.{id_}/rules.ron.difficulty.target_weights"),
            Semantics::Ordered,
            demands,
            length,
        )?;
    }
    Ok(())
}

fn observe_verses(input: &Inputs, game: &str) -> AuditResult<Vec<String>> {
    let row = raw_entry(input, "combat.ron", "warheads", "id", game)?;
    let raw = list(field(row, "versus", "warhead")?, "versus")?;
    if raw.len() != 11 {
        return Err("actual raw Verses must contain exact11 named armour records".into());
    }
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for component in raw {
        super::raw::exact_fields(component, &["armour", "percent"], "versus component")?;
        let armour = atom(field(component, "armour", "versus")?, "armour")?;
        if !ARMOURS.contains(&armour) || !seen.insert(armour) {
            return Err("unknown/duplicate actual raw armour association".into());
        }
        out.push(format!(
            "{armour}:{}",
            atom(field(component, "percent", "versus")?, "percent")?
        ));
    }
    if seen != ARMOURS.into_iter().collect() {
        return Err("actual raw Verses armour identity set differs from native11".into());
    }
    Ok(out)
}
fn difficulty_value(d: &sim::DifficultyRules, field: usize) -> AuditResult<String> {
    Ok(match field {
        0..=11 => d.target_weights[field].to_string(),
        12 => d.credits_bonus_percent.to_string(),
        13 => d.virtual_purifiers.to_string(),
        14 => d.team_delay.to_string(),
        15 => d.allied_base_defences.to_string(),
        16 => d.soviet_base_defences.to_string(),
        _ => return Err("unknown source difficulty array".into()),
    })
}
fn raw_difficulty_value(input: &Inputs, id: &str, field: usize) -> AuditResult<String> {
    let row = raw_entry(input, "rules.ron", "difficulty", "id", id)?;
    if field < 12 {
        let values = list(
            super::field(row, "target_weights", "difficulty")?,
            "target_weights",
        )?;
        if values.len() != 12 {
            return Err("actual raw target classes require exact12".into());
        }
        return atom(&values[field], "target weight").map(str::to_owned);
    }
    let key = match field {
        12 => "credits_bonus_percent",
        13 => "virtual_purifiers",
        14 => "team_delay",
        15 => "allied_base_defences",
        16 => "soviet_base_defences",
        _ => return Err("difficulty field index".into()),
    };
    atom(super::field(row, key, "difficulty")?, key).map(str::to_owned)
}
pub(super) fn compare(ledger: &mut Ledger, input: &Inputs, d: &Definitions) -> AuditResult<()> {
    let r = &input.reference;
    let item_ids = input.item_ids()?;
    let weapon_ids = index(&input.weapons);
    let rules = d.rules().ok_or("actual native Rules")?;
    if d.armour_classes()
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        != ARMOURS
    {
        return Err(
            "loaded armour ordinal order differs from native11 source Verses interpretation".into(),
        );
    }
    for (game, s) in &input.warheads {
        let c = format!("warhead.{game}@rules:{s}");
        let w = d
            .warhead_kind(game)
            .and_then(|k| d.warhead(k))
            .ok_or("actual mapped warhead")?;
        if w.versus.len() != 11 {
            return Err("loaded Verses exact11".into());
        }
        let raw = observe_verses(input, game)?;
        let loaded = ARMOURS
            .iter()
            .zip(&w.versus)
            .map(|(armour, value)| format!("{armour}:{value}"))
            .collect::<Vec<_>>();
        let mut expected = Vec::new();
        for (index, name) in ARMOURS.iter().enumerate() {
            let source = SourceKey::new("rules", s, "Verses");
            let value = if r.rows.contains_key(&source) {
                let mut part = numeric_csv(
                    r,
                    s,
                    "Verses",
                    index,
                    11,
                    super::combat::Conversion::Percent,
                )?;
                part.value = format!("{name}:{}", part.value);
                part
            } else {
                ExpectedElement {
                    value: format!("{name}:100"),
                    deps: Deps::new(),
                }
            };
            expected.push(value);
        }
        ledger.compare(
            r,
            &id(&c, "combat.ron.warheads.versus"),
            raw,
            loaded,
            expected,
            whole(r, s, "Verses"),
        )?;
    }
    for row in &input.mappings {
        let c = format!("{}@rules:{}", row.game_id, row.section);
        let s = &row.section;
        let item = items::item(d, row)?;
        let combat = match item {
            sim::ProductionItem::Unit(k) => d.unit_combat(k),
            sim::ProductionItem::Building(k) => d.building_combat(k),
        }
        .ok_or("actual combat terms")?;
        let meta = d.metadata(item).ok_or("actual metadata")?;
        let (container, identity) = if row.building {
            ("buildings", "building")
        } else {
            ("units", "unit")
        };
        for (elite, label) in [(false, "weapons"), (true, "elite_weapons")] {
            let (parts, length) = expected_slots(r, s, elite)?;
            let (dropped, premises) = omission(input, row, label, &parts)?;
            let expected = parts
                .into_iter()
                .filter(|(native, _)| !dropped.contains(native))
                .map(|(native, deps)| {
                    Ok(ExpectedElement {
                        value: weapon_ids
                            .get(&native)
                            .cloned()
                            .ok_or_else(|| format!("retained weapon missing map {native}"))?,
                        deps,
                    })
                })
                .collect::<AuditResult<Vec<_>>>()?;
            let slots = combat.weapons(elite);
            if slots[0].is_none() && slots[1].is_some() {
                return Err("actual secondary-only hole cannot acquire compact array proof".into());
            }
            let loaded = slots
                .into_iter()
                .flatten()
                .map(|kind| super::rules::weapon_id(d, kind))
                .collect::<AuditResult<Vec<_>>>()?;
            ledger.compare(
                r,
                &id(&c, &format!("combat.ron.{container}.{label}")),
                raw_strings(
                    input,
                    "combat.ron",
                    container,
                    identity,
                    &row.game_id,
                    label,
                )?,
                loaded,
                expected,
                merge(length, premises),
            )?;
        }
        let source = values(r, s, "Prerequisite")?;
        let production = d.production(item).ok_or("actual production")?;
        for groups in [false, true] {
            let physical = if groups {
                "arsenal.ron.entries.prerequisite_groups"
            } else if row.building {
                "economy.ron.buildings.prerequisites"
            } else {
                "economy.ron.units.prerequisites"
            };
            let mut expected = Vec::new();
            for (index, token) in source.iter().enumerate() {
                if let Some(value) = prereq_token(&item_ids, token, groups)? {
                    expected.push(ExpectedElement {
                        value,
                        deps: csv(s, "Prerequisite", index, source.len()),
                    });
                }
            }
            let (raw_values, loaded) = if groups {
                (
                    raw_strings(
                        input,
                        "arsenal.ron",
                        "entries",
                        "id",
                        &row.game_id,
                        "prerequisite_groups",
                    )?,
                    meta.prerequisite_groups
                        .iter()
                        .map(|k| {
                            rules
                                .group(*k)
                                .map(|v| v.id.clone())
                                .ok_or_else(|| "actual group ID missing".into())
                        })
                        .collect::<AuditResult<Vec<_>>>()?,
                )
            } else {
                let (cn, identity) = if row.building {
                    ("buildings", "id")
                } else {
                    ("units", "unit")
                };
                (
                    raw_strings(
                        input,
                        "economy.ron",
                        cn,
                        identity,
                        &row.game_id,
                        "prerequisites",
                    )?,
                    production
                        .prerequisites
                        .iter()
                        .map(|k| super::rules::building_id(d, *k))
                        .collect::<AuditResult<Vec<_>>>()?,
                )
            };
            ledger.compare(
                r,
                &id(&c, physical),
                raw_values,
                loaded,
                expected,
                whole(r, s, "Prerequisite"),
            )?;
        }
        if let sim::ProductionItem::Building(kind) = item {
            let b = d.building(kind).ok_or("actual building")?;
            let tokens = values(r, s, "FreeUnit")?;
            let expected = tokens
                .iter()
                .enumerate()
                .map(|(index, native)| {
                    Ok(ExpectedElement {
                        value: item_ids
                            .get(native)
                            .cloned()
                            .ok_or("unmapped source FreeUnit")?,
                        deps: csv(s, "FreeUnit", index, tokens.len()),
                    })
                })
                .collect::<AuditResult<Vec<_>>>()?;
            let loaded = b
                .free_units
                .iter()
                .map(|kind| super::rules::unit_id(d, *kind))
                .collect::<AuditResult<Vec<_>>>()?;
            ledger.compare(
                r,
                &id(&c, "economy.ron.buildings.free_units"),
                raw_strings(
                    input,
                    "economy.ron",
                    "buildings",
                    "id",
                    &row.game_id,
                    "free_units",
                )?,
                loaded,
                expected,
                whole(r, s, "FreeUnit"),
            )?;
        }
        let gunner = text(r, s, "Gunner", "no") == "yes";
        let selectors = if gunner {
            merge(whole(r, s, "Gunner"), whole(r, s, "WeaponCount"))
        } else {
            whole(r, s, "Gunner")
        };
        let mut expected = Vec::new();
        if gunner {
            if !meta.gunner {
                return Err("native Gunner is not actual Gunner".into());
            }
            for n in 1..=13 {
                let raw = super::actual::rule(r, s, &format!("Weapon{n}"))?;
                expected.push(ExpectedElement {
                    value: weapon_ids
                        .get(&raw.value)
                        .cloned()
                        .ok_or("numbered expected IFV weapon missing")?,
                    deps: merge(selectors.clone(), from_expected(r, &raw.deps)?),
                });
            }
        }
        let raw = raw_strings(
            input,
            "arsenal.ron",
            "entries",
            "id",
            &row.game_id,
            "ifv_weapons",
        )?;
        let loaded = meta
            .ifv_weapons
            .iter()
            .map(|k| super::rules::weapon_id(d, *k))
            .collect::<AuditResult<Vec<_>>>()?;
        ledger.compare(
            r,
            &id(&c, "arsenal.ron.entries.ifv_weapons"),
            raw.clone(),
            loaded,
            expected,
            selectors.clone(),
        )?;
        if gunner {
            if meta.ifv_weapons.len() != 13 || raw.len() != 13 {
                return Err("effective elite IFV requires actual/raw13".into());
            }
            let mut raw_effective = raw;
            let raw_elite = raw_strings(
                input,
                "combat.ron",
                container,
                identity,
                &row.game_id,
                "elite_weapons",
            )?;
            raw_effective[0] = raw_elite
                .first()
                .cloned()
                .ok_or("actual elite-primary raw origin absent")?;
            let loaded = (0..13)
                .map(|n| {
                    let k = if n == 0 {
                        combat.elite[0].ok_or("actual mode0 elite primary absent")?
                    } else {
                        meta.ifv_weapons[n]
                    };
                    super::rules::weapon_id(d, k)
                })
                .collect::<AuditResult<Vec<_>>>()?;
            let mut expected = Vec::new();
            for n in 1..=13 {
                let raw = super::actual::rule(r, s, &format!("EliteWeapon{n}"))?;
                expected.push(ExpectedElement {
                    value: weapon_ids
                        .get(&raw.value)
                        .cloned()
                        .ok_or("source elite IFV weapon missing")?,
                    deps: from_expected(r, &raw.deps)?,
                });
            }
            let observed = (0..13)
                .map(|n| {
                    if n == 0 {
                        origin(
                            "combat.ron",
                            container,
                            &row.game_id,
                            "elite_weapons",
                            Some(0),
                        )
                    } else {
                        origin(
                            "arsenal.ron",
                            "entries",
                            &row.game_id,
                            "ifv_weapons",
                            Some(n),
                        )
                    }
                })
                .collect();
            ledger.observe_origins(&id(&c, "derived.ifv_effective_elite"), observed)?;
            ledger.compare(
                r,
                &id(&c, "derived.ifv_effective_elite"),
                raw_effective,
                loaded,
                expected,
                selectors.clone(),
            )?;
            let mut named_expected = Vec::new();
            for name in IFV_NAMES {
                let selector = super::actual::rule(r, s, &format!("{name}TurretWeapon"))?;
                let n = selector.value.parse::<usize>().map_err(|e| e.to_string())?;
                if n >= 13 {
                    return Err("named expected IFV index outside13".into());
                }
                let weapon = super::actual::rule(r, s, &format!("Weapon{}", n + 1))?;
                named_expected.push(ExpectedElement {
                    value: weapon_ids
                        .get(&weapon.value)
                        .cloned()
                        .ok_or("named IFV source weapon unmapped")?,
                    deps: merge(
                        from_expected(r, &selector.deps)?,
                        from_expected(r, &weapon.deps)?,
                    ),
                });
            }
            ledger.observe_origins(
                &id(&c, "derived.ifv_named_modes"),
                (0..meta.ifv_weapons.len())
                    .map(|n| {
                        origin(
                            "arsenal.ron",
                            "entries",
                            &row.game_id,
                            "ifv_weapons",
                            Some(n),
                        )
                    })
                    .collect(),
            )?;
            ledger.compare(
                r,
                &id(&c, "derived.ifv_named_modes"),
                raw_strings(
                    input,
                    "arsenal.ron",
                    "entries",
                    "id",
                    &row.game_id,
                    "ifv_weapons",
                )?,
                meta.ifv_weapons
                    .iter()
                    .map(|k| super::rules::weapon_id(d, *k))
                    .collect::<AuditResult<Vec<_>>>()?,
                named_expected,
                selectors,
            )?;
        }
    }
    if rules.countries.len() != 9 {
        return Err("actual country order requires exact9".into());
    }
    let mut country_expected = Vec::new();
    let mut side_expected = Vec::new();
    let mut country_length = Deps::new();
    for (index, (native, game)) in super::rules::HOUSES.iter().enumerate() {
        let registry = super::actual::rule(r, "Countries", &index.to_string())?;
        if registry.value != *native {
            return Err("native playable country ordinal differs".into());
        }
        let playable = super::actual::rule(r, native, "Multiplay")?;
        if !matches!(playable.value.as_str(), "yes" | "true") {
            return Err("source country not playable".into());
        }
        let deps = merge(
            from_expected(r, &registry.deps)?,
            from_expected(r, &playable.deps)?,
        );
        country_length.extend(deps.clone());
        country_expected.push(ExpectedElement {
            value: game.to_string(),
            deps: deps.clone(),
        });
        let side = super::rules::source_side(super::actual::rule(r, native, "Side")?)?;
        side_expected.push(ExpectedElement {
            value: side.value,
            deps: merge(deps, from_expected(r, &side.deps)?),
        });
    }
    let raw_country_rows = list(
        super::field(&input.documents["rules.ron"], "countries", "rules")?,
        "countries",
    )?;
    let raw_factions = list(
        super::field(&input.documents["skirmish.ron"], "factions", "skirmish")?,
        "factions",
    )?;
    let country_raw = raw_country_rows
        .iter()
        .map(|row| atom(super::field(row, "id", "country")?, "id").map(str::to_owned))
        .collect::<AuditResult<Vec<_>>>()?;
    let faction_raw = raw_factions
        .iter()
        .map(|row| atom(super::field(row, "id", "faction")?, "id").map(str::to_owned))
        .collect::<AuditResult<Vec<_>>>()?;
    ledger.compare(
        r,
        "rules.ron.countries.id",
        country_raw,
        rules.countries.iter().map(|v| v.id.clone()).collect(),
        country_expected.clone(),
        country_length.clone(),
    )?;
    let skirmish = sim::load_skirmish(input.data_dir.join("skirmish.ron"), d)
        .map_err(|e| format!("actual skirmish array projection: {e}"))?;
    ledger.compare(
        r,
        "skirmish.ron.factions.id",
        faction_raw,
        skirmish.factions.iter().map(|v| v.id.clone()).collect(),
        country_expected,
        country_length.clone(),
    )?;
    let raw_side = raw_country_rows
        .iter()
        .map(|row| atom(super::field(row, "side", "country")?, "side").map(str::to_owned))
        .collect::<AuditResult<Vec<_>>>()?;
    ledger.compare(
        r,
        "rules.ron.countries.side",
        raw_side,
        rules
            .countries
            .iter()
            .map(|v| v.side.id().to_string())
            .collect(),
        side_expected,
        country_length,
    )?;
    let raw_ids = list(
        super::field(&input.documents["rules.ron"], "difficulty", "rules")?,
        "difficulty",
    )?
    .iter()
    .map(|row| atom(super::field(row, "id", "difficulty")?, "id").map(str::to_owned))
    .collect::<AuditResult<Vec<_>>>()?;
    ledger.compare(
        r,
        "rules.ron.difficulty.ids",
        raw_ids,
        rules.difficulties.iter().map(|d| d.id.clone()).collect(),
        ["easy", "medium", "brutal"]
            .into_iter()
            .map(|id| ExpectedElement {
                value: id.into(),
                deps: Deps::new(),
            })
            .collect(),
        Deps::new(),
    )?;
    if rules.difficulties.len() != 3 {
        return Err("actual difficulty exact3 before component projection".into());
    }
    for (key, conversion, field_index) in array17() {
        let mut expected = Vec::new();
        for source_index in [2, 1, 0] {
            expected.push(numeric_csv(
                r,
                "General",
                &key,
                source_index,
                3,
                conversion,
            )?);
        }
        let raw_values = ["easy", "medium", "brutal"]
            .into_iter()
            .map(|id| raw_difficulty_value(input, id, field_index))
            .collect::<AuditResult<Vec<_>>>()?;
        let loaded = rules
            .difficulties
            .iter()
            .map(|d| difficulty_value(d, field_index))
            .collect::<AuditResult<Vec<_>>>()?;
        let observed_field = if field_index < 12 {
            "target_weights"
        } else {
            [
                "credits_bonus_percent",
                "virtual_purifiers",
                "team_delay",
                "allied_base_defences",
                "soviet_base_defences",
            ][field_index - 12]
        };
        let actual_origins = rules
            .difficulties
            .iter()
            .map(|row| {
                origin(
                    "rules.ron",
                    "difficulty",
                    &row.id,
                    observed_field,
                    (field_index < 12).then_some(field_index),
                )
            })
            .collect();
        ledger.observe_origins(
            &format!("rules.ron.difficulty.by_source.{key}"),
            actual_origins,
        )?;
        ledger.compare(
            r,
            &format!("rules.ron.difficulty.by_source.{key}"),
            raw_values,
            loaded,
            expected,
            whole(r, "General", &key),
        )?;
    }
    for (index, (name, source_index)) in [("easy", 2), ("medium", 1), ("brutal", 0)]
        .into_iter()
        .enumerate()
    {
        let raw = raw_strings(
            input,
            "rules.ron",
            "difficulty",
            "id",
            name,
            "target_weights",
        )?;
        let loaded = rules.difficulties[index]
            .target_weights
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>();
        let mut expected = Vec::new();
        for target in TARGETS {
            expected.push(numeric_csv(
                r,
                "General",
                &format!("AIIonCannon{target}Value"),
                source_index,
                3,
                super::combat::Conversion::Int,
            )?);
        }
        let length = TARGETS
            .iter()
            .flat_map(|target| whole(r, "General", &format!("AIIonCannon{target}Value")))
            .collect();
        ledger.compare(
            r,
            &format!("difficulty.{name}/rules.ron.difficulty.target_weights"),
            raw,
            loaded,
            expected,
            length,
        )?;
    }
    Ok(())
}
pub(super) fn run(input: &Inputs, d: &Definitions) -> AuditResult<()> {
    let mut ledger = Ledger::default();
    declare(&mut ledger, input)?;
    compare(&mut ledger, input, d)?;
    ledger.finish()
}
