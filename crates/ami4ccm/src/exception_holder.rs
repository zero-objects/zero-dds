// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `CCM_AMI::ExceptionHolder` Modell — Spec §7.4.1.
//!
//! Spec-IDL (Annex A — `ami4ccm.idl`):
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

/// `CCM_AMI::UserExceptionBase` (Spec §7.4.1, S. 9).
///
/// Spec-Zitat: "Language mapping of this native type should allow any
/// user exception to be raised from this method. For instance, it is
/// mapped to CORBA::UserException in C++ and to org.omg.CORBA.
/// UserException in java."
///
/// In ZeroDDS modellieren wir die `native`-Form als (Repository-ID,
/// marshaled-Bytes)-Paar. Das ist ausreichend, um eine User-Exception
/// von Server-Side serialisiert und im Client wieder rekonstruiert zu
/// erhalten — Sprach-Mapping (CORBA::UserException, java.lang.Throwable,
/// etc.) erfolgt beim Codegen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserExceptionBase {
    /// CORBA Repository-ID der originalen Exception
    /// (z.B. `IDL:Stock/InvalidStock:1.0`).
    pub repository_id: String,
    /// CDR-marshaled Member-State der Exception.
    pub marshaled_exception: Vec<u8>,
}

impl UserExceptionBase {
    /// Konstruiert eine `UserExceptionBase` aus Repository-ID + Bytes.
    #[must_use]
    pub fn new(repository_id: impl Into<String>, marshaled_exception: Vec<u8>) -> Self {
        Self {
            repository_id: repository_id.into(),
            marshaled_exception,
        }
    }
}

/// `CCM_AMI::ExceptionHolder`-Instanz (Spec §7.4.1, S. 9).
///
/// Spec-Zitat: "The CCM_AMI::ExceptionHolder interface encapsulates the
/// exception data and enough information to turn that data back into a
/// raised exception."
///
/// Operation `raise_exception()` (Spec §7.4.1) rekonstruiert die
/// Exception aus dem Holder. In ZeroDDS modellieren wir das als
/// `Result<(), UserExceptionBase>` — `Err(...)` ist der "raise"-Pfad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExceptionHolder {
    inner: UserExceptionBase,
}

impl ExceptionHolder {
    /// Konstruiert `ExceptionHolder` aus `UserExceptionBase`.
    #[must_use]
    pub fn new(exception: UserExceptionBase) -> Self {
        Self { inner: exception }
    }

    /// Spec §7.4.1 — `raise_exception()`. Liefert die enthaltene
    /// Exception als `Err`-Variante. Da Exceptions nicht auf
    /// `core::error::Error`-Trait gemappt werden (das ist Sprach-spezifisch),
    /// ist der Rueckgabe-Typ `Err(UserExceptionBase)`.
    ///
    /// # Errors
    /// Diese Operation gibt **immer** `Err` zurueck — der Holder
    /// existiert genau, um eine Exception zu transportieren. Der `Ok(())`-
    /// Pfad ist nur formal, um den Spec-Signature-Style abzubilden.
    pub fn raise_exception(&self) -> Result<(), UserExceptionBase> {
        Err(self.inner.clone())
    }

    /// Read-Only-Zugriff auf die enthaltene Exception (z.B. fuer
    /// Diagnostik, ohne sie zu "raisen").
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
