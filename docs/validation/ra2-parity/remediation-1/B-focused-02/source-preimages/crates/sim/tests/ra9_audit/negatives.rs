//! Historical preparation note: UNRUN causal audit negatives against actual loaded Definitions/adapters.
//! Every mutation first requires the same real adapter to succeed on unchanged shipped input.
//! Loader/setup failures fail the test separately; they cannot count as the intended negative.
use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct OwnedData {
    path: PathBuf,
}
impl OwnedData {
    fn edit(file: &str, selector: &str, from: &str, to: &str) -> AuditResult<Self> {
        // Future test execution uses the user-mandated reviews cache, never /tmp, the repo,
        // CARGO_TARGET_DIR overrides, a clone/worktree, or a package cache.
        let home = std::env::var_os("HOME").ok_or("HOME unavailable for mandated review cache")?;
        let base = PathBuf::from(home).join("Library/Caches/Codex/reviews/MyGame001");
        std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
        let mut chosen = None;
        for _ in 0..32 {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let p = base.join(format!("ra9-audit-mutation-{}-{n}", std::process::id()));
            match std::fs::create_dir(&p) {
                Ok(()) => {
                    chosen = Some(p);
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        let owned = Self {
            path: chosen.ok_or("exclusive mutation directory exhausted")?,
        };
        for name in [
            "units.ron",
            "economy.ron",
            "combat.ron",
            "rules.ron",
            "arsenal.ron",
            "skirmish.ron",
            "ai.ron",
        ] {
            let mut text = std::fs::read_to_string(super::super::data_dir().join(name))
                .map_err(|e| e.to_string())?;
            if name == file {
                let found = text
                    .lines()
                    .filter(|line| line.contains(selector))
                    .collect::<Vec<_>>();
                if found.len() != 1 {
                    return Err(format!("mutation selector {selector:?} is not unique"));
                }
                let old = found[0];
                if old.matches(from).count() != 1 {
                    return Err(format!(
                        "mutation fragment {from:?} not unique in selected physical row"
                    ));
                }
                let new = old.replacen(from, to, 1);
                text = text.replacen(old, &new, 1);
            }
            std::fs::write(owned.path.join(name), text).map_err(|e| e.to_string())?;
        }
        Ok(owned)
    }
    fn load(&self) -> AuditResult<Definitions> {
        sim::load_ruleset(&self.path)
            .map_err(|e| format!("MUTANT LOADER FAILED (not adapter negative): {e}"))
    }
}
impl Drop for OwnedData {
    fn drop(&mut self) {
        // This path was exclusively created by this test; it never holds a repository/worktree.
        // Remove only the seven known regular files then the empty directory; never recurse.
        for file in [
            "units.ron",
            "economy.ron",
            "combat.ron",
            "rules.ron",
            "arsenal.ron",
            "skirmish.ron",
            "ai.ron",
        ] {
            let _ = std::fs::remove_file(self.path.join(file));
        }
        let _ = std::fs::remove_dir(&self.path);
    }
}
fn unchanged() -> AuditResult<(Inputs, Definitions)> {
    let data = super::super::data_dir();
    Ok((
        Inputs::load(&data, &super::super::reference_dir())?,
        sim::load_ruleset(&data).map_err(|e| format!("positive setup {e}"))?,
    ))
}
fn core(input: &Inputs, d: &Definitions, section: &str) -> AuditResult<Audit> {
    let row = input
        .mappings
        .iter()
        .find(|r| r.section == section)
        .ok_or("missing named consumer")?;
    let mut a = load_audit(input, d)?;
    actual::core_item(
        &mut a,
        &input.reference,
        d,
        &row.game_id,
        &row.section,
        row.building,
    )?;
    Ok(a)
}
fn intended(result: AuditResult<Audit>, physical: &str) -> AuditResult<()> {
    match result {
        Ok(_) => Err("mutated actual field escaped the actual comparison".into()),
        Err(e) => {
            if !e.contains("unexplained") || !e.contains(physical) {
                return Err(format!(
                    "WRONG FAILURE, expected real {physical} comparator: {e}"
                ));
            }
            Ok(())
        }
    }
}
#[test]
fn loaded_shipped_cost_mutation_reaches_actual_core_comparator() -> AuditResult<()> {
    let (input, d) = unchanged()?;
    let _positive = core(&input, &d, "MTNK")?;
    let changed = OwnedData::edit(
        "economy.ron",
        "(unit: \"grizzly\",",
        "cost: 700",
        "cost: 701",
    )?;
    intended(
        core(&input, &changed.load()?, "MTNK"),
        "economy.ron.units.cost",
    )
}
#[test]
fn loaded_nonfirst_hitpoints_mutation_reaches_actual_core_comparator() -> AuditResult<()> {
    let (input, d) = unchanged()?;
    let _positive = core(&input, &d, "MTNK")?;
    let changed = OwnedData::edit(
        "combat.ron",
        "(unit: \"grizzly\",",
        "hit_points: 300",
        "hit_points: 301",
    )?;
    intended(
        core(&input, &changed.load()?, "MTNK"),
        "combat.ron.units.hit_points",
    )
}
#[test]
fn loaded_weapon_range_mutation_reaches_actual_weapon_comparator() -> AuditResult<()> {
    let (input, d) = unchanged()?;
    let mut positive = load_audit(&input, &d)?;
    combat::weapon(
        &mut positive,
        &input.reference,
        &d,
        "gun_105mm",
        "105mm",
        &index(&input.warheads),
    )?;
    let changed = OwnedData::edit(
        "combat.ron",
        "(id: \"gun_105mm\",",
        "range: 1280,",
        "range: 1281,",
    )?;
    let mutant = changed.load()?;
    let mut a = load_audit(&input, &mutant)?;
    intended(
        combat::weapon(
            &mut a,
            &input.reference,
            &mutant,
            "gun_105mm",
            "105mm",
            &index(&input.warheads),
        )
        .map(|_| a),
        "combat.ron.weapons.range",
    )
}
#[test]
fn loaded_nonfirst_verses_component_mutation_reaches_actual_warhead_comparator() -> AuditResult<()>
{
    let (input, d) = unchanged()?;
    let mut positive = load_audit(&input, &d)?;
    combat::warhead(&mut positive, &input.reference, &d, "ap", "AP")?;
    let changed = OwnedData::edit(
        "combat.ron",
        "(id: \"ap\",",
        "(armour: \"special_1\", percent: 60)",
        "(armour: \"special_1\", percent: 61)",
    )?;
    let mutant = changed.load()?;
    let mut a = load_audit(&input, &mutant)?;
    intended(
        combat::warhead(&mut a, &input.reference, &mutant, "ap", "AP").map(|_| a),
        "combat.ron.warheads.versus",
    )
}
#[test]
fn exact_native_twin_requires_its_own_real_loaded_comparison() -> AuditResult<()> {
    let (input, d) = unchanged()?;
    let mut both = load_audit(&input, &d)?;
    for s in ["ENGINEER", "SENGINEER"] {
        actual::core_item(&mut both, &input.reference, &d, "engineer", s, false)?;
    }
    let mut omitted = load_audit(&input, &d)?;
    actual::core_item(
        &mut omitted,
        &input.reference,
        &d,
        "engineer",
        "ENGINEER",
        false,
    )?;
    let id = WitnessId::new("engineer@rules:SENGINEER", "economy.ron.units.cost");
    if !both.witnesses.contains_key(&id)
        || omitted.witnesses.contains_key(&id)
        || !omitted.required.contains(&id)
    {
        return Err("native twin omission not independently required".into());
    }
    // Directly verify this named missing actual witness; whole-table finish remains stricter.
    if omitted.finish(&input.reference).is_ok() {
        return Err("whole audit accepted omitted actual twin".into());
    }
    Ok(())
}
#[test]
fn foreign_dependency_on_a_real_loaded_comparison_is_rejected() -> AuditResult<()> {
    let (input, d) = unchanged()?;
    let mut a = core(&input, &d, "MTNK")?;
    let id = WitnessId::new("grizzly@rules:MTNK", "economy.ron.units.cost");
    let foreign = Atom {
        key: SourceKey::new("rules", "HTNK", "Cost"),
        portion: Portion::Whole,
    };
    match a.witnesses.get_mut(&id) {
        Some(Witness::Compared { dependencies, .. }) => {
            dependencies.insert(foreign.clone());
        }
        _ => return Err("positive core never made real cost comparison".into()),
    }
    if a.required_dependencies.contains(&(id.clone(), foreign)) {
        return Err("independent demand erroneously permits foreign cost".into());
    }
    // finish first checks all physical witnesses; isolate exact pair guard independently too.
    let actual = a
        .witnesses
        .iter()
        .flat_map(|(id, w)| match w {
            Witness::Compared { dependencies, .. } => dependencies
                .iter()
                .map(|d| (id.clone(), d.clone()))
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect::<BTreeSet<_>>();
    if actual.is_subset(&a.required_dependencies) {
        return Err("foreign cost was credited by whole-table pair guard".into());
    }
    Ok(())
}
#[test]
fn a_stale_equal_value_difference_is_rejected_by_the_real_loaded_adapter() -> AuditResult<()> {
    let (input, d) = unchanged()?;
    let _positive = core(&input, &d, "MTNK")?;
    let mut a = load_audit(&input, &d)?;
    a.add_difference(Difference {
        id: DifferenceId {
            game_id: "grizzly".into(),
            field: "cost".into(),
            game_value: "700".into(),
            reference_value: "700".into(),
        },
        reason: "deliberately stale negative fixture".into(),
    })?;
    let error = actual::core_item(&mut a, &input.reference, &d, "grizzly", "MTNK", false)
        .expect_err("stale equal difference must fail");
    if !error.contains("stale equal-value difference") {
        return Err(format!("wrong stale-difference failure: {error}"));
    }
    Ok(())
}
#[test]
fn changing_a_real_source_unused_value_cannot_reuse_an_exact_nonuse_row() -> AuditResult<()> {
    let (input, _) = unchanged()?;
    let key = SourceKey::new("art", "GAPOWR", "Height");
    let old = input
        .reference
        .rows
        .get(&key)
        .ok_or("real art Height absent")?
        .clone();
    let mut r = Reference::from_rows([(key.clone(), old.clone())])?;
    let mut a = Audit::default();
    a.add_unused(Unused {
        atom: Atom {
            key: key.clone(),
            portion: Portion::Whole,
        },
        exact_value: old,
        reason: "test-only native art elevation has no rendering consumer in sim".into(),
        code_evidence: "original sprites drawn in game; this checks exact-row machinery only"
            .into(),
    })?;
    a.finish(&r)?;
    r.rows.insert(key, "999".into());
    let e = a.finish(&r).expect_err("changed unused source must fail");
    if !e.contains("unused source changed") {
        return Err(e);
    }
    Ok(())
}
#[test]
fn a_new_raw_field_on_the_second_actual_entry_cannot_escape_the_schema_guard() -> AuditResult<()> {
    let (input, _) = unchanged()?;
    schema::check_all(&input.documents)?;
    let mut docs = input.documents;
    let Raw::Tuple(top) = docs.get_mut("units.ron").ok_or("units doc")? else {
        return Err("units tuple".into());
    };
    let Raw::List(rows) = &mut top
        .iter_mut()
        .find(|(k, _)| k == "units")
        .ok_or("units list")?
        .1
    else {
        return Err("unit list shape".into());
    };
    let Raw::Tuple(second) = rows.get_mut(1).ok_or("second unit")? else {
        return Err("unit tuple".into());
    };
    second.push(("new_ported_field".into(), Raw::Atom("1".into())));
    // check_all covers top/nested rules; item entry schemas are checked by declare_physical.
    let mut a = Audit::default();
    let spec = schema::ITEM_SCHEMAS
        .iter()
        .find(|(f, l, _, _)| *f == "units.ron" && *l == "units")
        .ok_or("schema")?;
    let fields = spec
        .3
        .iter()
        .map(|n| {
            (
                n.to_string(),
                if *n == "id" {
                    raw::FieldPolicy::Identity
                } else {
                    raw::FieldPolicy::Compared
                },
            )
        })
        .collect();
    let error = raw::enumerate_actual_entries(
        &mut a,
        "units.ron",
        &docs["units.ron"],
        &raw::EntrySpec {
            list: "units".into(),
            identity: "id".into(),
            fields,
        },
        &actual_item_consumers(
            &Inputs {
                documents: BTreeMap::new(),
                ..input
            },
            "units.ron",
            "units",
        ),
    )
    .expect_err("new nonfirst field must fail");
    if !error.contains("unaudited/stale raw fields") {
        return Err(format!("wrong raw guard failure: {error}"));
    }
    Ok(())
}
