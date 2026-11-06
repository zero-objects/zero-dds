// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL4 to Java Language Mapping 1.0 Annex A.1, the CORBA platform:
//! exceptions, `TypeCode`, `Object` and `any`, with the Java of the CORBA
//! module (`orb.idl` and its pseudo IDL) compiled together with the
//! runtime classes the annex defines.
//!
//! The check programs hold the annex's class and method declarations and
//! CORBA 3.3 part 1 §8.11.1's rules for type codes; the compiled classes
//! are inspected by reflection and exercised.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

#[path = "java-idl4/support.rs"]
mod support;

use support::{check_program, compile_and_run, corba_module, file, java};
use zerodds_lang_rules::emit::idl4_java::{JavaFile, Naming, Options, Platform};

fn corba(naming: Naming) -> Options {
    Options {
        naming,
        platform: Platform::Corba,
        ..Options::default()
    }
}

/// A.1.2: the types of the CORBA module are mapped under the IDL naming
/// scheme into `org.omg.CORBA`, whatever scheme the rest is mapped with,
/// and `CORBA::TypeCode` is the annex's class, not the interface its IDL
/// declares. Without `corba_module` the module is the ORB's and is not
/// written.
#[test]
fn idl4java_a_1_2_the_corba_module_is_org_omg_corba_under_the_idl_naming_scheme() {
    let source = format!(
        "{}\nmodule app {{ struct S {{ CORBA::PolicyList policies; }}; }};",
        support::corba_idl()
    );
    let with = java(
        &source,
        &Options {
            corba_module: true,
            ..corba(Naming::Java)
        },
    );
    let policy = file(&with, "org/omg/CORBA/Policy.java");
    assert!(policy.contains("package org.omg.CORBA;"), "{policy}");
    assert!(policy.contains("get_policy_type()"), "{policy}");
    assert!(
        with.iter().all(|f| f.path != "org/omg/CORBA/TypeCode.java"),
        "TypeCode is the runtime's class"
    );
    let s = file(&with, "app/S.java");
    assert!(s.contains("getPolicies()"), "{s}");
    let without = java(&source, &corba(Naming::Java));
    assert!(
        without.iter().all(|f| !f.path.starts_with("org/")),
        "{:?}",
        without.iter().map(|f| &f.path).collect::<Vec<_>>()
    );
    file(&without, "app/S.java");
}

/// A.1.2, A.1.3, A.1.4 and A.1.1: the four classes the annex defines,
/// declaration by declaration, and the behaviour of CORBA 3.3 part 1
/// §8.11.1 for type codes and of the any's insert and extract operations.
#[test]
fn idl4java_a_1_typecode_object_any_and_user_exception_are_the_annexs_classes() {
    let files = corba_module();
    let check = check_program(
        r#"
        Class<?> tc = org.omg.CORBA.TypeCode.class;
        Class<?> any = org.omg.CORBA.Any.class;
        Class<?> obj = org.omg.CORBA.Object.class;
        Class<?> kind = org.omg.CORBA.TCKind.class;
        // A.1.1
        check(org.omg.CORBA.UserException.class.getSuperclass() == RuntimeException.class,
            "UserException extends java.lang.RuntimeException");
        // A.1.2, the declaration
        check(org.omg.CORBA.TypeCode.Bounds.class.getSuperclass() == org.omg.CORBA.UserException.class,
            "TypeCode.Bounds extends UserException");
        check(org.omg.CORBA.TypeCode.BadKind.class.getSuperclass() == org.omg.CORBA.UserException.class,
            "TypeCode.BadKind extends UserException");
        returns(tc, "equal", boolean.class, tc);
        returns(tc, "equivalent", boolean.class, tc);
        returns(tc, "get_compact_typecode", tc);
        returns(tc, "kind", kind);
        returns(tc, "id", String.class);
        returns(tc, "name", String.class);
        returns(tc, "member_count", int.class);
        returns(tc, "member_name", String.class, int.class);
        returns(tc, "member_type", tc, int.class);
        returns(tc, "member_label", any, int.class);
        returns(tc, "discriminator_type", tc);
        returns(tc, "default_index", int.class);
        returns(tc, "length", int.class);
        returns(tc, "content_type", tc);
        returns(tc, "fixed_digits", short.class);
        returns(tc, "fixed_scale", short.class);
        returns(tc, "member_visibility", short.class, int.class);
        returns(tc, "type_modifier", short.class);
        returns(tc, "concrete_base_type", tc);
        check(kind.isEnum(), "TCKind is a Java enum (§7.2.4.3.3)");
        check(org.omg.CORBA.TCKind.valueOf("tk_event").getValue() == 36,
            "tk_event is the 37th enumerator of TCKind");
        // A.1.3: Object is an interface with CORBA 3.3 §8.3's operations
        check(obj.isInterface(), "Object is an interface");
        returns(obj, "get_interface", org.omg.CORBA.InterfaceDef.class);
        returns(obj, "is_nil", boolean.class);
        returns(obj, "duplicate", obj);
        returns(obj, "release", void.class);
        returns(obj, "is_a", boolean.class, String.class);
        returns(obj, "non_existent", boolean.class);
        returns(obj, "is_equivalent", boolean.class, obj);
        returns(obj, "hash", int.class, int.class);
        returns(obj, "create_request", void.class, org.omg.CORBA.Context.class, String.class,
            org.omg.CORBA.NVList.class, org.omg.type.Holder.class, org.omg.type.Holder.class, int.class);
        returns(obj, "get_policy", org.omg.CORBA.Policy.class, int.class);
        returns(obj, "get_domain_managers", java.util.List.class);
        returns(obj, "set_policy_overrides", obj, java.util.List.class, org.omg.CORBA.SetOverrideType.class);
        returns(obj, "get_client_policy", org.omg.CORBA.Policy.class, int.class);
        returns(obj, "get_policy_overrides", java.util.List.class, org.omg.type.IntegerSeq.class);
        returns(obj, "validate_connection", boolean.class, org.omg.type.Holder.class);
        returns(obj, "get_component", obj);
        returns(obj, "repository_id", String.class);
        returns(obj, "get_orb", org.omg.CORBA.ORB.class);
        // A.1.4: Any
        returns(any, "equal", boolean.class, any);
        returns(any, "type", tc);
        returns(any, "type", void.class, tc);
        String[][] pairs = {
            {"short", "short"}, {"long", "int"}, {"longlong", "long"}, {"ushort", "short"},
            {"ulong", "int"}, {"ulonglong", "long"}, {"float", "float"}, {"double", "double"},
            {"boolean", "boolean"}, {"char", "char"}, {"wchar", "char"}, {"octet", "byte"}};
        java.util.Map<String, Class<?>> prim = new java.util.HashMap<>();
        prim.put("short", short.class); prim.put("int", int.class); prim.put("long", long.class);
        prim.put("float", float.class); prim.put("double", double.class);
        prim.put("boolean", boolean.class); prim.put("char", char.class); prim.put("byte", byte.class);
        for (String[] pair : pairs) {
            returns(any, "insert_" + pair[0], void.class, prim.get(pair[1]));
            returns(any, "extract_" + pair[0], prim.get(pair[1]));
        }
        returns(any, "insert_any", void.class, any);
        returns(any, "extract_any", any);
        returns(any, "insert_object", void.class, obj);
        returns(any, "extract_object", obj);

        // §8.11.1 behaviour
        org.omg.CORBA.TypeCode tlong = org.omg.CORBA.TypeCode.get_primitive_tc(org.omg.CORBA.TCKind.tk_long);
        org.omg.CORBA.TypeCode tstr = org.omg.CORBA.TypeCode.create_string_tc(0);
        java.util.List<org.omg.CORBA.StructMember> members = new java.util.ArrayList<>();
        members.add(new org.omg.CORBA.StructMember("x", tlong, null));
        members.add(new org.omg.CORBA.StructMember("label", tstr, null));
        final org.omg.CORBA.TypeCode point = org.omg.CORBA.TypeCode.create_struct_tc("IDL:Point:1.0", "Point", members);
        check(point.kind() == org.omg.CORBA.TCKind.tk_struct, "kind");
        check(point.id().equals("IDL:Point:1.0") && point.name().equals("Point"), "id and name");
        check(point.member_count() == 2 && point.member_name(1).equals("label"), "members");
        check(point.member_type(0).equal(tlong), "member_type");
        throwsIt(org.omg.CORBA.TypeCode.Bounds.class, () -> point.member_name(2), "Bounds at member_count");
        throwsIt(org.omg.CORBA.TypeCode.BadKind.class, () -> point.length(), "BadKind for length of a struct");
        throwsIt(org.omg.CORBA.TypeCode.BadKind.class, () -> tlong.id(), "BadKind for id of long");
        // equal compares names; equivalent ignores them and unwinds aliases
        java.util.List<org.omg.CORBA.StructMember> renamed = new java.util.ArrayList<>();
        renamed.add(new org.omg.CORBA.StructMember("a", tlong, null));
        renamed.add(new org.omg.CORBA.StructMember("b", tstr, null));
        org.omg.CORBA.TypeCode anon1 = org.omg.CORBA.TypeCode.create_struct_tc("", "P1", members);
        org.omg.CORBA.TypeCode anon2 = org.omg.CORBA.TypeCode.create_struct_tc("", "P2", renamed);
        check(!anon1.equal(anon2), "equal compares the names");
        check(anon1.equivalent(anon2), "equivalent compares the structure when an id is empty");
        org.omg.CORBA.TypeCode other = org.omg.CORBA.TypeCode.create_struct_tc("IDL:Other:1.0", "Point", members);
        check(!point.equivalent(other), "two non-empty ids that differ are not equivalent");
        org.omg.CORBA.TypeCode alias = org.omg.CORBA.TypeCode.create_alias_tc("IDL:Count:1.0", "Count", tlong);
        check(alias.equivalent(tlong) && !alias.equal(tlong), "an alias is equivalent, not equal");
        check(alias.content_type().equal(tlong), "content_type of an alias");
        org.omg.CORBA.TypeCode compact = point.get_compact_typecode();
        check(compact.name().isEmpty() && compact.member_name(0).isEmpty(), "compact strips the names");
        check(compact.id().equals("IDL:Point:1.0") && compact.equivalent(point), "compact keeps the id");
        // a union: the default member's label is the zero octet
        org.omg.CORBA.Any one = new org.omg.CORBA.Any();
        one.insert_long(1);
        org.omg.CORBA.Any dflt = new org.omg.CORBA.Any();
        dflt.insert_octet((byte) 0);
        java.util.List<org.omg.CORBA.UnionMember> branches = new java.util.ArrayList<>();
        branches.add(new org.omg.CORBA.UnionMember("n", one, tlong, null));
        branches.add(new org.omg.CORBA.UnionMember("s", dflt, tstr, null));
        org.omg.CORBA.TypeCode u = org.omg.CORBA.TypeCode.create_union_tc("IDL:U:1.0", "U", tlong, branches);
        check(u.default_index() == 1, "default_index is the zero-octet label's");
        check(u.member_label(0).extract_long() == 1, "member_label");
        check(u.discriminator_type().equal(tlong), "discriminator_type");
        // a recursive structure through a sequence
        java.util.List<org.omg.CORBA.StructMember> node = new java.util.ArrayList<>();
        node.add(new org.omg.CORBA.StructMember("value", tlong, null));
        node.add(new org.omg.CORBA.StructMember("children", org.omg.CORBA.TypeCode.create_sequence_tc(0,
            org.omg.CORBA.TypeCode.create_recursive_tc("IDL:Node:1.0")), null));
        org.omg.CORBA.TypeCode tree = org.omg.CORBA.TypeCode.create_struct_tc("IDL:Node:1.0", "Node", node);
        check(tree.member_type(1).content_type().name().equals("Node"), "the placeholder is bound");
        check(tree.equal(tree.member_type(1).content_type()), "and is the structure");
        check(tree.equivalent(tree.get_compact_typecode()), "a cycle compares");
        final org.omg.CORBA.TypeCode loose = org.omg.CORBA.TypeCode.create_recursive_tc("IDL:None:1.0");
        throwsIt(org.omg.CORBA.BAD_TYPECODE.class, () -> loose.kind(), "an unbound placeholder");
        // a value type
        java.util.List<org.omg.CORBA.ValueMember> state = new java.util.ArrayList<>();
        state.add(new org.omg.CORBA.ValueMember("v", "", "", "", tlong, null,
            org.omg.CORBA.PRIVATE_MEMBER.value));
        org.omg.CORBA.TypeCode value = org.omg.CORBA.TypeCode.create_value_tc("IDL:V:1.0", "V",
            org.omg.CORBA.VM_TRUNCATABLE.value, null, state);
        check(value.type_modifier() == 3 && value.member_visibility(0) == 0, "value modifiers");
        check(value.concrete_base_type() == null, "no concrete base is a nil type code");
        org.omg.CORBA.TypeCode fixed = org.omg.CORBA.TypeCode.create_fixed_tc((short) 9, (short) 2);
        check(fixed.fixed_digits() == 9 && fixed.fixed_scale() == 2, "fixed");

        // A.1.4 behaviour: insert sets the type code, extract checks it
        final org.omg.CORBA.Any a = new org.omg.CORBA.Any();
        a.insert_ulonglong(-1L);
        check(a.type().kind() == org.omg.CORBA.TCKind.tk_ulonglong, "insert sets the kind");
        check(a.extract_ulonglong() == -1L, "extract returns it");
        throwsIt(org.omg.CORBA.BAD_OPERATION.class, () -> a.extract_longlong(), "another type");
        a.type(alias);
        throwsIt(org.omg.CORBA.BAD_OPERATION.class, () -> a.extract_long(), "a type without a value");
        org.omg.CORBA.Any b = new org.omg.CORBA.Any();
        b.insert_wchar('ä');
        org.omg.CORBA.Any c = new org.omg.CORBA.Any();
        c.insert_wchar('ä');
        check(b.equal(c), "equal anys");
        c.insert_char('ä');
        check(!b.equal(c), "char and wchar differ");
        org.omg.CORBA.Any outer = new org.omg.CORBA.Any();
        outer.insert_any(b);
        check(outer.extract_any().extract_wchar() == 'ä', "an any in an any");
"#,
    );
    compile_and_run("corba-runtime", &files, &check);
}

/// A.1.1's example under both naming schemes: an exception extends
/// `org.omg.CORBA.UserException` and is otherwise §7.4.1's class; `any`,
/// `Object` and `CORBA::TypeCode` are the annex's classes in a member.
#[test]
fn idl4java_a_1_1_an_exception_extends_user_exception_and_members_name_the_annexs_classes() {
    let source = format!(
        "{}\nexception AnException {{ long error_code; }};\n\
         struct Carrier {{ any a; Object o; CORBA::TypeCode t; }};\n\
         interface Service {{ void op() raises (AnException); }};",
        support::corba_idl()
    );
    for (naming, getter, setter, tag) in [
        (Naming::Idl, "get_error_code", "set_error_code", "idl"),
        (Naming::Java, "getErrorCode", "setErrorCode", "java"),
    ] {
        let mut files: Vec<JavaFile> = corba_module();
        files.extend(java(&source, &corba(naming)));
        let check = check_program(&format!(
            r#"
        check(AnException.class.getSuperclass() == org.omg.CORBA.UserException.class,
            "AnException extends org.omg.CORBA.UserException");
        AnException e = new AnException(7);
        check(e.{getter}() == 7, "the all values constructor and the accessor");
        e.{setter}(8);
        check(e.{getter}() == 8, "the modifier");
        check(new AnException().{getter}() == 0, "the default constructor");
        Class<?>[] thrown = method(Service.class, "op").getExceptionTypes();
        check(thrown.length == 1 && thrown[0] == AnException.class, "throws AnException");
        String prefix = "{tag}".equals("idl") ? "get_" : "get";
        returns(Carrier.class, prefix + ("{tag}".equals("idl") ? "a" : "A"), org.omg.CORBA.Any.class);
        returns(Carrier.class, prefix + ("{tag}".equals("idl") ? "o" : "O"), org.omg.CORBA.Object.class);
        returns(Carrier.class, prefix + ("{tag}".equals("idl") ? "t" : "T"), org.omg.CORBA.TypeCode.class);
"#
        ));
        compile_and_run(&format!("corba-exception-{tag}"), &files, &check);
    }
}
