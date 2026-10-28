// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Name matching after POSIX `fnmatch()` (POSIX 1003.2-1992 §B.6), which
//! DDS-Security 1.2 prescribes for topic and partition expressions
//! (§10.4.1.5.3.4.1.3, §10.4.1.5.3.4.1.4, §10.4.1.2.6.1), domain tag
//! expressions (§10.4.1.2.5.1) and subject name expressions
//! (§10.4.1.5.3.2.1).
//!
//! * `*` — zero or more characters.
//! * `?` — exactly one character.
//! * `[...]` — one character of the set: single characters and ranges
//!   `a-z`, negated with a leading `!`; a `]` first in the set is a member.
//!   A `[` without a closing `]` is an ordinary character.
//! * `\c` — the character `c`, unless [`FnmatchFlags::noescape`].
//!
//! With [`FnmatchFlags::pathname`] no wildcard and no set matches `/`.
//! Only `*` and `?` were supported: `<topic>Sq[ua]are</topic>` matched no
//! topic, and a backslash was an ordinary character.

use alloc::vec::Vec;

/// The flags of `fnmatch()` that DDS-Security uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FnmatchFlags {
    /// `FNM_PATHNAME`: `/` is matched only by a `/` in the pattern.
    pub pathname: bool,
    /// `FNM_NOESCAPE`: a backslash is an ordinary character.
    pub noescape: bool,
}

impl FnmatchFlags {
    /// The flags of §10.4.1.5.3.2.1 for subject name expressions:
    /// `FNM_PATHNAME=TRUE`, `FNM_PERIOD=FALSE`, `FNM_NOESCAPE=TRUE`.
    pub const SUBJECT_NAME: Self = Self {
        pathname: true,
        noescape: true,
    };
}

/// `fnmatch(pattern, name, flags) == 0`.
#[must_use]
pub fn fnmatch(pattern: &str, name: &str, flags: FnmatchFlags) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let s: Vec<char> = name.chars().collect();
    let (mut pi, mut si) = (0usize, 0usize);
    // The pattern position after the last `*` and the name position it
    // stands for, to resume from on a mismatch.
    let mut star: Option<(usize, usize)> = None;
    loop {
        if pi < p.len() {
            match p[pi] {
                '*' => {
                    pi += 1;
                    star = Some((pi, si));
                    continue;
                }
                '?' => {
                    if si < s.len() && !(flags.pathname && s[si] == '/') {
                        pi += 1;
                        si += 1;
                        continue;
                    }
                }
                '[' => match bracket(&p, pi, s.get(si).copied(), flags) {
                    Some((true, next)) => {
                        pi = next;
                        si += 1;
                        continue;
                    }
                    Some((false, _)) => {}
                    None => {
                        if si < s.len() && s[si] == '[' {
                            pi += 1;
                            si += 1;
                            continue;
                        }
                    }
                },
                '\\' if !flags.noescape && pi + 1 < p.len() => {
                    if si < s.len() && s[si] == p[pi + 1] {
                        pi += 2;
                        si += 1;
                        continue;
                    }
                }
                c => {
                    if si < s.len() && s[si] == c {
                        pi += 1;
                        si += 1;
                        continue;
                    }
                }
            }
        } else if si == s.len() {
            return true;
        }
        // Mismatch: let the last `*` take one more character.
        match star {
            Some((sp, ss)) if ss < s.len() && !(flags.pathname && s[ss] == '/') => {
                star = Some((sp, ss + 1));
                pi = sp;
                si = ss + 1;
            }
            _ => return false,
        }
    }
}

/// The set at `p[start] == '['` against `ch`: `Some((matched, next))` with
/// the pattern position after the `]`, or `None` when the set has no `]`.
fn bracket(
    p: &[char],
    start: usize,
    ch: Option<char>,
    flags: FnmatchFlags,
) -> Option<(bool, usize)> {
    let mut j = start + 1;
    let negate = p.get(j) == Some(&'!');
    if negate {
        j += 1;
    }
    let mut first = true;
    let mut hit = false;
    loop {
        let mut c = *p.get(j)?;
        if c == ']' && !first {
            break;
        }
        first = false;
        if c == '\\' && !flags.noescape {
            j += 1;
            c = *p.get(j)?;
        }
        if p.get(j + 1) == Some(&'-') && p.get(j + 2).is_some_and(|&n| n != ']') {
            let mut hi = p[j + 2];
            let mut end = j + 2;
            if hi == '\\' && !flags.noescape {
                end += 1;
                hi = *p.get(end)?;
            }
            if ch.is_some_and(|x| c <= x && x <= hi) {
                hit = true;
            }
            j = end + 1;
        } else {
            if ch == Some(c) {
                hit = true;
            }
            j += 1;
        }
    }
    let Some(x) = ch else {
        return Some((false, j + 1));
    };
    let matched = (hit != negate) && !(flags.pathname && x == '/');
    Some((matched, j + 1))
}

/// Topic, partition and domain tag expressions: `fnmatch()` with no flags.
#[must_use]
pub fn topic_match(pattern: &str, name: &str) -> bool {
    fnmatch(pattern, name, FnmatchFlags::default())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn exact_match() {
        assert!(topic_match("Chatter", "Chatter"));
        assert!(!topic_match("Chatter", "ChatterX"));
    }

    #[test]
    fn star_matches_all() {
        assert!(topic_match("*", "anything"));
        assert!(topic_match("*", ""));
    }

    #[test]
    fn prefix_wildcard() {
        assert!(topic_match("sensor_*", "sensor_temp"));
        assert!(topic_match("sensor_*", "sensor_"));
        assert!(!topic_match("sensor_*", "actuator_x"));
    }

    #[test]
    fn question_mark_exact_one() {
        assert!(topic_match("s?nsor", "sensor"));
        assert!(topic_match("s?nsor", "sansor"));
        assert!(!topic_match("s?nsor", "snsor"));
    }

    #[test]
    fn combined_wildcards() {
        assert!(topic_match("*_?", "hello_1"));
        assert!(!topic_match("*_?", "hello_"));
        assert!(topic_match("a*b*c", "axxbyyc"));
        assert!(!topic_match("a*b*c", "axxbyy"));
    }

    /// POSIX `fnmatch()` sets, ranges and negation (§B.6), which the
    /// matcher did not have.
    #[test]
    fn sets_ranges_and_negation_match_one_character() {
        assert!(topic_match("Sq[ua]are", "Square"));
        assert!(topic_match("Sq[ua]are", "Sqaare"));
        assert!(!topic_match("Sq[ua]are", "Sqxare"));
        assert!(topic_match("T[0-9]", "T7"));
        assert!(!topic_match("T[0-9]", "Tx"));
        assert!(topic_match("T[!0-9]", "Tx"));
        assert!(!topic_match("T[!0-9]", "T7"));
        assert!(topic_match("[]x]", "]"));
        assert!(topic_match("a[", "a["), "a [ without ] is a character");
    }

    /// A backslash quotes the next character; with `noescape` it is one.
    #[test]
    fn a_backslash_quotes_unless_noescape() {
        assert!(topic_match(r"a\*", "a*"));
        assert!(!topic_match(r"a\*", "ab"));
        let noescape = FnmatchFlags {
            noescape: true,
            ..FnmatchFlags::default()
        };
        assert!(fnmatch(r"a\b", r"a\b", noescape));
    }

    /// `FNM_PATHNAME` (§10.4.1.5.3.2.1): no wildcard matches `/`.
    #[test]
    fn pathname_keeps_slashes_out_of_wildcards() {
        let f = FnmatchFlags::SUBJECT_NAME;
        assert!(fnmatch("AGV/*", "AGV/agv1", f));
        assert!(!fnmatch("AGV*", "AGV/agv1", f));
        assert!(!fnmatch("AGV?agv1", "AGV/agv1", f));
        assert!(topic_match("AGV*", "AGV/agv1"), "without the flag it does");
    }
}
