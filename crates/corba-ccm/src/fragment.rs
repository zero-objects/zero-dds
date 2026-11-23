// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Connector fragments at run time — CORBA 3.3 Part 3 §7.5.
//!
//! A connector is realised by fragments, each collocated with a
//! component. "A `Components::CCMObject` interface has to be implemented
//! for each identified fragment" and Navigation, Receptacles and
//! `KeylessCCMHome` are mandatory. [`ConnectorFragment`] is that
//! `CCMObject`: `provide_facet`, `get_all_facets`, `get_named_facets`,
//! `same_component`, `connect`, `disconnect`, `get_connections`,
//! `get_all_receptacles`, `get_named_receptacles` over the basic ports of
//! the connector, `get_ccm_home`, `configuration_complete` and `remove`.
//! [`ConnectorHome`] is `KeylessCCMHome::create_component`,
//! `CCMHome::remove_component` and
//! `HomeConfiguration::set_configuration_values` (§7.5.3, §7.5.4). The
//! connector provider writes the behaviour behind them: a
//! [`ConnectorHomeExecutor`] (the `Components::HomeExecutorBase` the
//! "home factory" entry point returns, §7.5.3, §7.6.3.1) creates one
//! [`FragmentExecutor`] per fragment. [`HomeFactories`] holds the entry
//! points by name.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use core::any::Any;
use std::sync::Mutex;

use zerodds_corba_rt::CorbaAny;

use crate::component_def::{ComponentDef, FacetDef, ReceptacleDef, ReceptacleMultiplicity};
use crate::orb_core::ObjectRef;

// ---------------------------------------------------------------------------
// Runtime: Components exceptions, values and descriptions
// ---------------------------------------------------------------------------

/// `Components::ConfigValue` (Part 3 §6.10.1.2): a feature name and an
/// `any`.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigValue {
    /// The attribute the value is for.
    pub name: String,
    /// The value.
    pub value: CorbaAny,
}

/// `Components::Cookie` (§6.5.2.4): what `connect` returns for a
/// multiplex receptacle and `disconnect` takes back.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cookie(pub Vec<u8>);

/// The exceptions of the `Components` module the fragment operations
/// raise (§6.4.3, §6.5.2, §6.7, §7.5), plus `OBJECT_NOT_EXIST` for an
/// operation on a removed fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CcmError {
    /// `Components::InvalidName`: no port of that name and kind.
    InvalidName(String),
    /// `Components::InvalidConnection`: the object is nil or not of the
    /// receptacle's type, or the cookie names no connection.
    InvalidConnection(String),
    /// `Components::AlreadyConnected`: the simplex receptacle has a
    /// connection.
    AlreadyConnected(String),
    /// `Components::ExceededConnectionLimit`.
    ExceededConnectionLimit(String),
    /// `Components::CookieRequired`: a multiplex `disconnect` without a
    /// cookie.
    CookieRequired(String),
    /// `Components::NoConnection`: the simplex receptacle has none.
    NoConnection(String),
    /// `Components::InvalidConfiguration` (§7.5.2).
    InvalidConfiguration(String),
    /// `Components::CreateFailure` (§7.5.3.1).
    CreateFailure(String),
    /// `Components::RemoveFailure` (§7.5.2.2).
    RemoveFailure(String),
    /// `Components::CCMException`: a system-level error of an executor.
    CcmException(String),
    /// `CORBA::OBJECT_NOT_EXIST`: the fragment was removed.
    ObjectNotExist,
}

impl CcmError {
    /// The repository id of the exception.
    #[must_use]
    pub fn repository_id(&self) -> &'static str {
        match self {
            Self::InvalidName(_) => "IDL:omg.org/Components/InvalidName:1.0",
            Self::InvalidConnection(_) => "IDL:omg.org/Components/InvalidConnection:1.0",
            Self::AlreadyConnected(_) => "IDL:omg.org/Components/AlreadyConnected:1.0",
            Self::ExceededConnectionLimit(_) => {
                "IDL:omg.org/Components/ExceededConnectionLimit:1.0"
            }
            Self::CookieRequired(_) => "IDL:omg.org/Components/CookieRequired:1.0",
            Self::NoConnection(_) => "IDL:omg.org/Components/NoConnection:1.0",
            Self::InvalidConfiguration(_) => "IDL:omg.org/Components/InvalidConfiguration:1.0",
            Self::CreateFailure(_) => "IDL:omg.org/Components/CreateFailure:1.0",
            Self::RemoveFailure(_) => "IDL:omg.org/Components/RemoveFailure:1.0",
            Self::CcmException(_) => "IDL:omg.org/Components/CCMException:1.0",
            Self::ObjectNotExist => "IDL:omg.org/CORBA/OBJECT_NOT_EXIST:1.0",
        }
    }
}

impl core::fmt::Display for CcmError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidName(s)
            | Self::InvalidConnection(s)
            | Self::AlreadyConnected(s)
            | Self::ExceededConnectionLimit(s)
            | Self::CookieRequired(s)
            | Self::NoConnection(s)
            | Self::InvalidConfiguration(s)
            | Self::CreateFailure(s)
            | Self::RemoveFailure(s)
            | Self::CcmException(s) => write!(f, "{}: {s}", self.repository_id()),
            Self::ObjectNotExist => f.write_str(self.repository_id()),
        }
    }
}

impl std::error::Error for CcmError {}

fn poisoned() -> CcmError {
    CcmError::CcmException("a lock of the fragment was poisoned".to_string())
}

/// `Components::FacetDescription` (§6.4.3): name, type and reference.
#[derive(Debug, Clone)]
pub struct FacetDescription {
    /// The facet's name.
    pub name: String,
    /// The repository id of its interface.
    pub type_id: String,
    /// The facet's reference.
    pub facet_ref: ObjectRef,
}

/// `Components::ConnectionDescription` (§6.5.3): the cookie (none for a
/// simplex receptacle) and the connected object.
#[derive(Debug, Clone)]
pub struct ConnectionDescription {
    /// The cookie of a multiplex connection.
    pub ck: Option<Cookie>,
    /// The connected object.
    pub objref: ObjectRef,
}

/// `Components::ReceptacleDescription` (§6.5.3).
#[derive(Debug, Clone)]
pub struct ReceptacleDescription {
    /// The receptacle's name.
    pub name: String,
    /// The repository id of its interface.
    pub type_id: String,
    /// `uses multiple`.
    pub is_multiple: bool,
    /// The connections it has.
    pub connections: Vec<ConnectionDescription>,
}

/// A collocated facet object that knows its interface: the local
/// object a fragment or component hands out for a facet. `connect`
/// checks the type of a local object through it; the servant is reached
/// with [`LocalFacet::narrow`].
pub struct LocalFacet {
    type_ids: Vec<String>,
    servant: Arc<dyn Any + Send + Sync>,
}

impl core::fmt::Debug for LocalFacet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LocalFacet")
            .field("type_ids", &self.type_ids)
            .finish()
    }
}

impl LocalFacet {
    /// The local object of `servant` implementing the interface
    /// `type_id` and the interfaces `bases` it derives from.
    #[must_use]
    pub fn object<T: Any + Send + Sync>(
        type_id: &str,
        bases: &[&str],
        servant: Arc<T>,
    ) -> ObjectRef {
        let mut type_ids = Vec::with_capacity(bases.len() + 1);
        type_ids.push(type_id.to_string());
        type_ids.extend(bases.iter().map(|b| (*b).to_string()));
        ObjectRef::local_object(Arc::new(Self { type_ids, servant }))
    }

    /// The servant of a [`LocalFacet`] object as `T`.
    #[must_use]
    pub fn narrow<T: Any + Send + Sync>(object: &ObjectRef) -> Option<Arc<T>> {
        let facet = object.local::<Self>()?;
        Arc::clone(&facet.servant).downcast::<T>().ok()
    }

    /// Whether a [`LocalFacet`] object implements `type_id`; `None` for
    /// any other object.
    #[must_use]
    pub fn is_a(object: &ObjectRef, type_id: &str) -> Option<bool> {
        let facet = object.local::<Self>()?;
        Some(
            type_id == "IDL:omg.org/CORBA/Object:1.0"
                || facet.type_ids.iter().any(|t| t == type_id),
        )
    }
}

/// Whether two references denote the same object: the same local object
/// or the same IOR.
fn same_object(a: &ObjectRef, b: &ObjectRef) -> bool {
    match (a, b) {
        (ObjectRef::Local(x), ObjectRef::Local(y)) => Arc::ptr_eq(x, y),
        (ObjectRef::Reference(x), ObjectRef::Reference(y)) => {
            x.type_id == y.type_id && x.iiop_profile == y.iiop_profile
        }
        (ObjectRef::Group(x, _), ObjectRef::Group(y, _)) => x == y,
        _ => false,
    }
}

/// Navigation, Receptacles and the life-cycle operations of
/// `Components::CCMObject` a deployment tool calls on a fragment or a
/// component (§7.5, §7.5.1): what the launch of a plan needs to connect
/// basic ports and to complete the configuration.
pub trait CcmObject: Send + Sync {
    /// `Navigation::provide_facet`.
    ///
    /// # Errors
    /// `InvalidName`, `CCMException`, `OBJECT_NOT_EXIST`.
    fn provide_facet(&self, name: &str) -> Result<ObjectRef, CcmError>;

    /// `Receptacles::connect`: `Some` cookie for a multiplex receptacle.
    ///
    /// # Errors
    /// `InvalidName`, `InvalidConnection`, `AlreadyConnected`,
    /// `ExceededConnectionLimit`, `OBJECT_NOT_EXIST`.
    fn connect(&self, name: &str, connection: ObjectRef) -> Result<Option<Cookie>, CcmError>;

    /// `Receptacles::disconnect`.
    ///
    /// # Errors
    /// `InvalidName`, `InvalidConnection`, `CookieRequired`,
    /// `NoConnection`, `OBJECT_NOT_EXIST`.
    fn disconnect(&self, name: &str, ck: Option<&Cookie>) -> Result<ObjectRef, CcmError>;

    /// `CCMObject::configuration_complete` (§7.5.2).
    ///
    /// # Errors
    /// `InvalidConfiguration`.
    fn configuration_complete(&self) -> Result<(), CcmError>;

    /// `CCMObject::remove` (§7.5.2.2).
    ///
    /// # Errors
    /// `RemoveFailure`.
    fn remove(&self) -> Result<(), CcmError>;
}

// ---------------------------------------------------------------------------
// What the connector provider implements
// ---------------------------------------------------------------------------

/// The executor of one fragment (§7.5.6): the behaviour behind the basic
/// ports of the connector. The fragment ([`ConnectorFragment`]) does the
/// generic part of Navigation and Receptacles and calls this.
pub trait FragmentExecutor: Send + Sync {
    /// The object implementing the facet `facet` (a basic facet name of
    /// the connector). `None` is an error of the executor the fragment
    /// reports as `CCMException`.
    fn provide(&self, facet: &str) -> Option<ObjectRef>;

    /// Sets the attribute `name` (a connector attribute, §7.2.3.2) to
    /// `value`: the configuration of §7.5.4 and §7.6.4.
    ///
    /// # Errors
    /// `InvalidConfiguration` for a value the executor cannot take.
    fn set_attribute(&self, name: &str, value: &CorbaAny) -> Result<(), CcmError>;

    /// The receptacle `receptacle` was connected to `connection`.
    ///
    /// # Errors
    /// `InvalidConnection` when the executor refuses the connection; the
    /// fragment then does not record it.
    fn connected(&self, receptacle: &str, connection: &ObjectRef) -> Result<(), CcmError> {
        let _ = (receptacle, connection);
        Ok(())
    }

    /// The connection `connection` of `receptacle` was removed.
    fn disconnected(&self, receptacle: &str, connection: &ObjectRef) {
        let _ = (receptacle, connection);
    }

    /// `configuration_complete` (§7.5.2): the handshake between the
    /// fragments of a connector after all of them were configured; the
    /// executor reads its connections from `fragment`.
    ///
    /// # Errors
    /// `InvalidConfiguration`.
    fn configuration_complete(&self, fragment: &ConnectorFragment) -> Result<(), CcmError>;

    /// `remove` (§7.5.2.2).
    ///
    /// # Errors
    /// `RemoveFailure` for an application failure.
    fn remove(&self) -> Result<(), CcmError> {
        Ok(())
    }
}

/// The `Components::HomeExecutorBase` of a connector (§7.5.3): what the
/// "home factory" entry point returns. It knows the connector's basic
/// ports and creates the executor of a new fragment.
pub trait ConnectorHomeExecutor: Send + Sync {
    /// The basic ports and attributes of the connector type, e.g. from
    /// [`crate::connector::ConnectorTypeTable::basic_ports`].
    fn ports(&self) -> ComponentDef;

    /// A new fragment executor.
    ///
    /// # Errors
    /// `CreateFailure`.
    fn create(&self) -> Result<Arc<dyn FragmentExecutor>, CcmError>;
}

/// The name of the execution parameter that names the entry point of a
/// connector's home (§7.6.3.1; §15.7.1 for a component's home).
pub const HOME_FACTORY_EXEC_PARAMETER: &str = "home factory";

/// The name of the execution parameter that names the entry point of a
/// component (§15.7.1).
pub const COMPONENT_FACTORY_EXEC_PARAMETER: &str = "component factory";

/// An entry point: no parameters, returns the home executor (§7.5.3,
/// §7.6.3.1).
pub type HomeFactory = fn() -> Box<dyn ConnectorHomeExecutor>;

/// The entry points a node knows, by the name the "home factory"
/// execution parameter carries.
#[derive(Debug, Default, Clone)]
pub struct HomeFactories {
    by_name: BTreeMap<String, HomeFactory>,
}

impl HomeFactories {
    /// No entry points.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `factory` as the entry point `name`, replacing one of
    /// the same name.
    pub fn register(&mut self, name: &str, factory: HomeFactory) {
        self.by_name.insert(name.to_string(), factory);
    }

    /// The entry point `name`.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<HomeFactory> {
        self.by_name.get(name).copied()
    }

    /// The names of the entry points.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.by_name.keys().cloned().collect()
    }
}

// ---------------------------------------------------------------------------
// The fragment: Components::CCMObject (§7.5.1, §7.5.2)
// ---------------------------------------------------------------------------

#[derive(Default)]
struct FragmentState {
    facets: BTreeMap<String, ObjectRef>,
    connections: BTreeMap<String, Vec<(Cookie, ObjectRef)>>,
    next_cookie: u64,
    configured: bool,
    removed: bool,
}

/// One fragment of a connector: the `Components::CCMObject` of §7.5 over
/// the connector's basic ports.
pub struct ConnectorFragment {
    ports: ComponentDef,
    executor: Arc<dyn FragmentExecutor>,
    home: Weak<ConnectorHome>,
    interfaces: Option<Arc<zerodds_corba_ir::Repository>>,
    state: Mutex<FragmentState>,
}

impl core::fmt::Debug for ConnectorFragment {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ConnectorFragment")
            .field("connector", &self.ports.repository_id)
            .finish()
    }
}

impl ConnectorFragment {
    /// The basic ports and attributes of the fragment.
    #[must_use]
    pub fn ports(&self) -> &ComponentDef {
        &self.ports
    }

    /// `CCMObject::get_ccm_home` (§7.5.2.1): the home that created the
    /// fragment, while it exists.
    #[must_use]
    pub fn get_ccm_home(&self) -> Option<Arc<ConnectorHome>> {
        self.home.upgrade()
    }

    /// Whether `configuration_complete` succeeded.
    #[must_use]
    pub fn is_configured(&self) -> bool {
        self.state.lock().map(|s| s.configured).unwrap_or(false)
    }

    /// Whether the fragment was removed.
    #[must_use]
    pub fn is_removed(&self) -> bool {
        self.state.lock().map(|s| s.removed).unwrap_or(true)
    }

    fn live(&self) -> Result<std::sync::MutexGuard<'_, FragmentState>, CcmError> {
        let state = self.state.lock().map_err(|_| poisoned())?;
        if state.removed {
            return Err(CcmError::ObjectNotExist);
        }
        Ok(state)
    }

    fn facet_def(&self, name: &str) -> Result<&FacetDef, CcmError> {
        self.ports
            .facets
            .iter()
            .find(|f| f.name == name)
            .ok_or_else(|| CcmError::InvalidName(name.to_string()))
    }

    fn receptacle_def(&self, name: &str) -> Result<&ReceptacleDef, CcmError> {
        self.ports
            .receptacles
            .iter()
            .find(|r| r.name == name)
            .ok_or_else(|| CcmError::InvalidName(name.to_string()))
    }

    /// The object of facet `name`: the one handed out before, else the
    /// executor's (called without the fragment's lock held).
    fn facet_object(&self, name: &str) -> Result<ObjectRef, CcmError> {
        self.facet_def(name)?;
        if let Some(object) = self.live()?.facets.get(name) {
            return Ok(object.clone());
        }
        let object = self.executor.provide(name).ok_or_else(|| {
            CcmError::CcmException(format!(
                "the executor provides no object for facet `{name}`"
            ))
        })?;
        let mut state = self.live()?;
        Ok(state
            .facets
            .entry(name.to_string())
            .or_insert(object)
            .clone())
    }

    /// Whether `object` is of the interface `type_id`: a [`LocalFacet`]
    /// by its interfaces, a reference by its type id and the interface
    /// repository. Any other object is refused: its type cannot be
    /// established.
    fn conforms(&self, object: &ObjectRef, type_id: &str) -> Result<(), CcmError> {
        if object.is_nil() {
            return Err(CcmError::InvalidConnection(
                "a nil reference cannot be connected".to_string(),
            ));
        }
        let ok = match object {
            ObjectRef::Local(_) => LocalFacet::is_a(object, type_id).ok_or_else(|| {
                CcmError::InvalidConnection(
                    "a local object that is no LocalFacet has no known type".to_string(),
                )
            })?,
            ObjectRef::Reference(r) => {
                r.type_id == type_id
                    || self
                        .interfaces
                        .as_ref()
                        .and_then(|repo| repo.interface_is_a(&r.type_id, type_id))
                        .unwrap_or(false)
            }
            ObjectRef::Group(..) => {
                return Err(CcmError::InvalidConnection(
                    "a group reference has no single interface type".to_string(),
                ));
            }
        };
        if ok {
            Ok(())
        } else {
            Err(CcmError::InvalidConnection(format!(
                "the object is not of type `{type_id}`"
            )))
        }
    }

    /// `Navigation::get_all_facets`.
    ///
    /// # Errors
    /// `CCMException` when the executor provides no object for a facet,
    /// `OBJECT_NOT_EXIST`.
    pub fn get_all_facets(&self) -> Result<Vec<FacetDescription>, CcmError> {
        let names: Vec<&str> = self.ports.facets.iter().map(|f| f.name.as_str()).collect();
        self.get_named_facets(&names)
    }

    /// `Navigation::get_named_facets`.
    ///
    /// # Errors
    /// `InvalidName` for a name that is no facet, `CCMException`,
    /// `OBJECT_NOT_EXIST`.
    pub fn get_named_facets(&self, names: &[&str]) -> Result<Vec<FacetDescription>, CcmError> {
        drop(self.live()?);
        let mut out = Vec::with_capacity(names.len());
        for name in names {
            let def = self.facet_def(name)?;
            let type_id = def.interface_id.clone();
            let facet_ref = self.facet_object(name)?;
            out.push(FacetDescription {
                name: (*name).to_string(),
                type_id,
                facet_ref,
            });
        }
        Ok(out)
    }

    /// `Navigation::same_component`: whether `object` is one of the
    /// fragment's facets.
    #[must_use]
    pub fn same_component(&self, object: &ObjectRef) -> bool {
        self.state
            .lock()
            .map(|s| s.facets.values().any(|f| same_object(f, object)))
            .unwrap_or(false)
    }

    /// `Receptacles::get_connections`.
    ///
    /// # Errors
    /// `InvalidName`, `OBJECT_NOT_EXIST`.
    pub fn get_connections(&self, name: &str) -> Result<Vec<ConnectionDescription>, CcmError> {
        let state = self.live()?;
        let def = self.receptacle_def(name)?;
        Ok(describe(&state, def))
    }

    /// `Receptacles::get_all_receptacles`.
    ///
    /// # Errors
    /// `OBJECT_NOT_EXIST`.
    pub fn get_all_receptacles(&self) -> Result<Vec<ReceptacleDescription>, CcmError> {
        let names: Vec<&str> = self
            .ports
            .receptacles
            .iter()
            .map(|r| r.name.as_str())
            .collect();
        self.get_named_receptacles(&names)
    }

    /// `Receptacles::get_named_receptacles`.
    ///
    /// # Errors
    /// `InvalidName`, `OBJECT_NOT_EXIST`.
    pub fn get_named_receptacles(
        &self,
        names: &[&str],
    ) -> Result<Vec<ReceptacleDescription>, CcmError> {
        let state = self.live()?;
        let mut out = Vec::with_capacity(names.len());
        for name in names {
            let def = self.receptacle_def(name)?;
            out.push(ReceptacleDescription {
                name: def.name.clone(),
                type_id: def.interface_id.clone(),
                is_multiple: def.multiplicity == ReceptacleMultiplicity::Multiplex,
                connections: describe(&state, def),
            });
        }
        Ok(out)
    }

    /// The objects connected to the receptacle `name`, for the executor
    /// (the `get_connection_<name>` of a context).
    ///
    /// # Errors
    /// `InvalidName`, `OBJECT_NOT_EXIST`.
    pub fn connected_objects(&self, name: &str) -> Result<Vec<ObjectRef>, CcmError> {
        let state = self.live()?;
        self.receptacle_def(name)?;
        Ok(state
            .connections
            .get(name)
            .map(|c| c.iter().map(|(_, o)| o.clone()).collect())
            .unwrap_or_default())
    }
}

fn describe(state: &FragmentState, def: &ReceptacleDef) -> Vec<ConnectionDescription> {
    let multiple = def.multiplicity == ReceptacleMultiplicity::Multiplex;
    state
        .connections
        .get(&def.name)
        .map(|c| {
            c.iter()
                .map(|(ck, objref)| ConnectionDescription {
                    ck: multiple.then(|| ck.clone()),
                    objref: objref.clone(),
                })
                .collect()
        })
        .unwrap_or_default()
}

impl CcmObject for ConnectorFragment {
    fn provide_facet(&self, name: &str) -> Result<ObjectRef, CcmError> {
        self.facet_object(name)
    }

    fn connect(&self, name: &str, connection: ObjectRef) -> Result<Option<Cookie>, CcmError> {
        let def = self.receptacle_def(name)?;
        self.conforms(&connection, &def.interface_id)?;
        let multiple = def.multiplicity == ReceptacleMultiplicity::Multiplex;
        let occupied = |state: &FragmentState| {
            !multiple && state.connections.get(name).is_some_and(|c| !c.is_empty())
        };
        if occupied(&*self.live()?) {
            return Err(CcmError::AlreadyConnected(name.to_string()));
        }
        // The executor is called without the fragment's lock held: it may
        // read the fragment's connections.
        self.executor.connected(name, &connection)?;
        let mut state = self.live()?;
        if occupied(&state) {
            drop(state);
            self.executor.disconnected(name, &connection);
            return Err(CcmError::AlreadyConnected(name.to_string()));
        }
        state.next_cookie += 1;
        let ck = Cookie(state.next_cookie.to_be_bytes().to_vec());
        state
            .connections
            .entry(name.to_string())
            .or_default()
            .push((ck.clone(), connection));
        Ok(multiple.then_some(ck))
    }

    fn disconnect(&self, name: &str, ck: Option<&Cookie>) -> Result<ObjectRef, CcmError> {
        let mut state = self.live()?;
        let def = self.receptacle_def(name)?;
        let multiple = def.multiplicity == ReceptacleMultiplicity::Multiplex;
        let list = state.connections.entry(name.to_string()).or_default();
        let index = if multiple {
            let ck = ck.ok_or_else(|| CcmError::CookieRequired(name.to_string()))?;
            list.iter().position(|(c, _)| c == ck).ok_or_else(|| {
                CcmError::InvalidConnection(format!("the cookie names no connection of `{name}`"))
            })?
        } else {
            if list.is_empty() {
                return Err(CcmError::NoConnection(name.to_string()));
            }
            0
        };
        let (_, object) = list.remove(index);
        drop(state);
        self.executor.disconnected(name, &object);
        Ok(object)
    }

    fn configuration_complete(&self) -> Result<(), CcmError> {
        {
            let state = self.live()?;
            if state.configured {
                return Err(CcmError::InvalidConfiguration(
                    "configuration_complete was called before".to_string(),
                ));
            }
        }
        self.executor.configuration_complete(self)?;
        self.state.lock().map_err(|_| poisoned())?.configured = true;
        Ok(())
    }

    fn remove(&self) -> Result<(), CcmError> {
        {
            drop(self.live()?);
        }
        self.executor.remove()?;
        let connections = {
            let mut state = self.state.lock().map_err(|_| poisoned())?;
            state.removed = true;
            state.facets.clear();
            core::mem::take(&mut state.connections)
        };
        for (receptacle, list) in connections {
            for (_, object) in list {
                self.executor.disconnected(&receptacle, &object);
            }
        }
        if let Some(home) = self.home.upgrade() {
            home.forget(self);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// The home: KeylessCCMHome, CCMHome, HomeConfiguration (§7.5.3, §7.5.4)
// ---------------------------------------------------------------------------

/// The home of a connector's fragments: `KeylessCCMHome::create_component`,
/// `CCMHome::remove_component` and
/// `HomeConfiguration::set_configuration_values` over a
/// [`ConnectorHomeExecutor`].
pub struct ConnectorHome {
    executor: Box<dyn ConnectorHomeExecutor>,
    ports: ComponentDef,
    interfaces: Option<Arc<zerodds_corba_ir::Repository>>,
    configuration: Mutex<Vec<ConfigValue>>,
    fragments: Mutex<Vec<Arc<ConnectorFragment>>>,
    me: Weak<ConnectorHome>,
}

impl core::fmt::Debug for ConnectorHome {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ConnectorHome")
            .field("connector", &self.ports.repository_id)
            .finish()
    }
}

impl ConnectorHome {
    /// The home over the executor an entry point returned.
    #[must_use]
    pub fn new(executor: Box<dyn ConnectorHomeExecutor>) -> Arc<Self> {
        Self::build(executor, None)
    }

    /// As [`Self::new`], with the interface repository `connect` asks
    /// whether the type of a reference derives from a receptacle's
    /// interface.
    #[must_use]
    pub fn with_interface_repository(
        executor: Box<dyn ConnectorHomeExecutor>,
        interfaces: Arc<zerodds_corba_ir::Repository>,
    ) -> Arc<Self> {
        Self::build(executor, Some(interfaces))
    }

    fn build(
        executor: Box<dyn ConnectorHomeExecutor>,
        interfaces: Option<Arc<zerodds_corba_ir::Repository>>,
    ) -> Arc<Self> {
        let ports = executor.ports();
        Arc::new_cyclic(|me| Self {
            executor,
            ports,
            interfaces,
            configuration: Mutex::new(Vec::new()),
            fragments: Mutex::new(Vec::new()),
            me: me.clone(),
        })
    }

    /// The basic ports and attributes of the connector.
    #[must_use]
    pub fn ports(&self) -> &ComponentDef {
        &self.ports
    }

    /// `HomeConfiguration::set_configuration_values` (§7.5.4.1): the
    /// values `create_component` applies to every fragment it creates
    /// from now on.
    pub fn set_configuration_values(&self, config: Vec<ConfigValue>) {
        if let Ok(mut c) = self.configuration.lock() {
            *c = config;
        }
    }

    /// `KeylessCCMHome::create_component` (§7.5.3.1): a new fragment
    /// with the configuration values applied.
    ///
    /// # Errors
    /// `CreateFailure` when the executor fails or a configuration value
    /// names no writable attribute of the connector or is refused.
    pub fn create_component(&self) -> Result<Arc<ConnectorFragment>, CcmError> {
        let configuration = self.configuration.lock().map_err(|_| poisoned())?.clone();
        for value in &configuration {
            match self.ports.attributes.iter().find(|a| a.name == value.name) {
                None => {
                    return Err(CcmError::CreateFailure(format!(
                        "the connector has no attribute `{}`",
                        value.name
                    )));
                }
                Some(a) if a.readonly => {
                    return Err(CcmError::CreateFailure(format!(
                        "attribute `{}` is readonly",
                        value.name
                    )));
                }
                Some(_) => {}
            }
        }
        let executor = self.executor.create().map_err(|e| match e {
            CcmError::CreateFailure(s) => CcmError::CreateFailure(s),
            other => CcmError::CreateFailure(other.to_string()),
        })?;
        for value in &configuration {
            executor
                .set_attribute(&value.name, &value.value)
                .map_err(|e| CcmError::CreateFailure(e.to_string()))?;
        }
        let fragment = Arc::new(ConnectorFragment {
            ports: self.ports.clone(),
            executor,
            home: self.me.clone(),
            interfaces: self.interfaces.clone(),
            state: Mutex::new(FragmentState::default()),
        });
        self.fragments
            .lock()
            .map_err(|_| poisoned())?
            .push(Arc::clone(&fragment));
        Ok(fragment)
    }

    /// `CCMHome::remove_component`: removes a fragment of this home.
    ///
    /// # Errors
    /// `RemoveFailure` for a fragment of another home or when the
    /// executor fails.
    pub fn remove_component(&self, fragment: &ConnectorFragment) -> Result<(), CcmError> {
        let mine = self
            .fragments
            .lock()
            .map_err(|_| poisoned())?
            .iter()
            .any(|f| core::ptr::eq(Arc::as_ptr(f), fragment));
        if !mine {
            return Err(CcmError::RemoveFailure(
                "the fragment does not belong to this home".to_string(),
            ));
        }
        fragment.remove().map_err(|e| match e {
            CcmError::RemoveFailure(s) => CcmError::RemoveFailure(s),
            other => CcmError::RemoveFailure(other.to_string()),
        })
    }

    /// The fragments this home created and that were not removed.
    #[must_use]
    pub fn fragments(&self) -> Vec<Arc<ConnectorFragment>> {
        self.fragments.lock().map(|f| f.clone()).unwrap_or_default()
    }

    fn forget(&self, fragment: &ConnectorFragment) {
        if let Ok(mut f) = self.fragments.lock() {
            f.retain(|x| !core::ptr::eq(Arc::as_ptr(x), fragment));
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::connector::tests::{CONTROL, DATA, table};
    use zerodds_corba_rt::ObjectReference;
    use zerodds_corba_rt::any::AnyValue;

    #[derive(Default)]
    struct Recorder {
        attributes: Mutex<Vec<(String, AnyValue)>>,
        connected: Mutex<Vec<String>>,
        disconnected: Mutex<Vec<String>>,
        completed_with: Mutex<Vec<usize>>,
        removed: Mutex<bool>,
        fail_complete: bool,
    }

    struct Servant;

    struct Exec(Arc<Recorder>);

    impl FragmentExecutor for Exec {
        fn provide(&self, facet: &str) -> Option<ObjectRef> {
            let type_id = if facet == "cc_control" { CONTROL } else { DATA };
            Some(LocalFacet::object(type_id, &[], Arc::new(Servant)))
        }
        fn set_attribute(&self, name: &str, value: &CorbaAny) -> Result<(), CcmError> {
            if name == "topic" && !matches!(value.0, AnyValue::Str(_)) {
                return Err(CcmError::InvalidConfiguration("topic is a string".into()));
            }
            self.0
                .attributes
                .lock()
                .unwrap()
                .push((name.into(), value.0.clone()));
            Ok(())
        }
        fn connected(&self, receptacle: &str, _: &ObjectRef) -> Result<(), CcmError> {
            self.0.connected.lock().unwrap().push(receptacle.into());
            Ok(())
        }
        fn disconnected(&self, receptacle: &str, _: &ObjectRef) {
            self.0.disconnected.lock().unwrap().push(receptacle.into());
        }
        fn configuration_complete(&self, fragment: &ConnectorFragment) -> Result<(), CcmError> {
            if self.0.fail_complete {
                return Err(CcmError::InvalidConfiguration("no peer".into()));
            }
            let n = fragment.connected_objects("cc_consumer")?.len();
            self.0.completed_with.lock().unwrap().push(n);
            Ok(())
        }
        fn remove(&self) -> Result<(), CcmError> {
            *self.0.removed.lock().unwrap() = true;
            Ok(())
        }
    }

    struct HomeExec(Arc<Recorder>);

    impl ConnectorHomeExecutor for HomeExec {
        fn ports(&self) -> ComponentDef {
            table().basic_ports("IDL:Demo/Data_Cnx:1.0").unwrap()
        }
        fn create(&self) -> Result<Arc<dyn FragmentExecutor>, CcmError> {
            Ok(Arc::new(Exec(Arc::clone(&self.0))))
        }
    }

    fn home(recorder: &Arc<Recorder>) -> Arc<ConnectorHome> {
        ConnectorHome::new(Box::new(HomeExec(Arc::clone(recorder))))
    }

    fn data_object() -> ObjectRef {
        LocalFacet::object(DATA, &[], Arc::new(Servant))
    }

    #[test]
    fn corba_p3_7_5_a_fragment_answers_navigation_and_receptacles() {
        let recorder = Arc::new(Recorder::default());
        let home = home(&recorder);
        let fragment = home.create_component().unwrap();
        // Navigation over the basic facets of the connector.
        let control = fragment.provide_facet("cc_control").unwrap();
        assert_eq!(LocalFacet::is_a(&control, CONTROL), Some(true));
        assert!(LocalFacet::narrow::<Servant>(&control).is_some());
        assert!(fragment.same_component(&control));
        assert!(!fragment.same_component(&data_object()));
        assert!(matches!(
            fragment.provide_facet("cc_consumer"),
            Err(CcmError::InvalidName(_))
        ));
        let all: Vec<String> = fragment
            .get_all_facets()
            .unwrap()
            .into_iter()
            .map(|d| alloc::format!("{}:{}", d.name, d.type_id))
            .collect();
        assert_eq!(
            all,
            [
                alloc::format!("cc_control:{CONTROL}"),
                alloc::format!("p:{DATA}")
            ]
        );
        assert!(matches!(
            fragment.get_named_facets(&["p", "nope"]),
            Err(CcmError::InvalidName(_))
        ));
        // Receptacles: the mirrored provides is a simplex receptacle.
        assert_eq!(
            fragment.connect("cc_consumer", data_object()).unwrap(),
            None
        );
        assert!(matches!(
            fragment.connect("cc_consumer", data_object()),
            Err(CcmError::AlreadyConnected(_))
        ));
        assert!(matches!(
            fragment.connect("p", data_object()),
            Err(CcmError::InvalidName(_))
        ));
        let described = fragment.get_all_receptacles().unwrap();
        assert_eq!(described.len(), 1);
        assert_eq!(described[0].type_id, DATA);
        assert!(!described[0].is_multiple);
        assert_eq!(described[0].connections.len(), 1);
        assert!(described[0].connections[0].ck.is_none());
        assert_eq!(fragment.get_connections("cc_consumer").unwrap().len(), 1);
        fragment.disconnect("cc_consumer", None).unwrap();
        assert!(matches!(
            fragment.disconnect("cc_consumer", None),
            Err(CcmError::NoConnection(_))
        ));
        assert_eq!(*recorder.connected.lock().unwrap(), ["cc_consumer"]);
        assert_eq!(*recorder.disconnected.lock().unwrap(), ["cc_consumer"]);
        // get_ccm_home is the home that created it.
        assert!(Arc::ptr_eq(&fragment.get_ccm_home().unwrap(), &home));
    }

    #[test]
    fn corba_p3_7_5_connect_checks_the_type_of_the_object() {
        let recorder = Arc::new(Recorder::default());
        let fragment = home(&recorder).create_component().unwrap();
        // A facet of another interface.
        let control = LocalFacet::object(CONTROL, &[], Arc::new(Servant));
        assert!(matches!(
            fragment.connect("cc_consumer", control),
            Err(CcmError::InvalidConnection(_))
        ));
        // A local object without a known type.
        assert!(matches!(
            fragment.connect("cc_consumer", ObjectRef::local_object(Arc::new(Servant))),
            Err(CcmError::InvalidConnection(_))
        ));
        // A nil reference.
        assert!(matches!(
            fragment.connect(
                "cc_consumer",
                ObjectRef::Reference(ObjectReference::default())
            ),
            Err(CcmError::InvalidConnection(_))
        ));
        // A reference of the receptacle's type and a local object of a
        // derived interface.
        let reference = ObjectReference {
            type_id: DATA.into(),
            iiop_profile: alloc::vec![1, 2, 3],
            ..ObjectReference::default()
        };
        fragment
            .connect("cc_consumer", ObjectRef::Reference(reference.clone()))
            .unwrap();
        fragment.disconnect("cc_consumer", None).unwrap();
        let derived = LocalFacet::object("IDL:Demo/Fast_Pusher:1.0", &[DATA], Arc::new(Servant));
        fragment.connect("cc_consumer", derived).unwrap();
        fragment.disconnect("cc_consumer", None).unwrap();
        // A reference of a derived type: only through the interface
        // repository.
        let fast = ObjectReference {
            type_id: "IDL:Demo/Fast_Pusher:1.0".into(),
            ..reference
        };
        assert!(matches!(
            fragment.connect("cc_consumer", ObjectRef::Reference(fast.clone())),
            Err(CcmError::InvalidConnection(_))
        ));
        let mut repo = zerodds_corba_ir::Repository::new();
        repo.register(
            zerodds_corba_ir::Definition::new(
                "IDL:Demo/Fast_Pusher:1.0",
                "Fast_Pusher",
                "1.0",
                zerodds_corba_ir::DefinitionKind::Interface,
            )
            .with_details(zerodds_corba_ir::DefDetails::Interface {
                base_interfaces: alloc::vec![DATA.into()],
                is_abstract: false,
            }),
        )
        .unwrap();
        repo.register(zerodds_corba_ir::Definition::new(
            DATA,
            "Data_Pusher",
            "1.0",
            zerodds_corba_ir::DefinitionKind::Interface,
        ))
        .unwrap();
        let typed = ConnectorHome::with_interface_repository(
            Box::new(HomeExec(Arc::clone(&recorder))),
            Arc::new(repo),
        );
        let fragment = typed.create_component().unwrap();
        fragment
            .connect("cc_consumer", ObjectRef::Reference(fast))
            .unwrap();
    }

    #[test]
    fn corba_p3_7_5_a_multiplex_receptacle_hands_out_cookies() {
        struct MultiHome;
        struct MultiExec;
        impl FragmentExecutor for MultiExec {
            fn provide(&self, _: &str) -> Option<ObjectRef> {
                None
            }
            fn set_attribute(&self, _: &str, _: &CorbaAny) -> Result<(), CcmError> {
                Ok(())
            }
            fn configuration_complete(&self, _: &ConnectorFragment) -> Result<(), CcmError> {
                Ok(())
            }
        }
        impl ConnectorHomeExecutor for MultiHome {
            fn ports(&self) -> ComponentDef {
                ComponentDef {
                    repository_id: "IDL:Demo/M:1.0".into(),
                    receptacles: alloc::vec![ReceptacleDef {
                        name: "out".into(),
                        interface_id: DATA.into(),
                        multiplicity: ReceptacleMultiplicity::Multiplex,
                    }],
                    facets: alloc::vec![FacetDef {
                        name: "f".into(),
                        interface_id: DATA.into(),
                    }],
                    ..ComponentDef::default()
                }
            }
            fn create(&self) -> Result<Arc<dyn FragmentExecutor>, CcmError> {
                Ok(Arc::new(MultiExec))
            }
        }
        let fragment = ConnectorHome::new(Box::new(MultiHome))
            .create_component()
            .unwrap();
        let a = fragment.connect("out", data_object()).unwrap().unwrap();
        let b = fragment.connect("out", data_object()).unwrap().unwrap();
        assert_ne!(a, b);
        let described = fragment.get_named_receptacles(&["out"]).unwrap();
        assert!(described[0].is_multiple);
        assert_eq!(described[0].connections.len(), 2);
        assert!(matches!(
            fragment.disconnect("out", None),
            Err(CcmError::CookieRequired(_))
        ));
        fragment.disconnect("out", Some(&a)).unwrap();
        assert!(matches!(
            fragment.disconnect("out", Some(&a)),
            Err(CcmError::InvalidConnection(_))
        ));
        assert_eq!(fragment.get_connections("out").unwrap().len(), 1);
        // An executor that provides no facet object is reported.
        assert!(matches!(
            fragment.provide_facet("f"),
            Err(CcmError::CcmException(_))
        ));
    }

    #[test]
    fn corba_p3_7_5_4_configuration_values_reach_every_new_fragment() {
        let recorder = Arc::new(Recorder::default());
        let home = home(&recorder);
        home.set_configuration_values(alloc::vec![
            ConfigValue {
                name: "topic".into(),
                value: CorbaAny(AnyValue::Str("Square".into())),
            },
            ConfigValue {
                name: "cc_depth".into(),
                value: CorbaAny(AnyValue::Str("4".into())),
            },
        ]);
        let _ = home.create_component().unwrap();
        let _ = home.create_component().unwrap();
        let seen = recorder.attributes.lock().unwrap().clone();
        assert_eq!(seen.len(), 4);
        assert_eq!(seen[0], ("topic".into(), AnyValue::Str("Square".into())));
        assert_eq!(seen[3], ("cc_depth".into(), AnyValue::Str("4".into())));
        assert_eq!(home.fragments().len(), 2);
        // An unknown attribute, a readonly one and a refused value fail
        // the creation.
        for (name, value) in [
            ("nope", AnyValue::Str("x".into())),
            ("kind", AnyValue::Str("x".into())),
            ("topic", AnyValue::Long(3)),
        ] {
            home.set_configuration_values(alloc::vec![ConfigValue {
                name: name.into(),
                value: CorbaAny(value),
            }]);
            assert!(
                matches!(home.create_component(), Err(CcmError::CreateFailure(_))),
                "{name}"
            );
        }
    }

    #[test]
    fn corba_p3_7_5_2_configuration_complete_and_remove() {
        let recorder = Arc::new(Recorder::default());
        let home = home(&recorder);
        let fragment = home.create_component().unwrap();
        fragment.connect("cc_consumer", data_object()).unwrap();
        fragment.configuration_complete().unwrap();
        assert!(fragment.is_configured());
        assert_eq!(*recorder.completed_with.lock().unwrap(), [1]);
        assert!(matches!(
            fragment.configuration_complete(),
            Err(CcmError::InvalidConfiguration(_))
        ));
        // remove_component on another home's fragment is refused.
        let other = home_of_failing().create_component().unwrap();
        assert!(matches!(
            home.remove_component(&other),
            Err(CcmError::RemoveFailure(_))
        ));
        home.remove_component(&fragment).unwrap();
        assert!(*recorder.removed.lock().unwrap());
        assert!(fragment.is_removed());
        assert!(home.fragments().is_empty());
        assert_eq!(*recorder.disconnected.lock().unwrap(), ["cc_consumer"]);
        assert_eq!(
            fragment.provide_facet("p").unwrap_err(),
            CcmError::ObjectNotExist
        );
        assert_eq!(fragment.remove().unwrap_err(), CcmError::ObjectNotExist);
        // A failing handshake is InvalidConfiguration and leaves the
        // fragment unconfigured.
        assert!(matches!(
            other.configuration_complete(),
            Err(CcmError::InvalidConfiguration(_))
        ));
        assert!(!other.is_configured());
    }

    fn home_of_failing() -> Arc<ConnectorHome> {
        let recorder = Arc::new(Recorder {
            fail_complete: true,
            ..Recorder::default()
        });
        home(&recorder)
    }

    #[test]
    fn corba_p3_7_6_3_1_home_factories_by_entry_point_name() {
        fn create_data_cnx() -> Box<dyn ConnectorHomeExecutor> {
            Box::new(HomeExec(Arc::new(Recorder::default())))
        }
        let mut factories = HomeFactories::new();
        factories.register("create_Data_Cnx", create_data_cnx);
        assert_eq!(factories.names(), ["create_Data_Cnx"]);
        let entry = factories.get("create_Data_Cnx").unwrap();
        let home = ConnectorHome::new(entry());
        assert_eq!(home.ports().repository_id, "IDL:Demo/Data_Cnx:1.0");
        assert!(home.create_component().is_ok());
        assert!(factories.get("create_Other").is_none());
        assert_eq!(HOME_FACTORY_EXEC_PARAMETER, "home factory");
    }

    #[test]
    fn corba_p3_7_5_exceptions_carry_the_components_repository_ids() {
        assert_eq!(
            CcmError::InvalidName("x".into()).repository_id(),
            "IDL:omg.org/Components/InvalidName:1.0"
        );
        assert_eq!(
            CcmError::CreateFailure("x".into()).to_string(),
            "IDL:omg.org/Components/CreateFailure:1.0: x"
        );
    }
}
