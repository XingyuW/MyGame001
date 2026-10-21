//! UNCOMPILED/UNRUN five real S schema10 fields only. All actual source defaults and
//! physical consumers remain independent; no native behavioral coverage is inferred here.
use super::*;
const FIELDS: [(&str, &str, &str); 5] = [
    ("size_limit", "SizeLimit", "0"),
    ("death_weapon_damage_pm", "DeathWeaponDamageModifier", "1"),
    ("type_immune", "TypeImmune", "no"),
    ("bomb_sight", "BombSight", "0"),
    ("reveal_to_all", "RevealToAll", "no"),
];
pub(super) fn declare(a: &mut Audit, r: &Reference, row: &MappingRow) -> AuditResult<()> {
    let c = format!("{}@rules:{}", row.game_id, row.section);
    for (field, key, _) in FIELDS {
        let id = a.physical(WitnessId::new(&c, &format!("arsenal.{field}")));
        if !a.required.contains(&id) {
            return Err(format!("schema10 real raw physical field absent {id:?}"));
        }
        let key = SourceKey::new("rules", &row.section, key);
        if r.rows.contains_key(&key) {
            a.require_dependency(
                id,
                Atom {
                    key,
                    portion: Portion::Whole,
                },
            )?;
        }
    }
    Ok(())
}
pub(super) fn project(
    a: &mut Audit,
    r: &Reference,
    d: &Definitions,
    row: &MappingRow,
) -> AuditResult<()> {
    let m = d
        .metadata(items::item(d, row)?)
        .ok_or("schema10 actual metadata absent")?;
    let c = format!("{}@rules:{}", row.game_id, row.section);
    for (field, key, default) in FIELDS {
        let actual = match field {
            "size_limit" => i128::from(m.size_limit),
            "death_weapon_damage_pm" => i128::from(m.death_weapon_damage_pm),
            "type_immune" => i128::from(m.type_immune),
            "bomb_sight" => i128::from(m.bomb_sight),
            "reveal_to_all" => i128::from(m.reveal_to_all),
            _ => return Err("schema10 exact field catalogue".into()),
        };
        let source = actual::optional(r, &row.section, key, default)?;
        let display = source.value.clone();
        let conversion = match field {
            "type_immune" | "reveal_to_all" => combat::Conversion::Flag,
            "death_weapon_damage_pm" => combat::Conversion::Permille,
            _ => combat::Conversion::Int,
        };
        actual::check_int(
            a,
            &c,
            &format!("arsenal.{field}"),
            &row.game_id,
            field,
            actual,
            combat::converted(source, conversion)?,
            display,
        )?;
    }
    Ok(())
}
