// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Helpers of the unit tests of the daemon modules.

use super::*;

/// One YAML text through the shared parser and this daemon's reader.
pub(crate) fn parse_yaml_subset(yaml: &str) -> Result<DaemonConfig, String> {
    let mut config = zerodds_config::Config::default();
    let node = zerodds_config::parse_str(yaml, "<string>").map_err(|e| e.to_string())?;
    config
        .push(node, "<string>".to_string())
        .map_err(|e| e.to_string())?;
    DaemonConfig::from_config(Some(&config)).map_err(|e| e.to_string())
}

/// The participant of `domain` with the topics of `cfg`, without a DDS
/// side.
pub(crate) fn participant(cfg: &DaemonConfig) -> Participant {
    Participant {
        tenant: None,
        subjects: Vec::new(),
        topics: cfg.topics.clone(),
        dds: None,
    }
}

pub(crate) fn topic(name: &str) -> TopicMapping {
    TopicMapping {
        dds_name: name.into(),
        dds_type: "T::Type".into(),
        grpc_service: "TStream".into(),
        ..TopicMapping::default()
    }
}
