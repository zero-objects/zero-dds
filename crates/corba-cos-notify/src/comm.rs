// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CosNotifyComm §3 — structured-event push/pull traits + publish/subscribe.

use alloc::boxed::Box;

use crate::event::{EventType, StructuredEvent};

/// `Disconnected` — the proxy/endpoint is no longer connected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disconnected;

/// Connect error (§3 / §4): already connected or nil reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectError {
    /// `AlreadyConnected`.
    AlreadyConnected,
    /// `TypeError` — incompatible endpoint type.
    TypeError,
}

/// `CosNotifyComm::StructuredPushConsumer` (§3.2): receives pushed events.
pub trait StructuredPushConsumer: Send + Sync {
    /// `push_structured_event` — deliver an event.
    ///
    /// # Errors
    /// `Disconnected` if the consumer is disconnected.
    fn push_structured_event(&self, event: StructuredEvent) -> Result<(), Disconnected>;
    /// `disconnect_structured_push_consumer`.
    fn disconnect(&self);
}

/// `CosNotifyComm::StructuredPushSupplier` (§3.3).
pub trait StructuredPushSupplier: Send + Sync {
    /// `disconnect_structured_push_supplier`.
    fn disconnect(&self);
}

/// `CosNotifyComm::StructuredPullConsumer` (§3.4).
pub trait StructuredPullConsumer: Send + Sync {
    /// `disconnect_structured_pull_consumer`.
    fn disconnect(&self);
}

/// `CosNotifyComm::StructuredPullSupplier` (§3.5): supplies events on demand.
pub trait StructuredPullSupplier: Send + Sync {
    /// `pull_structured_event` — blocks (here: error if empty) until an event is available.
    ///
    /// # Errors
    /// `Disconnected`.
    fn pull_structured_event(&self) -> Result<StructuredEvent, Disconnected>;
    /// `try_pull_structured_event` — non-blocking; `bool` = `has_event`.
    ///
    /// # Errors
    /// `Disconnected`.
    fn try_pull_structured_event(&self) -> Result<(StructuredEvent, bool), Disconnected>;
    /// `disconnect_structured_pull_supplier`.
    fn disconnect(&self);
}

/// `CosNotifyComm::NotifyPublish` (§3.1): supplier announces event types.
pub trait NotifyPublish: Send + Sync {
    /// `offer_change` — `added`/`removed` event types.
    fn offer_change(&self, added: &[EventType], removed: &[EventType]);
}

/// `CosNotifyComm::NotifySubscribe` (§3.1): consumer subscribes to event types.
pub trait NotifySubscribe: Send + Sync {
    /// `subscription_change` — `added`/`removed` event types.
    fn subscription_change(&self, added: &[EventType], removed: &[EventType]);
}

/// Boxed push-consumer callback for the channel push path.
pub type BoxedPushConsumer = Box<dyn StructuredPushConsumer>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_error_variants_distinct() {
        assert_ne!(ConnectError::AlreadyConnected, ConnectError::TypeError);
    }
}
