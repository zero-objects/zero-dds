// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! AST walker that emits Java 17 source files.
//!
//! Block A: header layout (`package`, `import`, class modifiers).
//! Block B: primitive mapping (delegates to [`crate::type_map`]).
//! Block C: struct/enum/union/typedef/sequence/array/inheritance.
//! Block D: exception → `class X extends RuntimeException`.
//!
//! Java requires one `.java` file per top-level public class. The
//! emitter collects exactly one [`JavaFile`] structure per top-level
//! type during the AST walk.

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write;
// zerodds-lint: BTreeSet is used in the emitter for ImportSet + cycle detection.

use zerodds_idl::ast::{
    Annotation, AnnotationParams, CaseLabel, ConstExpr, ConstrTypeDecl, Declarator, Definition,
    EnumDef, ExceptDecl, IntegerType, InterfaceDcl, InterfaceDef, Literal, LiteralKind, Member,
    ScopedName, Specification, StructDcl, StructDef, SwitchTypeSpec, TypeDecl, TypeSpec,
    TypedefDecl, UnionDcl, UnionDef,
};

use zerodds_idl::semantics::annotations::PlacementKind;

use crate::JavaGenOptions;
use crate::annotations::{
    enum_value_override, has_nested, lower_or_empty, member_annotation_lines, type_annotation_lines,
};
use crate::bitset::{emit_bitmask_file, emit_bitset_file};
use crate::error::JavaGenError;
use crate::keywords::sanitize_identifier;
use crate::type_map::{
    floating_to_java, floating_to_java_boxed, integer_to_java, integer_to_java_boxed, is_unsigned,
    primitive_to_java, primitive_to_java_boxed,
};
use crate::verbatim::emit_verbatim_at;

/// A single generated Java source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaFile {
    /// Java package path with dot separators (e.g. `org.example.types`).
    pub package_path: String,
    /// Class name = file name without the `.java` suffix.
    pub class_name: String,
    /// Complete source file (including `package`, imports, class body).
    pub source: String,
}

impl JavaFile {
    /// Returns the relative path for the file (e.g. `org/example/Foo.java`).
    #[must_use]
    pub fn relative_path(&self) -> String {
        let dir = self.package_path.replace('.', "/");
        if dir.is_empty() {
            format!("{}.java", self.class_name)
        } else {
            format!("{dir}/{}.java", self.class_name)
        }
    }
}

/// Main entry: walks the IDL AST and emits a list of Java files.
pub(crate) fn emit_files(
    spec: &Specification,
    opts: &JavaGenOptions,
) -> Result<Vec<JavaFile>, JavaGenError> {
    detect_inheritance_cycles(spec)?;

    // Pre-pass: index every Struct-Name → its (transitive) base-chain
    // for the Multi-Inheritance Interface-Pattern (C5.4-b §3).
    let parent_of = collect_base_chain_index(spec);

    let mut files: Vec<JavaFile> = Vec::new();
    let pkg = sanitize_package(&opts.root_package);
    let ctx = EmitCtx { parent_of };
    walk_definitions(&spec.definitions, &pkg, opts, &mut files, &ctx)?;
    Ok(files)
}

/// Emitter context held read-only during the AST walk.
/// Contains the multi-inheritance index plus any future global
/// lookup tables (e.g. type-name → topic-eligibility).
#[derive(Debug, Default)]
pub(crate) struct EmitCtx {
    /// Mapping `struct name → direct base name` (short form, without
    /// module prefix). We use the last `.`-separated token
    /// (see [`scoped_to_short`]).
    pub parent_of: std::collections::HashMap<String, String>,
}

fn sanitize_package(p: &str) -> String {
    p.trim_matches('.').to_string()
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn walk_definitions(
    defs: &[Definition],
    pkg: &str,
    opts: &JavaGenOptions,
    files: &mut Vec<JavaFile>,
    ctx: &EmitCtx,
) -> Result<(), JavaGenError> {
    for d in defs {
        match d {
            Definition::Module(m) => {
                let name = sanitize_identifier(&m.name.text)?.to_lowercase();
                let sub_pkg = if pkg.is_empty() {
                    name
                } else {
                    format!("{pkg}.{name}")
                };
                walk_definitions(&m.definitions, &sub_pkg, opts, files, ctx)?;
            }
            Definition::Type(td) => emit_type_decl_top(td, pkg, opts, files, ctx)?,
            Definition::Const(c) => {
                let file = emit_const_holder(c, pkg, opts)?;
                files.push(file);
            }
            Definition::Except(e) => {
                let file = emit_exception_file(e, pkg, opts)?;
                files.push(file);
            }
            Definition::Interface(InterfaceDcl::Def(iface)) => {
                if is_service_interface(iface) {
                    emit_service_interface_files(iface, pkg, opts, files)?;
                } else {
                    // Spec idl4-java §7.4: IDL interface -> Java public interface.
                    files.push(emit_non_service_interface_file(iface, pkg, opts)?);
                    // §7.4: types/consts/exceptions nested in the interface
                    // body are their own scope. Emit them into a sub-package
                    // named after the interface (mirroring module -> package,
                    // lowercased) so a reference `Iface::Nested` resolves via
                    // `scoped_to_java` to `iface.Nested`. Previously silently
                    // dropped.
                    emit_interface_nested_types(iface, pkg, opts, files, ctx)?;
                }
            }
            Definition::Interface(InterfaceDcl::Forward(_)) => {
                // §7.4.2: forward decl has no Java mapping.
            }
            Definition::ValueDef(v) => {
                let value_files = emit_value_type_files(v, pkg, opts)?;
                files.extend(value_files);
            }
            Definition::ValueBox(_) | Definition::ValueForward(_) => {
                // ValueBox + ValueForward are no-ops in the foundation.
            }
            Definition::TypeId(_)
            | Definition::TypePrefix(_)
            | Definition::Import(_)
            | Definition::Component(_)
            | Definition::Home(_)
            | Definition::Event(_)
            | Definition::Porttype(_)
            | Definition::Connector(_)
            | Definition::TemplateModule(_)
            | Definition::TemplateModuleInst(_) => {
                return Err(JavaGenError::UnsupportedConstruct {
                    construct: "corba/ccm/template construct".into(),
                    context: None,
                });
            }
            Definition::Annotation(_) => {
                // §7.4.15: user-defined annotation defs are emitted at
                // the point of application on annotated members,
                // not as a standalone top-level Java construct.
            }
            Definition::VendorExtension(v) => {
                return Err(JavaGenError::UnsupportedConstruct {
                    construct: format!("vendor-extension:{}", v.production_name),
                    context: None,
                });
            }
        }
    }
    Ok(())
}

fn emit_type_decl_top(
    td: &TypeDecl,
    pkg: &str,
    opts: &JavaGenOptions,
    files: &mut Vec<JavaFile>,
    ctx: &EmitCtx,
) -> Result<(), JavaGenError> {
    match td {
        TypeDecl::Constr(c) => match c {
            ConstrTypeDecl::Struct(StructDcl::Def(s)) => {
                files.push(emit_struct_file(s, pkg, opts, ctx)?);
                // Multi-inheritance pattern: emit a companion interface
                // for every struct that is itself a base of another
                // struct — so a sub-sub-class can include the respective
                // transitive ancestor via `implements <Anc>Interface`.
                if ctx.parent_of.values().any(|p| p == &s.name.text) {
                    files.push(emit_struct_companion_interface(s, pkg, opts)?);
                }
                Ok(())
            }
            ConstrTypeDecl::Struct(StructDcl::Forward(_)) => {
                // Forward decls are implicit in Java (the class is
                // produced separately anyway) — no file needed.
                Ok(())
            }
            ConstrTypeDecl::Union(UnionDcl::Def(u)) => {
                files.extend(emit_union_files(u, pkg, opts)?);
                Ok(())
            }
            ConstrTypeDecl::Union(UnionDcl::Forward(_)) => Ok(()),
            ConstrTypeDecl::Enum(e) => {
                files.push(emit_enum_file(e, pkg, opts)?);
                Ok(())
            }
            ConstrTypeDecl::Bitset(b) => {
                files.push(emit_bitset_file(b, pkg, opts)?);
                Ok(())
            }
            ConstrTypeDecl::Bitmask(b) => {
                files.push(emit_bitmask_file(b, pkg, opts)?);
                Ok(())
            }
        },
        TypeDecl::Typedef(t) => {
            files.extend(emit_typedef_files(t, pkg, opts)?);
            Ok(())
        }
        // `native X;` — opaque, platform-specific type without an XCDR2
        // wire representation; not emitted in the DataType codegen.
        TypeDecl::Native(_) => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// Per-type emitter (one JavaFile each)
// ---------------------------------------------------------------------------

fn emit_struct_file(
    s: &StructDef,
    pkg: &str,
    opts: &JavaGenOptions,
    ctx: &EmitCtx,
) -> Result<JavaFile, JavaGenError> {
    let class = sanitize_identifier(&s.name.text)?;
    let mut imports = ImportSet::default();
    let ind = indent_unit(opts);

    // Pre-Walk: imports sammeln.
    for m in &s.members {
        collect_member_imports(m, &mut imports);
    }

    let mut body = String::new();

    // §7.2.2.4.8 — `@verbatim(placement=BEGIN_FILE)` right at the start
    // of the body (sits after package + imports in the compilation-unit wrap).
    emit_verbatim_at(&mut body, "", &s.annotations, PlacementKind::BeginFile)?;

    // §7.2.2.4.8 — `@verbatim(placement=BEFORE_DECLARATION)`.
    emit_verbatim_at(
        &mut body,
        "",
        &s.annotations,
        PlacementKind::BeforeDeclaration,
    )?;

    // Type-level Annotations (`@Nested`, `@Extensibility(...)`).
    for line in type_annotation_lines(&s.annotations) {
        writeln!(body, "{line}").map_err(fmt_err)?;
    }

    let extends = if let Some(base) = &s.base {
        let base_str = scoped_to_java(base);
        format!(" extends {base_str}")
    } else {
        String::new()
    };

    // Multi-inheritance pattern: all transitive ancestors *beyond* the
    // direct base are carried as `implements <X>Interface`. For a simple
    // hierarchy (single base without a grandparent) this list stays empty.
    let mut implements: Vec<String> = transitive_ancestors_beyond_base(&s.name.text, ctx)
        .into_iter()
        .map(|anc| format!("{anc}Interface"))
        .collect();

    // TopicType marker: every top-level struct without a `@nested`
    // marker **and without a base** implements `org.omg.dds.topic.TopicType<Self>`.
    //
    // Sub-structs (`struct Child : Base`) inherit the marker from the
    // parent via the regular `extends` chain — Java forbids
    // re-implementing it with its own generic param (`TopicType<Child>`
    // vs. `TopicType<Base>`). This is spec-conformant: in the DDS Java PSM,
    // `TopicType<T>` is a marker interface whose generic param sits only
    // at the root type of the inheritance chain. By the inheritance rule a
    // sub-struct is still `instanceof TopicType<Base>` and thus
    // registerable as a topic type.
    //
    // Findings anchor: TS-3 finding 4 (`internal/test-harness/plan.md`).
    let lowered_type = lower_or_empty(&s.annotations);
    if !has_nested(&lowered_type) && s.base.is_none() {
        implements.push(format!("org.omg.dds.topic.TopicType<{class}>"));
    }
    let implements_clause = if implements.is_empty() {
        String::new()
    } else {
        format!(" implements {}", implements.join(", "))
    };

    writeln!(body, "public class {class}{extends}{implements_clause} {{").map_err(fmt_err)?;

    // §7.2.2.4.8 — `@verbatim(placement=BEGIN_DECLARATION)` as the first
    // line inside the class body.
    emit_verbatim_at(
        &mut body,
        &ind,
        &s.annotations,
        PlacementKind::BeginDeclaration,
    )?;

    // Fields.
    for m in &s.members {
        emit_member_field(&mut body, m, &ind)?;
    }
    writeln!(body).map_err(fmt_err)?;

    // Default constructor.
    writeln!(body, "{ind}public {class}() {{}}").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // Bean-Style Getter / Setter.
    for m in &s.members {
        emit_member_accessors(&mut body, m, &ind)?;
    }

    // §7.2.2.4.8 — `@verbatim(placement=END_DECLARATION)` as the last
    // line before the closing `}`.
    emit_verbatim_at(
        &mut body,
        &ind,
        &s.annotations,
        PlacementKind::EndDeclaration,
    )?;

    writeln!(body, "}}").map_err(fmt_err)?;

    // §7.2.2.4.8 — `@verbatim(placement=AFTER_DECLARATION/END_FILE)`.
    emit_verbatim_at(
        &mut body,
        "",
        &s.annotations,
        PlacementKind::AfterDeclaration,
    )?;
    emit_verbatim_at(&mut body, "", &s.annotations, PlacementKind::EndFile)?;

    let source = wrap_compilation_unit(pkg, &imports, &body);
    Ok(JavaFile {
        package_path: pkg.to_string(),
        class_name: class,
        source,
    })
}

fn emit_enum_file(e: &EnumDef, pkg: &str, opts: &JavaGenOptions) -> Result<JavaFile, JavaGenError> {
    let class = sanitize_identifier(&e.name.text)?;
    let ind = indent_unit(opts);
    let mut body = String::new();

    emit_verbatim_at(&mut body, "", &e.annotations, PlacementKind::BeginFile)?;
    emit_verbatim_at(
        &mut body,
        "",
        &e.annotations,
        PlacementKind::BeforeDeclaration,
    )?;

    // Type-level annotations (`@Nested`, `@Extensibility(...)`).
    for line in type_annotation_lines(&e.annotations) {
        writeln!(body, "{line}").map_err(fmt_err)?;
    }

    writeln!(body, "public enum {class} {{").map_err(fmt_err)?;
    emit_verbatim_at(
        &mut body,
        &ind,
        &e.annotations,
        PlacementKind::BeginDeclaration,
    )?;

    let count = e.enumerators.len();
    let mut next_implicit: i64 = 0;
    for (idx, en) in e.enumerators.iter().enumerate() {
        let name = sanitize_identifier(&en.name.text)?;
        let sep = if idx + 1 == count { ';' } else { ',' };
        // Explicit `@value(N)` overrides the auto-assigned ordinal.
        // Spec idl4-java-1.0 §7.2 — custom values instead of auto ordinals.
        let value_lit = match enum_value_override(&en.annotations) {
            Some(raw) => match raw.parse::<i64>() {
                Ok(n) => {
                    next_implicit = n + 1;
                    n.to_string()
                }
                Err(_) => raw,
            },
            None => {
                let n = next_implicit;
                next_implicit += 1;
                n.to_string()
            }
        };
        writeln!(body, "{ind}{name}({value_lit}){sep}").map_err(fmt_err)?;
    }
    writeln!(body).map_err(fmt_err)?;
    writeln!(body, "{ind}private final int value;").map_err(fmt_err)?;
    writeln!(body, "{ind}{class}(int value) {{ this.value = value; }}").map_err(fmt_err)?;
    writeln!(body, "{ind}public int value() {{ return value; }}").map_err(fmt_err)?;
    emit_verbatim_at(
        &mut body,
        &ind,
        &e.annotations,
        PlacementKind::EndDeclaration,
    )?;
    writeln!(body, "}}").map_err(fmt_err)?;
    emit_verbatim_at(
        &mut body,
        "",
        &e.annotations,
        PlacementKind::AfterDeclaration,
    )?;
    emit_verbatim_at(&mut body, "", &e.annotations, PlacementKind::EndFile)?;

    let source = wrap_compilation_unit(pkg, &ImportSet::default(), &body);
    Ok(JavaFile {
        package_path: pkg.to_string(),
        class_name: class,
        source,
    })
}

/// Union → one sealed interface + one Java file per case record.
/// We return *one* file with the sealed interface + nested case records
/// (Java allows nested permits in one file). This keeps the file count
/// deterministic.
fn emit_union_files(
    u: &UnionDef,
    pkg: &str,
    opts: &JavaGenOptions,
) -> Result<Vec<JavaFile>, JavaGenError> {
    let class = sanitize_identifier(&u.name.text)?;
    let ind = indent_unit(opts);
    let imports = ImportSet::default();

    let _disc_ty = switch_type_to_java(&u.switch_type)?;

    // Permit list of the case records (unique per member name).
    let mut permits: Vec<String> = Vec::new();
    let mut case_records: Vec<(String, String, String)> = Vec::new(); // (record-name, field-ty, field-name)
    for c in &u.cases {
        let cpp_ty = type_for_declarator(&c.element.type_spec, &c.element.declarator)?;
        let field_name = sanitize_identifier(&c.element.declarator.name().text)?;
        // Record name: CapitalCase from the field name.
        let record_name = capitalize(&field_name);
        if !permits.iter().any(|p| p == &record_name) {
            permits.push(record_name.clone());
            case_records.push((record_name, cpp_ty, field_name));
        }
    }
    // Java requires qualified names in the `permits` clause for nested
    // records inside the sealed interface — `permits A, B, C` fails with
    // `cannot find symbol`; `permits Foo.A, Foo.B, Foo.C` is the correct
    // form.
    //
    // Findings anchor: TS-3 finding 5 (`internal/test-harness/plan.md`).
    let permits_clause = if permits.is_empty() {
        String::new()
    } else {
        let qualified: Vec<String> = permits.iter().map(|p| format!("{class}.{p}")).collect();
        format!(" permits {}", qualified.join(", "))
    };

    let mut body = String::new();
    emit_verbatim_at(&mut body, "", &u.annotations, PlacementKind::BeginFile)?;
    emit_verbatim_at(
        &mut body,
        "",
        &u.annotations,
        PlacementKind::BeforeDeclaration,
    )?;
    if opts.java8_compat {
        // Java-8 compat: `abstract class` instead of `sealed interface`
        // (Java 17), without `permits`. Pseudo-sealing via a private
        // constructor — only the nested `static final` subclasses can extend.
        writeln!(body, "public abstract class {class} {{").map_err(fmt_err)?;
    } else {
        writeln!(body, "public sealed interface {class}{permits_clause} {{").map_err(fmt_err)?;
    }
    emit_verbatim_at(
        &mut body,
        &ind,
        &u.annotations,
        PlacementKind::BeginDeclaration,
    )?;
    if opts.java8_compat {
        writeln!(body, "{ind}private {class}() {{}}").map_err(fmt_err)?;
        writeln!(body).map_err(fmt_err)?;
    }

    // Default marker for the default branch (a comment; branch labels
    // are emitted as a comment, not as a Java construct).
    let mut has_default = false;
    for c in &u.cases {
        for label in &c.labels {
            match label {
                CaseLabel::Default => {
                    has_default = true;
                    writeln!(
                        body,
                        "{ind}// case default -> {}",
                        c.element.declarator.name().text
                    )
                    .map_err(fmt_err)?;
                }
                CaseLabel::Value(expr) => {
                    let val = const_expr_to_java(expr);
                    writeln!(
                        body,
                        "{ind}// case {val} -> {}",
                        c.element.declarator.name().text
                    )
                    .map_err(fmt_err)?;
                }
            }
        }
    }
    if !has_default {
        writeln!(body, "{ind}// no explicit 'default:' branch").map_err(fmt_err)?;
    }
    writeln!(body).map_err(fmt_err)?;

    // Nested case-Typen.
    for (record_name, field_ty, field_name) in &case_records {
        if opts.java8_compat {
            // Java-8 equivalent of a case record: a `static final` subclass
            // with a final field + constructor + same-named accessor.
            writeln!(
                body,
                "{ind}public static final class {record_name} extends {class} {{",
            )
            .map_err(fmt_err)?;
            writeln!(body, "{ind}{ind}private final {field_ty} {field_name};").map_err(fmt_err)?;
            writeln!(
                body,
                "{ind}{ind}public {record_name}({field_ty} {field_name}) {{ this.{field_name} = {field_name}; }}",
            )
            .map_err(fmt_err)?;
            writeln!(
                body,
                "{ind}{ind}public {field_ty} {field_name}() {{ return {field_name}; }}",
            )
            .map_err(fmt_err)?;
            writeln!(body, "{ind}}}").map_err(fmt_err)?;
        } else {
            writeln!(
                body,
                "{ind}record {record_name}({field_ty} {field_name}) implements {class} {{}}",
            )
            .map_err(fmt_err)?;
        }
    }
    emit_verbatim_at(
        &mut body,
        &ind,
        &u.annotations,
        PlacementKind::EndDeclaration,
    )?;
    writeln!(body, "}}").map_err(fmt_err)?;
    emit_verbatim_at(
        &mut body,
        "",
        &u.annotations,
        PlacementKind::AfterDeclaration,
    )?;
    emit_verbatim_at(&mut body, "", &u.annotations, PlacementKind::EndFile)?;

    let source = wrap_compilation_unit(pkg, &imports, &body);
    Ok(vec![JavaFile {
        package_path: pkg.to_string(),
        class_name: class,
        source,
    }])
}

fn emit_typedef_files(
    t: &TypedefDecl,
    pkg: &str,
    _opts: &JavaGenOptions,
) -> Result<Vec<JavaFile>, JavaGenError> {
    // Java has no `using`/`typedef` — we emit a wrapper class per alias
    // (1 wrapper field, named `value`).
    let mut out = Vec::new();
    for decl in &t.declarators {
        let alias = sanitize_identifier(&decl.name().text)?;
        let target = type_for_declarator(&t.type_spec, decl)?;
        let imports = ImportSet::default();

        let mut body = String::new();
        writeln!(body, "public final class {alias} {{").map_err(fmt_err)?;
        writeln!(body, "    private {target} value;").map_err(fmt_err)?;
        writeln!(body).map_err(fmt_err)?;
        writeln!(body, "    public {alias}() {{}}").map_err(fmt_err)?;
        writeln!(
            body,
            "    public {alias}({target} value) {{ this.value = value; }}",
        )
        .map_err(fmt_err)?;
        writeln!(body).map_err(fmt_err)?;
        writeln!(body, "    public {target} value() {{ return value; }}").map_err(fmt_err)?;
        writeln!(
            body,
            "    public void value({target} value) {{ this.value = value; }}",
        )
        .map_err(fmt_err)?;
        writeln!(body, "}}").map_err(fmt_err)?;

        let source = wrap_compilation_unit(pkg, &imports, &body);
        out.push(JavaFile {
            package_path: pkg.to_string(),
            class_name: alias,
            source,
        });
    }
    Ok(out)
}

fn emit_exception_file(
    e: &ExceptDecl,
    pkg: &str,
    opts: &JavaGenOptions,
) -> Result<JavaFile, JavaGenError> {
    let class = sanitize_identifier(&e.name.text)?;
    let ind = indent_unit(opts);
    let mut imports = ImportSet::default();
    for m in &e.members {
        collect_member_imports(m, &mut imports);
    }

    let mut body = String::new();
    writeln!(body, "public class {class} extends RuntimeException {{").map_err(fmt_err)?;
    for m in &e.members {
        emit_member_field(&mut body, m, &ind)?;
    }
    writeln!(body).map_err(fmt_err)?;
    writeln!(body, "{ind}public {class}() {{ super(); }}").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public {class}(String message) {{ super(message); }}",
    )
    .map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;
    for m in &e.members {
        emit_member_accessors(&mut body, m, &ind)?;
    }
    writeln!(body, "}}").map_err(fmt_err)?;

    let source = wrap_compilation_unit(pkg, &imports, &body);
    Ok(JavaFile {
        package_path: pkg.to_string(),
        class_name: class,
        source,
    })
}

fn emit_const_holder(
    c: &zerodds_idl::ast::ConstDecl,
    pkg: &str,
    _opts: &JavaGenOptions,
) -> Result<JavaFile, JavaGenError> {
    // IDL `const` → public static final field in a holder class
    // namens `<NAME>Constant`.
    let name = sanitize_identifier(&c.name.text)?;
    let class = format!("{name}Constant");
    let java_ty = const_type_to_java(&c.type_)?;
    let val = coerce_const_value(&c.type_, const_expr_to_java(&c.value));
    let mut body = String::new();
    writeln!(body, "public final class {class} {{").map_err(fmt_err)?;
    writeln!(body, "    public static final {java_ty} {name} = {val};").map_err(fmt_err)?;
    writeln!(body, "    private {class}() {{}}").map_err(fmt_err)?;
    writeln!(body, "}}").map_err(fmt_err)?;
    let source = wrap_compilation_unit(pkg, &ImportSet::default(), &body);
    Ok(JavaFile {
        package_path: pkg.to_string(),
        class_name: class,
        source,
    })
}

// ---------------------------------------------------------------------------
// Member-Helpers
// ---------------------------------------------------------------------------

fn emit_member_field(out: &mut String, m: &Member, ind: &str) -> Result<(), JavaGenError> {
    let optional = has_optional_annotation(&m.annotations);
    let ann_lines = member_annotation_lines(&m.annotations);
    for decl in &m.declarators {
        let java_ty = type_for_declarator(&m.type_spec, decl)?;
        let name = sanitize_identifier(&decl.name().text)?;
        let final_ty = if optional {
            format!("java.util.Optional<{}>", boxed_for_optional(&m.type_spec))
        } else {
            java_ty
        };
        for ann in &ann_lines {
            writeln!(out, "{ind}{ann}").map_err(fmt_err)?;
        }
        // Doc comment for the unsigned workaround.
        if let TypeSpec::Primitive(zerodds_idl::ast::PrimitiveType::Integer(i)) = &m.type_spec {
            if is_unsigned(*i) {
                writeln!(
                    out,
                    "{ind}/** unsigned IDL value (Java unsigned-workaround) */"
                )
                .map_err(fmt_err)?;
            }
        }
        writeln!(out, "{ind}private {final_ty} {name};").map_err(fmt_err)?;
    }
    Ok(())
}

fn emit_member_accessors(out: &mut String, m: &Member, ind: &str) -> Result<(), JavaGenError> {
    let optional = has_optional_annotation(&m.annotations);
    for decl in &m.declarators {
        let java_ty = type_for_declarator(&m.type_spec, decl)?;
        let name = sanitize_identifier(&decl.name().text)?;
        let cap = capitalize(&name);
        let final_ty = if optional {
            format!("java.util.Optional<{}>", boxed_for_optional(&m.type_spec))
        } else {
            java_ty.clone()
        };
        writeln!(
            out,
            "{ind}public {final_ty} get{cap}() {{ return {name}; }}"
        )
        .map_err(fmt_err)?;
        writeln!(
            out,
            "{ind}public void set{cap}({final_ty} {name}) {{ this.{name} = {name}; }}",
        )
        .map_err(fmt_err)?;
    }
    Ok(())
}

fn boxed_for_optional(ts: &TypeSpec) -> String {
    match ts {
        TypeSpec::Primitive(p) => primitive_to_java_boxed(*p).to_string(),
        TypeSpec::Scoped(s) => scoped_to_java(s),
        TypeSpec::String(_) => "String".into(),
        // Aggregates (sequence, map, …) reuse the canonical declared-type
        // mapping so the `Optional<…>` slot is fully generic. Previously
        // `map<…>` and nested aggregates fell through to a raw `Object`, which
        // both lost the static type and produced an `Optional<Object>` field
        // the codec then could not assign into.
        TypeSpec::Sequence(_) | TypeSpec::Map(_) => {
            typespec_to_java(ts).unwrap_or_else(|_| "Object".into())
        }
        _ => "Object".into(),
    }
}

// ---------------------------------------------------------------------------
// TypeSpec / Declarator
// ---------------------------------------------------------------------------

/// Returns the Java type expression for a member (TypeSpec + Declarator).
pub(crate) fn type_for_declarator(
    ts: &TypeSpec,
    decl: &Declarator,
) -> Result<String, JavaGenError> {
    let base = typespec_to_java(ts)?;
    match decl {
        Declarator::Simple(_) => Ok(base),
        Declarator::Array(arr) => {
            let mut suffix = String::new();
            for _ in &arr.sizes {
                suffix.push_str("[]");
            }
            Ok(format!("{base}{suffix}"))
        }
    }
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
pub(crate) fn typespec_to_java(ts: &TypeSpec) -> Result<String, JavaGenError> {
    match ts {
        TypeSpec::Primitive(zerodds_idl::ast::PrimitiveType::Floating(
            zerodds_idl::ast::FloatingType::LongDouble,
        )) => Err(crate::typesupport::long_double_unsupported()),
        TypeSpec::Primitive(p) => Ok(primitive_to_java(*p).to_string()),
        TypeSpec::Scoped(s) => Ok(scoped_to_java(s)),
        TypeSpec::Sequence(s) => {
            let inner = match &*s.elem {
                TypeSpec::Primitive(p) => primitive_to_java_boxed(*p).to_string(),
                TypeSpec::Scoped(sn) => scoped_to_java(sn),
                TypeSpec::String(_) => "String".into(),
                other => typespec_to_java(other)?,
            };
            Ok(format!("java.util.List<{inner}>"))
        }
        TypeSpec::String(_) => Ok("String".into()),
        TypeSpec::Map(m) => {
            let k = match &*m.key {
                TypeSpec::Primitive(p) => primitive_to_java_boxed(*p).to_string(),
                TypeSpec::Scoped(sn) => scoped_to_java(sn),
                TypeSpec::String(_) => "String".into(),
                other => typespec_to_java(other)?,
            };
            let v = match &*m.value {
                TypeSpec::Primitive(p) => primitive_to_java_boxed(*p).to_string(),
                TypeSpec::Scoped(sn) => scoped_to_java(sn),
                TypeSpec::String(_) => "String".into(),
                other => typespec_to_java(other)?,
            };
            Ok(format!("java.util.Map<{k}, {v}>"))
        }
        TypeSpec::Fixed(_) => {
            // Spec idl4-java §7.2.4.2.4: fixed<digits,scale> ->
            // `java.math.BigDecimal` (Range-Check via
            // `java.lang.ArithmeticException` at runtime, scale via
            // `setScale(scale)` in the codegen output).
            Ok("java.math.BigDecimal".into())
        }
        TypeSpec::Any => {
            // Spec idl4-java §7.3: any -> `org.omg.type.Any`. ZeroDDS
            // mapping choice: `java.lang.Object` (reflection-based, the
            // spec explicitly says "implementation is middleware
            // specific"). An org.omg.type.Any wrapper variant is
            // possible, but Object is enough for the Java-Type-Repr §8 path.
            Ok("Object".into())
        }
    }
}

pub(crate) fn switch_type_to_java(s: &SwitchTypeSpec) -> Result<String, JavaGenError> {
    Ok(match s {
        SwitchTypeSpec::Integer(i) => integer_to_java(*i).to_string(),
        SwitchTypeSpec::Char => "char".into(),
        SwitchTypeSpec::Boolean => "boolean".into(),
        SwitchTypeSpec::Octet => "byte".into(),
        SwitchTypeSpec::Scoped(s) => scoped_to_java(s),
    })
}

/// Coerces a Java constant initializer so it assigns to its declared type
/// without a narrowing/precision compile error. `octet`/`int8` map to Java
/// `byte`, but an IDL octet value `0..255` overflows the signed `byte`, so it
/// is cast (`(byte) (255)` == -1, matching the CDR octet). A Java floating
/// literal is `double` by default and does not implicitly narrow to `float`,
/// so `float` constants are cast too. Every other const type assigns directly.
fn coerce_const_value(t: &zerodds_idl::ast::ConstType, val: String) -> String {
    use zerodds_idl::ast::{ConstType, FloatingType, IntegerType};
    match t {
        ConstType::Octet | ConstType::Integer(IntegerType::Int8) => format!("(byte) ({val})"),
        ConstType::Floating(FloatingType::Float) => format!("(float) ({val})"),
        _ => val,
    }
}

fn const_type_to_java(t: &zerodds_idl::ast::ConstType) -> Result<String, JavaGenError> {
    Ok(match t {
        zerodds_idl::ast::ConstType::Integer(i) => integer_to_java(*i).to_string(),
        zerodds_idl::ast::ConstType::Floating(zerodds_idl::ast::FloatingType::LongDouble) => {
            return Err(crate::typesupport::long_double_unsupported());
        }
        zerodds_idl::ast::ConstType::Floating(f) => floating_to_java(*f).to_string(),
        zerodds_idl::ast::ConstType::Boolean => "boolean".into(),
        zerodds_idl::ast::ConstType::Char => "char".into(),
        zerodds_idl::ast::ConstType::WideChar => "char".into(),
        zerodds_idl::ast::ConstType::Octet => "byte".into(),
        zerodds_idl::ast::ConstType::String { .. } => "String".into(),
        zerodds_idl::ast::ConstType::Scoped(s) => scoped_to_java(s),
        zerodds_idl::ast::ConstType::Fixed => "java.math.BigDecimal".into(),
    })
}

/// Maps an IDL scoped name (`Alpha::T`) to a fully-qualified Java name
/// (`alpha.T`). Module segments become Java packages, which are emitted
/// lowercase (mirroring [`walk_definitions`], where each module is
/// `to_lowercase`d into the package path); only the trailing type name
/// keeps its original case. The FQN is used inline — no `import` needed
/// (see [`wrap_compilation_unit`]).
fn scoped_to_java(s: &ScopedName) -> String {
    let n = s.parts.len();
    let parts: Vec<String> = s
        .parts
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if i + 1 < n {
                p.text.to_lowercase()
            } else {
                p.text.clone()
            }
        })
        .collect();
    parts.join(".")
}

// ---------------------------------------------------------------------------
// Imports collection
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone)]
pub(crate) struct ImportSet {
    #[allow(dead_code)]
    imports: BTreeSet<&'static str>,
}

impl ImportSet {
    #[allow(dead_code)]
    fn add(&mut self, fqn: &'static str) {
        self.imports.insert(fqn);
    }
}

/// Hook for C5.4-b: the per-member import collection can be extended
/// here. C5.4-a uses FQN throughout, hence a no-op.
#[allow(clippy::needless_pass_by_ref_mut)]
fn collect_member_imports(_m: &Member, _inc: &mut ImportSet) {
    // FQN strategy: java.util.List/Optional/Map are referenced inline as
    // `java.util.<X>`, so no import entries are needed.
}

// ---------------------------------------------------------------------------
// Compilation-Unit-Wrapping
// ---------------------------------------------------------------------------

pub(crate) fn wrap_compilation_unit(pkg: &str, _imports: &ImportSet, body: &str) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "// Generated by zerodds idl-java. Do not edit.");
    if !pkg.is_empty() {
        let _ = writeln!(out, "package {pkg};");
        let _ = writeln!(out);
    }
    // Imports are currently replaced by FQN — no import statements
    // required. This keeps the diff stable and avoids conflicts with
    // type names like `List`/`Map` if the IDL named a type that way.
    out.push_str(body);
    out
}

// ---------------------------------------------------------------------------
// ConstExpr → Java-Literal
// ---------------------------------------------------------------------------

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
pub(crate) fn const_expr_to_java(e: &ConstExpr) -> String {
    match e {
        ConstExpr::Literal(l) => literal_to_java(l),
        ConstExpr::Scoped(s) => scoped_to_java(s),
        ConstExpr::Unary { op, operand, .. } => {
            let prefix = match op {
                zerodds_idl::ast::UnaryOp::Plus => "+",
                zerodds_idl::ast::UnaryOp::Minus => "-",
                zerodds_idl::ast::UnaryOp::BitNot => "~",
            };
            format!("{prefix}{}", const_expr_to_java(operand))
        }
        ConstExpr::Binary { op, lhs, rhs, .. } => {
            let opstr = match op {
                zerodds_idl::ast::BinaryOp::Or => "|",
                zerodds_idl::ast::BinaryOp::Xor => "^",
                zerodds_idl::ast::BinaryOp::And => "&",
                zerodds_idl::ast::BinaryOp::Shl => "<<",
                zerodds_idl::ast::BinaryOp::Shr => ">>",
                zerodds_idl::ast::BinaryOp::Add => "+",
                zerodds_idl::ast::BinaryOp::Sub => "-",
                zerodds_idl::ast::BinaryOp::Mul => "*",
                zerodds_idl::ast::BinaryOp::Div => "/",
                zerodds_idl::ast::BinaryOp::Mod => "%",
            };
            format!(
                "({} {opstr} {})",
                const_expr_to_java(lhs),
                const_expr_to_java(rhs)
            )
        }
    }
}

fn literal_to_java(l: &Literal) -> String {
    match l.kind {
        // IDL boolean literals are `TRUE`/`FALSE`; Java requires `true`/`false`.
        LiteralKind::Boolean => {
            if l.raw.eq_ignore_ascii_case("true") {
                "true".to_string()
            } else if l.raw.eq_ignore_ascii_case("false") {
                "false".to_string()
            } else {
                l.raw.to_ascii_lowercase()
            }
        }
        // IDL wide char/string literals carry an `L` prefix (`L'x'`, `L"…"`)
        // that is not valid Java; strip it — Java `char`/`String` hold UTF-16.
        LiteralKind::WideChar | LiteralKind::WideString => {
            l.raw.strip_prefix('L').unwrap_or(&l.raw).to_string()
        }
        LiteralKind::Integer
        | LiteralKind::Floating
        | LiteralKind::Char
        | LiteralKind::String
        | LiteralKind::Fixed => l.raw.clone(),
    }
}

// ---------------------------------------------------------------------------
// Annotation-Helpers
// ---------------------------------------------------------------------------

fn has_optional_annotation(anns: &[Annotation]) -> bool {
    has_named_annotation(anns, "optional")
}

fn has_named_annotation(anns: &[Annotation], name: &str) -> bool {
    anns.iter().any(|a| {
        a.name.parts.last().is_some_and(|p| p.text == name)
            && matches!(a.params, AnnotationParams::None | AnnotationParams::Empty)
    })
}

// ---------------------------------------------------------------------------
// Inheritance-Cycle-Detection
// ---------------------------------------------------------------------------

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn collect_inheritance_edges(
    defs: &[Definition],
    parents: &mut HashMap<String, String>,
    prefix: &str,
) {
    for d in defs {
        match d {
            Definition::Module(m) => {
                let new_prefix = if prefix.is_empty() {
                    m.name.text.clone()
                } else {
                    format!("{prefix}.{}", m.name.text)
                };
                collect_inheritance_edges(&m.definitions, parents, &new_prefix);
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                let key = if prefix.is_empty() {
                    s.name.text.clone()
                } else {
                    format!("{prefix}.{}", s.name.text)
                };
                if let Some(b) = &s.base {
                    let base_str = b
                        .parts
                        .iter()
                        .map(|p| p.text.clone())
                        .collect::<Vec<_>>()
                        .join(".");
                    parents.insert(key, base_str);
                }
            }
            _ => {}
        }
    }
}

fn detect_inheritance_cycles(spec: &Specification) -> Result<(), JavaGenError> {
    let mut parents: HashMap<String, String> = HashMap::new();
    collect_inheritance_edges(&spec.definitions, &mut parents, "");

    for start in parents.keys() {
        let mut current = start.clone();
        let mut visited: BTreeSet<String> = BTreeSet::new();
        visited.insert(current.clone());
        while let Some(p) = parents.get(&current) {
            let resolved = parents
                .keys()
                .find(|k| *k == p || k.ends_with(&format!(".{p}")))
                .cloned()
                .unwrap_or_else(|| p.clone());
            if visited.contains(&resolved) {
                return Err(JavaGenError::InheritanceCycle {
                    type_name: short_name(&resolved),
                });
            }
            visited.insert(resolved.clone());
            if resolved == current {
                return Err(JavaGenError::InheritanceCycle {
                    type_name: short_name(&resolved),
                });
            }
            current = resolved;
            if !parents.contains_key(&current) {
                break;
            }
        }
    }
    Ok(())
}

fn short_name(s: &str) -> String {
    s.rsplit('.').next().unwrap_or(s).to_string()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(crate) fn indent_unit(opts: &JavaGenOptions) -> String {
    " ".repeat(opts.indent_width)
}

pub(crate) fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub(crate) fn fmt_err(_: core::fmt::Error) -> JavaGenError {
    JavaGenError::Internal("string formatting failed".into())
}

/// Wrap helper for bitset/bitmask: wraps header + body into a
/// compilation unit. Identical to [`wrap_compilation_unit`] but without
/// the import argument.
pub(crate) fn wrap_compilation_unit_default(pkg: &str, body: &str) -> String {
    wrap_compilation_unit(pkg, &ImportSet::default(), body)
}

// ---------------------------------------------------------------------------
// Multi-Inheritance — Interface-Pattern (C5.4-b §3)
// ---------------------------------------------------------------------------

/// Collects for each struct definition the (short) name of its direct
/// predecessor. IDL4 `struct` inheritance is single — the codegen
/// produces the `extends + implements XInterface, YInterface` form from
/// the transitive chain.
fn collect_base_chain_index(spec: &Specification) -> std::collections::HashMap<String, String> {
    let mut out: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    /// zerodds-lint: recursion-depth 32
    ///
    /// Module hierarchy in IDL files: typical depth 2-4
    /// (`org::omg::dds::core`), 32 covers edge cases.
    fn visit(defs: &[Definition], out: &mut std::collections::HashMap<String, String>) {
        for d in defs {
            match d {
                Definition::Module(m) => visit(&m.definitions, out),
                Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                    if let Some(b) = &s.base {
                        out.insert(s.name.text.clone(), scoped_to_short(b));
                    }
                }
                _ => {}
            }
        }
    }
    visit(&spec.definitions, &mut out);
    out
}

/// Returns the transitive ancestor names *beyond* the direct base
/// (all grandparents). For `A : B`, `B : C`, `C : D`,
/// `transitive_ancestors_beyond_base("A", ctx) → ["C", "D"]`.
fn transitive_ancestors_beyond_base(name: &str, ctx: &EmitCtx) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let direct = match ctx.parent_of.get(name) {
        Some(p) => p.clone(),
        None => return out,
    };
    let mut current = direct;
    let mut guard = 0usize;
    while let Some(p) = ctx.parent_of.get(&current) {
        if guard > 64 {
            break;
        }
        guard += 1;
        out.push(p.clone());
        current = p.clone();
    }
    out
}

fn scoped_to_short(s: &ScopedName) -> String {
    s.parts.last().map(|p| p.text.clone()).unwrap_or_default()
}

/// Emits a companion interface `<Name>Interface.java` with default
/// methods that mirror the bean-pattern read-only view of the members.
/// This lets sub-sub-classes include the ancestor via
/// `implements <Name>Interface` without violating the JVM class-file
/// constraints (single `extends`).
fn emit_struct_companion_interface(
    s: &StructDef,
    pkg: &str,
    opts: &JavaGenOptions,
) -> Result<JavaFile, JavaGenError> {
    let class = sanitize_identifier(&s.name.text)?;
    let interface_name = format!("{class}Interface");
    let ind = indent_unit(opts);
    let mut body = String::new();
    writeln!(
        body,
        "/** Companion interface for {class}; lets sub-sub-classes \
         participate in the {class} contract via `implements`. */",
    )
    .map_err(fmt_err)?;
    writeln!(body, "public interface {interface_name} {{").map_err(fmt_err)?;
    // Default methods — we render the getter signatures with a
    // `default` implementation that delegates to the concrete class via
    // a cast. Since we know the class at compile time (every `implements`
    // site is a subclass of `class`), we produce only abstract methods
    // here — the concrete class provides the implementation as a usual
    // bean getter.
    for m in &s.members {
        let opt = has_optional_annotation(&m.annotations);
        for decl in &m.declarators {
            let java_ty = type_for_declarator(&m.type_spec, decl)?;
            let name = sanitize_identifier(&decl.name().text)?;
            let cap = capitalize(&name);
            let final_ty = if opt {
                format!("java.util.Optional<{}>", boxed_for_optional(&m.type_spec))
            } else {
                java_ty
            };
            writeln!(body, "{ind}{final_ty} get{cap}();").map_err(fmt_err)?;
        }
    }
    writeln!(body, "}}").map_err(fmt_err)?;
    let source = wrap_compilation_unit_default(pkg, &body);
    Ok(JavaFile {
        package_path: pkg.to_string(),
        class_name: interface_name,
        source,
    })
}

// Marker so unused imports produce no warnings (e.g.
// `IntegerType`/`integer_to_java_boxed`, in case the detailed use is
// removed later).
#[allow(dead_code)]
fn _unused_marker(_i: IntegerType) {
    let _ = integer_to_java_boxed;
    let _ = floating_to_java_boxed;
}

// ---------------------------------------------------------------------------
// RPC-Service-Bridge (DDS-RPC §7.11.2)
// ---------------------------------------------------------------------------

/// Spec idl4-java §7.6: valuetype -> 2 Java classes
/// (`<Name>Abstract` abstract + `<Name>` non-abstract).
/// public state -> public abstract bean accessors; private state ->
/// protected abstract accessors; factory -> void method.
fn emit_value_type_files(
    v: &zerodds_idl::ast::ValueDef,
    pkg: &str,
    opts: &JavaGenOptions,
) -> Result<Vec<JavaFile>, JavaGenError> {
    use zerodds_idl::ast::{Export, StateVisibility, ValueElement};

    let class = sanitize_identifier(&v.name.text)?;
    let abstract_name = format!("{class}Abstract");
    let ind = indent_unit(opts);
    let imports = ImportSet::default();

    // Abstract base class.
    let mut body = String::new();
    let extends = match &v.inheritance {
        Some(inh) if !inh.bases.is_empty() => {
            // Java allows only one superclass — we take the first base.
            let base = scoped_to_java(&inh.bases[0]);
            format!(" extends {base}Abstract")
        }
        _ => String::new(),
    };
    let supports = match &v.inheritance {
        Some(inh) if !inh.supports.is_empty() => {
            let s: Vec<String> = inh.supports.iter().map(scoped_to_java).collect();
            format!(" implements {}", s.join(", "))
        }
        _ => String::new(),
    };

    writeln!(
        body,
        "public abstract class {abstract_name}{extends}{supports} {{"
    )
    .map_err(fmt_err)?;

    for el in &v.elements {
        match el {
            ValueElement::State(s) => {
                let ty = typespec_to_java(&s.type_spec)?;
                let visibility = match s.visibility {
                    StateVisibility::Public => "public",
                    StateVisibility::Private => "protected",
                };
                for d in &s.declarators {
                    let n = sanitize_identifier(&d.name().text)?;
                    writeln!(body, "{ind}{visibility} abstract {ty} get_{n}();")
                        .map_err(fmt_err)?;
                    writeln!(body, "{ind}{visibility} abstract void set_{n}({ty} value);")
                        .map_err(fmt_err)?;
                }
            }
            ValueElement::Init(i) => {
                let params: Vec<String> = i
                    .params
                    .iter()
                    .map(|p| -> Result<String, JavaGenError> {
                        let ty = typespec_to_java(&p.type_spec)?;
                        let pname = sanitize_identifier(&p.name.text)?;
                        Ok(format!("{ty} {pname}"))
                    })
                    .collect::<Result<_, _>>()?;
                writeln!(
                    body,
                    "{ind}public abstract void {}({});",
                    sanitize_identifier(&i.name.text)?,
                    params.join(", ")
                )
                .map_err(fmt_err)?;
            }
            ValueElement::Export(Export::Op(op)) => {
                let ret = match &op.return_type {
                    None => "void".to_string(),
                    Some(t) => typespec_to_java(t)?,
                };
                let params: Vec<String> = op
                    .params
                    .iter()
                    .map(|p| -> Result<String, JavaGenError> {
                        let ty = typespec_to_java(&p.type_spec)?;
                        let pname = sanitize_identifier(&p.name.text)?;
                        Ok(format!("{ty} {pname}"))
                    })
                    .collect::<Result<_, _>>()?;
                writeln!(
                    body,
                    "{ind}public abstract {ret} {}({});",
                    sanitize_identifier(&op.name.text)?,
                    params.join(", ")
                )
                .map_err(fmt_err)?;
            }
            _ => {}
        }
    }
    writeln!(body, "}}").map_err(fmt_err)?;

    let abstract_source = wrap_compilation_unit(pkg, &imports, &body);
    let abstract_file = JavaFile {
        package_path: pkg.to_string(),
        class_name: abstract_name.clone(),
        source: abstract_source,
    };

    // Concrete subclass skeleton.
    let concrete_body = format!(
        "public class {class} extends {abstract_name} {{\n{ind}// User implementation here\n}}\n"
    );
    let concrete_source = wrap_compilation_unit(pkg, &imports, &concrete_body);
    let concrete_file = JavaFile {
        package_path: pkg.to_string(),
        class_name: class,
        source: concrete_source,
    };

    Ok(vec![abstract_file, concrete_file])
}

/// Spec idl4-java §7.4: IDL interface -> Java public interface with a
/// method per operation (raises -> throws), a property per attribute.
fn emit_non_service_interface_file(
    iface: &InterfaceDef,
    pkg: &str,
    opts: &JavaGenOptions,
) -> Result<JavaFile, JavaGenError> {
    use zerodds_idl::ast::Export;

    let class = sanitize_identifier(&iface.name.text)?;
    let imports = ImportSet::default();
    let ind = indent_unit(opts);
    let mut body = String::new();

    let extends = if iface.bases.is_empty() {
        String::new()
    } else {
        let bases: Vec<String> = iface.bases.iter().map(scoped_to_java).collect();
        format!(" extends {}", bases.join(", "))
    };
    writeln!(body, "public interface {class}{extends} {{").map_err(fmt_err)?;

    for export in &iface.exports {
        match export {
            Export::Op(op) => {
                let ret = match &op.return_type {
                    None => "void".to_string(),
                    Some(t) => typespec_to_java(t)?,
                };
                let params: Vec<String> = op
                    .params
                    .iter()
                    .map(|p| -> Result<String, JavaGenError> {
                        let ty = typespec_to_java(&p.type_spec)?;
                        let pname = sanitize_identifier(&p.name.text)?;
                        Ok(format!("{ty} {pname}"))
                    })
                    .collect::<Result<_, _>>()?;
                let throws = if op.raises.is_empty() {
                    String::new()
                } else {
                    let raises: Vec<String> = op.raises.iter().map(scoped_to_java).collect();
                    format!(" throws {}", raises.join(", "))
                };
                writeln!(
                    body,
                    "{ind}{ret} {}({}){throws};",
                    sanitize_identifier(&op.name.text)?,
                    params.join(", ")
                )
                .map_err(fmt_err)?;
            }
            Export::Attr(attr) => {
                let ty = typespec_to_java(&attr.type_spec)?;
                let aname = sanitize_identifier(&attr.name.text)?;
                writeln!(body, "{ind}{ty} get_{aname}();").map_err(fmt_err)?;
                if !attr.readonly {
                    writeln!(body, "{ind}void set_{aname}({ty} value);").map_err(fmt_err)?;
                }
            }
            _ => {
                // Embedded type/const/exception: not currently implemented.
            }
        }
    }
    writeln!(body, "}}").map_err(fmt_err)?;

    let source = wrap_compilation_unit(pkg, &imports, &body);
    Ok(JavaFile {
        package_path: pkg.to_string(),
        class_name: class,
        source,
    })
}

/// Emits the type/const/exception declarations nested inside an interface
/// body as standalone Java files, placed in a sub-package named after the
/// interface (lowercased, like a module). Without this, `Export::Type`,
/// `Export::Const` and `Export::Except` in an interface were dropped
/// silently (F38).
fn emit_interface_nested_types(
    iface: &InterfaceDef,
    pkg: &str,
    opts: &JavaGenOptions,
    files: &mut Vec<JavaFile>,
    ctx: &EmitCtx,
) -> Result<(), JavaGenError> {
    use zerodds_idl::ast::Export;

    let has_nested = iface
        .exports
        .iter()
        .any(|e| matches!(e, Export::Type(_) | Export::Const(_) | Export::Except(_)));
    if !has_nested {
        return Ok(());
    }
    let scope = sanitize_identifier(&iface.name.text)?.to_lowercase();
    let sub_pkg = if pkg.is_empty() {
        scope
    } else {
        format!("{pkg}.{scope}")
    };
    for export in &iface.exports {
        match export {
            Export::Type(td) => emit_type_decl_top(td, &sub_pkg, opts, files, ctx)?,
            Export::Const(c) => files.push(emit_const_holder(c, &sub_pkg, opts)?),
            Export::Except(e) => files.push(emit_exception_file(e, &sub_pkg, opts)?),
            Export::Op(_) | Export::Attr(_) => {}
        }
    }
    Ok(())
}

/// `true` if the interface annotations contain `@service` — then we
/// treat the interface as an RPC service and delegate to
/// [`crate::rpc`].
fn is_service_interface(iface: &InterfaceDef) -> bool {
    iface
        .annotations
        .iter()
        .any(|a| a.name.parts.last().is_some_and(|p| p.text == "service"))
}

/// Emits the five RPC files for a `@service` interface plus the
/// `exception` files referenced in the `raises` clauses (declared
/// locally in the interface body).
fn emit_service_interface_files(
    iface: &InterfaceDef,
    pkg: &str,
    opts: &JavaGenOptions,
    files: &mut Vec<JavaFile>,
) -> Result<(), JavaGenError> {
    use zerodds_idl::ast::Export;
    use zerodds_rpc::annotations::lower_rpc_annotations;
    use zerodds_rpc::service_mapping::lower_service;

    // Inner exceptions — emit as RuntimeException subclasses, via the
    // existing exception path.
    for export in &iface.exports {
        if let Export::Except(e) = export {
            files.push(emit_exception_file(e, pkg, opts)?);
        }
    }

    // Lower IDL → ServiceDef.
    let lowered = lower_rpc_annotations(&iface.annotations);
    let svc = lower_service(iface, &lowered).map_err(|e| JavaGenError::Internal(e.to_string()))?;

    // Emit the five Java files.
    let svc_files = crate::rpc::emit_service_files(&svc, pkg, opts)?;
    files.extend(svc_files);

    Ok(())
}
