//! Historical preparation note: UNCOMPILED/UNRUN exact75 implementation-model nonread admission. No coverage from reading,
//! reasons, aliases or defaults. This module introduces no simulation field/getter/state.
//! Use with the actual v3 ledger + policy-admission.rs only after coordinated RA9 integration.
use super::*;
const TABLE: &str = include_str!("guard75-contracts.tsv");
const ROOTS: &str = include_str!("guard75-roots.tsv");
const SELECTORS: &str = include_str!("guard75-selectors.tsv");
const ORIGINAL: &str = include_str!("guard75-original-scope.tsv");
const ROSTER: &str = include_str!("guard75-loaded-roster.tsv");
const GATES: &str = include_str!("guard75-loaded-gates.tsv");
const PHYSICAL: &str = include_str!("guard75-physical-demands.tsv");
const ACCEPT: &str = include_str!("guard75-acceptance.tsv");
const RULES_SHA: &str = "adc869ca037db5141f562f18092d022045e194de0d1204a46b9ec8372b2b826f";
#[derive(Clone)]
struct Spec {
    key: SourceKey,
    exact: &'static str,
    group: &'static str,
    reason: &'static str,
    bindings: &'static str,
    guard: &'static str,
    routes: &'static str,
}
pub(super) struct Plan {
    wanted: BTreeMap<SourceKey, Spec>,
    physical: BTreeMap<String, BTreeSet<WitnessId>>,
    done: BTreeSet<SourceKey>,
}
// Generated original-scope observations and actual code-census receipt are separate inputs.
// Root must verify these real bytes/logs against the final input binding. Declared hash strings
// are NOT authentication. Do not pass the immutable expected table as its own observation.
pub(super) struct Scope<'a> {
    pub(super) observed_original: &'a str,
    pub(super) original_sha256: &'a str,
    pub(super) model_census_sha256: &'a str,
    pub(super) observed_model_scope: &'a str,
    pub(super) verified_final_binding: &'a str,
}
fn records(text: &str, width: usize) -> AuditResult<Vec<Vec<&str>>> {
    let headers = [
        "file\tsection\tkey\texact_value\tgroup_id\tclassification\tspecific_reason\tconsumer_bindings\tmodel_guard_id\trequired_routes\tprimary_line",
        "kind\tgame_id\tsource_section\tside\tnote",
        "section\tkey\tvalue",
        "group\tfact_kind\tsection_or_target\tkey\tvalue",
        "kind\tgame_id\trole\tdomain\tstrategic",
        "group_id\tconsumer\treal_field\texpected_model_value",
        "group_id\tconsumer\tphysical_field",
        "file\tsection\tkey\texact_value\tspecific_reason\tconsumer_bindings\tmodel_guard_id\trecorded_decision\tfinal_input_sha256\tnative_receipts",
    ];
    if !text.lines().next().is_some_and(|h| headers.contains(&h)) {
        return Err("guard75 exact table header".into());
    }
    text.lines()
        .skip(1)
        .map(|l| {
            let r = l.split('\t').collect::<Vec<_>>();
            if r.len() != width {
                Err("guard75 table width".into())
            } else {
                Ok(r)
            }
        })
        .collect()
}
fn counts() -> BTreeMap<&'static str, usize> {
    [
        ("atom-ammo", 2),
        ("gems", 4),
        ("inactive-droppod", 4),
        ("inactive-firestorm", 5),
        ("inactive-gates", 7),
        ("inactive-hunter", 8),
        ("inactive-ion-emp", 10),
        ("inactive-powerup", 1),
        ("inactive-trigger", 1),
        ("inactive-tunnel", 2),
        ("inactive-veins", 9),
        ("inactive-visceroid", 5),
        ("ore-growthrate", 1),
        ("paratrooper-typo", 1),
        ("prereq-override", 2),
        ("radiation-typo", 1),
        ("solo-crates", 1),
        ("technical-countries", 6),
        ("turbo", 4),
        ("upgrades", 1),
    ]
    .into_iter()
    .collect()
}
fn catalogue_from(text: &'static str) -> AuditResult<BTreeMap<SourceKey, Spec>> {
    let mut out = BTreeMap::new();
    let mut count = BTreeMap::new();
    for r in records(text, 11)? {
        let key = SourceKey::new(r[0], r[1], r[2]);
        let s = Spec {
            key: key.clone(),
            exact: r[3],
            group: r[4],
            reason: r[6],
            bindings: r[7],
            guard: r[8],
            routes: r[9],
        };
        if r[5] != "IMPLEMENTATION_MODEL_NONREAD"
            || s.reason.trim().is_empty()
            || s.reason.starts_with("PENDING")
            || out.insert(key, s.clone()).is_some()
        {
            return Err("guard75 duplicate/unspecific classification".into());
        }
        *count.entry(s.group).or_insert(0usize) += 1;
    }
    if count != counts() || out.len() != 75 {
        return Err("guard75 independent classification census".into());
    }
    Ok(out)
}
fn catalogue() -> AuditResult<BTreeMap<SourceKey, Spec>> {
    catalogue_from(TABLE)
}
fn source(input: &Inputs, s: &Spec) -> AuditResult<()> {
    if input.reference.rows.get(&s.key).map(String::as_str) != Some(s.exact)
        || input.reference.shapes.contains_key(&s.key)
    {
        return Err(format!(
            "guard75 exact source/whole shape changed {:?}",
            s.key
        ));
    }
    Ok(())
}
fn no_credit(a: &Audit) -> AuditResult<()> {
    let keys = catalogue()?.into_keys().collect::<BTreeSet<_>>();
    if a.required_dependencies.iter().any(|(_,v)|keys.contains(&v.key))||a.witnesses.values().any(|w|matches!(w,Witness::Compared{dependencies,..}if dependencies.iter().any(|v|keys.contains(&v.key)))){return Err("guard75 nonread source falsely demanded/Compared".into());}
    Ok(())
}
fn scope(s: &Scope<'_>) -> AuditResult<()> {
    if s.original_sha256 != RULES_SHA
        || s.model_census_sha256 != include_str!("ra9-approved-model-census-sha256.txt").trim()
        || s.observed_original != ORIGINAL
        || s.observed_model_scope.trim() != include_str!("ra9-approved-model-surface.txt").trim()
        || s.verified_final_binding.len() != 64
        || !s
            .verified_final_binding
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
    {
        return Err(
            "guard75 full original/producer/command/model scope observation changed or unverified"
                .into(),
        );
    }
    Ok(())
}
fn roots(input: &Inputs, d: &Definitions) -> AuditResult<()> {
    let expected = records(ROOTS, 5)?
        .into_iter()
        .map(|r| r.into_iter().map(str::to_owned).collect::<Vec<_>>())
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    for m in &input.mappings {
        if !actual.insert(vec![
            (if m.building { "building" } else { "unit" }).into(),
            m.game_id.clone(),
            m.section.clone(),
            m.side.clone(),
            m.note.clone(),
        ]) {
            return Err("guard75 duplicate mapped root".into());
        }
    }
    for (kind, rows) in [("weapon", &input.weapons), ("warhead", &input.warheads)] {
        for (id, s) in rows {
            if !actual.insert(vec![
                kind.into(),
                id.clone(),
                s.clone(),
                "-".into(),
                "-".into(),
            ]) {
                return Err("guard75 duplicate combat root".into());
            }
        }
    }
    if actual != expected {
        return Err("guard75 complete mapped/producer root closure changed".into());
    }
    for (building, loaded) in [
        (
            false,
            d.units()
                .iter()
                .map(|v| v.id.as_str())
                .collect::<BTreeSet<_>>(),
        ),
        (true, d.buildings().iter().map(|v| v.id.as_str()).collect()),
    ] {
        if loaded
            != input
                .mappings
                .iter()
                .filter(|v| v.building == building)
                .map(|v| v.game_id.as_str())
                .collect()
        {
            return Err("guard75 actual loaded item roster differs from roots".into());
        }
    }
    if d.weapons()
        .iter()
        .map(|v| v.id.as_str())
        .collect::<BTreeSet<_>>()
        != input.weapons.iter().map(|v| v.0.as_str()).collect()
        || d.warheads()
            .iter()
            .map(|v| v.id.as_str())
            .collect::<BTreeSet<_>>()
            != input.warheads.iter().map(|v| v.0.as_str()).collect()
    {
        return Err(
            "guard75 actual loaded weapon/warhead roster differs from complete mappings".into(),
        );
    }
    Ok(())
}
fn selector_key(k: &str) -> bool {
    [
        "Primary",
        "Secondary",
        "ElitePrimary",
        "EliteSecondary",
        "DeathWeapon",
        "Spawns",
        "SuperWeapon",
        "WeaponType",
        "Projectile",
        "Warhead",
        "PowersUpBuilding",
        "PowersUpToLevel",
        "DeploysInto",
        "UndeploysInto",
        "Locomotor",
        "MovementZone",
        "Weapon1",
        "Weapon2",
        "Weapon3",
        "Weapon4",
        "Weapon5",
        "Weapon6",
        "Weapon7",
        "Weapon8",
        "Weapon9",
        "Weapon10",
        "Weapon11",
        "Weapon12",
        "Weapon13",
        "EliteWeapon1",
        "EliteWeapon2",
        "EliteWeapon3",
        "EliteWeapon4",
        "EliteWeapon5",
        "EliteWeapon6",
        "EliteWeapon7",
        "EliteWeapon8",
        "EliteWeapon9",
        "EliteWeapon10",
        "EliteWeapon11",
        "EliteWeapon12",
        "EliteWeapon13",
    ]
    .contains(&k)
}
fn selectors(input: &Inputs) -> AuditResult<()> {
    let roots = input
        .mappings
        .iter()
        .map(|m| m.section.as_str())
        .chain(input.weapons.iter().map(|m| m.1.as_str()))
        .chain(input.warheads.iter().map(|m| m.1.as_str()))
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    for (k, v) in &input.reference.rows {
        if k.file == "rules" && roots.contains(k.section.as_str()) && selector_key(&k.key) {
            actual.insert(vec![k.section.clone(), k.key.clone(), v.clone()]);
        }
    }
    let expected = records(SELECTORS, 3)?
        .into_iter()
        .map(|r| r.into_iter().map(str::to_owned).collect())
        .collect();
    if actual != expected {
        return Err("guard75 complete active source selector/locomotor closure changed or extraction incomplete".into());
    }
    Ok(())
}
fn entry<'a>(d: &'a Definitions, id: &str) -> AuditResult<&'a sim::ArsenalEntry> {
    let item = if let Some(k) = d.unit_kind(id) {
        sim::ProductionItem::Unit(k)
    } else {
        sim::ProductionItem::Building(d.building_kind(id).ok_or("guard75 loaded item missing")?)
    };
    d.metadata(item)
        .ok_or_else(|| "guard75 metadata missing".into())
}
fn loaded_roster(d: &Definitions) -> AuditResult<()> {
    let expected = records(ROSTER, 5)?
        .into_iter()
        .map(|r| r.into_iter().map(str::to_owned).collect::<Vec<_>>())
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    for (item, m) in &d.arsenal().0 {
        let (kind, id) = match item {
            sim::ProductionItem::Unit(k) => {
                ("unit", d.unit(*k).ok_or("guard75 bad unit")?.id.clone())
            }
            sim::ProductionItem::Building(k) => (
                "building",
                d.building(*k).ok_or("guard75 bad building")?.id.clone(),
            ),
        };
        actual.insert(vec![
            kind.into(),
            id,
            format!("{:?}", m.role),
            format!("{:?}", m.domain),
            m.strategic.to_string(),
        ]);
    }
    if actual != expected {
        return Err("guard75 real loaded role/domain/strategic producer roster changed".into());
    }
    Ok(())
}
fn observe(d: &Definitions, id: &str, field: &str) -> AuditResult<String> {
    let r = d.rules().ok_or("guard75 real Rules missing")?;
    if id == "rules" {
        return match field {
            "countries" => Ok(r
                .countries
                .iter()
                .map(|c| format!("{}:{}", c.id, c.side.id()))
                .collect::<Vec<_>>()
                .join(",")),
            "sides" => Ok(r
                .sides
                .iter()
                .map(|s| s.side.id())
                .collect::<Vec<_>>()
                .join(",")),
            "paradrops" => r
                .sides
                .iter()
                .map(|s| {
                    Ok(format!(
                        "{}:{}:{}",
                        s.side.id(),
                        d.unit(s.paradrop_infantry)
                            .ok_or("guard75 bad side payload")?
                            .id,
                        s.paradrop_count
                    ))
                })
                .collect::<AuditResult<Vec<_>>>()
                .map(|v| v.join(",")),
            "powerups" => Ok(r
                .powerups
                .iter()
                .map(|p| format!("{}:{}:{}", p.id, p.share, p.value))
                .collect::<Vec<_>>()
                .join(",")),
            "ore" => Ok(format!(
                "{}:{}:{}:{}:{}:{}",
                r.economy.ore_growth_interval,
                r.economy.ore_growth_pm,
                r.economy.ore_spread_interval,
                r.economy.ore_spread_pm,
                r.economy.ore_max_bails,
                r.economy.ore_spread_above
            )),
            "american_count" => Ok(r.superweapons.american_paradrop_count.to_string()),
            _ => Err("guard75 unknown real Rules premise".into()),
        };
    }
    if let Some(f) = field.strip_prefix("weapon.") {
        let w = d
            .weapon(d.weapon_kind(id).ok_or("guard75 missing real weapon")?)
            .ok_or("guard75 weapon")?;
        return match f {
            "projectile_speed" => Ok(w.projectile_speed.to_string()),
            "damage" => Ok(w.damage.to_string()),
            "rad_level" => Ok(w.rad_level.to_string()),
            "warhead" => Ok(d.warhead(w.warhead).ok_or("guard75 warhead")?.id.clone()),
            "anti_air" => Ok(w.anti_air.to_string()),
            _ => Err("guard75 unknown actual weapon premise".into()),
        };
    }
    if field == "immune_radiation" {
        return Ok(entry(d, id)?.immune_radiation.to_string());
    }
    Err("guard75 unsupported/fictional loaded getter".into())
}
fn loaded(d: &Definitions, group: &str) -> AuditResult<()> {
    for r in records(GATES, 4)?.into_iter().filter(|r| r[0] == group) {
        if observe(d, r[1], r[2])? != r[3] {
            return Err(format!(
                "guard75 actual loaded premise changed {}.{}",
                r[1], r[2]
            ));
        }
    }
    Ok(())
}
fn compared(a: &Audit, w: &WitnessId) -> AuditResult<()> {
    match a.witnesses.get(w) {
        Some(Witness::Compared { dependencies, .. }) => {
            let wanted = a
                .required_dependencies
                .iter()
                .filter(|(id, _)| id == w)
                .map(|(_, v)| v.clone())
                .collect::<BTreeSet<_>>();
            if &wanted != dependencies {
                return Err(format!(
                    "guard75 actual comparison dependency mismatch {w:?}"
                ));
            }
            Ok(())
        }
        _ => Err(format!(
            "guard75 actual neighboring physical field not Compared {w:?}"
        )),
    }
}
fn live_binding(input: &Inputs, d: &Definitions, group: &str) -> AuditResult<String> {
    let mut facts = Vec::new();
    for r in records(GATES, 4)?.into_iter().filter(|r| r[0] == group) {
        facts.push(format!("{}.{}={}", r[1], r[2], observe(d, r[1], r[2])?));
    }
    let roots = input
        .mappings
        .iter()
        .map(|m| m.section.as_str())
        .chain(input.weapons.iter().map(|m| m.1.as_str()))
        .chain(input.warheads.iter().map(|m| m.1.as_str()))
        .collect::<BTreeSet<_>>();
    let edges = input
        .reference
        .rows
        .keys()
        .filter(|k| k.file == "rules" && roots.contains(k.section.as_str()) && selector_key(&k.key))
        .count();
    Ok(format!(
        "group={group};roots={};selectors={edges};loaded-items={};role-entries={};gates={}",
        input.mappings.len() + input.weapons.len() + input.warheads.len(),
        d.units().len() + d.buildings().len(),
        d.arsenal().0.len(),
        facts.join("|")
    ))
}
pub(super) fn declare(
    input: &Inputs,
    d: &Definitions,
    a: &Audit,
    s: &Scope<'_>,
) -> AuditResult<Plan> {
    let wanted = catalogue()?;
    for c in wanted.values() {
        source(input, c)?;
    }
    scope(s)?;
    roots(input, d)?;
    selectors(input)?;
    loaded_roster(d)?;
    for g in counts().keys() {
        loaded(d, g)?;
    }
    no_credit(a)?;
    let mut physical: BTreeMap<String, BTreeSet<WitnessId>> = BTreeMap::new();
    for r in records(PHYSICAL, 3)? {
        if !counts().contains_key(r[0]) {
            return Err("guard75 unknown physical-demand family".into());
        }
        let w = a.physical(WitnessId::new(r[1], r[2]));
        if !a.required.contains(&w) {
            return Err(format!(
                "guard75 independent real physical demand absent {w:?}"
            ));
        }
        if !physical.entry(r[0].into()).or_default().insert(w) {
            return Err("guard75 duplicate physical demand".into());
        }
    }
    Ok(Plan {
        wanted,
        physical,
        done: BTreeSet::new(),
    })
}
fn acceptance_from<'a>(
    text: &'a str,
    key: &SourceKey,
) -> AuditResult<super::policy_admission::SourceAcceptance<'a>> {
    let mut keys = BTreeSet::new();
    let mut found = None;
    for r in records(text, 10)? {
        let k = SourceKey::new(r[0], r[1], r[2]);
        if !keys.insert(k.clone()) {
            return Err("guard75 duplicate individual acceptance".into());
        }
        if &k == key {
            found = Some(super::policy_admission::SourceAcceptance {
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
    if keys != catalogue()?.into_keys().collect() {
        return Err("guard75 missing/foreign individual acceptance".into());
    }
    found.ok_or_else(|| "guard75 no individual acceptance".into())
}
fn receipts(s: &Spec, text: &str) -> AuditResult<()> {
    let want = s.routes.split(';').collect::<BTreeSet<_>>();
    let mut got = BTreeSet::new();
    for token in text.split(';') {
        let (id, h) = token
            .split_once('@')
            .ok_or("guard75 requires typed route@actual-artifact-sha256")?;
        if !matches!(id.split('/').next(), Some("native" | "loaded" | "static"))
            || h.len() != 64
            || !h.bytes().all(|b| b.is_ascii_hexdigit())
            || !got.insert(id)
        {
            return Err("guard75 malformed/duplicate receipt".into());
        }
    }
    if want != got {
        return Err("guard75 missing/foreign static/loaded/native route receipt".into());
    }
    Ok(())
}
fn admit_from(
    plan: &mut Plan,
    a: &Audit,
    input: &Inputs,
    d: &Definitions,
    row: &Unused,
    s: &Scope<'_>,
    acceptance_text: &str,
) -> AuditResult<()> {
    let c = plan
        .wanted
        .get(&row.atom.key)
        .ok_or("outside exact75 guarded-nonread scope")?;
    source(input, c)?;
    scope(s)?;
    roots(input, d)?;
    selectors(input)?;
    loaded_roster(d)?;
    loaded(d, c.group)?;
    no_credit(a)?;
    if row.atom.portion != Portion::Whole || row.exact_value != c.exact || row.reason != c.reason {
        return Err("guard75 exact source reason/shape mismatch".into());
    }
    if let Some(fields) = plan.physical.get(c.group) {
        for w in fields {
            compared(a, w)?;
        }
    }
    let accepted = acceptance_from(acceptance_text, &c.key)?;
    let p = super::policy_admission::SourceProposal {
        key: c.key.clone(),
        exact: c.exact,
        reason: c.reason,
        consumer_bindings: c.bindings,
        model_guard_id: c.guard,
    };
    let measured = live_binding(input, d, c.group)?;
    // Derived actual roster/selector counts and real loaded premise values follow exact full
    // set checks; this is a policy binding, never a getter for the omitted subsystem.
    super::policy_admission::source_reason(
        input,
        row,
        &p,
        Some(&accepted),
        &measured,
        s.verified_final_binding,
    )?;
    receipts(c, accepted.native_receipts)?;
    if !plan.done.insert(c.key.clone()) {
        return Err("guard75 duplicate successful classification".into());
    }
    Ok(())
}
pub(super) fn admit(
    plan: &mut Plan,
    a: &Audit,
    input: &Inputs,
    d: &Definitions,
    row: &Unused,
    s: &Scope<'_>,
) -> AuditResult<()> {
    admit_from(plan, a, input, d, row, s, ACCEPT)
}
pub(super) fn finish(plan: &Plan, a: &Audit) -> AuditResult<()> {
    if plan.done != plan.wanted.keys().cloned().collect() {
        return Err(
            "guard75 independently demanded classifications != successful exact admissions".into(),
        );
    }
    no_credit(a)
}
// Compile-checked current command/domain/role surface. No wildcard can silently admit a new
// subsystem. This is static closure evidence, never a native execution result.
fn command_surface(c: &sim::Command) -> &'static str {
    match c {
        sim::Command::Guard { .. } => "Guard",
        sim::Command::Scatter { .. } => "Scatter",
        sim::Command::Follow { .. } => "Follow",
        sim::Command::ForceFire { .. } => "ForceFire",
        sim::Command::ForceMove { .. } => "ForceMove",
        sim::Command::QueueMove { .. } => "QueueMove",
        sim::Command::Deploy { .. } => "Deploy",
        sim::Command::Undeploy { .. } => "Undeploy",
        sim::Command::Interact { .. } => "Interact",
        sim::Command::Chronoshift { .. } => "Chronoshift",
        sim::Command::Superweapon { .. } => "Superweapon",
        sim::Command::Move { .. } => "Move",
        sim::Command::Stop { .. } => "Stop",
        sim::Command::Harvest { .. } => "Harvest",
        sim::Command::QueueProduction { .. } => "QueueProduction",
        sim::Command::CancelProduction { .. } => "CancelProduction",
        sim::Command::PlaceBuilding { .. } => "PlaceBuilding",
        // R218/R219: the user-approved surface addition.
        sim::Command::PlaceUnit { .. } => "PlaceUnit",
        sim::Command::SellBuilding { .. } => "SellBuilding",
        sim::Command::RepairBuilding { .. } => "RepairBuilding",
        sim::Command::SetRallyPoint { .. } => "SetRallyPoint",
        sim::Command::Attack { .. } => "Attack",
        sim::Command::AttackMove { .. } => "AttackMove",
    }
}
fn domain_surface(d: sim::Domain) -> &'static str {
    match d {
        sim::Domain::Land => "Land",
        sim::Domain::Air => "Air",
        sim::Domain::Naval => "Naval",
        sim::Domain::Amphibious => "Amphibious",
    }
}
fn role_surface(r: sim::Role) -> &'static str {
    match r {
        sim::Role::Combat => "Combat",
        sim::Role::Engineer => "Engineer",
        sim::Role::Dog => "Dog",
        sim::Role::Spy => "Spy",
        sim::Role::Commando => "Commando",
        sim::Role::Chrono => "Chrono",
        sim::Role::Flak => "Flak",
        sim::Role::Tesla => "Tesla",
        sim::Role::Demolition => "Demolition",
        sim::Role::MindControl => "MindControl",
        sim::Role::Radiation => "Radiation",
        sim::Role::Suicide => "Suicide",
        sim::Role::ChronoMiner => "ChronoMiner",
        sim::Role::Transport => "Transport",
        sim::Role::Ifv => "Ifv",
        sim::Role::Drone => "Drone",
        sim::Role::Mcv => "Mcv",
        sim::Role::Mirage => "Mirage",
        sim::Role::Splash => "Splash",
        sim::Role::Strike => "Strike",
        sim::Role::Carrier => "Carrier",
        sim::Role::Squid => "Squid",
        sim::Role::Shipyard => "Shipyard",
        sim::Role::Purifier => "Purifier",
        sim::Role::Repair => "Repair",
        sim::Role::Cloner => "Cloner",
        sim::Role::Gap => "Gap",
        sim::Role::Satellite => "Satellite",
        sim::Role::Sensor => "Sensor",
        sim::Role::Chronosphere => "Chronosphere",
        sim::Role::Storm => "Storm",
        sim::Role::Curtain => "Curtain",
        sim::Role::Nuke => "Nuke",
        sim::Role::Paradrop => "Paradrop",
    }
}
fn match_rule_surface(r: sim::MatchRules) {
    let sim::MatchRules {
        short_game: _,
        superweapons: _,
        mcv_repacks: _,
        crates: _,
        ore_grows: _,
        bridge_destruction: _,
        fog_of_war: _,
    } = r;
}
fn option_surface(o: sim::MatchOptions) {
    let sim::MatchOptions {
        super_players: _,
        unlock_players: _,
        unit_speed_percent: _,
        free_for_all: _,
        random_countries: _,
        starting_credits: _,
        unit_count: _,
        mcv_start: _,
        short_game: _,
        superweapons: _,
        crates: _,
        mcv_repacks: _,
        ore_grows: _,
        bridge_destruction: _,
        fog_of_war: _,
    } = o;
}

#[cfg(test)]
#[path = "guard75-negatives.rs"]
mod tests;

pub(super) fn demanded_sources(p: &Plan) -> BTreeMap<SourceKey, (String, String)> {
    p.wanted
        .iter()
        .map(|(k, c)| (k.clone(), (c.exact.into(), c.reason.into())))
        .collect()
}

// Exhaustive compiler surface guards only; these grant no native/source coverage.
const _: fn(&sim::Command) -> &'static str = command_surface;
const _: fn(sim::Domain) -> &'static str = domain_surface;
const _: fn(sim::Role) -> &'static str = role_surface;
const _: fn(sim::MatchRules) = match_rule_surface;
const _: fn(sim::MatchOptions) = option_surface;

pub(super) fn contains(k: &SourceKey) -> bool {
    catalogue().is_ok_and(|c| c.contains_key(k))
}
// Keep the exact typed source-membership API checked; this grants no coverage.
const _: fn(&SourceKey) -> bool = contains;
