//! UNCOMPILED/UNRUN exact typed disposition routing. No record alone creates coverage.
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Class {
    OriginalEngineUnused,
    CurrentModelNonread,
    IntentionalDifference,
}
impl Class {
    fn parse(s: &str) -> AuditResult<Self> {
        match s {
            "ORIGINAL_ENGINE_UNUSED" => Ok(Self::OriginalEngineUnused),
            "CURRENT_MODEL_NONREAD" => Ok(Self::CurrentModelNonread),
            "INTENTIONAL_DIFFERENCE" => Ok(Self::IntentionalDifference),
            _ => Err("unknown typed source disposition class".into()),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum Route {
    Completion174,
    Static116,
    Metadata147,
    Residual117,
    Consumer62,
    Carry38,
    Model162,
    Guard75,
    Behavior87,
    Masked19,
    Vhp,
    Tree,
    MirageElite,
    Spark,
    LegacyAi361,
    LegacyModel543,
}
impl Route {
    fn parse(s: &str) -> AuditResult<Self> {
        match s {
            "completion174" => Ok(Self::Completion174),
            "static116" => Ok(Self::Static116),
            "metadata147" => Ok(Self::Metadata147),
            "residual117" => Ok(Self::Residual117),
            "consumer62" => Ok(Self::Consumer62),
            "carry38" => Ok(Self::Carry38),
            "model162" => Ok(Self::Model162),
            "guard75" => Ok(Self::Guard75),
            "behavior87" => Ok(Self::Behavior87),
            "masked19" => Ok(Self::Masked19),
            "vhp" => Ok(Self::Vhp),
            "tree" => Ok(Self::Tree),
            "mirage-elite" => Ok(Self::MirageElite),
            "spark" => Ok(Self::Spark),
            "legacy-ai361" => Ok(Self::LegacyAi361),
            "legacy-model543" => Ok(Self::LegacyModel543),
            _ => Err("unknown exact source route".into()),
        }
    }
}
#[derive(Clone)]
pub(super) struct Spec {
    pub(super) route: Route,
    pub(super) class: Class,
    pub(super) exact: String,
    pub(super) reason: String,
}
pub(super) struct Row {
    pub(super) row: Unused,
    pub(super) class: Class,
    route: Route,
}
pub(super) struct Plan {
    wanted: BTreeMap<Atom, Spec>,
    done: BTreeMap<Atom, Class>,
}
fn table(text: &str, names: &[&str]) -> AuditResult<Vec<BTreeMap<String, String>>> {
    let mut lines = text.lines();
    let h = lines
        .next()
        .ok_or("empty routing source catalogue")?
        .split('\t')
        .collect::<Vec<_>>();
    if h.iter().copied().collect::<BTreeSet<_>>().len() != h.len()
        || names.iter().any(|n| !h.contains(n))
    {
        return Err("routing catalogue required/duplicate columns".into());
    }
    lines
        .map(|line| {
            let r = line.split('\t').collect::<Vec<_>>();
            if r.len() != h.len() {
                return Err("routing source catalogue row width".into());
            }
            Ok(h.iter()
                .zip(r)
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect())
        })
        .collect()
}
fn key(r: &BTreeMap<String, String>) -> SourceKey {
    SourceKey::new(&r["file"], &r["section"], &r["key"])
}
fn insert(out: &mut BTreeMap<Atom, Spec>, k: SourceKey, s: Spec) -> AuditResult<()> {
    let atom = Atom {
        key: k,
        portion: Portion::Whole,
    };
    if s.reason.trim().is_empty()
        || s.reason.starts_with("PENDING")
        || out.insert(atom.clone(), s).is_some()
    {
        return Err(format!(
            "ambiguous/unspecific independently owned source {atom:?}"
        ));
    }
    Ok(())
}
fn append_table(
    out: &mut BTreeMap<Atom, Spec>,
    text: &str,
    route: Route,
    class: Class,
    value: &str,
    reason: &str,
    count: usize,
) -> AuditResult<BTreeSet<SourceKey>> {
    let rows = table(text, &["file", "section", "key", value, reason])?;
    if rows.len() != count {
        return Err("exact route source census changed".into());
    }
    let mut keys = BTreeSet::new();
    for r in rows {
        let k = key(&r);
        if !keys.insert(k.clone()) {
            return Err("duplicate route source identity".into());
        }
        insert(
            out,
            k,
            Spec {
                route,
                class,
                exact: r[value].clone(),
                reason: r[reason].clone(),
            },
        )?;
    }
    Ok(keys)
}
fn residual_classes() -> [(&'static str, Class, usize); 11] {
    [
        (
            "SPECIFIC_GAMEPLAY_MODEL_DIFFERENCE",
            Class::IntentionalDifference,
            74,
        ),
        (
            "SPECIFIC_ABSENT_TERRAIN_MODEL_BOUNDARY",
            Class::IntentionalDifference,
            5,
        ),
        (
            "EXACT_DISABLED_EFFECT_COMPATIBILITY_PROPOSAL",
            Class::CurrentModelNonread,
            18,
        ),
        (
            "GUARDED_PRESENTATION_REPLACEMENT_REQUIRES_REFERENCE_CLOSURE",
            Class::CurrentModelNonread,
            9,
        ),
        (
            "CONDITIONAL_INACTIVE_TRAVEL_BRANCH_WITH_SPECIALIST_MODEL",
            Class::CurrentModelNonread,
            4,
        ),
        (
            "PRIMARY_NATIVE_AND_SUPPORT_MODEL_SPLIT",
            Class::CurrentModelNonread,
            2,
        ),
        (
            "EXACT_GENERATED_EFFECT_MODEL_ASSUMPTION",
            Class::CurrentModelNonread,
            1,
        ),
        (
            "GUNNER_MECHANIC_AND_REPLACED_ART_SPLIT",
            Class::CurrentModelNonread,
            1,
        ),
        (
            "SPECIFIC_LIGHT_CADENCE_MODEL_BOUNDARY",
            Class::CurrentModelNonread,
            1,
        ),
        (
            "CONDITIONAL_SELECTOR_REDUNDANCY_PLUS_ANIMATION_DIFFERENCE",
            Class::CurrentModelNonread,
            1,
        ),
        (
            "CHARGE_ANIMATION_REPLACEMENT_WITH_REAL_RELOAD",
            Class::CurrentModelNonread,
            1,
        ),
    ]
}
fn append_residual(out: &mut BTreeMap<Atom, Spec>) -> AuditResult<BTreeSet<SourceKey>> {
    let rows = table(
        include_str!("residual-disposition-20261019-v1.UNVALIDATED.delta.tsv"),
        &[
            "file",
            "section",
            "key",
            "exact_value",
            "proposed_classification",
            "specific_proposed_reason",
        ],
    )?;
    let classes = residual_classes();
    let mut counts = BTreeMap::new();
    let mut keys = BTreeSet::new();
    for r in rows {
        let label = r["proposed_classification"].as_str();
        let (_, class, _) = classes
            .iter()
            .find(|(name, _, _)| *name == label)
            .ok_or("unknown exact residual classification")?;
        *counts.entry(label.to_owned()).or_insert(0usize) += 1;
        let k = key(&r);
        if !keys.insert(k.clone()) {
            return Err("duplicate residual exact source".into());
        }
        insert(
            out,
            k,
            Spec {
                route: Route::Residual117,
                class: *class,
                exact: r["exact_value"].clone(),
                reason: r["specific_proposed_reason"].clone(),
            },
        )?;
    }
    if keys.len() != 117
        || counts
            != classes
                .into_iter()
                .map(|(label, _, count)| (label.to_owned(), count))
                .collect()
    {
        return Err("exact residual class census changed".into());
    }
    Ok(keys)
}
pub(super) fn declare(input: &Inputs, plans: &source_plans::Plans) -> AuditResult<Plan> {
    let mut wanted = BTreeMap::new();
    let mut pending = BTreeSet::new();
    let mut master = BTreeSet::new();
    let mut counts = BTreeMap::new();
    let mut legacy_expected = BTreeMap::new();
    for r in table(
        include_str!("source-candidates.proposed.tsv"),
        &[
            "file",
            "section",
            "key",
            "portion",
            "exact_value",
            "disposition",
            "specific_semantic_reason",
        ],
    )? {
        let k = key(&r);
        if r["portion"] != "whole" || !master.insert(k.clone()) {
            return Err("master candidate source duplicate/shape".into());
        }
        *counts.entry(r["disposition"].clone()).or_insert(0usize) += 1;
        match r["disposition"].as_str() {
            "PENDING_SEMANTICS_OR_NATIVE" => {
                pending.insert(k);
            }
            "PROPOSED_STATIC_UNUSED" => {} // exact static table supplies its current-model reason below
            "PROPOSED_REPLACED_AI_POLICY" => {
                legacy_expected.insert(k, (Route::LegacyAi361, r["exact_value"].clone()));
            }
            "PROPOSED_SPECIFIC_MODEL_BOUNDARY" => {
                legacy_expected.insert(k, (Route::LegacyModel543, r["exact_value"].clone()));
            }
            _ => return Err("unknown original candidate classification".into()),
        }
    }
    if counts
        != [
            ("PENDING_SEMANTICS_OR_NATIVE".into(), 688),
            ("PROPOSED_STATIC_UNUSED".into(), 116),
            ("PROPOSED_REPLACED_AI_POLICY".into(), 361),
            ("PROPOSED_SPECIFIC_MODEL_BOUNDARY".into(), 543),
        ]
        .into_iter()
        .collect()
        || master.len() != 1708
    {
        return Err("independent original1708 family census changed".into());
    }
    append_table(
        &mut wanted,
        include_str!("source-static116.tsv"),
        Route::Static116,
        Class::CurrentModelNonread,
        "exact_value",
        "specific_semantic_reason",
        116,
    )?;
    let mut refinements = append_table(
        &mut wanted,
        include_str!("metadata147-dispositions.tsv"),
        Route::Metadata147,
        Class::CurrentModelNonread,
        "exact_value",
        "specific_semantic_reason",
        147,
    )?;
    refinements.extend(append_residual(&mut wanted)?);
    for (k, route, class, exact, reason) in source_plans::specifications(plans)? {
        if !refinements.insert(k.clone()) {
            return Err("later source package overlaps another exact owner".into());
        }
        insert(
            &mut wanted,
            k,
            Spec {
                route,
                class,
                exact,
                reason,
            },
        )?;
    }
    if refinements != pending {
        return Err("147+117+five S424 packages must refine exactly old688 pending atoms".into());
    }
    let mut legacy_seen = BTreeSet::new();
    for (k, route, class, exact, reason) in source_plans::legacy_specifications(plans) {
        if legacy_expected.get(&k) != Some(&(route, exact.clone()))
            || !legacy_seen.insert(k.clone())
        {
            return Err("legacy904 wrong owner/value/duplicate refinement".into());
        }
        insert(
            &mut wanted,
            k,
            Spec {
                route,
                class,
                exact,
                reason,
            },
        )?;
    }
    if legacy_seen != legacy_expected.into_keys().collect() {
        return Err("legacy904 guard modules must refine exactly old361+543 source keys".into());
    }
    append_table(
        &mut wanted,
        include_str!("masked-speed-atoms.tsv"),
        Route::Masked19,
        Class::CurrentModelNonread,
        "exact_value",
        "reason",
        19,
    )?;
    for (k, route, class, exact, reason) in special_source_admission::specifications()? {
        insert(
            &mut wanted,
            k,
            Spec {
                route,
                class,
                exact,
                reason,
            },
        )?;
    }
    // The actual whole-ledger finish discovered174 additional atoms beyond the prior1737.
    // Tuple components are independent atoms; no component disposition covers Whole/siblings.
    for (atom, class, exact, reason) in source_plans::completion_specifications(plans) {
        if reason.trim().is_empty()
            || reason.starts_with("PENDING")
            || wanted
                .insert(
                    atom.clone(),
                    Spec {
                        route: Route::Completion174,
                        class,
                        exact,
                        reason,
                    },
                )
                .is_some()
        {
            return Err(format!(
                "completion duplicate/unspecific independently owned atom {atom:?}"
            ));
        }
    }
    if wanted.len() != 1911 {
        return Err("exact1911 actual nonread/model atom census changed; reconcile real reads and classifications".into());
    }
    for (atom, s) in &wanted {
        let actual = match atom.portion {
            Portion::Whole => input.reference.expect(atom.key.clone())?.value,
            Portion::Component(index) => input.reference.component(atom.key.clone(), index)?.value,
        };
        if actual != s.exact {
            return Err(format!(
                "source route exact original atom value/shape changed {atom:?}"
            ));
        }
    }
    Ok(Plan {
        wanted,
        done: BTreeMap::new(),
    })
}
pub(super) fn single_table_keys(plan: &Plan) -> AuditResult<BTreeSet<SourceKey>> {
    let keys = plan
        .wanted
        .iter()
        .filter(|(_, spec)| {
            !matches!(
                spec.route,
                Route::Completion174
                    | Route::Consumer62
                    | Route::Carry38
                    | Route::Model162
                    | Route::Guard75
                    | Route::Behavior87
            )
        })
        .map(|(atom, _)| atom.key.clone())
        .collect::<BTreeSet<_>>();
    if keys.len() != 1313 {
        return Err("independent single-table source census changed".into());
    }
    Ok(keys)
}

pub(super) fn load_typed(path: &Path) -> AuditResult<Vec<Row>> {
    let rows = tsv(
        path,
        &[
            "file",
            "section",
            "key",
            "portion",
            "exact_value",
            "classification",
            "route",
            "specific_semantic_reason",
            "code_evidence",
        ],
    )?;
    let mut seen = BTreeSet::new();
    rows.into_iter()
        .map(|r| {
            let portion = if r[3] == "whole" {
                Portion::Whole
            } else {
                Portion::Component(r[3].parse::<usize>().map_err(|e| e.to_string())?)
            };
            let atom = Atom {
                key: SourceKey::new(&r[0], &r[1], &r[2]),
                portion,
            };
            if !seen.insert(atom.clone()) {
                return Err("duplicate typed source disposition row".into());
            }
            Ok(Row {
                row: Unused {
                    atom,
                    exact_value: r[4].clone(),
                    reason: r[7].clone(),
                    code_evidence: r[8].clone(),
                },
                class: Class::parse(&r[5])?,
                route: Route::parse(&r[6])?,
            })
        })
        .collect()
}
pub(super) fn validate_row<'a>(plan: &'a Plan, row: &Row) -> AuditResult<&'a Spec> {
    let s = plan
        .wanted
        .get(&row.row.atom)
        .ok_or("unowned/foreign source disposition")?;
    if plan.done.contains_key(&row.row.atom)
        || row.route != s.route
        || row.class != s.class
        || row.row.exact_value != s.exact
        || row.row.reason != s.reason
        || row.row.code_evidence.trim().is_empty()
    {
        return Err("stale/duplicate/wrong-class/source-route/reason disposition".into());
    }
    Ok(s)
}
pub(super) fn record(plan: &mut Plan, row: &Row) -> AuditResult<()> {
    validate_row(plan, row)?;
    plan.done.insert(row.row.atom.clone(), row.class);
    Ok(())
}
pub(super) fn finish(plan: &Plan, a: &Audit) -> AuditResult<()> {
    let wanted = plan.wanted.keys().cloned().collect::<BTreeSet<_>>();
    let done = plan.done.keys().cloned().collect::<BTreeSet<_>>();
    let stored = a.unused.keys().cloned().collect::<BTreeSet<_>>();
    if wanted != done || done != stored {
        return Err("independent typed source demands != successful guarded classifications != actual stored dispositions".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "compositor-negatives.rs"]
mod tests;
