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
fn ra9_legacy_model_nonfirst_unaccepted_decision_is_controlled_from_valid_final_record()
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
fn ra9_legacy_model_nonfirst_source_value_change_cannot_keep_old_policy() -> AuditResult<()> {
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
fn ra9_legacy_model_remove_nonfirst_actual_compared_witness_after_real_adapter() -> AuditResult<()>
{
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
fn ra9_legacy_model_remove_nonfirst_successful_component_dependency_after_adapter()
-> AuditResult<()> {
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
fn ra9_legacy_model_new_real_read_cannot_be_erased_to_preserve_old_nonread_count() -> AuditResult<()>
{
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
fn ra9_legacy_model_remove_nonfirst_source_admission_after_valid_complete_plan() -> AuditResult<()>
{
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
fn ra9_legacy_model_duplicate_nonfirst_master_atom_rejects_after_valid_final_baseline()
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
