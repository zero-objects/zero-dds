// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The audit records of the OMG Time Service administration (Time
//! Service 1.1 App. B.1: "They must also be mandatorily audited") as
//! JSON lines. Feature `time-service`.
//!
//! [`JsonLinesLoggingPlugin`] is the `AuditSink` of an administered
//! `zerodds_time_service::TimeService`: each record is one line with the
//! category `time.admin.<operation>` (`set-time`, `reset-time`,
//! `designate-source`, `set-inaccuracy`), level `NOTICE` for a granted
//! operation and `WARNING` for a denied or failed one, the participant
//! all zeros (the record belongs to no DDS participant), and the record
//! as message: `time=… subject=… op=… outcome=… before=… after=…`. The
//! line is written past the level filter and synchronized to the disk
//! before the operation takes effect
//! ([`JsonLinesLoggingPlugin::append_synced`]); a write error refuses the
//! operation. One file can hold the DDS-Security log and these records.

use alloc::string::ToString;

use zerodds_security::logging::LogLevel;
use zerodds_time_service::admin::{AuditError, AuditOutcome, AuditRecord, AuditSink};

use crate::JsonLinesLoggingPlugin;

/// The category of the line of `record`: `time.admin.<operation>`.
#[must_use]
pub fn category(record: &AuditRecord) -> alloc::string::String {
    alloc::format!("time.admin.{}", record.op.name())
}

impl AuditSink for JsonLinesLoggingPlugin {
    fn record(&self, record: &AuditRecord) -> Result<(), AuditError> {
        let level = match record.outcome {
            AuditOutcome::Granted => LogLevel::Notice,
            AuditOutcome::Denied | AuditOutcome::Failed => LogLevel::Warning,
        };
        self.append_synced(level, [0; 16], &category(record), &record.to_string())
            .map_err(|e| AuditError(e.to_string()))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::sync::Arc;
    use alloc::vec::Vec;
    use zerodds_security::logging::LoggingPlugin;
    use zerodds_time_service::TimeService;
    use zerodds_time_service::admin::{AdminError, AdminOp, AdminSubject};

    fn tmp_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(alloc::format!(
            "zerodds_time_audit_{}_{name}.jsonl",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn time_b_1_audit_record_reaches_the_jsonl_log() {
        let path = tmp_path("records");
        // The level filter of the security log does not hold back audit
        // records.
        let log = Arc::new(JsonLinesLoggingPlugin::open(&path, LogLevel::Emergency).unwrap());
        let (service, admin) = TimeService::with_source(|| 1_000_000)
            .with_tdf(0)
            .administered(
                Arc::new(|s: &AdminSubject, _: AdminOp| s.as_str() == "ops"),
                log.clone(),
            );
        admin
            .set_inaccuracy(&AdminSubject::new("ops"), 2_500)
            .unwrap();
        assert_eq!(
            admin.set_time(&AdminSubject::new("guest"), 5),
            Err(AdminError::Unauthorized)
        );
        // A DDS-Security event below the level stays filtered.
        log.log(LogLevel::Debug, [1; 16], "auth.debug", "not written");
        assert_eq!(service.inaccuracy(), 2_500);
        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2, "{content}");
        assert!(lines[0].contains(r#""level":"NOTICE""#), "{}", lines[0]);
        assert!(lines[0].contains(r#""category":"time.admin.set-inaccuracy""#));
        assert!(
            lines[0].contains("subject=ops op=set-inaccuracy outcome=granted before=0 after=2500"),
            "{}",
            lines[0]
        );
        assert!(lines[0].contains(r#""participant":"00000000000000000000000000000000""#));
        assert!(lines[1].contains(r#""level":"WARNING""#));
        assert!(lines[1].contains(r#""category":"time.admin.set-time""#));
        assert!(lines[1].contains("subject=guest op=set-time outcome=denied"));
        let _ = std::fs::remove_file(&path);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn time_b_1_a_full_audit_log_refuses_the_operation() {
        // /dev/full: every write fails with ENOSPC.
        let log = Arc::new(JsonLinesLoggingPlugin::open("/dev/full", LogLevel::Debug).unwrap());
        let (service, admin) = TimeService::with_source(|| 1_000_000)
            .administered(Arc::new(|_: &AdminSubject, _: AdminOp| true), log);
        match admin.set_inaccuracy(&AdminSubject::new("ops"), 9) {
            Err(AdminError::AuditFailed(e)) => assert!(!e.0.is_empty()),
            other => panic!("{other:?}"),
        }
        assert_eq!(service.inaccuracy(), 0);
    }
}
