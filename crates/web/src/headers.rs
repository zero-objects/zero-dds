// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! HTTP-Headers — Spec §8.3.5 Tab 7 + Tab 8.

use alloc::string::String;

/// Spec §8.3.5 Tab 7 — Required HTTP-Request-Header `OMG-DDS-API-Key`.
pub const REQUEST_API_KEY: &str = "OMG-DDS-API-Key";

/// Spec §8.3.5 Tab 7 — Request-Header-Set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequestHeaders {
    /// `Accept` header — Spec verlangt `application/zerodds-web+xml`.
    pub accept: Option<String>,
    /// `Content-Type` header (only POST/PUT).
    pub content_type: Option<String>,
    /// `Content-Length` header (only POST/PUT/DELETE).
    pub content_length: Option<u64>,
    /// `Cache-Control` header — Spec verlangt RFC 2616 §14.9.
    pub cache_control: Option<String>,
    /// `OMG-DDS-API-Key` — Spec §8.3.5 Required.
    pub api_key: Option<String>,
}

impl RequestHeaders {
    /// Spec §8.3.5 — Required-Felder pruefen.
    ///
    /// # Errors
    /// `Err(MissingHeader::ApiKey)` wenn `api_key` fehlt.
    /// `Err(MissingHeader::Accept)` wenn `accept` fehlt.
    /// `Err(MissingHeader::CacheControl)` wenn `cache_control` fehlt.
    pub fn validate_required(&self) -> Result<(), MissingHeader> {
        if self.api_key.is_none() {
            return Err(MissingHeader::ApiKey);
        }
        if self.accept.is_none() {
            return Err(MissingHeader::Accept);
        }
        if self.cache_control.is_none() {
            return Err(MissingHeader::CacheControl);
        }
        Ok(())
    }
}

/// Spec §8.3.5 Tab 8 — Response-Header-Set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResponseHeaders {
    /// `Authentication-Info` (auf Response-zu-Login Required).
    pub authentication_info: Option<String>,
    /// `Cache-Control` (Required).
    pub cache_control: Option<String>,
    /// `Content-Length` (Required).
    pub content_length: Option<u64>,
    /// `Content-Type` (Required) — `application/zerodds-web+xml`.
    pub content_type: Option<String>,
    /// `Date` (Optional).
    pub date: Option<String>,
    /// `Expires` (Optional).
    pub expires: Option<String>,
    /// `Location` (Required fuer 201-Response auf POST).
    pub location: Option<String>,
    /// `Last-Modified` (Required fuer 200-Response auf GET/HEAD).
    pub last_modified: Option<String>,
}

/// Header-Validation-Fehler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingHeader {
    /// `OMG-DDS-API-Key` fehlt.
    ApiKey,
    /// `Accept` fehlt.
    Accept,
    /// `Cache-Control` fehlt.
    CacheControl,
}

impl core::fmt::Display for MissingHeader {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ApiKey => f.write_str("missing OMG-DDS-API-Key"),
            Self::Accept => f.write_str("missing Accept"),
            Self::CacheControl => f.write_str("missing Cache-Control"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for MissingHeader {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_api_key_constant_matches_spec() {
        assert_eq!(REQUEST_API_KEY, "OMG-DDS-API-Key");
    }

    #[test]
    fn validate_required_passes_with_all_three_headers() {
        let h = RequestHeaders {
            accept: Some(String::from("application/zerodds-web+xml")),
            cache_control: Some(String::from("no-cache")),
            api_key: Some(String::from("secret-key-1234")),
            ..Default::default()
        };
        assert_eq!(h.validate_required(), Ok(()));
    }

    #[test]
    fn missing_api_key_rejected() {
        let h = RequestHeaders {
            accept: Some(String::from("application/zerodds-web+xml")),
            cache_control: Some(String::from("no-cache")),
            ..Default::default()
        };
        assert_eq!(h.validate_required(), Err(MissingHeader::ApiKey));
    }

    #[test]
    fn missing_accept_rejected() {
        let h = RequestHeaders {
            cache_control: Some(String::from("no-cache")),
            api_key: Some(String::from("k")),
            ..Default::default()
        };
        assert_eq!(h.validate_required(), Err(MissingHeader::Accept));
    }

    #[test]
    fn missing_cache_control_rejected() {
        let h = RequestHeaders {
            accept: Some(String::from("application/zerodds-web+xml")),
            api_key: Some(String::from("k")),
            ..Default::default()
        };
        assert_eq!(h.validate_required(), Err(MissingHeader::CacheControl));
    }

    #[test]
    fn response_headers_default_is_empty() {
        let r = ResponseHeaders::default();
        assert!(r.cache_control.is_none());
        assert!(r.content_type.is_none());
        assert!(r.location.is_none());
    }
}
