// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Interfaces (§7.4, §7.5), value types (§7.6), user annotations
//! (§7.16).

use super::text::Writer;
use super::types::Shape;
use super::{Gen, Idl4JavaError, Settings, abstract_class};
use zerodds_ast::{
    AnyId, Facets, InterfaceKind, ParamDirection, StateVisibility, TypeKind, TypeNode,
};
use zerodds_ast_core::{Id, Ref};

/// One method of an interface or a value type.
struct Method {
    /// The annotations on it.
    annotations: Vec<String>,
    /// `T name(params) throws E` without a body.
    signature: String,
    /// The name, for messages.
    idl: String,
}

impl Gen<'_> {
    // -- user annotations, §7.16 ----------------------------------------------

    /// The Java annotations a node carries, §7.16.2: "the corresponding
    /// Java element shall be annotated with the mapped Java annotation of
    /// the same name", and the `Group` annotation for several instances.
    pub(super) fn user_annotations(&self, facets: &Facets) -> Result<Vec<String>, Idl4JavaError> {
        self.annotations_of(&facets.annotations)
    }

    /// The Java annotations of a member: those of the typedefs its type
    /// is reached through — §7.2.4.6: "Annotations on an IDL typedef shall
    /// be applied to uses of the typedef in other type declarations" — and
    /// its own.
    pub(super) fn member_annotations(
        &self,
        facets: &Facets,
        type_: &Ref<TypeNode>,
    ) -> Result<Vec<String>, Idl4JavaError> {
        let mut applied: Vec<zerodds_ast::AppliedAnnotation> = Vec::new();
        let mut current = type_.resolved();
        let mut depth = 0;
        while let Some(id) = current {
            depth += 1;
            if depth > super::MAX_DEPTH {
                break;
            }
            match self.ast.type_(id).map(|node| (&node.kind, &node.facets)) {
                Ok((TypeKind::Alias { aliased }, alias_facets)) => {
                    applied.extend(alias_facets.annotations.iter().cloned());
                    current = aliased.resolved();
                }
                _ => break,
            }
        }
        applied.extend(facets.annotations.iter().cloned());
        self.annotations_of(&applied)
    }

    fn annotations_of(
        &self,
        annotations: &[zerodds_ast::AppliedAnnotation],
    ) -> Result<Vec<String>, Idl4JavaError> {
        let mut seen = Vec::new();
        let mut out = Vec::new();
        for applied in annotations {
            let Some(def) = applied.definition.resolved() else {
                continue;
            };
            if self.is_builtin(def) || seen.contains(&def) {
                continue;
            }
            seen.push(def);
            let same: Vec<_> = annotations
                .iter()
                .filter(|other| other.definition.resolved() == Some(def))
                .collect();
            let class = self.annotation_class(def)?;
            if let [only] = same.as_slice() {
                out.push(self.annotation_instance(only, def, &class)?);
            } else {
                let mut instances = Vec::new();
                for each in same {
                    instances.push(self.annotation_instance(each, def, &class)?);
                }
                out.push(format!("@{class}Group({{{}}})", instances.join(", ")));
            }
        }
        Ok(out)
    }

    fn is_builtin(&self, def: Id<zerodds_ast::AnnotationDefNode>) -> bool {
        self.ast
            .annotation_def(def)
            .ok()
            .and_then(|node| self.ast.module(node.module).ok())
            .is_some_and(|module| module.name == zerodds_ast::node::annotation::BUILTIN_MODULE_NAME)
    }

    /// The qualified name of the Java annotation type of an IDL
    /// annotation.
    fn annotation_class(
        &self,
        def: Id<zerodds_ast::AnnotationDefNode>,
    ) -> Result<String, Idl4JavaError> {
        let node = self
            .ast
            .annotation_def(def)
            .map_err(|_| Idl4JavaError::Unresolved {
                what: "an annotation".to_string(),
                at: String::new(),
            })?;
        let settings = self.settings_in(node.module, None, &node.facets);
        let package = self.package_of(node.module);
        let name = settings.naming.type_name(&node.name);
        Ok(if package.is_empty() {
            name
        } else {
            format!("{}.{name}", package.join("."))
        })
    }

    /// `@pkg.Name(member = value, …)`.
    fn annotation_instance(
        &self,
        applied: &zerodds_ast::AppliedAnnotation,
        def: Id<zerodds_ast::AnnotationDefNode>,
        class: &str,
    ) -> Result<String, Idl4JavaError> {
        let node = self
            .ast
            .annotation_def(def)
            .map_err(|_| Idl4JavaError::Unresolved {
                what: "an annotation".to_string(),
                at: applied.name.clone(),
            })?;
        let settings = self.settings_in(node.module, None, &node.facets);
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
                .or_else(|| {
                    node.members.iter().find_map(|id| {
                        self.ast
                            .annotation_member(*id)
                            .ok()
                            .filter(|m| m.is_value_member)
                    })
                })
                .or_else(|| {
                    // The short form names the one member there is.
                    match node.members.as_slice() {
                        [only] => self.ast.annotation_member(*only).ok(),
                        _ => None,
                    }
                })
                .ok_or_else(|| Idl4JavaError::Unresolved {
                    what: "the member an annotation argument names".to_string(),
                    at: applied.name.clone(),
                })?;
            let at = format!("@{}.{}", node.name, member.name);
            let jtype = self.annotation_element_type(&member.type_, &settings, &at)?;
            let value =
                self.ast
                    .eval_const(argument.value)
                    .map_err(|_| Idl4JavaError::Unevaluated {
                        what: "an annotation argument".to_string(),
                        at: at.clone(),
                    })?;
            pairs.push(format!(
                "{} = {}",
                super::names::escape(member.name.clone()),
                self.java_value(&value, &jtype, &at)?
            ));
        }
        Ok(format!("@{class}({})", pairs.join(", ")))
    }

    /// The Java type of an annotation element: Java admits primitives,
    /// `String` and enumerations there (JLS §9.6.1).
    fn annotation_element_type(
        &self,
        reference: &Ref<TypeNode>,
        settings: &Settings,
        at: &str,
    ) -> Result<super::JType, Idl4JavaError> {
        let jtype = self.java_type(reference, settings, at)?;
        match &jtype.shape {
            Shape::Boolean
            | Shape::Char
            | Shape::Integer { .. }
            | Shape::Float
            | Shape::Double
            | Shape::Enum { .. } => Ok(jtype),
            Shape::Str { .. } if jtype.text == "java.lang.String" => Ok(jtype),
            _ => Err(Idl4JavaError::NoJavaForm {
                construct: format!(
                    "an annotation member of type `{}`, which a Java annotation element cannot \
                     have (JLS §9.6.1)",
                    jtype.text
                ),
                at: at.to_string(),
            }),
        }
    }

    /// The two Java annotation types of a user-defined IDL annotation,
    /// §7.16.1.
    pub(super) fn annotation_type(
        &mut self,
        def: Id<zerodds_ast::AnnotationDefNode>,
    ) -> Result<(), Idl4JavaError> {
        if self.is_builtin(def) {
            return Ok(());
        }
        let node = self
            .ast
            .annotation_def(def)
            .map_err(|_| Idl4JavaError::Unresolved {
                what: "an annotation".to_string(),
                at: String::new(),
            })?;
        let module = node.module;
        let settings = self.settings_in(module, None, &node.facets);
        let name = settings.naming.type_name(&node.name);
        let origin = self.origin(AnyId::AnnotationDef(def, node.node_id));
        let mut w = Writer::new();
        w.line(&format!(
            "/** The IDL annotation `@{}` (§7.16.1). */",
            node.name
        ));
        w.line("@java.lang.annotation.Retention(java.lang.annotation.RetentionPolicy.RUNTIME)");
        w.open(&format!("public @interface {name}"));
        for member in &node.members {
            let m = self
                .ast
                .annotation_member(*member)
                .map_err(|_| Idl4JavaError::Unresolved {
                    what: "an annotation member".to_string(),
                    at: name.clone(),
                })?;
            let at = format!("@{}.{}", node.name, m.name);
            let jtype = self.annotation_element_type(&m.type_, &settings, &at)?;
            let default = match m.default {
                Some(expr) => {
                    let value =
                        self.ast
                            .eval_const(expr)
                            .map_err(|_| Idl4JavaError::Unevaluated {
                                what: "an annotation member's default".to_string(),
                                at: at.clone(),
                            })?;
                    format!(" default {}", self.java_value(&value, &jtype, &at)?)
                }
                None => String::new(),
            };
            w.line(&format!(
                "{} {}(){default};",
                jtype.text,
                super::names::escape_method(m.name.clone())
            ));
        }
        w.close();
        let body = w.finish();
        self.file(module, &name, &origin, &body)?;
        let group = format!("{name}Group");
        let mut w = Writer::new();
        w.line(&format!(
            "/** Several instances of `@{}` on one element (§7.16.2). */",
            node.name
        ));
        w.line("@java.lang.annotation.Retention(java.lang.annotation.RetentionPolicy.RUNTIME)");
        w.open(&format!("public @interface {group}"));
        w.line(&format!("{name}[] value();"));
        w.close();
        let body = w.finish();
        self.file(module, &group, &origin, &body)
    }

    // -- interfaces, §7.4 and §7.5 --------------------------------------------

    /// An interface declared in a module: its file.
    pub(super) fn top_level_interface(
        &mut self,
        id: Id<zerodds_ast::InterfaceNode>,
    ) -> Result<(), Idl4JavaError> {
        let node = self
            .ast
            .interface(id)
            .map_err(|_| Idl4JavaError::Unresolved {
                what: "an interface".to_string(),
                at: String::new(),
            })?;
        // §7.4.2: "An interface forward declaration has no mapping to the
        // Java language."
        if node.completeness == zerodds_ast::Completeness::Forward {
            return Ok(());
        }
        let origin = self.origin(AnyId::Interface(id, node.node_id));
        // A.1.2: `CORBA::TypeCode` maps to the class
        // `org.omg.CORBA.TypeCode` the clause defines, which the runtime
        // holds (`runtime/java/corba/org/omg/CORBA/TypeCode.java`),
        // not to the interface its IDL declares.
        if node.name == "TypeCode" && self.is_corba_module(node.module) {
            return Ok(());
        }
        match node.kind {
            InterfaceKind::Interface | InterfaceKind::Local | InterfaceKind::Abstract => {}
            // A port type is the grouping of basic ports the ports that
            // name it were replaced by (§7.12); it declares nothing of its
            // own in the intermediate IDL.
            InterfaceKind::PortType => return Ok(()),
            // §7.9, §7.10, §7.12: components, homes, port types and
            // connectors map through their intermediate IDL, which
            // `generate` rewrote them into before the walk.
            other => {
                return Err(Idl4JavaError::NoJavaForm {
                    construct: format!("a {} the intermediate IDL did not replace", other.as_str()),
                    at: origin,
                });
            }
        }
        let module = node.module;
        let settings = self.settings_in(module, None, &node.facets);
        let name = settings.naming.type_name(&node.name);
        let body = self.interface(id, &name)?;
        self.file(module, &name, &origin, &body)
    }

    /// The Java interface of an IDL interface, §7.4, with the declarations
    /// its body embeds, §7.5.
    fn interface(
        &self,
        id: Id<zerodds_ast::InterfaceNode>,
        name: &str,
    ) -> Result<String, Idl4JavaError> {
        let node = self
            .ast
            .interface(id)
            .map_err(|_| Idl4JavaError::Unresolved {
                what: "an interface".to_string(),
                at: name.to_string(),
            })?;
        let settings = self.settings_in(node.module, None, &node.facets);
        let mut bases = Vec::new();
        for base in &node.bases {
            let base = base.resolved().ok_or_else(|| Idl4JavaError::Unresolved {
                what: "a base interface".to_string(),
                at: name.to_string(),
            })?;
            bases.push(self.interface_name(base)?);
        }
        let mut methods = Vec::new();
        for attribute in self.ast.attributes_of(id) {
            methods.extend(self.attribute_methods(*attribute, &settings, name)?);
        }
        for operation in self.ast.operations_of(id) {
            methods.push(self.operation_method(*operation, &settings, name)?);
        }
        check_method_collisions(&methods, name)?;
        let mut w = Writer::new();
        for annotation in self.user_annotations(&node.facets)? {
            w.line(&annotation);
        }
        let extends = if bases.is_empty() {
            String::new()
        } else {
            format!(" extends {}", bases.join(", "))
        };
        w.open(&format!("public interface {name}{extends}"));
        for method in &methods {
            w.line("");
            for annotation in &method.annotations {
                w.line(annotation);
            }
            w.line(&format!("{};", method.signature));
        }
        self.embedded_declarations(&mut w, node.module, &node.name)?;
        w.close();
        Ok(w.finish())
    }

    /// The declarations an interface's or a value type's body embeds, as
    /// public nested declarations (§7.5).
    fn embedded_declarations(
        &self,
        w: &mut Writer,
        module: Id<zerodds_ast::ModuleNode>,
        scope: &str,
    ) -> Result<(), Idl4JavaError> {
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
                .map_err(|_| Idl4JavaError::Unresolved {
                    what: "an exception".to_string(),
                    at: scope.to_string(),
                })?;
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

    /// The accessor and, unless the attribute is `readonly`, the modifier
    /// of an attribute (§7.4).
    fn attribute_methods(
        &self,
        id: Id<zerodds_ast::AttributeNode>,
        settings: &Settings,
        owner: &str,
    ) -> Result<Vec<Method>, Idl4JavaError> {
        let node = self
            .ast
            .attribute(id)
            .map_err(|_| Idl4JavaError::Unresolved {
                what: "an attribute".to_string(),
                at: owner.to_string(),
            })?;
        let at = format!("{owner}.{}", node.name);
        let jtype = self.java_type(&node.type_, settings, &at)?;
        let annotations = self.user_annotations(&node.facets)?;
        let get_throws = self.throws(&node.get_raises, &at)?;
        let mut out = vec![Method {
            annotations: annotations.clone(),
            signature: format!(
                "{} {}(){get_throws}",
                jtype.text,
                settings.naming.getter(&node.name)
            ),
            idl: node.name.clone(),
        }];
        if !node.readonly {
            let set_throws = self.throws(&node.set_raises, &at)?;
            out.push(Method {
                annotations,
                signature: format!(
                    "void {}({} {}){set_throws}",
                    settings.naming.setter(&node.name),
                    jtype.text,
                    settings.naming.variable(&node.name)
                ),
                idl: node.name.clone(),
            });
        }
        Ok(out)
    }

    /// An operation as a Java method (§7.4): its parameters in IDL order,
    /// `out` and `inout` in `org.omg.type.Holder<E>` (§7.1.3), its
    /// exceptions in a `throws` clause.
    fn operation_method(
        &self,
        id: Id<zerodds_ast::OperationNode>,
        settings: &Settings,
        owner: &str,
    ) -> Result<Method, Idl4JavaError> {
        let node = self
            .ast
            .operation(id)
            .map_err(|_| Idl4JavaError::Unresolved {
                what: "an operation".to_string(),
                at: owner.to_string(),
            })?;
        let at = format!("{owner}.{}", node.name);
        let returns = match &node.return_type {
            Some(reference) => self.java_type(reference, settings, &at)?.text,
            None => "void".to_string(),
        };
        let mut params = Vec::new();
        for param in &node.params {
            let p = self
                .ast
                .param(*param)
                .map_err(|_| Idl4JavaError::Unresolved {
                    what: "a parameter".to_string(),
                    at: at.clone(),
                })?;
            let jtype = self.java_type(&p.type_, settings, &format!("{at}({})", p.name))?;
            let type_text = match p.direction {
                ParamDirection::In => jtype.text.clone(),
                ParamDirection::Out | ParamDirection::InOut => {
                    format!("org.omg.type.Holder<{}>", jtype.boxed)
                }
            };
            let mut annotations = self.user_annotations(&p.facets)?;
            annotations.push(String::new());
            params.push(format!(
                "{}{type_text} {}",
                annotations.join(" ").trim_start(),
                settings.naming.variable(&p.name)
            ));
        }
        let throws = self.throws(&node.raises, &at)?;
        Ok(Method {
            annotations: self.user_annotations(&node.facets)?,
            signature: format!(
                "{returns} {}({}){throws}",
                settings.naming.method(&node.name),
                params.join(", ")
            ),
            idl: node.name.clone(),
        })
    }

    /// ` throws A, B` for a `raises` clause.
    fn throws(
        &self,
        raises: &[Ref<zerodds_ast::ExceptionNode>],
        at: &str,
    ) -> Result<String, Idl4JavaError> {
        let mut names = Vec::new();
        for raise in raises {
            let exception = raise.resolved().ok_or_else(|| Idl4JavaError::Unresolved {
                what: "an exception of a raises clause".to_string(),
                at: at.to_string(),
            })?;
            names.push(self.exception_name(exception)?);
        }
        Ok(if names.is_empty() {
            String::new()
        } else {
            format!(" throws {}", names.join(", "))
        })
    }

    // -- value types, §7.6 and A.1.6 -------------------------------------------

    /// The two classes of a value type: `<Name>Abstract` with its state,
    /// operations and initializers, and `<Name>` extending it (§7.6).
    pub(super) fn valuetype(
        &self,
        id: Id<TypeNode>,
        name: &str,
    ) -> Result<Vec<(String, String)>, Idl4JavaError> {
        let node = self.ast.type_(id).map_err(|_| Idl4JavaError::Unresolved {
            what: "a value type".to_string(),
            at: name.to_string(),
        })?;
        let settings = self.settings_of_type(id);
        let abstract_name = abstract_class(name);
        // A boxed value is a value type whose one public member is `value`.
        let (bases, state, initializers, supported) = match &node.kind {
            TypeKind::Valuetype {
                bases,
                state,
                initializers,
                supported,
                ..
            } => (
                bases.clone(),
                state.clone(),
                initializers.clone(),
                supported.clone(),
            ),
            TypeKind::ValueBox { .. } => (Vec::new(), Vec::new(), Vec::new(), Vec::new()),
            _ => {
                return Err(Idl4JavaError::Unresolved {
                    what: "a value type".to_string(),
                    at: name.to_string(),
                });
            }
        };
        if bases.len() > 1 {
            return Err(Idl4JavaError::NoJavaForm {
                construct: "a value type with more than one base, which a Java class cannot \
                            extend (§7.6 maps a base to a superclass)"
                    .to_string(),
                at: name.to_string(),
            });
        }
        let base = match bases.first().and_then(Ref::resolved) {
            Some(base) => Some(self.qualified(self.through_aliases(base, name)?)?),
            None => None,
        };
        let mut interfaces = Vec::new();
        for interface in &supported {
            let interface = interface
                .resolved()
                .ok_or_else(|| Idl4JavaError::Unresolved {
                    what: "a supported interface".to_string(),
                    at: name.to_string(),
                })?;
            interfaces.push(interface);
        }
        // The fields: attributes (the clause's example maps one to a public
        // field), then the state members, private ones protected.
        let mut fields = Vec::new();
        for attribute in self.ast.attributes_of_value(id) {
            let a = self
                .ast
                .attribute(attribute)
                .map_err(|_| Idl4JavaError::Unresolved {
                    what: "an attribute".to_string(),
                    at: name.to_string(),
                })?;
            let jtype = self.java_type(&a.type_, &settings, &format!("{name}.{}", a.name))?;
            fields.push((
                "public",
                jtype.text.clone(),
                settings.naming.variable(&a.name),
                self.user_annotations(&a.facets)?,
            ));
        }
        if let TypeKind::ValueBox { boxed } = &node.kind {
            let jtype = self.java_type(boxed, &settings, name)?;
            fields.push(("public", jtype.text, "value".to_string(), Vec::new()));
        }
        for member in &state {
            let m = self
                .ast
                .member(*member)
                .map_err(|_| Idl4JavaError::Unresolved {
                    what: "a state member".to_string(),
                    at: name.to_string(),
                })?;
            let jtype = self.java_type(&m.type_, &settings, &format!("{name}.{}", m.name))?;
            let visibility = match m.facets.corba.as_ref().and_then(|c| c.state_visibility) {
                Some(StateVisibility::Private) => "protected",
                _ => "public",
            };
            fields.push((
                visibility,
                jtype.text,
                settings.naming.variable(&m.name),
                self.user_annotations(&m.facets)?,
            ));
        }
        // The operations, then the initializers as methods returning void
        // and taking the `in` parameters.
        let mut methods = Vec::new();
        for operation in self.ast.operations_of_value(id) {
            if initializers.contains(&operation) {
                continue;
            }
            methods.push(self.operation_method(operation, &settings, name)?);
        }
        // An initializer has no return type (rule (107)), so its method
        // returns void, as §7.6 asks.
        for initializer in &initializers {
            methods.push(self.operation_method(*initializer, &settings, name)?);
        }
        // The methods of the supported interfaces, which the non-abstract
        // class overrides too.
        let mut supported_methods = Vec::new();
        for interface in &interfaces {
            supported_methods.extend(self.all_interface_methods(*interface, name)?);
        }
        check_method_collisions(&methods, name)?;

        let mut implements = Vec::new();
        for interface in &interfaces {
            implements.push(self.interface_name(*interface)?);
        }
        if base.is_none() {
            implements.push("java.io.Serializable".to_string());
        }
        let mut w = Writer::new();
        for annotation in self.user_annotations(&node.facets)? {
            w.line(&annotation);
        }
        let extends = base
            .as_ref()
            .map_or_else(String::new, |base| format!(" extends {base}"));
        let implements_text = if implements.is_empty() {
            String::new()
        } else {
            format!(" implements {}", implements.join(", "))
        };
        w.open(&format!(
            "public abstract class {abstract_name}{extends}{implements_text}"
        ));
        w.line("private static final long serialVersionUID = 1L;");
        for (visibility, type_text, field, annotations) in &fields {
            w.line("");
            for annotation in annotations {
                w.line(annotation);
            }
            w.line(&format!("{visibility} {type_text} {field};"));
        }
        for method in &methods {
            w.line("");
            for annotation in &method.annotations {
                w.line(annotation);
            }
            w.line(&format!("public abstract {};", method.signature));
        }
        self.embedded_declarations(&mut w, node.module, &node.name)?;
        w.close();
        let abstract_source = w.finish();

        let mut w = Writer::new();
        w.open(&format!("public class {name} extends {abstract_name}"));
        w.line("private static final long serialVersionUID = 1L;");
        w.line("");
        w.open(&format!("public {name}()"));
        w.line("super();");
        w.close();
        for method in methods.iter().chain(supported_methods.iter()) {
            w.line("");
            w.line("@Override");
            w.open(&format!("public {}", method.signature));
            w.line(&format!(
                "throw new UnsupportedOperationException(\"{name}.{}\");",
                method.idl
            ));
            w.close();
        }
        w.close();
        let concrete_source = w.finish();
        Ok(vec![
            (abstract_name, abstract_source),
            (name.to_string(), concrete_source),
        ])
    }

    /// Every method of an interface and its bases, each once.
    fn all_interface_methods(
        &self,
        interface: Id<zerodds_ast::InterfaceNode>,
        owner: &str,
    ) -> Result<Vec<Method>, Idl4JavaError> {
        let mut out: Vec<Method> = Vec::new();
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
                .map_err(|_| Idl4JavaError::Unresolved {
                    what: "an interface".to_string(),
                    at: owner.to_string(),
                })?;
            let settings = self.settings_in(node.module, None, &node.facets);
            for attribute in self.ast.attributes_of(next) {
                for method in self.attribute_methods(*attribute, &settings, &node.name)? {
                    if !out.iter().any(|m| m.signature == method.signature) {
                        out.push(method);
                    }
                }
            }
            for operation in self.ast.operations_of(next) {
                let method = self.operation_method(*operation, &settings, &node.name)?;
                if !out.iter().any(|m| m.signature == method.signature) {
                    out.push(method);
                }
            }
            queue.extend(node.bases.iter().filter_map(Ref::resolved));
        }
        Ok(out)
    }
}

/// Two methods of one interface that Java cannot tell apart: the same
/// name and parameter list (§7.1.1.2).
fn check_method_collisions(methods: &[Method], owner: &str) -> Result<(), Idl4JavaError> {
    let key = |method: &Method| {
        let open = method.signature.find('(').unwrap_or(0);
        let name = method.signature[..open]
            .rsplit(' ')
            .next()
            .unwrap_or_default()
            .to_string();
        let close = method
            .signature
            .rfind(')')
            .unwrap_or(method.signature.len());
        let params: Vec<String> = method.signature[open + 1..close]
            .split(", ")
            .filter(|p| !p.is_empty())
            .map(|p| p.rsplit_once(' ').map_or(p, |(t, _)| t).to_string())
            .collect();
        (name, params)
    };
    for (index, method) in methods.iter().enumerate() {
        for other in &methods[..index] {
            if key(method) == key(other) {
                return Err(Idl4JavaError::Collision {
                    name: key(method).0,
                    first: format!("{owner}.{}", other.idl),
                    second: format!("{owner}.{}", method.idl),
                });
            }
        }
    }
    Ok(())
}
