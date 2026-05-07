// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Block-Wise Transfer — RFC 7959.
//!
//! Spec §2: Block-Option (Block1 / Block2) erlaubt segmentierten
//! Payload-Transfer. Encoding eines Block-Wertes:
//!
//! ```text
//!   NUM (4..20 bit) || M (1 bit) || SZX (3 bit)
//! ```
//!
//! `SZX` codiert die Block-Size als `2^(SZX+4)` (16 bytes..1024 bytes).
//! `M` = "more"-Flag.

use alloc::vec::Vec;

/// Block-Option-Number (RFC 7959 §2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum BlockOption {
    /// `Block1` (Option-Number 27) — Transfer im Request-Body.
    Block1 = 27,
    /// `Block2` (Option-Number 23) — Transfer im Response-Body.
    Block2 = 23,
}

/// Block-Wert (Spec §2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockValue {
    /// Block-Number (NUM).
    pub num: u32,
    /// "More"-Flag.
    pub more: bool,
    /// `SZX` (0..=6, 7 reserved); Spec verbietet 7.
    pub szx: u8,
}

/// Block-Codec-Fehler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockError {
    /// `SZX = 7` ist reserved.
    ReservedSzx,
    /// `NUM > 2^20 - 1`.
    NumTooLarge,
    /// Encoded value uebersteigt 3 Bytes.
    EncodingTooLong,
    /// Decode: 0 oder mehr als 3 Bytes.
    BadEncodingLength,
}

impl BlockValue {
    /// Block-Size in Bytes (Spec §2.2: 2^(SZX+4)).
    ///
    /// # Errors
    /// `ReservedSzx` wenn `szx == 7`.
    pub fn block_size(&self) -> Result<usize, BlockError> {
        if self.szx > 6 {
            return Err(BlockError::ReservedSzx);
        }
        Ok(1usize << (self.szx + 4))
    }

    /// Encode zu 1..=3 Bytes. Spec §2.2.
    ///
    /// # Errors
    /// `ReservedSzx` / `NumTooLarge`.
    pub fn encode(&self) -> Result<Vec<u8>, BlockError> {
        if self.szx > 6 {
            return Err(BlockError::ReservedSzx);
        }
        if self.num >= (1 << 20) {
            return Err(BlockError::NumTooLarge);
        }
        let raw = (self.num << 4) | (u32::from(self.more) << 3) | u32::from(self.szx);
        let mut out = Vec::with_capacity(3);
        if raw < 0x100 {
            out.push(raw as u8);
        } else if raw < 0x1_0000 {
            out.push(((raw >> 8) & 0xff) as u8);
            out.push((raw & 0xff) as u8);
        } else {
            out.push(((raw >> 16) & 0xff) as u8);
            out.push(((raw >> 8) & 0xff) as u8);
            out.push((raw & 0xff) as u8);
        }
        Ok(out)
    }

    /// Decode von 1..=3 Bytes.
    ///
    /// # Errors
    /// `BadEncodingLength` / `ReservedSzx`.
    pub fn decode(bytes: &[u8]) -> Result<Self, BlockError> {
        let raw: u32 = match bytes.len() {
            1 => u32::from(bytes[0]),
            2 => (u32::from(bytes[0]) << 8) | u32::from(bytes[1]),
            3 => (u32::from(bytes[0]) << 16) | (u32::from(bytes[1]) << 8) | u32::from(bytes[2]),
            _ => return Err(BlockError::BadEncodingLength),
        };
        let szx = (raw & 0x7) as u8;
        if szx == 7 {
            return Err(BlockError::ReservedSzx);
        }
        let more = (raw >> 3) & 0x1 != 0;
        let num = raw >> 4;
        Ok(Self { num, more, szx })
    }
}

/// Reassembler — sammelt Block-Slices in der korrekten Reihenfolge.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BlockReassembler {
    /// Erwartete Block-Size (Bytes).
    block_size: usize,
    /// Akkumuliertes Payload.
    buf: Vec<u8>,
    /// Naechste erwartete `num`.
    next_num: u32,
    /// `true` wenn der letzte Block (`!more`) angekommen ist.
    complete: bool,
}

impl BlockReassembler {
    /// Konstruktor — fixiert die Block-Size beim ersten Block.
    #[must_use]
    pub fn new(block_size: usize) -> Self {
        Self {
            block_size,
            buf: Vec::new(),
            next_num: 0,
            complete: false,
        }
    }

    /// Akzeptiere einen Block-Slice.
    ///
    /// # Errors
    /// Static-String wenn die Block-Sequenz inkonsistent ist (falsche
    /// Reihenfolge, Block-Size-Wechsel mit Daten, unerwartete more-
    /// Flag-Kombination).
    pub fn accept(&mut self, value: BlockValue, payload: &[u8]) -> Result<(), &'static str> {
        if self.complete {
            return Err("reassembler already complete");
        }
        let size = value.block_size().map_err(|_| "block size invalid")?;
        if value.num != self.next_num {
            return Err("block out of order");
        }
        if size != self.block_size {
            // Spec §2.2 erlaubt SZX-Verkleinerung beim ersten Block.
            if self.next_num == 0 {
                self.block_size = size;
            } else {
                return Err("block size changed mid-transfer");
            }
        }
        if value.more && payload.len() != size {
            return Err("intermediate block size mismatch");
        }
        if !value.more && payload.len() > size {
            return Err("final block too large");
        }
        self.buf.extend_from_slice(payload);
        self.next_num += 1;
        if !value.more {
            self.complete = true;
        }
        Ok(())
    }

    /// `true` wenn alle Blocks empfangen.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// Entnimmt das gesammelte Payload.
    #[must_use]
    pub fn into_payload(self) -> Vec<u8> {
        self.buf
    }

    /// Aktuelle Length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// `true` wenn keine Bytes empfangen.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn block_size_matches_spec_szx_table() {
        // Spec §2.2: szx 0=16, 1=32, 2=64, 3=128, 4=256, 5=512, 6=1024.
        let expected = [16, 32, 64, 128, 256, 512, 1024];
        for (szx, sz) in expected.iter().enumerate() {
            let v = BlockValue {
                num: 0,
                more: false,
                szx: szx as u8,
            };
            assert_eq!(v.block_size().unwrap(), *sz);
        }
    }

    #[test]
    fn szx_7_rejected() {
        let v = BlockValue {
            num: 0,
            more: false,
            szx: 7,
        };
        assert_eq!(v.block_size(), Err(BlockError::ReservedSzx));
    }

    #[test]
    fn round_trip_small_block_value() {
        let v = BlockValue {
            num: 0,
            more: true,
            szx: 6,
        };
        let bytes = v.encode().unwrap();
        let back = BlockValue::decode(&bytes).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn round_trip_large_block_value() {
        let v = BlockValue {
            num: 0xfffff,
            more: false,
            szx: 0,
        };
        let bytes = v.encode().unwrap();
        let back = BlockValue::decode(&bytes).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn num_too_large_rejected() {
        let v = BlockValue {
            num: 1 << 20,
            more: false,
            szx: 0,
        };
        assert_eq!(v.encode(), Err(BlockError::NumTooLarge));
    }

    #[test]
    fn decode_rejects_bad_length() {
        assert_eq!(
            BlockValue::decode(&[0; 4]),
            Err(BlockError::BadEncodingLength)
        );
        assert_eq!(BlockValue::decode(&[]), Err(BlockError::BadEncodingLength));
    }

    #[test]
    fn reassembler_combines_blocks_in_order() {
        let mut r = BlockReassembler::new(16);
        r.accept(
            BlockValue {
                num: 0,
                more: true,
                szx: 0,
            },
            &[1u8; 16],
        )
        .unwrap();
        r.accept(
            BlockValue {
                num: 1,
                more: true,
                szx: 0,
            },
            &[2u8; 16],
        )
        .unwrap();
        r.accept(
            BlockValue {
                num: 2,
                more: false,
                szx: 0,
            },
            &[3u8; 8],
        )
        .unwrap();
        assert!(r.is_complete());
        let buf = r.into_payload();
        assert_eq!(buf.len(), 16 + 16 + 8);
    }

    #[test]
    fn out_of_order_block_rejected() {
        let mut r = BlockReassembler::new(16);
        let err = r
            .accept(
                BlockValue {
                    num: 5,
                    more: true,
                    szx: 0,
                },
                &[1u8; 16],
            )
            .unwrap_err();
        assert_eq!(err, "block out of order");
    }

    #[test]
    fn intermediate_block_size_must_match() {
        let mut r = BlockReassembler::new(16);
        r.accept(
            BlockValue {
                num: 0,
                more: true,
                szx: 0,
            },
            &[1u8; 16],
        )
        .unwrap();
        let err = r
            .accept(
                BlockValue {
                    num: 1,
                    more: true,
                    szx: 0,
                },
                &[2u8; 8],
            )
            .unwrap_err();
        assert_eq!(err, "intermediate block size mismatch");
    }

    #[test]
    fn final_block_can_be_smaller() {
        let mut r = BlockReassembler::new(16);
        r.accept(
            BlockValue {
                num: 0,
                more: false,
                szx: 0,
            },
            &[7u8; 5],
        )
        .unwrap();
        assert!(r.is_complete());
        assert_eq!(r.len(), 5);
    }
}
