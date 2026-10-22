//! Historical adapter-draft note: UNVALIDATED adapter draft. Uses actual sim::Definitions access patterns already present
//! Historical adapter-draft note: in ra9_data_parity.rs. Schema10 metadata.size_limit and death_weapon_damage_pm are intentionally pending integration.
//! Historical adapter-draft note: Do not compile/run until explicit RA8 closure and canonical RA9 release.
use super::{Audit, AuditResult, Expected, Reference, SourceKey, WitnessId};
use sim::{Definitions, ProductionItem};

pub(super) fn number(value: Expected<String>) -> AuditResult<Expected<i128>> {
    value.try_map(|raw| {
        raw.parse::<i128>()
            .map_err(|e| format!("integer {raw:?}: {e}"))
    })
}
pub(super) fn positive_round(n: i128, d: i128) -> AuditResult<i128> {
    if n < 0 || d <= 0 {
        return Err("positive exact conversion domain".into());
    }
    n.checked_mul(2)
        .and_then(|v| v.checked_add(d))
        .and_then(|v| d.checked_mul(2).map(|div| v / div))
        .ok_or_else(|| "conversion arithmetic overflow".into())
}
pub(super) fn decimal(raw: &str) -> AuditResult<(i128, i128)> {
    let raw = raw.trim_end_matches('%');
    let (sign, digits) = if let Some(rest) = raw.strip_prefix('-') {
        (-1, rest)
    } else {
        (1, raw)
    };
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    if !whole
        .chars()
        .chain(fraction.chars())
        .all(|c| c.is_ascii_digit())
    {
        return Err(format!("bad decimal {raw:?}"));
    }
    let exponent = u32::try_from(fraction.len()).map_err(|_| "decimal too long")?;
    let den = 10i128
        .checked_pow(exponent)
        .ok_or_else(|| "decimal overflow".to_owned())?;
    let whole = if whole.is_empty() {
        0
    } else {
        whole.parse::<i128>().map_err(|e| e.to_string())?
    };
    let fraction = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<i128>().map_err(|e| e.to_string())?
    };
    let n = whole
        .checked_mul(den)
        .and_then(|n| n.checked_add(fraction))
        .and_then(|n| n.checked_mul(sign))
        .ok_or_else(|| "decimal overflow".to_owned())?;
    Ok((n, den))
}
pub(super) fn scaled(
    value: Expected<String>,
    factor: i128,
    divisor: i128,
) -> AuditResult<Expected<i128>> {
    value.try_map(|raw| {
        let (n, d) = decimal(&raw)?;
        let sign = n.signum();
        let a = n
            .checked_abs()
            .and_then(|n| n.checked_mul(factor))
            .ok_or_else(|| "scale overflow".to_owned())?;
        let b = d
            .checked_mul(divisor)
            .ok_or_else(|| "scale divisor overflow".to_owned())?;
        positive_round(a, b).and_then(|n| {
            n.checked_mul(sign)
                .ok_or_else(|| "signed scale overflow".into())
        })
    })
}
pub(super) fn rule(r: &Reference, section: &str, key: &str) -> AuditResult<Expected<String>> {
    r.expect(SourceKey::new("rules", section, key))
}
pub(super) fn optional(
    r: &Reference,
    section: &str,
    key: &str,
    default: &str,
) -> AuditResult<Expected<String>> {
    r.expect_or(SourceKey::new("rules", section, key), default)
}
#[allow(
    clippy::too_many_arguments,
    reason = "Independent actual, expected, difference and evidence inputs remain explicit at the audit boundary"
)]
pub(super) fn check_int(
    a: &mut Audit,
    consumer: &str,
    path: &str,
    game_id: &str,
    field: &str,
    actual: i128,
    expected: Expected<i128>,
    source_display: String,
) -> AuditResult<()> {
    a.check(
        WitnessId::new(consumer, path),
        game_id,
        field,
        actual.to_string(),
        expected.map(|v| v.to_string()),
        source_display,
    )
}

// Call separately for EVERY mapping row. Never deduplicate shared game IDs first.
pub(super) fn core_item(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    game_id: &str,
    section: &str,
    building: bool,
) -> AuditResult<()> {
    let consumer = format!("{game_id}@rules:{section}");
    let item = if building {
        d.building_kind(game_id).map(ProductionItem::Building)
    } else {
        d.unit_kind(game_id).map(ProductionItem::Unit)
    }
    .ok_or_else(|| format!("mapped item not shipped {consumer}"))?;
    let terms = d
        .production(item)
        .ok_or_else(|| format!("no production for {consumer}"))?;
    let combat = match item {
        ProductionItem::Building(k) => d.building_combat(k),
        ProductionItem::Unit(k) => d.unit_combat(k),
    }
    .ok_or_else(|| format!("no combat terms for {consumer}"))?;
    let entry = d
        .metadata(item)
        .ok_or_else(|| format!("no metadata for {consumer}"))?;
    for (path, field, key, actual, default) in [
        (
            "economy.production.cost",
            "cost",
            "Cost",
            i128::from(terms.cost),
            "0",
        ),
        (
            "combat.hit_points",
            "strength",
            "Strength",
            i128::from(combat.hit_points),
            "<required>",
        ),
        (
            "combat.sight",
            "sight",
            "Sight",
            i128::from(combat.sight),
            "0",
        ),
        (
            "arsenal.tech_level",
            "tech_level",
            "TechLevel",
            i128::from(entry.tech_level),
            "<required>",
        ),
        (
            "arsenal.size",
            "size",
            "Size",
            i128::from(entry.size),
            if building { "0" } else { "1" },
        ),
    ] {
        let expected = if default == "<required>" {
            rule(r, section, key)?
        } else {
            optional(r, section, key, default)?
        };
        let display = expected.value.clone();
        check_int(
            a,
            &consumer,
            path,
            game_id,
            field,
            actual,
            number(expected)?,
            display,
        )?;
    }
    // Current code previously materialized BuildSpeed/WallCoefficient without tracked
    // provenance. The complete expression now carries Cost, BuildSpeed, Wall selector and,
    // ONLY for actual walls, WallBuildSpeedCoefficient.
    let cost = number(optional(r, section, "Cost", "0")?)?;
    let build = scaled(rule(r, "General", "BuildSpeed")?, 1200, 1)?;
    let wall = if building {
        optional(r, section, "Wall", "no")?
    } else {
        Expected::literal("no".to_owned())
    };
    let base = cost.zip(build).try_map(|(cost, ticks)| {
        cost.checked_mul(ticks)
            .ok_or_else(|| "build-time overflow".to_owned())
            .and_then(|v| positive_round(v, 1000))
            .map(|v| v.max(1))
    })?;
    let expected = if building && wall.value == "yes" {
        base.zip(scaled(
            rule(r, "General", "WallBuildSpeedCoefficient")?,
            1000,
            1,
        )?)
        .try_map(|(base, multiplier)| {
            base.checked_mul(multiplier)
                .ok_or_else(|| "wall build-time overflow".to_owned())
                .and_then(|v| positive_round(v, 1000))
                .map(|v| v.max(1))
        })?
        .because(&wall)
    } else {
        base.because(&wall)
    };
    check_int(
        a,
        &consumer,
        "economy.production.build_time",
        game_id,
        "build_time",
        i128::from(terms.build_time),
        expected,
        "Cost;BuildSpeed;Wall;conditional WallBuildSpeedCoefficient".into(),
    )?;
    Ok(())
}

// Powerups: declare exact3 or exact4 per named content type (see explicit shape table below). Even a disabled share gets compared against actual absence;
// animation and water flags have separate exact-value semantic dispositions. No full-row credit
// is granted merely by using share or argument.

// Explicit shape catalogue, independent of the observed tuple length. A new extra component
// cannot silently extend coverage. The existing source has optional fourth arguments.
pub(super) fn declare_powerup_shapes(r: &mut Reference) -> AuditResult<()> {
    for (key, len) in [
        ("Armor", 4),
        ("Cloak", 3),
        ("Darkness", 3),
        ("Explosion", 4),
        ("Firepower", 4),
        ("Gas", 4),
        ("HealBase", 3),
        ("ICBM", 3),
        ("Invulnerability", 4),
        ("IonStorm", 3),
        ("Money", 4),
        ("Napalm", 4),
        ("Pod", 3),
        ("Reveal", 3),
        ("Speed", 4),
        ("Squad", 3),
        ("Tiberium", 3),
        ("Unit", 3),
        ("Veteran", 4),
    ] {
        r.require_components(SourceKey::new("rules", "Powerups", key), len)?;
    }
    Ok(())
}

// Difficulty source arrays: declare exact3 before component access, compare all actual
// easy/medium/brutal slots; reject missing or extra source components.

// Ordered arrays: exact lengths plus actual ordered values; never zip without asserting lengths,
// never filter_map that silently discards a missing numbered IFV weapon.

// Integration checklist, intentionally not a fake passing exhaustive test:
// 1. Refactor 4 current named audit tests into functions accepting shared Reference/Audit/Definitions.
// 2. Enumerate actual source fields/item IDs and mapping consumers before any comparison.
// 3. Complete all direct/derived adapters from comparison-witness-plan.pending.tsv.
// 4. Add per-consumer requirements independently; direct reads are not witnesses.
// 5. Resolve Image to art section (fallback section name) and compare actual footprint width/height
//    or exact listed game-owned difference; Height has its own documented non-use disposition.
// 6. Audit actual 13 IFV modes/count and EliteWeapon substitutions; ModeTurretWeapon/Index each
//    has an explicit source-component/mode disposition (turret-art may be unused).
// 7. Audit whole rules arrays, prerequisite memberships, country order, side components,
//    registries and entire raw shipped schema. Own fields need field-specific active/inactive reason.
// 8. Integrate exact unused rows only after source verification. No UNCLASSIFIED_PENDING can pass.
// 9. Run all adapters once in exhaustive wrapper, finish once over whole reference/differences.
//10. Existing named wrappers remain and use scoped reconciliation, not pre-marked foreign rows.
//11. Runtime metadata strength/radius is not universally effective mechanic: Iron Curtain duration
//    lives in Rules, Nuke damage/spread in native payload, radius6 active AI scoring stays distinct.
// Historical preparation note: 12. All tests/mutations remain UNRUN until explicit root release and coordinated source freeze.
