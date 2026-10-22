//! Historical preparation note: UNVALIDATED/PENDING native-key comparison bodies. No compile/simulation executed.
//! These inspect actual loaded public fields; behavior/timing proof is explicitly separate.
use super::actual::{check_int, optional, rule};
use super::combat::{Conversion, converted};
use super::rules::{MappingRow, text};
use super::{Audit, AuditResult, Reference, SourceKey};
use sim::{Definitions, ProductionItem};
pub(super) fn mapped_item(d: &Definitions, row: &MappingRow) -> AuditResult<ProductionItem> {
    if row.building {
        d.building_kind(&row.game_id).map(ProductionItem::Building)
    } else {
        d.unit_kind(&row.game_id).map(ProductionItem::Unit)
    }
    .ok_or_else(|| format!("missing actual mapped item {}", row.game_id))
}
pub(super) fn missile_marker(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    row: &MappingRow,
) -> AuditResult<()> {
    let m = d
        .metadata(mapped_item(d, row)?)
        .ok_or_else(|| "missing metadata".to_owned())?;
    let consumer = format!("{}@rules:{}", row.game_id, row.section);
    let raw = optional(r, &row.section, "MissileSpawn", "no")?;
    let display = raw.value.clone();
    check_int(
        a,
        &consumer,
        "derived.arsenal.missile_spawn_marker",
        &row.game_id,
        "missile_spawn_marker",
        i128::from(m.missile.is_some()),
        converted(raw, Conversion::Flag)?,
        display,
    )?;
    // Current TSV has MissileSpawn=yes only on DMISL and V3ROCKET. The V3/DRED parent
    // sections do not set it. Per ModEnc oldid33685 it is the LAUNCHER flag that changes20
    // to10frames. The spawnee marker here audits type only, not scheduled kills/survival.
    Ok(())
}
pub(super) fn movement_restriction(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    row: &MappingRow,
    rows: &[MappingRow],
) -> AuditResult<()> {
    let Some(value) = r.rows.get(&SourceKey::new(
        "rules",
        &row.section,
        "MovementRestrictedTo",
    )) else {
        return Ok(());
    };
    if value != "Water" {
        return Err(format!(
            "new native restricted LandType requires explicit adapter {value}"
        ));
    }
    let restriction = rule(r, &row.section, "MovementRestrictedTo")?;
    let actual_item = mapped_item(d, row)?;
    let m = d
        .metadata(actual_item)
        .ok_or_else(|| "missing metadata".to_owned())?;
    let consumer = format!("{}@rules:{}", row.game_id, row.section);
    let zone = optional(r, &row.section, "MovementZone", "")?;
    if zone.value != "Fly" {
        // HYD and SQD: source Water restriction agrees with actual naval navigation domain.
        text(
            a,
            &consumer,
            "derived.arsenal.restricted_terrain",
            &row.game_id,
            "restricted_terrain",
            format!("{:?}", m.domain),
            restriction.map(|_| "Naval".into()).because(&zone),
            "MovementRestrictedTo=Water; ground path".into(),
        )?;
        return Ok(());
    }
    text(
        a,
        &consumer,
        "derived.arsenal.restricted_air_domain",
        &row.game_id,
        "restricted_air_domain",
        format!("{:?}", m.domain),
        zone.clone().map(|_| "Air".into()),
        "source MovementZone Fly; actual air domain".into(),
    )?;
    let ProductionItem::Unit(child) = actual_item else {
        return Err("air restricted building".into());
    };
    let parents = rows
        .iter()
        .filter(|parent| {
            r.rows
                .get(&SourceKey::new("rules", &parent.section, "Spawns"))
                .is_some_and(|v| v == &row.section)
        })
        .collect::<Vec<_>>();
    if parents.is_empty() {
        return Err(
            "source restricted spawned aircraft has no documented parent; no made-up landing proof"
                .into(),
        );
    }
    for parent in parents {
        let p = d
            .metadata(mapped_item(d, parent)?)
            .ok_or_else(|| "missing parent metadata".to_owned())?;
        if p.spawns != Some(child) {
            return Err("actual parent-to-child spawn relation differs".into());
        }
        let link = rule(r, &parent.section, "Spawns")?;
        text(
            a,
            &consumer,
            &format!("derived.recovery_parent.{}.domain", parent.game_id),
            &row.game_id,
            "recovery_parent_domain",
            format!("{:?}", p.domain),
            restriction.clone().map(|_| "Naval".into()).because(&link),
            format!("MovementRestrictedTo Water; {} Spawns", parent.section),
        )?;
    }
    // Aircraft fly across land freely (native restriction applies only to landing). This game
    // has no numeric landing state: it recovers near its naval parent into the bay, flight.rs.
    // Above comparison audits that recovery configuration; it is not exact native air-descent.
    Ok(())
}
pub(super) fn default_death_fallback(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    row: &MappingRow,
) -> AuditResult<()> {
    let explodes = converted(
        optional(r, &row.section, "Explodes", "no")?,
        Conversion::Flag,
    )?;
    if explodes.value == 0 {
        return Ok(());
    }
    let explicit = optional(r, &row.section, "DeathWeapon", "none")?;
    let primary = optional(r, &row.section, "Primary", "none")?;
    let numbered = optional(r, &row.section, "Weapon1", "none")?;
    if explicit.value != "none" || primary.value != "none" || numbered.value != "none" {
        return Ok(());
    }
    // The two current source shipyards reach the global fallback. DefaultDeathWeapon has
    // Projectile/Warhead but no Damage: ModEnc Damage oldid33694 documents default0.
    let fallback = rule(r, "CombatDamage", "DeathWeapon")?;
    if !r
        .rows
        .keys()
        .any(|k| k.file == "rules" && k.section == fallback.value)
    {
        return Err("missing source default death weapon section".into());
    }
    let damage = converted(
        optional(r, &fallback.value, "Damage", "0")?,
        Conversion::Int,
    )?
    .because(&fallback)
    .because(&explodes)
    .because(&explicit)
    .because(&primary)
    .because(&numbered);
    let actual_item = mapped_item(d, row)?;
    let m = d
        .metadata(actual_item)
        .ok_or_else(|| "missing metadata".to_owned())?;
    let combat = match actual_item {
        ProductionItem::Unit(k) => d.unit_combat(k),
        ProductionItem::Building(k) => d.building_combat(k),
    }
    .ok_or_else(|| "missing actual combat terms".to_owned())?;
    let selected = if m.explodes {
        m.death_weapon.or(combat.weapons(false)[0])
    } else {
        m.death_weapon
    };
    let actual = selected
        .map(|k| {
            d.weapon(k)
                .map(|w| i128::from(w.damage))
                .ok_or_else(|| "invalid actual death weapon".to_owned())
        })
        .transpose()?
        .unwrap_or(0);
    check_int(
        a,
        &format!("{}@rules:{}", row.game_id, row.section),
        "derived.runtime.death_fallback_damage",
        &row.game_id,
        "death_fallback_damage",
        actual,
        damage,
        "CombatDamage DeathWeapon; missing Damage documented0".into(),
    )?;
    // Scope: zero effective damage, not claiming the default weapon identity/animation is
    // represented. Native scheduled shipyard destruction is a separate evidence obligation.
    Ok(())
}
