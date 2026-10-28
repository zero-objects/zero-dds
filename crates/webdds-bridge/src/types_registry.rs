// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Type resolution: turns a config-named IDL file, or a type a peer's
//! `DcpsRuntime` TypeLookup discovers, into a
//! [`zerodds_dcps::serialized_type::SerializedTypeSupport`] plus the
//! reflective [`zerodds_types::dynamic::DynamicType`] view the XML body
//! converter (`crate::dynxml`) walks.
//!
//! Two sources, per the task's requirement that types come "from IDL/XML
//! type files named in the config or from TypeLookup of discovered
//! types":
//!
//! * [`TypeRegistry::load_idl_file`] — parses a `.idl` file named in the
//!   daemon config through the same front end `zerodds-dcps` itself uses
//!   for its own tests (`zerodds_wire_probe::encode::model_from_idl`),
//!   derives the type's `CompleteTypeObject`
//!   (`zerodds_types::from_ast::complete_type_object`), and builds a
//!   [`SerializedTypeSupport`] from it. Fully working.
//! * [`TypeRegistry::resolve_discovered`] — for a type this daemon never
//!   read from a file: given the `EquivalenceHash` a peer's SEDP
//!   announced, reads the `CompleteTypeObject` (and its transitive
//!   dependencies) back out of the participant's own TypeLookup registry
//!   (`DcpsRuntime::type_lookup_server`, a public field) once
//!   `send_type_lookup_request` has resolved it, re-encodes the objects
//!   to bytes and feeds them through the same public
//!   `SerializedTypeSupport::from_type_objects` constructor the IDL path
//!   uses.
//! * [`TypeRegistry::resolve_named`] — waits (bounded, event-driven on
//!   [`zerodds_types::resolve::TypeRegistry::generation`], no polling
//!   sleep) for a topic's type to resolve via TypeLookup when a request
//!   names a topic this daemon does not know the type of yet — see
//!   `crate::gateway`'s use of it.
//!
//! `keyed`/`extensibility` are never brute-forced: XTypes 1.3 §7.3.4.5
//! states a struct's/union's extensibility in its own `struct_flags`/
//! `union_flags` (`IS_FINAL`/`IS_APPENDABLE`/`IS_MUTABLE`) and a
//! member's `@key` in its `member_flags` (`IS_KEY`, §7.3.4.6) — both
//! read directly off the `CompleteTypeObject` (via the model
//! `ModelFromTypeObjects` builds from it) in `resolve_from_complete`,
//! the same way `SerializedTypeSupport`'s own (private)
//! `from_decoded_objects` derives them internally.
//!
//! **DDS-XTYPES XML type-definition files.** `zerodds_xml::typeobject_bridge`
//! now builds a `CompleteTypeObject` from a `<types>` document
//! (`xml_type_to_complete_typeobject`, XTypes 1.3 §7.3.4.5/§Annex A) —
//! [`TypeRegistry::load_xml_file`] uses it the same way
//! [`TypeRegistry::load_idl_file`] uses the IDL front end.
//!
//! **A fifth source that is not a file at all.**
//! [`TypeRegistry::resolve_synthetic`] registers a `CompleteTypeObject`
//! this crate built itself, rather than read from anywhere — the
//! `In`/`Out` structs `crate::rpc_dispatch` synthesizes for a business
//! `interface` operation's parameters (DDS-RPC 1.0
//! §7.5.1.1.4/§7.5.1.1.5). It goes through the same
//! `resolve_from_complete` as every other source, so a synthetic
//! type's `keyed`/`extensibility` is read off itself exactly like a
//! discovered or file-loaded one, never asserted by the caller.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use zerodds_ast::Ast;
use zerodds_dcps::runtime::DcpsRuntime;
use zerodds_dcps::serialized_type::SerializedTypeSupport;
use zerodds_types::EquivalenceHash;
use zerodds_types::dynamic::DynamicType;
use zerodds_types::type_object::TypeObject;

/// One resolved type: the wire codec (`support`) plus the reflective
/// view (`dynamic_type`) the XML converter walks. Both are built from
/// the same `CompleteTypeObject`, so they always agree on shape.
#[derive(Clone)]
pub struct ResolvedType {
    /// Wire codec + key-hash support (`crates/dcps`'s runtime-described
    /// topic path).
    pub support: Arc<SerializedTypeSupport>,
    /// Reflective member/kind view for `dynxml`.
    pub dynamic_type: DynamicType,
    /// `true` if the type carries an `@key`.
    pub keyed: bool,
}

impl std::fmt::Debug for ResolvedType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedType")
            .field("type_name", &self.dynamic_type.name())
            .field("keyed", &self.keyed)
            .finish_non_exhaustive()
    }
}

/// Error resolving a type.
#[derive(Debug)]
pub enum TypeResolveError {
    /// The IDL file could not be read.
    Io(std::io::Error),
    /// The IDL text did not parse, or the named type is not in it.
    Idl(String),
    /// `CompleteTypeObject` derivation, or `SerializedTypeSupport`
    /// construction, refused the type.
    Model(String),
    /// The named type is not in the loaded model.
    NotFound(String),
    /// The type is not yet resolved via TypeLookup (still pending, or
    /// TypeLookup was never asked for it).
    NotYetResolved,
    /// No discovered publication or subscription names this topic, so
    /// there is no `TypeIdentifier` to even ask a peer about.
    NoDiscoveredEndpoint(String),
    /// A `getTypes` request was sent and awaited, but the type never
    /// resolved within the bound.
    TimedOut(String),
}

impl std::fmt::Display for TypeResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io: {e}"),
            Self::Idl(m) => write!(f, "idl: {m}"),
            Self::Model(m) => write!(f, "type model: {m}"),
            Self::NotFound(name) => write!(f, "type `{name}` not found"),
            Self::NotYetResolved => f.write_str("type not yet resolved via TypeLookup"),
            Self::NoDiscoveredEndpoint(topic) => {
                write!(
                    f,
                    "no discovered publication or subscription names topic `{topic}`"
                )
            }
            Self::TimedOut(name) => write!(
                f,
                "type `{name}` never resolved via TypeLookup within the bound"
            ),
        }
    }
}

impl std::error::Error for TypeResolveError {}

/// Type registry: caches every type resolved so far, by name.
#[derive(Default)]
pub struct TypeRegistry {
    by_name: Mutex<HashMap<String, ResolvedType>>,
}

impl TypeRegistry {
    /// Empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A previously resolved type, if any.
    #[must_use]
    pub fn get(&self, type_name: &str) -> Option<ResolvedType> {
        self.by_name
            .lock()
            .ok()
            .and_then(|m| m.get(type_name).cloned())
    }

    /// Registers an already-resolved type under its name (idempotent —
    /// a later call with the same name replaces the entry, since the
    /// daemon reloads its config on SIGHUP).
    pub fn insert(&self, type_name: &str, resolved: ResolvedType) {
        if let Ok(mut m) = self.by_name.lock() {
            m.insert(type_name.to_string(), resolved);
        }
    }

    /// Parses `idl_path`, resolves `type_name` in it, and registers the
    /// result. `type_name` is the IDL scoped name (`Mod::Type`), which
    /// is also the DDS `type_name` samples of this topic carry on the
    /// wire.
    ///
    /// # Errors
    /// See [`TypeResolveError`].
    pub fn load_idl_file(
        &self,
        idl_path: &std::path::Path,
        type_name: &str,
    ) -> Result<ResolvedType, TypeResolveError> {
        let source = std::fs::read_to_string(idl_path).map_err(TypeResolveError::Io)?;
        let ast = Arc::new(
            zerodds_wire_probe::encode::model_from_idl(&source, &idl_path.to_string_lossy())
                .map_err(|e| TypeResolveError::Idl(format!("{e}")))?,
        );
        let resolved = resolve_from_ast(&ast, type_name)?;
        self.insert(type_name, resolved.clone());
        Ok(resolved)
    }

    /// Parses `xml_path` as a DDS-XML 1.0 `<types>`/`<dds>` document
    /// (`zerodds_xml::parse_type_libraries`), resolves `type_name` in
    /// it (a top-level, or module-flattened `Outer::Inner`, name) and
    /// registers the result. Every other type the library declares is
    /// offered as a dependency (`SerializedTypeSupport` only uses what
    /// it needs) — the same "offer the whole model" shape
    /// [`Self::load_idl_file`] uses for the IDL front end.
    ///
    /// # Errors
    /// [`TypeResolveError::Io`] if the file cannot be read.
    /// [`TypeResolveError::Idl`] if it does not parse as DDS-XML, or
    /// declares no `<types>` library at all. [`TypeResolveError::NotFound`]
    /// if `type_name` is not among the library's (flattened) type names.
    /// [`TypeResolveError::Model`] if the bridge to `CompleteTypeObject`
    /// refuses a construct (`zerodds_xml::typeobject_bridge`'s
    /// documented gaps — a named sequence/array typedef, for one).
    pub fn load_xml_file(
        &self,
        xml_path: &std::path::Path,
        type_name: &str,
    ) -> Result<ResolvedType, TypeResolveError> {
        let source = std::fs::read_to_string(xml_path).map_err(TypeResolveError::Io)?;
        let libraries = zerodds_xml::parse_type_libraries(&source)
            .map_err(|e| TypeResolveError::Idl(format!("{e}")))?;
        let mut objects: HashMap<String, zerodds_types::CompleteTypeObject> = HashMap::new();
        for lib in &libraries {
            let bridged = zerodds_xml::bridge_library_complete(lib)
                .map_err(|e| TypeResolveError::Model(format!("{e}")))?;
            objects.extend(bridged);
        }
        let Some(root) = objects.remove(type_name) else {
            return Err(TypeResolveError::NotFound(format!(
                "{type_name} (not declared in {})",
                xml_path.display()
            )));
        };
        let dependencies: Vec<zerodds_types::CompleteTypeObject> = objects.into_values().collect();
        let resolved = resolve_from_complete(type_name, root, dependencies)?;
        self.insert(type_name, resolved.clone());
        Ok(resolved)
    }

    /// Resolves `type_name` from a discovered `hash` once the
    /// participant's `DcpsRuntime` TypeLookup registry holds its
    /// `CompleteTypeObject` (own `register_type_object` calls,
    /// `getTypes` replies from a peer, or `send_type_lookup_request`
    /// having completed). Caches the result under `type_name`.
    ///
    /// # Errors
    /// [`TypeResolveError::NotYetResolved`] if the registry does not
    /// (yet) hold the hash's complete object — the caller should retry
    /// after issuing/awaiting `DcpsRuntime::send_type_lookup_request`.
    pub fn resolve_discovered(
        &self,
        runtime: &DcpsRuntime,
        type_name: &str,
        hash: EquivalenceHash,
    ) -> Result<ResolvedType, TypeResolveError> {
        let (root, dependencies) = {
            let server = runtime
                .type_lookup_server
                .lock()
                .map_err(|_| TypeResolveError::Model("type_lookup_server poisoned".into()))?;
            let Some(root) = server.registry.get_complete(&hash).cloned() else {
                return Err(TypeResolveError::NotYetResolved);
            };
            let dep_hashes = server
                .registry
                .transitive_dependencies(&hash, zerodds_types::resolve::DEFAULT_MAX_RESOLVE_NODES);
            let mut deps = Vec::with_capacity(dep_hashes.len());
            for dep_hash in &dep_hashes {
                let Some(dep) = server.registry.get_complete(dep_hash).cloned() else {
                    return Err(TypeResolveError::NotYetResolved);
                };
                deps.push(dep);
            }
            (root, deps)
        };
        let resolved = resolve_from_complete(type_name, root, dependencies)?;
        self.insert(type_name, resolved.clone());
        Ok(resolved)
    }

    /// Resolves `type_name` for `topic_name` automatically: if a
    /// discovered publication or subscription names `topic_name`, its
    /// SEDP-announced `TypeIdentifier` is asked for from the peer that
    /// owns it (`DcpsRuntime::send_type_lookup_request`) and awaited,
    /// bounded by `timeout`. The wait is generation-driven, not a bare
    /// sleep loop: [`zerodds_types::resolve::TypeRegistry::generation`]
    /// is read every tick, and the (cheap) resolve attempt against the
    /// registry only re-runs when it has moved since the last one —
    /// the same short-circuit `DcpsRuntime`'s own receive path uses
    /// (`UserReaderSlot::resolve_key_hash_support`) to avoid rebuilding
    /// a model on every tick a reply has not (yet) landed.
    ///
    /// A response arrives on the receive thread via a callback that
    /// only ever updates the registry (`DcpsRuntime` has no dedicated
    /// wake signal for a TypeLookup reply the way `WaitSet`'s samples
    /// do) — this is the boundedly-polled substitute for a
    /// zero-poll wait; see the crate README "Open points".
    ///
    /// # Errors
    /// [`TypeResolveError::NoDiscoveredEndpoint`] if nothing discovered
    /// names `topic_name` at all (there is then no `TypeIdentifier` to
    /// even ask about). [`TypeResolveError::TimedOut`] if a request was
    /// sent and awaited but the type never resolved within `timeout`.
    pub fn resolve_named(
        &self,
        runtime: &DcpsRuntime,
        topic_name: &str,
        type_name: &str,
        timeout: core::time::Duration,
    ) -> Result<ResolvedType, TypeResolveError> {
        let endpoint = runtime
            .discovered_publication_endpoints()
            .into_iter()
            .chain(runtime.discovered_subscription_endpoints())
            .find(|e| e.topic_name == topic_name)
            .ok_or_else(|| TypeResolveError::NoDiscoveredEndpoint(topic_name.to_string()))?;

        let zerodds_types::TypeIdentifier::EquivalenceHashComplete(hash) =
            endpoint.type_identifier.clone()
        else {
            return Err(TypeResolveError::Model(format!(
                "topic `{topic_name}`'s discovered TypeIdentifier is not the \
                 EquivalenceHashComplete form this daemon can resolve"
            )));
        };

        match self.resolve_discovered(runtime, type_name, hash) {
            Ok(r) => return Ok(r),
            Err(TypeResolveError::NotYetResolved) => {}
            Err(e) => return Err(e),
        }

        let mut peer_prefix = [0u8; 12];
        peer_prefix.copy_from_slice(&endpoint.endpoint_guid[..12]);
        let peer = zerodds_rtps::wire_types::GuidPrefix(peer_prefix);
        let _ = runtime.send_type_lookup_request(peer, &[endpoint.type_identifier.clone()]);

        let deadline = std::time::Instant::now() + timeout;
        let mut last_generation = current_generation(runtime);
        loop {
            if std::time::Instant::now() >= deadline {
                return Err(TypeResolveError::TimedOut(type_name.to_string()));
            }
            std::thread::sleep(core::time::Duration::from_millis(10));
            let generation = current_generation(runtime);
            if generation == last_generation {
                continue;
            }
            last_generation = generation;
            match self.resolve_discovered(runtime, type_name, hash) {
                Ok(r) => return Ok(r),
                Err(TypeResolveError::NotYetResolved) => {}
                Err(e) => return Err(e),
            }
        }
    }

    /// Registers a `CompleteTypeObject` this crate synthesized itself
    /// (not read from an IDL file, an XML `<types>` document, or
    /// TypeLookup) — `crate::rpc_dispatch`'s per-operation In/Out
    /// structs, DDS-RPC 1.0 §7.5.1.1.4/§7.5.1.1.5, built from an
    /// operation's parameter list via
    /// `zerodds_types::from_ast::type_identifier`. Goes through the
    /// same `resolve_from_complete` every other source does, so a
    /// synthetic type is validated (its `keyed`/`extensibility` read
    /// off itself, never asserted by the caller) exactly like a
    /// discovered or file-loaded one.
    ///
    /// # Errors
    /// See [`TypeResolveError`].
    pub fn resolve_synthetic(
        &self,
        type_name: &str,
        root: zerodds_types::CompleteTypeObject,
        dependencies: Vec<zerodds_types::CompleteTypeObject>,
    ) -> Result<ResolvedType, TypeResolveError> {
        let resolved = resolve_from_complete(type_name, root, dependencies)?;
        self.insert(type_name, resolved.clone());
        Ok(resolved)
    }
}

/// The TypeLookup registry's generation counter, or `0` on a poisoned
/// lock (treated as "unknown", so the caller's first comparison always
/// looks unchanged rather than panicking).
fn current_generation(runtime: &DcpsRuntime) -> u64 {
    runtime
        .type_lookup_server
        .lock()
        .map(|s| s.registry.generation())
        .unwrap_or(0)
}

/// Builds a [`ResolvedType`] for `type_name` out of a locally parsed
/// `Ast` (the IDL-file path).
fn resolve_from_ast(ast: &Arc<Ast>, type_name: &str) -> Result<ResolvedType, TypeResolveError> {
    let dynamic_type = DynamicType::find(ast, type_name)
        .map_err(|e| TypeResolveError::NotFound(format!("{type_name}: {e}")))?;
    let root_id = dynamic_type.id();
    let complete = zerodds_types::from_ast::complete_type_object(ast, root_id)
        .map_err(|e| TypeResolveError::Model(format!("{e:?}")))?;
    // Every other aggregate type the root's members reach also needs a
    // CompleteTypeObject in the dependency closure — every named
    // (non-primitive) type in the model, minus the root itself, is
    // offered; `SerializedTypeSupport` only uses what it needs.
    let mut dependencies = Vec::new();
    for (id, _) in ast.types() {
        if id == root_id {
            continue;
        }
        if let Ok(dep) = zerodds_types::from_ast::complete_type_object(ast, id) {
            dependencies.push(dep);
        }
    }
    resolve_from_complete(type_name, complete, dependencies)
}

/// Shared tail: turns a `CompleteTypeObject` + its dependency closure
/// into a [`ResolvedType`].
///
/// `keyed`/`extensibility` are read directly off the object — XTypes
/// 1.3 §7.3.4.5 states a struct's/union's extensibility in its own
/// `struct_flags`/`union_flags` (`IS_FINAL`/`IS_APPENDABLE`/
/// `IS_MUTABLE`) and a member's `@key` in its `member_flags`
/// (`IS_KEY`, §7.3.4.6) — never brute-forced. The model
/// (`ModelFromTypeObjects`) is built first, and
/// [`zerodds_ast_layout::keyhash::holder_keeps_only_keys`] /
/// [`zerodds_ast_layout::keyhash::union_key_discriminator`] /
/// [`DynamicType::extensibility`] read those exact bits back off it —
/// the same public functions `SerializedTypeSupport`'s own (private)
/// `from_decoded_objects` calls internally to derive `actual_keyed`/
/// `actual_ext`, so this reads the object the identical way the
/// constructor that validates it would.
fn resolve_from_complete(
    type_name: &str,
    root: zerodds_types::CompleteTypeObject,
    dependencies: Vec<zerodds_types::CompleteTypeObject>,
) -> Result<ResolvedType, TypeResolveError> {
    let root_bytes = TypeObject::Complete(root.clone())
        .to_bytes_le()
        .map_err(|e| TypeResolveError::Model(format!("re-encoding root TypeObject: {e:?}")))?;
    let dep_bytes: Vec<Vec<u8>> = dependencies
        .iter()
        .map(|d| {
            TypeObject::Complete(d.clone())
                .to_bytes_le()
                .map_err(|e| TypeResolveError::Model(format!("re-encoding dependency: {e:?}")))
        })
        .collect::<Result<_, _>>()?;

    let mut registry = zerodds_types::resolve::TypeRegistry::new();
    for dep in &dependencies {
        registry
            .insert_object(TypeObject::Complete(dep.clone()))
            .map_err(|e| TypeResolveError::Model(format!("dependency hash: {e:?}")))?;
    }
    registry
        .insert_object(TypeObject::Complete(root.clone()))
        .map_err(|e| TypeResolveError::Model(format!("root hash: {e:?}")))?;
    let mut model = zerodds_types::dynamic::ModelFromTypeObjects::new(&registry);
    let root_id = model
        .add_type_object(&TypeObject::Complete(root))
        .map_err(|e| TypeResolveError::Model(format!("{e:?}")))?;
    let ast = model.finish();

    let dynamic_type = DynamicType::new(Arc::clone(&ast), root_id);
    let keyed = zerodds_ast_layout::keyhash::holder_keeps_only_keys(&ast, root_id)
        || zerodds_ast_layout::keyhash::union_key_discriminator(&ast, root_id).is_some();
    let extensibility = match dynamic_type.extensibility() {
        Some(zerodds_types::dynamic::ExtensibilityKind::Final) => 0,
        Some(zerodds_types::dynamic::ExtensibilityKind::Appendable) => 1,
        Some(zerodds_types::dynamic::ExtensibilityKind::Mutable) => 2,
        None => {
            return Err(TypeResolveError::Model(format!(
                "`{type_name}`'s root has no aggregate (struct/union) extensibility"
            )));
        }
    };

    let support = SerializedTypeSupport::from_type_objects(
        type_name,
        &root_bytes,
        &dep_bytes,
        keyed,
        extensibility,
    )
    .map_err(TypeResolveError::Model)?;

    Ok(ResolvedType {
        support,
        dynamic_type,
        keyed,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn write_idl(dir: &std::path::Path, text: &str) -> std::path::PathBuf {
        let path = dir.join("t.idl");
        std::fs::write(&path, text).unwrap();
        path
    }

    fn write_xml(dir: &std::path::Path, text: &str) -> std::path::PathBuf {
        let path = dir.join("t.xml");
        std::fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn loads_a_keyed_struct_from_a_dds_xml_types_file() {
        let dir = std::env::temp_dir().join(format!("zd-webdds-xmltypes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_xml(
            &dir,
            r#"<types>
                <struct name="Sample">
                    <member name="id" type="int32" key="true"/>
                    <member name="value" type="float64"/>
                </struct>
            </types>"#,
        );
        let reg = TypeRegistry::new();
        let resolved = reg.load_xml_file(&path, "Sample").expect("resolved");
        assert!(resolved.keyed);
        assert_eq!(resolved.dynamic_type.member_count(), 2);
        assert!(reg.get("Sample").is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dds_xml_types_file_with_an_enum_dependency_resolves() {
        let dir = std::env::temp_dir().join(format!("zd-webdds-xmltypes2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_xml(
            &dir,
            r#"<types>
                <enum name="Color">
                    <enumerator name="RED"/>
                    <enumerator name="GREEN"/>
                </enum>
                <struct name="Pixel">
                    <member name="id" type="int32" key="true"/>
                    <member name="color" type="nonBasic" nonBasicTypeName="Color"/>
                </struct>
            </types>"#,
        );
        let reg = TypeRegistry::new();
        let resolved = reg.load_xml_file(&path, "Pixel").expect("resolved");
        assert!(resolved.keyed);
        assert_eq!(resolved.dynamic_type.member_count(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_type_in_xml_file_is_reported() {
        let dir = std::env::temp_dir().join(format!("zd-webdds-xmltypes3-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_xml(
            &dir,
            r#"<types><struct name="A"><member name="x" type="int32"/></struct></types>"#,
        );
        let reg = TypeRegistry::new();
        let err = reg.load_xml_file(&path, "NoSuchType").unwrap_err();
        assert!(matches!(err, TypeResolveError::NotFound(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn loads_a_keyed_struct_from_an_idl_file() {
        let dir = std::env::temp_dir().join(format!("zd-webdds-types-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_idl(
            &dir,
            "module M { @final struct Sample { @key long id; string name; }; };",
        );
        let reg = TypeRegistry::new();
        let resolved = reg.load_idl_file(&path, "M::Sample").expect("resolved");
        assert!(resolved.keyed);
        assert_eq!(resolved.dynamic_type.member_count(), 2);
        // Cached under the name.
        assert!(reg.get("M::Sample").is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_type_name_is_reported() {
        let dir = std::env::temp_dir().join(format!("zd-webdds-types2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_idl(&dir, "module M { @final struct A { long x; }; };");
        let reg = TypeRegistry::new();
        let err = reg.load_idl_file(&path, "M::NoSuchType").unwrap_err();
        assert!(matches!(err, TypeResolveError::NotFound(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unkeyed_struct_roundtrip() {
        let dir = std::env::temp_dir().join(format!("zd-webdds-types3-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_idl(
            &dir,
            "module M { @final struct Plain { long x; long y; }; };",
        );
        let reg = TypeRegistry::new();
        let resolved = reg.load_idl_file(&path, "M::Plain").expect("resolved");
        assert!(!resolved.keyed);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A second participant, which never read the IDL file, resolves
    /// the topic's type purely from SEDP discovery + a real `getTypes`
    /// round trip — proving `resolve_named` end to end, not just its
    /// bookkeeping.
    #[test]
    fn resolve_named_discovers_a_type_never_loaded_locally() {
        let dir = std::env::temp_dir().join(format!("zd-webdds-types-tl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_idl(
            &dir,
            "module M { @final struct Tlq { @key long id; long value; }; };",
        );

        let lease = zerodds_test_domains::lease_domain();
        let domain = lease.id();

        // Side A: publishes the topic, with the type known locally.
        let reg_a = TypeRegistry::new();
        let resolved_a = reg_a.load_idl_file(&path, "M::Tlq").expect("resolve A");
        let participant_a = zerodds_dcps::DomainParticipantFactory::instance()
            .create_participant(domain, zerodds_dcps::DomainParticipantQos::default())
            .expect("participant A");
        let topic_a = participant_a
            .create_serialized_topic("Tlq", zerodds_dcps::TopicQos::default(), resolved_a.support)
            .expect("topic A");
        let publisher_a = participant_a.create_publisher(zerodds_dcps::PublisherQos::default());
        let _writer_a = publisher_a
            .create_datawriter(&topic_a, zerodds_dcps::DataWriterQos::default())
            .expect("writer A");

        // Side B: never loads the IDL file — only discovers the type.
        let participant_b = zerodds_dcps::DomainParticipantFactory::instance()
            .create_participant(domain, zerodds_dcps::DomainParticipantQos::default())
            .expect("participant B");
        let subscriber_b = participant_b.create_subscriber(zerodds_dcps::SubscriberQos::default());
        // A reader on the *typed* API needs its own type support to
        // build; the point under test is that `reg_b` resolves the type
        // without ever having read `path` — so `reg_b` uses a distinct
        // registry from `reg_a`, and only `reg_b`'s resolution result is
        // asserted on. The reader itself is not needed for that — SEDP
        // discovers the type from the *writer*'s announcement alone.
        let _ = &subscriber_b;

        let reg_b = TypeRegistry::new();
        let runtime_b = participant_b.runtime().expect("live runtime B").clone();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let mut resolved_b = None;
        while resolved_b.is_none() && std::time::Instant::now() < deadline {
            match reg_b.resolve_named(
                &runtime_b,
                "Tlq",
                "M::Tlq",
                std::time::Duration::from_secs(2),
            ) {
                Ok(r) => resolved_b = Some(r),
                Err(TypeResolveError::NoDiscoveredEndpoint(_)) => {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                Err(e) => panic!("resolve_named failed: {e}"),
            }
        }
        let resolved_b = resolved_b.expect("type resolved via TypeLookup within 20s");
        assert!(resolved_b.keyed);
        assert_eq!(resolved_b.dynamic_type.member_count(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
