// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The push targets of the daemon's Security Key Service (spec §4.3, OPC
//! 10000-14 §8.6, §8.7): the `KeyPushTargets` folder with `AddPushTarget`,
//! `RemovePushTarget`, `AddPushTargetFolder` and `RemovePushTargetFolder`,
//! a `PubSubKeyPushTargetType` Object per target with its Properties and
//! `ConnectSecurityGroups`, `DisconnectSecurityGroups`, `TriggerKeyUpdate`,
//! and the thread that pushes the keys of the connected SecurityGroups with
//! `SetSecurityKeys` (§5.4.5.3, §8.6.2).

use std::string::{String, ToString};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::vec::Vec;

use zerodds_opcua_gateway::data_value::{
    DataValue, ExtensionObject, ExtensionObjectBody, Variant, VariantValue,
};
use zerodds_opcua_gateway::node_id::NodeId;
use zerodds_opcua_gateway::types::{LocalizedText, QualifiedName};
use zerodds_opcua_pubsub::security::SetSecurityKeysInput;
use zerodds_opcua_server::address_space::{
    AddressSpace, Argument, MethodContext, MethodOutcome, NodeClass, NodeMeta, reference_types,
};
use zerodds_opcua_server::status;
use zerodds_opcua_uacp::crypto::{RsaPrivateKey, RsaPublicKey};

use super::keys::{
    BAD_BROWSE_NAME_DUPLICATED, BAD_NODE_ID_EXISTS, BAD_NODE_ID_INVALID, GOOD_DATA_IGNORED,
    KeyService, PUBLISH_SUBSCRIBE, SET_SECURITY_KEYS, folder_name,
};
use super::log::{Level, daemon_log};

/// `PublishSubscribe_KeyPushTargets` (`i=25440`).
pub const KEY_PUSH_TARGETS: NodeId = NodeId::numeric(0, 25440);
/// `PublishSubscribe_KeyPushTargets_AddPushTarget` (`i=25441`).
pub const ADD_PUSH_TARGET: NodeId = NodeId::numeric(0, 25441);
/// `PublishSubscribe_KeyPushTargets_RemovePushTarget` (`i=25444`).
pub const REMOVE_PUSH_TARGET: NodeId = NodeId::numeric(0, 25444);
/// `PublishSubscribe_KeyPushTargets_AddPushTargetFolder` (`i=25446`).
pub const ADD_PUSH_TARGET_FOLDER: NodeId = NodeId::numeric(0, 25446);
/// `PublishSubscribe_KeyPushTargets_RemovePushTargetFolder` (`i=25449`).
pub const REMOVE_PUSH_TARGET_FOLDER: NodeId = NodeId::numeric(0, 25449);
/// `PubSubKeyPushTargetType` (`i=25337`).
const PUSH_TARGET_TYPE: NodeId = NodeId::numeric(0, 25337);
/// `PubSubKeyPushTargetFolderType` (`i=25346`).
const PUSH_TARGET_FOLDER_TYPE: NodeId = NodeId::numeric(0, 25346);
/// `HasPushedSecurityGroup` (`i=25345`, §8.6.6).
pub const HAS_PUSHED_SECURITY_GROUP: NodeId = NodeId::numeric(0, 25345);
/// `UserTokenPolicy_Encoding_DefaultBinary` (`i=306`).
const USER_TOKEN_POLICY_BINARY: NodeId = NodeId::numeric(0, 306);
/// `Good_EntryReplaced` (`0x00A30000`, §8.6.3).
pub const GOOD_ENTRY_REPLACED: u32 = 0x00A3_0000;

/// The fewest keys a push carries (§8.6.1: "The minimum setting for this is
/// three").
const MIN_REQUESTED_KEY_COUNT: u16 = 3;
/// The longest name of a folder, and of an ApplicationUri.
const MAX_NAME_LEN: usize = 256;
/// How often the pushing thread looks for a push that is due.
const PUSH_POLL: Duration = Duration::from_millis(200);
/// How long a push waits for the target.
const PUSH_TIMEOUT: Duration = Duration::from_secs(10);

/// The application identity the SKS pushes with (§8.6.1: "authorization is
/// accomplished in this case with the application identity of the SKS"):
/// the certificate and private key of its server.
#[derive(Clone)]
pub struct SksIdentity {
    /// The DER certificate.
    pub certificate: Vec<u8>,
    /// Its private key.
    pub private_key: RsaPrivateKey,
}

/// One push target (§8.6.1).
#[derive(Debug, Clone)]
struct Target {
    node: NodeId,
    folder: NodeId,
    application_uri: String,
    endpoint_url: String,
    policy_uri: String,
    user_token: ExtensionObject,
    requested_key_count: u16,
    retry_interval_ms: f64,
    /// The SecurityGroups connected to it, by id.
    groups: Vec<String>,
    /// When the next push is due; `None` until a group is connected.
    next_push: Option<Instant>,
}

/// A folder of push targets (§8.7): its node and the names up to it.
struct Folder {
    node: NodeId,
    path: String,
}

/// The push targets of the SKS.
#[derive(Default)]
pub struct PushTargets {
    targets: Mutex<Vec<Target>>,
    folders: Mutex<Vec<Folder>>,
    identity: Mutex<Option<SksIdentity>>,
}

impl PushTargets {
    /// Sets the identity the SKS pushes with.
    pub fn set_identity(&self, identity: SksIdentity) {
        if let Ok(mut slot) = self.identity.lock() {
            *slot = Some(identity);
        }
    }

    /// Disconnects the SecurityGroup `id` from every target, when it is
    /// removed (its node and the references to it are gone with it).
    pub fn forget_group(&self, id: &str) {
        if let Ok(mut targets) = self.targets.lock() {
            for target in targets.iter_mut() {
                target.groups.retain(|g| g != id);
            }
        }
    }
}

fn qname(name: &str) -> QualifiedName {
    QualifiedName {
        namespace_index: 0,
        name: name.into(),
    }
}

fn text(name: &str) -> LocalizedText {
    LocalizedText {
        locale: None,
        text: Some(name.into()),
    }
}

/// The DateTime of now (100 ns since 1601-01-01).
fn date_time_now() -> i64 {
    const EPOCH_DIFF: i64 = 116_444_736_000_000_000;
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| {
            EPOCH_DIFF.saturating_add(i64::try_from(since.as_nanos() / 100).unwrap_or(i64::MAX))
        })
}

impl KeyService {
    /// The `KeyPushTargets` folder with its Methods (§8.7), a component of
    /// `PublishSubscribe`.
    pub(crate) fn install_push_targets(self: &Arc<Self>, space: &mut AddressSpace) {
        space.add_node(
            NodeMeta::new(KEY_PUSH_TARGETS, NodeClass::Object, qname("KeyPushTargets"))
                .with_display_name(text("KeyPushTargets"))
                .with_type_definition(PUSH_TARGET_FOLDER_TYPE),
        );
        space.add_reference(
            PUBLISH_SUBSCRIBE,
            reference_types::HAS_COMPONENT,
            KEY_PUSH_TARGETS,
        );
        space.add_reference(
            KEY_PUSH_TARGETS,
            reference_types::HAS_TYPE_DEFINITION,
            PUSH_TARGET_FOLDER_TYPE,
        );
        self.install_push_folder(
            space,
            &KEY_PUSH_TARGETS,
            [
                ADD_PUSH_TARGET,
                REMOVE_PUSH_TARGET,
                ADD_PUSH_TARGET_FOLDER,
                REMOVE_PUSH_TARGET_FOLDER,
            ],
        );
    }

    fn install_push_folder(
        self: &Arc<Self>,
        space: &mut AddressSpace,
        folder: &NodeId,
        ids: [NodeId; 4],
    ) {
        let string = NodeId::numeric(0, 12);
        let uint16 = NodeId::numeric(0, 5);
        let duration = NodeId::numeric(0, 290);
        let node_id = NodeId::numeric(0, 17);
        let user_token_policy = NodeId::numeric(0, 304);
        let [add, remove, add_folder, remove_folder] = ids;
        let (me, at) = (Arc::clone(self), folder.clone());
        space.add_method_mut(
            folder,
            add,
            qname("AddPushTarget"),
            &[
                Argument::scalar("ApplicationUri", string.clone()),
                Argument::scalar("EndpointUrl", string.clone()),
                Argument::scalar("SecurityPolicyUri", string.clone()),
                Argument::scalar("UserTokenType", user_token_policy),
                Argument::scalar("RequestedKeyCount", uint16),
                Argument::scalar("RetryInterval", duration),
            ],
            &[Argument::scalar("PushTargetId", node_id.clone())],
            move |space, context, args| me.add_push_target_object(space, &at, context, args),
        );
        let (me, at) = (Arc::clone(self), folder.clone());
        space.add_method_mut(
            folder,
            remove,
            qname("RemovePushTarget"),
            &[Argument::scalar("PushTargetId", node_id.clone())],
            &[],
            move |space, context, args| me.remove_push_target_object(space, &at, context, args),
        );
        let (me, at) = (Arc::clone(self), folder.clone());
        space.add_method_mut(
            folder,
            add_folder,
            qname("AddPushTargetFolder"),
            &[Argument::scalar("Name", string)],
            &[Argument::scalar("PushTargetFolderNodeId", node_id.clone())],
            move |space, context, args| me.add_push_folder(space, &at, context, args),
        );
        let (me, at) = (Arc::clone(self), folder.clone());
        space.add_method_mut(
            folder,
            remove_folder,
            qname("RemovePushTargetFolder"),
            &[Argument::scalar("PushTargetFolderNodeId", node_id)],
            &[],
            move |space, context, args| me.remove_push_folder(space, &at, context, args),
        );
    }

    /// `AddPushTarget` (§8.7.2).
    fn add_push_target_object(
        self: &Arc<Self>,
        space: &mut AddressSpace,
        folder: &NodeId,
        context: &MethodContext,
        args: &[Variant],
    ) -> MethodOutcome {
        if !context.signed {
            return MethodOutcome::fault(status::BAD_SECURITY_MODE_INSUFFICIENT);
        }
        if !self.is_admin(context) {
            return MethodOutcome::fault(status::BAD_USER_ACCESS_DENIED);
        }
        let scalar = |index: usize| {
            args.get(index)
                .filter(|v| v.is_scalar())
                .and_then(|v| v.value.first())
        };
        let (
            Some(VariantValue::String(application_uri)),
            Some(VariantValue::String(endpoint_url)),
            Some(VariantValue::String(policy_uri)),
            Some(VariantValue::ExtensionObject(user_token)),
            Some(VariantValue::UInt16(requested_key_count)),
            Some(VariantValue::Double(retry_interval)),
        ) = (
            scalar(0),
            scalar(1),
            scalar(2),
            scalar(3),
            scalar(4),
            scalar(5),
        )
        else {
            return MethodOutcome::fault(status::BAD_INVALID_ARGUMENT);
        };
        // The SecureChannel to the target: a policy this daemon speaks,
        // always SignAndEncrypt (§8.6.1); the user token Anonymous, the
        // identity of the SKS (the only kind it has no secret for to give).
        let valid = args.len() == 6
            && !application_uri.is_empty()
            && application_uri.len() <= MAX_NAME_LEN
            && endpoint_url.starts_with("opc.tcp://")
            && zerodds_opcua_uacp::crypto::SecurityPolicy::from_uri(policy_uri).is_some()
            && is_anonymous(user_token)
            && *requested_key_count >= MIN_REQUESTED_KEY_COUNT
            && retry_interval.is_finite()
            && *retry_interval > 0.0;
        if !valid {
            return MethodOutcome::fault(status::BAD_INVALID_ARGUMENT);
        }
        let Ok(mut targets) = self.push.targets.lock() else {
            return MethodOutcome::fault(status::BAD_INVALID_STATE);
        };
        if let Some(existing) = targets
            .iter()
            .find(|t| t.application_uri == *application_uri)
        {
            let same = existing.endpoint_url == *endpoint_url
                && existing.policy_uri == *policy_uri
                && existing.requested_key_count == *requested_key_count
                && existing.retry_interval_ms == *retry_interval
                && existing.folder == *folder;
            if !same {
                return MethodOutcome::fault(BAD_NODE_ID_EXISTS);
            }
            return MethodOutcome {
                status_code: GOOD_DATA_IGNORED,
                output_arguments: std::vec![Variant::scalar(VariantValue::NodeId(
                    existing.node.clone()
                ))],
            };
        }
        let Ok(node) = NodeId::string(self.ns(), std::format!("KeyPushTarget:{application_uri}"))
        else {
            return MethodOutcome::fault(status::BAD_INVALID_ARGUMENT);
        };
        let target = Target {
            node: node.clone(),
            folder: folder.clone(),
            application_uri: application_uri.clone(),
            endpoint_url: endpoint_url.clone(),
            policy_uri: policy_uri.clone(),
            user_token: user_token.clone(),
            requested_key_count: *requested_key_count,
            retry_interval_ms: *retry_interval,
            groups: Vec::new(),
            next_push: None,
        };
        self.install_target(space, &target);
        targets.push(target);
        daemon_log!(Level::Info, "key push target `{application_uri}` added");
        MethodOutcome::good(std::vec![Variant::scalar(VariantValue::NodeId(node))])
    }

    /// The Object of a push target with its Properties and Methods
    /// (§8.6.1 Table 221).
    fn install_target(self: &Arc<Self>, space: &mut AddressSpace, target: &Target) {
        let ns = self.ns();
        let child = |name: &str| {
            NodeId::string(
                ns,
                std::format!("KeyPushTarget:{}/{name}", target.application_uri),
            )
            .unwrap_or_else(|_| NodeId::numeric(ns, 0))
        };
        space.add_node(
            NodeMeta::new(
                target.node.clone(),
                NodeClass::Object,
                QualifiedName {
                    namespace_index: ns,
                    name: target.application_uri.clone(),
                },
            )
            .with_type_definition(PUSH_TARGET_TYPE),
        );
        space.add_reference(
            target.folder.clone(),
            reference_types::HAS_COMPONENT,
            target.node.clone(),
        );
        space.add_reference(
            target.node.clone(),
            reference_types::HAS_TYPE_DEFINITION,
            PUSH_TARGET_TYPE,
        );
        let properties = [
            (
                "ApplicationUri",
                VariantValue::String(target.application_uri.clone()),
            ),
            (
                "EndpointUrl",
                VariantValue::String(target.endpoint_url.clone()),
            ),
            (
                "SecurityPolicyUri",
                VariantValue::String(target.policy_uri.clone()),
            ),
            (
                "UserTokenType",
                VariantValue::ExtensionObject(target.user_token.clone()),
            ),
            (
                "RequestedKeyCount",
                VariantValue::UInt16(target.requested_key_count),
            ),
            (
                "RetryInterval",
                VariantValue::Double(target.retry_interval_ms),
            ),
            // "A null DateTime value indicates that no successful push was
            // executed" / "that no error has occurred".
            ("LastPushExecutionTime", VariantValue::DateTime(0)),
            ("LastPushErrorTime", VariantValue::DateTime(0)),
        ];
        for (name, value) in properties {
            let id = child(name);
            space.add_node(
                NodeMeta::new(id.clone(), NodeClass::Variable, qname(name))
                    .with_display_name(text(name))
                    .with_type_definition(NodeId::numeric(0, 68)),
            );
            space.set_value(
                id.clone(),
                DataValue::new_value(Variant::scalar(value), 0, 0),
            );
            space.add_reference(target.node.clone(), reference_types::HAS_PROPERTY, id);
        }
        let node_ids = NodeId::numeric(0, 17);
        let status_codes = NodeId::numeric(0, 19);
        for (name, which) in [
            ("ConnectSecurityGroups", 0u8),
            ("DisconnectSecurityGroups", 1),
            ("TriggerKeyUpdate", 2),
        ] {
            let me = Arc::clone(self);
            let at = target.node.clone();
            let (inputs, outputs) = match which {
                0 => (
                    std::vec![Argument::array("SecurityGroupIds", node_ids.clone())],
                    std::vec![Argument::array("ConnectResults", status_codes.clone())],
                ),
                1 => (
                    std::vec![Argument::array("SecurityGroupIds", node_ids.clone())],
                    std::vec![Argument::array("DisconnectResults", status_codes.clone())],
                ),
                _ => (Vec::new(), Vec::new()),
            };
            space.add_method_mut(
                &target.node,
                child(name),
                qname(name),
                &inputs,
                &outputs,
                move |space, context, args| me.target_method(space, &at, which, context, args),
            );
        }
    }

    /// `ConnectSecurityGroups`, `DisconnectSecurityGroups` and
    /// `TriggerKeyUpdate` (§8.6.3 to §8.6.5) of the target `node`.
    fn target_method(
        &self,
        space: &mut AddressSpace,
        node: &NodeId,
        which: u8,
        context: &MethodContext,
        args: &[Variant],
    ) -> MethodOutcome {
        if !context.signed {
            return MethodOutcome::fault(status::BAD_SECURITY_MODE_INSUFFICIENT);
        }
        if !self.is_admin(context) {
            return MethodOutcome::fault(status::BAD_USER_ACCESS_DENIED);
        }
        let Ok(mut targets) = self.push.targets.lock() else {
            return MethodOutcome::fault(status::BAD_INVALID_STATE);
        };
        let Some(target) = targets.iter_mut().find(|t| t.node == *node) else {
            return MethodOutcome::fault(status::BAD_INVALID_STATE);
        };
        if which == 2 {
            // "The SKS shall push the new set of keys for all related
            // SecurityGroups, even if not currently scheduled."
            if !target.groups.is_empty() {
                target.next_push = Some(Instant::now());
            }
            return MethodOutcome::good(Vec::new());
        }
        let nodes: Vec<NodeId> = match args {
            [v] => v
                .value
                .iter()
                .filter_map(|value| match value {
                    VariantValue::NodeId(id) => Some(id.clone()),
                    _ => None,
                })
                .collect(),
            _ => return MethodOutcome::fault(status::BAD_INVALID_ARGUMENT),
        };
        if args.first().is_some_and(|v| v.value.len() != nodes.len()) {
            return MethodOutcome::fault(status::BAD_INVALID_ARGUMENT);
        }
        let mut results = Vec::with_capacity(nodes.len());
        for group_node in nodes {
            let Some(id) = self.group_id_of(&group_node) else {
                results.push(VariantValue::StatusCode(
                    if space.node_meta(&group_node).is_some() {
                        BAD_NODE_ID_INVALID
                    } else {
                        status::BAD_NODE_ID_UNKNOWN
                    },
                ));
                continue;
            };
            let connected = target.groups.contains(&id);
            if which == 0 {
                if connected {
                    results.push(VariantValue::StatusCode(GOOD_ENTRY_REPLACED));
                } else {
                    target.groups.push(id);
                    space.add_reference(
                        target.node.clone(),
                        HAS_PUSHED_SECURITY_GROUP,
                        group_node.clone(),
                    );
                    results.push(VariantValue::StatusCode(status::GOOD));
                }
                // "The SKS shall push keys following this assignment";
                // for an existing one "a new push of the existing keys".
                target.next_push = Some(Instant::now());
            } else {
                if connected {
                    target.groups.retain(|g| *g != id);
                    space.remove_reference(&target.node, &HAS_PUSHED_SECURITY_GROUP, &group_node);
                }
                results.push(VariantValue::StatusCode(status::GOOD));
                if target.groups.is_empty() {
                    target.next_push = None;
                }
            }
        }
        let count = u32::try_from(results.len()).unwrap_or(u32::MAX);
        MethodOutcome::good(std::vec![Variant {
            array_dimensions: std::vec![count],
            value: results,
        }])
    }

    /// `RemovePushTarget` (§8.7.3).
    fn remove_push_target_object(
        &self,
        space: &mut AddressSpace,
        folder: &NodeId,
        context: &MethodContext,
        args: &[Variant],
    ) -> MethodOutcome {
        if !context.signed {
            return MethodOutcome::fault(status::BAD_SECURITY_MODE_INSUFFICIENT);
        }
        if !self.is_admin(context) {
            return MethodOutcome::fault(status::BAD_USER_ACCESS_DENIED);
        }
        let Some(node) = node_argument(args) else {
            return MethodOutcome::fault(status::BAD_INVALID_ARGUMENT);
        };
        let Ok(mut targets) = self.push.targets.lock() else {
            return MethodOutcome::fault(status::BAD_INVALID_STATE);
        };
        let Some(index) = targets.iter().position(|t| t.node == node) else {
            return MethodOutcome::fault(if space.node_meta(&node).is_some() {
                BAD_NODE_ID_INVALID
            } else {
                status::BAD_NODE_ID_UNKNOWN
            });
        };
        if targets[index].folder != *folder {
            return MethodOutcome::fault(BAD_NODE_ID_INVALID);
        }
        let target = targets.remove(index);
        space.remove_subtree(&target.node);
        daemon_log!(
            Level::Info,
            "key push target `{}` removed",
            target.application_uri
        );
        MethodOutcome::good(Vec::new())
    }

    /// `AddPushTargetFolder` (§8.7.4).
    fn add_push_folder(
        self: &Arc<Self>,
        space: &mut AddressSpace,
        folder: &NodeId,
        context: &MethodContext,
        args: &[Variant],
    ) -> MethodOutcome {
        if !self.is_admin(context) {
            return MethodOutcome::fault(status::BAD_USER_ACCESS_DENIED);
        }
        let name = match folder_name(args) {
            Ok(name) => name,
            Err(fault) => return fault,
        };
        let Ok(mut folders) = self.push.folders.lock() else {
            return MethodOutcome::fault(status::BAD_INVALID_STATE);
        };
        let parent = folders
            .iter()
            .find(|f| f.node == *folder)
            .map_or(String::new(), |f| f.path.clone());
        let path = std::format!("{parent}/{name}");
        if folders.iter().any(|f| f.path == path) {
            return MethodOutcome::fault(BAD_BROWSE_NAME_DUPLICATED);
        }
        let ns = self.ns();
        let Ok(node) = NodeId::string(ns, std::format!("KeyPushTargetFolder:{path}")) else {
            return MethodOutcome::fault(status::BAD_INVALID_ARGUMENT);
        };
        space.add_node(
            NodeMeta::new(
                node.clone(),
                NodeClass::Object,
                QualifiedName {
                    namespace_index: ns,
                    name,
                },
            )
            .with_type_definition(PUSH_TARGET_FOLDER_TYPE),
        );
        space.add_reference(folder.clone(), reference_types::ORGANIZES, node.clone());
        space.add_reference(
            node.clone(),
            reference_types::HAS_TYPE_DEFINITION,
            PUSH_TARGET_FOLDER_TYPE,
        );
        let method = |what: &str| {
            NodeId::string(ns, std::format!("KeyPushTargetFolder:{path}/{what}"))
                .unwrap_or_else(|_| NodeId::numeric(ns, 0))
        };
        self.install_push_folder(
            space,
            &node,
            [
                method("AddPushTarget"),
                method("RemovePushTarget"),
                method("AddPushTargetFolder"),
                method("RemovePushTargetFolder"),
            ],
        );
        folders.push(Folder {
            node: node.clone(),
            path,
        });
        MethodOutcome::good(std::vec![Variant::scalar(VariantValue::NodeId(node))])
    }

    /// `RemovePushTargetFolder` (§8.7.5): the folder, its folders and
    /// their targets.
    fn remove_push_folder(
        &self,
        space: &mut AddressSpace,
        folder: &NodeId,
        context: &MethodContext,
        args: &[Variant],
    ) -> MethodOutcome {
        if !self.is_admin(context) {
            return MethodOutcome::fault(status::BAD_USER_ACCESS_DENIED);
        }
        let Some(node) = node_argument(args) else {
            return MethodOutcome::fault(status::BAD_INVALID_ARGUMENT);
        };
        let Ok(mut folders) = self.push.folders.lock() else {
            return MethodOutcome::fault(status::BAD_INVALID_STATE);
        };
        let parent = folders
            .iter()
            .find(|f| f.node == *folder)
            .map_or(String::new(), |f| f.path.clone());
        let Some(path) = folders
            .iter()
            .find(|f| {
                f.node == node
                    && f.path
                        .strip_prefix(parent.as_str())
                        .is_some_and(|rest| rest.starts_with('/') && !rest[1..].contains('/'))
            })
            .map(|f| f.path.clone())
        else {
            return MethodOutcome::fault(status::BAD_NODE_ID_UNKNOWN);
        };
        let removed = space.remove_subtree(&node);
        folders.retain(|f| f.path != path && !f.path.starts_with(&std::format!("{path}/")));
        if let Ok(mut targets) = self.push.targets.lock() {
            targets.retain(|t| !removed.contains(&t.node));
        }
        MethodOutcome::good(Vec::new())
    }

    /// The pushes that are due now: per target, the input of
    /// `SetSecurityKeys` for each of its SecurityGroups.
    fn due_pushes(&self, now: Instant) -> Vec<(Target, Vec<SetSecurityKeysInput>)> {
        let Ok(mut targets) = self.push.targets.lock() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for target in targets.iter_mut() {
            if target.groups.is_empty() || target.next_push.is_none_or(|at| at > now) {
                continue;
            }
            // Not again before this one is through.
            target.next_push = Some(now + PUSH_TIMEOUT * 6);
            let inputs = target
                .groups
                .iter()
                .filter_map(|id| self.push_input(id, u32::from(target.requested_key_count), now))
                .collect();
            out.push((target.clone(), inputs));
        }
        out
    }

    /// Records the outcome of a push to `node`: the next push after half
    /// the shortest KeyLifetime of its groups (§8.6.2), after an error after
    /// the RetryInterval; the time in `LastPushExecutionTime` or
    /// `LastPushErrorTime`.
    fn pushed(&self, node: &NodeId, lifetime: Option<Duration>, ok: bool) -> Option<(NodeId, i64)> {
        let Ok(mut targets) = self.push.targets.lock() else {
            return None;
        };
        let target = targets.iter_mut().find(|t| t.node == *node)?;
        let now = Instant::now();
        let wait = if ok {
            lifetime.map_or(Duration::from_secs(60), |l| l / 2)
        } else {
            Duration::from_secs_f64(target.retry_interval_ms / 1000.0)
        };
        if !target.groups.is_empty() {
            target.next_push = Some(now + wait);
        }
        let name = if ok {
            "LastPushExecutionTime"
        } else {
            "LastPushErrorTime"
        };
        let property = NodeId::string(
            node.namespace_index,
            std::format!("KeyPushTarget:{}/{name}", target.application_uri),
        )
        .ok()?;
        Some((property, date_time_now()))
    }
}

/// Whether a `UserTokenPolicy` names the token type Anonymous (or is
/// empty, which §8.6.1 makes the default).
fn is_anonymous(object: &ExtensionObject) -> bool {
    match &object.body {
        ExtensionObjectBody::None => true,
        ExtensionObjectBody::ByteString(body) if object.type_id == USER_TOKEN_POLICY_BINARY => {
            zerodds_opcua_pubsub::from_binary::<
                zerodds_opcua_pubsub::uadp::datatypes::UserTokenPolicy,
            >(body)
            .is_ok_and(|policy| {
                matches!(
                    policy.token_type,
                    zerodds_opcua_pubsub::uadp::datatypes::UserTokenType::Anonymous
                )
            })
        }
        _ => false,
    }
}

fn node_argument(args: &[Variant]) -> Option<NodeId> {
    match args {
        [v] if v.is_scalar() => match v.value.first() {
            Some(VariantValue::NodeId(node)) => Some(node.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// The public key of a DER certificate.
fn public_key_of(certificate: &[u8]) -> Result<RsaPublicKey, String> {
    use rsa::pkcs8::DecodePublicKey;
    use x509_cert::der::{Decode, Encode};
    let cert = x509_cert::Certificate::from_der(certificate)
        .map_err(|e| std::format!("the target's certificate: {e}"))?;
    let spki = cert
        .tbs_certificate
        .subject_public_key_info
        .to_der()
        .map_err(|e| std::format!("the target's public key: {e}"))?;
    RsaPublicKey::from_public_key_der(&spki)
        .map_err(|e| std::format!("the target's public key: {e}"))
}

/// One push (§5.4.5.3): a SecureChannel with SignAndEncrypt and the policy
/// of the target to the endpoint of the target whose ApplicationUri it
/// names, an anonymous session, `SetSecurityKeys` for each group.
fn push_once(
    target: &Target,
    identity: &SksIdentity,
    inputs: &[SetSecurityKeysInput],
) -> Result<(), String> {
    use zerodds_opcua_pubsub::uadp::datatypes::MessageSecurityMode;
    use zerodds_opcua_server::client::{Client, ClientSecurity, TcpTransport};
    let address = target
        .endpoint_url
        .strip_prefix("opc.tcp://")
        .map(|rest| rest.split('/').next().unwrap_or(rest).to_string())
        .ok_or_else(|| std::format!("`{}` is no opc.tcp URL", target.endpoint_url))?;
    let error = |e: zerodds_opcua_server::client::ClientError| std::format!("{e:?}");
    let mut transport = TcpTransport::connect(&address).map_err(error)?;
    let mut probe = Client::new();
    probe
        .open_channel(&mut transport, &target.endpoint_url)
        .map_err(error)?;
    let endpoints = probe
        .get_endpoints(&mut transport, &target.endpoint_url)
        .map_err(error)?;
    drop(transport);
    let endpoint = endpoints
        .iter()
        .find(|e| {
            e.security_policy_uri == target.policy_uri
                && e.security_mode == MessageSecurityMode::SignAndEncrypt
        })
        .ok_or_else(|| {
            std::format!(
                "the target has no SignAndEncrypt endpoint of {}",
                target.policy_uri
            )
        })?;
    // "The push operation shall fail if the ApplicationUri of the
    // connected target Server does not match this parameter."
    if endpoint.server.application_uri != target.application_uri {
        return Err(std::format!(
            "the target's ApplicationUri is `{}`",
            endpoint.server.application_uri
        ));
    }
    let server_certificate = endpoint.server_certificate.clone();
    let policy = zerodds_opcua_uacp::crypto::SecurityPolicy::from_uri(&target.policy_uri)
        .ok_or_else(|| std::format!("unknown policy {}", target.policy_uri))?;
    let mut client = Client::new();
    client.set_security(ClientSecurity {
        policy,
        mode: zerodds_opcua_uacp::crypto::SecuredMode::SignAndEncrypt,
        private_key: identity.private_key.clone(),
        certificate: identity.certificate.clone(),
        server_public_key: public_key_of(&server_certificate)?,
        server_certificate,
        rng: std::boxed::Box::new(rand::rngs::OsRng),
    });
    let mut transport = TcpTransport::connect(&address).map_err(error)?;
    client
        .connect(&mut transport, &target.endpoint_url)
        .map_err(error)?;
    for input in inputs {
        let result = client
            .call_method(
                &mut transport,
                PUBLISH_SUBSCRIBE,
                SET_SECURITY_KEYS,
                input.to_arguments(),
            )
            .map_err(error)?;
        if result.status_code != status::GOOD {
            return Err(std::format!(
                "SetSecurityKeys of `{}`: 0x{:08X}",
                input.security_group_id,
                result.status_code
            ));
        }
    }
    let _ = client.close_session(&mut transport, true);
    Ok(())
}

/// The thread that pushes the keys of the connected SecurityGroups to the
/// push targets (§8.6.2) and writes the outcome into the target's
/// Properties of `server`. Runs until `stop`.
pub fn spawn_pusher(
    service: Arc<KeyService>,
    server: Arc<Mutex<zerodds_opcua_server::server::Server>>,
    stop: Arc<std::sync::atomic::AtomicBool>,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("opcua-sks-push".to_string())
        .spawn(move || {
            while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                std::thread::sleep(PUSH_POLL);
                let identity = service
                    .push
                    .identity
                    .lock()
                    .ok()
                    .and_then(|slot| slot.clone());
                for (target, inputs) in service.due_pushes(Instant::now()) {
                    let outcome = match identity.as_ref() {
                        Some(identity) if !inputs.is_empty() => {
                            push_once(&target, identity, &inputs)
                        }
                        Some(_) => Err("no keys for the connected SecurityGroups".to_string()),
                        None => Err(
                            "the SKS has no certificate to push with (opcua.server.certificate_der)"
                                .to_string(),
                        ),
                    };
                    if let Err(e) = &outcome {
                        daemon_log!(Level::Warn, "key push to `{}`: {e}", target.application_uri);
                    }
                    let lifetime = inputs
                        .iter()
                        .map(|input| Duration::from_secs_f64(input.key_lifetime_ms / 1000.0))
                        .min();
                    if let Some((property, at)) =
                        service.pushed(&target.node, lifetime, outcome.is_ok())
                        && let Ok(mut guard) = server.lock()
                    {
                        guard.address_space_mut().set_value(
                            property,
                            DataValue::new_value(Variant::scalar(VariantValue::DateTime(at)), 0, 0),
                        );
                    }
                }
            }
        })
}
