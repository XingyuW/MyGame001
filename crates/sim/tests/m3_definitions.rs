//! M3 tests: economy definitions from RON (PLAN.md sections 5 and 7, M3; invariants I8, I10,
//! I11, I12; docs/DECISIONS.md D18, D23).
//!
//! The economy file is checked against the supported subset of RON and its schema before `ron`
//! sees it, then validated: ranges at both ends, references to building and unit types,
//! uniqueness, and acyclic prerequisites. Every rejection is a typed `DataError` naming the file,
//! the field, and the reason. The tests are bounded as in D15: this test binary's global
//! allocator refuses allocations beyond [`MEMORY_BUDGET`], and every load runs on a worker thread
//! whose result must arrive within [`TIME_LIMIT`].
//!
//! Variants are derived from the committed fixture `tests/fixtures/data/economy.ron` by exact
//! text edits and written to Cargo's temporary directory for integration tests.

use std::alloc::{GlobalAlloc, Layout, System};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use sim::{
    BuildingKind, DataError, DataErrorKind, Definitions, HarvesterDef, MAX_BUILD_TIME,
    MAX_BUILDING_TYPES, MAX_COST, MAX_CREDITS_PER_ORE, MAX_DATA_FILE_BYTES, MAX_FOOTPRINT_SIDE,
    MAX_FREE_UNITS, MAX_ORE_CAPACITY, MAX_PLACEMENT_DISTANCE, MAX_POWER, MAX_PREREQUISITES,
    Production, ProductionItem, UnitKind, load_definitions,
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

/// Loads the economy file at `path` (with the fixture units file) on a worker thread, failing
/// the test if the loader panics or takes longer than [`TIME_LIMIT`].
fn load(path: &Path) -> Result<Definitions, DataError> {
    load_with_units(&units_fixture(), path)
}

fn load_with_units(units: &Path, path: &Path) -> Result<Definitions, DataError> {
    let (units, owned) = (units.to_owned(), path.to_owned());
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name(format!("load {}", path.display()))
        .spawn(move || {
            let _ = sender.send(load_definitions(&units, &owned));
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

fn units_fixture() -> PathBuf {
    fixtures().join("units_economy.ron")
}

fn fixture() -> PathBuf {
    fixtures().join("economy.ron")
}

fn fixture_text() -> String {
    std::fs::read_to_string(fixture()).expect("fixture exists")
}

fn temp_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("m3_definitions");
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

// --- Positive -------------------------------------------------------------------------------

#[test]
fn the_fixture_loads_with_every_field() {
    let definitions = load_ok(&fixture());
    assert_eq!(definitions.placement_distance(), 3);
    let ids: Vec<&str> = definitions
        .buildings()
        .iter()
        .map(|def| def.id.as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "yard", "power", "refinery", "barracks", "factory", "fence", "beacon", "flag"
        ]
    );
    let kind = |id| definitions.building_kind(id).expect("building");
    let refinery = definitions.building(kind("refinery")).expect("refinery");
    assert_eq!(
        (refinery.width, refinery.height, refinery.sprite.as_str()),
        (3, 2, "refinery")
    );
    assert_eq!(
        refinery.production,
        Production {
            cost: 200,
            build_time: 20,
            producer: kind("yard"),
            prerequisites: vec![kind("power")],
        }
    );
    assert_eq!(
        (
            refinery.power_produced,
            refinery.power_consumed,
            refinery.sell_value
        ),
        (0, 30, 100)
    );
    assert_eq!(refinery.free_units, [UnitKind(2)]);
    assert_eq!(refinery.credits_per_ore, Some(2));
    let power = definitions.building(kind("power")).expect("power");
    assert_eq!(power.credits_per_ore, None);
    assert_eq!(
        definitions.production(ProductionItem::Unit(UnitKind(0))),
        Some(&Production {
            cost: 50,
            build_time: 7,
            producer: kind("barracks"),
            prerequisites: vec![],
        })
    );
    assert_eq!(
        definitions.production(ProductionItem::Unit(UnitKind(1))),
        None,
        "the runner is not producible"
    );
    assert_eq!(
        definitions.harvester(UnitKind(2)),
        Some(HarvesterDef {
            capacity: 20,
            harvest_rate: 3,
        })
    );
    assert_eq!(definitions.harvester(UnitKind(0)), None);
    assert!(definitions.produces_units(kind("barracks")));
    assert!(definitions.produces_units(kind("factory")));
    assert!(!definitions.produces_units(kind("yard")));
    assert_eq!(definitions.building(BuildingKind(8)), None);
    // The unit definitions are those of the units file.
    assert_eq!(definitions.units().len(), 3);
}

// --- Negative: structure --------------------------------------------------------------------

#[test]
fn every_field_is_required_and_unknown_fields_are_rejected() {
    // (the removed text, which must be unique, and the field reported missing)
    let missing = [
        ("    placement_distance: 3,\n", "placement_distance"),
        ("            id: \"yard\",\n", "buildings[0].id"),
        ("            sprite: \"yard\",\n", "buildings[0].sprite"),
        (
            "            width: 3,\n            height: 3,\n            cost: 1000,\n",
            "buildings[0].width",
        ),
        ("            cost: 1000,\n", "buildings[0].cost"),
        ("            build_time: 100,\n", "buildings[0].build_time"),
        (
            "            producer: \"yard\",\n            prerequisites: [],\n            power_produced: 10,\n",
            "buildings[0].producer",
        ),
        (
            "            power_produced: 10,\n",
            "buildings[0].power_produced",
        ),
        ("            sell_value: 500,\n", "buildings[0].sell_value"),
        ("            unit: \"walker\",\n", "units[0].unit"),
        ("            capacity: 20,\n", "harvesters[0].capacity"),
        (
            "            harvest_rate: 3,\n",
            "harvesters[0].harvest_rate",
        ),
        (
            "            credits_per_ore: 2,\n",
            "refineries[0].credits_per_ore",
        ),
    ];
    for (index, (removed, field)) in missing.into_iter().enumerate() {
        // Only the first line of `removed` is deleted; the rest makes the edit unique.
        let kept = removed.split_once('\n').map_or("", |(_, rest)| rest);
        let path = variant(&format!("missing_{index}"), &[(removed, kept)]);
        assert_rejected(&path, field, DataErrorKind::MissingField);
    }
    for (name, from, to, field) in [
        (
            "unknown_building_field",
            "            sell_value: 500,\n",
            "            sell_value: 500,\n            armour: 1,\n",
            "buildings[0].armour",
        ),
        (
            "unknown_top_field",
            "    placement_distance: 3,\n",
            "    placement_distance: 3,\n    weapons: [],\n",
            "weapons",
        ),
        (
            "unknown_refinery_field",
            "            credits_per_ore: 2,\n",
            "            credits_per_ore: 2,\n            dock: 1,\n",
            "refineries[0].dock",
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
                "            sell_value: 500,\n",
                "            sell_value: 500,\n            sell_value: 5,\n",
            )],
        ),
        "buildings[0].sell_value",
        DataErrorKind::DuplicateField,
    );
    // Values of the wrong kind are syntax errors at their field.
    for (name, from, to, field) in [
        (
            "width_string",
            "width: 3,\n            height: 3,\n            cost: 1000,",
            "width: \"3\",\n            height: 3,\n            cost: 1000,",
            "buildings[0].width",
        ),
        (
            "prerequisites_string",
            "prerequisites: [\"power\"],\n            power_produced: 0,\n            power_consumed: 30,",
            "prerequisites: \"power\",\n            power_produced: 0,\n            power_consumed: 30,",
            "buildings[2].prerequisites",
        ),
        (
            "free_units_numbers",
            "free_units: [\"harvester\"],",
            "free_units: [1],",
            "buildings[2].free_units[0]",
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
    for version in [0, 2, -1, i64::MAX] {
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
    let cases: [(&str, &str, &str, i64, i64); 12] = [
        (
            "placement_distance: 3,",
            "placement_distance",
            "placement_distance: {},",
            1,
            i64::from(MAX_PLACEMENT_DISTANCE),
        ),
        (
            "width: 2,\n            height: 2,\n            cost: 105,",
            "buildings[1].width",
            "width: {},\n            height: 2,\n            cost: 105,",
            1,
            i64::from(MAX_FOOTPRINT_SIDE),
        ),
        (
            "height: 2,\n            cost: 105,",
            "buildings[1].height",
            "height: {},\n            cost: 105,",
            1,
            i64::from(MAX_FOOTPRINT_SIDE),
        ),
        (
            "build_time: 10,",
            "buildings[1].build_time",
            "build_time: {},",
            1,
            i64::from(MAX_BUILD_TIME),
        ),
        (
            "power_produced: 100,",
            "buildings[1].power_produced",
            "power_produced: {},",
            0,
            i64::from(MAX_POWER),
        ),
        (
            "power_consumed: 30,",
            "buildings[2].power_consumed",
            "power_consumed: {},",
            0,
            i64::from(MAX_POWER),
        ),
        (
            "cost: 7,\n            build_time: 1,",
            "buildings[6].cost",
            "cost: {},\n            build_time: 1,",
            0,
            MAX_COST,
        ),
        (
            "build_time: 1,",
            "buildings[6].build_time",
            "build_time: {},",
            1,
            i64::from(MAX_BUILD_TIME),
        ),
        ("cost: 50,", "units[0].cost", "cost: {},", 0, MAX_COST),
        (
            "build_time: 7,",
            "units[0].build_time",
            "build_time: {},",
            1,
            i64::from(MAX_BUILD_TIME),
        ),
        (
            "credits_per_ore: 2,",
            "refineries[0].credits_per_ore",
            "credits_per_ore: {},",
            1,
            MAX_CREDITS_PER_ORE,
        ),
        // The sell value of the power plant may equal its cost (105) but not exceed it.
        (
            "sell_value: 50,",
            "buildings[1].sell_value",
            "sell_value: {},",
            0,
            105,
        ),
    ];
    for (from, field, template, min, max) in cases {
        let with = |value: i64| template.replace("{}", &value.to_string());
        let name = field.replace(['[', ']', '.'], "_");
        load_ok(&variant(&format!("{name}_min"), &[(from, &with(min))]));
        load_ok(&variant(&format!("{name}_max"), &[(from, &with(max))]));
        for value in [min - 1, max + 1, i64::MIN, i64::MAX] {
            let path = variant(&format!("{name}_{value}"), &[(from, &with(value))]);
            assert_out_of_range(&path, field, &value.to_string());
        }
    }
    // The capacity bounds the harvest rate: capacity 1 needs rate 1; the rate may equal the
    // capacity but not exceed it.
    let harvester = |capacity: i64, rate: i64| {
        variant(
            &format!("harvester_{capacity}_{rate}"),
            &[(
                "capacity: 20,\n            harvest_rate: 3,",
                &format!("capacity: {capacity},\n            harvest_rate: {rate},"),
            )],
        )
    };
    load_ok(&harvester(1, 1));
    load_ok(&harvester(20, 20));
    let most = i64::from(MAX_ORE_CAPACITY);
    load_ok(&harvester(most, most));
    assert_out_of_range(&harvester(0, 1), "harvesters[0].capacity", "0");
    assert_out_of_range(
        &harvester(most + 1, 1),
        "harvesters[0].capacity",
        &(most + 1).to_string(),
    );
    assert_out_of_range(&harvester(20, 21), "harvesters[0].harvest_rate", "21");
    assert_out_of_range(&harvester(20, 0), "harvesters[0].harvest_rate", "0");
    // An integer outside i64 is rejected by the checker at its field.
    let path = variant(
        "cost_beyond_i64",
        &[("cost: 50,", "cost: 99999999999999999999,")],
    );
    assert_out_of_range(&path, "units[0].cost", "99999999999999999999");
}

#[test]
fn list_lengths_are_bounded() {
    // A file with `count` 1x1 buildings b0, b1, ...; the last lists `prerequisites` of the
    // first ones and `free_units` walkers.
    let generated = |name: &str, count: usize, prerequisites: usize, free_units: usize| {
        let mut entries = Vec::new();
        for index in 0..count {
            let last = index + 1 == count;
            let prerequisites: Vec<String> = if last {
                (0..prerequisites).map(|p| format!("\"b{p}\"")).collect()
            } else {
                Vec::new()
            };
            let free: Vec<&str> = if last {
                vec!["\"walker\""; free_units]
            } else {
                Vec::new()
            };
            entries.push(format!(
                "(id: \"b{index}\", sprite: \"s\", width: 1, height: 1, cost: 1, build_time: 1, \
                 producer: \"b0\", prerequisites: [{}], power_produced: 0, power_consumed: 0, \
                 sell_value: 0, free_units: [{}])",
                prerequisites.join(", "),
                free.join(", ")
            ));
        }
        let text = format!(
            "(schema_version: 1, placement_distance: 1, buildings: [{}], units: [], \
             harvesters: [], refineries: [])",
            entries.join(",\n")
        );
        temp_file(&format!("{name}.ron"), text.as_bytes())
    };
    let definitions = load_ok(&generated(
        "most",
        MAX_BUILDING_TYPES,
        MAX_PREREQUISITES,
        MAX_FREE_UNITS,
    ));
    assert_eq!(definitions.buildings().len(), MAX_BUILDING_TYPES);
    let last = definitions.buildings().last().expect("last");
    assert_eq!(last.production.prerequisites.len(), MAX_PREREQUISITES);
    assert_eq!(last.free_units.len(), MAX_FREE_UNITS);
    assert_rejected(
        &generated("too_many_buildings", MAX_BUILDING_TYPES + 1, 0, 0),
        "buildings",
        DataErrorKind::TooManyEntries {
            max: MAX_BUILDING_TYPES,
        },
    );
    assert_rejected(
        &generated("too_many_prerequisites", 12, MAX_PREREQUISITES + 1, 0),
        "buildings[11].prerequisites",
        DataErrorKind::TooManyEntries {
            max: MAX_PREREQUISITES,
        },
    );
    assert_rejected(
        &generated("too_many_free_units", 3, 0, MAX_FREE_UNITS + 1),
        "buildings[2].free_units",
        DataErrorKind::TooManyEntries {
            max: MAX_FREE_UNITS,
        },
    );
    // At least one building type.
    assert_rejected(
        &temp_file(
            "no_buildings.ron",
            b"(schema_version: 1, placement_distance: 1, buildings: [], units: [], \
              harvesters: [], refineries: [])",
        ),
        "buildings",
        DataErrorKind::InvalidValue {
            found: "an empty list".into(),
            expected: format!("1 to {MAX_BUILDING_TYPES} building types"),
        },
    );
}

// --- Negative: references, names, uniqueness, cycles ---------------------------------------

#[test]
fn references_must_name_defined_types() {
    let cases = [
        (
            "producer: \"yard\",\n            prerequisites: [],\n            power_produced: 10,",
            "producer: \"nowhere\",\n            prerequisites: [],\n            power_produced: 10,",
            "buildings[0].producer",
            unknown("building", "nowhere"),
        ),
        (
            "prerequisites: [\"refinery\"],\n            power_produced: 0,\n            power_consumed: 40,",
            "prerequisites: [\"refinery\", \"silo\"],\n            power_produced: 0,\n            power_consumed: 40,",
            "buildings[4].prerequisites[1]",
            unknown("building", "silo"),
        ),
        (
            "free_units: [\"harvester\"],",
            "free_units: [\"tank\"],",
            "buildings[2].free_units[0]",
            unknown("unit", "tank"),
        ),
        (
            "unit: \"walker\",",
            "unit: \"tank\",",
            "units[0].unit",
            unknown("unit", "tank"),
        ),
        (
            "producer: \"barracks\",",
            "producer: \"walker\",",
            "units[0].producer",
            unknown("building", "walker"),
        ),
        (
            "prerequisites: [\"refinery\"],\n        ),\n    ],\n    harvesters",
            "prerequisites: [\"refinery\", \"harvester\"],\n        ),\n    ],\n    harvesters",
            "units[1].prerequisites[1]",
            unknown("building", "harvester"),
        ),
        (
            "unit: \"harvester\",\n            capacity",
            "unit: \"runner_x\",\n            capacity",
            "harvesters[0].unit",
            unknown("unit", "runner_x"),
        ),
        (
            "building: \"refinery\",",
            "building: \"walker\",",
            "refineries[0].building",
            unknown("building", "walker"),
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
fn names_and_identifiers_are_checked() {
    let bad_name = |found: &str| DataErrorKind::InvalidValue {
        found: format!("{found:?}"),
        expected: "1 to 32 characters from a-z, 0-9, and _, starting with a letter".into(),
    };
    assert_rejected(
        &variant("upper_id", &[("id: \"yard\",", "id: \"Yard\",")]),
        "buildings[0].id",
        bad_name("Yard"),
    );
    assert_rejected(
        &variant(
            "bad_sprite",
            &[("sprite: \"power\",", "sprite: \"../power\",")],
        ),
        "buildings[1].sprite",
        bad_name("../power"),
    );
    assert_rejected(
        &variant("duplicate_id", &[("id: \"flag\",", "id: \"beacon\",")]),
        "buildings[7].id",
        DataErrorKind::DuplicateId("beacon".into()),
    );
    assert_rejected(
        &variant(
            "duplicate_prerequisite",
            &[(
                "prerequisites: [\"refinery\"],\n            power_produced: 0,\n            power_consumed: 40,",
                "prerequisites: [\"refinery\", \"refinery\"],\n            power_produced: 0,\n            power_consumed: 40,",
            )],
        ),
        "buildings[4].prerequisites[1]",
        DataErrorKind::DuplicateId("refinery".into()),
    );
    assert_rejected(
        &variant(
            "duplicate_production",
            &[(
                "unit: \"harvester\",\n            cost: 300,",
                "unit: \"walker\",\n            cost: 300,",
            )],
        ),
        "units[1].unit",
        DataErrorKind::DuplicateId("walker".into()),
    );
    assert_rejected(
        &variant(
            "duplicate_harvester",
            &[(
                "            harvest_rate: 3,\n        ),\n",
                "            harvest_rate: 3,\n        ),\n        (unit: \"harvester\", capacity: 5, harvest_rate: 1),\n",
            )],
        ),
        "harvesters[1].unit",
        DataErrorKind::DuplicateId("harvester".into()),
    );
    assert_rejected(
        &variant(
            "duplicate_refinery",
            &[(
                "            credits_per_ore: 2,\n        ),\n",
                "            credits_per_ore: 2,\n        ),\n        (building: \"refinery\", credits_per_ore: 3),\n",
            )],
        ),
        "refineries[1].building",
        DataErrorKind::DuplicateId("refinery".into()),
    );
}

#[test]
fn prerequisite_cycles_are_rejected() {
    let cycle = |ids: &[&str]| {
        DataErrorKind::PrerequisiteCycle(ids.iter().map(|id| id.to_string()).collect())
    };
    let power_requires = |name: &str, prerequisite: &str| {
        variant(
            name,
            &[(
                "prerequisites: [],\n            power_produced: 100,",
                &format!("prerequisites: [\"{prerequisite}\"],\n            power_produced: 100,"),
            )],
        )
    };
    // The power plant (buildings[1]) requires itself.
    assert_rejected(
        &power_requires("self_cycle", "power"),
        "buildings[1].prerequisites",
        cycle(&["power", "power"]),
    );
    // Power plant -> barracks -> power plant.
    assert_rejected(
        &power_requires("two_cycle", "barracks"),
        "buildings[3].prerequisites",
        cycle(&["power", "barracks", "power"]),
    );
    // Power plant -> factory -> refinery -> power plant.
    assert_rejected(
        &power_requires("three_cycle", "factory"),
        "buildings[2].prerequisites",
        cycle(&["power", "factory", "refinery", "power"]),
    );
    // A diamond (two paths to the same prerequisite) is not a cycle.
    load_ok(&variant(
        "diamond",
        &[(
            "prerequisites: [\"refinery\"],\n            power_produced: 0,\n            power_consumed: 40,",
            "prerequisites: [\"refinery\", \"power\"],\n            power_produced: 0,\n            power_consumed: 40,",
        )],
    ));
}

// --- Negative: files ------------------------------------------------------------------------

#[test]
fn unreadable_oversized_and_special_files_are_rejected() {
    let missing = temp_dir().join("does_not_exist.ron");
    let error = load(&missing).expect_err("missing");
    assert!(matches!(error.kind, DataErrorKind::Read(_)), "{error}");
    assert_eq!(
        (error.file.as_path(), error.field.as_str()),
        (missing.as_path(), "file")
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
    // An error in the units file is reported in the units file, before the economy file is read.
    let units = temp_file("bad_units.ron", b"(schema_version: 1, units: [])");
    let error = load_with_units(&units, &fixture()).expect_err("no units");
    assert_eq!((error.file, error.field.as_str()), (units, "units"));
}

#[test]
fn truncations_of_the_fixture_are_rejected_without_panic() {
    // Every seventh prefix and the last one before the closing parenthesis.
    let text = fixture_text();
    let end = text.rfind(')').expect("closing parenthesis");
    for cut in (0..=end).step_by(7).chain([end]) {
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
    "\"power\"",
    "\"yard\", ",
    "\"harvester\"",
    "prerequisites: [\"barracks\"],",
    "cost",
    "width: 9,",
    "{",
    "1.5",
    "\n",
    "\u{e9}",
];

#[test]
fn seeded_mutations_of_the_fixture_load_or_return_a_typed_error() {
    let base = fixture_text().into_bytes();
    let mut rng = SplitMix64(0x0e00_d0c5);
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
                let buildings = definitions.buildings();
                assert!(
                    (1..=MAX_BUILDING_TYPES).contains(&buildings.len()),
                    "case {case}"
                );
                for def in buildings {
                    assert!((1..=MAX_FOOTPRINT_SIDE).contains(&def.width), "case {case}");
                    assert!(
                        (1..=MAX_FOOTPRINT_SIDE).contains(&def.height),
                        "case {case}"
                    );
                    assert!((0..=MAX_COST).contains(&def.production.cost), "case {case}");
                    assert!((1..=MAX_BUILD_TIME).contains(&def.production.build_time));
                    assert!((0..=def.production.cost).contains(&def.sell_value));
                    assert!(usize::from(def.production.producer.0) < buildings.len());
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
