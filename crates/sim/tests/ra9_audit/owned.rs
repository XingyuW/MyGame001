//! Exact loaded projections for fields with game-owned semantics; no native key credit.
//! Current schema declarations are fixed before this walk, so a new field is never own by default.
//! Original art/text, authored map starts and tactical policies remain user-verification/intent
//! boundaries; matching raw to loaded values establishes loader preservation, not native parity.
use super::*;
use sim::ProductionItem;
fn leaves(node: &Raw, path: &str, out: &mut BTreeMap<String, String>) -> AuditResult<()> {
    match node {
        Raw::Atom(v) => {
            if out.insert(path.into(), v.clone()).is_some() {
                return Err("duplicate physical leaf".into());
            }
        }
        Raw::Tuple(fields) => {
            for (k, v) in fields {
                leaves(v, &format!("{path}.{k}"), out)?;
            }
        }
        Raw::List(values) => {
            out.insert(format!("{path}.len"), values.len().to_string());
            for (index, v) in values.iter().enumerate() {
                leaves(v, &format!("{path}[{index}]"), out)?;
            }
        }
    }
    Ok(())
}
fn ai_reason(path: &str) -> AuditResult<&'static str> {
    if path == "ai.ron.schema_version" {
        return Ok("Game file-format version; no RA2 reference counterpart");
    }
    if path == "ai.ron.rules_difficulty" {
        return Ok(
            "RA8 explicit switch applies separately audited native Rules house multipliers; it is not a native script parameter",
        );
    }
    let policy = [
        (
            "garrison_quota",
            "RA8 maximum dedicated garrison infantry count",
        ),
        (
            "garrison_radius",
            "RA8 public-nearby garrison search radius",
        ),
        (
            "depot_radius",
            "RA8 own-vehicle depot-service search radius",
        ),
        (
            "naval_rescan",
            "RA8 bounded own-coast/public-terrain rescan cadence",
        ),
        (
            "naval_detector_quota",
            "RA8 naval detector investment quota",
        ),
        (
            "artillery_min_range",
            "RA8 game classification threshold for artillery missions; not a source weapon range",
        ),
        ("artillery_standoff", "RA8 game tactical standoff offset"),
        (
            "paradrop_offset",
            "RA8 legal target-area offset for native paradrop commands",
        ),
        (
            "chrono_min_offset",
            "RA8 minimum tactical Chronosphere destination offset",
        ),
        (
            "chrono_max_offset",
            "RA8 maximum tactical Chronosphere destination offset",
        ),
    ];
    for (field, reason) in policy {
        if path == format!("ai.ron.policy.{field}") {
            return Ok(reason);
        }
    }
    if path == "ai.ron.plans.len" {
        return Ok(
            "Two side fallback plans plus nine exact-country tactical plans; canonical game catalogue",
        );
    }
    if let Some(rest) = path.strip_prefix("ai.ron.plans[") {
        let (_, field) = rest.split_once(']').ok_or("plan index syntax")?;
        let field = field
            .trim_start_matches('.')
            .split(['[', '.'])
            .next()
            .unwrap_or("");
        return match field {
            "side" | "country" => Ok(
                "Exact game plan identity selects a separately audited country/side; ordered loader projection, no invented native script",
            ),
            "build_order" => Ok(
                "Game-owned ordered rebuilding/tech investment plan; repeated entries preserve desired building counts",
            ),
            "power" => Ok("Game-owned selection of source-defined side power producer"),
            "harvester" => Ok("Game-owned selection of source-defined side harvester"),
            "army" => Ok(
                "Game-owned ordered army composition/rotation; source-defined native unit prices/weapons are audited elsewhere",
            ),
            "defences" => Ok("Game-owned ordered defensive investment policy"),
            "support" => Ok("Game-owned ordered Engineer/detector support policy"),
            "tier_reserves" => Ok(
                "Game-owned aircraft/advanced-land/naval reserve quotas, ordered in those three slots",
            ),
            "advanced_tech_level" => Ok(
                "Game-owned composition stage threshold; native item TechLevel is independently audited",
            ),
            _ => Err(format!("no explicit AI plan field policy {path}")),
        };
    }
    if path == "ai.ron.difficulties.len" {
        return Ok("Three game controller profiles mapped by explicit easy/medium/brutal names");
    }
    if let Some(rest) = path.strip_prefix("ai.ron.difficulties[") {
        let (_, f) = rest.split_once(']').ok_or("difficulty index syntax")?;
        return match f.trim_start_matches('.') {
            "id" => Ok(
                "Game controller profile name; native human-facing source section mapping is in Rules audit",
            ),
            "think_interval" => Ok(
                "Game-owned controller cadence; commands still pass through the native CommandQueue",
            ),
            "harvesters_per_refinery" => Ok("Game-owned desired miner count per refinery"),
            "max_army" => Ok("Game-owned simultaneous army cap; no source economy/stat multiplier"),
            "wave_size" => Ok("Game-owned minimum wave size"),
            "first_wave" => Ok("Game-owned earliest wave tick"),
            "wave_patience" => Ok(
                "R158 game wave timeout; source TeamDelays remains independently audited and unchanged",
            ),
            "defence_radius" => Ok("Game-owned local observed defence radius"),
            _ => Err(format!("no explicit AI difficulty policy {path}")),
        };
    }
    Err(format!(
        "new raw AI field needs explicit semantic policy {path}"
    ))
}
pub(super) fn declare(a: &mut Audit, input: &Inputs) -> AuditResult<()> {
    let mut ai = BTreeMap::new();
    leaves(&input.documents["ai.ron"], "ai.ron", &mut ai)?;
    for path in ai.keys() {
        let _specific_reason = ai_reason(path)?;
        a.require(WitnessId::new("ai", path))?;
    }
    for file in [
        "units.ron",
        "economy.ron",
        "combat.ron",
        "arsenal.ron",
        "rules.ron",
        "skirmish.ron",
    ] {
        a.require(WitnessId::new("schema", &format!("{file}.schema_version")))?;
    }
    // Cosmetic item fields already have independently registered physical requirements.
    // Powerup physical rows must also be keyed independently, including disabled absence via
    // the separate membership contracts. No fictional zero-share physical row is registered.
    let names = [
        ("Armor", "armour"),
        ("Cloak", "cloak"),
        ("Darkness", "darkness"),
        ("Explosion", "explosion"),
        ("Firepower", "firepower"),
        ("Gas", "gas"),
        ("HealBase", "heal_base"),
        ("ICBM", "icbm"),
        ("Invulnerability", "invulnerability"),
        ("IonStorm", "ion_storm"),
        ("Money", "money"),
        ("Napalm", "napalm"),
        ("Pod", "pod"),
        ("Reveal", "reveal"),
        ("Speed", "speed"),
        ("Squad", "squad"),
        ("Tiberium", "tiberium"),
        ("Unit", "unit"),
        ("Veteran", "veteran"),
    ];
    for row in list(
        field(&input.documents["rules.ron"], "powerups", "rules.ron")?,
        "powerups",
    )? {
        let id = atom(field(row, "id", "powerups")?, "powerups")?;
        let (native, _) = names
            .iter()
            .find(|(_, game)| *game == id)
            .ok_or("unmapped raw powerup")?;
        let c = format!("powerup@rules:Powerups:{native}");
        for f in ["share", "value"] {
            let path = format!("rules.ron.powerups.{f}");
            a.require(WitnessId::new(&c, &path))?;
            bind(a, &c, &format!("rules.powerups.{f}"), &path)?;
        }
    }
    a.require(WitnessId::new("rules.crate_units", "rules.ron.crate_units"))?;
    bind(
        a,
        "rules.crate_units",
        "rules.crate_units.ordered_ids",
        "rules.ron.crate_units",
    )?;
    a.require(WitnessId::new("skirmish", "skirmish.ron.starting_credits"))?;
    bind(
        a,
        "skirmish",
        "skirmish.starting_credits",
        "skirmish.ron.starting_credits",
    )?;
    for row in list(
        field(&input.documents["skirmish.ron"], "sides", "skirmish.ron")?,
        "sides",
    )? {
        let id = atom(field(row, "side", "sides")?, "sides")?;
        let c = format!("start.{id}");
        for (physical, semantic) in [
            ("starting_vehicles", "vehicles"),
            ("starting_infantry", "infantry"),
        ] {
            let path = format!("skirmish.ron.sides.{physical}");
            a.require(WitnessId::new(&c, &path))?;
            bind(a, &c, &format!("skirmish.sides.{semantic}"), &path)?;
        }
    }
    for (index, row) in list(
        field(&input.documents["skirmish.ron"], "factions", "skirmish.ron")?,
        "factions",
    )?
    .iter()
    .enumerate()
    {
        let c = format!("skirmish.faction[{index}]");
        for (f, _) in tuple(row, "factions")? {
            let path = format!("skirmish.ron.factions.{f}");
            a.require(WitnessId::new(&c, &path))?;
            bind(a, &c, &format!("skirmish.factions.{f}"), &path)?;
        }
    }
    for row in list(
        field(&input.documents["skirmish.ron"], "maps", "skirmish.ron")?,
        "maps",
    )? {
        let id = atom(field(row, "id", "maps")?, "maps")?;
        let mut all = BTreeMap::new();
        leaves(row, "skirmish.ron.maps", &mut all)?;
        for path in all.keys() {
            a.require(WitnessId::new(&format!("map.{id}"), path))?;
        }
    }
    Ok(())
}
fn own(
    a: &mut Audit,
    c: &str,
    path: &str,
    actual: String,
    expected: &str,
    reason: &str,
    evidence: &str,
) -> AuditResult<()> {
    if actual != expected {
        return Err(format!(
            "actual loaded own field {c}/{path}: {actual:?}!={expected:?}"
        ));
    }
    a.game_owned(
        WitnessId::new(c, path),
        actual,
        reason.into(),
        evidence.into(),
    )
}
fn insert_list(out: &mut BTreeMap<String, String>, path: &str, values: Vec<String>) {
    out.insert(format!("{path}.len"), values.len().to_string());
    for (index, value) in values.into_iter().enumerate() {
        out.insert(format!("{path}[{index}]"), value);
    }
}
pub(super) fn project(
    a: &mut Audit,
    input: &Inputs,
    d: &Definitions,
    s: &SkirmishDefinitions,
) -> AuditResult<()> {
    let ai = sim::load_ai(input.data_dir.join("ai.ron"), d)
        .map_err(|e| format!("loaded AI projection setup: {e}"))?;
    let mut actual = BTreeMap::new();
    actual.insert(
        "ai.ron.schema_version".into(),
        sim::AI_SCHEMA_VERSION.to_string(),
    );
    actual.insert(
        "ai.ron.rules_difficulty".into(),
        u8::from(ai.rules_difficulty).to_string(),
    );
    for (f, v) in [
        ("garrison_quota", ai.policy.garrison_quota),
        ("garrison_radius", ai.policy.garrison_radius),
        ("depot_radius", ai.policy.depot_radius),
        ("naval_rescan", ai.policy.naval_rescan),
        ("naval_detector_quota", ai.policy.naval_detector_quota),
        ("artillery_min_range", ai.policy.artillery_min_range),
        ("artillery_standoff", ai.policy.artillery_standoff),
        ("paradrop_offset", ai.policy.paradrop_offset),
        ("chrono_min_offset", ai.policy.chrono_min_offset),
        ("chrono_max_offset", ai.policy.chrono_max_offset),
    ] {
        actual.insert(format!("ai.ron.policy.{f}"), v.to_string());
    }
    actual.insert("ai.ron.plans.len".into(), ai.plans.len().to_string());
    let rules = d.rules().ok_or("rules for actual country IDs")?;
    for (index, plan) in ai.plans.iter().enumerate() {
        let p = format!("ai.ron.plans[{index}]");
        actual.insert(
            format!("{p}.side"),
            plan.side.map_or("none", |s| s.id()).into(),
        );
        actual.insert(
            format!("{p}.country"),
            plan.country
                .map(|c| {
                    rules
                        .country(c)
                        .map(|v| v.id.clone())
                        .ok_or("missing loaded country")
                })
                .transpose()?
                .unwrap_or_default(),
        );
        for (f, values) in [
            ("build_order", &plan.build_order),
            ("defences", &plan.defences),
        ] {
            insert_list(
                &mut actual,
                &format!("{p}.{f}"),
                values
                    .iter()
                    .map(|&k| rules::building_id(d, k))
                    .collect::<AuditResult<Vec<_>>>()?,
            );
        }
        for (f, values) in [("army", &plan.army), ("support", &plan.support)] {
            insert_list(
                &mut actual,
                &format!("{p}.{f}"),
                values
                    .iter()
                    .map(|&k| rules::unit_id(d, k))
                    .collect::<AuditResult<Vec<_>>>()?,
            );
        }
        actual.insert(format!("{p}.power"), rules::building_id(d, plan.power)?);
        actual.insert(format!("{p}.harvester"), rules::unit_id(d, plan.harvester)?);
        actual.insert(
            format!("{p}.advanced_tech_level"),
            plan.advanced_tech_level.to_string(),
        );
        insert_list(
            &mut actual,
            &format!("{p}.tier_reserves"),
            plan.tier_reserves.iter().map(u32::to_string).collect(),
        );
    }
    actual.insert(
        "ai.ron.difficulties.len".into(),
        ai.difficulties.len().to_string(),
    );
    for (index, row) in ai.difficulties.iter().enumerate() {
        let p = format!("ai.ron.difficulties[{index}]");
        actual.insert(format!("{p}.id"), row.id.clone());
        for (f, v) in [
            ("think_interval", row.think_interval),
            ("harvesters_per_refinery", row.harvesters_per_refinery),
            ("max_army", row.max_army),
            ("wave_size", row.wave_size),
            ("first_wave", row.first_wave),
            ("wave_patience", row.wave_patience),
            ("defence_radius", row.defence_radius),
        ] {
            actual.insert(format!("{p}.{f}"), v.to_string());
        }
    }
    let mut raw_ai = BTreeMap::new();
    leaves(&input.documents["ai.ron"], "ai.ron", &mut raw_ai)?;
    if raw_ai.keys().collect::<BTreeSet<_>>() != actual.keys().collect::<BTreeSet<_>>() {
        return Err("AI raw/loaded physical projection keys differ".into());
    }
    for (path, expected) in raw_ai {
        own(
            a,
            "ai",
            &path,
            actual
                .remove(&path)
                .ok_or("missing AI physical projection")?,
            &expected,
            ai_reason(&path)?,
            "ai_defs.rs loader; ai.rs/ai/strategy.rs command policy; DECISIONS R85,R158,R162–R171",
        )?;
    }
    for (file, version) in [
        ("units.ron", 1),
        ("economy.ron", 2),
        ("combat.ron", 5),
        ("arsenal.ron", 10),
        ("rules.ron", 4),
        ("skirmish.ron", 3),
    ] {
        let value = atom(field(&input.documents[file], "schema_version", file)?, file)?;
        own(
            a,
            "schema",
            &format!("{file}.schema_version"),
            version.to_string(),
            value,
            "Game format schema, not native balance data; schema10 migration must update the explicit arsenal expectation",
            "data.rs and respective format modules",
        )?;
    }
    for row in &input.mappings {
        let item = items::item(d, row)?;
        let m = d.metadata(item).ok_or("actual metadata")?;
        let c = format!("{}@rules:{}", row.game_id, row.section);
        let raw_entry = list(
            field(&input.documents["arsenal.ron"], "entries", "arsenal.ron")?,
            "entries",
        )?
        .iter()
        .find(|v| {
            field(v, "id", "entries")
                .and_then(|v| atom(v, "id"))
                .is_ok_and(|id| id == row.game_id)
        })
        .ok_or("raw arsenal entry")?;
        for (f, v) in [
            ("label", &m.label),
            ("description", &m.description),
            ("art", &m.art),
        ] {
            own(
                a,
                &c,
                &format!("arsenal.ron.entries.{f}"),
                v.clone(),
                atom(field(raw_entry, f, "arsenal")?, f)?,
                if f == "art" {
                    "Original generated game art identity; native art asset reuse is excluded"
                } else {
                    "Original game-facing display text; no reference display-string port"
                },
                "arsenal.rs metadata; game battlefield/sidebar original presentation; LICENSES",
            )?;
        }
        if !row.building {
            if m.adjacent != 0 {
                return Err("unit-only adjacent sentinel".into());
            }
            a.game_owned(WitnessId::new(&c,"arsenal.ron.entries.adjacent"),m.adjacent.to_string(),"Placement adjacency is a building-only mechanic; unit field is the exact zero sentinel".into(),"arsenal kind validation; original audit unit-adjacent assertion retained".into())?;
            for (f, v) in [
                ("base_normal", m.base_normal),
                ("radar", m.radar),
                ("sellable", m.sellable),
                ("repairable", m.repairable),
                ("powered", m.powered),
                ("unit_repair", m.unit_repair),
            ] {
                if v {
                    return Err(format!("unit has unsupported building-only {f}"));
                }
                a.game_owned(WitnessId::new(&c,&format!("arsenal.ron.entries.{f}")),"0".into(),"Building-only production/power/placement mechanic has the explicit false unit sentinel".into(),"arsenal kind validation; every original unit building-trait assertion retained".into())?;
            }
        } else {
            for (f, v) in [
                ("veteran_abilities", &m.veteran_abilities),
                ("elite_abilities", &m.elite_abilities),
            ] {
                if !v.is_empty() {
                    return Err(format!("building has unsupported ranked abilities {f}"));
                }
                a.game_owned(WitnessId::new(&c,&format!("arsenal.ron.entries.{f}")),"[]".into(),"This model promotes trainable units; building rank-ability lists are explicit empty sentinels".into(),"arsenal unit-only trainability and veteran dispatch; remaining native model scope documented".into())?;
            }
        }
        let (f, l, identity, value) = match item {
            ProductionItem::Unit(k) => (
                "units.ron",
                "units",
                "id",
                d.unit(k).ok_or("unit")?.sprite.clone(),
            ),
            ProductionItem::Building(k) => (
                "economy.ron",
                "buildings",
                "id",
                d.building(k).ok_or("building")?.sprite.clone(),
            ),
        };
        let entry = list(field(&input.documents[f], l, f)?, l)?
            .iter()
            .find(|v| {
                field(v, identity, l)
                    .and_then(|v| atom(v, identity))
                    .is_ok_and(|id| id == row.game_id)
            })
            .ok_or("sprite raw row")?;
        own(
            a,
            &c,
            &format!("{f}.{l}.sprite"),
            value,
            atom(field(entry, "sprite", l)?, l)?,
            "Original generated sprite identity; native art assets are not shipped",
            "definitions/economy_defs loaded sprite; original game art generators and LICENSES",
        )?;
    }
    // Native-country raw roster lists are schema-only empties. The loader derives
    // FactionDef.roster through country_tree; compare that set with the independent
    // reference-based oracle, never with an invented loaded empty list.
    super::country_rosters::raw_roster_schema(input, d, s)?;
    super::country_rosters::compare(input, d, s)?;
    let raw_factions = list(
        field(&input.documents["skirmish.ron"], "factions", "skirmish")?,
        "factions",
    )?;
    if raw_factions.len() != s.factions.len() {
        return Err("raw/loaded country faction count differs".into());
    }
    for (index, (faction, raw)) in s.factions.iter().zip(raw_factions).enumerate() {
        let c = format!("skirmish.faction[{index}]");
        own(
            a,
            &c,
            "skirmish.ron.factions.label",
            faction.label.clone(),
            atom(field(raw, "label", "faction")?, "label")?,
            "Original game country display label; native Countries identity/order is independently checked",
            "skirmish_defs FactionDef.label; original game menu",
        )?;
        if !faction.buildings.is_empty() || !faction.units.is_empty() {
            return Err("shipped native country has unexpected legacy start objects".into());
        }
        for f in ["roster_buildings", "roster_units", "buildings", "units"] {
            if !list(field(raw, f, "faction")?, f)?.is_empty() {
                return Err("legacy raw country content must have a new explicit policy".into());
            }
            let (reason, evidence) = if f.starts_with("roster_") {
                (
                    "Native-country legacy raw roster arrays are schema-only empty inputs; the loaded country tech-tree roster is independently source-compared, not empty",
                    "skirmish_defs native-country validation; country_rosters::raw_roster_schema and independent compare; no loaded empty-roster witness",
                )
            } else {
                (
                    "Legacy explicit starting-object arrays are empty in both raw native-country input and actual loaded FactionDef",
                    "skirmish_defs native-country branch; actual FactionDef.buildings/units emptiness checked separately from derived roster",
                )
            };
            a.game_owned(
                WitnessId::new(&c, &format!("skirmish.ron.factions.{f}")),
                "[]".into(),
                reason.into(),
                evidence.into(),
            )?;
        }
    }
    let raw_maps = list(
        field(&input.documents["skirmish.ron"], "maps", "skirmish.ron")?,
        "maps",
    )?;
    if raw_maps.len() != s.maps.len() {
        return Err("raw/loaded authored map count differs".into());
    }
    for (index, map) in s.maps.iter().enumerate() {
        let raw = &raw_maps[index];
        let id = atom(field(raw, "id", "map")?, "id")?;
        let c = format!("map.{id}");
        let mut expected = BTreeMap::new();
        leaves(raw, "skirmish.ron.maps", &mut expected)?;
        let mut actual = BTreeMap::from([
            ("skirmish.ron.maps.id".into(), map.id.clone()),
            (
                "skirmish.ron.maps.starts.len".into(),
                map.starts.len().to_string(),
            ),
        ]);
        for (i, p) in map.starts.iter().enumerate() {
            actual.insert(format!("skirmish.ron.maps.starts[{i}].x"), p.x.to_string());
            actual.insert(format!("skirmish.ron.maps.starts[{i}].y"), p.y.to_string());
        }
        if actual.keys().collect::<BTreeSet<_>>() != expected.keys().collect::<BTreeSet<_>>() {
            return Err(format!("authored map physical keys differ {id}"));
        }
        for (path, value) in actual {
            own(
                a,
                &c,
                &path,
                value,
                &expected[&path],
                "Original authored map identifier/start coordinate; no RA2 map assets or native map-layout parity claim",
                "skirmish_defs MapDef/CellPos; original map generators and setup tests",
            )?;
        }
    }
    Ok(())
}
