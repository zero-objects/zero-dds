// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The optional members `type_id` (`TypeIdV1`, 0x0069) and `type`
//! (`TypeObjectV1`, 0x0072) of the built-in topic data of XTypes 1.3
//! Annex D, in the parameter lists of the three discovery data types.
//!
//! §7.6.3.2.2: "TypeObjectV1 corresponds to the TypeObject data type
//! specified in Annex B of DDS-XTYPES Version 1.1"; the types are
//! [`zerodds_types::type_object_v1`]. A parameter that does not decode is
//! left out of the data: both members are optional, and the Non-normative
//! note of §7.6.3.2.2 says an endpoint that announces neither is matched on
//! its `type_name`.

use zerodds_types::TypeIdentifier;
use zerodds_types::type_object_v1::convert::to_complete;
use zerodds_types::type_object_v1::{TypeId, TypeObjectV1};

use crate::error::WireError;
use crate::parameter_list::{Parameter, ParameterList, pid};

/// Appends the parameters of the members that are set.
///
/// # Errors
/// [`WireError::ValueOutOfRange`] when the object does not serialize.
pub(crate) fn push(
    params: &mut ParameterList,
    type_id: Option<&TypeId>,
    type_object: Option<&TypeObjectV1>,
) -> Result<(), WireError> {
    if let Some(id) = type_id {
        let bytes = id.to_bytes_le().map_err(|_| WireError::ValueOutOfRange {
            message: "a TypeIdV1 that does not serialize",
        })?;
        params.push(Parameter::new(pid::TYPE_ID_V1, bytes));
    }
    if let Some(object) = type_object {
        let bytes = object
            .to_bytes_le()
            .map_err(|_| WireError::ValueOutOfRange {
                message: "a TypeObjectV1 that does not serialize (a name past its bound)",
            })?;
        params.push(Parameter::new(pid::TYPE_OBJECT_V1, bytes));
    }
    Ok(())
}

/// Reads the two members from a parameter list in the given byte order.
pub(crate) fn read(
    pl: &ParameterList,
    little_endian: bool,
) -> (Option<TypeId>, Option<TypeObjectV1>) {
    let type_id = pl
        .find(pid::TYPE_ID_V1)
        .and_then(|p| TypeId::from_bytes(&p.value, !little_endian).ok());
    let type_object = pl
        .find(pid::TYPE_OBJECT_V1)
        .and_then(|p| TypeObjectV1::from_bytes(&p.value, !little_endian).ok());
    (type_id, type_object)
}

/// The identifier a `TypeObjectV1` gives its type as a Complete TypeObject,
/// the identity an endpoint without `PID_TYPE_INFORMATION` is matched by.
///
/// `None` when the object has no type, or when it does not convert (see
/// [`zerodds_types::type_object_v1::convert`]).
#[must_use]
pub fn identifier_of(object: &TypeObjectV1) -> Option<TypeIdentifier> {
    to_complete(object).ok()?.roots.into_iter().next()
}
