// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! gRPC HTTP `:path` parsing — Spec §"Call-Definition".

use alloc::string::String;
use core::fmt;

/// Path parsing error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    /// Path does not start with `/`.
    NotAbsolute,
    /// Path does not have the format `/<service>/<method>`.
    MalformedPath,
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAbsolute => f.write_str("path must start with /"),
            Self::MalformedPath => f.write_str("path must be /<service>/<method>"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for PathError {}

/// Spec §"Path" — `:path` "/" Service-Name "/" {method-name}.
///
/// Returns `(service_name, method_name)`. Both MUST be non-empty.
///
/// # Errors
/// Siehe [`PathError`].
pub fn parse_path(path: &str) -> Result<(String, String), PathError> {
    let stripped = path.strip_prefix('/').ok_or(PathError::NotAbsolute)?;
    let mut parts = stripped.splitn(2, '/');
    let service = parts.next().ok_or(PathError::MalformedPath)?;
    let method = parts.next().ok_or(PathError::MalformedPath)?;
    if service.is_empty() || method.is_empty() {
        return Err(PathError::MalformedPath);
    }
    if method.contains('/') {
        // Spec — method is "{ method name }" (single segment).
        return Err(PathError::MalformedPath);
    }
    Ok((String::from(service), String::from(method)))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_grpc_path() {
        // Spec §"Path" example.
        let (s, m) = parse_path("/helloworld.Greeter/SayHello").expect("ok");
        assert_eq!(s, "helloworld.Greeter");
        assert_eq!(m, "SayHello");
    }

    #[test]
    fn rejects_relative_path() {
        assert_eq!(
            parse_path("helloworld.Greeter/SayHello"),
            Err(PathError::NotAbsolute)
        );
    }

    #[test]
    fn rejects_path_without_method() {
        assert_eq!(
            parse_path("/helloworld.Greeter"),
            Err(PathError::MalformedPath)
        );
    }

    #[test]
    fn rejects_empty_service() {
        assert_eq!(parse_path("//SayHello"), Err(PathError::MalformedPath));
    }

    #[test]
    fn rejects_empty_method() {
        assert_eq!(parse_path("/Service/"), Err(PathError::MalformedPath));
    }

    #[test]
    fn rejects_method_with_slash() {
        // Spec — method is a single segment.
        assert_eq!(
            parse_path("/Service/Method/Sub"),
            Err(PathError::MalformedPath)
        );
    }

    #[test]
    fn parses_dotted_service_name() {
        // Spec — the service name can be fully qualified.
        let (s, m) = parse_path("/google.api.something.LongServiceName/MyMethod").expect("ok");
        assert_eq!(s, "google.api.something.LongServiceName");
        assert_eq!(m, "MyMethod");
    }
}
