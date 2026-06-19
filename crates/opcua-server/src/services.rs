// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! OPC-UA service messages (Part 4 §5): Session lifecycle (CreateSession /
//! ActivateSession / CloseSession) plus the Read and Call service sets, with
//! the [`ServiceRequest`] / [`ServiceResponse`] dispatch enums.
//!
//! Each message body is `[TypeId:NodeId][RequestHeader|ResponseHeader][fields]`
//! per Part 6 §5; the common headers come from [`zerodds_opcua_uacp`].

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_opcua_gateway::data_value::{DataValue, ExtensionObject, ExtensionObjectBody, Variant};
use zerodds_opcua_gateway::node_id::{ExpandedNodeId, NodeId};
use zerodds_opcua_gateway::types::{LocalizedText, QualifiedName};
use zerodds_opcua_pubsub::uadp::datatypes::{ApplicationDescription, EndpointDescription};
use zerodds_opcua_pubsub::{DecodeError, EncodeError, UaDecode, UaEncode, UaReader, UaWriter};

use zerodds_opcua_uacp::securechannel::node_ids;
use zerodds_opcua_uacp::securechannel::{RequestHeader, ResponseHeader, null_extension_object};

use crate::wire::{
    read_array, read_byte_string, read_string, read_string_array, read_u32_array,
    skip_diagnostic_info_array, write_array, write_byte_string, write_empty_diagnostic_info_array,
    write_string, write_string_array, write_u32_array,
};

/// The `Value` attribute id (Part 4 §A.1).
pub const ATTRIBUTE_VALUE: u32 = 13;

// ---------------------------------------------------------------------------
// Shared sub-types.
// ---------------------------------------------------------------------------

/// `SignatureData` (Part 4 §7.36) — empty for SecurityMode `None`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SignatureData {
    /// Signature algorithm URI.
    pub algorithm: String,
    /// Signature bytes.
    pub signature: Vec<u8>,
}

impl UaEncode for SignatureData {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        write_string(w, &self.algorithm)?;
        write_byte_string(w, &self.signature)?;
        Ok(())
    }
}

impl UaDecode for SignatureData {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            algorithm: read_string(r)?,
            signature: read_byte_string(r)?,
        })
    }
}

/// `ReadValueId` (Part 4 §7.28).
#[derive(Debug, Clone, PartialEq)]
pub struct ReadValueId {
    /// Node to read.
    pub node_id: NodeId,
    /// Attribute id (e.g. [`ATTRIBUTE_VALUE`]).
    pub attribute_id: u32,
    /// Index range (empty = whole value).
    pub index_range: String,
    /// Requested data encoding (empty QualifiedName = default).
    pub data_encoding: QualifiedName,
}

impl UaEncode for ReadValueId {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        self.node_id.encode(w)?;
        w.write_u32(self.attribute_id);
        write_string(w, &self.index_range)?;
        self.data_encoding.encode(w)?;
        Ok(())
    }
}

impl UaDecode for ReadValueId {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            node_id: NodeId::decode(r)?,
            attribute_id: r.read_u32()?,
            index_range: read_string(r)?,
            data_encoding: QualifiedName::decode(r)?,
        })
    }
}

/// `WriteValue` (Part 4 §5.11.4) — a single node/attribute write.
#[derive(Debug, Clone, PartialEq)]
pub struct WriteValue {
    /// Node to write.
    pub node_id: NodeId,
    /// Attribute id (e.g. [`ATTRIBUTE_VALUE`]).
    pub attribute_id: u32,
    /// Index range (empty = whole value).
    pub index_range: String,
    /// The value to write.
    pub value: DataValue,
}

impl UaEncode for WriteValue {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        self.node_id.encode(w)?;
        w.write_u32(self.attribute_id);
        write_string(w, &self.index_range)?;
        self.value.encode(w)?;
        Ok(())
    }
}

impl UaDecode for WriteValue {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            node_id: NodeId::decode(r)?,
            attribute_id: r.read_u32()?,
            index_range: read_string(r)?,
            value: DataValue::decode(r)?,
        })
    }
}

/// `CallMethodRequest` (Part 4 §5.12.2).
#[derive(Debug, Clone, PartialEq)]
pub struct CallMethodRequest {
    /// Object the method belongs to.
    pub object_id: NodeId,
    /// Method to call.
    pub method_id: NodeId,
    /// Input argument values.
    pub input_arguments: Vec<Variant>,
}

impl UaEncode for CallMethodRequest {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        self.object_id.encode(w)?;
        self.method_id.encode(w)?;
        write_array(w, &self.input_arguments, "InputArguments")?;
        Ok(())
    }
}

impl UaDecode for CallMethodRequest {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            object_id: NodeId::decode(r)?,
            method_id: NodeId::decode(r)?,
            input_arguments: read_array::<Variant>(r)?,
        })
    }
}

/// `CallMethodResult` (Part 4 §5.12.2).
#[derive(Debug, Clone, PartialEq)]
pub struct CallMethodResult {
    /// Method call status.
    pub status_code: u32,
    /// Per-input-argument status codes.
    pub input_argument_results: Vec<u32>,
    /// Output argument values.
    pub output_arguments: Vec<Variant>,
}

impl UaEncode for CallMethodResult {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        w.write_u32(self.status_code);
        write_u32_array(w, &self.input_argument_results)?;
        write_empty_diagnostic_info_array(w); // inputArgumentDiagnosticInfos
        write_array(w, &self.output_arguments, "OutputArguments")?;
        Ok(())
    }
}

impl UaDecode for CallMethodResult {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let status_code = r.read_u32()?;
        let input_argument_results = read_u32_array(r)?;
        skip_diagnostic_info_array(r)?;
        let output_arguments = read_array::<Variant>(r)?;
        Ok(Self {
            status_code,
            input_argument_results,
            output_arguments,
        })
    }
}

// ---------------------------------------------------------------------------
// CreateSession (Part 4 §5.7.2).
// ---------------------------------------------------------------------------

/// `CreateSessionRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateSessionRequest {
    /// Common request header.
    pub request_header: RequestHeader,
    /// Client application description.
    pub client_description: ApplicationDescription,
    /// Server URI.
    pub server_uri: String,
    /// Endpoint URL.
    pub endpoint_url: String,
    /// Human-readable session name.
    pub session_name: String,
    /// Client nonce.
    pub client_nonce: Vec<u8>,
    /// Client certificate (DER).
    pub client_certificate: Vec<u8>,
    /// Requested session timeout in milliseconds.
    pub requested_session_timeout: f64,
    /// Maximum response message size.
    pub max_response_message_size: u32,
}

impl CreateSessionRequest {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            request_header: decode_request_header(r)?,
            client_description: ApplicationDescription::decode(r)?,
            server_uri: read_string(r)?,
            endpoint_url: read_string(r)?,
            session_name: read_string(r)?,
            client_nonce: read_byte_string(r)?,
            client_certificate: read_byte_string(r)?,
            requested_session_timeout: r.read_f64()?,
            max_response_message_size: r.read_u32()?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::CREATE_SESSION_REQUEST.encode(w)?;
        encode_request_header(&self.request_header, w)?;
        self.client_description.encode(w)?;
        write_string(w, &self.server_uri)?;
        write_string(w, &self.endpoint_url)?;
        write_string(w, &self.session_name)?;
        write_byte_string(w, &self.client_nonce)?;
        write_byte_string(w, &self.client_certificate)?;
        w.write_f64(self.requested_session_timeout);
        w.write_u32(self.max_response_message_size);
        Ok(())
    }
}

/// `CreateSessionResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateSessionResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
    /// Session identifier.
    pub session_id: NodeId,
    /// Authentication token for subsequent requests.
    pub authentication_token: NodeId,
    /// Revised session timeout in milliseconds.
    pub revised_session_timeout: f64,
    /// Server nonce.
    pub server_nonce: Vec<u8>,
    /// Server certificate (DER).
    pub server_certificate: Vec<u8>,
    /// Endpoints the server offers.
    pub server_endpoints: Vec<EndpointDescription>,
    /// Server signature over the client nonce+cert (empty for `None`).
    pub server_signature: SignatureData,
    /// Maximum request message size.
    pub max_request_message_size: u32,
}

impl CreateSessionResponse {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let response_header = decode_response_header(r)?;
        let session_id = NodeId::decode(r)?;
        let authentication_token = NodeId::decode(r)?;
        let revised_session_timeout = r.read_f64()?;
        let server_nonce = read_byte_string(r)?;
        let server_certificate = read_byte_string(r)?;
        let server_endpoints = read_array::<EndpointDescription>(r)?;
        // serverSoftwareCertificates (empty array).
        let _swc = r.read_i32()?;
        let server_signature = SignatureData::decode(r)?;
        let max_request_message_size = r.read_u32()?;
        Ok(Self {
            response_header,
            session_id,
            authentication_token,
            revised_session_timeout,
            server_nonce,
            server_certificate,
            server_endpoints,
            server_signature,
            max_request_message_size,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::CREATE_SESSION_RESPONSE.encode(w)?;
        encode_response_header(&self.response_header, w)?;
        self.session_id.encode(w)?;
        self.authentication_token.encode(w)?;
        w.write_f64(self.revised_session_timeout);
        write_byte_string(w, &self.server_nonce)?;
        write_byte_string(w, &self.server_certificate)?;
        write_array(w, &self.server_endpoints, "ServerEndpoints")?;
        w.write_i32(0); // serverSoftwareCertificates: empty
        self.server_signature.encode(w)?;
        w.write_u32(self.max_request_message_size);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ActivateSession (Part 4 §5.7.3).
// ---------------------------------------------------------------------------

/// `ActivateSessionRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct ActivateSessionRequest {
    /// Common request header (carries the authentication token).
    pub request_header: RequestHeader,
    /// Client signature (empty for `None`).
    pub client_signature: SignatureData,
    /// Preferred locales.
    pub locale_ids: Vec<String>,
    /// User identity token (an ExtensionObject; anonymous for the demo).
    pub user_identity_token: zerodds_opcua_gateway::data_value::ExtensionObject,
    /// User token signature (empty for `None`).
    pub user_token_signature: SignatureData,
}

impl ActivateSessionRequest {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let request_header = decode_request_header(r)?;
        let client_signature = SignatureData::decode(r)?;
        // clientSoftwareCertificates (empty array).
        let _swc = r.read_i32()?;
        let locale_ids = read_string_array(r)?;
        let user_identity_token = zerodds_opcua_gateway::data_value::ExtensionObject::decode(r)?;
        let user_token_signature = SignatureData::decode(r)?;
        Ok(Self {
            request_header,
            client_signature,
            locale_ids,
            user_identity_token,
            user_token_signature,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::ACTIVATE_SESSION_REQUEST.encode(w)?;
        encode_request_header(&self.request_header, w)?;
        self.client_signature.encode(w)?;
        w.write_i32(0); // clientSoftwareCertificates: empty
        write_string_array(w, &self.locale_ids)?;
        self.user_identity_token.encode(w)?;
        self.user_token_signature.encode(w)?;
        Ok(())
    }
}

/// `ActivateSessionResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct ActivateSessionResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
    /// New server nonce.
    pub server_nonce: Vec<u8>,
    /// Per-certificate results.
    pub results: Vec<u32>,
}

impl ActivateSessionResponse {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let response_header = decode_response_header(r)?;
        let server_nonce = read_byte_string(r)?;
        let results = read_u32_array(r)?;
        skip_diagnostic_info_array(r)?;
        Ok(Self {
            response_header,
            server_nonce,
            results,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::ACTIVATE_SESSION_RESPONSE.encode(w)?;
        encode_response_header(&self.response_header, w)?;
        write_byte_string(w, &self.server_nonce)?;
        write_u32_array(w, &self.results)?;
        write_empty_diagnostic_info_array(w);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// CloseSession (Part 4 §5.7.4).
// ---------------------------------------------------------------------------

/// `CloseSessionRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct CloseSessionRequest {
    /// Common request header.
    pub request_header: RequestHeader,
    /// Whether to delete the session's subscriptions.
    pub delete_subscriptions: bool,
}

impl CloseSessionRequest {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            request_header: decode_request_header(r)?,
            delete_subscriptions: r.read_u8()? != 0,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::CLOSE_SESSION_REQUEST.encode(w)?;
        encode_request_header(&self.request_header, w)?;
        w.write_u8(u8::from(self.delete_subscriptions));
        Ok(())
    }
}

/// `CloseSessionResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct CloseSessionResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
}

impl CloseSessionResponse {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            response_header: decode_response_header(r)?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::CLOSE_SESSION_RESPONSE.encode(w)?;
        encode_response_header(&self.response_header, w)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Read (Part 4 §5.11.2).
// ---------------------------------------------------------------------------

/// `ReadRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadRequest {
    /// Common request header.
    pub request_header: RequestHeader,
    /// Maximum acceptable value age in milliseconds.
    pub max_age: f64,
    /// TimestampsToReturn (0 Source / 1 Server / 2 Both / 3 Neither).
    pub timestamps_to_return: i32,
    /// Nodes/attributes to read.
    pub nodes_to_read: Vec<ReadValueId>,
}

impl ReadRequest {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            request_header: decode_request_header(r)?,
            max_age: r.read_f64()?,
            timestamps_to_return: r.read_i32()?,
            nodes_to_read: read_array::<ReadValueId>(r)?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::READ_REQUEST.encode(w)?;
        encode_request_header(&self.request_header, w)?;
        w.write_f64(self.max_age);
        w.write_i32(self.timestamps_to_return);
        write_array(w, &self.nodes_to_read, "NodesToRead")?;
        Ok(())
    }
}

/// `ReadResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
    /// One DataValue per requested node.
    pub results: Vec<DataValue>,
}

impl ReadResponse {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let response_header = decode_response_header(r)?;
        let results = read_array::<DataValue>(r)?;
        skip_diagnostic_info_array(r)?;
        Ok(Self {
            response_header,
            results,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::READ_RESPONSE.encode(w)?;
        encode_response_header(&self.response_header, w)?;
        write_array(w, &self.results, "Results")?;
        write_empty_diagnostic_info_array(w);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Write (Part 4 §5.11.4).
// ---------------------------------------------------------------------------

/// `WriteRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct WriteRequest {
    /// Common request header.
    pub request_header: RequestHeader,
    /// Nodes/attributes to write.
    pub nodes_to_write: Vec<WriteValue>,
}

impl WriteRequest {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            request_header: decode_request_header(r)?,
            nodes_to_write: read_array::<WriteValue>(r)?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::WRITE_REQUEST.encode(w)?;
        encode_request_header(&self.request_header, w)?;
        write_array(w, &self.nodes_to_write, "NodesToWrite")?;
        Ok(())
    }
}

/// `WriteResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct WriteResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
    /// One StatusCode per requested write.
    pub results: Vec<u32>,
}

impl WriteResponse {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let response_header = decode_response_header(r)?;
        let results = read_u32_array(r)?;
        skip_diagnostic_info_array(r)?;
        Ok(Self {
            response_header,
            results,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::WRITE_RESPONSE.encode(w)?;
        encode_response_header(&self.response_header, w)?;
        write_u32_array(w, &self.results)?;
        write_empty_diagnostic_info_array(w);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Call (Part 4 §5.12.2).
// ---------------------------------------------------------------------------

/// `CallRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct CallRequest {
    /// Common request header.
    pub request_header: RequestHeader,
    /// Methods to call.
    pub methods_to_call: Vec<CallMethodRequest>,
}

impl CallRequest {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            request_header: decode_request_header(r)?,
            methods_to_call: read_array::<CallMethodRequest>(r)?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::CALL_REQUEST.encode(w)?;
        encode_request_header(&self.request_header, w)?;
        write_array(w, &self.methods_to_call, "MethodsToCall")?;
        Ok(())
    }
}

/// `CallResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct CallResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
    /// One result per method call.
    pub results: Vec<CallMethodResult>,
}

impl CallResponse {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let response_header = decode_response_header(r)?;
        let results = read_array::<CallMethodResult>(r)?;
        skip_diagnostic_info_array(r)?;
        Ok(Self {
            response_header,
            results,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::CALL_RESPONSE.encode(w)?;
        encode_response_header(&self.response_header, w)?;
        write_array(w, &self.results, "Results")?;
        write_empty_diagnostic_info_array(w);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Browse (Part 4 §5.9.2).
// ---------------------------------------------------------------------------

/// `ViewDescription` (Part 4 §7.44) — a null view browses the whole address
/// space.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewDescription {
    /// View NodeId (null = no view).
    pub view_id: NodeId,
    /// Timestamp (DateTime ticks; `0` = any).
    pub timestamp: i64,
    /// View version (`0` = any).
    pub view_version: u32,
}

impl Default for ViewDescription {
    fn default() -> Self {
        Self {
            view_id: NodeId::numeric(0, 0),
            timestamp: 0,
            view_version: 0,
        }
    }
}

impl UaEncode for ViewDescription {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        self.view_id.encode(w)?;
        w.write_i64(self.timestamp);
        w.write_u32(self.view_version);
        Ok(())
    }
}

impl UaDecode for ViewDescription {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            view_id: NodeId::decode(r)?,
            timestamp: r.read_i64()?,
            view_version: r.read_u32()?,
        })
    }
}

/// `BrowseDescription` (Part 4 §7) — one node to browse.
#[derive(Debug, Clone, PartialEq)]
pub struct BrowseDescription {
    /// Node to browse.
    pub node_id: NodeId,
    /// BrowseDirection (0 Forward / 1 Inverse / 2 Both).
    pub browse_direction: i32,
    /// ReferenceType filter (null = all).
    pub reference_type_id: NodeId,
    /// Include subtypes of the reference type.
    pub include_subtypes: bool,
    /// NodeClass mask (0 = all).
    pub node_class_mask: u32,
    /// Result mask (which `ReferenceDescription` fields to fill).
    pub result_mask: u32,
}

impl UaEncode for BrowseDescription {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        self.node_id.encode(w)?;
        w.write_i32(self.browse_direction);
        self.reference_type_id.encode(w)?;
        w.write_u8(u8::from(self.include_subtypes));
        w.write_u32(self.node_class_mask);
        w.write_u32(self.result_mask);
        Ok(())
    }
}

impl UaDecode for BrowseDescription {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            node_id: NodeId::decode(r)?,
            browse_direction: r.read_i32()?,
            reference_type_id: NodeId::decode(r)?,
            include_subtypes: r.read_u8()? != 0,
            node_class_mask: r.read_u32()?,
            result_mask: r.read_u32()?,
        })
    }
}

/// `ReferenceDescription` (Part 4 §7.29) — one browse result entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceDescription {
    /// ReferenceType traversed.
    pub reference_type_id: NodeId,
    /// `true` for a forward reference.
    pub is_forward: bool,
    /// Target node.
    pub node_id: ExpandedNodeId,
    /// Target BrowseName.
    pub browse_name: QualifiedName,
    /// Target DisplayName.
    pub display_name: LocalizedText,
    /// Target NodeClass (enum value).
    pub node_class: i32,
    /// Target TypeDefinition.
    pub type_definition: ExpandedNodeId,
}

impl UaEncode for ReferenceDescription {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        self.reference_type_id.encode(w)?;
        w.write_u8(u8::from(self.is_forward));
        self.node_id.encode(w)?;
        self.browse_name.encode(w)?;
        self.display_name.encode(w)?;
        w.write_i32(self.node_class);
        self.type_definition.encode(w)?;
        Ok(())
    }
}

impl UaDecode for ReferenceDescription {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            reference_type_id: NodeId::decode(r)?,
            is_forward: r.read_u8()? != 0,
            node_id: ExpandedNodeId::decode(r)?,
            browse_name: QualifiedName::decode(r)?,
            display_name: LocalizedText::decode(r)?,
            node_class: r.read_i32()?,
            type_definition: ExpandedNodeId::decode(r)?,
        })
    }
}

/// `BrowseResult` (Part 4 §7.6) — the references found for one browsed node.
#[derive(Debug, Clone, PartialEq)]
pub struct BrowseResult {
    /// Per-node browse status.
    pub status_code: u32,
    /// Continuation point (empty = none).
    pub continuation_point: Vec<u8>,
    /// The references found.
    pub references: Vec<ReferenceDescription>,
}

impl UaEncode for BrowseResult {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        w.write_u32(self.status_code);
        write_byte_string(w, &self.continuation_point)?;
        write_array(w, &self.references, "References")?;
        Ok(())
    }
}

impl UaDecode for BrowseResult {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            status_code: r.read_u32()?,
            continuation_point: read_byte_string(r)?,
            references: read_array::<ReferenceDescription>(r)?,
        })
    }
}

/// `BrowseRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct BrowseRequest {
    /// Common request header.
    pub request_header: RequestHeader,
    /// View to browse (null = whole address space).
    pub view: ViewDescription,
    /// Max references per node (`0` = no limit).
    pub requested_max_references_per_node: u32,
    /// Nodes to browse.
    pub nodes_to_browse: Vec<BrowseDescription>,
}

impl BrowseRequest {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            request_header: decode_request_header(r)?,
            view: ViewDescription::decode(r)?,
            requested_max_references_per_node: r.read_u32()?,
            nodes_to_browse: read_array::<BrowseDescription>(r)?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::BROWSE_REQUEST.encode(w)?;
        encode_request_header(&self.request_header, w)?;
        self.view.encode(w)?;
        w.write_u32(self.requested_max_references_per_node);
        write_array(w, &self.nodes_to_browse, "NodesToBrowse")?;
        Ok(())
    }
}

/// `BrowseResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct BrowseResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
    /// One result per browsed node.
    pub results: Vec<BrowseResult>,
}

impl BrowseResponse {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let response_header = decode_response_header(r)?;
        let results = read_array::<BrowseResult>(r)?;
        skip_diagnostic_info_array(r)?;
        Ok(Self {
            response_header,
            results,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::BROWSE_RESPONSE.encode(w)?;
        encode_response_header(&self.response_header, w)?;
        write_array(w, &self.results, "Results")?;
        write_empty_diagnostic_info_array(w);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Discovery: GetEndpoints (Part 4 §5.5.4) + FindServers (§5.5.2).
// ---------------------------------------------------------------------------

/// `GetEndpointsRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct GetEndpointsRequest {
    /// Common request header.
    pub request_header: RequestHeader,
    /// Endpoint URL the client used.
    pub endpoint_url: String,
    /// Preferred locales.
    pub locale_ids: Vec<String>,
    /// Transport profile URIs filter (empty = all).
    pub profile_uris: Vec<String>,
}

impl GetEndpointsRequest {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            request_header: decode_request_header(r)?,
            endpoint_url: read_string(r)?,
            locale_ids: read_string_array(r)?,
            profile_uris: read_string_array(r)?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::GET_ENDPOINTS_REQUEST.encode(w)?;
        encode_request_header(&self.request_header, w)?;
        write_string(w, &self.endpoint_url)?;
        write_string_array(w, &self.locale_ids)?;
        write_string_array(w, &self.profile_uris)?;
        Ok(())
    }
}

/// `GetEndpointsResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct GetEndpointsResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
    /// The endpoints the server offers.
    pub endpoints: Vec<EndpointDescription>,
}

impl GetEndpointsResponse {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            response_header: decode_response_header(r)?,
            endpoints: read_array::<EndpointDescription>(r)?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::GET_ENDPOINTS_RESPONSE.encode(w)?;
        encode_response_header(&self.response_header, w)?;
        write_array(w, &self.endpoints, "Endpoints")?;
        Ok(())
    }
}

/// `FindServersRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct FindServersRequest {
    /// Common request header.
    pub request_header: RequestHeader,
    /// Endpoint URL the client used.
    pub endpoint_url: String,
    /// Preferred locales.
    pub locale_ids: Vec<String>,
    /// Server URIs filter (empty = all).
    pub server_uris: Vec<String>,
}

impl FindServersRequest {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            request_header: decode_request_header(r)?,
            endpoint_url: read_string(r)?,
            locale_ids: read_string_array(r)?,
            server_uris: read_string_array(r)?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::FIND_SERVERS_REQUEST.encode(w)?;
        encode_request_header(&self.request_header, w)?;
        write_string(w, &self.endpoint_url)?;
        write_string_array(w, &self.locale_ids)?;
        write_string_array(w, &self.server_uris)?;
        Ok(())
    }
}

/// `FindServersResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct FindServersResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
    /// The servers known at this endpoint.
    pub servers: Vec<ApplicationDescription>,
}

impl FindServersResponse {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            response_header: decode_response_header(r)?,
            servers: read_array::<ApplicationDescription>(r)?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::FIND_SERVERS_RESPONSE.encode(w)?;
        encode_response_header(&self.response_header, w)?;
        write_array(w, &self.servers, "Servers")?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Subscription / MonitoredItem (Part 4 §5.13 / §5.14).
// ---------------------------------------------------------------------------

/// `CreateSubscriptionRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateSubscriptionRequest {
    /// Common request header.
    pub request_header: RequestHeader,
    /// Requested publishing interval in milliseconds.
    pub requested_publishing_interval: f64,
    /// Requested lifetime count.
    pub requested_lifetime_count: u32,
    /// Requested max keep-alive count.
    pub requested_max_keep_alive_count: u32,
    /// Max notifications per publish (`0` = unlimited).
    pub max_notifications_per_publish: u32,
    /// Whether publishing is initially enabled.
    pub publishing_enabled: bool,
    /// Subscription priority.
    pub priority: u8,
}

impl CreateSubscriptionRequest {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            request_header: decode_request_header(r)?,
            requested_publishing_interval: r.read_f64()?,
            requested_lifetime_count: r.read_u32()?,
            requested_max_keep_alive_count: r.read_u32()?,
            max_notifications_per_publish: r.read_u32()?,
            publishing_enabled: r.read_u8()? != 0,
            priority: r.read_u8()?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::CREATE_SUBSCRIPTION_REQUEST.encode(w)?;
        encode_request_header(&self.request_header, w)?;
        w.write_f64(self.requested_publishing_interval);
        w.write_u32(self.requested_lifetime_count);
        w.write_u32(self.requested_max_keep_alive_count);
        w.write_u32(self.max_notifications_per_publish);
        w.write_u8(u8::from(self.publishing_enabled));
        w.write_u8(self.priority);
        Ok(())
    }
}

/// `CreateSubscriptionResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateSubscriptionResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
    /// Assigned subscription id.
    pub subscription_id: u32,
    /// Revised publishing interval.
    pub revised_publishing_interval: f64,
    /// Revised lifetime count.
    pub revised_lifetime_count: u32,
    /// Revised max keep-alive count.
    pub revised_max_keep_alive_count: u32,
}

impl CreateSubscriptionResponse {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            response_header: decode_response_header(r)?,
            subscription_id: r.read_u32()?,
            revised_publishing_interval: r.read_f64()?,
            revised_lifetime_count: r.read_u32()?,
            revised_max_keep_alive_count: r.read_u32()?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::CREATE_SUBSCRIPTION_RESPONSE.encode(w)?;
        encode_response_header(&self.response_header, w)?;
        w.write_u32(self.subscription_id);
        w.write_f64(self.revised_publishing_interval);
        w.write_u32(self.revised_lifetime_count);
        w.write_u32(self.revised_max_keep_alive_count);
        Ok(())
    }
}

/// `SetPublishingModeRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct SetPublishingModeRequest {
    /// Common request header.
    pub request_header: RequestHeader,
    /// New publishing-enabled flag.
    pub publishing_enabled: bool,
    /// Subscriptions to apply it to.
    pub subscription_ids: Vec<u32>,
}

impl SetPublishingModeRequest {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            request_header: decode_request_header(r)?,
            publishing_enabled: r.read_u8()? != 0,
            subscription_ids: read_u32_array(r)?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::SET_PUBLISHING_MODE_REQUEST.encode(w)?;
        encode_request_header(&self.request_header, w)?;
        w.write_u8(u8::from(self.publishing_enabled));
        write_u32_array(w, &self.subscription_ids)?;
        Ok(())
    }
}

/// `SetPublishingModeResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct SetPublishingModeResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
    /// Per-subscription status.
    pub results: Vec<u32>,
}

impl SetPublishingModeResponse {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let response_header = decode_response_header(r)?;
        let results = read_u32_array(r)?;
        skip_diagnostic_info_array(r)?;
        Ok(Self {
            response_header,
            results,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::SET_PUBLISHING_MODE_RESPONSE.encode(w)?;
        encode_response_header(&self.response_header, w)?;
        write_u32_array(w, &self.results)?;
        write_empty_diagnostic_info_array(w);
        Ok(())
    }
}

/// `MonitoringParameters` (Part 4 §7.21).
#[derive(Debug, Clone, PartialEq)]
pub struct MonitoringParameters {
    /// Client-assigned handle echoed in notifications.
    pub client_handle: u32,
    /// Requested sampling interval in milliseconds.
    pub sampling_interval: f64,
    /// Monitoring filter (null ExtensionObject = none).
    pub filter: ExtensionObject,
    /// Notification queue size.
    pub queue_size: u32,
    /// Discard oldest on overflow.
    pub discard_oldest: bool,
}

impl UaEncode for MonitoringParameters {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        w.write_u32(self.client_handle);
        w.write_f64(self.sampling_interval);
        self.filter.encode(w)?;
        w.write_u32(self.queue_size);
        w.write_u8(u8::from(self.discard_oldest));
        Ok(())
    }
}

impl UaDecode for MonitoringParameters {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            client_handle: r.read_u32()?,
            sampling_interval: r.read_f64()?,
            filter: ExtensionObject::decode(r)?,
            queue_size: r.read_u32()?,
            discard_oldest: r.read_u8()? != 0,
        })
    }
}

/// `MonitoredItemCreateRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct MonitoredItemCreateRequest {
    /// What to monitor.
    pub item_to_monitor: ReadValueId,
    /// MonitoringMode (0 Disabled / 1 Sampling / 2 Reporting).
    pub monitoring_mode: i32,
    /// Requested parameters.
    pub requested_parameters: MonitoringParameters,
}

impl UaEncode for MonitoredItemCreateRequest {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        self.item_to_monitor.encode(w)?;
        w.write_i32(self.monitoring_mode);
        self.requested_parameters.encode(w)?;
        Ok(())
    }
}

impl UaDecode for MonitoredItemCreateRequest {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            item_to_monitor: ReadValueId::decode(r)?,
            monitoring_mode: r.read_i32()?,
            requested_parameters: MonitoringParameters::decode(r)?,
        })
    }
}

/// `MonitoredItemCreateResult`.
#[derive(Debug, Clone, PartialEq)]
pub struct MonitoredItemCreateResult {
    /// Per-item status.
    pub status_code: u32,
    /// Assigned monitored item id.
    pub monitored_item_id: u32,
    /// Revised sampling interval.
    pub revised_sampling_interval: f64,
    /// Revised queue size.
    pub revised_queue_size: u32,
    /// Filter result (null ExtensionObject = none).
    pub filter_result: ExtensionObject,
}

impl UaEncode for MonitoredItemCreateResult {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        w.write_u32(self.status_code);
        w.write_u32(self.monitored_item_id);
        w.write_f64(self.revised_sampling_interval);
        w.write_u32(self.revised_queue_size);
        self.filter_result.encode(w)?;
        Ok(())
    }
}

impl UaDecode for MonitoredItemCreateResult {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            status_code: r.read_u32()?,
            monitored_item_id: r.read_u32()?,
            revised_sampling_interval: r.read_f64()?,
            revised_queue_size: r.read_u32()?,
            filter_result: ExtensionObject::decode(r)?,
        })
    }
}

/// A null monitoring filter / filter result.
#[must_use]
pub fn null_filter() -> ExtensionObject {
    null_extension_object()
}

/// `CreateMonitoredItemsRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateMonitoredItemsRequest {
    /// Common request header.
    pub request_header: RequestHeader,
    /// Target subscription.
    pub subscription_id: u32,
    /// TimestampsToReturn.
    pub timestamps_to_return: i32,
    /// Items to create.
    pub items_to_create: Vec<MonitoredItemCreateRequest>,
}

impl CreateMonitoredItemsRequest {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            request_header: decode_request_header(r)?,
            subscription_id: r.read_u32()?,
            timestamps_to_return: r.read_i32()?,
            items_to_create: read_array::<MonitoredItemCreateRequest>(r)?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::CREATE_MONITORED_ITEMS_REQUEST.encode(w)?;
        encode_request_header(&self.request_header, w)?;
        w.write_u32(self.subscription_id);
        w.write_i32(self.timestamps_to_return);
        write_array(w, &self.items_to_create, "ItemsToCreate")?;
        Ok(())
    }
}

/// `CreateMonitoredItemsResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateMonitoredItemsResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
    /// Per-item results.
    pub results: Vec<MonitoredItemCreateResult>,
}

impl CreateMonitoredItemsResponse {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let response_header = decode_response_header(r)?;
        let results = read_array::<MonitoredItemCreateResult>(r)?;
        skip_diagnostic_info_array(r)?;
        Ok(Self {
            response_header,
            results,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::CREATE_MONITORED_ITEMS_RESPONSE.encode(w)?;
        encode_response_header(&self.response_header, w)?;
        write_array(w, &self.results, "Results")?;
        write_empty_diagnostic_info_array(w);
        Ok(())
    }
}

/// `SubscriptionAcknowledgement`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionAcknowledgement {
    /// Subscription id.
    pub subscription_id: u32,
    /// Acknowledged sequence number.
    pub sequence_number: u32,
}

impl UaEncode for SubscriptionAcknowledgement {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        w.write_u32(self.subscription_id);
        w.write_u32(self.sequence_number);
        Ok(())
    }
}

impl UaDecode for SubscriptionAcknowledgement {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            subscription_id: r.read_u32()?,
            sequence_number: r.read_u32()?,
        })
    }
}

/// `MonitoredItemNotification` — one changed value in a `DataChangeNotification`.
#[derive(Debug, Clone, PartialEq)]
pub struct MonitoredItemNotification {
    /// The monitored item's client handle.
    pub client_handle: u32,
    /// The new value.
    pub value: DataValue,
}

impl UaEncode for MonitoredItemNotification {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        w.write_u32(self.client_handle);
        self.value.encode(w)?;
        Ok(())
    }
}

impl UaDecode for MonitoredItemNotification {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            client_handle: r.read_u32()?,
            value: DataValue::decode(r)?,
        })
    }
}

/// `DataChangeNotification` (Part 4 §7.25.2) — carried inside an
/// `ExtensionObject` in a [`NotificationMessage`].
#[derive(Debug, Clone, PartialEq)]
pub struct DataChangeNotification {
    /// The changed monitored items.
    pub monitored_items: Vec<MonitoredItemNotification>,
}

impl DataChangeNotification {
    /// Wraps this notification in its `ExtensionObject` (TypeId 811).
    ///
    /// # Errors
    /// [`EncodeError`] on an oversized field.
    pub fn to_extension_object(&self) -> Result<ExtensionObject, EncodeError> {
        let mut w = UaWriter::new();
        write_array(&mut w, &self.monitored_items, "MonitoredItems")?;
        write_empty_diagnostic_info_array(&mut w);
        Ok(ExtensionObject {
            type_id: node_ids::DATA_CHANGE_NOTIFICATION,
            body: ExtensionObjectBody::ByteString(w.into_vec()),
        })
    }

    /// Recovers a `DataChangeNotification` from its `ExtensionObject`.
    ///
    /// # Errors
    /// [`DecodeError`] if the body is not a matching ByteString.
    pub fn from_extension_object(eo: &ExtensionObject) -> Result<Self, DecodeError> {
        match &eo.body {
            ExtensionObjectBody::ByteString(b) => {
                let mut r = UaReader::new(b);
                let monitored_items = read_array::<MonitoredItemNotification>(&mut r)?;
                skip_diagnostic_info_array(&mut r)?;
                Ok(Self { monitored_items })
            }
            _ => Err(DecodeError::MalformedMessage {
                message: "DataChangeNotification ExtensionObject is not a ByteString body",
            }),
        }
    }
}

/// `NotificationMessage` (Part 4 §7.26).
#[derive(Debug, Clone, PartialEq)]
pub struct NotificationMessage {
    /// Sequence number.
    pub sequence_number: u32,
    /// Publish time (DateTime ticks).
    pub publish_time: i64,
    /// Notification data (each an `ExtensionObject`, e.g. DataChangeNotification).
    pub notification_data: Vec<ExtensionObject>,
}

impl UaEncode for NotificationMessage {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        w.write_u32(self.sequence_number);
        w.write_i64(self.publish_time);
        write_array(w, &self.notification_data, "NotificationData")?;
        Ok(())
    }
}

impl UaDecode for NotificationMessage {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            sequence_number: r.read_u32()?,
            publish_time: r.read_i64()?,
            notification_data: read_array::<ExtensionObject>(r)?,
        })
    }
}

/// `PublishRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct PublishRequest {
    /// Common request header.
    pub request_header: RequestHeader,
    /// Acknowledgements of previously received notifications.
    pub subscription_acknowledgements: Vec<SubscriptionAcknowledgement>,
}

impl PublishRequest {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            request_header: decode_request_header(r)?,
            subscription_acknowledgements: read_array::<SubscriptionAcknowledgement>(r)?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::PUBLISH_REQUEST.encode(w)?;
        encode_request_header(&self.request_header, w)?;
        write_array(w, &self.subscription_acknowledgements, "Acks")?;
        Ok(())
    }
}

/// `PublishResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct PublishResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
    /// Subscription the notification belongs to.
    pub subscription_id: u32,
    /// Sequence numbers still available for Republish.
    pub available_sequence_numbers: Vec<u32>,
    /// Whether more notifications are queued.
    pub more_notifications: bool,
    /// The notification message.
    pub notification_message: NotificationMessage,
    /// Acknowledgement results.
    pub results: Vec<u32>,
}

impl PublishResponse {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let response_header = decode_response_header(r)?;
        let subscription_id = r.read_u32()?;
        let available_sequence_numbers = read_u32_array(r)?;
        let more_notifications = r.read_u8()? != 0;
        let notification_message = NotificationMessage::decode(r)?;
        let results = read_u32_array(r)?;
        skip_diagnostic_info_array(r)?;
        Ok(Self {
            response_header,
            subscription_id,
            available_sequence_numbers,
            more_notifications,
            notification_message,
            results,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::PUBLISH_RESPONSE.encode(w)?;
        encode_response_header(&self.response_header, w)?;
        w.write_u32(self.subscription_id);
        write_u32_array(w, &self.available_sequence_numbers)?;
        w.write_u8(u8::from(self.more_notifications));
        self.notification_message.encode(w)?;
        write_u32_array(w, &self.results)?;
        write_empty_diagnostic_info_array(w);
        Ok(())
    }
}

/// `DeleteSubscriptionsRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct DeleteSubscriptionsRequest {
    /// Common request header.
    pub request_header: RequestHeader,
    /// Subscriptions to delete.
    pub subscription_ids: Vec<u32>,
}

impl DeleteSubscriptionsRequest {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            request_header: decode_request_header(r)?,
            subscription_ids: read_u32_array(r)?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::DELETE_SUBSCRIPTIONS_REQUEST.encode(w)?;
        encode_request_header(&self.request_header, w)?;
        write_u32_array(w, &self.subscription_ids)?;
        Ok(())
    }
}

/// `DeleteSubscriptionsResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct DeleteSubscriptionsResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
    /// Per-subscription status.
    pub results: Vec<u32>,
}

impl DeleteSubscriptionsResponse {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let response_header = decode_response_header(r)?;
        let results = read_u32_array(r)?;
        skip_diagnostic_info_array(r)?;
        Ok(Self {
            response_header,
            results,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::DELETE_SUBSCRIPTIONS_RESPONSE.encode(w)?;
        encode_response_header(&self.response_header, w)?;
        write_u32_array(w, &self.results)?;
        write_empty_diagnostic_info_array(w);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Dispatch enums.
// ---------------------------------------------------------------------------

/// Any decodable service request.
#[derive(Debug, Clone, PartialEq)]
pub enum ServiceRequest {
    /// CreateSession.
    CreateSession(CreateSessionRequest),
    /// ActivateSession.
    ActivateSession(ActivateSessionRequest),
    /// CloseSession.
    CloseSession(CloseSessionRequest),
    /// Read.
    Read(ReadRequest),
    /// Write.
    Write(WriteRequest),
    /// Browse.
    Browse(BrowseRequest),
    /// GetEndpoints.
    GetEndpoints(GetEndpointsRequest),
    /// FindServers.
    FindServers(FindServersRequest),
    /// CreateSubscription.
    CreateSubscription(CreateSubscriptionRequest),
    /// SetPublishingMode.
    SetPublishingMode(SetPublishingModeRequest),
    /// CreateMonitoredItems.
    CreateMonitoredItems(CreateMonitoredItemsRequest),
    /// Publish.
    Publish(PublishRequest),
    /// DeleteSubscriptions.
    DeleteSubscriptions(DeleteSubscriptionsRequest),
    /// Call.
    Call(CallRequest),
}

impl ServiceRequest {
    /// Encodes the full body (`TypeId` + fields).
    ///
    /// # Errors
    /// [`EncodeError`] on an oversized field.
    pub fn encode(&self) -> Result<Vec<u8>, EncodeError> {
        let mut w = UaWriter::new();
        match self {
            Self::CreateSession(m) => m.encode_into(&mut w)?,
            Self::ActivateSession(m) => m.encode_into(&mut w)?,
            Self::CloseSession(m) => m.encode_into(&mut w)?,
            Self::Read(m) => m.encode_into(&mut w)?,
            Self::Write(m) => m.encode_into(&mut w)?,
            Self::Browse(m) => m.encode_into(&mut w)?,
            Self::GetEndpoints(m) => m.encode_into(&mut w)?,
            Self::FindServers(m) => m.encode_into(&mut w)?,
            Self::CreateSubscription(m) => m.encode_into(&mut w)?,
            Self::SetPublishingMode(m) => m.encode_into(&mut w)?,
            Self::CreateMonitoredItems(m) => m.encode_into(&mut w)?,
            Self::Publish(m) => m.encode_into(&mut w)?,
            Self::DeleteSubscriptions(m) => m.encode_into(&mut w)?,
            Self::Call(m) => m.encode_into(&mut w)?,
        }
        Ok(w.into_vec())
    }

    /// Decodes a request body, dispatching on the leading `TypeId`.
    ///
    /// # Errors
    /// [`DecodeError`] on truncation or an unsupported service type.
    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let mut r = UaReader::new(body);
        let type_id = NodeId::decode(&mut r)?;
        Ok(if type_id == node_ids::CREATE_SESSION_REQUEST {
            Self::CreateSession(CreateSessionRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::ACTIVATE_SESSION_REQUEST {
            Self::ActivateSession(ActivateSessionRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::CLOSE_SESSION_REQUEST {
            Self::CloseSession(CloseSessionRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::READ_REQUEST {
            Self::Read(ReadRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::WRITE_REQUEST {
            Self::Write(WriteRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::BROWSE_REQUEST {
            Self::Browse(BrowseRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::GET_ENDPOINTS_REQUEST {
            Self::GetEndpoints(GetEndpointsRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::FIND_SERVERS_REQUEST {
            Self::FindServers(FindServersRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::CREATE_SUBSCRIPTION_REQUEST {
            Self::CreateSubscription(CreateSubscriptionRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::SET_PUBLISHING_MODE_REQUEST {
            Self::SetPublishingMode(SetPublishingModeRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::CREATE_MONITORED_ITEMS_REQUEST {
            Self::CreateMonitoredItems(CreateMonitoredItemsRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::PUBLISH_REQUEST {
            Self::Publish(PublishRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::DELETE_SUBSCRIPTIONS_REQUEST {
            Self::DeleteSubscriptions(DeleteSubscriptionsRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::CALL_REQUEST {
            Self::Call(CallRequest::decode_body(&mut r)?)
        } else {
            return Err(DecodeError::MalformedMessage {
                message: "unsupported OPC-UA service request type",
            });
        })
    }
}

/// Any decodable service response.
#[derive(Debug, Clone, PartialEq)]
pub enum ServiceResponse {
    /// CreateSession.
    CreateSession(CreateSessionResponse),
    /// ActivateSession.
    ActivateSession(ActivateSessionResponse),
    /// CloseSession.
    CloseSession(CloseSessionResponse),
    /// Read.
    Read(ReadResponse),
    /// Write.
    Write(WriteResponse),
    /// Browse.
    Browse(BrowseResponse),
    /// GetEndpoints.
    GetEndpoints(GetEndpointsResponse),
    /// FindServers.
    FindServers(FindServersResponse),
    /// CreateSubscription.
    CreateSubscription(CreateSubscriptionResponse),
    /// SetPublishingMode.
    SetPublishingMode(SetPublishingModeResponse),
    /// CreateMonitoredItems.
    CreateMonitoredItems(CreateMonitoredItemsResponse),
    /// Publish.
    Publish(PublishResponse),
    /// DeleteSubscriptions.
    DeleteSubscriptions(DeleteSubscriptionsResponse),
    /// Call.
    Call(CallResponse),
}

impl ServiceResponse {
    /// Encodes the full body (`TypeId` + fields).
    ///
    /// # Errors
    /// [`EncodeError`] on an oversized field.
    pub fn encode(&self) -> Result<Vec<u8>, EncodeError> {
        let mut w = UaWriter::new();
        match self {
            Self::CreateSession(m) => m.encode_into(&mut w)?,
            Self::ActivateSession(m) => m.encode_into(&mut w)?,
            Self::CloseSession(m) => m.encode_into(&mut w)?,
            Self::Read(m) => m.encode_into(&mut w)?,
            Self::Write(m) => m.encode_into(&mut w)?,
            Self::Browse(m) => m.encode_into(&mut w)?,
            Self::GetEndpoints(m) => m.encode_into(&mut w)?,
            Self::FindServers(m) => m.encode_into(&mut w)?,
            Self::CreateSubscription(m) => m.encode_into(&mut w)?,
            Self::SetPublishingMode(m) => m.encode_into(&mut w)?,
            Self::CreateMonitoredItems(m) => m.encode_into(&mut w)?,
            Self::Publish(m) => m.encode_into(&mut w)?,
            Self::DeleteSubscriptions(m) => m.encode_into(&mut w)?,
            Self::Call(m) => m.encode_into(&mut w)?,
        }
        Ok(w.into_vec())
    }

    /// Decodes a response body, dispatching on the leading `TypeId`.
    ///
    /// # Errors
    /// [`DecodeError`] on truncation or an unsupported response type.
    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let mut r = UaReader::new(body);
        let type_id = NodeId::decode(&mut r)?;
        Ok(if type_id == node_ids::CREATE_SESSION_RESPONSE {
            Self::CreateSession(CreateSessionResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::ACTIVATE_SESSION_RESPONSE {
            Self::ActivateSession(ActivateSessionResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::CLOSE_SESSION_RESPONSE {
            Self::CloseSession(CloseSessionResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::READ_RESPONSE {
            Self::Read(ReadResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::WRITE_RESPONSE {
            Self::Write(WriteResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::BROWSE_RESPONSE {
            Self::Browse(BrowseResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::GET_ENDPOINTS_RESPONSE {
            Self::GetEndpoints(GetEndpointsResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::FIND_SERVERS_RESPONSE {
            Self::FindServers(FindServersResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::CREATE_SUBSCRIPTION_RESPONSE {
            Self::CreateSubscription(CreateSubscriptionResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::SET_PUBLISHING_MODE_RESPONSE {
            Self::SetPublishingMode(SetPublishingModeResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::CREATE_MONITORED_ITEMS_RESPONSE {
            Self::CreateMonitoredItems(CreateMonitoredItemsResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::PUBLISH_RESPONSE {
            Self::Publish(PublishResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::DELETE_SUBSCRIPTIONS_RESPONSE {
            Self::DeleteSubscriptions(DeleteSubscriptionsResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::CALL_RESPONSE {
            Self::Call(CallResponse::decode_body(&mut r)?)
        } else {
            return Err(DecodeError::MalformedMessage {
                message: "unsupported OPC-UA service response type",
            });
        })
    }
}

// RequestHeader/ResponseHeader encode/decode are crate-private in opcua-uacp,
// so re-do the field order here against the public structs.
fn encode_request_header(h: &RequestHeader, w: &mut UaWriter) -> Result<(), EncodeError> {
    h.authentication_token.encode(w)?;
    w.write_i64(h.timestamp);
    w.write_u32(h.request_handle);
    w.write_u32(h.return_diagnostics);
    write_string(w, &h.audit_entry_id)?;
    w.write_u32(h.timeout_hint);
    h.additional_header.encode(w)?;
    Ok(())
}

fn decode_request_header(r: &mut UaReader<'_>) -> Result<RequestHeader, DecodeError> {
    Ok(RequestHeader {
        authentication_token: NodeId::decode(r)?,
        timestamp: r.read_i64()?,
        request_handle: r.read_u32()?,
        return_diagnostics: r.read_u32()?,
        audit_entry_id: read_string(r)?,
        timeout_hint: r.read_u32()?,
        additional_header: zerodds_opcua_gateway::data_value::ExtensionObject::decode(r)?,
    })
}

fn encode_response_header(h: &ResponseHeader, w: &mut UaWriter) -> Result<(), EncodeError> {
    w.write_i64(h.timestamp);
    w.write_u32(h.request_handle);
    w.write_u32(h.service_result);
    w.write_u8(0); // empty DiagnosticInfo
    write_string_array(w, &h.string_table)?;
    h.additional_header.encode(w)?;
    Ok(())
}

fn decode_response_header(r: &mut UaReader<'_>) -> Result<ResponseHeader, DecodeError> {
    let timestamp = r.read_i64()?;
    let request_handle = r.read_u32()?;
    let service_result = r.read_u32()?;
    // empty DiagnosticInfo (we only ever emit mask 0; tolerate present fields).
    crate::wire::skip_one_diagnostic_info(r)?;
    let string_table = read_string_array(r)?;
    let additional_header = zerodds_opcua_gateway::data_value::ExtensionObject::decode(r)?;
    Ok(ResponseHeader {
        timestamp,
        request_handle,
        service_result,
        string_table,
        additional_header,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zerodds_opcua_gateway::data_value::VariantValue;
    use zerodds_opcua_uacp::securechannel::null_extension_object;

    fn req_header() -> RequestHeader {
        RequestHeader::new(NodeId::numeric(0, 0), 1)
    }

    #[test]
    fn read_request_round_trips() {
        let req = ServiceRequest::Read(ReadRequest {
            request_header: req_header(),
            max_age: 0.0,
            timestamps_to_return: 0,
            nodes_to_read: alloc::vec![ReadValueId {
                node_id: NodeId::numeric(1, 42),
                attribute_id: ATTRIBUTE_VALUE,
                index_range: String::new(),
                data_encoding: QualifiedName {
                    namespace_index: 0,
                    name: String::new(),
                },
            }],
        });
        let body = req.encode().expect("encode");
        assert_eq!(ServiceRequest::decode(&body).expect("decode"), req);
    }

    #[test]
    fn read_response_round_trips() {
        let resp = ServiceResponse::Read(ReadResponse {
            response_header: ResponseHeader::new(1, 0),
            results: alloc::vec![DataValue::new_value(
                Variant::scalar(VariantValue::Int32(99)),
                0,
                0,
            )],
        });
        let body = resp.encode().expect("encode");
        assert_eq!(ServiceResponse::decode(&body).expect("decode"), resp);
    }

    #[test]
    fn write_round_trips() {
        let req = ServiceRequest::Write(WriteRequest {
            request_header: req_header(),
            nodes_to_write: alloc::vec![WriteValue {
                node_id: NodeId::numeric(1, 7),
                attribute_id: ATTRIBUTE_VALUE,
                index_range: String::new(),
                value: DataValue::new_value(Variant::scalar(VariantValue::Int32(5)), 0, 0),
            }],
        });
        let body = req.encode().expect("encode");
        assert_eq!(ServiceRequest::decode(&body).expect("decode"), req);

        let resp = ServiceResponse::Write(WriteResponse {
            response_header: ResponseHeader::new(1, 0),
            results: alloc::vec![0],
        });
        let rb = resp.encode().expect("encode");
        assert_eq!(ServiceResponse::decode(&rb).expect("decode"), resp);
    }

    #[test]
    fn browse_round_trips() {
        let req = ServiceRequest::Browse(BrowseRequest {
            request_header: req_header(),
            view: ViewDescription::default(),
            requested_max_references_per_node: 100,
            nodes_to_browse: alloc::vec![BrowseDescription {
                node_id: NodeId::numeric(0, 85),
                browse_direction: 2,
                reference_type_id: NodeId::numeric(0, 33),
                include_subtypes: true,
                node_class_mask: 0,
                result_mask: 0x3F,
            }],
        });
        let body = req.encode().expect("encode");
        assert_eq!(ServiceRequest::decode(&body).expect("decode"), req);

        let resp = ServiceResponse::Browse(BrowseResponse {
            response_header: ResponseHeader::new(1, 0),
            results: alloc::vec![BrowseResult {
                status_code: 0,
                continuation_point: Vec::new(),
                references: alloc::vec![ReferenceDescription {
                    reference_type_id: NodeId::numeric(0, 35),
                    is_forward: true,
                    node_id: ExpandedNodeId {
                        node_id: NodeId::numeric(1, 1),
                        namespace_uri: String::new(),
                        server_index: 0,
                    },
                    browse_name: QualifiedName {
                        namespace_index: 1,
                        name: String::from("Boiler"),
                    },
                    display_name: LocalizedText {
                        locale: None,
                        text: Some(String::from("Boiler")),
                    },
                    node_class: 1,
                    type_definition: ExpandedNodeId {
                        node_id: NodeId::numeric(0, 58),
                        namespace_uri: String::new(),
                        server_index: 0,
                    },
                }],
            }],
        });
        let rb = resp.encode().expect("encode");
        assert_eq!(ServiceResponse::decode(&rb).expect("decode"), resp);
    }

    #[test]
    fn discovery_requests_round_trip() {
        let ge = ServiceRequest::GetEndpoints(GetEndpointsRequest {
            request_header: req_header(),
            endpoint_url: String::from("opc.tcp://x:4840"),
            locale_ids: alloc::vec![String::from("en")],
            profile_uris: Vec::new(),
        });
        assert_eq!(
            ServiceRequest::decode(&ge.encode().expect("e")).expect("d"),
            ge
        );

        let fs = ServiceRequest::FindServers(FindServersRequest {
            request_header: req_header(),
            endpoint_url: String::from("opc.tcp://x:4840"),
            locale_ids: Vec::new(),
            server_uris: alloc::vec![String::from("urn:server")],
        });
        assert_eq!(
            ServiceRequest::decode(&fs.encode().expect("e")).expect("d"),
            fs
        );

        // Empty responses round-trip through the dispatch too.
        let ger = ServiceResponse::GetEndpoints(GetEndpointsResponse {
            response_header: ResponseHeader::new(1, 0),
            endpoints: Vec::new(),
        });
        assert_eq!(
            ServiceResponse::decode(&ger.encode().expect("e")).expect("d"),
            ger
        );
    }

    #[test]
    fn subscription_round_trips() {
        let cs = ServiceRequest::CreateSubscription(CreateSubscriptionRequest {
            request_header: req_header(),
            requested_publishing_interval: 500.0,
            requested_lifetime_count: 6000,
            requested_max_keep_alive_count: 20,
            max_notifications_per_publish: 0,
            publishing_enabled: true,
            priority: 5,
        });
        assert_eq!(
            ServiceRequest::decode(&cs.encode().expect("e")).expect("d"),
            cs
        );

        let cm = ServiceRequest::CreateMonitoredItems(CreateMonitoredItemsRequest {
            request_header: req_header(),
            subscription_id: 1,
            timestamps_to_return: 0,
            items_to_create: alloc::vec![MonitoredItemCreateRequest {
                item_to_monitor: ReadValueId {
                    node_id: NodeId::numeric(1, 1),
                    attribute_id: ATTRIBUTE_VALUE,
                    index_range: String::new(),
                    data_encoding: QualifiedName {
                        namespace_index: 0,
                        name: String::new(),
                    },
                },
                monitoring_mode: 2,
                requested_parameters: MonitoringParameters {
                    client_handle: 7,
                    sampling_interval: 250.0,
                    filter: null_filter(),
                    queue_size: 4,
                    discard_oldest: true,
                },
            }],
        });
        assert_eq!(
            ServiceRequest::decode(&cm.encode().expect("e")).expect("d"),
            cm
        );
    }

    #[test]
    fn publish_data_change_notification_round_trips() {
        // The DataChangeNotification survives the ExtensionObject wrapping.
        let dcn = DataChangeNotification {
            monitored_items: alloc::vec![MonitoredItemNotification {
                client_handle: 7,
                value: DataValue::new_value(Variant::scalar(VariantValue::Int32(99)), 0, 0),
            }],
        };
        let eo = dcn.to_extension_object().expect("wrap");
        assert_eq!(
            DataChangeNotification::from_extension_object(&eo).expect("unwrap"),
            dcn
        );

        // And the whole PublishResponse round-trips through the dispatch.
        let resp = ServiceResponse::Publish(PublishResponse {
            response_header: ResponseHeader::new(1, 0),
            subscription_id: 3,
            available_sequence_numbers: alloc::vec![1],
            more_notifications: false,
            notification_message: NotificationMessage {
                sequence_number: 1,
                publish_time: 0,
                notification_data: alloc::vec![eo],
            },
            results: Vec::new(),
        });
        assert_eq!(
            ServiceResponse::decode(&resp.encode().expect("e")).expect("d"),
            resp
        );
    }

    #[test]
    fn call_round_trips() {
        let req = ServiceRequest::Call(CallRequest {
            request_header: req_header(),
            methods_to_call: alloc::vec![CallMethodRequest {
                object_id: NodeId::numeric(0, 0),
                method_id: NodeId::numeric(1, 100),
                input_arguments: alloc::vec![Variant::scalar(VariantValue::String(String::from(
                    "group-1"
                )))],
            }],
        });
        let body = req.encode().expect("encode");
        assert_eq!(ServiceRequest::decode(&body).expect("decode"), req);

        let resp = ServiceResponse::Call(CallResponse {
            response_header: ResponseHeader::new(1, 0),
            results: alloc::vec![CallMethodResult {
                status_code: 0,
                input_argument_results: alloc::vec![0],
                output_arguments: alloc::vec![Variant::scalar(VariantValue::UInt32(7))],
            }],
        });
        let rb = resp.encode().expect("encode");
        assert_eq!(ServiceResponse::decode(&rb).expect("decode"), resp);
    }

    #[test]
    fn session_lifecycle_round_trips() {
        let close = ServiceRequest::CloseSession(CloseSessionRequest {
            request_header: req_header(),
            delete_subscriptions: true,
        });
        assert_eq!(
            ServiceRequest::decode(&close.encode().expect("e")).expect("d"),
            close
        );

        let act = ServiceResponse::ActivateSession(ActivateSessionResponse {
            response_header: ResponseHeader::new(1, 0),
            server_nonce: alloc::vec![1, 2, 3],
            results: alloc::vec![0],
        });
        let _ = null_extension_object();
        assert_eq!(
            ServiceResponse::decode(&act.encode().expect("e")).expect("d"),
            act
        );
    }
}
