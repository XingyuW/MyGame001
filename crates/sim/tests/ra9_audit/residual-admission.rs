//! Test-only proposed integration module. UNCOMPILED/UNRUN. No coverage mutation.
//! Parent module is the v1 audit implementation after v2/v3 and source-policy corrections.
//! Root independently verifies live census/original source before constructing Approval.
use super::*;
const DELTA: &str = include_str!("residual-disposition-20261019-v1.UNVALIDATED.delta.tsv");
const GUARDS: &str = include_str!("residual-disposition-20261019-v1.UNVALIDATED.guard-records.tsv");
const BINDINGS: &str = include_str!("residual-disposition-20261019-v1.UNVALIDATED.bindings.tsv");
pub(super) struct Approval<'a> {
    pub(super) recorded_decision: &'a str,
    pub(super) live_census_sha256: &'a str,
    pub(super) original_section_closure_record: &'a str,
    // Actual validation logs are checked by root, not authenticated by these strings.
    pub(super) required_native_receipts_record: &'a str,
}
fn records(text: &str) -> impl Iterator<Item = Vec<&str>> {
    text.lines().skip(1).map(|l| l.split('\t').collect())
}
fn compared(a: &Audit, consumer: &str, field: &str) -> AuditResult<()> {
    let id = a.physical(WitnessId::new(consumer, field));
    if !a.required.contains(&id) {
        return Err(format!("no independent residual-control demand {id:?}"));
    }
    match a.witnesses.get(&id) {
        Some(Witness::Compared { dependencies, .. }) => {
            let demanded = a
                .required_dependencies
                .iter()
                .filter(|(w, _)| w == &id)
                .map(|(_, atom)| atom.clone())
                .collect::<BTreeSet<_>>();
            if &demanded != dependencies {
                return Err(format!("residual control dependency inequality {id:?}"));
            }
            Ok(())
        }
        _ => Err(format!(
            "residual control is no successful actual comparison {id:?}"
        )),
    }
}
fn bound<'a>(section: &str, key: &str) -> AuditResult<Vec<Vec<&'a str>>> {
    // Static include data can safely outlive the borrow of the caller's source key.
    let rows = records(BINDINGS)
        .filter(|r| r.first() == Some(&section) && r.get(1) == Some(&key))
        .collect::<Vec<_>>();
    if rows.iter().any(|r| r.len() != 6) {
        return Err("malformed residual binding table".into());
    }
    Ok(rows)
}
pub(super) fn require(
    a: &Audit,
    input: &Inputs,
    d: &Definitions,
    row: &Unused,
    approval: Option<&Approval<'_>>,
) -> AuditResult<()> {
    let approval = approval.ok_or("root has not accepted this exact residual model policy")?;
    if approval.live_census_sha256 != include_str!("ra9-approved-model-census-sha256.txt").trim()
        || approval.recorded_decision.trim().is_empty()
        || approval.original_section_closure_record.trim().is_empty()
        || approval.required_native_receipts_record.trim().is_empty()
    {
        return Err("missing independent source/decision/native-receipt gate".into());
    }
    let r = records(DELTA)
        .find(|r| {
            r.first() == Some(&row.atom.key.file.as_str())
                && r.get(1) == Some(&row.atom.key.section.as_str())
                && r.get(2) == Some(&row.atom.key.key.as_str())
        })
        .ok_or("atom outside exact117 residual scope")?;
    if r.len() != 11 {
        return Err("malformed residual delta record".into());
    }
    let (section, key, exact, policy) = (r[1], r[2], r[3], r[5]);
    if row.atom.portion != Portion::Whole || input.reference.shapes.contains_key(&row.atom.key) {
        return Err("residual scalar cannot hide components".into());
    }
    if row.exact_value != exact
        || input.reference.rows.get(&row.atom.key).map(String::as_str) != Some(exact)
    {
        return Err("changed residual source value/presence".into());
    }
    let mut aliases = BTreeSet::new();
    for k in input.reference.rows.keys() {
        if !aliases.insert((
            k.file.to_ascii_lowercase(),
            k.section.to_ascii_lowercase(),
            k.key.to_ascii_lowercase(),
        )) {
            return Err("duplicate residual source alias".into());
        }
    }
    if a.required_dependencies.iter().any(|(_,atom)|atom.key==row.atom.key)||a.witnesses.values().any(|w|matches!(w,Witness::Compared{dependencies,..}if dependencies.iter().any(|atom|atom.key==row.atom.key))){return Err("residual source atom overlaps actual demanded/successful use".into());}
    for g in records(GUARDS).filter(|g| g.first() == Some(&section) && g.get(1) == Some(&key)) {
        if g.len() != 5 {
            return Err("malformed exact residual guard".into());
        }
        let expected = (g[4] != "@ABSENT@").then_some(g[4]);
        if input
            .reference
            .rows
            .get(&SourceKey::new("rules", g[2], g[3]))
            .map(String::as_str)
            != expected
        {
            return Err(format!(
                "changed/incompletely extracted residual premise {}.{}",
                g[2], g[3]
            ));
        }
    }
    let bindings = bound(section, key)?;
    let expected_items = bindings
        .iter()
        .filter(|r| r[2] == "item")
        .map(|r| (r[4].to_owned(), r[3].to_owned(), r[5] == "1"))
        .collect::<BTreeSet<_>>();
    let actual_items = input
        .mappings
        .iter()
        .filter(|m| m.section == section)
        .map(|m| (m.game_id.clone(), m.section.clone(), m.building))
        .collect::<BTreeSet<_>>();
    if expected_items != actual_items {
        return Err("residual exact item consumer set changed".into());
    }
    for (id, source, building) in expected_items {
        let item = if building {
            d.building_kind(&id).map(sim::ProductionItem::Building)
        } else {
            d.unit_kind(&id).map(sim::ProductionItem::Unit)
        }
        .ok_or("residual mapped actual item missing")?;
        let consumer = format!("{id}@rules:{source}");
        for field in [
            "economy.production.cost",
            "combat.hit_points",
            "combat.sight",
        ] {
            compared(a, &consumer, field)?;
        }
        if policy == "turret" && d.metadata(item).is_some_and(|m| m.can_be_occupied) {
            return Err("native turret/garrison interaction premise changed".into());
        }
    }
    let mut observed_weapons = BTreeSet::new();
    for (id, source) in &input.weapons {
        let kind = if source == section {
            Some("weapon")
        } else if input
            .reference
            .rows
            .get(&SourceKey::new("rules", source, "Projectile"))
            .map(String::as_str)
            == Some(section)
        {
            Some("projectile")
        } else {
            None
        };
        if let Some(kind) = kind {
            observed_weapons.insert((kind.to_owned(), source.clone(), id.clone()));
        }
    }
    let expected_weapons = bindings
        .iter()
        .filter(|r| matches!(r[2], "weapon" | "projectile"))
        .map(|r| (r[2].to_owned(), r[3].to_owned(), r[4].to_owned()))
        .collect::<BTreeSet<_>>();
    if observed_weapons != expected_weapons {
        return Err("residual exact native weapon/projectile consumer set changed".into());
    }
    for (_, source, id) in expected_weapons {
        let k = d.weapon_kind(&id).ok_or("residual actual weapon absent")?;
        d.weapon(k).ok_or("residual actual weapon missing")?;
        for field in ["damage", "range", "reload", "warhead", "projectile_speed"] {
            compared(
                a,
                &format!("weapon.{id}@rules:{source}"),
                &format!("combat.weapons.{field}"),
            )?;
        }
    }
    let expected_heads = bindings
        .iter()
        .filter(|r| r[2] == "warhead")
        .map(|r| (r[4].to_owned(), r[3].to_owned()))
        .collect::<BTreeSet<_>>();
    let actual_heads = input
        .warheads
        .iter()
        .filter(|(_, s)| s == section)
        .cloned()
        .collect::<BTreeSet<_>>();
    if expected_heads != actual_heads {
        return Err("residual warhead consumer changed".into());
    }
    for (id, source) in expected_heads {
        for field in ["versus", "wall", "wood"] {
            compared(
                a,
                &format!("warhead.{id}@rules:{source}"),
                &format!("combat.warheads.{field}"),
            )?;
        }
    }
    match policy {
        "charge-single-weapon" | "charge-turret-count" => {
            let k = d.unit_kind("prism_tank").ok_or("actual SREF absent")?;
            let t = d.unit_combat(k).ok_or("actual SREF slots absent")?;
            let base = d.weapon_kind("comet").ok_or("actual Comet absent")?;
            let elite = d
                .weapon_kind("super_comet")
                .ok_or("actual SuperComet absent")?;
            if t.weapons(false) != [Some(base), None] || t.weapons(true) != [Some(elite), None] {
                return Err(
                    "conditional charge selector now has a nonfirst/changed native slot".into(),
                );
            }
            super::array_plans::run(input, d)?;
        }
        "ifv-turret-count" => {
            let k = d.unit_kind("ifv").ok_or("actual FV absent")?;
            let m = d
                .metadata(sim::ProductionItem::Unit(k))
                .ok_or("actual FV metadata absent")?;
            if !m.gunner || m.ifv_weapons.len() != 13 {
                return Err("four art turrets cannot waive thirteen actual modes".into());
            }
            super::array_plans::run(input, d)?;
        }
        "building-laser-primary" => {
            let k = d
                .building_kind("prism_tower")
                .ok_or("actual ATESLA absent")?;
            let t = d.building_combat(k).ok_or("actual ATESLA combat absent")?;
            let primary = d
                .weapon_kind("prism_shot")
                .ok_or("actual PrismShot absent")?;
            let secondary = d
                .weapon_kind("prism_support")
                .ok_or("actual PrismSupport absent")?;
            if t.weapons(false) != [Some(primary), Some(secondary)]
                || t.weapons(true) != [Some(primary), Some(secondary)]
            {
                return Err("Prism Primary/support slots changed".into());
            }
            for field in ["support_max", "support_modifier_pm", "support_delay"] {
                compared(a, "rules", &format!("rules.prism.{field}"))?;
            }
            super::array_plans::run(input, d)?;
        }
        "radiation-light-delay" => {
            for field in [
                "application_delay",
                "level_delay",
                "level_factor_pm",
                "level_max",
                "duration_multiple_pm",
            ] {
                compared(a, "rules", &format!("rules.radiation.{field}"))?;
            }
        }
        "techno-turn" | "projectile-turn" | "instant-leap-turn" | "turret" | "sparky-false"
        | "electric-spark" | "fire-no-ice" | "big-laser" => {}
        _ => return Err("unknown residual policy".into()),
    }
    // This checks necessary loaded/source premises ONLY. It neither adds unused rows nor marks
    // a physical/source witness covered. Root must consume accepted specific decisions and actual
    // fresh native logs, reconcile incoming graph, then invoke the existing final ledger.
    Ok(())
}

// Installed actual prepared negative block: residual-disposition-20261019-v1.UNVALIDATED.adapter-negatives.rs.txt
// UNCOMPILED/UNRUN; append inside residual_admission.rs, not production.
// Actual unchanged loaded adapters and source guards must pass before each mutation.
// Electric control requires the real authorized metadata-extraction extension first.
#[cfg(test)]
mod tests {
    use super::*;
    fn approval(row: &Unused) -> AuditResult<Approval<'static>> {
        let _observed =
            super::super::scope_inputs::ObservedScope::read(&super::super::super::reference_dir())?;
        let accepted = super::super::policy_admission::source_acceptance(&row.atom.key)?
            .ok_or("actual accepted residual source missing")?;
        Ok(Approval {
            recorded_decision: accepted.recorded_decision,
            live_census_sha256: include_str!("ra9-approved-model-census-sha256.txt").trim(),
            original_section_closure_record: accepted.native_receipts,
            required_native_receipts_record: accepted.native_receipts,
        })
    }
    fn control(section: &str, key: &str) -> AuditResult<(Inputs, Definitions, Audit, Unused)> {
        let references = super::super::super::reference_dir();
        super::super::audit_at(&super::super::super::data_dir(), &references)?;
        let input = Inputs::load(&super::super::super::data_dir(), &references)?;
        let d = sim::load_ruleset(&input.data_dir).map_err(|e| e.to_string())?;
        let mut a = super::super::load_audit(&input, &d)?;
        let skirmish = sim::load_skirmish(input.data_dir.join("skirmish.ron"), &d)
            .map_err(|e| e.to_string())?;
        let errors = super::super::run_adapters(&mut a, &input, &d, &skirmish);
        assert!(errors.is_empty(), "{errors:?}");
        let row = super::super::source_routes::load_typed(
            &references.join("ra9_source_dispositions.tsv"),
        )?
        .into_iter()
        .map(|r| r.row)
        .find(|r| r.atom.key == SourceKey::new("rules", section, key))
        .ok_or("actual accepted source row")?;
        require(&a, &input, &d, &row, Some(&approval(&row)?))?;
        Ok((input, d, a, row))
    }
    #[test]
    fn ra9_residual_nonfirst_rot_source_rate_change_rejects() -> AuditResult<()> {
        let (mut input, d, a, row) = control("MTNK", "ROT")?;
        input
            .reference
            .rows
            .insert(SourceKey::new("rules", "MTNK", "ROT"), "6".into());
        assert!(require(&a, &input, &d, &row, Some(&approval(&row)?)).is_err());
        Ok(())
    }
    #[test]
    fn ra9_residual_sparky_false_cannot_cover_enablement() -> AuditResult<()> {
        let (mut input, d, a, row) = control("HE", "Sparky")?;
        input
            .reference
            .rows
            .insert(SourceKey::new("rules", "HE", "Sparky"), "yes".into());
        assert!(require(&a, &input, &d, &row, Some(&approval(&row)?)).is_err());
        Ok(())
    }
    #[test]
    fn ra9_residual_electric_spark_chain_cannot_become_damaging_fire() -> AuditResult<()> {
        let (mut input, d, a, row) = control("TankBolt", "IsElectricBolt")?;
        input.reference.rows.insert(
            SourceKey::new("rules", "SparkSys", "BehavesLike"),
            "Fire".into(),
        );
        assert!(require(&a, &input, &d, &row, Some(&approval(&row)?)).is_err());
        Ok(())
    }
    #[test]
    fn ra9_residual_new_particle_damage_breaks_absence_premise() -> AuditResult<()> {
        let (mut input, d, a, row) = control("TankBolt", "IsElectricBolt")?;
        input
            .reference
            .rows
            .insert(SourceKey::new("rules", "Spark", "Damage"), "1".into());
        assert!(require(&a, &input, &d, &row, Some(&approval(&row)?)).is_err());
        Ok(())
    }
    #[test]
    fn ra9_residual_turret_garrison_scope_change_rejects() -> AuditResult<()> {
        let (mut input, d, a, row) = control("GTGCAN", "Turret")?;
        input.reference.rows.insert(
            SourceKey::new("rules", "GTGCAN", "CanBeOccupied"),
            "yes".into(),
        );
        assert!(require(&a, &input, &d, &row, Some(&approval(&row)?)).is_err());
        Ok(())
    }
    #[test]
    fn ra9_residual_missing_approval_cannot_admit() -> AuditResult<()> {
        let (input, d, a, row) = control("MTNK", "ROT")?;
        assert!(require(&a, &input, &d, &row, None).is_err());
        Ok(())
    }
    #[test]
    fn ra9_residual_nonfirst_actual_elite_slot_mutation_is_detected() -> AuditResult<()> {
        // Baseline real loaded SREF ordered native adapter must succeed first.
        let input = Inputs::load(
            &super::super::super::data_dir(),
            &super::super::super::reference_dir(),
        )?;
        let d = sim::load_ruleset(&input.data_dir).map_err(|e| e.to_string())?;
        super::super::array_plans::run(&input, &d)?;
        let copy = super::super::array_negatives::DataCopy::replace_row(
            "combat.ron",
            "(unit: \"prism_tank\",",
            "elite_weapons: [\"super_comet\"]",
            "elite_weapons: [\"comet\"]",
        )?;
        let (input, d) = copy.loaded()?;
        assert!(
            super::super::array_plans::run(&input, &d).is_err(),
            "valid-loader nonfirst actual elite identity cannot hide behind single-slot policy"
        );
        Ok(())
    }
    #[test]
    fn ra9_residual_nonfirst_source_charge_slot_addition_is_detected() -> AuditResult<()> {
        let input = Inputs::load(
            &super::super::super::data_dir(),
            &super::super::super::reference_dir(),
        )?;
        let d = sim::load_ruleset(&input.data_dir).map_err(|e| e.to_string())?;
        super::super::array_plans::run(&input, &d)?;
        let mut changed = input;
        changed
            .reference
            .rows
            .insert(SourceKey::new("rules", "SREF", "WeaponCount"), "2".into());
        changed.reference.rows.insert(
            SourceKey::new("rules", "SREF", "Secondary"),
            "MirageGun".into(),
        );
        // Actual typed selector adapter must catch the second source/loaded mismatch; additionally
        // the exact charge policy's guard-records independently reject the added source selector.
        assert!(super::super::array_plans::run(&changed, &d).is_err());
        Ok(())
    }
}
