// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Sources of secure time — OMG Time Service 1.1 §1.1.3, §2.1.2,
//! Appendix A and B.1.1. Feature `std`.
//!
//! `secure_universal_time` returns a time only from a source an
//! administrator designated as authoritative through
//! [`crate::admin::TimeAdmin::designate_source`] (an authorized and
//! audited operation, App. B.1). The designation names the source, the
//! source itself and how the values its clock is synchronized with are
//! protected in transit ([`SyncProtection`], App. B.1.1). Each reading of
//! the source says whether the clock was synchronized and gives the
//! bound of its error ([`SourceReading`], §1.1.3).
//!
//! ZeroDDS synchronizes no clock and sends no synchronization values:
//! the clock of the host is synchronized by the host (chrony, ntpd,
//! ptp4l, a reference clock). [`SyncProtection`] is the administrator's
//! statement how that synchronization is protected; the source cannot
//! verify it, the audit record of the designation keeps who made it.

use alloc::string::String;
use alloc::sync::Arc;
use core::fmt;

use crate::service::TimeUnavailable;
use crate::time_base::{InaccuracyT, TimeT};

/// How the values the clock of a source is synchronized with are
/// protected while they are in transit over the network (App. B.1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyncProtection {
    /// No protection: plain NTP (RFC 5905 without authentication), PTP
    /// without the authentication TLV, or not known. A source with this
    /// protection never gives secure time.
    Unprotected,
    /// No synchronization values cross the network: a reference clock on
    /// the node (GNSS receiver with PPS, atomic clock, the PTP hardware
    /// clock of a local grandmaster).
    LocalReference,
    /// NTP with Network Time Security (RFC 8915).
    Nts,
    /// NTP with a symmetric key and a message authentication code
    /// (RFC 5905 §7.3, AES-CMAC per RFC 8573).
    NtpSymmetricKey,
    /// PTP with the authentication TLV (IEEE 1588-2019 §16.14, Annex P).
    PtpAuthenticated,
}

impl SyncProtection {
    /// `true` for every protection except [`Self::Unprotected`].
    #[must_use]
    pub const fn is_protected(self) -> bool {
        !matches!(self, Self::Unprotected)
    }

    /// The name in audit records: `unprotected`, `local-reference`,
    /// `nts`, `ntp-symmetric-key`, `ptp-authenticated`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Unprotected => "unprotected",
            Self::LocalReference => "local-reference",
            Self::Nts => "nts",
            Self::NtpSymmetricKey => "ntp-symmetric-key",
            Self::PtpAuthenticated => "ptp-authenticated",
        }
    }
}

impl fmt::Display for SyncProtection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// One reading of a [`SecureTimeSource`] (§1.1.3: "able to return
/// current time with an associated error parameter").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceReading {
    /// The time, 100 ns ticks since 1582-10-15 (§1.3.2.1).
    pub time: TimeT,
    /// The bound of the error of `time` in 100 ns units: the true time
    /// lies within `time ± inaccuracy` (§1.1.3: "can be trusted to be
    /// within the inaccuracy window provided by the underlying system").
    pub inaccuracy: InaccuracyT,
    /// `true` if the clock was synchronized to its reference when it was
    /// read; an unsynchronized clock gives no secure time.
    pub synchronized: bool,
}

/// A clock that can be designated as the authoritative source of a
/// [`crate::TimeService`].
///
/// An implementation reads the clock and its error bound at the same
/// instant and reports the state of synchronization of that instant;
/// it returns [`TimeUnavailable`] when the clock cannot be read. The
/// Linux kernel clock disciplined by the host
/// (`zerodds_dcps::time_source::KernelClock`, `adjtimex(2)`) is one.
pub trait SecureTimeSource: Send + Sync {
    /// Reads the clock.
    ///
    /// # Errors
    /// [`TimeUnavailable`] if the clock cannot be read.
    fn read(&self) -> Result<SourceReading, TimeUnavailable>;
}

/// An authoritative source as an administrator designates it (App. B.1).
#[derive(Clone)]
pub struct SourceDesignation {
    name: String,
    protection: SyncProtection,
    source: Arc<dyn SecureTimeSource>,
}

impl SourceDesignation {
    /// `source` under `name` (for audit records), with the protection of
    /// its synchronization in transit as the administrator states it.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        protection: SyncProtection,
        source: Arc<dyn SecureTimeSource>,
    ) -> Self {
        Self {
            name: name.into(),
            protection,
            source,
        }
    }

    /// The name of the source.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The stated protection of its synchronization.
    #[must_use]
    pub const fn protection(&self) -> SyncProtection {
        self.protection
    }

    /// Reads the source.
    ///
    /// # Errors
    /// [`TimeUnavailable`] if the clock cannot be read.
    pub fn read(&self) -> Result<SourceReading, TimeUnavailable> {
        self.source.read()
    }

    /// `<name> (<protection>)`, the form audit records use.
    #[must_use]
    pub fn describe(&self) -> String {
        alloc::format!("{} ({})", self.name, self.protection)
    }
}

impl fmt::Debug for SourceDesignation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceDesignation")
            .field("name", &self.name)
            .field("protection", &self.protection)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn time_b_1_1_only_unprotected_synchronization_is_not_protected() {
        assert!(!SyncProtection::Unprotected.is_protected());
        for p in [
            SyncProtection::LocalReference,
            SyncProtection::Nts,
            SyncProtection::NtpSymmetricKey,
            SyncProtection::PtpAuthenticated,
        ] {
            assert!(p.is_protected(), "{p}");
        }
        assert_eq!(
            SyncProtection::NtpSymmetricKey.to_string(),
            "ntp-symmetric-key"
        );
    }

    struct Fixed(SourceReading);
    impl SecureTimeSource for Fixed {
        fn read(&self) -> Result<SourceReading, TimeUnavailable> {
            Ok(self.0)
        }
    }

    #[test]
    fn time_a_1_readme_states_every_criterion() {
        // App. A: "Appropriate documented handling of the criteria [...]
        // is mandatory"; App. B.1.1: "implementors must state whether or
        // not their implementation is secure". The statement is the
        // README section; every test it names exists.
        let readme = include_str!("../README.md");
        let section = readme
            .split("## Secure time (Appendix A and B)")
            .nth(1)
            .and_then(|rest| rest.split("\n## ").next())
            .expect("README section on secure time");
        for needle in [
            "**Is the implementation secure?**",
            "| 1 | Only administrators authorized",
            "| 2 | Setting and resetting the time",
            "| 3 | An authoritative source",
            "| 4 | Current time with an error parameter | §1.1.3",
            "| 5 | Trusted within the inaccuracy window | §1.1.3",
            "| 6 | Monotonically increasing | §1.1.3",
            "| 7 | Synchronization values protected in transit | App. B.1.1",
            "| 8 | Proxies | App. B.2",
            "**Synchronization in transit (B.1.1).**",
            "**Threats of the Security Service.**",
            "| An authorized user gaining access",
            "| A user masquerading as someone else",
            "| Security controls being bypassed",
            "| Eavesdropping on a communication line",
            "| Tampering with communication between objects",
            "| Lack of accountability",
            "**Proxies (B.2).**",
        ] {
            assert!(section.contains(needle), "README lacks {needle:?}");
        }
        let sources = [
            include_str!("admin.rs"),
            include_str!("servant.rs"),
            include_str!("proxy.rs"),
            include_str!("../tests/time_svc_e2e.rs"),
        ];
        let mut named = 0;
        for word in section.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
            if word.starts_with("time_a_") || word.starts_with("time_b_") {
                named += 1;
                let def = alloc::format!("fn {word}(");
                assert!(
                    sources.iter().any(|s| s.contains(&def)),
                    "README names {word}, no such test"
                );
            }
        }
        assert!(named >= 10, "{named} tests named");
    }

    #[test]
    fn time_b_1_designation_describes_name_and_protection() {
        let reading = SourceReading {
            time: 5,
            inaccuracy: 1,
            synchronized: true,
        };
        let d = SourceDesignation::new(
            "gnss0",
            SyncProtection::LocalReference,
            Arc::new(Fixed(reading)),
        );
        assert_eq!(d.describe(), "gnss0 (local-reference)");
        assert_eq!(d.read(), Ok(reading));
        assert!(alloc::format!("{d:?}").contains("gnss0"));
    }
}
