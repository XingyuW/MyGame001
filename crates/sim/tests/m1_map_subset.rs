//! M1 remediation round 1: the supported subset of the Tiled format (docs/DECISIONS.md, D13;
//! invariants I8 and I11), with a regression test for each defect of M1 review 1 that concerns
//! the map loader, and a seeded mutation test over the demo map.
//!
//! Every test here feeds hostile input to `sim::load_map`, and before the remediation some of
//! these inputs exhausted the machine's memory (review P0-3) or overflowed the stack (P0-2). The
//! tests are therefore bounded:
//!
//! * Memory: this test binary's global allocator refuses any allocation that would raise the
//!   bytes in use above [`MEMORY_BUDGET`]. Rust reports a refused allocation by aborting the
//!   process ("memory allocation of N bytes failed"), so a regression fails the test run instead
//!   of growing until the machine swaps.
//! * Time: every load runs on a worker thread whose result must arrive within [`TIME_LIMIT`]
//!   (the mutation test: [`MUTATION_TIME_LIMIT`] for all cases).
//! * Stack: a stack overflow aborts the test process, which fails the run.
//!
//! Variants are derived from the committed fixtures by exact text edits and written to Cargo's
//! temporary directory for integration tests, under names unique to this file.

use std::alloc::{GlobalAlloc, Layout, System};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use sim::{
    LoadedMap, MAX_MAP_FILE_BYTES, MAX_TILESET_TILES, MapError, MapErrorKind, TILE_HEIGHT_PX,
    TILE_WIDTH_PX, load_map,
};

// --- Bounds ---------------------------------------------------------------------------------

/// Upper limit of heap bytes in use by this test process.
const MEMORY_BUDGET: usize = 256 << 20;
/// Upper limit for one load.
const TIME_LIMIT: Duration = Duration::from_secs(30);
/// Upper limit for all cases of the mutation test together.
const MUTATION_TIME_LIMIT: Duration = Duration::from_secs(300);

static IN_USE: AtomicUsize = AtomicUsize::new(0);

/// The system allocator with a budget: an allocation that would exceed [`MEMORY_BUDGET`] fails.
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

/// Runs `work` on a worker thread and returns its result, failing the test if the worker panics
/// or does not finish within `limit`.
fn bounded<T: Send + 'static>(
    what: &str,
    limit: Duration,
    work: impl FnOnce() -> T + Send + 'static,
) -> T {
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name(what.to_owned())
        .spawn(move || {
            let _ = sender.send(work());
        })
        .expect("worker thread");
    match receiver.recv_timeout(limit) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => panic!("{what}: no result within {limit:?}"),
        Err(RecvTimeoutError::Disconnected) => panic!("{what}: the loader panicked"),
    }
}

fn load(path: &Path) -> Result<LoadedMap, MapError> {
    let owned = path.to_owned();
    bounded(&format!("load {}", path.display()), TIME_LIMIT, move || {
        load_map(&owned)
    })
}

// --- Fixtures -------------------------------------------------------------------------------

const IMAGE: &str = r#"<image source="fixture_tiles.png" width="192" height="32"/>"#;
const MAP_SIZE: &str = r#"width="4" height="3" tilewidth="64" tileheight="32""#;
const LAYER_HEAD: &str = r#"<layer id="1" name="terrain" width="4" height="3">"#;
const SCHEMA_PROPERTY: &str = r#"<property name="schema_version" type="int" value="1"/>"#;
const ORE_PROPERTY: &str = r#"<property name="ore" type="int" value="750"/>"#;
const DATA_HEAD: &str = r#"<data encoding="csv">"#;
const VALID_DATA: &str = "1,1,2,3,\n1,2,2,1,\n3,1,1,1\n";
const MAP_END: &str = "</map>";

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/maps")
        .join(name)
}

fn demo_map() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/maps/demo/demo.tmx")
}

fn valid_text() -> String {
    std::fs::read_to_string(fixture("valid_4x3.tmx")).expect("valid fixture exists")
}

fn temp_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("m1_map_subset");
    std::fs::create_dir_all(&dir).expect("temporary directory");
    dir
}

/// Writes `contents` to the temporary file `name` and returns its path.
fn temp_file(name: &str, contents: &[u8]) -> PathBuf {
    let path = temp_dir().join(name);
    std::fs::write(&path, contents).expect("temporary file");
    path
}

/// `text` with each `(from, to)` edit applied; `from` must occur exactly once.
fn edited(text: &str, edits: &[(&str, &str)]) -> String {
    let mut text = text.to_owned();
    for (from, to) in edits {
        assert_eq!(text.matches(from).count(), 1, "fixture edit {from:?}");
        text = text.replacen(from, to, 1);
    }
    text
}

/// The valid fixture with `edits` applied, written to `name.tmx`.
fn variant(name: &str, edits: &[(&str, &str)]) -> PathBuf {
    temp_file(
        &format!("{name}.tmx"),
        edited(&valid_text(), edits).as_bytes(),
    )
}

/// The valid fixture with its tileset moved to the external file `name.tsx` (with `tsx_edits`
/// applied), referenced from `name.tmx` by `reference` (`{source}` stands for `name.tsx`).
/// Returns the map path and the tileset path.
fn external_variant(name: &str, tsx_edits: &[(&str, &str)], reference: &str) -> (PathBuf, PathBuf) {
    let text = valid_text();
    let start = text.find("<tileset").expect("embedded tileset");
    let end = text.find("</tileset>").expect("embedded tileset end") + "</tileset>".len();
    let tileset = text[start..end].replacen(r#" firstgid="1""#, "", 1);
    let tsx = edited(
        &format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{tileset}\n"),
        tsx_edits,
    );
    let tsx_name = format!("{name}.tsx");
    let tsx_path = temp_file(&tsx_name, tsx.as_bytes());
    let map = text.replacen(
        &text[start..end],
        &reference.replace("{source}", &tsx_name),
        1,
    );
    (temp_file(&format!("{name}.tmx"), map.as_bytes()), tsx_path)
}

/// Asserts that loading `path` fails, within the bounds, with `kind` at `field` in `file`, and
/// that the message names the file, the field, and the reason.
fn assert_rejected_in(path: &Path, file: &Path, field: &str, kind: MapErrorKind) {
    let error = load(path).expect_err("the map must be rejected");
    assert_eq!(
        (error.field.as_str(), &error.kind),
        (field, &kind),
        "{error}"
    );
    assert_eq!(error.file, file, "{error}");
    let message = error.to_string();
    for part in [
        file.display().to_string(),
        field.to_string(),
        kind.to_string(),
    ] {
        assert!(message.contains(&part), "{message:?} lacks {part:?}");
    }
}

fn assert_rejected(path: &Path, field: &str, kind: MapErrorKind) {
    assert_rejected_in(path, path, field, kind);
}

fn invalid(found: &str, expected: &str) -> MapErrorKind {
    MapErrorKind::InvalidValue {
        found: found.into(),
        expected: expected.into(),
    }
}

fn parse_error(path: &Path) -> MapError {
    let error = load(path).expect_err("the map must be rejected");
    assert!(matches!(error.kind, MapErrorKind::Parse(_)), "{error}");
    error
}

// --- Positive -------------------------------------------------------------------------------

#[test]
fn files_inside_the_subset_load() {
    // The committed maps and an external tileset written as the subset requires.
    assert!(load(&fixture("valid_4x3.tmx")).is_ok());
    let demo = load(&demo_map()).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        (demo.tile_width, demo.tile_height),
        (TILE_WIDTH_PX, TILE_HEIGHT_PX)
    );
    assert_eq!(demo.tileset_image.file_name, "demo_tiles.png");
    assert_eq!(
        (demo.tileset_image.width, demo.tileset_image.height),
        (512, 32)
    );
    let (map, _) = external_variant(
        "external_ok",
        &[],
        r#"<tileset firstgid="1" source="{source}"/>"#,
    );
    let loaded = load(&map).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(loaded, load(&fixture("valid_4x3.tmx")).expect("valid"));
}

// --- Review P0-1: tileset geometry that `tiled` computes with -------------------------------

#[test]
fn tileset_margin_spacing_and_missing_columns_are_rejected_before_tiled_parses() {
    // Without `columns`, `tiled` computes `(image width - margin + spacing) / (tile width +
    // spacing)` (`tileset.rs:577`): overflow panics in debug builds, division by zero in
    // release builds. `bevy_ecs_tiled` computes the same even with `columns` (`loader.rs:421`).
    let field = |name: &str| format!(r#"tileset "fixture" attribute "{name}""#);
    for (name, head, attribute) in [
        (
            "margin_no_columns",
            r#"tilecount="3" margin="1000">"#,
            "margin",
        ),
        (
            "spacing_no_columns",
            r#"tilecount="3" spacing="4294967232">"#,
            "spacing",
        ),
        (
            "margin_with_columns",
            r#"tilecount="3" columns="3" margin="1000">"#,
            "margin",
        ),
    ] {
        let path = variant(name, &[(r#"tilecount="3" columns="3">"#, head)]);
        assert_rejected(&path, &field(attribute), MapErrorKind::UnsupportedAttribute);
    }
    let path = variant(
        "columns_missing",
        &[(r#"tilecount="3" columns="3">"#, r#"tilecount="3">"#)],
    );
    assert_rejected(&path, &field("columns"), MapErrorKind::MissingAttribute);
    let path = variant(
        "columns_zero",
        &[(
            r#"tilecount="3" columns="3">"#,
            r#"tilecount="3" columns="0">"#,
        )],
    );
    assert_rejected(&path, &field("columns"), invalid("0", "at least 1"));
}

#[test]
fn tileset_image_must_hold_exactly_tilecount_tiles() {
    let width = r#"tileset "fixture" image attribute "width""#;
    let wide = variant(
        "image_width",
        &[(
            IMAGE,
            r#"<image source="fixture_tiles.png" width="191" height="32"/>"#,
        )],
    );
    assert_rejected(&wide, width, invalid("191", "3 columns x 64 px = 192"));
    let columns = variant(
        "columns_eight",
        &[(
            r#"tilecount="3" columns="3">"#,
            r#"tilecount="3" columns="8">"#,
        )],
    );
    assert_rejected(&columns, width, invalid("192", "8 columns x 64 px = 512"));
    let tall = variant(
        "image_height",
        &[(
            IMAGE,
            r#"<image source="fixture_tiles.png" width="192" height="33"/>"#,
        )],
    );
    assert_rejected(
        &tall,
        r#"tileset "fixture" image attribute "height""#,
        invalid("33", "a multiple of the tile height 32"),
    );
    let count = variant(
        "tilecount_four",
        &[(
            r#"tilecount="3" columns="3">"#,
            r#"tilecount="4" columns="3">"#,
        )],
    );
    assert_rejected(
        &count,
        r#"tileset "fixture" attribute "tilecount""#,
        invalid("4", "3 columns x 1 rows of the image = 3"),
    );
    let negative = variant(
        "image_negative",
        &[(
            IMAGE,
            r#"<image source="fixture_tiles.png" width="-192" height="32"/>"#,
        )],
    );
    assert_rejected(
        &negative,
        r#"tileset "fixture" image attribute "width""#,
        invalid("-192", "a positive 32-bit integer"),
    );
}

// --- Review P0-2: nesting that `tiled` parses recursively -----------------------------------

#[test]
fn deeply_nested_groups_and_class_properties_are_rejected_without_recursion() {
    // Before the remediation both aborted the process with a stack overflow.
    let depth = 10_000;
    let groups = format!(
        "{}{}{MAP_END}",
        "<group>".repeat(depth),
        "</group>".repeat(depth)
    );
    let path = variant("nested_groups", &[(MAP_END, &groups)]);
    assert_rejected(&path, r#"layer """#, MapErrorKind::UnexpectedLayer);

    // Unterminated, so that the file stays below the size limit.
    let classes = format!(
        "{SCHEMA_PROPERTY}{}",
        r#"<property name="c" type="class"><properties>"#.repeat(90_000)
    );
    let path = variant("nested_classes", &[(SCHEMA_PROPERTY, &classes)]);
    assert_rejected(
        &path,
        r#"map property "c" attribute "type""#,
        invalid("class", r#""string", "int", "float", or "bool""#),
    );
}

#[test]
fn files_above_the_size_limit_are_rejected() {
    let limit = usize::try_from(MAX_MAP_FILE_BYTES).expect("limit fits in usize");
    let mut text = valid_text().into_bytes();
    text.resize(limit + 1, b'\n');
    let path = temp_file("oversized.tmx", &text);
    assert_rejected(
        &path,
        "file",
        MapErrorKind::FileTooLarge {
            max_bytes: MAX_MAP_FILE_BYTES,
        },
    );
}

// --- Review P0-3: `tilecount` sizes an allocation in `tiled` --------------------------------

#[test]
fn tilecount_is_bounded_before_tiled_allocates() {
    // `tiled` reserves and fills one entry per tile id below `tilecount` before anything is
    // validated (`tileset.rs:484, 534`); 4294967295 grew the process to 246 GB.
    let expected = format!("1 to {MAX_TILESET_TILES}");
    // Largest first, so that a regression hits the memory budget before anything else.
    for count in [
        "4294967295",
        "2147483648",
        "2147483647",
        "2000000",
        "1025",
        "0",
    ] {
        let path = variant(
            &format!("tilecount_{count}"),
            &[(
                r#"tilecount="3" columns="3">"#,
                &format!(r#"tilecount="{count}" columns="3">"#),
            )],
        );
        assert_rejected(
            &path,
            r#"tileset "fixture" attribute "tilecount""#,
            invalid(count, &expected),
        );
    }
}

// --- Review P0-4: paths the renderer resolves -----------------------------------------------

#[test]
fn image_and_tileset_paths_outside_the_map_directory_are_rejected() {
    // A path that leaves the asset root made `bevy_asset` panic (`loader_builders.rs:251`).
    let rule = |extension: &str| {
        format!(
            "a file name in the map's directory, of ASCII letters, digits, '_', '-', and '.', \
             not starting with '.', ending in {extension:?}"
        )
    };
    for (name, source) in [
        ("image_parent", "../../../demo_tiles.png"),
        ("image_subdir", "sub/tiles.png"),
        ("image_absolute", "/tmp/tiles.png"),
        ("image_label", "tiles.png#label"),
        ("image_source", "embedded://tiles.png"),
        ("image_hidden", ".tiles.png"),
        ("image_jpeg", "tiles.jpg"),
    ] {
        let path = variant(
            name,
            &[(
                r#"source="fixture_tiles.png""#,
                &format!(r#"source="{source}""#),
            )],
        );
        assert_rejected(
            &path,
            r#"tileset "fixture" image attribute "source""#,
            invalid(source, &rule(".png")),
        );
    }
    for source in ["../outside.tsx", "/tmp/outside.tsx", "sub/outside.tsx"] {
        let reference = format!(r#"<tileset firstgid="1" source="{source}"/>"#);
        let (path, _) = external_variant("tileset_outside", &[], &reference);
        assert_rejected(
            &path,
            &format!(r#"tileset {source:?} attribute "source""#),
            invalid(source, &rule(".tsx")),
        );
    }
}

// --- Review P1-1: inputs the renderer would accept silently ---------------------------------

#[test]
fn tile_ids_outside_the_tileset_and_duplicate_tiles_are_rejected() {
    // A complete definition outside the image's tile grid, used by cell (3, 2): `tiled` accepted
    // it, and the renderer would have drawn texture index 100000 of a 3-tile image.
    let defined = |id: u32| {
        format!(
            r#"<tile id="{id}"><properties><property name="passable" type="bool" value="true"/><property name="buildable" type="bool" value="true"/><property name="ore" type="int" value="5"/></properties></tile></tileset>"#
        )
    };
    let path = variant(
        "tile_id_large",
        &[
            ("</tileset>", &defined(100_000)),
            (VALID_DATA, "1,1,2,3,\n1,2,2,1,\n3,1,1,100001\n"),
        ],
    );
    assert_rejected(
        &path,
        r#"tileset "fixture" tile 100000 attribute "id""#,
        invalid("100000", "a tile id below the tileset's tilecount of 3"),
    );
    let path = variant("tile_id_count", &[("</tileset>", &defined(3))]);
    assert_rejected(
        &path,
        r#"tileset "fixture" tile 3 attribute "id""#,
        invalid("3", "a tile id below the tileset's tilecount of 3"),
    );
    // A second definition of tile 1: `tiled` kept the last one silently.
    let path = variant("tile_id_duplicate", &[("</tileset>", &defined(1))]);
    assert_rejected(
        &path,
        r#"tileset "fixture" tile 1"#,
        MapErrorKind::Duplicate,
    );
}

#[test]
fn tile_sizes_without_established_exact_picking_are_rejected() {
    let path = variant(
        "map_tile_size",
        &[
            (
                MAP_SIZE,
                r#"width="4" height="3" tilewidth="128" tileheight="64""#,
            ),
            (
                r#"tilewidth="64" tileheight="32" tilecount"#,
                r#"tilewidth="128" tileheight="64" tilecount"#,
            ),
        ],
    );
    assert_rejected(
        &path,
        r#"map attributes "tilewidth" and "tileheight""#,
        invalid(
            "128x64",
            "64x32, the only tile size for which exact picking is established",
        ),
    );
    let path = variant(
        "tileset_tile_size",
        &[(
            r#"tilewidth="64" tileheight="32" tilecount"#,
            r#"tilewidth="32" tileheight="16" tilecount"#,
        )],
    );
    assert_rejected(
        &path,
        r#"tileset "fixture" attributes "tilewidth" and "tileheight""#,
        invalid("32x16", "64x32, the map's tile size"),
    );
}

#[test]
fn external_references_the_renderer_would_resolve_differently_are_rejected() {
    // `bevy_ecs_tiled` finds external tilesets with a line-based scan for `source="`; a
    // single-quoted reference loads in `sim` but not in the renderer.
    let (path, _) = external_variant(
        "single_quoted",
        &[],
        "<tileset firstgid='1' source='{source}'/>",
    );
    assert_rejected(
        &path,
        "external file references",
        MapErrorKind::ExternalReferenceMismatch {
            found: vec![],
            expected: vec!["single_quoted.tsx".into()],
        },
    );
    // A comment that the scan would follow to a file the check never saw.
    let path = variant(
        "commented_reference",
        &[(MAP_END, &format!(r#"<!-- source="other.tsx" -->{MAP_END}"#))],
    );
    assert_rejected(
        &path,
        "external file references",
        MapErrorKind::ExternalReferenceMismatch {
            found: vec!["other.tsx".into()],
            expected: vec![],
        },
    );
    // A non-empty reference: `tiled` would parse its content as content of the map.
    let (path, _) = external_variant(
        "reference_content",
        &[],
        r#"<tileset firstgid="1" source="{source}"></tileset>"#,
    );
    assert_rejected(
        &path,
        r#"tileset "reference_content.tsx""#,
        invalid(
            "a non-empty <tileset> element",
            r#"an empty element <tileset firstgid="1" source="name.tsx"/>"#,
        ),
    );
}

#[test]
fn offsets_parallax_and_other_rendering_attributes_are_rejected() {
    // Formerly detected only after `bevy_ecs_tiled` had loaded the map (`misalignment`).
    let layer = |attribute: &str| format!(r#"layer "terrain" attribute "{attribute}""#);
    for attribute in [r#"offsetx="4""#, r#"parallaxx="0.5""#, r#"opacity="0.5""#] {
        let name = attribute.split('=').next().expect("attribute name");
        let path = variant(
            &format!("layer_{name}"),
            &[(
                LAYER_HEAD,
                &LAYER_HEAD.replace('>', &format!(" {attribute}>")),
            )],
        );
        assert_rejected(&path, &layer(name), MapErrorKind::UnsupportedAttribute);
    }
    let path = variant(
        "tile_offset",
        &[(IMAGE, &format!(r#"<tileoffset x="0" y="8"/>{IMAGE}"#))],
    );
    assert_rejected(
        &path,
        r#"tileset "fixture" element <tileoffset>"#,
        MapErrorKind::UnsupportedElement,
    );
    let path = variant(
        "image_transparency",
        &[(
            IMAGE,
            r#"<image source="fixture_tiles.png" trans="ff00ff" width="192" height="32"/>"#,
        )],
    );
    assert_rejected(
        &path,
        r#"tileset "fixture" image attribute "trans""#,
        MapErrorKind::UnsupportedAttribute,
    );
}

// --- User notes, item 2: the rest of the class ----------------------------------------------

#[test]
fn duplicate_and_namespaced_attributes_are_rejected() {
    let path = variant(
        "attribute_duplicate",
        &[(MAP_SIZE, &format!(r#"width="4" {MAP_SIZE}"#))],
    );
    assert_rejected(&path, r#"map attribute "width""#, MapErrorKind::Duplicate);
    // `tiled` matches attributes by local name, so `a:width` would reach it as `width`.
    let path = variant(
        "attribute_namespaced",
        &[(MAP_SIZE, &format!(r#"{MAP_SIZE} a:width="9""#))],
    );
    assert_rejected(
        &path,
        r#"map attribute "a:width""#,
        MapErrorKind::UnsupportedAttribute,
    );
    let path = variant(
        "element_namespaced",
        &[(LAYER_HEAD, &LAYER_HEAD.replace("<layer", "<a:layer"))],
    );
    // The end tag must match for the XML to be well formed.
    let text =
        std::fs::read_to_string(&path)
            .expect("variant")
            .replacen("</layer>", "</a:layer>", 1);
    std::fs::write(&path, text).expect("variant");
    assert_rejected(
        &path,
        "map element <a:layer>",
        MapErrorKind::UnsupportedElement,
    );
}

#[test]
fn tile_animations_and_other_tile_content_are_rejected() {
    let tile = r#"<tile id="0">"#;
    for (name, content, element) in [
        (
            "animation",
            r#"<animation><frame tileid="1" duration="100"/></animation>"#,
            "animation",
        ),
        (
            "collision",
            r#"<objectgroup draworder="index"/>"#,
            "objectgroup",
        ),
        (
            "tile_image",
            r#"<image source="t.png" width="64" height="32"/>"#,
            "image",
        ),
    ] {
        let path = variant(
            &format!("tile_{name}"),
            &[(tile, &format!("{tile}{content}"))],
        );
        assert_rejected(
            &path,
            &format!(r#"tileset "fixture" tile 0 element <{element}>"#),
            MapErrorKind::UnsupportedElement,
        );
    }
    let path = variant(
        "tile_probability",
        &[(tile, r#"<tile id="0" probability="0.5">"#)],
    );
    assert_rejected(
        &path,
        r#"tileset "fixture" tile 0 attribute "probability""#,
        MapErrorKind::UnsupportedAttribute,
    );
    for (name, content) in [
        ("wangsets", "<wangsets/>"),
        ("transformations", r#"<transformations hflip="1"/>"#),
    ] {
        let path = variant(name, &[(IMAGE, &format!("{IMAGE}{content}"))]);
        assert_rejected(
            &path,
            &format!(r#"tileset "fixture" element <{name}>"#),
            MapErrorKind::UnsupportedElement,
        );
    }
}

#[test]
fn properties_outside_the_subset_are_rejected() {
    let ore = r#"tileset "fixture" tile 1 property "ore""#;
    // `tiled` keeps the last of two equal names silently.
    let path = variant(
        "property_duplicate",
        &[(ORE_PROPERTY, &format!("{ORE_PROPERTY}{ORE_PROPERTY}"))],
    );
    assert_rejected(&path, ore, MapErrorKind::Duplicate);
    let path = variant(
        "property_list",
        &[(ORE_PROPERTY, r#"<property name="ore" type="list"/>"#)],
    );
    assert_rejected(
        &path,
        &format!(r#"{ore} attribute "type""#),
        invalid("list", r#""string", "int", "float", or "bool""#),
    );
    let path = variant(
        "property_text",
        &[(
            ORE_PROPERTY,
            r#"<property name="ore" type="int">750</property>"#,
        )],
    );
    assert_rejected(
        &path,
        &format!(r#"{ore} attribute "value""#),
        MapErrorKind::MissingAttribute,
    );
    let path = variant(
        "property_not_int",
        &[(
            ORE_PROPERTY,
            r#"<property name="ore" type="int" value="7.5"/>"#,
        )],
    );
    assert_rejected(
        &path,
        &format!(r#"{ore} attribute "value""#),
        invalid("7.5", "a value of type int"),
    );
    let path = variant(
        "property_custom_type",
        &[(
            ORE_PROPERTY,
            r#"<property name="ore" type="int" propertytype="x" value="750"/>"#,
        )],
    );
    assert_rejected(
        &path,
        &format!(r#"{ore} attribute "propertytype""#),
        MapErrorKind::UnsupportedAttribute,
    );
}

#[test]
fn structure_outside_the_subset_is_rejected() {
    let path = variant(
        "second_tileset",
        &[(
            LAYER_HEAD,
            &format!(r#"<tileset firstgid="4" source="x.tsx"/>{LAYER_HEAD}"#),
        )],
    );
    assert_rejected(&path, "map element <tileset>", MapErrorKind::Duplicate);
    let path = variant(
        "first_gid",
        &[(r#"<tileset firstgid="1""#, r#"<tileset firstgid="2""#)],
    );
    assert_rejected(
        &path,
        r#"tileset "fixture" attribute "firstgid""#,
        invalid("2", "1"),
    );
    let path = variant(
        "render_order",
        &[(r#"renderorder="right-down""#, r#"renderorder="left-up""#)],
    );
    assert_rejected(
        &path,
        r#"map attribute "renderorder""#,
        invalid("left-up", r#""right-down""#),
    );
    let path = variant(
        "unknown_map_child",
        &[(MAP_END, &format!("<editorsettings/>{MAP_END}"))],
    );
    assert_rejected(
        &path,
        "map element <editorsettings>",
        MapErrorKind::UnsupportedElement,
    );
    let path = variant(
        "two_data",
        &[("</data>", &format!("</data>{DATA_HEAD}{VALID_DATA}</data>"))],
    );
    assert_rejected(
        &path,
        r#"layer "terrain" element <data>"#,
        MapErrorKind::Duplicate,
    );
}

/// A variant name, the edits that make it, and the field the parse error must name.
type Case<'a> = (&'a str, &'a [(&'a str, &'a str)], &'a str);

#[test]
fn xml_constructs_outside_the_subset_are_parse_errors() {
    let cases: [Case; 8] = [
        (
            "doctype",
            &[("<map ", "<!DOCTYPE map [<!ENTITY e \"1\">]><map ")],
            "file",
        ),
        (
            "processing_instruction",
            &[(MAP_END, "<?x y?></map>")],
            "map",
        ),
        (
            "cdata",
            &[(VALID_DATA, "<![CDATA[1,1,2,3,1,2,2,1,3,1,1,1]]>")],
            r#"layer "terrain" data"#,
        ),
        (
            "comment_in_data",
            &[(VALID_DATA, "1,1,2,3,1,2,<!-- x -->2,1,3,1,1,1")],
            r#"layer "terrain" data"#,
        ),
        (
            "text_in_tile",
            &[(r#"<tile id="0">"#, r#"<tile id="0">grass"#)],
            r#"tileset "fixture" tile 0"#,
        ),
        ("content_after_root", &[(MAP_END, "</map><map/>")], "file"),
        // P2-2 of the review: `quick-xml` accepts `</map` cut off at the end of the file.
        ("unterminated_end", &[(MAP_END, "</map")], "file"),
        (
            "entity",
            &[(r#"name="fixture""#, r#"name="&e;""#)],
            r#"tileset "" attribute "name""#,
        ),
    ];
    for (name, edits, field) in cases {
        let text = edited(&valid_text(), edits);
        let text = if name == "unterminated_end" {
            text.trim_end().to_owned()
        } else {
            text
        };
        let path = temp_file(&format!("{name}.tmx"), text.as_bytes());
        let error = parse_error(&path);
        assert_eq!(
            (error.file.as_path(), error.field.as_str()),
            (path.as_path(), field),
            "{name}: {error}"
        );
    }
    let mut bytes = valid_text().into_bytes();
    bytes.extend_from_slice(b"<!-- \xff -->");
    let path = temp_file("not_utf8.tmx", &bytes);
    assert_eq!(parse_error(&path).field, "file");
}

// --- Review P2-6: every tile id below `tilecount` must be defined (documented in D8) ---------

#[test]
fn every_tile_id_below_tilecount_needs_a_definition() {
    // `tiled` creates an entry without properties for each id below `tilecount`; the loader
    // reports it as missing its required properties, even if no cell uses it.
    let text = valid_text();
    let start = text.find(r#"<tile id="2">"#).expect("tile 2");
    let end = start + text[start..].find("</tile>").expect("end of tile 2") + "</tile>".len();
    let path = temp_file(
        "tile_undefined_below_count.tmx",
        text.replacen(&text[start..end], "", 1).as_bytes(),
    );
    assert_rejected(
        &path,
        r#"tileset "fixture" tile 2 property "passable""#,
        MapErrorKind::MissingProperty,
    );
}

// --- Review P1-3: errors inside an external tileset name the tileset file -------------------

#[test]
fn errors_in_external_tilesets_name_the_tileset_file() {
    let reference = r#"<tileset firstgid="1" source="{source}"/>"#;
    let (map, tsx) = external_variant(
        "external_bad_property",
        &[(
            r#"<property name="buildable" type="bool" value="true"/>"#,
            r#"<property name="buildable" type="bool" value="true"/><property name="height" type="int" value="2"/>"#,
        )],
        reference,
    );
    // Only tile 0 is buildable, so the edit applies to it.
    assert_rejected_in(
        &map,
        &tsx,
        r#"tileset "fixture" tile 0 property "height""#,
        MapErrorKind::UnknownProperty,
    );

    let (map, tsx) = external_variant(
        "external_margin",
        &[(r#"columns="3">"#, r#"columns="3" margin="1000">"#)],
        reference,
    );
    assert_rejected_in(
        &map,
        &tsx,
        r#"tileset "fixture" attribute "margin""#,
        MapErrorKind::UnsupportedAttribute,
    );

    let (map, tsx) = external_variant("external_truncated", &[], reference);
    let text = std::fs::read_to_string(&tsx).expect("tileset");
    let cut = text.find("tilewidth=\"64").expect("attribute") + 12;
    std::fs::write(&tsx, &text[..cut]).expect("truncated tileset");
    let error = parse_error(&map);
    assert_eq!(error.file, tsx, "{error}");
    assert!(error.to_string().contains(&tsx.display().to_string()));

    let (map, tsx) = external_variant(
        "external_nested_reference",
        // On a line of its own: the scan reads only the first `source="` of each line.
        &[(IMAGE, &format!("{IMAGE}\n<!-- source=\"more.tsx\" -->"))],
        reference,
    );
    assert_rejected_in(
        &map,
        &tsx,
        "external file references",
        MapErrorKind::ExternalReferenceMismatch {
            found: vec!["more.tsx".into()],
            expected: vec![],
        },
    );
}

// --- User notes, item 3: seeded mutation test over the demo map -----------------------------

/// SplitMix64: a small deterministic generator, so the cases do not depend on a crate's
/// stream stability.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `0..bound` (`bound` > 0).
    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    /// A random item, or `None` if there is none.
    fn pick<T: Copy>(&mut self, items: &[T]) -> Option<T> {
        if items.is_empty() {
            return None;
        }
        Some(items[self.below(items.len())])
    }
}

const MUTATION_SEED: u64 = 0x4D31_5245_4D31_0001;
const MUTATION_CASES: usize = 6000;

const NUMBERS: &[&str] = &[
    "0",
    "1",
    "-1",
    "2",
    "3",
    "7",
    "8",
    "9",
    "31",
    "32",
    "33",
    "63",
    "64",
    "65",
    "255",
    "256",
    "257",
    "511",
    "512",
    "513",
    "1000",
    "1024",
    "1025",
    "65535",
    "2147483647",
    "2147483648",
    "4294967232",
    "4294967295",
    "4294967296",
    "99999999999",
    "-0",
    "+1",
    "1e3",
    "0x10",
    "",
];
const VALUES: &[&str] = &[
    "",
    "../demo_tiles.png",
    "/tmp/x.png",
    "x.tsx",
    "class",
    "list",
    "isometric",
    "staggered",
    "csv",
    "base64",
    "true",
    "false",
    "1.5",
    "terrain",
    "demo_tiles",
    "&amp;",
    "a#b",
    "9999999999999999999999999999999999999999",
];
const ATTRIBUTES: &[&str] = &[
    r#" margin="1000""#,
    r#" spacing="4294967232""#,
    r#" columns="0""#,
    r#" tilecount="4294967295""#,
    r#" a:width="9""#,
    r#" xmlns:a="urn:x""#,
    r#" offsetx="4""#,
    r#" parallaxx="0.5""#,
    r#" source="../../x.png""#,
    r#" width="5""#,
    r#" infinite="1""#,
    r#" type="class""#,
    r#" id="100000""#,
    r#" trans="ff00ff""#,
    r#" encoding="base64""#,
    r#" compression="zlib""#,
    r#" propertytype="x""#,
];
const SNIPPETS: &[&str] = &[
    "<group>",
    r#"<objectgroup id="9" name="o"/>"#,
    r#"<imagelayer id="9" name="i"><image source="x.png" width="1" height="1"/></imagelayer>"#,
    r#"<tileoffset x="0" y="8"/>"#,
    r#"<animation><frame tileid="0" duration="1"/></animation>"#,
    r#"<properties><property name="p" type="class"><properties><property name="q" value="1"/></properties></property></properties>"#,
    r#"<property name="ore" type="int" value="5"/>"#,
    r#"<tile id="7"><properties/></tile>"#,
    r#"<tile id="100000"/>"#,
    "<!DOCTYPE map>",
    "<?pi x?>",
    "<![CDATA[1,2]]>",
    r#"<!-- source="x.tsx" -->"#,
    "<tileset firstgid='9' source='e.tsx'/>",
    r#"<tileset firstgid="1" source="e.tsx"/>"#,
    "<wangsets/>",
    r#"<transformations hflip="1"/>"#,
    r#"<data encoding="csv">1</data>"#,
    "&amp;",
    "&bogus;",
    "</tileset>",
    "</map>",
];

/// An attribute inside a start tag: the offset of the space before its name, and the offsets
/// of the first byte of its value and of the closing quote.
#[derive(Clone, Copy)]
struct AttributeSpan {
    start: usize,
    value: usize,
    end: usize,
}

/// The double-quoted attributes of a random start tag. The tag is drawn by element kind first,
/// so that the few attributes of the map, tileset, image, and layer are drawn about as often as
/// the many attributes of properties.
fn random_tag_attributes(text: &[u8], rng: &mut SplitMix64) -> Option<Vec<AttributeSpan>> {
    const KINDS: [&[u8]; 7] = [
        b"<map ",
        b"<tileset ",
        b"<image ",
        b"<layer ",
        b"<data ",
        b"<tile ",
        b"<property ",
    ];
    let tags: Vec<Vec<usize>> = KINDS
        .iter()
        .map(|kind| positions_after(text, kind))
        .filter(|positions| !positions.is_empty())
        .collect();
    let kind = rng.below(tags.len().max(1));
    let mut i = rng.pick(tags.get(kind)?)?;
    let mut attributes = Vec::new();
    while i < text.len() && text[i] != b'>' {
        if text[i..].starts_with(b"=\"") {
            let start = text[..i].iter().rposition(|&b| b == b' ')?;
            let value = i + 2;
            let end = value + text[value..].iter().position(|&b| b == b'"')?;
            attributes.push(AttributeSpan { start, value, end });
            i = end + 1;
        } else {
            i += 1;
        }
    }
    Some(attributes)
}

/// Byte offsets just after each occurrence of `needle`.
fn positions_after(text: &[u8], needle: &[u8]) -> Vec<usize> {
    text.windows(needle.len())
        .enumerate()
        .filter(|(_, window)| *window == needle)
        .map(|(index, _)| index + needle.len())
        .collect()
}

/// Applies one random mutation to `text` and describes it.
fn mutate(text: &mut Vec<u8>, rng: &mut SplitMix64) -> String {
    mutation(text, rng).unwrap_or_else(|| "no change".into())
}

fn mutation(text: &mut Vec<u8>, rng: &mut SplitMix64) -> Option<String> {
    let len = text.len();
    Some(match rng.below(11) {
        0 => {
            // Replace a run of digits.
            let starts: Vec<usize> = (0..len)
                .filter(|&i| text[i].is_ascii_digit() && (i == 0 || !text[i - 1].is_ascii_digit()))
                .collect();
            let start = rng.pick(&starts)?;
            let end = (start..len)
                .find(|&i| !text[i].is_ascii_digit())
                .unwrap_or(len);
            let number = rng.pick(NUMBERS)?;
            text.splice(start..end, number.bytes());
            format!("number at {start} -> {number:?}")
        }
        1 => {
            let start = rng.below(len.max(1));
            let end = (start + 1 + rng.below(64)).min(len);
            text.drain(start..end);
            format!("delete {start}..{end}")
        }
        2 => {
            let start = rng.below(len.max(1));
            let end = (start + 1 + rng.below(256)).min(len);
            let copy = text[start..end].to_vec();
            text.splice(start..start, copy);
            format!("duplicate {start}..{end}")
        }
        3 => {
            // Insert an element or markup snippet after a tag, sometimes 2000 times in a row.
            let at = rng.pick(&positions_after(text, b">"))?;
            let snippet = rng.pick(SNIPPETS)?;
            let repeat = if rng.below(8) == 0 { 2000 } else { 1 };
            text.splice(at..at, snippet.repeat(repeat).bytes());
            format!("insert {snippet:?} x{repeat} at {at}")
        }
        4 => {
            // Insert an attribute into a start tag, after the last attribute.
            let positions: Vec<usize> = positions_after(text, b"\"")
                .into_iter()
                .filter(|&i| matches!(text.get(i), Some(b'>' | b'/')))
                .collect();
            let at = rng.pick(&positions)?;
            let attribute = rng.pick(ATTRIBUTES)?;
            text.splice(at..at, attribute.bytes());
            format!("attribute {attribute:?} at {at}")
        }
        5 => {
            // Replace an attribute value.
            let attributes = random_tag_attributes(text, rng)?;
            let attribute = rng.pick(&attributes)?;
            let value = if rng.below(2) == 0 {
                rng.pick(NUMBERS)?
            } else {
                rng.pick(VALUES)?
            };
            text.splice(attribute.value..attribute.end, value.bytes());
            format!("value at {} -> {value:?}", attribute.value)
        }
        8 => {
            // Replace a whole attribute with another one.
            let attributes = random_tag_attributes(text, rng)?;
            let attribute = rng.pick(&attributes)?;
            let replacement = rng.pick(ATTRIBUTES)?;
            text.splice(attribute.start..=attribute.end, replacement.bytes());
            format!("attribute at {} -> {replacement:?}", attribute.start)
        }
        9 => {
            // Remove an attribute.
            let attributes = random_tag_attributes(text, rng)?;
            let attribute = rng.pick(&attributes)?;
            text.drain(attribute.start..=attribute.end);
            format!("remove attribute at {}", attribute.start)
        }
        6 => {
            let at = rng.below(len.max(1));
            let byte = rng.pick(b"<>/=\"' \n:&;!?-9a\xff")?;
            if let Some(slot) = text.get_mut(at) {
                *slot = byte;
            }
            format!("byte at {at} -> {byte:#04x}")
        }
        7 => {
            let at = rng.below(len.max(1));
            text.truncate(at);
            format!("truncate at {at}")
        }
        _ => {
            // Single quotes around one attribute value, as a hand edit might write them.
            let start = rng.pick(&positions_after(text, b"=\""))?;
            let end = (start..len).find(|&i| text[i] == b'"')?;
            text[start - 1] = b'\'';
            text[end] = b'\'';
            format!("single quotes at {start}")
        }
    })
}

#[test]
fn seeded_mutations_of_the_demo_map_return_a_map_or_a_typed_error_and_never_panic() {
    let original = std::fs::read(demo_map()).expect("demo map");
    let dir = temp_dir().join("mutations");
    std::fs::create_dir_all(&dir).expect("mutation directory");
    let path = dir.join("mutated.tmx");
    let report = bounded("mutation test", MUTATION_TIME_LIMIT, move || {
        let mut rng = SplitMix64(MUTATION_SEED);
        let (mut accepted, mut rejected) = (0, 0);
        let mut failures = Vec::new();
        for case in 0..MUTATION_CASES {
            let mut text = original.clone();
            let steps: Vec<String> = (0..1 + rng.below(3))
                .map(|_| mutate(&mut text, &mut rng))
                .collect();
            std::fs::write(&path, &text).expect("mutated map");
            let outcome =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| load_map(&path)));
            match outcome {
                Err(_) => failures.push(format!("case {case} panicked: {steps:?}")),
                Ok(Ok(map)) => {
                    accepted += 1;
                    let grid = &map.grid;
                    let valid = (1..=sim::MAX_MAP_SIDE).contains(&grid.width())
                        && (1..=sim::MAX_MAP_SIDE).contains(&grid.height())
                        && (map.tile_width, map.tile_height) == (TILE_WIDTH_PX, TILE_HEIGHT_PX)
                        && map.tileset_image.file_name.ends_with(".png")
                        && !map.tileset_image.file_name.contains(['/', '\\']);
                    if !valid {
                        failures.push(format!("case {case} returned an invalid map: {steps:?}"));
                    }
                }
                Ok(Err(error)) => {
                    rejected += 1;
                    let named = error.file == path || error.file.starts_with(&dir);
                    if !named || error.field.is_empty() || error.to_string().is_empty() {
                        failures.push(format!("case {case} error lacks context: {error}"));
                    }
                }
            }
        }
        (accepted, rejected, failures)
    });
    let (accepted, rejected, failures) = report;
    assert!(failures.is_empty(), "{failures:#?}");
    // Both outcomes occur, so the cases exercise acceptance as well as rejection.
    assert!(
        accepted > 0 && rejected > 0,
        "accepted {accepted}, rejected {rejected}"
    );
}
