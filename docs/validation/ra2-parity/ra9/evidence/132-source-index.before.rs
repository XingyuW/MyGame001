//! Exact80 current game-array contracts, UNCOMPILED/UNRUN. No native/source coverage.
use super::*;
const CONTRACTS: &str = include_str!("own-policy-contracts.tsv");
const ACCEPTANCES: &str = include_str!("own-policy-acceptance.tsv");
fn records<'a>(text: &'a str, header: &str) -> AuditResult<Vec<Vec<&'a str>>> {
    let mut lines = text.lines();
    if lines.next() != Some(header) {
        return Err("specific policy table header".into());
    }
    lines.map(|line| Ok(line.split('\t').collect())).collect()
}
fn policy_values(value: &str) -> Vec<String> {
    if value.is_empty() {
        Vec::new()
    } else {
        value.split(',').map(str::to_owned).collect()
    }
}
fn accepted_arrays(
    contracts: &str,
    acceptances: &str,
    current: &str,
    live_input_binding: &str,
) -> AuditResult<BTreeMap<String, Vec<String>>> {
    if live_input_binding.len() != 64 || !live_input_binding.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("root has not bound the actual final source/input/native receipt scope".into());
    }
    let source = records(
        contracts,
        "array_id\tvalues_comma_separated\tspecific_game_policy\tcontract_sha256",
    )?;
    let approved = records(
        acceptances,
        "array_id\tcontract_sha256\trecorded_decision\tfinal_input_sha256\tnative_receipts\tvalues_comma_separated\tspecific_game_policy",
    )?;
    let present = records(
        current,
        "array_id\tvalues_comma_separated\tspecific_game_policy",
    )?;
    let mut catalogue = BTreeMap::new();
    for c in source {
        if c.len() != 4
            || c[2].starts_with("PENDING")
            || c[2].trim().is_empty()
            || catalogue.insert(c[0], c).is_some()
        {
            return Err("duplicate/unspecific exact game policy contract".into());
        }
    }
    let mut receipts = BTreeMap::new();
    for c in approved {
        if c.len() != 7 || receipts.insert(c[0], c).is_some() {
            return Err("duplicate/malformed exact policy acceptance".into());
        }
    }
    let mut values = BTreeMap::new();
    for row in present {
        if row.len() != 3 {
            return Err("actual game-array policy row width".into());
        }
        let contract = catalogue
            .get(row[0])
            .ok_or("uncontracted game-array policy")?;
        let receipt = receipts
            .get(row[0])
            .ok_or("missing individual model decision")?;
        if row[1] != contract[1]
            || row[2] != contract[2]
            || receipt[1] != contract[3]
            || receipt[5] != row[1]
            || receipt[6] != row[2]
        {
            return Err(format!(
                "specific policy reason/value/consumer contract changed {}",
                row[0]
            ));
        }
        if receipt[2].is_empty()
            || receipt[2] == "UNACCEPTED"
            || receipt[3] != live_input_binding
            || receipt[4].is_empty()
            || receipt[4] == "UNRUN"
        {
            return Err(format!(
                "individual policy has no recorded final decision/native receipts {}",
                row[0]
            ));
        }
        if values
            .insert(row[0].into(), policy_values(row[1]))
            .is_some()
        {
            return Err("duplicate actual game-array policy".into());
        }
    }
    if values.keys().map(String::as_str).collect::<BTreeSet<_>>()
        != catalogue.keys().copied().collect()
        || values.keys().map(String::as_str).collect::<BTreeSet<_>>()
            != receipts.keys().copied().collect()
    {
        return Err("extra/missing exact model contract or acceptance record".into());
    }
    Ok(values)
}
pub(super) fn arrays() -> AuditResult<BTreeMap<String, Vec<String>>> {
    // Root must independently rehash actual live inputs and validate linked logs first. This
    // persisted exact binding is an auditable record, not a substitute for that external check.
    accepted_arrays(
        CONTRACTS,
        ACCEPTANCES,
        include_str!("game-arrays.tsv"),
        include_str!("root-policy-input-binding.txt").trim(),
    )
}
// Mandatory common source-policy guard for prior exact disposition packets (including S424
// and the separate VHP record). It grants NO source or physical coverage and must precede
// add_unused. Caller builds live_consumers with the real policy-specific adapter/selector.
pub(super) struct SourceProposal<'a> {
    pub(super) key: SourceKey,
    pub(super) exact: &'a str,
    pub(super) reason: &'a str,
    pub(super) consumer_bindings: &'a str,
    pub(super) model_guard_id: &'a str,
}
pub(super) struct SourceAcceptance<'a> {
    pub(super) key: SourceKey,
    pub(super) exact: &'a str,
    pub(super) reason: &'a str,
    pub(super) consumer_bindings: &'a str,
    pub(super) model_guard_id: &'a str,
    pub(super) recorded_decision: &'a str,
    pub(super) final_input_sha256: &'a str,
    pub(super) native_receipts: &'a str,
}
pub(super) fn source_reason(
    input: &Inputs,
    row: &Unused,
    p: &SourceProposal<'_>,
    accepted: Option<&SourceAcceptance<'_>>,
    live_consumers: &str,
    live_input_sha256: &str,
) -> AuditResult<()> {
    let a = accepted.ok_or("no individual source/model policy acceptance")?;
    if row.atom.key != p.key
        || row.atom.portion != Portion::Whole
        || row.exact_value != p.exact
        || row.reason != p.reason
        || input.reference.rows.get(&p.key).map(String::as_str) != Some(p.exact)
    {
        return Err("source proposal exact key/value/reason changed".into());
    }
    if a.key != p.key
        || a.exact != p.exact
        || a.reason != p.reason
        || a.consumer_bindings != p.consumer_bindings
        || a.model_guard_id != p.model_guard_id
        || live_consumers != p.consumer_bindings
    {
        return Err("source acceptance does not bind exact current consumer/model/reason".into());
    }
    if a.recorded_decision.is_empty()
        || a.recorded_decision == "UNACCEPTED"
        || a.native_receipts.is_empty()
        || a.native_receipts == "UNRUN"
        || live_input_sha256.len() != 64
        || !live_input_sha256.bytes().all(|b| b.is_ascii_hexdigit())
        || a.final_input_sha256 != live_input_sha256
    {
        return Err("individual source model decision/final scope/native receipts missing".into());
    }
    Ok(())
}

pub(super) fn input_binding() -> &'static str {
    include_str!("root-policy-input-binding.txt").trim()
}
const SOURCE_ACCEPTANCE_HEADER: &str = "file\tsection\tkey\texact_value\tspecific_reason\tconsumer_bindings\tmodel_guard_id\trecorded_decision\tfinal_input_sha256\tnative_receipts";
fn acceptance_census(text: &str, wanted: &BTreeSet<SourceKey>) -> AuditResult<()> {
    let mut actual = BTreeSet::new();
    for r in records(text, SOURCE_ACCEPTANCE_HEADER)? {
        if r.len() != 10 || !actual.insert(SourceKey::new(r[0], r[1], r[2])) {
            return Err("single-table source acceptance width/duplicate".into());
        }
    }
    if &actual != wanted {
        return Err("single-table acceptances differ from independent exact source owners".into());
    }
    Ok(())
}
pub(super) fn require_source_acceptance_census(wanted: &BTreeSet<SourceKey>) -> AuditResult<()> {
    acceptance_census(include_str!("source-policy-acceptance.tsv"), wanted)
}

pub(super) fn source_acceptance(key: &SourceKey) -> AuditResult<Option<SourceAcceptance<'static>>> {
    let rows = records(
        include_str!("source-policy-acceptance.tsv"),
        SOURCE_ACCEPTANCE_HEADER,
    )?;
    let mut seen = BTreeSet::new();
    let mut result = None;
    for r in rows {
        if r.len() != 10 {
            return Err("source acceptance width".into());
        }
        let item = SourceKey::new(r[0], r[1], r[2]);
        if !seen.insert(item.clone()) {
            return Err("duplicate exact source acceptance".into());
        }
        if &item == key {
            result = Some(SourceAcceptance {
                key: item,
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
    Ok(result)
}

// Installed actual prepared negative block: integration-blockers-20261019-v1.UNVALIDATED.policy-negatives.rs.txt
// UNCOMPILED/UNRUN; append to policy-admission.rs. No fake native witness or acceptance.
#[cfg(test)]
mod tests {
    use super::*;
    fn baseline() -> AuditResult<(String, String, String)> {
        super::super::audit_at(
            &super::super::super::data_dir(),
            &super::super::super::reference_dir(),
        )?;
        let current = include_str!("game-arrays.tsv").to_string();
        let accepted = ACCEPTANCES.to_string();
        let scope = input_binding().to_string();
        accepted_arrays(CONTRACTS, &accepted, &current, &scope)?;
        Ok((accepted, current, scope))
    }
    #[test]
    fn ra9_exact_policy_reason_nonfirst_change_cannot_keep_old_acceptance() -> AuditResult<()> {
        let (test, current, scope) = baseline()?;
        let lines = current.lines().collect::<Vec<_>>();
        let row = lines.get(5).ok_or("nonfirst exact current policy")?;
        let fields = row.split('\t').collect::<Vec<_>>();
        let changed = current.replacen(
            row,
            &format!("{}\t{}\tblanket approved game data", fields[0], fields[1]),
            1,
        );
        assert!(accepted_arrays(CONTRACTS, &test, &changed, &scope).is_err());
        Ok(())
    }
    #[test]
    fn ra9_provisional_pending_prefix_is_not_final_specific_policy() -> AuditResult<()> {
        let (test, current, scope) = baseline()?;
        let row = current.lines().nth(1).ok_or("actual first policy")?;
        let fields = row.split('\t').collect::<Vec<_>>();
        let changed = current.replacen(
            row,
            &format!(
                "{}\t{}\tPENDING policy capture: old provisional contract",
                fields[0], fields[1]
            ),
            1,
        );
        assert!(accepted_arrays(CONTRACTS, &test, &changed, &scope).is_err());
        Ok(())
    }
    #[test]
    fn ra9_unaccepted_individual_policy_table_fails_closed() -> AuditResult<()> {
        let (accepted, current, scope) = baseline()?;
        let line = accepted
            .lines()
            .nth(2)
            .ok_or("nonfirst actual accepted array")?;
        let mut fields = line.split('\t').collect::<Vec<_>>();
        assert_ne!(fields[2], "UNACCEPTED");
        fields[2] = "UNACCEPTED";
        let bad = accepted.replacen(line, &fields.join("\t"), 1);
        assert!(accepted_arrays(CONTRACTS, &bad, &current, &scope).is_err());
        Ok(())
    }
}

#[test]
fn ra9_single_table_acceptance_census_rejects_missing_foreign_and_duplicate_after_valid_final_baseline()
-> AuditResult<()> {
    super::audit_at(&super::super::data_dir(), &super::super::reference_dir())?;
    let input = Inputs::load(&super::super::data_dir(), &super::super::reference_dir())?;
    let d = sim::load_ruleset(super::super::data_dir()).map_err(|e| e.to_string())?;
    let scope = scope_inputs::ObservedScope::read(&super::super::reference_dir())?;
    let audit = super::load_audit(&input, &d)?;
    let plans = source_plans::declare(&input, &d, &audit, &scope)?;
    let route = source_routes::declare(&input, &plans)?;
    let want = source_routes::single_table_keys(&route)?;
    let installed = include_str!("source-policy-acceptance.tsv");
    acceptance_census(installed, &want)?;
    let mut lines = installed.lines().map(str::to_owned).collect::<Vec<_>>();
    let nonfirst = lines.remove(lines.len() / 2);
    assert!(acceptance_census(&lines.join("\n"), &want).is_err());
    lines.push(nonfirst.clone());
    lines.push(nonfirst);
    assert!(acceptance_census(&lines.join("\n"), &want).is_err());
    let mut lines = installed.lines().map(str::to_owned).collect::<Vec<_>>();
    let mut row = lines[2].split('\t').map(str::to_owned).collect::<Vec<_>>();
    row[1].push_str("_FOREIGN");
    lines[2] = row.join("\t");
    assert!(acceptance_census(&lines.join("\n"), &want).is_err());
    Ok(())
}
