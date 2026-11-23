// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `Deployment::Property` — a name and a typed value (CORBA 3.3 Part 3
//! §16, `struct Property { string name; any value; }`).
//!
//! A configuration property of a deployment plan carries its IDL type:
//! the example of Part 3 §7.6.4 writes
//!
//! ```xml
//! <configProperty>
//!   <name>mcast_port</name>
//!   <type>unsigned short</type>
//!   <value>31337</value>
//! </configProperty>
//! ```
//!
//! The value is held as the `any` it becomes in
//! `Components::ConfigValue` ([`zerodds_corba_rt::any::AnyValue`]). The
//! basic types of IDL and `string` have a text form; any other type is
//! refused with [`PropertyError::UnsupportedType`].

use alloc::format;
use alloc::string::{String, ToString};

use zerodds_corba_rt::TypeCode;
use zerodds_corba_rt::any::AnyValue;

/// One property: a name and a value.
#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    /// The property's name, e.g. an attribute of a component.
    pub name: String,
    /// The value.
    pub value: AnyValue,
}

impl Property {
    /// A property of IDL type `type_name` with the value written as
    /// `text`.
    ///
    /// # Errors
    /// [`PropertyError`] for a type without a text form or a text that is
    /// no value of the type.
    pub fn parse(name: &str, type_name: &str, text: &str) -> Result<Self, PropertyError> {
        Ok(Self {
            name: name.to_string(),
            value: value_from_text(type_name, text)?,
        })
    }

    /// A `string` property.
    #[must_use]
    pub fn string(name: &str, value: &str) -> Self {
        Self {
            name: name.to_string(),
            value: AnyValue::Str(value.to_string()),
        }
    }
}

/// What a property cannot be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyError {
    /// The type has no text form here.
    UnsupportedType(String),
    /// The text is no value of the type.
    InvalidValue {
        /// The IDL type.
        type_name: String,
        /// The text.
        text: String,
    },
}

impl core::fmt::Display for PropertyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsupportedType(t) => write!(f, "a property of type `{t}` has no text form"),
            Self::InvalidValue { type_name, text } => {
                write!(f, "`{text}` is no value of type `{type_name}`")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for PropertyError {}

/// The value of IDL type `type_name` written as `text`.
///
/// # Errors
/// As [`Property::parse`].
pub fn value_from_text(type_name: &str, text: &str) -> Result<AnyValue, PropertyError> {
    let normalized: String = type_name
        .split_whitespace()
        .collect::<alloc::vec::Vec<_>>()
        .join(" ");
    let invalid = || PropertyError::InvalidValue {
        type_name: normalized.clone(),
        text: text.to_string(),
    };
    let t = text.trim();
    Ok(match normalized.as_str() {
        "string" => AnyValue::Str(text.to_string()),
        "boolean" => match t {
            "true" | "TRUE" | "1" => AnyValue::Boolean(true),
            "false" | "FALSE" | "0" => AnyValue::Boolean(false),
            _ => return Err(invalid()),
        },
        "octet" => AnyValue::Octet(t.parse().map_err(|_| invalid())?),
        "char" => {
            let mut chars = text.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_ascii() => AnyValue::Char(c as u8),
                _ => return Err(invalid()),
            }
        }
        "short" => AnyValue::Short(t.parse().map_err(|_| invalid())?),
        "unsigned short" => AnyValue::UShort(t.parse().map_err(|_| invalid())?),
        "long" => AnyValue::Long(t.parse().map_err(|_| invalid())?),
        "unsigned long" => AnyValue::ULong(t.parse().map_err(|_| invalid())?),
        "long long" => AnyValue::LongLong(t.parse().map_err(|_| invalid())?),
        "unsigned long long" => AnyValue::ULongLong(t.parse().map_err(|_| invalid())?),
        "float" => AnyValue::Float(t.parse().map_err(|_| invalid())?),
        "double" => AnyValue::Double(t.parse().map_err(|_| invalid())?),
        _ => return Err(PropertyError::UnsupportedType(normalized)),
    })
}

/// The `TypeCode` of the IDL type `type_name`, for the types that have
/// a text form: the element type of a sequence of such values.
///
/// # Errors
/// [`PropertyError::UnsupportedType`] for any other type.
pub fn type_code_from_text(type_name: &str) -> Result<TypeCode, PropertyError> {
    let normalized: String = type_name
        .split_whitespace()
        .collect::<alloc::vec::Vec<_>>()
        .join(" ");
    Ok(match normalized.as_str() {
        "string" => TypeCode::String(0),
        "boolean" => TypeCode::Boolean,
        "octet" => TypeCode::Octet,
        "char" => TypeCode::Char,
        "short" => TypeCode::Short,
        "unsigned short" => TypeCode::UShort,
        "long" => TypeCode::Long,
        "unsigned long" => TypeCode::ULong,
        "long long" => TypeCode::LongLong,
        "unsigned long long" => TypeCode::ULongLong,
        "float" => TypeCode::Float,
        "double" => TypeCode::Double,
        _ => return Err(PropertyError::UnsupportedType(normalized)),
    })
}

/// The IDL type name of a `TypeCode` that [`type_code_from_text`] gives.
///
/// # Errors
/// [`PropertyError::UnsupportedType`] for any other `TypeCode`.
pub fn type_code_to_text(type_code: &TypeCode) -> Result<&'static str, PropertyError> {
    Ok(match type_code {
        TypeCode::String(0) => "string",
        TypeCode::Boolean => "boolean",
        TypeCode::Octet => "octet",
        TypeCode::Char => "char",
        TypeCode::Short => "short",
        TypeCode::UShort => "unsigned short",
        TypeCode::Long => "long",
        TypeCode::ULong => "unsigned long",
        TypeCode::LongLong => "long long",
        TypeCode::ULongLong => "unsigned long long",
        TypeCode::Float => "float",
        TypeCode::Double => "double",
        other => return Err(PropertyError::UnsupportedType(format!("{other:?}"))),
    })
}

/// The IDL type name and the text of a value.
///
/// # Errors
/// [`PropertyError::UnsupportedType`] for a value of another type.
pub fn value_to_text(value: &AnyValue) -> Result<(&'static str, String), PropertyError> {
    Ok(match value {
        AnyValue::Str(s) => ("string", s.clone()),
        AnyValue::Boolean(b) => ("boolean", b.to_string()),
        AnyValue::Octet(v) => ("octet", v.to_string()),
        AnyValue::Char(c) => ("char", char::from(*c).to_string()),
        AnyValue::Short(v) => ("short", v.to_string()),
        AnyValue::UShort(v) => ("unsigned short", v.to_string()),
        AnyValue::Long(v) => ("long", v.to_string()),
        AnyValue::ULong(v) => ("unsigned long", v.to_string()),
        AnyValue::LongLong(v) => ("long long", v.to_string()),
        AnyValue::ULongLong(v) => ("unsigned long long", v.to_string()),
        AnyValue::Float(v) => ("float", v.to_string()),
        AnyValue::Double(v) => ("double", v.to_string()),
        other => {
            return Err(PropertyError::UnsupportedType(format!("{other:?}")));
        }
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn corba_p3_7_6_4_typed_property_values_round_trip_through_text() {
        let cases = [
            ("string", "224.1.1.1", AnyValue::Str("224.1.1.1".into())),
            ("unsigned short", "31337", AnyValue::UShort(31337)),
            ("unsigned  long", "50", AnyValue::ULong(50)),
            ("long", "-7", AnyValue::Long(-7)),
            (
                "long long",
                "-9000000000",
                AnyValue::LongLong(-9_000_000_000),
            ),
            (
                "unsigned long long",
                "18000000000",
                AnyValue::ULongLong(18_000_000_000),
            ),
            ("short", "-3", AnyValue::Short(-3)),
            ("octet", "255", AnyValue::Octet(255)),
            ("boolean", "true", AnyValue::Boolean(true)),
            ("char", "x", AnyValue::Char(b'x')),
            ("double", "2.5", AnyValue::Double(2.5)),
            ("float", "0.5", AnyValue::Float(0.5)),
        ];
        for (type_name, text, value) in cases {
            let parsed = value_from_text(type_name, text).unwrap();
            assert_eq!(parsed, value, "{type_name}");
            let (back_type, back_text) = value_to_text(&parsed).unwrap();
            assert_eq!(value_from_text(back_type, &back_text).unwrap(), value);
        }
    }

    #[test]
    fn corba_p3_7_6_4_a_value_out_of_range_or_of_an_unknown_type_is_refused() {
        assert!(matches!(
            value_from_text("unsigned short", "70000"),
            Err(PropertyError::InvalidValue { .. })
        ));
        assert!(matches!(
            value_from_text("boolean", "yes"),
            Err(PropertyError::InvalidValue { .. })
        ));
        assert!(matches!(
            value_from_text("char", "xy"),
            Err(PropertyError::InvalidValue { .. })
        ));
        assert_eq!(
            value_from_text("Demo::Qos", "x"),
            Err(PropertyError::UnsupportedType("Demo::Qos".into()))
        );
        assert!(value_to_text(&AnyValue::Null).is_err());
    }
}
