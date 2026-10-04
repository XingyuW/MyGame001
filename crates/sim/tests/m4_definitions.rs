//! M4 tests: combat definitions from RON (PLAN.md sections 5 and 7, M4; invariants I8, I10,
//! I11, I12; docs/DECISIONS.md D18, D28).
//!
//! The combat file is checked against the supported subset of RON and its schema before `ron`
//! sees it, then validated: ranges at both ends, references to armour classes, warheads,
//! weapons, unit types, and building types, uniqueness, and completeness (every unit and
//! building type has combat terms, and every warhead lists every armour class). Every rejection
//! is a typed `DataError` naming the file, the field, and the reason. The tests are bounded as in
//! D15: this test binary's global allocator refuses allocations beyond [`MEMORY_BUDGET`], and
//! every load runs on a worker thread whose result must arrive within [`TIME_LIMIT`].
//!
//! Variants are derived from the committed fixture `tests/fixtures/data/combat.ron` by exact text
//! edits and written to Cargo's temporary directory for integration tests.

use std::alloc::{GlobalAlloc, Layout, System};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use sim::{
    ArmourClass, CombatTerms, DataError, DataErrorKind, Definitions, MAX_ARMOUR_CLASSES,
    MAX_DAMAGE, MAX_DATA_FILE_BYTES, MAX_HIT_POINTS, MAX_PROJECTILE_SPEED, MAX_RELOAD, MAX_SIGHT,
    MAX_VERSUS_PERCENT, MAX_WARHEADS, MAX_WEAPON_RANGE, MAX_WEAPONS, MAX_WEAPONS_PER_TYPE,
    MIN_PROJECTILE_SPEED, MIN_WEAPON_RANGE, WarheadKind, WeaponDef, WeaponKind, load_definitions,
};

/// Upper limit of heap bytes in use by this test process.
const MEMORY_BUDGET: usize = 64 << 20;
/// Upper limit for one load.
const TIME_LIMIT: Duration = Duration::from_secs(30);

static IN_USE: AtomicUsize = AtomicUsize::new(0);

struct BudgetAllocator;

fn reserve(size: usize) -> bool {
    IN_USE
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |in_use| {
            in_use
                .checked_add(size)
                .filter(|&total| total <= MEMORY_BUDGET)
        })
        .is_ok()
}

fn release(size: usize) {
    IN_USE.fetch_sub(size, Ordering::SeqCst);
}

// SAFETY: every call is forwarded to `System` unchanged; the counter only decides whether to
// forward an allocation at all, and failing an allocation by returning null is permitted.
unsafe impl GlobalAlloc for BudgetAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !reserve(layout.size()) {
            return std::ptr::null_mut();
        }
        // SAFETY: forwarded with the caller's guarantees.
        let pointer = unsafe { System.alloc(layout) };
        if pointer.is_null() {
            release(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if !reserve(layout.size()) {
            return std::ptr::null_mut();
        }
        // SAFETY: forwarded with the caller's guarantees.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if pointer.is_null() {
            release(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: forwarded with the caller's guarantees.
        unsafe { System.dealloc(pointer, layout) };
        release(layout.size());
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let old_size = layout.size();
        if new_size > old_size && !reserve(new_size - old_size) {
            return std::ptr::null_mut();
        }
        // SAFETY: forwarded with the caller's guarantees.
        let new_pointer = unsafe { System.realloc(pointer, layout, new_size) };
        if new_pointer.is_null() {
            if new_size > old_size {
                release(new_size - old_size);
            }
        } else if new_size < old_size {
            release(old_size - new_size);
        }
        new_pointer
    }
}

#[global_allocator]
static ALLOCATOR: BudgetAllocator = BudgetAllocator;

/// Loads the combat file at `path` with the fixture units and economy files on a worker thread,
/// failing the test if the loader panics or takes longer than [`TIME_LIMIT`].
fn load(path: &Path) -> Result<Definitions, DataError> {
    let owned = path.to_owned();
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name(format!("load {}", path.display()))
        .spawn(move || {
            let result = load_definitions(
                fixtures().join("units_combat.ron"),
                fixtures().join("economy_combat.ron"),
            )
            .and_then(|definitions| definitions.with_combat(&owned));
            let _ = sender.send(result);
        })
        .expect("worker thread");
    match receiver.recv_timeout(TIME_LIMIT) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => {
            panic!("{}: no result within {TIME_LIMIT:?}", path.display())
        }
        Err(RecvTimeoutError::Disconnected) => panic!("{}: the loader panicked", path.display()),
    }
}

// --- Files ----------------------------------------------------------------------------------

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data")
}

fn fixture() -> PathBuf {
    fixtures().join("combat.ron")
}

fn fixture_text() -> String {
    std::fs::read_to_string(fixture()).expect("fixture exists")
}

fn temp_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("m4_definitions");
    std::fs::create_dir_all(&dir).expect("temporary directory");
    dir
}

fn temp_file(name: &str, contents: &[u8]) -> PathBuf {
    let path = temp_dir().join(name);
    std::fs::write(&path, contents).expect("temporary file");
    path
}

/// The fixture with each `(from, to)` edit applied; `from` must occur exactly once.
fn variant(name: &str, edits: &[(&str, &str)]) -> PathBuf {
    let mut text = fixture_text();
    for (from, to) in edits {
        assert_eq!(
            text.matches(from).count(),
            1,
            "{name}: fixture edit {from:?}"
        );
        text = text.replacen(from, to, 1);
    }
    temp_file(&format!("{name}.ron"), text.as_bytes())
}

fn load_ok(path: &Path) -> Definitions {
    load(path).unwrap_or_else(|error| panic!("{error}"))
}

/// Asserts that `path` is rejected with `kind` at `field`, and that the message names the file,
/// the field, and the reason.
fn assert_rejected(path: &Path, field: &str, kind: DataErrorKind) {
    let error = load(path).expect_err("the file must be rejected");
    assert_eq!(
        (error.field.as_str(), &error.kind),
        (field, &kind),
        "{error}"
    );
    assert_eq!(error.file, path);
    let message = error.to_string();
    for part in [
        format!("data file {}", path.display()),
        field.to_string(),
        kind.to_string(),
    ] {
        assert!(message.contains(&part), "{message:?} lacks {part:?}");
    }
}

/// Asserts that `path` is rejected at `field` because the value `found` is out of range.
fn assert_out_of_range(path: &Path, field: &str, found: &str) {
    let error = load(path).expect_err("the file must be rejected");
    assert_eq!(error.field, field, "{error}");
    assert_eq!(error.file, path);
    match &error.kind {
        DataErrorKind::InvalidValue { found: f, .. } => assert_eq!(f, found, "{error}"),
        other => panic!("expected an invalid value, found {other:?}: {error}"),
    }
}

fn unknown(kind: &str, id: &str) -> DataErrorKind {
    DataErrorKind::UnknownReference {
        kind: kind.into(),
        id: id.into(),
    }
}

fn missing(kind: &str, id: &str) -> DataErrorKind {
    DataErrorKind::MissingEntry {
        kind: kind.into(),
        id: id.into(),
    }
}

// --- Positive -------------------------------------------------------------------------------

#[test]
fn the_fixture_loads_with_every_field() {
    let definitions = load_ok(&fixture());
    assert!(definitions.has_combat());
    assert_eq!(definitions.armour_classes(), ["soft", "plate", "concrete"]);
    let ids: Vec<&str> = definitions
        .warheads()
        .iter()
        .map(|warhead| warhead.id.as_str())
        .collect();
    assert_eq!(ids, ["bullet", "ap", "he"]);
    // The ap warhead lists its armour classes out of order; they are stored by class.
    assert_eq!(definitions.warheads()[1].versus, [30, 100, 50]);
    assert_eq!(definitions.warheads()[0].versus, [100, 1, 0]);
    let class = |name| definitions.armour_class(name).expect("class");
    assert_eq!(
        definitions.versus(WarheadKind(2), class("concrete")),
        Some(75)
    );
    assert_eq!(definitions.versus(WarheadKind(3), class("soft")), None);
    assert_eq!(
        definitions.versus(WarheadKind(0), ArmourClass(3)),
        None,
        "no such class"
    );
    assert_eq!(
        definitions.weapons()[1],
        WeaponDef {
            id: "cannon".into(),
            damage: 25,
            range: 1024,
            reload: 20,
            warhead: WarheadKind(1),
            projectile_speed: 128,
        }
    );
    assert_eq!(definitions.weapon_kind("howitzer"), Some(WeaponKind(2)));
    assert_eq!(definitions.weapon_kind("sword"), None);
    let unit = |id| definitions.unit_kind(id).expect("unit");
    assert_eq!(
        definitions.unit_combat(unit("tank")),
        Some(CombatTerms {
            hit_points: 300,
            armour: class("plate"),
            sight: 6,
            weapon: Some(WeaponKind(1)),
        })
    );
    assert_eq!(
        definitions
            .unit_combat(unit("crawler"))
            .map(|terms| terms.weapon),
        Some(None)
    );
    let building = |id| definitions.building_kind(id).expect("building");
    assert_eq!(
        definitions.building_combat(building("turret")),
        Some(CombatTerms {
            hit_points: 400,
            armour: class("concrete"),
            sight: 3,
            weapon: Some(WeaponKind(2)),
        })
    );
    assert_eq!(
        definitions
            .building_combat(building("fence"))
            .map(|t| t.sight),
        Some(0)
    );
    // Every unit and building type has terms.
    for index in 0..definitions.units().len() {
        let kind = sim::UnitKind(u16::try_from(index).expect("index"));
        assert!(definitions.unit_combat(kind).is_some());
    }
    for index in 0..definitions.buildings().len() {
        let kind = sim::BuildingKind(u16::try_from(index).expect("index"));
        assert!(definitions.building_combat(kind).is_some());
    }
}

#[test]
fn an_economy_file_loaded_after_the_combat_file_removes_it() {
    // Combat terms refer to the building types of the economy file they were loaded with.
    let definitions = load_ok(&fixture());
    let reloaded = definitions
        .with_economy(fixtures().join("economy_combat.ron"))
        .expect("economy");
    assert!(!reloaded.has_combat());
    assert_eq!(reloaded.unit_combat(sim::UnitKind(0)), None);
    assert_eq!(reloaded.building_combat(sim::BuildingKind(0)), None);
    // Without an economy file, building entries name unknown buildings.
    let units_only = sim::load_unit_definitions(fixtures().join("units_combat.ron"))
        .expect("units")
        .with_combat(fixture())
        .expect_err("buildings are unknown");
    assert_eq!(units_only.field, "buildings[0].building");
    assert_eq!(units_only.kind, unknown("building", "yard"));
}

// --- Negative: structure --------------------------------------------------------------------

#[test]
fn every_field_is_required_and_unknown_fields_are_rejected() {
    // (unique text, its replacement without the field, and the field reported missing)
    let missing_fields = [
        (
            "    armour_classes: [\"soft\", \"plate\", \"concrete\"],\n",
            "",
            "armour_classes",
        ),
        ("            id: \"bullet\",\n", "", "warheads[0].id"),
        (
            "(armour: \"plate\", percent: 1)",
            "(armour: \"plate\")",
            "warheads[0].versus[1].percent",
        ),
        (
            "(armour: \"plate\", percent: 100)",
            "(percent: 100)",
            "warheads[1].versus[0].armour",
        ),
        ("            damage: 20,\n", "", "weapons[0].damage"),
        ("            range: 768,\n", "", "weapons[0].range"),
        ("            reload: 10,\n", "", "weapons[0].reload"),
        (
            "            warhead: \"bullet\",\n",
            "",
            "weapons[0].warhead",
        ),
        (
            "            projectile_speed: 0,\n",
            "",
            "weapons[0].projectile_speed",
        ),
        (
            "(unit: \"tank\", hit_points: 300, armour: \"plate\", sight: 6, weapons: [\"cannon\"])",
            "(unit: \"tank\", hit_points: 300, armour: \"plate\", weapons: [\"cannon\"])",
            "units[1].sight",
        ),
        (
            "(unit: \"tank\", hit_points: 300, armour: \"plate\", sight: 6, weapons: [\"cannon\"])",
            "(unit: \"tank\", armour: \"plate\", sight: 6, weapons: [\"cannon\"])",
            "units[1].hit_points",
        ),
        (
            "(unit: \"tank\", hit_points: 300, armour: \"plate\", sight: 6, weapons: [\"cannon\"])",
            "(unit: \"tank\", hit_points: 300, armour: \"plate\", sight: 6)",
            "units[1].weapons",
        ),
        (
            "(building: \"power\", hit_points: 200, armour: \"concrete\", sight: 3, weapons: [])",
            "(building: \"power\", hit_points: 200, sight: 3, weapons: [])",
            "buildings[1].armour",
        ),
        (
            "(building: \"power\", hit_points: 200, armour: \"concrete\", sight: 3, weapons: [])",
            "(hit_points: 200, armour: \"concrete\", sight: 3, weapons: [])",
            "buildings[1].building",
        ),
    ];
    for (index, (from, to, field)) in missing_fields.into_iter().enumerate() {
        let path = variant(&format!("missing_{index}"), &[(from, to)]);
        assert_rejected(&path, field, DataErrorKind::MissingField);
    }
    // Whole top-level lists.
    for list in ["warheads", "weapons", "units", "buildings"] {
        let text = fixture_text();
        let start = text.find(&format!("    {list}: [")).expect("list");
        let end = start + text[start..].find("\n    ],\n").expect("end") + "\n    ],\n".len();
        let path = temp_file(
            &format!("missing_{list}.ron"),
            format!("{}{}", &text[..start], &text[end..]).as_bytes(),
        );
        assert_rejected(&path, list, DataErrorKind::MissingField);
    }
    for (name, from, to, field) in [
        (
            "unknown_top_field",
            "    armour_classes: [",
            "    factions: [],\n    armour_classes: [",
            "factions",
        ),
        (
            "unknown_weapon_field",
            "            reload: 10,\n",
            "            reload: 10,\n            burst: 3,\n",
            "weapons[0].burst",
        ),
        (
            "unknown_versus_field",
            "(armour: \"soft\", percent: 150)",
            "(armour: \"soft\", percent: 150, splash: 1)",
            "warheads[2].versus[0].splash",
        ),
        (
            "unknown_unit_field",
            "(unit: \"crawler\", hit_points: 50,",
            "(unit: \"crawler\", speed: 3, hit_points: 50,",
            "units[2].speed",
        ),
    ] {
        assert_rejected(
            &variant(name, &[(from, to)]),
            field,
            DataErrorKind::UnknownField,
        );
    }
    assert_rejected(
        &variant(
            "duplicate_field",
            &[(
                "            range: 1024,\n",
                "            range: 1024,\n            range: 1,\n",
            )],
        ),
        "weapons[1].range",
        DataErrorKind::DuplicateField,
    );
    // Values of the wrong kind are syntax errors at their field.
    for (name, from, to, field) in [
        (
            "damage_string",
            "damage: 40,",
            "damage: \"40\",",
            "weapons[2].damage",
        ),
        (
            "weapons_string",
            "weapons: [\"howitzer\"]",
            "weapons: \"howitzer\"",
            "buildings[6].weapons",
        ),
        (
            "armour_number",
            "armour_classes: [\"soft\", \"plate\", \"concrete\"]",
            "armour_classes: [\"soft\", 2, \"concrete\"]",
            "armour_classes[1]",
        ),
    ] {
        let error = load(&variant(name, &[(from, to)])).expect_err("rejected");
        assert_eq!(error.field, field, "{error}");
        assert!(
            matches!(error.kind, DataErrorKind::Syntax { .. }),
            "{error}"
        );
    }
}

#[test]
fn schema_versions_other_than_1_are_rejected() {
    for version in [0, 2, -1, i64::MIN, i64::MAX] {
        assert_rejected(
            &variant(
                &format!("version_{version}"),
                &[("schema_version: 1,", &format!("schema_version: {version},"))],
            ),
            "schema_version",
            DataErrorKind::UnsupportedSchemaVersion {
                found: version,
                expected: 1,
            },
        );
    }
}

// --- Negative and boundary: ranges ----------------------------------------------------------

#[test]
fn numeric_fields_accept_their_limits_and_reject_beyond() {
    // (unique fixture text, field, the replacement with `{}` for the value, smallest, largest)
    let cases: [(&str, &str, &str, i64, i64); 7] = [
        (
            "damage: 20,",
            "weapons[0].damage",
            "damage: {},",
            1,
            i64::from(MAX_DAMAGE),
        ),
        (
            "range: 768,",
            "weapons[0].range",
            "range: {},",
            i64::from(MIN_WEAPON_RANGE),
            i64::from(MAX_WEAPON_RANGE),
        ),
        (
            "reload: 10,",
            "weapons[0].reload",
            "reload: {},",
            1,
            i64::from(MAX_RELOAD),
        ),
        (
            "(armour: \"soft\", percent: 100)",
            "warheads[0].versus[0].percent",
            "(armour: \"soft\", percent: {})",
            0,
            i64::from(MAX_VERSUS_PERCENT),
        ),
        (
            "(unit: \"rifleman\", hit_points: 100,",
            "units[0].hit_points",
            "(unit: \"rifleman\", hit_points: {},",
            1,
            i64::from(MAX_HIT_POINTS),
        ),
        (
            "armour: \"soft\", sight: 5,",
            "units[0].sight",
            "armour: \"soft\", sight: {},",
            0,
            i64::from(MAX_SIGHT),
        ),
        (
            "(building: \"yard\", hit_points: 500,",
            "buildings[0].hit_points",
            "(building: \"yard\", hit_points: {},",
            1,
            i64::from(MAX_HIT_POINTS),
        ),
    ];
    for (from, field, template, min, max) in cases {
        for value in [min, max] {
            let path = variant(
                &format!("ok_{}_{value}", field.replace(['[', ']', '.'], "_")),
                &[(from, &template.replace("{}", &value.to_string()))],
            );
            load_ok(&path);
        }
        for value in [min - 1, max + 1, i64::MIN, i64::MAX] {
            let path = variant(
                &format!("bad_{}_{value}", field.replace(['[', ']', '.'], "_")),
                &[(from, &template.replace("{}", &value.to_string()))],
            );
            assert_out_of_range(&path, field, &value.to_string());
        }
    }
    // The projectile speed: 0 (instant), or MIN_PROJECTILE_SPEED to MAX_PROJECTILE_SPEED.
    let speed = |value: i64| {
        variant(
            &format!("speed_{value}"),
            &[(
                "projectile_speed: 128,",
                &format!("projectile_speed: {value},"),
            )],
        )
    };
    for value in [
        0,
        i64::from(MIN_PROJECTILE_SPEED),
        i64::from(MAX_PROJECTILE_SPEED),
    ] {
        let definitions = load_ok(&speed(value));
        assert_eq!(i64::from(definitions.weapons()[1].projectile_speed), value);
    }
    for value in [
        -1,
        1,
        i64::from(MIN_PROJECTILE_SPEED) - 1,
        i64::from(MAX_PROJECTILE_SPEED) + 1,
        i64::MAX,
    ] {
        assert_out_of_range(
            &speed(value),
            "weapons[1].projectile_speed",
            &value.to_string(),
        );
    }
    // An integer beyond i64 is rejected by the checker at its field.
    let error = load(&variant(
        "beyond_i64",
        &[("damage: 20,", "damage: 9223372036854775808,")],
    ))
    .expect_err("rejected");
    assert_eq!(error.field, "weapons[0].damage");
}

/// A combat file with `classes` armour classes, `warheads` warheads (each listing every class),
/// `weapons` weapons, and the fixture's unit and building types, the tank with `per_type`
/// weapons and `extra_units` further unit entries.
fn generated(
    name: &str,
    classes: usize,
    warheads: usize,
    weapons: usize,
    per_type: usize,
    extra_units: usize,
) -> PathBuf {
    let class_names: Vec<String> = (0..classes).map(|c| format!("\"a{c}\"")).collect();
    let versus: Vec<String> = (0..classes)
        .map(|c| format!("(armour: \"a{c}\", percent: 100)"))
        .collect();
    let warhead_list: Vec<String> = (0..warheads)
        .map(|w| format!("(id: \"w{w}\", versus: [{}])", versus.join(", ")))
        .collect();
    let weapon_list: Vec<String> = (0..weapons)
        .map(|k| {
            format!(
                "(id: \"k{k}\", damage: 1, range: 384, reload: 1, warhead: \"w0\", \
                 projectile_speed: 0)"
            )
        })
        .collect();
    let tank_weapons: Vec<String> = (0..per_type).map(|k| format!("\"k{k}\"")).collect();
    let mut units: Vec<String> = ["rifleman", "crawler", "harvester"]
        .iter()
        .map(|id| format!("(unit: \"{id}\", hit_points: 1, armour: \"a0\", sight: 0, weapons: [])"))
        .collect();
    units.push(format!(
        "(unit: \"tank\", hit_points: 1, armour: \"a0\", sight: 0, weapons: [{}])",
        tank_weapons.join(", ")
    ));
    for _ in 0..extra_units {
        units.push("(unit: \"tank\", hit_points: 1, armour: \"a0\", sight: 0, weapons: [])".into());
    }
    let buildings: Vec<String> = [
        "yard", "power", "refinery", "barracks", "factory", "fence", "turret", "beacon", "flag",
    ]
    .iter()
    .map(|id| format!("(building: \"{id}\", hit_points: 1, armour: \"a0\", sight: 0, weapons: [])"))
    .collect();
    let text = format!(
        "(schema_version: 1, armour_classes: [{}], warheads: [{}], weapons: [{}], units: [{}], \
         buildings: [{}])",
        class_names.join(", "),
        warhead_list.join(",\n"),
        weapon_list.join(",\n"),
        units.join(",\n"),
        buildings.join(",\n"),
    );
    temp_file(&format!("{name}.ron"), text.as_bytes())
}

#[test]
fn list_lengths_are_bounded() {
    let most = load_ok(&generated(
        "most",
        MAX_ARMOUR_CLASSES,
        MAX_WARHEADS,
        MAX_WEAPONS,
        MAX_WEAPONS_PER_TYPE,
        0,
    ));
    assert_eq!(most.armour_classes().len(), MAX_ARMOUR_CLASSES);
    assert_eq!(most.warheads().len(), MAX_WARHEADS);
    assert_eq!(most.weapons().len(), MAX_WEAPONS);
    assert!(
        most.warheads()
            .iter()
            .all(|warhead| warhead.versus.len() == MAX_ARMOUR_CLASSES)
    );
    // No weapons and no warheads at all is a valid (peaceful) file.
    let none = load_ok(&generated("fewest", 1, 0, 0, 0, 0));
    assert!(none.weapons().is_empty() && none.warheads().is_empty());
    for (name, path, field, max) in [
        (
            "classes",
            generated("too_many_classes", MAX_ARMOUR_CLASSES + 1, 1, 1, 1, 0),
            "armour_classes",
            MAX_ARMOUR_CLASSES,
        ),
        (
            "warheads",
            generated("too_many_warheads", 1, MAX_WARHEADS + 1, 1, 1, 0),
            "warheads",
            MAX_WARHEADS,
        ),
        (
            "weapons",
            generated("too_many_weapons", 1, 1, MAX_WEAPONS + 1, 1, 0),
            "weapons",
            MAX_WEAPONS,
        ),
        (
            "per_type",
            generated("two_weapons", 1, 1, 2, MAX_WEAPONS_PER_TYPE + 1, 0),
            "units[3].weapons",
            MAX_WEAPONS_PER_TYPE,
        ),
        (
            "units",
            generated("too_many_units", 1, 1, 1, 1, sim::MAX_UNIT_TYPES - 3),
            "units",
            sim::MAX_UNIT_TYPES,
        ),
    ] {
        let error = load(&path).expect_err(name);
        assert_eq!(
            (error.field.as_str(), error.kind),
            (field, DataErrorKind::TooManyEntries { max }),
            "{name}"
        );
    }
    // A warhead listing more entries than there can be armour classes.
    let most_text =
        std::fs::read_to_string(generated("most_again", MAX_ARMOUR_CLASSES, 1, 1, 1, 0))
            .expect("generated");
    let path = temp_file(
        "too_many_versus.ron",
        most_text
            .replacen("versus: [", "versus: [(armour: \"a0\", percent: 1), ", 1)
            .as_bytes(),
    );
    assert_rejected(
        &path,
        "warheads[0].versus",
        DataErrorKind::TooManyEntries {
            max: MAX_ARMOUR_CLASSES,
        },
    );
    // At least one armour class.
    assert_rejected(
        &generated("no_classes", 0, 0, 0, 0, 0),
        "armour_classes",
        DataErrorKind::InvalidValue {
            found: "an empty list".into(),
            expected: format!("1 to {MAX_ARMOUR_CLASSES} armour classes"),
        },
    );
}

// --- Negative: references, completeness, uniqueness, names ----------------------------------

#[test]
fn references_must_name_defined_entries() {
    let cases = [
        (
            "(armour: \"concrete\", percent: 0)",
            "(armour: \"steel\", percent: 0)",
            "warheads[0].versus[2].armour",
            unknown("armour class", "steel"),
        ),
        (
            "warhead: \"ap\",",
            "warhead: \"apds\",",
            "weapons[1].warhead",
            unknown("warhead", "apds"),
        ),
        (
            "weapons: [\"rifle\"]",
            "weapons: [\"bayonet\"]",
            "units[0].weapons[0]",
            unknown("weapon", "bayonet"),
        ),
        (
            "(unit: \"crawler\",",
            "(unit: \"snail\",",
            "units[2].unit",
            unknown("unit", "snail"),
        ),
        (
            "armour: \"plate\", sight: 3, weapons: []",
            "armour: \"tin\", sight: 3, weapons: []",
            "units[3].armour",
            unknown("armour class", "tin"),
        ),
        (
            "(building: \"beacon\",",
            "(building: \"lighthouse\",",
            "buildings[7].building",
            unknown("building", "lighthouse"),
        ),
        (
            "weapons: [\"howitzer\"]",
            "weapons: [\"mortar\"]",
            "buildings[6].weapons[0]",
            unknown("weapon", "mortar"),
        ),
    ];
    for (index, (from, to, field, kind)) in cases.into_iter().enumerate() {
        assert_rejected(
            &variant(&format!("reference_{index}"), &[(from, to)]),
            field,
            kind,
        );
    }
}

#[test]
fn every_type_and_armour_class_needs_exactly_one_entry() {
    // Missing entries: no silent default hit points, armour, sight, or percentage.
    assert_rejected(
        &variant(
            "no_crawler",
            &[(
                "        (unit: \"crawler\", hit_points: 50, armour: \"soft\", sight: 2, weapons: []),\n",
                "",
            )],
        ),
        "units",
        missing("unit", "crawler"),
    );
    assert_rejected(
        &variant(
            "no_flag",
            &[(
                "        (building: \"flag\", hit_points: 1, armour: \"concrete\", sight: 0, weapons: []),\n",
                "",
            )],
        ),
        "buildings",
        missing("building", "flag"),
    );
    assert_rejected(
        &variant(
            "versus_without_plate",
            &[("                (armour: \"plate\", percent: 40),\n", "")],
        ),
        "warheads[2].versus",
        missing("armour class", "plate"),
    );
    // Duplicates.
    let duplicates = [
        (
            "armour_classes: [\"soft\", \"plate\", \"concrete\"]",
            "armour_classes: [\"soft\", \"plate\", \"soft\"]",
            "armour_classes[2]",
            "soft",
        ),
        (
            "            id: \"he\",",
            "            id: \"ap\",",
            "warheads[2].id",
            "ap",
        ),
        (
            "            id: \"howitzer\",",
            "            id: \"rifle\",",
            "weapons[2].id",
            "rifle",
        ),
        (
            "                (armour: \"concrete\", percent: 50),",
            "                (armour: \"soft\", percent: 50),",
            "warheads[1].versus[2].armour",
            "soft",
        ),
        (
            "(unit: \"crawler\",",
            "(unit: \"rifleman\",",
            "units[2].unit",
            "rifleman",
        ),
        (
            "(building: \"beacon\",",
            "(building: \"yard\",",
            "buildings[7].building",
            "yard",
        ),
    ];
    for (index, (from, to, field, id)) in duplicates.into_iter().enumerate() {
        assert_rejected(
            &variant(&format!("duplicate_{index}"), &[(from, to)]),
            field,
            DataErrorKind::DuplicateId(id.into()),
        );
    }
}

#[test]
fn names_are_checked() {
    for (index, (from, to, field)) in [
        (
            "\"soft\", \"plate\", \"concrete\"]",
            "\"Soft\", \"plate\", \"concrete\"]",
            "armour_classes[0]",
        ),
        (
            "            id: \"bullet\",",
            "            id: \"\",",
            "warheads[0].id",
        ),
        (
            "            id: \"cannon\",",
            "            id: \"a_very_long_weapon_name_of_33_chars\",",
            "weapons[1].id",
        ),
        (
            "            id: \"rifle\",",
            "            id: \"9mm\",",
            "weapons[0].id",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let error = load(&variant(&format!("name_{index}"), &[(from, to)])).expect_err("name");
        assert_eq!(error.field, field, "{error}");
        assert!(
            matches!(error.kind, DataErrorKind::InvalidValue { .. }),
            "{error}"
        );
    }
}

// --- Negative: files ------------------------------------------------------------------------

#[test]
fn unreadable_oversized_and_special_files_are_rejected() {
    let missing_file = temp_dir().join("does_not_exist.ron");
    let error = load(&missing_file).expect_err("missing");
    assert!(matches!(error.kind, DataErrorKind::Read(_)), "{error}");
    assert_eq!(
        (error.file.as_path(), error.field.as_str()),
        (missing_file.as_path(), "file")
    );
    let mut text = fixture_text().into_bytes();
    text.resize(MAX_DATA_FILE_BYTES as usize + 1, b' ');
    assert_rejected(
        &temp_file("too_large.ron", &text),
        "file",
        DataErrorKind::FileTooLarge {
            max_bytes: MAX_DATA_FILE_BYTES,
        },
    );
    let fifo = temp_dir().join("pipe.ron");
    let _ = std::fs::remove_file(&fifo);
    let made = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("mkfifo runs");
    assert!(made.success(), "mkfifo failed");
    assert_rejected(
        &fifo,
        "file",
        DataErrorKind::Read("not a regular file".into()),
    );
    std::fs::remove_file(&fifo).expect("remove the pipe");
    assert_rejected(
        &temp_dir(),
        "file",
        DataErrorKind::Read("not a regular file".into()),
    );
}

#[test]
fn truncations_of_the_fixture_are_rejected_without_panic() {
    let text = fixture_text();
    let end = text.rfind(')').expect("closing parenthesis");
    for cut in (0..=end).step_by(5).chain([end]) {
        let path = temp_file("prefix.ron", &text.as_bytes()[..cut]);
        let error = load(&path).expect_err("a truncated file must be rejected");
        assert_eq!(error.file, path);
        assert!(!error.field.is_empty());
    }
}

// --- Mutation -------------------------------------------------------------------------------

/// SplitMix64 with a fixed seed, so the cases are the same on every run.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound.max(1) as u64) as usize
    }
}

const SNIPPETS: [&str; 22] = [
    "(",
    ")",
    "[",
    "]",
    ",",
    ":",
    "\"",
    "//",
    "#",
    "-",
    "0",
    "99999999999999999999",
    "\"soft\"",
    "\"cannon\", ",
    "\"tank\"",
    "(armour: \"plate\", percent: 5),",
    "projectile_speed",
    "weapons: [],",
    "{",
    "1.5",
    "\n",
    "\u{e9}",
];

#[test]
fn seeded_mutations_of_the_fixture_load_or_return_a_typed_error() {
    let base = fixture_text().into_bytes();
    let mut rng = SplitMix64(0x0e04_c0b4);
    let path = temp_dir().join("mutation.ron");
    let mut loaded = 0;
    for case in 0..2_000 {
        let mut bytes = base.clone();
        for _ in 0..1 + rng.below(3) {
            let at = rng.below(bytes.len() + 1);
            match rng.below(5) {
                0 => {
                    let snippet = SNIPPETS[rng.below(SNIPPETS.len())].as_bytes();
                    let repeat = if rng.below(10) == 0 { 2_000 } else { 1 };
                    bytes.splice(at..at, snippet.repeat(repeat));
                }
                1 => {
                    let end = (at + rng.below(20)).min(bytes.len());
                    bytes.drain(at..end);
                }
                2 => {
                    let end = (at + rng.below(60)).min(bytes.len());
                    let copy = bytes[at..end].to_vec();
                    bytes.splice(at..at, copy);
                }
                3 => {
                    if at < bytes.len() {
                        bytes[at] = (rng.next() & 0xff) as u8;
                    }
                }
                _ => bytes.truncate(at),
            }
        }
        std::fs::write(&path, &bytes).expect("mutation file");
        match load(&path) {
            Ok(definitions) => {
                loaded += 1;
                let classes = definitions.armour_classes().len();
                assert!((1..=MAX_ARMOUR_CLASSES).contains(&classes), "case {case}");
                for warhead in definitions.warheads() {
                    assert_eq!(warhead.versus.len(), classes, "case {case}");
                    assert!(
                        warhead
                            .versus
                            .iter()
                            .all(|v| (0..=MAX_VERSUS_PERCENT).contains(v))
                    );
                }
                for weapon in definitions.weapons() {
                    assert!((1..=MAX_DAMAGE).contains(&weapon.damage), "case {case}");
                    assert!((MIN_WEAPON_RANGE..=MAX_WEAPON_RANGE).contains(&weapon.range));
                    assert!((1..=MAX_RELOAD).contains(&weapon.reload));
                    assert!(
                        weapon.projectile_speed == 0
                            || (MIN_PROJECTILE_SPEED..=MAX_PROJECTILE_SPEED)
                                .contains(&weapon.projectile_speed)
                    );
                    assert!(usize::from(weapon.warhead.0) < definitions.warheads().len());
                }
                for index in 0..definitions.units().len() {
                    let kind = sim::UnitKind(u16::try_from(index).expect("index"));
                    let terms = definitions.unit_combat(kind).expect("complete");
                    assert!((1..=MAX_HIT_POINTS).contains(&terms.hit_points));
                    assert!(terms.sight <= MAX_SIGHT);
                    assert!(usize::from(terms.armour.0) < classes);
                }
            }
            Err(error) => {
                assert_eq!(error.file, path, "case {case}");
                assert!(!error.field.is_empty(), "case {case}");
                assert!(
                    !matches!(error.kind, DataErrorKind::Parse(_)),
                    "case {case}: ron rejected a file the checker accepted: {error}"
                );
            }
        }
    }
    assert!(loaded > 0, "no mutation stayed valid");
}
