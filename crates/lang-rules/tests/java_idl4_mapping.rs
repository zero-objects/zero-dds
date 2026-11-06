// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL4 to Java Language Mapping 1.0 (OMG formal/2021-08-01), clause 7
//! and 8, on the specification's own examples.
//!
//! Each test generates the Java of an IDL file, compiles it with `javac`
//! together with the `org.omg` runtime, and runs a check program whose
//! expectations are the clause's text: the class and method names and
//! types its examples print, the exceptions its sentences name. The
//! generator is not asked what it meant; the compiled classes are
//! inspected by reflection and exercised.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

#[path = "java-idl4/support.rs"]
mod support;

use support::{check_program, compile_and_run, file, java};
use zerodds_lang_rules::emit::idl4_java::{Naming, Options};

fn idl_naming() -> Options {
    Options::default()
}

fn java_naming() -> Options {
    Options {
        naming: Naming::Java,
        ..Options::default()
    }
}

/// §7.2.4.3.1's example under both naming schemes: a public class of the
/// structure's name, `java.io.Serializable`, a public default and all
/// values constructor, and an accessor and a modifier per member,
/// `get_<name>`/`set_<name>` under the IDL scheme and
/// `get<Name>`/`set<Name>` under the Java one.
#[test]
fn idl4java_7_2_4_3_1_a_structure_is_a_serializable_class_with_accessors_and_two_constructors() {
    let idl = "struct S1 {
                 long long_variable;
                 short short_variable;
                 long long longlong_variable;
                 string URL;
               };";
    let files = java(idl, &idl_naming());
    compile_and_run(
        "struct-idl",
        &files,
        &check_program(
            r#"
        check(java.io.Serializable.class.isAssignableFrom(S1.class), "S1 is Serializable");
        check(java.lang.reflect.Modifier.isPublic(S1.class.getModifiers()), "S1 is public");
        S1.class.getConstructor();
        S1.class.getConstructor(int.class, short.class, long.class, String.class);
        returns(S1.class, "get_long_variable", int.class);
        returns(S1.class, "set_long_variable", void.class, int.class);
        returns(S1.class, "get_short_variable", short.class);
        returns(S1.class, "set_short_variable", void.class, short.class);
        returns(S1.class, "get_longlong_variable", long.class);
        returns(S1.class, "set_longlong_variable", void.class, long.class);
        returns(S1.class, "get_URL", String.class);
        returns(S1.class, "set_URL", void.class, String.class);
        S1 s = new S1(1, (short) 2, 3L, "u");
        check(s.get_long_variable() == 1 && s.get_short_variable() == 2, "all values constructor");
        check(s.get_longlong_variable() == 3L && s.get_URL().equals("u"), "all values constructor");
        s.set_URL("v");
        check(s.get_URL().equals("v"), "modifier then accessor");
        // The default constructor: primitives as Java initialises them,
        // a string empty.
        S1 d = new S1();
        check(d.get_long_variable() == 0 && d.get_URL().equals(""), "default constructor");
        check(java.lang.reflect.Modifier.isPrivate(S1.class.getDeclaredField("URL").getModifiers()),
            "the state is not public");
"#,
        ),
    );
    let files = java(idl, &java_naming());
    compile_and_run(
        "struct-java",
        &files,
        &check_program(
            r#"
        S1.class.getConstructor(int.class, short.class, long.class, String.class);
        returns(S1.class, "getLongVariable", int.class);
        returns(S1.class, "setLongVariable", void.class, int.class);
        returns(S1.class, "getShortVariable", short.class);
        returns(S1.class, "getLonglongVariable", long.class);
        returns(S1.class, "getURL", String.class);
        returns(S1.class, "setURL", void.class, String.class);
"#,
        ),
    );
}

/// §7.2.4.3.1: "All array members shall be initialized to an array of
/// declared size whose elements are initialized with their default
/// constructor. All sequence members shall be initialized to zero-length
/// sequences of the corresponding type. All other members shall be
/// initialized to an object created with their respective default
/// constructor."
#[test]
fn idl4java_7_2_4_3_1_the_default_constructor_initialises_every_member() {
    let files = java(
        "struct Foo { long a; };
         enum Color { RED, GREEN };
         struct S {
           Foo nested;
           Foo several[2][3];
           long numbers[4];
           sequence<long> longs;
           sequence<Foo> foos;
           string text;
           Color tint;
           fixed<5,2> amount;
         };",
        &idl_naming(),
    );
    compile_and_run(
        "defaults",
        &files,
        &check_program(
            r#"
        S s = new S();
        check(s.get_nested() != null, "a structure member is default-constructed");
        check(s.get_several().length == 2 && s.get_several()[1].length == 3, "array size");
        check(s.get_several()[1][2] != null && s.get_several()[1][2] != s.get_several()[0][0],
            "every array element is its own default-constructed object");
        check(s.get_numbers().length == 4, "a primitive array at its size");
        check(s.get_longs().size() == 0 && s.get_foos().isEmpty(), "sequences are empty");
        check(s.get_text().equals(""), "a string is empty");
        check(s.get_tint() == Color.RED, "an enumeration member starts at its first enumerator");
        check(s.get_amount() != null, "a fixed member has a value");
"#,
        ),
    );
}

/// §7.2.4.3.2's examples `U1` and `U2`: a public final class, the
/// discriminator accessor, an accessor and a modifier per member, a
/// modifier with the discriminator for a member of several labels and for
/// the default member, `__default()` for a union with no default label
/// whose labels do not cover the discriminator type, IllegalStateException
/// from an accessor of a member not set and IllegalArgumentException from
/// a modifier given a discriminator not among the member's labels.
#[test]
fn idl4java_7_2_4_3_2_a_union_is_a_final_class_with_discriminator_accessors_and_modifiers() {
    let idl = "union U1 switch (octet) {
                 case 1: long long_variable;
                 case 2:
                 case 3: short short_variable;
                 default: octet octet_variable;
               };
               union U2 switch (long) {
                 case 1: short short_variable;
                 case 2: long long_variable;
               };";
    let files = java(idl, &idl_naming());
    compile_and_run(
        "union-idl",
        &files,
        &check_program(
            r#"
        check(java.lang.reflect.Modifier.isFinal(U1.class.getModifiers()), "U1 is final");
        check(java.io.Serializable.class.isAssignableFrom(U1.class), "U1 is Serializable");
        returns(U1.class, "get_discriminator", byte.class);
        returns(U1.class, "get_long_variable", int.class);
        returns(U1.class, "set_long_variable", void.class, int.class);
        returns(U1.class, "get_short_variable", short.class);
        returns(U1.class, "set_short_variable", void.class, short.class);
        returns(U1.class, "set_short_variable", void.class, short.class, byte.class);
        returns(U1.class, "get_octet_variable", byte.class);
        returns(U1.class, "set_octet_variable", void.class, byte.class);
        returns(U1.class, "set_octet_variable", void.class, byte.class, byte.class);
        absent(U1.class, "set_long_variable", int.class, byte.class);
        absent(U1.class, "__default");
        returns(U2.class, "get_discriminator", int.class);
        returns(U2.class, "__default", void.class);
        returns(U2.class, "__default", void.class, int.class);

        final U1 u = new U1();
        // The default value of an octet is 0, which no label names: the
        // default member is selected, at its default value.
        check(u.get_discriminator() == 0, "the discriminator starts at 0");
        check(u.get_octet_variable() == 0, "and selects the default member");
        u.set_long_variable(7);
        check(u.get_discriminator() == 1 && u.get_long_variable() == 7, "set_long_variable");
        throwsIt(IllegalStateException.class, () -> u.get_short_variable(), "accessor of another member");
        u.set_short_variable((short) 5, (byte) 3);
        check(u.get_discriminator() == 3 && u.get_short_variable() == 5, "the second label");
        throwsIt(IllegalArgumentException.class, () -> u.set_short_variable((short) 5, (byte) 1),
            "a discriminator that is no label of the member");
        u.set_octet_variable((byte) 9);
        check(u.get_discriminator() != 1 && u.get_discriminator() != 2 && u.get_discriminator() != 3,
            "the default member's modifier picks a value no label names");
        check(u.get_discriminator() == 0, "the first such value from 0");
        throwsIt(IllegalArgumentException.class, () -> u.set_octet_variable((byte) 9, (byte) 2),
            "the default member with a label of another member");

        final U2 v = new U2();
        v.__default();
        check(v.get_discriminator() == 0, "__default() picks the first value no label names");
        throwsIt(IllegalStateException.class, () -> v.get_short_variable(), "no member is set");
        v.__default(42);
        check(v.get_discriminator() == 42, "__default(int)");
        throwsIt(IllegalArgumentException.class, () -> v.__default(2), "__default with a label");
"#,
        ),
    );
    let files = java(idl, &java_naming());
    compile_and_run(
        "union-java",
        &files,
        &check_program(
            r#"
        returns(U1.class, "getDiscriminator", byte.class);
        returns(U1.class, "getLongVariable", int.class);
        returns(U1.class, "setShortVariable", void.class, short.class, byte.class);
        returns(U1.class, "setOctetVariable", void.class, byte.class, byte.class);
        returns(U2.class, "__default", void.class, int.class);
"#,
        ),
    );
}

/// §7.2.4.3.3's example: a Java `enum` of the enumeration's name, its
/// enumerators with their `@value`, `getValue()` and `valueOf(int)`, which
/// raises a RuntimeException for a value no enumerator has; the Java naming
/// scheme writes the enumerators in upper case.
#[test]
fn idl4java_7_2_4_3_3_an_enumeration_is_a_java_enum_with_value_of_int() {
    let idl = "enum AnEnum { @value(1) one, @value(2) two };";
    let files = java(idl, &idl_naming());
    compile_and_run(
        "enum-idl",
        &files,
        &check_program(
            r#"
        check(AnEnum.class.isEnum(), "AnEnum is a Java enum");
        check(AnEnum.one.getValue() == 1 && AnEnum.two.getValue() == 2, "the @value");
        check(AnEnum.valueOf(2) == AnEnum.two, "valueOf(int)");
        throwsIt(RuntimeException.class, () -> AnEnum.valueOf(3), "a value no enumerator has");
"#,
        ),
    );
    let files = java(idl, &java_naming());
    compile_and_run(
        "enum-java",
        &files,
        &check_program(
            r#"
        check(AnEnum.ONE.getValue() == 1 && AnEnum.valueOf(2) == AnEnum.TWO, "upper case");
"#,
        ),
    );
}

/// §7.2.2: "An IDL module is mapped to a Java package with the same name"
/// — the clause's `MY_MATH`, lower case under the Java scheme — and a
/// declaration outside any module is in the unnamed package. §7.2.3: "IDL
/// constants shall be mapped to public final classes of the same name
/// within the equivalent scope and package. The mapped class shall contain
/// a public final static field named value", with the clause's example.
#[test]
fn idl4java_7_2_2_and_7_2_3_a_module_is_a_package_and_a_constant_a_class_with_its_value() {
    let idl = "module MY_MATH {
                 const double PI = 3.141592;
                 const double e = 2.718282;
                 const string my_string = \"My String Value\";
                 module inner { struct P { long x; }; };
               };
               struct Outside { long y; };";
    let files = java(idl, &idl_naming());
    let pi = file(&files, "MY_MATH/PI.java");
    assert!(pi.contains("package MY_MATH;"), "{pi}");
    file(&files, "MY_MATH/inner/P.java");
    file(&files, "Outside.java");
    compile_and_run(
        "constants-idl",
        &files,
        &check_program(
            r#"
        check(MY_MATH.PI.value == 3.141592, "PI.value");
        check(MY_MATH.e.value == 2.718282, "e.value");
        check(MY_MATH.my_string.value.equals("My String Value"), "my_string.value");
        java.lang.reflect.Field f = MY_MATH.PI.class.getField("value");
        int m = f.getModifiers();
        check(java.lang.reflect.Modifier.isPublic(m) && java.lang.reflect.Modifier.isStatic(m)
            && java.lang.reflect.Modifier.isFinal(m), "a public final static field");
        check(java.lang.reflect.Modifier.isFinal(MY_MATH.PI.class.getModifiers()), "a final class");
        check(new MY_MATH.inner.P().get_x() == 0, "a nested module is a nested package");
        check(new Outside().get_y() == 0, "the unnamed package");
"#,
        ),
    );
    let files = java(idl, &java_naming());
    file(&files, "my_math/PI.java");
    file(&files, "my_math/E.java");
    file(&files, "my_math/MyString.java");
    compile_and_run(
        "constants-java",
        &files,
        &check_program(
            r#"
        check(my_math.E.value == 2.718282 && my_math.MyString.value.equals("My String Value"),
            "the Java naming scheme");
"#,
        ),
    );
}

/// §7.2.3.1 and §8.1.2: `@java_mapping(constants_container = ...)` on the
/// module collects its constants in one class, `Constants` by default,
/// named as the parameter says, with the field names in upper case under
/// the Java scheme.
#[test]
fn idl4java_7_2_3_1_and_8_1_2_the_constants_container_holds_the_constants_of_a_scope() {
    let idl = "@java_mapping(constants_container = \"MathematicalConstants\")
               module MY_MATH {
                 const double PI = 3.141592;
                 const double e = 2.718282;
               };
               @java_mapping(apply_naming_convention = JAVA_NAMING_CONVENTION,
                             constants_container = \"Constants\")
               module Other { const string my_string = \"x\"; };";
    let files = java(idl, &idl_naming());
    file(&files, "MY_MATH/MathematicalConstants.java");
    file(&files, "other/Constants.java");
    compile_and_run(
        "container",
        &files,
        &check_program(
            r#"
        check(MY_MATH.MathematicalConstants.PI == 3.141592, "PI");
        check(MY_MATH.MathematicalConstants.e == 2.718282, "e");
        check(other.Constants.MY_STRING.equals("x"), "upper case under the Java scheme");
"#,
        ),
    );
}

/// Tables 7.2 and 7.3, §7.2.4.1.3 to §7.2.4.1.6: the Java type of every
/// basic type, `TRUE`/`FALSE` as `true`/`false`; Table 8.2 with
/// `promote_integer_width` TRUE.
#[test]
fn idl4java_7_2_4_1_the_basic_types_map_by_tables_7_2_and_7_3() {
    let idl = "struct Basics {
                 int8 i8; uint8 u8; short s; int16 i16; unsigned short us; uint16 u16;
                 long l; int32 i32; unsigned long ul; uint32 u32;
                 long long ll; int64 i64; unsigned long long ull; uint64 u64;
                 float f; double d; long double ld;
                 char c; wchar wc; boolean b; octet o;
               };
               const boolean YES = TRUE;
               const boolean NO = FALSE;
               const octet BIG = 0xF1;
               const unsigned long MAXU = 4294967295;";
    let files = java(idl, &idl_naming());
    compile_and_run(
        "basic",
        &files,
        &check_program(
            r#"
        Object[][] table = {
            {"i8", byte.class}, {"u8", byte.class}, {"s", short.class}, {"i16", short.class},
            {"us", short.class}, {"u16", short.class}, {"l", int.class}, {"i32", int.class},
            {"ul", int.class}, {"u32", int.class}, {"ll", long.class}, {"i64", long.class},
            {"ull", long.class}, {"u64", long.class}, {"f", float.class}, {"d", double.class},
            {"ld", java.math.BigDecimal.class}, {"c", char.class}, {"wc", char.class},
            {"b", boolean.class}, {"o", byte.class},
        };
        for (Object[] row : table) {
            returns(Basics.class, "get_" + row[0], (Class<?>) row[1]);
        }
        check(YES.value == true && NO.value == false, "TRUE and FALSE");
        check(BIG.value == (byte) 0xF1, "an octet above 127 is its bit pattern");
        check(MAXU.value == -1, "an unsigned long holds its bit pattern in an int");
"#,
        ),
    );
    let promoted = Options {
        promote_integer_width: true,
        ..Options::default()
    };
    let files = java(idl, &promoted);
    compile_and_run(
        "basic-promoted",
        &files,
        &check_program(
            r#"
        returns(Basics.class, "get_u8", short.class);
        returns(Basics.class, "get_us", int.class);
        returns(Basics.class, "get_u16", int.class);
        returns(Basics.class, "get_ul", long.class);
        returns(Basics.class, "get_u32", long.class);
        returns(Basics.class, "get_ull", java.math.BigInteger.class);
        returns(Basics.class, "get_u64", java.math.BigInteger.class);
        returns(Basics.class, "get_i8", byte.class);
        returns(Basics.class, "get_o", byte.class);
        check(MAXU.value == 4294967295L, "a promoted unsigned long holds the number");
"#,
        ),
    );
}

/// §7.2.4.2.1.1, Table 7.4: a sequence of a basic type is the
/// type-specific class of `org.omg.type`; §7.2.4.2.1.2: any other is a
/// `java.util.List<E>` — the clause's `MyType`. "Bounds checking on bounded
/// sequences shall raise a java.lang.IndexOutOfBoundsException".
#[test]
fn idl4java_7_2_4_2_1_sequences_are_table_7_4_classes_or_lists_and_bounded_ones_are_checked() {
    let idl = "struct Foo { long a; };
               struct MyType {
                 sequence<long> long_sequence;
                 sequence<Foo> foo_sequence;
                 sequence<boolean> flags;
                 sequence<char> chars;
                 sequence<octet> bytes;
                 sequence<short> shorts;
                 sequence<long long> longs;
                 sequence<float> floats;
                 sequence<double> doubles;
                 sequence<long double> bigs;
                 sequence<string> strings;
                 sequence<long, 2> two;
               };";
    let files = java(idl, &idl_naming());
    compile_and_run(
        "sequences",
        &files,
        &check_program(
            r#"
        returns(MyType.class, "get_long_sequence", org.omg.type.IntegerSeq.class);
        returns(MyType.class, "set_long_sequence", void.class, org.omg.type.IntegerSeq.class);
        returns(MyType.class, "get_foo_sequence", java.util.List.class);
        check(method(MyType.class, "get_foo_sequence").getGenericReturnType().getTypeName()
            .equals("java.util.List<Foo>"), "List<Foo>");
        returns(MyType.class, "get_flags", org.omg.type.BooleanSeq.class);
        returns(MyType.class, "get_chars", org.omg.type.CharSeq.class);
        returns(MyType.class, "get_bytes", org.omg.type.ByteSeq.class);
        returns(MyType.class, "get_shorts", org.omg.type.ShortSeq.class);
        returns(MyType.class, "get_longs", org.omg.type.LongSeq.class);
        returns(MyType.class, "get_floats", org.omg.type.FloatSeq.class);
        returns(MyType.class, "get_doubles", org.omg.type.DoubleSeq.class);
        returns(MyType.class, "get_bigs", org.omg.type.BigDecimalSeq.class);
        check(java.util.List.class.isAssignableFrom(org.omg.type.IntegerSeq.class),
            "IntegerSeq is a List<Integer>");
        // The primitive operations of Table 7.4's interface.
        org.omg.type.IntegerSeq seq = new org.omg.type.IntegerSeq(new int[] {1, 2, 3});
        seq.add(4);
        seq.add(0, 9);
        seq.add(new int[] {5, 6, 7}, 1, 2);
        int first = seq.get(0);
        check(first == 9 && seq.size() == 7 && seq.get(6) == 7, "add and get");
        seq.set(0, 8);
        int[] out = seq.toArray(new int[0]);
        check(out.length == 7 && out[0] == 8 && out[5] == 6, "set and toArray");
        final MyType t = new MyType();
        t.set_two(new org.omg.type.IntegerSeq(new int[] {1, 2}));
        throwsIt(IndexOutOfBoundsException.class,
            () -> t.set_two(new org.omg.type.IntegerSeq(new int[] {1, 2, 3})),
            "a bounded sequence past its bound");
"#,
        ),
    );
}

/// §7.2.4.2.2, §7.2.4.2.3, §7.2.4.2.4: `string` and `wstring` are
/// `java.lang.String`, and "Range checking for characters in the string as
/// well as bounds checking of the string shall raise a
/// java.lang.IndexOutOfBoundsException"; `fixed` is `java.math.BigDecimal`
/// and "Range checking shall raise a java.lang.ArithmeticException".
#[test]
fn idl4java_7_2_4_2_strings_and_fixed_are_checked_by_their_modifiers() {
    let files = java(
        "struct T {
           string<4> short_text;
           string any_text;
           wstring<3> wide;
           fixed<5,2> amount;
         };",
        &idl_naming(),
    );
    compile_and_run(
        "strings",
        &files,
        &check_program(
            r#"
        returns(T.class, "get_wide", String.class);
        returns(T.class, "get_amount", java.math.BigDecimal.class);
        final T t = new T();
        t.set_short_text("abcd");
        throwsIt(IndexOutOfBoundsException.class, () -> t.set_short_text("abcde"), "a string past its bound");
        throwsIt(IndexOutOfBoundsException.class, () -> t.set_any_text("€"),
            "a character no IDL char is");
        t.set_wide("€€€");
        throwsIt(IndexOutOfBoundsException.class, () -> t.set_wide("€€€€"),
            "a wstring past its bound");
        t.set_amount(new java.math.BigDecimal("123.45"));
        check(t.get_amount().equals(new java.math.BigDecimal("123.45")), "fixed<5,2>");
        throwsIt(ArithmeticException.class, () -> t.set_amount(new java.math.BigDecimal("1234.5")),
            "more digits before the point than fixed<5,2> has");
        throwsIt(ArithmeticException.class, () -> t.set_amount(new java.math.BigDecimal("1.234")),
            "more digits after the point than fixed<5,2> has");
"#,
        ),
    );
}

/// §7.2.4.4: "An IDL array shall be mapped to a Java array of the mapped
/// element type. [...] Bound violations for the array shall raise a
/// java.lang.IndexOutOfBoundsException exception", with the clause's `S2`.
#[test]
fn idl4java_7_2_4_4_an_array_is_a_java_array_and_its_bounds_are_checked() {
    let files = java(
        "const long foo_array_length = 200;
         struct Foo { long a; };
         struct Bar { long b; };
         struct S2 {
           long array1[100];
           short array2[10];
           Foo array3[foo_array_length];
           Bar array4[12];
         };",
        &idl_naming(),
    );
    compile_and_run(
        "arrays",
        &files,
        &check_program(
            r#"
        S2.class.getConstructor(int[].class, short[].class, Foo[].class, Bar[].class);
        returns(S2.class, "get_array1", int[].class);
        returns(S2.class, "set_array3", void.class, Foo[].class);
        final S2 s = new S2();
        check(s.get_array3().length == 200 && s.get_array3()[199] != null, "Foo[200]");
        throwsIt(IndexOutOfBoundsException.class, () -> s.set_array1(new int[99]), "int[99] for long[100]");
        check(foo_array_length.value == 200, "the bounding constant");
"#,
        ),
    );
}

/// §7.2.4.6: "the IDL typedef does not result in any Java types. The use of
/// an IDL typedef type shall be replaced with the type referenced by the
/// typedef type" — the clause's `S3` — and "Annotations on an IDL typedef
/// shall be applied to uses of the typedef in other type declarations": the
/// clause's `@max(100)` reaches a member and a sequence's elements.
#[test]
fn idl4java_7_2_4_6_a_typedef_is_replaced_and_its_annotations_reach_its_uses() {
    let files = java(
        "typedef long Length;
         struct S3 { Length member_length; };
         typedef @max(100) long Bounded;
         struct MyType { Bounded a; sequence<Bounded> lengths; };
         @annotation Tag { string note default \"\"; };
         typedef @Tag(note = \"from the typedef\") long Tagged;
         struct Holder { Tagged t; @Tag(note = \"own\") Tagged both; };",
        &idl_naming(),
    );
    assert!(
        !files.iter().any(|f| f.path.contains("Length")),
        "no Java type for a typedef"
    );
    compile_and_run(
        "typedef",
        &files,
        &check_program(
            r#"
        S3.class.getConstructor(int.class);
        returns(S3.class, "get_member_length", int.class);
        final MyType t = new MyType();
        t.set_a(100);
        throwsIt(IllegalArgumentException.class, () -> t.set_a(101), "@max(100) from the typedef");
        throwsIt(IllegalArgumentException.class,
            () -> t.set_lengths(new org.omg.type.IntegerSeq(new int[] {5, 101})),
            "@max(100) on the elements of a sequence of the typedef");
        Tag tag = Holder.class.getDeclaredField("t").getAnnotation(Tag.class);
        check(tag != null && tag.note().equals("from the typedef"), "a user annotation of the typedef reaches the member");
        TagGroup group = Holder.class.getDeclaredField("both").getAnnotation(TagGroup.class);
        check(group != null && group.value().length == 2, "with the member's own, two instances in the group");
"#,
        ),
    );
}

/// §7.4's example `AnInterface` and §7.4.1's `CustomException`: a public
/// interface of the interface's name, an accessor and a modifier per
/// attribute (no modifier for a `readonly` one), a method per operation
/// with its parameters in IDL order, `out` and `inout` parameters as
/// `org.omg.type.Holder<E>` (§7.1.3), the `raises` clause as a `throws`
/// clause, and the exception a class extending `java.lang.RuntimeException`
/// with the members, accessors and constructors of a structure.
#[test]
fn idl4java_7_4_an_interface_is_a_java_interface_with_holders_and_throws() {
    let idl = "exception CustomException { long error_code; };
               interface Base { void ping(); };
               interface AnInterface : Base {
                 attribute long long_attribute;
                 readonly attribute long long_ro_attribute;
                 void op1(in long in_param, inout long inout_param, out long out_param)
                   raises (CustomException);
                 string op2(in string s);
               };
               interface Fwd;
               interface Fwd { void f(); };";
    let files = java(idl, &idl_naming());
    assert_eq!(
        files.iter().filter(|f| f.path == "Fwd.java").count(),
        1,
        "§7.4.2: a forward declaration has no mapping; the definition has its one file"
    );
    compile_and_run(
        "interface-idl",
        &files,
        &check_program(
            r#"
        check(AnInterface.class.isInterface(), "AnInterface is an interface");
        check(Base.class.isAssignableFrom(AnInterface.class), "it extends its base");
        returns(AnInterface.class, "get_long_attribute", int.class);
        returns(AnInterface.class, "set_long_attribute", void.class, int.class);
        returns(AnInterface.class, "get_long_ro_attribute", int.class);
        absent(AnInterface.class, "set_long_ro_attribute", int.class);
        java.lang.reflect.Method op1 = method(AnInterface.class, "op1",
            int.class, org.omg.type.Holder.class, org.omg.type.Holder.class);
        check(op1.getGenericParameterTypes()[1].getTypeName()
            .equals("org.omg.type.Holder<java.lang.Integer>"), "Holder<Integer> for an inout long");
        check(java.util.Arrays.asList(op1.getExceptionTypes()).contains(CustomException.class),
            "throws CustomException");
        check(RuntimeException.class.isAssignableFrom(CustomException.class),
            "an exception extends RuntimeException");
        returns(CustomException.class, "get_error_code", int.class);
        CustomException e = new CustomException(42);
        check(e.get_error_code() == 42, "the all values constructor of an exception");
        check(new CustomException().get_error_code() == 0, "its default constructor");
        // An implementation and a call through the holders.
        AnInterface impl = new AnInterface() {
            private int value = 1;
            public void ping() { }
            public int get_long_attribute() { return value; }
            public void set_long_attribute(int v) { value = v; }
            public int get_long_ro_attribute() { return 7; }
            public void op1(int in, org.omg.type.Holder<Integer> inout, org.omg.type.Holder<Integer> out)
                throws CustomException {
                if (in < 0) { throw new CustomException(in); }
                out.value = inout.value + in;
                inout.value = in;
            }
            public String op2(String s) { return s; }
        };
        org.omg.type.Holder<Integer> inout = new org.omg.type.Holder<Integer>(10);
        org.omg.type.Holder<Integer> out = new org.omg.type.Holder<Integer>();
        impl.op1(5, inout, out);
        check(out.value == 15 && inout.value == 5, "out and inout through Holder");
        try {
            impl.op1(-3, inout, out);
            check(false, "op1 raises");
        } catch (CustomException raised) {
            check(raised.get_error_code() == -3, "the raised exception carries its member");
        }
"#,
        ),
    );
    let files = java(idl, &java_naming());
    compile_and_run(
        "interface-java",
        &files,
        &check_program(
            r#"
        returns(AnInterface.class, "getLongAttribute", int.class);
        returns(AnInterface.class, "setLongAttribute", void.class, int.class);
        returns(AnInterface.class, "getLongRoAttribute", int.class);
        method(AnInterface.class, "op1", int.class, org.omg.type.Holder.class,
            org.omg.type.Holder.class);
        returns(CustomException.class, "getErrorCode", int.class);
"#,
        ),
    );
}

/// §7.5's example `FullInterface`: the types, constants and exceptions an
/// interface body declares are public declarations within the scope of the
/// Java interface.
#[test]
fn idl4java_7_5_declarations_embedded_in_an_interface_are_nested_in_it() {
    let files = java(
        "interface FullInterface {
           struct S { long a; };
           const double PI = 3.14;
           exception Failed { string why; };
           void op1(in S s_in) raises (Failed);
         };",
        &idl_naming(),
    );
    compile_and_run(
        "embedded",
        &files,
        &check_program(
            r#"
        FullInterface.S s = new FullInterface.S(3);
        check(s.get_a() == 3, "the nested structure");
        check(FullInterface.PI.value == 3.14, "the nested constant");
        check(new FullInterface.Failed("x").get_why().equals("x"), "the nested exception");
        method(FullInterface.class, "op1", FullInterface.S.class);
"#,
        ),
    );
}

/// §7.6's example `VT1` and `VT2`: an abstract class `<Name>Abstract` with
/// the attributes and public state as public fields, the private state
/// protected, the operations and the initializers (returning void) as
/// abstract methods, and the class `<Name>` extending it and overriding
/// every method; a base value type's class is what the derived abstract
/// class extends, and a supported interface is what it implements.
#[test]
fn idl4java_7_6_a_value_type_is_an_abstract_and_a_concrete_class() {
    let files = java(
        "valuetype VT1 {
           attribute long a_long_attr;
           void vt_op(in long p_long);
           public long a_public_long;
           private long a_private_long;
           factory vt_factory (in long a_long, in short a_short);
         };
         interface MyInterface { void op(); };
         valuetype VT2 : VT1 supports MyInterface {
           public long third_long;
         };",
        &idl_naming(),
    );
    compile_and_run(
        "valuetype",
        &files,
        &check_program(
            r#"
        check(java.lang.reflect.Modifier.isAbstract(VT1Abstract.class.getModifiers()), "VT1Abstract is abstract");
        check(VT1Abstract.class.isAssignableFrom(VT1.class), "VT1 extends VT1Abstract");
        check(java.lang.reflect.Modifier.isPublic(VT1Abstract.class.getField("a_long_attr").getModifiers()),
            "an attribute is a public field");
        check(java.lang.reflect.Modifier.isPublic(VT1Abstract.class.getField("a_public_long").getModifiers()),
            "public state");
        check(java.lang.reflect.Modifier.isProtected(VT1Abstract.class.getDeclaredField("a_private_long").getModifiers()),
            "private state is protected");
        check(java.lang.reflect.Modifier.isAbstract(method(VT1Abstract.class, "vt_op", int.class).getModifiers()),
            "an operation is abstract in the abstract class");
        returns(VT1Abstract.class, "vt_factory", void.class, int.class, short.class);
        check(!java.lang.reflect.Modifier.isAbstract(method(VT1.class, "vt_op", int.class).getModifiers()),
            "VT1 overrides it");
        check(VT1.class.isAssignableFrom(VT2Abstract.class), "VT2Abstract extends VT1");
        check(MyInterface.class.isAssignableFrom(VT2Abstract.class), "VT2Abstract implements MyInterface");
        check(VT2Abstract.class.isAssignableFrom(VT2.class), "VT2 extends VT2Abstract");
        method(VT2.class, "op");
        final VT2 v = new VT2();
        v.third_long = 3;
        v.a_long_attr = 4;
        check(v.third_long == 3 && v.a_long_attr == 4, "the fields");
        throwsIt(UnsupportedOperationException.class, () -> v.op(), "an operation not implemented");
"#,
        ),
    );
}

/// §7.14.1's example `S5 : S1`: the class of a derived structure extends
/// the base's, and "The all values constructor for the derived struct's
/// Java class shall take as its first parameter a non-null instance of the
/// base struct's Java class".
#[test]
fn idl4java_7_14_1_a_derived_structure_extends_its_base_class() {
    let files = java(
        "struct S1 { long long_variable; string URL; };
         struct S5 : S1 { float float_variable; };",
        &idl_naming(),
    );
    compile_and_run(
        "inheritance",
        &files,
        &check_program(
            r#"
        check(S1.class.isAssignableFrom(S5.class), "S5 extends S1");
        S5 s = new S5(new S1(7, "u"), 1.5f);
        check(s.get_long_variable() == 7 && s.get_URL().equals("u"), "the base's members");
        check(s.get_float_variable() == 1.5f, "its own member");
        throwsIt(NullPointerException.class, () -> new S5(null, 0f), "a null parent");
"#,
        ),
    );
}

/// §7.14.2: `int8`, `uint8`, `wchar` and `octet` discriminate a union as
/// §7.2.4.3.2 maps it.
#[test]
fn idl4java_7_14_2_int8_uint8_wchar_and_octet_discriminate_a_union() {
    let files = java(
        "union UA switch (int8) { case -1: long x; case 1: short y; };
         union UB switch (uint8) { case 200: long x; default: short y; };
         union UC switch (wchar) { case L'x': long x; case L'y': short y; };
         union UD switch (octet) { case 255: long x; };",
        &idl_naming(),
    );
    compile_and_run(
        "discriminators",
        &files,
        &check_program(
            r#"
        returns(UA.class, "get_discriminator", byte.class);
        returns(UB.class, "get_discriminator", byte.class);
        returns(UC.class, "get_discriminator", char.class);
        returns(UD.class, "get_discriminator", byte.class);
        UA a = new UA(); a.set_x(1); check(a.get_discriminator() == -1, "int8 label -1");
        UB b = new UB(); b.set_x(1); check(b.get_discriminator() == (byte) 200, "uint8 label 200");
        UC c = new UC(); c.set_y((short) 2); check(c.get_discriminator() == 'y', "wchar label");
        UD d = new UD(); d.set_x(1); check(d.get_discriminator() == (byte) 255, "octet label 255");
"#,
        ),
    );
}

/// §7.14.3.1's example `S4` and Table 7.5: a map is a `java.util.Map` of
/// the boxed key and value types — the unsigned ones a width up — and a
/// bounded one is checked.
#[test]
fn idl4java_7_14_3_1_a_map_is_a_java_util_map_of_table_7_5_types() {
    let files = java(
        "struct Foo { long a; };
         struct S4 {
           map<long, string> map1;
           map<string, Foo> map2;
           map<unsigned short, unsigned long> map3;
           map<unsigned long long, octet, 2> map4;
         };",
        &idl_naming(),
    );
    compile_and_run(
        "maps",
        &files,
        &check_program(
            r#"
        check(method(S4.class, "get_map1").getGenericReturnType().getTypeName()
            .equals("java.util.Map<java.lang.Integer, java.lang.String>"), "map1");
        check(method(S4.class, "get_map2").getGenericReturnType().getTypeName()
            .equals("java.util.Map<java.lang.String, Foo>"), "map2");
        check(method(S4.class, "get_map3").getGenericReturnType().getTypeName()
            .equals("java.util.Map<java.lang.Integer, java.lang.Long>"), "Table 7.5 boxes unsigned a width up");
        check(method(S4.class, "get_map4").getGenericReturnType().getTypeName()
            .equals("java.util.Map<java.math.BigInteger, java.lang.Byte>"), "uint64 is BigInteger");
        final S4 s = new S4();
        check(s.get_map1().isEmpty(), "an empty map by default");
        final java.util.Map<java.math.BigInteger, Byte> three = new java.util.HashMap<java.math.BigInteger, Byte>();
        three.put(java.math.BigInteger.ONE, (byte) 1);
        three.put(java.math.BigInteger.TEN, (byte) 2);
        three.put(java.math.BigInteger.ZERO, (byte) 3);
        throwsIt(IndexOutOfBoundsException.class, () -> s.set_map4(three), "a bounded map past its bound");
"#,
        ),
    );
}

/// §7.14.3.2's example `MyBitset`: a public class with an accessor and a
/// modifier per named bit field, the type of a field that names none the
/// smallest that holds it (boolean, octet, unsigned short, unsigned long,
/// unsigned long long).
#[test]
fn idl4java_7_14_3_2_a_bit_set_is_a_class_with_bit_field_accessors() {
    let files = java(
        "bitset MyBitset {
           bitfield<3> a;
           bitfield<1> b;
           bitfield<4>;
           bitfield<12, short> d;
           bitfield<20> e;
           bitfield<40> f;
         };",
        &idl_naming(),
    );
    compile_and_run(
        "bitset",
        &files,
        &check_program(
            r#"
        returns(MyBitset.class, "get_a", byte.class);
        returns(MyBitset.class, "set_a", void.class, byte.class);
        returns(MyBitset.class, "get_b", boolean.class);
        returns(MyBitset.class, "get_d", short.class);
        returns(MyBitset.class, "get_e", int.class);
        returns(MyBitset.class, "get_f", long.class);
        final MyBitset s = new MyBitset();
        s.set_a((byte) 7);
        check(s.get_a() == 7, "a");
        throwsIt(IllegalArgumentException.class, () -> s.set_a((byte) 8), "8 does not fit 3 bits");
"#,
        ),
    );
}

/// §7.14.3.3's example: a bit mask is a Java `enum` named `<Name>Flags`,
/// one constant per flag, whose value follows `@position`, and a member of
/// the bit mask is a `java.util.BitSet`; "If the size (number of bits)
/// exceeds that specified by the @bit_bound annotation, a
/// java.lang.IndexOutOfBoundsException exception shall be raised".
#[test]
fn idl4java_7_14_3_3_a_bit_mask_is_an_enum_of_flags_and_a_bit_set() {
    let idl = "bitmask MyBitMask { flag0, flag1, @position(5) flag5 };
               @bit_bound(8) bitmask Small { low, high };
               struct BitmaskExample { MyBitMask a_bitset; Small narrow; };";
    let files = java(idl, &idl_naming());
    compile_and_run(
        "bitmask-idl",
        &files,
        &check_program(
            r#"
        check(MyBitMaskFlags.class.isEnum(), "MyBitMaskFlags is a Java enum");
        check(MyBitMaskFlags.flag1.getPosition() == 1 && MyBitMaskFlags.flag5.getPosition() == 5,
            "@position");
        check(MyBitMaskFlags.flag5.getValue() == 32L, "the value is the power of two of the position");
        returns(BitmaskExample.class, "get_a_bitset", java.util.BitSet.class);
        final BitmaskExample e = new BitmaskExample();
        java.util.BitSet bits = new java.util.BitSet();
        bits.set(MyBitMaskFlags.flag5.getPosition());
        e.set_a_bitset(bits);
        check(e.get_a_bitset().get(5), "a flag set through the enum");
        final java.util.BitSet past = new java.util.BitSet();
        past.set(8);
        throwsIt(IndexOutOfBoundsException.class, () -> e.set_narrow(past), "bit 8 past @bit_bound(8)");
"#,
        ),
    );
    let files = java(idl, &java_naming());
    compile_and_run(
        "bitmask-java",
        &files,
        &check_program(
            r#"
        check(MyBitMaskFlags.FLAG0.getPosition() == 0, "upper case under the Java scheme");
"#,
        ),
    );
}

/// §7.16.1 and §7.16.2's examples: a user annotation is a Java annotation
/// type of its name with its members as elements, their defaults as
/// `default`, and a `<Name>Group` holding several; an element with one
/// instance carries the annotation, one with several the group.
#[test]
fn idl4java_7_16_a_user_annotation_is_a_java_annotation_type_and_its_group() {
    let files = java(
        "@annotation MyAnnotation { boolean value default TRUE; };
         @MyAnnotation
         struct AnnotatedStruct { long a_long; };
         @MyAnnotation(TRUE)
         @MyAnnotation(FALSE)
         struct MultiAnnotatedStruct { long a_long; };",
        &idl_naming(),
    );
    compile_and_run(
        "annotations",
        &files,
        &check_program(
            r#"
        check(MyAnnotation.class.isAnnotation(), "MyAnnotation is an annotation type");
        check(MyAnnotationGroup.class.isAnnotation(), "MyAnnotationGroup too");
        check(MyAnnotation.class.getMethod("value").getDefaultValue().equals(Boolean.TRUE),
            "the default TRUE");
        MyAnnotation one = AnnotatedStruct.class.getAnnotation(MyAnnotation.class);
        check(one != null && one.value(), "@MyAnnotation on the class");
        MyAnnotationGroup group = MultiAnnotatedStruct.class.getAnnotation(MyAnnotationGroup.class);
        check(group != null && group.value().length == 2, "the group of two");
        check(group.value()[0].value() && !group.value()[1].value(), "in order, with their values");
"#,
        ),
    );
}

/// Tables 7.6 to 7.8: `@optional` and `@external` box a basic type,
/// `@default` and `@default_literal` are the default constructor's values,
/// and `@range`, `@min` and `@max` are "tested in the member modifier
/// (setter), and a java.lang.IllegalArgumentException is raised if the
/// parameter does not meet requirements".
#[test]
fn idl4java_7_17_the_standardized_annotations_shape_the_class() {
    let files = java(
        "enum Level { LOW, @default_literal MEDIUM, HIGH };
         struct Tuned {
           @optional long maybe;
           @external long boxed;
           @default(5) long five;
           @default(\"hello\") string greeting;
           Level level_now;
           @range(min = 1, max = 10) long ranged;
           @min(0.5) double half;
           @max(200) unsigned long upper;
         };",
        &idl_naming(),
    );
    compile_and_run(
        "standard-annotations",
        &files,
        &check_program(
            r#"
        returns(Tuned.class, "get_maybe", Integer.class);
        returns(Tuned.class, "get_boxed", Integer.class);
        final Tuned t = new Tuned();
        check(t.get_maybe() == null, "an absent optional member");
        check(t.get_boxed() == 0, "an external member has a value");
        check(t.get_five() == 5 && t.get_greeting().equals("hello"), "@default");
        check(t.get_level_now() == Level.MEDIUM, "@default_literal");
        t.set_ranged(10);
        throwsIt(IllegalArgumentException.class, () -> t.set_ranged(11), "@range max");
        throwsIt(IllegalArgumentException.class, () -> t.set_ranged(0), "@range min");
        throwsIt(IllegalArgumentException.class, () -> t.set_half(0.25), "@min(0.5)");
        t.set_upper(200);
        throwsIt(IllegalArgumentException.class, () -> t.set_upper(-1), "@max(200) on the bit pattern of 4294967295");
        t.set_maybe(null);
"#,
        ),
    );
}

/// §8.1: `@java_mapping` on a module — the naming scheme, the width of
/// the unsigned integers (§8.1.3, Table 8.2) and the string type (§8.1.4)
/// — reaches what the module declares, and a declaration's own setting
/// wins over its module's.
#[test]
fn idl4java_8_1_java_mapping_sets_the_scheme_the_integer_width_and_the_string_type() {
    let files = java(
        "@java_mapping(apply_naming_convention = JAVA_NAMING_CONVENTION,
                       promote_integer_width = TRUE,
                       string_type = \"java.lang.StringBuilder\")
         module Tuned_Module {
           struct my_point { unsigned short x_value; string label; };
           @java_mapping(apply_naming_convention = IDL_NAMING_CONVENTION)
           struct kept_as_is { long a_b; };
         };",
        &idl_naming(),
    );
    file(&files, "tuned_module/MyPoint.java");
    file(&files, "tuned_module/kept_as_is.java");
    compile_and_run(
        "java-mapping",
        &files,
        &check_program(
            r#"
        returns(tuned_module.MyPoint.class, "getXValue", int.class);
        returns(tuned_module.MyPoint.class, "getLabel", StringBuilder.class);
        check(new tuned_module.MyPoint().getLabel().length() == 0, "an empty StringBuilder");
        returns(tuned_module.kept_as_is.class, "get_a_b", int.class);
"#,
        ),
    );
}

/// §7.1.1 and §7.1.2: a mapped name that is a Java keyword or literal, or
/// `Constants`, takes a leading underscore; §7.1.1.3: so does a name that
/// ends in the reserved suffix `Abstract`; a method that would collide
/// with a method of `java.lang.Object` too.
#[test]
fn idl4java_7_1_2_reserved_names_take_a_leading_underscore() {
    let files = java(
        "struct _class { long _null; };
         struct FooAbstract { long a; };
         struct Constants { long b; };
         interface I { void _hashCode(); };",
        &idl_naming(),
    );
    file(&files, "_class.java");
    file(&files, "_FooAbstract.java");
    file(&files, "_Constants.java");
    compile_and_run(
        "reserved",
        &files,
        &check_program(
            r#"
        returns(_class.class, "get_null", int.class);
        method(I.class, "_hashCode");
"#,
        ),
    );
}

/// §7.3: `any` is `org.omg.type.Any`, which carries a value and its type.
#[test]
fn idl4java_7_3_any_is_org_omg_type_any() {
    let files = java("struct Holds { any thing; };", &idl_naming());
    compile_and_run(
        "any",
        &files,
        &check_program(
            r#"
        returns(Holds.class, "get_thing", org.omg.type.Any.class);
        Holds h = new Holds();
        h.get_thing().insert(Integer.valueOf(5));
        check(h.get_thing().type() == Integer.class, "the actual type");
        check(h.get_thing().extract(Integer.class) == 5, "the value");
"#,
        ),
    );
}

/// §7.2.4.3.4: "Constructed recursive types are supported by mapping the
/// involved types directly to Java": a structure that holds a sequence of
/// itself, and a union that does, are classes whose members name their own
/// class, and a value built from them nests.
#[test]
fn idl4java_7_2_4_3_4_a_recursive_type_names_its_own_class() {
    let idl = "struct Node { long value; sequence<Node> children; };
               union Tree switch (boolean) {
                 case TRUE: sequence<Tree> branches;
                 case FALSE: long leaf;
               };";
    let files = java(idl, &idl_naming());
    let check = check_program(
        r#"
        returns(Node.class, "get_children", java.util.List.class);
        Node leaf = new Node(2, new java.util.ArrayList<Node>());
        java.util.List<Node> kids = new java.util.ArrayList<>();
        kids.add(leaf);
        Node root = new Node(1, kids);
        check(root.get_children().get(0).get_value() == 2, "a node holds nodes");
        Tree t = new Tree();
        java.util.List<Tree> branches = new java.util.ArrayList<>();
        Tree inner = new Tree();
        inner.set_leaf(5);
        branches.add(inner);
        t.set_branches(branches);
        check(t.get_branches().get(0).get_leaf() == 5, "a tree holds trees");
"#,
    );
    compile_and_run("recursive", &files, &check);
}

/// §7.17.5, Table 7.10: `@verbatim` "Copies verbatim text to the indicated
/// output position when the indicated language is "*" or "java"" — and not
/// for another language.
#[test]
fn idl4java_7_17_5_verbatim_text_is_copied_for_java_and_star_only() {
    let idl = r#"@verbatim(language = "java", placement = BEGIN_DECLARATION,
                         text = "public static final int FROM_JAVA = 1;")
               @verbatim(language = "*", placement = END_DECLARATION,
                         text = "public static final int FROM_STAR = 2;")
               @verbatim(language = "c", placement = BEGIN_DECLARATION,
                         text = "int from_c;")
               @verbatim(language = "java", placement = BEFORE_DECLARATION,
                         text = "/** written before the class */")
               struct Tagged { long m; };"#;
    let files = java(idl, &idl_naming());
    let tagged = file(&files, "Tagged.java");
    assert!(!tagged.contains("from_c"), "{tagged}");
    let before = tagged
        .find("/** written before the class */")
        .expect("BEFORE_DECLARATION");
    let class = tagged.find("public class Tagged").expect("the class");
    assert!(before < class, "{tagged}");
    let check = check_program(
        r#"
        check(Tagged.FROM_JAVA == 1, "BEGIN_DECLARATION for java");
        check(Tagged.FROM_STAR == 2, "END_DECLARATION for *");
"#,
    );
    compile_and_run("verbatim", &files, &check);
}

/// §7.17.6, Table 7.11: `@service`, `@oneway` and `@ami` have an impact
/// that "is middleware specific" — none on the language mapping: the
/// interface maps to the same Java with them as without them.
#[test]
fn idl4java_7_17_6_the_interface_annotations_leave_the_java_unchanged() {
    let plain = java(
        "interface Bank { long deposit(in long amount); void close(); };",
        &idl_naming(),
    );
    for annotated in [
        "@service(\"CORBA\") interface Bank { long deposit(in long amount); void close(); };",
        "@service(\"DDS\") interface Bank { long deposit(in long amount); void close(); };",
        "interface Bank { @ami long deposit(in long amount); @oneway void close(); };",
    ] {
        let files = java(annotated, &idl_naming());
        assert_eq!(
            file(&files, "Bank.java"),
            file(&plain, "Bank.java"),
            "{annotated}"
        );
    }
    let check = check_program(
        r#"
        returns(Bank.class, "deposit", int.class, int.class);
        returns(Bank.class, "close", void.class);
"#,
    );
    compile_and_run("interface-annotations", &plain, &check);
}

/// Tables 7.6, 7.7, 7.8 and 7.9: `@id`, `@autoid`, `@extensibility`,
/// `@final`, `@mutable`, `@appendable`, `@key`, `@must_understand`,
/// `@unit` and `@nested` have "No impact on mapping": the class is the one
/// the structure maps to without them.
#[test]
fn idl4java_7_17_the_annotations_without_impact_leave_the_class_unchanged() {
    let plain = java(
        "struct Reading { long sensor; double value; };",
        &idl_naming(),
    );
    for annotated in [
        "@autoid(SEQUENTIAL) struct Reading { @id(7) long sensor; double value; };",
        "@extensibility(MUTABLE) struct Reading { long sensor; double value; };",
        "@final struct Reading { long sensor; double value; };",
        "@mutable struct Reading { long sensor; double value; };",
        "@appendable struct Reading { long sensor; double value; };",
        "struct Reading { @key long sensor; @must_understand double value; };",
        "@nested struct Reading { long sensor; @unit(\"K\") double value; };",
    ] {
        let files = java(annotated, &idl_naming());
        assert_eq!(
            file(&files, "Reading.java"),
            file(&plain, "Reading.java"),
            "{annotated}"
        );
    }
}
