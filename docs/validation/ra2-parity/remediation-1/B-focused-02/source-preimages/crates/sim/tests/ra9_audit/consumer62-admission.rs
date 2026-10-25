//! Historical preparation note: UNCOMPILED/UNRUN exact62 current-model source admissions. No production/schema changes.
//! This module NEVER creates a WitnessId for a nonread source flag, never records coverage,
//! and never obtains Native proof from a constant getter. Actual typed metadata premises are
//! policy checks. Physical comparisons retain independent v1-v3 demands and adapters.
use super::*;
const CONTRACTS: &str = include_str!("consumer62-contracts.tsv");
const GATES: &str = include_str!("consumer62-loaded-gates.tsv");
const MAPPING: &str = include_str!("consumer62-mapping-guards.tsv");
const DEMANDS: &str = include_str!("consumer62-physical-demands.tsv");
const ACCEPT: &str = include_str!("consumer62-acceptance.tsv");
const GRAPH: &str = include_str!("consumer62-helper-graph.tsv");
#[derive(Clone)]
pub(super) struct Contract {
    key: SourceKey,
    exact: &'static str,
    group: &'static str,
    reason: &'static str,
    bindings: &'static str,
    guard: &'static str,
    routes: &'static str,
}
pub(super) struct Plan {
    contracts: BTreeMap<SourceKey, Contract>,
    physical: BTreeMap<String, BTreeSet<WitnessId>>,
    pad_dependencies: BTreeMap<WitnessId, BTreeSet<Atom>>,
    successful: BTreeSet<SourceKey>,
}
fn rows(text: &str, n: usize) -> AuditResult<Vec<Vec<&str>>> {
    text.lines()
        .skip(1)
        .map(|line| {
            let p = line.split('\t').collect::<Vec<_>>();
            if p.len() != n {
                Err("consumer62 row width".into())
            } else {
                Ok(p)
            }
        })
        .collect()
}
fn groups() -> BTreeMap<&'static str, usize> {
    [
        ("selectable", 6),
        ("disguise", 3),
        ("assault", 2),
        ("docks-reload", 2),
        ("spawned", 5),
        ("power-toggle", 9),
        ("waterbound", 2),
        ("strategic-actions", 14),
        ("nuke-selectors", 4),
        ("ore-switches", 3),
        ("power-selectors", 3),
        ("specialist-selectors", 5),
        ("invisible-projectile", 3),
        ("fog-default", 1),
    ]
    .into_iter()
    .collect()
}
fn contracts() -> AuditResult<BTreeMap<SourceKey, Contract>> {
    let mut out = BTreeMap::new();
    let mut counts = BTreeMap::new();
    for r in rows(CONTRACTS, 12)? {
        let key = SourceKey::new(r[0], r[1], r[2]);
        let c = Contract {
            key: key.clone(),
            exact: r[3],
            group: r[4],
            reason: r[8],
            bindings: r[9],
            guard: r[10],
            routes: r[11],
        };
        if c.reason.is_empty()
            || c.reason.starts_with("PENDING")
            || out.insert(key, c.clone()).is_some()
        {
            return Err("consumer62 duplicate/unspecific contract".into());
        }
        *counts.entry(c.group).or_insert(0usize) += 1;
    }
    if counts != groups() || out.len() != 62 {
        return Err("consumer62 exact independent family census".into());
    }
    Ok(out)
}
fn exact(input: &Inputs, c: &Contract) -> AuditResult<()> {
    if input.reference.rows.get(&c.key).map(String::as_str) != Some(c.exact) {
        return Err(format!("consumer62 changed exact source atom {:?}", c.key));
    }
    Ok(())
}
fn keyset() -> AuditResult<BTreeSet<SourceKey>> {
    Ok(contracts()?.into_keys().collect())
}
fn forbid_credit(a: &Audit) -> AuditResult<()> {
    let keys = keyset()?;
    if a.required_dependencies.iter().any(|(_,atom)|keys.contains(&atom.key))||a.witnesses.values().any(|w|matches!(w,Witness::Compared{dependencies,..}if dependencies.iter().any(|v|keys.contains(&v.key)))){return Err("consumer62 nonread literal falsely demanded/Compared; reading is not coverage".into());}
    Ok(())
}
fn mapping(input: &Inputs) -> AuditResult<()> {
    let expected = rows(MAPPING, 5)?
        .into_iter()
        .map(|r| {
            (
                r[0].to_owned(),
                r[1].to_owned(),
                r[2] == "building",
                r[3].to_owned(),
                r[4].to_owned(),
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
        return Err("consumer62 actual mapping identity/alias/kind changed".into());
    }
    Ok(())
}
fn helper_graph(input: &Inputs) -> AuditResult<()> {
    for r in rows(GRAPH, 4)? {
        if input
            .reference
            .rows
            .get(&SourceKey::new(r[0], r[1], r[2]))
            .map(String::as_str)
            != Some(r[3])
        {
            return Err("consumer62 omitted helper source graph changed".into());
        }
    }
    let helpers = ["CometFragment", "TeslaFragment", "SuperCometFragment"];
    let projectiles = ["SmallCometP", "SmallTeslaP", "SuperSmallCometP"];
    if input
        .weapons
        .iter()
        .any(|(_, s)| helpers.contains(&s.as_str()))
    {
        return Err("consumer62 fragment acquired an actual mapped weapon".into());
    }
    for (_, s) in &input.weapons {
        if input
            .reference
            .rows
            .get(&SourceKey::new("rules", s, "Projectile"))
            .is_some_and(|p| projectiles.contains(&p.as_str()))
        {
            return Err("consumer62 invisible helper acquired a direct physical consumer".into());
        }
    }
    Ok(())
}
fn entry<'a>(d: &'a Definitions, id: &str) -> AuditResult<&'a sim::ArsenalEntry> {
    let item = if let Some(k) = d.unit_kind(id) {
        sim::ProductionItem::Unit(k)
    } else {
        sim::ProductionItem::Building(
            d.building_kind(id)
                .ok_or("consumer62 loaded item missing")?,
        )
    };
    d.metadata(item)
        .ok_or_else(|| "consumer62 loaded metadata missing".into())
}
// These observations are genuine loaded values. Equality below checks chosen policy premises;
// it does not attach the nonread source flag to these values' independent native dependencies.
fn observe(d: &Definitions, id: &str, field: &str) -> AuditResult<String> {
    if id == "match_options" {
        let o = sim::MatchOptions::default();
        return match field {
            "fog_of_war" => Ok(o.fog_of_war.to_string()),
            "ore_grows" => Ok(o.ore_grows.to_string()),
            _ => Err("unknown consumer62 option fact".into()),
        };
    }
    if id == "rules" {
        return match field {
            "dialog.fog_of_war" => Ok(d
                .rules()
                .ok_or("consumer62 rules absent")?
                .dialog
                .fog_of_war
                .to_string()),
            "economy.reload_interval" => Ok(d
                .rules()
                .ok_or("consumer62 rules absent")?
                .economy
                .reload_interval
                .to_string()),
            "economy.repair_step" => Ok(d
                .rules()
                .ok_or("consumer62 rules absent")?
                .economy
                .repair_step
                .to_string()),
            "economy.repair_percent" => Ok(d
                .rules()
                .ok_or("consumer62 rules absent")?
                .economy
                .repair_percent
                .to_string()),
            _ => Err("unknown consumer62 rule fact".into()),
        };
    }
    if let Some(f) = field.strip_prefix("weapon.") {
        let w = d
            .weapon(d.weapon_kind(id).ok_or("consumer62 weapon absent")?)
            .ok_or("consumer62 loaded weapon absent")?;
        return match f {
            "damage" => Ok(w.damage.to_string()),
            "rad_level" => Ok(w.rad_level.to_string()),
            "warhead" => Ok(d
                .warhead(w.warhead)
                .ok_or("consumer62 warhead absent")?
                .id
                .clone()),
            _ => Err("unknown consumer62 weapon fact".into()),
        };
    }
    if field == "exists" {
        return if d.unit_kind(id).is_some() {
            Ok("unit".into())
        } else if d.building_kind(id).is_some() {
            Ok("building".into())
        } else {
            Err("consumer62 missing physical item".into())
        };
    }
    if field == "production.cost" {
        let k = d.unit_kind(id).ok_or("consumer62 priced aircraft absent")?;
        return d
            .production(sim::ProductionItem::Unit(k))
            .map(|p| p.cost.to_string())
            .ok_or_else(|| "consumer62 actual production absent".into());
    }
    if field == "combat.hit_points" {
        return d
            .unit_kind(id)
            .and_then(|k| d.unit_combat(k))
            .map(|c| c.hit_points.to_string())
            .ok_or_else(|| "consumer62 actual aircraft health absent".into());
    }
    if field == "power_consumed" {
        return d
            .building_kind(id)
            .and_then(|k| d.building(k))
            .map(|b| b.power_consumed.to_string())
            .ok_or_else(|| "consumer62 actual pad power terms absent".into());
    }
    if field == "harvester" {
        return Ok(d
            .unit_kind(id)
            .and_then(|k| d.harvester(k))
            .is_some()
            .to_string());
    }
    let m = entry(d, id)?;
    match field {
        "role" => Ok(format!("{:?}", m.role)),
        "factory" => m
            .factory
            .map(|f| format!("{f:?}"))
            .ok_or_else(|| "consumer62 factory missing".into()),
        "disguise_when_still" => Ok(m.disguise_when_still.to_string()),
        "occupier" => Ok(m.occupier.to_string()),
        "c4" => Ok(m.c4.to_string()),
        "docks" => Ok(m.docks.to_string()),
        "powered" => Ok(m.powered.to_string()),
        "airport_bound" => Ok(m.airport_bound.to_string()),
        "ammo" => Ok(m.ammo.to_string()),
        "strategic" => Ok(m.strategic.to_string()),
        _ => Err("unknown consumer62 typed loaded fact".into()),
    }
}
fn role_members(d: &Definitions, wanted: sim::Role) -> AuditResult<BTreeSet<String>> {
    d.arsenal()
        .0
        .iter()
        .filter(|(_, m)| m.role == wanted)
        .map(|(item, _)| {
            match item {
                sim::ProductionItem::Unit(k) => d.unit(*k).map(|v| v.id.clone()),
                sim::ProductionItem::Building(k) => d.building(*k).map(|v| v.id.clone()),
            }
            .ok_or_else(|| "consumer62 extra invalid role kind".into())
        })
        .collect()
}
fn exact_roles(d: &Definitions) -> AuditResult<()> {
    for (role, ids) in [
        (sim::Role::Spy, vec!["spy"]),
        (sim::Role::Mirage, vec!["mirage_tank"]),
        (sim::Role::Engineer, vec!["engineer"]),
        (
            sim::Role::Repair,
            vec!["service_depot", "soviet_service_depot"],
        ),
        (
            sim::Role::Shipyard,
            vec!["naval_shipyard", "soviet_naval_shipyard"],
        ),
    ] {
        if role_members(d, role)? != ids.into_iter().map(str::to_owned).collect() {
            return Err(format!(
                "consumer62 fixed {role:?} dispatch membership changed"
            ));
        }
    }
    Ok(())
}

// Exact current runtime branch drift check, not a native execution/UnitReload witness.
// Whitespace-normalized FNV binds the compiled source body; root still authenticates final SHA
// and actual named native logs before any individual source acceptance.
fn pad_runtime_policy() -> AuditResult<()> {
    fn checked(text: &str, start: &str, expected: u64) -> AuditResult<()> {
        let begin = text
            .find(start)
            .ok_or("consumer62 paid pad runtime start absent")?;
        let end = begin
            + text[begin..]
                .find("\n}\n")
                .ok_or("consumer62 paid pad runtime end absent")?
            + 3;
        let body = text[begin..end]
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>();
        let mut hash = sim::Fnv1a64::new();
        hash.write(body.as_bytes());
        if hash.finish() != expected {
            return Err("consumer62 paid-health-after-ammo implementation changed; refresh exact policy and real native receipts, not an ammo-only waiver".into());
        }
        Ok(())
    }
    checked(
        include_str!("../../src/flight.rs"),
        "fn docks(",
        // fmt49 adds only two equivalent return-None semicolons in let-else blocks.
        // Named source migration B-pad-fmt49-pin-20261021-v1; future body drift still fails.
        0xcf7463c09df7be8c,
    )?;
    checked(
        include_str!("../../src/economy.rs"),
        "pub fn repair_step_cost(",
        0x68f52add6a541aec,
    )
}
fn pad_loaded_policy(d: &Definitions) -> AuditResult<()> {
    pad_runtime_policy()?;
    let mut pads = BTreeSet::new();
    let mut bound = BTreeSet::new();
    for (item, m) in &d.arsenal().0 {
        if m.docks > 0 {
            let sim::ProductionItem::Building(k) = *item else {
                return Err("consumer62 non-building pad registry changed".into());
            };
            pads.insert(
                d.building(k)
                    .ok_or("consumer62 pad kind absent")?
                    .id
                    .clone(),
            );
        }
        if m.airport_bound {
            let sim::ProductionItem::Unit(k) = *item else {
                return Err("consumer62 non-unit airport registry changed".into());
            };
            bound.insert(
                d.unit(k)
                    .ok_or("consumer62 airport kind absent")?
                    .id
                    .clone(),
            );
        }
    }
    if pads
        != ["airforce_command", "american_airforce_command"]
            .into_iter()
            .map(str::to_owned)
            .collect()
        || bound
            != ["black_eagle", "harrier"]
                .into_iter()
                .map(str::to_owned)
                .collect()
    {
        return Err("consumer62 exact shipped pad/airport-bound registry changed; no generic UnitReload=false/other-type claim".into());
    }
    let rules = &d.rules().ok_or("consumer62 pad rules absent")?.economy;
    for (id, cost) in [("harrier", 9), ("black_eagle", 7)] {
        let k = d.unit_kind(id).ok_or("consumer62 real aircraft absent")?;
        let price = d
            .production(sim::ProductionItem::Unit(k))
            .ok_or("consumer62 aircraft price absent")?
            .cost;
        let max = d
            .unit_combat(k)
            .ok_or("consumer62 aircraft health absent")?
            .hit_points;
        if sim::repair_step_cost(rules, price, max, false) != cost
            || sim::repair_step_cost(rules, price, max, true) != 1
        {
            return Err("consumer62 actual paid step formula/stock values changed".into());
        }
    }
    Ok(())
}
fn pad_scope(c: &Contract) -> AuditResult<()> {
    if c.group == "docks-reload"
        && (c.key.file != "rules"
            || c.key.key != "UnitReload"
            || !matches!(c.key.section.as_str(), "GAAIRC" | "AMRADR")
            || c.exact != "yes")
    {
        return Err("consumer62 UnitReload scope is exactly two shipped true literals; no false/general feature admission".into());
    }
    Ok(())
}
fn pad_compared(p: &Plan, a: &Audit) -> AuditResult<()> {
    for (w, demanded) in &p.pad_dependencies {
        let current = a
            .required_dependencies
            .iter()
            .filter(|(id, _)| id == w)
            .map(|(_, atom)| atom.clone())
            .collect::<BTreeSet<_>>();
        if &current != demanded {
            return Err(
                "consumer62 independently declared paid-pad dependencies changed after declaration"
                    .into(),
            );
        }
        compared(a, w)?;
    }
    Ok(())
}

pub(super) fn loaded_group(d: &Definitions, group: &str) -> AuditResult<String> {
    if group == "docks-reload" {
        pad_loaded_policy(d)?;
    }
    let mut facts = Vec::new();
    for r in rows(GATES, 4)?.into_iter().filter(|r| r[0] == group) {
        let actual = observe(d, r[1], r[2])?;
        if actual != r[3] {
            return Err(format!(
                "consumer62 current loaded policy changed {}.{} actual {actual} expected {}",
                r[1], r[2], r[3]
            ));
        }
        facts.push(format!("{}.{}={actual}", r[1], r[2]));
    }
    Ok(if facts.is_empty() {
        "no-direct-loaded-consumer;exact-unmapped-helper-graph".into()
    } else {
        facts.join(";")
    })
}
pub(super) fn declare(input: &Inputs, d: &Definitions, a: &Audit) -> AuditResult<Plan> {
    let contracts = contracts()?;
    for c in contracts.values() {
        exact(input, c)?;
        pad_scope(c)?;
    }
    mapping(input)?;
    helper_graph(input)?;
    exact_roles(d)?;
    for g in groups().keys() {
        loaded_group(d, g)?;
    }
    forbid_credit(a)?;
    let mut physical: BTreeMap<String, BTreeSet<WitnessId>> = BTreeMap::new();
    for r in rows(DEMANDS, 3)? {
        if !groups().contains_key(r[0]) {
            return Err("consumer62 foreign physical demand family".into());
        }
        let w = a.physical(WitnessId::new(r[1], r[2]));
        if !a.required.contains(&w) {
            return Err(format!(
                "consumer62 physical demand absent from raw/native declaration {w:?}"
            ));
        }
        if !physical.entry(r[0].into()).or_default().insert(w) {
            return Err("consumer62 duplicate physical route demand".into());
        }
    }
    let pad_dependencies = physical
        .get("docks-reload")
        .ok_or("consumer62 no independent paid-pad demands")?
        .iter()
        .map(|w| {
            (
                w.clone(),
                a.required_dependencies
                    .iter()
                    .filter(|(id, _)| id == w)
                    .map(|(_, atom)| atom.clone())
                    .collect(),
            )
        })
        .collect();
    Ok(Plan {
        contracts,
        physical,
        pad_dependencies,
        successful: BTreeSet::new(),
    })
}
fn compared(a: &Audit, w: &WitnessId) -> AuditResult<()> {
    match a.witnesses.get(w) {
        Some(Witness::Compared { dependencies, .. }) => {
            let demanded = a
                .required_dependencies
                .iter()
                .filter(|(id, _)| id == w)
                .map(|(_, atom)| atom.clone())
                .collect::<BTreeSet<_>>();
            if &demanded != dependencies {
                return Err(format!(
                    "consumer62 genuine physical demand/compare dependencies differ {w:?}"
                ));
            }
            Ok(())
        }
        _ => Err(format!(
            "consumer62 genuine physical consumer not Compared {w:?}"
        )),
    }
}
fn acceptance_from<'a>(
    text: &'a str,
    key: &SourceKey,
) -> AuditResult<super::policy_admission::SourceAcceptance<'a>> {
    let records = rows(text, 10)?;
    let mut seen = BTreeSet::new();
    let mut result = None;
    for r in records {
        let k = SourceKey::new(r[0], r[1], r[2]);
        if !seen.insert(k.clone()) {
            return Err("consumer62 duplicate acceptance atom".into());
        }
        if &k == key {
            result = Some(super::policy_admission::SourceAcceptance {
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
    if seen != keyset()? {
        return Err("consumer62 missing/extra individual acceptance keys".into());
    }
    result.ok_or_else(|| "consumer62 missing individual source acceptance".into())
}
fn acceptance(key: &SourceKey) -> AuditResult<super::policy_admission::SourceAcceptance<'static>> {
    acceptance_from(ACCEPT, key)
}
fn receipts(c: &Contract, text: &str) -> AuditResult<()> {
    let required = c.routes.split(';').map(str::trim).collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    for token in text.split(';') {
        let (route, hash) = token
            .trim()
            .split_once('@')
            .ok_or("consumer62 native receipt must name route@actual-log-sha256")?;
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) || !actual.insert(route)
        {
            return Err("consumer62 malformed/duplicate actual native receipt".into());
        }
    }
    if required != actual {
        return Err("consumer62 missing/foreign native route receipt".into());
    }
    Ok(())
}
pub(super) fn admit(
    plan: &mut Plan,
    a: &Audit,
    input: &Inputs,
    d: &Definitions,
    row: &Unused,
    final_binding: &str,
) -> AuditResult<()> {
    let c = plan
        .contracts
        .get(&row.atom.key)
        .ok_or("outside exact62 source classifications")?;
    exact(input, c)?;
    mapping(input)?;
    helper_graph(input)?;
    exact_roles(d)?;
    forbid_credit(a)?;
    if let Some(fields) = plan.physical.get(c.group) {
        for w in fields {
            compared(a, w)?;
        }
    }
    if c.group == "docks-reload" {
        pad_scope(c)?;
        pad_compared(plan, a)?;
    }
    let live = loaded_group(d, c.group)?;
    let accepted = acceptance(&c.key)?;
    let p = super::policy_admission::SourceProposal {
        key: c.key.clone(),
        exact: c.exact,
        reason: c.reason,
        consumer_bindings: c.bindings,
        model_guard_id: c.guard,
    };
    super::policy_admission::source_reason(input, row, &p, Some(&accepted), &live, final_binding)?;
    receipts(c, accepted.native_receipts)?;
    // Caller performs add_unused ONLY after this succeeds. This is no original-engine unused
    // assertion: current-model nonread/explicit behavioral difference is stated in each reason.
    if !plan.successful.insert(c.key.clone()) {
        return Err("consumer62 duplicate source admission".into());
    }
    Ok(())
}
pub(super) fn finish(plan: &Plan, a: &Audit) -> AuditResult<()> {
    if plan.successful != plan.contracts.keys().cloned().collect() {
        return Err(
            "consumer62 independent demanded classification != successful admissions".into(),
        );
    }
    pad_compared(plan, a)?;
    forbid_credit(a)
}

// Immutable declaration snapshot; no successful adapter output is consulted.
pub(super) fn demanded_sources(p: &Plan) -> BTreeMap<SourceKey, (String, String)> {
    p.contracts
        .iter()
        .map(|(k, c)| (k.clone(), (c.exact.into(), c.reason.into())))
        .collect()
}

// Installed actual prepared negative block: consumer62-integration-20261019-v1.UNVALIDATED.audit-negatives.rs.txt
// Historical preparation note: UNCOMPILED/UNRUN. Append inside consumer62_admission.rs as child tests after v1->v2->v3
// and integration-blockers. Helpers are real adapters; tests do not assert acceptance success.
#[cfg(test)]
mod tests {
    use super::*;
    fn baseline() -> AuditResult<(Inputs, Definitions, Audit)> {
        super::super::audit_at(
            &super::super::super::data_dir(),
            &super::super::super::reference_dir(),
        )?;
        let input = Inputs::load(
            &super::super::super::data_dir(),
            &super::super::super::reference_dir(),
        )?;
        let d = sim::load_ruleset(&input.data_dir).map_err(|e| e.to_string())?;
        let mut a = super::super::load_audit(&input, &d)?;
        for s in ["SPY", "MGTK", "AMRADR", "GAAIRC"] {
            let row = input
                .mappings
                .iter()
                .find(|r| r.section == s)
                .ok_or("actual control mapped row")?;
            super::super::items::metadata_flags(&mut a, &input.reference, &d, row)?;
        }
        for g in groups().keys() {
            loaded_group(&d, g)?;
        }
        mapping(&input)?;
        helper_graph(&input)?;
        forbid_credit(&a)?;
        Ok((input, d, a))
    }
    #[test]
    fn ra9_consumer62_exact_source_plan_is_independent_and_has_no_flag_credit() -> AuditResult<()> {
        let (input, _, a) = baseline()?;
        let c = contracts()?;
        assert_eq!(c.len(), 62);
        for row in c.values() {
            exact(&input, row)?;
        }
        forbid_credit(&a)?;
        assert!(a.witnesses.values().any(|w|matches!(w,Witness::Compared{dependencies,..}if dependencies.iter().any(|x|x.key==SourceKey::new("rules","MGTK","DisguiseWhenStill")))));
        Ok(())
    }
    #[test]
    fn ra9_consumer62_nonfirst_permadisguise_change_stales_policy_without_faking_native_change()
    -> AuditResult<()> {
        let (mut input, d, _baseline) = baseline()?;
        let key = SourceKey::new("rules", "SPY", "PermaDisguise");
        input.reference.rows.insert(key.clone(), "no".into());
        let row = input
            .mappings
            .iter()
            .find(|r| r.section == "SPY")
            .ok_or("SPY mapping")?;
        let mut a = super::super::load_audit(&input, &d)?;
        super::super::items::metadata_flags(&mut a, &input.reference, &d, row)?;
        assert!(
            exact(
                &input,
                contracts()?
                    .get(&key)
                    .ok_or("exact PermaDisguise contract")?
            )
            .is_err()
        );
        assert_eq!(
            loaded_group(&d, "disguise")?,
            contracts()?.get(&key).ok_or("specific contract")?.bindings
        );
        forbid_credit(&a)
    }
    #[test]
    fn ra9_consumer62_actual_mirage_flag_mutation_fails_real_adapter_after_valid_loader()
    -> AuditResult<()> {
        let _ = baseline()?;
        let copy = super::super::array_negatives::DataCopy::replace_row(
            "arsenal.ron",
            "(id: \"mirage_tank\",",
            "disguise_when_still: 1",
            "disguise_when_still: 0",
        )?;
        let (input, d) = copy.loaded()?;
        let mut a = super::super::load_audit(&input, &d)?;
        let row = input
            .mappings
            .iter()
            .find(|r| r.section == "MGTK")
            .ok_or("actual Mirage row")?;
        let error = super::super::items::metadata_flags(&mut a, &input.reference, &d, row)
            .err()
            .ok_or("real Mirage native flag mutation escaped adapter")?;
        assert!(error.contains("disguise_when_still"), "{error}");
        assert!(loaded_group(&d, "disguise").is_err());
        Ok(())
    }
    #[test]
    fn ra9_consumer62_nonfirst_repair_role_mutation_fails_fixed_registry_guard_after_valid_loader()
    -> AuditResult<()> {
        let _ = baseline()?;
        let copy = super::super::array_negatives::DataCopy::replace_row(
            "arsenal.ron",
            "(id: \"soviet_service_depot\",",
            "role: \"repair\"",
            "role: \"combat\"",
        )?;
        let (_, d) = copy.loaded()?;
        let error = exact_roles(&d)
            .err()
            .ok_or("nonfirst actual repair role removal escaped registry guard")?;
        assert!(error.contains("Repair"), "{error}");
        assert!(loaded_group(&d, "specialist-selectors").is_err());
        Ok(())
    }
    #[test]
    fn ra9_consumer62_nonfirst_pad_dock_count_fails_real_dock_adapter() -> AuditResult<()> {
        let (input, d, _) = baseline()?;
        let row = input
            .mappings
            .iter()
            .find(|r| r.section == "AMRADR")
            .ok_or("AMRADR control")?;
        let mut a = super::super::load_audit(&input, &d)?;
        super::super::items::docks(&mut a, &input.reference, &d, row, &input.mappings)?;
        let copy = super::super::array_negatives::DataCopy::replace_row(
            "arsenal.ron",
            "(id: \"american_airforce_command\",",
            "docks: 4",
            "docks: 3",
        )?;
        let (input, d) = copy.loaded()?;
        let row = input
            .mappings
            .iter()
            .find(|r| r.section == "AMRADR")
            .ok_or("AMRADR mutant")?;
        let mut a = super::super::load_audit(&input, &d)?;
        let error = super::super::items::docks(&mut a, &input.reference, &d, row, &input.mappings)
            .err()
            .ok_or("nonfirst actual pad mutation escaped source adapter")?;
        assert_eq!(
            error,
            r#"unexplained WitnessId { consumer: "american_airforce_command@rules:AMRADR", field: "arsenal.ron.entries.docks" }: actual "3", expected "4""#
        );
        Ok(())
    }
    #[test]
    fn ra9_consumer62_fragment_actual_mapping_activation_invalidates_old_nonread_guard()
    -> AuditResult<()> {
        let (mut input, _, _) = baseline()?;
        assert!(helper_graph(&input).is_ok());
        input
            .weapons
            .push(("comet_fragment".into(), "CometFragment".into()));
        let e = helper_graph(&input)
            .err()
            .ok_or("new actual helper consumer escaped old policy")?;
        assert!(e.contains("actual mapped weapon"), "{e}");
        Ok(())
    }
    #[test]
    fn ra9_consumer62_changed_global_second_selector_component_stales_exact_scope()
    -> AuditResult<()> {
        let (mut input, d, _) = baseline()?;
        let key = SourceKey::new("rules", "General", "HarvesterUnit");
        let c = contracts()?;
        exact(&input, c.get(&key).ok_or("selector contract")?)?;
        input.reference.rows.insert(key.clone(), "HARV,HARV".into());
        assert!(exact(&input, c.get(&key).ok_or("selector contract")?).is_err());
        assert!(
            loaded_group(&d, "specialist-selectors").is_ok(),
            "source nonread mutation must not manufacture native mutation"
        );
        Ok(())
    }
    #[test]
    fn ra9_consumer62_shrapnel_helper_branch_link_change_stales_nonread_scope() -> AuditResult<()> {
        let (mut input, _, _) = baseline()?;
        input.reference.rows.insert(
            SourceKey::new("rules", "TeslaFragment", "Projectile"),
            "InvisibleLow".into(),
        );
        assert!(helper_graph(&input).is_err());
        Ok(())
    }
    #[test]
    fn ra9_consumer62_individual_source_acceptances_start_fail_closed() -> AuditResult<()> {
        let (input, d, _) = baseline()?;
        let contracts = contracts()?;
        let c = contracts
            .values()
            .nth(1)
            .ok_or("nonfirst actual accepted source")?;
        let accepted = acceptance(&c.key)?;
        receipts(c, accepted.native_receipts)?;
        let proposal = super::policy_admission::SourceProposal {
            key: c.key.clone(),
            exact: c.exact,
            reason: c.reason,
            consumer_bindings: c.bindings,
            model_guard_id: c.guard,
        };
        let live = loaded_group(&d, c.group)?;
        let row = Unused {
            atom: Atom {
                key: c.key.clone(),
                portion: Portion::Whole,
            },
            exact_value: c.exact.into(),
            reason: c.reason.into(),
            code_evidence: "actual accepted source negative control".into(),
        };
        super::policy_admission::source_reason(
            &input,
            &row,
            &proposal,
            Some(&accepted),
            &live,
            super::policy_admission::input_binding(),
        )?;
        let line = ACCEPT
            .lines()
            .skip(1)
            .find(|l| {
                let f = l.split('\t').collect::<Vec<_>>();
                f.len() == 10 && f[0] == c.key.file && f[1] == c.key.section && f[2] == c.key.key
            })
            .ok_or("actual accepted record")?;
        let mut fields = line.split('\t').collect::<Vec<_>>();
        assert_ne!(fields[7], "UNACCEPTED");
        fields[7] = "UNACCEPTED";
        let bad = ACCEPT.replacen(line, &fields.join("\t"), 1);
        let rejected = acceptance_from(&bad, &c.key)?;
        assert!(
            super::policy_admission::source_reason(
                &input,
                &row,
                &proposal,
                Some(&rejected),
                &live,
                super::policy_admission::input_binding()
            )
            .is_err()
        );
        Ok(())
    }
}
