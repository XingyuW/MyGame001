//! UNVALIDATED cache-only unit/building adapter body draft. No compilation/execution.
//! Uses actual public Definitions, Production, ArsenalEntry and CombatTerms accessors.
//! Shared physical path registration/difference identity normalization remains for integration.
use super::actual::{check_int, optional, positive_round, rule};
use super::combat::{Conversion, converted};
use super::rules::{
    GROUPS, HOUSES, MappingRow, building_id, csv, mapped, registered, text, unique_set, unit_id,
    weapon_id,
};
use super::{Audit, AuditResult, Expected, Reference, SourceKey, WitnessId, sequence};
use sim::{Category, Definitions, Domain, ProductionItem};
use std::collections::{BTreeMap, BTreeSet};
pub(super) fn product(v: &[i128]) -> AuditResult<i128> {
    v.iter().try_fold(1i128, |a, b| {
        a.checked_mul(*b)
            .ok_or_else(|| "exact arithmetic overflow".into())
    })
}
pub(super) fn item(d: &Definitions, row: &MappingRow) -> AuditResult<ProductionItem> {
    if row.building {
        d.building_kind(&row.game_id).map(ProductionItem::Building)
    } else {
        d.unit_kind(&row.game_id).map(ProductionItem::Unit)
    }
    .ok_or_else(|| format!("mapped item missing {}@{}", row.game_id, row.section))
}
pub(super) fn unit_speed(r: &Reference, section: &str) -> AuditResult<Expected<i128>> {
    let locomotor = rule(r, section, "Locomotor")?;
    let value = if locomotor.value.starts_with("{92612C46-") {
        converted(rule(r, section, "JumpjetSpeed")?, Conversion::Int)?
    } else {
        let speed = converted(rule(r, section, "Speed")?, Conversion::Int)?
            .try_map(|v| product(&[v, 256]).map(|n| (n / 100).min(255)))?;
        let infantry = registered(r, "InfantryTypes", section)?;
        speed.zip(infantry).try_map(|(speed, foot)| {
            if foot {
                product(&[speed, 3]).map(|v| v / 2)
            } else {
                Ok(speed)
            }
        })?
    };
    value
        .try_map(|v| positive_round(product(&[v, 3])?, 4).map(|n| n.max(1)))
        .map(|v| v.because(&locomotor))
}
pub(super) fn unit_terms(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    row: &MappingRow,
) -> AuditResult<()> {
    let sim::ProductionItem::Unit(kind) = item(d, row)? else {
        return Ok(());
    };
    let game = d
        .unit(kind)
        .ok_or_else(|| "missing actual unit".to_owned())?;
    let consumer = format!("{}@rules:{}", row.game_id, row.section);
    check_int(
        a,
        &consumer,
        "units.units.speed",
        &row.game_id,
        "speed",
        i128::from(game.speed),
        unit_speed(r, &row.section)?,
        "Locomotor active speed branch".into(),
    )?;
    let raw = optional(r, &row.section, "Harvester", "no")?;
    let display = raw.value.clone();
    let harvest = converted(raw, Conversion::Flag)?;
    let actual = d.harvester(kind);
    check_int(
        a,
        &consumer,
        "economy.harvesters.membership",
        &row.game_id,
        "harvester",
        i128::from(actual.is_some()),
        harvest.clone(),
        display,
    )?;
    if let Some(actual) = actual {
        let raw = rule(r, &row.section, "Storage")?;
        let display = raw.value.clone();
        check_int(
            a,
            &consumer,
            "economy.harvesters.capacity",
            &row.game_id,
            "storage",
            i128::from(actual.capacity),
            converted(raw, Conversion::Int)?.because(&harvest),
            display,
        )?;
        // Preserve existing pace assertions while correcting the stale claim that no native
        // Harvest.Rate key exists. Applicability of that key is a separate pending ledger lead.
        if actual.harvest_rate != 1 || actual.harvest_interval != 19 {
            return Err("R58 declared shipped one bail/19ticks assumption".into());
        }
        for (field, value, reason) in [
            (
                "harvest_rate",
                actual.harvest_rate,
                "R58 explicit one-bail extraction granularity",
            ),
            (
                "harvest_interval",
                actual.harvest_interval,
                "R58 declared19ticks assumption; [Harvest]Rate applicability unresolved, no fake direct parity claim",
            ),
        ] {
            a.game_owned(
                WitnessId::new(&consumer, &format!("economy.harvesters.{field}")),
                value.to_string(),
                reason.into(),
                "DECISIONS R58; plan AppendixD8; ledger HARVEST-SOURCE-02".into(),
            )?;
        }
    }
    Ok(())
}
pub(super) fn footprint(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    row: &MappingRow,
) -> AuditResult<()> {
    let sim::ProductionItem::Building(kind) = item(d, row)? else {
        return Ok(());
    };
    let actual = d
        .building(kind)
        .ok_or_else(|| "missing building".to_owned())?;
    let image = optional(r, &row.section, "Image", &row.section)?;
    let foundation = r.expect(SourceKey::new("art", &image.value, "Foundation"))?;
    let display = foundation.value.clone();
    let dims = foundation
        .try_map(|v| {
            let (x, y) = v
                .split_once('x')
                .ok_or_else(|| format!("not rectangular Foundation {v}"))?;
            let x = x.parse::<i128>().map_err(|e| e.to_string())?;
            let y = y.parse::<i128>().map_err(|e| e.to_string())?;
            if x <= 0 || y <= 0 {
                return Err("nonpositive Foundation".into());
            }
            Ok((x, y))
        })?
        .because(&image);
    let consumer = format!("{}@rules:{}", row.game_id, row.section);
    for (field, actual, expected) in [
        ("width", i128::from(actual.width), dims.clone().map(|v| v.0)),
        ("height", i128::from(actual.height), dims.map(|v| v.1)),
    ] {
        check_int(
            a,
            &consumer,
            &format!("economy.buildings.{field}"),
            &row.game_id,
            &format!("footprint.{field}"),
            actual,
            expected,
            display.clone(),
        )?;
    }
    // Native art.Height is visual elevation, separately exact-unused. It is never y dimension.
    //28 current raw rectangles differ: no approval or automatic remapping is embedded here.
    Ok(())
}
pub(super) fn building_terms(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    row: &MappingRow,
    ids: &BTreeMap<String, String>,
) -> AuditResult<()> {
    let sim::ProductionItem::Building(kind) = item(d, row)? else {
        return Ok(());
    };
    let game = d
        .building(kind)
        .ok_or_else(|| "missing actual building".to_owned())?;
    let consumer = format!("{}@rules:{}", row.game_id, row.section);
    let power = converted(optional(r, &row.section, "Power", "0")?, Conversion::Int)?;
    for (field, actual, expected) in [
        (
            "power_produced",
            i128::from(game.power_produced),
            power.clone().map(|v| v.max(0)),
        ),
        (
            "power_consumed",
            i128::from(game.power_consumed),
            power.try_map(|v| {
                v.checked_neg()
                    .map(|v| v.max(0))
                    .ok_or_else(|| "power negation overflow".into())
            })?,
        ),
    ] {
        check_int(
            a,
            &consumer,
            &format!("economy.buildings.{field}"),
            &row.game_id,
            field,
            actual,
            expected,
            "Power signed split".into(),
        )?;
    }
    let cost = converted(optional(r, &row.section, "Cost", "0")?, Conversion::Int)?;
    let refund = converted(rule(r, "General", "RefundPercent")?, Conversion::Percent)?;
    let by_rule = cost
        .zip(refund)
        .try_map(|(cost, refund)| product(&[cost, refund]).map(|v| v / 100))?;
    check_int(
        a,
        &consumer,
        "economy.buildings.sell_value",
        &row.game_id,
        "sell_value",
        i128::from(game.sell_value),
        by_rule.clone(),
        "Cost;RefundPercent".into(),
    )?;
    let soylent = converted(optional(r, &row.section, "Soylent", "0")?, Conversion::Int)?;
    let effective = if soylent.value != 0 {
        soylent.clone()
    } else {
        by_rule.because(&soylent)
    };
    check_int(
        a,
        &consumer,
        "economy.refunds.effective",
        &row.game_id,
        "refund",
        i128::from(d.refund(kind)),
        effective,
        "nonzero Soylent else Cost*RefundPercent".into(),
    )?;
    check_int(
        a,
        &consumer,
        "economy.refunds.membership",
        &row.game_id,
        "fixed_refund_present",
        i128::from(game.fixed_refund.is_some()),
        soylent.clone().map(|v| i128::from(v != 0)),
        "Soylent nonzero".into(),
    )?;
    if let Some(actual) = game.fixed_refund {
        check_int(
            a,
            &consumer,
            "economy.refunds.credits",
            &row.game_id,
            "fixed_refund",
            i128::from(actual),
            soylent,
            "Soylent".into(),
        )?;
    }
    let free = optional(r, &row.section, "FreeUnit", "")?;
    let display = free.value.clone();
    let expected = free.try_map(|v| {
        csv(&v)?
            .into_iter()
            .map(|native| {
                ids.get(&native)
                    .cloned()
                    .ok_or_else(|| format!("unmapped FreeUnit {native}"))
            })
            .collect::<AuditResult<Vec<_>>>()
            .map(|v| v.join(","))
    })?;
    let actual = game
        .free_units
        .iter()
        .map(|&k| unit_id(d, k))
        .collect::<AuditResult<Vec<_>>>()?
        .join(",");
    text(
        a,
        &consumer,
        "economy.buildings.free_units",
        &row.game_id,
        "free_units",
        actual,
        expected,
        display,
    )?;
    let refinery = converted(
        optional(r, &row.section, "Refinery", "no")?,
        Conversion::Flag,
    )?;
    check_int(
        a,
        &consumer,
        "economy.refineries.membership",
        &row.game_id,
        "refinery",
        i128::from(game.credits_per_ore.is_some()),
        refinery.clone(),
        "Refinery".into(),
    )?;
    if game.credits_per_ore.is_some() != game.credits_per_gem.is_some() {
        return Err("refinery ore/gem membership disagreement".into());
    }
    for (field, actual, section) in [
        ("credits_per_ore", game.credits_per_ore, "Riparius"),
        ("credits_per_gem", game.credits_per_gem, "Cruentus"),
    ] {
        if let Some(actual) = actual {
            let raw = rule(r, section, "Value")?;
            let display = raw.value.clone();
            check_int(
                a,
                &consumer,
                &format!("economy.refineries.{field}"),
                &row.game_id,
                field,
                i128::from(actual),
                converted(raw, Conversion::Int)?.because(&refinery),
                display,
            )?;
        }
    }
    Ok(())
}
pub(super) fn prerequisites(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    row: &MappingRow,
    ids: &BTreeMap<String, String>,
) -> AuditResult<()> {
    let item = item(d, row)?;
    let terms = d
        .production(item)
        .ok_or_else(|| "missing production".to_owned())?;
    let m = d
        .metadata(item)
        .ok_or_else(|| "missing metadata".to_owned())?;
    let rules = d.rules().ok_or_else(|| "missing Rules".to_owned())?;
    let consumer = format!("{}@rules:{}", row.game_id, row.section);
    let source = optional(r, &row.section, "Prerequisite", "")?;
    let display = source.value.clone();
    let tokens = source.clone().try_map(|v| csv(&v))?;
    let expected_buildings = tokens.clone().try_map(|v| {
        v.into_iter()
            .filter(|t| {
                !GROUPS
                    .iter()
                    .any(|(name, _, _)| t.eq_ignore_ascii_case(name))
            })
            .map(|native| {
                ids.get(&native)
                    .cloned()
                    .ok_or_else(|| format!("unmapped physical building prerequisite {native}"))
            })
            .collect::<AuditResult<Vec<_>>>()
            .and_then(|v| unique_set(v, "physical source building prerequisites"))
    })?;
    let expected_groups = tokens.try_map(|v| {
        let names = v
            .into_iter()
            .filter_map(|t| {
                GROUPS
                    .iter()
                    .find(|(name, _, _)| t.eq_ignore_ascii_case(name))
                    .map(|(_, _, id)| id.to_string())
            })
            .collect::<Vec<_>>();
        unique_set(names, "physical source group prerequisites")
    })?;
    let physical_list = if row.building { "buildings" } else { "units" };
    let actual_buildings = terms
        .prerequisites
        .iter()
        .map(|&k| building_id(d, k))
        .collect::<AuditResult<Vec<_>>>()?;
    text(
        a,
        &consumer,
        &format!("economy.ron.{physical_list}.prerequisites"),
        &row.game_id,
        "prerequisite_buildings",
        unique_set(actual_buildings, "actual physical building prerequisites")?,
        expected_buildings,
        display.clone(),
    )?;
    let actual_groups = m
        .prerequisite_groups
        .iter()
        .map(|&k| {
            rules
                .group(k)
                .map(|g| g.id.clone())
                .ok_or_else(|| "invalid physical group".to_owned())
        })
        .collect::<AuditResult<Vec<_>>>()?;
    text(
        a,
        &consumer,
        "arsenal.ron.entries.prerequisite_groups",
        &row.game_id,
        "prerequisite_groups",
        unique_set(actual_groups, "actual physical group prerequisites")?,
        expected_groups,
        display.clone(),
    )?;
    let expected = source.try_map(|v| {
        csv(&v)?
            .into_iter()
            .map(|token| {
                match GROUPS
                    .iter()
                    .find(|(name, _, _)| token.eq_ignore_ascii_case(name))
                {
                    Some((_, _, id)) => Ok(format!("group:{id}")),
                    None => ids
                        .get(&token)
                        .cloned()
                        .ok_or_else(|| format!("unmapped prerequisite token {token}")),
                }
            })
            .collect::<AuditResult<Vec<_>>>()
            .and_then(|v| unique_set(v, "source prerequisites"))
    })?;
    let mut actual = terms
        .prerequisites
        .iter()
        .map(|&k| building_id(d, k))
        .collect::<AuditResult<Vec<_>>>()?;
    for &group in &m.prerequisite_groups {
        actual.push(format!(
            "group:{}",
            rules
                .group(group)
                .ok_or_else(|| "invalid actual group".to_owned())?
                .id
        ));
    }
    text(
        a,
        &consumer,
        "economy+arsenal.prerequisites",
        &row.game_id,
        "prerequisites",
        unique_set(actual, "actual prerequisites")?,
        expected,
        display,
    )?;
    for (field, key, actual) in [
        (
            "required_countries",
            "RequiredHouses",
            &m.required_countries,
        ),
        (
            "forbidden_countries",
            "ForbiddenHouses",
            &m.forbidden_countries,
        ),
    ] {
        let raw = optional(r, &row.section, key, "")?;
        let display = raw.value.clone();
        let expected = raw.try_map(|v| {
            csv(&v)?
                .into_iter()
                .map(|house| {
                    HOUSES
                        .iter()
                        .find(|(native, _)| *native == house)
                        .map(|(_, id)| id.to_string())
                        .ok_or_else(|| format!("unknown restriction house {house}"))
                })
                .collect::<AuditResult<Vec<_>>>()
                .and_then(|v| unique_set(v, "source country restriction"))
        })?;
        let actual = actual
            .iter()
            .map(|&id| {
                rules
                    .country(id)
                    .map(|c| c.id.clone())
                    .ok_or_else(|| "invalid actual country".to_owned())
            })
            .collect::<AuditResult<Vec<_>>>()?;
        text(
            a,
            &consumer,
            &format!("arsenal.{field}"),
            &row.game_id,
            field,
            unique_set(actual, "actual country restriction")?,
            expected,
            display,
        )?;
    }
    Ok(())
}

pub(super) fn metadata_flags(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    row: &MappingRow,
) -> AuditResult<()> {
    let actual_item = item(d, row)?;
    let m = d
        .metadata(actual_item)
        .ok_or_else(|| "missing metadata".to_owned())?;
    let consumer = format!("{}@rules:{}", row.game_id, row.section);
    for (field, actual, key, default) in [
        ("needs_engineer", m.needs_engineer, "NeedsEngineer", "no"),
        ("capturable", m.capturable, "Capturable", "no"),
        ("insignificant", m.insignificant, "Insignificant", "no"),
        ("immune", m.immune, "Immune", "no"),
        ("hospital", m.hospital, "Hospital", "no"),
        (
            "bridge_repair_hut",
            m.bridge_repair_hut,
            "BridgeRepairHut",
            "no",
        ),
        ("occupier", m.occupier, "Occupier", "no"),
        ("can_be_occupied", m.can_be_occupied, "CanBeOccupied", "no"),
        ("crewed", m.crewed, "Crewed", "no"),
        ("deployer", m.deployer, "Deployer", "no"),
        ("deploy_fire", m.deploy_fire, "DeployFire", "no"),
        ("c4", m.c4, "C4", "no"),
        ("can_c4", m.can_c4, "CanC4", "yes"),
        ("bombable", m.bombable, "Bombable", "yes"),
        ("spyable", m.spyable, "Spyable", "no"),
        ("detect_disguise", m.detect_disguise, "DetectDisguise", "no"),
        (
            "immune_psionics",
            m.immune_psionics,
            "ImmuneToPsionics",
            if row.building { "yes" } else { "no" },
        ),
        (
            "immune_radiation",
            m.immune_radiation,
            "ImmuneToRadiation",
            "no",
        ),
        ("teleporter", m.teleporter, "Teleporter", "no"),
        (
            "stolen_allied",
            m.stolen_allied,
            "RequiresStolenAlliedTech",
            "no",
        ),
        (
            "stolen_soviet",
            m.stolen_soviet,
            "RequiresStolenSovietTech",
            "no",
        ),
        ("overpowerable", m.overpowerable, "Overpowerable", "no"),
        ("explodes", m.explodes, "Explodes", "no"),
        (
            "disguise_when_still",
            m.disguise_when_still,
            "DisguiseWhenStill",
            "no",
        ),
    ] {
        let raw = optional(r, &row.section, key, default)?;
        let display = raw.value.clone();
        check_int(
            a,
            &consumer,
            &format!("arsenal.{field}"),
            &row.game_id,
            field,
            i128::from(actual),
            converted(raw, Conversion::Flag)?,
            display,
        )?;
    }
    for (field, actual, key) in [
        ("max_occupants", m.max_occupants, "MaxNumberOccupants"),
        ("cash_startup", m.cash_startup, "ProduceCashStartup"),
        ("cash_amount", m.cash_amount, "ProduceCashAmount"),
        ("build_limit", m.build_limit, "BuildLimit"),
        ("guard_range", m.guard_range, "GuardRange"),
    ] {
        let raw = optional(r, &row.section, key, "0")?;
        let display = raw.value.clone();
        check_int(
            a,
            &consumer,
            &format!("arsenal.{field}"),
            &row.game_id,
            field,
            i128::from(actual),
            converted(raw, Conversion::Int)?,
            display,
        )?;
    }
    let delay = optional(r, &row.section, "ProduceCashDelay", "0")?;
    let display = delay.value.clone();
    let expected = if delay.value == "0" {
        Expected::literal(0).because(&delay)
    } else {
        converted(delay, Conversion::Frames)?
    };
    check_int(
        a,
        &consumer,
        "arsenal.cash_delay",
        &row.game_id,
        "cash_delay",
        i128::from(m.cash_delay),
        expected,
        display,
    )?;
    let sensors = converted(
        optional(r, &row.section, "Sensors", "no")?,
        Conversion::Flag,
    )?;
    let sight = if sensors.value == 1 {
        converted(
            optional(r, &row.section, "SensorsSight", "0")?,
            Conversion::Int,
        )?
        .because(&sensors)
    } else {
        Expected::literal(0).because(&sensors)
    };
    check_int(
        a,
        &consumer,
        "arsenal.sensors_sight",
        &row.game_id,
        "sensors_sight",
        i128::from(m.sensors_sight),
        sight,
        "Sensors active branch SensorsSight".into(),
    )?;
    // Sources whose inactive SensorsSight exists need a separate exact-unused disposition;
    // nominal parsing must not make that untaken branch count as an actual comparison.
    let zone = optional(r, &row.section, "MovementZone", "")?;
    let display = zone.value.clone();
    let expected = if row.building {
        Expected::literal("land".to_owned())
    } else {
        zone.map(|z| {
            match z.as_str() {
                "Fly" => "air",
                "Water" => "naval",
                "Amphibious" | "AmphibiousDestroyer" | "AmphibiousCrusher" => "amphibious",
                _ => "land",
            }
            .to_owned()
        })
    };
    let actual = match m.domain {
        Domain::Land => "land",
        Domain::Air => "air",
        Domain::Naval => "naval",
        Domain::Amphibious => "amphibious",
    };
    text(
        a,
        &consumer,
        "arsenal.domain",
        &row.game_id,
        "domain",
        actual.into(),
        expected,
        display,
    )?;
    let capacity = if row.building {
        Expected::literal(0)
    } else {
        converted(
            optional(r, &row.section, "Passengers", "0")?,
            Conversion::Int,
        )?
    };
    check_int(
        a,
        &consumer,
        "arsenal.capacity",
        &row.game_id,
        "capacity",
        i128::try_from(m.capacity).map_err(|_| "capacity conversion overflow")?,
        capacity,
        "Passengers for units;0 for buildings".into(),
    )?;
    let infantry = registered(r, "InfantryTypes", &row.section)?;
    if row.building {
        for (field, actual, key, default) in [
            ("base_normal", m.base_normal, "BaseNormal", "yes"),
            ("radar", m.radar, "Radar", "no"),
            ("repairable", m.repairable, "Repairable", "yes"),
            ("powered", m.powered, "Powered", "no"),
            ("unit_repair", m.unit_repair, "UnitRepair", "no"),
        ] {
            let raw = optional(r, &row.section, key, default)?;
            let display = raw.value.clone();
            check_int(
                a,
                &consumer,
                &format!("arsenal.{field}"),
                &row.game_id,
                field,
                i128::from(actual),
                converted(raw, Conversion::Flag)?,
                display,
            )?;
        }
        let wall = converted(optional(r, &row.section, "Wall", "no")?, Conversion::Flag)?;
        let unsellable = converted(
            optional(r, &row.section, "Unsellable", "no")?,
            Conversion::Flag,
        )?;
        check_int(
            a,
            &consumer,
            "arsenal.sellable",
            &row.game_id,
            "sellable",
            i128::from(m.sellable),
            wall.zip(unsellable)
                .map(|(w, u)| i128::from(w == 0 && u == 0)),
            "Wall;Unsellable".into(),
        )?;
        let raw = optional(r, &row.section, "Adjacent", "2")?;
        let display = raw.value.clone();
        check_int(
            a,
            &consumer,
            "arsenal.adjacent",
            &row.game_id,
            "adjacent",
            i128::from(m.adjacent),
            converted(raw, Conversion::Int)?,
            display,
        )?;
        // Native building category is separate from actual production lane.
        let buildcat = optional(r, &row.section, "BuildCat", "")?;
        let display = buildcat.value.clone();
        let category = buildcat
            .clone()
            .map(|v| if v == "Combat" { "armory" } else { "buildings" }.to_owned());
        let actual = match m.category {
            Category::Buildings => "buildings",
            Category::Armory => "armory",
            Category::Infantry => "infantry",
            Category::Vehicles => "vehicles",
        };
        text(
            a,
            &consumer,
            "arsenal.category",
            &row.game_id,
            "category",
            actual.into(),
            category,
            display,
        )?;
        // Cross-kind sentinels are actual checks, never invented source dependencies.
        for (field, ok, actual) in [
            ("infantry", !m.infantry, u8::from(m.infantry).to_string()),
            ("crusher", !m.crusher, u8::from(m.crusher).to_string()),
            ("crushable", !m.crushable, u8::from(m.crushable).to_string()),
            ("size", m.size == 0, m.size.to_string()),
            ("trainable", !m.trainable, u8::from(m.trainable).to_string()),
            (
                "self_healing",
                !m.self_healing,
                u8::from(m.self_healing).to_string(),
            ),
            ("ammo", m.ammo == 0, m.ammo.to_string()),
            (
                "airport_bound",
                !m.airport_bound,
                u8::from(m.airport_bound).to_string(),
            ),
            ("cloakable", !m.cloakable, u8::from(m.cloakable).to_string()),
        ] {
            if !ok {
                return Err(format!("building {field} cross-kind sentinel"));
            }
            a.game_owned(
                WitnessId::new(&consumer, &format!("arsenal.{field}.cross_kind")),
                actual,
                "Building has no corresponding unit-only trait".into(),
                "ArsenalEntry kind-specific validation and data model".into(),
            )?;
        }
    } else {
        check_int(
            a,
            &consumer,
            "arsenal.infantry",
            &row.game_id,
            "infantry",
            i128::from(m.infantry),
            infantry.clone().map(i128::from),
            "matching InfantryTypes registry member only".into(),
        )?;
        for (field, actual, key, default) in [
            ("crusher", m.crusher, "Crusher", "no"),
            (
                "crushable",
                m.crushable,
                "Crushable",
                if infantry.value { "yes" } else { "no" },
            ),
            ("trainable", m.trainable, "Trainable", "yes"),
            ("self_healing", m.self_healing, "SelfHealing", "no"),
            ("airport_bound", m.airport_bound, "AirportBound", "no"),
            ("cloakable", m.cloakable, "Cloakable", "no"),
        ] {
            let present = r
                .rows
                .contains_key(&SourceKey::new("rules", &row.section, key));
            let raw = optional(r, &row.section, key, default)?;
            let display = raw.value.clone();
            let expected = converted(raw, Conversion::Flag)?;
            let expected = if field == "crushable" && !present {
                expected.because(&infantry)
            } else {
                expected
            };
            check_int(
                a,
                &consumer,
                &format!("arsenal.{field}"),
                &row.game_id,
                field,
                i128::from(actual),
                expected,
                display,
            )?;
        }
        let category = infantry.map(|v| if v { "infantry" } else { "vehicles" }.to_owned());
        let actual = match m.category {
            Category::Buildings => "buildings",
            Category::Armory => "armory",
            Category::Infantry => "infantry",
            Category::Vehicles => "vehicles",
        };
        text(
            a,
            &consumer,
            "arsenal.category",
            &row.game_id,
            "category",
            actual.into(),
            category,
            "InfantryTypes membership".into(),
        )?;
        for (field, actual, key) in [
            (
                "veteran_abilities",
                &m.veteran_abilities,
                "VeteranAbilities",
            ),
            ("elite_abilities", &m.elite_abilities, "EliteAbilities"),
        ] {
            let raw = optional(r, &row.section, key, "")?;
            let display = raw.value.clone();
            let expected = raw.try_map(|v| {
                unique_set(
                    csv(&v)?
                        .into_iter()
                        .map(|v| v.to_ascii_lowercase())
                        .collect(),
                    "source abilities",
                )
            })?;
            text(
                a,
                &consumer,
                &format!("arsenal.{field}"),
                &row.game_id,
                field,
                unique_set(
                    actual.iter().map(|v| v.id().to_owned()).collect(),
                    "actual abilities",
                )?,
                expected,
                display,
            )?;
        }
        let raw = optional(r, &row.section, "Ammo", "-1")?;
        let display = raw.value.clone();
        check_int(
            a,
            &consumer,
            "arsenal.ammo",
            &row.game_id,
            "ammo",
            i128::from(m.ammo),
            converted(raw, Conversion::Int)?.map(|v| v.max(0)),
            display,
        )?;
    }
    Ok(())
}

pub(super) fn choose_slot(
    r: &Reference,
    section: &str,
    keys: &[&str],
) -> AuditResult<Expected<Option<String>>> {
    let mut skipped = Expected::literal(());
    for key in keys {
        let raw = optional(r, section, key, "none")?;
        if raw.value != "none" {
            return Ok(raw.map(Some).because(&skipped));
        }
        skipped = skipped.because(&raw);
    }
    Ok(Expected::literal(None).because(&skipped))
}
pub(super) fn combat_terms(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    row: &MappingRow,
    weapons: &BTreeMap<String, String>,
) -> AuditResult<()> {
    let actual_item = item(d, row)?;
    let combat = match actual_item {
        ProductionItem::Unit(k) => d.unit_combat(k),
        ProductionItem::Building(k) => d.building_combat(k),
    }
    .ok_or_else(|| "missing combat terms".to_owned())?;
    let consumer = format!("{}@rules:{}", row.game_id, row.section);
    let raw = optional(r, &row.section, "Armor", "none")?;
    let display = raw.value.clone();
    let actual = d
        .armour_classes()
        .get(usize::from(combat.armour.0))
        .ok_or_else(|| "invalid actual armour".to_owned())?
        .clone();
    text(
        a,
        &consumer,
        "combat.items.armour",
        &row.game_id,
        "armour",
        actual,
        raw.map(|v| v.to_ascii_lowercase()),
        display,
    )?;
    let p = choose_slot(r, &row.section, &["Primary", "Weapon1"])?;
    let q = choose_slot(r, &row.section, &["Secondary"])?;
    let ep = choose_slot(r, &row.section, &["ElitePrimary", "EliteWeapon1"])?;
    let eq = choose_slot(r, &row.section, &["EliteSecondary"])?;
    let ep = if ep.value.is_some() {
        ep
    } else {
        p.clone().because(&ep)
    };
    let eq = if eq.value.is_some() {
        eq
    } else {
        q.clone().because(&eq)
    };
    for (elite, label, parts) in [
        (false, "weapons", [p, q]),
        (true, "elite_weapons", [ep, eq]),
    ] {
        let native = sequence(parts);
        // Compact historic difference display has exact positional meaning only without a
        // secondary-only hole. Both actual and source shapes are explicitly checked first.
        if native.value[0].is_none() && native.value[1].is_some() {
            return Err(
                "source secondary-only slot shape requires explicit new representation/disposition"
                    .into(),
            );
        }
        let source_display = native
            .value
            .iter()
            .flatten()
            .cloned()
            .collect::<Vec<_>>()
            .join(",");
        let expected = native.map(|v| {
            v.into_iter()
                .flatten()
                .map(|name| {
                    weapons
                        .get(&name)
                        .cloned()
                        .unwrap_or_else(|| format!("?{name}"))
                })
                .collect::<Vec<_>>()
                .join(",")
        });
        let slots = combat.weapons(elite);
        if slots[0].is_none() && slots[1].is_some() {
            return Err(
                "actual secondary-only hole is not an approved historic weapon difference".into(),
            );
        }
        let actual = slots
            .into_iter()
            .flatten()
            .map(|kind| weapon_id(d, kind))
            .collect::<AuditResult<Vec<_>>>()?
            .join(",");
        text(
            a,
            &consumer,
            &format!("combat.items.{label}"),
            &row.game_id,
            label,
            actual,
            expected,
            source_display,
        )?;
    }
    for (field, actual, key, c) in [
        (
            "air_range_bonus",
            i128::from(combat.air_range_bonus),
            "AirRangeBonus",
            Conversion::Cells,
        ),
        (
            "land_targeting",
            i128::from(combat.land_targeting),
            "LandTargeting",
            Conversion::Int,
        ),
        (
            "naval_targeting",
            i128::from(combat.naval_targeting),
            "NavalTargeting",
            Conversion::Int,
        ),
    ] {
        let raw = optional(r, &row.section, key, "0")?;
        let display = raw.value.clone();
        check_int(
            a,
            &consumer,
            &format!("combat.items.{field}"),
            &row.game_id,
            field,
            actual,
            converted(raw, c)?,
            display,
        )?;
    }
    let fields = if row.building {
        vec![("wall", combat.wall, "Wall")]
    } else {
        vec![
            ("underwater", combat.underwater, "Underwater"),
            ("organic", combat.organic, "Organic"),
        ]
    };
    for (field, actual, key) in fields {
        let raw = optional(r, &row.section, key, "no")?;
        let display = raw.value.clone();
        check_int(
            a,
            &consumer,
            &format!("combat.items.{field}"),
            &row.game_id,
            field,
            i128::from(actual),
            converted(raw, Conversion::Flag)?,
            display,
        )?;
    }
    if row.building && (combat.underwater || combat.organic) || !row.building && combat.wall {
        return Err("cross-kind combat traits".into());
    }
    Ok(())
}
pub(super) fn spawned(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    row: &MappingRow,
    ids: &BTreeMap<String, String>,
    weapons: &BTreeMap<String, String>,
) -> AuditResult<()> {
    let actual_item = item(d, row)?;
    let m = d
        .metadata(actual_item)
        .ok_or_else(|| "missing metadata".to_owned())?;
    let consumer = format!("{}@rules:{}", row.game_id, row.section);
    let spawn = if row.building {
        Expected::literal("none".to_owned())
    } else {
        optional(r, &row.section, "Spawns", "none")?
    };
    let expected = if row.building {
        Expected::literal("none".to_owned())
    } else if spawn.value == "none" {
        spawn.clone()
    } else {
        mapped(spawn.clone(), ids)?
    };
    let actual = m
        .spawns
        .map(|k| unit_id(d, k))
        .transpose()?
        .unwrap_or_else(|| "none".into());
    text(
        a,
        &consumer,
        "arsenal.spawns",
        &row.game_id,
        "spawns",
        actual,
        expected,
        spawn.value.clone(),
    )?;
    for (field, actual, key, frames) in [
        ("spawns_number", m.spawns_number, "SpawnsNumber", false),
        ("spawn_regen", m.spawn_regen, "SpawnRegenRate", true),
        ("spawn_reload", m.spawn_reload, "SpawnReloadRate", true),
    ] {
        let expected = if row.building || spawn.value == "none" {
            Expected::literal(0).because(&spawn)
        } else {
            let raw = optional(r, &row.section, key, "0")?;
            if frames {
                let n = super::actual::decimal(&raw.value)?.0;
                if n == 0 {
                    Expected::literal(0).because(&raw).because(&spawn)
                } else {
                    converted(raw, Conversion::Frames)?.because(&spawn)
                }
            } else {
                converted(raw, Conversion::Int)?.because(&spawn)
            }
        };
        check_int(
            a,
            &consumer,
            &format!("arsenal.{field}"),
            &row.game_id,
            field,
            i128::from(actual),
            expected,
            format!("Spawns selector;{key}"),
        )?;
    }
    let mut launcher = Expected::literal("none".to_owned()).because(&spawn);
    if !row.building && spawn.value != "none" {
        // Secondary spawner has priority; evaluate Primary only if Secondary is not a spawner.
        for (key, label) in [("Secondary", "secondary"), ("Primary", "primary")] {
            let weapon = optional(r, &row.section, key, "none")?;
            if weapon.value == "none" {
                launcher = launcher.because(&weapon);
                continue;
            }
            let selector = converted(
                optional(r, &weapon.value, "Spawner", "no")?,
                Conversion::Flag,
            )?
            .because(&weapon);
            if selector.value == 1 {
                launcher = Expected::literal(label.to_owned())
                    .because(&launcher)
                    .because(&selector);
                break;
            }
            launcher = launcher.because(&selector);
        }
    }
    let actual = match m.launcher {
        None => "none",
        Some(sim::Launcher::Primary) => "primary",
        Some(sim::Launcher::Secondary) => "secondary",
    };
    text(
        a,
        &consumer,
        "arsenal.launcher",
        &row.game_id,
        "launcher",
        actual.into(),
        launcher,
        spawn.value.clone(),
    )?;
    let v3 = rule(r, "General", "V3RocketType")?;
    let missile = if row.building {
        Expected::literal("none".to_owned())
    } else if v3.value == row.section {
        Expected::literal("v3".to_owned()).because(&v3)
    } else {
        let dmis = rule(r, "General", "DMislType")?;
        let result = if dmis.value == row.section {
            "dmisl"
        } else {
            "none"
        };
        Expected::literal(result.to_owned())
            .because(&v3)
            .because(&dmis)
    };
    let actual = match m.missile {
        None => "none",
        Some(sim::Missile::V3) => "v3",
        Some(sim::Missile::DMisl) => "dmisl",
    };
    text(
        a,
        &consumer,
        "arsenal.missile",
        &row.game_id,
        "missile",
        actual.into(),
        missile,
        row.section.clone(),
    )?;
    // DeathWeapon fallback is only BalloonHover=yes primary when explicit DeathWeapon absent.
    let explicit = optional(r, &row.section, "DeathWeapon", "none")?;
    let death = if explicit.value != "none" {
        explicit
    } else {
        let balloon = converted(
            optional(r, &row.section, "BalloonHover", "no")?,
            Conversion::Flag,
        )?;
        if balloon.value == 1 {
            optional(r, &row.section, "Primary", "none")?
                .because(&balloon)
                .because(&explicit)
        } else {
            Expected::literal("none".to_owned())
                .because(&balloon)
                .because(&explicit)
        }
    };
    let death = death.map(|native| {
        if native == "none" {
            "none".to_owned()
        } else {
            weapons
                .get(&native)
                .cloned()
                .unwrap_or_else(|| format!("?{native}"))
        }
    });
    let actual = m
        .death_weapon
        .map(|k| weapon_id(d, k))
        .transpose()?
        .unwrap_or_else(|| "none".into());
    text(
        a,
        &consumer,
        "arsenal.death_weapon",
        &row.game_id,
        "death_weapon",
        actual,
        death,
        "DeathWeapon or BalloonHover primary fallback".into(),
    )?;
    Ok(())
}
pub(super) fn owner_sides(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    row: &MappingRow,
) -> AuditResult<()> {
    let m = d
        .metadata(item(d, row)?)
        .ok_or_else(|| "missing metadata".to_owned())?;
    let consumer = format!("{}@rules:{}", row.game_id, row.section);
    let owner = optional(r, &row.section, "Owner", "")?;
    let display = owner.value.clone();
    let allied = rule(r, "Sides", "GDI")?.try_map(|v| csv(&v))?;
    let soviet = rule(r, "Sides", "Nod")?.try_map(|v| csv(&v))?;
    let expected = if owner.value.is_empty() {
        Expected::literal(String::new()).because(&owner)
    } else {
        owner
            .zip(allied)
            .zip(soviet)
            .try_map(|((owner, allied), soviet)| {
                let owners = csv(&owner)?;
                let _ = unique_set(owners.clone(), "source Owner houses")?;
                let mut sides = BTreeSet::new();
                for house in owners {
                    if allied.contains(&house) {
                        sides.insert("allied".to_owned());
                    }
                    if soviet.contains(&house) {
                        sides.insert("soviet".to_owned());
                    }
                }
                // Multiple Owner houses intentionally project to one side; this is not silently
                // dropping duplicate source tokens (those were rejected above).
                Ok(sides.into_iter().collect::<Vec<_>>().join(","))
            })?
    };
    text(
        a,
        &consumer,
        "arsenal.owner_sides",
        &row.game_id,
        "owner_sides",
        unique_set(
            m.owner_sides.iter().map(|s| s.id().into()).collect(),
            "actual owner sides",
        )?,
        expected,
        display,
    )?;
    Ok(())
}

pub(super) fn factory_source(r: &Reference, section: &str) -> AuditResult<Expected<String>> {
    let factory = optional(r, section, "Factory", "")?;
    let expected = match factory.value.as_str() {
        "" => Expected::literal("none".to_owned()),
        "BuildingType" => Expected::literal("buildings".into()),
        "InfantryType" => Expected::literal("infantry".into()),
        "AircraftType" => Expected::literal("aircraft".into()),
        "UnitType" => converted(optional(r, section, "Naval", "no")?, Conversion::Flag)?
            .map(|n| if n == 1 { "naval" } else { "vehicles" }.to_owned()),
        other => return Err(format!("unknown source Factory {other}")),
    };
    Ok(expected.because(&factory))
}
pub(super) fn factory_and_producer(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    row: &MappingRow,
    rows: &[MappingRow],
) -> AuditResult<()> {
    let actual_item = item(d, row)?;
    let m = d
        .metadata(actual_item)
        .ok_or_else(|| "missing metadata".to_owned())?;
    let consumer = format!("{}@rules:{}", row.game_id, row.section);
    let factory = if row.building {
        factory_source(r, &row.section)?
    } else {
        Expected::literal("none".to_owned())
    };
    let actual = match m.factory {
        None => "none",
        Some(sim::arsenal::FactoryGroup::Buildings) => "buildings",
        Some(sim::arsenal::FactoryGroup::Infantry) => "infantry",
        Some(sim::arsenal::FactoryGroup::Vehicles) => "vehicles",
        Some(sim::arsenal::FactoryGroup::Naval) => "naval",
        Some(sim::arsenal::FactoryGroup::Aircraft) => "aircraft",
    };
    text(
        a,
        &consumer,
        "arsenal.factory",
        &row.game_id,
        "factory",
        actual.into(),
        factory,
        "Factory/conditional Naval".into(),
    )?;
    let sides = rows
        .iter()
        .filter(|r| r.game_id == row.game_id)
        .map(|r| r.side.clone())
        .collect::<BTreeSet<_>>();
    let side = if sides.len() == 1 {
        row.side.as_str()
    } else {
        "none"
    };
    let actual = m.side.map_or("none", |s| s.id());
    if actual != side {
        return Err(format!(
            "canonical mapped side {consumer}: {actual}!={side}"
        ));
    }
    a.game_owned(WitnessId::new(&consumer,"arsenal.side"),actual.into(),
        "Canonical mapping defines game type side; shared Engineer/Dog combines both native sides as none. Actual Owner sides and all native twin consumers are separately compared".into(),
        "committed ra2_mapping.tsv plus owner_sides adapter".into())?;
    let wanted = if row.building {
        Expected::literal("buildings".to_owned())
    } else {
        let infantry = registered(r, "InfantryTypes", &row.section)?;
        if infantry.value {
            Expected::literal("infantry".to_owned()).because(&infantry)
        } else {
            let naval = converted(optional(r, &row.section, "Naval", "no")?, Conversion::Flag)?;
            if naval.value == 1 {
                Expected::literal("naval".to_owned())
                    .because(&infantry)
                    .because(&naval)
            } else {
                let pads = rule(r, "General", "PadAircraft")?.try_map(|v| csv(&v))?;
                pads.map(|v| {
                    if v.contains(&row.section) {
                        "aircraft"
                    } else {
                        "vehicles"
                    }
                    .to_owned()
                })
                .because(&infantry)
                .because(&naval)
            }
        }
    };
    let own_side = if side == "none" { "allied" } else { side };
    let mut candidates = Vec::new();
    for other in rows.iter().filter(|r| r.building) {
        let factory = factory_source(r, &other.section)?;
        let selected = if factory.value == wanted.value {
            Some((other.game_id.clone(), other.side.clone()))
        } else {
            None
        };
        candidates.push(Expected::literal(selected).because(&factory));
    }
    let allowed = sequence(candidates)
        .try_map(|values| {
            let all = values.into_iter().flatten().collect::<Vec<_>>();
            let own = all
                .iter()
                .filter(|(_, side)| side == own_side)
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>();
            let chosen = if own.is_empty() {
                all.into_iter().map(|(id, _)| id).collect()
            } else {
                own
            };
            if chosen.is_empty() {
                return Err("no native-reference acceptable producer".into());
            }
            let _ = unique_set(chosen.clone(), "acceptable producers")?;
            Ok(chosen)
        })?
        .because(&wanted);
    let terms = d
        .production(actual_item)
        .ok_or_else(|| "missing actual production".to_owned())?;
    // check_member validates the actual producer against an independently built allowed set;
    // it never sets expected=actual merely because membership happened to be allowed.
    a.check_member(
        WitnessId::new(&consumer, "economy.production.producer"),
        building_id(d, terms.producer)?,
        allowed,
    )?;
    let lane = if row.building {
        optional(r, &row.section, "BuildCat", "")?.map(|v| {
            if v == "Combat" {
                "defences"
            } else {
                "buildings"
            }
            .to_owned()
        })
    } else {
        Expected::literal("buildings".into())
    };
    let actual = match d.lane(actual_item) {
        sim::Lane::Main => "buildings",
        sim::Lane::Defences => "defences",
    };
    text(
        a,
        &consumer,
        "economy.production.lane",
        &row.game_id,
        "lane",
        actual.into(),
        lane,
        "BuildCat; actual Definitions::lane".into(),
    )?;
    Ok(())
}
pub(super) fn docks(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    row: &MappingRow,
    rows: &[MappingRow],
) -> AuditResult<()> {
    let m = d
        .metadata(item(d, row)?)
        .ok_or_else(|| "missing metadata".to_owned())?;
    let consumer = format!("{}@rules:{}", row.game_id, row.section);
    let mut users = Vec::new();
    if row.building {
        for other in rows.iter().filter(|r| !r.building) {
            let bound = converted(
                optional(r, &other.section, "AirportBound", "no")?,
                Conversion::Flag,
            )?;
            let uses = if bound.value == 1 {
                optional(r, &other.section, "Dock", "")?
                    .try_map(|v| csv(&v))?
                    .map(|v| v.contains(&row.section))
                    .because(&bound)
            } else {
                Expected::literal(false).because(&bound)
            };
            users.push(uses);
        }
    }
    let users = sequence(users).map(|v| v.into_iter().any(|v| v));
    let expected = if users.value {
        converted(
            optional(r, &row.section, "NumberOfDocks", "0")?,
            Conversion::Int,
        )?
        .because(&users)
    } else {
        Expected::literal(0).because(&users)
    };
    check_int(
        a,
        &consumer,
        "arsenal.docks",
        &row.game_id,
        "docks",
        i128::from(m.docks),
        expected,
        "reverse AirportBound/Dock consumers; NumberOfDocks active only for actual pad types"
            .into(),
    )?;
    let repair = converted(
        optional(r, &row.section, "UnitRepair", "no")?,
        Conversion::Flag,
    )?;
    let expected = if repair.value == 1 {
        converted(
            optional(r, &row.section, "NumberOfDocks", "0")?,
            Conversion::Int,
        )?
        .because(&repair)
    } else {
        Expected::literal(0).because(&repair)
    };
    check_int(
        a,
        &consumer,
        "arsenal.vehicle_docks",
        &row.game_id,
        "vehicle_docks",
        i128::from(m.vehicle_docks),
        expected,
        "UnitRepair; NumberOfDocks active branch".into(),
    )?;
    // This audits the shipped pad-count derivation, not a stored per-aircraft Dock whitelist:
    // the runtime uses factory/side pad logic. That broader behavior needs its own disposition.
    Ok(())
}
