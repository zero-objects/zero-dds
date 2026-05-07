// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! SETTINGS Frame — RFC 9113 §6.5.
//!
//! Spec §6.5.1: jeder Setting-Eintrag ist 6 Bytes (2 Byte Identifier
//! + 4 Byte Value).

use alloc::vec::Vec;

use crate::error::Http2Error;

/// Setting-Identifier (RFC 9113 §6.5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum SettingId {
    /// `SETTINGS_HEADER_TABLE_SIZE` (0x1).
    HeaderTableSize = 0x1,
    /// `SETTINGS_ENABLE_PUSH` (0x2).
    EnablePush = 0x2,
    /// `SETTINGS_MAX_CONCURRENT_STREAMS` (0x3).
    MaxConcurrentStreams = 0x3,
    /// `SETTINGS_INITIAL_WINDOW_SIZE` (0x4).
    InitialWindowSize = 0x4,
    /// `SETTINGS_MAX_FRAME_SIZE` (0x5).
    MaxFrameSize = 0x5,
    /// `SETTINGS_MAX_HEADER_LIST_SIZE` (0x6).
    MaxHeaderListSize = 0x6,
}

impl SettingId {
    /// `u16 -> SettingId`. Spec §6.5.2: Empfaenger sollen unbekannte
    /// IDs ignorieren, aber wir liefern hier `None`, der Caller
    /// entscheidet.
    #[must_use]
    pub fn from_u16(v: u16) -> Option<Self> {
        match v {
            0x1 => Some(Self::HeaderTableSize),
            0x2 => Some(Self::EnablePush),
            0x3 => Some(Self::MaxConcurrentStreams),
            0x4 => Some(Self::InitialWindowSize),
            0x5 => Some(Self::MaxFrameSize),
            0x6 => Some(Self::MaxHeaderListSize),
            _ => None,
        }
    }
}

/// Ein einzelner Setting-Eintrag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Setting {
    /// Identifier.
    pub id: SettingId,
    /// Value (32-bit).
    pub value: u32,
}

/// Settings-Map mit Defaults laut Spec §6.5.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// `HEADER_TABLE_SIZE` (default 4096).
    pub header_table_size: u32,
    /// `ENABLE_PUSH` (default 1).
    pub enable_push: u32,
    /// `MAX_CONCURRENT_STREAMS` (default unlimited, repraesentiert als
    /// `u32::MAX`).
    pub max_concurrent_streams: u32,
    /// `INITIAL_WINDOW_SIZE` (default 65_535).
    pub initial_window_size: u32,
    /// `MAX_FRAME_SIZE` (default 16_384).
    pub max_frame_size: u32,
    /// `MAX_HEADER_LIST_SIZE` (default unlimited, `u32::MAX`).
    pub max_header_list_size: u32,
}

impl Default for Settings {
    fn default() -> Self {
        // Spec §6.5.2 — initial values.
        Self {
            header_table_size: 4096,
            enable_push: 1,
            max_concurrent_streams: u32::MAX,
            initial_window_size: 65_535,
            max_frame_size: 16_384,
            max_header_list_size: u32::MAX,
        }
    }
}

impl Settings {
    /// Wendet einen Setting-Eintrag an.
    ///
    /// # Errors
    /// `Protocol(FlowControlError)` wenn `INITIAL_WINDOW_SIZE`
    /// groesser als 2^31-1 (Spec §6.5.2). `Protocol(ProtocolError)`
    /// bei `ENABLE_PUSH` != 0/1 oder `MAX_FRAME_SIZE` ausserhalb
    /// [16384, 16777215].
    pub fn apply(&mut self, s: Setting) -> Result<(), Http2Error> {
        use crate::error::ErrorCode;
        match s.id {
            SettingId::HeaderTableSize => self.header_table_size = s.value,
            SettingId::EnablePush => {
                if s.value > 1 {
                    return Err(Http2Error::Protocol(ErrorCode::ProtocolError));
                }
                self.enable_push = s.value;
            }
            SettingId::MaxConcurrentStreams => self.max_concurrent_streams = s.value,
            SettingId::InitialWindowSize => {
                if s.value > 0x7fff_ffff {
                    return Err(Http2Error::Protocol(ErrorCode::FlowControlError));
                }
                self.initial_window_size = s.value;
            }
            SettingId::MaxFrameSize => {
                if !(16_384..=16_777_215).contains(&s.value) {
                    return Err(Http2Error::Protocol(ErrorCode::ProtocolError));
                }
                self.max_frame_size = s.value;
            }
            SettingId::MaxHeaderListSize => self.max_header_list_size = s.value,
        }
        Ok(())
    }
}

/// Decodiert einen SETTINGS-Frame-Payload zu einer Liste von
/// Settings. Spec §6.5.1.
///
/// # Errors
/// `Protocol(FrameSizeError)` wenn die Length nicht ein Vielfaches
/// von 6 ist.
pub fn decode_settings(payload: &[u8]) -> Result<Vec<Setting>, Http2Error> {
    use crate::error::ErrorCode;
    if payload.len() % 6 != 0 {
        return Err(Http2Error::Protocol(ErrorCode::FrameSizeError));
    }
    let mut out = Vec::with_capacity(payload.len() / 6);
    let mut i = 0;
    while i + 6 <= payload.len() {
        let id_u = (u16::from(payload[i]) << 8) | u16::from(payload[i + 1]);
        let value = (u32::from(payload[i + 2]) << 24)
            | (u32::from(payload[i + 3]) << 16)
            | (u32::from(payload[i + 4]) << 8)
            | u32::from(payload[i + 5]);
        if let Some(id) = SettingId::from_u16(id_u) {
            out.push(Setting { id, value });
        }
        // Spec §6.5.2: unbekannte IDs werden ignoriert.
        i += 6;
    }
    Ok(out)
}

/// Encode eine Liste von Settings zu einem SETTINGS-Frame-Payload.
#[must_use]
pub fn encode_settings(settings: &[Setting]) -> Vec<u8> {
    let mut out = Vec::with_capacity(settings.len() * 6);
    for s in settings {
        let id = s.id as u16;
        out.push((id >> 8) as u8);
        out.push((id & 0xff) as u8);
        out.push(((s.value >> 24) & 0xff) as u8);
        out.push(((s.value >> 16) & 0xff) as u8);
        out.push(((s.value >> 8) & 0xff) as u8);
        out.push((s.value & 0xff) as u8);
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let s = Settings::default();
        assert_eq!(s.header_table_size, 4096);
        assert_eq!(s.enable_push, 1);
        assert_eq!(s.initial_window_size, 65_535);
        assert_eq!(s.max_frame_size, 16_384);
    }

    #[test]
    fn apply_valid_settings() {
        let mut s = Settings::default();
        s.apply(Setting {
            id: SettingId::MaxFrameSize,
            value: 32_768,
        })
        .unwrap();
        assert_eq!(s.max_frame_size, 32_768);
    }

    #[test]
    fn invalid_enable_push_rejected() {
        let mut s = Settings::default();
        assert!(
            s.apply(Setting {
                id: SettingId::EnablePush,
                value: 2,
            })
            .is_err()
        );
    }

    #[test]
    fn invalid_initial_window_size_rejected() {
        let mut s = Settings::default();
        assert!(
            s.apply(Setting {
                id: SettingId::InitialWindowSize,
                value: 0x8000_0000,
            })
            .is_err()
        );
    }

    #[test]
    fn invalid_max_frame_size_rejected() {
        let mut s = Settings::default();
        assert!(
            s.apply(Setting {
                id: SettingId::MaxFrameSize,
                value: 1,
            })
            .is_err()
        );
        assert!(
            s.apply(Setting {
                id: SettingId::MaxFrameSize,
                value: 16_777_216,
            })
            .is_err()
        );
    }

    #[test]
    fn round_trip_encode_decode() {
        let settings = alloc::vec![
            Setting {
                id: SettingId::MaxConcurrentStreams,
                value: 100
            },
            Setting {
                id: SettingId::InitialWindowSize,
                value: 1_048_576,
            },
        ];
        let payload = encode_settings(&settings);
        let decoded = decode_settings(&payload).unwrap();
        assert_eq!(decoded, settings);
    }

    #[test]
    fn unknown_setting_id_ignored() {
        let payload = alloc::vec![0x00, 0xff, 0x00, 0x00, 0x00, 0x01];
        let decoded = decode_settings(&payload).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn odd_length_payload_rejected() {
        let payload = alloc::vec![0; 5];
        assert!(decode_settings(&payload).is_err());
    }
}
