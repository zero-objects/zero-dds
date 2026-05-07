// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Topic-Name-Matching mit Wildcards — Spec-konform zu OMG POSIX-Glob.
//!
//! Unterstuetzte Wildcards:
//! * `*` — null oder mehr beliebige Zeichen.
//! * `?` — genau ein Zeichen.
//!
//! Zweck: ein `<topic>*</topic>` im Permissions-XML matcht alle
//! Topics; `<topic>sensor_*</topic>` matcht `sensor_temp`, `sensor_pressure`.

/// Glob-Match. Rein iterativ/DP, kein Regex-Engine-Overhead.
#[must_use]
pub fn topic_match(pattern: &str, name: &str) -> bool {
    let p: alloc::vec::Vec<char> = pattern.chars().collect();
    let n: alloc::vec::Vec<char> = name.chars().collect();
    let (m, k) = (n.len(), p.len());
    // dp[i][j] = name[..i] matcht pattern[..j].
    let mut dp = alloc::vec![alloc::vec![false; k + 1]; m + 1];
    dp[0][0] = true;
    for j in 1..=k {
        if p[j - 1] == '*' {
            dp[0][j] = dp[0][j - 1];
        }
    }
    for i in 1..=m {
        for j in 1..=k {
            let pc = p[j - 1];
            dp[i][j] = if pc == '*' {
                dp[i - 1][j] || dp[i][j - 1]
            } else if pc == '?' || pc == n[i - 1] {
                dp[i - 1][j - 1]
            } else {
                false
            };
        }
    }
    dp[m][k]
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
    }
}
