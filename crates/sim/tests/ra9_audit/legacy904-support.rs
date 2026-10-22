//! Historical preparation note: UNCOMPILED/UNRUN small shared table/real-loader guards for two existing-style policies.
//! Uses the existing common SourceProposal/SourceAcceptance/ObservedScope gates; no new receipts.
use super::*;
pub(super) fn records<'a>(text: &'a str, header: &str) -> AuditResult<Vec<Vec<&'a str>>> {
    if text.lines().next() != Some(header) {
        return Err("legacy exact table header changed".into());
    }
    let n = header.split('\t').count();
    text.lines()
        .skip(1)
        .map(|l| {
            let r = l.split('\t').collect::<Vec<_>>();
            if r.len() != n {
                Err("legacy table width".into())
            } else {
                Ok(r)
            }
        })
        .collect()
}
pub(super) fn hash(text: &[u8]) -> String {
    let mut h = sim::Fnv1a64::new();
    h.write(text);
    format!("{:016x}", h.finish())
}
pub(super) fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
pub(super) fn current_code(paths: &str, profile: &str) -> AuditResult<String> {
    let root = root();
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    let want = paths.split(';').collect::<BTreeSet<_>>();
    for r in records(
        include_str!("legacy904-branch-files.tsv"),
        "repository_path\tbytes\tfnv1a64\tsampled_sha256",
    )? {
        if !want.contains(r[0]) {
            continue;
        }
        if !seen.insert(r[0]) {
            return Err("legacy duplicate code path".into());
        }
        let b = std::fs::read(root.join(r[0])).map_err(|e| e.to_string())?;
        if b.len().to_string() != r[1] || hash(&b) != r[2] {
            return Err(format!(
                "legacy current branch bytes changed {}; refresh actual final source then reconsider exact policy, not a frozen failure",
                r[0]
            ));
        }
        out.push(format!("{}:{}:{}", r[0], r[1], r[2]));
    }
    if seen != want {
        return Err("legacy missing/foreign exact code branch scope".into());
    }
    for r in records(
        include_str!("legacy904-branch-anchors.tsv"),
        "profile\trepository_path\tactual_function",
    )?
    .into_iter()
    .filter(|r| r[0] == profile)
    {
        let t = std::fs::read_to_string(root.join(r[1])).map_err(|e| e.to_string())?;
        if !t.contains(&format!("fn {}(", r[2])) && !t.contains(&format!("fn {}<", r[2])) {
            return Err("legacy actual branch anchor missing".into());
        }
    }
    Ok(out.join(";")) // Drift detection only. Root authenticates actual SHA256, including model scope.
}
pub(super) fn documents(ids: &str) -> AuditResult<()> {
    let wanted = ids.split(';').collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    for r in records(
        include_str!("legacy904-document-guards.tsv"),
        "authority_id\tdocument_path\tsection_start\tsection_end\tspan_bytes\tspan_fnv1a64\tsampled_span_sha256",
    )? {
        if !wanted.contains(r[0]) {
            continue;
        }
        if !seen.insert(r[0]) {
            return Err("legacy duplicate authority".into());
        }
        let t = std::fs::read_to_string(root().join(r[1])).map_err(|e| e.to_string())?;
        document_span(&t, r[2], r[3], r[4], r[5])?;
    }
    if seen != wanted {
        return Err("legacy missing exact authority".into());
    }
    Ok(())
}
pub(super) fn document_span<'a>(
    t: &'a str,
    start: &str,
    end: &str,
    len: &str,
    fp: &str,
) -> AuditResult<&'a str> {
    let i = t.find(start).ok_or("legacy missing authority START")?;
    let z = i
        + start.len()
        + t[i + start.len()..]
            .find(end)
            .ok_or("legacy missing authority END; no EOF fallback")?;
    let s = &t[i..z];
    if s.len().to_string() != len || hash(s.as_bytes()) != fp {
        return Err("legacy bounded authority text changed".into());
    }
    Ok(s)
}
pub(super) fn mappings(i: &Inputs) -> AuditResult<()> {
    let rows = records(
        include_str!("legacy904-mapping-expected.tsv"),
        "game_id\tkind\tsection\tside\tnote",
    )?;
    let want = rows
        .into_iter()
        .map(|r| {
            (
                r[0].to_owned(),
                r[1] == "building",
                r[2].to_owned(),
                r[3].to_owned(),
                r[4].to_owned(),
            )
        })
        .collect::<BTreeSet<_>>();
    let got = i
        .mappings
        .iter()
        .map(|r| {
            (
                r.game_id.clone(),
                r.building,
                r.section.clone(),
                r.side.clone(),
                r.note.clone(),
            )
        })
        .collect::<BTreeSet<_>>();
    if got.len() != i.mappings.len() || got != want {
        return Err("legacy all actual mapped aliases/kinds/sides/notes changed".into());
    }
    let cm = records(
        include_str!("legacy904-combat-mapping-expected.tsv"),
        "game_id\tkind\tsection",
    )?;
    for (kind, actual) in [("weapon", &i.weapons), ("warhead", &i.warheads)] {
        let want = cm
            .iter()
            .filter(|r| r[1] == kind)
            .map(|r| (r[0].to_owned(), r[2].to_owned()))
            .collect::<BTreeSet<_>>();
        let got = actual.iter().cloned().collect::<BTreeSet<_>>();
        if got.len() != actual.len() || want != got {
            return Err("legacy actual combat mapping closure changed".into());
        }
    }
    Ok(())
}
pub(super) fn item_set(profile: &str) -> AuditResult<BTreeSet<String>> {
    let rs = records(
        include_str!("legacy904-profile-items.tsv"),
        "profile\texact_item_set",
    )?;
    let row = rs.iter().filter(|r| r[0] == profile).collect::<Vec<_>>();
    if row.len() != 1 {
        return Err("legacy exact one profile item-set required".into());
    }
    let ids = row[0][1].split(',').map(str::to_owned).collect::<Vec<_>>();
    let set: BTreeSet<String> = ids.iter().cloned().collect();
    if ids.len() != set.len() {
        return Err("legacy duplicate policy consumer".into());
    }
    Ok(set)
}
pub(super) fn loaded(i: &Inputs, d: &Definitions, profile: &str, ai: bool) -> AuditResult<String> {
    mappings(i)?;
    let ids = item_set(profile)?;
    let mut actual = String::new();
    let mut seen = BTreeSet::new();
    for r in records(
        include_str!("legacy904-loaded-gates.tsv"),
        "game_id_or_scope\tactual_field\texpected_current_policy",
    )? {
        if r[0] != "global" && !ids.contains(r[0]) {
            continue;
        }
        let v = legacy904_loaded::observe(d, r[0], r[1])?;
        if v != r[2] {
            return Err(format!(
                "legacy actual loaded premise {}.{} changed {v} != {}",
                r[0], r[1], r[2]
            ));
        }
        if !seen.insert((r[0], r[1])) {
            return Err("legacy duplicate typed premise".into());
        }
        actual.push_str(&format!("{}\t{}\t{v}\n", r[0], r[1]));
    }
    if !ids.iter().all(|id| seen.iter().any(|(s, _)| *s == id)) {
        return Err("legacy missing real consumer observation".into());
    }
    if ai {
        let ai = sim::load_ai(i.data_dir.join("ai.ron"), d).map_err(|e| e.to_string())?;
        let got = checked_ai_snapshot(&ai, d)?;
        actual.push_str(&got);
    }
    Ok(hash(actual.as_bytes()))
}
fn original_closures(text: &str) -> AuditResult<BTreeMap<&str, Vec<&str>>> {
    let mut out = BTreeMap::new();
    for row in records(
        text,
        "family_id\tsection_count\tincoming_count\tmetadata_closure_sha256",
    )? {
        if out.insert(row[0], row).is_some() {
            return Err("duplicate original closure family".into());
        }
    }
    Ok(out)
}
pub(super) fn original(i: &Inputs, family: &str) -> AuditResult<String> {
    let want = original_closures(include_str!("legacy904-original-expected.tsv"))?;
    let text = std::fs::read_to_string(
        root().join("crates/sim/tests/reference/ra9_legacy_original.tsv"),
    )
    .map_err(|e| {
        format!("legacy real producer output absent (do not copy expectations as observation): {e}")
    })?;
    let actual = original_closures(&text)?;
    if want != actual {
        return Err("legacy full-original section/incoming closure changed".into());
    }
    let row = actual
        .get(family)
        .ok_or("legacy family has no original observation")?;
    let _ = i;
    Ok(row[3].into()) // Root rehashes actual producer output+R2 source before accepting static route.
}
pub(super) fn physical(
    i: &Inputs,
    a: &Audit,
    profile: &str,
    ai: bool,
) -> AuditResult<BTreeMap<WitnessId, BTreeSet<Atom>>> {
    let ids = item_set(profile)?;
    let mut out = BTreeMap::new();
    for r in records(
        include_str!("legacy904-physical-demands.tsv"),
        "independent_selector\tphysical_field",
    )? {
        let mut consumers = Vec::new();
        match r[0] {
            "profile-items"
            | "profile-units"
            | "ai-profile-items"
            | "death-profile-items"
            | "harvester-profile-items" => {
                if r[0] == "death-profile-items"
                    && !matches!(profile, "default-death" | "delayed-kill")
                {
                    continue;
                }
                if r[0] == "harvester-profile-items" && profile != "harvest-cadence" {
                    continue;
                }
                if r[0] == "ai-profile-items" && !ai {
                    continue;
                }
                for m in &i.mappings {
                    if ids.contains(&m.game_id)
                        && !(r[0] == "profile-units" && m.building)
                        && (r[0] != "harvester-profile-items"
                            || matches!(m.game_id.as_str(), "chrono_miner" | "war_miner"))
                    {
                        consumers.push(format!("{}@rules:{}", m.game_id, m.section));
                    }
                }
            }
            "all-mapped-weapons" => consumers.extend(
                i.weapons
                    .iter()
                    .map(|(id, s)| format!("weapon.{id}@rules:{s}")),
            ),
            "all-mapped-warheads" => consumers.extend(
                i.warheads
                    .iter()
                    .map(|(id, s)| format!("warhead.{id}@rules:{s}")),
            ),
            "ai-difficulty-three" => {
                if ai {
                    consumers.extend(
                        ["easy", "medium", "brutal"]
                            .into_iter()
                            .map(|s| format!("difficulty.{s}")),
                    );
                }
            }
            _ => return Err("legacy foreign independent physical selector".into()),
        };
        for c in consumers {
            let id = a.physical(WitnessId::new(&c, r[1]));
            if !a.required.contains(&id) {
                return Err(format!(
                    "legacy real neighbor not independently declared {id:?}"
                ));
            }
            let deps = a
                .required_dependencies
                .iter()
                .filter(|(w, _)| w == &id)
                .map(|(_, v)| v.clone())
                .collect();
            out.entry(id).or_insert(deps);
        }
    }
    if out.is_empty() {
        return Err("legacy no independently demanded real fields".into());
    }
    Ok(out)
}
pub(super) fn compared(a: &Audit, need: &BTreeMap<WitnessId, BTreeSet<Atom>>) -> AuditResult<()> {
    for (w, deps) in need {
        let declared = a
            .required_dependencies
            .iter()
            .filter(|(id, _)| id == w)
            .map(|(_, v)| v.clone())
            .collect::<BTreeSet<_>>();
        if &declared != deps {
            return Err("legacy independent native demand set changed after declaration".into());
        }
        match a.witnesses.get(w) {
            Some(Witness::Compared { dependencies, .. }) if dependencies == deps => {}
            _ => {
                return Err(format!(
                    "legacy actual Compared/dependency equality absent {w:?}"
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn checked_ai_snapshot(ai: &sim::AiDefinitions, d: &Definitions) -> AuditResult<String> {
    let got = legacy904_loaded::ai_snapshot(ai, d)?;
    let want = records(
        include_str!("legacy904-ai-loaded-expected.tsv"),
        "actual_field\texpected_current_policy",
    )?
    .into_iter()
    .map(|r| format!("{}\t{}\n", r[0], r[1]))
    .collect::<String>();
    if got != want {
        return Err("legacy actual ordered AI plan/policy/difficulty changed; refresh final R173/R174/R176/latest chosen policy and retain real timing comparisons".into());
    }
    Ok(got)
}
// Historical preparation note: UNCOMPILED/UNRUN. Append inside legacy904_support; typed current-policy guard only.
// This does not observe an unparsed source key, create Native credit or simulate a selector.
pub(super) fn last_five_model_guard(
    i: &Inputs,
    d: &Definitions,
    family: &str,
) -> AuditResult<String> {
    if matches!(family, "F146" | "F147") {
        let mut got = Vec::new();
        for (item, m) in &d.arsenal().0 {
            if !(m.explodes || m.gunner || m.death_weapon.is_some()) {
                continue;
            }
            let (id, terms) = match *item {
                sim::ProductionItem::Unit(k) => (
                    d.unit(k).ok_or("actual unit missing")?.id.as_str(),
                    d.unit_combat(k),
                ),
                sim::ProductionItem::Building(k) => (
                    d.building(k).ok_or("actual building missing")?.id.as_str(),
                    d.building_combat(k),
                ),
            };
            let name = |w: Option<sim::WeaponKind>| -> AuditResult<String> {
                w.map(|k| {
                    d.weapon(k)
                        .map(|v| v.id.clone())
                        .ok_or_else(|| "actual death/primary weapon missing".into())
                })
                .transpose()
                .map(|v| v.unwrap_or_else(|| "none".into()))
            };
            got.push(format!(
                "{id}\t{}\t{}\t{}\t{}\n",
                m.explodes,
                m.gunner,
                name(m.death_weapon)?,
                name(terms.and_then(|t| t.weapon))?
            ));
        }
        got.sort();
        let text = got.concat();
        if text != include_str!("legacy904-death-selectors.tsv") {
            return Err("exact current death-selector closure changed; reconsider F146/F147, do not preserve missing-default claim".into());
        }
        if d.weapon_kind("default_death_weapon").is_some() {
            return Err("DefaultDeathWeapon now loaded: re-evaluate source missing-Damage/current no-runtime-default distinction".into());
        }
        return Ok(hash(text.as_bytes()));
    }
    if family == "F251" {
        // Full original closure comes from actual producer scan, NOT these string identities.
        // Any mapped eligible source invalidates the present native-scope qualification.
        if i.mappings
            .iter()
            .any(|m| ["CAMISC01", "CAMISC02", "AMMOCRAT"].contains(&m.section.as_str()))
        {
            return Err("a source EligibleForDelayKill object is now mapped; add its real native death-timing route".into());
        }
        return Ok("CURRENT_NO_TIMER;ORIGINAL_ELIGIBLE_TYPES_UNMAPPED;NO_OBSERVED_ORIGINAL_TIMING_DIFFERENCE".into());
    }
    if family == "F208" {
        let mut out = String::new();
        for (id, capacity) in [("chrono_miner", 20), ("war_miner", 40)] {
            let h = d
                .harvester(d.unit_kind(id).ok_or("actual stock harvester absent")?)
                .ok_or("actual harvester terms absent")?;
            if (h.capacity, h.harvest_rate, h.harvest_interval) != (capacity, 1, 19) {
                return Err("R58 current loaded stock harvester1/19 premise changed; no Harvest.Rate source conversion credit".into());
            }
            out.push_str(&format!(
                "{id}\t{}\t{}\t{}\n",
                h.capacity, h.harvest_rate, h.harvest_interval
            ));
        }
        return Ok(hash(out.as_bytes()));
    }
    Ok(String::new())
}

#[test]
fn ra9_legacy_original_family_records_are_keyed_but_nonfirst_changes_fail() -> AuditResult<()> {
    let observed =
        std::fs::read_to_string(root().join("crates/sim/tests/reference/ra9_legacy_original.tsv"))
            .map_err(|e| e.to_string())?;
    let expected = original_closures(include_str!("legacy904-original-expected.tsv"))?;
    assert_eq!(original_closures(&observed)?, expected);
    let mut lines = observed.lines().collect::<Vec<_>>();
    let header = lines.remove(0);
    lines.reverse();
    let reordered = format!("{header}\n{}\n", lines.join("\n"));
    assert_eq!(original_closures(&reordered)?, expected);
    let nonfirst = lines[1].split('\t').collect::<Vec<_>>();
    let changed = reordered.replace(
        lines[1],
        &format!(
            "{}\t{}\t{}\t{}",
            nonfirst[0],
            nonfirst[1],
            nonfirst[2],
            "0".repeat(64)
        ),
    );
    assert_ne!(original_closures(&changed)?, expected);
    let duplicate = format!("{reordered}{}\n", lines[1]);
    assert!(original_closures(&duplicate).is_err());
    lines.remove(1);
    let missing = format!("{header}\n{}\n", lines.join("\n"));
    assert_ne!(original_closures(&missing)?, expected);
    Ok(())
}
