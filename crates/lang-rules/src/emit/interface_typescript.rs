// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! An IDL `interface`, as DDS-TS 1.0 Ch11 maps it to TypeScript.
//!
//! Three declarations per interface and no class: a `<I>Client`
//! interface, a `<I>Handler` interface and a `<I>Service` constant of
//! type `ServiceDescriptor<<I>Client, <I>Handler>` (Ch11 §Mapping
//! Surface). "Concrete client construction is the responsibility of the
//! runtime … which produces objects satisfying the client interface from
//! the Service Descriptor", so there is nothing here for a class to be.
//!
//! ## Everything is a promise
//!
//! Ch11: "Operations SHALL be mapped to `Promise<T>`, not to synchronous
//! returns … TypeScript runs on event-loop runtimes where synchronous
//! network I/O is unavailable." This is the one place the DDS-TS PSM
//! parts company with the OMG C++ and Java PSMs, which is why
//! `emit::interface_cpp` and `emit::interface_java` share a shape and
//! this file does not share it with them. What all three do share is
//! [`crate::adapter::interface`], which is the point of the seam.
//!
//! No `async` keyword appears on any signature: Ch11 forbids it, and
//! TypeScript would reject it — `async` marks an implementation, not a
//! type.
//!
//! ## `out` and `inout`
//!
//! Ch11 aggregates them into the resolved value: an operation that
//! declares either resolves to an object literal carrying the return
//! value as `result` and one property per `out`/`inout` parameter, named
//! after the IDL parameter, with `result` omitted for a `void` return.
//! An `inout` parameter appears in both the parameter list and the
//! resolved object, and the two are independent values.
//!
//! ## §11.7
//!
//! A forward declaration with no definition in the compilation unit is
//! fatal and carries the code `DDS-TS-E002`. That check is
//! [`crate::adapter::interface::services`]'s, because a C++ or a Java
//! binding over an interface with no body is just as empty; what is
//! TypeScript's is the code, which
//! [`crate::adapter::interface::BindingError::diagnostic_code`] answers.

use crate::adapter::cst::MaterialiseError;
use crate::adapter::interface::{self, Attribute, BindingError, Operation, Service};
use crate::adapter::typescript::ts_member_type;
use crate::apply::Applied;
use zerodds_ast::{Ast, Bound, CharWidth, ModuleNode, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::typescript as ts;

/// The module the Operations-Profile descriptor types come from.
///
/// `runtime/typescript/src/operations.ts`, re-exported by
/// `@zerodds/wire`'s `src/index.ts` per Ch2 §Operations Profile — the
/// one package this tree publishes the TypeScript runtime as. The name
/// here was `@zerodds/types`, which is a package that does not exist;
/// see [`crate::adapter::typescript::DEFAULT_RUNTIME_PACKAGE`].
pub const RUNTIME_MODULE: &str = "@zerodds/wire";

/// The property a resolved value carries an operation's return in.
/// Ch11 §Operation Mapping.
pub const RESULT_PROPERTY: &str = "result";

/// How deep an alias chain is followed before the model is taken to hold
/// a cycle.
const MAX_ALIAS_DEPTH: usize = 64;

/// The TypeScript declarations every interface of `ast` produces.
///
/// Nested in the namespace its IDL module names, as
/// [`crate::adapter::typescript::ts_items`] nests its own.
///
/// # Errors
/// [`BindingError`] — the interface did not reach the seam (`DDS-TS-E002`
/// among them), or a type in it has no TypeScript spelling.
pub fn ts_interface_items(ast: &Ast, applied: &Applied) -> Result<Vec<ts::Decl>, BindingError> {
    let mut pending: Vec<(Vec<String>, ts::Decl)> = Vec::new();
    for service in interface::services(ast)? {
        let chain: Vec<String> = interface::module_chain(ast, service.module)?
            .into_iter()
            .map(|(_, name)| ts::escape_keyword(&name))
            .collect();
        for decl in decls_of(ast, applied, &service)? {
            pending.push((chain.clone(), decl));
        }
    }
    Ok(nest(&pending, 0))
}

/// One level of the namespace fold.
///
/// zerodds-lint: recursion-depth 64 (module nesting; every chain is
/// built by `interface::module_chain`, which is bounded by
/// `interface::MAX_MODULE_DEPTH`)
fn nest(items: &[(Vec<String>, ts::Decl)], depth: usize) -> Vec<ts::Decl> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < items.len() {
        let Some((chain, decl)) = items.get(index) else {
            break;
        };
        let Some(name) = chain.get(depth) else {
            out.push(decl.clone());
            index += 1;
            continue;
        };
        let mut end = index;
        while items
            .get(end)
            .is_some_and(|(other, _)| other.get(depth) == Some(name))
        {
            end += 1;
        }
        let Some(run) = items.get(index..end) else {
            break;
        };
        out.push(ts::Decl::Namespace(ts::Namespace {
            doc: Doc::line("An IDL module, as the namespace its declarations nest in."),
            exported: true,
            declare: false,
            name: Ident::new(name),
            decls: nest(run, depth + 1),
        }));
        index = end;
    }
    out
}

fn decls_of(
    ast: &Ast,
    applied: &Applied,
    service: &Service,
) -> Result<Vec<ts::Decl>, BindingError> {
    let name = ts::escape_keyword(&service.name);
    let members = signatures(ast, applied, service)?;
    Ok(vec![
        ts::Decl::Interface(ts::Interface {
            doc: Doc::line("The client half of the service. DDS-TS 1.0 §11.1."),
            exported: true,
            name: Ident::new(&format!("{name}Client")),
            generics: Vec::new(),
            extends: service
                .bases
                .iter()
                .map(|base| named(&format!("{}Client", ts::escape_keyword(&base.name))))
                .collect(),
            members: members.clone(),
        }),
        ts::Decl::Interface(ts::Interface {
            doc: Doc::line(
                "The handler half of the service: what a service-side implementer satisfies. \
                 DDS-TS 1.0 §11.1.",
            ),
            exported: true,
            name: Ident::new(&format!("{name}Handler")),
            generics: Vec::new(),
            extends: service
                .bases
                .iter()
                .map(|base| named(&format!("{}Handler", ts::escape_keyword(&base.name))))
                .collect(),
            members,
        }),
        descriptor(ast, service, &name)?,
    ])
}

/// The method set both halves carry. DDS-TS 1.0 §11.1: the handler
/// "exposing the same set of methods".
fn signatures(
    ast: &Ast,
    applied: &Applied,
    service: &Service,
) -> Result<Vec<ts::Signature>, BindingError> {
    let mut out = Vec::new();
    for operation in &service.operations {
        out.push(ts::Signature::Method(operation_signature(
            ast, applied, service, operation,
        )?));
    }
    for attribute in &service.attributes {
        out.extend(attribute_signatures(ast, applied, service, attribute)?);
    }
    Ok(out)
}

fn operation_signature(
    ast: &Ast,
    applied: &Applied,
    service: &Service,
    operation: &Operation,
) -> Result<ts::MethodSignature, BindingError> {
    // §11.2: the parameter list is the `in` and `inout` parameters, in
    // declaration order.
    let mut params = Vec::new();
    for param in operation.inputs() {
        params.push(ts::Param {
            name: Ident::new(&ts::escape_keyword(&param.name)),
            optional: false,
            rest: false,
            type_: Some(ts_member_type(
                ast,
                applied,
                param.type_,
                &at(service, operation, &param.name),
            )?),
            default: None,
        });
    }

    // §11.2 "Out and InOut parameters": the resolved value aggregates
    // them, with the declared return under `result`, and `result`
    // omitted for a `void` return.
    let resolved = if operation.has_outputs() {
        let mut fields = Vec::new();
        if let Some(target) = operation.returns {
            fields.push(property(
                RESULT_PROPERTY,
                ts_member_type(ast, applied, target, &at(service, operation, "return"))?,
            ));
        }
        for param in operation.outputs() {
            fields.push(property(
                &ts::escape_member(&param.name),
                ts_member_type(
                    ast,
                    applied,
                    param.type_,
                    &at(service, operation, &param.name),
                )?,
            ));
        }
        ts::Type::Object(fields)
    } else if let Some(target) = operation.returns {
        ts_member_type(ast, applied, target, &at(service, operation, "return"))?
    } else {
        ts::Type::Keyword(ts::Keyword::Void)
    };

    Ok(ts::MethodSignature {
        doc: Doc::none(),
        name: ts::PropertyName::Ident(Ident::new(&ts::escape_member(&operation.name))),
        optional: false,
        generics: Vec::new(),
        params,
        returns: promise_of(resolved),
    })
}

/// §11.3: `get_<name>(): Promise<T>` and, unless `readonly`,
/// `set_<name>(value: T): Promise<void>`.
fn attribute_signatures(
    ast: &Ast,
    applied: &Applied,
    service: &Service,
    attribute: &Attribute,
) -> Result<Vec<ts::Signature>, BindingError> {
    let where_ = format!("{}::{}", service.name, attribute.name);
    let type_ = ts_member_type(ast, applied, attribute.type_, &where_)?;
    let mut out = vec![ts::Signature::Method(ts::MethodSignature {
        doc: Doc::line("An attribute's read half. DDS-TS 1.0 §11.3."),
        name: ts::PropertyName::Ident(Ident::new(&format!(
            "get_{}",
            ts::escape_member(&attribute.name)
        ))),
        optional: false,
        generics: Vec::new(),
        params: Vec::new(),
        returns: promise_of(type_.clone()),
    })];
    if !attribute.readonly {
        out.push(ts::Signature::Method(ts::MethodSignature {
            doc: Doc::line("An attribute's write half. DDS-TS 1.0 §11.3."),
            name: ts::PropertyName::Ident(Ident::new(&format!(
                "set_{}",
                ts::escape_member(&attribute.name)
            ))),
            optional: false,
            generics: Vec::new(),
            params: vec![ts::Param {
                name: Ident::new("value"),
                optional: false,
                rest: false,
                type_: Some(type_),
                default: None,
            }],
            returns: promise_of(ts::Type::Keyword(ts::Keyword::Void)),
        }));
    }
    Ok(out)
}

// -- the service descriptor --------------------------------------------

/// `export const <I>Service: ServiceDescriptor<<I>Client, <I>Handler>`
/// — DDS-TS 1.0 §11.5.
fn descriptor(ast: &Ast, service: &Service, name: &str) -> Result<ts::Decl, BindingError> {
    let mut operations = Vec::new();
    for operation in &service.operations {
        operations.push(operation_descriptor(ast, service, operation)?);
    }
    let mut attributes = Vec::new();
    for attribute in &service.attributes {
        attributes.push(attribute_descriptor(ast, service, attribute)?);
    }

    // §11.6: "The descriptor's `inherits` array SHALL contain all parent
    // service descriptors in declaration order; no flattening of
    // inherited operations into the child descriptor is performed."
    let inherits = service
        .bases
        .iter()
        .map(|base| {
            ts::Expr::Path(Path::single(&format!(
                "{}Service",
                ts::escape_keyword(&base.name)
            )))
        })
        .collect();

    Ok(ts::Decl::Variable(ts::Variable {
        doc: Doc::line("The service's operation metadata. DDS-TS 1.0 §11.5."),
        exported: true,
        declare: false,
        kind: ts::VarKind::Const,
        name: Ident::new(&format!("{name}Service")),
        type_: Some(ts::Type::Named {
            path: Path::single("ServiceDescriptor"),
            args: vec![
                named(&format!("{name}Client")),
                named(&format!("{name}Handler")),
            ],
        }),
        value: Some(ts::Expr::Object(vec![
            (
                ts::PropertyName::ident("name"),
                string(&qualified_name(ast, service)?),
            ),
            (
                ts::PropertyName::ident("inherits"),
                ts::Expr::Array(inherits),
            ),
            (
                ts::PropertyName::ident("operations"),
                ts::Expr::Array(operations),
            ),
            (
                ts::PropertyName::ident("attributes"),
                ts::Expr::Array(attributes),
            ),
        ])),
    }))
}

fn operation_descriptor(
    ast: &Ast,
    service: &Service,
    operation: &Operation,
) -> Result<ts::Expr, BindingError> {
    let mut params = Vec::new();
    for param in &operation.params {
        params.push(ts::Expr::Object(vec![
            (ts::PropertyName::ident("name"), string(&param.name)),
            (
                ts::PropertyName::ident("mode"),
                string(param.direction.as_str()),
            ),
            (
                ts::PropertyName::ident("type"),
                type_ref(ast, param.type_, &at(service, operation, &param.name))?,
            ),
        ]));
    }
    let returns = match operation.returns {
        Some(target) => type_ref(ast, target, &at(service, operation, "return"))?,
        None => ts::Expr::Object(vec![(ts::PropertyName::ident("kind"), string("void"))]),
    };
    Ok(ts::Expr::Object(vec![
        (ts::PropertyName::ident("name"), string(&operation.name)),
        (
            // The descriptor type of the TypeScript runtime has the field;
            // DDS-RPC 1.0 §7.3.1.1 production (87) has no `oneway`.
            ts::PropertyName::ident("oneway"),
            ts::Expr::Literal(Literal::Bool(false)),
        ),
        (ts::PropertyName::ident("returnType"), returns),
        (
            ts::PropertyName::ident("parameters"),
            ts::Expr::Array(params),
        ),
        (ts::PropertyName::ident("raises"), raises(&operation.raises)),
    ]))
}

fn attribute_descriptor(
    ast: &Ast,
    service: &Service,
    attribute: &Attribute,
) -> Result<ts::Expr, BindingError> {
    let where_ = format!("{}::{}", service.name, attribute.name);
    Ok(ts::Expr::Object(vec![
        (ts::PropertyName::ident("name"), string(&attribute.name)),
        (
            // `readonly` is a TypeScript modifier and a property name
            // here; a bare `readonly:` is legal as a key and is what
            // §11.5 writes.
            ts::PropertyName::ident("readonly"),
            ts::Expr::Literal(Literal::Bool(attribute.readonly)),
        ),
        (
            ts::PropertyName::ident("type"),
            type_ref(ast, attribute.type_, &where_)?,
        ),
        (
            ts::PropertyName::ident("getRaises"),
            raises(&attribute.get_raises),
        ),
        (
            ts::PropertyName::ident("setRaises"),
            raises(&attribute.set_raises),
        ),
    ]))
}

/// The exception descriptors a `raises`, `getraises` or `setraises`
/// clause names. §7 gives each IDL exception a
/// `<Name>Type: DdsTypeDescriptor`, which is what the array holds.
fn raises(list: &[interface::Raise]) -> ts::Expr {
    ts::Expr::Array(
        list.iter()
            .map(|raise| {
                ts::Expr::Path(Path::single(&format!(
                    "{}Type",
                    ts::escape_keyword(&raise.name)
                )))
            })
            .collect(),
    )
}

/// A `DdsTypeRef` for one type. DDS-TS 1.0 §8 §Type References.
fn type_ref(ast: &Ast, id: Id<TypeNode>, at: &str) -> Result<ts::Expr, BindingError> {
    type_ref_at(ast, id, at, 0)
}

/// zerodds-lint: recursion-depth 64 (collection nesting; bounded by `MAX_ALIAS_DEPTH`)
fn type_ref_at(
    ast: &Ast,
    id: Id<TypeNode>,
    at: &str,
    depth: usize,
) -> Result<ts::Expr, BindingError> {
    if depth > MAX_ALIAS_DEPTH {
        return Err(BindingError::Materialise(MaterialiseError::Unevaluated {
            construct: format!("type reference — it nests past {MAX_ALIAS_DEPTH}"),
            at: at.to_string(),
        }));
    }
    let node = ast.type_(id).map_err(|_| MaterialiseError::Unevaluated {
        construct: "type".to_string(),
        at: at.to_string(),
    })?;
    let expr = match &node.kind {
        TypeKind::Primitive(kind) => ts::Expr::Object(vec![
            (ts::PropertyName::ident("kind"), string("primitive")),
            (
                ts::PropertyName::ident("name"),
                string(primitive_name(*kind)),
            ),
        ]),
        TypeKind::String { width, bound } => {
            let mut fields = vec![(ts::PropertyName::ident("kind"), string("string"))];
            if let Some(value) = bound_value(ast, *bound, at)? {
                fields.push((ts::PropertyName::ident("bound"), number(value)));
            }
            fields.push((
                ts::PropertyName::ident("wide"),
                ts::Expr::Literal(Literal::Bool(matches!(width, CharWidth::WChar))),
            ));
            ts::Expr::Object(fields)
        }
        TypeKind::Sequence { element, bound } => {
            let mut fields = vec![(ts::PropertyName::ident("kind"), string("sequence"))];
            fields.push((
                ts::PropertyName::ident("element"),
                element_ref(ast, element.resolved(), at, depth)?,
            ));
            if let Some(value) = bound_value(ast, *bound, at)? {
                fields.push((ts::PropertyName::ident("bound"), number(value)));
            }
            ts::Expr::Object(fields)
        }
        TypeKind::Array { element, dims } => {
            let mut lengths = Vec::new();
            for dim in dims {
                let length =
                    ast.eval_integral(*dim)
                        .map_err(|_| MaterialiseError::Unevaluated {
                            construct: "array length".to_string(),
                            at: at.to_string(),
                        })?;
                let length = u64::try_from(length).map_err(|_| MaterialiseError::Unevaluated {
                    construct: format!("array length — it evaluates to {length}"),
                    at: at.to_string(),
                })?;
                lengths.push(number(length));
            }
            ts::Expr::Object(vec![
                (ts::PropertyName::ident("kind"), string("array")),
                (
                    ts::PropertyName::ident("element"),
                    element_ref(ast, element.resolved(), at, depth)?,
                ),
                (ts::PropertyName::ident("lengths"), ts::Expr::Array(lengths)),
            ])
        }
        TypeKind::Map { key, value, bound } => {
            let mut fields = vec![
                (ts::PropertyName::ident("kind"), string("map")),
                (
                    ts::PropertyName::ident("key"),
                    element_ref(ast, key.resolved(), at, depth)?,
                ),
                (
                    ts::PropertyName::ident("value"),
                    element_ref(ast, value.resolved(), at, depth)?,
                ),
            ];
            if let Some(value) = bound_value(ast, *bound, at)? {
                fields.push((ts::PropertyName::ident("bound"), number(value)));
            }
            ts::Expr::Object(fields)
        }
        TypeKind::Any => ts::Expr::Object(vec![(ts::PropertyName::ident("kind"), string("any"))]),
        // An anonymous typedef is one the frontend made for an inline
        // type; it is transparent, so the reference is the target's.
        TypeKind::Alias { aliased } if node.name.is_empty() => {
            element_ref(ast, aliased.resolved(), at, depth)?
        }
        // §8: "The `ref` kind carries the qualified IDL name of the
        // referenced type … Resolution is performed at use time via
        // `lookupType(name)`."
        TypeKind::Alias { .. }
        | TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Valuetype { .. } => ts::Expr::Object(vec![
            (ts::PropertyName::ident("kind"), string("ref")),
            (
                ts::PropertyName::ident("name"),
                string(&qualified_type_name(ast, id)?),
            ),
        ]),
        // `fixed<P,S>`, `native`, a value box, `ValueBase`, an object
        // reference and an annotation type each have no `DdsTypeRef`
        // kind. Refused and not written as a `ref` to a name the
        // registry will never hold: `lookupType` would answer
        // `undefined` at run time, from a descriptor that type-checked.
        TypeKind::Fixed { .. }
        | TypeKind::Native
        | TypeKind::ValueBox { .. }
        | TypeKind::ValueBase
        | TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. } => {
            return Err(BindingError::Materialise(
                MaterialiseError::NoLanguageForm {
                    construct: node.kind.as_str().to_string(),
                    language: "TypeScript",
                    at: at.to_string(),
                },
            ));
        }
    };
    Ok(expr)
}

/// A collection's declared bound, evaluated.
///
/// `Bound::Unbounded` is the absent property and not `bound: undefined`:
/// the runtime is compiled with `exactOptionalPropertyTypes`, under
/// which the two are different types.
fn bound_value(ast: &Ast, bound: Bound, at: &str) -> Result<Option<u64>, BindingError> {
    let Some(expr) = bound.expr() else {
        return Ok(None);
    };
    let value = ast
        .eval_integral(expr)
        .map_err(|_| MaterialiseError::Unevaluated {
            construct: "collection bound".to_string(),
            at: at.to_string(),
        })?;
    let value = u64::try_from(value).map_err(|_| MaterialiseError::Unevaluated {
        construct: format!("collection bound — it evaluates to {value}"),
        at: at.to_string(),
    })?;
    Ok(Some(value))
}

/// The `DdsTypeRef` for a collection's element, key or value.
///
/// zerodds-lint: recursion-depth 64 (collection nesting, via
/// [`type_ref_at`]; bounded by `MAX_ALIAS_DEPTH`, which returns
/// `MaterialiseError::Unevaluated` naming the type reference)
fn element_ref(
    ast: &Ast,
    target: Option<Id<TypeNode>>,
    at: &str,
    depth: usize,
) -> Result<ts::Expr, BindingError> {
    let target = target.ok_or(MaterialiseError::Unevaluated {
        construct: "element type".to_string(),
        at: at.to_string(),
    })?;
    type_ref_at(ast, target, at, depth + 1)
}

/// DDS-TS 1.0 §8's `PrimitiveName`.
///
/// `int8` is the one name past the list §8.2 prints: the list names
/// `octet`, which is `uint8`, and names nothing for the signed byte IDL
/// 4.2 §7.4.1.4.4.2 also declares. `runtime/typescript/src/types.ts`
/// carries the additive member, and this writes it rather than folding
/// `int8` onto `octet` — the two differ in signedness, and a negative
/// value read back through the unsigned name is a large positive one.
fn primitive_name(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Boolean => "boolean",
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => "octet",
        PrimitiveKind::Char => "char",
        PrimitiveKind::WChar => "wchar",
        PrimitiveKind::Int16 => "int16",
        PrimitiveKind::UInt16 => "uint16",
        PrimitiveKind::Int32 => "int32",
        PrimitiveKind::UInt32 => "uint32",
        PrimitiveKind::Int64 => "int64",
        PrimitiveKind::UInt64 => "uint64",
        PrimitiveKind::Float32 => "float",
        PrimitiveKind::Float64 => "double",
        PrimitiveKind::Float128 => "longDouble",
        PrimitiveKind::Int8 => "int8",
    }
}

// -- names --------------------------------------------------------------

/// The qualified IDL name of an interface, as §11.5's `name` field wants
/// it: the scoped name, `::`-separated, without a leading `::`.
fn qualified_name(ast: &Ast, service: &Service) -> Result<String, BindingError> {
    Ok(scoped(
        &interface::module_chain(ast, service.module)?,
        &service.name,
    ))
}

/// The qualified IDL name of a type, for a `{ kind: "ref" }`.
fn qualified_type_name(ast: &Ast, id: Id<TypeNode>) -> Result<String, BindingError> {
    let node = ast.type_(id).map_err(|_| MaterialiseError::Unevaluated {
        construct: "type".to_string(),
        at: "a type reference".to_string(),
    })?;
    Ok(scoped(
        &interface::module_chain(ast, node.module)?,
        &node.name,
    ))
}

fn scoped(chain: &[(Id<ModuleNode>, String)], name: &str) -> String {
    let mut segments: Vec<&str> = chain.iter().map(|(_, name)| name.as_str()).collect();
    segments.push(name);
    segments.join("::")
}

// -- small shapes -------------------------------------------------------

fn promise_of(payload: ts::Type) -> ts::Type {
    ts::Type::Named {
        path: Path::single("Promise"),
        args: vec![payload],
    }
}

/// zerodds-lint: recursion-depth 1 — this does not recurse. The callee
/// is `ts::Type::named`, an associated function of the CST type;
/// `dds_bounded_recursion` compares only the last segment of a call
/// path, so the constructor reads as a self-call.
fn named(name: &str) -> ts::Type {
    ts::Type::named(Path::single(name))
}

fn property(name: &str, type_: ts::Type) -> ts::Signature {
    ts::Signature::Property(ts::Property {
        doc: Doc::none(),
        readonly: false,
        name: ts::PropertyName::Ident(Ident::new(name)),
        optional: false,
        type_,
    })
}

fn string(text: &str) -> ts::Expr {
    ts::Expr::Literal(Literal::Str(text.to_string()))
}

fn number(value: u64) -> ts::Expr {
    ts::Expr::Literal(Literal::Int(value.to_string()))
}

fn at(service: &Service, operation: &Operation, what: &str) -> String {
    format!("{}::{}({what})", service.name, operation.name)
}
