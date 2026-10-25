//! Historical preparation note: UNCOMPILED/UNRUN exact162 current-model source classifications,32 families.
//! No fake physical flag, constant NativeWitness, source read/alias credit or new mechanics.
use super::*;
#[path = "model162-loaded.rs"]
mod loaded;
const TABLE: &str = include_str!("model162-contracts.tsv");
const ACCEPT: &str = include_str!("model162-acceptance.tsv");
const FACTS: &str = include_str!("model162-loaded-gates.tsv");
const MAPS: &str = include_str!("model162-mapping-guards.tsv");
const PHYSICAL: &str = include_str!("model162-physical-demands.tsv");
const DOCS: &str = include_str!("model162-document-guards.tsv");
const JETS: &str = include_str!("model162-jumpjet-premises.tsv");
const SHAPES: &str = include_str!("model162-source-shapes.tsv");
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
fn records<'a>(text: &'a str, header: &str) -> AuditResult<Vec<Vec<&'a str>>> {
    if text.lines().next() != Some(header) {
        return Err("model162 exact header".into());
    }
    let n = header.split('\t').count();
    text.lines()
        .skip(1)
        .map(|s| {
            let p = s.split('\t').collect::<Vec<_>>();
            if p.len() != n {
                Err("model162 table row width".into())
            } else {
                Ok(p)
            }
        })
        .collect()
}
fn families() -> BTreeMap<&'static str, usize> {
    [
        ("retaliation", 6),
        ("idle-area-guard", 4),
        ("names", 16),
        ("nonhuman", 3),
        ("reveal-fire", 5),
        ("cloak-stages", 4),
        ("approach-mirage", 1),
        ("suppression", 4),
        ("stupid-hunt", 4),
        ("crush-auto", 3),
        ("scatter-controls", 2),
        ("dock-unload", 2),
        ("air-model", 19),
        ("air-crash", 3),
        ("jumpjet-defaults", 7),
        ("deploy-times", 7),
        ("gap-boost", 2),
        ("allied-adjacency", 2),
        ("lobby-defaults", 5),
        ("lobby-modes", 4),
        ("ore-combat", 4),
        ("crates-material", 4),
        ("crew-fallback", 2),
        ("fall-kinematics", 2),
        ("radar-events", 8),
        ("fx-world", 16),
        ("waypoint-count", 1),
        ("target-cadence", 9),
        ("harvest-routing", 3),
        ("chrono-controls", 3),
        ("threat-score", 3),
        ("harvester-immunity", 4),
    ]
    .into_iter()
    .collect()
}
fn catalogue() -> AuditResult<BTreeMap<SourceKey, Contract>> {
    let mut out = BTreeMap::new();
    let mut counts = BTreeMap::new();
    for r in records(
        TABLE,
        "file\tsection\tkey\texact_value\tgroup_id\tclassification\tprimary_line\tprimary_sha256\tspecific_reason\tconsumer_bindings\tmodel_guard_id\trequired_evidence_routes",
    )? {
        let k = SourceKey::new(r[0], r[1], r[2]);
        let c = Contract {
            key: k.clone(),
            value: r[3],
            group: r[4],
            reason: r[8],
            bindings: r[9],
            guard: r[10],
            routes: r[11],
        };
        if c.reason.is_empty()
            || c.reason.starts_with("PENDING")
            || out.insert(k, c.clone()).is_some()
        {
            return Err("model162 duplicate/unspecific source contract".into());
        }
        *counts.entry(c.group).or_insert(0usize) += 1;
    }
    if counts != families() || out.len() != 162 {
        return Err("model162 exact independent family census".into());
    }
    Ok(out)
}
fn exact(input: &Inputs, c: &Contract) -> AuditResult<()> {
    if input.reference.rows.get(&c.key).map(String::as_str) != Some(c.value) {
        return Err(format!("model162 changed source literal {:?}", c.key));
    }
    Ok(())
}
fn no_credit(a: &Audit) -> AuditResult<()> {
    let keys = catalogue()?.into_keys().collect::<BTreeSet<_>>();
    if a.required_dependencies.iter().any(|(_,v)|keys.contains(&v.key))||a.witnesses.values().any(|w|matches!(w,Witness::Compared{dependencies,..}if dependencies.iter().any(|v|keys.contains(&v.key)))){return Err("model162 nonread source falsely demanded/Compared".into());}
    Ok(())
}
fn scope(input: &Inputs, group: &str) -> AuditResult<()> {
    let expected = records(MAPS, "group_id\tgame_id\tsection\tkind\tside\tnote")?
        .into_iter()
        .filter(|r| r[0] == group)
        .map(|r| {
            (
                r[1].to_owned(),
                r[2].to_owned(),
                r[3] == "building",
                r[4].to_owned(),
                r[5].to_owned(),
            )
        })
        .collect::<BTreeSet<_>>();
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
                r.section.clone(),
                r.building,
                r.side.clone(),
                r.note.clone(),
            )
        })
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err("model162 actual source alias/mapping scope changed".into());
    }
    Ok(())
}
fn document_span<'a>(text: &'a str, start: &str, end: &str) -> AuditResult<&'a str> {
    let a = text
        .find(start)
        .ok_or("model162 missing policy start anchor")?;
    let z = text[a + start.len()..]
        .find(end)
        .map(|n| a + start.len() + n)
        .ok_or("model162 missing policy end anchor")?;
    Ok(&text[a..z])
}
fn documents(group: &str) -> AuditResult<()> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut count = 0;
    for r in records(DOCS,"group_id\tdocument_path\tsection_start\tsection_end\tspan_authority_id\trequired_case_insensitive_needle")?.into_iter().filter(|r|r[0]==group){let text=std::fs::read_to_string(root.join(r[1])).map_err(|e|e.to_string())?;let span=document_span(&text,r[2],r[3])?;if !span.to_ascii_lowercase().contains(&r[5].to_ascii_lowercase()){return Err("model162 exact documented policy premise changed".into());}count+=1;}
    if count == 0 {
        return Err("model162 missing exact family authority".into());
    }
    Ok(())
}
fn shapes(input: &Inputs) -> AuditResult<()> {
    for r in records(SHAPES, "file\tsection\tkey\texact_components")? {
        let k = SourceKey::new(r[0], r[1], r[2]);
        let text = input
            .reference
            .rows
            .get(&k)
            .ok_or("model162 source tuple absent")?;
        let want = r[3].parse::<usize>().map_err(|e| e.to_string())?;
        let parts = text.split(',').map(str::trim).collect::<Vec<_>>();
        if parts.len() != want || parts.iter().any(|s| s.is_empty()) {
            return Err("model162 nonread source tuple shape changed".into());
        }
    }
    Ok(())
}
fn jumpjets(input: &Inputs) -> AuditResult<()> {
    let mut actual = BTreeSet::new();
    for m in input.mappings.iter().filter(|m| !m.building) {
        let key = SourceKey::new("rules", &m.section, "Locomotor");
        if input
            .reference
            .rows
            .get(&key)
            .is_some_and(|l| l.starts_with("{92612C46"))
        {
            let speed=input.reference.rows.get(&SourceKey::new("rules",&m.section,"JumpjetSpeed")).ok_or("model162 new applicable Jumpjet lacks explicit speed; global fallback is no longer masked")?;
            actual.insert((
                m.game_id.clone(),
                m.section.clone(),
                input.reference.rows.get(&key).ok_or("locomotor")?.clone(),
                speed.clone(),
            ));
        }
    }
    let expected = records(
        JETS,
        "game_id\tsection\texact_locomotor\texplicit_JumpjetSpeed",
    )?
    .into_iter()
    .map(|r| {
        (
            r[0].to_owned(),
            r[1].to_owned(),
            r[2].to_owned(),
            r[3].to_owned(),
        )
    })
    .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err("model162 exact applicable Jumpjet override census changed".into());
    }
    Ok(())
}
pub(super) fn live_group(input: &Inputs, d: &Definitions, g: &str) -> AuditResult<String> {
    scope(input, g)?;
    let mut actual = Vec::new();
    for r in records(
        FACTS,
        "group_id\tgame_id_or_scope\tactual_field\texpected_current_policy_value",
    )?
    .into_iter()
    .filter(|r| r[0] == g)
    {
        let v = loaded::observe(d, r[1], r[2])?;
        if v != r[3] {
            return Err(format!(
                "model162 actual policy premise changed {}.{}: {v} != {}",
                r[1], r[2], r[3]
            ));
        }
        actual.push(format!("{}.{}={v}", r[1], r[2]));
    }
    if actual.is_empty() {
        return Err("model162 family has no real loaded premise".into());
    }
    if g == "jumpjet-defaults" {
        jumpjets(input)?;
    }
    Ok(actual.join(";"))
}
pub(super) fn declare(input: &Inputs, d: &Definitions, a: &Audit) -> AuditResult<Plan> {
    let rows = catalogue()?;
    shapes(input)?;
    no_credit(a)?;
    for c in rows.values() {
        exact(input, c)?;
        documents(c.group)?;
        if live_group(input, d, c.group)? != c.bindings {
            return Err("model162 actual consumer bindings changed".into());
        }
    }
    let mut physical: BTreeMap<String, BTreeSet<WitnessId>> = BTreeMap::new();
    for r in records(PHYSICAL, "group_id\tconsumer\tphysical_field")? {
        if !families().contains_key(r[0]) {
            return Err("model162 foreign physical family".into());
        }
        let w = a.physical(WitnessId::new(r[1], r[2]));
        if !a.required.contains(&w) {
            return Err(format!(
                "model162 genuine physical consumer not independently declared {w:?}"
            ));
        }
        physical.entry(r[0].into()).or_default().insert(w);
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
                .map(|(_, v)| v.clone())
                .collect::<BTreeSet<_>>();
            if &need != dependencies {
                return Err(format!(
                    "model162 demanded/successful native dependency sets differ {w:?}"
                ));
            }
            Ok(())
        }
        _ => Err(format!(
            "model162 real neighboring physical field not Compared {w:?}"
        )),
    }
}
fn accepted_from<'a>(
    text: &'a str,
    key: &SourceKey,
) -> AuditResult<super::policy_admission::SourceAcceptance<'a>> {
    let mut seen = BTreeSet::new();
    let mut out = None;
    for r in records(
        text,
        "file\tsection\tkey\texact_value\tspecific_reason\tconsumer_bindings\tmodel_guard_id\trecorded_decision\tfinal_input_sha256\tnative_receipts",
    )? {
        let k = SourceKey::new(r[0], r[1], r[2]);
        if !seen.insert(k.clone()) {
            return Err("model162 duplicate decision key".into());
        }
        if &k == key {
            out = Some(super::policy_admission::SourceAcceptance {
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
        return Err("model162 missing/foreign decision source atom".into());
    }
    out.ok_or_else(|| "model162 individual source decision absent".into())
}
fn evidence(c: &Contract, text: &str) -> AuditResult<()> {
    let need = c.routes.split(';').collect::<BTreeSet<_>>();
    let mut got = BTreeSet::new();
    for token in text.split(';') {
        let (route, hash) = token
            .split_once('@')
            .ok_or("model162 needs kind/scoped-route@actual-evidence-sha256")?;
        if !matches!(
            route.split('/').next(),
            Some("native" | "static" | "client")
        ) || hash.len() != 64
            || !hash.bytes().all(|b| b.is_ascii_hexdigit())
            || !got.insert(route)
        {
            return Err("model162 malformed/duplicate scoped evidence".into());
        }
    }
    if need != got {
        return Err("model162 missing/foreign exact evidence route".into());
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
        .ok_or("outside exact162 model classifications")?;
    exact(input, c)?;
    documents(c.group)?;
    shapes(input)?;
    no_credit(a)?;
    let live = live_group(input, d, c.group)?;
    if let Some(fields) = plan.physical.get(c.group) {
        for w in fields {
            compared(a, w)?;
        }
    }
    let accepted = accepted_from(acceptance_text, &c.key)?;
    let p = super::policy_admission::SourceProposal {
        key: c.key.clone(),
        exact: c.value,
        reason: c.reason,
        consumer_bindings: c.bindings,
        model_guard_id: c.guard,
    };
    super::policy_admission::source_reason(input, row, &p, Some(&accepted), &live, binding)?;
    evidence(c, accepted.native_receipts)?;
    if !plan.successful.insert(c.key.clone()) {
        return Err("model162 duplicate source admission".into());
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
        return Err("model162 independent source demand != successful classification set".into());
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

// Installed actual prepared negative block: model162-integration-20261019-v1.UNVALIDATED.audit-negatives.rs.txt
// Historical preparation note: UNCOMPILED/UNRUN. Append to model162_admission.rs as child tests after all actual A/S
// integration. Genuine accepted baselines are required for admission-record mutations.
// No permanent assertion that installed final acceptances must stay UNACCEPTED.
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
        let _p = declare(&input, &d, &a)?;
        let errors = super::super::run_adapters(&mut a, &input, &d, &sk);
        if !errors.is_empty() {
            return Err(format!(
                "model162 unchanged native-adapter baseline failed {errors:?}"
            ));
        }
        no_credit(&a)?;
        Ok((input, d, a))
    }
    fn candidate(c: &Contract) -> Unused {
        Unused {
            atom: Atom {
                key: c.key.clone(),
                portion: Portion::Whole,
            },
            exact_value: c.value.into(),
            reason: c.reason.into(),
            code_evidence: "model162 exact code/loaded/native or static-client policy record"
                .into(),
        }
    }
    #[test]
    fn ra9_model162_exact_source_demand_and_all_real_loaded_controls() -> AuditResult<()> {
        let (input, d, a) = baseline()?;
        assert_eq!(catalogue()?.len(), 162);
        for g in families().keys() {
            let _ = live_group(&input, &d, g)?;
        }
        no_credit(&a)
    }
    #[test]
    fn ra9_model162_nonfirst_radar_tuple_value_change_stales_scope_without_native_field()
    -> AuditResult<()> {
        let (mut input, _, a) = baseline()?;
        let key = SourceKey::new("rules", "General", "RadarEventVisibilityDurations");
        let row = input.reference.rows.get_mut(&key).ok_or("actual6 tuple")?;
        let mut parts = row.split(',').map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(parts.len(), 6);
        parts[4] = "201".into();
        *row = parts.join(",");
        shapes(&input)?;
        assert!(
            exact(
                &input,
                catalogue()?.get(&key).ok_or("exact radar contract")?
            )
            .is_err()
        );
        no_credit(&a)
    }
    #[test]
    fn ra9_model162_truncated_nonread_suppression_tuple_fails_exact_shape() -> AuditResult<()> {
        let (mut input, _, _) = baseline()?;
        input.reference.rows.insert(
            SourceKey::new("rules", "General", "RadarEventSuppressionDistances"),
            "8,8,8,8,8".into(),
        );
        assert!(shapes(&input).is_err());
        Ok(())
    }
    #[test]
    fn ra9_model162_nonfirst_shad_override_removal_unmasks_unknown_global_fallback()
    -> AuditResult<()> {
        let (mut input, _, _) = baseline()?;
        jumpjets(&input)?;
        input
            .reference
            .rows
            .remove(&SourceKey::new("rules", "SHAD", "JumpjetSpeed"));
        let e = jumpjets(&input)
            .err()
            .ok_or("new absent unit speed escaped exact masked-fallback guard")?;
        assert!(e.contains("lacks explicit speed"), "{e}");
        Ok(())
    }
    #[test]
    fn ra9_model162_changed_global_jumpjet_speed_does_not_gain_native_speed_credit()
    -> AuditResult<()> {
        let (mut input, d, _) = baseline()?;
        let key = SourceKey::new("rules", "JumpjetControls", "Speed");
        input.reference.rows.insert(key.clone(), "15".into());
        jumpjets(&input)?;
        let mut a = super::super::load_audit(&input, &d)?;
        for section in ["JUMPJET", "SHAD", "ZEP"] {
            let row = input
                .mappings
                .iter()
                .find(|r| r.section == section)
                .ok_or("actual matching locomotor")?;
            super::super::items::unit_terms(&mut a, &input.reference, &d, row)?;
        }
        assert!(exact(&input, catalogue()?.get(&key).ok_or("global14 policy")?).is_err());
        no_credit(&a)
    }
    #[test]
    fn ra9_model162_actual_nonfirst_sub_cloak_flag_reaches_real_source_adapter() -> AuditResult<()>
    {
        let _ = baseline()?;
        let copy = super::super::array_negatives::DataCopy::replace_row(
            "arsenal.ron",
            "(id: \"typhoon\",",
            "cloakable: 1",
            "cloakable: 0",
        )?;
        let (input, d) = copy.loaded()?;
        let mut a = super::super::load_audit(&input, &d)?;
        let row = input
            .mappings
            .iter()
            .find(|r| r.section == "SUB")
            .ok_or("actual SUB")?;
        let e = super::super::items::metadata_flags(&mut a, &input.reference, &d, row)
            .err()
            .ok_or("actual cloak mutation escaped existing source adapter")?;
        assert!(e.contains("cloakable"), "{e}");
        assert!(live_group(&input, &d, "cloak-stages").is_err());
        Ok(())
    }
    #[test]
    fn ra9_model162_nonfirst_loaded_nominal_label_change_is_policy_guard_not_flag_comparison()
    -> AuditResult<()> {
        let _ = baseline()?;
        let copy = super::super::array_negatives::DataCopy::replace_row(
            "arsenal.ron",
            "(id: \"nuclear_silo\",",
            "label: \"Nuclear Missile Silo\"",
            "label: \"Nuclear Missile Silo X\"",
        )?;
        let (input, d) = copy.loaded()?;
        let e = live_group(&input, &d, "names")
            .err()
            .ok_or("actual label mutation escaped generated-name policy")?;
        assert!(e.contains("nuclear_silo.label"), "{e}");
        Ok(())
    }
    #[test]
    fn ra9_model162_actual_gap_radius_change_fails_native_comparison_and_no_boost_policy()
    -> AuditResult<()> {
        let _ = baseline()?;
        let copy = super::super::array_negatives::DataCopy::replace_row(
            "arsenal.ron",
            "(id: \"gap_generator\",",
            "radius: 10",
            "radius: 9",
        )?;
        let (input, d) = copy.loaded()?;
        let sk = sim::load_skirmish(input.data_dir.join("skirmish.ron"), &d)
            .map_err(|e| e.to_string())?;
        let mut a = super::super::load_audit(&input, &d)?;
        let errors = super::super::run_adapters(&mut a, &input, &d, &sk);
        assert_eq!(
            errors,
            vec![r#"specialist native/own field policies: unexplained WitnessId { consumer: "gap_generator@rules:GAGAP", field: "arsenal.ron.entries.radius" }: actual "9", expected "10""#.to_owned()]
        );
        assert!(live_group(&input, &d, "gap-boost").is_err());
        Ok(())
    }
    #[test]
    fn ra9_model162_missing_exact_document_end_fails_instead_of_widening_authority()
    -> AuditResult<()> {
        let _ = baseline()?;
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let r=records(DOCS,"group_id\tdocument_path\tsection_start\tsection_end\tspan_authority_id\trequired_case_insensitive_needle")?.into_iter().find(|r|r[4]=="R48").ok_or("actual R48 scope")?;
        let text = std::fs::read_to_string(root.join(r[1])).map_err(|e| e.to_string())?;
        let _ = document_span(&text, r[2], r[3])?;
        assert_eq!(
            text.matches(r[3]).count(),
            1,
            "unique actual next-section fixture"
        );
        let changed = text.replacen(r[3], "REMOVED EXACT END ANCHOR", 1);
        let e = document_span(&changed, r[2], r[3])
            .err()
            .ok_or("missing end silently widened to file end")?;
        assert!(e.contains("end anchor"), "{e}");
        Ok(())
    }
    #[test]
    fn ra9_model162_nonread_flag_cannot_be_credited_by_actual_neighbor_witness() -> AuditResult<()>
    {
        let (_, _, mut a) = baseline()?;
        let id = a.physical(WitnessId::new("grizzly@rules:MTNK", "combat.items.weapons"));
        assert!(a.required.contains(&id));
        a.require_dependency(
            id,
            Atom {
                key: SourceKey::new("rules", "CombatDamage", "HarvesterImmune"),
                portion: Portion::Whole,
            },
        )?;
        assert!(no_credit(&a).is_err());
        Ok(())
    }
    #[test]
    fn ra9_model162_controlled_unaccepted_source_decision_rejects_after_real_accepted_baseline()
    -> AuditResult<()> {
        let (input, d, a) = baseline()?;
        let key = SourceKey::new("rules", "General", "MaxWaypointPathLength");
        let mut p = declare(&input, &d, &a)?;
        let c = p.rows.get(&key).ok_or("actual source15 policy")?.clone();
        let row = candidate(&c);
        let binding = super::super::policy_admission::input_binding();
        admit(&mut p, &a, &input, &d, &row, binding)?;
        let mut lines = Vec::new();
        for line in ACCEPT.lines() {
            let mut f = line.split('\t').collect::<Vec<_>>();
            if f.len() == 10
                && f[0] == "rules"
                && f[1] == "General"
                && f[2] == "MaxWaypointPathLength"
            {
                f[7] = "UNACCEPTED";
            }
            lines.push(f.join("\t"));
        }
        let fixture = lines.join("\n") + "\n";
        let mut p = declare(&input, &d, &a)?;
        let e = admit_from(&mut p, &a, &input, &d, &row, binding, &fixture)
            .err()
            .ok_or("controlled actual unaccepted source escaped real parser/gate")?;
        assert!(e.contains("decision") || e.contains("receipts"), "{e}");
        Ok(())
    }
    #[test]
    fn ra9_model162_generic_cosmetic_reason_cannot_replace_exact_hazard_boundary() -> AuditResult<()>
    {
        let (input, d, a) = baseline()?;
        let key = SourceKey::new("rules", "CombatDamage", "CollapseChance");
        let mut p = declare(&input, &d, &a)?;
        let c = p.rows.get(&key).ok_or("exact hazard contract")?.clone();
        let row = candidate(&c);
        let binding = super::super::policy_admission::input_binding();
        admit(&mut p, &a, &input, &d, &row, binding)?;
        let mut wrong = row;
        wrong.reason = "All effects are cosmetic".into();
        let mut p = declare(&input, &d, &a)?;
        assert!(admit(&mut p, &a, &input, &d, &wrong, binding).is_err());
        Ok(())
    }
}
