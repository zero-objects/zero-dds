// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-py`. Safety classification: **STANDARD** (Binding-FFI).
//!
//! PyO3 bindings for the ZeroDDS DCPS API. Provides an importable
//! Python module `zerodds_py` via `maturin build` or directly via
//! `cargo build --features extension-module`.
//!
//! Spec: OMG DDS 1.4 (formal/2015-04-10) §2.2.2 with Python idioms.
//!
//! ## Layer position
//!
//! Layer 6 — PSMs / bindings.
//!
//! ## Build
//!
//! - **Default cargo build** (without Python headers): pyo3 is optional,
//!   the crate compiles as a lib placeholder. No Python API
//!   available — this is how the workspace CI runs without a Python dependency.
//! - **With `--features extension-module`** (or via `maturin build`):
//!   compiled as a `cdylib` and produces an importable
//!   Python module `zerodds_py`.
//!
//! ## API
//!
//! ```python
//! import zerodds_py as zerodds
//!
//! factory = zerodds.DomainParticipantFactory.instance()
//! participant = factory.create_participant_offline(0)
//!
//! topic = participant.create_bytes_topic("Chatter")
//! publisher = participant.create_publisher()
//! writer = publisher.create_bytes_writer(topic)
//!
//! subscriber = participant.create_subscriber()
//! reader = subscriber.create_bytes_reader(topic)
//!
//! writer.write(b"hello")
//! samples = reader.take()  # List[bytes]
//! ```
//!
//! Typed `DdsType` bindings (dataclass mapping via the IDL generator)
//! follow from `crates/idl-rust` through the Python bindings generator.

#![warn(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "extension-module")]
mod conditions;
#[cfg(feature = "extension-module")]
mod ffi;
#[cfg(feature = "extension-module")]
mod listener;
#[cfg(feature = "extension-module")]
mod qos;

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    #[test]
    fn dcps_re_export_compiles() {
        // Minimal smoke test: zerodds-dcps is reachable as a dependency.
        // Die echten PyO3-Tests laufen aus Python heraus (pytest).
        use zerodds_dcps::DomainParticipantFactory;
        let _f: &DomainParticipantFactory = DomainParticipantFactory::instance();
    }
}
