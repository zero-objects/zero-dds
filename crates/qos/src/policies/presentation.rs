// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! PresentationQosPolicy (DDS 1.4 §2.2.3.6).
//!
//! Wire-Format: u32 access_scope + bool coherent + bool ordered =
//! 4 + 1 + 1 + padding = 8 byte (4-byte aligned).

use zerodds_wire::{Reader, WireError, Writer};

/// Presentation-Access-Scope.
///
/// Compatibility per §2.2.3.6.6: `offered.access_scope >= requested.access_scope`
/// (INSTANCE < TOPIC < GROUP).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(u32)]
pub enum PresentationAccessScope {
    /// Instance-Scope (default).
    #[default]
    Instance = 0,
    /// Topic-Scope.
    Topic = 1,
    /// Group-scope (Publisher/Subscriber-wide).
    Group = 2,
}

impl PresentationAccessScope {
    /// Strict mapper.
    #[must_use]
    pub const fn try_from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::Instance),
            1 => Some(Self::Topic),
            2 => Some(Self::Group),
            _ => None,
        }
    }

    /// Forward-compatible mapper.
    #[must_use]
    pub const fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::Topic,
            2 => Self::Group,
            _ => Self::Instance,
        }
    }
}

/// PresentationQosPolicy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PresentationQosPolicy {
    /// Access-Scope.
    pub access_scope: PresentationAccessScope,
    /// Coherent-Access.
    pub coherent_access: bool,
    /// Ordered-Access.
    pub ordered_access: bool,
}

impl PresentationQosPolicy {
    /// Wire-Encoding.
    pub fn encode_into(self, w: &mut Writer) {
        w.write_u32(self.access_scope as u32);
        w.write_u8(u8::from(self.coherent_access));
        w.write_u8(u8::from(self.ordered_access));
        // 2 byte padding to 4-byte alignment.
        w.write_u8(0);
        w.write_u8(0);
    }

    /// Wire-Decoding (strict).
    ///
    /// # Errors
    /// Buffer underflow or unknown AccessScope value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let at = r.position();
        let v = r.read_u32()?;
        let access_scope = PresentationAccessScope::try_from_u32(v).ok_or_else(|| {
            WireError::refused(
                at,
                &alloc::format!("a PresentationAccessScope enumerator, not {v}"),
            )
        })?;
        let coherent_access = r.read_u8()? != 0;
        let ordered_access = r.read_u8()? != 0;
        let _pad1 = r.read_u8()?;
        let _pad2 = r.read_u8()?;
        Ok(Self {
            access_scope,
            coherent_access,
            ordered_access,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_wire::Endianness;

    #[test]
    fn default_instance_no_flags() {
        let d = PresentationQosPolicy::default();
        assert_eq!(d.access_scope, PresentationAccessScope::Instance);
        assert!(!d.coherent_access);
        assert!(!d.ordered_access);
    }

    #[test]
    fn scope_ordering() {
        use PresentationAccessScope::*;
        assert!(Instance < Topic);
        assert!(Topic < Group);
    }

    #[test]
    fn try_from_u32_strict() {
        assert_eq!(
            PresentationAccessScope::try_from_u32(0),
            Some(PresentationAccessScope::Instance)
        );
        assert_eq!(
            PresentationAccessScope::try_from_u32(2),
            Some(PresentationAccessScope::Group)
        );
        assert_eq!(PresentationAccessScope::try_from_u32(5), None);
    }

    #[test]
    fn from_u32_forward_compat() {
        assert_eq!(
            PresentationAccessScope::from_u32(99),
            PresentationAccessScope::Instance
        );
        assert_eq!(
            PresentationAccessScope::from_u32(1),
            PresentationAccessScope::Topic
        );
    }

    #[test]
    fn roundtrip_instance_no_flags() {
        let p = PresentationQosPolicy::default();
        let mut w = Writer::new(Endianness::Little);
        p.encode_into(&mut w);
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Little);
        assert_eq!(PresentationQosPolicy::decode_from(&mut r).unwrap(), p);
    }

    #[test]
    fn roundtrip_all_flags() {
        let p = PresentationQosPolicy {
            access_scope: PresentationAccessScope::Group,
            coherent_access: true,
            ordered_access: true,
        };
        let mut w = Writer::new(Endianness::Little);
        p.encode_into(&mut w);
        let bytes = w.finish();
        assert_eq!(bytes.len(), 8);
        let mut r = Reader::new(&bytes, Endianness::Little);
        assert_eq!(PresentationQosPolicy::decode_from(&mut r).unwrap(), p);
    }
}
