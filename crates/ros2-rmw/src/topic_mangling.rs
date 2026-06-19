// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! ROS 2 → DDS Topic-Name-Mangling.
//!
//! Convention from `rmw_dds_common`: ROS-2 logical names are mapped to
//! DDS topic names with a prefix code:
//! * `rt/<name>` — ROS topic.
//! * `rq/<name>Request` — service request.
//! * `rr/<name>Reply` — service reply.
//! * `rs/<name>` — service-server discovery (legacy).
//!
//! Example: `/chatter` → `rt/chatter`. The leading slash is removed;
//! internal `/` are kept 1:1.

use alloc::string::String;
use core::fmt;

/// ROS 2 Endpoint-Kategorie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RosKind {
    /// `rt/` Prefix — pub/sub Topic.
    Topic,
    /// `rq/` Prefix — Service-Request.
    ServiceRequest,
    /// `rr/` Prefix — Service-Reply.
    ServiceReply,
    /// `rs/` Prefix — Service-Discovery (legacy).
    ServiceLegacy,
}

impl RosKind {
    /// Wire-Prefix.
    #[must_use]
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Topic => "rt/",
            Self::ServiceRequest => "rq/",
            Self::ServiceReply => "rr/",
            Self::ServiceLegacy => "rs/",
        }
    }
}

/// Mangling error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameMangleError {
    /// ROS name is empty.
    EmptyName,
    /// ROS name starts with an invalid character.
    InvalidLeadingChar(char),
    /// DDS name matches no known prefix during demangling.
    UnknownPrefix(String),
}

impl fmt::Display for NameMangleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => f.write_str("empty ROS name"),
            Self::InvalidLeadingChar(c) => write!(f, "invalid leading character `{c}`"),
            Self::UnknownPrefix(s) => write!(f, "unknown DDS topic prefix `{s}`"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for NameMangleError {}

/// Maps a ROS-2 topic name to a DDS topic name.
///
/// The leading `/` is removed (ROS topics are absolute), internal `/`
/// are kept. Suffixes (`Request`/`Reply`) are the caller's
/// responsibility — this function only adds the prefix.
///
/// # Errors
/// * `EmptyName` if `ros_name` is empty.
/// * `InvalidLeadingChar` if `ros_name` starts with neither `/` nor an
///   alphanumeric character.
pub fn mangle_topic_name(ros_name: &str, kind: RosKind) -> Result<String, NameMangleError> {
    if ros_name.is_empty() {
        return Err(NameMangleError::EmptyName);
    }
    let stripped = ros_name.strip_prefix('/').unwrap_or(ros_name);
    if stripped.is_empty() {
        return Err(NameMangleError::EmptyName);
    }
    let first = stripped.chars().next().ok_or(NameMangleError::EmptyName)?;
    if !first.is_alphabetic() && first != '_' {
        return Err(NameMangleError::InvalidLeadingChar(first));
    }
    let mut out = String::with_capacity(kind.prefix().len() + stripped.len());
    out.push_str(kind.prefix());
    out.push_str(stripped);
    Ok(out)
}

/// Maps a DDS topic name back to `(kind, ros_name_with_slash)`.
///
/// # Errors
/// `UnknownPrefix` if none of the four prefixes match.
pub fn demangle_topic_name(dds_name: &str) -> Result<(RosKind, String), NameMangleError> {
    for kind in [
        RosKind::Topic,
        RosKind::ServiceRequest,
        RosKind::ServiceReply,
        RosKind::ServiceLegacy,
    ] {
        if let Some(rest) = dds_name.strip_prefix(kind.prefix()) {
            let mut out = String::with_capacity(rest.len() + 1);
            out.push('/');
            out.push_str(rest);
            return Ok((kind, out));
        }
    }
    Err(NameMangleError::UnknownPrefix(String::from(dds_name)))
}

/// `true` if the DDS topic name carries a ROS-2 prefix.
#[must_use]
pub fn is_ros_topic(dds_name: &str) -> bool {
    dds_name.starts_with("rt/")
        || dds_name.starts_with("rq/")
        || dds_name.starts_with("rr/")
        || dds_name.starts_with("rs/")
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn mangle_topic_strips_leading_slash_and_prepends_rt() {
        // `/chatter` → `rt/chatter`.
        assert_eq!(
            mangle_topic_name("/chatter", RosKind::Topic).expect("ok"),
            "rt/chatter"
        );
    }

    #[test]
    fn mangle_preserves_internal_slashes() {
        assert_eq!(
            mangle_topic_name("/sensors/imu/data", RosKind::Topic).expect("ok"),
            "rt/sensors/imu/data"
        );
    }

    #[test]
    fn mangle_each_kind_uses_correct_prefix() {
        assert!(
            mangle_topic_name("/foo", RosKind::Topic)
                .expect("ok")
                .starts_with("rt/")
        );
        assert!(
            mangle_topic_name("/foo", RosKind::ServiceRequest)
                .expect("ok")
                .starts_with("rq/")
        );
        assert!(
            mangle_topic_name("/foo", RosKind::ServiceReply)
                .expect("ok")
                .starts_with("rr/")
        );
        assert!(
            mangle_topic_name("/foo", RosKind::ServiceLegacy)
                .expect("ok")
                .starts_with("rs/")
        );
    }

    #[test]
    fn mangle_handles_already_unprefixed_name() {
        // `chatter` (no leading slash) → `rt/chatter`.
        assert_eq!(
            mangle_topic_name("chatter", RosKind::Topic).expect("ok"),
            "rt/chatter"
        );
    }

    #[test]
    fn mangle_rejects_empty_name() {
        assert_eq!(
            mangle_topic_name("", RosKind::Topic),
            Err(NameMangleError::EmptyName)
        );
        assert_eq!(
            mangle_topic_name("/", RosKind::Topic),
            Err(NameMangleError::EmptyName)
        );
    }

    #[test]
    fn mangle_rejects_invalid_leading_character() {
        // ROS 2 topic names must not start with a digit.
        assert_eq!(
            mangle_topic_name("/1foo", RosKind::Topic),
            Err(NameMangleError::InvalidLeadingChar('1'))
        );
        assert_eq!(
            mangle_topic_name("/-foo", RosKind::Topic),
            Err(NameMangleError::InvalidLeadingChar('-'))
        );
    }

    #[test]
    fn mangle_accepts_underscore_leading() {
        assert_eq!(
            mangle_topic_name("/_private", RosKind::Topic).expect("ok"),
            "rt/_private"
        );
    }

    #[test]
    fn demangle_round_trips_all_kinds() {
        for (kind, prefix) in [
            (RosKind::Topic, "rt/"),
            (RosKind::ServiceRequest, "rq/"),
            (RosKind::ServiceReply, "rr/"),
            (RosKind::ServiceLegacy, "rs/"),
        ] {
            let dds = alloc::format!("{prefix}sensors/imu");
            let (k, ros) = demangle_topic_name(&dds).expect("ok");
            assert_eq!(k, kind);
            assert_eq!(ros, "/sensors/imu");
        }
    }

    #[test]
    fn demangle_rejects_unknown_prefix() {
        assert!(matches!(
            demangle_topic_name("xyz/foo"),
            Err(NameMangleError::UnknownPrefix(_))
        ));
    }

    #[test]
    fn is_ros_topic_recognizes_all_four_prefixes() {
        assert!(is_ros_topic("rt/chatter"));
        assert!(is_ros_topic("rq/foo"));
        assert!(is_ros_topic("rr/foo"));
        assert!(is_ros_topic("rs/foo"));
        assert!(!is_ros_topic("foo/bar"));
        assert!(!is_ros_topic("dds/foo"));
    }

    #[test]
    fn mangle_demangle_round_trip() {
        let original = "/sensors/imu/data";
        let dds = mangle_topic_name(original, RosKind::Topic).expect("mangle");
        let (kind, ros) = demangle_topic_name(&dds).expect("demangle");
        assert_eq!(kind, RosKind::Topic);
        assert_eq!(ros, original);
    }
}
