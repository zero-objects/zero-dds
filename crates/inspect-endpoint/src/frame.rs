//! Side-Channel-Frame-Format.
//!
//! Versioned, length-prefixed Frames die zwischen Inspect-Endpoint
//! und externem Reality Inspector ausgetauscht werden. Layer-agnostisch:
//! ein Frame transportiert ein DCPS-/RTPS-/Transport-Sample plus
//! Metadata.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Welcher Tap-Layer hat das Sample emittiert.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FrameKind {
    /// DCPS-Layer Sample (typed nach Deserialization).
    Dcps,
    /// RTPS-Layer Sample (Reader/Writer-Cache, sieht alle Transports).
    Rtps,
    /// Transport-Layer raw Bytes (sieht auch Shared-Memory).
    Transport,
}

/// Ein Side-Channel-Frame.
///
/// Frames sind Audit-faehig (Zero-Principle Pillar 6 Zero-Context-Loss):
/// `content_hash` ist SHA-256 ueber `(kind, topic, timestamp_ns,
/// correlation_id, payload)`. Phase-1: hash-Feld ist optional und
/// default `None`; Phase-2 wird der Hash beim Server-side dispatch
/// gesetzt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    /// Welcher Layer hat den Frame emittiert.
    pub kind: FrameKind,
    /// Topic-Name (oder leer wenn Transport-Layer ohne TypeObject).
    pub topic: String,
    /// Wall-Clock-Timestamp in Nanosekunden seit UNIX-Epoche.
    pub timestamp_ns: u64,
    /// Sequence-Number / Sample-Identity-GUID-Praefix fuer Cross-
    /// Layer-Korrelation (R-077).
    pub correlation_id: u64,
    /// Sample-Payload als Bytes (Layer-spezifisches Encoding).
    pub payload: Vec<u8>,
    /// Optionaler Content-Hash (Zero-Principle Foundation 2). Wird
    /// vom Server gesetzt wenn Audit-Streams aktiv sind. Phase-1
    /// default `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<[u8; 32]>,
}

impl Frame {
    /// Erzeugt einen DCPS-Frame.
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

    /// Erzeugt einen RTPS-Frame.
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

    /// Erzeugt einen Transport-Frame.
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

    /// Berechnet den Content-Hash und speichert ihn.
    ///
    /// Hash-Eingabe: kind-Tag + topic + timestamp\_ns + correlation\_id +
    /// payload. Idempotent — wiederholtes Aufrufen aendert nichts solange
    /// Content gleich bleibt.
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

    /// Verifiziert dass der gespeicherte Hash mit dem aus dem aktuellen
    /// Frame-Inhalt berechneten uebereinstimmt.
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
