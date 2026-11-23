// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Identity assertion (CORBA 3.3 Part 2 §10.2.5, §10.3.1.1, §10.6.1.2.3).
//!
//! * The GSS mechanism-independent exported name of RFC 2743 §3.2 with
//!   the GSSUP mechanism OID and a scoped-username (§10.2.5.1,
//!   §10.2.5.2): [`ScopedUsername`], [`exported_gssup_name`],
//!   [`parse_exported_name`].
//! * The forms an identity token's octets arrive in: the ASN.1 encoding
//!   directly, or a CDR encapsulation of it (first octet 0 or 1,
//!   §10.2.5): [`AssertedIdentity::from_token`].
//! * Backward trust evaluation (§10.3.1.1): the rules of the target that
//!   say which client authentication identity may assert which identity,
//!   [`AssertionTrust`]. An identity is always trusted to assert exactly
//!   itself.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_wire::{Endianness, Reader};

use crate::gssup::GSSUP_OID_DER;
use crate::sas::IdentityToken;

/// The token identifier of an exported name (RFC 2743 §3.2).
const EXPORTED_NAME_TOK_ID: [u8; 2] = [0x04, 0x01];

/// A scoped-username (§10.2.5.2): `name_value`, `name_value@name_scope`
/// or `@name_scope`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScopedUsername {
    /// The user name; empty for the `@name_scope` form.
    pub name_value: String,
    /// The scope, if any.
    pub name_scope: Option<String>,
}

impl ScopedUsername {
    /// Parses the scoped-username syntax: the first unquoted `@`
    /// delimits value and scope; `\@` and `\\` are a quoted `@` and `\`.
    ///
    /// # Errors
    /// [`IdentityError::Malformed`] for an empty name, an empty value
    /// with no scope, an empty scope after `@`, a second unquoted `@` or a
    /// `\` that quotes neither `@` nor `\`.
    pub fn parse(text: &str) -> Result<Self, IdentityError> {
        let mut value = String::new();
        let mut scope = String::new();
        let mut scoped = false;
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            let ch = match c {
                '\\' => match chars.next() {
                    Some(q @ ('@' | '\\')) => q,
                    _ => return Err(IdentityError::Malformed("a \\ that quotes @ or \\")),
                },
                '@' if !scoped => {
                    scoped = true;
                    continue;
                }
                '@' => return Err(IdentityError::Malformed("one unquoted @")),
                other => other,
            };
            if scoped {
                scope.push(ch);
            } else {
                value.push(ch);
            }
        }
        if scoped && scope.is_empty() {
            return Err(IdentityError::Malformed("a scope after @"));
        }
        if !scoped && value.is_empty() {
            return Err(IdentityError::Malformed("a name value or a scope"));
        }
        Ok(Self {
            name_value: value,
            name_scope: scoped.then_some(scope),
        })
    }

    /// The scoped-username text, with `@` and `\` inside value and scope
    /// quoted.
    #[must_use]
    pub fn to_text(&self) -> String {
        let quote = |s: &str, out: &mut String| {
            for c in s.chars() {
                if c == '@' || c == '\\' {
                    out.push('\\');
                }
                out.push(c);
            }
        };
        let mut out = String::new();
        quote(&self.name_value, &mut out);
        if let Some(scope) = &self.name_scope {
            out.push('@');
            quote(scope, &mut out);
        }
        out
    }
}

/// Why an identity could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityError {
    /// The octets are not of the expected form; the text names it.
    Malformed(&'static str),
    /// An exported name of another mechanism than GSSUP: the target has
    /// no reading of its name (§10.2.5: the mechanism is invalid).
    UnsupportedMechanism(Vec<u8>),
}

/// The GSS exported name (RFC 2743 §3.2) of a GSSUP scoped-username:
/// `04 01`, the two-octet length of the mechanism OID, its DER form, the
/// four-octet length of the name, the name (§10.2.5.1).
#[must_use]
pub fn exported_gssup_name(name: &ScopedUsername) -> Vec<u8> {
    let text = name.to_text();
    let mut out = Vec::with_capacity(8 + GSSUP_OID_DER.len() + text.len());
    out.extend_from_slice(&EXPORTED_NAME_TOK_ID);
    #[allow(clippy::cast_possible_truncation)]
    out.extend_from_slice(&(GSSUP_OID_DER.len() as u16).to_be_bytes());
    out.extend_from_slice(GSSUP_OID_DER);
    #[allow(clippy::cast_possible_truncation)]
    out.extend_from_slice(&(text.len() as u32).to_be_bytes());
    out.extend_from_slice(text.as_bytes());
    out
}

/// Reads an exported name: its mechanism OID (DER) and its name octets.
///
/// # Errors
/// [`IdentityError::Malformed`] for octets that are not an exported name.
pub fn parse_exported_name(bytes: &[u8]) -> Result<(Vec<u8>, Vec<u8>), IdentityError> {
    let bad = IdentityError::Malformed("an exported name (RFC 2743 3.2)");
    if bytes.len() < 4 || bytes[..2] != EXPORTED_NAME_TOK_ID {
        return Err(bad);
    }
    let oid_len = usize::from(u16::from_be_bytes([bytes[2], bytes[3]]));
    let oid_end = 4 + oid_len;
    let name_start = oid_end + 4;
    if bytes.len() < name_start {
        return Err(bad);
    }
    let oid = bytes[4..oid_end].to_vec();
    let name_len = u32::from_be_bytes([
        bytes[oid_end],
        bytes[oid_end + 1],
        bytes[oid_end + 2],
        bytes[oid_end + 3],
    ]) as usize;
    if bytes.len() != name_start + name_len {
        return Err(bad);
    }
    Ok((oid, bytes[name_start..].to_vec()))
}

/// The ASN.1 octets of an identity token: given directly, or inside a
/// CDR encapsulation (`sequence<octet>` after the byte-order octet) when
/// the first octet is 0 or 1 (§10.2.5).
fn token_octets(bytes: &[u8]) -> Result<Vec<u8>, IdentityError> {
    match bytes.first() {
        Some(0 | 1) => {
            let e = if bytes[0] == 0 {
                Endianness::Big
            } else {
                Endianness::Little
            };
            let mut r = Reader::new(bytes, e);
            let parsed = r.read_u8().and_then(|_| {
                let n = r.read_u32()? as usize;
                r.read_bytes(n).map(<[u8]>::to_vec)
            });
            parsed.map_err(|_| IdentityError::Malformed("a CDR encapsulation of an octet sequence"))
        }
        Some(_) => Ok(bytes.to_vec()),
        None => Err(IdentityError::Malformed("an identity token with octets")),
    }
}

/// An asserted identity as the target compares it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AssertedIdentity {
    /// `ITTAnonymous`.
    Anonymous,
    /// `ITTPrincipalName` with the GSSUP mechanism.
    GssupPrincipal(ScopedUsername),
    /// `ITTDistinguishedName`: the ASN.1 DER of the name.
    DistinguishedName(Vec<u8>),
    /// `ITTX509CertChain`: the ASN.1 DER of the chain.
    X509CertChain(Vec<u8>),
}

impl AssertedIdentity {
    /// The identity a token asserts; `None` for `ITTAbsent`.
    ///
    /// # Errors
    /// [`IdentityError::UnsupportedMechanism`] for a principal name of
    /// another mechanism than GSSUP and for an identity extension;
    /// [`IdentityError::Malformed`] for octets of the wrong form.
    pub fn from_token(token: &IdentityToken) -> Result<Option<Self>, IdentityError> {
        Ok(Some(match token {
            IdentityToken::Absent => return Ok(None),
            IdentityToken::Anonymous => Self::Anonymous,
            IdentityToken::PrincipalName(b) => {
                let (oid, name) = parse_exported_name(&token_octets(b)?)?;
                if oid != GSSUP_OID_DER {
                    return Err(IdentityError::UnsupportedMechanism(oid));
                }
                let text = core::str::from_utf8(&name)
                    .map_err(|_| IdentityError::Malformed("a scoped-username in UTF-8"))?;
                Self::GssupPrincipal(ScopedUsername::parse(text)?)
            }
            IdentityToken::DistinguishedName(b) => Self::DistinguishedName(der_value(b)?),
            IdentityToken::X509CertChain(b) => Self::X509CertChain(der_value(b)?),
            IdentityToken::Extension { id, .. } => {
                return Err(IdentityError::UnsupportedMechanism(id.clone()));
            }
        }))
    }

    /// The principal text a server's access control sees: the
    /// scoped-username, or the hexadecimal DER of a name or chain.
    #[must_use]
    pub fn principal_text(&self) -> String {
        match self {
            Self::Anonymous => String::from("anonymous"),
            Self::GssupPrincipal(n) => n.to_text(),
            Self::DistinguishedName(der) | Self::X509CertChain(der) => {
                der.iter().map(|b| alloc::format!("{b:02x}")).collect()
            }
        }
    }
}

/// An ASN.1 SEQUENCE (`0x30`) given directly or in an encapsulation.
fn der_value(bytes: &[u8]) -> Result<Vec<u8>, IdentityError> {
    let der = token_octets(bytes)?;
    if der.first() == Some(&0x30) {
        Ok(der)
    } else {
        Err(IdentityError::Malformed("an ASN.1 SEQUENCE"))
    }
}

/// What a client authentication identity is trusted to assert.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Trusted {
    Any,
    Only(BTreeSet<AssertedIdentity>),
}

/// The trust rules of a target for backward trust evaluation
/// (§10.3.1.1): which client authentication identity (a GSSUP
/// scoped-username) may assert which identities. Without a rule an
/// identity may assert exactly itself and the anonymous identity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssertionTrust {
    rules: BTreeMap<ScopedUsername, Trusted>,
}

impl AssertionTrust {
    /// Rules that trust no identity to assert another one.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `proxy` may assert any identity.
    pub fn allow_any(&mut self, proxy: ScopedUsername) {
        self.rules.insert(proxy, Trusted::Any);
    }

    /// `proxy` may assert `asserted`.
    pub fn allow(&mut self, proxy: ScopedUsername, asserted: AssertedIdentity) {
        match self
            .rules
            .entry(proxy)
            .or_insert_with(|| Trusted::Only(BTreeSet::new()))
        {
            Trusted::Any => {}
            Trusted::Only(set) => {
                set.insert(asserted);
            }
        }
    }

    /// Whether `proxy`, the client authentication identity, is trusted
    /// to assert `asserted`.
    #[must_use]
    pub fn trusts(&self, proxy: &ScopedUsername, asserted: &AssertedIdentity) -> bool {
        match asserted {
            AssertedIdentity::Anonymous => return true,
            AssertedIdentity::GssupPrincipal(name) if name == proxy => return true,
            _ => {}
        }
        match self.rules.get(proxy) {
            Some(Trusted::Any) => true,
            Some(Trusted::Only(set)) => set.contains(asserted),
            None => false,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::vec;

    fn name(s: &str) -> ScopedUsername {
        ScopedUsername::parse(s).unwrap()
    }

    #[test]
    fn corba_p2_10_2_5_2_scoped_username_forms_and_quoting() {
        assert_eq!(
            name("alice"),
            ScopedUsername {
                name_value: "alice".into(),
                name_scope: None
            }
        );
        assert_eq!(name("alice@REALM").name_scope.as_deref(), Some("REALM"));
        assert_eq!(name("@REALM").name_value, "");
        let quoted = name(r"a\@b\\c@x\@y");
        assert_eq!(quoted.name_value, r"a@b\c");
        assert_eq!(quoted.name_scope.as_deref(), Some("x@y"));
        assert_eq!(quoted.to_text(), r"a\@b\\c@x\@y");
        for bad in ["", "a@", "a@b@c", r"a\b", "@"] {
            assert!(ScopedUsername::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn corba_p2_10_2_5_1_the_gssup_exported_name_follows_rfc_2743() {
        let bytes = exported_gssup_name(&name("bob@CORP"));
        let mut expected = vec![0x04, 0x01, 0x00, 0x08];
        expected.extend_from_slice(GSSUP_OID_DER);
        expected.extend_from_slice(&[0, 0, 0, 8]);
        expected.extend_from_slice(b"bob@CORP");
        assert_eq!(bytes, expected);
        let (oid, n) = parse_exported_name(&bytes).unwrap();
        assert_eq!(oid, GSSUP_OID_DER);
        assert_eq!(n, b"bob@CORP");
        assert!(parse_exported_name(&bytes[..bytes.len() - 1]).is_err());
    }

    #[test]
    fn corba_p2_10_2_5_a_token_is_read_directly_or_from_an_encapsulation() {
        let direct = exported_gssup_name(&name("carol"));
        // CDR encapsulation: byte order, then sequence<octet>.
        let mut encap = vec![0x00, 0, 0, 0];
        encap.extend_from_slice(&u32::try_from(direct.len()).unwrap().to_be_bytes());
        encap.extend_from_slice(&direct);
        for bytes in [direct, encap] {
            assert_eq!(
                AssertedIdentity::from_token(&IdentityToken::PrincipalName(bytes)).unwrap(),
                Some(AssertedIdentity::GssupPrincipal(name("carol")))
            );
        }
        // Another mechanism's name: the mechanism is not supported.
        let mut kerberos = vec![0x04, 0x01, 0x00, 0x0b];
        kerberos.extend_from_slice(&[0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x12, 1, 2, 2]);
        kerberos.extend_from_slice(&[0, 0, 0, 1, b'k']);
        assert!(matches!(
            AssertedIdentity::from_token(&IdentityToken::PrincipalName(kerberos)),
            Err(IdentityError::UnsupportedMechanism(_))
        ));
        assert!(matches!(
            AssertedIdentity::from_token(&IdentityToken::DistinguishedName(vec![0x31, 0])),
            Err(IdentityError::Malformed(_))
        ));
        assert_eq!(
            AssertedIdentity::from_token(&IdentityToken::Absent).unwrap(),
            None
        );
    }

    #[test]
    fn corba_p2_10_3_1_1_backward_trust_evaluation() {
        let mut trust = AssertionTrust::new();
        let gateway = name("gateway");
        let alice = AssertedIdentity::GssupPrincipal(name("alice"));
        let bob = AssertedIdentity::GssupPrincipal(name("bob"));
        // Exactly itself, and anonymous, without a rule.
        assert!(trust.trusts(&gateway, &AssertedIdentity::GssupPrincipal(name("gateway"))));
        assert!(trust.trusts(&gateway, &AssertedIdentity::Anonymous));
        assert!(!trust.trusts(&gateway, &alice));
        trust.allow(gateway.clone(), alice.clone());
        assert!(trust.trusts(&gateway, &alice));
        assert!(!trust.trusts(&gateway, &bob));
        // The same name in another scope is another identity.
        assert!(!trust.trusts(&gateway, &AssertedIdentity::GssupPrincipal(name("alice@X"))));
        trust.allow_any(name("root"));
        assert!(trust.trusts(&name("root"), &bob));
        assert!(trust.trusts(
            &name("root"),
            &AssertedIdentity::DistinguishedName(vec![0x30, 0])
        ));
    }
}
