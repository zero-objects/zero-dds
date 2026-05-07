// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-py`. Safety classification: **STANDARD** (Binding-FFI).
//!
//! PyO3-Bindings fuer ZeroDDS DCPS-API. Liefert ein importierbares
//! Python-Modul `zerodds_py` ueber `maturin build` oder direkt via
//! `cargo build --features extension-module`.
//!
//! Spec: OMG DDS 1.4 (formal/2015-04-10) §2.2.2 mit Python-Idiomen.
//!
//! ## Schichten-Position
//!
//! Layer 6 — PSMs / Bindings.
//!
//! ## Build
//!
//! - **Default-Cargo-Build** (ohne Python-Headers): pyo3 ist optional,
//!   das Crate compilet als lib-Platzhalter. Keine Python-API
//!   verfuegbar — so laeuft der Workspace-CI ohne Python-Dependency.
//! - **Mit `--features extension-module`** (oder via `maturin build`):
//!   wird als `cdylib` kompiliert und erzeugt ein importierbares
//!   Python-Modul `zerodds_py`.
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
//! Typisierte `DdsType`-Bindings (Dataclass-Mapping via IDL-Generator)
//! folgen aus `crates/idl-rust` ueber Python-Bindings-Generator.

#![warn(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "extension-module")]
mod ffi;

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    #[test]
    fn dcps_re_export_compiles() {
        // Minimaler Smoketest: zerodds-dcps ist als Dependency greifbar.
        // Die echten PyO3-Tests laufen aus Python heraus (pytest).
        use zerodds_dcps::DomainParticipantFactory;
        let _f: &DomainParticipantFactory = DomainParticipantFactory::instance();
    }
}
