//! Errors with input offset, record/field context and expected values.

use std::fmt;

use crate::records::StorageType;

/// A malformed input or a document that cannot be written.
#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    /// Byte offset in the input (reading) or output (writing).
    pub offset: u64,
    /// Record and field being processed, such as `Model.indices`.
    pub context: &'static str,
    pub kind: ErrorKind,
}

impl Error {
    pub(crate) fn new(offset: usize, context: &'static str, kind: ErrorKind) -> Self {
        Error {
            offset: offset as u64,
            context,
            kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorKind {
    /// The file does not start with `RDM\x00` or `RDM\x01`.
    BadSignature { actual: Vec<u8> },
    /// A word with a fixed value has another value.
    UnexpectedValue { expected: u32, actual: u32 },
    /// An enumerated code is not one of the known values.
    UnknownCode {
        expected: &'static [u32],
        actual: u32,
    },
    /// A required reference is zero.
    NullReference,
    /// A reference does not point to the next block in physical order.
    UnexpectedTarget { expected: u64, actual: u64 },
    /// A block extends past the end of the input.
    OutOfBounds { end: u64, len: u64 },
    /// A record block has a stride not allowed for its record type.
    UnsupportedStride {
        expected: &'static [u32],
        actual: u32,
    },
    /// A block's stride disagrees with its format description.
    StrideMismatch { expected: u64, actual: u64 },
    /// A block or collection has the wrong number of elements.
    CountMismatch { expected: u64, actual: u64 },
    /// A storage type is not supported for this payload.
    UnsupportedStorage { actual: StorageType },
    /// Stored values disagree with the storage type in their format.
    StorageMismatch {
        expected: StorageType,
        actual: StorageType,
    },
    /// A vertex format without attributes.
    EmptyVertexFormat,
    /// Bytes remain after the last block.
    TrailingBytes { len: u64 },
    /// The document's structure no longer matches its preserved layout.
    LayoutChanged,
    /// An offset or size does not fit the format's 32-bit fields.
    Overflow,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} at {:#x}: {}", self.context, self.offset, self.kind)
    }
}

impl std::error::Error for Error {}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ErrorKind::BadSignature { actual } => {
                write!(
                    f,
                    "expected signature RDM\\x00 or RDM\\x01, found {actual:02x?}"
                )
            }
            ErrorKind::UnexpectedValue { expected, actual } => {
                write!(f, "expected {expected:#x}, found {actual:#x}")
            }
            ErrorKind::UnknownCode { expected, actual } => {
                write!(f, "expected one of {expected:?}, found {actual}")
            }
            ErrorKind::NullReference => write!(f, "expected a block reference, found null"),
            ErrorKind::UnexpectedTarget { expected, actual } => {
                let problem = if actual < expected {
                    "backward or shared reference"
                } else {
                    "gap or reordered block"
                };
                write!(f, "{problem}: expected {expected:#x}, found {actual:#x}")
            }
            ErrorKind::OutOfBounds { end, len } => {
                write!(f, "block ends at {end:#x}, past input length {len:#x}")
            }
            ErrorKind::UnsupportedStride { expected, actual } => {
                write!(f, "expected stride {expected:?}, found {actual}")
            }
            ErrorKind::StrideMismatch { expected, actual } => {
                write!(f, "expected stride {expected}, found {actual}")
            }
            ErrorKind::CountMismatch { expected, actual } => {
                write!(f, "expected {expected} elements, found {actual}")
            }
            ErrorKind::UnsupportedStorage { actual } => {
                write!(f, "storage type {actual:?} is not supported here")
            }
            ErrorKind::StorageMismatch { expected, actual } => {
                write!(f, "expected {expected:?} values, found {actual:?}")
            }
            ErrorKind::EmptyVertexFormat => write!(f, "expected at least one vertex attribute"),
            ErrorKind::TrailingBytes { len } => {
                write!(f, "expected end of input, found {len} more bytes")
            }
            ErrorKind::LayoutChanged => {
                write!(
                    f,
                    "document structure no longer matches the layout it was read with"
                )
            }
            ErrorKind::Overflow => write!(f, "value exceeds the format's 32-bit range"),
        }
    }
}

pub(crate) type Result<T> = std::result::Result<T, Error>;
