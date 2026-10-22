//! Historical preparation note: UNCOMPILED/UNRUN compositor only; each original module retains its own exact gates.
use super::*;
use source_routes::{Class, Route};
type Specification = (SourceKey, Route, Class, String, String);
pub(super) struct Plans {
    completion: completion::Plan,
    c62: consumer62_admission::Plan,
    c38: carry38_admission::Plan,
    m162: model162_admission::Plan,
    g75: guard75_admission::Plan,
    b87: behavior87_admission::Plan,
    masked: BTreeSet<SourceKey>,
    masked_done: BTreeSet<SourceKey>,
    special: special_source_admission::Plan,
    legacy_ai: legacy_ai_policy::Plan,
    legacy_model: legacy_model_policy::Plan,
}
pub(super) fn declare(
    i: &Inputs,
    d: &Definitions,
    a: &Audit,
    s: &scope_inputs::ObservedScope,
) -> AuditResult<Plans> {
    Ok(Plans {
        completion: completion::declare(i, d, a)?,
        c62: consumer62_admission::declare(i, d, a)?,
        c38: carry38_admission::declare(i, d, a)?,
        m162: model162_admission::declare(i, d, a)?,
        g75: guard75_admission::declare(i, d, a, &s.guard75())?,
        b87: behavior87_admission::declare(i, d, a)?,
        masked: masked_speed_admission::demand(i)?,
        masked_done: BTreeSet::new(),
        special: special_source_admission::declare(i, d, a, s)?,
        legacy_ai: legacy_ai_policy::declare(i, d, a, s)?,
        legacy_model: legacy_model_policy::declare(i, d, a, s)?,
    })
}
fn exact_classes(
    module: &str,
    text: &str,
    rows: BTreeMap<SourceKey, (String, String)>,
) -> AuditResult<Vec<(SourceKey, Class, String, String)>> {
    let mut records = text.lines();
    let header = records
        .next()
        .ok_or("empty per-contract classification table")?
        .split('\t')
        .collect::<Vec<_>>();
    let at = |name| {
        header
            .iter()
            .position(|v| *v == name)
            .ok_or("missing per-contract classification column")
    };
    let fi = at("file")?;
    let si = at("section")?;
    let ki = at("key")?;
    let ci = at("classification")?;
    let vi = at("exact_value")?;
    let ri = at("specific_reason")?;
    let mut counts = BTreeMap::new();
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for line in records {
        let r = line.split('\t').collect::<Vec<_>>();
        if r.len() != header.len() {
            return Err("per-contract class row width".into());
        }
        let key = SourceKey::new(r[fi], r[si], r[ki]);
        let (exact, reason) = rows
            .get(&key)
            .ok_or("class row outside independently demanded source plan")?;
        if exact != r[vi] || reason != r[ri] || !seen.insert(key.clone()) {
            return Err("exact class/value/reason/duplicate mismatch".into());
        }
        let mappings = legacy904_support::records(
            include_str!("legacy904-class-contracts.tsv"),
            "module\tactual_contract_classification\texact_atom_count\ttyped_class\tpolicy",
        )?;
        let matches = mappings
            .iter()
            .filter(|m| m[0] == module && m[1] == r[ci])
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err("uncontracted/ambiguous exact classification".into());
        }
        let class = match matches[0][3] {
            "CURRENT_MODEL_NONREAD" => Class::CurrentModelNonread,
            "INTENTIONAL_DIFFERENCE" => Class::IntentionalDifference,
            _ => return Err("foreign typed class".into()),
        };
        *counts.entry(r[ci].to_owned()).or_insert(0usize) += 1;
        out.push((key, class, exact.clone(), reason.clone()));
    }
    let expected = legacy904_support::records(
        include_str!("legacy904-class-contracts.tsv"),
        "module\tactual_contract_classification\texact_atom_count\ttyped_class\tpolicy",
    )?
    .into_iter()
    .filter(|m| m[0] == module)
    .map(|m| {
        m[2].parse::<usize>()
            .map(|n| (m[1].to_owned(), n))
            .map_err(|e| e.to_string())
    })
    .collect::<AuditResult<BTreeMap<_, _>>>()?;
    if counts != expected || seen != rows.into_keys().collect() {
        return Err("exact module classification census/demand differs".into());
    }
    Ok(out)
}
pub(super) fn specifications(p: &Plans) -> AuditResult<Vec<Specification>> {
    let mut out = Vec::new();
    for (route, module, table, rows, count) in [
        (
            Route::Consumer62,
            "consumer62",
            include_str!("consumer62-contracts.tsv"),
            consumer62_admission::demanded_sources(&p.c62),
            62,
        ),
        (
            Route::Model162,
            "model162",
            include_str!("model162-contracts.tsv"),
            model162_admission::demanded_sources(&p.m162),
            162,
        ),
        (
            Route::Guard75,
            "guard75",
            include_str!("guard75-contracts.tsv"),
            guard75_admission::demanded_sources(&p.g75),
            75,
        ),
        (
            Route::Behavior87,
            "behavior87",
            include_str!("behavior87-contracts.tsv"),
            behavior87_admission::demanded_sources(&p.b87),
            87,
        ),
    ] {
        if rows.len() != count {
            return Err("composed source demand census".into());
        }
        for (k, class, exact, reason) in exact_classes(module, table, rows)? {
            out.push((k, route, class, exact, reason));
        }
    }
    let rows = carry38_admission::demanded_sources(&p.c38);
    if rows.len() != 38 {
        return Err("carry38 demand count".into());
    }
    for (k, (exact, reason)) in rows {
        out.push((
            k,
            Route::Carry38,
            Class::IntentionalDifference,
            exact,
            reason,
        ));
    }
    Ok(out)
}
pub(super) fn legacy_specifications(p: &Plans) -> Vec<(SourceKey, Route, Class, String, String)> {
    let mut out = legacy_ai_policy::specifications(&p.legacy_ai);
    out.extend(legacy_model_policy::specifications(&p.legacy_model));
    out
}
pub(super) fn completion_specifications(p: &Plans) -> Vec<(Atom, Class, String, String)> {
    completion::specifications(&p.completion)
}
// Bind proposals to the real declaration, never a copied acceptance or expected table.
pub(super) fn single_proposal_binding(
    p: &Plans,
    row: &Unused,
    route: Route,
    scope: &scope_inputs::ObservedScope,
) -> AuditResult<(String, String)> {
    match route {
        Route::LegacyAi361 => legacy_ai_policy::proposal_binding(&p.legacy_ai, &row.atom.key),
        Route::LegacyModel543 => {
            legacy_model_policy::proposal_binding(&p.legacy_model, &row.atom.key)
        }
        Route::Masked19 => {
            if !p.masked.contains(&row.atom.key) {
                return Err("undeclared masked proposal".into());
            }
            let rows = legacy904_support::records(
                include_str!("masked-speed-atoms.tsv"),
                "file\tsection\tkey\texact_value\tprimary_line\tgame_id\tprojectile\tpolicy\treason",
            )?;
            let r = rows
                .iter()
                .find(|r| SourceKey::new(r[0], r[1], r[2]) == row.atom.key)
                .ok_or("masked proposal has no exact selector row")?;
            if row.exact_value != r[3] || row.reason != r[8] {
                return Err("masked proposal changed".into());
            }
            Ok((
                format!("weapon.{}@rules:{}", r[5], r[1]),
                format!("ra9.{}.{}.v1", r[7], r[1]),
            ))
        }
        Route::Vhp | Route::Tree | Route::MirageElite | Route::Spark => Ok((
            format!(
                "source={}:{}:{};model-census={};original-closure={}",
                row.atom.key.file,
                row.atom.key.section,
                row.atom.key.key,
                scope.model_sha256(),
                scope.original_closure_sha256()
            ),
            format!(
                "ra9.compositor.special.{route:?}.{}.{}",
                row.atom.key.section, row.atom.key.key
            ),
        )),
        Route::Static116 | Route::Metadata147 | Route::Residual117 => Ok((
            format!(
                "current-model-policy={route:?};source={}:{}:{};model-census={};original-closure={}",
                row.atom.key.file,
                row.atom.key.section,
                row.atom.key.key,
                scope.model_sha256(),
                scope.original_closure_sha256()
            ),
            format!(
                "ra9.source-policy.{route:?}.{}.{}",
                row.atom.key.section, row.atom.key.key
            ),
        )),
        _ => Err("paired acceptance owner has no single-table proposal".into()),
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Independent actual, expected, difference and evidence inputs remain explicit at the audit boundary"
)]
pub(super) fn admit(
    p: &mut Plans,
    a: &Audit,
    i: &Inputs,
    d: &Definitions,
    row: &Unused,
    route: Route,
    s: &scope_inputs::ObservedScope,
    fire: &fire_keys::Evidence,
    replacement: Option<&policy_admission::SourceAcceptance<'_>>,
) -> AuditResult<()> {
    let binding = policy_admission::input_binding();
    match route {
        Route::Completion174 => completion::admit(&mut p.completion, a, i, d, row, s),
        Route::Consumer62 => consumer62_admission::admit(&mut p.c62, a, i, d, row, binding),
        Route::Carry38 => carry38_admission::admit(&mut p.c38, a, i, d, row, binding),
        Route::Model162 => model162_admission::admit(&mut p.m162, a, i, d, row, binding),
        Route::Guard75 => guard75_admission::admit(&mut p.g75, a, i, d, row, &s.guard75()),
        Route::Behavior87 => behavior87_admission::admit(&mut p.b87, a, i, d, row, binding),
        Route::Masked19 => {
            if !p.masked.contains(&row.atom.key) {
                return Err("masked source not independently demanded".into());
            }
            let approval = selected_source_acceptance(&row.atom.key, replacement)?;
            masked_speed_admission::require(a, i, d, row, approval.as_ref(), binding)?;
            let accepted = approval
                .as_ref()
                .ok_or("missing individual masked source acceptance")?;
            s.require_routes(
                accepted.native_receipts,
                &BTreeSet::from(["static/exact-source-current-model/masked19"]),
            )?;
            if !p.masked_done.insert(row.atom.key.clone()) {
                return Err("duplicate masked source outcome".into());
            }
            Ok(())
        }
        Route::Vhp | Route::Tree | Route::MirageElite | Route::Spark => {
            special_source_admission::admit(&mut p.special, a, i, d, row, route, s, fire)
        }
        Route::LegacyAi361 => legacy_ai_policy::admit(&mut p.legacy_ai, a, i, d, row, s),
        Route::LegacyModel543 => legacy_model_policy::admit(&mut p.legacy_model, a, i, d, row, s),
        Route::Static116 | Route::Metadata147 | Route::Residual117 => {
            let approval = selected_source_acceptance(&row.atom.key, replacement)?;
            legacy_source_admission::require(a, i, d, row, route, s, approval.as_ref())?;
            let accepted = approval
                .as_ref()
                .ok_or("missing individual exact source acceptance")?;
            // Declare exact live identity from the route, source key and observed scope.
            // The accepted identity and receipt text never supply their own expectations.
            let (consumer, guard) = single_proposal_binding(p, row, route, s)?;
            let proposal = policy_admission::SourceProposal {
                key: row.atom.key.clone(),
                exact: &row.exact_value,
                reason: &row.reason,
                consumer_bindings: &consumer,
                model_guard_id: &guard,
            };
            policy_admission::source_reason(i, row, &proposal, Some(accepted), &consumer, binding)?;
            let receipt_route = match route {
                Route::Static116 => "static/exact-source-current-model/static116",
                Route::Metadata147 => "static/exact-source-current-model/metadata147",
                Route::Residual117 => "static/exact-source-current-model/residual117",
                _ => return Err("foreign legacy source evidence route".into()),
            };
            s.require_routes(accepted.native_receipts, &BTreeSet::from([receipt_route]))
        }
    }
}
fn selected_source_acceptance<'a>(
    key: &SourceKey,
    replacement: Option<&policy_admission::SourceAcceptance<'a>>,
) -> AuditResult<Option<policy_admission::SourceAcceptance<'a>>> {
    // The immutable replacement belongs only to this audit call and this exact source key.
    // All other records use the installed index, including during parallel test execution.
    match replacement.filter(|a| &a.key == key) {
        Some(a) => Ok(Some(a.clone())),
        None => policy_admission::source_acceptance(key),
    }
}
pub(super) fn finish(p: &Plans, a: &Audit) -> AuditResult<()> {
    completion::finish(&p.completion, a)?;
    consumer62_admission::finish(&p.c62, a)?;
    carry38_admission::finish(&p.c38, a)?;
    model162_admission::finish(&p.m162, a)?;
    guard75_admission::finish(&p.g75, a)?;
    behavior87_admission::finish(&p.b87, a)?;
    if p.masked != p.masked_done {
        return Err("independent masked19 demands != guarded successes".into());
    }
    special_source_admission::finish(&p.special, a)?;
    legacy_ai_policy::finish(&p.legacy_ai, a)?;
    legacy_model_policy::finish(&p.legacy_model, a)
}
