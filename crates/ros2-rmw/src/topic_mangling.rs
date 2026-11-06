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

/// ROS 2 Endpoint category.
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
    //! The oracle: the design article "Topic and Service name mapping to
    //! DDS" (design.ros2.org/articles/topic_and_service_names.html),
    //! §"ROS Specific Namespace Prefix" and §"ROS 2 Topic and Service Name
    //! Constraints", as `rmw_dds_common` and the other RMWs apply it.
    use super::*;

    #[test]
    fn ros_names_prefix_a_topic_loses_its_leading_slash_and_takes_rt() {
        for (ros, dds) in [
            ("/chatter", "rt/chatter"),
            ("/sensors/imu/data", "rt/sensors/imu/data"),
            ("chatter", "rt/chatter"),
            ("/_private", "rt/_private"),
        ] {
            assert_eq!(
                mangle_topic_name(ros, RosKind::Topic).expect("ok"),
                dds,
                "{ros}"
            );
        }
    }

    #[test]
    fn ros_names_prefix_each_kind_takes_its_prefix() {
        for (kind, dds) in [
            (RosKind::Topic, "rt/foo"),
            (RosKind::ServiceRequest, "rq/foo"),
            (RosKind::ServiceReply, "rr/foo"),
            (RosKind::ServiceLegacy, "rs/foo"),
        ] {
            assert_eq!(mangle_topic_name("/foo", kind).expect("ok"), dds);
        }
    }

    #[test]
    fn ros_names_constraints_an_empty_name_is_refused() {
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
    fn ros_names_constraints_a_name_starting_with_a_digit_or_a_hyphen_is_refused() {
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
    fn ros_names_prefix_the_four_prefixes_read_back_as_their_kind() {
        for (dds, kind) in [
            ("rt/sensors/imu", RosKind::Topic),
            ("rq/sensors/imu", RosKind::ServiceRequest),
            ("rr/sensors/imu", RosKind::ServiceReply),
            ("rs/sensors/imu", RosKind::ServiceLegacy),
        ] {
            let (k, ros) = demangle_topic_name(dds).expect("ok");
            assert_eq!(k, kind, "{dds}");
            assert_eq!(ros, "/sensors/imu", "{dds}");
            assert!(is_ros_topic(dds), "{dds}");
        }
    }

    #[test]
    fn ros_names_prefix_a_name_without_a_ros_prefix_is_no_ros_name() {
        assert!(matches!(
            demangle_topic_name("xyz/foo"),
            Err(NameMangleError::UnknownPrefix(_))
        ));
        assert!(!is_ros_topic("foo/bar"));
        assert!(!is_ros_topic("dds/foo"));
    }
}
