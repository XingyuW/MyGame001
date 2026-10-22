//! UNVALIDATED raw shipped schema/completeness draft.
//! This walks every actual entry; it does not infer all instances from the first entry.
//! Parse only the generated data subset, fail closed on unexpected syntax. Loaded sim data
//! remains the source of actual gameplay values used in comparisons.
use super::{Audit, AuditResult, WitnessId};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub(super) enum Raw {
    Atom(String),
    Tuple(Vec<(String, Raw)>),
    List(Vec<Raw>),
}
pub(super) struct Reader {
    chars: Vec<char>,
    at: usize,
}
impl Reader {
    pub(super) fn new(text: &str) -> Self {
        Self {
            chars: text.chars().collect(),
            at: 0,
        }
    }
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }
    fn skip(&mut self) -> AuditResult<()> {
        loop {
            while self.peek().is_some_and(char::is_whitespace) {
                self.at += 1;
            }
            if self.peek() == Some('/') && self.chars.get(self.at + 1) == Some(&'/') {
                while self.peek().is_some_and(|c| c != '\n') {
                    self.at += 1;
                }
            } else {
                break;
            }
        }
        Ok(())
    }
    fn token(&mut self, expected: char) -> AuditResult<()> {
        self.skip()?;
        if self.peek() != Some(expected) {
            return Err(format!("raw schema offset{} expected{expected:?}", self.at));
        }
        self.at += 1;
        Ok(())
    }
    fn name(&mut self) -> AuditResult<String> {
        self.skip()?;
        let begin = self.at;
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            self.at += 1;
        }
        if begin == self.at {
            return Err(format!("raw field name missing at{}", self.at));
        }
        Ok(self.chars[begin..self.at].iter().collect())
    }
    fn quoted(&mut self) -> AuditResult<String> {
        self.token('"')?;
        let mut value = String::new();
        loop {
            match self.peek() {
                None => return Err("unterminated raw string".into()),
                Some('"') => {
                    self.at += 1;
                    return Ok(value);
                }
                Some('\\') => {
                    self.at += 1;
                    let c = self
                        .peek()
                        .ok_or_else(|| "unterminated raw escape".to_owned())?;
                    self.at += 1;
                    value.push(match c {
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        '"' => '"',
                        '\\' => '\\',
                        _ => return Err(format!("unexpected raw string escape{c:?}")),
                    });
                }
                Some(c) => {
                    value.push(c);
                    self.at += 1;
                }
            }
        }
    }
    fn value(&mut self) -> AuditResult<Raw> {
        self.skip()?;
        match self.peek() {
            Some('(') => {
                self.at += 1;
                let mut fields = Vec::new();
                let mut names = BTreeSet::new();
                loop {
                    self.skip()?;
                    if self.peek() == Some(')') {
                        self.at += 1;
                        break;
                    }
                    let name = self.name()?;
                    if !names.insert(name.clone()) {
                        return Err(format!("duplicate raw field{name}"));
                    }
                    self.token(':')?;
                    fields.push((name, self.value()?));
                    self.skip()?;
                    if self.peek() == Some(',') {
                        self.at += 1;
                    } else if self.peek() != Some(')') {
                        return Err("raw tuple missing comma".into());
                    }
                }
                Ok(Raw::Tuple(fields))
            }
            Some('[') => {
                self.at += 1;
                let mut values = Vec::new();
                loop {
                    self.skip()?;
                    if self.peek() == Some(']') {
                        self.at += 1;
                        break;
                    }
                    values.push(self.value()?);
                    self.skip()?;
                    if self.peek() == Some(',') {
                        self.at += 1;
                    } else if self.peek() != Some(']') {
                        return Err("raw list missing comma".into());
                    }
                }
                Ok(Raw::List(values))
            }
            Some('"') => Ok(Raw::Atom(self.quoted()?)),
            Some(_) => {
                let begin = self.at;
                while self
                    .peek()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '+'))
                {
                    self.at += 1;
                }
                if begin == self.at {
                    return Err(format!("unexpected raw value at{}", self.at));
                }
                Ok(Raw::Atom(self.chars[begin..self.at].iter().collect()))
            }
            None => Err("missing raw value".into()),
        }
    }
    pub(super) fn document(mut self) -> AuditResult<Raw> {
        let value = self.value()?;
        self.skip()?;
        if self.at != self.chars.len() {
            return Err(format!("trailing raw schema text at{}", self.at));
        }
        Ok(value)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum FieldPolicy {
    Identity,
    Compared,
}
pub(super) struct EntrySpec {
    pub(super) list: String,
    pub(super) identity: String,
    pub(super) fields: BTreeMap<String, FieldPolicy>,
}
pub(super) fn tuple<'a>(node: &'a Raw, path: &str) -> AuditResult<&'a [(String, Raw)]> {
    match node {
        Raw::Tuple(v) => Ok(v),
        _ => Err(format!("{path}: expected raw tuple")),
    }
}
pub(super) fn field<'a>(node: &'a Raw, name: &str, path: &str) -> AuditResult<&'a Raw> {
    tuple(node, path)?
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v)
        .ok_or_else(|| format!("{path}: missing field{name}"))
}
pub(super) fn atom<'a>(node: &'a Raw, path: &str) -> AuditResult<&'a str> {
    match node {
        Raw::Atom(v) => Ok(v),
        _ => Err(format!("{path}: expected raw atom")),
    }
}
pub(super) fn list<'a>(node: &'a Raw, path: &str) -> AuditResult<&'a [Raw]> {
    match node {
        Raw::List(v) => Ok(v),
        _ => Err(format!("{path}: expected raw list")),
    }
}

// Mapped consumers supplied independently from the whole mapping table. One game ID can
// return ENGINEER and SENGINEER. Combat IDs similarly include kind to avoid collisions.
// Every raw item ID must have a consumer; every declared consumer must resolve to actual data.
pub(super) fn enumerate_actual_entries(
    audit: &mut Audit,
    file: &str,
    document: &Raw,
    spec: &EntrySpec,
    consumers: &BTreeMap<String, Vec<String>>,
) -> AuditResult<BTreeSet<String>> {
    let entries = list(
        field(document, &spec.list, file)?,
        &format!("{file}.{}", spec.list),
    )?;
    let mut ids = BTreeSet::new();
    for (index, entry) in entries.iter().enumerate() {
        let context = format!("{file}.{}[{index}]", spec.list);
        let id = atom(field(entry, &spec.identity, &context)?, &context)?.to_owned();
        if !ids.insert(id.clone()) {
            return Err(format!("duplicate actual entry {context}/{id}"));
        }
        let named = consumers
            .get(&id)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| format!("unmapped actual item {context}/{id}"))?;
        let actualfields = tuple(entry, &context)?
            .iter()
            .map(|(k, _)| k.clone())
            .collect::<BTreeSet<_>>();
        let policyfields = spec.fields.keys().cloned().collect::<BTreeSet<_>>();
        if actualfields != policyfields {
            return Err(format!(
                "{context}: unaudited/stale raw fields missing {:?}; extra {:?}",
                policyfields.difference(&actualfields).collect::<Vec<_>>(),
                actualfields.difference(&policyfields).collect::<Vec<_>>()
            ));
        }
        for consumer in named {
            for (name, policy) in &spec.fields {
                if *policy == FieldPolicy::Identity {
                    continue;
                }
                // This registration occurs before adapters. Omitting one item or one mapped
                // twin's successful comparison cannot be covered by another field's success.
                audit.require(WitnessId::new(
                    consumer,
                    &format!("{file}.{}.{name}", spec.list),
                ))?;
            }
        }
    }
    Ok(ids)
}

// Top-level keys and list shapes require separate exhaustive policy, not only item entries.
// Rules nested arrays require exact object/list-field and per-slot schemas as well.
// A new raw field cannot become implicitly Own: policy tables are deliberately explicit.
pub(super) fn require_top_level(document: &Raw, expected: &[&str], file: &str) -> AuditResult<()> {
    let actual = tuple(document, file)?
        .iter()
        .map(|(k, _)| k.as_str())
        .collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        Err(format!(
            "{file}: raw top-level schema differs {actual:?} vs {expected:?}"
        ))
    } else {
        Ok(())
    }
}

// Required integration negatives (not executed):
// - actual second entry gains a field absent from the first: must fail;
// - one actual entry loses a field: must fail;
// - mapping row/twin missing: must fail either actual unmapped or missing required witness;
// - new loaded field/list/top-level group: no generic fallback, must fail;
// - truncate versus/IFV/difficulty arrays: exact length/order guards fail;
// - actual loaded value mutation: fails comparator, not merely schema scanner;
// - map art mutation: actual footprint comparator/explicit difference fails;
// - no whole-table reconciliation can be skipped by a named wrapper's foreign-row marking.

pub(super) fn exact_fields(node: &Raw, names: &[&str], path: &str) -> AuditResult<()> {
    let actual = tuple(node, path)?
        .iter()
        .map(|(k, _)| k.as_str())
        .collect::<BTreeSet<_>>();
    let expected = names.iter().copied().collect::<BTreeSet<_>>();
    if expected.len() != names.len() || actual != expected {
        return Err(format!(
            "{path}: exact nested schema {actual:?}!={expected:?}"
        ));
    }
    Ok(())
}
pub(super) fn record_list<'a>(
    document: &'a Raw,
    name: &str,
    identity: &str,
    names: &[&str],
    file: &str,
) -> AuditResult<Vec<(&'a str, &'a Raw)>> {
    let entries = list(field(document, name, file)?, name)?;
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let path = format!("{file}.{name}[{index}]");
        exact_fields(entry, names, &path)?;
        let id = atom(field(entry, identity, &path)?, &path)?;
        if !seen.insert(id) {
            return Err(format!("duplicate nested identity {path}/{id}"));
        }
        result.push((id, entry));
    }
    Ok(result)
}
pub(super) fn rules_nested_schema(document: &Raw) -> AuditResult<()> {
    require_top_level(
        document,
        &[
            "schema_version",
            "production",
            "economy",
            "veterancy",
            "crew",
            "combat",
            "radiation",
            "superweapons",
            "prism",
            "crates",
            "dialog",
            "powerups",
            "crate_units",
            "difficulty",
            "prerequisite_groups",
            "sides",
            "countries",
        ],
        "rules.ron",
    )?;
    let _ = record_list(
        document,
        "powerups",
        "id",
        &["id", "share", "value"],
        "rules.ron",
    )?;
    let diff = record_list(
        document,
        "difficulty",
        "id",
        &[
            "id",
            "rof_pm",
            "build_time_pm",
            "credits_bonus_percent",
            "virtual_purifiers",
            "team_delay",
            "allied_base_defences",
            "soviet_base_defences",
            "target_weights",
        ],
        "rules.ron",
    )?;
    for (id, entry) in diff {
        if list(field(entry, "target_weights", id)?, id)?.len() != 12 {
            return Err(format!("{id}: exact12 target weights"));
        }
    }
    let _ = record_list(
        document,
        "prerequisite_groups",
        "id",
        &["id", "buildings"],
        "rules.ron",
    )?;
    let _ = record_list(
        document,
        "sides",
        "id",
        &[
            "id",
            "base_unit",
            "construction_yard",
            "crew",
            "survivor_divisor",
            "disguise",
            "paradrop_infantry",
            "paradrop_count",
        ],
        "rules.ron",
    )?;
    let _ = record_list(document, "countries", "id", &["id", "side"], "rules.ron")?;
    // Scalar-group exact field sets come from independently fixed SCALAR_ROWS + three own
    // rows, not from Rules::fields()/generator output (which would accept a new field twice).
    for group in [
        "production",
        "economy",
        "veterancy",
        "crew",
        "combat",
        "radiation",
        "superweapons",
        "prism",
        "crates",
        "dialog",
    ] {
        let mut names = super::combat::SCALAR_ROWS
            .iter()
            .filter(|r| r.0 == group)
            .map(|r| r.1)
            .collect::<Vec<_>>();
        match group {
            "economy" => names.extend(["ore_max_bails", "ore_spread_above"]),
            "superweapons" => names.push("nuclear_flight"),
            _ => {}
        }
        exact_fields(
            field(document, group, "rules.ron")?,
            &names,
            &format!("rules.ron.{group}"),
        )?;
    }
    Ok(())
}
pub(super) fn skirmish_nested_schema(document: &Raw) -> AuditResult<()> {
    require_top_level(
        document,
        &[
            "schema_version",
            "starting_credits",
            "sides",
            "factions",
            "maps",
        ],
        "skirmish.ron",
    )?;
    let _ = record_list(
        document,
        "sides",
        "side",
        &["side", "starting_vehicles", "starting_infantry"],
        "skirmish.ron",
    )?;
    let factions = record_list(
        document,
        "factions",
        "id",
        &[
            "id",
            "label",
            "side",
            "roster_buildings",
            "roster_units",
            "buildings",
            "units",
        ],
        "skirmish.ron",
    )?;
    for (id, entry) in factions {
        for name in ["roster_buildings", "roster_units", "buildings", "units"] {
            if !list(field(entry, name, id)?, id)?.is_empty() {
                return Err(format!(
                    "shipped country {id} has nonempty game-owned legacy {name}; explicit new audit required"
                ));
            }
        }
    }
    let maps = record_list(document, "maps", "id", &["id", "starts"], "skirmish.ron")?;
    for (id, entry) in maps {
        for (index, start) in list(field(entry, "starts", id)?, id)?.iter().enumerate() {
            exact_fields(start, &["x", "y"], &format!("map.{id}.starts[{index}]"))?;
        }
    }
    Ok(())
}
