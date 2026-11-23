// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The keys of secured PubSub connections (spec §4.3, OPC 10000-14 §5.4.5
//! and §8): this daemon as a Security Key Service for the SecurityGroups of
//! `opcua.server.security_groups` (`GetSecurityKeys`, `GetSecurityGroup`,
//! the SecurityGroup Objects with `InvalidateKeys` and `ForceKeyRotation`),
//! as a target an SKS pushes keys to (`SetSecurityKeys`), and as a client
//! that pulls the keys of a SecurityGroup from an SKS (`GetSecurityKeys`).

use std::string::{String, ToString};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::vec::Vec;

use zerodds_opcua_gateway::data_value::{DataValue, Variant, VariantValue};
use zerodds_opcua_gateway::node_id::NodeId;
use zerodds_opcua_gateway::types::{LocalizedText, QualifiedName};
use zerodds_opcua_pubsub::security::{
    GetSecurityKeysResult, SecurityKey, SecurityKeyService, SecurityPolicy, SetSecurityKeysInput,
    get_security_keys_input, next_token_id, parse_get_security_keys_input,
};
use zerodds_opcua_server::address_space::{
    AddressSpace, Argument, MethodContext, MethodOutcome, NodeClass, NodeMeta, reference_types,
};
use zerodds_opcua_server::status;

use super::config::{PubSubSecurityPolicy, SecurityGroupConfig, SksClientConfig, SksRoles};
use super::log::{Level, daemon_log};
use crate::pubsub_glue::{KeyState, SharedKeys};

/// `PublishSubscribe` (`i=14443`), the Object the key Methods are
/// components of (Part 14 §8.2).
pub const PUBLISH_SUBSCRIBE: NodeId = NodeId::numeric(0, 14443);
/// `PublishSubscribe_GetSecurityKeys` (`i=15215`).
pub const GET_SECURITY_KEYS: NodeId = NodeId::numeric(0, 15215);
/// `PublishSubscribe_GetSecurityGroup` (`i=15440`).
pub const GET_SECURITY_GROUP: NodeId = NodeId::numeric(0, 15440);
/// `PublishSubscribe_SecurityGroups` (`i=15443`).
pub const SECURITY_GROUPS: NodeId = NodeId::numeric(0, 15443);
/// `PublishSubscribe_SetSecurityKeys` (`i=17364`).
pub const SET_SECURITY_KEYS: NodeId = NodeId::numeric(0, 17364);
/// `PublishSubscribe_SecurityGroups_AddSecurityGroup` (`i=15444`).
pub const ADD_SECURITY_GROUP: NodeId = NodeId::numeric(0, 15444);
/// `PublishSubscribe_SecurityGroups_RemoveSecurityGroup` (`i=15447`).
pub const REMOVE_SECURITY_GROUP: NodeId = NodeId::numeric(0, 15447);
/// `PublishSubscribe_SecurityGroups_AddSecurityGroupFolder` (`i=25434`).
pub const ADD_SECURITY_GROUP_FOLDER: NodeId = NodeId::numeric(0, 25434);
/// `PublishSubscribe_SecurityGroups_RemoveSecurityGroupFolder` (`i=25437`).
pub const REMOVE_SECURITY_GROUP_FOLDER: NodeId = NodeId::numeric(0, 25437);
/// `PublishSubscribe_SecurityGroups_SupportedSecurityPolicyUris`
/// (`i=25439`).
pub const SUPPORTED_SECURITY_POLICY_URIS: NodeId = NodeId::numeric(0, 25439);
/// `SecurityGroupFolderType` (`i=15452`).
const SECURITY_GROUP_FOLDER_TYPE: NodeId = NodeId::numeric(0, 15452);
/// `SecurityGroupType` (`i=15471`).
const SECURITY_GROUP_TYPE: NodeId = NodeId::numeric(0, 15471);
/// `Good_DataIgnored` (`0x00D90000`): AddSecurityGroup found the group as
/// asked for and returned it (Part 14 §8.5.2).
pub const GOOD_DATA_IGNORED: u32 = 0x00D9_0000;
/// `Bad_NodeIdExists` (`0x805E0000`).
pub const BAD_NODE_ID_EXISTS: u32 = 0x805E_0000;
/// `Bad_BrowseNameDuplicated` (`0x80610000`).
pub const BAD_BROWSE_NAME_DUPLICATED: u32 = 0x8061_0000;
/// `Bad_NodeIdInvalid` (`0x80330000`).
pub const BAD_NODE_ID_INVALID: u32 = 0x8033_0000;

/// The policies the SKS makes keys for, its default first (Part 14 §8.5.1:
/// "The default SecurityPolicyUri is the first array element").
const SUPPORTED_POLICIES: [SecurityPolicy; 3] = [
    SecurityPolicy::Aes256Ctr,
    SecurityPolicy::Aes128Ctr,
    SecurityPolicy::Aes256Gcm,
];
/// The KeyLifetime of a group added with `KeyLifetime` 0, and its bounds.
const DEFAULT_KEY_LIFETIME_MS: u64 = 3_600_000;
const MIN_KEY_LIFETIME_MS: u64 = 1_000;
const MAX_KEY_LIFETIME_MS: u64 = 30 * 24 * 3_600_000;
/// The MaxFutureKeyCount of a group added with 0, and the bound of both
/// counts.
const DEFAULT_FUTURE_KEYS: u32 = 3;
const MAX_KEY_COUNT: u32 = 64;
/// The longest name of a SecurityGroup or a folder.
const MAX_NAME_LEN: usize = 256;

/// How long the client of an SKS waits before it tries again after a call
/// that failed (Part 14 §5.4.5.3: "the Client executing the key exchange
/// needs to retry the key exchange at a faster rate than the key
/// lifetime").
const PULL_RETRY: Duration = Duration::from_secs(5);

/// The SecurityPolicy a config entry names.
#[must_use]
pub fn policy_of(policy: PubSubSecurityPolicy) -> SecurityPolicy {
    match policy {
        PubSubSecurityPolicy::Aes128Ctr => SecurityPolicy::Aes128Ctr,
        PubSubSecurityPolicy::Aes256Ctr => SecurityPolicy::Aes256Ctr,
        PubSubSecurityPolicy::Aes256Gcm => SecurityPolicy::Aes256Gcm,
    }
}

/// A new key of `policy` for `token_id`, from the operating system's
/// random source.
#[must_use]
pub fn generate_key(policy: SecurityPolicy, token_id: u32) -> SecurityKey {
    use rand::RngCore;
    let bytes = |len: usize| {
        let mut out = std::vec![0u8; len];
        rand::rngs::OsRng.fill_bytes(&mut out);
        out
    };
    SecurityKey {
        token_id,
        signing_key: bytes(policy.signing_key_len()),
        encrypting_key: bytes(policy.encrypting_key_len()),
        key_nonce: bytes(policy.key_nonce_len()),
    }
}

/// A SecurityGroup whose keys this daemon makes (Part 14 §8.4.1).
struct LocalGroup {
    config: SecurityGroupConfig,
    policy: SecurityPolicy,
    node: NodeId,
    /// The folder it is a component of (Part 14 §8.5.1).
    folder: NodeId,
    keys: SharedKeys,
}

impl LocalGroup {
    fn new(config: SecurityGroupConfig, ns: u16, now: Instant) -> Self {
        let policy = policy_of(config.policy);
        Self::with_policy(config, policy, ns, now)
    }

    fn with_policy(
        config: SecurityGroupConfig,
        policy: SecurityPolicy,
        ns: u16,
        now: Instant,
    ) -> Self {
        let lifetime = Duration::from_millis(config.key_lifetime_ms);
        // Part 14 §8.3.2: "It starts at 1".
        let mut service = SecurityKeyService::new(
            policy,
            config.security_group_id.clone(),
            generate_key(policy, 1),
        );
        service.set_key_lifetime_ms(config.key_lifetime_ms as f64);
        service.set_max_past_key_count(config.max_past_key_count as usize);
        let mut token = 1;
        for _ in 0..config.max_future_key_count {
            token = next_token_id(token);
            service.push_future_key(generate_key(policy, token));
        }
        let node = NodeId::string(
            ns,
            std::format!("SecurityGroups/{}", config.security_group_id),
        )
        .unwrap_or_else(|_| NodeId::numeric(ns, 0));
        let keys = KeyState::waiting(policy);
        if let Ok(mut state) = keys.lock() {
            state.service = Some(service);
            state.key_lifetime = lifetime;
            state.next_rotation = Some(now + lifetime);
        }
        Self {
            config,
            policy,
            node,
            folder: SECURITY_GROUPS,
            keys,
        }
    }

    /// Keeps `max_future_key_count` future keys after the current one.
    fn top_up(&self, state: &mut KeyState) {
        let Some(service) = state.service.as_mut() else {
            return;
        };
        let mut last = service
            .future_keys()
            .last()
            .map_or(service.current_key().token_id, |k| k.token_id);
        while service.future_keys().len() < self.config.max_future_key_count as usize {
            last = next_token_id(last);
            service.push_future_key(generate_key(self.policy, last));
        }
    }

    /// Rotates every key whose lifetime is over, on the schedule: a pump
    /// that comes late, or a `GetSecurityKeys` that comes before the pump,
    /// sees the keys the clock says are current.
    fn catch_up(&self, state: &mut KeyState, now: Instant) {
        let Some(mut at) = state.next_rotation else {
            return;
        };
        if state.key_lifetime.is_zero() {
            return;
        }
        while now >= at {
            self.top_up(state);
            if let Some(service) = state.service.as_mut() {
                service.rotate();
            }
            at += state.key_lifetime;
        }
        self.top_up(state);
        state.next_rotation = Some(at);
    }

    /// The next key current now (Part 14 §8.4.3 `ForceKeyRotation`): the
    /// lifetime of the new current key starts now.
    fn rotate_now(&self, now: Instant) {
        let Ok(mut state) = self.keys.lock() else {
            return;
        };
        self.top_up(&mut state);
        if let Some(service) = state.service.as_mut() {
            service.rotate();
        }
        self.top_up(&mut state);
        state.next_rotation = Some(now + state.key_lifetime);
    }

    /// New current and future keys (Part 14 §8.4.2 `InvalidateKeys`): the
    /// new current token is beyond the last invalidated future one.
    fn invalidate(&self, now: Instant) {
        let Ok(mut state) = self.keys.lock() else {
            return;
        };
        if let Some(service) = state.service.as_mut() {
            let last = service
                .future_keys()
                .last()
                .map_or(service.current_key().token_id, |k| k.token_id);
            service.invalidate(generate_key(self.policy, next_token_id(last)));
        }
        self.top_up(&mut state);
        state.next_rotation = Some(now + state.key_lifetime);
    }
}

/// A connection whose keys an SKS pushes with `SetSecurityKeys`.
struct PushTarget {
    security_group_id: String,
    subject: String,
    keys: SharedKeys,
}

/// A folder of SecurityGroups (Part 14 §8.5): its node and the names of
/// the folders up to it.
struct Folder {
    node: NodeId,
    path: String,
}

/// The key services of the daemon: its SecurityGroups and their folders,
/// the push targets of its connections, the users of the SKS roles.
pub struct KeyService {
    ns: u16,
    roles: SksRoles,
    groups: Mutex<Vec<LocalGroup>>,
    /// The folders added at run time; the root is [`SECURITY_GROUPS`].
    folders: Mutex<Vec<Folder>>,
    push_targets: Mutex<Vec<PushTarget>>,
    /// Keys of connections pulled from an SKS: rotated with the others.
    pulled: Mutex<Vec<SharedKeys>>,
    /// The targets the SKS pushes keys to (Part 14 §8.6, §8.7).
    pub(crate) push: super::push::PushTargets,
}

impl KeyService {
    /// The SecurityGroups of `groups`, each with a current key and its
    /// future keys; `roles` are the users of the SKS roles (Part 14 §8.8).
    #[must_use]
    pub fn new(
        groups: &[SecurityGroupConfig],
        roles: &SksRoles,
        ns: u16,
        now: Instant,
    ) -> Arc<Self> {
        Arc::new(Self {
            ns,
            roles: roles.clone(),
            groups: Mutex::new(
                groups
                    .iter()
                    .cloned()
                    .map(|g| LocalGroup::new(g, ns, now))
                    .collect(),
            ),
            folders: Mutex::new(Vec::new()),
            push_targets: Mutex::new(Vec::new()),
            pulled: Mutex::new(Vec::new()),
            push: super::push::PushTargets::default(),
        })
    }

    /// The namespace of the SKS's nodes.
    pub(crate) fn ns(&self) -> u16 {
        self.ns
    }

    /// The identity the SKS pushes keys with: the certificate and the
    /// private key of its server (Part 14 §8.6.1).
    pub fn set_push_identity(&self, identity: super::push::SksIdentity) {
        self.push.set_identity(identity);
    }

    /// The id of the SecurityGroup whose Object is `node`.
    pub(crate) fn group_id_of(&self, node: &NodeId) -> Option<String> {
        self.groups.lock().ok().and_then(|groups| {
            groups
                .iter()
                .find(|g| g.node == *node)
                .map(|g| g.config.security_group_id.clone())
        })
    }

    /// The input of `SetSecurityKeys` (Part 14 §9.1.3.3) that pushes the
    /// current key of the SecurityGroup `id` and up to `count` − 1 future
    /// ones.
    pub(crate) fn push_input(
        &self,
        id: &str,
        count: u32,
        now: Instant,
    ) -> Option<zerodds_opcua_pubsub::security::SetSecurityKeysInput> {
        let groups = self.groups.lock().ok()?;
        let group = groups.iter().find(|g| g.config.security_group_id == id)?;
        let mut state = group.keys.lock().ok()?;
        group.catch_up(&mut state, now);
        let time_to_next = state.time_to_next_key(now);
        let service = state.service.as_mut()?;
        service.set_time_to_next_key_ms(time_to_next.as_secs_f64() * 1000.0);
        let result = service.get_security_keys_for(id, 0, count)?;
        let mut keys = result.keys.into_iter();
        Some(zerodds_opcua_pubsub::security::SetSecurityKeysInput {
            security_group_id: id.to_string(),
            security_policy_uri: result.security_policy_uri,
            current_token_id: result.first_token_id,
            current_key: keys.next()?,
            future_keys: keys.collect(),
            time_to_next_key_ms: result.time_to_next_key_ms,
            key_lifetime_ms: result.key_lifetime_ms,
        })
    }

    /// The keys of this daemon's SecurityGroup `id`, for a connection with
    /// `key_source: local`.
    #[must_use]
    pub fn local_keys(&self, id: &str) -> Option<SharedKeys> {
        self.groups.lock().ok().and_then(|groups| {
            groups
                .iter()
                .find(|g| g.config.security_group_id == id)
                .map(|g| Arc::clone(&g.keys))
        })
    }

    /// Whether the daemon is a Security Key Service: it has SecurityGroups
    /// or users that may add them.
    fn is_sks(&self) -> bool {
        !self.roles.admin.is_empty() || self.groups.lock().is_ok_and(|g| !g.is_empty())
    }

    /// Keys of a connection that an SKS pushes, for the session of
    /// `subject`.
    pub fn add_push_target(&self, security_group_id: &str, subject: &str, keys: SharedKeys) {
        if let Ok(mut targets) = self.push_targets.lock() {
            targets.push(PushTarget {
                security_group_id: security_group_id.to_string(),
                subject: subject.to_string(),
                keys,
            });
        }
    }

    /// Keys of a connection pulled from an SKS: their rotation runs with
    /// [`Self::tick`].
    pub fn add_pulled(&self, keys: SharedKeys) {
        if let Ok(mut pulled) = self.pulled.lock() {
            pulled.push(keys);
        }
    }

    /// Rotates every key whose time has come: the daemon's own groups
    /// make the next future key as well; pushed and pulled keys switch to
    /// the next one they were given.
    pub fn tick(&self, now: Instant) {
        if let Ok(groups) = self.groups.lock() {
            for group in groups.iter() {
                if let Ok(mut state) = group.keys.lock() {
                    group.catch_up(&mut state, now);
                }
            }
        }
        let others: Vec<SharedKeys> = self
            .push_targets
            .lock()
            .map(|t| t.iter().map(|t| Arc::clone(&t.keys)).collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .chain(self.pulled.lock().map(|p| p.clone()).unwrap_or_default())
            .collect();
        for keys in others {
            if let Ok(mut state) = keys.lock() {
                state.rotate_due(now);
            }
        }
    }

    /// Adds the PublishSubscribe Object with the key Methods and the
    /// SecurityGroup Objects to `space` (Part 14 §8.2 to §8.4).
    pub fn install(self: &Arc<Self>, space: &mut AddressSpace) {
        if !self.is_sks() && self.push_targets.lock().map_or(true, |t| t.is_empty()) {
            return;
        }
        let text = |name: &str| LocalizedText {
            locale: None,
            text: Some(name.into()),
        };
        let qname = |name: &str| QualifiedName {
            namespace_index: 0,
            name: name.into(),
        };
        space.add_node(
            NodeMeta::new(
                PUBLISH_SUBSCRIBE,
                NodeClass::Object,
                qname("PublishSubscribe"),
            )
            .with_display_name(text("PublishSubscribe"))
            .with_type_definition(NodeId::numeric(0, 14416)),
        );
        // Part 14 §8.2: a component of the Server Object; the Objects
        // folder organizes it as well, so that a client browses to it.
        space.add_reference(
            NodeId::numeric(0, 2253),
            reference_types::HAS_COMPONENT,
            PUBLISH_SUBSCRIBE,
        );
        space.add_reference(
            NodeId::numeric(0, 85),
            reference_types::ORGANIZES,
            PUBLISH_SUBSCRIBE,
        );
        let string = NodeId::numeric(0, 12);
        let uint32 = NodeId::numeric(0, 7);
        let integer_id = NodeId::numeric(0, 288);
        let duration = NodeId::numeric(0, 290);
        let byte_string = NodeId::numeric(0, 15);
        if self.is_sks() {
            let me = Arc::clone(self);
            space.add_method(
                &PUBLISH_SUBSCRIBE,
                GET_SECURITY_KEYS,
                qname("GetSecurityKeys"),
                &[
                    Argument::scalar("SecurityGroupId", string.clone()),
                    Argument::scalar("StartingTokenId", integer_id.clone()),
                    Argument::scalar("RequestedKeyCount", uint32.clone()),
                ],
                &[
                    Argument::scalar("SecurityPolicyUri", string.clone()),
                    Argument::scalar("FirstTokenId", integer_id.clone()),
                    Argument::array("Keys", byte_string.clone()),
                    Argument::scalar("TimeToNextKey", duration.clone()),
                    Argument::scalar("KeyLifetime", duration.clone()),
                ],
                move |context, args| me.get_security_keys(context, args, Instant::now()),
            );
            let me = Arc::clone(self);
            space.add_method(
                &PUBLISH_SUBSCRIBE,
                GET_SECURITY_GROUP,
                qname("GetSecurityGroup"),
                &[Argument::scalar("SecurityGroupId", string.clone())],
                &[Argument::scalar(
                    "SecurityGroupNodeId",
                    NodeId::numeric(0, 17),
                )],
                move |_, args| me.get_security_group(args),
            );
            space.add_node(
                NodeMeta::new(SECURITY_GROUPS, NodeClass::Object, qname("SecurityGroups"))
                    .with_display_name(text("SecurityGroups"))
                    .with_type_definition(SECURITY_GROUP_FOLDER_TYPE),
            );
            space.add_reference(
                PUBLISH_SUBSCRIBE,
                reference_types::HAS_COMPONENT,
                SECURITY_GROUPS,
            );
            space.add_reference(
                SECURITY_GROUPS,
                reference_types::HAS_TYPE_DEFINITION,
                SECURITY_GROUP_FOLDER_TYPE,
            );
            self.install_folder(
                space,
                &SECURITY_GROUPS,
                [
                    ADD_SECURITY_GROUP,
                    REMOVE_SECURITY_GROUP,
                    ADD_SECURITY_GROUP_FOLDER,
                    REMOVE_SECURITY_GROUP_FOLDER,
                ],
            );
            // Part 14 §8.5.1: "The Property shall be provided at the root
            // SecurityGroupFolder."
            let uris: Vec<VariantValue> = SUPPORTED_POLICIES
                .iter()
                .map(|p| VariantValue::String(p.uri().to_string()))
                .collect();
            space.add_node(
                NodeMeta::new(
                    SUPPORTED_SECURITY_POLICY_URIS,
                    NodeClass::Variable,
                    qname("SupportedSecurityPolicyUris"),
                )
                .with_display_name(text("SupportedSecurityPolicyUris"))
                .with_type_definition(NodeId::numeric(0, 68)),
            );
            space.set_value(
                SUPPORTED_SECURITY_POLICY_URIS,
                DataValue::new_value(
                    Variant {
                        array_dimensions: std::vec![uris.len() as u32],
                        value: uris,
                    },
                    0,
                    0,
                ),
            );
            space.add_reference(
                SECURITY_GROUPS,
                reference_types::HAS_PROPERTY,
                SUPPORTED_SECURITY_POLICY_URIS,
            );
            if let Ok(groups) = self.groups.lock() {
                for group in groups.iter() {
                    self.install_group(space, group);
                }
            }
            self.install_push_targets(space);
        }
        if self.push_targets.lock().is_ok_and(|t| !t.is_empty()) {
            let me = Arc::clone(self);
            space.add_method(
                &PUBLISH_SUBSCRIBE,
                SET_SECURITY_KEYS,
                qname("SetSecurityKeys"),
                &[
                    Argument::scalar("SecurityGroupId", string.clone()),
                    Argument::scalar("SecurityPolicyUri", string),
                    Argument::scalar("CurrentTokenId", integer_id),
                    Argument::scalar("CurrentKey", byte_string.clone()),
                    Argument::array("FutureKeys", byte_string),
                    Argument::scalar("TimeToNextKey", duration.clone()),
                    Argument::scalar("KeyLifetime", duration),
                ],
                &[],
                move |context, args| me.set_security_keys(context, args, Instant::now()),
            );
        }
    }

    /// The Methods of a SecurityGroup folder (Part 14 §8.5.2 to §8.5.5),
    /// with the NodeIds `ids` (Add, Remove, AddFolder, RemoveFolder).
    fn install_folder(
        self: &Arc<Self>,
        space: &mut AddressSpace,
        folder: &NodeId,
        ids: [NodeId; 4],
    ) {
        let qname = |name: &str| QualifiedName {
            namespace_index: 0,
            name: name.into(),
        };
        let string = NodeId::numeric(0, 12);
        let uint32 = NodeId::numeric(0, 7);
        let duration = NodeId::numeric(0, 290);
        let node_id = NodeId::numeric(0, 17);
        let [add, remove, add_folder, remove_folder] = ids;
        let (me, at) = (Arc::clone(self), folder.clone());
        space.add_method_mut(
            folder,
            add,
            qname("AddSecurityGroup"),
            &[
                Argument::scalar("SecurityGroupName", string.clone()),
                Argument::scalar("KeyLifetime", duration),
                Argument::scalar("SecurityPolicyUri", string.clone()),
                Argument::scalar("MaxFutureKeyCount", uint32.clone()),
                Argument::scalar("MaxPastKeyCount", uint32),
            ],
            &[
                Argument::scalar("SecurityGroupId", string.clone()),
                Argument::scalar("SecurityGroupNodeId", node_id.clone()),
            ],
            move |space, context, args| me.add_security_group(space, &at, context, args),
        );
        let (me, at) = (Arc::clone(self), folder.clone());
        space.add_method_mut(
            folder,
            remove,
            qname("RemoveSecurityGroup"),
            &[Argument::scalar("SecurityGroupNodeId", node_id.clone())],
            &[],
            move |space, context, args| me.remove_security_group(space, &at, context, args),
        );
        let (me, at) = (Arc::clone(self), folder.clone());
        space.add_method_mut(
            folder,
            add_folder,
            qname("AddSecurityGroupFolder"),
            &[Argument::scalar("Name", string)],
            &[Argument::scalar(
                "SecurityGroupFolderNodeId",
                node_id.clone(),
            )],
            move |space, context, args| me.add_folder(space, &at, context, args),
        );
        let (me, at) = (Arc::clone(self), folder.clone());
        space.add_method_mut(
            folder,
            remove_folder,
            qname("RemoveSecurityGroupFolder"),
            &[Argument::scalar("SecurityGroupFolderNodeId", node_id)],
            &[],
            move |space, context, args| me.remove_folder(space, &at, context, args),
        );
    }

    fn install_group(self: &Arc<Self>, space: &mut AddressSpace, group: &LocalGroup) {
        let ns = group.node.namespace_index;
        let text = |name: &str| LocalizedText {
            locale: None,
            text: Some(name.into()),
        };
        space.add_node(
            NodeMeta::new(
                group.node.clone(),
                NodeClass::Object,
                QualifiedName {
                    namespace_index: ns,
                    name: group.config.security_group_id.clone(),
                },
            )
            .with_display_name(text(&group.config.security_group_id))
            .with_type_definition(SECURITY_GROUP_TYPE),
        );
        space.add_reference(
            group.folder.clone(),
            reference_types::HAS_COMPONENT,
            group.node.clone(),
        );
        space.add_reference(
            group.node.clone(),
            reference_types::HAS_TYPE_DEFINITION,
            SECURITY_GROUP_TYPE,
        );
        // Part 14 §8.4.1 Table 213: the five Properties.
        let properties = [
            (
                "SecurityGroupId",
                VariantValue::String(group.config.security_group_id.clone()),
            ),
            (
                "KeyLifetime",
                VariantValue::Double(group.config.key_lifetime_ms as f64),
            ),
            (
                "SecurityPolicyUri",
                VariantValue::String(group.policy.uri().to_string()),
            ),
            (
                "MaxFutureKeyCount",
                VariantValue::UInt32(group.config.max_future_key_count),
            ),
            (
                "MaxPastKeyCount",
                VariantValue::UInt32(group.config.max_past_key_count),
            ),
        ];
        for (name, value) in properties {
            let id = NodeId::string(
                ns,
                std::format!("{}/{name}", group.node.to_canonical_string()),
            )
            .unwrap_or_else(|_| NodeId::numeric(ns, 0));
            space.add_node(
                NodeMeta::new(
                    id.clone(),
                    NodeClass::Variable,
                    QualifiedName {
                        namespace_index: 0,
                        name: name.into(),
                    },
                )
                .with_display_name(text(name))
                .with_type_definition(NodeId::numeric(0, 68)),
            );
            space.set_value(
                id.clone(),
                DataValue::new_value(Variant::scalar(value), 0, 0),
            );
            space.add_reference(group.node.clone(), reference_types::HAS_PROPERTY, id);
        }
        for (name, invalidate) in [("InvalidateKeys", true), ("ForceKeyRotation", false)] {
            let id = NodeId::string(
                ns,
                std::format!("{}/{name}", group.node.to_canonical_string()),
            )
            .unwrap_or_else(|_| NodeId::numeric(ns, 0));
            let me = Arc::clone(self);
            let group_id = group.config.security_group_id.clone();
            space.add_method(
                &group.node,
                id,
                QualifiedName {
                    namespace_index: 0,
                    name: name.into(),
                },
                &[],
                &[],
                move |context, _| me.administer(&group_id, invalidate, context, Instant::now()),
            );
        }
    }

    /// `GetSecurityKeys` (Part 14 §8.3.2).
    fn get_security_keys(
        &self,
        context: &MethodContext,
        args: &[Variant],
        now: Instant,
    ) -> MethodOutcome {
        // "Encryption is required for this Method. The Method shall return
        // Bad_SecurityModeInsufficient if the communication is not
        // encrypted."
        if !context.encrypted {
            return MethodOutcome::fault(status::BAD_SECURITY_MODE_INSUFFICIENT);
        }
        let Some((id, start, count)) = parse_get_security_keys_input(args) else {
            return MethodOutcome::fault(status::BAD_INVALID_ARGUMENT);
        };
        let Ok(groups) = self.groups.lock() else {
            return MethodOutcome::fault(status::BAD_INVALID_STATE);
        };
        let Some(group) = groups.iter().find(|g| g.config.security_group_id == id) else {
            return MethodOutcome::fault(status::BAD_NOT_FOUND);
        };
        // Part 14 §8.8: the role SecurityKeyServerAccess, or the users of
        // the group's own permission.
        let subject = super::security::client_subject(&context.user);
        if !group.config.key_subjects.contains(&subject) && !self.roles.access.contains(&subject) {
            return MethodOutcome::fault(status::BAD_USER_ACCESS_DENIED);
        }
        let Ok(mut state) = group.keys.lock() else {
            return MethodOutcome::fault(status::BAD_INVALID_STATE);
        };
        group.catch_up(&mut state, now);
        let time_to_next = state.time_to_next_key(now);
        let Some(service) = state.service.as_mut() else {
            return MethodOutcome::fault(status::BAD_INVALID_STATE);
        };
        service.set_time_to_next_key_ms(time_to_next.as_secs_f64() * 1000.0);
        // "If the caller requests a number larger than the Security Key
        // Service permits, then the SKS shall return the maximum it allows."
        let count = count.min(group.config.max_future_key_count.saturating_add(1));
        match service.get_security_keys_for(&id, start, count) {
            Some(result) => MethodOutcome::good(result.to_output_arguments()),
            None => MethodOutcome::fault(status::BAD_NOT_FOUND),
        }
    }

    /// `GetSecurityGroup` (Part 14 §8.3.3).
    fn get_security_group(&self, args: &[Variant]) -> MethodOutcome {
        let id = match args {
            [v] if v.is_scalar() => match v.value.first() {
                Some(VariantValue::String(id)) => id.clone(),
                _ => return MethodOutcome::fault(status::BAD_INVALID_ARGUMENT),
            },
            _ => return MethodOutcome::fault(status::BAD_INVALID_ARGUMENT),
        };
        let Ok(groups) = self.groups.lock() else {
            return MethodOutcome::fault(status::BAD_INVALID_STATE);
        };
        match groups.iter().find(|g| g.config.security_group_id == id) {
            Some(group) => MethodOutcome::good(std::vec![Variant::scalar(VariantValue::NodeId(
                group.node.clone()
            ))]),
            None => MethodOutcome::fault(status::BAD_NO_MATCH),
        }
    }

    /// `InvalidateKeys` / `ForceKeyRotation` (Part 14 §8.4.2, §8.4.3): "The
    /// Client shall be authorized to modify the configuration for the SKS
    /// functionality and shall use at least a signed communication
    /// channel".
    fn administer(
        &self,
        id: &str,
        invalidate: bool,
        context: &MethodContext,
        now: Instant,
    ) -> MethodOutcome {
        if !context.signed {
            return MethodOutcome::fault(status::BAD_SECURITY_MODE_INSUFFICIENT);
        }
        let Ok(groups) = self.groups.lock() else {
            return MethodOutcome::fault(status::BAD_INVALID_STATE);
        };
        let Some(group) = groups.iter().find(|g| g.config.security_group_id == id) else {
            return MethodOutcome::fault(status::BAD_INVALID_STATE);
        };
        let subject = super::security::client_subject(&context.user);
        if !group.config.admin_subjects.contains(&subject) && !self.roles.admin.contains(&subject) {
            return MethodOutcome::fault(status::BAD_USER_ACCESS_DENIED);
        }
        if invalidate {
            group.invalidate(now);
        } else {
            group.rotate_now(now);
        }
        daemon_log!(
            Level::Info,
            "security group `{}`: keys {} by `{subject}`",
            group.config.security_group_id,
            if invalidate { "invalidated" } else { "rotated" }
        );
        MethodOutcome::good(Vec::new())
    }

    /// `SetSecurityKeys` (Part 14 §9.1.3.3): an SKS pushes the keys of a
    /// SecurityGroup; encryption is required as for `GetSecurityKeys`.
    fn set_security_keys(
        &self,
        context: &MethodContext,
        args: &[Variant],
        now: Instant,
    ) -> MethodOutcome {
        if !context.encrypted {
            return MethodOutcome::fault(status::BAD_SECURITY_MODE_INSUFFICIENT);
        }
        let Ok(input) = SetSecurityKeysInput::from_arguments(args) else {
            return MethodOutcome::fault(status::BAD_INVALID_ARGUMENT);
        };
        let Ok((policy, keys)) = input.security_keys() else {
            return MethodOutcome::fault(status::BAD_INVALID_ARGUMENT);
        };
        let subject = super::security::client_subject(&context.user);
        let Ok(targets) = self.push_targets.lock() else {
            return MethodOutcome::fault(status::BAD_INVALID_STATE);
        };
        let mut known = false;
        let mut allowed = false;
        for target in targets
            .iter()
            .filter(|t| t.security_group_id == input.security_group_id)
        {
            known = true;
            if target.subject != subject && !self.roles.push.contains(&subject) {
                continue;
            }
            allowed = true;
            if let Ok(mut state) = target.keys.lock() {
                state.apply(
                    &input.security_group_id,
                    policy,
                    input.current_token_id,
                    keys.clone(),
                    Duration::from_secs_f64(input.time_to_next_key_ms.max(0.0) / 1000.0),
                    Duration::from_secs_f64(input.key_lifetime_ms.max(0.0) / 1000.0),
                    now,
                );
            }
        }
        if !known {
            return MethodOutcome::fault(status::BAD_NOT_FOUND);
        }
        if !allowed {
            return MethodOutcome::fault(status::BAD_USER_ACCESS_DENIED);
        }
        MethodOutcome::good(Vec::new())
    }

    /// Whether the session may manage the SKS (Part 14 §8.8
    /// `SecurityKeyServerAdmin`).
    pub(crate) fn is_admin(&self, context: &MethodContext) -> bool {
        self.roles
            .admin
            .contains(&super::security::client_subject(&context.user))
    }

    /// `AddSecurityGroup` (Part 14 §8.5.2) on the folder `folder`.
    fn add_security_group(
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
            Some(VariantValue::String(name)),
            Some(VariantValue::Double(lifetime)),
            Some(VariantValue::String(policy_uri)),
            Some(VariantValue::UInt32(future)),
            Some(VariantValue::UInt32(past)),
        ) = (scalar(0), scalar(1), scalar(2), scalar(3), scalar(4))
        else {
            return MethodOutcome::fault(status::BAD_INVALID_ARGUMENT);
        };
        if args.len() != 5 || name.is_empty() || name.len() > MAX_NAME_LEN || name.contains('/') {
            return MethodOutcome::fault(status::BAD_INVALID_ARGUMENT);
        }
        // "If a null or empty String is passed in, the SKS sets the default
        // SecurityPolicyUri. If the SecurityPolicyUri is not known to the
        // SKS, Bad_InvalidArgument shall be returned."
        let policy = if policy_uri.is_empty() {
            SUPPORTED_POLICIES[0]
        } else {
            match SUPPORTED_POLICIES.iter().find(|p| p.uri() == policy_uri) {
                Some(policy) => *policy,
                None => return MethodOutcome::fault(status::BAD_INVALID_ARGUMENT),
            }
        };
        // "If 0 is passed in, the SKS sets the default"; a value past the
        // limits of the SKS is adjusted.
        let lifetime_ms = if *lifetime <= 0.0 || !lifetime.is_finite() {
            DEFAULT_KEY_LIFETIME_MS
        } else {
            (*lifetime as u64).clamp(MIN_KEY_LIFETIME_MS, MAX_KEY_LIFETIME_MS)
        };
        let future = if *future == 0 {
            DEFAULT_FUTURE_KEYS
        } else {
            (*future).min(MAX_KEY_COUNT)
        };
        let past = (*past).min(MAX_KEY_COUNT);
        let Ok(mut groups) = self.groups.lock() else {
            return MethodOutcome::fault(status::BAD_INVALID_STATE);
        };
        let output = |id: &str, node: &NodeId| {
            std::vec![
                Variant::scalar(VariantValue::String(id.to_string())),
                Variant::scalar(VariantValue::NodeId(node.clone())),
            ]
        };
        if let Some(existing) = groups.iter().find(|g| g.config.security_group_id == *name) {
            let same = existing.policy == policy
                && existing.config.key_lifetime_ms == lifetime_ms
                && existing.config.max_future_key_count == future
                && existing.config.max_past_key_count == past
                && existing.folder == *folder;
            if !same {
                return MethodOutcome::fault(BAD_NODE_ID_EXISTS);
            }
            return MethodOutcome {
                status_code: GOOD_DATA_IGNORED,
                output_arguments: output(name, &existing.node),
            };
        }
        let config = SecurityGroupConfig {
            security_group_id: name.clone(),
            policy: match policy {
                SecurityPolicy::Aes128Ctr => PubSubSecurityPolicy::Aes128Ctr,
                SecurityPolicy::Aes256Gcm => PubSubSecurityPolicy::Aes256Gcm,
                _ => PubSubSecurityPolicy::Aes256Ctr,
            },
            key_lifetime_ms: lifetime_ms,
            max_future_key_count: future,
            max_past_key_count: past,
            key_subjects: Vec::new(),
            admin_subjects: Vec::new(),
        };
        let mut group = LocalGroup::with_policy(config, policy, self.ns, Instant::now());
        group.folder = folder.clone();
        let result = output(name, &group.node);
        self.install_group(space, &group);
        daemon_log!(
            Level::Info,
            "security group `{name}` added by `{}`",
            super::security::client_subject(&context.user)
        );
        groups.push(group);
        MethodOutcome::good(result)
    }

    /// `RemoveSecurityGroup` (Part 14 §8.5.3) on the folder `folder`.
    fn remove_security_group(
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
        let Some(node) = node_id_argument(args) else {
            return MethodOutcome::fault(status::BAD_INVALID_ARGUMENT);
        };
        let Ok(mut groups) = self.groups.lock() else {
            return MethodOutcome::fault(status::BAD_INVALID_STATE);
        };
        let Some(index) = groups.iter().position(|g| g.node == node) else {
            return MethodOutcome::fault(if space.node_meta(&node).is_some() {
                BAD_NODE_ID_INVALID
            } else {
                status::BAD_NODE_ID_UNKNOWN
            });
        };
        if groups[index].folder != *folder {
            return MethodOutcome::fault(BAD_NODE_ID_INVALID);
        }
        let group = groups.remove(index);
        space.remove_subtree(&group.node);
        self.push.forget_group(&group.config.security_group_id);
        daemon_log!(
            Level::Info,
            "security group `{}` removed by `{}`",
            group.config.security_group_id,
            super::security::client_subject(&context.user)
        );
        MethodOutcome::good(Vec::new())
    }

    /// `AddSecurityGroupFolder` (Part 14 §8.5.4) on the folder `folder`.
    fn add_folder(
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
        let Ok(mut folders) = self.folders.lock() else {
            return MethodOutcome::fault(status::BAD_INVALID_STATE);
        };
        let parent_path = folders
            .iter()
            .find(|f| f.node == *folder)
            .map_or(String::new(), |f| f.path.clone());
        let path = std::format!("{parent_path}/{name}");
        if folders.iter().any(|f| f.path == path) {
            return MethodOutcome::fault(BAD_BROWSE_NAME_DUPLICATED);
        }
        let Ok(node) = NodeId::string(self.ns, std::format!("SecurityGroupFolder:{path}")) else {
            return MethodOutcome::fault(status::BAD_INVALID_ARGUMENT);
        };
        space.add_node(
            NodeMeta::new(
                node.clone(),
                NodeClass::Object,
                QualifiedName {
                    namespace_index: self.ns,
                    name: name.clone(),
                },
            )
            .with_type_definition(SECURITY_GROUP_FOLDER_TYPE),
        );
        space.add_reference(folder.clone(), reference_types::ORGANIZES, node.clone());
        space.add_reference(
            node.clone(),
            reference_types::HAS_TYPE_DEFINITION,
            SECURITY_GROUP_FOLDER_TYPE,
        );
        let method = |what: &str| {
            NodeId::string(self.ns, std::format!("SecurityGroupFolder:{path}/{what}"))
                .unwrap_or_else(|_| NodeId::numeric(self.ns, 0))
        };
        self.install_folder(
            space,
            &node,
            [
                method("AddSecurityGroup"),
                method("RemoveSecurityGroup"),
                method("AddSecurityGroupFolder"),
                method("RemoveSecurityGroupFolder"),
            ],
        );
        folders.push(Folder {
            node: node.clone(),
            path,
        });
        MethodOutcome::good(std::vec![Variant::scalar(VariantValue::NodeId(node))])
    }

    /// `RemoveSecurityGroupFolder` (Part 14 §8.5.5) on the folder
    /// `folder`: the folder, the folders in it and their SecurityGroups.
    fn remove_folder(
        &self,
        space: &mut AddressSpace,
        folder: &NodeId,
        context: &MethodContext,
        args: &[Variant],
    ) -> MethodOutcome {
        if !self.is_admin(context) {
            return MethodOutcome::fault(status::BAD_USER_ACCESS_DENIED);
        }
        let Some(node) = node_id_argument(args) else {
            return MethodOutcome::fault(status::BAD_INVALID_ARGUMENT);
        };
        let Ok(mut folders) = self.folders.lock() else {
            return MethodOutcome::fault(status::BAD_INVALID_STATE);
        };
        let parent_path = folders
            .iter()
            .find(|f| f.node == *folder)
            .map_or(String::new(), |f| f.path.clone());
        // A folder of this folder: its path is the parent's and one name.
        let Some(path) = folders
            .iter()
            .find(|f| {
                f.node == node
                    && f.path
                        .strip_prefix(parent_path.as_str())
                        .is_some_and(|rest| rest.starts_with('/') && !rest[1..].contains('/'))
            })
            .map(|f| f.path.clone())
        else {
            return MethodOutcome::fault(status::BAD_NODE_ID_UNKNOWN);
        };
        let removed = space.remove_subtree(&node);
        folders.retain(|f| f.path != path && !f.path.starts_with(&std::format!("{path}/")));
        if let Ok(mut groups) = self.groups.lock() {
            for group in groups.iter().filter(|g| removed.contains(&g.node)) {
                self.push.forget_group(&group.config.security_group_id);
            }
            groups.retain(|g| !removed.contains(&g.node));
        }
        MethodOutcome::good(Vec::new())
    }
}

/// The name argument of `AddSecurityGroupFolder` (Part 14 §8.5.4) and
/// `AddPushTargetFolder` (§8.7.4): one String, not empty, at most
/// `MAX_NAME_LEN` octets, without `/`; `Bad_InvalidArgument` otherwise.
pub(super) fn folder_name(args: &[Variant]) -> Result<String, MethodOutcome> {
    let name = match args {
        [v] if v.is_scalar() => match v.value.first() {
            Some(VariantValue::String(name)) => name.clone(),
            _ => return Err(MethodOutcome::fault(status::BAD_INVALID_ARGUMENT)),
        },
        _ => return Err(MethodOutcome::fault(status::BAD_INVALID_ARGUMENT)),
    };
    if name.is_empty() || name.len() > MAX_NAME_LEN || name.contains('/') {
        return Err(MethodOutcome::fault(status::BAD_INVALID_ARGUMENT));
    }
    Ok(name)
}

/// The single NodeId argument of a Method.
fn node_id_argument(args: &[Variant]) -> Option<NodeId> {
    match args {
        [v] if v.is_scalar() => match v.value.first() {
            Some(VariantValue::NodeId(node)) => Some(node.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// Pulls the keys of a SecurityGroup from an SKS into `keys` (Part 14
/// §5.4.5.3, §8.3.2): calls `GetSecurityKeys` with `StartingTokenId` 0
/// over an encrypted session, again at half the key lifetime, after a
/// failure every [`PULL_RETRY`], and at once when a subscriber met a
/// token it has no key for. Runs until `stop`.
pub fn spawn_puller(
    config: SksClientConfig,
    security_group_id: String,
    keys: SharedKeys,
    stop: Arc<std::sync::atomic::AtomicBool>,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("zerodds-opcua-sks-pull".into())
        .spawn(move || {
            let mut next = Instant::now();
            while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                let now = Instant::now();
                let unknown = keys.lock().ok().and_then(|s| s.unknown_token).is_some();
                if now >= next || unknown {
                    next = match pull_once(&config, &security_group_id, &keys, now) {
                        Ok(lifetime) => now + lifetime / 2,
                        Err(e) => {
                            daemon_log!(
                                Level::Warn,
                                "security group `{security_group_id}`: GetSecurityKeys at `{}`: {e}",
                                config.endpoint_url
                            );
                            // A failing call is not repeated at once for a
                            // token that stays unknown.
                            if let Ok(mut state) = keys.lock() {
                                state.unknown_token = None;
                            }
                            now + PULL_RETRY
                        }
                    };
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        })
}

/// One `GetSecurityKeys` call; the key lifetime it gave.
fn pull_once(
    config: &SksClientConfig,
    security_group_id: &str,
    keys: &SharedKeys,
    now: Instant,
) -> Result<Duration, String> {
    use zerodds_opcua_server::client::{Client, ClientSecurity, TcpTransport};
    let policy = match config.channel_policy.as_str() {
        "aes128_sha256_rsaoaep" => zerodds_opcua_uacp::crypto::SecurityPolicy::Aes128Sha256RsaOaep,
        "aes256_sha256_rsapss" => zerodds_opcua_uacp::crypto::SecurityPolicy::Aes256Sha256RsaPss,
        _ => zerodds_opcua_uacp::crypto::SecurityPolicy::Basic256Sha256,
    };
    let private_key = super::security::load_private_key_pem(&config.private_key_pem)
        .map_err(|e| e.to_string())?;
    let certificate =
        super::security::load_der(&config.certificate_der).map_err(|e| e.to_string())?;
    let server_certificate =
        super::security::load_der(&config.server_certificate_der).map_err(|e| e.to_string())?;
    let server_public_key = super::security::load_public_key_pem(&config.server_public_key_pem)
        .map_err(|e| e.to_string())?;
    let mut client = Client::new();
    client.set_security(ClientSecurity {
        policy,
        mode: zerodds_opcua_uacp::crypto::SecuredMode::SignAndEncrypt,
        private_key,
        certificate,
        server_certificate,
        server_public_key,
        rng: std::boxed::Box::new(rand::rngs::OsRng),
    });
    if let (Some(user), Some(password)) = (&config.username, &config.password) {
        client.set_identity(
            zerodds_opcua_server::identity::UserIdentityToken::user_name(
                &config.user_token_policy_id,
                user,
                password,
            ),
        );
    }
    let address = config
        .endpoint_url
        .strip_prefix("opc.tcp://")
        .unwrap_or(&config.endpoint_url);
    let address = address.split('/').next().unwrap_or(address);
    let mut transport = TcpTransport::connect(address).map_err(|e| e.to_string())?;
    client
        .connect(&mut transport, &config.endpoint_url)
        .map_err(|e| e.to_string())?;
    let result = client
        .call_method(
            &mut transport,
            PUBLISH_SUBSCRIBE,
            GET_SECURITY_KEYS,
            get_security_keys_input(security_group_id, 0, config.requested_key_count),
        )
        .map_err(|e| e.to_string());
    let _ = client.close_session(&mut transport, false);
    let result = result?;
    if result.status_code != status::GOOD {
        return Err(std::format!("status 0x{:08X}", result.status_code));
    }
    let fetched = GetSecurityKeysResult::from_output_arguments(&result.output_arguments)
        .map_err(|e| e.to_string())?;
    let (policy, fetched_keys) = fetched.security_keys().map_err(|e| e.to_string())?;
    let lifetime = Duration::from_secs_f64(fetched.key_lifetime_ms.max(1.0) / 1000.0);
    // `TimeToNextKey` counts from the answer, not from the start of the
    // call: the handshake before it takes its time.
    let now = Instant::now().max(now);
    let mut state = keys.lock().map_err(|_| "key state poisoned".to_string())?;
    state.apply(
        security_group_id,
        policy,
        fetched.first_token_id,
        fetched_keys,
        Duration::from_secs_f64(fetched.time_to_next_key_ms.max(0.0) / 1000.0),
        lifetime,
        now,
    );
    Ok(lifetime)
}
