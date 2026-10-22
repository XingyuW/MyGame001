//! Historical preparation note: UNCOMPILED/UNRUN negative bodies. Install under the owning policy module.
use super::*;
struct Context {
    input: Inputs,
    d: Definitions,
    scope: scope_inputs::ObservedScope,
    a: Audit,
    p: Plan,
}
fn final_context() -> AuditResult<Context> {
    let data = super::super::super::data_dir();
    let refs = super::super::super::reference_dir();
    super::super::audit_at(&data, &refs)?;
    let input = Inputs::load(&data, &refs)?;
    let d = sim::load_ruleset(&data).map_err(|e| e.to_string())?;
    let skirmish = sim::load_skirmish(data.join("skirmish.ron"), &d).map_err(|e| e.to_string())?;
    let scope = scope_inputs::ObservedScope::read(&refs)?;
    let mut a = load_audit(&input, &d)?;
    let p = declare(&input, &d, &a, &scope)?;
    let errors = run_adapters(&mut a, &input, &d, &skirmish);
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    for fields in p.physical.values() {
        s::compared(&a, fields)?;
    }
    Ok(Context {
        input,
        d,
        scope,
        a,
        p,
    })
}
fn second(p: &Plan) -> AuditResult<Unused> {
    let c = p
        .rows
        .values()
        .nth(1)
        .ok_or("need actual nonfirst source")?;
    Ok(Unused {
        atom: Atom {
            key: c.key.clone(),
            portion: Portion::Whole,
        },
        exact_value: c.value.clone(),
        reason: c.reason.clone(),
        code_evidence: c.family.paths.clone(),
    })
}
fn baseline(c: &Context, row: &Unused) -> AuditResult<()> {
    let mut p = c.p.clone();
    admit(&mut p, &c.a, &c.input, &c.d, row, &c.scope)
}
#[test]
fn ra9_legacy_ai_nonfirst_unaccepted_decision_is_controlled_from_valid_final_record()
-> AuditResult<()> {
    let mut c = final_context()?;
    let row = second(&c.p)?;
    baseline(&c, &row)?;
    let a =
        policy_admission::source_acceptance(&row.atom.key)?.ok_or("real accepted record absent")?;
    let bad = policy_admission::SourceAcceptance {
        recorded_decision: "UNACCEPTED",
        ..a
    };
    let e = admit_from(&mut c.p, &c.a, &c.input, &c.d, &row, &c.scope, Some(&bad))
        .expect_err("controlled unaccepted real row");
    assert!(e.contains("decision/final scope/native receipts"), "{e}");
    Ok(())
}
#[test]
fn ra9_legacy_ai_nonfirst_source_value_change_cannot_keep_old_policy() -> AuditResult<()> {
    let mut c = final_context()?;
    let row = second(&c.p)?;
    baseline(&c, &row)?;
    c.input
        .reference
        .rows
        .insert(row.atom.key.clone(), format!("{}-changed", row.exact_value));
    let e = admit(&mut c.p, &c.a, &c.input, &c.d, &row, &c.scope)
        .expect_err("actual exact source changed");
    assert!(e.contains("source/shape changed"), "{e}");
    Ok(())
}
#[test]
fn ra9_legacy_ai_remove_nonfirst_actual_compared_witness_after_real_adapter() -> AuditResult<()> {
    let mut c = final_context()?;
    let row = second(&c.p)?;
    baseline(&c, &row)?;
    let profile = c.p.rows[&row.atom.key].family.profile.clone();
    let w = c.p.physical[&profile]
        .keys()
        .nth(1)
        .cloned()
        .ok_or("nonfirst actual compared field")?;
    assert!(matches!(
        c.a.witnesses.remove(&w),
        Some(Witness::Compared { .. })
    ));
    let e = admit(&mut c.p, &c.a, &c.input, &c.d, &row, &c.scope)
        .expect_err("remove actual successful comparison");
    assert!(e.contains("Compared/dependency equality absent"), "{e}");
    Ok(())
}
#[test]
fn ra9_legacy_ai_remove_nonfirst_successful_component_dependency_after_adapter() -> AuditResult<()>
{
    let mut c = final_context()?;
    let row = second(&c.p)?;
    baseline(&c, &row)?;
    let profile = c.p.rows[&row.atom.key].family.profile.clone();
    let (w, deps) = c.p.physical[&profile]
        .iter()
        .filter(|(_, deps)| deps.len() > 1)
        .nth(1)
        .ok_or("actual multi-dependency field")?;
    let atom = deps
        .iter()
        .nth(1)
        .cloned()
        .ok_or("nonfirst actual source dependency")?;
    let w = w.clone();
    let Some(Witness::Compared { dependencies, .. }) = c.a.witnesses.get_mut(&w) else {
        return Err("actual baseline Compared missing".into());
    };
    assert!(dependencies.remove(&atom));
    let e = admit(&mut c.p, &c.a, &c.input, &c.d, &row, &c.scope)
        .expect_err("successful dependencies must equal independent demands");
    assert!(e.contains("Compared/dependency equality absent"), "{e}");
    Ok(())
}
#[test]
fn ra9_legacy_ai_new_real_read_cannot_be_erased_to_preserve_old_nonread_count() -> AuditResult<()> {
    let mut c = final_context()?;
    let row = second(&c.p)?;
    baseline(&c, &row)?;
    let profile = c.p.rows[&row.atom.key].family.profile.clone();
    let w = c.p.physical[&profile]
        .keys()
        .nth(1)
        .cloned()
        .ok_or("actual field")?;
    c.a.required_dependencies
        .insert((w.clone(), row.atom.clone()));
    let Some(Witness::Compared { dependencies, .. }) = c.a.witnesses.get_mut(&w) else {
        return Err("actual Compared missing".into());
    };
    assert!(dependencies.insert(row.atom.clone()));
    let e = admit(&mut c.p, &c.a, &c.input, &c.d, &row, &c.scope)
        .expect_err("new real read needs exact Compared migration");
    assert!(e.contains("migrate exact atom to Compared"), "{e}");
    Ok(())
}
#[test]
fn ra9_legacy_ai_remove_nonfirst_source_admission_after_valid_complete_plan() -> AuditResult<()> {
    let c = final_context()?;
    let mut p = c.p;
    let rows = p
        .rows
        .values()
        .map(|v| Unused {
            atom: Atom {
                key: v.key.clone(),
                portion: Portion::Whole,
            },
            exact_value: v.value.clone(),
            reason: v.reason.clone(),
            code_evidence: v.family.paths.clone(),
        })
        .collect::<Vec<_>>();
    for row in &rows {
        admit(&mut p, &c.a, &c.input, &c.d, row, &c.scope)?;
    }
    finish(&p, &c.a)?;
    assert!(p.successful.remove(&rows[1].atom.key));
    let e = finish(&p, &c.a).expect_err("independent source demanded==successful set");
    assert!(e.contains("classification demands != successful"), "{e}");
    Ok(())
}
#[test]
fn ra9_legacy_ai_duplicate_nonfirst_master_atom_rejects_after_valid_final_baseline()
-> AuditResult<()> {
    let c = final_context()?;
    let row = second(&c.p)?;
    baseline(&c, &row)?;
    let master = include_str!("source-candidates.proposed.tsv");
    let prefix = format!(
        "{}\t{}\t{}\t",
        row.atom.key.file, row.atom.key.section, row.atom.key.key
    );
    let real = master
        .lines()
        .find(|v| v.starts_with(&prefix))
        .ok_or("actual master source line")?;
    let bad = format!("{master}{real}\n");
    let e = catalogue_from(&bad)
        .err()
        .ok_or("duplicate actual source must reject")?;
    assert!(e.contains("duplicate original source atom"), "{e}");
    Ok(())
}

#[test]
fn ra9_legacy_ai_valid_loader_nonfirst_order_mutation_cannot_keep_policy_snapshot()
-> AuditResult<()> {
    let c = final_context()?;
    let ai = sim::load_ai(c.input.data_dir.join("ai.ron"), &c.d).map_err(|e| e.to_string())?;
    s::checked_ai_snapshot(&ai, &c.d)?;
    let text =
        std::fs::read_to_string(c.input.data_dir.join("ai.ron")).map_err(|e| e.to_string())?;
    let a = text.find("army: [").ok_or("actual army list")? + "army: [".len();
    let z = a + text[a..].find(']').ok_or("actual army end")?;
    let mut entries = text[a..z].split(',').map(str::trim).collect::<Vec<_>>();
    if entries.len() < 3 || entries[1] == entries[2] {
        return Err("choose a real nonfirst distinct ordered pair in final native AI file".into());
    }
    entries.swap(1, 2);
    let changed = format!("{}{}{}", &text[..a], entries.join(", "), &text[z..]);
    // One short-lived text fixture, no directory/clone/cache/env change; remove even on load error.
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "ra9_legacy_ai_nonfirst_{}-{}.ron",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    if path.exists() {
        return Err("refuse to overwrite temporary fixture".into());
    }
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .and_then(|mut f| f.write_all(changed.as_bytes()))
        .map_err(|e| e.to_string())?;
    let actual = sim::load_ai(&path, &c.d).map_err(|e| e.to_string());
    let removed = std::fs::remove_file(&path).map_err(|e| e.to_string());
    let mutant = actual?;
    removed?;
    assert_eq!(mutant.plans.len(), ai.plans.len());
    assert_eq!(mutant.plans[0].army.len(), ai.plans[0].army.len());
    let e = s::checked_ai_snapshot(&mutant, &c.d)
        .expect_err("valid-loader nonfirst reorder changes real ordered policy");
    assert!(
        e.contains("ordered AI plan/policy/difficulty changed"),
        "{e}"
    );
    Ok(())
}

#[test]
fn ra9_legacy_ai_cross_family_swap_preserving_counts_still_rejects_exact_membership()
-> AuditResult<()> {
    let c = final_context()?;
    let row = second(&c.p)?;
    baseline(&c, &row)?;
    let master = include_str!("source-candidates.proposed.tsv");
    let mut lines = master.lines().map(str::to_owned).collect::<Vec<_>>();
    let records = lines
        .iter()
        .enumerate()
        .skip(1)
        .filter_map(|(n, l)| {
            let r = l.split('\t').collect::<Vec<_>>();
            (r[5] == "PROPOSED_REPLACED_AI_POLICY").then(|| (n, r[6].to_owned()))
        })
        .collect::<Vec<_>>();
    let one = records.get(1).ok_or("nonfirst actual family row")?;
    let two = records
        .iter()
        .skip(2)
        .find(|(_, family)| family != &one.1)
        .ok_or("different actual source family")?;
    for (n, family) in [(one.0, two.1.as_str()), (two.0, one.1.as_str())] {
        let mut fields = lines[n].split('\t').map(str::to_owned).collect::<Vec<_>>();
        fields[6] = family.into();
        lines[n] = fields.join("\t");
    }
    let bad = lines.join("\n") + "\n";
    let e = catalogue_from(&bad)
        .err()
        .ok_or("real family swap with preserved counts must reject")?;
    assert!(e.contains("exact family atom/value membership"), "{e}");
    Ok(())
}
