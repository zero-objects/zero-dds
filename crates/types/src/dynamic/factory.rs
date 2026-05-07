// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DynamicDataFactory (XTypes 1.3 §7.5.7).

use super::data::DynamicData;
use super::error::DynamicError;
use super::type_::DynamicType;

/// XTypes 1.3 §7.5.7 DynamicDataFactory.
///
/// Stateless. `delete_data` ist ein No-Op (Rust-RAII
/// uebernimmt das Cleanup), wird aber als Spec-API exponiert fuer
/// API-Treue.
pub struct DynamicDataFactory;

impl DynamicDataFactory {
    /// Spec §7.5.7.1.1 `create_data(type)`.
    ///
    /// # Errors
    /// `Inconsistent` wenn der Type nicht konsistent ist.
    pub fn create_data(type_: &DynamicType) -> Result<DynamicData, DynamicError> {
        type_.is_consistent()?;
        Ok(DynamicData::new(type_.clone()))
    }

    /// Spec §7.5.7.1.2 `delete_data(data)` — RAII-No-Op.
    pub fn delete_data(_data: DynamicData) {
        // Drop-Impl macht den Rest.
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::dynamic::DynamicTypeBuilderFactory;
    use crate::dynamic::descriptor::{TypeDescriptor, TypeKind};

    #[test]
    fn factory_create_data_returns_empty_instance() {
        let mut b = DynamicTypeBuilderFactory::create_struct("::S");
        b.add_struct_member("x", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
            .unwrap();
        let t = b.build().unwrap();
        let d = DynamicDataFactory::create_data(&t).unwrap();
        assert_eq!(d.item_count(), 0);
        assert_eq!(d.dynamic_type().name(), "::S");
    }

    #[test]
    fn factory_delete_data_is_noop() {
        let t = DynamicTypeBuilderFactory::get_primitive_type(TypeKind::Int32).unwrap();
        let d = DynamicDataFactory::create_data(&t).unwrap();
        DynamicDataFactory::delete_data(d);
    }
}
