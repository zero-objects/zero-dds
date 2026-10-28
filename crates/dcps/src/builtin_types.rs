// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The built-in types of DDS-XTypes 1.3 §7.6.5 and Annex E, as topic types
//! with their type-specific DataWriters and DataReaders.
//!
//! ```idl
//! module DDS {
//!   @extensibility(APPENDABLE) struct _String { string value; };
//!   @extensibility(APPENDABLE) struct KeyedString { @key string key; string value; };
//!   @extensibility(APPENDABLE) struct Bytes { ByteSeq value; };
//!   @extensibility(APPENDABLE) struct KeyedBytes { @key string key; ByteSeq value; };
//! };
//! ```
//!
//! | Annex E | Rust |
//! |---|---|
//! | `DDS::String` | [`DdsString`] |
//! | `DDS::KeyedString` | [`DdsKeyedString`] |
//! | `DDS::Bytes` | [`DdsBytes`] |
//! | `DDS::KeyedBytes` | [`DdsKeyedBytes`] |
//! | `StringDataWriter`, `StringDataReader`, … | [`StringDataWriter`], [`StringDataReader`], … (`DataWriter<T>` and `DataReader<T>` of the type: "instantiate the type FooDataWriter defined by the DDS specification") |
//! | the operations of `KeyedStringDataWriter`, `BytesDataWriter`, `KeyedBytesDataWriter` | [`KeyedStringDataWriterOperations`], [`BytesDataWriterOperations`], [`KeyedBytesDataWriterOperations`], implemented for those writers |
//! | `StringTypeSupport`, … | [`StringTypeSupport`], [`KeyedStringTypeSupport`], [`BytesTypeSupport`], [`KeyedBytesTypeSupport`], each a [`TypeSupport`] with `get_type_name` and `register_type`; a participant also holds the four declarations under their physical names from its creation (see [`crate::builtin_type_set`]) |
//!
//! The wire form is the appendable one of Annex E: under XCDR2 a DHEADER,
//! then the members; under XCDR1 the members alone (Table 60, `CDR_*`).

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_types::TypeIdentifier;
use zerodds_types::type_object::TypeObject;
use zerodds_wire::{Endianness, Reader, WireError, Writer};

use crate::builtin_type_set::{
    BuiltinTypeSet, NAME_DDS_BYTES, NAME_DDS_KEYED_BYTES, NAME_DDS_KEYED_STRING, NAME_DDS_STRING,
};
use crate::dds_type::{DdsType, DecodeError, EncodeError, Extensibility, PlainCdr2BeKeyHolder};
use crate::error::{DdsError, Result};
use crate::instance_handle::{HANDLE_NIL, InstanceHandle};
use crate::subscriber::DataReader;
use crate::time::Time;

use crate::publisher::DataWriter;

/// The members of one built-in type and how they stand on the wire.
trait Members: Sized {
    fn write(&self, writer: &mut Writer);
    fn read(reader: &mut Reader<'_>) -> core::result::Result<Self, WireError>;
}

fn endianness(big: bool) -> Endianness {
    if big {
        Endianness::Big
    } else {
        Endianness::Little
    }
}

/// XCDR2 (`appendable`: a DHEADER in front of the members, alignment
/// capped at four) or XCDR1 (the members alone, alignment capped at eight).
fn encode_members<T: Members>(
    value: &T,
    big: bool,
    xcdr1: bool,
    out: &mut Vec<u8>,
) -> core::result::Result<(), EncodeError> {
    let mut writer = Writer::with_max_align(endianness(big), if xcdr1 { 8 } else { 4 });
    if xcdr1 {
        value.write(&mut writer);
    } else {
        let header = writer.write_dheader();
        value.write(&mut writer);
        writer.finish_dheader(header);
    }
    out.extend_from_slice(&writer.finish());
    Ok(())
}

fn decode_members<T: Members>(
    bytes: &[u8],
    big: bool,
    xcdr1: bool,
) -> core::result::Result<T, DecodeError> {
    let mut reader = Reader::with_max_align(bytes, endianness(big), if xcdr1 { 8 } else { 4 });
    if xcdr1 {
        return Ok(T::read(&mut reader)?);
    }
    // An appendable type's DHEADER says where the members end; a writer
    // that appended members this reader does not know is read up to its
    // own, and the rest is skipped (§7.4.3.5.3).
    let length = reader.read_dheader()?;
    let end = reader.position() + length;
    let value = T::read(&mut reader)?;
    reader.skip_to(end)?;
    Ok(value)
}

/// The identifier and objects of a built-in type, from the Annex E
/// declaration in [`BuiltinTypeSet`].
fn identity(name: &str) -> Option<(TypeIdentifier, TypeObject, Vec<TypeObject>)> {
    let set = BuiltinTypeSet::build().ok()?;
    let built = set.get(name)?;
    let complete = zerodds_types::from_ast::complete_type_object(built.ast(), built.id()).ok()?;
    let identifier = zerodds_types::from_ast::type_identifier(
        built.ast(),
        built.id(),
        zerodds_types::from_ast::Form::Complete,
    )
    .ok()?;
    let dependencies = zerodds_types::from_ast::dependency_type_objects(built.ast(), built.id())
        .ok()?
        .into_iter()
        .map(TypeObject::Complete)
        .collect();
    Some((identifier, TypeObject::Complete(complete), dependencies))
}

macro_rules! built_in_codec {
    ($name:literal) => {
        const EXTENSIBILITY: Extensibility = Extensibility::Appendable;
        const XCDR1_MATCHES_XCDR2: bool = true;

        fn type_identifier() -> TypeIdentifier {
            identity($name).map_or(TypeIdentifier::None, |(identifier, _, _)| identifier)
        }

        fn type_object() -> Option<TypeObject> {
            identity($name).map(|(_, object, _)| object)
        }

        fn type_object_dependencies() -> Vec<TypeObject> {
            identity($name).map_or_else(Vec::new, |(_, _, dependencies)| dependencies)
        }

        fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
            encode_members(self, false, false, out)
        }

        fn encode_be(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
            encode_members(self, true, false, out)
        }

        fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
            decode_members(bytes, false, false)
        }

        fn decode_be(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
            decode_members(bytes, true, false)
        }

        fn encode_xcdr1(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
            encode_members(self, false, true, out)
        }

        fn encode_xcdr1_be(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
            encode_members(self, true, true, out)
        }

        fn decode_xcdr1(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
            decode_members(bytes, false, true)
        }

        fn decode_xcdr1_be(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
            decode_members(bytes, true, true)
        }
    };
}

/// `DDS::String`: a single unbounded string (§7.6.5.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DdsString {
    /// The string.
    pub value: String,
}

impl DdsString {
    /// A `DDS::String` holding `value`.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }
}

impl Members for DdsString {
    fn write(&self, writer: &mut Writer) {
        writer.write_string(&self.value);
    }

    fn read(reader: &mut Reader<'_>) -> core::result::Result<Self, WireError> {
        Ok(Self {
            value: reader.read_string()?,
        })
    }
}

impl DdsType for DdsString {
    const TYPE_NAME: &'static str = NAME_DDS_STRING;
    built_in_codec!("DDS::String");
}

/// `DDS::KeyedString`: a payload string and a key string (§7.6.5.2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DdsKeyedString {
    /// The key (`@key`).
    pub key: String,
    /// The payload.
    pub value: String,
}

impl DdsKeyedString {
    /// A `DDS::KeyedString` with that key and payload.
    #[must_use]
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

impl Members for DdsKeyedString {
    fn write(&self, writer: &mut Writer) {
        writer.write_string(&self.key);
        writer.write_string(&self.value);
    }

    fn read(reader: &mut Reader<'_>) -> core::result::Result<Self, WireError> {
        Ok(Self {
            key: reader.read_string()?,
            value: reader.read_string()?,
        })
    }
}

/// The key holder of a type whose only key is a string (§7.6.8): the
/// string in PLAIN_CDR2 big-endian.
fn key_holder_string(key: &str, holder: &mut PlainCdr2BeKeyHolder) {
    holder.write_string(key);
}

fn read_key_holder_string(bytes: &[u8]) -> core::result::Result<String, DecodeError> {
    let mut reader = Reader::with_max_align(bytes, Endianness::Big, 4);
    Ok(reader.read_string()?)
}

impl DdsType for DdsKeyedString {
    const TYPE_NAME: &'static str = NAME_DDS_KEYED_STRING;
    const HAS_KEY: bool = true;
    built_in_codec!("DDS::KeyedString");

    fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
        key_holder_string(&self.key, holder);
    }

    fn decode_key_holder_be(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        Ok(Self {
            key: read_key_holder_string(bytes)?,
            value: String::new(),
        })
    }
}

/// `DDS::Bytes`: an unbounded sequence of bytes (§7.6.5.3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DdsBytes {
    /// The bytes.
    pub value: Vec<u8>,
}

impl DdsBytes {
    /// A `DDS::Bytes` holding `value`.
    #[must_use]
    pub fn new(value: impl Into<Vec<u8>>) -> Self {
        Self {
            value: value.into(),
        }
    }
}

fn write_octets(writer: &mut Writer, octets: &[u8]) {
    // A `sequence<octet>`: the count, then the octets. The element is a
    // primitive, so there is no DHEADER of its own (§7.4.3.5.3).
    #[allow(clippy::cast_possible_truncation)]
    writer.write_u32(octets.len() as u32);
    writer.write_bytes(octets);
}

fn read_octets(reader: &mut Reader<'_>) -> core::result::Result<Vec<u8>, WireError> {
    let count = reader.read_u32()? as usize;
    if count > reader.remaining() {
        return Err(WireError::ImpossibleLength {
            at: reader.position(),
            claimed: count,
            available: reader.remaining(),
        });
    }
    Ok(reader.read_bytes(count)?.to_vec())
}

impl Members for DdsBytes {
    fn write(&self, writer: &mut Writer) {
        write_octets(writer, &self.value);
    }

    fn read(reader: &mut Reader<'_>) -> core::result::Result<Self, WireError> {
        Ok(Self {
            value: read_octets(reader)?,
        })
    }
}

impl DdsType for DdsBytes {
    const TYPE_NAME: &'static str = NAME_DDS_BYTES;
    built_in_codec!("DDS::Bytes");
}

/// `DDS::KeyedBytes`: a byte sequence and a key string (§7.6.5.4).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DdsKeyedBytes {
    /// The key (`@key`).
    pub key: String,
    /// The bytes.
    pub value: Vec<u8>,
}

impl DdsKeyedBytes {
    /// A `DDS::KeyedBytes` with that key and payload.
    #[must_use]
    pub fn new(key: impl Into<String>, value: impl Into<Vec<u8>>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

impl Members for DdsKeyedBytes {
    fn write(&self, writer: &mut Writer) {
        writer.write_string(&self.key);
        write_octets(writer, &self.value);
    }

    fn read(reader: &mut Reader<'_>) -> core::result::Result<Self, WireError> {
        Ok(Self {
            key: reader.read_string()?,
            value: read_octets(reader)?,
        })
    }
}

impl DdsType for DdsKeyedBytes {
    const TYPE_NAME: &'static str = NAME_DDS_KEYED_BYTES;
    const HAS_KEY: bool = true;
    built_in_codec!("DDS::KeyedBytes");

    fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
        key_holder_string(&self.key, holder);
    }

    fn decode_key_holder_be(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        Ok(Self {
            key: read_key_holder_string(bytes)?,
            value: Vec::new(),
        })
    }
}

/// `StringDataWriter`.
pub type StringDataWriter = DataWriter<DdsString>;
/// `StringDataReader`.
pub type StringDataReader = DataReader<DdsString>;
/// `KeyedStringDataWriter`; its extra operations are
/// [`KeyedStringDataWriterOperations`].
pub type KeyedStringDataWriter = DataWriter<DdsKeyedString>;
/// `KeyedStringDataReader`.
pub type KeyedStringDataReader = DataReader<DdsKeyedString>;
/// `BytesDataWriter`; its extra operations are
/// [`BytesDataWriterOperations`].
pub type BytesDataWriter = DataWriter<DdsBytes>;
/// `BytesDataReader`.
pub type BytesDataReader = DataReader<DdsBytes>;
/// `KeyedBytesDataWriter`; its extra operations are
/// [`KeyedBytesDataWriterOperations`].
pub type KeyedBytesDataWriter = DataWriter<DdsKeyedBytes>;
/// `KeyedBytesDataReader`.
pub type KeyedBytesDataReader = DataReader<DdsKeyedBytes>;

/// The base `TypeSupport` of DDS 1.4 §2.2.2.3.6, as the Annex E interfaces
/// `StringTypeSupport`, `KeyedStringTypeSupport`, `BytesTypeSupport` and
/// `KeyedBytesTypeSupport` have it: "This interface shall instantiate the
/// type FooTypeSupport defined by the DDS specification".
pub trait TypeSupport {
    /// `get_type_name`: the name the type is registered under unless
    /// `register_type` is given another one.
    fn get_type_name(&self) -> &str;

    /// `register_type(participant, type_name)`: registers the type with
    /// `participant` under `type_name`, or under
    /// [`Self::get_type_name`] where `None` is given. The `TypeObject`s of
    /// the type go to the participant's TypeLookup registry and
    /// [`crate::participant::DomainParticipant::create_registered_topic`]
    /// creates topics of the type by that name. A participant holds the four built-in types under
    /// their own names from its creation (§7.6.5), so registering one
    /// under its own name again is not an error.
    ///
    /// # Errors
    /// `PreconditionNotMet` when another type is registered under the
    /// name; `BadParameter` for a name that is no name; `AlreadyDeleted`
    /// for a deleted participant.
    #[cfg(feature = "std")]
    fn register_type(
        &self,
        participant: &crate::participant::DomainParticipant,
        type_name: Option<&str>,
    ) -> Result<()>;
}

#[cfg(feature = "std")]
fn register_builtin(
    physical: &'static str,
    participant: &crate::participant::DomainParticipant,
    type_name: Option<&str>,
) -> Result<()> {
    fn wire<E>(_: E) -> DdsError {
        DdsError::PreconditionNotMet {
            reason: "the built-in type has no TypeObject",
        }
    }
    let set = BuiltinTypeSet::build().map_err(wire)?;
    let built = set.get(physical).ok_or(DdsError::PreconditionNotMet {
        reason: "the built-in type is not declared",
    })?;
    let root =
        zerodds_types::from_ast::complete_type_object(built.ast(), built.id()).map_err(wire)?;
    let dependencies =
        zerodds_types::from_ast::dependency_type_objects(built.ast(), built.id()).map_err(wire)?;
    let support = crate::serialized_type::SerializedTypeSupport::from_complete_type_objects(
        physical,
        root,
        dependencies,
    )
    .map_err(|message| DdsError::WireError { message })?;
    participant.register_serialized_type(&support, type_name)
}

macro_rules! built_in_type_support {
    ($(#[$doc:meta])* $name:ident, $physical:expr) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub struct $name;

        impl $name {
            /// The topic type this support is for.
            #[must_use]
            pub const fn type_name() -> &'static str {
                $physical
            }
        }

        impl TypeSupport for $name {
            fn get_type_name(&self) -> &str {
                $physical
            }

            #[cfg(feature = "std")]
            fn register_type(
                &self,
                participant: &crate::participant::DomainParticipant,
                type_name: Option<&str>,
            ) -> Result<()> {
                register_builtin($physical, participant, type_name)
            }
        }
    };
}

built_in_type_support!(
    /// `StringTypeSupport` (Annex E): the support of `DDS::String`.
    StringTypeSupport,
    NAME_DDS_STRING
);
built_in_type_support!(
    /// `KeyedStringTypeSupport` (Annex E): the support of `DDS::KeyedString`.
    KeyedStringTypeSupport,
    NAME_DDS_KEYED_STRING
);
built_in_type_support!(
    /// `BytesTypeSupport` (Annex E): the support of `DDS::Bytes`.
    BytesTypeSupport,
    NAME_DDS_BYTES
);
built_in_type_support!(
    /// `KeyedBytesTypeSupport` (Annex E): the support of `DDS::KeyedBytes`.
    KeyedBytesTypeSupport,
    NAME_DDS_KEYED_BYTES
);

/// The octets `bytes[offset .. offset + length]`, the way the
/// `write_w_bytes` family of Annex E names them.
fn octet_range(bytes: &[u8], offset: i32, length: i32) -> Result<&[u8]> {
    let range = usize::try_from(offset)
        .ok()
        .zip(usize::try_from(length).ok())
        .and_then(|(offset, length)| Some(offset..offset.checked_add(length)?));
    range
        .and_then(|range| bytes.get(range))
        .ok_or(DdsError::BadParameter {
            what: "offset and length name octets outside the array",
        })
}

/// The handle of a write must be nil or the one of the sample's instance
/// (DDS 1.4 §2.2.2.4.2.11).
fn check_write_handle<T: DdsType>(
    writer: &DataWriter<T>,
    sample: &T,
    handle: InstanceHandle,
) -> Result<()> {
    if handle != HANDLE_NIL && writer.lookup_instance(sample) != handle {
        return Err(DdsError::BadParameter {
            what: "the handle is not the instance of the key",
        });
    }
    Ok(())
}

/// The operations Annex E adds to `KeyedStringDataWriter`.
pub trait KeyedStringDataWriterOperations {
    /// `register_instance_w_key`.
    ///
    /// # Errors
    /// As [`DataWriter::register_instance`].
    fn register_instance_w_key(&self, key: &str) -> Result<InstanceHandle>;
    /// `register_instance_w_key_w_timestamp`.
    ///
    /// # Errors
    /// As [`DataWriter::register_instance_w_timestamp`].
    fn register_instance_w_key_w_timestamp(
        &self,
        key: &str,
        source_timestamp: Time,
    ) -> Result<InstanceHandle>;
    /// `unregister_instance_w_key`.
    ///
    /// # Errors
    /// As [`DataWriter::unregister_instance`].
    fn unregister_instance_w_key(&self, key: &str) -> Result<()>;
    /// `unregister_instance_w_key_w_timestamp`.
    ///
    /// # Errors
    /// As [`DataWriter::unregister_instance_w_timestamp`].
    fn unregister_instance_w_key_w_timestamp(
        &self,
        key: &str,
        source_timestamp: Time,
    ) -> Result<()>;
    /// `write_string_w_key`: writes `str` under `key`; `handle` is nil or
    /// the handle of the key's instance.
    ///
    /// # Errors
    /// `BadParameter` for a handle that is not the key's; otherwise as
    /// [`DataWriter::write`].
    fn write_string_w_key(&self, key: &str, str: &str, handle: InstanceHandle) -> Result<()>;
    /// `write_string_w_key_w_timestamp`.
    ///
    /// # Errors
    /// As [`Self::write_string_w_key`], with the timestamp of
    /// [`DataWriter::write_w_timestamp`].
    fn write_string_w_key_w_timestamp(
        &self,
        key: &str,
        str: &str,
        handle: InstanceHandle,
        source_timestamp: Time,
    ) -> Result<()>;
    /// `dispose_w_key`.
    ///
    /// # Errors
    /// As [`DataWriter::dispose`].
    fn dispose_w_key(&self, key: &str) -> Result<()>;
    /// `dispose_w_key_w_timestamp`.
    ///
    /// # Errors
    /// As [`DataWriter::dispose_w_timestamp`].
    fn dispose_w_key_w_timestamp(&self, key: &str, source_timestamp: Time) -> Result<()>;
    /// `get_key_value_w_key`: sets `key` to the key of the instance.
    ///
    /// # Errors
    /// As [`DataWriter::get_key_value`].
    fn get_key_value_w_key(&self, key: &mut String, handle: InstanceHandle) -> Result<()>;
    /// `lookup_instance_w_key`.
    fn lookup_instance_w_key(&self, key: &str) -> InstanceHandle;
}

impl KeyedStringDataWriterOperations for DataWriter<DdsKeyedString> {
    fn register_instance_w_key(&self, key: &str) -> Result<InstanceHandle> {
        self.register_instance(&DdsKeyedString::new(key, ""))
    }

    fn register_instance_w_key_w_timestamp(
        &self,
        key: &str,
        source_timestamp: Time,
    ) -> Result<InstanceHandle> {
        self.register_instance_w_timestamp(&DdsKeyedString::new(key, ""), source_timestamp)
    }

    fn unregister_instance_w_key(&self, key: &str) -> Result<()> {
        self.unregister_instance(&DdsKeyedString::new(key, ""), HANDLE_NIL)
    }

    fn unregister_instance_w_key_w_timestamp(
        &self,
        key: &str,
        source_timestamp: Time,
    ) -> Result<()> {
        self.unregister_instance_w_timestamp(
            &DdsKeyedString::new(key, ""),
            HANDLE_NIL,
            source_timestamp,
        )
    }

    fn write_string_w_key(&self, key: &str, str: &str, handle: InstanceHandle) -> Result<()> {
        let sample = DdsKeyedString::new(key, str);
        check_write_handle(self, &sample, handle)?;
        self.write(&sample)
    }

    fn write_string_w_key_w_timestamp(
        &self,
        key: &str,
        str: &str,
        handle: InstanceHandle,
        source_timestamp: Time,
    ) -> Result<()> {
        let sample = DdsKeyedString::new(key, str);
        check_write_handle(self, &sample, handle)?;
        self.write_w_timestamp(&sample, source_timestamp)
    }

    fn dispose_w_key(&self, key: &str) -> Result<()> {
        self.dispose(&DdsKeyedString::new(key, ""), HANDLE_NIL)
    }

    fn dispose_w_key_w_timestamp(&self, key: &str, source_timestamp: Time) -> Result<()> {
        self.dispose_w_timestamp(&DdsKeyedString::new(key, ""), HANDLE_NIL, source_timestamp)
    }

    fn get_key_value_w_key(&self, key: &mut String, handle: InstanceHandle) -> Result<()> {
        *key = self.get_key_value(handle)?.key;
        Ok(())
    }

    fn lookup_instance_w_key(&self, key: &str) -> InstanceHandle {
        self.lookup_instance(&DdsKeyedString::new(key, ""))
    }
}

/// The operations Annex E adds to `BytesDataWriter`.
pub trait BytesDataWriterOperations {
    /// `write_w_bytes`: writes `bytes[offset .. offset + length]`.
    ///
    /// # Errors
    /// `BadParameter` for a range outside `bytes`; otherwise as
    /// [`DataWriter::write`].
    fn write_w_bytes(
        &self,
        bytes: &[u8],
        offset: i32,
        length: i32,
        handle: InstanceHandle,
    ) -> Result<()>;
    /// `write_w_bytes_w_timestamp`.
    ///
    /// # Errors
    /// As [`Self::write_w_bytes`], with the timestamp of
    /// [`DataWriter::write_w_timestamp`].
    fn write_w_bytes_w_timestamp(
        &self,
        bytes: &[u8],
        offset: i32,
        length: i32,
        handle: InstanceHandle,
        source_timestamp: Time,
    ) -> Result<()>;
}

impl BytesDataWriterOperations for DataWriter<DdsBytes> {
    fn write_w_bytes(
        &self,
        bytes: &[u8],
        offset: i32,
        length: i32,
        handle: InstanceHandle,
    ) -> Result<()> {
        let sample = DdsBytes::new(octet_range(bytes, offset, length)?);
        check_write_handle(self, &sample, handle)?;
        self.write(&sample)
    }

    fn write_w_bytes_w_timestamp(
        &self,
        bytes: &[u8],
        offset: i32,
        length: i32,
        handle: InstanceHandle,
        source_timestamp: Time,
    ) -> Result<()> {
        let sample = DdsBytes::new(octet_range(bytes, offset, length)?);
        check_write_handle(self, &sample, handle)?;
        self.write_w_timestamp(&sample, source_timestamp)
    }
}

/// The operations Annex E adds to `KeyedBytesDataWriter`.
pub trait KeyedBytesDataWriterOperations {
    /// `register_instance_w_key`.
    ///
    /// # Errors
    /// As [`DataWriter::register_instance`].
    fn register_instance_w_key(&self, key: &str) -> Result<InstanceHandle>;
    /// `register_instance_w_key_w_timestamp`.
    ///
    /// # Errors
    /// As [`DataWriter::register_instance_w_timestamp`].
    fn register_instance_w_key_w_timestamp(
        &self,
        key: &str,
        source_timestamp: Time,
    ) -> Result<InstanceHandle>;
    /// `unregister_instance_w_key`.
    ///
    /// # Errors
    /// As [`DataWriter::unregister_instance`].
    fn unregister_instance_w_key(&self, key: &str) -> Result<()>;
    /// `unregister_instance_w_key_w_timestamp`.
    ///
    /// # Errors
    /// As [`DataWriter::unregister_instance_w_timestamp`].
    fn unregister_instance_w_key_w_timestamp(
        &self,
        key: &str,
        source_timestamp: Time,
    ) -> Result<()>;
    /// `write_bytes_w_key`: writes `bytes[offset .. offset + length]`
    /// under `key`.
    ///
    /// # Errors
    /// `BadParameter` for a range outside `bytes` or a handle that is not
    /// the key's; otherwise as [`DataWriter::write`].
    fn write_bytes_w_key(
        &self,
        key: &str,
        bytes: &[u8],
        offset: i32,
        length: i32,
        handle: InstanceHandle,
    ) -> Result<()>;
    /// `write_bytes_w_key_w_timestamp`.
    ///
    /// # Errors
    /// As [`Self::write_bytes_w_key`], with the timestamp of
    /// [`DataWriter::write_w_timestamp`].
    fn write_bytes_w_key_w_timestamp(
        &self,
        key: &str,
        bytes: &[u8],
        offset: i32,
        length: i32,
        handle: InstanceHandle,
        source_timestamp: Time,
    ) -> Result<()>;
    /// `dispose_w_key`.
    ///
    /// # Errors
    /// As [`DataWriter::dispose`].
    fn dispose_w_key(&self, key: &str) -> Result<()>;
    /// `dispose_w_key_w_timestamp`.
    ///
    /// # Errors
    /// As [`DataWriter::dispose_w_timestamp`].
    fn dispose_w_key_w_timestamp(&self, key: &str, source_timestamp: Time) -> Result<()>;
    /// `get_key_value_w_key`: sets `key` to the key of the instance.
    ///
    /// # Errors
    /// As [`DataWriter::get_key_value`].
    fn get_key_value_w_key(&self, key: &mut String, handle: InstanceHandle) -> Result<()>;
    /// `lookup_instance_w_key`.
    fn lookup_instance_w_key(&self, key: &str) -> InstanceHandle;
}

impl KeyedBytesDataWriterOperations for DataWriter<DdsKeyedBytes> {
    fn register_instance_w_key(&self, key: &str) -> Result<InstanceHandle> {
        self.register_instance(&DdsKeyedBytes::new(key, Vec::new()))
    }

    fn register_instance_w_key_w_timestamp(
        &self,
        key: &str,
        source_timestamp: Time,
    ) -> Result<InstanceHandle> {
        self.register_instance_w_timestamp(&DdsKeyedBytes::new(key, Vec::new()), source_timestamp)
    }

    fn unregister_instance_w_key(&self, key: &str) -> Result<()> {
        self.unregister_instance(&DdsKeyedBytes::new(key, Vec::new()), HANDLE_NIL)
    }

    fn unregister_instance_w_key_w_timestamp(
        &self,
        key: &str,
        source_timestamp: Time,
    ) -> Result<()> {
        self.unregister_instance_w_timestamp(
            &DdsKeyedBytes::new(key, Vec::new()),
            HANDLE_NIL,
            source_timestamp,
        )
    }

    fn write_bytes_w_key(
        &self,
        key: &str,
        bytes: &[u8],
        offset: i32,
        length: i32,
        handle: InstanceHandle,
    ) -> Result<()> {
        let sample = DdsKeyedBytes::new(key, octet_range(bytes, offset, length)?);
        check_write_handle(self, &sample, handle)?;
        self.write(&sample)
    }

    fn write_bytes_w_key_w_timestamp(
        &self,
        key: &str,
        bytes: &[u8],
        offset: i32,
        length: i32,
        handle: InstanceHandle,
        source_timestamp: Time,
    ) -> Result<()> {
        let sample = DdsKeyedBytes::new(key, octet_range(bytes, offset, length)?);
        check_write_handle(self, &sample, handle)?;
        self.write_w_timestamp(&sample, source_timestamp)
    }

    fn dispose_w_key(&self, key: &str) -> Result<()> {
        self.dispose(&DdsKeyedBytes::new(key, Vec::new()), HANDLE_NIL)
    }

    fn dispose_w_key_w_timestamp(&self, key: &str, source_timestamp: Time) -> Result<()> {
        self.dispose_w_timestamp(
            &DdsKeyedBytes::new(key, Vec::new()),
            HANDLE_NIL,
            source_timestamp,
        )
    }

    fn get_key_value_w_key(&self, key: &mut String, handle: InstanceHandle) -> Result<()> {
        *key = self.get_key_value(handle)?.key;
        Ok(())
    }

    fn lookup_instance_w_key(&self, key: &str) -> InstanceHandle {
        self.lookup_instance(&DdsKeyedBytes::new(key, Vec::new()))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// Annex E gives the structures `@extensibility(APPENDABLE)`; under
    /// XCDR2 a DHEADER (the length of the members) precedes them.
    #[test]
    fn a_string_is_a_dheader_and_a_cdr_string() {
        let mut out = Vec::new();
        DdsString::new("hi").encode(&mut out).unwrap();
        assert_eq!(
            out,
            [
                7, 0, 0, 0, // DHEADER: 4 (length) + 3 (h, i, NUL)
                3, 0, 0, 0, b'h', b'i', 0
            ]
        );
        assert_eq!(DdsString::decode(&out).unwrap(), DdsString::new("hi"));
    }

    #[test]
    fn xcdr1_has_the_members_alone() {
        let mut out = Vec::new();
        DdsString::new("hi").encode_xcdr1(&mut out).unwrap();
        assert_eq!(out, [3, 0, 0, 0, b'h', b'i', 0]);
        assert_eq!(DdsString::decode_xcdr1(&out).unwrap().value, "hi");
    }

    /// The key string is followed by the payload on a four-octet boundary.
    #[test]
    fn a_keyed_string_pads_the_second_member_to_four() {
        let mut out = Vec::new();
        DdsKeyedString::new("k", "v").encode(&mut out).unwrap();
        assert_eq!(
            out,
            [
                // DHEADER: key (4 + 2) + 2 padding + value (4 + 2).
                14, 0, 0, 0, //
                2, 0, 0, 0, b'k', 0, 0, 0, //
                2, 0, 0, 0, b'v', 0
            ]
        );
        let back = DdsKeyedString::decode(&out).unwrap();
        assert_eq!((back.key.as_str(), back.value.as_str()), ("k", "v"));
    }

    #[test]
    fn bytes_are_a_count_and_the_octets_without_an_element_dheader() {
        let mut out = Vec::new();
        DdsBytes::new(vec![9, 8, 7]).encode(&mut out).unwrap();
        assert_eq!(out, [7, 0, 0, 0, 3, 0, 0, 0, 9, 8, 7]);
        assert_eq!(DdsBytes::decode(&out).unwrap().value, vec![9, 8, 7]);
        let mut big = Vec::new();
        DdsBytes::new(vec![9]).encode_be(&mut big).unwrap();
        assert_eq!(big, [0, 0, 0, 5, 0, 0, 0, 1, 9]);
        assert_eq!(DdsBytes::decode_be(&big).unwrap().value, vec![9]);
    }

    /// A writer that appended a member the reader does not know is read up
    /// to the DHEADER's end.
    #[test]
    fn a_reader_skips_what_an_appended_member_adds() {
        let mut out = Vec::new();
        DdsString::new("hi").encode(&mut out).unwrap();
        out[0] = 11;
        out.extend_from_slice(&[0xAA; 4]);
        assert_eq!(DdsString::decode(&out).unwrap().value, "hi");
    }

    #[test]
    fn the_key_holder_is_the_key_string_big_endian() {
        let mut holder = PlainCdr2BeKeyHolder::new();
        DdsKeyedBytes::new("ab", vec![1]).encode_key_holder_be(&mut holder);
        let bytes = holder.into_bytes();
        assert_eq!(bytes, [0, 0, 0, 3, b'a', b'b', 0]);
        assert_eq!(
            DdsKeyedBytes::decode_key_holder_be(&bytes).unwrap().key,
            "ab"
        );
    }

    #[test]
    fn the_types_carry_their_annex_e_declaration() {
        assert_eq!(DdsString::TYPE_NAME, "DDS::String");
        assert_eq!(DdsKeyedString::TYPE_NAME, "DDS::KeyedString");
        assert_eq!(DdsBytes::TYPE_NAME, "DDS::Bytes");
        assert_eq!(DdsKeyedBytes::TYPE_NAME, "DDS::KeyedBytes");
        assert!(DdsKeyedString::has_key() && DdsKeyedBytes::has_key());
        assert!(!DdsString::has_key() && !DdsBytes::has_key());
        assert_eq!(DdsString::extensibility(), Extensibility::Appendable);
        assert!(DdsString::type_object().is_some());
        assert_ne!(DdsString::type_identifier(), TypeIdentifier::None);
        assert_ne!(
            DdsString::type_identifier(),
            DdsKeyedString::type_identifier()
        );
    }

    #[test]
    fn a_range_outside_the_array_is_refused() {
        let bytes = [1u8, 2, 3, 4];
        assert_eq!(octet_range(&bytes, 1, 2).unwrap(), &[2, 3]);
        assert!(octet_range(&bytes, -1, 2).is_err());
        assert!(octet_range(&bytes, 3, 2).is_err());
        assert!(octet_range(&bytes, 0, -1).is_err());
    }
}
