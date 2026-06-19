//! Side-channel frame format.
//!
//! Versioned, length-prefixed frames exchanged between the inspect endpoint
//! and the external Reality Inspector. Layer-agnostic:
//! a frame carries a DCPS/RTPS/transport sample plus
//! metadata.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Which tap layer emitted the sample.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FrameKind {
    /// DCPS-layer sample (typed after deserialization).
    Dcps,
    /// RTPS-layer sample (reader/writer cache, sees all transports).
    Rtps,
    /// Transport-layer raw bytes (also sees shared memory).
    Transport,
}

/// A side-channel frame.
///
/// Frames are auditable (Zero-Principle Pillar 6 zero-context-loss):
/// `content_hash` is SHA-256 over `(kind, topic, timestamp_ns,
/// correlation_id, payload)`. Phase-1: the hash field is optional and
/// defaults to `None`; in Phase-2 the hash is set during server-side
/// dispatch.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    /// Which layer emitted the frame.
    pub kind: FrameKind,
    /// Topic name (or empty when a transport layer without a TypeObject).
    pub topic: String,
    /// Wall-clock timestamp in nanoseconds since the UNIX epoch.
    pub timestamp_ns: u64,
    /// Sequence number / sample-identity GUID prefix for cross-
    /// layer correlation (R-077).
    pub correlation_id: u64,
    /// Sample payload as bytes (layer-specific encoding).
    pub payload: Vec<u8>,
    /// Optional content hash (Zero-Principle Foundation 2). Set
    /// by the server when audit streams are active. Phase-1
    /// defaults to `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<[u8; 32]>,
}

impl Frame {
    /// Creates a DCPS frame.
    #[must_use]
    pub fn dcps(topic: String, timestamp_ns: u64, correlation_id: u64, payload: Vec<u8>) -> Self {
        Self {
            kind: FrameKind::Dcps,
            topic,
            timestamp_ns,
            correlation_id,
            payload,
            content_hash: None,
        }
    }

    /// Creates an RTPS frame.
    #[must_use]
    pub fn rtps(topic: String, timestamp_ns: u64, correlation_id: u64, payload: Vec<u8>) -> Self {
        Self {
            kind: FrameKind::Rtps,
            topic,
            timestamp_ns,
            correlation_id,
            payload,
            content_hash: None,
        }
    }

    /// Creates a transport frame.
    #[must_use]
    pub fn transport(timestamp_ns: u64, correlation_id: u64, payload: Vec<u8>) -> Self {
        Self {
            kind: FrameKind::Transport,
            topic: String::new(),
            timestamp_ns,
            correlation_id,
            payload,
            content_hash: None,
        }
    }

    /// Computes the content hash and stores it.
    ///
    /// Hash input: kind tag + topic + timestamp\_ns + correlation\_id +
    /// payload. Idempotent — repeated calls change nothing as long as the
    /// content stays the same.
    pub fn with_content_hash(mut self) -> Self {
        let mut hasher = Sha256::new();
        let kind_tag: u8 = match self.kind {
            FrameKind::Dcps => 1,
            FrameKind::Rtps => 2,
            FrameKind::Transport => 3,
        };
        hasher.update([kind_tag]);
        hasher.update(self.topic.as_bytes());
        hasher.update(b"\0");
        hasher.update(self.timestamp_ns.to_be_bytes());
        hasher.update(self.correlation_id.to_be_bytes());
        hasher.update(&self.payload);
        let h: [u8; 32] = hasher.finalize().into();
        self.content_hash = Some(h);
        self
    }

    /// Verifies that the stored hash matches the one computed from the
    /// current frame content.
    #[must_use]
    pub fn verify_content_hash(&self) -> bool {
        let Some(stored) = self.content_hash else {
            return false;
        };
        let mut hasher = Sha256::new();
        let kind_tag: u8 = match self.kind {
            FrameKind::Dcps => 1,
            FrameKind::Rtps => 2,
            FrameKind::Transport => 3,
        };
        hasher.update([kind_tag]);
        hasher.update(self.topic.as_bytes());
        hasher.update(b"\0");
        hasher.update(self.timestamp_ns.to_be_bytes());
        hasher.update(self.correlation_id.to_be_bytes());
        hasher.update(&self.payload);
        let computed: [u8; 32] = hasher.finalize().into();
        stored == computed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dcps_frame_has_dcps_kind() {
        let f = Frame::dcps("speed".into(), 1000, 42, vec![1, 2, 3]);
        assert_eq!(f.kind, FrameKind::Dcps);
        assert_eq!(f.topic, "speed");
        assert_eq!(f.correlation_id, 42);
    }

    #[test]
    fn rtps_frame_has_rtps_kind() {
        let f = Frame::rtps("speed".into(), 0, 0, vec![]);
        assert_eq!(f.kind, FrameKind::Rtps);
    }

    #[test]
    fn transport_frame_has_no_topic() {
        let f = Frame::transport(0, 0, vec![0xFF]);
        assert_eq!(f.kind, FrameKind::Transport);
        assert!(f.topic.is_empty());
    }

    #[test]
    fn frame_clone_preserves_fields() {
        let f = Frame::dcps("topic".into(), 12345, 99, vec![0xDE, 0xAD]);
        let cloned = f.clone();
        assert_eq!(f, cloned);
    }

    #[test]
    fn frame_kinds_are_distinct() {
        let a = Frame::dcps(String::new(), 0, 0, vec![]);
        let b = Frame::rtps(String::new(), 0, 0, vec![]);
        let c = Frame::transport(0, 0, vec![]);
        assert_ne!(a.kind, b.kind);
        assert_ne!(b.kind, c.kind);
    }

    #[test]
    fn unhashed_frame_has_no_content_hash() {
        let f = Frame::dcps("t".into(), 0, 0, vec![]);
        assert!(f.content_hash.is_none());
    }

    #[test]
    fn with_content_hash_sets_hash() {
        let f = Frame::dcps("t".into(), 100, 1, vec![1, 2]).with_content_hash();
        assert!(f.content_hash.is_some());
    }

    #[test]
    fn content_hash_is_deterministic() {
        let f1 = Frame::dcps("t".into(), 100, 1, vec![1, 2]).with_content_hash();
        let f2 = Frame::dcps("t".into(), 100, 1, vec![1, 2]).with_content_hash();
        assert_eq!(f1.content_hash, f2.content_hash);
    }

    #[test]
    fn payload_change_changes_hash() {
        let f1 = Frame::dcps("t".into(), 100, 1, vec![1, 2]).with_content_hash();
        let f2 = Frame::dcps("t".into(), 100, 1, vec![1, 3]).with_content_hash();
        assert_ne!(f1.content_hash, f2.content_hash);
    }

    #[test]
    fn topic_change_changes_hash() {
        let f1 = Frame::dcps("a".into(), 100, 1, vec![1]).with_content_hash();
        let f2 = Frame::dcps("b".into(), 100, 1, vec![1]).with_content_hash();
        assert_ne!(f1.content_hash, f2.content_hash);
    }

    #[test]
    fn timestamp_change_changes_hash() {
        let f1 = Frame::dcps("t".into(), 100, 1, vec![1]).with_content_hash();
        let f2 = Frame::dcps("t".into(), 200, 1, vec![1]).with_content_hash();
        assert_ne!(f1.content_hash, f2.content_hash);
    }

    #[test]
    fn kind_change_changes_hash() {
        let f1 = Frame::dcps("t".into(), 100, 1, vec![1]).with_content_hash();
        let f2 = Frame::rtps("t".into(), 100, 1, vec![1]).with_content_hash();
        assert_ne!(f1.content_hash, f2.content_hash);
    }

    #[test]
    fn verify_content_hash_returns_false_when_unhashed() {
        let f = Frame::dcps("t".into(), 0, 0, vec![]);
        assert!(!f.verify_content_hash());
    }

    #[test]
    fn verify_content_hash_detects_post_hash_mutation() {
        let mut f = Frame::dcps("t".into(), 100, 1, vec![1, 2]).with_content_hash();
        assert!(f.verify_content_hash());
        f.payload.push(3);
        assert!(!f.verify_content_hash());
    }
}
