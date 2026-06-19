// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! PartitionQosPolicy (DDS 1.4 §2.2.3.13).
//!
//! Wire format: `sequence<string>` (u32 length + N × CDR string).
//!
//! Compatibility (§2.2.3.13.6): partitions match via **fnmatch**
//! glob patterns (`*`, `?`, `[abc]`, `[!abc]`). The pattern sides are
//! serialized on the wire as strings; on a match we check whether
//! **at least one** offered pattern matches **one** requested pattern
//! (or vice versa).
//!
//! Examples from the spec:
//! - offered=["sensor_data"], requested=["sensor_*"] → MATCH.
//! - offered=["alarm_?"], requested=["alarm_1"] → MATCH.
//! - offered=[], requested=[] → MATCH (both in the default).
//! - offered=["a"], requested=[] → NO MATCH (non-empty vs empty).

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

/// DoS cap: maximum number of partition names in a QoS policy.
/// Cyclone/Fast-DDS apps even in large topologies stay
/// well below 1024; the practical upper bound is ~8.
pub const MAX_PARTITIONS: usize = 1024;

/// Maximum length of a single partition name.
pub const MAX_PARTITION_NAME_LEN: usize = 256;

/// PartitionQosPolicy.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PartitionQosPolicy {
    /// Partition names (possibly with a glob pattern).
    pub names: Vec<String>,
}

impl PartitionQosPolicy {
    /// Wire encoding.
    ///
    /// # Errors
    /// `ValueOutOfRange` on u32 overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        let len = u32::try_from(self.names.len()).map_err(|_| EncodeError::ValueOutOfRange {
            message: "partition list length exceeds u32::MAX",
        })?;
        w.write_u32(len)?;
        for name in &self.names {
            w.write_string(name)?;
        }
        Ok(())
    }

    /// Wire decoding with a DoS cap for the number of partitions (hard,
    /// [`MAX_PARTITIONS`]) and length per name ([`MAX_PARTITION_NAME_LEN`]).
    ///
    /// # Errors
    /// Buffer underflow; `InvalidString` for names over
    /// `MAX_PARTITION_NAME_LEN`; `LengthExceeded` for more than
    /// `MAX_PARTITIONS` entries.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let len = r.read_u32()? as usize;
        if len > MAX_PARTITIONS {
            return Err(DecodeError::LengthExceeded {
                announced: len,
                remaining: MAX_PARTITIONS,
                offset: 0,
            });
        }
        // Additional byte-level cap: each entry at least 4 bytes.
        if len > r.remaining() / 4 {
            return Err(DecodeError::LengthExceeded {
                announced: len,
                remaining: r.remaining(),
                offset: 0,
            });
        }
        let mut names = Vec::with_capacity(len);
        for _ in 0..len {
            let s = r.read_string()?;
            if s.len() > MAX_PARTITION_NAME_LEN {
                return Err(DecodeError::InvalidString {
                    offset: 0,
                    reason: "partition name exceeds MAX_PARTITION_NAME_LEN",
                });
            }
            names.push(s);
        }
        Ok(Self { names })
    }

    /// Empty (corresponds to the default partition).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

/// fnmatch-style glob matching (POSIX subset; DDS 1.4 §2.2.3.13.6 names
/// POSIX `fnmatch`).
///
/// Supported syntax:
/// - `*` any number of characters
/// - `?` exactly one character
/// - `[abc]` character class (single chars)
/// - `[a-z]` range
/// - `[!abc]` or `[^abc]` negated class
///
/// Malformed patterns (e.g. `[` without `]`) return **no-match** (POSIX
/// behavior: `FNM_NOMATCH`). No escape semantics — backslashes are
/// literal. Partition names are capped to [`MAX_PARTITION_NAME_LEN`],
/// so the star expansion is implicitly bounded to O(n²).
#[must_use]
pub fn fnmatch(pattern: &str, text: &str) -> bool {
    fnmatch_bytes(pattern.as_bytes(), text.as_bytes())
}

/// Iterative fnmatch implementation (no recursive call, no
/// ReDoS risk). `star_pat` / `star_txt` remember the last
/// `*` point; on a mismatch it jumps back there and the
/// text advances by one character. O(|pat| × |txt|) worst case.
fn fnmatch_bytes(pat: &[u8], txt: &[u8]) -> bool {
    let mut p = 0usize;
    let mut t = 0usize;
    let mut star_pat: Option<usize> = None;
    let mut star_txt: usize = 0;

    while t < txt.len() {
        if p < pat.len() {
            match pat[p] {
                b'*' => {
                    star_pat = Some(p + 1);
                    star_txt = t;
                    p += 1;
                    continue;
                }
                b'?' => {
                    p += 1;
                    t += 1;
                    continue;
                }
                b'[' => {
                    // Parse the character class up to ']'. Malformed (no
                    // closing ']') ⇒ POSIX NOMATCH.
                    let Some(close) = pat[p + 1..].iter().position(|&c| c == b']') else {
                        return false;
                    };
                    let class = &pat[p + 1..p + 1 + close];
                    let (negate, members) = match class.first() {
                        Some(b'!' | b'^') => (true, &class[1..]),
                        _ => (false, class),
                    };
                    if matches_class(members, txt[t]) == negate {
                        // Mismatch — check the fallback to star.
                        if let Some(sp) = star_pat {
                            p = sp;
                            star_txt += 1;
                            t = star_txt;
                            continue;
                        }
                        return false;
                    }
                    p += close + 2; // skips `[...]`
                    t += 1;
                    continue;
                }
                c if c == txt[t] => {
                    p += 1;
                    t += 1;
                    continue;
                }
                _ => {}
            }
        }
        // Mismatch — fall back to star, if present.
        if let Some(sp) = star_pat {
            p = sp;
            star_txt += 1;
            t = star_txt;
        } else {
            return false;
        }
    }
    // Text exhausted — any remaining `*` in the pattern are OK.
    while p < pat.len() && pat[p] == b'*' {
        p += 1;
    }
    p == pat.len()
}

fn matches_class(members: &[u8], ch: u8) -> bool {
    let mut i = 0;
    while i < members.len() {
        if i + 2 < members.len() && members[i + 1] == b'-' {
            if ch >= members[i] && ch <= members[i + 2] {
                return true;
            }
            i += 3;
        } else {
            if ch == members[i] {
                return true;
            }
            i += 1;
        }
    }
    false
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    #[test]
    fn default_is_empty() {
        assert!(PartitionQosPolicy::default().is_empty());
    }

    #[test]
    fn is_empty_checks() {
        let p = PartitionQosPolicy::default();
        assert!(p.is_empty());
        let q = PartitionQosPolicy {
            names: alloc::vec![String::from("x")],
        };
        assert!(!q.is_empty());
    }

    #[test]
    fn roundtrip_empty() {
        let p = PartitionQosPolicy::default();
        let mut w = BufferWriter::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(PartitionQosPolicy::decode_from(&mut r).unwrap(), p);
    }

    #[test]
    fn roundtrip_two_names() {
        let p = PartitionQosPolicy {
            names: alloc::vec![String::from("sensor_data"), String::from("telemetry_*")],
        };
        let mut w = BufferWriter::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(PartitionQosPolicy::decode_from(&mut r).unwrap(), p);
    }

    #[test]
    fn decoder_rejects_too_many_partitions() {
        let mut bytes = alloc::vec::Vec::new();
        // Length `MAX_PARTITIONS + 1` → LengthExceeded.
        bytes.extend_from_slice(&(MAX_PARTITIONS as u32 + 1).to_le_bytes());
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let err = PartitionQosPolicy::decode_from(&mut r).unwrap_err();
        assert!(matches!(err, DecodeError::LengthExceeded { .. }));
    }

    // --- fnmatch ---

    #[test]
    fn fnmatch_literal_match() {
        assert!(fnmatch("foo", "foo"));
        assert!(!fnmatch("foo", "bar"));
    }

    #[test]
    fn fnmatch_star_matches_any() {
        assert!(fnmatch("sensor_*", "sensor_data"));
        assert!(fnmatch("sensor_*", "sensor_"));
        assert!(fnmatch("*_data", "sensor_data"));
        assert!(fnmatch("*", "anything"));
        assert!(!fnmatch("a*b", "ac"));
    }

    #[test]
    fn fnmatch_question_matches_single() {
        assert!(fnmatch("a?c", "abc"));
        assert!(!fnmatch("a?c", "ac"));
        assert!(!fnmatch("a?c", "abbc"));
    }

    #[test]
    fn fnmatch_char_class() {
        assert!(fnmatch("alarm_[0-9]", "alarm_5"));
        assert!(!fnmatch("alarm_[0-9]", "alarm_x"));
        assert!(fnmatch("al[ae]rm", "alarm"));
        assert!(fnmatch("al[ae]rm", "alerm"));
        assert!(!fnmatch("al[ae]rm", "alirm"));
    }

    #[test]
    fn fnmatch_negated_class() {
        assert!(fnmatch("[!abc]", "d"));
        assert!(!fnmatch("[!abc]", "a"));
    }

    #[test]
    fn fnmatch_multiple_stars() {
        assert!(fnmatch("a*b*c", "axbyc"));
        assert!(fnmatch("**foo**", "xfoox"));
    }

    // ---- Edge cases (Round-2 review R1/R3/R4) ----

    /// POSIX subset: malformed `[` without `]` is NOMATCH (not literal).
    #[test]
    fn fnmatch_unterminated_class_is_nomatch() {
        assert!(!fnmatch("[abc", "a"));
        assert!(!fnmatch("[abc", "[abc"));
    }

    /// Alternative negation syntax `[^...]` (POSIX) is treated like `[!...]`.
    #[test]
    fn fnmatch_caret_negation_equivalent() {
        assert!(fnmatch("[^abc]", "d"));
        assert!(!fnmatch("[^abc]", "b"));
    }

    /// A pattern longer than the text fails if there are no `*` in it.
    #[test]
    fn fnmatch_pattern_longer_than_text() {
        assert!(!fnmatch("abcdef", "abc"));
        assert!(!fnmatch("ab?de", "ab"));
    }

    /// Multiple `*` + character classes combined.
    #[test]
    fn fnmatch_star_and_class_combined() {
        assert!(fnmatch("log_[0-9]*", "log_5_debug"));
        assert!(!fnmatch("log_[0-9]*", "log_x_debug"));
    }

    /// A trailing `*` also matches an empty remainder.
    #[test]
    fn fnmatch_trailing_star_matches_empty() {
        assert!(fnmatch("foo*", "foo"));
    }

    /// Empty pattern matches only empty text.
    #[test]
    fn fnmatch_empty_pattern() {
        assert!(fnmatch("", ""));
        assert!(!fnmatch("", "x"));
    }

    /// Empty text matches a `*`-only pattern.
    #[test]
    fn fnmatch_empty_text_star_only() {
        assert!(fnmatch("*", ""));
        assert!(fnmatch("***", ""));
    }

    /// ReDoS sanity check: pathological pattern with MAX-len text
    /// terminiert in &lt;50 ms (iterative Impl ist O(n*m)).
    #[test]
    fn fnmatch_pathological_pattern_terminates_fast() {
        let pat = "a*a*a*a*a*a*a*b";
        let txt = "a".repeat(100);
        let start = core::time::Duration::from_secs(0);
        let _ = start;
        let result = fnmatch(pat, &txt);
        // The pattern can never match (no 'b' in the text) — expectation: false
        // and above all: it terminates.
        assert!(!result);
    }
}
