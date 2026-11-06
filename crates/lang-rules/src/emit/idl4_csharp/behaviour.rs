// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Interfaces (§7.4, §7.5, A.1.5), value types (§7.6), user annotations
//! (§7.16).

use super::text::Writer;
use super::types::Shape;
use super::{Gen, Idl4CsharpError, Platform, Settings};
use zerodds_ast::{
    AnyId, Facets, InterfaceKind, ParamDirection, StateVisibility, TypeKind, TypeNode,
};
use zerodds_ast_core::{Id, Ref};

/// One member of an interface: a method or a property.
#[derive(Clone)]
struct Signature {
    /// The attributes on it.
    attributes: Vec<String>,
    /// `T name(params)` or `T name { get; set; }`, without modifiers.
    text: String,
    /// Whether it is a property.
    property: bool,
    /// The key that tells two apart: the name and the parameter types.
    key: String,
    /// The IDL name, for messages.
    idl: String,
    /// The result type, for a method.
    returns: String,
    /// The C# name.
    name: String,
    /// The parameter list as written, and as passed on (`ref x`, `out y`).
    params: String,
    arguments: String,
    /// The interface that declares it, qualified: `I<Name>Operations`
    /// under §7.5, `I<Name>` under §7.4.
    owner: String,
}

impl Gen<'_> {
    // -- user annotations, §7.16 ----------------------------------------------

    /// The C# attributes a node carries, §7.16.2: "IDL elements annotated
    /// with user-defined annotations shall map to equivalent C# elements
    /// annotated with the corresponding attribute". Several applications
    /// are several attributes (`AllowMultiple = true`, §7.16.1).
    pub(super) fn user_annotations(&self, facets: &Facets) -> Result<Vec<String>, Idl4CsharpError> {
        let mut out = Vec::new();
        for applied in &facets.annotations {
            let Some(def) = applied.definition.resolved() else {
                continue;
            };
            if self.is_builtin(def) {
                continue;
            }
            out.push(self.annotation_instance(applied, def)?);
        }
        Ok(out)
    }

    /// The attributes of a member: those of the typedefs its type is
    /// reached through — §7.16.2.1: "Annotations on an IDL typedef shall
    /// be applied to uses of the typedef in other type declarations" — and
    /// its own.
    pub(super) fn member_annotations(
        &self,
        facets: &Facets,
        type_: &Ref<TypeNode>,
    ) -> Result<Vec<String>, Idl4CsharpError> {
        let mut out = Vec::new();
        let mut current = type_.resolved();
        let mut depth = 0;
        while let Some(id) = current {
            depth += 1;
            if depth > super::MAX_DEPTH {
                break;
            }
            match self.ast.type_(id).map(|node| (&node.kind, &node.facets)) {
                Ok((TypeKind::Alias { aliased }, alias_facets)) => {
                    out.extend(self.user_annotations(alias_facets)?);
                    current = aliased.resolved();
                }
                _ => break,
            }
        }
        out.extend(self.user_annotations(facets)?);
        Ok(out)
    }

    fn is_builtin(&self, def: Id<zerodds_ast::AnnotationDefNode>) -> bool {
        self.ast
            .annotation_def(def)
            .ok()
            .and_then(|node| self.ast.module(node.module).ok())
            .is_some_and(|module| module.name == zerodds_ast::node::annotation::BUILTIN_MODULE_NAME)
    }

    /// `[global::Ns.Name(member = value, …)]`.
    fn annotation_instance(
        &self,
        applied: &zerodds_ast::AppliedAnnotation,
        def: Id<zerodds_ast::AnnotationDefNode>,
    ) -> Result<String, Idl4CsharpError> {
        let node = self
            .ast
            .annotation_def(def)
            .map_err(|_| unresolved("an annotation", &applied.name))?;
        let settings = self.settings_in(node.module, None, &node.facets);
        let class = format!(
            "{}{}",
            self.qualified_prefix(node.module),
            settings.naming.annotation_name(&node.name)
        );
        let mut pairs = Vec::new();
        for argument in &applied.arguments {
            let member = argument
                .name
                .as_deref()
                .and_then(|written| {
                    node.members.iter().find_map(|id| {
                        self.ast
                            .annotation_member(*id)
                            .ok()
                            .filter(|m| m.name.eq_ignore_ascii_case(written))
                    })
                })
                .or_else(|| match node.members.as_slice() {
                    [only] => self.ast.annotation_member(*only).ok(),
                    _ => None,
                })
                .ok_or_else(|| {
                    unresolved("the member an annotation argument names", &applied.name)
                })?;
            let at = format!("@{}.{}", node.name, member.name);
            let ctype = self.attribute_parameter_type(&member.type_, &settings, &at)?;
            let value =
                self.ast
                    .eval_const(argument.value)
                    .map_err(|_| Idl4CsharpError::Unevaluated {
                        what: "an annotation argument".to_string(),
                        at: at.clone(),
                    })?;
            pairs.push(format!(
                "{} = {}",
                settings.naming.member(&member.name),
                self.cs_value(&value, &ctype, &at)?
            ));
        }
        Ok(if pairs.is_empty() {
            format!("[{class}]")
        } else {
            format!("[{class}({})]", pairs.join(", "))
        })
    }

    /// The C# type of an annotation member: what an attribute's named
    /// argument admits (ECMA-334 §22.2.4).
    fn attribute_parameter_type(
        &self,
        reference: &Ref<TypeNode>,
        settings: &Settings,
        at: &str,
    ) -> Result<super::CType, Idl4CsharpError> {
        let ctype = self.cs_type(reference, settings, at)?;
        match &ctype.shape {
            Shape::Bool
            | Shape::Char { .. }
            | Shape::Integer { .. }
            | Shape::Float
            | Shape::Double
            | Shape::Str { .. }
            | Shape::Enum { .. } => Ok(ctype),
            _ => Err(Idl4CsharpError::NoCsharpForm {
                construct: format!(
                    "an annotation member of type `{}`, which an attribute parameter cannot have \
                     (ECMA-334 §22.2.4)",
                    ctype.text
                ),
                at: at.to_string(),
            }),
        }
    }

    /// The attribute class of a user-defined IDL annotation, §7.16.1.
    pub(super) fn annotation_type(
        &mut self,
        def: Id<zerodds_ast::AnnotationDefNode>,
    ) -> Result<(), Idl4CsharpError> {
        if self.is_builtin(def) {
            return Ok(());
        }
        let node = self
            .ast
            .annotation_def(def)
            .map_err(|_| unresolved("an annotation", ""))?;
        let module = node.module;
        let settings = self.settings_in(module, None, &node.facets);
        let name = settings.naming.annotation_name(&node.name);
        let origin = self.origin(AnyId::AnnotationDef(def, node.node_id));
        let mut members = Vec::new();
        for member in &node.members {
            let m = self
                .ast
                .annotation_member(*member)
                .map_err(|_| unresolved("an annotation member", &name))?;
            let at = format!("@{}.{}", node.name, m.name);
            let ctype = self.attribute_parameter_type(&m.type_, &settings, &at)?;
            let default = match m.default {
                Some(expr) => {
                    let value =
                        self.ast
                            .eval_const(expr)
                            .map_err(|_| Idl4CsharpError::Unevaluated {
                                what: "an annotation member's default".to_string(),
                                at: at.clone(),
                            })?;
                    Some(self.cs_value(&value, &ctype, &at)?)
                }
                None => None,
            };
            members.push((
                settings.naming.member(&m.name),
                settings.naming.parameter(&m.name),
                ctype.text,
                default,
            ));
        }
        let mut w = Writer::new();
        w.line(&format!(
            "/// <summary>The IDL annotation <c>@{}</c> (§7.16.1).</summary>",
            node.name
        ));
        w.line(
            "[global::System.AttributeUsage(global::System.AttributeTargets.All, AllowMultiple = true)]",
        );
        w.open(&format!("public class {name} : global::System.Attribute"));
        w.line("/// <summary>The members at their defaults.</summary>");
        w.open(&format!("public {name}()"));
        for (property, _, _, default) in &members {
            if let Some(default) = default {
                w.line(&format!("this.{property} = {default};"));
            }
        }
        w.close();
        if !members.is_empty() {
            let params: Vec<String> = members
                .iter()
                .map(|(_, param, text, _)| format!("{text} {param}"))
                .collect();
            w.line("");
            w.open(&format!("public {name}({})", params.join(", ")));
            for (property, param, _, _) in &members {
                w.line(&format!("this.{property} = {param};"));
            }
            w.close();
        }
        for (property, _, text, _) in &members {
            w.line("");
            w.line(&format!("public {text} {property} {{ get; set; }}"));
        }
        w.close();
        let body = w.finish();
        self.file(module, &name, &origin, &body)
    }

    // -- interfaces, §7.4, §7.5 and A.1.5 ---------------------------------------

    /// An interface declared in a module: its files.
    pub(super) fn top_level_interface(
        &mut self,
        id: Id<zerodds_ast::InterfaceNode>,
    ) -> Result<(), Idl4CsharpError> {
        let node = self
            .ast
            .interface(id)
            .map_err(|_| unresolved("an interface", ""))?;
        // §7.4.2: "An IDL interface forward declaration has no mapping to
        // the C# language."
        if node.completeness == zerodds_ast::Completeness::Forward {
            return Ok(());
        }
        let origin = self.origin(AnyId::Interface(id, node.node_id));
        // A.1.2: `CORBA::TypeCode` is the class `Corba.TypeCode` the runtime
        // holds.
        if node.name == "TypeCode" && self.is_corba_module(node.module) {
            return Ok(());
        }
        match node.kind {
            InterfaceKind::Interface | InterfaceKind::Local | InterfaceKind::Abstract => {}
            // A port type is the grouping of basic ports its ports were
            // replaced by (§7.12).
            InterfaceKind::PortType => return Ok(()),
            other => {
                return Err(Idl4CsharpError::NoCsharpForm {
                    construct: format!("a {} the intermediate IDL did not replace", other.as_str()),
                    at: origin,
                });
            }
        }
        let module = node.module;
        let settings = self.settings_in(module, None, &node.facets);
        let embedded = self.embedded(module, &node.name);
        // §7.5 when the body embeds declarations; A.1.5 always on the CORBA
        // platform.
        let full = !embedded.is_empty() || self.options.platform == Platform::Corba;
        let interface = settings.naming.interface_name(&node.name);
        let signatures = self.interface_signatures(id, &settings, &node.name)?;
        let mut bases = Vec::new();
        for base in &node.bases {
            let base = base
                .resolved()
                .ok_or_else(|| unresolved("a base interface", &node.name))?;
            bases.push(self.interface_name(base)?);
        }
        let attributes = self.user_annotations(&node.facets)?;
        if !full {
            let mut w = Writer::new();
            for attribute in &attributes {
                w.line(attribute);
            }
            let extends = if bases.is_empty() {
                String::new()
            } else {
                format!(" : {}", bases.join(", "))
            };
            w.open(&format!("public interface {interface}{extends}"));
            write_signatures(&mut w, &signatures, "");
            w.close();
            return self.file(module, &interface, &origin, &w.finish());
        }
        // §7.5: I<Name>Operations with the methods and attributes,
        // I<Name> inheriting from it, and the class <Name> holding the
        // embedded declarations.
        let operations = format!("{interface}Operations");
        let mut w = Writer::new();
        w.open(&format!("public interface {operations}"));
        write_signatures(&mut w, &signatures, "");
        w.close();
        self.file(module, &operations, &origin, &w.finish())?;

        let mut extends = Vec::new();
        if self.options.platform == Platform::Corba {
            extends.push("global::Corba.IObject".to_string());
        }
        extends.extend(bases);
        extends.push(operations.clone());
        let mut w = Writer::new();
        for attribute in &attributes {
            w.line(attribute);
        }
        w.open(&format!(
            "public interface {interface} : {}",
            extends.join(", ")
        ));
        w.close();
        self.file(module, &interface, &origin, &w.finish())?;

        // The class: abstract, implementing I<Name> with the operations and
        // attributes of the IDL interface left abstract ("The class may
        // inherit from I<InterfaceName> to provide an implementation"),
        // and on the CORBA platform derived from Corba.Object (A.1.5).
        let class = settings.naming.type_name(&node.name);
        let mut all = self.all_signatures(id, &node.name)?;
        let mut explicit = Vec::new();
        if self.options.platform == Platform::Corba {
            // Corba.Object declares these already: one of the same result
            // is the inherited abstract member; one of another result is
            // implemented explicitly for its interface, through a protected
            // abstract method of its own name.
            all.retain(|signature| {
                match OBJECT_OPERATIONS
                    .iter()
                    .find(|(key, _)| *key == signature.key)
                {
                    Some((_, returns)) if *returns == signature.returns => false,
                    Some(_) => {
                        explicit.push(signature.clone());
                        false
                    }
                    None => true,
                }
            });
        }
        let mut w = Writer::new();
        let base = if self.options.platform == Platform::Corba {
            "global::Corba.Object, "
        } else {
            ""
        };
        w.open(&format!(
            "public abstract class {class} : {base}{interface}"
        ));
        self.embedded_declarations(&mut w, module, &node.name)?;
        write_signatures(&mut w, &all, "public abstract ");
        for signature in &explicit {
            let own = format!("{}_{}", signature.name.trim_start_matches('@'), class);
            w.line("");
            w.line(&format!(
                "{} {}.{}({}) {{ {}{own}({}); }}",
                signature.returns,
                signature.owner,
                signature.name,
                signature.params,
                if signature.returns == "void" {
                    ""
                } else {
                    "return "
                },
                signature.arguments
            ));
            w.line("");
            w.line(&format!(
                "protected abstract {} {own}({});",
                signature.returns, signature.params
            ));
        }
        w.close();
        self.file(module, &class, &origin, &w.finish())
    }

    /// The interface that holds an IDL interface's operations: on the
    /// CORBA platform and for an interface with embedded declarations
    /// `I<Name>Operations` (§7.5), otherwise `I<Name>` (§7.4).
    fn operations_interface(
        &self,
        id: Id<zerodds_ast::InterfaceNode>,
    ) -> Result<String, Idl4CsharpError> {
        let node = self
            .ast
            .interface(id)
            .map_err(|_| unresolved("an interface", ""))?;
        let full = self.options.platform == Platform::Corba
            || !self.embedded(node.module, &node.name).is_empty();
        let name = self.interface_name(id)?;
        Ok(if full {
            format!("{name}Operations")
        } else {
            name
        })
    }

    /// The members an interface declares: its attributes as properties and
    /// its operations as methods (§7.4).
    fn interface_signatures(
        &self,
        id: Id<zerodds_ast::InterfaceNode>,
        settings: &Settings,
        owner: &str,
    ) -> Result<Vec<Signature>, Idl4CsharpError> {
        let mut out = Vec::new();
        for attribute in self.ast.attributes_of(id) {
            out.push(self.attribute_property(*attribute, settings, owner)?);
        }
        for operation in self.ast.operations_of(id) {
            out.push(self.operation_method(*operation, settings, owner)?);
        }
        check_signature_collisions(&out, owner)?;
        Ok(out)
    }

    /// Every member of an interface and its bases, each once: what a class
    /// implementing the interface has to declare.
    fn all_signatures(
        &self,
        interface: Id<zerodds_ast::InterfaceNode>,
        owner: &str,
    ) -> Result<Vec<Signature>, Idl4CsharpError> {
        let mut out: Vec<Signature> = Vec::new();
        let mut queue = vec![interface];
        let mut seen = Vec::new();
        while let Some(next) = queue.pop() {
            if seen.contains(&next) || seen.len() > super::MAX_DEPTH {
                continue;
            }
            seen.push(next);
            let node = self
                .ast
                .interface(next)
                .map_err(|_| unresolved("an interface", owner))?;
            let settings = self.settings_in(node.module, None, &node.facets);
            let declaring = self.operations_interface(next)?;
            for mut signature in self.interface_signatures(next, &settings, &node.name)? {
                signature.owner = declaring.clone();
                if !out.iter().any(|s| s.key == signature.key) {
                    out.push(signature);
                }
            }
            queue.extend(node.bases.iter().filter_map(Ref::resolved));
        }
        Ok(out)
    }

    /// The declarations an interface's or a value type's body embeds, as
    /// public nested declarations (§7.5).
    fn embedded_declarations(
        &self,
        w: &mut Writer,
        module: Id<zerodds_ast::ModuleNode>,
        scope: &str,
    ) -> Result<(), Idl4CsharpError> {
        let embedded = self.embedded(module, scope);
        for type_ in &embedded.types {
            for (_, source) in self.type_declaration(*type_)? {
                w.line("");
                w.nested(&source);
            }
        }
        for exception in &embedded.exceptions {
            let node = self
                .ast
                .exception(*exception)
                .map_err(|_| unresolved("an exception", scope))?;
            let settings = self.settings_in(module, Some(scope), &node.facets);
            let class = settings.naming.type_name(&node.name);
            w.line("");
            w.nested(&self.exception(*exception, &class)?);
        }
        for (_, _, source) in self.constant_classes(module, Some(scope), &embedded.constants)? {
            w.line("");
            w.nested(&source);
        }
        Ok(())
    }

    /// An attribute as a property: "Properties representing attributes
    /// shall have a getter and a setter. If the attribute is read only,
    /// the mapping shall omit the setter."
    fn attribute_property(
        &self,
        id: Id<zerodds_ast::AttributeNode>,
        settings: &Settings,
        owner: &str,
    ) -> Result<Signature, Idl4CsharpError> {
        let node = self
            .ast
            .attribute(id)
            .map_err(|_| unresolved("an attribute", owner))?;
        let at = format!("{owner}.{}", node.name);
        let ctype = self.cs_type(&node.type_, settings, &at)?;
        let name = settings.naming.member(&node.name);
        let accessors = if node.readonly {
            "{ get; }"
        } else {
            "{ get; set; }"
        };
        Ok(Signature {
            attributes: self.user_annotations(&node.facets)?,
            text: format!("{} {name} {accessors}", ctype.text),
            property: true,
            key: name.clone(),
            idl: node.name.clone(),
            returns: ctype.text,
            name,
            params: String::new(),
            arguments: String::new(),
            owner: String::new(),
        })
    }

    /// An operation as a method (§7.4): parameters in IDL order, `out` as
    /// `out`, `inout` as `ref`.
    fn operation_method(
        &self,
        id: Id<zerodds_ast::OperationNode>,
        settings: &Settings,
        owner: &str,
    ) -> Result<Signature, Idl4CsharpError> {
        let node = self
            .ast
            .operation(id)
            .map_err(|_| unresolved("an operation", owner))?;
        let at = format!("{owner}.{}", node.name);
        let returns = match &node.return_type {
            Some(reference) => self.cs_type(reference, settings, &at)?.text,
            None => "void".to_string(),
        };
        let mut params = Vec::new();
        let mut types = Vec::new();
        let mut arguments = Vec::new();
        for param in &node.params {
            let p = self
                .ast
                .param(*param)
                .map_err(|_| unresolved("a parameter", &at))?;
            let ctype = self.cs_type(&p.type_, settings, &format!("{at}({})", p.name))?;
            let modifier = match p.direction {
                ParamDirection::In => "",
                ParamDirection::Out => "out ",
                ParamDirection::InOut => "ref ",
            };
            let mut attributes = self.user_annotations(&p.facets)?;
            attributes.push(String::new());
            types.push(format!("{modifier}{}", ctype.text));
            arguments.push(format!("{modifier}{}", settings.naming.parameter(&p.name)));
            params.push(format!(
                "{}{modifier}{} {}",
                attributes.join(" ").trim_start(),
                ctype.text,
                settings.naming.parameter(&p.name)
            ));
        }
        let name = settings.naming.member(&node.name);
        let mut attributes = self.user_annotations(&node.facets)?;
        // A value type's initializer (§7.6): "annotated with the
        // FactoryAttribute".
        if self.is_initializer(id) {
            attributes.insert(0, "[global::Omg.Types.Factory]".to_string());
        }
        Ok(Signature {
            attributes,
            text: format!("{returns} {name}({})", params.join(", ")),
            property: false,
            key: format!("{name}({})", types.join(", ")),
            idl: node.name.clone(),
            returns,
            name,
            params: params.join(", "),
            arguments: arguments.join(", "),
            owner: String::new(),
        })
    }

    /// Whether an operation is a value type's initializer (`factory`).
    fn is_initializer(&self, id: Id<zerodds_ast::OperationNode>) -> bool {
        self.ast.types().any(|(_, node)| match &node.kind {
            TypeKind::Valuetype { initializers, .. } => initializers.contains(&id),
            _ => false,
        })
    }

    // -- value types, §7.6 and A.1.6 -------------------------------------------

    /// "An IDL valuetype type shall be mapped to a C# abstract class."
    pub(super) fn valuetype(
        &self,
        id: Id<TypeNode>,
        name: &str,
    ) -> Result<String, Idl4CsharpError> {
        let node = self
            .ast
            .type_(id)
            .map_err(|_| unresolved("a value type", name))?;
        let settings = self.settings_of_type(id);
        if let TypeKind::ValueBox { boxed } = &node.kind {
            // A boxed value is a value of one member; §7.6 names no form of
            // its own, so it is a class holding that member.
            let ctype = self.cs_type(boxed, &settings, name)?;
            let property = settings.naming.member("value");
            let mut w = Writer::new();
            for attribute in self.user_annotations(&node.facets)? {
                w.line(&attribute);
            }
            w.open(&format!("public class {name}"));
            w.line(&format!("public {name}() {{ }}"));
            w.line("");
            w.line(&format!(
                "public {name}({} value) {{ this.{property} = value; }}",
                ctype.text
            ));
            w.line("");
            w.line(&format!("public {} {property} {{ get; set; }}", ctype.text));
            w.close();
            return Ok(w.finish());
        }
        let TypeKind::Valuetype {
            bases,
            state,
            initializers,
            supported,
            ..
        } = &node.kind
        else {
            return Err(unresolved("a value type", name));
        };
        if bases.len() > 1 {
            return Err(Idl4CsharpError::NoCsharpForm {
                construct: "a value type with more than one base, which a C# class cannot \
                            extend (§7.6 maps a base to a base class)"
                    .to_string(),
                at: name.to_string(),
            });
        }
        let mut inherits = Vec::new();
        if let Some(base) = bases.first().and_then(Ref::resolved) {
            inherits.push(self.qualified(self.through_aliases(base, name)?)?);
        }
        let mut supported_ids = Vec::new();
        for interface in supported {
            let interface = interface
                .resolved()
                .ok_or_else(|| unresolved("a supported interface", name))?;
            inherits.push(self.interface_name(interface)?);
            supported_ids.push(interface);
        }
        let mut w = Writer::new();
        for attribute in self.user_annotations(&node.facets)? {
            w.line(&attribute);
        }
        let inherit_text = if inherits.is_empty() {
            String::new()
        } else {
            format!(" : {}", inherits.join(", "))
        };
        w.open(&format!("public abstract class {name}{inherit_text}"));
        // Attributes: properties.
        for attribute in self.ast.attributes_of_value(id) {
            let signature = self.attribute_property(attribute, &settings, name)?;
            w.line("");
            for a in &signature.attributes {
                w.line(a);
            }
            w.line(&format!("public {}", signature.text));
        }
        // "valuetype members shall be mapped onto the abstract class the
        // same way as struct members, with the addition that private
        // members shall have the C# protected access modifier".
        for member in state {
            let m = self
                .ast
                .member(*member)
                .map_err(|_| unresolved("a state member", name))?;
            let ctype = self.cs_type(&m.type_, &settings, &format!("{name}.{}", m.name))?;
            let visibility = match m.facets.corba.as_ref().and_then(|c| c.state_visibility) {
                Some(StateVisibility::Private) => "protected",
                _ => "public",
            };
            w.line("");
            for attribute in self.user_annotations(&m.facets)? {
                w.line(&attribute);
            }
            w.line(&format!(
                "{visibility} {} {} {{ get; set; }}",
                ctype.text,
                settings.naming.member(&m.name)
            ));
        }
        // Operations, then initializers as abstract void methods with
        // [Omg.Types.Factory].
        let mut methods = Vec::new();
        for operation in self.ast.operations_of_value(id) {
            if initializers.contains(&operation) {
                continue;
            }
            methods.push(self.operation_method(operation, &settings, name)?);
        }
        for initializer in initializers {
            methods.push(self.operation_method(*initializer, &settings, name)?);
        }
        for interface in &supported_ids {
            for signature in self.all_signatures(*interface, name)? {
                if !methods.iter().any(|m| m.key == signature.key) {
                    methods.push(signature);
                }
            }
        }
        check_signature_collisions(&methods, name)?;
        write_signatures(&mut w, &methods, "public abstract ");
        self.embedded_declarations(&mut w, node.module, &node.name)?;
        w.close();
        Ok(w.finish())
    }
}

/// The operations of `Corba.Object` (CORBA 3.3 part 1 §8.3) by the key
/// [`Signature::key`] gives them, for an interface that declares one of
/// them with the same parameters.
const OBJECT_OPERATIONS: [(&str, &str); 18] = [
    (
        "create_request(global::Corba.IContext, string, global::Corba.INVList, ref global::Corba.NamedValue, out global::Corba.IRequest, uint)",
        "void",
    ),
    (
        "set_policy_overrides(global::Omg.Types.ISequence<global::Corba.IPolicy>, global::Corba.SetOverrideType)",
        "global::Corba.IObject",
    ),
    (
        "get_policy_overrides(global::Omg.Types.ISequence<uint>)",
        "global::Omg.Types.ISequence<global::Corba.IPolicy>",
    ),
    (
        "validate_connection(out global::Omg.Types.ISequence<global::Corba.IPolicy>)",
        "bool",
    ),
    ("get_interface()", "global::Corba.IInterfaceDef"),
    ("is_nil()", "bool"),
    ("duplicate()", "global::Corba.IObject"),
    ("release()", "void"),
    ("is_a(string)", "bool"),
    ("non_existent()", "bool"),
    ("is_equivalent(global::Corba.IObject)", "bool"),
    ("hash(uint)", "uint"),
    ("get_policy(uint)", "global::Corba.IPolicy"),
    (
        "get_domain_managers()",
        "global::Omg.Types.ISequence<global::Corba.IDomainManager>",
    ),
    ("get_client_policy(uint)", "global::Corba.IPolicy"),
    ("get_component()", "global::Corba.IObject"),
    ("repository_id()", "string"),
    ("get_orb()", "global::Corba.IORB"),
];

/// The members of an interface or class, each after its attributes, with
/// `prefix` (`public abstract ` in a class, nothing in an interface).
fn write_signatures(w: &mut Writer, signatures: &[Signature], prefix: &str) {
    for signature in signatures {
        w.line("");
        for attribute in &signature.attributes {
            w.line(attribute);
        }
        if signature.property {
            w.line(&format!("{prefix}{}", signature.text));
        } else {
            w.line(&format!("{prefix}{};", signature.text));
        }
    }
}

/// Two members of one interface that C# cannot tell apart.
fn check_signature_collisions(
    signatures: &[Signature],
    owner: &str,
) -> Result<(), Idl4CsharpError> {
    for (index, signature) in signatures.iter().enumerate() {
        if let Some(other) = signatures[..index].iter().find(|s| s.key == signature.key) {
            return Err(Idl4CsharpError::Collision {
                name: signature.key.clone(),
                first: format!("{owner}.{}", other.idl),
                second: format!("{owner}.{}", signature.idl),
            });
        }
    }
    Ok(())
}

fn unresolved(what: &str, at: &str) -> Idl4CsharpError {
    Idl4CsharpError::Unresolved {
        what: what.to_string(),
        at: at.to_string(),
    }
}
