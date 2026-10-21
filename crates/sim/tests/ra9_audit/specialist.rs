//! UNVALIDATED v2 metadata policy adapters. No generator/reference coverage is obtained by
//! recognizing a Role enum or a legacy numeric metadata value. Active native terms use
//! Expected provenance; legacy-only fields require actual loaded branch premises.
use super::actual::{check_int, optional, rule};
use super::combat::{Conversion, converted};
use super::*;
use sim::Role;
fn native_selected<'a>(
    d: &'a Definitions,
    row: &MappingRow,
) -> AuditResult<Vec<(&'a sim::WeaponDef, &'a sim::WarheadDef)>> {
    let item = items::item(d, row)?;
    let t = match item {
        sim::ProductionItem::Unit(k) => d.unit_combat(k),
        sim::ProductionItem::Building(k) => d.building_combat(k),
    }
    .ok_or("missing actual combat")?;
    let mut out = Vec::new();
    for k in t
        .weapons(false)
        .into_iter()
        .chain(t.weapons(true))
        .flatten()
    {
        let w = d.weapon(k).ok_or("actual weapon missing")?;
        let h = d.warhead(w.warhead).ok_or("actual WH missing")?;
        out.push((w, h));
    }
    Ok(out)
}
fn fallback_boundary(
    d: &Definitions,
    row: &MappingRow,
    m: &sim::arsenal::ArsenalEntry,
) -> AuditResult<&'static str> {
    if d.rules().is_none() {
        return Err("legacy-only policy cannot be used without actual Rules".into());
    }
    let shots = native_selected(d, row)?;
    match m.role {
        Role::Chrono=>{if !m.teleporter||shots.is_empty()||shots.iter().any(|(_,h)|!h.temporal){return Err("Chrono metadata fallback is not proven shadowed by every actual selected temporal weapon".into());}Ok("Rules present; native temporal WH handled by infantry::fired and Rules chrono movement precedes legacy role fallback")},
        Role::Demolition=>{if shots.is_empty()||shots.iter().any(|(_,h)|!h.ivan_bomb){return Err("Ivan legacy fallback not shadowed by every actual selected IvanBomb WH".into());}Ok("Actual selected IvanBomb WH returns from infantry::fired; interaction plant uses Rules Ivan delay/actual effect or weapon damage")},
        Role::Drone=>{if shots.is_empty()||shots.iter().any(|(_,h)|!h.parasite){return Err("Drone legacy fallback not shadowed by actual parasite WH".into());}Ok("Actual Drone parasite WH handled by vehicles::fired before resolve role fallback; timing comes from actual weapon reload")},
        Role::Squid=>{if shots.is_empty(){return Err("actual Squid selected weapons missing".into());}Ok("Rules-present resolve explicitly excludes legacy Squid role disable; vehicles::fired handles actual Parasite grab, while ordinary Punch uses its actual weapon/warhead and never metadata cooldown")},
        Role::MindControl=>{if shots.is_empty()||!shots.iter().any(|(_,h)|h.mind_control){return Err("native mind-control primary missing".into());}
if !m.deployer{return Err("deployed psi-wave path must be the native Deployer branch".into());}Ok("Native MindControl WH returns from infantry::fired; native Deployer secondary uses actual weapon damage/reload; old role strength is no payload")},
        Role::Commando=>{if (!m.c4&&!m.teleporter)||shots.is_empty()||shots.iter().any(|(w,_)|w.suicide){return Err("legacy commando fallback needs actual C4/Teleporter and non-Suicide selected-weapon premises".into());}Ok("Rules-present infantry::fired returns for actual C4 OR Teleporter after non-Suicide selected weapons; Chrono Commando has Teleporter but not C4. Plant uses Rules max_damage/c4_delay where applicable; Sapper/FakeC4 ordinary impacts stay independent")},
        Role::Radiation=>{if !m.deployer{return Err("legacy radiation Zone still reachable without actual native Deployer trait".into());}Ok("Rules plus Deployer routes commands to infantry::deploy; native weapon RadLevel/radiation sites replace legacy metadata Zone")},
        Role::Repair=>Ok("Rules present disables free Role::Repair aura; native paid docking reads Rules/economy terms"),
        Role::Ifv=>{if !m.gunner{return Err("legacy repair-IFV metadata aura remains active for non-Gunner IFV".into());}Ok("Actual Gunner trait disables legacy IFV aura; native passenger weapon modes supply damage/range/reload")},
        Role::Splash=>Ok("resolve Role::Splash metadata blast is guarded by Rules absence; current Rules presence selects actual warhead spread"),
        Role::Suicide=>{if shots.is_empty()||shots.iter().any(|(w,_)|!w.suicide){return Err("suicide fallback requires actual selected Suicide weapon".into());}Ok("Rules-present suicide path uses actual weapon warhead/damage, not metadata blast terms; death payload remains independently audited")},
        Role::Strike|Role::Carrier=>Err("actual legacy Strike/Carrier dispatcher requires a new native-versus-legacy field contract; current shipped native spawners use Combat".into()),
        Role::Combat|Role::Engineer|Role::Dog|Role::Spy|Role::Flak|Role::Tesla|Role::ChronoMiner|Role::Transport|Role::Mcv|Role::Mirage|Role::Shipyard|Role::Purifier|Role::Cloner|Role::Gap|Role::Satellite|Role::Sensor=>Ok("No selected Rules-present specialist consumer reads these metadata payload/timer terms; native traits/weapons/Rules are compared separately"),
        Role::Chronosphere|Role::Storm|Role::Curtain|Role::Nuke|Role::Paradrop=>{
            if !m.strategic{return Err("legacy strategic fallback remains reachable without actual strategic=true".into());}
            if m.role==Role::Nuke&&d.weapon_kind("nuke_payload").and_then(|k|d.weapon(k)).is_none(){return Err("actual native missile payload weapon missing".into());}
            if m.role==Role::Storm&&d.effect(sim::Effect::Lightning).is_none(){return Err("actual native Lightning effect missing".into());}
            Ok("Actual strategic=true and Rules route launch to superweapons::launch; Nuke uses loaded nuke_payload weapon/warhead, Lightning its effect, Curtain its Rules duration, and area geometry is native square3x3; metadata radius remains active AI grouping policy")
        },
    }
}
// Per-role dispatcher rationale is separate from the native-field Expected proof. These
// explanations grant no source coverage and cannot turn an unresolved native key into Own.
fn role_detail(role: Role) -> &'static str {
    match role {
        Role::Combat => {
            "Combat dispatcher: actual weapon/WH terms drive ordinary firing; no metadata blast/aura/timer case selected"
        }
        Role::Engineer => {
            "Engineer: commands::engineer and infantry::interact use capture/repair/bridge Rules and actual engineer flag; metadata payload/timer is not the capture formula"
        }
        Role::Dog => {
            "Dog: infantry::fired selects the actual parasite WH against infantry; detection uses role and sight, not metadata radius/strength/cooldown"
        }
        Role::Spy => {
            "Spy: infantry disguise/infiltration and reconnaissance use actual MakesDisguise/Spyable/detection traits; these three metadata numbers are not infiltration terms"
        }
        Role::Commando => {
            "Commando: actual C4 or Teleporter trait masks legacy Commando metadata strength times10 after non-Suicide weapons"
        }
        Role::Chrono => {
            "Chrono: Rules-present temporal firing and teleporter movement bypass old disabled-until metadata cooldown; advance legacy teleport explicitly requires Rules absent"
        }
        Role::Flak => {
            "Flak dispatcher: no specialist metadata case; actual Flak weapon projectile/WH supplies spread and damage"
        }
        Role::Tesla => {
            "Tesla: infantry overcharge interaction uses native Overpowerable/actual weapons; ordinary shot does not read these metadata payload terms"
        }
        Role::Demolition => {
            "Demolition: IvanBomb WH return is checked for every selected weapon; native planted bomb delay/effect are separate Rules/effect projections"
        }
        Role::MindControl => {
            "MindControl: native MindControl WH plus native Deployer/actual secondary weapon are checked; no claim that all secondary shots are MindControl"
        }
        Role::Radiation => {
            "Radiation: actual Deployer branch routes before legacy commands Zone metadata allocation; actual weapon RadLevel/radiation Rules are separate"
        }
        Role::Suicide => {
            "Suicide: selected actual Suicide weapons and Rules-present WH payload resolve before legacy blast fallback"
        }
        Role::ChronoMiner => {
            "ChronoMiner: advance refinery-return teleport and harvest use native movement/economy terms; no specialist numeric metadata payload case"
        }
        Role::Transport => {
            "Transport: commands unload/native occupancy consume capacity/size separately; metadata payload/timer is not transport room or movement speed"
        }
        Role::Ifv => {
            "IFV: actual Gunner avoids legacy Engineer repair aura; native13 weapon slots/passenger mode provide combat terms"
        }
        Role::Drone => {
            "Drone: actual Parasite WH selects vehicles::fired; damage and periodic timing use the actual weapon, not old disable timer"
        }
        Role::Mcv => {
            "MCV: commands deploy uses actual deploy target, map footprint and Rules; no metadata payload/timer formula"
        }
        Role::Mirage => {
            "Mirage: reconnaissance checks orders/step and actual DisguiseWhenStill; actual weapon supplies fire, no metadata payload/timer case"
        }
        Role::Splash => {
            "Splash: resolve metadata radial blast exists only when Rules absent; native weapon WH CellSpread is separate"
        }
        Role::Strike => {
            "Strike: unresolved legacy dispatcher is rejected before ownership recognition"
        }
        Role::Carrier => {
            "Carrier: unresolved legacy dispatcher is rejected before ownership recognition"
        }
        Role::Squid => {
            "Squid: actual Parasite WH selects vehicles::fired and weapon timing, not metadata disable timer"
        }
        Role::Shipyard => {
            "Shipyard: paid repair/native docks use actual UnitRepair and economy Rules; no metadata repair aura for this role"
        }
        Role::Purifier => {
            "Purifier: resolve harvested credits checks Role::Purifier and powered state, then loaded Rules multiplier, not metadata strength"
        }
        Role::Repair => {
            "Repair: advance free repair aura requires Rules absent; paid native repair consumes economy terms"
        }
        Role::Cloner => {
            "Cloner: resolve produced infantry checks powered Role::Cloner and native clone terms; metadata payload/timer is not clone health"
        }
        Role::Gap => {
            "Gap: reconnaissance conceal/reshroud actively consumes metadata radius; it must equal native GapRadiusInCells, while cooldown/strength are inactive"
        }
        Role::Satellite => {
            "Satellite: reconnaissance reveal and spy blackout use Role::Satellite/powered, with no metadata radius/strength/timer formula"
        }
        Role::Sensor => {
            "Sensor: reconnaissance and strategic missile warning actively consume metadata radius; native PsychicDetectionRadius/DetectDisguiseRange are independent exact comparisons"
        }
        Role::Chronosphere => {
            "Chronosphere: native Rules square3x3 teleport geometry; cooldown is source RechargeTime and radius remains AI group-selection policy"
        }
        Role::Storm => {
            "Storm: native Lightning effect and Rules storm duration/bolt cadence; cooldown is source RechargeTime and radius remains AI target scoring policy"
        }
        Role::Curtain => {
            "Curtain: native Rules iron_curtain_duration and square3x3 geometry; legacy strength200 is not native duration1000"
        }
        Role::Nuke => {
            "Nuke: native runtime missile uses actual nuke_payload WeaponDef damage600 and NUKE CellSpread10; metadata strength2500/radius6 remain distinct policy values"
        }
        Role::Paradrop => {
            "Paradrop: native Rules plane/infantry/count and delivery behavior; metadata payload numbers are not drop count or native area geometry"
        }
    }
}
fn declared_role(text: &str) -> AuditResult<Role> {
    use Role::*;
    Ok(match text {
        "combat" => Combat,
        "engineer" => Engineer,
        "dog" => Dog,
        "spy" => Spy,
        "commando" => Commando,
        "chrono" => Chrono,
        "flak" => Flak,
        "tesla" => Tesla,
        "demolition" => Demolition,
        "mind_control" => MindControl,
        "radiation" => Radiation,
        "suicide" => Suicide,
        "chrono_miner" => ChronoMiner,
        "transport" => Transport,
        "ifv" => Ifv,
        "drone" => Drone,
        "mcv" => Mcv,
        "mirage" => Mirage,
        "splash" => Splash,
        "strike" => Strike,
        "carrier" => Carrier,
        "squid" => Squid,
        "shipyard" => Shipyard,
        "purifier" => Purifier,
        "repair" => Repair,
        "cloner" => Cloner,
        "gap" => Gap,
        "satellite" => Satellite,
        "sensor" => Sensor,
        "chronosphere" => Chronosphere,
        "storm" => Storm,
        "curtain" => Curtain,
        "nuke" => Nuke,
        "paradrop" => Paradrop,
        _ => return Err(format!("new game-owned role needs a contract {text}")),
    })
}
fn catalogue() -> AuditResult<BTreeMap<String, (Role, u32, u32, i32)>> {
    let text = include_str!("role-policy.tsv");
    let mut rows = text.lines();
    if rows.next()
        != Some(
            "game_id\tgame_dispatch_role\tcurrent_metadata_cooldown\tcurrent_metadata_radius\tcurrent_metadata_strength\tstatus",
        )
    {
        return Err("role policy table header".into());
    }
    let mut out = BTreeMap::new();
    for line in rows {
        let r = line.split('\t').collect::<Vec<_>>();
        if r.len() != 6 {
            return Err("role policy width".into());
        }
        let v = (
            declared_role(r[1])?,
            r[2].parse::<u32>().map_err(|e| e.to_string())?,
            r[3].parse::<u32>().map_err(|e| e.to_string())?,
            r[4].parse::<i32>().map_err(|e| e.to_string())?,
        );
        if out.insert(r[0].into(), v).is_some() {
            return Err("duplicate own role policy ID".into());
        }
    }
    Ok(out)
}
pub(super) fn declare(a: &mut Audit, input: &Inputs) -> AuditResult<()> {
    let r = &input.reference;
    for row in &input.mappings {
        let c = format!("{}@rules:{}", row.game_id, row.section);
        let s = &row.section;
        for (path, names) in [
            ("arsenal.strategic", vec!["SuperWeapon"]),
            ("arsenal.undeploy_delay", vec!["UndeployDelay"]),
        ] {
            super::declare_special_extra(a, r, &c, path, s, &names)?;
        }
        for key in [
            "GapGenerator",
            "SpySat",
            "OrePurifier",
            "Cloning",
            "Engineer",
        ] {
            super::declare_special_extra(
                a,
                r,
                &c,
                &format!("derived.role_capability.{key}"),
                s,
                &[key],
            )?;
        }
        if let Some(sw) = r.rows.get(&SourceKey::new("rules", s, "SuperWeapon")) {
            let _ = sw; /* existing native SW contracts own IsPowered/ShowTimer/cooldown */
        } else {
            for f in ["is_powered", "show_timer"] {
                super::declare_special_extra(a, r, &c, &format!("arsenal.{f}"), s, &[])?;
            }
        }
        let native = ["GapRadiusInCells", "PsychicDetectionRadius"]
            .into_iter()
            .filter(|k| r.rows.contains_key(&SourceKey::new("rules", s, k)))
            .collect::<Vec<_>>();
        if native.len() > 1 {
            return Err(format!("ambiguous two native radius selectors {s}"));
        }
        if let Some(key) = native.first() {
            super::declare_special_extra(a, r, &c, "arsenal.radius", s, &[key])?;
        }
        if r.rows
            .contains_key(&SourceKey::new("rules", s, "DetectDisguiseRange"))
        {
            super::declare_special_extra(
                a,
                r,
                &c,
                "derived.runtime.sensor_disguise_radius",
                s,
                &["DetectDisguiseRange"],
            )?;
        }
    }
    Ok(())
}
pub(super) fn project(a: &mut Audit, input: &Inputs, d: &Definitions) -> AuditResult<()> {
    let policies = catalogue()?;
    let ids = input
        .mappings
        .iter()
        .map(|r| r.game_id.clone())
        .collect::<BTreeSet<_>>();
    if policies.keys().cloned().collect::<BTreeSet<_>>() != ids {
        return Err("own role catalogue and actual full mapping IDs differ".into());
    }
    let r = &input.reference;
    for row in &input.mappings {
        let c = format!("{}@rules:{}", row.game_id, row.section);
        let s = &row.section;
        let m = d.metadata(items::item(d, row)?).ok_or("actual metadata")?;
        let (expected_role, cooldown, radius, strength) =
            policies.get(&row.game_id).ok_or("new role ID")?;
        if m.role != *expected_role {
            return Err(format!(
                "actual game dispatcher role differs {}",
                row.game_id
            ));
        }
        let boundary = format!("{}; {}", fallback_boundary(d, row, m)?, role_detail(m.role));
        a.game_owned(WitnessId::new(&c,"arsenal.role"),format!("{:?}",m.role),format!("Explicit game dispatcher taxonomy, not a native Role field; {boundary}"),"special::{commands,infantry,vehicles,resolve,advance,reconnaissance,superweapons}; curated native traits/weapons audited independently".into())?;
        let sw = optional(r, s, "SuperWeapon", "none")?;
        let present = sw.value != "none";
        check_int(
            a,
            &c,
            "arsenal.strategic",
            &row.game_id,
            "strategic",
            i128::from(m.strategic),
            sw.clone().map(|v| i128::from(v != "none")),
            "SuperWeapon presence, actual strategic dispatch".into(),
        )?;
        if m.role.superweapon() != present {
            return Err(format!(
                "actual role and native SuperWeapon link disagree {s}"
            ));
        }
        if !present {
            for (f, v) in [("is_powered", m.is_powered), ("show_timer", m.show_timer)] {
                check_int(
                    a,
                    &c,
                    &format!("arsenal.{f}"),
                    &row.game_id,
                    f,
                    i128::from(v),
                    Expected::literal(0),
                    "No native SuperWeapon link: explicit false strategic-only sentinel".into(),
                )?;
            }
            if m.cooldown != *cooldown {
                return Err("own inactive cooldown changed without declared policy update".into());
            }
            a.game_owned(WitnessId::new(&c,"arsenal.cooldown"),m.cooldown.to_string(),format!("Game-owned inactive/legacy metadata timer on current Rules-present role: {boundary}"),"special resolve/commands/advance dispatch; actual loaded native weapon/Deployer/Gunner premises checked above".into())?;
        }
        let delay = optional(r, s, "UndeployDelay", "0")?;
        let display = delay.value.clone();
        let expected = if delay.value == "0" {
            Expected::literal(0).because(&delay)
        } else {
            converted(delay, Conversion::Frames)?
        };
        check_int(
            a,
            &c,
            "arsenal.undeploy_delay",
            &row.game_id,
            "undeploy_delay",
            i128::from(m.undeploy_delay),
            expected,
            display,
        )?;
        let native = ["GapRadiusInCells", "PsychicDetectionRadius"]
            .into_iter()
            .filter(|k| r.rows.contains_key(&SourceKey::new("rules", s, k)))
            .collect::<Vec<_>>();
        if let Some(key) = native.first() {
            let raw = rule(r, s, key)?;
            let display = raw.value.clone();
            check_int(
                a,
                &c,
                "arsenal.radius",
                &row.game_id,
                if *key == "GapRadiusInCells" {
                    "gap_radius"
                } else {
                    "sensor_radius"
                },
                i128::from(m.radius),
                converted(raw, Conversion::Int)?,
                display,
            )?;
        } else {
            if m.radius != *radius {
                return Err("own metadata radius changed without declared policy update".into());
            }
            a.game_owned(WitnessId::new(&c,"arsenal.radius"),m.radius.to_string(),if present{format!("Active game-owned strategic AI grouping/scoring radius{radius}; physical Chronosphere/Curtain geometry is square3x3, native Nuke spread10 and native paradrop distance come from Rules/effect paths, not this radius") }else{format!("Inactive/legacy metadata radius on current Rules-present role: {boundary}")},"special superweapons square/covered/drop and effect payload; ai/strategy strategic target grouping; native Gap/Sensor radii handled separately".into())?;
        }
        if r.rows
            .contains_key(&SourceKey::new("rules", s, "DetectDisguiseRange"))
        {
            if m.role != Role::Sensor || !m.detect_disguise {
                return Err(
                    "native DetectDisguiseRange has no proven actual sensor-radius consumer".into(),
                );
            }
            let raw = rule(r, s, "DetectDisguiseRange")?;
            let display = raw.value.clone();
            check_int(
                a,
                &c,
                "derived.runtime.sensor_disguise_radius",
                &row.game_id,
                "sensor_disguise_radius",
                i128::from(m.radius),
                converted(raw, Conversion::Int)?,
                display,
            )?;
        }
        if m.strength != *strength {
            return Err(
                "own legacy metadata strength changed without declared policy update".into(),
            );
        }
        a.game_owned(WitnessId::new(&c,"arsenal.strength"),m.strength.to_string(),format!("Legacy/inactive metadata strength{strength} on current Rules-present role: {boundary}; it is not native Curtain duration1000, Nuke payload600 or radiation level500"),"superweapons::advance resolves Nuke through nuke_payload weapon; bolt resolves Lightning effect; Curtain uses Rules iron_curtain_duration; special fired/plant paths checked above".into())?;
        for (key, role) in [
            ("GapGenerator", Role::Gap),
            ("SpySat", Role::Satellite),
            ("OrePurifier", Role::Purifier),
            ("Cloning", Role::Cloner),
            ("Engineer", Role::Engineer),
        ] {
            let raw = optional(r, s, key, "no")?;
            let display = raw.value.clone();
            check_int(
                a,
                &c,
                &format!("derived.role_capability.{key}"),
                &row.game_id,
                &format!("role_capability.{key}"),
                i128::from(m.role == role),
                converted(raw, Conversion::Flag)?,
                display,
            )?;
        }
    }
    Ok(())
}
