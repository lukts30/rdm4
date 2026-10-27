use std::fmt;

/// A read or write failure with its file offset and record path.
#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    offset: u64,
    path: String,
    kind: ErrorKind,
}

/// What went wrong.
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorKind {
    /// A read would run past the end of the input.
    Truncated { end: u64, len: u64 },
    /// A decoded value differs from what the format requires.
    Mismatch {
        what: &'static str,
        expected: String,
        actual: String,
    },
    /// Facet rejected a value while building or inspecting a record.
    Reflect(String),
}

impl Error {
    pub(crate) fn new(offset: u64, path: String, kind: ErrorKind) -> Self {
        Error { offset, path, kind }
    }

    /// File offset (input for reads, output for writes) where the error was detected.
    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// Record path, such as `models[0].groups`.
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }
}

impl ErrorKind {
    pub(crate) fn mismatch(
        what: &'static str,
        expected: impl fmt::Display,
        actual: impl fmt::Display,
    ) -> Self {
        ErrorKind::Mismatch {
            what,
            expected: expected.to_string(),
            actual: actual.to_string(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "at {:#x} in `{}`: ", self.offset, self.path)?;
        match &self.kind {
            ErrorKind::Truncated { end, len } => {
                write!(f, "data ends at {end:#x} but input length is {len:#x}")
            }
            ErrorKind::Mismatch {
                what,
                expected,
                actual,
            } => write!(f, "{what}: expected {expected}, found {actual}"),
            ErrorKind::Reflect(message) => write!(f, "reflection error: {message}"),
        }
    }
}

impl std::error::Error for Error {}

/// One step of a record path.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Segment {
    Field(&'static str),
    Index(u32),
}

/// Tracks the record path during traversal; formatted only on error.
#[derive(Debug, Default)]
pub(crate) struct Path {
    segments: Vec<Segment>,
}

impl Path {
    pub(crate) fn push(&mut self, segment: Segment) {
        self.segments.push(segment);
    }

    pub(crate) fn pop(&mut self) {
        self.segments.pop();
    }

    pub(crate) fn error(&self, offset: u64, kind: ErrorKind) -> Error {
        Error::new(offset, self.to_string(), kind)
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.segments.is_empty() {
            return write!(f, "root");
        }
        for (i, segment) in self.segments.iter().enumerate() {
            match segment {
                Segment::Field(name) if i == 0 => write!(f, "{name}")?,
                Segment::Field(name) => write!(f, ".{name}")?,
                Segment::Index(index) => write!(f, "[{index}]")?,
            }
        }
        Ok(())
    }
}
