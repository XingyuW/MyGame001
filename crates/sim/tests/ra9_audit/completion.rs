//! Exact additional atoms discovered by the first complete source census; no blanket coverage.
use super::*;
use source_routes::Class;
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Spec {
    pub(super) atom: Atom,
    pub(super) exact: String,
    pub(super) class: Class,
    pub(super) reason: String,
    pub(super) binding: String,
    pub(super) guard: String,
    pub(super) physical: BTreeMap<WitnessId, BTreeSet<Atom>>,
    pub(super) routes: BTreeSet<String>,
}
#[derive(Clone)]
pub(super) struct Plan {
    rows: BTreeMap<Atom, Spec>,
    done: BTreeSet<Atom>,
}
const ACCEPT: &str = include_str!("completion-acceptance.tsv");
const ACCEPT_HEADER: &str = "file\tsection\tkey\tportion\texact_value\tspecific_reason\tconsumer_bindings\tmodel_guard_id\trecorded_decision\tfinal_input_sha256\tnative_receipts";
fn records<'a>(text: &'a str, header: &str, width: usize) -> AuditResult<Vec<Vec<&'a str>>> {
    let mut lines = text.lines();
    if lines.next() != Some(header) {
        return Err("completion exact header".into());
    }
    lines
        .map(|line| {
            let row = line.split('\t').collect::<Vec<_>>();
            if row.len() != width {
                Err("completion exact width".into())
            } else {
                Ok(row)
            }
        })
        .collect()
}
fn portion(value: &str) -> AuditResult<Portion> {
    if value == "whole" {
        Ok(Portion::Whole)
    } else {
        let index = value.parse::<usize>().map_err(|e| e.to_string())?;
        if index.to_string() != value {
            return Err("completion noncanonical component identity".into());
        }
        Ok(Portion::Component(index))
    }
}
fn portion_text(value: &Portion) -> String {
    match value {
        Portion::Whole => "whole".into(),
        Portion::Component(index) => index.to_string(),
    }
}
fn value(i: &Inputs, atom: &Atom) -> AuditResult<String> {
    match atom.portion {
        Portion::Whole => Ok(i.reference.expect(atom.key.clone())?.value),
        Portion::Component(index) => Ok(i.reference.component(atom.key.clone(), index)?.value),
    }
}
fn local_code() -> AuditResult<String> {
    let mut text = String::new();
    for r in records(
        include_str!("completion-local-code.tsv"),
        "repository_path\tbytes\tfnv1a64\tsha256",
        4,
    )? {
        let bytes =
            std::fs::read(legacy904_support::root().join(r[0])).map_err(|e| e.to_string())?;
        if bytes.len().to_string() != r[1] || legacy904_support::hash(&bytes) != r[2] {
            return Err(format!("completion actual code changed {}", r[0]));
        }
        text.push_str(&format!("{}:{}:{};", r[0], r[1], r[2]));
    }
    Ok(legacy904_support::hash(text.as_bytes()))
}
fn physical(
    a: &Audit,
    selected: &[(String, String)],
) -> AuditResult<BTreeMap<WitnessId, BTreeSet<Atom>>> {
    let mut result = BTreeMap::new();
    for (consumer, field) in selected {
        let id = a.physical(WitnessId::new(consumer, field));
        if !a.required.contains(&id) {
            return Err(format!("completion undeclared actual neighbor {id:?}"));
        }
        let deps = a
            .required_dependencies
            .iter()
            .filter(|(w, _)| w == &id)
            .map(|(_, v)| v.clone())
            .collect::<BTreeSet<_>>();
        result.insert(id, deps);
    }
    Ok(result)
}
fn local_specs(i: &Inputs, d: &Definitions, a: &Audit) -> AuditResult<Vec<Spec>> {
    let code = local_code()?;
    let mut out = Vec::new();
    for r in records(
        include_str!("completion-local-contracts.tsv"),
        "file\tsection\tkey\tportion\texact_value\tclassification\tfamily\tspecific_reason\texact_whole_source",
        9,
    )? {
        let atom = Atom {
            key: SourceKey::new(r[0], r[1], r[2]),
            portion: portion(r[3])?,
        };
        if value(i, &atom)? != r[4]
            || i.reference.rows.get(&atom.key).map(String::as_str) != Some(r[8])
        {
            return Err(format!(
                "completion exact original atom/branch tuple changed {atom:?}"
            ));
        }
        let class = match r[5] {
            "CURRENT_MODEL_NONREAD" => Class::CurrentModelNonread,
            "INTENTIONAL_DIFFERENCE" => Class::IntentionalDifference,
            _ => return Err("completion class".into()),
        };
        let mut selected = Vec::new();
        let mut loaded = String::new();
        let mut routes = BTreeSet::from([
            format!("static/completion/{}", r[6]),
            format!("loaded/completion/{}", r[6]),
        ]);
        if r[1] == "Powerups" {
            let rules = d
                .rules()
                .ok_or("completion requires actual Rules-present powerups")?;
            let share = r[8]
                .split(',')
                .next()
                .ok_or("powerup share")?
                .parse::<i32>()
                .map_err(|e| e.to_string())?;
            let id = match r[2] {
                "Armor" => Some("armour"),
                "Firepower" => Some("firepower"),
                "HealBase" => Some("heal_base"),
                "Money" => Some("money"),
                "Reveal" => Some("reveal"),
                "Speed" => Some("speed"),
                "Veteran" => Some("veteran"),
                "Unit" => Some("unit"),
                _ => None,
            };
            let active = id.and_then(|id| rules.powerups.iter().find(|p| p.id == id));
            if (share > 0) != active.is_some() {
                return Err("completion positive/zero weight handler closure changed".into());
            }
            if share == 0
                && rules
                    .powerups
                    .iter()
                    .any(|p| p.id.eq_ignore_ascii_case(r[2]))
            {
                return Err("completion unsupported effect became loaded".into());
            }
            loaded.push_str(&format!(
                "source-tuple={};native={:?};all={:?}",
                r[8], active, rules.powerups
            ));
            let c = format!("powerup@rules:Powerups:{}", r[2]);
            selected.push((c.clone(), "rules.powerups.membership".into()));
            if share > 0 {
                selected.push((c.clone(), "rules.powerups.share".into()));
                if r[8].split(',').count() == 4 {
                    selected.push((c, "rules.powerups.value".into()));
                }
            }
            if r[3] == "2" && share > 0 {
                routes.insert("native/completion/crates-land-water".into());
            }
        } else {
            let rows = i
                .mappings
                .iter()
                .filter(|m| m.section == r[1])
                .collect::<Vec<_>>();
            if rows.is_empty() {
                return Err("completion unmapped active source".into());
            }
            for row in rows {
                let item = items::item(d, row)?;
                let m = d.metadata(item).ok_or("completion metadata")?;
                loaded.push_str(&format!("{}:{}:{:?};", row.game_id, row.section, m));
                let c = format!("{}@rules:{}", row.game_id, row.section);
                selected.push((c.clone(), "economy.production.cost".into()));
                selected.push((c.clone(), "combat.hit_points".into()));
                if r[6] == "sensor-mask" {
                    selected.push((c.clone(), "arsenal.sensors_sight".into()));
                }
                if r[6] == "factory-alias" {
                    selected.push((c.clone(), "arsenal.factory".into()));
                }
                if r[6] == "refinery-docks" {
                    selected.push((c.clone(), "arsenal.vehicle_docks".into()));
                }
                if r[6] == "starting-twin" {
                    if row.note != "twin" {
                        return Err("completion secondary starting alias became a primary".into());
                    }
                    let primary = i
                        .mappings
                        .iter()
                        .find(|m| m.game_id == row.game_id && m.note != "twin")
                        .ok_or("completion twin primary")?;
                    loaded.push_str(&format!("primary={}:{};", primary.game_id, primary.section));
                }
                if r[6] == "sensor-mask"
                    && (m.sensors_sight != 0 || format!("{:?}", m.role) != "Sensor")
                {
                    return Err("completion inactive SensorsSight/fixed Sensor role changed".into());
                }
                if r[6] == "ivan-role" && format!("{:?}", m.role) != "Demolition" {
                    return Err("completion fixed Ivan role changed".into());
                }
                if r[6] == "spy-role" && format!("{:?}", m.role) != "Spy" {
                    return Err("completion fixed Spy role changed".into());
                }
                if r[6] == "building-ammo" && m.ammo != 0 {
                    return Err("completion building ammo became active".into());
                }
                if r[6] == "refinery-docks" && (m.unit_repair || m.vehicle_docks != 0) {
                    return Err("completion refinery inactive docks branch changed".into());
                }
            }
            let native = match r[6] {
                "factory-alias" => Some("current-paid-production"),
                "neutral-base" => Some("current-neutral-interaction"),
                "ivan-role" => Some("current-ivan-charge"),
                "ground-dock" | "refinery-storage" | "refinery-docks" => {
                    Some("current-ore-delivery")
                }
                "sensor-mask" => Some("current-powered-sensor"),
                "spy-role" => Some("current-spy-copy-move"),
                "nuke-role" => Some("current-nuclear-launch"),
                "building-ammo" => Some("current-natural-paradrop"),
                _ => None,
            };
            if let Some(native) = native {
                routes.insert(format!("native/completion/{native}"));
            }
        }
        let binding = format!(
            "family={};source={}:{}:{}:{};code={code};loaded={}",
            r[6],
            r[0],
            r[1],
            r[2],
            r[3],
            legacy904_support::hash(loaded.as_bytes())
        );
        out.push(Spec {
            atom,
            exact: r[4].into(),
            class,
            reason: r[7].into(),
            binding,
            guard: format!("completion/{}/{}:{}:{}", r[6], r[1], r[2], r[3]),
            physical: physical(a, &selected)?,
            routes,
        });
    }
    if out.len() != 72 {
        return Err("completion exact72 local atoms".into());
    }
    Ok(out)
}
fn all_specs(i: &Inputs, d: &Definitions, a: &Audit) -> AuditResult<BTreeMap<Atom, Spec>> {
    let mut rows = BTreeMap::new();
    let mut specs = local_specs(i, d, a)?;
    specs.extend(completion_whole::specs(i, d, a)?);
    for s in specs {
        if value(i, &s.atom)? != s.exact
            || s.reason.trim().is_empty()
            || s.physical.is_empty()
            || rows.insert(s.atom.clone(), s).is_some()
        {
            return Err("completion duplicate/unspecific exact source spec".into());
        }
    }
    if rows.len() != 174 {
        return Err("completion actual174 source census".into());
    }
    Ok(rows)
}
fn no_credit(a: &Audit, rows: &BTreeMap<Atom, Spec>) -> AuditResult<()> {
    if a.required_dependencies.iter().any(|(_,v)|rows.contains_key(v)) || a.witnesses.values().any(|w|matches!(w,Witness::Compared{dependencies,..} if dependencies.iter().any(|v|rows.contains_key(v)))) {return Err("completion nonread atom became an actual compared dependency; migrate source ownership explicitly".into());}
    Ok(())
}
pub(super) fn declare(i: &Inputs, d: &Definitions, a: &Audit) -> AuditResult<Plan> {
    let rows = all_specs(i, d, a)?;
    no_credit(a, &rows)?;
    Ok(Plan {
        rows,
        done: BTreeSet::new(),
    })
}
pub(super) fn specifications(p: &Plan) -> Vec<(Atom, Class, String, String)> {
    p.rows
        .values()
        .map(|s| (s.atom.clone(), s.class, s.exact.clone(), s.reason.clone()))
        .collect()
}
fn accepted_from(text: &str) -> AuditResult<BTreeMap<Atom, Vec<&str>>> {
    let mut out = BTreeMap::new();
    for r in records(text, ACCEPT_HEADER, 11)? {
        let atom = Atom {
            key: SourceKey::new(r[0], r[1], r[2]),
            portion: portion(r[3])?,
        };
        if out.insert(atom, r).is_some() {
            return Err("completion duplicate exact per-atom acceptance".into());
        }
    }
    Ok(out)
}
pub(super) fn admit(
    p: &mut Plan,
    a: &Audit,
    i: &Inputs,
    d: &Definitions,
    row: &Unused,
    scope: &scope_inputs::ObservedScope,
) -> AuditResult<()> {
    admit_from(p, a, i, d, row, scope, ACCEPT)
}
fn admit_from(
    p: &mut Plan,
    a: &Audit,
    i: &Inputs,
    d: &Definitions,
    row: &Unused,
    scope: &scope_inputs::ObservedScope,
    text: &str,
) -> AuditResult<()> {
    let s = p
        .rows
        .get(&row.atom)
        .ok_or("completion wrong independently owned atom")?;
    if value(i, &s.atom)? != s.exact
        || row.exact_value != s.exact
        || row.reason != s.reason
        || p.done.contains(&s.atom)
    {
        return Err("completion stale exact atom/portion/value/reason".into());
    }
    let current = all_specs(i, d, a)?;
    if current.get(&s.atom) != Some(s) {
        return Err("completion current loaded/mapping/code branch changed".into());
    }
    no_credit(a, &p.rows)?;
    owned_zero_sentinels(a)?;
    legacy904_support::compared(a, &s.physical)?;
    if s.atom.portion == Portion::Whole
        && s.atom.key.section != "Powerups"
        && !include_str!("completion-local-contracts.tsv")
            .lines()
            .skip(1)
            .any(|line| {
                let r = line.split('\t').collect::<Vec<_>>();
                r[0] == s.atom.key.file && r[1] == s.atom.key.section && r[2] == s.atom.key.key
            })
        && completion_whole::guard(i, d, a, s)? != s.binding
    {
        return Err("completion whole current guard changed".into());
    }
    let records = accepted_from(text)?;
    if records.keys().cloned().collect::<BTreeSet<_>>() != p.rows.keys().cloned().collect() {
        return Err("completion per-atom acceptance census mismatch".into());
    }
    let r = records
        .get(&s.atom)
        .ok_or("completion missing exact atom acceptance")?;
    if r[4] != s.exact
        || r[5] != s.reason
        || r[6] != s.binding
        || r[7] != s.guard
        || r[8].is_empty()
        || r[8] == "UNACCEPTED"
        || r[9] != policy_admission::input_binding()
        || r[10].is_empty()
        || r[10] == "UNRUN"
    {
        return Err(
            "completion individual exact atom/current-model choice/input/real evidence missing"
                .into(),
        );
    }
    scope.require_routes(r[10], &s.routes.iter().map(String::as_str).collect())?;
    p.done.insert(s.atom.clone());
    Ok(())
}
pub(super) fn finish(p: &Plan, a: &Audit) -> AuditResult<()> {
    if p.done != p.rows.keys().cloned().collect() {
        return Err("completion independent exact atom demands != successful admissions".into());
    }
    no_credit(a, &p.rows)?;
    owned_zero_sentinels(a)?;
    for s in p.rows.values() {
        legacy904_support::compared(a, &s.physical)?;
    }
    Ok(())
}
fn owned_zero_sentinels(a: &Audit) -> AuditResult<()> {
    for (consumer, field) in [
        ("airport@rules:CAAIRP", "arsenal.ammo.cross_kind"),
        ("powerup@rules:Powerups:HealBase", "rules.powerups.value"),
        ("powerup@rules:Powerups:Reveal", "rules.powerups.value"),
        ("powerup@rules:Powerups:Unit", "rules.powerups.value"),
    ] {
        let id = a.physical(WitnessId::new(consumer, field));
        if !a.required.contains(&id) || a.required_dependencies.iter().any(|(w, _)| *w == id) {
            return Err(format!(
                "completion owned sentinel must be independently required with zero source dependencies {id:?}"
            ));
        }
        match a.witnesses.get(&id) {
            Some(w @ Witness::GameOwned { actual, .. }) if actual == "0" => {
                a.validate_witness(&id, w)?
            }
            _ => {
                return Err(format!(
                    "completion actual GameOwned zero sentinel missing/corrupt; never source Compared credit {id:?}"
                ));
            }
        }
    }
    Ok(())
}
pub(super) fn export(p: &Plan, a: &Audit) -> AuditResult<()> {
    owned_zero_sentinels(a)?;
    println!(
        "RA9_COMPLETION_HEADER\tfile\tsection\tkey\tportion\texact_value\tclassification\tspecific_reason\tconsumer_bindings\tmodel_guard_id\trequired_routes"
    );
    let errors = p
        .rows
        .values()
        .filter_map(|s| legacy904_support::compared(a, &s.physical).err())
        .collect::<Vec<_>>();
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    for s in p.rows.values() {
        let class = match s.class {
            Class::CurrentModelNonread => "CURRENT_MODEL_NONREAD",
            Class::IntentionalDifference => "INTENTIONAL_DIFFERENCE",
            Class::OriginalEngineUnused => {
                return Err("completion never claims original-engine nonuse".into());
            }
        };
        println!(
            "RA9_COMPLETION\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            s.atom.key.file,
            s.atom.key.section,
            s.atom.key.key,
            portion_text(&s.atom.portion),
            s.exact,
            class,
            s.reason,
            s.binding,
            s.guard,
            s.routes.iter().cloned().collect::<Vec<_>>().join(";")
        );
    }
    Ok(())
}

#[test]
fn ra9_completion_powerup_nonfirst_components_and_inactive_branch_use_real_adapter_baseline()
-> AuditResult<()> {
    let mut input = Inputs::load(&super::super::data_dir(), &super::super::reference_dir())?;
    let d = sim::load_ruleset(&input.data_dir).map_err(|e| e.to_string())?;
    let sk =
        sim::load_skirmish(input.data_dir.join("skirmish.ron"), &d).map_err(|e| e.to_string())?;
    let mut a = load_audit(&input, &d)?;
    let p = declare(&input, &d, &a)?;
    assert_eq!(p.rows.len(), 174);
    assert_eq!(
        p.rows
            .keys()
            .filter(|v| matches!(v.portion, Portion::Component(_)))
            .count(),
        42
    );
    assert!(p.done.is_empty(), "declaration grants no source credit");
    let errors = run_adapters(&mut a, &input, &d, &sk);
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    for s in p.rows.values() {
        legacy904_support::compared(&a, &s.physical)?;
    }
    let key = SourceKey::new("rules", "Powerups", "Armor");
    let original = input.reference.rows[&key].clone();
    let mut parts = original.split(',').map(str::to_owned).collect::<Vec<_>>();
    parts[2] = "no".into();
    input.reference.rows.insert(key.clone(), parts.join(","));
    assert!(
        declare(&input, &d, &a).is_err(),
        "nonfirst water component cannot inherit share credit"
    );
    input.reference.rows.insert(key.clone(), original.clone());
    let mut parts = original.split(',').map(str::to_owned).collect::<Vec<_>>();
    parts.swap(1, 2);
    input.reference.rows.insert(key.clone(), parts.join(","));
    assert!(
        declare(&input, &d, &a).is_err(),
        "animation/water association cannot reorder"
    );
    input.reference.rows.insert(key, original);
    let gas = SourceKey::new("rules", "Powerups", "Gas");
    let original = input.reference.rows[&gas].clone();
    input
        .reference
        .rows
        .insert(gas.clone(), original.replacen("0,", "1,", 1));
    assert!(
        declare(&input, &d, &a).is_err(),
        "zero-share inactive premise must fail when activated"
    );
    input.reference.rows.insert(gas, original);
    let valid = declare(&input, &d, &a)?;
    let s = valid
        .rows
        .values()
        .find(|s| {
            s.atom.key == SourceKey::new("rules", "Powerups", "Armor")
                && s.atom.portion == Portion::Component(2)
        })
        .ok_or("Armor water spec")?;
    let mut missing = a.clone();
    assert!(
        missing
            .witnesses
            .remove(s.physical.keys().next().ok_or("real neighbor")?)
            .is_some()
    );
    assert!(legacy904_support::compared(&missing, &s.physical).is_err());
    let mut fake = a.clone();
    fake.required_dependencies.insert((
        s.physical.keys().next().ok_or("real neighbor")?.clone(),
        s.atom.clone(),
    ));
    assert!(
        declare(&input, &d, &fake).is_err(),
        "nonread component may not get fake Compared demand"
    );
    declare(&input, &d, &a)?;
    Ok(())
}

#[test]
fn ra9_completion_exact_component_acceptance_mutations_start_from_valid_full_audit()
-> AuditResult<()> {
    audit_at(&super::super::data_dir(), &super::super::reference_dir())?;
    let input = Inputs::load(&super::super::data_dir(), &super::super::reference_dir())?;
    let d = sim::load_ruleset(&input.data_dir).map_err(|e| e.to_string())?;
    let sk =
        sim::load_skirmish(input.data_dir.join("skirmish.ron"), &d).map_err(|e| e.to_string())?;
    let mut a = load_audit(&input, &d)?;
    let plan = declare(&input, &d, &a)?;
    let errors = run_adapters(&mut a, &input, &d, &sk);
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    let scope = scope_inputs::ObservedScope::read(&super::super::reference_dir())?;
    let s = plan
        .rows
        .values()
        .find(|s| {
            s.atom.key == SourceKey::new("rules", "Powerups", "Armor")
                && s.atom.portion == Portion::Component(2)
        })
        .ok_or("Armor water spec")?;
    let row = Unused {
        atom: s.atom.clone(),
        exact_value: s.exact.clone(),
        reason: s.reason.clone(),
        code_evidence: s.binding.clone(),
    };
    admit_from(&mut plan.clone(), &a, &input, &d, &row, &scope, ACCEPT)?;
    let lines = ACCEPT.lines().map(str::to_owned).collect::<Vec<_>>();
    let at = lines
        .iter()
        .position(|line| line.starts_with("rules\tPowerups\tArmor\t2\t"))
        .ok_or("installed accepted water row")?;
    for mode in 0..5 {
        let mut changed = lines.clone();
        let mut cells = changed[at]
            .split('\t')
            .map(str::to_owned)
            .collect::<Vec<_>>();
        match mode {
            0 => cells[3] = "1".into(),
            1 => cells[3] = "whole".into(),
            2 => cells[4] = "no".into(),
            3 => cells[8] = "UNACCEPTED".into(),
            _ => cells[3] = "02".into(),
        }
        changed[at] = cells.join("\t");
        let text = changed.join("\n") + "\n";
        assert!(
            admit_from(&mut plan.clone(), &a, &input, &d, &row, &scope, &text).is_err(),
            "intended component/sibling/whole/value/unaccepted mutation {mode}"
        );
    }
    Ok(())
}

#[test]
fn ra9_completion_building_ammo_keeps_real_owned_zero_sentinel_without_source_credit()
-> AuditResult<()> {
    let input = Inputs::load(&super::super::data_dir(), &super::super::reference_dir())?;
    let d = sim::load_ruleset(&input.data_dir).map_err(|e| e.to_string())?;
    let sk =
        sim::load_skirmish(input.data_dir.join("skirmish.ron"), &d).map_err(|e| e.to_string())?;
    let mut a = load_audit(&input, &d)?;
    let p = declare(&input, &d, &a)?;
    let errors = run_adapters(&mut a, &input, &d, &sk);
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    owned_zero_sentinels(&a)?;
    let atom = Atom {
        key: SourceKey::new("rules", "CAAIRP", "Ammo"),
        portion: Portion::Whole,
    };
    let spec = p.rows.get(&atom).ok_or("actual building-ammo policy")?;
    legacy904_support::compared(&a, &spec.physical)?;
    assert!(!a.required_dependencies.iter().any(|(_, s)| *s == atom));
    let id = a.physical(WitnessId::new(
        "airport@rules:CAAIRP",
        "arsenal.ammo.cross_kind",
    ));
    assert!(
        !spec.physical.contains_key(&id),
        "GameOwned sentinel is not disguised as a source comparison"
    );
    let mut missing = a.clone();
    assert!(missing.witnesses.remove(&id).is_some());
    assert!(owned_zero_sentinels(&missing).is_err());
    let mut bad = a.clone();
    if let Some(Witness::GameOwned { actual, .. }) = bad.witnesses.get_mut(&id) {
        *actual = "1".into();
    } else {
        return Err("actual GameOwned baseline absent".into());
    }
    assert!(owned_zero_sentinels(&bad).is_err());
    let mut fake = a.clone();
    fake.witnesses.insert(
        id,
        Witness::Compared {
            actual: "0".into(),
            expected: "0".into(),
            dependencies: BTreeSet::new(),
            difference: None,
            relation: ComparisonRelation::Exact,
        },
    );
    assert!(
        owned_zero_sentinels(&fake).is_err(),
        "no fake Compare/native-source credit"
    );
    for key in ["HealBase", "Reveal", "Unit"] {
        let id = a.physical(WitnessId::new(
            &format!("powerup@rules:Powerups:{key}"),
            "rules.powerups.value",
        ));
        let mut absent = a.clone();
        assert!(absent.witnesses.remove(&id).is_some());
        assert!(
            owned_zero_sentinels(&absent).is_err(),
            "real argumentless {key} sentinel must not vanish"
        );
    }
    owned_zero_sentinels(&a)?;
    Ok(())
}
