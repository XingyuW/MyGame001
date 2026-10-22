//! Exact19 speed-selector dispositions. UNCOMPILED/UNRUN, zero coverage mutation.
use super::*;
const ROWS: &str = include_str!("masked-speed-atoms.tsv");
fn cell<'a>(r: &'a Reference, s: &str, k: &str, default: &'a str) -> &'a str {
    r.rows
        .get(&SourceKey::new("rules", s, k))
        .map_or(default, String::as_str)
}
fn yes(r: &Reference, s: &str, k: &str) -> AuditResult<bool> {
    match cell(r, s, k, "no") {
        "yes" | "true" | "1" => Ok(true),
        "no" | "false" | "0" => Ok(false),
        v => Err(format!("invalid masked-speed source flag {s}.{k}={v}")),
    }
}
fn compared(a: &Audit, id: &str) -> AuditResult<()> {
    let witness = a.physical(WitnessId::new(id, "combat.weapons.projectile_speed"));
    if !a.required.contains(&witness) {
        return Err("no independent actual physical speed demand".into());
    }
    match a.witnesses.get(&witness) {
        Some(Witness::Compared { dependencies, .. }) => {
            let required = a
                .required_dependencies
                .iter()
                .filter(|(w, _)| w == &witness)
                .map(|(_, atom)| atom.clone())
                .collect::<BTreeSet<_>>();
            if &required != dependencies {
                return Err(
                    "physical speed successful dependencies differ from independent branch demand"
                        .into(),
                );
            }
            Ok(())
        }
        _ => Err("masked speed policy requires actual successful physical speed comparison".into()),
    }
}
pub(super) fn require(
    a: &Audit,
    input: &Inputs,
    d: &Definitions,
    row: &Unused,
    acceptance: Option<&super::policy_admission::SourceAcceptance<'_>>,
    live_input_sha256: &str,
) -> AuditResult<()> {
    let r = ROWS
        .lines()
        .skip(1)
        .map(|l| l.split('\t').collect::<Vec<_>>())
        .find(|r| r.len() == 9 && SourceKey::new(r[0], r[1], r[2]) == row.atom.key)
        .ok_or("source outside exact19 masked-speed policies")?;
    let section = r[1];
    let projectile = r[6];
    let policy = r[7];
    let id = r[5];
    if row.atom.portion != Portion::Whole
        || row.exact_value != r[3]
        || row.reason != r[8]
        || input.reference.rows.get(&row.atom.key).map(String::as_str) != Some(r[3])
    {
        return Err("masked speed exact source/reason changed".into());
    }
    if a.required_dependencies.iter().any(|(_,v)|v.key==row.atom.key)||a.witnesses.values().any(|w|matches!(w,Witness::Compared{dependencies,..}if dependencies.iter().any(|v|v.key==row.atom.key))){return Err("masked atom still falsely demanded or marked successfully used".into());}
    let source = &input.reference;
    if cell(source, section, "Projectile", "") != projectile {
        return Err("source speed selector link changed".into());
    }
    let consumer = format!("weapon.{id}@rules:{section}");
    let actual = input
        .weapons
        .iter()
        .filter(|(_, s)| s == section)
        .map(|(id, _)| id.as_str())
        .collect::<Vec<_>>();
    if actual != vec![id] {
        return Err("masked source consumer mapping changed".into());
    }
    compared(a, &consumer)?;
    let kind = d
        .weapon_kind(id)
        .ok_or("actual masked-source weapon missing")?;
    let w = d.weapon(kind).ok_or("actual masked-source weapon absent")?;
    let expected = super::combat::projectile_speed(source, section)?;
    if i128::from(w.projectile_speed) != expected.value {
        return Err("physical loaded speed differs from active source expression".into());
    }
    match policy {
        "inviso-masked-limbo" => {
            if projectile != "SQDJUMP"
                || !yes(source, projectile, "Inviso")?
                || !yes(source, section, "LimboLaunch")?
                || w.projectile_speed != 0
            {
                return Err("Inviso-first mask premise no longer holds".into());
            }
            let h = d.warhead(w.warhead).ok_or("actual squid warhead absent")?;
            if !h.parasite || h.paralyzes == 0 {
                return Err("actual native squid parasite/paralysis premise lost".into());
            }
            let k = d.unit_kind("giant_squid").ok_or("actual squid missing")?;
            let m = d
                .metadata(sim::ProductionItem::Unit(k))
                .ok_or("actual squid metadata absent")?;
            if m.role != sim::Role::Squid {
                return Err("actual squid role dispatch changed".into());
            }
            let t = d.unit_combat(k).ok_or("actual squid slots absent")?;
            if !t.weapons(section == "SquidGrabE").contains(&Some(kind)) {
                return Err("exact squid rank slots no longer carry this source weapon".into());
            }
        }
        "arcing-range-gravity" => {
            if yes(source, projectile, "Inviso")?
                || yes(source, section, "LimboLaunch")?
                || !yes(source, projectile, "Arcing")?
                || w.projectile_speed == 0
            {
                return Err("nominal Speed is no longer masked by the active arcing model".into());
            }
            if expected.deps.iter().any(|a| a.key == row.atom.key)
                || !expected
                    .deps
                    .iter()
                    .any(|a| a.key == SourceKey::new("rules", section, "Range"))
                || !expected
                    .deps
                    .iter()
                    .any(|a| a.key == SourceKey::new("rules", "AudioVisual", "Gravity"))
            {
                return Err("actual arcing physical expression lost Range+Gravity association or falsely credited nominal Speed".into());
            }
        }
        _ => return Err("unknown exact speed disposition".into()),
    }
    let guard_id = format!("ra9.{policy}.{section}.v1");
    let proposal = super::policy_admission::SourceProposal {
        key: row.atom.key.clone(),
        exact: r[3],
        reason: r[8],
        consumer_bindings: &consumer,
        model_guard_id: &guard_id,
    };
    // Exact guard value/absence catalogue and incoming source graph in masked-speed-guards.json
    // must be reconciled against the live original/reference scope by root before this call.
    super::policy_admission::source_reason(
        input,
        row,
        &proposal,
        acceptance,
        &consumer,
        live_input_sha256,
    )
}

pub(super) fn demand(input: &Inputs) -> AuditResult<BTreeSet<SourceKey>> {
    // Independently authored classification obligations, before actual adapter callbacks.
    // Never inspect the successful projectile expression's dependency collection here.
    let mut demand = BTreeSet::new();
    for row in ROWS
        .lines()
        .skip(1)
        .map(|l| l.split('\t').collect::<Vec<_>>())
    {
        if row.len() != 9 {
            return Err("masked source demand row width".into());
        }
        let key = SourceKey::new(row[0], row[1], row[2]);
        if input.reference.rows.get(&key).map(String::as_str) != Some(row[3]) {
            return Err("exact original masked-source demand value/presence changed".into());
        }
        let p = cell(&input.reference, row[1], "Projectile", "");
        if p != row[6] {
            return Err("masked source-demand projectile link changed".into());
        }
        match row[7] {
            "inviso-masked-limbo" => {
                if !yes(&input.reference, p, "Inviso")? {
                    return Err(
                        "Inviso no longer masks right OR operand; old classification invalid"
                            .into(),
                    );
                }
            }
            "arcing-range-gravity" => {
                if yes(&input.reference, p, "Inviso")?
                    || yes(&input.reference, row[1], "LimboLaunch")?
                    || !yes(&input.reference, p, "Arcing")?
                {
                    return Err(
                        "old arcing nominal-speed classification is not the actual active branch"
                            .into(),
                    );
                }
            }
            _ => return Err("unknown independently demanded source disposition".into()),
        }
        if !demand.insert(key) {
            return Err("duplicate masked source demand".into());
        }
    }
    if demand.len() != 19 {
        return Err("exact19 source classification demand count".into());
    }
    Ok(demand)
}

pub(super) fn contains(key: &SourceKey) -> bool {
    ROWS.lines()
        .skip(1)
        .map(|l| l.split('\t').collect::<Vec<_>>())
        .any(|r| r.len() == 9 && SourceKey::new(r[0], r[1], r[2]) == *key)
}
// Keep the exact typed source-membership API checked; this grants no coverage.
const _: fn(&SourceKey) -> bool = contains;

// Installed actual prepared negative block: integration-blockers-20261019-v1.UNVALIDATED.speed-negatives.rs.txt
// UNCOMPILED/UNRUN; add to masked-speed-admission.rs after speed-shortcircuit.patch.
#[cfg(test)]
mod tests {
    use super::*;
    fn control(section: &str, id: &str) -> AuditResult<(Inputs, Definitions, Audit)> {
        let input = Inputs::load(
            &super::super::super::data_dir(),
            &super::super::super::reference_dir(),
        )?;
        let d = sim::load_ruleset(&input.data_dir).map_err(|e| e.to_string())?;
        let mut a = super::super::load_audit(&input, &d)?;
        super::super::combat::weapon(
            &mut a,
            &input.reference,
            &d,
            id,
            section,
            &super::super::index(&input.warheads),
        )?;
        compared(&a, &format!("weapon.{id}@rules:{section}"))?;
        Ok((input, d, a))
    }
    fn has(value: &Expected<i128>, section: &str, key: &str) -> bool {
        value
            .deps
            .iter()
            .any(|d| d.key == SourceKey::new("rules", section, key))
    }
    #[test]
    fn ra9_squid_base_and_elite_inviso_masks_limbo_dependency() -> AuditResult<()> {
        for (section, id) in [("SquidGrab", "squid_grab"), ("SquidGrabE", "squid_grab_e")] {
            let (input, d, _) = control(section, id)?;
            let v = super::super::combat::projectile_speed(&input.reference, section)?;
            assert_eq!(v.value, 0);
            assert!(has(&v, section, "Projectile") && has(&v, "SQDJUMP", "Inviso"));
            assert!(!has(&v, section, "LimboLaunch"));
            assert!(!has(&v, "SQDJUMP", "Arcing"));
            assert_eq!(
                d.weapon(d.weapon_kind(id).ok_or("actual squid weapon")?)
                    .ok_or("actual weapon")?
                    .projectile_speed,
                0
            );
        }
        Ok(())
    }
    #[test]
    fn ra9_dog_unmasked_limbo_remains_an_actual_speed_selector() -> AuditResult<()> {
        let (input, _, _) = control("GoodTeeth", "good_teeth")?;
        let v = super::super::combat::projectile_speed(&input.reference, "GoodTeeth")?;
        assert_eq!(v.value, 0);
        assert!(has(&v, "GoodTeeth", "LimboLaunch"));
        Ok(())
    }
    #[test]
    fn ra9_changed_masked_limbo_does_not_gain_physical_speed_credit() -> AuditResult<()> {
        let (mut input, _, _) = control("SquidGrab", "squid_grab")?;
        let before = super::super::combat::projectile_speed(&input.reference, "SquidGrab")?;
        input.reference.rows.insert(
            SourceKey::new("rules", "SquidGrab", "LimboLaunch"),
            "no".into(),
        );
        let after = super::super::combat::projectile_speed(&input.reference, "SquidGrab")?;
        assert_eq!(before.value, after.value);
        assert_eq!(before.deps, after.deps);
        assert!(!has(&after, "SquidGrab", "LimboLaunch"));
        let spec = ROWS
            .lines()
            .skip(1)
            .map(|l| l.split('\t').collect::<Vec<_>>())
            .find(|r| r[1] == "SquidGrab")
            .ok_or("exact original mask record")?;
        assert_ne!(
            input
                .reference
                .rows
                .get(&SourceKey::new("rules", "SquidGrab", "LimboLaunch"))
                .map(String::as_str),
            Some(spec[3]),
            "old exact unused/model record must be stale even though physical speed stays0"
        );
        Ok(())
    }
    #[test]
    fn ra9_nonfirst_arcing_nominal_speed_mutation_is_not_comparison_coverage() -> AuditResult<()> {
        let (mut input, _, _) = control("SABOTE", "sabot_e")?;
        let before = super::super::combat::projectile_speed(&input.reference, "SABOTE")?;
        input
            .reference
            .rows
            .insert(SourceKey::new("rules", "SABOTE", "Speed"), "61".into());
        let after = super::super::combat::projectile_speed(&input.reference, "SABOTE")?;
        assert_eq!(before.value, after.value);
        assert_eq!(before.deps, after.deps);
        assert!(!has(&after, "SABOTE", "Speed"));
        assert!(has(&after, "SABOTE", "Range") && has(&after, "AudioVisual", "Gravity"));
        Ok(())
    }
    #[test]
    fn ra9_arcing_selector_removal_activates_nominal_speed_and_fails_native_adapter()
    -> AuditResult<()> {
        let (mut input, d, _) = control("SABOTE", "sabot_e")?;
        input
            .reference
            .rows
            .insert(SourceKey::new("rules", "Cannon", "Arcing"), "no".into());
        let active = super::super::combat::projectile_speed(&input.reference, "SABOTE")?;
        assert!(has(&active, "SABOTE", "Speed") && !has(&active, "AudioVisual", "Gravity"));
        let mut a = super::super::load_audit(&input, &d)?;
        let e = super::super::combat::weapon(
            &mut a,
            &input.reference,
            &d,
            "sabot_e",
            "SABOTE",
            &super::super::index(&input.warheads),
        )
        .err()
        .ok_or("changed source travel branch escaped physical loaded-speed comparison")?;
        if !e.contains("projectile_speed") {
            return Err(format!("wrong actual branch mismatch {e}"));
        }
        Ok(())
    }
}
