//! Historical preparation note: UNCOMPILED/UNRUN typed current-policy observations. NO source Expected/witness credit.
use super::*;
pub(super) fn ai_snapshot(ai: &sim::AiDefinitions, d: &Definitions) -> AuditResult<String> {
    let mut out = BTreeMap::<String, String>::new();
    out.insert("rules_difficulty".into(), ai.rules_difficulty.to_string());
    let p = ai.policy;
    for (k, v) in [
        ("garrison_quota", p.garrison_quota),
        ("garrison_radius", p.garrison_radius),
        ("depot_radius", p.depot_radius),
        ("naval_rescan", p.naval_rescan),
        ("naval_detector_quota", p.naval_detector_quota),
        ("artillery_min_range", p.artillery_min_range),
        ("artillery_standoff", p.artillery_standoff),
        ("paradrop_offset", p.paradrop_offset),
        ("chrono_min_offset", p.chrono_min_offset),
        ("chrono_max_offset", p.chrono_max_offset),
    ] {
        out.insert(format!("policy.{k}"), v.to_string());
    }
    let b = |k| {
        d.building(k)
            .map(|v| v.id.clone())
            .ok_or_else(|| "legacy actual AI building kind absent".to_owned())
    };
    let u = |k| {
        d.unit(k)
            .map(|v| v.id.clone())
            .ok_or_else(|| "legacy actual AI unit kind absent".to_owned())
    };
    out.insert("plans.len".into(), ai.plans.len().to_string());
    for (i, p) in ai.plans.iter().enumerate() {
        let pre = format!("plans[{i}]");
        out.insert(
            format!("{pre}.side"),
            p.side.map(|v| v.id().to_owned()).unwrap_or_default(),
        );
        out.insert(
            format!("{pre}.country"),
            p.country
                .map(|v| {
                    d.rules()
                        .and_then(|r| r.country(v))
                        .map(|v| v.id.clone())
                        .ok_or_else(|| "legacy actual AI country absent".to_owned())
                })
                .transpose()?
                .unwrap_or_default(),
        );
        for (k, v) in [
            (
                "build_order",
                p.build_order
                    .iter()
                    .copied()
                    .map(&b)
                    .collect::<AuditResult<Vec<_>>>()?
                    .join(","),
            ),
            (
                "defences",
                p.defences
                    .iter()
                    .copied()
                    .map(&b)
                    .collect::<AuditResult<Vec<_>>>()?
                    .join(","),
            ),
            (
                "support",
                p.support
                    .iter()
                    .copied()
                    .map(&u)
                    .collect::<AuditResult<Vec<_>>>()?
                    .join(","),
            ),
            (
                "army",
                p.army
                    .iter()
                    .copied()
                    .map(&u)
                    .collect::<AuditResult<Vec<_>>>()?
                    .join(","),
            ),
            ("power", b(p.power)?),
            ("harvester", u(p.harvester)?),
            (
                "tier_reserves",
                p.tier_reserves
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            ),
            ("advanced_tech_level", p.advanced_tech_level.to_string()),
        ] {
            out.insert(format!("{pre}.{k}"), v);
        }
    }
    out.insert("difficulties.len".into(), ai.difficulties.len().to_string());
    for (i, p) in ai.difficulties.iter().enumerate() {
        let pre = format!("difficulties[{i}]");
        out.insert(format!("{pre}.id"), p.id.clone());
        for (k, v) in [
            ("think_interval", p.think_interval),
            ("harvesters_per_refinery", p.harvesters_per_refinery),
            ("max_army", p.max_army),
            ("wave_size", p.wave_size),
            ("first_wave", p.first_wave),
            ("wave_patience", p.wave_patience),
            ("defence_radius", p.defence_radius),
        ] {
            out.insert(format!("{pre}.{k}"), v.to_string());
        }
    }
    Ok(out
        .into_iter()
        .map(|(k, v)| format!("{k}\t{v}\n"))
        .collect())
}
pub(super) fn observe(d: &Definitions, id: &str, f: &str) -> AuditResult<String> {
    if id == "global" {
        let o = sim::MatchOptions::default();
        return match f {
            "options.fog_of_war" => Ok(o.fog_of_war.to_string()),
            "options.free_for_all" => Ok(o.free_for_all.to_string()),
            _ => Err("unknown actual option".into()),
        };
    }
    let unit = d.unit_kind(id);
    let building = d.building_kind(id);
    let item = if let Some(k) = unit {
        sim::ProductionItem::Unit(k)
    } else {
        sim::ProductionItem::Building(building.ok_or("legacy actual item absent")?)
    };
    if f == "unit.speed" {
        return unit
            .and_then(|k| d.unit(k))
            .map(|u| u.speed.to_string())
            .ok_or_else(|| "legacy typed speed absent".into());
    }
    if matches!(f, "combat.weapons" | "combat.elite_weapons") {
        let t = match item {
            sim::ProductionItem::Unit(k) => d.unit_combat(k),
            sim::ProductionItem::Building(k) => d.building_combat(k),
        }
        .ok_or("legacy actual combat absent")?;
        return t
            .weapons(f == "combat.elite_weapons")
            .into_iter()
            .flatten()
            .map(|k| {
                d.weapon(k)
                    .map(|v| v.id.clone())
                    .ok_or_else(|| "legacy actual selected weapon absent".into())
            })
            .collect::<AuditResult<Vec<_>>>()
            .map(|v| v.join(","));
    }
    let m = d.metadata(item).ok_or("legacy actual metadata absent")?;
    match f {
        "metadata.role" => Ok(match m.role {
            sim::Role::MindControl => "mind_control".into(),
            sim::Role::ChronoMiner => "chrono_miner".into(),
            other => format!("{other:?}").to_ascii_lowercase(),
        }),
        "metadata.domain" => Ok(format!("{:?}", m.domain).to_ascii_lowercase()),
        "metadata.infantry" => Ok(m.infantry.to_string()),
        "metadata.ammo" => Ok(m.ammo.to_string()),
        "metadata.airport_bound" => Ok(m.airport_bound.to_string()),
        "metadata.docks" => Ok(m.docks.to_string()),
        "metadata.missile" => Ok(m
            .missile
            .map(|v| format!("{v:?}").to_ascii_lowercase())
            .unwrap_or_else(|| "none".into())),
        "metadata.spawns" => m
            .spawns
            .map(|k| {
                d.unit(k)
                    .map(|v| v.id.clone())
                    .ok_or_else(|| "legacy actual spawn absent".into())
            })
            .transpose()
            .map(|v| v.unwrap_or_else(|| "none".into())),
        _ => Err(format!(
            "unknown REAL legacy current-model policy field {f}"
        )),
    }
}
