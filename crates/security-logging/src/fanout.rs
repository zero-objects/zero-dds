// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Fan-Out: ein Event an mehrere Backends gleichzeitig.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (Der Fan-Out haelt eine Liste polymorpher `LoggingPlugin`s via
//! `Box<dyn ...>` — architektur-bedingt, da der Nutzer verschiedene
//! Backend-Typen kombinieren koennen will.)

use alloc::boxed::Box;
use alloc::vec::Vec;

use zerodds_security::logging::{LogLevel, LoggingPlugin};

/// Fan-Out-Adapter — broadcast ein Event an alle eingetragenen
/// Backends. Nuetzlich fuer Setup mit `stderr` + audit-JSON-File
/// parallel.
pub struct FanOutLoggingPlugin {
    sinks: Vec<Box<dyn LoggingPlugin>>,
}

impl Default for FanOutLoggingPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl FanOutLoggingPlugin {
    /// Leerer Fan-Out — Events werden stillschweigend verworfen.
    #[must_use]
    pub fn new() -> Self {
        Self { sinks: Vec::new() }
    }

    /// Backend hinzufuegen. Builder-Style.
    #[must_use]
    pub fn with<P: LoggingPlugin + 'static>(mut self, sink: P) -> Self {
        self.sinks.push(Box::new(sink));
        self
    }

    /// Backend hinzufuegen per `Box<dyn ...>` (wenn Nutzer schon einen
    /// Box-Sink hat).
    #[must_use]
    pub fn with_boxed(mut self, sink: Box<dyn LoggingPlugin>) -> Self {
        self.sinks.push(sink);
        self
    }

    /// Anzahl registrierter Backends.
    #[must_use]
    pub fn sink_count(&self) -> usize {
        self.sinks.len()
    }
}

impl LoggingPlugin for FanOutLoggingPlugin {
    fn log(&self, level: LogLevel, participant: [u8; 16], category: &str, message: &str) {
        for sink in &self.sinks {
            sink.log(level, participant, category, message);
        }
    }

    fn plugin_class_id(&self) -> &str {
        "DDS:Logging:fanout"
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use zerodds_security::mock::{MockLogEntry, MockLogSink, MockLoggingPlugin};

    fn sink() -> MockLogSink {
        Arc::new(Mutex::new(Vec::<MockLogEntry>::new()))
    }

    #[test]
    fn empty_fanout_drops_events_without_panic() {
        let f = FanOutLoggingPlugin::new();
        f.log(LogLevel::Error, [0u8; 16], "cat", "msg");
        assert_eq!(f.sink_count(), 0);
    }

    #[test]
    fn event_reaches_all_registered_sinks() {
        let s1 = sink();
        let s2 = sink();
        let f = FanOutLoggingPlugin::new()
            .with(MockLoggingPlugin::new(Arc::clone(&s1)))
            .with(MockLoggingPlugin::new(Arc::clone(&s2)));
        f.log(LogLevel::Error, [0xAA; 16], "auth.bad", "nope");
        assert_eq!(s1.lock().unwrap().len(), 1);
        assert_eq!(s2.lock().unwrap().len(), 1);
        assert_eq!(s1.lock().unwrap()[0].category, "auth.bad");
    }

    #[test]
    fn plugin_class_id_stable() {
        assert_eq!(
            FanOutLoggingPlugin::new().plugin_class_id(),
            "DDS:Logging:fanout"
        );
    }
}
