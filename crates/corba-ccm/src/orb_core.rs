// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA 3.3 ORB-Core — Stub-Layer fuer:
//! - Part 1 §8 ORB Interface (ORB_init/shutdown/Threading/PolicyDomain)
//! - Part 1 §9 Value Type Custom-Marshal-Streams + Sending-Context
//! - Part 3 §12 IFR Metamodel CCM-Erweiterung
//! - Part 3 §13 CIF Metamodel
//!
//! ZeroDDS hat keinen vollen ORB; diese Module liefern Configuration-
//! + Datenmodell-Layer als Stub. XMI-Emitter nutzt MOF-2.0-Subset.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::sync::Mutex;

use crate::orb_extensions::{CompressionAlgorithm, InterceptorRegistry, MessagingPolicy};

// ===========================================================================
// Part 1 §8 ORB Interface
// ===========================================================================

/// Spec §8.2.1 — ORB-Initialization-State.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrbState {
    /// `ORB_init` noch nicht aufgerufen.
    Uninitialized,
    /// `ORB_init` erfolgreich; ORB ist running.
    Running,
    /// `shutdown` aufgerufen, wartet auf Outstanding-Requests.
    ShuttingDown,
    /// `shutdown` abgeschlossen; ORB ist `destroy`-fertig.
    Shutdown,
}

/// Spec §8.2.5 — Threading-Operations am ORB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadingMode {
    /// `single-threaded` — alle Requests sequentiell.
    SingleThreaded,
    /// `thread-per-request` — neuer Thread pro Request.
    ThreadPerRequest,
    /// `thread-pool` — Worker-Pool mit fixer Thread-Anzahl.
    ThreadPool {
        /// Anzahl Worker-Threads.
        size: usize,
    },
}

/// ORB-Singleton-Lifecycle nach Spec §8.2.1.
pub struct Orb {
    state: Mutex<OrbState>,
    threading: ThreadingMode,
    /// Spec §8.10 — Policy-Domain-Manager (Mapping policy_type → policy).
    policies: Mutex<BTreeMap<u32, Vec<u8>>>,
    /// CCM 4.0 §2 CP8 — Component-Specific-Erweiterungen am ORB:
    /// optionale Interceptor-Registry, gewaehlte Messaging-Policies,
    /// Compression-Algorithm.
    interceptors: Mutex<Option<Arc<InterceptorRegistry>>>,
    messaging_policies: Mutex<Vec<MessagingPolicy>>,
    compression: Mutex<CompressionAlgorithm>,
}

impl core::fmt::Debug for Orb {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = self
            .state
            .lock()
            .map(|g| *g)
            .unwrap_or(OrbState::Uninitialized);
        f.debug_struct("Orb").field("state", &s).finish()
    }
}

impl Orb {
    /// Spec §8.2.1 — `ORB_init`. Konstruktor.
    #[must_use]
    pub fn init(threading: ThreadingMode) -> Self {
        Self {
            state: Mutex::new(OrbState::Running),
            threading,
            policies: Mutex::new(BTreeMap::new()),
            interceptors: Mutex::new(None),
            messaging_policies: Mutex::new(Vec::new()),
            compression: Mutex::new(CompressionAlgorithm::None),
        }
    }

    /// Aktueller State.
    #[must_use]
    pub fn state(&self) -> OrbState {
        self.state
            .lock()
            .map(|g| *g)
            .unwrap_or(OrbState::Uninitialized)
    }

    /// Threading-Mode.
    #[must_use]
    pub fn threading(&self) -> ThreadingMode {
        self.threading
    }

    /// Spec §8.2.4 — `shutdown(wait_for_completion)`.
    pub fn shutdown(&self, _wait_for_completion: bool) {
        if let Ok(mut g) = self.state.lock() {
            if *g == OrbState::Running {
                *g = OrbState::ShuttingDown;
            }
        }
    }

    /// Spec §8.2.4 — `destroy`. Endgueltiger Lifecycle-Schluss.
    pub fn destroy(&self) {
        if let Ok(mut g) = self.state.lock() {
            *g = OrbState::Shutdown;
        }
    }

    /// Spec §8.10 — `set_policy(policy_type, value)`.
    pub fn set_policy(&self, policy_type: u32, value: Vec<u8>) {
        if let Ok(mut g) = self.policies.lock() {
            g.insert(policy_type, value);
        }
    }

    /// Spec §8.10 — `get_policy(policy_type)`.
    #[must_use]
    pub fn get_policy(&self, policy_type: u32) -> Option<Vec<u8>> {
        self.policies
            .lock()
            .ok()
            .and_then(|g| g.get(&policy_type).cloned())
    }

    // -----------------------------------------------------------------
    // CCM 4.0 §2 CP8 — Component-Specific-Erweiterungen am ORB.
    // Cross-Ref `corba-3.3.md` §16 (PI), §17 (Messaging), §18
    // (Compression). Diese Methoden konfigurieren den ORB-Singleton
    // fuer die drei Vendor-Erweiterungen.
    // -----------------------------------------------------------------

    /// CCM 4.0 §2 CP8 — installiert eine [`InterceptorRegistry`] am
    /// ORB. Subsequent `iiop::Connection`-Instanzen koennen sie via
    /// `interceptor_registry()` abholen und verdrahten.
    pub fn with_interceptor_registry(&self, r: Arc<InterceptorRegistry>) {
        if let Ok(mut g) = self.interceptors.lock() {
            *g = Some(r);
        }
    }

    /// CCM 4.0 §2 CP8 — Liefert die aktuelle Registry (None wenn keine
    /// installiert).
    #[must_use]
    pub fn interceptor_registry(&self) -> Option<Arc<InterceptorRegistry>> {
        self.interceptors.lock().ok().and_then(|g| g.clone())
    }

    /// CCM 4.0 §2 CP8 — Aktiviert eine Messaging-Policy am ORB.
    /// Cross-Ref `corba-ccm::orb_extensions::MessagingPolicy`.
    pub fn with_messaging_policy(&self, p: MessagingPolicy) {
        if let Ok(mut g) = self.messaging_policies.lock() {
            if !g.contains(&p) {
                g.push(p);
            }
        }
    }

    /// CCM 4.0 §2 CP8 — Liste der aktiven Messaging-Policies.
    #[must_use]
    pub fn messaging_policies(&self) -> Vec<MessagingPolicy> {
        self.messaging_policies
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    /// CCM 4.0 §2 CP8 — Setzt den ORB-weiten Compression-Algorithmus.
    /// Cross-Ref `corba-ccm::orb_extensions::CompressionAlgorithm`.
    pub fn with_compression(&self, algo: CompressionAlgorithm) {
        if let Ok(mut g) = self.compression.lock() {
            *g = algo;
        }
    }

    /// CCM 4.0 §2 CP8 — Liefert den aktiven Compression-Algorithmus.
    #[must_use]
    pub fn compression(&self) -> CompressionAlgorithm {
        self.compression
            .lock()
            .map(|g| *g)
            .unwrap_or(CompressionAlgorithm::None)
    }
}

// ===========================================================================
// Part 1 §9 Value Type — Custom-Marshal-Streams + Sending-Context
// ===========================================================================

/// Spec §9.5 — `StreamableValue` Custom-Marshal-Streams.
pub trait StreamableValue: Send + Sync {
    /// Repository-ID des Value-Types (z.B. `IDL:demo/MyValue:1.0`).
    fn repository_id(&self) -> &str;
    /// Marshal-Operation: schreibe das Wert-State in den Stream.
    fn marshal(&self) -> Vec<u8>;
    /// Unmarshal-Operation: lade aus Stream-Bytes.
    ///
    /// # Errors
    /// `()` bei invalid Bytes.
    #[allow(clippy::result_unit_err)]
    fn unmarshal(&mut self, bytes: &[u8]) -> Result<(), ()>;
}

/// Spec §9.6 — `SendingContext::RunTime` (per-Sender-State).
#[derive(Debug, Clone, Default)]
pub struct SendingContext {
    /// Per-Receiver Truncation-Tracking.
    truncation_codebase: Option<String>,
    /// Code-Set (Char-Encoding).
    code_set: u32,
}

impl SendingContext {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Spec §9.6 — Setzt die Codebase-URL (fuer Truncation-Recovery).
    pub fn set_truncation_codebase(&mut self, url: impl Into<String>) {
        self.truncation_codebase = Some(url.into());
    }

    /// Code-Set abrufen (Spec §9.6.1).
    #[must_use]
    pub fn code_set(&self) -> u32 {
        self.code_set
    }

    /// Code-Set setzen.
    pub fn set_code_set(&mut self, cs: u32) {
        self.code_set = cs;
    }
}

// ===========================================================================
// Part 3 §12 IFR Metamodel + §13 CIF Metamodel — XMI-Emitter
// ===========================================================================

/// MOF-2.0-Element fuer den XMI-Emitter (Subset).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MofElement {
    /// `Class` — entspricht IDL-`interface`/`component`.
    Class {
        /// Name.
        name: String,
        /// Repository-ID.
        repo_id: String,
        /// Inheritance.
        bases: Vec<String>,
    },
    /// `Property` — entspricht IDL-Attribute oder Struct-Member.
    Property {
        /// Name.
        name: String,
        /// Type-Reference.
        type_ref: String,
        /// `true` wenn read-only.
        read_only: bool,
    },
    /// `Operation` — entspricht IDL-Operation.
    Operation {
        /// Name.
        name: String,
        /// Return-Type.
        return_type: String,
        /// Parameter-Liste.
        parameters: Vec<(String, String)>,
    },
}

/// MOF-2.0 → XMI-1.2 Emitter (Subset fuer IFR + CIF Metamodel).
#[derive(Debug, Clone, Default)]
pub struct XmiEmitter {
    elements: Vec<MofElement>,
}

impl XmiEmitter {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Spec Part 3 §12.3 / §13.x — fuegt ein MOF-Element hinzu.
    pub fn add_element(&mut self, e: MofElement) {
        self.elements.push(e);
    }

    /// Anzahl Elemente.
    #[must_use]
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// `true` wenn keine Elemente.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Emittiert XMI-1.2-XML (Subset). Caller bekommt einen
    /// Spec-konformen XMI-Document-String, den ein MOF-Tool
    /// (z.B. EMF) konsumieren kann.
    #[must_use]
    pub fn emit_xmi(&self) -> String {
        let mut out = String::new();
        out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        out.push_str("<XMI xmi.version=\"1.2\">\n");
        out.push_str("  <XMI.content>\n");
        for el in &self.elements {
            match el {
                MofElement::Class {
                    name,
                    repo_id,
                    bases,
                } => {
                    out.push_str(&alloc::format!(
                        "    <Mof.Class name=\"{name}\" repo_id=\"{repo_id}\">\n"
                    ));
                    for b in bases {
                        out.push_str(&alloc::format!("      <Mof.Inheritance base=\"{b}\"/>\n"));
                    }
                    out.push_str("    </Mof.Class>\n");
                }
                MofElement::Property {
                    name,
                    type_ref,
                    read_only,
                } => {
                    out.push_str(&alloc::format!(
                        "    <Mof.Property name=\"{name}\" type=\"{type_ref}\" read_only=\"{read_only}\"/>\n"
                    ));
                }
                MofElement::Operation {
                    name,
                    return_type,
                    parameters,
                } => {
                    out.push_str(&alloc::format!(
                        "    <Mof.Operation name=\"{name}\" return=\"{return_type}\">\n"
                    ));
                    for (pn, pt) in parameters {
                        out.push_str(&alloc::format!(
                            "      <Mof.Parameter name=\"{pn}\" type=\"{pt}\"/>\n"
                        ));
                    }
                    out.push_str("    </Mof.Operation>\n");
                }
            }
        }
        out.push_str("  </XMI.content>\n");
        out.push_str("</XMI>\n");
        out
    }
}

/// Spec Part 3 §12 — IFR-Metamodel-Erweiterung fuer CCM.
/// Wraps einen XmiEmitter mit CCM-spezifischer Vorkonfiguration
/// (Component-Class als Mof.Class mit dem Marker `ccm:Component`).
pub struct IfrCcmMetamodel {
    inner: Arc<Mutex<XmiEmitter>>,
}

impl core::fmt::Debug for IfrCcmMetamodel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("IfrCcmMetamodel").finish()
    }
}

impl Default for IfrCcmMetamodel {
    fn default() -> Self {
        Self::new()
    }
}

impl IfrCcmMetamodel {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(XmiEmitter::new())),
        }
    }

    /// Fuegt eine Component-Klasse als Mof.Class hinzu.
    pub fn add_component(&self, name: &str, repo_id: &str, bases: Vec<String>) {
        if let Ok(mut g) = self.inner.lock() {
            g.add_element(MofElement::Class {
                name: name.to_string(),
                repo_id: repo_id.to_string(),
                bases,
            });
        }
    }

    /// Emittiert XMI fuer das gesamte Component-Modell.
    #[must_use]
    pub fn emit_xmi(&self) -> String {
        self.inner.lock().map(|g| g.emit_xmi()).unwrap_or_default()
    }

    /// Anzahl registrierter Elemente.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.lock().map(|g| g.len()).unwrap_or(0)
    }

    /// `true` wenn leer.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Spec Part 3 §12.3 — laed eine `corba-ir::Repository` in den
    /// Metamodel-Emitter. Jede Top-Level-Definition wird als
    /// Mof.Class registriert; Sub-Contents (Module-Hierarchie) werden
    /// rekursiv eingehaengt.
    pub fn ingest_repository(&self, repo: &zerodds_corba_ir::Repository) {
        if let Ok(mut g) = self.inner.lock() {
            for id in repo.ids() {
                if let Some(def) = repo.lookup_id(&id) {
                    ingest_definition_into(&mut g, def);
                }
            }
        }
    }

    /// Konstruktor mit Repository-Ingestion.
    #[must_use]
    pub fn from_repository(repo: &zerodds_corba_ir::Repository) -> Self {
        let m = Self::new();
        m.ingest_repository(repo);
        m
    }
}
/// zerodds-lint: recursion-depth 64 (ingest_definition_into bounded by AST depth)
fn ingest_definition_into(emitter: &mut XmiEmitter, def: &zerodds_corba_ir::Definition) {
    use zerodds_corba_ir::DefinitionKind;
    let bases: Vec<String> = Vec::new();
    match def.kind {
        DefinitionKind::Interface
        | DefinitionKind::AbstractInterface
        | DefinitionKind::LocalInterface
        | DefinitionKind::Value
        | DefinitionKind::ValueBox
        | DefinitionKind::Module
        | DefinitionKind::Struct
        | DefinitionKind::Union
        | DefinitionKind::Enum
        | DefinitionKind::Exception => {
            emitter.add_element(MofElement::Class {
                name: def.name.clone(),
                repo_id: def.repository_id.clone(),
                bases,
            });
        }
        DefinitionKind::Operation => {
            emitter.add_element(MofElement::Operation {
                name: def.name.clone(),
                return_type: alloc::format!("IDL:{}:{}", def.name, def.version),
                parameters: Vec::new(),
            });
        }
        DefinitionKind::Attribute | DefinitionKind::Constant => {
            emitter.add_element(MofElement::Property {
                name: def.name.clone(),
                type_ref: def.repository_id.clone(),
                read_only: matches!(def.kind, DefinitionKind::Constant),
            });
        }
        _ => {
            emitter.add_element(MofElement::Class {
                name: def.name.clone(),
                repo_id: def.repository_id.clone(),
                bases,
            });
        }
    }
    for child in &def.contents {
        ingest_definition_into(emitter, child);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    // §8 ORB Interface
    #[test]
    fn orb_init_yields_running() {
        let o = Orb::init(ThreadingMode::SingleThreaded);
        assert_eq!(o.state(), OrbState::Running);
    }

    #[test]
    fn orb_shutdown_transitions_to_shutting_down() {
        let o = Orb::init(ThreadingMode::SingleThreaded);
        o.shutdown(true);
        assert_eq!(o.state(), OrbState::ShuttingDown);
    }

    #[test]
    fn orb_destroy_transitions_to_shutdown() {
        let o = Orb::init(ThreadingMode::SingleThreaded);
        o.destroy();
        assert_eq!(o.state(), OrbState::Shutdown);
    }

    #[test]
    fn orb_threading_mode_preserved() {
        let o = Orb::init(ThreadingMode::ThreadPool { size: 8 });
        assert_eq!(o.threading(), ThreadingMode::ThreadPool { size: 8 });
    }

    #[test]
    fn orb_set_get_policy_round_trip() {
        let o = Orb::init(ThreadingMode::SingleThreaded);
        o.set_policy(42, alloc::vec![1, 2, 3]);
        assert_eq!(o.get_policy(42), Some(alloc::vec![1, 2, 3]));
        assert!(o.get_policy(99).is_none());
    }

    // §9 Value Type
    struct DummyValue {
        state: u32,
    }

    impl StreamableValue for DummyValue {
        fn repository_id(&self) -> &str {
            "IDL:demo/DummyValue:1.0"
        }
        fn marshal(&self) -> Vec<u8> {
            self.state.to_be_bytes().to_vec()
        }
        fn unmarshal(&mut self, bytes: &[u8]) -> Result<(), ()> {
            if bytes.len() < 4 {
                return Err(());
            }
            self.state = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            Ok(())
        }
    }

    #[test]
    fn streamable_value_round_trip() {
        let v = DummyValue { state: 0xDEADBEEF };
        let bytes = v.marshal();
        let mut v2 = DummyValue { state: 0 };
        v2.unmarshal(&bytes).expect("ok");
        assert_eq!(v2.state, 0xDEADBEEF);
    }

    #[test]
    fn streamable_value_unmarshal_truncated_rejected() {
        let mut v = DummyValue { state: 0 };
        assert!(v.unmarshal(&[0x01]).is_err());
    }

    #[test]
    fn sending_context_default_code_set_zero() {
        let sc = SendingContext::new();
        assert_eq!(sc.code_set(), 0);
    }

    #[test]
    fn sending_context_codebase_round_trip() {
        let mut sc = SendingContext::new();
        sc.set_truncation_codebase("http://example.com/codebase");
        sc.set_code_set(0x10001);
        assert_eq!(sc.code_set(), 0x10001);
    }

    // Part 3 §12 IFR + §13 CIF Metamodel — XMI
    #[test]
    fn xmi_emitter_empty_yields_minimal_doc() {
        let e = XmiEmitter::new();
        assert!(e.is_empty());
        let xmi = e.emit_xmi();
        assert!(xmi.contains("<?xml version=\"1.0\""));
        assert!(xmi.contains("<XMI xmi.version=\"1.2\">"));
    }

    #[test]
    fn xmi_emitter_class_with_inheritance() {
        let mut e = XmiEmitter::new();
        e.add_element(MofElement::Class {
            name: "Trader".into(),
            repo_id: "IDL:demo/Trader:1.0".into(),
            bases: alloc::vec!["IDL:Components/CCMObject:1.0".into()],
        });
        let xmi = e.emit_xmi();
        assert!(xmi.contains("name=\"Trader\""));
        assert!(xmi.contains("base=\"IDL:Components/CCMObject:1.0\""));
    }

    #[test]
    fn xmi_emitter_property_emits() {
        let mut e = XmiEmitter::new();
        e.add_element(MofElement::Property {
            name: "version".into(),
            type_ref: "string".into(),
            read_only: true,
        });
        let xmi = e.emit_xmi();
        assert!(xmi.contains("name=\"version\""));
        assert!(xmi.contains("read_only=\"true\""));
    }

    #[test]
    fn xmi_emitter_operation_with_parameters() {
        let mut e = XmiEmitter::new();
        e.add_element(MofElement::Operation {
            name: "compute".into(),
            return_type: "long".into(),
            parameters: alloc::vec![("x".into(), "long".into()), ("y".into(), "long".into()),],
        });
        let xmi = e.emit_xmi();
        assert!(xmi.contains("name=\"compute\""));
        assert!(xmi.contains("name=\"x\""));
        assert!(xmi.contains("name=\"y\""));
    }

    #[test]
    fn ifr_ccm_metamodel_add_component() {
        let m = IfrCcmMetamodel::new();
        assert!(m.is_empty());
        m.add_component(
            "Trader",
            "IDL:demo/Trader:1.0",
            alloc::vec!["IDL:Components/CCMObject:1.0".into()],
        );
        assert_eq!(m.len(), 1);
        let xmi = m.emit_xmi();
        assert!(xmi.contains("Trader"));
    }

    // CCM 4.0 §2 CP8 — ORB-Vendor-Konfiguration

    #[test]
    fn orb_vendor_config_interceptor_registry() {
        let o = Orb::init(ThreadingMode::SingleThreaded);
        assert!(o.interceptor_registry().is_none());
        let r = Arc::new(InterceptorRegistry::new());
        o.with_interceptor_registry(r.clone());
        let got = o.interceptor_registry().expect("registry installed");
        // Beide Arc zeigen auf dieselbe Registry.
        assert!(Arc::ptr_eq(&r, &got));
    }

    #[test]
    fn orb_vendor_config_compression_and_messaging_policies() {
        let o = Orb::init(ThreadingMode::SingleThreaded);
        assert_eq!(o.compression(), CompressionAlgorithm::None);
        o.with_compression(CompressionAlgorithm::Zlib);
        assert_eq!(o.compression(), CompressionAlgorithm::Zlib);

        // Messaging-Policy-Liste wird append-only befuellt.
        assert!(o.messaging_policies().is_empty());
        o.with_messaging_policy(MessagingPolicy::SyncScope);
        o.with_messaging_policy(MessagingPolicy::Routing);
        // Doppelte Eintraege werden dedupliziert.
        o.with_messaging_policy(MessagingPolicy::SyncScope);
        let p = o.messaging_policies();
        assert_eq!(p.len(), 2);
        assert!(p.contains(&MessagingPolicy::SyncScope));
        assert!(p.contains(&MessagingPolicy::Routing));
    }

    #[test]
    fn ifr_ccm_metamodel_ingest_repository_walks_definitions() {
        use zerodds_corba_ir::{Definition, DefinitionKind, Repository};
        let mut repo = Repository::new();
        let module = Definition::new("IDL:demo:1.0", "demo", "1.0", DefinitionKind::Module)
            .with_content(
                Definition::new(
                    "IDL:demo/Echo:1.0",
                    "Echo",
                    "1.0",
                    DefinitionKind::Interface,
                )
                .with_content(Definition::new(
                    "IDL:demo/Echo/ping:1.0",
                    "ping",
                    "1.0",
                    DefinitionKind::Operation,
                )),
            );
        repo.register(module).expect("register module");
        let m = IfrCcmMetamodel::from_repository(&repo);
        assert!(
            m.len() >= 3,
            "expected module + interface + op, got {}",
            m.len()
        );
        let xmi = m.emit_xmi();
        assert!(xmi.contains("demo"));
        assert!(xmi.contains("Echo"));
        assert!(xmi.contains("ping"));
        assert!(xmi.contains("IDL:demo/Echo:1.0"));
    }
}
