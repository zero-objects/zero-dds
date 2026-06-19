// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! UADP framing (OPC Foundation Part 14 §7.2.2) — the publish/subscribe
//! wire format built on top of the Part 6 binary codec ([`crate::binary`]).
//!
//! A UADP `NetworkMessage` carries one or more [`DataSetMessage`]s, each
//! produced by a `DataSetWriter`. This module builds the framing bottom-up:
//!
//! - [`dataset_message`] — a single DataSet sample ([`DataSetMessage`]).
//! - [`network_message`] — the outer [`NetworkMessage`] that frames one or
//!   more DataSetMessages on the wire.
//! - [`discovery`] — the DiscoveryRequest/Response messages and the
//!   `DataSetMetaData` wire codec.

pub mod dataset_message;
pub mod datatypes;
pub mod discovery;
pub mod network_message;

pub use dataset_message::{DataSetData, DataSetMessage, DataSetMessageKind, FieldEncoding};
pub use datatypes::{
    ApplicationDescription, ApplicationType, DataSetWriterDataType, EndpointDescription,
    MessageSecurityMode, UserTokenPolicy, UserTokenType, WriterGroupDataType,
};
pub use discovery::{
    DataSetMetaDataResponse, DataSetWriterConfigurationResponse, DiscoveryRequest,
    DiscoveryResponse, DiscoveryResponsePayload, InformationType, PublisherEndpointsResponse,
};
pub use network_message::{GroupHeader, NetworkMessage, PublisherId};
