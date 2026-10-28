// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The configurations an access-control plugin hands out and takes back
//! (DDS-Security 1.2 §9.4.2.9.22 to §9.4.2.9.29), shared by the builtin
//! plugin and the PSK plugin.

use alloc::vec::Vec;

use zerodds_security::error::{SecurityError, SecurityResult};

/// Takes `item` out of `issued`, or says that `plugin` did not issue it.
pub(crate) fn give_back<T: PartialEq>(
    issued: &mut Vec<T>,
    item: &T,
    plugin: &str,
    what: &str,
) -> SecurityResult<()> {
    match issued.iter().position(|i| i == item) {
        Some(index) => {
            issued.swap_remove(index);
            Ok(())
        }
        None => Err(SecurityError::bad_argument(alloc::format!(
            "{plugin}: {what} is none that the plugin issued"
        ))),
    }
}

/// The four `return_*_security_config` operations of `AccessControlPlugin`
/// (§9.4.2.9.26 to §9.4.2.9.29) over the fields `issued_participant_configs`,
/// `issued_topic_configs`, `issued_writer_configs` and
/// `issued_reader_configs`; `$plugin` names the plugin in the error.
macro_rules! return_security_configs {
    ($plugin:literal) => {
        fn return_participant_security_config(
            &mut self,
            config: ParticipantSecurityConfig,
        ) -> SecurityResult<()> {
            $crate::issued::give_back(
                &mut self.issued_participant_configs,
                &config,
                $plugin,
                "the ParticipantSecurityConfig",
            )
        }

        fn return_topic_security_config(
            &mut self,
            config: TopicSecurityConfig,
        ) -> SecurityResult<()> {
            $crate::issued::give_back(
                &mut self.issued_topic_configs,
                &config,
                $plugin,
                "the TopicSecurityConfig",
            )
        }

        fn return_datawriter_security_config(
            &mut self,
            config: EndpointSecurityConfig,
        ) -> SecurityResult<()> {
            $crate::issued::give_back(
                &mut self.issued_writer_configs,
                &config,
                $plugin,
                "the EndpointSecurityConfig of the DataWriter",
            )
        }

        fn return_datareader_security_config(
            &mut self,
            config: EndpointSecurityConfig,
        ) -> SecurityResult<()> {
            $crate::issued::give_back(
                &mut self.issued_reader_configs,
                &config,
                $plugin,
                "the EndpointSecurityConfig of the DataReader",
            )
        }
    };
}
