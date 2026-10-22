//! UNVALIDATED typed physical-array proof ledger. This ledger adds NO global reference
//! coverage. Existing real aggregate comparisons remain mandatory. Independent demand
//! contracts and successful element/length comparisons must have exactly equal edges.
use super::{AuditResult, Reference, SourceKey};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum SourcePart {
    Whole(SourceKey),
    Csv {
        key: SourceKey,
        index: usize,
        exact_len: usize,
    },
}
impl SourcePart {
    pub(super) fn read<'a>(&self, r: &'a Reference) -> AuditResult<&'a str> {
        match self {
            Self::Whole(key) => r
                .rows
                .get(key)
                .map(String::as_str)
                .ok_or_else(|| format!("nonexistent array dependency {key:?}")),
            Self::Csv {
                key,
                index,
                exact_len,
            } => {
                let raw = r
                    .rows
                    .get(key)
                    .ok_or_else(|| format!("nonexistent array tuple {key:?}"))?;
                let parts = raw.split(',').map(str::trim).collect::<Vec<_>>();
                if parts.len() != *exact_len || *index >= *exact_len {
                    return Err(format!(
                        "array tuple shape/index differs {key:?}[{index}]/{exact_len}"
                    ));
                }
                Ok(parts[*index])
            }
        }
    }
}
pub(super) type Deps = BTreeSet<SourcePart>;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Semantics {
    Ordered,
    UniqueSet,
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum Element {
    Index(usize),
    Member(String),
}
#[derive(Clone, Debug)]
pub(super) struct ExpectedElement {
    pub(super) value: String,
    pub(super) deps: Deps,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PhysicalOrigin {
    pub(super) file: String,
    pub(super) list: String,
    pub(super) item: String,
    pub(super) field: String,
    pub(super) index: Option<usize>,
}
#[derive(Clone, Debug)]
struct Contract {
    semantics: Semantics,
    elements: BTreeMap<Element, Deps>,
    length_deps: Deps,
    origins: Option<Vec<PhysicalOrigin>>,
}
#[derive(Clone, Debug)]
struct Proof {
    length: usize,
    physical_positions: Vec<(usize, Element)>,
    elements: BTreeMap<Element, Deps>,
    length_deps: Deps,
    origins: Option<Vec<PhysicalOrigin>>,
}
#[derive(Clone, Debug, Default)]
pub(super) struct Ledger {
    required: BTreeMap<String, Contract>,
    successful: BTreeMap<String, Proof>,
    observed_origins: BTreeMap<String, Vec<PhysicalOrigin>>,
}
impl Ledger {
    pub(super) fn declare(
        &mut self,
        id: String,
        semantics: Semantics,
        demands: Vec<(Element, Deps)>,
        length_deps: Deps,
    ) -> AuditResult<()> {
        let mut elements = BTreeMap::new();
        for (key, deps) in demands {
            if elements.insert(key, deps).is_some() {
                return Err(format!(
                    "duplicate independently demanded array element {id}"
                ));
            }
        }
        if semantics == Semantics::Ordered
            && elements.keys().cloned().collect::<BTreeSet<_>>()
                != (0..elements.len()).map(Element::Index).collect()
        {
            return Err(format!("noncontiguous independent ordered demand {id}"));
        }
        if self.required.contains_key(&id) {
            return Err(format!("duplicate array contract {id}"));
        }
        self.required.insert(
            id,
            Contract {
                semantics,
                elements,
                length_deps,
                origins: None,
            },
        );
        Ok(())
    }
    pub(super) fn declare_origins(
        &mut self,
        id: &str,
        origins: Vec<PhysicalOrigin>,
    ) -> AuditResult<()> {
        let contract = self
            .required
            .get_mut(id)
            .ok_or("physical origin contract missing array")?;
        if contract.origins.is_some() || origins.len() != contract.elements.len() {
            return Err("duplicate/wrong length physical origin demands".into());
        }
        contract.origins = Some(origins);
        Ok(())
    }
    pub(super) fn observe_origins(
        &mut self,
        id: &str,
        origins: Vec<PhysicalOrigin>,
    ) -> AuditResult<()> {
        let expected = self
            .required
            .get(id)
            .and_then(|v| v.origins.as_ref())
            .ok_or("undeclared physical origin observation")?;
        if expected != &origins || self.observed_origins.contains_key(id) {
            return Err(format!("actual physical component origins differ {id}"));
        }
        self.observed_origins.insert(id.into(), origins);
        Ok(())
    }
    pub(super) fn compare(
        &mut self,
        r: &Reference,
        id: &str,
        raw_values: Vec<String>,
        loaded_values: Vec<String>,
        expected: Vec<ExpectedElement>,
        length_deps: Deps,
    ) -> AuditResult<()> {
        let contract = self
            .required
            .get(id)
            .ok_or_else(|| format!("unregistered physical array {id}"))?;
        if self.successful.contains_key(id) {
            return Err(format!("duplicate physical array comparison {id}"));
        }
        let raw_set = raw_values.iter().collect::<BTreeSet<_>>();
        let loaded_set = loaded_values.iter().collect::<BTreeSet<_>>();
        if raw_values.len() != loaded_values.len() {
            return Err(format!("physical raw/loaded array length mismatch {id}"));
        }
        match contract.semantics {
            Semantics::Ordered => {
                if raw_values != loaded_values {
                    return Err(format!("physical raw/loaded ordered array differs {id}"));
                }
            }
            Semantics::UniqueSet => {
                if raw_set.len() != raw_values.len()
                    || loaded_set.len() != loaded_values.len()
                    || raw_set != loaded_set
                {
                    return Err(format!("duplicate or raw/loaded set mismatch {id}"));
                }
            }
        }
        if expected.len() != contract.elements.len() || loaded_values.len() != expected.len() {
            return Err(format!(
                "actual/expected/required array length mismatch {id}: actual{} expected{} required{}",
                loaded_values.len(),
                expected.len(),
                contract.elements.len()
            ));
        }
        let mut values = BTreeMap::new();
        let mut element_deps = BTreeMap::new();
        for (index, entry) in expected.into_iter().enumerate() {
            let key = match contract.semantics {
                Semantics::Ordered => Element::Index(index),
                Semantics::UniqueSet => Element::Member(entry.value.clone()),
            };
            if values.insert(key.clone(), entry.value).is_some() {
                return Err(format!("duplicate expected semantic member {id}"));
            }
            element_deps.insert(key, entry.deps);
        }
        if values.keys().collect::<BTreeSet<_>>()
            != contract.elements.keys().collect::<BTreeSet<_>>()
        {
            return Err(format!(
                "expected element identities differ from independent demands {id}"
            ));
        }
        let mut positions = Vec::new();
        for (index, actual) in raw_values.iter().enumerate() {
            let key = match contract.semantics {
                Semantics::Ordered => Element::Index(index),
                Semantics::UniqueSet => Element::Member(actual.clone()),
            };
            if values.get(&key) != Some(actual) {
                return Err(format!(
                    "actual physical array component differs {id}[{index}]: {actual:?}"
                ));
            }
            positions.push((index, key));
        }
        // Presence and exact component shape are checked before any proof insertion.
        for dep in length_deps
            .iter()
            .chain(element_deps.values().flat_map(|v| v.iter()))
        {
            let _exact_source_value = dep.read(r)?;
        }
        if length_deps != contract.length_deps || element_deps != contract.elements {
            return Err(format!(
                "exact array dependency-edge mismatch {id}: missing/foreign element or length dependencies"
            ));
        }
        let origins = match &contract.origins {
            Some(expected) => {
                let observed = self.observed_origins.get(id).ok_or_else(|| {
                    format!("missing actual physical component origin observations {id}")
                })?;
                if observed != expected {
                    return Err("component physical origin mismatch".into());
                }
                Some(observed.clone())
            }
            None => None,
        };
        self.successful.insert(
            id.into(),
            Proof {
                length: loaded_values.len(),
                physical_positions: positions,
                elements: element_deps,
                length_deps,
                origins,
            },
        );
        Ok(())
    }
    pub(super) fn finish(&self) -> AuditResult<()> {
        if self.required.keys().collect::<BTreeSet<_>>()
            != self.successful.keys().collect::<BTreeSet<_>>()
        {
            return Err("missing/unexpected physical array witnesses".into());
        }
        for (id, contract) in &self.required {
            let proof = &self.successful[id];
            if proof.length != contract.elements.len()
                || proof.physical_positions.len() != proof.length
                || proof.elements != contract.elements
                || proof.length_deps != contract.length_deps
                || proof.origins != contract.origins
            {
                return Err(format!("physical component/length witness changed {id}"));
            }
            if proof
                .physical_positions
                .iter()
                .map(|(_, key)| key)
                .collect::<BTreeSet<_>>()
                != contract.elements.keys().collect()
            {
                return Err(format!("missing/duplicate actual physical component {id}"));
            }
        }
        Ok(())
    }
}
pub(super) fn whole(r: &Reference, s: &str, key: &str) -> Deps {
    let key = SourceKey::new("rules", s, key);
    if r.rows.contains_key(&key) {
        BTreeSet::from([SourcePart::Whole(key)])
    } else {
        Deps::new()
    }
}
pub(super) fn csv(s: &str, key: &str, index: usize, exact_len: usize) -> Deps {
    BTreeSet::from([SourcePart::Csv {
        key: SourceKey::new("rules", s, key),
        index,
        exact_len,
    }])
}
pub(super) fn merge(mut a: Deps, b: Deps) -> Deps {
    a.extend(b);
    a
}
pub(super) fn values(r: &Reference, s: &str, key: &str) -> AuditResult<Vec<String>> {
    let Some(raw) = r.rows.get(&SourceKey::new("rules", s, key)) else {
        return Ok(Vec::new());
    };
    if raw.is_empty() {
        return Ok(Vec::new());
    };
    let parts = raw
        .split(',')
        .map(str::trim)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if parts.iter().any(String::is_empty) {
        return Err("empty interior native CSV component".into());
    }
    Ok(parts)
}
