// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The incompatible-QoS statuses with their per-policy counts (DDS 1.4
//! §2.2.4.1, DDS-PSM-Cxx 1.0 §7.5.4: `QosPolicyCountSeq policies`). The
//! plain `zerodds_*_get_*_incompatible_qos_status` carry the last policy id
//! only; reading either resets the change field, so the counts come with the
//! status in one call.

use core::ffi::c_int;

use zerodds_dcps::status::QosPolicyCount;

use crate::ZeroDdsStatus;
use crate::entities::{ZeroDdsDataReader, ZeroDdsDataWriter};

/// `QosPolicyCount` (DDS 1.4 §2.2.4.1).
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsQosPolicyCount {
    /// The policy's id.
    pub policy_id: u32,
    /// How often the policy was incompatible.
    pub count: i32,
}

/// An incompatible-QoS status with its `policies`.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsIncompatibleQosStatusEx {
    /// Total cumulative count.
    pub total_count: i32,
    /// Change since the last read.
    pub total_count_change: i32,
    /// The policy that was incompatible last.
    pub last_policy_id: u32,
    /// How many entries of `policies` are set.
    pub policies_len: u32,
    /// Per-policy counts: the policy ids of DDS 1.4 and DDS-XTypes 1.3 are 1
    /// to 24, so 32 entries hold them all.
    pub policies: [ZeroDdsQosPolicyCount; 32],
}

fn fill(
    total_count: i32,
    total_count_change: i32,
    last_policy_id: u32,
    policies: &[QosPolicyCount],
) -> ZeroDdsIncompatibleQosStatusEx {
    let mut out = ZeroDdsIncompatibleQosStatusEx {
        total_count,
        total_count_change,
        last_policy_id,
        ..Default::default()
    };
    for (slot, p) in out.policies.iter_mut().zip(policies) {
        *slot = ZeroDdsQosPolicyCount {
            policy_id: p.policy_id,
            count: p.count,
        };
        out.policies_len += 1;
    }
    out
}

/// `get_offered_incompatible_qos_status` of a DataWriter with the
/// per-policy counts (DDS 1.4 §2.2.2.4.2). Reading resets the change field.
///
/// # Safety
/// `dw` is a live DataWriter handle, `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_offered_incompatible_qos_status_ex(
    dw: *mut ZeroDdsDataWriter,
    out: *mut ZeroDdsIncompatibleQosStatusEx,
) -> c_int {
    // SAFETY: a non-null `dw` is a live handle; a disabled entity's status
    // is untriggered (DDS 1.4 §2.2.2.1.1.7).
    if unsafe { crate::entity_ffi::untriggered_if_disabled(dw, out) } {
        return ZeroDdsStatus::Ok as c_int;
    }
    if dw.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: non-null, live per the caller's contract.
    unsafe {
        let dwr = &*dw;
        let st = dwr
            .rt
            .take_user_writer_offered_incompatible_qos_status(dwr.eid)
            .unwrap_or_default();
        *out = fill(
            st.total_count,
            st.total_count_change,
            st.last_policy_id,
            &st.policies,
        );
    }
    ZeroDdsStatus::Ok as c_int
}

/// `get_requested_incompatible_qos_status` of a DataReader with the
/// per-policy counts (DDS 1.4 §2.2.2.5.3). Reading resets the change field.
///
/// # Safety
/// `dr` is a live DataReader handle, `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_requested_incompatible_qos_status_ex(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsIncompatibleQosStatusEx,
) -> c_int {
    // SAFETY: as above.
    if unsafe { crate::entity_ffi::untriggered_if_disabled(dr, out) } {
        return ZeroDdsStatus::Ok as c_int;
    }
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: non-null, live per the caller's contract.
    unsafe {
        let drr = &*dr;
        let st = drr
            .rt
            .take_user_reader_requested_incompatible_qos_status(drr.eid)
            .unwrap_or_default();
        *out = fill(
            st.total_count,
            st.total_count_change,
            st.last_policy_id,
            &st.policies,
        );
    }
    ZeroDdsStatus::Ok as c_int
}
