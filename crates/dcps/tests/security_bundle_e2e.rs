// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! E2E: a `SecurityBundle` built with a logger wires that logger into a
//! `RuntimeConfig` via `with_security_bundle`, and the wired logger actually
//! receives `log()` calls. This is the path the audit-logging docs show.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#![cfg(feature = "security")]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_security_runtime::{LogLevel, LoggingPlugin, SecurityBundle};

struct CapturingLogger {
    count: Arc<AtomicUsize>,
    last_category: std::sync::Mutex<String>,
}

impl LoggingPlugin for CapturingLogger {
    fn log(&self, _level: LogLevel, _participant: [u8; 16], category: &str, _message: &str) {
        self.count.fetch_add(1, Ordering::SeqCst);
        *self.last_category.lock().unwrap() = category.to_string();
    }
    fn plugin_class_id(&self) -> &str {
        "test:capturing"
    }
}

#[test]
fn bundle_logger_is_wired_into_runtime_config_and_fires() {
    let count = Arc::new(AtomicUsize::new(0));
    let logger = Arc::new(CapturingLogger {
        count: Arc::clone(&count),
        last_category: std::sync::Mutex::new(String::new()),
    });

    // Build the bundle from a boxed logger (the audit-logging-doc flow).
    let bundle = SecurityBundle::builder()
        .logging_plugin(Box::new(LoggerHandle(Arc::clone(&logger))))
        .build();
    assert!(bundle.has_logging());
    assert!(!bundle.has_profile());

    // Apply it to a RuntimeConfig.
    let cfg = RuntimeConfig::default().with_security_bundle(&bundle);
    assert!(
        cfg.security_logger.is_some(),
        "with_security_bundle must wire security_logger"
    );

    // The wired logger forwards real calls to the underlying logger.
    cfg.security_logger.as_ref().unwrap().log(
        LogLevel::Warning,
        [0u8; 16],
        "access_control",
        "permission denied",
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(
        logger.last_category.lock().unwrap().as_str(),
        "access_control"
    );
}

#[test]
fn empty_bundle_leaves_runtime_config_logger_unset() {
    let cfg = RuntimeConfig::default().with_security_bundle(&SecurityBundle::builder().build());
    assert!(cfg.security_logger.is_none());
}

/// Thin newtype so the test can share one `Arc<CapturingLogger>` between the
/// boxed-into-bundle logger and the assertion handle.
struct LoggerHandle(Arc<CapturingLogger>);
impl LoggingPlugin for LoggerHandle {
    fn log(&self, level: LogLevel, participant: [u8; 16], category: &str, message: &str) {
        self.0.log(level, participant, category, message);
    }
    fn plugin_class_id(&self) -> &str {
        self.0.plugin_class_id()
    }
}
