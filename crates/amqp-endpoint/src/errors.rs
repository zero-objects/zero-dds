// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AMQP-Error-Code-Mapping + Diagnostic-Description-Format.
//!
//! Spec-Quellen:
//! * dds-amqp-1.0 §11.1 Encode/Decode Failures.
//! * §11.2 Connection / Session / Link Lifecycle Failures.
//! * §11.3 Instance-Lifecycle Failures.
//! * §11.4 Diagnostic Description Strings (Format
//!   `<spec-section>: <human-readable>`).
//!
//! Ergaenzt §7.5.1 — `ResolutionError::NoRoute` ↔
//! `amqp:not-found`-Link-Error wenn `permit_dynamic_topics =
//! false`.

use alloc::string::String;
use core::fmt;

use crate::mapping::MappingError;
use crate::routing::ResolutionError;

/// Spec §11 — Disposition-Scope eines Errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorScope {
    /// Per-Transfer Disposition `rejected`. Link bleibt offen.
    Transfer,
    /// Link-Error. Per Spec auch der entsprechende Detach.
    Link,
    /// Connection-Close mit Frame.
    Connection,
}

/// Spec §11 — kanonische AMQP-Error-Conditions, die der Endpoint
/// emittiert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmqpErrorCondition {
    /// `amqp:decode-error` — XCDR2-Decode/Body-Mismatch/Type-
    /// Collision.
    DecodeError,
    /// `amqp:not-implemented` — durability=unsettled-state,
    /// unbekannter `dds:operation`-Wert, etc.
    NotImplemented,
    /// `amqp:not-found` — Address nicht im Catalog und
    /// `permit_dynamic_topics = false`.
    NotFound,
    /// `amqp:resource-limit-exceeded` — max-connections,
    /// catalog-cap, max-frame-size, idle-timeout.
    ResourceLimitExceeded,
    /// `amqp:unauthorized-access` — DDS-Security AccessControl-
    /// Plugin lehnt ab.
    UnauthorizedAccess,
    /// `amqp:precondition-failed` — `dds:operation = unregister`
    /// auf unbekannter Instanz, Reply-Validation D.4.1
    /// `correlation-id absent`.
    PreconditionFailed,
    /// `amqp:connection:framing-error` — Frame-Size ueberschreitet
    /// `max-frame-size`.
    FramingError,
}

impl AmqpErrorCondition {
    /// Spec-konformer Wire-String fuer das `error.condition`-Feld.
    #[must_use]
    pub const fn as_symbol(self) -> &'static str {
        match self {
            Self::DecodeError => "amqp:decode-error",
            Self::NotImplemented => "amqp:not-implemented",
            Self::NotFound => "amqp:not-found",
            Self::ResourceLimitExceeded => "amqp:resource-limit-exceeded",
            Self::UnauthorizedAccess => "amqp:unauthorized-access",
            Self::PreconditionFailed => "amqp:precondition-failed",
            Self::FramingError => "amqp:connection:framing-error",
        }
    }
}

/// Spec §11.4 — `error.description`-String mit kanonischem
/// `<spec-section>: <human-text>`-Format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorDescription {
    /// `<spec-section>` (z.B. `§7.2.1.3` oder `Annex D §D.4.1`).
    pub spec_section: String,
    /// Mensch-lesbarer Text (Englisch; Spec verlangt ASCII-only
    /// nicht, aber die OMG-Konvention ist Englisch).
    pub message: String,
}

impl ErrorDescription {
    /// Konstruktor.
    pub fn new(spec_section: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            spec_section: spec_section.into(),
            message: message.into(),
        }
    }

    /// Spec §11.4 — Wire-String `<§-Ref>: <Text>`.
    #[must_use]
    pub fn render(&self) -> String {
        alloc::format!("{}: {}", self.spec_section, self.message)
    }
}

impl fmt::Display for ErrorDescription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.spec_section, self.message)
    }
}

/// Spec §11 — vollstaendige Error-Information, die der Endpoint
/// ueber AMQP zurueckmeldet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmqpError {
    /// AMQP-Wire-Error-Condition.
    pub condition: AmqpErrorCondition,
    /// Disposition-Scope (Transfer / Link / Connection).
    pub scope: ErrorScope,
    /// Diagnostic-Description fuer den `error.description`-String.
    pub description: ErrorDescription,
}

impl AmqpError {
    /// Konstruktor.
    pub fn new(
        condition: AmqpErrorCondition,
        scope: ErrorScope,
        description: ErrorDescription,
    ) -> Self {
        Self {
            condition,
            scope,
            description,
        }
    }
}

// ============================================================
// Spec-Mapping-Helpers
// ============================================================

/// Spec §11.1 — `MappingError` aus dem Body-Encoding-Pfad auf
/// AMQP-Errors abbilden.
#[must_use]
pub fn map_mapping_error(err: &MappingError) -> AmqpError {
    match err {
        MappingError::InvalidUtf8 => AmqpError::new(
            AmqpErrorCondition::DecodeError,
            ErrorScope::Transfer,
            ErrorDescription::new("§11.1", "JSON body is not valid UTF-8"),
        ),
        MappingError::InvalidJson(msg) => AmqpError::new(
            AmqpErrorCondition::DecodeError,
            ErrorScope::Transfer,
            ErrorDescription::new("§11.1", alloc::format!("JSON body parse error: {msg}")),
        ),
        MappingError::EmptyBody => AmqpError::new(
            AmqpErrorCondition::DecodeError,
            ErrorScope::Transfer,
            ErrorDescription::new("§11.1", "body section is empty"),
        ),
    }
}

/// Spec §7.5.1 — `ResolutionError::NoRoute` auf
/// `amqp:not-found`-Link-Error abbilden, wenn
/// `permit_dynamic_topics = false`.
#[must_use]
pub fn map_resolution_error(err: &ResolutionError, permit_dynamic_topics: bool) -> AmqpError {
    match err {
        ResolutionError::NoRoute(addr) => {
            if permit_dynamic_topics {
                // permit_dynamic_topics=true: Caller soll on-the-fly
                // anlegen; hier melden wir es als NotFound mit
                // Hinweis auf §7.5.1, das Caller-Verhalten regelt.
                AmqpError::new(
                    AmqpErrorCondition::NotFound,
                    ErrorScope::Link,
                    ErrorDescription::new(
                        "§7.5.1",
                        alloc::format!(
                            "address '{addr}' not in catalog (dynamic-topic creation enabled)"
                        ),
                    ),
                )
            } else {
                AmqpError::new(
                    AmqpErrorCondition::NotFound,
                    ErrorScope::Link,
                    ErrorDescription::new(
                        "§7.5.1",
                        alloc::format!(
                            "address '{addr}' not in catalog and permit_dynamic_topics = false"
                        ),
                    ),
                )
            }
        }
        ResolutionError::Malformed(addr) => AmqpError::new(
            AmqpErrorCondition::DecodeError,
            ErrorScope::Link,
            ErrorDescription::new("§7.3", alloc::format!("malformed AMQP address '{addr}'")),
        ),
    }
}

/// Spec §11.2 — Resource-Limit-Verletzungen.
#[must_use]
pub fn resource_limit_exceeded(
    spec_section: impl Into<String>,
    message: impl Into<String>,
) -> AmqpError {
    AmqpError::new(
        AmqpErrorCondition::ResourceLimitExceeded,
        ErrorScope::Connection,
        ErrorDescription::new(spec_section, message),
    )
}

/// Spec §11.2 — `terminus.durable=unsettled-state`-Reject.
#[must_use]
pub fn unsettled_state_not_implemented() -> AmqpError {
    AmqpError::new(
        AmqpErrorCondition::NotImplemented,
        ErrorScope::Link,
        ErrorDescription::new(
            "§7.4.2",
            "terminus.durable = unsettled-state requires broker functionality (out of scope)",
        ),
    )
}

/// Spec §11.2 — Unbekannter `dds:operation`-Wert.
#[must_use]
pub fn unknown_dds_operation(value: &str) -> AmqpError {
    AmqpError::new(
        AmqpErrorCondition::NotImplemented,
        ErrorScope::Link,
        ErrorDescription::new(
            "§7.7",
            alloc::format!("unknown dds:operation value '{value}'"),
        ),
    )
}

/// Spec §11.3 — Instance-Lifecycle-Failure: `unregister`/`dispose`
/// auf unbekannter Instanz.
#[must_use]
pub fn instance_unknown(op: &str, key: &str) -> AmqpError {
    AmqpError::new(
        AmqpErrorCondition::PreconditionFailed,
        ErrorScope::Transfer,
        ErrorDescription::new(
            "§11.3",
            alloc::format!("dds:operation = {op} on unknown instance key '{key}'"),
        ),
    )
}

/// Spec §11.3 — `register` ohne Key-Felder im Body.
#[must_use]
pub fn register_missing_key() -> AmqpError {
    AmqpError::new(
        AmqpErrorCondition::DecodeError,
        ErrorScope::Transfer,
        ErrorDescription::new(
            "§11.3",
            "dds:operation = register but body lacks key fields",
        ),
    )
}

/// Spec §11.2 — DDS-Security AccessControl-Reject.
#[must_use]
pub fn access_denied(subject: &str, address: &str) -> AmqpError {
    AmqpError::new(
        AmqpErrorCondition::UnauthorizedAccess,
        ErrorScope::Link,
        ErrorDescription::new(
            "§10.3.3",
            alloc::format!("subject '{subject}' denied access to '{address}'"),
        ),
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    // --- Symbols ---

    #[test]
    fn condition_symbols_match_spec() {
        assert_eq!(
            AmqpErrorCondition::DecodeError.as_symbol(),
            "amqp:decode-error"
        );
        assert_eq!(
            AmqpErrorCondition::NotImplemented.as_symbol(),
            "amqp:not-implemented"
        );
        assert_eq!(AmqpErrorCondition::NotFound.as_symbol(), "amqp:not-found");
        assert_eq!(
            AmqpErrorCondition::ResourceLimitExceeded.as_symbol(),
            "amqp:resource-limit-exceeded"
        );
        assert_eq!(
            AmqpErrorCondition::UnauthorizedAccess.as_symbol(),
            "amqp:unauthorized-access"
        );
        assert_eq!(
            AmqpErrorCondition::PreconditionFailed.as_symbol(),
            "amqp:precondition-failed"
        );
        assert_eq!(
            AmqpErrorCondition::FramingError.as_symbol(),
            "amqp:connection:framing-error"
        );
    }

    // --- §11.4 Diagnostic-Format ---

    #[test]
    fn description_renders_spec_section_then_text() {
        let d = ErrorDescription::new("§7.2.1.3", "type-id collision detected");
        assert_eq!(d.render(), "§7.2.1.3: type-id collision detected");
    }

    #[test]
    fn description_display_matches_render() {
        let d = ErrorDescription::new("§D.4.1", "correlation-id absent");
        assert_eq!(alloc::format!("{d}"), d.render());
    }

    // --- §11.1 Mapping-Error-Mapper ---

    #[test]
    fn invalid_utf8_maps_to_decode_error_transfer() {
        let e = map_mapping_error(&MappingError::InvalidUtf8);
        assert_eq!(e.condition, AmqpErrorCondition::DecodeError);
        assert_eq!(e.scope, ErrorScope::Transfer);
        assert!(e.description.spec_section.contains("§11.1"));
    }

    #[test]
    fn invalid_json_maps_to_decode_error() {
        let e = map_mapping_error(&MappingError::InvalidJson("bad token".into()));
        assert_eq!(e.condition, AmqpErrorCondition::DecodeError);
        assert!(e.description.message.contains("bad token"));
    }

    #[test]
    fn empty_body_maps_to_decode_error() {
        let e = map_mapping_error(&MappingError::EmptyBody);
        assert_eq!(e.condition, AmqpErrorCondition::DecodeError);
    }

    // --- §7.5.1 NoRoute-Mapping ---

    #[test]
    fn no_route_with_dynamic_disabled_yields_not_found_link() {
        let e = map_resolution_error(&ResolutionError::NoRoute("X".into()), false);
        assert_eq!(e.condition, AmqpErrorCondition::NotFound);
        assert_eq!(e.scope, ErrorScope::Link);
        assert!(
            e.description
                .message
                .contains("permit_dynamic_topics = false")
        );
        assert!(e.description.spec_section.contains("§7.5.1"));
    }

    #[test]
    fn no_route_with_dynamic_enabled_still_not_found() {
        // Spec §7.5.1 — `true`-Pfad wird Caller-seitig in
        // dynamic-topic-creation umgesetzt; wir liefern hier
        // dieselbe Condition aber andere Description.
        let e = map_resolution_error(&ResolutionError::NoRoute("X".into()), true);
        assert_eq!(e.condition, AmqpErrorCondition::NotFound);
        assert!(
            e.description
                .message
                .contains("dynamic-topic creation enabled")
        );
    }

    #[test]
    fn malformed_address_maps_to_decode_error() {
        let e = map_resolution_error(&ResolutionError::Malformed("bad://".into()), false);
        assert_eq!(e.condition, AmqpErrorCondition::DecodeError);
        assert_eq!(e.scope, ErrorScope::Link);
    }

    // --- §11.2 Resource-Limits + Durability ---

    #[test]
    fn resource_limit_exceeded_is_connection_scope() {
        let e = resource_limit_exceeded("§7.10", "max-connections cap reached");
        assert_eq!(e.condition, AmqpErrorCondition::ResourceLimitExceeded);
        assert_eq!(e.scope, ErrorScope::Connection);
    }

    #[test]
    fn unsettled_state_yields_not_implemented() {
        let e = unsettled_state_not_implemented();
        assert_eq!(e.condition, AmqpErrorCondition::NotImplemented);
        assert_eq!(e.scope, ErrorScope::Link);
        assert!(e.description.spec_section.contains("§7.4.2"));
    }

    #[test]
    fn unknown_dds_operation_yields_not_implemented() {
        let e = unknown_dds_operation("teleport");
        assert_eq!(e.condition, AmqpErrorCondition::NotImplemented);
        assert!(e.description.message.contains("teleport"));
    }

    // --- §11.3 Instance-Lifecycle ---

    #[test]
    fn instance_unknown_yields_precondition_failed() {
        let e = instance_unknown("unregister", "key-7");
        assert_eq!(e.condition, AmqpErrorCondition::PreconditionFailed);
        assert_eq!(e.scope, ErrorScope::Transfer);
        assert!(e.description.message.contains("key-7"));
        assert!(e.description.message.contains("unregister"));
    }

    #[test]
    fn register_missing_key_yields_decode_error() {
        let e = register_missing_key();
        assert_eq!(e.condition, AmqpErrorCondition::DecodeError);
        assert!(e.description.message.contains("register"));
    }

    // --- §10.3.3 AccessControl-Reject ---

    #[test]
    fn access_denied_yields_unauthorized_access_link() {
        let e = access_denied("CN=eve", "Sensor");
        assert_eq!(e.condition, AmqpErrorCondition::UnauthorizedAccess);
        assert_eq!(e.scope, ErrorScope::Link);
        assert!(e.description.message.contains("CN=eve"));
        assert!(e.description.message.contains("Sensor"));
    }
}
