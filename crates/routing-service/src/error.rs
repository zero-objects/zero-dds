// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Error type for the routing service.

use thiserror::Error;

/// Result alias for routing-service operations.
pub type Result<T> = core::result::Result<T, RoutingError>;

/// Failure modes of the routing service.
#[derive(Debug, Error)]
pub enum RoutingError {
    /// Invalid or unparseable configuration.
    #[error("config error: {0}")]
    Config(String),

    /// A DDS participant/reader/writer could not be created or used.
    #[error("dds runtime error: {0}")]
    Dds(String),

    /// A route references a type that could not be resolved into a DynamicData
    /// shape (required for content filtering or transformation).
    #[error("type shape unavailable for route '{route}': {reason}")]
    TypeShape {
        /// Route name.
        route: String,
        /// Why the shape could not be resolved.
        reason: String,
    },

    /// A content-filter expression failed to parse.
    #[error("filter error in route '{route}': {reason}")]
    Filter {
        /// Route name.
        route: String,
        /// Parser/binding failure detail.
        reason: String,
    },

    /// A transform rule was invalid for the route's type.
    #[error("transform error in route '{route}': {reason}")]
    Transform {
        /// Route name.
        route: String,
        /// Detail.
        reason: String,
    },

    /// Internal lock poisoned.
    #[error("internal: {0}")]
    Internal(&'static str),
}
