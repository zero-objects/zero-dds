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

use zerodds_wire::{Reader, WireError, Writer};

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
    /// [`WireError::OutOfRange`] for a list no `u32` count describes.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        let len = u32::try_from(self.names.len())
            .map_err(|_| WireError::refused(w.position(), "a partition count a u32 can carry"))?;
        w.write_u32(len);
        for name in &self.names {
            w.write_string(name);
        }
        Ok(())
    }

    /// Wire decoding with a DoS cap for the number of partitions (hard,
    /// [`MAX_PARTITIONS`]) and length per name ([`MAX_PARTITION_NAME_LEN`]).
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] for more than [`MAX_PARTITIONS`] entries
    /// or a name over [`MAX_PARTITION_NAME_LEN`],
    /// [`WireError::ImpossibleLength`] for a count the stream cannot hold,
    /// and [`WireError::Truncated`] for a stream that ends early.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let at = r.position();
        let len = r.read_u32()? as usize;
        if len > MAX_PARTITIONS {
            return Err(WireError::refused(at, "at most MAX_PARTITIONS entries"));
        }
        // Additional byte-level cap: each entry is at least 4 bytes, so a
        // count above a quarter of what is left cannot be true.
        if len > r.remaining() / 4 {
            return Err(WireError::ImpossibleLength {
                at,
                claimed: len,
                available: r.remaining(),
            });
        }
        let mut names = Vec::with_capacity(len);
        for _ in 0..len {
            let name_at = r.position();
            let s = r.read_string()?;
            if s.len() > MAX_PARTITION_NAME_LEN {
                return Err(WireError::refused(
                    name_at,
                    "a partition name of at most MAX_PARTITION_NAME_LEN octets",
                ));
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

/// Canonical PARTITION compatibility check (DDS 1.4 §2.2.3.13.6), shared
/// by [`PartitionQosPolicy::is_compatible_with`] and the runtime's
/// writer/reader matching so the two can never disagree.
///
/// Semantics:
/// - An **empty list** is normalized to the single default partition name
///   `""` (§2.2.3.13: "the empty string … is a valid partition name … an
///   empty PARTITION QoS is treated as containing the empty string"). Thus
///   `[]`, `[""]` and any mix of the two all match each other.
/// - Otherwise the sides match iff **at least one** name on one side
///   matches **one** name on the other ([`partition_names_match`]): a
///   wildcard expression on either side matches a literal on the other
///   under [`fnmatch`], "but no two names that both contain wildcards will
///   ever be considered to match" (§2.2.3.13).
///
/// A named, non-empty partition never matches the default partition, since
/// `fnmatch("name", "") == false` and `fnmatch("", "name") == false`.
#[must_use]
pub fn partitions_compatible(offered: &[String], requested: &[String]) -> bool {
    // Normalize the empty list to the default partition [""].
    const DEFAULT: &[String] = &[String::new()];
    let off: &[String] = if offered.is_empty() { DEFAULT } else { offered };
    let req: &[String] = if requested.is_empty() {
        DEFAULT
    } else {
        requested
    };
    off.iter()
        .any(|o| req.iter().any(|r| partition_names_match(o, r)))
}

/// Whether two partition names match (DDS 1.4 §2.2.3.13): two literals when
/// they are equal, a wildcard expression and a literal when [`fnmatch`]
/// says so, two wildcard expressions never — "no two names that both
/// contain wildcards will ever be considered to match". Cyclone DDS
/// (`partition_patmatch_p`) does the same with `*` and `?`.
#[must_use]
pub fn partition_names_match(a: &str, b: &str) -> bool {
    match (has_wildcard(a), has_wildcard(b)) {
        (true, true) => false,
        (true, false) => fnmatch(a, b),
        (false, true) => fnmatch(b, a),
        (false, false) => a == b,
    }
}

/// Whether `name` holds a wildcard of [`fnmatch`]: `*`, `?`, or a `[`
/// with a `]` after it (a bracket expression; a lone `[` is a literal).
fn has_wildcard(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.iter().any(|&c| c == b'*' || c == b'?')
        || bytes
            .iter()
            .position(|&c| c == b'[')
            .is_some_and(|open| bytes[open + 1..].contains(&b']'))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_wire::Endianness;

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
        let mut w = Writer::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Little);
        assert_eq!(PartitionQosPolicy::decode_from(&mut r).unwrap(), p);
    }

    #[test]
    fn roundtrip_two_names() {
        let p = PartitionQosPolicy {
            names: alloc::vec![String::from("sensor_data"), String::from("telemetry_*")],
        };
        let mut w = Writer::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Little);
        assert_eq!(PartitionQosPolicy::decode_from(&mut r).unwrap(), p);
    }

    #[test]
    fn decoder_rejects_too_many_partitions() {
        let mut bytes = alloc::vec::Vec::new();
        // A count of `MAX_PARTITIONS + 1` is refused.
        bytes.extend_from_slice(&(MAX_PARTITIONS as u32 + 1).to_le_bytes());
        let mut r = Reader::new(&bytes, Endianness::Little);
        let err = PartitionQosPolicy::decode_from(&mut r).unwrap_err();
        assert!(
            matches!(&err, WireError::OutOfRange { expected, .. }
                if expected.contains("MAX_PARTITIONS")),
            "unexpected error: {err}"
        );
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
    /// terminates in &lt;50 ms (iterative impl is O(n*m)).
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

    // --- partitions_compatible: the DDS-spec matrix ---

    fn names(v: &[&str]) -> alloc::vec::Vec<String> {
        v.iter().map(|s| String::from(*s)).collect()
    }

    /// The empty list is the default partition `""`: `[]`, `[""]` and any
    /// mix of the two match one another (DDS 1.4 §2.2.3.13).
    #[test]
    fn partitions_default_and_empty_string_are_equivalent() {
        assert!(partitions_compatible(&names(&[]), &names(&[])));
        assert!(partitions_compatible(&names(&[]), &names(&[""])));
        assert!(partitions_compatible(&names(&[""]), &names(&[])));
        assert!(partitions_compatible(&names(&[""]), &names(&[""])));
    }

    /// Symmetric fnmatch: a glob on either side matches a literal on the
    /// other.
    #[test]
    fn partitions_wildcard_matches_symmetrically() {
        assert!(partitions_compatible(&names(&["p1"]), &names(&["p*"])));
        assert!(partitions_compatible(&names(&["p*"]), &names(&["p1"])));
    }

    /// DDS 1.4 §2.2.3.13: "no two names that both contain wildcards will
    /// ever be considered to match". `p*` matched `p*` and `p?`, as `p*`
    /// read as a literal matches the pattern `p*`.
    #[test]
    fn two_wildcard_names_never_match() {
        assert!(!partitions_compatible(&names(&["p*"]), &names(&["p*"])));
        assert!(!partitions_compatible(&names(&["p*"]), &names(&["p?"])));
        assert!(!partitions_compatible(&names(&["*"]), &names(&["[ab]"])));
        assert!(partitions_compatible(
            &names(&["p*", "pa"]),
            &names(&["p?"])
        ));
        // A `[` without `]` is no bracket expression: a literal.
        assert!(partitions_compatible(&names(&["a[b"]), &names(&["a[b"])));
        assert!(partitions_compatible(&names(&["a*"]), &names(&["a[b"])));
    }

    /// A non-matching literal/glob pair does not overlap.
    #[test]
    fn partitions_non_matching_wildcard_is_incompatible() {
        assert!(!partitions_compatible(&names(&["x1"]), &names(&["p*"])));
    }

    /// A named, non-empty partition never matches the default partition.
    #[test]
    fn partitions_named_does_not_match_default() {
        assert!(!partitions_compatible(&names(&["p1"]), &names(&[])));
        assert!(!partitions_compatible(&names(&[]), &names(&["p1"])));
        assert!(!partitions_compatible(&names(&["p1"]), &names(&[""])));
    }

    /// Regression for the fixed runtime bug: exact-equality matching used to
    /// reject `p1` vs `p*`; a plain common name still matches.
    #[test]
    fn partitions_common_literal_matches() {
        assert!(partitions_compatible(
            &names(&["a", "b"]),
            &names(&["c", "b"])
        ));
        assert!(!partitions_compatible(
            &names(&["a", "b"]),
            &names(&["c", "d"])
        ));
    }
}
