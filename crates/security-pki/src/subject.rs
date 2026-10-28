// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The SubjectName of an identity certificate as attribute pairs, for the
//! binding of a permissions grant to the identity (DDS-Security 1.2
//! §10.4.1.5.3.1: "The contents of the subject name element shall be the
//! x.509 subject name for the DomainParticipant as is given in the
//! 'Subject' field of its Identity Certificate").

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::identity::PkiError;

/// The attributes of the certificate's SubjectName as `(name, value)`, in
/// the order of the certificate, with the attribute names OpenSSL prints
/// (`CN`, `O`, `OU`, `C`, `ST`, `L`, `emailAddress`, …) upper-cased, and
/// the dotted OID for an attribute without one.
///
/// # Errors
/// `PkiError::InvalidPem` if `cert_der` is no certificate or an attribute
/// value is not a string.
pub fn subject_attributes(cert_der: &[u8]) -> Result<Vec<(String, String)>, PkiError> {
    use x509_cert::Certificate;
    use x509_cert::der::Decode;
    use x509_cert::der::asn1::{Ia5StringRef, PrintableStringRef, TeletexStringRef, Utf8StringRef};
    use x509_cert::der::{Tag, Tagged};

    let cert = Certificate::from_der(cert_der)
        .map_err(|e| PkiError::InvalidPem(alloc::format!("subject: cert der: {e}")))?;
    let mut out = Vec::new();
    for rdn in cert.tbs_certificate.subject.0.iter() {
        for atv in rdn.0.iter() {
            let value = match atv.value.tag() {
                Tag::PrintableString => PrintableStringRef::try_from(&atv.value)
                    .ok()
                    .map(|s| s.as_str().to_string()),
                Tag::Utf8String => Utf8StringRef::try_from(&atv.value)
                    .ok()
                    .map(|s| s.as_str().to_string()),
                Tag::Ia5String => Ia5StringRef::try_from(&atv.value)
                    .ok()
                    .map(|s| s.as_str().to_string()),
                Tag::TeletexString => TeletexStringRef::try_from(&atv.value)
                    .ok()
                    .map(|s| s.as_str().to_string()),
                _ => None,
            }
            .ok_or_else(|| {
                PkiError::InvalidPem(alloc::format!(
                    "subject: attribute {} is not a string",
                    atv.oid
                ))
            })?;
            out.push((attribute_name(&atv.oid.to_string()), value));
        }
    }
    Ok(out)
}

/// The OpenSSL short name of an attribute OID, upper-cased; the dotted OID
/// otherwise.
fn attribute_name(oid: &str) -> String {
    let name = match oid {
        "2.5.4.3" => "CN",
        "2.5.4.4" => "SN",
        "2.5.4.5" => "SERIALNUMBER",
        "2.5.4.6" => "C",
        "2.5.4.7" => "L",
        "2.5.4.8" => "ST",
        "2.5.4.9" => "STREET",
        "2.5.4.10" => "O",
        "2.5.4.11" => "OU",
        "2.5.4.12" => "TITLE",
        "2.5.4.42" => "GN",
        "2.5.4.43" => "INITIALS",
        "2.5.4.46" => "DNQUALIFIER",
        "0.9.2342.19200300.100.1.1" => "UID",
        "0.9.2342.19200300.100.1.25" => "DC",
        "1.2.840.113549.1.9.1" => "EMAILADDRESS",
        other => other,
    };
    name.to_string()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_subject_is_read_in_certificate_order_with_short_names() {
        use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};
        let mut params = CertificateParams::new(Vec::new()).unwrap();
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CountryName, "DE");
        dn.push(DnType::OrganizationName, "Example, Inc.");
        dn.push(DnType::CommonName, "alice");
        params.distinguished_name = dn;
        let key = KeyPair::generate().unwrap();
        let cert = params.self_signed(&key).unwrap();
        let subject = subject_attributes(cert.der()).unwrap();
        assert_eq!(
            subject,
            alloc::vec![
                ("C".to_string(), "DE".to_string()),
                ("O".to_string(), "Example, Inc.".to_string()),
                ("CN".to_string(), "alice".to_string()),
            ]
        );
    }
}
