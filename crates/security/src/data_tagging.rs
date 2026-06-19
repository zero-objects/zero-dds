// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Data tagging plugin SPI (OMG DDS-Security 1.1 §8.7).
//!
//! Optional plugin for application-level labels (classification
//! markers, data sensitivity, etc.). Attached to every DataWriter/Reader
//! and propagated on the wire via the `DataTags` submessage.
//!
//! In v1.3 only as a trait interface — production use only comes
//! with NGVA/FACE integration in v2.0.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (The plugin SPI needs `Box<dyn DataTaggingPlugin>`.)

extern crate alloc;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

/// A tag = name + value pair (comparable to [`crate::Property`],
/// but at application-data level, not at participant-config level).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTag {
    /// Tag name.
    pub name: String,
    /// Tag value.
    pub value: String,
}

/// Data tagging plugin (spec §8.7.2).
pub trait DataTaggingPlugin: Send + Sync {
    /// Attach tags to an endpoint (DataWriter/Reader). The tags
    /// are propagated to remote participants via SEDP.
    fn set_tags(&mut self, endpoint_guid: [u8; 16], tags: Vec<DataTag>);

    /// Query the tags of an endpoint.
    fn get_tags(&self, endpoint_guid: [u8; 16]) -> Vec<DataTag>;

    /// Plugin class id.
    fn plugin_class_id(&self) -> &str;
}

/// Factory alias.
pub type DataTaggingBox = Box<dyn DataTaggingPlugin>;
