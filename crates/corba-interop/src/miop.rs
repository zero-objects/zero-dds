// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::missing_panics_doc)]

//! MIOP in the ORB runtime — CORBA 3.3 Part 2 §11.2.4, §11.2.5, §11.3.
//!
//! * [`MiopClient`] sends oneway requests to the group of a group
//!   reference over its UIPMC profile (§11.3.1): a GIOP 1.2 request
//!   names the UIPMC profile in its target; GIOP 1.0 and 1.1 put
//!   `"MIOP"` and the profile encapsulation into the object key
//!   (§11.3.1.2). Two-way calls go to the IIOP profile of the
//!   `TAG_GROUP_IIOP` component or of the gateway ([`two_way_reference`]).
//! * [`Goa`] is `PortableGroup::GOA` (§11.2.4): it maps a group reference
//!   to object ids, joins the multicast endpoint of the reference, and
//!   dispatches every request a group receives to each servant activated
//!   with an object id associated with that group.
//! * [`MiopGateway`] is the MIOP gateway of §11.2.5: an IIOP server whose
//!   clients address a group by its UIPMC profile; it forwards their
//!   requests to the group.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Cursor;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use zerodds_corba_giop::object_ops::is_object_operation;
use zerodds_corba_giop::{
    LocateReply, LocateStatusType, Message, Reply, ReplyStatusType, Request, ResponseFlags,
    ServiceContextList, TargetAddress, TargetProfile, Version, encode_message,
};
use zerodds_corba_iiop::miop::{
    MiopReceiver, MiopSender, MulticastInterface, ReassemblyStats, Received,
};
use zerodds_corba_iiop::{Acceptor, AcceptorConfig, IiopError, IiopProfileBody};
use zerodds_corba_ior::miop::{GroupInfo, UipmcProfileBody, group_profiles};
use zerodds_corba_ior::{Ior, ProfileId};
use zerodds_corba_ir::Repository;
use zerodds_corba_rust::{
    CompletionStatus, CorbaException, ObjectReference, SkeletonResult, SystemExceptionKind,
};
use zerodds_wire::Endianness;

use crate::runtime::Dispatcher;

/// The prefix of an object key that carries a UIPMC profile in GIOP 1.0
/// and 1.1 (§11.3.1.2).
pub const MIOP_OBJECT_KEY_PREFIX: &[u8; 4] = b"MIOP";

/// `PortableGroup::NotAGroupObject` (§11.2.4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotAGroupObject;

impl core::fmt::Display for NotAGroupObject {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("the reference is not a group reference")
    }
}

impl std::error::Error for NotAGroupObject {}

/// The identity of an object group for dispatch: its domain and id (the
/// reference version may differ between references to one group).
type GroupKey = (String, u64);

fn group_key(g: &GroupInfo) -> GroupKey {
    (g.group_domain_id.clone(), g.object_group_id)
}

/// The UIPMC profile and group of a group reference (the first UIPMC
/// profile with a `TAG_GROUP` component).
fn uipmc_group(ior: &Ior) -> Result<(UipmcProfileBody, GroupInfo), NotAGroupObject> {
    let profiles = group_profiles(ior).map_err(|_| NotAGroupObject)?;
    profiles
        .uipmc
        .into_iter()
        .find_map(|u| match u.group_info() {
            Ok(Some(g)) => Some((u, g)),
            _ => None,
        })
        .ok_or(NotAGroupObject)
}

/// The multicast endpoint of a UIPMC profile; an alias is resolved and
/// must name a multicast address (§11.2.2.2).
fn endpoint(u: &UipmcProfileBody) -> Result<SocketAddr, CorbaException> {
    let inv = || CorbaException::system(SystemExceptionKind::InvObjref, 0, CompletionStatus::No);
    let addr = match u.the_address.parse::<IpAddr>() {
        Ok(ip) => SocketAddr::new(ip, u.the_port),
        Err(_) => (u.the_address.as_str(), u.the_port)
            .to_socket_addrs()
            .map_err(|_| inv())?
            .find(|a| a.ip().is_multicast())
            .ok_or_else(inv)?,
    };
    if addr.ip().is_multicast() {
        Ok(addr)
    } else {
        Err(inv())
    }
}

/// The reference for two-way calls and the operations of
/// `CORBA::Object` on a group (§11.2.6.10, §11.3.1): the IIOP profile of
/// the `TAG_GROUP_IIOP` component, else the first IIOP profile of the
/// reference (a gateway); `None` when there is neither.
#[must_use]
pub fn two_way_reference(group: &Ior) -> Option<ObjectReference> {
    let profiles = group_profiles(group).ok()?;
    let body = profiles
        .uipmc
        .iter()
        .find_map(|u| u.group_iiop_profile().ok().flatten())
        .or_else(|| profiles.iiop.first().cloned())?;
    Some(ObjectReference {
        type_id: group.type_id.clone(),
        iiop_profile: body.encode_encapsulation(Endianness::Big).ok()?,
        policy_overrides: Vec::new(),
        profiles: Vec::new(),
    })
}

/// The reference of the IIOP profile in the `TAG_GROUP_IIOP` component
/// of a UIPMC profile of `group` (§11.2.2.4.2), with the type id of the
/// group.
fn group_iiop_reference(group: &Ior) -> Option<ObjectReference> {
    let body = group_profiles(group)
        .ok()?
        .uipmc
        .iter()
        .find_map(|u| u.group_iiop_profile().ok().flatten())?;
    Some(ObjectReference {
        type_id: group.type_id.clone(),
        iiop_profile: body.encode_encapsulation(Endianness::Big).ok()?,
        policy_overrides: Vec::new(),
        profiles: Vec::new(),
    })
}

/// The object key of a gateway's IIOP profile in a group reference
/// (§11.2.3.1, §11.3.1.2): `"MIOP"` and the encapsulation of the UIPMC
/// profile, so that a request with this key names the group.
///
/// # Errors
/// `MARSHAL` when the profile does not encode.
pub fn gateway_object_key(profile: &UipmcProfileBody) -> Result<Vec<u8>, CorbaException> {
    let mut key = MIOP_OBJECT_KEY_PREFIX.to_vec();
    key.extend_from_slice(&profile.encode_encapsulation(Endianness::Big).map_err(|_| {
        CorbaException::system(SystemExceptionKind::Marshal, 0, CompletionStatus::No)
    })?);
    Ok(key)
}

fn uipmc_of_key(key: &[u8]) -> Option<UipmcProfileBody> {
    key.strip_prefix(MIOP_OBJECT_KEY_PREFIX.as_slice())
        .and_then(|e| UipmcProfileBody::decode_encapsulation(e).ok())
}

/// The UIPMC profile of a tagged profile: the profile itself, or the one
/// in the `"MIOP"` object key of an IIOP profile.
fn uipmc_of_profile(p: &TargetProfile) -> Option<UipmcProfileBody> {
    if p.tag == ProfileId::Uipmc.as_u32() {
        UipmcProfileBody::decode_encapsulation(&p.profile_data).ok()
    } else if p.tag == ProfileId::InternetIop.as_u32() {
        IiopProfileBody::decode_encapsulation(&p.profile_data)
            .ok()
            .and_then(|b| uipmc_of_key(&b.object_key))
    } else {
        None
    }
}

/// The UIPMC profile a request names (§11.3.1): the UIPMC profile of a
/// GIOP 1.2 target, the encapsulation behind `"MIOP"` in an object key
/// (GIOP 1.0 and 1.1, or the key of a gateway profile), or for a
/// reference target the selected profile, else the first UIPMC profile of
/// the reference.
fn target_uipmc(target: &TargetAddress) -> Option<UipmcProfileBody> {
    match target {
        TargetAddress::Profile(p) => uipmc_of_profile(p),
        TargetAddress::Reference(r) => {
            r.selected_profile().and_then(uipmc_of_profile).or_else(|| {
                r.profiles
                    .iter()
                    .find(|p| p.tag == ProfileId::Uipmc.as_u32())
                    .and_then(uipmc_of_profile)
            })
        }
        TargetAddress::Key(k) => uipmc_of_key(k),
    }
}

/// Sends requests to object groups (§11.3.1).
#[derive(Debug)]
pub struct MiopClient {
    ttl: u32,
    max_datagram: usize,
    giop_version: Version,
    senders: Mutex<HashMap<bool, Arc<MiopSender>>>,
    next_request_id: AtomicU32,
    repository: Option<Arc<Repository>>,
}

impl Default for MiopClient {
    fn default() -> Self {
        Self::new()
    }
}

impl MiopClient {
    /// A client that sends GIOP 1.2 requests in datagrams of at most 1472
    /// octets with a time-to-live of 1.
    #[must_use]
    pub fn new() -> Self {
        Self {
            ttl: 1,
            max_datagram: zerodds_corba_iiop::miop::DEFAULT_MAX_DATAGRAM,
            giop_version: Version::V1_2,
            senders: Mutex::new(HashMap::new()),
            next_request_id: AtomicU32::new(1),
            repository: None,
        }
    }

    /// The interface repository `is_a` resolves interfaces in when a group
    /// reference has no `TAG_GROUP_IIOP` component (§11.2.6.10).
    #[must_use]
    pub fn with_interface_repository(mut self, repository: Arc<Repository>) -> Self {
        self.repository = Some(repository);
        self
    }

    /// The time-to-live of the datagrams (§11.1.6.1).
    #[must_use]
    pub fn with_ttl(mut self, ttl: u32) -> Self {
        self.ttl = ttl;
        self
    }

    /// The largest datagram.
    #[must_use]
    pub fn with_max_datagram(mut self, max_datagram: usize) -> Self {
        self.max_datagram = max_datagram;
        self
    }

    /// The GIOP version of the requests: 1.2 names the UIPMC profile in
    /// the target, 1.0 and 1.1 in the object key (§11.3.1.2).
    #[must_use]
    pub fn with_giop_version(mut self, version: Version) -> Self {
        self.giop_version = version;
        self
    }

    fn sender(&self, ipv4: bool, family: IpAddr) -> Result<Arc<MiopSender>, CorbaException> {
        let mut g = self
            .senders
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(s) = g.get(&ipv4) {
            return Ok(Arc::clone(s));
        }
        let s = Arc::new(
            MiopSender::new(family, self.ttl)
                .map_err(|_| {
                    CorbaException::system(SystemExceptionKind::Transient, 0, CompletionStatus::No)
                })?
                .with_max_datagram(self.max_datagram),
        );
        g.insert(ipv4, Arc::clone(&s));
        Ok(s)
    }

    /// The GIOP request `operation` to `profile` as MIOP sends it:
    /// oneway, the UIPMC profile in the target (GIOP 1.2) or in the
    /// object key (1.0, 1.1); `arguments` write the body at its offset in
    /// the message (Part 2 §9.4.2.1).
    ///
    /// # Errors
    /// `MARSHAL` when the profile does not encode; the error of
    /// `arguments`.
    pub fn request_message(
        &self,
        profile: &UipmcProfileBody,
        operation: &str,
        endianness: Endianness,
        arguments: &zerodds_corba_rust::MarshalArgs<'_>,
        service_context: ServiceContextList,
    ) -> Result<Vec<u8>, CorbaException> {
        let marshal =
            || CorbaException::system(SystemExceptionKind::Marshal, 0, CompletionStatus::No);
        let profile_data = profile
            .encode_encapsulation(endianness)
            .map_err(|_| marshal())?;
        let target = if self.giop_version.uses_v1_2_request_layout() {
            TargetAddress::Profile(TargetProfile {
                tag: ProfileId::Uipmc.as_u32(),
                profile_data,
            })
        } else {
            let mut key = MIOP_OBJECT_KEY_PREFIX.to_vec();
            key.extend_from_slice(&profile_data);
            TargetAddress::Key(key)
        };
        let mut request = Request {
            request_id: self.next_request_id.fetch_add(1, Ordering::Relaxed),
            response_flags: ResponseFlags::SYNC_NONE,
            target,
            operation: operation.to_string(),
            requesting_principal: None,
            service_context,
            body: Vec::new(),
        };
        let offset = request
            .body_offset(self.giop_version, endianness)
            .map_err(|_| marshal())?;
        let mut w = zerodds_corba_giop::body_writer(offset, endianness);
        arguments(&mut w)?;
        request.body = zerodds_corba_giop::finish_body(w, offset);
        encode_message(
            self.giop_version,
            endianness,
            false,
            &Message::Request(request),
        )
        .map_err(|_| marshal())
    }

    /// Sends the oneway request `operation` with the body `arguments`
    /// write ([`zerodds_corba_rust::MarshalArgs`]) to the group of
    /// `group` over its UIPMC profile (§11.3.3.4). Nothing tells whether
    /// any member received it (§11.3.2).
    ///
    /// # Errors
    /// `INV_OBJREF` for a reference without a UIPMC group profile or with
    /// an address that is not multicast; `MARSHAL`; `TRANSIENT` when the
    /// datagrams cannot be sent.
    pub fn send_oneway(
        &self,
        group: &Ior,
        operation: &str,
        endianness: Endianness,
        arguments: &zerodds_corba_rust::MarshalArgs<'_>,
    ) -> Result<(), CorbaException> {
        let (profile, _) = uipmc_group(group).map_err(|_| {
            CorbaException::system(SystemExceptionKind::InvObjref, 0, CompletionStatus::No)
        })?;
        let message = self.request_message(
            &profile,
            operation,
            endianness,
            arguments,
            ServiceContextList::default(),
        )?;
        self.send_message(&profile, &message)
    }

    /// `CORBA::Object::non_existent` of a group reference: always `true`
    /// (§11.2.6.10).
    #[must_use]
    pub fn non_existent(&self, _group: &Ior) -> bool {
        true
    }

    /// `CORBA::Object::validate_connection` of a group reference
    /// (§11.2.6.10): `true` when the reference has a UIPMC group profile
    /// whose endpoint is a multicast address this client can send to.
    #[must_use]
    pub fn validate_connection(&self, group: &Ior) -> bool {
        uipmc_group(group)
            .ok()
            .and_then(|(u, _)| endpoint(&u).ok())
            .is_some_and(|to| self.sender(to.is_ipv4(), to.ip()).is_ok())
    }

    /// `CORBA::Object::is_a` of a group reference (§11.2.6.10): `true`
    /// for the type id of the reference and for
    /// `IDL:omg.org/CORBA/Object:1.0`; otherwise the question goes as
    /// `_is_a` over the IIOP profile of the `TAG_GROUP_IIOP` component of
    /// the UIPMC profile through `connection`. A reference without that
    /// component is resolved in the interface repository of
    /// [`Self::with_interface_repository`]: `true` when the type of the
    /// reference inherits from `logical_type_id`, `false` only when the
    /// repository cannot resolve it.
    ///
    /// # Errors
    /// The exception of the `_is_a` call.
    pub fn is_a(
        &self,
        group: &Ior,
        logical_type_id: &str,
        connection: &crate::runtime::IiopCorbaConnection,
    ) -> Result<bool, CorbaException> {
        use zerodds_corba_rust::CorbaConnection;
        if (!group.type_id.is_empty() && group.type_id == logical_type_id)
            || logical_type_id == zerodds_corba_giop::object_ops::OBJECT_REPOSITORY_ID
        {
            return Ok(true);
        }
        let Some(target) = group_iiop_reference(group) else {
            return Ok(self
                .repository
                .as_ref()
                .and_then(|r| r.interface_is_a(&group.type_id, logical_type_id))
                .unwrap_or(false));
        };
        let (body, e) = connection.invoke(&target, "_is_a", Endianness::Big, &|w| {
            w.write_string(logical_type_id);
            Ok(())
        })?;
        zerodds_wire::Reader::new(&body, e)
            .read_bool()
            .map_err(|_| {
                CorbaException::system(SystemExceptionKind::Marshal, 0, CompletionStatus::Maybe)
            })
    }

    /// Sends a GIOP request message to the endpoint of `profile`.
    fn send_message(
        &self,
        profile: &UipmcProfileBody,
        message: &[u8],
    ) -> Result<(), CorbaException> {
        let to = endpoint(profile)?;
        let sender = self.sender(to.is_ipv4(), to.ip())?;
        sender.send(to, message).map(|_| ()).map_err(|_| {
            CorbaException::system(SystemExceptionKind::Transient, 0, CompletionStatus::No)
        })
    }
}

/// What a [`Goa`] counted besides what it dispatched: every received
/// request that reached no servant is in one of these counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GoaStats {
    /// Requests dispatched to a servant (one per servant).
    pub dispatched: u64,
    /// Collections that are no single GIOP request message.
    pub not_a_request: u64,
    /// Requests that expect a reply, which MIOP cannot carry.
    pub two_way: u64,
    /// Requests whose target names no UIPMC group profile.
    pub no_group: u64,
    /// Requests for a group no object id of this adapter is associated with.
    pub no_member: u64,
    /// Requests for an object id without an active servant.
    pub no_servant: u64,
    /// Servants that raised an exception, which cannot be returned.
    pub exceptions: u64,
    /// The packets and collections the receivers did not deliver.
    pub reassembly: ReassemblyStats,
}

#[derive(Default)]
struct GoaState {
    /// The object ids of each group.
    members: BTreeMap<GroupKey, BTreeSet<Vec<u8>>>,
    /// The servants by object id.
    servants: HashMap<Vec<u8>, Dispatcher>,
    /// The endpoints the adapter listens on.
    receivers: BTreeMap<SocketAddr, JoinHandle<()>>,
    stats: GoaStats,
}

/// `PortableGroup::GOA` (§11.2.4): object ids for group references and
/// the dispatch of MIOP requests to the servants of those ids.
pub struct Goa {
    state: Arc<Mutex<GoaState>>,
    stop: Arc<AtomicBool>,
    interface: MulticastInterface,
    collection_timeout: Duration,
    next_id: AtomicU64,
}

impl core::fmt::Debug for Goa {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Goa")
            .field("interface", &self.interface)
            .finish()
    }
}

impl Default for Goa {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Goa {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl Goa {
    /// An adapter that joins groups on the interface the system chooses
    /// and drops a collection still incomplete after two seconds.
    #[must_use]
    pub fn new() -> Self {
        Self::with_interface(MulticastInterface::Default, Duration::from_secs(2))
    }

    /// An adapter that joins groups on `interface` and drops incomplete
    /// collections after `collection_timeout` (§11.1.6.2).
    #[must_use]
    pub fn with_interface(interface: MulticastInterface, collection_timeout: Duration) -> Self {
        Self {
            state: Arc::new(Mutex::new(GoaState::default())),
            stop: Arc::new(AtomicBool::new(false)),
            interface,
            collection_timeout,
            next_id: AtomicU64::new(1),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, GoaState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `create_id_for_reference` (§11.2.4.1.1): a new unique object id,
    /// associated with the group of `the_ref` as by
    /// [`Self::associate_reference_with_id`] (§11.2.4.2).
    ///
    /// # Errors
    /// As [`Self::associate_reference_with_id`].
    pub fn create_id_for_reference(&self, the_ref: &Ior) -> Result<Vec<u8>, GoaError> {
        let n = self.next_id.fetch_add(1, Ordering::Relaxed);
        let oid = format!("goa-{n}").into_bytes();
        self.associate_reference_with_id(the_ref, &oid)?;
        Ok(oid)
    }

    /// `reference_to_ids` (§11.2.4.1.1): the object ids associated with
    /// the group of `the_ref`.
    ///
    /// # Errors
    /// [`GoaError::NotAGroupObject`].
    pub fn reference_to_ids(&self, the_ref: &Ior) -> Result<Vec<Vec<u8>>, GoaError> {
        let (_, group) = uipmc_group(the_ref)?;
        Ok(self
            .lock()
            .members
            .get(&group_key(&group))
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default())
    }

    /// `associate_reference_with_id` (§11.2.4.1.1): servants activated
    /// with `oid` receive the requests of the group of `ref`; the adapter
    /// joins the multicast endpoint of the reference (§11.3.4.1). A
    /// repeated association is ignored.
    ///
    /// # Errors
    /// [`GoaError::NotAGroupObject`]; [`GoaError::System`] `INV_OBJREF`
    /// for an address that is not multicast, `TRANSIENT` when the group
    /// cannot be joined.
    pub fn associate_reference_with_id(&self, r#ref: &Ior, oid: &[u8]) -> Result<(), GoaError> {
        let (profile, group) = uipmc_group(r#ref)?;
        let at = endpoint(&profile).map_err(GoaError::System)?;
        let mut g = self.lock();
        if let std::collections::btree_map::Entry::Vacant(slot) = g.receivers.entry(at) {
            let receiver =
                MiopReceiver::join(at.ip(), at.port(), self.interface, self.collection_timeout)
                    .map_err(|_| {
                        GoaError::System(CorbaException::system(
                            SystemExceptionKind::Transient,
                            0,
                            CompletionStatus::No,
                        ))
                    })?;
            slot.insert(spawn_receiver(
                receiver,
                Arc::clone(&self.state),
                Arc::clone(&self.stop),
            ));
        }
        g.members
            .entry(group_key(&group))
            .or_default()
            .insert(oid.to_vec());
        Ok(())
    }

    /// `disassociate_reference_with_id` (§11.2.4.1.1): the servants of
    /// `oid` no longer receive the requests of the group of `ref`; an
    /// association that does not exist is ignored. The object id itself
    /// stays (§11.2.4.2).
    ///
    /// # Errors
    /// [`GoaError::NotAGroupObject`].
    pub fn disassociate_reference_with_id(&self, r#ref: &Ior, oid: &[u8]) -> Result<(), GoaError> {
        let (_, group) = uipmc_group(r#ref)?;
        if let Some(set) = self.lock().members.get_mut(&group_key(&group)) {
            set.remove(oid);
        }
        Ok(())
    }

    /// `PortableServer::POA::activate_object_with_id`: `dispatch` serves
    /// the requests for `oid`. Typically the generated
    /// `dispatch_<iface>(servant, op, body, endianness)`.
    pub fn activate_object_with_id<F>(&self, oid: &[u8], dispatch: F)
    where
        F: Fn(&str, &zerodds_corba_rust::AlignedBody, Endianness) -> SkeletonResult
            + Send
            + Sync
            + 'static,
    {
        self.lock()
            .servants
            .insert(oid.to_vec(), Arc::new(dispatch));
    }

    /// `deactivate_object`: `true` when `oid` had a servant.
    pub fn deactivate_object(&self, oid: &[u8]) -> bool {
        self.lock().servants.remove(oid).is_some()
    }

    /// The counters of the adapter and its receivers.
    #[must_use]
    pub fn stats(&self) -> GoaStats {
        self.lock().stats
    }

    /// Leaves every group and stops the receivers.
    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::Relaxed);
        let handles: Vec<JoinHandle<()>> = {
            let mut g = self.lock();
            std::mem::take(&mut g.receivers).into_values().collect()
        };
        for h in handles {
            let _ = h.join();
        }
    }
}

/// What a [`Goa`] operation raises.
#[derive(Debug, Clone, PartialEq)]
pub enum GoaError {
    /// `PortableGroup::NotAGroupObject`.
    NotAGroupObject,
    /// A system exception.
    System(CorbaException),
}

impl From<NotAGroupObject> for GoaError {
    fn from(_: NotAGroupObject) -> Self {
        Self::NotAGroupObject
    }
}

fn spawn_receiver(
    mut receiver: MiopReceiver,
    state: Arc<Mutex<GoaState>>,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut last = receiver.stats();
        while !stop.load(Ordering::Relaxed) {
            let received = receiver.receive(Duration::from_millis(50));
            let stats = receiver.stats();
            if stats != last {
                let mut g = state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                add_reassembly(&mut g.stats.reassembly, last, stats);
                last = stats;
            }
            match received {
                Ok(Received::Collection(giop, _from)) => dispatch_collection(&state, &giop),
                // Refused datagrams are in the reassembly counters.
                Ok(Received::Refused(..) | Received::Idle) => {}
                Err(_) => std::thread::sleep(Duration::from_millis(10)),
            }
        }
    })
}

fn add_reassembly(total: &mut ReassemblyStats, before: ReassemblyStats, now: ReassemblyStats) {
    total.delivered += now.delivered - before.delivered;
    total.malformed += now.malformed - before.malformed;
    total.duplicates += now.duplicates - before.duplicates;
    total.incomplete += now.incomplete - before.incomplete;
    total.timed_out += now.timed_out - before.timed_out;
    total.too_large += now.too_large - before.too_large;
}

/// One GIOP request message (with its fragments) out of a collection,
/// and its body at its offset in the message.
fn single_request(giop: &[u8]) -> Option<(Request, Endianness, zerodds_corba_rust::AlignedBody)> {
    let mut cursor = Cursor::new(giop);
    let (msg, endianness, version) =
        zerodds_corba_iiop::framing::read_giop_message_full(&mut cursor).ok()?;
    if usize::try_from(cursor.position()).ok()? != giop.len() {
        return None;
    }
    match msg {
        Message::Request(r) => {
            let offset = r.body_offset(version, endianness).ok()?;
            let body = zerodds_corba_rust::AlignedBody::new(&r.body, offset);
            Some((r, endianness, body))
        }
        _ => None,
    }
}

/// Dispatches the request of a collection to every servant of the group
/// it names (§11.2.4: the plural covers collocated members).
fn dispatch_collection(state: &Arc<Mutex<GoaState>>, giop: &[u8]) {
    let lock = || {
        state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    };
    let Some((req, endianness, body)) = single_request(giop) else {
        lock().stats.not_a_request += 1;
        return;
    };
    if req.response_flags.response_expected() {
        lock().stats.two_way += 1;
        return;
    }
    let Some(group) = target_uipmc(&req.target).and_then(|u| u.group_info().ok().flatten()) else {
        lock().stats.no_group += 1;
        return;
    };
    let servants: Vec<Option<Dispatcher>> = {
        let g = lock();
        match g.members.get(&group_key(&group)) {
            Some(oids) if !oids.is_empty() => {
                oids.iter().map(|o| g.servants.get(o).cloned()).collect()
            }
            _ => Vec::new(),
        }
    };
    if servants.is_empty() {
        lock().stats.no_member += 1;
        return;
    }
    for servant in servants {
        match servant {
            Some(d) => {
                let result = d(&req.operation, &body, endianness);
                let mut g = lock();
                g.stats.dispatched += 1;
                if matches!(result, SkeletonResult::Exception(_)) {
                    g.stats.exceptions += 1;
                }
            }
            None => lock().stats.no_servant += 1,
        }
    }
}

/// What a [`MiopGateway`] counted besides the requests it forwarded.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GatewayStats {
    /// Requests forwarded to a group over multicast.
    pub forwarded: u64,
    /// `CORBA::Object` operations forwarded to the `TAG_GROUP_IIOP`
    /// profile of the group (§11.2.6.10).
    pub object_operations: u64,
    /// `LocateRequest`s answered.
    pub locate_requests: u64,
    /// `CancelRequest`s: the gateway answers each request before it reads
    /// the next message, so there is nothing left to cancel (§9.4.4).
    pub cancel_requests: u64,
    /// Messages refused with `MessageError`: a header the gateway cannot
    /// read, a `Reply`, a `LocateReply`, a `Fragment` without a fragmented
    /// message before it (Part 2 §9.4.8).
    pub refused: u64,
}

/// The MIOP gateway (§11.2.5): an IIOP server that forwards the requests
/// of MIOP-unaware clients to the group whose UIPMC profile the request
/// names (GIOP 1.2 target, a reference target, or the `"MIOP"` object key
/// of GIOP 1.0 and 1.1).
///
/// A group operation has a `void` result, `in` parameters only and no
/// exceptions (§11.2.6): a request that expects a reply is forwarded and
/// answered with an empty `NO_EXCEPTION` reply. The operations of
/// `CORBA::Object` (`_is_a`, `_non_existent`, `_interface`,
/// `_domain_managers`, `_component`, `_repository_id`) go to the IIOP
/// profile of the `TAG_GROUP_IIOP` component of the UIPMC profile, which
/// is there to support them (§11.2.6.10, §11.3.1); the reply of that
/// object is the gateway's reply. Without that component `_non_existent`
/// is answered `true` (§11.2.6.10) and the others, which need the type of
/// the group, `NO_IMPLEMENT`. A request whose target names no UIPMC group
/// profile is answered `OBJECT_NOT_EXIST`. A `LocateRequest` is answered
/// `OBJECT_HERE` for a target whose UIPMC profile names a group, else
/// `UNKNOWN_OBJECT`. A header the gateway cannot read, a `Reply`, a
/// `LocateReply` and a `Fragment` without a fragmented message before it
/// are answered with `MessageError` and end the connection (Part 2
/// §9.4.8); `CloseConnection` and `MessageError` from the client end it.
#[derive(Clone)]
pub struct MiopGateway {
    client: Arc<MiopClient>,
    iiop: Arc<crate::runtime::IiopCorbaConnection>,
    stats: Arc<Mutex<GatewayStats>>,
}

impl core::fmt::Debug for MiopGateway {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MiopGateway")
            .field("client", &self.client)
            .field("stats", &self.stats())
            .finish_non_exhaustive()
    }
}

impl MiopGateway {
    /// A gateway that forwards with `client` and reaches the
    /// `TAG_GROUP_IIOP` profiles over a new IIOP connection pool.
    #[must_use]
    pub fn new(client: MiopClient) -> Self {
        Self::with_connection(client, crate::runtime::IiopCorbaConnection::new())
    }

    /// A gateway that forwards with `client` and reaches the
    /// `TAG_GROUP_IIOP` profiles over `iiop` (e.g. one with TLS or an
    /// ORB's interceptors).
    #[must_use]
    pub fn with_connection(client: MiopClient, iiop: crate::runtime::IiopCorbaConnection) -> Self {
        Self {
            client: Arc::new(client),
            iiop: Arc::new(iiop),
            stats: Arc::default(),
        }
    }

    /// The counters of the gateway.
    #[must_use]
    pub fn stats(&self) -> GatewayStats {
        *self
            .stats
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Accepts IIOP connections on `bind`.
    ///
    /// # Errors
    /// Bind error.
    pub fn serve(&self, bind: SocketAddr) -> Result<Acceptor, IiopError> {
        let gateway = self.clone();
        Acceptor::start(AcceptorConfig::new(bind), move |mut conn| {
            gateway.serve_connection(&mut conn);
        })
    }

    fn count(&self, f: impl FnOnce(&mut GatewayStats)) {
        f(&mut self
            .stats
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner));
    }

    fn serve_connection(&self, conn: &mut zerodds_corba_iiop::Connection) {
        loop {
            let (msg, endianness, version) = match conn.read_message_full() {
                Ok(m) => m,
                Err(e) => {
                    if crate::runtime::refuse_message(conn, &e) {
                        self.count(|s| s.refused += 1);
                    }
                    return;
                }
            };
            let reply_version = if version.uses_v1_2_request_layout() {
                Version::V1_2
            } else {
                version
            };
            let req = match msg {
                Message::Request(req) => req,
                Message::LocateRequest(lr) => {
                    self.count(|s| s.locate_requests += 1);
                    let here = target_uipmc(&lr.target)
                        .is_some_and(|u| matches!(u.group_info(), Ok(Some(_))));
                    let reply = Message::LocateReply(LocateReply {
                        request_id: lr.request_id,
                        locate_status: if here {
                            LocateStatusType::ObjectHere
                        } else {
                            LocateStatusType::UnknownObject
                        },
                        body: Vec::new(),
                    });
                    let _ = conn.write_message(reply_version, endianness, false, &reply);
                    continue;
                }
                other => {
                    match &other {
                        Message::CancelRequest(_) => self.count(|s| s.cancel_requests += 1),
                        Message::Reply(_) | Message::LocateReply(_) | Message::Fragment(_) => {
                            self.count(|s| s.refused += 1);
                        }
                        _ => {}
                    }
                    if crate::runtime::server_other_message(conn, &other) {
                        continue;
                    }
                    return;
                }
            };
            let (outcome, reply_contexts) = match req.body_offset(version, endianness) {
                Ok(offset) => self.forward(&req, endianness, offset),
                Err(_) => (
                    Err(CorbaException::system(
                        SystemExceptionKind::Marshal,
                        0,
                        CompletionStatus::No,
                    )),
                    ServiceContextList::default(),
                ),
            };
            if req.response_flags.response_expected() {
                // The body goes on, on the offset modulo eight it stood
                // at in the reply of the group's server (Part 2 §9.4.3).
                let (reply_status, body, reply_endianness, origin) = match outcome {
                    Ok((body, e)) => (
                        ReplyStatusType::NoException,
                        body.bytes().to_vec(),
                        e,
                        body.offset(),
                    ),
                    // In the byte order the group's server wrote it in.
                    Err(CorbaException::UserException {
                        body, endianness, ..
                    }) => (
                        ReplyStatusType::UserException,
                        body.bytes().to_vec(),
                        endianness,
                        body.offset(),
                    ),
                    Err(e) => crate::runtime::reply_status_and_body(
                        SkeletonResult::Exception(e),
                        endianness,
                    ),
                };
                let mut reply = Message::Reply(Reply {
                    request_id: req.request_id,
                    reply_status,
                    service_context: reply_contexts,
                    body,
                });
                if crate::runtime::place_reply_body(
                    &mut reply,
                    reply_version,
                    reply_endianness,
                    origin,
                )
                .is_ok()
                {
                    let _ = conn.write_message(reply_version, reply_endianness, false, &reply);
                }
            }
        }
    }

    /// Forwards one request with its service contexts (Part 2 §7.7.2: a
    /// bridge passes them on); the reply body of a successful forward and
    /// its byte order, and the service contexts of the reply an object
    /// operation got over IIOP, which go back to the client. `offset` is
    /// that of the body in the received message: the body goes on as it
    /// is ([`zerodds_corba_rust::marshaled_body`]).
    fn forward(
        &self,
        req: &Request,
        endianness: Endianness,
        offset: usize,
    ) -> (
        Result<(zerodds_corba_rust::AlignedBody, Endianness), CorbaException>,
        ServiceContextList,
    ) {
        if is_object_operation(&req.operation)
            && let Some(Ok(Some(oma))) = target_uipmc(&req.target)
                .filter(|u| matches!(u.group_info(), Ok(Some(_))))
                .map(|u| u.group_iiop_profile())
        {
            let target = match oma.encode_encapsulation(Endianness::Big) {
                Ok(iiop_profile) => ObjectReference {
                    type_id: String::new(),
                    iiop_profile,
                    policy_overrides: Vec::new(),
                    profiles: Vec::new(),
                },
                Err(_) => {
                    return (
                        Err(CorbaException::system(
                            SystemExceptionKind::InvObjref,
                            0,
                            CompletionStatus::No,
                        )),
                        ServiceContextList::default(),
                    );
                }
            };
            self.count(|s| s.object_operations += 1);
            return self.iiop.invoke_passing_contexts(
                &target,
                &req.operation,
                endianness,
                &zerodds_corba_rust::marshaled_body(&req.body, offset),
                &req.service_context,
            );
        }
        (
            self.forward_to_group(req, endianness, offset)
                .map(|(body, e)| (zerodds_corba_rust::AlignedBody::new(&body, 0), e)),
            ServiceContextList::default(),
        )
    }

    /// Forwards a request that is not an object operation answered over
    /// the group's IIOP profile: an object operation without one, or a
    /// request to the group over MIOP.
    fn forward_to_group(
        &self,
        req: &Request,
        endianness: Endianness,
        offset: usize,
    ) -> Result<(Vec<u8>, Endianness), CorbaException> {
        use zerodds_corba_giop::object_ops::{OP_NON_EXISTENT, OP_NOT_EXISTENT};
        let profile = target_uipmc(&req.target)
            .filter(|u| matches!(u.group_info(), Ok(Some(_))))
            .ok_or_else(|| {
                CorbaException::system(SystemExceptionKind::ObjectNotExist, 0, CompletionStatus::No)
            })?;
        if is_object_operation(&req.operation) {
            // `forward` sent it over the group's IIOP profile when the
            // profile has one that decodes.
            if matches!(req.operation.as_str(), OP_NON_EXISTENT | OP_NOT_EXISTENT)
                && !matches!(profile.group_iiop_profile(), Ok(Some(_)))
            {
                let mut w = zerodds_wire::Writer::new(endianness);
                w.write_bool(true);
                return Ok((w.finish(), endianness));
            }
            return Err(CorbaException::system(
                SystemExceptionKind::NoImplement,
                0,
                CompletionStatus::No,
            ));
        }
        let message = self.client.request_message(
            &profile,
            &req.operation,
            endianness,
            &zerodds_corba_rust::marshaled_body(&req.body, offset),
            req.service_context.clone(),
        )?;
        self.client.send_message(&profile, &message)?;
        self.count(|s| s.forwarded += 1);
        Ok((Vec::new(), endianness))
    }
}
