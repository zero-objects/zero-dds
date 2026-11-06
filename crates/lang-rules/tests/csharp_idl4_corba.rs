// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL4 to C# Language Mapping 1.0 Annex A.1, the CORBA platform, and
//! §7.9 to §7.13, components, homes, ports, connectors and template
//! module instances through their intermediate IDL (whose `Components`
//! module names `Object`, so these run on the CORBA platform).
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

#[path = "csharp-idl4/support.rs"]
mod support;

use support::{check_program, compile_and_run, corba_idl, corba_module, cs, file};
use zerodds_lang_rules::emit::idl4_csharp::{CsFile, Naming, Options, Platform};

fn corba(naming: Naming) -> Options {
    Options {
        naming,
        platform: Platform::Corba,
        ..Options::default()
    }
}

/// A.1.2: the CORBA module is the `Corba` namespace under the IDL naming
/// scheme; `CORBA::TypeCode` is the runtime's class; without
/// `corba_module` the module is the ORB's and is not written.
#[test]
fn idl4csharp_a_1_2_the_corba_module_is_the_corba_namespace_under_the_idl_naming_scheme() {
    let source = format!(
        "{}\nmodule app {{ struct S {{ CORBA::PolicyList policies; }}; }};",
        corba_idl()
    );
    let with = cs(
        &source,
        &Options {
            corba_module: true,
            ..corba(Naming::DotNet)
        },
    );
    let policy = file(&with, "Corba/IPolicyOperations.cs");
    assert!(policy.contains("namespace Corba"), "{policy}");
    assert!(policy.contains("policy_type"), "{policy}");
    assert!(
        with.iter()
            .all(|f| f.path != "Corba/TypeCode.cs" && f.path != "Corba/ITypeCode.cs")
    );
    let without = cs(&source, &corba(Naming::DotNet));
    assert!(without.iter().all(|f| !f.path.starts_with("Corba/")));
    file(&without, "App/S.cs");
}

/// A.1.1 to A.1.4: the classes the annex defines, declaration by
/// declaration, and the behaviour of CORBA 3.3 part 1 §8.11.1 for type
/// codes and of the any's insert and extract operations.
#[test]
fn idl4csharp_a_1_typecode_object_any_and_user_exception_are_the_annexs_classes() {
    let files = corba_module();
    let check = check_program(
        r#"
        Ok(typeof(Corba.UserException).BaseType == typeof(System.Exception), "UserException : System.Exception");
        var tc = typeof(Corba.TypeCode);
        Ok(typeof(Corba.TypeCode.Bounds).BaseType == typeof(Corba.UserException), "TypeCode.Bounds");
        Ok(typeof(Corba.TypeCode.BadKind).BaseType == typeof(Corba.UserException), "TypeCode.BadKind");
        Returns(tc, "equal", typeof(bool), tc);
        Returns(tc, "equivalent", typeof(bool), tc);
        Returns(tc, "get_compact_typecode", tc);
        Returns(tc, "kind", typeof(Corba.TCKind));
        Returns(tc, "id", typeof(string));
        Returns(tc, "name", typeof(string));
        Returns(tc, "member_count", typeof(uint));
        Returns(tc, "member_name", typeof(string), typeof(uint));
        Returns(tc, "member_type", tc, typeof(uint));
        Returns(tc, "member_label", typeof(Corba.Any), typeof(uint));
        Returns(tc, "discriminator_type", tc);
        Returns(tc, "default_index", typeof(int));
        Returns(tc, "length", typeof(uint));
        Returns(tc, "content_type", tc);
        Returns(tc, "fixed_digits", typeof(ushort));
        Returns(tc, "fixed_scale", typeof(short));
        Returns(tc, "member_visibility", typeof(short), typeof(uint));
        Returns(tc, "type_modifier", typeof(short));
        Returns(tc, "concrete_base_type", tc);
        Ok(typeof(Corba.IObject).IsInterface && typeof(Corba.IObjectOperations).IsAssignableFrom(typeof(Corba.IObject)),
            "IObject : IObjectOperations");
        Ok(typeof(Corba.IObject).IsAssignableFrom(typeof(Corba.Object)), "Object : IObject");
        Returns(typeof(Corba.IObjectOperations), "get_interface", typeof(Corba.IInterfaceDef));
        Returns(typeof(Corba.IObjectOperations), "is_a", typeof(bool), typeof(string));
        Returns(typeof(Corba.IObjectOperations), "get_orb", typeof(Corba.IORB));
        Property(typeof(Corba.Any), "type", tc, false);
        Returns(typeof(Corba.Any), "insert_ulonglong", typeof(void), typeof(ulong));
        Returns(typeof(Corba.Any), "extract_ushort", typeof(ushort));
        Returns(typeof(Corba.Any), "insert_object", typeof(void), typeof(Corba.Object));

        var tlong = Corba.TypeCode.get_primitive_tc(Corba.TCKind.tk_long);
        var members = new Omg.Types.SequenceList<Corba.StructMember>();
        members.Add(new Corba.StructMember("x", tlong, null));
        members.Add(new Corba.StructMember("label", Corba.TypeCode.create_string_tc(0), null));
        var point = Corba.TypeCode.create_struct_tc("IDL:Point:1.0", "Point", members);
        Ok(point.kind() == Corba.TCKind.tk_struct && point.member_count() == 2u && point.member_name(1) == "label",
            "a structure's type code");
        Throws<Corba.TypeCode.Bounds>(() => point.member_name(2), "Bounds");
        Throws<Corba.TypeCode.BadKind>(() => point.length(), "BadKind");
        var alias = Corba.TypeCode.create_alias_tc("IDL:Count:1.0", "Count", tlong);
        Ok(alias.equivalent(tlong) && !alias.equal(tlong), "an alias is equivalent, not equal");
        var compact = point.get_compact_typecode();
        Ok(compact.name() == "" && compact.member_name(0) == "" && compact.equivalent(point), "compact");
        var node = new Omg.Types.SequenceList<Corba.StructMember>();
        node.Add(new Corba.StructMember("children",
            Corba.TypeCode.create_sequence_tc(0, Corba.TypeCode.create_recursive_tc("IDL:Node:1.0")), null));
        var tree = Corba.TypeCode.create_struct_tc("IDL:Node:1.0", "Node", node);
        Ok(tree.member_type(0).content_type().name() == "Node", "a recursive type code");
        Throws<Corba.BAD_TYPECODE>(() => Corba.TypeCode.create_recursive_tc("IDL:X:1.0").kind(), "an unbound placeholder");

        var a = new Corba.Any();
        a.insert_ushort(65535);
        Ok(a.type.kind() == Corba.TCKind.tk_ushort && a.extract_ushort() == 65535, "insert and extract");
        Throws<Corba.BAD_OPERATION>(() => a.extract_short(), "another type");
        var b = new Corba.Any();
        b.insert_ushort(65535);
        Ok(a.equal(b), "equal anys");
"#,
    );
    compile_and_run("corba-runtime", &files, &check);
}

/// A.1.1 and A.1.5: an exception extends `Corba.UserException`; an
/// interface is `I<Name>` extending `Corba.IObject`, and its class
/// derives from `Corba.Object`.
#[test]
fn idl4csharp_a_1_1_and_a_1_5_exceptions_and_interfaces_on_the_corba_platform() {
    let source = format!(
        "{}\nexception AnException {{ long error_code; }};\n\
         interface FullInterface {{ struct S {{ long a_long; }}; void op1(in S s_in) raises (AnException); \
         attribute any value; }};",
        corba_idl()
    );
    for (naming, error_code, tag) in [
        (Naming::Idl, "error_code", "idl"),
        (Naming::DotNet, "ErrorCode", "dotnet"),
    ] {
        let mut files: Vec<CsFile> = corba_module();
        files.extend(cs(&source, &corba(naming)));
        let check = check_program(&format!(
            r#"
        Ok(typeof(AnException).BaseType == typeof(Corba.UserException), "AnException : Corba.UserException");
        Ok(new AnException(3).{error_code} == 3, "the all values constructor");
        Ok(typeof(Corba.IObject).IsAssignableFrom(typeof(IFullInterface)), "IFullInterface : Corba.IObject");
        Ok(typeof(IFullInterfaceOperations).IsAssignableFrom(typeof(IFullInterface)), "and IFullInterfaceOperations");
        Ok(typeof(FullInterface).BaseType == typeof(Corba.Object), "FullInterface : Corba.Object");
"#
        ));
        compile_and_run(&format!("corba-interface-{tag}"), &files, &check);
    }
}

fn components_idl() -> String {
    let components = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/java-idl4/Components.idl"),
    )
    .expect("tests/java-idl4/Components.idl");
    format!("{}\n{components}\n", corba_idl())
}

const SHOP: &str = r"
module Shop {
    interface Pricing { long price(in string item); };
    eventtype Sold { public long amount; };
    porttype Till { provides Pricing pricing; uses Pricing fallback; };
    component Cashier {
        provides Pricing prices;
        uses multiple Pricing others;
        publishes Sold sales;
        consumes Sold returns;
        port Till till;
        attribute long lane;
    };
    home CashierHome manages Cashier { factory open(in long lane); };
    connector Link { provides Pricing out_price; mirrorport Till other; };
    module Buffer<typename T, const long Capacity> {
        struct Slot { T value; };
        const long Size = Capacity;
    };
    module Buffer<long, 4> LongBuffer;
};
";

/// §7.9, §7.10, §7.12 and §7.13: the equivalent IDL of CCM 4.0 §6 in C#,
/// and a template module instance as a namespace of its own.
#[test]
fn idl4csharp_7_9_to_7_13_components_homes_connectors_and_template_instances() {
    let mut files = corba_module();
    files.extend(cs(
        &format!("{}{SHOP}", components_idl()),
        &corba(Naming::Idl),
    ));
    let check = check_program(
        r#"
        var c = typeof(Shop.ICashierOperations);
        Ok(typeof(Components.ICCMObject).IsAssignableFrom(typeof(Shop.ICashier)), "ICashier : Components.ICCMObject");
        Returns(c, "provide_prices", typeof(Shop.IPricing));
        Returns(c, "connect_others", typeof(Components.Cookie), typeof(Shop.IPricing));
        Returns(c, "subscribe_sales", typeof(Components.Cookie), typeof(Shop.ISoldConsumer));
        Returns(c, "get_consumer_returns", typeof(Shop.ISoldConsumer));
        Returns(c, "provide_till_pricing", typeof(Shop.IPricing));
        Returns(c, "connect_till_fallback", typeof(void), typeof(Shop.IPricing));
        Property(c, "lane", typeof(int), true);
        Returns(typeof(Shop.ISoldConsumerOperations), "push_Sold", typeof(void), typeof(Shop.Sold));
        Ok(typeof(Components.ICCMHome).IsAssignableFrom(typeof(Shop.ICashierHomeExplicit)), "the explicit home");
        Returns(typeof(Shop.ICashierHomeExplicitOperations), "open", typeof(Shop.ICashier), typeof(int));
        Returns(typeof(Shop.ICashierHomeImplicitOperations), "create", typeof(Shop.ICashier));
        Returns(typeof(Shop.ILinkOperations), "connect_other_pricing", typeof(void), typeof(Shop.IPricing));
        Returns(typeof(Shop.ILinkOperations), "provide_other_fallback", typeof(Shop.IPricing));
        Property(typeof(Shop.LongBuffer.Slot), "value", typeof(int), true);
        Ok(Shop.LongBuffer.Size.Value == 4, "the instance's constant");
"#,
    );
    compile_and_run("components", &files, &check);
}
