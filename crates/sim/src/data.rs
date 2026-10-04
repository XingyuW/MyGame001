//! Data files: the supported subset of RON (docs/DECISIONS.md D18; invariants I8 and I11).
//!
//! A data file is handed to `ron` only after the checker below has accepted it. The checker reads
//! the raw bytes with a grammar that is a strict subset of RON and is driven by the schema of the
//! file, so that everything it accepts has exactly one reading in `ron` and matches the schema:
//!
//! * the file is a regular file of at most [`MAX_DATA_FILE_BYTES`], read with that bound, and
//!   holds only printable ASCII, spaces, tabs, and line breaks (so no byte order mark);
//! * between tokens only whitespace and `//` line comments may appear; block comments,
//!   attributes such as `#![enable(...)]` (which switch on `ron` extensions), raw and byte
//!   strings, characters, floats, maps, tuples, enum variants, `Some`/`None`, booleans, and struct
//!   names are outside the subset;
//! * a value is an integer (`-`, then decimal digits without leading zeros, within `i64`), a
//!   string (`"`, printable ASCII without `"` and `\`, at most [`MAX_STRING_BYTES`] bytes, `"`),
//!   a list (`[`, values separated by commas, an optional trailing comma, `]`, at most as many
//!   entries as the schema allows), or a struct (`(`, `name: value` pairs separated by commas, an
//!   optional trailing comma, `)`);
//! * a struct holds exactly the fields its schema lists, each once, in any order, and every
//!   value has the kind its schema names.
//!
//! The schema is at most six levels deep (the combat file's integers inside structs inside lists
//! inside structs inside lists inside the file's struct) and the checker recurses on the schema,
//! never on the input, so its stack depth is bounded however deeply a file nests. Errors are typed
//! [`DataError`]s naming the file, the field (as a path such as `units[2].speed`), and the
//! reason.

use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Largest accepted data file in bytes.
pub const MAX_DATA_FILE_BYTES: u64 = 64 * 1024;

/// Longest accepted string value in bytes.
pub const MAX_STRING_BYTES: usize = 64;

/// The recursion limit given to `ron`. The deepest file the checker accepts nests six levels (five
/// containers); the limit is a second line of defence.
pub(crate) const RON_RECURSION_LIMIT: usize = 16;

/// Why a data file was rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataError {
    pub file: PathBuf,
    /// The field at fault as a path, e.g. `units[2].speed`, or `file` for the file as a whole.
    pub field: String,
    pub kind: DataErrorKind,
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "data file {}: {}: {}",
            self.file.display(),
            self.field,
            self.kind
        )
    }
}

impl std::error::Error for DataError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataErrorKind {
    /// The file could not be read, or is not a regular file.
    Read(String),
    /// The file is larger than [`MAX_DATA_FILE_BYTES`].
    FileTooLarge { max_bytes: u64 },
    /// The text is outside the supported subset of RON at the given position (1-based line and
    /// column).
    Syntax {
        line: usize,
        column: usize,
        expected: String,
        found: String,
    },
    /// The struct has no field of this name in its schema.
    UnknownField,
    /// A field the schema requires is missing (no field is optional, so there are no silent
    /// defaults).
    MissingField,
    /// The field appears more than once.
    DuplicateField,
    /// The list holds more entries than the schema allows.
    TooManyEntries { max: usize },
    /// The value is outside the range or pattern the schema allows.
    InvalidValue { found: String, expected: String },
    /// The file declares a schema version this crate does not read.
    UnsupportedSchemaVersion { found: i64, expected: i64 },
    /// An identifier that must be unique appears twice.
    DuplicateId(String),
    /// The value names a building or unit type (`kind`) that is not defined.
    UnknownReference { kind: String, id: String },
    /// The prerequisites of the buildings form a cycle, listed from a building back to itself.
    PrerequisiteCycle(Vec<String>),
    /// The list has no entry for the `kind` named `id`, which needs one (for example a unit type
    /// without combat terms, or an armour class a warhead does not list).
    MissingEntry { kind: String, id: String },
    /// `ron` rejected a file that the checker accepted (message from `ron`); this indicates a
    /// difference between the checker and `ron` and is reported rather than ignored.
    Parse(String),
}

impl fmt::Display for DataErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(f, "cannot read the file: {error}"),
            Self::FileTooLarge { max_bytes } => {
                write!(f, "the file is larger than {max_bytes} bytes")
            }
            Self::Syntax {
                line,
                column,
                expected,
                found,
            } => write!(
                f,
                "line {line}, column {column}: expected {expected}, found {found}"
            ),
            Self::UnknownField => write!(f, "unknown field"),
            Self::MissingField => write!(f, "required field is missing"),
            Self::DuplicateField => write!(f, "field appears more than once"),
            Self::TooManyEntries { max } => write!(f, "more than {max} entries"),
            Self::InvalidValue { found, expected } => {
                write!(f, "value {found} is not supported, expected {expected}")
            }
            Self::UnsupportedSchemaVersion { found, expected } => write!(
                f,
                "schema version {found} is not supported, expected {expected}"
            ),
            Self::DuplicateId(id) => write!(f, "identifier {id:?} is defined more than once"),
            Self::UnknownReference { kind, id } => write!(f, "{kind} {id:?} is not defined"),
            Self::PrerequisiteCycle(cycle) => {
                write!(f, "the prerequisites form a cycle: {}", cycle.join(" -> "))
            }
            Self::MissingEntry { kind, id } => write!(f, "{kind} {id:?} has no entry"),
            Self::Parse(error) => write!(f, "cannot be parsed: {error}"),
        }
    }
}

/// Reads a regular file without reading more than `max_bytes` + 1 bytes. Anything that is not a
/// regular file (a directory, a device, or a named pipe, which would block the reader until a
/// writer appears) is rejected before it is opened.
pub(crate) fn read_bounded(path: &Path, max_bytes: u64) -> Result<Vec<u8>, DataErrorKind> {
    let metadata = std::fs::metadata(path).map_err(|e| DataErrorKind::Read(e.to_string()))?;
    if !metadata.is_file() {
        return Err(DataErrorKind::Read("not a regular file".into()));
    }
    let file = std::fs::File::open(path).map_err(|e| DataErrorKind::Read(e.to_string()))?;
    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| DataErrorKind::Read(e.to_string()))?;
    if u64::try_from(bytes.len()).map_or(true, |len| len > max_bytes) {
        return Err(DataErrorKind::FileTooLarge { max_bytes });
    }
    Ok(bytes)
}

// --- Schema ---------------------------------------------------------------------------------

/// The shape of a value in a data file.
#[derive(Debug)]
pub(crate) enum Shape {
    Int,
    /// An integer that must equal the schema version (checked as soon as it is read, so that a
    /// file of another version is reported as such).
    SchemaVersion(i64),
    Str,
    /// A list of values of the inner shape, with at most `max` entries.
    List {
        entry: &'static Shape,
        max: usize,
    },
    /// A struct with exactly these fields.
    Struct(&'static [(&'static str, Shape)]),
}

// --- Lexer ----------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Token<'a> {
    Open(u8),
    Close(u8),
    Colon,
    Comma,
    Ident(&'a str),
    Int(&'a str),
    Str(&'a str),
    End,
}

impl Token<'_> {
    fn describe(&self) -> String {
        match self {
            Token::Open(c) | Token::Close(c) => format!("{:?}", char::from(*c)),
            Token::Colon => "\":\"".into(),
            Token::Comma => "\",\"".into(),
            Token::Ident(name) => format!("identifier {name:?}"),
            Token::Int(digits) => format!("integer {digits}"),
            Token::Str(text) => format!("string {text:?}"),
            Token::End => "the end of the file".into(),
        }
    }
}

/// A position-tracking lexer over ASCII text.
struct Lexer<'a> {
    text: &'a str,
    position: usize,
}

/// A syntax problem at a byte offset.
struct SyntaxIssue {
    offset: usize,
    expected: String,
    found: String,
}

impl<'a> Lexer<'a> {
    fn byte(&self, offset: usize) -> Option<u8> {
        self.text.as_bytes().get(offset).copied()
    }

    fn issue(&self, offset: usize, expected: &str, found: impl Into<String>) -> SyntaxIssue {
        SyntaxIssue {
            offset,
            expected: expected.to_owned(),
            found: found.into(),
        }
    }

    /// Skips whitespace and `//` comments.
    fn skip_trivia(&mut self) -> Result<(), SyntaxIssue> {
        loop {
            match self.byte(self.position) {
                Some(b' ' | b'\t' | b'\n' | b'\r') => self.position += 1,
                Some(b'/') => {
                    if self.byte(self.position + 1) != Some(b'/') {
                        return Err(self.issue(
                            self.position,
                            "a value or a \"//\" comment",
                            "\"/\" (block comments are not supported)",
                        ));
                    }
                    while let Some(byte) = self.byte(self.position) {
                        if byte == b'\n' {
                            break;
                        }
                        self.position += 1;
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    /// The next token and the offset at which it starts.
    fn next(&mut self) -> Result<(Token<'a>, usize), SyntaxIssue> {
        self.skip_trivia()?;
        let start = self.position;
        let Some(byte) = self.byte(start) else {
            return Ok((Token::End, start));
        };
        let token = match byte {
            b'(' | b'[' => {
                self.position += 1;
                Token::Open(byte)
            }
            b')' | b']' => {
                self.position += 1;
                Token::Close(byte)
            }
            b':' => {
                self.position += 1;
                Token::Colon
            }
            b',' => {
                self.position += 1;
                Token::Comma
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                let end = self.scan(start, |b| b.is_ascii_alphanumeric() || b == b'_');
                Token::Ident(&self.text[start..end])
            }
            b'-' | b'0'..=b'9' => self.integer(start)?,
            b'"' => self.string(start)?,
            other => {
                return Err(self.issue(
                    start,
                    "a value or punctuation of the supported subset",
                    format!("{:?}", char::from(other)),
                ));
            }
        };
        Ok((token, start))
    }

    fn scan(&mut self, start: usize, accept: impl Fn(u8) -> bool) -> usize {
        let mut end = start;
        while self.byte(end).is_some_and(&accept) {
            end += 1;
        }
        self.position = end;
        end
    }

    fn integer(&mut self, start: usize) -> Result<Token<'a>, SyntaxIssue> {
        let digits_start = if self.byte(start) == Some(b'-') {
            start + 1
        } else {
            start
        };
        let end = self.scan(digits_start, |b| b.is_ascii_digit());
        let digits = &self.text[digits_start..end];
        let expected = "a decimal integer without leading zeros";
        if digits.is_empty() || (digits.len() > 1 && digits.starts_with('0')) {
            return Err(self.issue(start, expected, format!("{:?}", &self.text[start..end])));
        }
        // `1.5`, `1e3`, `0x1F`, `1_000`, and `5u8` are outside the subset.
        if let Some(next) = self.byte(end)
            && (next.is_ascii_alphanumeric() || matches!(next, b'_' | b'.'))
        {
            return Err(self.issue(
                start,
                expected,
                format!(
                    "{:?} followed by {:?}",
                    &self.text[start..end],
                    char::from(next)
                ),
            ));
        }
        Ok(Token::Int(&self.text[start..end]))
    }

    fn string(&mut self, start: usize) -> Result<Token<'a>, SyntaxIssue> {
        let content_start = start + 1;
        let mut end = content_start;
        loop {
            match self.byte(end) {
                Some(b'"') => break,
                Some(b'\\') => {
                    return Err(self.issue(end, "a string without escape sequences", "\"\\\""));
                }
                Some(byte) if (b' '..=b'~').contains(&byte) => end += 1,
                Some(byte) => {
                    return Err(self.issue(
                        end,
                        "a string on one line",
                        format!("{:?}", char::from(byte)),
                    ));
                }
                None => {
                    return Err(self.issue(end, "the end of the string", "the end of the file"));
                }
            }
        }
        self.position = end + 1;
        Ok(Token::Str(&self.text[content_start..end]))
    }
}

// --- Checker --------------------------------------------------------------------------------

/// Checks `bytes`, read from `path`, against `schema`. On success the text is returned; it is the
/// only text that may be handed to `ron`.
pub(crate) fn check<'a>(
    path: &Path,
    bytes: &'a [u8],
    schema: &Shape,
) -> Result<&'a str, DataError> {
    let error = |field: &str, kind| DataError {
        file: path.to_owned(),
        field: field.to_owned(),
        kind,
    };
    // Printable ASCII and whitespace only: this also excludes a byte order mark and makes byte
    // offsets equal to column numbers.
    if let Some(offset) = bytes
        .iter()
        .position(|&b| !matches!(b, b' '..=b'~' | b'\t' | b'\n' | b'\r'))
    {
        let (line, column) = line_and_column(bytes, offset);
        return Err(error(
            "file",
            DataErrorKind::Syntax {
                line,
                column,
                expected: "printable ASCII, tabs, and line breaks".into(),
                found: format!("byte 0x{:02x}", bytes[offset]),
            },
        ));
    }
    let text = std::str::from_utf8(bytes).map_err(|e| {
        // Unreachable after the ASCII check; reported rather than unwrapped (invariant I8).
        error("file", DataErrorKind::Read(e.to_string()))
    })?;
    let mut checker = Checker {
        lexer: Lexer { text, position: 0 },
    };
    let to_error = |(field, issue): (String, SyntaxIssue)| {
        let (line, column) = line_and_column(bytes, issue.offset);
        error(
            &field,
            DataErrorKind::Syntax {
                line,
                column,
                expected: issue.expected,
                found: issue.found,
            },
        )
    };
    match checker.value(schema, "file") {
        Ok(()) => {}
        Err(Problem::Syntax(field, issue)) => return Err(to_error((field, issue))),
        Err(Problem::Field(field, kind)) => return Err(error(&field, kind)),
    }
    let (token, offset) = checker
        .lexer
        .next()
        .map_err(|issue| to_error(("file".into(), issue)))?;
    if token != Token::End {
        return Err(to_error((
            "file".into(),
            checker
                .lexer
                .issue(offset, "the end of the file", token.describe()),
        )));
    }
    Ok(text)
}

/// 1-based line and column of a byte offset in ASCII text.
fn line_and_column(bytes: &[u8], offset: usize) -> (usize, usize) {
    let before = &bytes[..offset.min(bytes.len())];
    let line = 1 + before.iter().filter(|&&b| b == b'\n').count();
    let line_start = before
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |newline| newline + 1);
    (line, 1 + before.len() - line_start)
}

enum Problem {
    Syntax(String, SyntaxIssue),
    Field(String, DataErrorKind),
}

impl From<(String, SyntaxIssue)> for Problem {
    fn from((field, issue): (String, SyntaxIssue)) -> Self {
        Problem::Syntax(field, issue)
    }
}

struct Checker<'a> {
    lexer: Lexer<'a>,
}

impl<'a> Checker<'a> {
    fn next(&mut self, field: &str) -> Result<(Token<'a>, usize), Problem> {
        self.lexer
            .next()
            .map_err(|issue| Problem::Syntax(field.to_owned(), issue))
    }

    fn unexpected(&self, field: &str, offset: usize, expected: &str, found: Token<'_>) -> Problem {
        Problem::Syntax(
            field.to_owned(),
            self.lexer.issue(offset, expected, found.describe()),
        )
    }

    /// Checks one value of `shape` at `field`. Recurses only on the schema.
    fn value(&mut self, shape: &Shape, field: &str) -> Result<(), Problem> {
        let (token, offset) = self.next(field)?;
        match shape {
            Shape::Int | Shape::SchemaVersion(_) => {
                let Token::Int(digits) = token else {
                    return Err(self.unexpected(field, offset, "an integer", token));
                };
                let value = digits.parse::<i64>().map_err(|_| {
                    Problem::Field(
                        field.to_owned(),
                        DataErrorKind::InvalidValue {
                            found: digits.to_owned(),
                            expected: "a 64-bit signed integer".into(),
                        },
                    )
                })?;
                if let Shape::SchemaVersion(expected) = shape
                    && value != *expected
                {
                    return Err(Problem::Field(
                        field.to_owned(),
                        DataErrorKind::UnsupportedSchemaVersion {
                            found: value,
                            expected: *expected,
                        },
                    ));
                }
                Ok(())
            }
            Shape::Str => {
                let Token::Str(text) = token else {
                    return Err(self.unexpected(field, offset, "a string", token));
                };
                if text.len() > MAX_STRING_BYTES {
                    return Err(Problem::Field(
                        field.to_owned(),
                        DataErrorKind::InvalidValue {
                            found: format!("of {} bytes", text.len()),
                            expected: format!("a string of at most {MAX_STRING_BYTES} bytes"),
                        },
                    ));
                }
                Ok(())
            }
            Shape::List { entry, max } => {
                if token != Token::Open(b'[') {
                    return Err(self.unexpected(field, offset, "a list \"[\"", token));
                }
                let mut count = 0_usize;
                loop {
                    // An empty list, or a trailing comma before the closing bracket.
                    if self.peek_close(b']', field)? {
                        return Ok(());
                    }
                    if count == *max {
                        return Err(Problem::Field(
                            field.to_owned(),
                            DataErrorKind::TooManyEntries { max: *max },
                        ));
                    }
                    self.value(entry, &format!("{field}[{count}]"))?;
                    count += 1;
                    let (token, offset) = self.next(field)?;
                    match token {
                        Token::Comma => {}
                        Token::Close(b']') => return Ok(()),
                        other => {
                            return Err(self.unexpected(field, offset, "\",\" or \"]\"", other));
                        }
                    }
                }
            }
            Shape::Struct(fields) => {
                if token != Token::Open(b'(') {
                    return Err(self.unexpected(field, offset, "a struct \"(\"", token));
                }
                let mut seen = vec![false; fields.len()];
                loop {
                    if self.peek_close(b')', field)? {
                        break;
                    }
                    let (token, offset) = self.next(field)?;
                    let Token::Ident(name) = token else {
                        return Err(self.unexpected(field, offset, "a field name", token));
                    };
                    let path = if field == "file" {
                        name.to_owned()
                    } else {
                        format!("{field}.{name}")
                    };
                    let Some(index) = fields.iter().position(|(known, _)| *known == name) else {
                        return Err(Problem::Field(path, DataErrorKind::UnknownField));
                    };
                    if std::mem::replace(&mut seen[index], true) {
                        return Err(Problem::Field(path, DataErrorKind::DuplicateField));
                    }
                    let (token, offset) = self.next(&path)?;
                    if token != Token::Colon {
                        return Err(self.unexpected(&path, offset, "\":\"", token));
                    }
                    self.value(&fields[index].1, &path)?;
                    let (token, offset) = self.next(field)?;
                    match token {
                        Token::Comma => {}
                        Token::Close(b')') => break,
                        other => {
                            return Err(self.unexpected(field, offset, "\",\" or \")\"", other));
                        }
                    }
                }
                match seen.iter().position(|&present| !present) {
                    Some(missing) => {
                        let name = fields[missing].0;
                        let path = if field == "file" {
                            name.to_owned()
                        } else {
                            format!("{field}.{name}")
                        };
                        Err(Problem::Field(path, DataErrorKind::MissingField))
                    }
                    None => Ok(()),
                }
            }
        }
    }

    /// Consumes the closing bracket `close` if it is the next token.
    fn peek_close(&mut self, close: u8, field: &str) -> Result<bool, Problem> {
        let saved = self.lexer.position;
        let (token, _) = self.next(field)?;
        if token == Token::Close(close) {
            return Ok(true);
        }
        self.lexer.position = saved;
        Ok(false)
    }
}

/// Parses checked text with `ron`, into a type whose fields match the schema the text was
/// checked against.
pub(crate) fn parse<T: serde::de::DeserializeOwned>(
    path: &Path,
    text: &str,
) -> Result<T, DataError> {
    ron::Options::default()
        .with_recursion_limit(RON_RECURSION_LIMIT)
        .from_str(text)
        .map_err(|e| DataError {
            file: path.to_owned(),
            field: "file".into(),
            kind: DataErrorKind::Parse(e.to_string()),
        })
}
