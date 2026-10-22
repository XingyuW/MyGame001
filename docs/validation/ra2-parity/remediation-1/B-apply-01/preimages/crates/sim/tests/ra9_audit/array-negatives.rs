//! UNRUN actual loaded-array mutations. Positive calls invoke the unchanged native adapter
//! first; mutants must load successfully and fail the intended array component/length check.
//! Keyed Verses and unique prerequisite reorders are separate expected-pass controls.
use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct DataCopy {
    path: PathBuf,
}
impl Drop for DataCopy {
    fn drop(&mut self) {
        for name in [
            "units.ron",
            "economy.ron",
            "combat.ron",
            "rules.ron",
            "arsenal.ron",
            "skirmish.ron",
            "ai.ron",
        ] {
            let _ = std::fs::remove_file(self.path.join(name));
        }
        let _ = std::fs::remove_dir(&self.path);
    }
}
fn split_components(raw: &str) -> AuditResult<Vec<String>> {
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    let (mut begin, mut depth, mut quoted, mut escaped) = (0usize, 0i32, false, false);
    let mut out = Vec::new();
    for (index, c) in raw.char_indices() {
        if quoted {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                quoted = false;
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            '(' | '[' => depth += 1,
            ')' | ']' => {
                depth -= 1;
                if depth < 0 {
                    return Err("array mutation fixture nesting".into());
                }
            }
            ',' if depth == 0 => {
                out.push(raw[begin..index].trim().into());
                begin = index + 1;
            }
            _ => {}
        }
    }
    if depth != 0 || quoted {
        return Err("array mutation fixture syntax".into());
    }
    out.push(raw[begin..].trim().into());
    Ok(out)
}
impl DataCopy {
    pub(super) fn edit(
        file: &str,
        selector: &str,
        array: &str,
        edit: impl FnOnce(&mut Vec<String>) -> AuditResult<()>,
    ) -> AuditResult<Self> {
        let home = std::env::var_os("HOME").ok_or("HOME needed for user-mandated review cache")?;
        let base = PathBuf::from(home).join("Library/Caches/Codex/reviews/MyGame001");
        std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
        let mut path = None;
        for _ in 0..32 {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let candidate = base.join(format!("ra9-array-mutation-{}-{n}", std::process::id()));
            match std::fs::create_dir(&candidate) {
                Ok(()) => {
                    path = Some(candidate);
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        let owned = Self {
            path: path.ok_or("exclusive array fixture path exhausted")?,
        };
        let mut edit = Some(edit);
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
                let rows = text
                    .lines()
                    .filter(|line| line.contains(selector))
                    .collect::<Vec<_>>();
                if rows.len() != 1 {
                    return Err("array mutation row selector not unique".into());
                }
                let row = rows[0];
                let marker = format!("{array}:");
                let start = row.find(&marker).ok_or("array field not found")? + marker.len();
                let bracket = row[start..].find('[').ok_or("array opening")? + start;
                let mut depth = 0usize;
                let mut end = None;
                for (index, c) in row[bracket..].char_indices() {
                    match c {
                        '[' => depth += 1,
                        ']' => {
                            depth -= 1;
                            if depth == 0 {
                                end = Some(bracket + index);
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                let end = end.ok_or("array closing")?;
                let mut parts = split_components(&row[bracket + 1..end])?;
                edit.take().ok_or("array edit used twice")?(&mut parts)?;
                let new = format!("{}{}{}", &row[..bracket + 1], parts.join(", "), &row[end..]);
                text = text.replacen(row, &new, 1);
            }
            std::fs::write(owned.path.join(name), text).map_err(|e| e.to_string())?;
        }
        Ok(owned)
    }
    pub(super) fn scalar(file: &str, selector: &str, from: &str, to: &str) -> AuditResult<Self> {
        let owned = Self::edit(file, selector, "ifv_weapons", |_| Ok(()))?;
        // Every arsenal entry has an ifv_weapons array. The no-op array operation above only
        // allocates the same owned seven-file cache fixture; scalar mutation remains exact.
        let path = owned.path.join(file);
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let rows = text
            .lines()
            .filter(|line| line.contains(selector))
            .collect::<Vec<_>>();
        if rows.len() != 1 || rows[0].matches(from).count() != 1 {
            return Err("scalar mutation fragment not unique".into());
        }
        let changed = rows[0].replacen(from, to, 1);
        let output = text.replacen(rows[0], &changed, 1);
        std::fs::write(path, output).map_err(|e| e.to_string())?;
        Ok(owned)
    }
    pub(super) fn replace_row(
        file: &str,
        selector: &str,
        from: &str,
        to: &str,
    ) -> AuditResult<Self> {
        let owned = Self::edit(
            "arsenal.ron",
            "(id: \"construction_yard\",",
            "ifv_weapons",
            |_| Ok(()),
        )?;
        let path = owned.path.join(file);
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let rows = text
            .lines()
            .filter(|line| line.contains(selector))
            .collect::<Vec<_>>();
        if rows.len() != 1 || rows[0].matches(from).count() != 1 {
            return Err("nested scalar mutation fragment not unique".into());
        }
        let changed = rows[0].replacen(from, to, 1);
        let output = text.replacen(rows[0], &changed, 1);
        std::fs::write(path, output).map_err(|e| e.to_string())?;
        Ok(owned)
    }
    // Only the shipped flat quoted-ID crate list uses this multiline fixture helper.
    pub(super) fn edit_crate_ids(
        edit: impl FnOnce(&mut Vec<String>) -> AuditResult<()>,
    ) -> AuditResult<Self> {
        let owned = Self::edit(
            "arsenal.ron",
            "(id: \"construction_yard\",",
            "ifv_weapons",
            |_| Ok(()),
        )?;
        let path = owned.path.join("rules.ron");
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let marker = "\n    crate_units:";
        if text.matches(marker).count() != 1 {
            return Err("crate array marker not unique".into());
        }
        let start = text.find(marker).ok_or("crate array field")? + marker.len();
        let open = text[start..].find('[').ok_or("crate array opening")? + start;
        // Strip comments before searching: source annotations have [section] delimiters.
        let tail = &text[open + 1..];
        let mut offset = 0usize;
        let mut close = None;
        for line in tail.split_inclusive('\n') {
            let code = line.split("//").next().ok_or("crate fixture line")?;
            if let Some(n) = code.find(']') {
                close = Some(open + 1 + offset + n);
                break;
            }
            offset = offset
                .checked_add(line.len())
                .ok_or("crate fixture offset overflow")?;
        }
        let close = close.ok_or("crate array closing")?;
        let body = text[open + 1..close]
            .lines()
            .map(|line| line.split("//").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n");
        let body = body.trim().trim_end_matches(',');
        let mut values = split_components(body)?;
        if values
            .iter()
            .any(|v| !v.starts_with('"') || !v.ends_with('"'))
        {
            return Err("crate fixture only supports flat quoted IDs".into());
        }
        edit(&mut values)?;
        let output = format!(
            "{}{}{}",
            &text[..open + 1],
            values.join(", "),
            &text[close..]
        );
        std::fs::write(path, output).map_err(|e| e.to_string())?;
        Ok(owned)
    }
    pub(super) fn data_dir(&self) -> &Path {
        &self.path
    }
    pub(super) fn loaded(&self) -> AuditResult<(Inputs, Definitions)> {
        let input = Inputs::load(&self.path, &super::super::reference_dir())?;
        let definitions = sim::load_ruleset(&self.path)
            .map_err(|e| format!("MUTANT LOADER FAILURE, not typed-adapter evidence: {e}"))?;
        Ok((input, definitions))
    }
}
fn positive() -> AuditResult<()> {
    let dir = super::super::data_dir();
    let input = Inputs::load(&dir, &super::super::reference_dir())?;
    let d = sim::load_ruleset(&dir).map_err(|e| format!("positive native setup: {e}"))?;
    array_plans::run(&input, &d)
}
fn intended(copy: DataCopy, field: &str, kind: &str) -> AuditResult<()> {
    let (input, d) = copy.loaded()?;
    match array_plans::run(&input, &d) {
        Ok(()) => Err("actual changed component escaped typed native adapter".into()),
        Err(e) => {
            if !e.contains(field) || !e.contains(kind) {
                return Err(format!("wrong failure; expected {field}/{kind}: {e}"));
            }
            Ok(())
        }
    }
}
#[test]
fn actual_nonfirst_ifv_mode_reorder_reaches_component_comparison() -> AuditResult<()> {
    positive()?;
    let copy = DataCopy::edit("arsenal.ron", "(id: \"ifv\",", "ifv_weapons", |v| {
        if v.len() != 13 || v[2] == v[7] {
            return Err("native distinct nonfirst IFV modes absent".into());
        }
        v.swap(2, 7);
        Ok(())
    })?;
    intended(
        copy,
        "arsenal.ron.entries.ifv_weapons",
        "actual physical array component differs",
    )
}
#[test]
fn actual_nonfirst_mirage_elite_slot_removal_is_a_loaded_length_failure() -> AuditResult<()> {
    positive()?;
    let copy = DataCopy::edit(
        "combat.ron",
        "(unit: \"mirage_tank\",",
        "elite_weapons",
        |v| {
            if v.len() != 2 {
                return Err("native Mirage expected two actual elite slots".into());
            }
            let _removed = v.remove(1);
            Ok(())
        },
    )?;
    intended(
        copy,
        "combat.ron.units.elite_weapons",
        "actual/expected/required array length mismatch",
    )
}
#[test]
fn actual_nonfirst_difficulty_target_reorder_reaches_component_comparison() -> AuditResult<()> {
    positive()?;
    let copy = DataCopy::edit("rules.ron", "(id: \"medium\",", "target_weights", |v| {
        if v.len() != 12 || v[7] == v[8] {
            return Err("native distinct nonfirst target classes absent".into());
        }
        v.swap(7, 8);
        Ok(())
    })?;
    intended(
        copy,
        "rules.ron.difficulty.by_source.AIIonCannonAPCValue",
        "actual physical array component differs",
    )
}
#[test]
fn actual_keyed_verses_reorder_preserves_armour_association() -> AuditResult<()> {
    positive()?;
    let copy = DataCopy::edit("combat.ron", "(id: \"ap\",", "versus", |v| {
        if v.len() != 11 {
            return Err("native11 Verses".into());
        }
        v.swap(3, 8);
        Ok(())
    })?;
    let (input, d) = copy.loaded()?;
    array_plans::run(&input, &d)
}
#[test]
fn actual_nonfirst_verses_percent_change_fails_exact_native_association() -> AuditResult<()> {
    positive()?;
    let copy = DataCopy::edit("combat.ron", "(id: \"ap\",", "versus", |v| {
        let record = v.get_mut(9).ok_or("nonfirst armour record")?;
        if !record.contains("armour: \"special_1\"") || !record.contains("percent: 60") {
            return Err("native special_1 fixture association differs".into());
        }
        *record = record.replacen("percent: 60", "percent: 61", 1);
        Ok(())
    })?;
    intended(
        copy,
        "combat.ron.warheads.versus",
        "actual physical array component differs",
    )
}
#[test]
fn actual_prerequisite_set_reorder_is_equivalent() -> AuditResult<()> {
    positive()?;
    let copy = DataCopy::edit(
        "economy.ron",
        "(unit: \"apocalypse\",",
        "prerequisites",
        |v| {
            if v.len() != 2 || v[0] == v[1] {
                return Err("native two-member prerequisite fixture".into());
            }
            v.swap(0, 1);
            Ok(())
        },
    )?;
    let (input, d) = copy.loaded()?;
    array_plans::run(&input, &d)
}
#[test]
fn actual_nonfirst_prerequisite_member_removal_fails_set_completeness() -> AuditResult<()> {
    positive()?;
    let copy = DataCopy::edit(
        "economy.ron",
        "(unit: \"apocalypse\",",
        "prerequisites",
        |v| {
            if v.len() != 2 {
                return Err("native prerequisite fixture".into());
            }
            v.truncate(1);
            Ok(())
        },
    )?;
    intended(
        copy,
        "economy.ron.units.prerequisites",
        "actual/expected/required array length mismatch",
    )
}

fn positive_specialists() -> AuditResult<()> {
    let dir = super::super::data_dir();
    let input = Inputs::load(&dir, &super::super::reference_dir())?;
    let d = sim::load_ruleset(&dir).map_err(|e| e.to_string())?;
    let mut a = load_audit(&input, &d)?;
    specialist::project(&mut a, &input, &d)
}
#[test]
fn native_gap_radius_metadata_mutation_reaches_the_ported_radius_comparator() -> AuditResult<()> {
    positive_specialists()?;
    let copy = DataCopy::scalar(
        "arsenal.ron",
        "(id: \"gap_generator\",",
        "radius: 10,",
        "radius: 11,",
    )?;
    let (input, d) = copy.loaded()?;
    let mut a = load_audit(&input, &d)?;
    let error =
        specialist::project(&mut a, &input, &d).expect_err("native gap radius mutation must fail");
    if !error.contains("unexplained") || !error.contains("arsenal.ron.entries.radius") {
        return Err(format!("wrong native-radius failure: {error}"));
    }
    Ok(())
}
#[test]
fn native_undeploy_timer_mutation_reaches_the_source_frames_comparator() -> AuditResult<()> {
    positive_specialists()?;
    let copy = DataCopy::scalar(
        "arsenal.ron",
        "(id: \"yuri\",",
        "undeploy_delay: 200,",
        "undeploy_delay: 201,",
    )?;
    let (input, d) = copy.loaded()?;
    let mut a = load_audit(&input, &d)?;
    let error =
        specialist::project(&mut a, &input, &d).expect_err("native timer mutation must fail");
    if !error.contains("unexplained") || !error.contains("arsenal.ron.entries.undeploy_delay") {
        return Err(format!("wrong native timer failure: {error}"));
    }
    Ok(())
}
#[test]
fn legacy_metadata_strength_mutation_does_not_change_loaded_native_nuke_weapon_payload()
-> AuditResult<()> {
    positive_specialists()?;
    let copy = DataCopy::scalar(
        "arsenal.ron",
        "(id: \"nuclear_silo\",",
        "strength: 2500,",
        "strength: 2501,",
    )?;
    let (input, d) = copy.loaded()?;
    let expected = super::actual::number(super::actual::rule(
        &input.reference,
        "NukePayload",
        "Damage",
    )?)?
    .value;
    let native_id = index(&input.weapons)
        .get("NukePayload")
        .cloned()
        .ok_or("native NukePayload mapping")?;
    if native_id != "nuke_payload" {
        return Err("native runtime missile weapon-name selector changed".into());
    }
    let weapon = d
        .weapon_kind(&native_id)
        .and_then(|k| d.weapon(k))
        .ok_or("actual runtime NukePayload weapon")?;
    if i128::from(weapon.damage) != expected {
        return Err("legacy metadata changed loaded native weapon payload definition".into());
    }
    let spread = super::combat::converted(
        super::actual::rule(&input.reference, "NUKE", "CellSpread")?,
        super::combat::Conversion::Cells,
    )?
    .value;
    if i128::from(
        d.warhead(weapon.warhead)
            .ok_or("actual runtime Nuke warhead")?
            .cell_spread,
    ) != spread
    {
        return Err("legacy metadata changed loaded native warhead spread".into());
    }
    if i128::from(
        d.effect(sim::Effect::Nuke)
            .ok_or("separately loaded Nuke effect")?
            .damage,
    ) != expected
    {
        return Err("static Nuke effect definition changed".into());
    }
    let mut a = load_audit(&input, &d)?;
    let error = specialist::project(&mut a, &input, &d)
        .expect_err("game-owned metadata policy change remains explicit");
    if !error.contains("own legacy metadata strength changed") {
        return Err(format!("wrong metadata-boundary failure: {error}"));
    }
    Ok(())
}
