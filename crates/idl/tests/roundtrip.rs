//! Roundtrip test (T5.6 + T6.0): parse → AST → print → parse → AST equivalence.
//!
//! Pipeline smoke test for the top-level parser. Compares AST structures
//! without span information (spans shift on re-print, because the
//! format is canonical and does not preserve the original whitespace).
//!
//! T6.0 (CST memoization) removed the phase-0 limit: all three
//! OMG fixtures (zerodds_dcps/security/xtypes) are now included in the roundtrip
//! and run under 200 ms.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_idl::ast::*;
use zerodds_idl::config::ParserConfig;
use zerodds_idl::errors::Span;
use zerodds_idl::parse;

// With T6.0 (CST memoization) full fixture roundtrips are possible
// again. Previously exponential backtracking through nullable
// `<annotation_appl_seq>` hooks; now polynomial via memo.
const DDS_DCPS_IDL: &str = include_str!("fixtures/omg/zerodds_dcps.idl");
const DDS_SECURITY_IDL: &str = include_str!("fixtures/omg/zerodds_security.idl");
const DDS_XTYPES_IDL: &str = include_str!("fixtures/omg/dds_xtypes.idl");

fn roundtrip(name: &str, src: &str) {
    roundtrip_with(name, src, ParserConfig::default());
}

fn roundtrip_with(name: &str, src: &str, cfg: ParserConfig) {
    let ast1 = parse(src, &cfg).unwrap_or_else(|e| panic!("first parse {name} failed: {e}"));
    let printed = format!("{ast1}");
    let ast2 = parse(&printed, &cfg).unwrap_or_else(|e| {
        panic!(
            "second parse {name} failed: {e}\n\
             printed output was:\n{printed}"
        )
    });
    let n1 = normalize_spec(&ast1);
    let n2 = normalize_spec(&ast2);
    assert_eq!(
        n1, n2,
        "AST drift in {name}\nfirst:\n{ast1}\n\nprinted:\n{printed}\n\nsecond:\n{ast2}"
    );
}

#[test]
fn roundtrip_simple_struct() {
    roundtrip("struct", "struct P { long x; long y; };");
}

#[test]
fn roundtrip_module_with_types() {
    roundtrip(
        "module",
        r"
        module svc {
            struct Point { long x; long y; };
            typedef sequence<Point> Path;
            const long MAX = 100;
        };
        ",
    );
}

#[test]
fn roundtrip_annotated_topic() {
    roundtrip(
        "topic",
        r#"
        @topic
        @appendable
        struct Sensor {
            @key long sensor_id;
            double value;
            @optional string label;
        };
        "#,
    );
}

#[test]
fn roundtrip_interface_with_ops() {
    // The `oneway` op is a CORBA construct, gated via corba_oneway_op.
    roundtrip_with(
        "interface",
        r#"
        interface Service {
            long add(in long a, in long b);
            oneway void log(in string msg);
            void put(@key in string id, in string value) raises (NotFound);
            readonly attribute long version;
        };
        "#,
        ParserConfig::full_4_2(),
    );
}

#[test]
fn roundtrip_union_enum_bitset_bitmask() {
    roundtrip(
        "constructed",
        r#"
        enum Color { RED, GREEN, BLUE };
        union V switch (long) {
            case 1: long a;
            default: string b;
        };
        bitset Flags { bitfield<3> level; bitfield<1> on; };
        bitmask Permissions { READ, WRITE, EXEC };
        "#,
    );
}

#[test]
fn roundtrip_struct_inheritance_and_map() {
    roundtrip(
        "extended",
        r"
        struct Base { long id; };
        struct Derived : Base { string name; };
        typedef map<string, long> Idx;
        ",
    );
}

#[test]
fn roundtrip_dds_dcps_fixture() {
    roundtrip("zerodds_dcps.idl", DDS_DCPS_IDL);
}

#[test]
fn roundtrip_dds_security_fixture() {
    roundtrip("zerodds_security.idl", DDS_SECURITY_IDL);
}

#[test]
fn roundtrip_dds_xtypes_fixture() {
    roundtrip("dds_xtypes.idl", DDS_XTYPES_IDL);
}

// ============================================================================
// Span normalization
// ============================================================================
// Spans change between the original source and the re-print. The comparison
// must therefore be span-free. We set all spans to SYNTHETIC and then compare
// with derived `PartialEq`.

fn normalize_spec(spec: &Specification) -> Specification {
    Specification {
        definitions: spec.definitions.iter().map(normalize_def).collect(),
        span: Span::SYNTHETIC,
    }
}

fn normalize_def(def: &Definition) -> Definition {
    match def {
        Definition::Module(m) => Definition::Module(ModuleDef {
            name: normalize_id(&m.name),
            definitions: m.definitions.iter().map(normalize_def).collect(),
            annotations: m.annotations.iter().map(normalize_annotation).collect(),
            span: Span::SYNTHETIC,
        }),
        Definition::Type(t) => Definition::Type(normalize_type_decl(t)),
        Definition::Const(c) => Definition::Const(normalize_const_decl(c)),
        Definition::Except(e) => Definition::Except(normalize_except_decl(e)),
        Definition::Interface(i) => Definition::Interface(normalize_interface_dcl(i)),
        Definition::ValueBox(v) => Definition::ValueBox(ValueBoxDecl {
            name: normalize_id(&v.name),
            type_spec: normalize_type_spec(&v.type_spec),
            annotations: v.annotations.iter().map(normalize_annotation).collect(),
            span: Span::SYNTHETIC,
        }),
        Definition::ValueForward(v) => Definition::ValueForward(ValueForwardDecl {
            name: normalize_id(&v.name),
            span: Span::SYNTHETIC,
        }),
        Definition::Annotation(a) => Definition::Annotation(AnnotationDcl {
            name: normalize_id(&a.name),
            members: a
                .members
                .iter()
                .map(|m| AnnotationMember {
                    name: normalize_id(&m.name),
                    type_spec: m.type_spec.clone(),
                    default: m.default.clone(),
                    span: Span::SYNTHETIC,
                })
                .collect(),
            embedded_types: a.embedded_types.iter().map(normalize_type_decl).collect(),
            embedded_consts: a.embedded_consts.iter().map(normalize_const_decl).collect(),
            span: Span::SYNTHETIC,
        }),
        Definition::VendorExtension(v) => Definition::VendorExtension(VendorExtension {
            production_name: v.production_name.clone(),
            raw: v.raw.clone(),
            span: Span::SYNTHETIC,
        }),
        // New AST variants (CORBA top-level, components, templates):
        // roundtrip normalization follows with the per-variant builder
        // refactor. Until then: identity (clones keep their content; spans
        // remain non-normalized in the roundtrip diagnostics, which
        // is acceptable for a pure recognizer-roundtrip comparison).
        Definition::ValueDef(_)
        | Definition::TypeId(_)
        | Definition::TypePrefix(_)
        | Definition::Import(_)
        | Definition::Component(_)
        | Definition::Home(_)
        | Definition::Event(_)
        | Definition::Porttype(_)
        | Definition::Connector(_)
        | Definition::TemplateModule(_)
        | Definition::TemplateModuleInst(_) => def.clone(),
    }
}

fn normalize_id(id: &Identifier) -> Identifier {
    Identifier::new(id.text.clone(), Span::SYNTHETIC)
}

fn normalize_scoped(s: &ScopedName) -> ScopedName {
    ScopedName {
        absolute: s.absolute,
        parts: s.parts.iter().map(normalize_id).collect(),
        span: Span::SYNTHETIC,
    }
}

fn normalize_type_decl(t: &TypeDecl) -> TypeDecl {
    match t {
        TypeDecl::Constr(c) => TypeDecl::Constr(normalize_constr(c)),
        TypeDecl::Typedef(t) => TypeDecl::Typedef(TypedefDecl {
            type_spec: normalize_type_spec(&t.type_spec),
            declarators: t.declarators.iter().map(normalize_declarator).collect(),
            annotations: t.annotations.iter().map(normalize_annotation).collect(),
            span: Span::SYNTHETIC,
        }),
        TypeDecl::Native(n) => TypeDecl::Native(NativeDecl {
            name: normalize_id(&n.name),
            span: Span::SYNTHETIC,
        }),
    }
}

fn normalize_constr(c: &ConstrTypeDecl) -> ConstrTypeDecl {
    match c {
        ConstrTypeDecl::Struct(s) => ConstrTypeDecl::Struct(normalize_struct(s)),
        ConstrTypeDecl::Union(u) => ConstrTypeDecl::Union(normalize_union(u)),
        ConstrTypeDecl::Enum(e) => ConstrTypeDecl::Enum(EnumDef {
            name: normalize_id(&e.name),
            enumerators: e
                .enumerators
                .iter()
                .map(|en| Enumerator {
                    name: normalize_id(&en.name),
                    annotations: en.annotations.iter().map(normalize_annotation).collect(),
                    span: Span::SYNTHETIC,
                })
                .collect(),
            annotations: e.annotations.iter().map(normalize_annotation).collect(),
            span: Span::SYNTHETIC,
        }),
        ConstrTypeDecl::Bitset(b) => ConstrTypeDecl::Bitset(BitsetDecl {
            name: normalize_id(&b.name),
            base: b.base.as_ref().map(normalize_scoped),
            bitfields: b.bitfields.iter().map(normalize_bitfield).collect(),
            annotations: b.annotations.iter().map(normalize_annotation).collect(),
            span: Span::SYNTHETIC,
        }),
        ConstrTypeDecl::Bitmask(b) => ConstrTypeDecl::Bitmask(BitmaskDecl {
            name: normalize_id(&b.name),
            values: b
                .values
                .iter()
                .map(|v| BitValue {
                    name: normalize_id(&v.name),
                    annotations: v.annotations.iter().map(normalize_annotation).collect(),
                    span: Span::SYNTHETIC,
                })
                .collect(),
            annotations: b.annotations.iter().map(normalize_annotation).collect(),
            span: Span::SYNTHETIC,
        }),
    }
}

fn normalize_struct(s: &StructDcl) -> StructDcl {
    match s {
        StructDcl::Def(d) => StructDcl::Def(StructDef {
            name: normalize_id(&d.name),
            base: d.base.as_ref().map(normalize_scoped),
            members: d.members.iter().map(normalize_member).collect(),
            annotations: d.annotations.iter().map(normalize_annotation).collect(),
            span: Span::SYNTHETIC,
        }),
        StructDcl::Forward(d) => StructDcl::Forward(StructForwardDecl {
            name: normalize_id(&d.name),
            span: Span::SYNTHETIC,
        }),
    }
}

fn normalize_member(m: &Member) -> Member {
    Member {
        type_spec: normalize_type_spec(&m.type_spec),
        declarators: m.declarators.iter().map(normalize_declarator).collect(),
        annotations: m.annotations.iter().map(normalize_annotation).collect(),
        span: Span::SYNTHETIC,
    }
}

fn normalize_union(u: &UnionDcl) -> UnionDcl {
    match u {
        UnionDcl::Def(d) => UnionDcl::Def(UnionDef {
            name: normalize_id(&d.name),
            switch_type: normalize_switch_type(&d.switch_type),
            cases: d
                .cases
                .iter()
                .map(|c| Case {
                    labels: c.labels.iter().map(normalize_case_label).collect(),
                    element: ElementSpec {
                        type_spec: normalize_type_spec(&c.element.type_spec),
                        declarator: normalize_declarator(&c.element.declarator),
                        annotations: c
                            .element
                            .annotations
                            .iter()
                            .map(normalize_annotation)
                            .collect(),
                        span: Span::SYNTHETIC,
                    },
                    annotations: c.annotations.iter().map(normalize_annotation).collect(),
                    span: Span::SYNTHETIC,
                })
                .collect(),
            annotations: d.annotations.iter().map(normalize_annotation).collect(),
            span: Span::SYNTHETIC,
        }),
        UnionDcl::Forward(d) => UnionDcl::Forward(UnionForwardDecl {
            name: normalize_id(&d.name),
            span: Span::SYNTHETIC,
        }),
    }
}

fn normalize_switch_type(s: &SwitchTypeSpec) -> SwitchTypeSpec {
    match s {
        SwitchTypeSpec::Scoped(n) => SwitchTypeSpec::Scoped(normalize_scoped(n)),
        other => other.clone(),
    }
}

fn normalize_case_label(c: &CaseLabel) -> CaseLabel {
    match c {
        CaseLabel::Default => CaseLabel::Default,
        CaseLabel::Value(e) => CaseLabel::Value(normalize_const_expr(e)),
    }
}

fn normalize_bitfield(b: &Bitfield) -> Bitfield {
    Bitfield {
        spec: BitfieldSpec {
            width: normalize_const_expr(&b.spec.width),
            dest_type: b.spec.dest_type,
            span: Span::SYNTHETIC,
        },
        name: b.name.as_ref().map(normalize_id),
        annotations: b.annotations.iter().map(normalize_annotation).collect(),
        span: Span::SYNTHETIC,
    }
}

fn normalize_declarator(d: &Declarator) -> Declarator {
    match d {
        Declarator::Simple(n) => Declarator::Simple(normalize_id(n)),
        Declarator::Array(a) => Declarator::Array(ArrayDeclarator {
            name: normalize_id(&a.name),
            sizes: a.sizes.iter().map(normalize_const_expr).collect(),
            span: Span::SYNTHETIC,
        }),
    }
}

fn normalize_const_decl(c: &ConstDecl) -> ConstDecl {
    ConstDecl {
        name: normalize_id(&c.name),
        type_: normalize_const_type(&c.type_),
        value: normalize_const_expr(&c.value),
        annotations: c.annotations.iter().map(normalize_annotation).collect(),
        span: Span::SYNTHETIC,
    }
}

fn normalize_const_type(t: &ConstType) -> ConstType {
    match t {
        ConstType::Scoped(s) => ConstType::Scoped(normalize_scoped(s)),
        other => other.clone(),
    }
}

fn normalize_const_expr(e: &ConstExpr) -> ConstExpr {
    match e {
        ConstExpr::Literal(l) => ConstExpr::Literal(Literal {
            kind: l.kind,
            raw: l.raw.clone(),
            span: Span::SYNTHETIC,
        }),
        ConstExpr::Scoped(s) => ConstExpr::Scoped(normalize_scoped(s)),
        ConstExpr::Unary { op, operand, .. } => ConstExpr::Unary {
            op: *op,
            operand: Box::new(normalize_const_expr(operand)),
            span: Span::SYNTHETIC,
        },
        ConstExpr::Binary { op, lhs, rhs, .. } => ConstExpr::Binary {
            op: *op,
            lhs: Box::new(normalize_const_expr(lhs)),
            rhs: Box::new(normalize_const_expr(rhs)),
            span: Span::SYNTHETIC,
        },
    }
}

fn normalize_except_decl(e: &ExceptDecl) -> ExceptDecl {
    ExceptDecl {
        name: normalize_id(&e.name),
        members: e.members.iter().map(normalize_member).collect(),
        annotations: e.annotations.iter().map(normalize_annotation).collect(),
        span: Span::SYNTHETIC,
    }
}

fn normalize_interface_dcl(i: &InterfaceDcl) -> InterfaceDcl {
    match i {
        InterfaceDcl::Def(d) => InterfaceDcl::Def(InterfaceDef {
            kind: d.kind,
            name: normalize_id(&d.name),
            bases: d.bases.iter().map(normalize_scoped).collect(),
            exports: d.exports.iter().map(normalize_export).collect(),
            annotations: d.annotations.iter().map(normalize_annotation).collect(),
            span: Span::SYNTHETIC,
        }),
        InterfaceDcl::Forward(d) => InterfaceDcl::Forward(InterfaceForwardDecl {
            kind: d.kind,
            name: normalize_id(&d.name),
            span: Span::SYNTHETIC,
        }),
    }
}

fn normalize_export(e: &Export) -> Export {
    match e {
        Export::Op(o) => Export::Op(OpDecl {
            name: normalize_id(&o.name),
            oneway: o.oneway,
            return_type: o.return_type.as_ref().map(normalize_type_spec),
            params: o
                .params
                .iter()
                .map(|p| ParamDecl {
                    attribute: p.attribute,
                    type_spec: normalize_type_spec(&p.type_spec),
                    name: normalize_id(&p.name),
                    annotations: p.annotations.iter().map(normalize_annotation).collect(),
                    span: Span::SYNTHETIC,
                })
                .collect(),
            raises: o.raises.iter().map(normalize_scoped).collect(),
            context: o.context.clone(),
            annotations: o.annotations.iter().map(normalize_annotation).collect(),
            span: Span::SYNTHETIC,
        }),
        Export::Attr(a) => Export::Attr(AttrDecl {
            name: normalize_id(&a.name),
            type_spec: normalize_type_spec(&a.type_spec),
            readonly: a.readonly,
            get_raises: a.get_raises.iter().map(normalize_scoped).collect(),
            set_raises: a.set_raises.iter().map(normalize_scoped).collect(),
            annotations: a.annotations.iter().map(normalize_annotation).collect(),
            span: Span::SYNTHETIC,
        }),
        Export::Type(t) => Export::Type(normalize_type_decl(t)),
        Export::Const(c) => Export::Const(normalize_const_decl(c)),
        Export::Except(e) => Export::Except(normalize_except_decl(e)),
    }
}

fn normalize_type_spec(t: &TypeSpec) -> TypeSpec {
    match t {
        TypeSpec::Primitive(p) => TypeSpec::Primitive(*p),
        TypeSpec::Scoped(s) => TypeSpec::Scoped(normalize_scoped(s)),
        TypeSpec::Sequence(s) => TypeSpec::Sequence(SequenceType {
            elem: Box::new(normalize_type_spec(&s.elem)),
            bound: s.bound.as_ref().map(normalize_const_expr),
            span: Span::SYNTHETIC,
        }),
        TypeSpec::String(s) => TypeSpec::String(StringType {
            wide: s.wide,
            bound: s.bound.as_ref().map(normalize_const_expr),
            span: Span::SYNTHETIC,
        }),
        TypeSpec::Fixed(p) => TypeSpec::Fixed(FixedPtType {
            digits: normalize_const_expr(&p.digits),
            scale: normalize_const_expr(&p.scale),
            span: Span::SYNTHETIC,
        }),
        TypeSpec::Map(m) => TypeSpec::Map(MapType {
            key: Box::new(normalize_type_spec(&m.key)),
            value: Box::new(normalize_type_spec(&m.value)),
            bound: m.bound.as_ref().map(normalize_const_expr),
            span: Span::SYNTHETIC,
        }),
        TypeSpec::Any => TypeSpec::Any,
    }
}

fn normalize_annotation(a: &Annotation) -> Annotation {
    Annotation {
        name: normalize_scoped(&a.name),
        params: match &a.params {
            AnnotationParams::None => AnnotationParams::None,
            AnnotationParams::Empty => AnnotationParams::Empty,
            AnnotationParams::Single(e) => AnnotationParams::Single(normalize_const_expr(e)),
            AnnotationParams::Named(items) => AnnotationParams::Named(
                items
                    .iter()
                    .map(|p| NamedParam {
                        name: normalize_id(&p.name),
                        value: normalize_const_expr(&p.value),
                        span: Span::SYNTHETIC,
                    })
                    .collect(),
            ),
        },
        span: Span::SYNTHETIC,
    }
}
