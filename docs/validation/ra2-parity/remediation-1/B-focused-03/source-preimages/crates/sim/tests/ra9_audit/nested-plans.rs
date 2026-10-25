//! Historical preparation note: UNVALIDATED v3 remaining typed native and explicit game-owned array/tuple witnesses.
//! This supplemental ledger records NO global reference coverage or semantic non-use.
use super::arrays::{
    Deps, Element, ExpectedElement, Ledger, PhysicalOrigin, Semantics, SourcePart, csv, merge,
    values, whole,
};
use super::combat::{Conversion, converted};
use super::*;
const ABILITIES: [&str; 7] = [
    "stronger",
    "firepower",
    "rof",
    "sight",
    "faster",
    "self_heal",
    "scatter",
];
const SIDE_FIELDS: [&str; 8] = [
    "id",
    "base_unit",
    "construction_yard",
    "crew",
    "survivor_divisor",
    "disguise",
    "paradrop_infantry",
    "paradrop_count",
];
const SIDE_KEYS: [(&str, &str, &str, &str, &str, &str, &str); 2] = [
    (
        "allied",
        "GDI",
        "AlliedCrew",
        "AlliedSurvivorDivisor",
        "AlliedDisguise",
        "AllyParaDropInf",
        "AllyParaDropNum",
    ),
    (
        "soviet",
        "Nod",
        "SovietCrew",
        "SovietSurvivorDivisor",
        "SovietDisguise",
        "SovParaDropInf",
        "SovParaDropNum",
    ),
];
const POWERUPS: [(&str, &str, Option<Conversion>); 19] = [
    ("Armor", "armour", Some(Conversion::Permille)),
    ("Cloak", "cloak", None),
    ("Darkness", "darkness", None),
    ("Explosion", "explosion", None),
    ("Firepower", "firepower", Some(Conversion::Permille)),
    ("Gas", "gas", None),
    ("HealBase", "heal_base", None),
    ("ICBM", "icbm", None),
    ("Invulnerability", "invulnerability", None),
    ("IonStorm", "ion_storm", None),
    ("Money", "money", Some(Conversion::Int)),
    ("Napalm", "napalm", None),
    ("Pod", "pod", None),
    ("Reveal", "reveal", None),
    ("Speed", "speed", Some(Conversion::Permille)),
    ("Squad", "squad", None),
    ("Tiberium", "tiberium", None),
    ("Unit", "unit", None),
    ("Veteran", "veteran", Some(Conversion::Int)),
];
fn source<'a>(r: &'a Reference, s: &str, k: &str, default: &'a str) -> &'a str {
    r.rows
        .get(&SourceKey::new("rules", s, k))
        .map_or(default, String::as_str)
}
fn flag(r: &Reference, s: &str, k: &str, default: &str) -> AuditResult<bool> {
    match source(r, s, k, default) {
        "yes" | "true" => Ok(true),
        "no" | "false" => Ok(false),
        v => Err(format!("unrecognized native flag {s}/{k}/{v}")),
    }
}
fn dependency(r: &Reference, atoms: &BTreeSet<Atom>) -> AuditResult<Deps> {
    atoms
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
                        .ok_or("expected atom has no independent explicit shape")?,
                },
            })
        })
        .collect()
}
fn entry<'a>(
    input: &'a Inputs,
    file: &str,
    list_name: &str,
    key: &str,
    wanted: &str,
) -> AuditResult<&'a Raw> {
    let matches = list(field(&input.documents[file], list_name, file)?, list_name)?
        .iter()
        .filter(|v| {
            field(v, key, list_name)
                .and_then(|v| atom(v, key))
                .is_ok_and(|s| s == wanted)
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(format!(
            "raw keyed row missing/duplicated {file}/{list_name}/{wanted}"
        ));
    }
    Ok(matches[0])
}
fn strings(
    input: &Inputs,
    file: &str,
    ln: &str,
    key: &str,
    wanted: &str,
    name: &str,
) -> AuditResult<Vec<String>> {
    list(field(entry(input, file, ln, key, wanted)?, name, ln)?, name)?
        .iter()
        .map(|v| atom(v, name).map(str::to_owned))
        .collect()
}
fn ids(input: &Inputs, file: &str, ln: &str, key: &str) -> AuditResult<Vec<String>> {
    list(field(&input.documents[file], ln, file)?, ln)?
        .iter()
        .map(|v| atom(field(v, key, ln)?, key).map(str::to_owned))
        .collect()
}
fn item_consumer(row: &MappingRow) -> String {
    format!("{}@rules:{}", row.game_id, row.section)
}
fn field_id(row: &MappingRow, name: &str) -> String {
    format!("{}/arsenal.ron.entries.{name}", item_consumer(row))
}
fn e(value: String, deps: Deps) -> ExpectedElement {
    ExpectedElement { value, deps }
}
fn set_demands(parts: Vec<(String, Deps)>) -> Vec<(Element, Deps)> {
    parts
        .into_iter()
        .map(|(v, d)| (Element::Member(v), d))
        .collect()
}
fn house_id(native: &str) -> AuditResult<String> {
    rules::HOUSES
        .iter()
        .find(|(n, _)| *n == native)
        .map(|(_, id)| id.to_string())
        .ok_or_else(|| format!("unresolved native house identity {native}"))
}
fn ability(native: &str) -> AuditResult<String> {
    let v = native.to_ascii_lowercase();
    if !ABILITIES.contains(&v.as_str()) {
        return Err(format!("unresolved native ability {native}"));
    }
    Ok(v)
}
fn origin(file: &str, ln: &str, item: &str, key: &str, index: Option<usize>) -> PhysicalOrigin {
    PhysicalOrigin {
        file: file.into(),
        list: ln.into(),
        item: item.into(),
        field: key.into(),
        index,
    }
}
// A narrow pre-existing explicit projection, not generic omission. Both native twins are
// demanded independently and retain their exact source components in the length premise.
fn merged_restriction(input: &Inputs, row: &MappingRow, key: &str) -> AuditResult<bool> {
    if key != "ForbiddenHouses" || !matches!(row.game_id.as_str(), "engineer" | "attack_dog") {
        return Ok(false);
    }
    let sections = if row.game_id == "engineer" {
        ["ENGINEER", "SENGINEER"]
    } else {
        ["ADOG", "DOG"]
    };
    if !sections.contains(&row.section.as_str()) {
        return Err("foreign merged-type restriction projection".into());
    }
    let native = source(&input.reference, &row.section, key, "");
    let found = input
        .differences
        .iter()
        .filter(|d| {
            d.id.game_id == row.game_id
                && d.id.field == "forbidden_countries"
                && d.id.reference_value == native
                && d.id.game_value.is_empty()
        })
        .collect::<Vec<_>>();
    if found.len() != 1 || native.is_empty() {
        return Err("merged country exclusion lost exact existing 3.3 difference".into());
    }
    Ok(true)
}
fn restriction_length(input: &Inputs, row: &MappingRow, key: &str) -> AuditResult<Deps> {
    let r = &input.reference;
    let mut deps = whole(r, &row.section, key);
    if merged_restriction(input, row, key)? {
        let tokens = values(r, &row.section, key)?;
        for n in 0..tokens.len() {
            deps.extend(csv(&row.section, key, n, tokens.len()));
        }
    }
    Ok(deps)
}
fn independent_owner(r: &Reference, s: &str) -> AuditResult<(Vec<(String, Deps)>, Deps)> {
    let owners = values(r, s, "Owner")?;
    let mut length = whole(r, s, "Owner");
    if owners.is_empty() {
        return Ok((Vec::new(), length));
    }
    if owners.iter().collect::<BTreeSet<_>>().len() != owners.len() {
        return Err("native duplicate Owner house".into());
    }
    let mut memberships: BTreeMap<String, Deps> = BTreeMap::new();
    let mut matched = BTreeSet::new();
    for (side, native) in [("allied", "GDI"), ("soviet", "Nod")] {
        let houses = values(r, "Sides", native)?;
        if houses.iter().collect::<BTreeSet<_>>().len() != houses.len() {
            return Err("duplicate native side house".into());
        }
        length.extend(whole(r, "Sides", native));
        for (i, owner) in owners.iter().enumerate() {
            for (j, house) in houses.iter().enumerate() {
                if owner == house {
                    matched.insert(i);
                    memberships.entry(side.into()).or_default().extend(merge(
                        csv(s, "Owner", i, owners.len()),
                        csv("Sides", native, j, houses.len()),
                    ));
                }
            }
        }
    }
    if matched.len() != owners.len() {
        return Err("Owner has an unresolved/unmapped side house".into());
    }
    Ok((memberships.into_iter().collect(), length))
}
fn expected_owner(r: &Reference, s: &str) -> AuditResult<(Vec<ExpectedElement>, Deps)> {
    let raw = super::actual::optional(r, s, "Owner", "")?;
    let owners = rules::csv(&raw.value)?;
    let mut length = dependency(r, &raw.deps)?;
    if owners.is_empty() {
        return Ok((Vec::new(), length));
    }
    let a = super::actual::rule(r, "Sides", "GDI")?;
    let b = super::actual::rule(r, "Sides", "Nod")?;
    length.extend(dependency(r, &a.deps)?);
    length.extend(dependency(r, &b.deps)?);
    let sides = [
        ("allied", "GDI", rules::csv(&a.value)?),
        ("soviet", "Nod", rules::csv(&b.value)?),
    ];
    let mut result: BTreeMap<String, Deps> = BTreeMap::new();
    let mut found = BTreeSet::new();
    for (index, owner) in owners.iter().enumerate() {
        for (side, native, houses) in &sides {
            if let Some(slot) = houses.iter().position(|h| h == owner) {
                found.insert(index);
                result.entry((*side).into()).or_default().extend(merge(
                    csv(s, "Owner", index, owners.len()),
                    csv("Sides", native, slot, houses.len()),
                ));
            }
        }
    }
    if found.len() != owners.len() {
        return Err("expected Owner projection lost native house".into());
    }
    Ok((result.into_iter().map(|(v, d)| e(v, d)).collect(), length))
}
// Independent active short-circuit demand. It intentionally does not call the successful
// rules::starting_eligible expression or copy that expression's dependency collection.
fn eligible_demand(
    r: &Reference,
    row: &MappingRow,
    native_side: &str,
) -> AuditResult<(bool, Deps)> {
    let s = &row.section;
    let mut deps = whole(r, s, "AllowedToStartInMultiplayer");
    if !flag(r, s, "AllowedToStartInMultiplayer", "yes")? {
        return Ok((false, deps));
    }
    deps.extend(whole(r, s, "TechLevel"));
    let tech = source(r, s, "TechLevel", "")
        .parse::<i128>()
        .map_err(|e| e.to_string())?;
    if tech < 0 {
        return Ok((false, deps));
    }
    deps.extend(whole(r, "MultiplayerDialogSettings", "TechLevel"));
    let top = source(r, "MultiplayerDialogSettings", "TechLevel", "")
        .parse::<i128>()
        .map_err(|e| e.to_string())?;
    if tech > top {
        return Ok((false, deps));
    }
    deps.extend(whole(r, s, "Owner"));
    let owners = values(r, s, "Owner")?;
    if owners.is_empty() {
        return Ok((false, deps));
    }
    deps.extend(whole(r, "Sides", native_side));
    let houses = values(r, "Sides", native_side)?;
    if !owners.iter().all(|o| houses.contains(o)) {
        return Ok((false, deps));
    }
    for key in [
        "RequiredHouses",
        "ForbiddenHouses",
        "RequiresStolenAlliedTech",
        "RequiresStolenSovietTech",
    ] {
        let d = whole(r, s, key);
        if !d.is_empty() {
            deps.extend(d);
            return Ok((false, deps));
        }
    }
    deps.extend(whole(r, "General", "BaseUnit"));
    Ok((!values(r, "General", "BaseUnit")?.contains(s), deps))
}
fn infantry_demand(r: &Reference, s: &str) -> AuditResult<(bool, Deps)> {
    let keys = r
        .rows
        .iter()
        .filter(|(k, v)| k.file == "rules" && k.section == "InfantryTypes" && v.as_str() == s)
        .map(|(k, _)| SourcePart::Whole(k.clone()))
        .collect::<Deps>();
    if keys.len() > 1 {
        return Err("duplicate native infantry registry identity".into());
    }
    Ok((!keys.is_empty(), keys))
}
fn own_policy() -> AuditResult<BTreeMap<String, Vec<String>>> {
    super::policy_admission::arrays()
}
fn own_origin(id: &str, values: &[String]) -> AuditResult<Option<Vec<PhysicalOrigin>>> {
    if let Some(map) = id
        .strip_prefix("skirmish.map.")
        .and_then(|v| v.strip_suffix(".starts.xy"))
    {
        if !values.len().is_multiple_of(2) {
            return Err("authored start coordinate pair policy is odd".into());
        }
        return Ok(Some(
            (0..values.len())
                .map(|n| {
                    origin(
                        "skirmish.ron",
                        "maps",
                        map,
                        &format!("starts[{}].{}", n / 2, if n % 2 == 0 { "x" } else { "y" }),
                        None,
                    )
                })
                .collect(),
        ));
    }
    Ok(None)
}
fn declare_native(ledger: &mut Ledger, input: &Inputs) -> AuditResult<()> {
    let r = &input.reference;
    let map = input.item_ids()?;
    for row in &input.mappings {
        for (name, key) in [
            ("veteran_abilities", "VeteranAbilities"),
            ("elite_abilities", "EliteAbilities"),
        ] {
            let tokens = values(r, &row.section, key)?;
            let parts = if row.building {
                Vec::new()
            } else {
                tokens
                    .iter()
                    .enumerate()
                    .map(|(n, v)| Ok((ability(v)?, csv(&row.section, key, n, tokens.len()))))
                    .collect::<AuditResult<Vec<_>>>()?
            };
            ledger.declare(
                field_id(row, name),
                Semantics::UniqueSet,
                set_demands(parts),
                if row.building {
                    Deps::new()
                } else {
                    whole(r, &row.section, key)
                },
            )?;
        }
        let (parts, length) = independent_owner(r, &row.section)?;
        ledger.declare(
            field_id(row, "owner_sides"),
            Semantics::UniqueSet,
            set_demands(parts),
            length,
        )?;
        for (name, key) in [
            ("required_countries", "RequiredHouses"),
            ("forbidden_countries", "ForbiddenHouses"),
        ] {
            let tokens = values(r, &row.section, key)?;
            let parts = if merged_restriction(input, row, key)? {
                Vec::new()
            } else {
                tokens
                    .iter()
                    .enumerate()
                    .map(|(n, v)| Ok((house_id(v)?, csv(&row.section, key, n, tokens.len()))))
                    .collect::<AuditResult<Vec<_>>>()?
            };
            ledger.declare(
                field_id(row, name),
                Semantics::UniqueSet,
                set_demands(parts),
                restriction_length(input, row, key)?,
            )?;
        }
    }
    for (_, key, id) in rules::GROUPS {
        let tokens = values(r, "General", key)?;
        let parts = tokens
            .iter()
            .enumerate()
            .map(|(n, v)| {
                Ok((
                    map.get(v)
                        .cloned()
                        .ok_or("native group building unmapped")?,
                    csv("General", key, n, tokens.len()),
                ))
            })
            .collect::<AuditResult<Vec<_>>>()?;
        ledger.declare(
            format!("group.{id}/rules.ron.prerequisite_groups.buildings"),
            Semantics::UniqueSet,
            set_demands(parts),
            whole(r, "General", key),
        )?;
    }
    // Side fields are a keyed record, not an incidental RON tuple or side-record order.
    let bases = values(r, "General", "BaseUnit")?;
    if bases.len() != 2 {
        return Err("native BaseUnit exact2".into());
    }
    for (n, (side, _, crew, divisor, disguise, infantry, count)) in
        SIDE_KEYS.into_iter().enumerate()
    {
        let mut parts = Vec::new();
        for name in SIDE_FIELDS {
            let deps = match name {
                "id" => Deps::new(),
                "base_unit" => csv("General", "BaseUnit", n, 2),
                "construction_yard" => merge(
                    csv("General", "BaseUnit", n, 2),
                    whole(r, &bases[n], "DeploysInto"),
                ),
                "crew" => whole(r, "General", crew),
                "survivor_divisor" => whole(r, "General", divisor),
                "disguise" => whole(r, "General", disguise),
                "paradrop_infantry" => whole(r, "General", infantry),
                "paradrop_count" => whole(r, "General", count),
                _ => return Err("unregistered side field".into()),
            };
            parts.push((Element::Member(name.to_owned()), deps));
        } // Member key is field name; actual values include field:value below, so declare exact values separately.
        let mut keyed = Vec::new();
        for (field, _) in &parts {
            let Element::Member(name) = field else {
                return Err("side field member key".into());
            };
            let value = declared_side_value(
                r,
                &map,
                side,
                name,
                &bases[n],
                [crew, divisor, disguise, infantry, count],
            )?;
            let deps = parts
                .iter()
                .find(|(k, _)| k == field)
                .ok_or("independent side field demand")?
                .1
                .clone();
            keyed.push((Element::Member(format!("{name}:{value}")), deps));
        }
        ledger.declare(
            format!("side.{side}/rules.ron.sides.fields"),
            Semantics::UniqueSet,
            keyed,
            whole(r, "General", "BaseUnit"),
        )?;
    }
    let mut crate_parts: BTreeMap<String, Deps> = BTreeMap::new();
    let mut crate_length = Deps::new();
    for row in input.mappings.iter().filter(|v| !v.building) {
        let deps = whole(r, &row.section, "CrateGoodie");
        crate_length.extend(deps.clone());
        if flag(r, &row.section, "CrateGoodie", "no")? {
            crate_parts
                .entry(row.game_id.clone())
                .or_default()
                .extend(deps);
        }
    }
    let catalogue = own_policy()?
        .get("units.ron.units.identities.order")
        .cloned()
        .ok_or("explicit game unit order policy")?;
    let selected = catalogue
        .iter()
        .filter_map(|id| crate_parts.get(id).cloned())
        .enumerate()
        .map(|(n, d)| (Element::Index(n), d))
        .collect::<Vec<_>>();
    if selected.len() != crate_parts.len() {
        return Err("CrateGoodie type absent from explicit unit order policy".into());
    }
    ledger.declare(
        "rules.ron.crate_units.ordered".into(),
        Semantics::Ordered,
        selected,
        crate_length,
    )?;
    let mut cost_parts = Vec::new();
    let mut cost_origins = Vec::new();
    let mut cost_length = Deps::new();
    for (side, native, _, _, _, _, _) in SIDE_KEYS {
        for (infantry, label) in [(false, "starting_vehicles"), (true, "starting_infantry")] {
            let mut parts = Vec::new();
            let mut length = Deps::new();
            for row in input
                .mappings
                .iter()
                .filter(|v| !v.building && v.note != "twin")
            {
                let (eligible, mut deps) = eligible_demand(r, row, native)?;
                if eligible {
                    let (foot, registry) = infantry_demand(r, &row.section)?;
                    deps.extend(registry);
                    if foot == infantry {
                        parts.push((Element::Index(parts.len()), deps.clone()));
                        let mut cost = deps.clone();
                        cost.extend(whole(r, &row.section, "Cost"));
                        cost_parts.push((Element::Index(cost_parts.len()), cost));
                        cost_origins.push(origin(
                            "economy.ron",
                            "units",
                            &row.game_id,
                            "cost",
                            None,
                        ));
                    }
                }
                length.extend(deps);
            }
            cost_length.extend(length.clone());
            ledger.declare(
                format!("start.{side}/skirmish.ron.sides.{label}"),
                Semantics::Ordered,
                parts,
                length,
            )?;
        }
    }
    ledger.declare(
        "derived.starting.native_costs".into(),
        Semantics::Ordered,
        cost_parts,
        cost_length,
    )?;
    ledger.declare_origins("derived.starting.native_costs", cost_origins)?;
    Ok(())
}
fn declared_side_value(
    r: &Reference,
    map: &BTreeMap<String, String>,
    side: &str,
    field: &str,
    base: &str,
    keys: [&str; 5],
) -> AuditResult<String> {
    let [crew, divisor, disguise, infantry, count] = keys;
    let native = match field {
        "id" => return Ok(side.into()),
        "base_unit" => base,
        "construction_yard" => source(r, base, "DeploysInto", ""),
        "crew" => source(r, "General", crew, ""),
        "disguise" => source(r, "General", disguise, ""),
        "paradrop_infantry" => source(r, "General", infantry, ""),
        "survivor_divisor" => {
            return Ok(source(r, "General", divisor, "")
                .parse::<i128>()
                .map_err(|e| e.to_string())?
                .to_string());
        }
        "paradrop_count" => {
            return Ok(source(r, "General", count, "")
                .parse::<i128>()
                .map_err(|e| e.to_string())?
                .to_string());
        }
        _ => return Err("unknown side source field".into()),
    };
    map.get(native)
        .cloned()
        .ok_or_else(|| format!("unmapped native side field {native}"))
}
fn compare_native(
    ledger: &mut Ledger,
    input: &Inputs,
    d: &Definitions,
    s: &SkirmishDefinitions,
) -> AuditResult<()> {
    let r = &input.reference;
    let rules = d.rules().ok_or("actual native Rules")?;
    let map = input.item_ids()?;
    for row in &input.mappings {
        let m = d.metadata(items::item(d, row)?).ok_or("actual metadata")?;
        for (name, key, actual) in [
            (
                "veteran_abilities",
                "VeteranAbilities",
                &m.veteran_abilities,
            ),
            ("elite_abilities", "EliteAbilities", &m.elite_abilities),
        ] {
            let tokens = if row.building {
                Vec::new()
            } else {
                rules::csv(&super::actual::optional(r, &row.section, key, "")?.value)?
            };
            let expected = tokens
                .iter()
                .enumerate()
                .map(|(n, v)| Ok(e(ability(v)?, csv(&row.section, key, n, tokens.len()))))
                .collect::<AuditResult<Vec<_>>>()?;
            ledger.compare(
                r,
                &field_id(row, name),
                strings(input, "arsenal.ron", "entries", "id", &row.game_id, name)?,
                actual.iter().map(|v| v.id().to_owned()).collect(),
                expected,
                if row.building {
                    Deps::new()
                } else {
                    whole(r, &row.section, key)
                },
            )?;
        }
        let (expected, length) = expected_owner(r, &row.section)?;
        ledger.compare(
            r,
            &field_id(row, "owner_sides"),
            strings(
                input,
                "arsenal.ron",
                "entries",
                "id",
                &row.game_id,
                "owner_sides",
            )?,
            m.owner_sides.iter().map(|v| v.id().into()).collect(),
            expected,
            length,
        )?;
        for (name, key, actual) in [
            (
                "required_countries",
                "RequiredHouses",
                &m.required_countries,
            ),
            (
                "forbidden_countries",
                "ForbiddenHouses",
                &m.forbidden_countries,
            ),
        ] {
            let tokens = rules::csv(&super::actual::optional(r, &row.section, key, "")?.value)?;
            let expected = if merged_restriction(input, row, key)? {
                Vec::new()
            } else {
                tokens
                    .iter()
                    .enumerate()
                    .map(|(n, v)| Ok(e(house_id(v)?, csv(&row.section, key, n, tokens.len()))))
                    .collect::<AuditResult<Vec<_>>>()?
            };
            let loaded = actual
                .iter()
                .map(|id| {
                    rules
                        .country(*id)
                        .map(|c| c.id.clone())
                        .ok_or("actual country ID missing".into())
                })
                .collect::<AuditResult<Vec<_>>>()?;
            ledger.compare(
                r,
                &field_id(row, name),
                strings(input, "arsenal.ron", "entries", "id", &row.game_id, name)?,
                loaded,
                expected,
                restriction_length(input, row, key)?,
            )?;
        }
    }
    for (_, key, id) in rules::GROUPS {
        let source = super::actual::rule(r, "General", key)?;
        let tokens = rules::csv(&source.value)?;
        let expected = tokens
            .iter()
            .enumerate()
            .map(|(n, v)| {
                Ok(e(
                    map.get(v)
                        .cloned()
                        .ok_or("expected group building unmapped")?,
                    csv("General", key, n, tokens.len()),
                ))
            })
            .collect::<AuditResult<Vec<_>>>()?;
        let group = rules
            .group_id(id)
            .and_then(|k| rules.group(k))
            .ok_or("actual native group")?;
        ledger.compare(
            r,
            &format!("group.{id}/rules.ron.prerequisite_groups.buildings"),
            strings(
                input,
                "rules.ron",
                "prerequisite_groups",
                "id",
                id,
                "buildings",
            )?,
            group
                .buildings
                .iter()
                .map(|k| super::rules::building_id(d, *k))
                .collect::<AuditResult<Vec<_>>>()?,
            expected,
            dependency(r, &source.deps)?,
        )?;
    }
    let base = super::actual::rule(r, "General", "BaseUnit")?;
    let bases = rules::csv(&base.value)?;
    if bases.len() != 2 {
        return Err("expected BaseUnit exact2".into());
    }
    for (index, (side, _, crew, divisor, disguise, infantry, count)) in
        SIDE_KEYS.into_iter().enumerate()
    {
        let side_kind = if side == "allied" {
            sim::Side::Allied
        } else {
            sim::Side::Soviet
        };
        let actual = rules.side(side_kind).ok_or("actual side")?;
        let mut expected = Vec::new();
        expected.push(e(format!("id:{side}"), Deps::new()));
        let native_base = bases[index].clone();
        expected.push(e(
            format!(
                "base_unit:{}",
                map.get(&native_base).ok_or("source base map")?
            ),
            csv("General", "BaseUnit", index, 2),
        ));
        let deploy = super::actual::rule(r, &native_base, "DeploysInto")?;
        expected.push(e(
            format!(
                "construction_yard:{}",
                map.get(&deploy.value).ok_or("source yard map")?
            ),
            merge(
                csv("General", "BaseUnit", index, 2),
                dependency(r, &deploy.deps)?,
            ),
        ));
        for (name, key) in [
            ("crew", crew),
            ("disguise", disguise),
            ("paradrop_infantry", infantry),
        ] {
            let raw = super::actual::rule(r, "General", key)?;
            expected.push(e(
                format!(
                    "{name}:{}",
                    map.get(&raw.value).ok_or("source side unit map")?
                ),
                dependency(r, &raw.deps)?,
            ));
        }
        for (name, key) in [("survivor_divisor", divisor), ("paradrop_count", count)] {
            let value = converted(super::actual::rule(r, "General", key)?, Conversion::Int)?;
            expected.push(e(
                format!("{name}:{}", value.value),
                dependency(r, &value.deps)?,
            ));
        }
        let raw = entry(input, "rules.ron", "sides", "id", side)?;
        let raw_values = tuple(raw, "side tuple")?
            .iter()
            .map(|(name, v)| Ok(format!("{name}:{}", atom(v, name)?)))
            .collect::<AuditResult<Vec<_>>>()?;
        let loaded = vec![
            format!("id:{}", actual.side.id()),
            format!("base_unit:{}", super::rules::unit_id(d, actual.base_unit)?),
            format!(
                "construction_yard:{}",
                super::rules::building_id(d, actual.construction_yard)?
            ),
            format!("crew:{}", super::rules::unit_id(d, actual.crew)?),
            format!("survivor_divisor:{}", actual.survivor_divisor),
            format!("disguise:{}", super::rules::unit_id(d, actual.disguise)?),
            format!(
                "paradrop_infantry:{}",
                super::rules::unit_id(d, actual.paradrop_infantry)?
            ),
            format!("paradrop_count:{}", actual.paradrop_count),
        ];
        ledger.compare(
            r,
            &format!("side.{side}/rules.ron.sides.fields"),
            raw_values,
            loaded,
            expected,
            dependency(r, &base.deps)?,
        )?;
    }
    let mut crate_expected: BTreeMap<String, Deps> = BTreeMap::new();
    let mut length = Deps::new();
    for row in input.mappings.iter().filter(|v| !v.building) {
        let value = converted(
            super::actual::optional(r, &row.section, "CrateGoodie", "no")?,
            Conversion::Flag,
        )?;
        let deps = dependency(r, &value.deps)?;
        length.extend(deps.clone());
        if value.value == 1 {
            crate_expected
                .entry(row.game_id.clone())
                .or_default()
                .extend(deps);
        }
    }
    let raw = list(
        field(&input.documents["rules.ron"], "crate_units", "rules")?,
        "crate_units",
    )?
    .iter()
    .map(|v| atom(v, "crate unit").map(str::to_owned))
    .collect::<AuditResult<Vec<_>>>()?;
    ledger.compare(
        r,
        "rules.ron.crate_units.ordered",
        raw,
        rules
            .crate_units
            .iter()
            .map(|k| super::rules::unit_id(d, *k))
            .collect::<AuditResult<Vec<_>>>()?,
        own_policy()?
            .get("units.ron.units.identities.order")
            .ok_or("explicit unit order policy")?
            .iter()
            .filter_map(|id| crate_expected.get(id).cloned().map(|d| e(id.clone(), d)))
            .collect(),
        length,
    )?;
    let top = converted(
        super::actual::rule(r, "MultiplayerDialogSettings", "TechLevel")?,
        Conversion::Int,
    )?;
    let bases = super::actual::rule(r, "General", "BaseUnit")?.try_map(|v| rules::csv(&v))?;
    let mut costs = Vec::new();
    let mut raw_costs = Vec::new();
    let mut native_costs = Vec::new();
    let mut cost_expected = Vec::new();
    let mut cost_origins = Vec::new();
    let mut cost_length = Deps::new();
    for (side, native, _, _, _, _, _) in SIDE_KEYS {
        let houses = super::actual::rule(r, "Sides", native)?.try_map(|v| rules::csv(&v))?;
        let pool = s
            .side(if side == "allied" {
                sim::Side::Allied
            } else {
                sim::Side::Soviet
            })
            .ok_or("actual side start")?;
        for (infantry, label, actual) in [
            (false, "starting_vehicles", &pool.vehicles),
            (true, "starting_infantry", &pool.infantry),
        ] {
            let mut expected = Vec::new();
            let mut length = Deps::new();
            for row in input
                .mappings
                .iter()
                .filter(|v| !v.building && v.note != "twin")
            {
                let eligible = super::rules::starting_eligible(r, row, &houses, &bases, &top)?;
                let mut deps = dependency(r, &eligible.deps)?;
                if eligible.value {
                    let foot = super::rules::registered(r, "InfantryTypes", &row.section)?;
                    deps.extend(dependency(r, &foot.deps)?);
                    if foot.value == infantry {
                        let price = converted(
                            super::actual::rule(r, &row.section, "Cost")?,
                            Conversion::Int,
                        )?;
                        expected.push(e(row.game_id.clone(), deps.clone()));
                        let mut cost_deps = deps.clone();
                        cost_deps.extend(dependency(r, &price.deps)?);
                        cost_expected.push(e(price.value.to_string(), cost_deps));
                        native_costs.push(price.value);
                    }
                }
                length.extend(deps);
            }
            cost_length.extend(length.clone());
            let loaded = actual
                .iter()
                .map(|k| super::rules::unit_id(d, *k))
                .collect::<AuditResult<Vec<_>>>()?;
            ledger.compare(
                r,
                &format!("start.{side}/skirmish.ron.sides.{label}"),
                strings(input, "skirmish.ron", "sides", "side", side, label)?,
                loaded,
                expected,
                length,
            )?;
            for kind in actual {
                let id = super::rules::unit_id(d, *kind)?;
                raw_costs.push(
                    atom(
                        field(
                            entry(input, "economy.ron", "units", "unit", &id)?,
                            "cost",
                            "starting cost",
                        )?,
                        "cost",
                    )?
                    .to_string(),
                );
                cost_origins.push(origin("economy.ron", "units", &id, "cost", None));
                costs.push(i128::from(
                    d.production(sim::ProductionItem::Unit(*kind))
                        .ok_or("actual starting production")?
                        .cost,
                ));
            }
        }
    }
    ledger.observe_origins("derived.starting.native_costs", cost_origins)?;
    ledger.compare(
        r,
        "derived.starting.native_costs",
        raw_costs,
        costs.iter().map(i128::to_string).collect(),
        cost_expected,
        cost_length,
    )?;
    if costs.len() != native_costs.len() || costs.is_empty() {
        return Err("actual/native starting mean component length".into());
    }
    for (index, (actual, expected)) in costs.iter().zip(&native_costs).enumerate() {
        if actual != expected {
            return Err(format!(
                "actual starting cost source chain differs component{index}"
            ));
        }
    }
    let sum = costs.iter().try_fold(0i128, |n, v| {
        n.checked_add(*v).ok_or("actual starting cost sum overflow")
    })?;
    let source_sum = native_costs.iter().try_fold(0i128, |n, v| {
        n.checked_add(*v).ok_or("native starting cost sum overflow")
    })?;
    let count = i128::try_from(costs.len()).map_err(|e| e.to_string())?;
    if i128::from(s.average_starting_cost(d)) != sum / count || sum / count != source_sum / count {
        return Err("actual starting mean lacks exact native cost-source association".into());
    }
    Ok(())
}
// Main catalogue arrays are keyed by actual game identity. Their RON record order is not a
// native type-list ordering claim. Optional membership includes every actual native selector.
type RegistryDemand = (String, Vec<(String, Deps)>, Deps);
fn registry_demands(input: &Inputs) -> AuditResult<Vec<RegistryDemand>> {
    let r = &input.reference;
    let unit_ids = input
        .mappings
        .iter()
        .filter(|v| !v.building)
        .map(|v| v.game_id.clone())
        .collect::<BTreeSet<_>>();
    let building_ids = input
        .mappings
        .iter()
        .filter(|v| v.building)
        .map(|v| v.game_id.clone())
        .collect::<BTreeSet<_>>();
    let all = unit_ids
        .union(&building_ids)
        .cloned()
        .collect::<BTreeSet<_>>();
    let literal = |ids: BTreeSet<String>| {
        ids.into_iter()
            .map(|id| (id, Deps::new()))
            .collect::<Vec<_>>()
    };
    let mut out = Vec::new();
    for (file, ln, kind) in [
        ("units.ron", "units", 0),
        ("economy.ron", "units", 0),
        ("economy.ron", "buildings", 1),
        ("combat.ron", "units", 0),
        ("combat.ron", "buildings", 1),
        ("arsenal.ron", "entries", 2),
    ] {
        out.push((
            format!("{file}.{ln}.identities"),
            literal(match kind {
                0 => unit_ids.clone(),
                1 => building_ids.clone(),
                _ => all.clone(),
            }),
            Deps::new(),
        ));
    }
    for (ln, table) in [("weapons", &input.weapons), ("warheads", &input.warheads)] {
        out.push((
            format!("combat.ron.{ln}.identities"),
            literal(table.iter().map(|v| v.0.clone()).collect()),
            Deps::new(),
        ));
    }
    out.push((
        "combat.ron.effects.identities".into(),
        literal(
            sim::Effect::ALL
                .iter()
                .map(|v| v.id().to_string())
                .collect(),
        ),
        Deps::new(),
    ));
    out.push((
        "combat.ron.armour_classes.identities".into(),
        literal(
            [
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
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        ),
        Deps::new(),
    ));
    for (ln, key, buildings) in [
        ("harvesters", "Harvester", false),
        ("refineries", "Refinery", true),
        ("refunds", "Soylent", true),
    ] {
        let mut elements: BTreeMap<String, Deps> = BTreeMap::new();
        let mut length = Deps::new();
        for row in input.mappings.iter().filter(|v| v.building == buildings) {
            let deps = whole(r, &row.section, key);
            length.extend(deps.clone());
            let present = if key == "Soylent" {
                source(r, &row.section, key, "0")
                    .parse::<i128>()
                    .map_err(|e| e.to_string())?
                    != 0
            } else {
                flag(r, &row.section, key, "no")?
            };
            if present {
                elements
                    .entry(row.game_id.clone())
                    .or_default()
                    .extend(deps);
            }
        }
        out.push((
            format!("economy.ron.{ln}.identities"),
            elements.into_iter().collect(),
            length,
        ));
    }
    out.push((
        "rules.ron.prerequisite_groups.identities".into(),
        literal(rules::GROUPS.iter().map(|v| v.2.to_string()).collect()),
        Deps::new(),
    ));
    out.push((
        "rules.ron.sides.identities".into(),
        literal(
            ["allied", "soviet"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        ),
        Deps::new(),
    ));
    out.push((
        "skirmish.ron.sides.identities".into(),
        literal(
            ["allied", "soviet"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        ),
        Deps::new(),
    ));
    let mut parts = Vec::new();
    let mut length = Deps::new();
    for (native, game, _) in POWERUPS {
        let tokens = values(r, "Powerups", native)?;
        if !matches!(tokens.len(), 3 | 4) {
            return Err("native Powerup requires exact3or4 tuple".into());
        }
        let deps = csv("Powerups", native, 0, tokens.len());
        length.extend(deps.clone());
        if tokens[0].parse::<i128>().map_err(|e| e.to_string())? > 0 {
            parts.push((game.into(), deps));
        }
    }
    out.push(("rules.ron.powerups.identities".into(), parts, length));
    Ok(out)
}
fn declare_registries(ledger: &mut Ledger, input: &Inputs) -> AuditResult<()> {
    for (id, parts, length) in registry_demands(input)? {
        ledger.declare(id, Semantics::UniqueSet, set_demands(parts), length)?;
    }
    Ok(())
}
fn compare_registries(
    ledger: &mut Ledger,
    input: &Inputs,
    d: &Definitions,
    s: &SkirmishDefinitions,
) -> AuditResult<()> {
    let r = &input.reference;
    let rules = d.rules().ok_or("actual Rules")?;
    let unit_ids = d.units().iter().map(|v| v.id.clone()).collect::<Vec<_>>();
    let building_ids = d
        .buildings()
        .iter()
        .map(|v| v.id.clone())
        .collect::<Vec<_>>();
    // Successful expected identity sets are rebuilt from committed mapping rows; demands
    // above are a separate traversal. Runtime identity sets come from actual loaded tables.
    let mapped_units = input
        .mappings
        .iter()
        .filter(|v| !v.building)
        .map(|v| v.game_id.clone())
        .collect::<BTreeSet<_>>();
    let mapped_buildings = input
        .mappings
        .iter()
        .filter(|v| v.building)
        .map(|v| v.game_id.clone())
        .collect::<BTreeSet<_>>();
    for (file, ln, key, kind) in [
        ("units.ron", "units", "id", 0),
        ("economy.ron", "units", "unit", 0),
        ("economy.ron", "buildings", "id", 1),
        ("combat.ron", "units", "unit", 0),
        ("combat.ron", "buildings", "building", 1),
        ("arsenal.ron", "entries", "id", 2),
    ] {
        let expected_ids = match kind {
            0 => mapped_units.clone(),
            1 => mapped_buildings.clone(),
            _ => mapped_units.union(&mapped_buildings).cloned().collect(),
        };
        let loaded = match (file, kind) {
            ("economy.ron", 0) => d
                .units()
                .iter()
                .filter_map(|u| {
                    d.unit_kind(&u.id)
                        .filter(|k| d.production(sim::ProductionItem::Unit(*k)).is_some())
                        .map(|_| u.id.clone())
                })
                .collect(),
            ("combat.ron", 0) => d
                .units()
                .iter()
                .filter_map(|u| {
                    d.unit_kind(&u.id)
                        .filter(|k| d.unit_combat(*k).is_some())
                        .map(|_| u.id.clone())
                })
                .collect(),
            ("combat.ron", 1) => d
                .buildings()
                .iter()
                .filter_map(|b| {
                    d.building_kind(&b.id)
                        .filter(|k| d.building_combat(*k).is_some())
                        .map(|_| b.id.clone())
                })
                .collect(),
            (_, 0) => unit_ids.clone(),
            (_, 1) => building_ids.clone(),
            _ => d
                .arsenal()
                .0
                .keys()
                .map(|item| match item {
                    sim::ProductionItem::Unit(k) => super::rules::unit_id(d, *k),
                    sim::ProductionItem::Building(k) => super::rules::building_id(d, *k),
                })
                .collect::<AuditResult<Vec<_>>>()?,
        };
        ledger.compare(
            r,
            &format!("{file}.{ln}.identities"),
            ids(input, file, ln, key)?,
            loaded,
            expected_ids
                .into_iter()
                .map(|v| e(v, Deps::new()))
                .collect(),
            Deps::new(),
        )?;
    }
    for (ln, loaded, table) in [
        (
            "weapons",
            d.weapons().iter().map(|v| v.id.clone()).collect::<Vec<_>>(),
            &input.weapons,
        ),
        (
            "warheads",
            d.warheads()
                .iter()
                .map(|v| v.id.clone())
                .collect::<Vec<_>>(),
            &input.warheads,
        ),
    ] {
        ledger.compare(
            r,
            &format!("combat.ron.{ln}.identities"),
            ids(input, "combat.ron", ln, "id")?,
            loaded,
            table
                .iter()
                .map(|v| v.0.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .map(|v| e(v, Deps::new()))
                .collect(),
            Deps::new(),
        )?;
    }
    ledger.compare(
        r,
        "combat.ron.effects.identities",
        ids(input, "combat.ron", "effects", "id")?,
        sim::Effect::ALL
            .iter()
            .filter(|v| d.effect(**v).is_some())
            .map(|v| v.id().to_string())
            .collect(),
        sim::Effect::ALL
            .iter()
            .map(|v| e(v.id().to_string(), Deps::new()))
            .collect(),
        Deps::new(),
    )?;
    ledger.compare(
        r,
        "combat.ron.armour_classes.identities",
        list(
            field(&input.documents["combat.ron"], "armour_classes", "combat")?,
            "armour_classes",
        )?
        .iter()
        .map(|v| atom(v, "armour").map(str::to_owned))
        .collect::<AuditResult<Vec<_>>>()?,
        d.armour_classes().to_vec(),
        [
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
        ]
        .into_iter()
        .map(|v| e(v.into(), Deps::new()))
        .collect(),
        Deps::new(),
    )?;
    for (ln, key, buildings) in [
        ("harvesters", "Harvester", false),
        ("refineries", "Refinery", true),
        ("refunds", "Soylent", true),
    ] {
        let mut expected: BTreeMap<String, Deps> = BTreeMap::new();
        let mut length = Deps::new();
        for row in input.mappings.iter().filter(|v| v.building == buildings) {
            let raw = super::actual::optional(
                r,
                &row.section,
                key,
                if key == "Soylent" { "0" } else { "no" },
            )?;
            let parsed = converted(
                raw,
                if key == "Soylent" {
                    Conversion::Int
                } else {
                    Conversion::Flag
                },
            )?;
            let deps = dependency(r, &parsed.deps)?;
            length.extend(deps.clone());
            if parsed.value != 0 {
                expected
                    .entry(row.game_id.clone())
                    .or_default()
                    .extend(deps);
            }
        }
        let loaded = if !buildings {
            d.units()
                .iter()
                .filter_map(|u| {
                    d.unit_kind(&u.id)
                        .filter(|k| d.harvester(*k).is_some())
                        .map(|_| u.id.clone())
                })
                .collect()
        } else {
            d.buildings()
                .iter()
                .filter(|b| {
                    if key == "Refinery" {
                        b.credits_per_ore.is_some() && b.credits_per_gem.is_some()
                    } else {
                        b.fixed_refund.is_some()
                    }
                })
                .map(|v| v.id.clone())
                .collect()
        };
        ledger.compare(
            r,
            &format!("economy.ron.{ln}.identities"),
            ids(
                input,
                "economy.ron",
                ln,
                if buildings { "building" } else { "unit" },
            )?,
            loaded,
            expected.into_iter().map(|(v, d)| e(v, d)).collect(),
            length,
        )?;
    }
    ledger.compare(
        r,
        "rules.ron.prerequisite_groups.identities",
        ids(input, "rules.ron", "prerequisite_groups", "id")?,
        rules
            .prerequisite_groups
            .iter()
            .map(|v| v.id.clone())
            .collect(),
        rules::GROUPS
            .iter()
            .map(|v| e(v.2.to_string(), Deps::new()))
            .collect(),
        Deps::new(),
    )?;
    ledger.compare(
        r,
        "rules.ron.sides.identities",
        ids(input, "rules.ron", "sides", "id")?,
        rules.sides.iter().map(|v| v.side.id().into()).collect(),
        ["allied", "soviet"]
            .into_iter()
            .map(|v| e(v.into(), Deps::new()))
            .collect(),
        Deps::new(),
    )?;
    ledger.compare(
        r,
        "skirmish.ron.sides.identities",
        ids(input, "skirmish.ron", "sides", "side")?,
        s.sides.iter().map(|v| v.side.id().into()).collect(),
        ["allied", "soviet"]
            .into_iter()
            .map(|v| e(v.into(), Deps::new()))
            .collect(),
        Deps::new(),
    )?;
    let mut expected = Vec::new();
    let mut length = Deps::new();
    for (native, game, _) in POWERUPS {
        let tokens = values(r, "Powerups", native)?;
        if !matches!(tokens.len(), 3 | 4) {
            return Err("Powerup source shape changed".into());
        }
        let dep = SourcePart::Csv {
            key: SourceKey::new("rules", "Powerups", native),
            index: 0,
            exact_len: tokens.len(),
        };
        let parsed = converted(
            r.component(SourceKey::new("rules", "Powerups", native), 0)?,
            Conversion::Int,
        )?
        .value;
        let deps = BTreeSet::from([dep]);
        length.extend(deps.clone());
        if parsed > 0 {
            expected.push(e(game.into(), deps));
        }
    }
    ledger.compare(
        r,
        "rules.ron.powerups.identities",
        ids(input, "rules.ron", "powerups", "id")?,
        rules.powerups.iter().map(|p| p.id.clone()).collect(),
        expected,
        length,
    )?;
    Ok(())
}
fn declare_powerup_fields(ledger: &mut Ledger, input: &Inputs) -> AuditResult<()> {
    let r = &input.reference;
    for (native, game, conversion) in POWERUPS {
        let tokens = values(r, "Powerups", native)?;
        if !matches!(tokens.len(), 3 | 4) {
            return Err("Powerup tuple size".into());
        }
        let share = tokens[0].parse::<i128>().map_err(|e| e.to_string())?;
        if share <= 0 {
            continue;
        }
        let mut demands = vec![
            (
                Element::Member(format!("id:{game}")),
                csv("Powerups", native, 0, tokens.len()),
            ),
            (
                Element::Member(format!("share:{share}")),
                csv("Powerups", native, 0, tokens.len()),
            ),
        ];
        let (value, deps) = if let Some(conversion) = conversion {
            if tokens.len() != 4 {
                return Err("native Powerup argument missing".into());
            }
            (
                converted(Expected::literal(tokens[3].clone()), conversion)?.value,
                csv("Powerups", native, 3, 4),
            )
        } else {
            if tokens.len() == 4 {
                return Err(
                    "unresolved argument for nominally argumentless enabled Powerup".into(),
                );
            }
            (0, Deps::new())
        };
        demands.push((Element::Member(format!("value:{value}")), deps));
        ledger.declare(
            format!("powerup.{game}/rules.ron.powerups.fields"),
            Semantics::UniqueSet,
            demands,
            csv("Powerups", native, 0, tokens.len()),
        )?;
    }
    Ok(())
}
fn compare_powerup_fields(ledger: &mut Ledger, input: &Inputs, d: &Definitions) -> AuditResult<()> {
    let r = &input.reference;
    let rules = d.rules().ok_or("actual Rules")?;
    for (native, game, conversion) in POWERUPS {
        let tokens = values(r, "Powerups", native)?;
        let share_dep = SourcePart::Csv {
            key: SourceKey::new("rules", "Powerups", native),
            index: 0,
            exact_len: tokens.len(),
        };
        let share = converted(
            r.component(SourceKey::new("rules", "Powerups", native), 0)?,
            Conversion::Int,
        )?
        .value;
        if share <= 0 {
            continue;
        }
        let deps = BTreeSet::from([share_dep]);
        let expected_value = if let Some(conversion) = conversion {
            let dep = SourcePart::Csv {
                key: SourceKey::new("rules", "Powerups", native),
                index: 3,
                exact_len: 4,
            };
            let value = converted(
                r.component(SourceKey::new("rules", "Powerups", native), 3)?,
                conversion,
            )?
            .value;
            e(format!("value:{value}"), BTreeSet::from([dep]))
        } else {
            e("value:0".into(), Deps::new())
        };
        let expected = vec![
            e(format!("id:{game}"), deps.clone()),
            e(format!("share:{share}"), deps.clone()),
            expected_value,
        ];
        let actual = rules
            .powerups
            .iter()
            .find(|v| v.id == game)
            .ok_or("actual enabled Powerup missing")?;
        let raw = entry(input, "rules.ron", "powerups", "id", game)?;
        let raw_values = tuple(raw, "Powerup tuple")?
            .iter()
            .map(|(k, v)| Ok(format!("{k}:{}", atom(v, k)?)))
            .collect::<AuditResult<Vec<_>>>()?;
        ledger.compare(
            r,
            &format!("powerup.{game}/rules.ron.powerups.fields"),
            raw_values,
            vec![
                format!("id:{}", actual.id),
                format!("share:{}", actual.share),
                format!("value:{}", actual.value),
            ],
            expected,
            deps,
        )?;
    }
    Ok(())
}
// The own-policy catalogue is an explicit provisional game contract. It proves unchanged
// actual loader projections, never native tactical/map semantics or reference-key coverage.
fn declare_own(ledger: &mut Ledger) -> AuditResult<()> {
    for (id, values) in own_policy()? {
        ledger.declare(
            id.clone(),
            Semantics::Ordered,
            (0..values.len())
                .map(|n| (Element::Index(n), Deps::new()))
                .collect(),
            Deps::new(),
        )?;
        if let Some(origins) = own_origin(&id, &values)? {
            ledger.declare_origins(&id, origins)?;
        }
    }
    for (_, id) in rules::HOUSES {
        for field in ["buildings", "units"] {
            ledger.declare(
                format!("skirmish.faction.{id}.{field}"),
                Semantics::Ordered,
                Vec::new(),
                Deps::new(),
            )?;
        }
    }
    Ok(())
}
fn compare_own(
    ledger: &mut Ledger,
    input: &Inputs,
    d: &Definitions,
    s: &SkirmishDefinitions,
) -> AuditResult<()> {
    let r = &input.reference;
    let policy = own_policy()?;
    let ai = sim::load_ai(input.data_dir.join("ai.ron"), d)
        .map_err(|e| format!("actual AI structural projection: {e}"))?;
    let rules = d.rules().ok_or("actual country identities")?;
    let raw_plans = list(field(&input.documents["ai.ron"], "plans", "ai")?, "plans")?;
    let mut actual: BTreeMap<String, (Vec<String>, Vec<String>)> = BTreeMap::new();
    let raw_identity = raw_plans
        .iter()
        .map(|p| {
            Ok(format!(
                "{}:{}",
                atom(field(p, "side", "plan")?, "side")?,
                atom(field(p, "country", "plan")?, "country")?
            ))
        })
        .collect::<AuditResult<Vec<_>>>()?;
    let loaded_identity = ai
        .plans
        .iter()
        .map(|p| {
            let country = p
                .country
                .map(|c| {
                    rules
                        .country(c)
                        .map(|c| c.id.clone())
                        .ok_or("actual plan country")
                })
                .transpose()?
                .unwrap_or_default();
            Ok(format!("{}:{country}", p.side.map_or("none", |s| s.id())))
        })
        .collect::<AuditResult<Vec<_>>>()?;
    actual.insert(
        "ai.ron.plans.identities".into(),
        (raw_identity, loaded_identity),
    );
    actual.insert(
        "units.ron.units.identities.order".into(),
        (
            ids(input, "units.ron", "units", "id")?,
            d.units().iter().map(|u| u.id.clone()).collect(),
        ),
    );
    if raw_plans.len() != ai.plans.len() {
        return Err("raw/loaded AI plan cardinality".into());
    }
    for (n, p) in ai.plans.iter().enumerate() {
        let raw = &raw_plans[n];
        for (name, loaded) in [
            (
                "build_order",
                p.build_order
                    .iter()
                    .map(|k| super::rules::building_id(d, *k))
                    .collect::<AuditResult<Vec<_>>>()?,
            ),
            (
                "army",
                p.army
                    .iter()
                    .map(|k| super::rules::unit_id(d, *k))
                    .collect::<AuditResult<Vec<_>>>()?,
            ),
            (
                "defences",
                p.defences
                    .iter()
                    .map(|k| super::rules::building_id(d, *k))
                    .collect::<AuditResult<Vec<_>>>()?,
            ),
            (
                "support",
                p.support
                    .iter()
                    .map(|k| super::rules::unit_id(d, *k))
                    .collect::<AuditResult<Vec<_>>>()?,
            ),
            (
                "tier_reserves",
                p.tier_reserves.iter().map(u32::to_string).collect(),
            ),
        ] {
            let raw_values = list(field(raw, name, "AI plan")?, name)?
                .iter()
                .map(|v| atom(v, name).map(str::to_owned))
                .collect::<AuditResult<Vec<_>>>()?;
            actual.insert(format!("ai.ron.plans[{n}].{name}"), (raw_values, loaded));
        }
    }
    actual.insert(
        "ai.ron.difficulties.identities".into(),
        (
            ids(input, "ai.ron", "difficulties", "id")?,
            ai.difficulties.iter().map(|d| d.id.clone()).collect(),
        ),
    );
    actual.insert(
        "skirmish.ron.maps.identities".into(),
        (
            ids(input, "skirmish.ron", "maps", "id")?,
            s.maps.iter().map(|m| m.id.clone()).collect(),
        ),
    );
    for map in &s.maps {
        let raw = entry(input, "skirmish.ron", "maps", "id", &map.id)?;
        let starts = list(field(raw, "starts", "map")?, "starts")?;
        let mut raw_values = Vec::new();
        for start in starts {
            super::raw::exact_fields(start, &["x", "y"], "map start tuple")?;
            for key in ["x", "y"] {
                raw_values.push(atom(field(start, key, "start")?, key)?.to_string());
            }
        }
        let loaded = map
            .starts
            .iter()
            .flat_map(|p| [p.x.to_string(), p.y.to_string()])
            .collect::<Vec<_>>();
        let id = format!("skirmish.map.{}.starts.xy", map.id);
        let origins = (0..map.starts.len() * 2)
            .map(|n| {
                origin(
                    "skirmish.ron",
                    "maps",
                    &map.id,
                    &format!("starts[{}].{}", n / 2, if n % 2 == 0 { "x" } else { "y" }),
                    None,
                )
            })
            .collect();
        ledger.observe_origins(&id, origins)?;
        actual.insert(id, (raw_values, loaded));
    }
    if actual.keys().collect::<BTreeSet<_>>() != policy.keys().collect::<BTreeSet<_>>() {
        return Err("explicit own array policy and actual fields/identities differ".into());
    }
    for (id, (raw, loaded)) in actual {
        let values = policy.get(&id).ok_or("missing own array policy")?;
        ledger.compare(
            r,
            &id,
            raw,
            loaded,
            values.iter().map(|v| e(v.clone(), Deps::new())).collect(),
            Deps::new(),
        )?;
    }
    for (_, id) in rules::HOUSES {
        let f = s
            .factions
            .iter()
            .find(|f| f.id == id)
            .ok_or("actual native country faction")?;
        for name in ["buildings", "units"] {
            let loaded = if name == "buildings" {
                if !f.buildings.is_empty() {
                    return Err("native country has legacy starting buildings".into());
                }
                Vec::new()
            } else {
                if !f.units.is_empty() {
                    return Err("native country has legacy starting units".into());
                }
                Vec::new()
            };
            ledger.compare(
                r,
                &format!("skirmish.faction.{id}.{name}"),
                strings(input, "skirmish.ron", "factions", "id", id, name)?,
                loaded,
                Vec::new(),
                Deps::new(),
            )?;
        }
    }
    super::country_rosters::raw_roster_schema(input, d, s)?;
    super::country_rosters::compare(input, d, s)?;
    Ok(())
}
fn legacy_dispatch_guard(d: &Definitions) -> AuditResult<()> {
    // Exact current absence, not an invented generic native implementation. Role::Strike
    // consumes cooldown after burst_end in resolve; Role::Carrier consumes capacity in
    // carrier::launch_fighters, which searches a Role::Strike. Neither exists in shipped data.
    let actual = d
        .arsenal()
        .0
        .iter()
        .filter(|(_, m)| matches!(m.role, sim::Role::Strike | sim::Role::Carrier))
        .map(|(item, m)| format!("{item:?}/{:?}", m.role))
        .collect::<Vec<_>>();
    if !actual.is_empty() {
        return Err(format!(
            "legacy Strike/Carrier native policy unresolved for actual items: {actual:?}; Strike: rearming cooldown after burst; Carrier: capacity and first Strike definition"
        ));
    }
    for id in ["aircraft_carrier", "dreadnought", "destroyer"] {
        let kind = d.unit_kind(id).ok_or("actual native spawner missing")?;
        let m = d
            .metadata(sim::ProductionItem::Unit(kind))
            .ok_or("native spawner metadata")?;
        if m.role != sim::Role::Combat || m.spawns.is_none() || m.launcher.is_none() {
            return Err(format!(
                "actual native flight path no longer independent of legacy Carrier role: {id}"
            ));
        }
    }
    for id in ["harrier", "black_eagle"] {
        let kind = d.unit_kind(id).ok_or("actual native strike aircraft")?;
        let m = d
            .metadata(sim::ProductionItem::Unit(kind))
            .ok_or("native strike metadata")?;
        if m.role != sim::Role::Combat || !m.airport_bound || m.ammo == 0 {
            return Err(format!(
                "actual native aircraft path no longer independent of legacy Strike: {id}"
            ));
        }
    }
    Ok(())
}
pub(super) fn run(input: &Inputs, d: &Definitions) -> AuditResult<()> {
    family_guard(input)?;
    let s = sim::load_skirmish(input.data_dir.join("skirmish.ron"), d)
        .map_err(|e| format!("actual nested skirmish setup: {e}"))?;
    let mut ledger = Ledger::default();
    declare_native(&mut ledger, input)?;
    declare_registries(&mut ledger, input)?;
    declare_powerup_fields(&mut ledger, input)?;
    declare_own(&mut ledger)?;
    compare_native(&mut ledger, input, d, &s)?;
    compare_registries(&mut ledger, input, d, &s)?;
    compare_powerup_fields(&mut ledger, input, d)?;
    compare_own(&mut ledger, input, d, &s)?;
    legacy_dispatch_guard(d)?;
    ledger.finish()
}

// Exhaustive family routing guard. This is an obligation guard, not a successful component
// witness. Each owning ledger still has to finish and all old actual scalar checks still run.
fn family_guard(input: &Inputs) -> AuditResult<()> {
    fn walk(node: &Raw, path: &str, families: &mut BTreeSet<String>) -> AuditResult<()> {
        match node {
            Raw::Atom(_) => {}
            Raw::Tuple(fields) => {
                for (name, value) in fields {
                    walk(value, &format!("{path}.{name}"), families)?;
                }
            }
            Raw::List(values) => {
                let mut normalized = String::new();
                let mut chars = path.chars().peekable();
                while let Some(c) = chars.next() {
                    if c == '[' {
                        normalized.push_str("[*]");
                        let mut closed = false;
                        for n in chars.by_ref() {
                            if n == ']' {
                                closed = true;
                                break;
                            }
                            if !n.is_ascii_digit() {
                                return Err("non-index raw list path".into());
                            }
                        }
                        if !closed {
                            return Err("unclosed raw list index".into());
                        }
                    } else {
                        normalized.push(c);
                    }
                }
                families.insert(normalized);
                for (n, value) in values.iter().enumerate() {
                    walk(value, &format!("{path}[{n}]"), families)?;
                }
            }
        }
        Ok(())
    }
    let mut expected = BTreeSet::new();
    let mut rows = include_str!("list-families.tsv").lines();
    if rows.next() != Some("family\tsemantics\texact_witness_route\tnative_coverage_granted") {
        return Err("array family routing table header".into());
    }
    for row in rows {
        let parts = row.split('\t').collect::<Vec<_>>();
        if parts.len() != 4
            || parts[1].is_empty()
            || parts[2].is_empty()
            || parts[3] != "none"
            || !expected.insert(parts[0].to_string())
        {
            return Err(
                "missing/duplicate family contract or fictional tracker native coverage".into(),
            );
        }
    }
    let mut actual = BTreeSet::new();
    for (file, raw) in &input.documents {
        walk(raw, file, &mut actual)?;
    }
    if actual != expected {
        return Err(format!(
            "actual nested list families have missing/stale routes: missing{:?} stale{:?}",
            actual.difference(&expected).collect::<Vec<_>>(),
            expected.difference(&actual).collect::<Vec<_>>()
        ));
    }
    Ok(())
}
