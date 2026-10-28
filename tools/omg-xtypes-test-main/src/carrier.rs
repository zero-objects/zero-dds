// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The DDS topic type of a run, which is not known until the run starts.
//!
//! `DdsType` states a type's name, identity, extensibility and keyedness
//! through associated items. For a compiled-in type those are constants,
//! and the generator writes them. This suite's type arrives on the
//! command line, so this implementation answers the five run-time
//! accessors of `DdsType` — `type_name`, `type_identifier`,
//! `extensibility`, `has_key`, `key_holder_max_size` — out of the
//! [`Context`] the run installs before it creates a participant.
//!
//! The sample itself is carried as the octets the type's own codec
//! produced. `zerodds_wire_codec::encode` executes the layout plan of
//! the model that was read at start-up against a `serde_json::Value`,
//! and `::decode` reads it back; [`Sample::encode`] and
//! [`Sample::decode`] hand those octets to the writer and take them from
//! the reader unchanged. Nothing here reframes anything: the DHEADER of
//! an `@appendable` type and the EMHEADERs of a `@mutable` one are
//! inside the octets, written by the same plan a compiled-in type's
//! generated codec is compiled from.

use std::sync::OnceLock;

use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_dcps::dds_type::{
    DdsType, DecodeError, EncodeError, Extensibility, PlainCdr2BeKeyHolder,
};
use zerodds_types::TypeIdentifier;
use zerodds_wire_codec::{Emit, ProbeRequest};

use crate::model::Model;

/// What a run installs once, before it touches DDS.
pub struct Context {
    /// The type, read from the file the command line named.
    pub model: Model,
    /// The request the sample codec runs under — encoding and byte order
    /// from `-x` and from this build's wire byte order.
    pub request: ProbeRequest,
    /// The topic type name, leaked so the `DdsType` accessor can hand
    /// out a `&'static str`. One per process: the run publishes or
    /// subscribes exactly one type, so nothing accumulates.
    pub type_name: &'static str,
    /// `--disable-type-info`: announce neither `TypeObject` nor
    /// `TypeIdentifier`, which is what drives a peer into Step 2 of
    /// XTypes 1.3 §7.6.3.4.2.
    pub disable_type_info: bool,
}

static CONTEXT: OnceLock<Context> = OnceLock::new();

/// Installs the run's type. Returns `false` if one is already installed.
pub fn install(context: Context) -> bool {
    CONTEXT.set(context).is_ok()
}

/// The installed type.
///
/// # Panics
/// If no [`install`] has run. Every path that reaches a `Sample` runs
/// after one, and a panic here is a wiring defect rather than an input
/// the suite can produce.
#[must_use]
#[allow(clippy::expect_used)]
pub fn context() -> &'static Context {
    CONTEXT
        .get()
        .expect("the run installs its type before it creates a participant")
}

/// The `zerodds_ast_layout::Encoding` a `-x` value and an extensibility
/// name together.
///
/// `-x` gives the XCDR **version**; XTypes 1.3 Table 59 gives the
/// encoding, which is the version crossed with the type's extensibility:
/// XCDR1 frames `@final` and `@appendable` as PLAIN_CDR and `@mutable`
/// as PL_CDR; XCDR2 frames them as PLAIN_CDR2, DELIMITED_CDR and
/// PL_CDR2. Choosing on the version alone would write an `@appendable`
/// type with no DHEADER under XCDR2.
#[must_use]
pub fn encoding_for(
    data_representation: u8,
    extensibility: zerodds_ast::Extensibility,
) -> Encoding {
    use zerodds_ast::Extensibility as E;
    if data_representation == 1 {
        match extensibility {
            E::Final | E::Appendable => Encoding::PlainCdr1,
            E::Mutable => Encoding::PlCdr1,
        }
    } else {
        match extensibility {
            E::Final => Encoding::PlainCdr2,
            E::Appendable => Encoding::DelimitedCdr,
            E::Mutable => Encoding::PlCdr2,
        }
    }
}

/// Builds the codec request for a data representation and byte order.
#[must_use]
pub fn request_for(
    type_name: &str,
    data_representation: u8,
    extensibility: zerodds_ast::Extensibility,
    big_endian: bool,
) -> ProbeRequest {
    ProbeRequest::new(
        type_name,
        encoding_for(data_representation, extensibility),
        if big_endian {
            Endianness::Big
        } else {
            Endianness::Little
        },
    )
    .emitting(Emit::Payload)
}

/// One sample, as the octets its type's layout plan produced.
#[derive(Debug, Clone)]
pub struct Sample {
    /// The serialised sample, with no encapsulation header.
    pub bytes: Vec<u8>,
    /// The KeyHash of the instance (DDSI-RTPS 2.5 §9.6.3.8), computed by
    /// the same plan. All sixteen octets, so the `DdsType`
    /// `key_holder_max_size` of `Some(16)` makes the DCPS side copy them
    /// verbatim rather than digest them a second time.
    pub key_hash: [u8; 16],
    /// The value the octets carry, when it could be read back. The
    /// subscriber compares this against the sample it loaded from
    /// `--data-file`; the publisher fills it from that file.
    pub value: Option<serde_json::Value>,
}

impl Sample {
    /// A sample from a value, encoded under the run's request.
    ///
    /// # Errors
    /// The codec's own error, naming the path in the value.
    pub fn from_value(value: &serde_json::Value) -> Result<Self, String> {
        let cx = context();
        let bytes = zerodds_wire_codec::encode(&cx.model.ast, &cx.request, value)
            .map_err(|e| format!("{e:?}"))?;
        Ok(Self {
            key_hash: key_hash_of(value),
            bytes,
            value: Some(value.clone()),
        })
    }
}

/// The sixteen-octet KeyHash of a value, or zeroes for a keyless type.
fn key_hash_of(value: &serde_json::Value) -> [u8; 16] {
    let cx = context();
    if !cx.model.keyed {
        return [0u8; 16];
    }
    // §7.6.8 in full — the key-holder stream, the ≤16 zero-pad branch and
    // the MD5 branch — is what `Emit::KeyHash` executes. Recomputing it
    // here would be a second answer to the same question.
    let request = ProbeRequest::new(&cx.model.type_name, cx.request.encoding, Endianness::Big)
        .emitting(Emit::KeyHash);
    match zerodds_wire_codec::encode(&cx.model.ast, &request, value) {
        Ok(bytes) if bytes.len() == 16 => {
            let mut out = [0u8; 16];
            out.copy_from_slice(&bytes);
            out
        }
        _ => [0u8; 16],
    }
}

impl DdsType for Sample {
    /// Never announced: [`Sample::type_name`] answers instead. A run
    /// that reached DDS without installing a [`Context`] would announce
    /// this name, and it is spelled so that it cannot be mistaken for
    /// one of the suite's.
    const TYPE_NAME: &'static str = "zerodds::omg::xtypes::TypeNotInstalled";

    fn type_name() -> &'static str {
        context().type_name
    }

    fn type_identifier() -> TypeIdentifier {
        let cx = context();
        if cx.disable_type_info {
            TypeIdentifier::None
        } else {
            cx.model.type_identifier.clone()
        }
    }

    fn extensibility() -> Extensibility {
        match context().model.extensibility {
            zerodds_ast::Extensibility::Final => Extensibility::Final,
            zerodds_ast::Extensibility::Appendable => Extensibility::Appendable,
            zerodds_ast::Extensibility::Mutable => Extensibility::Mutable,
        }
    }

    fn has_key() -> bool {
        context().model.keyed
    }

    fn key_holder_max_size() -> Option<usize> {
        // The holder written below is the finished sixteen-octet
        // KeyHash, so the DCPS side must take the zero-pad branch of
        // §7.6.8 step 5 and copy it rather than digest it again.
        Some(16)
    }

    fn type_object() -> Option<zerodds_types::type_object::TypeObject> {
        let cx = context();
        if cx.disable_type_info {
            None
        } else {
            Some(cx.model.type_object.clone())
        }
    }

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.bytes);
        Ok(())
    }

    fn encode_be(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        // The octets were produced under the byte order the run's
        // request names, which is the one the writer stamps on the
        // encapsulation header. There is no second byte order to
        // produce here without re-running the plan, and the writer never
        // asks for one it did not configure.
        out.extend_from_slice(&self.bytes);
        Ok(())
    }

    fn encode_xcdr1(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        // The octets already are what `-x` asked for: the run built its
        // codec request from that flag and from the type's
        // extensibility (`carrier::encoding_for`, XTypes 1.3 Table 59),
        // so there is one stream and both versions hand it out. A run
        // whose writer offered a representation the request was not
        // built for would be a wiring defect, not a stream to convert
        // here — `app::publish` pins the offer to the same flag.
        out.extend_from_slice(&self.bytes);
        Ok(())
    }

    fn encode_xcdr1_be(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.bytes);
        Ok(())
    }

    fn decode_xcdr1(bytes: &[u8]) -> Result<Self, DecodeError> {
        Self::decode(bytes)
    }

    fn decode_xcdr1_be(bytes: &[u8]) -> Result<Self, DecodeError> {
        Self::decode(bytes)
    }

    fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
        holder.write_bytes(&self.key_hash);
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let cx = context();
        // Three outcomes, not two.
        //
        // A body that decodes gives its value. A body that does not is
        // still handed up as octets — the run says what it received and
        // then says the comparison failed, which is a different report
        // than "no sample arrived".
        //
        // And a body the receiving type **discards** is neither. XTypes
        // 1.3 §7.2.4.1: "the default behavior is to discard the O2
        // object to avoid misinterpretation"; §7.3.1.2.1.12 makes
        // DISCARD what an absent `@try_construct` means, so it is the
        // default outcome of a narrowing. The sample must not become a
        // sample: `DecodeError::Discarded` is what the reader path
        // skips on. Reporting it as a decoded-but-different sample said
        // `DATA_NOT_CORRECT` where the suite expects
        // `DATA_NOT_RECEIVED` — fifteen cases, `tryc_str_2` and
        // `tryc_seq_4` and `string_string10` among them.
        match zerodds_wire_codec::decode(&cx.model.ast, &cx.request, bytes) {
            Ok(value) => {
                let key_hash = key_hash_of(&value);
                Ok(Self {
                    bytes: bytes.to_vec(),
                    key_hash,
                    value: Some(value),
                })
            }
            Err(zerodds_wire_codec::ProbeError::Discarded { .. }) => {
                // `DecodeError` carries a `&'static str`, and a reason
                // built per sample would have to be leaked to become
                // one — on a run that discards every sample at 10 Hz
                // that is an unbounded leak. The position and the two
                // lengths are the codec's to report; what the reader
                // needs from here is the verdict.
                Err(DecodeError::Discarded {
                    what: "the value does not fit this member and @try_construct there is DISCARD",
                })
            }
            Err(_) => Ok(Self {
                bytes: bytes.to_vec(),
                key_hash: [0u8; 16],
                value: None,
            }),
        }
    }
}
