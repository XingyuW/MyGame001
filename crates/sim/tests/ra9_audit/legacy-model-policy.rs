//! UNCOMPILED/UNRUN 543 exact old source classifications in 62 families.
//! No original-engine unused claim, source literal getter, alias/Own/native read credit.
use super::*;
use legacy904_support as s;
const ROUTE: &str = "legacy-model543";
const AI: bool = false;
#[derive(Clone)]
struct Family {
    id: String,
    count: usize,
    profile: String,
    class: source_routes::Class,
    reason: String,
    paths: String,
    routes: String,
    docs: String,
    obligation: String,
    atom_fp: String,
}
#[derive(Clone)]
struct Contract {
    key: SourceKey,
    value: String,
    family: Family,
    reason: String,
}
#[derive(Clone)]
pub(super) struct Plan {
    rows: BTreeMap<SourceKey, Contract>,
    physical: BTreeMap<String, BTreeMap<WitnessId, BTreeSet<Atom>>>,
    bindings: BTreeMap<String, String>,
    successful: BTreeSet<SourceKey>,
}
fn families() -> AuditResult<BTreeMap<String, Family>> {
    let mut out = BTreeMap::new();
    for r in s::records(include_str!("legacy904-family-contracts.tsv"),"route\tfamily_id\tatom_count\told_identity_value_sha256\tprofile\ttyped_class\tspecific_current_model_reason\tactual_paths\trequired_routes\tdocument_authorities\tadmission_obligation\tsource_sections\tsource_keys\tindependent_atom_fnv1a64")?.into_iter().filter(|r|r[0]==ROUTE){let class=match r[5]{"CURRENT_MODEL_NONREAD"=>source_routes::Class::CurrentModelNonread,"INTENTIONAL_DIFFERENCE"=>source_routes::Class::IntentionalDifference,_=>return Err("legacy no original unused/unknown type".into())};let f=Family{id:r[1].into(),count:r[2].parse::<usize>().map_err(|e|e.to_string())?,profile:r[4].into(),class,reason:r[6].into(),paths:r[7].into(),routes:r[8].into(),docs:r[9].into(),obligation:r[10].into(),atom_fp:r[13].into()};if f.reason.starts_with("PENDING")||f.reason.is_empty()||out.insert(f.id.clone(),f).is_some(){return Err("legacy ambiguous family contract".into());}}
    if out.len() != 62 || out.values().map(|f| f.count).sum::<usize>() != 543 {
        return Err("legacy independent exact family census changed".into());
    }
    Ok(out)
}
fn mode(c: &Contract) -> &'static str {
    if matches!(
        c.value.to_ascii_lowercase().as_str(),
        "no" | "false" | "0" | "0.0"
    ) {
        "EXACT_SOURCE_FALSE_OR_ZERO_UNPARSED_NO_TRUE_SEMANTICS_INFERRED"
    } else if c.family.profile == "default-death" {
        "CURRENT_NO_DEFAULT_RUNTIME_SELECTOR_SOURCE_DAMAGE_ABSENT"
    } else if matches!(
        c.family.profile.as_str(),
        "carrier-collision" | "virtual-scanner" | "nuke-up-carrier" | "fire-helpers" | "shrapnel"
    ) {
        "ORIGINAL_HELPER_GRAPH_HAS_NO_CURRENT_LOADED_NATIVE_SELECTOR"
    } else if c.family.profile == "campaign-economy" {
        "CURRENT_SKIRMISH_BRANCH_NO_CAMPAIGN_PATH"
    } else {
        "EXACT_CURRENT_MODEL_BOUNDARY_NOT_ORIGINAL_NONUSE"
    }
}
fn catalogue_from(master: &str) -> AuditResult<BTreeMap<SourceKey, Contract>> {
    let families = families()?;
    let mut rows = BTreeMap::new();
    let mut counts = BTreeMap::new();
    let expected_disposition = if AI {
        "PROPOSED_REPLACED_AI_POLICY"
    } else {
        "PROPOSED_SPECIFIC_MODEL_BOUNDARY"
    };
    for r in s::records(master,"file\tsection\tkey\tportion\texact_value\tdisposition\tfamily_id\tspecific_semantic_reason\tactual_code_evidence\tactive_premise\tconsumer_binding\tprimary_line\tprimary_written_section\tprimary_written_key\tprimary_sha256\tacceptance\tactual_implementation_policy\tdifference_scope")?.into_iter().filter(|r|r[5]==expected_disposition){let family=families.get(r[6]).ok_or("legacy unknown exact source family")?.clone();if r[0]!="rules"||r[3]!="whole"||r[14]!="adc869ca037db5141f562f18092d022045e194de0d1204a46b9ec8372b2b826f"{return Err("legacy non-whole/foreign original source".into());}let key=SourceKey::new(r[0],r[1],r[2]);let mut c=Contract{key:key.clone(),value:r[4].into(),family,reason:String::new()};c.reason=format!("[{}] {}={}; {}; {}",key.section,key.key,c.value,c.family.reason,mode(&c));*counts.entry(c.family.id.clone()).or_insert(0usize)+=1;if rows.insert(key,c).is_some(){return Err("legacy duplicate original source atom".into());}}
    for f in families.values() {
        let text = rows
            .values()
            .filter(|c| c.family.id == f.id)
            .map(|c| {
                format!(
                    "{}\t{}\t{}\twhole\t{}\n",
                    c.key.file, c.key.section, c.key.key, c.value
                )
            })
            .collect::<String>();
        if s::hash(text.as_bytes()) != f.atom_fp {
            return Err("legacy independent exact family atom/value membership changed".into());
        }
    }
    let want = families
        .into_iter()
        .map(|(k, f)| (k, f.count))
        .collect::<BTreeMap<_, _>>();
    if counts != want || rows.len() != 543 {
        return Err("legacy exact per-family original master identity/count mismatch".into());
    }
    Ok(rows)
}
fn catalogue() -> AuditResult<BTreeMap<SourceKey, Contract>> {
    catalogue_from(include_str!("source-candidates.proposed.tsv"))
}
fn no_credit(a: &Audit, rows: &BTreeMap<SourceKey, Contract>) -> AuditResult<()> {
    if a.required_dependencies.iter().any(|(_,v)|rows.contains_key(&v.key))||a.witnesses.values().any(|w|matches!(w,Witness::Compared{dependencies,..}if dependencies.iter().any(|v|rows.contains_key(&v.key)))){return Err("legacy nonread atom is now a REAL demanded/Compared input: migrate exact atom to Compared, preserve comparison and reconcile1737, never suppress it to keep old count".into());}
    Ok(())
}
fn source(i: &Inputs, c: &Contract) -> AuditResult<()> {
    if i.reference.shapes.contains_key(&c.key) || i.reference.rows.get(&c.key) != Some(&c.value) {
        return Err(format!("legacy exact source/shape changed {:?}", c.key));
    }
    Ok(())
}
fn live(
    i: &Inputs,
    d: &Definitions,
    f: &Family,
    scope: &scope_inputs::ObservedScope,
) -> AuditResult<String> {
    s::documents(&f.docs)?;
    let code = s::current_code(&f.paths, &f.profile)?;
    let loaded = s::loaded(i, d, &f.profile, AI || f.profile.starts_with("difficulty-"))?;
    let original = s::original(i, &f.id)?;
    let last_five = s::last_five_model_guard(i, d, &f.id)?;
    // Mapping expected sets and typed policy snapshots are local guards, not proof of original semantics.
    // The root rehashes full actual inputs plus original producer and each linked scoped route.
    Ok(format!(
        "family={};profile={};loaded-fnv={loaded};branch-fnv={};original={original};model-census={};last-five={last_five}",
        f.id,
        f.profile,
        s::hash(code.as_bytes()),
        scope.model_sha256()
    ))
}
pub(super) fn declare(
    i: &Inputs,
    d: &Definitions,
    a: &Audit,
    scope: &scope_inputs::ObservedScope,
) -> AuditResult<Plan> {
    let rows = catalogue()?;
    no_credit(a, &rows)?;
    let mut physical = BTreeMap::new();
    let mut bindings = BTreeMap::new();
    for c in rows.values() {
        source(i, c)?;
        if !bindings.contains_key(&c.family.id) {
            bindings.insert(c.family.id.clone(), live(i, d, &c.family, scope)?);
        }
        if !physical.contains_key(&c.family.profile) {
            physical.insert(
                c.family.profile.clone(),
                s::physical(
                    i,
                    a,
                    &c.family.profile,
                    AI || c.family.profile.starts_with("difficulty-"),
                )?,
            );
        }
    }
    Ok(Plan {
        rows,
        physical,
        bindings,
        successful: BTreeSet::new(),
    })
}
pub(super) fn specifications(
    p: &Plan,
) -> Vec<(
    SourceKey,
    source_routes::Route,
    source_routes::Class,
    String,
    String,
)> {
    p.rows
        .values()
        .map(|c| {
            (
                c.key.clone(),
                source_routes::Route::LegacyModel543,
                c.family.class,
                c.value.clone(),
                c.reason.clone(),
            )
        })
        .collect()
}
// Export only a binding already established by declare's real source/loaded/model checks.
// This does not install an acceptance, add a disposition, or grant source/native credit.
pub(super) fn proposal_binding(p: &Plan, key: &SourceKey) -> AuditResult<(String, String)> {
    let c = p
        .rows
        .get(key)
        .ok_or("proposal source belongs to another legacy owner")?;
    let binding = p
        .bindings
        .get(&c.family.id)
        .ok_or("real declared legacy binding missing")?;
    Ok((
        binding.clone(),
        format!("legacy904/{}:{}", ROUTE, c.family.id),
    ))
}

fn resolution(
    i: &Inputs,
    c: &Contract,
    a: &policy_admission::SourceAcceptance<'_>,
) -> AuditResult<()> {
    if c.family.obligation != "HARD_BLOCK_RANGE_AUTHORITY_CONFLICT" {
        return Ok(());
    }
    let text = std::fs::read_to_string(
        s::root().join("crates/sim/tests/reference/ra9_legacy_range_resolution.tsv"),
    )
    .map_err(|e| format!("explicit appendixD4/current R34 conflict resolution missing: {e}"))?;
    let rs = s::records(
        &text,
        "family_id\tchosen_current_model\trecorded_decision\tfinal_input_sha256",
    )?;
    if rs.len() != 1
        || rs[0][0] != "F083"
        || rs[0][1] != "nearest-subcell-current-model-with-explicit-appendix-difference"
        || rs[0][2] != a.recorded_decision
        || rs[0][2] == "UNACCEPTED"
        || rs[0][3] != policy_admission::input_binding()
    {
        return Err("range conflict unresolved; if original Chebyshev is implemented instead migrate relevant keys/fields/native obligations, do not waive a completed promise".into());
    }
    let _ = i;
    Ok(())
}
pub(super) fn admit_from(
    p: &mut Plan,
    a: &Audit,
    i: &Inputs,
    d: &Definitions,
    row: &Unused,
    scope: &scope_inputs::ObservedScope,
    accepted: Option<&policy_admission::SourceAcceptance<'_>>,
) -> AuditResult<()> {
    let c = p
        .rows
        .get(&row.atom.key)
        .ok_or("source belongs to a different exact legacy owner")?;
    source(i, c)?;
    no_credit(a, &p.rows)?;
    let actual = live(i, d, &c.family, scope)?;
    if p.bindings.get(&c.family.id) != Some(&actual) {
        return Err("legacy declared/current branch changed during adapters".into());
    }
    s::compared(
        a,
        p.physical
            .get(&c.family.profile)
            .ok_or("legacy no independent physical demand plan")?,
    )?;
    let guard = format!("legacy904/{}:{}", ROUTE, c.family.id);
    let proposal = policy_admission::SourceProposal {
        key: c.key.clone(),
        exact: &c.value,
        reason: &c.reason,
        consumer_bindings: &actual,
        model_guard_id: &guard,
    };
    policy_admission::source_reason(
        i,
        row,
        &proposal,
        accepted,
        &actual,
        policy_admission::input_binding(),
    )?;
    let accepted = accepted.ok_or("individual legacy source decision missing")?;
    resolution(i, c, accepted)?;
    let mut routes = c.family.routes.split(';').collect::<BTreeSet<_>>();
    let static_route = format!("static/legacy904-{}-current-branch", c.family.id);
    let loaded_route = format!("loaded/legacy904-{}-current-policy", c.family.id);
    routes.insert(&static_route);
    routes.insert(&loaded_route);
    routes.insert("static/legacy904-original-source-closure");
    scope.require_routes(accepted.native_receipts, &routes)?;
    if !p.successful.insert(c.key.clone()) {
        return Err("duplicate legacy source success".into());
    }
    Ok(())
}
pub(super) fn admit(
    p: &mut Plan,
    a: &Audit,
    i: &Inputs,
    d: &Definitions,
    row: &Unused,
    scope: &scope_inputs::ObservedScope,
) -> AuditResult<()> {
    let accepted = policy_admission::source_acceptance(&row.atom.key)?;
    admit_from(p, a, i, d, row, scope, accepted.as_ref())
}
pub(super) fn finish(p: &Plan, a: &Audit) -> AuditResult<()> {
    if p.successful != p.rows.keys().cloned().collect() {
        return Err("legacy independent exact source classification demands != successful guarded admissions".into());
    }
    for fields in p.physical.values() {
        s::compared(a, fields)?;
    }
    no_credit(a, &p.rows)
}

#[cfg(test)]
#[path = "legacy-model-negatives.rs"]
mod tests;
