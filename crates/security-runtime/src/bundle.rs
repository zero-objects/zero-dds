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

// The security-event logger is a runtime plug-in boundary; `Arc/Box<dyn LoggingPlugin>` is the intended heterogeneous sink type (mirrors RuntimeConfig.security_logger).
// zerodds-lint: allow no_dyn_in_safe

use alloc::boxed::Box;
use alloc::sync::Arc;
use std::sync::Mutex;

use crate::profile::SecurityProfile;
use zerodds_security::data_tagging::DataTaggingPlugin;
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
    data_tagging_plugin: Option<Arc<Mutex<dyn DataTaggingPlugin>>>,
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

    /// The DataTagging plugin, if one was configured (DDS-Security 1.2
    /// §9.7, §10.1). The returned `Arc` is what
    /// `RuntimeConfig.security_data_tagging` expects.
    #[must_use]
    pub fn data_tagging_plugin(&self) -> Option<Arc<Mutex<dyn DataTaggingPlugin>>> {
        self.data_tagging_plugin.clone()
    }

    /// `true` if a DataTagging plugin is configured.
    #[must_use]
    pub fn has_data_tagging(&self) -> bool {
        self.data_tagging_plugin.is_some()
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

/// A boxed [`DataTaggingPlugin`] as an unsized `Mutex` payload.
struct PluginBox(Box<dyn DataTaggingPlugin>);

impl DataTaggingPlugin for PluginBox {
    fn set_tags(
        &mut self,
        endpoint_guid: [u8; 16],
        tags: alloc::vec::Vec<zerodds_security::data_tagging::DataTag>,
    ) {
        self.0.set_tags(endpoint_guid, tags);
    }

    fn get_tags(
        &self,
        endpoint_guid: [u8; 16],
    ) -> alloc::vec::Vec<zerodds_security::data_tagging::DataTag> {
        self.0.get_tags(endpoint_guid)
    }

    fn plugin_class_id(&self) -> &str {
        self.0.plugin_class_id()
    }
}

/// Builder for [`SecurityBundle`]. Every setter is optional and chainable.
#[derive(Default)]
pub struct SecurityBundleBuilder {
    logging_plugin: Option<Arc<dyn LoggingPlugin>>,
    data_tagging_plugin: Option<Arc<Mutex<dyn DataTaggingPlugin>>>,
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

    /// Set the DataTagging plugin, e.g. the builtin
    /// `DDS:Tagging:DDS_Discovery`
    /// ([`BuiltinDataTaggingPlugin`](crate::BuiltinDataTaggingPlugin)). The
    /// runtime registers the `data_tags` of every endpoint it creates with
    /// the plugin and announces the tags the plugin returns for it.
    #[must_use]
    pub fn data_tagging_plugin(mut self, plugin: Box<dyn DataTaggingPlugin>) -> Self {
        self.data_tagging_plugin = Some(Arc::new(Mutex::new(PluginBox(plugin))));
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
            data_tagging_plugin: self.data_tagging_plugin,
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
        // Test double: it records `log` calls only; the Table 46 operations
        // have nothing to configure.
        fn set_log_options(
            &self,
            _options: zerodds_security::logging::LogOptions,
        ) -> Result<(), zerodds_security::error::SecurityError> {
            Ok(())
        }
        fn enable_logging(&self) -> Result<(), zerodds_security::error::SecurityError> {
            Ok(())
        }
        fn set_listener(
            &self,
            _listener: Option<std::sync::Arc<dyn zerodds_security::logging::LoggerListener>>,
        ) -> Result<(), zerodds_security::error::SecurityError> {
            Ok(())
        }
        fn log(&self, _level: LogLevel, _participant: [u8; 16], _category: &str, _message: &str) {
            self.calls.fetch_add(1, Ordering::SeqCst);
        }
        fn plugin_class_id(&self) -> &str {
            "test:counting"
        }
    }

    /// DDS-Security 1.2 §10.1 Table 49: the DataTagging plugin is chosen on
    /// the bundle; the shared plugin keeps the tags set through it.
    #[test]
    fn a_bundle_carries_the_data_tagging_plugin() {
        use zerodds_security::data_tagging::DataTag;
        let b = SecurityBundle::builder()
            .data_tagging_plugin(Box::new(crate::BuiltinDataTaggingPlugin::new()))
            .build();
        assert!(b.has_data_tagging());
        assert!(!SecurityBundle::builder().build().has_data_tagging());
        let plugin = b.data_tagging_plugin().expect("plugin");
        let tag = DataTag {
            name: "classification".into(),
            value: "secret".into(),
        };
        plugin
            .lock()
            .expect("lock")
            .set_tags([7; 16], vec![tag.clone()]);
        // A clone of the bundle shares the plugin.
        let again = b.clone().data_tagging_plugin().expect("plugin");
        let guard = again.lock().expect("lock");
        assert_eq!(guard.get_tags([7; 16]), vec![tag]);
        assert_eq!(guard.plugin_class_id(), "DDS:Tagging:DDS_Discovery");
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
