// Historical preparation note: UNCOMPILED/UNRUN. Child of source_routes.rs. Every negative starts with real final
// audit_at success and actual native/loaded adapters. Missing decisions are not positive fixtures.
use super::super::{
    Audit, Definitions, Inputs, audit_at, fire_audit, fire_keys, load_audit, run_adapters,
    scope_inputs, source_plans, special_source_admission,
};
use super::*;
fn baseline() -> AuditResult<(
    Inputs,
    Definitions,
    Audit,
    Plan,
    scope_inputs::ObservedScope,
    fire_keys::Evidence,
)> {
    let data = super::super::super::data_dir();
    let refs = super::super::super::reference_dir();
    audit_at(&data, &refs)?;
    let i = Inputs::load(&data, &refs)?;
    let d = sim::load_ruleset(&data).map_err(|e| e.to_string())?;
    let sk = sim::load_skirmish(data.join("skirmish.ron"), &d).map_err(|e| e.to_string())?;
    let s = scope_inputs::ObservedScope::read(&refs)?;
    let mut a = load_audit(&i, &d)?;
    let mut p = source_plans::declare(&i, &d, &a, &s)?;
    let mut r = declare(&i, &p)?;
    let e = run_adapters(&mut a, &i, &d, &sk);
    if !e.is_empty() {
        return Err(format!("actual adapter baseline {e:?}"));
    }
    let decision = s.return_fire_decision()?;
    let fire = fire_audit::project(&mut a, &i, &d, Some(&decision))?;
    super::super::load_exact_unused(
        &mut a,
        &i,
        &d,
        &refs.join("ra9_source_dispositions.tsv"),
        &mut p,
        &mut r,
        &s,
        &fire,
        None,
    )?;
    a.finish(&i.reference)?;
    Ok((i, d, a, r, s, fire))
}
fn selected(plan: &Plan, route: Route) -> AuditResult<(Atom, Spec)> {
    plan.wanted
        .iter()
        .filter(|(_, s)| s.route == route)
        .nth(1)
        .or_else(|| plan.wanted.iter().find(|(_, s)| s.route == route))
        .map(|(k, s)| (k.clone(), s.clone()))
        .ok_or_else(|| "actual source route missing".into())
}
fn row(k: Atom, s: &Spec) -> Row {
    Row {
        row: Unused {
            atom: k,
            exact_value: s.exact.clone(),
            reason: s.reason.clone(),
            code_evidence: "actual final-source typed admission fixture".into(),
        },
        class: s.class,
        route: s.route,
    }
}
#[test]
fn ra9_compositor_original_unused_cannot_replace_actual_nonfirst_model_class() -> AuditResult<()> {
    let (_, _, _, mut p, _, _) = baseline()?;
    let (k, s) = selected(&p, Route::Model162)?;
    p.done.remove(&k);
    let mut v = row(k, &s);
    validate_row(&p, &v)?;
    v.class = Class::OriginalEngineUnused;
    assert!(validate_row(&p, &v).is_err());
    Ok(())
}
#[test]
fn ra9_compositor_nonfirst_source_cannot_route_through_another_valid_package() -> AuditResult<()> {
    let (_, _, _, mut p, _, _) = baseline()?;
    let (k, s) = selected(&p, Route::Behavior87)?;
    p.done.remove(&k);
    let mut v = row(k, &s);
    validate_row(&p, &v)?;
    v.route = Route::Consumer62;
    assert!(validate_row(&p, &v).is_err());
    Ok(())
}
#[test]
fn ra9_compositor_actual_source_cannot_have_two_independent_owners() -> AuditResult<()> {
    let (_, _, _, mut p, _, _) = baseline()?;
    let (k, s) = selected(&p, Route::Carry38)?;
    assert!(insert(&mut p.wanted, k.key, s).is_err());
    Ok(())
}
#[test]
fn ra9_compositor_nonfirst_successful_classification_omission_fails_equal_sets() -> AuditResult<()>
{
    let (_, _, a, mut p, _, _) = baseline()?;
    finish(&p, &a)?;
    let k = p
        .done
        .keys()
        .nth(1)
        .cloned()
        .ok_or("nonfirst real classification")?;
    p.done.remove(&k);
    assert!(finish(&p, &a).is_err());
    Ok(())
}
#[test]
fn ra9_compositor_actual_vhp_neighbor_must_remain_compared() -> AuditResult<()> {
    let (i, d, mut a, _, s, fire) = baseline()?;
    let k = SourceKey::new("rules", "NASAM", "VHPScan");
    let path = super::super::super::reference_dir().join("ra9_source_dispositions.tsv");
    let v = load_typed(&path)?
        .into_iter()
        .find(|r| r.row.atom.key == k)
        .ok_or("actual VHP classified row")?;
    let mut p = special_source_admission::declare(&i, &d, &a, &s)?;
    special_source_admission::admit(&mut p, &a, &i, &d, &v.row, Route::Vhp, &s, &fire)?;
    let id = a.physical(WitnessId::new("patriot@rules:NASAM", "arsenal.powered"));
    assert!(matches!(
        a.witnesses.get(&id),
        Some(Witness::Compared { .. })
    ));
    a.witnesses.remove(&id);
    let mut p = special_source_admission::declare(&i, &d, &a, &s)?;
    let error = special_source_admission::admit(&mut p, &a, &i, &d, &v.row, Route::Vhp, &s, &fire)
        .err()
        .ok_or("missing real VHP neighbor escaped gate")?;
    assert!(error.contains("compared neighbor"), "{error}");
    Ok(())
}
#[test]
fn ra9_compositor_actual_model_census_nonfirst_record_change_is_detected() -> AuditResult<()> {
    let (_, _, _, _, _, _) = baseline()?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let text =
        std::fs::read_to_string(super::super::super::reference_dir().join("ra9_model_census.tsv"))
            .map_err(|e| e.to_string())?;
    scope_inputs::live_model(&root, &text)?;
    let mut lines = text.lines().map(str::to_owned).collect::<Vec<_>>();
    let mut fields = lines
        .get(2)
        .ok_or("actual nonfirst code-census record")?
        .split('\t')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert_eq!(fields.len(), 4);
    fields[2] = if fields[2] == "0000000000000000" {
        "0000000000000001".into()
    } else {
        "0000000000000000".into()
    };
    lines[2] = fields.join("\t");
    assert!(scope_inputs::live_model(&root, &(lines.join("\n") + "\n")).is_err());
    Ok(())
}
