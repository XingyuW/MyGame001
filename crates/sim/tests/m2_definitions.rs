//! M2 tests: unit definitions from RON (PLAN.md sections 5 and 7; invariants I8, I10, I11;
//! docs/DECISIONS.md D18).
//!
//! Every malformed file must be rejected with a typed `DataError` naming the file, the field,
//! and the reason, before `ron` sees it where it is outside the supported subset. The tests feed
//! hostile files to the loader and are bounded as in D15: this test binary's global allocator
//! refuses allocations beyond [`MEMORY_BUDGET`] (the process then aborts, failing the run), and
//! every load runs on a worker thread whose result must arrive within [`TIME_LIMIT`].
//!
//! Variants are derived from the committed fixture `tests/fixtures/data/units.ron` by exact text
//! edits and written to Cargo's temporary directory for integration tests.

use std::alloc::{GlobalAlloc, Layout, System};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use sim::{
    DataError, DataErrorKind, Definitions, MAX_DATA_FILE_BYTES, MAX_NAME_LEN, MAX_STRING_BYTES,
    MAX_UNIT_SPEED, MAX_UNIT_TYPES, UnitDef, UnitKind, load_unit_definitions,
};

// --- Bounds ---------------------------------------------------------------------------------

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

/// Loads `path` on a worker thread, failing the test if the loader panics or takes longer than
/// [`TIME_LIMIT`].
fn load(path: &Path) -> Result<Definitions, DataError> {
    let owned = path.to_owned();
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name(format!("load {}", path.display()))
        .spawn(move || {
            let _ = sender.send(load_unit_definitions(&owned));
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

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data/units.ron")
}

fn fixture_text() -> String {
    std::fs::read_to_string(fixture()).expect("fixture exists")
}

fn temp_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("m2_definitions");
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

/// A units file with the given unit entries (each the inside of a struct).
fn units_file(name: &str, entries: &[String]) -> PathBuf {
    let body: Vec<String> = entries
        .iter()
        .map(|entry| format!("        ({entry}),"))
        .collect();
    let text = format!(
        "(\n    schema_version: 1,\n    units: [\n{}\n    ],\n)\n",
        body.join("\n")
    );
    temp_file(&format!("{name}.ron"), text.as_bytes())
}

fn unit_entry(id: &str, speed: &str, sprite: &str) -> String {
    format!("id: {id}, speed: {speed}, sprite: {sprite}")
}

fn walker() -> UnitDef {
    UnitDef {
        id: "walker".into(),
        speed: 32,
        sprite: "walker".into(),
    }
}

fn runner() -> UnitDef {
    UnitDef {
        id: "runner".into(),
        speed: 100,
        sprite: "runner".into(),
    }
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

/// Asserts a syntax error at `field` and the given line and column, with `expected` among the
/// expected tokens.
fn assert_syntax(path: &Path, field: &str, line: usize, column: usize, expected: &str) {
    let error = load(path).expect_err("the file must be rejected");
    assert_eq!(error.field, field, "{error}");
    match &error.kind {
        DataErrorKind::Syntax {
            line: l,
            column: c,
            expected: e,
            ..
        } => {
            assert_eq!((*l, *c), (line, column), "{error}");
            assert!(e.contains(expected), "{error}");
        }
        other => panic!("expected a syntax error, found {other:?}: {error}"),
    }
    assert!(
        error
            .to_string()
            .contains(&format!("line {line}, column {column}"))
    );
}

fn invalid(found: &str, expected: &str) -> DataErrorKind {
    DataErrorKind::InvalidValue {
        found: found.into(),
        expected: expected.into(),
    }
}

// --- Positive -------------------------------------------------------------------------------

#[test]
fn the_fixture_loads_with_every_field() {
    let definitions = load_ok(&fixture());
    assert_eq!(definitions.units(), [walker(), runner()]);
    assert_eq!(definitions.unit_kind("walker"), Some(UnitKind(0)));
    assert_eq!(definitions.unit_kind("runner"), Some(UnitKind(1)));
    assert_eq!(definitions.unit_kind("tank"), None);
    assert_eq!(definitions.unit(UnitKind(1)), Some(&runner()));
    assert_eq!(definitions.unit(UnitKind(2)), None);
}

#[test]
fn layout_variations_inside_the_subset_load_to_the_same_definitions() {
    let expected = load_ok(&fixture());
    let cases: [(&str, &str); 6] = [
        (
            "one_line",
            "(schema_version:1,units:[(id:\"walker\",speed:32,sprite:\"walker\"),(id:\"runner\",speed:100,sprite:\"runner\")])",
        ),
        (
            "no_trailing_commas",
            "(schema_version: 1, units: [(id: \"walker\", speed: 32, sprite: \"walker\"), (id: \"runner\", speed: 100, sprite: \"runner\")])",
        ),
        (
            "field_order",
            "(units: [(sprite: \"walker\", speed: 32, id: \"walker\"), (speed: 100, id: \"runner\", sprite: \"runner\")], schema_version: 1)",
        ),
        (
            "comments_and_tabs",
            "// header\n(\t// a\n\tschema_version: 1, // b\n\tunits: [ // c\n\t\t(id: \"walker\", speed: 32, sprite: \"walker\"),\n\t\t(id: \"runner\", speed: 100, sprite: \"runner\"),\n\t],\n)\n// end",
        ),
        (
            "crlf",
            "(\r\n    schema_version: 1,\r\n    units: [\r\n        (id: \"walker\", speed: 32, sprite: \"walker\"),\r\n        (id: \"runner\", speed: 100, sprite: \"runner\"),\r\n    ],\r\n)\r\n",
        ),
        (
            "comment_at_end_without_newline",
            "(schema_version: 1, units: [(id: \"walker\", speed: 32, sprite: \"walker\"), (id: \"runner\", speed: 100, sprite: \"runner\")]) // no newline",
        ),
    ];
    for (name, text) in cases {
        let path = temp_file(&format!("{name}.ron"), text.as_bytes());
        assert_eq!(load_ok(&path), expected, "{name}");
    }
}

#[test]
fn value_limits_are_inclusive() {
    let longest = format!("\"{}\"", "a".repeat(MAX_NAME_LEN));
    let path = units_file(
        "limits",
        &[
            unit_entry("\"a\"", "1", "\"s\""),
            unit_entry(&longest, &MAX_UNIT_SPEED.to_string(), &longest),
        ],
    );
    let definitions = load_ok(&path);
    assert_eq!(definitions.units()[0].speed, 1);
    assert_eq!(definitions.units()[1].speed, MAX_UNIT_SPEED);
    assert_eq!(definitions.units()[1].id.len(), MAX_NAME_LEN);
    // The largest number of unit types.
    let entries: Vec<String> = (0..MAX_UNIT_TYPES)
        .map(|i| unit_entry(&format!("\"u{i}\""), "10", "\"s\""))
        .collect();
    assert_eq!(
        load_ok(&units_file("most_units", &entries)).units().len(),
        MAX_UNIT_TYPES
    );
    // A file of exactly the size limit, padded with a comment.
    let mut text = fixture_text().into_bytes();
    text.extend_from_slice(b"//");
    text.resize(MAX_DATA_FILE_BYTES as usize, b'x');
    let path = temp_file("largest.ron", &text);
    assert_eq!(load_ok(&path), load_ok(&fixture()));
}

// --- Negative: schema -----------------------------------------------------------------------

#[test]
fn unknown_missing_and_duplicate_fields_are_rejected() {
    assert_rejected(
        &variant(
            "unknown_top",
            &[("    units: [", "    extra: 1,\n    units: [")],
        ),
        "extra",
        DataErrorKind::UnknownField,
    );
    assert_rejected(
        &variant(
            "unknown_unit",
            &[("speed: 32,", "speed: 32,\n            armour: 2,")],
        ),
        "units[0].armour",
        DataErrorKind::UnknownField,
    );
    assert_rejected(
        &variant("missing_speed", &[("            speed: 100,\n", "")]),
        "units[1].speed",
        DataErrorKind::MissingField,
    );
    assert_rejected(
        &variant("missing_version", &[("    schema_version: 1,\n", "")]),
        "schema_version",
        DataErrorKind::MissingField,
    );
    assert_rejected(
        &temp_file("missing_units.ron", b"(schema_version: 1)"),
        "units",
        DataErrorKind::MissingField,
    );
    assert_rejected(
        &variant(
            "duplicate_speed",
            &[("speed: 32,", "speed: 32,\n            speed: 33,")],
        ),
        "units[0].speed",
        DataErrorKind::DuplicateField,
    );
    assert_rejected(
        &temp_file("empty_struct.ron", b"()"),
        "schema_version",
        DataErrorKind::MissingField,
    );
}

#[test]
fn values_of_the_wrong_kind_are_syntax_errors_at_their_field() {
    assert_syntax(
        &variant("string_speed", &[("speed: 32,", "speed: \"fast\",")]),
        "units[0].speed",
        8,
        20,
        "an integer",
    );
    assert_syntax(
        &variant("int_id", &[("id: \"runner\",", "id: 7,")]),
        "units[1].id",
        12,
        17,
        "a string",
    );
    assert_syntax(
        &variant(
            "string_version",
            &[("schema_version: 1,", "schema_version: \"1\",")],
        ),
        "schema_version",
        4,
        21,
        "an integer",
    );
    assert_syntax(
        &variant("units_not_a_list", &[("    units: [", "    units: (")]),
        "units",
        5,
        12,
        "a list",
    );
    // A tuple where a unit struct is expected: the first element is not a field name.
    assert_syntax(
        &units_file("tuple_unit", &["\"walker\", 32, \"walker\"".into()]),
        "units[0]",
        4,
        10,
        "a field name",
    );
}

#[test]
fn schema_versions_other_than_1_are_rejected() {
    for version in ["0", "2", "-1", "9223372036854775807"] {
        assert_rejected(
            &variant(
                &format!("version_{version}"),
                &[("schema_version: 1,", &format!("schema_version: {version},"))],
            ),
            "schema_version",
            DataErrorKind::UnsupportedSchemaVersion {
                found: version.parse().expect("i64"),
                expected: 1,
            },
        );
    }
    // A file of another version is reported as such even if its other fields differ.
    assert_rejected(
        &temp_file(
            "version_2_new_fields.ron",
            b"(schema_version: 2, units: [(id: \"a\", speed: 1, sprite: \"a\", armour: 3)])",
        ),
        "schema_version",
        DataErrorKind::UnsupportedSchemaVersion {
            found: 2,
            expected: 1,
        },
    );
}

#[test]
fn unit_values_outside_their_ranges_are_rejected() {
    let speed_range = format!("1 to {MAX_UNIT_SPEED} subcells per tick");
    for speed in ["0", "-1", "129", "65536", "9223372036854775807"] {
        assert_rejected(
            &units_file(
                &format!("speed_{speed}"),
                &[unit_entry("\"a\"", speed, "\"a\"")],
            ),
            "units[0].speed",
            invalid(speed, &speed_range),
        );
    }
    assert_rejected(
        &units_file(
            "speed_overflow",
            &[unit_entry("\"a\"", "9223372036854775808", "\"a\"")],
        ),
        "units[0].speed",
        invalid("9223372036854775808", "a 64-bit signed integer"),
    );
    let name_rule =
        format!("1 to {MAX_NAME_LEN} characters from a-z, 0-9, and _, starting with a letter");
    let too_long = "a".repeat(MAX_NAME_LEN + 1);
    for id in [
        "",
        "Tank",
        "1tank",
        "tank-1",
        "tank 1",
        "_tank",
        too_long.as_str(),
    ] {
        assert_rejected(
            &units_file("bad_id", &[unit_entry(&format!("\"{id}\""), "5", "\"a\"")]),
            "units[0].id",
            invalid(&format!("{id:?}"), &name_rule),
        );
    }
    for sprite in ["../x", "a/b", "x.png", "C", ""] {
        assert_rejected(
            &units_file(
                "bad_sprite",
                &[unit_entry("\"a\"", "5", &format!("\"{sprite}\""))],
            ),
            "units[0].sprite",
            invalid(&format!("{sprite:?}"), &name_rule),
        );
    }
    let longest_string = "a".repeat(MAX_STRING_BYTES + 1);
    assert_rejected(
        &units_file(
            "long_string",
            &[unit_entry(&format!("\"{longest_string}\""), "5", "\"a\"")],
        ),
        "units[0].id",
        invalid(
            &format!("of {} bytes", MAX_STRING_BYTES + 1),
            &format!("a string of at most {MAX_STRING_BYTES} bytes"),
        ),
    );
}

#[test]
fn unit_lists_must_be_non_empty_bounded_and_unique() {
    assert_rejected(
        &temp_file("no_units.ron", b"(schema_version: 1, units: [])"),
        "units",
        invalid(
            "an empty list",
            &format!("1 to {MAX_UNIT_TYPES} unit types"),
        ),
    );
    let entries: Vec<String> = (0..=MAX_UNIT_TYPES)
        .map(|i| unit_entry(&format!("\"u{i}\""), "10", "\"s\""))
        .collect();
    assert_rejected(
        &units_file("too_many_units", &entries),
        "units",
        DataErrorKind::TooManyEntries {
            max: MAX_UNIT_TYPES,
        },
    );
    assert_rejected(
        &variant("duplicate_id", &[("id: \"runner\",", "id: \"walker\",")]),
        "units[1].id",
        DataErrorKind::DuplicateId("walker".into()),
    );
}

// --- Negative: syntax outside the subset ----------------------------------------------------

/// File name, contents, field, line, column, and part of the expected tokens.
type SyntaxCase = (
    &'static str,
    &'static [u8],
    &'static str,
    usize,
    usize,
    &'static str,
);

#[test]
fn ron_features_outside_the_subset_are_rejected_before_ron_parses() {
    let cases: [SyntaxCase; 18] = [
        // `ron` extensions are switched on by an attribute at the top of the file.
        (
            "extension",
            b"#![enable(implicit_some)]\n(schema_version: 1, units: [])",
            "file",
            1,
            1,
            "a value or punctuation",
        ),
        (
            "block_comment",
            b"/* c */ (schema_version: 1)",
            "file",
            1,
            1,
            "\"//\" comment",
        ),
        (
            "struct_name",
            b"Units(schema_version: 1)",
            "file",
            1,
            1,
            "a struct",
        ),
        (
            "float",
            b"(schema_version: 1.0)",
            "schema_version",
            1,
            18,
            "decimal integer",
        ),
        (
            "hex",
            b"(schema_version: 0x1)",
            "schema_version",
            1,
            18,
            "decimal integer",
        ),
        (
            "underscore",
            b"(schema_version: 1_0)",
            "schema_version",
            1,
            18,
            "decimal integer",
        ),
        (
            "suffix",
            b"(schema_version: 1u8)",
            "schema_version",
            1,
            18,
            "decimal integer",
        ),
        (
            "leading_zero",
            b"(schema_version: 01)",
            "schema_version",
            1,
            18,
            "decimal integer",
        ),
        (
            "plus",
            b"(schema_version: +1)",
            "schema_version",
            1,
            18,
            "a value or punctuation",
        ),
        (
            "minus_alone",
            b"(schema_version: -)",
            "schema_version",
            1,
            18,
            "decimal integer",
        ),
        (
            "boolean",
            b"(schema_version: true)",
            "schema_version",
            1,
            18,
            "an integer",
        ),
        (
            "option",
            b"(schema_version: Some(1))",
            "schema_version",
            1,
            18,
            "an integer",
        ),
        (
            "map",
            b"(schema_version: {1: 2})",
            "schema_version",
            1,
            18,
            "a value or punctuation",
        ),
        (
            "char",
            b"(schema_version: 'a')",
            "schema_version",
            1,
            18,
            "a value or punctuation",
        ),
        (
            "escape",
            b"(schema_version: 1, units: [(id: \"a\\n\")])",
            "units[0].id",
            1,
            36,
            "without escape sequences",
        ),
        (
            "raw_string",
            b"(schema_version: 1, units: [(id: r\"a\")])",
            "units[0].id",
            1,
            34,
            "a string",
        ),
        (
            "multi_line_string",
            b"(schema_version: 1, units: [(id: \"a\nb\")])",
            "units[0].id",
            1,
            36,
            "on one line",
        ),
        (
            "missing_comma",
            b"(schema_version: 1 units: [])",
            "file",
            1,
            20,
            "\",\" or \")\"",
        ),
    ];
    for (name, text, field, line, column, expected) in cases {
        let path = temp_file(&format!("syntax_{name}.ron"), text);
        assert_syntax(&path, field, line, column, expected);
    }
}

#[test]
fn bytes_outside_printable_ascii_are_rejected() {
    let fixture = fixture_text();
    let cases: [(&str, Vec<u8>, usize, usize); 4] = [
        (
            "bom",
            [b"\xef\xbb\xbf".as_slice(), fixture.as_bytes()].concat(),
            1,
            1,
        ),
        (
            "umlaut_in_comment",
            fixture.replacen("fixture", "fixtür", 1).into_bytes(),
            1,
            11,
        ),
        (
            "nul",
            fixture.replacen("walker", "wal\0ker", 1).into_bytes(),
            7,
            21,
        ),
        (
            "vertical_tab",
            fixture
                .replacen("    units", "\x0b   units", 1)
                .into_bytes(),
            5,
            1,
        ),
    ];
    for (name, bytes, line, column) in cases {
        let path = temp_file(&format!("bytes_{name}.ron"), &bytes);
        assert_syntax(&path, "file", line, column, "printable ASCII");
    }
}

#[test]
fn content_after_the_root_and_truncated_files_are_rejected() {
    let text = fixture_text();
    assert_syntax(
        &temp_file("after_root.ron", format!("{text}()").as_bytes()),
        "file",
        18,
        1,
        "the end of the file",
    );
    // Every prefix of the fixture shorter than its last closing parenthesis is rejected with a
    // typed error; none panics.
    let end = text.rfind(')').expect("closing parenthesis");
    for cut in 0..=end {
        let path = temp_file("prefix.ron", &text.as_bytes()[..cut]);
        let error = load(&path).expect_err("a truncated file must be rejected");
        assert_eq!(error.file, path);
        assert!(!error.field.is_empty());
    }
}

#[test]
fn deeply_nested_input_is_rejected_without_recursion() {
    for (name, open) in [("lists", "["), ("structs", "(")] {
        let mut text = String::from("(schema_version: 1, units: ");
        text.push_str(&open.repeat(60_000));
        let path = temp_file(&format!("nested_{name}.ron"), text.as_bytes());
        let error = load(&path).expect_err("rejected");
        assert!(
            matches!(error.kind, DataErrorKind::Syntax { .. }),
            "{name}: {error}"
        );
    }
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

    let directory = temp_dir().join("a_directory.ron");
    std::fs::create_dir_all(&directory).expect("directory");
    assert_rejected(
        &directory,
        "file",
        DataErrorKind::Read("not a regular file".into()),
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
    // A 1 GiB sparse file is rejected without being read whole.
    let sparse = temp_dir().join("sparse.ron");
    std::fs::File::create(&sparse)
        .and_then(|file| file.set_len(1 << 30))
        .expect("sparse file");
    assert_rejected(
        &sparse,
        "file",
        DataErrorKind::FileTooLarge {
            max_bytes: MAX_DATA_FILE_BYTES,
        },
    );
    std::fs::remove_file(&sparse).expect("remove the sparse file");

    // A named pipe would block a reader until a writer appears (M1 review 3, P2-1); it is
    // rejected before it is opened.
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

const SNIPPETS: [&str; 24] = [
    "(",
    ")",
    "[",
    "]",
    ",",
    ":",
    "\"",
    "//",
    "/*",
    "#",
    "-",
    "0",
    "99999999999999999999",
    "speed",
    "id: \"x\"",
    "units",
    "schema_version",
    "\\",
    "r#",
    "Some(",
    "{",
    "1.5",
    "\n",
    "\u{e9}",
];

#[test]
fn seeded_mutations_of_the_fixture_load_or_return_a_typed_error() {
    let base = fixture_text().into_bytes();
    let mut rng = SplitMix64(0x5eed_da7a);
    let path = temp_dir().join("mutation.ron");
    let mut loaded = 0;
    for case in 0..4_000 {
        let mut bytes = base.clone();
        for _ in 0..1 + rng.below(3) {
            let at = rng.below(bytes.len() + 1);
            match rng.below(5) {
                0 => {
                    let snippet = SNIPPETS[rng.below(SNIPPETS.len())].as_bytes();
                    let repeat = if rng.below(10) == 0 { 2_000 } else { 1 };
                    let insert: Vec<u8> = snippet.repeat(repeat);
                    bytes.splice(at..at, insert);
                }
                1 => {
                    let end = (at + rng.below(20)).min(bytes.len());
                    bytes.drain(at..end);
                }
                2 => {
                    let end = (at + rng.below(40)).min(bytes.len());
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
                let units = definitions.units();
                assert!((1..=MAX_UNIT_TYPES).contains(&units.len()), "case {case}");
                for unit in units {
                    assert!((1..=MAX_UNIT_SPEED).contains(&unit.speed), "case {case}");
                    assert!((1..=MAX_NAME_LEN).contains(&unit.id.len()), "case {case}");
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
