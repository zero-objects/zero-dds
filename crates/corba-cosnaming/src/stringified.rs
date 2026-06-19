// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Stringified-Name — Spec §2.5.4.20 (`to_string` / `to_name`).
//!
//! ```text
//! to_string("MyApp/Trader" with `id` = "MyApp", "Trader" + empty kind)
//!   -> "MyApp/Trader"
//! to_string with kind: id="x" + kind="Server" -> "x.Server"
//! Escaping: `\` escapes `/`, `.`, `\` self.
//! ```
//!
//! Spec normative:
//! * `/` separator between NameComponents.
//! * `.` separator between `id` and `kind`. If `kind` is empty, no `.`
//!   in the output.
//! * For an empty `id` and a non-empty `kind`: `.kind`.
//! * Backslash escape for special characters (`/`, `.`, `\`).

use alloc::string::String;

use crate::error::NamingError;
use crate::name::{Name, NameComponent};

/// Spec §2.5.4.20 `to_string`.
///
/// # Errors
/// `InvalidName` when `name` is empty.
pub fn name_to_string(name: &Name) -> Result<String, NamingError> {
    if name.is_empty() {
        return Err(NamingError::InvalidName);
    }
    let mut out = String::new();
    for (i, nc) in name.iter().enumerate() {
        if i > 0 {
            out.push('/');
        }
        if !nc.id.is_empty() {
            push_escaped(&mut out, &nc.id);
        }
        if !nc.kind.is_empty() {
            out.push('.');
            push_escaped(&mut out, &nc.kind);
        }
    }
    Ok(out)
}

/// Spec §2.5.4.20 `to_name`.
///
/// # Errors
/// `InvalidName` when `s` is empty or a component is empty.
pub fn string_to_name(s: &str) -> Result<Name, NamingError> {
    if s.is_empty() {
        return Err(NamingError::InvalidName);
    }
    // The outer loop splits at unescaped '/'. Escape sequences are
    // passed through as two chars (`\X`) to the inner component
    // (`parse_component`) — it owns `.` splitting and escape
    // decoding.
    let mut out = Name::new();
    let mut current = String::new();
    let mut chars = s.chars().peekable();
    let mut bytes_in_current = false;
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                let next = chars.next().ok_or(NamingError::InvalidName)?;
                current.push('\\');
                current.push(next);
                bytes_in_current = true;
            }
            '/' => {
                if !bytes_in_current {
                    return Err(NamingError::InvalidName);
                }
                out.push(parse_component(&current)?);
                current.clear();
                bytes_in_current = false;
            }
            other => {
                current.push(other);
                bytes_in_current = true;
            }
        }
    }
    if !bytes_in_current {
        return Err(NamingError::InvalidName);
    }
    out.push(parse_component(&current)?);
    Ok(out)
}

fn parse_component(s: &str) -> Result<NameComponent, NamingError> {
    // `.` split (escaped via backslash). We traverse manually.
    let mut id = String::new();
    let mut kind = String::new();
    let mut in_kind = false;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            let next = chars.next().ok_or(NamingError::InvalidName)?;
            if in_kind {
                kind.push(next);
            } else {
                id.push(next);
            }
        } else if c == '.' && !in_kind {
            in_kind = true;
        } else if in_kind {
            kind.push(c);
        } else {
            id.push(c);
        }
    }
    Ok(NameComponent { id, kind })
}

fn push_escaped(out: &mut String, s: &str) {
    for c in s.chars() {
        if matches!(c, '/' | '.' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_simple() {
        let n: Name = alloc::vec![NameComponent::new("MyApp"), NameComponent::new("Trader")];
        let s = name_to_string(&n).unwrap();
        assert_eq!(s, "MyApp/Trader");
        assert_eq!(string_to_name(&s).unwrap(), n);
    }

    #[test]
    fn round_trip_with_kind() {
        let n: Name = alloc::vec![NameComponent::with_kind("Trader", "Server")];
        let s = name_to_string(&n).unwrap();
        assert_eq!(s, "Trader.Server");
        assert_eq!(string_to_name(&s).unwrap(), n);
    }

    #[test]
    fn empty_id_with_kind_emits_leading_dot() {
        let n: Name = alloc::vec![NameComponent::with_kind("", "OnlyKind")];
        let s = name_to_string(&n).unwrap();
        assert_eq!(s, ".OnlyKind");
        assert_eq!(string_to_name(&s).unwrap(), n);
    }

    #[test]
    fn slash_in_id_is_escaped() {
        let n: Name = alloc::vec![NameComponent::new("a/b")];
        let s = name_to_string(&n).unwrap();
        assert_eq!(s, "a\\/b");
        assert_eq!(string_to_name(&s).unwrap(), n);
    }

    #[test]
    fn dot_in_id_is_escaped() {
        let n: Name = alloc::vec![NameComponent::new("file.cfg")];
        let s = name_to_string(&n).unwrap();
        assert_eq!(s, "file\\.cfg");
        assert_eq!(string_to_name(&s).unwrap(), n);
    }

    #[test]
    fn backslash_self_is_escaped() {
        let n: Name = alloc::vec![NameComponent::new("a\\b")];
        let s = name_to_string(&n).unwrap();
        assert_eq!(s, "a\\\\b");
        assert_eq!(string_to_name(&s).unwrap(), n);
    }

    #[test]
    fn empty_input_is_invalid() {
        assert!(matches!(string_to_name(""), Err(NamingError::InvalidName)));
        assert!(matches!(
            name_to_string(&Name::new()),
            Err(NamingError::InvalidName)
        ));
    }

    #[test]
    fn empty_component_after_slash_is_invalid() {
        assert!(matches!(
            string_to_name("a/"),
            Err(NamingError::InvalidName)
        ));
        assert!(matches!(
            string_to_name("/a"),
            Err(NamingError::InvalidName)
        ));
    }

    #[test]
    fn dangling_backslash_is_invalid() {
        assert!(matches!(
            string_to_name("abc\\"),
            Err(NamingError::InvalidName)
        ));
    }
}
