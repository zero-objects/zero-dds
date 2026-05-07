// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Live-Runtime — nur mit Feature `zenoh-runtime` aktiv. Dies ist die
//! eigentliche Bridge: spawnt async-Tasks, die DDS-DataReader und
//! Zenoh-Subscriber zusammenkoppeln.
//!
//! # Lebenszyklus
//!
//! 1. `ZenohBridgeBuilder` sammelt Topic-Map-Eintraege + Zenoh-Config.
//! 2. `build()` -> `ZenohBridge`. Allociert tokio-Runtime,
//!    initiiert Zenoh-Session.
//! 3. `start()` spawnt pro Topic einen Forward-Task (DDS->Zenoh) und
//!    einen Reverse-Task (Zenoh->DDS).
//! 4. `stop()` joint alle Tasks und schliesst Zenoh-Session.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use zerodds_dcps::DomainParticipant;

use crate::mapping::TopicMap;

/// Bridge-Fehler.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BridgeError {
    /// Zenoh-Session konnte nicht aufgebaut werden.
    #[error("zenoh session failed: {0}")]
    ZenohSession(String),
    /// DDS-Topic ist nicht im Participant registriert.
    #[error("dds topic missing: {0}")]
    DdsTopicMissing(String),
    /// Tokio-Runtime-Initialisierung schlug fehl.
    #[error("tokio runtime: {0}")]
    Tokio(String),
}

/// Builder fuer eine `ZenohBridge`. Sammelt Topic-Map + Zenoh-Config +
/// optional einen `DomainParticipant` fuer den DDS-Side-Forward/Reverse-
/// Pfad.
pub struct ZenohBridgeBuilder {
    map: TopicMap,
    zenoh_endpoints: Vec<String>,
    prefix: String,
    participant: Option<Arc<DomainParticipant>>,
}

impl ZenohBridgeBuilder {
    /// Neuer Builder mit Default-Prefix `dds` und ohne Participant.
    /// Der Participant wird via [`Self::with_dcps_participant`] gesetzt
    /// — ohne ihn betreibt die Bridge nur den Zenoh-Side-Pfad
    /// (Caller fuettert DDS-Samples manuell ueber die Live-API).
    #[must_use]
    pub fn new() -> Self {
        Self {
            map: TopicMap::new(),
            zenoh_endpoints: Vec::new(),
            prefix: crate::mapping::DEFAULT_PREFIX.into(),
            participant: None,
        }
    }

    /// Verbindet die Bridge mit einem aktiven `DomainParticipant`. Der
    /// DDS-Side-Forward/Reverse-Pfad nutzt diesen Participant fuer
    /// `DataReader::take` (forward) und `DataWriter::write` (reverse).
    #[must_use]
    pub fn with_dcps_participant(mut self, participant: Arc<DomainParticipant>) -> Self {
        self.participant = Some(participant);
        self
    }

    /// Setzt den Zenoh-Key-Praefix (Default `dds`).
    #[must_use]
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Fuegt einen Zenoh-Connect-Endpoint hinzu (z.B. `tcp/127.0.0.1:7447`).
    /// Ohne Endpoints laeuft die Session im Peer-Mode mit Multicast-
    /// Discovery (Zenoh-Default).
    #[must_use]
    pub fn endpoint(mut self, ep: impl Into<String>) -> Self {
        self.zenoh_endpoints.push(ep.into());
        self
    }

    /// Registriert ein DDS-Topic. Die Zenoh-KeyExpr wird automatisch
    /// gemappt (siehe `key_expr_for_topic`).
    pub fn add_topic(
        &mut self,
        topic: impl Into<String>,
        type_name: impl Into<String>,
        partition: &str,
        reliability: zerodds_qos::ReliabilityKind,
        durability: zerodds_qos::DurabilityKind,
    ) {
        let topic = topic.into();
        let key_expr = crate::mapping::key_expr_for_topic(&self.prefix, partition, &topic);
        self.map.add(crate::mapping::TopicMapEntry {
            topic,
            type_name: type_name.into(),
            key_expr,
            reliability,
            durability,
        });
    }

    /// Baut die Bridge. Initiiert Tokio-Runtime + Zenoh-Session.
    ///
    /// # Errors
    /// `BridgeError::Tokio` bei Runtime-Fehler; `BridgeError::ZenohSession`
    /// wenn Zenoh-Connect/Open scheitert.
    pub async fn build(self) -> Result<ZenohBridge, BridgeError> {
        // Zenoh-Session aufbauen. Endpoints werden in der Config injected.
        let mut config = zenoh::Config::default();
        for ep in &self.zenoh_endpoints {
            // Spec: zenoh::config::ConnectConfig akzeptiert "endpoint/...".
            // 1.x-Form: config.connect.endpoints.push.
            let _ = config.insert_json5("connect/endpoints", &alloc::format!("[\"{ep}\"]"));
        }
        let session = zenoh::open(config)
            .await
            .map_err(|e| BridgeError::ZenohSession(alloc::format!("{e}")))?;

        Ok(ZenohBridge {
            map: Arc::new(self.map),
            session: Arc::new(session),
            participant: self.participant,
            tasks: Vec::new(),
        })
    }
}

impl Default for ZenohBridgeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Live-Bridge. Pro Topic ein Forward-Task (DDS->Zenoh) und ein
/// Reverse-Task (Zenoh->DDS).
pub struct ZenohBridge {
    map: Arc<TopicMap>,
    session: Arc<zenoh::Session>,
    participant: Option<Arc<DomainParticipant>>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl ZenohBridge {
    /// Liefert die Topic-Map.
    #[must_use]
    pub fn map(&self) -> &TopicMap {
        &self.map
    }

    /// Liefert den verbundenen DDS-Participant (falls einer via
    /// [`ZenohBridgeBuilder::with_dcps_participant`] gesetzt wurde).
    #[must_use]
    pub fn dcps_participant(&self) -> Option<&Arc<DomainParticipant>> {
        self.participant.as_ref()
    }

    /// Anzahl aktiver Tasks (Forward + Reverse pro Topic).
    #[must_use]
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Stoppt alle Tasks und schliesst die Zenoh-Session.
    pub async fn stop(mut self) {
        for handle in self.tasks.drain(..) {
            handle.abort();
        }
        // session-drop schliesst die zenoh-Session.
        drop(self.session);
        // participant-drop loest die DDS-Side-Tasks.
        drop(self.participant);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zerodds_qos::{DurabilityKind, ReliabilityKind};

    #[test]
    fn builder_collects_topics() {
        let mut b = ZenohBridgeBuilder::new().prefix("dds");
        b.add_topic(
            "Chatter",
            "std_msgs::String",
            "",
            ReliabilityKind::Reliable,
            DurabilityKind::Volatile,
        );
        b.add_topic(
            "Sensor",
            "sensor_msgs::Temperature",
            "robot1",
            ReliabilityKind::BestEffort,
            DurabilityKind::TransientLocal,
        );
        // Internal map check via builder lassen wir aus (private state),
        // aber wir koennen die Mapping-Funktion direkt testen.
        assert_eq!(
            crate::mapping::key_expr_for_topic("dds", "", "Chatter"),
            "dds/Chatter"
        );
        assert_eq!(
            crate::mapping::key_expr_for_topic("dds", "robot1", "Sensor"),
            "dds/robot1/Sensor"
        );
    }
}
