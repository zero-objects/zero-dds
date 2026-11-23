// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA 3.3 ORB core:
//! - Part 1 §8 ORB Interface: `ORB_init` with its `-ORB<suffix>`
//!   arguments ([`orb_init`], §8.5.1), the ORB identity, object
//!   references to and from strings (§8.2.2), initial references
//!   (`resolve_initial_references`, `list_initial_services`,
//!   `register_initial_reference`, `-ORBInitRef`,
//!   `-ORBDefaultInitRef`, §8.5.2, §8.5.3, §16.10), shutdown and
//!   destroy, the client-side policy overrides of the ORB, the thread
//!   and the object reference (§8.3.8, §8.3.9, §8.9, with
//!   [`crate::policy_management`]).
//! - Part 1 §9 Value Type custom-marshal streams + sending context
//! - Part 3 §12 IFR metamodel CCM extension
//! - Part 3 §13 CIF metamodel
//!
//! Remote work of the ORB (the `resolve` of a `corbaname:` URL) goes
//! through the [`CorbaConnection`] the transport layer installs with
//! [`Orb::set_connection`]; `zerodds-corba-interop` installs its IIOP
//! connection. The XMI emitter uses a MOF-2.0 subset.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::any::Any;
use std::sync::{Condvar, Mutex, PoisonError};

use zerodds_corba_iiop::IiopProfileBody;
use zerodds_corba_ior::{
    Ior, ProfileId, TaggedProfile, from_stringified, parse_corbaloc, parse_corbaname,
    to_stringified,
};
use zerodds_corba_rt::{
    CompletionStatus, ObjectReference, PolicyOverride, RawProfile, SystemException,
    SystemExceptionKind,
};
use zerodds_corba_rust::{CorbaConnection, CorbaException};
use zerodds_wire::Endianness;

use crate::compression::{
    COMPRESSORID_NONE, CompressionManager, CompressorIdLevel, UnknownCompressorId, check_level,
};
use crate::orb_extensions::MessagingPolicy;
use crate::policy_management::{
    ORB_POLICY_MANAGER_INITIAL_REFERENCE, OverrideError, POLICY_CURRENT_INITIAL_REFERENCE,
    PolicyCurrent, PolicyManager, SetOverrideType, client_policy, get_object_overrides,
    set_object_overrides,
};
use crate::portable_interceptor::{
    InitializerError, InterceptorRegistry, OrbInitInfo, PiCurrent, Policy, PolicyError,
    registered_initializers,
};
use zerodds_corba_rt::CorbaAny;

// ===========================================================================
// Part 1 §8 ORB Interface
// ===========================================================================

/// Spec §8.2.1 — ORB initialization state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrbState {
    /// `ORB_init` not yet called.
    Uninitialized,
    /// `ORB_init` succeeded; the ORB is running.
    Running,
    /// `shutdown` called, waiting on outstanding requests.
    ShuttingDown,
    /// `shutdown` complete; the ORB is ready for `destroy`.
    Shutdown,
    /// `destroy` called (§8.2.5.5): every operation raises
    /// `OBJECT_NOT_EXIST`.
    Destroyed,
}

/// Spec §8.2.5 — threading operations on the ORB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadingMode {
    /// `single-threaded` — all requests sequential.
    SingleThreaded,
    /// `thread-per-request` — a new thread per request.
    ThreadPerRequest,
    /// `thread-pool` — worker pool with a fixed thread count.
    ThreadPool {
        /// Number of worker threads.
        size: usize,
    },
}

/// The ORB pseudo-object (§8.2): identity, lifecycle, initial
/// references, object reference strings, policies.
pub struct Orb {
    /// §8.2.1 — the ORBid the ORB was created with.
    id: String,
    state: Mutex<OrbState>,
    threading: ThreadingMode,
    /// §8.5.1.1 — `-ORBServerId`.
    server_id: Option<String>,
    /// §8.5.1.2 — `-ORBListenEndpoints`, in the order given.
    listen_endpoints: Vec<String>,
    /// §8.5.1.3 — `-ORBNoProprietaryActivation`.
    no_proprietary_activation: bool,
    /// §8.5.3.2 — `-ORBInitRef` entries, ObjectId to URL.
    init_refs: Mutex<BTreeMap<String, String>>,
    /// §8.5.3.3 — `-ORBDefaultInitRef`.
    default_init_ref: Mutex<Option<String>>,
    /// §16.10 — `register_initial_reference` entries.
    registered: Mutex<BTreeMap<String, ObjectRef>>,
    /// §8.5.3.4 step 3 — the services this ORB brings itself.
    preconfigured: Mutex<BTreeMap<String, ObjectRef>>,
    /// The transport for the ORB's own remote calls.
    connection: Mutex<Option<Arc<dyn CorbaConnection + Send + Sync>>>,
    /// §16.5 — `PortableInterceptor::Current` of this ORB.
    pi_current: Arc<PiCurrent>,
    /// §8.9.1 — the ORB-scope overrides (`"ORBPolicyManager"`).
    policy_manager: Arc<PolicyManager>,
    /// §8.9.1 — the thread-scope overrides (`"PolicyCurrent"`).
    policy_current: Arc<PolicyCurrent>,
    /// CCM 4.0 §2 CP8 — component-specific extensions on the ORB:
    /// optional interceptor registry, selected messaging policies,
    /// the ORB-wide compressor.
    interceptors: Mutex<Option<Arc<InterceptorRegistry>>>,
    messaging_policies: Mutex<Vec<MessagingPolicy>>,
    compression: Mutex<CompressorIdLevel>,
    /// Part 1 §18.4 — the initial reference `CompressionManager`.
    compression_manager: Arc<CompressionManager>,
}

impl core::fmt::Debug for Orb {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = self
            .state
            .lock()
            .map(|g| *g)
            .unwrap_or(OrbState::Uninitialized);
        f.debug_struct("Orb").field("state", &s).finish()
    }
}

impl Orb {
    /// An ORB with the empty ORBid and no `ORB_init` arguments, outside
    /// the ORB table of [`orb_init`].
    #[must_use]
    pub fn init(threading: ThreadingMode) -> Self {
        Self::with_arguments(String::new(), OrbArguments::default(), threading)
    }

    fn with_arguments(id: String, args: OrbArguments, threading: ThreadingMode) -> Self {
        Self {
            id,
            state: Mutex::new(OrbState::Running),
            threading,
            server_id: args.server_id,
            listen_endpoints: args.listen_endpoints,
            no_proprietary_activation: args.no_proprietary_activation,
            init_refs: Mutex::new(args.init_refs.into_iter().collect()),
            default_init_ref: Mutex::new(args.default_init_ref),
            registered: Mutex::new(BTreeMap::new()),
            preconfigured: Mutex::new(BTreeMap::new()),
            connection: Mutex::new(None),
            pi_current: Arc::new(PiCurrent::new()),
            policy_manager: Arc::new(PolicyManager::new()),
            policy_current: Arc::new(PolicyCurrent::new()),
            interceptors: Mutex::new(None),
            messaging_policies: Mutex::new(Vec::new()),
            compression: Mutex::new(NO_COMPRESSION),
            compression_manager: Arc::new(CompressionManager::with_default_factories()),
        }
    }

    /// Current state.
    #[must_use]
    pub fn state(&self) -> OrbState {
        self.state
            .lock()
            .map(|g| *g)
            .unwrap_or(OrbState::Uninitialized)
    }

    /// Threading mode.
    #[must_use]
    pub fn threading(&self) -> ThreadingMode {
        self.threading
    }

    /// §8.2.5.4 — `shutdown(wait_for_completion)`: the ORB stops
    /// taking operations.
    ///
    /// # Errors
    /// `OBJECT_NOT_EXIST` after `destroy` (§8.2.5.5).
    pub fn shutdown(&self, _wait_for_completion: bool) -> Result<(), SystemException> {
        let mut g = self.state.lock().map_err(|_| internal())?;
        match *g {
            OrbState::Destroyed => Err(object_not_exist()),
            OrbState::Running => {
                *g = OrbState::ShuttingDown;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// §8.2.5.5 — `destroy`: an ORB that is not shut down is shut down
    /// first; the registered interceptors get `destroy` (§16.3); every
    /// later operation raises `OBJECT_NOT_EXIST`, and the ORBid is free
    /// again: a later [`orb_init`] with it creates a new ORB.
    ///
    /// # Errors
    /// `OBJECT_NOT_EXIST` when the ORB is already destroyed.
    pub fn destroy(&self) -> Result<(), SystemException> {
        self.shutdown(true)?;
        {
            let mut g = self.state.lock().map_err(|_| internal())?;
            if *g == OrbState::Destroyed {
                return Err(object_not_exist());
            }
            *g = OrbState::Destroyed;
        }
        if let Some(r) = self.interceptor_registry() {
            r.destroy_all();
        }
        if let Ok(mut table) = ORB_TABLE.lock()
            && table
                .orbs
                .get(&self.id)
                .is_some_and(|o| core::ptr::eq(Arc::as_ptr(o), self))
        {
            table.orbs.remove(&self.id);
        }
        Ok(())
    }

    /// §8.2.1 — `id()`: the ORBid passed to `ORB_init`, or the value of
    /// `-ORBid` when that was empty; the empty string for the default
    /// ORB.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// §8.5.1.1 — the `-ORBServerId` value.
    #[must_use]
    pub fn server_id(&self) -> Option<&str> {
        self.server_id.as_deref()
    }

    /// §8.5.1.2 — the `-ORBListenEndpoints` values, in the order given.
    #[must_use]
    pub fn listen_endpoints(&self) -> &[String] {
        &self.listen_endpoints
    }

    /// §8.5.1.3 — `true` when `-ORBNoProprietaryActivation` was given.
    #[must_use]
    pub fn no_proprietary_activation(&self) -> bool {
        self.no_proprietary_activation
    }

    /// Installs the transport for the ORB's own remote calls (the
    /// `resolve` of a `corbaname:` URL).
    pub fn set_connection(&self, connection: Arc<dyn CorbaConnection + Send + Sync>) {
        if let Ok(mut g) = self.connection.lock() {
            *g = Some(connection);
        }
    }

    /// `true` when a transport is installed.
    #[must_use]
    pub fn has_connection(&self) -> bool {
        self.connection.lock().is_ok_and(|g| g.is_some())
    }

    /// Makes `obj` an ORB-configured initial reference (§8.5.3.4 step 3:
    /// "pre-configured ORB settings"). The services of this ORB register
    /// themselves here; `register_initial_reference` and `-ORBInitRef`
    /// take precedence over it.
    pub fn configure_initial_reference(&self, id: &str, obj: ObjectRef) {
        if let Ok(mut g) = self.preconfigured.lock() {
            g.insert(id.to_string(), obj);
        }
    }

    /// §16.5 — the ORB's `PortableInterceptor::Current` (also the
    /// initial reference `PICurrent`).
    #[must_use]
    pub fn pi_current(&self) -> Arc<PiCurrent> {
        Arc::clone(&self.pi_current)
    }

    /// §8.9.1 — the ORB's `PolicyManager`, the overrides at the ORB
    /// scope (also its initial reference `"ORBPolicyManager"`).
    #[must_use]
    pub fn policy_manager(&self) -> Arc<PolicyManager> {
        Arc::clone(&self.policy_manager)
    }

    /// §8.9.1 — the ORB's `PolicyCurrent`, the overrides of the calling
    /// thread (also its initial reference `"PolicyCurrent"`).
    #[must_use]
    pub fn policy_current(&self) -> Arc<PolicyCurrent> {
        Arc::clone(&self.policy_current)
    }

    /// §8.3.8.2 — `Object::get_client_policy(type)` of `reference`: the
    /// override at the object scope, else at the calling thread's scope,
    /// else at the ORB scope; `None` when no scope overrides the type.
    ///
    /// # Errors
    /// `MARSHAL` for an override of `reference` that does not decode.
    pub fn get_client_policy(
        &self,
        reference: &ObjectReference,
        policy_type: u32,
    ) -> Result<Option<Policy>, SystemException> {
        client_policy(
            reference,
            Some(&self.policy_current),
            Some(&self.policy_manager),
            policy_type,
        )
    }

    /// Part 2 §12.2 — the ZIOP policies a client of this ORB uses for an
    /// invocation on `reference`: [`client_ziop_policies`] with the
    /// calling thread's and the ORB's overrides.
    ///
    /// # Errors
    /// As [`client_ziop_policies`].
    pub fn client_ziop_policies(
        &self,
        reference: &ObjectReference,
        base: Option<&zerodds_corba_giop::ziop::ZiopPolicies>,
    ) -> Result<Option<zerodds_corba_giop::ziop::ZiopPolicies>, SystemException> {
        client_ziop_policies(
            reference,
            Some(&self.policy_current),
            Some(&self.policy_manager),
            base,
        )
    }

    /// Part 2 §12.2 — the ZIOP policies of a server bound to this ORB that
    /// has none of its own: the ZIOP overrides at the ORB scope over the
    /// ORB defaults; `None` when the ORB scope overrides no ZIOP policy.
    ///
    /// # Errors
    /// `INV_POLICY` when the overrides do not make ZIOP policies.
    pub fn server_ziop_policies(
        &self,
    ) -> Result<Option<zerodds_corba_giop::ziop::ZiopPolicies>, SystemException> {
        let overrides: Vec<Policy> = self
            .policy_manager
            .get_policy_overrides(&[])
            .into_iter()
            .filter(|p| ziop_policy(p.policy_type, &p.value).is_some())
            .collect();
        if overrides.is_empty() {
            return Ok(None);
        }
        apply_ziop_policies(
            zerodds_corba_giop::ziop::ZiopPolicies::default(),
            &overrides,
        )
        .map(Some)
        .map_err(|_| inv_policy())
    }

    /// §8.8.2 `create_policy(type, val)`: the four ZIOP policies of Part 2
    /// §12.3 are the ORB's own ([`ziop_policy`]); every other type goes to
    /// the policy factories the ORB initializers registered (§16.8).
    ///
    /// # Errors
    /// `PolicyError` `BAD_POLICY_VALUE` for a ZIOP policy value of the
    /// wrong type or out of range, `BAD_POLICY_TYPE` for a type without a
    /// factory, else the factory's `PolicyError`.
    pub fn create_policy(&self, policy_type: u32, value: &CorbaAny) -> Result<Policy, PolicyError> {
        if let Some(created) = ziop_policy(policy_type, value) {
            return created;
        }
        match self.interceptor_registry() {
            Some(r) => r.create_policy(policy_type, value),
            None => Err(PolicyError {
                reason: PolicyError::BAD_POLICY_TYPE,
            }),
        }
    }

    /// §16.9 — runs `pre_init` and then `post_init` of every registered
    /// `ORBInitializer` on this ORB: the interceptors, slots, policy
    /// factories and initial references they register become the ORB's.
    /// The PICurrent slots cannot be read or written meanwhile
    /// (§16.9.2.14). An error of an initializer abandons the ORB
    /// (§16.9.1): the interceptors added so far get `destroy` and the ORB
    /// is destroyed.
    fn run_initializers(&self, arguments: &[String]) -> Result<(), SystemException> {
        let initializers = registered_initializers();
        let mut registry = InterceptorRegistry::new();
        self.pi_current.set_initializing(true);
        let mut failed = None;
        'run: for in_post_init in [false, true] {
            for init in &initializers {
                let mut info = OrbInitInfo {
                    arguments,
                    orb: self,
                    registry: &mut registry,
                    in_post_init,
                };
                let r = if in_post_init {
                    init.post_init(&mut info)
                } else {
                    init.pre_init(&mut info)
                };
                if let Err(e) = r {
                    failed = Some(e);
                    break 'run;
                }
            }
        }
        self.pi_current.set_initializing(false);
        if let Some(e) = failed {
            registry.destroy_all();
            if let Ok(mut g) = self.state.lock() {
                *g = OrbState::Destroyed;
            }
            return Err(match e {
                InitializerError::System(e) => e,
                InitializerError::DuplicateName(_) | InitializerError::InvalidName => {
                    SystemException::new(SystemExceptionKind::Initialize, 0, CompletionStatus::No)
                }
            });
        }
        self.with_interceptor_registry(Arc::new(registry));
        Ok(())
    }

    /// `BAD_INV_ORDER` minor 4 once the ORB has shut down (§8.2.5.4),
    /// `OBJECT_NOT_EXIST` once it is destroyed (§8.2.5.5).
    fn check_running(&self) -> Result<(), SystemException> {
        match self.state() {
            OrbState::Running => Ok(()),
            OrbState::Destroyed => Err(object_not_exist()),
            _ => Err(SystemException::omg(
                SystemExceptionKind::BadInvOrder,
                4,
                CompletionStatus::No,
            )),
        }
    }

    /// §8.5.2 — `list_initial_services`: the ids of
    /// `register_initial_reference`, of every `-ORBInitRef` and of the
    /// ORB-configured services, sorted, each once.
    ///
    /// # Errors
    /// `BAD_INV_ORDER` minor 4 after shutdown.
    pub fn list_initial_services(&self) -> Result<Vec<String>, SystemException> {
        self.check_running()?;
        let mut ids: Vec<String> = Vec::new();
        for map in [&self.registered, &self.preconfigured] {
            if let Ok(g) = map.lock() {
                ids.extend(g.keys().cloned());
            }
        }
        if let Ok(g) = self.init_refs.lock() {
            ids.extend(g.keys().cloned());
        }
        ids.sort();
        ids.dedup();
        Ok(ids)
    }

    /// §8.5.2 — `resolve_initial_references`, in the default order of
    /// §8.5.3.4: `register_initial_reference`, `-ORBInitRef`, the
    /// ORB-configured services, `-ORBDefaultInitRef` with `/` and the id
    /// appended. A nil reference is never returned (`InvalidName`).
    ///
    /// # Errors
    /// [`ResolveError::InvalidName`] when no step yields a non-nil
    /// reference; [`ResolveError::System`] for `BAD_INV_ORDER` minor 4
    /// after shutdown and for the exception `string_to_object` raises
    /// for a configured URL.
    pub fn resolve_initial_references(&self, id: &str) -> Result<ObjectRef, ResolveError> {
        self.check_running()?;
        // A `rir` URL configured for an id can lead back to the id being
        // resolved (`-ORBInitRef X=corbaname:rir:/X#n`): that resolution
        // finds nothing.
        let Some(_guard) = Resolving::enter(self, id) else {
            return Err(ResolveError::InvalidName);
        };
        if let Some(obj) = self.registered.lock().ok().and_then(|g| g.get(id).cloned()) {
            return non_nil(obj);
        }
        if let Some(url) = self.init_refs.lock().ok().and_then(|g| g.get(id).cloned()) {
            return non_nil(self.string_to_object(&url)?);
        }
        if let Some(obj) = self
            .preconfigured
            .lock()
            .ok()
            .and_then(|g| g.get(id).cloned())
        {
            return non_nil(obj);
        }
        if let Some(base) = self.default_init_ref.lock().ok().and_then(|g| g.clone()) {
            return non_nil(self.string_to_object(&default_init_ref_url(&base, id))?);
        }
        Err(ResolveError::InvalidName)
    }

    /// §16.10 — `register_initial_reference`.
    ///
    /// # Errors
    /// [`ResolveError::InvalidName`] for an empty id and for an id that
    /// is already registered here or ORB-configured;
    /// [`ResolveError::System`] with `BAD_PARAM` minor 27 for a nil
    /// object, `BAD_INV_ORDER` minor 4 after shutdown.
    pub fn register_initial_reference(&self, id: &str, obj: ObjectRef) -> Result<(), ResolveError> {
        self.check_running()?;
        if obj.is_nil() {
            return Err(ResolveError::System(SystemException::omg(
                SystemExceptionKind::BadParam,
                27,
                CompletionStatus::No,
            )));
        }
        if id.is_empty() || self.preconfigured.lock().is_ok_and(|g| g.contains_key(id)) {
            return Err(ResolveError::InvalidName);
        }
        let mut g = self
            .registered
            .lock()
            .map_err(|_| ResolveError::System(internal()))?;
        if g.contains_key(id) {
            return Err(ResolveError::InvalidName);
        }
        g.insert(id.to_string(), obj);
        Ok(())
    }

    /// §8.2.2 — `object_to_string`: the `IOR:` form of the reference.
    ///
    /// # Errors
    /// `MARSHAL` minor 4 for a local object (Annex A.6, "Attempt to
    /// marshal Local object"), `MARSHAL` for a reference whose profile
    /// does not encode, `BAD_INV_ORDER` minor 4 after shutdown.
    pub fn object_to_string(&self, obj: &ObjectRef) -> Result<String, SystemException> {
        self.check_running()?;
        let reference = match obj {
            ObjectRef::Reference(r) => r,
            ObjectRef::Local(_) => {
                return Err(SystemException::omg(
                    SystemExceptionKind::Marshal,
                    4,
                    CompletionStatus::No,
                ));
            }
            ObjectRef::Group(ior, _) => {
                return to_stringified(ior, Endianness::Big).map_err(|_| {
                    SystemException::new(SystemExceptionKind::Marshal, 0, CompletionStatus::No)
                });
            }
        };
        // Part 2 §7.6.3: every profile of the reference, in its order.
        let profiles = reference
            .tagged_profiles()
            .into_iter()
            .map(|p| TaggedProfile {
                tag: ProfileId::from_u32(p.tag),
                profile_data: p.data,
            })
            .collect();
        let ior = Ior::new(reference.type_id.clone(), profiles);
        to_stringified(&ior, Endianness::Big).map_err(|_| {
            SystemException::new(SystemExceptionKind::Marshal, 0, CompletionStatus::No)
        })
    }

    /// §8.2.2 — `string_to_object` over the URL schemes of Part 2
    /// §7.6.10, Table 7.1: `IOR:`, `corbaloc:` (`iiop`, `rir`, `miop`)
    /// and `corbaname:`, and the optional `file://`, `http://` and
    /// `ftp://`, which name a file holding a URL or IOR that is converted
    /// in turn. Scheme names are matched without regard to case.
    ///
    /// A `corbaloc` gives one IIOP profile per IIOP address, in the order
    /// of the URL; an address of another protocol (§7.6.10.6) is skipped.
    ///
    /// # Errors
    /// `BAD_PARAM` with the minor codes of §7.6.10: 7 for an unknown
    /// scheme, 8 for a bad address (also a `corbaloc` without an address
    /// this ORB can use, `rir` beside another address, a `file` URL on
    /// another host), 9 for a bad scheme-specific part (an IOR or key
    /// string that does not decode; fetched text, which is the
    /// scheme-specific part of its `file`, `http` or `ftp` URL, when it
    /// is empty, not UTF-8 or has no scheme this ORB converts), 10
    /// when a `rir` or `corbaname` resolution finds nothing, a file
    /// cannot be fetched, or fetched URLs lead on more than
    /// [`MAX_URL_INDIRECTIONS`] times. `INITIALIZE` for `corbaname:`
    /// when no transport is installed; the system exception of the
    /// naming context's `resolve`. `BAD_INV_ORDER` minor 4 after
    /// shutdown.
    pub fn string_to_object(&self, url: &str) -> Result<ObjectRef, SystemException> {
        self.url_to_object(url, 0)
    }

    fn url_to_object(&self, url: &str, depth: usize) -> Result<ObjectRef, SystemException> {
        self.check_running()?;
        match scheme_of(url).as_deref() {
            Some("ior:") => ior_string_to_object(&url[4..]),
            Some("corbaloc:") => self.corbaloc_to_object(url),
            Some("corbaname:") => self.corbaname_to_object(url),
            Some("file:" | "http:" | "ftp:") => {
                if depth >= MAX_URL_INDIRECTIONS {
                    return Err(bad_param(10));
                }
                let text = crate::object_url::fetch(url).map_err(|e| bad_param(e.minor()))?;
                self.url_to_object(&text, depth + 1)
            }
            // Fetched text (depth > 0) is the content of a URL whose
            // scheme was good: a bad schema specific part.
            _ => Err(bad_param(if depth == 0 { 7 } else { 9 })),
        }
    }

    fn corbaloc_to_object(&self, url: &str) -> Result<ObjectRef, SystemException> {
        let payload = &url["corbaloc:".len()..];
        // Part 2 §11.2.7: a URL with an MIOP address is a group reference.
        if payload.starts_with("miop:") || payload.contains(",miop:") {
            return zerodds_corba_ior::miop::parse_miop_corbaloc(&alloc::format!(
                "corbaloc:{payload}"
            ))
            .map(|ior| ObjectRef::Group(ior, Vec::new()))
            .map_err(|_| bad_param(8));
        }
        let addr = parse_corbaloc(url).map_err(|e| bad_param(e.bad_param_minor()))?;
        if addr.is_rir() {
            // Part 2 §7.6.10.2: the key names the initial reference.
            return self.rir_object(&addr.object_key);
        }
        corbaloc_iiop_reference(&addr).map(ObjectRef::Reference)
    }

    /// The initial reference a `rir` URL names (§7.6.10.2).
    fn rir_object(&self, key: &[u8]) -> Result<ObjectRef, SystemException> {
        let key = core::str::from_utf8(key).map_err(|_| bad_param(9))?;
        match self.resolve_initial_references(key) {
            Ok(obj) => Ok(obj),
            Err(ResolveError::InvalidName) => Err(bad_param(10)),
            Err(ResolveError::System(e)) => Err(e),
        }
    }

    fn corbaname_to_object(&self, url: &str) -> Result<ObjectRef, SystemException> {
        let addr = parse_corbaname(url).map_err(|e| bad_param(e.bad_param_minor()))?;
        let name = if addr.stringified_name.is_empty() {
            Vec::new()
        } else {
            zerodds_corba_cosnaming::string_to_name(&addr.stringified_name)
                .map_err(|_| bad_param(9))?
        };
        let context = if addr.address.is_rir() {
            self.rir_object(&addr.address.object_key)?
        } else {
            ObjectRef::Reference(corbaloc_iiop_reference(&addr.address)?)
        };
        if name.is_empty() {
            return Ok(context);
        }
        let ObjectRef::Reference(context) = context else {
            return Err(bad_param(10));
        };
        let connection = self
            .connection
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .ok_or_else(|| {
                SystemException::new(SystemExceptionKind::Initialize, 0, CompletionStatus::No)
            })?;
        // The name is written where the connection places the body
        // (CORBA 3.3 Part 2 §9.4.2.1); a name that does not encode is
        // BAD_PARAM 9, the request is not sent.
        let (reply, endianness) =
            match connection.invoke(&context, "resolve", Endianness::Big, &|w| {
                zerodds_corba_cosnaming::wire::write_name(w, &name)
                    .map_err(|_| CorbaException::System(bad_param(9)))
            }) {
                Ok(r) => r,
                // NotFound, CannotProceed, InvalidName of CosNaming.
                Err(CorbaException::UserException { .. }) => return Err(bad_param(10)),
                Err(CorbaException::System(e)) => return Err(e),
                Err(_) => return Err(bad_param(10)),
            };
        let ior = zerodds_corba_cosnaming::wire::decode_object_reply(&reply, endianness).map_err(
            |_| SystemException::new(SystemExceptionKind::Marshal, 0, CompletionStatus::Yes),
        )?;
        Ok(ObjectRef::Reference(reference_from_ior(&ior)))
    }

    // -----------------------------------------------------------------
    // CCM 4.0 §2 CP8 — component-specific extensions on the ORB.
    // Cross-ref `corba-3.3.md` §16 (PI), §17 (Messaging), §18
    // (Compression). These methods configure the ORB singleton
    // for the three vendor extensions.
    // -----------------------------------------------------------------

    /// CCM 4.0 §2 CP8 — installs an [`InterceptorRegistry`] on the
    /// ORB. Subsequent `iiop::Connection` instances can fetch and
    /// wire it via `interceptor_registry()`.
    pub fn with_interceptor_registry(&self, r: Arc<InterceptorRegistry>) {
        if let Ok(mut g) = self.interceptors.lock() {
            *g = Some(r);
        }
    }

    /// CCM 4.0 §2 CP8 — returns the current registry (None if none
    /// installed).
    #[must_use]
    pub fn interceptor_registry(&self) -> Option<Arc<InterceptorRegistry>> {
        self.interceptors.lock().ok().and_then(|g| g.clone())
    }

    /// CCM 4.0 §2 CP8 — activates a messaging policy on the ORB.
    /// Cross-ref `corba-ccm::orb_extensions::MessagingPolicy`.
    pub fn with_messaging_policy(&self, p: MessagingPolicy) {
        if let Ok(mut g) = self.messaging_policies.lock()
            && !g.contains(&p)
        {
            g.push(p);
        }
    }

    /// CCM 4.0 §2 CP8 — list of active messaging policies.
    #[must_use]
    pub fn messaging_policies(&self) -> Vec<MessagingPolicy> {
        self.messaging_policies
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    /// Part 1 §18.4 — the `CompressionManager` of this ORB (also its
    /// initial reference `"CompressionManager"`), with the built-in zlib
    /// and gzip factories.
    #[must_use]
    pub fn compression_manager(&self) -> Arc<CompressionManager> {
        Arc::clone(&self.compression_manager)
    }

    /// CCM 4.0 §2 CP8 — sets the ORB-wide compressor and level
    /// (Part 1 §18.5 `CompressorIdLevel`); `COMPRESSORID_NONE` turns
    /// compression off.
    ///
    /// # Errors
    /// [`CompressionConfigError::UnknownCompressorId`] when the
    /// `CompressionManager` has no factory for the id,
    /// [`CompressionConfigError::System`] with `BAD_PARAM` minor 44 for a
    /// level above 9.
    pub fn with_compression(&self, c: CompressorIdLevel) -> Result<(), CompressionConfigError> {
        check_level(c.compression_level).map_err(CompressionConfigError::System)?;
        if c.compressor_id != COMPRESSORID_NONE {
            self.compression_manager
                .get_factory(c.compressor_id)
                .map_err(|UnknownCompressorId| CompressionConfigError::UnknownCompressorId)?;
        }
        let mut g = self
            .compression
            .lock()
            .map_err(|_| CompressionConfigError::System(internal()))?;
        *g = c;
        Ok(())
    }

    /// CCM 4.0 §2 CP8 — the ORB-wide compressor and level.
    #[must_use]
    pub fn compression(&self) -> CompressorIdLevel {
        self.compression
            .lock()
            .map(|g| *g)
            .unwrap_or(NO_COMPRESSION)
    }
}

/// No compression: `COMPRESSORID_NONE`, level 0.
const NO_COMPRESSION: CompressorIdLevel = CompressorIdLevel {
    compressor_id: COMPRESSORID_NONE,
    compression_level: 0,
};

/// Why [`Orb::with_compression`] refused a compressor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompressionConfigError {
    /// The `CompressionManager` has no factory for the id (§18.4).
    UnknownCompressorId,
    /// A system exception (`BAD_PARAM` minor 44 for a level above 9).
    System(SystemException),
}

// ---------------------------------------------------------------------------
// §8.5 ORB_init, initial references, object URLs
// ---------------------------------------------------------------------------

/// `CORBA::Object` as the ORB hands it out (§8.5.2): a reference to an
/// object, possibly in another process, or a locality-constrained
/// object of this ORB (`LocalObject`, §8.3.14) such as a factory the ORB
/// provides.
#[derive(Clone)]
pub enum ObjectRef {
    /// An object reference (IOR).
    Reference(ObjectReference),
    /// A local object; [`ObjectRef::local`] narrows it to its type.
    Local(Arc<dyn Any + Send + Sync>),
    /// A group reference (Part 2 §11.2.3): a UIPMC profile with a
    /// `TAG_GROUP` component and possibly gateway IIOP profiles, kept as
    /// the whole IOR, and its client-side policy overrides (§8.3.9; Part 2
    /// §11.2.6.10: `set_policy_overrides` is unchanged for a group).
    Group(Ior, Vec<PolicyOverride>),
}

impl ObjectRef {
    /// A local object.
    #[must_use]
    pub fn local_object<T: Any + Send + Sync>(obj: Arc<T>) -> Self {
        Self::Local(obj)
    }

    /// The object reference, when this is one.
    #[must_use]
    pub fn reference(&self) -> Option<&ObjectReference> {
        match self {
            Self::Reference(r) => Some(r),
            Self::Local(_) | Self::Group(..) => None,
        }
    }

    /// The IOR of a group reference (Part 2 §11.2.3), when this is one.
    #[must_use]
    pub fn group(&self) -> Option<&Ior> {
        match self {
            Self::Group(ior, _) => Some(ior),
            Self::Reference(_) | Self::Local(_) => None,
        }
    }

    /// Narrows a local object to `T`; `None` for a reference or another
    /// type.
    #[must_use]
    pub fn local<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        match self {
            Self::Local(obj) => Arc::clone(obj).downcast::<T>().ok(),
            Self::Reference(_) | Self::Group(..) => None,
        }
    }

    /// §8.3.3 — `true` for the nil reference: no type id and no profile.
    /// A group reference is nil when it has no profile (Part 2
    /// §11.2.6.10).
    #[must_use]
    pub fn is_nil(&self) -> bool {
        match self {
            Self::Reference(r) => {
                r.type_id.is_empty() && r.iiop_profile.is_empty() && r.profiles.is_empty()
            }
            Self::Local(_) => false,
            Self::Group(ior, _) => ior.profiles.is_empty(),
        }
    }

    /// §8.3.9.1 — `set_policy_overrides(policies, set_add)`: a copy of
    /// this reference whose object-scope overrides are `policies` applied
    /// by `set_add`; this reference keeps its own.
    ///
    /// # Errors
    /// [`OverrideError`] as
    /// [`crate::policy_management::check_overrides`]; `NO_IMPLEMENT`
    /// minor 8 for a local object (§8.3.14).
    pub fn set_policy_overrides(
        &self,
        policies: &[Policy],
        set_add: SetOverrideType,
    ) -> Result<Self, OverrideError> {
        match self {
            Self::Reference(r) => set_object_overrides(r, policies, set_add).map(Self::Reference),
            Self::Local(_) => Err(OverrideError::System(local_no_implement())),
            Self::Group(ior, overrides) => {
                let carrier = ObjectReference {
                    policy_overrides: overrides.clone(),
                    ..ObjectReference::default()
                };
                let with = set_object_overrides(&carrier, policies, set_add)?;
                Ok(Self::Group(ior.clone(), with.policy_overrides))
            }
        }
    }

    /// §8.3.8.3 — `get_policy_overrides(types)`: the object-scope
    /// overrides of the types `ts`, all of them for an empty `ts`.
    ///
    /// # Errors
    /// `NO_IMPLEMENT` minor 8 for a local object (§8.3.14); `MARSHAL` for
    /// an override that does not decode.
    pub fn get_policy_overrides(&self, ts: &[u32]) -> Result<Vec<Policy>, SystemException> {
        match self {
            Self::Reference(r) => get_object_overrides(r, ts),
            Self::Local(_) => Err(local_no_implement()),
            Self::Group(_, overrides) => get_object_overrides(
                &ObjectReference {
                    policy_overrides: overrides.clone(),
                    ..ObjectReference::default()
                },
                ts,
            ),
        }
    }
}

/// `NO_IMPLEMENT` minor 8: an operation `LocalObject` does not provide
/// (§8.3.14; Annex A.6).
fn local_no_implement() -> SystemException {
    SystemException::omg(SystemExceptionKind::NoImplement, 8, CompletionStatus::No)
}

fn inv_policy() -> SystemException {
    SystemException::new(SystemExceptionKind::InvPolicy, 0, CompletionStatus::No)
}

impl core::fmt::Debug for ObjectRef {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Reference(r) => f.debug_tuple("Reference").field(r).finish(),
            Self::Local(_) => f.write_str("Local(..)"),
            Self::Group(ior, overrides) => {
                f.debug_tuple("Group").field(ior).field(overrides).finish()
            }
        }
    }
}

/// What `resolve_initial_references` and `register_initial_reference`
/// raise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    /// `CORBA::ORB::InvalidName` (§8.5.2, §16.10).
    InvalidName,
    /// A system exception.
    System(SystemException),
}

impl From<SystemException> for ResolveError {
    fn from(e: SystemException) -> Self {
        Self::System(e)
    }
}

impl core::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidName => f.write_str("CORBA::ORB::InvalidName"),
            Self::System(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ResolveError {}

/// The `-ORB<suffix>` arguments of `ORB_init` this ORB recognises
/// (§8.5.1, §8.5.3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrbArguments {
    /// `-ORBid`.
    pub orb_id: Option<String>,
    /// `-ORBInitRef <ObjectID>=<ObjectURL>`, in the order given.
    pub init_refs: Vec<(String, String)>,
    /// `-ORBDefaultInitRef <URL>`.
    pub default_init_ref: Option<String>,
    /// `-ORBServerId`.
    pub server_id: Option<String>,
    /// `-ORBListenEndpoints`, every occurrence in the order given.
    pub listen_endpoints: Vec<String>,
    /// `-ORBNoProprietaryActivation`.
    pub no_proprietary_activation: bool,
}

/// The suffixes of the recognised `-ORB<suffix>` parameters; the flag
/// takes no value.
const ORB_SUFFIXES: [&str; 6] = [
    "NoProprietaryActivation",
    "ListenEndpoints",
    "DefaultInitRef",
    "ServerId",
    "InitRef",
    "id",
];

impl OrbArguments {
    /// Reads the `-ORB<suffix>` parameters out of `args` and removes
    /// them with their values (§8.5.1). A parameter is either one
    /// string, `-ORB<suffix><optional white space><value>`, or two,
    /// `-ORB<suffix>` and `<value>`. Strings that do not start with
    /// `-ORB` stay in `args` in their order.
    ///
    /// # Errors
    /// `BAD_PARAM` for a `-ORB<suffix>` this ORB does not recognise, a
    /// missing value, an `-ORBInitRef` without `<ObjectID>=` or with a
    /// URL `string_to_object` would refuse (or `corbaloc:rir`,
    /// §8.5.3.2), and an `-ORBDefaultInitRef` that is not a `corbaloc:`
    /// or `corbaname:` URL a key can be appended to.
    pub fn take_from(args: &mut Vec<String>) -> Result<Self, SystemException> {
        let mut out = Self::default();
        let mut kept = Vec::with_capacity(args.len());
        let mut iter = core::mem::take(args).into_iter();
        while let Some(arg) = iter.next() {
            let Some(rest) = arg.strip_prefix("-ORB") else {
                kept.push(arg);
                continue;
            };
            let suffix = ORB_SUFFIXES
                .iter()
                .find(|s| rest.starts_with(**s))
                .ok_or_else(|| bad_param(0))?;
            let inline = rest[suffix.len()..].trim_start();
            if *suffix == "NoProprietaryActivation" {
                if !inline.is_empty() {
                    return Err(bad_param(0));
                }
                out.no_proprietary_activation = true;
                continue;
            }
            let value = if inline.is_empty() {
                iter.next().ok_or_else(|| bad_param(0))?
            } else {
                inline.to_string()
            };
            match *suffix {
                "id" => out.orb_id = Some(value),
                "ServerId" => out.server_id = Some(value),
                "ListenEndpoints" => out.listen_endpoints.push(value),
                "InitRef" => {
                    let (id, url) = value.split_once('=').ok_or_else(|| bad_param(0))?;
                    if id.is_empty() {
                        return Err(bad_param(0));
                    }
                    validate_init_ref_url(url)?;
                    out.init_refs.push((id.to_string(), url.to_string()));
                }
                _ => {
                    validate_object_url(&default_init_ref_url(&value, "NameService"))?;
                    let scheme = value.get(..10).map(str::to_ascii_lowercase);
                    let scheme = scheme.as_deref().unwrap_or_default();
                    if !scheme.starts_with("corbaloc:") && scheme != "corbaname:" {
                        return Err(bad_param(7));
                    }
                    // `corbaloc:rir:` + `/<id>` names the id being resolved
                    // itself: it can never yield a reference.
                    if scheme.starts_with("corbaloc:") && is_corbaloc_rir(&value) {
                        return Err(bad_param(8));
                    }
                    out.default_init_ref = Some(value);
                }
            }
        }
        *args = kept;
        Ok(out)
    }
}

std::thread_local! {
    /// The initial reference ids this thread is resolving, with the ORB
    /// they are resolved on.
    static RESOLVING: core::cell::RefCell<Vec<(usize, String)>> =
        const { core::cell::RefCell::new(Vec::new()) };
}

/// One id in [`RESOLVING`] while it is resolved; leaves it on drop.
struct Resolving {
    orb: usize,
    id: String,
}

impl Resolving {
    /// Enters `id` for `orb`; `None` when this thread is resolving it on
    /// that ORB already.
    fn enter(orb: &Orb, id: &str) -> Option<Self> {
        let key = core::ptr::from_ref(orb) as usize;
        RESOLVING.with(|r| {
            let mut r = r.borrow_mut();
            if r.iter().any(|(o, i)| *o == key && i == id) {
                return None;
            }
            r.push((key, id.to_string()));
            Some(Self {
                orb: key,
                id: id.to_string(),
            })
        })
    }
}

impl Drop for Resolving {
    fn drop(&mut self) {
        RESOLVING.with(|r| {
            let mut r = r.borrow_mut();
            if let Some(pos) = r.iter().rposition(|(o, i)| *o == self.orb && *i == self.id) {
                r.remove(pos);
            }
        });
    }
}

/// The ORBs `orb_init` created, by ORBid (§8.5.1: the same ORBid gives
/// the same ORB), and the ORBids an instantiating call is making, with
/// the thread that runs their initializers.
struct OrbTable {
    orbs: BTreeMap<String, Arc<Orb>>,
    initializing: BTreeMap<String, std::thread::ThreadId>,
}

static ORB_TABLE: Mutex<OrbTable> = Mutex::new(OrbTable {
    orbs: BTreeMap::new(),
    initializing: BTreeMap::new(),
});

/// Signalled when an instantiating `orb_init` ends, with an ORB or
/// without one.
static ORB_TABLE_CHANGED: Condvar = Condvar::new();

/// An ORBid an instantiating `orb_init` is making: when the call ends,
/// by success, error or panic, the id is free again and the callers
/// waiting for it look at the table again.
struct Initializing(String);

impl Drop for Initializing {
    fn drop(&mut self) {
        ORB_TABLE
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .initializing
            .remove(&self.0);
        ORB_TABLE_CHANGED.notify_all();
    }
}

/// §8.5.1 — `CORBA::ORB_init(argv, orb_identifier)`.
///
/// The recognised `-ORB<suffix>` parameters are removed from `args`. An
/// empty `orb_identifier` takes the `-ORBid` value, else the default
/// ORB (empty ORBid); a non-empty one ignores `-ORBid`. A second call
/// with the same ORBid returns the same ORB and adds its `-ORBInitRef`
/// and `-ORBDefaultInitRef` values; after [`Orb::destroy`] the ORBid
/// creates a new ORB. A new ORB runs the registered `ORBInitializer`s
/// (§16.9.3) and has `PICurrent`, `DynAnyFactory` (§13.2.1),
/// `CompressionManager` (§18.4), `ORBPolicyManager` and `PolicyCurrent`
/// (§8.9.1) and `TransactionCurrent` (the local `CosTransactions::Current`
/// of Part 3 §6.13.2.2) as initial references.
///
/// The initializers run without a lock on the table of ORBs: one may
/// call `orb_init` for another ORBid, destroy another ORB or resolve an
/// initial reference over the network meanwhile, and `orb_init` and
/// [`Orb::destroy`] of other threads go on. A call for an ORBid whose
/// ORB another thread is making waits for that call and gets its ORB.
///
/// # Errors
/// `BAD_PARAM` as [`OrbArguments::take_from`]; `BAD_INV_ORDER` when an
/// initializer of the ORB being made asks `orb_init` for that ORB, which
/// does not exist before its initializers end.
pub fn orb_init(args: &mut Vec<String>, orb_identifier: &str) -> Result<Arc<Orb>, SystemException> {
    let original = args.clone();
    let parsed = OrbArguments::take_from(args)?;
    let id = if orb_identifier.is_empty() {
        parsed.orb_id.clone().unwrap_or_default()
    } else {
        orb_identifier.to_string()
    };
    let me = std::thread::current().id();
    let making = {
        let mut table = ORB_TABLE.lock().map_err(|_| internal())?;
        loop {
            if let Some(orb) = table.orbs.get(&id) {
                if let Ok(mut g) = orb.init_refs.lock() {
                    g.extend(parsed.init_refs);
                }
                if let Some(d) = parsed.default_init_ref
                    && let Ok(mut g) = orb.default_init_ref.lock()
                {
                    *g = Some(d);
                }
                return Ok(Arc::clone(orb));
            }
            match table.initializing.get(&id).copied() {
                None => break,
                Some(owner) if owner == me => {
                    return Err(SystemException::new(
                        SystemExceptionKind::BadInvOrder,
                        0,
                        CompletionStatus::No,
                    ));
                }
                Some(_) => {
                    table = ORB_TABLE_CHANGED.wait(table).map_err(|_| internal())?;
                }
            }
        }
        table.initializing.insert(id.clone(), me);
        Initializing(id.clone())
    };
    let orb = Arc::new(Orb::with_arguments(
        id.clone(),
        parsed,
        ThreadingMode::ThreadPerRequest,
    ));
    orb.configure_initial_reference("PICurrent", ObjectRef::local_object(orb.pi_current()));
    orb.configure_initial_reference(
        "DynAnyFactory",
        ObjectRef::local_object(Arc::new(zerodds_corba_rt::DynAnyFactory)),
    );
    orb.configure_initial_reference(
        "CompressionManager",
        ObjectRef::local_object(orb.compression_manager()),
    );
    // CORBA 3.3 Part 2 §7.8: "The CodecFactory is obtained through a call
    // to ORB::resolve_initial_references ("CodecFactory")".
    orb.configure_initial_reference(
        crate::iop_codec::CODEC_FACTORY_INITIAL_REFERENCE,
        ObjectRef::local_object(Arc::new(crate::iop_codec::CodecFactory)),
    );
    orb.configure_initial_reference(
        ORB_POLICY_MANAGER_INITIAL_REFERENCE,
        ObjectRef::local_object(orb.policy_manager()),
    );
    orb.configure_initial_reference(
        POLICY_CURRENT_INITIAL_REFERENCE,
        ObjectRef::local_object(orb.policy_current()),
    );
    orb.configure_initial_reference(
        zerodds_corba_cos_transactions::TRANSACTION_CURRENT_INITIAL_REFERENCE,
        ObjectRef::local_object(Arc::new(
            zerodds_corba_cos_transactions::TransactionCurrent::new(0),
        )),
    );
    orb.run_initializers(&original)?;
    ORB_TABLE
        .lock()
        .map_err(|_| internal())?
        .orbs
        .insert(id, Arc::clone(&orb));
    // The waiting callers find the ORB in the table.
    drop(making);
    Ok(orb)
}

/// `BAD_PARAM` with standard minor code `code`, `COMPLETED_NO`; minor 0
/// for code 0.
fn bad_param(code: u32) -> SystemException {
    if code == 0 {
        SystemException::new(SystemExceptionKind::BadParam, 0, CompletionStatus::No)
    } else {
        SystemException::omg(SystemExceptionKind::BadParam, code, CompletionStatus::No)
    }
}

fn internal() -> SystemException {
    SystemException::new(SystemExceptionKind::Internal, 0, CompletionStatus::No)
}

/// §8.2.5.5 — what an operation on a destroyed ORB raises.
fn object_not_exist() -> SystemException {
    SystemException::new(SystemExceptionKind::ObjectNotExist, 0, CompletionStatus::No)
}

fn non_nil(obj: ObjectRef) -> Result<ObjectRef, ResolveError> {
    if obj.is_nil() {
        Err(ResolveError::InvalidName)
    } else {
        Ok(obj)
    }
}

/// §8.5.3.3 — the URL `-ORBDefaultInitRef` gives for `id`: the base, a
/// `/` and the id.
fn default_init_ref_url(base: &str, id: &str) -> String {
    alloc::format!("{base}/{id}")
}

/// How many `file`, `http` and `ftp` URLs one `string_to_object` follows
/// before it gives up (a file naming itself).
pub const MAX_URL_INDIRECTIONS: usize = 8;

/// The scheme of `url` up to and including its `:`, in lower case.
fn scheme_of(url: &str) -> Option<String> {
    url.get(..url.find(':')? + 1).map(str::to_ascii_lowercase)
}

/// The syntax check of `string_to_object` without resolving anything; a
/// `file`, `http` or `ftp` URL is checked as a URL, not fetched.
fn validate_object_url(url: &str) -> Result<(), SystemException> {
    match scheme_of(url).as_deref() {
        Some("ior:") => ior_string_to_object(&url[4..]).map(|_| ()),
        Some("corbaloc:") => {
            if url[9..].starts_with("miop:") || url[9..].contains(",miop:") {
                return zerodds_corba_ior::miop::parse_miop_corbaloc(&alloc::format!(
                    "corbaloc:{}",
                    &url[9..]
                ))
                .map(|_| ())
                .map_err(|_| bad_param(8));
            }
            let addr = parse_corbaloc(url).map_err(|e| bad_param(e.bad_param_minor()))?;
            if addr.is_rir() {
                Ok(())
            } else {
                corbaloc_iiop_reference(&addr).map(|_| ())
            }
        }
        Some("corbaname:") => {
            let addr = parse_corbaname(url).map_err(|e| bad_param(e.bad_param_minor()))?;
            if !addr.address.is_rir() {
                corbaloc_iiop_reference(&addr.address)?;
            }
            if !addr.stringified_name.is_empty() {
                zerodds_corba_cosnaming::string_to_name(&addr.stringified_name)
                    .map_err(|_| bad_param(9))?;
            }
            Ok(())
        }
        Some("file:" | "http:" | "ftp:") => crate::object_url::FetchUrl::parse(url)
            .map(|_| ())
            .map_err(|e| bad_param(e.minor())),
        _ => Err(bad_param(7)),
    }
}

/// §8.5.3.2 — an `-ORBInitRef` URL: any scheme `string_to_object`
/// takes, except `corbaloc` with the `rir` protocol.
fn validate_init_ref_url(url: &str) -> Result<(), SystemException> {
    validate_object_url(url)?;
    if is_corbaloc_rir(url) {
        return Err(bad_param(8));
    }
    Ok(())
}

/// `true` for a `corbaloc` URL with a `rir` address.
fn is_corbaloc_rir(url: &str) -> bool {
    scheme_of(url).as_deref() == Some("corbaloc:") && parse_corbaloc(url).is_ok_and(|a| a.is_rir())
}

/// `IOR:<hex>` to a reference with every profile of the IOR.
fn ior_string_to_object(hex: &str) -> Result<ObjectRef, SystemException> {
    let ior = from_stringified(&alloc::format!("IOR:{hex}"))
        .map_err(|e| bad_param(e.bad_param_minor()))?;
    // Part 2 §11.2.3: a group reference keeps its UIPMC profile.
    if zerodds_corba_ior::miop::is_group_reference(&ior) {
        return Ok(ObjectRef::Group(ior, Vec::new()));
    }
    Ok(ObjectRef::Reference(reference_from_ior(&ior)))
}

/// An [`Ior`] as an [`ObjectReference`]: the type id and every profile in
/// its order (Part 2 §7.6.3, Full IOR conformance). The nil IOR gives the
/// nil reference. Whether a profile is usable is decided when the
/// reference is invoked (`IMP_LIMIT` minor 1 or `TRANSIENT` minor 2).
fn reference_from_ior(ior: &Ior) -> ObjectReference {
    if ior.is_nil() {
        return ObjectReference::default();
    }
    ObjectReference::from_profiles(
        ior.type_id.clone(),
        ior.profiles
            .iter()
            .map(|p| RawProfile {
                tag: p.tag.as_u32(),
                data: p.profile_data.clone(),
            })
            .collect(),
    )
}

/// A `corbaloc` with `iiop` addresses as a reference: one IIOP profile
/// per IIOP address, in the order of the URL, each with the version of
/// its address. Addresses of other protocols are skipped (§7.6.10.6).
/// The type id is empty: a `corbaloc` does not carry it.
fn corbaloc_iiop_reference(
    addr: &zerodds_corba_ior::CorbalocAddress,
) -> Result<ObjectReference, SystemException> {
    let mut profiles = Vec::new();
    for a in addr.iiop_addresses() {
        let body = IiopProfileBody::new(a.version, a.host.clone(), a.port, addr.object_key.clone());
        profiles.push(RawProfile {
            tag: ProfileId::InternetIop.as_u32(),
            data: body
                .encode_encapsulation(Endianness::Big)
                .map_err(|_| bad_param(8))?,
        });
    }
    if profiles.is_empty() {
        // No address of a protocol this ORB speaks.
        return Err(bad_param(8));
    }
    Ok(ObjectReference::from_profiles(String::new(), profiles))
}

// ===========================================================================
// Part 1 §9 Value Type — Custom-Marshal-Streams + Sending-Context
// ===========================================================================

/// Spec §9.5 — `StreamableValue` custom-marshal streams.
pub trait StreamableValue: Send + Sync {
    /// Repository ID of the value type (e.g. `IDL:demo/MyValue:1.0`).
    fn repository_id(&self) -> &str;
    /// Marshal operation: write the value state into the stream.
    fn marshal(&self) -> Vec<u8>;
    /// Unmarshal operation: load from stream bytes.
    ///
    /// # Errors
    /// `()` on invalid bytes.
    #[allow(clippy::result_unit_err)]
    fn unmarshal(&mut self, bytes: &[u8]) -> Result<(), ()>;
}

/// Spec §9.6 — `SendingContext::RunTime` (per-sender state).
#[derive(Debug, Clone, Default)]
pub struct SendingContext {
    /// Per-receiver truncation tracking.
    truncation_codebase: Option<String>,
    /// Code set (char encoding).
    code_set: u32,
}

impl SendingContext {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Spec §9.6 — sets the codebase URL (for truncation recovery).
    pub fn set_truncation_codebase(&mut self, url: impl Into<String>) {
        self.truncation_codebase = Some(url.into());
    }

    /// Get the code set (spec §9.6.1).
    #[must_use]
    pub fn code_set(&self) -> u32 {
        self.code_set
    }

    /// Set the code set.
    pub fn set_code_set(&mut self, cs: u32) {
        self.code_set = cs;
    }
}

// ===========================================================================
// Part 3 §12 IFR Metamodel + §13 CIF Metamodel — XMI-Emitter
// ===========================================================================

/// MOF-2.0 element for the XMI emitter (subset).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MofElement {
    /// `Class` — corresponds to IDL `interface`/`component`.
    Class {
        /// Name.
        name: String,
        /// Repository ID.
        repo_id: String,
        /// Inheritance.
        bases: Vec<String>,
    },
    /// `Property` — corresponds to an IDL attribute or struct member.
    Property {
        /// Name.
        name: String,
        /// Type reference.
        type_ref: String,
        /// `true` if read-only.
        read_only: bool,
    },
    /// `Operation` — corresponds to an IDL operation.
    Operation {
        /// Name.
        name: String,
        /// Return type.
        return_type: String,
        /// Parameter list.
        parameters: Vec<(String, String)>,
    },
    /// `ExtendedPortType` (Part 3 §7.6.5, Figure 7.11): a `porttype`, the
    /// aggregation of its `ProvidesDef`s and `UsesDef`s, parameterized by
    /// IDL types when it comes from a template module.
    ExtendedPortType {
        /// Name.
        name: String,
        /// Repository ID.
        repo_id: String,
        /// `ProvidesDef`s: (name, interface repository ID).
        provides: Vec<(String, String)>,
        /// `UsesDef`s: (name, interface repository ID, `is_multiple`).
        uses: Vec<(String, String, bool)>,
        /// `parameterization`: the IDL types of the template parameters.
        parameterization: Vec<String>,
    },
    /// `ExtendedPortDef` (§7.6.5): an extended port of a component, which
    /// refers to its `ExtendedPortType`.
    ExtendedPortDef {
        /// The component or connector the port belongs to.
        owner: String,
        /// The port's name.
        name: String,
        /// The repository ID of the `ExtendedPortType`.
        port_type: String,
        /// `mirrorport` rather than `port`.
        mirror: bool,
    },
    /// `ConnectorDef` (§7.6.5): a connector with its `ProvidesDef`s,
    /// `UsesDef`s and `ExtendedPortDef`s, parameterized by IDL types.
    ConnectorDef {
        /// Name.
        name: String,
        /// Repository ID.
        repo_id: String,
        /// The inherited connector (§7.2.3.3).
        bases: Vec<String>,
        /// `ProvidesDef`s: (name, interface repository ID).
        provides: Vec<(String, String)>,
        /// `UsesDef`s: (name, interface repository ID, `is_multiple`).
        uses: Vec<(String, String, bool)>,
        /// `ExtendedPortDef`s: (name, port type repository ID, mirror).
        extended_ports: Vec<(String, String, bool)>,
        /// `parameterization`: the IDL types of the template parameters.
        parameterization: Vec<String>,
    },
}

fn emit_basic_ports(
    out: &mut String,
    provides: &[(String, String)],
    uses: &[(String, String, bool)],
) {
    for (n, t) in provides {
        out.push_str(&alloc::format!(
            "      <CCM.ProvidesDef name=\"{n}\" interface=\"{t}\"/>\n"
        ));
    }
    for (n, t, m) in uses {
        out.push_str(&alloc::format!(
            "      <CCM.UsesDef name=\"{n}\" interface=\"{t}\" is_multiple=\"{m}\"/>\n"
        ));
    }
}

fn emit_parameterization(out: &mut String, parameterization: &[String]) {
    for t in parameterization {
        out.push_str(&alloc::format!(
            "      <CCM.Parameterization type=\"{t}\"/>\n"
        ));
    }
}

/// MOF-2.0 → XMI-1.2 emitter (subset for IFR + CIF metamodel).
#[derive(Debug, Clone, Default)]
pub struct XmiEmitter {
    elements: Vec<MofElement>,
}

impl XmiEmitter {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Spec Part 3 §12.3 / §13.x — adds a MOF element.
    pub fn add_element(&mut self, e: MofElement) {
        self.elements.push(e);
    }

    /// Number of elements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// `true` if there are no elements.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Emits XMI-1.2 XML (subset). The caller gets a
    /// spec-compliant XMI document string that a MOF tool
    /// (e.g. EMF) can consume.
    #[must_use]
    pub fn emit_xmi(&self) -> String {
        let mut out = String::new();
        out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        out.push_str("<XMI xmi.version=\"1.2\">\n");
        out.push_str("  <XMI.content>\n");
        for el in &self.elements {
            match el {
                MofElement::Class {
                    name,
                    repo_id,
                    bases,
                } => {
                    out.push_str(&alloc::format!(
                        "    <Mof.Class name=\"{name}\" repo_id=\"{repo_id}\">\n"
                    ));
                    for b in bases {
                        out.push_str(&alloc::format!("      <Mof.Inheritance base=\"{b}\"/>\n"));
                    }
                    out.push_str("    </Mof.Class>\n");
                }
                MofElement::Property {
                    name,
                    type_ref,
                    read_only,
                } => {
                    out.push_str(&alloc::format!(
                        "    <Mof.Property name=\"{name}\" type=\"{type_ref}\" read_only=\"{read_only}\"/>\n"
                    ));
                }
                MofElement::Operation {
                    name,
                    return_type,
                    parameters,
                } => {
                    out.push_str(&alloc::format!(
                        "    <Mof.Operation name=\"{name}\" return=\"{return_type}\">\n"
                    ));
                    for (pn, pt) in parameters {
                        out.push_str(&alloc::format!(
                            "      <Mof.Parameter name=\"{pn}\" type=\"{pt}\"/>\n"
                        ));
                    }
                    out.push_str("    </Mof.Operation>\n");
                }
                MofElement::ExtendedPortType {
                    name,
                    repo_id,
                    provides,
                    uses,
                    parameterization,
                } => {
                    out.push_str(&alloc::format!(
                        "    <CCM.ExtendedPortType name=\"{name}\" repo_id=\"{repo_id}\">\n"
                    ));
                    emit_basic_ports(&mut out, provides, uses);
                    emit_parameterization(&mut out, parameterization);
                    out.push_str("    </CCM.ExtendedPortType>\n");
                }
                MofElement::ExtendedPortDef {
                    owner,
                    name,
                    port_type,
                    mirror,
                } => {
                    out.push_str(&alloc::format!(
                        "    <CCM.ExtendedPortDef owner=\"{owner}\" name=\"{name}\" type=\"{port_type}\" mirror=\"{mirror}\"/>\n"
                    ));
                }
                MofElement::ConnectorDef {
                    name,
                    repo_id,
                    bases,
                    provides,
                    uses,
                    extended_ports,
                    parameterization,
                } => {
                    out.push_str(&alloc::format!(
                        "    <CCM.ConnectorDef name=\"{name}\" repo_id=\"{repo_id}\">\n"
                    ));
                    for b in bases {
                        out.push_str(&alloc::format!("      <Mof.Inheritance base=\"{b}\"/>\n"));
                    }
                    emit_basic_ports(&mut out, provides, uses);
                    for (n, t, m) in extended_ports {
                        out.push_str(&alloc::format!(
                            "      <CCM.ExtendedPortDef name=\"{n}\" type=\"{t}\" mirror=\"{m}\"/>\n"
                        ));
                    }
                    emit_parameterization(&mut out, parameterization);
                    out.push_str("    </CCM.ConnectorDef>\n");
                }
            }
        }
        out.push_str("  </XMI.content>\n");
        out.push_str("</XMI>\n");
        out
    }
}

/// Spec Part 3 §12 — IFR metamodel extension for CCM.
/// Wraps an XmiEmitter with CCM-specific preconfiguration
/// (component class as a Mof.Class with the marker `ccm:Component`).
pub struct IfrCcmMetamodel {
    inner: Arc<Mutex<XmiEmitter>>,
}

impl core::fmt::Debug for IfrCcmMetamodel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("IfrCcmMetamodel").finish()
    }
}

impl Default for IfrCcmMetamodel {
    fn default() -> Self {
        Self::new()
    }
}

impl IfrCcmMetamodel {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(XmiEmitter::new())),
        }
    }

    /// Adds a component class as a Mof.Class.
    pub fn add_component(&self, name: &str, repo_id: &str, bases: Vec<String>) {
        if let Ok(mut g) = self.inner.lock() {
            g.add_element(MofElement::Class {
                name: name.to_string(),
                repo_id: repo_id.to_string(),
                bases,
            });
        }
    }

    /// Part 3 §7.6.5 — adds a `porttype` as an `ExtendedPortType` with
    /// its `ProvidesDef`s and `UsesDef`s; `parameterization` names the
    /// IDL types of the template parameters it was instantiated with.
    pub fn add_port_type(
        &self,
        port_type: &crate::connector::PortTypeDef,
        parameterization: Vec<String>,
    ) {
        use crate::connector::PortTypeMember;
        let mut provides = Vec::new();
        let mut uses = Vec::new();
        for m in &port_type.members {
            match m {
                PortTypeMember::Provides { name, interface_id } => {
                    provides.push((name.clone(), interface_id.clone()));
                }
                PortTypeMember::Uses {
                    name,
                    interface_id,
                    multiple,
                } => uses.push((name.clone(), interface_id.clone(), *multiple)),
                PortTypeMember::Attribute(_) => {}
            }
        }
        if let Ok(mut g) = self.inner.lock() {
            g.add_element(MofElement::ExtendedPortType {
                name: port_type.name.clone(),
                repo_id: port_type.repository_id.clone(),
                provides,
                uses,
                parameterization,
            });
        }
    }

    /// Part 3 §7.6.5 — an `ExtendedPortDef` of a component.
    pub fn add_extended_port(&self, owner: &str, name: &str, port_type: &str, mirror: bool) {
        if let Ok(mut g) = self.inner.lock() {
            g.add_element(MofElement::ExtendedPortDef {
                owner: owner.to_string(),
                name: name.to_string(),
                port_type: port_type.to_string(),
                mirror,
            });
        }
    }

    /// Part 3 §7.6.5 — adds a connector as a `ConnectorDef` with its
    /// basic and extended ports.
    pub fn add_connector(
        &self,
        connector: &crate::connector::ConnectorDef,
        parameterization: Vec<String>,
    ) {
        use crate::connector::ConnectorMember;
        let mut provides = Vec::new();
        let mut uses = Vec::new();
        let mut extended_ports = Vec::new();
        for m in &connector.members {
            match m {
                ConnectorMember::Provides { name, interface_id } => {
                    provides.push((name.clone(), interface_id.clone()));
                }
                ConnectorMember::Uses {
                    name,
                    interface_id,
                    multiple,
                } => uses.push((name.clone(), interface_id.clone(), *multiple)),
                ConnectorMember::Port { name, port_type_id } => {
                    extended_ports.push((name.clone(), port_type_id.clone(), false));
                }
                ConnectorMember::MirrorPort { name, port_type_id } => {
                    extended_ports.push((name.clone(), port_type_id.clone(), true));
                }
                ConnectorMember::Attribute(_) => {}
            }
        }
        if let Ok(mut g) = self.inner.lock() {
            g.add_element(MofElement::ConnectorDef {
                name: connector.name.clone(),
                repo_id: connector.repository_id.clone(),
                bases: connector.base.iter().cloned().collect(),
                provides,
                uses,
                extended_ports,
                parameterization,
            });
        }
    }

    /// Emits XMI for the entire component model.
    #[must_use]
    pub fn emit_xmi(&self) -> String {
        self.inner.lock().map(|g| g.emit_xmi()).unwrap_or_default()
    }

    /// Number of registered elements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.lock().map(|g| g.len()).unwrap_or(0)
    }

    /// `true` if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Spec Part 3 §12.3 — loads a `corba-ir::Repository` into the
    /// metamodel emitter. Each top-level definition is registered as a
    /// Mof.Class; sub-contents (module hierarchy) are
    /// attached recursively.
    pub fn ingest_repository(&self, repo: &zerodds_corba_ir::Repository) {
        if let Ok(mut g) = self.inner.lock() {
            for id in repo.ids() {
                if let Some(def) = repo.lookup_id(&id) {
                    ingest_definition_into(&mut g, def);
                }
            }
        }
    }

    /// Constructor with repository ingestion.
    #[must_use]
    pub fn from_repository(repo: &zerodds_corba_ir::Repository) -> Self {
        let m = Self::new();
        m.ingest_repository(repo);
        m
    }
}
/// zerodds-lint: recursion-depth 64 (ingest_definition_into bounded by AST depth)
fn ingest_definition_into(emitter: &mut XmiEmitter, def: &zerodds_corba_ir::Definition) {
    use zerodds_corba_ir::DefinitionKind;
    let bases: Vec<String> = Vec::new();
    match def.kind {
        DefinitionKind::Interface
        | DefinitionKind::AbstractInterface
        | DefinitionKind::LocalInterface
        | DefinitionKind::Value
        | DefinitionKind::ValueBox
        | DefinitionKind::Module
        | DefinitionKind::Struct
        | DefinitionKind::Union
        | DefinitionKind::Enum
        | DefinitionKind::Exception => {
            emitter.add_element(MofElement::Class {
                name: def.name.clone(),
                repo_id: def.repository_id.clone(),
                bases,
            });
        }
        DefinitionKind::Operation => {
            emitter.add_element(MofElement::Operation {
                name: def.name.clone(),
                return_type: alloc::format!("IDL:{}:{}", def.name, def.version),
                parameters: Vec::new(),
            });
        }
        DefinitionKind::Attribute | DefinitionKind::Constant => {
            emitter.add_element(MofElement::Property {
                name: def.name.clone(),
                type_ref: def.repository_id.clone(),
                read_only: matches!(def.kind, DefinitionKind::Constant),
            });
        }
        _ => {
            emitter.add_element(MofElement::Class {
                name: def.name.clone(),
                repo_id: def.repository_id.clone(),
                bases,
            });
        }
    }
    for child in &def.contents {
        ingest_definition_into(emitter, child);
    }
}

/// The ZIOP policies (CORBA 3.3 Part 2 §12.3, §12.5) as
/// `CORBA::ORB::create_policy` makes them: `None` for another policy
/// type; else the policy, or `BAD_POLICY_VALUE` when `value` is not of the
/// policy's value type — `boolean` for `CompressionEnablingPolicy` (64),
/// a sequence of `Compression::CompressorIdLevel` structs with levels up
/// to 9 for `CompressorIdLevelListPolicy` (65), `unsigned long` for
/// `CompressionLowValuePolicy` (66), a finite, non-negative `float` for
/// `CompressionMinRatioPolicy` (67).
#[must_use]
pub fn ziop_policy(policy_type: u32, value: &CorbaAny) -> Option<Result<Policy, PolicyError>> {
    use zerodds_corba_giop::ziop as z;
    use zerodds_corba_rt::any::AnyValue;
    let valid = match policy_type {
        z::COMPRESSION_ENABLING_POLICY_ID => matches!(value.0, AnyValue::Boolean(_)),
        z::COMPRESSOR_ID_LEVEL_LIST_POLICY_ID => compressor_list_of(&value.0).is_some(),
        z::COMPRESSION_LOW_VALUE_POLICY_ID => matches!(value.0, AnyValue::ULong(_)),
        z::COMPRESSION_MIN_RATIO_POLICY_ID => {
            matches!(value.0, AnyValue::Float(r) if r.is_finite() && r >= 0.0)
        }
        _ => return None,
    };
    Some(if valid {
        Ok(Policy {
            policy_type,
            value: value.clone(),
        })
    } else {
        Err(PolicyError {
            reason: PolicyError::BAD_POLICY_VALUE,
        })
    })
}

/// The `Compression::CompressorIdLevelList` in an any: a sequence of
/// structs with the `unsigned short` members `compressor_id` and
/// `compression_level` (level at most 9).
fn compressor_list_of(
    value: &zerodds_corba_rt::any::AnyValue,
) -> Option<zerodds_corba_giop::ziop::CompressorIdLevelList> {
    use zerodds_corba_rt::any::AnyValue;
    let AnyValue::Seq { items, .. } = value else {
        return None;
    };
    items
        .iter()
        .map(|item| match item {
            AnyValue::Struct { members, .. } => match members.as_slice() {
                [(_, AnyValue::UShort(id)), (_, AnyValue::UShort(level))]
                    if *level <= crate::compression::MAX_COMPRESSION_LEVEL =>
                {
                    Some(CompressorIdLevel {
                        compressor_id: *id,
                        compression_level: *level,
                    })
                }
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// The [`ZiopPolicies`](zerodds_corba_giop::ziop::ZiopPolicies) of a
/// policy list made by [`ziop_policy`] (Part 2 §12.3): each ZIOP policy
/// sets its value, a policy that is not in the list keeps the ORB
/// default, other policy types are not ZIOP's and stay with their owners.
///
/// # Errors
/// `BAD_POLICY_VALUE` for a ZIOP policy whose value is not of its type,
/// `BAD_POLICY` (0) when a ZIOP policy type is in the list twice.
pub fn ziop_policies(
    policies: &[Policy],
) -> Result<zerodds_corba_giop::ziop::ZiopPolicies, PolicyError> {
    apply_ziop_policies(zerodds_corba_giop::ziop::ZiopPolicies::default(), policies)
}

/// The four ZIOP policy types (Part 2 §12.3).
const ZIOP_POLICY_TYPES: [u32; 4] = [
    zerodds_corba_giop::ziop::COMPRESSION_ENABLING_POLICY_ID,
    zerodds_corba_giop::ziop::COMPRESSOR_ID_LEVEL_LIST_POLICY_ID,
    zerodds_corba_giop::ziop::COMPRESSION_LOW_VALUE_POLICY_ID,
    zerodds_corba_giop::ziop::COMPRESSION_MIN_RATIO_POLICY_ID,
];

/// The ZIOP policies of a client for an invocation on `reference` (Part
/// 2 §12.2: the client sets them "at ORB, thread or reference level"):
/// for each ZIOP policy type the effective override (Part 1 §8.3.8.2:
/// the object scope of `reference`, else the thread scope of `current`,
/// else the ORB scope of `orb`) over `base` (the client's own policies,
/// else the ORB defaults). `base` unchanged when no scope overrides a
/// ZIOP policy.
///
/// # Errors
/// `MARSHAL` for an override of `reference` that does not decode,
/// `INV_POLICY` when an override is not a ZIOP policy value.
pub fn client_ziop_policies(
    reference: &ObjectReference,
    current: Option<&PolicyCurrent>,
    orb: Option<&PolicyManager>,
    base: Option<&zerodds_corba_giop::ziop::ZiopPolicies>,
) -> Result<Option<zerodds_corba_giop::ziop::ZiopPolicies>, SystemException> {
    let mut effective = Vec::new();
    for t in ZIOP_POLICY_TYPES {
        if let Some(p) = client_policy(reference, current, orb, t)? {
            effective.push(p);
        }
    }
    if effective.is_empty() {
        return Ok(base.cloned());
    }
    apply_ziop_policies(base.cloned().unwrap_or_default(), &effective)
        .map(Some)
        .map_err(|_| inv_policy())
}

/// [`ziop_policies`] over `base` instead of the ORB defaults.
///
/// # Errors
/// As [`ziop_policies`].
pub fn apply_ziop_policies(
    base: zerodds_corba_giop::ziop::ZiopPolicies,
    policies: &[Policy],
) -> Result<zerodds_corba_giop::ziop::ZiopPolicies, PolicyError> {
    use zerodds_corba_giop::ziop as z;
    use zerodds_corba_rt::any::AnyValue;
    let bad = PolicyError {
        reason: PolicyError::BAD_POLICY_VALUE,
    };
    let mut out = base;
    let mut seen = Vec::new();
    for p in policies {
        if ziop_policy(p.policy_type, &p.value).is_none() {
            continue;
        }
        if seen.contains(&p.policy_type) {
            return Err(PolicyError {
                reason: PolicyError::BAD_POLICY,
            });
        }
        seen.push(p.policy_type);
        match (p.policy_type, &p.value.0) {
            (z::COMPRESSION_ENABLING_POLICY_ID, AnyValue::Boolean(b)) => {
                out.compression_enabled = *b;
            }
            (z::COMPRESSOR_ID_LEVEL_LIST_POLICY_ID, v) => {
                out.compressor_ids = compressor_list_of(v).ok_or(bad)?;
            }
            (z::COMPRESSION_LOW_VALUE_POLICY_ID, AnyValue::ULong(n)) => out.low_value = *n,
            (z::COMPRESSION_MIN_RATIO_POLICY_ID, AnyValue::Float(r))
                if r.is_finite() && *r >= 0.0 =>
            {
                out.min_ratio = Some(*r);
            }
            _ => return Err(bad),
        }
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_corba_iiop::IiopVersion;

    // §8 ORB Interface
    #[test]
    fn orb_init_yields_running() {
        let o = Orb::init(ThreadingMode::SingleThreaded);
        assert_eq!(o.state(), OrbState::Running);
    }

    #[test]
    fn orb_shutdown_transitions_to_shutting_down() {
        let o = Orb::init(ThreadingMode::SingleThreaded);
        o.shutdown(true).unwrap();
        assert_eq!(o.state(), OrbState::ShuttingDown);
    }

    #[test]
    fn corba_p1_8_2_5_5_a_destroyed_orb_raises_object_not_exist() {
        let expected =
            SystemException::new(SystemExceptionKind::ObjectNotExist, 0, CompletionStatus::No);
        // destroy on a running ORB shuts it down first, then destroys it.
        let o = Orb::init(ThreadingMode::SingleThreaded);
        o.destroy().unwrap();
        assert_eq!(o.state(), OrbState::Destroyed);
        assert_eq!(o.list_initial_services().unwrap_err(), expected);
        assert_eq!(
            o.resolve_initial_references("PICurrent").unwrap_err(),
            ResolveError::System(expected)
        );
        assert_eq!(sys(o.string_to_object("IOR:00")), expected);
        assert_eq!(
            o.object_to_string(&ObjectRef::Reference(ObjectReference::default()))
                .unwrap_err(),
            expected
        );
        assert_eq!(o.shutdown(false).unwrap_err(), expected);
        assert_eq!(o.destroy().unwrap_err(), expected);
        // A shut down ORB raises BAD_INV_ORDER 4 until it is destroyed.
        let p = Orb::init(ThreadingMode::SingleThreaded);
        p.shutdown(false).unwrap();
        assert_eq!(p.list_initial_services().unwrap_err().omg_minor(), Some(4));
        p.destroy().unwrap();
        assert_eq!(p.list_initial_services().unwrap_err(), expected);
    }

    #[test]
    fn orb_threading_mode_preserved() {
        let o = Orb::init(ThreadingMode::ThreadPool { size: 8 });
        assert_eq!(o.threading(), ThreadingMode::ThreadPool { size: 8 });
    }

    // §9 Value Type
    struct DummyValue {
        state: u32,
    }

    impl StreamableValue for DummyValue {
        fn repository_id(&self) -> &str {
            "IDL:demo/DummyValue:1.0"
        }
        fn marshal(&self) -> Vec<u8> {
            self.state.to_be_bytes().to_vec()
        }
        fn unmarshal(&mut self, bytes: &[u8]) -> Result<(), ()> {
            if bytes.len() < 4 {
                return Err(());
            }
            self.state = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            Ok(())
        }
    }

    #[test]
    fn streamable_value_round_trip() {
        let v = DummyValue { state: 0xDEADBEEF };
        let bytes = v.marshal();
        let mut v2 = DummyValue { state: 0 };
        v2.unmarshal(&bytes).expect("ok");
        assert_eq!(v2.state, 0xDEADBEEF);
    }

    #[test]
    fn streamable_value_unmarshal_truncated_rejected() {
        let mut v = DummyValue { state: 0 };
        assert!(v.unmarshal(&[0x01]).is_err());
    }

    #[test]
    fn sending_context_default_code_set_zero() {
        let sc = SendingContext::new();
        assert_eq!(sc.code_set(), 0);
    }

    #[test]
    fn sending_context_codebase_round_trip() {
        let mut sc = SendingContext::new();
        sc.set_truncation_codebase("http://example.com/codebase");
        sc.set_code_set(0x10001);
        assert_eq!(sc.code_set(), 0x10001);
    }

    // Part 3 §12 IFR + §13 CIF Metamodel — XMI
    #[test]
    fn xmi_emitter_empty_yields_minimal_doc() {
        let e = XmiEmitter::new();
        assert!(e.is_empty());
        let xmi = e.emit_xmi();
        assert!(xmi.contains("<?xml version=\"1.0\""));
        assert!(xmi.contains("<XMI xmi.version=\"1.2\">"));
    }

    #[test]
    fn xmi_emitter_class_with_inheritance() {
        let mut e = XmiEmitter::new();
        e.add_element(MofElement::Class {
            name: "Trader".into(),
            repo_id: "IDL:demo/Trader:1.0".into(),
            bases: alloc::vec!["IDL:Components/CCMObject:1.0".into()],
        });
        let xmi = e.emit_xmi();
        assert!(xmi.contains("name=\"Trader\""));
        assert!(xmi.contains("base=\"IDL:Components/CCMObject:1.0\""));
    }

    #[test]
    fn xmi_emitter_property_emits() {
        let mut e = XmiEmitter::new();
        e.add_element(MofElement::Property {
            name: "version".into(),
            type_ref: "string".into(),
            read_only: true,
        });
        let xmi = e.emit_xmi();
        assert!(xmi.contains("name=\"version\""));
        assert!(xmi.contains("read_only=\"true\""));
    }

    #[test]
    fn xmi_emitter_operation_with_parameters() {
        let mut e = XmiEmitter::new();
        e.add_element(MofElement::Operation {
            name: "compute".into(),
            return_type: "long".into(),
            parameters: alloc::vec![("x".into(), "long".into()), ("y".into(), "long".into()),],
        });
        let xmi = e.emit_xmi();
        assert!(xmi.contains("name=\"compute\""));
        assert!(xmi.contains("name=\"x\""));
        assert!(xmi.contains("name=\"y\""));
    }

    #[test]
    fn ifr_ccm_metamodel_add_component() {
        let m = IfrCcmMetamodel::new();
        assert!(m.is_empty());
        m.add_component(
            "Trader",
            "IDL:demo/Trader:1.0",
            alloc::vec!["IDL:Components/CCMObject:1.0".into()],
        );
        assert_eq!(m.len(), 1);
        let xmi = m.emit_xmi();
        assert!(xmi.contains("Trader"));
    }

    /// Part 3 §7.6.5, Figure 7.11: `ExtendedPortType` aggregates
    /// `ProvidesDef`s and `UsesDef`s and is parameterized by IDL types;
    /// `ExtendedPortDef` refers to it; `ConnectorDef` has `ProvidesDef`s,
    /// `UsesDef`s, `ExtendedPortDef`s and its base.
    #[test]
    fn corba_p3_7_6_5_metamodel_has_extended_port_types_ports_and_connectors() {
        use crate::component_def::AttributeDef;
        use crate::connector::{ConnectorDef, ConnectorMember, PortTypeDef, PortTypeMember};
        let m = IfrCcmMetamodel::new();
        m.add_port_type(
            &PortTypeDef {
                name: "Feed".into(),
                repository_id: "IDL:Demo/Feed:1.0".into(),
                members: alloc::vec![
                    PortTypeMember::Provides {
                        name: "data".into(),
                        interface_id: "IDL:Demo/Data:1.0".into(),
                    },
                    PortTypeMember::Uses {
                        name: "ctl".into(),
                        interface_id: "IDL:Demo/Ctl:1.0".into(),
                        multiple: true,
                    },
                    PortTypeMember::Attribute(AttributeDef {
                        name: "rate".into(),
                        type_spec: "long".into(),
                        readonly: false,
                        set_raises: Vec::new(),
                        get_raises: Vec::new(),
                    }),
                ],
            },
            alloc::vec!["IDL:Demo/Sample:1.0".into()],
        );
        m.add_extended_port("IDL:Demo/C:1.0", "in_feed", "IDL:Demo/Feed:1.0", false);
        m.add_connector(
            &ConnectorDef {
                name: "Cnx".into(),
                repository_id: "IDL:Demo/Cnx:1.0".into(),
                base: Some("IDL:Demo/BaseCnx:1.0".into()),
                members: alloc::vec![
                    ConnectorMember::MirrorPort {
                        name: "out".into(),
                        port_type_id: "IDL:Demo/Feed:1.0".into(),
                    },
                    ConnectorMember::Provides {
                        name: "p".into(),
                        interface_id: "IDL:Demo/Data:1.0".into(),
                    },
                ],
            },
            Vec::new(),
        );
        assert_eq!(m.len(), 3);
        let xmi = m.emit_xmi();
        for expected in [
            "<CCM.ExtendedPortType name=\"Feed\" repo_id=\"IDL:Demo/Feed:1.0\">",
            "<CCM.ProvidesDef name=\"data\" interface=\"IDL:Demo/Data:1.0\"/>",
            "<CCM.UsesDef name=\"ctl\" interface=\"IDL:Demo/Ctl:1.0\" is_multiple=\"true\"/>",
            "<CCM.Parameterization type=\"IDL:Demo/Sample:1.0\"/>",
            "<CCM.ExtendedPortDef owner=\"IDL:Demo/C:1.0\" name=\"in_feed\" type=\"IDL:Demo/Feed:1.0\" mirror=\"false\"/>",
            "<CCM.ConnectorDef name=\"Cnx\" repo_id=\"IDL:Demo/Cnx:1.0\">",
            "<Mof.Inheritance base=\"IDL:Demo/BaseCnx:1.0\"/>",
            "<CCM.ExtendedPortDef name=\"out\" type=\"IDL:Demo/Feed:1.0\" mirror=\"true\"/>",
            "<CCM.ProvidesDef name=\"p\" interface=\"IDL:Demo/Data:1.0\"/>",
        ] {
            assert!(xmi.contains(expected), "{expected} in\n{xmi}");
        }
        // An attribute is no port of the metamodel.
        assert!(!xmi.contains("rate"));
    }

    // CCM 4.0 §2 CP8 — ORB vendor configuration

    #[test]
    fn orb_vendor_config_interceptor_registry() {
        let o = Orb::init(ThreadingMode::SingleThreaded);
        assert!(o.interceptor_registry().is_none());
        let r = Arc::new(InterceptorRegistry::new());
        o.with_interceptor_registry(r.clone());
        let got = o.interceptor_registry().expect("registry installed");
        // Both Arcs point to the same registry.
        assert!(Arc::ptr_eq(&r, &got));
    }

    #[test]
    fn orb_vendor_config_compression_and_messaging_policies() {
        use crate::compression::{COMPRESSORID_LZMA, COMPRESSORID_ZLIB};
        let o = Orb::init(ThreadingMode::SingleThreaded);
        assert_eq!(o.compression(), NO_COMPRESSION);
        let zlib = CompressorIdLevel {
            compressor_id: COMPRESSORID_ZLIB,
            compression_level: 9,
        };
        o.with_compression(zlib).unwrap();
        assert_eq!(o.compression(), zlib);
        // An id without a factory and a level above 9 are refused and
        // leave the setting as it was.
        assert_eq!(
            o.with_compression(CompressorIdLevel {
                compressor_id: COMPRESSORID_LZMA,
                compression_level: 1,
            }),
            Err(CompressionConfigError::UnknownCompressorId)
        );
        assert_eq!(
            o.with_compression(CompressorIdLevel {
                compressor_id: COMPRESSORID_ZLIB,
                compression_level: 10,
            }),
            Err(CompressionConfigError::System(
                crate::compression::bad_compression_level()
            ))
        );
        assert_eq!(o.compression(), zlib);

        // The messaging-policy list is filled append-only.
        assert!(o.messaging_policies().is_empty());
        o.with_messaging_policy(MessagingPolicy::SyncScope);
        o.with_messaging_policy(MessagingPolicy::Routing);
        // Duplicate entries are deduplicated.
        o.with_messaging_policy(MessagingPolicy::SyncScope);
        let p = o.messaging_policies();
        assert_eq!(p.len(), 2);
        assert!(p.contains(&MessagingPolicy::SyncScope));
        assert!(p.contains(&MessagingPolicy::Routing));
    }

    #[test]
    fn ifr_ccm_metamodel_ingest_repository_walks_definitions() {
        use zerodds_corba_ir::{Definition, DefinitionKind, Repository};
        let mut repo = Repository::new();
        let module = Definition::new("IDL:demo:1.0", "demo", "1.0", DefinitionKind::Module)
            .with_content(
                Definition::new(
                    "IDL:demo/Echo:1.0",
                    "Echo",
                    "1.0",
                    DefinitionKind::Interface,
                )
                .with_content(Definition::new(
                    "IDL:demo/Echo/ping:1.0",
                    "ping",
                    "1.0",
                    DefinitionKind::Operation,
                )),
            );
        repo.register(module).expect("register module");
        let m = IfrCcmMetamodel::from_repository(&repo);
        assert!(
            m.len() >= 3,
            "expected module + interface + op, got {}",
            m.len()
        );
        let xmi = m.emit_xmi();
        assert!(xmi.contains("demo"));
        assert!(xmi.contains("Echo"));
        assert!(xmi.contains("ping"));
        assert!(xmi.contains("IDL:demo/Echo:1.0"));
    }

    // -----------------------------------------------------------------
    // §8.5 ORB_init, initial references; §8.2.2 object URLs
    // -----------------------------------------------------------------

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    fn sample_reference(host: &str, port: u16, key: &[u8]) -> ObjectReference {
        let body = IiopProfileBody::new(IiopVersion::V1_2, host.to_string(), port, key.to_vec());
        ObjectReference {
            type_id: "IDL:demo/Echo:1.0".to_string(),
            iiop_profile: body.encode_encapsulation(Endianness::Big).unwrap(),
            policy_overrides: Vec::new(),
            profiles: Vec::new(),
        }
    }

    fn profile_of(obj: &ObjectRef) -> IiopProfileBody {
        IiopProfileBody::decode_encapsulation(&obj.reference().unwrap().iiop_profile).unwrap()
    }

    fn sys(r: Result<ObjectRef, SystemException>) -> SystemException {
        r.expect_err("a system exception")
    }

    #[test]
    fn corba_p1_8_5_1_orb_init_takes_its_arguments_out_of_the_list() {
        let ior = Orb::init(ThreadingMode::SingleThreaded)
            .object_to_string(&ObjectRef::Reference(sample_reference("h", 1, b"k")))
            .unwrap();
        let mut argv = args(&[
            "prog",
            "-ORBid",
            "orb-8-5-1-a",
            &alloc::format!("-ORBInitRef NameService={ior}"),
            "-ORBServerIdsrv-7",
            "-ORBListenEndpoints",
            "iiop://0.0.0.0:2809",
            "-ORBNoProprietaryActivation",
            "-ORBDefaultInitRef",
            "corbaloc::backup.example:2809",
            "payload",
        ]);
        let orb = orb_init(&mut argv, "").unwrap();
        assert_eq!(
            argv,
            args(&["prog", "payload"]),
            "only foreign strings stay"
        );
        assert_eq!(orb.id(), "orb-8-5-1-a");
        assert_eq!(orb.server_id(), Some("srv-7"));
        assert_eq!(orb.listen_endpoints(), &["iiop://0.0.0.0:2809".to_string()]);
        assert!(orb.no_proprietary_activation());
        assert_eq!(
            orb.list_initial_services().unwrap(),
            vec![
                "CodecFactory".to_string(),
                "CompressionManager".to_string(),
                "DynAnyFactory".to_string(),
                "NameService".to_string(),
                "ORBPolicyManager".to_string(),
                "PICurrent".to_string(),
                "PolicyCurrent".to_string(),
                "TransactionCurrent".to_string()
            ]
        );
        // §18.4: the CompressionManager is a local object of the ORB with
        // the zlib compressor (§18.1).
        let cm = orb
            .resolve_initial_references("CompressionManager")
            .unwrap();
        let cm = cm.local::<CompressionManager>().unwrap();
        assert!(Arc::ptr_eq(&cm, &orb.compression_manager()));
        let zlib = cm
            .get_compressor(crate::compression::COMPRESSORID_ZLIB, 6)
            .unwrap();
        let mut packed = Vec::new();
        zlib.compress(b"payload", &mut packed).unwrap();
        let mut back = Vec::new();
        zlib.decompress(&packed, &mut back).unwrap();
        assert_eq!(back, b"payload");
        // §13.2.1: the DynAnyFactory is a local object of the ORB.
        let factory = orb.resolve_initial_references("DynAnyFactory").unwrap();
        let factory = factory.local::<zerodds_corba_rt::DynAnyFactory>().unwrap();
        let d = factory
            .create_dyn_any_from_type_code(&zerodds_corba_rt::TypeCode::Long)
            .unwrap();
        assert_eq!(
            d.to_any(),
            zerodds_corba_rt::CorbaAny(zerodds_corba_rt::AnyValue::Long(0))
        );
        let ns = orb.resolve_initial_references("NameService").unwrap();
        assert_eq!(profile_of(&ns).object_key, b"k");
        orb.destroy().unwrap();
    }

    #[test]
    fn corba_p1_8_5_1_an_unrecognised_orb_parameter_or_a_missing_value_is_bad_param() {
        for bad in [
            args(&["-ORBNoSuchOption", "x"]),
            args(&["-ORBInitRef"]),
            args(&["-ORBInitRef", "=IOR:00"]),
            args(&["-ORBInitRef", "NameService"]),
            args(&["-ORBNoProprietaryActivationYes"]),
        ] {
            let mut argv = bad.clone();
            let e = orb_init(&mut argv, "orb-8-5-1-bad").unwrap_err();
            assert_eq!(e.kind, SystemExceptionKind::BadParam, "{bad:?}");
        }
    }

    #[test]
    fn corba_p1_8_5_1_the_same_orbid_gives_the_same_orb_until_destroy() {
        let a = orb_init(&mut args(&["-ORBidorb-8-5-1-same"]), "").unwrap();
        let b = orb_init(&mut Vec::new(), "orb-8-5-1-same").unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        // A non-empty identifier ignores -ORBid (and still removes it).
        let mut argv = args(&["-ORBid", "other"]);
        let c = orb_init(&mut argv, "orb-8-5-1-same").unwrap();
        assert!(Arc::ptr_eq(&a, &c));
        assert!(argv.is_empty());
        a.destroy().unwrap();
        let d = orb_init(&mut Vec::new(), "orb-8-5-1-same").unwrap();
        assert!(!Arc::ptr_eq(&a, &d), "a destroyed ORBid creates a new ORB");
        assert_eq!(d.state(), OrbState::Running);
        d.destroy().unwrap();
    }

    #[test]
    fn corba_p1_8_5_3_2_an_init_ref_url_is_checked_and_corbaloc_rir_refused() {
        for bad in [
            "NameService=corbaloc:rir:/NameService",
            "NameService=IOR:zz",
            "NameService=http-not-a-scheme:x",
            "NameService=corbaloc:iiop:host:notaport/key",
        ] {
            let e = OrbArguments::take_from(&mut args(&["-ORBInitRef", bad])).unwrap_err();
            assert_eq!(e.kind, SystemExceptionKind::BadParam, "{bad}");
        }
        let ok = OrbArguments::take_from(&mut args(&[
            "-ORBInitRef",
            "Trader=corbaloc:iiop:1.2@trader.example:2809/Trader",
        ]))
        .unwrap();
        assert_eq!(
            ok.init_refs,
            vec![(
                "Trader".to_string(),
                "corbaloc:iiop:1.2@trader.example:2809/Trader".to_string()
            )]
        );
        let e = OrbArguments::take_from(&mut args(&["-ORBDefaultInitRef", "IOR:00"])).unwrap_err();
        assert_eq!(e.kind, SystemExceptionKind::BadParam);
        // Scheme names without regard to case, also for an MIOP group
        // URL; file, http and ftp URLs are checked as URLs, not fetched.
        for good in [
            "Group=CORBALOC:miop:1.0@1.0-Shop-5/239.255.43.1:6000",
            "NameService=file:///nonexistent/zerodds/ns.ior",
            "NameService=http://ns.example/ns.ior",
            "NameService=ftp://ns.example/refs/ns.ior",
        ] {
            assert!(
                OrbArguments::take_from(&mut args(&["-ORBInitRef", good])).is_ok(),
                "{good}"
            );
        }
        let e = OrbArguments::take_from(&mut args(&[
            "-ORBInitRef",
            "NameService=file://other.example/ns.ior",
        ]))
        .unwrap_err();
        assert_eq!(e.omg_minor(), Some(8));
    }

    #[test]
    fn corba_p1_8_5_3_4_resolution_order_register_init_ref_configured_default() {
        let mut argv = args(&[
            "-ORBInitRef",
            "Svc=corbaloc:iiop:1.2@from-init-ref:1/Svc",
            "-ORBInitRef",
            "Both=corbaloc:iiop:1.2@from-init-ref:1/Both",
            "-ORBDefaultInitRef",
            "corbaloc:iiop:1.2@from-default:2",
        ]);
        let orb = orb_init(&mut argv, "orb-8-5-3-4").unwrap();
        // Step 4: only -ORBDefaultInitRef knows "Other"; the key is the id.
        let other = profile_of(&orb.resolve_initial_references("Other").unwrap());
        assert_eq!((other.host.as_str(), other.port), ("from-default", 2));
        assert_eq!(other.object_key, b"Other");
        // Step 3 beats step 4.
        orb.configure_initial_reference(
            "Other",
            ObjectRef::Reference(sample_reference("configured", 3, b"c")),
        );
        let other = profile_of(&orb.resolve_initial_references("Other").unwrap());
        assert_eq!(other.host, "configured");
        // Step 2 beats step 3.
        orb.configure_initial_reference(
            "Both",
            ObjectRef::Reference(sample_reference("configured", 3, b"c")),
        );
        let both = profile_of(&orb.resolve_initial_references("Both").unwrap());
        assert_eq!(both.host, "from-init-ref");
        // Step 1 beats step 2.
        orb.register_initial_reference(
            "Svc",
            ObjectRef::Reference(sample_reference("registered", 4, b"r")),
        )
        .unwrap();
        let svc = profile_of(&orb.resolve_initial_references("Svc").unwrap());
        assert_eq!(svc.host, "registered");
        orb.destroy().unwrap();
    }

    #[test]
    fn corba_p1_8_5_2_an_unknown_or_nil_initial_reference_is_invalid_name() {
        let orb = Orb::init(ThreadingMode::SingleThreaded);
        assert_eq!(
            orb.resolve_initial_references("NoSuchService").unwrap_err(),
            ResolveError::InvalidName
        );
        // A nil reference is never returned, even when configured.
        orb.configure_initial_reference("Nil", ObjectRef::Reference(ObjectReference::default()));
        assert_eq!(
            orb.resolve_initial_references("Nil").unwrap_err(),
            ResolveError::InvalidName
        );
    }

    #[test]
    fn corba_p1_16_10_register_initial_reference_refuses_empty_duplicate_and_nil() {
        let orb = Orb::init(ThreadingMode::SingleThreaded);
        let obj = ObjectRef::Reference(sample_reference("h", 1, b"k"));
        assert_eq!(
            orb.register_initial_reference("", obj.clone()).unwrap_err(),
            ResolveError::InvalidName
        );
        orb.register_initial_reference("Mine", obj.clone()).unwrap();
        assert_eq!(
            orb.register_initial_reference("Mine", obj.clone())
                .unwrap_err(),
            ResolveError::InvalidName
        );
        orb.configure_initial_reference("Builtin", obj.clone());
        assert_eq!(
            orb.register_initial_reference("Builtin", obj).unwrap_err(),
            ResolveError::InvalidName
        );
        let nil = orb
            .register_initial_reference("NilRef", ObjectRef::Reference(ObjectReference::default()))
            .unwrap_err();
        assert_eq!(
            nil,
            ResolveError::System(SystemException::omg(
                SystemExceptionKind::BadParam,
                27,
                CompletionStatus::No
            ))
        );
        assert_eq!(
            orb.list_initial_services().unwrap(),
            vec!["Builtin".to_string(), "Mine".to_string()]
        );
    }

    #[test]
    fn corba_p1_8_5_2_a_local_initial_reference_narrows_to_its_type() {
        struct Factory(u32);
        let orb = Orb::init(ThreadingMode::SingleThreaded);
        orb.configure_initial_reference("MyFactory", ObjectRef::local_object(Arc::new(Factory(7))));
        let obj = orb.resolve_initial_references("MyFactory").unwrap();
        assert_eq!(obj.local::<Factory>().unwrap().0, 7);
        assert!(obj.local::<String>().is_none());
        assert!(obj.reference().is_none());
        // §8.2.2 — a local object has no string form.
        assert_eq!(
            orb.object_to_string(&obj).unwrap_err(),
            SystemException::omg(SystemExceptionKind::Marshal, 4, CompletionStatus::No)
        );
    }

    #[test]
    fn corba_p1_8_2_2_string_to_object_of_object_to_string_is_the_same_reference() {
        let orb = Orb::init(ThreadingMode::SingleThreaded);
        let obj = ObjectRef::Reference(sample_reference("echo.example", 4242, b"\x00Echo\xff"));
        let s = orb.object_to_string(&obj).unwrap();
        assert!(s.starts_with("IOR:"));
        let back = orb.string_to_object(&s).unwrap();
        assert_eq!(back.reference(), obj.reference());
        // Lower-case scheme, and the nil IOR.
        let lower = alloc::format!("ior:{}", &s[4..]);
        assert_eq!(
            orb.string_to_object(&lower).unwrap().reference(),
            obj.reference()
        );
        let nil = orb
            .object_to_string(&ObjectRef::Reference(ObjectReference::default()))
            .unwrap();
        assert!(orb.string_to_object(&nil).unwrap().is_nil());
        // Part 2 §7.6.2: nil is a null type id and no profile. A null type
        // id with one profile is not nil, also when the profile is an
        // empty IIOP profile or no IIOP profile.
        for profile in [
            RawProfile {
                tag: ProfileId::InternetIop.as_u32(),
                data: Vec::new(),
            },
            RawProfile {
                tag: ProfileId::MultipleComponents.as_u32(),
                data: alloc::vec![0, 0, 0, 0, 0, 0, 0, 0],
            },
        ] {
            let obj = ObjectRef::Reference(ObjectReference::from_profiles(
                String::new(),
                alloc::vec![profile.clone()],
            ));
            assert!(!obj.is_nil(), "{profile:?}");
            let back = orb
                .string_to_object(&orb.object_to_string(&obj).unwrap())
                .unwrap();
            assert!(!back.is_nil(), "{profile:?}");
            assert_eq!(
                back.reference().unwrap().tagged_profiles(),
                alloc::vec![profile]
            );
        }
    }

    #[test]
    fn corba_p1_8_2_2_string_to_object_raises_the_bad_param_minor_codes() {
        let orb = Orb::init(ThreadingMode::SingleThreaded);
        let minor = |url: &str| sys(orb.string_to_object(url)).omg_minor();
        assert_eq!(minor("telnet://host"), Some(7), "bad scheme");
        assert_eq!(minor("no scheme at all"), Some(7));
        assert_eq!(minor("IOR:0"), Some(9), "bad schema-specific part");
        assert_eq!(minor("IOR:zz00"), Some(9));
        assert_eq!(
            minor("corbaloc:iiop:host:99999/key"),
            Some(8),
            "bad address"
        );
        assert_eq!(minor("corbaloc:rir:/NoSuchService"), Some(10));
        // Only an address of a protocol this ORB does not speak.
        assert_eq!(minor("corbaloc:atm:E.164:358.400.1234567/key"), Some(8));
        // Part 2 §7.6.10.2: rir cannot be used with another protocol, and
        // has no address.
        assert_eq!(minor("corbaloc:iiop:1.2@host:1,rir:/PICurrent"), Some(8));
        assert_eq!(minor("corbaname:rir:,:host:1/NameService#a"), Some(8));
        assert_eq!(minor("corbaloc:rir:host/NameService"), Some(8));
        // An IPv6 address outside brackets, a bad key escape, a bad name
        // escape.
        assert_eq!(minor("corbaloc::::1:2809/key"), Some(8));
        assert_eq!(minor("corbaloc::h/key%4"), Some(9));
        assert_eq!(minor("corbaname::h#a%2"), Some(9));
        // Table 7.1: file, http and ftp URLs.
        assert_eq!(minor("file://elsewhere.example/ns.ior"), Some(8));
        assert_eq!(minor("file:///nonexistent/zerodds/ns.ior"), Some(10));
    }

    /// Part 2 §7.6.10.1, §7.6.10.3, §7.6.10.6: every IIOP address of a
    /// `corbaloc` is a profile of its own with its version, in the order
    /// of the URL; an IPv6 host without brackets; an address of another
    /// protocol is skipped; no host is the local host.
    #[test]
    fn corba_p2_7_6_10_corbaloc_addresses_become_profiles_in_their_order() {
        let orb = Orb::init(ThreadingMode::SingleThreaded);
        let obj = orb
            .string_to_object("corbaloc::555xyz.com,:556xyz.com:80/Dev/NameService")
            .unwrap();
        let reference = obj.reference().unwrap();
        let bodies: Vec<IiopProfileBody> = reference
            .iiop_profiles()
            .into_iter()
            .map(|p| IiopProfileBody::decode_encapsulation(p).unwrap())
            .collect();
        assert_eq!(bodies.len(), 2);
        assert_eq!(
            (
                bodies[0].host.as_str(),
                bodies[0].port,
                bodies[0].iiop_version
            ),
            ("555xyz.com", 2809, IiopVersion::V1_0)
        );
        assert_eq!(
            (bodies[1].host.as_str(), bodies[1].port),
            ("556xyz.com", 80)
        );
        assert!(bodies.iter().all(|b| b.object_key == b"Dev/NameService"));
        // object_to_string writes both profiles.
        let back = orb
            .string_to_object(&orb.object_to_string(&obj).unwrap())
            .unwrap();
        assert_eq!(back.reference(), Some(reference));

        let p = profile_of(
            &orb.string_to_object("corbaloc::[1080::8:800:200C:417A]:88/DefaultEventChannel")
                .unwrap(),
        );
        assert_eq!((p.host.as_str(), p.port), ("1080::8:800:200C:417A", 88));
        let p = profile_of(&orb.string_to_object("corbaloc::[::1]/k").unwrap());
        assert_eq!((p.host.as_str(), p.port), ("::1", 2809));

        let obj = orb
            .string_to_object(
                "corbaloc:iiop:1.2@xyz.com,atm:E.164:358.400.1234567/dev/test/objectX",
            )
            .unwrap();
        assert_eq!(obj.reference().unwrap().iiop_profiles().len(), 1);
        assert_eq!(profile_of(&obj).host, "xyz.com");

        let p = profile_of(&orb.string_to_object("corbaloc::/k").unwrap());
        assert_eq!((p.host.as_str(), p.port), ("localhost", 2809));
    }

    /// Part 2 §7.6.10.2: an empty key string of a `rir` URL names
    /// `NameService`; §7.6.10.5: so does a `corbaname` without a key.
    #[test]
    fn corba_p2_7_6_10_2_an_empty_rir_key_resolves_the_nameservice() {
        let orb = Orb::init(ThreadingMode::SingleThreaded);
        orb.register_initial_reference(
            "NameService",
            ObjectRef::Reference(sample_reference("ns.example", 2809, b"NameService")),
        )
        .unwrap();
        for url in [
            "corbaloc:rir:",
            "corbaloc:rir:/",
            "corbaname:rir:",
            "corbaname:rir:#",
        ] {
            let ns = orb.string_to_object(url).unwrap();
            assert_eq!(profile_of(&ns).host, "ns.example", "{url}");
        }
        let ctx = orb.string_to_object("corbaname::555objs.com").unwrap();
        assert_eq!(profile_of(&ctx).object_key, b"NameService");
    }

    /// Part 2 §7.6.10.7, Table 7.1: a `file` URL names a file holding a
    /// URL or an IOR, which is converted in turn; a file naming itself
    /// ends with BAD_PARAM 10.
    #[test]
    fn corba_p2_7_6_10_7_a_file_url_yields_the_reference_in_the_file() {
        let orb = Orb::init(ThreadingMode::SingleThreaded);
        let dir = std::env::temp_dir().join(alloc::format!(
            "zerodds-orb-file-url-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let reference = ObjectRef::Reference(sample_reference("echo.example", 4242, b"Echo"));
        let ior_file = dir.join("echo.ior");
        std::fs::write(
            &ior_file,
            alloc::format!("{}\n", orb.object_to_string(&reference).unwrap()),
        )
        .unwrap();
        let url = alloc::format!("file://{}", ior_file.display());
        assert_eq!(
            orb.string_to_object(&url).unwrap().reference(),
            reference.reference()
        );
        // A file with a corbaloc URL, named by a file URL in another file.
        let loc_file = dir.join("loc.txt");
        std::fs::write(&loc_file, "corbaloc::echo.example:4242/Echo").unwrap();
        let outer = dir.join("outer.txt");
        std::fs::write(
            &outer,
            alloc::format!("file://localhost{}", loc_file.display()),
        )
        .unwrap();
        let p = profile_of(
            &orb.string_to_object(&alloc::format!("FILE://{}", outer.display()))
                .unwrap(),
        );
        assert_eq!(
            (p.host.as_str(), p.port, p.object_key.as_slice()),
            ("echo.example", 4242, &b"Echo"[..])
        );
        // Text that is no URL or has an unknown scheme is the bad
        // scheme-specific part of a good `file` URL (minor 9); an empty
        // file too; a file naming itself.
        for (name, text) in [
            ("junk.txt", "not a reference"),
            ("gopher.txt", "gopher://h/x"),
            ("empty.txt", " \n"),
        ] {
            let file = dir.join(name);
            std::fs::write(&file, text).unwrap();
            assert_eq!(
                sys(orb.string_to_object(&alloc::format!("file://{}", file.display()))).omg_minor(),
                Some(9),
                "{text:?}"
            );
        }
        let me = dir.join("me.txt");
        std::fs::write(&me, alloc::format!("file://{}", me.display())).unwrap();
        assert_eq!(
            sys(orb.string_to_object(&alloc::format!("file://{}", me.display()))).omg_minor(),
            Some(10)
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn corba_p1_8_2_2_corbaloc_iiop_builds_an_iiop_profile() {
        let orb = Orb::init(ThreadingMode::SingleThreaded);
        let obj = orb
            .string_to_object("corbaloc:iiop:1.2@echo.example:4711/Echo%00Key")
            .unwrap();
        let p = profile_of(&obj);
        assert_eq!(p.iiop_version, IiopVersion::V1_2);
        assert_eq!((p.host.as_str(), p.port), ("echo.example", 4711));
        assert_eq!(p.object_key, b"Echo\x00Key");
        assert!(p.components.is_empty());
        // Default port 2809 and IIOP 1.0 when not given.
        let p = profile_of(&orb.string_to_object("corbaloc::echo.example/K").unwrap());
        assert_eq!((p.iiop_version, p.port), (IiopVersion::V1_0, 2809));
        // Several addresses: one profile each, whatever their version.
        let obj = orb
            .string_to_object("corbaloc:iiop:1.2@a.example:1,iiop:1.0@b.example:2/K")
            .unwrap();
        let bodies: Vec<IiopProfileBody> = obj
            .reference()
            .unwrap()
            .iiop_profiles()
            .into_iter()
            .map(|p| IiopProfileBody::decode_encapsulation(p).unwrap())
            .collect();
        assert_eq!(bodies.len(), 2);
        assert_eq!(
            (bodies[0].host.as_str(), bodies[0].iiop_version),
            ("a.example", IiopVersion::V1_2)
        );
        assert_eq!(
            (bodies[1].host.as_str(), bodies[1].iiop_version),
            ("b.example", IiopVersion::V1_0)
        );
    }

    #[test]
    fn corba_p1_8_2_2_corbaloc_rir_resolves_an_initial_reference() {
        let orb = Orb::init(ThreadingMode::SingleThreaded);
        orb.register_initial_reference(
            "NameService",
            ObjectRef::Reference(sample_reference("ns.example", 2809, b"NameService")),
        )
        .unwrap();
        let ns = orb.string_to_object("corbaloc:rir:/NameService").unwrap();
        assert_eq!(profile_of(&ns).host, "ns.example");
    }

    /// A naming context that answers `resolve` for one name.
    struct FakeNamingContext {
        expected: zerodds_corba_cosnaming::Name,
        answer: Ior,
        target_key: Vec<u8>,
    }

    impl CorbaConnection for FakeNamingContext {
        fn invoke(
            &self,
            target: &ObjectReference,
            operation: &str,
            e: Endianness,
            arguments: &zerodds_corba_rust::MarshalArgs<'_>,
        ) -> Result<(Vec<u8>, Endianness), CorbaException> {
            let target = IiopProfileBody::decode_encapsulation(&target.iiop_profile).unwrap();
            assert_eq!(target.object_key, self.target_key);
            assert_eq!(operation, "resolve");
            let mut w = zerodds_wire::Writer::new(e);
            arguments(&mut w)?;
            let bytes = w.finish();
            let name = zerodds_corba_cosnaming::wire::decode_name_request(
                &mut zerodds_wire::Reader::new(&bytes, e),
            )
            .unwrap();
            if name != self.expected {
                return Err(CorbaException::UserException {
                    repository_id: "IDL:omg.org/CosNaming/NamingContext/NotFound:1.0".into(),
                    body: zerodds_corba_rust::AlignedBody::new(&[], 0),
                    endianness: e,
                });
            }
            Ok((
                zerodds_corba_cosnaming::wire::encode_object_reply(&self.answer, e).unwrap(),
                e,
            ))
        }
        fn invoke_oneway(
            &self,
            _: &ObjectReference,
            _: &str,
            _: Endianness,
            _: &zerodds_corba_rust::MarshalArgs<'_>,
        ) -> Result<(), CorbaException> {
            Err(CorbaException::system(
                SystemExceptionKind::BadOperation,
                0,
                CompletionStatus::No,
            ))
        }
    }

    #[test]
    fn corba_p1_8_2_2_corbaname_resolves_the_name_on_the_naming_context() {
        let orb = Orb::init(ThreadingMode::SingleThreaded);
        let url = "corbaname::ns.example:2809/NameService#dev/echo.obj";
        // Without a transport the ORB cannot call the naming context.
        assert_eq!(
            sys(orb.string_to_object(url)).kind,
            SystemExceptionKind::Initialize
        );
        let target = sample_reference("echo.example", 4242, b"Echo");
        let answer = Ior::new(
            target.type_id.clone(),
            vec![TaggedProfile {
                tag: ProfileId::InternetIop,
                profile_data: target.iiop_profile.clone(),
            }],
        );
        orb.set_connection(Arc::new(FakeNamingContext {
            expected: vec![
                zerodds_corba_cosnaming::NameComponent::new("dev"),
                zerodds_corba_cosnaming::NameComponent::with_kind("echo", "obj"),
            ],
            answer,
            target_key: b"NameService".to_vec(),
        }));
        let obj = orb.string_to_object(url).unwrap();
        assert_eq!(obj.reference(), Some(&target));
        // A name the context does not know: BAD_PARAM minor 10.
        let e = sys(orb.string_to_object("corbaname::ns.example:2809/NameService#nope"));
        assert_eq!(e.omg_minor(), Some(10));
        // No name: the naming context itself.
        let ctx = orb
            .string_to_object("corbaname::ns.example:2809/NameService")
            .unwrap();
        assert_eq!(profile_of(&ctx).object_key, b"NameService");
    }

    #[test]
    fn corba_p1_8_2_5_after_shutdown_the_orb_operations_raise_bad_inv_order_4() {
        let orb = Orb::init(ThreadingMode::SingleThreaded);
        orb.shutdown(false).unwrap();
        let expected =
            SystemException::omg(SystemExceptionKind::BadInvOrder, 4, CompletionStatus::No);
        assert_eq!(orb.list_initial_services().unwrap_err(), expected);
        assert_eq!(
            orb.resolve_initial_references("NameService").unwrap_err(),
            ResolveError::System(expected)
        );
        assert_eq!(sys(orb.string_to_object("IOR:00")), expected);
    }

    #[test]
    fn corba_p1_8_5_3_3_a_rir_default_init_ref_is_refused_and_a_rir_cycle_ends() {
        // `corbaloc:rir:` as -ORBDefaultInitRef names the id being
        // resolved itself.
        for bad in ["corbaloc:rir:", "CORBALOC:rir:"] {
            let e = OrbArguments::take_from(&mut args(&["-ORBDefaultInitRef", bad])).unwrap_err();
            assert_eq!(e.omg_minor(), Some(8), "{bad}");
        }
        // -ORBInitRef X=corbaname:rir:/X#n leads back to X: the
        // resolution ends with BAD_PARAM 10 instead of recursing.
        let mut argv = args(&["-ORBInitRef", "Loop=corbaname:rir:/Loop#n"]);
        let orb = orb_init(&mut argv, "orb-8-5-3-3-loop").unwrap();
        assert_eq!(
            orb.resolve_initial_references("Loop").unwrap_err(),
            ResolveError::System(bad_param(10))
        );
        assert_eq!(
            sys(orb.string_to_object("corbaloc:rir:/Loop")),
            bad_param(10)
        );
        orb.destroy().unwrap();
        // corbaname:rir as -ORBDefaultInitRef without a NameService: the
        // NameService lookup comes back to itself and ends.
        let mut argv = args(&["-ORBDefaultInitRef", "corbaname:rir:/NameService#Prod"]);
        let orb = orb_init(&mut argv, "orb-8-5-3-3-default").unwrap();
        assert_eq!(
            orb.resolve_initial_references("Svc").unwrap_err(),
            ResolveError::System(bad_param(10))
        );
        // With a NameService configured, the same default resolves the
        // name Prod/Svc on it.
        orb.register_initial_reference(
            "NameService",
            ObjectRef::Reference(sample_reference("ns.example", 2809, b"NameService")),
        )
        .unwrap();
        let target = sample_reference("svc.example", 4242, b"Svc");
        orb.set_connection(Arc::new(FakeNamingContext {
            expected: vec![
                zerodds_corba_cosnaming::NameComponent::new("Prod"),
                zerodds_corba_cosnaming::NameComponent::new("Svc"),
            ],
            answer: Ior::new(
                target.type_id.clone(),
                vec![TaggedProfile {
                    tag: ProfileId::InternetIop,
                    profile_data: target.iiop_profile.clone(),
                }],
            ),
            target_key: b"NameService".to_vec(),
        }));
        let svc = orb.resolve_initial_references("Svc").unwrap();
        assert_eq!(svc.reference(), Some(&target));
        orb.destroy().unwrap();
    }

    /// An initializer that acts only on the ORBs named `orb-16-9-*`
    /// (the initializer list is process-wide) and records what the
    /// `ORBInitInfo` operations raise.
    struct Probe {
        seen: Mutex<Vec<String>>,
    }

    struct Named(&'static str);

    impl crate::portable_interceptor::ClientRequestInterceptor for Named {
        fn name(&self) -> &str {
            self.0
        }
    }

    impl crate::portable_interceptor::OrbInitializer for Probe {
        fn pre_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), InitializerError> {
            let mut seen = self.seen.lock().unwrap();
            match info.orb_id() {
                "orb-16-9-rules" => {
                    let slot = info.allocate_slot_id()?;
                    info.add_client_request_interceptor(Arc::new(Named("pre")))?;
                    // §16.9.2.6: the ORB's own services count as registered.
                    let any = ObjectRef::Reference(sample_reference("h", 1, b"k"));
                    seen.push(format!(
                        "PICurrent:{:?}",
                        info.register_initial_reference("PICurrent", any.clone())
                    ));
                    seen.push(format!(
                        "DynAnyFactory:{:?}",
                        info.register_initial_reference("DynAnyFactory", any.clone())
                    ));
                    info.register_initial_reference("FromInit", any)?;
                    seen.push(format!("slot:{slot}"));
                }
                "orb-16-9-fails" => {
                    info.add_client_request_interceptor(Arc::new(Named("doomed")))?;
                    return Err(InitializerError::System(SystemException::new(
                        SystemExceptionKind::NoResources,
                        0,
                        CompletionStatus::No,
                    )));
                }
                "orb-16-9-user" => {
                    info.add_client_request_interceptor(Arc::new(Named("twice")))?;
                    info.add_client_request_interceptor(Arc::new(Named("twice")))?;
                }
                _ => {}
            }
            Ok(())
        }

        fn post_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), InitializerError> {
            if info.orb_id() != "orb-16-9-rules" {
                return Ok(());
            }
            let mut seen = self.seen.lock().unwrap();
            // §16.9.1.2: no interceptors and no slots in post_init.
            seen.push(format!(
                "add:{:?}",
                info.add_client_request_interceptor(Arc::new(Named("post")))
            ));
            seen.push(format!("slot:{:?}", info.allocate_slot_id()));
            // §16.9.2.14: PICurrent slots cannot be read or written.
            let current = info
                .resolve_initial_references("PICurrent")?
                .local::<PiCurrent>()
                .unwrap();
            seen.push(format!("get:{:?}", current.get_slot(0)));
            seen.push(format!(
                "set:{:?}",
                current.set_slot(0, CorbaAny(zerodds_corba_rt::AnyValue::Long(1)))
            ));
            Ok(())
        }
    }

    #[test]
    fn corba_p1_16_9_orb_init_info_rules_and_an_abandoned_initialization() {
        use crate::portable_interceptor::{CurrentError, RegistrationError};
        let probe = Arc::new(Probe {
            seen: Mutex::new(Vec::new()),
        });
        crate::portable_interceptor::register_orb_initializer(probe.clone());
        let orb = orb_init(&mut Vec::new(), "orb-16-9-rules").unwrap();
        let bad_inv_order =
            |m| SystemException::omg(SystemExceptionKind::BadInvOrder, m, CompletionStatus::No);
        assert_eq!(
            *probe.seen.lock().unwrap(),
            [
                "PICurrent:Err(InvalidName)".to_string(),
                "DynAnyFactory:Err(InvalidName)".to_string(),
                "slot:0".to_string(),
                format!(
                    "add:{:?}",
                    Err::<(), _>(RegistrationError::System(bad_inv_order(26)))
                ),
                format!("slot:{:?}", Err::<u32, _>(bad_inv_order(26))),
                format!(
                    "get:{:?}",
                    Err::<CorbaAny, _>(CurrentError::System(bad_inv_order(14)))
                ),
                format!(
                    "set:{:?}",
                    Err::<(), _>(CurrentError::System(bad_inv_order(14)))
                ),
            ]
        );
        // After ORB_init the slots work, the pre_init interceptor is on
        // the ORB and the initial reference registered there resolves.
        let current = orb.pi_current();
        current
            .set_slot(0, CorbaAny(zerodds_corba_rt::AnyValue::Long(7)))
            .unwrap();
        assert_eq!(
            current.get_slot(0).unwrap(),
            CorbaAny(zerodds_corba_rt::AnyValue::Long(7))
        );
        assert_eq!(orb.interceptor_registry().unwrap().client_count(), 1);
        assert!(orb.resolve_initial_references("FromInit").is_ok());
        orb.destroy().unwrap();

        // §16.9.1: an exception from an initializer abandons the ORB.
        let e = orb_init(&mut Vec::new(), "orb-16-9-fails").unwrap_err();
        assert_eq!(e.kind, SystemExceptionKind::NoResources);
        assert!(
            !ORB_TABLE
                .lock()
                .unwrap()
                .orbs
                .contains_key("orb-16-9-fails")
        );
        // A user exception from an initializer: INITIALIZE.
        let e = orb_init(&mut Vec::new(), "orb-16-9-user").unwrap_err();
        assert_eq!(e.kind, SystemExceptionKind::Initialize);
        assert!(!ORB_TABLE.lock().unwrap().orbs.contains_key("orb-16-9-user"));
    }

    /// An initializer for the ORBs `orb-p1-8-5-1-*`: one that calls
    /// `orb_init` itself, and one that waits until the test lets it go.
    struct Gate {
        seen: Mutex<Vec<String>>,
        started: Mutex<std::sync::mpsc::Sender<()>>,
        release: Mutex<std::sync::mpsc::Receiver<()>>,
        slow_runs: Mutex<u32>,
        released: Mutex<Option<bool>>,
    }

    impl crate::portable_interceptor::OrbInitializer for Gate {
        fn pre_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), InitializerError> {
            match info.orb_id() {
                "orb-p1-8-5-1-reentrant" => {
                    let own = orb_init(&mut Vec::new(), "orb-p1-8-5-1-reentrant");
                    let inner = orb_init(&mut Vec::new(), "orb-p1-8-5-1-inner");
                    let mut seen = self.seen.lock().unwrap();
                    seen.push(format!("own:{:?}", own.map(|_| ()).map_err(|e| e.kind)));
                    seen.push(format!("inner:{}", inner.is_ok()));
                    if let Ok(o) = inner {
                        seen.push(format!("destroy:{}", o.destroy().is_ok()));
                    }
                }
                "orb-p1-8-5-1-slow" => {
                    *self.slow_runs.lock().unwrap() += 1;
                    let _ = self.started.lock().unwrap().send(());
                    let released = self
                        .release
                        .lock()
                        .unwrap()
                        .recv_timeout(std::time::Duration::from_secs(10))
                        .is_ok();
                    *self.released.lock().unwrap() = Some(released);
                }
                _ => {}
            }
            Ok(())
        }
    }

    /// §8.5.1 with §16.9: the initializers of a new ORB run while the
    /// table of ORBs is free. An initializer that calls `orb_init` gets
    /// another ORB and `BAD_INV_ORDER` for its own, which is not made
    /// yet, instead of a deadlock; a slow initializer does not hold up
    /// `orb_init` and `destroy` of another ORB in another thread; a
    /// second call for the ORBid being made waits and gets the same ORB,
    /// whose initializers ran once.
    #[test]
    fn corba_p1_8_5_1_orb_init_runs_the_initializers_without_holding_other_callers() {
        use std::sync::mpsc;
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let gate = Arc::new(Gate {
            seen: Mutex::new(Vec::new()),
            started: Mutex::new(started_tx),
            release: Mutex::new(release_rx),
            slow_runs: Mutex::new(0),
            released: Mutex::new(None),
        });
        crate::portable_interceptor::register_orb_initializer(gate.clone());

        let orb = orb_init(&mut Vec::new(), "orb-p1-8-5-1-reentrant").unwrap();
        assert_eq!(
            *gate.seen.lock().unwrap(),
            ["own:Err(BadInvOrder)", "inner:true", "destroy:true"]
        );
        orb.destroy().unwrap();

        let slow = std::thread::spawn(|| orb_init(&mut Vec::new(), "orb-p1-8-5-1-slow"));
        started_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        let same = std::thread::spawn(|| orb_init(&mut Vec::new(), "orb-p1-8-5-1-slow"));
        // While the slow initializer waits: another ORB is made and
        // destroyed.
        let other = orb_init(&mut Vec::new(), "orb-p1-8-5-1-other").unwrap();
        other.destroy().unwrap();
        release_tx.send(()).unwrap();
        let a = slow.join().unwrap().unwrap();
        let b = same.join().unwrap().unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(*gate.slow_runs.lock().unwrap(), 1);
        assert_eq!(
            *gate.released.lock().unwrap(),
            Some(true),
            "the other ORB was made while the slow initializer ran"
        );
        a.destroy().unwrap();
        assert!(
            !ORB_TABLE
                .lock()
                .unwrap()
                .initializing
                .contains_key("orb-p1-8-5-1-slow")
        );
    }

    #[test]
    fn corba_p2_7_8_codec_factory_is_an_initial_reference() {
        use crate::iop_codec::{CodecFactory, Encoding};
        use zerodds_corba_rt::AnyValue;
        let orb = orb_init(&mut Vec::new(), "orb-p2-7-8-codec").unwrap();
        assert!(
            orb.list_initial_services()
                .unwrap()
                .contains(&"CodecFactory".to_string())
        );
        let factory = orb
            .resolve_initial_references("CodecFactory")
            .unwrap()
            .local::<CodecFactory>()
            .unwrap();
        let codec = factory.create_codec(Encoding::cdr_encaps(1, 2)).unwrap();
        let any = CorbaAny(AnyValue::ULong(9));
        assert_eq!(codec.decode(&codec.encode(&any).unwrap()).unwrap(), any);
        // The ORB's own service counts as registered.
        let other = ObjectRef::Reference(sample_reference("h", 1, b"k"));
        assert!(matches!(
            orb.register_initial_reference("CodecFactory", other),
            Err(ResolveError::InvalidName)
        ));
        orb.destroy().unwrap();
    }

    /// Encodes in `pre_init` with the codec of `ORBInitInfo::codec_factory`.
    struct CodecProbe {
        encoded: Mutex<Vec<Vec<u8>>>,
    }

    impl crate::portable_interceptor::OrbInitializer for CodecProbe {
        fn pre_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), InitializerError> {
            if info.orb_id() == "orb-p1-16-9-2-5" {
                let codec = info
                    .codec_factory()
                    .create_codec(crate::iop_codec::Encoding::cdr_encaps(1, 2))
                    .unwrap();
                self.encoded.lock().unwrap().push(
                    codec
                        .encode_value(&CorbaAny(zerodds_corba_rt::AnyValue::Short(-2)))
                        .unwrap(),
                );
            }
            Ok(())
        }
    }

    #[test]
    fn corba_p1_16_9_2_5_orb_init_info_offers_the_codec_factory_in_pre_init() {
        let probe = Arc::new(CodecProbe {
            encoded: Mutex::new(Vec::new()),
        });
        crate::portable_interceptor::register_orb_initializer(probe.clone());
        let orb = orb_init(&mut Vec::new(), "orb-p1-16-9-2-5").unwrap();
        assert_eq!(*probe.encoded.lock().unwrap(), [vec![0, 0, 0xff, 0xfe]]);
        orb.destroy().unwrap();
    }

    #[test]
    fn corba_p2_12_3_create_policy_makes_the_four_ziop_policies() {
        use zerodds_corba_giop::ziop as z;
        use zerodds_corba_rt::AnyValue;
        let orb = orb_init(&mut Vec::new(), "orb-p2-12-3-ziop").unwrap();
        let pair = |id: u16, level: u16| AnyValue::Struct {
            repo_id: "IDL:omg.org/Compression/CompressorIdLevel:1.0".into(),
            name: "CompressorIdLevel".into(),
            members: alloc::vec![
                ("compressor_id".into(), AnyValue::UShort(id)),
                ("compression_level".into(), AnyValue::UShort(level)),
            ],
        };
        let list = |items: Vec<AnyValue>| {
            CorbaAny(AnyValue::Seq {
                element: zerodds_corba_rt::TypeCode::Null,
                items,
            })
        };
        let policies = alloc::vec![
            orb.create_policy(64, &CorbaAny(AnyValue::Boolean(true)))
                .unwrap(),
            orb.create_policy(65, &list(alloc::vec![pair(1, 9), pair(4, 3)]))
                .unwrap(),
            orb.create_policy(66, &CorbaAny(AnyValue::ULong(512)))
                .unwrap(),
            orb.create_policy(67, &CorbaAny(AnyValue::Float(0.75)))
                .unwrap(),
        ];
        let p = ziop_policies(&policies).unwrap();
        assert!(p.compression_enabled);
        assert_eq!(
            p.compressor_ids,
            alloc::vec![
                CompressorIdLevel {
                    compressor_id: 1,
                    compression_level: 9
                },
                CompressorIdLevel {
                    compressor_id: 4,
                    compression_level: 3
                },
            ]
        );
        assert_eq!(p.low_value, 512);
        assert_eq!(p.min_ratio, Some(0.75));
        // A policy that is not in the list keeps the ORB default.
        let only = ziop_policies(&policies[..1]).unwrap();
        assert_eq!(only.low_value, z::DEFAULT_LOW_VALUE);
        assert_eq!(only.compressor_ids, alloc::vec![z::DEFAULT_COMPRESSOR]);
        assert_eq!(only.min_ratio, None);
        // Values of the wrong type or out of range.
        let bad_value = PolicyError {
            reason: PolicyError::BAD_POLICY_VALUE,
        };
        for (t, v) in [
            (64, CorbaAny(AnyValue::Long(1))),
            (65, list(alloc::vec![pair(4, 10)])),
            (65, list(alloc::vec![AnyValue::UShort(4)])),
            (66, CorbaAny(AnyValue::Long(5))),
            (67, CorbaAny(AnyValue::Float(f32::NAN))),
            (67, CorbaAny(AnyValue::Float(-0.5))),
        ] {
            assert_eq!(orb.create_policy(t, &v).unwrap_err(), bad_value, "{t}");
        }
        // The same ZIOP policy twice in one list.
        let twice = alloc::vec![policies[0].clone(), policies[0].clone()];
        assert_eq!(
            ziop_policies(&twice).unwrap_err().reason,
            PolicyError::BAD_POLICY
        );
        // Other types still go to the registered factories.
        assert_eq!(
            orb.create_policy(68, &CorbaAny(AnyValue::Boolean(true)))
                .unwrap_err()
                .reason,
            PolicyError::BAD_POLICY_TYPE
        );
    }

    #[test]
    fn corba_p3_6_13_2_2_transaction_current_is_a_local_initial_reference() {
        use zerodds_corba_cos_transactions::{Status, TransactionCurrent};
        let orb = orb_init(&mut Vec::new(), "orb-p3-6-13-2-2").unwrap();
        let obj = orb
            .resolve_initial_references("TransactionCurrent")
            .unwrap();
        assert!(obj.reference().is_none(), "a local object has no IOR");
        // A local object is not marshalled: object_to_string is MARSHAL 4.
        let e = orb.object_to_string(&obj).unwrap_err();
        assert_eq!(e.kind, SystemExceptionKind::Marshal);
        assert_eq!(
            e.minor,
            SystemException::omg(SystemExceptionKind::Marshal, 4, CompletionStatus::No).minor
        );
        let current = obj.local::<TransactionCurrent>().expect("narrow");
        current.with(|c| c.begin()).unwrap();
        assert_eq!(current.with(|c| c.get_status()), Status::Active);
        // The same object for every caller of this ORB.
        let again = orb
            .resolve_initial_references("TransactionCurrent")
            .unwrap()
            .local::<TransactionCurrent>()
            .unwrap();
        assert_eq!(again.with(|c| c.get_status()), Status::Active);
        current.with(|c| c.rollback()).unwrap();
        assert!(
            orb.list_initial_services()
                .unwrap()
                .iter()
                .any(|s| s == "TransactionCurrent")
        );
    }

    #[test]
    fn corba_p1_8_9_1_policy_manager_and_policy_current_are_initial_references() {
        use crate::policy_management::{PolicyCurrent, PolicyManager, SetOverrideType};
        use zerodds_corba_giop::ziop as z;
        use zerodds_corba_rt::AnyValue;
        let orb = orb_init(&mut Vec::new(), "orb-p1-8-9-1").unwrap();
        let manager = orb
            .resolve_initial_references("ORBPolicyManager")
            .unwrap()
            .local::<PolicyManager>()
            .expect("narrow");
        let current = orb
            .resolve_initial_references("PolicyCurrent")
            .unwrap()
            .local::<PolicyCurrent>()
            .expect("narrow");
        let services = orb.list_initial_services().unwrap();
        assert!(services.iter().any(|s| s == "ORBPolicyManager"));
        assert!(services.iter().any(|s| s == "PolicyCurrent"));
        let low = |n: u32| {
            orb.create_policy(
                z::COMPRESSION_LOW_VALUE_POLICY_ID,
                &CorbaAny(AnyValue::ULong(n)),
            )
            .unwrap()
        };
        // The objects the ORB hands out are the ones it reads.
        manager
            .set_policy_overrides(&[low(100)], SetOverrideType::SetOverride)
            .unwrap();
        let plain = sample_reference("h", 1, b"k");
        let effective = |r: &ObjectReference| {
            orb.client_ziop_policies(r, None)
                .unwrap()
                .map(|p| p.low_value)
        };
        assert_eq!(effective(&plain), Some(100));
        current
            .set_policy_overrides(&[low(200)], SetOverrideType::SetOverride)
            .unwrap();
        assert_eq!(effective(&plain), Some(200));
        let obj = ObjectRef::Reference(plain.clone())
            .set_policy_overrides(&[low(300)], SetOverrideType::SetOverride)
            .unwrap();
        let with = obj.reference().unwrap();
        assert_eq!(effective(with), Some(300));
        assert_eq!(
            orb.get_client_policy(with, z::COMPRESSION_LOW_VALUE_POLICY_ID)
                .unwrap(),
            Some(low(300))
        );
        assert_eq!(obj.get_policy_overrides(&[]).unwrap(), vec![low(300)]);
        // Another thread sees the ORB scope only.
        let o = Arc::clone(&orb);
        let r = plain.clone();
        let other = std::thread::spawn(move || {
            o.client_ziop_policies(&r, None)
                .unwrap()
                .map(|p| p.low_value)
        })
        .join()
        .unwrap();
        assert_eq!(other, Some(100));
        // A server of this ORB takes the ORB-scope ZIOP overrides.
        assert_eq!(
            orb.server_ziop_policies().unwrap().map(|p| p.low_value),
            Some(100)
        );
        current
            .set_policy_overrides(&[], SetOverrideType::SetOverride)
            .unwrap();
        manager
            .set_policy_overrides(&[], SetOverrideType::SetOverride)
            .unwrap();
        assert_eq!(effective(&plain), None);
        assert_eq!(orb.server_ziop_policies().unwrap(), None);
        // A local object has no policy overrides (§8.3.14); a group
        // reference has them like any reference (Part 2 §11.2.6.10).
        let local = orb.resolve_initial_references("PolicyCurrent").unwrap();
        let no_impl =
            SystemException::omg(SystemExceptionKind::NoImplement, 8, CompletionStatus::No);
        assert_eq!(
            local
                .set_policy_overrides(&[low(1)], SetOverrideType::SetOverride)
                .unwrap_err(),
            crate::policy_management::OverrideError::System(no_impl)
        );
        assert_eq!(local.get_policy_overrides(&[]).unwrap_err(), no_impl);
        let group = orb
            .string_to_object("corbaloc:miop:1.0@1.0-Shop-5/239.255.43.1:6000")
            .unwrap();
        let with = group
            .set_policy_overrides(&[low(5)], SetOverrideType::SetOverride)
            .unwrap();
        assert_eq!(with.group(), group.group());
        assert_eq!(with.get_policy_overrides(&[]).unwrap(), vec![low(5)]);
        assert!(group.get_policy_overrides(&[]).unwrap().is_empty());
    }

    #[test]
    fn corba_p2_11_2_7_string_to_object_makes_a_group_reference_of_corbaloc_miop() {
        let orb = orb_init(&mut Vec::new(), "orb-p2-11-2-7").unwrap();
        let obj = orb
            .string_to_object("corbaloc:miop:1.0@1.0-Shop-5/239.255.43.1:6000")
            .unwrap();
        let ior = obj.group().expect("a group reference").clone();
        assert!(obj.reference().is_none());
        assert!(!obj.is_nil());
        assert!(zerodds_corba_ior::miop::is_group_reference(&ior));
        // object_to_string keeps the UIPMC profile, and the IOR string is
        // a group reference again.
        let s = orb.object_to_string(&obj).unwrap();
        assert!(s.starts_with("IOR:"));
        let back = orb.string_to_object(&s).unwrap();
        assert_eq!(back.group(), Some(&ior));
        assert!(
            orb.string_to_object("corbaloc:miop:1.0-Shop-5/10.0.0.1:6000")
                .is_err()
        );
    }
}
