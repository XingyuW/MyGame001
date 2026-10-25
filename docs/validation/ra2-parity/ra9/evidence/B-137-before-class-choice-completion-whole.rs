//! Exact 81 active registry ordinals and 21 unit Soylent current-model policies.
//! Neither a registry read nor a loaded getter creates native source credit.
//! Completion admission/root choices/receipts are owned by completion.rs.
use super::*;
use super::completion::Spec;

const CONTRACTS: &str = include_str!("completion-whole-contracts.tsv");
const HEADER: &str = "file\tsection\tkey\texact_value\tgroup_id\tclassification\tspecific_reason\tconsumer_bindings\tmodel_guard_id\trequired_evidence_routes\tmapping_rows\tloaded_rows";

#[derive(Clone)]
struct Contract<'a> {
    key: SourceKey,
    exact: &'a str,
    group: &'a str,
    reason: &'a str,
    binding: &'a str,
    guard: &'a str,
    routes: BTreeSet<String>,
    mapping: &'a str,
    loaded: &'a str,
}
fn contracts(text: &str) -> AuditResult<Vec<Contract<'_>>> {
    let mut lines = text.lines();
    if lines.next() != Some(HEADER) {
        return Err("completion102 exact contract header".into());
    }
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    let mut counts = BTreeMap::new();
    for line in lines {
        let r = line.split('\t').collect::<Vec<_>>();
        if r.len() != 12 || r[5] != "CURRENT_MODEL_NONREAD" {
            return Err("completion102 contract width/class".into());
        }
        let key = SourceKey::new(r[0], r[1], r[2]);
        if r[0] != "rules" || !seen.insert(key.clone()) || r[6].is_empty()
            || r[6].starts_with("PENDING") || r[10].is_empty() || r[11].is_empty()
        {
            return Err("completion102 duplicate/unspecific/non-rules contract".into());
        }
        let routes = r[9].split(';').map(str::to_owned).collect::<BTreeSet<_>>();
        if routes.len() != 2 || routes.iter().any(|s| !s.starts_with("static/") && !s.starts_with("loaded/")) {
            return Err("completion102 exact static/loaded route scope".into());
        }
        *counts.entry(r[4]).or_insert(0usize) += 1;
        rows.push(Contract { key, exact: r[3], group: r[4], reason: r[6], binding: r[7],
            guard: r[8], routes, mapping: r[10], loaded: r[11] });
    }
    let wanted = [("registry-building",46usize),("registry-vehicle",28),("registry-aircraft",7),("unit-soylent",21)]
        .into_iter().collect::<BTreeMap<_,_>>();
    if counts != wanted || rows.len() != 102 {
        return Err("completion102 independent46/28/7/21 census".into());
    }
    Ok(rows)
}
fn code_guard() -> AuditResult<()> {
    // Exact original source bytes are a drift guard, not authentication/native execution.
    // Root authenticates the SHA/source Scope independently before materialization.
    for (path, source, expected) in [
        ("scripts/generate_ra2_data.py", include_str!("../../../../scripts/generate_ra2_data.py"), 0xa7ce0c09a609cad9u64),
        ("crates/sim/src/command.rs", include_str!("../../src/command.rs"), 0xfa6e7f66569e6b1fu64),
        ("crates/sim/src/arsenal.rs", include_str!("../../src/arsenal.rs"), 0x40d7f3082d74ff37u64),
        ("crates/sim/src/economy_defs.rs", include_str!("../../src/economy_defs.rs"), 0x72658060a6fb85c8u64),
        ("crates/sim/src/veterancy.rs", include_str!("../../src/veterancy.rs"), 0x53186583ba89d820u64),
        ("crates/sim/src/special/interaction.rs", include_str!("../../src/special/interaction.rs"), 0xc80ce6fa4483e553u64),
    ] {
        let mut h = sim::Fnv1a64::new();
        h.write(source.as_bytes());
        if h.finish() != expected {
            return Err(format!("completion102 current roster/unit-refund code policy changed: {path}"));
        }
    }
    Ok(())
}
fn actual_keyset(input: &Inputs) -> BTreeSet<SourceKey> {
    let units = input.mappings.iter().filter(|m| !m.building)
        .map(|m| m.section.as_str()).collect::<BTreeSet<_>>();
    input.reference.rows.keys().filter(|key| key.file == "rules"
        && (matches!(key.section.as_str(), "BuildingTypes" | "VehicleTypes" | "AircraftTypes")
            || (key.key == "Soylent" && units.contains(key.section.as_str()))))
        .cloned().collect()
}
fn normalized(value: &str) -> String {
    value.chars().filter(|c| *c != '_').flat_map(char::to_lowercase).collect()
}
fn source_section<'a>(c: &'a Contract<'_>) -> &'a str {
    if c.group == "unit-soylent" { &c.key.section } else { c.exact }
}
fn observe(input: &Inputs, d: &Definitions, c: &Contract<'_>) -> AuditResult<String> {
    if d.rules().is_none() || input.reference.shapes.contains_key(&c.key)
        || input.reference.rows.get(&c.key).map(String::as_str) != Some(c.exact)
    {
        return Err(format!("completion102 exact Whole/Rules-present source changed {:?}", c.key));
    }
    let section = source_section(c);
    let mut actual_mapping = Vec::new();
    let mut actual_loaded = Vec::new();
    for m in input.mappings.iter().filter(|m| m.section == section) {
        actual_mapping.push(format!("{}|{}|{}|{}|{}", m.game_id,
            if m.building { "building" } else { "unit" }, m.section, m.side, m.note));
        if (c.group == "registry-building") != m.building {
            return Err(format!("completion102 mapped kind policy changed {section}"));
        }
        let (item, hp) = if m.building {
            let k = d.building_kind(&m.game_id).ok_or("completion102 actual mapped building absent")?;
            if d.unit_kind(&m.game_id).is_some() {
                return Err("completion102 mapped kind ambiguity".into());
            }
            (sim::ProductionItem::Building(k),d.building_combat(k).ok_or("completion102 actual building combat absent")?.hit_points)
        } else {
            let k = d.unit_kind(&m.game_id).ok_or("completion102 actual mapped unit absent")?;
            if d.building_kind(&m.game_id).is_some() {
                return Err("completion102 mapped kind ambiguity".into());
            }
            (sim::ProductionItem::Unit(k),d.unit_combat(k).ok_or("completion102 actual unit combat absent")?.hit_points)
        };
        let metadata = d.metadata(item).ok_or("completion102 actual typed metadata absent")?;
        let cost = d.production(item).ok_or("completion102 actual production absent")?.cost;
        // Category/domain/role are actual typed policy, never evidence that these original
        // registries were parsed. In particular VehicleTypes includes actual Air ZEP/SHAD.
        actual_loaded.push(format!("{}|{}|{}|{}|{}|{}|{}|{}", m.game_id,
            if m.building { "building" } else { "unit" },
            normalized(&format!("{:?}", metadata.category)),normalized(&format!("{:?}", metadata.domain)),
            normalized(&format!("{:?}",metadata.role)),u8::from(metadata.infantry),cost,hp));
    }
    actual_mapping.sort();
    actual_loaded.sort();
    if actual_mapping.is_empty() || actual_mapping.join(";") != c.mapping {
        return Err(format!("completion102 exact active mapping/alias census changed {:?}", c.key));
    }
    if actual_loaded.join(";") != c.loaded {
        return Err(format!("completion102 typed mapped identity/category/domain/role/Cost/HP changed {:?}", c.key));
    }
    let actual = format!("mapping={};loaded={}",actual_mapping.join(";"),actual_loaded.join(";"));
    if actual != c.binding {
        return Err("completion102 exact actual binding differs from independent contract".into());
    }
    Ok(actual)
}
fn physical(a: &Audit, input: &Inputs, c: &Contract<'_>) -> AuditResult<BTreeMap<WitnessId,BTreeSet<Atom>>> {
    let mut out = BTreeMap::new();
    for m in input.mappings.iter().filter(|m| m.section == source_section(c)) {
        let fields = if m.building {
            vec!["economy.production.cost","combat.hit_points"]
        } else {
            vec!["economy.production.cost","combat.hit_points","arsenal.infantry"]
        };
        for field in fields {
            let id = a.physical(WitnessId::new(&format!("{}@rules:{}",m.game_id,m.section),field));
            if !a.required.contains(&id) {
                return Err(format!("completion102 neighbor not independently required {id:?}"));
            }
            let dependencies = a.required_dependencies.iter().filter(|(found,_)| *found == id)
                .map(|(_,atom)|atom.clone()).collect::<BTreeSet<_>>();
            let source = match field {
                "economy.production.cost" => Some("Cost"),
                "combat.hit_points" => Some("Strength"),
                _ => None,
            };
            if let Some(key) = source {
                let key = SourceKey::new("rules",&m.section,key);
                if input.reference.rows.contains_key(&key)
                    && !dependencies.contains(&Atom { key, portion: Portion::Whole })
                {
                    return Err("completion102 real Cost/Strength neighbor demand missing source".into());
                }
            }
            // Empty dependencies on a native default/false membership compare are allowed;
            // no absent original key is invented and none of the102 policy atoms is credited.
            if let Some(previous) = out.insert(id.clone(), dependencies.clone())
                && previous != dependencies
            {
                return Err(format!("completion102 physical alias dependency disagreement {id:?}"));
            }
        }
    }
    if out.is_empty() { return Err("completion102 lacks actual physical neighbors".into()); }
    Ok(out)
}
pub(super) fn specs(input: &Inputs, d: &Definitions, a: &Audit) -> AuditResult<Vec<Spec>> {
    code_guard()?;
    let contracts = contracts(CONTRACTS)?;
    let keys = contracts.iter().map(|c| c.key.clone()).collect::<BTreeSet<_>>();
    if actual_keyset(input) != keys {
        return Err("completion102 exact original registry/unit-Soylent key census changed".into());
    }
    if a.required_dependencies.iter().any(|(_,atom)| keys.contains(&atom.key))
        || a.witnesses.values().any(|w| matches!(w,Witness::Compared {dependencies,..}
            if dependencies.iter().any(|atom| keys.contains(&atom.key))))
    {
        return Err("completion102 nonread source incorrectly demanded/Compared; reading grants no credit".into());
    }
    contracts.iter().map(|c| {
        let binding = observe(input,d,c)?;
        Ok(Spec { atom: Atom {key:c.key.clone(),portion:Portion::Whole},
            exact:c.exact.into(),class:source_routes::Class::CurrentModelNonread,
            reason:c.reason.into(),binding,guard:c.guard.into(),physical:physical(a,input,c)?,routes:c.routes.clone() })
    }).collect()
}
pub(super) fn guard(input: &Inputs, d: &Definitions, a: &Audit, spec: &Spec) -> AuditResult<String> {
    let found = specs(input,d,a)?.into_iter().find(|c| c.atom == spec.atom)
        .ok_or("completion102 outside exact declared atom set")?;
    if found != *spec { return Err("completion102 source/binding/guard/demands changed after declaration".into()); }
    for (id,wanted) in &spec.physical {
        match a.witnesses.get(id) {
            Some(Witness::Compared { dependencies,.. }) if dependencies == wanted => {}
            _ => return Err(format!("completion102 successful genuine neighbor absent/stale {id:?}")),
        }
    }
    Ok(found.binding)
}

#[test]
fn ra9_completion_whole102_live_baseline_then_source_and_real_neighbor_negatives() -> AuditResult<()> {
    let mut input = Inputs::load(&super::super::data_dir(), &super::super::reference_dir())?;
    let d = sim::load_ruleset(&input.data_dir).map_err(|e|e.to_string())?;
    let sk = sim::load_skirmish(input.data_dir.join("skirmish.ron"),&d).map_err(|e|e.to_string())?;
    let mut a = load_audit(&input,&d)?;
    let plan = specs(&input,&d,&a)?;
    let errors = run_adapters(&mut a,&input,&d,&sk);
    if !errors.is_empty() { return Err(errors.join("\n")); }
    for spec in &plan { guard(&input,&d,&a,spec)?; }
    let spec = plan.iter().find(|s|s.atom.key == SourceKey::new("rules","ADOG","Soylent"))
        .ok_or("ADOG exact policy absent")?;
    let id = spec.physical.keys().next().ok_or("real neighbor absent")?.clone();
    let mut missing = a.clone();
    assert!(missing.witnesses.remove(&id).is_some(),"remove actual successful baseline neighbor");
    assert!(guard(&input,&d,&missing,spec).is_err(),"no loaded getter substitutes successful native-field comparison");
    input.reference.rows.insert(SourceKey::new("rules","ADOG","Soylent"),"51".into());
    assert!(specs(&input,&d,&a).is_err(),"exact source literal changes require a new policy decision");
    Ok(())
}
#[test]
fn ra9_completion_whole_registry_active_alias_and_fake_read_credit_negatives() -> AuditResult<()> {
    let mut input = Inputs::load(&super::super::data_dir(), &super::super::reference_dir())?;
    let d = sim::load_ruleset(&input.data_dir).map_err(|e|e.to_string())?;
    let mut a = load_audit(&input,&d)?;
    specs(&input,&d,&a)?;
    let at = input.mappings.iter().position(|m|m.section=="ORCA").ok_or("mapped active ORCA absent")?;
    let removed = input.mappings.remove(at);
    assert!(specs(&input,&d,&a).is_err(),"active ORCA presence cannot be waived by a registry nonread policy");
    input.mappings.insert(at,removed);
    let baseline = specs(&input,&d,&a)?;
    let spec = baseline.iter().find(|s|s.atom.key==SourceKey::new("rules","AircraftTypes","2"))
        .ok_or("ORCA exact registry policy absent")?;
    let id = spec.physical.keys().next().ok_or("physical neighbor absent")?.clone();
    a.required_dependencies.insert((id,spec.atom.clone()));
    assert!(specs(&input,&d,&a).is_err(),"no artificial source dependency/read credit for registry ordinal");
    Ok(())
}
