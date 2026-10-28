//! DDS-XTypes 1.3 §7.5.2.2 and §7.5.2.9: `DynamicTypeBuilderFactory`
//! (Table 48) and `DynamicTypeBuilder` (Table 56).

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use std::sync::Arc;

use zerodds_ast::Ast;
use zerodds_types::dynamic::{
    AnnotationDescriptor, AnnotationType, DynamicError, DynamicType, DynamicTypeBuilderFactory,
    ExtensibilityKind, LENGTH_UNLIMITED, MEMBER_ID_INVALID, MemberDescriptor, TryConstructKind,
    TypeDescriptor, TypeKind,
};

fn model(idl: &str) -> Arc<Ast> {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "factory.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("the resolve pass");
    assert!(report.is_clean(), "{:?}", report.broken);
    Arc::new(ast)
}

fn descriptor(kind: TypeKind, name: &str) -> TypeDescriptor {
    TypeDescriptor {
        kind,
        name: name.to_string(),
        base_type: None,
        discriminator_type: None,
        bound: Vec::new(),
        element_type: None,
        key_element_type: None,
        extensibility_kind: Some(ExtensibilityKind::Final),
        is_nested: false,
        bit_bound: None,
    }
}

fn member(name: &str, type_: DynamicType) -> MemberDescriptor {
    MemberDescriptor {
        name: name.to_string(),
        id: MEMBER_ID_INVALID,
        type_,
        index: u32::MAX,
        label: Vec::new(),
        is_default_label: false,
        try_construct_kind: TryConstructKind::Discard,
        is_key: false,
        is_optional: false,
        is_must_understand: false,
        is_shared: false,
        has_default_value: false,
        default_value: String::new(),
    }
}

/// §7.5.2.2.12: a primitive kind answers its type; any other kind fails
/// with RETCODE_BAD_PARAMETER.
#[test]
fn a_primitive_type_is_answered_and_a_composite_kind_is_not() {
    let factory = DynamicTypeBuilderFactory::get_instance();
    assert_eq!(
        factory.get_primitive_type(TypeKind::Int32).unwrap().kind(),
        TypeKind::Int32
    );
    let error = factory.get_primitive_type(TypeKind::Structure).unwrap_err();
    assert_eq!(error.return_code(), "RETCODE_BAD_PARAMETER");
    // §7.5.2.2.11: get_instance is legal after delete_instance.
    DynamicTypeBuilderFactory::delete_instance().unwrap();
    let again = DynamicTypeBuilderFactory::get_instance();
    // §7.5.2.2.10: deleting a primitive type, and any type, succeeds.
    let long = again.get_primitive_type(TypeKind::Int32).unwrap();
    again.delete_type(long).unwrap();
}

/// §7.5.2.2.6 and §7.5.2.9.4: a structure from a descriptor, members added
/// one by one; an id of MEMBER_ID_INVALID takes the maximum plus one
/// (§7.5.2.7.5); a member added at an index shifts the rest
/// (§7.5.2.7.6).
#[test]
fn a_structure_is_created_from_a_descriptor_and_members_are_added() {
    let factory = DynamicTypeBuilderFactory::get_instance();
    let long = factory.get_primitive_type(TypeKind::Int32).unwrap();
    let double = factory.get_primitive_type(TypeKind::Float64).unwrap();

    let mut builder = factory
        .create_type(&descriptor(TypeKind::Structure, "demo::Point"))
        .unwrap();
    assert_eq!(builder.get_kind().unwrap(), TypeKind::Structure);
    assert_eq!(builder.get_name().unwrap(), "demo::Point");
    builder.add_member(&member("x", long.clone())).unwrap();
    builder.add_member(&member("y", long.clone())).unwrap();
    let before = builder.build().unwrap();
    assert_eq!(before.member_count(), 2);

    let mut z = member("z", double);
    z.index = 1;
    builder.add_member(&z).unwrap();
    assert_eq!(builder.get_member_count().unwrap(), 3);
    let after = builder.build().unwrap();
    let names: Vec<String> = (0..3)
        .map(|i| after.get_member_by_index(i).unwrap().name().to_string())
        .collect();
    assert_eq!(names, ["x", "z", "y"]);
    // The ids are 0, 1, 2 in the order of addition.
    assert_eq!(after.get_member_by_name("x").unwrap().id(), 0);
    assert_eq!(after.get_member_by_name("y").unwrap().id(), 1);
    assert_eq!(after.get_member_by_name("z").unwrap().id(), 2);

    // §7.5.2.9: the lookups of the builder are those of the type as it stands.
    assert_eq!(builder.get_member_by_name("z").unwrap().id(), 2);
    assert_eq!(builder.get_member(1).unwrap().name(), "y");
    assert_eq!(builder.get_member_by_index(0).unwrap().name(), "x");
    assert!(builder.get_member(99).is_err());
    assert!(builder.get_member_by_index(3).is_err());
    assert_eq!(builder.all_members_by_name().unwrap().len(), 3);
    assert_eq!(builder.all_members_by_id().unwrap().len(), 3);

    // The type built earlier does not change under its holder.
    assert_eq!(before.member_count(), 2);
    assert!(!builder.equals(&before).unwrap());
    assert!(builder.equals(&after).unwrap());
}

/// §7.5.2.2.6: an inconsistent descriptor fails.
#[test]
fn an_inconsistent_descriptor_creates_no_type() {
    let factory = DynamicTypeBuilderFactory::get_instance();
    // A sequence with no element type.
    let error = factory
        .create_type(&descriptor(TypeKind::Sequence, ""))
        .err()
        .expect("an error");
    assert!(
        matches!(error, DynamicError::BadParameter { .. }),
        "{error}"
    );
    // A structure with no name.
    assert!(
        factory
            .create_type(&descriptor(TypeKind::Structure, ""))
            .is_err()
    );
}

/// Table 55: a member of the wrong shape fails with RETCODE_BAD_PARAMETER
/// and an alias passes the member on to its base type or fails with
/// RETCODE_PRECONDITION_NOT_MET.
#[test]
fn add_member_follows_table_55() {
    let factory = DynamicTypeBuilderFactory::get_instance();
    let long = factory.get_primitive_type(TypeKind::Int32).unwrap();

    // A union member needs a label or the default.
    let mut union = descriptor(TypeKind::Union, "demo::U");
    union.discriminator_type = Some(long.clone());
    let mut builder = factory.create_type(&union).unwrap();
    let error = builder.add_member(&member("a", long.clone())).unwrap_err();
    assert!(
        matches!(error, DynamicError::BadParameter { .. }),
        "{error}"
    );
    let mut labelled = member("a", long.clone());
    labelled.label = vec![1, 2];
    builder.add_member(&labelled).unwrap();
    let mut default = member("b", long.clone());
    default.is_default_label = true;
    builder.add_member(&default).unwrap();
    let union = builder.build().unwrap();
    let a = union.member_by_name("a").unwrap().descriptor().unwrap();
    assert_eq!(a.label, [1, 2]);
    assert!(
        union
            .member_by_name("b")
            .unwrap()
            .descriptor()
            .unwrap()
            .is_default_label
    );

    // An alias of a structure takes the member to the structure.
    let structure = factory
        .create_type(&descriptor(TypeKind::Structure, "demo::S"))
        .unwrap()
        .build()
        .unwrap();
    let mut alias = descriptor(TypeKind::Alias, "demo::Alias");
    alias.base_type = Some(structure);
    let mut builder = factory.create_type(&alias).unwrap();
    builder.add_member(&member("m", long.clone())).unwrap();
    let alias = builder.build().unwrap();
    assert_eq!(alias.kind(), TypeKind::Alias);
    assert_eq!(alias.resolve_alias().unwrap().member_count(), 1);

    // An alias of an int takes no member.
    let mut to_int = descriptor(TypeKind::Alias, "demo::Meters");
    to_int.base_type = Some(long.clone());
    let mut builder = factory.create_type(&to_int).unwrap();
    let error = builder.add_member(&member("m", long)).unwrap_err();
    assert!(
        matches!(error, DynamicError::PreconditionNotMet { .. }),
        "{error}"
    );
    assert_eq!(error.return_code(), "RETCODE_PRECONDITION_NOT_MET");
}

/// Table 55: an enumeration takes a literal with its own type, a bitmask
/// a Boolean flag inside the bound.
#[test]
fn an_enumeration_and_a_bitmask_take_literals_and_flags() {
    let factory = DynamicTypeBuilderFactory::get_instance();
    let long = factory.get_primitive_type(TypeKind::Int32).unwrap();
    let boolean = factory.get_primitive_type(TypeKind::Boolean).unwrap();

    let mut enumeration = factory
        .create_type(&descriptor(TypeKind::Enumeration, "demo::Color"))
        .unwrap();
    let own = enumeration.build().unwrap();
    // A literal that is not of the enumeration's type is refused.
    assert!(enumeration.add_member(&member("RED", long)).is_err());
    enumeration.add_member(&member("RED", own.clone())).unwrap();
    let mut green = member("GREEN", own);
    green.default_value = "5".to_string();
    enumeration.add_member(&green).unwrap();
    let color = enumeration.build().unwrap();
    assert_eq!(
        color.member_by_name("RED").unwrap().literal_value(),
        Some(0)
    );
    assert_eq!(
        color.member_by_name("GREEN").unwrap().literal_value(),
        Some(5)
    );
    // The descriptor of a literal carries its value, so it adds back.
    assert_eq!(
        color
            .member_by_name("GREEN")
            .unwrap()
            .descriptor()
            .unwrap()
            .default_value,
        "5"
    );

    let mut mask = factory.create_bitmask_type(8).unwrap();
    mask.add_member(&member("READ", boolean.clone())).unwrap();
    let mut write = member("WRITE", boolean.clone());
    write.default_value = "3".to_string();
    mask.add_member(&write).unwrap();
    let mut outside = member("TOO_HIGH", boolean);
    outside.default_value = "8".to_string();
    let error = mask.add_member(&outside).unwrap_err();
    assert!(
        matches!(error, DynamicError::BadParameter { .. }),
        "{error}"
    );
    let mask = mask.build().unwrap();
    assert_eq!(mask.kind(), TypeKind::Bitmask);
    assert_eq!(
        mask.member_by_name("WRITE").unwrap().literal_value(),
        Some(3)
    );

    // §7.5.2.2.2: a bound out of range fails.
    assert!(factory.create_bitmask_type(0).is_err());
    assert!(factory.create_bitmask_type(65).is_err());
}

/// §7.5.2.2.1, .3, .4, .5: the collection and string creators.
#[test]
fn the_collection_and_string_creators_build_their_kinds() {
    let factory = DynamicTypeBuilderFactory::get_instance();
    let ast = model("struct Item { long id; };");
    let item = DynamicType::find(&ast, "Item").unwrap();
    let text = factory.create_string_type(16).unwrap().build().unwrap();
    assert_eq!(text.kind(), TypeKind::String8);
    assert_eq!(text.descriptor().unwrap().bound, [16]);
    let wide = factory
        .create_wstring_type(zerodds_types::dynamic::LENGTH_UNLIMITED)
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(wide.kind(), TypeKind::String16);
    assert_eq!(wide.descriptor().unwrap().bound, [LENGTH_UNLIMITED]);

    let seq = factory
        .create_sequence_type(&item, 4)
        .unwrap()
        .build()
        .unwrap();
    let d = seq.descriptor().unwrap();
    assert_eq!((d.kind, d.bound.as_slice()), (TypeKind::Sequence, &[4][..]));
    let element = d.element_type.unwrap();
    assert_eq!(element.name(), "Item");
    assert!(
        element.equals(&item),
        "the element is a copy of the same type"
    );

    let array = factory
        .create_array_type(&item, &[3, 2])
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(array.descriptor().unwrap().bound, [3, 2]);
    assert!(factory.create_array_type(&item, &[]).is_err());

    let key = factory.get_primitive_type(TypeKind::Int32).unwrap();
    let map = factory
        .create_map_type(&key, &item, 8)
        .unwrap()
        .build()
        .unwrap();
    let d = map.descriptor().unwrap();
    assert_eq!(d.kind, TypeKind::Map);
    assert_eq!(d.key_element_type.unwrap().kind(), TypeKind::Int32);
    assert_eq!(d.element_type.unwrap().name(), "Item");
}

/// §7.5.2.2.7: the copy has the state of the original, changes to it do
/// not reach the original, and a recursive type is copied once.
#[test]
fn a_copy_carries_the_state_and_leaves_the_original_alone() {
    let factory = DynamicTypeBuilderFactory::get_instance();
    let ast = model(
        "module m { @mutable struct Node {
            @key @id(4) long serial;
            @default(9) long nine;
            sequence<Node> children;
         }; };",
    );
    let original = DynamicType::find(&ast, "m::Node").unwrap();
    let mut copy = factory.create_type_copy(&original).unwrap();
    assert!(copy.equals(&original).unwrap());
    let built = copy.build().unwrap();
    assert_eq!(built.qualified_name(), "m::Node");
    assert_eq!(
        built.descriptor().unwrap().extensibility_kind,
        Some(ExtensibilityKind::Mutable)
    );
    assert_eq!(built.get_member_by_name("serial").unwrap().id(), 4);
    assert_eq!(
        built
            .get_member_by_name("nine")
            .unwrap()
            .descriptor()
            .unwrap()
            .default_value,
        "9"
    );
    // The self-reference points at the copy.
    let children = built
        .get_member_by_name("children")
        .unwrap()
        .dynamic_type()
        .unwrap();
    assert_eq!(
        children.descriptor().unwrap().element_type.unwrap().id(),
        built.id()
    );

    let long = factory.get_primitive_type(TypeKind::Int32).unwrap();
    copy.add_member(&member("extra", long)).unwrap();
    assert_eq!(copy.get_member_count().unwrap(), 4);
    assert_eq!(original.member_count(), 3);
    assert!(!copy.equals(&original).unwrap());
}

/// §7.5.2.9.5 and §7.5.2.9.6: annotations applied to the type and to a
/// member; a declared annotation is copied with its declaration.
#[test]
fn annotations_are_applied_to_the_type_and_to_a_member() {
    let factory = DynamicTypeBuilderFactory::get_instance();
    let ast = model(
        "@annotation Widgets { long widgets; string label; };
         @Widgets(widgets = 12, label = \"gears\") struct Source { long a; };",
    );
    let source = DynamicType::find(&ast, "Source").unwrap();
    let long = factory.get_primitive_type(TypeKind::Int32).unwrap();

    let mut builder = factory
        .create_type(&descriptor(TypeKind::Structure, "demo::T"))
        .unwrap();
    builder.add_member(&member("a", long.clone())).unwrap();
    builder.add_member(&member("b", long)).unwrap();

    // A built-in annotation on the type.
    let mutable = AnnotationDescriptor::of_type(
        AnnotationType::find(&ast, "mutable").expect("a built-in annotation"),
    );
    builder.apply_annotation(&mutable).unwrap();
    assert_eq!(
        builder.get_descriptor().unwrap().extensibility_kind,
        Some(ExtensibilityKind::Mutable)
    );

    // A declared annotation, taken from another model.
    let widgets = source
        .annotations()
        .into_iter()
        .find(|d| d.annotation_type().unwrap().name() == "Widgets")
        .unwrap();
    builder.apply_annotation(&widgets).unwrap();
    let built = builder.build().unwrap();
    let applied = built
        .annotations()
        .into_iter()
        .find(|d| d.annotation_type().unwrap().name() == "Widgets")
        .expect("the annotation is applied");
    assert_eq!(applied.get_value("widgets").unwrap(), "12");
    assert_eq!(applied.get_value("label").unwrap(), "gears");
    assert!(applied.is_consistent());
    assert_eq!(
        builder.get_annotation_count().unwrap(),
        built.annotation_count()
    );
    assert!(builder.get_annotation(0).is_ok());

    // A built-in annotation on a member, by its id.
    let key = AnnotationDescriptor::of_type(AnnotationType::find(&ast, "key").unwrap());
    builder.apply_annotation_to_member(1, &key).unwrap();
    let built = builder.build().unwrap();
    assert!(
        built
            .get_member_by_name("b")
            .unwrap()
            .descriptor()
            .unwrap()
            .is_key
    );
    assert!(
        !built
            .get_member_by_name("a")
            .unwrap()
            .descriptor()
            .unwrap()
            .is_key
    );
    // No member of that id.
    assert!(matches!(
        builder.apply_annotation_to_member(99, &key).unwrap_err(),
        DynamicError::BadParameter { .. }
    ));
    // A nil type and an inconsistent descriptor.
    assert!(
        builder
            .apply_annotation(&AnnotationDescriptor::new())
            .is_err()
    );
    let mut bad = AnnotationDescriptor::of_type(AnnotationType::find(&ast, "Widgets").unwrap());
    bad.set_value("gadgets", "1").unwrap();
    assert!(builder.apply_annotation(&bad).is_err());
}

/// §7.5.2.2.8: a builder for a type identical to the one a TypeObject
/// describes; a MINIMAL object has no names and is refused.
#[test]
fn a_type_is_created_from_a_type_object() {
    use zerodds_types::resolve::TypeRegistry;
    use zerodds_types::type_object::TypeObject;

    let ast = model("module m { @appendable struct Plain { @key long id; double v; }; };");
    let original = DynamicType::find(&ast, "m::Plain").unwrap();
    let complete = zerodds_types::from_ast::complete_type_object(&ast, original.id()).unwrap();
    let registry = TypeRegistry::new();
    let factory = DynamicTypeBuilderFactory::get_instance();
    let mut builder = factory
        .create_type_w_type_object(&TypeObject::Complete(complete), &registry)
        .unwrap();
    let built = builder.build().unwrap();
    assert_eq!(built.qualified_name(), "m::Plain");
    assert_eq!(built.member_count(), 2);
    assert!(
        built
            .get_member_by_name("id")
            .unwrap()
            .descriptor()
            .unwrap()
            .is_key
    );
    // Changes to the builder do not reach the original, and the builder
    // can be changed.
    let long = factory.get_primitive_type(TypeKind::Int32).unwrap();
    builder.add_member(&member("extra", long)).unwrap();
    assert_eq!(builder.get_member_count().unwrap(), 3);
    assert_eq!(original.member_count(), 2);

    let minimal = zerodds_types::from_ast::minimal_type_object(&ast, original.id()).unwrap();
    let error = factory
        .create_type_w_type_object(&TypeObject::Minimal(minimal), &registry)
        .err()
        .expect("a minimal object has no names");
    assert!(
        matches!(error, DynamicError::MinimalHasNoNames { .. }),
        "{error}"
    );
}

/// §7.5.2.2.6 with the kind `ANNOTATION_TYPE`: an annotation type is
/// built, not declared in IDL, and applied from its descriptor.
#[test]
fn an_annotation_type_is_built_and_applied() {
    let factory = DynamicTypeBuilderFactory::get_instance();
    let long = factory.get_primitive_type(TypeKind::Int32).unwrap();
    // `create_type` refuses the annotation kind and names the operation.
    let error = factory
        .create_type(&descriptor(TypeKind::Annotation, "Scaled"))
        .err()
        .expect("refused");
    assert!(
        error.to_string().contains("create_annotation_type"),
        "{error}"
    );

    let mut annotation = factory.create_annotation_type("Scaled").unwrap();
    assert_eq!(annotation.get_name(), "Scaled");
    annotation.add_member("scale", &long, Some("10")).unwrap();
    // Refused: a duplicate, a type an attribute cannot have, a default that
    // is none of the type's, an empty name, a built-in's name.
    assert!(annotation.add_member("scale", &long, None).is_err());
    assert!(annotation.add_member("bad", &long, Some("ten")).is_err());
    assert!(annotation.add_member("", &long, None).is_err());
    let structure = factory
        .create_type(&descriptor(TypeKind::Structure, "S"))
        .unwrap()
        .build()
        .unwrap();
    assert!(annotation.add_member("nested", &structure, None).is_err());
    assert!(factory.create_annotation_type("key").is_err());
    assert!(factory.create_annotation_type("").is_err());

    // The descriptor form of Table 55: a consistent annotation member is
    // taken, one with a property an annotation member lacks is not.
    let mut as_descriptor = member("offset", long.clone());
    as_descriptor.default_value = "0".to_string();
    annotation.add_member_descriptor(&as_descriptor).unwrap();
    let mut keyed = member("flag", long.clone());
    keyed.is_key = true;
    assert!(annotation.add_member_descriptor(&keyed).is_err());
    let mut optional = member("flag", long.clone());
    optional.is_optional = true;
    assert!(annotation.add_member_descriptor(&optional).is_err());

    let built = annotation.build().unwrap();
    assert_eq!(built.name(), "Scaled");
    assert_eq!(built.kind(), TypeKind::Annotation);
    assert!(!built.is_builtin());
    let attributes = built.attributes();
    assert_eq!(attributes[0].0, "scale");
    assert_eq!(attributes[1].0, "offset");

    // Applied to a member and to the type; read back with the values.
    let mut descriptor_of = AnnotationDescriptor::of_type(built.clone());
    descriptor_of.set_value("scale", "100").unwrap();
    assert!(descriptor_of.is_consistent());
    let mut builder = factory
        .create_type(&descriptor(TypeKind::Structure, "demo::Measured"))
        .unwrap();
    builder.add_member(&member("reading", long)).unwrap();
    builder.apply_annotation(&descriptor_of).unwrap();
    builder
        .apply_annotation_to_member(0, &descriptor_of)
        .unwrap();
    let type_ = builder.build().unwrap();
    let on_type = type_
        .annotations()
        .into_iter()
        .find(|d| d.annotation_type().unwrap().name() == "Scaled")
        .expect("applied to the type");
    assert_eq!(on_type.get_value("scale").unwrap(), "100");
    let on_member = type_
        .get_member_by_name("reading")
        .unwrap()
        .annotations()
        .into_iter()
        .find(|d| d.annotation_type().unwrap().name() == "Scaled")
        .expect("applied to the member");
    assert!(on_member.equals(&on_type));
}

/// §7.5.2.8.4 and §7.5.2.6.3: types and members are equal only if their
/// annotations are; the lists are compared in any order, and the
/// annotations that restate a compared property are not compared twice.
#[test]
fn equals_includes_the_applied_annotations() {
    let factory = DynamicTypeBuilderFactory::get_instance();
    let long = factory.get_primitive_type(TypeKind::Int32).unwrap();
    let mut annotation = factory.create_annotation_type("Tag").unwrap();
    annotation.add_member("value", &long, Some("1")).unwrap();
    let tag = annotation.build().unwrap();
    let make = |on_type: Option<&str>, on_member: Option<&str>| {
        let mut builder = factory
            .create_type(&descriptor(TypeKind::Structure, "demo::T"))
            .unwrap();
        builder.add_member(&member("a", long.clone())).unwrap();
        for (value, member_id) in [(on_type, None), (on_member, Some(0))] {
            let Some(value) = value else { continue };
            let mut d = AnnotationDescriptor::of_type(tag.clone());
            d.set_value("value", value).unwrap();
            match member_id {
                None => builder.apply_annotation(&d).unwrap(),
                Some(id) => builder.apply_annotation_to_member(id, &d).unwrap(),
            }
        }
        builder.build().unwrap()
    };
    let plain = make(None, None);
    let on_type = make(Some("2"), None);
    let on_member = make(None, Some("2"));
    assert!(plain.equals(&make(None, None)));
    assert!(on_type.equals(&make(Some("2"), None)));
    assert!(!plain.equals(&on_type), "an annotation on the type");
    assert!(!plain.equals(&on_member), "an annotation on a member");
    assert!(!on_type.equals(&on_member));
    assert!(!on_type.equals(&make(Some("3"), None)), "another value");
    // What `equals` leaves out of account is the annotations alone.
    assert!(plain.equals_ignoring_annotations(&on_type));
    assert!(plain.equals_ignoring_annotations(&on_member));
    // The member compares by its annotations too.
    let (a, b) = (
        plain.get_member_by_name("a").unwrap(),
        on_member.get_member_by_name("a").unwrap(),
    );
    assert!(!a.equals(&b));
    // Order does not matter: two annotations applied in either order.
    let two = |first: &str, second: &str| {
        let mut builder = factory
            .create_type(&descriptor(TypeKind::Structure, "demo::T"))
            .unwrap();
        builder.add_member(&member("a", long.clone())).unwrap();
        for value in [first, second] {
            let mut d = AnnotationDescriptor::of_type(tag.clone());
            d.set_value("value", value).unwrap();
            builder.apply_annotation(&d).unwrap();
        }
        builder.build().unwrap()
    };
    assert!(two("1", "2").equals(&two("2", "1")));
    assert!(!two("1", "2").equals(&two("1", "3")));
}
