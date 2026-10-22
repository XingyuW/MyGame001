//! Historical preparation note: UNCOMPILED/UNRUN bounded38 documented-model source classifications. No coverage added.
//! A document establishes chosen policy, not source-engine truth or native execution.
use super::*;
const TABLE: &str = include_str!("carry38-contracts.tsv");
const ACCEPT: &str = include_str!("carry38-acceptance.tsv");
const UNITS: &str = include_str!("carry38-unit-gates.tsv");
const PHYSICAL: &str = include_str!("carry38-physical-demands.tsv");
const DOCS: &str = include_str!("carry38-document-guards.tsv");
#[derive(Clone)]
struct Contract {
    key: SourceKey,
    value: &'static str,
    group: &'static str,
    authorities: &'static str,
    reason: &'static str,
    bindings: &'static str,
    guard: &'static str,
    routes: &'static str,
}
pub(super) struct Plan {
    rows: BTreeMap<SourceKey, Contract>,
    physical: BTreeMap<String, BTreeSet<WitnessId>>,
    successful: BTreeSet<SourceKey>,
}
fn records(text: &str, width: usize) -> AuditResult<Vec<Vec<&str>>> {
    let header = match width {
        13 => {
            "file\tsection\tkey\texact_value\tgroup_id\tclassification\tprimary_line\tprimary_sha256\tdocument_authority_ids\tspecific_reason\tconsumer_bindings\tmodel_guard_id\trequired_native_routes"
        }
        10 => {
            "file\tsection\tkey\texact_value\tspecific_reason\tconsumer_bindings\tmodel_guard_id\trecorded_decision\tfinal_input_sha256\tnative_receipts"
        }
        9 => {
            "group_id\tsection\tgame_id\tactual_role\tactual_base_weapons\tactual_elite_weapons\tkind\tside\tnote"
        }
        3 => "group_id\tconsumer\tphysical_field",
        5 => "authority_id\tdocument_path\tsection_start\tsection_end\trequired_needle",
        _ => return Err("carry38 unknown table shape".into()),
    };
    if text.lines().next() != Some(header) {
        return Err("carry38 exact table header changed".into());
    }
    text.lines()
        .skip(1)
        .map(|s| {
            let r = s.split('\t').collect::<Vec<_>>();
            if r.len() != width {
                Err("carry38 table width".into())
            } else {
                Ok(r)
            }
        })
        .collect()
}
fn catalogue() -> AuditResult<BTreeMap<SourceKey, Contract>> {
    let mut rows = BTreeMap::new();
    let mut counts = BTreeMap::new();
    for r in records(TABLE, 13)? {
        let key = SourceKey::new(r[0], r[1], r[2]);
        let c = Contract {
            key: key.clone(),
            value: r[3],
            group: r[4],
            authorities: r[8],
            reason: r[9],
            bindings: r[10],
            guard: r[11],
            routes: r[12],
        };
        if c.reason.is_empty()
            || c.reason.starts_with("PENDING")
            || rows.insert(key, c.clone()).is_some()
        {
            return Err("carry38 duplicate/unspecific contract".into());
        }
        *counts.entry(c.group).or_insert(0usize) += 1;
    }
    if counts
        != [
            ("idle-acquisition", 11),
            ("moving-fire", 5),
            ("difficulty", 20),
            ("power-curve", 2),
        ]
        .into_iter()
        .collect()
        || rows.len() != 38
    {
        return Err("carry38 independent exact family census".into());
    }
    Ok(rows)
}
fn source(input: &Inputs, c: &Contract) -> AuditResult<()> {
    if input.reference.rows.get(&c.key).map(String::as_str) != Some(c.value) {
        return Err(format!("carry38 exact source atom changed {:?}", c.key));
    }
    Ok(())
}
fn no_credit(a: &Audit) -> AuditResult<()> {
    let keys = catalogue()?.into_keys().collect::<BTreeSet<_>>();
    if a.required_dependencies.iter().any(|(_,atom)|keys.contains(&atom.key))||a.witnesses.values().any(|w|matches!(w,Witness::Compared{dependencies,..}if dependencies.iter().any(|x|keys.contains(&x.key)))){return Err("carry38 nonread source falsely demanded/Compared".into());}
    Ok(())
}
fn documents(authorities: &str) -> AuditResult<()> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut seen = BTreeSet::new();
    let needed = authorities.split(',').collect::<BTreeSet<_>>();
    for r in records(DOCS, 5)? {
        if !needed.contains(r[0]) {
            continue;
        }
        if !seen.insert(r[0]) {
            return Err("carry38 duplicate document authority".into());
        }
        let text = std::fs::read_to_string(root.join(r[1])).map_err(|e| e.to_string())?;
        let at = text
            .find(r[2])
            .ok_or("carry38 documented model section missing")?;
        let end = text[at + r[2].len()..]
            .find(r[3])
            .map(|n| at + r[2].len() + n)
            .ok_or("carry38 exact policy end anchor missing; EOF widening forbidden")?;
        if !text[at..end].contains(r[4]) {
            return Err("carry38 exact current policy anchor changed".into());
        }
    }
    if seen != needed {
        return Err("carry38 missing requested document authority".into());
    }
    Ok(())
}
fn role_text(role: sim::Role) -> AuditResult<&'static str> {
    match role {
        sim::Role::Combat => Ok("combat"),
        sim::Role::Flak => Ok("flak"),
        sim::Role::Transport => Ok("transport"),
        sim::Role::Spy => Ok("spy"),
        sim::Role::Suicide => Ok("suicide"),
        sim::Role::MindControl => Ok("mind_control"),
        _ => Err("carry38 source type acquired a different role".into()),
    }
}
fn slots(d: &Definitions, terms: &sim::CombatTerms, elite: bool) -> AuditResult<String> {
    terms
        .weapons(elite)
        .into_iter()
        .flatten()
        .map(|k| {
            d.weapon(k)
                .map(|w| w.id.clone())
                .ok_or_else(|| "carry38 invalid loaded slot".into())
        })
        .collect::<AuditResult<Vec<_>>>()
        .map(|v| v.join(","))
}
fn unit_bindings(input: &Inputs, d: &Definitions, c: &Contract) -> AuditResult<String> {
    let r = records(UNITS, 9)?
        .into_iter()
        .find(|r| r[0] == c.group && r[1] == c.key.section)
        .ok_or("carry38 exact mapped unit policy missing")?;
    let matching = input
        .mappings
        .iter()
        .filter(|m| m.section == r[1] || m.game_id == r[2])
        .collect::<Vec<_>>();
    if matching.len() != 1
        || matching[0].game_id != r[2]
        || matching[0].building
        || matching[0].side != r[7]
        || matching[0].note != r[8]
    {
        return Err("carry38 actual mapping/alias/kind changed".into());
    }
    let k = d.unit_kind(r[2]).ok_or("carry38 real unit missing")?;
    let m = d
        .metadata(sim::ProductionItem::Unit(k))
        .ok_or("carry38 metadata missing")?;
    let t = d.unit_combat(k).ok_or("carry38 combat terms missing")?;
    let role = role_text(m.role)?;
    let base = slots(d, &t, false)?;
    let elite = slots(d, &t, true)?;
    if role != r[3] || base != r[4] || elite != r[5] {
        return Err(format!(
            "carry38 actual role/weapon policy changed {}",
            r[2]
        ));
    }
    if c.key.section == "SPY" {
        if t.weapons(false)[1].is_some() || t.weapons(true)[1].is_some() {
            return Err("carry38 Spy acquired a second passive weapon".into());
        }
        for k in t
            .weapons(false)
            .into_iter()
            .chain(t.weapons(true))
            .flatten()
        {
            let w = d.weapon(k).ok_or("Spy weapon")?;
            if !d.warhead(w.warhead).is_some_and(|h| h.makes_disguise) {
                return Err("carry38 Spy special-interaction exclusion premise lost".into());
            }
        }
    }
    if c.key.section == "PDPLANE" && (!m.domain.eq(&sim::Domain::Air) || base != "para_drop_weapon")
    {
        return Err("carry38 real strategic plane capability/identity changed".into());
    }
    Ok([r[2].to_owned(), role.into(), base, elite].join(";"))
}
fn selected_difficulty(d: &Definitions) -> AuditResult<String> {
    let r = d.rules().ok_or("carry38 Rules missing")?;
    if r.difficulties.len() != 3 {
        return Err("carry38 extra/missing loaded difficulty".into());
    }
    for (i, (id, rof, build)) in [
        ("easy", 1200, 1000),
        ("medium", 1000, 1000),
        ("brutal", 800, 800),
    ]
    .into_iter()
    .enumerate()
    {
        let row = &r.difficulties[i];
        if row.id != id || row.rof_pm != rof || row.build_time_pm != build {
            return Err("carry38 selected difficulty IDs/order/rates changed".into());
        }
    }
    // No Armor/speed/cost/etc getter exists. Actual native first-hit/motion/charge receipts
    // below are mandatory; definition equality alone cannot prove neutral runtime behavior.
    Ok(
        "Difficult/Normal/Easy=>easy/medium/brutal;ROF=1200,1000,800;BuildTime=1000,1000,800"
            .into(),
    )
}
fn selected_power(d: &Definitions) -> AuditResult<String> {
    let p = d.rules().ok_or("carry38 Rules missing")?.production;
    if (
        p.low_power_penalty_pm,
        p.min_low_power_speed_pm,
        p.max_low_power_speed_pm,
    ) != (1000, 500, 800)
    {
        return Err("carry38 chosen shortage scalar premises changed".into());
    }
    Ok("power_rate_pm:shortage-ratio;penalty=1000;min=500;max=800".into())
}
fn live(input: &Inputs, d: &Definitions, c: &Contract) -> AuditResult<String> {
    match c.group {
        "idle-acquisition" | "moving-fire" => unit_bindings(input, d, c),
        "difficulty" => selected_difficulty(d),
        "power-curve" => selected_power(d),
        _ => Err("carry38 foreign group".into()),
    }
}
pub(super) fn declare(input: &Inputs, d: &Definitions, a: &Audit) -> AuditResult<Plan> {
    let rows = catalogue()?;
    for c in rows.values() {
        source(input, c)?;
        documents(c.authorities)?;
        if live(input, d, c)? != c.bindings {
            return Err("carry38 exact live consumer binding changed".into());
        }
    }
    no_credit(a)?;
    let mut physical: BTreeMap<String, BTreeSet<WitnessId>> = BTreeMap::new();
    for r in records(PHYSICAL, 3)? {
        let w = a.physical(WitnessId::new(r[1], r[2]));
        if !a.required.contains(&w) {
            return Err(format!(
                "carry38 genuine physical consumer not independently declared {w:?}"
            ));
        }
        if !physical.entry(r[0].into()).or_default().insert(w) {
            return Err("carry38 duplicate physical demand".into());
        }
    }
    Ok(Plan {
        rows,
        physical,
        successful: BTreeSet::new(),
    })
}
fn compared(a: &Audit, w: &WitnessId) -> AuditResult<()> {
    match a.witnesses.get(w) {
        Some(Witness::Compared { dependencies, .. }) => {
            let need = a
                .required_dependencies
                .iter()
                .filter(|(id, _)| id == w)
                .map(|(_, x)| x.clone())
                .collect::<BTreeSet<_>>();
            if &need != dependencies {
                return Err(format!(
                    "carry38 actual successful dependencies differ from independent demand {w:?}"
                ));
            }
            Ok(())
        }
        _ => Err(format!("carry38 actual physical field not Compared {w:?}")),
    }
}
fn accepted_from<'a>(
    text: &'a str,
    key: &SourceKey,
) -> AuditResult<super::policy_admission::SourceAcceptance<'a>> {
    let mut seen = BTreeSet::new();
    let mut chosen = None;
    for r in records(text, 10)? {
        let k = SourceKey::new(r[0], r[1], r[2]);
        if !seen.insert(k.clone()) {
            return Err("carry38 duplicate acceptance".into());
        }
        if &k == key {
            chosen = Some(super::policy_admission::SourceAcceptance {
                key: k,
                exact: r[3],
                reason: r[4],
                consumer_bindings: r[5],
                model_guard_id: r[6],
                recorded_decision: r[7],
                final_input_sha256: r[8],
                native_receipts: r[9],
            });
        }
    }
    if seen != catalogue()?.into_keys().collect() {
        return Err("carry38 missing/foreign acceptance atom".into());
    }
    chosen.ok_or_else(|| "carry38 exact source decision absent".into())
}
fn native_routes(c: &Contract, text: &str) -> AuditResult<()> {
    let need = c.routes.split(';').collect::<BTreeSet<_>>();
    let mut have = BTreeSet::new();
    for v in text.split(';') {
        let (id, hash) = v
            .split_once('@')
            .ok_or("carry38 requires route@actual-log-sha256")?;
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) || !have.insert(id) {
            return Err("carry38 duplicate/malformed receipt".into());
        }
    }
    if need != have {
        return Err(
            "carry38 missing/foreign native route, including exact source-type scope".into(),
        );
    }
    Ok(())
}
fn admit_from(
    plan: &mut Plan,
    a: &Audit,
    input: &Inputs,
    d: &Definitions,
    row: &Unused,
    binding: &str,
    acceptance_text: &str,
) -> AuditResult<()> {
    let c = plan
        .rows
        .get(&row.atom.key)
        .ok_or("outside exact38 documented-model policy")?;
    source(input, c)?;
    documents(c.authorities)?;
    no_credit(a)?;
    if let Some(fields) = plan.physical.get(c.group) {
        for w in fields {
            compared(a, w)?;
        }
    }
    let live = live(input, d, c)?;
    let accepted = accepted_from(acceptance_text, &c.key)?;
    let proposal = super::policy_admission::SourceProposal {
        key: c.key.clone(),
        exact: c.value,
        reason: c.reason,
        consumer_bindings: c.bindings,
        model_guard_id: c.guard,
    };
    super::policy_admission::source_reason(input, row, &proposal, Some(&accepted), &live, binding)?;
    native_routes(c, accepted.native_receipts)?;
    if !plan.successful.insert(c.key.clone()) {
        return Err("carry38 duplicate source admission".into());
    }
    Ok(())
}
pub(super) fn admit(
    plan: &mut Plan,
    a: &Audit,
    input: &Inputs,
    d: &Definitions,
    row: &Unused,
    binding: &str,
) -> AuditResult<()> {
    admit_from(plan, a, input, d, row, binding, ACCEPT)
}
pub(super) fn finish(plan: &Plan, a: &Audit) -> AuditResult<()> {
    if plan.successful != plan.rows.keys().cloned().collect() {
        return Err(
            "carry38 demanded exact source classifications != successful admissions".into(),
        );
    }
    no_credit(a)
}

// Immutable declaration snapshot; no successful adapter output is consulted.
pub(super) fn demanded_sources(p: &Plan) -> BTreeMap<SourceKey, (String, String)> {
    p.rows
        .iter()
        .map(|(k, c)| (k.clone(), (c.value.into(), c.reason.into())))
        .collect()
}

// Installed actual prepared negative block: modelcarry38-integration-20261019-v1.UNVALIDATED.audit-negatives.rs.txt
// Historical preparation note: UNCOMPILED/UNRUN. Append inside carry38_admission.rs. Final acceptance-record negatives
// require a genuine fully integrated accepted baseline. NEVER assert shipped records remain
// UNACCEPTED, and NEVER fabricate accepted strings/logs simply to make these tests pass.
#[cfg(test)]
mod tests {
    use super::*;
    fn baseline() -> AuditResult<(Inputs, Definitions, Audit)> {
        let input = Inputs::load(
            &super::super::super::data_dir(),
            &super::super::super::reference_dir(),
        )?;
        let d = sim::load_ruleset(&input.data_dir).map_err(|e| e.to_string())?;
        let sk = sim::load_skirmish(input.data_dir.join("skirmish.ron"), &d)
            .map_err(|e| e.to_string())?;
        let mut a = super::super::load_audit(&input, &d)?;
        let _plan = declare(&input, &d, &a)?;
        let errors = super::super::run_adapters(&mut a, &input, &d, &sk);
        if !errors.is_empty() {
            return Err(format!(
                "carry38 unchanged actual adapter baseline failed: {errors:?}"
            ));
        }
        no_credit(&a)?;
        Ok((input, d, a))
    }
    fn row(c: &Contract) -> Unused {
        Unused {
            atom: Atom {
                key: c.key.clone(),
                portion: Portion::Whole,
            },
            exact_value: c.value.into(),
            reason: c.reason.into(),
            code_evidence: "carry38 exact current consumer/native policy gate".into(),
        }
    }
    #[test]
    fn ra9_carry38_independent_exact_source_and_actual_native_control() -> AuditResult<()> {
        let (input, _, a) = baseline()?;
        let rows = catalogue()?;
        assert_eq!(rows.len(), 38);
        for c in rows.values() {
            source(&input, c)?;
        }
        no_credit(&a)
    }
    #[test]
    fn ra9_carry38_nonfirst_easy_armor_source_mutation_stales_scope_not_fake_loaded_field()
    -> AuditResult<()> {
        let (mut input, d, _) = baseline()?;
        let key = SourceKey::new("rules", "Easy", "Armor");
        let before = selected_difficulty(&d)?;
        input.reference.rows.insert(key.clone(), "1.3".into());
        assert!(source(&input, catalogue()?.get(&key).ok_or("exact Easy Armor")?).is_err());
        let mut a = super::super::load_audit(&input, &d)?;
        super::super::rules::difficulties(&mut a, &input.reference, &d)?;
        assert_eq!(selected_difficulty(&d)?, before);
        no_credit(&a)
    }
    #[test]
    fn ra9_carry38_nonfirst_opportunity_flag_mutation_does_not_create_weapon_credit()
    -> AuditResult<()> {
        let (mut input, d, _) = baseline()?;
        let key = SourceKey::new("rules", "HTNK", "OpportunityFire");
        input.reference.rows.insert(key.clone(), "no".into());
        let mut a = super::super::load_audit(&input, &d)?;
        let r = input
            .mappings
            .iter()
            .find(|r| r.section == "HTNK")
            .ok_or("Rhino mapping")?;
        super::super::items::combat_terms(
            &mut a,
            &input.reference,
            &d,
            r,
            &super::super::index(&input.weapons),
        )?;
        assert!(source(&input, catalogue()?.get(&key).ok_or("exact Rhino flag")?).is_err());
        no_credit(&a)
    }
    #[test]
    fn ra9_carry38_actual_nonfirst_brutal_rof_mutation_hits_genuine_native_adapter()
    -> AuditResult<()> {
        let _ = baseline()?;
        let copy = super::super::array_negatives::DataCopy::replace_row(
            "rules.ron",
            "(id: \"brutal\",",
            "rof_pm: 800",
            "rof_pm: 801",
        )?;
        let (input, d) = copy.loaded()?;
        let mut a = super::super::load_audit(&input, &d)?;
        let e = super::super::rules::difficulties(&mut a, &input.reference, &d)
            .err()
            .ok_or("changed real Brutal ROF escaped genuine adapter")?;
        assert!(e.contains("rof_pm"), "{e}");
        assert!(selected_difficulty(&d).is_err());
        Ok(())
    }
    #[test]
    fn ra9_carry38_actual_power_minimum_mutation_hits_selected_scalar_not_old_coefficients()
    -> AuditResult<()> {
        let _ = baseline()?;
        let copy = super::super::array_negatives::DataCopy::replace_row(
            "rules.ron",
            "min_low_power_speed_pm:",
            "min_low_power_speed_pm: 500",
            "min_low_power_speed_pm: 501",
        )?;
        let (input, d) = copy.loaded()?;
        let mut a = super::super::load_audit(&input, &d)?;
        let e = super::super::combat::scalars(&mut a, &input.reference, &d)
            .err()
            .ok_or("changed actual selected power scalar escaped adapter")?;
        assert!(e.contains("min_low_power_speed_pm"), "{e}");
        assert!(selected_power(&d).is_err());
        Ok(())
    }
    #[test]
    fn ra9_carry38_spy_actual_snapshot_flag_change_invalidates_conditional_boundary()
    -> AuditResult<()> {
        let _ = baseline()?;
        let copy = super::super::array_negatives::DataCopy::replace_row(
            "combat.ron",
            "(id: \"snapshot\",",
            "makes_disguise: 1",
            "makes_disguise: 0",
        )?;
        let (input, d) = copy.loaded()?;
        let c = catalogue()?
            .remove(&SourceKey::new("rules", "SPY", "CanPassiveAquire"))
            .ok_or("exact Spy contract")?;
        let e = unit_bindings(&input, &d, &c)
            .err()
            .ok_or("changed actual Spy special-interaction premise escaped guard")?;
        assert!(e.contains("special-interaction"), "{e}");
        Ok(())
    }
    #[test]
    fn ra9_carry38_actual_plane_weapon_change_invalidates_conditional_scope() -> AuditResult<()> {
        let _ = baseline()?;
        let copy = super::super::array_negatives::DataCopy::replace_row(
            "combat.ron",
            "(unit: \"paradrop_plane\",",
            ", weapons: [\"para_drop_weapon\"]",
            ", weapons: [\"black_hawk_cannon\"]",
        )?;
        let (input, d) = copy.loaded()?;
        let c = catalogue()?
            .remove(&SourceKey::new("rules", "PDPLANE", "CanPassiveAquire"))
            .ok_or("exact plane contract")?;
        assert!(unit_bindings(&input, &d, &c).is_err());
        Ok(())
    }
    #[test]
    fn ra9_carry38_controlled_unaccepted_record_rejects_actual_admission_after_genuine_accepted_baseline()
    -> AuditResult<()> {
        let (input, d, a) = baseline()?;
        let key = SourceKey::new("rules", "Easy", "Armor");
        let mut plan = declare(&input, &d, &a)?;
        let c = plan.rows.get(&key).ok_or("actual contract")?.clone();
        let candidate = row(&c);
        let binding = super::super::policy_admission::input_binding();
        admit(&mut plan, &a, &input, &d, &candidate, binding)?;
        let mut changed = Vec::new();
        for line in ACCEPT.lines() {
            let mut parts = line.split('\t').collect::<Vec<_>>();
            if parts.len() == 10 && parts[0] == "rules" && parts[1] == "Easy" && parts[2] == "Armor"
            {
                parts[7] = "UNACCEPTED";
            }
            changed.push(parts.join("\t"));
        }
        let text = changed.join("\n") + "\n";
        let mut fresh = declare(&input, &d, &a)?;
        let e = admit_from(&mut fresh, &a, &input, &d, &candidate, binding, &text)
            .err()
            .ok_or("controlled actual unaccepted decision escaped admission")?;
        assert!(e.contains("decision") || e.contains("receipts"), "{e}");
        Ok(())
    }
    #[test]
    fn ra9_carry38_controlled_reason_mutation_rejects_after_genuine_accepted_baseline()
    -> AuditResult<()> {
        let (input, d, a) = baseline()?;
        let key = SourceKey::new("rules", "General", "WorstLowPowerBuildRateCoefficient");
        let mut p = declare(&input, &d, &a)?;
        let c = p.rows.get(&key).ok_or("exact old coefficient")?.clone();
        let candidate = row(&c);
        let binding = super::super::policy_admission::input_binding();
        admit(&mut p, &a, &input, &d, &candidate, binding)?;
        let mut wrong = candidate;
        wrong.reason = "Generic documented unused".into();
        let mut p = declare(&input, &d, &a)?;
        assert!(admit(&mut p, &a, &input, &d, &wrong, binding).is_err());
        Ok(())
    }
}
