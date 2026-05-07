// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! POA-Fehler — Spec §11.3.7.

use alloc::string::String;

/// Result-Alias.
pub type PoaResult<T> = Result<T, PoaError>;

/// POA-Fehler. Spec §11.3.7 listet die normativen Exceptions; wir
/// modellieren sie als Rust-Enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoaError {
    /// `AdapterAlreadyExists` — POA mit dem Namen existiert schon.
    AdapterAlreadyExists(String),
    /// `AdapterNonExistent` — `find_POA` mit `activate_it=false`
    /// und der POA existiert nicht.
    AdapterNonExistent(String),
    /// `AdapterInactive` — POAManager ist im INACTIVE-State.
    AdapterInactive,
    /// `InvalidPolicy` — inkompatible Policy-Kombination.
    InvalidPolicy(String),
    /// `WrongPolicy` — Operation passt nicht zu den aktuellen Policies
    /// (z.B. `activate_object` mit USER_ID).
    WrongPolicy(String),
    /// `ObjectNotActive` — kein Mapping in der AOM.
    ObjectNotActive,
    /// `ObjectAlreadyActive` — Object-Id ist schon in der AOM.
    ObjectAlreadyActive,
    /// `ServantAlreadyActive` — Servant ist mit anderer Object-Id
    /// schon in der AOM (UNIQUE_ID-Verletzung).
    ServantAlreadyActive,
    /// `ServantNotActive` — `servant_to_id` ohne aktive Mapping.
    ServantNotActive,
    /// `BadInvocationOrder` — z.B. POAManager-Operation in
    /// inkonsistentem State.
    BadInvocationOrder(String),
    /// `NoServant` — Default-Servant nicht gesetzt.
    NoServant,
    /// `OBJ_ADAPTER` System-Exception (transport-/adapter-Level).
    ObjAdapter(String),
}
