// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `DdsType::encode_key_holder_be` for a `derive(DdsType)` type, apart
//! from the rest of the crate: it panics on a key the holder refuses,
//! because the trait method returns `()` and cannot report one
//! (OPEN-ITEMS 35, the IDL route of `zerodds-build` does the same). Once
//! the method returns a `Result`, the panic and this marker go.
// zerodds-lint: allow no_panic_in_safe

use zerodds_dcps::dds_type::PlainCdr2BeKeyHolder;

use crate::{CodecError, DdsSerialize};

/// `DdsType::encode_key_holder_be`.
///
/// A type with no model writes nothing: every encode of it fails with
/// the model's reason, so no sample of it is sent.
///
/// # Panics
/// A key the holder refuses — a `@key string<8>` holding nine
/// characters — as the generated impl of `zerodds-build` does: the
/// trait method cannot report (OPEN-ITEMS 35), and an empty holder
/// would give every such instance one KeyHash and merge instances the
/// application keeps apart.
#[allow(clippy::panic)] // see the Panics section; the IDL route does the same
pub fn encode_key_holder_be<T: DdsSerialize>(value: &T, holder: &mut PlainCdr2BeKeyHolder) {
    match crate::key_holder(value) {
        Ok(bytes) => holder.write_bytes(&bytes),
        Err(CodecError::Model(_)) => {}
        Err(error) => panic!(
            "the key holder of `{}` refused this instance's key: {error}",
            T::LAYOUT.scoped_name()
        ),
    }
}
