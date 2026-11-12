// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The CORBA platform of Annex A.1: the bases and members it adds to the
//! classes of clause 7, the `_tc_` constants (A.1.3), the insertion and
//! extraction operators of `CORBA::Any` (A.1.7), the value boxes of
//! A.1.8.6, and the `CORBA::traits` specializations of A.1.1.

use super::text::{self, Writer};
use super::{Gen, Idl4CppError, Item, Platform, Standard, names, resolved_all, unresolved};
use zerodds_ast::{AnyId, CaseLabel, CharWidth, InterfaceKind, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, Ref};

/// The declarations of the `CORBA` module that `omg/corba.hpp` defines by
/// hand, as Annex A.1 gives them, and that are therefore not derived from
/// the IDL of `orb.idl`: `TypeCode` with `TCKind`, `Visibility` and
/// `ValueModifier` and their constants (A.1.3), and the standard
/// exceptions with their completion status (A.1.2).
pub(super) const RUNTIME_HOLDS: &[&str] = &[
    "TypeCode",
    "TCKind",
    "Visibility",
    "PRIVATE_MEMBER",
    "PUBLIC_MEMBER",
    "ValueModifier",
    "VM_NONE",
    "VM_CUSTOM",
    "VM_ABSTRACT",
    "VM_TRUNCATABLE",
    "completion_status",
    "UNKNOWN",
    "BAD_PARAM",
    "NO_MEMORY",
    "IMP_LIMIT",
    "COMM_FAILURE",
    "INV_OBJREF",
    "NO_PERMISSION",
    "INTERNAL",
    "MARSHAL",
    "INITIALIZE",
    "NO_IMPLEMENT",
    "BAD_TYPECODE",
    "BAD_OPERATION",
    "NO_RESOURCES",
    "NO_RESPONSE",
    "PERSIST_STORE",
    "BAD_INV_ORDER",
    "TRANSIENT",
    "FREE_MEM",
    "INV_IDENT",
    "INV_FLAG",
    "INTF_REPOS",
    "BAD_CONTEXT",
    "OBJ_ADAPTER",
    "DATA_CONVERSION",
    "OBJECT_NOT_EXIST",
    "TRANSACTION_REQUIRED",
    "TRANSACTION_ROLLEDBACK",
    "INVALID_TRANSACTION",
    "INV_POLICY",
    "CODESET_INCOMPATIBLE",
    "REBIND",
    "TIMEOUT",
    "TRANSACTION_UNAVAILABLE",
    "TRANSACTION_MODE",
    "BAD_QOS",
];

/// The `_tc_` constant and the `Any` operators of one declaration.
pub(super) struct Extras {
    /// `_tc_<Name>`.
    pub name: String,
    /// The expression that creates the type code.
    pub typecode: String,
    /// The insertion and extraction operators, as namespace-scope
    /// functions (`inline ...`).
    pub operators: String,
}

impl Gen<'_> {
    /// The repository id of a declaration: the model's, else `IDL:` and
    /// its scoped name (CORBA 3.3 part 1 §14.7.1).
    pub(super) fn repository_id(&self, any: AnyId) -> String {
        if let Some(id) = self
            .ast
            .facets(any)
            .and_then(|f| f.corba.as_ref())
            .and_then(|c| c.repository_id.as_ref())
        {
            return id.0.clone();
        }
        let scoped = self
            .ast
            .scoped_name_of(any)
            .map_or_else(Vec::new, |name| name.segments().to_vec());
        format!("IDL:{}:1.0", scoped.join("/"))
    }

    /// The base of an interface without IDL bases: `CORBA::Object`
    /// (A.1.5), `CORBA::LocalObject` for a local interface (A.1.6),
    /// `CORBA::AbstractBase` for an abstract one (A.1.9.1).
    pub(super) fn corba_object_base(
        &self,
        id: Id<zerodds_ast::InterfaceNode>,
    ) -> Result<String, Idl4CppError> {
        let node = self
            .ast
            .interface(id)
            .map_err(|_| unresolved("an interface"))?;
        Ok(match node.kind {
            InterfaceKind::Local => "public virtual ::CORBA::LocalObject".to_string(),
            InterfaceKind::Abstract => "public virtual ::CORBA::AbstractBase".to_string(),
            _ => "public virtual ::CORBA::Object".to_string(),
        })
    }

    /// The abstract interfaces among the bases of `roots`, direct and
    /// inherited, and the roots themselves when they are abstract.
    fn abstract_interfaces(
        &self,
        roots: impl Iterator<Item = Id<zerodds_ast::InterfaceNode>>,
    ) -> Result<Vec<Id<zerodds_ast::InterfaceNode>>, Idl4CppError> {
        let mut found = Vec::new();
        let mut seen = Vec::new();
        let mut queue: Vec<Id<zerodds_ast::InterfaceNode>> = roots.collect();
        while let Some(next) = queue.pop() {
            if seen.contains(&next) {
                continue;
            }
            seen.push(next);
            let node = self
                .ast
                .interface(next)
                .map_err(|_| unresolved("a base interface"))?;
            if node.kind == InterfaceKind::Abstract {
                found.push(next);
            }
            queue.extend(resolved_all(&node.bases, "a base interface")?);
        }
        Ok(found)
    }

    /// The abstract interfaces an interface inherits from.
    fn abstract_ancestors(
        &self,
        id: Id<zerodds_ast::InterfaceNode>,
    ) -> Result<Vec<Id<zerodds_ast::InterfaceNode>>, Idl4CppError> {
        let node = self
            .ast
            .interface(id)
            .map_err(|_| unresolved("an interface"))?;
        self.abstract_interfaces(resolved_all(&node.bases, "a base interface")?.into_iter())
    }

    /// The abstract interfaces a value type supports, by itself or through
    /// a base value type.
    pub(super) fn abstract_interfaces_of_value(
        &self,
        id: Id<TypeNode>,
    ) -> Result<Vec<Id<zerodds_ast::InterfaceNode>>, Idl4CppError> {
        let mut roots = Vec::new();
        let mut seen = Vec::new();
        let mut queue = vec![id];
        while let Some(next) = queue.pop() {
            if seen.contains(&next) {
                continue;
            }
            seen.push(next);
            if let Ok(TypeKind::Valuetype {
                bases, supported, ..
            }) = self.ast.type_(next).map(|n| &n.kind)
            {
                roots.extend(resolved_all(supported, "a supported interface")?);
                for base in resolved_all(bases, "a base value type")? {
                    queue.push(self.through_aliases(base)?);
                }
            }
        }
        self.abstract_interfaces(roots.into_iter())
    }

    /// Whether a value type has operations, its own or inherited: an
    /// operation or attribute of the value type or of one of its base value
    /// types, or a supported interface. A.1.8.9.2 generates the concrete
    /// factory only for a value type without them, whose OBV class is
    /// complete.
    pub(super) fn value_has_operations(&self, id: Id<TypeNode>) -> Result<bool, Idl4CppError> {
        let mut seen = Vec::new();
        let mut queue = vec![id];
        while let Some(next) = queue.pop() {
            if seen.contains(&next) {
                continue;
            }
            seen.push(next);
            if !self.ast.operations_of_value(next).is_empty()
                || !self.ast.attributes_of_value(next).is_empty()
            {
                return Ok(true);
            }
            if let Ok(TypeKind::Valuetype {
                bases, supported, ..
            }) = self.ast.type_(next).map(|n| &n.kind)
            {
                if !supported.is_empty() {
                    return Ok(true);
                }
                for base in resolved_all(bases, "a base value type")? {
                    queue.push(self.through_aliases(base)?);
                }
            }
        }
        Ok(false)
    }

    /// Whether the interface is an abstract one.
    pub(super) fn corba_abstract_interface(&self, id: Id<zerodds_ast::InterfaceNode>) -> bool {
        self.ast
            .interface(id)
            .map(|n| n.kind == InterfaceKind::Abstract)
            .unwrap_or(false)
    }

    /// Whether the class of an interface names `CORBA::Object`,
    /// `CORBA::LocalObject` or `CORBA::AbstractBase` among its bases: an
    /// interface without IDL bases, and an interface all of whose bases are
    /// abstract — "C++ classes for abstract interfaces are not derived from
    /// the CORBA::Object C++ class" (A.1.9.1), so the interface derived
    /// from them brings it.
    pub(super) fn corba_needs_object_base(
        &self,
        id: Id<zerodds_ast::InterfaceNode>,
    ) -> Result<bool, Idl4CppError> {
        let node = self
            .ast
            .interface(id)
            .map_err(|_| unresolved("an interface"))?;
        if node.kind == InterfaceKind::Abstract {
            return Ok(node.bases.is_empty());
        }
        Ok(!resolved_all(&node.bases, "a base interface")?
            .into_iter()
            .any(|base| !self.corba_abstract_interface(base)))
    }

    /// What A.1 adds to an interface class: `_is_a` and `_repository_id`
    /// of `CORBA::Object` (A.1.5), answered with the interface's repository
    /// id and its bases'; `_to_object` of the abstract interfaces it
    /// inherits from (A.1.9.1). For an abstract interface, the protected
    /// constructors and destructor of A.1.9.2.
    pub(super) fn corba_interface_members(
        &self,
        w: &mut Writer,
        id: Id<zerodds_ast::InterfaceNode>,
        name: &str,
    ) -> Result<(), Idl4CppError> {
        let node = self
            .ast
            .interface(id)
            .map_err(|_| unresolved("an interface"))?;
        if node.kind == InterfaceKind::Abstract {
            // "they shall provide a protected default constructor, a
            // protected copy constructor, and a protected destructor (which
            // is virtual by virtue of inheritance from AbstractBase)".
            w.line("");
            w.line("protected:");
            w.line(&format!("{name}() = default;"));
            w.line(&format!("{name}(const {name}&) = default;"));
            w.line(&format!("~{name}() override = default;"));
            return Ok(());
        }
        let repository_id = self.repository_id(AnyId::Interface(id, node.node_id));
        let abstract_ancestors = self.abstract_ancestors(id)?;
        let mut tests = vec![format!(
            "logical_type_id == {}",
            text::string_literal(&repository_id)
        )];
        let mut bases = Vec::new();
        for base in resolved_all(&node.bases, "a base interface")? {
            let base_node = self
                .ast
                .interface(base)
                .map_err(|_| unresolved("a base interface"))?;
            if base_node.kind == InterfaceKind::Abstract {
                continue;
            }
            bases.push(self.interface_name(base)?);
        }
        if bases.is_empty() {
            bases.push(match node.kind {
                InterfaceKind::Local => "::CORBA::LocalObject".to_string(),
                _ => "::CORBA::Object".to_string(),
            });
        }
        for base in &bases {
            tests.push(format!("{base}::_is_a(logical_type_id)"));
        }
        for ancestor in &abstract_ancestors {
            let ancestor_node = self
                .ast
                .interface(*ancestor)
                .map_err(|_| unresolved("a base interface"))?;
            tests.push(format!(
                "logical_type_id == {}",
                text::string_literal(
                    &self.repository_id(AnyId::Interface(*ancestor, ancestor_node.node_id))
                )
            ));
        }
        w.line("");
        if !abstract_ancestors.is_empty() {
            // A.1.9.1: "If the concrete type of an abstract interface
            // instance is a normal object reference, the _to_object function
            // returns a reference to that object".
            w.line(
                "::omg::types::ref_type<::CORBA::Object> _to_object() override { return ::CORBA::Object::shared_from_this(); }",
            );
        }
        w.line(&format!(
            "bool _is_a(const std::string& logical_type_id) override {{ return {}; }}",
            tests.join(" || ")
        ));
        w.line(&format!(
            "std::string _repository_id() override {{ return {}; }}",
            text::string_literal(&repository_id)
        ));
        w.line(&format!(
            "static const char* _interface_repository_id() {{ return {}; }}",
            text::string_literal(&repository_id)
        ));
        Ok(())
    }

    /// Whether Table A.13 passes a value of this type by value: an
    /// interface, value type, `TypeCode`, enumeration or basic type; Table
    /// A.14 passes the rest by `const T&`.
    pub(super) fn corba_by_value(&self, ctype: &super::types::CppType) -> bool {
        ctype.by_value() || ctype.shape == super::types::Shape::Reference
    }

    /// The OBV class of a concrete value type (A.1.8.1, A.1.8.2, A.1.8.4),
    /// and, for a value type without operations and initializers, the
    /// concrete factory of A.1.8.9.2 that makes it.
    #[allow(clippy::too_many_lines)]
    pub(super) fn corba_valuetype_tail(
        &self,
        id: Id<TypeNode>,
        name: &str,
    ) -> Result<String, Idl4CppError> {
        let node = self.ast.type_(id).map_err(|_| unresolved("a value type"))?;
        let TypeKind::Valuetype {
            kind,
            bases,
            initializers,
            ..
        } = &node.kind
        else {
            return Err(unresolved("a value type"));
        };
        // A.1.8.7: "because they have no data members, the IDL compiler does
        // not generate the OBV traits for them".
        if *kind == zerodds_ast::ValuetypeKind::Abstract {
            return Ok(String::new());
        }
        let owner = self.origin_of_type(id);
        let concrete_base = bases
            .first()
            .and_then(Ref::resolved)
            .map(|b| self.through_aliases(b))
            .transpose()?
            .filter(|b| {
                matches!(
                    self.ast.type_(*b).map(|n| &n.kind),
                    Ok(TypeKind::Valuetype { kind, .. }) if *kind != zerodds_ast::ValuetypeKind::Abstract
                )
            });
        // The members of the concrete base first (A.1.8.2: "If the valuetype
        // inherits from a concrete valuetype, then parameters for the data
        // members of the inherited valuetype appear first"), then its own.
        let own_state = |vt: Id<TypeNode>| -> Vec<Id<zerodds_ast::MemberNode>> {
            match self.ast.type_(vt).map(|n| &n.kind) {
                Ok(TypeKind::Valuetype { state, .. }) => state.clone(),
                _ => Vec::new(),
            }
        };
        let mut chain = Vec::new();
        let mut current = concrete_base;
        while let Some(base) = current {
            if chain.len() > super::MAX_DEPTH {
                return Err(Idl4CppError::Unevaluated {
                    what: "a chain of base value types deeper than the mapping follows".to_string(),
                    at: name.to_string(),
                });
            }
            chain.push(base);
            current = match self.ast.type_(base).map(|n| &n.kind) {
                Ok(TypeKind::Valuetype { bases, .. }) => bases
                    .first()
                    .and_then(Ref::resolved)
                    .map(|b| self.through_aliases(b))
                    .transpose()?
                    .filter(|b| {
                        matches!(
                            self.ast.type_(*b).map(|n| &n.kind),
                            Ok(TypeKind::Valuetype { kind, .. }) if *kind != zerodds_ast::ValuetypeKind::Abstract
                        )
                    }),
                _ => None,
            };
        }
        chain.reverse();
        let mut base_members = Vec::new();
        for base in &chain {
            base_members.extend(own_state(*base));
        }
        let own = own_state(id);
        let obv = format!("OBV_{name}");
        let mut inherit = vec![format!("public virtual {}", self.qualified(id)?)];
        let base_obv = match concrete_base {
            Some(base) => {
                let qualified = self.qualified(base)?;
                let (scope, last) = qualified.rsplit_once("::").unwrap_or(("", &qualified));
                let base_obv = format!("{scope}::OBV_{last}");
                inherit.push(format!("public {base_obv}"));
                Some(base_obv)
            }
            None => None,
        };
        struct Field {
            name: String,
            text: String,
            by_value: bool,
            private: bool,
            init: String,
        }
        let field = |m_id: Id<zerodds_ast::MemberNode>| -> Result<Field, Idl4CppError> {
            let m = self
                .ast
                .member(m_id)
                .map_err(|_| unresolved("a state member"))?;
            let ctype = self.cpp_type(&m.type_, &format!("{owner}::{}", m.name))?;
            let init = match &ctype.shape {
                super::types::Shape::Enum { default } => default.clone(),
                _ => String::new(),
            };
            Ok(Field {
                name: names::name(&m.name),
                by_value: self.corba_by_value(&ctype),
                text: ctype.text,
                private: matches!(
                    m.facets.corba.as_ref().and_then(|c| c.state_visibility),
                    Some(zerodds_ast::StateVisibility::Private)
                ),
                init,
            })
        };
        let own_fields: Vec<Field> = own.iter().map(|m| field(*m)).collect::<Result<_, _>>()?;
        let base_fields: Vec<Field> = base_members
            .iter()
            .map(|m| field(*m))
            .collect::<Result<_, _>>()?;
        let accessors = |f: &Field| -> Vec<String> {
            let (n, t) = (&f.name, &f.text);
            if f.by_value {
                vec![
                    format!("{t} {n}() const override {{ return {n}_; }}"),
                    format!("{t}& {n}() override {{ return {n}_; }}"),
                    format!("void {n}({t} value) override {{ {n}_ = value; }}"),
                ]
            } else {
                vec![
                    format!("const {t}& {n}() const override {{ return {n}_; }}"),
                    format!("{t}& {n}() override {{ return {n}_; }}"),
                    format!("void {n}(const {t}& value) override {{ {n}_ = value; }}"),
                    format!("void {n}({t}&& value) override {{ {n}_ = std::move(value); }}"),
                ]
            }
        };
        let mut w = Writer::new();
        w.open(&format!("class {obv} : {}", inherit.join(", ")));
        w.line("public:");
        for f in own_fields.iter().filter(|f| !f.private) {
            for line in accessors(f) {
                w.line(&line);
            }
        }
        w.line("");
        w.line("protected:");
        // "The protected default constructor initializes object reference
        // members to appropriately-typed nil object references, basic data
        // types to their default value ..., and enums to their first
        // value."
        let inits: Vec<String> = own_fields
            .iter()
            .map(|f| format!("{}_({})", f.name, f.init))
            .collect();
        if inits.is_empty() {
            w.line(&format!("{obv}() = default;"));
        } else {
            w.line(&format!("{obv}() : {} {{}}", inits.join(", ")));
        }
        w.line(&format!("{obv}(const {obv}&) = default;"));
        w.line(&format!("{obv}({obv}&&) = default;"));
        let params: Vec<String> = base_fields
            .iter()
            .chain(own_fields.iter())
            .map(|f| {
                if f.by_value {
                    format!("{} {}", f.text, f.name)
                } else {
                    format!("const {}& {}", f.text, f.name)
                }
            })
            .collect();
        if !params.is_empty() {
            let mut sets = Vec::new();
            if let Some(base_obv) = &base_obv {
                let args: Vec<&str> = base_fields.iter().map(|f| f.name.as_str()).collect();
                sets.push(format!("{base_obv}({})", args.join(", ")));
            }
            sets.extend(own_fields.iter().map(|f| format!("{n}_({n})", n = f.name)));
            w.line(&format!(
                "explicit {obv}({}) : {} {{}}",
                params.join(", "),
                sets.join(", ")
            ));
        }
        w.line(&format!("~{obv}() override = default;"));
        for f in own_fields.iter().filter(|f| f.private) {
            for line in accessors(f) {
                w.line(&line);
            }
        }
        w.line("");
        w.line("private:");
        for f in &own_fields {
            w.line(&format!("{} {}_;", f.text, f.name));
        }
        w.close_with(";");
        // A.1.8.9.2: "For valuetypes that have no operations or initializers,
        // a concrete type-specific factory class is generated whose
        // implementation of the create_for_unmarshal function simply
        // constructs an instance of the CORBA::traits<>::obv_type trait class
        // for the valuetype using CORBA::traits<>::make_reference."
        let has_operations = self.value_has_operations(id)?;
        if initializers.is_empty() && !has_operations {
            let factory = format!("{name}_factory");
            w.line("");
            w.open(&format!(
                "class {factory} : public virtual ::CORBA::ValueFactoryBase"
            ));
            w.line("public:");
            w.line(&format!("{factory}() = default;"));
            w.line(&format!("~{factory}() override = default;"));
            w.line("");
            w.line("private:");
            w.open(&format!("struct instance final : {obv}"));
            w.line(&format!("instance() : {obv}() {{}}"));
            w.close_with(";");
            w.line(
                "::omg::types::ref_type<::CORBA::ValueBase> create_for_unmarshal() override { return ::CORBA::make_reference<instance>(); }",
            );
            w.close_with(";");
        }
        Ok(w.finish())
    }

    /// What A.1.2 adds to a user exception: `raise()`, `_name()`,
    /// `_rep_id()`, and the insertion into an `Any` that
    /// `operator<<=(Any&, const Exception&)` uses (A.1.2.2).
    pub(super) fn corba_exception_members(
        &self,
        w: &mut Writer,
        id: Id<zerodds_ast::ExceptionNode>,
    ) -> Result<(), Idl4CppError> {
        let node = self
            .ast
            .exception(id)
            .map_err(|_| unresolved("an exception"))?;
        let repository_id = self.repository_id(AnyId::Exception(id, node.node_id));
        let override_ = match self.options.standard {
            Standard::Cxx11 => " override",
            Standard::Cxx03 => "",
        };
        w.line("");
        w.line(&format!("void raise() const{override_} {{ throw *this; }}"));
        w.line(&format!(
            "const char* _name() const{override_} {{ return {}; }}",
            text::string_literal(&node.name)
        ));
        w.line(&format!(
            "const char* _rep_id() const{override_} {{ return {}; }}",
            text::string_literal(&repository_id)
        ));
        w.line(&format!(
            "void _insert_into(::CORBA::Any& any) const{override_};"
        ));
        Ok(())
    }

    /// A value box (A.1.8.6): a final class on `CORBA::ValueBase` with the
    /// `_value` accessors and modifiers, protected constructors and
    /// destructor, and deleted assignment.
    pub(super) fn valuebox(&self, id: Id<TypeNode>, name: &str) -> Result<String, Idl4CppError> {
        let node = self.ast.type_(id).map_err(|_| unresolved("a value box"))?;
        let TypeKind::ValueBox { boxed } = &node.kind else {
            return Err(unresolved("a value box"));
        };
        let at = self.origin_of_type(id);
        if self.options.platform != Platform::Corba {
            return Err(Idl4CppError::NoCppForm {
                construct: "a value box outside the CORBA platform (OMG IDL 4.2 places it in \
                            the building block CORBA-Specific – Value Types, which A.1.8.6 maps)"
                    .to_string(),
                at,
            });
        }
        let ctype = self.cpp_type(boxed, &at)?;
        let t = &ctype.text;
        let mut w = Writer::new();
        w.open(&format!("class {name} final : public ::CORBA::ValueBase"));
        w.line("public:");
        if ctype.by_value() {
            w.line(&format!("{t} _value() const {{ return value_; }}"));
        } else {
            w.line(&format!("const {t}& _value() const {{ return value_; }}"));
        }
        w.line(&format!("{t}& _value() {{ return value_; }}"));
        w.line(&format!(
            "void _value({} value) {{ value_ = value; }}",
            ctype.in_param()
        ));
        if !ctype.by_value() {
            w.line(&format!(
                "void _value({t}&& value) {{ value_ = std::move(value); }}"
            ));
        }
        w.line("");
        // "A protected destructor and protected constructors": the
        // instances are made by CORBA::make_reference, through this friend.
        w.line("protected:");
        w.line("friend struct ::CORBA::detail::reference_access;");
        w.line(&format!("{name}() : value_() {{}}"));
        w.line(&format!(
            "explicit {name}({} value) : value_(value) {{}}",
            ctype.in_param()
        ));
        w.line(&format!("{name}({name}&&) = default;"));
        w.line(&format!("{name}(const {name}&) = default;"));
        w.line(&format!("~{name}() override = default;"));
        w.line("");
        // "Private, deleted copy and move assignment operators."
        w.line("private:");
        w.line(&format!("{name}& operator=(const {name}&) = delete;"));
        w.line(&format!("{name}& operator=({name}&&) = delete;"));
        w.line(&format!("{t} value_;"));
        w.close_with(";");
        Ok(w.finish())
    }

    // -- A.1.3: type codes ---------------------------------------------------------

    /// The type code of the type `reference` names, as a C++ expression.
    pub(super) fn typecode_of(
        &self,
        reference: &Ref<TypeNode>,
        at: &str,
    ) -> Result<String, Idl4CppError> {
        let id = reference.resolved().ok_or_else(|| unresolved("a type"))?;
        self.typecode_at(id, at, 0)
    }

    fn typecode_at(
        &self,
        id: Id<TypeNode>,
        at: &str,
        depth: usize,
    ) -> Result<String, Idl4CppError> {
        if depth > super::MAX_DEPTH {
            return Err(Idl4CppError::Unevaluated {
                what: "a type code nested deeper than the mapping follows".to_string(),
                at: at.to_string(),
            });
        }
        let node = self.ast.type_(id).map_err(|_| unresolved("a type"))?;
        let tc = "::CORBA::TypeCode::";
        Ok(match &node.kind {
            TypeKind::Primitive(kind) => format!(
                "::CORBA::_tc_{}",
                match kind {
                    PrimitiveKind::Boolean => "boolean",
                    PrimitiveKind::Char => "char",
                    PrimitiveKind::WChar => "wchar",
                    PrimitiveKind::Octet | PrimitiveKind::UInt8 => "octet",
                    PrimitiveKind::Int8 | PrimitiveKind::Int16 => "short",
                    PrimitiveKind::UInt16 => "ushort",
                    PrimitiveKind::Int32 => "long",
                    PrimitiveKind::UInt32 => "ulong",
                    PrimitiveKind::Int64 => "longlong",
                    PrimitiveKind::UInt64 => "ulonglong",
                    PrimitiveKind::Float32 => "float",
                    PrimitiveKind::Float64 => "double",
                    PrimitiveKind::Float128 => "longdouble",
                }
            ),
            TypeKind::String { width, bound } => {
                let n = self.bound(*bound, at)?.unwrap_or(0);
                match width {
                    CharWidth::WChar => format!("{tc}create_wstring_tc({n})"),
                    CharWidth::Char => format!("{tc}create_string_tc({n})"),
                }
            }
            TypeKind::Fixed { digits, scale } => {
                format!("{tc}create_fixed_tc({digits}, {scale})")
            }
            TypeKind::Sequence { element, bound } => {
                let n = self.bound(*bound, at)?.unwrap_or(0);
                let element = element
                    .resolved()
                    .ok_or_else(|| unresolved("an element type"))?;
                format!(
                    "{tc}create_sequence_tc({n}, {})",
                    self.typecode_at(element, at, depth + 1)?
                )
            }
            TypeKind::Array { element, dims } => {
                let element = element
                    .resolved()
                    .ok_or_else(|| unresolved("an element type"))?;
                let mut out = self.typecode_at(element, at, depth + 1)?;
                for dim in dims.iter().rev() {
                    let length =
                        self.ast
                            .eval_integral(*dim)
                            .map_err(|_| Idl4CppError::Unevaluated {
                                what: "an array dimension".to_string(),
                                at: at.to_string(),
                            })?;
                    out = format!("{tc}create_array_tc({length}, {out})");
                }
                out
            }
            TypeKind::Any => "::CORBA::_tc_any".to_string(),
            TypeKind::ObjectRef { interface } => match interface.as_ref().and_then(Ref::resolved) {
                Some(target) => {
                    let n = self
                        .ast
                        .interface(target)
                        .map_err(|_| unresolved("an interface"))?;
                    let rid = self.repository_id(AnyId::Interface(target, n.node_id));
                    let create = match n.kind {
                        InterfaceKind::Local => "create_local_interface_tc",
                        InterfaceKind::Abstract => "create_abstract_interface_tc",
                        _ => "create_interface_tc",
                    };
                    format!(
                        "{tc}{create}({}, {})",
                        text::string_literal(&rid),
                        text::string_literal(&n.name)
                    )
                }
                None => "::CORBA::_tc_Object".to_string(),
            },
            TypeKind::ValueBase => "::CORBA::_tc_ValueBase".to_string(),
            TypeKind::Native => format!(
                "{tc}create_native_tc({}, {})",
                text::string_literal(&self.repository_id(AnyId::Type(id, node.node_id))),
                text::string_literal(&node.name)
            ),
            TypeKind::Map { .. } | TypeKind::Bitset { .. } | TypeKind::Bitmask { .. } => {
                return Err(Idl4CppError::NoCppForm {
                    construct: "a type code for a map, bit set or bit mask (CORBA 3.3 part 1 \
                                §8.11.1 has no kind for them; OMG IDL 4.2 §9.2 leaves Extended \
                                Data Types out of the CORBA profiles)"
                        .to_string(),
                    at: at.to_string(),
                });
            }
            TypeKind::AnnotationType { .. } => {
                return Err(Idl4CppError::NoCppForm {
                    construct: "a type code for an annotation".to_string(),
                    at: at.to_string(),
                });
            }
            // A declared type: its `_tc_` constant once the header has
            // defined it, a recursive type code until then.
            _ => {
                if self.typecodes.borrow().contains(&id) {
                    self.typecode_name(id)?
                } else {
                    format!(
                        "{tc}create_recursive_tc({})",
                        text::string_literal(&self.repository_id(AnyId::Type(id, node.node_id)))
                    )
                }
            }
        })
    }

    /// The qualified name of a type's `_tc_` constant.
    fn typecode_name(&self, id: Id<TypeNode>) -> Result<String, Idl4CppError> {
        let qualified = self.qualified(id)?;
        let (scope, name) = qualified.rsplit_once("::").unwrap_or(("", &qualified));
        Ok(format!("{scope}::_tc_{name}"))
    }

    /// The `_tc_` constant and the `Any` operators of a type declaration
    /// under the CORBA platform; `None` for a declaration that has neither
    /// (a map, bit set or bit mask, a typedef of one).
    #[allow(clippy::too_many_lines)]
    pub(super) fn corba_type_extras(
        &self,
        id: Id<TypeNode>,
    ) -> Result<Option<Extras>, Idl4CppError> {
        let node = self.ast.type_(id).map_err(|_| unresolved("a type"))?;
        let at = self.origin_of_type(id);
        let rid = text::string_literal(&self.repository_id(AnyId::Type(id, node.node_id)));
        let idl_name = text::string_literal(&node.name);
        let tc = "::CORBA::TypeCode::";
        let name = names::name(&node.name);
        let member = |m_id: Id<zerodds_ast::MemberNode>,
                      label: Option<String>,
                      visibility: &str|
         -> Result<String, Idl4CppError> {
            let m = self.ast.member(m_id).map_err(|_| unresolved("a member"))?;
            let label = label.map_or_else(
                || "nullptr".to_string(),
                |l| format!("std::make_shared<::CORBA::Any>({l})"),
            );
            Ok(format!(
                "::CORBA::TypeCode::Member{{{}, {}, {label}, {visibility}}}",
                text::string_literal(&m.name),
                self.typecode_of(&m.type_, &at)?
            ))
        };
        let (typecode, by_value) = match &node.kind {
            TypeKind::Structure { members, base } => {
                let mut list = Vec::new();
                if let Some(base) = base.as_ref().and_then(Ref::resolved) {
                    for inherited in self.ast.all_members_of(self.through_aliases(base)?) {
                        list.push(member(inherited, None, "::CORBA::PUBLIC_MEMBER")?);
                    }
                }
                for m in members {
                    list.push(member(*m, None, "::CORBA::PUBLIC_MEMBER")?);
                }
                (
                    format!(
                        "{tc}create_struct_tc({rid}, {idl_name}, {{{}}})",
                        list.join(", ")
                    ),
                    false,
                )
            }
            TypeKind::Union {
                discriminator,
                cases,
                default_case,
            } => {
                let disc = discriminator
                    .resolved()
                    .ok_or_else(|| unresolved("a discriminator"))?;
                let disc_type = self.cpp_type(discriminator, &at)?;
                let disc_id = self.through_aliases(disc)?;
                let mut list = Vec::new();
                for case in cases.iter().chain(default_case.iter()) {
                    let c = self.ast.case(*case).map_err(|_| unresolved("a case"))?;
                    for label in &c.labels {
                        let any = match label {
                            // "For the default member, the label is the zero
                            // octet" (CORBA 3.3 part 1 §8.11.1).
                            CaseLabel::Default => {
                                "::CORBA::detail::any_of(static_cast<std::uint8_t>(0))".to_string()
                            }
                            CaseLabel::Value(expr) => {
                                let value = self.ast.eval_integral(*expr).map_err(|_| {
                                    Idl4CppError::Unevaluated {
                                        what: "a case label".to_string(),
                                        at: at.clone(),
                                    }
                                })?;
                                format!(
                                    "::CORBA::detail::any_of({})",
                                    self.discriminator_literal_for(
                                        &disc_type, disc_id, value, &at
                                    )?
                                )
                            }
                        };
                        list.push(member(c.member, Some(any), "::CORBA::PUBLIC_MEMBER")?);
                    }
                }
                (
                    format!(
                        "{tc}create_union_tc({rid}, {idl_name}, {}, {{{}}})",
                        self.typecode_of(discriminator, &at)?,
                        list.join(", ")
                    ),
                    false,
                )
            }
            TypeKind::Enumeration { .. } => {
                let mut list = Vec::new();
                for e in self.ast.enumerators_of(id) {
                    let n = self
                        .ast
                        .enumerator(*e)
                        .map_err(|_| unresolved("an enumerator"))?;
                    list.push(text::string_literal(&n.name));
                }
                (
                    format!(
                        "{tc}create_enum_tc({rid}, {idl_name}, {{{}}})",
                        list.join(", ")
                    ),
                    true,
                )
            }
            TypeKind::Alias { aliased } => {
                let target = aliased.resolved().ok_or_else(|| unresolved("a typedef"))?;
                let behind = self.through_aliases(target)?;
                if matches!(
                    self.ast.type_(behind).map(|n| &n.kind),
                    Ok(TypeKind::Map { .. } | TypeKind::Bitset { .. } | TypeKind::Bitmask { .. })
                ) {
                    return Ok(None);
                }
                (
                    format!(
                        "{tc}create_alias_tc({rid}, {idl_name}, {})",
                        self.typecode_of(aliased, &at)?
                    ),
                    false,
                )
            }
            TypeKind::Valuetype {
                kind, bases, state, ..
            } => {
                let modifier = match kind {
                    zerodds_ast::ValuetypeKind::Abstract => "::CORBA::VM_ABSTRACT",
                    zerodds_ast::ValuetypeKind::Custom => "::CORBA::VM_CUSTOM",
                    zerodds_ast::ValuetypeKind::Truncatable => "::CORBA::VM_TRUNCATABLE",
                    zerodds_ast::ValuetypeKind::Regular => "::CORBA::VM_NONE",
                };
                let base = match bases.first().and_then(Ref::resolved) {
                    Some(base) => self.typecode_at(self.through_aliases(base)?, &at, 0)?,
                    None => "nullptr".to_string(),
                };
                let mut list = Vec::new();
                for m in state {
                    let n = self
                        .ast
                        .member(*m)
                        .map_err(|_| unresolved("a state member"))?;
                    let visibility = match n.facets.corba.as_ref().and_then(|c| c.state_visibility)
                    {
                        Some(zerodds_ast::StateVisibility::Private) => "::CORBA::PRIVATE_MEMBER",
                        _ => "::CORBA::PUBLIC_MEMBER",
                    };
                    list.push(member(*m, None, visibility)?);
                }
                (
                    format!(
                        "{tc}create_value_tc({rid}, {idl_name}, {modifier}, {base}, {{{}}})",
                        list.join(", ")
                    ),
                    true,
                )
            }
            TypeKind::ValueBox { boxed } => (
                format!(
                    "{tc}create_value_box_tc({rid}, {idl_name}, {})",
                    self.typecode_of(boxed, &at)?
                ),
                true,
            ),
            _ => return Ok(None),
        };
        self.typecodes.borrow_mut().insert(id);
        let tc_name = format!("_tc_{name}");
        let qualified = self.qualified(id)?;
        let mut ops = Writer::new();
        match &node.kind {
            // A.1.7.4: an alias "cannot be generated anyway"; its underlying
            // type has the operators, when it is one of the header's.
            TypeKind::Alias { .. } => {}
            TypeKind::Valuetype { .. } | TypeKind::ValueBox { .. } => {
                let reference = format!("::omg::types::ref_type<{qualified}>");
                ops.line(&format!(
                    "inline void operator<<=(::CORBA::Any& any, {reference} value) {{ any._insert<::omg::types::ref_type<::CORBA::ValueBase>>(std::move(value), {tc_name}); }}"
                ));
                ops.line(&format!(
                    "inline bool operator>>=(const ::CORBA::Any& any, {reference}& value) {{ ::omg::types::ref_type<::CORBA::ValueBase> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<{qualified}>(base); return !base || value; }}"
                ));
            }
            _ if by_value => {
                ops.line(&format!(
                    "inline void operator<<=(::CORBA::Any& any, {qualified} value) {{ any._insert<{qualified}>(value, {tc_name}); }}"
                ));
                ops.line(&format!(
                    "inline bool operator>>=(const ::CORBA::Any& any, {qualified}& value) {{ return any._extract<{qualified}>(value); }}"
                ));
            }
            _ => {
                ops.line(&format!(
                    "inline void operator<<=(::CORBA::Any& any, const {qualified}& value) {{ any._insert<{qualified}>(value, {tc_name}); }}"
                ));
                ops.line(&format!(
                    "inline void operator<<=(::CORBA::Any& any, {qualified}&& value) {{ any._insert<{qualified}>(std::move(value), {tc_name}); }}"
                ));
                ops.line(&format!(
                    "inline bool operator>>=(const ::CORBA::Any& any, {qualified}& value) {{ return any._extract<{qualified}>(value); }}"
                ));
            }
        }
        Ok(Some(Extras {
            name: tc_name,
            typecode,
            operators: ops.finish(),
        }))
    }

    /// The `_tc_` constant and the `Any` operators of an exception.
    pub(super) fn corba_exception_extras(
        &self,
        id: Id<zerodds_ast::ExceptionNode>,
    ) -> Result<Extras, Idl4CppError> {
        let node = self
            .ast
            .exception(id)
            .map_err(|_| unresolved("an exception"))?;
        let at = self.origin(AnyId::Exception(id, node.node_id));
        let rid = text::string_literal(&self.repository_id(AnyId::Exception(id, node.node_id)));
        let mut list = Vec::new();
        for m in &node.members {
            let n = self.ast.member(*m).map_err(|_| unresolved("a member"))?;
            list.push(format!(
                "::CORBA::TypeCode::Member{{{}, {}, nullptr, ::CORBA::PUBLIC_MEMBER}}",
                text::string_literal(&n.name),
                self.typecode_of(&n.type_, &at)?
            ));
        }
        let name = names::name(&node.name);
        let tc_name = format!("_tc_{name}");
        let qualified = self.exception_name(id)?;
        let mut ops = Writer::new();
        ops.line(&format!(
            "inline void operator<<=(::CORBA::Any& any, const {qualified}& value) {{ any._insert<{qualified}>(value, {tc_name}); }}"
        ));
        ops.line(&format!(
            "inline void operator<<=(::CORBA::Any& any, {qualified}&& value) {{ any._insert<{qualified}>(std::move(value), {tc_name}); }}"
        ));
        ops.line(&format!(
            "inline bool operator>>=(const ::CORBA::Any& any, {qualified}& value) {{ return any._extract<{qualified}>(value); }}"
        ));
        ops.line(&format!(
            "inline void {qualified}::_insert_into(::CORBA::Any& any) const {{ any <<= *this; }}"
        ));
        Ok(Extras {
            name: tc_name,
            typecode: format!(
                "::CORBA::TypeCode::create_exception_tc({rid}, {}, {{{}}})",
                text::string_literal(&node.name),
                list.join(", ")
            ),
            operators: ops.finish(),
        })
    }

    /// The `_tc_` constant and the `Any` operators of an interface: an
    /// object reference goes in as a reference to `CORBA::Object` (an
    /// abstract one to `CORBA::AbstractBase`), so that it comes out as the
    /// base reference A.1.7.3 asks for, and as itself by narrowing.
    pub(super) fn corba_interface_extras(
        &self,
        id: Id<zerodds_ast::InterfaceNode>,
    ) -> Result<Extras, Idl4CppError> {
        let node = self
            .ast
            .interface(id)
            .map_err(|_| unresolved("an interface"))?;
        let rid = text::string_literal(&self.repository_id(AnyId::Interface(id, node.node_id)));
        let create = match node.kind {
            InterfaceKind::Local => "create_local_interface_tc",
            InterfaceKind::Abstract => "create_abstract_interface_tc",
            _ => "create_interface_tc",
        };
        let base = if node.kind == InterfaceKind::Abstract {
            "::CORBA::AbstractBase"
        } else {
            "::CORBA::Object"
        };
        let name = names::name(&node.name);
        let tc_name = format!("_tc_{name}");
        let qualified = self.interface_name(id)?;
        let reference = format!("::omg::types::ref_type<{qualified}>");
        let mut ops = Writer::new();
        ops.line(&format!(
            "inline void operator<<=(::CORBA::Any& any, {reference} value) {{ any._insert<::omg::types::ref_type<{base}>>(std::move(value), {tc_name}); }}"
        ));
        ops.line(&format!(
            "inline bool operator>>=(const ::CORBA::Any& any, {reference}& value) {{ ::omg::types::ref_type<{base}> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<{qualified}>(base); return !base || value; }}"
        ));
        Ok(Extras {
            name: tc_name,
            typecode: format!(
                "::CORBA::TypeCode::{create}({rid}, {})",
                text::string_literal(&node.name)
            ),
            operators: ops.finish(),
        })
    }

    /// The `_tc_` constant and the operators at namespace scope.
    pub(super) fn extras_text(extras: &Extras) -> String {
        format!(
            "static const ::omg::types::ref_type<::CORBA::TypeCode> {} = {};\n{}",
            extras.name, extras.typecode, extras.operators
        )
    }

    /// The literal of a discriminator value, for a union's type code.
    fn discriminator_literal_for(
        &self,
        disc: &super::types::CppType,
        disc_id: Id<TypeNode>,
        value: i128,
        at: &str,
    ) -> Result<String, Idl4CppError> {
        match &disc.shape {
            super::types::Shape::Bool => Ok(if value == 0 { "false" } else { "true" }.to_string()),
            super::types::Shape::Enum { .. } => self
                .enumerator_with_value(disc_id, value)?
                .ok_or_else(|| Idl4CppError::Unevaluated {
                    what: format!("an enumerator of value {value}"),
                    at: at.to_string(),
                }),
            _ => Ok(format!("static_cast<{}>({value})", disc.text)),
        }
    }

    // -- A.1.1: traits -------------------------------------------------------------------

    /// The `CORBA::traits` specializations of A.1.1 for what the header
    /// declares.
    pub(super) fn corba_traits(&self, items: &[Item]) -> Result<String, Idl4CppError> {
        let mut w = Writer::new();
        for id in self.declared_types(items) {
            let node = self.ast.type_(id).map_err(|_| unresolved("a type"))?;
            let q = self.qualified(id)?;
            match &node.kind {
                TypeKind::Structure { .. } | TypeKind::Union { .. } | TypeKind::Bitset { .. } => {
                    w.line(&format!(
                        "template <> struct traits<{q}> : detail::common_traits<{q}, false> {{}};"
                    ));
                }
                // A.1.1.8: "the traits template is specialized on the
                // <Bitmask>Bits type".
                TypeKind::Bitmask { .. } => {
                    let bound = self.ast.effective_bit_bound(id);
                    w.line(&format!(
                        "template <> struct traits<{q}::_flags> : detail::common_traits<{q}::_flags, true> {{"
                    ));
                    w.line(&format!(
                        "    using bit_bound = std::integral_constant<std::uint32_t, {bound}>;"
                    ));
                    w.line(&format!(
                        "    using underlying_type = ::omg::types::underlying_type_t<{q}>;"
                    ));
                    w.line("};");
                }
                TypeKind::Valuetype {
                    kind, initializers, ..
                } => {
                    let abstract_ = *kind == zerodds_ast::ValuetypeKind::Abstract;
                    let truncatable = *kind == zerodds_ast::ValuetypeKind::Truncatable;
                    // The abstract factory of the initializers, or the
                    // concrete one of a value type without operations
                    // (A.1.8.9.2); ValueFactoryBase for the rest, whose
                    // factory the application writes.
                    let has_operations = self.value_has_operations(id)?;
                    let factory = if !initializers.is_empty() || (!abstract_ && !has_operations) {
                        format!("{q}_factory")
                    } else {
                        "::CORBA::ValueFactoryBase".to_string()
                    };
                    w.line(&format!(
                        "template <> struct traits<{q}> : detail::reference_traits<{q}> {{"
                    ));
                    w.line(&format!(
                        "    using is_abstract = std::{}_type;",
                        if abstract_ { "true" } else { "false" }
                    ));
                    w.line(&format!(
                        "    using is_truncatable = std::{}_type;",
                        if truncatable { "true" } else { "false" }
                    ));
                    w.line(&format!("    using factory_type = {factory};"));
                    // A.1.8: "An IDL valuetype shall be mapped to the C++
                    // trait CORBA::traits<>::base_type."
                    w.line(&format!("    using base_type = {q};"));
                    if !abstract_ {
                        let (scope, last) = q.rsplit_once("::").unwrap_or(("", &q));
                        w.line(&format!("    using obv_type = {scope}::OBV_{last};"));
                    }
                    w.line(&format!(
                        "    static ::omg::types::ref_type<{q}> narrow(::omg::types::ref_type<::CORBA::ValueBase> p) {{ return std::dynamic_pointer_cast<{q}>(p); }}"
                    ));
                    // A.1.9.1: "to facilitate narrowing from an abstract
                    // interface base class down to ... derived valuetype
                    // types".
                    if !self.abstract_interfaces_of_value(id)?.is_empty() {
                        w.line(&format!(
                            "    static ::omg::types::ref_type<{q}> narrow(::omg::types::ref_type<::CORBA::AbstractBase> p) {{ return std::dynamic_pointer_cast<{q}>(p); }}"
                        ));
                    }
                    w.line("};");
                }
                TypeKind::ValueBox { boxed } => {
                    let at = self.origin_of_type(id);
                    let ctype = self.cpp_type(boxed, &at)?;
                    w.line(&format!(
                        "template <> struct traits<{q}> : detail::reference_traits<{q}> {{"
                    ));
                    w.line(&format!("    using boxed_traits = traits<{}>;", ctype.text));
                    w.line("};");
                }
                _ => {}
            }
        }
        for item in items {
            if let Item::Interface(id) = item {
                let node = self
                    .ast
                    .interface(*id)
                    .map_err(|_| unresolved("an interface"))?;
                if node.completeness == zerodds_ast::Completeness::Forward {
                    continue;
                }
                let q = self.interface_name(*id)?;
                let base = if node.kind == InterfaceKind::Abstract {
                    "::CORBA::AbstractBase"
                } else {
                    "::CORBA::Object"
                };
                w.line(&format!(
                    "template <> struct traits<{q}> : detail::reference_traits<{q}> {{"
                ));
                w.line(&format!(
                    "    using is_local = std::{}_type;",
                    if node.kind == InterfaceKind::Local {
                        "true"
                    } else {
                        "false"
                    }
                ));
                w.line(&format!(
                    "    using is_abstract = std::{}_type;",
                    if node.kind == InterfaceKind::Abstract {
                        "true"
                    } else {
                        "false"
                    }
                ));
                // A.1.6: the class "shall be available as
                // CORBA::traits<>::base_type".
                w.line(&format!("    using base_type = {q};"));
                w.line(&format!(
                    "    static ::omg::types::ref_type<{q}> narrow(::omg::types::ref_type<{base}> p) {{ return std::dynamic_pointer_cast<{q}>(p); }}"
                ));
                if node.kind != InterfaceKind::Abstract && !self.abstract_ancestors(*id)?.is_empty()
                {
                    w.line(&format!(
                        "    static ::omg::types::ref_type<{q}> narrow(::omg::types::ref_type<::CORBA::AbstractBase> p) {{ return std::dynamic_pointer_cast<{q}>(p); }}"
                    ));
                }
                w.line("};");
            }
        }
        Ok(w.finish())
    }
}
