//! Derived-country oracle, UNCOMPILED/UNRUN. No call to country_tree/check_buildable/
//! produces/missing_prerequisite/same_factory or any generator implementation.
//! All source configuration dependencies here are supplemental, zero global source coverage.
use super::arrays::{Deps, SourcePart};
use super::*;
#[derive(Clone)]
struct Candidate {
    id: String,
    building: bool,
    side: String,
    tech: i32,
    owners: BTreeSet<String>,
    required: BTreeSet<String>,
    forbidden: BTreeSet<String>,
    stolen: bool,
    prereqs: BTreeSet<String>,
    groups: BTreeSet<String>,
    factory: Option<String>,
    wanted: String,
    airport: bool,
    docks: u32,
}
struct Model {
    items: Vec<Candidate>,
    groups: BTreeMap<String, BTreeSet<String>>,
    base: Vec<String>,
    seeds: Vec<String>,
    max_tech: i32,
    side: String,
    deps: Deps,
}
fn csv_values(s: &str) -> AuditResult<Vec<String>> {
    if s.is_empty() {
        return Ok(Vec::new());
    }
    let p = s
        .split(',')
        .map(str::trim)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if p.iter().any(String::is_empty) || p.iter().collect::<BTreeSet<_>>().len() != p.len() {
        return Err("roster source config contains empty/duplicate set member".into());
    }
    Ok(p)
}
fn yes(s: &str) -> AuditResult<bool> {
    match s {
        "yes" | "true" | "1" => Ok(true),
        "no" | "false" | "0" => Ok(false),
        v => Err(format!("invalid roster flag {v}")),
    }
}
fn read(
    r: &Reference,
    deps: &mut Deps,
    section: &str,
    key: &str,
    default: &str,
    parts: bool,
) -> AuditResult<String> {
    let k = SourceKey::new("rules", section, key);
    let text = r.rows.get(&k).cloned().unwrap_or_else(|| default.into());
    if r.rows.contains_key(&k) {
        deps.insert(SourcePart::Whole(k.clone()));
        if parts {
            let v = csv_values(&text)?;
            for n in 0..v.len() {
                deps.insert(SourcePart::Csv {
                    key: k.clone(),
                    index: n,
                    exact_len: v.len(),
                });
            }
        }
    }
    Ok(text)
}
fn native_id(input: &Inputs, section: &str) -> AuditResult<String> {
    let found = input
        .mappings
        .iter()
        .filter(|m| m.section == section)
        .map(|m| m.game_id.clone())
        .collect::<BTreeSet<_>>();
    if found.len() != 1 {
        return Err(format!("roster native item identity unresolved {section}"));
    }
    found
        .into_iter()
        .next()
        .ok_or("roster mapping empty".into())
}
fn group_value(factory: &str, naval: bool) -> AuditResult<Option<String>> {
    Ok(match factory {
        "" => None,
        "BuildingType" => Some("buildings".into()),
        "InfantryType" => Some("infantry".into()),
        "AircraftType" => Some("aircraft".into()),
        "UnitType" => Some(if naval { "naval" } else { "vehicles" }.into()),
        v => return Err(format!("unknown native factory {v}")),
    })
}
// Source fact decoding is eager and explicit: the derived set is a function of the entire
// checked current buildability configuration. This is NOT runtime short-circuit flag coverage.
fn source_model(input: &Inputs, house: &str) -> AuditResult<Model> {
    let r = &input.reference;
    let mut deps = Deps::new();
    let side = match read(r, &mut deps, house, "Side", "", false)?.as_str() {
        "GDI" => "allied",
        "Nod" => "soviet",
        _ => return Err("roster country side unresolved".into()),
    }
    .to_string();
    let max_tech = read(
        r,
        &mut deps,
        "MultiplayerDialogSettings",
        "TechLevel",
        "",
        false,
    )?
    .parse::<i32>()
    .map_err(|e| e.to_string())?;
    let base = csv_values(&read(r, &mut deps, "General", "BaseUnit", "", true)?)?;
    if base.len() != 2 {
        return Err("country oracle requires exact two side BaseUnit identities".into());
    }
    let mut seeds = Vec::new();
    for b in &base {
        let yard = read(r, &mut deps, b, "DeploysInto", "", false)?;
        seeds.push(native_id(input, &yard)?);
    }
    let mut houses = BTreeMap::new();
    for (native, side) in [("GDI", "allied"), ("Nod", "soviet")] {
        for h in csv_values(&read(r, &mut deps, "Sides", native, "", true)?)? {
            if houses.insert(h, side.to_string()).is_some() {
                return Err("source house belongs to two sides".into());
            }
        }
    }
    let pads = csv_values(&read(r, &mut deps, "General", "PadAircraft", "", true)?)?;
    let infantry = r
        .rows
        .iter()
        .filter(|(k, _)| k.file == "rules" && k.section == "InfantryTypes")
        .map(|(k, v)| {
            deps.insert(SourcePart::Whole(k.clone()));
            v.clone()
        })
        .collect::<BTreeSet<_>>();
    let mut groups = BTreeMap::new();
    for (token, key, _) in super::rules::GROUPS {
        let members = csv_values(&read(r, &mut deps, "General", key, "", true)?)?
            .iter()
            .map(|s| native_id(input, s))
            .collect::<AuditResult<BTreeSet<_>>>()?;
        if members.is_empty() {
            return Err("empty native prerequisite group".into());
        }
        groups.insert(token.to_string(), members);
    }
    let mut facts = BTreeMap::new();
    for row in &input.mappings {
        let mut f = BTreeMap::new();
        for (key, default, parts) in [
            ("TechLevel", "", false),
            ("Owner", "", true),
            ("RequiredHouses", "", true),
            ("ForbiddenHouses", "", true),
            ("RequiresStolenAlliedTech", "no", false),
            ("RequiresStolenSovietTech", "no", false),
            ("Prerequisite", "", true),
            ("Factory", "", false),
            ("Naval", "no", false),
            ("AirportBound", "no", false),
            ("Dock", "", true),
            ("NumberOfDocks", "0", false),
        ] {
            f.insert(key, read(r, &mut deps, &row.section, key, default, parts)?);
        }
        facts.insert(row.section.clone(), f);
    }
    let mut dock_sections = BTreeSet::new();
    for row in input.mappings.iter().filter(|m| !m.building) {
        let f = &facts[&row.section];
        if yes(&f["AirportBound"])? {
            dock_sections.extend(csv_values(&f["Dock"])?);
        }
    }
    let mut items = Vec::new();
    for row in input.mappings.iter().filter(|m| m.note != "twin") {
        let f = &facts[&row.section];
        let owners = csv_values(&f["Owner"])?
            .into_iter()
            .map(|h| {
                houses
                    .get(&h)
                    .cloned()
                    .ok_or_else(|| format!("unresolved source Owner {h}"))
            })
            .collect::<AuditResult<BTreeSet<_>>>()?;
        let countries = |key: &str| -> AuditResult<BTreeSet<String>> {
            csv_values(&f[key])?
                .into_iter()
                .map(|h| {
                    super::rules::HOUSES
                        .iter()
                        .find(|(native, _)| *native == h)
                        .map(|(_, id)| id.to_string())
                        .ok_or_else(|| format!("unknown country restriction {h}"))
                })
                .collect()
        };
        let mut required = countries("RequiredHouses")?;
        let mut forbidden = countries("ForbiddenHouses")?;
        // Named accepted pre-existing merged Engineer/Dog projection, not a generic waiver.
        if matches!(row.game_id.as_str(), "engineer" | "attack_dog")
            && input
                .mappings
                .iter()
                .filter(|m| m.game_id == row.game_id)
                .count()
                == 2
        {
            for other in input.mappings.iter().filter(|m| m.game_id == row.game_id) {
                let value = &facts[&other.section]["ForbiddenHouses"];
                let count = input
                    .differences
                    .iter()
                    .filter(|v| {
                        v.id.game_id == row.game_id
                            && v.id.field == "forbidden_countries"
                            && v.id.reference_value == *value
                            && v.id.game_value.is_empty()
                    })
                    .count();
                if count != 1 {
                    return Err(
                        "merged roster-country restriction lacks exact named existing difference"
                            .into(),
                    );
                }
            }
            required.clear();
            forbidden.clear();
        }
        let mut prereqs = BTreeSet::new();
        let mut needed = BTreeSet::new();
        for token in csv_values(&f["Prerequisite"])? {
            let t = token.to_ascii_uppercase();
            if groups.contains_key(&t) {
                needed.insert(t);
            } else {
                prereqs.insert(native_id(input, &token)?);
            }
        }
        let naval = yes(&f["Naval"])?;
        let factory = if row.building {
            group_value(&f["Factory"], naval)?
        } else {
            None
        };
        let wanted = if row.building {
            "buildings"
        } else if infantry.contains(&row.section) {
            "infantry"
        } else if naval {
            "naval"
        } else if pads.contains(&row.section) {
            "aircraft"
        } else {
            "vehicles"
        }
        .to_string();
        let docks = if row.building && dock_sections.contains(&row.section) {
            f["NumberOfDocks"]
                .parse::<u32>()
                .map_err(|e| e.to_string())?
        } else {
            0
        };
        let sides = input
            .mappings
            .iter()
            .filter(|m| m.game_id == row.game_id)
            .map(|m| m.side.clone())
            .collect::<BTreeSet<_>>();
        let mapped_side = if sides.len() == 1 {
            row.side.clone()
        } else {
            "none".into()
        };
        items.push(Candidate {
            id: row.game_id.clone(),
            building: row.building,
            side: mapped_side,
            tech: f["TechLevel"].parse::<i32>().map_err(|e| e.to_string())?,
            owners,
            required,
            forbidden,
            stolen: yes(&f["RequiresStolenAlliedTech"])? || yes(&f["RequiresStolenSovietTech"])?,
            prereqs,
            groups: needed,
            factory,
            wanted,
            airport: yes(&f["AirportBound"])?,
            docks,
        });
    }
    items.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Model {
        items,
        groups,
        base,
        seeds,
        max_tech,
        side,
        deps,
    })
}
// Independently enumerate whole + CSV configuration input obligations. This deliberately
// duplicates the source-model selector catalogue, never harvests its successful dependency set.
fn demand_inputs(input: &Inputs, house: &str) -> AuditResult<Deps> {
    let r = &input.reference;
    let mut demands = Deps::new();
    let mut names = Vec::<(String, String, bool)>::new();
    for (s, k, parts) in [
        (house, "Side", false),
        ("MultiplayerDialogSettings", "TechLevel", false),
        ("General", "BaseUnit", true),
        ("General", "PadAircraft", true),
        ("Sides", "GDI", true),
        ("Sides", "Nod", true),
    ] {
        names.push((s.into(), k.into(), parts));
    }
    if let Some(base) = r.rows.get(&SourceKey::new("rules", "General", "BaseUnit")) {
        for native in csv_values(base)? {
            names.push((native, "DeploysInto".into(), false));
        }
    }
    for (_, key, _) in super::rules::GROUPS {
        names.push(("General".into(), key.into(), true));
    }
    for row in &input.mappings {
        for key in [
            "TechLevel",
            "Owner",
            "RequiredHouses",
            "ForbiddenHouses",
            "RequiresStolenAlliedTech",
            "RequiresStolenSovietTech",
            "Prerequisite",
            "Factory",
            "Naval",
            "AirportBound",
            "Dock",
            "NumberOfDocks",
        ] {
            names.push((
                row.section.clone(),
                key.into(),
                matches!(
                    key,
                    "Owner" | "RequiredHouses" | "ForbiddenHouses" | "Prerequisite" | "Dock"
                ),
            ));
        }
    }
    for key in r
        .rows
        .keys()
        .filter(|k| k.file == "rules" && k.section == "InfantryTypes")
    {
        demands.insert(SourcePart::Whole(key.clone()));
    }
    for (s, k, parts) in names {
        let key = SourceKey::new("rules", &s, &k);
        if let Some(value) = r.rows.get(&key) {
            demands.insert(SourcePart::Whole(key.clone()));
            if parts {
                let len = csv_values(value)?.len();
                for index in 0..len {
                    demands.insert(SourcePart::Csv {
                        key: key.clone(),
                        index,
                        exact_len: len,
                    });
                }
            }
        }
    }
    Ok(demands)
}
fn allowed(m: &Model, owned: &BTreeSet<String>, f: &Candidate, country: &str) -> bool {
    f.tech >= 0
        && f.tech <= m.max_tech
        && !f.stolen
        && (f.required.is_empty() || f.required.contains(country))
        && !f.forbidden.contains(country)
        && f.prereqs.iter().all(|p| owned.contains(p))
        && f.groups
            .iter()
            .all(|g| m.groups[g].iter().any(|id| owned.contains(id)))
        && m.items.iter().any(|producer| {
            owned.contains(&producer.id)
                && producer.factory.as_ref() == Some(&f.wanted)
                && (f.owners.is_empty() || f.owners.contains(&producer.side))
        })
        && (!f.airport
            || m.items
                .iter()
                .any(|dock| owned.contains(&dock.id) && dock.docks > 0))
}
fn expected(m: &Model, country: &str) -> AuditResult<BTreeSet<String>> {
    if m.base.len() != 2 || m.seeds.len() != 2 {
        return Err("exact side seeds lost".into());
    }
    let seed = &m.seeds[if m.side == "allied" { 0 } else { 1 }];
    let mut owned = BTreeSet::from([seed.clone()]);
    let mut result = BTreeSet::new();
    // Monotone fixed point in independent lexical-id order, not production BuildingKind order.
    loop {
        let mut added = false;
        for f in m.items.iter().filter(|f| f.building) {
            if !owned.contains(&f.id) && allowed(m, &owned, f, country) {
                owned.insert(f.id.clone());
                result.insert(format!("building:{}", f.id));
                added = true;
            }
        }
        if !added {
            break;
        }
    }
    for f in m.items.iter().filter(|f| !f.building) {
        if allowed(m, &owned, f, country) {
            result.insert(format!("unit:{}", f.id));
        }
    }
    if result.is_empty() {
        return Err("unexpected empty derived country roster".into());
    }
    Ok(result)
}
// Separate simultaneous-layer declaration, independent of the successful sequential
// expected-expression traversal. Both use source-decoded facts, never actual loaded rosters.
fn declared_members(m: &Model, country: &str) -> AuditResult<BTreeSet<String>> {
    let seed = m
        .seeds
        .get(if m.side == "allied" { 0 } else { 1 })
        .ok_or("independent source seed")?;
    let mut known = BTreeSet::from([seed.clone()]);
    let mut members = BTreeSet::new();
    let admits = |f: &Candidate, known: &BTreeSet<String>| -> bool {
        if f.tech < 0 || f.tech > m.max_tech || f.stolen {
            return false;
        }
        if !f.required.is_empty() && !f.required.contains(country) || f.forbidden.contains(country)
        {
            return false;
        }
        if !f.prereqs.is_subset(known) {
            return false;
        }
        for group in &f.groups {
            if m.groups[group].is_disjoint(known) {
                return false;
            }
        }
        let producers = m
            .items
            .iter()
            .filter(|p| known.contains(&p.id) && p.factory.as_ref().is_some_and(|g| g == &f.wanted))
            .collect::<Vec<_>>();
        if !producers
            .iter()
            .any(|p| f.owners.is_empty() || f.owners.contains(&p.side))
        {
            return false;
        }
        if f.airport
            && !m
                .items
                .iter()
                .filter(|p| known.contains(&p.id))
                .any(|p| p.docks != 0)
        {
            return false;
        }
        true
    };
    loop {
        let additions = m
            .items
            .iter()
            .filter(|f| f.building && !known.contains(&f.id) && admits(f, &known))
            .map(|f| f.id.clone())
            .collect::<BTreeSet<_>>();
        if additions.is_empty() {
            break;
        }
        for id in additions {
            members.insert(format!("building:{id}"));
            known.insert(id);
        }
    }
    for f in m.items.iter().filter(|f| !f.building) {
        if admits(f, &known) {
            members.insert(format!("unit:{}", f.id));
        }
    }
    Ok(members)
}
pub(super) fn compare(input: &Inputs, d: &Definitions, s: &SkirmishDefinitions) -> AuditResult<()> {
    let rules = d.rules().ok_or("actual roster Rules absent")?;
    for (house, country) in super::rules::HOUSES {
        let model = source_model(input, house)?;
        let demanded = demand_inputs(input, house)?;
        if model.deps != demanded {
            return Err(format!(
                "country configured-source dependency inequality {country}"
            ));
        }
        for dep in &demanded {
            let _ = dep.read(&input.reference)?;
        }
        let declared = declared_members(&model, country)?;
        let want = expected(&model, country)?;
        // Explicit derived UniqueSet member and length contracts. Catalogue dependencies are
        // configuration provenance, not per-native-flag gameplay coverage or a physical raw array.
        let member_demands = declared
            .iter()
            .map(|id| (id.clone(), demanded.clone()))
            .collect::<BTreeMap<_, _>>();
        let successful_members = want
            .iter()
            .map(|id| (id.clone(), model.deps.clone()))
            .collect::<BTreeMap<_, _>>();
        if member_demands != successful_members || declared.len() != want.len() {
            return Err(format!(
                "derived roster member/length dependency contract mismatch {country}"
            ));
        }
        let f = s
            .factions
            .iter()
            .find(|f| f.id == country)
            .ok_or("actual derived faction missing")?;
        if f.country != rules.country_id(country) || f.side.map_or("none", |s| s.id()) != model.side
        {
            return Err("actual derived country/side identity differs".into());
        }
        let actual = f
            .roster
            .iter()
            .map(|i| match i {
                sim::ProductionItem::Building(k) => {
                    super::rules::building_id(d, *k).map(|id| format!("building:{id}"))
                }
                sim::ProductionItem::Unit(k) => {
                    super::rules::unit_id(d, *k).map(|id| format!("unit:{id}"))
                }
            })
            .collect::<AuditResult<BTreeSet<_>>>()?;
        if actual.len() != f.roster.len() || actual != want {
            return Err(format!(
                "derived country roster exact unique membership/length differs {country}: missing{:?};foreign{:?}",
                want.difference(&actual).collect::<Vec<_>>(),
                actual.difference(&want).collect::<Vec<_>>()
            ));
        }
    }
    Ok(())
}
// The raw roster lists are schema-only inputs, with no loaded field projection. A successful
// real load_skirmish supplies the branch validation; no fabricated loaded Vec::new witness.
pub(super) fn raw_roster_schema(
    input: &Inputs,
    d: &Definitions,
    s: &SkirmishDefinitions,
) -> AuditResult<()> {
    let mut required = BTreeSet::new();
    let mut witnessed = BTreeSet::new();
    for (_, country) in super::rules::HOUSES {
        for name in ["roster_buildings", "roster_units"] {
            required.insert((country.to_string(), name.to_string()));
        }
    }
    let rows = super::raw::list(
        super::raw::field(&input.documents["skirmish.ron"], "factions", "skirmish")?,
        "factions",
    )?;
    for (_, country) in super::rules::HOUSES {
        let f = s
            .factions
            .iter()
            .find(|f| f.id == country)
            .ok_or("loaded country branch missing")?;
        if f.side.is_none() || f.country != d.rules().and_then(|r| r.country_id(country)) {
            return Err(
                "raw legacy-empty requirement lacks actual validated native-country branch".into(),
            );
        }
        let raw = rows
            .iter()
            .filter(|v| {
                super::raw::field(v, "id", "faction")
                    .and_then(|v| super::raw::atom(v, "id"))
                    .is_ok_and(|v| v == country)
            })
            .collect::<Vec<_>>();
        if raw.len() != 1 {
            return Err("raw country identity absent/duplicate".into());
        }
        for name in ["roster_buildings", "roster_units"] {
            let list = super::raw::list(super::raw::field(raw[0], name, "faction")?, name)?;
            if !list.is_empty() {
                return Err(format!(
                    "actual raw legacy roster must be schema-empty {country}/{name}"
                ));
            }
            if !witnessed.insert((country.to_string(), name.to_string())) {
                return Err("duplicate raw roster schema proof".into());
            }
        }
    }
    if required != witnessed {
        return Err("missing/foreign raw roster schema obligations".into());
    }
    Ok(())
}

// Installed actual prepared negative block: integration-blockers-20261019-v1.UNVALIDATED.roster-negatives.rs.txt
// UNCOMPILED/UNRUN; append to country-rosters.rs. These use actual unchanged loaded data first.
#[cfg(test)]
mod tests {
    use super::*;
    fn control() -> AuditResult<(Inputs, Definitions, SkirmishDefinitions)> {
        let input = Inputs::load(
            &super::super::super::data_dir(),
            &super::super::super::reference_dir(),
        )?;
        let d = sim::load_ruleset(&input.data_dir).map_err(|e| e.to_string())?;
        let s = sim::load_skirmish(input.data_dir.join("skirmish.ron"), &d)
            .map_err(|e| e.to_string())?;
        raw_roster_schema(&input, &d, &s)?;
        compare(&input, &d, &s)?;
        Ok((input, d, s))
    }
    #[test]
    fn ra9_derived_roster_nonfirst_loaded_member_removal_is_detected() -> AuditResult<()> {
        let (input, d, mut s) = control()?;
        let f = s
            .factions
            .iter_mut()
            .find(|f| f.id == "great_britain")
            .ok_or("actual British faction")?;
        let nonfirst = f
            .roster
            .iter()
            .nth(1)
            .copied()
            .ok_or("actual derived nonfirst member")?;
        if !f.roster.remove(&nonfirst) {
            return Err("actual member not removed".into());
        }
        let e = compare(&input, &d, &s)
            .err()
            .ok_or("nonfirst actual roster member escaped")?;
        if !e.contains("derived country roster exact unique membership/length") {
            return Err(format!("wrong removal failure {e}"));
        }
        Ok(())
    }
    #[test]
    fn ra9_derived_roster_foreign_national_member_is_detected() -> AuditResult<()> {
        let (input, d, mut s) = control()?;
        let foreign = sim::ProductionItem::Unit(
            d.unit_kind("black_eagle")
                .ok_or("actual Korean national unit")?,
        );
        let f = s
            .factions
            .iter_mut()
            .find(|f| f.id == "great_britain")
            .ok_or("actual British faction")?;
        if f.roster.contains(&foreign) || !f.roster.insert(foreign) {
            return Err("foreign member premise not established".into());
        }
        assert!(compare(&input, &d, &s).is_err());
        Ok(())
    }
    #[test]
    fn ra9_derived_roster_valid_loader_country_reassociation_is_detected() -> AuditResult<()> {
        let _baseline = control()?;
        let copy = super::super::array_negatives::DataCopy::edit(
            "arsenal.ron",
            "(id: \"black_eagle\",",
            "required_countries",
            |v| {
                if v.as_slice() != ["\"korea\""] {
                    return Err("exact actual native Korean restriction".into());
                }
                v[0] = "\"america\"".into();
                Ok(())
            },
        )?;
        let (input, d) = copy.loaded()?;
        let s = sim::load_skirmish(input.data_dir.join("skirmish.ron"), &d)
            .map_err(|e| format!("MUTANT SKIRMISH LOADER FAILURE, not closure evidence: {e}"))?;
        let e = compare(&input, &d, &s)
            .err()
            .ok_or("changed actual native country tree escaped unchanged source")?;
        if !e.contains("derived country roster exact unique membership/length") {
            return Err(format!("wrong native country association failure {e}"));
        }
        Ok(())
    }
    #[test]
    fn ra9_raw_roster_nonempty_is_schema_rejection_not_fake_loaded_empty() -> AuditResult<()> {
        let _baseline = control()?;
        let copy = super::super::array_negatives::DataCopy::replace_row(
            "skirmish.ron",
            "(id: \"america\",",
            "roster_units: []",
            "roster_units: [\"grizzly\"]",
        )?;
        let (input, d) = copy.loaded()?;
        let e = sim::load_skirmish(input.data_dir.join("skirmish.ron"), &d)
            .err()
            .ok_or("native country raw roster unexpectedly accepted")?
            .to_string();
        if !e.contains("roster_units") {
            return Err(format!("wrong native raw-list rejection {e}"));
        }
        Ok(())
    }
}
