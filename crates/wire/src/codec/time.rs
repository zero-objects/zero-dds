// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `core::time::Duration` as the DDS `Duration_t`, and (with the `std`
//! feature) `std::time::SystemTime` as the DDS `Time_t`.
//!
//! DDS 1.4 §2.3.3 declares both in the module `DDS`:
//!
//! ```idl
//! module DDS {
//!     @final struct Duration_t { long sec; unsigned long nanosec; };
//!     @final struct Time_t { long sec; unsigned long nanosec; };
//! };
//! ```
//!
//! A member of either type is that struct on the wire and in the
//! `TypeObject`, so every other vendor reads it as the struct its own
//! `dds_dcps.idl` declares. `@final` because the two are fixed by the
//! specification and carried without a DHEADER in every DCPS built-in
//! topic that holds one.
//!
//! The special values of §2.3.3 map onto Rust's: `DURATION_INFINITE`
//! (`{0x7fffffff, 0x7fffffff}`) is `Duration::MAX`. A `Duration` longer
//! than `0x7fffffff` seconds that is not `Duration::MAX` is refused rather
//! than cut; a `Time_t` before 1970 and `TIME_INVALID` are refused on the
//! way in, because a `SystemTime` cannot say "invalid".

use core::time::Duration;

use super::{
    DdsDeserialize, DdsDeserializer, DdsMember, DdsMemberRead, DdsSerialize, DdsSerializer,
    DdsTypeInfo, Decoded, FieldValue, Final, MemberDesc, Serialized, TypeDesc, ValueDesc,
    ValueDeserializer, ValueSerializer,
};
use crate::WireError;

/// `DURATION_INFINITE_SEC` and `DURATION_INFINITE_NSEC`, DDS 1.4 §2.3.3.
const INFINITE_SEC: i32 = 0x7fff_ffff;
const INFINITE_NSEC: u32 = 0x7fff_ffff;

const NANOS_PER_SECOND: u32 = 1_000_000_000;

impl DdsTypeInfo for Duration {
    type Members = (i32, (u32, ()));
    type Extensibility = Final;
    const LAYOUT: TypeDesc = TypeDesc::structure::<Self::Members, Final>(
        "DDS::Duration_t",
        &[MemberDesc::new("sec"), MemberDesc::new("nanosec")],
    );
}

/// `sec` and `nanosec` of a `Duration_t`.
fn duration_parts(value: &Duration) -> Result<(i32, u32), WireError> {
    if *value == Duration::MAX {
        return Ok((INFINITE_SEC, INFINITE_NSEC));
    }
    let sec = i32::try_from(value.as_secs())
        .ok()
        .filter(|sec| *sec < INFINITE_SEC)
        .ok_or_else(|| {
            WireError::invalid_value(
                "a Duration of fewer than 0x7fffffff seconds, or Duration::MAX for \
                 DURATION_INFINITE (DDS 1.4 §2.3.3)",
            )
        })?;
    Ok((sec, value.subsec_nanos()))
}

/// `sec` or `nanosec` of a `Duration_t` or `Time_t` for a content filter.
fn time_field(parts: Option<(i32, u32)>, path: Option<&str>) -> Option<FieldValue> {
    let (sec, nanosec) = parts?;
    match path? {
        "sec" => Some(FieldValue::Int(i64::from(sec))),
        "nanosec" => Some(FieldValue::Int(i64::from(nanosec))),
        _ => None,
    }
}

impl DdsSerialize for Duration {
    fn serialize(&self, out: DdsSerializer<'_>) -> Result<Serialized, WireError> {
        let (sec, nanosec) = duration_parts(self)?;
        out.serialize_struct::<Self>()?
            .member(&sec)?
            .member(&nanosec)?
            .end()
    }
}

impl<'de> DdsDeserialize<'de> for Duration {
    fn deserialize(input: DdsDeserializer<'_, 'de>) -> Result<Decoded<Self>, WireError> {
        let input = input.deserialize_struct::<Self>()?;
        let (sec, input) = input.member()?;
        let (nanosec, input) = input.member()?;
        let finish = input.end()?;
        let value = if sec == INFINITE_SEC && nanosec == INFINITE_NSEC {
            Duration::MAX
        } else {
            let sec = u64::try_from(sec)
                .map_err(|_| WireError::invalid_value("a Duration_t of zero or more seconds"))?;
            if nanosec >= NANOS_PER_SECOND {
                return Err(WireError::invalid_value(
                    "a Duration_t of fewer than 10^9 nanoseconds past its second",
                ));
            }
            Duration::new(sec, nanosec)
        };
        Ok(finish.with(value))
    }
}

impl DdsMember for Duration {
    const DESC: ValueDesc = ValueDesc::Type {
        desc: <Self as DdsTypeInfo>::LAYOUT_REF,
    };

    fn serialize_member(&self, out: ValueSerializer<'_>) -> Result<(), WireError> {
        out.nested(self)
    }

    fn default_member() -> Result<Self, WireError> {
        Ok(Duration::ZERO)
    }

    fn field_value(&self, path: Option<&str>) -> Option<FieldValue> {
        time_field(duration_parts(self).ok(), path)
    }
}

impl<'de> DdsMemberRead<'de> for Duration {
    fn deserialize_member(input: ValueDeserializer<'_, 'de>) -> Result<Self, WireError> {
        input.nested()
    }
}

#[cfg(feature = "std")]
mod system_time {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::super::{
        DdsDeserialize, DdsDeserializer, DdsMember, DdsMemberRead, DdsSerialize, DdsSerializer,
        DdsTypeInfo, Decoded, FieldValue, Final, MemberDesc, Serialized, TypeDesc, ValueDesc,
        ValueDeserializer, ValueSerializer,
    };
    use super::NANOS_PER_SECOND;
    use crate::WireError;

    /// `TIME_INVALID_SEC`, DDS 1.4 §2.3.3.
    const INVALID_SEC: i32 = -1;

    impl DdsTypeInfo for SystemTime {
        type Members = (i32, (u32, ()));
        type Extensibility = Final;
        const LAYOUT: TypeDesc = TypeDesc::structure::<Self::Members, Final>(
            "DDS::Time_t",
            &[MemberDesc::new("sec"), MemberDesc::new("nanosec")],
        );
    }

    /// `sec` and `nanosec` of a `Time_t`.
    fn time_parts(value: &SystemTime) -> Result<(i32, u32), WireError> {
        let since = value.duration_since(UNIX_EPOCH).map_err(|_| {
            WireError::invalid_value("a SystemTime at or after 1970-01-01T00:00:00Z")
        })?;
        let sec = i32::try_from(since.as_secs())
            .map_err(|_| WireError::invalid_value("a SystemTime a Time_t's 32-bit second holds"))?;
        Ok((sec, since.subsec_nanos()))
    }

    impl DdsSerialize for SystemTime {
        fn serialize(&self, out: DdsSerializer<'_>) -> Result<Serialized, WireError> {
            let (sec, nanosec) = time_parts(self)?;
            out.serialize_struct::<Self>()?
                .member(&sec)?
                .member(&nanosec)?
                .end()
        }
    }

    impl<'de> DdsDeserialize<'de> for SystemTime {
        fn deserialize(input: DdsDeserializer<'_, 'de>) -> Result<Decoded<Self>, WireError> {
            let input = input.deserialize_struct::<Self>()?;
            let (sec, input) = input.member()?;
            let (nanosec, input) = input.member()?;
            let finish = input.end()?;
            if sec == INVALID_SEC {
                return Err(WireError::invalid_value(
                    "a valid Time_t; TIME_INVALID has no SystemTime",
                ));
            }
            let sec = u64::try_from(sec)
                .map_err(|_| WireError::invalid_value("a Time_t at or after 1970"))?;
            if nanosec >= NANOS_PER_SECOND {
                return Err(WireError::invalid_value(
                    "a Time_t of fewer than 10^9 nanoseconds past its second",
                ));
            }
            let value = UNIX_EPOCH
                .checked_add(core::time::Duration::new(sec, nanosec))
                .ok_or_else(|| WireError::invalid_value("a Time_t this host's clock holds"))?;
            Ok(finish.with(value))
        }
    }

    impl DdsMember for SystemTime {
        const DESC: ValueDesc = ValueDesc::Type {
            desc: <Self as DdsTypeInfo>::LAYOUT_REF,
        };

        fn serialize_member(&self, out: ValueSerializer<'_>) -> Result<(), WireError> {
            out.nested(self)
        }

        fn default_member() -> Result<Self, WireError> {
            Ok(UNIX_EPOCH)
        }

        fn field_value(&self, path: Option<&str>) -> Option<FieldValue> {
            super::time_field(time_parts(self).ok(), path)
        }
    }

    impl<'de> DdsMemberRead<'de> for SystemTime {
        fn deserialize_member(input: ValueDeserializer<'_, 'de>) -> Result<Self, WireError> {
            input.nested()
        }
    }
}
