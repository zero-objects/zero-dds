// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// `IoxOracle::KMsg`, the type the Cyclone side of this proof uses:
//
//     module IoxOracle { @final struct KMsg { @key unsigned long id; string name; unsigned long value; }; };
//
// Written against `zerodds_wire` the way `crates/dcps/examples/classic_shapes.rs`
// is: `@final`, no DHEADER, members tight-packed at XCDR2's MAXALIGN 4. With no
// eight-octet member the XCDR1 body equals the XCDR2 body.

#![allow(non_snake_case)]

pub mod IoxOracle {
    use zerodds_dcps::dds_type::{DdsType, DecodeError, EncodeError, Extensibility, PlainCdr2BeKeyHolder};
    use zerodds_wire::{Endianness, Reader, Writer};

    /// XCDR2's `MAXALIGN` (XTypes 1.3 §7.4.1).
    const XCDR2_MAX_ALIGN: usize = 4;

    #[derive(Debug, Clone, PartialEq, Default)]
    pub struct KMsg {
        pub id: u32,
        pub name: String,
        pub value: u32,
    }

    impl KMsg {
        fn write(&self, e: Endianness, out: &mut Vec<u8>) {
            let mut w = Writer::with_max_align(e, XCDR2_MAX_ALIGN);
            w.write_u32(self.id);
            w.write_string(&self.name);
            w.write_u32(self.value);
            out.extend_from_slice(w.as_bytes());
        }

        fn read(bytes: &[u8], e: Endianness) -> Result<Self, DecodeError> {
            let mut r = Reader::with_max_align(bytes, e, XCDR2_MAX_ALIGN);
            let id = r.read_u32()?;
            let name = r.read_string()?;
            let value = r.read_u32()?;
            Ok(Self { id, name, value })
        }
    }

    impl DdsType for KMsg {
        const TYPE_NAME: &'static str = "IoxOracle::KMsg";
        const EXTENSIBILITY: Extensibility = Extensibility::Final;
        const XCDR1_MATCHES_XCDR2: bool = true;
        const HAS_KEY: bool = true;
        const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(4);
        const TYPE_IDENTIFIER: zerodds_types::TypeIdentifier =
            zerodds_types::TypeIdentifier::EquivalenceHashComplete(zerodds_types::EquivalenceHash([
                0xc9, 0x6b, 0x7a, 0x5e, 0x86, 0xef, 0xe2, 0xe6, 0xd1, 0x99, 0xd2, 0x97, 0xcc, 0x4c,
            ]));

        fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
            self.write(Endianness::Little, out);
            Ok(())
        }

        fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
            Self::read(bytes, Endianness::Little)
        }

        fn encode_be(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
            self.write(Endianness::Big, out);
            Ok(())
        }

        fn decode_be(bytes: &[u8]) -> Result<Self, DecodeError> {
            Self::read(bytes, Endianness::Big)
        }

        fn encode_xcdr1(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
            self.encode(out)
        }

        fn decode_xcdr1(bytes: &[u8]) -> Result<Self, DecodeError> {
            Self::decode(bytes)
        }

        fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
            holder.write_u32(self.id);
        }

        fn field_value(&self, path: &str) -> Option<zerodds_sql_filter::Value> {
            match path {
                "id" => Some(zerodds_sql_filter::Value::Int(i64::from(self.id))),
                "name" => Some(zerodds_sql_filter::Value::String(self.name.clone())),
                "value" => Some(zerodds_sql_filter::Value::Int(i64::from(self.value))),
                _ => None,
            }
        }
    }
}
