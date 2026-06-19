// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! UADP discovery (OPC Foundation Part 14 §7.2.2.4) — the request/response
//! messages a Subscriber and Publisher exchange to learn each other's
//! DataSet layout, plus the on-wire serialisation of [`DataSetMetaData`].
//!
//! A discovery message is a UADP NetworkMessage whose ExtendedFlags2
//! NetworkMessage type is `DiscoveryRequest` (1) or `DiscoveryResponse` (2)
//! instead of the DataSetMessage type (0). The header (version, PublisherId)
//! is shared with [`crate::uadp::network_message`]; the payload differs.
//!
//! # Scope
//!
//! All three [`InformationType`] exchanges are implemented — request plus
//! response:
//!
//! - `DataSetMetaData`: the complete `DataSetMetaDataType` wire codec — field
//!   descriptions *and* custom DataType descriptions (`StructureDescription` /
//!   `EnumDescription` / `SimpleTypeDescription`) all round-trip. Decoding a
//!   RawData *value* of a custom type still depends on dynamic
//!   structured-type support in `zerodds-opcua-gateway`.
//! - `PublisherEndpoints`: an [`EndpointDescription`] list (Part 4 type).
//! - `DataSetWriterConfiguration`: a [`WriterGroupDataType`] config.

use alloc::vec::Vec;

use zerodds_opcua_gateway::data_value::Variant;
use zerodds_opcua_gateway::node_id::NodeId;
use zerodds_opcua_gateway::types::{Guid, LocalizedText, QualifiedName, StatusCode};

use crate::binary::{
    UaDecode, UaEncode, UaReader, UaWriter, builtin_type_from_value, read_array, read_string,
    read_string_array, read_u16_array, read_u32_array, write_array, write_string,
    write_string_array, write_u16_array, write_u32_array,
};
use crate::config::{
    ConfigurationVersion, DataSetMetaData, EnumDefinition, EnumDescription, EnumField,
    FieldMetaData, KeyValuePair, SimpleTypeDescription, StructureDefinition, StructureDescription,
    StructureField, StructureType,
};
use crate::error::{DecodeError, EncodeError};
use crate::uadp::datatypes::{EndpointDescription, WriterGroupDataType};
use crate::uadp::network_message::{PublisherId, read_discovery_header, write_discovery_header};

const NM_TYPE_DISCOVERY_REQUEST: u8 = 1;
const NM_TYPE_DISCOVERY_RESPONSE: u8 = 2;

/// The kind of information a discovery request asks for / a response carries
/// (Part 14 §7.2.2.4.2.1, `InformationType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InformationType {
    /// The Publisher's endpoint descriptions (type 1).
    PublisherEndpoints,
    /// DataSet metadata for the requested DataSetWriters (type 2).
    DataSetMetaData,
    /// Full DataSetWriter/WriterGroup configuration (type 3).
    DataSetWriterConfiguration,
}

impl InformationType {
    const fn to_u8(self) -> u8 {
        match self {
            Self::PublisherEndpoints => 1,
            Self::DataSetMetaData => 2,
            Self::DataSetWriterConfiguration => 3,
        }
    }

    const fn from_u8(v: u8) -> Result<Self, DecodeError> {
        match v {
            1 => Ok(Self::PublisherEndpoints),
            2 => Ok(Self::DataSetMetaData),
            3 => Ok(Self::DataSetWriterConfiguration),
            other => Err(DecodeError::InvalidDiscriminant {
                field: "discovery InformationType",
                value: other as u32,
            }),
        }
    }
}

/// A UADP DiscoveryRequest (Part 14 §7.2.2.4.2).
#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveryRequest {
    /// Publisher the request is directed at (`None` = broadcast).
    pub publisher_id: Option<PublisherId>,
    /// What is being requested.
    pub information_type: InformationType,
    /// DataSetWriter ids the request targets (empty = all; unused for
    /// `PublisherEndpoints`).
    pub data_set_writer_ids: Vec<u16>,
}

impl UaEncode for DiscoveryRequest {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        write_discovery_header(w, NM_TYPE_DISCOVERY_REQUEST, self.publisher_id.as_ref())?;
        w.write_u8(self.information_type.to_u8());
        if self.information_type != InformationType::PublisherEndpoints {
            write_u16_array(w, &self.data_set_writer_ids)?;
        }
        Ok(())
    }
}

impl UaDecode for DiscoveryRequest {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let (nm_type, publisher_id) = read_discovery_header(r)?;
        if nm_type != NM_TYPE_DISCOVERY_REQUEST {
            return Err(DecodeError::MalformedMessage {
                message: "NetworkMessage is not a DiscoveryRequest",
            });
        }
        let information_type = InformationType::from_u8(r.read_u8()?)?;
        let data_set_writer_ids = if information_type == InformationType::PublisherEndpoints {
            Vec::new()
        } else {
            read_u16_array(r)?
        };
        Ok(Self {
            publisher_id,
            information_type,
            data_set_writer_ids,
        })
    }
}

/// The DataSetMetaData payload of a DiscoveryResponse (Part 14 §7.2.2.4.3.3).
#[derive(Debug, Clone, PartialEq)]
pub struct DataSetMetaDataResponse {
    /// DataSetWriter the metadata describes.
    pub data_set_writer_id: u16,
    /// The DataSet layout.
    pub meta_data: DataSetMetaData,
    /// Operation result (`0` = Good).
    pub status_code: StatusCode,
}

/// The PublisherEndpoints payload of a DiscoveryResponse (Part 14
/// §7.2.2.4.3.2).
#[derive(Debug, Clone, PartialEq)]
pub struct PublisherEndpointsResponse {
    /// The Publisher's endpoints.
    pub endpoints: Vec<EndpointDescription>,
    /// Operation result (`0` = Good).
    pub status_code: StatusCode,
}

/// The DataSetWriterConfiguration payload of a DiscoveryResponse (Part 14
/// §7.2.2.4.3.4).
#[derive(Debug, Clone, PartialEq)]
pub struct DataSetWriterConfigurationResponse {
    /// DataSetWriter ids the configuration covers.
    pub data_set_writer_ids: Vec<u16>,
    /// The WriterGroup configuration.
    pub config: WriterGroupDataType,
    /// Per-writer operation results.
    pub statuses: Vec<StatusCode>,
}

/// The body of a [`DiscoveryResponse`] — one per [`InformationType`].
#[derive(Debug, Clone, PartialEq)]
pub enum DiscoveryResponsePayload {
    /// A PublisherEndpoints response.
    PublisherEndpoints(PublisherEndpointsResponse),
    /// A DataSetMetaData response.
    DataSetMetaData(DataSetMetaDataResponse),
    /// A DataSetWriterConfiguration response.
    DataSetWriterConfiguration(DataSetWriterConfigurationResponse),
}

/// A UADP DiscoveryResponse (Part 14 §7.2.2.4.3).
#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveryResponse {
    /// Publisher that produced the response.
    pub publisher_id: Option<PublisherId>,
    /// Rolling response sequence number.
    pub sequence_number: u16,
    /// The response body.
    pub payload: DiscoveryResponsePayload,
}

impl UaEncode for DiscoveryResponse {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        write_discovery_header(w, NM_TYPE_DISCOVERY_RESPONSE, self.publisher_id.as_ref())?;
        let response_type = match &self.payload {
            DiscoveryResponsePayload::PublisherEndpoints(_) => InformationType::PublisherEndpoints,
            DiscoveryResponsePayload::DataSetMetaData(_) => InformationType::DataSetMetaData,
            DiscoveryResponsePayload::DataSetWriterConfiguration(_) => {
                InformationType::DataSetWriterConfiguration
            }
        };
        w.write_u8(response_type.to_u8());
        w.write_u16(self.sequence_number);
        match &self.payload {
            DiscoveryResponsePayload::PublisherEndpoints(resp) => {
                write_array(w, &resp.endpoints, "Endpoints")?;
                w.write_u32(resp.status_code);
            }
            DiscoveryResponsePayload::DataSetMetaData(resp) => {
                w.write_u16(resp.data_set_writer_id);
                resp.meta_data.encode(w)?;
                w.write_u32(resp.status_code);
            }
            DiscoveryResponsePayload::DataSetWriterConfiguration(resp) => {
                write_u16_array(w, &resp.data_set_writer_ids)?;
                resp.config.encode(w)?;
                write_u32_array(w, &resp.statuses)?;
            }
        }
        Ok(())
    }
}

impl UaDecode for DiscoveryResponse {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let (nm_type, publisher_id) = read_discovery_header(r)?;
        if nm_type != NM_TYPE_DISCOVERY_RESPONSE {
            return Err(DecodeError::MalformedMessage {
                message: "NetworkMessage is not a DiscoveryResponse",
            });
        }
        let response_type = InformationType::from_u8(r.read_u8()?)?;
        let sequence_number = r.read_u16()?;
        let payload = match response_type {
            InformationType::PublisherEndpoints => {
                let endpoints = read_array::<EndpointDescription>(r)?;
                let status_code = r.read_u32()?;
                DiscoveryResponsePayload::PublisherEndpoints(PublisherEndpointsResponse {
                    endpoints,
                    status_code,
                })
            }
            InformationType::DataSetMetaData => {
                let data_set_writer_id = r.read_u16()?;
                let meta_data = DataSetMetaData::decode(r)?;
                let status_code = r.read_u32()?;
                DiscoveryResponsePayload::DataSetMetaData(DataSetMetaDataResponse {
                    data_set_writer_id,
                    meta_data,
                    status_code,
                })
            }
            InformationType::DataSetWriterConfiguration => {
                let data_set_writer_ids = read_u16_array(r)?;
                let config = WriterGroupDataType::decode(r)?;
                let statuses = read_u32_array(r)?;
                DiscoveryResponsePayload::DataSetWriterConfiguration(
                    DataSetWriterConfigurationResponse {
                        data_set_writer_ids,
                        config,
                        statuses,
                    },
                )
            }
        };
        Ok(Self {
            publisher_id,
            sequence_number,
            payload,
        })
    }
}

// ---------------------------------------------------------------------------
// DataSetMetaDataType and its members (Part 14 §6.2.3.2).
// ---------------------------------------------------------------------------

impl UaEncode for ConfigurationVersion {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        w.write_u32(self.major_version);
        w.write_u32(self.minor_version);
        Ok(())
    }
}

impl UaDecode for ConfigurationVersion {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            major_version: r.read_u32()?,
            minor_version: r.read_u32()?,
        })
    }
}

impl UaEncode for KeyValuePair {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        self.key.encode(w)?;
        self.value.encode(w)?;
        Ok(())
    }
}

impl UaDecode for KeyValuePair {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            key: QualifiedName::decode(r)?,
            value: Variant::decode(r)?,
        })
    }
}

impl UaEncode for FieldMetaData {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        write_string(w, &self.name)?;
        self.description.encode(w)?;
        w.write_u16(self.field_flags);
        w.write_u8(self.builtin_type.value());
        self.data_type.encode(w)?;
        w.write_i32(self.value_rank);
        write_u32_array(w, &self.array_dimensions)?;
        w.write_u32(self.max_string_length);
        self.data_set_field_id.encode(w)?;
        write_array(w, &self.properties, "Properties")?;
        Ok(())
    }
}

impl UaDecode for FieldMetaData {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let name = read_string(r)?;
        let description = LocalizedText::decode(r)?;
        let field_flags = r.read_u16()?;
        let builtin_type = builtin_type_from_value(r.read_u8()?)?;
        let data_type = NodeId::decode(r)?;
        let value_rank = r.read_i32()?;
        let array_dimensions = read_u32_array(r)?;
        let max_string_length = r.read_u32()?;
        let data_set_field_id = Guid::decode(r)?;
        let properties = read_array::<KeyValuePair>(r)?;
        Ok(Self {
            name,
            description,
            field_flags,
            builtin_type,
            data_type,
            value_rank,
            array_dimensions,
            max_string_length,
            data_set_field_id,
            properties,
        })
    }
}

impl UaEncode for DataSetMetaData {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        // DataTypeSchemaHeader: namespaces + custom DataType descriptions.
        write_string_array(w, &self.namespaces)?;
        write_array(w, &self.structure_data_types, "StructureDataTypes")?;
        write_array(w, &self.enum_data_types, "EnumDataTypes")?;
        write_array(w, &self.simple_data_types, "SimpleDataTypes")?;
        // DataSetMetaDataType body.
        write_string(w, &self.name)?;
        self.description.encode(w)?;
        write_array(w, &self.fields, "Fields")?;
        self.data_set_class_id.encode(w)?;
        self.configuration_version.encode(w)?;
        Ok(())
    }
}

impl UaDecode for DataSetMetaData {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let namespaces = read_string_array(r)?;
        let structure_data_types = read_array::<StructureDescription>(r)?;
        let enum_data_types = read_array::<EnumDescription>(r)?;
        let simple_data_types = read_array::<SimpleTypeDescription>(r)?;
        let name = read_string(r)?;
        let description = LocalizedText::decode(r)?;
        let fields = read_array::<FieldMetaData>(r)?;
        let data_set_class_id = Guid::decode(r)?;
        let configuration_version = ConfigurationVersion::decode(r)?;
        Ok(Self {
            name,
            description,
            namespaces,
            structure_data_types,
            enum_data_types,
            simple_data_types,
            fields,
            data_set_class_id,
            configuration_version,
        })
    }
}

impl UaEncode for StructureField {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        write_string(w, &self.name)?;
        self.description.encode(w)?;
        self.data_type.encode(w)?;
        w.write_i32(self.value_rank);
        write_u32_array(w, &self.array_dimensions)?;
        w.write_u32(self.max_string_length);
        w.write_u8(u8::from(self.is_optional));
        Ok(())
    }
}

impl UaDecode for StructureField {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            name: read_string(r)?,
            description: LocalizedText::decode(r)?,
            data_type: NodeId::decode(r)?,
            value_rank: r.read_i32()?,
            array_dimensions: read_u32_array(r)?,
            max_string_length: r.read_u32()?,
            is_optional: r.read_u8()? != 0,
        })
    }
}

impl UaEncode for StructureDefinition {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        self.default_encoding_id.encode(w)?;
        self.base_data_type.encode(w)?;
        w.write_i32(self.structure_type.to_i32());
        write_array(w, &self.fields, "StructureFields")?;
        Ok(())
    }
}

impl UaDecode for StructureDefinition {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let default_encoding_id = NodeId::decode(r)?;
        let base_data_type = NodeId::decode(r)?;
        let raw = r.read_i32()?;
        let structure_type =
            StructureType::from_i32(raw).ok_or(DecodeError::InvalidDiscriminant {
                field: "StructureType",
                value: raw as u32,
            })?;
        let fields = read_array::<StructureField>(r)?;
        Ok(Self {
            default_encoding_id,
            base_data_type,
            structure_type,
            fields,
        })
    }
}

impl UaEncode for StructureDescription {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        self.data_type_id.encode(w)?;
        self.name.encode(w)?;
        self.structure_definition.encode(w)?;
        Ok(())
    }
}

impl UaDecode for StructureDescription {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            data_type_id: NodeId::decode(r)?,
            name: QualifiedName::decode(r)?,
            structure_definition: StructureDefinition::decode(r)?,
        })
    }
}

impl UaEncode for EnumField {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        w.write_i64(self.value);
        self.display_name.encode(w)?;
        self.description.encode(w)?;
        write_string(w, &self.name)?;
        Ok(())
    }
}

impl UaDecode for EnumField {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            value: r.read_i64()?,
            display_name: LocalizedText::decode(r)?,
            description: LocalizedText::decode(r)?,
            name: read_string(r)?,
        })
    }
}

impl UaEncode for EnumDefinition {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        write_array(w, &self.fields, "EnumFields")
    }
}

impl UaDecode for EnumDefinition {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            fields: read_array::<EnumField>(r)?,
        })
    }
}

impl UaEncode for EnumDescription {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        self.data_type_id.encode(w)?;
        self.name.encode(w)?;
        self.enum_definition.encode(w)?;
        w.write_u8(self.builtin_type);
        Ok(())
    }
}

impl UaDecode for EnumDescription {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            data_type_id: NodeId::decode(r)?,
            name: QualifiedName::decode(r)?,
            enum_definition: EnumDefinition::decode(r)?,
            builtin_type: r.read_u8()?,
        })
    }
}

impl UaEncode for SimpleTypeDescription {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        self.data_type_id.encode(w)?;
        self.name.encode(w)?;
        self.base_data_type.encode(w)?;
        w.write_u8(self.builtin_type);
        Ok(())
    }
}

impl UaDecode for SimpleTypeDescription {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            data_type_id: NodeId::decode(r)?,
            name: QualifiedName::decode(r)?,
            base_data_type: NodeId::decode(r)?,
            builtin_type: r.read_u8()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binary::{from_binary, to_binary};
    use alloc::string::String;
    use zerodds_opcua_gateway::data_value::VariantValue;
    use zerodds_opcua_gateway::types::BuiltinTypeKind;

    fn sample_meta() -> DataSetMetaData {
        let mut m = DataSetMetaData::new(
            "ds1",
            alloc::vec![
                FieldMetaData::scalar("a", BuiltinTypeKind::Int32),
                FieldMetaData::scalar("temp", BuiltinTypeKind::Double),
            ],
        );
        m.namespaces = alloc::vec![String::from("urn:example")];
        m.configuration_version = ConfigurationVersion {
            major_version: 3,
            minor_version: 4,
        };
        m
    }

    #[test]
    fn data_set_metadata_round_trip() {
        let m = sample_meta();
        let bytes = to_binary(&m).expect("encode");
        let back: DataSetMetaData = from_binary(&bytes).expect("decode");
        assert_eq!(back, m);
    }

    #[test]
    fn field_metadata_with_properties_round_trip() {
        let mut f = FieldMetaData::scalar("pressure", BuiltinTypeKind::Float);
        f.value_rank = 1;
        f.array_dimensions = alloc::vec![3];
        f.field_flags = 0x0001; // promoted
        f.properties = alloc::vec![KeyValuePair {
            key: QualifiedName {
                namespace_index: 2,
                name: String::from("EngineeringUnits"),
            },
            value: Variant::scalar(VariantValue::String(String::from("bar"))),
        }];
        let m = DataSetMetaData::new("ds", alloc::vec![f.clone()]);
        let back: DataSetMetaData = from_binary(&to_binary(&m).expect("enc")).expect("dec");
        assert_eq!(back.fields[0], f);
        assert!(back.fields[0].is_promoted());
    }

    #[test]
    fn discovery_request_round_trip() {
        let req = DiscoveryRequest {
            publisher_id: Some(PublisherId::UInt16(9)),
            information_type: InformationType::DataSetMetaData,
            data_set_writer_ids: alloc::vec![5, 6, 7],
        };
        let back: DiscoveryRequest = from_binary(&to_binary(&req).expect("enc")).expect("dec");
        assert_eq!(back, req);
    }

    #[test]
    fn publisher_endpoints_request_has_no_writer_ids() {
        let req = DiscoveryRequest {
            publisher_id: None,
            information_type: InformationType::PublisherEndpoints,
            data_set_writer_ids: Vec::new(),
        };
        let back: DiscoveryRequest = from_binary(&to_binary(&req).expect("enc")).expect("dec");
        assert_eq!(back, req);
    }

    #[test]
    fn discovery_response_metadata_round_trip() {
        let resp = DiscoveryResponse {
            publisher_id: Some(PublisherId::String(String::from("pub-1"))),
            sequence_number: 42,
            payload: DiscoveryResponsePayload::DataSetMetaData(DataSetMetaDataResponse {
                data_set_writer_id: 5,
                meta_data: sample_meta(),
                status_code: 0,
            }),
        };
        let back: DiscoveryResponse = from_binary(&to_binary(&resp).expect("enc")).expect("dec");
        assert_eq!(back, resp);
    }

    #[test]
    fn discovery_response_publisher_endpoints_round_trip() {
        use crate::uadp::datatypes::{
            ApplicationDescription, ApplicationType, EndpointDescription, MessageSecurityMode,
        };

        let resp = DiscoveryResponse {
            publisher_id: Some(PublisherId::Byte(1)),
            sequence_number: 7,
            payload: DiscoveryResponsePayload::PublisherEndpoints(PublisherEndpointsResponse {
                endpoints: alloc::vec![EndpointDescription {
                    endpoint_url: String::from("opc.tcp://h:4840"),
                    server: ApplicationDescription {
                        application_uri: String::from("urn:h:app"),
                        product_uri: String::new(),
                        application_name: LocalizedText {
                            locale: None,
                            text: Some(String::from("App")),
                        },
                        application_type: ApplicationType::Server,
                        gateway_server_uri: String::new(),
                        discovery_profile_uri: String::new(),
                        discovery_urls: Vec::new(),
                    },
                    server_certificate: Vec::new(),
                    security_mode: MessageSecurityMode::None,
                    security_policy_uri: String::from("None"),
                    user_identity_tokens: Vec::new(),
                    transport_profile_uri: String::new(),
                    security_level: 0,
                }],
                status_code: 0,
            }),
        };
        let back: DiscoveryResponse = from_binary(&to_binary(&resp).expect("enc")).expect("dec");
        assert_eq!(back, resp);
    }

    #[test]
    fn discovery_response_writer_configuration_round_trip() {
        use crate::uadp::datatypes::{MessageSecurityMode, WriterGroupDataType};
        use zerodds_opcua_gateway::data_value::{ExtensionObject, ExtensionObjectBody};
        use zerodds_opcua_gateway::node_id::NodeId;

        let ext = ExtensionObject {
            type_id: NodeId::numeric(0, 0),
            body: ExtensionObjectBody::None,
        };
        let resp = DiscoveryResponse {
            publisher_id: None,
            sequence_number: 3,
            payload: DiscoveryResponsePayload::DataSetWriterConfiguration(
                DataSetWriterConfigurationResponse {
                    data_set_writer_ids: alloc::vec![5, 6],
                    config: WriterGroupDataType {
                        name: String::from("wg1"),
                        enabled: true,
                        security_mode: MessageSecurityMode::SignAndEncrypt,
                        security_group_id: String::from("sg1"),
                        security_key_services: Vec::new(),
                        max_network_message_size: 1500,
                        group_properties: Vec::new(),
                        writer_group_id: 1,
                        publishing_interval: 50.0,
                        keep_alive_time: 1000.0,
                        priority: 5,
                        locale_ids: Vec::new(),
                        header_layout_uri: String::new(),
                        transport_settings: ext.clone(),
                        message_settings: ext,
                        data_set_writers: Vec::new(),
                    },
                    statuses: alloc::vec![0, 0],
                },
            ),
        };
        let back: DiscoveryResponse = from_binary(&to_binary(&resp).expect("enc")).expect("dec");
        assert_eq!(back, resp);
    }

    #[test]
    fn request_decode_rejects_wrong_message_type() {
        // A DiscoveryResponse decoded as a request must be rejected.
        let resp = DiscoveryResponse {
            publisher_id: None,
            sequence_number: 1,
            payload: DiscoveryResponsePayload::DataSetMetaData(DataSetMetaDataResponse {
                data_set_writer_id: 1,
                meta_data: DataSetMetaData::new("d", Vec::new()),
                status_code: 0,
            }),
        };
        let bytes = to_binary(&resp).expect("enc");
        assert!(matches!(
            from_binary::<DiscoveryRequest>(&bytes),
            Err(DecodeError::MalformedMessage { .. })
        ));
    }

    #[test]
    fn metadata_with_custom_datatypes_round_trips() {
        use crate::config::{
            EnumDefinition, EnumDescription, EnumField, SimpleTypeDescription, StructureDefinition,
            StructureDescription, StructureField, StructureType,
        };
        use zerodds_opcua_gateway::node_id::NodeId;

        let mut m = sample_meta();
        m.structure_data_types = alloc::vec![StructureDescription {
            data_type_id: NodeId::numeric(2, 4000),
            name: QualifiedName {
                namespace_index: 2,
                name: String::from("Point"),
            },
            structure_definition: StructureDefinition {
                default_encoding_id: NodeId::numeric(2, 4001),
                base_data_type: NodeId::numeric(0, 22),
                structure_type: StructureType::StructureWithOptionalFields,
                fields: alloc::vec![StructureField {
                    name: String::from("x"),
                    description: LocalizedText {
                        locale: None,
                        text: None,
                    },
                    data_type: NodeId::numeric(0, 11),
                    value_rank: -1,
                    array_dimensions: Vec::new(),
                    max_string_length: 0,
                    is_optional: true,
                }],
            },
        }];
        m.enum_data_types = alloc::vec![EnumDescription {
            data_type_id: NodeId::numeric(2, 5000),
            name: QualifiedName {
                namespace_index: 2,
                name: String::from("Color"),
            },
            enum_definition: EnumDefinition {
                fields: alloc::vec![EnumField {
                    value: 1,
                    display_name: LocalizedText {
                        locale: None,
                        text: Some(String::from("Red")),
                    },
                    description: LocalizedText {
                        locale: None,
                        text: None,
                    },
                    name: String::from("Red"),
                }],
            },
            builtin_type: 6,
        }];
        m.simple_data_types = alloc::vec![SimpleTypeDescription {
            data_type_id: NodeId::numeric(2, 6000),
            name: QualifiedName {
                namespace_index: 2,
                name: String::from("Celsius"),
            },
            base_data_type: NodeId::numeric(0, 11),
            builtin_type: 11,
        }];

        let back: DataSetMetaData = from_binary(&to_binary(&m).expect("enc")).expect("dec");
        assert_eq!(back, m);
    }
}
