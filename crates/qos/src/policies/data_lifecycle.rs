// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Reader/Writer DataLifecycle (DDS 1.4 §2.2.3.20, §2.2.3.21).
//!
//! Wire-Format:
//! - `WriterDataLifecycle`: bool autodispose (4 bytes with padding).
//! - `ReaderDataLifecycle`: 2 × Duration (16 byte).

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

use crate::duration::Duration;
use crate::wire_helpers::{read_bool_padded, write_bool_padded};

/// WriterDataLifecycleQosPolicy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriterDataLifecycleQosPolicy {
    /// Whether to autodispose on unregister_instance. Default: `true`.
    pub autodispose_unregistered_instances: bool,
}

impl Default for WriterDataLifecycleQosPolicy {
    fn default() -> Self {
        Self {
            autodispose_unregistered_instances: true,
        }
    }
}

impl WriterDataLifecycleQosPolicy {
    /// Wire-Encoding (1 bool + 3 pad = 4 byte).
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        write_bool_padded(w, self.autodispose_unregistered_instances)
    }

    /// Wire-Decoding.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            autodispose_unregistered_instances: read_bool_padded(r)?,
        })
    }
}

/// ReaderDataLifecycleQosPolicy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReaderDataLifecycleQosPolicy {
    /// Autopurge NotAlive-no-Writers.
    pub autopurge_nowriter_samples_delay: Duration,
    /// Autopurge Disposed-Samples.
    pub autopurge_disposed_samples_delay: Duration,
}

impl Default for ReaderDataLifecycleQosPolicy {
    fn default() -> Self {
        Self {
            autopurge_nowriter_samples_delay: Duration::INFINITE,
            autopurge_disposed_samples_delay: Duration::INFINITE,
        }
    }
}

impl ReaderDataLifecycleQosPolicy {
    /// Wire-Encoding (2 × Duration = 16 byte).
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        self.autopurge_nowriter_samples_delay.encode_into(w)?;
        self.autopurge_disposed_samples_delay.encode_into(w)
    }

    /// Wire-Decoding.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let nowriter = Duration::decode_from(r)?;
        let disposed = Duration::decode_from(r)?;
        Ok(Self {
            autopurge_nowriter_samples_delay: nowriter,
            autopurge_disposed_samples_delay: disposed,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    #[test]
    fn writer_default_is_autodispose_true() {
        assert!(WriterDataLifecycleQosPolicy::default().autodispose_unregistered_instances);
    }

    #[test]
    fn writer_roundtrip() {
        let p = WriterDataLifecycleQosPolicy {
            autodispose_unregistered_instances: false,
        };
        let mut w = BufferWriter::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(bytes.len(), 4);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(
            WriterDataLifecycleQosPolicy::decode_from(&mut r).unwrap(),
            p
        );
    }

    #[test]
    fn reader_default_is_infinite() {
        let d = ReaderDataLifecycleQosPolicy::default();
        assert!(d.autopurge_nowriter_samples_delay.is_infinite());
        assert!(d.autopurge_disposed_samples_delay.is_infinite());
    }

    #[test]
    fn reader_roundtrip() {
        let p = ReaderDataLifecycleQosPolicy {
            autopurge_nowriter_samples_delay: Duration::from_secs(5),
            autopurge_disposed_samples_delay: Duration::from_secs(10),
        };
        let mut w = BufferWriter::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(bytes.len(), 16);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(
            ReaderDataLifecycleQosPolicy::decode_from(&mut r).unwrap(),
            p
        );
    }
}
