// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MQTT Topic Filter Matching — Spec §4.7.
//!
//! Spec §4.7.1: topics are structured by `/`-separated levels.
//! Wildcards:
//! * `+` — one level (single-level wildcard).
//! * `#` — multi-level (only allowed at the end).
//! * `$` prefix — system topics, not matchable by `#`/`+` at the
//!   wildcard position (Spec §4.7.2).

/// Validation error for topic filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopicFilterError {
    /// `#` is not at the end.
    HashNotAtEnd,
    /// `+`/`#` is not alone in its level.
    WildcardMixedWithChars,
    /// Filter is empty.
    Empty,
}

impl core::fmt::Display for TopicFilterError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::HashNotAtEnd => f.write_str("`#` must be the last level"),
            Self::WildcardMixedWithChars => {
                f.write_str("wildcard `+`/`#` must occupy a level alone")
            }
            Self::Empty => f.write_str("empty topic filter"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TopicFilterError {}

/// Validates a topic filter per Spec §4.7.1.
///
/// # Errors
/// See [`TopicFilterError`].
pub fn validate_filter(filter: &str) -> Result<(), TopicFilterError> {
    if filter.is_empty() {
        return Err(TopicFilterError::Empty);
    }
    let levels: alloc::vec::Vec<&str> = filter.split('/').collect();
    let last = levels.len() - 1;
    for (i, level) in levels.iter().enumerate() {
        if *level == "#" {
            if i != last {
                return Err(TopicFilterError::HashNotAtEnd);
            }
        } else if *level == "+" {
            // OK
        } else if level.contains('#') || level.contains('+') {
            return Err(TopicFilterError::WildcardMixedWithChars);
        }
    }
    Ok(())
}

/// Validates a topic name (publish side, Spec §4.7.1.2).
/// Topic names have NO wildcards.
///
/// # Errors
/// `WildcardMixedWithChars` if `+` or `#` is contained;
/// `Empty` if empty.
pub fn validate_topic_name(topic: &str) -> Result<(), TopicFilterError> {
    if topic.is_empty() {
        return Err(TopicFilterError::Empty);
    }
    if topic.contains('+') || topic.contains('#') {
        return Err(TopicFilterError::WildcardMixedWithChars);
    }
    Ok(())
}

/// Spec §4.7 — checks whether `topic` (a topic name without wildcards)
/// matches `filter`.
///
/// `$` prefix on `topic`: wildcards `+`/`#` do not match the
/// first level if it has a `$` prefix (Spec §4.7.2).
#[must_use]
pub fn matches(filter: &str, topic: &str) -> bool {
    let f_levels: alloc::vec::Vec<&str> = filter.split('/').collect();
    let t_levels: alloc::vec::Vec<&str> = topic.split('/').collect();
    let dollar_topic = topic.starts_with('$');

    let mut fi = 0;
    let mut ti = 0;
    while fi < f_levels.len() {
        let fl = f_levels[fi];
        if fl == "#" {
            // Multi-level — matches everything from here on.
            // Spec §4.7.2: `$` topics do not match across a wildcard
            // when the wildcard reaches the `$` level.
            if dollar_topic && fi == 0 {
                return false;
            }
            return true;
        }
        if ti >= t_levels.len() {
            return false;
        }
        let tl = t_levels[ti];
        if fl == "+" {
            if dollar_topic && fi == 0 && ti == 0 {
                return false;
            }
        } else if fl != tl {
            return false;
        }
        fi += 1;
        ti += 1;
    }
    ti == t_levels.len()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn validate_simple_filter() {
        validate_filter("a/b/c").unwrap();
    }

    #[test]
    fn validate_plus_in_middle() {
        validate_filter("a/+/c").unwrap();
    }

    #[test]
    fn validate_hash_at_end() {
        validate_filter("a/b/#").unwrap();
    }

    #[test]
    fn validate_only_hash() {
        validate_filter("#").unwrap();
    }

    #[test]
    fn validate_hash_in_middle_rejected() {
        assert_eq!(
            validate_filter("a/#/c"),
            Err(TopicFilterError::HashNotAtEnd)
        );
    }

    #[test]
    fn validate_mixed_wildcard_rejected() {
        assert_eq!(
            validate_filter("a/foo+/c"),
            Err(TopicFilterError::WildcardMixedWithChars)
        );
    }

    #[test]
    fn validate_empty_rejected() {
        assert_eq!(validate_filter(""), Err(TopicFilterError::Empty));
    }

    #[test]
    fn topic_name_with_wildcard_rejected() {
        assert!(validate_topic_name("a/+/b").is_err());
        assert!(validate_topic_name("a/#").is_err());
    }

    #[test]
    fn match_exact_topic() {
        assert!(matches("a/b", "a/b"));
        assert!(!matches("a/b", "a/c"));
    }

    #[test]
    fn match_plus_wildcard() {
        assert!(matches("a/+/c", "a/b/c"));
        assert!(matches("a/+/c", "a/X/c"));
        assert!(!matches("a/+/c", "a/b/c/d"));
        assert!(!matches("a/+/c", "a/c"));
    }

    #[test]
    fn match_hash_wildcard() {
        assert!(matches("a/#", "a/b"));
        assert!(matches("a/#", "a/b/c/d"));
        assert!(matches("a/#", "a"));
        assert!(!matches("a/#", "b"));
    }

    #[test]
    fn match_root_hash_matches_all() {
        assert!(matches("#", "a"));
        assert!(matches("#", "a/b/c"));
    }

    #[test]
    fn dollar_topic_not_matched_by_root_wildcard() {
        // Spec §4.7.2: "#" must NOT match $SYS/...
        assert!(!matches("#", "$SYS/uptime"));
        assert!(!matches("+/uptime", "$SYS/uptime"));
    }

    #[test]
    fn dollar_topic_matched_by_explicit_filter() {
        assert!(matches("$SYS/uptime", "$SYS/uptime"));
        assert!(matches("$SYS/+", "$SYS/uptime"));
    }
}
