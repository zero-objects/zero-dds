// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The `ros2.*` settings of the RMW (`zerodds-ros2-bridge-1.0.md` §3, §5.3,
//! §6), read from the same layered runtime configuration the RMW's DDS
//! runtime reads: `zerodds.yaml` and its drop-ins, `include:`, the
//! environment beneath, `--set` in the diagnostics CLI.
//!
//! ```yaml
//! ros2:
//!   topic_mangling: rmw            # rmw | none | custom
//!   custom_mangling:              # custom only
//!     topic_prefix: "rt/"
//!     request_prefix: "rq/"        # service requests, default rq/
//!     response_prefix: "rr/"       # service replies, default rr/
//!   qos_profiles:                  # REP-2009 table, each entry replaceable
//!     sensor_data:
//!       reliability: best_effort   # reliable | best_effort | system_default
//!       durability: volatile       # volatile | transient_local | system_default
//!       history: keep_last         # keep_last | keep_all | system_default
//!       depth: 5
//!       deadline_ms: 0             # 0 = none
//!       lifespan_ms: 0
//!       liveliness: automatic      # automatic | manual_by_topic | system_default
//!       liveliness_lease_ms: 0
//!   topic_overrides:
//!     - ros_topic: /scan           # the ROS name, before mangling
//!       qos_profile: sensor_data   # a profile of the table, or:
//!     - ros_topic: /map
//!       qos:                       # the fields to replace
//!         durability: transient_local
//!         depth: 1
//! ```
//!
//! An override replaces the fields it names of what the application
//! requested; a profile names every field. Keys rcl owns are refused with
//! the reason: `ros2.namespace` and `ros2.ros_domain_id` (rcl resolves names
//! and passes the domain before the RMW is called; `--ros-args -r __ns:=`,
//! `ROS_DOMAIN_ID`) and `logging.*` (rcutils logging).
//!
//! A service `/add_two_ints` has the DDS topics `rq/add_two_intsRequest` and
//! `rr/add_two_intsReply`, as rmw_cyclonedds_cpp and rmw_fastrtps_cpp name
//! them (`make_fqtopic`, prefix `rq`/`rr`, the ROS name, suffix
//! `Request`/`Reply`).

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_config::ConfigError;
use zerodds_config::read::{Section, first_origin};

use crate::qos::RmwZerodsQos;

/// Every `ros2.*` key the RMW reads; map- and list-valued keys stop at the
/// map or list.
pub const KEYS: &[&str] = &[
    "ros2.topic_mangling",
    "ros2.custom_mangling.topic_prefix",
    "ros2.custom_mangling.request_prefix",
    "ros2.custom_mangling.response_prefix",
    "ros2.qos_profiles",
    "ros2.topic_overrides",
    "ros2.time_based_filter",
    "ros2.spatial_bridge",
];

/// The arguments of a ROS 2 process that are the application's: everything
/// outside the `--ros-args … --` blocks, which belong to rcl (`--ros-args`
/// opens one, a lone `--` or the end of the line closes it). The RMW takes
/// `--config` and `--set` from these, by the same contract as every other
/// ZeroDDS program: defaults and environment, the files (`--config`, else
/// the system directory), then `--set`.
#[must_use]
pub fn application_args<I: IntoIterator<Item = String>>(args: I) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_ros = false;
    for arg in args {
        if in_ros {
            if arg == "--" {
                in_ros = false;
            }
            continue;
        }
        if arg == "--ros-args" {
            in_ros = true;
            continue;
        }
        out.push(arg);
    }
    out
}

/// The layers the RMW starts from, for the process it is loaded into: the
/// environment, the files (`--config` from the command line, else the system
/// directory with its drop-ins), `--set` from the command line.
/// `--check-config` on a node's command line is not acted on; the RMW's
/// report is `zerodds-ros2-shim --check-config` with the same flags.
///
/// # Errors
/// A malformed `--config`/`--set`, a `--set` on a path nothing reads, or a
/// file that does not load.
pub fn process_source() -> Result<zerodds_dcps::runtime::ConfigSource, String> {
    let (command_line, _) =
        zerodds_config::CommandLine::extract(application_args(std::env::args().skip(1)))
            .map_err(|e| e.to_string())?;
    zerodds_dcps::runtime::ConfigSource::for_program(
        &command_line,
        "zerodds",
        &is_key,
        &[],
        |name| std::env::var(name).ok(),
    )
    .map_err(|e| e.to_string())
}

/// Whether `path` is a key [`Ros2Config::from_config`] reads.
#[must_use]
pub fn is_key(path: &str) -> bool {
    KEYS.contains(&path)
        || path.starts_with("ros2.qos_profiles.")
        || path.starts_with("ros2.spatial_bridge.")
}

/// Keys this configuration refuses, with the reason.
const OWNED_ELSEWHERE: &[(&str, &str)] = &[
    (
        "ros2.namespace",
        "rcl resolves names with the node namespace before the RMW sees them; \
         use `--ros-args -r __ns:=/ns` or ROS_NAMESPACE",
    ),
    (
        "ros2.ros_domain_id",
        "rcl passes the domain to rmw_init; use ROS_DOMAIN_ID",
    ),
    (
        "ros2.enclave",
        "the enclave is security.enclave_dir (env ZERODDS_SECURITY_DIR)",
    ),
    (
        "logging",
        "the RMW logs through rcutils; use --ros-args --log-level",
    ),
    (
        "discovery.multicast",
        "the setting is discovery.no_multicast",
    ),
    (
        "discovery.unicast_initial_peers",
        "the setting is discovery.peers",
    ),
];

/// How a ROS topic name becomes a DDS topic name (§5.1, §5.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mangling {
    /// `rt/` + the name without its leading `/` (REP-2007); services `rq/`
    /// and `rr/`.
    Rmw,
    /// The name without its leading `/`: coexistence with non-ROS DDS apps.
    None,
    /// A prefix + the name without its leading `/`.
    Custom {
        /// Topics.
        topic: String,
        /// Service requests.
        request: String,
        /// Service replies.
        response: String,
    },
}

/// The fields of a QoS an override replaces; `None` keeps the request's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct QosPatch {
    /// rmw history value.
    pub history: Option<u32>,
    /// KEEP_LAST depth.
    pub depth: Option<u64>,
    /// rmw reliability value.
    pub reliability: Option<u32>,
    /// rmw durability value.
    pub durability: Option<u32>,
    /// nanoseconds, `0` = none.
    pub deadline_ns: Option<u64>,
    /// nanoseconds, `0` = none.
    pub lifespan_ns: Option<u64>,
    /// rmw liveliness value.
    pub liveliness: Option<u32>,
    /// nanoseconds, `0` = none.
    pub liveliness_lease_ns: Option<u64>,
}

impl QosPatch {
    /// `requested` with this patch's fields in place of its own.
    #[must_use]
    pub fn apply(&self, requested: &RmwZerodsQos) -> RmwZerodsQos {
        RmwZerodsQos {
            history: self.history.unwrap_or(requested.history),
            depth: self.depth.unwrap_or(requested.depth),
            reliability: self.reliability.unwrap_or(requested.reliability),
            durability: self.durability.unwrap_or(requested.durability),
            deadline_ns: self.deadline_ns.unwrap_or(requested.deadline_ns),
            lifespan_ns: self.lifespan_ns.unwrap_or(requested.lifespan_ns),
            liveliness: self.liveliness.unwrap_or(requested.liveliness),
            liveliness_lease_ns: self
                .liveliness_lease_ns
                .unwrap_or(requested.liveliness_lease_ns),
            avoid_ros_namespace_conventions: requested.avoid_ros_namespace_conventions,
        }
    }

    fn complete(q: RmwZerodsQos) -> Self {
        Self {
            history: Some(q.history),
            depth: Some(q.depth),
            reliability: Some(q.reliability),
            durability: Some(q.durability),
            deadline_ns: Some(q.deadline_ns),
            lifespan_ns: Some(q.lifespan_ns),
            liveliness: Some(q.liveliness),
            liveliness_lease_ns: Some(q.liveliness_lease_ns),
        }
    }
}

/// The RMW's `ros2.*` settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ros2Config {
    /// Topic-name mangling.
    pub mangling: Mangling,
    /// Named QoS profiles: the REP-2009 table with the file's replacements
    /// and additions.
    pub profiles: BTreeMap<String, QosPatch>,
    /// Per ROS topic, the fields that replace the application's request.
    pub overrides: Vec<TopicOverride>,
}

/// One `ros2.topic_overrides` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicOverride {
    /// The ROS topic name, before mangling.
    pub ros_topic: String,
    /// The `qos_profile` it names; `None` for an inline `qos` map.
    pub profile: Option<String>,
    /// The fields that replace the application's request.
    pub qos: QosPatch,
}

fn qos(history: u32, depth: u64, reliability: u32, durability: u32) -> RmwZerodsQos {
    RmwZerodsQos {
        history,
        depth,
        reliability,
        durability,
        liveliness: 1,
        ..RmwZerodsQos::default()
    }
}

/// The REP-2009 table of §6.
fn standard_profiles() -> BTreeMap<String, QosPatch> {
    let mut t = BTreeMap::new();
    for (name, q) in [
        ("sensor_data", qos(1, 5, 2, 2)),
        ("services", qos(1, 10, 1, 2)),
        ("parameters", qos(1, 1000, 1, 2)),
        ("parameter_events", qos(2, 0, 1, 2)),
        ("default", qos(1, 10, 1, 2)),
    ] {
        t.insert(name.to_string(), QosPatch::complete(q));
    }
    t
}

impl Default for Ros2Config {
    fn default() -> Self {
        Self {
            mangling: Mangling::Rmw,
            profiles: standard_profiles(),
            overrides: Vec::new(),
        }
    }
}

fn ms(section: &Section<'_>, key: &str) -> Result<Option<u64>, ConfigError> {
    Ok(section
        .parse::<u64>(key, "milliseconds, 0 for none")?
        .map(|v| v.saturating_mul(1_000_000)))
}

/// The fields a `qos_profiles` entry or an override's `qos` map names.
fn read_patch(section: &Section<'_>) -> Result<QosPatch, ConfigError> {
    section.only(&[
        "reliability",
        "durability",
        "history",
        "depth",
        "deadline_ms",
        "lifespan_ms",
        "liveliness",
        "liveliness_lease_ms",
    ])?;
    let pick = |key: &str, choices: &[(&str, u32)]| -> Result<Option<u32>, ConfigError> {
        let names: Vec<&str> = choices.iter().map(|(n, _)| *n).collect();
        Ok(section
            .choice(key, &names)?
            .and_then(|v| choices.iter().find(|(n, _)| *n == v).map(|(_, x)| *x)))
    };
    Ok(QosPatch {
        reliability: pick(
            "reliability",
            &[("system_default", 0), ("reliable", 1), ("best_effort", 2)],
        )?,
        durability: pick(
            "durability",
            &[
                ("system_default", 0),
                ("transient_local", 1),
                ("volatile", 2),
            ],
        )?,
        history: pick(
            "history",
            &[("system_default", 0), ("keep_last", 1), ("keep_all", 2)],
        )?,
        depth: section.parse::<u64>("depth", "a KEEP_LAST depth")?,
        deadline_ns: ms(section, "deadline_ms")?,
        lifespan_ns: ms(section, "lifespan_ms")?,
        liveliness: pick(
            "liveliness",
            &[
                ("system_default", 0),
                ("automatic", 1),
                ("manual_by_topic", 3),
            ],
        )?,
        liveliness_lease_ns: ms(section, "liveliness_lease_ms")?,
    })
}

impl Ros2Config {
    /// Reads `ros2.*` out of a merged runtime tree, over [`Self::default`].
    ///
    /// # Errors
    /// [`ConfigError::Invalid`] naming the setting, its origin and what it
    /// accepts, or why it is refused.
    pub fn from_config(config: Option<&zerodds_config::Config>) -> Result<Self, ConfigError> {
        let tree = config.and_then(zerodds_config::Config::root);
        for (key, why) in OWNED_ELSEWHERE {
            if let Some(node) = tree.and_then(|t| t.get(key)) {
                return Err(ConfigError::Invalid {
                    setting: (*key).to_string(),
                    origin: first_origin(node),
                    message: format!("not an RMW setting: {why}"),
                });
            }
        }
        let root = Section::root(tree)?;
        let ros2 = root.section("ros2")?;
        ros2.only(&[
            "topic_mangling",
            "custom_mangling",
            "qos_profiles",
            "topic_overrides",
            "time_based_filter",
            "spatial_bridge",
        ])?;
        let mut out = Self::default();
        let custom = ros2.section("custom_mangling")?;
        custom.only(&["topic_prefix", "request_prefix", "response_prefix"])?;
        let prefix = custom.string("topic_prefix")?;
        let request_prefix = custom.string("request_prefix")?;
        let response_prefix = custom.string("response_prefix")?;
        out.mangling = match ros2
            .choice("topic_mangling", &["rmw", "none", "custom"])?
            .as_deref()
        {
            Some("none") => Mangling::None,
            Some("custom") => Mangling::Custom {
                topic: prefix.clone().ok_or_else(|| ConfigError::Invalid {
                    setting: "ros2.custom_mangling.topic_prefix".to_string(),
                    origin: ros2
                        .node("topic_mangling")
                        .map_or(zerodds_config::Origin::Default, first_origin),
                    message: "topic_mangling custom needs a topic_prefix".to_string(),
                })?,
                request: request_prefix.clone().unwrap_or_else(|| "rq/".to_string()),
                response: response_prefix.clone().unwrap_or_else(|| "rr/".to_string()),
            },
            _ => Mangling::Rmw,
        };
        if !matches!(out.mangling, Mangling::Custom { .. }) {
            for (key, value) in [
                ("topic_prefix", &prefix),
                ("request_prefix", &request_prefix),
                ("response_prefix", &response_prefix),
            ] {
                if value.is_some() {
                    return Err(ConfigError::Invalid {
                        setting: format!("ros2.custom_mangling.{key}"),
                        origin: custom
                            .node(key)
                            .map_or(zerodds_config::Origin::Default, first_origin),
                        message: "applies only with topic_mangling: custom".to_string(),
                    });
                }
            }
        }
        let profiles = ros2.section("qos_profiles")?;
        if let Some(zerodds_config::Node::Map(map)) = ros2.node("qos_profiles") {
            for name in map.keys() {
                let entry = profiles.section(name)?;
                let patch = read_patch(&entry)?;
                // A named profile replaces the standard one field by field.
                let base = out.profiles.get(name).copied().unwrap_or_default();
                out.profiles.insert(
                    name.clone(),
                    QosPatch {
                        history: patch.history.or(base.history),
                        depth: patch.depth.or(base.depth),
                        reliability: patch.reliability.or(base.reliability),
                        durability: patch.durability.or(base.durability),
                        deadline_ns: patch.deadline_ns.or(base.deadline_ns),
                        lifespan_ns: patch.lifespan_ns.or(base.lifespan_ns),
                        liveliness: patch.liveliness.or(base.liveliness),
                        liveliness_lease_ns: patch.liveliness_lease_ns.or(base.liveliness_lease_ns),
                    },
                );
            }
        }
        for (path, item) in ros2.items("topic_overrides")? {
            let entry = Section::of(Some(item), path.clone())?;
            entry.only(&["ros_topic", "qos_profile", "qos"])?;
            let origin = first_origin(item);
            let ros_topic = entry
                .string("ros_topic")?
                .ok_or_else(|| ConfigError::Invalid {
                    setting: format!("{path}.ros_topic"),
                    origin: origin.clone(),
                    message: "an override needs a ros_topic".to_string(),
                })?;
            let profile = entry.string("qos_profile")?;
            let qos = match (&profile, entry.node("qos")) {
                (Some(name), None) => {
                    *out.profiles.get(name).ok_or_else(|| ConfigError::Invalid {
                        setting: format!("{path}.qos_profile"),
                        origin: origin.clone(),
                        message: format!(
                            "no profile `{name}`; known: {}",
                            out.profiles.keys().cloned().collect::<Vec<_>>().join(", ")
                        ),
                    })?
                }
                (None, Some(_)) => read_patch(&entry.section("qos")?)?,
                _ => {
                    return Err(ConfigError::Invalid {
                        setting: path,
                        origin,
                        message: "an override names either qos_profile or qos".to_string(),
                    });
                }
            };
            out.overrides.push(TopicOverride {
                ros_topic,
                profile,
                qos,
            });
        }
        Ok(out)
    }

    /// The QoS an endpoint on `ros_topic` is created with: the application's
    /// request with the topic's override applied.
    #[must_use]
    pub fn effective_qos(&self, ros_topic: &str, requested: &RmwZerodsQos) -> RmwZerodsQos {
        self.overrides
            .iter()
            .find(|o| o.ros_topic == ros_topic)
            .map_or(*requested, |o| o.qos.apply(requested))
    }

    /// The DDS topic of a ROS topic name under this mangling.
    /// `avoid_ros_namespace_conventions` uses the ROS name as it is, leading
    /// `/` included, as rmw_cyclonedds_cpp and rmw_fastrtps_cpp do (they
    /// leave out only the `rt` prefix), so such endpoints match across RMWs.
    #[must_use]
    pub fn dds_topic(&self, ros_topic: &str, avoid_ros_namespace_conventions: bool) -> String {
        if avoid_ros_namespace_conventions {
            return ros_topic.to_string();
        }
        let bare = ros_topic.strip_prefix('/').unwrap_or(ros_topic);
        match &self.mangling {
            Mangling::Rmw => zerodds_ros2_rmw::topic_mangling::mangle_topic_name(
                ros_topic,
                zerodds_ros2_rmw::topic_mangling::RosKind::Topic,
            )
            .unwrap_or_else(|_| format!("rt/{bare}")),
            Mangling::None => bare.to_string(),
            Mangling::Custom { topic, .. } => format!("{topic}{bare}"),
        }
    }

    /// The DDS request and reply topics of a ROS service name, as
    /// rmw_cyclonedds_cpp and rmw_fastrtps_cpp form them: `rq` + the name +
    /// `Request`, `rr` + the name + `Reply` (`/add_two_ints` →
    /// `rq/add_two_intsRequest`). `avoid_ros_namespace_conventions` leaves
    /// the prefixes out and keeps the name as it is.
    #[must_use]
    pub fn dds_service_topics(
        &self,
        ros_service: &str,
        avoid_ros_namespace_conventions: bool,
    ) -> (String, String) {
        if avoid_ros_namespace_conventions {
            return (
                format!("{ros_service}Request"),
                format!("{ros_service}Reply"),
            );
        }
        let bare = ros_service.strip_prefix('/').unwrap_or(ros_service);
        let (rq, rr) = match &self.mangling {
            Mangling::Rmw => ("rq/", "rr/"),
            Mangling::None => ("", ""),
            Mangling::Custom {
                request, response, ..
            } => (request.as_str(), response.as_str()),
        };
        (format!("{rq}{bare}Request"), format!("{rr}{bare}Reply"))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn load(yaml: &str) -> Result<Ros2Config, ConfigError> {
        let mut c = zerodds_config::Config::default();
        c.push(
            zerodds_config::parse_str(yaml, "<string>").unwrap(),
            "<string>".into(),
        )
        .unwrap();
        Ros2Config::from_config(Some(&c))
    }

    #[test]
    fn the_defaults_are_rmw_mangling_and_the_rep_2009_table() {
        let c = Ros2Config::from_config(None).unwrap();
        assert_eq!(c.mangling, Mangling::Rmw);
        assert_eq!(c.dds_topic("/chatter", false), "rt/chatter");
        assert_eq!(c.profiles["sensor_data"].depth, Some(5));
        assert_eq!(c.profiles["sensor_data"].reliability, Some(2));
        assert_eq!(c.profiles["parameter_events"].history, Some(2));
    }

    #[test]
    fn mangling_none_and_custom_name_the_dds_topic() {
        let c = load("ros2:\n  topic_mangling: none\n").unwrap();
        assert_eq!(c.dds_topic("/chatter", false), "chatter");
        let c =
            load("ros2:\n  topic_mangling: custom\n  custom_mangling:\n    topic_prefix: \"x/\"\n")
                .unwrap();
        assert_eq!(c.dds_topic("/chatter", false), "x/chatter");
        assert_eq!(
            c.dds_topic("/chatter", true),
            "/chatter",
            "avoid_ros_namespace_conventions: the ROS name as it is"
        );
    }

    /// The service topics rmw_cyclonedds_cpp forms (`make_fqtopic(prefix
    /// "rq"/"rr", name, "Request"/"Reply")`, rmw_cyclonedds 1.3.4
    /// `rmw_node.cpp`); `avoid_ros_namespace_conventions` drops the prefix.
    #[test]
    fn service_topics_are_the_rq_rr_names_of_the_other_rmws() {
        let c = Ros2Config::from_config(None).unwrap();
        let pair = |a: &str, b: &str| (a.to_string(), b.to_string());
        assert_eq!(
            c.dds_service_topics("/add_two_ints", false),
            pair("rq/add_two_intsRequest", "rr/add_two_intsReply")
        );
        assert_eq!(
            c.dds_service_topics("/ns/talker/get_parameters", false),
            pair(
                "rq/ns/talker/get_parametersRequest",
                "rr/ns/talker/get_parametersReply"
            )
        );
        assert_eq!(
            c.dds_service_topics("/add_two_ints", true),
            pair("/add_two_intsRequest", "/add_two_intsReply")
        );
        let none = load("ros2:\n  topic_mangling: none\n").unwrap();
        assert_eq!(
            none.dds_service_topics("/s", false),
            pair("sRequest", "sReply")
        );
        let custom = load(
            "ros2:\n  topic_mangling: custom\n  custom_mangling:\n    topic_prefix: t/\n    request_prefix: q/\n",
        )
        .unwrap();
        assert_eq!(
            custom.dds_service_topics("/s", false),
            pair("q/sRequest", "rr/sReply"),
            "an unset prefix keeps its default"
        );
    }

    #[test]
    fn an_override_replaces_only_what_it_names() {
        let c = load(
            "ros2:\n  topic_overrides:\n    - ros_topic: /map\n      qos:\n        durability: transient_local\n        depth: 1\n    - ros_topic: /scan\n      qos_profile: sensor_data\n",
        )
        .unwrap();
        let req = RmwZerodsQos {
            history: 1,
            depth: 10,
            reliability: 1,
            durability: 2,
            ..RmwZerodsQos::default()
        };
        let map = c.effective_qos("/map", &req);
        assert_eq!((map.durability, map.depth, map.reliability), (1, 1, 1));
        let scan = c.effective_qos("/scan", &req);
        assert_eq!((scan.reliability, scan.depth), (2, 5));
        assert_eq!(c.effective_qos("/other", &req), req);
    }

    #[test]
    fn a_profile_can_be_replaced_and_added() {
        let c = load(
            "ros2:\n  qos_profiles:\n    sensor_data:\n      depth: 50\n    fast:\n      reliability: best_effort\n      history: keep_last\n      depth: 1\n  topic_overrides:\n    - ros_topic: /imu\n      qos_profile: fast\n",
        )
        .unwrap();
        assert_eq!(c.profiles["sensor_data"].depth, Some(50));
        assert_eq!(
            c.profiles["sensor_data"].reliability,
            Some(2),
            "the rest stays"
        );
        assert_eq!(c.overrides[0].qos.depth, Some(1));
    }

    /// The shipped drop-in, through the same reader.
    #[test]
    fn the_shipped_example_reads_as_it_says() {
        let c = load(include_str!(
            "../../../packaging/linux/configs/ros2.yaml.example"
        ))
        .unwrap();
        assert_eq!(c.mangling, Mangling::Rmw);
        assert_eq!(c.overrides.len(), 2);
        assert_eq!(c.overrides[0].profile.as_deref(), Some("sensor_data"));
        assert_eq!(c.overrides[1].qos.durability, Some(1));
        assert_eq!(c.overrides[1].qos.depth, Some(1));
        assert_eq!(c.overrides[1].qos.reliability, None);
    }

    #[test]
    fn keys_rcl_owns_and_malformed_entries_are_refused() {
        for (yaml, needle) in [
            ("ros2:\n  namespace: /x\n", "ros2.namespace"),
            ("ros2:\n  ros_domain_id: 3\n", "ROS_DOMAIN_ID"),
            ("logging:\n  level: info\n", "rcutils"),
            (
                "ros2:\n  custom_mangling:\n    request_prefix: rq/\n",
                "only with topic_mangling",
            ),
            ("ros2:\n  topic_mangling: custom\n", "needs a topic_prefix"),
            (
                "ros2:\n  custom_mangling:\n    topic_prefix: x/\n",
                "only with topic_mangling",
            ),
            (
                "ros2:\n  topic_overrides:\n    - ros_topic: /a\n      qos_profile: nope\n",
                "no profile",
            ),
            (
                "ros2:\n  topic_overrides:\n    - ros_topic: /a\n",
                "either qos_profile or qos",
            ),
            (
                "ros2:\n  qos_profiles:\n    x:\n      reliabilty: reliable\n",
                "reliabilty",
            ),
            ("ros2:\n  topic_manglin: none\n", "topic_manglin"),
        ] {
            let err = load(yaml).expect_err(yaml).to_string();
            assert!(err.contains(needle), "{yaml}: {err}");
        }
    }

    #[test]
    fn the_ros_args_blocks_are_rcls_and_the_rest_is_the_applications() {
        let args = |v: &[&str]| v.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        assert_eq!(
            application_args(args(&[
                "--config",
                "/etc/a.yaml",
                "--ros-args",
                "-r",
                "__node:=talker",
                "--",
                "--set",
                "domain=3",
                "--ros-args",
                "--log-level",
                "debug",
            ])),
            args(&["--config", "/etc/a.yaml", "--set", "domain=3"])
        );
        assert_eq!(
            application_args(args(&["--ros-args", "--config", "x"])),
            Vec::<String>::new()
        );
    }
}
