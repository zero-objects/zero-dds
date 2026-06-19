// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Structured OPC-UA types carried by the non-metadata discovery responses
//! (Part 14 §7.2.2.4.3):
//!
//! - [`EndpointDescription`] (+ [`ApplicationDescription`],
//!   [`UserTokenPolicy`] and the security/application enums) for the
//!   `PublisherEndpoints` response — a Client/Server (Part 4) type.
//! - [`WriterGroupDataType`] (+ [`DataSetWriterDataType`]) for the
//!   `DataSetWriterConfiguration` response.
//!
//! These are the OPC-UA binary wire encodings of the OPC Foundation
//! information-model structures, built on the [`crate::binary`] codec and the
//! `zerodds-opcua-gateway` value types.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_opcua_gateway::data_value::ExtensionObject;
use zerodds_opcua_gateway::types::{ByteString, LocalizedText};

use crate::binary::{
    UaDecode, UaEncode, UaReader, UaWriter, read_array, read_byte_string, read_string,
    read_string_array, write_array, write_byte_string, write_string, write_string_array,
};
use crate::config::KeyValuePair;
use crate::error::{DecodeError, EncodeError};

/// `MessageSecurityMode` (Part 4 §7.15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MessageSecurityMode {
    /// The mode is invalid.
    #[default]
    Invalid,
    /// No security applied.
    None,
    /// Messages are signed.
    Sign,
    /// Messages are signed and encrypted.
    SignAndEncrypt,
}

impl MessageSecurityMode {
    /// Wire value (Part 4 §7.15 enum).
    #[must_use]
    pub const fn to_i32(self) -> i32 {
        match self {
            Self::Invalid => 0,
            Self::None => 1,
            Self::Sign => 2,
            Self::SignAndEncrypt => 3,
        }
    }

    /// Builds from a wire value.
    ///
    /// # Errors
    /// [`DecodeError::InvalidDiscriminant`] for an out-of-range value.
    pub fn from_i32(v: i32) -> Result<Self, DecodeError> {
        Ok(match v {
            0 => Self::Invalid,
            1 => Self::None,
            2 => Self::Sign,
            3 => Self::SignAndEncrypt,
            other => {
                return Err(DecodeError::InvalidDiscriminant {
                    field: "MessageSecurityMode",
                    value: other as u32,
                });
            }
        })
    }
}

/// `ApplicationType` (Part 4 §7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ApplicationType {
    /// A server.
    #[default]
    Server,
    /// A client.
    Client,
    /// Both client and server.
    ClientAndServer,
    /// A discovery server.
    DiscoveryServer,
}

impl ApplicationType {
    const fn to_i32(self) -> i32 {
        match self {
            Self::Server => 0,
            Self::Client => 1,
            Self::ClientAndServer => 2,
            Self::DiscoveryServer => 3,
        }
    }

    fn from_i32(v: i32) -> Result<Self, DecodeError> {
        Ok(match v {
            0 => Self::Server,
            1 => Self::Client,
            2 => Self::ClientAndServer,
            3 => Self::DiscoveryServer,
            other => {
                return Err(DecodeError::InvalidDiscriminant {
                    field: "ApplicationType",
                    value: other as u32,
                });
            }
        })
    }
}

/// `UserTokenType` (Part 4 §7.42).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UserTokenType {
    /// Anonymous access.
    #[default]
    Anonymous,
    /// Username/password.
    UserName,
    /// X.509 certificate.
    Certificate,
    /// An issued (e.g. JWT) token.
    IssuedToken,
}

impl UserTokenType {
    const fn to_i32(self) -> i32 {
        match self {
            Self::Anonymous => 0,
            Self::UserName => 1,
            Self::Certificate => 2,
            Self::IssuedToken => 3,
        }
    }

    fn from_i32(v: i32) -> Result<Self, DecodeError> {
        Ok(match v {
            0 => Self::Anonymous,
            1 => Self::UserName,
            2 => Self::Certificate,
            3 => Self::IssuedToken,
            other => {
                return Err(DecodeError::InvalidDiscriminant {
                    field: "UserTokenType",
                    value: other as u32,
                });
            }
        })
    }
}

/// `ApplicationDescription` (Part 4 §7.2).
#[derive(Debug, Clone, PartialEq)]
pub struct ApplicationDescription {
    /// Globally unique application identifier.
    pub application_uri: String,
    /// Product identifier.
    pub product_uri: String,
    /// Human-readable application name.
    pub application_name: LocalizedText,
    /// Application type.
    pub application_type: ApplicationType,
    /// Gateway server URI (empty if not a gateway).
    pub gateway_server_uri: String,
    /// Discovery profile URI.
    pub discovery_profile_uri: String,
    /// Discovery endpoint URLs.
    pub discovery_urls: Vec<String>,
}

impl UaEncode for ApplicationDescription {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        write_string(w, &self.application_uri)?;
        write_string(w, &self.product_uri)?;
        self.application_name.encode(w)?;
        w.write_i32(self.application_type.to_i32());
        write_string(w, &self.gateway_server_uri)?;
        write_string(w, &self.discovery_profile_uri)?;
        write_string_array(w, &self.discovery_urls)?;
        Ok(())
    }
}

impl UaDecode for ApplicationDescription {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            application_uri: read_string(r)?,
            product_uri: read_string(r)?,
            application_name: LocalizedText::decode(r)?,
            application_type: ApplicationType::from_i32(r.read_i32()?)?,
            gateway_server_uri: read_string(r)?,
            discovery_profile_uri: read_string(r)?,
            discovery_urls: read_string_array(r)?,
        })
    }
}

/// `UserTokenPolicy` (Part 4 §7.42).
#[derive(Debug, Clone, PartialEq)]
pub struct UserTokenPolicy {
    /// Identifier used by the client to select the policy.
    pub policy_id: String,
    /// Kind of user token.
    pub token_type: UserTokenType,
    /// Issued-token type URI.
    pub issued_token_type: String,
    /// Issuer endpoint URL.
    pub issuer_endpoint_url: String,
    /// SecurityPolicy URI applied to the token.
    pub security_policy_uri: String,
}

impl UaEncode for UserTokenPolicy {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        write_string(w, &self.policy_id)?;
        w.write_i32(self.token_type.to_i32());
        write_string(w, &self.issued_token_type)?;
        write_string(w, &self.issuer_endpoint_url)?;
        write_string(w, &self.security_policy_uri)?;
        Ok(())
    }
}

impl UaDecode for UserTokenPolicy {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            policy_id: read_string(r)?,
            token_type: UserTokenType::from_i32(r.read_i32()?)?,
            issued_token_type: read_string(r)?,
            issuer_endpoint_url: read_string(r)?,
            security_policy_uri: read_string(r)?,
        })
    }
}

/// `EndpointDescription` (Part 4 §7.10) — carried by the PublisherEndpoints
/// discovery response.
#[derive(Debug, Clone, PartialEq)]
pub struct EndpointDescription {
    /// Endpoint URL.
    pub endpoint_url: String,
    /// Describing the server.
    pub server: ApplicationDescription,
    /// Server's application instance certificate (DER), empty if none.
    pub server_certificate: ByteString,
    /// Message security mode.
    pub security_mode: MessageSecurityMode,
    /// SecurityPolicy URI.
    pub security_policy_uri: String,
    /// Supported user identity token policies.
    pub user_identity_tokens: Vec<UserTokenPolicy>,
    /// Transport profile URI.
    pub transport_profile_uri: String,
    /// Relative security level.
    pub security_level: u8,
}

impl UaEncode for EndpointDescription {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        write_string(w, &self.endpoint_url)?;
        self.server.encode(w)?;
        write_byte_string(w, &self.server_certificate)?;
        w.write_i32(self.security_mode.to_i32());
        write_string(w, &self.security_policy_uri)?;
        write_array(w, &self.user_identity_tokens, "UserTokenPolicies")?;
        write_string(w, &self.transport_profile_uri)?;
        w.write_u8(self.security_level);
        Ok(())
    }
}

impl UaDecode for EndpointDescription {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            endpoint_url: read_string(r)?,
            server: ApplicationDescription::decode(r)?,
            server_certificate: read_byte_string(r)?,
            security_mode: MessageSecurityMode::from_i32(r.read_i32()?)?,
            security_policy_uri: read_string(r)?,
            user_identity_tokens: read_array::<UserTokenPolicy>(r)?,
            transport_profile_uri: read_string(r)?,
            security_level: r.read_u8()?,
        })
    }
}

/// `DataSetWriterDataType` (Part 14 §6.2.4.5.1) — a DataSetWriter's full
/// configuration as carried by the DataSetWriterConfiguration response.
#[derive(Debug, Clone, PartialEq)]
pub struct DataSetWriterDataType {
    /// Writer name.
    pub name: String,
    /// Whether the writer is enabled.
    pub enabled: bool,
    /// Numeric DataSetWriter id.
    pub data_set_writer_id: u16,
    /// `DataSetFieldContentMask` bits.
    pub data_set_field_content_mask: u32,
    /// KeyFrame period.
    pub key_frame_count: u32,
    /// Referenced PublishedDataSet name.
    pub data_set_name: String,
    /// Additional writer properties.
    pub data_set_writer_properties: Vec<KeyValuePair>,
    /// Transport-specific settings (e.g. broker queue name).
    pub transport_settings: ExtensionObject,
    /// Message-specific settings (e.g. UADP DataSetMessage content mask).
    pub message_settings: ExtensionObject,
}

impl UaEncode for DataSetWriterDataType {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        write_string(w, &self.name)?;
        w.write_u8(u8::from(self.enabled));
        w.write_u16(self.data_set_writer_id);
        w.write_u32(self.data_set_field_content_mask);
        w.write_u32(self.key_frame_count);
        write_string(w, &self.data_set_name)?;
        write_array(
            w,
            &self.data_set_writer_properties,
            "DataSetWriterProperties",
        )?;
        self.transport_settings.encode(w)?;
        self.message_settings.encode(w)?;
        Ok(())
    }
}

impl UaDecode for DataSetWriterDataType {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            name: read_string(r)?,
            enabled: r.read_u8()? != 0,
            data_set_writer_id: r.read_u16()?,
            data_set_field_content_mask: r.read_u32()?,
            key_frame_count: r.read_u32()?,
            data_set_name: read_string(r)?,
            data_set_writer_properties: read_array::<KeyValuePair>(r)?,
            transport_settings: ExtensionObject::decode(r)?,
            message_settings: ExtensionObject::decode(r)?,
        })
    }
}

/// `WriterGroupDataType` (Part 14 §6.2.5.6.1) — a WriterGroup's full
/// configuration as carried by the DataSetWriterConfiguration response.
#[derive(Debug, Clone, PartialEq)]
pub struct WriterGroupDataType {
    /// Group name.
    pub name: String,
    /// Whether the group is enabled.
    pub enabled: bool,
    /// Message security mode.
    pub security_mode: MessageSecurityMode,
    /// SecurityGroup id.
    pub security_group_id: String,
    /// Security Key Service endpoints.
    pub security_key_services: Vec<EndpointDescription>,
    /// Maximum NetworkMessage size in bytes.
    pub max_network_message_size: u32,
    /// Additional group properties.
    pub group_properties: Vec<KeyValuePair>,
    /// Numeric WriterGroup id.
    pub writer_group_id: u16,
    /// Publishing interval in milliseconds.
    pub publishing_interval: f64,
    /// Keep-alive time in milliseconds.
    pub keep_alive_time: f64,
    /// Network priority.
    pub priority: u8,
    /// Preferred locales.
    pub locale_ids: Vec<String>,
    /// Header layout URI.
    pub header_layout_uri: String,
    /// Transport-specific settings.
    pub transport_settings: ExtensionObject,
    /// Message-specific settings.
    pub message_settings: ExtensionObject,
    /// The group's DataSetWriters.
    pub data_set_writers: Vec<DataSetWriterDataType>,
}

impl UaEncode for WriterGroupDataType {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        write_string(w, &self.name)?;
        w.write_u8(u8::from(self.enabled));
        w.write_i32(self.security_mode.to_i32());
        write_string(w, &self.security_group_id)?;
        write_array(w, &self.security_key_services, "SecurityKeyServices")?;
        w.write_u32(self.max_network_message_size);
        write_array(w, &self.group_properties, "GroupProperties")?;
        w.write_u16(self.writer_group_id);
        w.write_f64(self.publishing_interval);
        w.write_f64(self.keep_alive_time);
        w.write_u8(self.priority);
        write_string_array(w, &self.locale_ids)?;
        write_string(w, &self.header_layout_uri)?;
        self.transport_settings.encode(w)?;
        self.message_settings.encode(w)?;
        write_array(w, &self.data_set_writers, "DataSetWriters")?;
        Ok(())
    }
}

impl UaDecode for WriterGroupDataType {
    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            name: read_string(r)?,
            enabled: r.read_u8()? != 0,
            security_mode: MessageSecurityMode::from_i32(r.read_i32()?)?,
            security_group_id: read_string(r)?,
            security_key_services: read_array::<EndpointDescription>(r)?,
            max_network_message_size: r.read_u32()?,
            group_properties: read_array::<KeyValuePair>(r)?,
            writer_group_id: r.read_u16()?,
            publishing_interval: r.read_f64()?,
            keep_alive_time: r.read_f64()?,
            priority: r.read_u8()?,
            locale_ids: read_string_array(r)?,
            header_layout_uri: read_string(r)?,
            transport_settings: ExtensionObject::decode(r)?,
            message_settings: ExtensionObject::decode(r)?,
            data_set_writers: read_array::<DataSetWriterDataType>(r)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binary::{from_binary, to_binary};
    use zerodds_opcua_gateway::data_value::ExtensionObjectBody;
    use zerodds_opcua_gateway::node_id::NodeId;

    fn ext_none() -> ExtensionObject {
        ExtensionObject {
            type_id: NodeId::numeric(0, 0),
            body: ExtensionObjectBody::None,
        }
    }

    fn endpoint() -> EndpointDescription {
        EndpointDescription {
            endpoint_url: String::from("opc.tcp://host:4840"),
            server: ApplicationDescription {
                application_uri: String::from("urn:host:app"),
                product_uri: String::from("urn:vendor:product"),
                application_name: LocalizedText {
                    locale: Some(String::from("en")),
                    text: Some(String::from("App")),
                },
                application_type: ApplicationType::Server,
                gateway_server_uri: String::new(),
                discovery_profile_uri: String::new(),
                discovery_urls: alloc::vec![String::from("opc.tcp://host:4840")],
            },
            server_certificate: alloc::vec![0xDE, 0xAD],
            security_mode: MessageSecurityMode::SignAndEncrypt,
            security_policy_uri: String::from(
                "http://opcfoundation.org/UA/SecurityPolicy#Basic256Sha256",
            ),
            user_identity_tokens: alloc::vec![UserTokenPolicy {
                policy_id: String::from("anon"),
                token_type: UserTokenType::Anonymous,
                issued_token_type: String::new(),
                issuer_endpoint_url: String::new(),
                security_policy_uri: String::new(),
            }],
            transport_profile_uri: String::from(
                "http://opcfoundation.org/UA-Profile/Transport/uatcp-uasc-uabinary",
            ),
            security_level: 3,
        }
    }

    #[test]
    fn endpoint_description_round_trip() {
        let e = endpoint();
        let back: EndpointDescription = from_binary(&to_binary(&e).expect("enc")).expect("dec");
        assert_eq!(back, e);
    }

    #[test]
    fn writer_group_data_type_round_trip() {
        let g = WriterGroupDataType {
            name: String::from("wg1"),
            enabled: true,
            security_mode: MessageSecurityMode::Sign,
            security_group_id: String::from("sg1"),
            security_key_services: alloc::vec![endpoint()],
            max_network_message_size: 1500,
            group_properties: Vec::new(),
            writer_group_id: 42,
            publishing_interval: 100.0,
            keep_alive_time: 5000.0,
            priority: 7,
            locale_ids: alloc::vec![String::from("en"), String::from("de")],
            header_layout_uri: String::from(
                "http://opcfoundation.org/UA/PubSub/HeaderLayout/UADP-Dynamic",
            ),
            transport_settings: ext_none(),
            message_settings: ext_none(),
            data_set_writers: alloc::vec![DataSetWriterDataType {
                name: String::from("w1"),
                enabled: true,
                data_set_writer_id: 5,
                data_set_field_content_mask: 0x20,
                key_frame_count: 10,
                data_set_name: String::from("ds1"),
                data_set_writer_properties: Vec::new(),
                transport_settings: ext_none(),
                message_settings: ext_none(),
            }],
        };
        let back: WriterGroupDataType = from_binary(&to_binary(&g).expect("enc")).expect("dec");
        assert_eq!(back, g);
    }

    #[test]
    fn security_mode_discriminant_rejected() {
        // i32 = 9 is not a valid MessageSecurityMode.
        let mut w = UaWriter::new();
        w.write_i32(9);
        assert!(MessageSecurityMode::from_i32(9).is_err());
        let _ = w;
    }
}
