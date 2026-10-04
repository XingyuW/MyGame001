//! M1 remediation round 2: the supported subset of the tileset image (docs/DECISIONS.md, D16;
//! invariants I8 and I11), with the regression tests for M1 review 2, P0-A (a PNG of a few
//! megabytes made the renderer's decoder allocate gigabytes) and for the pixel formats the
//! renderer cannot sample.
//!
//! The tests feed hostile images to `check_tileset_image` and are bounded as in D15:
//!
//! * Memory: this test binary's global allocator refuses any allocation that would raise the
//!   bytes in use above [`MEMORY_BUDGET`], so a regression aborts the run ("memory allocation of
//!   N bytes failed") instead of growing. Every bomb below decompresses to [`BOMB_BYTES`], four
//!   times the budget.
//! * Time: every check runs on a worker thread whose result must arrive within [`TIME_LIMIT`].
//!
//! Images are written to Cargo's temporary directory for integration tests.

#[path = "../src/tileset_image.rs"]
mod tileset_image;

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, Image, ImageSampler, ImageType};
use bevy::render::render_resource::TextureFormat;
use flate2::Compression;
use flate2::write::ZlibEncoder;
use sim::TilesetImage;
use tileset_image::{
    MAX_TILESET_IMAGE_BYTES, TilesetImageError, TilesetImageErrorKind, check_tileset_image,
};

// --- Bounds ---------------------------------------------------------------------------------

/// Upper limit of heap bytes in use by this test process.
const MEMORY_BUDGET: usize = 64 << 20;
/// Decompressed size of every compressed bomb.
const BOMB_BYTES: usize = 256 << 20;
/// Upper limit for one check.
const TIME_LIMIT: Duration = Duration::from_secs(60);

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
        // SAFETY: forwarded unchanged.
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
        // SAFETY: forwarded unchanged.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if pointer.is_null() {
            release(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: forwarded unchanged.
        unsafe { System.dealloc(pointer, layout) };
        release(layout.size());
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if new_size > layout.size() && !reserve(new_size - layout.size()) {
            return std::ptr::null_mut();
        }
        // SAFETY: forwarded unchanged.
        let new_pointer = unsafe { System.realloc(pointer, layout, new_size) };
        if new_pointer.is_null() {
            if new_size > layout.size() {
                release(new_size - layout.size());
            }
        } else if new_size < layout.size() {
            release(layout.size() - new_size);
        }
        new_pointer
    }
}

#[global_allocator]
static ALLOCATOR: BudgetAllocator = BudgetAllocator;

/// Runs `work` on a worker thread and returns its result, failing the test if the worker panics
/// or does not finish within [`TIME_LIMIT`].
fn bounded<T: Send + 'static>(what: &str, work: impl FnOnce() -> T + Send + 'static) -> T {
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name(what.to_owned())
        .spawn(move || {
            let _ = sender.send(work());
        })
        .expect("worker thread");
    match receiver.recv_timeout(TIME_LIMIT) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => panic!("{what}: no result within {TIME_LIMIT:?}"),
        Err(RecvTimeoutError::Disconnected) => panic!("{what}: the check panicked"),
    }
}

// --- PNG writing ----------------------------------------------------------------------------

type Chunk = (&'static [u8; 4], Vec<u8>);

fn png(chunks: &[Chunk]) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    for (kind, data) in chunks {
        bytes.extend(u32::try_from(data.len()).expect("chunk size").to_be_bytes());
        bytes.extend(*kind);
        bytes.extend(data);
        let mut crc = crc32fast::Hasher::new();
        crc.update(*kind);
        crc.update(data);
        bytes.extend(crc.finalize().to_be_bytes());
    }
    bytes
}

fn ihdr(width: u32, height: u32, depth: u8, colour: u8, interlace: u8) -> Chunk {
    let mut data = width.to_be_bytes().to_vec();
    data.extend(height.to_be_bytes());
    data.extend([depth, colour, 0, 0, interlace]);
    (b"IHDR", data)
}

/// `length` zero bytes, zlib-compressed, written in pieces so that the input never needs
/// memory of its size.
fn zlib_zeros(length: usize) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    let piece = vec![0u8; 1 << 16];
    let mut left = length;
    while left > 0 {
        let n = left.min(piece.len());
        encoder.write_all(&piece[..n]).expect("compress");
        left -= n;
    }
    encoder.finish().expect("compress")
}

/// The image data of a `width` × `height` image of `bytes_per_pixel`, all zero, filter 0.
fn idat(width: u32, height: u32, bytes_per_pixel: u32) -> Chunk {
    let row = 1 + usize::try_from(width * bytes_per_pixel).expect("row");
    let rows = usize::try_from(height).expect("rows");
    (b"IDAT", zlib_zeros(row * rows))
}

fn iend() -> Chunk {
    (b"IEND", Vec::new())
}

/// An 8-bit RGBA image of the given size inside the subset.
fn rgba(width: u32, height: u32) -> Vec<u8> {
    png(&[ihdr(width, height, 8, 6, 0), idat(width, height, 4), iend()])
}

/// The zlib stream of [`BOMB_BYTES`] zero bytes, computed once.
fn bomb_stream() -> &'static [u8] {
    static STREAM: OnceLock<Vec<u8>> = OnceLock::new();
    STREAM.get_or_init(|| zlib_zeros(BOMB_BYTES))
}

// --- Checking -------------------------------------------------------------------------------

/// Writes `bytes` as the tileset image of a map in a directory of its own named `name`, and
/// checks it against a declared size of `width` × `height`.
fn check(name: &str, bytes: &[u8], width: u32, height: u32) -> Result<Vec<u8>, TilesetImageError> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("m1_tileset_image")
        .join(name);
    std::fs::create_dir_all(&dir).expect("temporary directory");
    std::fs::write(dir.join("tiles.png"), bytes).expect("image");
    check_file(name, &dir.join("map.tmx"), width, height)
}

fn check_file(
    name: &str,
    map: &Path,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, TilesetImageError> {
    let map = map.to_owned();
    let declared = TilesetImage {
        file_name: "tiles.png".into(),
        width,
        height,
    };
    bounded(name, move || check_tileset_image(&map, &declared))
}

/// Asserts that the image is rejected with `kind` at `field`, and that the message names the
/// image file, the field, and the reason.
fn assert_rejected(
    name: &str,
    bytes: &[u8],
    size: (u32, u32),
    field: &str,
    kind: TilesetImageErrorKind,
) {
    let error = check(name, bytes, size.0, size.1).expect_err(name);
    assert_eq!(
        (error.field.as_str(), &error.kind),
        (field, &kind),
        "{name}: {error}"
    );
    assert!(error.file.ends_with(format!("{name}/tiles.png")), "{error}");
    let message = error.to_string();
    for part in [
        error.file.display().to_string(),
        field.to_owned(),
        kind.to_string(),
    ] {
        assert!(message.contains(&part), "{message:?} lacks {part:?}");
    }
}

fn invalid(found: &str, expected: &str) -> TilesetImageErrorKind {
    TilesetImageErrorKind::InvalidValue {
        found: found.into(),
        expected: expected.into(),
    }
}

/// The texture format Bevy's image loader makes of `bytes` (`Image::from_buffer`, as
/// `bevy_image`'s `ImageLoader` calls it for a PNG file).
fn bevy_texture_format(bytes: &[u8]) -> TextureFormat {
    Image::from_buffer(
        bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::Default,
        RenderAssetUsages::default(),
    )
    .expect("Bevy decodes the image")
    .texture_descriptor
    .format
}

// --- Positive -------------------------------------------------------------------------------

#[test]
fn images_inside_the_subset_pass_and_their_checked_bytes_are_returned() {
    let demo_map =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/maps/demo/demo.tmx");
    let demo = sim::load_map(&demo_map).unwrap_or_else(|error| panic!("{error}"));
    let declared = demo.tileset_image.clone();
    let checked = bounded("demo", move || check_tileset_image(&demo_map, &declared))
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        checked,
        std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/maps/demo/demo_tiles.png")
        )
        .expect("demo image")
    );

    let ancillary = png(&[
        ihdr(512, 32, 8, 6, 0),
        (b"sRGB", vec![0]),
        (b"gAMA", 45455u32.to_be_bytes().to_vec()),
        (b"cHRM", vec![0; 32]),
        (b"pHYs", vec![0, 0, 11, 19, 0, 0, 11, 19, 1]),
        idat(512, 32, 4),
        iend(),
    ]);
    // Image data split over several IDAT chunks.
    let (_, data) = idat(512, 32, 3);
    let (first, second) = data.split_at(data.len() / 2);
    let split_rgb = png(&[
        ihdr(512, 32, 8, 2, 0),
        (b"IDAT", first.to_vec()),
        (b"IDAT", second.to_vec()),
        iend(),
    ]);
    let cases = [
        ("rgba", rgba(512, 32), 512, 32),
        ("ancillary", ancillary, 512, 32),
        ("split_rgb", split_rgb, 512, 32),
        // The largest images `sim` accepts (M1 review 2, P1-A): 8192 px wide, 8192 px tall.
        ("widest", rgba(8192, 64), 8192, 64),
        ("tallest", rgba(64, 8192), 64, 8192),
    ];
    for (name, bytes, width, height) in cases {
        let checked = check(name, &bytes, width, height).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(checked, bytes, "{name}");
        // Bevy turns every image of the subset into a texture every GPU can sample.
        assert_eq!(
            bevy_texture_format(&bytes),
            TextureFormat::Rgba8UnormSrgb,
            "{name}"
        );
    }
}

// --- Review 2, P0-A: unbounded allocation in the renderer's decoder -------------------------

#[test]
fn compressed_and_other_unlisted_chunks_are_rejected_before_any_decoding() {
    // Before the fix only the 24-byte header was read, and each of these was handed to the
    // renderer, whose decoder runs without a memory limit and inflated the bomb (review: 0.58 GB
    // from a 587 KB file, 7.58 GB from 7.4 MB).
    let stream = bomb_stream();
    let with_prefix = |prefix: &[u8]| {
        let mut data = prefix.to_vec();
        data.extend(stream);
        data
    };
    let bombs: [Chunk; 3] = [
        (b"iCCP", with_prefix(b"profile\0\0")),
        (b"zTXt", with_prefix(b"Comment\0\0")),
        (b"iTXt", with_prefix(b"Comment\0\x01\0\0\0")),
    ];
    for (kind, data) in bombs {
        let bytes = png(&[
            ihdr(512, 32, 8, 6, 0),
            (kind, data),
            idat(512, 32, 4),
            iend(),
        ]);
        assert!(bytes.len() < usize::try_from(MAX_TILESET_IMAGE_BYTES).expect("limit"));
        let name = String::from_utf8_lossy(kind).into_owned();
        assert_rejected(
            &format!("bomb_{name}"),
            &bytes,
            (512, 32),
            &format!("chunk {name:?}"),
            TilesetImageErrorKind::UnsupportedChunk,
        );
    }
    // Every other chunk outside the list, before and after the image data.
    for kind in [
        b"tEXt", b"eXIf", b"acTL", b"fcTL", b"PLTE", b"tRNS", b"bKGD", b"prIv",
    ] {
        let name = String::from_utf8_lossy(kind).into_owned();
        for (position, chunks) in [
            (
                "before",
                vec![
                    ihdr(512, 32, 8, 6, 0),
                    (kind, vec![0; 8]),
                    idat(512, 32, 4),
                    iend(),
                ],
            ),
            (
                "after",
                vec![
                    ihdr(512, 32, 8, 6, 0),
                    idat(512, 32, 4),
                    (kind, vec![0; 8]),
                    iend(),
                ],
            ),
        ] {
            assert_rejected(
                &format!("chunk_{name}_{position}"),
                &png(&chunks),
                (512, 32),
                &format!("chunk {name:?}"),
                TilesetImageErrorKind::UnsupportedChunk,
            );
        }
    }
}

#[test]
fn image_files_above_the_size_limit_are_rejected_without_being_read_whole() {
    let limit = usize::try_from(MAX_TILESET_IMAGE_BYTES).expect("limit");
    let mut bytes = rgba(512, 32);
    bytes.resize(limit + 1, 0);
    let size = TilesetImageErrorKind::FileTooLarge {
        max_bytes: MAX_TILESET_IMAGE_BYTES,
    };
    assert_rejected("oversized", &bytes, (512, 32), "file", size.clone());

    // A 1 GiB file: reading it whole would exceed the memory budget.
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("m1_tileset_image/huge");
    std::fs::create_dir_all(&dir).expect("temporary directory");
    let file = std::fs::File::create(dir.join("tiles.png")).expect("image");
    file.set_len(1 << 30).expect("sparse file");
    drop(file);
    let error = check_file("huge", &dir.join("map.tmx"), 512, 32).expect_err("huge file");
    assert_eq!(error.kind, size, "{error}");
    let _ = std::fs::remove_file(dir.join("tiles.png"));
}

// --- Review 2, P1-A class: pixel formats the renderer cannot sample -------------------------

#[test]
fn pixel_formats_other_than_8_bit_rgb_and_rgba_are_rejected() {
    // A 16-bit greyscale PNG became an `R16Uint` texture, which the tilemap shader cannot
    // sample: the release binary quit with "Texture binding 0 expects sample type Float
    // { filterable: true }, but was given a view with format R16Uint" (found in this round).
    let luma16 = png(&[ihdr(512, 32, 16, 0, 0), idat(512, 32, 2), iend()]);
    assert_eq!(bevy_texture_format(&luma16), TextureFormat::R16Uint);
    assert_rejected(
        "luma16",
        &luma16,
        (512, 32),
        r#"chunk "IHDR" bit depth"#,
        invalid("16", "8"),
    );

    let rgba16 = png(&[ihdr(512, 32, 16, 6, 0), idat(512, 32, 8), iend()]);
    assert_rejected(
        "rgba16",
        &rgba16,
        (512, 32),
        r#"chunk "IHDR" bit depth"#,
        invalid("16", "8"),
    );

    let colour = r#"chunk "IHDR" colour type"#;
    let colours = "2 (RGB) or 6 (RGBA)";
    for (name, code, bytes_per_pixel) in
        [("luma8", 0u8, 1), ("palette8", 3, 1), ("luma_alpha8", 4, 2)]
    {
        let bytes = png(&[
            ihdr(512, 32, 8, code, 0),
            idat(512, 32, bytes_per_pixel),
            iend(),
        ]);
        assert_rejected(
            name,
            &bytes,
            (512, 32),
            colour,
            invalid(&code.to_string(), colours),
        );
    }
    let interlaced = png(&[ihdr(512, 32, 8, 6, 1), idat(512, 32, 4), iend()]);
    assert_rejected(
        "interlaced",
        &interlaced,
        (512, 32),
        r#"chunk "IHDR" interlace method"#,
        invalid("1", "0"),
    );
}

// --- Chunk structure and image data ---------------------------------------------------------

#[test]
fn chunk_structure_outside_the_subset_is_rejected() {
    let size = (512, 32);
    let head = || ihdr(512, 32, 8, 6, 0);
    let data = || idat(512, 32, 4);

    let mut trailing = rgba(512, 32);
    trailing.extend(b"junk");
    assert_rejected(
        "after_iend",
        &trailing,
        size,
        r#"content after chunk "IEND""#,
        invalid("4 bytes", "the end of the file"),
    );
    assert_rejected(
        "srgb_after_idat",
        &png(&[head(), data(), (b"sRGB", vec![0]), iend()]),
        size,
        r#"chunk "sRGB""#,
        invalid("a chunk after IDAT", "a chunk before IDAT"),
    );
    assert_rejected(
        "srgb_twice",
        &png(&[
            head(),
            (b"sRGB", vec![0]),
            (b"sRGB", vec![0]),
            data(),
            iend(),
        ]),
        size,
        r#"chunk "sRGB""#,
        TilesetImageErrorKind::Duplicate,
    );
    assert_rejected(
        "srgb_length",
        &png(&[head(), (b"sRGB", vec![0, 0]), data(), iend()]),
        size,
        r#"chunk "sRGB" length"#,
        invalid("2", "1 bytes"),
    );
    assert_rejected(
        "no_idat",
        &png(&[head(), iend()]),
        size,
        r#"chunk "IDAT""#,
        TilesetImageErrorKind::MissingChunk,
    );
    assert_rejected(
        "no_iend",
        &png(&[head(), data()]),
        size,
        r#"chunk "IEND""#,
        TilesetImageErrorKind::MissingChunk,
    );
    let mut cut = rgba(512, 32);
    cut.truncate(cut.len() - 20);
    assert_rejected(
        "cut",
        &cut,
        size,
        r#"chunk "IDAT""#,
        TilesetImageErrorKind::Truncated,
    );
    let iend_with_data = png(&[head(), data(), (b"IEND", vec![0])]);
    assert_rejected(
        "iend_length",
        &iend_with_data,
        size,
        r#"chunk "IEND" length"#,
        invalid("1", "0 bytes"),
    );
}

#[test]
fn image_data_the_decoder_rejects_is_rejected_before_the_hand_off() {
    // Before the fix, such files were handed to the renderer and failed only there (D14).
    let garbage = png(&[
        ihdr(512, 32, 8, 6, 0),
        (b"IDAT", b"not zlib data".to_vec()),
        iend(),
    ]);
    let error = check("garbage_idat", &garbage, 512, 32).expect_err("garbage image data");
    assert_eq!(error.field, "image data", "{error}");
    assert!(
        matches!(error.kind, TilesetImageErrorKind::Decode(_)),
        "{error}"
    );

    let mut bad_crc = rgba(512, 32);
    let crc_of_idat = bad_crc.len() - 12 - 4;
    bad_crc[crc_of_idat] ^= 0xFF;
    let error = check("bad_crc", &bad_crc, 512, 32).expect_err("corrupt checksum");
    assert!(
        matches!(error.kind, TilesetImageErrorKind::Decode(_)),
        "{error}"
    );

    // Too little image data for the declared size.
    let short = png(&[ihdr(512, 32, 8, 6, 0), idat(512, 16, 4), iend()]);
    let error = check("short_idat", &short, 512, 32).expect_err("short image data");
    assert!(
        matches!(error.kind, TilesetImageErrorKind::Decode(_)),
        "{error}"
    );
}
