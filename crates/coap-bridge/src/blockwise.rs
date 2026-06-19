// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Block-Wise Transfer — RFC 7959.
//!
//! Spec §2: the block option (Block1 / Block2) allows segmented
//! payload transfer. Encoding of a block value:
//!
//! ```text
//!   NUM (4..20 bit) || M (1 bit) || SZX (3 bit)
//! ```
//!
//! `SZX` encodes the block size as `2^(SZX+4)` (16 bytes..1024 bytes).
//! `M` = "more" flag.

use alloc::vec::Vec;

/// Block option number (RFC 7959 §2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum BlockOption {
    /// `Block1` (option number 27) — transfer in the request body.
    Block1 = 27,
    /// `Block2` (option number 23) — transfer in the response body.
    Block2 = 23,
}

/// Block value (Spec §2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockValue {
    /// Block number (NUM).
    pub num: u32,
    /// "More" flag.
    pub more: bool,
    /// `SZX` (0..=6, 7 reserved); the spec forbids 7.
    pub szx: u8,
}

/// Block codec error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockError {
    /// `SZX = 7` is reserved.
    ReservedSzx,
    /// `NUM > 2^20 - 1`.
    NumTooLarge,
    /// Encoded value exceeds 3 bytes.
    EncodingTooLong,
    /// Decode: 0 or more than 3 bytes.
    BadEncodingLength,
}

impl BlockValue {
    /// Block size in bytes (Spec §2.2: 2^(SZX+4)).
    ///
    /// # Errors
    /// `ReservedSzx` if `szx == 7`.
    pub fn block_size(&self) -> Result<usize, BlockError> {
        if self.szx > 6 {
            return Err(BlockError::ReservedSzx);
        }
        Ok(1usize << (self.szx + 4))
    }

    /// Encode to 1..=3 bytes. Spec §2.2.
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

    /// Decode from 1..=3 bytes.
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

/// Reassembler — collects block slices in the correct order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BlockReassembler {
    /// Expected block size (bytes).
    block_size: usize,
    /// Accumulated payload.
    buf: Vec<u8>,
    /// Next expected `num`.
    next_num: u32,
    /// `true` when the last block (`!more`) has arrived.
    complete: bool,
}

impl BlockReassembler {
    /// Constructor — fixes the block size at the first block.
    #[must_use]
    pub fn new(block_size: usize) -> Self {
        Self {
            block_size,
            buf: Vec::new(),
            next_num: 0,
            complete: false,
        }
    }

    /// Accept a block slice.
    ///
    /// # Errors
    /// Static string if the block sequence is inconsistent (wrong
    /// order, block-size change with data, unexpected more-flag
    /// combination).
    pub fn accept(&mut self, value: BlockValue, payload: &[u8]) -> Result<(), &'static str> {
        if self.complete {
            return Err("reassembler already complete");
        }
        let size = value.block_size().map_err(|_| "block size invalid")?;
        if value.num != self.next_num {
            return Err("block out of order");
        }
        if size != self.block_size {
            // Spec §2.2 allows shrinking SZX at the first block.
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

    /// `true` when all blocks have been received.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// Takes out the collected payload.
    #[must_use]
    pub fn into_payload(self) -> Vec<u8> {
        self.buf
    }

    /// Current length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// `true` when no bytes have been received.
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
