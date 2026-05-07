// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Data-Tagging-Plugin SPI (OMG DDS-Security 1.1 §8.7).
//!
//! Optional-Plugin fuer Application-Level-Labels (Classification-
//! Marker, Data-Sensitivity, etc.). Wird an jeden DataWriter/Reader
//! mitgeliefert und auf dem Wire via `DataTags`-Submessage propagiert.
//!
//! In v1.3 nur als Trait-Interface — produktive Nutzung kommt erst
//! mit NGVA/FACE-Integration in v2.0.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (Plugin-SPI benötigt `Box<dyn DataTaggingPlugin>`.)

extern crate alloc;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

/// Ein Tag = Name + Value-Paar (vergleichbar mit [`crate::Property`],
/// aber auf Application-Data-Level, nicht auf Participant-Config-Level).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTag {
    /// Tag-Name.
    pub name: String,
    /// Tag-Value.
    pub value: String,
}

/// Data-Tagging-Plugin (Spec §8.7.2).
pub trait DataTaggingPlugin: Send + Sync {
    /// Tags an einen Endpoint (DataWriter/Reader) anhaengen. Die Tags
    /// werden via SEDP an Remote-Participants propagiert.
    fn set_tags(&mut self, endpoint_guid: [u8; 16], tags: Vec<DataTag>);

    /// Tags eines Endpoints abfragen.
    fn get_tags(&self, endpoint_guid: [u8; 16]) -> Vec<DataTag>;

    /// Plugin-Class-Id.
    fn plugin_class_id(&self) -> &str;
}

/// Factory-Alias.
pub type DataTaggingBox = Box<dyn DataTaggingPlugin>;
