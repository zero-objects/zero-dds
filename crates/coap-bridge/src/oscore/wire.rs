//! OSCORE wire pieces: the CoAP OSCORE option (No. 9) codec (RFC 8613 §6.1) and
//! the anti-replay window (RFC 8613 §3.2.2 / §7.4).
//!
//! The OSCORE option carries the compressed COSE Encrypt0 header — the Partial
//! IV, the `kid` (Sender ID) and the optional `kid context` (ID Context). The
//! replay window is the Recipient's sliding window over received Partial IVs.
//!
//! `no_std + alloc`.

extern crate alloc;

use alloc::vec::Vec;

use super::OscoreError;

/// The decoded OSCORE option fields (RFC 8613 §6.1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OscoreOption {
    /// Partial IV (`n` bytes; empty if absent).
    pub partial_iv: Vec<u8>,
    /// `kid` = Sender ID (present iff the `k` flag is set; may be empty).
    pub kid: Option<Vec<u8>>,
    /// `kid context` = ID Context (present iff the `h` flag is set).
    pub kid_context: Option<Vec<u8>>,
}

impl OscoreOption {
    /// Encode the OSCORE option value (RFC 8613 §6.1). An all-empty option (no
    /// Partial IV, no `kid`, no `kid context`) encodes to the empty byte string.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let n = self.partial_iv.len().min(7) as u8;
        let k = u8::from(self.kid.is_some());
        let h = u8::from(self.kid_context.is_some());
        if n == 0 && k == 0 && h == 0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        out.push(n | (k << 3) | (h << 4));
        out.extend_from_slice(&self.partial_iv[..n as usize]);
        if let Some(ctx) = &self.kid_context {
            out.push(ctx.len() as u8); // s
            out.extend_from_slice(ctx);
        }
        if let Some(kid) = &self.kid {
            out.extend_from_slice(kid); // kid runs to the end of the option
        }
        out
    }

    /// Decode an OSCORE option value (RFC 8613 §6.1).
    ///
    /// # Errors
    /// `Err` on a malformed/truncated option.
    pub fn decode(bytes: &[u8]) -> Result<Self, OscoreError> {
        if bytes.is_empty() {
            return Ok(Self::default());
        }
        let flag = bytes[0];
        if flag & 0xE0 != 0 {
            return Err(OscoreError); // reserved bits 5-7 must be zero
        }
        let n = (flag & 0x07) as usize;
        let k = (flag >> 3) & 0x01 == 1;
        let h = (flag >> 4) & 0x01 == 1;
        let mut pos = 1usize;
        if pos + n > bytes.len() {
            return Err(OscoreError);
        }
        let partial_iv = bytes[pos..pos + n].to_vec();
        pos += n;
        let mut kid_context = None;
        if h {
            if pos >= bytes.len() {
                return Err(OscoreError);
            }
            let s = bytes[pos] as usize;
            pos += 1;
            if pos + s > bytes.len() {
                return Err(OscoreError);
            }
            kid_context = Some(bytes[pos..pos + s].to_vec());
            pos += s;
        }
        let kid = if k { Some(bytes[pos..].to_vec()) } else { None };
        Ok(Self {
            partial_iv,
            kid,
            kid_context,
        })
    }
}

/// Anti-replay window (RFC 8613 §3.2.2): a sliding window over the highest
/// received Partial IV (interpreted as a big-endian integer). A 64-bit window
/// behind the high-water mark; values at/above the mark advance it, values
/// within the window are accepted once, anything older is rejected.
#[derive(Clone, Debug, Default)]
pub struct ReplayWindow {
    high: u64,
    seen: u64, // bitmap of the `width` values strictly below `high`
    primed: bool,
}

impl ReplayWindow {
    /// Window width (number of out-of-order values tracked below the high mark).
    pub const WIDTH: u64 = 64;

    /// New empty window.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse a (big-endian, ≤8-byte) Partial IV into an integer.
    #[must_use]
    pub fn piv_to_u64(piv: &[u8]) -> u64 {
        let mut v = 0u64;
        for &b in piv.iter().take(8) {
            v = (v << 8) | u64::from(b);
        }
        v
    }

    /// Check + record a received Partial IV. Returns `true` if it is fresh
    /// (accept) and `false` if it is a replay (reject). On accept, the window
    /// state is updated.
    pub fn check_and_update(&mut self, piv: &[u8]) -> bool {
        let seq = Self::piv_to_u64(piv);
        if !self.primed {
            self.primed = true;
            self.high = seq;
            self.seen = 0;
            return true;
        }
        if seq > self.high {
            let shift = seq - self.high;
            if shift >= Self::WIDTH {
                self.seen = 0;
            } else {
                // mark the old high as seen, then shift the window up.
                self.seen = (self.seen << 1) | 1;
                self.seen <<= (shift - 1) as u32;
            }
            self.high = seq;
            true
        } else if seq == self.high {
            false // exact replay of the high-water value
        } else {
            let diff = self.high - seq; // 1..
            if diff > Self::WIDTH {
                return false; // too old
            }
            let bit = 1u64 << (diff - 1);
            if self.seen & bit != 0 {
                false // already seen
            } else {
                self.seen |= bit;
                true
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        let s: alloc::string::String = s.chars().filter(|c| !c.is_whitespace()).collect();
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    // RFC 8613 §6.3 — option with Partial IV 0x05, kid 0x09 (k=1, n=1):
    // flag = 0x09, then PIV 05, then kid 09  =>  09 05 09.
    #[test]
    fn option_piv_kid() {
        let opt = OscoreOption {
            partial_iv: hex("05"),
            kid: Some(hex("09")),
            kid_context: None,
        };
        assert_eq!(opt.encode(), hex("090509"));
        assert_eq!(OscoreOption::decode(&hex("090509")).unwrap(), opt);
    }

    // RFC 8613 §6.3 — option with PIV 0x00, kid context 0x44616c656b, kid 0x25:
    // flag = n(1)|k(8)|h(16) = 0x19; PIV 00; s=05; ctx 44616c656b; kid 25.
    #[test]
    fn option_piv_kidctx_kid() {
        let opt = OscoreOption {
            partial_iv: hex("00"),
            kid: Some(hex("25")),
            kid_context: Some(hex("44616c656b")),
        };
        assert_eq!(
            opt.encode(),
            hex("19 00 05 44616c656b 25".replace(' ', "").as_str())
        );
        assert_eq!(OscoreOption::decode(&opt.encode()).unwrap(), opt);
    }

    #[test]
    fn option_empty() {
        let opt = OscoreOption::default();
        assert_eq!(opt.encode(), Vec::<u8>::new());
        assert_eq!(OscoreOption::decode(&[]).unwrap(), opt);
    }

    #[test]
    fn option_empty_kid_present() {
        // k=1 with empty kid (a Sender ID of zero length) still emits a flag byte.
        let opt = OscoreOption {
            partial_iv: hex("07"),
            kid: Some(Vec::new()),
            kid_context: None,
        };
        let enc = opt.encode();
        assert_eq!(enc, hex("0907"));
        assert_eq!(OscoreOption::decode(&enc).unwrap(), opt);
    }

    #[test]
    fn option_reserved_bits_rejected() {
        assert!(OscoreOption::decode(&[0x20]).is_err());
    }

    #[test]
    fn replay_window_basic() {
        let mut w = ReplayWindow::new();
        assert!(w.check_and_update(&[0x05])); // prime at 5
        assert!(!w.check_and_update(&[0x05])); // exact replay rejected
        assert!(w.check_and_update(&[0x06])); // advance
        assert!(w.check_and_update(&[0x03])); // older-but-in-window, fresh
        assert!(!w.check_and_update(&[0x03])); // now a replay
        assert!(w.check_and_update(&[0x04])); // another in-window fresh
        // jump far ahead resets the window; very old is rejected.
        assert!(w.check_and_update(&[0x01, 0x00])); // 256
        assert!(!w.check_and_update(&[0x06])); // 6 is now far below -> reject
    }

    #[test]
    fn replay_window_out_of_order_accept_once() {
        let mut w = ReplayWindow::new();
        assert!(w.check_and_update(&[0x0a])); // 10
        assert!(w.check_and_update(&[0x08])); // 8 fresh
        assert!(w.check_and_update(&[0x09])); // 9 fresh
        assert!(!w.check_and_update(&[0x08])); // 8 replay
        assert!(!w.check_and_update(&[0x09])); // 9 replay
        assert!(!w.check_and_update(&[0x0a])); // 10 replay
    }
}
