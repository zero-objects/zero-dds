// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Codec-Profile-Conformance-Marker.
//!
//! Spec DDS-AMQP-1.0:
//! * §2.3 Codec Profile — Type-System + Performatives + Sections
//!   (voll).
//! * §2.4 Codec-Lite Profile — strikte Untermenge: nur Primitive
//!   + `data`-Body-Section.
//!
//! Cargo-Feature `codec-lite` aktiviert die Lite-Variante des
//! Markers. Das beeinflusst nicht den ausgelieferten Code (der
//! ist immer das volle Codec-Profile), sondern den
//! Conformance-Claim: Code, der nur Lite-Subset benutzt, kann
//! gegen `is_codec_lite_active()` testen, dass keine compound-
//! types out-of-spec verwendet werden.

use crate::extended_types::AmqpExtValue;
use crate::sections::MessageSection;

/// Spec §2.3 / §2.4 — claimed Profile dieser Crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecProfile {
    /// `Codec` — voller Codec-Profile (Default).
    Full,
    /// `Codec-Lite` — strikte Untermenge.
    Lite,
}

/// Welches Profile wird durch das Cargo-Feature `codec-lite`
/// claimed?
#[must_use]
pub const fn active_profile() -> CodecProfile {
    if cfg!(feature = "codec-lite") {
        CodecProfile::Lite
    } else {
        CodecProfile::Full
    }
}

/// Spec §2.4 — pruefen, ob ein `AmqpExtValue` Codec-Lite-konform
/// ist.
///
/// Codec-Lite erlaubt:
/// * alle Primitive (null, boolean, int-Typen, float, double,
///   char, timestamp, uuid, binary, string, symbol).
///
/// Codec-Lite verbietet:
/// * `List`, `Map`, `Array` (compound types).
#[must_use]
pub const fn is_codec_lite_value(v: &AmqpExtValue) -> bool {
    matches!(
        v,
        AmqpExtValue::Null
            | AmqpExtValue::Boolean(_)
            | AmqpExtValue::Ubyte(_)
            | AmqpExtValue::Ushort(_)
            | AmqpExtValue::Uint(_)
            | AmqpExtValue::Ulong(_)
            | AmqpExtValue::Byte(_)
            | AmqpExtValue::Short(_)
            | AmqpExtValue::Int(_)
            | AmqpExtValue::Long(_)
            | AmqpExtValue::Float(_)
            | AmqpExtValue::Double(_)
            | AmqpExtValue::Char(_)
            | AmqpExtValue::Timestamp(_)
            | AmqpExtValue::Uuid(_)
            | AmqpExtValue::Binary(_)
            | AmqpExtValue::Str(_)
            | AmqpExtValue::Symbol(_)
    )
}

/// Spec §2.4 — pruefen, ob eine `MessageSection` Codec-Lite-
/// konform ist (nur `Data`-Body, alle anderen Sections out-of-
/// scope).
#[must_use]
pub const fn is_codec_lite_section(s: &MessageSection) -> bool {
    matches!(s, MessageSection::Data(_))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn active_profile_full_by_default() {
        // Tests laufen ohne `codec-lite`-Feature → Full.
        // Ist `codec-lite` aktiv waere `Lite`.
        let p = active_profile();
        let expected = if cfg!(feature = "codec-lite") {
            CodecProfile::Lite
        } else {
            CodecProfile::Full
        };
        assert_eq!(p, expected);
    }

    #[test]
    fn primitives_are_codec_lite() {
        for v in [
            AmqpExtValue::Null,
            AmqpExtValue::Boolean(true),
            AmqpExtValue::Ubyte(0),
            AmqpExtValue::Int(-1),
            AmqpExtValue::Long(42),
            AmqpExtValue::Float(1.0),
            AmqpExtValue::Double(2.0),
            AmqpExtValue::Timestamp(1_000),
            AmqpExtValue::Uuid([0; 16]),
        ] {
            assert!(is_codec_lite_value(&v), "{v:?} should be lite");
        }
    }

    #[test]
    fn list_map_array_are_not_codec_lite() {
        assert!(!is_codec_lite_value(&AmqpExtValue::List(vec![])));
        assert!(!is_codec_lite_value(&AmqpExtValue::Map(vec![])));
        assert!(!is_codec_lite_value(&AmqpExtValue::Array(vec![])));
    }

    #[test]
    fn data_section_is_codec_lite() {
        assert!(is_codec_lite_section(&MessageSection::Data(vec![1, 2, 3])));
    }

    #[test]
    fn other_sections_are_not_codec_lite() {
        let v = AmqpExtValue::Null;
        for s in [
            MessageSection::Header(v.clone()),
            MessageSection::Properties(v.clone()),
            MessageSection::ApplicationProperties(v.clone()),
            MessageSection::AmqpValue(v.clone()),
            MessageSection::AmqpSequence(vec![]),
            MessageSection::Footer(v.clone()),
            MessageSection::DeliveryAnnotations(v.clone()),
            MessageSection::MessageAnnotations(v.clone()),
        ] {
            assert!(!is_codec_lite_section(&s), "{s:?} should not be lite");
        }
    }
}
