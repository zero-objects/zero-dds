// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! One-stop security configuration facade for a participant.
//!
//! [`SecurityBundle`] groups the two things a participant needs to run with
//! DDS-Security observability — the access-control/crypto [`SecurityProfile`]
//! and the security-event [`LoggingPlugin`] — behind a single builder. It is
//! handed to the runtime via `RuntimeConfig::with_security_bundle`, which wires
//! the profile into `RuntimeConfig.security` and the logger into
//! `RuntimeConfig.security_logger`.
//!
//! ```
//! use zerodds_security_runtime::SecurityBundle;
//! use zerodds_security_logging::StderrLoggingPlugin;
//! use zerodds_security_runtime::LogLevel;
//!
//! let bundle = SecurityBundle::builder()
//!     .logging_plugin(Box::new(StderrLoggingPlugin::with_level(LogLevel::Warning)))
//!     .build();
//! assert!(bundle.has_logging());
//! ```

use alloc::boxed::Box;
use alloc::sync::Arc;

use crate::profile::SecurityProfile;
use zerodds_security::logging::LoggingPlugin;

/// Bundles the security configuration a participant needs — the
/// access-control/crypto [`SecurityProfile`] and the security-event
/// [`LoggingPlugin`] — behind one builder, ready to hand to a runtime config.
///
/// Build it with [`SecurityBundle::builder`] and apply it via
/// `RuntimeConfig::with_security_bundle` (in `zerodds-dcps`).
#[derive(Clone, Default)]
pub struct SecurityBundle {
    logging_plugin: Option<Arc<dyn LoggingPlugin>>,
    profile: Option<Arc<SecurityProfile>>,
}

impl SecurityBundle {
    /// Start building a bundle.
    #[must_use]
    pub fn builder() -> SecurityBundleBuilder {
        SecurityBundleBuilder::default()
    }

    /// The security-event logger, if one was configured. The returned `Arc`
    /// is exactly what `RuntimeConfig.security_logger` expects.
    #[must_use]
    pub fn logging_plugin(&self) -> Option<Arc<dyn LoggingPlugin>> {
        self.logging_plugin.clone()
    }

    /// The access-control/crypto profile, if one was configured.
    #[must_use]
    pub fn security_profile(&self) -> Option<Arc<SecurityProfile>> {
        self.profile.clone()
    }

    /// `true` if a security-event logger is configured.
    #[must_use]
    pub fn has_logging(&self) -> bool {
        self.logging_plugin.is_some()
    }

    /// `true` if an access-control/crypto profile is configured.
    #[must_use]
    pub fn has_profile(&self) -> bool {
        self.profile.is_some()
    }
}

/// Builder for [`SecurityBundle`]. Every setter is optional and chainable.
#[derive(Default)]
pub struct SecurityBundleBuilder {
    logging_plugin: Option<Arc<dyn LoggingPlugin>>,
    profile: Option<Arc<SecurityProfile>>,
}

impl SecurityBundleBuilder {
    /// Set the security-event logger. Accepts any boxed [`LoggingPlugin`] —
    /// e.g. a `FanOutLoggingPlugin` fanning out to stderr plus an audit file.
    #[must_use]
    pub fn logging_plugin(mut self, plugin: Box<dyn LoggingPlugin>) -> Self {
        self.logging_plugin = Some(Arc::from(plugin));
        self
    }

    /// Set the access-control/crypto profile, e.g. from
    /// [`SecurityProfile::from_files`].
    #[must_use]
    pub fn security_profile(mut self, profile: SecurityProfile) -> Self {
        self.profile = Some(Arc::new(profile));
        self
    }

    /// Finalize the bundle.
    #[must_use]
    pub fn build(self) -> SecurityBundle {
        SecurityBundle {
            logging_plugin: self.logging_plugin,
            profile: self.profile,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicUsize, Ordering};
    use zerodds_security::logging::LogLevel;

    #[derive(Default)]
    struct CountingLogger {
        calls: Arc<AtomicUsize>,
    }
    impl LoggingPlugin for CountingLogger {
        fn log(&self, _level: LogLevel, _participant: [u8; 16], _category: &str, _message: &str) {
            self.calls.fetch_add(1, Ordering::SeqCst);
        }
        fn plugin_class_id(&self) -> &str {
            "test:counting"
        }
    }

    #[test]
    fn empty_bundle_has_no_logging_or_profile() {
        let b = SecurityBundle::builder().build();
        assert!(!b.has_logging());
        assert!(!b.has_profile());
        assert!(b.logging_plugin().is_none());
    }

    #[test]
    fn bundle_carries_the_logger_and_forwards_log_calls() {
        let calls = Arc::new(AtomicUsize::new(0));
        let logger = CountingLogger {
            calls: Arc::clone(&calls),
        };
        let bundle = SecurityBundle::builder()
            .logging_plugin(Box::new(logger))
            .build();

        assert!(bundle.has_logging());
        let plugin = bundle.logging_plugin().expect("logger present");
        plugin.log(LogLevel::Warning, [0u8; 16], "auth", "test event");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn bundle_is_cloneable_and_shares_the_same_logger() {
        let calls = Arc::new(AtomicUsize::new(0));
        let bundle = SecurityBundle::builder()
            .logging_plugin(Box::new(CountingLogger {
                calls: Arc::clone(&calls),
            }))
            .build();
        let clone = bundle.clone();
        bundle
            .logging_plugin()
            .unwrap()
            .log(LogLevel::Error, [0u8; 16], "crypto", "a");
        clone
            .logging_plugin()
            .unwrap()
            .log(LogLevel::Error, [0u8; 16], "crypto", "b");
        // Both handles point at the same underlying logger.
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
