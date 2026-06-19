// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! RepositoryId — spec §10.7.3.
//!
//! Format: `IDL:<scoped-name>:<major>.<minor>`. Example:
//! `IDL:omg.org/CosNaming/NamingContext:1.0`.

use alloc::string::{String, ToString};

use crate::error::{IrError, IrResult};

/// Structured RepositoryId.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RepositoryId {
    /// Scoped name in `:`-separated format (e.g.
    /// `omg.org/CosNaming/NamingContext`).
    pub scoped_name: String,
    /// Major version (`1`).
    pub major: u16,
    /// Minor version (`0`).
    pub minor: u16,
}

impl RepositoryId {
    /// Constructor.
    #[must_use]
    pub fn new(scoped_name: impl Into<String>, major: u16, minor: u16) -> Self {
        Self {
            scoped_name: scoped_name.into(),
            major,
            minor,
        }
    }

    /// Parses a canonical RepositoryId string.
    ///
    /// # Errors
    /// `InvalidRepositoryId` for forms other than `IDL:<scoped>:<m>.<n>`.
    pub fn parse(s: &str) -> IrResult<Self> {
        let payload = s
            .strip_prefix("IDL:")
            .ok_or_else(|| IrError::InvalidRepositoryId(s.to_string()))?;
        let (scoped, version) = payload
            .rsplit_once(':')
            .ok_or_else(|| IrError::InvalidRepositoryId(s.to_string()))?;
        let (maj, minr) = version
            .split_once('.')
            .ok_or_else(|| IrError::InvalidRepositoryId(s.to_string()))?;
        let major = maj
            .parse::<u16>()
            .map_err(|_| IrError::InvalidRepositoryId(s.to_string()))?;
        let minor = minr
            .parse::<u16>()
            .map_err(|_| IrError::InvalidRepositoryId(s.to_string()))?;
        Ok(Self {
            scoped_name: scoped.to_string(),
            major,
            minor,
        })
    }

    /// Returns the canonical string form.
    #[must_use]
    pub fn to_canonical(&self) -> String {
        alloc::format!("IDL:{}:{}.{}", self.scoped_name, self.major, self.minor)
    }
}

impl core::fmt::Display for RepositoryId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "IDL:{}:{}.{}", self.scoped_name, self.major, self.minor)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_omg_namingcontext() {
        let s = "IDL:omg.org/CosNaming/NamingContext:1.0";
        let r = RepositoryId::parse(s).unwrap();
        assert_eq!(r.scoped_name, "omg.org/CosNaming/NamingContext");
        assert_eq!(r.major, 1);
        assert_eq!(r.minor, 0);
        assert_eq!(r.to_canonical(), s);
    }

    #[test]
    fn round_trip_user_repo_id() {
        let s = "IDL:demo/Echo:2.5";
        let r = RepositoryId::parse(s).unwrap();
        assert_eq!(r.scoped_name, "demo/Echo");
        assert_eq!(r.major, 2);
        assert_eq!(r.minor, 5);
    }

    #[test]
    fn missing_idl_prefix_is_invalid() {
        assert!(RepositoryId::parse("omg.org/X:1.0").is_err());
    }

    #[test]
    fn missing_version_is_invalid() {
        assert!(RepositoryId::parse("IDL:omg.org/X").is_err());
    }

    #[test]
    fn non_numeric_version_is_invalid() {
        assert!(RepositoryId::parse("IDL:omg.org/X:a.b").is_err());
    }
}
