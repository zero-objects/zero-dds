// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL4 to C# Language Mapping 1.0 (OMG formal/2021-07-01), clauses 7 and
//! 8, on the specification's own examples.
//!
//! Each test generates the C# of an IDL file, compiles it with Roslyn
//! together with the `Omg.Types` runtime, and runs a check program whose
//! expectations are the clause's text: the type, property and method
//! names and types its examples print, the exceptions its sentences name.
//! The compiled types are inspected by reflection and exercised.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

#[path = "csharp-idl4/support.rs"]
mod support;

use support::{check_program, compile_and_run, cs, file, model};
use zerodds_lang_rules::emit::idl4_csharp::{Naming, Options, generate};

fn idl_naming() -> Options {
    Options::default()
}

fn dotnet_naming() -> Options {
    Options {
        naming: Naming::DotNet,
        ..Options::default()
    }
}

const MY_STRUCT: &str = "struct MyStruct {
    long a_long;
    short a_short;
    long a_long_array[10];
    sequence<long> a_long_seq;
    string a_string;
};";

/// §7.2.4.3.1's example under both naming schemes: a public class with a
/// property per member — a getter only for the sequence — the default,
/// copy and all values constructors, and `IEquatable<MyStruct>`.
#[test]
fn idl4csharp_7_2_4_3_1_a_structure_is_a_class_with_properties_three_constructors_and_iequatable() {
    for (options, tag, long, short, array, seq, string) in [
        (
            idl_naming(),
            "idl",
            "a_long",
            "a_short",
            "a_long_array",
            "a_long_seq",
            "a_string",
        ),
        (
            dotnet_naming(),
            "dotnet",
            "ALong",
            "AShort",
            "ALongArray",
            "ALongSeq",
            "AString",
        ),
    ] {
        let files = cs(MY_STRUCT, &options);
        let text = file(&files, "MyStruct.cs");
        assert!(
            text.contains("public class MyStruct : global::System.IEquatable<MyStruct>"),
            "{text}"
        );
        let check = check_program(&format!(
            r#"
        var t = typeof(MyStruct);
        Ok(typeof(System.IEquatable<MyStruct>).IsAssignableFrom(t), "IEquatable<MyStruct>");
        Property(t, "{long}", typeof(int), true);
        Property(t, "{short}", typeof(short), true);
        Property(t, "{array}", typeof(int[]), true);
        Property(t, "{seq}", typeof(Omg.Types.ISequence<int>), false);
        Property(t, "{string}", typeof(string), true);
        Ok(t.GetConstructor(System.Type.EmptyTypes) != null, "the default constructor");
        Ok(t.GetConstructor(new[] {{ t }}) != null, "the copy constructor");
        Ok(t.GetConstructor(new[] {{ typeof(int), typeof(short), typeof(int[]), typeof(Omg.Types.ISequence<int>), typeof(string) }}) != null,
            "the all values constructor");
        var d = new MyStruct();
        Ok(d.{array}.Length == 10, "the array at its declared size");
        Ok(d.{seq}.Count == 0, "the sequence empty");
        Ok(d.{string} == string.Empty, "the string empty");
        d.{long} = 7;
        d.{seq}.Add(1);
        var copy = new MyStruct(d);
        Ok(copy.Equals(d) && copy.GetHashCode() == d.GetHashCode(), "a copy is equal");
        copy.{seq}.Add(2);
        copy.{array}[0] = 9;
        Ok(d.{seq}.Count == 1 && d.{array}[0] == 0, "the copy is deep");
        Ok(!copy.Equals(d), "a changed copy is not equal");
        Throws<System.ArgumentOutOfRangeException>(() => d.{array} = new int[3], "§7.2.4.4: an array of another size");
"#
        ));
        compile_and_run(&format!("struct-{tag}"), &files, &check);
    }
}

const A_UNION: &str = "union AUnion switch (octet) {
    case 1: long a_long;
    case 2:
    case 3: short a_short;
    case 4: sequence<long> a_long_seq;
    default: octet a_byte_default;
};";

/// §7.2.4.3.2's example `AUnion`: a read-only `Discriminator`, a property
/// per member whose getter raises `InvalidOperationException` for another
/// member, the setter setting the first label, `Set<Member>(value,
/// discriminator)` refusing a foreign label with `ArgumentException`, the
/// two `Set<Member>` methods of a sequence member, and the default member
/// at the first free value from zero.
#[test]
fn idl4csharp_7_2_4_3_2_a_union_is_a_class_with_a_discriminator_and_member_properties() {
    let files = cs(A_UNION, &idl_naming());
    let check = check_program(
        r#"
        var t = typeof(AUnion);
        Property(t, "Discriminator", typeof(byte), false);
        Property(t, "a_long", typeof(int), true);
        Property(t, "a_long_seq", typeof(Omg.Types.ISequence<int>), false);
        Ok(typeof(System.IEquatable<AUnion>).IsAssignableFrom(t), "IEquatable<AUnion>");
        var u = new AUnion();
        u.a_short = 5;
        Ok(u.Discriminator == 2, "the first label of a_short");
        Throws<System.InvalidOperationException>(() => { var x = u.a_long; }, "a_long is not selected");
        u.Seta_short(6, 3);
        Ok(u.Discriminator == 3 && u.a_short == 6, "Seta_short with the label 3");
        Throws<System.ArgumentException>(() => u.Seta_short(1, 1), "1 is no label of a_short");
        u.Seta_long_seq(new[] { 1, 2, 3 });
        Ok(u.Discriminator == 4 && u.a_long_seq.Count == 3, "Seta_long_seq(elements)");
        u.Seta_long_seq();
        Ok(u.a_long_seq.Count == 0, "Seta_long_seq() clears");
        u.a_byte_default = 9;
        Ok(u.Discriminator == 0, "the default member at the first free value from zero");
        var copy = new AUnion(u);
        Ok(copy.Equals(u), "the copy constructor");
"#,
    );
    compile_and_run("union", &files, &check);
}

/// §7.2.4.3.3's examples: a C# enum with the `@value`s, and under
/// `@bit_bound(6)` the underlying type `sbyte`; `short` and `long` for
/// the two wider ranges.
///
/// DDS-XTypes 1.3 §7.3.1.2.1.5 bounds an enumeration's `@bit_bound` at
/// 32, and the front end refuses a wider one in the source (asserted
/// below). The clause's `long` range is reached by writing 40 into the
/// composed model's `@bit_bound` facet, which is where the generator
/// reads it.
#[test]
fn idl4csharp_7_2_4_3_3_an_enumeration_is_a_csharp_enum_of_its_bit_bound() {
    // Each example in its own module: IDL declares enumerators in the
    // enclosing scope, so the clause's two enumerations would collide in
    // one.
    let source = "module plain { enum AnEnum { @value(1) one, @value(2) two }; };
                  module bound { @bit_bound(6) enum ABoundEnum { @value(1) one, @value(2) two }; };
                  module wider { @bit_bound(12) enum AShortEnum { a }; };
                  module widest { @bit_bound(32) enum ALongEnum { b }; };";
    let refused = zerodds_idl_compose::compose_source_with(
        "test.idl",
        "@bit_bound(40) enum E { e };",
        &zerodds_idl_parse::MemoryLoader::new(),
        zerodds_idl_compose::ComposeOptions::default(),
    );
    assert!(
        refused
            .diagnostics
            .iter()
            .any(|d| d.is_error() && d.spec_reference.contains("7.3.1.2.1.5")),
        "{:?}",
        refused.diagnostics
    );
    let mut ast = model(source);
    let (id, node) = ast
        .types()
        .find(|(_, node)| node.name == "ALongEnum")
        .map(|(id, node)| (id, node.node_id))
        .expect("ALongEnum");
    ast.facets_mut(zerodds_ast::AnyId::Type(id, node))
        .expect("facets")
        .xtypes_mut()
        .bit_bound = Some(40);
    for (options, one, plain, bound, wider, widest, tag) in [
        (
            idl_naming(),
            "one",
            "plain",
            "bound",
            "wider",
            "widest",
            "idl",
        ),
        (
            dotnet_naming(),
            "One",
            "Plain",
            "Bound",
            "Wider",
            "Widest",
            "dotnet",
        ),
    ] {
        let files = generate(&ast, &options).unwrap_or_else(|error| panic!("{error}"));
        let check = check_program(&format!(
            r#"
        Ok(typeof({plain}.AnEnum).IsEnum && (int){plain}.AnEnum.{one} == 1, "AnEnum.{one} = 1");
        Ok(System.Enum.GetUnderlyingType(typeof({plain}.AnEnum)) == typeof(int), "no @bit_bound: int");
        Ok(System.Enum.GetUnderlyingType(typeof({bound}.ABoundEnum)) == typeof(sbyte), "@bit_bound(6): sbyte");
        Ok(System.Enum.GetUnderlyingType(typeof({wider}.AShortEnum)) == typeof(short), "@bit_bound(12): short");
        Ok(System.Enum.GetUnderlyingType(typeof({widest}.ALongEnum)) == typeof(long), "@bit_bound(40): long");
"#
        ));
        compile_and_run(&format!("enum-{tag}"), &files, &check);
    }
}

/// §7.2.2 and §7.2.3.1's example `my_math`: a namespace per module
/// (`MyMath` under the .NET scheme) and a static class per constant with
/// `Value`; §7.2.3.2 and §8.1.2: the container, `Constants` or the name
/// given, and the empty name back to standalone classes.
#[test]
fn idl4csharp_7_2_2_and_7_2_3_modules_are_namespaces_and_constants_classes_or_a_container() {
    let source = r#"module my_math {
            const double PI = 3.141592;
            const double e = 2.718282;
            const string my_string = "My String Value";
        };
        @csharp_mapping(constants_container = "MathematicalConstants")
        module my_container { const double PI = 3.141592; const double e = 2.718282; };
        @csharp_mapping(constants_container = "")
        module my_properties { const float speed_of_light = 1080000000; };"#;
    let idl = cs(source, &idl_naming());
    file(&idl, "my_math/PI.cs");
    let check = check_program(
        r#"
        Ok(my_math.PI.Value == 3.141592, "my_math.PI.Value");
        Ok(my_math.e.Value == 2.718282, "my_math.e.Value");
        Ok(my_math.my_string.Value == "My String Value", "my_math.my_string.Value");
        Ok(my_container.MathematicalConstants.PI == 3.141592 && my_container.MathematicalConstants.e == 2.718282,
            "the container MathematicalConstants");
        Ok(my_properties.speed_of_light.Value == 1080000000f, "the empty name: standalone");
        Ok(typeof(my_math.PI).IsAbstract && typeof(my_math.PI).IsSealed, "a static class");
"#,
    );
    compile_and_run("constants-idl", &idl, &check);
    let dotnet = cs(source, &dotnet_naming());
    file(&dotnet, "MyMath/PI.cs");
    let check = check_program(
        r#"
        Ok(MyMath.E.Value == 2.718282 && MyMath.MyString.Value == "My String Value", "MyMath.E, MyMath.MyString");
        Ok(MyContainer.MathematicalConstants.E == 2.718282, "the container's E");
"#,
    );
    compile_and_run("constants-dotnet", &dotnet, &check);
}

/// Tables 7.2 and 7.3, §7.2.4.1.3 to §7.2.4.1.6: the C# type of every
/// basic type; `TRUE` and `FALSE` are `true` and `false`.
#[test]
fn idl4csharp_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3() {
    let source = "struct Basics {
            int8 i8; uint8 u8; short s; int16 s16; unsigned short us; uint16 u16;
            long l; int32 l32; unsigned long ul; uint32 u32;
            long long ll; int64 l64; unsigned long long ull; uint64 u64;
            float f; double d; long double ld; char c; wchar wc; boolean b; octet o;
        };
        const boolean YES = TRUE;
        const boolean NO = FALSE;";
    let files = cs(source, &idl_naming());
    let check = check_program(
        r#"
        var t = typeof(Basics);
        Property(t, "i8", typeof(sbyte), true);
        Property(t, "u8", typeof(byte), true);
        Property(t, "s", typeof(short), true);
        Property(t, "s16", typeof(short), true);
        Property(t, "us", typeof(ushort), true);
        Property(t, "u16", typeof(ushort), true);
        Property(t, "l", typeof(int), true);
        Property(t, "l32", typeof(int), true);
        Property(t, "ul", typeof(uint), true);
        Property(t, "u32", typeof(uint), true);
        Property(t, "ll", typeof(long), true);
        Property(t, "l64", typeof(long), true);
        Property(t, "ull", typeof(ulong), true);
        Property(t, "u64", typeof(ulong), true);
        Property(t, "f", typeof(float), true);
        Property(t, "d", typeof(double), true);
        Property(t, "ld", typeof(decimal), true);
        Property(t, "c", typeof(char), true);
        Property(t, "wc", typeof(char), true);
        Property(t, "b", typeof(bool), true);
        Property(t, "o", typeof(byte), true);
        Ok(YES.Value == true && NO.Value == false, "TRUE and FALSE");
        Throws<System.ArgumentOutOfRangeException>(() => new Basics().c = 'Ā', "an IDL char is 8 bits");
"#,
    );
    compile_and_run("basic-types", &files, &check);
}

/// §7.2.4.2.1 and Table 7.4: a sequence is `Omg.Types.ISequence<T>` with
/// the `List<T>` methods; a bounded one refuses to grow past its bound.
/// §7.2.4.2.2 to §7.2.4.2.4: `string` and `wstring` are `string`, `fixed`
/// is `decimal`, and "Range checking shall raise a
/// System.ArithmeticException".
#[test]
fn idl4csharp_7_2_4_2_sequences_strings_and_fixed() {
    let source = "struct Templates {
            sequence<boolean> bs; sequence<int8> i8s; sequence<uint16> u16s;
            sequence<long double> lds; sequence<string> ss; sequence<long, 2> bounded;
            string<4> short_text; wstring wide; fixed<5, 2> money;
        };";
    let files = cs(source, &idl_naming());
    let check = check_program(
        r#"
        var t = typeof(Templates);
        Property(t, "bs", typeof(Omg.Types.ISequence<bool>), false);
        Property(t, "i8s", typeof(Omg.Types.ISequence<sbyte>), false);
        Property(t, "u16s", typeof(Omg.Types.ISequence<ushort>), false);
        Property(t, "lds", typeof(Omg.Types.ISequence<decimal>), false);
        Property(t, "ss", typeof(Omg.Types.ISequence<string>), false);
        Property(t, "wide", typeof(string), true);
        Property(t, "money", typeof(decimal), true);
        Ok(typeof(System.Collections.Generic.IList<int>).IsAssignableFrom(typeof(Omg.Types.ISequence<int>)),
            "ISequence<T> extends IList<T>");
        var v = new Templates();
        v.bounded.Add(1);
        v.bounded.Add(2);
        Throws<System.ArgumentOutOfRangeException>(() => v.bounded.Add(3), "a bounded sequence past its bound");
        v.ss.AddRange(new[] { "b", "a" });
        v.ss.Sort();
        Ok(v.ss[0] == "a" && v.ss.FindIndex(x => x == "b") == 1, "the List<T> methods");
        Throws<System.ArgumentOutOfRangeException>(() => v.short_text = "12345", "a string past its bound");
        v.money = 123.45m;
        Throws<System.ArithmeticException>(() => v.money = 1234.5m, "fixed<5,2> holds three digits before the point");
        Throws<System.ArithmeticException>(() => v.money = 1.234m, "and two after it");
"#,
    );
    compile_and_run("templates", &files, &check);
}

/// §7.2.4.4's example `MyType`: arrays, "The bounds for the array shall be
/// checked by the setter of the corresponding property and a
/// System.ArgumentOutOfRangeException shall be raised".
#[test]
fn idl4csharp_7_2_4_4_an_array_is_a_csharp_array_checked_by_its_setter() {
    let source = "const long foo_array_length = 200;
                  struct Foo { long x; };
                  struct MyType { long long_array[100]; Foo foo_array[foo_array_length]; long grid[2][3]; };";
    let files = cs(source, &idl_naming());
    let check = check_program(
        r#"
        var t = typeof(MyType);
        Property(t, "long_array", typeof(int[]), true);
        Property(t, "foo_array", typeof(Foo[]), true);
        Property(t, "grid", typeof(int[,]), true);
        var m = new MyType();
        Ok(m.foo_array.Length == foo_array_length.Value && m.foo_array[199] != null,
            "the elements default-constructed");
        Ok(m.grid.GetLength(0) == 2 && m.grid.GetLength(1) == 3, "a rectangular array");
        Throws<System.ArgumentOutOfRangeException>(() => m.long_array = new int[99], "99 elements");
        Throws<System.ArgumentOutOfRangeException>(() => m.grid = new int[3, 2], "the dimensions swapped");
        m.long_array = new int[100];
"#,
    );
    compile_and_run("arrays", &files, &check);
}

/// §7.2.4.6 and §7.16.2.1's examples: a typedef creates no type, every use
/// takes the referenced type, and `@max(100)` on the typedef reaches a
/// member and a sequence's elements.
#[test]
fn idl4csharp_7_2_4_6_a_typedef_is_replaced_and_its_annotations_reach_its_uses() {
    let source = "typedef long Length;
                  typedef @max(100) long Bounded;
                  struct MyType { Length my_type_length; Bounded a; sequence<Bounded> a_seq; };
                  @annotation Tag { string note default \"\"; };
                  typedef @Tag(note = \"from the typedef\") long Tagged;
                  struct Holder { Tagged t; };";
    let files = cs(source, &idl_naming());
    assert!(
        files.iter().all(|f| !f.path.contains("Length")),
        "a typedef creates no type"
    );
    let check = check_program(
        r#"
        Property(typeof(MyType), "my_type_length", typeof(int), true);
        var m = new MyType();
        m.a = 100;
        Throws<System.ArgumentOutOfRangeException>(() => m.a = 101, "@max(100) on the typedef");
        Throws<System.ArgumentOutOfRangeException>(() => new MyType(0, 0, new Omg.Types.SequenceList<int>(new[] { 101 })),
            "and on the sequence's elements");
        var tag = (Tag)System.Attribute.GetCustomAttribute(typeof(Holder).GetProperty("t"), typeof(Tag));
        Ok(tag != null && tag.note == "from the typedef", "a user annotation of the typedef reaches the member");
"#,
    );
    compile_and_run("typedefs", &files, &check);
}

/// §7.2.4.3.4: "Constructed recursive types are supported by mapping the
/// involved types directly to C#".
#[test]
fn idl4csharp_7_2_4_3_4_a_recursive_type_names_its_own_class() {
    let files = cs(
        "struct Node { long value; sequence<Node> children; };",
        &idl_naming(),
    );
    let check = check_program(
        r#"
        var root = new Node();
        root.children.Add(new Node(2, new Omg.Types.SequenceList<Node>()));
        Ok(root.children[0].value == 2, "a node holds nodes");
        Ok(new Node(root).Equals(root), "and copies deep");
"#,
    );
    compile_and_run("recursive", &files, &check);
}

/// §7.3: `any` is `Omg.Types.Any`, which holds a value and tells its type.
#[test]
fn idl4csharp_7_3_any_is_omg_types_any() {
    let files = cs("struct Holder { any value; };", &idl_naming());
    let check = check_program(
        r#"
        Property(typeof(Holder), "value", typeof(Omg.Types.Any), true);
        var h = new Holder();
        h.value.Insert(42);
        Ok(h.value.Extract<int>() == 42 && h.value.Type == typeof(int), "insert, extract, type");
        Throws<System.InvalidCastException>(() => h.value.Extract<string>(), "another type");
"#,
    );
    compile_and_run("any", &files, &check);
}

/// §7.4's example `AnInterface` under both schemes: `I<Name>`, a property
/// per attribute (no setter for `readonly`), a method per operation with
/// `out` as `out` and `inout` as `ref`; §7.4.1's `AnException` extends
/// `System.Exception` with the three constructors and a property; §7.4.2:
/// a forward declaration maps to nothing.
#[test]
fn idl4csharp_7_4_an_interface_is_i_name_with_properties_and_ref_and_out() {
    let source = "struct Foo { long x; };
        exception AnException { long error_code; };
        interface Base { void ping(); };
        interface AnInterface : Base {
            attribute long attr;
            readonly attribute long ro_attr;
            void op1(in long i_param, inout long io_param, out long o_param, out Foo fo_param)
                raises (AnException);
        };
        interface Fwd;
        interface Fwd { void f(); };";
    let idl = cs(source, &idl_naming());
    assert_eq!(idl.iter().filter(|f| f.path == "IFwd.cs").count(), 1);
    let check = check_program(
        r#"
        var t = typeof(IAnInterface);
        Ok(t.IsInterface && typeof(IBase).IsAssignableFrom(t), "IAnInterface : IBase");
        Property(t, "attr", typeof(int), true);
        Property(t, "ro_attr", typeof(int), false);
        var op1 = Method(t, "op1", typeof(int), typeof(int).MakeByRefType(), typeof(int).MakeByRefType(),
            typeof(Foo).MakeByRefType());
        if (op1 != null)
        {
            var ps = op1.GetParameters();
            Ok(!ps[1].IsOut && ps[2].IsOut && ps[3].IsOut, "inout is ref, out is out");
            Ok(ps[0].Name == "i_param" && ps[3].Name == "fo_param", "the parameters keep their IDL names");
        }
        Ok(typeof(AnException).BaseType == typeof(System.Exception), "AnException : System.Exception");
        var e = new AnException(7);
        Ok(e.error_code == 7 && new AnException(e).error_code == 7 && new AnException().error_code == 0,
            "the three constructors");
"#,
    );
    compile_and_run("interface-idl", &idl, &check);
    let dotnet = cs(source, &dotnet_naming());
    let check = check_program(
        r#"
        var t = typeof(IAnInterface);
        Property(t, "Attr", typeof(int), true);
        Property(t, "RoAttr", typeof(int), false);
        var op1 = Method(t, "Op1", typeof(int), typeof(int).MakeByRefType(), typeof(int).MakeByRefType(),
            typeof(Foo).MakeByRefType());
        if (op1 != null)
        {
            Ok(op1.GetParameters()[1].Name == "ioParam", "camel-case parameters");
        }
        Property(typeof(AnException), "ErrorCode", typeof(int), true);
"#,
    );
    compile_and_run("interface-dotnet", &dotnet, &check);
}

/// §7.5's example `FullInterface`: `IFullInterfaceOperations` with the
/// operations and attributes, `IFullInterface` inheriting it, and the class
/// `FullInterface` holding the embedded `S` and `PI`.
#[test]
fn idl4csharp_7_5_a_full_interface_is_operations_interface_and_class() {
    let source = "interface FullInterface {
            struct S { long a_long; };
            const double PI = 3.14;
            void op1(in S s_in);
            attribute long an_attribute;
        };";
    let files = cs(source, &idl_naming());
    let check = check_program(
        r#"
        var ops = typeof(IFullInterfaceOperations);
        Method(ops, "op1", typeof(FullInterface.S));
        Property(ops, "an_attribute", typeof(int), true);
        Ok(ops.IsAssignableFrom(typeof(IFullInterface)), "IFullInterface : IFullInterfaceOperations");
        Ok(typeof(IFullInterface).IsAssignableFrom(typeof(FullInterface)), "the class implements it");
        Ok(FullInterface.PI.Value == 3.14, "the embedded constant");
        Ok(new FullInterface.S(4).a_long == 4, "the embedded structure");
"#,
    );
    compile_and_run("full-interface", &files, &check);
}

/// §7.6's example `VT1` and `VT2`: abstract classes, the private member
/// protected, the operations abstract, the factory a void method with
/// `[Omg.Types.Factory]`, the base and the supported interface.
#[test]
fn idl4csharp_7_6_a_value_type_is_an_abstract_class() {
    let source = "valuetype VT1 {
            attribute long a_long_attr;
            void vt_op(in long p_long);
            public long a_public_long;
            private long a_private_long;
            factory vt_factory(in long a_long, in short a_short);
        };
        interface MyInterface { void op(); };
        valuetype VT2 : VT1 supports MyInterface { public long third_long; };";
    let files = cs(source, &idl_naming());
    let check = check_program(
        r#"
        var vt1 = typeof(VT1);
        Ok(vt1.IsAbstract && vt1.IsClass, "VT1 is an abstract class");
        Property(vt1, "a_long_attr", typeof(int), true);
        Property(vt1, "a_public_long", typeof(int), true);
        var priv = vt1.GetProperty("a_private_long", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic);
        Ok(priv != null && priv.GetMethod.IsFamily, "a_private_long is protected");
        var op = Method(vt1, "vt_op", typeof(int));
        Ok(op != null && op.IsAbstract, "vt_op is abstract");
        var factory = Method(vt1, "vt_factory", typeof(int), typeof(short));
        Ok(factory != null && factory.ReturnType == typeof(void)
            && factory.GetCustomAttributes(typeof(Omg.Types.FactoryAttribute), false).Length == 1,
            "vt_factory returns void with [Omg.Types.Factory]");
        Ok(typeof(VT2).BaseType == vt1 && typeof(IMyInterface).IsAssignableFrom(typeof(VT2)), "VT2 : VT1, IMyInterface");
        Property(typeof(VT2), "third_long", typeof(int), true);
"#,
    );
    compile_and_run("valuetypes", &files, &check);
}

/// §7.14.1's example `ChildStruct : MyStruct`: the class extends the
/// base's, the all values constructor takes the base object first, the
/// copy constructor copies both.
#[test]
fn idl4csharp_7_14_1_a_derived_structure_extends_its_base() {
    let source = format!("{MY_STRUCT} struct ChildStruct : MyStruct {{ float a_float; }};");
    let files = cs(&source, &idl_naming());
    let check = check_program(
        r#"
        Ok(typeof(ChildStruct).BaseType == typeof(MyStruct), "ChildStruct : MyStruct");
        Ok(typeof(System.IEquatable<ChildStruct>).IsAssignableFrom(typeof(ChildStruct)), "IEquatable<ChildStruct>");
        var parent = new MyStruct();
        parent.a_long = 3;
        var child = new ChildStruct(parent, 1.5f);
        Ok(child.a_long == 3 && child.a_float == 1.5f, "the all values constructor");
        var copy = new ChildStruct(child);
        Ok(copy.Equals(child) && copy.a_long == 3, "the copy constructor");
        copy.a_long = 4;
        Ok(!copy.Equals(child), "base members take part in equality");
"#,
    );
    compile_and_run("struct-inheritance", &files, &check);
}

/// §7.14.2: `int8`, `uint8`, `wchar` and `octet` discriminate a union as
/// §7.2.4.3.2 maps it.
#[test]
fn idl4csharp_7_14_2_int8_uint8_wchar_and_octet_discriminate_a_union() {
    let source = "union U8 switch (int8) { case -1: long neg; case 1: long pos; };
        union UU8 switch (uint8) { case 200: long high; };
        union UW switch (wchar) { case L'x': long x; };
        union UO switch (octet) { case 7: long seven; };";
    let files = cs(source, &idl_naming());
    let check = check_program(
        r#"
        Property(typeof(U8), "Discriminator", typeof(sbyte), false);
        Property(typeof(UU8), "Discriminator", typeof(byte), false);
        Property(typeof(UW), "Discriminator", typeof(char), false);
        Property(typeof(UO), "Discriminator", typeof(byte), false);
        var u = new U8();
        u.neg = 1;
        Ok(u.Discriminator == -1, "int8 label -1");
        var w = new UW();
        w.x = 2;
        Ok(w.Discriminator == 'x', "wchar label");
"#,
    );
    compile_and_run("discriminators", &files, &check);
}

/// §7.14.3.1's example `MyType`: a map is
/// `System.Collections.Generic.IDictionary<TKey, TValue>`, a read-only
/// property; "Bounds checking shall raise an exception if necessary".
#[test]
fn idl4csharp_7_14_3_1_a_map_is_an_idictionary() {
    let source = "struct Foo { long x; };
        struct MyType { map<long, string> long_str_map; map<string, Foo> str_foo_map; map<long, long, 1> small; };";
    let files = cs(source, &idl_naming());
    let check = check_program(
        r#"
        Property(typeof(MyType), "long_str_map", typeof(System.Collections.Generic.IDictionary<int, string>), false);
        Property(typeof(MyType), "str_foo_map", typeof(System.Collections.Generic.IDictionary<string, Foo>), false);
        var m = new MyType();
        m.long_str_map[1] = "one";
        m.small[1] = 1;
        Throws<System.ArgumentOutOfRangeException>(() => m.small[2] = 2, "a bounded map past its bound");
        var copy = new MyType(m);
        Ok(copy.Equals(m), "maps compare by their entries");
        copy.long_str_map[2] = "two";
        Ok(m.long_str_map.Count == 1, "and copy deep");
"#,
    );
    compile_and_run("maps", &files, &check);
}

/// §7.14.3.2's example `MyBitset`: a C# struct with a property per named
/// bit field, the smallest unsigned type when the IDL names none, and
/// `IEquatable<MyBitset>`.
#[test]
fn idl4csharp_7_14_3_2_a_bit_set_is_a_struct() {
    let source = "bitset MyBitset {
            bitfield<3> a;
            bitfield<1> b;
            bitfield<4>;
            bitfield<12, short> d;
            bitfield<20> e;
        };";
    let files = cs(source, &idl_naming());
    let check = check_program(
        r#"
        var t = typeof(MyBitset);
        Ok(t.IsValueType && typeof(System.IEquatable<MyBitset>).IsAssignableFrom(t), "a struct, IEquatable");
        Property(t, "a", typeof(byte), true);
        Property(t, "b", typeof(byte), true);
        Property(t, "d", typeof(short), true);
        Property(t, "e", typeof(uint), true);
        var v = new MyBitset();
        v.a = 7;
        Throws<System.ArgumentOutOfRangeException>(() => { var w = new MyBitset(); w.a = 8; }, "8 is no 3-bit value");
        var same = new MyBitset();
        same.a = 7;
        Ok(v.Equals(same), "equal bit sets");
"#,
    );
    compile_and_run("bitset", &files, &check);
}

/// §7.14.3.3's example: `MyBitMaskFlags`, `[Flags]`, `1 << n`, the type of
/// its size, and a member of the bit mask a `System.Collections.BitArray`.
#[test]
fn idl4csharp_7_14_3_3_a_bit_mask_is_a_flags_enum_and_a_bit_array() {
    let source = "bitmask MyBitMask { flag0, flag1, flag2, @position(7) flag7 };
        @bit_bound(20) bitmask Wide { w0 };
        struct BitMaskExample { MyBitMask a_bitmask; };";
    let files = cs(source, &idl_naming());
    let check = check_program(
        r#"
        var t = typeof(MyBitMaskFlags);
        Ok(t.IsEnum && t.GetCustomAttributes(typeof(System.FlagsAttribute), false).Length == 1, "[Flags] enum");
        Ok(System.Enum.GetUnderlyingType(t) == typeof(uint), "the default @bit_bound 32: uint");
        Ok((int)MyBitMaskFlags.flag2 == 4 && (int)MyBitMaskFlags.flag7 == 128, "1 << position");
        Ok(System.Enum.GetUnderlyingType(typeof(WideFlags)) == typeof(uint), "@bit_bound(20): uint");
        Property(typeof(BitMaskExample), "a_bitmask", typeof(System.Collections.BitArray), true);
        var e = new BitMaskExample();
        e.a_bitmask[1] = true;
        Ok(e.a_bitmask[1], "a flag set in the bit array");
"#,
    );
    compile_and_run("bitmask", &files, &check);
}

/// §7.16's examples: an annotation is an attribute class
/// `[AttributeUsage(AttributeTargets.All, AllowMultiple = true)]` with a
/// property per member and its default, `MyAnnotationAttribute` under the
/// .NET scheme; an annotated element carries the attribute.
#[test]
fn idl4csharp_7_16_a_user_annotation_is_an_attribute_class() {
    let source = "@annotation MyAnnotation { boolean value default TRUE; long level default 3; };
        @MyAnnotation struct AnnotatedStruct { @MyAnnotation(level = 5) @MyAnnotation(value = FALSE) long a_long; };";
    for (options, class, value, level, tag) in [
        (idl_naming(), "MyAnnotation", "value", "level", "idl"),
        (
            dotnet_naming(),
            "MyAnnotationAttribute",
            "Value",
            "Level",
            "dotnet",
        ),
    ] {
        let files = cs(source, &options);
        let member = if tag == "idl" { "a_long" } else { "ALong" };
        let check = check_program(&format!(
            r#"
        var t = typeof({class});
        Ok(typeof(System.Attribute).IsAssignableFrom(t), "an attribute class");
        var usage = (System.AttributeUsageAttribute)System.Attribute.GetCustomAttribute(t, typeof(System.AttributeUsageAttribute));
        Ok(usage != null && usage.ValidOn == System.AttributeTargets.All && usage.AllowMultiple, "AttributeUsage");
        var fresh = new {class}();
        Ok(fresh.{value} == true && fresh.{level} == 3, "the defaults");
        Ok(typeof(AnnotatedStruct).GetCustomAttributes(t, false).Length == 1, "the structure carries it");
        var on = typeof(AnnotatedStruct).GetProperty("{member}").GetCustomAttributes(t, false);
        Ok(on.Length == 2, "two applications, two attributes");
"#
        ));
        compile_and_run(&format!("annotations-{tag}"), &files, &check);
    }
}

/// Tables 7.5 to 7.8: `@optional` is `System.Nullable<T>` for a value type
/// and `[Omg.Types.Optional]` on the property and the constructor's
/// parameter; `@key` is `[Omg.Types.Key]`; `@default` and
/// `@default_literal` are the default constructor's values; `@range`,
/// `@min` and `@max` throw `ArgumentOutOfRangeException`; `@unit` is
/// `[Omg.Types.Unit]`; `@external` is `Nullable<T>`, and `IEnumerable<T>`
/// with a setter for a sequence.
#[test]
fn idl4csharp_7_17_the_standardized_annotations_shape_the_class() {
    let source = "enum Color { RED, @default_literal GREEN };
        struct CoordinatesInfo { long id; };
        struct Coordinates {
            @key long x;
            long y;
            @optional long z;
            @optional CoordinatesInfo extra_info;
            @default(7) long seven;
            Color shade;
            @range(min = 1, max = 9) long digit;
            @min(0) double positive;
            @unit(\"m\") double distance;
            @external long boxed;
            @external sequence<long> many;
        };";
    let files = cs(source, &idl_naming());
    let check = check_program(
        r#"
        var t = typeof(Coordinates);
        Property(t, "z", typeof(int?), true);
        Property(t, "extra_info", typeof(CoordinatesInfo), true);
        Ok(t.GetProperty("z").GetCustomAttributes(typeof(Omg.Types.OptionalAttribute), false).Length == 1, "[Optional] on z");
        Ok(t.GetProperty("x").GetCustomAttributes(typeof(Omg.Types.KeyAttribute), false).Length == 1, "[Key] on x");
        var unit = (Omg.Types.UnitAttribute)System.Attribute.GetCustomAttribute(t.GetProperty("distance"), typeof(Omg.Types.UnitAttribute));
        Ok(unit != null && unit.UnitName == "m", "[Unit(\"m\")]");
        Property(t, "boxed", typeof(int?), true);
        Property(t, "many", typeof(System.Collections.Generic.IEnumerable<int>), true);
        var all = t.GetConstructors();
        bool optionalParameter = false;
        foreach (var c in all)
            foreach (var p in c.GetParameters())
                if (p.Name == "z" && p.GetCustomAttributes(typeof(Omg.Types.OptionalAttribute), false).Length == 1)
                    optionalParameter = true;
        Ok(optionalParameter, "[Optional] on the constructor's parameter");
        var c0 = new Coordinates();
        Ok(c0.seven == 7 && c0.shade == Color.GREEN && c0.z == null, "@default, @default_literal, an unset optional");
        Throws<System.ArgumentOutOfRangeException>(() => c0.digit = 10, "@range max");
        Throws<System.ArgumentOutOfRangeException>(() => c0.digit = 0, "@range min");
        Throws<System.ArgumentOutOfRangeException>(() => c0.positive = -1, "@min");
        c0.digit = 5;
"#,
    );
    compile_and_run("standardized", &files, &check);
}

/// Tables 7.5, 7.6 and 7.8: `@id`, `@autoid`, the extensibility
/// annotations, `@must_understand` and `@nested` have "No impact on
/// language mapping".
#[test]
fn idl4csharp_7_17_the_annotations_without_impact_leave_the_class_unchanged() {
    let plain = cs(
        "struct Reading { long sensor; double value; };",
        &idl_naming(),
    );
    for annotated in [
        "@autoid(SEQUENTIAL) struct Reading { @id(7) long sensor; double value; };",
        "@extensibility(MUTABLE) struct Reading { long sensor; double value; };",
        "@final struct Reading { long sensor; double value; };",
        "@mutable struct Reading { long sensor; double value; };",
        "@appendable struct Reading { long sensor; double value; };",
        "struct Reading { long sensor; @must_understand double value; };",
        "@nested struct Reading { long sensor; double value; };",
    ] {
        let files = cs(annotated, &idl_naming());
        assert_eq!(
            file(&files, "Reading.cs"),
            file(&plain, "Reading.cs"),
            "{annotated}"
        );
    }
}

/// Table 7.9: `@verbatim` text is copied "when the indicated language is
/// "*", "c#", "cs", or "csharp"", and not for another language.
#[test]
fn idl4csharp_7_17_5_verbatim_text_is_copied_for_its_four_names() {
    let source = r#"@verbatim(language = "c#", placement = BEGIN_DECLARATION, text = "public const int FromHash = 1;")
        @verbatim(language = "cs", placement = BEGIN_DECLARATION, text = "public const int FromCs = 2;")
        @verbatim(language = "csharp", placement = END_DECLARATION, text = "public const int FromCsharp = 3;")
        @verbatim(language = "*", placement = END_DECLARATION, text = "public const int FromStar = 4;")
        @verbatim(language = "java", placement = BEGIN_DECLARATION, text = "int fromJava;")
        struct Tagged { long m; };"#;
    let files = cs(source, &idl_naming());
    assert!(!file(&files, "Tagged.cs").contains("fromJava"));
    let check = check_program(
        r#"
        Ok(Tagged.FromHash == 1 && Tagged.FromCs == 2 && Tagged.FromCsharp == 3 && Tagged.FromStar == 4, "the four names");
"#,
    );
    compile_and_run("verbatim", &files, &check);
}

/// Table 7.10: `@service`, `@oneway` and `@ami` have a platform-specific
/// impact, none on the language mapping.
#[test]
fn idl4csharp_7_17_6_the_interface_annotations_leave_the_csharp_unchanged() {
    let plain = cs(
        "interface Bank { long deposit(in long amount); void close(); };",
        &idl_naming(),
    );
    for annotated in [
        "@service(\"DDS\") interface Bank { long deposit(in long amount); void close(); };",
        "interface Bank { @ami long deposit(in long amount); @oneway void close(); };",
    ] {
        let files = cs(annotated, &idl_naming());
        assert_eq!(
            file(&files, "IBank.cs"),
            file(&plain, "IBank.cs"),
            "{annotated}"
        );
    }
}

/// §8.1: `@csharp_mapping` sets the naming scheme per module and
/// declaration, and `struct_type = "struct"` (§8.1.3) maps a structure to
/// a C# struct.
#[test]
fn idl4csharp_8_1_csharp_mapping_sets_the_scheme_and_the_struct_type() {
    let source = "@csharp_mapping(apply_naming_convention = DOTNET_NAMING_CONVENTION)
        module my_mod {
            struct my_struct { long a_value; };
            @csharp_mapping(apply_naming_convention = IDL_NAMING_CONVENTION)
            struct kept_struct { long a_value; };
            @csharp_mapping(struct_type = \"struct\")
            struct Point { long x; long y; };
        };";
    let files = cs(source, &idl_naming());
    let check = check_program(
        r#"
        Property(typeof(MyMod.MyStruct), "AValue", typeof(int), true);
        Property(typeof(MyMod.kept_struct), "a_value", typeof(int), true);
        Ok(typeof(MyMod.Point).IsValueType, "struct_type = struct");
        var p = new MyMod.Point(1, 2);
        var q = p;
        q.X = 5;
        Ok(p.X == 1 && new MyMod.Point(p).Equals(p), "copied by value");
"#,
    );
    compile_and_run("csharp-mapping", &files, &check);
}

/// §7.1.1 and §7.1.2: a mapped name that is a C# keyword takes `@`, one
/// that is `Constants` takes `_`.
#[test]
fn idl4csharp_7_1_2_reserved_names_take_their_prefix() {
    let source = "struct Constants { long x; }; struct Keywords { long _class; long _object; };";
    let files = cs(source, &idl_naming());
    file(&files, "_Constants.cs");
    let check = check_program(
        r#"
        var u = new Keywords();
        u.@class = 1;
        u.@object = 2;
        Ok(new _Constants().x == 0 && u.@class == 1 && u.@object == 2, "@class, @object, _Constants");
"#,
    );
    compile_and_run("reserved", &files, &check);
}
