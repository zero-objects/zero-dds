// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! POA errors — Spec §11.3.7.

use alloc::string::String;

/// Result alias.
pub type PoaResult<T> = Result<T, PoaError>;

/// POA error. Spec §11.3.7 lists the normative exceptions; we
/// model them as a Rust enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoaError {
    /// `AdapterAlreadyExists` — a POA with that name already exists.
    AdapterAlreadyExists(String),
    /// `AdapterNonExistent` — `find_POA` with `activate_it=false`
    /// and the POA does not exist.
    AdapterNonExistent(String),
    /// `AdapterInactive` — POAManager is in the INACTIVE state.
    AdapterInactive,
    /// `InvalidPolicy` — incompatible policy combination.
    InvalidPolicy(String),
    /// `WrongPolicy` — operation does not match the current policies
    /// (e.g. `activate_object` with USER_ID).
    WrongPolicy(String),
    /// `ObjectNotActive` — no mapping in the AOM.
    ObjectNotActive,
    /// `ObjectAlreadyActive` — the ObjectId is already in the AOM.
    ObjectAlreadyActive,
    /// `ServantAlreadyActive` — the servant is already in the AOM
    /// under a different ObjectId (UNIQUE_ID violation).
    ServantAlreadyActive,
    /// `ServantNotActive` — `servant_to_id` without an active mapping.
    ServantNotActive,
    /// `BadInvocationOrder` — e.g. a POAManager operation in an
    /// inconsistent state.
    BadInvocationOrder(String),
    /// `NoServant` — default servant not set.
    NoServant,
    /// `OBJ_ADAPTER` system exception (transport/adapter level).
    ObjAdapter(String),
}
