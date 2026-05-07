// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! AST-Walker, der Java-17-Source-Files emittiert.
//!
//! Block-A: Header-Layout (`package`, `import`, Class-Modifiers).
//! Block-B: Primitive-Mapping (delegiert an [`crate::type_map`]).
//! Block-C: struct/enum/union/typedef/sequence/array/inheritance.
//! Block-D: Exception → `class X extends RuntimeException`.
//!
//! Java erfordert eine `.java`-Datei pro top-level public class. Der
//! Emitter sammelt waehrend des AST-Walks pro Top-Level-Type genau eine
//! [`JavaFile`]-Struktur.

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write;
// zerodds-lint: BTreeSet wird im Emitter fuer ImportSet + Cycle-Detection verwendet.

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

/// Eine einzelne generierte Java-Source-Datei.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaFile {
    /// Java-Package-Pfad mit Punkt-Trennern (z.B. `org.example.types`).
    pub package_path: String,
    /// Class-Name = Datei-Name ohne `.java`-Suffix.
    pub class_name: String,
    /// Vollstaendige Source-Datei (inkl. `package`, Imports, Class-Body).
    pub source: String,
}

impl JavaFile {
    /// Liefert den relativen Pfad fuer die Datei (z.B. `org/example/Foo.java`).
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

/// Haupt-Entry: walkt das IDL-AST und emittiert eine Liste von Java-Files.
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

/// Emitter-Kontext, der waehrend des AST-Walks read-only gehalten wird.
/// Hier landet der Multi-Inheritance-Index plus Spaeter weitere globale
/// Lookup-Tables (z.B. type-name → topic-eligibility).
#[derive(Debug, Default)]
pub(crate) struct EmitCtx {
    /// Mapping `struct-Name → direkter Basis-Name` (kurzform, ohne
    /// Modul-Prefix). Wir benutzen den letzten `.`-getrennten Token
    /// (siehe [`scoped_to_short`]).
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
                // ValueBox + ValueForward sind Foundation-no-op.
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
                // §7.4.15: User-Defined Annotation-Defs werden bei
                // Anwendung in den annotierten Mitgliedern emittiert,
                // nicht als eigenes Top-Level-Java-Konstrukt.
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
                // Multi-Inheritance-Pattern: emittiere fuer jeden
                // Struct, der selbst Basis eines anderen Struct ist,
                // ein Companion-Interface — so kann ein Sub-Sub-Class
                // den jeweils transitiven Vorfahren via `implements
                // <Anc>Interface` einbinden.
                if ctx.parent_of.values().any(|p| p == &s.name.text) {
                    files.push(emit_struct_companion_interface(s, pkg, opts)?);
                }
                Ok(())
            }
            ConstrTypeDecl::Struct(StructDcl::Forward(_)) => {
                // Forward-Decls in Java implizit (Class wird ohnehin
                // separat erzeugt) — kein File noetig.
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
    }
}

// ---------------------------------------------------------------------------
// Per-Type-Emitter (jeweils 1 JavaFile)
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

    // §7.2.2.4.8 — `@verbatim(placement=BEGIN_FILE)` direkt am Anfang
    // des body (sitzt nach package + imports im Compilation-Unit-Wrap).
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

    // Multi-Inheritance-Pattern: alle transitiven Vorfahren *jenseits*
    // der direkten Basis werden als `implements <X>Interface` gefuehrt.
    // Bei einer einfachen Hierarchie (Single-Base ohne Grandparent)
    // bleibt diese Liste leer.
    let mut implements: Vec<String> = transitive_ancestors_beyond_base(&s.name.text, ctx)
        .into_iter()
        .map(|anc| format!("{anc}Interface"))
        .collect();

    // TopicType-Marker: jede top-level-Struct ohne `@nested`-Marker
    // **und ohne Basis** implementiert `org.omg.dds.topic.TopicType<Self>`.
    //
    // Sub-Strukturen (`struct Child : Base`) erben den Marker vom
    // Parent ueber die regulaere `extends`-Kette — Java verbietet die
    // erneute Implementierung mit eigenem Generic-Param (`TopicType<Child>`
    // vs. `TopicType<Base>`). Das ist spec-konform: in DDS-Java-PSM ist
    // `TopicType<T>` ein Marker-Interface, dessen Generic-Param nur am
    // Wurzel-Type der Vererbungs-Kette steht. Ein Sub-struct ist gemaess
    // Inheritance-Regel weiterhin `instanceof TopicType<Base>` und damit
    // als Topic-Type registrierbar.
    //
    // Findings-Anker: TS-3-Finding 4 (`docs/test-harness/plan.md`).
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

    // §7.2.2.4.8 — `@verbatim(placement=BEGIN_DECLARATION)` als erste
    // Zeile innerhalb des Class-Bodys.
    emit_verbatim_at(
        &mut body,
        &ind,
        &s.annotations,
        PlacementKind::BeginDeclaration,
    )?;

    // Felder.
    for m in &s.members {
        emit_member_field(&mut body, m, &ind)?;
    }
    writeln!(body).map_err(fmt_err)?;

    // Default-Konstruktor.
    writeln!(body, "{ind}public {class}() {{}}").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // Bean-Style Getter / Setter.
    for m in &s.members {
        emit_member_accessors(&mut body, m, &ind)?;
    }

    // §7.2.2.4.8 — `@verbatim(placement=END_DECLARATION)` als letzte
    // Zeile vor dem schliessenden `}`.
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
        // Spec idl4-java-1.0 §7.2 — Custom-Werte statt Auto-Ordinals.
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

/// Union → ein sealed interface + ein Java-File pro Case-Record.
/// Wir geben *einen* File mit sealed interface + nested case-records
/// zurueck (Java erlaubt nested permits in einem File). Dadurch bleibt
/// die Datei-Anzahl deterministisch.
fn emit_union_files(
    u: &UnionDef,
    pkg: &str,
    opts: &JavaGenOptions,
) -> Result<Vec<JavaFile>, JavaGenError> {
    let class = sanitize_identifier(&u.name.text)?;
    let ind = indent_unit(opts);
    let imports = ImportSet::default();

    let _disc_ty = switch_type_to_java(&u.switch_type)?;

    // Permit-Liste der Case-Records (per Member-Name eindeutig).
    let mut permits: Vec<String> = Vec::new();
    let mut case_records: Vec<(String, String, String)> = Vec::new(); // (record-name, field-ty, field-name)
    for c in &u.cases {
        let cpp_ty = type_for_declarator(&c.element.type_spec, &c.element.declarator)?;
        let field_name = sanitize_identifier(&c.element.declarator.name().text)?;
        // Record-Name: CapitalCase aus Field-Name.
        let record_name = capitalize(&field_name);
        if !permits.iter().any(|p| p == &record_name) {
            permits.push(record_name.clone());
            case_records.push((record_name, cpp_ty, field_name));
        }
    }
    // Java verlangt fuer nested-records innerhalb des sealed
    // interface qualifizierte Namen in der `permits`-Klausel —
    // `permits A, B, C` schlaegt mit `cannot find symbol` fehl,
    // `permits Foo.A, Foo.B, Foo.C` ist die korrekte Form.
    //
    // Findings-Anker: TS-3-Finding 5 (`docs/test-harness/plan.md`).
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
    writeln!(body, "public sealed interface {class}{permits_clause} {{").map_err(fmt_err)?;
    emit_verbatim_at(
        &mut body,
        &ind,
        &u.annotations,
        PlacementKind::BeginDeclaration,
    )?;

    // Default-Marker fuer den default-Branch (Kommentar; Branch-Labels
    // werden als Kommentar emittiert, nicht als Java-Konstrukt).
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

    // Nested case-records.
    for (record_name, field_ty, field_name) in &case_records {
        writeln!(
            body,
            "{ind}record {record_name}({field_ty} {field_name}) implements {class} {{}}",
        )
        .map_err(fmt_err)?;
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
    // Java hat keine `using`/`typedef` — wir emittieren eine Wrapper-
    // Klasse pro Alias (1 Wrapper-Field, named `value`).
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
    // IDL-`const` → public static final field in einer Holder-Class
    // namens `<NAME>Constant`.
    let name = sanitize_identifier(&c.name.text)?;
    let class = format!("{name}Constant");
    let java_ty = const_type_to_java(&c.type_)?;
    let val = const_expr_to_java(&c.value);
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
        // Doc-Comment fuer unsigned-Workaround.
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
        TypeSpec::Sequence(s) => {
            // List<Boxed<T>>
            let inner = match &*s.elem {
                TypeSpec::Primitive(p) => primitive_to_java_boxed(*p).to_string(),
                TypeSpec::Scoped(sn) => scoped_to_java(sn),
                TypeSpec::String(_) => "String".into(),
                _ => "Object".into(),
            };
            format!("java.util.List<{inner}>")
        }
        _ => "Object".into(),
    }
}

// ---------------------------------------------------------------------------
// TypeSpec / Declarator
// ---------------------------------------------------------------------------

/// Liefert den Java-Type-Ausdruck fuer Member (TypeSpec + Declarator).
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
            // `java.lang.ArithmeticException` zur Laufzeit, Scale via
            // `setScale(scale)` im Codegen-Output).
            Ok("java.math.BigDecimal".into())
        }
        TypeSpec::Any => {
            // Spec idl4-java §7.3: any -> `org.omg.type.Any`. ZeroDDS-
            // Mapping-Wahl: `java.lang.Object` (Reflection-basiert,
            // Spec sagt explizit "implementation is middleware
            // specific"). org.omg.type.Any-Wrapper-Variante moeglich,
            // aber Object reicht fuer Java-Type-Repr-§8 Pfad.
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

fn const_type_to_java(t: &zerodds_idl::ast::ConstType) -> Result<String, JavaGenError> {
    Ok(match t {
        zerodds_idl::ast::ConstType::Integer(i) => integer_to_java(*i).to_string(),
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

fn scoped_to_java(s: &ScopedName) -> String {
    let parts: Vec<String> = s.parts.iter().map(|p| p.text.clone()).collect();
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

/// Hook fuer C5.4-b: hier kann die Import-Sammlung pro Member
/// erweitert werden. C5.4-a nutzt durchgaengig FQN, daher No-op.
#[allow(clippy::needless_pass_by_ref_mut)]
fn collect_member_imports(_m: &Member, _inc: &mut ImportSet) {
    // FQN-Strategie: java.util.List/Optional/Map werden inline als
    // `java.util.<X>` referenziert, daher keine Import-Eintraege noetig.
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
    // Imports werden derzeit per FQN ersetzt — keine import-Statements
    // erforderlich. Das haelt den Diff stabil und vermeidet Konflikte
    // bei Type-Namen wie `List`/`Map`, falls die IDL einen Type so
    // benannt hat.
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
        LiteralKind::Boolean
        | LiteralKind::Integer
        | LiteralKind::Floating
        | LiteralKind::Char
        | LiteralKind::WideChar
        | LiteralKind::String
        | LiteralKind::WideString
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

/// Wrap-Helper fuer Bitset/Bitmask: wickelt Header + Body in eine
/// Compilation-Unit. Identisch zu [`wrap_compilation_unit`] nur ohne
/// Import-Argument.
pub(crate) fn wrap_compilation_unit_default(pkg: &str, body: &str) -> String {
    wrap_compilation_unit(pkg, &ImportSet::default(), body)
}

// ---------------------------------------------------------------------------
// Multi-Inheritance — Interface-Pattern (C5.4-b §3)
// ---------------------------------------------------------------------------

/// Sammelt fuer jede Struct-Definition den (kurzen) Namen ihres
/// direkten Vorgaengers. IDL4-`struct`-Inheritance ist single — der
/// Codegen erzeugt aus der transitiven Kette die `extends + implements
/// XInterface, YInterface`-Form.
fn collect_base_chain_index(spec: &Specification) -> std::collections::HashMap<String, String> {
    let mut out: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    /// zerodds-lint: recursion-depth 32
    ///
    /// Modul-Hierarchie in IDL-Files: typische Tiefe 2-4
    /// (`org::omg::dds::core`), 32 deckt Edge-Cases.
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

/// Liefert die transitiven Vorfahren-Namen *jenseits* der direkten
/// Basis (alle Grandparents). Bei `A : B`, `B : C`, `C : D` liefert
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

/// Emittiert ein Companion-Interface `<Name>Interface.java` mit
/// Default-Methoden, die das Bean-Pattern-Read-Only-Abbild der
/// Members spiegeln. Dadurch koennen Sub-Sub-Klassen den Vorfahren
/// ueber `implements <Name>Interface` einbinden, ohne dass die
/// JVM-Class-File-Constraints (single `extends`) verletzt werden.
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
    // Default-Methoden — wir geben die Getter-Signaturen mit
    // `default`-Implementierung wieder, die per cast auf die
    // konkrete Class delegiert. Da wir die Klasse zur Compile-Zeit
    // kennen (jede `implements`-Stelle ist eine Subklasse von
    // `class`), erzeugen wir hier nur abstrakte Methoden — die
    // konkrete Class liefert die Implementierung als ueblicher
    // Bean-Getter.
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

// Marker, damit unused-imports keine Warnings produziert (z.B.
// `IntegerType`/`integer_to_java_boxed`, falls Detail-Use spaeter
// wegfaellt).
#[allow(dead_code)]
fn _unused_marker(_i: IntegerType) {
    let _ = integer_to_java_boxed;
    let _ = floating_to_java_boxed;
}

// ---------------------------------------------------------------------------
// RPC-Service-Bridge (DDS-RPC §7.11.2)
// ---------------------------------------------------------------------------

/// Spec idl4-java §7.6: valuetype -> 2 Java-Klassen
/// (`<Name>Abstract` abstract + `<Name>` non-abstract).
/// public state -> public abstract Bean-Accessoren; private state ->
/// protected abstract Accessoren; factory -> void-Method.
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
            // Java erlaubt nur eine super-Class — wir nehmen die erste base.
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

    // Concrete subclass-Skelett.
    let concrete_body = format!(
        "public class {class} extends {abstract_name} {{\n{ind}// User-Implementation hier\n}}\n"
    );
    let concrete_source = wrap_compilation_unit(pkg, &imports, &concrete_body);
    let concrete_file = JavaFile {
        package_path: pkg.to_string(),
        class_name: class,
        source: concrete_source,
    };

    Ok(vec![abstract_file, concrete_file])
}

/// Spec idl4-java §7.4: IDL interface -> Java public interface mit
/// Method pro Operation (raises -> throws), Property pro Attribute.
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
                // Embedded type/const/exception: aktuell nicht implementiert.
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

/// `true` wenn die Interface-Annotations `@service` enthalten — dann
/// behandeln wir das Interface als RPC-Service und delegieren an
/// [`crate::rpc`].
fn is_service_interface(iface: &InterfaceDef) -> bool {
    iface
        .annotations
        .iter()
        .any(|a| a.name.parts.last().is_some_and(|p| p.text == "service"))
}

/// Emittiert die fuenf RPC-Files fuer ein `@service`-Interface plus die
/// `exception`-Files, die in den `raises`-Klauseln referenziert werden
/// (lokal im Interface-Body deklariert).
fn emit_service_interface_files(
    iface: &InterfaceDef,
    pkg: &str,
    opts: &JavaGenOptions,
    files: &mut Vec<JavaFile>,
) -> Result<(), JavaGenError> {
    use zerodds_idl::ast::Export;
    use zerodds_rpc::annotations::lower_rpc_annotations;
    use zerodds_rpc::service_mapping::lower_service;

    // Inner exceptions — emit als RuntimeException-Subclasses, ueber
    // den existierenden Exception-Pfad.
    for export in &iface.exports {
        if let Export::Except(e) = export {
            files.push(emit_exception_file(e, pkg, opts)?);
        }
    }

    // Lower IDL → ServiceDef.
    let lowered = lower_rpc_annotations(&iface.annotations);
    let svc = lower_service(iface, &lowered).map_err(|e| JavaGenError::Internal(e.to_string()))?;

    // Emit die fuenf Java-Files.
    let svc_files = crate::rpc::emit_service_files(&svc, pkg, opts)?;
    files.extend(svc_files);

    Ok(())
}
