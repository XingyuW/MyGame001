//! UNCOMPILED/UNRUN exact87 current-model classifications. No runtime feature or coverage.
//! Real loaded policy premises are not source Expected values for the87 nonread literals.
use super::*;
const ATOMS: &str = include_str!("behavior87-atoms.tsv");
const CONTRACTS: &str = include_str!("behavior87-contracts.tsv");
const ACCEPT: &str = include_str!("behavior87-acceptance.tsv");
const GATES: &str = include_str!("behavior87-loaded-gates.tsv");
const MAPPING: &str = include_str!("behavior87-mapping-guards.tsv");
const PHYSICAL: &str = include_str!("behavior87-physical-demands.tsv");
const ANCHORS: &str = include_str!("behavior87-scope-anchors.tsv");
#[derive(Clone)]
struct Contract {
    key: SourceKey,
    value: &'static str,
    group: &'static str,
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
fn records<'a>(text: &'a str, header: &str, width: usize) -> AuditResult<Vec<Vec<&'a str>>> {
    let mut lines = text.lines();
    if lines.next() != Some(header) {
        return Err("behavior87 exact table header changed".into());
    }
    lines
        .map(|line| {
            let r = line.split('\t').collect::<Vec<_>>();
            if r.len() != width {
                Err("behavior87 table width".into())
            } else {
                Ok(r)
            }
        })
        .collect()
}
fn groups() -> BTreeMap<&'static str, usize> {
    [
        ("attackmove-gate", 9),
        ("move-shroud", 13),
        ("combat-selection", 40),
        ("radar-visibility", 14),
        ("decloak-fire", 6),
        ("parasite", 3),
        ("legal-target", 2),
    ]
    .into_iter()
    .collect()
}
type ScopeValue = (&'static str, &'static str, &'static str, &'static str);
fn scope() -> AuditResult<BTreeMap<SourceKey, ScopeValue>> {
    let mut result = BTreeMap::new();
    let mut counts = BTreeMap::new();
    for r in records(
        ATOMS,
        "file\tsection\tkey\tportion\texact_value\tmaster_row\tpending_family\tprimary_line\tprimary_written_section\tprimary_written_key\tprimary_sha256\tgroup_id\tproposal\tstatus",
        14,
    )? {
        if r[3] != "whole"
            || r[12] != "BEHAVIOR_CHOICE"
            || r[10] != "adc869ca037db5141f562f18092d022045e194de0d1204a46b9ec8372b2b826f"
            || r[7].parse::<usize>().is_err()
        {
            return Err("behavior87 exact primary provenance changed".into());
        }
        if result
            .insert(SourceKey::new(r[0], r[1], r[2]), (r[4], r[11], r[7], r[10]))
            .is_some()
        {
            return Err("behavior87 duplicate scope atom".into());
        }
        *counts.entry(r[11]).or_insert(0usize) += 1;
    }
    if result.len() != 87 || counts != groups() {
        return Err("behavior87 independent exact87/seven-group census".into());
    }
    Ok(result)
}
fn catalogue() -> AuditResult<BTreeMap<SourceKey, Contract>> {
    let expected = scope()?;
    let mut result = BTreeMap::new();
    for r in records(
        CONTRACTS,
        "file\tsection\tkey\texact_value\tgroup_id\tclassification\tprimary_line\tprimary_sha256\tspecific_reason\tconsumer_bindings\tmodel_guard_id\trequired_routes",
        12,
    )? {
        let key = SourceKey::new(r[0], r[1], r[2]);
        if expected.get(&key) != Some(&(r[3], r[4], r[6], r[7]))
            || r[8].is_empty()
            || r[9].is_empty()
            || r[10].is_empty()
            || r[11].is_empty()
            || !matches!(
                r[5],
                "CURRENT_MODEL_NONREAD"
                    | "INTENTIONAL_CURRENT_MODEL_DIFFERENCE"
                    | "CURRENT_GROUPING_MODEL_WITH_ORIGINAL_SELECTION_UNCERTAINTY"
                    | "CURRENT_TARGET_SELECTION_MODEL_WITH_SOURCE_UNCERTAINTY"
            )
        {
            return Err("behavior87 foreign/unspecific exact contract".into());
        }
        let c = Contract {
            key: key.clone(),
            value: r[3],
            group: r[4],
            reason: r[8],
            bindings: r[9],
            guard: r[10],
            routes: r[11],
        };
        if result.insert(key, c).is_some() {
            return Err("behavior87 duplicate contract".into());
        }
    }
    if result.keys().cloned().collect::<BTreeSet<_>>() != expected.into_keys().collect() {
        return Err("behavior87 missing/extra exact contract".into());
    }
    Ok(result)
}
fn source(input: &Inputs, c: &Contract) -> AuditResult<()> {
    if input.reference.rows.get(&c.key).map(String::as_str) != Some(c.value) {
        return Err(format!(
            "behavior87 exact source literal changed {:?}",
            c.key
        ));
    }
    Ok(())
}
fn no_credit(a: &Audit) -> AuditResult<()> {
    let keys = scope()?.into_keys().collect::<BTreeSet<_>>();
    if a.required_dependencies.iter().any(|(_,atom)|keys.contains(&atom.key))||a.witnesses.values().any(|w|matches!(w,Witness::Compared{dependencies,..}if dependencies.iter().any(|atom|keys.contains(&atom.key)))){return Err("behavior87 nonread literal falsely demanded/Compared; lookup is not coverage".into());}
    Ok(())
}
fn mapping(input: &Inputs) -> AuditResult<()> {
    let expected = records(MAPPING, "game_id\tkind\tsection\tside\tnote", 5)?
        .into_iter()
        .map(|r| {
            (
                r[0].to_owned(),
                r[1] == "building",
                r[2].to_owned(),
                r[3].to_owned(),
                r[4].to_owned(),
            )
        })
        .collect::<BTreeSet<_>>();
    if expected.len() != 59 {
        return Err("behavior87 exact actual mapped/twin census changed".into());
    }
    let subjects = expected
        .iter()
        .map(|r| r.0.as_str())
        .collect::<BTreeSet<_>>();
    let actual = input
        .mappings
        .iter()
        .filter(|r| subjects.contains(r.game_id.as_str()))
        .map(|r| {
            (
                r.game_id.clone(),
                r.building,
                r.section.clone(),
                r.side.clone(),
                r.note.clone(),
            )
        })
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err("behavior87 actual mapping/twin/alias/kind changed".into());
    }
    Ok(())
}
fn span(text: &str, start: &str, end: &str, needle: &str) -> AuditResult<()> {
    let from = text.find(start).ok_or("behavior87 scope start absent")?;
    let to = text[from + start.len()..]
        .find(end)
        .map(|n| from + start.len() + n)
        .ok_or("behavior87 scope end absent; EOF widening forbidden")?;
    if !text[from..to]
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
    {
        return Err("behavior87 exact current-model scope changed".into());
    }
    Ok(())
}
fn static_scope(group: &str) -> AuditResult<()> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut found = false;
    for r in records(
        ANCHORS,
        "group_id\tpath\tsection_start\tsection_end\trequired_case_insensitive_needle",
        5,
    )?
    .into_iter()
    .filter(|r| r[0] == group)
    {
        if found {
            return Err("behavior87 duplicate group scope anchor".into());
        }
        found = true;
        let text = std::fs::read_to_string(root.join(r[1])).map_err(|e| e.to_string())?;
        span(&text, r[2], r[3], r[4])?;
    }
    if !found {
        return Err("behavior87 group scope anchor absent".into());
    }
    Ok(())
}
fn item(d: &Definitions, id: &str) -> AuditResult<sim::ProductionItem> {
    if let Some(k) = d.unit_kind(id) {
        Ok(sim::ProductionItem::Unit(k))
    } else {
        d.building_kind(id)
            .map(sim::ProductionItem::Building)
            .ok_or_else(|| format!("behavior87 real item absent {id}"))
    }
}
fn observe(d: &Definitions, id: &str, f: &str) -> AuditResult<String> {
    if id == "rules" && f == "combat.cloak_delay" {
        return Ok(d
            .rules()
            .ok_or("behavior87 rules absent")?
            .combat
            .cloak_delay
            .to_string());
    }
    if let Some(f) = f.strip_prefix("weapon.") {
        let w = d
            .weapon(d.weapon_kind(id).ok_or("behavior87 actual weapon absent")?)
            .ok_or("behavior87 actual weapon absent")?;
        return match f {
            "damage" => Ok(w.damage.to_string()),
            "range" => Ok(w.range.to_string()),
            "reload" => Ok(w.reload.to_string()),
            "warhead" => d
                .warhead(w.warhead)
                .map(|h| h.id.clone())
                .ok_or_else(|| "behavior87 actual warhead absent".into()),
            "anti_air" => Ok(u8::from(w.anti_air).to_string()),
            "anti_ground" => Ok(u8::from(w.anti_ground).to_string()),
            _ => Err("behavior87 unknown real weapon policy premise".into()),
        };
    }
    if f == "weapons" || f == "elite_weapons" || f.starts_with("combat.") {
        let t = d
            .unit_combat(d.unit_kind(id).ok_or("behavior87 actual unit absent")?)
            .ok_or("behavior87 real combat terms absent")?;
        if f == "weapons" || f == "elite_weapons" {
            return t
                .weapons(f == "elite_weapons")
                .into_iter()
                .flatten()
                .map(|k| {
                    d.weapon(k)
                        .map(|w| w.id.clone())
                        .ok_or_else(|| "behavior87 invalid real slot".into())
                })
                .collect::<AuditResult<Vec<_>>>()
                .map(|v| v.join(","));
        }
        return match f {
            "combat.underwater" => Ok(u8::from(t.underwater).to_string()),
            "combat.organic" => Ok(u8::from(t.organic).to_string()),
            "combat.naval_targeting" => Ok(t.naval_targeting.to_string()),
            "combat.land_targeting" => Ok(t.land_targeting.to_string()),
            _ => Err("behavior87 unknown real combat policy premise".into()),
        };
    }
    let m = d
        .metadata(item(d, id)?)
        .ok_or("behavior87 actual metadata absent")?;
    match f {
        "role" => Ok(format!("{:?}", m.role)),
        "domain" => Ok(format!("{:?}", m.domain)),
        "cloakable" => Ok(m.cloakable.to_string()),
        "infantry" => Ok(m.infantry.to_string()),
        "airport_bound" => Ok(m.airport_bound.to_string()),
        "ammo" => Ok(m.ammo.to_string()),
        "spawns_number" => Ok(m.spawns_number.to_string()),
        "spawns" => m
            .spawns
            .map(|k| {
                d.unit(k)
                    .map(|u| u.id.clone())
                    .ok_or_else(|| "behavior87 invalid real spawn kind".into())
            })
            .transpose()
            .map(|v| v.unwrap_or_else(|| "none".into())),
        "launcher" => Ok(m
            .launcher
            .map(|v| format!("{v:?}").to_ascii_lowercase())
            .unwrap_or_else(|| "none".into())),
        "strategic" => Ok(m.strategic.to_string()),
        "cooldown" => Ok(m.cooldown.to_string()),
        "is_powered" => Ok(m.is_powered.to_string()),
        "immune" => Ok(m.immune.to_string()),
        "sensors_sight" => Ok(m.sensors_sight.to_string()),
        _ => Err("behavior87 invented or unknown policy getter forbidden".into()),
    }
}
fn loaded(d: &Definitions, group: &str) -> AuditResult<String> {
    let mut facts = Vec::new();
    let mut seen = BTreeSet::new();
    for r in records(GATES, "group_id\tsubject\tfield\tactual_value", 4)?
        .into_iter()
        .filter(|r| r[0] == group)
    {
        if !seen.insert((r[1], r[2])) {
            return Err("behavior87 duplicate actual loaded gate".into());
        }
        let value = observe(d, r[1], r[2])?;
        if value != r[3] {
            return Err(format!(
                "behavior87 actual loaded premise changed {}.{} actual {value} expected {}",
                r[1], r[2], r[3]
            ));
        }
        facts.push(format!("{}.{}={value}", r[1], r[2]));
    }
    if facts.is_empty() {
        return Err("behavior87 no genuine loaded group premises".into());
    }
    Ok(facts.join(";"))
}
pub(super) fn declare(input: &Inputs, d: &Definitions, a: &Audit) -> AuditResult<Plan> {
    let rows = catalogue()?;
    mapping(input)?;
    no_credit(a)?;
    for c in rows.values() {
        source(input, c)?;
        static_scope(c.group)?;
        if loaded(d, c.group)? != c.bindings {
            return Err("behavior87 exact current consumer binding changed".into());
        }
    }
    let mut physical: BTreeMap<String, BTreeSet<WitnessId>> = BTreeMap::new();
    let mut count = 0;
    for r in records(PHYSICAL, "group_id\tconsumer\tphysical_field", 3)? {
        if !groups().contains_key(r[0]) {
            return Err("behavior87 foreign demanded physical group".into());
        }
        let w = a.physical(WitnessId::new(r[1], r[2]));
        if !a.required.contains(&w) {
            return Err(format!(
                "behavior87 physical dependency not independently declared {w:?}"
            ));
        }
        if !physical.entry(r[0].into()).or_default().insert(w) {
            return Err("behavior87 duplicate resolved physical demand".into());
        }
        count += 1;
    }
    if count != 437
        || physical.keys().map(String::as_str).collect::<BTreeSet<_>>()
            != groups().into_keys().collect()
    {
        return Err("behavior87 independent physical demand census changed".into());
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
                .map(|(_, atom)| atom.clone())
                .collect::<BTreeSet<_>>();
            if &need != dependencies {
                return Err(format!(
                    "behavior87 independent physical demand dependencies != successful dependencies {w:?}"
                ));
            }
            Ok(())
        }
        _ => Err(format!(
            "behavior87 genuine physical dependency not Compared {w:?}"
        )),
    }
}
fn accepted_from<'a>(
    text: &'a str,
    key: &SourceKey,
) -> AuditResult<super::policy_admission::SourceAcceptance<'a>> {
    let mut seen = BTreeSet::new();
    let mut chosen = None;
    for r in records(
        text,
        "file\tsection\tkey\texact_value\tspecific_reason\tconsumer_bindings\tmodel_guard_id\trecorded_decision\tfinal_input_sha256\tnative_receipts",
        10,
    )? {
        let k = SourceKey::new(r[0], r[1], r[2]);
        if !seen.insert(k.clone()) {
            return Err("behavior87 duplicate acceptance key".into());
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
    if seen != scope()?.into_keys().collect() {
        return Err("behavior87 missing/foreign individual acceptance atom".into());
    }
    chosen.ok_or_else(|| "behavior87 missing exact source decision".into())
}
fn routes(c: &Contract, text: &str) -> AuditResult<()> {
    let need = c.routes.split(';').collect::<BTreeSet<_>>();
    let mut have = BTreeSet::new();
    for token in text.split(';') {
        let (route, hash) = token
            .split_once('@')
            .ok_or("behavior87 requires scoped route@actual-record-sha256")?;
        if !["static/", "loaded/", "native/", "client/"]
            .iter()
            .any(|prefix| route.starts_with(prefix))
            || hash.len() != 64
            || !hash.bytes().all(|b| b.is_ascii_hexdigit())
            || !have.insert(route)
        {
            return Err("behavior87 malformed/duplicate route record".into());
        }
    }
    if have != need {
        return Err(
            "behavior87 missing/foreign exact route record; client/static/loaded is not native"
                .into(),
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
    acceptances: &str,
) -> AuditResult<()> {
    if binding.len() != 64 || !binding.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("behavior87 actual final input SHA binding absent".into());
    }
    let c = plan
        .rows
        .get(&row.atom.key)
        .ok_or("outside exact87 current-model classifications")?;
    source(input, c)?;
    mapping(input)?;
    static_scope(c.group)?;
    no_credit(a)?;
    for w in plan
        .physical
        .get(c.group)
        .ok_or("behavior87 missing independent group demands")?
    {
        compared(a, w)?;
    }
    let live = loaded(d, c.group)?;
    let acceptance = accepted_from(acceptances, &c.key)?;
    let proposal = super::policy_admission::SourceProposal {
        key: c.key.clone(),
        exact: c.value,
        reason: c.reason,
        consumer_bindings: c.bindings,
        model_guard_id: c.guard,
    };
    super::policy_admission::source_reason(
        input,
        row,
        &proposal,
        Some(&acceptance),
        &live,
        binding,
    )?;
    routes(c, acceptance.native_receipts)?;
    if !plan.successful.insert(c.key.clone()) {
        return Err("behavior87 duplicate exact source admission".into());
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
            "behavior87 independently demanded classifications != successful exact87 admissions"
                .into(),
        );
    }
    for fields in plan.physical.values() {
        for w in fields {
            compared(a, w)?;
        }
    }
    no_credit(a)
}

pub(super) fn demanded_sources(p: &Plan) -> BTreeMap<SourceKey, (String, String)> {
    p.rows
        .iter()
        .map(|(k, c)| (k.clone(), (c.value.into(), c.reason.into())))
        .collect()
}

pub(super) fn owns(plan: &Plan, key: &SourceKey) -> bool {
    plan.rows.contains_key(key)
}
const _: fn(&Plan, &SourceKey) -> bool = owns;

// Installed actual prepared negative block: behavior87-20261019-v1.UNVALIDATED.audit-negatives.rs.txt
// UNCOMPILED/UNRUN. Append inside behavior87_admission.rs after actual final integration.
// Every negative FIRST requires the genuinely accepted installed final audit baseline.
// Current UNACCEPTED tables are not a passing baseline, nor an expected test result.
#[cfg(test)]
mod tests {
    use super::*;
    fn baseline() -> AuditResult<(Inputs, Definitions, Audit, BTreeMap<SourceKey, Unused>)> {
        let data = super::super::super::data_dir();
        let references = super::super::super::reference_dir();
        super::super::audit_at(&data, &references)?;
        let input = Inputs::load(&data, &references)?;
        let d = sim::load_ruleset(&data).map_err(|e| e.to_string())?;
        let sk = sim::load_skirmish(data.join("skirmish.ron"), &d).map_err(|e| e.to_string())?;
        let mut a = super::super::load_audit(&input, &d)?;
        let mut plan = declare(&input, &d, &a)?;
        let errors = super::super::run_adapters(&mut a, &input, &d, &sk);
        if !errors.is_empty() {
            return Err(format!(
                "behavior87 actual unchanged comparison baseline: {errors:?}"
            ));
        }
        let mut installed = BTreeMap::new();
        // Use the actual installed typed compositor table, not the obsolete seven-column
        // draft filename. The full accepted audit above independently validates ownership.
        for classified in super::super::source_routes::load_typed(
            &references.join("ra9_source_dispositions.tsv"),
        )? {
            let row = classified.row;
            let key = row.atom.key.clone();
            if !owns(&plan, &key) {
                continue;
            }
            if row.atom.portion != Portion::Whole {
                return Err("behavior87 actual installed source portion changed".into());
            }
            admit(
                &mut plan,
                &a,
                &input,
                &d,
                &row,
                super::super::policy_admission::input_binding(),
            )?;
            if installed.insert(key, row).is_some() {
                return Err("behavior87 actual duplicate installed disposition".into());
            }
        }
        finish(&plan, &a)?;
        Ok((input, d, a, installed))
    }
    fn edit_record(key: &SourceKey, column: usize, replacement: &str) -> AuditResult<String> {
        let mut found = 0;
        let mut lines = Vec::new();
        for line in ACCEPT.lines() {
            let mut r = line.split('\t').map(str::to_owned).collect::<Vec<_>>();
            if r.len() == 10 && r[0] == key.file && r[1] == key.section && r[2] == key.key {
                r[column] = replacement.into();
                found += 1;
            }
            lines.push(r.join("\t"));
        }
        if found != 1 {
            return Err("behavior87 controlled actual record mutation not unique".into());
        }
        Ok(lines.join("\n") + "\n")
    }
    #[test]
    fn ra9_behavior87_actual_final_accepted_scope_and_real_compared_control() -> AuditResult<()> {
        let (input, d, a, rows) = baseline()?;
        assert_eq!(rows.len(), 87);
        let _ = declare(&input, &d, &a)?;
        no_credit(&a)
    }
    #[test]
    fn ra9_behavior87_nonfirst_source_true_mutation_stales_only_exact_contract() -> AuditResult<()>
    {
        let (mut input, d, a, _) = baseline()?;
        let key = SourceKey::new("rules", "YURIPR", "IsSelectableCombatant");
        let before = loaded(&d, "combat-selection")?;
        input.reference.rows.insert(key, "no".into());
        let error = declare(&input, &d, &a)
            .err()
            .ok_or("changed actual exact source escaped declare")?;
        assert!(error.contains("source literal"), "{error}");
        assert_eq!(loaded(&d, "combat-selection")?, before);
        Ok(())
    }
    #[test]
    fn ra9_behavior87_source_false_is_guarded_and_never_fake_loaded_credit() -> AuditResult<()> {
        let (mut input, d, a, _) = baseline()?;
        let key = SourceKey::new("rules", "ENGINEER", "IsSelectableCombatant");
        let before = loaded(&d, "combat-selection")?;
        input.reference.rows.insert(key, "yes".into());
        assert!(declare(&input, &d, &a).is_err());
        assert_eq!(loaded(&d, "combat-selection")?, before);
        no_credit(&a)
    }
    #[test]
    fn ra9_behavior87_actual_successful_comparison_removal_fails_before_admission()
    -> AuditResult<()> {
        let (input, d, mut a, rows) = baseline()?;
        let mut plan = declare(&input, &d, &a)?;
        let w = plan.physical["decloak-fire"]
            .iter()
            .next()
            .ok_or("real demand")?
            .clone();
        assert!(
            matches!(a.witnesses.remove(&w), Some(Witness::Compared { .. })),
            "remove actual successful baseline comparison"
        );
        let row = rows
            .get(&SourceKey::new("rules", "SonicZapE", "DecloakToFire"))
            .ok_or("installed exact row")?;
        let error = admit(
            &mut plan,
            &a,
            &input,
            &d,
            row,
            super::super::policy_admission::input_binding(),
        )
        .err()
        .ok_or("missing actual comparison escaped")?;
        assert!(error.contains("not Compared"), "{error}");
        Ok(())
    }
    #[test]
    fn ra9_behavior87_actual_successful_dependency_mismatch_fails_before_admission()
    -> AuditResult<()> {
        let (input, d, mut a, rows) = baseline()?;
        let mut plan = declare(&input, &d, &a)?;
        let pair = a
            .required_dependencies
            .iter()
            .find(|(w, _)| plan.physical["decloak-fire"].contains(w))
            .ok_or("actual independently demanded source dependency")?
            .clone();
        assert!(a.required_dependencies.remove(&pair));
        let row = rows
            .get(&SourceKey::new("rules", "SubTorpedoE", "DecloakToFire"))
            .ok_or("installed exact row")?;
        let error = admit(
            &mut plan,
            &a,
            &input,
            &d,
            row,
            super::super::policy_admission::input_binding(),
        )
        .err()
        .ok_or("changed independent dependency escaped")?;
        assert!(error.contains("dependencies !="), "{error}");
        Ok(())
    }
    #[test]
    fn ra9_behavior87_nonfirst_actual_acceptance_decision_mutation_fails_closed() -> AuditResult<()>
    {
        let (input, d, a, rows) = baseline()?;
        let key = SourceKey::new("rules", "CATHOSP", "LegalTarget");
        let bad = edit_record(&key, 7, "UNACCEPTED")?;
        let mut plan = declare(&input, &d, &a)?;
        let row = rows.get(&key).ok_or("installed exact row")?;
        let error = admit_from(
            &mut plan,
            &a,
            &input,
            &d,
            row,
            super::super::policy_admission::input_binding(),
            &bad,
        )
        .err()
        .ok_or("controlled unaccepted decision escaped")?;
        assert!(error.contains("decision"), "{error}");
        Ok(())
    }
    #[test]
    fn ra9_behavior87_remove_one_real_native_route_from_actual_acceptance_fails() -> AuditResult<()>
    {
        let (input, d, a, rows) = baseline()?;
        let key = SourceKey::new("rules", "CATHOSP", "LegalTarget");
        let actual = accepted_from(ACCEPT, &key)?;
        let mut remaining = actual.native_receipts.split(';').collect::<Vec<_>>();
        let at = remaining
            .iter()
            .position(|s| s.starts_with("native/b87/direct-target/"))
            .ok_or("actual accepted native route")?;
        remaining.remove(at);
        let bad = edit_record(&key, 9, &remaining.join(";"))?;
        let mut plan = declare(&input, &d, &a)?;
        let error = admit_from(
            &mut plan,
            &a,
            &input,
            &d,
            rows.get(&key).ok_or("installed exact row")?,
            super::super::policy_admission::input_binding(),
            &bad,
        )
        .err()
        .ok_or("missing real route escaped")?;
        assert!(error.contains("route record"), "{error}");
        Ok(())
    }
    #[test]
    fn ra9_behavior87_actual_installed_reason_change_fails_exact_model_guard() -> AuditResult<()> {
        let (input, d, a, rows) = baseline()?;
        let key = SourceKey::new("rules", "SQD", "Parasiteable");
        let mut row = rows.get(&key).ok_or("installed exact row")?.clone();
        row.reason = "Generic no flag unused".into();
        let mut plan = declare(&input, &d, &a)?;
        assert!(
            admit(
                &mut plan,
                &a,
                &input,
                &d,
                &row,
                super::super::policy_admission::input_binding()
            )
            .is_err()
        );
        Ok(())
    }
    #[test]
    fn ra9_behavior87_actual_nonfirst_organic_change_hits_real_adapter_and_policy()
    -> AuditResult<()> {
        let _ = baseline()?;
        let copy = super::super::array_negatives::DataCopy::replace_row(
            "combat.ron",
            "(unit: \"giant_squid\",",
            "organic: 1",
            "organic: 0",
        )?;
        let (input, d) = copy.loaded()?;
        let mut a = super::super::load_audit(&input, &d)?;
        let row = input
            .mappings
            .iter()
            .find(|r| r.section == "SQD")
            .ok_or("actual SQD mapping")?;
        let error = super::super::items::combat_terms(
            &mut a,
            &input.reference,
            &d,
            row,
            &super::super::index(&input.weapons),
        )
        .err()
        .ok_or("changed actual Organic escaped real adapter")?;
        assert!(error.contains("organic"), "{error}");
        assert!(loaded(&d, "parasite").is_err());
        Ok(())
    }
    #[test]
    fn ra9_behavior87_missing_scope_end_never_accepts_eof_widening() -> AuditResult<()> {
        let _ = baseline()?;
        let text = "start\ncurrent policy\nend\nunrelated";
        span(text, "start", "end", "current policy")?;
        assert!(span(text, "start", "absent-end", "current policy").is_err());
        Ok(())
    }
}
