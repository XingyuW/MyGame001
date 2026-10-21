//! UNCOMPILED/UNRUN real typed policy observations. These are NOT source Expected/native
//! witnesses. No getter is invented for an unparsed source flag. No coverage is recorded.
use super::*;
fn item(d: &Definitions, id: &str) -> AuditResult<sim::ProductionItem> {
    if let Some(k) = d.unit_kind(id) {
        Ok(sim::ProductionItem::Unit(k))
    } else {
        d.building_kind(id)
            .map(sim::ProductionItem::Building)
            .ok_or_else(|| format!("model162 real item absent {id}"))
    }
}
fn weapon_list(d: &Definitions, id: &str, elite: bool) -> AuditResult<String> {
    let k = d.unit_kind(id).ok_or("model162 unit absent")?;
    let t = d.unit_combat(k).ok_or("model162 unit combat absent")?;
    t.weapons(elite)
        .into_iter()
        .flatten()
        .map(|k| {
            d.weapon(k)
                .map(|w| w.id.clone())
                .ok_or_else(|| "invalid model162 actual weapon".into())
        })
        .collect::<AuditResult<Vec<_>>>()
        .map(|v| v.join(","))
}
pub(super) fn observe(d: &Definitions, id: &str, f: &str) -> AuditResult<String> {
    if id == "global" {
        let o = sim::MatchOptions::default();
        return match f {
            "options.free_for_all" => Ok(o.free_for_all.to_string()),
            "options.fog_of_war" => Ok(o.fog_of_war.to_string()),
            "options.short_game" => Ok(o.short_game.to_string()),
            "max_waypoints" => Ok(sim::MAX_WAYPOINTS.to_string()),
            "ticks_per_second" => Ok(sim::TICKS_PER_SECOND.to_string()),
            _ => Err("unknown model162 real option/constant".into()),
        };
    }
    if id == "rules" {
        let r = d.rules().ok_or("model162 Rules absent")?;
        let v = match f {
            "combat.cloak_delay" => r.combat.cloak_delay,
            "combat.chrono_distance_factor" => r.combat.chrono_distance_factor,
            "combat.chrono_minimum_delay" => r.combat.chrono_minimum_delay,
            "superweapons.paradrop_radius" => r.superweapons.paradrop_radius,
            "crates.minimum" => r.crates.minimum,
            "crates.maximum" => r.crates.maximum,
            "crates.radius" => r.crates.radius,
            "crates.free_mcv" => r.crates.free_mcv,
            "economy.ore_value" => r.economy.ore_value,
            "economy.gem_value" => r.economy.gem_value,
            "economy.unit_repair_interval" => r.economy.unit_repair_interval,
            "economy.repair_step" => r.economy.repair_step,
            _ => return Err("unknown model162 real scalar".into()),
        };
        return Ok(v.to_string());
    }
    if f == "weapons" || f == "elite_weapons" {
        return weapon_list(d, id, f == "elite_weapons");
    }
    if f == "speed" {
        return d
            .unit_kind(id)
            .and_then(|k| d.unit(k))
            .map(|u| u.speed.to_string())
            .ok_or_else(|| "model162 actual speed missing".into());
    }
    let m = d
        .metadata(item(d, id)?)
        .ok_or("model162 actual metadata missing")?;
    match f {
        "role" => Ok(format!("{:?}", m.role)),
        "domain" => Ok(format!("{:?}", m.domain)),
        "label" => Ok(m.label.clone()),
        "art" => Ok(m.art.clone()),
        "ammo" => Ok(m.ammo.to_string()),
        "airport_bound" => Ok(m.airport_bound.to_string()),
        "guard_range" => Ok(m.guard_range.to_string()),
        "infantry" => Ok(m.infantry.to_string()),
        "disguise_when_still" => Ok(m.disguise_when_still.to_string()),
        "cloakable" => Ok(m.cloakable.to_string()),
        "sensors_sight" => Ok(m.sensors_sight.to_string()),
        "self_healing" => Ok(m.self_healing.to_string()),
        "crusher" => Ok(m.crusher.to_string()),
        "crushable" => Ok(m.crushable.to_string()),
        "docks" => Ok(m.docks.to_string()),
        "capacity" => Ok(m.capacity.to_string()),
        "spawns" => m
            .spawns
            .map(|k| {
                d.unit(k)
                    .map(|u| u.id.clone())
                    .ok_or_else(|| "invalid real spawn kind".into())
            })
            .transpose()
            .map(|v| v.unwrap_or_else(|| "none".into())),
        "missile" => Ok(m
            .missile
            .map(|v| format!("{v:?}").to_ascii_lowercase())
            .unwrap_or_else(|| "none".into())),
        "explodes" => Ok(m.explodes.to_string()),
        "death_weapon" => m
            .death_weapon
            .map(|k| {
                d.weapon(k)
                    .map(|w| w.id.clone())
                    .ok_or_else(|| "invalid actual death weapon".into())
            })
            .transpose()
            .map(|v| v.unwrap_or_else(|| "none".into())),
        "ifv_mode" => Ok(m.ifv_mode.to_string()),
        "radius" => Ok(m.radius.to_string()),
        "powered" => Ok(m.powered.to_string()),
        "deployer" => Ok(m.deployer.to_string()),
        "base_normal" => Ok(m.base_normal.to_string()),
        "adjacent" => Ok(m.adjacent.to_string()),
        "capturable" => Ok(m.capturable.to_string()),
        "crewed" => Ok(m.crewed.to_string()),
        "radar" => Ok(m.radar.to_string()),
        "teleporter" => Ok(m.teleporter.to_string()),
        "unit_repair" => Ok(m.unit_repair.to_string()),
        "vehicle_docks" => Ok(m.vehicle_docks.to_string()),
        "strategic" => Ok(m.strategic.to_string()),
        "cooldown" => Ok(m.cooldown.to_string()),
        "detect_disguise" => Ok(m.detect_disguise.to_string()),
        "can_be_occupied" => Ok(m.can_be_occupied.to_string()),
        "bombable" => Ok(m.bombable.to_string()),
        "c4" => Ok(m.c4.to_string()),
        _ => Err(format!("unknown real model162 metadata field {f}")),
    }
}
