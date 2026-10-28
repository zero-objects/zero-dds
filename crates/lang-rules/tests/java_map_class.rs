// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Java binding of a map: DDS-XTypes 1.3 §7.5.1.3.1 and §7.5.1.3.5.
//!
//! "An IDL map type shall be represented in Java by an implementation of
//! the standard java.util.Map interface." The generated Java goes through
//! `javac` under `-Xlint:all -Werror`, with the wire runtime, and a program
//! uses the map as a `java.util.Map`, calls the operations of Table 47 Java
//! does not have, and sends a sample with maps through the generated codec.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_ast::Ast;
use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::Language;

const IDL: &str = "\
module m {
  struct Foo { long a; };
  enum Color { RED, GREEN };
  @final struct Holder {
    map<unsigned long, Foo> by_id;
    map<string, long> counts;
    map<long, string, 3> small;
    map<unsigned long long, Color> colors;
  };
};
";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root")
}

fn javac_available() -> bool {
    Command::new("javac")
        .arg("-version")
        .output()
        .is_ok_and(|output| output.status.success())
        && Command::new("java")
            .arg("-version")
            .output()
            .is_ok_and(|output| output.status.success())
}

fn generated(idl: &str) -> String {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "maps.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast: Ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("resolve");
    assert!(report.is_clean(), "{:?}", report.broken);
    // the Java programs read the IDL-to-Java shape (public fields)
    zerodds_lang_rules::adapter::java::with_mapping(
        zerodds_lang_rules::adapter::java::JavaMapping::Omg,
        || {
            zerodds_lang_rules::generate::source(
                &ast,
                Language::Java,
                LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
            )
        },
    )
    .expect("the Java column renders the maps")
}

const MAIN: &str = r#"
import org.zerodds.wire.*;

public final class Main {
    static int bad = 0;

    static void check(boolean condition, String what) {
        if (!condition) {
            System.out.println("FAIL: " + what);
            bad++;
        }
    }

    public static void main(String[] args) throws Exception {
        // new, initialize: a java.util.Map, empty, with no capacity.
        ZdTypes.ZdMap_string_long counts = new ZdTypes.ZdMap_string_long();
        java.util.Map<java.lang.String, java.lang.Integer> asMap = counts;
        check(asMap.isEmpty() && counts.getMaxSize() == 0, "initialize");
        // insert past the capacity, then with room.
        check(counts.insert("one", 1, false) == ZdTypes.ZdMap_string_long.RETCODE_PRECONDITION_NOT_MET, "full");
        check(counts.setMaxSize(2) == ZdTypes.ZdMap_string_long.RETCODE_OK, "set_max_size");
        check(counts.insert("one", 1, false) == ZdTypes.ZdMap_string_long.RETCODE_OK, "insert");
        check(counts.insert("one", 5, false) == ZdTypes.ZdMap_string_long.RETCODE_BAD_PARAMETER, "insert: key exists");
        check(counts.insert("one", 5, true) == ZdTypes.ZdMap_string_long.RETCODE_OK && counts.get("one") == 5, "insert_or_assign");
        check(counts.insert(null, 5, true) == ZdTypes.ZdMap_string_long.RETCODE_BAD_PARAMETER, "a null key");
        check(counts.insert("two", 2, false) == ZdTypes.ZdMap_string_long.RETCODE_OK, "second");
        check(counts.insert("three", 3, false) == ZdTypes.ZdMap_string_long.RETCODE_PRECONDITION_NOT_MET, "past the maximum");
        check(counts.setMaxSize(1) == ZdTypes.ZdMap_string_long.RETCODE_BAD_PARAMETER, "below the size");
        // The order of the entries is the order they were put in.
        java.util.Iterator<java.util.Map.Entry<java.lang.String, java.lang.Integer>> it = counts.entrySet().iterator();
        check(it.next().getKey().equals("one") && it.next().getKey().equals("two") && !it.hasNext(), "order");
        // copy.
        ZdTypes.ZdMap_string_long other = new ZdTypes.ZdMap_string_long();
        check(other.copyFrom(counts, false) == ZdTypes.ZdMap_string_long.RETCODE_PRECONDITION_NOT_MET && other.isEmpty(), "copy without autogrow");
        check(other.copyFrom(counts, true) == ZdTypes.ZdMap_string_long.RETCODE_OK && other.size() == 2 && other.getMaxSize() >= 2, "copy with autogrow");
        check(other.copyFrom(null, true) == ZdTypes.ZdMap_string_long.RETCODE_BAD_PARAMETER, "copy of null");
        // erase and clear are the Map's.
        check(counts.remove("one") != null && counts.size() == 1, "erase");
        counts.clear();
        check(counts.isEmpty() && counts.getMaxSize() == 2, "clear keeps the maximum");

        // A bound is the largest capacity.
        ZdTypes.ZdMap_b3_long_string small = new ZdTypes.ZdMap_b3_long_string();
        check(small.setMaxSize(4) == ZdTypes.ZdMap_b3_long_string.RETCODE_BAD_PARAMETER, "past the bound");
        check(small.setMaxSize(3) == ZdTypes.ZdMap_b3_long_string.RETCODE_OK, "at the bound");

        // A sample with maps through the generated codec.
        ZdTypes.m.Holder holder = new ZdTypes.m.Holder();
        holder.by_id = new ZdTypes.ZdMap_unsigned_long_m_Foo();
        ZdTypes.m.Foo first = new ZdTypes.m.Foo();
        first.a = 11;
        ZdTypes.m.Foo second = new ZdTypes.m.Foo();
        second.a = 22;
        holder.by_id.put(9L, first);
        holder.by_id.put(3L, second);
        holder.counts = new ZdTypes.ZdMap_string_long();
        holder.counts.put("x", 5);
        holder.small = new ZdTypes.ZdMap_b3_long_string();
        holder.small.put(-1, "a");
        holder.colors = new ZdTypes.ZdMap_unsigned_long_long_m_Color();
        holder.colors.put(5L, ZdTypes.m.Color.GREEN);

        Writer w = new Writer(Endianness.LITTLE, 4, PlCdr1Framing.defaults(), false);
        ZdCodec.zd_enc_m_Holder(w, holder);
        byte[] bytes = w.finish();
        Reader r = new Reader(bytes, Endianness.LITTLE, 4, PlCdr1Framing.defaults());
        ZdTypes.m.Holder back = ZdCodec.zd_dec_m_Holder(r, TryConstruct.DISCARD);
        check(back.by_id.size() == 2, "two entries back");
        java.util.Iterator<java.lang.Long> keys = back.by_id.keySet().iterator();
        check(keys.next() == 9L && keys.next() == 3L, "the entries come back in the order they were put");
        check(back.by_id.get(3L).a == 22, "an element");
        check(back.counts.get("x") == 5, "a string key");
        check(back.small.get(-1).equals("a"), "a bounded map");
        check(back.colors.get(5L) == ZdTypes.m.Color.GREEN, "an enumeration element");
        // What was read fills the map; there is no room until someone makes some.
        check(back.by_id.getMaxSize() == 2, "the maximum of a map that was read");
        check(back.by_id.insert(1L, first, false) == ZdTypes.ZdMap_unsigned_long_m_Foo.RETCODE_PRECONDITION_NOT_MET, "full after a read");
        System.exit(bad);
    }
}
"#;

fn run(command: &mut Command, what: &str) -> String {
    let output = command.output().unwrap_or_else(|e| panic!("{what}: {e}"));
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "{what}:\n{text}");
    text
}

/// `main` against the Java of `idl`, compiled under `-Xlint:all -Werror` with the wire runtime,
/// and run.
fn build_and_run(tag: &str, idl: &str, main: &str) {
    if !javac_available() {
        eprintln!("SKIPPED: `javac` or `java` is not usable.");
        return;
    }
    let root = repository_root();
    let dir = std::env::temp_dir().join(format!("zd-java-{tag}-{}", std::process::id()));
    let classes = dir.join("classes");
    std::fs::create_dir_all(&classes).expect("a work directory");
    let source = generated(idl);
    let units = zerodds_lang_rules::generate::java_units(&source);
    assert!(!units.is_empty(), "no compilation units in:\n{source}");
    let mut files = Vec::new();
    for unit in &units {
        let file = dir.join(format!("{}.java", unit.class));
        // The imports stand with the first unit; the others need them too.
        let text = if unit.text.contains("import org.zerodds.wire") {
            unit.text.clone()
        } else {
            format!("import org.zerodds.wire.*;\n{}", unit.text)
        };
        std::fs::write(&file, text).unwrap();
        files.push(file);
    }
    std::fs::write(dir.join("Main.java"), main).unwrap();
    files.push(dir.join("Main.java"));
    // The wire runtime.
    let wire = root.join("runtime/java/src/main/java/org/zerodds/wire");
    let mut runtime: Vec<PathBuf> = std::fs::read_dir(&wire)
        .unwrap()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "java"))
        .collect();
    runtime.sort();
    run(
        Command::new("javac")
            .args(["-encoding", "UTF-8"])
            .arg("-d")
            .arg(&classes)
            .args(&runtime),
        "javac of the wire runtime",
    );
    run(
        Command::new("javac")
            .args(["-encoding", "UTF-8", "-Xlint:all,-options", "-Werror"])
            .arg("-cp")
            .arg(&classes)
            .arg("-d")
            .arg(&classes)
            .args(&files),
        "javac of the generated Java",
    );
    let output = Command::new("java")
        .arg("-cp")
        .arg(&classes)
        .arg("Main")
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "{} check(s) failed:\n{text}",
        output.status.code().unwrap_or(-1)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_map_is_a_java_util_map_with_the_operations_of_table_47() {
    build_and_run("map", IDL, MAIN);
}

const INHERITANCE_IDL: &str = "\
module m {
  struct Base { long a; };
  struct Derived : Base { string s; };
  struct Flat { long a; string s; };
};
";

const INHERITANCE_MAIN: &str = r#"
import org.zerodds.wire.*;

public final class Main {
    static int bad = 0;

    static void check(boolean condition, String what) {
        if (!condition) {
            System.out.println("FAIL: " + what);
            bad++;
        }
    }

    public static void main(String[] args) throws Exception {
        // The class of the subtype extends the class of the supertype.
        check(ZdTypes.m.Base.class.isAssignableFrom(ZdTypes.m.Derived.class), "extends");
        check(ZdTypes.m.Derived.class.getSuperclass() == ZdTypes.m.Base.class, "the direct superclass");
        check(ZdTypes.m.Derived.class.getDeclaredFields().length == 1, "only its own members are declared");
        ZdTypes.m.Derived derived = new ZdTypes.m.Derived();
        derived.a = 7;
        derived.s = "text";
        ZdTypes.m.Base base = derived;
        check(base.a == 7, "a derived value is a base value");
        // 7.4.1.1.5.1: the representation is exactly as if all the members had been defined, in
        // the same order, in the most-derived type.
        ZdTypes.m.Flat flat = new ZdTypes.m.Flat();
        flat.a = 7;
        flat.s = "text";
        Writer w1 = new Writer(Endianness.LITTLE, 4, PlCdr1Framing.defaults(), false);
        ZdCodec.zd_enc_m_Derived(w1, derived);
        Writer w2 = new Writer(Endianness.LITTLE, 4, PlCdr1Framing.defaults(), false);
        ZdCodec.zd_enc_m_Flat(w2, flat);
        byte[] bytes = w1.finish();
        check(java.util.Arrays.equals(bytes, w2.finish()), "the bytes of the derived and the flat type");
        Reader r = new Reader(bytes, Endianness.LITTLE, 4, PlCdr1Framing.defaults());
        ZdTypes.m.Derived back = ZdCodec.zd_dec_m_Derived(r, TryConstruct.DISCARD);
        check(back.a == 7 && back.s.equals("text"), "the value comes back");
        System.exit(bad);
    }
}
"#;

#[test]
fn a_derived_class_extends_its_base_class() {
    build_and_run("inheritance", INHERITANCE_IDL, INHERITANCE_MAIN);
}

const KEYS_IDL: &str = "\
module m {
  @final struct Point { long x; long y; };
  @final struct Tagged { string name; sequence<octet> data; };
  @final union Choice switch (long) { case 1: long number; case 2: string text; };
  @final struct Holder {
    map<Point, string> by_point;
    map<Choice, long> by_choice;
    map<Tagged, long> by_tagged;
  };
};
";

const KEYS_MAIN: &str = r#"
import org.zerodds.wire.*;

public final class Main {
    static int bad = 0;

    static void check(boolean condition, String what) {
        if (!condition) {
            System.out.println("FAIL: " + what);
            bad++;
        }
    }

    static ZdTypes.m.Point point(int x, int y) {
        ZdTypes.m.Point p = new ZdTypes.m.Point();
        p.x = x;
        p.y = y;
        return p;
    }

    public static void main(String[] args) throws Exception {
        // XTypes 1.3 7.5.1.3.5: a structure is a key of a java.util.Map; the class compares by
        // its members.
        ZdTypes.ZdMap_m_Point_string table = new ZdTypes.ZdMap_m_Point_string();
        java.util.Map<ZdTypes.m.Point, java.lang.String> asMap = table;
        asMap.put(point(1, 2), "a");
        asMap.put(point(3, 4), "b");
        check(asMap.get(point(1, 2)).equals("a"), "found by an equal key");
        check(asMap.get(point(2, 1)) == null, "not found by another");
        check(point(1, 2).equals(point(1, 2)) && point(1, 2).hashCode() == point(1, 2).hashCode(), "equals and hashCode");
        check(!point(1, 2).equals(point(1, 3)) && !point(1, 2).equals(null), "not equal");
        check(asMap.put(point(1, 2), "c").equals("a") && asMap.size() == 2, "the same key is replaced");

        // An array member is compared by its elements.
        ZdTypes.m.Tagged t1 = new ZdTypes.m.Tagged();
        t1.name = "n";
        t1.data = new byte[] {1, 2};
        ZdTypes.m.Tagged t2 = new ZdTypes.m.Tagged();
        t2.name = "n";
        t2.data = new byte[] {1, 2};
        check(t1.equals(t2) && t1.hashCode() == t2.hashCode(), "arrays by their elements");

        // A union is a key by its discriminator and its case.
        ZdTypes.m.Choice c1 = new ZdTypes.m.Choice();
        c1._d = 1;
        c1.number = 5;
        ZdTypes.m.Choice c2 = new ZdTypes.m.Choice();
        c2._d = 1;
        c2.number = 5;
        ZdTypes.m.Choice c3 = new ZdTypes.m.Choice();
        c3._d = 1;
        c3.number = 6;
        ZdTypes.ZdMap_m_Choice_long choices = new ZdTypes.ZdMap_m_Choice_long();
        choices.put(c1, 10);
        check(choices.get(c2) == 10 && choices.get(c3) == null, "a union key");

        // Through the codec.
        ZdTypes.m.Holder holder = new ZdTypes.m.Holder();
        holder.by_point = table;
        holder.by_choice = choices;
        holder.by_tagged = new ZdTypes.ZdMap_m_Tagged_long();
        holder.by_tagged.put(t1, 7);
        Writer w = new Writer(Endianness.LITTLE, 4, PlCdr1Framing.defaults(), false);
        ZdCodec.zd_enc_m_Holder(w, holder);
        byte[] bytes = w.finish();
        Reader r = new Reader(bytes, Endianness.LITTLE, 4, PlCdr1Framing.defaults());
        ZdTypes.m.Holder back = ZdCodec.zd_dec_m_Holder(r, TryConstruct.DISCARD);
        check(back.by_point.get(point(3, 4)).equals("b"), "a structure key after the codec");
        check(back.by_choice.get(c2) == 10, "a union key after the codec");
        check(back.by_tagged.get(t2) == 7, "a key with an array after the codec");
        System.exit(bad);
    }
}
"#;

#[test]
fn a_structure_or_union_is_a_key_of_a_java_util_map() {
    build_and_run("keys", KEYS_IDL, KEYS_MAIN);
}

const VALUES_IDL: &str = "\
module m {
  @final struct Sample { octet o; uint8 u; sequence<octet> data; fixed<5,2> price; octet tail[2]; };
};
";

const VALUES_MAIN: &str = r#"
import org.zerodds.wire.*;

public final class Main {
    static int bad = 0;

    static void check(boolean condition, String what) {
        if (!condition) {
            System.out.println("FAIL: " + what);
            bad++;
        }
    }

    static String hex(byte[] bytes) {
        StringBuilder out = new StringBuilder();
        for (byte b : bytes) {
            out.append(String.format("%02x", b & 0xFF));
        }
        return out.toString();
    }

    public static void main(String[] args) throws Exception {
        // CORBA 3.3 part 3 1.5.2 and 1.5.6: `octet` is a byte, `fixed` a BigDecimal.
        ZdTypes.m.Sample sample = new ZdTypes.m.Sample();
        sample.o = (byte) 200;
        sample.u = 200;
        sample.data = new byte[] {(byte) 0x80, 1};
        sample.price = new java.math.BigDecimal("12.50");
        sample.tail = new byte[] {(byte) 255, 7};
        Writer w = new Writer(Endianness.LITTLE, 4, PlCdr1Framing.defaults(), false);
        ZdCodec.zd_enc_m_Sample(w, sample);
        byte[] bytes = w.finish();
        // o, u, a length of 2 and two octets, five digits and a sign, the two octets of tail.
        check(hex(bytes).equals("c8c8" + "0000" + "02000000" + "8001" + "01250c" + "ff07"), "the bytes: " + hex(bytes));
        Reader r = new Reader(bytes, Endianness.LITTLE, 4, PlCdr1Framing.defaults());
        ZdTypes.m.Sample back = ZdCodec.zd_dec_m_Sample(r, TryConstruct.DISCARD);
        check(back.o == (byte) 200 && (back.o & 0xFF) == 200, "the octet comes back");
        check(back.u == 200, "the uint8 comes back");
        check(java.util.Arrays.equals(back.data, new byte[] {(byte) 0x80, 1}), "the octets come back");
        check(back.price.compareTo(new java.math.BigDecimal("12.50")) == 0, "the fixed comes back: " + back.price);
        check(java.util.Arrays.equals(back.tail, new byte[] {(byte) 255, 7}), "the array comes back");
        System.exit(bad);
    }
}
"#;

#[test]
fn an_octet_is_a_byte_and_a_fixed_is_a_bigdecimal() {
    build_and_run("values", VALUES_IDL, VALUES_MAIN);
}

const ENUM_IDL: &str = "\
module m {
  enum Color { RED, GREEN, @value(7) BLUE };
  @final union Pick switch (Color) { case RED: long number; case BLUE: string text; };
  @final struct Holder { Color c; Pick p; sequence<Color> all; };
};
";

const ENUM_MAIN: &str = r#"
import org.zerodds.wire.*;

public final class Main {
    static int bad = 0;

    static void check(boolean condition, String what) {
        if (!condition) {
            System.out.println("FAIL: " + what);
            bad++;
        }
    }

    public static void main(String[] args) throws Exception {
        // CORBA 3.3 part 3 1.5.9: an instance per enumerator, value() and from_int().
        check(ZdTypes.m.Color.BLUE.value() == 7 && ZdTypes.m.Color._BLUE == 7, "value");
        check(ZdTypes.m.Color.from_int(1) == ZdTypes.m.Color.GREEN, "from_int");
        boolean refused = false;
        try {
            ZdTypes.m.Color.from_int(5);
        } catch (IllegalArgumentException expected) {
            refused = true;
        }
        check(refused, "from_int of a value no enumerator has");

        ZdTypes.m.Holder holder = new ZdTypes.m.Holder();
        holder.c = ZdTypes.m.Color.BLUE;
        holder.p = new ZdTypes.m.Pick();
        holder.p._d = ZdTypes.m.Color.BLUE;
        holder.p.text = "seven";
        holder.all = new ZdTypes.m.Color[] {ZdTypes.m.Color.RED, ZdTypes.m.Color.BLUE};
        Writer w = new Writer(Endianness.LITTLE, 4, PlCdr1Framing.defaults(), false);
        ZdCodec.zd_enc_m_Holder(w, holder);
        byte[] bytes = w.finish();
        Reader r = new Reader(bytes, Endianness.LITTLE, 4, PlCdr1Framing.defaults());
        ZdTypes.m.Holder back = ZdCodec.zd_dec_m_Holder(r, TryConstruct.DISCARD);
        check(back.c == ZdTypes.m.Color.BLUE, "the enumerator comes back as its instance");
        check(back.p._d == ZdTypes.m.Color.BLUE && back.p.text.equals("seven"), "a union over an enumeration");
        check(back.all.length == 2 && back.all[0] == ZdTypes.m.Color.RED && back.all[1] == ZdTypes.m.Color.BLUE, "an array of enumerators");
        // IDL to Java 1.3 1.5.10: `discriminator()`, an accessor and a modifier per case.
        ZdTypes.m.Pick pick = new ZdTypes.m.Pick();
        pick.number(5);
        check(pick.discriminator() == ZdTypes.m.Color.RED && pick.number() == 5, "a modifier selects its case");
        boolean refusedAccess = false;
        try {
            pick.text();
        } catch (IllegalStateException expected) {
            refusedAccess = true;
        }
        check(refusedAccess, "an accessor of a case the discriminator does not select");
        pick.text(ZdTypes.m.Color.BLUE, "seven");
        check(pick.discriminator() == ZdTypes.m.Color.BLUE && pick.text().equals("seven"), "the modifier that names the discriminator");
        boolean refusedModifier = false;
        try {
            pick.text(ZdTypes.m.Color.RED, "x");
        } catch (IllegalArgumentException expected) {
            refusedModifier = true;
        }
        check(refusedModifier && pick.discriminator() == ZdTypes.m.Color.BLUE, "a discriminator that selects another case is refused");
        // The value is written as the integer of the enumerator: BLUE is 7.
        check((bytes[0] & 0xFF) == 7 && bytes[1] == 0 && bytes[2] == 0 && bytes[3] == 0, "BLUE is 7 on the wire");
        System.exit(bad);
    }
}
"#;

#[test]
fn an_enumeration_is_a_class_of_instances_with_value_and_from_int() {
    build_and_run("enum", ENUM_IDL, ENUM_MAIN);
}
