// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Runtime-described samples using the common AST codec and ordinary DDS entities.
//! No foreign callbacks execute in this adapter, including under reader locks.

use crate::dds_type::{DdsType, DecodeError, EncodeError, Extensibility};
use crate::topic::{TopicCodec, TopicTypeMetadata};
use std::borrow::Cow;
use std::sync::Arc;
use zerodds_ast::{Ast, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_types::dynamic::DynamicSample;
use zerodds_types::dynamic::{DynamicData, ModelFromTypeObjects};
use zerodds_types::{TypeIdentifier, TypeObject};
use zerodds_wire_codec::{Emit, ProbeRequest, Source};

/// A validated sample. Original wire coordinates and bytes are retained.
#[derive(Debug, Clone)]
pub struct SerializedSample {
    data: DynamicData,
    identity: TypeIdentifier,
    body: Vec<u8>,
    representation: u8,
    big_endian: bool,
    key: Option<([u8; 16], Vec<u8>)>,
    /// The type the value was read as, when the model names it: what a
    /// filter needs to tell a map from a sequence of pairs, and an
    /// optional member that is absent from a member that is not there.
    model: Option<SampleModel>,
}

/// The model and the root type a sample was read as.
#[derive(Clone)]
struct SampleModel {
    ast: Arc<Ast>,
    root: Id<TypeNode>,
}

impl core::fmt::Debug for SampleModel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SampleModel")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

/// `id` with the aliases followed to the type they name.
fn unaliased(ast: &Ast, mut id: Id<TypeNode>) -> Id<TypeNode> {
    for _ in 0..32 {
        let Ok(node) = ast.type_(id) else {
            break;
        };
        let TypeKind::Alias { aliased } = &node.kind else {
            break;
        };
        match aliased.resolved() {
            Some(next) => id = next,
            None => break,
        }
    }
    id
}

/// The type of the member `name` of the aggregate `type_`, and whether it
/// is optional; `None` when the type has no such member.
fn member_of(ast: &Ast, type_: Id<TypeNode>, name: &str) -> Option<(Id<TypeNode>, bool)> {
    let node = ast.type_(type_).ok()?;
    let members: Vec<Id<zerodds_ast::MemberNode>> = match &node.kind {
        TypeKind::Union { .. } => ast
            .cases_of(type_)
            .iter()
            .filter_map(|case| ast.case(*case).ok().map(|case| case.member))
            .collect(),
        _ => ast.all_members_of(type_),
    };
    members.into_iter().find_map(|id| {
        let member = ast.member(id).ok()?;
        (member.name == name).then(|| {
            let optional = member.facets.xtypes.as_ref().is_some_and(|x| x.is_optional);
            member
                .type_
                .resolved()
                .map(|target| (unaliased(ast, target), optional))
        })?
    })
}

/// Whether a map entry's key, a value of the key type, is the text `key`.
///
/// §7.6.7.1: "the key is a string even when the map's key type is an
/// integer type", so an integer key is compared by its decimal spelling.
fn key_is(entry: &DynamicData, key: &str) -> bool {
    match entry {
        DynamicData::Text(text) | DynamicData::Enumerator(text) => text == key,
        DynamicData::Integer { value, .. } => value.to_string() == key,
        DynamicData::Character { value, .. } => value == key,
        _ => false,
    }
}

impl SerializedSample {
    /// Original body without its RTPS encapsulation.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }
    /// The decoded value — the reflective counterpart of a typed
    /// `DataReader<T>::take`'s sample, for a caller that resolved this
    /// support at run time and has no compiled `T` to read fields off of
    /// (a generic bridge mapping the value into another protocol's own
    /// model, e.g. an OPC-UA `Variant` tree or a Part-14 DataSet).
    #[must_use]
    pub fn data(&self) -> &DynamicData {
        &self.data
    }
    /// DDS representation ID, XCDR1=0 or XCDR2=2.
    #[must_use]
    pub fn representation(&self) -> u8 {
        self.representation
    }
    /// Original byte order.
    #[must_use]
    pub fn big_endian(&self) -> bool {
        self.big_endian
    }
    /// The instance's `KeyHash` (DDS 1.4 §2.2.2.4.2, DDSI-RTPS 2.5
    /// §9.6.3.3), computed by `SerializedTypeSupport::read_body` when the
    /// type is keyed — `None` for an unkeyed type. Exposed as a plain
    /// accessor (rather than only through the crate-private `TopicCodec`
    /// trait `key_material` computes it for) so a caller that only has a
    /// `SerializedTypeSupport`/`SerializedSample` pair — not the full
    /// `DomainParticipant`/`Topic<T>`/`DataWriter<T>` typed-entity stack —
    /// can still pass a real `KeyHash` to `DcpsRuntime::
    /// write_user_sample_keyed`/`write_user_lifecycle` on the raw byte
    /// path (`crates/routing-service`, XRCE-agent daemon bridges).
    #[must_use]
    pub fn key_hash(&self) -> Option<[u8; 16]> {
        self.key.as_ref().map(|(hash, _)| *hash)
    }
}

impl DdsType for SerializedSample {
    const TYPE_NAME: &'static str = "zerodds::SerializedSample (requires per-topic support)";
    fn encode(&self, _: &mut Vec<u8>) -> Result<(), EncodeError> {
        Err(EncodeError::Invalid {
            what: "serialized samples require their topic support",
        })
    }
    fn decode(_: &[u8]) -> Result<Self, DecodeError> {
        Err(DecodeError::Invalid {
            what: "serialized samples require their topic support",
        })
    }
    fn field_value(&self, path: &str) -> Option<zerodds_sql_filter::Value> {
        dynamic_field_value(
            &self.data,
            self.model
                .as_ref()
                .map(|model| (model.ast.as_ref(), model.root)),
            path,
        )
    }
}

/// The value a content-filter path names in a dynamic value
/// (DDS-XTypes 1.3 §7.6.7), for a sample that knows its type (`model`:
/// the model and the root type) or one that does not.
pub(crate) fn dynamic_field_value(
    data: &DynamicData,
    model: Option<(&Ast, Id<TypeNode>)>,
    path: &str,
) -> Option<zerodds_sql_filter::Value> {
    {
        use zerodds_sql_filter::Value;
        use zerodds_sql_filter::{PathStep, split_path};
        let mut value = data;
        // The type of `value`, as far as the sample knows its type. DDS-
        // XTypes 1.3 §7.6.7.1: `[n]` is the element of an array or a
        // sequence, `['NAME']` the value of a map or the flag of a bitmask
        // (Boolean). The dynamic form keeps a map as a collection of
        // two-element collections and does not say which collections are
        // maps, so the type decides what a step means.
        let mut at = model.map(|(ast, root)| (ast, unaliased(ast, root)));
        let steps = split_path(path)?;
        let mut flag = None;
        for step in &steps {
            if flag.is_some() {
                return None;
            }
            match step {
                PathStep::Member(name) => match value.field(name) {
                    Some(member) => {
                        at = at.and_then(|(ast, type_)| {
                            member_of(ast, type_, name).map(|(next, _)| (ast, next))
                        });
                        value = member;
                    }
                    // §7.6.7.2: an optional member that is not there is
                    // `null`. A name the type does not have is no field.
                    None => {
                        return match at {
                            Some((ast, type_))
                                if member_of(ast, type_, name)
                                    .is_some_and(|(_, optional)| optional) =>
                            {
                                Some(Value::Null)
                            }
                            _ => None,
                        };
                    }
                },
                PathStep::Index(n) => {
                    let DynamicData::Collection(items) = value else {
                        return None;
                    };
                    let map = at.and_then(|(ast, type_)| match &ast.type_(type_).ok()?.kind {
                        TypeKind::Map { value, .. } => value.resolved().map(|v| (ast, v)),
                        _ => None,
                    });
                    if let Some((ast, value_type)) = map {
                        // A map whose key type is an integer: `[5]` is the
                        // key 5, as `['5']` is.
                        let entry = items.iter().find(|entry| {
                            matches!(entry, DynamicData::Collection(pair)
                                if pair.first().is_some_and(|key| key_is(key, &n.to_string())))
                        })?;
                        let DynamicData::Collection(pair) = entry else {
                            return None;
                        };
                        value = pair.get(1)?;
                        at = Some((ast, unaliased(ast, value_type)));
                    } else {
                        value = items.get(usize::try_from(*n).ok()?)?;
                        at = at.and_then(|(ast, type_)| match &ast.type_(type_).ok()?.kind {
                            TypeKind::Sequence { element, .. }
                            | TypeKind::Array { element, .. } => {
                                element.resolved().map(|e| (ast, unaliased(ast, e)))
                            }
                            _ => None,
                        });
                    }
                }
                PathStep::Key(name) => match value {
                    DynamicData::Bits(bits) => {
                        flag = Some(
                            bits.iter()
                                .any(|bit| matches!(bit, DynamicData::Text(set) if set == name)),
                        );
                    }
                    // The value of a map entry, found by its key. Only a
                    // sample that knows its type can say a collection is a
                    // map.
                    DynamicData::Collection(items) => {
                        let (ast, type_) = at?;
                        let TypeKind::Map {
                            value: value_type, ..
                        } = &ast.type_(type_).ok()?.kind
                        else {
                            return None;
                        };
                        let value_type = value_type.resolved()?;
                        let entry = items.iter().find(|entry| {
                            matches!(entry, DynamicData::Collection(pair)
                                if pair.first().is_some_and(|key| key_is(key, name)))
                        })?;
                        let DynamicData::Collection(pair) = entry else {
                            return None;
                        };
                        value = pair.get(1)?;
                        at = Some((ast, unaliased(ast, value_type)));
                    }
                    _ => return None,
                },
            }
        }
        if let Some(set) = flag {
            return Some(Value::Bool(set));
        }
        match value {
            DynamicData::Boolean(v) => Some(Value::Bool(*v)),
            DynamicData::Integer { value, .. } => i64::try_from(*value).ok().map(Value::Int),
            DynamicData::Float { value, .. } => Some(Value::Float(*value)),
            // Annex B.2 ENUMERATEDVALUE: "the name of the enumeration label
            // enclosed in single quotes", so an enumerated member compares
            // with the name of its label.
            DynamicData::Text(v)
            | DynamicData::Character { value: v, .. }
            | DynamicData::Enumerator(v) => Some(Value::String(v.clone())),
            _ => None,
        }
    }
}

/// `DynamicData` as a topic type: the `FooDataWriter` and `FooDataReader`
/// of DDS-XTypes 1.3 §7.6.6.2 for "Foo" = `DynamicData` are
/// `DataWriter<DynamicSample>` and `DataReader<DynamicSample>`
/// ([`DynamicDataWriter`], [`DynamicDataReader`]), created over a topic from
/// [`crate::participant::DomainParticipant::create_dynamic_topic`]. They
/// "do not define additional properties or operations".
impl DdsType for DynamicSample {
    const TYPE_NAME: &'static str = "zerodds::DynamicData (requires per-topic support)";
    fn encode(&self, _: &mut Vec<u8>) -> Result<(), EncodeError> {
        Err(EncodeError::Invalid {
            what: "dynamic samples require their topic support",
        })
    }
    fn decode(_: &[u8]) -> Result<Self, DecodeError> {
        Err(DecodeError::Invalid {
            what: "dynamic samples require their topic support",
        })
    }
    fn field_value(&self, path: &str) -> Option<zerodds_sql_filter::Value> {
        let data = self.snapshot().ok()?;
        dynamic_field_value(
            &data,
            Some((self.type_().ast().as_ref(), self.type_().id())),
            path,
        )
    }
}

/// The writer of `DynamicData` samples: `FooDataWriter` with "Foo" =
/// `DynamicData` (DDS-XTypes 1.3 §7.6.6.2).
pub type DynamicDataWriter = crate::publisher::DataWriter<DynamicSample>;

/// The reader of `DynamicData` samples: `FooDataReader` with "Foo" =
/// `DynamicData` (DDS-XTypes 1.3 §7.6.6.2).
pub type DynamicDataReader = crate::subscriber::DataReader<DynamicSample>;

/// The `representation` argument [`SerializedTypeSupport::read_body`]
/// takes (0 = XCDR1, 2 = XCDR2) for the tag a
/// [`crate::runtime::UserSample::Alive`] carries (0 = XCDR1, 1 = XCDR2).
/// The two conventions differ; passing the sample's tag through unchanged
/// makes every XCDR2 sample fail to decode.
#[must_use]
pub const fn read_body_representation(user_sample_representation: u8) -> u8 {
    if user_sample_representation == 0 {
        0
    } else {
        2
    }
}

/// The PL_CDR1 settings of the process: `wire.pl_cdr1_headers` (0 keeps the
/// dialect's, otherwise the index in `PlCdr1HeaderPolicy::ALL` plus one) and
/// `wire.pl_cdr1_reading`. Read where a request is built and no runtime is at
/// hand; the first runtime that sets them sets them for all.
static PL_CDR1_HEADERS: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0);
static PL_CDR1_TOLERANT: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// Sets the process-wide PL_CDR1 settings (see [`PL_CDR1_HEADERS`]).
pub(crate) fn set_codec_overrides(
    headers: Option<zerodds_ast_layout::PlCdr1HeaderPolicy>,
    reading: zerodds_ast_layout::PlCdr1Reading,
) {
    use core::sync::atomic::Ordering::Relaxed;
    if let Some(policy) = headers {
        let index = zerodds_ast_layout::PlCdr1HeaderPolicy::ALL
            .iter()
            .position(|candidate| *candidate == policy)
            .map_or(0, |at| at + 1);
        PL_CDR1_HEADERS.store(u8::try_from(index).unwrap_or(0), Relaxed);
    }
    if reading.tolerant() {
        PL_CDR1_TOLERANT.store(true, Relaxed);
    }
}

/// `request` with the process-wide PL_CDR1 settings applied.
fn with_codec_overrides(mut request: ProbeRequest) -> ProbeRequest {
    use core::sync::atomic::Ordering::Relaxed;
    let index = usize::from(PL_CDR1_HEADERS.load(Relaxed));
    if let Some(policy) = index
        .checked_sub(1)
        .and_then(|at| zerodds_ast_layout::PlCdr1HeaderPolicy::ALL.get(at))
    {
        request.pl_cdr1_headers = *policy;
    }
    if PL_CDR1_TOLERANT.load(Relaxed) {
        request.pl_cdr1_reading = zerodds_ast_layout::PlCdr1Reading::Tolerant;
    }
    request
}

/// Immutable, owned type metadata and Rust-only codec state.
#[derive(Clone)]
pub struct SerializedTypeSupport {
    ast: Arc<Ast>,
    /// The name the model knows the type by. [`Self::get_type_name`] is
    /// the name the type is registered under, which is this one unless
    /// [`Self::with_type_name`] named another.
    model_name: String,
    metadata: TopicTypeMetadata,
    dependencies: Vec<TypeObject>,
    /// The CDR dialect the bodies and keys are read and written in; the
    /// specification's unless [`Self::in_dialect`] names another.
    dialect: zerodds_ast_layout::Dialect,
}

impl core::fmt::Debug for SerializedTypeSupport {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SerializedTypeSupport")
            .field("metadata", &self.metadata)
            .field("dialect", &self.dialect)
            .finish_non_exhaustive()
    }
}

impl SerializedTypeSupport {
    /// Reconstruct a self-contained type. For dependent types use `from_type_objects`.
    /// Missing dependencies refuse
    /// registration, never produce a reduced/keyless topic.
    /// # Errors
    /// Invalid/incomplete metadata, unsupported key inverse or identity mismatch.
    pub fn from_type_object(
        type_name: &str,
        bytes: &[u8],
        keyed: bool,
        extensibility: u8,
    ) -> Result<Arc<Self>, String> {
        Self::from_type_objects(type_name, bytes, &[], keyed, extensibility)
    }

    /// Load the complete dependency closure before reconstructing the shared AST.
    /// # Errors
    /// Invalid, incomplete or conflicting metadata, or unsupported key contexts.
    pub fn from_type_objects(
        type_name: &str,
        bytes: &[u8],
        dependencies: &[Vec<u8>],
        keyed: bool,
        extensibility: u8,
    ) -> Result<Arc<Self>, String> {
        let mut reader = zerodds_types::framing::xcdr2_reader(bytes);
        let object = TypeObject::decode_from(&mut reader)
            .map_err(|e| format!("invalid TypeObject: {e:?}"))?;
        if reader.remaining() != 0 {
            return Err("trailing bytes after CompleteTypeObject".into());
        }
        if dependencies.len() > zerodds_types::resolve::DEFAULT_MAX_RESOLVE_NODES {
            return Err("type dependency closure exceeds node limit".into());
        }
        let mut decoded_dependencies = Vec::new();
        for bytes in dependencies {
            let mut reader = zerodds_types::framing::xcdr2_reader(bytes);
            let object = TypeObject::decode_from(&mut reader)
                .map_err(|e| format!("invalid dependency TypeObject: {e:?}"))?;
            if reader.remaining() != 0 || !matches!(object, TypeObject::Complete(_)) {
                return Err("dependency must contain exactly one complete TypeObject".into());
            }
            decoded_dependencies.push(object);
        }
        Self::from_decoded_objects(
            type_name,
            object,
            decoded_dependencies,
            Some(keyed),
            Some(extensibility),
        )
    }

    /// [`Self::from_type_objects`] for a root already resolved through the
    /// runtime's TypeLookup registry (`zerodds_types::resolve::TypeRegistry`,
    /// filled by `DcpsRuntime::register_type_object`) instead of caller-owned
    /// bytes. Used by the receive path to compute a keyed reader's KeyHash
    /// from the type when a writer's DATA carries none (DDSI-RTPS 2.5
    /// §9.6.3.3 leaves `PID_KEY_HASH` optional) — public so a caller outside
    /// `crates/dcps` holding a `CompleteTypeObject` from
    /// `DcpsRuntime::type_lookup_server` (e.g. a bridge daemon resolving an
    /// auto-discovered topic's type, `DiscoveredEndpointInfo::type_identifier`)
    /// can build the same support without re-encoding the object to bytes
    /// first.
    ///
    /// Unlike [`Self::from_type_objects`] there is no separately asserted
    /// `keyed`/`extensibility` to check `root` against — both are the ones
    /// already registered for this type's `TypeIdentifier`, so they are read
    /// off `root` itself rather than re-asserted by the caller; read them
    /// back with [`Self::is_keyed`] / [`Self::extensibility`].
    ///
    /// # Errors
    /// Incomplete or conflicting metadata, or an unsupported key context —
    /// see [`Self::from_type_objects`].
    pub fn from_complete_type_objects(
        type_name: &str,
        root: zerodds_types::CompleteTypeObject,
        dependencies: Vec<zerodds_types::CompleteTypeObject>,
    ) -> Result<Arc<Self>, String> {
        if dependencies.len() > zerodds_types::resolve::DEFAULT_MAX_RESOLVE_NODES {
            return Err("type dependency closure exceeds node limit".into());
        }
        let decoded_dependencies = dependencies.into_iter().map(TypeObject::Complete).collect();
        Self::from_decoded_objects(
            type_name,
            TypeObject::Complete(root),
            decoded_dependencies,
            None,
            None,
        )
    }

    /// The support of a topic whose `TypeObject` and dependency closure are
    /// at hand (`TopicTypeMetadata::type_object`), under `type_name`: what a
    /// MultiTopic reads the members of its topics' types through.
    ///
    /// # Errors
    /// `root` or a dependency is not a `CompleteTypeObject`, or the objects
    /// do not describe a type of that name.
    pub(crate) fn from_topic_objects(
        type_name: &str,
        root: &TypeObject,
        dependencies: &[TypeObject],
    ) -> Result<Arc<Self>, String> {
        if !matches!(root, TypeObject::Complete(_))
            || dependencies
                .iter()
                .any(|dep| !matches!(dep, TypeObject::Complete(_)))
        {
            return Err("a CompleteTypeObject is needed to read the members of the type".into());
        }
        Self::from_decoded_objects(type_name, root.clone(), dependencies.to_vec(), None, None)
    }

    /// Shared tail of [`Self::from_type_objects`] and
    /// [`Self::from_complete_type_objects`]: builds the shared AST from
    /// `object` and `decoded_dependencies`, then either asserts `keyed`/
    /// `extensibility` against what the object actually describes
    /// (`Some`, the caller-supplied-bytes path) or simply reads them off it
    /// (`None`, the registry path — there is nothing external left to
    /// disagree with the object).
    fn from_decoded_objects(
        type_name: &str,
        object: TypeObject,
        decoded_dependencies: Vec<TypeObject>,
        keyed: Option<bool>,
        extensibility: Option<u8>,
    ) -> Result<Arc<Self>, String> {
        let mut registry = zerodds_types::resolve::TypeRegistry::new();
        for dep in &decoded_dependencies {
            registry
                .insert_object(dep.clone())
                .map_err(|e| format!("dependency hash: {e:?}"))?;
        }
        let hash = registry
            .insert_object(object.clone())
            .map_err(|e| format!("TypeObject hash: {e:?}"))?;
        let mut model = ModelFromTypeObjects::new(&registry);
        let root = model
            .add_type_object(&object)
            .map_err(|e| format!("incomplete TypeObject: {e:?}"))?;
        let ast = model.finish();
        let dynamic = zerodds_types::dynamic::DynamicType::find(&ast, type_name)
            .map_err(|e| format!("TypeObject name: {e:?}"))?;
        if dynamic.id() != root {
            return Err("descriptor name does not identify the supplied root TypeObject".into());
        }
        let actual_keyed = zerodds_ast_layout::keyhash::holder_keeps_only_keys(&ast, root)
            || zerodds_ast_layout::union_key_discriminator(&ast, root).is_some();
        let actual_ext = match dynamic.extensibility() {
            Some(zerodds_types::dynamic::ExtensibilityKind::Final) => 0,
            Some(zerodds_types::dynamic::ExtensibilityKind::Appendable) => 1,
            Some(zerodds_types::dynamic::ExtensibilityKind::Mutable) => 2,
            _ => return Err("topic root must have aggregate extensibility".into()),
        };
        if let Some(extensibility) = extensibility {
            if !matches!(extensibility, 0..=2) {
                return Err("invalid extensibility".into());
            }
        }
        if keyed.is_some_and(|keyed| keyed != actual_keyed)
            || extensibility.is_some_and(|extensibility| extensibility != actual_ext)
        {
            return Err("descriptor flags disagree with CompleteTypeObject".into());
        }
        let ext = match actual_ext {
            0 => Extensibility::Final,
            1 => Extensibility::Appendable,
            _ => Extensibility::Mutable,
        };
        let metadata = TopicTypeMetadata {
            type_name: Cow::Owned(type_name.into()),
            type_identifier: TypeIdentifier::EquivalenceHashComplete(hash),
            type_object: Some(object),
            keyed: actual_keyed,
            extensibility: ext,
            key_holder_max_size: None,
            max_serialized_size: None,
        };
        let support = Self {
            ast,
            model_name: type_name.into(),
            metadata,
            dependencies: decoded_dependencies,
            dialect: zerodds_ast_layout::Dialect::default(),
        };
        if actual_keyed {
            zerodds_wire_codec::decode::validate_key_holder(
                &support.ast,
                &support.request(2, true)?,
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(Arc::new(support))
    }

    pub(crate) fn metadata(&self) -> &TopicTypeMetadata {
        &self.metadata
    }

    /// DDS-XTypes 1.3 §7.6.6.1.4 `get_type_name`.
    #[must_use]
    pub fn get_type_name(&self) -> &str {
        &self.metadata.type_name
    }

    /// The same type support under another registered name, for
    /// `register_type(participant, type_name)` (DDS 1.4 §2.2.2.3.6): the
    /// name is what the topics created from it announce and what a peer
    /// matches on (`string<Char8,256>`). The type and its `TypeIdentifier`
    /// are the support's own.
    ///
    /// # Errors
    /// The name is empty or longer than 256 characters.
    pub fn with_type_name(&self, type_name: &str) -> Result<Arc<Self>, String> {
        if type_name.is_empty() || type_name.chars().count() > 256 {
            return Err(format!(
                "a type name is 1 to 256 characters, `{type_name}` has {}",
                type_name.chars().count()
            ));
        }
        let mut renamed = self.clone();
        renamed.metadata.type_name = Cow::Owned(type_name.into());
        Ok(Arc::new(renamed))
    }

    /// The `TypeObject`s of the support: the type and what it needs.
    pub(crate) fn type_objects(&self) -> impl Iterator<Item = &TypeObject> {
        self.metadata
            .type_object
            .iter()
            .chain(self.dependencies.iter())
    }

    /// DDS-XTypes 1.3 §7.6.6.1.4 `register_type` (Table 65, defined by DDS
    /// 1.4 §2.2.2.3.6): registers the type with `participant` under
    /// `type_name`, or under [`Self::get_type_name`] where none is given.
    /// The `TypeObject`s of the type go to the participant's TypeLookup
    /// registry, and [`DomainParticipant::create_registered_topic`](crate::DomainParticipant::create_registered_topic) creates
    /// topics of the type by its name.
    ///
    /// Registering the same type under the same name again is not an
    /// error.
    ///
    /// # Errors
    /// `PreconditionNotMet` when the name is registered for another type;
    /// `BadParameter` for a name that is no name;
    /// `AlreadyDeleted` for a deleted participant.
    pub fn register_type(
        self: &Arc<Self>,
        participant: &crate::participant::DomainParticipant,
        type_name: Option<&str>,
    ) -> crate::error::Result<()> {
        participant.register_serialized_type(self, type_name)
    }

    /// DDS-XTypes 1.3 §7.6.6.1.1.1 `get_type` (Table 63): the
    /// `DynamicType` of the support's data type.
    ///
    /// # Errors
    /// The model does not name the type, which a support built by this
    /// crate's constructors always does.
    pub fn get_type(&self) -> Result<zerodds_types::dynamic::DynamicType, String> {
        zerodds_types::dynamic::DynamicType::find(&self.ast, &self.model_name)
            .map_err(|e| e.to_string())
    }

    /// DDS-XTypes 1.3 §7.6.6.1.2.2 `create_dynamic_sample` (Table 64): a
    /// `DynamicData` with the contents of a sample of the support's data
    /// type.
    ///
    /// # Errors
    /// The sample belongs to another type.
    pub fn create_dynamic_sample(
        &self,
        src: &SerializedSample,
    ) -> Result<zerodds_types::dynamic::DynamicSample, String> {
        if src.identity != self.metadata.type_identifier {
            return Err("sample belongs to another topic type".into());
        }
        Ok(zerodds_types::dynamic::DynamicSample::with_value(
            self.get_type()?,
            src.data.clone(),
        ))
    }

    /// DDS-XTypes 1.3 §7.6.6.1.2.1 `create_sample` (Table 64): a sample
    /// of the support's data type with the contents of a `DynamicData`.
    ///
    /// "This method shall fail [...] if the DynamicType of this object is
    /// not compatible with the TypeSupport’s data type." Compatible here
    /// is equal but for annotations
    /// (`DynamicType::equals_ignoring_annotations`): the sample is encoded
    /// by the support's type, and a sample of another type has other
    /// members.
    ///
    /// # Errors
    /// The type of `src` is not the support's, or the codec refuses a
    /// value.
    pub fn create_sample(
        &self,
        src: &zerodds_types::dynamic::DynamicSample,
    ) -> Result<SerializedSample, String> {
        // Compatible, as Table 64 says: the support's type was rebuilt
        // from `TypeObject`s, which carry no declared annotation, so the
        // annotations of `src`'s type are not what decides.
        if !src.type_().equals_ignoring_annotations(&self.get_type()?) {
            return Err(format!(
                "the DynamicData is of `{}`, not of `{}`",
                src.type_().qualified_name(),
                self.metadata.type_name
            ));
        }
        self.from_dynamic_data(src.value().map_err(|e| e.to_string())?)
    }

    /// DDS-XTypes 1.3 §7.6.6.1.5 `create_type_support` (Table 65): a type
    /// support for a `DynamicType`, with a **copy** of the type — it is
    /// rebuilt from the type's `TypeObject`s, so later changes to the
    /// argument's model do not reach it.
    ///
    /// # Errors
    /// "If this argument [...] is a nested type, the operation shall fail":
    /// a nested type, and a type that is not a structure or a union
    /// (§7.6.2: only aggregated types are topic types), are refused, as is
    /// a type with no `TypeObject`.
    pub fn create_type_support(
        type_: &zerodds_types::dynamic::DynamicType,
    ) -> Result<Arc<Self>, String> {
        use zerodds_types::dynamic::TypeKind as K;
        let descriptor = type_.descriptor().map_err(|e| e.to_string())?;
        if !matches!(descriptor.kind, K::Structure | K::Union) {
            return Err(format!(
                "`{}` is {}, and only a structure or a union is a topic type (§7.6.2)",
                type_.qualified_name(),
                descriptor.kind.as_str()
            ));
        }
        if descriptor.is_nested {
            return Err(format!(
                "`{}` is a nested type and has no topic (§7.6.6.1.5)",
                type_.qualified_name()
            ));
        }
        let root = zerodds_types::from_ast::complete_type_object(type_.ast(), type_.id())
            .map_err(|e| format!("{e:?}"))?;
        let dependencies =
            zerodds_types::from_ast::dependency_type_objects(type_.ast(), type_.id())
                .map_err(|e| format!("{e:?}"))?;
        Self::from_complete_type_objects(&type_.qualified_name(), root, dependencies)
    }

    /// DDS-XTypes 1.3 §7.6.6.1.6 `delete_type_support`: a nil support is
    /// deleted "successfully without having any observable effect"; the
    /// support is dropped when its last holder drops it.
    pub fn delete_type_support(support: Option<Arc<Self>>) {
        drop(support);
    }

    /// The same type, read and written in `dialect` — the one the language
    /// binding that produces its bodies was generated in (`zerodds-idlc
    /// generate --dialect`). The runtime sets it from `wire.codec_dialect`.
    /// It used to be the specification's always, so a binding generated for
    /// Fast DDS had every PL_CDR1 body refused and no key derived.
    #[must_use]
    pub fn in_dialect(self: Arc<Self>, dialect: zerodds_ast_layout::Dialect) -> Arc<Self> {
        if self.dialect == dialect {
            return self;
        }
        let mut support = Arc::try_unwrap(self).unwrap_or_else(|shared| (*shared).clone());
        support.dialect = dialect;
        Arc::new(support)
    }

    /// The dialect this support reads and writes in.
    #[must_use]
    pub fn dialect(&self) -> zerodds_ast_layout::Dialect {
        self.dialect
    }

    fn request(&self, representation: u8, big_endian: bool) -> Result<ProbeRequest, String> {
        let encoding = match representation {
            0 => Encoding::PlainCdr1,
            2 => Encoding::PlainCdr2,
            _ => return Err("DDS representation must be 0 or 2".into()),
        };
        Ok(with_codec_overrides(ProbeRequest::in_dialect(
            &self.model_name,
            encoding,
            if big_endian {
                Endianness::Big
            } else {
                Endianness::Little
            },
            self.dialect,
        )))
    }

    /// Validate externally generated body and derive real key material in Rust.
    /// # Errors
    /// Malformed payload or unsupported representation.
    pub fn read_body(
        &self,
        bytes: &[u8],
        representation: u8,
        big_endian: bool,
    ) -> Result<SerializedSample, String> {
        let request = self.request(representation, big_endian)?;
        let data = zerodds_wire_codec::decode::<DynamicData>(&self.ast, &request, bytes)
            .map_err(|e| e.to_string())?;
        self.sample(data, bytes.to_vec(), representation, big_endian)
    }

    /// Like [`Self::read_body`] for the first sample of `bytes`, which may hold
    /// more behind it: returns the sample and the number of octets it took.
    /// A transport that packs serialized samples back to back with no length
    /// for each (DDS-XRCE 1.0 Annex A `sequence<SampleData>`) tells them apart
    /// by this; each sample is read from alignment origin 0, and the octets
    /// after it are not looked at.
    ///
    /// # Errors
    /// The bytes do not start with a sample of this type in the given
    /// representation, or `representation` is neither 0 nor 2.
    pub fn read_body_prefix(
        &self,
        bytes: &[u8],
        representation: u8,
        big_endian: bool,
    ) -> Result<(SerializedSample, usize), String> {
        let request = self.request(representation, big_endian)?;
        let (data, taken) =
            zerodds_wire_codec::decode_prefix::<DynamicData>(&self.ast, &request, bytes)
                .map_err(|e| e.to_string())?;
        let sample = self.sample(data, bytes[..taken].to_vec(), representation, big_endian)?;
        Ok((sample, taken))
    }

    /// Validate a key-only wire payload — the `serializedKey` a writer puts
    /// behind a dispose/unregister DATA's K flag (RTPS 2.5 §9.4.2.11) — and
    /// derive its instance's real KeyHash.
    ///
    /// A writer serializes `serializedKey` in **its own** active
    /// representation and byte order, and it is not a (possibly
    /// truncated) full sample: it holds only the `@key` members —
    /// recursing into a nested keyed aggregate's own `@key` members the
    /// same way — in **declaration** order, framed by the type's OWN
    /// extensibility. A `@key` member is not necessarily the first one
    /// declared, so reading these bytes as a truncated sample (this
    /// method's previous implementation) decoded the *first* declared
    /// member's bytes under the *last* declared member's name whenever
    /// they disagreed — silently computing the wrong KeyHash. Measured
    /// against Eclipse Cyclone 11.0.1 (`ddsi_serdata_from_sample(sertype,
    /// SDK_KEY, …)` + `ddsi_serdata_to_ser`) on
    /// `@appendable struct A { long x; @key long id; long y; }`,
    /// `id = 7`: the XCDR2 `serializedKey` is `000900000400000007000000`
    /// — a 4-byte DHEADER over `id` alone, nothing else.
    ///
    /// [`zerodds_wire_codec::decode::serialized_key`] is the real decode
    /// of that wire form: `@key` members only, declaration order, the
    /// type's own extensibility (a DHEADER for `@appendable`/`@mutable`
    /// under XCDR2, an EMHEADER per member for `@mutable`, nothing for
    /// `@final` or under XCDR1) — the wire form Cyclone (and, by XTypes
    /// 1.3 §7.6.8's Step 1 definition, any vendor extracting exactly the
    /// key members) produces. Non-key members come back at their default
    /// value. The KeyHash below is unaffected by that default either
    /// way, since it is derived from the key members alone through the
    /// canonical big-endian PLAIN_CDR2 encoder regardless of the
    /// representation this decode read.
    ///
    /// # Errors
    /// The payload does not decode as this type's key-only wire form in
    /// the given representation — a context the planner cannot describe,
    /// a value the type refuses, or bytes left over after its declared
    /// members and the encapsulation's own padding.
    pub fn read_key_only(
        &self,
        bytes: &[u8],
        representation: u8,
        big_endian: bool,
    ) -> Result<SerializedSample, String> {
        let mut request = self.request(representation, big_endian)?;
        // §7.6.8's canonical Key Holder (`Self::sample` below, always
        // `ForcedFinal` + `AscendingMemberId` via `self.request(2, true)`)
        // is a different reading of "key holder" from the wire's own —
        // this is the wire's: the type's own extensibility framing, in
        // declaration order.
        request.key_holder = zerodds_ast_layout::KeyHolderExtensibility::AsDeclared;
        request.key_holder_order = zerodds_ast_layout::KeyHolderOrder::Declaration;
        let data =
            zerodds_wire_codec::decode::serialized_key::<DynamicData>(&self.ast, &request, bytes)
                .map_err(|e| e.to_string())?;
        self.sample(data, bytes.to_vec(), representation, big_endian)
    }

    /// Encodes `sample`'s instance key as the wire `serializedKey`
    /// (DDSI-RTPS 2.5 §9.4.2.11) a dispose/unregister DATA carries behind
    /// its K flag — the exact inverse of [`Self::read_key_only`]: the
    /// `@key` members only, in declaration order, framed by the type's OWN
    /// extensibility, in `representation` (0 = XCDR1, 2 = XCDR2) and
    /// `big_endian`.
    ///
    /// Unlike `Self::request` (used by [`Self::read_body`]/[`Self::write_body`],
    /// always `PlainCdr1`/`PlainCdr2`), the encapsulation kind this chooses
    /// reflects the type's own extensibility under XCDR2 — `DelimitedCdr`
    /// for `@appendable`, `PlCdr2` for `@mutable`, `PlainCdr2` only for
    /// `@final` — matching what Eclipse Cyclone 11.0.1 and RTI Connext
    /// 7.7.0 were both measured to write for their own `serializedKey`
    /// (never `PlainCdr2` for anything but a `@final` type); under XCDR1,
    /// `PlCdr1` for `@mutable` and `PlainCdr1` otherwise, the same split
    /// `crate::runtime::user_payload_encap` uses for a full sample's own
    /// encapsulation header.
    ///
    /// # Errors
    /// The sample belongs to another topic type, the type has no key, or
    /// the codec refuses a `@key` member's value.
    pub fn write_key_only(
        &self,
        sample: &SerializedSample,
        representation: u8,
        big_endian: bool,
    ) -> Result<Vec<u8>, String> {
        if sample.identity != self.metadata.type_identifier {
            return Err("sample belongs to another topic type".into());
        }
        self.write_key_only_data(&sample.data, representation, big_endian)
    }

    /// [`Self::write_key_only`] for a value that holds the `@key` members
    /// and nothing else, which is what an XML `serializedKey`
    /// (DDS-XTypes 1.3 §7.4.4) parses to.
    ///
    /// # Errors
    /// The type has no key, or the codec refuses a `@key` member's value.
    pub fn write_key_only_data(
        &self,
        data: &DynamicData,
        representation: u8,
        big_endian: bool,
    ) -> Result<Vec<u8>, String> {
        if !self.metadata.keyed {
            return Err("type has no key".into());
        }
        let encoding = match (representation, self.metadata.extensibility) {
            (0, Extensibility::Mutable) => zerodds_ast_layout::Encoding::PlCdr1,
            (0, Extensibility::Final | Extensibility::Appendable) => {
                zerodds_ast_layout::Encoding::PlainCdr1
            }
            (_, Extensibility::Final) => zerodds_ast_layout::Encoding::PlainCdr2,
            (_, Extensibility::Appendable) => zerodds_ast_layout::Encoding::DelimitedCdr,
            (_, Extensibility::Mutable) => zerodds_ast_layout::Encoding::PlCdr2,
        };
        let mut request = with_codec_overrides(ProbeRequest::in_dialect(
            &self.model_name,
            encoding,
            if big_endian {
                Endianness::Big
            } else {
                Endianness::Little
            },
            self.dialect,
        ));
        request.key_holder = zerodds_ast_layout::KeyHolderExtensibility::AsDeclared;
        request.key_holder_order = zerodds_ast_layout::KeyHolderOrder::Declaration;
        request.emit = Emit::PayloadWithEncapsulation;
        zerodds_wire_codec::encode::serialized_key(&self.ast, &request, data)
            .map_err(|e| e.to_string())
    }

    fn sample(
        &self,
        data: DynamicData,
        body: Vec<u8>,
        representation: u8,
        big_endian: bool,
    ) -> Result<SerializedSample, String> {
        let key = if self.metadata.keyed {
            let mut request = self.request(2, true)?;
            let holder = zerodds_wire_codec::encode::key_holder(&self.ast, &request, &data)
                .map_err(|e| e.to_string())?;
            request.emit = Emit::KeyHash;
            // The shared encoder uses the declared maximum-size decision, not
            // this sample's holder length. No saturated maximum is advertised.
            let hash: [u8; 16] = zerodds_wire_codec::encode(&self.ast, &request, &data)
                .map_err(|e| e.to_string())?
                .try_into()
                .map_err(|_| "invalid KeyHash length")?;
            Some((hash, holder))
        } else {
            None
        };
        let segments: Vec<String> = self
            .model_name
            .split("::")
            .filter(|segment| !segment.is_empty())
            .map(str::to_string)
            .collect();
        let root = self
            .ast
            .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(segments))
            .ok()
            .and_then(zerodds_ast::AnyId::as_type);
        Ok(SerializedSample {
            data,
            identity: self.metadata.type_identifier.clone(),
            body,
            representation,
            big_endian,
            key,
            model: root.map(|root| SampleModel {
                ast: Arc::clone(&self.ast),
                root,
            }),
        })
    }

    /// Encode a validated sample in the requested binding representation.
    /// # Errors
    /// Sample belongs to another type, or the codec refuses the value.
    pub fn write_body(
        &self,
        sample: &SerializedSample,
        representation: u8,
        big_endian: bool,
    ) -> Result<Vec<u8>, String> {
        if sample.identity != self.metadata.type_identifier {
            return Err("sample belongs to another topic type".into());
        }
        if sample.representation == representation && sample.big_endian == big_endian {
            return Ok(sample.body.clone());
        }
        zerodds_wire_codec::encode(
            &self.ast,
            &self.request(representation, big_endian)?,
            &sample.data,
        )
        .map_err(|e| e.to_string())
    }

    /// Builds a [`SerializedSample`] from a freshly constructed [`DynamicData`]
    /// rather than decoding one off the wire — the write path a bridge takes
    /// when a value originates *outside* DDS (an OPC-UA client `Write`, a
    /// PubSub `DataSetMessage`) and has to be validated and keyed exactly the
    /// way an incoming DATA payload is, before [`Self::write_body`] can encode
    /// it. Encodes once, in the canonical wire form (XCDR2, little-endian),
    /// so [`Self::write_body`]'s "already matches" fast path applies whenever
    /// the caller asks for that same representation back.
    ///
    /// # Errors
    /// The codec refuses `data` against this type — a missing/extra member,
    /// a value out of the member's declared range, an unresolvable
    /// enumerator, or any other mismatch [`Self::read_body`] would also
    /// reject on the decode side.
    pub fn from_dynamic_data(&self, data: DynamicData) -> Result<SerializedSample, String> {
        let request = self.request(2, false)?;
        let body =
            zerodds_wire_codec::encode(&self.ast, &request, &data).map_err(|e| e.to_string())?;
        self.sample(data, body, 2, false)
    }

    /// The sample's key hash and key holder, for a keyed topic (`None` for a
    /// keyless one). Same value as `TopicCodec::key_material` (that trait is
    /// crate-internal, used generically by `create_datawriter` etc.); this
    /// inherent method is the public accessor a caller holding a validated
    /// [`SerializedSample`] from [`Self::read_body`] or
    /// `Self::decode_key_holder` uses to derive the real per-instance key
    /// — e.g. a byte-forwarding bridge writing a keyed topic without a
    /// compiled type (`write_user_sample_keyed` needs the hash; without it,
    /// every sample collapses onto the one default instance).
    #[must_use]
    pub fn key_material(&self, sample: &SerializedSample) -> Option<([u8; 16], Vec<u8>)> {
        sample.key.clone()
    }

    /// Whether the resolved type has one or more `@key` fields. Same value
    /// as `TopicCodec::extensibility`'s sibling checks internally; public so
    /// a caller that resolved this support from a discovered type (not
    /// caller-asserted `keyed`/`extensibility`, see
    /// [`Self::from_complete_type_objects`]) can learn what the type
    /// actually is instead of guessing.
    #[must_use]
    pub fn is_keyed(&self) -> bool {
        self.metadata.keyed
    }

    /// The resolved type's extensibility (Final/Appendable/Mutable, XTypes
    /// 1.3 §7.2.2) — see [`Self::is_keyed`] for why this is exposed.
    #[must_use]
    pub fn extensibility(&self) -> Extensibility {
        self.metadata.extensibility
    }
}

impl TopicCodec<SerializedSample> for SerializedTypeSupport {
    fn dependent_type_objects(&self) -> &[TypeObject] {
        &self.dependencies
    }
    fn validate_sample(&self, sample: &SerializedSample) -> crate::error::Result<()> {
        if sample.identity != self.metadata.type_identifier {
            return Err(crate::error::DdsError::BadParameter {
                what: "sample belongs to another runtime-described topic type",
            });
        }
        Ok(())
    }
    fn validates_static_metadata(&self) -> bool {
        false
    }
    fn extensibility(&self) -> Extensibility {
        self.metadata.extensibility
    }
    fn encode(
        &self,
        sample: &SerializedSample,
        out: &mut Vec<u8>,
        xcdr1: bool,
        big_endian: bool,
    ) -> Result<(), EncodeError> {
        out.extend(
            self.write_body(sample, if xcdr1 { 0 } else { 2 }, big_endian)
                .map_err(|_| EncodeError::Invalid {
                    what: "runtime-described payload refused",
                })?,
        );
        Ok(())
    }
    fn decode(
        &self,
        bytes: &[u8],
        representation: u8,
        big_endian: bool,
    ) -> Result<SerializedSample, DecodeError> {
        let representation = if representation == 0 { 0 } else { 2 };
        let request =
            self.request(representation, big_endian)
                .map_err(|_| DecodeError::Invalid {
                    what: "unsupported representation",
                })?;
        let data =
            zerodds_wire_codec::decode::<DynamicData>(&self.ast, &request, bytes).map_err(|e| {
                match e {
                    zerodds_wire_codec::ProbeError::Discarded { .. } => DecodeError::Discarded {
                        what: "runtime-described receiving type discarded the sample",
                    },
                    _ => DecodeError::Invalid {
                        what: "runtime-described payload refused",
                    },
                }
            })?;
        self.sample(data, bytes.to_vec(), representation, big_endian)
            .map_err(|_| DecodeError::Invalid {
                what: "runtime-described key material refused",
            })
    }
    fn key_material(&self, sample: &SerializedSample) -> Option<([u8; 16], Vec<u8>)> {
        sample.key.clone()
    }
    fn decode_key_holder(&self, bytes: &[u8]) -> Result<SerializedSample, DecodeError> {
        let decode = || -> Result<SerializedSample, String> {
            let request = self.request(2, true)?;
            let data =
                zerodds_wire_codec::decode::key_holder::<DynamicData>(&self.ast, &request, bytes)
                    .map_err(|e| e.to_string())?;
            let body = zerodds_wire_codec::encode(&self.ast, &request, &data)
                .map_err(|e| e.to_string())?;
            self.sample(data, body, 2, true)
        };
        decode().map_err(|_| DecodeError::Invalid {
            what: "runtime-described key holder refused",
        })
    }
}

impl SerializedTypeSupport {
    /// The sample whose key a KeyHash spells out: DDSI-RTPS 2.5 §9.6.4.8
    /// Step 5.1 serializes the key holder in big-endian CDR and pads it with
    /// zeros to 16 octets. The holder is decoded from the 16 octets, and
    /// every octet after it must be zero.
    ///
    /// # Errors
    /// `DecodeError::Invalid` if the octets are no holder of this type or
    /// an octet after it is not zero.
    pub fn decode_zero_padded_key_holder(
        &self,
        hash: &[u8; 16],
    ) -> Result<SerializedSample, DecodeError> {
        let refused = || DecodeError::Invalid {
            what: "the KeyHash does not spell a key holder of this type",
        };
        let decode = || -> Result<Option<SerializedSample>, String> {
            let request = self.request(2, true)?;
            let (data, consumed) = zerodds_wire_codec::decode::key_holder_prefix::<DynamicData>(
                &self.ast, &request, hash,
            )
            .map_err(|e| e.to_string())?;
            if hash[consumed..].iter().any(|octet| *octet != 0) {
                return Ok(None);
            }
            let body = zerodds_wire_codec::encode(&self.ast, &request, &data)
                .map_err(|e| e.to_string())?;
            self.sample(data, body, 2, true).map(Some)
        };
        decode().map_err(|_| refused())?.ok_or_else(refused)
    }

    /// The sample as a `DynamicData` of the support's type.
    fn dynamic_of(&self, sample: SerializedSample) -> Result<DynamicSample, String> {
        Ok(DynamicSample::with_value(self.get_type()?, sample.data))
    }
}

impl TopicCodec<DynamicSample> for SerializedTypeSupport {
    fn dependent_type_objects(&self) -> &[TypeObject] {
        &self.dependencies
    }
    fn validate_sample(&self, sample: &DynamicSample) -> crate::error::Result<()> {
        let own = self
            .get_type()
            .map_err(|_| crate::error::DdsError::BadParameter {
                what: "the support's type is not in its model",
            })?;
        if !sample.type_().equals_ignoring_annotations(&own) {
            return Err(crate::error::DdsError::BadParameter {
                what: "the DynamicData is of another type than the topic's",
            });
        }
        Ok(())
    }
    fn validates_static_metadata(&self) -> bool {
        false
    }
    fn extensibility(&self) -> Extensibility {
        self.metadata.extensibility
    }
    fn encode(
        &self,
        sample: &DynamicSample,
        out: &mut Vec<u8>,
        xcdr1: bool,
        big_endian: bool,
    ) -> Result<(), EncodeError> {
        let invalid = |what| EncodeError::Invalid { what };
        let data = sample
            .snapshot()
            .map_err(|_| invalid("the DynamicData has no value"))?;
        let request = self
            .request(if xcdr1 { 0 } else { 2 }, big_endian)
            .map_err(|_| invalid("unsupported representation"))?;
        out.extend(
            zerodds_wire_codec::encode(&self.ast, &request, &data)
                .map_err(|_| invalid("runtime-described payload refused"))?,
        );
        Ok(())
    }
    fn decode(
        &self,
        bytes: &[u8],
        representation: u8,
        big_endian: bool,
    ) -> Result<DynamicSample, DecodeError> {
        let sample = <Self as TopicCodec<SerializedSample>>::decode(
            self,
            bytes,
            representation,
            big_endian,
        )?;
        self.dynamic_of(sample).map_err(|_| DecodeError::Invalid {
            what: "the support's type is not in its model",
        })
    }
    fn key_material(&self, sample: &DynamicSample) -> Option<([u8; 16], Vec<u8>)> {
        let data = sample.snapshot().ok()?;
        let body = self.from_dynamic_data(data).ok()?;
        body.key
    }
    fn decode_key_holder(&self, bytes: &[u8]) -> Result<DynamicSample, DecodeError> {
        let sample = <Self as TopicCodec<SerializedSample>>::decode_key_holder(self, bytes)?;
        self.dynamic_of(sample).map_err(|_| DecodeError::Invalid {
            what: "the support's type is not in its model",
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {

    /// Annex B.2 ENUMERATEDVALUE ("the name of the enumeration label
    /// enclosed in single quotes") and CHARVALUE ("a single character
    /// enclosed between single quotes"): a filter compares an enumerated
    /// member with its label and a character member with its character.
    #[test]
    fn a_filter_compares_an_enumerated_and_a_character_member() {
        use zerodds_sql_filter::{RowAccess, Value};
        struct Row<'a>(&'a zerodds_types::dynamic::DynamicData);
        impl RowAccess for Row<'_> {
            fn get(&self, path: &str) -> Option<Value> {
                super::dynamic_field_value(self.0, None, path)
            }
        }
        let data = zerodds_types::dynamic::DynamicData::Aggregate(vec![
            (
                "color".into(),
                zerodds_types::dynamic::DynamicData::Enumerator("RED".into()),
            ),
            (
                "grade".into(),
                zerodds_types::dynamic::DynamicData::character(
                    zerodds_ast::PrimitiveKind::Char,
                    'A',
                ),
            ),
        ]);
        let row = Row(&data);
        let yes = zerodds_sql_filter::parse("color = 'RED' AND grade = 'A'").unwrap();
        assert_eq!(yes.evaluate(&row, &[]), Ok(true));
        let no = zerodds_sql_filter::parse("color = 'BLUE' OR grade = 'B'").unwrap();
        assert_eq!(no.evaluate(&row, &[]), Ok(false));
    }

    /// Serialized samples packed back to back with no length
    /// (DDS-XRCE 1.0 Annex A `sequence<SampleData>`) are told apart by the
    /// type: `read_body_prefix` reads the first and returns where it ended.
    #[test]
    fn read_body_prefix_tells_where_the_first_of_several_packed_samples_ends() {
        let ast = std::sync::Arc::new(
            zerodds_wire_probe::encode::model_from_idl(
                "@final struct Packed { @key long id; string label; };",
                "serialized-type-prefix.idl",
            )
            .expect("idl"),
        );
        let ty = zerodds_types::dynamic::DynamicType::find(&ast, "Packed").expect("type");
        let root = zerodds_types::from_ast::complete_type_object(&ast, ty.id()).expect("object");
        let support = SerializedTypeSupport::from_complete_type_objects("Packed", root, Vec::new())
            .expect("support");
        let sample = |id: i32, label: &str| {
            let mut v = id.to_le_bytes().to_vec();
            v.extend_from_slice(&u32::try_from(label.len() + 1).expect("len").to_le_bytes());
            v.extend_from_slice(label.as_bytes());
            v.push(0);
            v
        };
        let (a, b) = (sample(1, "ab"), sample(2, "cdefg"));
        let mut packed = a.clone();
        packed.extend_from_slice(&b);
        let (first, taken) = support.read_body_prefix(&packed, 2, false).expect("first");
        assert_eq!(taken, a.len());
        assert_eq!(first.body(), a.as_slice());
        let (second, taken2) = support
            .read_body_prefix(&packed[taken..], 2, false)
            .expect("second");
        assert_eq!(taken2, b.len());
        assert_eq!(second.body(), b.as_slice());
        assert_ne!(first.key_hash(), second.key_hash());
        // `read_body` of the whole buffer refuses what follows the first.
        assert!(support.read_body(&packed, 2, false).is_err());
        // A truncated sample is an error.
        assert!(
            support
                .read_body_prefix(&b[..b.len() - 2], 2, false)
                .is_err()
        );
    }

    #[test]
    fn read_body_representation_maps_the_user_sample_tag() {
        assert_eq!(super::read_body_representation(0), 0);
        assert_eq!(super::read_body_representation(1), 2);
    }

    use super::*;

    /// A binding generated in the Fast DDS dialect hands the runtime a
    /// @mutable body in Fast DDS's PL_CDR1 framing: bare parameter ids, true
    /// lengths, a bare 0x3f02 sentinel. The octets are Fast DDS 3.6.1's own,
    /// `RichMPubSubType::serialize` from fastddsgen's output for this IDL.
    /// The support reads them, and derives their key, only in that dialect;
    /// it used to read in the specification's always.
    #[test]
    fn a_support_in_the_fastdds_dialect_reads_the_body_fast_dds_writes() {
        const FAST_DDS: [u8; 36] = [
            0x00, 0x00, 0x04, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x08, 0x00, 0x18, 0x2d,
            0x44, 0x54, 0xfb, 0x21, 0x09, 0x40, 0x02, 0x00, 0x08, 0x00, 0x04, 0x00, 0x00, 0x00,
            0x00, 0x01, 0x02, 0x03, 0x02, 0x3f, 0x00, 0x00,
        ];
        let ast = Arc::new(
            zerodds_wire_probe::encode::model_from_idl(
                "@mutable struct RichM { @key unsigned long id; double value; sequence<octet> blob; };",
                "richm.idl",
            )
            .unwrap(),
        );
        let type_ = zerodds_types::dynamic::DynamicType::find(&ast, "RichM").unwrap();
        let object = TypeObject::Complete(
            zerodds_types::from_ast::complete_type_object(&ast, type_.id()).unwrap(),
        )
        .to_bytes_le()
        .unwrap();
        let support = SerializedTypeSupport::from_type_object("RichM", &object, true, 2).unwrap();
        assert_eq!(
            support.dialect(),
            zerodds_ast_layout::Dialect::Specification
        );
        assert!(support.read_body(&FAST_DDS, 0, false).is_err());
        let fastdds = support.in_dialect(zerodds_ast_layout::Dialect::FastDds);
        let sample = fastdds.read_body(&FAST_DDS, 0, false).unwrap();
        let (hash, holder) = fastdds.key_material(&sample).unwrap();
        assert_eq!(holder, 1u32.to_be_bytes());
        assert_eq!(hash[..4], [0, 0, 0, 1]);
    }

    /// `wire.codec_dialect: opensplice` gives the runtime OpenSplice's
    /// KeyHash: key fields back to back, as OpenSplice 6.9 puts them in
    /// `PID_KEY_HASH` (measured, `tools/keyhash-probe/opensplice`). The
    /// specification's dialect keeps §7.6.8's aligned holder.
    #[test]
    fn the_opensplice_dialect_gives_the_runtime_opensplices_key_hash() {
        // `@key octet a; @key long b`, a=0x11, b=0x22334455, v=1, XCDR1 LE.
        const BODY: [u8; 12] = [
            0x11, 0x00, 0x00, 0x00, 0x55, 0x44, 0x33, 0x22, 0x01, 0x00, 0x00, 0x00,
        ];
        let ast = Arc::new(
            zerodds_wire_probe::encode::model_from_idl(
                "@final struct PadKey { @key octet a; @key long b; long v; };",
                "padkey.idl",
            )
            .unwrap(),
        );
        let type_ = zerodds_types::dynamic::DynamicType::find(&ast, "PadKey").unwrap();
        let object = TypeObject::Complete(
            zerodds_types::from_ast::complete_type_object(&ast, type_.id()).unwrap(),
        )
        .to_bytes_le()
        .unwrap();
        let support = SerializedTypeSupport::from_type_object("PadKey", &object, true, 0).unwrap();
        let spec = support.read_body(&BODY, 0, false).unwrap();
        let (hash, _) = support.key_material(&spec).unwrap();
        assert_eq!(
            hash,
            [
                0x11, 0, 0, 0, 0x22, 0x33, 0x44, 0x55, 0, 0, 0, 0, 0, 0, 0, 0
            ]
        );
        let opensplice = support.in_dialect(zerodds_ast_layout::Dialect::OpenSplice);
        let sample = opensplice.read_body(&BODY, 0, false).unwrap();
        let (hash, _) = opensplice.key_material(&sample).unwrap();
        assert_eq!(
            hash,
            [
                0x11, 0x22, 0x33, 0x44, 0x55, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
            ]
        );
    }

    fn dynamic_support(
        idl: &str,
        name: &str,
    ) -> (
        Arc<SerializedTypeSupport>,
        zerodds_types::dynamic::DynamicType,
    ) {
        let ast = Arc::new(zerodds_wire_probe::encode::model_from_idl(idl, "support.idl").unwrap());
        let type_ = zerodds_types::dynamic::DynamicType::find(&ast, name).unwrap();
        let support = SerializedTypeSupport::create_type_support(&type_).unwrap();
        (support, type_)
    }

    /// DDS-XTypes 1.3 §7.6.6.1.5 and §7.6.2: a support is created for a
    /// structure or a union that is not nested, with a copy of the type; a
    /// nested type, an enumeration and a primitive are refused.
    #[test]
    fn create_type_support_refuses_a_nested_type_and_a_non_aggregate() {
        let (support, type_) = dynamic_support(
            "@final struct Topical { @key long id; double v; };
             @nested @final struct Part { long x; };
             enum Color { RED, GREEN };",
            "Topical",
        );
        assert_eq!(support.get_type_name(), "Topical");
        assert!(support.is_keyed());
        // The support's type is a copy: another model, equal type.
        let own = support.get_type().unwrap();
        assert!(!Arc::ptr_eq(own.ast(), type_.ast()));
        assert!(own.equals(&type_));

        let ast = type_.ast();
        let part = zerodds_types::dynamic::DynamicType::find(ast, "Part").unwrap();
        let error = SerializedTypeSupport::create_type_support(&part).unwrap_err();
        assert!(error.contains("nested"), "{error}");
        let color = zerodds_types::dynamic::DynamicType::find(ast, "Color").unwrap();
        let error = SerializedTypeSupport::create_type_support(&color).unwrap_err();
        assert!(error.contains("topic type"), "{error}");
        // §7.6.6.1.6: a nil support is deleted without effect, a real one is
        // dropped.
        SerializedTypeSupport::delete_type_support(None);
        SerializedTypeSupport::delete_type_support(Some(support));
    }

    /// DDS-XTypes 1.3 §7.6.6.1.2 (Table 64): `create_dynamic_sample` and
    /// `create_sample` convert between a sample and a `DynamicData`, and a
    /// `DynamicData` of another type is refused.
    #[test]
    fn a_sample_converts_to_dynamic_data_and_back() {
        let (support, _) = dynamic_support(
            "@final struct Pair { @key long id; double v; };
             @final struct Other { long id; };",
            "Pair",
        );
        let own = support.get_type().unwrap();
        let mut dynamic = zerodds_types::dynamic::DynamicSample::new(&own).unwrap();
        let id = dynamic.get_member_id_by_name("id").unwrap();
        let v = dynamic.get_member_id_by_name("v").unwrap();
        dynamic.set_int32_value(id, 7).unwrap();
        dynamic.set_float64_value(v, 2.5).unwrap();

        let sample = support.create_sample(&dynamic).unwrap();
        assert!(sample.key_hash().is_some());
        let back = support.create_dynamic_sample(&sample).unwrap();
        assert!(back.equals(&dynamic));
        assert_eq!(back.get_int32_value(id).unwrap(), 7);
        assert_eq!(back.get_float64_value(v).unwrap(), 2.5);

        // A DynamicData of another type.
        let other_ast = Arc::new(
            zerodds_wire_probe::encode::model_from_idl(
                "@final struct Other { long id; };",
                "o.idl",
            )
            .unwrap(),
        );
        let other = zerodds_types::dynamic::DynamicType::find(&other_ast, "Other").unwrap();
        let foreign = zerodds_types::dynamic::DynamicSample::new(&other).unwrap();
        assert!(support.create_sample(&foreign).is_err());
    }

    /// DDS-XTypes 1.3 §7.6.7.1: a filter reaches an element of a sequence
    /// by its zero-based subscript and a bitmask flag by its quoted name.
    #[test]
    fn a_filter_subscripts_a_sequence_and_a_bitmask() {
        let ast = Arc::new(
            zerodds_wire_probe::encode::model_from_idl(
                "@bit_bound(8) bitmask Flags { READ, WRITE };
                 @final struct S { sequence<long> q; Flags f; };",
                "subscripts.idl",
            )
            .unwrap(),
        );
        let type_ = zerodds_types::dynamic::DynamicType::find(&ast, "S").unwrap();
        let object = TypeObject::Complete(
            zerodds_types::from_ast::complete_type_object(&ast, type_.id()).unwrap(),
        )
        .to_bytes_le()
        .unwrap();
        let dependencies: Vec<Vec<u8>> =
            zerodds_types::from_ast::dependency_type_objects(&ast, type_.id())
                .unwrap()
                .into_iter()
                .map(|object| TypeObject::Complete(object).to_bytes_le().unwrap())
                .collect();
        let support =
            SerializedTypeSupport::from_type_objects("S", &object, &dependencies, false, 0)
                .unwrap();
        // XCDR2: sequence<long> = count + elements, then the 8-bit mask
        // with only WRITE (bit 1) set.
        let mut body = 2u32.to_le_bytes().to_vec();
        body.extend_from_slice(&7i32.to_le_bytes());
        body.extend_from_slice(&9i32.to_le_bytes());
        body.push(0b10);
        let sample = support.read_body(&body, 2, false).unwrap();
        let value = |path: &str| sample.field_value(path);
        assert_eq!(value("q[0]"), Some(zerodds_sql_filter::Value::Int(7)));
        assert_eq!(value("q[1]"), Some(zerodds_sql_filter::Value::Int(9)));
        assert_eq!(value("q[2]"), None);
        assert_eq!(
            value("f['WRITE']"),
            Some(zerodds_sql_filter::Value::Bool(true))
        );
        assert_eq!(
            value("f['READ']"),
            Some(zerodds_sql_filter::Value::Bool(false))
        );
    }

    /// DDS-XTypes 1.3 §7.6.7.1: `['NAME']` is the value of a map for that
    /// key, "the key is a string even when the map's key type is an integer
    /// type". §7.6.7.2: an optional member that is absent equals `null`.
    ///
    /// The dynamic form keeps a map as a collection of pairs and does not
    /// say which collections are maps, and an absent optional member is no
    /// entry, which a name the type does not have is not either; the type
    /// the sample was read as decides both.
    #[test]
    fn a_filter_reaches_a_map_value_and_tells_an_absent_optional_member_from_no_field() {
        let ast = Arc::new(
            zerodds_wire_probe::encode::model_from_idl(
                "@final struct S {
                     map<string, long, 4> m;
                     map<long, long, 4> n;
                     @optional long o;
                 };",
                "maps.idl",
            )
            .unwrap(),
        );
        let type_ = zerodds_types::dynamic::DynamicType::find(&ast, "S").unwrap();
        let object = TypeObject::Complete(
            zerodds_types::from_ast::complete_type_object(&ast, type_.id()).unwrap(),
        )
        .to_bytes_le()
        .unwrap();
        let support =
            SerializedTypeSupport::from_type_objects("S", &object, &[], false, 0).unwrap();
        let long = |value: i128| DynamicData::integer(zerodds_ast::PrimitiveKind::Int32, value);
        let pair = |key: DynamicData, value: DynamicData| DynamicData::Collection(vec![key, value]);
        let sample_with = |optional: Option<i128>| {
            let mut members = vec![
                (
                    "m".to_string(),
                    DynamicData::Collection(vec![
                        pair(DynamicData::Text("a".into()), long(5)),
                        pair(DynamicData::Text("b".into()), long(7)),
                    ]),
                ),
                (
                    "n".to_string(),
                    DynamicData::Collection(vec![pair(long(3), long(30))]),
                ),
            ];
            if let Some(value) = optional {
                members.push(("o".to_string(), long(value)));
            }
            let data = DynamicData::Aggregate(members);
            let body = zerodds_wire_codec::encode(
                &support.ast,
                &support.request(2, false).unwrap(),
                &data,
            )
            .unwrap();
            support.read_body(&body, 2, false).unwrap()
        };

        let absent = sample_with(None);
        let value = |path: &str| absent.field_value(path);
        assert_eq!(value("m['a']"), Some(zerodds_sql_filter::Value::Int(5)));
        assert_eq!(value("m['b']"), Some(zerodds_sql_filter::Value::Int(7)));
        assert_eq!(value("m['zz']"), None, "a key the map does not hold");
        assert_eq!(
            value("n['3']"),
            Some(zerodds_sql_filter::Value::Int(30)),
            "the key of an integer map is spelled as a string"
        );
        assert_eq!(value("n[3]"), Some(zerodds_sql_filter::Value::Int(30)));
        assert_eq!(
            value("o"),
            Some(zerodds_sql_filter::Value::Null),
            "an absent optional member equals null"
        );
        assert_eq!(
            value("nosuchmember"),
            None,
            "and a name the type lacks is no field"
        );

        let present = sample_with(Some(11));
        assert_eq!(
            present.field_value("o"),
            Some(zerodds_sql_filter::Value::Int(11))
        );
    }

    #[test]
    fn runtime_support_preserves_identity_before_instance_mutation() {
        let ast = Arc::new(
            zerodds_wire_probe::encode::model_from_idl(
                "@final struct A { @key long id; }; @final struct B { @key long id; };",
                "runtime-support.idl",
            )
            .unwrap(),
        );
        let object = |name| {
            let type_ = zerodds_types::dynamic::DynamicType::find(&ast, name).unwrap();
            TypeObject::Complete(
                zerodds_types::from_ast::complete_type_object(&ast, type_.id()).unwrap(),
            )
            .to_bytes_le()
            .unwrap()
        };
        let bytes = object("A");
        let a = SerializedTypeSupport::from_type_object("A", &bytes, true, 0).unwrap();
        let b = SerializedTypeSupport::from_type_object("B", &object("B"), true, 0).unwrap();
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(SerializedTypeSupport::from_type_object("A", &trailing, true, 0).is_err());
        assert!(SerializedTypeSupport::from_type_object("A", &bytes, false, 0).is_err());
        let sample = a.read_body(&177i32.to_le_bytes(), 2, false).unwrap();
        let wrong = b.read_body(&177i32.to_le_bytes(), 2, false).unwrap();
        assert_eq!(sample.representation(), 2);
        assert!(!sample.big_endian());
        assert_eq!(sample.body(), &177i32.to_le_bytes());
        let (hash, holder) = a.key_material(&sample).unwrap();
        assert_eq!(holder, 177i32.to_be_bytes());
        assert_eq!(&hash[..4], holder.as_slice());
        assert_eq!(
            TopicCodec::<SerializedSample>::decode_key_holder(&*a, &holder)
                .unwrap()
                .field_value("id"),
            Some(zerodds_sql_filter::Value::Int(177))
        );
        let p = crate::DomainParticipantFactory::instance()
            .create_participant_offline(177, crate::DomainParticipantQos::default());
        let topic = p
            .create_serialized_topic("identity", crate::TopicQos::default(), Arc::clone(&a))
            .unwrap();
        assert_eq!(topic.type_name(), "A");
        // XTypes 1.3 §7.6.4.1: another type under the name of a topic is
        // another topic of that name, with a QoS consistent with the first's.
        assert!(
            p.create_serialized_topic("identity", crate::TopicQos::default(), b)
                .is_ok()
        );
        assert_eq!(p.topics_len(), 2);
        let publisher = p.create_publisher(crate::PublisherQos::default());
        let writer = publisher
            .create_datawriter(&topic, crate::DataWriterQos::default())
            .unwrap();
        let handle = writer.register_instance(&sample).unwrap();
        assert!(writer.register_instance(&wrong).is_err());
        assert!(writer.lookup_instance(&wrong).is_nil());
        assert!(writer.dispose(&wrong, handle).is_err());
        assert!(writer.unregister_instance(&wrong, handle).is_err());
        assert_eq!(writer.lookup_instance(&sample), handle);
        writer.dispose(&sample, handle).unwrap();
    }

    /// DDS-XTypes 1.3 Table 65 `register_type`: the type is registered with
    /// the participant under its own name or another, again under the same
    /// name is not an error, another type under a taken name is, and a
    /// topic is created from the registered name.
    #[test]
    fn register_type_registers_a_type_under_a_name() {
        let ast = Arc::new(
            zerodds_wire_probe::encode::model_from_idl(
                "@final struct First { @key long id; }; @final struct Second { long value; };",
                "register-type.idl",
            )
            .unwrap(),
        );
        let support = |name: &str, keyed| {
            let type_ = zerodds_types::dynamic::DynamicType::find(&ast, name).unwrap();
            let object = TypeObject::Complete(
                zerodds_types::from_ast::complete_type_object(&ast, type_.id()).unwrap(),
            )
            .to_bytes_le()
            .unwrap();
            SerializedTypeSupport::from_type_object(name, &object, keyed, 0).unwrap()
        };
        let (first, second) = (support("First", true), support("Second", false));
        let p = crate::DomainParticipantFactory::instance()
            .create_participant_offline(178, crate::DomainParticipantQos::default());
        // Nothing is registered yet.
        assert!(
            p.create_registered_topic("t", "First", crate::TopicQos::default())
                .is_err()
        );
        first.register_type(&p, None).unwrap();
        first.register_type(&p, Some("First")).unwrap();
        first.register_type(&p, None).unwrap();
        assert_eq!(
            p.create_registered_topic("t", "First", crate::TopicQos::default())
                .unwrap()
                .type_name(),
            "First"
        );
        // Another type under a taken name.
        assert!(second.register_type(&p, Some("First")).is_err());
        // The same type under another name: the topic announces that name,
        // and the type is still the model's.
        first.register_type(&p, Some("Alias")).unwrap();
        let aliased = p
            .create_registered_topic("u", "Alias", crate::TopicQos::default())
            .unwrap();
        assert_eq!(aliased.type_name(), "Alias");
        let renamed = first.with_type_name("Alias").unwrap();
        assert_eq!(renamed.get_type_name(), "Alias");
        assert_eq!(renamed.get_type().unwrap().name(), "First");
        let body = renamed.read_body(&5i32.to_le_bytes(), 2, false).unwrap();
        assert!(renamed.key_material(&body).is_some());
        // Not a name.
        assert!(first.register_type(&p, Some("")).is_err());
        assert!(first.with_type_name(&"x".repeat(257)).is_err());
    }

    #[test]
    fn key_hash_accessor_matches_key_material_and_is_none_when_unkeyed() {
        let ast = Arc::new(
            zerodds_wire_probe::encode::model_from_idl(
                "@final struct Keyed { @key long id; long value; }; \
                 @final struct Unkeyed { long value; };",
                "key-hash-accessor.idl",
            )
            .unwrap(),
        );
        let object = |name| {
            let type_ = zerodds_types::dynamic::DynamicType::find(&ast, name).unwrap();
            TypeObject::Complete(
                zerodds_types::from_ast::complete_type_object(&ast, type_.id()).unwrap(),
            )
            .to_bytes_le()
            .unwrap()
        };
        let keyed =
            SerializedTypeSupport::from_type_object("Keyed", &object("Keyed"), true, 0).unwrap();
        let mut body = 7i32.to_le_bytes().to_vec(); // id
        body.extend_from_slice(&99i32.to_le_bytes()); // value
        let sample = keyed.read_body(&body, 2, false).unwrap();
        let hash = sample.key_hash().expect("keyed type has a KeyHash");
        let (expected_hash, _holder) = keyed.key_material(&sample).unwrap();
        assert_eq!(hash, expected_hash);

        let unkeyed =
            SerializedTypeSupport::from_type_object("Unkeyed", &object("Unkeyed"), false, 0)
                .unwrap();
        let unkeyed_sample = unkeyed.read_body(&42i32.to_le_bytes(), 2, false).unwrap();
        assert_eq!(unkeyed_sample.key_hash(), None);
    }
}
