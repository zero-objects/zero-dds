// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Config for `zerodds-durability-svc`. Spec: ADR 0009 +
//! `zerodds-durability-svc(1)`/`(5)`.
//!
//! Read by the shared layered loader (`zerodds-config`): base file,
//! `durability-svc.d/` drop-ins, includes, `${VAR}`, and the precedence
//! defaults < environment < files < `--set`. The DDS runtime's keys may sit
//! in the same file. [`DaemonConfig::from_config`] reads this daemon's keys
//! and refuses an unknown key inside a known section, and a value of the
//! wrong shape or type, with its file and line.

use std::path::Path;
use std::string::{String, ToString};
use std::vec::Vec;

pub use zerodds_config::ConfigError;
use zerodds_config::read::{Section, first_origin};
use zerodds_durability_store::Contract;
use zerodds_qos::policies::history::HistoryKind;

/// The base name of the daemon's file: `durability-svc.yaml`,
/// `durability-svc.d/`.
pub const CONFIG_STEM: &str = "durability-svc";

/// Every key this daemon reads; list-valued keys stop at the list.
pub const KEYS: &[&str] = &["domain", "store", "path", "auto", "durability.topics"];

/// Whether `path` is one of [`KEYS`].
#[must_use]
pub fn is_key(path: &str) -> bool {
    KEYS.contains(&path)
}

fn invalid(setting: String, origin: zerodds_config::Origin, message: String) -> ConfigError {
    ConfigError::Invalid {
        setting,
        origin,
        message,
    }
}

/// One `durability.topics[]` entry — a bare topic-name string, or a map for
/// an explicit override. `zerodds-durability-svc(5)`, "Which type/keyed/
/// contract wins", spells out the precedence over the discovered writer.
#[derive(Debug, Clone, PartialEq)]
pub struct TopicConfig {
    /// Topic name — required.
    pub name: String,
    /// Explicit application type name (`type:`). `None` resolves the type
    /// from the discovered writer, like `--auto` does
    /// (`DurabilityService::serve_discovered`).
    pub type_name: Option<String>,
    /// Explicit keyedness (`keyed:`), only meaningful together with
    /// `type_name`. `None` with a `type_name` given defaults to `false`
    /// (unkeyed) — set it explicitly for a keyed type; there is no discovery
    /// to fall back on once `type` names the type outright.
    pub keyed: Option<bool>,
    /// Explicit retention contract (`contract:`). `None` = the discovered
    /// writer's `DurabilityServiceQosPolicy`, or the daemon's own default
    /// when the writer's is indistinguishable from unset — see the
    /// `zerodds-durability-service` crate docs, "The retention contract".
    pub contract: Option<Contract>,
}

/// Daemon config.
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// `domain:` — required from some source (file, env, or `--domain`).
    pub domain: Option<i32>,
    /// `store:` — `sqlite` (default) | `file` | `lakehouse` | `postgres`.
    pub store: String,
    /// `path:` — store location; meaning depends on `store` (see
    /// `zerodds-durability-svc --help`).
    pub path: Option<String>,
    /// `auto:` — auto-serve TRANSIENT/PERSISTENT topics (no app-side code).
    pub auto: bool,
    /// `durability.topics:` — explicitly served topics, each with its own
    /// optional overrides.
    pub topics: Vec<TopicConfig>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self::default_for_dev()
    }
}

impl DaemonConfig {
    /// Compiled-in defaults. `domain: None` and no topics/`auto` — a real
    /// daemon always needs at least a domain and (auto or a topic) from
    /// some source; [`Self::from_config`] refuses their absence.
    #[must_use]
    pub fn default_for_dev() -> Self {
        Self {
            domain: None,
            store: "sqlite".to_string(),
            path: None,
            auto: false,
            topics: Vec::new(),
        }
    }

    /// Loads one file (and its sibling `<stem>.d/`), for tests and tools.
    /// The daemon itself loads through `zerodds_dcps::cli::Configured` (this
    /// bin crate has no `[lib]` target, so nothing outside its own
    /// `#[cfg(test)]` module can call this — hence the `cfg_attr` below).
    ///
    /// # Errors
    /// Unreadable file, text outside the subset, or a refused value.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn load_from_file(path: &Path) -> Result<Self, ConfigError> {
        let config = zerodds_config::Loader::new()
            .base(path)
            .require_base()
            .drop_in_dir(path.with_extension("d"))
            .load()?;
        Self::from_config(Some(&config))
    }

    /// Parses from a string, for tests.
    ///
    /// # Errors
    /// Text outside the subset, or a refused value.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn load_from_str(raw: &str) -> Result<Self, ConfigError> {
        let mut config = zerodds_config::Config::default();
        config.push(
            zerodds_config::parse_str(raw, "<string>")?,
            "<string>".to_string(),
        )?;
        Self::from_config(Some(&config))
    }

    /// Reads this daemon's keys out of a merged tree, over
    /// [`Self::default_for_dev`]. `None` = nothing was loaded.
    ///
    /// # Errors
    /// [`ConfigError::Invalid`] naming the setting, its origin and what it
    /// accepts — including "neither `auto` nor a topic is set" once every
    /// source (file + `--set`) is merged.
    pub fn from_config(config: Option<&zerodds_config::Config>) -> Result<Self, ConfigError> {
        let root = Section::root(config.and_then(zerodds_config::Config::root))?;
        let mut out = Self::default_for_dev();
        if let Some(v) = root.parse::<i32>("domain", "a DDS domain id")? {
            out.domain = Some(v);
        }
        if let Some(v) = root.choice("store", &["sqlite", "file", "lakehouse", "postgres"])? {
            out.store = v;
        }
        out.path = root.string("path")?;
        if let Some(v) = root.boolean("auto")? {
            out.auto = v;
        }

        let durability = root.section("durability")?;
        durability.only(&["topics"])?;
        for (path, item) in durability.items("topics")? {
            out.topics.push(read_topic(&path, item)?);
        }

        if out.domain.is_none() {
            return Err(invalid(
                "domain".to_string(),
                zerodds_config::Origin::Default,
                "a DDS domain id is required (equivalently, --domain N)".to_string(),
            ));
        }
        if !out.auto && out.topics.is_empty() {
            return Err(invalid(
                "auto".to_string(),
                zerodds_config::Origin::Default,
                "at least one of `auto: true` or a `durability.topics` entry is required \
                 (equivalently, --auto or --topic)"
                    .to_string(),
            ));
        }
        Ok(out)
    }
}

/// One `durability.topics[]` entry: a bare string is a topic name with every
/// override at `None`; a map reads `name`/`type`/`keyed`/`contract`.
fn read_topic(path: &str, item: &zerodds_config::Node) -> Result<TopicConfig, ConfigError> {
    if let zerodds_config::Node::Scalar(text, _) = item {
        return Ok(TopicConfig {
            name: text.clone(),
            type_name: None,
            keyed: None,
            contract: None,
        });
    }
    let entry = Section::of(Some(item), path.to_string())?;
    entry.only(&["name", "type", "keyed", "contract"])?;
    let name = entry.string("name")?.ok_or_else(|| {
        invalid(
            entry.path_of("name"),
            first_origin(item),
            "a durability.topics entry needs a name".to_string(),
        )
    })?;
    let type_name = entry.string("type")?;
    let keyed = entry.boolean("keyed")?;
    if keyed.is_some() && type_name.is_none() {
        return Err(invalid(
            entry.path_of("keyed"),
            first_origin(item),
            "keyed needs type (keyedness is only meaningful for an explicit type override; \
             without `type`, keyedness comes from discovery)"
                .to_string(),
        ));
    }
    let contract = match entry.node("contract") {
        Some(node) => Some(read_contract(&entry.path_of("contract"), node)?),
        None => None,
    };
    Ok(TopicConfig {
        name,
        type_name,
        keyed,
        contract,
    })
}

/// A `durability.topics[].contract` map. Any field left out keeps
/// [`Contract::keep_all`]'s value for it (unlimited caps, no cleanup
/// delay) — not the DDS `DurabilityServiceQosPolicy` default
/// (`KEEP_LAST(1)`), which is rarely what an operator pinning a contract in
/// config wants.
fn read_contract(path: &str, node: &zerodds_config::Node) -> Result<Contract, ConfigError> {
    let entry = Section::of(Some(node), path.to_string())?;
    entry.only(&[
        "history_kind",
        "history_depth",
        "max_samples",
        "max_instances",
        "max_samples_per_instance",
        "cleanup_delay_secs",
    ])?;
    let mut c = Contract::keep_all();
    if let Some(v) = entry.choice("history_kind", &["keep_last", "keep_all"])? {
        c.history_kind = if v == "keep_all" {
            HistoryKind::KeepAll
        } else {
            HistoryKind::KeepLast
        };
    }
    if let Some(v) = entry.parse::<i32>("history_depth", "an integer (KEEP_LAST depth)")? {
        c.history_depth = v;
    }
    if let Some(v) = entry.parse::<i32>("max_samples", "an integer, -1 for unlimited")? {
        c.max_samples = v;
    }
    if let Some(v) = entry.parse::<i32>("max_instances", "an integer, -1 for unlimited")? {
        c.max_instances = v;
    }
    if let Some(v) =
        entry.parse::<i32>("max_samples_per_instance", "an integer, -1 for unlimited")?
    {
        c.max_samples_per_instance = v;
    }
    if let Some(v) = entry.parse::<u64>("cleanup_delay_secs", "an integer, seconds")? {
        c.cleanup_delay = core::time::Duration::from_secs(v);
    }
    Ok(c)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// What `zerodds-durability-svc.yaml(5)` requires and the parser
    /// refuses: `keyed` only together with `type`; at least one of `auto`
    /// and a `durability.topics` entry; no unknown key in the `durability`
    /// section or in a topic entry (that stops the start).
    #[test]
    fn what_the_manual_requires_is_refused_when_missing() {
        for (yaml, setting) in [
            (
                "domain: 0\ndurability:\n  topics:\n    - name: Robots\n      keyed: true\n",
                None,
            ),
            ("domain: 0\n", None),
            (
                "domain: 0\nauto: true\ndurability:\n  bogus: 1\n",
                Some("durability.bogus"),
            ),
            (
                "domain: 0\nauto: true\ndurability:\n  topics:\n    - name: Sensors\n      bogus: 1\n",
                None,
            ),
        ] {
            let err = DaemonConfig::load_from_str(yaml).unwrap_err();
            match setting {
                Some(want) => assert!(
                    matches!(&err, ConfigError::Invalid { setting, .. } if setting == want),
                    "{yaml}: {err}"
                ),
                None => assert!(matches!(err, ConfigError::Invalid { .. }), "{yaml}: {err}"),
            }
        }
    }

    /// `zerodds-durability-svc.yaml(5)`, `contract`: a field left out keeps
    /// `keep_all`'s unlimited value, not the DDS policy default.
    #[test]
    fn a_contract_field_left_out_keeps_keep_alls_unlimited_value() {
        let yaml = "\
domain: 0
durability:
  topics:
    - name: Sensors
      contract:
        history_kind: keep_last
        history_depth: 2
        max_instances: 100
        cleanup_delay_secs: 30
";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        let c = cfg.topics[0].contract.expect("contract override");
        assert_eq!(c.history_kind, HistoryKind::KeepLast);
        assert_eq!(c.history_depth, 2);
        assert_eq!(c.max_instances, 100);
        assert_eq!(c.cleanup_delay, core::time::Duration::from_secs(30));
        // Fields left out keep `keep_all`'s unlimited, not the QoS default.
        assert_eq!(c.max_samples, -1);
    }

    /// The packaged example (`packaging/linux/configs/`) reads under this
    /// daemon's strict key check as what it spells: a bare topic name and a
    /// map entry with `type`, `keyed` and a `contract`. This bin crate has
    /// no `[lib]` target, so an external `tests/` target cannot see
    /// `config::DaemonConfig`; the check runs as a unit test.
    #[test]
    fn the_packaged_example_reads_as_what_it_spells() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packaging/linux/configs/durability-svc.yaml.example");
        let cfg = DaemonConfig::load_from_file(&path)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(cfg.domain, Some(0));
        assert_eq!(cfg.store, "sqlite");
        assert!(cfg.auto);
        assert_eq!(cfg.topics.len(), 2);
        assert_eq!(cfg.topics[0].name, "Sensors");
        assert!(cfg.topics[0].type_name.is_none());
        assert_eq!(cfg.topics[1].name, "Commands");
        assert_eq!(cfg.topics[1].type_name.as_deref(), Some("demo::Command"));
        assert_eq!(cfg.topics[1].keyed, Some(true));
        let contract = cfg.topics[1]
            .contract
            .expect("Commands has a contract override");
        assert_eq!(contract.history_kind, HistoryKind::KeepLast);
        assert_eq!(contract.history_depth, 5);
        assert_eq!(contract.max_instances, 1000);
        assert_eq!(contract.cleanup_delay, core::time::Duration::from_secs(300));
    }
}
