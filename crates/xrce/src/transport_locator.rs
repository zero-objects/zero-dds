// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `TransportLocator`-Wire-Encoding (Spec §7.7.4).
//!
//! Eine `TransportLocator`-Variante kennzeichnet eine erreichbare
//! Transport-Adresse, ueber die ein Agent (oder Client) erreichbar ist.
//! Die Spec definiert drei Groessen-Klassen:
//!
//! ```text
//!   +----+ ----+
//!   |fmt |     |   fmt = 0x00 → Small  (4 Adress + 2 Port = 6 Byte)
//!   +----+     |         0x01 → Medium (4 Adress + 4 Port = 8 Byte)
//!   |  payload |         0x02 → Large  (16 Adress + 4 Port = 20 Byte)
//!   ~          ~         0x03 → String (UTF-8)
//!   +----------+
//! ```
//!
//! `Small` ist gedacht fuer "klassisches IPv4+UDP-Port". `Medium`
//! erlaubt einen 32-Bit-Port-Identifier (z.B. fuer L4-Multipath-IDs).
//! `Large` ist IPv6+Port oder beliebiger 16-Byte-Adressraum (z.B. UUID).
//!
//! Diese Datei kapselt nur das Wire-Format (Endianness folgt dem Aufrufer
//! analog zu anderen XCDR2-Strukturen). Auflosung von String-Locators auf
//! tatsaechliche `SocketAddr` ist Aufgabe des Transport-Layers.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::encoding::{Endianness, read_u32, write_u32};
use crate::error::XrceError;

/// Format-Discriminator (Spec §7.7.4).
pub mod fmt_disc {
    /// `ADDRESS_FORMAT_SMALL`.
    pub const SMALL: u8 = 0x00;
    /// `ADDRESS_FORMAT_MEDIUM`.
    pub const MEDIUM: u8 = 0x01;
    /// `ADDRESS_FORMAT_LARGE`.
    pub const LARGE: u8 = 0x02;
    /// `ADDRESS_FORMAT_STRING` (z.B. `"udpv4://192.168.0.5:7400"`).
    pub const STRING: u8 = 0x03;
    /// TCP-Locator (C6.2.D, Spec §11.3.1) — IPv4 + 2-Byte Port.
    pub const TCP: u8 = 0x04;
    /// Serial-Locator (C6.2.D, Spec Annex C) — Device-String + Baud-Rate.
    pub const SERIAL: u8 = 0x05;
}

/// `ADDRESS_FORMAT_SMALL` (UDPv4 4-Byte Adresse + 2-Byte Port).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TransportLocatorSmall {
    /// 4 Adress-Bytes (z.B. IPv4).
    pub address: [u8; 4],
    /// 2 Port-Bytes (Big-Endian, wie im Netzwerk-Standard).
    pub port: [u8; 2],
}

/// `ADDRESS_FORMAT_MEDIUM` (4-Byte Adresse + 4-Byte Port-Identifier).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TransportLocatorMedium {
    /// 4 Adress-Bytes.
    pub address: [u8; 4],
    /// 4 Port-Bytes (BE).
    pub port: [u8; 4],
}

/// `ADDRESS_FORMAT_LARGE` (16-Byte Adresse + 4-Byte Port).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TransportLocatorLarge {
    /// 16 Adress-Bytes (IPv6 oder UUID).
    pub address: [u8; 16],
    /// 4 Port-Bytes (BE).
    pub port: [u8; 4],
}

/// Cap fuer String-Locator (vermeidet OOM bei boeswilligem Length-Header).
pub const TRANSPORT_LOCATOR_STRING_MAX: usize = 256;

/// Cap fuer Serial-Device-Strings (z.B. `"/dev/ttyUSB0"`).
pub const TRANSPORT_LOCATOR_SERIAL_DEVICE_MAX: usize = 64;

/// TCP-Locator (Spec §11.3.1) — analog UDPv4-Medium aber als eigene
/// Variante markiert, damit Sender/Empfaenger den passenden Transport-
/// Stack waehlen koennen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TransportLocatorTcp {
    /// 4 Adress-Bytes (IPv4).
    pub address: [u8; 4],
    /// 2 Port-Bytes (Big-Endian).
    pub port: [u8; 2],
}

/// Serial-Locator (Spec Annex C) — Device-String + Baud-Rate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportLocatorSerial {
    /// Device-Pfad (z.B. `"/dev/ttyUSB0"` oder `"COM3"`).
    pub device: String,
    /// Baud-Rate (z.B. 115200).
    pub baud_rate: u32,
}

/// `TransportLocator`-Variante.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportLocator {
    /// 4-Byte Adresse + 2-Byte Port.
    Small(TransportLocatorSmall),
    /// 4-Byte Adresse + 4-Byte Port.
    Medium(TransportLocatorMedium),
    /// 16-Byte Adresse + 4-Byte Port.
    Large(TransportLocatorLarge),
    /// String-Form (UTF-8).
    String(String),
    /// TCP-Locator (C6.2.D).
    Tcp(TransportLocatorTcp),
    /// Serial-Locator (C6.2.D).
    Serial(TransportLocatorSerial),
}

impl TransportLocator {
    /// Discriminator-Byte.
    #[must_use]
    pub fn discriminator(&self) -> u8 {
        match self {
            Self::Small(_) => fmt_disc::SMALL,
            Self::Medium(_) => fmt_disc::MEDIUM,
            Self::Large(_) => fmt_disc::LARGE,
            Self::String(_) => fmt_disc::STRING,
            Self::Tcp(_) => fmt_disc::TCP,
            Self::Serial(_) => fmt_disc::SERIAL,
        }
    }

    /// Encodiert. `e` regelt nur die `u32`-Length-Felder (String und
    /// Medium/Large-Port-Lange-Werte folgen `e`).
    ///
    /// # Errors
    /// `PayloadTooLarge`, wenn ein String laenger als
    /// `TRANSPORT_LOCATOR_STRING_MAX` ist.
    pub fn encode(&self, e: Endianness) -> Result<Vec<u8>, XrceError> {
        let mut out = Vec::new();
        out.push(self.discriminator());
        match self {
            Self::Small(s) => {
                out.extend_from_slice(&s.address);
                out.extend_from_slice(&s.port);
            }
            Self::Medium(m) => {
                out.extend_from_slice(&m.address);
                out.extend_from_slice(&m.port);
            }
            Self::Large(l) => {
                out.extend_from_slice(&l.address);
                out.extend_from_slice(&l.port);
            }
            Self::String(s) => {
                if s.len() > TRANSPORT_LOCATOR_STRING_MAX {
                    return Err(XrceError::PayloadTooLarge {
                        limit: TRANSPORT_LOCATOR_STRING_MAX,
                        actual: s.len(),
                    });
                }
                let mut len_buf = [0u8; 4];
                let len_u32 =
                    u32::try_from(s.len() + 1).map_err(|_| XrceError::ValueOutOfRange {
                        message: "TransportLocator string too long",
                    })?;
                write_u32(&mut len_buf, len_u32, e)?;
                out.extend_from_slice(&len_buf);
                out.extend_from_slice(s.as_bytes());
                out.push(0u8); // NUL-Terminator (XCDR2 string<>)
            }
            Self::Tcp(t) => {
                out.extend_from_slice(&t.address);
                out.extend_from_slice(&t.port);
            }
            Self::Serial(s) => {
                if s.device.len() > TRANSPORT_LOCATOR_SERIAL_DEVICE_MAX {
                    return Err(XrceError::PayloadTooLarge {
                        limit: TRANSPORT_LOCATOR_SERIAL_DEVICE_MAX,
                        actual: s.device.len(),
                    });
                }
                let mut len_buf = [0u8; 4];
                let len_u32 =
                    u32::try_from(s.device.len() + 1).map_err(|_| XrceError::ValueOutOfRange {
                        message: "TransportLocator serial device too long",
                    })?;
                write_u32(&mut len_buf, len_u32, e)?;
                out.extend_from_slice(&len_buf);
                out.extend_from_slice(s.device.as_bytes());
                out.push(0u8); // NUL-Terminator
                let mut baud_buf = [0u8; 4];
                write_u32(&mut baud_buf, s.baud_rate, e)?;
                out.extend_from_slice(&baud_buf);
            }
        }
        Ok(out)
    }

    /// Decodiert. Liefert `(locator, bytes_consumed)`.
    ///
    /// # Errors
    /// `UnexpectedEof`, `ValueOutOfRange`, `PayloadTooLarge`.
    pub fn decode(bytes: &[u8], e: Endianness) -> Result<(Self, usize), XrceError> {
        if bytes.is_empty() {
            return Err(XrceError::UnexpectedEof {
                needed: 1,
                offset: 0,
            });
        }
        let disc = bytes[0];
        let body = &bytes[1..];
        match disc {
            fmt_disc::SMALL => {
                if body.len() < 6 {
                    return Err(XrceError::UnexpectedEof {
                        needed: 7,
                        offset: bytes.len(),
                    });
                }
                let mut s = TransportLocatorSmall::default();
                s.address.copy_from_slice(&body[0..4]);
                s.port.copy_from_slice(&body[4..6]);
                Ok((Self::Small(s), 7))
            }
            fmt_disc::MEDIUM => {
                if body.len() < 8 {
                    return Err(XrceError::UnexpectedEof {
                        needed: 9,
                        offset: bytes.len(),
                    });
                }
                let mut m = TransportLocatorMedium::default();
                m.address.copy_from_slice(&body[0..4]);
                m.port.copy_from_slice(&body[4..8]);
                Ok((Self::Medium(m), 9))
            }
            fmt_disc::LARGE => {
                if body.len() < 20 {
                    return Err(XrceError::UnexpectedEof {
                        needed: 21,
                        offset: bytes.len(),
                    });
                }
                let mut l = TransportLocatorLarge::default();
                l.address.copy_from_slice(&body[0..16]);
                l.port.copy_from_slice(&body[16..20]);
                Ok((Self::Large(l), 21))
            }
            fmt_disc::STRING => {
                if body.len() < 4 {
                    return Err(XrceError::UnexpectedEof {
                        needed: 5,
                        offset: bytes.len(),
                    });
                }
                let len = read_u32(&body[0..4], e)? as usize;
                if len > TRANSPORT_LOCATOR_STRING_MAX + 1 {
                    return Err(XrceError::PayloadTooLarge {
                        limit: TRANSPORT_LOCATOR_STRING_MAX,
                        actual: len,
                    });
                }
                if body.len() < 4 + len {
                    return Err(XrceError::UnexpectedEof {
                        needed: 5 + len,
                        offset: bytes.len(),
                    });
                }
                let raw = &body[4..4 + len];
                // NUL-Terminator trimmen
                let trimmed = if let Some((&last, rest)) = raw.split_last() {
                    if last == 0 { rest } else { raw }
                } else {
                    raw
                };
                let s = core::str::from_utf8(trimmed)
                    .map(alloc::string::ToString::to_string)
                    .map_err(|_| XrceError::ValueOutOfRange {
                        message: "TransportLocator string is not valid utf-8",
                    })?;
                Ok((Self::String(s), 1 + 4 + len))
            }
            fmt_disc::TCP => {
                if body.len() < 6 {
                    return Err(XrceError::UnexpectedEof {
                        needed: 7,
                        offset: bytes.len(),
                    });
                }
                let mut t = TransportLocatorTcp::default();
                t.address.copy_from_slice(&body[0..4]);
                t.port.copy_from_slice(&body[4..6]);
                Ok((Self::Tcp(t), 7))
            }
            fmt_disc::SERIAL => {
                if body.len() < 4 {
                    return Err(XrceError::UnexpectedEof {
                        needed: 5,
                        offset: bytes.len(),
                    });
                }
                let dev_len = read_u32(&body[0..4], e)? as usize;
                if dev_len > TRANSPORT_LOCATOR_SERIAL_DEVICE_MAX + 1 {
                    return Err(XrceError::PayloadTooLarge {
                        limit: TRANSPORT_LOCATOR_SERIAL_DEVICE_MAX,
                        actual: dev_len,
                    });
                }
                if body.len() < 4 + dev_len + 4 {
                    return Err(XrceError::UnexpectedEof {
                        needed: 1 + 4 + dev_len + 4,
                        offset: bytes.len(),
                    });
                }
                let raw = &body[4..4 + dev_len];
                let trimmed = if let Some((&last, rest)) = raw.split_last() {
                    if last == 0 { rest } else { raw }
                } else {
                    raw
                };
                let device = core::str::from_utf8(trimmed)
                    .map(alloc::string::ToString::to_string)
                    .map_err(|_| XrceError::ValueOutOfRange {
                        message: "TransportLocator serial device is not valid utf-8",
                    })?;
                let baud_rate = read_u32(&body[4 + dev_len..4 + dev_len + 4], e)?;
                Ok((
                    Self::Serial(TransportLocatorSerial { device, baud_rate }),
                    1 + 4 + dev_len + 4,
                ))
            }
            _ => Err(XrceError::ValueOutOfRange {
                message: "unknown TransportLocator discriminator",
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn small_roundtrip() {
        let l = TransportLocator::Small(TransportLocatorSmall {
            address: [192, 168, 1, 5],
            port: [0x1C, 0xE8], // 7400
        });
        let enc = l.encode(Endianness::Little).unwrap();
        let (back, n) = TransportLocator::decode(&enc, Endianness::Little).unwrap();
        assert_eq!(back, l);
        assert_eq!(n, 7);
    }

    #[test]
    fn medium_roundtrip() {
        let l = TransportLocator::Medium(TransportLocatorMedium {
            address: [10, 0, 0, 1],
            port: [0, 0, 0x1C, 0xE8],
        });
        let enc = l.encode(Endianness::Little).unwrap();
        let (back, n) = TransportLocator::decode(&enc, Endianness::Little).unwrap();
        assert_eq!(back, l);
        assert_eq!(n, 9);
    }

    #[test]
    fn large_roundtrip() {
        let l = TransportLocator::Large(TransportLocatorLarge {
            address: [
                0xFE, 0x80, 0, 0, 0, 0, 0, 0, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0,
            ],
            port: [0, 0, 0x1C, 0xE8],
        });
        let enc = l.encode(Endianness::Big).unwrap();
        let (back, n) = TransportLocator::decode(&enc, Endianness::Big).unwrap();
        assert_eq!(back, l);
        assert_eq!(n, 21);
    }

    #[test]
    fn string_roundtrip() {
        let l = TransportLocator::String("udpv4://192.168.0.5:7400".into());
        let enc = l.encode(Endianness::Little).unwrap();
        let (back, _) = TransportLocator::decode(&enc, Endianness::Little).unwrap();
        assert_eq!(back, l);
    }

    #[test]
    fn unknown_discriminator_rejected() {
        let res = TransportLocator::decode(&[0x99, 0, 0, 0, 0, 0, 0], Endianness::Little);
        assert!(matches!(res, Err(XrceError::ValueOutOfRange { .. })));
    }

    #[test]
    fn small_truncated_returns_eof() {
        let res = TransportLocator::decode(&[fmt_disc::SMALL, 1, 2, 3], Endianness::Little);
        assert!(matches!(res, Err(XrceError::UnexpectedEof { .. })));
    }

    #[test]
    fn string_too_long_rejected_on_encode() {
        let s: alloc::string::String = (0..TRANSPORT_LOCATOR_STRING_MAX + 1).map(|_| 'a').collect();
        let l = TransportLocator::String(s);
        let res = l.encode(Endianness::Little);
        assert!(matches!(res, Err(XrceError::PayloadTooLarge { .. })));
    }

    #[test]
    fn string_oversized_length_rejected_on_decode() {
        let mut bad = alloc::vec![fmt_disc::STRING];
        let huge_len = (TRANSPORT_LOCATOR_STRING_MAX as u32) + 100;
        bad.extend_from_slice(&huge_len.to_le_bytes());
        let res = TransportLocator::decode(&bad, Endianness::Little);
        assert!(matches!(res, Err(XrceError::PayloadTooLarge { .. })));
    }

    #[test]
    fn tcp_roundtrip_le() {
        let l = TransportLocator::Tcp(TransportLocatorTcp {
            address: [192, 168, 1, 5],
            port: [0x1C, 0xE8],
        });
        let enc = l.encode(Endianness::Little).unwrap();
        let (back, n) = TransportLocator::decode(&enc, Endianness::Little).unwrap();
        assert_eq!(back, l);
        assert_eq!(n, 7);
        assert_eq!(enc[0], fmt_disc::TCP);
    }

    #[test]
    fn tcp_roundtrip_be() {
        let l = TransportLocator::Tcp(TransportLocatorTcp {
            address: [10, 0, 0, 1],
            port: [0x00, 0x50],
        });
        let enc = l.encode(Endianness::Big).unwrap();
        let (back, _) = TransportLocator::decode(&enc, Endianness::Big).unwrap();
        assert_eq!(back, l);
    }

    #[test]
    fn tcp_truncated_returns_eof() {
        let res = TransportLocator::decode(&[fmt_disc::TCP, 1, 2, 3], Endianness::Little);
        assert!(matches!(res, Err(XrceError::UnexpectedEof { .. })));
    }

    #[test]
    fn serial_roundtrip_typical_device() {
        let l = TransportLocator::Serial(TransportLocatorSerial {
            device: "/dev/ttyUSB0".into(),
            baud_rate: 115200,
        });
        let enc = l.encode(Endianness::Little).unwrap();
        let (back, _) = TransportLocator::decode(&enc, Endianness::Little).unwrap();
        assert_eq!(back, l);
    }

    #[test]
    fn serial_roundtrip_windows_com_port() {
        let l = TransportLocator::Serial(TransportLocatorSerial {
            device: "COM3".into(),
            baud_rate: 9600,
        });
        let enc = l.encode(Endianness::Big).unwrap();
        let (back, _) = TransportLocator::decode(&enc, Endianness::Big).unwrap();
        assert_eq!(back, l);
    }

    #[test]
    fn serial_too_long_device_rejected_on_encode() {
        let dev: alloc::string::String = (0..TRANSPORT_LOCATOR_SERIAL_DEVICE_MAX + 1)
            .map(|_| 'a')
            .collect();
        let l = TransportLocator::Serial(TransportLocatorSerial {
            device: dev,
            baud_rate: 9600,
        });
        let res = l.encode(Endianness::Little);
        assert!(matches!(res, Err(XrceError::PayloadTooLarge { .. })));
    }

    #[test]
    fn serial_oversized_length_rejected_on_decode() {
        let mut bad = alloc::vec![fmt_disc::SERIAL];
        let huge: u32 = (TRANSPORT_LOCATOR_SERIAL_DEVICE_MAX as u32) + 200;
        bad.extend_from_slice(&huge.to_le_bytes());
        let res = TransportLocator::decode(&bad, Endianness::Little);
        assert!(matches!(res, Err(XrceError::PayloadTooLarge { .. })));
    }

    #[test]
    fn serial_truncated_baud_rate_returns_eof() {
        // 4 Byte len-Header (=5 → 4 device + NUL), 4 device + NUL, KEINE Baud-Bytes.
        let mut bad = alloc::vec![fmt_disc::SERIAL];
        bad.extend_from_slice(&5u32.to_le_bytes());
        bad.extend_from_slice(b"COM1\0");
        let res = TransportLocator::decode(&bad, Endianness::Little);
        assert!(matches!(res, Err(XrceError::UnexpectedEof { .. })));
    }
}
