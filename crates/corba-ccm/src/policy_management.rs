// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Client-side policy management — CORBA 3.3 Part 1 §8.9 and the policy
//! operations of `CORBA::Object` (§8.3.8, §8.3.9).
//!
//! Overrides are set at three scopes (§8.9.1):
//! * the ORB: [`PolicyManager`], the ORB's initial reference
//!   `"ORBPolicyManager"`;
//! * the thread: [`PolicyCurrent`], the initial reference
//!   `"PolicyCurrent"`; a thread starts without overrides;
//! * the object reference: [`set_object_overrides`] returns a copy of a
//!   reference with overrides (`Object::set_policy_overrides`), kept in
//!   [`ObjectReference::policy_overrides`].
//!
//! The effective override of a policy type is the one of the reference,
//! else of the calling thread, else of the ORB ([`client_policy`],
//! `Object::get_client_policy`, §8.3.8.2).
//!
//! The policies an application can override are those the ORB applies to
//! an invocation at the client end (§8.9.3.2): the four ZIOP policies of
//! Part 2 §12.3, which Part 2 §12.2 lets a client set "at ORB, thread or
//! reference level". Any other policy type raises `NO_PERMISSION`.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use zerodds_corba_giop::ziop as z;
use zerodds_corba_rt::{
    CompletionStatus, CorbaAny, ObjectReference, PolicyOverride, SystemException,
    SystemExceptionKind,
};
use zerodds_wire::{Endianness, Reader, Writer};

use crate::orb_core::ziop_policy;
use crate::portable_interceptor::Policy;

/// The `ORB::resolve_initial_references` identifier of the ORB's
/// `PolicyManager` (§8.9.1).
pub const ORB_POLICY_MANAGER_INITIAL_REFERENCE: &str = "ORBPolicyManager";

/// The `ORB::resolve_initial_references` identifier of `PolicyCurrent`
/// (§8.9.1).
pub const POLICY_CURRENT_INITIAL_REFERENCE: &str = "PolicyCurrent";

/// `CORBA::SetOverrideType` (§8.3.9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetOverrideType {
    /// `SET_OVERRIDE`: the policies replace every override of the scope.
    SetOverride,
    /// `ADD_OVERRIDE`: the policies are added; one of a type already
    /// overridden replaces that override.
    AddOverride,
}

/// What `set_policy_overrides` raises (§8.3.9.1, §8.9.3.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverrideError {
    /// `CORBA::InvalidPolicies`: the indices in the list of the policies
    /// whose value is not valid for their type.
    InvalidPolicies(Vec<u16>),
    /// `BAD_PARAM` minor 30 for two policies of one type in the list,
    /// `NO_PERMISSION` for a policy that cannot be overridden at the client
    /// end, `NO_IMPLEMENT` minor 8 on a local object (§8.3.14).
    System(SystemException),
}

impl From<SystemException> for OverrideError {
    fn from(e: SystemException) -> Self {
        Self::System(e)
    }
}

/// `true` for the policy types an application can override at the
/// client end: the ZIOP policies (Part 2 §12.2, §12.3).
#[must_use]
pub fn is_client_override(policy_type: u32) -> bool {
    matches!(
        policy_type,
        z::COMPRESSION_ENABLING_POLICY_ID
            | z::COMPRESSOR_ID_LEVEL_LIST_POLICY_ID
            | z::COMPRESSION_LOW_VALUE_POLICY_ID
            | z::COMPRESSION_MIN_RATIO_POLICY_ID
    )
}

/// Checks a list for `set_policy_overrides`: `BAD_PARAM` minor 30 when a
/// policy type is in it twice, `NO_PERMISSION` for a policy that cannot
/// be overridden, `InvalidPolicies` with the indices of the policies
/// whose value is not of their type. Nothing is changed when the list is
/// refused.
///
/// # Errors
/// As described.
pub fn check_overrides(policies: &[Policy]) -> Result<(), OverrideError> {
    for (i, p) in policies.iter().enumerate() {
        if policies[..i].iter().any(|q| q.policy_type == p.policy_type) {
            return Err(OverrideError::System(SystemException::omg(
                SystemExceptionKind::BadParam,
                30,
                CompletionStatus::No,
            )));
        }
    }
    if policies.iter().any(|p| !is_client_override(p.policy_type)) {
        return Err(OverrideError::System(SystemException::new(
            SystemExceptionKind::NoPermission,
            0,
            CompletionStatus::No,
        )));
    }
    let invalid: Vec<u16> = policies
        .iter()
        .enumerate()
        .filter(|(_, p)| !matches!(ziop_policy(p.policy_type, &p.value), Some(Ok(_))))
        .map(|(i, _)| u16::try_from(i).unwrap_or(u16::MAX))
        .collect();
    if invalid.is_empty() {
        Ok(())
    } else {
        Err(OverrideError::InvalidPolicies(invalid))
    }
}

/// `policies` applied to the overrides `current` of a scope by
/// `set_add`.
fn apply(current: &mut Vec<Policy>, policies: &[Policy], set_add: SetOverrideType) {
    if set_add == SetOverrideType::SetOverride {
        current.clear();
    }
    for p in policies {
        match current.iter_mut().find(|c| c.policy_type == p.policy_type) {
            Some(slot) => *slot = p.clone(),
            None => current.push(p.clone()),
        }
    }
}

/// The overrides of `types` among `overrides`; all of them for an empty
/// `types` (§8.9.3.1 `get_policy_overrides`).
fn select(overrides: &[Policy], types: &[u32]) -> Vec<Policy> {
    overrides
        .iter()
        .filter(|p| types.is_empty() || types.contains(&p.policy_type))
        .cloned()
        .collect()
}

/// `CORBA::PolicyManager` (§8.9.3.1): the overrides of one scope. The
/// ORB's own is its initial reference `"ORBPolicyManager"`.
#[derive(Debug, Default)]
pub struct PolicyManager {
    overrides: Mutex<Vec<Policy>>,
}

impl PolicyManager {
    /// A manager without overrides.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `get_policy_overrides(ts)`: the overrides of the policy types `ts`
    /// set at this scope, every override for an empty `ts`.
    #[must_use]
    pub fn get_policy_overrides(&self, ts: &[u32]) -> Vec<Policy> {
        select(
            &self
                .overrides
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            ts,
        )
    }

    /// `set_policy_overrides(policies, set_add)` (§8.9.3.2).
    ///
    /// # Errors
    /// As [`check_overrides`]; nothing changes then.
    pub fn set_policy_overrides(
        &self,
        policies: &[Policy],
        set_add: SetOverrideType,
    ) -> Result<(), OverrideError> {
        check_overrides(policies)?;
        apply(
            &mut self
                .overrides
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            policies,
            set_add,
        );
        Ok(())
    }
}

static NEXT_CURRENT: AtomicUsize = AtomicUsize::new(1);

std::thread_local! {
    /// The overrides of each `PolicyCurrent` on this thread.
    static THREAD_OVERRIDES: RefCell<BTreeMap<usize, Vec<Policy>>> =
        const { RefCell::new(BTreeMap::new()) };
}

/// `CORBA::PolicyCurrent` (§8.9.3.3): the overrides of the calling
/// thread. One object is shared by every thread; a thread starts without
/// overrides (§8.9.1).
#[derive(Debug)]
pub struct PolicyCurrent {
    id: usize,
}

impl Default for PolicyCurrent {
    fn default() -> Self {
        Self::new()
    }
}

impl PolicyCurrent {
    /// A `PolicyCurrent` of its own: two of them never share a thread's
    /// overrides.
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: NEXT_CURRENT.fetch_add(1, Ordering::Relaxed),
        }
    }

    fn thread_overrides(&self) -> Vec<Policy> {
        THREAD_OVERRIDES.with(|m| m.borrow().get(&self.id).cloned().unwrap_or_default())
    }

    fn set_thread_overrides(&self, overrides: Vec<Policy>) {
        THREAD_OVERRIDES.with(|m| {
            let mut m = m.borrow_mut();
            if overrides.is_empty() {
                m.remove(&self.id);
            } else {
                m.insert(self.id, overrides);
            }
        });
    }

    /// `get_policy_overrides(ts)` of the calling thread.
    #[must_use]
    pub fn get_policy_overrides(&self, ts: &[u32]) -> Vec<Policy> {
        select(&self.thread_overrides(), ts)
    }

    /// `set_policy_overrides(policies, set_add)` for the calling thread
    /// (§8.9.3.2).
    ///
    /// # Errors
    /// As [`check_overrides`]; nothing changes then.
    pub fn set_policy_overrides(
        &self,
        policies: &[Policy],
        set_add: SetOverrideType,
    ) -> Result<(), OverrideError> {
        check_overrides(policies)?;
        let mut current = self.thread_overrides();
        apply(&mut current, policies, set_add);
        self.set_thread_overrides(current);
        Ok(())
    }

    /// Runs `f` as the dispatch of a request to a servant: the thread has
    /// no overrides while `f` runs (§8.9.1), and the overrides it had are
    /// restored afterwards, whatever `f` set.
    pub fn dispatch<R>(&self, f: impl FnOnce() -> R) -> R {
        let saved = self.thread_overrides();
        self.set_thread_overrides(Vec::new());
        let r = f();
        self.set_thread_overrides(saved);
        r
    }
}

impl Drop for PolicyCurrent {
    fn drop(&mut self) {
        // The other threads' entries go with their threads.
        let _ = THREAD_OVERRIDES.try_with(|m| m.borrow_mut().remove(&self.id));
    }
}

/// A policy as a [`PolicyOverride`]: its value as an encapsulated `any`.
///
/// # Errors
/// `MARSHAL` when the value does not encode.
pub fn to_override(policy: &Policy) -> Result<PolicyOverride, SystemException> {
    let mut w = Writer::new(Endianness::Big);
    w.write_u8(0);
    policy
        .value
        .encode(&mut w)
        .map_err(|_| SystemException::new(SystemExceptionKind::Marshal, 0, CompletionStatus::No))?;
    Ok(PolicyOverride {
        policy_type: policy.policy_type,
        value: w.finish(),
    })
}

/// The policy of a [`PolicyOverride`].
///
/// # Errors
/// `MARSHAL` when the value does not decode.
pub fn from_override(o: &PolicyOverride) -> Result<Policy, SystemException> {
    let marshal = || SystemException::new(SystemExceptionKind::Marshal, 0, CompletionStatus::No);
    let endianness = match o.value.first() {
        Some(0) => Endianness::Big,
        Some(1) => Endianness::Little,
        _ => return Err(marshal()),
    };
    let mut r = Reader::new(&o.value, endianness);
    r.read_u8().map_err(|_| marshal())?;
    let value = CorbaAny::decode(&mut r).map_err(|_| marshal())?;
    Ok(Policy {
        policy_type: o.policy_type,
        value,
    })
}

/// `Object::set_policy_overrides(policies, set_add)` (§8.3.9.1): a copy
/// of `reference` whose object-scope overrides are `policies` applied by
/// `set_add`; `reference` itself keeps its overrides.
///
/// # Errors
/// As [`check_overrides`]; `MARSHAL` for a value that does not encode or
/// an override of `reference` that does not decode.
pub fn set_object_overrides(
    reference: &ObjectReference,
    policies: &[Policy],
    set_add: SetOverrideType,
) -> Result<ObjectReference, OverrideError> {
    check_overrides(policies)?;
    let mut current = object_overrides(reference)?;
    apply(&mut current, policies, set_add);
    let mut out = reference.clone();
    out.policy_overrides = current.iter().map(to_override).collect::<Result<_, _>>()?;
    Ok(out)
}

/// The object-scope overrides of `reference`.
///
/// # Errors
/// `MARSHAL` for an override that does not decode.
pub fn object_overrides(reference: &ObjectReference) -> Result<Vec<Policy>, SystemException> {
    reference
        .policy_overrides
        .iter()
        .map(from_override)
        .collect()
}

/// `Object::get_policy_overrides(types)` (§8.3.8.3): the object-scope
/// overrides of `reference` of the policy types `ts`, all of them for an
/// empty `ts`.
///
/// # Errors
/// `MARSHAL` for an override that does not decode.
pub fn get_object_overrides(
    reference: &ObjectReference,
    ts: &[u32],
) -> Result<Vec<Policy>, SystemException> {
    Ok(select(&object_overrides(reference)?, ts))
}

/// `Object::get_client_policy(type)` (§8.3.8.2): the override of
/// `policy_type` at the object scope of `reference`, else at the thread
/// scope of `current`, else at the ORB scope of `orb` (a scope that is
/// `None` has no overrides); `None` (no default) when no scope overrides
/// it.
///
/// # Errors
/// `MARSHAL` for an override of `reference` that does not decode.
pub fn client_policy(
    reference: &ObjectReference,
    current: Option<&PolicyCurrent>,
    orb: Option<&PolicyManager>,
    policy_type: u32,
) -> Result<Option<Policy>, SystemException> {
    if let Some(o) = reference
        .policy_overrides
        .iter()
        .find(|o| o.policy_type == policy_type)
    {
        return from_override(o).map(Some);
    }
    let thread = current.and_then(|c| c.get_policy_overrides(&[policy_type]).into_iter().next());
    Ok(thread
        .or_else(|| orb.and_then(|m| m.get_policy_overrides(&[policy_type]).into_iter().next())))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_corba_rt::AnyValue;

    fn enabling(on: bool) -> Policy {
        Policy {
            policy_type: z::COMPRESSION_ENABLING_POLICY_ID,
            value: CorbaAny(AnyValue::Boolean(on)),
        }
    }

    fn low_value(n: u32) -> Policy {
        Policy {
            policy_type: z::COMPRESSION_LOW_VALUE_POLICY_ID,
            value: CorbaAny(AnyValue::ULong(n)),
        }
    }

    #[test]
    fn corba_p1_8_9_3_set_and_add_override_at_a_scope() {
        let m = PolicyManager::new();
        m.set_policy_overrides(
            &[enabling(true), low_value(10)],
            SetOverrideType::SetOverride,
        )
        .unwrap();
        assert_eq!(m.get_policy_overrides(&[]).len(), 2);
        assert_eq!(
            m.get_policy_overrides(&[z::COMPRESSION_LOW_VALUE_POLICY_ID]),
            vec![low_value(10)]
        );
        assert!(
            m.get_policy_overrides(&[z::COMPRESSION_MIN_RATIO_POLICY_ID])
                .is_empty()
        );
        // ADD_OVERRIDE replaces the override of the same type and keeps
        // the others.
        m.set_policy_overrides(&[low_value(20)], SetOverrideType::AddOverride)
            .unwrap();
        assert_eq!(
            m.get_policy_overrides(&[]),
            vec![enabling(true), low_value(20)]
        );
        // SET_OVERRIDE replaces them all; the empty list removes all.
        m.set_policy_overrides(&[enabling(false)], SetOverrideType::SetOverride)
            .unwrap();
        assert_eq!(m.get_policy_overrides(&[]), vec![enabling(false)]);
        m.set_policy_overrides(&[], SetOverrideType::SetOverride)
            .unwrap();
        assert!(m.get_policy_overrides(&[]).is_empty());
    }

    #[test]
    fn corba_p1_8_9_3_2_refused_lists_change_nothing() {
        let m = PolicyManager::new();
        m.set_policy_overrides(&[low_value(1)], SetOverrideType::SetOverride)
            .unwrap();
        let bad_param_30 = OverrideError::System(SystemException::omg(
            SystemExceptionKind::BadParam,
            30,
            CompletionStatus::No,
        ));
        assert_eq!(
            m.set_policy_overrides(&[low_value(2), low_value(3)], SetOverrideType::AddOverride),
            Err(bad_param_30)
        );
        // A policy the client end does not apply: NO_PERMISSION.
        let poa_thread = Policy {
            policy_type: 16,
            value: CorbaAny(AnyValue::Long(0)),
        };
        assert!(matches!(
            m.set_policy_overrides(&[low_value(4), poa_thread], SetOverrideType::AddOverride),
            Err(OverrideError::System(e)) if e.kind == SystemExceptionKind::NoPermission
        ));
        // A ZIOP policy of the wrong value type: its index.
        let wrong = Policy {
            policy_type: z::COMPRESSION_ENABLING_POLICY_ID,
            value: CorbaAny(AnyValue::Long(1)),
        };
        assert_eq!(
            m.set_policy_overrides(&[low_value(5), wrong], SetOverrideType::SetOverride),
            Err(OverrideError::InvalidPolicies(vec![1]))
        );
        assert_eq!(m.get_policy_overrides(&[]), vec![low_value(1)]);
    }

    #[test]
    fn corba_p1_8_9_1_policy_current_is_per_thread_and_starts_empty() {
        let current = std::sync::Arc::new(PolicyCurrent::new());
        current
            .set_policy_overrides(&[enabling(true)], SetOverrideType::SetOverride)
            .unwrap();
        let other = std::sync::Arc::clone(&current);
        let seen = std::thread::spawn(move || {
            let before = other.get_policy_overrides(&[]);
            other
                .set_policy_overrides(&[low_value(7)], SetOverrideType::SetOverride)
                .unwrap();
            before
        })
        .join()
        .unwrap();
        assert!(seen.is_empty());
        assert_eq!(current.get_policy_overrides(&[]), vec![enabling(true)]);
        // Another PolicyCurrent does not see this thread's overrides.
        assert!(PolicyCurrent::new().get_policy_overrides(&[]).is_empty());
        // A dispatched request starts without overrides; what the servant
        // sets does not stay.
        let inside = current.dispatch(|| {
            let seen = current.get_policy_overrides(&[]);
            current
                .set_policy_overrides(&[low_value(9)], SetOverrideType::AddOverride)
                .unwrap();
            seen
        });
        assert!(inside.is_empty());
        assert_eq!(current.get_policy_overrides(&[]), vec![enabling(true)]);
    }

    #[test]
    fn corba_p1_8_3_9_object_overrides_make_a_new_reference() {
        let original = ObjectReference {
            type_id: "IDL:Echo:1.0".into(),
            iiop_profile: alloc::vec![0, 1, 2],
            policy_overrides: Vec::new(),
            profiles: Vec::new(),
        };
        let with = set_object_overrides(
            &original,
            &[enabling(true), low_value(3)],
            SetOverrideType::SetOverride,
        )
        .unwrap();
        assert!(original.policy_overrides.is_empty());
        assert_eq!(
            get_object_overrides(&with, &[]).unwrap(),
            vec![enabling(true), low_value(3)]
        );
        let added =
            set_object_overrides(&with, &[low_value(4)], SetOverrideType::AddOverride).unwrap();
        assert_eq!(
            get_object_overrides(&added, &[z::COMPRESSION_LOW_VALUE_POLICY_ID]).unwrap(),
            vec![low_value(4)]
        );
        let replaced =
            set_object_overrides(&added, &[low_value(5)], SetOverrideType::SetOverride).unwrap();
        assert_eq!(
            get_object_overrides(&replaced, &[]).unwrap(),
            vec![low_value(5)]
        );
        // The overrides are the client's: the IOR on the wire is the same.
        let ior = |r: &ObjectReference| {
            let mut w = Writer::new(Endianness::Big);
            r.encode(&mut w);
            w.finish()
        };
        assert_eq!(ior(&replaced), ior(&original));
        assert!(matches!(
            set_object_overrides(&original, &[enabling(true), enabling(false)], SetOverrideType::SetOverride),
            Err(OverrideError::System(e)) if e.kind == SystemExceptionKind::BadParam && e.minor == SystemException::omg(SystemExceptionKind::BadParam, 30, CompletionStatus::No).minor
        ));
    }

    #[test]
    fn corba_p1_8_3_8_2_the_client_policy_is_the_reference_then_the_thread_then_the_orb() {
        let orb = PolicyManager::new();
        let current = PolicyCurrent::new();
        let plain = ObjectReference::default();
        let t = z::COMPRESSION_LOW_VALUE_POLICY_ID;
        assert_eq!(
            client_policy(&plain, Some(&current), Some(&orb), t).unwrap(),
            None
        );
        orb.set_policy_overrides(&[low_value(1)], SetOverrideType::SetOverride)
            .unwrap();
        assert_eq!(
            client_policy(&plain, Some(&current), Some(&orb), t).unwrap(),
            Some(low_value(1))
        );
        current
            .set_policy_overrides(&[low_value(2)], SetOverrideType::SetOverride)
            .unwrap();
        assert_eq!(
            client_policy(&plain, Some(&current), Some(&orb), t).unwrap(),
            Some(low_value(2))
        );
        let with =
            set_object_overrides(&plain, &[low_value(3)], SetOverrideType::SetOverride).unwrap();
        assert_eq!(
            client_policy(&with, Some(&current), Some(&orb), t).unwrap(),
            Some(low_value(3))
        );
        // Another type is taken from where it is overridden.
        orb.set_policy_overrides(&[enabling(true)], SetOverrideType::AddOverride)
            .unwrap();
        assert_eq!(
            client_policy(
                &with,
                Some(&current),
                Some(&orb),
                z::COMPRESSION_ENABLING_POLICY_ID
            )
            .unwrap(),
            Some(enabling(true))
        );
    }
}
