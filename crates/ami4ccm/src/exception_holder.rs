// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `CCM_AMI::ExceptionHolder` model — spec §7.4.1.
//!
//! Spec IDL (Annex A — `ami4ccm.idl`):
//! ```idl
//! module CCM_AMI {
//!     native UserExceptionBase;
//!     local interface ExceptionHolder {
//!         void raise_exception() raises (UserExceptionBase);
//!     };
//! };
//! ```

use alloc::string::String;
use alloc::vec::Vec;

/// `CCM_AMI::UserExceptionBase` (spec §7.4.1, p. 9).
///
/// Spec quote: "Language mapping of this native type should allow any
/// user exception to be raised from this method. For instance, it is
/// mapped to CORBA::UserException in C++ and to org.omg.CORBA.
/// UserException in java."
///
/// In ZeroDDS we model the `native` form as a (repository ID,
/// marshaled bytes) pair. This is sufficient to serialize a user
/// exception on the server side and reconstruct it on the client —
/// the language mapping (CORBA::UserException, java.lang.Throwable,
/// etc.) happens at codegen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserExceptionBase {
    /// CORBA repository ID of the original exception
    /// (e.g. `IDL:Stock/InvalidStock:1.0`).
    pub repository_id: String,
    /// CDR-marshaled member state of the exception.
    pub marshaled_exception: Vec<u8>,
}

impl UserExceptionBase {
    /// Constructs a `UserExceptionBase` from a repository ID + bytes.
    #[must_use]
    pub fn new(repository_id: impl Into<String>, marshaled_exception: Vec<u8>) -> Self {
        Self {
            repository_id: repository_id.into(),
            marshaled_exception,
        }
    }
}

/// `CCM_AMI::ExceptionHolder` instance (spec §7.4.1, p. 9).
///
/// Spec quote: "The CCM_AMI::ExceptionHolder interface encapsulates the
/// exception data and enough information to turn that data back into a
/// raised exception."
///
/// The `raise_exception()` operation (spec §7.4.1) reconstructs the
/// exception from the holder. In ZeroDDS we model this as
/// `Result<(), UserExceptionBase>` — `Err(...)` is the "raise" path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExceptionHolder {
    inner: UserExceptionBase,
}

impl ExceptionHolder {
    /// Constructs `ExceptionHolder` from `UserExceptionBase`.
    #[must_use]
    pub fn new(exception: UserExceptionBase) -> Self {
        Self { inner: exception }
    }

    /// Spec §7.4.1 — `raise_exception()`. Returns the contained
    /// exception as the `Err` variant. Since exceptions are not mapped
    /// to the `core::error::Error` trait (that is language-specific),
    /// the return type is `Err(UserExceptionBase)`.
    ///
    /// # Errors
    /// This operation **always** returns `Err` — the holder
    /// exists precisely to transport an exception. The `Ok(())`
    /// path is only formal, to mirror the spec signature style.
    pub fn raise_exception(&self) -> Result<(), UserExceptionBase> {
        Err(self.inner.clone())
    }

    /// Read-only access to the contained exception (e.g. for
    /// diagnostics, without "raising" it).
    #[must_use]
    pub const fn user_exception(&self) -> &UserExceptionBase {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_exception_base_new_stores_id_and_bytes() {
        let ub = UserExceptionBase::new("IDL:Stock/InvalidStock:1.0", alloc::vec![0xDE, 0xAD]);
        assert_eq!(ub.repository_id, "IDL:Stock/InvalidStock:1.0");
        assert_eq!(ub.marshaled_exception, alloc::vec![0xDE, 0xAD]);
    }

    #[test]
    fn raise_exception_returns_err_with_user_exception() {
        // Spec §7.4.1 — raise_exception() raises UserExceptionBase.
        let ub = UserExceptionBase::new("IDL:Foo:1.0", alloc::vec![1, 2, 3]);
        let holder = ExceptionHolder::new(ub.clone());
        let result = holder.raise_exception();
        assert_eq!(result, Err(ub));
    }

    #[test]
    fn user_exception_accessor_returns_inner() {
        let ub = UserExceptionBase::new("IDL:Bar:1.0", alloc::vec![]);
        let holder = ExceptionHolder::new(ub.clone());
        assert_eq!(holder.user_exception(), &ub);
    }
}
