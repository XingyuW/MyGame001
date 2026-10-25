//! UNRUN actual valid-loader nested component/length mutations and semantic reorder controls.
//! A failing unchanged adapter or a mutant loader rejection cannot count as evidence.
use super::array_negatives::DataCopy;
use super::*;
fn positive() -> AuditResult<()> {
    let dir = super::super::data_dir();
    let input = Inputs::load(&dir, &super::super::reference_dir())?;
    let d = sim::load_ruleset(&dir).map_err(|e| format!("positive actual native setup: {e}"))?;
    nested_plans::run(&input, &d)
}
fn fails(copy: DataCopy, field: &str, kind: &str) -> AuditResult<()> {
    let (input, d) = copy.loaded()?;
    match nested_plans::run(&input, &d) {
        Ok(()) => Err("actual changed nested component escaped adapter".into()),
        Err(e) => {
            if e.contains(field) && e.contains(kind) {
                Ok(())
            } else {
                Err(format!("wrong failure, expected {field}/{kind}: {e}"))
            }
        }
    }
}
fn passes(copy: DataCopy) -> AuditResult<()> {
    let (input, d) = copy.loaded()?;
    nested_plans::run(&input, &d)
}
#[test]
fn ra9_actual_nonfirst_veteran_ability_removal_fails_native_set_completeness() -> AuditResult<()> {
    positive()?;
    let copy = DataCopy::edit("arsenal.ron", "(id: \"gi\",", "veteran_abilities", |v| {
        if v.len() != 5 || v[1] != "\"firepower\"" {
            return Err("actual nonfirst ability fixture".into());
        }
        let _ = v.remove(1);
        Ok(())
    })?;
    fails(
        copy,
        "gi@rules:E1/arsenal.ron.entries.veteran_abilities",
        "actual/expected/required array length mismatch",
    )
}
#[test]
fn ra9_actual_ability_set_reorder_preserves_native_membership() -> AuditResult<()> {
    positive()?;
    let copy = DataCopy::edit("arsenal.ron", "(id: \"gi\",", "veteran_abilities", |v| {
        if v.len() != 5 || v[1] == v[4] {
            return Err("actual distinct veteran ability fixture".into());
        }
        v.swap(1, 4);
        Ok(())
    })?;
    passes(copy)
}
#[test]
fn ra9_actual_nonfirst_owner_side_removal_fails_many_to_one_projection() -> AuditResult<()> {
    positive()?;
    let copy = DataCopy::edit(
        "arsenal.ron",
        "(id: \"gap_generator\",",
        "owner_sides",
        |v| {
            if v.len() != 2 || v[1] != "\"soviet\"" {
                return Err("actual nonfirst Owner side fixture".into());
            }
            let _ = v.remove(1);
            Ok(())
        },
    )?;
    fails(
        copy,
        "gap_generator@rules:GAGAP/arsenal.ron.entries.owner_sides",
        "actual/expected/required array length mismatch",
    )
}
#[test]
fn ra9_actual_owner_side_reorder_preserves_native_membership() -> AuditResult<()> {
    positive()?;
    let copy = DataCopy::edit(
        "arsenal.ron",
        "(id: \"gap_generator\",",
        "owner_sides",
        |v| {
            if v.len() != 2 {
                return Err("actual Owner sides fixture".into());
            }
            v.swap(0, 1);
            Ok(())
        },
    )?;
    passes(copy)
}
#[test]
fn ra9_actual_required_country_association_change_is_not_a_new_native_mapping() -> AuditResult<()> {
    positive()?;
    let copy = DataCopy::edit(
        "arsenal.ron",
        "(id: \"black_eagle\",",
        "required_countries",
        |v| {
            if v.len() != 1 || v[0] != "\"korea\"" {
                return Err("actual unique required country fixture".into());
            }
            v[0] = "\"america\"".into();
            Ok(())
        },
    )?;
    fails(
        copy,
        "black_eagle@rules:BEAG/arsenal.ron.entries.required_countries",
        "actual physical array component differs",
    )
}
#[test]
fn ra9_actual_forbidden_country_association_change_keeps_the_source_identity_fixed()
-> AuditResult<()> {
    positive()?;
    let copy = DataCopy::edit(
        "arsenal.ron",
        "(id: \"harrier\",",
        "forbidden_countries",
        |v| {
            if v.len() != 1 || v[0] != "\"korea\"" {
                return Err("actual forbidden country fixture".into());
            }
            v[0] = "\"america\"".into();
            Ok(())
        },
    )?;
    fails(
        copy,
        "harrier@rules:ORCA/arsenal.ron.entries.forbidden_countries",
        "actual physical array component differs",
    )
}
#[test]
fn ra9_actual_nonfirst_group_building_removal_fails_native_component_length() -> AuditResult<()> {
    positive()?;
    let copy = DataCopy::edit("rules.ron", "(id: \"power\",", "buildings", |v| {
        if v.len() != 3 || v[1] != "\"tesla_reactor\"" {
            return Err("actual native group member fixture".into());
        }
        let _ = v.remove(1);
        Ok(())
    })?;
    fails(
        copy,
        "group.power/rules.ron.prerequisite_groups.buildings",
        "actual/expected/required array length mismatch",
    )
}
#[test]
fn ra9_actual_group_member_reorder_is_a_keyed_set_pass_control() -> AuditResult<()> {
    positive()?;
    let copy = DataCopy::edit("rules.ron", "(id: \"power\",", "buildings", |v| {
        if v.len() != 3 {
            return Err("actual power group fixture".into());
        }
        v.swap(1, 2);
        Ok(())
    })?;
    passes(copy)
}
#[test]
fn ra9_actual_nonfirst_crate_candidate_reorder_fails_canonical_unit_order() -> AuditResult<()> {
    positive()?;
    let copy = DataCopy::edit_crate_ids(|v| {
        if v.len() != 10 || v[1] != "\"war_miner\"" || v[2] != "\"v3_launcher\"" {
            return Err("actual CrateUnits order fixture".into());
        }
        v.swap(1, 2);
        Ok(())
    })?;
    fails(
        copy,
        "rules.ron.crate_units.ordered",
        "actual physical array component differs",
    )
}
#[test]
fn ra9_actual_starting_pool_extra_second_item_fails_active_native_eligibility_length()
-> AuditResult<()> {
    positive()?;
    let copy = DataCopy::edit(
        "skirmish.ron",
        "(side: \"allied\",",
        "starting_vehicles",
        |v| {
            if v.len() != 1 || v[0] != "\"grizzly\"" {
                return Err("actual shipped singleton start pool fixture".into());
            }
            v.push("\"ifv\"".into());
            Ok(())
        },
    )?;
    fails(
        copy,
        "start.allied/skirmish.ron.sides.starting_vehicles",
        "actual/expected/required array length mismatch",
    )
}
#[test]
fn ra9_actual_second_side_tuple_count_change_fails_its_native_source_field() -> AuditResult<()> {
    positive()?;
    let copy = DataCopy::replace_row(
        "rules.ron",
        "(id: \"soviet\",",
        "paradrop_count: 9",
        "paradrop_count: 8",
    )?;
    fails(
        copy,
        "side.soviet/rules.ron.sides.fields",
        "actual physical array component differs",
    )
}
#[test]
fn ra9_actual_powerup_argument_change_fails_native_tuple_component3() -> AuditResult<()> {
    positive()?;
    let copy = DataCopy::replace_row("rules.ron", "(id: \"speed\",", "value: 1200", "value: 1201")?;
    fails(
        copy,
        "powerup.speed/rules.ron.powerups.fields",
        "actual physical array component differs",
    )
}
#[test]
fn ra9_actual_nonfirst_game_army_rotation_change_remains_explicit_game_policy() -> AuditResult<()> {
    positive()?;
    let copy = DataCopy::edit(
        "ai.ron",
        "army: [\"grizzly\", \"gi\", \"grizzly\", \"sniper\",",
        "army",
        |v| {
            if v.len() != 14 || v[3] != "\"sniper\"" || v[4] != "\"prism_tank\"" {
                return Err("actual British army policy fixture".into());
            }
            v.swap(3, 4);
            Ok(())
        },
    )?;
    fails(
        copy,
        "ai.ron.plans[6].army",
        "actual physical array component differs",
    )
}
#[test]
fn ra9_actual_nonfirst_authored_start_x_change_fails_tuple_origin_policy() -> AuditResult<()> {
    positive()?;
    let copy = DataCopy::edit("skirmish.ron", "(id: \"demo\",", "starts", |v| {
        let point = v.get_mut(1).ok_or("actual second authored start")?;
        if point != "(x: 52, y: 42)" {
            return Err("actual second map tuple fixture".into());
        }
        *point = "(x: 53, y: 42)".into();
        Ok(())
    })?;
    fails(
        copy,
        "skirmish.map.demo.starts.xy",
        "actual physical array component differs",
    )
}
#[test]
fn ra9_actual_legacy_strike_activation_cannot_receive_an_automatic_own_disposition()
-> AuditResult<()> {
    positive()?;
    let copy = DataCopy::scalar(
        "arsenal.ron",
        "(id: \"harrier\",",
        "role: \"combat\"",
        "role: \"strike\"",
    )?;
    fails(
        copy,
        "legacy Strike/Carrier native policy unresolved",
        "Strike: rearming cooldown after burst",
    )
}
#[test]
fn ra9_actual_legacy_carrier_activation_cannot_receive_an_automatic_own_disposition()
-> AuditResult<()> {
    positive()?;
    let copy = DataCopy::scalar(
        "arsenal.ron",
        "(id: \"aircraft_carrier\",",
        "role: \"combat\"",
        "role: \"carrier\"",
    )?;
    fails(
        copy,
        "legacy Strike/Carrier native policy unresolved",
        "Carrier: capacity and first Strike definition",
    )
}

#[test]
fn ra9_actual_starting_cost_change_reaches_exact_native_cost_component_before_mean()
-> AuditResult<()> {
    positive()?;
    let copy = DataCopy::replace_row(
        "economy.ron",
        "(unit: \"grizzly\",",
        "cost: 700,",
        "cost: 701,",
    )?;
    fails(
        copy,
        "derived.starting.native_costs",
        "actual physical array component differs",
    )
}
