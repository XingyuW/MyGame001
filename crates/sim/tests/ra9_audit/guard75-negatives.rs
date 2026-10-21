//! UNCOMPILED/UNRUN. Install as child tests of guard75-admission.
//! Native gameplay acceptance is NOT granted here. Real Definitions/Inputs are loaded first;
//! source/mapping/metadata mutations below test admission, not original engine behavior.
use super::*;
fn context() -> AuditResult<(Inputs, Definitions)> {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let data = manifest.join("../../assets/data");
    let reference = manifest.join("tests/reference");
    super::super::audit_at(&data, &reference)
        .map_err(|e| format!("actual accepted positive audit baseline {e}"))?;
    let i =
        Inputs::load(&data, &reference).map_err(|e| format!("positive real input setup {e}"))?;
    let d =
        sim::load_ruleset(&data).map_err(|e| format!("positive actual Definitions setup {e}"))?;
    roots(&i, &d)?;
    selectors(&i)?;
    loaded_roster(&d)?;
    for g in counts().keys() {
        loaded(&d, g)?;
    }
    Ok((i, d))
}
fn test_scope() -> AuditResult<super::super::scope_inputs::ObservedScope> {
    super::super::scope_inputs::ObservedScope::read(&super::super::super::reference_dir())
}

fn row(spec: &Spec) -> Unused {
    Unused {
        atom: Atom {
            key: spec.key.clone(),
            portion: Portion::Whole,
        },
        exact_value: spec.exact.into(),
        reason: spec.reason.into(),
        code_evidence: "test-only guarded classification; no native acceptance".into(),
    }
}
fn text_change(
    d: &Definitions,
    file: &str,
    needle: &str,
    old: &str,
    new: &str,
    arsenal: bool,
) -> AuditResult<Definitions> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/data")
        .join(file);
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut changed = 0;
    let text = text
        .lines()
        .map(|l| {
            if l.contains(needle) {
                changed += 1;
                assert_eq!(l.matches(old).count(), 1, "mutation setup");
                l.replace(old, new)
            } else {
                l.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(changed, 1, "mutation setup must locate one actual entry");
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let p = std::env::temp_dir().join(format!(
        "guard75-{}-{:?}-{}-{file}",
        std::process::id(),
        std::thread::current().id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&p)
        .and_then(|mut f| f.write_all(text.as_bytes()))
        .map_err(|e| e.to_string())?;
    let result = if arsenal {
        d.clone().with_arsenal(&p)
    } else {
        d.clone().with_combat(&p)
    };
    std::fs::remove_file(&p).map_err(|e| e.to_string())?;
    result.map_err(|e| format!("mutated real loader must succeed {e}"))
}
#[test]
fn guard75_declares_exact_classifications_before_unused_callbacks() -> AuditResult<()> {
    let (i, d) = context()?;
    let a = super::super::load_audit(&i, &d)?;
    let observed = test_scope()?;
    let scope = observed.guard75();
    let p = declare(&i, &d, &a, &scope)?;
    assert_eq!(p.wanted.len(), 75);
    assert_eq!(p.done.len(), 0);
    assert_eq!(counts().len(), 20);
    assert!(
        finish(&p, &a)
            .unwrap_err()
            .contains("independently demanded")
    );
    Ok(())
}
#[test]
fn guard75_positive_zero_is_an_exact_condition_not_a_blanket_waiver() -> AuditResult<()> {
    let (mut i, _) = context()?;
    let c = catalogue()?
        .into_values()
        .find(|s| s.key.section == "Cruentus" && s.key.key == "GrowthPercentage")
        .ok_or("spec")?;
    source(&i, &c)?;
    i.reference.rows.insert(c.key.clone(), "1".into());
    assert!(source(&i, &c).unwrap_err().contains("exact source"));
    Ok(())
}
#[test]
fn guard75_omitted_producer_cannot_be_introduced_by_mapping_existing_loaded_actor()
-> AuditResult<()> {
    let (mut i, d) = context()?;
    let n = i
        .mappings
        .iter()
        .position(|m| m.section == "GAPOWR")
        .ok_or("mapped power")?;
    i.mappings[n].section = "GAPOWRUP".into();
    assert!(roots(&i, &d).unwrap_err().contains("root closure"));
    Ok(())
}
#[test]
fn guard75_new_ion_warhead_mapping_fails_even_before_any_nonread_row() -> AuditResult<()> {
    let (mut i, d) = context()?;
    i.warheads.push(("new_ion".into(), "IonCannonWH".into()));
    assert!(roots(&i, &d).unwrap_err().contains("root closure"));
    Ok(())
}
#[test]
fn guard75_a_real_mapped_weapon_selector_cannot_start_using_firestorm() -> AuditResult<()> {
    let (mut i, _) = context()?;
    let k = SourceKey::new("rules", "M60", "Warhead");
    assert!(i.reference.rows.contains_key(&k), "source selector setup");
    i.reference.rows.insert(k, "FirestormWH".into());
    assert!(selectors(&i).unwrap_err().contains("selector"));
    Ok(())
}
#[test]
fn guard75_new_upgrade_selector_on_a_current_producer_is_not_ignored() -> AuditResult<()> {
    let (mut i, _) = context()?;
    let k = SourceKey::new("rules", "GAPOWR", "PowersUpBuilding");
    assert!(
        !i.reference.rows.contains_key(&k),
        "original absent-key setup"
    );
    i.reference.rows.insert(k, "GAPOWR".into());
    assert!(selectors(&i).unwrap_err().contains("selector"));
    Ok(())
}
#[test]
fn guard75_present_original_scope_cannot_be_relabelled_absent() -> AuditResult<()> {
    let (_i, _d) = context()?;
    let marker = "prereq-override\tsection\tCAWASH16\tpresent\ttrue";
    assert_eq!(ORIGINAL.matches(marker).count(), 1);
    let wrong = ORIGINAL.replace(marker, "prereq-override\tsection\tCAWASH16\tpresent\tfalse");
    let observed = test_scope()?;
    let s = Scope {
        observed_original: &wrong,
        ..observed.guard75()
    };
    assert!(scope(&s).unwrap_err().contains("scope observation"));
    Ok(())
}
#[test]
fn guard75_a_new_command_or_model_census_requires_a_new_scope_decision() -> AuditResult<()> {
    let (_i, _d) = context()?;
    let observed = test_scope()?;
    let mut s = observed.guard75();
    s.model_census_sha256 = "changed";
    assert!(scope(&s).unwrap_err().contains("scope observation"));
    Ok(())
}
#[test]
fn guard75_reactor_correct_field_is_real_loaded_data_not_a_typo_alias() -> AuditResult<()> {
    let (_i, d) = context()?;
    loaded(&d, "radiation-typo")?;
    let bad = text_change(
        &d,
        "arsenal.ron",
        r#"id: "nuclear_reactor""#,
        "immune_radiation: 0",
        "immune_radiation: 1",
        true,
    )?;
    assert!(entry(&bad, "nuclear_reactor")?.immune_radiation);
    assert!(
        loaded(&bad, "radiation-typo")
            .unwrap_err()
            .contains("actual loaded premise")
    );
    Ok(())
}
#[test]
fn guard75_aa_speed_model_premise_reads_a_real_loaded_weapon() -> AuditResult<()> {
    let (_i, d) = context()?;
    loaded(&d, "turbo")?;
    let bad = text_change(
        &d,
        "combat.ron",
        r#"id: "red_eye2""#,
        "projectile_speed: 191",
        "projectile_speed: 190",
        false,
    )?;
    assert_eq!(
        bad.weapon(bad.weapon_kind("red_eye2").ok_or("weapon")?)
            .ok_or("weapon")?
            .projectile_speed,
        190
    );
    assert!(
        loaded(&bad, "turbo")
            .unwrap_err()
            .contains("actual loaded premise")
    );
    Ok(())
}
#[test]
fn guard75_nonread_source_cannot_be_falsely_compared_or_aliased() -> AuditResult<()> {
    let (i, d) = context()?;
    let mut a = super::super::load_audit(&i, &d)?;
    let c = catalogue()?
        .into_values()
        .find(|s| s.key.key == "GrowthRate")
        .ok_or("source")?;
    let physical = a.physical(WitnessId::new("rules", "rules.economy.ore_growth_interval"));
    assert!(
        a.required.contains(&physical),
        "actual independent field declared first"
    );
    a.required_dependencies.insert((
        physical,
        Atom {
            key: c.key.clone(),
            portion: Portion::Whole,
        },
    ));
    assert!(no_credit(&a).unwrap_err().contains("falsely demanded"));
    Ok(())
}
#[test]
fn guard75_missing_actual_comparison_or_own_is_not_a_physical_receipt() -> AuditResult<()> {
    let (i, d) = context()?;
    let mut a = super::super::load_audit(&i, &d)?;
    let w = a.physical(WitnessId::new("rules", "rules.economy.ore_growth_interval"));
    let skirmish =
        sim::load_skirmish(i.data_dir.join("skirmish.ron"), &d).map_err(|e| e.to_string())?;
    let errors = super::super::run_adapters(&mut a, &i, &d, &skirmish);
    assert!(errors.is_empty(), "{errors:?}");
    assert!(a.required.contains(&w));
    compared(&a, &w)?;
    assert!(a.witnesses.remove(&w).is_some());
    assert!(compared(&a, &w).unwrap_err().contains("not Compared"));
    a.witnesses.insert(
        w.clone(),
        Witness::GameOwned {
            actual: "2933".into(),
            reason: "test-only own".into(),
            code_evidence: "does not grant source credit".into(),
        },
    );
    assert!(compared(&a, &w).unwrap_err().contains("not Compared"));
    Ok(())
}
#[test]
fn guard75_initial_records_cannot_admit_a_copied_reason_without_a_decision() -> AuditResult<()> {
    let (i, d) = context()?;
    let a = super::super::load_audit(&i, &d)?;
    let observed = test_scope()?;
    let scope = observed.guard75();
    let p = declare(&i, &d, &a, &scope)?;
    let c = p
        .wanted
        .values()
        .find(|s| s.group == "inactive-hunter")
        .ok_or("spec")?
        .clone();
    let mut accepted = acceptance_from(ACCEPT, &c.key)?;
    let proposal = super::policy_admission::SourceProposal {
        key: c.key.clone(),
        exact: c.exact,
        reason: c.reason,
        consumer_bindings: c.bindings,
        model_guard_id: c.guard,
    };
    let live = live_binding(&i, &d, c.group)?;
    super::policy_admission::source_reason(
        &i,
        &row(&c),
        &proposal,
        Some(&accepted),
        &live,
        scope.verified_final_binding,
    )?;
    accepted.recorded_decision = "UNACCEPTED";
    let outcome = super::policy_admission::source_reason(
        &i,
        &row(&c),
        &proposal,
        Some(&accepted),
        &live,
        scope.verified_final_binding,
    );
    assert!(outcome.is_err());
    assert!(p.done.is_empty());
    Ok(())
}
