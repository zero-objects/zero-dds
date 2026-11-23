// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The character sets a code set encodes, for the compatibility test of
//! code set negotiation — CORBA 3.3 Part 2 §7.10.2.6, §7.10.5.
//!
//! When neither side can convert to the other's native code set and no
//! conversion code set is shared, the negotiation falls back to UTF-8
//! (char) or UTF-16 (wchar) only when the two native code sets are
//! `compatible`: "This test looks at the character sets encoded by the
//! client and server native code sets. If they are different (e.g.,
//! Korean and French), then meaningful communication between the client
//! and server is not possible and a CODESET_INCOMPATIBLE exception with
//! standard minor code 1 is raised." The clause leaves the test to the
//! OSF registry interfaces; "If either the CNCS or SNCS is from the IANA
//! Character Set registry, then the codesets are automatically assumed to
//! be compatible".
//!
//! The table holds the OSF registry entries of the code sets this ORB
//! names. Its rows are those of ACE's `Codeset_Registry_db.cpp`, which
//! ACE generated from the OSF registry text `code_set_registry1.2g.txt`;
//! the OSF text itself is not in `internal/standards/cache`. Two code sets
//! are compatible when they share a character set, the rule of ACE's
//! `ACE_Codeset_Registry::is_compatible_i` (the DCE test the clause
//! points to); a code set without a row is compatible with itself only.

/// One OSF registry row: a code set and the character sets it encodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegistryEntry {
    /// The OSF code set id.
    pub code_set: u32,
    /// The character set ids the code set encodes.
    pub char_sets: &'static [u16],
    /// The largest number of octets one character takes.
    pub max_bytes: u8,
}

/// The OSF registry rows of the code sets this ORB names.
pub const ENTRIES: &[RegistryEntry] = &[
    // "ISO 8859-1:1987; Latin Alphabet No. 1"
    RegistryEntry {
        code_set: 0x0001_0001,
        char_sets: &[0x0011],
        max_bytes: 1,
    },
    // "ISO/IEC 10646-1:1993; UCS-2, Level 1"
    RegistryEntry {
        code_set: 0x0001_0100,
        char_sets: &[0x1000],
        max_bytes: 2,
    },
    // "ISO/IEC 10646-1:1993; UCS-4, Level 1"
    RegistryEntry {
        code_set: 0x0001_0104,
        char_sets: &[0x1000],
        max_bytes: 4,
    },
    // "ISO/IEC 10646-1:1993; UTF-16, UCS Transformation Format 16-bit form"
    RegistryEntry {
        code_set: 0x0001_0109,
        char_sets: &[0x1000],
        max_bytes: 2,
    },
    // "X/Open UTF-8; UCS Transformation Format 8 (UTF-8)"
    RegistryEntry {
        code_set: 0x0501_0001,
        char_sets: &[0x1000],
        max_bytes: 6,
    },
];

/// `true` for an id of the IANA Character Set registry (its MIBenum
/// values). §7.10.2.5: "The OSF Registry and the IANA Registry have
/// non-overlapping ranges"; the OSF ids start at `0x00010001` and the
/// MIBenum values are below `0x10000` — the boundary is inferred from the
/// two registries' values, the clause does not state it.
#[must_use]
pub const fn is_iana(id: u32) -> bool {
    id != 0 && id < 0x0001_0000
}

/// The OSF registry row of `code_set`, if this ORB knows it.
#[must_use]
pub fn entry(code_set: u32) -> Option<&'static RegistryEntry> {
    ENTRIES.iter().find(|e| e.code_set == code_set)
}

/// `compatible(CNCS, SNCS)` of the negotiation algorithm (§7.10.2.6): an
/// IANA id on either side is compatible; two OSF code sets are compatible
/// when they share a character set; a code set is compatible with itself.
/// The null code set (0, "no native code set", §7.10.2.4) is compatible
/// with nothing.
#[must_use]
pub fn compatible(client_native: u32, server_native: u32) -> bool {
    if client_native == 0 || server_native == 0 {
        return false;
    }
    if is_iana(client_native) || is_iana(server_native) || client_native == server_native {
        return true;
    }
    match (entry(client_native), entry(server_native)) {
        (Some(c), Some(s)) => c.char_sets.iter().any(|x| s.char_sets.contains(x)),
        _ => false,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::code_set::well_known::{ISO_8859_1, UCS_2, UTF_8, UTF_16};

    /// §7.10.2.6: the Unicode code sets encode one character set and are
    /// compatible with each other; ISO 8859-1 encodes another (OSF
    /// registry rows as ACE carries them).
    #[test]
    fn corba_p2_7_10_2_6_unicode_code_sets_are_compatible_latin_1_is_not() {
        assert!(compatible(UTF_8, UTF_16));
        assert!(compatible(UCS_2, UTF_8));
        assert!(compatible(ISO_8859_1, ISO_8859_1));
        assert!(!compatible(ISO_8859_1, UTF_8));
        assert!(!compatible(UTF_16, ISO_8859_1));
        // A code set without a row is compatible with itself only.
        assert!(compatible(0x0003_0010, 0x0003_0010));
        assert!(!compatible(0x0003_0010, UTF_8));
        // The null code set is compatible with nothing.
        assert!(!compatible(0, UTF_8));
        assert!(!compatible(0, 0));
    }

    /// §7.10.2.6: "If either the CNCS or SNCS is from the IANA Character
    /// Set registry, then the codesets are automatically assumed to be
    /// compatible". 4 is ISO-8859-1, 106 UTF-8 in the IANA registry.
    #[test]
    fn corba_p2_7_10_2_6_iana_ids_are_always_compatible() {
        assert!(is_iana(4) && is_iana(106));
        assert!(!is_iana(0) && !is_iana(ISO_8859_1) && !is_iana(UTF_8));
        assert!(compatible(4, 0x0003_0010));
        assert!(compatible(UTF_16, 106));
    }
}
