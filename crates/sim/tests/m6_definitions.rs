//! M6 tests: the skirmish file and the AI file (PLAN.md sections 5 and 7, M6; invariants I8, I10,
//! I11; docs/DECISIONS.md D18, D40, D42).
//!
//! Both files are checked against the supported subset of RON and their schemas before `ron` sees
//! them, then validated: ranges at both ends, references to unit and building types, uniqueness,
//! and the cross checks of the AI file (prerequisites come earlier in the build order, the power
//! building produces power, the harvester harvests, the army is armed and can damage every
//! building). Every rejection is a typed `DataError` naming the file, the field, and the reason.
//! The tests are bounded as in D15: this test binary's global allocator refuses allocations beyond
//! [`MEMORY_BUDGET`], and every load runs on a worker thread whose result must arrive within
//! [`TIME_LIMIT`].
//!
//! Variants are derived from the committed fixtures `tests/fixtures/data/skirmish.ron` and
//! `tests/fixtures/data/ai.ron` by exact text edits and written to Cargo's temporary directory for
//! integration tests, under names unique to each test.

use std::alloc::{GlobalAlloc, Layout, System};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use sim::{
    AiDefinitions, BuildingKind, CellPos, DataError, DataErrorKind, Definitions, MAX_AI_ARMY,
    MAX_BUILD_ORDER, MAX_DATA_FILE_BYTES, MAX_DEFENCE_RADIUS, MAX_GROUP_UNITS,
    MAX_HARVESTERS_PER_REFINERY, MAX_START_OFFSET, MAX_STARTING_CREDITS, MAX_THINK_INTERVAL,
    MapDef, SkirmishDefinitions, StartingBuilding, StartingGroup, UnitKind, check_ai_factions,
    check_map_starts, load_ai, load_map, load_match_definitions, load_skirmish, map_asset_path,
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

// --- Files ----------------------------------------------------------------------------------

/// Which of the two files a test loads.
#[derive(Clone, Copy, Debug)]
enum File {
    Skirmish,
    Ai,
}

impl File {
    fn fixture(self) -> PathBuf {
        fixtures().join(match self {
            File::Skirmish => "skirmish.ron",
            File::Ai => "ai.ron",
        })
    }

    fn text(self) -> String {
        std::fs::read_to_string(self.fixture()).expect("fixture exists")
    }
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data")
}

fn definitions() -> Definitions {
    load_match_definitions(
        fixtures().join("units_combat.ron"),
        fixtures().join("economy_combat.ron"),
        fixtures().join("combat.ron"),
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

/// What a load returned: the skirmish or the AI definitions.
#[derive(Debug)]
enum Loaded {
    Skirmish(SkirmishDefinitions),
    Ai(AiDefinitions),
}

/// Loads `path` as `file` against `definitions` on a worker thread, failing the test if the loader
/// panics or takes longer than [`TIME_LIMIT`].
fn load_with(file: File, path: &Path, definitions: Definitions) -> Result<Loaded, DataError> {
    let owned = path.to_owned();
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name(format!("load {}", path.display()))
        .spawn(move || {
            let result = match file {
                File::Skirmish => load_skirmish(&owned, &definitions).map(Loaded::Skirmish),
                File::Ai => load_ai(&owned, &definitions).map(Loaded::Ai),
            };
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

fn load(file: File, path: &Path) -> Result<Loaded, DataError> {
    load_with(file, path, definitions())
}

fn temp_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("m6_definitions");
    std::fs::create_dir_all(&dir).expect("temporary directory");
    dir
}

fn temp_file(name: &str, contents: &[u8]) -> PathBuf {
    let path = temp_dir().join(name);
    std::fs::write(&path, contents).expect("temporary file");
    path
}

/// The fixture of `file` with each `(from, to)` edit applied; `from` must occur exactly once.
/// `name` must be unique among all tests, since the tests run in parallel.
fn variant(file: File, name: &str, edits: &[(&str, &str)]) -> PathBuf {
    let mut text = file.text();
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

/// Asserts that `path` is rejected with `kind` at `field`, and that the message names the file,
/// the field, and the reason.
fn assert_rejected(file: File, path: &Path, field: &str, kind: DataErrorKind) {
    let error = load(file, path).expect_err("the file must be rejected");
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

/// Asserts that `path` is rejected at `field` because the value `found` is not supported.
fn assert_invalid(file: File, path: &Path, field: &str, found: &str) {
    let error = load(file, path).expect_err("the file must be rejected");
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
fn the_skirmish_fixture_loads_with_every_field() {
    let Ok(Loaded::Skirmish(skirmish)) = load(File::Skirmish, &File::Skirmish.fixture()) else {
        panic!("the fixture loads");
    };
    let definitions = definitions();
    let building = |id| definitions.building_kind(id).expect("building");
    let unit = |id| definitions.unit_kind(id).expect("unit");
    assert_eq!(skirmish.starting_credits, 1500);
    assert_eq!(skirmish.factions.len(), 2);
    assert_eq!(skirmish.factions[0].id, "red");
    assert_eq!(
        skirmish.factions[0].buildings,
        [StartingBuilding {
            kind: building("yard"),
            offset: (0, 0),
        }]
    );
    assert_eq!(
        skirmish.factions[0].units,
        [
            StartingGroup {
                kind: unit("tank"),
                offset: (4, 1),
                count: 2,
            },
            StartingGroup {
                kind: unit("rifleman"),
                offset: (1, 4),
                count: 2,
            },
        ]
    );
    assert_eq!(
        skirmish.factions[1].buildings[1],
        StartingBuilding {
            kind: building("turret"),
            offset: (-2, 1),
        }
    );
    assert_eq!(
        skirmish.maps,
        [MapDef {
            id: "skirmish_40x40".into(),
            starts: vec![CellPos::new(5, 5), CellPos::new(32, 32)],
        }]
    );
    assert_eq!(
        map_asset_path("demo"),
        PathBuf::from("maps").join("demo").join("demo.tmx")
    );
}

#[test]
fn the_ai_fixture_loads_with_every_field() {
    let Ok(Loaded::Ai(ai)) = load(File::Ai, &File::Ai.fixture()) else {
        panic!("the fixture loads");
    };
    let definitions = definitions();
    let building = |id| definitions.building_kind(id).expect("building");
    let unit = |id| definitions.unit_kind(id).expect("unit");
    assert_eq!(
        ai.build_order,
        [
            "power", "refinery", "barracks", "factory", "power", "turret"
        ]
        .map(building)
    );
    assert_eq!(ai.power, building("power"));
    assert_eq!(ai.harvester, unit("harvester"));
    assert_eq!(ai.army, ["tank", "tank", "rifleman"].map(unit));
    assert_eq!(ai.difficulties.len(), 2);
    let normal = &ai.difficulties[1];
    assert_eq!(normal.id, "normal");
    assert_eq!(
        (
            normal.think_interval,
            normal.harvesters_per_refinery,
            normal.max_army,
            normal.wave_size,
            normal.first_wave,
            normal.wave_patience,
            normal.defence_radius,
        ),
        (10, 2, 12, 5, 600, 800, 8)
    );
}

#[test]
fn limits_are_accepted_at_both_ends() {
    let skirmish = variant(
        File::Skirmish,
        "skirmish_limits",
        &[
            ("starting_credits: 1500", "starting_credits: 1000000"),
            (
                "(unit: \"tank\", x: 4, y: 1, count: 2)",
                "(unit: \"tank\", x: -32, y: 32, count: 64)",
            ),
            (
                "(unit: \"rifleman\", x: 1, y: 4, count: 2)",
                "(unit: \"rifleman\", x: 32, y: -32, count: 1)",
            ),
            (
                "(x: 5, y: 5), (x: 32, y: 32)",
                "(x: 0, y: 0), (x: 255, y: 255)",
            ),
        ],
    );
    let Ok(Loaded::Skirmish(loaded)) = load(File::Skirmish, &skirmish) else {
        panic!("limits load");
    };
    assert_eq!(loaded.starting_credits, MAX_STARTING_CREDITS);
    assert_eq!(
        loaded.factions[0].units[0].offset,
        (-MAX_START_OFFSET, MAX_START_OFFSET)
    );
    assert_eq!(loaded.factions[0].units[0].count, MAX_GROUP_UNITS);
    assert_eq!(loaded.maps[0].starts[1], CellPos::new(255, 255));
    let zero = variant(
        File::Skirmish,
        "skirmish_zero_credits",
        &[
            ("starting_credits: 1500", "starting_credits: 0"),
            (
                "            units: [(unit: \"rifleman\", x: 4, y: 1, count: 3)],",
                "            units: [],",
            ),
        ],
    );
    assert!(matches!(
        load(File::Skirmish, &zero),
        Ok(Loaded::Skirmish(_))
    ));

    let ai = variant(
        File::Ai,
        "ai_limits",
        &[
            ("think_interval: 20", "think_interval: 1"),
            ("think_interval: 10", "think_interval: 1200"),
            ("harvesters_per_refinery: 1", "harvesters_per_refinery: 0"),
            ("harvesters_per_refinery: 2", "harvesters_per_refinery: 8"),
            ("max_army: 6", "max_army: 1"),
            ("wave_size: 4", "wave_size: 1"),
            ("max_army: 12", "max_army: 200"),
            ("wave_size: 5", "wave_size: 200"),
            ("first_wave: 1200", "first_wave: 0"),
            ("first_wave: 600", "first_wave: 72000"),
            ("wave_patience: 1200", "wave_patience: 1"),
            ("wave_patience: 800", "wave_patience: 72000"),
            ("defence_radius: 6", "defence_radius: 0"),
            ("defence_radius: 8", "defence_radius: 32"),
        ],
    );
    let Ok(Loaded::Ai(loaded)) = load(File::Ai, &ai) else {
        panic!("limits load");
    };
    let (easy, normal) = (&loaded.difficulties[0], &loaded.difficulties[1]);
    assert_eq!(
        (easy.think_interval, normal.think_interval),
        (1, MAX_THINK_INTERVAL)
    );
    assert_eq!(normal.harvesters_per_refinery, MAX_HARVESTERS_PER_REFINERY);
    assert_eq!((easy.max_army, easy.wave_size), (1, 1));
    assert_eq!(
        (normal.max_army, normal.wave_size),
        (MAX_AI_ARMY, MAX_AI_ARMY)
    );
    assert_eq!(normal.defence_radius, MAX_DEFENCE_RADIUS);
    // The longest build order.
    // The army's and the harvester's producers and prerequisites must be in the build order
    // (M6 review 1 P2-1), so the longest one lists them before its turrets.
    let long = format!(
        "build_order: [\"power\", \"refinery\", \"barracks\", \"factory\", {}],",
        vec!["\"turret\""; MAX_BUILD_ORDER - 4].join(", ")
    );
    let path = variant(
        File::Ai,
        "ai_long_build_order",
        &[(
            "build_order: [\"power\", \"refinery\", \"barracks\", \"factory\", \"power\", \"turret\"],",
            &long,
        )],
    );
    let Ok(Loaded::Ai(loaded)) = load(File::Ai, &path) else {
        panic!("the longest build order loads");
    };
    assert_eq!(loaded.build_order.len(), MAX_BUILD_ORDER);
}

// --- Negative: values -----------------------------------------------------------------------

#[test]
fn skirmish_values_outside_their_ranges_are_rejected() {
    for (index, (from, to, field, found)) in [
        (
            "starting_credits: 1500",
            "starting_credits: -1",
            "starting_credits",
            "-1",
        ),
        (
            "starting_credits: 1500",
            "starting_credits: 1000001",
            "starting_credits",
            "1000001",
        ),
        (
            "(unit: \"tank\", x: 4, y: 1, count: 2)",
            "(unit: \"tank\", x: -33, y: 1, count: 2)",
            "factions[0].units[0].x",
            "-33",
        ),
        (
            "(unit: \"tank\", x: 4, y: 1, count: 2)",
            "(unit: \"tank\", x: 4, y: 33, count: 2)",
            "factions[0].units[0].y",
            "33",
        ),
        (
            "(unit: \"tank\", x: 4, y: 1, count: 2)",
            "(unit: \"tank\", x: 4, y: 1, count: 0)",
            "factions[0].units[0].count",
            "0",
        ),
        (
            "(unit: \"tank\", x: 4, y: 1, count: 2)",
            "(unit: \"tank\", x: 4, y: 1, count: 65)",
            "factions[0].units[0].count",
            "65",
        ),
        (
            "(building: \"turret\", x: -2, y: 1)",
            "(building: \"turret\", x: 33, y: 1)",
            "factions[1].buildings[1].x",
            "33",
        ),
        (
            "(x: 5, y: 5), (x: 32, y: 32)",
            "(x: 5, y: 5), (x: 256, y: 32)",
            "maps[0].starts[1].x",
            "256",
        ),
        (
            "(x: 5, y: 5), (x: 32, y: 32)",
            "(x: -1, y: 5), (x: 32, y: 32)",
            "maps[0].starts[0].x",
            "-1",
        ),
        (
            "(x: 5, y: 5), (x: 32, y: 32)",
            "(x: 5, y: 5)",
            "maps[0].starts",
            "1 start positions",
        ),
        (
            "(x: 5, y: 5), (x: 32, y: 32)",
            "(x: 5, y: 5), (x: 5, y: 5)",
            "maps[0].starts[1]",
            "(5, 5), the cell of an earlier start",
        ),
        (
            "            buildings: [(building: \"yard\", x: 0, y: 0)],",
            "            buildings: [],",
            "factions[0].buildings",
            "an empty list",
        ),
        (
            "            id: \"red\",",
            "            id: \"Red\",",
            "factions[0].id",
            "\"Red\"",
        ),
        (
            "(id: \"skirmish_40x40\",",
            "(id: \"../demo\",",
            "maps[0].id",
            "\"../demo\"",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let path = variant(
            File::Skirmish,
            &format!("skirmish_range_{index}"),
            &[(from, to)],
        );
        assert_invalid(File::Skirmish, &path, field, found);
    }
    // Lists that must not be empty.
    let text = File::Skirmish.text();
    let factions_start = text.find("    factions: [").expect("factions");
    let maps_start = text.find("    maps: [").expect("maps");
    let no_factions = format!(
        "{}    factions: [],\n{}",
        &text[..factions_start],
        &text[maps_start..]
    );
    let path = temp_file("skirmish_no_factions.ron", no_factions.as_bytes());
    assert_invalid(File::Skirmish, &path, "factions", "an empty list");
    let end = text.rfind(')').expect("end");
    let no_maps = format!("{}    maps: [],\n{}", &text[..maps_start], &text[end..]);
    let path = temp_file("skirmish_no_maps.ron", no_maps.as_bytes());
    assert_invalid(File::Skirmish, &path, "maps", "an empty list");
    // A map may have more starts, for matches with several computer players (up to
    // `MAX_MATCH_PLAYERS`); one more exceeds the schema's list bound.
    let path = variant(
        File::Skirmish,
        "skirmish_three_starts",
        &[(
            "(x: 5, y: 5), (x: 32, y: 32)",
            "(x: 5, y: 5), (x: 32, y: 32), (x: 20, y: 20)",
        )],
    );
    load(File::Skirmish, &path).unwrap_or_else(|error| panic!("{error}"));
    let starts = |count: usize| {
        (0..count)
            .map(|k| format!("(x: {}, y: {})", 2 + 4 * (k % 8), 5 + 4 * (k / 8)))
            .collect::<Vec<_>>()
            .join(", ")
    };
    assert_eq!(sim::MAX_MATCH_PLAYERS, 10);
    let path = variant(
        File::Skirmish,
        "skirmish_ten_starts",
        &[("(x: 5, y: 5), (x: 32, y: 32)", starts(10).as_str())],
    );
    load(File::Skirmish, &path).unwrap_or_else(|error| panic!("{error}"));
    let path = variant(
        File::Skirmish,
        "skirmish_eleven_starts",
        &[("(x: 5, y: 5), (x: 32, y: 32)", starts(11).as_str())],
    );
    assert_rejected(
        File::Skirmish,
        &path,
        "maps[0].starts",
        DataErrorKind::TooManyEntries {
            max: sim::MAX_MATCH_PLAYERS,
        },
    );
}

#[test]
fn skirmish_references_and_ids_are_checked() {
    let cases: [(&str, &str, &str, DataErrorKind); 6] = [
        (
            "(building: \"yard\", x: 0, y: 0)],",
            "(building: \"castle\", x: 0, y: 0)],",
            "factions[0].buildings[0].building",
            unknown("building", "castle"),
        ),
        (
            "(unit: \"tank\", x: 4, y: 1, count: 2)",
            "(unit: \"dragon\", x: 4, y: 1, count: 2)",
            "factions[0].units[0].unit",
            unknown("unit", "dragon"),
        ),
        (
            "            id: \"blue\",",
            "            id: \"red\",",
            "factions[1].id",
            DataErrorKind::DuplicateId("red".into()),
        ),
        (
            "        (id: \"skirmish_40x40\", starts: [(x: 5, y: 5), (x: 32, y: 32)]),",
            "        (id: \"skirmish_40x40\", starts: [(x: 5, y: 5), (x: 32, y: 32)]),\n        (id: \"skirmish_40x40\", starts: [(x: 1, y: 1), (x: 2, y: 2)]),",
            "maps[1].id",
            DataErrorKind::DuplicateId("skirmish_40x40".into()),
        ),
        // Schema version 2 since M7 (faction rosters, docs/DECISIONS.md D48): a file of version
        // 1 (M6, without rosters) and of any later version is rejected as such.
        (
            "schema_version: 2,",
            "schema_version: 3,",
            "schema_version",
            DataErrorKind::UnsupportedSchemaVersion {
                found: 3,
                expected: 2,
            },
        ),
        (
            "schema_version: 2,",
            "schema_version: 1,",
            "schema_version",
            DataErrorKind::UnsupportedSchemaVersion {
                found: 1,
                expected: 2,
            },
        ),
    ];
    for (index, (from, to, field, kind)) in cases.into_iter().enumerate() {
        let path = variant(
            File::Skirmish,
            &format!("skirmish_reference_{index}"),
            &[(from, to)],
        );
        assert_rejected(File::Skirmish, &path, field, kind);
    }
}

#[test]
fn ai_values_outside_their_ranges_are_rejected() {
    for (index, (from, to, field, found)) in [
        ("think_interval: 20", "think_interval: 0", "difficulties[0].think_interval", "0"),
        ("think_interval: 20", "think_interval: 1201", "difficulties[0].think_interval", "1201"),
        (
            "harvesters_per_refinery: 1",
            "harvesters_per_refinery: -1",
            "difficulties[0].harvesters_per_refinery",
            "-1",
        ),
        (
            "harvesters_per_refinery: 1",
            "harvesters_per_refinery: 9",
            "difficulties[0].harvesters_per_refinery",
            "9",
        ),
        ("max_army: 6", "max_army: 0", "difficulties[0].max_army", "0"),
        ("max_army: 6", "max_army: 201", "difficulties[0].max_army", "201"),
        ("wave_size: 4", "wave_size: 0", "difficulties[0].wave_size", "0"),
        ("wave_size: 4", "wave_size: 7", "difficulties[0].wave_size", "7"),
        ("first_wave: 1200", "first_wave: -1", "difficulties[0].first_wave", "-1"),
        ("first_wave: 1200", "first_wave: 72001", "difficulties[0].first_wave", "72001"),
        ("wave_patience: 1200", "wave_patience: 0", "difficulties[0].wave_patience", "0"),
        (
            "wave_patience: 1200",
            "wave_patience: 72001",
            "difficulties[0].wave_patience",
            "72001",
        ),
        ("defence_radius: 6", "defence_radius: 33", "difficulties[0].defence_radius", "33"),
        ("            id: \"easy\",", "            id: \"Easy!\",", "difficulties[0].id", "\"Easy!\""),
        (
            "build_order: [\"power\", \"refinery\", \"barracks\", \"factory\", \"power\", \"turret\"],",
            "build_order: [],",
            "build_order",
            "an empty list",
        ),
        ("army: [\"tank\", \"tank\", \"rifleman\"],", "army: [],", "army", "an empty list"),
        (
            "army: [\"tank\", \"tank\", \"rifleman\"],",
            "army: [\"tank\", \"crawler\"],",
            "army[1]",
            "\"crawler\"",
        ),
        (
            "army: [\"tank\", \"tank\", \"rifleman\"],",
            "army: [\"harvester\"],",
            "army[0]",
            "\"harvester\"",
        ),
        ("harvester: \"harvester\",", "harvester: \"tank\",", "harvester", "\"tank\""),
        ("power: \"power\",", "power: \"barracks\",", "power", "\"barracks\", which produces no power"),
        (
            "build_order: [\"power\", \"refinery\",",
            "build_order: [\"refinery\", \"power\",",
            "build_order[0]",
            "\"refinery\", whose prerequisite \"power\" is not listed before it",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let path = variant(File::Ai, &format!("ai_range_{index}"), &[(from, to)]);
        assert_invalid(File::Ai, &path, field, found);
    }
}

#[test]
fn ai_references_ids_and_the_armys_reach_are_checked() {
    let cases: [(&str, &str, &str, DataErrorKind); 6] = [
        (
            "build_order: [\"power\",",
            "build_order: [\"castle\",",
            "build_order[0]",
            unknown("building", "castle"),
        ),
        (
            "power: \"power\",",
            "power: \"castle\",",
            "power",
            unknown("building", "castle"),
        ),
        (
            "harvester: \"harvester\",",
            "harvester: \"dragon\",",
            "harvester",
            unknown("unit", "dragon"),
        ),
        (
            "army: [\"tank\",",
            "army: [\"dragon\",",
            "army[0]",
            unknown("unit", "dragon"),
        ),
        (
            "            id: \"normal\",",
            "            id: \"easy\",",
            "difficulties[1].id",
            DataErrorKind::DuplicateId("easy".into()),
        ),
        // Riflemen alone cannot damage concrete (bullet against concrete: 0 %), so a match of
        // computer players with this army could not end.
        (
            "army: [\"tank\", \"tank\", \"rifleman\"],",
            "army: [\"rifleman\"],",
            "army",
            DataErrorKind::MissingEntry {
                kind: "unit able to damage building".into(),
                id: "yard".into(),
            },
        ),
    ];
    for (index, (from, to, field, kind)) in cases.into_iter().enumerate() {
        let path = variant(File::Ai, &format!("ai_reference_{index}"), &[(from, to)]);
        assert_rejected(File::Ai, &path, field, kind);
    }
    // The AI file is checked against the weapons of a combat file; without one it is rejected.
    let without_combat = sim::load_definitions(
        fixtures().join("units_combat.ron"),
        fixtures().join("economy_combat.ron"),
    )
    .expect("definitions");
    let error = load_with(File::Ai, &File::Ai.fixture(), without_combat).expect_err("no combat");
    assert_eq!(error.field, "file", "{error}");
    // A skirmish file whose names are unknown to other definitions (the fixture names fixture
    // types; the production units file has none of them).
    let production = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data");
    let other = sim::load_match_definitions(
        production.join("units.ron"),
        production.join("economy.ron"),
        production.join("combat.ron"),
    )
    .expect("production definitions");
    let error = load_with(File::Skirmish, &File::Skirmish.fixture(), other).expect_err("unknown");
    assert_eq!(error.kind, unknown("building", "yard"), "{error}");
}

/// The fixture definitions with each `(from, to)` edit applied to the economy fixture.
fn definitions_with_economy(name: &str, edits: &[(&str, &str)]) -> Definitions {
    let mut text =
        std::fs::read_to_string(fixtures().join("economy_combat.ron")).expect("fixture exists");
    for (from, to) in edits {
        assert_eq!(
            text.matches(from).count(),
            1,
            "{name}: economy edit {from:?}"
        );
        text = text.replacen(from, to, 1);
    }
    let economy = temp_file(&format!("{name}.ron"), text.as_bytes());
    load_match_definitions(
        fixtures().join("units_combat.ron"),
        economy,
        fixtures().join("combat.ron"),
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn the_army_harvester_and_power_building_can_be_obtained_through_the_build_order() {
    // M6 review 1 P2-1: an AI file whose army or harvester could never be produced (its producer
    // or a prerequisite is not in the build order) was accepted, and two computer players with it
    // played 36,000 ticks without an outcome. The reviewer's case on the fixture: the factory, the
    // producer of the tank and the harvester, removed from the build order.
    let order =
        "build_order: [\"power\", \"refinery\", \"barracks\", \"factory\", \"power\", \"turret\"],";
    let path = variant(
        File::Ai,
        "ai_no_factory",
        &[
            (
                order,
                "build_order: [\"power\", \"refinery\", \"barracks\", \"power\", \"turret\"],",
            ),
            (
                "army: [\"tank\", \"tank\", \"rifleman\"],",
                "army: [\"tank\"],",
            ),
        ],
    );
    assert_invalid(
        File::Ai,
        &path,
        "harvester",
        "\"harvester\", whose producer \"factory\" is not listed in the build order",
    );
    // An army unit type whose producer is missing.
    let path = variant(
        File::Ai,
        "ai_no_barracks",
        &[(
            order,
            "build_order: [\"power\", \"refinery\", \"factory\", \"power\", \"turret\"],",
        )],
    );
    assert_invalid(
        File::Ai,
        &path,
        "army[2]",
        "\"rifleman\", whose producer \"barracks\" is not listed in the build order",
    );
    // An army unit type whose prerequisite is missing (the tank needs a fence in this variant of
    // the economy fixture).
    let tank = "unit: \"tank\",\n            cost: 200,\n            build_time: 20,\n            producer: \"factory\",\n            prerequisites: [],";
    let definitions = definitions_with_economy(
        "economy_tank_needs_fence",
        &[(
            tank,
            &tank.replace("prerequisites: []", "prerequisites: [\"fence\"]"),
        )],
    );
    let error = load_with(File::Ai, &File::Ai.fixture(), definitions).expect_err("fence");
    assert_eq!(error.field, "army[0]", "{error}");
    assert_eq!(
        error.kind,
        DataErrorKind::InvalidValue {
            found: "\"tank\", whose prerequisite \"fence\" is not listed in the build order".into(),
            expected: "a unit type whose producer and prerequisites are listed in the build order"
                .into(),
        }
    );
    // A power building with prerequisites: it is queued whenever power is short, possibly before
    // they are built. The build order lists the fence first, so only the power rule fails.
    let power = "id: \"power\",\n            sprite: \"power\",\n            width: 2,\n            height: 2,\n            cost: 105,\n            build_time: 10,\n            producer: \"yard\",\n            prerequisites: [],";
    let definitions = definitions_with_economy(
        "economy_power_needs_fence",
        &[(
            power,
            &power.replace("prerequisites: []", "prerequisites: [\"fence\"]"),
        )],
    );
    let path = variant(
        File::Ai,
        "ai_fence_first",
        &[(
            "build_order: [\"power\",",
            "build_order: [\"fence\", \"power\",",
        )],
    );
    let error = load_with(File::Ai, &path, definitions).expect_err("power prerequisites");
    assert_eq!(error.field, "power", "{error}");
    assert_eq!(
        error.kind,
        DataErrorKind::InvalidValue {
            found: "\"power\", which has prerequisites".into(),
            expected: "a building type that produces power and has no prerequisites".into(),
        }
    );
    // The fixture, whose build order lists the factory, the barracks, and the refinery, loads.
    assert!(matches!(
        load(File::Ai, &File::Ai.fixture()),
        Ok(Loaded::Ai(_))
    ));
}

#[test]
fn the_ai_file_is_checked_against_every_faction_of_the_skirmish_file() {
    // M6 review 1 P2-1: the buildings of the build order and the power building are produced by
    // a building the factions start with (the yard); a faction without it could never build.
    let definitions = definitions();
    let Ok(Loaded::Ai(ai)) = load(File::Ai, &File::Ai.fixture()) else {
        panic!("the AI fixture loads");
    };
    let Ok(Loaded::Skirmish(skirmish)) = load(File::Skirmish, &File::Skirmish.fixture()) else {
        panic!("the skirmish fixture loads");
    };
    let path = File::Ai.fixture();
    check_ai_factions(&path, &ai, &skirmish, &definitions)
        .unwrap_or_else(|error| panic!("{error}"));
    let building = |id| definitions.building_kind(id).expect("building");
    let mut without_yard = skirmish.clone();
    without_yard.factions[1].buildings = vec![StartingBuilding {
        kind: building("power"),
        offset: (0, 0),
    }];
    let error = check_ai_factions(&path, &ai, &without_yard, &definitions).expect_err("no yard");
    assert_eq!(
        (error.file.as_path(), error.field.as_str()),
        (path.as_path(), "build_order[0]")
    );
    let message = error.to_string();
    assert!(
        message.contains("\"power\", whose producer \"yard\" faction \"blue\" of the skirmish file does not start with"),
        "{message}"
    );
    // A building of the build order may be the producer of a later one.
    let mut ai_with_yard = ai.clone();
    ai_with_yard.build_order.insert(0, building("yard"));
    let mut yard_produces_itself = without_yard.clone();
    yard_produces_itself.factions[1].buildings[0].kind = building("yard");
    check_ai_factions(&path, &ai_with_yard, &yard_produces_itself, &definitions)
        .unwrap_or_else(|error| panic!("{error}"));
    // The power building's producer must be a starting building (it is queued from the first
    // turn on whenever power is short).
    let mut power_only = ai.clone();
    power_only.build_order.clear();
    let error =
        check_ai_factions(&path, &power_only, &without_yard, &definitions).expect_err("power");
    assert_eq!(error.field, "power", "{error}");
}

#[test]
fn start_positions_are_checked_against_their_map() {
    // M6 review 1 P1-1: the skirmish file is read without its maps, so its start positions are
    // limited only to the largest map (0 to 255, `limits_are_accepted_at_both_ends`); every loader
    // of a map checks them against the map's grid. The reviewer's case: (255, 255) on the 40 x 40
    // fixture map.
    let grid = load_map(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/maps/skirmish_40x40.tmx"),
    )
    .unwrap_or_else(|error| panic!("{error}"))
    .grid;
    let path = File::Skirmish.fixture();
    let Ok(Loaded::Skirmish(skirmish)) = load(File::Skirmish, &path) else {
        panic!("the skirmish fixture loads");
    };
    check_map_starts(&path, &skirmish, 0, &grid).unwrap_or_else(|error| panic!("{error}"));
    let cases = [
        (
            1,
            CellPos::new(255, 255),
            "(255, 255), which is outside the 40 x 40 cells of map \"skirmish_40x40\"",
        ),
        (
            0,
            CellPos::new(40, 5),
            "(40, 5), which is outside the 40 x 40 cells of map \"skirmish_40x40\"",
        ),
        (
            1,
            CellPos::new(-1, 0),
            "(-1, 0), which is outside the 40 x 40 cells of map \"skirmish_40x40\"",
        ),
        // The fixture map has rock at (25..=27, 9).
        (
            0,
            CellPos::new(25, 9),
            "(25, 9), which is impassable terrain of map \"skirmish_40x40\"",
        ),
    ];
    for (position, cell, found) in cases {
        let mut bad = skirmish.clone();
        bad.maps[0].starts[position] = cell;
        let error = check_map_starts(&path, &bad, 0, &grid).expect_err("a start off the map");
        assert_eq!(error.file, path);
        assert_eq!(
            error.field,
            format!("maps[0].starts[{position}]"),
            "{error}"
        );
        assert_eq!(
            error.kind,
            DataErrorKind::InvalidValue {
                found: found.into(),
                expected: "a cell of the map with passable terrain".into(),
            }
        );
        assert!(
            error
                .to_string()
                .contains(&format!("data file {}", path.display()))
        );
    }
    // The corners of the map are cells of it.
    let mut corners = skirmish.clone();
    corners.maps[0].starts = vec![CellPos::new(0, 0), CellPos::new(39, 39)];
    check_map_starts(&path, &corners, 0, &grid).unwrap_or_else(|error| panic!("{error}"));
    // A map index outside the file.
    let error = check_map_starts(&path, &skirmish, 1, &grid).expect_err("no map 1");
    assert_eq!(error.field, "maps", "{error}");
}

#[test]
fn every_field_is_required_and_unknown_fields_are_rejected() {
    let skirmish_fields = [
        ("    starting_credits: 1500,\n", "starting_credits"),
        ("            id: \"red\",\n", "factions[0].id"),
        (
            "            buildings: [(building: \"yard\", x: 0, y: 0)],\n",
            "factions[0].buildings",
        ),
    ];
    for (index, (from, field)) in skirmish_fields.into_iter().enumerate() {
        let path = variant(
            File::Skirmish,
            &format!("skirmish_missing_{index}"),
            &[(from, "")],
        );
        assert_rejected(File::Skirmish, &path, field, DataErrorKind::MissingField);
    }
    let path = variant(
        File::Skirmish,
        "skirmish_missing_count",
        &[(
            "(unit: \"tank\", x: 4, y: 1, count: 2)",
            "(unit: \"tank\", x: 4, y: 1)",
        )],
    );
    assert_rejected(
        File::Skirmish,
        &path,
        "factions[0].units[0].count",
        DataErrorKind::MissingField,
    );
    let path = variant(
        File::Skirmish,
        "skirmish_unknown_field",
        &[(
            "    starting_credits: 1500,",
            "    starting_credits: 1500,\n    seed: 4,",
        )],
    );
    assert_rejected(File::Skirmish, &path, "seed", DataErrorKind::UnknownField);

    let ai_fields = [
        ("    power: \"power\",\n", "power"),
        ("    harvester: \"harvester\",\n", "harvester"),
        (
            "            wave_patience: 1200,\n",
            "difficulties[0].wave_patience",
        ),
        (
            "            defence_radius: 8,\n",
            "difficulties[1].defence_radius",
        ),
    ];
    for (index, (from, field)) in ai_fields.into_iter().enumerate() {
        let path = variant(File::Ai, &format!("ai_missing_{index}"), &[(from, "")]);
        assert_rejected(File::Ai, &path, field, DataErrorKind::MissingField);
    }
    let path = variant(
        File::Ai,
        "ai_unknown_field",
        &[(
            "            max_army: 6,",
            "            max_army: 6,\n            cheat: 1,",
        )],
    );
    assert_rejected(
        File::Ai,
        &path,
        "difficulties[0].cheat",
        DataErrorKind::UnknownField,
    );
    let path = variant(
        File::Ai,
        "ai_duplicate_field",
        &[(
            "            max_army: 6,",
            "            max_army: 6,\n            max_army: 7,",
        )],
    );
    assert_rejected(
        File::Ai,
        &path,
        "difficulties[0].max_army",
        DataErrorKind::DuplicateField,
    );
    let path = variant(
        File::Ai,
        "ai_wrong_kind",
        &[("    power: \"power\",", "    power: 3,")],
    );
    let error = load(File::Ai, &path).expect_err("kind");
    assert_eq!(error.field, "power", "{error}");
    assert!(
        matches!(error.kind, DataErrorKind::Syntax { .. }),
        "{error}"
    );
    let too_long = format!(
        "build_order: [{}],",
        vec!["\"power\""; MAX_BUILD_ORDER + 1].join(", ")
    );
    let path = variant(
        File::Ai,
        "ai_too_long",
        &[(
            "build_order: [\"power\", \"refinery\", \"barracks\", \"factory\", \"power\", \"turret\"],",
            &too_long,
        )],
    );
    assert_rejected(
        File::Ai,
        &path,
        "build_order",
        DataErrorKind::TooManyEntries {
            max: MAX_BUILD_ORDER,
        },
    );
}

// --- Negative: files ------------------------------------------------------------------------

#[test]
fn unreadable_oversized_and_special_files_are_rejected() {
    for file in [File::Skirmish, File::Ai] {
        let name = format!("{file:?}").to_lowercase();
        let missing_file = temp_dir().join(format!("{name}_does_not_exist.ron"));
        let error = load(file, &missing_file).expect_err("missing");
        assert!(matches!(error.kind, DataErrorKind::Read(_)), "{error}");
        assert_eq!(error.field, "file");
        let mut text = file.text().into_bytes();
        text.resize(MAX_DATA_FILE_BYTES as usize + 1, b' ');
        assert_rejected(
            file,
            &temp_file(&format!("{name}_too_large.ron"), &text),
            "file",
            DataErrorKind::FileTooLarge {
                max_bytes: MAX_DATA_FILE_BYTES,
            },
        );
        let fifo = temp_dir().join(format!("{name}_pipe.ron"));
        let _ = std::fs::remove_file(&fifo);
        let made = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("mkfifo runs");
        assert!(made.success(), "mkfifo failed");
        assert_rejected(
            file,
            &fifo,
            "file",
            DataErrorKind::Read("not a regular file".into()),
        );
        std::fs::remove_file(&fifo).expect("remove the pipe");
        assert_rejected(
            file,
            &temp_dir(),
            "file",
            DataErrorKind::Read("not a regular file".into()),
        );
        let attribute = temp_file(
            &format!("{name}_attribute.ron"),
            format!("#![enable(implicit_some)]\n{}", file.text()).as_bytes(),
        );
        let error = load(file, &attribute).expect_err("attribute");
        assert!(
            matches!(
                error.kind,
                DataErrorKind::Syntax {
                    line: 1,
                    column: 1,
                    ..
                }
            ),
            "{error}"
        );
    }
}

#[test]
fn truncations_of_the_fixtures_are_rejected_without_panic() {
    for file in [File::Skirmish, File::Ai] {
        let text = file.text();
        let end = text.rfind(')').expect("closing parenthesis");
        let path = temp_dir().join(format!("{file:?}_prefix.ron"));
        for cut in (0..=end).step_by(3).chain([end]) {
            std::fs::write(&path, &text.as_bytes()[..cut]).expect("prefix");
            let error = load(file, &path).expect_err("a truncated file must be rejected");
            assert_eq!(error.file, path);
            assert!(!error.field.is_empty());
        }
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

const SNIPPETS: [&str; 20] = [
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
    "\"tank\"",
    "\"power\", ",
    "(x: 1, y: 2),",
    "(unit: \"tank\", x: 0, y: 0, count: 1),",
    "{",
    "1.5",
    "\n",
    "\u{e9}",
];

#[test]
fn seeded_mutations_of_the_fixtures_load_or_return_a_typed_error() {
    for (file, seed) in [(File::Skirmish, 0x0006_5c01_u64), (File::Ai, 0x0006_a1a1)] {
        let base = file.text().into_bytes();
        let mut rng = SplitMix64(seed);
        let path = temp_dir().join(format!("{file:?}_mutation.ron"));
        let definitions = definitions();
        let mut loaded = 0;
        for case in 0..1_500 {
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
            match load_with(file, &path, definitions.clone()) {
                Ok(Loaded::Skirmish(skirmish)) => {
                    loaded += 1;
                    assert!((0..=MAX_STARTING_CREDITS).contains(&skirmish.starting_credits));
                    for faction in &skirmish.factions {
                        assert!(!faction.buildings.is_empty(), "case {case}");
                        assert!(faction.buildings.iter().all(|building| {
                            usize::from(building.kind.0) < definitions.buildings().len()
                        }));
                        assert!(faction.units.iter().all(|group| {
                            (1..=MAX_GROUP_UNITS).contains(&group.count)
                                && usize::from(group.kind.0) < definitions.units().len()
                        }));
                    }
                    assert!(!skirmish.maps.is_empty(), "case {case}");
                    for map in &skirmish.maps {
                        assert_ne!(map.starts[0], map.starts[1], "case {case}");
                    }
                }
                Ok(Loaded::Ai(ai)) => {
                    loaded += 1;
                    assert!(!ai.build_order.is_empty() && ai.build_order.len() <= MAX_BUILD_ORDER);
                    assert!(ai.army.iter().all(|&kind: &UnitKind| {
                        definitions
                            .unit_combat(kind)
                            .is_some_and(|terms| terms.weapon.is_some())
                    }));
                    assert!(ai.build_order.iter().all(|&kind: &BuildingKind| {
                        usize::from(kind.0) < definitions.buildings().len()
                    }));
                    for difficulty in &ai.difficulties {
                        assert!((1..=difficulty.max_army).contains(&difficulty.wave_size));
                        assert!(difficulty.max_army <= MAX_AI_ARMY);
                        assert!((1..=MAX_THINK_INTERVAL).contains(&difficulty.think_interval));
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
        assert!(loaded > 0, "{file:?}: no mutation stayed valid");
    }
}
