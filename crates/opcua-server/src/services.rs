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
    read_array, read_bool, read_byte_string, read_i64_array, read_string, read_string_array,
    read_u32_array, skip_diagnostic_info_array, write_array, write_bool, write_byte_string,
    write_empty_diagnostic_info_array, write_i64_array, write_string, write_string_array,
    write_u32_array,
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
// BrowseNext (Part 4 §5.9.3).
// ---------------------------------------------------------------------------

/// `BrowseNextRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct BrowseNextRequest {
    /// Common request header.
    pub request_header: RequestHeader,
    /// `TRUE` frees the continuation points instead of continuing them.
    pub release_continuation_points: bool,
    /// The continuation points a previous Browse/BrowseNext returned.
    pub continuation_points: Vec<Vec<u8>>,
}

impl BrowseNextRequest {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let request_header = decode_request_header(r)?;
        let release_continuation_points = r.read_u8()? != 0;
        let n = r.read_i32()?;
        let mut continuation_points = Vec::new();
        if n > 0 {
            let n = n as usize;
            zerodds_opcua_pubsub::binary::check_array_len(
                r,
                n,
                4,
                "ContinuationPoint array length exceeds remaining bytes",
            )?;
            for _ in 0..n {
                continuation_points.push(read_byte_string(r)?);
            }
        }
        Ok(Self {
            request_header,
            release_continuation_points,
            continuation_points,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::BROWSE_NEXT_REQUEST.encode(w)?;
        encode_request_header(&self.request_header, w)?;
        w.write_u8(u8::from(self.release_continuation_points));
        let len = i32::try_from(self.continuation_points.len()).map_err(|_| {
            EncodeError::LengthOverflow {
                what: "ContinuationPoints",
                len: self.continuation_points.len(),
            }
        })?;
        w.write_i32(len);
        for cp in &self.continuation_points {
            write_byte_string(w, cp)?;
        }
        Ok(())
    }
}

/// `BrowseNextResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct BrowseNextResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
    /// One result per continuation point.
    pub results: Vec<BrowseResult>,
}

impl BrowseNextResponse {
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
        node_ids::BROWSE_NEXT_RESPONSE.encode(w)?;
        encode_response_header(&self.response_header, w)?;
        write_array(w, &self.results, "Results")?;
        write_empty_diagnostic_info_array(w);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ModifySubscription (Part 4 §5.14.3).
// ---------------------------------------------------------------------------

/// `ModifySubscriptionRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct ModifySubscriptionRequest {
    /// Common request header.
    pub request_header: RequestHeader,
    /// The subscription to modify.
    pub subscription_id: u32,
    /// Requested publishing interval (ms).
    pub requested_publishing_interval: f64,
    /// Requested lifetime count.
    pub requested_lifetime_count: u32,
    /// Requested max keep-alive count.
    pub requested_max_keep_alive_count: u32,
    /// Max notifications per Publish (`0` = no limit).
    pub max_notifications_per_publish: u32,
    /// Relative priority.
    pub priority: u8,
}

impl ModifySubscriptionRequest {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            request_header: decode_request_header(r)?,
            subscription_id: r.read_u32()?,
            requested_publishing_interval: r.read_f64()?,
            requested_lifetime_count: r.read_u32()?,
            requested_max_keep_alive_count: r.read_u32()?,
            max_notifications_per_publish: r.read_u32()?,
            priority: r.read_u8()?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::MODIFY_SUBSCRIPTION_REQUEST.encode(w)?;
        encode_request_header(&self.request_header, w)?;
        w.write_u32(self.subscription_id);
        w.write_f64(self.requested_publishing_interval);
        w.write_u32(self.requested_lifetime_count);
        w.write_u32(self.requested_max_keep_alive_count);
        w.write_u32(self.max_notifications_per_publish);
        w.write_u8(self.priority);
        Ok(())
    }
}

/// `ModifySubscriptionResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct ModifySubscriptionResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
    /// Revised publishing interval (ms).
    pub revised_publishing_interval: f64,
    /// Revised lifetime count.
    pub revised_lifetime_count: u32,
    /// Revised max keep-alive count.
    pub revised_max_keep_alive_count: u32,
}

impl ModifySubscriptionResponse {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            response_header: decode_response_header(r)?,
            revised_publishing_interval: r.read_f64()?,
            revised_lifetime_count: r.read_u32()?,
            revised_max_keep_alive_count: r.read_u32()?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::MODIFY_SUBSCRIPTION_RESPONSE.encode(w)?;
        encode_response_header(&self.response_header, w)?;
        w.write_f64(self.revised_publishing_interval);
        w.write_u32(self.revised_lifetime_count);
        w.write_u32(self.revised_max_keep_alive_count);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ModifyMonitoredItems (Part 4 §5.13.3).
// ---------------------------------------------------------------------------

/// `MonitoredItemModifyRequest` (Part 4 §5.13.3.2).
#[derive(Debug, Clone, PartialEq)]
pub struct MonitoredItemModifyRequest {
    /// Server-assigned monitored item id.
    pub monitored_item_id: u32,
    /// The requested monitoring parameters.
    pub requested_parameters: MonitoringParameters,
}

impl UaEncode for MonitoredItemModifyRequest {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        w.write_u32(self.monitored_item_id);
        self.requested_parameters.encode(w)?;
        Ok(())
    }
}

impl UaDecode for MonitoredItemModifyRequest {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            monitored_item_id: r.read_u32()?,
            requested_parameters: MonitoringParameters::decode(r)?,
        })
    }
}

/// `MonitoredItemModifyResult` (Part 4 §5.13.3.2).
#[derive(Debug, Clone, PartialEq)]
pub struct MonitoredItemModifyResult {
    /// Per-item StatusCode.
    pub status_code: u32,
    /// Revised sampling interval (ms).
    pub revised_sampling_interval: f64,
    /// Revised queue size.
    pub revised_queue_size: u32,
    /// Filter result (null when no filter errors occurred).
    pub filter_result: ExtensionObject,
}

impl UaEncode for MonitoredItemModifyResult {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        w.write_u32(self.status_code);
        w.write_f64(self.revised_sampling_interval);
        w.write_u32(self.revised_queue_size);
        self.filter_result.encode(w)?;
        Ok(())
    }
}

impl UaDecode for MonitoredItemModifyResult {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            status_code: r.read_u32()?,
            revised_sampling_interval: r.read_f64()?,
            revised_queue_size: r.read_u32()?,
            filter_result: ExtensionObject::decode(r)?,
        })
    }
}

/// `ModifyMonitoredItemsRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct ModifyMonitoredItemsRequest {
    /// Common request header.
    pub request_header: RequestHeader,
    /// The subscription owning the items.
    pub subscription_id: u32,
    /// TimestampsToReturn for the modified items.
    pub timestamps_to_return: i32,
    /// The items to modify.
    pub items_to_modify: Vec<MonitoredItemModifyRequest>,
}

impl ModifyMonitoredItemsRequest {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            request_header: decode_request_header(r)?,
            subscription_id: r.read_u32()?,
            timestamps_to_return: r.read_i32()?,
            items_to_modify: read_array::<MonitoredItemModifyRequest>(r)?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::MODIFY_MONITORED_ITEMS_REQUEST.encode(w)?;
        encode_request_header(&self.request_header, w)?;
        w.write_u32(self.subscription_id);
        w.write_i32(self.timestamps_to_return);
        write_array(w, &self.items_to_modify, "ItemsToModify")?;
        Ok(())
    }
}

/// `ModifyMonitoredItemsResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct ModifyMonitoredItemsResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
    /// One result per modified item.
    pub results: Vec<MonitoredItemModifyResult>,
}

impl ModifyMonitoredItemsResponse {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let response_header = decode_response_header(r)?;
        let results = read_array::<MonitoredItemModifyResult>(r)?;
        skip_diagnostic_info_array(r)?;
        Ok(Self {
            response_header,
            results,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::MODIFY_MONITORED_ITEMS_RESPONSE.encode(w)?;
        encode_response_header(&self.response_header, w)?;
        write_array(w, &self.results, "Results")?;
        write_empty_diagnostic_info_array(w);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SetMonitoringMode (Part 4 §5.13.4) / DeleteMonitoredItems (§5.13.6).
// ---------------------------------------------------------------------------

/// `SetMonitoringModeRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct SetMonitoringModeRequest {
    /// Common request header.
    pub request_header: RequestHeader,
    /// The subscription owning the items.
    pub subscription_id: u32,
    /// The MonitoringMode to set (Part 4 §7.23).
    pub monitoring_mode: i32,
    /// The items to set it on.
    pub monitored_item_ids: Vec<u32>,
}

impl SetMonitoringModeRequest {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            request_header: decode_request_header(r)?,
            subscription_id: r.read_u32()?,
            monitoring_mode: r.read_i32()?,
            monitored_item_ids: read_u32_array(r)?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::SET_MONITORING_MODE_REQUEST.encode(w)?;
        encode_request_header(&self.request_header, w)?;
        w.write_u32(self.subscription_id);
        w.write_i32(self.monitoring_mode);
        write_u32_array(w, &self.monitored_item_ids)?;
        Ok(())
    }
}

/// `SetMonitoringModeResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct SetMonitoringModeResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
    /// One StatusCode per item.
    pub results: Vec<u32>,
}

impl SetMonitoringModeResponse {
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
        node_ids::SET_MONITORING_MODE_RESPONSE.encode(w)?;
        encode_response_header(&self.response_header, w)?;
        write_u32_array(w, &self.results)?;
        write_empty_diagnostic_info_array(w);
        Ok(())
    }
}

/// `DeleteMonitoredItemsRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct DeleteMonitoredItemsRequest {
    /// Common request header.
    pub request_header: RequestHeader,
    /// The subscription owning the items.
    pub subscription_id: u32,
    /// The items to delete.
    pub monitored_item_ids: Vec<u32>,
}

impl DeleteMonitoredItemsRequest {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            request_header: decode_request_header(r)?,
            subscription_id: r.read_u32()?,
            monitored_item_ids: read_u32_array(r)?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::DELETE_MONITORED_ITEMS_REQUEST.encode(w)?;
        encode_request_header(&self.request_header, w)?;
        w.write_u32(self.subscription_id);
        write_u32_array(w, &self.monitored_item_ids)?;
        Ok(())
    }
}

/// `DeleteMonitoredItemsResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct DeleteMonitoredItemsResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
    /// One StatusCode per item.
    pub results: Vec<u32>,
}

impl DeleteMonitoredItemsResponse {
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
        node_ids::DELETE_MONITORED_ITEMS_RESPONSE.encode(w)?;
        encode_response_header(&self.response_header, w)?;
        write_u32_array(w, &self.results)?;
        write_empty_diagnostic_info_array(w);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SetTriggering (Part 4 §5.13.5).
// ---------------------------------------------------------------------------

/// `SetTriggeringRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct SetTriggeringRequest {
    /// Common request header.
    pub request_header: RequestHeader,
    /// The subscription owning the items.
    pub subscription_id: u32,
    /// The triggering item.
    pub triggering_item_id: u32,
    /// Links to add (processed after `links_to_remove`).
    pub links_to_add: Vec<u32>,
    /// Links to remove (processed first).
    pub links_to_remove: Vec<u32>,
}

impl SetTriggeringRequest {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            request_header: decode_request_header(r)?,
            subscription_id: r.read_u32()?,
            triggering_item_id: r.read_u32()?,
            links_to_add: read_u32_array(r)?,
            links_to_remove: read_u32_array(r)?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::SET_TRIGGERING_REQUEST.encode(w)?;
        encode_request_header(&self.request_header, w)?;
        w.write_u32(self.subscription_id);
        w.write_u32(self.triggering_item_id);
        write_u32_array(w, &self.links_to_add)?;
        write_u32_array(w, &self.links_to_remove)?;
        Ok(())
    }
}

/// `SetTriggeringResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct SetTriggeringResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
    /// One StatusCode per added link.
    pub add_results: Vec<u32>,
    /// One StatusCode per removed link.
    pub remove_results: Vec<u32>,
}

impl SetTriggeringResponse {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let response_header = decode_response_header(r)?;
        let add_results = read_u32_array(r)?;
        skip_diagnostic_info_array(r)?;
        let remove_results = read_u32_array(r)?;
        skip_diagnostic_info_array(r)?;
        Ok(Self {
            response_header,
            add_results,
            remove_results,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::SET_TRIGGERING_RESPONSE.encode(w)?;
        encode_response_header(&self.response_header, w)?;
        write_u32_array(w, &self.add_results)?;
        write_empty_diagnostic_info_array(w);
        write_u32_array(w, &self.remove_results)?;
        write_empty_diagnostic_info_array(w);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Republish (Part 4 §5.14.6) / TransferSubscriptions (§5.14.7).
// ---------------------------------------------------------------------------

/// `RepublishRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct RepublishRequest {
    /// Common request header.
    pub request_header: RequestHeader,
    /// The subscription to republish from.
    pub subscription_id: u32,
    /// The sequence number to retransmit.
    pub retransmit_sequence_number: u32,
}

impl RepublishRequest {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            request_header: decode_request_header(r)?,
            subscription_id: r.read_u32()?,
            retransmit_sequence_number: r.read_u32()?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::REPUBLISH_REQUEST.encode(w)?;
        encode_request_header(&self.request_header, w)?;
        w.write_u32(self.subscription_id);
        w.write_u32(self.retransmit_sequence_number);
        Ok(())
    }
}

/// `RepublishResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct RepublishResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
    /// The retransmitted NotificationMessage.
    pub notification_message: NotificationMessage,
}

impl RepublishResponse {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            response_header: decode_response_header(r)?,
            notification_message: NotificationMessage::decode(r)?,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::REPUBLISH_RESPONSE.encode(w)?;
        encode_response_header(&self.response_header, w)?;
        self.notification_message.encode(w)?;
        Ok(())
    }
}

/// `TransferResult` (Part 4 §5.14.7.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferResult {
    /// Per-subscription StatusCode.
    pub status_code: u32,
    /// Sequence numbers still in the retransmission queue.
    pub available_sequence_numbers: Vec<u32>,
}

impl UaEncode for TransferResult {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        w.write_u32(self.status_code);
        write_u32_array(w, &self.available_sequence_numbers)?;
        Ok(())
    }
}

impl UaDecode for TransferResult {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            status_code: r.read_u32()?,
            available_sequence_numbers: read_u32_array(r)?,
        })
    }
}

/// `TransferSubscriptionsRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct TransferSubscriptionsRequest {
    /// Common request header.
    pub request_header: RequestHeader,
    /// The subscriptions to transfer to this session.
    pub subscription_ids: Vec<u32>,
    /// Whether the first Publish after the transfer repeats current values.
    pub send_initial_values: bool,
}

impl TransferSubscriptionsRequest {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            request_header: decode_request_header(r)?,
            subscription_ids: read_u32_array(r)?,
            send_initial_values: r.read_u8()? != 0,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::TRANSFER_SUBSCRIPTIONS_REQUEST.encode(w)?;
        encode_request_header(&self.request_header, w)?;
        write_u32_array(w, &self.subscription_ids)?;
        w.write_u8(u8::from(self.send_initial_values));
        Ok(())
    }
}

/// `TransferSubscriptionsResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct TransferSubscriptionsResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
    /// One result per subscription.
    pub results: Vec<TransferResult>,
}

impl TransferSubscriptionsResponse {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let response_header = decode_response_header(r)?;
        let results = read_array::<TransferResult>(r)?;
        skip_diagnostic_info_array(r)?;
        Ok(Self {
            response_header,
            results,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::TRANSFER_SUBSCRIPTIONS_RESPONSE.encode(w)?;
        encode_response_header(&self.response_header, w)?;
        write_array(w, &self.results, "Results")?;
        write_empty_diagnostic_info_array(w);
        Ok(())
    }
}

/// `StatusChangeNotification` (Part 4 §7.25.4) — the notification a
/// Subscription sends when its status changes (e.g. `Bad_Timeout` on lifetime
/// expiry, `Good_SubscriptionTransferred` to the session it left).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusChangeNotification {
    /// The StatusCode that indicates the status change.
    pub status: u32,
}

impl StatusChangeNotification {
    /// Wraps this notification in its `ExtensionObject` (TypeId 820).
    ///
    /// # Errors
    /// [`EncodeError`] on an oversized field.
    pub fn to_extension_object(&self) -> Result<ExtensionObject, EncodeError> {
        let mut w = UaWriter::new();
        w.write_u32(self.status);
        // The DiagnosticInfo of the status change; we emit the empty form.
        w.write_u8(0);
        Ok(ExtensionObject {
            type_id: node_ids::STATUS_CHANGE_NOTIFICATION,
            body: ExtensionObjectBody::ByteString(w.into_vec()),
        })
    }

    /// Recovers a `StatusChangeNotification` from its `ExtensionObject`.
    ///
    /// # Errors
    /// [`DecodeError`] if the body is not a matching ByteString.
    pub fn from_extension_object(eo: &ExtensionObject) -> Result<Self, DecodeError> {
        match &eo.body {
            ExtensionObjectBody::ByteString(b) => {
                let mut r = UaReader::new(b);
                let status = r.read_u32()?;
                crate::wire::skip_one_diagnostic_info(&mut r)?;
                Ok(Self { status })
            }
            _ => Err(DecodeError::MalformedMessage {
                message: "StatusChangeNotification ExtensionObject is not a ByteString body",
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// HistoryRead (Part 4 §5.10.3; the HistoryReadDetails/HistoryData shapes are
// OPC UA Part 11 §6 — Historical Access).
// ---------------------------------------------------------------------------

/// `ReadRawModifiedDetails` (Part 11 §6.4.3, Table 35) — a time-range read.
/// `is_read_modified` names the "Read Modified" function (superseded
/// values a Server kept after an update) — DDS has no concept of a sample
/// being modified after the fact, only disposed/replaced, so this gateway
/// always writes/expects `false` and [`crate::server::Server`] refuses a
/// request with it `true` (`Bad_HistoryOperationUnsupported`) rather than
/// silently answering as if it were a plain raw read.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadRawModifiedDetails {
    /// `true` = Read Modified (not supported here), `false` = Read Raw.
    pub is_read_modified: bool,
    /// Start of the requested period (`UtcTime`, DateTime ticks).
    pub start_time: i64,
    /// End of the requested period.
    pub end_time: i64,
    /// Maximum number of values returned for any one node (`0` = unlimited).
    pub num_values_per_node: u32,
    /// Whether boundary values outside the range should be included.
    pub return_bounds: bool,
}

impl ReadRawModifiedDetails {
    fn to_extension_object(&self) -> Result<ExtensionObject, EncodeError> {
        let mut w = UaWriter::new();
        write_bool(&mut w, self.is_read_modified);
        w.write_i64(self.start_time);
        w.write_i64(self.end_time);
        w.write_u32(self.num_values_per_node);
        write_bool(&mut w, self.return_bounds);
        Ok(ExtensionObject {
            type_id: node_ids::READ_RAW_MODIFIED_DETAILS,
            body: ExtensionObjectBody::ByteString(w.into_vec()),
        })
    }

    fn from_extension_object(eo: &ExtensionObject) -> Result<Self, DecodeError> {
        match &eo.body {
            ExtensionObjectBody::ByteString(b) => {
                let mut r = UaReader::new(b);
                Ok(Self {
                    is_read_modified: read_bool(&mut r)?,
                    start_time: r.read_i64()?,
                    end_time: r.read_i64()?,
                    num_values_per_node: r.read_u32()?,
                    return_bounds: read_bool(&mut r)?,
                })
            }
            _ => Err(DecodeError::MalformedMessage {
                message: "ReadRawModifiedDetails ExtensionObject is not a ByteString body",
            }),
        }
    }
}

/// `ReadAtTimeDetails` (Part 11 §6.4.4/§6.5.5, Table 36).
#[derive(Debug, Clone, PartialEq)]
pub struct ReadAtTimeDetails {
    /// The specific timestamps to read a value at.
    pub req_times: Vec<i64>,
    /// Whether "simple bounds" interpolation applies (this gateway always
    /// returns the nearest prior sample, the simple-bounds behaviour,
    /// regardless of this flag — DDS keeps no continuous function to
    /// interpolate between samples).
    pub use_simple_bounds: bool,
}

impl ReadAtTimeDetails {
    fn to_extension_object(&self) -> Result<ExtensionObject, EncodeError> {
        let mut w = UaWriter::new();
        write_i64_array(&mut w, &self.req_times)?;
        write_bool(&mut w, self.use_simple_bounds);
        Ok(ExtensionObject {
            type_id: node_ids::READ_AT_TIME_DETAILS,
            body: ExtensionObjectBody::ByteString(w.into_vec()),
        })
    }

    fn from_extension_object(eo: &ExtensionObject) -> Result<Self, DecodeError> {
        match &eo.body {
            ExtensionObjectBody::ByteString(b) => {
                let mut r = UaReader::new(b);
                Ok(Self {
                    req_times: read_i64_array(&mut r)?,
                    use_simple_bounds: read_bool(&mut r)?,
                })
            }
            _ => Err(DecodeError::MalformedMessage {
                message: "ReadAtTimeDetails ExtensionObject is not a ByteString body",
            }),
        }
    }
}

/// `HistoryReadDetails` (Part 11 §6.4, Table 34) — the extensible parameter
/// naming which HistoryRead function to perform. DDS has no "Processed"
/// (aggregate) or "Event" history, so only the two variants a DDS-OPCUA
/// gateway can serve are named here; a request naming another one is a
/// [`DecodeError::MalformedMessage`], never silently misread as one of
/// these two.
#[derive(Debug, Clone, PartialEq)]
pub enum HistoryReadDetails {
    /// `ReadRawModifiedDetails`.
    RawModified(ReadRawModifiedDetails),
    /// `ReadAtTimeDetails`.
    AtTime(ReadAtTimeDetails),
}

impl HistoryReadDetails {
    fn to_extension_object(&self) -> Result<ExtensionObject, EncodeError> {
        match self {
            Self::RawModified(d) => d.to_extension_object(),
            Self::AtTime(d) => d.to_extension_object(),
        }
    }

    fn from_extension_object(eo: &ExtensionObject) -> Result<Self, DecodeError> {
        if eo.type_id == node_ids::READ_RAW_MODIFIED_DETAILS {
            Ok(Self::RawModified(
                ReadRawModifiedDetails::from_extension_object(eo)?,
            ))
        } else if eo.type_id == node_ids::READ_AT_TIME_DETAILS {
            Ok(Self::AtTime(ReadAtTimeDetails::from_extension_object(eo)?))
        } else {
            Err(DecodeError::MalformedMessage {
                message: "HistoryReadDetails names a HistoryRead function this gateway does not implement (only ReadRaw/ReadAtTime, no Processed/Event/Modified history)",
            })
        }
    }
}

/// `HistoryReadValueId` (Part 4 §5.10.3, Table 33).
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryReadValueId {
    /// Node to read history from.
    pub node_id: NodeId,
    /// Index range (empty = whole value).
    pub index_range: String,
    /// Requested data encoding.
    pub data_encoding: QualifiedName,
    /// Continuation point from a previous HistoryRead on this node, or empty.
    pub continuation_point: Vec<u8>,
}

impl UaEncode for HistoryReadValueId {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        self.node_id.encode(w)?;
        write_string(w, &self.index_range)?;
        self.data_encoding.encode(w)?;
        write_byte_string(w, &self.continuation_point)?;
        Ok(())
    }
}

impl UaDecode for HistoryReadValueId {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            node_id: NodeId::decode(r)?,
            index_range: read_string(r)?,
            data_encoding: QualifiedName::decode(r)?,
            continuation_point: read_byte_string(r)?,
        })
    }
}

/// `HistoryData` (Part 11 §6.6.2, Table 47) — the historical values for one
/// HistoricalDataNode. DDS has no Events, so this gateway never returns the
/// sibling `HistoryEvent` structure.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HistoryData {
    /// The historical samples, in the order the query returned them.
    pub data_values: Vec<DataValue>,
}

impl HistoryData {
    fn to_extension_object(&self) -> Result<ExtensionObject, EncodeError> {
        let mut w = UaWriter::new();
        write_array(&mut w, &self.data_values, "DataValues")?;
        Ok(ExtensionObject {
            type_id: node_ids::HISTORY_DATA,
            body: ExtensionObjectBody::ByteString(w.into_vec()),
        })
    }

    fn from_extension_object(eo: &ExtensionObject) -> Result<Self, DecodeError> {
        match &eo.body {
            ExtensionObjectBody::ByteString(b) => {
                let mut r = UaReader::new(b);
                Ok(Self {
                    data_values: read_array::<DataValue>(&mut r)?,
                })
            }
            _ => Err(DecodeError::MalformedMessage {
                message: "HistoryData ExtensionObject is not a ByteString body",
            }),
        }
    }
}

/// `HistoryReadResult` (Part 4 §5.10.3, Table 32).
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryReadResult {
    /// Per-node operation status.
    pub status_code: u32,
    /// Set when more data remains — pass back on the next HistoryRead to
    /// continue.
    pub continuation_point: Vec<u8>,
    /// The historical samples this call returned.
    pub history_data: HistoryData,
}

impl UaEncode for HistoryReadResult {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        w.write_u32(self.status_code);
        write_byte_string(w, &self.continuation_point)?;
        self.history_data.to_extension_object()?.encode(w)?;
        Ok(())
    }
}

impl UaDecode for HistoryReadResult {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let status_code = r.read_u32()?;
        let continuation_point = read_byte_string(r)?;
        let eo = ExtensionObject::decode(r)?;
        let history_data = if eo.type_id == node_ids::HISTORY_DATA {
            HistoryData::from_extension_object(&eo)?
        } else {
            HistoryData::default()
        };
        Ok(Self {
            status_code,
            continuation_point,
            history_data,
        })
    }
}

/// `HistoryReadRequest` (Part 4 §5.10.3, Table 31).
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryReadRequest {
    /// Common request header.
    pub request_header: RequestHeader,
    /// Which HistoryRead function to perform.
    pub history_read_details: HistoryReadDetails,
    /// TimestampsToReturn (0 Source / 1 Server / 2 Both / 3 Neither).
    pub timestamps_to_return: i32,
    /// Whether continuation points named below should be released instead
    /// of continued.
    pub release_continuation_points: bool,
    /// Nodes/attributes to read history from.
    pub nodes_to_read: Vec<HistoryReadValueId>,
}

impl HistoryReadRequest {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let request_header = decode_request_header(r)?;
        let details_eo = ExtensionObject::decode(r)?;
        let history_read_details = HistoryReadDetails::from_extension_object(&details_eo)?;
        let timestamps_to_return = r.read_i32()?;
        let release_continuation_points = read_bool(r)?;
        let nodes_to_read = read_array::<HistoryReadValueId>(r)?;
        Ok(Self {
            request_header,
            history_read_details,
            timestamps_to_return,
            release_continuation_points,
            nodes_to_read,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::HISTORY_READ_REQUEST.encode(w)?;
        encode_request_header(&self.request_header, w)?;
        self.history_read_details.to_extension_object()?.encode(w)?;
        w.write_i32(self.timestamps_to_return);
        write_bool(w, self.release_continuation_points);
        write_array(w, &self.nodes_to_read, "NodesToRead")?;
        Ok(())
    }
}

/// `HistoryReadResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryReadResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
    /// One result per requested node.
    pub results: Vec<HistoryReadResult>,
}

impl HistoryReadResponse {
    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let response_header = decode_response_header(r)?;
        let results = read_array::<HistoryReadResult>(r)?;
        skip_diagnostic_info_array(r)?;
        Ok(Self {
            response_header,
            results,
        })
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::HISTORY_READ_RESPONSE.encode(w)?;
        encode_response_header(&self.response_header, w)?;
        write_array(w, &self.results, "Results")?;
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
    /// BrowseNext.
    BrowseNext(BrowseNextRequest),
    /// GetEndpoints.
    GetEndpoints(GetEndpointsRequest),
    /// FindServers.
    FindServers(FindServersRequest),
    /// CreateSubscription.
    CreateSubscription(CreateSubscriptionRequest),
    /// ModifySubscription.
    ModifySubscription(ModifySubscriptionRequest),
    /// SetPublishingMode.
    SetPublishingMode(SetPublishingModeRequest),
    /// CreateMonitoredItems.
    CreateMonitoredItems(CreateMonitoredItemsRequest),
    /// ModifyMonitoredItems.
    ModifyMonitoredItems(ModifyMonitoredItemsRequest),
    /// SetMonitoringMode.
    SetMonitoringMode(SetMonitoringModeRequest),
    /// SetTriggering.
    SetTriggering(SetTriggeringRequest),
    /// DeleteMonitoredItems.
    DeleteMonitoredItems(DeleteMonitoredItemsRequest),
    /// Publish.
    Publish(PublishRequest),
    /// Republish.
    Republish(RepublishRequest),
    /// TransferSubscriptions.
    TransferSubscriptions(TransferSubscriptionsRequest),
    /// DeleteSubscriptions.
    DeleteSubscriptions(DeleteSubscriptionsRequest),
    /// Call.
    Call(CallRequest),
    /// HistoryRead.
    HistoryRead(HistoryReadRequest),
}

impl ServiceRequest {
    /// The request header every service message carries (Part 4 §7.33).
    #[must_use]
    pub const fn request_header(&self) -> &RequestHeader {
        match self {
            Self::CreateSession(m) => &m.request_header,
            Self::ActivateSession(m) => &m.request_header,
            Self::CloseSession(m) => &m.request_header,
            Self::Read(m) => &m.request_header,
            Self::Write(m) => &m.request_header,
            Self::Browse(m) => &m.request_header,
            Self::BrowseNext(m) => &m.request_header,
            Self::GetEndpoints(m) => &m.request_header,
            Self::FindServers(m) => &m.request_header,
            Self::CreateSubscription(m) => &m.request_header,
            Self::ModifySubscription(m) => &m.request_header,
            Self::SetPublishingMode(m) => &m.request_header,
            Self::CreateMonitoredItems(m) => &m.request_header,
            Self::ModifyMonitoredItems(m) => &m.request_header,
            Self::SetMonitoringMode(m) => &m.request_header,
            Self::SetTriggering(m) => &m.request_header,
            Self::DeleteMonitoredItems(m) => &m.request_header,
            Self::Publish(m) => &m.request_header,
            Self::Republish(m) => &m.request_header,
            Self::TransferSubscriptions(m) => &m.request_header,
            Self::DeleteSubscriptions(m) => &m.request_header,
            Self::Call(m) => &m.request_header,
            Self::HistoryRead(m) => &m.request_header,
        }
    }

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
            Self::BrowseNext(m) => m.encode_into(&mut w)?,
            Self::GetEndpoints(m) => m.encode_into(&mut w)?,
            Self::FindServers(m) => m.encode_into(&mut w)?,
            Self::CreateSubscription(m) => m.encode_into(&mut w)?,
            Self::ModifySubscription(m) => m.encode_into(&mut w)?,
            Self::SetPublishingMode(m) => m.encode_into(&mut w)?,
            Self::CreateMonitoredItems(m) => m.encode_into(&mut w)?,
            Self::ModifyMonitoredItems(m) => m.encode_into(&mut w)?,
            Self::SetMonitoringMode(m) => m.encode_into(&mut w)?,
            Self::SetTriggering(m) => m.encode_into(&mut w)?,
            Self::DeleteMonitoredItems(m) => m.encode_into(&mut w)?,
            Self::Publish(m) => m.encode_into(&mut w)?,
            Self::Republish(m) => m.encode_into(&mut w)?,
            Self::TransferSubscriptions(m) => m.encode_into(&mut w)?,
            Self::DeleteSubscriptions(m) => m.encode_into(&mut w)?,
            Self::Call(m) => m.encode_into(&mut w)?,
            Self::HistoryRead(m) => m.encode_into(&mut w)?,
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
        } else if type_id == node_ids::BROWSE_NEXT_REQUEST {
            Self::BrowseNext(BrowseNextRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::GET_ENDPOINTS_REQUEST {
            Self::GetEndpoints(GetEndpointsRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::FIND_SERVERS_REQUEST {
            Self::FindServers(FindServersRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::CREATE_SUBSCRIPTION_REQUEST {
            Self::CreateSubscription(CreateSubscriptionRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::MODIFY_SUBSCRIPTION_REQUEST {
            Self::ModifySubscription(ModifySubscriptionRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::SET_PUBLISHING_MODE_REQUEST {
            Self::SetPublishingMode(SetPublishingModeRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::CREATE_MONITORED_ITEMS_REQUEST {
            Self::CreateMonitoredItems(CreateMonitoredItemsRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::MODIFY_MONITORED_ITEMS_REQUEST {
            Self::ModifyMonitoredItems(ModifyMonitoredItemsRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::SET_MONITORING_MODE_REQUEST {
            Self::SetMonitoringMode(SetMonitoringModeRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::SET_TRIGGERING_REQUEST {
            Self::SetTriggering(SetTriggeringRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::DELETE_MONITORED_ITEMS_REQUEST {
            Self::DeleteMonitoredItems(DeleteMonitoredItemsRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::PUBLISH_REQUEST {
            Self::Publish(PublishRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::REPUBLISH_REQUEST {
            Self::Republish(RepublishRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::TRANSFER_SUBSCRIPTIONS_REQUEST {
            Self::TransferSubscriptions(TransferSubscriptionsRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::DELETE_SUBSCRIPTIONS_REQUEST {
            Self::DeleteSubscriptions(DeleteSubscriptionsRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::CALL_REQUEST {
            Self::Call(CallRequest::decode_body(&mut r)?)
        } else if type_id == node_ids::HISTORY_READ_REQUEST {
            Self::HistoryRead(HistoryReadRequest::decode_body(&mut r)?)
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
    /// BrowseNext.
    BrowseNext(BrowseNextResponse),
    /// GetEndpoints.
    GetEndpoints(GetEndpointsResponse),
    /// FindServers.
    FindServers(FindServersResponse),
    /// CreateSubscription.
    CreateSubscription(CreateSubscriptionResponse),
    /// ModifySubscription.
    ModifySubscription(ModifySubscriptionResponse),
    /// SetPublishingMode.
    SetPublishingMode(SetPublishingModeResponse),
    /// CreateMonitoredItems.
    CreateMonitoredItems(CreateMonitoredItemsResponse),
    /// ModifyMonitoredItems.
    ModifyMonitoredItems(ModifyMonitoredItemsResponse),
    /// SetMonitoringMode.
    SetMonitoringMode(SetMonitoringModeResponse),
    /// SetTriggering.
    SetTriggering(SetTriggeringResponse),
    /// DeleteMonitoredItems.
    DeleteMonitoredItems(DeleteMonitoredItemsResponse),
    /// Publish.
    Publish(PublishResponse),
    /// Republish.
    Republish(RepublishResponse),
    /// TransferSubscriptions.
    TransferSubscriptions(TransferSubscriptionsResponse),
    /// DeleteSubscriptions.
    DeleteSubscriptions(DeleteSubscriptionsResponse),
    /// Call.
    Call(CallResponse),
    /// HistoryRead.
    HistoryRead(HistoryReadResponse),
    /// ServiceFault (Part 4 §7.34) — the response a server sends when it
    /// rejects a request outright. Without this variant every rejection a
    /// foreign server sends decodes as "unsupported service response type"
    /// and the status code it carries is lost.
    ServiceFault(ServiceFault),
}

/// Part 4 §7.34 — a ServiceFault carries nothing but a `ResponseHeader`; the
/// rejection reason is its `service_result`.
#[derive(Debug, Clone, PartialEq)]
pub struct ServiceFault {
    /// The response header, whose `service_result` is the rejection reason.
    pub response_header: ResponseHeader,
}

impl ServiceFault {
    /// A fault answering `request_handle` with `service_result`.
    #[must_use]
    pub fn new(request_handle: u32, service_result: u32) -> Self {
        Self {
            response_header: ResponseHeader::new(request_handle, service_result),
        }
    }

    fn encode_into(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        node_ids::SERVICE_FAULT.encode(w)?;
        encode_response_header(&self.response_header, w)
    }

    fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            response_header: decode_response_header(r)?,
        })
    }
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
            Self::BrowseNext(m) => m.encode_into(&mut w)?,
            Self::GetEndpoints(m) => m.encode_into(&mut w)?,
            Self::FindServers(m) => m.encode_into(&mut w)?,
            Self::CreateSubscription(m) => m.encode_into(&mut w)?,
            Self::ModifySubscription(m) => m.encode_into(&mut w)?,
            Self::SetPublishingMode(m) => m.encode_into(&mut w)?,
            Self::CreateMonitoredItems(m) => m.encode_into(&mut w)?,
            Self::ModifyMonitoredItems(m) => m.encode_into(&mut w)?,
            Self::SetMonitoringMode(m) => m.encode_into(&mut w)?,
            Self::SetTriggering(m) => m.encode_into(&mut w)?,
            Self::DeleteMonitoredItems(m) => m.encode_into(&mut w)?,
            Self::Publish(m) => m.encode_into(&mut w)?,
            Self::Republish(m) => m.encode_into(&mut w)?,
            Self::TransferSubscriptions(m) => m.encode_into(&mut w)?,
            Self::DeleteSubscriptions(m) => m.encode_into(&mut w)?,
            Self::Call(m) => m.encode_into(&mut w)?,
            Self::HistoryRead(m) => m.encode_into(&mut w)?,
            Self::ServiceFault(m) => m.encode_into(&mut w)?,
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
        } else if type_id == node_ids::BROWSE_NEXT_RESPONSE {
            Self::BrowseNext(BrowseNextResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::GET_ENDPOINTS_RESPONSE {
            Self::GetEndpoints(GetEndpointsResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::FIND_SERVERS_RESPONSE {
            Self::FindServers(FindServersResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::CREATE_SUBSCRIPTION_RESPONSE {
            Self::CreateSubscription(CreateSubscriptionResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::MODIFY_SUBSCRIPTION_RESPONSE {
            Self::ModifySubscription(ModifySubscriptionResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::SET_PUBLISHING_MODE_RESPONSE {
            Self::SetPublishingMode(SetPublishingModeResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::CREATE_MONITORED_ITEMS_RESPONSE {
            Self::CreateMonitoredItems(CreateMonitoredItemsResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::MODIFY_MONITORED_ITEMS_RESPONSE {
            Self::ModifyMonitoredItems(ModifyMonitoredItemsResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::SET_MONITORING_MODE_RESPONSE {
            Self::SetMonitoringMode(SetMonitoringModeResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::SET_TRIGGERING_RESPONSE {
            Self::SetTriggering(SetTriggeringResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::DELETE_MONITORED_ITEMS_RESPONSE {
            Self::DeleteMonitoredItems(DeleteMonitoredItemsResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::PUBLISH_RESPONSE {
            Self::Publish(PublishResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::REPUBLISH_RESPONSE {
            Self::Republish(RepublishResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::TRANSFER_SUBSCRIPTIONS_RESPONSE {
            Self::TransferSubscriptions(TransferSubscriptionsResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::DELETE_SUBSCRIPTIONS_RESPONSE {
            Self::DeleteSubscriptions(DeleteSubscriptionsResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::CALL_RESPONSE {
            Self::Call(CallResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::HISTORY_READ_RESPONSE {
            Self::HistoryRead(HistoryReadResponse::decode_body(&mut r)?)
        } else if type_id == node_ids::SERVICE_FAULT {
            Self::ServiceFault(ServiceFault::decode_body(&mut r)?)
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
    fn history_read_raw_round_trips() {
        let req = ServiceRequest::HistoryRead(HistoryReadRequest {
            request_header: req_header(),
            history_read_details: HistoryReadDetails::RawModified(ReadRawModifiedDetails {
                is_read_modified: false,
                start_time: 100,
                end_time: 200,
                num_values_per_node: 10,
                return_bounds: true,
            }),
            timestamps_to_return: 0,
            release_continuation_points: false,
            nodes_to_read: alloc::vec![HistoryReadValueId {
                node_id: NodeId::numeric(1, 42),
                index_range: String::new(),
                data_encoding: QualifiedName {
                    namespace_index: 0,
                    name: String::new(),
                },
                continuation_point: alloc::vec![1, 2, 3],
            }],
        });
        let body = req.encode().expect("encode");
        assert_eq!(ServiceRequest::decode(&body).expect("decode"), req);

        let resp = ServiceResponse::HistoryRead(HistoryReadResponse {
            response_header: ResponseHeader::new(1, 0),
            results: alloc::vec![HistoryReadResult {
                status_code: 0,
                continuation_point: Vec::new(),
                history_data: HistoryData {
                    data_values: alloc::vec![DataValue::new_value(
                        Variant::scalar(VariantValue::Int32(7)),
                        0,
                        123,
                    )],
                },
            }],
        });
        let rb = resp.encode().expect("encode");
        assert_eq!(ServiceResponse::decode(&rb).expect("decode"), resp);
    }

    #[test]
    fn history_read_at_time_round_trips() {
        let req = ServiceRequest::HistoryRead(HistoryReadRequest {
            request_header: req_header(),
            history_read_details: HistoryReadDetails::AtTime(ReadAtTimeDetails {
                req_times: alloc::vec![10, 20, 30],
                use_simple_bounds: true,
            }),
            timestamps_to_return: 2,
            release_continuation_points: true,
            nodes_to_read: alloc::vec![HistoryReadValueId {
                node_id: NodeId::numeric(1, 43),
                index_range: String::new(),
                data_encoding: QualifiedName {
                    namespace_index: 0,
                    name: String::new(),
                },
                continuation_point: Vec::new(),
            }],
        });
        let body = req.encode().expect("encode");
        assert_eq!(ServiceRequest::decode(&body).expect("decode"), req);
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
