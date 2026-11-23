// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The connection a connector fragment holds to an interface the other
//! side provides: `provides Listener data_listener` and
//! `provides PortStatusListener status` of a port (the component provides
//! them, the fragment calls them), `uses ConnectorStatusListener
//! error_listener` of a connector.

use std::sync::{Arc, PoisonError, RwLock};

/// A simplex receptacle for the interface `L`.
pub struct Receptacle<L: ?Sized> {
    connection: RwLock<Option<Arc<L>>>,
}

impl<L: ?Sized> Receptacle<L> {
    /// A receptacle without a connection.
    #[must_use]
    pub fn new() -> Self {
        Self {
            connection: RwLock::new(None),
        }
    }

    /// Connects `object` and returns the connection it replaces.
    pub fn connect(&self, object: Arc<L>) -> Option<Arc<L>> {
        self.connection
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .replace(object)
    }

    /// Removes the connection and returns it.
    pub fn disconnect(&self) -> Option<Arc<L>> {
        self.connection
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }

    /// The connected object, `None` without a connection.
    #[must_use]
    pub fn get_connection(&self) -> Option<Arc<L>> {
        self.connection
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl<L: ?Sized> Default for Receptacle<L> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dds4ccm_7_2_2_2_a_receptacle_holds_one_connection() {
        let receptacle: Receptacle<str> = Receptacle::new();
        assert!(receptacle.get_connection().is_none());
        assert!(receptacle.connect(Arc::from("a")).is_none());
        assert_eq!(receptacle.connect(Arc::from("b")).as_deref(), Some("a"));
        assert_eq!(receptacle.get_connection().as_deref(), Some("b"));
        assert_eq!(receptacle.disconnect().as_deref(), Some("b"));
        assert!(receptacle.get_connection().is_none());
        assert!(receptacle.disconnect().is_none());
    }
}
