// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A data-arrival notice for a runtime reader that leaves the samples where
//! they are.
//!
//! [`zerodds_reader_set_data_callback`](crate::zerodds_reader_set_data_callback)
//! hands the payload to the callback and takes it out of the reader, and it
//! forbids ZeroDDS calls inside. DDS-RPC 1.0 §7.9.2.2 and §7.11.1.4.7-.10
//! have a listener answer a request: take it, run the service, write the
//! reply. That needs a signal and nothing else; the reader's own `take`
//! stays the way to read.

use alloc::sync::Arc;
use core::ffi::{c_int, c_void};

use crate::{ZeroDdsReader, ZeroDdsStatus};

/// What the arrival hook owns: the caller's pointer and the function that
/// releases it. The hook is cloned out of the reader for each call, so a
/// call that is running when the callback is cleared still has its
/// `user_data`; the release runs when the last clone is gone.
struct Release {
    user_data: usize,
    release: Option<extern "C" fn(*mut c_void)>,
}

impl Drop for Release {
    fn drop(&mut self) {
        if let Some(release) = self.release {
            release(self.user_data as *mut c_void);
        }
    }
}

/// Calls `callback(user_data)` on the receive thread once after each run of
/// samples the reader has queued, or removes the callback with `NULL`.
///
/// * The samples stay in the reader; read them with `zerodds_reader_take*`.
/// * The callback runs on the runtime's receive thread and must be short and
///   must not call ZeroDDS: signal a thread of your own and return. It must
///   not let a C++ exception escape.
/// * Samples that arrived before the callback was set do not call it; take
///   once after setting it.
/// * With `callback == NULL`, `user_data` and `release` are ignored.
/// * `release` (nullable) is called with `user_data` when the callback has
///   been replaced or removed and no call of it is running any more, so the
///   caller can free what `user_data` points to without waiting for the
///   receive thread.
///
/// # Safety
/// `reader` from `zerodds_reader_create*`; `user_data` stays valid until
/// `release` runs (until the callback is cleared, when `release` is NULL).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_set_arrival_callback(
    reader: *mut ZeroDdsReader,
    callback: Option<extern "C" fn(*mut c_void)>,
    user_data: *mut c_void,
    release: Option<extern "C" fn(*mut c_void)>,
) -> c_int {
    if reader.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: reader NULL-checked above.
    let r = unsafe { &*reader };
    let signal: Option<zerodds_dcps::runtime::UserReaderArrivalHook> = match callback {
        Some(callback) => {
            let owned = Release {
                user_data: user_data as usize,
                release,
            };
            // The closure must own the whole `Release`, not only its
            // `user_data` field (edition 2021 captures disjoint fields), or
            // `release` would run as soon as this arm ends.
            Some(Arc::new(move || {
                let owned = &owned;
                callback(owned.user_data as *mut c_void);
            }))
        }
        // Removing the callback: the hook that was set drops, and releases
        // its own `user_data` when its last call is over.
        None => None,
    };
    if r.rt.set_user_reader_data_signal(r.eid, signal) {
        ZeroDdsStatus::Ok as c_int
    } else {
        ZeroDdsStatus::BadHandle as c_int
    }
}
