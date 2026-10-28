// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Forms of the wire that vendors write differently from the spec, as named
//! settings of ZeroDDS. The spec form is the default of each.

/// How the algorithm names `c.dsign_algo` and `c.kagree_algo` of a handshake
/// token are written as `BinaryProperty_t` values.
///
/// DDS-Security 1.2 §7.3.3.1: the length of the octet sequence "shall be set to
/// the number of characters in the string, counting the NUL terminating
/// character [...] the string 'ECDSA-SHA256' shall result in an octet sequence
/// value with length 13". Fast DDS hashes and parses the names without the NUL;
/// OpenDDS requires it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlgoNameNul {
    /// Always with the NUL (§7.3.3.1), the default.
    #[default]
    Spec,
    /// Never: the form of Fast DDS. A named setting, not the default.
    Omit,
    /// With the NUL as an initiator (§7.3.3.1); as a replier in the form of
    /// the request: a request without the NUL (Fast DDS) is answered without
    /// it, a request with it with it.
    Mirror,
    /// With the NUL for an OpenDDS peer and without it for any other: the
    /// form ZeroDDS wrote before the spec form was its default.
    ByVendor,
}

impl AlgoNameNul {
    /// The setting named by `value`: `spec`, `omit`, `mirror` or `by_vendor`.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "spec" => Some(Self::Spec),
            "omit" => Some(Self::Omit),
            "mirror" => Some(Self::Mirror),
            "by_vendor" => Some(Self::ByVendor),
            _ => None,
        }
    }

    /// Whether the names go out with the NUL to a peer that is (or is not) an
    /// OpenDDS participant.
    #[must_use]
    pub fn nul_for(self, peer_is_opendds: bool) -> bool {
        match self {
            Self::Spec | Self::Mirror => true,
            Self::Omit => false,
            Self::ByVendor => peer_is_opendds,
        }
    }
}

/// How the subject names of the IdentityToken (`dds.cert.sn`, `dds.ca.sn`)
/// and of the PermissionsToken (`dds.perm_ca.sn`) are written.
///
/// DDS-Security 1.2 §9.4.1.2.3 gives the string form of an X.509 subject name
/// as RFC 4514, and Tables 51 and 65 name "the subject name" without a form.
/// Fast DDS 3.6 compares a received `dds.perm_ca.sn` with the string
/// `X509_NAME_oneline(name, 0, 0)` of its own Permissions CA
/// (`FileProvider.cpp`, `Permissions.cpp`); OpenDDS compares a received
/// `dds.ca.sn` with the `X509_NAME_print_ex(.., XN_FLAG_ONELINE)` string of its
/// own CA (`AuthenticationBuiltInImpl.cpp`, `subject_name_to_str`); Cyclone DDS
/// writes `X509_NAME_oneline` (`auth_utils.c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubjectNameForm {
    /// RFC 4514, `CN=y,O=x,C=DE` (last RDN first), the default.
    #[default]
    Rfc4514,
    /// The form of OpenSSL `X509_NAME_oneline`, `/C=DE/O=x/CN=y`: the form
    /// Fast DDS compares and Cyclone DDS writes. A named setting.
    X509NameOneline,
    /// The form of OpenSSL `X509_NAME_print_ex` with `XN_FLAG_ONELINE`,
    /// `C = DE, O = x, CN = y`: the form OpenDDS compares. A named setting.
    XnFlagOneline,
}

impl SubjectNameForm {
    /// The setting named by `value`: `rfc4514`, `x509_name_oneline` or
    /// `xn_flag_oneline`.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "rfc4514" => Some(Self::Rfc4514),
            "x509_name_oneline" => Some(Self::X509NameOneline),
            "xn_flag_oneline" => Some(Self::XnFlagOneline),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §7.3.3.1: the spec form has the NUL for every peer; the two vendor
    /// forms are named settings.
    #[test]
    fn the_spec_form_has_the_nul_for_every_peer() {
        assert_eq!(AlgoNameNul::default(), AlgoNameNul::Spec);
        assert!(AlgoNameNul::Spec.nul_for(false));
        assert!(AlgoNameNul::Spec.nul_for(true));
        assert!(!AlgoNameNul::Omit.nul_for(true));
        assert!(AlgoNameNul::ByVendor.nul_for(true));
        assert!(!AlgoNameNul::ByVendor.nul_for(false));
        assert_eq!(AlgoNameNul::parse("omit"), Some(AlgoNameNul::Omit));
        assert_eq!(AlgoNameNul::parse("mirror"), Some(AlgoNameNul::Mirror));
        assert!(AlgoNameNul::Mirror.nul_for(false));
        assert_eq!(AlgoNameNul::parse("NUL"), None);
    }
}
