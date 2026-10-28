//! DDS-XTypes 1.3 §7.5.2.3 and Annex C: `AnnotationDescriptor`,
//! `VerbatimTextDescriptor`, `get_annotation` and `get_annotation_count`
//! of a type and a member, and the operations of the descriptors.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use std::sync::Arc;

use zerodds_ast::Ast;
use zerodds_types::dynamic::{
    AnnotationDescriptor, AnnotationType, DynamicError, DynamicType, TypeKind,
    VerbatimTextDescriptor,
};

fn model(idl: &str) -> Arc<Ast> {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "annotations.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("the resolve pass");
    assert!(report.is_clean(), "{:?}", report.broken);
    Arc::new(ast)
}

const IDL: &str = r#"
    @annotation Widgets { long widgets; string label; };
    @mutable @Widgets(widgets = 12, label = "gears")
    @verbatim(language = "c", placement = BEGIN_DECLARATION, text = "/* top */")
    struct Machine {
        @key @id(7) long serial;
        @optional @Widgets(widgets = 3) double load;
    };
"#;

/// §7.5.2.8.6 and §7.5.2.8.5: a type reports every annotation applied to
/// it, the ones XTypes defines (`@mutable`) as well as the declared one,
/// each as a descriptor with its type and string values.
#[test]
fn a_type_reports_its_builtin_and_declared_annotations() {
    let ast = model(IDL);
    let machine = DynamicType::find(&ast, "Machine").unwrap();
    let count = machine.annotation_count();
    assert_eq!(count as usize, machine.annotations().len());
    let mut names = Vec::new();
    for index in 0..count {
        let d = machine.annotation(index).unwrap();
        let t = d.annotation_type().expect("a type");
        assert_eq!(t.kind(), TypeKind::Annotation);
        names.push(t.name().to_string());
    }
    assert!(names.contains(&"mutable".to_string()), "{names:?}");
    assert!(names.contains(&"Widgets".to_string()), "{names:?}");

    let widgets = (0..count)
        .map(|i| machine.annotation(i).unwrap())
        .find(|d| d.annotation_type().unwrap().name() == "Widgets")
        .unwrap();
    assert_eq!(widgets.get_value("widgets").unwrap(), "12");
    assert_eq!(widgets.get_value("label").unwrap(), "gears");
    assert!(widgets.is_consistent());
    assert!(!widgets.annotation_type().unwrap().is_builtin());
}

/// §7.5.2.8.5: an index past the end is RETCODE_BAD_PARAMETER.
#[test]
fn an_index_past_the_end_is_a_bad_parameter() {
    let ast = model(IDL);
    let machine = DynamicType::find(&ast, "Machine").unwrap();
    let past = machine.annotation_count();
    let error = machine.annotation(past).unwrap_err();
    assert!(
        matches!(error, DynamicError::BadParameter { .. }),
        "{error}"
    );
    assert_eq!(error.return_code(), "RETCODE_BAD_PARAMETER");
    assert!(matches!(
        machine.verbatim_text(5).unwrap_err(),
        DynamicError::BadParameter { .. }
    ));
}

/// §7.5.2.6.1: a member reports the annotations applied to it.
#[test]
fn a_member_reports_its_annotations() {
    let ast = model(IDL);
    let machine = DynamicType::find(&ast, "Machine").unwrap();
    let serial = machine.member_by_name("serial").unwrap();
    let applied: Vec<(String, Vec<(String, String)>)> = serial
        .annotations()
        .iter()
        .map(|d| {
            (
                d.annotation_type().unwrap().name().to_string(),
                d.get_all_value()
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            )
        })
        .collect();
    assert!(applied.iter().any(|(n, _)| n == "key"), "{applied:?}");
    assert!(
        applied
            .iter()
            .any(|(n, v)| n == "id" && v == &vec![("value".to_string(), "7".to_string())]),
        "{applied:?}"
    );
    let load = machine.member_by_name("load").unwrap();
    assert!(
        load.annotations()
            .iter()
            .any(|d| d.annotation_type().unwrap().name() == "optional")
    );
    assert_eq!(load.annotation_count() as usize, load.annotations().len());
    assert!(load.annotation(load.annotation_count()).is_err());
}

/// Annex C `get_verbatim_text`: the placement in the spelling of IDL
/// §8.3.5.1, the text and the language.
#[test]
fn verbatim_text_is_reported_with_its_placement() {
    let ast = model(IDL);
    let machine = DynamicType::find(&ast, "Machine").unwrap();
    assert_eq!(machine.verbatim_text_count(), 1);
    let text = machine.verbatim_text(0).unwrap();
    assert_eq!(text.placement, "begin-declaration");
    assert_eq!(text.text, "/* top */");
    assert_eq!(text.language, "c");
    assert!(text.is_consistent());
    let directive = text.to_directive().unwrap();
    assert_eq!(directive.text, "/* top */");
    let mut other = VerbatimTextDescriptor::default();
    assert!(!other.is_consistent());
    other.copy_from(&text);
    assert!(other.equals(&text));
    other.placement = "sideways".into();
    assert!(!other.is_consistent());
    assert!(other.to_directive().is_err());
}

/// §7.5.2.3.1 to §7.5.2.3.3 and Annex C: the value operations, `copy_from`,
/// `equals` and `is_consistent`.
#[test]
fn an_annotation_descriptor_has_value_semantics() {
    let ast = model(IDL);
    let widgets = AnnotationType::find(&ast, "Widgets").expect("the annotation type");

    // A newly created descriptor has a nil type (§7.5.2.3.4) and is not
    // consistent.
    let mut d = AnnotationDescriptor::new();
    assert!(d.annotation_type().is_none());
    assert!(!d.is_consistent());

    d.set_annotation_type(Some(widgets.clone()));
    assert!(
        d.is_consistent(),
        "no value is set, every attribute defaults"
    );
    d.set_value("widgets", "5").unwrap();
    assert!(d.is_consistent());
    assert_eq!(d.get_value("widgets").unwrap(), "5");

    // Annex C: a nil argument is RETCODE_BAD_PARAMETER, a key with no value
    // is RETCODE_NO_DATA.
    assert!(matches!(
        d.set_value("", "1").unwrap_err(),
        DynamicError::BadParameter { .. }
    ));
    assert!(matches!(
        d.set_value("widgets", "").unwrap_err(),
        DynamicError::BadParameter { .. }
    ));
    let missing = d.get_value("label").unwrap_err();
    assert!(matches!(missing, DynamicError::NoData { .. }));
    assert_eq!(missing.return_code(), "RETCODE_NO_DATA");

    // §7.5.2.3.3: an attribute the type does not define, and a value that
    // is no literal of the attribute's type, make it inconsistent.
    let mut unknown = d.clone();
    unknown.set_value("gadgets", "1").unwrap();
    assert!(!unknown.is_consistent());
    let mut not_a_number = d.clone();
    not_a_number.set_value("widgets", "2 + 3").unwrap();
    assert!(!not_a_number.is_consistent());

    // copy_from makes equals true and leaves the source alone.
    let mut copy = AnnotationDescriptor::new();
    assert!(!copy.equals(&d));
    copy.copy_from(&d);
    assert!(copy.equals(&d));
    assert_eq!(d.get_value("widgets").unwrap(), "5");
    copy.set_value("label", "x").unwrap();
    assert!(!copy.equals(&d));
    assert_eq!(d.get_all_value().len(), 1);
}

/// §7.5.2.6.1 and the member descriptor: the default value is a string,
/// copy_from and equals follow Table 53, and `is_consistent_in` applies
/// the rules that need the owner.
#[test]
fn a_member_descriptor_has_a_default_string_and_value_semantics() {
    let ast = model(
        "struct Defaults { @default(5) long five; long none; };
         union U switch (long) { case 1: long a; default: double b; };",
    );
    let ty = DynamicType::find(&ast, "Defaults").unwrap();
    let five = ty.member_by_name("five").unwrap().descriptor().unwrap();
    assert_eq!(five.default_value, "5");
    assert!(five.has_default_value);
    let none = ty.member_by_name("none").unwrap().descriptor().unwrap();
    assert_eq!(none.default_value, "");
    assert!(!none.has_default_value);
    assert!(!five.equals(&none));

    let mut copy = none.clone();
    copy.copy_from(&five);
    assert!(copy.equals(&five));

    // A struct member: legal id, no labels.
    assert!(five.is_consistent_in(TypeKind::Structure).is_ok());
    // The same member claimed for a bitmask: it has a legal aggregated id.
    assert!(five.is_consistent_in(TypeKind::Bitmask).is_err());

    let union = DynamicType::find(&ast, "U").unwrap();
    let a = union.member_by_name("a").unwrap().descriptor().unwrap();
    let b = union.member_by_name("b").unwrap().descriptor().unwrap();
    assert!(a.is_consistent_in(TypeKind::Union).is_ok());
    assert!(b.is_default_label);
    assert!(b.is_consistent_in(TypeKind::Union).is_ok());
    // Labels on a struct member are inconsistent.
    assert!(a.is_consistent_in(TypeKind::Structure).is_err());
    // A union member with no label that is not the default one.
    let mut unlabeled = a.clone();
    unlabeled.label.clear();
    assert!(unlabeled.is_consistent_in(TypeKind::Union).is_err());
}

/// §7.5.2.4.3 and §7.5.2.4.6: the type descriptor.
#[test]
fn a_type_descriptor_has_value_semantics() {
    let ast = model("struct A { long x; }; @appendable struct B { long x; };");
    let a = DynamicType::find(&ast, "A").unwrap().descriptor().unwrap();
    let b = DynamicType::find(&ast, "B").unwrap().descriptor().unwrap();
    assert!(!a.equals(&b));
    let mut copy = b.clone();
    copy.copy_from(&a);
    assert!(copy.equals(&a));
}

/// Annex C `get_all_members_by_name` and `get_all_members`, and the
/// Result forms of the three lookups.
#[test]
fn the_member_lookups_report_a_bad_parameter() {
    let ast = model("struct S { @id(3) long x; long y; }; enum E { P, Q };");
    let s = DynamicType::find(&ast, "S").unwrap();
    assert_eq!(s.all_members_by_name().len(), 2);
    assert_eq!(s.all_members_by_id().len(), 2);
    assert_eq!(s.get_member(3).unwrap().name(), "x");
    assert_eq!(s.get_member_by_name("y").unwrap().name(), "y");
    assert_eq!(s.get_member_by_index(0).unwrap().name(), "x");
    for error in [
        s.get_member(99).unwrap_err(),
        s.get_member_by_name("zzz").unwrap_err(),
        s.get_member_by_index(2).unwrap_err(),
    ] {
        assert!(
            matches!(error, DynamicError::BadParameter { .. }),
            "{error}"
        );
    }
    // §7.5.2.8.2: for a type that is not aggregated the `member` property
    // is empty, but the members by name are the literals (Table 51).
    let e = DynamicType::find(&ast, "E").unwrap();
    assert!(e.all_members_by_id().is_empty());
    assert_eq!(e.all_members_by_name().len(), 2);
}

/// §7.5.2.4.4 and §7.5.2.4.8: a union descriptor carries the discriminator
/// type and a map descriptor the key type, and a descriptor without them is
/// inconsistent; §7.5.2.4.10 and §7.5.2.7.9: so is a name that is no
/// identifier.
#[test]
fn the_descriptor_of_a_union_and_a_map_and_the_names() {
    let ast = model(
        "union U switch (long) { case 1: long a; };
         struct S { map<string, long> table; };",
    );
    let union = DynamicType::find(&ast, "U").unwrap().descriptor().unwrap();
    assert_eq!(
        union.discriminator_type.as_ref().unwrap().kind(),
        TypeKind::Int32
    );
    assert!(union.is_consistent().is_ok());
    let mut without = union.clone();
    without.discriminator_type = None;
    assert!(without.is_consistent().is_err());

    let table = DynamicType::find(&ast, "S")
        .unwrap()
        .member_by_name("table")
        .unwrap()
        .dynamic_type()
        .unwrap()
        .descriptor()
        .unwrap();
    assert_eq!(
        table.key_element_type.as_ref().unwrap().kind(),
        TypeKind::String8
    );
    assert!(table.is_consistent().is_ok());
    let mut keyless = table.clone();
    keyless.key_element_type = None;
    assert!(keyless.is_consistent().is_err());

    let mut renamed = union.clone();
    renamed.name = "not an identifier".to_string();
    assert!(renamed.is_consistent().is_err());
    renamed.name = "a::b::C".to_string();
    assert!(renamed.is_consistent().is_ok());
    renamed.name = String::new();
    assert!(renamed.is_consistent().is_err());

    let s = DynamicType::find(&ast, "S").unwrap();
    let mut member = s.member_by_name("table").unwrap().descriptor().unwrap();
    assert!(member.is_consistent_in(TypeKind::Structure).is_ok());
    member.name = "9lives".to_string();
    assert!(member.is_consistent_in(TypeKind::Structure).is_err());
    member.name = String::new();
    assert!(member.is_consistent_in(TypeKind::Structure).is_err());
}
