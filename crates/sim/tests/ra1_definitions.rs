//! RA1 tests: the data files of sides, countries and match setup (docs/RA2_PARITY_PLAN.md section
//! 5, RA1, "Tests": malformed `rules.ron`, every field required, unknown field, each range, each
//! dangling reference; invariants I8, I10, I11; docs/DECISIONS.md D15, D18).
//!
//! Four loaders are covered: the rules file (schema version 1, `Definitions::with_rules`), the
//! arsenal (schema version 2, `Definitions::with_arsenal`), the skirmish file (schema version 3,
//! `load_skirmish`) and the AI file (schema version 2, `load_ai` and `check_ai_factions`). Each is
//! checked against the supported subset of RON and its schema before `ron` sees it, then validated:
//! ranges at both ends, references to unit and building types, to countries, prerequisite groups
//! and sides, uniqueness, and the cross checks of each file. Every rejection is a typed
//! `DataError` naming the file, the field, and the reason.
//!
//! The tests are written in the style of `m6_definitions.rs` and bounded in the same way (D15):
//! this test binary's global allocator refuses allocations beyond [`MEMORY_BUDGET`], and every
//! load runs on a worker thread whose result must arrive within [`TIME_LIMIT`].
//!
//! Variants are derived from the committed RA1 fixture `tests/fixtures/data/ra1` (a small world
//! with two sides and three countries) by exact text edits and written to Cargo's temporary
//! directory for integration tests, under names unique to each test. The fixture itself is never
//! changed.

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeSet;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use sim::{
    AiDefinitions, AiPlan, BuildingKind, Category, CellPos, Country, CountryId, DataError,
    DataErrorKind, Definitions, DifficultyRules, Domain, FactoryGroup, GroupId, MAX_AI_ARMY,
    MAX_ARMY_ENTRIES, MAX_BUILD_ORDER, MAX_COUNTRIES, MAX_DEFENCE_RADIUS, MAX_DIFFICULTIES,
    MAX_FACTIONS, MAX_GROUP_BUILDINGS, MAX_GROUP_UNITS, MAX_HARVESTERS_PER_REFINERY, MAX_LABEL_LEN,
    MAX_MAPS, MAX_MATCH_PLAYERS, MAX_PLANS, MAX_POWERUPS, MAX_PREREQUISITE_GROUPS,
    MAX_ROSTER_ENTRIES, MAX_RULES_DIFFICULTIES, MAX_START_OFFSET, MAX_STARTING_BUILDINGS,
    MAX_STARTING_CREDITS, MAX_STARTING_GROUPS, MAX_STARTING_TYPES, MAX_STRING_BYTES,
    MAX_TECH_LEVEL, MAX_THINK_INTERVAL, MapDef, Powerup, PrerequisiteGroup, ProductionItem, Role,
    Rules, Side, SideRules, SideStart, SkirmishDefinitions, StartingBuilding, StartingGroup,
    UnitKind, check_ai_factions, country_tree, load_ai, load_definitions, load_match_definitions,
    load_skirmish,
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

/// Which of the four files a test loads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum File {
    Rules,
    Arsenal,
    Skirmish,
    Ai,
}

impl File {
    const ALL: [File; 4] = [File::Rules, File::Arsenal, File::Skirmish, File::Ai];

    fn fixture(self) -> PathBuf {
        fixtures().join(match self {
            File::Rules => "rules.ron",
            File::Arsenal => "arsenal.ron",
            File::Skirmish => "skirmish.ron",
            File::Ai => "ai.ron",
        })
    }

    fn text(self) -> String {
        std::fs::read_to_string(self.fixture()).expect("fixture exists")
    }

    /// The fixture definitions the file is loaded against: the rules file names the types of
    /// the units and economy files, the arsenal the countries and groups of the rules, and the
    /// skirmish and AI files all of them.
    fn context(self) -> Definitions {
        match self {
            File::Rules => base_definitions(),
            File::Arsenal => rules_definitions(),
            File::Skirmish | File::Ai => definitions(),
        }
    }
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data/ra1")
}

/// The definitions of the units and combat fixtures and the economy file `economy`, with the
/// rules file `rules` and then the arsenal `arsenal` if they are given.
fn definitions_of(economy: &Path, rules: Option<&Path>, arsenal: Option<&Path>) -> Definitions {
    let mut definitions = load_match_definitions(
        fixtures().join("units.ron"),
        economy,
        fixtures().join("combat.ron"),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    if let Some(rules) = rules {
        definitions = definitions
            .with_rules(rules)
            .unwrap_or_else(|error| panic!("{error}"));
    }
    if let Some(arsenal) = arsenal {
        definitions = definitions
            .with_arsenal(arsenal)
            .unwrap_or_else(|error| panic!("{error}"));
    }
    definitions
}

/// The units, economy and combat fixtures: definitions without a rules file.
fn base_definitions() -> Definitions {
    static BASE: OnceLock<Definitions> = OnceLock::new();
    BASE.get_or_init(|| definitions_of(&fixtures().join("economy.ron"), None, None))
        .clone()
}

/// [`base_definitions`] with the rules fixture, without an arsenal.
fn rules_definitions() -> Definitions {
    static WITH_RULES: OnceLock<Definitions> = OnceLock::new();
    WITH_RULES
        .get_or_init(|| {
            definitions_of(
                &fixtures().join("economy.ron"),
                Some(&File::Rules.fixture()),
                None,
            )
        })
        .clone()
}

/// The whole RA1 fixture: units, economy, combat, rules and arsenal.
fn definitions() -> Definitions {
    static FULL: OnceLock<Definitions> = OnceLock::new();
    FULL.get_or_init(|| {
        definitions_of(
            &fixtures().join("economy.ron"),
            Some(&File::Rules.fixture()),
            Some(&File::Arsenal.fixture()),
        )
    })
    .clone()
}

/// What a load returned: the definitions with the rules file or the arsenal, the skirmish
/// definitions, or the AI definitions.
#[derive(Debug)]
enum Loaded {
    Definitions(Box<Definitions>),
    Skirmish(SkirmishDefinitions),
    Ai(AiDefinitions),
}

/// Loads `path` as `file` against `context` on a worker thread, failing the test if the loader
/// panics or takes longer than [`TIME_LIMIT`].
fn load_with(file: File, path: &Path, context: Definitions) -> Result<Loaded, DataError> {
    let owned = path.to_owned();
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name(format!("load {}", path.display()))
        .spawn(move || {
            let result = match file {
                File::Rules => context
                    .with_rules(&owned)
                    .map(|definitions| Loaded::Definitions(Box::new(definitions))),
                File::Arsenal => context
                    .with_arsenal(&owned)
                    .map(|definitions| Loaded::Definitions(Box::new(definitions))),
                File::Skirmish => load_skirmish(&owned, &context).map(Loaded::Skirmish),
                File::Ai => load_ai(&owned, &context).map(Loaded::Ai),
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
    load_with(file, path, file.context())
}

/// The definitions a load of the rules file or the arsenal returned.
fn as_definitions(result: Result<Loaded, DataError>) -> Definitions {
    match result {
        Ok(Loaded::Definitions(definitions)) => *definitions,
        Ok(other) => panic!("expected definitions, found {other:?}"),
        Err(error) => panic!("{error}"),
    }
}

/// The skirmish definitions a load of the skirmish file returned.
fn as_skirmish(result: Result<Loaded, DataError>) -> SkirmishDefinitions {
    match result {
        Ok(Loaded::Skirmish(skirmish)) => skirmish,
        Ok(other) => panic!("expected skirmish definitions, found {other:?}"),
        Err(error) => panic!("{error}"),
    }
}

/// The AI definitions a load of the AI file returned.
fn as_ai(result: Result<Loaded, DataError>) -> AiDefinitions {
    match result {
        Ok(Loaded::Ai(ai)) => ai,
        Ok(other) => panic!("expected AI definitions, found {other:?}"),
        Err(error) => panic!("{error}"),
    }
}

fn temp_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ra1_definitions");
    std::fs::create_dir_all(&dir).expect("temporary directory");
    dir
}

/// `name` must be unique among all tests, since the tests run in parallel.
fn temp_file(name: &str, contents: &[u8]) -> PathBuf {
    let path = temp_dir().join(name);
    std::fs::write(&path, contents).expect("temporary file");
    path
}

/// The `(from, to)` edits of one case of a table.
type Edits = &'static [(&'static str, &'static str)];

/// `text` with the edit `(from, to)` applied; `from` must occur exactly once.
fn edit(name: &str, text: &str, from: &str, to: &str) -> String {
    assert_eq!(
        text.matches(from).count(),
        1,
        "{name}: fixture edit {from:?}"
    );
    text.replacen(from, to, 1)
}

/// The fixture of `file` with each `(from, to)` edit applied; `from` must occur exactly once.
/// `name` must be unique among all tests, since the tests run in parallel.
fn variant(file: File, name: &str, edits: &[(&str, &str)]) -> PathBuf {
    let mut text = file.text();
    for (from, to) in edits {
        text = edit(name, &text, from, to);
    }
    temp_file(&format!("{name}.ron"), text.as_bytes())
}

/// `text` with its one line that contains `line` replaced by what `change` makes of it.
fn with_line(name: &str, text: &str, line: &str, change: impl FnOnce(&str) -> String) -> String {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let found: Vec<usize> = (0..lines.len())
        .filter(|&index| lines[index].contains(line))
        .collect();
    assert_eq!(found.len(), 1, "{name}: fixture line {line:?}");
    let index = found[0];
    format!(
        "{}{}{}",
        lines[..index].concat(),
        change(lines[index]),
        lines[index + 1..].concat()
    )
}

/// `text` with each `(from, to)` edit applied within its one line that contains `line`; `from`
/// must occur exactly once in that line. The entries of the fixture lists stand on one line
/// each, and most of their values recur in the other entries.
fn line_edit(name: &str, text: &str, line: &str, edits: &[(&str, &str)]) -> String {
    with_line(name, text, line, |found| {
        let mut edited = found.to_owned();
        for (from, to) in edits {
            edited = edit(name, &edited, from, to);
        }
        edited
    })
}

/// The fixture of `file` with each `(from, to)` edit applied within its one line that contains
/// `line` ([`line_edit`]).
fn line_variant(file: File, name: &str, line: &str, edits: &[(&str, &str)]) -> PathBuf {
    let text = line_edit(name, &file.text(), line, edits);
    temp_file(&format!("{name}.ron"), text.as_bytes())
}

/// `text` with its one line that contains `line` written `times` times (0 removes the line).
fn repeat_line(name: &str, text: &str, line: &str, times: usize) -> String {
    with_line(name, text, line, |found| found.repeat(times))
}

/// `text` with its one block from `start` to the next `end` written `times` times (0 removes
/// the block): a plan or a difficulty of the AI file, which stand on several lines.
fn repeat_block(name: &str, text: &str, block: (&str, &str), times: usize) -> String {
    let (start, end) = block;
    assert_eq!(
        text.matches(start).count(),
        1,
        "{name}: fixture block {start:?}"
    );
    let from = text.find(start).expect("start of the block");
    let to = from + text[from..].find(end).expect("end of the block") + end.len();
    format!(
        "{}{}{}",
        &text[..from],
        text[from..to].repeat(times),
        &text[to..]
    )
}

/// `text` without the field `field` of the file's struct. The fields of the rules, skirmish and
/// AI fixtures are indented by four spaces; a group or a list ends with the next line `    ),`
/// or `    ],`.
fn without_field(name: &str, text: &str, field: &str) -> String {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let prefix = format!("    {field}: ");
    let found: Vec<usize> = (0..lines.len())
        .filter(|&index| lines[index].starts_with(&prefix))
        .collect();
    assert_eq!(found.len(), 1, "{name}: fixture field {field:?}");
    let start = found[0];
    let end = if lines[start].trim_end().ends_with(['(', '[']) {
        start
            + lines[start..]
                .iter()
                .position(|line| matches!(line.trim_end(), "    )," | "    ],"))
                .expect("end of the field")
    } else {
        start
    };
    format!("{}{}", lines[..start].concat(), lines[end + 1..].concat())
}

/// `[entry, entry, ...]` with `count` entries.
fn list(entry: &str, count: usize) -> String {
    format!("[{}]", vec![entry; count].join(", "))
}

/// Asserts that `error` is of `kind` at `field` of the file at `path`, and that its message
/// names the file, the field, and the reason.
fn assert_error(error: &DataError, path: &Path, field: &str, kind: &DataErrorKind) {
    assert_eq!(
        (error.field.as_str(), &error.kind),
        (field, kind),
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

/// Asserts that `path` is rejected with `kind` at `field`, and that the message names the file,
/// the field, and the reason.
fn assert_rejected(file: File, path: &Path, field: &str, kind: DataErrorKind) {
    assert_rejected_with(file, path, file.context(), field, kind);
}

/// [`assert_rejected`] for a load against `context` instead of the fixture definitions.
fn assert_rejected_with(
    file: File,
    path: &Path,
    context: Definitions,
    field: &str,
    kind: DataErrorKind,
) {
    let error = load_with(file, path, context).expect_err("the file must be rejected");
    assert_error(&error, path, field, &kind);
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

fn invalid(found: &str, expected: &str) -> DataErrorKind {
    DataErrorKind::InvalidValue {
        found: found.into(),
        expected: expected.into(),
    }
}

fn unknown(kind: &str, id: &str) -> DataErrorKind {
    DataErrorKind::UnknownReference {
        kind: kind.into(),
        id: id.into(),
    }
}

fn building(definitions: &Definitions, id: &str) -> BuildingKind {
    definitions
        .building_kind(id)
        .unwrap_or_else(|| panic!("building {id}"))
}

fn unit(definitions: &Definitions, id: &str) -> UnitKind {
    definitions
        .unit_kind(id)
        .unwrap_or_else(|| panic!("unit {id}"))
}

/// The building or unit type named `id`.
fn item(definitions: &Definitions, id: &str) -> ProductionItem {
    definitions
        .building_kind(id)
        .map(ProductionItem::Building)
        .or_else(|| definitions.unit_kind(id).map(ProductionItem::Unit))
        .unwrap_or_else(|| panic!("item {id}"))
}

// --- The scalar groups of the rules file ---------------------------------------------------------

/// Longest duration in ticks a rule may hold (one hour).
const TICKS: i64 = 72_000;
/// Largest permille value (ten times).
const PM: i64 = 10_000;
/// Largest distance in subcells (64 cells).
const SUBCELLS: i64 = 16_384;
/// Largest credit, damage or hit point amount a rule may hold.
const AMOUNT: i64 = 1_000_000;

/// A scalar rule: its name and the least and the greatest value `rules.rs` documents for it.
type Scalar = (&'static str, i64, i64);

/// The scalar groups of the rules file in the file's order, with every field of each.
const SCALAR_GROUPS: [(&str, &[Scalar]); 10] = [
    (
        "production",
        &[
            ("build_ticks_per_1000", 1, TICKS),
            ("multiple_factory_pm", 1, 1000),
            ("wall_build_time_pm", 1, PM),
            ("low_power_penalty_pm", 0, PM),
            ("min_low_power_speed_pm", 0, 1000),
            ("max_low_power_speed_pm", 0, 1000),
            ("max_queued_objects", 0, 255),
        ],
    ),
    (
        "economy",
        &[
            ("refund_percent", 0, 100),
            ("repair_percent", 0, 100),
            ("repair_step", 1, AMOUNT),
            ("repair_interval", 1, TICKS),
            ("unit_repair_interval", 1, TICKS),
            ("infantry_repair_step", 1, AMOUNT),
            ("infantry_repair_interval", 1, TICKS),
            ("reload_interval", 1, TICKS),
            ("purifier_bonus_percent", 0, 1000),
            ("ore_value", 0, AMOUNT),
            ("gem_value", 0, AMOUNT),
            ("ore_growth_interval", 1, TICKS),
            ("ore_growth_pm", 0, 1000),
            ("ore_spread_interval", 1, TICKS),
            ("ore_spread_pm", 0, 1000),
            ("harvesters_per_refinery", 0, 8),
        ],
    ),
    (
        "veterancy",
        &[
            ("veteran_ratio_pm", 1, 100_000),
            ("veteran_cap", 0, 2),
            ("combat_pm", 0, PM),
            ("speed_pm", 0, PM),
            ("sight_pm", 0, PM),
            ("armor_pm", 1, PM),
            ("rof_pm", 1, PM),
            ("initial_veteran", 0, 1),
        ],
    ),
    (
        "crew",
        &[
            ("crew_escape_percent", 0, 100),
            ("survivor_rate_pm", 0, 1000),
        ],
    ),
    (
        "combat",
        &[
            ("max_damage", 1, AMOUNT),
            ("min_damage", 0, AMOUNT),
            ("condition_yellow_percent", 0, 100),
            ("condition_red_percent", 0, 100),
            ("ballistic_scatter", 0, SUBCELLS),
            ("homing_scatter", 0, SUBCELLS),
            ("c4_delay", 1, TICKS),
            ("ivan_damage", 0, AMOUNT),
            ("ivan_delay", 1, TICKS),
            ("v3_rocket_damage", 0, AMOUNT),
            ("v3_rocket_elite_damage", 0, AMOUNT),
            ("dmisl_damage", 0, AMOUNT),
            ("dmisl_elite_damage", 0, AMOUNT),
            ("bridge_strength", 1, AMOUNT),
            ("cloak_delay", 1, TICKS),
            ("chrono_distance_factor", 1, SUBCELLS),
            ("chrono_minimum_delay", 0, TICKS),
            ("spy_power_blackout", 0, TICKS),
            ("spy_money_steal_percent", 0, 100),
            ("tree_strength", 1, AMOUNT),
        ],
    ),
    (
        "radiation",
        &[
            ("application_delay", 1, TICKS),
            ("level_delay", 1, TICKS),
            ("level_factor_pm", 0, PM),
            ("level_max", 0, AMOUNT),
            ("duration_multiple_pm", 0, PM),
        ],
    ),
    (
        "superweapons",
        &[
            ("nuclear_recharge", 1, TICKS),
            ("lightning_recharge", 1, TICKS),
            ("iron_curtain_recharge", 1, TICKS),
            ("chronosphere_recharge", 1, TICKS),
            ("paradrop_recharge", 1, TICKS),
            ("american_paradrop_recharge", 1, TICKS),
            ("iron_curtain_duration", 1, TICKS),
            ("lightning_deferment", 0, TICKS),
            ("lightning_duration", 1, TICKS),
            ("lightning_damage", 0, AMOUNT),
            ("lightning_hit_delay", 1, TICKS),
            ("lightning_scatter_delay", 1, TICKS),
            ("lightning_cell_spread", 0, 64),
            ("lightning_separation", 0, 64),
            ("american_paradrop_count", 0, 64),
            ("paradrop_radius", 0, SUBCELLS),
        ],
    ),
    (
        "prism",
        &[
            ("support_delay", 1, TICKS),
            ("support_duration", 1, TICKS),
            ("support_max", 0, 64),
            ("support_modifier_pm", 0, PM),
            ("support_height", 0, SUBCELLS),
        ],
    ),
    (
        "crates",
        &[
            ("maximum", 0, 255),
            ("minimum", 0, 255),
            ("radius", 0, SUBCELLS),
            ("regen", 1, TICKS),
            ("free_mcv", 0, 1),
        ],
    ),
    (
        "dialog",
        &[
            ("min_credits", 0, AMOUNT),
            ("credits", 0, AMOUNT),
            ("max_credits", 0, AMOUNT),
            ("credits_step", 1, AMOUNT),
            ("min_unit_count", 0, 64),
            ("unit_count", 0, 64),
            ("max_unit_count", 0, 64),
            ("tech_level", 0, MAX_TECH_LEVEL as i64),
            ("bases", 0, 1),
            ("short_game", 0, 1),
            ("crates", 0, 1),
            ("mcv_redeploys", 0, 1),
            ("bridge_destruction", 0, 1),
            ("fog_of_war", 0, 1),
            ("shroud", 0, 1),
            ("max_players", 2, 16),
        ],
    ),
];

/// The scalars whose least (`false`) or greatest (`true`) value the cross checks of the rules
/// file forbid beside the other values of the fixture (low power speed 500 to 800, condition
/// red 25 and yellow 50, 1 to 255 crates, credits 1000 to 5000 by 500 with the default 3000,
/// unit count 0 to 10 with the default 4).
const CROSS_CHECKED: [(&str, &str, bool); 13] = [
    ("production", "min_low_power_speed_pm", true),
    ("production", "max_low_power_speed_pm", false),
    ("combat", "condition_yellow_percent", false),
    ("combat", "condition_red_percent", true),
    ("crates", "maximum", false),
    ("dialog", "min_credits", true),
    ("dialog", "credits", false),
    ("dialog", "credits", true),
    ("dialog", "max_credits", false),
    ("dialog", "credits_step", true),
    ("dialog", "min_unit_count", true),
    ("dialog", "unit_count", true),
    ("dialog", "max_unit_count", false),
];

/// The fields of scalar group `group` of `rules`, in the file's order, with their values.
fn scalars(rules: &Rules, group: &str) -> Vec<(&'static str, i32)> {
    match group {
        "production" => rules.production.fields(),
        "economy" => rules.economy.fields(),
        "veterancy" => rules.veterancy.fields(),
        "crew" => rules.crew.fields(),
        "combat" => rules.combat.fields(),
        "radiation" => rules.radiation.fields(),
        "superweapons" => rules.superweapons.fields(),
        "prism" => rules.prism.fields(),
        "crates" => rules.crates.fields(),
        "dialog" => rules.dialog.fields(),
        other => panic!("no scalar group {other}"),
    }
}

/// The byte range of the line of `group.field` in the rules text `text`, and the byte range of
/// its value. A scalar group stands as `    group: (`, one line `        field: value,` per
/// field, and `    ),`; several groups have fields of the same name.
fn scalar_line(text: &str, group: &str, field: &str) -> (Range<usize>, Range<usize>) {
    let open = format!("\n    {group}: (\n");
    assert_eq!(text.matches(&open).count(), 1, "group {group}");
    let body_start = text.find(&open).expect("group") + open.len();
    let body_end = body_start + text[body_start..].find("    ),\n").expect("end of group");
    let body = &text[body_start..body_end];
    let name = format!("        {field}: ");
    assert_eq!(body.matches(&name).count(), 1, "field {group}.{field}");
    let start = body_start + body.find(&name).expect("field");
    let end = start + text[start..].find('\n').expect("end of line") + 1;
    let value = start + name.len()..end - ",\n".len();
    assert_eq!(&text[value.end..end], ",\n", "field {group}.{field}");
    (start..end, value)
}

/// The value of `group.field` in the rules text `text`.
fn scalar_value(text: &str, group: &str, field: &str) -> i64 {
    let (_, value) = scalar_line(text, group, field);
    text[value].parse().expect("an integer")
}

/// The rules fixture with the value of `group.field` replaced by `value`, or without the field
/// for `None`, written under `name`.
fn scalar_variant(name: &str, group: &str, field: &str, value: Option<&str>) -> PathBuf {
    let text = File::Rules.text();
    let (line, at) = scalar_line(&text, group, field);
    let changed = match value {
        Some(value) => format!("{}{value}{}", &text[..at.start], &text[at.end..]),
        None => format!("{}{}", &text[..line.start], &text[line.end..]),
    };
    temp_file(&format!("{name}.ron"), changed.as_bytes())
}

// --- Lines and blocks of the fixtures -------------------------------------------------------------

/// The lines of entries of the rules fixture: powerups, difficulties, prerequisite groups, sides
/// and countries.
const MONEY: &str = r#"(id: "money""#;
const ARMOUR: &str = r#"(id: "armour""#;
const EASY: &str = r#"(id: "easy""#;
const BRUTAL: &str = r#"(id: "brutal""#;
const POWER_GROUP: &str = r#"(id: "power", buildings"#;
const RADAR_GROUP: &str = r#"(id: "radar", buildings"#;
const ALLIED_RULES: &str = r#"(id: "allied", base_unit"#;
const SOVIET_RULES: &str = r#"(id: "soviet", base_unit"#;
const BETA_COUNTRY: &str = r#"(id: "beta", side"#;
const GAMMA_COUNTRY: &str = r#"(id: "gamma", side"#;

/// The lines of entries of the arsenal fixture, by index: 6, 10, 12, 13, 14, 15, 19, 20, 27.
const BARRACKS: &str = r#"(id: "allied_barracks","#;
const RADAR: &str = r#"(id: "allied_radar","#;
const TURRET: &str = r#"(id: "turret","#;
const STORM: &str = r#"(id: "storm_device","#;
const ALLIED_MCV: &str = r#"(id: "allied_mcv","#;
const SOVIET_MCV: &str = r#"(id: "soviet_mcv","#;
const SNIPER: &str = r#"(id: "sniper","#;
const RAIDER: &str = r#"(id: "raider","#;
const MINER_S: &str = r#"(id: "miner_s","#;

/// The lines of entries of the skirmish fixture: sides, factions and the map.
const ALLIED_START: &str = r#"(side: "allied", starting_vehicles"#;
const SOVIET_START: &str = r#"(side: "soviet", starting_vehicles"#;
const ALPHA_FACTION: &str = r#"(id: "alpha", label"#;
const BETA_FACTION: &str = r#"(id: "beta", label"#;
const GAMMA_FACTION: &str = r#"(id: "gamma", label"#;
const MAP: &str = r#"(id: "skirmish_40x40""#;

/// The country alpha of the skirmish fixture.
const ALPHA: &str = r#"(id: "alpha", label: "Alpha", side: "allied", roster_buildings: [], roster_units: [], buildings: [], units: []),"#;

/// A faction without a side in the place of [`ALPHA`]: a roster and starting lists, as the
/// factions of M6 and M7 have them.
const SIDELESS: &str = r#"(id: "alpha", label: "Alpha", side: "none", roster_buildings: ["allied_power"], roster_units: ["tank"], buildings: [(building: "allied_yard", x: 0, y: 0)], units: [(unit: "tank", x: 4, y: 1, count: 2)]),"#;

/// The skirmish fixture whose first faction is [`SIDELESS`] with each `(from, to)` edit applied
/// to it; `from` must occur exactly once in it.
fn sideless_variant(name: &str, edits: &[(&str, &str)]) -> PathBuf {
    let mut faction = SIDELESS.to_owned();
    for (from, to) in edits {
        faction = edit(name, &faction, from, to);
    }
    variant(File::Skirmish, name, &[(ALPHA, &faction)])
}

/// The lists of the two plans of the AI fixture.
const ALLIED_ORDER: &str = r#"build_order: ["allied_power", "allied_refinery", "allied_barracks", "allied_factory", "allied_power"],"#;
const SOVIET_ORDER: &str = r#"build_order: ["soviet_power", "soviet_refinery", "soviet_barracks", "soviet_factory", "soviet_power"],"#;
const ALLIED_ARMY: &str = r#"army: ["tank", "tank", "rifleman"],"#;
const SOVIET_ARMY: &str = r#"army: ["heavy", "conscript", "heavy"],"#;

/// The blocks of the AI fixture: the plans and the difficulties, from their first line to
/// their closing line.
const ALLIED_PLAN: (&str, &str) = ("        (\n            side: \"allied\",", "        ),\n");
const SOVIET_PLAN: (&str, &str) = ("        (\n            side: \"soviet\",", "        ),\n");
const EASY_DIFFICULTY: (&str, &str) = ("        (\n            id: \"easy\",", "        ),\n");
const BRUTAL_DIFFICULTY: (&str, &str) = ("        (\n            id: \"brutal\",", "        ),\n");

/// An arsenal file with the entries of the fixture for the items `ids` only, in this order.
fn arsenal_of(name: &str, ids: &[&str]) -> PathBuf {
    let fixture = File::Arsenal.text();
    let mut text = String::from("(schema_version: 2, entries: [\n");
    for id in ids {
        let entry = format!("(id: {id:?},");
        let lines: Vec<&str> = fixture
            .split_inclusive('\n')
            .filter(|line| line.contains(&entry))
            .collect();
        assert_eq!(lines.len(), 1, "{name}: fixture entry {id:?}");
        text.push_str(lines[0]);
    }
    text.push_str("])\n");
    temp_file(&format!("{name}.ron"), text.as_bytes())
}

/// The economy fixture with each `(from, to)` edit applied, written under `name`.
fn economy_variant(name: &str, edits: &[(&str, &str)]) -> PathBuf {
    let mut text = std::fs::read_to_string(fixtures().join("economy.ron")).expect("fixture exists");
    for (from, to) in edits {
        text = edit(name, &text, from, to);
    }
    temp_file(&format!("{name}.ron"), text.as_bytes())
}

/// The rules fixture without the Soviet side and its country, written under `name`.
fn allied_rules(name: &str) -> PathBuf {
    let text = repeat_line(name, &File::Rules.text(), SOVIET_RULES, 0);
    let text = repeat_line(name, &text, GAMMA_COUNTRY, 0);
    temp_file(&format!("{name}.ron"), text.as_bytes())
}

// --- Positive -------------------------------------------------------------------------------

#[test]
fn the_rules_fixture_loads_with_every_field() {
    let definitions = as_definitions(load(File::Rules, &File::Rules.fixture()));
    let rules = definitions.rules().expect("the rules are loaded");
    let building = |id| building(&definitions, id);
    let unit = |id| unit(&definitions, id);
    // Every scalar of every group has the value the file gives it, and the groups have no
    // other fields.
    let text = File::Rules.text();
    for (group, fields) in SCALAR_GROUPS {
        let expected: Vec<(&str, i32)> = fields
            .iter()
            .map(|&(field, _, _)| {
                let value = scalar_value(&text, group, field);
                (field, i32::try_from(value).expect("a fixture value"))
            })
            .collect();
        assert_eq!(scalars(rules, group), expected, "{group}");
    }
    assert_eq!(rules.dialog.credits, 3000);
    assert_eq!(
        (
            rules.dialog.min_credits,
            rules.dialog.max_credits,
            rules.dialog.credits_step,
        ),
        (1000, 5000, 500)
    );
    assert_eq!(
        (
            rules.dialog.min_unit_count,
            rules.dialog.unit_count,
            rules.dialog.max_unit_count,
            rules.dialog.tech_level,
        ),
        (0, 4, 10, 5)
    );
    assert_eq!(rules.production.build_ticks_per_1000, 840);
    assert_eq!(rules.economy.refund_percent, 50);
    assert_eq!(rules.crates.maximum, 255);
    assert_eq!(
        rules.powerups,
        [
            Powerup {
                id: "money".into(),
                share: 20,
                value: 2000,
            },
            Powerup {
                id: "armour".into(),
                share: 10,
                value: 1500,
            },
        ]
    );
    assert_eq!(rules.difficulties.len(), 2);
    assert_eq!(rules.difficulties[0].id, "easy");
    assert_eq!(
        rules.difficulties[1],
        DifficultyRules {
            id: "brutal".into(),
            rof_pm: 800,
            build_time_pm: 800,
            credits_bonus_percent: 400,
            virtual_purifiers: 4,
            team_delay: 2667,
            allied_base_defences: 25,
            soviet_base_defences: 25,
        }
    );
    assert_eq!(
        rules.prerequisite_groups,
        [
            PrerequisiteGroup {
                id: "power".into(),
                buildings: vec![building("allied_power"), building("soviet_power")],
            },
            PrerequisiteGroup {
                id: "barracks".into(),
                buildings: vec![building("allied_barracks"), building("soviet_barracks")],
            },
            PrerequisiteGroup {
                id: "radar".into(),
                buildings: vec![building("allied_radar"), building("soviet_radar")],
            },
        ]
    );
    assert_eq!(
        rules.sides,
        [
            SideRules {
                side: Side::Allied,
                base_unit: unit("allied_mcv"),
                construction_yard: building("allied_yard"),
                crew: unit("rifleman"),
                survivor_divisor: 500,
                disguise: unit("rifleman"),
                paradrop_infantry: unit("rifleman"),
                paradrop_count: 6,
            },
            SideRules {
                side: Side::Soviet,
                base_unit: unit("soviet_mcv"),
                construction_yard: building("soviet_yard"),
                crew: unit("conscript"),
                survivor_divisor: 250,
                disguise: unit("conscript"),
                paradrop_infantry: unit("conscript"),
                paradrop_count: 9,
            },
        ]
    );
    assert_eq!(
        rules.countries,
        [
            Country {
                id: "alpha".into(),
                side: Side::Allied,
            },
            Country {
                id: "beta".into(),
                side: Side::Allied,
            },
            Country {
                id: "gamma".into(),
                side: Side::Soviet,
            },
        ]
    );
    // The lookups by id and by side.
    assert_eq!(rules.country_id("beta"), Some(CountryId(1)));
    assert_eq!(rules.country_id("delta"), None);
    assert_eq!(
        rules.country(CountryId(2)).map(|country| country.side),
        Some(Side::Soviet)
    );
    assert_eq!(rules.group_id("radar"), Some(GroupId(2)));
    assert_eq!(rules.group_id("airfield"), None);
    assert_eq!(rules.side(Side::Soviet), Some(&rules.sides[1]));
    assert!(rules.is_base_unit(unit("soviet_mcv")));
    assert!(!rules.is_base_unit(unit("tank")));
    assert_eq!(
        rules
            .side_of_construction_yard(building("allied_yard"))
            .map(|side| side.side),
        Some(Side::Allied)
    );
    // The rules file is an extension: the definitions it is loaded onto have none.
    assert!(base_definitions().rules().is_none());
}

#[test]
fn the_arsenal_fixture_loads_with_the_fields_of_schema_version_2() {
    let definitions = as_definitions(load(File::Arsenal, &File::Arsenal.fixture()));
    let entry = |id: &str| {
        definitions
            .metadata(item(&definitions, id))
            .unwrap_or_else(|| panic!("{id} has an entry"))
    };
    assert_eq!(definitions.arsenal().0.len(), 29);
    // The special of country alpha: an Allied unit that needs any radar.
    let sniper = entry("sniper");
    assert_eq!(
        (sniper.label.as_str(), sniper.description.as_str()),
        ("Sniper", "Fixture item.")
    );
    assert_eq!(
        (sniper.category, sniper.domain, sniper.role),
        (Category::Infantry, Domain::Land, Role::Combat)
    );
    assert_eq!(
        (
            sniper.cooldown,
            sniper.radius,
            sniper.strength,
            sniper.capacity
        ),
        (100, 2, 20, 0)
    );
    assert_eq!(sniper.art, "sniper");
    assert_eq!(sniper.side, Some(Side::Allied));
    assert_eq!(sniper.factory, None);
    assert_eq!(sniper.owner_sides, [Side::Allied]);
    assert_eq!(sniper.required_countries, [CountryId(0)]);
    assert!(sniper.forbidden_countries.is_empty());
    assert_eq!(sniper.tech_level, 1);
    assert_eq!(sniper.prerequisite_groups, [GroupId(2)]);
    // A factory: its group, its side, and both owner sides.
    let barracks = entry("allied_barracks");
    assert_eq!(barracks.category, Category::Buildings);
    assert_eq!(barracks.side, Some(Side::Allied));
    assert_eq!(barracks.factory, Some(FactoryGroup::Infantry));
    assert_eq!(barracks.owner_sides, [Side::Allied, Side::Soviet]);
    assert_eq!(barracks.prerequisite_groups, [GroupId(0)]);
    assert_eq!(barracks.tech_level, 1);
    let factory = entry("soviet_factory");
    assert_eq!(
        (factory.side, factory.factory, factory.tech_level),
        (Some(Side::Soviet), Some(FactoryGroup::Vehicles), 2)
    );
    let yard = entry("allied_yard");
    assert_eq!(
        (yard.factory, yard.tech_level),
        (Some(FactoryGroup::Buildings), -1)
    );
    // An item without a side and without owner sides.
    let turret = entry("turret");
    assert_eq!(turret.category, Category::Armory);
    assert_eq!(turret.side, None);
    assert!(turret.owner_sides.is_empty());
    assert_eq!(turret.prerequisite_groups, [GroupId(1)]);
    let engineer = entry("engineer");
    assert_eq!((engineer.side, engineer.role), (None, Role::Engineer));
    assert_eq!(engineer.owner_sides, [Side::Allied, Side::Soviet]);
    // Forbidden countries, tech levels, roles.
    let raider = entry("raider");
    assert!(raider.required_countries.is_empty());
    assert_eq!(raider.forbidden_countries, [CountryId(0)]);
    assert_eq!(entry("prototype").tech_level, -1);
    assert_eq!(entry("advanced").tech_level, 7);
    let storm = entry("storm_device");
    assert_eq!(
        (storm.role, storm.cooldown, storm.radius, storm.strength),
        (Role::Storm, 600, 3, 100)
    );
    assert_eq!(entry("allied_mcv").role, Role::Mcv);
    assert_eq!(entry("soviet_mcv").role, Role::Mcv);
    // What the definitions derive from the entries.
    assert_eq!(
        definitions.factory_group(building(&definitions, "soviet_yard")),
        Some(FactoryGroup::Buildings)
    );
    assert_eq!(
        definitions.factory_group(building(&definitions, "allied_power")),
        None
    );
    assert_eq!(definitions.art(item(&definitions, "sniper")), "sniper");
    // Loading a rules file removes an arsenal loaded before, since the arsenal names the
    // countries and groups of the rules.
    let reloaded = as_definitions(load_with(
        File::Rules,
        &File::Rules.fixture(),
        definitions.clone(),
    ));
    assert!(reloaded.rules().is_some());
    assert!(reloaded.arsenal().0.is_empty());
}

#[test]
fn the_skirmish_fixture_loads_with_sides_and_countries() {
    let definitions = definitions();
    let skirmish = as_skirmish(load(File::Skirmish, &File::Skirmish.fixture()));
    let unit = |id| unit(&definitions, id);
    let item = |id| item(&definitions, id);
    assert_eq!(skirmish.starting_credits, 3000);
    assert_eq!(
        skirmish.sides,
        [
            SideStart {
                side: Side::Allied,
                vehicles: vec![unit("tank")],
                infantry: vec![unit("rifleman")],
            },
            SideStart {
                side: Side::Soviet,
                vehicles: vec![unit("heavy")],
                infantry: vec![unit("conscript")],
            },
        ]
    );
    assert_eq!(skirmish.side(Side::Soviet), Some(&skirmish.sides[1]));
    // (400 + 100 + 650 + 50) / 4, as the fixture says.
    assert_eq!(skirmish.average_starting_cost(&definitions), 300);
    // Every faction is a country: its side, its country of the rules file, and its own tech
    // tree as its roster.
    assert_eq!(skirmish.countries(), [0, 1, 2]);
    assert_eq!(skirmish.factions.len(), 3);
    for (index, (id, label, side)) in [
        ("alpha", "Alpha", Side::Allied),
        ("beta", "Beta", Side::Allied),
        ("gamma", "Gamma", Side::Soviet),
    ]
    .into_iter()
    .enumerate()
    {
        let faction = &skirmish.factions[index];
        assert_eq!(
            (faction.id.as_str(), faction.label.as_str(), faction.side),
            (id, label, Some(side))
        );
        let country = definitions
            .rules()
            .and_then(|rules| rules.country_id(id))
            .expect("a country of the rules file");
        assert_eq!(usize::from(country.0), index);
        assert_eq!(faction.country, Some(country));
        assert!(!faction.roster.is_empty(), "{id}");
        assert_eq!(faction.roster, country_tree(&definitions, country), "{id}");
        assert!(faction.buildings.is_empty() && faction.units.is_empty());
    }
    let [alpha, beta, gamma] = &skirmish.factions[..] else {
        panic!("three factions");
    };
    // The specials of the countries, the other side, and the tech level decide the rosters.
    assert!(alpha.offers(item("sniper")) && !alpha.offers(item("raider")));
    assert!(beta.offers(item("raider")) && !beta.offers(item("sniper")));
    assert!(alpha.offers(item("tank")) && !alpha.offers(item("heavy")));
    assert!(gamma.offers(item("heavy")) && gamma.offers(item("militia")));
    assert!(!gamma.offers(item("tank")) && !gamma.offers(item("storm_device")));
    assert!(alpha.offers(item("storm_device")) && alpha.offers(item("engineer")));
    for faction in &skirmish.factions {
        assert!(!faction.offers(item("prototype")) && !faction.offers(item("advanced")));
    }
    // A country starts with the Construction Yard of its side.
    assert!(alpha.starts_with(&definitions, building(&definitions, "allied_yard")));
    assert!(!alpha.starts_with(&definitions, building(&definitions, "soviet_yard")));
    assert!(gamma.starts_with(&definitions, building(&definitions, "soviet_yard")));
    assert_eq!(
        skirmish.maps,
        [MapDef {
            id: "skirmish_40x40".into(),
            starts: vec![CellPos::new(5, 5), CellPos::new(32, 32)],
        }]
    );
}

#[test]
fn the_ai_fixture_loads_with_a_plan_per_side() {
    let definitions = definitions();
    let fixture = File::Ai.fixture();
    let ai = as_ai(load(File::Ai, &fixture));
    let building = |id| building(&definitions, id);
    let unit = |id| unit(&definitions, id);
    assert_eq!(ai.plans.len(), 2);
    let (index, allied) = ai.plan(Some(Side::Allied)).expect("the Allied plan");
    assert_eq!(index, 0);
    assert_eq!(allied.side, Some(Side::Allied));
    assert_eq!(
        allied.build_order,
        [
            "allied_power",
            "allied_refinery",
            "allied_barracks",
            "allied_factory",
            "allied_power",
        ]
        .map(building)
    );
    assert_eq!(allied.power, building("allied_power"));
    assert_eq!(allied.harvester, unit("miner_a"));
    assert_eq!(allied.army, ["tank", "tank", "rifleman"].map(unit));
    let (index, soviet) = ai.plan(Some(Side::Soviet)).expect("the Soviet plan");
    assert_eq!(index, 1);
    assert_eq!(
        soviet.build_order,
        [
            "soviet_power",
            "soviet_refinery",
            "soviet_barracks",
            "soviet_factory",
            "soviet_power",
        ]
        .map(building)
    );
    assert_eq!(soviet.power, building("soviet_power"));
    assert_eq!(soviet.harvester, unit("miner_s"));
    assert_eq!(soviet.army, ["heavy", "conscript", "heavy"].map(unit));
    // The fixture has no plan for factions without a side.
    assert!(ai.plan(None).is_none());
    assert_eq!(ai.difficulties.len(), 2);
    let values = |index: usize| {
        let difficulty = &ai.difficulties[index];
        (
            difficulty.id.as_str(),
            [
                difficulty.think_interval,
                difficulty.harvesters_per_refinery,
                difficulty.max_army,
                difficulty.wave_size,
                difficulty.first_wave,
                difficulty.wave_patience,
                difficulty.defence_radius,
            ],
        )
    };
    assert_eq!(values(0), ("easy", [20, 1, 6, 4, 1200, 1200, 6]));
    assert_eq!(values(1), ("brutal", [10, 2, 12, 5, 600, 800, 8]));
    // A computer player of a country follows the whole plan of its side, and the fixture
    // files agree with each other.
    let skirmish = as_skirmish(load(File::Skirmish, &File::Skirmish.fixture()));
    assert_eq!(
        ai.plan_for(&skirmish.factions[2]),
        Some(AiPlan {
            build_order: soviet.build_order.clone(),
            power: soviet.power,
            harvester: soviet.harvester,
            army: soviet.army.clone(),
        })
    );
    assert_eq!(
        ai.plan_for(&skirmish.factions[1])
            .map(|plan| plan.build_order),
        Some(allied.build_order.clone())
    );
    check_ai_factions(&fixture, &ai, &skirmish, &definitions)
        .unwrap_or_else(|error| panic!("{error}"));
}

#[test]
fn the_limits_of_every_scalar_rule_are_accepted() {
    for (group, fields) in SCALAR_GROUPS {
        for &(field, min, max) in fields {
            for (at_max, value) in [(false, min), (true, max)] {
                let end = if at_max { "max" } else { "min" };
                let path = scalar_variant(
                    &format!("rules_limit_{group}_{field}_{end}"),
                    group,
                    field,
                    Some(&value.to_string()),
                );
                let result = load(File::Rules, &path);
                if CROSS_CHECKED.contains(&(group, field, at_max)) {
                    // Within its own range, but not beside the other values of the fixture
                    // (`the_cross_checks_of_the_rules_file_are_enforced`).
                    let error = result.expect_err("a cross check forbids the value");
                    assert!(
                        matches!(error.kind, DataErrorKind::InvalidValue { .. })
                            && error.field.starts_with(&format!("{group}.")),
                        "{group}.{field} = {value}: {error}"
                    );
                    continue;
                }
                let definitions = as_definitions(result);
                let rules = definitions.rules().expect("the rules are loaded");
                let loaded = scalars(rules, group)
                    .into_iter()
                    .find(|&(name, _)| name == field)
                    .map(|(_, loaded)| i64::from(loaded));
                assert_eq!(loaded, Some(value), "{group}.{field}");
            }
        }
    }
    // The entries of the lists, at both ends.
    let text = File::Rules.text();
    let n = "rules_list_limits";
    let text = line_edit(
        n,
        &text,
        MONEY,
        &[("share: 20", "share: 0"), ("value: 2000", "value: 0")],
    );
    let text = line_edit(
        n,
        &text,
        ARMOUR,
        &[
            ("share: 10", "share: 1000"),
            ("value: 1500", "value: 1000000"),
        ],
    );
    let text = line_edit(
        n,
        &text,
        EASY,
        &[
            ("rof_pm: 1200", "rof_pm: 1"),
            ("build_time_pm: 1000", "build_time_pm: 1"),
            ("team_delay: 4667", "team_delay: 1"),
            ("allied_base_defences: 6", "allied_base_defences: 0"),
            ("soviet_base_defences: 6", "soviet_base_defences: 0"),
        ],
    );
    let text = line_edit(
        n,
        &text,
        BRUTAL,
        &[
            ("rof_pm: 800", "rof_pm: 10000"),
            ("build_time_pm: 800", "build_time_pm: 10000"),
            ("credits_bonus_percent: 400", "credits_bonus_percent: 10000"),
            ("virtual_purifiers: 4", "virtual_purifiers: 64"),
            ("team_delay: 2667", "team_delay: 72000"),
            ("allied_base_defences: 25", "allied_base_defences: 255"),
            ("soviet_base_defences: 25", "soviet_base_defences: 255"),
        ],
    );
    let text = line_edit(
        n,
        &text,
        ALLIED_RULES,
        &[
            ("survivor_divisor: 500", "survivor_divisor: 1"),
            ("paradrop_count: 6", "paradrop_count: 0"),
        ],
    );
    let text = line_edit(
        n,
        &text,
        SOVIET_RULES,
        &[
            ("survivor_divisor: 250", "survivor_divisor: 1000000"),
            ("paradrop_count: 9", "paradrop_count: 64"),
        ],
    );
    let path = temp_file("rules_list_limits.ron", text.as_bytes());
    let definitions = as_definitions(load(File::Rules, &path));
    let rules = definitions.rules().expect("the rules are loaded");
    assert_eq!((rules.powerups[0].share, rules.powerups[0].value), (0, 0));
    assert_eq!(
        (rules.powerups[1].share, rules.powerups[1].value),
        (1000, 1_000_000)
    );
    let easy = &rules.difficulties[0];
    assert_eq!(
        [
            easy.rof_pm,
            easy.build_time_pm,
            easy.credits_bonus_percent,
            easy.virtual_purifiers,
            easy.team_delay,
            easy.allied_base_defences,
            easy.soviet_base_defences,
        ],
        [1, 1, 0, 0, 1, 0, 0]
    );
    let brutal = &rules.difficulties[1];
    assert_eq!(
        [
            brutal.rof_pm,
            brutal.build_time_pm,
            brutal.credits_bonus_percent,
            brutal.virtual_purifiers,
            brutal.team_delay,
            brutal.allied_base_defences,
            brutal.soviet_base_defences,
        ],
        [10_000, 10_000, 10_000, 64, 72_000, 255, 255]
    );
    assert_eq!(
        (
            rules.sides[0].survivor_divisor,
            rules.sides[0].paradrop_count
        ),
        (1, 0)
    );
    assert_eq!(
        (
            rules.sides[1].survivor_divisor,
            rules.sides[1].paradrop_count
        ),
        (1_000_000, 64)
    );
}

#[test]
fn arsenal_limits_are_accepted_at_both_ends() {
    let longest = "d".repeat(MAX_STRING_BYTES);
    for (index, (cooldown, radius, strength, capacity, tech_level, description)) in [
        (0, 0, 0, 0, -1, "d"),
        (72_000, 32, 10_000, 16, MAX_TECH_LEVEL, longest.as_str()),
    ]
    .into_iter()
    .enumerate()
    {
        let path = line_variant(
            File::Arsenal,
            &format!("arsenal_limits_{index}"),
            SNIPER,
            &[
                ("cooldown: 100", &format!("cooldown: {cooldown}")),
                ("radius: 2", &format!("radius: {radius}")),
                ("strength: 20", &format!("strength: {strength}")),
                ("capacity: 0", &format!("capacity: {capacity}")),
                ("tech_level: 1", &format!("tech_level: {tech_level}")),
                (
                    r#"description: "Fixture item.""#,
                    &format!("description: \"{description}\""),
                ),
            ],
        );
        let definitions = as_definitions(load(File::Arsenal, &path));
        let sniper = definitions
            .metadata(item(&definitions, "sniper"))
            .expect("the sniper has an entry");
        assert_eq!(
            (
                i64::from(sniper.cooldown),
                i64::from(sniper.radius),
                sniper.strength,
                sniper.capacity,
                sniper.tech_level,
                sniper.description.as_str(),
            ),
            (
                cooldown,
                radius,
                strength,
                capacity,
                tech_level,
                description
            )
        );
    }
    // A superweapon needs a cooldown and a radius above 0; 1 is enough.
    let path = line_variant(
        File::Arsenal,
        "arsenal_limits_superweapon",
        STORM,
        &[("cooldown: 600", "cooldown: 1"), ("radius: 3", "radius: 1")],
    );
    let definitions = as_definitions(load(File::Arsenal, &path));
    let storm = definitions
        .metadata(item(&definitions, "storm_device"))
        .expect("the storm device has an entry");
    assert_eq!((storm.cooldown, storm.radius), (1, 1));
}

#[test]
fn skirmish_and_ai_limits_are_accepted_at_both_ends() {
    let label = "L".repeat(MAX_LABEL_LEN);
    let text = File::Skirmish.text();
    let n = "skirmish_limits";
    let text = edit(
        n,
        &text,
        "starting_credits: 3000",
        "starting_credits: 1000000",
    );
    let text = line_edit(
        n,
        &text,
        ALPHA_FACTION,
        &[(r#"label: "Alpha""#, &format!("label: \"{label}\""))],
    );
    let text = line_edit(
        n,
        &text,
        BETA_FACTION,
        &[(r#"label: "Beta""#, r#"label: "B""#)],
    );
    let text = line_edit(
        n,
        &text,
        MAP,
        &[(
            "(x: 5, y: 5), (x: 32, y: 32)",
            "(x: 0, y: 0), (x: 255, y: 255)",
        )],
    );
    let path = temp_file("skirmish_limits.ron", text.as_bytes());
    let loaded = as_skirmish(load(File::Skirmish, &path));
    assert_eq!(loaded.starting_credits, MAX_STARTING_CREDITS);
    assert_eq!(loaded.factions[0].label, label);
    assert_eq!(loaded.factions[1].label, "B");
    assert_eq!(
        loaded.maps[0].starts,
        [CellPos::new(0, 0), CellPos::new(255, 255)]
    );
    let zero = variant(
        File::Skirmish,
        "skirmish_zero_credits",
        &[("starting_credits: 3000", "starting_credits: 0")],
    );
    assert_eq!(as_skirmish(load(File::Skirmish, &zero)).starting_credits, 0);
    // The offsets and the group size of a faction without a side.
    let path = sideless_variant(
        "skirmish_sideless_limits",
        &[
            (
                r#"(building: "allied_yard", x: 0, y: 0)"#,
                r#"(building: "allied_yard", x: -32, y: 32)"#,
            ),
            (
                r#"(unit: "tank", x: 4, y: 1, count: 2)"#,
                r#"(unit: "tank", x: 32, y: -32, count: 64), (unit: "rifleman", x: 0, y: 0, count: 1)"#,
            ),
        ],
    );
    let loaded = as_skirmish(load(File::Skirmish, &path));
    assert_eq!(
        loaded.factions[0].buildings[0].offset,
        (-MAX_START_OFFSET, MAX_START_OFFSET)
    );
    assert_eq!(
        loaded.factions[0].units[0].offset,
        (MAX_START_OFFSET, -MAX_START_OFFSET)
    );
    assert_eq!(loaded.factions[0].units[0].count, MAX_GROUP_UNITS);
    assert_eq!(loaded.factions[0].units[1].count, 1);

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
    let loaded = as_ai(load(File::Ai, &ai));
    let (easy, brutal) = (&loaded.difficulties[0], &loaded.difficulties[1]);
    assert_eq!(
        (easy.think_interval, brutal.think_interval),
        (1, MAX_THINK_INTERVAL)
    );
    assert_eq!(
        (easy.harvesters_per_refinery, brutal.harvesters_per_refinery),
        (0, MAX_HARVESTERS_PER_REFINERY)
    );
    assert_eq!((easy.max_army, easy.wave_size), (1, 1));
    assert_eq!(
        (brutal.max_army, brutal.wave_size),
        (MAX_AI_ARMY, MAX_AI_ARMY)
    );
    assert_eq!((easy.first_wave, brutal.first_wave), (0, 72_000));
    assert_eq!((easy.wave_patience, brutal.wave_patience), (1, 72_000));
    assert_eq!(
        (easy.defence_radius, brutal.defence_radius),
        (0, MAX_DEFENCE_RADIUS)
    );
    // The longest build order and the largest army composition.
    let order = format!(
        r#"build_order: ["allied_power", "allied_refinery", "allied_barracks", "allied_factory", {}],"#,
        vec![r#""allied_power""#; MAX_BUILD_ORDER - 4].join(", ")
    );
    let army = format!("army: {},", list(r#""tank""#, MAX_ARMY_ENTRIES));
    let path = variant(
        File::Ai,
        "ai_longest_lists",
        &[(ALLIED_ORDER, &order), (ALLIED_ARMY, &army)],
    );
    let loaded = as_ai(load(File::Ai, &path));
    assert_eq!(loaded.plans[0].build_order.len(), MAX_BUILD_ORDER);
    assert_eq!(loaded.plans[0].army.len(), MAX_ARMY_ENTRIES);
}

#[test]
fn a_faction_without_a_side_loads_with_its_roster_and_starting_lists() {
    let definitions = definitions();
    let loaded = as_skirmish(load(
        File::Skirmish,
        &sideless_variant("skirmish_sideless", &[]),
    ));
    let faction = &loaded.factions[0];
    assert_eq!((faction.side, faction.country), (None, None));
    assert_eq!(
        faction.roster,
        BTreeSet::from([
            item(&definitions, "allied_power"),
            item(&definitions, "tank")
        ])
    );
    assert_eq!(
        faction.buildings,
        [StartingBuilding {
            kind: building(&definitions, "allied_yard"),
            offset: (0, 0),
        }]
    );
    assert_eq!(
        faction.units,
        [StartingGroup {
            kind: unit(&definitions, "tank"),
            offset: (4, 1),
            count: 2,
        }]
    );
    // The other factions are still countries.
    assert_eq!(loaded.countries(), [1, 2]);
    assert_eq!(loaded.factions[1].country, Some(CountryId(1)));
    // The id of a faction without a side need not be a country, and its roster may be empty.
    let path = sideless_variant(
        "skirmish_sideless_other_id",
        &[
            (r#"id: "alpha""#, r#"id: "outlaws""#),
            (
                r#"roster_buildings: ["allied_power"]"#,
                "roster_buildings: []",
            ),
            (r#"roster_units: ["tank"]"#, "roster_units: []"),
            (
                r#"units: [(unit: "tank", x: 4, y: 1, count: 2)]"#,
                "units: []",
            ),
        ],
    );
    let loaded = as_skirmish(load(File::Skirmish, &path));
    let faction = &loaded.factions[0];
    assert_eq!(faction.id, "outlaws");
    assert_eq!((faction.side, faction.country), (None, None));
    assert!(faction.roster.is_empty() && faction.units.is_empty());
    assert_eq!(faction.buildings.len(), 1);
    // Without a rules file there are no sides, and a file of such factions alone loads.
    let text = File::Skirmish.text();
    let n = "skirmish_without_sides";
    let text = repeat_line(n, &text, ALLIED_START, 0);
    let text = repeat_line(n, &text, SOVIET_START, 0);
    let text = repeat_line(n, &text, BETA_FACTION, 0);
    let text = repeat_line(n, &text, GAMMA_FACTION, 0);
    let text = edit(
        n,
        &text,
        ALPHA,
        &edit(
            n,
            &edit(
                n,
                SIDELESS,
                r#"roster_buildings: ["allied_power"]"#,
                "roster_buildings: []",
            ),
            r#"roster_units: ["tank"]"#,
            "roster_units: []",
        ),
    );
    let path = temp_file("skirmish_without_sides.ron", text.as_bytes());
    let loaded = as_skirmish(load_with(File::Skirmish, &path, base_definitions()));
    assert!(loaded.sides.is_empty() && loaded.countries().is_empty());
    assert_eq!(loaded.factions.len(), 1);
}

#[test]
fn a_plan_may_list_every_building_of_the_tree_of_its_side() {
    let definitions = definitions();
    // The radar, the superweapon building behind it, and the turret, which has no owner sides
    // and needs any barracks; the Soviet plan lists the turret too, and the militia.
    let path = variant(
        File::Ai,
        "ai_whole_trees",
        &[
            (
                r#""allied_factory", "allied_power"],"#,
                r#""allied_factory", "allied_power", "allied_radar", "storm_device", "turret"],"#,
            ),
            (ALLIED_ARMY, r#"army: ["tank", "rifleman"],"#),
            (
                r#""soviet_factory", "soviet_power"],"#,
                r#""soviet_factory", "soviet_power", "soviet_radar", "turret"],"#,
            ),
            (SOVIET_ARMY, r#"army: ["heavy", "militia", "conscript"],"#),
        ],
    );
    let ai = as_ai(load(File::Ai, &path));
    let (_, allied) = ai.plan(Some(Side::Allied)).expect("the Allied plan");
    assert_eq!(allied.build_order.len(), 8);
    assert_eq!(
        allied.build_order[5..],
        ["allied_radar", "storm_device", "turret"].map(|id| building(&definitions, id))
    );
    let (_, soviet) = ai.plan(Some(Side::Soviet)).expect("the Soviet plan");
    assert_eq!(
        soviet.build_order[5..],
        ["soviet_radar", "turret"].map(|id| building(&definitions, id))
    );
    assert_eq!(
        soviet.army,
        ["heavy", "militia", "conscript"].map(|id| unit(&definitions, id))
    );
    let skirmish = as_skirmish(load(File::Skirmish, &File::Skirmish.fixture()));
    check_ai_factions(&path, &ai, &skirmish, &definitions)
        .unwrap_or_else(|error| panic!("{error}"));
}

// --- Negative: structure --------------------------------------------------------------------

#[test]
fn every_field_of_the_rules_file_is_required() {
    // The fields of the file.
    let text = File::Rules.text();
    let mut fields = vec!["schema_version"];
    fields.extend(SCALAR_GROUPS[..9].iter().map(|&(group, _)| group));
    fields.extend([
        "powerups",
        "dialog",
        "difficulty",
        "prerequisite_groups",
        "sides",
        "countries",
    ]);
    assert_eq!(fields.len(), 16);
    for field in fields {
        let name = format!("rules_missing_{field}");
        let path = temp_file(
            &format!("{name}.ron"),
            without_field(&name, &text, field).as_bytes(),
        );
        assert_rejected(File::Rules, &path, field, DataErrorKind::MissingField);
    }
    // Every scalar of every group.
    for (group, fields) in SCALAR_GROUPS {
        for &(field, _, _) in fields {
            let path = scalar_variant(
                &format!("rules_missing_{group}_{field}"),
                group,
                field,
                None,
            );
            assert_rejected(
                File::Rules,
                &path,
                &format!("{group}.{field}"),
                DataErrorKind::MissingField,
            );
        }
    }
    // Every field of the entries of the lists.
    let entries: [(&str, &str, Edits); 5] = [
        (
            MONEY,
            "powerups[0]",
            &[
                ("id", r#"id: "money", "#),
                ("share", "share: 20, "),
                ("value", ", value: 2000"),
            ],
        ),
        (
            EASY,
            "difficulty[0]",
            &[
                ("id", r#"id: "easy", "#),
                ("rof_pm", "rof_pm: 1200, "),
                ("build_time_pm", "build_time_pm: 1000, "),
                ("credits_bonus_percent", "credits_bonus_percent: 0, "),
                ("virtual_purifiers", "virtual_purifiers: 0, "),
                ("team_delay", "team_delay: 4667, "),
                ("allied_base_defences", "allied_base_defences: 6, "),
                ("soviet_base_defences", ", soviet_base_defences: 6"),
            ],
        ),
        (
            POWER_GROUP,
            "prerequisite_groups[0]",
            &[
                ("id", r#"id: "power", "#),
                (
                    "buildings",
                    r#", buildings: ["allied_power", "soviet_power"]"#,
                ),
            ],
        ),
        (
            ALLIED_RULES,
            "sides[0]",
            &[
                ("id", r#"id: "allied", "#),
                ("base_unit", r#"base_unit: "allied_mcv", "#),
                ("construction_yard", r#"construction_yard: "allied_yard", "#),
                ("crew", r#"crew: "rifleman", "#),
                ("survivor_divisor", "survivor_divisor: 500, "),
                ("disguise", r#"disguise: "rifleman", "#),
                ("paradrop_infantry", r#"paradrop_infantry: "rifleman", "#),
                ("paradrop_count", ", paradrop_count: 6"),
            ],
        ),
        (
            BETA_COUNTRY,
            "countries[1]",
            &[("id", r#"id: "beta", "#), ("side", r#", side: "allied""#)],
        ),
    ];
    for (line, entry, fields) in entries {
        for &(field, from) in fields {
            let at = format!("{entry}.{field}");
            let path = line_variant(
                File::Rules,
                &format!("rules_missing_{at}"),
                line,
                &[(from, "")],
            );
            assert_rejected(File::Rules, &path, &at, DataErrorKind::MissingField);
        }
    }
}

#[test]
fn every_field_of_the_arsenal_file_is_required() {
    let path = variant(
        File::Arsenal,
        "arsenal_missing_schema_version",
        &[("(schema_version: 2, entries: [", "(entries: [")],
    );
    assert_rejected(
        File::Arsenal,
        &path,
        "schema_version",
        DataErrorKind::MissingField,
    );
    let path = temp_file("arsenal_missing_entries.ron", b"(schema_version: 2)\n");
    assert_rejected(File::Arsenal, &path, "entries", DataErrorKind::MissingField);
    // Every field of an entry: those of schema version 1 and the eight of version 2.
    let fields = [
        ("id", r#"id: "sniper", "#),
        ("label", r#"label: "Sniper", "#),
        ("description", r#"description: "Fixture item.", "#),
        ("category", r#"category: "infantry", "#),
        ("domain", r#"domain: "land", "#),
        ("role", r#"role: "combat", "#),
        ("cooldown", "cooldown: 100, "),
        ("radius", "radius: 2, "),
        ("strength", "strength: 20, "),
        ("capacity", "capacity: 0, "),
        ("art", r#"art: "sniper", "#),
        ("side", r#"side: "allied", "#),
        ("factory", r#"factory: "none", "#),
        ("owner_sides", r#"owner_sides: ["allied"], "#),
        ("required_countries", r#"required_countries: ["alpha"], "#),
        ("forbidden_countries", "forbidden_countries: [], "),
        ("tech_level", "tech_level: 1, "),
        ("prerequisite_groups", r#", prerequisite_groups: ["radar"]"#),
    ];
    for (field, from) in fields {
        let path = line_variant(
            File::Arsenal,
            &format!("arsenal_missing_{field}"),
            SNIPER,
            &[(from, "")],
        );
        assert_rejected(
            File::Arsenal,
            &path,
            &format!("entries[19].{field}"),
            DataErrorKind::MissingField,
        );
    }
}

#[test]
fn every_field_of_the_skirmish_file_is_required() {
    let text = File::Skirmish.text();
    for field in [
        "schema_version",
        "starting_credits",
        "sides",
        "factions",
        "maps",
    ] {
        let name = format!("skirmish_missing_{field}");
        let path = temp_file(
            &format!("{name}.ron"),
            without_field(&name, &text, field).as_bytes(),
        );
        assert_rejected(File::Skirmish, &path, field, DataErrorKind::MissingField);
    }
    // The entries of the lists: a side, a faction, a map and a start position, and the starting
    // buildings and groups of a faction (the schema is checked before the rule that a country
    // has none).
    let cases = [
        (ALLIED_START, r#"side: "allied", "#, "", "sides[0].side"),
        (
            ALLIED_START,
            r#"starting_vehicles: ["tank"], "#,
            "",
            "sides[0].starting_vehicles",
        ),
        (
            SOVIET_START,
            r#", starting_infantry: ["conscript"]"#,
            "",
            "sides[1].starting_infantry",
        ),
        (ALPHA_FACTION, r#"id: "alpha", "#, "", "factions[0].id"),
        (
            ALPHA_FACTION,
            r#"label: "Alpha", "#,
            "",
            "factions[0].label",
        ),
        (ALPHA_FACTION, r#"side: "allied", "#, "", "factions[0].side"),
        (
            ALPHA_FACTION,
            "roster_buildings: [], ",
            "",
            "factions[0].roster_buildings",
        ),
        (
            ALPHA_FACTION,
            "roster_units: [], ",
            "",
            "factions[0].roster_units",
        ),
        (
            GAMMA_FACTION,
            "roster_units: [], buildings: [], units: []",
            "roster_units: [], units: []",
            "factions[2].buildings",
        ),
        (GAMMA_FACTION, ", units: []", "", "factions[2].units"),
        (
            ALPHA_FACTION,
            "roster_units: [], buildings: []",
            r#"roster_units: [], buildings: [(x: 0, y: 0)]"#,
            "factions[0].buildings[0].building",
        ),
        (
            ALPHA_FACTION,
            "roster_units: [], buildings: []",
            r#"roster_units: [], buildings: [(building: "allied_yard", y: 0)]"#,
            "factions[0].buildings[0].x",
        ),
        (
            ALPHA_FACTION,
            "roster_units: [], buildings: []",
            r#"roster_units: [], buildings: [(building: "allied_yard", x: 0)]"#,
            "factions[0].buildings[0].y",
        ),
        (
            ALPHA_FACTION,
            ", units: []",
            ", units: [(x: 4, y: 1, count: 2)]",
            "factions[0].units[0].unit",
        ),
        (
            ALPHA_FACTION,
            ", units: []",
            r#", units: [(unit: "tank", y: 1, count: 2)]"#,
            "factions[0].units[0].x",
        ),
        (
            ALPHA_FACTION,
            ", units: []",
            r#", units: [(unit: "tank", x: 4, count: 2)]"#,
            "factions[0].units[0].y",
        ),
        (
            ALPHA_FACTION,
            ", units: []",
            r#", units: [(unit: "tank", x: 4, y: 1)]"#,
            "factions[0].units[0].count",
        ),
        (MAP, r#"id: "skirmish_40x40", "#, "", "maps[0].id"),
        (
            MAP,
            ", starts: [(x: 5, y: 5), (x: 32, y: 32)]",
            "",
            "maps[0].starts",
        ),
        (MAP, "(x: 5, y: 5)", "(y: 5)", "maps[0].starts[0].x"),
        (MAP, "(x: 32, y: 32)", "(x: 32)", "maps[0].starts[1].y"),
    ];
    for (line, from, to, field) in cases {
        let path = line_variant(
            File::Skirmish,
            &format!("skirmish_missing_{field}"),
            line,
            &[(from, to)],
        );
        assert_rejected(File::Skirmish, &path, field, DataErrorKind::MissingField);
    }
}

#[test]
fn every_field_of_the_ai_file_is_required() {
    let text = File::Ai.text();
    for field in ["schema_version", "plans", "difficulties"] {
        let name = format!("ai_missing_{field}");
        let path = temp_file(
            &format!("{name}.ron"),
            without_field(&name, &text, field).as_bytes(),
        );
        assert_rejected(File::Ai, &path, field, DataErrorKind::MissingField);
    }
    let allied_order = format!("            {ALLIED_ORDER}\n");
    let soviet_army = format!("            {SOVIET_ARMY}\n");
    let fields = [
        ("            side: \"allied\",\n", "plans[0].side"),
        (allied_order.as_str(), "plans[0].build_order"),
        ("            power: \"allied_power\",\n", "plans[0].power"),
        (
            "            harvester: \"miner_s\",\n",
            "plans[1].harvester",
        ),
        (soviet_army.as_str(), "plans[1].army"),
        ("            id: \"easy\",\n", "difficulties[0].id"),
        (
            "            think_interval: 20,\n",
            "difficulties[0].think_interval",
        ),
        (
            "            harvesters_per_refinery: 1,\n",
            "difficulties[0].harvesters_per_refinery",
        ),
        ("            max_army: 6,\n", "difficulties[0].max_army"),
        ("            wave_size: 4,\n", "difficulties[0].wave_size"),
        (
            "            first_wave: 600,\n",
            "difficulties[1].first_wave",
        ),
        (
            "            wave_patience: 800,\n",
            "difficulties[1].wave_patience",
        ),
        (
            "            defence_radius: 8,\n",
            "difficulties[1].defence_radius",
        ),
    ];
    for (from, field) in fields {
        let path = variant(File::Ai, &format!("ai_missing_{field}"), &[(from, "")]);
        assert_rejected(File::Ai, &path, field, DataErrorKind::MissingField);
    }
}

#[test]
fn unknown_and_duplicate_fields_are_rejected_in_every_file() {
    let sniper_tech = r#"required_countries: ["alpha"], forbidden_countries: [], tech_level: 1,"#;
    let sniper_veteran =
        r#"required_countries: ["alpha"], forbidden_countries: [], tech_level: 1, veteran: 1,"#;
    let cases = [
        // The rules file: the file's struct, a scalar group, an entry of a list.
        (
            File::Rules,
            "    schema_version: 1,",
            "    schema_version: 1,\n    seed: 4,",
            "seed",
            DataErrorKind::UnknownField,
        ),
        (
            File::Rules,
            "        build_ticks_per_1000: 840,",
            "        build_ticks_per_1000: 840,\n        cheat: 1,",
            "production.cheat",
            DataErrorKind::UnknownField,
        ),
        (
            File::Rules,
            r#"(id: "alpha", side: "allied")"#,
            r#"(id: "alpha", side: "allied", capital: "a")"#,
            "countries[0].capital",
            DataErrorKind::UnknownField,
        ),
        (
            File::Rules,
            r#"(id: "money", share: 20, value: 2000)"#,
            r#"(id: "money", share: 20, value: 2000, weight: 1)"#,
            "powerups[0].weight",
            DataErrorKind::UnknownField,
        ),
        (
            File::Rules,
            "    schema_version: 1,",
            "    schema_version: 1,\n    schema_version: 1,",
            "schema_version",
            DataErrorKind::DuplicateField,
        ),
        (
            File::Rules,
            "        build_ticks_per_1000: 840,",
            "        build_ticks_per_1000: 840,\n        build_ticks_per_1000: 841,",
            "production.build_ticks_per_1000",
            DataErrorKind::DuplicateField,
        ),
        (
            File::Rules,
            r#"(id: "alpha", side: "allied")"#,
            r#"(id: "alpha", side: "allied", side: "allied")"#,
            "countries[0].side",
            DataErrorKind::DuplicateField,
        ),
        // The arsenal.
        (
            File::Arsenal,
            "(schema_version: 2, entries: [",
            "(schema_version: 2, mods: 1, entries: [",
            "mods",
            DataErrorKind::UnknownField,
        ),
        (
            File::Arsenal,
            sniper_tech,
            sniper_veteran,
            "entries[19].veteran",
            DataErrorKind::UnknownField,
        ),
        (
            File::Arsenal,
            r#"art: "sniper","#,
            r#"art: "sniper", art: "sniper","#,
            "entries[19].art",
            DataErrorKind::DuplicateField,
        ),
        (
            File::Arsenal,
            "(schema_version: 2, entries: [",
            "(schema_version: 2, schema_version: 2, entries: [",
            "schema_version",
            DataErrorKind::DuplicateField,
        ),
        // The skirmish file. The base unit of a side belongs to the rules file.
        (
            File::Skirmish,
            "    starting_credits: 3000,",
            "    starting_credits: 3000,\n    seed: 4,",
            "seed",
            DataErrorKind::UnknownField,
        ),
        (
            File::Skirmish,
            r#"(side: "allied", starting_vehicles"#,
            r#"(side: "allied", base_unit: "allied_mcv", starting_vehicles"#,
            "sides[0].base_unit",
            DataErrorKind::UnknownField,
        ),
        (
            File::Skirmish,
            r#"label: "Alpha", "#,
            r#"label: "Alpha", colour: "red", "#,
            "factions[0].colour",
            DataErrorKind::UnknownField,
        ),
        (
            File::Skirmish,
            "    starting_credits: 3000,",
            "    starting_credits: 3000,\n    starting_credits: 3000,",
            "starting_credits",
            DataErrorKind::DuplicateField,
        ),
        (
            File::Skirmish,
            r#"label: "Alpha", "#,
            r#"label: "Alpha", label: "Alpha", "#,
            "factions[0].label",
            DataErrorKind::DuplicateField,
        ),
        (
            File::Skirmish,
            r#"(side: "allied", starting_vehicles"#,
            r#"(side: "allied", side: "allied", starting_vehicles"#,
            "sides[0].side",
            DataErrorKind::DuplicateField,
        ),
        // The AI file. A build order at the top level is the layout of schema version 1.
        (
            File::Ai,
            "    schema_version: 2,",
            "    schema_version: 2,\n    build_order: [],",
            "build_order",
            DataErrorKind::UnknownField,
        ),
        (
            File::Ai,
            "            power: \"allied_power\",",
            "            power: \"allied_power\",\n            tech: 1,",
            "plans[0].tech",
            DataErrorKind::UnknownField,
        ),
        (
            File::Ai,
            "            max_army: 6,",
            "            max_army: 6,\n            cheat: 1,",
            "difficulties[0].cheat",
            DataErrorKind::UnknownField,
        ),
        (
            File::Ai,
            "            max_army: 6,",
            "            max_army: 6,\n            max_army: 7,",
            "difficulties[0].max_army",
            DataErrorKind::DuplicateField,
        ),
        (
            File::Ai,
            "            side: \"soviet\",",
            "            side: \"soviet\",\n            side: \"soviet\",",
            "plans[1].side",
            DataErrorKind::DuplicateField,
        ),
    ];
    for (index, (file, from, to, field, kind)) in cases.into_iter().enumerate() {
        let path = variant(file, &format!("fields_{index}"), &[(from, to)]);
        assert_rejected(file, &path, field, kind);
    }
}

#[test]
fn values_of_the_wrong_kind_are_syntax_errors_in_every_file() {
    let cases = [
        // A string, a float and a boolean where an integer is expected (yes/no keys are 1 or
        // 0); an integer where a string is expected; a string where a list is expected; a
        // list where a struct is expected.
        (
            File::Rules,
            "build_ticks_per_1000: 840,",
            "build_ticks_per_1000: \"840\",",
            "production.build_ticks_per_1000",
        ),
        (
            File::Rules,
            "build_ticks_per_1000: 840,",
            "build_ticks_per_1000: 840.5,",
            "production.build_ticks_per_1000",
        ),
        (
            File::Rules,
            "initial_veteran: 0,",
            "initial_veteran: false,",
            "veterancy.initial_veteran",
        ),
        (
            File::Rules,
            r#"(id: "alpha", side: "allied")"#,
            r#"(id: 5, side: "allied")"#,
            "countries[0].id",
        ),
        (
            File::Rules,
            r#"(id: "power", buildings: ["allied_power", "soviet_power"])"#,
            r#"(id: "power", buildings: "allied_power")"#,
            "prerequisite_groups[0].buildings",
        ),
        (File::Rules, "    crew: (", "    crew: [", "crew"),
        (
            File::Arsenal,
            r#"strength: 20, capacity: 0, art: "sniper""#,
            r#"strength: "20", capacity: 0, art: "sniper""#,
            "entries[19].strength",
        ),
        (
            File::Arsenal,
            r#"art: "sniper", side: "allied""#,
            r#"art: "sniper", side: 1"#,
            "entries[19].side",
        ),
        (
            File::Arsenal,
            r#"owner_sides: ["allied"], required_countries: ["alpha"]"#,
            r#"owner_sides: "allied", required_countries: ["alpha"]"#,
            "entries[19].owner_sides",
        ),
        (
            File::Arsenal,
            r#"forbidden_countries: [], tech_level: 1, prerequisite_groups: ["radar"]"#,
            r#"forbidden_countries: [], tech_level: "1", prerequisite_groups: ["radar"]"#,
            "entries[19].tech_level",
        ),
        (
            File::Skirmish,
            "starting_credits: 3000,",
            "starting_credits: \"3000\",",
            "starting_credits",
        ),
        (
            File::Skirmish,
            r#"label: "Alpha""#,
            "label: 7",
            "factions[0].label",
        ),
        (
            File::Skirmish,
            r#"starting_vehicles: ["tank"]"#,
            r#"starting_vehicles: "tank""#,
            "sides[0].starting_vehicles",
        ),
        (
            File::Skirmish,
            "(x: 5, y: 5)",
            "(x: \"5\", y: 5)",
            "maps[0].starts[0].x",
        ),
        (
            File::Ai,
            "            power: \"allied_power\",",
            "            power: 3,",
            "plans[0].power",
        ),
        (
            File::Ai,
            "            side: \"allied\",",
            "            side: allied,",
            "plans[0].side",
        ),
        (
            File::Ai,
            "max_army: 6,",
            "max_army: \"6\",",
            "difficulties[0].max_army",
        ),
        (File::Ai, ALLIED_ARMY, "army: \"tank\",", "plans[0].army"),
    ];
    for (index, (file, from, to, field)) in cases.into_iter().enumerate() {
        let path = variant(file, &format!("kind_{index}"), &[(from, to)]);
        let error = load(file, &path).expect_err("kind");
        assert_eq!(error.field, field, "{error}");
        assert_eq!(error.file, path);
        assert!(
            matches!(error.kind, DataErrorKind::Syntax { .. }),
            "{error}"
        );
    }
}

#[test]
fn other_schema_versions_are_rejected_in_every_file() {
    // The rules file is new at version 1. The arsenal is at 2 since RA1 (1: M9, without sides,
    // countries and tech levels), the skirmish file at 3 (2: M7, without sides; 1: M6, without
    // rosters), the AI file at 2 (1: one plan at the top level); a file of an earlier version
    // and of any later version is rejected as such.
    let cases = [
        (File::Rules, "schema_version: 1,", 0, 1),
        (File::Rules, "schema_version: 1,", 2, 1),
        (File::Arsenal, "schema_version: 2,", 1, 2),
        (File::Arsenal, "schema_version: 2,", 3, 2),
        (File::Skirmish, "schema_version: 3,", 2, 3),
        (File::Skirmish, "schema_version: 3,", 1, 3),
        (File::Skirmish, "schema_version: 3,", 4, 3),
        (File::Ai, "schema_version: 2,", 1, 2),
        (File::Ai, "schema_version: 2,", 3, 2),
    ];
    for (index, (file, from, found, expected)) in cases.into_iter().enumerate() {
        let path = variant(
            file,
            &format!("version_{index}"),
            &[(from, &format!("schema_version: {found},"))],
        );
        assert_rejected(
            file,
            &path,
            "schema_version",
            DataErrorKind::UnsupportedSchemaVersion { found, expected },
        );
    }
}

#[test]
fn lists_longer_than_their_schema_allows_are_rejected() {
    let n = "too_many";
    let rules = File::Rules.text();
    let arsenal = File::Arsenal.text();
    let skirmish = File::Skirmish.text();
    let ai = File::Ai.text();
    let yard = r#"(building: "allied_yard", x: 0, y: 0)"#;
    let group = r#"(unit: "tank", x: 4, y: 1, count: 2)"#;
    let cases: Vec<(File, String, &str, usize)> = vec![
        // The rules file: every list of the file, and the buildings of a group.
        (
            File::Rules,
            repeat_line(n, &rules, SOVIET_RULES, 2),
            "sides",
            2,
        ),
        (
            File::Rules,
            repeat_line(n, &rules, GAMMA_COUNTRY, MAX_COUNTRIES - 1),
            "countries",
            MAX_COUNTRIES,
        ),
        (
            File::Rules,
            repeat_line(n, &rules, RADAR_GROUP, MAX_PREREQUISITE_GROUPS - 1),
            "prerequisite_groups",
            MAX_PREREQUISITE_GROUPS,
        ),
        (
            File::Rules,
            repeat_line(n, &rules, ARMOUR, MAX_POWERUPS),
            "powerups",
            MAX_POWERUPS,
        ),
        (
            File::Rules,
            repeat_line(n, &rules, BRUTAL, MAX_RULES_DIFFICULTIES),
            "difficulty",
            MAX_RULES_DIFFICULTIES,
        ),
        (
            File::Rules,
            line_edit(
                n,
                &rules,
                POWER_GROUP,
                &[(
                    r#"["allied_power", "soviet_power"]"#,
                    &list(r#""allied_power""#, MAX_GROUP_BUILDINGS + 1),
                )],
            ),
            "prerequisite_groups[0].buildings",
            MAX_GROUP_BUILDINGS,
        ),
        // The arsenal: the entries and the four lists of an entry.
        (
            File::Arsenal,
            repeat_line(n, &arsenal, MINER_S, 102),
            "entries",
            128,
        ),
        (
            File::Arsenal,
            line_edit(
                n,
                &arsenal,
                SNIPER,
                &[(
                    r#"owner_sides: ["allied"]"#,
                    r#"owner_sides: ["allied", "soviet", "allied"]"#,
                )],
            ),
            "entries[19].owner_sides",
            2,
        ),
        (
            File::Arsenal,
            line_edit(
                n,
                &arsenal,
                SNIPER,
                &[(
                    r#"required_countries: ["alpha"]"#,
                    &format!(
                        "required_countries: {}",
                        list(r#""alpha""#, MAX_COUNTRIES + 1)
                    ),
                )],
            ),
            "entries[19].required_countries",
            MAX_COUNTRIES,
        ),
        (
            File::Arsenal,
            line_edit(
                n,
                &arsenal,
                RAIDER,
                &[(
                    r#"forbidden_countries: ["alpha"]"#,
                    &format!(
                        "forbidden_countries: {}",
                        list(r#""alpha""#, MAX_COUNTRIES + 1)
                    ),
                )],
            ),
            "entries[20].forbidden_countries",
            MAX_COUNTRIES,
        ),
        (
            File::Arsenal,
            line_edit(
                n,
                &arsenal,
                SNIPER,
                &[(
                    r#"prerequisite_groups: ["radar"]"#,
                    &format!(
                        "prerequisite_groups: {}",
                        list(r#""radar""#, MAX_PREREQUISITE_GROUPS + 1)
                    ),
                )],
            ),
            "entries[19].prerequisite_groups",
            MAX_PREREQUISITE_GROUPS,
        ),
        // The skirmish file.
        (
            File::Skirmish,
            repeat_line(n, &skirmish, SOVIET_START, 2),
            "sides",
            2,
        ),
        (
            File::Skirmish,
            line_edit(
                n,
                &skirmish,
                ALLIED_START,
                &[(
                    r#"starting_vehicles: ["tank"]"#,
                    &format!(
                        "starting_vehicles: {}",
                        list(r#""tank""#, MAX_STARTING_TYPES + 1)
                    ),
                )],
            ),
            "sides[0].starting_vehicles",
            MAX_STARTING_TYPES,
        ),
        (
            File::Skirmish,
            line_edit(
                n,
                &skirmish,
                SOVIET_START,
                &[(
                    r#"starting_infantry: ["conscript"]"#,
                    &format!(
                        "starting_infantry: {}",
                        list(r#""conscript""#, MAX_STARTING_TYPES + 1)
                    ),
                )],
            ),
            "sides[1].starting_infantry",
            MAX_STARTING_TYPES,
        ),
        (
            File::Skirmish,
            repeat_line(n, &skirmish, GAMMA_FACTION, MAX_FACTIONS - 1),
            "factions",
            MAX_FACTIONS,
        ),
        (
            File::Skirmish,
            line_edit(
                n,
                &skirmish,
                ALPHA_FACTION,
                &[(
                    "roster_buildings: []",
                    &format!(
                        "roster_buildings: {}",
                        list(r#""allied_power""#, MAX_ROSTER_ENTRIES + 1)
                    ),
                )],
            ),
            "factions[0].roster_buildings",
            MAX_ROSTER_ENTRIES,
        ),
        (
            File::Skirmish,
            line_edit(
                n,
                &skirmish,
                ALPHA_FACTION,
                &[(
                    "roster_units: []",
                    &format!(
                        "roster_units: {}",
                        list(r#""tank""#, MAX_ROSTER_ENTRIES + 1)
                    ),
                )],
            ),
            "factions[0].roster_units",
            MAX_ROSTER_ENTRIES,
        ),
        (
            File::Skirmish,
            line_edit(
                n,
                &skirmish,
                ALPHA_FACTION,
                &[(
                    "roster_units: [], buildings: []",
                    &format!(
                        "roster_units: [], buildings: {}",
                        list(yard, MAX_STARTING_BUILDINGS + 1)
                    ),
                )],
            ),
            "factions[0].buildings",
            MAX_STARTING_BUILDINGS,
        ),
        (
            File::Skirmish,
            line_edit(
                n,
                &skirmish,
                ALPHA_FACTION,
                &[(
                    ", units: []",
                    &format!(", units: {}", list(group, MAX_STARTING_GROUPS + 1)),
                )],
            ),
            "factions[0].units",
            MAX_STARTING_GROUPS,
        ),
        (
            File::Skirmish,
            repeat_line(n, &skirmish, MAP, MAX_MAPS + 1),
            "maps",
            MAX_MAPS,
        ),
        (
            File::Skirmish,
            line_edit(
                n,
                &skirmish,
                MAP,
                &[(
                    "starts: [(x: 5, y: 5), (x: 32, y: 32)]",
                    &format!("starts: {}", list("(x: 1, y: 1)", MAX_MATCH_PLAYERS + 1)),
                )],
            ),
            "maps[0].starts",
            MAX_MATCH_PLAYERS,
        ),
        // The AI file.
        (
            File::Ai,
            repeat_block(n, &ai, SOVIET_PLAN, MAX_PLANS),
            "plans",
            MAX_PLANS,
        ),
        (
            File::Ai,
            edit(
                n,
                &ai,
                ALLIED_ORDER,
                &format!(
                    "build_order: {},",
                    list(r#""allied_power""#, MAX_BUILD_ORDER + 1)
                ),
            ),
            "plans[0].build_order",
            MAX_BUILD_ORDER,
        ),
        (
            File::Ai,
            edit(
                n,
                &ai,
                SOVIET_ARMY,
                &format!("army: {},", list(r#""heavy""#, MAX_ARMY_ENTRIES + 1)),
            ),
            "plans[1].army",
            MAX_ARMY_ENTRIES,
        ),
        (
            File::Ai,
            repeat_block(n, &ai, BRUTAL_DIFFICULTY, MAX_DIFFICULTIES),
            "difficulties",
            MAX_DIFFICULTIES,
        ),
    ];
    for (index, (file, text, field, max)) in cases.into_iter().enumerate() {
        let path = temp_file(&format!("too_many_{index}.ron"), text.as_bytes());
        assert_rejected(file, &path, field, DataErrorKind::TooManyEntries { max });
    }
}

// --- Negative: the rules file ---------------------------------------------------------------

#[test]
fn every_scalar_rule_is_rejected_one_step_outside_its_range() {
    for (group, fields) in SCALAR_GROUPS {
        for &(field, min, max) in fields {
            for (end, value) in [("below", min - 1), ("above", max + 1)] {
                let path = scalar_variant(
                    &format!("rules_range_{group}_{field}_{end}"),
                    group,
                    field,
                    Some(&value.to_string()),
                );
                assert_rejected(
                    File::Rules,
                    &path,
                    &format!("{group}.{field}"),
                    invalid(&value.to_string(), &format!("{min} to {max}")),
                );
            }
        }
    }
    // A value beyond 32 bits is reported with its field and its range, and one beyond 64 bits
    // by the check of the subset of RON.
    let path = scalar_variant(
        "rules_range_32_bits",
        "economy",
        "ore_value",
        Some("2147483648"),
    );
    assert_rejected(
        File::Rules,
        &path,
        "economy.ore_value",
        invalid("2147483648", "0 to 1000000"),
    );
    let path = scalar_variant(
        "rules_range_64_bits",
        "economy",
        "ore_value",
        Some("9223372036854775808"),
    );
    assert_rejected(
        File::Rules,
        &path,
        "economy.ore_value",
        invalid("9223372036854775808", "a 64-bit signed integer"),
    );
}

#[test]
fn the_cross_checks_of_the_rules_file_are_enforced() {
    let cases: Vec<(Edits, &str, &str, &str)> = vec![
        // The least low power speed is at most the greatest.
        (
            &[("min_low_power_speed_pm: 500", "min_low_power_speed_pm: 801")],
            "production.min_low_power_speed_pm",
            "801",
            "at most max_low_power_speed_pm",
        ),
        (
            &[("max_low_power_speed_pm: 800", "max_low_power_speed_pm: 499")],
            "production.min_low_power_speed_pm",
            "500",
            "at most max_low_power_speed_pm",
        ),
        // Condition red lies at or below condition yellow.
        (
            &[("condition_red_percent: 25", "condition_red_percent: 51")],
            "combat.condition_red_percent",
            "51",
            "at most condition_yellow_percent",
        ),
        (
            &[(
                "condition_yellow_percent: 50",
                "condition_yellow_percent: 24",
            )],
            "combat.condition_red_percent",
            "25",
            "at most condition_yellow_percent",
        ),
        // The least number of crates is at most the greatest.
        (
            &[
                ("        maximum: 255,", "        maximum: 10,"),
                ("        minimum: 1,", "        minimum: 11,"),
            ],
            "crates.minimum",
            "11",
            "at most crates.maximum",
        ),
        // The default of the starting credits and of the unit count lies within its range,
        // and the range is not empty.
        (
            &[("        credits: 3000,", "        credits: 5500,")],
            "dialog.credits",
            "5500",
            "the default within its range, 1000 to 5000",
        ),
        (
            &[("        credits: 3000,", "        credits: 500,")],
            "dialog.credits",
            "500",
            "the default within its range, 1000 to 5000",
        ),
        (
            &[("min_credits: 1000", "min_credits: 6000")],
            "dialog.credits",
            "3000",
            "the default within its range, 6000 to 5000",
        ),
        (
            &[("        unit_count: 4,", "        unit_count: 11,")],
            "dialog.unit_count",
            "11",
            "the default within its range, 0 to 10",
        ),
        (
            &[("min_unit_count: 0", "min_unit_count: 5")],
            "dialog.unit_count",
            "4",
            "the default within its range, 5 to 10",
        ),
        // The step of the credits divides their range and reaches their default.
        (
            &[("credits_step: 500", "credits_step: 300")],
            "dialog.credits_step",
            "300",
            "a step that divides the range of the credits and reaches their default",
        ),
        (
            &[
                ("max_credits: 5000", "max_credits: 5500"),
                ("credits_step: 500", "credits_step: 1000"),
            ],
            "dialog.credits_step",
            "1000",
            "a step that divides the range of the credits and reaches their default",
        ),
    ];
    for (index, (edits, field, found, expected)) in cases.into_iter().enumerate() {
        let path = variant(File::Rules, &format!("rules_cross_{index}"), edits);
        assert_rejected(File::Rules, &path, field, invalid(found, expected));
    }
    // A default the step does not reach from the least amount.
    let path = variant(
        File::Rules,
        "rules_cross_default_off_step",
        &[("        credits: 3000,", "        credits: 3200,")],
    );
    assert_invalid(File::Rules, &path, "dialog.credits_step", "500");
    // Equal values are within every cross check, and a range may be a single value.
    let path = variant(
        File::Rules,
        "rules_cross_equal",
        &[
            ("min_low_power_speed_pm: 500", "min_low_power_speed_pm: 800"),
            ("condition_red_percent: 25", "condition_red_percent: 50"),
            ("        minimum: 1,", "        minimum: 255,"),
            ("min_credits: 1000", "min_credits: 3000"),
            ("max_credits: 5000", "max_credits: 3000"),
            ("min_unit_count: 0", "min_unit_count: 4"),
            ("max_unit_count: 10", "max_unit_count: 4"),
        ],
    );
    let definitions = as_definitions(load(File::Rules, &path));
    let rules = definitions.rules().expect("the rules are loaded");
    assert_eq!(
        (
            rules.production.min_low_power_speed_pm,
            rules.production.max_low_power_speed_pm,
        ),
        (800, 800)
    );
    assert_eq!(
        (
            rules.combat.condition_red_percent,
            rules.combat.condition_yellow_percent,
        ),
        (50, 50)
    );
    assert_eq!((rules.crates.minimum, rules.crates.maximum), (255, 255));
    assert_eq!(
        (
            rules.dialog.min_credits,
            rules.dialog.credits,
            rules.dialog.max_credits,
        ),
        (3000, 3000, 3000)
    );
    assert_eq!(
        (
            rules.dialog.min_unit_count,
            rules.dialog.unit_count,
            rules.dialog.max_unit_count,
        ),
        (4, 4, 4)
    );
}

#[test]
fn rules_list_values_outside_their_ranges_are_rejected() {
    let long_name = format!("id: \"{}\"", "n".repeat(33));
    let long_found = format!("{:?}", "n".repeat(33));
    let cases = [
        (MONEY, "share: 20", "share: -1", "powerups[0].share", "-1"),
        (
            MONEY,
            "share: 20",
            "share: 1001",
            "powerups[0].share",
            "1001",
        ),
        (MONEY, "value: 2000", "value: -1", "powerups[0].value", "-1"),
        (
            MONEY,
            "value: 2000",
            "value: 1000001",
            "powerups[0].value",
            "1000001",
        ),
        (
            MONEY,
            r#"id: "money""#,
            r#"id: "Money""#,
            "powerups[0].id",
            r#""Money""#,
        ),
        (
            EASY,
            "rof_pm: 1200",
            "rof_pm: 0",
            "difficulty[0].rof_pm",
            "0",
        ),
        (
            EASY,
            "rof_pm: 1200",
            "rof_pm: 10001",
            "difficulty[0].rof_pm",
            "10001",
        ),
        (
            EASY,
            "build_time_pm: 1000",
            "build_time_pm: 0",
            "difficulty[0].build_time_pm",
            "0",
        ),
        (
            EASY,
            "build_time_pm: 1000",
            "build_time_pm: 10001",
            "difficulty[0].build_time_pm",
            "10001",
        ),
        (
            EASY,
            "credits_bonus_percent: 0",
            "credits_bonus_percent: -1",
            "difficulty[0].credits_bonus_percent",
            "-1",
        ),
        (
            EASY,
            "credits_bonus_percent: 0",
            "credits_bonus_percent: 10001",
            "difficulty[0].credits_bonus_percent",
            "10001",
        ),
        (
            EASY,
            "virtual_purifiers: 0",
            "virtual_purifiers: -1",
            "difficulty[0].virtual_purifiers",
            "-1",
        ),
        (
            EASY,
            "virtual_purifiers: 0",
            "virtual_purifiers: 65",
            "difficulty[0].virtual_purifiers",
            "65",
        ),
        (
            EASY,
            "team_delay: 4667",
            "team_delay: 0",
            "difficulty[0].team_delay",
            "0",
        ),
        (
            EASY,
            "team_delay: 4667",
            "team_delay: 72001",
            "difficulty[0].team_delay",
            "72001",
        ),
        (
            EASY,
            "allied_base_defences: 6",
            "allied_base_defences: -1",
            "difficulty[0].allied_base_defences",
            "-1",
        ),
        (
            EASY,
            "allied_base_defences: 6",
            "allied_base_defences: 256",
            "difficulty[0].allied_base_defences",
            "256",
        ),
        (
            EASY,
            "soviet_base_defences: 6",
            "soviet_base_defences: -1",
            "difficulty[0].soviet_base_defences",
            "-1",
        ),
        (
            EASY,
            "soviet_base_defences: 6",
            "soviet_base_defences: 256",
            "difficulty[0].soviet_base_defences",
            "256",
        ),
        (
            EASY,
            r#"id: "easy""#,
            r#"id: "1easy""#,
            "difficulty[0].id",
            r#""1easy""#,
        ),
        (
            POWER_GROUP,
            r#"["allied_power", "soviet_power"]"#,
            "[]",
            "prerequisite_groups[0].buildings",
            "an empty list",
        ),
        (
            POWER_GROUP,
            r#"id: "power""#,
            r#"id: "the power""#,
            "prerequisite_groups[0].id",
            r#""the power""#,
        ),
        (
            POWER_GROUP,
            r#"id: "power""#,
            long_name.as_str(),
            "prerequisite_groups[0].id",
            long_found.as_str(),
        ),
        (
            SOVIET_RULES,
            "survivor_divisor: 250",
            "survivor_divisor: 0",
            "sides[1].survivor_divisor",
            "0",
        ),
        (
            SOVIET_RULES,
            "survivor_divisor: 250",
            "survivor_divisor: 1000001",
            "sides[1].survivor_divisor",
            "1000001",
        ),
        (
            SOVIET_RULES,
            "paradrop_count: 9",
            "paradrop_count: -1",
            "sides[1].paradrop_count",
            "-1",
        ),
        (
            SOVIET_RULES,
            "paradrop_count: 9",
            "paradrop_count: 65",
            "sides[1].paradrop_count",
            "65",
        ),
        (
            BETA_COUNTRY,
            r#"id: "beta""#,
            r#"id: """#,
            "countries[1].id",
            r#""""#,
        ),
    ];
    for (index, (line, from, to, field, found)) in cases.into_iter().enumerate() {
        let path = line_variant(
            File::Rules,
            &format!("rules_list_range_{index}"),
            line,
            &[(from, to)],
        );
        assert_invalid(File::Rules, &path, field, found);
    }
}

#[test]
fn rules_references_and_ids_are_checked() {
    let cases = [
        // The members of a prerequisite group are building types, each once.
        (
            POWER_GROUP,
            r#""soviet_power"]"#,
            r#""castle"]"#,
            "prerequisite_groups[0].buildings[1]",
            unknown("building", "castle"),
        ),
        (
            POWER_GROUP,
            r#""soviet_power"]"#,
            r#""tank"]"#,
            "prerequisite_groups[0].buildings[1]",
            unknown("building", "tank"),
        ),
        (
            POWER_GROUP,
            r#""soviet_power"]"#,
            r#""allied_power"]"#,
            "prerequisite_groups[0].buildings[1]",
            DataErrorKind::DuplicateId("allied_power".into()),
        ),
        // Ids are unique within their list.
        (
            RADAR_GROUP,
            r#"id: "radar""#,
            r#"id: "power""#,
            "prerequisite_groups[2].id",
            DataErrorKind::DuplicateId("power".into()),
        ),
        (
            ARMOUR,
            r#"id: "armour""#,
            r#"id: "money""#,
            "powerups[1].id",
            DataErrorKind::DuplicateId("money".into()),
        ),
        (
            BRUTAL,
            r#"id: "brutal""#,
            r#"id: "easy""#,
            "difficulty[1].id",
            DataErrorKind::DuplicateId("easy".into()),
        ),
        (
            GAMMA_COUNTRY,
            r#"id: "gamma""#,
            r#"id: "alpha""#,
            "countries[2].id",
            DataErrorKind::DuplicateId("alpha".into()),
        ),
        // A side is `allied` or `soviet`, at most once each.
        (
            SOVIET_RULES,
            r#"id: "soviet""#,
            r#"id: "nod""#,
            "sides[1].id",
            invalid(r#""nod""#, r#""allied" or "soviet""#),
        ),
        (
            SOVIET_RULES,
            r#"id: "soviet""#,
            r#"id: "none""#,
            "sides[1].id",
            invalid(r#""none""#, r#""allied" or "soviet""#),
        ),
        (
            SOVIET_RULES,
            r#"id: "soviet""#,
            r#"id: "allied""#,
            "sides[1].id",
            DataErrorKind::DuplicateId("allied".into()),
        ),
        // The units and the building a side names are defined, as that kind of type.
        (
            SOVIET_RULES,
            r#"base_unit: "soviet_mcv""#,
            r#"base_unit: "dragon""#,
            "sides[1].base_unit",
            unknown("unit", "dragon"),
        ),
        (
            SOVIET_RULES,
            r#"base_unit: "soviet_mcv""#,
            r#"base_unit: "soviet_yard""#,
            "sides[1].base_unit",
            unknown("unit", "soviet_yard"),
        ),
        (
            SOVIET_RULES,
            r#"construction_yard: "soviet_yard""#,
            r#"construction_yard: "castle""#,
            "sides[1].construction_yard",
            unknown("building", "castle"),
        ),
        (
            SOVIET_RULES,
            r#"construction_yard: "soviet_yard""#,
            r#"construction_yard: "soviet_mcv""#,
            "sides[1].construction_yard",
            unknown("building", "soviet_mcv"),
        ),
        (
            SOVIET_RULES,
            r#"crew: "conscript""#,
            r#"crew: "dragon""#,
            "sides[1].crew",
            unknown("unit", "dragon"),
        ),
        (
            SOVIET_RULES,
            r#"disguise: "conscript""#,
            r#"disguise: "dragon""#,
            "sides[1].disguise",
            unknown("unit", "dragon"),
        ),
        (
            SOVIET_RULES,
            r#"paradrop_infantry: "conscript""#,
            r#"paradrop_infantry: "dragon""#,
            "sides[1].paradrop_infantry",
            unknown("unit", "dragon"),
        ),
        (
            ALLIED_RULES,
            r#"crew: "rifleman""#,
            r#"crew: "allied_barracks""#,
            "sides[0].crew",
            unknown("unit", "allied_barracks"),
        ),
        // The base unit and the construction yard of a side belong to no other side.
        (
            SOVIET_RULES,
            r#"base_unit: "soviet_mcv""#,
            r#"base_unit: "allied_mcv""#,
            "sides[1].base_unit",
            invalid(r#""allied_mcv""#, "a base unit no other side has"),
        ),
        (
            SOVIET_RULES,
            r#"construction_yard: "soviet_yard""#,
            r#"construction_yard: "allied_yard""#,
            "sides[1].construction_yard",
            invalid(r#""allied_yard""#, "a construction yard no other side has"),
        ),
        // Every country names a listed side.
        (
            BETA_COUNTRY,
            r#"side: "allied""#,
            r#"side: "nod""#,
            "countries[1].side",
            invalid(r#""nod""#, "a side listed in sides"),
        ),
        (
            BETA_COUNTRY,
            r#"side: "allied""#,
            r#"side: "none""#,
            "countries[1].side",
            invalid(r#""none""#, "a side listed in sides"),
        ),
        (
            BETA_COUNTRY,
            r#"side: "allied""#,
            r#"side: "Allied""#,
            "countries[1].side",
            invalid(r#""Allied""#, "a side listed in sides"),
        ),
    ];
    for (index, (line, from, to, field, kind)) in cases.into_iter().enumerate() {
        let path = line_variant(
            File::Rules,
            &format!("rules_reference_{index}"),
            line,
            &[(from, to)],
        );
        assert_rejected(File::Rules, &path, field, kind);
    }
    // A side the file does not list: without the Soviet side its country names no side.
    let name = "rules_country_of_unlisted_side";
    let text = repeat_line(name, &File::Rules.text(), SOVIET_RULES, 0);
    let path = temp_file(&format!("{name}.ron"), text.as_bytes());
    assert_rejected(
        File::Rules,
        &path,
        "countries[2].side",
        invalid(r#""soviet""#, "a side listed in sides"),
    );
    // Without that country the file loads with one side.
    let definitions = as_definitions(load(File::Rules, &allied_rules("rules_one_side")));
    let rules = definitions.rules().expect("the rules are loaded");
    assert_eq!(rules.sides.len(), 1);
    assert_eq!(rules.countries.len(), 2);
    assert!(rules.side(Side::Soviet).is_none());
    // The building and unit types are those of the definitions the file is loaded onto: the
    // fixture of M4 to M6 in the parent directory has none of the RA1 fixture's types.
    let legacy = fixtures().join("..");
    let other = load_match_definitions(
        legacy.join("units_combat.ron"),
        legacy.join("economy_combat.ron"),
        legacy.join("combat.ron"),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert_rejected_with(
        File::Rules,
        &File::Rules.fixture(),
        other,
        "prerequisite_groups[0].buildings[0]",
        unknown("building", "allied_power"),
    );
}

// --- Negative: the arsenal ------------------------------------------------------------------

#[test]
fn arsenal_values_outside_their_ranges_are_rejected() {
    let long_description = format!("description: \"{}\"", "d".repeat(MAX_STRING_BYTES + 1));
    let incompatible = "role/domain incompatible with entity type";
    let cases = [
        // The category, and an id that is not a type of that category or is listed twice.
        (
            SNIPER,
            r#"category: "infantry""#,
            r#"category: "cavalry""#,
            "entries[19].category",
            "cavalry",
        ),
        (
            SNIPER,
            r#"(id: "sniper""#,
            r#"(id: "dragon""#,
            "entries[19].id",
            "dragon",
        ),
        (
            SNIPER,
            r#"(id: "sniper""#,
            r#"(id: "turret""#,
            "entries[19].id",
            "turret",
        ),
        (
            SNIPER,
            r#"(id: "sniper""#,
            r#"(id: "rifleman""#,
            "entries[19].id",
            "rifleman",
        ),
        (
            TURRET,
            r#"category: "armory""#,
            r#"category: "vehicles""#,
            "entries[12].id",
            "turret",
        ),
        // The domain and the role, and what an entity of that kind may have.
        (
            SNIPER,
            r#"domain: "land""#,
            r#"domain: "space""#,
            "entries[19].domain",
            "space",
        ),
        (
            SNIPER,
            r#"role: "combat""#,
            r#"role: "wizard""#,
            "entries[19].role",
            "wizard",
        ),
        (
            SNIPER,
            r#"role: "combat""#,
            r#"role: "storm""#,
            "entries[19].role",
            incompatible,
        ),
        (
            RADAR,
            r#"domain: "land""#,
            r#"domain: "air""#,
            "entries[10].role",
            incompatible,
        ),
        // The numbers.
        (
            SNIPER,
            "cooldown: 100",
            "cooldown: -1",
            "entries[19].cooldown",
            "-1",
        ),
        (
            SNIPER,
            "cooldown: 100",
            "cooldown: 72001",
            "entries[19].cooldown",
            "72001",
        ),
        (
            SNIPER,
            "radius: 2",
            "radius: -1",
            "entries[19].radius",
            "-1",
        ),
        (
            SNIPER,
            "radius: 2",
            "radius: 33",
            "entries[19].radius",
            "33",
        ),
        (
            SNIPER,
            "strength: 20",
            "strength: -1",
            "entries[19].strength",
            "-1",
        ),
        (
            SNIPER,
            "strength: 20",
            "strength: 10001",
            "entries[19].strength",
            "10001",
        ),
        (
            SNIPER,
            "capacity: 0",
            "capacity: -1",
            "entries[19].capacity",
            "-1",
        ),
        (
            SNIPER,
            "capacity: 0",
            "capacity: 17",
            "entries[19].capacity",
            "17",
        ),
        (
            SNIPER,
            "tech_level: 1",
            "tech_level: -2",
            "entries[19].tech_level",
            "-2",
        ),
        (
            SNIPER,
            "tech_level: 1",
            "tech_level: 11",
            "entries[19].tech_level",
            "11",
        ),
        (
            STORM,
            "cooldown: 600",
            "cooldown: 0",
            "entries[13].cooldown",
            "zero superweapon timing/radius",
        ),
        (
            STORM,
            "radius: 3",
            "radius: 0",
            "entries[13].cooldown",
            "zero superweapon timing/radius",
        ),
        // The fields of schema version 2: art, side, factory, owner sides, countries.
        (
            SNIPER,
            r#"art: "sniper""#,
            r#"art: "Sniper""#,
            "entries[19].art",
            r#""Sniper""#,
        ),
        (
            SNIPER,
            r#"art: "sniper""#,
            r#"art: """#,
            "entries[19].art",
            r#""""#,
        ),
        (
            SNIPER,
            r#"art: "sniper""#,
            r#"art: "../sniper""#,
            "entries[19].art",
            r#""../sniper""#,
        ),
        (
            SNIPER,
            r#"side: "allied""#,
            r#"side: "nod""#,
            "entries[19].side",
            "nod",
        ),
        (
            BARRACKS,
            r#"factory: "infantry""#,
            r#"factory: "kitchen""#,
            "entries[6].factory",
            "kitchen",
        ),
        (
            SNIPER,
            r#"factory: "none""#,
            r#"factory: "infantry""#,
            "entries[19].factory",
            "a factory group on a unit type",
        ),
        (
            SNIPER,
            r#"owner_sides: ["allied"]"#,
            r#"owner_sides: ["nod"]"#,
            "entries[19].owner_sides[0]",
            r#""nod""#,
        ),
        (
            SNIPER,
            r#"owner_sides: ["allied"]"#,
            r#"owner_sides: ["allied", "none"]"#,
            "entries[19].owner_sides[1]",
            r#""none""#,
        ),
        (
            SNIPER,
            "forbidden_countries: []",
            r#"forbidden_countries: ["beta", "alpha"]"#,
            "entries[19].forbidden_countries[1]",
            r#""alpha""#,
        ),
        // A string longer than the subset of RON allows.
        (
            SNIPER,
            r#"description: "Fixture item.""#,
            long_description.as_str(),
            "entries[19].description",
            "of 257 bytes",
        ),
    ];
    for (index, (line, from, to, field, found)) in cases.into_iter().enumerate() {
        let path = line_variant(
            File::Arsenal,
            &format!("arsenal_range_{index}"),
            line,
            &[(from, to)],
        );
        assert_invalid(File::Arsenal, &path, field, found);
    }
}

#[test]
fn arsenal_references_and_ids_are_checked() {
    let cases = [
        // The art is that of a defined item of the same kind: a unit's for a unit.
        (
            SNIPER,
            r#"art: "sniper""#,
            r#"art: "dragon""#,
            "entries[19].art",
            unknown("unit", "dragon"),
        ),
        (
            SNIPER,
            r#"art: "sniper""#,
            r#"art: "allied_barracks""#,
            "entries[19].art",
            unknown("unit", "allied_barracks"),
        ),
        // Countries are those of the rules file, each once per list.
        (
            SNIPER,
            r#"required_countries: ["alpha"]"#,
            r#"required_countries: ["omega"]"#,
            "entries[19].required_countries[0]",
            unknown("country", "omega"),
        ),
        (
            SNIPER,
            r#"required_countries: ["alpha"]"#,
            r#"required_countries: ["alpha", "allied"]"#,
            "entries[19].required_countries[1]",
            unknown("country", "allied"),
        ),
        (
            SNIPER,
            r#"required_countries: ["alpha"]"#,
            r#"required_countries: ["alpha", "beta", "alpha"]"#,
            "entries[19].required_countries[2]",
            DataErrorKind::DuplicateId("alpha".into()),
        ),
        (
            RAIDER,
            r#"forbidden_countries: ["alpha"]"#,
            r#"forbidden_countries: ["omega"]"#,
            "entries[20].forbidden_countries[0]",
            unknown("country", "omega"),
        ),
        (
            RAIDER,
            r#"forbidden_countries: ["alpha"]"#,
            r#"forbidden_countries: ["beta", "beta"]"#,
            "entries[20].forbidden_countries[1]",
            DataErrorKind::DuplicateId("beta".into()),
        ),
        // Prerequisite groups are those of the rules file, each once.
        (
            SNIPER,
            r#"prerequisite_groups: ["radar"]"#,
            r#"prerequisite_groups: ["airfield"]"#,
            "entries[19].prerequisite_groups[0]",
            unknown("prerequisite group", "airfield"),
        ),
        (
            SNIPER,
            r#"prerequisite_groups: ["radar"]"#,
            r#"prerequisite_groups: ["radar", "allied_radar"]"#,
            "entries[19].prerequisite_groups[1]",
            unknown("prerequisite group", "allied_radar"),
        ),
        (
            SNIPER,
            r#"prerequisite_groups: ["radar"]"#,
            r#"prerequisite_groups: ["radar", "power", "radar"]"#,
            "entries[19].prerequisite_groups[2]",
            DataErrorKind::DuplicateId("radar".into()),
        ),
        // An owner side is listed once.
        (
            SNIPER,
            r#"owner_sides: ["allied"]"#,
            r#"owner_sides: ["allied", "allied"]"#,
            "entries[19].owner_sides[1]",
            DataErrorKind::DuplicateId("allied".into()),
        ),
        // A country is not both required and forbidden.
        (
            SNIPER,
            "forbidden_countries: []",
            r#"forbidden_countries: ["alpha"]"#,
            "entries[19].forbidden_countries[0]",
            invalid(r#""alpha""#, "a country that is not also required"),
        ),
    ];
    for (index, (line, from, to, field, kind)) in cases.into_iter().enumerate() {
        let path = line_variant(
            File::Arsenal,
            &format!("arsenal_reference_{index}"),
            line,
            &[(from, to)],
        );
        assert_rejected(File::Arsenal, &path, field, kind);
    }
    // Every entry is an item with production terms: against an economy file that gives the
    // militia none, its entry is rejected.
    let economy = economy_variant(
        "economy_militia_without_terms",
        &[(
            "        (unit: \"militia\", cost: 60, build_time: 10, producer: \"soviet_barracks\", prerequisites: []),\n",
            "",
        )],
    );
    let context = definitions_of(&economy, Some(&File::Rules.fixture()), None);
    let error = load_with(File::Arsenal, &File::Arsenal.fixture(), context)
        .expect_err("an entry without production terms");
    assert_eq!(error.field, "entries[21].id", "{error}");
    assert!(
        matches!(&error.kind, DataErrorKind::InvalidValue { found, .. } if found == "militia"),
        "{error}"
    );
}

#[test]
fn an_arsenal_that_names_countries_or_groups_needs_a_rules_file() {
    // Sides and factory groups need no rules file.
    let path = arsenal_of("arsenal_plain", &["allied_yard", "rifleman", "tank"]);
    let definitions = as_definitions(load_with(File::Arsenal, &path, base_definitions()));
    assert_eq!(definitions.arsenal().0.len(), 3);
    let rifleman = definitions
        .metadata(item(&definitions, "rifleman"))
        .expect("the rifleman has an entry");
    assert_eq!(rifleman.side, Some(Side::Allied));
    assert_eq!(rifleman.owner_sides, [Side::Allied]);
    assert_eq!(
        definitions.factory_group(building(&definitions, "allied_yard")),
        Some(FactoryGroup::Buildings)
    );
    // A country or a prerequisite group is one of the rules file; without one there is none.
    let cases: Vec<(&[&str], &str, DataErrorKind)> = vec![
        (
            &["rifleman", "sniper"],
            "entries[1].required_countries[0]",
            unknown("country", "alpha"),
        ),
        (
            &["raider"],
            "entries[0].forbidden_countries[0]",
            unknown("country", "alpha"),
        ),
        (
            &["rifleman", "tank", "engineer"],
            "entries[2].prerequisite_groups[0]",
            unknown("prerequisite group", "barracks"),
        ),
        (
            &["allied_refinery"],
            "entries[0].prerequisite_groups[0]",
            unknown("prerequisite group", "power"),
        ),
    ];
    for (index, (ids, field, kind)) in cases.into_iter().enumerate() {
        let path = arsenal_of(&format!("arsenal_without_rules_{index}"), ids);
        assert_rejected_with(File::Arsenal, &path, base_definitions(), field, kind);
    }
    // The fixture itself: its first entry that names a group is the Allied refinery.
    assert_rejected_with(
        File::Arsenal,
        &File::Arsenal.fixture(),
        base_definitions(),
        "entries[4].prerequisite_groups[0]",
        unknown("prerequisite group", "power"),
    );
}

#[test]
fn the_base_unit_of_every_side_has_the_role_mcv() {
    let missing = |side: &str, id: &str| DataErrorKind::MissingEntry {
        kind: format!("base unit of side {side:?} with the role \"mcv\""),
        id: id.into(),
    };
    // Another role.
    let path = line_variant(
        File::Arsenal,
        "arsenal_allied_mcv_role",
        ALLIED_MCV,
        &[(r#"role: "mcv""#, r#"role: "combat""#)],
    );
    assert_rejected(
        File::Arsenal,
        &path,
        "entries",
        missing("allied", "allied_mcv"),
    );
    let path = line_variant(
        File::Arsenal,
        "arsenal_soviet_mcv_role",
        SOVIET_MCV,
        &[(r#"role: "mcv""#, r#"role: "transport""#)],
    );
    assert_rejected(
        File::Arsenal,
        &path,
        "entries",
        missing("soviet", "soviet_mcv"),
    );
    // No entry at all.
    let name = "arsenal_no_soviet_mcv";
    let text = repeat_line(name, &File::Arsenal.text(), SOVIET_MCV, 0);
    let path = temp_file(&format!("{name}.ron"), text.as_bytes());
    assert_rejected(
        File::Arsenal,
        &path,
        "entries",
        missing("soviet", "soviet_mcv"),
    );
    let path = arsenal_of("arsenal_no_mcv", &["rifleman"]);
    assert_rejected(
        File::Arsenal,
        &path,
        "entries",
        missing("allied", "allied_mcv"),
    );
    // The two base units alone satisfy the rule; without a rules file there are no sides and
    // so no base units; and with the rules of the Allied side alone only its base unit counts.
    let path = arsenal_of("arsenal_only_mcvs", &["allied_mcv", "soviet_mcv"]);
    let definitions = as_definitions(load(File::Arsenal, &path));
    assert_eq!(definitions.arsenal().0.len(), 2);
    let path = arsenal_of("arsenal_only_rifleman", &["rifleman"]);
    let definitions = as_definitions(load_with(File::Arsenal, &path, base_definitions()));
    assert_eq!(definitions.arsenal().0.len(), 1);
    let context = definitions_of(
        &fixtures().join("economy.ron"),
        Some(&allied_rules("rules_allied_for_arsenal")),
        None,
    );
    let path = arsenal_of("arsenal_only_allied_mcv", &["allied_mcv"]);
    let definitions = as_definitions(load_with(File::Arsenal, &path, context));
    assert_eq!(definitions.arsenal().0.len(), 1);
}

// --- Negative: the skirmish file ------------------------------------------------------------

#[test]
fn skirmish_sides_are_checked() {
    let cost = "a unit type with production terms and a cost above 0";
    let cases = [
        // A side is one the rules file lists, at most once.
        (
            ALLIED_START,
            r#"side: "allied""#,
            r#"side: "nod""#,
            "sides[0].side",
            invalid(r#""nod""#, "a side of the rules file"),
        ),
        (
            ALLIED_START,
            r#"side: "allied""#,
            r#"side: "none""#,
            "sides[0].side",
            invalid(r#""none""#, "a side of the rules file"),
        ),
        (
            SOVIET_START,
            r#"side: "soviet""#,
            r#"side: "allied""#,
            "sides[1].side",
            DataErrorKind::DuplicateId("allied".into()),
        ),
        // The starting units are unit types, each once per list.
        (
            ALLIED_START,
            r#"starting_vehicles: ["tank"]"#,
            r#"starting_vehicles: ["dragon"]"#,
            "sides[0].starting_vehicles[0]",
            unknown("unit", "dragon"),
        ),
        (
            SOVIET_START,
            r#"starting_infantry: ["conscript"]"#,
            r#"starting_infantry: ["conscript", "dragon"]"#,
            "sides[1].starting_infantry[1]",
            unknown("unit", "dragon"),
        ),
        (
            SOVIET_START,
            r#"starting_infantry: ["conscript"]"#,
            r#"starting_infantry: ["soviet_barracks"]"#,
            "sides[1].starting_infantry[0]",
            unknown("unit", "soviet_barracks"),
        ),
        (
            SOVIET_START,
            r#"starting_vehicles: ["heavy"]"#,
            r#"starting_vehicles: ["castle"]"#,
            "sides[1].starting_vehicles[0]",
            unknown("unit", "castle"),
        ),
        (
            ALLIED_START,
            r#"starting_vehicles: ["tank"]"#,
            r#"starting_vehicles: ["tank", "tank"]"#,
            "sides[0].starting_vehicles[1]",
            DataErrorKind::DuplicateId("tank".into()),
        ),
        (
            ALLIED_START,
            r#"starting_infantry: ["rifleman"]"#,
            r#"starting_infantry: ["rifleman", "engineer", "rifleman"]"#,
            "sides[0].starting_infantry[2]",
            DataErrorKind::DuplicateId("rifleman".into()),
        ),
    ];
    for (index, (line, from, to, field, kind)) in cases.into_iter().enumerate() {
        let path = line_variant(
            File::Skirmish,
            &format!("skirmish_side_{index}"),
            line,
            &[(from, to)],
        );
        assert_rejected(File::Skirmish, &path, field, kind);
    }
    // Without a rules file there are no sides.
    assert_rejected_with(
        File::Skirmish,
        &File::Skirmish.fixture(),
        base_definitions(),
        "sides[0].side",
        invalid(r#""allied""#, "a side of the rules file"),
    );
    // A side the rules file does not list.
    let allied = allied_rules("rules_allied_for_skirmish");
    let context = definitions_of(
        &fixtures().join("economy.ron"),
        Some(&allied),
        Some(&File::Arsenal.fixture()),
    );
    assert_rejected_with(
        File::Skirmish,
        &File::Skirmish.fixture(),
        context,
        "sides[1].side",
        invalid(r#""soviet""#, "a side of the rules file"),
    );
    // A starting unit has a cost above 0, since the budget of the starting units is spent by
    // cost. The fixture has no such unit type, so the economy file of this context gives the
    // tank the cost 0.
    let economy = economy_variant(
        "economy_free_tank",
        &[(r#"(unit: "tank", cost: 400,"#, r#"(unit: "tank", cost: 0,"#)],
    );
    let context = definitions_of(
        &economy,
        Some(&File::Rules.fixture()),
        Some(&File::Arsenal.fixture()),
    );
    assert_rejected_with(
        File::Skirmish,
        &File::Skirmish.fixture(),
        context,
        "sides[0].starting_vehicles[0]",
        invalid(r#""tank""#, cost),
    );
    // And it has production terms: here the economy file has none for the militia (and the
    // arsenal no entry for it).
    let economy = economy_variant(
        "economy_no_militia_for_skirmish",
        &[(
            "        (unit: \"militia\", cost: 60, build_time: 10, producer: \"soviet_barracks\", prerequisites: []),\n",
            "",
        )],
    );
    let name = "arsenal_no_militia_for_skirmish";
    let arsenal = temp_file(
        &format!("{name}.ron"),
        repeat_line(name, &File::Arsenal.text(), r#"(id: "militia","#, 0).as_bytes(),
    );
    let context = definitions_of(&economy, Some(&File::Rules.fixture()), Some(&arsenal));
    let path = line_variant(
        File::Skirmish,
        "skirmish_side_militia",
        SOVIET_START,
        &[(
            r#"starting_infantry: ["conscript"]"#,
            r#"starting_infantry: ["conscript", "militia"]"#,
        )],
    );
    assert_rejected_with(
        File::Skirmish,
        &path,
        context,
        "sides[1].starting_infantry[1]",
        invalid(r#""militia""#, cost),
    );
    // With the fixture's economy the militia is a starting unit like any other.
    let loaded = as_skirmish(load(File::Skirmish, &path));
    assert_eq!(loaded.sides[1].infantry.len(), 2);
}

#[test]
fn skirmish_countries_are_checked() {
    let empty = "an empty list: a country starts with the base unit of its side and builds by \
                 the rules";
    let label = "1 to 32 characters";
    let long_label = "L".repeat(MAX_LABEL_LEN + 1);
    let long_label_edit = format!("label: \"{long_label}\"");
    let long_label_found = format!("{long_label:?}");
    let cases = [
        // The id of a country is a country of the rules file of the faction's side.
        (
            ALPHA_FACTION,
            r#"id: "alpha""#,
            r#"id: "omega""#,
            "factions[0].id",
            invalid(
                r#""omega""#,
                r#"a country of the rules file of the side "allied""#,
            ),
        ),
        (
            ALPHA_FACTION,
            r#"side: "allied""#,
            r#"side: "soviet""#,
            "factions[0].id",
            invalid(
                r#""alpha""#,
                r#"a country of the rules file of the side "soviet""#,
            ),
        ),
        (
            GAMMA_FACTION,
            r#"side: "soviet""#,
            r#"side: "allied""#,
            "factions[2].id",
            invalid(
                r#""gamma""#,
                r#"a country of the rules file of the side "allied""#,
            ),
        ),
        // Its side is `none` or a side listed in `sides`.
        (
            ALPHA_FACTION,
            r#"side: "allied""#,
            r#"side: "nod""#,
            "factions[0].side",
            invalid(r#""nod""#, r#""none" or a side listed in sides"#),
        ),
        // Its four lists are empty.
        (
            ALPHA_FACTION,
            "roster_buildings: []",
            r#"roster_buildings: ["allied_power"]"#,
            "factions[0].roster_buildings",
            invalid("1 entries", empty),
        ),
        (
            ALPHA_FACTION,
            "roster_units: []",
            r#"roster_units: ["tank", "rifleman"]"#,
            "factions[0].roster_units",
            invalid("2 entries", empty),
        ),
        (
            ALPHA_FACTION,
            "roster_units: [], buildings: []",
            r#"roster_units: [], buildings: [(building: "allied_yard", x: 0, y: 0)]"#,
            "factions[0].buildings",
            invalid("1 entries", empty),
        ),
        (
            ALPHA_FACTION,
            ", units: []",
            r#", units: [(unit: "tank", x: 4, y: 1, count: 2)]"#,
            "factions[0].units",
            invalid("1 entries", empty),
        ),
        // Its label has 1 to `MAX_LABEL_LEN` characters.
        (
            ALPHA_FACTION,
            r#"label: "Alpha""#,
            r#"label: """#,
            "factions[0].label",
            invalid(r#""""#, label),
        ),
        (
            ALPHA_FACTION,
            r#"label: "Alpha""#,
            long_label_edit.as_str(),
            "factions[0].label",
            invalid(&long_label_found, label),
        ),
        // Its id is a name, once.
        (
            ALPHA_FACTION,
            r#"id: "alpha""#,
            r#"id: "Alpha""#,
            "factions[0].id",
            invalid(
                r#""Alpha""#,
                "1 to 32 characters from a-z, 0-9, and _, starting with a letter",
            ),
        ),
        (
            BETA_FACTION,
            r#"id: "beta""#,
            r#"id: "alpha""#,
            "factions[1].id",
            DataErrorKind::DuplicateId("alpha".into()),
        ),
        (
            GAMMA_FACTION,
            r#"id: "gamma""#,
            r#"id: "beta""#,
            "factions[2].id",
            DataErrorKind::DuplicateId("beta".into()),
        ),
    ];
    for (index, (line, from, to, field, kind)) in cases.into_iter().enumerate() {
        let path = line_variant(
            File::Skirmish,
            &format!("skirmish_country_{index}"),
            line,
            &[(from, to)],
        );
        assert_rejected(File::Skirmish, &path, field, kind);
    }
    // A side of the rules file that `sides` does not list: every side a country names has an
    // entry.
    let name = "skirmish_country_of_unlisted_side";
    let text = repeat_line(name, &File::Skirmish.text(), SOVIET_START, 0);
    let path = temp_file(&format!("{name}.ron"), text.as_bytes());
    assert_rejected(
        File::Skirmish,
        &path,
        "factions[2].side",
        invalid(r#""soviet""#, r#""none" or a side listed in sides"#),
    );
    // A label longer than a string of the subset of RON.
    let path = line_variant(
        File::Skirmish,
        "skirmish_country_label_bytes",
        ALPHA_FACTION,
        &[(
            r#"label: "Alpha""#,
            &format!("label: \"{}\"", "L".repeat(MAX_STRING_BYTES + 1)),
        )],
    );
    assert_invalid(File::Skirmish, &path, "factions[0].label", "of 257 bytes");
}

#[test]
fn skirmish_factions_without_a_side_are_checked() {
    // A faction without a side still needs at least one starting building, or its player would
    // be defeated at once.
    let path = sideless_variant(
        "skirmish_sideless_no_buildings",
        &[(
            r#"buildings: [(building: "allied_yard", x: 0, y: 0)]"#,
            "buildings: []",
        )],
    );
    assert_rejected(
        File::Skirmish,
        &path,
        "factions[0].buildings",
        invalid("an empty list", "1 to 16 starting buildings"),
    );
    // The country made a faction without a side by its side alone: all four lists are empty.
    let path = line_variant(
        File::Skirmish,
        "skirmish_country_to_none",
        ALPHA_FACTION,
        &[(r#"side: "allied""#, r#"side: "none""#)],
    );
    assert_invalid(
        File::Skirmish,
        &path,
        "factions[0].buildings",
        "an empty list",
    );
    let references = [
        (
            r#"roster_buildings: ["allied_power"]"#,
            r#"roster_buildings: ["castle"]"#,
            "factions[0].roster_buildings[0]",
            unknown("building", "castle"),
        ),
        (
            r#"roster_units: ["tank"]"#,
            r#"roster_units: ["tank", "dragon"]"#,
            "factions[0].roster_units[1]",
            unknown("unit", "dragon"),
        ),
        (
            r#"roster_buildings: ["allied_power"]"#,
            r#"roster_buildings: ["allied_power", "turret", "allied_power"]"#,
            "factions[0].roster_buildings[2]",
            DataErrorKind::DuplicateId("allied_power".into()),
        ),
        (
            r#"roster_units: ["tank"]"#,
            r#"roster_units: ["tank", "tank"]"#,
            "factions[0].roster_units[1]",
            DataErrorKind::DuplicateId("tank".into()),
        ),
        (
            r#"(building: "allied_yard""#,
            r#"(building: "castle""#,
            "factions[0].buildings[0].building",
            unknown("building", "castle"),
        ),
        (
            r#"(unit: "tank""#,
            r#"(unit: "dragon""#,
            "factions[0].units[0].unit",
            unknown("unit", "dragon"),
        ),
        (
            r#"(unit: "tank""#,
            r#"(unit: "allied_yard""#,
            "factions[0].units[0].unit",
            unknown("unit", "allied_yard"),
        ),
    ];
    for (index, (from, to, field, kind)) in references.into_iter().enumerate() {
        let path = sideless_variant(
            &format!("skirmish_sideless_reference_{index}"),
            &[(from, to)],
        );
        assert_rejected(File::Skirmish, &path, field, kind);
    }
    let ranges = [
        ("x: 0", "x: -33", "factions[0].buildings[0].x", "-33"),
        ("x: 0", "x: 33", "factions[0].buildings[0].x", "33"),
        ("y: 0", "y: -33", "factions[0].buildings[0].y", "-33"),
        ("y: 0", "y: 33", "factions[0].buildings[0].y", "33"),
        ("x: 4", "x: -33", "factions[0].units[0].x", "-33"),
        ("x: 4", "x: 33", "factions[0].units[0].x", "33"),
        ("y: 1", "y: -33", "factions[0].units[0].y", "-33"),
        ("y: 1", "y: 33", "factions[0].units[0].y", "33"),
        ("count: 2", "count: 0", "factions[0].units[0].count", "0"),
        ("count: 2", "count: 65", "factions[0].units[0].count", "65"),
    ];
    for (index, (from, to, field, found)) in ranges.into_iter().enumerate() {
        let path = sideless_variant(&format!("skirmish_sideless_range_{index}"), &[(from, to)]);
        assert_invalid(File::Skirmish, &path, field, found);
    }
}

#[test]
fn skirmish_values_outside_their_ranges_are_rejected() {
    for (index, (from, to, field, found)) in [
        (
            "starting_credits: 3000",
            "starting_credits: -1",
            "starting_credits",
            "-1",
        ),
        (
            "starting_credits: 3000",
            "starting_credits: 1000001",
            "starting_credits",
            "1000001",
        ),
        (
            "(x: 5, y: 5), (x: 32, y: 32)",
            "(x: 5, y: 5), (x: 256, y: 32)",
            "maps[0].starts[1].x",
            "256",
        ),
        (
            "(x: 5, y: 5), (x: 32, y: 32)",
            "(x: 5, y: 5), (x: 32, y: 256)",
            "maps[0].starts[1].y",
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
            "(x: 5, y: -1), (x: 32, y: 32)",
            "maps[0].starts[0].y",
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
    let name = "skirmish_no_factions";
    let mut no_factions = text.clone();
    for line in [ALPHA_FACTION, BETA_FACTION, GAMMA_FACTION] {
        no_factions = repeat_line(name, &no_factions, line, 0);
    }
    let path = temp_file(&format!("{name}.ron"), no_factions.as_bytes());
    assert_invalid(File::Skirmish, &path, "factions", "an empty list");
    let name = "skirmish_no_maps";
    let path = temp_file(
        &format!("{name}.ron"),
        repeat_line(name, &text, MAP, 0).as_bytes(),
    );
    assert_invalid(File::Skirmish, &path, "maps", "an empty list");
    // The id of a map is unique.
    let name = "skirmish_two_maps";
    let path = temp_file(
        &format!("{name}.ron"),
        repeat_line(name, &text, MAP, 2).as_bytes(),
    );
    assert_rejected(
        File::Skirmish,
        &path,
        "maps[1].id",
        DataErrorKind::DuplicateId("skirmish_40x40".into()),
    );
}

// --- Negative: the AI file ------------------------------------------------------------------

#[test]
fn ai_plans_are_checked_for_their_side() {
    let side = r#""none" or a side of the rules file"#;
    // A plan is for `none` or a side of the rules file; each side has at most one plan.
    let path = variant(
        File::Ai,
        "ai_unknown_side",
        &[("side: \"allied\",", "side: \"nod\",")],
    );
    assert_rejected(File::Ai, &path, "plans[0].side", invalid(r#""nod""#, side));
    let path = variant(
        File::Ai,
        "ai_duplicate_side",
        &[("side: \"soviet\",", "side: \"allied\",")],
    );
    assert_rejected(
        File::Ai,
        &path,
        "plans[1].side",
        DataErrorKind::DuplicateId("allied".into()),
    );
    // Without a rules file there are no sides, and a side the rules file does not list is none
    // either.
    assert_rejected_with(
        File::Ai,
        &File::Ai.fixture(),
        base_definitions(),
        "plans[0].side",
        invalid(r#""allied""#, side),
    );
    let context = definitions_of(
        &fixtures().join("economy.ron"),
        Some(&allied_rules("rules_allied_for_ai")),
        Some(&File::Arsenal.fixture()),
    );
    assert_rejected_with(
        File::Ai,
        &File::Ai.fixture(),
        context,
        "plans[1].side",
        invalid(r#""soviet""#, side),
    );
    // The plan for the factions without a side does not start from a Construction Yard, so the
    // yard the Allied buildings need is not listed before them.
    let path = variant(
        File::Ai,
        "ai_plan_without_a_side",
        &[("side: \"allied\",", "side: \"none\",")],
    );
    assert_rejected(
        File::Ai,
        &path,
        "plans[0].build_order[0]",
        invalid(
            r#""allied_power", whose prerequisite "allied_yard" is not listed before it"#,
            "a building type whose prerequisites are listed before it",
        ),
    );
    // No plans, and no difficulties.
    let text = File::Ai.text();
    let name = "ai_no_plans";
    let no_plans = repeat_block(name, &text, ALLIED_PLAN, 0);
    let no_plans = repeat_block(name, &no_plans, SOVIET_PLAN, 0);
    let path = temp_file(&format!("{name}.ron"), no_plans.as_bytes());
    assert_rejected(
        File::Ai,
        &path,
        "plans",
        invalid("an empty list", "1 to 3 plans"),
    );
    let name = "ai_no_difficulties";
    let no_difficulties = repeat_block(name, &text, EASY_DIFFICULTY, 0);
    let no_difficulties = repeat_block(name, &no_difficulties, BRUTAL_DIFFICULTY, 0);
    let path = temp_file(&format!("{name}.ron"), no_difficulties.as_bytes());
    assert_rejected(
        File::Ai,
        &path,
        "difficulties",
        invalid("an empty list", "1 to 8 difficulties"),
    );
    // The file is checked against the weapons of a combat file; without one it is rejected.
    let without_combat =
        load_definitions(fixtures().join("units.ron"), fixtures().join("economy.ron"))
            .and_then(|definitions| definitions.with_rules(File::Rules.fixture()))
            .and_then(|definitions| definitions.with_arsenal(File::Arsenal.fixture()))
            .unwrap_or_else(|error| panic!("{error}"));
    assert_rejected_with(
        File::Ai,
        &File::Ai.fixture(),
        without_combat,
        "file",
        invalid(
            "definitions without a combat file",
            "the definitions of a match, with a combat file",
        ),
    );
}

#[test]
fn ai_plan_entries_can_be_obtained_through_the_build_order() {
    let before = "a building type whose prerequisites are listed before it";
    let obtainable = "a unit type whose producer and prerequisites are listed in the build order";
    let armed = "an armed unit type with production terms";
    let cases: Vec<(Edits, &str, &str, &str)> = vec![
        // A prerequisite of a building is listed before it: a building of the economy file,
        // and a member of a prerequisite group of the arsenal (named by its first member).
        (
            &[(
                r#""allied_barracks", "allied_factory", "allied_power"],"#,
                r#""allied_factory", "allied_barracks", "allied_power"],"#,
            )],
            "plans[0].build_order[2]",
            r#""allied_factory", whose prerequisite "allied_barracks" is not listed before it"#,
            before,
        ),
        (
            &[(
                r#"build_order: ["allied_power", "allied_refinery","#,
                r#"build_order: ["allied_refinery", "allied_power","#,
            )],
            "plans[0].build_order[0]",
            r#""allied_refinery", whose prerequisite "allied_power" is not listed before it"#,
            before,
        ),
        (
            &[(
                r#"build_order: ["soviet_power", "soviet_refinery","#,
                r#"build_order: ["soviet_refinery", "soviet_power","#,
            )],
            "plans[1].build_order[0]",
            r#""soviet_refinery", whose prerequisite "allied_power" is not listed before it"#,
            before,
        ),
        // A building of the other side needs the other Construction Yard.
        (
            &[(
                r#""allied_factory", "allied_power"],"#,
                r#""allied_factory", "soviet_power"],"#,
            )],
            "plans[0].build_order[4]",
            r#""soviet_power", whose prerequisite "soviet_yard" is not listed before it"#,
            before,
        ),
        (
            &[(
                r#""soviet_factory", "soviet_power"],"#,
                r#""soviet_factory", "soviet_power", "storm_device"],"#,
            )],
            "plans[1].build_order[5]",
            r#""storm_device", whose prerequisite "allied_radar" is not listed before it"#,
            before,
        ),
        // The power building produces power and needs nothing the side does not start with.
        (
            &[("power: \"allied_power\",", "power: \"allied_barracks\",")],
            "plans[0].power",
            r#""allied_barracks", which produces no power"#,
            "a building type that produces power",
        ),
        (
            &[("power: \"soviet_power\",", "power: \"allied_power\",")],
            "plans[1].power",
            r#""allied_power", which has prerequisites"#,
            "a building type that produces power and has no prerequisites",
        ),
        (
            &[("power: \"allied_power\",", "power: \"soviet_power\",")],
            "plans[0].power",
            r#""soviet_power", which has prerequisites"#,
            "a building type that produces power and has no prerequisites",
        ),
        // The harvester is a harvester type whose producer the build order lists; a factory
        // of the other side does not produce it.
        (
            &[("harvester: \"miner_a\",", "harvester: \"tank\",")],
            "plans[0].harvester",
            r#""tank""#,
            "a harvester unit type with production terms",
        ),
        (
            &[(
                r#""allied_barracks", "allied_factory", "allied_power"],"#,
                r#""allied_barracks", "allied_power"],"#,
            )],
            "plans[0].harvester",
            r#""miner_a", whose producer "allied_factory" is not listed in the build order"#,
            obtainable,
        ),
        (
            &[("harvester: \"miner_a\",", "harvester: \"miner_s\",")],
            "plans[0].harvester",
            r#""miner_s", whose producer "soviet_factory" is not listed in the build order"#,
            obtainable,
        ),
        // Every army unit type is armed and has its producer and its prerequisites in the
        // build order.
        (
            &[(ALLIED_ARMY, r#"army: ["tank", "engineer", "rifleman"],"#)],
            "plans[0].army[1]",
            r#""engineer""#,
            armed,
        ),
        (
            &[(ALLIED_ARMY, r#"army: ["miner_a"],"#)],
            "plans[0].army[0]",
            r#""miner_a""#,
            armed,
        ),
        // The Soviet miner is armed, and still no army unit: a harvester mines.
        (
            &[(SOVIET_ARMY, r#"army: ["miner_s"],"#)],
            "plans[1].army[0]",
            r#""miner_s", which is a harvester"#,
            "an armed unit type that does not harvest",
        ),
        (
            &[(ALLIED_ARMY, r#"army: ["tank", "militia", "rifleman"],"#)],
            "plans[0].army[1]",
            r#""militia", whose producer "soviet_barracks" is not listed in the build order"#,
            obtainable,
        ),
        (
            &[(SOVIET_ARMY, r#"army: ["heavy", "conscript", "tank"],"#)],
            "plans[1].army[2]",
            r#""tank", whose producer "allied_factory" is not listed in the build order"#,
            obtainable,
        ),
        (
            &[(ALLIED_ARMY, r#"army: ["tank", "sniper", "rifleman"],"#)],
            "plans[0].army[1]",
            r#""sniper", whose prerequisite "allied_radar" is not listed in the build order"#,
            obtainable,
        ),
    ];
    for (index, (edits, field, found, expected)) in cases.into_iter().enumerate() {
        let path = variant(File::Ai, &format!("ai_obtainable_{index}"), edits);
        assert_rejected(File::Ai, &path, field, invalid(found, expected));
    }
}

#[test]
fn the_army_of_a_plan_can_damage_every_building_type() {
    // The bullets of the fixture harm concrete a little, so every armed unit can destroy every
    // building. With the combat file of this context they do not, and an army of riflemen or
    // of conscripts alone could never end a match between computer players.
    let name = "combat_bullets_spare_concrete";
    let combat = std::fs::read_to_string(fixtures().join("combat.ron")).expect("fixture exists");
    let combat = edit(
        name,
        &combat,
        r#"(armour: "concrete", percent: 10)"#,
        r#"(armour: "concrete", percent: 0)"#,
    );
    let combat = temp_file(&format!("{name}.ron"), combat.as_bytes());
    let context = load_match_definitions(
        fixtures().join("units.ron"),
        fixtures().join("economy.ron"),
        &combat,
    )
    .and_then(|definitions| definitions.with_rules(File::Rules.fixture()))
    .and_then(|definitions| definitions.with_arsenal(File::Arsenal.fixture()))
    .unwrap_or_else(|error| panic!("{error}"));
    // The first building type of the economy file stands for all of them: all are concrete.
    let unharmed = DataErrorKind::MissingEntry {
        kind: "unit able to damage building".into(),
        id: "allied_yard".into(),
    };
    let riflemen = variant(
        File::Ai,
        "ai_riflemen_only",
        &[(ALLIED_ARMY, r#"army: ["rifleman"],"#)],
    );
    assert_rejected_with(
        File::Ai,
        &riflemen,
        context.clone(),
        "plans[0].army",
        unharmed.clone(),
    );
    let conscripts = variant(
        File::Ai,
        "ai_conscripts_only",
        &[(SOVIET_ARMY, r#"army: ["conscript", "militia"],"#)],
    );
    assert_rejected_with(
        File::Ai,
        &conscripts,
        context.clone(),
        "plans[1].army",
        unharmed,
    );
    // The armies of the fixture have tanks, whose shells harm concrete; and with the fixture's
    // own combat file the riflemen and the conscripts are enough.
    as_ai(load_with(File::Ai, &File::Ai.fixture(), context));
    assert_eq!(as_ai(load(File::Ai, &riflemen)).plans[0].army.len(), 1);
    assert_eq!(as_ai(load(File::Ai, &conscripts)).plans[1].army.len(), 2);
}

#[test]
fn ai_plan_entries_are_built_by_every_country_of_the_side() {
    let every = "an item every country of the plan's side can build on its own";
    const WITH_RADAR: (&str, &str) = (
        r#""allied_factory", "allied_power"],"#,
        r#""allied_factory", "allied_power", "allied_radar"],"#,
    );
    let cases: Vec<(Edits, &str, &str)> = vec![
        // The sniper is the special of country alpha: with a radar in the build order alpha
        // can obtain it, and beta, of the same side, cannot.
        (
            &[
                WITH_RADAR,
                (ALLIED_ARMY, r#"army: ["tank", "sniper", "rifleman"],"#),
            ],
            "plans[0].army[1]",
            r#""sniper", which the country "beta" cannot build"#,
        ),
        // The raider is forbidden to alpha.
        (
            &[(ALLIED_ARMY, r#"army: ["tank", "tank", "raider"],"#)],
            "plans[0].army[2]",
            r#""raider", which the country "alpha" cannot build"#,
        ),
        // Items above the tech level of the rules and items that cannot be queued at all.
        (
            &[(ALLIED_ARMY, r#"army: ["advanced", "tank"],"#)],
            "plans[0].army[0]",
            r#""advanced", which the country "alpha" cannot build"#,
        ),
        (
            &[(ALLIED_ARMY, r#"army: ["tank", "prototype"],"#)],
            "plans[0].army[1]",
            r#""prototype", which the country "alpha" cannot build"#,
        ),
        // A Construction Yard has no prerequisites, but no country builds one (tech level -1):
        // the other side's in the Allied plan, and its own in the Soviet plan.
        (
            &[(
                r#""allied_factory", "allied_power"],"#,
                r#""allied_factory", "allied_power", "soviet_yard"],"#,
            )],
            "plans[0].build_order[5]",
            r#""soviet_yard", which the country "alpha" cannot build"#,
        ),
        (
            &[(
                r#"build_order: ["soviet_power","#,
                r#"build_order: ["soviet_yard", "soviet_power","#,
            )],
            "plans[1].build_order[0]",
            r#""soviet_yard", which the country "gamma" cannot build"#,
        ),
    ];
    for (index, (edits, field, found)) in cases.into_iter().enumerate() {
        let path = variant(File::Ai, &format!("ai_country_{index}"), edits);
        assert_rejected(File::Ai, &path, field, invalid(found, every));
    }
    // With the rules of the Allied side and country alpha alone, the sniper is in the tree of
    // every country of the side.
    let name = "rules_alpha_only";
    let rules = repeat_line(name, &File::Rules.text(), BETA_COUNTRY, 0);
    let rules = temp_file(&format!("{name}.ron"), rules.as_bytes());
    let context = definitions_of(
        &fixtures().join("economy.ron"),
        Some(&rules),
        Some(&File::Arsenal.fixture()),
    );
    let path = variant(
        File::Ai,
        "ai_country_alpha_only",
        &[
            WITH_RADAR,
            (ALLIED_ARMY, r#"army: ["tank", "sniper", "rifleman"],"#),
        ],
    );
    let ai = as_ai(load_with(File::Ai, &path, context.clone()));
    assert_eq!(
        ai.plans[0].army,
        ["tank", "sniper", "rifleman"].map(|id| unit(&context, id))
    );
}

#[test]
fn ai_references_and_difficulty_values_are_checked() {
    let references = [
        (
            "build_order: [\"allied_power\",",
            "build_order: [\"castle\",",
            "plans[0].build_order[0]",
            unknown("building", "castle"),
        ),
        (
            r#""allied_barracks", "allied_factory", "allied_power"],"#,
            r#""tank", "allied_factory", "allied_power"],"#,
            "plans[0].build_order[2]",
            unknown("building", "tank"),
        ),
        (
            "power: \"soviet_power\",",
            "power: \"castle\",",
            "plans[1].power",
            unknown("building", "castle"),
        ),
        (
            "harvester: \"miner_s\",",
            "harvester: \"dragon\",",
            "plans[1].harvester",
            unknown("unit", "dragon"),
        ),
        (
            "army: [\"heavy\",",
            "army: [\"dragon\",",
            "plans[1].army[0]",
            unknown("unit", "dragon"),
        ),
        (
            ALLIED_ARMY,
            r#"army: ["tank", "tank", "allied_barracks"],"#,
            "plans[0].army[2]",
            unknown("unit", "allied_barracks"),
        ),
        (
            "            id: \"brutal\",",
            "            id: \"easy\",",
            "difficulties[1].id",
            DataErrorKind::DuplicateId("easy".into()),
        ),
    ];
    for (index, (from, to, field, kind)) in references.into_iter().enumerate() {
        let path = variant(File::Ai, &format!("ai_reference_{index}"), &[(from, to)]);
        assert_rejected(File::Ai, &path, field, kind);
    }
    for (index, (from, to, field, found)) in [
        (
            "think_interval: 20",
            "think_interval: 0",
            "difficulties[0].think_interval",
            "0",
        ),
        (
            "think_interval: 20",
            "think_interval: 1201",
            "difficulties[0].think_interval",
            "1201",
        ),
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
        (
            "max_army: 6",
            "max_army: 0",
            "difficulties[0].max_army",
            "0",
        ),
        (
            "max_army: 6",
            "max_army: 201",
            "difficulties[0].max_army",
            "201",
        ),
        (
            "wave_size: 4",
            "wave_size: 0",
            "difficulties[0].wave_size",
            "0",
        ),
        (
            "wave_size: 4",
            "wave_size: 7",
            "difficulties[0].wave_size",
            "7",
        ),
        (
            "first_wave: 1200",
            "first_wave: -1",
            "difficulties[0].first_wave",
            "-1",
        ),
        (
            "first_wave: 1200",
            "first_wave: 72001",
            "difficulties[0].first_wave",
            "72001",
        ),
        (
            "wave_patience: 1200",
            "wave_patience: 0",
            "difficulties[0].wave_patience",
            "0",
        ),
        (
            "wave_patience: 1200",
            "wave_patience: 72001",
            "difficulties[0].wave_patience",
            "72001",
        ),
        (
            "defence_radius: 8",
            "defence_radius: -1",
            "difficulties[1].defence_radius",
            "-1",
        ),
        (
            "defence_radius: 8",
            "defence_radius: 33",
            "difficulties[1].defence_radius",
            "33",
        ),
        (
            "            id: \"easy\",",
            "            id: \"Easy!\",",
            "difficulties[0].id",
            "\"Easy!\"",
        ),
        (
            SOVIET_ORDER,
            "build_order: [],",
            "plans[1].build_order",
            "an empty list",
        ),
        (SOVIET_ARMY, "army: [],", "plans[1].army", "an empty list"),
    ]
    .into_iter()
    .enumerate()
    {
        let path = variant(File::Ai, &format!("ai_range_{index}"), &[(from, to)]);
        assert_invalid(File::Ai, &path, field, found);
    }
}

#[test]
fn the_ai_file_needs_a_plan_for_the_side_of_every_faction() {
    let definitions = definitions();
    let skirmish = as_skirmish(load(File::Skirmish, &File::Skirmish.fixture()));
    let fixture = File::Ai.fixture();
    let ai = as_ai(load(File::Ai, &fixture));
    check_ai_factions(&fixture, &ai, &skirmish, &definitions)
        .unwrap_or_else(|error| panic!("{error}"));
    // Without the Soviet plan the AI file loads, since a file need not have a plan for every
    // side, but the Soviet country of the skirmish file has none to follow.
    let name = "ai_no_soviet_plan";
    let path = temp_file(
        &format!("{name}.ron"),
        repeat_block(name, &File::Ai.text(), SOVIET_PLAN, 0).as_bytes(),
    );
    let allied_only = as_ai(load(File::Ai, &path));
    assert_eq!(allied_only.plans.len(), 1);
    assert!(allied_only.plan(Some(Side::Soviet)).is_none());
    assert!(allied_only.plan_for(&skirmish.factions[2]).is_none());
    let error = check_ai_factions(&path, &allied_only, &skirmish, &definitions)
        .expect_err("no plan for the Soviet side");
    assert_error(
        &error,
        &path,
        "plans",
        &DataErrorKind::MissingEntry {
            kind: "plan for the side of faction \"gamma\"".into(),
            id: "soviet".into(),
        },
    );
    // Against the Allied countries alone the same file is complete.
    let mut allied_countries = skirmish.clone();
    allied_countries.factions.truncate(2);
    check_ai_factions(&path, &allied_only, &allied_countries, &definitions)
        .unwrap_or_else(|error| panic!("{error}"));
    // A faction without a side needs the plan of the side `none`, which the fixture lacks.
    let sideless = as_skirmish(load(
        File::Skirmish,
        &sideless_variant("skirmish_sideless_for_ai", &[]),
    ));
    let error = check_ai_factions(&fixture, &ai, &sideless, &definitions)
        .expect_err("no plan for the factions without a side");
    assert_error(
        &error,
        &fixture,
        "plans",
        &DataErrorKind::MissingEntry {
            kind: "plan for the side of faction \"alpha\"".into(),
            id: "none".into(),
        },
    );
}

// --- Negative: files ------------------------------------------------------------------------

#[test]
fn truncations_of_the_fixtures_are_rejected_without_panic() {
    for file in File::ALL {
        let text = file.text();
        let end = text.rfind(')').expect("closing parenthesis");
        let path = temp_dir().join(format!("{file:?}_prefix.ron"));
        let context = file.context();
        // Every prefix that ends before the parenthesis that closes the file's struct.
        for cut in 0..=end {
            std::fs::write(&path, &text.as_bytes()[..cut]).expect("prefix");
            let error = load_with(file, &path, context.clone())
                .expect_err("a truncated file must be rejected");
            assert_eq!(error.file, path);
            assert!(!error.field.is_empty());
            assert!(
                !matches!(error.kind, DataErrorKind::Parse(_)),
                "{file:?}, {cut} bytes: ron rejected a file the checker accepted: {error}"
            );
        }
    }
}
