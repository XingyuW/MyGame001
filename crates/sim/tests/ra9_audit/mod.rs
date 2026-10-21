//! Preliminary RA9 integration in an ordinary validation copy; canonical acceptance is pending.
//! The typed modules compile in the recorded preliminary checks. Execution results, failures,
//! source bindings and controlled fixture migrations live in ra9-preliminary-logs/.
//! Actual source-policy decisions and native/client receipts remain fail-closed until bound.
//! Reading a reference value grants no coverage: only an actual successful comparison does.
//! Every old ra9_data_parity named test remains alongside these additional strict checks.
use std::collections::{BTreeMap, BTreeSet};

type AuditResult<T> = Result<T, String>;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct SourceKey {
    file: String,
    section: String,
    key: String,
}
impl SourceKey {
    fn new(file: &str, section: &str, key: &str) -> Self {
        Self {
            file: file.into(),
            section: section.into(),
            key: key.into(),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum Portion {
    Whole,
    Component(usize),
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Atom {
    key: SourceKey,
    portion: Portion,
}

#[derive(Clone, Debug)]
struct Reference {
    rows: BTreeMap<SourceKey, String>,
    shapes: BTreeMap<SourceKey, usize>,
}
impl Reference {
    fn from_rows(rows: impl IntoIterator<Item = (SourceKey, String)>) -> AuditResult<Self> {
        let mut stored = BTreeMap::new();
        for (key, value) in rows {
            if !matches!(key.file.as_str(), "rules" | "art") {
                return Err(format!("unsupported reference file {:?}", key.file));
            }
            if stored.insert(key.clone(), value).is_some() {
                return Err(format!("duplicate reference key {key:?}"));
            }
        }
        Ok(Self {
            rows: stored,
            shapes: BTreeMap::new(),
        })
    }

    // Call only for deliberately component-audited tuples. Every component must then be
    // compared or explicitly unused. Full-array comparisons otherwise use a Whole atom.
    fn require_components(&mut self, key: SourceKey, exact_len: usize) -> AuditResult<()> {
        let raw = self
            .rows
            .get(&key)
            .ok_or_else(|| format!("missing tuple {key:?}"))?;
        let found = raw.split(',').count();
        if found != exact_len {
            return Err(format!(
                "{key:?}: expected {exact_len} components, found {found}"
            ));
        }
        if self.shapes.insert(key.clone(), exact_len).is_some() {
            return Err(format!("component shape declared twice for {key:?}"));
        }
        Ok(())
    }

    fn expect(&self, key: SourceKey) -> AuditResult<Expected<String>> {
        if self.shapes.contains_key(&key) {
            return Err(format!(
                "{key:?}: component-audited tuple needs exact ordered component expectations"
            ));
        }
        let raw = self
            .rows
            .get(&key)
            .ok_or_else(|| format!("missing key {key:?}"))?;
        Ok(Expected::source(
            raw.clone(),
            Atom {
                key,
                portion: Portion::Whole,
            },
        ))
    }

    fn component(&self, key: SourceKey, index: usize) -> AuditResult<Expected<String>> {
        let len = self
            .shapes
            .get(&key)
            .ok_or_else(|| format!("no explicit shape for {key:?}"))?;
        if index >= *len {
            return Err(format!("component {index} outside {key:?}/{len}"));
        }
        let raw = self
            .rows
            .get(&key)
            .ok_or_else(|| format!("missing tuple {key:?}"))?;
        let value = raw
            .split(',')
            .nth(index)
            .ok_or_else(|| format!("missing {key:?}[{index}]"))?;
        Ok(Expected::source(
            value.into(),
            Atom {
                key,
                portion: Portion::Component(index),
            },
        ))
    }

    // An absent source default carries no invented key coverage. Presence/absence is still
    // checked by the adapter's explicit branch logic and the whole reference reconciliation.
    fn expect_or(&self, key: SourceKey, default: &str) -> AuditResult<Expected<String>> {
        if self.rows.contains_key(&key) {
            self.expect(key)
        } else {
            Ok(Expected::literal(default.to_owned()))
        }
    }

    fn expected_atoms(&self) -> BTreeSet<Atom> {
        self.rows
            .keys()
            .flat_map(|key| match self.shapes.get(key) {
                Some(len) => (0..*len)
                    .map(|index| Atom {
                        key: key.clone(),
                        portion: Portion::Component(index),
                    })
                    .collect::<Vec<_>>(),
                None => vec![Atom {
                    key: key.clone(),
                    portion: Portion::Whole,
                }],
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
struct Expected<T> {
    value: T,
    deps: BTreeSet<Atom>,
}
impl<T> Expected<T> {
    fn literal(value: T) -> Self {
        Self {
            value,
            deps: BTreeSet::new(),
        }
    }
    fn source(value: T, source: Atom) -> Self {
        Self {
            value,
            deps: BTreeSet::from([source]),
        }
    }
    fn map<U>(self, f: impl FnOnce(T) -> U) -> Expected<U> {
        Expected {
            value: f(self.value),
            deps: self.deps,
        }
    }
    fn try_map<U>(self, f: impl FnOnce(T) -> AuditResult<U>) -> AuditResult<Expected<U>> {
        Ok(Expected {
            value: f(self.value)?,
            deps: self.deps,
        })
    }
    fn zip<U>(self, other: Expected<U>) -> Expected<(T, U)> {
        let mut deps = self.deps;
        deps.extend(other.deps);
        Expected {
            value: (self.value, other.value),
            deps,
        }
    }
    // A branch selector remains a dependency whichever branch was taken. Untaken branch
    // values are NOT silently credited: they need their own comparison/non-use disposition.
    fn because<U>(mut self, selector: &Expected<U>) -> Self {
        self.deps.extend(selector.deps.iter().cloned());
        self
    }
}
fn sequence<T>(parts: impl IntoIterator<Item = Expected<T>>) -> Expected<Vec<T>> {
    let mut values = Vec::new();
    let mut deps = BTreeSet::new();
    for part in parts {
        values.push(part.value);
        deps.extend(part.deps);
    }
    Expected {
        value: values,
        deps,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct WitnessId {
    // E.g. engineer@rules:ENGINEER and engineer@rules:SENGINEER remain distinct.
    consumer: String,
    // Physical file/list/field or indexed component, not only a global field name.
    field: String,
}
impl WitnessId {
    fn new(consumer: &str, field: &str) -> Self {
        Self {
            consumer: consumer.into(),
            field: field.into(),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct DifferenceId {
    game_id: String,
    field: String,
    game_value: String,
    reference_value: String,
}
#[derive(Clone, Debug)]
struct Difference {
    id: DifferenceId,
    reason: String,
}
#[derive(Clone, Debug)]
struct Unused {
    atom: Atom,
    exact_value: String,
    reason: String,
    code_evidence: String,
}
#[derive(Clone, Debug)]
enum ComparisonRelation {
    Exact,
    MemberOf(Vec<String>),
}
#[derive(Clone, Debug)]
enum Witness {
    Compared {
        actual: String,
        expected: String,
        dependencies: BTreeSet<Atom>,
        difference: Option<DifferenceId>,
        relation: ComparisonRelation,
    },
    GameOwned {
        actual: String,
        reason: String,
        code_evidence: String,
    },
}

#[derive(Clone, Debug, Default)]
struct Audit {
    // Build this by walking actual shipped entries/fields plus EACH mapping consumer, not
    // from successful adapter calls. A skipped item/twin/branch then leaves a required witness.
    required: BTreeSet<WitnessId>,
    // Independent demands for consumers of shared reference rows. A single global success
    // cannot cover two mapped consumers or two actual fields.
    required_dependencies: BTreeSet<(WitnessId, Atom)>,
    witnesses: BTreeMap<WitnessId, Witness>,
    physical_aliases: BTreeMap<WitnessId, WitnessId>,
    differences: BTreeMap<DifferenceId, Difference>,
    used_differences: BTreeSet<DifferenceId>,
    unused: BTreeMap<Atom, Unused>,
}
impl Audit {
    fn require(&mut self, id: WitnessId) -> AuditResult<()> {
        if !self.required.insert(id.clone()) {
            return Err(format!("duplicate required field {id:?}"));
        }
        Ok(())
    }
    fn bind_physical_alias(&mut self, alias: WitnessId, physical: WitnessId) -> AuditResult<()> {
        if !self.required.contains(&physical) {
            return Err(format!(
                "alias target not independently enumerated {physical:?}"
            ));
        }
        if alias == physical {
            return Ok(());
        }
        if self.required.contains(&alias) || self.physical_aliases.contains_key(&alias) {
            return Err(format!("ambiguous physical alias {alias:?}"));
        }
        self.physical_aliases.insert(alias, physical);
        Ok(())
    }
    fn physical(&self, id: WitnessId) -> WitnessId {
        self.physical_aliases.get(&id).cloned().unwrap_or(id)
    }
    fn require_dependency(&mut self, id: WitnessId, atom: Atom) -> AuditResult<()> {
        let id = self.physical(id);
        if !self.required.contains(&id) {
            return Err(format!("dependency demand on unregistered {id:?}"));
        }
        if !self
            .required_dependencies
            .insert((id.clone(), atom.clone()))
        {
            return Err(format!("duplicate dependency demand {id:?} {atom:?}"));
        }
        Ok(())
    }
    fn add_difference(&mut self, difference: Difference) -> AuditResult<()> {
        if difference.reason.trim().is_empty() {
            return Err(format!("difference lacks reason {:?}", difference.id));
        }
        let id = difference.id.clone();
        if self.differences.insert(id.clone(), difference).is_some() {
            return Err(format!("duplicate difference {id:?}"));
        }
        Ok(())
    }
    fn check(
        &mut self,
        id: WitnessId,
        game_id: &str,
        difference_field: &str,
        actual: String,
        expected: Expected<String>,
        source_display: String,
    ) -> AuditResult<()> {
        let id = self.physical(id);
        if !self.required.contains(&id) {
            return Err(format!("comparison on unregistered actual field {id:?}"));
        }
        if self.witnesses.contains_key(&id) {
            return Err(format!("duplicate comparison {id:?}"));
        }
        let key = DifferenceId {
            game_id: game_id.into(),
            field: difference_field.into(),
            game_value: actual.clone(),
            reference_value: source_display,
        };
        let listed = self.differences.contains_key(&key);
        let difference = match (actual == expected.value, listed) {
            (true, false) => None,
            (true, true) => return Err(format!("stale equal-value difference {key:?}")),
            (false, false) => {
                return Err(format!(
                    "unexplained {id:?}: actual {actual:?}, expected {:?}",
                    expected.value
                ));
            }
            (false, true) => Some(key),
        };
        // No mutation of coverage occurs before all validation above succeeds.
        if let Some(key) = &difference {
            self.used_differences.insert(key.clone());
        }
        self.witnesses.insert(
            id,
            Witness::Compared {
                actual,
                expected: expected.value,
                dependencies: expected.deps,
                difference,
                relation: ComparisonRelation::Exact,
            },
        );
        Ok(())
    }
    fn check_member(
        &mut self,
        id: WitnessId,
        actual: String,
        expected: Expected<Vec<String>>,
    ) -> AuditResult<()> {
        let id = self.physical(id);
        if !self.required.contains(&id) {
            return Err(format!("unregistered membership {id:?}"));
        }
        if self.witnesses.contains_key(&id) {
            return Err(format!("duplicate membership {id:?}"));
        }
        let unique = expected.value.iter().collect::<BTreeSet<_>>();
        if unique.len() != expected.value.len() || !expected.value.contains(&actual) {
            return Err(format!(
                "actual {id:?} {actual:?} not in independently derived {:?}",
                expected.value
            ));
        }
        // Actual ID is tested against an independent allowed set; it never enters expected.
        self.witnesses.insert(
            id,
            Witness::Compared {
                actual,
                expected: expected.value.join("|"),
                dependencies: expected.deps,
                difference: None,
                relation: ComparisonRelation::MemberOf(expected.value),
            },
        );
        Ok(())
    }
    fn game_owned(
        &mut self,
        id: WitnessId,
        actual: String,
        reason: String,
        code_evidence: String,
    ) -> AuditResult<()> {
        let id = self.physical(id);
        if !self.required.contains(&id) {
            return Err(format!("own field not enumerated {id:?}"));
        }
        if reason.trim().is_empty() || code_evidence.trim().is_empty() {
            return Err(format!(
                "game-owned field needs semantic reason and code evidence {id:?}"
            ));
        }
        if self.witnesses.contains_key(&id) {
            return Err(format!("duplicate own/compared field {id:?}"));
        }
        self.witnesses.insert(
            id,
            Witness::GameOwned {
                actual,
                reason,
                code_evidence,
            },
        );
        Ok(())
    }
    fn add_unused(&mut self, row: Unused) -> AuditResult<()> {
        if row.reason.trim().is_empty() || row.code_evidence.trim().is_empty() {
            return Err(format!(
                "unused atom needs semantic reason and source evidence {:?}",
                row.atom
            ));
        }
        let atom = row.atom.clone();
        if self.unused.insert(atom.clone(), row).is_some() {
            return Err(format!("duplicate unused atom {atom:?}"));
        }
        Ok(())
    }
    fn validate_witness(&self, id: &WitnessId, witness: &Witness) -> AuditResult<()> {
        match witness {
            Witness::Compared {
                actual,
                expected,
                difference,
                relation,
                ..
            } => match relation {
                ComparisonRelation::Exact => {
                    if difference.is_none() && actual != expected {
                        return Err(format!("corrupt unequal unlisted witness {id:?}"));
                    }
                    if let Some(key) = difference
                        && (actual == expected || !self.differences.contains_key(key))
                    {
                        return Err(format!("corrupt listed witness {id:?}"));
                    }
                }
                ComparisonRelation::MemberOf(allowed) => {
                    let unique = allowed.iter().collect::<BTreeSet<_>>();
                    if difference.is_some()
                        || allowed.is_empty()
                        || unique.len() != allowed.len()
                        || !allowed.contains(actual)
                        || allowed.join("|") != *expected
                    {
                        return Err(format!("corrupt independent membership witness {id:?}"));
                    }
                }
            },
            Witness::GameOwned {
                actual,
                reason,
                code_evidence,
            } => {
                let _physical_value = actual;
                if reason.trim().is_empty() || code_evidence.trim().is_empty() {
                    return Err(format!("unexplained own witness {id:?}"));
                }
            }
        }
        Ok(())
    }
    fn finish(&self, reference: &Reference) -> AuditResult<()> {
        let found = self.witnesses.keys().cloned().collect::<BTreeSet<_>>();
        if found != self.required {
            return Err(format!(
                "missing/unexpected actual-field witnesses: missing {:?}, unexpected {:?}",
                self.required.difference(&found).collect::<Vec<_>>(),
                found.difference(&self.required).collect::<Vec<_>>()
            ));
        }
        let mut used = BTreeSet::new();
        let mut pairs = BTreeSet::new();
        for (id, witness) in &self.witnesses {
            self.validate_witness(id, witness)?;
            if let Witness::Compared { dependencies, .. } = witness {
                for atom in dependencies {
                    used.insert(atom.clone());
                    pairs.insert((id.clone(), atom.clone()));
                }
            }
        }
        // Exact equality forbids accidental foreign-read coverage by `because`/`zip`.
        // Demand enumeration is independent of adapter output and uses an explicitly reviewed
        // branch model. No adapter may enlarge its own demand set to hide a foreign dependency.
        let missing = self
            .required_dependencies
            .difference(&pairs)
            .collect::<Vec<_>>();
        let foreign = pairs
            .difference(&self.required_dependencies)
            .collect::<Vec<_>>();
        if !missing.is_empty() || !foreign.is_empty() {
            return Err(format!(
                "consumer dependency mismatch: missing {missing:?}, foreign {foreign:?}"
            ));
        }
        let expected = reference.expected_atoms();
        for atom in &used {
            if !expected.contains(atom) {
                return Err(format!(
                    "comparison names nonexistent/wrong-shape source {atom:?}"
                ));
            }
            if self.unused.contains_key(atom) {
                return Err(format!("audited/unused overlap {atom:?}"));
            }
        }
        for (atom, row) in &self.unused {
            if !expected.contains(atom) {
                return Err(format!("stale/nonexistent unused source {atom:?}"));
            }
            let raw = reference
                .rows
                .get(&atom.key)
                .ok_or_else(|| format!("missing unused source {atom:?}"))?;
            let value = match atom.portion {
                Portion::Whole => raw.as_str(),
                Portion::Component(index) => raw
                    .split(',')
                    .nth(index)
                    .ok_or_else(|| format!("missing component {atom:?}"))?,
            };
            if value != row.exact_value {
                return Err(format!(
                    "unused source changed {atom:?}: {value:?} != {:?}",
                    row.exact_value
                ));
            }
        }
        let accounted = used
            .union(&self.unused.keys().cloned().collect())
            .cloned()
            .collect::<BTreeSet<_>>();
        if accounted != expected {
            return Err(format!(
                "uncovered reference atoms {:?}",
                expected.difference(&accounted).collect::<Vec<_>>()
            ));
        }
        let listed = self.differences.keys().cloned().collect::<BTreeSet<_>>();
        if listed != self.used_differences {
            return Err(format!(
                "stale/unmet differences {:?}",
                listed
                    .difference(&self.used_differences)
                    .collect::<Vec<_>>()
            ));
        }
        Ok(())
    }
}

// These tests are DRAFT CODE, NOT RUN. Integration must add tests against actual loaded shipped
// fields as well; synthetic ledger-only negatives do not prove production adapters are causal.
#[cfg(test)]
mod draft_tests {
    use super::*;
    fn scalar() -> AuditResult<(Reference, Audit, WitnessId)> {
        let key = SourceKey::new("rules", "MTNK", "Cost");
        let reference = Reference::from_rows([(key.clone(), "700".into())])?;
        let mut audit = Audit::default();
        let id = WitnessId::new("grizzly@rules:MTNK", "economy.units.cost");
        audit.require(id.clone())?;
        audit.require_dependency(
            id.clone(),
            Atom {
                key,
                portion: Portion::Whole,
            },
        )?;
        Ok((reference, audit, id))
    }
    #[test]
    fn reading_a_key_without_a_comparison_does_not_cover_it() -> AuditResult<()> {
        let (r, a, _) = scalar()?;
        let _ = r.expect(SourceKey::new("rules", "MTNK", "Cost"))?;
        assert!(a.finish(&r).is_err());
        Ok(())
    }
    #[test]
    fn a_successful_actual_comparison_records_its_dependency() -> AuditResult<()> {
        let (r, mut a, id) = scalar()?;
        a.check(
            id,
            "grizzly",
            "cost",
            "700".into(),
            r.expect(SourceKey::new("rules", "MTNK", "Cost"))?,
            "700".into(),
        )?;
        a.finish(&r)
    }
    #[test]
    fn an_actual_value_mutation_is_unexplained() -> AuditResult<()> {
        let (r, mut a, id) = scalar()?;
        assert!(
            a.check(
                id,
                "grizzly",
                "cost",
                "701".into(),
                r.expect(SourceKey::new("rules", "MTNK", "Cost"))?,
                "700".into()
            )
            .is_err()
        );
        assert!(a.finish(&r).is_err());
        Ok(())
    }
    #[test]
    fn another_twin_is_a_distinct_required_consumer() -> AuditResult<()> {
        let (r, mut a, id) = scalar()?;
        a.require(WitnessId::new(
            "grizzly@rules:SECOND_TWIN",
            "economy.units.cost",
        ))?;
        a.check(
            id,
            "grizzly",
            "cost",
            "700".into(),
            r.expect(SourceKey::new("rules", "MTNK", "Cost"))?,
            "700".into(),
        )?;
        assert!(a.finish(&r).is_err());
        Ok(())
    }
    #[test]
    fn duplicate_reference_rows_fail() -> AuditResult<()> {
        let k = SourceKey::new("art", "GACNST", "Foundation");
        assert!(Reference::from_rows([(k.clone(), "4x4".into()), (k, "3x3".into())]).is_err());
        Ok(())
    }

    #[test]
    fn a_foreign_because_dependency_cannot_create_coverage() -> AuditResult<()> {
        let (mut r, mut a, id) = scalar()?;
        let foreign = SourceKey::new("rules", "HTNK", "Cost");
        r.rows.insert(foreign.clone(), "900".into());
        let expected = r
            .expect(SourceKey::new("rules", "MTNK", "Cost"))?
            .because(&r.expect(foreign)?);
        a.check(id, "grizzly", "cost", "700".into(), expected, "700".into())?;
        assert!(a.finish(&r).is_err());
        Ok(())
    }
    fn completed_scalar() -> AuditResult<(Reference, Audit)> {
        let (r, mut a, id) = scalar()?;
        a.check(
            id,
            "grizzly",
            "cost",
            "700".into(),
            r.expect(SourceKey::new("rules", "MTNK", "Cost"))?,
            "700".into(),
        )?;
        Ok((r, a))
    }
    #[test]
    fn new_reference_key_is_uncovered() -> AuditResult<()> {
        let (mut r, a) = completed_scalar()?;
        r.rows
            .insert(SourceKey::new("rules", "MTNK", "NewKey"), "yes".into());
        assert!(a.finish(&r).is_err());
        Ok(())
    }
    #[test]
    fn changed_unused_value_is_rejected() -> AuditResult<()> {
        let (mut r, mut a) = completed_scalar()?;
        let key = SourceKey::new("art", "MTNK", "Height");
        r.rows.insert(key.clone(), "2".into());
        a.add_unused(Unused {
            atom: Atom {
                key: key.clone(),
                portion: Portion::Whole,
            },
            exact_value: "1".into(),
            reason: "Native art elevation has no generated-sprite model".into(),
            code_evidence: "renderer uses original generated sprite geometry".into(),
        })?;
        assert!(a.finish(&r).is_err());
        Ok(())
    }
    #[test]
    fn used_unused_overlap_is_rejected() -> AuditResult<()> {
        let (r, mut a) = completed_scalar()?;
        a.add_unused(Unused {
            atom: Atom {
                key: SourceKey::new("rules", "MTNK", "Cost"),
                portion: Portion::Whole,
            },
            exact_value: "700".into(),
            reason: "Deliberately invalid test overlap".into(),
            code_evidence: "test negative".into(),
        })?;
        assert!(a.finish(&r).is_err());
        Ok(())
    }
    #[test]
    fn stale_unused_key_is_rejected() -> AuditResult<()> {
        let (r, mut a) = completed_scalar()?;
        a.add_unused(Unused {
            atom: Atom {
                key: SourceKey::new("art", "MISSING", "Height"),
                portion: Portion::Whole,
            },
            exact_value: "1".into(),
            reason: "Deliberately stale test row".into(),
            code_evidence: "test negative".into(),
        })?;
        assert!(a.finish(&r).is_err());
        Ok(())
    }
    #[test]
    fn duplicate_unused_atom_is_rejected() -> AuditResult<()> {
        let (_, mut a) = completed_scalar()?;
        let row = Unused {
            atom: Atom {
                key: SourceKey::new("art", "MTNK", "Height"),
                portion: Portion::Whole,
            },
            exact_value: "1".into(),
            reason: "Original sprite does not consume native elevation".into(),
            code_evidence: "test negative".into(),
        };
        a.add_unused(row.clone())?;
        assert!(a.add_unused(row).is_err());
        Ok(())
    }
    #[test]
    fn exact_tuple_shape_rejects_truncation_and_extension() -> AuditResult<()> {
        let key = SourceKey::new("rules", "Powerups", "Armor");
        for raw in ["10,ARMOR,yes", "10,ARMOR,yes,1.5,EXTRA"] {
            let mut r = Reference::from_rows([(key.clone(), raw.into())])?;
            assert!(r.require_components(key.clone(), 4).is_err());
        }
        Ok(())
    }
    #[test]
    fn comparing_one_tuple_component_does_not_cover_others() -> AuditResult<()> {
        let key = SourceKey::new("rules", "Powerups", "Armor");
        let mut r = Reference::from_rows([(key.clone(), "10,ARMOR,yes,1.5".into())])?;
        r.require_components(key.clone(), 4)?;
        let mut a = Audit::default();
        let id = WitnessId::new("crate.armor", "rules.powerups.share");
        a.require(id.clone())?;
        a.require_dependency(
            id.clone(),
            Atom {
                key: key.clone(),
                portion: Portion::Component(0),
            },
        )?;
        a.check(
            id,
            "rules",
            "powerups.armor.share",
            "10".into(),
            r.component(key, 0)?,
            "10".into(),
        )?;
        assert!(a.finish(&r).is_err());
        Ok(())
    }
    #[test]
    fn physical_alias_binding_alone_never_creates_coverage() -> AuditResult<()> {
        let (r, mut a, physical) = scalar()?;
        let alias = WitnessId::new("grizzly@rules:MTNK", "semantic.cost");
        a.bind_physical_alias(alias.clone(), physical)?;
        assert!(a.finish(&r).is_err());
        a.check(
            alias,
            "grizzly",
            "cost",
            "700".into(),
            r.expect(SourceKey::new("rules", "MTNK", "Cost"))?,
            "700".into(),
        )?;
        a.finish(&r)
    }
    #[test]
    fn an_alias_cannot_register_a_missing_physical_field() -> AuditResult<()> {
        let (_, mut a, _) = scalar()?;
        assert!(
            a.bind_physical_alias(
                WitnessId::new("grizzly", "semantic.fake"),
                WitnessId::new("grizzly", "physical.missing")
            )
            .is_err()
        );
        Ok(())
    }
    #[test]
    fn two_semantic_aliases_cannot_double_count_one_actual_field() -> AuditResult<()> {
        let (r, mut a, physical) = scalar()?;
        let first = WitnessId::new("grizzly@rules:MTNK", "semantic.first");
        let second = WitnessId::new("grizzly@rules:MTNK", "semantic.second");
        a.bind_physical_alias(first.clone(), physical.clone())?;
        a.bind_physical_alias(second.clone(), physical)?;
        a.check(
            first,
            "grizzly",
            "cost",
            "700".into(),
            r.expect(SourceKey::new("rules", "MTNK", "Cost"))?,
            "700".into(),
        )?;
        assert!(
            a.check(
                second,
                "grizzly",
                "cost",
                "700".into(),
                r.expect(SourceKey::new("rules", "MTNK", "Cost"))?,
                "700".into()
            )
            .is_err()
        );
        Ok(())
    }
}

#[path = "actual.rs"]
mod actual;
#[path = "array-negatives.rs"]
mod array_negatives;
#[path = "array-plans.rs"]
mod array_plans;
#[path = "arrays.rs"]
mod arrays;
#[path = "behavior87-admission.rs"]
mod behavior87_admission;
#[path = "carry38-admission.rs"]
mod carry38_admission;
#[path = "combat.rs"]
mod combat;
#[path = "completion.rs"]
mod completion;
#[path = "completion-whole.rs"]
mod completion_whole;
#[path = "consumer62-admission.rs"]
mod consumer62_admission;
#[path = "contracts.rs"]
mod contracts;
#[path = "country-rosters.rs"]
mod country_rosters;
#[path = "fire-audit.rs"]
mod fire_audit;
#[path = "fire-keys.rs"]
mod fire_keys;
#[path = "guard75-admission.rs"]
mod guard75_admission;
#[path = "items.rs"]
mod items;
#[path = "legacy904-loaded.rs"]
mod legacy904_loaded;
#[cfg(test)]
#[path = "legacy904-scope-negatives.rs"]
mod legacy904_scope_negatives;
#[path = "legacy904-support.rs"]
mod legacy904_support;
#[path = "legacy-ai-policy.rs"]
mod legacy_ai_policy;
#[path = "legacy-model-policy.rs"]
mod legacy_model_policy;
#[path = "legacy-source-admission.rs"]
mod legacy_source_admission;
#[path = "masked-speed-admission.rs"]
mod masked_speed_admission;
#[path = "metadata-admission.rs"]
mod metadata_admission;
#[path = "model162-admission.rs"]
mod model162_admission;
#[path = "native.rs"]
mod native;
#[path = "negatives.rs"]
mod negatives;
#[path = "nested-negatives.rs"]
mod nested_negatives;
#[path = "nested-plans.rs"]
mod nested_plans;
#[path = "owned.rs"]
mod owned;
#[path = "policy-admission.rs"]
mod policy_admission;
#[path = "raw.rs"]
mod raw;
#[path = "residual-admission.rs"]
mod residual_admission;
#[path = "rules.rs"]
mod rules;
#[path = "schema.rs"]
mod schema;
#[path = "schema10-adapters.rs"]
mod schema10_adapters;
#[path = "scope-inputs.rs"]
mod scope_inputs;
#[path = "source-disposition.rs"]
mod source_disposition;
#[path = "source-plans.rs"]
mod source_plans;
#[path = "source-routes.rs"]
mod source_routes;
#[path = "special-source-admission.rs"]
mod special_source_admission;
#[path = "specialist.rs"]
mod specialist;
use raw::{Raw, Reader, atom, field, list, tuple};
use rules::MappingRow;
use sim::{Definitions, SkirmishDefinitions};
use std::path::{Path, PathBuf};

struct Inputs {
    data_dir: PathBuf,
    reference: Reference,
    mappings: Vec<MappingRow>,
    weapons: Vec<(String, String)>,
    warheads: Vec<(String, String)>,
    differences: Vec<Difference>,
    documents: BTreeMap<String, Raw>,
}
fn tsv(path: &Path, header: &[&str]) -> AuditResult<Vec<Vec<String>>> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut lines = text.lines();
    let actual = lines
        .next()
        .ok_or("empty TSV")?
        .split('\t')
        .collect::<Vec<_>>();
    if actual != header {
        return Err(format!("{}: TSV header", path.display()));
    }
    let mut result = Vec::new();
    for (index, line) in lines.enumerate() {
        let row = line.split('\t').map(str::to_owned).collect::<Vec<_>>();
        if row.len() != header.len() {
            return Err(format!("{} row{} width", path.display(), index + 2));
        }
        result.push(row);
    }
    Ok(result)
}
impl Inputs {
    fn load(data: &Path, reference_dir: &Path) -> AuditResult<Self> {
        let rows = tsv(
            &reference_dir.join("ra2_reference.tsv"),
            &["file", "section", "key", "value"],
        )?;
        let mut reference = Reference::from_rows(
            rows.into_iter()
                .map(|r| (SourceKey::new(&r[0], &r[1], &r[2]), r[3].clone())),
        )?;
        actual::declare_powerup_shapes(&mut reference)?;
        rules::declare_difficulty_shapes(&mut reference)?;
        let mappings = tsv(
            &reference_dir.join("ra2_mapping.tsv"),
            &["game_id", "kind", "section", "side", "note"],
        )?
        .into_iter()
        .map(|r| {
            if !matches!(r[1].as_str(), "unit" | "building") {
                return Err(format!("unknown mapping kind {}", r[1]));
            }
            Ok(MappingRow {
                game_id: r[0].clone(),
                building: r[1] == "building",
                section: r[2].clone(),
                side: r[3].clone(),
                note: r[4].clone(),
            })
        })
        .collect::<AuditResult<Vec<_>>>()?;
        let mut identities = BTreeSet::new();
        for row in &mappings {
            if !identities.insert((row.game_id.clone(), row.section.clone(), row.building)) {
                return Err("duplicate mapping consumer".into());
            }
        }
        let (mut weapons, mut warheads) = (Vec::new(), Vec::new());
        let mut combatids = BTreeSet::new();
        let mut combatsections = BTreeSet::new();
        for r in tsv(
            &reference_dir.join("ra2_combat.tsv"),
            &["game_id", "kind", "section"],
        )? {
            if !combatids.insert((r[1].clone(), r[0].clone(), r[2].clone()))
                || !combatsections.insert((r[1].clone(), r[2].clone()))
            {
                return Err("duplicate combat mapping identity".into());
            }
            match r[1].as_str() {
                "weapon" => weapons.push((r[0].clone(), r[2].clone())),
                "warhead" => warheads.push((r[0].clone(), r[2].clone())),
                _ => return Err("unknown combat mapping kind".into()),
            }
        }
        let differences = tsv(
            &reference_dir.join("ra2_differences.tsv"),
            &[
                "game_id",
                "field",
                "game_value",
                "reference_value",
                "reason",
            ],
        )?
        .into_iter()
        .map(|r| Difference {
            id: DifferenceId {
                game_id: r[0].clone(),
                field: r[1].clone(),
                game_value: r[2].clone(),
                reference_value: r[3].clone(),
            },
            reason: r[4].clone(),
        })
        .collect();
        let mut documents = BTreeMap::new();
        for file in [
            "units.ron",
            "economy.ron",
            "combat.ron",
            "arsenal.ron",
            "rules.ron",
            "skirmish.ron",
            "ai.ron",
        ] {
            let text =
                std::fs::read_to_string(data.join(file)).map_err(|e| format!("{file}: {e}"))?;
            documents.insert(file.into(), Reader::new(&text).document()?);
        }
        schema::check_all(&documents)?;
        Ok(Self {
            data_dir: data.to_path_buf(),
            reference,
            mappings,
            weapons,
            warheads,
            differences,
            documents,
        })
    }
    fn item_ids(&self) -> AuditResult<BTreeMap<String, String>> {
        let mut ids = BTreeMap::new();
        for row in &self.mappings {
            if let Some(old) = ids.insert(row.section.clone(), row.game_id.clone())
                && old != row.game_id
            {
                return Err("source section maps to multiple game IDs".into());
            }
        }
        Ok(ids)
    }
}
fn index(rows: &[(String, String)]) -> BTreeMap<String, String> {
    rows.iter().map(|(id, s)| (s.clone(), id.clone())).collect()
}
fn bind(a: &mut Audit, c: &str, semantic: &str, physical: &str) -> AuditResult<()> {
    a.bind_physical_alias(WitnessId::new(c, semantic), WitnessId::new(c, physical))
}
fn actual_item_consumers(
    input: &Inputs,
    file: &str,
    list_name: &str,
) -> BTreeMap<String, Vec<String>> {
    let mut result: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if file == "combat.ron" && matches!(list_name, "weapons" | "warheads") {
        let (kind, rows) = if list_name == "weapons" {
            ("weapon", &input.weapons)
        } else {
            ("warhead", &input.warheads)
        };
        for (id, s) in rows {
            result
                .entry(id.clone())
                .or_default()
                .push(format!("{kind}.{id}@rules:{s}"));
        }
    } else if file == "combat.ron" && list_name == "effects" {
        for kind in sim::Effect::ALL {
            let id = kind.id();
            result.insert(id.into(), vec![format!("effect.{id}")]);
        }
    } else {
        let building = matches!(list_name, "buildings" | "refineries" | "refunds");
        for row in &input.mappings {
            if (list_name == "entries") || row.building == building {
                result
                    .entry(row.game_id.clone())
                    .or_default()
                    .push(format!("{}@rules:{}", row.game_id, row.section));
            }
        }
    }
    result
}
fn declare_physical(a: &mut Audit, input: &Inputs, d: &Definitions) -> AuditResult<()> {
    // All actual IDs must equal the whole independent mapping catalogue, retaining twins.
    let mapped_units = input
        .mappings
        .iter()
        .filter(|v| !v.building)
        .map(|v| v.game_id.as_str())
        .collect::<BTreeSet<_>>();
    let mapped_buildings = input
        .mappings
        .iter()
        .filter(|v| v.building)
        .map(|v| v.game_id.as_str())
        .collect::<BTreeSet<_>>();
    if mapped_units != d.units().iter().map(|v| v.id.as_str()).collect()
        || mapped_buildings != d.buildings().iter().map(|v| v.id.as_str()).collect()
    {
        return Err("loaded units/buildings and mappings differ".into());
    }
    for (file, name, identity, names) in schema::ITEM_SCHEMAS {
        let consumers = actual_item_consumers(input, file, name);
        let policies = names
            .iter()
            .map(|n| {
                (
                    n.to_string(),
                    if n == identity {
                        raw::FieldPolicy::Identity
                    } else {
                        raw::FieldPolicy::Compared
                    },
                )
            })
            .collect();
        let spec = raw::EntrySpec {
            list: name.to_string(),
            identity: identity.to_string(),
            fields: policies,
        };
        raw::enumerate_actual_entries(a, file, &input.documents[*file], &spec, &consumers)?;
        // Optional lists are checked by membership projections; their absence invents no row.
        // Required base lists must also include every independently mapped identity.
        if !matches!(*name, "harvesters" | "refineries" | "refunds") {
            let actual = list(field(&input.documents[*file], name, file)?, name)?
                .iter()
                .map(|row| atom(field(row, identity, name)?, name).map(str::to_owned))
                .collect::<AuditResult<BTreeSet<_>>>()?;
            if actual != consumers.keys().cloned().collect() {
                return Err(format!("{file}.{name}: missing mapped entry"));
            }
        }
    }
    for row in &input.mappings {
        let c = format!("{}@rules:{}", row.game_id, row.section);
        let name = if row.building { "buildings" } else { "units" };
        for f in ["cost", "build_time", "producer"] {
            bind(
                a,
                &c,
                &format!("economy.production.{f}"),
                &format!("economy.ron.{name}.{f}"),
            )?;
        }
        for f in ["hit_points", "sight"] {
            bind(
                a,
                &c,
                &format!("combat.{f}"),
                &format!("combat.ron.{name}.{f}"),
            )?;
        }
        for f in [
            "armour",
            "weapons",
            "elite_weapons",
            "air_range_bonus",
            "land_targeting",
            "naval_targeting",
        ] {
            bind(
                a,
                &c,
                &format!("combat.items.{f}"),
                &format!("combat.ron.{name}.{f}"),
            )?;
        }
        if row.building {
            bind(a, &c, "combat.items.wall", "combat.ron.buildings.wall")?;
            for f in [
                "width",
                "height",
                "power_produced",
                "power_consumed",
                "sell_value",
                "free_units",
            ] {
                bind(
                    a,
                    &c,
                    &format!("economy.buildings.{f}"),
                    &format!("economy.ron.buildings.{f}"),
                )?;
            }
        } else {
            for f in ["organic", "underwater"] {
                bind(
                    a,
                    &c,
                    &format!("combat.items.{f}"),
                    &format!("combat.ron.units.{f}"),
                )?;
            }
            bind(a, &c, "units.units.speed", "units.ron.units.speed")?;
        }
        let spec = schema::ITEM_SCHEMAS
            .iter()
            .find(|(f, l, _, _)| *f == "arsenal.ron" && *l == "entries")
            .ok_or("arsenal fixed schema missing")?;
        for f in spec.3.iter().copied().filter(|f| *f != "id") {
            bind(
                a,
                &c,
                &format!("arsenal.{f}"),
                &format!("arsenal.ron.entries.{f}"),
            )?;
        }
        if row.building {
            for f in [
                "infantry",
                "crusher",
                "crushable",
                "trainable",
                "self_healing",
                "ammo",
                "airport_bound",
                "cloakable",
            ] {
                bind(
                    a,
                    &c,
                    &format!("arsenal.{f}.cross_kind"),
                    &format!("arsenal.ron.entries.{f}"),
                )?;
            }
        }
        // Aliases require actual optional-row existence, established independently above.
        for (l, identity, fs) in [
            (
                "harvesters",
                "unit",
                &["capacity", "harvest_rate", "harvest_interval"][..],
            ),
            (
                "refineries",
                "building",
                &["credits_per_ore", "credits_per_gem"][..],
            ),
            ("refunds", "building", &["credits"][..]),
        ] {
            let present = list(field(&input.documents["economy.ron"], l, "economy.ron")?, l)?
                .iter()
                .any(|entry| {
                    field(entry, identity, l)
                        .and_then(|v| atom(v, l))
                        .is_ok_and(|id| id == row.game_id)
                });
            if present {
                for f in fs {
                    bind(
                        a,
                        &c,
                        &format!("economy.{l}.{f}"),
                        &format!("economy.ron.{l}.{f}"),
                    )?;
                }
            }
        }
    }
    for (kind, rows, name) in [
        ("weapon", &input.weapons, "weapons"),
        ("warhead", &input.warheads, "warheads"),
    ] {
        let spec = schema::ITEM_SCHEMAS
            .iter()
            .find(|(f, l, _, _)| *f == "combat.ron" && *l == name)
            .ok_or("combat fixed schema missing")?;
        for (id, s) in rows {
            let c = format!("{kind}.{id}@rules:{s}");
            for f in spec.3.iter().copied().filter(|f| *f != "id") {
                bind(
                    a,
                    &c,
                    &format!("combat.{name}.{f}"),
                    &format!("combat.ron.{name}.{f}"),
                )?;
            }
        }
    }
    for kind in sim::Effect::ALL {
        for f in ["damage", "warhead"] {
            bind(
                a,
                &format!("effect.{}", kind.id()),
                &format!("combat.effects.{f}"),
                &format!("combat.ron.effects.{f}"),
            )?;
        }
    }
    // Rule records and scalar fields are walked from raw data before adapters as well.
    for group in combat::SCALAR_ROWS
        .iter()
        .map(|row| row.0)
        .collect::<BTreeSet<_>>()
    {
        let node = field(&input.documents["rules.ron"], group, "rules.ron")?;
        for (f, _) in tuple(node, group)? {
            let id = WitnessId::new("rules", &format!("rules.ron.{group}.{f}"));
            if !a.required.contains(&id) {
                a.require(id)?;
            }
            bind(
                a,
                "rules",
                &format!("rules.{group}.{f}"),
                &format!("rules.ron.{group}.{f}"),
            )?;
        }
    }
    for (index, entry) in list(
        field(&input.documents["rules.ron"], "countries", "rules.ron")?,
        "countries",
    )?
    .iter()
    .enumerate()
    {
        for (f, _) in tuple(entry, "countries")? {
            let c = format!("country[{index}]");
            let p = format!("rules.ron.countries.{f}");
            a.require(WitnessId::new(&c, &p))?;
            bind(a, &c, &format!("rules.countries.{f}"), &p)?;
        }
    }
    for (name, identity, prefix) in [
        ("difficulty", "id", "difficulty"),
        ("prerequisite_groups", "id", "group"),
        ("sides", "id", "side"),
    ] {
        for entry in list(
            field(&input.documents["rules.ron"], name, "rules.ron")?,
            name,
        )? {
            let id = atom(field(entry, identity, name)?, name)?;
            let c = format!("{prefix}.{id}");
            for (f, _) in tuple(entry, name)? {
                if f == identity && name != "difficulty" {
                    continue;
                }
                let p = format!("rules.ron.{name}.{f}");
                a.require(WitnessId::new(&c, &p))?;
                let semantic = if name == "difficulty" {
                    "difficulties"
                } else {
                    name
                };
                bind(a, &c, &format!("rules.{semantic}.{f}"), &p)?;
            }
        }
    }
    // Remaining rules powerup/CrateUnits, skirmish/AI fields are explicitly added by the
    // independent contracts. Raw-leaf manifest records all components; unresolved projected
    // own fields are listed in HANDOFF and MUST be added before declaring final completeness.
    owned::declare(a, input)?;
    Ok(())
}
fn declare_special_extra(
    a: &mut Audit,
    r: &Reference,
    c: &str,
    path: &str,
    section: &str,
    keys: &[&str],
) -> AuditResult<()> {
    // Independently listed v2 specialist physical/native dependency demands. No comparison
    // output is consulted, and no source retrieval records successful coverage.
    let id = a.physical(WitnessId::new(c, path));
    if !a.required.contains(&id) {
        a.require(id.clone())?;
    }
    for key in keys {
        let k = SourceKey::new("rules", section, key);
        if r.rows.contains_key(&k) {
            a.require_dependency(
                id.clone(),
                Atom {
                    key: k,
                    portion: Portion::Whole,
                },
            )?;
        }
    }
    Ok(())
}
fn declare_contracts(a: &mut Audit, input: &Inputs) -> AuditResult<()> {
    for row in &input.mappings {
        contracts::item(a, &input.reference, row, &input.mappings)?;
    }
    for (id, s) in &input.weapons {
        contracts::weapon(a, &input.reference, id, s)?;
    }
    for (id, s) in &input.warheads {
        contracts::warhead(a, &input.reference, id, s)?;
    }
    contracts::globals(a, &input.reference, &input.mappings)?;
    specialist::declare(a, input)
}
fn load_audit(input: &Inputs, d: &Definitions) -> AuditResult<Audit> {
    let mut a = Audit::default();
    for difference in &input.differences {
        a.add_difference(difference.clone())?;
    }
    declare_physical(&mut a, input, d)?;
    declare_contracts(&mut a, input)?;
    // Actual schema10 raw fields exist before these independent source demands. No adapter
    // may create a physical field or silently introduce a source dependency during comparison.
    for row in &input.mappings {
        schema10_adapters::declare(&mut a, &input.reference, row)?;
    }
    fire_audit::declare(&mut a)?;
    Ok(a)
}
fn run_adapters(
    a: &mut Audit,
    input: &Inputs,
    d: &Definitions,
    skirmish: &SkirmishDefinitions,
) -> Vec<String> {
    let r = &input.reference;
    let mut errors = Vec::new();
    let ids = match input.item_ids() {
        Ok(v) => v,
        Err(e) => return vec![e],
    };
    let weapons = index(&input.weapons);
    let warheads = index(&input.warheads);
    let mut collect = |label: String, result: AuditResult<()>| {
        if let Err(e) = result {
            errors.push(format!("{label}: {e}"));
        }
    };
    for row in &input.mappings {
        let c = format!("{}@{}", row.game_id, row.section);
        collect(
            format!("{c} core"),
            actual::core_item(a, r, d, &row.game_id, &row.section, row.building),
        );
        collect(format!("{c} unit"), items::unit_terms(a, r, d, row));
        collect(format!("{c} footprint"), items::footprint(a, r, d, row));
        collect(
            format!("{c} building"),
            items::building_terms(a, r, d, row, &ids),
        );
        collect(
            format!("{c} prerequisites"),
            items::prerequisites(a, r, d, row, &ids),
        );
        collect(format!("{c} flags"), items::metadata_flags(a, r, d, row));
        collect(
            format!("{c} schema10"),
            schema10_adapters::project(a, r, d, row),
        );
        collect(
            format!("{c} combat"),
            items::combat_terms(a, r, d, row, &weapons),
        );
        collect(
            format!("{c} spawns"),
            items::spawned(a, r, d, row, &ids, &weapons),
        );
        collect(format!("{c} Owner"), items::owner_sides(a, r, d, row));
        collect(
            format!("{c} producer"),
            items::factory_and_producer(a, r, d, row, &input.mappings),
        );
        collect(
            format!("{c} docks"),
            items::docks(a, r, d, row, &input.mappings),
        );
        collect(format!("{c} IFV"), rules::ifv(a, r, d, row, &weapons));
        collect(format!("{c} SW"), rules::superweapon_links(a, r, d, row));
        collect(
            format!("{c} MissileSpawn"),
            native::missile_marker(a, r, d, row),
        );
        collect(
            format!("{c} terrain"),
            native::movement_restriction(a, r, d, row, &input.mappings),
        );
        collect(
            format!("{c} death fallback"),
            native::default_death_fallback(a, r, d, row),
        );
    }
    for (id, s) in &input.weapons {
        collect(
            format!("weapon {id}"),
            combat::weapon(a, r, d, id, s, &warheads),
        );
    }
    for (id, s) in &input.warheads {
        collect(format!("warhead {id}"), combat::warhead(a, r, d, id, s));
    }
    collect("scalars".into(), combat::scalars(a, r, d));
    collect("effects".into(), combat::effects(a, r, d, &warheads));
    collect("powerups".into(), combat::powerups(a, r, d));
    collect("difficulty".into(), rules::difficulties(a, r, d));
    collect("countries".into(), rules::countries(a, r, d));
    collect(
        "groups/sides".into(),
        rules::groups_and_sides(a, r, d, &ids),
    );
    collect(
        "lobby".into(),
        rules::lobby(a, r, d, skirmish, &input.mappings),
    );
    collect("Tiberiums".into(), rules::tiberium_registry(a, r, d));
    collect(
        "crate pool".into(),
        rules::crate_units(a, r, d, &input.mappings),
    );
    collect(
        "American drop".into(),
        rules::american_paradrop_infantry(a, r, d, &ids),
    );
    collect(
        "specialist native/own field policies".into(),
        specialist::project(a, input, d),
    );
    collect(
        "typed physical component arrays".into(),
        array_plans::run(input, d),
    );
    collect(
        "remaining typed nested arrays/tuples".into(),
        nested_plans::run(input, d),
    );
    collect(
        "game-owned loaded projections".into(),
        owned::project(a, input, d, skirmish),
    );
    errors
}
// Legacy name retained for the existing test call graph; this loader accepts ONLY the
// new typed format. There is no seven-column/fallback path that bypasses source admission.
#[allow(
    clippy::too_many_arguments,
    reason = "The admission boundary keeps independently produced evidence inputs explicit"
)]
fn load_exact_unused(
    a: &mut Audit,
    input: &Inputs,
    d: &Definitions,
    path: &Path,
    plans: &mut source_plans::Plans,
    route: &mut source_routes::Plan,
    scope: &scope_inputs::ObservedScope,
    fire: &fire_keys::Evidence,
) -> AuditResult<()> {
    policy_admission::require_source_acceptance_census(&source_routes::single_table_keys(route)?)?;
    for classified in source_routes::load_typed(path)? {
        let spec = source_routes::validate_row(route, &classified)?;
        // Exactly one enum dispatch. No sequence of conditional callbacks can bless the same key
        // through two packages; callback success alone does not register source coverage.
        source_plans::admit(plans, a, input, d, &classified.row, spec.route, scope, fire)?;
        a.add_unused(classified.row.clone())?;
        source_routes::record(route, &classified)?;
    }
    source_plans::finish(plans, a)?;
    source_routes::finish(route, a)
}
fn audit_at(data: &Path, references: &Path) -> AuditResult<()> {
    let input = Inputs::load(data, references)?;
    let d = sim::load_ruleset(data).map_err(|e| e.to_string())?;
    let skirmish = sim::load_skirmish(data.join("skirmish.ron"), &d).map_err(|e| e.to_string())?;
    let scope = scope_inputs::ObservedScope::read(references)?;
    let mut a = load_audit(&input, &d)?;
    let mut plans = source_plans::declare(&input, &d, &a, &scope)?;
    // Independent source identities/classification demands are fixed before callbacks.
    let mut route = source_routes::declare(&input, &plans)?;
    let errors = run_adapters(&mut a, &input, &d, &skirmish);
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    // Existing fire-key package is a real native CommandQueue/SimSchedule observation, not a
    // constant flag getter. The two proposed runtime field witnesses are independently declared.
    let decision = scope.return_fire_decision()?;
    let fire = fire_audit::project(&mut a, &input, &d, Some(&decision))?;
    load_exact_unused(
        &mut a,
        &input,
        &d,
        &references.join("ra9_source_dispositions.tsv"),
        &mut plans,
        &mut route,
        &scope,
        &fire,
    )?;
    a.finish(&input.reference)?;
    if atom(
        field(
            &input.documents["arsenal.ron"],
            "schema_version",
            "arsenal.ron",
        )?,
        "schema_version",
    )? != "10"
    {
        return Err("RA9 requires actual coordinated five-field schema10/generator/all fixtures/hash migration".into());
    }
    let compared = a
        .witnesses
        .values()
        .filter(|w| matches!(w, Witness::Compared { .. }))
        .count();
    let source_atoms = a
        .required_dependencies
        .iter()
        .map(|(_, atom)| atom)
        .collect::<BTreeSet<_>>()
        .len();
    println!(
        "RA9_AUDIT_SUCCESS physical={} compared={} game_owned={} demanded_successful_pairs={} compared_source_atoms={} disposition_atoms={} reference_atoms={} used_differences={}",
        a.required.len(),
        compared,
        a.witnesses.len() - compared,
        a.required_dependencies.len(),
        source_atoms,
        a.unused.len(),
        input.reference.expected_atoms().len(),
        a.used_differences.len()
    );
    Ok(())
}
#[test]
fn every_physical_consumer_and_reference_atom_has_an_exact_successful_disposition()
-> AuditResult<()> {
    audit_at(&super::data_dir(), &super::reference_dir())
}

// The physical comparison path is independently testable even while source policy admission
// is deliberately closed. This never installs classifications or weakens the full audit above.
#[test]
fn every_loaded_physical_adapter_reports_exact_failures_before_source_policy_admission()
-> AuditResult<()> {
    let input = Inputs::load(&super::data_dir(), &super::reference_dir())?;
    let d = sim::load_ruleset(&input.data_dir).map_err(|e| e.to_string())?;
    let skirmish =
        sim::load_skirmish(input.data_dir.join("skirmish.ron"), &d).map_err(|e| e.to_string())?;
    let mut audit = load_audit(&input, &d)?;
    let errors = run_adapters(&mut audit, &input, &d, &skirmish);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

// Real consumer-premise observations are separate from policy acceptance and native proof.
// This diagnostic never calls add_unused or supplies a successful source disposition.
#[test]
fn ra9_actual_source_policy_consumer_premises_and_physical_dependencies_match() -> AuditResult<()> {
    let input = Inputs::load(&super::data_dir(), &super::reference_dir())?;
    let d = sim::load_ruleset(&input.data_dir).map_err(|e| e.to_string())?;
    let skirmish =
        sim::load_skirmish(input.data_dir.join("skirmish.ron"), &d).map_err(|e| e.to_string())?;
    let mut audit = load_audit(&input, &d)?;
    let mut errors = Vec::new();
    macro_rules! observe {
        ($id:expr, $call:expr) => {
            match $call {
                Ok(_) => eprintln!(
                    "ACTUAL_STATIC_LOADED_PREMISE {} PASS; no policy/native credit",
                    $id
                ),
                Err(e) => errors.push(format!("{}: {e}", $id)),
            }
        };
    }
    // All independent source/physical demands are declared before adapters execute.
    observe!(
        "consumer62",
        consumer62_admission::declare(&input, &d, &audit)
    );
    observe!("carry38", carry38_admission::declare(&input, &d, &audit));
    observe!("model162", model162_admission::declare(&input, &d, &audit));
    observe!(
        "behavior87",
        behavior87_admission::declare(&input, &d, &audit)
    );
    observe!("legacy-all-mappings", legacy904_support::mappings(&input));
    for r in legacy904_support::records(
        include_str!("legacy904-family-contracts.tsv"),
        "route\tfamily_id\tatom_count\told_identity_value_sha256\tprofile\ttyped_class\tspecific_current_model_reason\tactual_paths\trequired_routes\tdocument_authorities\tadmission_obligation\tsource_sections\tsource_keys\tindependent_atom_fnv1a64",
    )? {
        observe!(
            format!("{}/source", r[1]),
            legacy904_support::original(&input, r[1])
        );
        observe!(
            format!("{}/code", r[1]),
            legacy904_support::current_code(r[7], r[4])
        );
        observe!(
            format!("{}/documents", r[1]),
            legacy904_support::documents(r[9])
        );
        observe!(
            format!("{}/loaded", r[1]),
            legacy904_support::loaded(
                &input,
                &d,
                r[4],
                r[0] == "legacy-ai361" || r[4].starts_with("difficulty-")
            )
        );
        observe!(
            format!("{}/selectors", r[1]),
            legacy904_support::last_five_model_guard(&input, &d, r[1])
        );
    }
    errors.extend(run_adapters(&mut audit, &input, &d, &skirmish));
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

// This export deliberately requires authenticated actual Scope and successful physical adapters.
// Its stdout is a proposal only: it does not run admissions or mark any source unused.
#[test]
fn ra9_export_actual_single_table_proposals() -> AuditResult<()> {
    let references = super::reference_dir();
    let input = Inputs::load(&super::data_dir(), &references)?;
    let d = sim::load_ruleset(super::data_dir()).map_err(|e| e.to_string())?;
    let skirmish = sim::load_skirmish(super::data_dir().join("skirmish.ron"), &d)
        .map_err(|e| e.to_string())?;
    let scope = scope_inputs::ObservedScope::read(&references)?;
    let mut a = load_audit(&input, &d)?;
    let plans = source_plans::declare(&input, &d, &a, &scope)?;
    let route = source_routes::declare(&input, &plans)?;
    let wanted = source_routes::single_table_keys(&route)?;
    let errors = run_adapters(&mut a, &input, &d, &skirmish);
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    let mut emitted = BTreeSet::new();
    println!(
        "RA9_PROPOSAL_HEADER\tfile\tsection\tkey\texact_value\tspecific_reason\tconsumer_bindings\tmodel_guard_id"
    );
    for classified in source_routes::load_typed(&references.join("ra9_source_dispositions.tsv"))? {
        let spec = source_routes::validate_row(&route, &classified)?;
        if !wanted.contains(&classified.row.atom.key) {
            continue;
        }
        let (binding, guard) =
            source_plans::single_proposal_binding(&plans, &classified.row, spec.route, &scope)?;
        if !emitted.insert(classified.row.atom.key.clone()) {
            return Err("duplicate exported source proposal".into());
        }
        let k = &classified.row.atom.key;
        println!(
            "RA9_PROPOSAL\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            k.file,
            k.section,
            k.key,
            classified.row.exact_value,
            classified.row.reason,
            binding,
            guard
        );
    }
    if emitted != wanted {
        return Err("exported proposals differ from independent exact source ownership".into());
    }
    Ok(())
}

// A real loaded Black Eagle producer may be one of multiple independent native-side factories.
// Membership retains the full allowed set; it is not scalar equality or a listed difference.
#[test]
fn ra9_actual_producer_membership_rejects_nonmember_duplicate_and_corrupt_set() -> AuditResult<()> {
    let input = Inputs::load(&super::data_dir(), &super::reference_dir())?;
    let d = sim::load_ruleset(&input.data_dir).map_err(|e| e.to_string())?;
    let sk =
        sim::load_skirmish(input.data_dir.join("skirmish.ron"), &d).map_err(|e| e.to_string())?;
    let mut a = load_audit(&input, &d)?;
    let errors = run_adapters(&mut a, &input, &d, &sk);
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    let id = WitnessId::new("black_eagle@rules:BEAG", "economy.ron.units.producer");
    let witness = a
        .witnesses
        .get(&id)
        .ok_or("real producer witness missing")?;
    a.validate_witness(&id, witness)?;
    let Witness::Compared {
        actual,
        expected,
        difference,
        dependencies,
        relation: ComparisonRelation::MemberOf(allowed),
    } = witness
    else {
        return Err("real producer must retain independent membership relation".into());
    };
    assert!(allowed.len() > 1 && allowed.contains(actual));
    assert_ne!(actual, expected);
    assert!(difference.is_none());
    assert!(!dependencies.is_empty());
    assert_eq!(
        dependencies,
        &a.required_dependencies
            .iter()
            .filter(|(w, _)| w == &id)
            .map(|(_, atom)| atom.clone())
            .collect::<BTreeSet<_>>()
    );
    for mode in 0..4 {
        let mut bad = witness.clone();
        let Witness::Compared {
            actual,
            expected,
            relation: ComparisonRelation::MemberOf(allowed),
            ..
        } = &mut bad
        else {
            unreachable!()
        };
        match mode {
            0 => *actual = "not_a_native_producer".into(),
            1 => {
                allowed.push(allowed[0].clone());
                *expected = allowed.join("|");
            }
            2 => {
                allowed.clear();
                expected.clear();
            }
            3 => *expected = actual.clone(),
            _ => unreachable!(),
        }
        assert!(
            a.validate_witness(&id, &bad).is_err(),
            "corrupt membership mode {mode}"
        );
    }
    Ok(())
}

#[test]
fn ra9_export_actual_completion174_atom_proposals() -> AuditResult<()> {
    let input = Inputs::load(&super::data_dir(), &super::reference_dir())?;
    let d = sim::load_ruleset(&input.data_dir).map_err(|e| e.to_string())?;
    let sk =
        sim::load_skirmish(input.data_dir.join("skirmish.ron"), &d).map_err(|e| e.to_string())?;
    let _actual_scope = scope_inputs::ObservedScope::read(&super::reference_dir())?;
    let mut a = load_audit(&input, &d)?;
    let plan = completion::declare(&input, &d, &a)?;
    let errors = run_adapters(&mut a, &input, &d, &sk);
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    completion::export(&plan, &a)
}
