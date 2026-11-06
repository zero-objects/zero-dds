// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Which wire encoding a received sample is in.
//!
//! A reader is told two things about a sample: the XCDR version and the byte
//! order. It is not told the framing — whether the payload is plain, carries
//! a delimiter, or is a parameter list — because that follows from the type's
//! extensibility, and both peers know the type. XTypes 1.3 §7.4 Table 17:
//!
//! | extensibility | XCDR1      | XCDR2           |
//! |---------------|------------|-----------------|
//! | `@final`      | PLAIN_CDR1 | PLAIN_CDR2      |
//! | `@appendable` | PLAIN_CDR1 | DELIMITED_CDR   |
//! | `@mutable`    | PL_CDR1    | PL_CDR2         |
//!
//! `@appendable` under XCDR1 is PLAIN_CDR1 and not a framing of its own:
//! XCDR1 has no delimiter, which is why appending a member to an
//! `@appendable` type is only safe under XCDR2.

use zerodds_ast_layout::key::Encoding;
use zerodds_types::dynamic::descriptor::ExtensibilityKind;
use zerodds_types::dynamic::type_::DynamicType;

/// The encoding a sample of `ty` is in, given the XCDR version the reader was
/// told.
///
/// A type with no extensibility of its own — a primitive or a collection as a
/// whole topic — is treated as `@final`, which is what an unframed value is.
#[must_use]
pub fn encoding_of(ty: &DynamicType, xcdr2: bool) -> Encoding {
    match (ty.extensibility(), xcdr2) {
        (Some(ExtensibilityKind::Mutable), false) => Encoding::PlCdr1,
        (Some(ExtensibilityKind::Mutable), true) => Encoding::PlCdr2,
        (Some(ExtensibilityKind::Appendable), true) => Encoding::DelimitedCdr,
        (_, true) => Encoding::PlainCdr2,
        (_, false) => Encoding::PlainCdr1,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use crate::type_source::TypeBook;

    // `demo` and not `m`: IDL compares identifiers without regard to
    // case (§7.2.3), so `struct M` inside `module m` would redeclare the
    // scope's own name in its immediate scope, which IDL 4.2 §7.5.2 does
    // not admit. The struct names are the point here — F, A, M for the
    // three extensibilities — so the module's is the one that moves.
    const IDL: &str = r"
        module demo {
            @final struct F { long a; };
            @appendable struct A { long v; };
            @mutable struct M { long a; };
            struct D { long a; };
        };
    ";

    /// DDS-XTypes 1.3 §7.4.1.1 and §7.4.2: under XCDR1 a final or
    /// appendable struct is PLAIN_CDR and a mutable one PL_CDR; under XCDR2
    /// PLAIN_CDR2, DELIMITED_CDR and PL_CDR2. A struct without an
    /// extensibility annotation is appendable (§7.3.1.2.1.8), so its XCDR2
    /// form is delimited: a reader that assumed plain would be one delimiter
    /// out on every sample.
    #[test]
    fn xtypes_7_4_2_the_encoding_follows_the_extensibility_and_the_xcdr_version() {
        let book = TypeBook::from_idl(IDL).expect("idl");
        for (name, xcdr1, xcdr2) in [
            ("demo::F", Encoding::PlainCdr1, Encoding::PlainCdr2),
            ("demo::A", Encoding::PlainCdr1, Encoding::DelimitedCdr),
            ("demo::M", Encoding::PlCdr1, Encoding::PlCdr2),
            ("demo::D", Encoding::PlainCdr1, Encoding::DelimitedCdr),
        ] {
            let ty = book.resolve(name).expect(name);
            assert_eq!(encoding_of(&ty, false), xcdr1, "{name} under XCDR1");
            assert_eq!(encoding_of(&ty, true), xcdr2, "{name} under XCDR2");
        }
    }
}
