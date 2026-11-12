// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Interfaces (§7.4, §7.4.2, §7.5, Annex C.2.2) and value types (§7.6).

use super::text::Writer;
use super::types::{CppType, Shape};
use super::{Gen, Idl4CppError, Platform, Standard, names, resolved_all, unresolved};
use zerodds_ast::{AnyId, Completeness, ParamDirection, StateVisibility, TypeKind, TypeNode};
use zerodds_ast_core::Id;

impl Gen<'_> {
    /// The parameter type of an argument (§7.4): "IDL out and inout
    /// arguments shall be passed by reference. IDL in arguments shall be
    /// passed by value if the resulting type after resolving any typedef
    /// C++ is a built-in type or an enum, and as a reference to const
    /// otherwise." Under Annex C.2.2 an interface is `T*`, passed as it
    /// is.
    pub(super) fn param_text(&self, ctype: &CppType, direction: ParamDirection) -> String {
        match direction {
            ParamDirection::In => {
                // Annex C.2.2 passes `T*` as it is; Table A.13 passes the
                // reference of an interface or value type by value.
                if ctype.shape == Shape::Reference
                    && (self.options.standard == Standard::Cxx03
                        || self.options.platform == Platform::Corba)
                {
                    ctype.text.clone()
                } else {
                    ctype.in_param()
                }
            }
            ParamDirection::Out | ParamDirection::InOut => format!("{}&", ctype.text),
        }
    }

    /// The type an operation returns, refused for a C array (Annex C has
    /// no array a function can return).
    fn return_text(&self, ctype: &CppType, at: &str) -> Result<String, Idl4CppError> {
        if !ctype.suffix.is_empty() {
            return Err(Idl4CppError::NoCppForm {
                construct: "an anonymous array as a result".to_string(),
                at: at.to_string(),
            });
        }
        Ok(ctype.text.clone())
    }

    /// `virtual R name(params) = 0;` for an operation.
    pub(super) fn operation_text(
        &self,
        id: Id<zerodds_ast::OperationNode>,
        owner: &str,
        suffix: &str,
    ) -> Result<String, Idl4CppError> {
        let node = self
            .ast
            .operation(id)
            .map_err(|_| unresolved("an operation"))?;
        let at = format!("{owner}::{}", node.name);
        let returns = match &node.return_type {
            Some(reference) => {
                let ctype = self.cpp_type(reference, &at)?;
                if self.options.standard == Standard::Cxx03 && ctype.suffix.is_empty() {
                    ctype.text
                } else {
                    self.return_text(&ctype, &at)?
                }
            }
            None => "void".to_string(),
        };
        let mut params = Vec::new();
        for param in &node.params {
            let p = self
                .ast
                .param(*param)
                .map_err(|_| unresolved("a parameter"))?;
            let ctype = self.cpp_type(&p.type_, &format!("{at}({})", p.name))?;
            params.push(format!(
                "{} {}",
                self.param_text(&ctype, p.direction),
                names::name(&p.name)
            ));
        }
        Ok(format!(
            "virtual {returns} {}({}){suffix} = 0;",
            names::name(&node.name),
            params.join(", ")
        ))
    }

    /// The accessor and, unless it is read only, the modifier of an
    /// attribute (§7.4): "a pair of pure virtual methods in the C++ class
    /// ... with the same name as the attribute".
    pub(super) fn attribute_text(
        &self,
        id: Id<zerodds_ast::AttributeNode>,
        owner: &str,
    ) -> Result<Vec<String>, Idl4CppError> {
        let node = self
            .ast
            .attribute(id)
            .map_err(|_| unresolved("an attribute"))?;
        let at = format!("{owner}::{}", node.name);
        let ctype = self.cpp_type(&node.type_, &at)?;
        let name = names::name(&node.name);
        let mut out = Vec::new();
        if !node.readonly {
            out.push(format!(
                "virtual void {name}({} value) = 0;",
                self.param_text(&ctype, ParamDirection::In)
            ));
        }
        out.push(format!(
            "virtual {} {name}() const = 0;",
            self.return_text(&ctype, &at)?
        ));
        Ok(out)
    }

    /// The declarations of an interface's or value type's body (§7.5):
    /// "adding to mapped class the declaration every type, exception, or
    /// constant declaration in the IDL interface body". Under the CORBA
    /// platform each nested type's `_tc_` constant is a static member and
    /// its `Any` operators are friends; what C++ takes only outside the
    /// class — the definitions of those constants before C++17 and of an
    /// exception's `_insert_into` — goes to `after`, written after the
    /// enclosing class by the caller.
    pub(super) fn embedded_declarations(
        &self,
        w: &mut Writer,
        module: Id<zerodds_ast::ModuleNode>,
        scope: &str,
        owner: &str,
        after: &mut Vec<String>,
    ) -> Result<(), Idl4CppError> {
        let corba = self.options.platform == Platform::Corba;
        let embedded = self.embedded(module, scope);
        let tc_type = "::omg::types::ref_type<::CORBA::TypeCode>";
        let nested = |w: &mut Writer, extras: &super::corba::Extras, after: &mut Vec<String>| {
            w.line("#if __cplusplus >= 201703L");
            w.line(&format!(
                "static inline const {tc_type} {} = {};",
                extras.name, extras.typecode
            ));
            w.line("#else");
            w.line(&format!("static const {tc_type} {};", extras.name));
            w.line("#endif");
            // `ref_type<TypeCode> ::M::I::_tc_T` would read as a name in
            // `ref_type<TypeCode>`; the definition stands in the
            // namespace of the class, where the name without the leading
            // `::` resolves.
            after.push(format!(
                "#if __cplusplus < 201703L\nconst {tc_type} {}::{} = {};\n#endif",
                owner.trim_start_matches("::"),
                extras.name,
                extras.typecode
            ));
            for line in extras.operators.lines() {
                if line.contains("::_insert_into(") {
                    after.push(line.to_string());
                } else if let Some(rest) = line.strip_prefix("inline ") {
                    w.line(&format!("friend {rest}"));
                } else {
                    w.line(line);
                }
            }
        };
        for type_ in &embedded.types {
            let source = self.type_declaration(*type_)?;
            if !source.is_empty() {
                w.line("");
                w.nested(&nested_free_functions(&source));
                if corba && let Some(extras) = self.corba_type_extras(*type_)? {
                    nested(w, &extras, after);
                }
            }
        }
        for exception in &embedded.exceptions {
            w.line("");
            w.nested(&self.exception(*exception)?);
            if corba {
                let extras = self.corba_exception_extras(*exception)?;
                nested(w, &extras, after);
            }
        }
        for constant in &embedded.constants {
            w.line(&self.constant(*constant, true)?);
        }
        Ok(())
    }

    // -- interfaces, §7.4 and §7.5 ---------------------------------------------------

    pub(super) fn interface(
        &self,
        id: Id<zerodds_ast::InterfaceNode>,
    ) -> Result<String, Idl4CppError> {
        let node = self
            .ast
            .interface(id)
            .map_err(|_| unresolved("an interface"))?;
        // §7.4.2: "An IDL interface forward declaration shall be mapped to
        // a partial forward declaration in C++", which every interface has
        // ahead of the definitions.
        if node.completeness == Completeness::Forward {
            return Ok(String::new());
        }
        let name = names::name(&node.name);
        let owner = self.origin(AnyId::Interface(id, node.node_id));
        let mut bases = Vec::new();
        for base in resolved_all(&node.bases, "a base interface")? {
            bases.push(format!("public virtual {}", self.interface_name(base)?));
        }
        let corba = self.options.platform == Platform::Corba;
        if corba && self.corba_needs_object_base(id)? {
            bases.insert(0, self.corba_object_base(id)?);
        }
        // A.1.9.2 gives an abstract interface a protected destructor,
        // written with its constructors.
        let public_destructor = !(corba && self.corba_abstract_interface(id));
        let inherit = if bases.is_empty() {
            String::new()
        } else {
            format!(" : {}", bases.join(", "))
        };
        let mut w = Writer::new();
        let mut after = Vec::new();
        let qualified = self.interface_name(id)?;
        w.open(&format!("class {name}{inherit}"));
        w.line("public:");
        self.embedded_declarations(&mut w, node.module, &node.name, &qualified, &mut after)?;
        w.line("");
        match self.options.standard {
            _ if !public_destructor => {}
            Standard::Cxx11 => w.line(&format!("virtual ~{name}() = default;")),
            Standard::Cxx03 => w.line(&format!("virtual ~{name}() {{}}")),
        }
        for attribute in self.ast.attributes_of(id).iter().copied() {
            w.line("");
            for line in self.attribute_text(attribute, &owner)? {
                w.line(&line);
            }
        }
        for operation in self.ast.operations_of(id).iter().copied() {
            w.line("");
            w.line(&self.operation_text(operation, &owner, "")?);
        }
        if self.options.platform == Platform::Corba {
            self.corba_interface_members(&mut w, id, &name)?;
        }
        w.close_with(";");
        for text in &after {
            w.nested(text);
        }
        Ok(w.finish())
    }

    // -- value types, §7.6 -------------------------------------------------------------

    /// "An IDL valuetype shall be mapped to a C++ class with the same name
    /// as the IDL valuetype." Public state members are public pure virtual
    /// accessors and modifiers, private ones protected; a base value type
    /// is a `public virtual` base; the operations of a supported interface
    /// are pure virtual methods of the class; factory operations are the
    /// pure virtual methods of `<Name>_factory`.
    pub(super) fn valuetype(&self, id: Id<TypeNode>, name: &str) -> Result<String, Idl4CppError> {
        let node = self.ast.type_(id).map_err(|_| unresolved("a value type"))?;
        let TypeKind::Valuetype {
            bases,
            state,
            initializers,
            supported,
            ..
        } = &node.kind
        else {
            return Err(unresolved("a value type"));
        };
        let owner = self.origin_of_type(id);
        let mut inherit = Vec::new();
        for base in resolved_all(bases, "a base value type")? {
            inherit.push(format!(
                "public virtual {}",
                self.qualified(self.through_aliases(base)?)?
            ));
        }
        let mut to_value = false;
        if self.options.platform == Platform::Corba {
            if inherit.is_empty() {
                inherit.push("public virtual ::CORBA::ValueBase".to_string());
            }
            // A.1.8.8: "Interface classes supported by the IDL valuetype
            // are not inherited (except for abstract interfaces because
            // here the valuetype class has to support implicit widening".
            for interface in resolved_all(supported, "a supported interface")? {
                if self.corba_abstract_interface(interface) {
                    inherit.push(format!(
                        "public virtual {}",
                        self.interface_name(interface)?
                    ));
                    to_value = true;
                }
            }
        }
        let inherit_text = if inherit.is_empty() {
            String::new()
        } else {
            format!(" : {}", inherit.join(", "))
        };
        let mut public = Vec::new();
        let mut protected = Vec::new();
        for attribute in self.ast.attributes_of_value(id) {
            public.extend(self.attribute_text(attribute, &owner)?);
        }
        for operation in self.ast.operations_of_value(id) {
            if initializers.contains(&operation) {
                continue;
            }
            public.push(self.operation_text(operation, &owner, "")?);
        }
        for member in state {
            let m = self
                .ast
                .member(*member)
                .map_err(|_| unresolved("a state member"))?;
            let at = format!("{owner}::{}", m.name);
            let ctype = self.cpp_type(&m.type_, &at)?;
            let n = names::name(&m.name);
            let t = self.return_text(&ctype, &at)?;
            let lines = if self.options.platform == Platform::Corba && !self.corba_by_value(&ctype)
            {
                // A.1.8.1: the accessors and modifiers of Clause 8.1.1 for
                // a structured type, pure virtual.
                vec![
                    format!("virtual const {t}& {n}() const = 0;"),
                    format!("virtual {t}& {n}() = 0;"),
                    format!("virtual void {n}(const {t}& value) = 0;"),
                    format!("virtual void {n}({t}&& value) = 0;"),
                ]
            } else {
                vec![
                    format!(
                        "virtual void {n}({} value) = 0;",
                        self.param_text(&ctype, ParamDirection::In)
                    ),
                    format!("virtual {t} {n}() const = 0;"),
                    format!("virtual {t}& {n}() = 0;"),
                ]
            };
            match m.facets.corba.as_ref().and_then(|c| c.state_visibility) {
                Some(StateVisibility::Private) => protected.extend(lines),
                _ => public.extend(lines),
            }
        }
        // "If the IDL valuetype supports an interface type, all the
        // operations in the interface, and corresponding base interfaces if
        // any, shall be mapped to pure virtual methods in the mapped
        // class."
        let mut seen = Vec::new();
        let mut queue = resolved_all(supported, "a supported interface")?;
        while let Some(next) = queue.pop() {
            if seen.contains(&next) {
                continue;
            }
            seen.push(next);
            let interface = self
                .ast
                .interface(next)
                .map_err(|_| unresolved("a supported interface"))?;
            let interface_owner = self.origin(AnyId::Interface(next, interface.node_id));
            for attribute in self.ast.attributes_of(next).iter().copied() {
                for line in self.attribute_text(attribute, &interface_owner)? {
                    if !public.contains(&line) {
                        public.push(line);
                    }
                }
            }
            for operation in self.ast.operations_of(next).iter().copied() {
                let line = self.operation_text(operation, &interface_owner, "")?;
                if !public.contains(&line) {
                    public.push(line);
                }
            }
            queue.extend(resolved_all(&interface.bases, "a base interface")?);
        }
        let mut w = Writer::new();
        let mut after = Vec::new();
        let qualified = self.qualified(id)?;
        w.open(&format!("class {name}{inherit_text}"));
        w.line("public:");
        self.embedded_declarations(&mut w, node.module, &node.name, &qualified, &mut after)?;
        let corba = self.options.platform == Platform::Corba;
        if !corba {
            w.line("");
            match self.options.standard {
                Standard::Cxx11 => w.line(&format!("virtual ~{name}() = default;")),
                Standard::Cxx03 => w.line(&format!("virtual ~{name}() {{}}")),
            }
        }
        for line in &public {
            w.line(line);
        }
        if to_value {
            // A.1.9.1: "If the concrete type is a valuetype, _to_value
            // returns a reference to that valuetype".
            w.line(
                "::omg::types::ref_type<::CORBA::ValueBase> _to_value() override { return ::CORBA::ValueBase::shared_from_this(); }",
            );
        }
        if corba || !protected.is_empty() {
            w.line("");
            w.line("protected:");
            // A.1.8.2: "a protected default constructor, protected copy
            // constructor, protected move constructor, and a protected
            // virtual destructor".
            if corba {
                w.line(&format!("{name}() = default;"));
                w.line(&format!("{name}(const {name}&) = default;"));
                w.line(&format!("{name}({name}&&) = default;"));
                w.line(&format!("~{name}() override = default;"));
            }
            for line in &protected {
                w.line(line);
            }
        }
        if corba {
            // The assignments the example of A.1.8.4 deletes.
            w.line("");
            w.line("private:");
            w.line(&format!("{name}& operator=(const {name}&) = delete;"));
            w.line(&format!("{name}& operator=({name}&&) = delete;"));
        }
        w.close_with(";");
        for text in &after {
            w.nested(text);
        }
        if !initializers.is_empty() {
            // "If the valuetype contains factory operations, the mapping
            // shall declare a class named <ValueTypeName>_factory in the
            // same scope ... a pure virtual method for every factory
            // operation, returning either std::shared_ptr<T> or
            // omg::types::ref_type<T> ... a protected virtual destructor,
            // and a protected constructor."
            let reference = self.reference(&self.qualified(id)?).text;
            w.line("");
            let factory = format!("{name}_factory");
            let base = if self.options.platform == Platform::Corba {
                " : public virtual ::CORBA::ValueFactoryBase"
            } else {
                ""
            };
            w.open(&format!("class {factory}{base}"));
            w.line("public:");
            for initializer in initializers {
                let op = self
                    .ast
                    .operation(*initializer)
                    .map_err(|_| unresolved("a factory"))?;
                let at = format!("{owner}::{}", op.name);
                let mut params = Vec::new();
                for param in &op.params {
                    let p = self
                        .ast
                        .param(*param)
                        .map_err(|_| unresolved("a parameter"))?;
                    let ctype = self.cpp_type(&p.type_, &format!("{at}({})", p.name))?;
                    params.push(format!(
                        "{} {}",
                        self.param_text(&ctype, ParamDirection::In),
                        names::name(&p.name)
                    ));
                }
                w.line(&format!(
                    "virtual {reference} {}({}) = 0;",
                    names::name(&op.name),
                    params.join(", ")
                ));
            }
            w.line("");
            w.line("protected:");
            match self.options.standard {
                Standard::Cxx11 if self.options.platform == Platform::Corba => {
                    // A.1.8.9.2: "a protected virtual destructor, a protected
                    // default constructor, deleted copy/move constructors,
                    // and deleted copy/move assignment operators".
                    w.line(&format!("~{factory}() override = default;"));
                    w.line(&format!("{factory}() = default;"));
                    w.line("");
                    w.line("private:");
                    w.line(&format!("{factory}(const {factory}&) = delete;"));
                    w.line(&format!("{factory}({factory}&&) = delete;"));
                    w.line(&format!("{factory}& operator=(const {factory}&) = delete;"));
                    w.line(&format!("{factory}& operator=({factory}&&) = delete;"));
                }
                Standard::Cxx11 => {
                    w.line(&format!("virtual ~{factory}() = default;"));
                    w.line(&format!("{factory}() = default;"));
                }
                Standard::Cxx03 => {
                    w.line(&format!("virtual ~{factory}() {{}}"));
                    w.line(&format!("{factory}() {{}}"));
                }
            }
            w.close_with(";");
        }
        if self.options.platform == Platform::Corba {
            let tail = self.corba_valuetype_tail(id, name)?;
            if !tail.is_empty() {
                w.line("");
                w.nested(&tail);
            }
        }
        Ok(w.finish())
    }
}

/// A declaration written for a namespace, moved into a class body (§7.5):
/// its free functions — the comparison operators and `swap` of a
/// structure or union, the comparison operators of a bit set — become
/// `friend` functions, which is how a class body declares a function of
/// the enclosing namespace.
fn nested_free_functions(source: &str) -> String {
    source
        .lines()
        .map(|line| {
            if let Some(rest) = line.strip_prefix("inline ") {
                format!("friend {rest}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
