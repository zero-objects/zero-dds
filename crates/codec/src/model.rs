// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A described type's model, its layout plans and its identity, built once
//! per type and kept.
//!
//! The description becomes IDL ([`crate::idl::idl_of`]), the IDL becomes
//! the model through `zerodds-idl-compose`, and the model goes to the same
//! three places a `.idl` file's does: `zerodds-ast-layout` plans it,
//! `zerodds-types` names it (`TypeObject`, `TypeIdentifier`), and the
//! plans are copied into the [`PlanSet`]s the serializer executes.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use zerodds_ast::facet::Extensibility;
use zerodds_ast::{Ast, CharWidth, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::key::{Encoding, Endianness, LayoutKey, Purpose};
use zerodds_ast_layout::keyhash::KeyHashForm;
use zerodds_ast_layout::xcdr::DepCollector;
use zerodds_ast_layout::{ElementPadding, MemberHeader};
use zerodds_types::TypeIdentifier;
use zerodds_types::type_object::TypeObject;
use zerodds_wire::ParameterRole;
use zerodds_wire::codec::{
    BranchPlan, DdsTypeInfo, Header, HolderPlan, KeyPlan, MemberFrame, PlanSet, Primitive, Root,
    StructPlan, TypeDesc, UnionPlan, ValuePlan,
};

/// Why a described type has no model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelError {
    /// The type.
    pub type_name: String,
    /// What went wrong, as the stage that refused it says.
    pub detail: String,
}

impl core::fmt::Display for ModelError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}: {}", self.type_name, self.detail)
    }
}

impl std::error::Error for ModelError {}

/// One described type's model, plans and identity.
#[derive(Debug)]
pub struct Model {
    /// The scoped name, a generic type's with its arguments spelled in.
    pub name: &'static str,
    /// The IDL the description stands for, every reached type included.
    pub idl: String,
    /// The plans of the XCDR2 encapsulation, with the key holder.
    pub xcdr2: PlanSet,
    /// The plans of the XCDR1 encapsulation, or why there are none (XCDR1
    /// cannot frame every type XCDR2 can).
    pub xcdr1: Result<PlanSet, String>,
    /// The complete `TypeIdentifier` (XTypes 1.3 §7.3.4.6).
    pub type_identifier: TypeIdentifier,
    /// The complete `TypeObject`, when it survives its own round trip.
    pub type_object: Option<TypeObject>,
    /// The `TypeObject`s it names by hash.
    pub dependencies: Vec<TypeObject>,
    /// The strongly connected component a recursive type is in.
    pub component: Option<zerodds_types::resolve::TypeComponent>,
    /// §7.6.8 step 5: `Some(16)` when the holder is at most sixteen octets
    /// and is the hash itself, `None` for its MD5.
    pub key_holder_max_size: Option<usize>,
}

/// The model of `T`, built on the first call and kept for the process.
///
/// Kept by the address of `T`'s description ([`DdsTypeInfo::LAYOUT_REF`]):
/// a type with a lifetime has no `TypeId`, and each instantiation of a
/// generic type has a description of its own. A constant may be
/// duplicated per codegen unit; the second address builds the same model
/// once more, which is the cost of a duplicate and not a wrong answer.
///
/// # Errors
/// [`ModelError`] when the description does not compose, plan or name:
/// a shape IDL cannot declare, or a model the planner refuses.
pub fn model<T: DdsTypeInfo>() -> Result<&'static Model, ModelError> {
    model_of(T::LAYOUT_REF)
}

/// [`model`] for a description.
///
/// # Errors
/// As [`model`].
pub fn model_of(desc: &'static TypeDesc) -> Result<&'static Model, ModelError> {
    type Models = HashMap<usize, Result<&'static Model, ModelError>>;
    static MODELS: OnceLock<Mutex<Models>> = OnceLock::new();
    let models = MODELS.get_or_init(|| Mutex::new(HashMap::new()));
    let key = core::ptr::from_ref(desc).addr();
    if let Some(found) = models.lock().map_err(|_| poisoned(desc.name))?.get(&key) {
        return found.clone();
    }
    // Built outside the lock: a build reaches no other type's model, but it
    // is the expensive part and nothing else need wait for it.
    let built = build(desc).map(|model| &*Box::leak(Box::new(model)));
    let mut models = models.lock().map_err(|_| poisoned(desc.name))?;
    models.entry(key).or_insert(built).clone()
}

fn poisoned(name: &str) -> ModelError {
    refused(
        name,
        "the model cache was poisoned by a panic in another thread",
    )
}

fn refused(name: &str, detail: impl Into<String>) -> ModelError {
    ModelError {
        type_name: name.to_string(),
        detail: detail.into(),
    }
}

/// Build the model of one description.
///
/// # Errors
/// As [`model`].
pub fn build(desc: &'static TypeDesc) -> Result<Model, ModelError> {
    let scoped = desc.scoped_name();
    let name: &str = &scoped;
    let idl = crate::idl::idl_of(desc).map_err(|error| refused(name, error.to_string()))?;
    let composed = zerodds_idl_compose::compose_source(
        "derived.idl",
        &idl,
        &zerodds_idl_compose::PathLoader::bare(),
    );
    if composed.failed() {
        let errors: Vec<String> = composed
            .diagnostics
            .iter()
            .map(|d| d.message.clone())
            .collect();
        return Err(refused(
            name,
            format!("its IDL does not compose: {}\n{idl}", errors.join("; ")),
        ));
    }
    let ast = composed.ast;
    let root = find(&ast, name).ok_or_else(|| refused(name, "its IDL declares no such type"))?;

    let xcdr2 = plan_set(&ast, root, Version::Two).map_err(|detail| refused(name, detail))?;
    let xcdr1 = plan_set(&ast, root, Version::One);

    // As the generated `DdsType` impl of `zerodds-build` states it: the
    // complete equivalence hash, or a strongly connected component's
    // identifier; any other shape is `TypeIdentifier::None`, and matching
    // falls back to the type name (DDS 1.4 §2.2.3).
    let type_identifier = match zerodds_types::from_ast::type_identifier(
        &ast,
        root,
        zerodds_types::from_ast::Form::Complete,
    ) {
        Ok(id @ TypeIdentifier::EquivalenceHashComplete(_)) => id,
        Ok(TypeIdentifier::StronglyConnectedComponent(scc))
            if scc.equivalence_kind == zerodds_types::type_identifier::kinds::EK_COMPLETE =>
        {
            TypeIdentifier::StronglyConnectedComponent(scc)
        }
        _ => TypeIdentifier::None,
    };
    // The object only if it survives its own round trip, as the generated
    // `DdsType` impl of `zerodds-build` keeps it.
    let type_object = zerodds_types::from_ast::complete_type_object(&ast, root)
        .ok()
        .map(TypeObject::Complete)
        .filter(|object| {
            object
                .to_bytes_le()
                .is_ok_and(|bytes| TypeObject::from_bytes_le(&bytes).is_ok())
        });
    let dependencies = if type_object.is_none() {
        Vec::new()
    } else {
        zerodds_types::from_ast::dependency_type_objects(&ast, root).unwrap_or_default()
    };
    let dependencies = dependencies
        .into_iter()
        .map(TypeObject::Complete)
        .filter(|object| {
            object
                .to_bytes_le()
                .is_ok_and(|bytes| TypeObject::from_bytes_le(&bytes).is_ok())
        })
        .collect();
    let component = if matches!(
        type_identifier,
        TypeIdentifier::StronglyConnectedComponent(_)
    ) {
        zerodds_types::from_ast::type_component(&ast, root)
            .ok()
            .flatten()
    } else {
        None
    };
    let key_holder_max_size = xcdr2.key.as_ref().filter(|key| key.zero_padded).map(|_| 16);
    // Leaked once per type: `DdsType::type_name` hands out a `&'static
    // str`, and a generic type's name exists only at run time.
    let leaked: &'static str = match scoped {
        std::borrow::Cow::Borrowed(name) => name,
        std::borrow::Cow::Owned(ref name) => Box::leak(name.clone().into_boxed_str()),
    };
    Ok(Model {
        name: leaked,
        idl,
        xcdr2,
        xcdr1,
        type_identifier,
        type_object,
        dependencies,
        component,
        key_holder_max_size,
    })
}

/// The type of the scoped name `name` (`app::Track`).
fn find(ast: &Ast, name: &str) -> Option<Id<TypeNode>> {
    ast.types()
        .find(|(id, node)| {
            !node.name.is_empty()
                && ast
                    .scoped_name_of(zerodds_ast::AnyId::Type(*id, node.node_id))
                    .is_some_and(|scoped| {
                        let text = scoped.to_string();
                        text.strip_prefix("::").unwrap_or(&text) == name
                    })
        })
        .map(|(id, _)| id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Version {
    One,
    Two,
}

/// The encoding a struct's own extensibility asks for, as `zerodds-build`
/// plans each generated type under it.
const fn encoding_for(version: Version, extensibility: Extensibility) -> Encoding {
    match (version, extensibility) {
        (Version::Two, Extensibility::Final) => Encoding::PlainCdr2,
        (Version::Two, Extensibility::Appendable) => Encoding::DelimitedCdr,
        (Version::Two, Extensibility::Mutable) => Encoding::PlCdr2,
        (Version::One, Extensibility::Final | Extensibility::Appendable) => Encoding::PlainCdr1,
        (Version::One, Extensibility::Mutable) => Encoding::PlCdr1,
    }
}

/// The plans of `root` and every struct and union it reaches, for one
/// version, and the key holder under XCDR2.
fn plan_set(ast: &Ast, root: Id<TypeNode>, version: Version) -> Result<PlanSet, String> {
    let mut builder = Builder {
        ast,
        version,
        structs: Vec::new(),
        unions: Vec::new(),
        holders: Vec::new(),
        index: HashMap::new(),
        union_index: HashMap::new(),
        holder_index: HashMap::new(),
        key_union_index: HashMap::new(),
    };
    let is_union = matches!(
        ast.type_(root).map(|node| &node.kind),
        Ok(TypeKind::Union { .. })
    );
    let root_plan = if is_union {
        Root::Union(builder.union_plan_under(root, None, 0)?)
    } else {
        Root::Struct(builder.struct_plan(root, 0)?)
    };
    let key = if version == Version::Two {
        builder.key_plan(root, is_union)?
    } else {
        None
    };
    Ok(PlanSet {
        structs: builder.structs,
        unions: builder.unions,
        holders: builder.holders,
        root: root_plan,
        key,
        max_align: if version == Version::Two { 4 } else { 8 },
    })
}

struct Builder<'a> {
    ast: &'a Ast,
    version: Version,
    structs: Vec<StructPlan>,
    unions: Vec<UnionPlan>,
    holders: Vec<HolderPlan>,
    index: HashMap<Id<TypeNode>, usize>,
    union_index: HashMap<Id<TypeNode>, usize>,
    /// The key holders of structs, by type.
    holder_index: HashMap<Id<TypeNode>, usize>,
    /// The plans of unions inside a key holder, by type.
    key_union_index: HashMap<Id<TypeNode>, usize>,
}

/// How deep the plan of a value may nest before it is refused.
const MAX_DEPTH: usize = 64;

impl Builder<'_> {
    fn name_of(&self, id: Id<TypeNode>) -> String {
        self.ast
            .type_(id)
            .ok()
            .and_then(|node| {
                self.ast
                    .scoped_name_of(zerodds_ast::AnyId::Type(id, node.node_id))
            })
            .map(|scoped| {
                let text = scoped.to_string();
                text.strip_prefix("::").unwrap_or(&text).to_string()
            })
            .unwrap_or_default()
    }

    /// The plan of one struct, by its index in the set.
    ///
    /// zerodds-lint: recursion-depth 64 (one level per struct a member
    /// reaches; bounded by `MAX_DEPTH`)
    fn struct_plan(&mut self, id: Id<TypeNode>, depth: usize) -> Result<usize, String> {
        if let Some(index) = self.index.get(&id) {
            return Ok(*index);
        }
        if depth > MAX_DEPTH {
            return Err(String::from("a type that nests past 64 levels"));
        }
        let name = self.name_of(id);
        let index = self.structs.len();
        self.index.insert(id, index);
        let extensibility = self.ast.effective_extensibility(id);
        self.structs.push(StructPlan {
            name: name.clone(),
            needs_dheader: false,
            needs_sentinel: false,
            by_member_id: extensibility == Extensibility::Mutable,
            appendable: extensibility == Extensibility::Appendable,
            members: Vec::new(),
        });
        let key = LayoutKey::data(
            encoding_for(self.version, extensibility),
            Endianness::Little,
        );
        let layout =
            zerodds_ast_layout::xcdr::plan(self.ast, id, key, &mut DepCollector::default())
                .map_err(|error| format!("{name}: {error}"))?;
        let declared = self.ast.members_of(id).to_vec();
        if layout.members.len() != declared.len() {
            return Err(format!(
                "{name}: the plan frames {} members where the type declares {}",
                layout.members.len(),
                declared.len()
            ));
        }
        let mut members = Vec::with_capacity(declared.len());
        for (member_plan, member) in layout.members.iter().zip(&declared) {
            let member = self
                .ast
                .member(*member)
                .map_err(|error| format!("{name}: {error}"))?;
            if member_plan.member != member.node_id {
                return Err(format!(
                    "{name}: the plan frames the members in another order than the declaration"
                ));
            }
            let target = member
                .type_
                .resolved()
                .ok_or_else(|| format!("{name}.{}: an unresolved type", member.name))?;
            members.push(MemberFrame {
                member_id: member_plan.member_id,
                header: header(member_plan.header),
                optional: member.facets.is_optional(),
                value: self.value_plan(target, key, depth + 1)?,
            });
        }
        let plan = &mut self.structs[index];
        plan.needs_dheader = layout.needs_dheader;
        plan.needs_sentinel = layout.needs_sentinel;
        plan.members = members;
        Ok(index)
    }

    /// The plan of one value under `key`.
    ///
    /// zerodds-lint: recursion-depth 64 (one level per collection or
    /// typedef; bounded by `MAX_DEPTH`)
    fn value_plan(
        &mut self,
        id: Id<TypeNode>,
        key: LayoutKey,
        depth: usize,
    ) -> Result<ValuePlan, String> {
        if depth > MAX_DEPTH {
            return Err(String::from("a member type that nests past 64 levels"));
        }
        let node = self.ast.type_(id).map_err(|error| error.to_string())?;
        // This version plans the specification's dialect, whose element
        // padding is aligned (XTypes 1.3 §7.4.1).
        if key.element_padding == ElementPadding::Packed {
            return Err(String::from(
                "a packed element layout, which no description asks for",
            ));
        }
        Ok(match &node.kind {
            TypeKind::Alias { aliased } => {
                let target = aliased
                    .resolved()
                    .ok_or_else(|| String::from("an unresolved typedef"))?;
                self.value_plan(target, key, depth + 1)?
            }
            TypeKind::Primitive(kind) => ValuePlan::Primitive(primitive(*kind)?),
            TypeKind::String {
                width: CharWidth::Char,
                bound,
            } => ValuePlan::String {
                bound: self.evaluated(bound.expr())?,
            },
            TypeKind::Sequence { element, bound } => {
                let element = element
                    .resolved()
                    .ok_or_else(|| String::from("a sequence of an unresolved type"))?;
                if zerodds_ast_layout::xcdr::sequence_element_carries_length(self.ast, element, key)
                    != Some(false)
                {
                    return Err(String::from(
                        "a sequence whose elements carry their own length, which no description \
                         asks for",
                    ));
                }
                ValuePlan::Sequence {
                    element: Box::new(self.value_plan(element, key, depth + 1)?),
                    bound: self.evaluated(bound.expr())?,
                    delimited: zerodds_ast_layout::collection_is_delimited(self.ast, element, key),
                }
            }
            TypeKind::Array { element, dims } => {
                let element = element
                    .resolved()
                    .ok_or_else(|| String::from("an array of an unresolved type"))?;
                let mut lengths = Vec::with_capacity(dims.len());
                for dim in dims {
                    let length = self
                        .ast
                        .eval_integral(*dim)
                        .map_err(|_| String::from("an array length that does not evaluate"))?;
                    lengths.push(
                        usize::try_from(length)
                            .map_err(|_| String::from("an array length a usize holds"))?,
                    );
                }
                ValuePlan::Array {
                    element: Box::new(self.value_plan(element, key, depth + 1)?),
                    dims: lengths,
                    delimited: zerodds_ast_layout::array_is_delimited(self.ast, element, key),
                }
            }
            TypeKind::Enumeration { enumerators } => {
                let mut values = Vec::with_capacity(enumerators.len());
                for enumerator in enumerators {
                    values.push(
                        zerodds_ast_layout::enumerator_wire_value(
                            self.ast,
                            *enumerator,
                            key.enumerator_value,
                        )
                        .ok_or_else(|| String::from("an enumerator with no wire value"))?,
                    );
                }
                ValuePlan::Enum {
                    holder_bits: zerodds_ast_layout::enumeration_bit_bound(
                        self.ast,
                        id,
                        key.enum_holder,
                    ),
                    values,
                }
            }
            // Inside a key holder a struct is its own key holder and a
            // union is planned `@final` (XTypes 1.3 §7.6.8 steps 1 and 3).
            TypeKind::Structure { .. } if key.purpose == Purpose::Key => {
                ValuePlan::Holder(self.holder_plan(id, key, depth + 1)?)
            }
            TypeKind::Union { .. } if key.purpose == Purpose::Key => {
                ValuePlan::Union(self.union_plan_under(id, Some(key), depth + 1)?)
            }
            TypeKind::Structure { .. } => ValuePlan::Struct(self.struct_plan(id, depth + 1)?),
            TypeKind::Union { .. } => {
                ValuePlan::Union(self.union_plan_under(id, None, depth + 1)?)
            }
            other => {
                return Err(format!(
                    "a member of IDL kind `{}`, which no description declares",
                    other.as_str()
                ));
            }
        })
    }

    /// The plan of one union, by its index in the set: its framing from
    /// the planner, the discriminator under member id 0, and each case's
    /// labels and member. Under `holder`, the key of a key holder, the
    /// union as that holder carries it: `@final`, and the discriminator
    /// alone for a union keyed by it.
    ///
    /// zerodds-lint: recursion-depth 64 (one level per union a member
    /// reaches; bounded by `MAX_DEPTH`)
    fn union_plan_under(
        &mut self,
        id: Id<TypeNode>,
        holder: Option<LayoutKey>,
        depth: usize,
    ) -> Result<usize, String> {
        let cached = match holder {
            None => self.union_index.get(&id),
            Some(_) => self.key_union_index.get(&id),
        };
        if let Some(index) = cached {
            return Ok(*index);
        }
        if depth > MAX_DEPTH {
            return Err(String::from("a type that nests past 64 levels"));
        }
        let name = self.name_of(id);
        let node = self.ast.type_(id).map_err(|error| error.to_string())?;
        let TypeKind::Union {
            discriminator,
            cases,
            default_case,
        } = &node.kind
        else {
            return Err(format!("{name}: not a union"));
        };
        let discriminator = discriminator
            .resolved()
            .ok_or_else(|| format!("{name}: an unresolved discriminator"))?;
        let cases = cases.clone();
        let default_case = *default_case;
        let extensibility = self.ast.effective_extensibility(id);
        let key = holder.unwrap_or_else(|| {
            LayoutKey::data(
                encoding_for(self.version, extensibility),
                Endianness::Little,
            )
        });
        let layout =
            zerodds_ast_layout::xcdr::plan(self.ast, id, key, &mut DepCollector::default())
                .map_err(|error| format!("{name}: {error}"))?;
        let discriminator_only = holder.is_some()
            && zerodds_ast_layout::keyhash::union_key_discriminator(self.ast, id).is_some();
        let index = self.unions.len();
        match holder {
            None => self.union_index.insert(id, index),
            Some(_) => self.key_union_index.insert(id, index),
        };
        let discriminator_value = self.value_plan(discriminator, key, depth + 1)?;
        self.unions.push(UnionPlan {
            name: name.clone(),
            needs_dheader: layout.needs_dheader,
            needs_sentinel: layout.needs_sentinel,
            discriminator: MemberFrame {
                member_id: 0,
                header: layout.discriminator.map_or(Header::None, header),
                optional: false,
                value: discriminator_value,
            },
            discriminator_only,
            branches: Vec::new(),
        });
        let mut branches = Vec::with_capacity(cases.len());
        for case in cases {
            let case_node = self
                .ast
                .case(case)
                .map_err(|error| format!("{name}: {error}"))?;
            let mut labels = Vec::new();
            let mut is_default = default_case == Some(case);
            for label in &case_node.labels {
                match label {
                    zerodds_ast::CaseLabel::Default => is_default = true,
                    zerodds_ast::CaseLabel::Value(expr) => labels.push(
                        self.ast
                            .eval_integral(*expr)
                            .map_err(|_| format!("{name}: a case label that does not evaluate"))?,
                    ),
                }
            }
            let member = self
                .ast
                .member(case_node.member)
                .map_err(|error| format!("{name}: {error}"))?;
            let target = member
                .type_
                .resolved()
                .ok_or_else(|| format!("{name}.{}: an unresolved type", member.name))?;
            // Only a `@mutable` union's plan has a frame per case (rules
            // (26) and (30)): every other one writes the member bare.
            let (member_id, member_header) = layout
                .member(member.node_id)
                .map_or((0, Header::None), |plan| {
                    (plan.member_id, header(plan.header))
                });
            branches.push(BranchPlan {
                labels,
                default: is_default,
                frame: MemberFrame {
                    member_id,
                    header: member_header,
                    optional: false,
                    value: self.value_plan(target, key, depth + 1)?,
                },
            });
        }
        self.unions[index].branches = branches;
        Ok(index)
    }

    fn evaluated(
        &self,
        bound: Option<Id<zerodds_ast::ConstExprNode>>,
    ) -> Result<Option<usize>, String> {
        bound
            .map(|expr| {
                self.ast
                    .eval_integral(expr)
                    .ok()
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or_else(|| String::from("a bound that does not evaluate"))
            })
            .transpose()
    }

    /// The key holder of `root` (XTypes 1.3 §7.6.8), when it is a keyed
    /// type: a struct's key members in the holder's order, each one inside
    /// the holder as step 3 has it — a struct as its own holder, a union
    /// `@final` — or the discriminator of a union keyed by it. Big-endian,
    /// PLAIN_CDR2's alignment cap.
    fn key_plan(&mut self, root: Id<TypeNode>, is_union: bool) -> Result<Option<KeyPlan>, String> {
        let name = self.name_of(root);
        let extensibility = self.ast.effective_extensibility(root);
        let data = LayoutKey::data(
            encoding_for(Version::Two, extensibility),
            Endianness::Little,
        );
        let key = LayoutKey {
            purpose: Purpose::Key,
            endianness: Endianness::Big,
            ..data
        };
        let keyed = if is_union {
            zerodds_ast_layout::keyhash::union_key_discriminator(self.ast, root).is_some()
        } else {
            !zerodds_ast_layout::keyhash::key_members_with(
                self.ast,
                root,
                key.key_holder_nesting,
                key.key_holder_order,
            )
            .map_err(|error| format!("{name}: {error}"))?
            .is_empty()
        };
        if !keyed {
            return Ok(None);
        }
        let form = zerodds_ast_layout::keyhash::key_hash_form_for(self.ast, root, key)
            .map_err(|error| format!("{name}: {error}"))?;
        let plan = if is_union {
            Root::Union(self.union_plan_under(root, Some(key), 1)?)
        } else {
            Root::Struct(self.holder_plan(root, key, 1)?)
        };
        Ok(Some(KeyPlan {
            root: plan,
            max_align: usize::from(key.key_holder_alignment.max_alignment(key.encoding)),
            zero_padded: form == KeyHashForm::ZeroPadded,
        }))
    }

    /// The key holder of one struct under the holder's `key`, by its
    /// index in the set (XTypes 1.3 §7.6.8 step 1): the members the
    /// planner keeps — the key members, or all of them for a struct that
    /// declares none — in the order it keeps them, `@final`.
    ///
    /// zerodds-lint: recursion-depth 64 (one level per struct a key
    /// member reaches; bounded by `MAX_DEPTH`)
    fn holder_plan(
        &mut self,
        id: Id<TypeNode>,
        key: LayoutKey,
        depth: usize,
    ) -> Result<usize, String> {
        if let Some(index) = self.holder_index.get(&id) {
            return Ok(*index);
        }
        if depth > MAX_DEPTH {
            return Err(String::from("a type that nests past 64 levels"));
        }
        let name = self.name_of(id);
        let index = self.holders.len();
        self.holder_index.insert(id, index);
        self.holders.push(HolderPlan {
            name: name.clone(),
            members: Vec::new(),
        });
        let layout =
            zerodds_ast_layout::xcdr::plan(self.ast, id, key, &mut DepCollector::default())
                .map_err(|error| format!("{name}: {error}"))?;
        if layout.needs_dheader || layout.needs_sentinel {
            return Err(format!(
                "{name}: a key holder the planner frames as not @final"
            ));
        }
        // Below a struct that declares no key the planner may plan the
        // members as data (`KeyHolderNesting`); the same key it planned
        // them under.
        let member_key = zerodds_ast_layout::member_key(self.ast, id, key);
        let declared = self.ast.members_of(id).to_vec();
        let mut members = Vec::with_capacity(layout.members.len());
        for member_plan in &layout.members {
            let Some(position) = declared.iter().position(|member| {
                self.ast
                    .member(*member)
                    .is_ok_and(|m| m.node_id == member_plan.member)
            }) else {
                return Err(format!(
                    "{name}: a key holder member the type does not declare"
                ));
            };
            let member = self
                .ast
                .member(declared[position])
                .map_err(|error| format!("{name}: {error}"))?;
            // `@final`: no header, or an `@optional` member's
            // `<is_present>` (rule (20)).
            if !matches!(
                member_plan.header,
                MemberHeader::NoHeader | MemberHeader::OptionalFlag
            ) {
                return Err(format!(
                    "{name}.{}: a key holder member framed as not @final",
                    member.name
                ));
            }
            let target = member
                .type_
                .resolved()
                .ok_or_else(|| format!("{name}.{}: an unresolved type", member.name))?;
            members.push((
                position,
                MemberFrame {
                    member_id: member_plan.member_id,
                    header: header(member_plan.header),
                    optional: member.facets.is_optional(),
                    value: self.value_plan(target, member_key, depth + 1)?,
                },
            ));
        }
        self.holders[index].members = members;
        Ok(index)
    }
}

fn header(header: MemberHeader) -> Header {
    match header {
        MemberHeader::NoHeader => Header::None,
        MemberHeader::OptionalFlag => Header::OptionalFlag,
        MemberHeader::PidCdr1 {
            pid,
            extended,
            must_understand,
        } => Header::Pid {
            pid,
            extended,
            must_understand,
            role: ParameterRole::Member,
        },
        MemberHeader::OptionalCdr1 {
            pid,
            extended,
            must_understand,
        } => Header::Pid {
            pid,
            extended,
            must_understand,
            role: ParameterRole::Optional,
        },
        MemberHeader::EmHeader {
            lc,
            must_understand,
        } => Header::EmHeader {
            length_code: lc as u8,
            must_understand,
        },
    }
}

fn primitive(kind: zerodds_ast::PrimitiveKind) -> Result<Primitive, String> {
    use zerodds_ast::PrimitiveKind as K;
    Ok(match kind {
        K::Boolean => Primitive::Boolean,
        K::Octet => Primitive::Octet,
        K::Char => Primitive::Char,
        K::Int8 => Primitive::Int8,
        K::UInt8 => Primitive::UInt8,
        K::Int16 => Primitive::Int16,
        K::UInt16 => Primitive::UInt16,
        K::Int32 => Primitive::Int32,
        K::UInt32 => Primitive::UInt32,
        K::Int64 => Primitive::Int64,
        K::UInt64 => Primitive::UInt64,
        K::Float32 => Primitive::Float32,
        K::Float64 => Primitive::Float64,
        K::WChar | K::Float128 => {
            return Err(format!(
                "a `{}` member, which no description declares",
                kind.as_str()
            ));
        }
    })
}
