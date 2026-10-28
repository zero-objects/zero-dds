// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `TypeSupport` and `register_type` (DDS 1.4 §2.2.2.3.6, §2.3.3).

use core::marker::PhantomData;

use crate::dds_type::DdsType;
use crate::error::Result;
use crate::participant::DomainParticipant;

/// The TypeSupport of the data type `T` (DDS 1.4 §2.2.2.3.6): "A TypeSupport
/// must be registered using the register_type operation on this type-specific
/// class before it can be used to create Topic objects." `FooTypeSupport` of
/// the IDL PSM (§2.3.3) is `TypeSupport<Foo>`; the key, the encoding and the
/// default type name come from the [`DdsType`] impl.
#[derive(Debug)]
pub struct TypeSupport<T: DdsType> {
    _type: PhantomData<fn() -> T>,
}

impl<T: DdsType> Clone for TypeSupport<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: DdsType> Copy for TypeSupport<T> {}

impl<T: DdsType> Default for TypeSupport<T> {
    fn default() -> Self {
        Self { _type: PhantomData }
    }
}

impl<T: DdsType + 'static> TypeSupport<T> {
    /// The TypeSupport of `T`.
    #[must_use]
    pub const fn new() -> Self {
        Self { _type: PhantomData }
    }

    /// DDS 1.4 §2.2.2.3.6.2 `get_type_name`: "the default name for the
    /// data-type represented by the TypeSupport".
    #[must_use]
    pub fn get_type_name(&self) -> &'static str {
        T::type_name()
    }

    /// DDS 1.4 §2.2.2.3.6.1 `register_type`: makes the type known to the
    /// service under `type_name`; `None` (nil) registers it under its
    /// default name ([`Self::get_type_name`]). Registering the same
    /// TypeSupport again, under the same or another name, is ignored and
    /// returns OK.
    ///
    /// # Errors
    /// `PreconditionNotMet` if `type_name` is registered with this
    /// participant for a different TypeSupport; `BadParameter` for an empty
    /// name or one longer than 256 octets; `AlreadyDeleted` for a deleted
    /// participant.
    pub fn register_type(
        &self,
        participant: &DomainParticipant,
        type_name: Option<&str>,
    ) -> Result<()> {
        participant.register_type_support(
            type_name.unwrap_or_else(|| T::type_name()),
            core::any::TypeId::of::<T>(),
        )
    }
}
