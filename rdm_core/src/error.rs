use std::fmt;

/// A rejected input or an inconsistent document.
///
/// `offset` is an input offset for read errors and an output offset for
/// write errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    offset: usize,
    context: &'static str,
    expected: String,
    actual: String,
}

impl Error {
    pub(crate) fn new(
        offset: usize,
        context: &'static str,
        expected: impl fmt::Display,
        actual: impl fmt::Display,
    ) -> Self {
        Self {
            offset,
            context,
            expected: expected.to_string(),
            actual: actual.to_string(),
        }
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Record and field, such as `Model.vertices`.
    pub fn context(&self) -> &str {
        self.context
    }

    pub fn expected(&self) -> &str {
        &self.expected
    }

    pub fn actual(&self) -> &str {
        &self.actual
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at {:#x}: expected {}, found {}",
            self.context, self.offset, self.expected, self.actual
        )
    }
}

impl std::error::Error for Error {}
