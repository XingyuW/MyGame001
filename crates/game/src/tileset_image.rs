//! The supported subset of the tileset image (docs/DECISIONS.md, D16; invariant I11).
//!
//! The renderer decodes the tileset image with `image` 0.25.10 and no memory limit
//! (`bevy_image` 0.19.1, `Image::from_buffer`), and a PNG of a few megabytes can make that
//! decoder allocate gigabytes (M1 review 2, P0-A: a compressed `iCCP` chunk). The image is
//! therefore checked here, before the map is handed to the renderer, and only the bytes checked
//! here are handed on (`crate::checked_assets`):
//!
//! * the file holds at most [`MAX_TILESET_IMAGE_BYTES`] and is read with that bound;
//! * it is a PNG file whose `IHDR` chunk declares the size the map declares for the image, a bit
//!   depth of 8, colour type 2 (RGB) or 6 (RGBA), and no interlacing, so that Bevy converts it to
//!   an `Rgba8UnormSrgb` texture, which every GPU samples as a filterable float texture;
//! * its chunks are `IHDR`, then at most one each of `sRGB`, `gAMA`, `cHRM`, and `pHYs` (fixed
//!   size, uncompressed), then one or more consecutive `IDAT`, then `IEND` at the end of the file;
//!   every other chunk, including the compressed `iCCP`, `zTXt`, and `iTXt`, is rejected;
//! * it decodes completely with the decoder and the settings Bevy uses, under a memory limit of
//!   [`DECODE_MEMORY_LIMIT`], to an RGB or RGBA image of 8 bits per channel and the declared size.
//!
//! Anything else is a typed [`TilesetImageError`] naming the image file, the field, and the reason.

use std::fmt;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageFormat, ImageReader, Limits};
use sim::{MAX_TILESET_IMAGE_SIDE_PX, TilesetImage};

/// Largest accepted tileset image file. A tileset holds at most 256 tiles of 64×32 px
/// (`sim::MAX_TILESET_TILES`), 2 MiB of RGBA pixels, which fits even without compression.
pub const MAX_TILESET_IMAGE_BYTES: u64 = 4 * 1024 * 1024;

/// What the decoder may allocate while the image is checked: the decoded pixels (at most 2 MiB)
/// and the decoder's buffers.
pub const DECODE_MEMORY_LIMIT: u64 = 16 * 1024 * 1024;

const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// The ancillary chunks a tileset image may hold before its image data, with their exact length.
/// Tiled and image editors commonly write them; none of them is compressed.
const ANCILLARY_CHUNKS: [(&[u8; 4], u32); 4] =
    [(b"sRGB", 1), (b"gAMA", 4), (b"cHRM", 32), (b"pHYs", 9)];

/// Why a tileset image was rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TilesetImageError {
    pub file: PathBuf,
    /// The part of the file at fault, e.g. `chunk "iCCP"`, or `file`.
    pub field: String,
    pub kind: TilesetImageErrorKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TilesetImageErrorKind {
    /// The file could not be read, or ends before the PNG header is complete.
    Read(String),
    FileTooLarge {
        max_bytes: u64,
    },
    /// The file does not start with a PNG signature and an `IHDR` chunk.
    NotPng,
    /// The image size differs from the size the map declares for it.
    SizeMismatch {
        width: u32,
        height: u32,
        declared_width: u32,
        declared_height: u32,
    },
    /// The chunk type is outside the supported subset.
    UnsupportedChunk,
    /// The chunk may appear only once.
    Duplicate,
    InvalidValue {
        found: String,
        expected: String,
    },
    /// The file ends inside a chunk.
    Truncated,
    /// A required chunk (`IDAT` or `IEND`) is missing.
    MissingChunk,
    /// The decoder Bevy uses rejected the image (message from the decoder).
    Decode(String),
}

impl fmt::Display for TilesetImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "tileset image {}: {}: {}",
            self.file.display(),
            self.field,
            self.kind
        )
    }
}

impl std::error::Error for TilesetImageError {}

impl fmt::Display for TilesetImageErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(f, "cannot be read: {error}"),
            Self::FileTooLarge { max_bytes } => {
                write!(f, "the file is larger than {max_bytes} bytes")
            }
            Self::NotPng => write!(f, "not a PNG file"),
            Self::SizeMismatch {
                width,
                height,
                declared_width,
                declared_height,
            } => write!(
                f,
                "the image is {width}x{height} px, but the map declares \
                 {declared_width}x{declared_height} px"
            ),
            Self::UnsupportedChunk => write!(f, "chunk is not supported"),
            Self::Duplicate => write!(f, "defined more than once"),
            Self::InvalidValue { found, expected } => {
                write!(f, "value {found} is not supported, expected {expected}")
            }
            Self::Truncated => write!(f, "the file ends inside this chunk"),
            Self::MissingChunk => write!(f, "required chunk is missing"),
            Self::Decode(error) => write!(f, "cannot be decoded: {error}"),
        }
    }
}

/// Checks the tileset image of the map at `map_path` against the supported subset and returns
/// the bytes that were checked, which are the only bytes the renderer may be given.
pub fn check_tileset_image(
    map_path: &Path,
    image: &TilesetImage,
) -> Result<Vec<u8>, TilesetImageError> {
    let path = map_path
        .parent()
        .unwrap_or(Path::new(""))
        .join(&image.file_name);
    let error = |field: &str, kind| TilesetImageError {
        file: path.clone(),
        field: field.to_owned(),
        kind,
    };
    let bytes = read_bounded(&path).map_err(|kind| error("file", kind))?;
    check_header(&bytes, image).map_err(|kind| error("file", kind))?;
    check_chunks(&bytes).map_err(|(field, kind)| error(&field, kind))?;
    decode(&bytes, image).map_err(|kind| error("image data", kind))?;
    Ok(bytes)
}

/// Reads the file without reading more than [`MAX_TILESET_IMAGE_BYTES`] + 1 bytes.
fn read_bounded(path: &Path) -> Result<Vec<u8>, TilesetImageErrorKind> {
    let file = std::fs::File::open(path).map_err(|e| TilesetImageErrorKind::Read(e.to_string()))?;
    let mut bytes = Vec::new();
    file.take(MAX_TILESET_IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| TilesetImageErrorKind::Read(e.to_string()))?;
    if u64::try_from(bytes.len()).map_or(true, |len| len > MAX_TILESET_IMAGE_BYTES) {
        return Err(TilesetImageErrorKind::FileTooLarge {
            max_bytes: MAX_TILESET_IMAGE_BYTES,
        });
    }
    Ok(bytes)
}

/// The signature and the image size of the `IHDR` chunk, which must come first and equal the
/// size the map declares (M1 review 1, P1-1).
fn check_header(bytes: &[u8], image: &TilesetImage) -> Result<(), TilesetImageErrorKind> {
    let Some(header) = bytes.get(..24) else {
        let prefix = &bytes[..bytes.len().min(SIGNATURE.len())];
        return Err(if SIGNATURE.starts_with(prefix) {
            TilesetImageErrorKind::Read("the file ends inside the PNG header".into())
        } else {
            TilesetImageErrorKind::NotPng
        });
    };
    // The first chunk of a PNG file is IHDR, 13 bytes long, starting with the width and height
    // as big-endian 32-bit integers.
    if &header[..8] != SIGNATURE || &header[8..16] != b"\0\0\0\x0dIHDR" {
        return Err(TilesetImageErrorKind::NotPng);
    }
    let width = u32::from_be_bytes([header[16], header[17], header[18], header[19]]);
    let height = u32::from_be_bytes([header[20], header[21], header[22], header[23]]);
    if (width, height) != (image.width, image.height) {
        return Err(TilesetImageErrorKind::SizeMismatch {
            width,
            height,
            declared_width: image.width,
            declared_height: image.height,
        });
    }
    Ok(())
}

type ChunkIssue = (String, TilesetImageErrorKind);

fn chunk_field(kind: &[u8]) -> String {
    format!("chunk {:?}", String::from_utf8_lossy(kind))
}

fn invalid(field: String, found: impl ToString, expected: &str) -> ChunkIssue {
    (
        field,
        TilesetImageErrorKind::InvalidValue {
            found: found.to_string(),
            expected: expected.to_owned(),
        },
    )
}

/// Walks the chunks after the signature; see the module documentation for the subset.
fn check_chunks(bytes: &[u8]) -> Result<(), ChunkIssue> {
    let mut position = SIGNATURE.len();
    let mut seen_ancillary = [false; ANCILLARY_CHUNKS.len()];
    // 0: IHDR expected; 1: ancillary chunks or the first IDAT; 2: inside IDAT; 3: after IEND.
    let mut stage = 0;
    while position < bytes.len() {
        if stage == 3 {
            return Err(invalid(
                "content after chunk \"IEND\"".into(),
                format!("{} bytes", bytes.len() - position),
                "the end of the file",
            ));
        }
        let Some(head) = bytes.get(position..position + 8) else {
            return Err((
                format!("chunk at byte {position}"),
                TilesetImageErrorKind::Truncated,
            ));
        };
        let length = u32::from_be_bytes([head[0], head[1], head[2], head[3]]);
        let kind: &[u8; 4] = head[4..8].try_into().unwrap_or(b"????");
        let field = chunk_field(kind);
        // The data and the 4-byte CRC; the CRC itself is verified by the decoder.
        let end = usize::try_from(length)
            .ok()
            .and_then(|length| (position + 8).checked_add(length))
            .and_then(|data_end| data_end.checked_add(4))
            .filter(|&end| end <= bytes.len())
            .ok_or_else(|| (field.clone(), TilesetImageErrorKind::Truncated))?;
        match (stage, kind) {
            (0, b"IHDR") => {
                check_ihdr(&bytes[position + 8..end - 4])?;
                stage = 1;
            }
            (1, kind) if ANCILLARY_CHUNKS.iter().any(|(name, _)| *name == kind) => {
                let index = ANCILLARY_CHUNKS
                    .iter()
                    .position(|(name, _)| *name == kind)
                    .unwrap_or(0);
                let expected_length = ANCILLARY_CHUNKS[index].1;
                if length != expected_length {
                    return Err(invalid(
                        format!("{field} length"),
                        length,
                        &format!("{expected_length} bytes"),
                    ));
                }
                if std::mem::replace(&mut seen_ancillary[index], true) {
                    return Err((field, TilesetImageErrorKind::Duplicate));
                }
            }
            (1 | 2, b"IDAT") => stage = 2,
            (1, b"IEND") => {
                return Err((chunk_field(b"IDAT"), TilesetImageErrorKind::MissingChunk));
            }
            (2, b"IEND") => {
                if length != 0 {
                    return Err(invalid(format!("{field} length"), length, "0 bytes"));
                }
                stage = 3;
            }
            // After the image data only IEND may follow, so IDAT chunks are consecutive.
            (_, kind) if ANCILLARY_CHUNKS.iter().any(|(name, _)| *name == kind) => {
                return Err(invalid(field, "a chunk after IDAT", "a chunk before IDAT"));
            }
            _ => return Err((field, TilesetImageErrorKind::UnsupportedChunk)),
        }
        position = end;
    }
    if stage != 3 {
        let missing: &[u8] = if stage < 2 { b"IDAT" } else { b"IEND" };
        return Err((chunk_field(missing), TilesetImageErrorKind::MissingChunk));
    }
    Ok(())
}

/// The `IHDR` fields other than the size: 8 bits per channel, RGB or RGBA, no interlacing.
fn check_ihdr(data: &[u8]) -> Result<(), ChunkIssue> {
    let field = |name: &str| format!("chunk \"IHDR\" {name}");
    let [
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        depth,
        colour,
        compression,
        filter,
        interlace,
    ] = data
    else {
        return Err(invalid(field("length"), data.len(), "13 bytes"));
    };
    if *depth != 8 {
        return Err(invalid(field("bit depth"), depth, "8"));
    }
    if !matches!(colour, 2 | 6) {
        return Err(invalid(field("colour type"), colour, "2 (RGB) or 6 (RGBA)"));
    }
    if *compression != 0 || *filter != 0 {
        return Err(invalid(
            field("compression and filter methods"),
            format!("{compression}, {filter}"),
            "0, 0",
        ));
    }
    if *interlace != 0 {
        return Err(invalid(field("interlace method"), interlace, "0"));
    }
    Ok(())
}

/// Decodes the image as Bevy does (`bevy_image` 0.19.1, `Image::from_buffer`: an
/// `image::ImageReader` over the bytes with the format set to PNG), but under limits, and checks
/// the result.
fn decode(bytes: &[u8], image: &TilesetImage) -> Result<(), TilesetImageErrorKind> {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_TILESET_IMAGE_SIDE_PX);
    limits.max_image_height = Some(MAX_TILESET_IMAGE_SIDE_PX);
    limits.max_alloc = Some(DECODE_MEMORY_LIMIT);
    let mut reader = ImageReader::new(Cursor::new(bytes));
    reader.set_format(ImageFormat::Png);
    reader.limits(limits);
    let decoded = reader
        .decode()
        .map_err(|e| TilesetImageErrorKind::Decode(e.to_string()))?;
    let size = (decoded.width(), decoded.height());
    if !matches!(
        decoded,
        DynamicImage::ImageRgb8(_) | DynamicImage::ImageRgba8(_)
    ) || size != (image.width, image.height)
    {
        return Err(TilesetImageErrorKind::InvalidValue {
            found: format!("a {:?} image of {}x{} px", decoded.color(), size.0, size.1),
            expected: format!(
                "an RGB8 or RGBA8 image of {}x{} px",
                image.width, image.height
            ),
        });
    }
    Ok(())
}
