//! Historical preparation note: UNVALIDATED / PENDING actual-value adapters. NO compilation or test execution performed.
//! Conversion rows copied from the independent existing audit, never from generator output.
//! Must integrate with registered actual-field requirements before root releases canonical RA9.
use super::actual::{check_int, optional, positive_round, rule, scaled};
use super::{Audit, AuditResult, Expected, Reference, SourceKey, WitnessId};
use sim::{Definitions, Rules};
use std::collections::BTreeMap;
#[derive(Clone, Copy)]
pub(super) enum Conversion {
    Int,
    Percent,
    Permille,
    PercentAsPermille,
    Minutes,
    Frames,
    Cells,
    Flag,
}
pub(super) fn converted(
    value: Expected<String>,
    conversion: Conversion,
) -> AuditResult<Expected<i128>> {
    match conversion {
        Conversion::Int => scaled(value, 1, 1),
        Conversion::Percent => {
            let factor = if value.value.ends_with('%') { 1 } else { 100 };
            scaled(value, factor, 1)
        }
        Conversion::Permille => scaled(value, 1000, 1),
        Conversion::PercentAsPermille => scaled(value, 10, 1),
        Conversion::Minutes => scaled(value, 1200, 1),
        Conversion::Frames => Ok(scaled(value, 4, 3)?.map(|v| v.max(1))),
        Conversion::Cells => scaled(value, 256, 1),
        Conversion::Flag => value.try_map(|s| match s.to_ascii_lowercase().as_str() {
            "yes" | "true" => Ok(1),
            "no" | "false" => Ok(0),
            _ => Err(format!("not a source flag {s:?}")),
        }),
    }
}
pub(super) const SCALAR_ROWS: &[(&str, &str, &str, &str, Conversion)] = {
    use Conversion::*;
    &[
        (
            "production",
            "build_ticks_per_1000",
            "General",
            "BuildSpeed",
            Minutes,
        ),
        (
            "production",
            "multiple_factory_pm",
            "General",
            "MultipleFactory",
            Permille,
        ),
        (
            "production",
            "wall_build_time_pm",
            "General",
            "WallBuildSpeedCoefficient",
            Permille,
        ),
        (
            "production",
            "low_power_penalty_pm",
            "General",
            "LowPowerPenaltyModifier",
            Permille,
        ),
        (
            "production",
            "min_low_power_speed_pm",
            "General",
            "MinLowPowerProductionSpeed",
            Permille,
        ),
        (
            "production",
            "max_low_power_speed_pm",
            "General",
            "MaxLowPowerProductionSpeed",
            Permille,
        ),
        (
            "production",
            "max_queued_objects",
            "General",
            "MaximumQueuedObjects",
            Int,
        ),
        (
            "economy",
            "refund_percent",
            "General",
            "RefundPercent",
            Percent,
        ),
        (
            "economy",
            "repair_percent",
            "General",
            "RepairPercent",
            Percent,
        ),
        ("economy", "repair_step", "General", "RepairStep", Int),
        (
            "economy",
            "repair_interval",
            "General",
            "RepairRate",
            Minutes,
        ),
        (
            "economy",
            "unit_repair_interval",
            "General",
            "URepairRate",
            Minutes,
        ),
        (
            "economy",
            "infantry_repair_step",
            "General",
            "IRepairStep",
            Int,
        ),
        (
            "economy",
            "infantry_repair_interval",
            "General",
            "IRepairRate",
            Minutes,
        ),
        (
            "economy",
            "reload_interval",
            "General",
            "ReloadRate",
            Minutes,
        ),
        (
            "economy",
            "purifier_bonus_percent",
            "General",
            "PurifierBonus",
            Percent,
        ),
        ("economy", "ore_value", "Riparius", "Value", Int),
        ("economy", "gem_value", "Cruentus", "Value", Int),
        (
            "economy",
            "ore_growth_interval",
            "Riparius",
            "Growth",
            Frames,
        ),
        (
            "economy",
            "ore_growth_pm",
            "Riparius",
            "GrowthPercentage",
            Permille,
        ),
        (
            "economy",
            "ore_spread_interval",
            "Riparius",
            "Spread",
            Frames,
        ),
        (
            "economy",
            "ore_spread_pm",
            "Riparius",
            "SpreadPercentage",
            Permille,
        ),
        (
            "economy",
            "harvesters_per_refinery",
            "General",
            "HarvestersPerRefinery",
            Int,
        ),
        (
            "veterancy",
            "veteran_ratio_pm",
            "General",
            "VeteranRatio",
            Permille,
        ),
        ("veterancy", "veteran_cap", "General", "VeteranCap", Int),
        (
            "veterancy",
            "combat_pm",
            "General",
            "VeteranCombat",
            Permille,
        ),
        ("veterancy", "speed_pm", "General", "VeteranSpeed", Permille),
        ("veterancy", "sight_pm", "General", "VeteranSight", Permille),
        ("veterancy", "armor_pm", "General", "VeteranArmor", Permille),
        ("veterancy", "rof_pm", "General", "VeteranROF", Permille),
        (
            "veterancy",
            "initial_veteran",
            "General",
            "InitialVeteran",
            Flag,
        ),
        (
            "crew",
            "crew_escape_percent",
            "General",
            "CrewEscape",
            Percent,
        ),
        (
            "crew",
            "survivor_rate_pm",
            "General",
            "SurvivorRate",
            Permille,
        ),
        ("combat", "max_damage", "CombatDamage", "MaxDamage", Int),
        ("combat", "min_damage", "CombatDamage", "MinDamage", Int),
        (
            "combat",
            "condition_yellow_percent",
            "AudioVisual",
            "ConditionYellow",
            Percent,
        ),
        (
            "combat",
            "condition_red_percent",
            "AudioVisual",
            "ConditionRed",
            Percent,
        ),
        (
            "combat",
            "ballistic_scatter",
            "CombatDamage",
            "BallisticScatter",
            Cells,
        ),
        (
            "combat",
            "homing_scatter",
            "CombatDamage",
            "HomingScatter",
            Cells,
        ),
        (
            "superweapons",
            "chrono_freeze",
            "General",
            "ChronoDelay",
            Frames,
        ),
        ("combat", "c4_delay", "CombatDamage", "C4Delay", Minutes),
        ("combat", "ivan_damage", "CombatDamage", "IvanDamage", Int),
        (
            "combat",
            "ivan_delay",
            "CombatDamage",
            "IvanTimedDelay",
            Frames,
        ),
        (
            "combat",
            "v3_rocket_damage",
            "General",
            "V3RocketDamage",
            Int,
        ),
        (
            "combat",
            "v3_rocket_elite_damage",
            "General",
            "V3RocketEliteDamage",
            Int,
        ),
        ("combat", "dmisl_damage", "General", "DMislDamage", Int),
        (
            "combat",
            "dmisl_elite_damage",
            "General",
            "DMislEliteDamage",
            Int,
        ),
        (
            "combat",
            "bridge_strength",
            "CombatDamage",
            "BridgeStrength",
            Int,
        ),
        ("combat", "cloak_delay", "General", "CloakDelay", Minutes),
        (
            "combat",
            "chrono_distance_factor",
            "General",
            "ChronoDistanceFactor",
            Int,
        ),
        (
            "combat",
            "chrono_minimum_delay",
            "General",
            "ChronoMinimumDelay",
            Frames,
        ),
        (
            "combat",
            "spy_power_blackout",
            "General",
            "SpyPowerBlackout",
            Frames,
        ),
        (
            "combat",
            "spy_money_steal_percent",
            "General",
            "SpyMoneyStealPercent",
            Percent,
        ),
        ("combat", "tree_strength", "General", "TreeStrength", Int),
        (
            "radiation",
            "application_delay",
            "Radiation",
            "RadApplicationDelay",
            Frames,
        ),
        (
            "radiation",
            "level_delay",
            "Radiation",
            "RadLevelDelay",
            Frames,
        ),
        (
            "radiation",
            "level_factor_pm",
            "Radiation",
            "RadLevelFactor",
            Permille,
        ),
        ("radiation", "level_max", "Radiation", "RadLevelMax", Int),
        (
            "radiation",
            "duration_multiple_pm",
            "Radiation",
            "RadDurationMultiple",
            Permille,
        ),
        (
            "superweapons",
            "nuclear_recharge",
            "MultiSpecial",
            "RechargeTime",
            Minutes,
        ),
        (
            "superweapons",
            "lightning_recharge",
            "LightningStormSpecial",
            "RechargeTime",
            Minutes,
        ),
        (
            "superweapons",
            "iron_curtain_recharge",
            "IronCurtainSpecial",
            "RechargeTime",
            Minutes,
        ),
        (
            "superweapons",
            "chronosphere_recharge",
            "ChronoSphereSpecial",
            "RechargeTime",
            Minutes,
        ),
        (
            "superweapons",
            "paradrop_recharge",
            "ParaDropSpecial",
            "RechargeTime",
            Minutes,
        ),
        (
            "superweapons",
            "american_paradrop_recharge",
            "AmericanParaDropSpecial",
            "RechargeTime",
            Minutes,
        ),
        (
            "superweapons",
            "iron_curtain_duration",
            "CombatDamage",
            "IronCurtainDuration",
            Frames,
        ),
        (
            "superweapons",
            "lightning_deferment",
            "General",
            "LightningDeferment",
            Frames,
        ),
        (
            "superweapons",
            "lightning_duration",
            "General",
            "LightningStormDuration",
            Frames,
        ),
        (
            "superweapons",
            "lightning_damage",
            "General",
            "LightningDamage",
            Int,
        ),
        (
            "superweapons",
            "lightning_hit_delay",
            "General",
            "LightningHitDelay",
            Frames,
        ),
        (
            "superweapons",
            "lightning_scatter_delay",
            "General",
            "LightningScatterDelay",
            Frames,
        ),
        (
            "superweapons",
            "lightning_cell_spread",
            "General",
            "LightningCellSpread",
            Int,
        ),
        (
            "superweapons",
            "lightning_separation",
            "General",
            "LightningSeparation",
            Int,
        ),
        (
            "superweapons",
            "american_paradrop_count",
            "General",
            "AmerParaDropNum",
            Int,
        ),
        (
            "superweapons",
            "paradrop_radius",
            "General",
            "ParadropRadius",
            Int,
        ),
        (
            "prism",
            "support_delay",
            "General",
            "PrismSupportDelay",
            Frames,
        ),
        (
            "prism",
            "support_duration",
            "General",
            "PrismSupportDuration",
            Frames,
        ),
        ("prism", "support_max", "General", "PrismSupportMax", Int),
        (
            "prism",
            "support_modifier_pm",
            "General",
            "PrismSupportModifier",
            PercentAsPermille,
        ),
        (
            "prism",
            "support_height",
            "General",
            "PrismSupportHeight",
            Int,
        ),
        ("crates", "maximum", "CrateRules", "CrateMaximum", Int),
        ("crates", "minimum", "CrateRules", "CrateMinimum", Int),
        ("crates", "radius", "CrateRules", "CrateRadius", Cells),
        ("crates", "regen", "CrateRules", "CrateRegen", Minutes),
        ("crates", "free_mcv", "CrateRules", "FreeMCV", Flag),
        (
            "dialog",
            "min_credits",
            "MultiplayerDialogSettings",
            "MinMoney",
            Int,
        ),
        (
            "dialog",
            "credits",
            "MultiplayerDialogSettings",
            "Money",
            Int,
        ),
        (
            "dialog",
            "max_credits",
            "MultiplayerDialogSettings",
            "MaxMoney",
            Int,
        ),
        (
            "dialog",
            "credits_step",
            "MultiplayerDialogSettings",
            "MoneyIncrement",
            Int,
        ),
        (
            "dialog",
            "min_unit_count",
            "MultiplayerDialogSettings",
            "MinUnitCount",
            Int,
        ),
        (
            "dialog",
            "unit_count",
            "MultiplayerDialogSettings",
            "UnitCount",
            Int,
        ),
        (
            "dialog",
            "max_unit_count",
            "MultiplayerDialogSettings",
            "MaxUnitCount",
            Int,
        ),
        (
            "dialog",
            "tech_level",
            "MultiplayerDialogSettings",
            "TechLevel",
            Int,
        ),
        (
            "dialog",
            "bases",
            "MultiplayerDialogSettings",
            "Bases",
            Flag,
        ),
        (
            "dialog",
            "short_game",
            "MultiplayerDialogSettings",
            "ShortGame",
            Flag,
        ),
        (
            "dialog",
            "crates",
            "MultiplayerDialogSettings",
            "Crates",
            Flag,
        ),
        (
            "dialog",
            "mcv_redeploys",
            "MultiplayerDialogSettings",
            "MCVRedeploys",
            Flag,
        ),
        (
            "dialog",
            "bridge_destruction",
            "MultiplayerDialogSettings",
            "BridgeDestruction",
            Flag,
        ),
        (
            "dialog",
            "fog_of_war",
            "MultiplayerDialogSettings",
            "FogOfWar",
            Flag,
        ),
        (
            "dialog",
            "shroud",
            "MultiplayerDialogSettings",
            "Shroud",
            Flag,
        ),
        ("dialog", "max_players", "Maximums", "Players", Int),
    ]
};

pub(super) fn groups(r: &Rules) -> Vec<(&'static str, Vec<(&'static str, i32)>)> {
    vec![
        ("production", r.production.fields()),
        ("economy", r.economy.fields()),
        ("veterancy", r.veterancy.fields()),
        ("crew", r.crew.fields()),
        ("combat", r.combat.fields()),
        ("radiation", r.radiation.fields()),
        ("superweapons", r.superweapons.fields()),
        ("prism", r.prism.fields()),
        ("crates", r.crates.fields()),
        ("dialog", r.dialog.fields()),
    ]
}
pub(super) fn scalars(a: &mut Audit, r: &Reference, d: &Definitions) -> AuditResult<()> {
    let rules = d
        .rules()
        .ok_or_else(|| "shipped rules missing".to_owned())?;
    let mut used = std::collections::BTreeSet::new();
    for (group, fields) in groups(rules) {
        for (field, actual) in fields {
            let path = format!("rules.{group}.{field}");
            let own = match (group, field) {
                ("superweapons", "nuclear_flight") => Some((
                    200,
                    "Fixed launch-to-impact200 ticks is game-owned; R80 timed nuclear payload, not a native NukeUp/NukeDown flight simulation.",
                )),
                ("economy", "ore_max_bails") => Some((
                    12,
                    "Twelve map ore-density stages; R58 declared game-owned bound.",
                )),
                ("economy", "ore_spread_above") => Some((
                    6,
                    "R58 declared half-density spread threshold; not a native reference value.",
                )),
                _ => None,
            };
            if let Some((expected, reason)) = own {
                if actual != expected {
                    return Err(format!("game-owned {path}: {actual} != {expected}"));
                }
                a.game_owned(
                    WitnessId::new("rules", &path),
                    actual.to_string(),
                    reason.into(),
                    "rules.rs fields; DECISIONS R58/R80".into(),
                )?;
                continue;
            }
            let candidates = SCALAR_ROWS
                .iter()
                .enumerate()
                .filter(|(_, row)| row.0 == group && row.1 == field)
                .collect::<Vec<_>>();
            if candidates.len() != 1 {
                return Err(format!(
                    "{path}: expected exactly one independent conversion"
                ));
            }
            let (index, (_, _, section, key, c)) = candidates[0];
            used.insert(index);
            let source = rule(r, section, key)?;
            let display = source.value.clone();
            check_int(
                a,
                "rules",
                &path,
                "rules",
                &format!("{group}.{field}"),
                i128::from(actual),
                converted(source, *c)?,
                display,
            )?;
        }
    }
    if used.len() != SCALAR_ROWS.len() {
        return Err("stale scalar conversion row".into());
    }
    Ok(())
}
pub(super) fn checked_product(values: &[i128]) -> AuditResult<i128> {
    values.iter().try_fold(1i128, |a, b| {
        a.checked_mul(*b)
            .ok_or_else(|| "exact arithmetic overflow".into())
    })
}
pub(super) fn floor_root(n: i128) -> AuditResult<i128> {
    if n < 0 {
        return Err("negative root".into());
    }
    let (mut lo, mut hi) = (0i128, n.min(1_000_000_000) + 1);
    if n > 1_000_000_000i128.pow(2) {
        return Err("root input beyond explicit audit bound".into());
    }
    while lo + 1 < hi {
        let mid = lo + (hi - lo) / 2;
        if mid * mid <= n { lo = mid } else { hi = mid }
    }
    Ok(lo)
}
pub(super) fn ticks_from_leptons(v: i128) -> AuditResult<i128> {
    positive_round(checked_product(&[v, 3])?, 4).map(|n| n.max(1))
}

// Derived projectile expression tracks selectors, source link and ONLY the taken branch.
// Untaken Speed/Range/Gravity sources still require another witness or exact non-use reason.
pub(super) fn projectile_speed(r: &Reference, section: &str) -> AuditResult<Expected<i128>> {
    let projectile = rule(r, section, "Projectile")?;
    let p = projectile.value.clone();
    let inviso = converted(optional(r, &p, "Inviso", "no")?, Conversion::Flag)?;
    if inviso.value == 1 {
        // Actual OR short circuit: LimboLaunch and Arcing are not read or credited here.
        return Ok(Expected::literal(0).because(&projectile).because(&inviso));
    }
    let limbo = converted(optional(r, section, "LimboLaunch", "no")?, Conversion::Flag)?;
    if limbo.value == 1 {
        return Ok(Expected::literal(0)
            .because(&projectile)
            .because(&inviso)
            .because(&limbo));
    }
    let arcing = converted(optional(r, &p, "Arcing", "no")?, Conversion::Flag)?;
    let answer = if arcing.value == 1 {
        converted(rule(r, section, "Range")?, Conversion::Cells)?
            .zip(converted(
                rule(r, "AudioVisual", "Gravity")?,
                Conversion::Int,
            )?)
            .try_map(|(range, gravity)| {
                checked_product(&[range, gravity, 6])
                    .and_then(|n| floor_root(n / 5))
                    .and_then(ticks_from_leptons)
            })?
    } else {
        converted(rule(r, section, "Speed")?, Conversion::Int)?.try_map(|speed| {
            checked_product(&[speed, 256]).and_then(|n| ticks_from_leptons((n / 100).min(255)))
        })?
    };
    Ok(answer
        .because(&projectile)
        .because(&inviso)
        .because(&limbo)
        .because(&arcing))
}

pub(super) fn weapon(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    game_id: &str,
    section: &str,
    warhead_ids: &BTreeMap<String, String>,
) -> AuditResult<()> {
    let w = d
        .weapon_kind(game_id)
        .and_then(|k| d.weapon(k))
        .ok_or_else(|| format!("missing weapon {game_id}"))?;
    let id = format!("weapon.{game_id}");
    let consumer = format!("{id}@rules:{section}");
    for (field, actual, key, default, c) in [
        (
            "damage",
            i128::from(w.damage),
            "Damage",
            "",
            Conversion::Int,
        ),
        (
            "range",
            i128::from(w.range),
            "Range",
            if section == "OilExplosion" { "0" } else { "" },
            Conversion::Cells,
        ),
        (
            "min_range",
            i128::from(w.min_range),
            "MinimumRange",
            "0",
            Conversion::Cells,
        ),
        (
            "reload",
            i128::from(w.reload),
            "ROF",
            if matches!(section, "NukePayload" | "OilExplosion") {
                "0"
            } else {
                ""
            },
            Conversion::Frames,
        ),
        ("burst", i128::from(w.burst), "Burst", "1", Conversion::Int),
        (
            "rad_level",
            i128::from(w.rad_level),
            "RadLevel",
            "0",
            Conversion::Int,
        ),
        (
            "area_fire",
            i128::from(w.area_fire),
            "AreaFire",
            "no",
            Conversion::Flag,
        ),
        (
            "suicide",
            i128::from(w.suicide),
            "Suicide",
            "no",
            Conversion::Flag,
        ),
        (
            "sonic",
            i128::from(w.sonic),
            "IsSonic",
            "no",
            Conversion::Flag,
        ),
    ] {
        let source = if default.is_empty() {
            rule(r, section, key)?
        } else {
            optional(r, section, key, default)?
        };
        let display = source.value.clone();
        check_int(
            a,
            &consumer,
            &format!("combat.weapons.{field}"),
            &id,
            field,
            actual,
            converted(source, c)?,
            display,
        )?;
    }
    let damage = converted(rule(r, section, "Damage")?, Conversion::Int)?;
    check_int(
        a,
        &consumer,
        "combat.weapons.repair",
        &id,
        "repair",
        i128::from(w.repair),
        damage.map(|n| i128::from(n < 0)),
        "Damage<0".into(),
    )?;
    let native = rule(r, section, "Warhead")?;
    let display = native.value.clone();
    let expected = native.map(|name| {
        warhead_ids
            .get(&name)
            .cloned()
            .unwrap_or_else(|| format!("?{name}"))
    });
    let actual = d.warhead(w.warhead).map_or(String::new(), |v| v.id.clone());
    a.check(
        WitnessId::new(&consumer, "combat.weapons.warhead"),
        &id,
        "warhead",
        actual,
        expected,
        display,
    )?;
    check_int(
        a,
        &consumer,
        "combat.weapons.projectile_speed",
        &id,
        "projectile_speed",
        i128::from(w.projectile_speed),
        projectile_speed(r, section)?,
        "projectile branch conversion; source_display must preserve existing difference identity"
            .into(),
    )?;
    let projectile = rule(r, section, "Projectile")?;
    let p = projectile.value.clone();
    let aa = converted(optional(r, &p, "AA", "no")?, Conversion::Flag)?.because(&projectile);
    let ag = converted(optional(r, &p, "AG", "yes")?, Conversion::Flag)?.because(&projectile);
    for (field, actual, expected) in [
        ("anti_air", i128::from(w.anti_air), aa.clone()),
        ("anti_ground", i128::from(w.anti_ground), ag.clone()),
        (
            "naval_only",
            i128::from(w.naval_only),
            ag.zip(aa).map(|(ag, aa)| i128::from(ag == 0 && aa == 0)),
        ),
    ] {
        let display = match field {
            "anti_air" => optional(r, &p, "AA", "no")?.value,
            "anti_ground" => optional(r, &p, "AG", "yes")?.value,
            _ => expected.value.to_string(),
        };
        check_int(
            a,
            &consumer,
            &format!("combat.weapons.{field}"),
            &id,
            field,
            actual,
            expected,
            display,
        )?;
    }
    let inaccurate = converted(optional(r, &p, "Inaccurate", "no")?, Conversion::Flag)?;
    let scatter = if inaccurate.value == 1 {
        converted(
            rule(r, "CombatDamage", "BallisticScatter")?,
            Conversion::Cells,
        )?
        .zip(converted(
            optional(r, &p, "Inviso", "no")?,
            Conversion::Flag,
        )?)
        .try_map(|(base, inviso)| checked_product(&[base, 1 + inviso]))?
    } else {
        Expected::literal(0)
    }
    .because(&inaccurate)
    .because(&projectile);
    check_int(
        a,
        &consumer,
        "combat.weapons.scatter",
        &id,
        "scatter",
        i128::from(w.scatter),
        scatter,
        "Inaccurate branch; source display pending exact existing difference format".into(),
    )?;
    Ok(())
}
pub(super) fn warhead(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    game_id: &str,
    section: &str,
) -> AuditResult<()> {
    let w = d
        .warhead_kind(game_id)
        .and_then(|k| d.warhead(k))
        .ok_or_else(|| format!("missing warhead {game_id}"))?;
    let id = format!("warhead.{game_id}");
    let consumer = format!("{id}@rules:{section}");
    let verses = optional(
        r,
        section,
        "Verses",
        "100%,100%,100%,100%,100%,100%,100%,100%,100%,100%,100%",
    )?;
    let display = verses.value.clone();
    let expected = verses.try_map(|s| {
        let components = s.split(',').collect::<Vec<_>>();
        if components.len() != 11 {
            return Err(format!("exact11 Verses required for {section}"));
        }
        components
            .iter()
            .map(|v| {
                super::actual::decimal(v.trim()).and_then(|(n, den)| {
                    if den == 1 {
                        Ok(n.to_string())
                    } else {
                        Err("noninteger Verses percentage".into())
                    }
                })
            })
            .collect::<AuditResult<Vec<_>>>()
            .map(|v| v.join(","))
    })?;
    if w.versus.len() != 11 {
        return Err(format!("actual11 armour entries required for {game_id}"));
    }
    a.check(
        WitnessId::new(&consumer, "combat.warheads.versus"),
        &id,
        "versus",
        w.versus
            .iter()
            .map(i32::to_string)
            .collect::<Vec<_>>()
            .join(","),
        expected,
        display,
    )?;
    for (field, actual, key, default, c) in [
        (
            "cell_spread",
            i128::from(w.cell_spread),
            "CellSpread",
            "0",
            Conversion::Cells,
        ),
        (
            "percent_at_max",
            i128::from(w.percent_at_max),
            "PercentAtMax",
            "1",
            Conversion::Percent,
        ),
        (
            "paralyzes",
            i128::from(w.paralyzes),
            "Paralyzes",
            "0",
            Conversion::Int,
        ),
        ("wall", i128::from(w.wall), "Wall", "no", Conversion::Flag),
        ("wood", i128::from(w.wood), "Wood", "no", Conversion::Flag),
        (
            "temporal",
            i128::from(w.temporal),
            "Temporal",
            "no",
            Conversion::Flag,
        ),
        (
            "mind_control",
            i128::from(w.mind_control),
            "MindControl",
            "no",
            Conversion::Flag,
        ),
        (
            "psychic_damage",
            i128::from(w.psychic_damage),
            "PsychicDamage",
            "no",
            Conversion::Flag,
        ),
        (
            "radiation",
            i128::from(w.radiation),
            "Radiation",
            "no",
            Conversion::Flag,
        ),
        (
            "parasite",
            i128::from(w.parasite),
            "Parasite",
            "no",
            Conversion::Flag,
        ),
        (
            "ivan_bomb",
            i128::from(w.ivan_bomb),
            "IvanBomb",
            "no",
            Conversion::Flag,
        ),
        (
            "bomb_disarm",
            i128::from(w.bomb_disarm),
            "BombDisarm",
            "no",
            Conversion::Flag,
        ),
        (
            "electric_assault",
            i128::from(w.electric_assault),
            "ElectricAssault",
            "no",
            Conversion::Flag,
        ),
        (
            "makes_disguise",
            i128::from(w.makes_disguise),
            "MakesDisguise",
            "no",
            Conversion::Flag,
        ),
        (
            "culling",
            i128::from(w.culling),
            "Culling",
            "no",
            Conversion::Flag,
        ),
    ] {
        let raw = optional(r, section, key, default)?;
        let display = raw.value.clone();
        check_int(
            a,
            &consumer,
            &format!("combat.warheads.{field}"),
            &id,
            field,
            actual,
            converted(raw, c)?,
            display,
        )?;
    }
    Ok(())
}
// Historical preparation note: PENDING integration: wrappers must enumerate actual complete arrays/mappings before calls;
// compare armour identities/order explicitly; audit effect9 damage/warhead; register physical
// requirements independently; port source_display exactly for23 current differences; don't
// pre-mark foreign differences or let component/whole row credit overlap.

// Nine explicit effect IDs; rejecting a new/missing effect is separate from field coverage.
pub(super) fn effects(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    warhead_ids: &BTreeMap<String, String>,
) -> AuditResult<()> {
    use sim::Effect::*;
    let rows = [
        (
            Nuke,
            Some(("NukePayload", "Damage")),
            ("NukePayload", "Warhead"),
        ),
        (
            Lightning,
            Some(("General", "LightningDamage")),
            ("General", "LightningWarhead"),
        ),
        (
            IvanBomb,
            Some(("CombatDamage", "IvanDamage")),
            ("CombatDamage", "IvanWarhead"),
        ),
        (
            C4,
            Some(("CombatDamage", "MaxDamage")),
            ("CombatDamage", "C4Warhead"),
        ),
        (Radiation, None, ("Radiation", "RadSiteWarhead")),
        (
            V3Rocket,
            Some(("General", "V3RocketDamage")),
            ("CombatDamage", "V3Warhead"),
        ),
        (
            V3RocketElite,
            Some(("General", "V3RocketEliteDamage")),
            ("CombatDamage", "V3EliteWarhead"),
        ),
        (
            DMisl,
            Some(("General", "DMislDamage")),
            ("CombatDamage", "DMislWarhead"),
        ),
        (
            DMislElite,
            Some(("General", "DMislEliteDamage")),
            ("CombatDamage", "DMislEliteWarhead"),
        ),
    ];
    if rows.map(|v| v.0) != sim::Effect::ALL {
        return Err("effect identity/order changed".into());
    }
    for (kind, damage, (section, key)) in rows {
        let game = d
            .effect(kind)
            .ok_or_else(|| format!("missing effect {}", kind.id()))?;
        let id = format!("effect.{}", kind.id());
        let (expected, display) = match damage {
            Some((section, key)) => {
                let raw = rule(r, section, key)?;
                let display = raw.value.clone();
                (
                    converted(raw, Conversion::Int)?.map(|n| n.to_string()),
                    display,
                )
            }
            None => (Expected::literal("-".to_owned()), "-".to_owned()),
        };
        // Radiation's exact existing explicit '-' difference remains required. Do not replace
        // it with a silent own-field success or credit invented damage source coverage.
        a.check(
            WitnessId::new(&id, "combat.effects.damage"),
            &id,
            "damage",
            game.damage.to_string(),
            expected,
            display,
        )?;
        let native = rule(r, section, key)?;
        let display = native.value.clone();
        let expected = native.map(|n| {
            warhead_ids
                .get(&n)
                .cloned()
                .unwrap_or_else(|| format!("?{n}"))
        });
        let actual = d
            .warhead(game.warhead)
            .map_or(String::new(), |v| v.id.clone());
        a.check(
            WitnessId::new(&id, "combat.effects.warhead"),
            &id,
            "warhead",
            actual,
            expected,
            display,
        )?;
    }
    Ok(())
}
// Exact complete Powerups catalogue includes zero-share rows. Enabled presence/disabled absence
// is an ACTUAL loaded-list comparison, not a reference-only assertion that share is zero.
// Candidate game IDs are an explicit independent semantic mapping (not generated from data).
// The final complete actual-ID guard rejects any extra ID, including unsupported aliases.
pub(super) fn powerups(a: &mut Audit, r: &Reference, d: &Definitions) -> AuditResult<()> {
    let rules = d
        .rules()
        .ok_or_else(|| "shipped rules missing".to_owned())?;
    let rows = [
        ("Armor", "armour", Some(Conversion::Permille)),
        ("Cloak", "cloak", None),
        ("Darkness", "darkness", None),
        ("Explosion", "explosion", None),
        ("Firepower", "firepower", Some(Conversion::Permille)),
        ("Gas", "gas", None),
        ("HealBase", "heal_base", None),
        ("ICBM", "icbm", None),
        ("Invulnerability", "invulnerability", None),
        ("IonStorm", "ion_storm", None),
        ("Money", "money", Some(Conversion::Int)),
        ("Napalm", "napalm", None),
        ("Pod", "pod", None),
        ("Reveal", "reveal", None),
        ("Speed", "speed", Some(Conversion::Permille)),
        ("Squad", "squad", None),
        ("Tiberium", "tiberium", None),
        ("Unit", "unit", None),
        ("Veteran", "veteran", Some(Conversion::Int)),
    ];
    let mut seen = std::collections::BTreeSet::new();
    for (key, id, conversion) in rows {
        let source = SourceKey::new("rules", "Powerups", key);
        let share = converted(r.component(source.clone(), 0)?, Conversion::Int)?;
        let consumer = format!("powerup@rules:Powerups:{key}");
        // A keyed search must reject duplicate IDs globally before this loop (raw-schema guard).
        let actual = rules.powerups.iter().find(|v| v.id == id);
        if let Some(actual) = actual {
            seen.insert(actual.id.as_str());
        }
        let presence = share.clone().map(|n| i128::from(n > 0));
        check_int(
            a,
            &consumer,
            "rules.powerups.membership",
            "rules",
            &format!("powerups.{key}.present"),
            i128::from(actual.is_some()),
            presence,
            share.value.to_string(),
        )?;
        if let Some(actual) = actual {
            let display = share.value.to_string();
            check_int(
                a,
                &consumer,
                "rules.powerups.share",
                "rules",
                &format!("powerups.{key}.share"),
                i128::from(actual.share),
                share,
                display,
            )?;
            match conversion {
                Some(conversion) => {
                    let raw = r.component(source, 3)?;
                    let display = raw.value.clone();
                    check_int(
                        a,
                        &consumer,
                        "rules.powerups.value",
                        "rules",
                        &format!("powerups.{key}.value"),
                        i128::from(actual.value),
                        converted(raw, conversion)?,
                        display,
                    )?;
                }
                None => {
                    if actual.value != 0 {
                        return Err(format!("argumentless {key} value must be0"));
                    }
                    a.game_owned(WitnessId::new(&consumer,"rules.powerups.value"),actual.value.to_string(),
                        "This enabled source tuple has no fourth argument; zero is the explicit no-argument sentinel, not copied source coverage".into(),
                        "crates.rs::open content-specific dispatch".into())?;
                }
            }
        }
    }
    if seen.len() != rules.powerups.len() {
        return Err("unmapped/duplicate actual Powerup entry".into());
    }
    Ok(())
}
