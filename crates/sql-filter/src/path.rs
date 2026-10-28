// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The steps of a field reference.
//!
//! [`RowAccess::get`](crate::RowAccess::get) receives the reference as
//! the parser read it: names joined by `.`, each optionally followed by
//! subscripts, e.g. `a.b[3].c['key']` (DDS-XTypes 1.3 §7.6.7.1).
//! [`split_path`] takes it apart.

use alloc::string::String;
use alloc::vec::Vec;

/// One step of a field reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathStep {
    /// A member name.
    Member(String),
    /// `[n]`: the zero-based element of an array or a sequence.
    Index(u32),
    /// `['key']`: the value of a map for that key, or the flag of a
    /// bitmask of that name. The key is a string even when the map's key
    /// type is an integer type (§7.6.7.1).
    Key(String),
}

/// Splits a field reference as the parser produces it into its steps.
///
/// `None` when the text is not such a reference.
#[must_use]
pub fn split_path(path: &str) -> Option<Vec<PathStep>> {
    let bytes = path.as_bytes();
    let mut steps = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'.' => i += 1,
            b'[' => {
                i += 1;
                if bytes.get(i) == Some(&b'\'') {
                    i += 1;
                    let mut key = String::new();
                    loop {
                        match bytes.get(i)? {
                            b'\'' if bytes.get(i + 1) == Some(&b'\'') => {
                                key.push('\'');
                                i += 2;
                            }
                            b'\'' => {
                                i += 1;
                                break;
                            }
                            _ => {
                                let start = i;
                                while i < bytes.len() && bytes[i] != b'\'' {
                                    i += 1;
                                }
                                key.push_str(&path[start..i]);
                            }
                        }
                    }
                    steps.push(PathStep::Key(key));
                } else {
                    let start = i;
                    while bytes.get(i).is_some_and(u8::is_ascii_digit) {
                        i += 1;
                    }
                    steps.push(PathStep::Index(path[start..i].parse().ok()?));
                }
                if bytes.get(i) != Some(&b']') {
                    return None;
                }
                i += 1;
            }
            _ => {
                let start = i;
                while i < bytes.len() && bytes[i] != b'.' && bytes[i] != b'[' {
                    i += 1;
                }
                steps.push(PathStep::Member(path[start..i].into()));
            }
        }
    }
    Some(steps)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_dotted_name_is_members() {
        assert_eq!(
            split_path("a.b.c").unwrap(),
            [
                PathStep::Member("a".into()),
                PathStep::Member("b".into()),
                PathStep::Member("c".into())
            ]
        );
    }

    #[test]
    fn subscripts_are_indices_and_keys() {
        assert_eq!(
            split_path("a.b[3].c['it''s']").unwrap(),
            [
                PathStep::Member("a".into()),
                PathStep::Member("b".into()),
                PathStep::Index(3),
                PathStep::Member("c".into()),
                PathStep::Key("it's".into()),
            ]
        );
    }

    #[test]
    fn a_broken_reference_is_none() {
        assert_eq!(split_path("a[x]"), None);
        assert_eq!(split_path("a['k"), None);
    }
}
