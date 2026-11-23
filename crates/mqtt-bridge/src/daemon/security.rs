// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MQTT bridge §7.1: the TLS client configuration towards the broker
//! (`mqtts://`, `wss://`), from [`zerodds_bridge_security`]. The auth modes
//! of §7.2 are in [`super::auth`], the ACL of §7.3 in [`super::acl`].

pub use zerodds_bridge_security::{SecurityError, build_client_tls_connector};

use rustls::ClientConfig;
use std::path::Path;
use std::sync::Arc;

/// The rustls client configuration of `cfg`'s broker connection — the CA
/// of `mqtt.tls.ca_file`, the client certificate and key of
/// `mqtt.tls.cert_file` and `key_file` — or `None` without
/// `mqtt.tls.enabled`.
///
/// # Errors
/// [`SecurityError::Tls`] for TLS material that does not load.
pub fn client_tls_from_config(
    cfg: &super::config::DaemonConfig,
) -> Result<Option<Arc<ClientConfig>>, SecurityError> {
    if !cfg.broker_tls_enabled {
        return Ok(None);
    }
    let path = |p: &str| (!p.is_empty()).then(|| Path::new(p).to_path_buf());
    let ca = path(&cfg.broker_tls_ca_file);
    let cert = path(&cfg.broker_tls_client_cert_file);
    let key = path(&cfg.broker_tls_client_key_file);
    build_client_tls_connector(ca.as_deref(), cert.as_deref(), key.as_deref())
        .map(Some)
        .map_err(SecurityError::Tls)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn without_tls_there_is_no_client_configuration() {
        let cfg = crate::daemon::config::DaemonConfig::default_for_dev();
        assert!(client_tls_from_config(&cfg).unwrap().is_none());
        let mut missing = cfg;
        missing.broker_tls_enabled = true;
        missing.broker_tls_ca_file = "/nonexistent/ca.pem".into();
        assert!(matches!(
            client_tls_from_config(&missing),
            Err(SecurityError::Tls(_))
        ));
    }
}
