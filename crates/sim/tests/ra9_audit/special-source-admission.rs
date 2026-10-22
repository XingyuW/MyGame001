//! Historical preparation note: UNCOMPILED/UNRUN special exact3 nonread keys +7 source-presentation premises.
//! Root records actual source/native/client decisions. No nonexistent source flag witness.
use super::*;
use source_routes::{Class, Route};
type Specification = (SourceKey, Route, Class, String, String);
const CONTRACTS: &str = include_str!("special-source-contracts.tsv");
const PHYSICAL: &str = include_str!("special-physical-demands.tsv");
const ORIGINAL: &str = include_str!("special-original-expected.tsv");
pub(super) struct Plan {
    wanted: BTreeMap<SourceKey, (Route, Class, String, String)>,
    done: BTreeSet<SourceKey>,
    physical: BTreeSet<WitnessId>,
}
fn rows<'a>(text: &'a str, header: &str, n: usize) -> AuditResult<Vec<Vec<&'a str>>> {
    if text.lines().next() != Some(header) {
        return Err("special source exact header".into());
    }
    text.lines()
        .skip(1)
        .map(|l| {
            let v = l.split('\t').collect::<Vec<_>>();
            if v.len() != n {
                Err("special source exact width".into())
            } else {
                Ok(v)
            }
        })
        .collect()
}
pub(super) fn specifications() -> AuditResult<Vec<Specification>> {
    let mut v = Vec::new();
    for r in rows(
        CONTRACTS,
        "file\tsection\tkey\texact_value\troute\tclassification\tspecific_semantic_reason",
        7,
    )? {
        let route = match r[4] {
            "vhp" => Route::Vhp,
            "tree" => Route::Tree,
            "mirage-elite" => Route::MirageElite,
            "spark" => Route::Spark,
            _ => return Err("unknown special key route".into()),
        };
        let class = match r[5] {
            "CURRENT_MODEL_NONREAD" => Class::CurrentModelNonread,
            "INTENTIONAL_DIFFERENCE" => Class::IntentionalDifference,
            _ => return Err("special source cannot assert original-engine unused".into()),
        };
        v.push((
            SourceKey::new(r[0], r[1], r[2]),
            route,
            class,
            r[3].into(),
            r[6].into(),
        ));
    }
    if v.len() != 10 {
        return Err("exact10 special source metadata census".into());
    }
    Ok(v)
}
fn original(s: &scope_inputs::ObservedScope) -> AuditResult<()> {
    if s.special_original() != ORIGINAL {
        return Err(
            "actual full-original VHP/Spark query output differs from exact source premises".into(),
        );
    }
    Ok(())
}
fn vhp_keys(i: &Inputs) -> AuditResult<()> {
    let found = i
        .reference
        .rows
        .iter()
        .filter(|(k, _)| k.key.eq_ignore_ascii_case("vhpscan"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect::<Vec<_>>();
    if found != vec![(SourceKey::new("rules", "NASAM", "VHPScan"), "Strong".into())] {
        return Err("VHP full reference singleton/literal/case identity changed".into());
    }
    Ok(())
}
fn vhp_loaded(i: &Inputs, d: &Definitions) -> AuditResult<()> {
    let m = i
        .mappings
        .iter()
        .filter(|m| m.section == "NASAM" || m.game_id == "patriot")
        .collect::<Vec<_>>();
    if m.len() != 1
        || m[0].section != "NASAM"
        || m[0].game_id != "patriot"
        || !m[0].building
        || m[0].side != "allied"
        || m[0].note != "-"
    {
        return Err("exact NASAM/Patriot consumer changed".into());
    }
    let k = d.building_kind("patriot").ok_or("Patriot actual kind")?;
    let t = d.building_combat(k).ok_or("Patriot actual combat")?;
    let red = d.weapon_kind("red_eye2").ok_or("actual RedEye2")?;
    if t.weapons(false) != [Some(red), None]
        || t.weapons(true) != [Some(red), None]
        || !d
            .metadata(sim::ProductionItem::Building(k))
            .is_some_and(|m| m.powered)
    {
        return Err("VHP current primary/elite/powered branch changed".into());
    }
    if !d
        .weapon(red)
        .and_then(|w| d.warhead(w.warhead))
        .is_some_and(|h| h.id == "samwh")
    {
        return Err("VHP selected real SAMWH changed".into());
    }
    Ok(())
}
fn no_credit(
    a: &Audit,
    wanted: &BTreeMap<SourceKey, (Route, Class, String, String)>,
) -> AuditResult<()> {
    if a.required_dependencies.iter().any(|(_,v)|wanted.contains_key(&v.key))||a.witnesses.values().any(|w|matches!(w,Witness::Compared{dependencies,..}if dependencies.iter().any(|v|wanted.contains_key(&v.key)))){return Err("special nonread atom falsely credited as physical source dependency".into());}
    Ok(())
}
fn compared(a: &Audit, w: &WitnessId) -> AuditResult<()> {
    match a.witnesses.get(w) {
        Some(Witness::Compared { dependencies, .. }) => {
            let expected = a
                .required_dependencies
                .iter()
                .filter(|(id, _)| id == w)
                .map(|(_, atom)| atom.clone())
                .collect::<BTreeSet<_>>();
            if &expected != dependencies {
                return Err("VHP genuine physical demanded/compared dependencies differ".into());
            }
            Ok(())
        }
        _ => Err(format!("VHP missing actual compared neighbor {w:?}")),
    }
}
pub(super) fn declare(
    i: &Inputs,
    d: &Definitions,
    a: &Audit,
    s: &scope_inputs::ObservedScope,
) -> AuditResult<Plan> {
    let mut wanted = BTreeMap::new();
    for (k, route, class, exact, reason) in specifications()? {
        if i.reference.rows.get(&k) != Some(&exact)
            || i.reference.shapes.contains_key(&k)
            || wanted.insert(k, (route, class, exact, reason)).is_some()
        {
            return Err("special exact key/value/whole demand changed".into());
        }
    }
    original(s)?;
    vhp_keys(i)?;
    vhp_loaded(i, d)?;
    no_credit(a, &wanted)?;
    let mut physical = BTreeSet::new();
    for r in rows(PHYSICAL, "route\tconsumer\tphysical_field", 3)? {
        if r[0] != "vhp" {
            return Err("unknown special physical scope".into());
        }
        let w = a.physical(WitnessId::new(r[1], r[2]));
        if !a.required.contains(&w) || !physical.insert(w) {
            return Err("VHP missing/duplicate independent real physical demand".into());
        }
    }
    Ok(Plan {
        wanted,
        physical,
        done: BTreeSet::new(),
    })
}
fn routes(route: Route) -> BTreeSet<&'static str> {
    match route {
        Route::Vhp => [
            "native/vhp/three-native-patriots-oversaturate",
            "native/vhp/current-target-death-releases",
            "native/vhp/self-repair-during-delayed-flight",
            "native/vhp/source-death-preserves-missile",
            "static/vhp/current-automatic-retained-nearest-source-census",
        ]
        .into_iter()
        .collect(),
        Route::Tree => [
            "client/tree/ordinary-cursor-preview-sink",
            "client/tree/ctrl-forcefire-positive",
            "client/tree/hidden-missing-dead-matrix",
            "static/tree/ra2-only-shown-scenery-branch",
        ]
        .into_iter()
        .collect(),
        Route::MirageElite => [
            "native/fire/mirage-undisguised-base-and-elite-primary",
            "static/fire/current-primary-first-eligibility-mask-and-source-uncertainty",
        ]
        .into_iter()
        .collect(),
        Route::Spark => [
            "static/spark/full-original-nondamage-chain",
            "static/spark/generated-electric-effects-current-consumers",
        ]
        .into_iter()
        .collect(),
        _ => BTreeSet::new(),
    }
}
#[allow(
    clippy::too_many_arguments,
    reason = "Independent actual, expected, difference and evidence inputs remain explicit at the audit boundary"
)]
pub(super) fn admit(
    p: &mut Plan,
    a: &Audit,
    i: &Inputs,
    d: &Definitions,
    row: &Unused,
    route: Route,
    s: &scope_inputs::ObservedScope,
    fire: &fire_keys::Evidence,
) -> AuditResult<()> {
    let spec = p
        .wanted
        .get(&row.atom.key)
        .ok_or("unowned special source")?;
    if spec.0 != route
        || row.atom.portion != Portion::Whole
        || row.exact_value != spec.2
        || row.reason != spec.3
    {
        return Err("exact special source route/reason changed".into());
    }
    original(s)?;
    no_credit(a, &p.wanted)?;
    match route {
        Route::Vhp => {
            vhp_keys(i)?;
            vhp_loaded(i, d)?;
            for w in &p.physical {
                compared(a, w)?;
            }
        }
        Route::MirageElite => fire_audit::require_masked_elite(i, d, &fire.mirage_elite)?,
        Route::Tree | Route::Spark => {} // Actual final source census + exact per-source decision and
        // client/static route evidence below are mandatory. No boolean constant/read is a witness.
        _ => return Err("unknown special source route".into()),
    }
    let accepted = s.accepted_source(row)?;
    let guard = format!(
        "ra9.compositor.special.{route:?}.{}.{}",
        row.atom.key.section, row.atom.key.key
    );
    // Exact observed source/consumer premise fingerprint. This is a policy binding, never
    // a synthetic NativeWitness. Actual runtime/client receipts remain separate below.
    let binding = format!(
        "source={}:{}:{};model-census={};original-closure={}",
        row.atom.key.file,
        row.atom.key.section,
        row.atom.key.key,
        s.model_sha256(),
        s.original_closure_sha256()
    );
    if accepted.model_guard_id != guard || accepted.consumer_bindings != binding {
        return Err("special exact live source/model/consumer contract identity changed".into());
    }
    s.require_routes(accepted.native_receipts, &routes(route))?;
    if !p.done.insert(row.atom.key.clone()) {
        return Err("duplicate special source admission".into());
    }
    Ok(())
}
pub(super) fn finish(p: &Plan, a: &Audit) -> AuditResult<()> {
    if p.done != p.wanted.keys().cloned().collect() {
        return Err("special source demand != successful guarded classifications".into());
    }
    no_credit(a, &p.wanted)
}
