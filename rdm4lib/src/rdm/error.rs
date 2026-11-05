use std::fmt;

/// A rejected input or inconsistent output, with its byte offset and field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RdmError {
    offset: usize,
    context: &'static str,
    expected: String,
    actual: String,
}

impl RdmError {
    pub(super) fn new(
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

    /// Input or output byte offset where validation failed.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Record, field or framing component being checked.
    pub fn context(&self) -> &str {
        self.context
    }

    /// Required value or constraint.
    pub fn expected(&self) -> &str {
        &self.expected
    }

    /// Observed value or constraint violation.
    pub fn actual(&self) -> &str {
        &self.actual
    }
}

impl fmt::Display for RdmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at {:#x}: expected {}, found {}",
            self.context, self.offset, self.expected, self.actual
        )
    }
}

impl std::error::Error for RdmError {}
