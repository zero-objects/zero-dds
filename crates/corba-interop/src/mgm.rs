// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The Multicast Group Manager — CORBA 3.3 Part 2 §11.2.6:
//! `MGM::ObjectGroupFactory`, which is a `PortableGroup::GenericFactory`,
//! `PropertyManager` and `ObjectGroupManager` in one, with the MGM
//! properties of §11.2.6.8.
//!
//! [`Mgm::create_object`] creates an object group and its group
//! reference: a UIPMC profile with the `TAG_GROUP` component (the group
//! domain, a new group id, reference version 1), the `TAG_GROUP_IIOP`
//! component for the operations of `CORBA::Object`, and the IIOP profile
//! of a gateway. A property set later (`set_properties_dynamically`) that
//! changes the reference raises its version; `get_object_group_ref`
//! returns the current one.
//!
//! [`Mgm::serve`] makes the MGM a CORBA object on a [`CorbaServer`]
//! (§11.3.3.1.1: an application obtains the reference of the MGM and
//! invokes `create_object` on it): the operations of
//! `MGM::ObjectGroupFactory` over IIOP, and for each group it creates an
//! object under the MGM's address that answers the operations of
//! `CORBA::Object` for that group (§11.2.6.8.3, §11.3.2). [`MgmProxy`]
//! calls an MGM over IIOP.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex, Weak};

use zerodds_corba_cosnaming::wire::{read_name, write_name};
use zerodds_corba_cosnaming::{Name, NameComponent};
use zerodds_corba_giop::object_ops::{self, ObjectOpReply};
use zerodds_corba_iiop::IiopProfileBody;
use zerodds_corba_ior::miop::{
    DEFAULT_GROUP_DOMAIN, GroupInfo, UipmcProfileBody, group_iiop_component, group_profiles,
    is_multicast_address,
};
use zerodds_corba_ior::{Ior, TaggedProfile};
use zerodds_corba_ir::Repository;
use zerodds_corba_rt::{AnyValue, CorbaAny};
use zerodds_corba_rust::{
    AlignedBody, CompletionStatus, CorbaConnection, CorbaException, ObjectReference,
    SkeletonResult, SystemExceptionKind,
};
use zerodds_wire::{Endianness, Reader, Writer};

use crate::miop::gateway_object_key;
use crate::runtime::CorbaServer;

/// `MGM::GroupCreationMode` `CREATE_ADDRESS_DEFERED` (§11.2.6.7).
pub const CREATE_ADDRESS_DEFERED: i32 = 0;
/// `MGM::GroupCreationMode` `CREATE_ADDRESS_GENERATED`.
pub const CREATE_ADDRESS_GENERATED: i32 = 1;
/// `MGM::GroupCreationMode` `CREATE_ADDRESS_SUPPLIED`.
pub const CREATE_ADDRESS_SUPPLIED: i32 = 2;

/// `org.omg.mgm.GroupCreationMode` (§11.2.6.8.1).
pub const GROUP_CREATION_MODE: &str = "org.omg.mgm.GroupCreationMode";
/// `org.omg.mgm.CreateSpecifyGateway` (§11.2.6.8.2).
pub const CREATE_SPECIFY_GATEWAY: &str = "org.omg.mgm.CreateSpecifyGateway";
/// `org.omg.mgm.SupportImplicitOperations` (§11.2.6.8.3).
pub const SUPPORT_IMPLICIT_OPERATIONS: &str = "org.omg.mgm.SupportImplicitOperations";
/// `org.omg.mgm.CreateIncludeGateway` (§11.2.6.8.4).
pub const CREATE_INCLUDE_GATEWAY: &str = "org.omg.mgm.CreateIncludeGateway";
/// `org.omg.mgm.ProtocolEndpointsIPPort` (§11.2.6.8.5).
pub const PROTOCOL_ENDPOINTS_IP_PORT: &str = "org.omg.mgm.ProtocolEndpointsIPPort";
/// `org.omg.mgm.ProtocolEndpointsIPv4Address` (§11.2.6.8.6).
pub const PROTOCOL_ENDPOINTS_IP_ADDRESS: &str = "org.omg.mgm.ProtocolEndpointsIPv4Address";
/// `org.omg.mgm.GroupDomainId` (§11.2.6.8.7).
pub const GROUP_DOMAIN_ID: &str = "org.omg.mgm.GroupDomainId";

/// `PortableGroup::Property`.
#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    /// `nam`: the name, one component whose id is the property name.
    pub nam: Name,
    /// `val`.
    pub val: CorbaAny,
}

impl Property {
    /// The property `name` with `value`.
    #[must_use]
    pub fn new(name: &str, value: AnyValue) -> Self {
        Self {
            nam: vec![NameComponent::new(name)],
            val: CorbaAny(value),
        }
    }
}

/// `PortableGroup::Properties`, also `Criteria`.
pub type Properties = Vec<Property>;

/// The exceptions of the MGM (§11.2.6.3).
#[derive(Debug, Clone, PartialEq)]
pub enum MgmError {
    /// `ObjectGroupNotFound`.
    ObjectGroupNotFound,
    /// `ObjectNotFound`: no group of this factory creation id.
    ObjectNotFound,
    /// `UnsupportedProperty`: the name is not an MGM property.
    UnsupportedProperty {
        /// `nam`.
        nam: Name,
    },
    /// `InvalidProperty` with its `nam` and `val`: a value of the wrong
    /// type, a property repeated or in conflict, one that can be set only
    /// at creation or once.
    InvalidProperty(Box<Property>),
    /// `CannotMeetCriteria`: the properties are understood but cannot be
    /// met (the creation mode and the endpoints disagree, no address is
    /// free, a gateway is to be included but none is set, the MGM is to
    /// support the implicit operations but is not served).
    CannotMeetCriteria(Properties),
    /// `InvalidCriteria`: criteria of `create_object` the MGM does not
    /// understand (properties that are no MGM property).
    InvalidCriteria(Properties),
    /// A system exception: `NO_IMPLEMENT` for the operations of
    /// `ObjectGroupManager` the MGM does not use (§11.2.6.5).
    System(CorbaException),
}

/// The multicast endpoints the MGM hands out in
/// `CREATE_ADDRESS_GENERATED` mode: `count` consecutive IPv4 addresses
/// from `first`, each with `port`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddressPool {
    /// The first address.
    pub first: Ipv4Addr,
    /// How many addresses.
    pub count: u32,
    /// The port of every group.
    pub port: u16,
}

impl Default for AddressPool {
    /// 239.255.100.0 to 239.255.100.255 (organization-local scope,
    /// RFC 2365), port 7_300.
    fn default() -> Self {
        Self {
            first: Ipv4Addr::new(239, 255, 100, 0),
            count: 256,
            port: 7_300,
        }
    }
}

#[derive(Debug, Clone)]
struct Group {
    type_id: String,
    creation: Properties,
    dynamic: Properties,
    ref_version: u32,
    reference: Ior,
    /// The endpoint the MGM generated for the group, if it did.
    generated: Option<(Ipv4Addr, u16)>,
}

#[derive(Debug, Default)]
struct State {
    defaults: Properties,
    by_type: BTreeMap<String, Properties>,
    groups: BTreeMap<u64, Group>,
    next_group_id: u64,
}

/// `MGM::ObjectGroupFactory` (§11.2.6.9).
pub struct Mgm {
    state: Mutex<State>,
    pool: AddressPool,
    repository: Option<Arc<Repository>>,
    served: Option<Served>,
}

/// Where a served MGM is: the server, its reference and IIOP profile.
struct Served {
    server: CorbaServer,
    reference: ObjectReference,
    profile: IiopProfileBody,
    me: Weak<Mgm>,
}

impl core::fmt::Debug for Mgm {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Mgm")
            .field("pool", &self.pool)
            .field("reference", &self.reference())
            .finish_non_exhaustive()
    }
}

/// The repository id of `MGM::ObjectGroupFactory`.
pub const OBJECT_GROUP_FACTORY_REPOSITORY_ID: &str = "IDL:omg.org/MGM/ObjectGroupFactory:1.0";

/// The interfaces `MGM::ObjectGroupFactory` inherits (§11.2.6.9).
const OBJECT_GROUP_FACTORY_BASES: [&str; 3] = [
    "IDL:omg.org/PortableGroup/GenericFactory:1.0",
    "IDL:omg.org/PortableGroup/PropertyManager:1.0",
    "IDL:omg.org/PortableGroup/ObjectGroupManager:1.0",
];

/// `PortableGroup::ObjectGroupNotFound`.
pub const OBJECT_GROUP_NOT_FOUND_ID: &str = "IDL:omg.org/PortableGroup/ObjectGroupNotFound:1.0";
/// `PortableGroup::ObjectNotFound`.
pub const OBJECT_NOT_FOUND_ID: &str = "IDL:omg.org/PortableGroup/ObjectNotFound:1.0";
/// `PortableGroup::UnsupportedProperty`.
pub const UNSUPPORTED_PROPERTY_ID: &str = "IDL:omg.org/PortableGroup/UnsupportedProperty:1.0";
/// `PortableGroup::InvalidProperty`.
pub const INVALID_PROPERTY_ID: &str = "IDL:omg.org/PortableGroup/InvalidProperty:1.0";
/// `PortableGroup::InvalidCriteria`.
pub const INVALID_CRITERIA_ID: &str = "IDL:omg.org/PortableGroup/InvalidCriteria:1.0";
/// `PortableGroup::CannotMeetCriteria`.
pub const CANNOT_MEET_CRITERIA_ID: &str = "IDL:omg.org/PortableGroup/CannotMeetCriteria:1.0";

fn name_of(p: &Property) -> Option<&str> {
    match p.nam.as_slice() {
        [c] if c.kind.is_empty() => Some(c.id.as_str()),
        _ => None,
    }
}

fn invalid(p: &Property) -> MgmError {
    MgmError::InvalidProperty(Box::new(p.clone()))
}

fn is_endpoint(name: &str) -> bool {
    name == PROTOCOL_ENDPOINTS_IP_PORT || name == PROTOCOL_ENDPOINTS_IP_ADDRESS
}

/// Checks the name and the value type of every property and that no name
/// is repeated (§11.2.6.3).
fn validate(props: &[Property]) -> Result<(), MgmError> {
    let mut seen: Vec<&str> = Vec::new();
    for p in props {
        let Some(name) = name_of(p) else {
            return Err(MgmError::UnsupportedProperty { nam: p.nam.clone() });
        };
        let ok = match name {
            GROUP_CREATION_MODE => matches!(
                p.val.0,
                AnyValue::Long(CREATE_ADDRESS_DEFERED..=CREATE_ADDRESS_SUPPLIED)
            ),
            CREATE_SPECIFY_GATEWAY | SUPPORT_IMPLICIT_OPERATIONS => {
                matches!(p.val.0, AnyValue::ObjRef { .. })
            }
            CREATE_INCLUDE_GATEWAY => matches!(p.val.0, AnyValue::Boolean(_)),
            PROTOCOL_ENDPOINTS_IP_PORT => matches!(p.val.0, AnyValue::UShort(_)),
            PROTOCOL_ENDPOINTS_IP_ADDRESS => {
                matches!(&p.val.0, AnyValue::Str(a) if is_multicast_address(a))
            }
            GROUP_DOMAIN_ID => matches!(&p.val.0, AnyValue::Str(d) if !d.is_empty()),
            _ => return Err(MgmError::UnsupportedProperty { nam: p.nam.clone() }),
        };
        if !ok || seen.contains(&name) {
            return Err(invalid(p));
        }
        seen.push(name);
    }
    Ok(())
}

/// `over` on top of `under`: a property of `over` replaces the one of
/// the same name.
fn layered(under: &[Property], over: &[Property]) -> Properties {
    let mut out: Properties = under
        .iter()
        .filter(|u| !over.iter().any(|o| o.nam == u.nam))
        .cloned()
        .collect();
    out.extend(over.iter().cloned());
    out
}

fn lookup<'a>(props: &'a [Property], name: &str) -> Option<&'a AnyValue> {
    props
        .iter()
        .find(|p| name_of(p) == Some(name))
        .map(|p| &p.val.0)
}

fn object_of(v: Option<&AnyValue>) -> Option<&ObjectReference> {
    match v {
        Some(AnyValue::ObjRef { reference, .. }) => reference.as_ref(),
        _ => None,
    }
}

impl Default for Mgm {
    fn default() -> Self {
        Self::new(AddressPool::default())
    }
}

impl Mgm {
    /// An MGM that generates endpoints from `pool`. It is a local object
    /// until [`Self::serve`]: without a reference of its own it cannot
    /// support the operations of `CORBA::Object` of its groups, and a nil
    /// `SupportImplicitOperations` is refused (§11.2.6.8.3).
    #[must_use]
    pub fn new(pool: AddressPool) -> Self {
        Self {
            state: Mutex::new(State {
                next_group_id: 1,
                ..State::default()
            }),
            pool,
            repository: None,
            served: None,
        }
    }

    /// The interface repository the objects of the groups answer `_is_a`
    /// from: the type of a group and the interfaces it inherits. Without
    /// one only the type of the group and `CORBA::Object` are known.
    #[must_use]
    pub fn with_interface_repository(mut self, repository: Arc<Repository>) -> Self {
        self.repository = Some(repository);
        self
    }

    /// Serves this MGM on `server` under `object_key`, reached at
    /// `host:port` (§11.3.3.1.1): the operations of
    /// `MGM::ObjectGroupFactory` and of `CORBA::Object` on its reference.
    /// Every group created from now on has an object under
    /// `<object_key>/group/<id>` at the same address that answers the
    /// operations of `CORBA::Object` for the group: `_is_a` for its type
    /// and the interfaces it inherits, `_non_existent` `false`; after
    /// `delete_object` `_is_a` is `false` and `_non_existent` `true`
    /// (§11.3.2). A nil `SupportImplicitOperations` puts that object into
    /// the `TAG_GROUP_IIOP` component (§11.2.6.8.3).
    ///
    /// # Errors
    /// `MARSHAL` when the reference of `server` does not encode.
    pub fn serve(
        mut self,
        server: &CorbaServer,
        host: &str,
        port: u16,
        object_key: &[u8],
    ) -> Result<Arc<Self>, CorbaException> {
        let reference =
            server.object_reference(OBJECT_GROUP_FACTORY_REPOSITORY_ID, host, port, object_key);
        let profile = IiopProfileBody::decode_encapsulation(&reference.iiop_profile)
            .map_err(|_| marshal())?;
        let mgm = Arc::new_cyclic(|me| {
            self.served = Some(Served {
                server: server.clone(),
                reference,
                profile,
                me: me.clone(),
            });
            self
        });
        let me = Arc::downgrade(&mgm);
        server.register(object_key, move |op, body, e| match me.upgrade() {
            Some(mgm) => mgm.dispatch(op, body, e),
            None => object_not_exist(),
        });
        Ok(mgm)
    }

    /// The reference of a served MGM.
    #[must_use]
    pub fn reference(&self) -> Option<&ObjectReference> {
        self.served.as_ref().map(|s| &s.reference)
    }

    /// The IIOP profile of the object that answers the operations of
    /// `CORBA::Object` for group `id` on a served MGM.
    fn group_object_profile(&self, id: u64) -> Option<IiopProfileBody> {
        let served = self.served.as_ref()?;
        let mut profile = served.profile.clone();
        profile.object_key = group_object_key(&served.profile.object_key, id);
        Some(profile)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    // -- PropertyManager (§11.2.6.4) ---------------------------------------

    /// `set_default_properties`: the defaults of every group created later.
    ///
    /// # Errors
    /// `InvalidProperty`, `UnsupportedProperty`.
    pub fn set_default_properties(&self, props: &[Property]) -> Result<(), MgmError> {
        validate(props)?;
        let mut g = self.lock();
        g.defaults = layered(&g.defaults, props);
        Ok(())
    }

    /// `get_default_properties`.
    #[must_use]
    pub fn get_default_properties(&self) -> Properties {
        self.lock().defaults.clone()
    }

    /// `remove_default_properties`: removes the defaults of these names.
    ///
    /// # Errors
    /// `InvalidProperty`, `UnsupportedProperty` as for setting them.
    pub fn remove_default_properties(&self, props: &[Property]) -> Result<(), MgmError> {
        validate_names(props)?;
        let mut g = self.lock();
        g.defaults.retain(|d| !props.iter().any(|p| p.nam == d.nam));
        Ok(())
    }

    /// `set_type_properties`: overrides the defaults for the groups of
    /// `type_id` created later.
    ///
    /// # Errors
    /// `InvalidProperty`, `UnsupportedProperty`.
    pub fn set_type_properties(
        &self,
        type_id: &str,
        overrides: &[Property],
    ) -> Result<(), MgmError> {
        validate(overrides)?;
        let mut g = self.lock();
        let entry = g.by_type.entry(type_id.to_string()).or_default();
        *entry = layered(entry, overrides);
        Ok(())
    }

    /// `get_type_properties`: the type's properties and the defaults they
    /// do not override.
    #[must_use]
    pub fn get_type_properties(&self, type_id: &str) -> Properties {
        let g = self.lock();
        layered(
            &g.defaults,
            g.by_type.get(type_id).map_or(&[][..], Vec::as_slice),
        )
    }

    /// `remove_type_properties`.
    ///
    /// # Errors
    /// `InvalidProperty`, `UnsupportedProperty` as for setting them.
    pub fn remove_type_properties(
        &self,
        type_id: &str,
        props: &[Property],
    ) -> Result<(), MgmError> {
        validate_names(props)?;
        if let Some(t) = self.lock().by_type.get_mut(type_id) {
            t.retain(|d| !props.iter().any(|p| p.nam == d.nam));
        }
        Ok(())
    }

    /// `set_properties_dynamically`: overrides the properties of a group
    /// while it lives (§11.2.6.4.1). `GroupCreationMode` can be set only
    /// at creation, `GroupDomainId` only while it has its default; a
    /// change of the endpoints, the gateway or the implicit operations
    /// gives the group a new reference with the next reference version.
    ///
    /// # Errors
    /// `ObjectGroupNotFound`, `InvalidProperty`, `UnsupportedProperty`,
    /// `CannotMeetCriteria` when the new properties cannot make a
    /// reference.
    pub fn set_properties_dynamically(
        &self,
        object_group: &Ior,
        overrides: &[Property],
    ) -> Result<(), MgmError> {
        validate(overrides)?;
        let id = self.group_id_of(object_group)?;
        let mut st = self.lock();
        let base = typed(&st, id).ok_or(MgmError::ObjectGroupNotFound)?;
        let group = st
            .groups
            .get_mut(&id)
            .ok_or(MgmError::ObjectGroupNotFound)?;
        let current = layered(&layered(&base, &group.creation), &group.dynamic);
        for p in overrides {
            match name_of(p) {
                Some(GROUP_CREATION_MODE) => return Err(invalid(p)),
                Some(GROUP_DOMAIN_ID) if domain_of(&current) != DEFAULT_GROUP_DOMAIN => {
                    return Err(invalid(p));
                }
                _ => {}
            }
        }
        let dynamic = layered(&group.dynamic, overrides);
        let effective = layered(&layered(&base, &group.creation), &dynamic);
        // `set_properties_dynamically` raises no `CannotMeetCriteria`: a
        // property with which no reference can be made is in conflict
        // with the others (§11.2.6.3 `InvalidProperty`).
        let conflict = |_| conflicting(overrides);
        let same = self
            .reference_for(
                &group.type_id,
                id,
                group.ref_version,
                &effective,
                group.generated,
            )
            .map_err(conflict)?;
        if same.profiles != group.reference.profiles {
            group.reference = self
                .reference_for(
                    &group.type_id,
                    id,
                    group.ref_version + 1,
                    &effective,
                    group.generated,
                )
                .map_err(conflict)?;
            group.ref_version += 1;
        }
        group.dynamic = dynamic;
        Ok(())
    }

    /// `get_properties`: the properties of the group, each from the most
    /// specific place — set dynamically, at creation, for its type, by
    /// default.
    ///
    /// # Errors
    /// `ObjectGroupNotFound`.
    pub fn get_properties(&self, object_group: &Ior) -> Result<Properties, MgmError> {
        let id = self.group_id_of(object_group)?;
        let st = self.lock();
        effective(&st, id).ok_or(MgmError::ObjectGroupNotFound)
    }

    // -- ObjectGroupManager (§11.2.6.5) -------------------------------------

    /// `get_object_group_id`.
    ///
    /// # Errors
    /// `ObjectGroupNotFound` for a reference that is no group of this MGM.
    pub fn get_object_group_id(&self, object_group: &Ior) -> Result<u64, MgmError> {
        self.group_id_of(object_group)
    }

    /// `get_object_group_ref`: the current reference of the group, with
    /// the endpoints and the reference version it has now.
    ///
    /// # Errors
    /// `ObjectGroupNotFound`, also for a group deleted since (§11.3.2).
    pub fn get_object_group_ref(&self, object_group: &Ior) -> Result<Ior, MgmError> {
        let id = self.group_id_of(object_group)?;
        self.lock()
            .groups
            .get(&id)
            .map(|g| g.reference.clone())
            .ok_or(MgmError::ObjectGroupNotFound)
    }

    /// `create_member`, `add_member`, `remove_member`,
    /// `locations_of_members`, `get_member_ref`: the MGM manages no
    /// membership (§11.2.6.5): `NO_IMPLEMENT`.
    ///
    /// # Errors
    /// Always `NO_IMPLEMENT`.
    pub fn membership_operation(&self, _operation: &str) -> Result<(), MgmError> {
        Err(MgmError::System(CorbaException::system(
            SystemExceptionKind::NoImplement,
            0,
            CompletionStatus::No,
        )))
    }

    // -- GenericFactory (§11.2.6.6) ----------------------------------------

    /// `create_object(type_id, the_criteria)`: a new object group of the
    /// interface `type_id` and its reference, with `the_criteria` over the
    /// properties of the type and the defaults. Returns the reference and
    /// the `factory_creation_id`, an any holding the `ObjectGroupId`.
    ///
    /// # Errors
    /// `InvalidCriteria` for criteria that are no MGM property,
    /// `InvalidProperty` for a value of the wrong type or a repeated
    /// property; `CannotMeetCriteria` when the creation mode and the
    /// endpoint properties disagree (§11.2.6.8.1), no generated address
    /// is free, a gateway is to be included and none is set, or the MGM
    /// is to support the implicit operations and is not served.
    pub fn create_object(
        &self,
        type_id: &str,
        the_criteria: &[Property],
    ) -> Result<(Ior, CorbaAny), MgmError> {
        validate(the_criteria).map_err(|e| match e {
            // `create_object` raises no `UnsupportedProperty`: criteria the
            // factory does not understand are `InvalidCriteria`.
            MgmError::UnsupportedProperty { .. } => MgmError::InvalidCriteria(
                the_criteria
                    .iter()
                    .filter(|p| validate_names(core::slice::from_ref(*p)).is_err())
                    .cloned()
                    .collect(),
            ),
            other => other,
        })?;
        let mut st = self.lock();
        let typed = layered(
            &st.defaults,
            st.by_type.get(type_id).map_or(&[][..], Vec::as_slice),
        );
        let effective = layered(&typed, the_criteria);
        let mode = match lookup(&effective, GROUP_CREATION_MODE) {
            Some(AnyValue::Long(m)) => *m,
            _ => CREATE_ADDRESS_GENERATED,
        };
        let has_endpoints = effective
            .iter()
            .any(|p| name_of(p).is_some_and(is_endpoint));
        let cannot = || MgmError::CannotMeetCriteria(the_criteria.to_vec());
        let generated = match mode {
            CREATE_ADDRESS_DEFERED | CREATE_ADDRESS_GENERATED if has_endpoints => {
                return Err(cannot());
            }
            CREATE_ADDRESS_SUPPLIED
                if lookup(&effective, PROTOCOL_ENDPOINTS_IP_ADDRESS).is_none()
                    || lookup(&effective, PROTOCOL_ENDPOINTS_IP_PORT).is_none() =>
            {
                return Err(cannot());
            }
            CREATE_ADDRESS_GENERATED => Some(self.free_address(&st).ok_or_else(cannot)?),
            _ => None,
        };
        let id = st.next_group_id;
        let reference = self
            .reference_for(type_id, id, 1, &effective, generated)
            .map_err(|_| cannot())?;
        st.next_group_id += 1;
        st.groups.insert(
            id,
            Group {
                type_id: type_id.to_string(),
                creation: the_criteria.to_vec(),
                dynamic: Vec::new(),
                ref_version: 1,
                reference: reference.clone(),
                generated,
            },
        );
        drop(st);
        if let Some(served) = &self.served {
            let me = served.me.clone();
            served.server.register(
                &group_object_key(&served.profile.object_key, id),
                move |op, body, e| match me.upgrade() {
                    Some(mgm) => mgm.dispatch_group(id, op, body, e),
                    None => object_not_exist(),
                },
            );
        }
        Ok((reference, CorbaAny(AnyValue::ULongLong(id))))
    }

    /// `delete_object(factory_creation_id)`: removes the group and its
    /// reference.
    ///
    /// # Errors
    /// `ObjectNotFound` for an id this MGM did not hand out or already
    /// deleted.
    pub fn delete_object(&self, factory_creation_id: &CorbaAny) -> Result<(), MgmError> {
        let AnyValue::ULongLong(id) = factory_creation_id.0 else {
            return Err(MgmError::ObjectNotFound);
        };
        self.lock()
            .groups
            .remove(&id)
            .map(|_| ())
            .ok_or(MgmError::ObjectNotFound)
    }

    // -- the reference -----------------------------------------------------

    fn group_id_of(&self, object_group: &Ior) -> Result<u64, MgmError> {
        let groups = group_profiles(object_group).map_err(|_| MgmError::ObjectGroupNotFound)?;
        let info = groups.groups.first().ok_or(MgmError::ObjectGroupNotFound)?;
        let st = self.lock();
        let props = effective(&st, info.object_group_id).ok_or(MgmError::ObjectGroupNotFound)?;
        if domain_of(&props) == info.group_domain_id {
            Ok(info.object_group_id)
        } else {
            Err(MgmError::ObjectGroupNotFound)
        }
    }

    fn free_address(&self, st: &State) -> Option<(Ipv4Addr, u16)> {
        let base = u32::from(self.pool.first);
        (0..self.pool.count)
            .map(|i| Ipv4Addr::from(base.wrapping_add(i)))
            .find(|a| {
                !st.groups
                    .values()
                    .any(|g| g.generated.is_some_and(|(used, _)| used == *a))
            })
            .map(|a| (a, self.pool.port))
    }

    /// The reference of group `id` with the properties `effective`.
    fn reference_for(
        &self,
        type_id: &str,
        id: u64,
        ref_version: u32,
        effective: &[Property],
        generated: Option<(Ipv4Addr, u16)>,
    ) -> Result<Ior, MgmError> {
        let cannot = || MgmError::CannotMeetCriteria(effective.to_vec());
        let group = GroupInfo::new(domain_of(effective), id, ref_version);
        let endpoint = match (
            lookup(effective, PROTOCOL_ENDPOINTS_IP_ADDRESS),
            lookup(effective, PROTOCOL_ENDPOINTS_IP_PORT),
        ) {
            (Some(AnyValue::Str(a)), Some(AnyValue::UShort(p))) => Some((a.clone(), *p)),
            (None, None) => generated.map(|(a, p)| (IpAddr::V4(a).to_string(), p)),
            _ => return Err(cannot()),
        };
        // §11.2.6.8.3: the object that supports the OMA of the group; a
        // nil value names the MGM, which needs a reference to do so.
        let implicit = match effective
            .iter()
            .find(|p| name_of(p) == Some(SUPPORT_IMPLICIT_OPERATIONS))
        {
            Some(p) => Some(match object_of(Some(&p.val.0)) {
                Some(r) => {
                    IiopProfileBody::decode_encapsulation(&r.iiop_profile).map_err(|_| cannot())?
                }
                None => self.group_object_profile(id).ok_or_else(cannot)?,
            }),
            None => None,
        };
        let mut components = vec![group.to_component(Endianness::Big)];
        if let Some(body) = implicit {
            components.push(group_iiop_component(&body, Endianness::Big).map_err(|_| cannot())?);
        }
        let mut profiles = Vec::new();
        let uipmc =
            endpoint.map(|(address, port)| UipmcProfileBody::new(address, port, components));
        if let Some(u) = &uipmc {
            profiles.push(u.to_tagged_profile(Endianness::Big).map_err(|_| cannot())?);
        }
        if matches!(
            lookup(effective, CREATE_INCLUDE_GATEWAY),
            Some(AnyValue::Boolean(true))
        ) {
            let gateway =
                object_of(lookup(effective, CREATE_SPECIFY_GATEWAY)).ok_or_else(cannot)?;
            let mut body = IiopProfileBody::decode_encapsulation(&gateway.iiop_profile)
                .map_err(|_| cannot())?;
            // The gateway learns the group from the object key (§11.2.3.1);
            // a deferred group has no UIPMC profile to name yet.
            if let Some(u) = &uipmc {
                body.object_key = gateway_object_key(u).map_err(|_| cannot())?;
            }
            body.components.push(group.to_component(Endianness::Big));
            profiles.push(TaggedProfile::iiop(&body, Endianness::Big).map_err(|_| cannot())?);
        }
        Ok(Ior::new(type_id.to_string(), profiles))
    }
}

/// The defaults with the properties of the type of group `id` over them.
fn typed(st: &State, id: u64) -> Option<Properties> {
    let g = st.groups.get(&id)?;
    Some(layered(
        &st.defaults,
        st.by_type.get(&g.type_id).map_or(&[][..], Vec::as_slice),
    ))
}

/// The properties group `id` has now (§11.2.6.4.1 `get_properties`).
fn effective(st: &State, id: u64) -> Option<Properties> {
    let g = st.groups.get(&id)?;
    Some(layered(&layered(&typed(st, id)?, &g.creation), &g.dynamic))
}

fn domain_of(props: &[Property]) -> String {
    match lookup(props, GROUP_DOMAIN_ID) {
        Some(AnyValue::Str(d)) => d.clone(),
        _ => DEFAULT_GROUP_DOMAIN.to_string(),
    }
}

/// The names of `props` must be MGM properties; their values are not
/// looked at (removal).
fn validate_names(props: &[Property]) -> Result<(), MgmError> {
    for p in props {
        match name_of(p) {
            Some(
                GROUP_CREATION_MODE
                | CREATE_SPECIFY_GATEWAY
                | SUPPORT_IMPLICIT_OPERATIONS
                | CREATE_INCLUDE_GATEWAY
                | PROTOCOL_ENDPOINTS_IP_PORT
                | PROTOCOL_ENDPOINTS_IP_ADDRESS
                | GROUP_DOMAIN_ID,
            ) => {}
            _ => return Err(MgmError::UnsupportedProperty { nam: p.nam.clone() }),
        }
    }
    Ok(())
}

// -- the MGM over IIOP (§11.2.6.9, §11.3.2, §11.3.3.1.1) -------------------

/// A value that does not encode or decode.
#[derive(Debug)]
struct Codec;

impl From<zerodds_wire::WireError> for Codec {
    fn from(_: zerodds_wire::WireError) -> Self {
        Self
    }
}

impl From<zerodds_corba_iiop::profile_body::CdrError> for Codec {
    fn from(_: zerodds_corba_iiop::profile_body::CdrError) -> Self {
        Self
    }
}

/// `MARSHAL`, `COMPLETED_NO`.
fn marshal() -> CorbaException {
    CorbaException::system(SystemExceptionKind::Marshal, 0, CompletionStatus::No)
}

/// `MARSHAL`, `COMPLETED_YES`: the operation ran, its reply does not
/// encode or decode.
fn reply_marshal() -> CorbaException {
    CorbaException::system(SystemExceptionKind::Marshal, 0, CompletionStatus::Yes)
}

fn object_not_exist() -> SkeletonResult {
    SkeletonResult::Exception(CorbaException::system(
        SystemExceptionKind::ObjectNotExist,
        0,
        CompletionStatus::No,
    ))
}

/// The object key of the object that answers the operations of
/// `CORBA::Object` for group `id`.
fn group_object_key(mgm_key: &[u8], id: u64) -> Vec<u8> {
    let mut key = mgm_key.to_vec();
    key.extend_from_slice(format!("/group/{id}").as_bytes());
    key
}

/// The `InvalidProperty` for overrides with which no reference can be
/// made: the first override that goes into the reference (endpoints,
/// gateway, implicit operations).
fn conflicting(overrides: &[Property]) -> MgmError {
    overrides
        .iter()
        .find(|p| {
            name_of(p).is_some_and(|n| {
                is_endpoint(n)
                    || n == CREATE_SPECIFY_GATEWAY
                    || n == CREATE_INCLUDE_GATEWAY
                    || n == SUPPORT_IMPLICIT_OPERATIONS
            })
        })
        .or_else(|| overrides.first())
        .map_or_else(
            || {
                MgmError::System(CorbaException::system(
                    SystemExceptionKind::Internal,
                    0,
                    CompletionStatus::No,
                ))
            },
            invalid,
        )
}

/// `PortableGroup::Properties`: a sequence of `Property { Name nam; any
/// val; }`.
fn write_properties(w: &mut Writer, props: &[Property]) -> Result<(), Codec> {
    w.write_u32(u32::try_from(props.len()).map_err(|_| Codec)?);
    for p in props {
        write_name(w, &p.nam)?;
        p.val.encode(w)?;
    }
    Ok(())
}

fn read_properties(r: &mut Reader<'_>) -> Result<Properties, Codec> {
    let n = r.read_u32()? as usize;
    // A property is at least a name count and a TCKind.
    if n > r.remaining() / 8 {
        return Err(Codec);
    }
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let nam = read_name(r)?;
        let val = CorbaAny::decode(r)?;
        out.push(Property { nam, val });
    }
    Ok(out)
}

/// A request to the MGM, its arguments decoded.
enum Call {
    SetDefaultProperties(Properties),
    GetDefaultProperties,
    RemoveDefaultProperties(Properties),
    SetTypeProperties(String, Properties),
    GetTypeProperties(String),
    RemoveTypeProperties(String, Properties),
    SetPropertiesDynamically(Ior, Properties),
    GetProperties(Ior),
    GetObjectGroupId(Ior),
    GetObjectGroupRef(Ior),
    CreateObject(String, Properties),
    DeleteObject(CorbaAny),
    Membership(String),
}

/// The reply of the MGM to a [`Call`].
enum Answer {
    Void,
    Properties(Properties),
    Id(u64),
    Object(Ior),
    Created(Ior, CorbaAny),
}

/// The arguments of `operation`; `None` for an operation the interface
/// does not have.
fn decode_call(operation: &str, r: &mut Reader<'_>) -> Result<Option<Call>, Codec> {
    Ok(Some(match operation {
        "set_default_properties" => Call::SetDefaultProperties(read_properties(r)?),
        "get_default_properties" => Call::GetDefaultProperties,
        "remove_default_properties" => Call::RemoveDefaultProperties(read_properties(r)?),
        "set_type_properties" => Call::SetTypeProperties(r.read_string()?, read_properties(r)?),
        "get_type_properties" => Call::GetTypeProperties(r.read_string()?),
        "remove_type_properties" => {
            Call::RemoveTypeProperties(r.read_string()?, read_properties(r)?)
        }
        "set_properties_dynamically" => {
            Call::SetPropertiesDynamically(Ior::decode(r)?, read_properties(r)?)
        }
        "get_properties" => Call::GetProperties(Ior::decode(r)?),
        "get_object_group_id" => Call::GetObjectGroupId(Ior::decode(r)?),
        "get_object_group_ref" => Call::GetObjectGroupRef(Ior::decode(r)?),
        "create_object" => Call::CreateObject(r.read_string()?, read_properties(r)?),
        "delete_object" => Call::DeleteObject(CorbaAny::decode(r)?),
        "create_member"
        | "add_member"
        | "remove_member"
        | "locations_of_members"
        | "get_member_ref" => Call::Membership(operation.to_string()),
        _ => return Ok(None),
    }))
}

fn encode_answer(answer: &Answer, w: &mut Writer) -> Result<(), Codec> {
    match answer {
        Answer::Void => {}
        Answer::Properties(p) => write_properties(w, p)?,
        Answer::Id(id) => w.write_u64(*id),
        Answer::Object(ior) => ior.encode(w)?,
        Answer::Created(ior, id) => {
            ior.encode(w)?;
            id.encode(w)?;
        }
    }
    Ok(())
}

/// A user exception reply: the repository id and the members.
fn user_exception(
    id: &str,
    e: Endianness,
    members: impl FnOnce(&mut Writer) -> Result<(), Codec>,
) -> SkeletonResult {
    let mut w = Writer::new(e);
    w.write_string(id);
    match members(&mut w) {
        Ok(()) => SkeletonResult::Exception(CorbaException::UserException {
            repository_id: id.to_string(),
            body: zerodds_corba_rust::AlignedBody::new(&w.finish(), 0),
            endianness: e,
        }),
        Err(Codec) => SkeletonResult::Exception(reply_marshal()),
    }
}

/// The reply for an exception of the MGM (§11.2.6.3).
fn exception_reply(err: MgmError, e: Endianness) -> SkeletonResult {
    match err {
        MgmError::ObjectGroupNotFound => user_exception(OBJECT_GROUP_NOT_FOUND_ID, e, |_| Ok(())),
        MgmError::ObjectNotFound => user_exception(OBJECT_NOT_FOUND_ID, e, |_| Ok(())),
        MgmError::UnsupportedProperty { nam } => {
            user_exception(UNSUPPORTED_PROPERTY_ID, e, |w| Ok(write_name(w, &nam)?))
        }
        MgmError::InvalidProperty(p) => user_exception(INVALID_PROPERTY_ID, e, |w| {
            write_name(w, &p.nam)?;
            Ok(p.val.encode(w)?)
        }),
        MgmError::InvalidCriteria(c) => {
            user_exception(INVALID_CRITERIA_ID, e, |w| write_properties(w, &c))
        }
        MgmError::CannotMeetCriteria(c) => {
            user_exception(CANNOT_MEET_CRITERIA_ID, e, |w| write_properties(w, &c))
        }
        MgmError::System(x) => SkeletonResult::Exception(x),
    }
}

/// The exception of an MGM reply as an [`MgmError`]; a user exception
/// that is none of the MGM's stays as it is.
fn decode_exception(x: CorbaException) -> MgmError {
    let CorbaException::UserException {
        repository_id,
        body,
        endianness,
    } = x
    else {
        return MgmError::System(x);
    };
    // Part 2 §9.4.3: the members align from the reply message's start.
    let mut r = body.reader(endianness);
    let members = |r: &mut Reader<'_>| -> Result<Option<MgmError>, Codec> {
        r.read_string()?;
        Ok(Some(match repository_id.as_str() {
            OBJECT_GROUP_NOT_FOUND_ID => MgmError::ObjectGroupNotFound,
            OBJECT_NOT_FOUND_ID => MgmError::ObjectNotFound,
            UNSUPPORTED_PROPERTY_ID => MgmError::UnsupportedProperty { nam: read_name(r)? },
            INVALID_PROPERTY_ID => {
                let nam = read_name(r)?;
                let val = CorbaAny::decode(r)?;
                MgmError::InvalidProperty(Box::new(Property { nam, val }))
            }
            INVALID_CRITERIA_ID => MgmError::InvalidCriteria(read_properties(r)?),
            CANNOT_MEET_CRITERIA_ID => MgmError::CannotMeetCriteria(read_properties(r)?),
            _ => return Ok(None),
        }))
    };
    match members(&mut r) {
        Ok(Some(e)) => e,
        Ok(None) => MgmError::System(CorbaException::UserException {
            repository_id,
            body,
            endianness,
        }),
        Err(Codec) => MgmError::System(reply_marshal()),
    }
}

impl Mgm {
    /// One request to the MGM's own object: the operations of
    /// `CORBA::Object` for `MGM::ObjectGroupFactory` and the interfaces it
    /// inherits, then those of `PropertyManager`, `ObjectGroupManager` and
    /// `GenericFactory`. Arguments that do not decode are `MARSHAL`
    /// `COMPLETED_NO`; an operation the interface does not have is
    /// `BAD_OPERATION`.
    fn dispatch(&self, operation: &str, body: &AlignedBody, e: Endianness) -> SkeletonResult {
        if let Some(r) = zerodds_corba_rust::runtime::dispatch_object_operation(
            operation,
            body,
            e,
            OBJECT_GROUP_FACTORY_REPOSITORY_ID,
            &OBJECT_GROUP_FACTORY_BASES,
        ) {
            return r;
        }
        let call = match decode_call(operation, &mut body.reader(e)) {
            Ok(Some(c)) => c,
            Ok(None) => return SkeletonResult::BadOperation,
            Err(Codec) => return SkeletonResult::Exception(marshal()),
        };
        let outcome = match call {
            Call::SetDefaultProperties(p) => self.set_default_properties(&p).map(|()| Answer::Void),
            Call::GetDefaultProperties => Ok(Answer::Properties(self.get_default_properties())),
            Call::RemoveDefaultProperties(p) => {
                self.remove_default_properties(&p).map(|()| Answer::Void)
            }
            Call::SetTypeProperties(t, p) => {
                self.set_type_properties(&t, &p).map(|()| Answer::Void)
            }
            Call::GetTypeProperties(t) => Ok(Answer::Properties(self.get_type_properties(&t))),
            Call::RemoveTypeProperties(t, p) => {
                self.remove_type_properties(&t, &p).map(|()| Answer::Void)
            }
            Call::SetPropertiesDynamically(g, p) => self
                .set_properties_dynamically(&g, &p)
                .map(|()| Answer::Void),
            Call::GetProperties(g) => self.get_properties(&g).map(Answer::Properties),
            Call::GetObjectGroupId(g) => self.get_object_group_id(&g).map(Answer::Id),
            Call::GetObjectGroupRef(g) => self.get_object_group_ref(&g).map(Answer::Object),
            Call::CreateObject(t, c) => self
                .create_object(&t, &c)
                .map(|(ior, id)| Answer::Created(ior, id)),
            Call::DeleteObject(id) => self.delete_object(&id).map(|()| Answer::Void),
            Call::Membership(op) => self.membership_operation(&op).map(|()| Answer::Void),
        };
        match outcome {
            Ok(answer) => {
                let mut w = Writer::new(e);
                match encode_answer(&answer, &mut w) {
                    Ok(()) => SkeletonResult::Reply(w.finish()),
                    Err(Codec) => SkeletonResult::Exception(reply_marshal()),
                }
            }
            Err(err) => exception_reply(err, e),
        }
    }

    /// One request to the object of group `id` (§11.2.6.8.3, §11.3.2):
    /// while the group lives, the operations of `CORBA::Object` for its
    /// type (`_is_a` also for the interfaces the interface repository
    /// says it inherits, `_non_existent` `false`, `_interface`
    /// `INTF_REPOS`); the operations of the group's interface are not
    /// served here (`NO_IMPLEMENT`). After `delete_object`, `_is_a` is
    /// `false`, `_non_existent` `true` and every other operation
    /// `OBJECT_NOT_EXIST`.
    fn dispatch_group(
        &self,
        id: u64,
        operation: &str,
        body: &AlignedBody,
        e: Endianness,
    ) -> SkeletonResult {
        let type_id = self.lock().groups.get(&id).map(|g| g.type_id.clone());
        let Some(type_id) = type_id else {
            return match operation {
                object_ops::OP_IS_A => match body.reader(e).read_string() {
                    Ok(_) => SkeletonResult::Reply(object_ops::encode_bool(false, e)),
                    Err(_) => SkeletonResult::Exception(marshal()),
                },
                object_ops::OP_NON_EXISTENT | object_ops::OP_NOT_EXISTENT => {
                    SkeletonResult::Reply(object_ops::encode_bool(true, e))
                }
                _ => object_not_exist(),
            };
        };
        let inherits = |candidate: &str| {
            self.repository
                .as_ref()
                .and_then(|r| r.interface_is_a(&type_id, candidate))
                == Some(true)
        };
        match object_ops::dispatch(operation, body, e, &type_id, &inherits) {
            Some(ObjectOpReply::Ok(b)) => SkeletonResult::Reply(b),
            Some(ObjectOpReply::Marshal) => SkeletonResult::Exception(marshal()),
            // Annex A.6: INTF_REPOS minor 1, "Interface Repository not
            // available".
            Some(ObjectOpReply::InterfaceRepositoryUnavailable) => SkeletonResult::Exception(
                CorbaException::omg(SystemExceptionKind::IntfRepos, 1, CompletionStatus::No),
            ),
            None => SkeletonResult::Exception(CorbaException::system(
                SystemExceptionKind::NoImplement,
                0,
                CompletionStatus::No,
            )),
        }
    }
}

/// `MGM::ObjectGroupFactory` reached over IIOP (§11.3.3.1.1): the
/// operations of [`Mgm`] as GIOP requests to the MGM's reference, its
/// exceptions as [`MgmError`].
#[derive(Clone)]
pub struct MgmProxy {
    target: ObjectReference,
    connection: Arc<dyn CorbaConnection + Send + Sync>,
}

impl core::fmt::Debug for MgmProxy {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MgmProxy")
            .field("target", &self.target)
            .finish_non_exhaustive()
    }
}

impl MgmProxy {
    /// The MGM at `target`, called through `connection`.
    #[must_use]
    pub fn new(
        target: ObjectReference,
        connection: Arc<dyn CorbaConnection + Send + Sync>,
    ) -> Self {
        Self { target, connection }
    }

    fn call(
        &self,
        operation: &str,
        args: impl Fn(&mut Writer) -> Result<(), Codec>,
    ) -> Result<(Vec<u8>, Endianness), MgmError> {
        self.connection
            .invoke(&self.target, operation, Endianness::Big, &|w| {
                args(w).map_err(|Codec| marshal())
            })
            .map_err(decode_exception)
    }

    fn read<T>(
        reply: &(Vec<u8>, Endianness),
        f: impl FnOnce(&mut Reader<'_>) -> Result<T, Codec>,
    ) -> Result<T, MgmError> {
        f(&mut Reader::new(&reply.0, reply.1)).map_err(|Codec| MgmError::System(reply_marshal()))
    }

    /// `set_default_properties`.
    ///
    /// # Errors
    /// As [`Mgm::set_default_properties`], or the system exception of the
    /// call.
    pub fn set_default_properties(&self, props: &[Property]) -> Result<(), MgmError> {
        self.call("set_default_properties", |w| write_properties(w, props))
            .map(drop)
    }

    /// `get_default_properties`.
    ///
    /// # Errors
    /// The system exception of the call.
    pub fn get_default_properties(&self) -> Result<Properties, MgmError> {
        let reply = self.call("get_default_properties", |_| Ok(()))?;
        Self::read(&reply, read_properties)
    }

    /// `remove_default_properties`.
    ///
    /// # Errors
    /// As [`Mgm::remove_default_properties`], or the system exception of
    /// the call.
    pub fn remove_default_properties(&self, props: &[Property]) -> Result<(), MgmError> {
        self.call("remove_default_properties", |w| write_properties(w, props))
            .map(drop)
    }

    /// `set_type_properties`.
    ///
    /// # Errors
    /// As [`Mgm::set_type_properties`], or the system exception of the
    /// call.
    pub fn set_type_properties(
        &self,
        type_id: &str,
        overrides: &[Property],
    ) -> Result<(), MgmError> {
        self.call("set_type_properties", |w| {
            w.write_string(type_id);
            write_properties(w, overrides)
        })
        .map(drop)
    }

    /// `get_type_properties`.
    ///
    /// # Errors
    /// The system exception of the call.
    pub fn get_type_properties(&self, type_id: &str) -> Result<Properties, MgmError> {
        let reply = self.call("get_type_properties", |w| {
            w.write_string(type_id);
            Ok(())
        })?;
        Self::read(&reply, read_properties)
    }

    /// `remove_type_properties`.
    ///
    /// # Errors
    /// As [`Mgm::remove_type_properties`], or the system exception of the
    /// call.
    pub fn remove_type_properties(
        &self,
        type_id: &str,
        props: &[Property],
    ) -> Result<(), MgmError> {
        self.call("remove_type_properties", |w| {
            w.write_string(type_id);
            write_properties(w, props)
        })
        .map(drop)
    }

    /// `set_properties_dynamically`.
    ///
    /// # Errors
    /// As [`Mgm::set_properties_dynamically`], or the system exception of
    /// the call.
    pub fn set_properties_dynamically(
        &self,
        object_group: &Ior,
        overrides: &[Property],
    ) -> Result<(), MgmError> {
        self.call("set_properties_dynamically", |w| {
            object_group.encode(w)?;
            write_properties(w, overrides)
        })
        .map(drop)
    }

    /// `get_properties`.
    ///
    /// # Errors
    /// `ObjectGroupNotFound`, or the system exception of the call.
    pub fn get_properties(&self, object_group: &Ior) -> Result<Properties, MgmError> {
        let reply = self.call("get_properties", |w| Ok(object_group.encode(w)?))?;
        Self::read(&reply, read_properties)
    }

    /// `get_object_group_id`.
    ///
    /// # Errors
    /// `ObjectGroupNotFound`, or the system exception of the call.
    pub fn get_object_group_id(&self, object_group: &Ior) -> Result<u64, MgmError> {
        let reply = self.call("get_object_group_id", |w| Ok(object_group.encode(w)?))?;
        Self::read(&reply, |r| Ok(r.read_u64()?))
    }

    /// `get_object_group_ref`.
    ///
    /// # Errors
    /// `ObjectGroupNotFound`, or the system exception of the call.
    pub fn get_object_group_ref(&self, object_group: &Ior) -> Result<Ior, MgmError> {
        let reply = self.call("get_object_group_ref", |w| Ok(object_group.encode(w)?))?;
        Self::read(&reply, |r| Ok(Ior::decode(r)?))
    }

    /// `create_object(type_id, the_criteria, out factory_creation_id)`.
    ///
    /// # Errors
    /// As [`Mgm::create_object`], or the system exception of the call.
    pub fn create_object(
        &self,
        type_id: &str,
        the_criteria: &[Property],
    ) -> Result<(Ior, CorbaAny), MgmError> {
        let reply = self.call("create_object", |w| {
            w.write_string(type_id);
            write_properties(w, the_criteria)
        })?;
        Self::read(&reply, |r| Ok((Ior::decode(r)?, CorbaAny::decode(r)?)))
    }

    /// `delete_object`.
    ///
    /// # Errors
    /// `ObjectNotFound`, or the system exception of the call.
    pub fn delete_object(&self, factory_creation_id: &CorbaAny) -> Result<(), MgmError> {
        self.call("delete_object", |w| Ok(factory_creation_id.encode(w)?))
            .map(drop)
    }
}
