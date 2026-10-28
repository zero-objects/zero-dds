// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The `REPRESENTATION_IN_BINARY` structures of DDS-XRCE 1.0 §7.7.3 and
//! Annex A, serialized with XCDR version 2 (§7.7.3.1.3).
//!
//! The specification gives each structure twice and the two disagree:
//!
//! * **Annex A** (the IDL file, S. 109 to 111): the structures are
//!   `@extensibility(APPENDABLE)`, so each starts with a DHEADER;
//!   `OBJK_DomainParticipant_Binary` has no `domain_id`;
//!   `OBJK_DataWriter_Binary` is a `topic_name` and an `@optional`
//!   `OBJK_DataWriter_Binary_Qos`; bit 1 of `EndpointQosFlags` is
//!   `is_history_keep_all`.
//! * **The clauses** §7.7.3.6.3 to §7.7.3.11.2: the structures are
//!   `@extensibility(FINAL)`, `OBJK_DomainParticipant_Binary` starts with a
//!   `long domain_id`, `OBJK_DataWriter_Binary` is a `topic_name`, an
//!   `OBJK_Endpoint_QosBinary` that is not optional, and the role's own
//!   `@optional` members; bit 1 of `EndpointQosFlags` is
//!   `is_history_keep_last`.
//!
//! A blob is read as the Annex A form first and, when that is not a complete
//! reading of it, as the form of the clauses; one that is neither is
//! refused. The caller falls back to the classic-CDR structures that the
//! eProsima Micro-XRCE-DDS Client sends (`crate::daemon::entity_repr`). For
//! `OBJK_DomainParticipant_Binary` that client sends the two references
//! with neither a DHEADER nor a `domain_id` (the captured blob is the two
//! octets `00 00`); that is the third form read here.
//!
//! The Annex B.2 examples put the DHEADER where the `sequence<octet>` of
//! `binary_representation` has its length, with bit 31 set
//! (`0x80000002`, `0x8000000C`); `crate::daemon::entity_repr` reads that form
//! (`zerodds_xrce::object_repr::ObjectVariant::decode_at` takes the DHEADER
//! for the blob) and the structures in it are the ones read here. Which
//! reading each example needed is in the tests of B.2.2 to B.2.7
//! (`object_repr.rs`, `entity_repr.rs`, `session.rs`).

use zerodds_types::TypeIdentifier;
use zerodds_wire::{Endianness, Reader, WireError};

use crate::daemon::entity_repr::ReprError;

/// XCDR2 aligns at most to four octets.
const MAX_ALIGN: usize = 4;

/// `string<128>`.
const REFERENCE_BOUND: usize = 128;
/// `string<256>`.
const NAME_BOUND: usize = 256;

fn wire(e: WireError) -> ReprError {
    ReprError::Malformed(format!("{e}"))
}

fn reader(bytes: &[u8], big_endian: bool) -> Reader<'_> {
    Reader::with_max_align(
        bytes,
        if big_endian {
            Endianness::Big
        } else {
            Endianness::Little
        },
        MAX_ALIGN,
    )
}

/// Which of the two forms a structure is read as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Form {
    /// Annex A: `@extensibility(APPENDABLE)`, a DHEADER in front.
    AnnexA,
    /// The clauses of §7.7.3: `@extensibility(FINAL)`, nothing in front.
    Clauses,
    /// `OBJK_DomainParticipant_Binary` as the eProsima client sends it: the
    /// two references, nothing in front, no `domain_id`.
    References,
}

/// Reads a structure of `form`: inside a DHEADER for [`Form::AnnexA`], to
/// the end of `bytes` for [`Form::Clauses`]; the structure has to take all
/// of `bytes`.
fn structure<T>(
    bytes: &[u8],
    big_endian: bool,
    form: Form,
    what: &str,
    read: impl FnOnce(&mut Reader<'_>) -> Result<T, ReprError>,
) -> Result<T, ReprError> {
    let mut r = reader(bytes, big_endian);
    let value = match form {
        Form::AnnexA => {
            let length = r.read_dheader().map_err(wire)?;
            let end = r.position() + length;
            let value = read(&mut r)?;
            if r.position() > end {
                return Err(ReprError::Malformed(format!(
                    "{what}: the members run past their DHEADER"
                )));
            }
            // An appended member of a later version is skipped.
            r.skip_to(end).map_err(wire)?;
            value
        }
        Form::Clauses | Form::References => read(&mut r)?,
    };
    if r.remaining() != 0 {
        return Err(ReprError::Malformed(format!(
            "{what}: {} bytes follow the structure",
            r.remaining()
        )));
    }
    Ok(value)
}

/// Tries `forms` in order; the error names what each of them found.
fn either<T>(
    what: &str,
    forms: &[Form],
    mut read: impl FnMut(Form) -> Result<T, ReprError>,
) -> Result<(Form, T), ReprError> {
    let mut found = Vec::new();
    for form in forms {
        match read(*form) {
            Ok(v) => return Ok((*form, v)),
            Err(e) => found.push(format!("as {form:?}: {}", e.message())),
        }
    }
    Err(ReprError::Malformed(format!(
        "{what} is none of the XCDR2 forms ({})",
        found.join("; ")
    )))
}

fn flag(r: &mut Reader<'_>) -> Result<bool, ReprError> {
    r.read_bool().map_err(wire)
}

fn string(r: &mut Reader<'_>, bound: usize, what: &str) -> Result<String, ReprError> {
    let s = r.read_string().map_err(wire)?;
    if s.len() > bound {
        return Err(ReprError::Malformed(format!(
            "{what} of {} characters exceeds string<{bound}>",
            s.len()
        )));
    }
    Ok(s)
}

fn optional_string(
    r: &mut Reader<'_>,
    bound: usize,
    what: &str,
) -> Result<Option<String>, ReprError> {
    if flag(r)? {
        string(r, bound, what).map(Some)
    } else {
        Ok(None)
    }
}

fn octets(r: &mut Reader<'_>) -> Result<Vec<u8>, ReprError> {
    let n = r.read_u32().map_err(wire)? as usize;
    if n > r.remaining() {
        return Err(ReprError::Malformed(format!(
            "sequence<octet> of {n} in {} bytes",
            r.remaining()
        )));
    }
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(r.read_u8().map_err(wire)?);
    }
    Ok(out)
}

fn string_seq(r: &mut Reader<'_>) -> Result<Vec<String>, ReprError> {
    let n = r.read_u32().map_err(wire)? as usize;
    // A string takes at least its length word.
    if n > r.remaining() / 4 {
        return Err(ReprError::Malformed(format!(
            "sequence<string> of {n} in {} bytes",
            r.remaining()
        )));
    }
    (0..n).map(|_| string(r, usize::MAX, "string")).collect()
}

/// `OBJK_DomainParticipant_Binary` (§7.7.3.6.3, Annex A).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParticipantBinary {
    /// `long domain_id` — the form of the clauses only.
    pub domain_id: Option<i32>,
    /// `domain_reference`.
    pub domain_reference: Option<String>,
    /// `qos_profile_reference`.
    pub qos_profile_reference: Option<String>,
}

/// Reads an `OBJK_DomainParticipant_Binary`.
///
/// # Errors
/// [`ReprError::Malformed`] when `bytes` are neither form.
pub(crate) fn participant(bytes: &[u8], big_endian: bool) -> Result<ParticipantBinary, ReprError> {
    either(
        "OBJK_DomainParticipant_Binary",
        &[Form::AnnexA, Form::Clauses, Form::References],
        |form| {
            structure(
                bytes,
                big_endian,
                form,
                "OBJK_DomainParticipant_Binary",
                |r| {
                    let domain_id = match form {
                        Form::AnnexA | Form::References => None,
                        Form::Clauses => Some(r.read_i32().map_err(wire)?),
                    };
                    Ok(ParticipantBinary {
                        domain_id,
                        domain_reference: optional_string(r, REFERENCE_BOUND, "domain_reference")?,
                        qos_profile_reference: optional_string(
                            r,
                            REFERENCE_BOUND,
                            "qos_profile_reference",
                        )?,
                    })
                },
            )
        },
    )
    .map(|(_, v)| v)
}

/// `OBJK_Topic_Binary` (§7.7.3.7.3, Annex A).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TopicBinary {
    /// `topic_name`.
    pub topic_name: String,
    /// `type_reference`: the fully qualified name of the type.
    pub type_reference: Option<String>,
    /// `type_identifier`: the XTypes `TypeIdentifier` of the type; it takes
    /// precedence over `type_reference`.
    pub type_identifier: Option<TypeIdentifier>,
}

/// Reads an `OBJK_Topic_Binary`.
///
/// # Errors
/// [`ReprError::Malformed`] when `bytes` are neither form, or name no type
/// (§7.7.3.7.3: "Either member may be omitted, but not both").
pub(crate) fn topic(bytes: &[u8], big_endian: bool) -> Result<TopicBinary, ReprError> {
    let (_, topic) = either(
        "OBJK_Topic_Binary",
        &[Form::AnnexA, Form::Clauses],
        |form| {
            structure(bytes, big_endian, form, "OBJK_Topic_Binary", |r| {
                let topic_name = string(r, NAME_BOUND, "topic_name")?;
                let type_reference = optional_string(r, NAME_BOUND, "type_reference")?;
                let type_identifier = if flag(r)? {
                    Some(TypeIdentifier::decode_from(r).map_err(wire)?)
                } else {
                    None
                };
                Ok(TopicBinary {
                    topic_name,
                    type_reference,
                    type_identifier,
                })
            })
        },
    )?;
    if topic.type_reference.is_none() && topic.type_identifier.is_none() {
        return Err(ReprError::Malformed(format!(
            "OBJK_Topic_Binary `{}` has neither a type_reference nor a type_identifier",
            topic.topic_name
        )));
    }
    Ok(topic)
}

/// `OBJK_Publisher_Binary` / `OBJK_Subscriber_Binary` (§7.7.3.8.2,
/// §7.7.3.9.2, Annex A).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ContainerBinary {
    /// `publisher_name` / `subscriber_name`.
    pub name: Option<String>,
    /// `qos.partitions`.
    pub partitions: Option<Vec<String>>,
    /// `qos.group_data`.
    pub group_data: Option<Vec<u8>>,
}

/// Reads an `OBJK_Publisher_Binary` or `OBJK_Subscriber_Binary` in Annex A's
/// form (a DHEADER in front). The form of the clauses has no DHEADER and is
/// the one the eProsima client sends, which the caller reads.
///
/// # Errors
/// [`ReprError::Malformed`] when `bytes` are not that form.
pub(crate) fn container(bytes: &[u8], big_endian: bool) -> Result<ContainerBinary, ReprError> {
    structure(
        bytes,
        big_endian,
        Form::AnnexA,
        "OBJK_Publisher_Binary",
        |r| {
            let name = optional_string(r, usize::MAX, "name")?;
            let mut out = ContainerBinary {
                name,
                ..ContainerBinary::default()
            };
            if flag(r)? {
                if flag(r)? {
                    out.partitions = Some(string_seq(r)?);
                }
                if flag(r)? {
                    out.group_data = Some(octets(r)?);
                }
            }
            Ok(out)
        },
    )
}

/// `OBJK_Endpoint_Binary_Qos` and the members the role adds (§7.7.3.10.2,
/// §7.7.3.11.2, Annex A).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct EndpointQosBinary {
    /// `qos_flags`.
    pub qos_flags: u16,
    /// `history_depth`.
    pub history_depth: Option<u16>,
    /// `deadline_msec`.
    pub deadline_msec: Option<u32>,
    /// `lifespan_msec`.
    pub lifespan_msec: Option<u32>,
    /// `user_data`.
    pub user_data: Option<Vec<u8>>,
    /// `ownership_strength` of a DataWriter.
    pub ownership_strength: Option<u32>,
    /// `timebasedfilter_msec` of a DataReader.
    pub timebasedfilter_msec: Option<u32>,
    /// `contentbased_filter` of a DataReader.
    pub contentbased_filter: Option<String>,
}

/// `OBJK_DataWriter_Binary` / `OBJK_DataReader_Binary`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EndpointBinary {
    /// `topic_name`.
    pub topic_name: String,
    /// The QoS, when the structure carries one.
    pub qos: Option<EndpointQosBinary>,
    /// Whether bit 1 of `qos_flags` is `is_history_keep_all` (Annex A) and
    /// not `is_history_keep_last` (the clauses).
    pub bit1_is_keep_all: bool,
}

/// Reads an `OBJK_DataWriter_Binary` (`writer`) or `OBJK_DataReader_Binary`.
///
/// # Errors
/// [`ReprError::Malformed`] when `bytes` are neither form.
pub(crate) fn endpoint(
    bytes: &[u8],
    big_endian: bool,
    writer: bool,
) -> Result<EndpointBinary, ReprError> {
    let what = if writer {
        "OBJK_DataWriter_Binary"
    } else {
        "OBJK_DataReader_Binary"
    };
    either(what, &[Form::AnnexA, Form::Clauses], |form| {
        structure(bytes, big_endian, form, what, |r| {
            let topic_name = string(r, usize::MAX, "topic_name")?;
            let qos = match form {
                // Annex A: `@optional OBJK_Data{Writer,Reader}_Binary_Qos qos`.
                Form::AnnexA => {
                    if flag(r)? {
                        Some(endpoint_qos(r, writer)?)
                    } else {
                        None
                    }
                }
                // The clauses: the QoS is a member of its own, not optional.
                Form::Clauses | Form::References => Some(endpoint_qos(r, writer)?),
            };
            Ok(EndpointBinary {
                topic_name,
                qos,
                bit1_is_keep_all: form == Form::AnnexA,
            })
        })
    })
    .map(|(_, v)| v)
}

fn endpoint_qos(r: &mut Reader<'_>, writer: bool) -> Result<EndpointQosBinary, ReprError> {
    let mut qos = EndpointQosBinary {
        qos_flags: r.read_u16().map_err(wire)?,
        ..EndpointQosBinary::default()
    };
    if flag(r)? {
        qos.history_depth = Some(r.read_u16().map_err(wire)?);
    }
    if flag(r)? {
        qos.deadline_msec = Some(r.read_u32().map_err(wire)?);
    }
    if flag(r)? {
        qos.lifespan_msec = Some(r.read_u32().map_err(wire)?);
    }
    if flag(r)? {
        qos.user_data = Some(octets(r)?);
    }
    if writer {
        if flag(r)? {
            qos.ownership_strength = Some(r.read_u32().map_err(wire)?);
        }
    } else {
        if flag(r)? {
            qos.timebasedfilter_msec = Some(r.read_u32().map_err(wire)?);
        }
        qos.contentbased_filter = optional_string(r, usize::MAX, "contentbased_filter")?;
    }
    Ok(qos)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn le(v: u32) -> [u8; 4] {
        v.to_le_bytes()
    }

    /// §7.7.3.1.3: the 36 octets the clause gives for the topic "Square" of
    /// type `MyTypes::ShapeType`, little-endian, in the form of the clause
    /// (`@extensibility(FINAL)`): length 7 and "Square\0", the presence of
    /// `type_reference` (0x01), length 19 and "MyTypes::ShapeType\0", and
    /// 0x00 for the absent `type_identifier`.
    #[test]
    fn the_36_octet_example_of_the_clause_is_a_topic() {
        let bytes: [u8; 36] = [
            0x07, 0x00, 0x00, 0x00, b'S', b'q', b'u', b'e', b'r', b'e', 0x00, 0x01, 0x13, 0x00,
            0x00, 0x00, b'M', b'y', b'T', b'y', b'p', b'e', b's', b':', b':', b'S', b'h', b'a',
            b'p', b'e', b'T', b'y', b'p', b'e', 0x00, 0x00,
        ];
        assert_eq!(
            topic(&bytes, false).unwrap(),
            TopicBinary {
                // The octets of the clause spell 'S','q','u','e','r','e'
                // although its text calls the topic "Square".
                topic_name: "Squere".into(),
                type_reference: Some("MyTypes::ShapeType".into()),
                type_identifier: None,
            }
        );
    }

    /// Annex A: the same topic as an `@extensibility(APPENDABLE)` structure,
    /// a DHEADER with the length of the members in front (here 36, the
    /// members of the example above).
    #[test]
    fn annex_a_topic_has_a_dheader() {
        let members: [u8; 36] = [
            0x07, 0x00, 0x00, 0x00, b'S', b'q', b'u', b'e', b'r', b'e', 0x00, 0x01, 0x13, 0x00,
            0x00, 0x00, b'M', b'y', b'T', b'y', b'p', b'e', b's', b':', b':', b'S', b'h', b'a',
            b'p', b'e', b'T', b'y', b'p', b'e', 0x00, 0x00,
        ];
        let mut bytes = le(36).to_vec();
        bytes.extend_from_slice(&members);
        let t = topic(&bytes, false).unwrap();
        assert_eq!(t.topic_name, "Squere");
        assert_eq!(t.type_reference.as_deref(), Some("MyTypes::ShapeType"));
        // Big-endian: the members are length 7 "Square\0", 1, length 3
        // "Ty\0", 0 — 20 octets.
        let mut be = Vec::new();
        be.extend_from_slice(&20u32.to_be_bytes());
        be.extend_from_slice(&7u32.to_be_bytes());
        be.extend_from_slice(b"Square\0");
        be.push(1);
        be.extend_from_slice(&3u32.to_be_bytes());
        be.extend_from_slice(b"Ty\0");
        be.push(0);
        assert_eq!(be.len(), 24);
        assert_eq!(
            topic(&be, true).unwrap().type_reference.as_deref(),
            Some("Ty")
        );
    }

    /// §7.7.3.7.3: a `type_identifier` is read as the XTypes
    /// `TypeIdentifier`, and a topic names its type by one of the two.
    #[test]
    fn a_topic_with_a_type_identifier_reads_it_and_one_without_a_type_is_refused() {
        // Clause form: length 2 "T\0" (0..6), type_reference? 0 (6),
        // type_identifier? 1 (7), then TI_STRING8_SMALL (0x70) with the
        // bound 16: a TypeIdentifier that needs no hash.
        let bytes = [2, 0, 0, 0, b'T', 0, 0, 1, 0x70, 0x10];
        let t = topic(&bytes, false).unwrap();
        assert!(t.type_identifier.is_some());
        assert_eq!(t.type_reference, None);
        // Neither member: refused.
        let none = [2, 0, 0, 0, b'T', 0, 0, 0];
        assert!(matches!(topic(&none, false), Err(ReprError::Malformed(_))));
    }

    /// `string<256> topic_name`: a longer one is refused.
    #[test]
    fn string_bounds_are_checked() {
        let name = "n".repeat(257);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&le(u32::try_from(name.len() + 1).unwrap()));
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(0); // NUL, ends at 262
        bytes.extend_from_slice(&[1, 0]); // type_reference present, pad to 264
        bytes.extend_from_slice(&le(2));
        bytes.extend_from_slice(b"T\0");
        bytes.push(0);
        assert!(topic(&bytes, false).is_err());
    }

    /// §7.7.3.6.3 (the clause): `long domain_id` first, then the two
    /// optional references. Annex A: a DHEADER and the two references only.
    #[test]
    fn participant_binary_in_both_forms() {
        // 0..4 domain_id 7; 4 domain_reference? 1; 5..8 padding; 8..12
        // length 4; 12..16 "Lib\0"; 16 qos_profile_reference? 0.
        let clause = [7, 0, 0, 0, 1, 0, 0, 0, 4, 0, 0, 0, b'L', b'i', b'b', 0, 0];
        let p = participant(&clause, false).unwrap();
        assert_eq!(p.domain_id, Some(7));
        assert_eq!(p.domain_reference.as_deref(), Some("Lib"));
        assert_eq!(p.qos_profile_reference, None);

        // Annex A: DHEADER 2, both references absent.
        let annex = [2u8, 0, 0, 0, 0, 0];
        let p = participant(&annex, false).unwrap();
        assert_eq!(p.domain_id, None);
        assert_eq!(p.domain_reference, None);
        assert_eq!(p.qos_profile_reference, None);
        // The eProsima client sends the two references with neither a
        // DHEADER nor a domain_id: the captured blob is `00 00`.
        let captured = participant(&[0, 0], false).unwrap();
        assert_eq!(captured.domain_id, None);
        assert_eq!(captured.domain_reference, None);
        // None of the forms.
        assert!(participant(&[1, 2, 3], false).is_err());
    }

    /// Annex A `OBJK_Publisher_Binary`: a DHEADER, an optional name, an
    /// optional qos with optional partitions and group_data.
    ///
    /// 0 DHEADER 27; 4 name? 1; 5..8 padding; 8..12 length 3; 12..15 "Pu\0";
    /// 15 qos? 1; 16 partitions? 1; 17..20 padding; 20..24 count 1; 24..28
    /// length 2; 28..30 "p\0"; 30 group_data? 0.
    #[test]
    fn publisher_binary_of_annex_a() {
        let bytes = [
            27, 0, 0, 0, 1, 0, 0, 0, 3, 0, 0, 0, b'P', b'u', 0, 1, 1, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0,
            0, b'p', 0, 0,
        ];
        assert_eq!(bytes.len(), 31);
        let c = container(&bytes, false).unwrap();
        assert_eq!(c.name.as_deref(), Some("Pu"));
        assert_eq!(c.partitions, Some(vec!["p".to_string()]));
        assert_eq!(c.group_data, None);
        // Not Annex A: the form without a DHEADER.
        assert!(container(&bytes[4..], false).is_err());
    }

    /// Annex A `OBJK_DataWriter_Binary` with a QoS (the Annex B.2.4 case:
    /// reliable, keep all, a deadline of two minutes).
    ///
    /// 0 DHEADER 23; 4..8 length 7; 8..15 "Square\0"; 15 qos? 1; 16..18
    /// qos_flags 0x0003; 18 history_depth? 0; 19 deadline? 1; 20..24
    /// deadline 120000; 24 lifespan? 0; 25 user_data? 0; 26
    /// ownership_strength? 0.
    #[test]
    fn writer_binary_of_annex_a_with_qos() {
        let mut bytes = vec![23, 0, 0, 0, 7, 0, 0, 0];
        bytes.extend_from_slice(b"Square\0");
        bytes.extend_from_slice(&[1, 3, 0, 0, 1]);
        bytes.extend_from_slice(&le(120_000));
        bytes.extend_from_slice(&[0, 0, 0]);
        assert_eq!(bytes.len(), 27);
        let w = endpoint(&bytes, false, true).unwrap();
        assert_eq!(w.topic_name, "Square");
        assert!(w.bit1_is_keep_all);
        let q = w.qos.unwrap();
        assert_eq!(q.qos_flags, 3);
        assert_eq!(q.history_depth, None);
        assert_eq!(q.deadline_msec, Some(120_000));
        assert_eq!(q.lifespan_msec, None);
        assert_eq!(q.user_data, None);
        assert_eq!(q.ownership_strength, None);
        // Without a QoS: `qos?` is 0 and the structure ends.
        let mut plain = vec![12, 0, 0, 0, 7, 0, 0, 0];
        plain.extend_from_slice(b"Square\0");
        plain.push(0);
        let w = endpoint(&plain, false, true).unwrap();
        assert_eq!(w.qos, None);
    }

    /// Annex A `OBJK_DataWriter_Binary_Qos` with an `ownership_strength`: the
    /// last member of the writer's QoS, an `@optional unsigned long` after the
    /// presence octet, aligned to 4.
    ///
    /// 0 DHEADER 20; 4..8 length 3; 8..11 "Sq\0"; 11 qos? 1; 12..14 qos_flags 0;
    /// 14 history_depth? 0; 15 deadline? 0; 16 lifespan? 0; 17 user_data? 0;
    /// 18 ownership_strength? 1; 19 padding; 20..24 9.
    #[test]
    fn writer_binary_of_annex_a_with_ownership_strength() {
        let bytes = [
            20, 0, 0, 0, 3, 0, 0, 0, b'S', b'q', 0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 9, 0, 0, 0,
        ];
        assert_eq!(bytes.len(), 24);
        let w = endpoint(&bytes, false, true).unwrap();
        assert_eq!(w.topic_name, "Sq");
        let q = w.qos.unwrap();
        assert_eq!(q.ownership_strength, Some(9));
        assert_eq!(q.deadline_msec, None);
        // Big-endian: the number in the byte order of the message.
        let be = [
            0, 0, 0, 20, 0, 0, 0, 3, b'S', b'q', 0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 9,
        ];
        let w = endpoint(&be, true, true).unwrap();
        assert_eq!(w.qos.unwrap().ownership_strength, Some(9));
    }

    /// The clause's `OBJK_DataReader_Binary`: the topic name, the endpoint QoS
    /// (not optional; bit 1 is `is_history_keep_last`) and the reader's
    /// members.
    ///
    /// 0..4 length 3; 4..7 "Sq\0"; 7 padding; 8..10 qos_flags 0x0002; 10
    /// history_depth? 1; 11 padding; 12..14 depth 5; 14 deadline? 0; 15
    /// lifespan? 0; 16 user_data? 0; 17 timebasedfilter_msec? 1; 18..20
    /// padding; 20..24 50; 24 contentbased_filter? 0.
    #[test]
    fn reader_binary_of_the_clause() {
        let bytes = [
            3, 0, 0, 0, b'S', b'q', 0, 0, 2, 0, 1, 0, 5, 0, 0, 0, 0, 1, 0, 0, 50, 0, 0, 0, 0,
        ];
        assert_eq!(bytes.len(), 25);
        let r = endpoint(&bytes, false, false).unwrap();
        assert_eq!(r.topic_name, "Sq");
        assert!(!r.bit1_is_keep_all);
        let q = r.qos.unwrap();
        assert_eq!(q.qos_flags, 2);
        assert_eq!(q.history_depth, Some(5));
        assert_eq!(q.timebasedfilter_msec, Some(50));
        assert_eq!(q.contentbased_filter, None);
    }
}
