//! Shared test helpers.
//!
//! Integration tests in this directory include this via
//! `#[path = "common/mod.rs"] mod common;` in the respective
//! test file.

#![allow(dead_code)]
// A helper that cannot get a domain, or cannot write its lease, has to
// stop the test that called it — there is no correct value to return,
// and a returned default would put two tests on one set of UDP ports.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::unwrap_used
)]

use core::sync::atomic::{AtomicU32, Ordering};
use std::net::Ipv4Addr;
use std::time::Duration;

use zerodds_dcps::runtime::RuntimeConfig;

/// Returns a `RuntimeConfig` with an SPDP multicast group that is
/// **unique per call**.
///
/// Background: by default all DCPS integration tests in the same binary
/// share the spec multicast group `239.255.0.1:7400`. When N tests run
/// in parallel, N participants compete for SPDP multicast reception —
/// packets get delivered to the "wrong" reader, causing match timeouts.
///
/// This function generates an admin-scoped multicast group (`239.X.Y.Z`,
/// RFC 2365 organization-local) from PID + call counter. Two
/// `isolated_cfg()` calls return **different** groups — anyone who wants
/// the same bus for multiple participants **must clone the config**.
///
/// ```ignore
/// let cfg = isolated_cfg();
/// let a = factory.create_participant_with_config(domain, qos, cfg.clone()).unwrap();
/// let b = factory.create_participant_with_config(domain, qos, cfg).unwrap();
/// // a and b find each other via multicast 239.X.Y.Z.
/// ```
///
/// Fast discovery period for tests (100 ms instead of the 5 s spec default).
pub fn isolated_cfg() -> RuntimeConfig {
    static SLOT: AtomicU32 = AtomicU32::new(0);
    let slot = SLOT.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let raw = pid.wrapping_mul(2654435761) ^ slot.wrapping_mul(0x9E3779B9);
    // 239.255.X.Y — same-subnet scope (RFC 2365 local scope, /16 suffix of
    // the spec default 239.255.0.1). GitHub Actions runners have restricted
    // multicast routing tables; only local-scope addresses (239.255.0.0/16)
    // are reliably reachable. Avoid 239.255.0.1 (spec default) →
    // y_lo starts at 2.
    let x = 1 + ((raw >> 8) & 0x7F) as u8; // [1, 128]
    let y = 2 + (raw & 0x7F) as u8; // [2, 129]
    let group = Ipv4Addr::new(239, 255, x, y);
    RuntimeConfig {
        tick_period: Duration::from_millis(20),
        spdp_period: Duration::from_millis(100),
        // The spec's defaults are 200 ms and 500 ms; these tests wait for
        // repairs by the tick.
        nack_response_delay: Duration::ZERO,
        reader_heartbeat_response_delay: Duration::ZERO,
        spdp_multicast_group: group,
        ..RuntimeConfig::default()
    }
}

// ---------------------------------------------------------------------
// Domain leasing
// ---------------------------------------------------------------------
//
// The leasing lives in `crates/test-domains` because the resource is the
// whole host's: `zerodds-c-api`, `zerodds-durability-service` and
// `zerodds-routing-service` bind the same ports out of their own test
// binaries, and a helper private to this directory could not arbitrate
// them.

#[allow(unused_imports)]
pub use zerodds_test_domains::{
    DomainLease, MAX_DOMAIN, RESERVED_DOMAINS, domain_port_window, lease_domain, unique_domain,
};

/// Discovery match timeout, adaptive for CI vs. a local dev run.
///
/// Background: `wait_for_matched_*(1, 5s)` is plenty on an unloaded
/// laptop; on a GitHub Actions runner (CI), under
/// `cargo llvm-cov` (instrumentation), or with parallel test
/// threads in the same process, 5s is too tight. Detection: env vars
/// LLVM_PROFILE_FILE (set by llvm-cov), CARGO_LLVM_COV
/// (sometimes), CI (GitHub/GitLab/etc.).
/// RAII wrapper around a DomainParticipant that is cleanly removed
/// from the factory singleton on drop.
///
/// Background: `DomainParticipantFactory::instance()` is, per
/// OMG DDS 1.4 §2.2.2.2, a singleton that holds all created
/// participants in a map (`lookup_participant` spec).
/// Tests that create + drop a participant let it accumulate in the
/// singleton — the Arc refcount stays > 0, runtime threads keep
/// running, and the next tests see 4+ participants in the
/// same process.
///
/// On going out of scope, this guard calls `factory.delete_participant`,
/// which releases the factory reference → Arc count drops → the
/// actual DomainParticipant drop runs → Runtime.shutdown.
pub struct ParticipantGuard {
    inner: Option<zerodds_dcps::DomainParticipant>,
}

impl ParticipantGuard {
    #[must_use]
    pub fn new(p: zerodds_dcps::DomainParticipant) -> Self {
        Self { inner: Some(p) }
    }
}

impl core::ops::Deref for ParticipantGuard {
    type Target = zerodds_dcps::DomainParticipant;
    fn deref(&self) -> &Self::Target {
        // We only release the inner `Some(...)` after drop; before that
        // it is always safe to unwrap.
        self.inner
            .as_ref()
            .expect("ParticipantGuard accessed after drop")
    }
}

impl Drop for ParticipantGuard {
    fn drop(&mut self) {
        if let Some(p) = self.inner.take() {
            let factory = zerodds_dcps::DomainParticipantFactory::instance();
            // DDS 1.4 §2.2.2.2.2.2: the contained entities go first.
            let _ = p.delete_contained_entities();
            let _ = factory.delete_participant(&p);
            // p (DomainParticipant Arc) is dropped here; if the
            // factory reference was the last one, this now leads to
            // ParticipantInner drop → Runtime::shutdown.
        }
    }
}

pub fn match_timeout() -> std::time::Duration {
    let cov = std::env::var_os("LLVM_PROFILE_FILE").is_some()
        || std::env::var_os("CARGO_LLVM_COV").is_some();
    let ci = std::env::var_os("CI").is_some();
    // Coverage instrumentation is ~3-5x slower; plain CI runners
    // (cargo test without llvm-cov) are 1.5-2x slower than a dev laptop.
    if cov {
        std::time::Duration::from_secs(60)
    } else if ci {
        std::time::Duration::from_secs(20)
    } else {
        std::time::Duration::from_secs(5)
    }
}

/// Types the MultiTopic tests join: two topics that share `id`, and the
/// result of their natural join.
pub const ORDER_IDL: &str = "@final struct Order { @key long id; double amount; };";
/// See [`ORDER_IDL`].
pub const CUSTOMER_IDL: &str = "@final struct Customer { @key long id; string<16> country; };";
/// The type of `SELECT * FROM Order`.
pub const ORDER_COPY_IDL: &str = "@final struct OrderCopy { @key long id; double amount; };";
/// The type of `SELECT * FROM Order NATURAL JOIN Customer`.
pub const SALE_IDL: &str =
    "@final struct Sale { @key long id; double amount; string<16> country; };";

/// The type support of the struct `name` of `idl`, whose members and key
/// are read from its `CompleteTypeObject`.
pub fn dynamic_support(
    idl: &str,
    name: &str,
    keyed: bool,
) -> std::sync::Arc<zerodds_dcps::serialized_type::SerializedTypeSupport> {
    let ast = std::sync::Arc::new(
        zerodds_wire_probe::encode::model_from_idl(idl, "multitopic.idl").unwrap(),
    );
    let type_ = zerodds_types::dynamic::DynamicType::find(&ast, name).unwrap();
    let object = zerodds_types::type_object::TypeObject::Complete(
        zerodds_types::from_ast::complete_type_object(&ast, type_.id()).unwrap(),
    )
    .to_bytes_le()
    .unwrap();
    zerodds_dcps::serialized_type::SerializedTypeSupport::from_type_object(name, &object, keyed, 0)
        .unwrap()
}

/// A topic `name` of the type `Order`, which a MultiTopic can read.
pub fn order_topic(
    p: &zerodds_dcps::DomainParticipant,
    name: &str,
) -> zerodds_dcps::Topic<zerodds_types::dynamic::DynamicSample> {
    p.create_dynamic_topic(
        name,
        zerodds_dcps::TopicQos::default(),
        dynamic_support(ORDER_IDL, "Order", true),
    )
    .unwrap()
}

/// `SELECT * FROM <topic>` as a MultiTopic `name` of the type `OrderCopy`.
pub fn order_copy_multitopic(
    p: &zerodds_dcps::DomainParticipant,
    name: &str,
    topic: &str,
) -> zerodds_dcps::MultiTopic<zerodds_types::dynamic::DynamicSample> {
    dynamic_support(ORDER_COPY_IDL, "OrderCopy", true)
        .register_type(p, None)
        .unwrap();
    p.create_dynamic_multitopic(
        name,
        "OrderCopy",
        &format!("SELECT * FROM '{topic}'"),
        Vec::new(),
    )
    .unwrap()
}

/// A struct with a base structure: `id` is inherited.
pub const DERIVED_ORDER_IDL: &str = "@final struct OrderBase { @key long id; }; \
     @final struct DerivedOrder : OrderBase { double amount; };";

/// [`dynamic_support`] for a struct that extends the structs `bases`: their
/// `TypeObject`s are its dependencies.
pub fn dynamic_support_with_bases(
    idl: &str,
    name: &str,
    bases: &[&str],
    keyed: bool,
) -> std::sync::Arc<zerodds_dcps::serialized_type::SerializedTypeSupport> {
    let ast = std::sync::Arc::new(
        zerodds_wire_probe::encode::model_from_idl(idl, "multitopic.idl").unwrap(),
    );
    let bytes_of = |type_name: &str| {
        let type_ = zerodds_types::dynamic::DynamicType::find(&ast, type_name).unwrap();
        zerodds_types::type_object::TypeObject::Complete(
            zerodds_types::from_ast::complete_type_object(&ast, type_.id()).unwrap(),
        )
        .to_bytes_le()
        .unwrap()
    };
    let dependencies: Vec<Vec<u8>> = bases.iter().map(|base| bytes_of(base)).collect();
    zerodds_dcps::serialized_type::SerializedTypeSupport::from_type_objects(
        name,
        &bytes_of(name),
        &dependencies,
        keyed,
        0,
    )
    .unwrap()
}
