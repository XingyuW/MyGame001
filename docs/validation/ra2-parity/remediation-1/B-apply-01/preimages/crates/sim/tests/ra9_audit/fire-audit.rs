//! UNVALIDATED source/provenance contracts. Integrate inside ra9_audit only after native runs.
//! No source retrieval, branch premise or recognized key by itself grants coverage.
use super::fire_keys::Evidence;
use super::*;
const BASE_SECTION: &str = "MirageGun";
const ELITE_SECTION: &str = "MirageGunE";
fn false_only(r: &Reference, section: &str, key: &str) -> AuditResult<Expected<bool>> {
    // All three committed keys are PRESENT no. No generic absent/default or true semantics
    // is invented; a missing key or changed value invalidates this narrow supported contract.
    super::actual::rule(r,section,key)?.try_map(|v|if v=="no"{Ok(false)}else{Err(format!("{section}/{key}: only the exact shipped no case has evidence; reassess source semantics"))})
}
fn declare_source(
    a: &mut Audit,
    consumer: &str,
    field: &str,
    section: &str,
    key: &str,
) -> AuditResult<()> {
    let id = WitnessId::new(consumer, field);
    a.require(id.clone())?;
    a.require_dependency(
        id,
        Atom {
            key: SourceKey::new("rules", section, key),
            portion: Portion::Whole,
        },
    )
}
pub(super) fn declare(a: &mut Audit) -> AuditResult<()> {
    declare_source(
        a,
        "weapon.mirage_gun@rules:MirageGun",
        "runtime.false_disguise_fire_restriction",
        BASE_SECTION,
        "DisguiseFireOnly",
    )?;
    declare_source(
        a,
        "rules.player_return_fire",
        "runtime.manual_move_hit_response",
        "CombatDamage",
        "PlayerReturnFire",
    )
}
fn mirage_native_premises(e: &super::fire_keys::MirageObservation, elite: bool) -> AuditResult<()> {
    if e.elite != elite
        || !e.initial_tree
        || !e.moved_out_of_tree
        || e.shot.tree
        || e.shot.hidden_to_target_owner
        || e.event_weapon != "mirage_gun"
        || e.actual_primary != e.event_weapon
        || e.shot.tick == 0
        || e.shot.weapon != e.shot.primary
    {
        return Err(format!(
            "actual native Mirage undisguised firing premise missing: {e:?}"
        ));
    }
    if !matches!(e.shot.order,Some(sim::AttackOrder::Attack{target,..})if target==e.shot.target) {
        return Err("native Mirage fire is not the actual explicit target".into());
    }
    let _actual_motion_sample = (e.shot.stepping, e.shot.destination);
    if elite && e.actual_secondary.as_deref() != Some("mirage_gun_e") {
        return Err("native elite secondary mapping differs".into());
    }
    if !elite && e.actual_secondary.is_some() {
        return Err("base Mirage unexpectedly has an actual secondary".into());
    }
    Ok(())
}
pub(super) fn compare_base(a: &mut Audit, input: &Inputs, e: &Evidence) -> AuditResult<()> {
    mirage_native_premises(&e.mirage_base, false)?;
    mirage_native_premises(&e.mirage_elite, true)?;
    let expected = false_only(&input.reference, BASE_SECTION, "DisguiseFireOnly")?;
    let actual = e.mirage_base.shot.tree || e.mirage_base.shot.hidden_to_target_owner;
    a.check(
        WitnessId::new(
            "weapon.mirage_gun@rules:MirageGun",
            "runtime.false_disguise_fire_restriction",
        ),
        "weapon.mirage_gun",
        "disguise_fire_only",
        u8::from(actual).to_string(),
        expected.map(|v| u8::from(v).to_string()),
        "no".into(),
    )
}
// A's authoritative exact-unused table may contain ONLY the exact elite no row after this
// guard has run on actual loaded definitions AND fresh native evidence in the same audit.
// This function grants no global source credit and does not insert an unused row by itself.
pub(super) fn require_masked_elite(
    input: &Inputs,
    d: &Definitions,
    e: &super::fire_keys::MirageObservation,
) -> AuditResult<()> {
    let _present_false = false_only(&input.reference, ELITE_SECTION, "DisguiseFireOnly")?;
    mirage_native_premises(e, true)?;
    let r = &input.reference;
    if super::actual::rule(r, "MGTK", "Primary")?.value != BASE_SECTION
        || super::actual::rule(r, "MGTK", "EliteSecondary")?.value != ELITE_SECTION
    {
        return Err("native Mirage source slot identities changed".into());
    }
    for key in ["ElitePrimary", "EliteWeapon1"] {
        if super::actual::optional(r, "MGTK", key, "none")?.value != "none" {
            return Err("native Mirage elite-primary fallback selector changed".into());
        }
    }
    let base_id = index(&input.weapons)
        .get(BASE_SECTION)
        .cloned()
        .ok_or("source base weapon mapping")?;
    let elite_id = index(&input.weapons)
        .get(ELITE_SECTION)
        .cloned()
        .ok_or("source elite weapon mapping")?;
    if base_id != "mirage_gun" || elite_id != "mirage_gun_e" {
        return Err("canonical Mirage weapon-name mapping changed".into());
    }
    let b = d.weapon_kind(&base_id).ok_or("actual base kind")?;
    let second = d
        .weapon_kind(&elite_id)
        .ok_or("actual elite secondary kind")?;
    let k = d.unit_kind("mirage_tank").ok_or("actual Mirage type")?;
    let terms = d.unit_combat(k).ok_or("actual Mirage combat")?;
    let m = d
        .metadata(sim::ProductionItem::Unit(k))
        .ok_or("actual Mirage metadata")?;
    if terms.weapons(false) != [Some(b), None]
        || terms.weapons(true) != [Some(b), Some(second)]
        || terms.land_targeting != 0
        || terms.naval_targeting != 0
        || m.gunner
        || m.deployer
        || m.overpowerable
    {
        return Err("actual Mirage slot/land/naval/override selector premise changed".into());
    }
    let primary = d.weapon(b).ok_or("base definition")?;
    let secondary = d.weapon(second).ok_or("secondary definition")?;
    if primary.warhead != secondary.warhead
        || primary.anti_air != secondary.anti_air
        || primary.anti_ground != secondary.anti_ground
        || primary.repair != secondary.repair
        || primary.area_fire != secondary.area_fire
        || primary.naval_only != secondary.naval_only
    {
        return Err("actual secondary is no longer selection-equivalent to primary".into());
    }
    let wh = d
        .warhead(primary.warhead)
        .ok_or("actual shared Mirage WH")?;
    if wh.id != "mirage_wh" || wh.versus.len() != 11 || d.armour_classes().len() != 11 {
        return Err("shared exact native11 Mirage WH premise changed".into());
    }
    // Current weapon_against chooses first eligible positive-versus slot before range tests.
    // With both slots allowed, equal layers/filter flags and the same WH imply equal target
    // eligibility/versus for every target trait; a zero-versus fallback also retains the first.
    // Secondary longer range/higher damage do not bypass that selector. No E fire is claimed.
    // These actual loaded premises are compared to reference by the existing ordinary/elite
    // slot, metadata and weapon/WH adapters; this guard does not credit those native keys again.
    Ok(())
}
#[derive(Clone, Copy, Debug)]
pub(super) enum ReturnPolicyAcceptance {
    ExplicitAssumption,
    SpecificDifference,
}
#[derive(Clone, Debug)]
pub(super) struct ReturnPolicyDecision {
    pub(super) decision_id: String,
    pub(super) explicit_model_assumption: String,
    pub(super) acceptance: ReturnPolicyAcceptance,
}
// There is deliberately no Default or automatic acceptance. Root/A must record the narrow
// assumption/specific difference before a successful runtime projection can cover this key.
pub(super) fn compare_return_policy(
    a: &mut Audit,
    input: &Inputs,
    e: &Evidence,
    decision: Option<&ReturnPolicyDecision>,
) -> AuditResult<()> {
    let expected = false_only(&input.reference, "CombatDamage", "PlayerReturnFire")?;
    let decision=decision.ok_or("PENDING PlayerReturnFire: record explicit narrow manual-Move hit-response assumption/specific difference; source annotation does not define exact native priority/range/order policy")?;
    if decision.decision_id.trim().is_empty()
        || decision.explicit_model_assumption.trim().is_empty()
    {
        return Err("PlayerReturnFire disposition must name an actual recorded decision and narrow uncertainty".into());
    }
    let v = &e.returned;
    if !v.hostile_fire
        || !v.actual_hit
        || !v.hit_before_explicit_attack
        || v.hp_after >= v.hp_before
        || v.hp_after <= 0
        || !v.explicit_attack_fired
        || !e.acquisition.idle_without_hit
        || !e.acquisition.attack_move_without_hit
        || v.destination != sim::CellPos::new(8, 25)
    {
        return Err(format!(
            "native PlayerReturnFire real-hit/control premises missing: {e:?}"
        ));
    }
    let reaction = !(v.move_preserved_after_hit && v.no_unsolicited_shot);
    match decision.acceptance {
        ReturnPolicyAcceptance::ExplicitAssumption => a.check(
            WitnessId::new(
                "rules.player_return_fire",
                "runtime.manual_move_hit_response",
            ),
            "rules",
            "player_return_fire.manual_move_hit_response",
            u8::from(reaction).to_string(),
            expected.map(|v| u8::from(v).to_string()),
            "no".into(),
        ),
        ReturnPolicyAcceptance::SpecificDifference => {
            if reaction {
                return Err("recorded no-override game-policy difference is not met by actual native behavior".into());
            }
            let actual="manual_move_not_overridden_after_hit;idle_and_attackmove_acquire;explicit_attack_works".to_string();
            // Exact candidate difference must be loaded and used. This source-dependent opaque
            // label explicitly refuses to pretend the undocumented native detailed policy is known.
            a.check(
                WitnessId::new(
                    "rules.player_return_fire",
                    "runtime.manual_move_hit_response",
                ),
                "rules",
                "player_return_fire.manual_move_hit_response",
                actual,
                expected
                    .map(|_| "native_PlayerReturnFire_no_detailed_scope_unestablished".to_string()),
                "no".into(),
            )
        }
    }
}
// Until A merges the decision, project is deliberately fail-closed for PlayerReturnFire.
pub(super) fn project(
    a: &mut Audit,
    input: &Inputs,
    d: &Definitions,
    decision: Option<&ReturnPolicyDecision>,
) -> AuditResult<Evidence> {
    let e = super::fire_keys::collect(d)?;
    compare_base(a, input, &e)?;
    require_masked_elite(input, d, &e.mirage_elite)?;
    compare_return_policy(a, input, &e, decision)?;
    Ok(e)
}

#[test]
fn ra9_current_elite_shadow_guard_rejects_a_changed_false_source_flag() -> AuditResult<()> {
    let data = super::super::data_dir();
    let mut input = Inputs::load(&data, &super::super::reference_dir())?;
    let d = sim::load_ruleset(&data).map_err(|e| e.to_string())?;
    let native = super::fire_keys::observe_mirage_undisguised(&d, true)?;
    require_masked_elite(&input, &d, &native)?;
    let key = SourceKey::new("rules", "MirageGunE", "DisguiseFireOnly");
    input.reference.rows.insert(key, "yes".into());
    let error = require_masked_elite(&input, &d, &native)
        .expect_err("changed source false-only premise must fail");
    if !error.contains("only the exact shipped no case") {
        return Err(format!("wrong source guard failure {error}"));
    }
    Ok(())
}
#[test]
fn ra9_current_elite_shadow_guard_rejects_a_changed_actual_loaded_slot() -> AuditResult<()> {
    let data = super::super::data_dir();
    let input = Inputs::load(&data, &super::super::reference_dir())?;
    let d = sim::load_ruleset(&data).map_err(|e| e.to_string())?;
    let native = super::fire_keys::observe_mirage_undisguised(&d, true)?;
    require_masked_elite(&input, &d, &native)?;
    let copy = super::array_negatives::DataCopy::edit(
        "combat.ron",
        "(unit: \"mirage_tank\",",
        "elite_weapons",
        |v| {
            if v.len() != 2 || v[0] != "\"mirage_gun\"" || v[1] != "\"mirage_gun_e\"" {
                return Err("actual elite two-slot premise missing".into());
            }
            let _ = v.remove(0);
            Ok(())
        },
    )?;
    let (mutant_input, mutant) = copy.loaded()?;
    let error = require_masked_elite(&mutant_input, &mutant, &native)
        .expect_err("an actual changed selector invalidates the conditional non-use disposition");
    if !error.contains("actual Mirage slot/land/naval/override selector premise changed") {
        return Err(format!("wrong actual loaded guard failure {error}"));
    }
    Ok(())
}
