//! Bounded ingestion of one selected file's textual patch.
//!
//! Callers feed decoded transport chunks and stop reading at the first error.
//! A successful finish proves only that text fits the native resource limits;
//! the caller must still bind it to the exact file membership and range. This
//! collector never treats a missing provider patch as an empty textual diff.

use super::{MAX_PULL_FILE_LINE_BYTES, MAX_PULL_FILE_TEXT_BYTES, MAX_PULL_FILE_TEXT_LINES};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullFileTextError {
    ByteLimit,
    LineLimit,
    LineLengthLimit,
    InvalidUtf8,
    NulByte,
}

impl PullFileTextError {
    pub const fn is_oversized(self) -> bool {
        matches!(
            self,
            Self::ByteLimit | Self::LineLimit | Self::LineLengthLimit
        )
    }
}

/// At most one selected patch is retained. Failure immediately drops its body,
/// and every later operation returns the same error, preventing a caller from
/// accidentally accepting a prefix after a transport limit has been reached.
#[derive(Debug)]
pub struct PullFileTextCollector {
    bytes: Vec<u8>,
    line_bytes: usize,
    lines: usize,
    error: Option<PullFileTextError>,
}

impl Default for PullFileTextCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl PullFileTextCollector {
    pub fn new() -> Self {
        Self {
            bytes: Vec::new(),
            line_bytes: 0,
            lines: 1,
            error: None,
        }
    }

    pub fn push(&mut self, chunk: &[u8]) -> Result<(), PullFileTextError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if chunk.len() > MAX_PULL_FILE_TEXT_BYTES.saturating_sub(self.bytes.len()) {
            return self.reject(PullFileTextError::ByteLimit);
        }
        for byte in chunk {
            match byte {
                0 => return self.reject(PullFileTextError::NulByte),
                b'\n' => {
                    self.lines += 1;
                    self.line_bytes = 0;
                    if self.lines > MAX_PULL_FILE_TEXT_LINES {
                        return self.reject(PullFileTextError::LineLimit);
                    }
                }
                _ => {
                    self.line_bytes += 1;
                    if self.line_bytes > MAX_PULL_FILE_LINE_BYTES {
                        return self.reject(PullFileTextError::LineLengthLimit);
                    }
                }
            }
        }
        // Grow geometrically even for tiny transport chunks, but cap the
        // requested capacity so a near-limit chunk cannot double retention.
        let needed = self.bytes.len() + chunk.len();
        if needed > self.bytes.capacity() {
            let capacity = self
                .bytes
                .capacity()
                .max(4096)
                .saturating_mul(2)
                .max(needed)
                .min(MAX_PULL_FILE_TEXT_BYTES);
            self.bytes.reserve_exact(capacity - self.bytes.len());
        }
        self.bytes.extend_from_slice(chunk);
        Ok(())
    }

    fn reject(&mut self, error: PullFileTextError) -> Result<(), PullFileTextError> {
        self.bytes = Vec::new();
        self.error = Some(error);
        Err(error)
    }

    pub fn finish(self) -> Result<String, PullFileTextError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        String::from_utf8(self.bytes).map_err(|_| PullFileTextError::InvalidUtf8)
    }
}

/// An already decoded, bounded JSON field still goes through exactly the same
/// limits as streamed bytes. The adapter handles `None` as omission before this
/// function, while `Some("")` remains a valid zero-byte textual observation.
pub fn bounded_pull_file_text(text: &str) -> Result<String, PullFileTextError> {
    let mut collector = PullFileTextCollector::new();
    collector.push(text.as_bytes())?;
    collector.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_utf8_and_empty_text_preserve_exact_bytes() {
        let text = "@@ -1 +1 @@\n-old\n+雪🙂\n";
        for split in 0..=text.len() {
            let mut collector = PullFileTextCollector::new();
            collector.push(&text.as_bytes()[..split]).unwrap();
            collector.push(&text.as_bytes()[split..]).unwrap();
            assert_eq!(collector.finish().unwrap(), text);
        }
        assert_eq!(bounded_pull_file_text("").unwrap(), "");
    }

    #[test]
    fn byte_limit_is_exact_and_failure_cannot_return_a_prefix() {
        let line = format!("{}\n", "a".repeat(1023));
        let text = line.repeat(MAX_PULL_FILE_TEXT_BYTES / line.len());
        assert_eq!(
            bounded_pull_file_text(&text).unwrap().len(),
            MAX_PULL_FILE_TEXT_BYTES
        );
        let mut collector = PullFileTextCollector::new();
        for chunk in text.as_bytes().chunks(4093) {
            collector.push(chunk).unwrap();
        }
        assert_eq!(collector.push(b"x"), Err(PullFileTextError::ByteLimit));
        assert_eq!(collector.bytes.capacity(), 0);
        assert_eq!(collector.push(b""), Err(PullFileTextError::ByteLimit));
        assert_eq!(collector.finish(), Err(PullFileTextError::ByteLimit));
    }

    #[test]
    fn line_limits_hold_across_transport_boundaries() {
        let mut collector = PullFileTextCollector::new();
        collector
            .push(&vec![b'a'; MAX_PULL_FILE_LINE_BYTES - 1])
            .unwrap();
        collector.push(b"b").unwrap();
        assert_eq!(
            collector.push(b"c"),
            Err(PullFileTextError::LineLengthLimit)
        );
        let mut collector = PullFileTextCollector::new();
        collector
            .push(&vec![b'\n'; MAX_PULL_FILE_TEXT_LINES - 1])
            .unwrap();
        assert_eq!(collector.push(b"\n"), Err(PullFileTextError::LineLimit));
        assert!(PullFileTextError::LineLimit.is_oversized());
        assert!(!PullFileTextError::InvalidUtf8.is_oversized());
    }

    #[test]
    fn malformed_text_cannot_be_lossily_accepted_as_a_patch() {
        let mut collector = PullFileTextCollector::new();
        collector.push(&[0xf0, 0x9f]).unwrap();
        assert_eq!(collector.finish(), Err(PullFileTextError::InvalidUtf8));
        let mut collector = PullFileTextCollector::new();
        assert_eq!(
            collector.push(b"diff\0tail"),
            Err(PullFileTextError::NulByte)
        );
        assert_eq!(collector.finish(), Err(PullFileTextError::NulByte));
    }

    #[test]
    fn tiny_chunks_keep_capacity_bounded_without_losing_bytes() {
        let mut collector = PullFileTextCollector::new();
        for _ in 0..20_000 {
            collector.push(b"a\n").unwrap();
            assert!(collector.bytes.capacity() <= MAX_PULL_FILE_TEXT_BYTES);
        }
        assert_eq!(collector.finish().unwrap().len(), 40_000);
    }
}
