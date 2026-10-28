// SPDX-License-Identifier: Apache-2.0
//! The wire type of the #32 harness: `Robot` from `idl/Robot.idl`,
//! generated at build time by `zerodds-build`, plus the `DdsType`
//! surface the readers and writers in `src/bin` take.
//!
//! The bridge is hand-written because the generated codec is a pair of
//! inherent functions over `zerodds_wire` and `DdsType` is
//! `zerodds-dcps`'s trait; nothing generates that impl yet. The
//! constants are the retired generator's, and the tests below pin the
//! octets to the shape the old checked-in fixture produced, so the port
//! is measured rather than assumed.

include!(concat!(env!("OUT_DIR"), "/Robot.rs"));

impl zerodds_dcps::DdsType for Robot {
    const TYPE_NAME: &'static str = "Robot";
    const EXTENSIBILITY: zerodds_dcps::Extensibility = zerodds_dcps::Extensibility::Appendable;
    const TYPE_IDENTIFIER: zerodds_types::TypeIdentifier = type_identifiers_robot::Robot;
    // Both members are `unsigned long`, so nothing in this type needs an
    // alignment wider than 4 and the XCDR1 and XCDR2 member octets are
    // identical; the only difference is the DHEADER XCDR2 puts in front of an
    // `@appendable` object (XTypes 1.3 Tab. 59). That is exactly the condition
    // `DdsType::xcdr1_reframes_dheader` asserts, so the trait's XCDR1 default
    // is sound for `Robot`.
    //
    // Without this the default `encode_xcdr1` refuses, and a writer left on the
    // default `DataRepresentation` offer — which starts with XCDR1 — fails
    // every `write`. That is what `robot_hard_reliable` and
    // `robot_graceful_reliable` reported as "gen1 never streamed".
    const XCDR1_MATCHES_XCDR2: bool = true;

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), zerodds_dcps::EncodeError> {
        let mut w = zerodds_wire::Writer::new(zerodds_wire::Endianness::Little);
        Robot::encode(self, &mut w)?;
        out.extend_from_slice(&w.finish());
        Ok(())
    }

    fn encode_be(&self, out: &mut Vec<u8>) -> Result<(), zerodds_dcps::EncodeError> {
        let mut w = zerodds_wire::Writer::new(zerodds_wire::Endianness::Big);
        Robot::encode(self, &mut w)?;
        out.extend_from_slice(&w.finish());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, zerodds_dcps::DecodeError> {
        let mut r = zerodds_wire::Reader::new(bytes, zerodds_wire::Endianness::Little);
        Ok(Robot::decode(&mut r)?)
    }

    fn decode_be(bytes: &[u8]) -> Result<Self, zerodds_dcps::DecodeError> {
        let mut r = zerodds_wire::Reader::new(bytes, zerodds_wire::Endianness::Big);
        Ok(Robot::decode(&mut r)?)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::Robot;
    use zerodds_dcps::DdsType;

    /// XCDR2 `@appendable`: a four-octet DHEADER carrying 8, then the
    /// two members. This is the stream the old fixture's
    /// `encode_appendable` produced; a change here is a wire change.
    #[test]
    fn the_le_octets_are_the_old_fixtures() {
        let mut out = Vec::new();
        DdsType::encode(&Robot { id: 1, label: 2 }, &mut out).unwrap();
        assert_eq!(out, [8, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0]);
    }

    #[test]
    fn the_be_octets_swap_every_field() {
        let mut out = Vec::new();
        Robot { id: 1, label: 2 }.encode_be(&mut out).unwrap();
        assert_eq!(out, [0, 0, 0, 8, 0, 0, 0, 1, 0, 0, 0, 2]);
    }

    /// The constant comes from `zerodds_types::from_ast` at build time
    /// and is pinned here so a drift in that computation is a failing
    /// test, because the hash is what a peer matches the type by.
    ///
    /// Measured 2×: the retired generator emitted `[0x23, 0x49, …]` for
    /// this type; the rebuilt computation — the one held byte-identical
    /// to OpenDDS's `TypeIdentifier`s by `crates/types`' vendor tests —
    /// produces the value below. The two never agreed, so the old
    /// fixture's constant was a hash no vendor computation confirms.
    #[test]
    fn the_type_identifier_is_the_measured_computation() {
        assert_eq!(
            <Robot as DdsType>::TYPE_IDENTIFIER,
            zerodds_types::TypeIdentifier::EquivalenceHashComplete(zerodds_types::EquivalenceHash([
                0x92, 0x5d, 0xb9, 0x08, 0xf0, 0x16, 0xe9, 0x7e, 0x45, 0xe3, 0x21, 0x4e, 0x71, 0x3a,
            ]))
        );
    }

    /// XCDR1 `@appendable` is PLAIN_CDR: the same member octets as the XCDR2
    /// stream above with the four-octet DHEADER removed. This is what a writer
    /// on the default `DataRepresentation` offer puts on the wire.
    #[test]
    fn the_xcdr1_octets_are_the_xcdr2_body_without_the_dheader() {
        let mut out = Vec::new();
        Robot { id: 1, label: 2 }.encode_xcdr1(&mut out).unwrap();
        assert_eq!(out, [1, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(Robot::decode_xcdr1(&out).unwrap(), Robot { id: 1, label: 2 });
    }

    #[test]
    fn both_orders_round_trip() {
        let sample = Robot { id: 7, label: 9 };
        let mut le = Vec::new();
        DdsType::encode(&sample, &mut le).unwrap();
        assert_eq!(<Robot as DdsType>::decode(&le).unwrap(), sample);
        let mut be = Vec::new();
        sample.encode_be(&mut be).unwrap();
        assert_eq!(Robot::decode_be(&be).unwrap(), sample);
    }
}
